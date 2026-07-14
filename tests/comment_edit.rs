//! CLI-level integration tests for `jr issue comment edit`.
//!
//! S-577-4 (11 tests): body-only PUT — handler implemented; all S-577-4 tests GREEN.
//! S-577-5 (13 new tests, 12 ACs): visibility flags (--internal/--public/--yes).
//!
//! S-577-5 Red Gate: tests fail because visibility branches are ABSENT from
//! `handle_comment_edit` — internal: _, public: _, yes: _ are currently ignored.
//! Feature-absence failures (not compilation panics):
//!   RED  — AC-001..009, AC-012: visibility branches not yet implemented → wrong
//!           exit codes or wrong wire/response shape.
//!   GREEN — AC-010 (2 variants), AC-011: pre-satisfied by S-577-4/S-577-1 (regression guards).
//!
//! BC anchors (S-577-4): BC-3.5.005, BC-3.5.009
//! BC anchors (S-577-5): BC-3.5.006, BC-3.5.007, BC-3.5.008, BC-3.5.011
//! VPs (S-577-4): VP-577-001, VP-577-011, VP-577-012, VP-577-022(b), VP-577-023,
//!               VP-577-024, VP-577-026 (variant 3)
//! VPs (S-577-5): VP-577-002, VP-577-003, VP-577-006, VP-577-010, VP-577-017,
//!               VP-577-025, VP-577-026 (variants 1+2), VP-577-028, VP-577-029, VP-577-030 (v2)
//! Stories: S-577-4, S-577-5, GitHub issue #577

use assert_cmd::Command;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Write;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helper
// ---------------------------------------------------------------------------

/// Build a `jr` command pointing at the mock server with XDG isolation.
/// Does NOT add `--no-input` or any defaults — callers supply all flags.
fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"));
    cmd
}

/// Minimal Jira PUT-comment response body.
/// `update_comment` returns `Result<()>` — the handler builds the JSON
/// response locally from caller-supplied state and discards the Jira body.
fn minimal_comment_response() -> serde_json::Value {
    serde_json::json!({
        "id": "10001",
        "self": "https://example.atlassian.net/rest/api/3/issue/FOO-1/comment/10001"
    })
}

// ---------------------------------------------------------------------------
// AC-001 / VP-577-023
// BC-3.5.005 postcondition — response JSON exact top-level key-set + human mode
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "new body" --output json`
/// against a wiremock returning 200 exits 0; stdout parses as JSON; top-level
/// key set == `BTreeSet::from(["changed_fields","id","key","updated"])` (exact);
/// `changed_fields` has exactly 1 sub-key: `"body"` (no `"jsm_internal"`).
///
/// Human-mode variant: `jr issue comment edit FOO-1 --id 10001 "Updated text"`
/// → exit 0; stderr contains `"Updated comment 10001 on FOO-1"`; stdout empty.
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_005_edit_response_exact_key_set() {
    // --- JSON mode variant ---
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "new body", "--output", "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-001 JSON: must exit 0 on 200; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("AC-001 JSON: stdout must be valid JSON; parse error: {e}\nstdout: {stdout}")
    });

    let top_keys: BTreeSet<&str> = parsed
        .as_object()
        .expect("AC-001 JSON: stdout must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();

    assert_eq!(
        top_keys,
        BTreeSet::from(["changed_fields", "id", "key", "updated"]),
        "AC-001 JSON: top-level key set must be exactly \
         {{\"changed_fields\", \"id\", \"key\", \"updated\"}}; got {top_keys:?}"
    );

    let cf_keys: BTreeSet<&str> = parsed["changed_fields"]
        .as_object()
        .expect("AC-001 JSON: changed_fields must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();

    assert_eq!(
        cf_keys,
        BTreeSet::from(["body"]),
        "AC-001 JSON: changed_fields must have exactly 1 sub-key 'body' \
         (no 'jsm_internal' in the default path); got {cf_keys:?}"
    );

    // --- Human mode variant ---
    let server2 = MockServer::start().await;
    let cache_dir2 = tempfile::tempdir().unwrap();
    let config_dir2 = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server2)
        .await;

    let output2 = jr_cmd(&server2.uri(), cache_dir2.path(), config_dir2.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "Updated text",
        ])
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    assert_eq!(
        output2.status.code(),
        Some(0),
        "AC-001 human: must exit 0; got {:?}\nstderr: {stderr2}\nstdout: {stdout2}",
        output2.status.code()
    );
    assert!(
        stderr2.contains("Updated comment 10001 on FOO-1"),
        "AC-001 human: stderr must contain 'Updated comment 10001 on FOO-1'; got: {stderr2}"
    );
    assert!(
        stdout2.trim().is_empty(),
        "AC-001 human: stdout must be empty on human success path (Symmetric profile); \
         got stdout: {stdout2}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 / VP-577-023
// BC-3.5.005 postcondition — changed_fields.body is raw pre-trim; ADF is trimmed
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "  hello world  " --output json`
/// → exit 0; `changed_fields.body` == `"  hello world  "` (raw pre-trim echo preserved);
/// AND the PUT request body ADF text node == `"hello world"` (trimmed per BC-3.5.005 ~2295).
///
/// Two assertions in the same test:
/// 1. `changed_fields.body` == `"  hello world  "` (raw pre-trim — lossless channel)
/// 2. PUT wire body: `body.content[0].content[0].text` == `"hello world"` (trimmed)
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_005_edit_changed_fields_body_is_raw_pre_trim() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "  hello world  ",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-002: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("AC-002: stdout must be valid JSON; parse error: {e}\nstdout: {stdout}")
    });

    // Assertion 1: raw pre-trim echo in changed_fields.body
    assert_eq!(
        parsed["changed_fields"]["body"].as_str(),
        Some("  hello world  "),
        "AC-002: changed_fields.body must be raw pre-trim input '  hello world  '; \
         got: {:?}",
        parsed["changed_fields"]["body"]
    );

    // Assertion 2: PUT wire body ADF text node must be trimmed
    let reqs = server.received_requests().await.unwrap();
    assert_eq!(
        reqs.len(),
        1,
        "AC-002: expected exactly 1 PUT request; got {}",
        reqs.len()
    );

    let put_body: Value =
        serde_json::from_slice(&reqs[0].body).expect("AC-002: PUT request body must be valid JSON");

    let adf_text = put_body["body"]["content"][0]["content"][0]["text"].as_str();
    assert_eq!(
        adf_text,
        Some("hello world"),
        "AC-002: ADF text node must be trimmed to 'hello world' (trim-then-ADF per BC-3.5.005); \
         got: {adf_text:?}\nfull PUT body: {put_body}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 / VP-577-001
// BC-3.5.005 EC-3.5.005-1 — PUT request body contains ONLY "body" key
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body text" --output json`
/// sends a PUT request body with exactly key `"body"` and does NOT contain `"properties"`.
///
/// Wire-level inspection via `server.received_requests()`.
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_005_ec1_put_request_has_only_body_key() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "body text",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-003: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(
        reqs.len(),
        1,
        "AC-003: expected exactly 1 PUT request; got {}",
        reqs.len()
    );

    let body_json: Value =
        serde_json::from_slice(&reqs[0].body).expect("AC-003: PUT request body must be valid JSON");

    assert!(
        body_json.get("properties").is_none(),
        "AC-003: PUT request body must NOT contain 'properties' key in default path \
         (body-only invariant EC-3.5.005-1); got body: {body_json}"
    );
    assert!(
        body_json.get("body").is_some(),
        "AC-003: PUT request body must contain 'body' key; got body: {body_json}"
    );

    let keys: Vec<&str> = body_json
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        keys.len(),
        1,
        "AC-003: PUT request body must have exactly 1 key (only 'body'); got keys: {keys:?}"
    );
}

// ---------------------------------------------------------------------------
// AC-004
// BC-3.5.009 body-source list — --file happy path
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 --file <path> --output json`
/// with a temp file containing `"body from file\n"` → exit 0;
/// `changed_fields.body` == `"body from file\n"` (raw file content).
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_009_edit_file_body_source() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Create a temp file with body content
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "body from file").unwrap();
    let file_path = tmp.path().to_str().unwrap().to_owned();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "--file", &file_path, "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-004: must exit 0 on --file happy path; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("AC-004: stdout must be valid JSON; parse error: {e}\nstdout: {stdout}")
    });

    assert_eq!(
        parsed["changed_fields"]["body"].as_str(),
        Some("body from file\n"),
        "AC-004: changed_fields.body must be raw file content 'body from file\\n'; \
         got: {:?}",
        parsed["changed_fields"]["body"]
    );
}

// ---------------------------------------------------------------------------
// AC-005
// BC-3.5.009 body-source list — --stdin happy path
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 --stdin --no-input --output json`
/// with stdin fed `"body from stdin"` → exit 0; `changed_fields.body` contains
/// `"body from stdin"`.
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_009_edit_stdin_body_source() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "--stdin",
            "--no-input",
            "--output",
            "json",
        ])
        .write_stdin("body from stdin")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-005: must exit 0 on --stdin happy path; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("AC-005: stdout must be valid JSON; parse error: {e}\nstdout: {stdout}")
    });

    assert!(
        parsed["changed_fields"]["body"]
            .as_str()
            .map(|s| s.contains("body from stdin"))
            .unwrap_or(false),
        "AC-005: changed_fields.body must contain 'body from stdin'; \
         got: {:?}",
        parsed["changed_fields"]["body"]
    );
}

// ---------------------------------------------------------------------------
// AC-006 / VP-577-011
// BC-3.5.009 EC-3.5.009-1 — file not found → exit 64 (NOT exit 1)
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 --file /no/such/file.md`
/// → exit 64; stderr contains `"file not found"` (EC-3.5.009-1 explicit remap);
/// MUST NOT exit 1 (default ApiError path). No PUT call is made.
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_009_ec1_file_not_found_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "--file",
            "/no/such/file.md",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-006: must exit 64 on file not found (EC-3.5.009-1 UserError remap); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("file not found"),
        "AC-006: stderr must contain 'file not found'; got: {stderr}"
    );
    assert_ne!(
        output.status.code(),
        Some(1),
        "AC-006: must NOT exit 1 (default ApiError path); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-007 / VP-577-012
// BC-3.5.009 EC-3.5.009-5 — empty/whitespace body → exit 64
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "   "` (all-whitespace
/// positional arg) → exit 64; stderr contains `"comment body cannot be empty"`;
/// no HTTP PUT call.
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_009_ec5_empty_whitespace_body_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "edit", "FOO-1", "--id", "10001", "   "])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-007: must exit 64 on whitespace-only body (EC-3.5.009-5); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("comment body cannot be empty"),
        "AC-007: stderr must contain 'comment body cannot be empty' \
         (load-bearing substring); got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-008 / VP-577-022 variant (b)
// BC-3.5.005 EC-3.5.005-2 (cross-ref BC-3.5.002 EC-3.5.002-1) — invalid --id → exit 64
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id "10001;x" "body"` → exit 64;
/// stderr contains `"invalid comment id"`; PUT has `.expect(0)`.
///
/// The semicolon is not in the allowed set `^[0-9A-Za-z_-]+$`
/// (EC-3.5.002-1 shared charset validation).
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_002_ec1_edit_invalid_id_regex_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001;x", "body",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-008: must exit 64 on invalid --id charset (EC-3.5.002-1); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("invalid comment id"),
        "AC-008: stderr must contain 'invalid comment id'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 / VP-577-026 variant 3
// BC-3.5.005 postcondition — jsm_internal absent in default path
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --output json`
/// → exit 0; parsed `changed_fields` does NOT contain key `"jsm_internal"`.
///
/// `jsm_internal` is only present when `--internal` or `--public` flags are
/// used — that is S-577-5's scope.
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_005_jsm_internal_absent_in_default_path() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "body", "--output", "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-009: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("AC-009: stdout must be valid JSON; parse error: {e}\nstdout: {stdout}")
    });

    assert!(
        parsed["changed_fields"].get("jsm_internal").is_none(),
        "AC-009: changed_fields must NOT contain 'jsm_internal' in the default \
         no-visibility-flag path (VP-577-026 variant 3); \
         got changed_fields: {:?}",
        parsed["changed_fields"]
    );
}

// ---------------------------------------------------------------------------
// AC-010 / VP-577-024
// BC-3.5.005 postcondition — PUT 404 → exit 64 with dual-line stderr
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body"` against a wiremock
/// returning `404 {"errorMessages":["Comment with id '10001' does not exist."]}`
/// → exit 64; stderr contains BOTH:
/// (a) preamble `"comment not found or permission denied"` AND
/// (b) Jira error text `"Comment with id '10001' does not exist."` on a SEPARATE line.
///
/// Mirrors VP-577-004 (delete 404 handling) applied to the PUT route.
/// 404 is NOT idempotent. No stdout output.
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_005_put_404_exits_64_with_dual_stderr() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["Comment with id '10001' does not exist."]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "edit", "FOO-1", "--id", "10001", "body"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-010: must exit 64 on PUT 404; 404 is NOT idempotent; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // BC-3.5.005 §Response 404 pins the full context form "comment not found or
    // permission denied: <KEY>#<ID>" — mirrors the delete handler at interactions.rs:225.
    assert!(
        stderr.contains("comment not found or permission denied: FOO-1#10001"),
        "AC-010: stderr must contain full preamble \
         'comment not found or permission denied: FOO-1#10001' \
         (BC-3.5.005 §Response 404 verbatim pin); got: {stderr}"
    );
    assert!(
        stderr.contains("Comment with id '10001' does not exist."),
        "AC-010: stderr must contain Jira error body text \
         'Comment with id '10001' does not exist.' on a separate line; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Mutation-kill AC-013 — non-404/403 API error propagates as exit 1
// BC-3.5.005 — kills guard `*status == 404 || *status == 403` → `true` mutant
//              and `== 403` → `!= 403` mutant (interactions.rs:357:64, :357:90)
// ---------------------------------------------------------------------------

/// Verify that a 500 Internal Server Error from the PUT endpoint exits 1
/// (not 64) and does NOT emit the "comment not found or permission denied"
/// preamble.
///
/// This test kills two guard mutations in the 404/403 re-wrap block:
/// 1. replacing the guard with `true` — ANY ApiError would get exit 64 + preamble
/// 2. replacing `== 403` with `!= 403` — 500 would match `500 != 403` = true → exit 64
///
/// With the correct guard, 500 is neither 404 nor 403, so the error propagates
/// as-is through `Err(e)`, which JrError maps to exit 1 (ApiError exit code).
///
/// Coverage-additive: SHOULD pass against current code (500 propagates as exit 1).
#[tokio::test]
async fn test_bc_3_5_005_edit_500_exits_1_not_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "edit", "FOO-1", "--id", "10001", "body"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-3.5.005 500-guard: 500 error must exit 1 (not 64); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.contains("comment not found or permission denied"),
        "BC-3.5.005 500-guard: 500 error must NOT emit the 404/403 preamble; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-012 / BC-3.5.009 markdown body source
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "**bold**" --markdown --output json`
/// → exit 0; AND wire-inspect: the PUT ADF contains a "strong" mark on the text node,
/// proving `markdown_to_adf` was called (not `text_to_adf`).
///
/// `text_to_adf("**bold**")` would emit a literal text node with text "**bold**" and no marks.
/// `markdown_to_adf("**bold**")` emits text "bold" with a `{"type":"strong"}` mark.
/// This difference kills the if/else converter-swap mutant on the markdown fork.
///
/// Coverage-additive (not red-first): MUST pass against current code.
#[tokio::test]
async fn test_bc_3_5_009_edit_markdown_source() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "**bold**",
            "--markdown",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "AC-012: must exit 0 on --markdown happy path; \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    // Wire-inspect: ADF must have a "strong" mark on the text node (markdown_to_adf path).
    // text_to_adf("**bold**") emits literal text "**bold**" with NO marks — the two
    // converters produce distinguishable ADF, which kills the converter-swap mutant.
    let reqs = server.received_requests().await.unwrap();
    assert_eq!(
        reqs.len(),
        1,
        "AC-012: expected exactly 1 PUT request; got {}",
        reqs.len()
    );

    let put_body: Value =
        serde_json::from_slice(&reqs[0].body).expect("AC-012: PUT request body must be valid JSON");

    // markdown_to_adf("**bold**") → doc → paragraph → text "bold" + strong mark
    let text_node = &put_body["body"]["content"][0]["content"][0];

    let has_strong = text_node["marks"]
        .as_array()
        .map(|marks| marks.iter().any(|m| m["type"].as_str() == Some("strong")))
        .unwrap_or(false);
    assert!(
        has_strong,
        "AC-012: PUT ADF text node must have a 'strong' mark when --markdown is used \
         (proves markdown_to_adf was called, not text_to_adf which emits literal '**bold**'); \
         text node: {text_node}\nfull PUT body: {put_body}"
    );

    // Secondary differentiator: markdown path strips the ** delimiters from the text
    assert_eq!(
        text_node["text"].as_str(),
        Some("bold"),
        "AC-012: text node text must be 'bold' (markdown parsed), not '**bold**' (literal); \
         full PUT body: {put_body}"
    );
}

// ---------------------------------------------------------------------------
// AC-011
// BC-3.5.009 top-level rule — no body source → exit 64
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001` (no body source) →
/// exit 64; stderr contains `"body is required"`; wiremock PUT has `.expect(0)`
/// (no HTTP PUT call made).
///
/// Red Gate: fails because `handle_comment_edit` is `todo!()` → exit 101.
#[tokio::test]
async fn test_bc_3_5_009_no_body_source_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "edit", "FOO-1", "--id", "10001"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-011: must exit 64 when no body source provided \
         (BC-3.5.009 top-level rule); got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("body is required"),
        "AC-011: stderr must contain 'body is required'; got: {stderr}"
    );
}

// ===========================================================================
// S-577-5 TESTS — 13 functions covering 12 ACs (AC-010 has 2 variants)
//
// Red Gate: AC-001..009, AC-012 fail because visibility branches are absent
// (internal: _, public: _, yes: _ ignored in current handle_comment_edit).
// Green pre-satisfied: AC-010 v1, AC-010 v2, AC-011.
// ===========================================================================

// ---------------------------------------------------------------------------
// S-577-5 AC-001 / VP-577-002
// BC-3.5.006 postcondition — --internal adds properties array to PUT body
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --internal --output json`
/// against a wiremock → exit 0; inspect PUT wire body:
/// - `"properties"` key present (VP-577-002 clause b)
/// - `properties` array has exactly one element:
///   `{"key":"sd.public.comment","value":{"internal":true}}` (VP-577-002 clause c)
/// - Top-level PUT body key-set == `{"body","properties"}` (no "visibility" key)
///
/// Red Gate: fails because current code ignores `--internal` → sends only
/// `{"body": adf}` → `"properties"` key absent → assertion panics.
#[tokio::test]
async fn test_bc_3_5_006_internal_puts_properties_true() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "body",
            "--internal",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "S-577-5 AC-001: must exit 0 on --internal PUT; \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(
        reqs.len(),
        1,
        "S-577-5 AC-001: expected exactly 1 PUT request; got {}",
        reqs.len()
    );

    let put_body: Value = serde_json::from_slice(&reqs[0].body)
        .expect("S-577-5 AC-001: PUT request body must be valid JSON");

    // VP-577-002 clause (b): "properties" key must be present
    let props = put_body.get("properties").unwrap_or_else(|| {
        panic!(
            "S-577-5 AC-001 VP-577-002(b): PUT body must contain 'properties' key \
             when --internal is passed; got body: {put_body}"
        )
    });

    // VP-577-002 clause (c): properties[0] shape
    let props_arr = props
        .as_array()
        .expect("S-577-5 AC-001: 'properties' must be a JSON array");
    assert_eq!(
        props_arr.len(),
        1,
        "S-577-5 AC-001: 'properties' array must have exactly 1 element; got: {props_arr:?}"
    );
    assert_eq!(
        props_arr[0]["key"].as_str(),
        Some("sd.public.comment"),
        "S-577-5 AC-001: properties[0].key must be 'sd.public.comment'; got: {:?}",
        props_arr[0]["key"]
    );
    assert_eq!(
        props_arr[0]["value"]["internal"].as_bool(),
        Some(true),
        "S-577-5 AC-001: properties[0].value.internal must be boolean true \
         for --internal; got: {:?}",
        props_arr[0]["value"]["internal"]
    );

    // VP-577-002 top-level key-set: exactly {"body","properties"}, no "visibility"
    let top_keys: BTreeSet<&str> = put_body
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        top_keys,
        BTreeSet::from(["body", "properties"]),
        "S-577-5 AC-001 VP-577-002: PUT body top-level key-set must be exactly \
         {{\"body\",\"properties\"}}; got: {top_keys:?}"
    );
    assert!(
        put_body.get("visibility").is_none(),
        "S-577-5 AC-001: PUT body must NOT contain 'visibility' key \
         (wrong Jira endpoint field); got: {put_body}"
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-002 / VP-577-026 variant 1
// BC-3.5.006 postcondition — changed_fields.jsm_internal is boolean true for --internal
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --internal --output json`
/// → exit 0; parsed `changed_fields` contains key `"jsm_internal"` with boolean `true`;
/// AND `changed_fields` key-set == `{"body","jsm_internal"}` (exact; VP-577-026 variant 1).
///
/// Red Gate: fails because current code omits `jsm_internal` from changed_fields
/// (the visibility response-building branch is not yet implemented).
#[tokio::test]
async fn test_bc_3_5_006_changed_fields_jsm_internal_true() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "body",
            "--internal",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "S-577-5 AC-002: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("S-577-5 AC-002: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
    });

    assert_eq!(
        parsed["changed_fields"]["jsm_internal"].as_bool(),
        Some(true),
        "S-577-5 AC-002 VP-577-026(v1): changed_fields.jsm_internal must be boolean true \
         for --internal; got: {:?}",
        parsed["changed_fields"]["jsm_internal"]
    );

    let cf_keys: BTreeSet<&str> = parsed["changed_fields"]
        .as_object()
        .expect("S-577-5 AC-002: changed_fields must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        cf_keys,
        BTreeSet::from(["body", "jsm_internal"]),
        "S-577-5 AC-002 VP-577-026(v1): changed_fields key-set must be exactly \
         {{\"body\",\"jsm_internal\"}}; got: {cf_keys:?}"
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-003 / VP-577-003
// BC-3.5.007 postcondition — --public adds properties internal:false to PUT
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --public --yes --output json`
/// against a wiremock → exit 0; PUT wire body:
/// - `"properties"` key present (VP-577-003 clause b)
/// - `properties[0]` == `{"key":"sd.public.comment","value":{"internal":false}}` (clause c)
/// - Top-level PUT body key-set == `{"body","properties"}`; no "visibility" key
///
/// Red Gate: fails because current code ignores --public → sends only `{"body": adf}`.
#[tokio::test]
async fn test_bc_3_5_007_public_puts_properties_false() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "body", "--public", "--yes",
            "--output", "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "S-577-5 AC-003: must exit 0 on --public --yes PUT; \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(
        reqs.len(),
        1,
        "S-577-5 AC-003: expected exactly 1 PUT request; got {}",
        reqs.len()
    );

    let put_body: Value = serde_json::from_slice(&reqs[0].body)
        .expect("S-577-5 AC-003: PUT request body must be valid JSON");

    let props = put_body.get("properties").unwrap_or_else(|| {
        panic!(
            "S-577-5 AC-003 VP-577-003(b): PUT body must contain 'properties' key \
             when --public; got body: {put_body}"
        )
    });

    let props_arr = props
        .as_array()
        .expect("S-577-5 AC-003: 'properties' must be an array");
    assert_eq!(
        props_arr.len(),
        1,
        "S-577-5 AC-003: 'properties' array must have exactly 1 element; got: {props_arr:?}"
    );
    assert_eq!(
        props_arr[0]["key"].as_str(),
        Some("sd.public.comment"),
        "S-577-5 AC-003: properties[0].key must be 'sd.public.comment'; got: {:?}",
        props_arr[0]["key"]
    );
    assert_eq!(
        props_arr[0]["value"]["internal"].as_bool(),
        Some(false),
        "S-577-5 AC-003: properties[0].value.internal must be boolean false for --public; \
         got: {:?}",
        props_arr[0]["value"]["internal"]
    );

    let top_keys: BTreeSet<&str> = put_body
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        top_keys,
        BTreeSet::from(["body", "properties"]),
        "S-577-5 AC-003 VP-577-003: PUT body top-level key-set must be exactly \
         {{\"body\",\"properties\"}}; got: {top_keys:?}"
    );
    assert!(
        put_body.get("visibility").is_none(),
        "S-577-5 AC-003: PUT body must NOT contain 'visibility' key; got: {put_body}"
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-004 / VP-577-026 variant 2
// BC-3.5.007 postcondition — changed_fields.jsm_internal is boolean false for --public+yes
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --public --yes --output json`
/// → exit 0; `changed_fields.jsm_internal` is boolean `false` (NOT missing, NOT null,
/// NOT string "false"); changed_fields key-set == `{"body","jsm_internal"}` (VP-577-026 v2).
///
/// Red Gate: fails because current code omits `jsm_internal` from changed_fields.
#[tokio::test]
async fn test_bc_3_5_007_changed_fields_jsm_internal_false() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "body", "--public", "--yes",
            "--output", "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "S-577-5 AC-004: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("S-577-5 AC-004: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
    });

    // VP-577-026 variant 2: jsm_internal must be boolean false (not missing, not null,
    // not the string "false" — as_bool() returns None for non-boolean JSON values)
    assert_eq!(
        parsed["changed_fields"]["jsm_internal"].as_bool(),
        Some(false),
        "S-577-5 AC-004 VP-577-026(v2): changed_fields.jsm_internal must be boolean false \
         for --public --yes (VP-577-029 exact pin — NOT a missing key, NOT null, NOT \"false\"); \
         got: {:?}",
        parsed["changed_fields"]["jsm_internal"]
    );

    let cf_keys: BTreeSet<&str> = parsed["changed_fields"]
        .as_object()
        .expect("S-577-5 AC-004: changed_fields must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        cf_keys,
        BTreeSet::from(["body", "jsm_internal"]),
        "S-577-5 AC-004 VP-577-026(v2): changed_fields key-set must be exactly \
         {{\"body\",\"jsm_internal\"}}; got: {cf_keys:?}"
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-005 / VP-577-025 variant 1
// BC-3.5.006 EC-3.5.006-1 — JSDCLOUD-6050 hint fires before PUT on --internal path
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --internal` (table mode) →
/// exit 0; stderr contains both:
/// - `"JSDCLOUD-6050"` (VP-577-025 variant 1 load-bearing substring)
/// - `"(marked internal)"` (BC-3.5.005 human-channel echo marker)
///
/// Note: hint fires to stderr even with `--output json`; `"(marked internal)"` is
/// table-mode-only (print_success is not called under --output json).
///
/// Red Gate: fails because current code ignores --internal → neither substring emitted.
#[tokio::test]
async fn test_bc_3_5_006_jsdcloud_hint_appears_on_internal() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "body",
            "--internal",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "S-577-5 AC-005: must exit 0 on --internal table-mode path; \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stderr.contains("JSDCLOUD-6050"),
        "S-577-5 AC-005 VP-577-025(v1): stderr must contain 'JSDCLOUD-6050' \
         on --internal path (load-bearing substring); got: {stderr}"
    );
    assert!(
        stderr.contains("(marked internal)"),
        "S-577-5 AC-005 VP-577-025(v1): stderr must contain '(marked internal)' \
         (BC-3.5.005 human-channel echo marker for --internal table-mode path); got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-006 / VP-577-025 variant 2
// BC-3.5.007 EC-3.5.007-1 + EC-3.5.008-1 — JSDCLOUD-6050 hint fires after --yes bypass
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --public --yes` (table mode) →
/// exit 0; stderr contains both:
/// - `"JSDCLOUD-6050"` (VP-577-025 variant 2; also proves EC-3.5.008-1:
///   `--yes` does NOT suppress the JSDCLOUD-6050 hint)
/// - `"(marked public)"` (BC-3.5.005 human-channel echo marker for --public path)
///
/// Timing: hint fires AFTER confirmation bypass (--yes), AFTER ADF conversion, BEFORE PUT.
///
/// Red Gate: fails because current code ignores --public → neither substring emitted.
#[tokio::test]
async fn test_bc_3_5_007_jsdcloud_hint_appears_on_public_yes() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "body", "--public", "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "S-577-5 AC-006: must exit 0 on --public --yes table-mode path; \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stderr.contains("JSDCLOUD-6050"),
        "S-577-5 AC-006 VP-577-025(v2): stderr must contain 'JSDCLOUD-6050' on \
         --public --yes path (proves EC-3.5.008-1: --yes does NOT suppress the hint); \
         got: {stderr}"
    );
    assert!(
        stderr.contains("(marked public)"),
        "S-577-5 AC-006 VP-577-025(v2): stderr must contain '(marked public)' \
         (BC-3.5.005 human-channel echo marker for --public path); got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-007 / VP-577-006
// BC-3.5.008 precondition — --public + --no-input without --yes → exit 64, no PUT
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --public --no-input`
/// (no `--yes`) → exit 64; stderr contains BOTH `"visibility to public"` AND `"--yes"`
/// (VP-577-006 dual-pin: proves exit originates from the step-3 --public gate, not the
/// step-2 body gate — the non-empty body `"body"` is required for this test to be
/// meaningful); wiremock PUT has `.expect(0)`.
///
/// Red Gate: fails because current code ignores --public → does body-only PUT → exit 0.
#[tokio::test]
async fn test_bc_3_5_008_public_no_input_without_yes_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "body",
            "--public",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "S-577-5 AC-007 VP-577-006: must exit 64 on --public --no-input without --yes \
         (confirmation gate fires — non-empty body proves exit is from --public gate, \
         not body gate); got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("visibility to public"),
        "S-577-5 AC-007 VP-577-006: stderr must contain 'visibility to public' \
         (dual-pin load-bearing substring); got: {stderr}"
    );
    assert!(
        stderr.contains("--yes"),
        "S-577-5 AC-007 VP-577-006: stderr must contain '--yes' \
         (hint to bypass with the confirmation flag); got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-008 / VP-577-029
// BC-3.5.008 EC-3.5.008-2 — interactive N → exit 0, cancel envelope, no PUT
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --public --output json`
/// with `JR_STDIN_IS_TTY=1` and stdin fed `"N\n"` → exit 0; stdout JSON:
/// - Top-level key-set == `{"cancelled","updated"}` (exact; VP-577-029)
/// - `"cancelled"` == `true`
/// - `"updated"` == `false` (boolean false — VP-577-029 exact pin: NOT a timestamp or
///   empty string; `"id"` and `"key"` MUST NOT appear — not confirmed server-side)
/// - Wiremock PUT has `.expect(0)`
///
/// Red Gate: fails because current code ignores --public → does body-only PUT →
/// returns `{"changed_fields":{"body":"body"},"id":"10001","key":"FOO-1","updated":true}`,
/// which has wrong key-set and wrong "updated" semantics.
#[tokio::test]
async fn test_bc_3_5_008_public_interactive_cancel_json_key_set() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "body", "--public", "--output",
            "json",
        ])
        .write_stdin("N\n")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "S-577-5 AC-008 VP-577-029: must exit 0 on interactive cancel (N); \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("S-577-5 AC-008: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
    });

    let top_keys: BTreeSet<&str> = parsed
        .as_object()
        .expect("S-577-5 AC-008: stdout must be a JSON object")
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        top_keys,
        BTreeSet::from(["cancelled", "updated"]),
        "S-577-5 AC-008 VP-577-029: cancel envelope key-set must be exactly \
         {{\"cancelled\",\"updated\"}} (no 'id' or 'key' — not confirmed server-side \
         on cancel path); got: {top_keys:?}\nfull stdout: {stdout}"
    );

    assert_eq!(
        parsed["cancelled"].as_bool(),
        Some(true),
        "S-577-5 AC-008 VP-577-029: 'cancelled' must be boolean true; got: {:?}",
        parsed["cancelled"]
    );
    assert_eq!(
        parsed["updated"].as_bool(),
        Some(false),
        "S-577-5 AC-008 VP-577-029: 'updated' must be boolean false (VP-577-029 exact \
         pin — NOT a timestamp or empty string); got: {:?}",
        parsed["updated"]
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-009 / VP-577-017, EC-3.5.008-3
// BC-3.5.008 EC-3.5.008-3 — --stdin flag implies no_input gate; TTY-agnostic
// ---------------------------------------------------------------------------

/// Verify EC-3.5.008-3: `--stdin` flag-based no_input mutation is TTY-agnostic.
///
/// Both variants are in one test function (VP-577-017 prescriptive-rule pin):
///
/// **Variant 1 (pipe stdin, no JR_STDIN_IS_TTY):**
/// stdin is a pipe → main.rs auto-sets no_input=true; `--stdin --public` (no `--yes`) →
/// exit 64; stderr contains BOTH `"--stdin"` AND `"--yes"` (dual-pin).
///
/// **Variant 2 (JR_STDIN_IS_TTY=1, seam active — prescribed-rule pin):**
/// Same flags but JR_STDIN_IS_TTY=1 suppresses the main.rs auto-flip → STILL exit 64;
/// same stderr assertions. Proves the `--stdin` flag-based branch fires INDEPENDENTLY
/// of TTY-detection state per EC-3.5.008-3.
///
/// Setup: stdin MUST contain a NON-EMPTY body (so EC-3.5.009-5 body-empty guard passes;
/// the EC-3.5.008-3 targeted message fires at step-3, not as a handler-start short-circuit).
///
/// Red Gate: fails because current code ignores --public → reads body from stdin,
/// does body-only PUT → exit 0.
#[tokio::test]
async fn test_bc_3_5_008_ec3_stdin_without_yes_public_exits_64() {
    // --- Variant 1: pipe stdin, no JR_STDIN_IS_TTY ---
    let server1 = MockServer::start().await;
    let cache_dir1 = tempfile::tempdir().unwrap();
    let config_dir1 = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server1)
        .await;

    let output1 = jr_cmd(&server1.uri(), cache_dir1.path(), config_dir1.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "--stdin", "--public",
        ])
        .write_stdin("body")
        .output()
        .unwrap();

    let stderr1 = String::from_utf8_lossy(&output1.stderr);

    assert_eq!(
        output1.status.code(),
        Some(64),
        "S-577-5 AC-009 VP-577-017(v1): must exit 64 on --stdin --public without --yes \
         (pipe stdin, no JR_STDIN_IS_TTY); got {:?}\nstderr: {stderr1}",
        output1.status.code()
    );
    assert!(
        stderr1.contains("--stdin"),
        "S-577-5 AC-009 VP-577-017(v1): stderr must contain '--stdin' \
         (EC-3.5.008-3 targeted hint load-bearing substring); got: {stderr1}"
    );
    assert!(
        stderr1.contains("--yes"),
        "S-577-5 AC-009 VP-577-017(v1): stderr must contain '--yes' \
         (EC-3.5.008-3 targeted hint load-bearing substring); got: {stderr1}"
    );

    // --- Variant 2: JR_STDIN_IS_TTY=1 — proves EC-3.5.008-3 is TTY-agnostic ---
    let server2 = MockServer::start().await;
    let cache_dir2 = tempfile::tempdir().unwrap();
    let config_dir2 = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server2)
        .await;

    let output2 = jr_cmd(&server2.uri(), cache_dir2.path(), config_dir2.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "--stdin", "--public",
        ])
        .write_stdin("body")
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&output2.stderr);

    assert_eq!(
        output2.status.code(),
        Some(64),
        "S-577-5 AC-009 VP-577-017(v2): must STILL exit 64 on --stdin --public without \
         --yes when JR_STDIN_IS_TTY=1 (proves EC-3.5.008-3 fires independently of TTY \
         detection state); got {:?}\nstderr: {stderr2}",
        output2.status.code()
    );
    assert!(
        stderr2.contains("--stdin"),
        "S-577-5 AC-009 VP-577-017(v2): stderr must contain '--stdin' \
         (EC-3.5.008-3 targeted hint, TTY-agnostic); got: {stderr2}"
    );
    assert!(
        stderr2.contains("--yes"),
        "S-577-5 AC-009 VP-577-017(v2): stderr must contain '--yes' \
         (EC-3.5.008-3 targeted hint, TTY-agnostic); got: {stderr2}"
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-010 / VP-577-028, EC-3.5.008-4
// BC-3.5.008 EC-3.5.008-4 — --yes without --public is silent no-op
// PRE-SATISFIED by S-577-4 (body-only path) — regression assertion, GREEN
// ---------------------------------------------------------------------------

/// **Variant 1 (VP-577-028 v1 — silent no-op success):** [PRE-SATISFIED — GREEN]
/// `jr issue comment edit FOO-1 --id 10001 "body" --yes --output json` (no `--public`) →
/// exit 0; body-only PUT succeeds; `--yes` is a silent no-op; no error, no clap rejection.
/// Clap MUST NOT have `requires("public")` on `--yes`.
///
/// S-577-4's body-only path handles this: `yes: _` in destructure = clap accepts `--yes`
/// without `requires` enforcement. Regression assertion — must stay GREEN.
#[tokio::test]
async fn test_bc_3_5_008_ec4_yes_without_public_is_silent_noop() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "body", "--yes", "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "S-577-5 AC-010(v1) VP-577-028: must exit 0 — --yes without --public is a silent \
         no-op; clap MUST NOT have requires(\"public\") on --yes \
         (EC-3.5.008-4, DEC-169); got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
}

/// **Variant 2 (VP-577-028 v2 — runtime clap-requires probe):** [PRE-SATISFIED — GREEN]
/// `jr issue comment edit FOO-1 --id 10001 "" --yes` (empty body, no `--public`) →
/// exit 64 (NOT exit 2); stderr contains `"comment body cannot be empty"`.
///
/// The exit-64-vs-2 discrimination is the operative test signal:
/// - exit 2 = clap rejected with `requires("public")` BEFORE the empty-body guard fires
/// - exit 64 = handler's EC-3.5.009-5 guard fires (proves `requires("public")` is absent)
///
/// EC-3.5.009-5 empty-body guard already exits 64. Regression assertion — must stay GREEN.
#[tokio::test]
async fn test_bc_3_5_008_ec4_yes_without_public_runtime_probe_exit64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "", "--yes",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "S-577-5 AC-010(v2) VP-577-028: must exit 64 (EC-3.5.009-5 empty-body guard), \
         NOT exit 2 (clap requires(\"public\") is forbidden on --yes); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("comment body cannot be empty"),
        "S-577-5 AC-010(v2) VP-577-028: stderr must contain 'comment body cannot be empty' \
         (proves handler-level guard fires, not clap-level requires rejection); got: {stderr}"
    );
    assert_ne!(
        output.status.code(),
        Some(2),
        "S-577-5 AC-010(v2) VP-577-028: must NOT exit 2 — exit 2 would indicate clap \
         requires(\"public\") is present on --yes, which is forbidden (EC-3.5.008-4); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-011 / VP-577-010, BC-3.5.011
// clap conflicts_with: --internal + --public → exit 2
// PRE-SATISFIED by S-577-1 (conflicts_with added) — regression assertion, GREEN
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --internal --public`
/// → exit 2 (clap error); stderr contains `"cannot be used with"`
/// (VP-577-010 discriminator for the clap conflicts_with error message).
///
/// S-577-1 added `conflicts_with` on `--internal`/`--public`. Regression assertion —
/// must stay GREEN.
#[tokio::test]
async fn test_bc_3_5_011_internal_and_public_clap_exit_2() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Clap exits before any handler code runs → no PUT expected
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "body",
            "--internal",
            "--public",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(2),
        "S-577-5 AC-011 VP-577-010: must exit 2 (clap conflicts_with on \
         --internal + --public, BC-3.5.011); got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("cannot be used with"),
        "S-577-5 AC-011 VP-577-010: stderr must contain 'cannot be used with' \
         (clap error message discriminator for conflicts_with); got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// S-577-5 AC-012 / VP-577-030 variant 2
// BC-3.5.008 EC-3.5.008-5 — EOF during --public interactive prompt → exit 130
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --public`
/// with `JR_STDIN_IS_TTY=1` and stdin fed EOF (empty write_stdin) → exit 130
/// (`JrError::Interrupted`); wiremock PUT has `.expect(0)`.
///
/// Body is from positional arg `"body"` (not stdin), so step-2 body resolution
/// completes. The interactive path then attempts `io::stdin().lock().read_line()` →
/// Ok(0) (EOF) → `JrError::Interrupted` → exit 130 (EC-3.5.008-5).
///
/// Red Gate: fails because current code ignores --public → does body-only PUT → exit 0.
#[tokio::test]
async fn test_bc_3_5_008_ec5_public_prompt_eof_exits_130() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "body", "--public",
        ])
        .write_stdin("") // empty stdin = immediate EOF on read_line
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(130),
        "S-577-5 AC-012 VP-577-030(v2): must exit 130 (JrError::Interrupted) on EOF \
         during --public interactive prompt (EC-3.5.008-5: Ok(0) from read_line → \
         JrError::Interrupted); got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// Mutation-kill AC-008 — interactive "y" / "yes" confirm → PUT proceeds
// BC-3.5.008 — kills `answer != "y" && answer != "yes"` → `||` mutant
// ---------------------------------------------------------------------------

/// Verify that `jr issue comment edit FOO-1 --id 10001 "body" --public --output json`
/// with `JR_STDIN_IS_TTY=1` and stdin fed `"y\n"` confirms the visibility change,
/// calls the HTTP PUT endpoint exactly once with
/// `properties[0].value.internal == false`, and exits 0 with
/// `changed_fields.jsm_internal == false` and `updated == true` in the JSON output.
///
/// This test kills the `&&` → `||` mutation at the answer-check condition in
/// `interactions.rs::handle_comment_edit` step 3b:
/// if `||` is used instead of `&&`, "y" would trigger the cancel path
/// (`"y" != "y" || "y" != "yes"` = `false || true` = true), so the PUT
/// endpoint would receive 0 calls instead of 1.
///
/// Also asserts stderr carries the confirmation prompt and the JSDCLOUD-6050
/// hint (the hint fires on all confirmed `--internal`/`--public` paths, not on
/// the cancel path, so its presence proves the confirmed branch was taken).
///
/// Variant 2 (`"yes\n"`) kills the `|| answer != "yes"` arm of the same mutant.
#[tokio::test]
async fn test_bc_3_5_008_public_interactive_yes_proceeds() {
    // --- Variant 1: "y\n" confirms and causes PUT to fire ---
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "body", "--public", "--output",
            "json",
        ])
        .write_stdin("y\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(0),
        "S-577-5 AC-008 mutant-kill: interactive 'y' must exit 0 after PUT 200; \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    // Prompt must appear in stderr.
    assert!(
        stderr.contains("This will set the comment's visibility to public."),
        "S-577-5 AC-008: stderr must contain the prompt text; got: {stderr}"
    );

    // JSDCLOUD-6050 hint must appear in stderr (fires on confirmed --public paths;
    // NOT emitted on cancel path — confirms the y-branch was actually taken).
    assert!(
        stderr.contains("JSDCLOUD-6050"),
        "S-577-5 AC-008: stderr must contain JSDCLOUD-6050 hint after confirmed 'y'; \
         got: {stderr}"
    );

    // PUT must have fired with properties[0].value.internal == false.
    let reqs = server.received_requests().await.unwrap();
    assert_eq!(
        reqs.len(),
        1,
        "S-577-5 AC-008: expected exactly 1 PUT request after 'y'; got {}",
        reqs.len()
    );
    let put_body: Value =
        serde_json::from_slice(&reqs[0].body).expect("PUT request body must be valid JSON");
    let props = put_body
        .get("properties")
        .and_then(Value::as_array)
        .expect("S-577-5 AC-008: PUT body must contain 'properties' array for confirmed --public");
    assert_eq!(
        props[0]["value"]["internal"].as_bool(),
        Some(false),
        "S-577-5 AC-008: properties[0].value.internal must be boolean false \
         for confirmed --public; got: {:?}",
        props[0]["value"]["internal"]
    );

    // JSON output must carry updated:true and jsm_internal:false.
    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("S-577-5 AC-008: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
    });
    assert_eq!(
        parsed["updated"].as_bool(),
        Some(true),
        "S-577-5 AC-008: 'updated' must be boolean true after confirmed PUT; \
         got: {:?}",
        parsed["updated"]
    );
    assert_eq!(
        parsed["changed_fields"]["jsm_internal"].as_bool(),
        Some(false),
        "S-577-5 AC-008: changed_fields.jsm_internal must be boolean false \
         for confirmed --public; got: {:?}",
        parsed["changed_fields"]["jsm_internal"]
    );

    // --- Variant 2: "yes\n" also proceeds (kills the `|| answer != "yes"` arm mutant) ---
    let server2 = MockServer::start().await;
    let cache_dir2 = tempfile::tempdir().unwrap();
    let config_dir2 = tempfile::tempdir().unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(minimal_comment_response()))
        .expect(1)
        .mount(&server2)
        .await;

    let output2 = jr_cmd(&server2.uri(), cache_dir2.path(), config_dir2.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "body", "--public", "--output",
            "json",
        ])
        .write_stdin("yes\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    assert_eq!(
        output2.status.code(),
        Some(0),
        "S-577-5 AC-008 mutant-kill: interactive 'yes' must also exit 0 after PUT 200 \
         (kills the `|| answer != \"yes\"` arm mutant); \
         got {:?}\nstderr: {stderr2}\nstdout: {stdout2}",
        output2.status.code()
    );
    let reqs2 = server2.received_requests().await.unwrap();
    assert_eq!(
        reqs2.len(),
        1,
        "S-577-5 AC-008: expected exactly 1 PUT request after 'yes'; got {}",
        reqs2.len()
    );
}
