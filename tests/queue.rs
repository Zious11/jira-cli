#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_queues_returns_all_queues() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "10", "name": "Triage", "jql": "project = HELPDESK AND status = New", "issueCount": 12 },
                { "id": "20", "name": "In Progress", "jql": "project = HELPDESK AND status = \"In Progress\"", "issueCount": 7 }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let queues = client.list_queues("15").await.unwrap();
    assert_eq!(queues.len(), 2);
    assert_eq!(queues[0].name, "Triage");
    assert_eq!(queues[0].issue_count, Some(12));
    assert_eq!(queues[1].name, "In Progress");
}

#[tokio::test]
async fn list_queues_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 0,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": []
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let queues = client.list_queues("15").await.unwrap();
    assert!(queues.is_empty());
}

#[tokio::test]
async fn get_queue_issue_keys_returns_keys() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                {
                    "key": "HELPDESK-42",
                    "fields": {
                        "summary": "VPN not working",
                        "status": { "name": "New", "statusCategory": { "name": "To Do", "key": "new" } },
                        "issuetype": { "name": "Service Request" },
                        "priority": { "name": "High" },
                        "assignee": null
                    }
                },
                {
                    "key": "HELPDESK-41",
                    "fields": {
                        "summary": "Need license renewal",
                        "status": { "name": "New", "statusCategory": { "name": "To Do", "key": "new" } },
                        "issuetype": { "name": "Service Request" },
                        "assignee": { "accountId": "abc", "displayName": "Jane D." }
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let keys = client.get_queue_issue_keys("15", "10", None).await.unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], "HELPDESK-42");
    assert_eq!(keys[1], "HELPDESK-41");
}

#[tokio::test]
async fn get_queue_issue_keys_with_limit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 1,
            "isLastPage": false,
            "values": [
                {
                    "key": "HELPDESK-42",
                    "fields": {
                        "summary": "VPN not working"
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let keys = client
        .get_queue_issue_keys("15", "10", Some(1))
        .await
        .unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0], "HELPDESK-42");
}

#[tokio::test]
async fn get_queue_issue_keys_paginated() {
    let server = MockServer::start().await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 1,
            "isLastPage": false,
            "values": [
                { "key": "HELPDESK-2", "fields": { "summary": "Issue A" } }
            ]
        })))
        .mount(&server)
        .await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "1"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 1,
            "limit": 1,
            "isLastPage": true,
            "values": [
                { "key": "HELPDESK-1", "fields": { "summary": "Issue B" } }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let keys = client.get_queue_issue_keys("15", "10", None).await.unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], "HELPDESK-2");
    assert_eq!(keys[1], "HELPDESK-1");
}

#[tokio::test]
async fn resolve_queue_duplicate_names_error_message() {
    let server = MockServer::start().await;

    // Two queues with the same name but different IDs
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "10", "name": "Triage", "issueCount": 5 },
                { "id": "20", "name": "Triage", "issueCount": 3 }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = jr::cli::queue::resolve_queue_by_name("15", "Triage", &client).await;

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Multiple queues named \"Triage\""),
        "Expected queue name in error, got: {msg}"
    );
    assert!(
        msg.contains("10, 20"),
        "Expected both queue IDs in error, got: {msg}"
    );
    assert!(
        msg.contains("Use --id 10 to specify"),
        "Expected --id suggestion in error, got: {msg}"
    );
}

#[tokio::test]
async fn resolve_queue_mixed_case_duplicate_names_error_message() {
    let server = MockServer::start().await;

    // Two queues whose names differ only in casing — unlike the exact-duplicate
    // test above, this exercises the to_lowercase() normalization path
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "30", "name": "Triage", "issueCount": 5 },
                { "id": "40", "name": "TRIAGE", "issueCount": 3 }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    // Lowercase input — differs in casing from both stored names,
    // so to_lowercase() must normalize both input and candidates
    let result = jr::cli::queue::resolve_queue_by_name("15", "triage", &client).await;

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Multiple queues named \"Triage\""),
        "Expected queue name in error, got: {msg}"
    );
    assert!(
        msg.contains("30, 40"),
        "Expected both queue IDs in error, got: {msg}"
    );
    assert!(
        msg.contains("Use --id 30 to specify"),
        "Expected --id suggestion in error, got: {msg}"
    );
}

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

/// Write a minimal jr config to a temp XDG_CONFIG_HOME so the subprocess
/// finds a URL while JR_BASE_URL / JR_AUTH_HEADER override the real values.
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
async fn queue_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    // Fail the FIRST call in the queue-list chain:
    // require_service_desk → get_or_fetch_project_meta → GET /rest/api/3/project/{key}
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
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
        .args(["queue", "list", "--project", "PROJ"])
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
async fn queue_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), &server.uri());

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
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
        .args(["queue", "list", "--project", "PROJ"])
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
async fn queue_list_network_drop_surfaces_reach_error() {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_minimal_config(config_dir.path(), "http://127.0.0.1:1");

    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["queue", "list", "--project", "PROJ"])
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
