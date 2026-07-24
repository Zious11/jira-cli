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

    // Non-numeric --id → handler must exit 64 + exact canonical "invalid attachment id: '<VALUE>' (must be numeric)"
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
    // RED GATE: impl emits "invalid attachment id: 'not-a-number' — must be a numeric Jira attachment ID."
    // Missing "(must be numeric)" suffix required by BC-2.7.007 ~735 → assertion fails.
    assert_eq!(
        out_bad.status.code(),
        Some(64),
        "non-numeric --id must exit 64"
    );
    let stderr = String::from_utf8_lossy(&out_bad.stderr);
    assert!(
        stderr.contains("invalid attachment id: 'not-a-number' (must be numeric)"),
        "stderr must contain exact canonical 'invalid attachment id: \\'not-a-number\\' (must be numeric)' (BC-2.7.007 ~735) — got: {stderr}"
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
    let combined_c = format!("{}{}", String::from_utf8_lossy(&out_c.stdout), stderr_c);
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
    let combined_d = format!("{}{}", String::from_utf8_lossy(&out_d.stdout), stderr_d);
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

    // invalid AID (non-numeric) → exit 64 + exact canonical "invalid attachment id: '<VALUE>' (must be numeric)"
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
    // RED GATE: impl emits "invalid attachment id: 'not-numeric' — must be a numeric Jira attachment ID."
    // Missing "(must be numeric)" suffix required by BC-2.7.012 ~960 → assertion fails.
    assert_eq!(
        out_inv.status.code(),
        Some(64),
        "non-numeric AID must exit 64"
    );
    assert!(
        String::from_utf8_lossy(&out_inv.stderr)
            .contains("invalid attachment id: 'not-numeric' (must be numeric)"),
        "non-numeric AID stderr must contain exact canonical 'invalid attachment id: \\'not-numeric\\' (must be numeric)' (BC-2.7.012 ~960)"
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
    // ENOSPC: not deterministically triggerable in CI (requires filling a real disk).
    // Its canonical string is tested at the pure-classifier level inside
    // src/cli/issue/attachments.rs::tests (test_bc_2_7_012_classify_storage_full_*
    // and test_bc_2_7_012_classify_quota_exceeded_*).
    //
    // EACCES: covered by the dedicated integration test below —
    // test_bc_2_7_012_eacces_permission_denied_error_message (FIX-F5-010).
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

/// Raw TCP server that handles attachment requests, sending partial body on the
/// content endpoint to drive `stream_to_file`'s mid-stream-error + cleanup branch.
///
/// hyper 1.x panics server-side on Content-Length mismatch (role.rs:704), so a
/// wiremock ResponseTemplate with mismatched content-length cannot be used.  Instead
/// this raw server sends `Content-Length: 99999` in raw HTTP/1.1 headers but only
/// writes 7 bytes of body before closing — no hyper validation involved.
///
/// Returns `"http://127.0.0.1:PORT"` for use as `JR_BASE_URL`.
fn start_partial_stream_server() -> String {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", listener.local_addr().unwrap());

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut conn) => {
                    let mut buf = [0u8; 4096];
                    let n = match conn.read(&mut buf) {
                        Ok(n) if n > 0 => n,
                        _ => continue,
                    };
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let first_line = req.lines().next().unwrap_or("");

                    if first_line.contains("/attachment/content/") {
                        // Content GET: return 200 with Content-Length: 99999 but only
                        // 7 bytes of body ("partial"), then close the connection.
                        // reqwest reads the 7-byte chunk, then hits EOF while still
                        // expecting (99999 − 7) more bytes → stream error in chunk iterator.
                        let _ = conn.write_all(
                            b"HTTP/1.1 200 OK\r\n\
                              Content-Type: application/octet-stream\r\n\
                              Content-Length: 99999\r\n\
                              Connection: close\r\n\r\n\
                              partial",
                        );
                        // `conn` dropped here → FIN sent → reqwest gets incomplete body
                    } else {
                        // Metadata GET: return full valid JSON response.
                        let body = r#"{"id":"70001","filename":"partial.bin","size":99999,"mimeType":"application/octet-stream"}"#;
                        let header = format!(
                            "HTTP/1.1 200 OK\r\n\
                             Content-Type: application/json\r\n\
                             Content-Length: {}\r\n\
                             Connection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = conn.write_all(header.as_bytes());
                        let _ = conn.write_all(body.as_bytes());
                    }
                }
                Err(_) => break,
            }
        }
    });

    addr
}

/// AC-011 / BC-2.7.007: atomic rename from temp file; cleanup on mid-stream error.
///
/// Drives the `if result.is_err() { remove_file(&tmp_path) }` cleanup branch in
/// `stream_to_file` (attachments.rs ~594-596).  Uses `start_partial_stream_server`
/// (raw TCP, bypasses hyper) to return 200 OK + partial body + close so that:
///   1. `get_attachment_content` returns `Ok(Response)` (200 headers received)
///   2. `stream_to_file` is entered and creates the temp file
///   3. A mid-stream EOF error fires in the chunk iterator
///   4. The cleanup branch deletes the temp file
///
/// A plain 4xx/5xx response errors in `send_inner` BEFORE `stream_to_file` is
/// called and is vacuous for this specific BC clause.
#[tokio::test]
async fn test_bc_2_7_007_atomic_rename_cleanup_on_error() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let final_path = out_dir.path().join("partial.bin");

    // Raw TCP server: metadata GET returns full response, content GET returns
    // partial body then closes (drives mid-stream stream error).
    let server_addr = start_partial_stream_server();

    let output = jr_cmd_with_xdg(&server_addr, cache.path(), config.path())
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "mid-stream error must exit 1 (EC-2.7.007-4); stderr: {stderr}"
    );
    assert!(
        !final_path.exists(),
        "final path must NOT exist after stream error (no partial file left behind)"
    );
    // No tmp_ files must remain: proves the cleanup branch ran and deleted the temp file.
    let tmp_files: Vec<_> = std::fs::read_dir(out_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("tmp_"))
        .collect();
    assert!(
        tmp_files.is_empty(),
        "no tmp_ files must remain after error (EC-2.7.007-4 cleanup required); found: {:?}",
        tmp_files
            .iter()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    );
    // Prove stream_to_file was entered: stderr must reference a stream/body/connection error,
    // not an API-level error from send_inner (which would say "500 Internal Server Error").
    assert!(
        stderr.contains("stream error")
            || stderr.contains("body error")
            || stderr.contains("incomplete")
            || stderr.contains("connection")
            || stderr.contains("reset"),
        "stderr must reference a mid-stream/body failure (proves stream_to_file branch was \
         entered, not just the send_inner error path); got: {stderr}"
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
// P8-001 — BC-2.7.011 ~936: success hint display-sanitizes filename (CWE-116)
// ---------------------------------------------------------------------------

/// P8-001 / BC-2.7.011 ~936: the single-id success hint (`Downloaded: <path> (<size>).`)
/// MUST pass the filename portion through `display_sanitize_filename` (CWE-116
/// every-call-site clause) before emitting to stderr.
///
/// Fixture: metadata returns `filename: "evil\u{7f}name.txt"` — contains:
///   - U+007F (DEL, cp 0x7F, matches `cp == 0x7F` in `display_sanitize_filename` → `?`)
///
/// U+202E (BiDi RLO) and CR (`\r`, 0x0D) are both rejected by the GitHub Actions
/// Windows runner with OS error 123 (InvalidFilename). U+007F (DEL = 127) is NOT in
/// the Windows forbidden range (1–31) and passes NTFS validation on all CI platforms.
///
/// Expected hint: `Downloaded: <dir>/evil?name.txt (3 B).`
///
/// `sanitize_attachment_filename` disk-variant keeps U+007F (it is NOT in the scrub
/// set: `/`, `\`, `:`), so the ON-DISK file is named with the raw DEL char.
///
/// RED: current impl uses `final_path.display()` raw → hint emits raw U+007F.
#[tokio::test]
async fn test_bc_2_7_007_success_hint_display_sanitizes_filename() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    // subprocess CWD: default-path mode (no --out) → file lands in CWD.
    let cwd_dir = TempDir::new().unwrap();

    // U+007F (DEL = 127, 0x7F) is valid on both NTFS and ext4: it is NOT in the Windows
    // forbidden range (chars 1–31 are forbidden; 0x7F = 127 is not). U+202E (BiDi RLO)
    // and CR (\r, 0x0D) are both rejected by the Windows runner (OS error 123).
    let poisoned_filename = "evil\u{7f}name.txt";
    let display_safe = "evil?name.txt"; // display_sanitize_filename replaces U+007F with '?'

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/96001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "96001",
            "filename": poisoned_filename,
            "size": 3,
            "mimeType": "text/plain",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/96001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"abc"))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .current_dir(cwd_dir.path()) // default-path: file lands here
        .args(["issue", "attachment", "download", "HINT-1", "--id", "96001"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "download with poisoned filename must succeed (exit 0); stderr: {stderr}"
    );

    // BC-2.7.011 ~936: every call site must display-sanitize. RED against current impl
    // which uses final_path.display() raw.
    assert!(
        !stderr.contains('\u{7f}'),
        "P8-001: raw U+007F (DEL, 0x7F) MUST NOT appear in Downloaded: hint \
         (BC-2.7.011 every-call-site CWE-116 clause); got: {stderr:?}"
    );
    assert!(
        stderr.contains(display_safe),
        "P8-001: hint must contain display-sanitized filename '{display_safe}'; \
         got: {stderr:?}"
    );

    // On-disk file keeps the disk-variant name (sanitize_attachment_filename does NOT
    // scrub U+007F — it is not /, \\, :, or NUL).
    let disk_file = cwd_dir.path().join(poisoned_filename);
    assert!(
        disk_file.exists(),
        "P8-001: on-disk file must use disk-variant name (raw chars preserved on disk); \
         expected: {disk_file:?}"
    );

    // Mutant 15 (760:28 delete!/always-false): success hint must include the parent dir path.
    // Under delete! guard: `Some(d) if d.is_empty()` → fires only for empty parent →
    // non-empty CWD parent → `_ => fname` (bare filename, no parent dir) → assertion fails.
    // Under always-false: guard never fires → same bare-fname result → assertion fails.
    assert!(
        stderr.contains(cwd_dir.path().to_str().unwrap()),
        "P8-001: success hint must contain parent dir path (non-empty parent guard at ~760); \
         got: {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// P9-001 — BC-2.7.011 / CWE-116: rename-failure error display-sanitizes filename
// ---------------------------------------------------------------------------

/// P9-001 (CWE-116 error-path sibling of P8-001): `stream_to_file` rename-failure
/// branch must display-sanitize the filename in the error message.
///
/// Route: default-path (no `--out`) + pre-create DIRECTORY at the final path + `--force`.
///   - EC-11 is-directory pre-flight only fires for `--out` case → SKIPPED here.
///   - Collision check (`out.is_none() && final_path.exists() && !force`): `!force` is
///     false with `--force` → SKIPPED.
///   - `stream_to_file` enters: creates temp file, writes body, `rename(tmp, dir)` → EISDIR.
///   - Error: `"failed to rename temp to <CWD>/evil\u{7f}name.txt: Is a directory"`.
///
/// `sanitize_attachment_filename("evil\u{7f}name.txt")` keeps U+007F on disk
/// (not in the scrub set: `/`, `\`, `:`). So the pre-created DIRECTORY uses the raw char.
/// U+202E (BiDi RLO) and CR (`\r`, 0x0D) are both rejected by the Windows runner
/// (OS error 123, InvalidFilename). U+007F (DEL = 127) is NOT in the Windows forbidden
/// range (1–31) and is valid on all CI platforms.
///
/// RED: current impl uses `final_path.display()` raw at `stream_to_file` ~587 →
/// error message emits raw U+007F (DEL).
#[tokio::test]
async fn test_bc_2_7_007_rename_failure_error_display_sanitizes_filename() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    // subprocess CWD: default-path (no --out) → final_path is CWD/<sanitized>.
    let cwd_dir = TempDir::new().unwrap();

    // U+007F (DEL = 127, 0x7F) is valid on both NTFS and ext4: NOT in Windows forbidden
    // range (1-31). U+202E (BiDi RLO) and CR (\r, 0x0D) are rejected by Windows runner.
    let poisoned_filename = "evil\u{7f}name.txt";
    // display_sanitize_filename: U+007F (DEL, cp 0x7F, matches cp == 0x7F) → '?'
    let display_safe = "evil?name.txt";

    // Pre-create a DIRECTORY at the on-disk path so rename(tmp, dir) → EISDIR.
    // sanitize_attachment_filename keeps U+007F (it is not /, \, :, or NUL),
    // so the disk path is cwd_dir / "evil\u{7f}name.txt" verbatim.
    std::fs::create_dir(cwd_dir.path().join(poisoned_filename))
        .expect("P9-001 setup: failed to create pre-existing directory at final path");

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/97001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "97001",
            "filename": poisoned_filename,
            "size": 3,
            "mimeType": "text/plain",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/97001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"xyz"))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .current_dir(cwd_dir.path())
        .args([
            "issue",
            "attachment",
            "download",
            "RF-1",
            "--id",
            "97001",
            "--force",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must fail: rename of temp file to a directory → EISDIR.
    assert!(
        !output.status.success(),
        "P9-001: rename to a pre-created directory must cause exit != 0; stderr: {stderr}"
    );

    // BC-2.7.011 ~936: every call site must display-sanitize. RED against current impl
    // which uses `final_path.display()` raw in the rename-failure anyhow message (~587).
    assert!(
        !stderr.contains('\u{7f}'),
        "P9-001: raw U+007F (DEL, 0x7F) MUST NOT appear in rename-failure error \
         (BC-2.7.011 every-call-site CWE-116 clause); got: {stderr:?}"
    );
    assert!(
        stderr.contains(display_safe),
        "P9-001: error must contain display-sanitized filename '{display_safe}'; \
         got: {stderr:?}"
    );

    // Cleanup guarantee (BC-2.7.007 EC-2.7.007-4): temp file removed on error.
    let tmp_files: Vec<_> = std::fs::read_dir(cwd_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("tmp_"))
        .collect();
    assert!(
        tmp_files.is_empty(),
        "P9-001: temp file not cleaned up after rename failure: {tmp_files:?}"
    );

    // Mutants 8,10 (602:32 delete!/always-false): rename-failure error must include the
    // parent dir path.  Under delete! guard (`Some(d) if d.is_empty()`), the non-empty
    // CWD parent fails the guard → `_ => fname` (bare filename) → assertion fails.
    // Under always-false: same → assertion fails.
    assert!(
        stderr.contains(cwd_dir.path().to_str().unwrap()),
        "P9-001: rename-failure error must contain parent dir path (non-empty parent guard at ~602); \
         got: {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// PG-F4-10 mutation-kill integration tests
// ---------------------------------------------------------------------------

/// Mutant 1 (api/jira/attachments.rs 153:59): `*status == 403` → `true`.
///
/// Under the mutant, ANY `ApiError` status triggers the 403-specific
/// "Permission denied: cannot access attachment" branch.  A 400 response
/// must NOT produce that message.
#[tokio::test]
async fn test_bc_2_7_metadata_400_not_permission_denied() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/98001"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": ["bad request"],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args(["issue", "attachment", "download", "TEST-1", "--id", "98001"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "400 metadata response must cause non-zero exit; stderr: {stderr}"
    );
    // Under the → true mutant, a 400 would produce "Permission denied" in the error.
    assert!(
        !stderr.contains("Permission denied"),
        "400 metadata response must NOT produce 'Permission denied' (only 403 should); \
         got: {stderr:?}"
    );
}

/// Mutants 12,14 (721:24 delete!/always-false in default-path collision error format).
///
/// Default-path collision error must include the full parent dir path (CWD), not just the
/// bare filename.  Under delete! or always-false, the guard `!d.as_os_str().is_empty()`
/// never fires for non-empty CWD → falls to `_ => fname` → bare name only in error.
#[tokio::test]
async fn test_bc_2_7_single_download_collision_error_includes_parent_dir() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let cwd_dir = TempDir::new().unwrap();

    let filename = "collision.txt";

    // Pre-create the file at cwd_dir/collision.txt so the collision check fires.
    std::fs::write(cwd_dir.path().join(filename), b"existing").unwrap();

    // Metadata GET fires before the collision check (no content GET needed).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/98002"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "98002",
            "filename": filename,
            "size": 8,
            "mimeType": "text/plain",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .current_dir(cwd_dir.path())
        .args(["issue", "attachment", "download", "COLL-1", "--id", "98002"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "default-path collision must exit 64; stderr: {stderr}"
    );
    // Error must include the parent dir path (CWD), not just the bare filename.
    assert!(
        stderr.contains(cwd_dir.path().to_str().unwrap()),
        "collision error must contain parent dir path; got: {stderr:?}"
    );
    assert!(
        stderr.contains(filename),
        "collision error must contain the filename; got: {stderr:?}"
    );
}

/// Mutant 11 (644:25 bare-filename parent guard) and mutant 15 always-true (760:28).
///
/// Mutant 11: `pp == Path::new("")` → false (always-false guard).  For `--out "doc.txt"`,
/// parent is `Some("")` → guard fails → falls to `Some(pp) => pp.to_path_buf()` →
/// effective_parent = `""` → `"".exists()` = false → "Output directory does not exist" exit 64.
/// Original: guard fires → effective_parent = CWD → exists → success.
///
/// Mutant 15 always-true (760:28): `!d.as_os_str().is_empty()` → `true`.
/// For bare `--out "doc.txt"`, `final_path.parent() = Some("")` → always-true guard fires →
/// `format!("{}{}{fname}", "".display(), MAIN_SEPARATOR)` = `"/doc.txt"` → hint
/// = `"Downloaded: /doc.txt ..."`.
/// Original: guard fails for empty parent → `_ => fname` → `"doc.txt"` (no separator).
#[tokio::test]
async fn test_bc_2_7_bare_out_filename_success_hint_no_leading_separator() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let cwd_dir = TempDir::new().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/98003"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "98003",
            "filename": "doc.txt",
            "size": 5,
            "mimeType": "text/plain",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/98003"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello"))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .current_dir(cwd_dir.path())
        .args([
            "issue",
            "attachment",
            "download",
            "BARE-1",
            "--id",
            "98003",
            "--out",
            "doc.txt", // bare filename — no directory component
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Mutant 11 (guard → false): effective_parent="" → doesn't exist → exit 64.
    // Original: CWD → exists → success.
    assert!(
        output.status.success(),
        "bare --out 'doc.txt' must succeed (parent '' → CWD exists); stderr: {stderr}"
    );

    // Mutant 15 always-true (760:28): empty parent "" → format!("" + "/" + "doc.txt") =
    // "/doc.txt" → hint = "Downloaded: /doc.txt (...)".
    // Original: empty parent guard fails → `_ => "doc.txt"` → "Downloaded: doc.txt (...)".
    assert!(
        !stderr.contains("/doc.txt"),
        "bare --out hint must NOT have a leading path separator before the filename \
         (empty-parent guard at ~760); got: {stderr:?}"
    );
    assert!(
        stderr.contains("Downloaded: doc.txt"),
        "bare --out hint must start with 'Downloaded: doc.txt'; got: {stderr:?}"
    );

    // File must exist at cwd_dir/doc.txt.
    assert!(
        cwd_dir.path().join("doc.txt").exists(),
        "doc.txt must be created in CWD for bare --out path"
    );
}

/// Mutants 16,17 (918:28): `fail_count += 1` in `stream_to_file` error branch.
///
/// Under `*= 1`: `fail_count = 0 * 1 = 0` → never triggers `fail_count > 0` → exit 0.
/// Under `-=`:   `0 - 1` usize underflow → panic in debug → exit 101.
/// Both fail: test asserts exit code = 1 and canonical "Downloaded 0 of 1" summary.
///
/// Route: batch `--all` with one attachment.  Content-GET returns 200, but a DIRECTORY
/// is pre-created at the computed batch output path → `rename(tmp, dir)` = EISDIR →
/// `stream_to_file` returns `Err` → line 918 `fail_count += 1` fires.
#[tokio::test]
async fn test_bc_2_7_stream_to_file_failure_increments_fail_count() {
    use sha1::Digest as _;

    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    let att_id = "94001";
    let att_filename = "payload.bin";

    // Compute the batch output path: {sha1_hex(att_id)}_{sanitized_filename}.
    // Mirrors compute_default_output_path in src/cli/issue/attachments.rs (batch-only).
    let hash: String = sha1::Sha1::new()
        .chain_update(att_id.as_bytes())
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let batch_path = out_dir.path().join(format!("{hash}_{att_filename}"));

    // Pre-create a DIRECTORY at the batch path so rename(tmp, dir) → EISDIR.
    std::fs::create_dir(&batch_path)
        .expect("pre-create directory at batch path for EISDIR trigger");

    let att = make_attachment(
        att_id,
        att_filename,
        "application/octet-stream",
        4,
        "2026-07-19T10:00:00.000+0000",
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/STFAIL-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments("STFAIL-1", vec![att])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/attachment/content/{att_id}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"data"))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "STFAIL-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
            "--force", // bypass batch collision-skip so stream_to_file is reached
        ])
        .output()
        .unwrap();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Under *=1 mutant: fail_count=0 → exit 0 (no error return) → code ≠ 1 → fails.
    // Under -= mutant:  usize underflow panic in debug → exit 101 ≠ 1 → fails.
    assert_eq!(
        output.status.code(),
        Some(1),
        "stream_to_file failure must cause exit 1 (fail_count > 0); output: {combined}"
    );
    // Canonical "Downloaded N of M" summary confirms the batch completed with 0 successes.
    assert!(
        combined.contains("Downloaded 0 of 1 attachments to"),
        "batch must report 0 of 1 on stream_to_file failure; got: {combined}"
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
// P4-001 — BC-2.7.008 fail-soft spirit: sparse AttachmentObject tolerance
// ---------------------------------------------------------------------------

/// P4-001 residual / BC-2.7.008 fail-soft spirit: `--all` must NOT abort the whole batch
/// when the issue-GET response contains a sparse `AttachmentObject` (a field that is non-Option
/// in the current struct is absent from the JSON wire body).
///
/// Chosen sparse field: `mimeType` — optional in practice; Jira can omit it for certain
/// attachment types (restricted, deleted, third-party integrations). The current struct has
/// `mime_type: String` (non-Option, no `#[serde(default)]`) → serde fails with
/// `missing field "mimeType"` → `list_attachments` returns an error → batch aborts (exit ≠ 0,
/// no downloads).
///
/// Fixture:
///   AID 70001: SPARSE — `mimeType` omitted; all other fields (id, filename, size, created,
///              self, content) present.
///   AID 70002: NORMAL — fully-populated attachment; must be downloaded in every graceful path.
///
/// Expected behavior after fix (BC-2.7.008 fail-soft spirit):
///   Either: make `mime_type: Option<String>` or add `#[serde(default)]` so the sparse
///   attachment deserializes without error and the batch proceeds.
///   OR: filter the malformed entry at list level and process the rest (skip is non-error
///   for the batch when the normal attachment still succeeds).
///   In both cases: exit 0 + AID 70002 in manifest + no "missing field" in stderr.
///
/// RED against current impl: `mime_type: String` aborts list deserialization →
///   exit 1 + "missing field" in stderr + no manifest output.
#[tokio::test]
async fn test_bc_2_7_008_sparse_attachment_object_tolerated() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    // Sparse attachment: mimeType intentionally omitted.
    // All other required fields are present so the issue is the missing mimeType only.
    let sparse = serde_json::json!({
        "id": "70001",
        "filename": "sparse_no_mime.txt",
        // "mimeType": intentionally absent — tests tolerance of AttachmentObject
        "size": 4,
        "created": "2026-07-10T14:00:00.000+0000",
        "author": null,
        "self": "https://example.atlassian.net/rest/api/3/attachment/70001",
        "content": "https://example.atlassian.net/rest/api/3/attachment/content/70001",
    });

    // Normal attachment: fully populated.
    let normal = make_attachment(
        "70002",
        "normal.txt",
        "text/plain",
        4,
        "2026-07-10T15:00:00.000+0000",
    );

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/SPARSE-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_with_attachments("SPARSE-1", vec![sparse, normal])),
        )
        .mount(&server)
        .await;

    // Content mock for sparse attachment (if the fix makes it download, it should succeed).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/70001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"sparse_content"))
        .mount(&server)
        .await;

    // Content mock for normal attachment — must always be downloaded.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/70002"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"normal_content"))
        .expect(1) // must be called: if batch aborts (serde error), this fires → RED
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "SPARSE-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    // RED GATE: current `mime_type: String` (non-Option) causes serde to fail on the
    // entire IssueAttachmentResponse → list_attachments returns error → batch aborts.
    // After fix (make mime_type Option<String> or #[serde(default)]): exit 0 + manifest.
    assert!(
        output.status.success(),
        "sparse-attachment batch must NOT abort (BC-2.7.008 fail-soft spirit) — exit: {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Serde missing-field error must NOT appear — it signals batch-aborting parse failure.
    assert!(
        !stderr.contains("missing field"),
        "batch must NOT emit serde 'missing field' error (AbortObject → BC-2.7.008 violation) — got: {stderr}"
    );

    // Normal attachment must appear in the manifest — proves the batch was not aborted.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON — {e}\nstdout: {stdout}"));
    let downloaded = manifest["downloaded"].as_array().unwrap();
    let ids: Vec<&str> = downloaded
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&"70002"),
        "normal attachment (70002) must appear in manifest after sparse-tolerant batch — got: {ids:?}"
    );
}

// ---------------------------------------------------------------------------
// P3-003 — AC-008 / BC-2.7.009: filter-then-newest composition (regression pin)
// ---------------------------------------------------------------------------

/// P3-003 / AC-008 / BC-2.7.009: `--filter X --newest N` must apply filter FIRST, then select
/// the top-N from the filtered set (filter-before-select order, story ~283).
///
/// Fixture design — the chronologically-NEWEST attachment does NOT match the filter:
///   AID 55001  2026-07-10T14:00Z  mime=text/plain   ← matches filter, OLDER of filtered set
///   AID 55002  2026-07-10T16:00Z  mime=text/plain   ← matches filter, NEWEST of filtered set
///   AID 55003  2026-07-10T18:00Z  mime=image/png    ← NEWEST OVERALL but does NOT match filter
///
/// With `--filter mime=text/plain --newest 1`:
///   filter-then-newest (CORRECT): filtered=[55001,55002] → newest=55002 → download 55002
///   newest-then-filter (WRONG):   newest=[55003] → filter=[nothing] → zero downloads
///
/// .expect(0) on 55003 content mock + .expect(1) on 55002 ensure an order-swap
/// regression fails at mock-server drop.
///
/// Expected status: GREEN — the implementation applies filter before newest truncation
/// (src/cli/issue/attachments.rs::handle_batch_download, filter pass ~line 736 precedes
/// newest truncation ~line 755). This is a regression pin per AC-008.
#[tokio::test]
async fn test_bc_2_7_009_filter_then_newest_composition() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    let atts = vec![
        make_attachment(
            "55001",
            "older_plain.txt",
            "text/plain",
            4,
            "2026-07-10T14:00:00.000+0000", // oldest, matches --filter mime=text/plain
        ),
        make_attachment(
            "55002",
            "newer_plain.txt",
            "text/plain",
            4,
            "2026-07-10T16:00:00.000+0000", // middle, matches filter — NEWEST of filtered set
        ),
        make_attachment(
            "55003",
            "newest_png.png",
            "image/png",
            4,
            "2026-07-10T18:00:00.000+0000", // NEWEST OVERALL but does NOT match text/plain filter
        ),
    ];

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FILTNEW-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments("FILTNEW-1", atts)),
        )
        .mount(&server)
        .await;

    // Correct answer: 55002 must be downloaded (newest of text/plain-filtered set).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/55002"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"newer_plain"))
        .expect(1) // must be called exactly once
        .mount(&server)
        .await;

    // Wrong answer: 55003 must NOT be downloaded — it is filtered out by --filter mime=text/plain.
    // If newest-then-filter order were used, 55003 would be selected and this would fire → RED.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/55003"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"newest_png"))
        .expect(0) // AC-008: filter-before-select; 55003 must never be content-fetched
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FILTNEW-1",
            "--newest",
            "1",
            "--filter",
            "mime=text/plain",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "--filter mime=text/plain --newest 1 must succeed (exit 0)"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON — {e}\nstdout: {stdout}"));
    let downloaded = manifest["downloaded"].as_array().unwrap();
    assert_eq!(
        downloaded.len(),
        1,
        "--filter + --newest 1 must download exactly 1 attachment"
    );
    let ids: Vec<&str> = downloaded
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    // 55002 is the newest of the text/plain-filtered set (AC-008 filter-before-select).
    assert!(
        ids.contains(&"55002"),
        "newest of filtered set (55002, text/plain) must be selected \
         (AC-008: filter-before-select) — got: {ids:?}"
    );
    // 55003 is the newest overall but does not pass the text/plain filter.
    assert!(
        !ids.contains(&"55003"),
        "overall-newest (55003, image/png) must NOT be selected \
         (filtered out by mime=text/plain) — got: {ids:?}"
    );
    // 55001 is filtered in but not selected (N=1 and 55002 is newer).
    assert!(
        !ids.contains(&"55001"),
        "older text/plain (55001) must NOT be selected (newest-1 of filtered is 55002) — got: {ids:?}"
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

// ---------------------------------------------------------------------------
// FIX-576-DL — mock-vs-live drift: metadata GET returns integer id on live Cloud
// ---------------------------------------------------------------------------

/// FIX-576-DL / BC-2.7.007: `GET /rest/api/3/attachment/{id}` returns `"id"` as
/// an **integer** on live Jira Cloud (e.g. `10008`), while the issue-fields
/// attachment list endpoint returns string IDs.  The S-576-2 mocks used string
/// IDs throughout; this test pins the integer-id wire shape so the serde
/// deserializer is verified against the live-faithful response.
///
/// Discovered via S-576-6 live validation run 30031724733:
/// `Error: invalid type: integer \`10008\`, expected a string at line 1 column 11`
#[tokio::test]
async fn test_download_integer_id_in_metadata_succeeds() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Live-faithful shape: `id` is an INTEGER (not a string).
    // Before the fix this causes: "invalid type: integer `10008`, expected a string"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10008"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 10008,
            "filename": "live_doc.pdf",
            "size": 7,
            "mimeType": "application/pdf",
            "content": format!("{}/rest/api/3/attachment/content/10008", server.uri()),
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/10008"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"pdfdata"))
        .mount(&server)
        .await;

    let out_dir = TempDir::new().unwrap();
    let out_path = out_dir.path().join("live_doc.pdf");

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10008",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED (before fix): serde error "invalid type: integer `10008`, expected a string"
    //   → binary exits 1 with an API/parse error.
    // GREEN (after fix): AttachmentMetadata.id accepts both string and integer forms.
    assert!(
        output.status.success(),
        "download with integer id in metadata must succeed (exit 0) — FIX-576-DL; \
         stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_path.exists(), "output file must be created");
    assert_eq!(
        std::fs::read(&out_path).unwrap(),
        b"pdfdata",
        "file content must match mocked body"
    );
}

/// FIX-576-DL / BC-2.7.007: `id` as string still works after the fix
/// (regression guard — existing mocks and the issue-fields list path use strings).
#[tokio::test]
async fn test_download_string_id_in_metadata_still_succeeds() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // String id — must continue to work after the deserializer change.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10009"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10009",
            "filename": "string_id.txt",
            "size": 3,
            "mimeType": "text/plain",
            "content": format!("{}/rest/api/3/attachment/content/10009", server.uri()),
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/10009"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"txt"))
        .mount(&server)
        .await;

    let out_dir = TempDir::new().unwrap();
    let out_path = out_dir.path().join("string_id.txt");

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10009",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "download with string id in metadata must still succeed (exit 0) — regression guard; \
         stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_path.exists(), "output file must be created");
    assert_eq!(std::fs::read(&out_path).unwrap(), b"txt");
}

// ---------------------------------------------------------------------------
// F5-R1-002: --newest timestamp parser unification
// ---------------------------------------------------------------------------

/// F5-R1-002: `--newest N` must rank attachments by true chronological order even
/// when the "newest" attachment's `created` timestamp has a NON-STANDARD number of
/// fractional-second digits (not 0 or 3).
///
/// Bug: `parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.3f%z")` in chrono 0.4 only accepts
/// EXACTLY 0 or EXACTLY 3 fractional digits. Timestamps with 1, 2, 4, 6, or 9 digits
/// fail silently (None), so `--newest 1` sorts them LAST (None > Some ordering) and
/// downloads an OLDER attachment instead of the genuinely newer one.
///
/// Target fix: the `--newest` sort path must use the same relaxed parsing as the
/// `--older-than` path — i.e. `.parse::<DateTime<FixedOffset>>()` (RFC 3339) — which
/// accepts any number of fractional digits.
///
/// Fixture:
///   A: id="newer001" filename="newer_file.txt" created="2026-07-20T10:00:00.1+0000"
///      (1 fractional digit — valid RFC 3339, but %.3f fails → sorts LAST in buggy code)
///   B: id="older001" filename="older_file.txt" created="2026-01-01T08:00:00.000+0000"
///      (3 fractional digits — %.3f succeeds, RFC 3339 succeeds — genuinely OLDER)
///
/// With `--newest 1`, only A must be downloaded; B must be skipped.
///
/// RED: current code (%.3f) cannot parse A → A sorts LAST → downloads B (older).
#[tokio::test]
async fn test_newest_selects_no_millis_attachment_over_millis_older() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    let att_newer = make_attachment(
        "newer001",
        "newer_file.txt",
        "text/plain",
        14,
        "2026-07-20T10:00:00.1+0000", // 1 fractional digit — %.3f FAILS, RFC 3339 OK — NEWER
    );
    let att_older = make_attachment(
        "older001",
        "older_file.txt",
        "text/plain",
        14,
        "2026-01-01T08:00:00.000+0000", // 3 fractional digits — %.3f OK, RFC 3339 OK — OLDER
    );

    // Issue GET — returns both attachments.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/NEW-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_with_attachments("NEW-1", vec![att_newer, att_older])),
        )
        .mount(&server)
        .await;

    // Content GET for attachment A (newer) — returns unique body.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/newer001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"NEWEST_CONTENT"))
        .mount(&server)
        .await;

    // Content GET for attachment B (older) — returns unique body.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/older001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"OLDER_CONTENT"))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "NEW-1",
            "--newest",
            "1",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "F5-R1-002: --newest 1 must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // Exactly one file must be written.
    let entries: Vec<_> = std::fs::read_dir(out_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "F5-R1-002: exactly one file must be written by --newest 1; got {} files\nstderr: {stderr}",
        entries.len()
    );

    // The content of the written file must be from the NEWER attachment (A).
    // RED: current code downloads B (older, parseable) → content "OLDER_CONTENT".
    // GREEN (fixed): code downloads A (newer, no-millis) → content "NEWEST_CONTENT".
    let written_content = std::fs::read(entries[0].path()).unwrap();
    assert_eq!(
        written_content,
        b"NEWEST_CONTENT",
        "F5-R1-002: --newest 1 must download the genuinely newest attachment \
         (1-digit fractional seconds); got content {:?} — \
         this fails when %.3f rejects non-0/non-3-digit fractional timestamps \
         (1-digit, 2-digit, 4-digit etc sort LAST due to None > Some ordering)",
        String::from_utf8_lossy(&written_content)
    );
}

// ---------------------------------------------------------------------------
// F5-R1-001: Batch traversal security property pin (observable behavior)
// ---------------------------------------------------------------------------

/// F5-R1-001 observable security property: batch download (`--all`) with a
/// path-traversal filename (`../../evil.txt`) from the server must write the
/// file INSIDE `--out-dir`, never outside it.
///
/// `sanitize_attachment_filename("../../evil.txt")` returns `Some("evil.txt")`
/// via `Path::file_name()`, and the batch path then prefixes a SHA-1 of the
/// attachment ID, producing `<sha1>_evil.txt` safely inside out_dir.
///
/// This test pins the END-TO-END observable behavior at the integration level.
/// The `test_bc_2_7_011_vp576_001_containment_prop` proptest covers the same
/// invariant at the `sanitize_attachment_filename` function level; this test
/// complements it by running the full CLI invocation.
///
/// Expected: GREEN (the sanitizer is correctly implemented and the batch path
/// uses it). This is a security-property regression guard, not a RED gate test.
#[tokio::test]
async fn test_batch_download_traversal_filename_lands_inside_out_dir() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    // Attachment with a traversal filename supplied by the "server".
    let att = make_attachment(
        "trav001",
        "../../evil.txt", // path traversal attempt
        "text/plain",
        13,
        "2026-07-20T10:00:00.000+0000",
    );

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/TRAV-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(issue_with_attachments("TRAV-1", vec![att])),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/trav001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"traversal content"))
        .mount(&server)
        .await;

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "TRAV-1",
            "--all",
            "--out-dir",
            out_dir.path().to_str().unwrap(),
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "F5-R1-001 traversal pin: batch download with traversal filename must exit 0; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // Security property: any files written must be inside out_dir (not above it).
    let resolved_out = out_dir
        .path()
        .canonicalize()
        .unwrap_or_else(|_| out_dir.path().to_path_buf());

    let entries: Vec<_> = std::fs::read_dir(out_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    // At least one file must be written (the traversal-named attachment).
    assert!(
        !entries.is_empty(),
        "F5-R1-001 traversal pin: file from traversal-named attachment must land in out_dir; \
         out_dir is empty\nstderr: {stderr}"
    );

    // Every written file must be inside out_dir.
    for entry in &entries {
        let entry_path = entry.path().canonicalize().unwrap_or_else(|_| entry.path());
        assert!(
            entry_path.starts_with(&resolved_out),
            "F5-R1-001 traversal pin: SECURITY VIOLATION — file escaped out_dir! \
             file={:?} not inside {:?}",
            entry_path,
            resolved_out
        );
    }

    // Verify "../../evil.txt" was NOT written outside out_dir.
    // The parent of out_dir must NOT have gained an "evil.txt" file.
    let parent = out_dir.path().parent();
    if let Some(p) = parent {
        let evil_in_parent = p.join("evil.txt");
        assert!(
            !evil_in_parent.exists(),
            "F5-R1-001 traversal pin: SECURITY VIOLATION — 'evil.txt' escaped to parent dir {:?}",
            evil_in_parent
        );
        let grandparent = p.parent();
        if let Some(gp) = grandparent {
            let evil_in_gp = gp.join("evil.txt");
            assert!(
                !evil_in_gp.exists(),
                "F5-R1-001 traversal pin: SECURITY VIOLATION — 'evil.txt' escaped two levels up {:?}",
                evil_in_gp
            );
        }
    }
}

// ---------------------------------------------------------------------------
// F5-R3-001 — BC-2.7.012: download --id 404 must emit canonical string ONLY
// ---------------------------------------------------------------------------

/// F5-R3-001 / BC-2.7.012: when `jr issue attachment download <KEY> --id <AID>`
/// encounters a 404 on the metadata GET, stderr MUST contain the canonical
/// "Attachment {id} not found or not accessible." string and MUST NOT contain
/// the Jira API error body text.
///
/// The delete single-AID path (`delete_attachment_targeted` / `DEC-168`) surfaces
/// the body intentionally — that is a different operation.  The download path must
/// not propagate the raw server error body to the user.
///
/// **Current defect (F5-R3-001):** `get_attachment_metadata` (introduced in
/// the F5-R1-004 fix) appends `\n{message}` to the canonical prefix.  This
/// leaks the Jira error body to the download caller, which should only see the
/// canonical one-liner.  BC-2.7.012 §"404 body-surfacing asymmetry" requires
/// body surfacing on DELETE but CANONICAL-ONLY on DOWNLOAD.
///
/// **RED gate:** the assertion `!contains(SENTINEL)` fails until
/// `get_attachment_metadata` stops appending the body and the enrichment is
/// relocated to the delete call site.
#[tokio::test]
async fn test_f5_r3_001_download_id_404_canonical_only_no_jira_body() {
    // A sentinel string that is unlikely to appear in any other output.
    // This is the distinctive body text the mock server returns.
    const SENTINEL: &str = "SENTINEL_F5_R3_001_BODY_MUST_NOT_APPEAR_IN_DOWNLOAD_STDERR";

    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;
    let out_dir = TempDir::new().unwrap();

    // Mount a metadata GET that returns 404 with the sentinel body text.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/55555"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": [SENTINEL],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let out_path = out_dir.path().join("attachment.bin");
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "55555",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // (a) Exit code must be 64 (UserError).
    assert_eq!(
        output.status.code(),
        Some(64),
        "F5-R3-001: download --id 404 must exit 64; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // (b) Canonical prefix must be present.
    assert!(
        stderr.contains("Attachment 55555 not found or not accessible."),
        "F5-R3-001: download --id 404 stderr must contain canonical \
         'Attachment 55555 not found or not accessible.'; got: {stderr}"
    );

    // (c) The Jira API body MUST NOT appear in stderr.
    //
    // RED GATE: currently `get_attachment_metadata` appends `\n{message}` to
    // the canonical prefix (F5-R1-004 fix), so the sentinel leaks into stderr.
    // This assertion fails until the enrichment is relocated to the delete
    // call site (the fix for F5-R3-001).
    assert!(
        !stderr.contains(SENTINEL),
        "F5-R3-001 RED: download --id 404 must NOT surface the Jira error body \
         (BC-2.7.012 canonical-only); sentinel found in stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// FIX-F5-010 — BC-2.7.012 v1.3.104: EACCES permission-denied disk-write error
// ---------------------------------------------------------------------------

/// BC-2.7.012 v1.3.103 / FIX-F5-010: downloading to a non-writable directory → exit 1
/// with `Permission denied: cannot write to <dir> (writing <dest>): <os_error>. Check
/// directory permissions and try again.` AND no `tmp_` leak in stderr.
///
/// Unix-only (`chmod 0o555` semantics). Skipped cleanly when running as root
/// (uid 0) because root bypasses directory permission bits — detected by
/// probing whether a write into the restricted dir actually fails.
#[cfg(unix)]
#[tokio::test]
async fn test_bc_2_7_012_eacces_permission_denied_error_message() {
    use std::os::unix::fs::PermissionsExt;

    // Create a directory that will be made non-writable.
    let restricted = TempDir::new().unwrap();
    std::fs::set_permissions(restricted.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

    // Root-skip guard: probe whether the restriction actually binds.
    // Running as root (uid 0) bypasses permission bits → test would not exercise
    // the EACCES path → skip rather than emit a false GREEN.
    let probe = restricted.path().join(".probe_f5010");
    if std::fs::write(&probe, b"").is_ok() {
        let _ = std::fs::remove_file(&probe);
        let _ = std::fs::set_permissions(restricted.path(), std::fs::Permissions::from_mode(0o755));
        eprintln!(
            "test_bc_2_7_012_eacces_permission_denied_error_message: SKIPPED \
             (write into 0o555 dir succeeded — running as root)"
        );
        return;
    }

    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Step-1: attachment metadata GET.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/77777"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "77777",
            "filename": "eacces_test.bin",
            "size": 4,
            "mimeType": "application/octet-stream",
            "content": format!("{}/rest/api/3/attachment/content/77777", server.uri()),
        })))
        .mount(&server)
        .await;

    // Step-2: attachment content GET (small body; stream_to_file hits EACCES on
    // File::create before writing any bytes).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/77777"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"data"))
        .mount(&server)
        .await;

    let out_path = restricted.path().join("eacces_test.bin");
    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "EACCES-1",
            "--id",
            "77777",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Restore permissions so TempDir::Drop can remove the directory.
    let _ = std::fs::set_permissions(restricted.path(), std::fs::Permissions::from_mode(0o755));

    let stderr = String::from_utf8_lossy(&output.stderr);

    // (a) Must exit 1 (write failure, not panic / exit 101).
    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-2.7.012 EACCES: must exit 1; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    // (b) Must contain the "Permission denied: cannot write to" prefix.
    assert!(
        stderr.contains("Permission denied: cannot write to "),
        "BC-2.7.012 EACCES: stderr must contain \
         'Permission denied: cannot write to '; got: {stderr}"
    );

    // (c) Must name the FINAL destination parent directory (not the tmp_ path).
    let dir_str = restricted.path().to_str().unwrap();
    assert!(
        stderr.contains(dir_str),
        "BC-2.7.012 EACCES: stderr must contain the restricted dir path '{dir_str}'; \
         got: {stderr}"
    );

    // (d) Must include the remediation hint (BC-2.7.012 table).
    assert!(
        stderr.contains("Check directory permissions and try again."),
        "BC-2.7.012 EACCES: stderr must contain remediation hint \
         'Check directory permissions and try again.'; got: {stderr}"
    );

    // (e) Must NOT leak the internal tmp_<hex> path (tmp-path-leak pin).
    assert!(
        !stderr.contains("tmp_"),
        "BC-2.7.012 EACCES: stderr must NOT contain 'tmp_' (internal temp path \
         must not be surfaced to the user); got: {stderr}"
    );

    // (f) BC-2.7.012 v1.3.103 shape: dest basename must appear in the `(writing <dest>)`
    //     parenthetical of the PermissionDenied branch.
    assert!(
        stderr.contains("eacces_test.bin"),
        "BC-2.7.012 v1.3.103 EACCES: stderr must contain dest basename 'eacces_test.bin' \
         in (writing <dest>) parenthetical; got: {stderr}"
    );
}
