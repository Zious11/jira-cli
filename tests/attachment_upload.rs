//! CLI-level integration tests for `jr issue attachment upload`.
//!
//! RED GATE: all tests in this file FAIL because `handle_attachment_upload`,
//! `upload_attachments`, and `delete_attachment` contain `todo!()` — the spawned
//! subprocess exits 101 (Rust panic) instead of the expected exit codes and output.
//!
//! After Task 4/5 (handler + API implementation), all tests become GREEN.
//!
//! BC anchors: BC-3.9.001, BC-3.9.002, BC-3.9.009, BC-3.9.012, BC-3.9.014,
//!             BC-3.9.017, BC-3.9.018, BC-3.9.020
//! VPs: VP-576-003 (DELETE-before-POST ordering), VP-576-004 (curated JSON shape)
//! Security: SEC-576-004 (CWE-93 Content-Disposition CRLF injection guard)
//! Story: S-576-3, GitHub issue #576

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// VP-576-004 direct unit test imports (P74-001 — pub fn + pub fields obligation)
use jr::api::jira::attachments::AttachmentObject;
use jr::cli::issue::attachments::serialize_attachment_curated;

// ---------------------------------------------------------------------------
// Harness helper
// ---------------------------------------------------------------------------

fn jr_cmd_with_xdg(
    server_uri: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache_dir)
        .env("JR_CONFIG_DIR", config_dir);
    cmd
}

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

/// Raw Jira attachment object (as returned by the API — includes "self" and
/// raw "content" fields that serialize_attachment_curated will curate away).
fn make_upload_attachment(id: &str, filename: &str) -> Value {
    serde_json::json!({
        "id": id,
        "filename": filename,
        "self": format!("https://example.atlassian.net/rest/api/3/attachment/{id}"),
        "content": format!("https://example.atlassian.net/rest/api/3/attachment/content/{id}"),
        "created": "2026-07-20T00:00:00.000+0000",
        "size": 4096_u64,
        "mimeType": "text/plain",
        "author": {
            "accountId": "user123",
            "displayName": "Test User",
            "self": "https://example.atlassian.net/rest/api/3/user?accountId=user123",
            "avatarUrls": { "48x48": "https://example.atlassian.net/avatar/user123" },
            "accountType": "atlassian"
        }
    })
}

/// Wire-format issue response for the list endpoint used by --replace-existing.
fn issue_with_attachments(key: &str, attachments: Vec<Value>) -> Value {
    serde_json::json!({
        "key": key,
        "fields": {
            "attachment": attachments
        }
    })
}

// ---------------------------------------------------------------------------
// AC-001: X-Atlassian-Token: no-check mandatory header (BC-3.9.001)
// ---------------------------------------------------------------------------

/// Verifies that `upload_attachments` sends `X-Atlassian-Token: no-check` on
/// the POST request and that the upload succeeds with that header present.
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_001_multipart_post_x_atlassian_token_mandatory() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf content").unwrap();

    // Mount: POST with X-Atlassian-Token: no-check header required.
    // .expect(1) asserts the mock is matched exactly once (header must be sent).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("10001", "report.pdf")
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.001: upload with X-Atlassian-Token must exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

/// Verifies that stdin `-` as a FILE argument is rejected before any HTTP call
/// (EC-3.9.001-6 canonical rejection string, verbatim).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 64" fails.
#[tokio::test]
async fn test_bc_3_9_001_stdin_rejected() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // POST must NOT be called — all pre-HTTP checks fire before HTTP.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args(["issue", "attachment", "upload", "TEST-1", "-"])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "EC-3.9.001-6: stdin '-' must exit 64; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("stdin upload is not supported; provide a file path."),
        "EC-3.9.001-6 VERBATIM: stderr must contain canonical rejection string; got: {stderr}"
    );
}

/// Verifies that on a 429 rate-limit response, the retry path correctly
/// rebuilds the multipart request from a fresh tokio::fs::File::open
/// (ADR-0017 constraint: Request::try_clone() returns None for multipart).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_001_rate_limit_retry_rebuilds_request() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("retry_test.txt");
    std::fs::write(&file, b"retry content").unwrap();

    // First request: 429 Retry-After: 0 (immediate retry).
    // Register 200 mock FIRST, then 429 (wiremock: most recently registered wins).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("10001", "retry_test.txt")
        ])))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(15))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "ADR-0017 retry: 429 → rebuilt multipart → 200; must exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

/// Verifies that all file pre-checks (is_file() / existence) fire BEFORE any
/// HTTP call (EC-3.9.001-4 / BC-3.9.001).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 64" fails.
#[tokio::test]
async fn test_bc_3_9_001_file_prechecks_before_http() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // HTTP must not fire — pre-check fires before any HTTP call.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let nonexistent = "/nonexistent/__s576_3_precheck.txt";
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args(["issue", "attachment", "upload", "TEST-1", nonexistent])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "EC-3.9.001-4: non-existent file must exit 64; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("file not found:"),
        "EC-3.9.001-4: stderr must contain 'file not found:'; got: {stderr}"
    );
    // Canonical form: "file not found: <path>" — no HTTP must fire (checked by .expect(0) above).

    // ----- P1-002: Directory argument → EC-3.9.001-4 "not a regular file: <path>" -----
    // BC-3.9.001 mandates a distinct pre-HTTP check: is_file() must reject directories with
    // the exact message "not a regular file: <path>" (EC-3.9.001-4), not re-use the
    // "file not found:" path.
    // RED Gate: todo!() → exit 101; assert exit 64 fails immediately.
    // When implemented naively with tokio::fs::File::open(dir), the OS error varies by platform
    // and leaks as "file not found:"; these assertions are RED until the correct is_file()
    // pre-check is added that emits "not a regular file: <path>".
    {
        let dir_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&dir_server)
            .await;

        // Use an existing TempDir as the directory argument.
        let dir_arg = TempDir::new().unwrap();
        let dir_path_str = dir_arg.path().to_string_lossy().to_string();

        let dir_output = jr_cmd_with_xdg(&dir_server.uri(), cache.path(), config.path())
            .args(["issue", "attachment", "upload", "TEST-1", &dir_path_str])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();

        let dir_stderr = String::from_utf8_lossy(&dir_output.stderr);
        assert_eq!(
            dir_output.status.code(),
            Some(64),
            "EC-3.9.001-4 directory: passing a directory must exit 64; \
             got {:?}\nstderr: {dir_stderr}",
            dir_output.status.code()
        );
        assert!(
            dir_stderr.contains("not a regular file:"),
            "EC-3.9.001-4 VERBATIM: stderr must contain 'not a regular file:' for a \
             directory argument; got: {dir_stderr}"
        );
        // HTTP must NOT have fired (verified by .expect(0) on dir_server above).
    }
}

/// Verifies that the human-readable table output echoes each uploaded
/// attachment's filename, size, ID, and created columns (BC-3.9.001 table mode).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_001_human_table_display() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf data").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "id": "10099",
                "filename": "report.pdf",
                "self": "https://example.atlassian.net/rest/api/3/attachment/10099",
                "content": "https://example.atlassian.net/rest/api/3/attachment/content/10099",
                "created": "2026-07-20T00:00:00.000+0000",
                "size": 8_u64,
                "mimeType": "application/pdf",
                "author": { "accountId": "user123", "displayName": "Test User" }
            }])),
        )
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            // no --output json → human table mode
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.001 table: upload must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // 4-column table: Filename / Size / ID / Created
    assert!(
        stdout.contains("report.pdf"),
        "BC-3.9.001 table: stdout must contain filename; got: {stdout}"
    );
    assert!(
        stdout.contains("10099"),
        "BC-3.9.001 table: stdout must contain attachment ID; got: {stdout}"
    );
}

/// Verifies that multiple files are sent as separate `file`-named parts in a
/// SINGLE multipart POST request (EC-3.9.001-2 — one POST regardless of count).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_001_multi_file_single_multipart_post() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file_a = tmp.path().join("file_a.txt");
    let file_b = tmp.path().join("file_b.txt");
    std::fs::write(&file_a, b"content a").unwrap();
    std::fs::write(&file_b, b"content b").unwrap();

    // EC-3.9.001-2: exactly one POST even with two files.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("10001", "file_a.txt"),
            make_upload_attachment("10002", "file_b.txt"),
        ])))
        .expect(1) // exactly one POST for two files
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file_a.to_string_lossy(),
            &file_b.to_string_lossy(),
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "EC-3.9.001-2: two-file upload must exit 0 (one POST); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-016: JSM no-flag upload uses platform POST (BC-3.9.002)
// ---------------------------------------------------------------------------

/// Verifies that `jr issue attachment upload EJ-1 FILE` without --public or
/// --internal uses the platform POST endpoint and zero servicedeskapi calls
/// are issued (BC-3.9.002).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_002_jsm_no_flag_uses_platform_post_zero_servicedeskapi_calls() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("jsm_test.txt");
    std::fs::write(&file, b"jsm content").unwrap();

    // Platform POST endpoint — the one that must be called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/EJ-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("20001", "jsm_test.txt")
        ])))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "EJ-1",
            &file.to_string_lossy(),
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.002: JSM upload without --public/--internal must exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // Zero servicedeskapi calls — verify via request journal.
    let received = server.received_requests().await.unwrap();
    let servicedeskapi_calls: Vec<_> = received
        .iter()
        .filter(|r| r.url.path().contains("servicedeskapi"))
        .collect();
    assert!(
        servicedeskapi_calls.is_empty(),
        "BC-3.9.002: zero servicedeskapi calls expected; got {} calls: {:?}",
        servicedeskapi_calls.len(),
        servicedeskapi_calls
            .iter()
            .map(|r| r.url.as_str())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// AC-003/004: JSON output curated shape (BC-3.9.009 / VP-576-004)
// ---------------------------------------------------------------------------

/// Verifies that `--output json` upload response omits `self` and renames
/// `content` → `contentUrl` in each attachment object (BC-3.9.009).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_009_upload_json_shape_self_omitted_content_renamed() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("shape_test.pdf");
    std::fs::write(&file, b"pdf content").unwrap();

    // Raw Jira response includes "self" and "content" — curated output must strip/rename.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("10042", "shape_test.pdf")
        ])))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.009: upload --output json must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // These JSON shape assertions are reached only when GREEN.
    let parsed: Vec<Value> = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("BC-3.9.009: stdout must be JSON array: {e}\nstdout: {stdout}"));
    assert!(
        !parsed.is_empty(),
        "BC-3.9.009: parsed array must not be empty"
    );
    for elem in &parsed {
        let map = elem.as_object().expect("each element must be an object");
        assert!(
            !map.contains_key("self"),
            "BC-3.9.009: 'self' key must be omitted from curated output; got: {elem}"
        );
        assert!(
            !map.contains_key("content"),
            "BC-3.9.009: raw 'content' key must be absent (renamed to 'contentUrl'); got: {elem}"
        );
        assert!(
            map.contains_key("contentUrl"),
            "BC-3.9.009: 'contentUrl' key must be present; got: {elem}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-011: Error taxonomy (BC-3.9.012)
// ---------------------------------------------------------------------------

/// Full error taxonomy for upload. Nine sub-assertions, each in its own scoped
/// block with a fresh MockServer. The first failing assertion (exit 101 ≠ 64)
/// marks the test RED at Red Gate.
///
/// RED Gate: todo!() → subprocess exits 101; first assertion "exit 64" fails.
#[tokio::test]
async fn test_bc_3_9_012_error_taxonomy() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let existing_file = tmp.path().join("tax_upload.txt");
    std::fs::write(&existing_file, b"tax data").unwrap();

    // ----- (1) file not found → exit 64 -----
    {
        let server = MockServer::start().await;
        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                "/nonexistent/__s576_3_tax_99.txt",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.9.012(1) file not found → exit 64; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("file not found:"),
            "BC-3.9.012(1) must contain 'file not found:'; got: {stderr}"
        );
    }

    // ----- (2) stdin '-' → exit 64 + verbatim canonical message -----
    {
        let server = MockServer::start().await;
        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args(["issue", "attachment", "upload", "TEST-1", "-"])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.9.012(2) stdin '-' → exit 64; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("stdin upload is not supported; provide a file path."),
            "BC-3.9.012(2) EC-3.9.001-6 VERBATIM: must contain canonical message; got: {stderr}"
        );
    }

    // ----- (3) KEY 404 → exit 64 -----
    {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &existing_file.to_string_lossy(),
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-3.9.012(3) 404 KEY → exit 64; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
    }

    // ----- (4) 401 → exit 2, stderr contains "Not authenticated" AND "jr auth login" -----
    {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(401).set_body_json(
                serde_json::json!({"errorMessages": ["Your session has expired."], "errors": {}}),
            ))
            .mount(&server)
            .await;
        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &existing_file.to_string_lossy(),
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(2),
            "BC-3.9.012(4) 401 → exit 2; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("Not authenticated"),
            "BC-3.9.012(4) 401: stderr must contain 'Not authenticated'; got: {stderr}"
        );
        assert!(
            stderr.contains("jr auth login"),
            "BC-3.9.012(4) 401: stderr must contain 'jr auth login'; got: {stderr}"
        );
    }

    // ----- (5) 403 → exit 1 -----
    {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &existing_file.to_string_lossy(),
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "BC-3.9.012(5) 403 → exit 1; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
    }

    // ----- (6) 413 → exit 1, stderr contains verbatim "Attachment too large" message -----
    {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(413))
            .mount(&server)
            .await;
        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &existing_file.to_string_lossy(),
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "BC-3.9.012(6) 413 → exit 1; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("Attachment too large: the file exceeds the server-configured limit."),
            "BC-3.9.012(6) 413 VERBATIM: must contain canonical 413 message; got: {stderr}"
        );
    }

    // ----- (7) 400 generic → exit 1 -----
    {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(
                    serde_json::json!({"errorMessages": ["Bad request"], "errors": {}}),
                ),
            )
            .mount(&server)
            .await;
        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &existing_file.to_string_lossy(),
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "BC-3.9.012(7) 400 → exit 1; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
    }

    // ----- (8) 500 → exit 1, stderr contains "API error (" -----
    {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &existing_file.to_string_lossy(),
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "BC-3.9.012(8) 500 → exit 1; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("API error ("),
            "BC-3.9.012(8) 5xx: stderr must contain 'API error ('; got: {stderr}"
        );
    }

    // ----- (9) network error → exit 1, stderr contains "Could not reach" -----
    // Use port 1 (reserved, never listening) to force connection-refused error.
    {
        let output = jr_cmd_with_xdg("http://127.0.0.1:1", cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &existing_file.to_string_lossy(),
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "BC-3.9.012(9) network error → exit 1; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("Could not reach"),
            "BC-3.9.012(9) network: stderr must contain 'Could not reach'; got: {stderr}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-007: --replace-existing confirmation gate (BC-3.9.014)
// ---------------------------------------------------------------------------

/// Verifies that when the user confirms with 'y', the gate returns true and
/// the upload proceeds (DELETE + POST fire, exit 0).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_014_gate_confirm_proceeds() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf data").unwrap();

    // List endpoint: existing same-filename attachment.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments(
                "TEST-1",
                vec![make_upload_attachment("AID-001", "report.pdf")],
            )),
        )
        .mount(&server)
        .await;

    // DELETE the existing attachment.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // POST: upload new file.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("AID-NEW", "report.pdf")
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--replace-existing",
            "--output",
            "json",
        ])
        .write_stdin("y\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.014 gate confirm 'y': must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // P1-001: BC-3.9.014 consumer-2 VERBATIM prompt pins.
    // Implementation must emit the canonical prompt template (bc-3-issue-write.md ~line 3632):
    //   "Replace existing attachment(s) on <KEY>:\n  <filename> (id: <AID>)\nContinue? [y/N] "
    // Two-space indent, "(id: " form, NO dash prefix.
    // These assertions are RED until the implementation uses the exact canonical format.
    // (The naive implementation uses "About to delete N existing..." + "  - name (id)" form —
    // both header and entry assertions will fail against that format.)
    assert!(
        stderr.contains("Replace existing attachment(s) on TEST-1:"),
        "BC-3.9.014 consumer-2 VERBATIM header: stderr must contain \
         'Replace existing attachment(s) on TEST-1:'; got: {stderr}"
    );
    assert!(
        stderr.contains("  report.pdf (id: AID-001)"),
        "BC-3.9.014 consumer-2 VERBATIM entry: two-space indent, '(id: ' form, no dash prefix; \
         stderr must contain '  report.pdf (id: AID-001)'; got: {stderr}"
    );
}

/// Verifies that when the user inputs 'n' (anything other than y/yes), the
/// upload is cancelled: exit 0, stderr "Upload cancelled.",
/// JSON `{"cancelled":true,"uploaded":false}`, zero DELETE/POST calls.
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_014_gate_cancel_exits_0() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf data").unwrap();

    // List endpoint: existing same-filename attachment.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments(
                "TEST-1",
                vec![make_upload_attachment("AID-001", "report.pdf")],
            )),
        )
        .mount(&server)
        .await;

    // DELETE must NOT fire on cancel.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    // POST must NOT fire on cancel.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--replace-existing",
            "--output",
            "json",
        ])
        .write_stdin("n\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.014 cancel 'n': must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Upload cancelled."),
        "BC-3.9.014 cancel: stderr must contain 'Upload cancelled.'; got: {stderr}"
    );

    // JSON shape on cancel: {"cancelled": true, "uploaded": false}
    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("BC-3.9.014 cancel: stdout must be JSON: {e}\nstdout: {stdout}")
    });
    assert_eq!(
        parsed.get("cancelled"),
        Some(&Value::Bool(true)),
        "BC-3.9.014 cancel JSON: 'cancelled' must be true; got: {stdout}"
    );
    assert_eq!(
        parsed.get("uploaded"),
        Some(&Value::Bool(false)),
        "BC-3.9.014 cancel JSON: 'uploaded' must be false; got: {stdout}"
    );
}

/// Verifies that EOF on stdin triggers exit 130 (JrError::Interrupted).
/// Uses JR_STDIN_IS_TTY=1 seam to force interactive branch.
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 130" fails.
#[tokio::test]
async fn test_bc_3_9_014_gate_eof_exits_130() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf data").unwrap();

    // List endpoint: same-filename attachment.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments(
                "TEST-1",
                vec![make_upload_attachment("AID-001", "report.pdf")],
            )),
        )
        .mount(&server)
        .await;

    // Empty stdin = immediate EOF on read_line → JrError::Interrupted → exit 130.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .env("JR_STDIN_IS_TTY", "1")
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--replace-existing",
        ])
        .write_stdin("") // EOF immediately
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(130),
        "BC-3.9.014 EOF: must exit 130 (JrError::Interrupted); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

/// Verifies that --replace-existing without --yes in non-interactive mode
/// exits 64 with the canonical actionable message (BC-3.9.014 enforcement).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 64" fails.
#[tokio::test]
async fn test_bc_3_9_014_non_interactive_without_yes_exits_64() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf data").unwrap();

    // List endpoint: existing same-filename attachment.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments(
                "TEST-1",
                vec![make_upload_attachment("AID-001", "report.pdf")],
            )),
        )
        .mount(&server)
        .await;

    // DELETE must NOT fire — guard fires before DELETE.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    // Non-interactive: piped stdin without JR_STDIN_IS_TTY=1 auto-flips to --no-input.
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--replace-existing",
            // --yes intentionally absent
        ])
        // piped stdin (no JR_STDIN_IS_TTY=1) → auto-no-input
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-3.9.014 non-interactive: missing --yes must exit 64; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Use --yes to confirm deletion of existing same-filename attachments."),
        "BC-3.9.014 non-interactive VERBATIM: must contain canonical --yes hint; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-006: --replace-existing deletes then uploads (BC-3.9.017 / VP-576-003)
// ---------------------------------------------------------------------------

/// Verifies that --replace-existing with a same-filename match deletes first,
/// then uploads. Ordering is enforced via the wiremock request journal.
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_017_replace_existing_delete_then_post() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"new pdf").unwrap();

    // List: existing same-filename attachment.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments(
                "TEST-1",
                vec![make_upload_attachment("AID-001", "report.pdf")],
            )),
        )
        .mount(&server)
        .await;

    // DELETE the existing attachment.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-001"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    // POST new file.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("AID-NEW", "report.pdf")
        ])))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--replace-existing",
            "--yes",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.017: replace-existing must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // VP-576-003: DELETE must precede POST in the request journal.
    let received = server.received_requests().await.unwrap();
    let delete_pos = received
        .iter()
        .position(|r| r.method == wiremock::http::Method::DELETE);
    let post_pos = received
        .iter()
        .position(|r| r.method == wiremock::http::Method::POST);
    if let (Some(d), Some(p)) = (delete_pos, post_pos) {
        assert!(
            d < p,
            "VP-576-003: DELETE must precede POST; DELETE at {d}, POST at {p}"
        );
    }
    // If either is None, the test is already RED from the exit-code assertion.
}

// ---------------------------------------------------------------------------
// AC-012: --replace-existing with no match → direct upload (BC-3.9.018)
// ---------------------------------------------------------------------------

/// Verifies that --replace-existing when no same-filename attachment exists
/// performs a direct upload without any DELETE calls.
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_bc_3_9_018_replace_existing_no_match_direct_upload() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("new_file.txt");
    std::fs::write(&file, b"new content").unwrap();

    // List: DIFFERENT filename (no match for "new_file.txt").
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments(
                "TEST-1",
                vec![make_upload_attachment("AID-001", "OTHER_FILE.pdf")],
            )),
        )
        .mount(&server)
        .await;

    // DELETE must NOT fire — no matching filename.
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    // POST: direct upload proceeds.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("AID-NEW", "new_file.txt")
        ])))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--replace-existing",
            "--yes",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.018: no-match → direct upload must exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

// ---------------------------------------------------------------------------
// AC-008: --dry-run path-c (BC-3.9.020)
// ---------------------------------------------------------------------------

/// Dry-run path-c: list GET fires (mandatory for wouldDelete preview),
/// DELETE and POST are suppressed.
///
/// Sub-assertions:
/// (i) --dry-run --replace-existing → list GET fires; DELETE/POST suppressed;
///     wouldDelete populated; JSON shape verified.
/// (ii) EC-3.9.020-6 CRITICAL: --dry-run WITHOUT --replace-existing → exit 2
///      (clap `requires` annotation — fires BEFORE todo!() handler).
///
/// RED Gate: sub-assertion (i) is checked first; exits 101 ≠ 0 → RED.
/// Sub-assertion (ii) exits 2 via clap (before handler) → PASSES in RED Gate.
#[tokio::test]
async fn test_bc_3_9_020_dry_run_path_c_guards_not_suppressed_gates_suppressed() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"dry run test").unwrap();

    // ----- Sub-assertion (i): --dry-run --replace-existing -----
    {
        let server = MockServer::start().await;

        // List: mandatory read-only call for wouldDelete preview.
        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/TEST-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_with_attachments(
                    "TEST-1",
                    vec![make_upload_attachment("AID-001", "report.pdf")],
                )),
            )
            .expect(1) // mandatory list GET must fire
            .mount(&server)
            .await;

        // DELETE must NOT fire in dry-run.
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;

        // POST must NOT fire in dry-run.
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &file.to_string_lossy(),
                "--replace-existing",
                "--dry-run",
                "--output",
                "json",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-3.9.020(i): --dry-run --replace-existing must exit 0; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        // Dry-run JSON shape: {dryRun: true, wouldDelete: [...], wouldUpload: [...]}
        let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("BC-3.9.020(i): stdout must be JSON: {e}\nstdout: {stdout}")
        });
        assert_eq!(
            parsed.get("dryRun"),
            Some(&Value::Bool(true)),
            "BC-3.9.020(i): 'dryRun' must be true; got: {stdout}"
        );
        let would_delete = parsed
            .get("wouldDelete")
            .and_then(|v| v.as_array())
            .expect("BC-3.9.020(i): 'wouldDelete' must be array");
        assert!(
            !would_delete.is_empty(),
            "BC-3.9.020(i): wouldDelete must be non-empty (same-filename match); got: {stdout}"
        );
        // Verify the entry has the expected id and filename.
        let first = &would_delete[0];
        assert_eq!(
            first.get("id").and_then(|v| v.as_str()),
            Some("AID-001"),
            "BC-3.9.020(i): wouldDelete[0].id must be 'AID-001'; got: {first}"
        );
        assert_eq!(
            first.get("filename").and_then(|v| v.as_str()),
            Some("report.pdf"),
            "BC-3.9.020(i): wouldDelete[0].filename must be 'report.pdf'; got: {first}"
        );
    }

    // ----- Sub-assertion (ii): EC-3.9.020-6 — --dry-run alone → exit 2 -----
    // clap `#[arg(long, requires = "replace_existing")]` fires BEFORE the handler.
    // This assertion PASSES even at Red Gate (clap exits 2 before todo!()).
    {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &file.to_string_lossy(),
                "--dry-run",
                // --replace-existing intentionally absent
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(2),
            "EC-3.9.020-6: --dry-run without --replace-existing must exit 2 (clap); \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
    }
}

// ---------------------------------------------------------------------------
// VP-576-003: DELETE-before-POST ordering invariant
// ---------------------------------------------------------------------------

/// Property: across multiple same-filename attachments, every DELETE completes
/// before the first POST upload. Ordering validated via wiremock request journal.
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_vp_576_003_delete_before_post_ordering_invariant() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"new report").unwrap();

    // List: TWO existing same-filename attachments (JRACLOUD-96384 coexistence).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TEST-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments(
                "TEST-1",
                vec![
                    make_upload_attachment("AID-001", "report.pdf"),
                    make_upload_attachment("AID-002", "report.pdf"),
                ],
            )),
        )
        .mount(&server)
        .await;

    // DELETE both existing attachments.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-001"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-002"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    // POST new attachment.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("AID-NEW", "report.pdf")
        ])))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--replace-existing",
            "--yes",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "VP-576-003: two-delete replace-existing must exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // VP-576-003 ordering invariant: ALL DELETEs must precede the POST.
    let received = server.received_requests().await.unwrap();

    // Find the position of the POST.
    let post_pos = received
        .iter()
        .position(|r| r.method == wiremock::http::Method::POST);

    // Find positions of all DELETEs.
    let delete_positions: Vec<usize> = received
        .iter()
        .enumerate()
        .filter(|(_, r)| r.method == wiremock::http::Method::DELETE)
        .map(|(i, _)| i)
        .collect();

    if let Some(p) = post_pos {
        assert_eq!(
            delete_positions.len(),
            2,
            "VP-576-003: expected 2 DELETE requests; got {} in journal",
            delete_positions.len()
        );
        for &d in &delete_positions {
            assert!(
                d < p,
                "VP-576-003: DELETE at position {d} must precede POST at position {p}"
            );
        }
    }
    // If post_pos is None, the test is already RED from exit-code assertion.
}

// ---------------------------------------------------------------------------
// VP-576-004: curated JSON shape cross-path consistency
// ---------------------------------------------------------------------------

/// VP-576-004: verify that serialize_attachment_curated produces the correct
/// curated shape and that the upload path uses it (structurally identical to
/// the list path).
///
/// Part A: direct unit assertions on serialize_attachment_curated (may PASS
/// since S-576-1 implemented the function).
///
/// Part B: subprocess invocation ensures the test is RED at Red Gate — the
/// subprocess exits 101 (todo!() in handle_attachment_upload).
#[tokio::test]
async fn test_vp_576_004_curated_shape_upload_and_list_are_structurally_identical() {
    // ----- Part A: direct unit assertions (P74-001) -----

    // Full-author fixture: self, avatarUrls, accountType must be stripped.
    let full_author = serde_json::json!({
        "accountId": "user123",
        "displayName": "Alice Smith",
        "self": "https://example.atlassian.net/rest/api/3/user?accountId=user123",
        "avatarUrls": {"48x48": "https://example.atlassian.net/avatar/user123"},
        "accountType": "atlassian"
    });
    let obj_full = AttachmentObject {
        self_url: "https://example.atlassian.net/rest/api/3/attachment/10001".into(),
        id: "10001".into(),
        filename: "report.pdf".into(),
        author: Some(full_author),
        created: "2026-07-20T00:00:00.000+0000".into(),
        size: 4096,
        mime_type: Some("application/pdf".into()),
        content: "https://example.atlassian.net/rest/api/3/attachment/content/10001".into(),
    };

    let curated_full = serialize_attachment_curated(&obj_full);
    let map_full = curated_full.as_object().expect("curated must be an object");

    // BC-3.9.009 invariant: "self" OMITTED.
    assert!(
        !map_full.contains_key("self"),
        "VP-576-004: 'self' must be omitted from curated shape; got: {curated_full}"
    );
    // BC-3.9.009 invariant: "content" absent (renamed to "contentUrl").
    assert!(
        !map_full.contains_key("content"),
        "VP-576-004: raw 'content' key must be absent; got: {curated_full}"
    );
    // BC-3.9.009 invariant: "contentUrl" present.
    assert!(
        map_full.contains_key("contentUrl"),
        "VP-576-004: 'contentUrl' must be present; got: {curated_full}"
    );
    // Author curation: ONLY {accountId, displayName} — all other fields stripped.
    let author_out = map_full.get("author").expect("'author' key must exist");
    let author_obj = author_out.as_object().expect("author must be an object");
    assert_eq!(
        author_obj.get("accountId").and_then(|v| v.as_str()),
        Some("user123"),
        "VP-576-004: author.accountId must be preserved; got: {author_out}"
    );
    assert_eq!(
        author_obj.get("displayName").and_then(|v| v.as_str()),
        Some("Alice Smith"),
        "VP-576-004: author.displayName must be preserved; got: {author_out}"
    );
    assert!(
        !author_obj.contains_key("self"),
        "VP-576-004: author 'self' must be stripped; got: {author_out}"
    );
    assert!(
        !author_obj.contains_key("avatarUrls"),
        "VP-576-004: author 'avatarUrls' must be stripped; got: {author_out}"
    );
    assert!(
        !author_obj.contains_key("accountType"),
        "VP-576-004: author 'accountType' must be stripped; got: {author_out}"
    );
    assert_eq!(
        author_obj.len(),
        2,
        "VP-576-004: author must have exactly 2 keys {{accountId, displayName}}; \
         got {} keys: {:?}",
        author_obj.len(),
        author_obj.keys().collect::<Vec<_>>()
    );

    // Partial-author fixture: {accountId: null, displayName: null} in author object.
    // Curated output must be {"accountId": null, "displayName": null} — NOT null whole field.
    let partial_author = serde_json::json!({"accountId": null, "displayName": null});
    let obj_partial = AttachmentObject {
        self_url: "https://example.atlassian.net/rest/api/3/attachment/10002".into(),
        id: "10002".into(),
        filename: "doc.txt".into(),
        author: Some(partial_author),
        created: "2026-07-20T00:00:00.000+0000".into(),
        size: 100,
        mime_type: None,
        content: "https://example.atlassian.net/rest/api/3/attachment/content/10002".into(),
    };
    let curated_partial = serialize_attachment_curated(&obj_partial);
    let map_partial = curated_partial
        .as_object()
        .expect("partial curated must be object");
    let partial_author_out = map_partial.get("author").expect("'author' key must exist");
    assert!(
        partial_author_out.is_object(),
        "VP-576-004 partial-author: must be {{accountId:null,displayName:null}} NOT top-level null; \
         got: {partial_author_out}"
    );
    let partial_obj = partial_author_out.as_object().unwrap();
    assert!(
        partial_obj.get("accountId").is_some_and(|v| v.is_null()),
        "VP-576-004 partial-author: accountId must be null; got: {partial_author_out}"
    );
    assert!(
        partial_obj
            .get("displayName")
            .is_some_and(|v| v.is_null()),
        "VP-576-004 partial-author: displayName must be null; got: {partial_author_out}"
    );
    assert_eq!(
        partial_obj.len(),
        2,
        "VP-576-004 partial-author: must have exactly 2 keys; got {}: {:?}",
        partial_obj.len(),
        partial_obj.keys().collect::<Vec<_>>()
    );

    // ----- Part B: subprocess invocation (RED Gate anchor) -----
    // The upload handler is todo!() → exits 101 → assertion "exit 0" fails → RED.
    // This ensures VP-576-004 is RED even though Part A (direct calls) may pass.
    {
        let server = MockServer::start().await;
        let cache = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("vp004_test.txt");
        std::fs::write(&file, b"vp004 data").unwrap();

        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                make_upload_attachment("10001", "vp004_test.txt")
            ])))
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &file.to_string_lossy(),
                "--output",
                "json",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        // RED Gate anchor: subprocess exits 101 (todo!()) → this assertion FAILS.
        assert_eq!(
            output.status.code(),
            Some(0),
            "VP-576-004 Part B: upload --output json must exit 0 to validate \
             cross-path shape consistency; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        // Reached only when GREEN: verify upload JSON matches list curated shape.
        let stdout = String::from_utf8_lossy(&output.stdout);
        let uploaded: Vec<Value> = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("VP-576-004 Part B: stdout must be JSON array: {e}\nstdout: {stdout}")
        });
        assert!(
            !uploaded.is_empty(),
            "VP-576-004 Part B: uploaded array must not be empty"
        );
        for elem in &uploaded {
            let m = elem
                .as_object()
                .expect("each uploaded element must be object");
            assert!(
                !m.contains_key("self"),
                "VP-576-004 Part B: upload output must omit 'self'; got: {elem}"
            );
            assert!(
                m.contains_key("contentUrl"),
                "VP-576-004 Part B: upload output must have 'contentUrl'; got: {elem}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// AC-017: --public/--internal interim rejection (temporary until S-576-5)
// ---------------------------------------------------------------------------

/// Verifies that --public and --internal are rejected with exit 64 and the
/// verbatim interim-rejection message BEFORE any file pre-check or HTTP call.
/// Ordering proof: using a nonexistent file with --public → interim rejection
/// fires first; stderr must NOT contain "file not found:".
///
/// REMOVED-AT-S5: this guard and the test are removed when S-576-5 wires
/// actual JSM visibility behavior (attachment --public/--internal).
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 64" fails.
#[tokio::test]
async fn test_bc_3_9_001_public_internal_interim_rejection_exits_64() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // HTTP must not fire — interim rejection fires before file pre-check or HTTP.
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    // Use a nonexistent file to prove interim rejection fires BEFORE file pre-check.
    let nonexistent = "/nonexistent/__s576_3_interim_public.txt";
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            nonexistent,
            "--public",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-017 interim --public: must exit 64; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // Verbatim interim-rejection message (BC-3.9.001 / AC-017).
    assert!(
        stderr.contains(
            "--public and --internal are not yet supported. \
             JSM visibility will be shipped in a follow-on story."
        ),
        "AC-017 VERBATIM: stderr must contain canonical interim message; got: {stderr}"
    );
    // Ordering proof: "file not found:" must NOT appear (interim fires before file check).
    assert!(
        !stderr.contains("file not found:"),
        "AC-017 ordering: interim rejection must fire BEFORE file pre-check; \
         'file not found:' must not appear in stderr; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-018: SEC-576-004 CWE-93 Content-Disposition CRLF injection guard
// ---------------------------------------------------------------------------

/// Verifies that filenames containing semicolons, double-quotes, or DEL
/// (U+007F, cross-platform stand-in for CRLF per S-576-2 lesson B4/B5) do
/// not produce malformed Content-Disposition headers (SEC-576-004 / CWE-93).
///
/// Poison chars:
/// - ';'  : boundary separator injection
/// - '"'  : value delimiter injection (Unix only — Windows rejects in filename)
/// - '\x7F': DEL / control char (safe on all platforms; stands in for \r\n)
///
/// RED Gate: todo!() → subprocess exits 101; assertion "exit 0" fails.
#[tokio::test]
async fn test_sec_576_004_content_disposition_crlf_injection_guard() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();

    // Poison char 1: semicolon (valid on all platforms).
    let file_semi = tmp.path().join("file;name.txt");
    std::fs::write(&file_semi, b"semicolon content").unwrap();

    // Poison char 2: DEL (U+007F) — cross-platform CWE-93 vector per S-576-2 B4/B5.
    // CRLF (\r\n) is rejected by Windows runner filesystem; DEL is the safe stand-in.
    let file_del = tmp.path().join("file\x7fname.txt");
    std::fs::write(&file_del, b"del content").unwrap();

    // ----- Sub-assertion: semicolon in filename -----
    {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                make_upload_attachment("10301", "file;name.txt")
            ])))
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &file_semi.to_string_lossy(),
                "--output",
                "json",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(0),
            "SEC-576-004(semicolon): upload with semicolon filename must exit 0; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        // Content-Disposition structure check (reached only when GREEN).
        // Note: the CRLF/NUL injection vectors are unit-covered in src/api/jira/attachments.rs
        // (safe_name maps \r, \n, \0 → '_'; test_sec_576_004_safe_name_*).
        // Creating CRLF-named files on Windows CI is impossible, so integration coverage
        // is limited to platform-safe vectors (semicolon, DEL).
        let received = server.received_requests().await.unwrap();
        for req in &received {
            if req.method == wiremock::http::Method::POST {
                let body = std::str::from_utf8(&req.body).unwrap_or("");
                // (a) Exactly one Content-Disposition for a single-file upload.
                //     A semicolon that breaks the quoted-string parameter would manifest
                //     as a malformed part structure; this count is the structural smoke signal.
                let cd_count = body.matches("Content-Disposition").count();
                assert_eq!(
                    cd_count, 1,
                    "SEC-576-004(semicolon): expected 1 Content-Disposition in multipart body; \
                     body excerpt: {}",
                    &body[..body.len().min(400)]
                );
                // (b) The literal filename appears in the body (reqwest properly quotes it).
                assert!(
                    body.contains("file;name.txt"),
                    "SEC-576-004(semicolon): filename must appear verbatim in multipart body; \
                     body excerpt: {}",
                    &body[..body.len().min(400)]
                );
            }
        }
    }

    // ----- Sub-assertion: DEL char in filename -----
    {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-1/attachments"))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                make_upload_attachment("10302", "file\x7fname.txt")
            ])))
            .mount(&server)
            .await;

        let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
            .args([
                "issue",
                "attachment",
                "upload",
                "TEST-1",
                &file_del.to_string_lossy(),
                "--output",
                "json",
            ])
            .timeout(std::time::Duration::from_secs(10))
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(0),
            "SEC-576-004(DEL): upload with DEL-char filename must exit 0; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        // Content-Disposition structure check (reached only when GREEN).
        // CRLF/NUL injection is unit-covered in src/api/jira/attachments.rs
        // (safe_name maps \r, \n, \0 → '_'; see test_sec_576_004_safe_name_*).
        // This integration pass verifies DEL (0x7F) — which safe_name does NOT map —
        // does not cause multipart boundary corruption (DEL passes through as-is or is
        // encoded by reqwest at the Part level).
        let received = server.received_requests().await.unwrap();
        for req in &received {
            if req.method == wiremock::http::Method::POST {
                let body = std::str::from_utf8(&req.body).unwrap_or("");
                // Exactly one Content-Disposition for a single-file upload.
                // Corruption from an unhandled control char would manifest here
                // (e.g., doubled header lines or missing part boundary).
                let cd_count = body.matches("Content-Disposition").count();
                assert_eq!(
                    cd_count, 1,
                    "SEC-576-004(DEL): expected 1 Content-Disposition in multipart body; \
                     body excerpt: {}",
                    &body[..body.len().min(400)]
                );
            }
        }
    }
}
