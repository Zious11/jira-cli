#[allow(dead_code)]
mod common;

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
