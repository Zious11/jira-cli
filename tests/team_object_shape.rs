//! End-to-end coverage for the Atlas Teams object shape on the team custom
//! field (#254). Proves that `jr issue view KEY --output json` preserves the
//! full `{"id": "...", "name": "..."}` object when a tenant returns the
//! object form of the Team field — Task 1 widened `IssueFields::team_id` to
//! accept both the scalar UUID and the object shape, and this test locks in
//! the end-to-end contract so a regression in deserialization, JSON
//! serialization, or the once-per-process warning path would surface here.
//!
//! Conventions match `tests/issue_view_errors.rs` — plain `#[tokio::test]`,
//! the `JR_BASE_URL` / `JR_AUTH_HEADER` env pattern, and `XDG_CONFIG_HOME` +
//! `XDG_CACHE_HOME` tempdirs for isolation. The `/rest/api/3/field` stub is
//! belt-and-braces: `handle_view` calls `get_or_fetch_cmdb_fields(...)
//! .unwrap_or_default()`, so a cache miss that falls through to the live
//! endpoint is already tolerated, but mocking keeps the test resilient if
//! that swallow is ever removed.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn issue_view_json_extracts_team_uuid_from_object_shape() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-700"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": "PROJ-700",
            "fields": {
                "summary": "test",
                "status": { "name": "To Do", "statusCategory": { "name": "To Do", "key": "new" } },
                "issuetype": { "name": "Task" },
                "project": { "key": "PROJ" },
                "customfield_10001": { "id": "team-uuid-alpha", "name": "Platform Team" }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(config_dir.path().join("jr")).unwrap();
    std::fs::write(
        config_dir.path().join("jr/config.toml"),
        "[fields]\nteam_field_id = \"customfield_10001\"\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue",
            "view",
            "PROJ-700",
            "--output",
            "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let parsed: Value = serde_json::from_str(&stdout).expect("valid JSON");

    assert_eq!(
        parsed["fields"]["customfield_10001"]["id"], "team-uuid-alpha",
        "team field must be preserved in JSON output as object with .id"
    );
    assert!(
        !stderr.contains("unexpected shape"),
        "warning about unexpected shape must not fire on object-shape response; stderr: {stderr}"
    );
}
