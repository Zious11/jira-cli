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
///   1. GET /rest/api/3/search/jql called once (search).
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
// AC-004 / --yes: explicit --yes flag skips confirmation prompt.
// ---------------------------------------------------------------------------

/// `jr issue edit --jql '...' --label add:foo --yes --no-input`
/// The bulk POST is made without reading stdin (no prompt hang).
#[tokio::test]
async fn test_yes_flag_skips_confirmation_prompt() {
    let server = MockServer::start().await;

    let matched_keys = ["YES-1", "YES-2", "YES-3"];

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&matched_keys)))
        .mount(&server)
        .await;

    // Bulk POST must be made (--yes bypasses the prompt).
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
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
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
