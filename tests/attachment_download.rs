//! CLI-level integration tests for `jr issue attachment download`.
//!
//! RED GATE: all tests in this file FAIL because `handle_attachment_download`
//! contains `todo!()` — the spawned subprocess exits 101 (Rust panic) instead of
//! the expected exit codes and output.
//!
//! After Task 2 (sanitize_attachment_filename) + Task 4/5 (handler implementation),
//! all tests become GREEN.
//!
//! BC anchors: BC-2.7.007, BC-2.7.008, BC-2.7.009, BC-2.7.010, BC-2.7.011,
//!             BC-2.7.012
//! VPs: VP-576-001 (sanitize proptest lives in src/cli/issue/attachments.rs)
//! Story: S-576-2, GitHub issue #576

use assert_cmd::Command;
use proptest::prelude::*;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

fn make_attachment(id: &str, filename: &str, mime: &str, size: u64, created: &str) -> Value {
    serde_json::json!({
        "id": id,
        "filename": filename,
        "mimeType": mime,
        "size": size,
        "created": created,
        "author": {
            "accountId": "abc123",
            "displayName": "Test User",
        },
        "self": format!("https://example.atlassian.net/rest/api/3/attachment/{id}"),
        "content": format!("https://example.atlassian.net/rest/api/3/attachment/content/{id}"),
    })
}

fn issue_with_attachments(key: &str, attachments: Vec<Value>) -> Value {
    serde_json::json!({
        "key": key,
        "fields": {
            "attachment": attachments
        }
    })
}

// ---------------------------------------------------------------------------
// AC-001 — BC-2.7.007: two-step wire path; streaming; JRACLOUD-97046; GHSA-9857-6MW7-FQ2M
// ---------------------------------------------------------------------------

/// AC-001 / BC-2.7.007: `jr issue attachment download <KEY> --id <AID>` issues
/// step-1 metadata GET then step-2 content GET; no `?redirect=false`.
#[tokio::test]
async fn test_bc_2_7_007_two_step_streaming_wire_path() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Step-1: metadata GET
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "filename": "test.txt",
            "size": 5,
            "mimeType": "text/plain",
            "content": format!("{}/rest/api/3/attachment/content/10001", server.uri())
        })))
        .mount(&server)
        .await;

    // Step-2: content GET
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello"))
        .mount(&server)
        .await;

    let out_dir = TempDir::new().unwrap();
    let out_path = out_dir.path().join("test.txt");

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10001",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: handler panics with todo!() → exit 101 → success() returns false.
    // After implementation: exit 0, file exists with content "hello".
    assert!(output.status.success(), "download must succeed (exit 0)");
    assert!(out_path.exists(), "output file must be created");
    assert_eq!(std::fs::read(&out_path).unwrap(), b"hello");
}

/// AC-001 / BC-2.7.007: content GET URL MUST NOT include `?redirect=false`.
#[tokio::test]
async fn test_bc_2_7_007_no_redirect_false_param() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10002"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10002", "filename": "f.bin", "size": 1,
            "mimeType": "application/octet-stream",
            "content": format!("{}/rest/api/3/attachment/content/10002", server.uri())
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/10002"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"x"))
        .mount(&server)
        .await;

    let out_dir = TempDir::new().unwrap();
    let out_path = out_dir.path().join("f.bin");

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10002",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 → success() fails.
    // After implementation: exit 0 + verify no redirect=false in content GET URL.
    assert!(output.status.success(), "download must succeed (exit 0)");

    let requests = server.received_requests().await.unwrap();
    let content_reqs: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path().contains("/attachment/content/"))
        .collect();
    assert!(
        !content_reqs.is_empty(),
        "content GET must have been issued"
    );
    for req in &content_reqs {
        assert!(
            !req.url.query().unwrap_or("").contains("redirect=false"),
            "content GET MUST NOT include ?redirect=false (JRACLOUD-97046)"
        );
    }
}

/// AC-001 / BC-2.7.007 / EC-2.7.007-3 / SEC-576-003 / GHSA-9857-6MW7-FQ2M:
/// Authorization header MUST be absent on the redirect-target request.
/// EC-2.7.007-3 DISTINCT-HOST mandate: uses 127.0.0.1 vs [::1] to avoid vacuous assertion.
#[tokio::test]
async fn test_bc_2_7_007_auth_absent_on_redirect_target() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // Primary Jira API server — default MockServer binds to 127.0.0.1.
    let jira_server = MockServer::start().await;

    // CDN redirect target on [::1] — DISTINCT HOST per EC-2.7.007-3.
    // Same host + different port is vacuous: reqwest host_str() ignores port numbers.
    let cdn_listener = std::net::TcpListener::bind("[::1]:0").unwrap();
    let cdn_server = MockServer::builder().listener(cdn_listener).start().await;

    let cdn_path = "/cdn/att/10003";
    let cdn_url = format!("{}{}", cdn_server.uri(), cdn_path);

    // Step-1: metadata
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10003"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10003",
            "filename": "secure.pdf",
            "size": 4,
            "mimeType": "application/pdf",
        })))
        .mount(&jira_server)
        .await;

    // Step-2: content GET on Jira → 302 redirect to CDN ([::1])
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/10003"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", cdn_url.as_str()))
        .mount(&jira_server)
        .await;

    // CDN target: serves content; we assert Authorization is absent here
    Mock::given(method("GET"))
        .and(path(cdn_path))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"data"))
        .mount(&cdn_server)
        .await;

    let out_dir = TempDir::new().unwrap();
    let out_path = out_dir.path().join("secure.pdf");

    let output = jr_cmd_with_xdg(&jira_server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10003",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 → success() fails.
    // After implementation: exit 0 + Authorization absent on CDN request.
    assert!(
        output.status.success(),
        "cross-host redirect download must succeed (exit 0)"
    );

    let cdn_reqs = cdn_server.received_requests().await.unwrap();
    assert!(
        !cdn_reqs.is_empty(),
        "CDN server must have received a request"
    );
    for req in &cdn_reqs {
        assert!(
            !req.headers.contains_key("authorization"),
            "Authorization MUST be absent on cross-host redirect target (GHSA-9857-6MW7-FQ2M)"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-002 — BC-2.7.007 / P32-001: pre-flights before metadata GET
// ---------------------------------------------------------------------------

/// AC-002 / P32-001: local pre-flight checks fire BEFORE step-1 metadata GET.
#[tokio::test]
async fn test_bc_2_7_007_out_preflight_before_get_p32_001() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Metadata GET MUST NOT be called when pre-flight fails (expect 0 calls).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10004"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10004", "filename": "x.txt", "size": 1
        })))
        .expect(0)
        .mount(&server)
        .await;

    // --out path with non-existent parent directory (EC-2.7.007-6)
    let nonexistent = cache.path().join("no_such_dir").join("file.txt");

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10004",
            "--out",
            nonexistent.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 ≠ 64 → assertion fails.
    // After implementation: exit 64 + "Output directory does not exist:"; no HTTP issued.
    assert_eq!(
        output.status.code(),
        Some(64),
        "--out missing parent must exit 64 before HTTP (P32-001)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Output directory does not exist:"),
        "stderr must contain 'Output directory does not exist:' — got: {stderr}"
    );

    // Sub-assertion (b): P1-003 / EC-2.7.007-11 — --out names an existing DIRECTORY.
    // Pre-flight must reject with "output path is a directory: <PATH>" BEFORE any metadata GET.
    // Current impl: missing directory check → falls through to "File already exists:" at line 655
    // AFTER the metadata GET at line 631 → both wrong message AND ordering violation → RED.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10006"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10006", "filename": "x.txt", "size": 1,
        })))
        .expect(0) // P32-001: must NOT be called — pre-flight fires first
        .mount(&server)
        .await;

    let existing_dir = TempDir::new().unwrap();
    let out_b = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10006",
            "--out",
            existing_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: impl calls metadata GET BEFORE collision check → .expect(0) violated at drop.
    // Also wrong message: "File already exists:" not "output path is a directory:".
    assert_eq!(
        out_b.status.code(),
        Some(64),
        "(b) EC-2.7.007-11: --out existing directory must exit 64 before HTTP (P32-001)"
    );
    let stderr_b = String::from_utf8_lossy(&out_b.stderr);
    assert!(
        stderr_b.contains("output path is a directory:"),
        "(b) stderr must contain 'output path is a directory:' (BC-2.7.007 EC-2.7.007-11) — got: {stderr_b}"
    );

    // Sub-assertion (c): P1-004 / EC-2.7.007-12 + P32-001 — --out is an existing file, no --force.
    // Collision-refuse pre-flight must fire BEFORE the metadata GET.
    // Current impl: metadata GET (line 631) fires BEFORE collision check (line 652) → RED.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10007"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10007", "filename": "y.txt", "size": 1,
        })))
        .expect(0) // P32-001: collision check must fire BEFORE metadata GET
        .mount(&server)
        .await;

    let existing_file_dir = TempDir::new().unwrap();
    let existing_file = existing_file_dir.path().join("already_here.txt");
    std::fs::write(&existing_file, b"original").unwrap();
    let out_c = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10007",
            "--out",
            existing_file.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: impl calls metadata GET BEFORE collision check → .expect(0) violated at drop.
    assert_eq!(
        out_c.status.code(),
        Some(64),
        "(c) EC-2.7.007-12: --out existing file without --force must exit 64 before HTTP (P32-001)"
    );
    let stderr_c = String::from_utf8_lossy(&out_c.stderr);
    assert!(
        stderr_c.contains("File already exists:") && stderr_c.contains("Use --force to overwrite."),
        "(c) stderr must contain 'File already exists: <path>. Use --force to overwrite.' — got: {stderr_c}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 — BC-2.7.007: selector required; AID validation
// ---------------------------------------------------------------------------

/// AC-003 / BC-2.7.007: non-numeric `--id` → exit 64; no selector → exit 2.
#[test]
fn test_bc_2_7_007_selector_required_aid_validation() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    // No selector → clap required-group → exit 2 (fires before handler)
    let out_no_sel = jr_cmd_with_xdg("http://127.0.0.1:1", cache.path(), config.path())
        .args(["issue", "attachment", "download", "FOO-1"])
        .output()
        .unwrap();
    assert_eq!(
        out_no_sel.status.code(),
        Some(2),
        "no selector must exit 2 (clap required-group)"
    );

    // Non-numeric --id → handler must exit 64 + "invalid attachment id:"
    let out_bad = jr_cmd_with_xdg("http://127.0.0.1:1", cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "not-a-number",
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 64 → assertion fails.
    assert_eq!(
        out_bad.status.code(),
        Some(64),
        "non-numeric --id must exit 64"
    );
    let stderr = String::from_utf8_lossy(&out_bad.stderr);
    assert!(
        stderr.contains("invalid attachment id:"),
        "stderr must contain 'invalid attachment id:' — got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-005 — BC-2.7.010: default output path SHA-1 prefix (batch)
// ---------------------------------------------------------------------------

/// AC-005 / BC-2.7.010: batch default path = `<sha1-of-id>_<sanitized-filename>`.
#[tokio::test]
async fn test_bc_2_7_010_default_path_sha1_prefix_batch() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    let att = make_attachment(
        "20001",
        "report.pdf",
        "application/pdf",
        4,
        "2026-07-10T14:00:00.000+0000",
    );

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments("FOO-2", vec![att])),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/20001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"pdf!"))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-2",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 → success() fails.
    // After implementation: exit 0 + file at <sha1("20001")>_report.pdf.
    assert!(
        output.status.success(),
        "batch download must succeed (exit 0)"
    );

    let entries: Vec<_> = std::fs::read_dir(out_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1, "exactly one file must be written");
    let name = entries[0].file_name().to_string_lossy().into_owned();
    // Pattern: 40 hex chars + "_" + sanitized basename
    assert!(
        name.len() > 41,
        "filename must have SHA-1 prefix + underscore + basename, got: {name}"
    );
    let (prefix, rest) = name.split_at(41); // 40 hex + "_"
    assert!(
        prefix.ends_with('_') && prefix[..40].chars().all(|c| c.is_ascii_hexdigit()),
        "first 40 chars must be hex SHA-1 followed by '_', got prefix: {prefix}"
    );
    assert_eq!(
        rest, "report.pdf",
        "basename after SHA-1 prefix must be sanitized filename"
    );
}

// ---------------------------------------------------------------------------
// AC-006 — BC-2.7.007 / P27-001 / P31-002: JSON manifest shape
// ---------------------------------------------------------------------------

/// AC-006 / P27-001 / P31-002: JSON manifest — filename=RAW, size=bytes-written.
#[tokio::test]
async fn test_bc_2_7_007_json_manifest_raw_filename_written_size_p27_p31() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();
    let out_path = out_dir.path().join("output.txt");

    // metadata size=100 differs from actual body size=5
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10005"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10005",
            "filename": "raw_name.txt",
            "size": 100,
            "mimeType": "text/plain",
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/10005"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello"))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10005",
            "--out",
            out_path.to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 → success() fails.
    // After implementation: exit 0, JSON manifest with filename=RAW, size=5.
    assert!(
        output.status.success(),
        "JSON download must succeed (exit 0)"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON — {e}\nstdout: {stdout}"));
    let downloaded = manifest["downloaded"].as_array().unwrap();
    assert_eq!(downloaded.len(), 1);
    let entry = &downloaded[0];
    // P27-001: filename = RAW Jira name (pre-sanitization)
    assert_eq!(
        entry["filename"], "raw_name.txt",
        "filename must be RAW Jira name (P27-001)"
    );
    // P31-002: size = bytes written (5), NOT metadata size (100)
    assert_eq!(
        entry["size"], 5,
        "size must be bytes written (P31-002), not metadata size"
    );
    assert_eq!(entry["id"], "10005");
}

// ---------------------------------------------------------------------------
// AC-007 — BC-2.7.008: --all batch; fail-soft; out-dir checks; cwd default
// ---------------------------------------------------------------------------

/// AC-007 / BC-2.7.008: batch --all fail-soft; out-dir checks; partial/all-fail exit 1.
#[tokio::test]
async fn test_bc_2_7_008_all_batch_fail_soft() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Sub-assertion (a): --out-dir not-exist → exit 64 + canonical string (EC-2.7.008-2)
    let nonexistent = cache.path().join("no_such_dir");
    let out_a = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--all",
            "--out-dir",
            nonexistent.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 64.
    assert_eq!(
        out_a.status.code(),
        Some(64),
        "(a) --out-dir not-exist must exit 64 (EC-2.7.008-2)"
    );
    let stderr_a = String::from_utf8_lossy(&out_a.stderr);
    assert!(
        stderr_a.contains("Output directory does not exist:"),
        "(a) stderr must contain 'Output directory does not exist:' — got: {stderr_a}"
    );

    // Sub-assertion (b): --out-dir is a file → exit 64 + "Not a directory:" (EC-2.7.008-4)
    let existing_file = cache.path().join("is_a_file.txt");
    std::fs::write(&existing_file, b"content").unwrap();
    let out_b = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--all",
            "--out-dir",
            existing_file.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 64.
    assert_eq!(
        out_b.status.code(),
        Some(64),
        "(b) --out-dir pointing at a file must exit 64 (EC-2.7.008-4)"
    );
    let stderr_b = String::from_utf8_lossy(&out_b.stderr);
    assert!(
        stderr_b.contains("Not a directory:"),
        "(b) stderr must contain 'Not a directory:' — got: {stderr_b}"
    );

    // Sub-assertion (c): EC-2.7.008-7 partial-fail: 1-of-2 content-GETs fails → exit 1
    let out_dir_c = TempDir::new().unwrap();
    let att1 = make_attachment(
        "30001",
        "file1.txt",
        "text/plain",
        5,
        "2026-07-10T14:00:00.000+0000",
    );
    let att2 = make_attachment(
        "30002",
        "file2.txt",
        "text/plain",
        5,
        "2026-07-10T14:00:01.000+0000",
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PARTIAL-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_with_attachments("PARTIAL-1", vec![att1, att2])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/30001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/30002"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let out_c = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "PARTIAL-1",
            "--all",
            "--out-dir",
            out_dir_c.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 1.
    assert_eq!(
        out_c.status.code(),
        Some(1),
        "(c) partial batch failure must exit 1 (EC-2.7.008-7)"
    );
    let stderr_c = String::from_utf8_lossy(&out_c.stderr);
    assert!(
        stderr_c.contains("warning: failed to download attachment 30002:"),
        "(c) stderr must contain per-file failure warning — got: {stderr_c}"
    );
    // P1-007 / BC-2.7.008: fail-soft batch must NOT emit "API error (1)" on stderr.
    // Current impl: JrError::ApiError { status: 1, message: "batch download completed with errors" }
    // propagates through anyhow → main.rs prints "API error (1): ..." to stderr → RED.
    assert!(
        !stderr_c.contains("API error (1)"),
        "(c) batch fail-soft must NOT emit spurious 'API error (1)' to stderr (BC-2.7.008) — got: {stderr_c}"
    );
    let combined_c = format!("{}{}", String::from_utf8_lossy(&out_c.stdout), &stderr_c);
    // P1-008 / BC-2.7.008 ~799: canonical summary must end with trailing period.
    // Current impl: "Downloaded {} of {} attachments to {}" (no period) → RED.
    let expected_summary_c = format!(
        "Downloaded 1 of 2 attachments to {}.",
        out_dir_c.path().to_str().unwrap()
    );
    assert!(
        combined_c.contains(&expected_summary_c),
        "(c) canonical summary 'Downloaded 1 of 2 attachments to <dir>.' required (BC-2.7.008 ~799) — stdout+stderr: {combined_c}"
    );

    // Sub-assertion (d): EC-2.7.008-8 all-fail: every content-GET fails → exit 1
    let out_dir_d = TempDir::new().unwrap();
    let att3 = make_attachment(
        "30003",
        "f3.txt",
        "text/plain",
        5,
        "2026-07-10T14:00:00.000+0000",
    );
    let att4 = make_attachment(
        "30004",
        "f4.txt",
        "text/plain",
        5,
        "2026-07-10T14:00:01.000+0000",
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ALLFAIL-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_with_attachments("ALLFAIL-1", vec![att3, att4])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/30003"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/30004"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let out_d = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "ALLFAIL-1",
            "--all",
            "--out-dir",
            out_dir_d.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 1.
    assert_eq!(
        out_d.status.code(),
        Some(1),
        "(d) all-fail batch must exit 1 (EC-2.7.008-8)"
    );
    let stderr_d = String::from_utf8_lossy(&out_d.stderr);
    // P1-007 / BC-2.7.008: all-fail batch must NOT emit "API error (1)" on stderr.
    assert!(
        !stderr_d.contains("API error (1)"),
        "(d) all-fail batch must NOT emit spurious 'API error (1)' to stderr (BC-2.7.008) — got: {stderr_d}"
    );
    let combined_d = format!("{}{}", String::from_utf8_lossy(&out_d.stdout), &stderr_d);
    // P1-008 / BC-2.7.008 ~799: canonical summary must end with trailing period.
    let expected_summary_d = format!(
        "Downloaded 0 of 2 attachments to {}.",
        out_dir_d.path().to_str().unwrap()
    );
    assert!(
        combined_d.contains(&expected_summary_d),
        "(d) canonical summary 'Downloaded 0 of 2 attachments to <dir>.' required (BC-2.7.008 ~799) — stdout+stderr: {combined_d}"
    );
}

/// AC-007 / BC-2.7.008 ~785/791: --all without --out-dir downloads to cwd.
#[tokio::test]
async fn test_bc_2_7_008_all_no_out_dir_defaults_to_cwd() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let cwd = TempDir::new().unwrap();

    let att = make_attachment(
        "40001",
        "cwdfile.txt",
        "text/plain",
        3,
        "2026-07-10T14:00:00.000+0000",
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/CWD-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments("CWD-1", vec![att])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/40001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"cwd"))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .current_dir(cwd.path())
        .args(["issue", "attachment", "download", "CWD-1", "--all"])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 → success() fails.
    // After implementation: exit 0 + file lands in cwd.
    assert!(
        output.status.success(),
        "--all without --out-dir must succeed (exit 0)"
    );
    let entries: Vec<_> = std::fs::read_dir(cwd.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        !entries.is_empty(),
        "file must land in cwd when --out-dir is absent"
    );
}

/// AC-007 / EC-2.7.008-1 / EC-2.7.009-4: empty issue → hint + exit 0.
#[tokio::test]
async fn test_bc_2_7_008_empty_issue_no_attachments_hint() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/EMPTY-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments("EMPTY-1", vec![])),
        )
        .mount(&server)
        .await;

    // Sub-assertion (a): human mode --all on zero-attachment issue (EC-2.7.008-1)
    let out_human = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "EMPTY-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 → success() fails.
    assert!(
        out_human.status.success(),
        "(a) empty issue --all must exit 0 (EC-2.7.008-1)"
    );
    let stderr_h = String::from_utf8_lossy(&out_human.stderr);
    assert!(
        stderr_h.contains("No attachments on EMPTY-1."),
        "(a) stderr must contain 'No attachments on EMPTY-1.' — got: {stderr_h}"
    );

    // Sub-assertion (b): JSON mode --all on zero-attachment issue
    let out_json = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "EMPTY-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 → success() fails.
    assert!(
        out_json.status.success(),
        "(b) empty issue --all JSON mode must exit 0"
    );
    let stdout_j = String::from_utf8_lossy(&out_json.stdout);
    let manifest: Value = serde_json::from_str(&stdout_j)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON — {e}\nstdout: {stdout_j}"));
    assert_eq!(
        manifest["downloaded"],
        serde_json::json!([]),
        "(b) JSON mode empty issue must return {{\"downloaded\":[]}}"
    );
    let stderr_j = String::from_utf8_lossy(&out_json.stderr);
    assert!(
        !stderr_j.contains("No attachments on"),
        "(b) JSON mode must NOT emit hint to stderr — got: {stderr_j}"
    );

    // Sub-assertion (c): EC-2.7.009-4: --newest N on zero-attachment issue
    let out_newest = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "EMPTY-1",
            "--newest",
            "3",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 → success() fails.
    assert!(
        out_newest.status.success(),
        "(c) --newest on empty issue must exit 0 (EC-2.7.009-4)"
    );
    let stdout_n = String::from_utf8_lossy(&out_newest.stdout);
    let manifest_n: Value = serde_json::from_str(&stdout_n)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON — {e}\nstdout: {stdout_n}"));
    assert_eq!(
        manifest_n["downloaded"],
        serde_json::json!([]),
        "(c) --newest on empty issue must return {{\"downloaded\":[]}}"
    );
}

// ---------------------------------------------------------------------------
// AC-008 — BC-2.7.009: --newest N by created desc
// ---------------------------------------------------------------------------

/// AC-008 / BC-2.7.009: --newest N selects top-N by created descending.
#[tokio::test]
async fn test_bc_2_7_009_newest_n_by_created_desc() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    // P1-001 / BC-2.7.009: mixed UTC offsets disambiguate chrono sort from lexicographic sort.
    //
    // id=50002 "2026-07-10T20:00:00.000+0900" = 2026-07-10T11:00Z  ← lex-rank-1 (string "20:00" > "14:00")
    // id=50003 "2026-07-10T14:00:00.000+0000" = 2026-07-10T14:00Z  ← chrono-rank-1 (TRULY newest)
    //
    // Lexicographic sort: would pick 50002 as the single newest (WRONG).
    // Chronological sort: must pick 50003 (correct per BC-2.7.009 ~830).
    //
    // 50002 content mock is mounted with .expect(0): if lexicographic sort is used, the handler
    // calls the 50002 content GET, the expectation is violated, and the mock server panics on drop.
    let atts = vec![
        make_attachment(
            "50001",
            "oldest.txt",
            "text/plain",
            1,
            "2026-01-10T10:00:00.000+0000", // 2026-01-10T10:00Z — clearly oldest
        ),
        make_attachment(
            "50002",
            "lex_newer.txt",
            "text/plain",
            1,
            "2026-07-10T20:00:00.000+0900", // = 2026-07-10T11:00Z — lex-newer string, chrono-OLDER
        ),
        make_attachment(
            "50003",
            "chrono_newest.txt",
            "text/plain",
            1,
            "2026-07-10T14:00:00.000+0000", // = 2026-07-10T14:00Z — lex-older string, chrono-NEWEST
        ),
    ];

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/NEWEST-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments("NEWEST-1", atts)),
        )
        .mount(&server)
        .await;

    // Correct answer (chrono): only 50003 should be downloaded with --newest 1.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/50003"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"chrono_newest"))
        .mount(&server)
        .await;

    // Wrong answer (lex): 50002 must NOT be called — if lexicographic sort is used, this fires.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/50002"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"lex_newer"))
        .expect(0) // BC-2.7.009: chrono sort must be used; lex sort picks 50002 (WRONG)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "NEWEST-1",
            "--newest",
            "1",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    // RED GATE: lexicographic sort (b.created.cmp(&a.created)) picks 50002 (lex-rank-1); the
    // .expect(0) on the 50002 content mock fires → test fails at server drop.
    // After fix: chrono DateTime<FixedOffset> parsing picks 50003 (chrono-rank-1).
    assert!(output.status.success(), "--newest 1 must succeed (exit 0)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON — {e}\nstdout: {stdout}"));
    let downloaded = manifest["downloaded"].as_array().unwrap();
    assert_eq!(
        downloaded.len(),
        1,
        "--newest 1 must download exactly 1 attachment"
    );
    let ids: Vec<&str> = downloaded
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    // Chrono-newest is 50003 (14:00Z) — must be selected.
    assert!(
        ids.contains(&"50003"),
        "chrono-newest (50003, 14:00Z) must be selected — BC-2.7.009 requires instant-based sort; got: {ids:?}"
    );
    // Lex-newest is 50002 (20:00+09:00 = 11:00Z) — must NOT be selected.
    assert!(
        !ids.contains(&"50002"),
        "lex-newest (50002, 11:00Z) must NOT be selected — BC-2.7.009 forbids lexicographic sort; got: {ids:?}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 — BC-2.7.012: error taxonomy
// ---------------------------------------------------------------------------

/// AC-009 / BC-2.7.012: full error taxonomy — ALL rows as explicit sub-assertions.
#[tokio::test]
async fn test_bc_2_7_012_error_taxonomy() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    // invalid AID (non-numeric) → exit 64 + "invalid attachment id:"
    let out_inv = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "not-numeric",
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 64.
    assert_eq!(
        out_inv.status.code(),
        Some(64),
        "non-numeric AID must exit 64"
    );
    assert!(
        String::from_utf8_lossy(&out_inv.stderr).contains("invalid attachment id:"),
        "non-numeric AID stderr must contain 'invalid attachment id:'"
    );

    // AID 404 (--id path) → exit 64 + "Attachment" AND "not found or not accessible"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/99999"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(serde_json::json!({"errorMessages": ["Not found"]})),
        )
        .mount(&server)
        .await;
    let tmp_404 = out_dir.path().join("notfound.txt");
    let out_404 = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "99999",
            "--out",
            tmp_404.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 64.
    assert_eq!(out_404.status.code(), Some(64), "AID 404 must exit 64");
    let s = String::from_utf8_lossy(&out_404.stderr);
    assert!(
        s.contains("Attachment") && s.contains("not found or not accessible"),
        "AID 404 stderr must contain 'Attachment' + 'not found or not accessible' — got: {s}"
    );

    // KEY 404 (batch --all path) → exit 64 + "Issue" AND "not found or not accessible"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/NOTEXIST-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let out_key404 = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "NOTEXIST-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 64.
    assert_eq!(
        out_key404.status.code(),
        Some(64),
        "KEY 404 batch must exit 64"
    );
    let s = String::from_utf8_lossy(&out_key404.stderr);
    assert!(
        s.contains("Issue") && s.contains("not found or not accessible"),
        "KEY 404 stderr must contain 'Issue' + 'not found or not accessible' — got: {s}"
    );

    // 401 → exit 2 + "Not authenticated" AND "jr auth login"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/88888"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let tmp_401 = out_dir.path().join("auth.txt");
    let out_401 = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "88888",
            "--out",
            tmp_401.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 2.
    assert_eq!(out_401.status.code(), Some(2), "401 must exit 2");
    let s = String::from_utf8_lossy(&out_401.stderr);
    assert!(
        s.contains("Not authenticated") && s.contains("jr auth login"),
        "401 stderr must contain 'Not authenticated' AND 'jr auth login' — got: {s}"
    );

    // AID 403 (single --id path) → exit 1 + "Permission denied: cannot access attachment "
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/77777"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let tmp_403 = out_dir.path().join("forbidden.txt");
    let out_403id = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "77777",
            "--out",
            tmp_403.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 1.
    assert_eq!(out_403id.status.code(), Some(1), "AID 403 must exit 1");
    let s = String::from_utf8_lossy(&out_403id.stderr);
    assert!(
        s.contains("Permission denied: cannot access attachment "),
        "AID 403 stderr must contain 'Permission denied: cannot access attachment ' — got: {s}"
    );

    // KEY 403 (batch --all/--newest path) → exit 1 + "Permission denied: cannot access issue "
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FORBIDDEN-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let out_403key = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FORBIDDEN-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 1.
    assert_eq!(
        out_403key.status.code(),
        Some(1),
        "KEY 403 batch must exit 1"
    );
    let s = String::from_utf8_lossy(&out_403key.stderr);
    assert!(
        s.contains("Permission denied: cannot access issue "),
        "KEY 403 stderr must contain 'Permission denied: cannot access issue ' — got: {s}"
    );

    // 5xx single → exit 1 + "API error ("
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/66666"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let tmp_5xx = out_dir.path().join("fivehundred.txt");
    let out_5xx = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "66666",
            "--out",
            tmp_5xx.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 1.
    assert_eq!(out_5xx.status.code(), Some(1), "5xx must exit 1");
    let s = String::from_utf8_lossy(&out_5xx.stderr);
    assert!(
        s.contains("API error ("),
        "5xx stderr must contain 'API error (' — got: {s}"
    );

    // Network error → exit 1 + "Could not reach" (port 1 = connection refused)
    let tmp_net = out_dir.path().join("network.txt");
    let out_net = jr_cmd_with_xdg("http://127.0.0.1:1", cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "55555",
            "--out",
            tmp_net.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 1.
    assert_eq!(out_net.status.code(), Some(1), "network error must exit 1");
    let s = String::from_utf8_lossy(&out_net.stderr);
    assert!(
        s.contains("Could not reach"),
        "network error stderr must contain 'Could not reach' — got: {s}"
    );
    // Note: ENOSPC and EACCES are documented in BC-2.7.012 but are not deterministically
    // triggerable in CI. Their canonical strings are:
    //   ENOSPC → exit 1 + "Disk full: not enough space to write <path>"
    //   EACCES → exit 1 + "Permission denied: cannot write to <dir>"
}

// ---------------------------------------------------------------------------
// AC-010 — BC-2.7.007 / EC-2.7.007-2 / JSDCLOUD-10841: platform content URL
// ---------------------------------------------------------------------------

/// AC-010 / EC-2.7.007-2: download ALWAYS uses platform content URL, not JSM links.
#[tokio::test]
async fn test_bc_2_7_007_uses_platform_content_url_not_jsm_links_ec_2_7_007_2() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();
    let out_path = out_dir.path().join("jsm_file.txt");

    // JSM-issue attachment where 'content' is a servicedeskapi URL.
    // The handler MUST NOT follow this URL — JSDCLOUD-10841.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/60001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "60001",
            "filename": "jsm_doc.txt",
            "size": 3,
            "mimeType": "text/plain",
            "content": format!("{}/rest/servicedeskapi/attachment/60001", server.uri()),
        })))
        .mount(&server)
        .await;

    // Platform content endpoint (must be used)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/60001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"jsm"))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "JSM-1",
            "--id",
            "60001",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 → success() fails.
    // After implementation: exit 0 + only platform endpoint called.
    assert!(
        output.status.success(),
        "JSM attachment download must succeed (exit 0)"
    );
    assert!(out_path.exists(), "output file must be created");

    let requests = server.received_requests().await.unwrap();
    let jsm_calls: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path().contains("servicedeskapi"))
        .collect();
    assert!(
        jsm_calls.is_empty(),
        "MUST NOT use servicedeskapi content URL (JSDCLOUD-10841)"
    );
    let platform_calls: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path() == "/rest/api/3/attachment/content/60001")
        .collect();
    assert!(
        !platform_calls.is_empty(),
        "MUST use platform endpoint /rest/api/3/attachment/content/{{id}}"
    );
}

// ---------------------------------------------------------------------------
// AC-011 — BC-2.7.007: write-to-temp + atomic rename; cleanup on error
// ---------------------------------------------------------------------------

/// AC-011 / BC-2.7.007: atomic rename from temp file; cleanup on mid-stream error.
#[tokio::test]
async fn test_bc_2_7_007_atomic_rename_cleanup_on_error() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();
    let final_path = out_dir.path().join("partial.bin");

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/70001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "70001",
            "filename": "partial.bin",
            "size": 1000,
            "mimeType": "application/octet-stream",
        })))
        .mount(&server)
        .await;

    // Content GET fails → mid-stream error
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/70001"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "70001",
            "--out",
            final_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 ≠ 1.
    // After implementation: exit 1 + final path absent + no tmp_ files remain.
    assert_eq!(
        output.status.code(),
        Some(1),
        "mid-stream error must exit 1 (EC-2.7.007-4)"
    );
    assert!(
        !final_path.exists(),
        "final path must NOT exist after stream error (no partial file)"
    );
    let tmp_files: Vec<_> = std::fs::read_dir(out_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("tmp_"))
        .collect();
    assert!(
        tmp_files.is_empty(),
        "no tmp_ files must remain after error (temp cleanup required)"
    );
}

/// AC-011 / BC-2.7.007 ~749: temp file naming — `tmp_<random>` in same dir as final path.
#[tokio::test]
async fn test_bc_2_7_007_temp_file_same_dir_tmp_random_prefix() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();
    let final_path = out_dir.path().join("output.dat");

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/70002"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "70002",
            "filename": "output.dat",
            "size": 5,
            "mimeType": "application/octet-stream",
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/70002"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"abcde"))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "70002",
            "--out",
            final_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 → success() fails.
    // After implementation: exit 0 + no <basename>.tmp + no tmp_ remaining.
    assert!(output.status.success(), "download must succeed (exit 0)");
    assert!(final_path.exists(), "final path must exist");

    // MUST NOT use <basename>.tmp naming (BC-2.7.007 ~749)
    let basename_tmp = out_dir.path().join("output.dat.tmp");
    assert!(
        !basename_tmp.exists(),
        "MUST NOT use '<basename>.tmp' naming — BC-2.7.007 requires 'tmp_<random>'"
    );

    // No tmp_ files remain after successful atomic rename
    let tmp_remaining: Vec<_> = std::fs::read_dir(out_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("tmp_"))
        .collect();
    assert!(
        tmp_remaining.is_empty(),
        "no tmp_ files must remain after successful atomic rename"
    );
    assert_eq!(std::fs::read(&final_path).unwrap(), b"abcde");
}

// ---------------------------------------------------------------------------
// AC-012 — BC-2.7.008 / EC-2.7.008-6/7: JSON mode hint/error taxonomy
// ---------------------------------------------------------------------------

/// AC-012 / EC-2.7.008-6/7: JSON mode — per-file failures → stderr warning; exit 1.
#[tokio::test]
async fn test_bc_2_7_008_json_mode_error_vs_hint_taxonomy() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    let att_ok = make_attachment(
        "80001",
        "ok.txt",
        "text/plain",
        2,
        "2026-07-10T14:00:00.000+0000",
    );
    let att_fail = make_attachment(
        "80002",
        "fail.txt",
        "text/plain",
        2,
        "2026-07-10T14:00:01.000+0000",
    );

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/JSONERR-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_with_attachments("JSONERR-1", vec![att_ok, att_fail])),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/80001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/80002"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "JSONERR-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 ≠ 1.
    // After implementation: exit 1 + per-file warning on stderr + manifest on stdout.
    assert_eq!(
        output.status.code(),
        Some(1),
        "partial JSON mode failure must exit 1 (EC-2.7.008-6/7)"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: failed to download attachment 80002:"),
        "stderr must contain per-file failure warning — got: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON — {e}\nstdout: {stdout}"));
    let downloaded = manifest["downloaded"].as_array().unwrap();
    assert_eq!(
        downloaded.len(),
        1,
        "manifest must contain only the successful entry"
    );
    assert_eq!(downloaded[0]["id"], "80001");

    // "Downloaded N of M" summary is a hint — must NOT appear in stdout (JSON mode)
    assert!(
        !stdout.contains("Downloaded"),
        "JSON mode must NOT emit 'Downloaded N of M' hint to stdout — got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-015 — BC-2.7.010 / SEC-576-011 / CWE-116: degenerate name warning
// ---------------------------------------------------------------------------

/// AC-015 / BC-2.7.010 SEC-576-011: degenerate name warning uses display_sanitize_filename.
#[tokio::test]
async fn test_bc_2_7_010_degenerate_name_warning_display_sanitized() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    // ".." sanitizes to None → degenerate fallback → warning
    let att = make_attachment(
        "90001",
        "..",
        "text/plain",
        5,
        "2026-07-10T14:00:00.000+0000",
    );

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/DEGEN-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments("DEGEN-1", vec![att])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/90001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"degen"))
        .mount(&server)
        .await;

    // Human mode: warning to stderr
    let out_human = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "DEGEN-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 → success() fails.
    assert!(
        out_human.status.success(),
        "degenerate-name download must succeed (exit 0)"
    );
    let stderr = String::from_utf8_lossy(&out_human.stderr);
    // P1-010 / BC-2.7.010 ~872: exact canonical wording with em-dash U+2014, "original name",
    // single-quotes around raw name, and trailing period.
    // Current impl (line 772): "warning: using id as filename for attachment {}: '{}' could not be sanitized"
    // (wrong: no em-dash, no "original name", no period) → RED.
    assert!(
        stderr.contains(
            "warning: using id as filename for attachment 90001 \u{2014} original name '..' could not be sanitized."
        ),
        "exact canonical degenerate-name warning required (BC-2.7.010 ~872): \
         'warning: using id as filename for attachment <id> \u{2014} original name \'<raw>\' could not be sanitized.' \
         — got: {stderr}"
    );

    // JSON mode: no warning (hint, suppressed in JSON mode)
    let out_json = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "DEGEN-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
            "--force",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 → success() fails.
    assert!(
        out_json.status.success(),
        "degenerate-name JSON mode must succeed (exit 0)"
    );
    let stderr_j = String::from_utf8_lossy(&out_json.stderr);
    assert!(
        !stderr_j.contains("using id as filename"),
        "JSON mode must NOT emit degenerate-name warning to stderr — got: {stderr_j}"
    );
}

// ---------------------------------------------------------------------------
// AC-016 — BC-2.7.011 / SEC-576-001: Windows device-name escape at single-id call site
// ---------------------------------------------------------------------------

/// AC-016 / SEC-576-001: Windows device name escape at single-id call site.
#[tokio::test]
async fn test_bc_2_7_011_windows_device_name_escape_single_id_call_site() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    // filename "CON" → sanitize_attachment_filename returns Some("CON")
    // single-id call site MUST prepend "_" → file written as "_CON"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/91001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "91001",
            "filename": "CON",
            "size": 3,
            "mimeType": "application/octet-stream",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/91001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"con"))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .current_dir(out_dir.path())
        .args(["issue", "attachment", "download", "FOO-1", "--id", "91001"])
        .output()
        .unwrap();

    // RED GATE: todo!() → exit 101 → success() fails.
    // After implementation: exit 0 + file "_CON" (not bare "CON").
    assert!(
        output.status.success(),
        "device-name download must succeed (exit 0)"
    );
    assert!(
        out_dir.path().join("_CON").exists(),
        "single-id call site MUST escape Windows device name CON → _CON (SEC-576-001)"
    );
    assert!(
        !out_dir.path().join("CON").exists(),
        "bare 'CON' MUST NOT be created (Windows device name)"
    );
}

// ---------------------------------------------------------------------------
// AC-017 — BC-2.7.007/008/009: clap structural constraints + handler N-validation
// ---------------------------------------------------------------------------

/// AC-017 Layer 1: 10 clap exit-2 cases for download flag combinations.
#[test]
fn test_bc_2_7_download_clap_structural_constraints() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();

    let cases: &[(&[&str], &str)] = &[
        // (1) --id + --all → exit 2
        (
            &[
                "issue",
                "attachment",
                "download",
                "FOO-1",
                "--id",
                "10001",
                "--all",
            ],
            "--id + --all must exit 2",
        ),
        // (2) --id + --newest 3 → exit 2
        (
            &[
                "issue",
                "attachment",
                "download",
                "FOO-1",
                "--id",
                "10001",
                "--newest",
                "3",
            ],
            "--id + --newest must exit 2",
        ),
        // (3) --all + --newest 3 → exit 2
        (
            &[
                "issue",
                "attachment",
                "download",
                "FOO-1",
                "--all",
                "--newest",
                "3",
            ],
            "--all + --newest must exit 2",
        ),
        // (4) --all + --out /tmp → exit 2
        (
            &[
                "issue",
                "attachment",
                "download",
                "FOO-1",
                "--all",
                "--out",
                "/tmp/foo",
            ],
            "--all + --out must exit 2",
        ),
        // (5) --newest 3 + --out /tmp → exit 2
        (
            &[
                "issue",
                "attachment",
                "download",
                "FOO-1",
                "--newest",
                "3",
                "--out",
                "/tmp/foo",
            ],
            "--newest + --out must exit 2",
        ),
        // (6) no selector → exit 2
        (
            &["issue", "attachment", "download", "FOO-1"],
            "no selector must exit 2",
        ),
        // (7) --newest foo (non-integer) → exit 2
        (
            &[
                "issue",
                "attachment",
                "download",
                "FOO-1",
                "--newest",
                "foo",
            ],
            "--newest non-integer must exit 2",
        ),
        // (8) --id + --filter k=v → exit 2
        (
            &[
                "issue",
                "attachment",
                "download",
                "FOO-1",
                "--id",
                "10001",
                "--filter",
                "mime=image/png",
            ],
            "--id + --filter must exit 2",
        ),
        // (9) --out-dir /tmp (no batch selector) → exit 2
        (
            &[
                "issue",
                "attachment",
                "download",
                "FOO-1",
                "--out-dir",
                "/tmp",
            ],
            "--out-dir without batch selector must exit 2",
        ),
        // (10) --out-dir /tmp --id AID → exit 2
        (
            &[
                "issue",
                "attachment",
                "download",
                "FOO-1",
                "--out-dir",
                "/tmp",
                "--id",
                "10001",
            ],
            "--out-dir + --id must exit 2",
        ),
    ];

    for (args, msg) in cases {
        let output = jr_cmd_with_xdg("http://127.0.0.1:1", cache.path(), config.path())
            .args(*args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{msg}");
    }

    // RED GATE sentinel: clap constraints are wired in the stub so the 10 cases above
    // PASS before the handler runs. This sentinel forces the test to FAIL until the
    // handler is implemented: a valid invocation must reach the handler, but todo!()
    // panics → exit 101. The assertion below fails (101 ≠ not-101 is false, meaning
    // assert_ne! succeeds only when code IS 101 → wait, we need to fail RED.
    //
    // assert_ne!(code, Some(101)) → FAILS when code == Some(101) (todo!() panic).
    let sentinel_out = cache.path().join("sentinel.txt");
    let sentinel = jr_cmd_with_xdg("http://127.0.0.1:1", cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10001",
            "--out",
            sentinel_out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ne!(
        sentinel.status.code(),
        Some(101),
        "RED GATE: handle_attachment_download must not be a todo!() stub (exit 101 detected)"
    );
}

/// AC-017 Layer 2: handler-level N-validation (N ≤ 0 → exit 64).
#[test]
fn test_bc_2_7_009_newest_nonpositive_exits_64() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();

    // --newest 0 → exit 64 + "--newest requires a positive integer."
    let out_zero = jr_cmd_with_xdg("http://127.0.0.1:1", cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--newest",
            "0",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 64.
    assert_eq!(
        out_zero.status.code(),
        Some(64),
        "--newest 0 must exit 64 (EC-2.7.009-1)"
    );
    let s = String::from_utf8_lossy(&out_zero.stderr);
    assert!(
        s.contains("--newest requires a positive integer."),
        "--newest 0 must say '--newest requires a positive integer.' — got: {s}"
    );

    // --newest -3 → exit 64 + same message (allow_negative_numbers=true lets -3 reach handler)
    let out_neg = jr_cmd_with_xdg("http://127.0.0.1:1", cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--newest",
            "-3",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 ≠ 64.
    assert_eq!(
        out_neg.status.code(),
        Some(64),
        "--newest -3 must exit 64 (EC-2.7.009-1)"
    );
    let s = String::from_utf8_lossy(&out_neg.stderr);
    assert!(
        s.contains("--newest requires a positive integer."),
        "--newest -3 must say '--newest requires a positive integer.' — got: {s}"
    );
}

// ---------------------------------------------------------------------------
// AC-018 — BC-2.7.007 ~747: single-id success hint to stderr
// ---------------------------------------------------------------------------

/// AC-018 / BC-2.7.007 ~747: single-id success hint emitted to stderr (not in JSON mode).
#[tokio::test]
async fn test_bc_2_7_007_single_id_success_hint_stderr() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    // Human mode: uses AID 92001
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/92001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "92001", "filename": "hint.txt", "size": 4, "mimeType": "text/plain",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/92001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"data"))
        .mount(&server)
        .await;

    let out_path = out_dir.path().join("hint.txt");
    let out_human = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "92001",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 → success() fails.
    assert!(
        out_human.status.success(),
        "single-id download must succeed (exit 0)"
    );
    let stderr = String::from_utf8_lossy(&out_human.stderr);
    // P1-002 / BC-2.7.007 ~751: exact canonical "Downloaded: <path> (<size_human>)." format.
    // Current impl emits only "Downloaded: {path}" (no size, no period) → RED.
    let path_str = out_path.to_str().unwrap();
    assert!(
        stderr.contains(&format!("Downloaded: {path_str} (")) && stderr.contains(")."),
        "human mode must emit canonical 'Downloaded: <path> (<size_human>).' hint to stderr (BC-2.7.007 ~751) — got: {stderr}"
    );

    // JSON mode: uses AID 92002 (distinct to avoid mock-matching ambiguity)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/92002"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "92002", "filename": "hint2.txt", "size": 4, "mimeType": "text/plain",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/92002"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"data"))
        .mount(&server)
        .await;

    let out_path2 = out_dir.path().join("hint2.txt");
    let out_json = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "92002",
            "--out",
            out_path2.to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 → success() fails.
    assert!(
        out_json.status.success(),
        "JSON mode single-id download must succeed (exit 0)"
    );
    let stderr_j = String::from_utf8_lossy(&out_json.stderr);
    assert!(
        !stderr_j.contains("Downloaded:"),
        "JSON mode must NOT emit 'Downloaded:' hint to stderr — got: {stderr_j}"
    );
    let stdout_j = String::from_utf8_lossy(&out_json.stdout);
    let manifest: Value = serde_json::from_str(&stdout_j).unwrap_or_else(|e| {
        panic!("JSON mode stdout must be valid JSON — {e}\nstdout: {stdout_j}")
    });
    assert!(
        manifest["downloaded"].is_array(),
        "JSON manifest must have 'downloaded' array"
    );
}

// ---------------------------------------------------------------------------
// AC-019 — EC-2.7.008-10 / EC-2.7.009-3: filtered-to-zero hint
// ---------------------------------------------------------------------------

/// AC-019 / EC-2.7.008-10 / EC-2.7.009-3: filter eliminates all attachments → hint + exit 0.
#[tokio::test]
async fn test_bc_2_7_008_filtered_to_zero_hint() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    // Issue with one PNG — filter for PDF matches nothing
    let att = make_attachment(
        "93001",
        "image.png",
        "image/png",
        100,
        "2026-07-10T14:00:00.000+0000",
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FILTERED-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_with_attachments("FILTERED-1", vec![att])),
        )
        .mount(&server)
        .await;

    // Human mode: "No attachments matched the filter on FILTERED-1."
    let out_human = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FILTERED-1",
            "--all",
            "--filter",
            "mime=application/pdf",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 → success() fails.
    assert!(
        out_human.status.success(),
        "filtered-to-zero must exit 0 (EC-2.7.008-10)"
    );
    let stderr = String::from_utf8_lossy(&out_human.stderr);
    assert!(
        stderr.contains("No attachments matched the filter on FILTERED-1."),
        "human mode must emit filter hint — got: {stderr}"
    );

    // JSON mode: {"downloaded":[]} on stdout, NO hint on stderr
    let out_json = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FILTERED-1",
            "--all",
            "--filter",
            "mime=application/pdf",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    // RED GATE: todo!() → exit 101 → success() fails.
    assert!(
        out_json.status.success(),
        "filtered-to-zero JSON mode must exit 0"
    );
    let stdout = String::from_utf8_lossy(&out_json.stdout);
    let manifest: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON — {e}\nstdout: {stdout}"));
    assert_eq!(
        manifest["downloaded"],
        serde_json::json!([]),
        "JSON mode filtered-to-zero must return {{\"downloaded\":[]}}"
    );
    let stderr_j = String::from_utf8_lossy(&out_json.stderr);
    assert!(
        !stderr_j.contains("No attachments matched"),
        "JSON mode must NOT emit filter hint to stderr — got: {stderr_j}"
    );
}

// ---------------------------------------------------------------------------
// P1-005 — BC-2.7.010 R3.10: single-id degenerate-name fallback + exact canonical warning
// ---------------------------------------------------------------------------

/// P1-005 / BC-2.7.010 ~872: single-id `--id` with a filename that sanitizes to None
/// (e.g. "..") must use the attachment id as the filename AND emit the exact canonical
/// warning with em-dash U+2014, "original name", single-quoted raw name, and trailing period.
///
/// Current impl: no warning emitted in `handle_single_download` (only batch path has it) → RED.
#[tokio::test]
async fn test_bc_2_7_010_single_id_degenerate_name_fallback() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let cwd = TempDir::new().unwrap();

    // filename=".." → sanitize_attachment_filename returns None → id "92010" used as filename.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/92010"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "92010",
            "filename": "..",
            "size": 4,
            "mimeType": "text/plain",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/92010"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"data"))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .current_dir(cwd.path())
        .args(["issue", "attachment", "download", "FOO-1", "--id", "92010"])
        .output()
        .unwrap();

    // RED GATE: no warning emitted → fails on the exact-canonical-warning assertion below.
    assert!(
        output.status.success(),
        "single-id degenerate-name download must succeed (exit 0)"
    );

    // File must exist at CWD/"92010" (bare id as filename, no SHA-1 prefix in single-id path).
    assert!(
        cwd.path().join("92010").exists(),
        "single-id degenerate fallback must write file named '92010' (the attachment id)"
    );

    // Exact canonical warning per BC-2.7.010 ~872: em-dash U+2014, "original name",
    // single-quoted raw name, trailing period.
    // Current impl: no warning in handle_single_download → assertion fails → RED.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "warning: using id as filename for attachment 92010 \u{2014} original name '..' could not be sanitized."
        ),
        "single-id must emit exact canonical degenerate-name warning (BC-2.7.010 ~872) — got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// P1-006 — BC-2.7.011 VP-576-001: containment property via integration proptest
// ---------------------------------------------------------------------------

// VP-576-001 / BC-2.7.011 ~934: for any Some(name) produced by sanitize_attachment_filename,
// resolved_dir.join(&name).starts_with(&resolved_dir) must hold (path-traversal containment).
//
// The src/ unit proptest lives in src/cli/issue/attachments.rs #[cfg(test)].
// This integration-level proptest pins the same invariant from the public API surface,
// ensuring it holds for any input string (including path-traversal attempts).
//
// Expected status: GREEN (sanitize_attachment_filename is correctly implemented).
// Role: regression guard — any future breakage in the sanitizer would be caught here.
proptest! {
    #[test]
    fn test_bc_2_7_011_vp576_001_containment_prop(name in any::<String>()) {
        if let Some(sanitized) = jr::cli::issue::attachments::sanitize_attachment_filename(&name) {
            // Use the OS temp dir as a fixed canonical base (avoids TempDir per proptest case).
            let resolved_dir = std::env::temp_dir()
                .canonicalize()
                .expect("temp_dir() must canonicalize");
            let joined = resolved_dir.join(&sanitized);
            prop_assert!(
                joined.starts_with(&resolved_dir),
                "BC-2.7.011 containment violation: \
                 {:?}.starts_with({:?}) is false; sanitized={:?} from input={:?}",
                joined,
                resolved_dir,
                sanitized,
                name
            );
        }
    }
}

// ---------------------------------------------------------------------------
// P1-009 — BC-2.7.008 ~797: batch collision-skip → exact canonical message + exit 0
// ---------------------------------------------------------------------------

/// P1-009 / BC-2.7.008 ~797: when a batch download (`--all`) encounters a file that
/// already exists in the output directory and `--force` is NOT set, it must:
///   1. Skip the file (exit 0 — collision is NON-ERROR per BC-2.7.008 ~797).
///   2. Emit the exact canonical warning:
///      "Skipping <filename>: file already exists. Use --force to overwrite."
///
/// Current impl (line 782): "warning: skipping existing file: <path>" (wrong format) → RED.
#[tokio::test]
async fn test_bc_2_7_008_batch_collision_skip_no_force() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    let att = make_attachment(
        "95001",
        "collision.txt",
        "text/plain",
        5,
        "2026-07-10T14:00:00.000+0000",
    );

    // Issue GET — responds to multiple calls (both the first and second run need it).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/COLLIDE-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_with_attachments("COLLIDE-1", vec![att])),
        )
        .mount(&server)
        .await;

    // Content GET — expect exactly 1 call: the first run downloads, the second skips.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/95001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello"))
        .expect(1)
        .mount(&server)
        .await;

    // First run: download the file so it exists in out_dir.
    let first_run = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "COLLIDE-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    // RED GATE: if handler is todo!() or broken, first run fails → test fails here.
    assert!(
        first_run.status.success(),
        "first run must succeed (pre-condition for collision skip test)"
    );

    // Capture the SHA-1-prefixed filename created on the first run.
    let entries: Vec<_> = std::fs::read_dir(out_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "exactly one file must exist in out_dir after first run"
    );
    let created_filename = entries[0].file_name().to_string_lossy().into_owned();

    // Second run: same --all without --force → collision-skip + exit 0.
    let second_run = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "COLLIDE-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: wrong skip message → assertion fails.
    // Also: .expect(1) on content GET enforces the second run does NOT re-download.
    assert!(
        second_run.status.success(),
        "collision-skip must exit 0 (BC-2.7.008 ~797: collision is NON-ERROR)"
    );
    let stderr = String::from_utf8_lossy(&second_run.stderr);
    // Exact canonical collision-skip message per BC-2.7.008 ~797.
    // Current impl: "warning: skipping existing file: <path>" (wrong format) → RED.
    assert!(
        stderr.contains(&format!(
            "Skipping {created_filename}: file already exists. Use --force to overwrite."
        )),
        "collision-skip stderr must match exact canonical format \
         'Skipping <filename>: file already exists. Use --force to overwrite.' \
         (BC-2.7.008 ~797) — got: {stderr}"
    );
}
