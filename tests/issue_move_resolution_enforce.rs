//! Integration tests for BC-3.2.013 proactive resolution enforcement on done-category transitions.
//!
//! Test naming follows `test_<verb>_<subject>_<expected_outcome>` per docs/specs/test-naming-convention.md.

use assert_cmd::Command;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method, path},
};

#[allow(dead_code)]
mod common;

// ── Fixture builders ─────────────────────────────────────────────────────────

/// Build a transitions response WITH statusCategory on `to` AND a `fields.resolution`
/// entry in the expanded shape.
///
/// This is the full expand=transitions.fields response for a done-category transition
/// that explicitly offers a resolution field.
fn transitions_with_done_cat_and_resolution_field(required: bool) -> serde_json::Value {
    json!({
        "transitions": [{
            "id": "31",
            "name": "Resolve Issue",
            "to": {
                "name": "Resolved",
                "statusCategory": {
                    "key": "done",
                    "name": "Done"
                }
            },
            "isConditional": false,
            "fields": {
                "resolution": {
                    "required": required,
                    "schema": {"type": "resolution", "system": "resolution"},
                    "allowedValues": [
                        {"id": "10000", "name": "Done"},
                        {"id": "10001", "name": "Won't Do"}
                    ]
                }
            }
        }]
    })
}

/// Build a transitions response for a done-category transition with NO fields map at all.
fn transitions_with_done_cat_no_fields() -> serde_json::Value {
    json!({
        "transitions": [{
            "id": "31",
            "name": "Resolve Issue",
            "to": {
                "name": "Resolved",
                "statusCategory": {
                    "key": "done",
                    "name": "Done"
                }
            }
        }]
    })
}

/// Build a transitions response with NO statusCategory on `to`.
fn transitions_with_no_status_category() -> serde_json::Value {
    json!({
        "transitions": [{
            "id": "31",
            "name": "Resolve Issue",
            "to": {
                "name": "Resolved"
            }
        }]
    })
}

/// Build a transitions response for a non-done-category transition that has a resolution field.
/// (Gate must not fire — statusCategory.key != "done".)
fn transitions_with_indeterminate_cat_and_resolution_field() -> serde_json::Value {
    json!({
        "transitions": [{
            "id": "21",
            "name": "Start Progress",
            "to": {
                "name": "In Progress",
                "statusCategory": {
                    "key": "indeterminate",
                    "name": "In Progress"
                }
            },
            "isConditional": false,
            "fields": {
                "resolution": {
                    "required": false,
                    "schema": {"type": "resolution", "system": "resolution"},
                    "allowedValues": []
                }
            }
        }]
    })
}

/// Build a transitions response for a done-category transition with `isConditional=true`
/// but NO resolution key in `fields` (validator-only scenario, EC-3.2.013-1).
fn transitions_with_done_cat_is_conditional() -> serde_json::Value {
    json!({
        "transitions": [{
            "id": "31",
            "name": "Resolve Issue",
            "to": {
                "name": "Resolved",
                "statusCategory": {
                    "key": "done",
                    "name": "Done"
                }
            },
            "isConditional": true,
            "fields": {}
        }]
    })
}

/// Build an issue GET response with status "Open" (for idempotency check).
/// Using "Open" / "new" category ensures idempotency doesn't short-circuit
/// when targeting done-category or in-progress transitions.
fn issue_response_in_progress(key: &str) -> serde_json::Value {
    json!({
        "key": key,
        "fields": {
            "summary": "Test issue",
            "status": {
                "name": "Open",
                "statusCategory": {"key": "new", "name": "To Do"}
            },
            "issuetype": {"name": "Task"},
            "priority": {"name": "Medium"},
            "assignee": null,
            "reporter": null,
            "project": {"key": "EJ"}
        }
    })
}

/// Resolutions list response (used when allowedValues absent or for fallback).
fn resolutions_response() -> serde_json::Value {
    json!([
        {"id": "10000", "name": "Done", "description": "Work complete."},
        {"id": "10001", "name": "Won't Do", "description": "Won't fix."}
    ])
}

// ── Helper: build a jr Command with wiremock + temp dirs ─────────────────────

fn jr_cmd_with_server(server: &MockServer) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0");
    cmd
}

// ── Test: read-command-stability — EXPECTED TO PASS (skip_serializing) ───────

/// BC-3.2.013 / ADR-0015 §3 read-command stability contract.
///
/// `Transition.fields` and `Transition.is_conditional` carry `#[serde(skip_serializing)]`.
/// A `Transition` deserialized from an expanded response (WITH populated `fields`)
/// must serialize back to JSON WITHOUT those keys appearing — ensuring
/// `jr issue transitions --output json` is byte-identical before and after this feature.
///
/// This test is expected to PASS immediately (skip_serializing is already in place
/// as part of the stub). It is the CI-enforced mechanical proof of ADR-0015 §3.
#[test]
fn test_transitions_json_output_unchanged_after_fields_added() {
    use jr::types::jira::{Transition, TransitionsResponse};

    // Deserialize a Transition WITH populated `fields` and `isConditional`.
    let json_with_fields = json!({
        "transitions": [{
            "id": "31",
            "name": "Resolve Issue",
            "to": {
                "name": "Resolved",
                "statusCategory": {"key": "done", "name": "Done"}
            },
            "isConditional": true,
            "fields": {
                "resolution": {
                    "required": true,
                    "allowedValues": [{"id": "10000", "name": "Done"}]
                }
            }
        }]
    });

    let resp: TransitionsResponse =
        serde_json::from_value(json_with_fields).expect("must deserialize expanded transitions");
    let t: &Transition = &resp.transitions[0];

    // Verify deserialization populated the new fields.
    assert!(
        t.fields.is_some(),
        "Transition.fields should be populated from the expanded response"
    );
    assert_eq!(
        t.is_conditional,
        Some(true),
        "Transition.is_conditional should be Some(true)"
    );

    // Now serialize and verify `fields` and `isConditional` do NOT appear.
    let serialized = serde_json::to_string(t).expect("must serialize Transition");
    assert!(
        !serialized.contains("\"fields\""),
        "skip_serializing VIOLATED: 'fields' appeared in serialized Transition: {serialized}"
    );
    assert!(
        !serialized.contains("\"isConditional\""),
        "skip_serializing VIOLATED: 'isConditional' appeared in serialized Transition: {serialized}"
    );

    // Verify the existing fields still serialize correctly.
    assert!(
        serialized.contains("\"id\""),
        "id must serialize: {serialized}"
    );
    assert!(
        serialized.contains("\"name\""),
        "name must serialize: {serialized}"
    );
}

// ── Test: mutual exclusion — EXPECTED TO PASS (clap handles this) ────────────

/// BC-3.2.013 flag constraint: --resolution and --no-resolution are mutually exclusive.
/// Clap exits 2 before any HTTP call when both are supplied.
///
/// This test is expected to PASS immediately (conflicts_with is already declared).
#[test]
fn test_move_mutual_exclusion_both_flags() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:19999") // unreachable — no HTTP should fire
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "move",
            "EJ-1",
            "Resolved",
            "--resolution",
            "Done",
            "--no-resolution",
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(2),
        "BC-3.2.013 mutual-exclusion: clap must exit 2 when both --resolution and \
         --no-resolution are supplied; got status {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── Enforcement tests — ALL EXPECTED TO FAIL (Red Gate) ──────────────────────
//
// The following tests assert that the proactive enforcement gate fires.
// Until the implementer adds the gate in workflow.rs, these will fail because
// `handle_move` does not yet call `get_transitions_with_fields` and has no
// enforcement block. The tests will get exit 0 (transition attempted) when they
// expect exit 64 (enforcement fired).

/// AC-002/AC-003: done-category + resolution field required + --no-input → exit 64.
/// POST must NOT be called (verified by wiremock expect(0) on the transition endpoint).
#[tokio::test]
async fn test_move_refuses_required_done_category_no_input() {
    let server = MockServer::start().await;

    // Mock the expand transitions GET (used by handle_move after enforcement is implemented).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(transitions_with_done_cat_and_resolution_field(true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Issue GET for idempotency check.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    // Resolutions endpoint — may or may not be called depending on implementation path.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/resolution"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resolutions_response()))
        .mount(&server)
        .await;

    // POST /transitions must NOT be called (enforcement gate must block it).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "move", "EJ-1", "Resolved", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.2.013 REQUIRED branch: must exit 64 when done-category transition \
         requires resolution and --no-input is set; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // Discriminating assertions (E-F1): verify REQUIRED-specific phrase is present
    // and OPTIONAL-specific phrase is absent — a `required→false` mutation that routes
    // to the OPTIONAL arm would produce "must explicitly choose" and miss "requires a
    // resolution", flipping both assertions.
    assert!(
        stderr.contains("requires a resolution"),
        "BC-3.2.013 REQUIRED branch: stderr must contain REQUIRED-specific phrase \
         'requires a resolution'; got stderr: {stderr}"
    );
    assert!(
        !stderr.contains("must explicitly choose"),
        "BC-3.2.013 REQUIRED branch: stderr must NOT contain OPTIONAL phrase \
         'must explicitly choose' (would indicate wrong branch); got stderr: {stderr}"
    );
    assert!(
        stderr.contains("--resolution"),
        "BC-3.2.013 REQUIRED branch: stderr must contain '--resolution' hint; \
         got stderr: {stderr}"
    );
    assert!(
        stderr.contains("jr issue resolutions"),
        "BC-3.2.013 REQUIRED branch: stderr must contain 'jr issue resolutions'; \
         got stderr: {stderr}"
    );

    server.verify().await;
}

/// AC-003: done-category + resolution field required + --no-resolution → exit 64
/// with "cannot be used" message (not the generic missing-resolution message).
#[tokio::test]
async fn test_move_refuses_required_done_category_with_no_resolution_flag() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(transitions_with_done_cat_and_resolution_field(true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    // POST must NOT be called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "move",
            "EJ-1",
            "Resolved",
            "--no-input",
            "--no-resolution",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.2.013 REQUIRED branch: --no-resolution on a required transition must exit 64; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.to_lowercase().contains("cannot be used")
            || stderr.to_lowercase().contains("cannot"),
        "BC-3.2.013 REQUIRED branch: stderr must indicate --no-resolution cannot be used \
         here; got stderr: {stderr}"
    );

    server.verify().await;
}

/// AC-003: done-category + resolution field required + --resolution Done + allowedValues present
/// → exit 0 + POST body contains resolution.name="Done".
#[tokio::test]
async fn test_move_proceeds_required_with_resolution_flag() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(transitions_with_done_cat_and_resolution_field(true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/resolution"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resolutions_response()))
        .mount(&server)
        .await;

    // POST must be called with resolution in body — BC-3.2.011 payload shape:
    // {"transition":{"id":"31"},"fields":{"resolution":{"name":"Done"}}}
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .and(body_partial_json(json!({
            "transition": {"id": "31"},
            "fields": {"resolution": {"name": "Done"}}
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "move",
            "EJ-1",
            "Resolved",
            "--no-input",
            "--resolution",
            "Done",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.2.013 REQUIRED branch: --resolution Done must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // wiremock.verify() confirms the POST was called with the correct body.
    server.verify().await;
}

/// AC-004: done-category + resolution field optional + --no-input + no flags → exit 64
/// with "must explicitly choose" in stderr AND both "--resolution" and "--no-resolution".
#[tokio::test]
async fn test_move_refuses_optional_done_category_no_input() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(transitions_with_done_cat_and_resolution_field(false)),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    // POST must NOT be called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "move", "EJ-1", "Resolved", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.2.013 OPTIONAL branch: must exit 64 when no flag supplied in --no-input mode; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--resolution"),
        "BC-3.2.013 OPTIONAL branch: stderr must mention '--resolution'; got: {stderr}"
    );
    assert!(
        stderr.contains("--no-resolution"),
        "BC-3.2.013 OPTIONAL branch: stderr must mention '--no-resolution'; got: {stderr}"
    );
    assert!(
        stderr.to_lowercase().contains("must explicitly choose")
            || stderr.to_lowercase().contains("choose"),
        "BC-3.2.013 OPTIONAL branch: stderr must indicate user must choose explicitly; \
         got: {stderr}"
    );

    server.verify().await;
}

/// AC-004: done-category + optional + --no-resolution --no-input → exit 0 + POST body
/// has NO top-level `fields` key (BC-3.2.012: intentional null-resolution close sends
/// only `{"transition":{"id":"31"}}`).
#[tokio::test]
async fn test_move_proceeds_optional_with_no_resolution_flag() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(transitions_with_done_cat_and_resolution_field(false)),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    // POST must be called with transition id in body but WITHOUT a top-level "fields" key.
    // body_partial_json asserts the transition id is present; received_requests() below
    // asserts "fields" is absent — together they pin BC-3.2.012.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .and(body_partial_json(json!({"transition": {"id": "31"}})))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "move",
            "EJ-1",
            "Resolved",
            "--no-input",
            "--no-resolution",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.2.013 OPTIONAL branch: --no-resolution must exit 0 (opt-out accepted); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // Verify the POST body has no "fields" key — the no-resolution path must NOT
    // include a resolution body (BC-3.2.012).
    let requests = server.received_requests().await.unwrap();
    let post_req = requests
        .iter()
        .find(|r| r.method == wiremock::http::Method::POST)
        .expect("POST /transitions must have been called");
    let body = String::from_utf8_lossy(&post_req.body);
    assert!(
        !body.contains("\"fields\""),
        "BC-3.2.012: POST body must NOT contain 'fields' key when --no-resolution is used; \
         got body: {body}"
    );

    server.verify().await;
}

/// AC-004: done-category + optional + --resolution Done --no-input → exit 0 + POST fired.
#[tokio::test]
async fn test_move_proceeds_optional_with_resolution_flag() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(transitions_with_done_cat_and_resolution_field(false)),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/resolution"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resolutions_response()))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "move",
            "EJ-1",
            "Resolved",
            "--no-input",
            "--resolution",
            "Done",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.2.013 OPTIONAL branch: --resolution Done must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    server.verify().await;
}

/// EC-3.2.013-1: done-category + isConditional=true (no resolution in fields) + --no-input
/// → treated as REQUIRED branch → exit 64 + stderr contains "--resolution".
#[tokio::test]
async fn test_move_isconditional_treated_as_required() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(transitions_with_done_cat_is_conditional()),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    // POST must NOT be called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "move", "EJ-1", "Resolved", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "EC-3.2.013-1: isConditional=true must be treated as REQUIRED branch → exit 64; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // Discriminating assertions (E-F1): isConditional forces the REQUIRED branch.
    // A `required = resolution_required_flag || is_conditional` → `false` mutation
    // would route to OPTIONAL and produce "must explicitly choose" instead of
    // "requires a resolution", flipping both assertions below.
    assert!(
        stderr.contains("requires a resolution"),
        "EC-3.2.013-1: isConditional=true must emit REQUIRED-specific phrase \
         'requires a resolution'; got: {stderr}"
    );
    assert!(
        !stderr.contains("must explicitly choose"),
        "EC-3.2.013-1: isConditional=true must NOT emit OPTIONAL phrase \
         'must explicitly choose' (would indicate wrong branch); got: {stderr}"
    );
    assert!(
        stderr.contains("--resolution"),
        "EC-3.2.013-1: stderr must contain '--resolution' hint; got: {stderr}"
    );

    server.verify().await;
}

// ── Conservative gate tests — EXPECTED TO PASS (no enforcement yet) ──────────

/// AC-002 conservative gate: no statusCategory on `to` + --no-input → exit 0 + POST fired.
/// Because enforcement is disabled, this passes immediately and remains green after implementation.
#[tokio::test]
async fn test_move_skips_gate_when_no_status_category() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(transitions_with_no_status_category()),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "move", "EJ-1", "Resolve Issue", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-002 conservative gate: no statusCategory → POST must fire, exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    server.verify().await;
}

/// AC-002: statusCategory.key="indeterminate" + resolution field present + --no-input
/// → gate does NOT fire → exit 0 + POST fired.
#[tokio::test]
async fn test_move_skips_gate_when_not_done_category() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(transitions_with_indeterminate_cat_and_resolution_field()),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "move", "EJ-1", "Start Progress", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-002: indeterminate statusCategory → gate must NOT fire → exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    server.verify().await;
}

/// EC-3.2.013-4: done-category + NO `fields` key at all + --no-input
/// → conservative gate fires → exit 0 + POST fired.
#[tokio::test]
async fn test_move_skips_gate_when_fields_absent() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(transitions_with_done_cat_no_fields()),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "move", "EJ-1", "Resolve Issue", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "EC-3.2.013-4: done-cat + no fields → conservative gate → exit 0 + POST fired; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    server.verify().await;
}

/// AC-006/EC-3.2.013-8: bulk move (multi-key) with done-category target.
/// Proactive gate must NOT fire on the bulk path. The bulk POST is attempted.
/// (Verified by checking that the bulk transition endpoint is hit, not single-key.)
#[tokio::test]
async fn test_bulk_move_excludes_proactive_enforcement() {
    let server = MockServer::start().await;

    // Transitions GET for first key (bulk discovery).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(transitions_with_done_cat_and_resolution_field(true)),
        )
        .mount(&server)
        .await;

    // Bulk transition POST — must be called (no proactive gate on bulk path).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/transition"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "taskId": "bulk-task-123"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Bulk task poll — uses /rest/api/3/bulk/queue/{taskId} (NOT /rest/api/3/task/).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/bulk-task-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "COMPLETE",
            "progress": {
                "processedAccessibleIssues": ["EJ-1", "EJ-2"],
                "failedAccessibleIssues": {}
            }
        })))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "move",
            "EJ-1",
            "EJ-2",
            "--to",
            "Resolve Issue",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The bulk path must complete successfully (exit 0) — no enforcement gate,
    // bulk POST was called and the mock task resolved COMPLETE.
    assert_eq!(
        output.status.code(),
        Some(0),
        "EC-3.2.013-8: bulk move must NOT trigger proactive enforcement and must exit 0 \
         when bulk task completes successfully. got exit {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    // Verify bulk POST was called (expect(1) mock confirms the gate was NOT bypassed).
    server.verify().await;
}

// ── FIX F-2: allowedValues validation (EC-3.2.013-3) ─────────────────────────

/// EC-3.2.013-3: done-category + required resolution + allowedValues present + --resolution
/// value NOT in allowedValues → exit 64 listing allowed values; POST NOT called.
///
/// Fixture has allowedValues=[Done, Won't Do]. Passing --resolution "Bogus" should fail
/// before the POST with a message naming the allowed values.
#[tokio::test]
async fn test_move_refuses_resolution_not_in_allowed_values() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(transitions_with_done_cat_and_resolution_field(true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    // POST must NOT be called — allowedValues validation fires before the HTTP request.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "move",
            "EJ-1",
            "Resolved",
            "--no-input",
            "--resolution",
            "Bogus",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "EC-3.2.013-3: --resolution value not in allowedValues must exit 64; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // The error must name the disallowed value and list the allowed ones.
    assert!(
        stderr.contains("Bogus") || stderr.to_lowercase().contains("not allowed"),
        "EC-3.2.013-3: stderr must reference the invalid value or 'not allowed'; \
         got: {stderr}"
    );
    assert!(
        stderr.contains("Done") || stderr.contains("Won't Do"),
        "EC-3.2.013-3: stderr must list allowed values (Done, Won't Do); got: {stderr}"
    );

    server.verify().await;
}

// ── OBS-2: BC-3.2.009 reactive 400 backstop — pins finish_transition error arm ──

/// BC-3.2.009 reactive backstop via `finish_transition` (OBS-2, F6 mutant pre-emption).
///
/// The proactive gate (BC-3.2.013) does NOT fire here — the GET transitions fixture
/// returns a done-category `to` with NO `fields` key and `isConditional` absent, so
/// the conservative gate applies (EC-3.2.013-4) and the POST is attempted.
///
/// The POST then returns a Jira-style 400 with both "resolution" and "required" in the
/// error body, which `finish_transition` must detect and rewrite into an actionable
/// `--resolution` hint (BC-3.2.009).
///
/// Killing mutants on `finish_transition`'s error arm:
///   - `&&` → `||`:  "required" alone would match on many non-resolution errors → test
///     would pass even if the arm fires spuriously, but `--resolution` would still appear.
///     With the `&&` mutated to `||`, a body containing only "resolution" (no "required")
///     would trigger the hint when it shouldn't. The test fixture uses the minimal real
///     Atlassian body shape so the `&&` is load-bearing.
///   - `.contains("resolution")` deleted: arm never fires → raw API error propagates →
///     stderr no longer contains "--resolution" hint → assertion fails.
///   - `.contains("required")` deleted: same — arm never fires on non-resolution 400s →
///     but this specific test fixture body contains both, so the deleted check would
///     make the arm fire unconditionally → hint always appears → a DIFFERENT test (one
///     sending a non-resolution 400) would be needed to kill this mutant. Coverage is
///     sufficient for this diff scope; the `&&` test kills the join-condition mutant.
///   - Entire `if let Err(err)` arm deleted: exit 0 instead of exit 64 → `assert_eq!
///     (code, Some(64))` fails.
#[tokio::test]
async fn test_move_reactive_backstop_400_resolution_required() {
    let server = MockServer::start().await;

    // GET transitions returns done-category `to` BUT with NO `fields` key and no
    // `isConditional` flag — conservative gate fires (EC-3.2.013-4), skips proactive
    // enforcement, and lets the POST reach the server.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(transitions_with_done_cat_no_fields()),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    // POST returns 400 with the Atlassian-format resolution-required error body.
    // `finish_transition` must detect "resolution" AND "required" (case-insensitive)
    // and rewrite the raw API error into the actionable --resolution hint.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "errors": {
                "resolution": "Field 'resolution' is required"
            },
            "errorMessages": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "move", "EJ-1", "Resolve Issue", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.2.009 reactive backstop: 400 resolution-required must exit 64; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // finish_transition must rewrite the raw API error into an actionable hint.
    assert!(
        stderr.contains("--resolution"),
        "BC-3.2.009 reactive backstop: stderr must contain '--resolution' hint \
         (finish_transition error arm); got: {stderr}"
    );
    assert!(
        stderr.contains("requires a resolution") || stderr.contains("jr issue resolutions"),
        "BC-3.2.009 reactive backstop: stderr must contain resolution-required hint text; \
         got: {stderr}"
    );
    // The raw API error body must NOT leak through unformatted.
    assert!(
        !stderr.contains("Field 'resolution' is required"),
        "BC-3.2.009 reactive backstop: raw Jira error must be rewritten, not leaked; \
         got: {stderr}"
    );

    server.verify().await;
}

// ── FIX O-5: unit tests for pure interactive-choice helper ───────────────────

/// Unit tests for `resolve_interactive_choice` — the pure helper that maps a
/// dialoguer selection index onto `Option<String>` (Some(name) → set resolution;
/// None → no resolution in body).
///
/// This tests the security-relevant "(none)" decision without a TTY or a live server.
#[cfg(test)]
mod resolve_interactive_choice_tests {
    use jr::cli::issue::workflow::resolve_interactive_choice;

    #[test]
    fn test_resolve_interactive_choice_none_sentinel_returns_none() {
        let options = vec!["Done".to_string(), "Won't Do".to_string()];
        let none_label = "(none — no resolution)";
        // Selecting the "(none)" sentinel → None (no resolution in POST body).
        let result = resolve_interactive_choice(&options, none_label, 2);
        assert_eq!(
            result, None,
            "selecting the '(none)' option must return None (no resolution body)"
        );
    }

    #[test]
    fn test_resolve_interactive_choice_real_value_returns_some() {
        let options = vec!["Done".to_string(), "Won't Do".to_string()];
        let none_label = "(none — no resolution)";
        // Selecting index 0 → Some("Done").
        let result = resolve_interactive_choice(&options, none_label, 0);
        assert_eq!(
            result,
            Some("Done".to_string()),
            "selecting a real resolution must return Some(name)"
        );
    }

    #[test]
    fn test_resolve_interactive_choice_second_real_value_returns_some() {
        let options = vec!["Done".to_string(), "Won't Do".to_string()];
        let none_label = "(none — no resolution)";
        let result = resolve_interactive_choice(&options, none_label, 1);
        assert_eq!(result, Some("Won't Do".to_string()));
    }
}

// ── FIX E-F2: unit tests for build_resolution_prompt ─────────────────────────

/// Unit tests for `build_resolution_prompt` — the pure helper that builds the
/// dialoguer item list with or without the "(none — no resolution)" sentinel.
///
/// The REQUIRED-vs-OPTIONAL "(none)" inclusion is the security-relevant distinction:
/// REQUIRED must never offer the sentinel; OPTIONAL must always end with it.
/// These tests catch any `allow_none` routing bug (including mutations) without a TTY.
#[cfg(test)]
mod build_resolution_prompt_tests {
    use jr::cli::issue::workflow::{NONE_LABEL, build_resolution_prompt};

    #[test]
    fn test_build_resolution_prompt_required_branch_excludes_none_sentinel() {
        let base = vec!["Done".to_string(), "Won't Do".to_string()];
        // REQUIRED branch (allow_none=false): result must NOT contain/end-with NONE_LABEL.
        let prompt = build_resolution_prompt(&base, false);
        assert_eq!(
            prompt, base,
            "REQUIRED branch: prompt must equal base list with no sentinel appended"
        );
        assert!(
            !prompt.contains(&NONE_LABEL.to_string()),
            "REQUIRED branch: prompt must NOT contain the '(none)' sentinel; got: {prompt:?}"
        );
    }

    #[test]
    fn test_build_resolution_prompt_optional_branch_ends_with_none_sentinel() {
        let base = vec!["Done".to_string(), "Won't Do".to_string()];
        // OPTIONAL branch (allow_none=true): result must end with NONE_LABEL.
        let prompt = build_resolution_prompt(&base, true);
        assert_eq!(
            prompt.last().map(String::as_str),
            Some(NONE_LABEL),
            "OPTIONAL branch: prompt must end with NONE_LABEL sentinel; got: {prompt:?}"
        );
        assert_eq!(
            prompt.len(),
            base.len() + 1,
            "OPTIONAL branch: prompt length must be base.len() + 1"
        );
        // Base entries are preserved at the start.
        assert_eq!(&prompt[..base.len()], base.as_slice());
    }

    #[test]
    fn test_build_resolution_prompt_empty_base_optional_still_has_sentinel() {
        // Edge case: even with an empty base list, OPTIONAL appends the sentinel.
        // (OBS-1 guard in handle_move prevents an actually-empty REQUIRED prompt;
        //  this test just validates the pure helper is sentinel-inclusive for OPTIONAL.)
        let base: Vec<String> = vec![];
        let prompt = build_resolution_prompt(&base, true);
        assert_eq!(
            prompt,
            vec![NONE_LABEL.to_string()],
            "OPTIONAL with empty base must still produce [NONE_LABEL]"
        );
    }

    #[test]
    fn test_build_resolution_prompt_empty_base_required_stays_empty() {
        let base: Vec<String> = vec![];
        let prompt = build_resolution_prompt(&base, false);
        assert!(
            prompt.is_empty(),
            "REQUIRED with empty base must produce an empty list (OBS-1 guard handles this at call site)"
        );
    }
}

// ── F6 helper: refuse_noninteractive unit tests (kills S2/S3/S5 + timeout) ────

/// Unit tests for `refuse_noninteractive` — all four (no_input, stdin_is_tty) combos.
///
/// Varying each operand independently kills:
///   - `||` → `&&` mutant: (false,true)→false proves the short-circuit cannot be AND
///   - `!stdin_is_tty` → `stdin_is_tty` mutant: (false,true)→false fails if `!` removed
///   - `no_input`-only mutant: (false,false)→true proves stdin_is_tty is load-bearing
///   - entire-expression-true mutant: (false,true)→false proves the guard isn't always-on
#[cfg(test)]
mod refuse_noninteractive_tests {
    use jr::cli::issue::workflow::refuse_noninteractive;

    #[test]
    fn test_refuse_noninteractive_no_input_true_tty_true_refuses() {
        // no_input=true always refuses regardless of TTY.
        assert!(
            refuse_noninteractive(true, true),
            "no_input=true must refuse even when stdin is a TTY"
        );
    }

    #[test]
    fn test_refuse_noninteractive_no_input_true_tty_false_refuses() {
        // Double-refusal: both conditions say non-interactive.
        assert!(
            refuse_noninteractive(true, false),
            "no_input=true + not-a-TTY must refuse"
        );
    }

    #[test]
    fn test_refuse_noninteractive_no_input_false_tty_false_refuses() {
        // stdin not a TTY → refuse even if --no-input was not set.
        assert!(
            refuse_noninteractive(false, false),
            "stdin not a TTY must refuse even without --no-input"
        );
    }

    #[test]
    fn test_refuse_noninteractive_no_input_false_tty_true_allows() {
        // Only case that allows interactive: no flag AND stdin IS a TTY.
        assert!(
            !refuse_noninteractive(false, true),
            "no_input=false + stdin is TTY must allow interactive prompt"
        );
    }
}

// ── F6 helper: select_prompt_base_names unit tests (kills S4/S6) ──────────────

/// Unit tests for `select_prompt_base_names` — kills the `!`-deletion mutant on
/// the emptiness check that would cause the function to always return `instance_list`.
#[cfg(test)]
mod select_prompt_base_names_tests {
    use jr::cli::issue::workflow::select_prompt_base_names;

    #[test]
    fn test_select_prompt_base_names_non_empty_transition_list_wins() {
        let transition = vec!["Done".to_string(), "Won't Do".to_string()];
        let instance = vec!["Fixed".to_string(), "Duplicate".to_string()];
        // Non-empty transition list must be returned, NOT the instance list.
        let result = select_prompt_base_names(&transition, &instance);
        assert_eq!(
            result,
            transition.as_slice(),
            "non-empty allowed_from_transition must be returned (not instance_list)"
        );
        // Verify the instance list is genuinely different so the test is discriminating.
        assert_ne!(result, instance.as_slice());
    }

    #[test]
    fn test_select_prompt_base_names_empty_transition_list_falls_back_to_instance() {
        let transition: Vec<String> = vec![];
        let instance = vec!["Fixed".to_string(), "Duplicate".to_string()];
        // Empty transition list must fall back to instance list.
        let result = select_prompt_base_names(&transition, &instance);
        assert_eq!(
            result,
            instance.as_slice(),
            "empty allowed_from_transition must fall back to instance_list"
        );
    }

    #[test]
    fn test_select_prompt_base_names_both_empty_returns_empty_instance() {
        let transition: Vec<String> = vec![];
        let instance: Vec<String> = vec![];
        let result = select_prompt_base_names(&transition, &instance);
        assert!(result.is_empty(), "both empty → result is empty");
    }
}

// ── F6 helper: optional_prompt_default_index unit tests (kills S7/S8) ─────────

/// Unit tests for `optional_prompt_default_index` — kills `-`→`+`, `-`→`*`, and
/// deletion mutants on the `len.saturating_sub(1)` default-index computation.
#[cfg(test)]
mod optional_prompt_default_index_tests {
    use jr::cli::issue::workflow::optional_prompt_default_index;

    #[test]
    fn test_optional_prompt_default_index_typical_list() {
        // 3 items → default index 2 (the last item, which is NONE_LABEL).
        assert_eq!(
            optional_prompt_default_index(3),
            2,
            "len=3 must yield index 2 (last item)"
        );
    }

    #[test]
    fn test_optional_prompt_default_index_single_item() {
        // 1 item → default index 0.
        assert_eq!(
            optional_prompt_default_index(1),
            0,
            "len=1 must yield index 0"
        );
    }

    #[test]
    fn test_optional_prompt_default_index_zero_saturates() {
        // len=0 → saturating_sub gives 0 (no panic, no wrap-around).
        assert_eq!(
            optional_prompt_default_index(0),
            0,
            "len=0 must yield 0 via saturating_sub (no underflow)"
        );
    }

    #[test]
    fn test_optional_prompt_default_index_larger_list() {
        // Spot-check: 5 items → index 4.
        assert_eq!(optional_prompt_default_index(5), 4);
    }
}

// ── S1: finish_transition `&&` passthrough test ──────────────────────────────

/// BC-3.2.009 backstop: 400 body contains "resolution" but NOT "required" — the
/// reactive arm must NOT fire → raw error propagates (no --resolution rewrite).
///
/// Under the `&&`→`||` mutant, "resolution" alone would wrongly trigger the rewrite
/// hint, making stderr contain "--resolution" when it should not.  Asserting the
/// ABSENCE of the hint kills that mutant.
///
/// Setup: conservative gate (no statusCategory on `to`) skips proactive enforcement;
/// POST returns 400 mentioning "resolution" in a non-required context (e.g., a generic
/// field validation error that happens to use the word).
#[tokio::test]
async fn test_move_reactive_backstop_400_resolution_word_only_passthrough() {
    let server = MockServer::start().await;

    // Conservative gate: no statusCategory → proactive enforcement skipped.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(transitions_with_no_status_category()),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_response_in_progress("EJ-1")))
        .mount(&server)
        .await;

    // 400 body contains "resolution" but NOT "required" — a generic field error.
    // The `&&` condition in finish_transition must NOT match → raw error propagates.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/transitions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "errors": {
                "resolution": "Invalid resolution value supplied"
            },
            "errorMessages": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = jr_cmd_with_server(&server)
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "move", "EJ-1", "Resolve Issue", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must be non-zero (the 400 is a real error).
    assert_ne!(
        output.status.code(),
        Some(0),
        "400 response must cause non-zero exit; got 0\nstderr: {stderr}"
    );
    // The --resolution rewrite hint must NOT appear — the `&&` arm requires BOTH
    // "resolution" and "required".  Under ||→&& the hint would appear; this assertion
    // kills that mutant.
    assert!(
        !stderr.contains("requires a resolution") && !stderr.contains("jr issue resolutions"),
        "BC-3.2.009 `&&` gate: 'resolution'-only 400 must NOT trigger the \
         --resolution rewrite hint; got stderr: {stderr}"
    );

    server.verify().await;
}
