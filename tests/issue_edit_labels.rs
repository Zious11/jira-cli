// Integration tests for the single-key `issue edit --label` fix (BUG-LABEL-400).
//
// Root cause: `jr issue edit SINGLE-KEY --label add:X / remove:Y` was routing
// single-key label edits through POST /rest/api/3/bulk/issues/fields with a
// malformed payload, causing HTTP 400 from real Jira. Found by live E2E run
// 26730687481.
//
// Fix: route single-key label edits to PUT /rest/api/3/issue/{key} with:
//   {"update": {"labels": [{"add": "foo"}, {"remove": "bar"}]}}
// where label values are BARE STRINGS (not {"name":...} objects).
// Multi-key (2+ keys) stays on the existing bulk path — DO NOT change it.
//
// Red Gate proof:
//   These tests MUST fail before the fix because the current code routes single-key
//   labels to POST /rest/api/3/bulk/issues/fields, not PUT /rest/api/3/issue/{key}.
//   The new mocks return 501 for the bulk endpoint (to fail loudly) and the tests
//   assert against PUT /rest/api/3/issue/{key} — which the current code never calls.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

/// Build a `jr` command pointing at the mock server.
fn jr_cmd(server_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0"); // test:test base64
    cmd
}

// ---------------------------------------------------------------------------
// Single-key add:foo + remove:bar — the bug scenario that caused live HTTP 400.
//
// Contract: exactly ONE PUT /rest/api/3/issue/ABC-1 with body:
//   {"update":{"labels":[{"add":"foo"},{"remove":"bar"}]}}
// and ZERO calls to /rest/api/3/bulk/issues/fields.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_single_key_label_add_and_remove_uses_put_issue_update() {
    let server = MockServer::start().await;

    // Bulk endpoint MUST NOT be called. .expect(0) panics on drop if called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(501)
                .set_body_string("BUG: single-key label edit routed to bulk endpoint"),
        )
        .expect(0)
        .mount(&server)
        .await;

    // PUT /rest/api/3/issue/ABC-1 with the canonical update.labels body.
    // Bare strings: {"add": "foo"} / {"remove": "bar"} — NOT {"name": "foo"}.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/ABC-1"))
        .and(body_partial_json(serde_json::json!({
            "update": {
                "labels": [
                    {"add": "foo"},
                    {"remove": "bar"}
                ]
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "ABC-1",
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
        "Expected exit 0 for single-key add+remove; stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(1) on PUT and .expect(0) on bulk POST fire on drop.
}

// ---------------------------------------------------------------------------
// Single-key add only — pure add path.
//
// Contract: exactly ONE PUT with {"update":{"labels":[{"add":"alpha"}]}}
// and ZERO calls to the bulk endpoint.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_single_key_label_add_only_uses_put_issue_update() {
    let server = MockServer::start().await;

    // Bulk endpoint MUST NOT be called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(501)
                .set_body_string("BUG: single-key label edit routed to bulk endpoint"),
        )
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/ADD-1"))
        .and(body_partial_json(serde_json::json!({
            "update": {
                "labels": [
                    {"add": "alpha"}
                ]
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "ADD-1",
            "--label",
            "add:alpha",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for single-key add-only; stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Single-key remove only — pure remove path.
//
// Contract: exactly ONE PUT with {"update":{"labels":[{"remove":"beta"}]}}
// and ZERO calls to the bulk endpoint.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_single_key_label_remove_only_uses_put_issue_update() {
    let server = MockServer::start().await;

    // Bulk endpoint MUST NOT be called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(501)
                .set_body_string("BUG: single-key label edit routed to bulk endpoint"),
        )
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/REM-1"))
        .and(body_partial_json(serde_json::json!({
            "update": {
                "labels": [
                    {"remove": "beta"}
                ]
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "REM-1",
            "--label",
            "remove:beta",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for single-key remove-only; stderr={stderr} stdout={stdout}"
    );
}

// ---------------------------------------------------------------------------
// Single-key --output json: must produce the existing single-key edit JSON shape
// {"key": "...", "updated": true, "changed_fields": {...}}.
//
// This is the same shape as non-label single-key edits (json_output::edit_response).
// The old bulk shape ({"taskId":...,"results":[...]}) must NOT appear.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_single_key_label_json_output_matches_edit_response_shape() {
    let server = MockServer::start().await;

    // Bulk endpoint MUST NOT be called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(501)
                .set_body_string("BUG: single-key label edit routed to bulk endpoint"),
        )
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/JSON-1"))
        .and(body_partial_json(serde_json::json!({
            "update": {
                "labels": [{"add": "foo"}]
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "JSON-1",
            "--label",
            "add:foo",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for single-key --output json label add; stderr={stderr} stdout={stdout}"
    );

    // stdout must be valid JSON.
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout not valid JSON: {e}; stdout={stdout}"));

    // Must have the single-key edit shape: {"key": ..., "updated": true, ...}
    assert_eq!(
        parsed.get("key").and_then(|v| v.as_str()),
        Some("JSON-1"),
        "Expected \"key\": \"JSON-1\" in JSON output; got: {stdout}"
    );
    assert_eq!(
        parsed.get("updated").and_then(|v| v.as_bool()),
        Some(true),
        "Expected \"updated\": true in JSON output; got: {stdout}"
    );

    // Must NOT have the bulk task shape.
    assert!(
        parsed.get("taskId").is_none(),
        "Expected no 'taskId' in single-key label JSON output; got: {stdout}"
    );
    assert!(
        parsed.get("results").is_none(),
        "Expected no 'results' in single-key label JSON output (bulk shape); got: {stdout}"
    );

    // changed_fields must be present (may be empty or contain "labels").
    assert!(
        parsed.get("changed_fields").is_some(),
        "Expected 'changed_fields' in JSON output; got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Regression: multi-key (2+ keys) still uses the BULK endpoint — unchanged path.
//
// This test must PASS before and after the fix (it pins the multi-key behavior
// to remain on the bulk path). It is not a red-gate test — it is a regression pin.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multi_key_label_still_uses_bulk_endpoint() {
    let server = MockServer::start().await;

    // The bulk endpoint MUST be called for multi-key.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": ["MK-1", "MK-2"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": "task-mk-001",
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

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-mk-001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "taskId": "task-mk-001",
            "status": "COMPLETE",
            "progressPercent": 100,
            "totalIssueCount": 2,
            "processedAccessibleIssues": ["MK-1", "MK-2"],
            "failedAccessibleIssues": {},
            "invalidOrInaccessibleIssueCount": 0
        })))
        .mount(&server)
        .await;

    // PUT /rest/api/3/issue/{key} must NOT be called for multi-key.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/MK-1"))
        .respond_with(
            ResponseTemplate::new(501)
                .set_body_string("BUG: multi-key label edit routed to single-key PUT"),
        )
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/MK-2"))
        .respond_with(
            ResponseTemplate::new(501)
                .set_body_string("BUG: multi-key label edit routed to single-key PUT"),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "MK-1",
            "MK-2",
            "--label",
            "add:foo",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit 0 for multi-key label add via bulk; stderr={stderr} stdout={stdout}"
    );
    // wiremock .expect(1) on bulk POST and .expect(0) on PUTs fire on drop.
}
