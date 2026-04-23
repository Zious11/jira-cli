//! Integration tests for `jr issue create --output json` — issue #253.
//!
//! The JSON output of `issue create` must return the same full Issue payload
//! shape as `issue view --output json`, plus a top-level `url` field. After a
//! successful POST /rest/api/3/issue, the handler does a follow-up GET
//! /rest/api/3/issue/{key} and merges `url` into the result. On GET failure it
//! warns on stderr and falls back to the old `{key, url}` shape (exit 0).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn issue_create_json_returns_full_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // POST /rest/api/3/issue — minimal Atlassian create response.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // GET /rest/api/3/issue/PROJ-123 — full issue payload the handler fetches
    // after a successful create.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
            "fields": {
                "summary": "test summary",
                "status": {
                    "name": "To Do",
                    "statusCategory": {"name": "To Do", "key": "new"}
                },
                "issuetype": {"name": "Task"},
                "project": {"key": "PROJ"}
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // GET /rest/api/3/field — fetched by `get_or_fetch_cmdb_fields` on a cold
    // cache. An empty array means "no CMDB fields" and keeps the test focused
    // on the shape guarantee.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test summary",
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
    assert!(
        parsed["url"]
            .as_str()
            .expect("url must be a string")
            .ends_with("/browse/PROJ-123"),
        "url must end with /browse/PROJ-123, got: {}",
        parsed["url"]
    );
    assert_eq!(
        parsed["fields"]["summary"], "test summary",
        "fields.summary must be present in the full shape"
    );
    assert_eq!(
        parsed["fields"]["status"]["name"], "To Do",
        "fields.status.name must be present in the full shape"
    );
    assert_eq!(parsed["fields"]["issuetype"]["name"], "Task");
    assert_eq!(parsed["fields"]["project"]["key"], "PROJ");

    assert!(
        !stderr.to_lowercase().contains("warning") && !stderr.to_lowercase().contains("error"),
        "expected clean stderr on happy path, got: {stderr}"
    );
}

/// If the follow-up GET fails, the handler warns on stderr and falls back to
/// the old minimal `{key, url}` shape. The command still exits 0 — the create
/// succeeded, only the enrichment didn't.
#[tokio::test]
async fn issue_create_json_falls_back_on_get_failure() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10002",
            "key": "PROJ-456",
            "self": format!("{}/rest/api/3/issue/10002", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Follow-up GET fails with 500.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-456"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["boom"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "fallback summary",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(
        output.status.success(),
        "create must still succeed when the follow-up GET fails, stderr: {stderr}"
    );
    assert!(
        stderr.contains("PROJ-456") && stderr.to_lowercase().contains("warn"),
        "expected a stderr warning mentioning the new key, got: {stderr}"
    );

    let parsed: Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed["key"], "PROJ-456");
    assert!(
        parsed["url"]
            .as_str()
            .expect("url must be a string")
            .ends_with("/browse/PROJ-456")
    );
    // The fallback shape is the old minimal `{key, url}` — no `fields` object.
    assert!(
        parsed.get("fields").is_none(),
        "fallback shape must not contain `fields`, got: {parsed}"
    );
}
