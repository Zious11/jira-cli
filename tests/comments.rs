#[allow(dead_code)]
mod common;

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
