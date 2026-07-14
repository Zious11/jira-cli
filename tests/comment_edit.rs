//! CLI-level integration tests for `jr issue comment edit`.
//!
//! Red Gate: all tests FAIL because `handle_comment_edit` is `todo!()`.
//! Every subprocess exits 101 (Rust panic/todo!() exit code) instead of the
//! expected exit codes — exit 0 (success), exit 64 (user error).
//!
//! BC anchors: BC-3.5.005, BC-3.5.009
//! VPs: VP-577-001, VP-577-011, VP-577-012, VP-577-022(b), VP-577-023,
//!      VP-577-024, VP-577-026 (variant 3)
//! Story: S-577-4, GitHub issue #577

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
