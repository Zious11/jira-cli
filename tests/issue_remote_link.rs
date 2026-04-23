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

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-123/remotelink"))
        .and(body_partial_json(serde_json::json!({
            "object": {
                "url": "https://example.com",
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
    assert_eq!(parsed["url"], "https://example.com");
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "expected failure on 400, stderr: {stderr}, stdout: {stdout}"
    );

    let lower = stderr.to_lowercase();
    assert!(
        lower.contains("issue does not exist") || stderr.contains("400"),
        "stderr must surface the server error (either the message or the status), got: {stderr}"
    );
}
