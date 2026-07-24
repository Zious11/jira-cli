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

/// Persistent 429 across all retry attempts terminates with exit 1.
///
/// MAX_RETRIES = 3 → loop `for attempt in 0..=MAX_RETRIES` → 4 total POST attempts.
/// `.expect(4)` pins the exact count, killing:
///   - MAX_RETRIES off-by-one mutants (2 or 4 instead of 3)
///   - Exclusive-range mutant (`0..MAX_RETRIES` = 3 attempts instead of 4)
///   - Boundary-operator mutant (`attempt <= MAX_RETRIES` = 5 attempts instead of 4)
///
/// GREEN pin (impl's `!status.is_success()` fallthrough on exhaustion is correct;
/// `JrError::ApiError { status: 429 }` → exit 1).
#[tokio::test]
async fn test_bc_3_9_001_persistent_429_exhausts_retries() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf content").unwrap();

    // All 4 attempts return 429.  Retry-After: 0 avoids actual sleeps in tests.
    // .expect(4) is the mutation-killing assertion: any change to MAX_RETRIES or the
    // loop bound changes the request count and fails wiremock's expectation on drop.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_string("Rate limit exceeded."),
        )
        .expect(4)
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
    // Exhausted retries → JrError::ApiError { status: 429, ... } → exit 1.
    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-3.9.012: persistent 429 terminal failure must exit 1; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // wiremock .expect(4) validates exactly 4 attempts on server drop.
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

/// P4-003: gate accepts uppercase "Y" — case-insensitive match.
///
/// BC-3.9.014 mandates `eq_ignore_ascii_case("y")` / `eq_ignore_ascii_case("yes")`.
/// This test kills the `eq_ignore_ascii_case → ==` mutant (which would reject "Y").
///
/// GREEN pin (impl is correct).
#[tokio::test]
async fn test_bc_3_9_014_gate_confirm_uppercase_y_proceeds() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf data").unwrap();

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

    // Both DELETE and POST must fire — uppercase "Y" must proceed like "y".
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

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
        .write_stdin("Y\n")
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.014: uppercase 'Y' must be accepted (eq_ignore_ascii_case); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // .expect(1) on DELETE and POST verify the flow proceeded (not cancelled).
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
    let d = delete_pos.expect("VP-576-003: DELETE must be present in request journal");
    let p = post_pos.expect("VP-576-003: POST must be present in request journal");
    assert!(
        d < p,
        "VP-576-003: DELETE must precede POST; DELETE at {d}, POST at {p}"
    );
}

// ---------------------------------------------------------------------------
// EC-3.9.017-4: DELETE 404 on --replace-existing is a benign skip (BC-3.9.017)
// ---------------------------------------------------------------------------

/// A 404 response on DELETE during --replace-existing is a benign skip
/// (the attachment was already deleted by a concurrent actor); the handler
/// MUST continue to the next DELETE and to the POST upload.
///
/// EC-3.9.017-4 / AC-006 step 4.
///
/// RED Gate: current implementation propagates the 404 as exit 64, halting
/// before the second DELETE and the POST.
#[tokio::test]
async fn test_bc_3_9_017_delete_404_is_benign_skip() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"new report").unwrap();

    // List: TWO same-filename attachments (JRACLOUD-96384 coexistence).
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

    // FIRST DELETE: returns 404 (already-deleted; concurrent race).
    // .expect(1): the impl must CALL this DELETE exactly once and treat the
    // 404 as a benign skip — it must NOT abort on 404.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-001"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    // SECOND DELETE: returns 204 (nominal). Must still fire after the 404.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-002"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // POST must fire after BOTH deletes (including the 404-returning one).
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
        "EC-3.9.017-4: DELETE-404 must be treated as benign skip; flow must continue \
         to second DELETE and POST (exit 0); got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // VP-576-003 hard ordering: all DELETEs must precede POST.
    let received = server.received_requests().await.unwrap();
    let post_pos = received
        .iter()
        .position(|r| r.method == wiremock::http::Method::POST)
        .expect("VP-576-003: POST must be present in request journal (EC-3.9.017-4 flow)");
    let delete_positions: Vec<usize> = received
        .iter()
        .enumerate()
        .filter(|(_, r)| r.method == wiremock::http::Method::DELETE)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        delete_positions.len(),
        2,
        "EC-3.9.017-4: both DELETEs must fire (the 404 does not suppress the second); \
         got {} DELETE(s) in journal",
        delete_positions.len()
    );
    for &d in &delete_positions {
        assert!(
            d < post_pos,
            "VP-576-003: DELETE at {d} must precede POST at {post_pos}"
        );
    }
    // wiremock .expect(1) on both DELETE mocks and .expect(1) on POST enforces
    // that each fires exactly once (verified at server drop).
}

/// P4-002: non-404 DELETE error during --replace-existing must ABORT the flow.
///
/// EC-3.9.017-4: DELETE 404 = benign skip (tested separately). Any other HTTP
/// error (403, 5xx) is a hard abort — `delete_attachment` returns `Err(e)` which
/// propagates up through the upload handler.
///
/// Fixture determinism: GET returns [AID-001, AID-002] in that order; the
/// delete loop iterates `would_delete` in GET-response order, so AID-001 is
/// attempted first. AID-001 → 403 → abort immediately; AID-002 DELETE must not
/// fire; POST must not fire.
///
/// `.expect(0)` on AID-002 DELETE and POST kills:
///   - the `return Err(e)` deletion mutant (which would ignore the error and continue)
///   - the `!is_benign_404 → true/false` boundary mutants
///
/// GREEN pin (implementation is correct).
#[tokio::test]
async fn test_bc_3_9_017_delete_403_aborts_flow() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf data").unwrap();

    // GET returns two attachments both named "report.pdf"; loop processes AID-001 first.
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

    // AID-001 DELETE returns 403 — non-benign error triggers abort.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-001"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    // AID-002 DELETE must NOT fire after the 403 abort.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/attachment/AID-002"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    // POST (upload) must NOT fire if DELETE aborted.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
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
        Some(1),
        "EC-3.9.017-4: DELETE 403 must abort with exit 1 (ApiError); \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    // .expect(0) on AID-002 DELETE and POST are verified at MockServer drop.
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

/// Mutation pre-empt: table dry-run arm (OutputFormat::Table) exact-string pins.
///
/// BC-3.9.020 / `dry_run_upload` `OutputFormat::Table` arm:
///   1. "DRY RUN — no changes will be made."  (U+2014 em-dash)
///   2. "Would delete N existing attachment(s)."  (N ≥ 2)
///   3. "Would upload N file(s)."
///
/// `.expect(0)` on DELETE and POST ensures no network I/O occurs in dry-run.
/// The assertions kill:
///   - any mutation that alters or removes the em-dash (em-dash vs hyphen)
///   - any mutation that changes N in "Would delete N" (off-by-one on count)
///   - any mutation that omits the "Would upload" line
///
/// GREEN pin (implementation is correct).
#[tokio::test]
async fn test_bc_3_9_020_dry_run_table_output_strings() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("report.pdf");
    std::fs::write(&file, b"pdf data").unwrap();

    // Two attachments with same filename — "Would delete 2 existing attachment(s)."
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

    // No DELETE or POST should fire in dry-run mode.
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&server)
        .await;

    // No --output json → default Table output channel.
    // --yes suppresses the interactive gate (dry-run flag suppresses it anyway,
    // but --yes prevents needing JR_STDIN_IS_TTY and .write_stdin()).
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file.to_string_lossy(),
            "--replace-existing",
            "--dry-run",
            "--yes",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.020: table dry-run must exit 0; got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );

    // Exact string pin — em-dash U+2014, not hyphen-minus U+002D.
    assert!(
        stdout.contains("DRY RUN \u{2014} no changes will be made."),
        "BC-3.9.020: stdout must contain 'DRY RUN \u{2014} no changes will be made.' (em-dash); \
         got: {stdout}"
    );

    // Count pin: N=2 (two same-filename attachments in GET fixture).
    assert!(
        stdout.contains("Would delete 2 existing attachment(s)."),
        "BC-3.9.020: stdout must contain 'Would delete 2 existing attachment(s).'; got: {stdout}"
    );

    // Upload count pin: N=1 (one file argument).
    assert!(
        stdout.contains("Would upload 1 file(s)."),
        "BC-3.9.020: stdout must contain 'Would upload 1 file(s).'; got: {stdout}"
    );
    // .expect(0) on DELETE and POST mocks are verified at MockServer drop.
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

    let p = post_pos.expect("VP-576-003: POST must be present in request journal");
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
        partial_obj.get("displayName").is_some_and(|v| v.is_null()),
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

// AC-017: removed at S-576-5 — --public/--internal now routes to the JSM visibility
// handler instead of the interim rejection guard. See tests/attachment_jsm.rs for
// AC-001 through AC-016 coverage of the new behavior.

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
                    cd_count,
                    1,
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
                    cd_count,
                    1,
                    "SEC-576-004(DEL): expected 1 Content-Disposition in multipart body; \
                     body excerpt: {}",
                    &body[..body.len().min(400)]
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Mutation-kill tests: retry-boundary comparisons in upload_attachments
//
// Surviving mutants (cargo-mutants, PR #635 diff scope):
//   Line 257: `if delay > MAX_RETRY_AFTER_SECS`
//     - replace > with == (kills Test 3 below — delay=61, abort is expected;
//       mutant sees 61==60 = false → proceeds to sleep 61s → timeout fires)
//     - replace > with >= (kills Test 2 below — delay=60, proceed expected;
//       mutant sees 60>=60 = true → aborts immediately → assertion fails)
//   Line 267: `if delay > 0 { tokio::time::sleep(...).await }`
//     - replace > with == / < / >= : all produce indistinguishable behavior
//       when delay=0 (sleep(0) ≈ no-op on tokio). Test 1 documents the
//       boundary path but these three are genuinely equivalent mutants for
//       delay=0 and cannot be killed via black-box subprocess testing without
//       timing assertions or a src-level seam.
// ---------------------------------------------------------------------------

/// BC-3.9.001 retry boundary: Retry-After:0 proceeds and retries immediately (no sleep).
///
/// Mutation-kill targets (line 267: `if delay > 0 { sleep }`):
///   - `replace > with ==`: `if delay == 0 { sleep }` — sleep(0) is a no-op for u64
///   - `replace > with <`:  `if delay < 0 { sleep }` — never fires for u64 (equivalent)
///   - `replace > with >=`: `if delay >= 0 { sleep }` — always true for u64 (equivalent)
///
/// Note: all three line-267 mutations are behaviorally equivalent for `u64 delay`:
/// sleep(Duration::ZERO) is a near-instant no-op and `u64 >= 0` is always true.
/// This test documents the boundary and kills any abort-on-zero regression (line 257
/// path) but cannot distinguish equivalent sleep-vs-no-sleep u64 variants.
///
/// Design: subprocess via `jr_cmd_with_xdg` (proven pattern — same as
/// `test_bc_3_9_001_rate_limit_retry_rebuilds_request`). Wiremock priority:
/// most-recently-registered-wins — register 200 first, 429 second so 429 (higher
/// priority) serves the initial request; once exhausted, 200 serves the retry.
/// `.expect(1)` on the 429 mock is a drop-time guard that fires if jr never hit
/// the 429 path (which would mean the test is testing the wrong code path).
#[tokio::test]
async fn test_bc_3_9_001_rate_limit_retry_after_zero_skips_sleep() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("zero_delay_test.txt");
    std::fs::write(&file, b"zero delay content").unwrap();

    // Register 429 FIRST so it is served on the initial request; once its
    // up_to_n_times(1) capacity is exhausted, the second-registered 200 mock
    // serves the retry.  Empirically, wiremock 0.6 matches in registration order
    // (first-registered wins when multiple mocks match the same request), so the
    // 429 mock must be registered before the 200 fallback.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("10501", "zero_delay_test.txt")
        ])))
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

    // wiremock .expect(1) on the 429 mock fires at drop if the 429 path was never hit.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-3.9.001 delay=0: Retry-After:0 must not abort (delay 0 <= cap 60) and \
         must retry immediately (delay=0 skips sleep) → exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

/// BC-3.9.001 retry boundary: Retry-After equal to MAX_RETRY_AFTER_SECS should
/// proceed (not abort).
///
/// Mutation-kill target: `replace > with >=` at line 257 of
/// `src/api/jira/attachments.rs`.
///
/// The comparison is `if delay > MAX_RETRY_AFTER_SECS` (currently 60). When
/// delay == 60:
/// - Original (`> 60`): 60 > 60 = false → does NOT abort → starts sleeping 60s
/// - `>=` mutant:       60 >= 60 = true  → ABORTS immediately → returns ApiError
///
/// Design: uses `JiraClient::new_for_test` directly (no subprocess) with a
/// `tokio::time::timeout(5s)` outer gate. The original code sleeps 60 real seconds
/// before retrying — the 5-second timeout fires first, causing `result.is_err()`
/// (the task is still sleeping, not aborted). The `>=` mutant aborts immediately
/// → `result.is_ok()` with inner Err → assertion `result.is_err()` FAILS → mutant
/// KILLED.
///
/// Note: `start_paused = true` is intentionally absent — incompatible with wiremock
/// (see `tests/rate_limit_cap_tests.rs::ac_001_retry_after_exceeds_cap_aborts_retry`
/// for the known incompatibility). This test intentionally waits up to 5 real
/// seconds.
#[tokio::test]
async fn test_bc_3_9_001_rate_limit_retry_after_at_cap_proceeds() {
    use jr::api::client::JiraClient;
    use jr::api::rate_limit::MAX_RETRY_AFTER_SECS;

    let server = MockServer::start().await;
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("cap_boundary_test.txt");
    std::fs::write(&file, b"cap boundary content").unwrap();

    // First request: 429 with Retry-After exactly equal to MAX_RETRY_AFTER_SECS (60).
    // Original code: delay=60, `60 > 60` = false → proceeds, sleeps 60s then retries.
    // `>=` mutant:   delay=60, `60 >= 60` = true → aborts → returns ApiError.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", MAX_RETRY_AFTER_SECS.to_string().as_str()),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Success response reachable only after the 60s sleep elapses (never in this test
    // because the 5-second timeout fires first — the point is to demonstrate the code
    // is sleeping, not aborting).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("10502", "cap_boundary_test.txt")
        ])))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let file_path = file.to_path_buf();

    // Spawn `upload_attachments` as a task so we can apply an outer timeout.
    let handle =
        tokio::spawn(async move { client.upload_attachments("TEST-1", &[file_path]).await });

    // 5-second wall-clock gate.
    //   Original code: sleep(60s) → timeout fires at ~5s → result.is_err() ✓
    //   `>=` mutant:   aborts immediately → result.is_ok() with inner Err → assertion ✗
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;

    assert!(
        result.is_err(),
        "BC-3.9.001 at-cap boundary: Retry-After:{cap} (== MAX_RETRY_AFTER_SECS) must \
         NOT abort — code must sleep and then retry. Original code sleeps {cap}s, so a \
         5s timeout must fire first (result.is_err()). If result.is_ok(), the `>= {cap}` \
         mutant is active and is aborting when it should proceed.",
        cap = MAX_RETRY_AFTER_SECS
    );
}

/// BC-3.9.001 retry boundary: Retry-After one above MAX_RETRY_AFTER_SECS aborts.
///
/// Mutation-kill target: `replace > with ==` at line 257 of
/// `src/api/jira/attachments.rs`.
///
/// The comparison is `if delay > MAX_RETRY_AFTER_SECS` (currently 60). With delay=61:
/// - Original (`> 60`): 61 > 60 = true  → ABORTS immediately → ApiError → exit 1
/// - `==` mutant:       61 == 60 = false → does NOT abort → sleeps 61s → timeout
///   kills subprocess at 15s → exit signal (None) → assertion `Some(1)` FAILS → KILLED
///
/// The `.expect(1)` on the mock pins that exactly 1 POST was made (the initial request
/// that got 429 — no retry after abort).
#[tokio::test]
async fn test_bc_3_9_001_rate_limit_retry_after_above_cap_aborts() {
    use jr::api::rate_limit::MAX_RETRY_AFTER_SECS;

    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("above_cap_test.txt");
    std::fs::write(&file, b"above cap content").unwrap();

    // ONE mock: 429 with Retry-After one above the cap (MAX_RETRY_AFTER_SECS + 1 = 61).
    // Original code: 61 > 60 = true → aborts → only 1 request.
    // `==` mutant:   61 == 60 = false → proceeds → sleeps 61s → subprocess killed by
    //                15s timeout → exit code is signal (None), not Some(1) → test fails.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(429).insert_header(
            "Retry-After",
            (MAX_RETRY_AFTER_SECS + 1).to_string().as_str(),
        ))
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
        .timeout(std::time::Duration::from_secs(15))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-3.9.001 above-cap: Retry-After:{above} (> MAX_RETRY_AFTER_SECS={cap}) \
         must abort immediately and exit 1; got {:?}\nstderr: {stderr}",
        output.status.code(),
        above = MAX_RETRY_AFTER_SECS + 1,
        cap = MAX_RETRY_AFTER_SECS
    );

    // Error message must describe the cap exceedance (not generic).
    assert!(
        stderr.contains("cap") || stderr.contains("exceeds") || stderr.contains("Retry-After"),
        "BC-3.9.001 above-cap: stderr must mention cap/exceeds/Retry-After; got: {stderr}"
    );
    // wiremock .expect(1) validates exactly 1 POST on drop (abort = no retry).
}

// ---------------------------------------------------------------------------
// AC-018: double-quote filename → Content-Disposition well-formed (Unix only)
// ---------------------------------------------------------------------------

/// AC-018 double-quote vector — regression pin for reqwest Content-Disposition encoding.
///
/// Unix-only: `"` is a valid filename character on Unix; Windows rejects it at the
/// OS/filesystem level before `jr` sees the file.
///
/// Verifies:
/// (a) Exactly one Content-Disposition header (no split at the `"` boundary).
/// (b) The escaped form appears in the multipart body — reqwest 0.13 uses RFC 2616/7230
///     backslash-escape (`\"`) within the quoted-string, producing `filename="file\"name.txt"`.
/// (c) The raw broken form `filename="file"name` does NOT appear (a bare `"` would end
///     the quoted-string prematurely and could inject arbitrary header content).
///
/// Expected GREEN (reqwest 0.13 escapes `"` as `\"` inside the filename quoted-string).
/// A reqwest encoding regression (raw `"` emitted unescaped) would cause (b) and (c) to fail.
#[cfg(unix)]
#[tokio::test]
async fn test_ac_018_double_quote_filename_well_formed_content_disposition() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();

    // `"` is valid on Unix but forbidden on Windows — #[cfg(unix)] gates this block.
    let file_quote = tmp.path().join("file\"name.txt");
    std::fs::write(&file_quote, b"quote content").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("10401", "file\"name.txt")
        ])))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file_quote.to_string_lossy(),
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
        "AC-018 double-quote filename: upload must exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // Content-Disposition well-formedness assertions (F5-R1-006: '"' is now mapped
    // to '_' by the SEC-576-004 guard before reaching Part::file_name()).
    let received = server.received_requests().await.unwrap();
    for req in &received {
        if req.method == wiremock::http::Method::POST {
            let body = std::str::from_utf8(&req.body).unwrap_or("");

            // (a) Exactly one Content-Disposition — structural smoke test.
            let cd_count = body.matches("Content-Disposition").count();
            assert_eq!(
                cd_count,
                1,
                "AC-018: expected 1 Content-Disposition in multipart body; \
                 body excerpt: {}",
                &body[..body.len().min(400)]
            );

            // (b) The sanitized form MUST be present: '"' → '_', so filename is file_name.txt.
            assert!(
                body.contains("file_name.txt"),
                "AC-018: SEC-576-004 guard maps '\"' to '_'; \
                 Content-Disposition filename must be 'file_name.txt'; \
                 body excerpt: {}",
                &body[..body.len().min(400)]
            );

            // (c) No raw or reqwest-escaped '"' form should appear (guard fired before
            //     Part::file_name(), so reqwest never sees the '"').
            assert!(
                !body.contains(r#"\""#) && !body.contains("%22"),
                "AC-018: SEC-576-004 guard must suppress '\"' before reqwest encoding; \
                 backslash-escaped or percent-encoded '\"' must NOT appear; \
                 body excerpt: {}",
                &body[..body.len().min(400)]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// F5-R1-006: SEC-576-004 double-quote guard (platform upload path)
// ---------------------------------------------------------------------------

/// F5-R1-006 (platform): double-quote (`"`) in the upload filename must be mapped
/// to underscore (`_`) by the SEC-576-004 guard BEFORE passing to `Part::file_name()`.
///
/// When the guard replaces `"` with `_`:
/// - The Content-Disposition filename becomes `file_name.txt` (safe).
/// - There is no raw `"` or reqwest-escaped `\"` or `%22` form of the original `"`.
///
/// Currently the guard only maps `\r`, `\n`, `\0` — NOT `"`. So the current
/// Content-Disposition body contains reqwest's escaped form (`file\"name.txt`), NOT
/// the sanitized form (`file_name.txt`).
///
/// Note: `test_ac_018_double_quote_filename_well_formed_content_disposition` (above)
/// pins the CURRENT reqwest-encoding behavior. When this F5-R1-006 test turns GREEN,
/// the AC-018 test must be updated to reflect that `"` is now mapped to `_` by the
/// guard (so the body contains `file_name.txt`, not `file\"name.txt`).
///
/// Unix only: `"` is a valid filename char on Unix; Windows rejects it at the FS level.
///
/// RED: current guard does NOT map `"` to `_` → body contains `file\"name.txt` (or
///      `file%22name.txt`), not `file_name.txt` → first assertion fails.
#[cfg(unix)]
#[tokio::test]
async fn test_f5_r1_006_upload_content_disposition_double_quote_mapped_to_underscore() {
    let server = MockServer::start().await;
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let tmp = TempDir::new().unwrap();

    // `"` is valid on Unix; guard must replace it with `_`.
    let file_quote = tmp.path().join("file\"name.txt");
    std::fs::write(&file_quote, b"f5-r1-006 guard test").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/TEST-1/attachments"))
        .and(header("X-Atlassian-Token", "no-check"))
        // Response uses the SANITIZED filename (what the server would receive post-fix).
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            make_upload_attachment("10601", "file_name.txt")
        ])))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "upload",
            "TEST-1",
            &file_quote.to_string_lossy(),
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
        "F5-R1-006 platform: upload with '\"' in filename must exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // Inspect the multipart body received by the mock server.
    let received = server.received_requests().await.unwrap();
    let post_req = received
        .iter()
        .find(|r| r.method == wiremock::http::Method::POST)
        .expect("F5-R1-006 platform: POST to /attachments must have been received");

    let body = std::str::from_utf8(&post_req.body).unwrap_or("");

    // Target: SEC-576-004 guard maps '"' → '_' → filename in body is "file_name.txt".
    // RED: current guard does NOT map '"'; body still contains the '"' form.
    assert!(
        body.contains("file_name.txt"),
        "F5-R1-006 platform: '\"' must be mapped to '_' by SEC-576-004 guard; \
         Content-Disposition filename must be 'file_name.txt'; \
         body excerpt: {}",
        &body[..body.len().min(500)]
    );

    // Neither the backslash-escaped nor percent-encoded form should appear.
    // (Both are present only when the guard fails to strip the '"'.)
    let has_backslash_escaped = body.contains("file\\\"name.txt");
    let has_pct_encoded = body.contains("file%22name.txt");
    assert!(
        !has_backslash_escaped && !has_pct_encoded,
        "F5-R1-006 platform: raw or reqwest-encoded '\"' must NOT appear \
         when guard maps it to '_'; \
         backslash-escaped={has_backslash_escaped} pct-encoded={has_pct_encoded}; \
         body excerpt: {}",
        &body[..body.len().min(500)]
    );
}
