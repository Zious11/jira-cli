#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_get_org_metadata() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/gateway/api/graphql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::graphql_org_metadata_json()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let metadata = client.get_org_metadata("test.atlassian.net").await.unwrap();
    assert_eq!(metadata.org_id, "test-org-id-456");
    assert_eq!(metadata.cloud_id, "test-cloud-id-123");
}

#[tokio::test]
async fn test_list_teams() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/gateway/api/public/teams/v1/org/.*/teams"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::teams_list_json()))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let teams = client.list_teams("test-org-id-456").await.unwrap();
    assert_eq!(teams.len(), 3);
    assert_eq!(teams[0].display_name, "Alpha Team");
    assert_eq!(teams[0].team_id, "team-uuid-alpha");
}

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

/// Write a minimal jr config to a temp XDG_CONFIG_HOME so the subprocess
/// sees no org_id (forcing the GraphQL metadata call) while still having
/// a url (required by resolve_org_id to build the hostname arg).
fn write_minimal_config(config_home: &std::path::Path, url: &str) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        format!("[instance]\nurl = \"{url}\"\n"),
    )
    .unwrap();
}

#[tokio::test]
async fn team_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Fail the first call in the team-list chain (GraphQL org metadata).
    Mock::given(method("POST"))
        .and(path("/gateway/api/graphql"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["team", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "5xx should exit 1, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("API error (500)"),
        "Expected 'API error (500)' in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn team_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    Mock::given(method("POST"))
        .and(path("/gateway/api/graphql"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["team", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "401 should exit 2, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Not authenticated"),
        "Expected 'Not authenticated' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "Expected 'jr auth login' suggestion in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn team_list_network_drop_surfaces_reach_error() {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    // Point the config url at the unreachable address so resolve_org_id has a hostname.
    write_minimal_config(config_dir.path(), "http://127.0.0.1:1");

    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["team", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "Net-drop should exit 1, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Could not reach"),
        "Expected 'Could not reach' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("check your connection"),
        "Expected 'check your connection' in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
