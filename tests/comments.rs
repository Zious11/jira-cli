#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_comments_returns_all_comments() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Alice", "emailAddress": "a@test.com", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "First comment" }] }] },
                    "created": "2026-03-20T10:00:00.000+0000"
                },
                {
                    "id": "10002",
                    "author": { "accountId": "def", "displayName": "Bob", "emailAddress": "b@test.com", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Second comment" }] }] },
                    "created": "2026-03-21T11:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 2
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let comments = client.list_comments("FOO-1", None).await.unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].id.as_deref(), Some("10001"));
    assert_eq!(comments[1].id.as_deref(), Some("10002"));
}

#[tokio::test]
async fn list_comments_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [],
            "startAt": 0,
            "maxResults": 100,
            "total": 0
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let comments = client.list_comments("FOO-2", None).await.unwrap();
    assert!(comments.is_empty());
}

#[tokio::test]
async fn list_comments_with_limit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-3/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "1"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Alice", "emailAddress": "a@test.com", "active": true },
                    "body": null,
                    "created": "2026-03-20T10:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 1,
            "total": 2
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let comments = client.list_comments("FOO-3", Some(1)).await.unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id.as_deref(), Some("10001"));
}

#[tokio::test]
async fn list_comments_paginated() {
    let server = MockServer::start().await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-4/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Alice", "emailAddress": "a@test.com", "active": true },
                    "body": null,
                    "created": "2026-03-20T10:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 1,
            "total": 2
        })))
        .mount(&server)
        .await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-4/comment"))
        .and(query_param("startAt", "1"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10002",
                    "author": { "accountId": "def", "displayName": "Bob", "emailAddress": "b@test.com", "active": true },
                    "body": null,
                    "created": "2026-03-21T11:00:00.000+0000"
                }
            ],
            "startAt": 1,
            "maxResults": 1,
            "total": 2
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let comments = client.list_comments("FOO-4", None).await.unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].id.as_deref(), Some("10001"));
    assert_eq!(comments[1].id.as_deref(), Some("10002"));
}

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn issue_comments_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
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
        .args(["issue", "comments", "PROJ-1"])
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
async fn issue_comments_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
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
        .args(["issue", "comments", "PROJ-1"])
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
async fn issue_comments_network_drop_surfaces_reach_error() {
    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "PROJ-1"])
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
