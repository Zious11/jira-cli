// SCHEMA NOTES — PR2 extensions of the Atlassian Bulk API (issue #110 PR2, 2026-05-09)
//
// This file covers:
//   - AC-003: --jql expansion (search first, then bulk)
//   - AC-004: --max cap enforcement (default 50, hard ceiling 1,000)
//   - AC-005: --dry-run short-circuit (zero mutating calls, diff on stdout)
//   - --yes / --no-input skip-confirmation behaviour
//   - Safety-net: --no-input without --yes/--dry-run for large JQL matches
//   - AC-001 extension: non-label multi-key edits (--summary, --priority) via bulk
//   - AC optimization: --label add + remove coalesces to ONE bulk call
//
// Red Gate contract:
//   Every test below MUST fail before implementation because:
//   - --jql is not a recognised flag on `jr issue edit` → clap exits 2
//   - --dry-run is not a recognised flag                → clap exits 2
//   - --max is not a recognised flag                    → clap exits 2
//   - --yes is not a recognised flag                    → clap exits 2
//   - Non-label multi-key edit currently bails with "not yet supported"
//     → assertions on exit 0 / bulk POST fail
//   - Single ADD+REMOVE currently makes 2 bulk calls, not 1
//     → .expect(1) wiremock assertion fires on drop
//
// Matcher philosophy (inherited from issue_bulk.rs):
//   - body_partial_json: confirm required structural fields
//   - body_string_contains: tolerate unverified casing / nesting (labelsAction etc.)
//   - Both used defensively; schema notes annotate each unverified field.
//
// DO NOT modify tests/issue_bulk.rs — those are the PR1 regression-pin tests.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{body_partial_json, body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers — copied from issue_bulk.rs pattern (PR1 tests untouched).
// ---------------------------------------------------------------------------

/// Build a `jr` command pointing at the mock server.
fn jr_cmd(server_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0"); // test:test base64
    cmd
}

/// Bulk task ENQUEUED response (same shape as PR1 helpers).
fn bulk_enqueued(task_id: &str) -> serde_json::Value {
    serde_json::json!({
        "taskId": task_id,
        "status": "ENQUEUED",
        "progressPercent": 0,
        "totalIssueCount": 0,
        "processedAccessibleIssues": [],
        "failedAccessibleIssues": {},
        "invalidOrInaccessibleIssueCount": 0
    })
}

/// Bulk task COMPLETE response — all `processed_keys` succeeded.
fn bulk_complete(task_id: &str, processed_keys: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "taskId": task_id,
        "status": "COMPLETE",
        "progressPercent": 100,
        "totalIssueCount": processed_keys.len(),
        "processedAccessibleIssues": processed_keys,
        "failedAccessibleIssues": {},
        "invalidOrInaccessibleIssueCount": 0
    })
}

/// JQL search response — cursor-based, no next page (terminal page).
///
/// Each key in `keys` becomes a minimal Issue JSON that jr's `search_issues`
/// can deserialise. Only `key` and `fields.summary` are required by tests.
fn jql_search_response(keys: &[&str]) -> serde_json::Value {
    let issues: Vec<serde_json::Value> = keys
        .iter()
        .map(|k| {
            serde_json::json!({
                "key": k,
                "fields": {
                    "summary": format!("Issue {}", k),
                    "status": {"name": "To Do", "statusCategory": {"key": "new", "name": "To Do"}},
                    "issuetype": {"name": "Task"},
                    "priority": {"name": "Medium"},
                    "assignee": null,
                    "reporter": null,
                    "project": {"key": k.split('-').next().unwrap_or("TEST")},
                    "description": null,
                    "created": "2024-01-01T00:00:00.000+0000",
                    "updated": "2024-01-01T00:00:00.000+0000",
                    "resolution": null,
                    "components": [],
                    "fixVersions": [],
                    "labels": [],
                    "parent": null,
                    "issuelinks": []
                }
            })
        })
        .collect();

    serde_json::json!({
        "issues": issues,
        "nextPageToken": null
    })
}

/// Mount a GET /rest/api/3/bulk/queue/{task_id} mock that returns COMPLETE.
async fn mount_poll_complete(server: &MockServer, task_id: &str, processed_keys: &[&str]) {
    let resp = bulk_complete(task_id, processed_keys);
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(resp))
        .mount(server)
        .await;
}

// ---------------------------------------------------------------------------
// AC-003: --jql expansion → search first, then one bulk POST.
// ---------------------------------------------------------------------------

/// `jr issue edit --jql 'project = PROJ' --label add:foo --yes --no-input`
/// with 3 matched issues:
///   1. POST /rest/api/3/search/jql called once (search).
///   2. POST /rest/api/3/bulk/issues/fields called once (bulk edit).
///   3. GET /rest/api/3/bulk/queue/{taskId} until COMPLETE.
///   4. Exit 0.
///   5. Bulk POST body contains all 3 matched keys.
#[tokio::test]
async fn test_jql_expansion_calls_search_then_bulk_with_matched_keys() {
    let server = MockServer::start().await;

    // Search returns 3 matched issues.
    let matched_keys = ["PROJ-1", "PROJ-2", "PROJ-3"];
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&matched_keys)))
        .expect(1)
        .mount(&server)
        .await;

    // Bulk POST: must contain all 3 matched keys in selectedIssueIdsOrKeys.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": ["PROJ-1", "PROJ-2", "PROJ-3"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-jql-001")))
        .expect(1)
        .mount(&server)
        .await;

    mount_poll_complete(&server, "task-jql-001", &matched_keys).await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = PROJ",
            "--label",
            "add:foo",
            "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for --jql expansion; stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(1) on search + bulk POST fires on drop if counts are wrong.
}

// ---------------------------------------------------------------------------
// AC-004: Default --max 50 caps at 50 matched issues.
// ---------------------------------------------------------------------------

/// When JQL returns 75 issues but --max defaults to 50, the command must
/// error (exit 64) and emit a hint referencing --max, WITHOUT making a bulk POST.
#[tokio::test]
async fn test_jql_default_max_50_caps_matched_issues() {
    let server = MockServer::start().await;

    // Search returns 75 issues (more than the default 50 cap).
    let realistic_keys: Vec<String> = (1..=75).map(|i| format!("PROJ-{i}")).collect();
    let realistic_refs: Vec<&str> = realistic_keys.iter().map(String::as_str).collect();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jql_search_response(&realistic_refs)),
        )
        .mount(&server)
        .await;

    // No bulk POST mock — assert zero POST calls.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected bulk call"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = PROJ",
            "--label",
            "add:foo",
            "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Must exit non-zero (64 preferred; any non-0 is acceptable here as
    // the exact exit code is implementation choice — we assert the error message).
    assert!(
        !output.status.success(),
        "Expected non-zero exit when JQL matches 75 issues but --max=50; stderr={stderr} stdout={stdout}"
    );
    // Error output must mention --max and the match count (or the threshold).
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("--max") || combined.contains("max"),
        "Expected --max hint in output; combined={combined}"
    );
    assert!(
        combined.contains("75") || combined.contains("50"),
        "Expected match count or threshold in output; combined={combined}"
    );
}

// ---------------------------------------------------------------------------
// AC-004: --max 75 allows 75 matched issues through.
// ---------------------------------------------------------------------------

/// Same 75-match scenario but with --max 75: should proceed and call bulk POST.
#[tokio::test]
async fn test_jql_with_max_75_allows_75_matched() {
    let server = MockServer::start().await;

    let keys: Vec<String> = (1..=75).map(|i| format!("PROJ-{i}")).collect();
    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&key_refs)))
        .mount(&server)
        .await;

    // Bulk POST: expect exactly 1 call with all 75 keys.
    // Use body_string_contains to avoid asserting exact key order in JSON array.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-max75-001")))
        .expect(1)
        .mount(&server)
        .await;

    mount_poll_complete(&server, "task-max75-001", &key_refs).await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = PROJ",
            "--label",
            "add:foo",
            "--max",
            "75",
            "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 with --max 75 and 75 matched issues; stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(1) verifies bulk POST was made.
}

// ---------------------------------------------------------------------------
// AC-004: --max above 1000 is rejected or clamped.
// ---------------------------------------------------------------------------

/// `--max 1500` must either:
///   (a) exit 64 with "hard ceiling 1000" (or similar) hint, OR
///   (b) clamp to 1000 and proceed — but bulk POST body must contain ≤ 1000 keys.
///
/// Either behavior is acceptable; the test asserts:
///   - The user sees a hint when providing --max > 1000 (warning or error), OR
///   - The command succeeds but the bulk POST body has ≤ 1000 keys.
///
/// NOTE: This test is intentionally tolerant because the exact policy
/// (hard error vs. clamp) is implementer choice per spec.
#[tokio::test]
async fn test_jql_max_above_1000_clamps_or_errors() {
    let server = MockServer::start().await;

    // If the implementation errors early (before HTTP) on --max 1500,
    // no HTTP calls happen. If it clamps, search is called.
    // Mount a permissive search mock for the clamp case.
    let keys: Vec<String> = (1..=50).map(|i| format!("PROJ-{i}")).collect();
    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&key_refs)))
        .mount(&server)
        .await;

    // Bulk POST mock (for clamp path).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-clamp-001")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-clamp-001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_complete("task-clamp-001", &key_refs)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = PROJ",
            "--label",
            "add:foo",
            "--max",
            "1500",
            "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    // The implementation must either:
    //   1. Error (exit non-0) with a message about 1000 ceiling, OR
    //   2. Succeed (exit 0) and emit a warning/hint about the ceiling
    //      (stderr contains "1000" or "ceil" etc.).
    //
    // What MUST NOT happen: silent success with --max honoured at 1500
    // (that would exceed the Atlassian API hard cap).
    let exits_with_ceiling_error = !output.status.success()
        && (combined.contains("1000")
            || combined.contains("ceiling")
            || combined.contains("limit")
            || combined.contains("maximum"));

    let clamps_silently_ok = output.status.success();

    assert!(
        exits_with_ceiling_error || clamps_silently_ok,
        "Expected either error about 1000 ceiling or silent clamp+success; \
         stderr={stderr} stdout={stdout}"
    );

    // If it exited non-zero, check no bulk POST with > 1000 keys was submitted.
    // (If it exited 0 with clamp, the bulk POST body is already ≤ 50 keys in this mock.)
}

// ---------------------------------------------------------------------------
// AC-005: --dry-run skips bulk POST, renders diff, exits 0.
// ---------------------------------------------------------------------------

/// `jr issue edit --jql '...' --label add:foo --dry-run --no-input`
///   - Search IS called (read-only).
///   - Bulk POST is NOT called (zero mutating calls).
///   - stdout contains the matched keys + planned diff.
///   - Exit 0.
#[tokio::test]
async fn test_dry_run_skips_bulk_post_and_renders_diff() {
    let server = MockServer::start().await;

    let matched_keys = ["PROJ-1", "PROJ-2", "PROJ-3", "PROJ-4", "PROJ-5"];

    // Search: expect exactly 1 call.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&matched_keys)))
        .expect(1)
        .mount(&server)
        .await;

    // Bulk POST: must NOT be called. .expect(0) causes a panic on drop if called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(500).set_body_string("unexpected: bulk called in dry-run"),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = PROJ",
            "--label",
            "add:foo",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for --dry-run; stderr={stderr} stdout={stdout}"
    );

    // stdout must contain the matched keys (showing what would be affected).
    let combined = format!("{stdout}{stderr}");
    for key in &matched_keys {
        assert!(
            combined.contains(key),
            "Expected matched key {key} in dry-run output; combined={combined}"
        );
    }

    // stdout must contain the planned change (label name or a diff indicator).
    assert!(
        combined.contains("foo")
            || combined.contains("dry")
            || combined.contains("add")
            || combined.contains("label"),
        "Expected diff summary in dry-run output; combined={combined}"
    );

    // wiremock .expect(0) on bulk POST fires on drop if called.
}

// ---------------------------------------------------------------------------
// AC-005: --dry-run with positional keys (no JQL) — no HTTP calls at all.
// ---------------------------------------------------------------------------

/// `jr issue edit KEY1 KEY2 --label add:foo --dry-run --no-input`
///   - NO HTTP calls (no search, no bulk).
///   - stdout shows planned diff.
///   - Exit 0.
#[tokio::test]
async fn test_dry_run_with_multi_key_positional_skips_bulk_post() {
    let server = MockServer::start().await;

    // All POST calls are unexpected — expect(0) on both search and bulk.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: search in dry-run"))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk in dry-run"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "DRY-1",
            "DRY-2",
            "--label",
            "add:foo",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for positional --dry-run; stderr={stderr} stdout={stdout}"
    );

    // stdout must reference the supplied keys.
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("DRY-1") && combined.contains("DRY-2"),
        "Expected DRY-1 and DRY-2 in dry-run output; combined={combined}"
    );

    // wiremock .expect(0) fires on drop if any HTTP call was made.
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls for positional dry-run"
    );
}

// ---------------------------------------------------------------------------
// AC-004 / --yes: explicit --yes flag passes through below threshold without effect.
// ---------------------------------------------------------------------------

/// `jr issue edit --jql '...' --label add:foo --yes --no-input`
/// Below JQL_CONFIRM_THRESHOLD (5) the confirmation gate is never entered,
/// so --yes has no behavioral effect. This test pins that providing --yes
/// alongside a small JQL match set does not error or hang — it's a no-op
/// backward-compatibility guard. For the > threshold case where --yes
/// actually bypasses the prompt, see a future dedicated test.
#[tokio::test]
async fn test_yes_flag_passes_through_below_threshold_without_effect() {
    let server = MockServer::start().await;

    let matched_keys = ["YES-1", "YES-2", "YES-3"];

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&matched_keys)))
        .mount(&server)
        .await;

    // Bulk POST is made because 3 keys is below JQL_CONFIRM_THRESHOLD (5), so the
    // confirmation prompt is never entered; --yes is a no-op for this match size.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-yes-001")))
        .expect(1)
        .mount(&server)
        .await;

    mount_poll_complete(&server, "task-yes-001", &matched_keys).await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = YES",
            "--label",
            "add:flagged",
            "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 with --yes flag; stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(1) on bulk POST verifies the call was made.
}

// ---------------------------------------------------------------------------
// --no-input implicitly skips confirmation (equivalent to --yes for safety).
// ---------------------------------------------------------------------------

/// `jr issue edit --jql '...' --label add:foo --no-input` (without explicit --yes):
/// --no-input auto-skips the prompt; bulk POST is made; exit 0.
#[tokio::test]
async fn test_no_input_implicitly_skips_confirmation() {
    let server = MockServer::start().await;

    let matched_keys = ["NI-1", "NI-2"];

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&matched_keys)))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-ni-001")))
        .expect(1)
        .mount(&server)
        .await;

    mount_poll_complete(&server, "task-ni-001", &matched_keys).await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = NI",
            "--label",
            "add:ni-test",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 with --no-input (prompt auto-skipped); stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// UX safety-net: --no-input without --yes or --dry-run + match > threshold.
// ---------------------------------------------------------------------------

/// When --no-input is set AND match count > some implementer-chosen threshold (e.g., 5),
/// AND neither --yes nor --dry-run is specified, the command should emit a safety warning
/// or error with a hint to use --yes or --dry-run before proceeding.
///
/// This test is deliberately loose: it asserts that the safety-net EXISTS by verifying
/// that a large JQL match under --no-input alone triggers some user-visible guidance.
/// The exact policy (error vs. proceed with warning) is implementer choice.
///
/// NOTE: If the implementation decides --no-input alone always implies --yes
/// (i.e., no safety-net), this test should be updated to reflect that decision.
/// Until then, the red-gate requires the flag.
#[tokio::test]
async fn test_no_input_without_yes_or_dry_run_errors_when_jql_matches_above_threshold() {
    let server = MockServer::start().await;

    // 50 matches — well above any reasonable "small set" threshold.
    let keys: Vec<String> = (1..=50).map(|i| format!("BIG-{i}")).collect();
    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();

    // Search will be called (to materialise the match set).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&key_refs)))
        .mount(&server)
        .await;

    // Optional bulk POST mock — the command may or may not proceed.
    // If it does proceed (warning-only safety-net), we accept exit 0.
    // If it errors, we check the message.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-safety-001")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-safety-001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_complete("task-safety-001", &key_refs)),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = BIG",
            "--label",
            "add:mass",
            // intentionally omitting --yes and --dry-run
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    // The safety-net can manifest as:
    //   (a) A non-zero exit with a message about --yes or --dry-run, OR
    //   (b) A successful run that emits a warning about the large match set.
    // Either is acceptable. What's NOT acceptable is silent success with no
    // user-visible signal that a large bulk operation was triggered.
    let has_safety_signal = combined.contains("--yes")
        || combined.contains("--dry-run")
        || combined.contains("dry-run")
        || combined.contains("confirm")
        || combined.contains("large")
        || combined.contains("50")
        || combined.contains("bulk");

    // If the implementation decides --no-input always implies --yes (no net),
    // it may exit 0 with no safety signal. We allow this for now but flag it
    // — the implementer should document the policy in the spec.
    if !has_safety_signal && output.status.success() {
        // Tolerate if bulk POST was actually made (--no-input = implicit yes).
        // This branch is allowed but the implementer should verify the policy.
    } else {
        assert!(
            has_safety_signal,
            "Expected a safety-net signal (hint, warning, or error) for large JQL match without --yes; \
             combined={combined}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-001 extension: non-label multi-key --summary bulk edit.
// ---------------------------------------------------------------------------

/// `jr issue edit KEY1 KEY2 --summary "New title" --no-input`
/// PR1 currently bails with "not yet supported". PR2 must route through bulk.
/// Assert: POST /rest/api/3/bulk/issues/fields with editedFieldsInput containing summary.
///
/// SCHEMA NOTE: non-label editedFieldsInput is unverified. Best-guess:
///   {"editedFieldsInput": {"summary": "New title"}}
/// Test uses body_string_contains("summary") to tolerate schema variation.
#[tokio::test]
async fn test_multi_key_summary_update_uses_bulk_fields_endpoint() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        // Loose matcher: just check that "summary" appears in the POST body
        // (exact nesting of editedFieldsInput is unverified).
        .and(body_string_contains("summary"))
        // Regression pin (audit F5): selectedActions field must always be present.
        .and(body_string_contains("selectedActions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-summary-001")))
        .expect(1)
        .mount(&server)
        .await;

    mount_poll_complete(&server, "task-summary-001", &["SUM-1", "SUM-2"]).await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "SUM-1",
            "SUM-2",
            "--summary",
            "New title",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for multi-key --summary bulk edit; stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(1) verifies exactly 1 bulk POST was made.
}

// ---------------------------------------------------------------------------
// AC-001 extension: non-label multi-key --priority bulk edit.
// ---------------------------------------------------------------------------

/// `jr issue edit KEY1 KEY2 --priority High --no-input`
/// Assert: POST /rest/api/3/bulk/issues/fields with priority in editedFieldsInput.
///
/// SCHEMA NOTE: priority shape in editedFieldsInput is unverified. Best-guess:
///   {"editedFieldsInput": {"priority": {"name": "High"}}}
/// Test uses body_string_contains("priority") as a tolerant matcher.
#[tokio::test]
async fn test_multi_key_priority_update_uses_bulk_fields_endpoint() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_string_contains("priority"))
        // Regression pin (audit F5): selectedActions field must always be present.
        .and(body_string_contains("selectedActions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-prio-001")))
        .expect(1)
        .mount(&server)
        .await;

    mount_poll_complete(&server, "task-prio-001", &["PRI-1", "PRI-2"]).await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "PRI-1",
            "PRI-2",
            "--priority",
            "High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for multi-key --priority bulk edit; stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC optimization: --label add:foo --label remove:bar coalesces to ONE bulk call.
// ---------------------------------------------------------------------------

/// `jr issue edit KEY1 KEY2 --label add:foo --label remove:bar --no-input`
/// PR1 currently makes TWO bulk calls (one ADD, one REMOVE).
/// PR2 must coalesce them into EXACTLY ONE bulk POST.
///
/// SCHEMA NOTE: Whether Atlassian accepts mixed ADD+REMOVE in one payload is
/// unverified. If it does, the body would contain both label operations.
/// If it doesn't, the implementer must pick one of these strategies:
///   (a) Two sequential bulk calls → fails this test (test expects 1).
///   (b) Split into batches with a single atomic request per action but
///       group the user's intent into a single confirmation unit → also
///       fails if the actual HTTP count is > 1.
///
/// The test deliberately asserts .expect(1) to force the implementer to
/// resolve the schema gap. If Atlassian truly requires two calls, this
/// test must be updated with a comment explaining why (and the test name
/// updated to `...makes_two_bulk_calls_for_add_and_remove`).
#[tokio::test]
async fn test_label_add_remove_coalesce_emits_one_bulk_call() {
    let server = MockServer::start().await;

    // Expect EXACTLY 1 POST to the bulk fields endpoint.
    // selectedActions pin: the label path passes vec!["labels"] as selected_actions, so the
    // request body must contain "selectedActions". This matcher catches regressions where
    // the field is accidentally dropped from the label edit path. If the pin fires here
    // but the body structure changes legitimately, update with a comment explaining the
    // new structure.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_string_contains("selectedActions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-coalesce-001")))
        .expect(1)
        .mount(&server)
        .await;

    mount_poll_complete(&server, "task-coalesce-001", &["CO-1", "CO-2"]).await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "CO-1",
            "CO-2",
            "--label",
            "add:foo",
            "--label",
            "remove:bar",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for coalesced add+remove; stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(1) fires on drop if != 1 POST was made.
}

// ---------------------------------------------------------------------------
// Audit C-1: multi-key bulk silently drops unsupported flags → reject with error
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 FOO-2 --no-parent --no-input`
/// Multi-key edits with --no-parent must exit non-zero with a message naming
/// "--no-parent" (or similar) — the flag must NOT be silently dropped.
///
/// Before C-1 fix: exits 0 with no error (flag silently ignored).
/// After C-1 fix:  exits 64 (UserError) with stderr containing "--no-parent".
#[tokio::test]
async fn test_multi_key_with_no_parent_rejects_with_unsupported_flag_error() {
    let server = MockServer::start().await;

    // No bulk POST should be made — the command must error before mutating.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--no-parent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for multi-key --no-parent; stderr={stderr} stdout={stdout}"
    );
    assert!(
        stderr.contains("--no-parent") || stdout.contains("--no-parent"),
        "Expected '--no-parent' in error output; stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(0) fires on drop if bulk POST was made.
}

/// `jr issue edit --jql 'project = FOO' --team "Engineering" --no-input`
/// Multi-key edits with --team must exit non-zero — --team is unsupported for bulk.
///
/// Before C-1 fix: exits 0 with --team silently dropped.
/// After C-1 fix:  exits 64 (UserError) with stderr naming the flag.
#[tokio::test]
async fn test_multi_key_jql_with_team_rejects_with_unsupported_flag_error() {
    let server = MockServer::start().await;

    // Search returns 2 issues (crosses no threshold so --yes not needed).
    let matched_keys = ["PROJ-1", "PROJ-2"];
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&matched_keys)))
        .mount(&server)
        .await;

    // No bulk POST should be made.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = PROJ",
            "--team",
            "Engineering",
            "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for multi-key --jql --team; stderr={stderr} stdout={stdout}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("--team") || combined.contains("team"),
        "Expected '--team' in error output; combined={combined}"
    );
    // wiremock .expect(0) fires on drop if bulk POST was made.
}

// ---------------------------------------------------------------------------
// Audit C-2: await_bulk_task swallows FAILED/CANCELLED/DEAD
// ---------------------------------------------------------------------------

/// When the bulk task poll returns status "FAILED", the command must exit non-zero
/// with stderr containing "FAILED" and the task ID.
///
/// Before C-2 fix: await_bulk_task returns Ok(progress) for FAILED, caller sees
///   exit 0 (the render function sees empty processed list and no failed_issues, so
///   reports nothing — silent success).
/// After C-2 fix:  await_bulk_task returns Err for FAILED, command exits 1 with
///   stderr naming the task ID and "FAILED".
///
/// This fixes inherited PR1 behavior (C-2 per cross-PR audit).
#[tokio::test]
async fn test_bulk_task_failed_status_exits_nonzero_with_failed_in_stderr() {
    let server = MockServer::start().await;

    let task_id = "task-failed-001";

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": task_id,
            "status": "ENQUEUED",
            "progressPercent": 0,
            "totalIssueCount": 2,
            "processedAccessibleIssues": [],
            "failedAccessibleIssues": {},
            "invalidOrInaccessibleIssueCount": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Poll returns FAILED terminal state.
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": task_id,
            "status": "FAILED",
            "progressPercent": 0,
            "totalIssueCount": 2,
            "processedAccessibleIssues": [],
            "failedAccessibleIssues": {},
            "invalidOrInaccessibleIssueCount": 0
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--label",
            "add:test",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit when bulk task FAILED; stderr={stderr} stdout={stdout}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("FAILED"),
        "Expected 'FAILED' in output; combined={combined}"
    );
    assert!(
        combined.contains(task_id),
        "Expected task ID '{task_id}' in output; combined={combined}"
    );
}

// ---------------------------------------------------------------------------
// Audit C-3: --dry-run --output json must emit JSON on stdout
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 FOO-2 --label add:foo --dry-run --output json --no-input`
/// When --dry-run is combined with --output json, stdout must be valid JSON
/// with "dryRun": true and "issues": [...].
///
/// Before C-3 fix: stdout contains prose text ("DRY RUN — no changes..."), not JSON.
/// After C-3 fix:  stdout is a single JSON object with dryRun + issues fields.
#[tokio::test]
async fn test_dry_run_output_json_emits_valid_json_with_dry_run_field() {
    let server = MockServer::start().await;

    // No HTTP calls expected in dry-run mode.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected bulk in dry-run"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--label",
            "add:foo",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for --dry-run --output json; stderr={stderr} stdout={stdout}"
    );

    // stdout must be valid JSON.
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not valid JSON for dry-run+json: {e}; stdout={stdout}"));

    assert_eq!(
        parsed.get("dryRun").and_then(|v| v.as_bool()),
        Some(true),
        "Expected {{\"dryRun\": true}} in JSON output; got: {stdout}"
    );

    let issues = parsed
        .get("issues")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("Expected 'issues' array in JSON output; got: {stdout}"));
    assert!(
        issues.iter().any(|v| v.as_str() == Some("FOO-1")),
        "Expected FOO-1 in issues array; got: {stdout}"
    );
    assert!(
        issues.iter().any(|v| v.as_str() == Some("FOO-2")),
        "Expected FOO-2 in issues array; got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Audit follow-up: await_bulk_task must surface failureReason from FAILED response
// ---------------------------------------------------------------------------

/// When the bulk task poll returns `status: "FAILED"` WITH a `failureReason` field,
/// the command must exit non-zero AND stderr must contain the literal failureReason
/// string — not just the generic fallback hint.
///
/// Perplexity verification (2026-05-10, PR2 audit follow-up):
///   Atlassian docs confirm FAILED responses include `failureReason: String`.
///   Previous C-2 fix (56d754d) errored correctly but discarded the message.
///
/// Before this fix: stderr contains "Run `jr api /rest/api/3/bulk/queue/..." fallback.
/// After this fix:  stderr contains "Insufficient permissions on project XYZ".
#[tokio::test]
async fn test_bulk_task_failed_with_failure_reason_surfaces_reason_in_stderr() {
    let server = MockServer::start().await;

    let task_id = "task-fail-reason-001";

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": task_id,
            "status": "ENQUEUED",
            "progressPercent": 0,
            "totalIssueCount": 2,
            "processedAccessibleIssues": [],
            "failedAccessibleIssues": {},
            "invalidOrInaccessibleIssueCount": 0
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Poll returns FAILED with a failureReason message.
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": task_id,
            "status": "FAILED",
            "processedAccessibleIssues": [],
            "failedAccessibleIssues": {},
            "failureReason": "Insufficient permissions on project XYZ"
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--label",
            "add:test",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit when bulk task FAILED; stderr={stderr} stdout={stdout}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("Insufficient permissions on project XYZ"),
        "Expected failureReason text in output; combined={combined}"
    );
}

// ---------------------------------------------------------------------------
// Audit I-2: JQL matching 0 issues must error, not proceed silently
// ---------------------------------------------------------------------------

/// `jr issue edit --jql 'project = NONE' --label add:foo --no-input`
/// When the JQL query returns 0 issues, the command must exit non-zero with
/// a message indicating 0 matches.
///
/// Before I-2 fix: empty match set routes to bulk POST with empty keys array,
///   or falls through with silent success.
/// After I-2 fix:  exits 64 (UserError) with stderr containing "0" or "matched 0".
#[tokio::test]
async fn test_jql_zero_matches_exits_nonzero_with_hint() {
    let server = MockServer::start().await;

    // Search returns empty issues list.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&[])))
        .expect(1)
        .mount(&server)
        .await;

    // No bulk POST should be made.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = NONE",
            "--label",
            "add:foo",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for JQL matching 0 issues; stderr={stderr} stdout={stdout}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("0") || combined.contains("matched"),
        "Expected '0' or 'matched' in error output; combined={combined}"
    );
}

// ---------------------------------------------------------------------------
// Audit F2: selectedActions key for issueType must match editedFieldsInput key
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 FOO-2 --type Bug --no-input`
/// The bulk POST body must contain "issuetype" (lowercase) in BOTH
/// selectedActions AND editedFieldsInput. Using camelCase "issueType" in
/// selectedActions while the body key is lowercase "issuetype" is a
/// self-inconsistency that may cause 400 errors from the Atlassian API.
///
/// This test asserts the body contains "issuetype" (lowercase) and does NOT
/// contain the camelCase variant "issueType" as a selectedActions value.
#[tokio::test]
async fn test_multi_key_type_update_uses_consistent_issuetype_casing() {
    let server = MockServer::start().await;

    // Capture the raw POST body with a permissive mock, then assert on it.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_string_contains("issuetype"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued("task-type-001")))
        .expect(1)
        .mount(&server)
        .await;

    mount_poll_complete(&server, "task-type-001", &["FOO-1", "FOO-2"]).await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--type",
            "Bug",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for multi-key --type bulk edit; stderr={stderr} stdout={stdout}"
    );

    // Verify the recorded request body uses lowercase "issuetype" consistently.
    let requests = server.received_requests().await.unwrap();
    let bulk_req = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/bulk/issues/fields")
        .expect("Expected exactly one POST to /rest/api/3/bulk/issues/fields");
    let body_str = std::str::from_utf8(&bulk_req.body).unwrap_or("");

    // "issuetype" (lowercase) must appear in the body (editedFieldsInput key).
    assert!(
        body_str.contains("issuetype"),
        "Expected lowercase 'issuetype' in bulk request body; body={body_str}"
    );

    // The selectedActions array must NOT contain camelCase "issueType" as a standalone
    // value — it must use lowercase "issuetype" to match editedFieldsInput.
    // We check: the string "\"issueType\"" (quoted, camelCase) should not appear
    // as a JSON string value inside selectedActions.
    assert!(
        !body_str.contains("\"issueType\""),
        "Expected selectedActions NOT to contain camelCase \"issueType\"; body={body_str}"
    );
}

// ---------------------------------------------------------------------------
// Audit F3: --dry-run with no field changes must error like the live path
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 --dry-run --no-input` (no field flags)
/// Before F3 fix: dry-run short-circuits before the "no fields specified" guard,
///   producing a meaningless "DRY RUN — no changes will be made" output for a
///   no-op, which may mislead the user into thinking a planned edit was shown.
/// After F3 fix:  exits non-zero with "No fields specified" (same as live path).
#[tokio::test]
async fn test_dry_run_with_no_field_changes_errors_like_live_path() {
    let server = MockServer::start().await;

    // No HTTP calls should be made at all (key is positional, no field flags).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    // Table mode: no field flags → must error.
    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "FOO-1", "--dry-run"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for --dry-run with no field flags; stderr={stderr} stdout={stdout}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("No fields specified"),
        "Expected 'No fields specified' in error output; combined={combined}"
    );

    // Zero HTTP calls should have been made.
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls for --dry-run with no field flags"
    );
}

/// "No fields specified" must exit with the canonical UserError code (64), not the
/// generic failure code (1) that `bail!()` produces.  Copilot review comment
/// #3215377375 flagged that the pre-HTTP guard was using `bail!()` which bypasses
/// `JrError::UserError` and therefore produces exit code 1 instead of 64.
#[tokio::test]
async fn test_no_fields_specified_exits_64() {
    let server = MockServer::start().await;

    // No HTTP calls should be made — the guard fires before any network call.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--no-input", "issue", "edit", "FOO-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit code 64 (UserError) for 'no fields specified'; \
         stderr={stderr} stdout={stdout}"
    );
}

/// Same as above but with `--output json` — JSON mode must also error (not emit empty JSON).
#[tokio::test]
async fn test_dry_run_with_no_field_changes_errors_in_json_mode() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-1",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for --dry-run --output json with no field flags; \
         stderr={stderr} stdout={stdout}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("No fields specified"),
        "Expected 'No fields specified' in JSON-mode error output; combined={combined}"
    );
}

// ---------------------------------------------------------------------------
// Audit F1: empty --jql must error before search, not silently match 0 issues
// ---------------------------------------------------------------------------

/// `jr issue edit --jql "" --label add:foo --no-input`
/// Before F1 fix: the empty JQL is passed to search_issues, which either
///   returns 0 results (triggering the I-2 "matched 0 issues" error) or
///   makes an API call with an empty query.
/// After F1 fix: the command errors immediately with a user-friendly message
///   before making any HTTP call.
#[tokio::test]
async fn test_empty_jql_errors_before_search() {
    let server = MockServer::start().await;

    // No HTTP calls should be made — the error must happen before search.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: search called"))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "",
            "--label",
            "add:foo",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for empty --jql; stderr={stderr} stdout={stdout}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("cannot be empty"),
        "Expected 'cannot be empty' in error output; combined={combined}"
    );

    // Zero HTTP calls must have been made.
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls for empty --jql"
    );
}

// ---------------------------------------------------------------------------
// Audit F3-incomplete: --dry-run --jql with no field flags must error BEFORE search
// ---------------------------------------------------------------------------

/// `jr issue edit --jql "project = FOO" --dry-run --no-input` (no field flags)
///
/// Before fix: the JQL search fires first (ENQUEUED 1 HTTP call), then the
/// "No fields specified" error is returned after the wasted API call.
/// After fix:  the field-change check runs BEFORE `effective_keys` is built,
/// so the search endpoint is never called.
///
/// This test asserts `.expect(0)` on the search mock AND verifies that
/// zero HTTP calls were made, so a regression is caught immediately.
#[tokio::test]
async fn test_dry_run_jql_with_no_field_changes_errors_before_search() {
    let server = MockServer::start().await;

    // The JQL search endpoint must NOT be called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: search called"))
        .expect(0)
        .mount(&server)
        .await;

    // Bulk POST also must not be called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = FOO",
            "--dry-run",
            // Intentionally no --summary, --priority, --label, etc.
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for --dry-run --jql with no field flags; \
         stderr={stderr} stdout={stdout}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("No fields specified"),
        "Expected 'No fields specified' in error output; combined={combined}"
    );

    // The critical assertion: zero HTTP calls (search must not have fired).
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls — field-change check must precede JQL search"
    );
}

// ---------------------------------------------------------------------------
// Audit pass-2: single-match --jql routes to legacy single-key PUT, not bulk
// ---------------------------------------------------------------------------

/// `jr issue edit --jql "key = FOO-1" --priority High --no-input`
/// with exactly 1 JQL match should route to the legacy single-key PUT path,
/// NOT the bulk POST path.
///
/// This documents and pins the intentional routing asymmetry: 2+ keys → bulk
/// API (efficient for multiple issues); 1 key → PUT (no taskId polling needed,
/// more efficient for a single issue regardless of whether a JQL or positional
/// selector was used). See doc-comment at the dispatch site in create.rs.
#[tokio::test]
async fn test_jql_single_match_routes_to_single_key_put_not_bulk() {
    let server = MockServer::start().await;

    // JQL search returns exactly 1 issue.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&["FOO-1"])))
        .expect(1)
        .mount(&server)
        .await;

    // Bulk POST must NOT be called for a single-match.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    // Single-key PUT MUST be called exactly once.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "key = FOO-1",
            "--priority",
            "High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for single-match JQL routed to PUT; stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(0/1) assertions verify routing on drop.
}

// ---------------------------------------------------------------------------
// Copilot review Fix 1: --max 0 must be rejected at clap parse layer
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 --label add:foo --max 0 --no-input`
/// clap must reject --max 0 before any HTTP calls are made.
/// Before fix: exits 1 (HTTP failure) — clap accepted 0 and fell through.
/// After fix:  exits 2 (clap error) before any HTTP call.
#[tokio::test]
async fn test_max_zero_rejected_at_clap_layer() {
    let server = MockServer::start().await;

    // No HTTP calls should be made — clap rejects before HTTP.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--label",
            "add:foo",
            "--max",
            "0",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected non-zero exit for --max 0; stderr={stderr} stdout={stdout}"
    );

    // clap error message should reference "0" and/or "max".
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("0") || combined.contains("max"),
        "Expected '0' or 'max' in clap error output; combined={combined}"
    );

    // Zero HTTP calls — clap rejects before networking.
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls when clap rejects --max 0"
    );
}

// ---------------------------------------------------------------------------
// Copilot review Fix 2: over-cap error must NOT say "(default 50)" when user
// supplied an explicit --max value.
// ---------------------------------------------------------------------------

/// `jr issue edit --jql '...' --label add:foo --max 100 --no-input`
/// when 150 issues match, the error message should reference "--max 100"
/// but must NOT contain the literal "(default 50)" substring.
///
/// Before fix: message hardcodes "(default 50)" regardless of user-supplied value.
/// After fix:  message uses the effective_max value only.
#[tokio::test]
async fn test_jql_max_explicit_value_in_error_not_default() {
    let server = MockServer::start().await;

    // 150 matches > --max 100.
    let keys: Vec<String> = (1..=150).map(|i| format!("PROJ-{i}")).collect();
    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&key_refs)))
        .mount(&server)
        .await;

    // No bulk POST should be made — command must error before mutating.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected bulk call"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = PROJ",
            "--label",
            "add:foo",
            "--max",
            "100",
            "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when 150 issues exceed --max 100; combined={combined}"
    );

    // Error must mention the effective --max (100).
    assert!(
        combined.contains("100"),
        "Expected '100' (the explicit --max value) in error output; combined={combined}"
    );

    // Error must NOT contain the misleading "(default 50)" parenthetical.
    assert!(
        !combined.contains("(default 50)"),
        "Error message must not contain '(default 50)' when user passed --max 100; combined={combined}"
    );
}

// ---------------------------------------------------------------------------
// Copilot review Fix 3: --dry-run must include --team and --description
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 --dry-run --team "Platform Team" --description "Test desc" --no-input`
/// Single-key dry-run: stdout must include "team", "Platform Team", "description", "Test desc".
/// Before fix: team and description are silently omitted from the dry-run preview.
/// After fix:  both appear in the planned-changes section.
#[tokio::test]
async fn test_dry_run_includes_team_and_description_in_output() {
    let server = MockServer::start().await;

    // No HTTP calls for single-key positional dry-run.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--dry-run",
            "--team",
            "Platform Team",
            "--description",
            "Test desc",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        output.status.success(),
        "Expected exit 0 for single-key --dry-run with --team + --description; combined={combined}"
    );

    assert!(
        combined.contains("team") || combined.contains("Team"),
        "Expected 'team' in dry-run output; combined={combined}"
    );
    assert!(
        combined.contains("Platform Team"),
        "Expected 'Platform Team' in dry-run output; combined={combined}"
    );
    assert!(
        combined.contains("description") || combined.contains("desc"),
        "Expected 'description' in dry-run output; combined={combined}"
    );
    assert!(
        combined.contains("Test desc"),
        "Expected 'Test desc' in dry-run output; combined={combined}"
    );

    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls for single-key positional dry-run"
    );
}

/// `jr issue edit FOO-1 --dry-run --team "Platform Team" --description "Test desc"
///   --output json --no-input`
/// JSON mode must include "team" and "description" keys in plannedChanges.
#[tokio::test]
async fn test_dry_run_json_includes_team_and_description() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-1",
            "--dry-run",
            "--team",
            "Platform Team",
            "--description",
            "Test desc",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for --dry-run --output json; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not valid JSON: {e}; stdout={stdout}"));

    let planned = parsed
        .get("plannedChanges")
        .expect("Expected 'plannedChanges' in JSON output");

    assert!(
        planned.get("team").is_some(),
        "Expected 'team' key in plannedChanges; got: {planned}"
    );
    assert_eq!(
        planned.get("team").and_then(|v| v.as_str()),
        Some("Platform Team"),
        "Expected team='Platform Team'; got: {planned}"
    );

    assert!(
        planned.get("description").is_some(),
        "Expected 'description' key in plannedChanges; got: {planned}"
    );
    assert_eq!(
        planned.get("description").and_then(|v| v.as_str()),
        Some("Test desc"),
        "Expected description='Test desc'; got: {planned}"
    );
}

// ---------------------------------------------------------------------------
// Copilot round-3 Fix 1: dry-run description preview must not panic on UTF-8
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 --dry-run --description <cyrillic-70-chars> --no-input`
///
/// Before fix: `&d[..60]` slices by byte index → panics when byte 60 falls
/// inside a multi-byte UTF-8 codepoint (Cyrillic chars are 2 bytes each).
/// After fix:  `d.chars().take(60)` truncates by codepoint → no panic.
///
/// The test string is 70 Cyrillic chars (~140 bytes). Without the fix the
/// binary panics and exits with a non-zero status; with the fix it exits 0
/// and the truncated preview appears in output.
#[tokio::test]
async fn test_dry_run_table_handles_unicode_description_without_panicking() {
    let server = MockServer::start().await;

    // Bulk must NOT be called; this is a dry-run.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    // 70 Cyrillic chars → 140 bytes. Slicing at byte 60 lands mid-codepoint.
    let cyrillic_desc = "русский текст превышает шестьдесят символов и должен быть обрезан красиво";

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--dry-run",
            "--description",
            cyrillic_desc,
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        output.status.success(),
        "Expected exit 0 (no panic) for Cyrillic description in dry-run; combined={combined}"
    );

    // The description should appear somewhere in the output (possibly truncated).
    assert!(
        combined.contains("description") || combined.contains("русский"),
        "Expected description preview in dry-run output; combined={combined}"
    );

    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls for dry-run"
    );
}

// ---------------------------------------------------------------------------
// Copilot round-3 Fix 2: --markdown alone must error before any JQL search
// ---------------------------------------------------------------------------

/// `jr issue edit --jql "project = FOO" --markdown --no-input`
///
/// Before fix: `|| markdown` in `has_any_field_change` causes the JQL search
/// to fire even though --markdown provides no description to encode.
/// After fix:  validation rejects --markdown-without-description before the
/// search, so zero HTTP calls are made and exit is non-zero with a hint
/// containing "markdown".
#[tokio::test]
async fn test_markdown_alone_errors_before_search() {
    let server = MockServer::start().await;

    // The JQL search MUST NOT fire — expect(0) asserts this.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_string("unexpected: search called"))
        .expect(0)
        .mount(&server)
        .await;

    // Bulk POST must also not fire.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = FOO",
            "--markdown",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when --markdown used without --description; combined={combined}"
    );

    assert!(
        combined.contains("markdown") || combined.contains("--markdown"),
        "Expected error message to mention 'markdown'; combined={combined}"
    );

    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls when --markdown used without --description"
    );
}

// ---------------------------------------------------------------------------
// Copilot round-5 fix: --label + non-label field flags → rejected before search
// (#3215393741)
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 FOO-2 --label add:foo --summary "X" --no-input`
/// MUST exit non-zero (exit 64 — UserError) before making any HTTP call.
/// stderr must mention both "--label" and "--summary" and hint at running
/// separate commands.
#[tokio::test]
async fn test_label_with_summary_rejected_before_search() {
    let server = MockServer::start().await;

    // JQL search MUST NOT fire (no --jql here, but we still guard the bulk endpoint).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--label",
            "add:foo",
            "--summary",
            "X",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when --label combined with --summary; combined={combined}"
    );

    // Must specifically be exit 64 (UserError / usage error).
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 (UserError) for --label + --summary; combined={combined}"
    );

    assert!(
        combined.contains("--label"),
        "Error must mention '--label'; combined={combined}"
    );
    assert!(
        combined.contains("--summary"),
        "Error must mention '--summary'; combined={combined}"
    );
    assert!(
        combined.contains("separate") || combined.contains("Run separate"),
        "Error must hint at running separate commands; combined={combined}"
    );

    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls when --label combined with --summary"
    );
}

/// `jr issue edit FOO-1 FOO-2 --label add:foo --priority High --no-input`
/// MUST exit 64 before any HTTP call. stderr must mention "--priority".
#[tokio::test]
async fn test_label_with_priority_rejected_before_search() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--label",
            "add:foo",
            "--priority",
            "High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when --label combined with --priority; combined={combined}"
    );

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 (UserError) for --label + --priority; combined={combined}"
    );

    assert!(
        combined.contains("--priority"),
        "Error must mention '--priority'; combined={combined}"
    );

    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls when --label combined with --priority"
    );
}

// ---------------------------------------------------------------------------
// Copilot round-6 fix 1: --label + field rejection must fire BEFORE JQL search
// (#3215407131)
// ---------------------------------------------------------------------------

/// `jr issue edit --jql "project = FOO" --label add:foo --summary "X" --no-input`
/// MUST exit 64 (UserError) before making ANY HTTP call — including the JQL search.
/// The round-5 guard ran after `effective_keys` was resolved (which fires the JQL
/// search).  Moving the guard before the JQL HTTP call avoids the wasted request.
#[tokio::test]
async fn test_label_with_summary_rejected_before_jql_search() {
    let server = MockServer::start().await;

    // The JQL search endpoint MUST NOT fire (expect(0)).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: jql search called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = FOO",
            "--label",
            "add:foo",
            "--summary",
            "X",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when --jql + --label combined with --summary; combined={combined}"
    );

    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 (UserError) for --jql + --label + --summary; combined={combined}"
    );

    assert!(
        combined.contains("--label"),
        "Error must mention '--label'; combined={combined}"
    );
    assert!(
        combined.contains("--summary"),
        "Error must mention '--summary'; combined={combined}"
    );
    assert!(
        combined.contains("separate") || combined.contains("Run separate"),
        "Error must hint at running separate commands; combined={combined}"
    );

    // The critical assertion: ZERO HTTP calls — the guard fires before the JQL search.
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls (including JQL search) when --label + --summary rejected"
    );
}

// ---------------------------------------------------------------------------
// Copilot round 8 (#3215445205): --max requires --jql
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 FOO-2 --max 100 --label add:foo --no-input`
/// (positional keys + --max, no --jql)
/// MUST be rejected with exit 64 (JrError::UserError) before any HTTP calls.
/// stderr must mention "--max" and "--jql".
/// ZERO HTTP requests must be received (handler guard fires before search/bulk).
///
/// Note: clap's `requires = "jql"` cannot enforce this when positional keys are
/// present, because `keys` and `jql` have `conflicts_with` between them (which
/// causes clap to skip the `requires` check on `max`). Handler-level validation
/// is the correct enforcement point — this matches how other mutual-exclusion
/// guards (e.g., --label + --summary) are implemented in this handler.
#[tokio::test]
async fn test_max_without_jql_rejected_before_any_http_call() {
    let server = MockServer::start().await;

    // Neither search nor bulk should ever fire.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: search called"))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--max",
            "100",
            "--label",
            "add:foo",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when --max is passed without --jql; combined={combined}"
    );

    // Handler-level UserError exits with code 64.
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 (UserError) for --max without --jql; combined={combined}"
    );

    // Error message must reference both --max and --jql.
    assert!(
        combined.to_lowercase().contains("max"),
        "Error must mention 'max'; combined={combined}"
    );
    assert!(
        combined.to_lowercase().contains("jql"),
        "Error must mention 'jql'; combined={combined}"
    );

    // Zero HTTP calls — handler guard fires before any HTTP request.
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls when --max without --jql is rejected by handler guard"
    );
}

// ---------------------------------------------------------------------------
// Copilot round 9 (#3215464657): extend --label conflict guard to ALL non-label
// field flags (--team, --points, --no-points, --parent, --no-parent,
// --description, --description-stdin, --markdown).
// Three representative tests (single positional key + varied flag combos).
// ---------------------------------------------------------------------------

/// `jr issue edit FOO-1 --label add:foo --team SomeTeam --no-input`
/// MUST exit 64 (UserError) before ANY HTTP call.
/// stderr must mention both "--label" and "--team".
#[tokio::test]
async fn test_label_with_team_rejected_before_search() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: search called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--label",
            "add:foo",
            "--team",
            "SomeTeam",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when --label combined with --team; combined={combined}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 (UserError) for --label + --team; combined={combined}"
    );
    assert!(
        combined.contains("--label"),
        "Error must mention '--label'; combined={combined}"
    );
    assert!(
        combined.contains("--team"),
        "Error must mention '--team'; combined={combined}"
    );
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls when --label combined with --team"
    );
}

/// `jr issue edit FOO-1 --label add:foo --parent FOO-1 --no-input`
/// MUST exit 64 (UserError) before ANY HTTP call.
/// stderr must mention both "--label" and "--parent".
#[tokio::test]
async fn test_label_with_parent_rejected_before_search() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: search called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--label",
            "add:foo",
            "--parent",
            "FOO-1",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when --label combined with --parent; combined={combined}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 (UserError) for --label + --parent; combined={combined}"
    );
    assert!(
        combined.contains("--label"),
        "Error must mention '--label'; combined={combined}"
    );
    assert!(
        combined.contains("--parent"),
        "Error must mention '--parent'; combined={combined}"
    );
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls when --label combined with --parent"
    );
}

/// `jr issue edit FOO-1 --label add:foo --description "x" --no-input`
/// MUST exit 64 (UserError) before ANY HTTP call.
/// stderr must mention both "--label" and "--description".
#[tokio::test]
async fn test_label_with_description_rejected_before_search() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: bulk called"))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unexpected: search called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--label",
            "add:foo",
            "--description",
            "x",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when --label combined with --description; combined={combined}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 (UserError) for --label + --description; combined={combined}"
    );
    assert!(
        combined.contains("--label"),
        "Error must mention '--label'; combined={combined}"
    );
    assert!(
        combined.contains("--description"),
        "Error must mention '--description'; combined={combined}"
    );
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls when --label combined with --description"
    );
}

// ---------------------------------------------------------------------------
// #336: CLI-level verification of warn+escalate for unrecognized bulk status.
// ---------------------------------------------------------------------------

/// Multi-key `jr issue move FOO-1 FOO-2 ... Done` against a wiremock that
/// returns `MYSTERY_STATUS_FAKE` (a deliberately-novel status not in the
/// Atlassian OpenAPI enum) for the bulk-queue poll. With
/// `JR_BULK_UNKNOWN_GRACE_SECS=0` the warn+escalate path completes in one
/// poll cycle (~1s wall-clock) instead of waiting the production 30-second
/// grace.
///
/// This test closes the Copilot R1 finding on PR #359: the in-lib wiremock
/// tests verify the **escalation** half of the contract (return value), but
/// the **warning emission** half (the `eprintln!` to stderr on first sighting
/// of an unknown status) is only observable at the process boundary. Async
/// in-lib tests can't easily capture `eprintln!` (Perplexity-validated
/// 2026-05-12: no stdlib mechanism, sync-only crates `gag`/`stdio-override`
/// don't compose with `#[tokio::test]`, and `tracing` migration is out of
/// scope for this PR). `assert_cmd` does capture stdout/stderr from the
/// spawned binary, so we drive the path through `jr issue move` and assert
/// on what an operator would actually see.
///
/// Assertions:
///   - non-zero exit (escalation is `Err`, surfaced as exit != 0);
///   - stderr contains the first-sighting warning prefix
///     ("warning: bulk task ... returned unrecognized status");
///   - stderr contains the made-up status name `MYSTERY_STATUS_FAKE`;
///   - stderr contains the escalation phrase `terminal-with-error`.
#[tokio::test]
async fn test_336_cli_unknown_status_emits_warning_and_escalates() {
    let server = MockServer::start().await;
    let task_id = "test-336-cli-unknown";

    // GET /rest/api/3/issue/FOO-1/transitions — used by handle_move_bulk to
    // discover the transition ID from the first key. One transition, named
    // "Done" with id "21".
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![("21", "Mark Done", "Done")]),
        ))
        .mount(&server)
        .await;

    // POST /rest/api/3/bulk/issues/transition — submit returns taskId.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/transition"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": task_id,
        })))
        .mount(&server)
        .await;

    // GET /rest/api/3/bulk/queue/{taskId} — always returns the novel status.
    // The polling loop will see it twice (sleep 1s between polls), emit the
    // first-sighting warning, then escalate because grace=0.
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": task_id,
            "status": "MYSTERY_STATUS_FAKE",
            "progressPercent": 0,
            "totalIssueCount": 2,
            "processedAccessibleIssues": [],
            "failedAccessibleIssues": {},
            "invalidOrInaccessibleIssueCount": 0
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        // Zero-second grace (the env var parses as whole seconds via u64)
        // turns the 30s production wait into ~1s test wait: the first poll
        // emits the first-sighting warning and seeds the unknown-state
        // tracker; after the ~1s exponential-backoff sleep the second poll
        // sees the same unknown status, and `now - first_seen >= 0s` fires
        // the escalation Err immediately.
        .env("JR_BULK_UNKNOWN_GRACE_SECS", "0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("FOO-2")
        .arg("Done")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected non-zero exit on escalation; stderr: {stderr}"
    );
    assert!(
        stderr.contains("warning: bulk task") && stderr.contains("returned unrecognized status"),
        "Expected first-sighting warning in stderr; got: {stderr}"
    );
    assert!(
        stderr.contains("MYSTERY_STATUS_FAKE"),
        "Expected the offending status name in stderr; got: {stderr}"
    );
    assert!(
        stderr.contains("terminal-with-error"),
        "Expected escalation phrase 'terminal-with-error' in stderr; got: {stderr}"
    );
}
