//! Integration tests for `jr issue remote-link` — issue #199.
//!
//! Covers the happy path (explicit title), the default-title-to-URL behavior,
//! and server error propagation. The command POSTs to
//! `/rest/api/3/issue/{key}/remotelink` with a body shaped like
//! `{"object": {"url": "...", "title": "..."}}` and renders the Atlassian
//! response (`id`, `self`) plus the caller-provided `key`/`url`/`title` in its
//! `--output json` payload.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn remote_link_creates_with_explicit_title() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let self_url = format!(
        "{}/rest/api/3/issue/PROJ-123/remotelink/10000",
        server.uri()
    );

    // Note: the CLI normalizes URLs via `url::Url::parse` before sending, so
    // `https://example.com` becomes `https://example.com/` (trailing slash
    // added). The mock body + stdout assertions use the normalized form.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-123/remotelink"))
        .and(body_partial_json(serde_json::json!({
            "object": {
                "url": "https://example.com/",
                "title": "Example"
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 10000,
            "self": self_url,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "--no-input",
            "issue",
            "remote-link",
            "PROJ-123",
            "--url",
            "https://example.com",
            "--title",
            "Example",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected success, stderr: {stderr}, stdout: {stdout}"
    );

    let parsed: Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed["key"], "PROJ-123");
    assert_eq!(parsed["id"], 10000);
    assert_eq!(parsed["url"], "https://example.com/");
    assert_eq!(parsed["title"], "Example");
    assert_eq!(parsed["self"], self_url.as_str());
}

#[tokio::test]
async fn remote_link_defaults_title_to_url() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let self_url = format!(
        "{}/rest/api/3/issue/PROJ-123/remotelink/10001",
        server.uri()
    );

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-123/remotelink"))
        .and(body_partial_json(serde_json::json!({
            "object": {
                "url": "https://example.com/page",
                "title": "https://example.com/page"
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 10001,
            "self": self_url,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "--no-input",
            "issue",
            "remote-link",
            "PROJ-123",
            "--url",
            "https://example.com/page",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected success, stderr: {stderr}, stdout: {stdout}"
    );

    let parsed: Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(
        parsed["title"], "https://example.com/page",
        "title must default to the URL when --title is omitted"
    );
    assert_eq!(parsed["url"], "https://example.com/page");
}

#[tokio::test]
async fn remote_link_surfaces_server_error() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-123/remotelink"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": ["Issue does not exist or you do not have permission to see it."],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "--no-input",
            "issue",
            "remote-link",
            "PROJ-123",
            "--url",
            "https://example.com",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "malformed-request API error should exit 1 (ApiError default), got: {:?}, stderr: {}",
        output.status.code(),
        stderr
    );

    assert!(
        stderr.to_lowercase().contains("issue does not exist"),
        "server error body should surface on stderr, got: {stderr}"
    );
}

#[tokio::test]
async fn remote_link_surfaces_not_authenticated_on_401() {
    // Mirrors the 401 regression guard at tests/issue_changelog.rs:1109 —
    // pins that unauthenticated responses route through JrError::NotAuthenticated
    // (exit 2) with the "run jr auth login" hint, instead of a generic
    // JrError::ApiError (exit 1).
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-125/remotelink"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Authentication required"],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "remote-link",
            "PROJ-125",
            "--url",
            "https://example.com",
            "--output",
            "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(2),
        "401 should route to NotAuthenticated (exit 2), got: {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Not authenticated") || stderr.contains("jr auth login"),
        "401 should surface reauth hint, got: {stderr}"
    );
}

/// Validation guard: junk string in `--url` must exit 64 at the CLI boundary
/// before any HTTP call is made. Mirrors the empty-input guards at
/// `tests/issue_changelog.rs:477` — Jira's /remotelink endpoint accepts any
/// string and would silently create a broken remote link without this check.
#[test]
fn remote_link_rejects_invalid_url_with_exit_64() {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        // Unreachable base URL — validation must short-circuit before any
        // network call, so this should never be dialed.
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "remote-link",
            "PROJ-1",
            "--url",
            "not-a-url",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(64),
        "junk --url must exit 64 (UserError), got: {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--url"),
        "stderr should name the offending flag, got: {stderr}"
    );
    assert!(
        stderr.to_lowercase().contains("not a valid url"),
        "stderr should describe the failure mode, got: {stderr}"
    );
}

/// Validation guard: the http|https scheme gate rejects URLs that parse
/// cleanly as URLs but would render as unclickable/unsafe links in Jira
/// (e.g. `ftp://`, `javascript:`, `file://`). Locks the `matches!(..., "http"
/// | "https")` check so a future refactor that broadens the scheme set gets
/// a test failure.
#[test]
fn remote_link_rejects_non_http_scheme_with_exit_64() {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "remote-link",
            "PROJ-1",
            "--url",
            "ftp://example.com",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(64),
        "non-http scheme must exit 64 (UserError), got: {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("http or https"),
        "stderr should name the accepted schemes, got: {stderr}"
    );
    assert!(
        stderr.contains("ftp"),
        "stderr should echo the rejected scheme, got: {stderr}"
    );
}
