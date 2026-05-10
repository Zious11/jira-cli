// SCHEMA NOTES — Atlassian Bulk API (issue #110 PR1, locked 2026-05-09)
//
// Sources: OpenAPI spec (developer.atlassian.com/cloud/jira/platform/swagger-v3.v3.json)
// fetched 2026-05-09. HTML doc pages truncated every attempt; schema extracted from
// the machine-readable OpenAPI JSON instead.
//
// === POST /rest/api/3/bulk/issues/fields (bulk edit fields) ===
//
// Schema name: BulkEditSubmitInput (not fully retrievable from OpenAPI JSON due to
// truncation at CustomFieldContextDefaultValue definitions). The following fields are
// CONFIRMED from research report + partial OpenAPI extract:
//
//   CONFIRMED:
//     selectedIssueIdsOrKeys  string[]  required  — list of up to 1,000 issue IDs or keys
//   BEST-GUESS (unverified, mark for empirical check):
//     editedFieldsInput       object    required  — per-field edit spec
//       labels                object    optional  — label field edit
//         labelsAction        enum      required  — ADD | REMOVE | REPLACE (consistent with
//                                                   Atlassian bulk-edit UI semantics; exact
//                                                   string values unverified — could be
//                                                   "ADD"/"add"/"Add")
//         labels              string[]  required  — label names to add/remove/replace
//   UNVERIFIED:
//     sendBulkNotification    bool      optional  — suppress notifications during bulk op
//     jql                     string    optional  — NOT confirmed; research report flags
//                                                   as "assert by one source, not confirmed";
//                                                   implementation must verify empirically
//
// Response: HTTP 200 with body containing `taskId` (string).
//   CONFIRMED: status 200 (not 202 or 204). BulkOperationProgress schema returned.
//   NOTE: research report flagged taskId-in-body vs Location-header as uncertain.
//   OpenAPI extract confirms `taskId` field directly in BulkOperationProgress response body.
//   Tests use body_string_contains("taskId") rather than exact-JSON to tolerate minor
//   schema drift.
//
// === POST /rest/api/3/bulk/issues/transition ===
//
// Schema name: BulkTransitionSubmitInput — CONFIRMED from OpenAPI JSON:
//   selectedIssueIdsOrKeys  string[]  required  writeOnly — issue IDs or keys
//   transitionId            string    required  writeOnly — transition ID (NOT "id" or
//                                                           "transition.id"; direct top-level
//                                                           field named "transitionId")
//
// CONFIRMED: NOT "issueIds" (that was from an early secondary source, now refuted).
// Response: HTTP 200 with BulkOperationProgress (same shape as fields endpoint).
//
// === GET /rest/api/3/bulk/queue/{taskId} ===
//
// Schema name: BulkOperationProgress — CONFIRMED from OpenAPI JSON:
//   taskId                          string    readOnly  — the task ID
//   status                          enum      — CONFIRMED values: ENQUEUED | RUNNING |
//                                               COMPLETE | FAILED | CANCEL_REQUESTED |
//                                               CANCELLED | DEAD
//                                               NOTE: value is "COMPLETE" not "COMPLETED"
//                                               (OpenAPI schema). Research report used
//                                               "COMPLETED" (unverified). Tests use
//                                               "COMPLETE" per OpenAPI; if live API
//                                               returns "COMPLETED", tests will need
//                                               adjustment — flagged as empirical-verify.
//   processedAccessibleIssues       string[]  — issue IDs where operation succeeded
//   failedAccessibleIssues          object    — map of issue ID → error details
//   invalidOrInaccessibleIssueCount int32     — issues that couldn't be acted on
//   totalIssueCount                 int32
//   progressPercent                 int64
//   created                         date-time
//
// Per-issue error shape (BulkEditActionError):
//   errorMessages   string[]
//   errors          object    — map of field name → error message
//
// HTTP status codes:
//   POST /bulk/issues/fields     → 200 (confirmed from OpenAPI response refs)
//   POST /bulk/issues/transition → 200
//   GET  /bulk/queue/{taskId}    → 200
//
// KNOWN GAPS (implementer must verify empirically):
//   1. Exact casing/format of labelsAction enum values ("ADD" vs "add" vs "Add")
//   2. Whether "COMPLETE" or "COMPLETED" is the actual live API status string
//   3. Whether sendBulkNotification is a real field
//   4. Whether jql is an accepted optional field on /bulk/issues/fields
//   5. Exact nesting of editedFieldsInput for non-label fields (priority, assignee, etc.)
//
// Red Gate: all tests in this file MUST FAIL before implementation because:
//   - `--to` flag does not exist on `jr issue move`
//   - `keys: Vec<String>` positional does not exist (currently `key: String`)
//   - `src/api/jira/bulk.rs` (or equivalent) does not exist
//   - polling endpoint logic does not exist
//   Expected failure modes:
//     - Tests exercising new flags: clap exits 2 ("unrecognized argument")
//       → assertions on exit 0 or specific output fail.
//     - Tests checking bulk POST mock calls: command never reaches HTTP layer
//       → wiremock .expect(1) panics on drop (0 calls, expected 1).
//     - Cap test: no `> 1000 keys` check exists → command attempts HTTP call
//       → mock for exact 1-call assertion fails, or exit code differs.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{body_partial_json, body_string_contains, method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Build a `jr` command pointing at the mock server.
/// Auth injected via JR_AUTH_HEADER; instance URL via JR_BASE_URL.
fn jr_cmd(server_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0"); // test:test base64
    cmd
}

/// Build a bulk-edit fields endpoint response (HTTP 200 with taskId).
/// Matches BulkOperationProgress shape from the Atlassian OpenAPI spec.
fn bulk_task_enqueued_response(task_id: &str) -> serde_json::Value {
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

/// Build a completed task poll response (all issues succeeded).
fn bulk_task_complete_response(task_id: &str, processed_ids: Vec<&str>) -> serde_json::Value {
    serde_json::json!({
        "taskId": task_id,
        "status": "COMPLETE",
        "progressPercent": 100,
        "totalIssueCount": processed_ids.len(),
        "processedAccessibleIssues": processed_ids,
        "failedAccessibleIssues": {},
        "invalidOrInaccessibleIssueCount": 0
    })
}

/// Build a completed task response with some per-issue failures.
fn bulk_task_complete_with_failures_response(
    task_id: &str,
    processed_ids: Vec<&str>,
    failed: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "taskId": task_id,
        "status": "COMPLETE",
        "progressPercent": 100,
        "totalIssueCount": processed_ids.len() + failed.as_object().map_or(0, |m| m.len()),
        "processedAccessibleIssues": processed_ids,
        "failedAccessibleIssues": failed,
        "invalidOrInaccessibleIssueCount": 0
    })
}

/// Build an in-progress task poll response.
fn bulk_task_in_progress_response(task_id: &str) -> serde_json::Value {
    serde_json::json!({
        "taskId": task_id,
        "status": "RUNNING",
        "progressPercent": 50,
        "totalIssueCount": 3,
        "processedAccessibleIssues": [],
        "failedAccessibleIssues": {},
        "invalidOrInaccessibleIssueCount": 0
    })
}

// ---------------------------------------------------------------------------
// AC-001: jr issue edit KEY1 KEY2 KEY3 --label add:foo
//   → exactly 1 POST to /rest/api/3/bulk/issues/fields
//   + 1+ polls to /rest/api/3/bulk/queue/{taskId}
//   + exit 0 on COMPLETE
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_edit_multi_key_issues_one_bulk_post_then_polls_to_complete() {
    let server = MockServer::start().await;

    // The bulk edit POST: expect exactly 1 call.
    // Body must include selectedIssueIdsOrKeys with all three keys.
    // SCHEMA NOTE: labelsAction casing is best-guess "ADD"; see SCHEMA NOTES block.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": ["FOO-1", "FOO-2", "FOO-3"]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_enqueued_response("task-abc-001")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Poll endpoint: first call returns in-progress, second returns complete.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-abc-001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bulk_task_in_progress_response("task-abc-001")),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-abc-001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_complete_response(
                "task-abc-001",
                vec!["FOO-1", "FOO-2", "FOO-3"],
            )),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "FOO-3",
            "--label",
            "add:foo",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for multi-key bulk edit; stderr={stderr} stdout={stdout}"
    );
    // wiremock verifies .expect(1) on drop — missing bulk POST panics.
}

// ---------------------------------------------------------------------------
// AC-002: jr issue move KEY1 KEY2 KEY3 --to "Done"
//   → exactly 1 POST to /rest/api/3/bulk/issues/transition
//   + 1+ polls + exit 0
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_move_multi_key_issues_one_bulk_transition_post_then_polls() {
    let server = MockServer::start().await;

    // Transition lookup: need to resolve "Done" → transitionId.
    // jr currently calls GET /issue/{key}/transitions for single-key move.
    // For multi-key, it may call it for the first key only, or require --transition-id.
    // We mock a typical transitions response so the name-lookup path can work.
    // If the implementation uses a different discovery path, this mock is harmless.
    Mock::given(method("GET"))
        .and(path_regex(r"^/rest/api/3/issue/[A-Z]+-\d+/transitions$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [
                {"id": "31", "name": "Done", "to": {"name": "Done"}},
                {"id": "11", "name": "To Do", "to": {"name": "To Do"}},
                {"id": "21", "name": "In Progress", "to": {"name": "In Progress"}}
            ]
        })))
        .mount(&server)
        .await;

    // The bulk transition POST: expect exactly 1 call.
    // CONFIRMED schema: selectedIssueIdsOrKeys + transitionId (top-level, not nested).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/transition"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": ["BAR-10", "BAR-11", "BAR-12"],
            "transitionId": "31"
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_enqueued_response("task-trans-001")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Poll: in-progress then complete.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-trans-001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bulk_task_in_progress_response("task-trans-001")),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-trans-001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_complete_response(
                "task-trans-001",
                vec!["BAR-10", "BAR-11", "BAR-12"],
            )),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "move",
            "BAR-10",
            "BAR-11",
            "BAR-12",
            "--to",
            "Done",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for multi-key bulk move; stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-007 (partial-failure): Bulk task COMPLETE with per-issue errors
//   → exit 1 + stdout lists per-key success/error breakdown
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_edit_partial_failure_exits_one_with_per_key_breakdown() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bulk_task_enqueued_response("task-partial-001")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Poll response: COMPLETE but FOO-3 failed.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-partial-001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            bulk_task_complete_with_failures_response(
                "task-partial-001",
                vec!["FOO-1", "FOO-2"],
                serde_json::json!({
                    "FOO-3": {
                        "errorMessages": ["You do not have permission to edit this issue."],
                        "errors": {}
                    }
                }),
            ),
        ))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "FOO-3",
            "--label",
            "add:urgent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Must exit 1 (partial failure, not total failure or usage error).
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected exit 1 for partial bulk failure; stderr={stderr} stdout={stdout}"
    );

    // Output must reference FOO-3 and its error (or "failed"/"error").
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("FOO-3"),
        "Expected FOO-3 key in output for partial failure; combined={combined}"
    );
    assert!(
        combined.contains("permission")
            || combined.contains("failed")
            || combined.contains("error"),
        "Expected error description in output; combined={combined}"
    );
}

// ---------------------------------------------------------------------------
// AC-008 (polling): Polling respects Retry-After on 429
//   Mock sequence: 429 with Retry-After: 1 → 200 COMPLETE
//   Test verifies retry happens (exit 0, not exit 1 after giving up on 429).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_polling_respects_retry_after_on_429() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_enqueued_response("task-429-001")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // First poll: 429 with Retry-After header.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-429-001"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "1")
                .set_body_json(serde_json::json!({
                    "errorMessages": ["Rate limit exceeded"]
                })),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second poll: COMPLETE.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-429-001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_complete_response(
                "task-429-001",
                vec!["FOO-1", "FOO-2"],
            )),
        )
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
            "add:retry-test",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Must succeed after retry — NOT exit 1 treating 429 as fatal.
    assert!(
        output.status.success(),
        "Expected exit 0 after polling retried past 429 Retry-After; stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 (single-key BC): jr issue edit KEY1 --label add:foo
//   → still routes via bulk API (same code path), exits 0,
//   --output json shape has "key" field for backward compatibility.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_edit_single_key_routes_via_bulk_api_backward_compatible() {
    let server = MockServer::start().await;

    // Even for a single key, the bulk endpoint is called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": ["SOLO-1"]
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_enqueued_response("task-solo-001")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-solo-001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bulk_task_complete_response("task-solo-001", vec!["SOLO-1"])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "SOLO-1",
            "--label",
            "add:solo-test",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for single-key bulk edit; stderr={stderr} stdout={stdout}"
    );

    // --output json must produce parseable JSON.
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not valid JSON: {e}; stdout={stdout}"));

    // Backward-compat shape: either {"key":"SOLO-1"} (single-key shorthand)
    // or {"results":[{"key":"SOLO-1","status":"ok"}]} (multi-key shape with 1 item).
    // Either is acceptable; verify one of these shapes is present.
    let has_key_field = parsed.get("key").is_some();
    let has_results_field = parsed.get("results").is_some();
    assert!(
        has_key_field || has_results_field,
        "Expected {{\"key\":...}} or {{\"results\":[...]}} in --output json; got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Cap: > 1000 keys → exit 64 (USAGE error) with hint, NO HTTP call made.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_edit_more_than_1000_keys_exits_64_without_http_call() {
    let server = MockServer::start().await;

    // No mock registered — any HTTP call will return 501, causing a different error.
    // The test asserts exit 64 before any HTTP call is made.

    // Build 1001 fake issue keys.
    let keys: Vec<String> = (1..=1001).map(|i| format!("FOO-{i}")).collect();

    let output = jr_cmd(&server.uri())
        .arg("--no-input")
        .arg("issue")
        .arg("edit")
        .args(&keys)
        .arg("--label")
        .arg("add:overflow")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Exit 64 = USAGE error (EX_USAGE from sysexits.h, used by JrError).
    assert_eq!(
        output.status.code(),
        Some(64),
        "Expected exit 64 for > 1000 keys; stderr={stderr} stdout={stdout}"
    );

    // Hint must mention the cap or splitting.
    assert!(
        stderr.contains("1000")
            || stderr.contains("1,000")
            || stderr.contains("split")
            || stderr.contains("batch"),
        "Expected cap hint in stderr; stderr={stderr}"
    );

    // Verify no HTTP call was made to the bulk endpoint.
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "Expected zero HTTP calls for > 1000 key cap check"
    );
}

// ---------------------------------------------------------------------------
// --no-input skips the confirmation prompt for multi-key edits.
// (Interactive mode would prompt "Edit 3 issues? [y/N]"; --no-input must skip it.)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_edit_multi_key_with_no_input_skips_confirmation_prompt() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bulk_task_enqueued_response("task-noinput-001")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-noinput-001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_complete_response(
                "task-noinput-001",
                vec!["X-1", "X-2", "X-3"],
            )),
        )
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "X-1",
            "X-2",
            "X-3",
            "--label",
            "add:no-prompt",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 with --no-input (no prompt hang); stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(1) fires on drop — verifies the POST was made.
}

// ---------------------------------------------------------------------------
// Bonus: labels ADD vs REMOVE — verify labelsAction field is passed correctly.
// SCHEMA NOTE: "ADD" and "REMOVE" are best-guess casing; implementer must
// verify empirically against live Atlassian schema.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_edit_label_remove_sends_remove_action_in_bulk_payload() {
    let server = MockServer::start().await;

    // Use body_string_contains instead of body_partial_json for the action value,
    // because the exact casing of labelsAction enum is unverified (see SCHEMA NOTES).
    // This matcher is intentionally loose: it checks the substring "REMOVE" appears
    // in the request body JSON, tolerating different nesting structures.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_string_contains("REMOVE"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bulk_task_enqueued_response("task-remove-001")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-remove-001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_complete_response(
                "task-remove-001",
                vec!["FOO-1", "FOO-2"],
            )),
        )
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
            "remove:stale",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for --label remove:stale; stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Bonus: --output json for multi-key edit returns results array shape.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_edit_multi_key_output_json_returns_results_array() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_enqueued_response("task-json-001")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-json-001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bulk_task_complete_response(
                "task-json-001",
                vec!["FOO-1", "FOO-2"],
            )),
        )
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
            "add:json-test",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "Expected exit 0 for multi-key --output json; stderr={stderr} stdout={stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not valid JSON: {e}; stdout={stdout}"));

    // For multi-key, the JSON must have a "results" array with per-key entries.
    let results = parsed.get("results").and_then(|v| v.as_array());
    assert!(
        results.is_some(),
        "Expected {{\"results\":[...]}} for multi-key --output json; got: {stdout}"
    );
    let results = results.unwrap();
    assert_eq!(
        results.len(),
        2,
        "Expected 2 result entries for 2 keys; got: {stdout}"
    );
}
