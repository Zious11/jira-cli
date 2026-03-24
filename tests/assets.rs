#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn search_assets_returns_objects() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "25"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 2,
            "isLast": true,
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" }
                },
                {
                    "id": "71",
                    "label": "Globex Inc",
                    "objectKey": "OBJ-71",
                    "objectType": { "id": "13", "name": "Client" }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let results = client
        .search_assets("ws-123", "objectType = Client", None, false)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].label, "Acme Corp");
    assert_eq!(results[1].object_key, "OBJ-71");
}

#[tokio::test]
async fn search_assets_empty() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 0,
            "isLast": true,
            "values": []
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let results = client
        .search_assets("ws-123", "objectType = Nonexistent", None, false)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn search_assets_with_limit() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("maxResults", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 1,
            "total": 5,
            "isLast": false,
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let results = client
        .search_assets("ws-123", "objectType = Client", Some(1), false)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn search_assets_is_last_as_string() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": "true",
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let results = client
        .search_assets("ws-123", "objectType = Client", None, false)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn get_asset_returns_object() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/70"))
        .and(query_param("includeAttributes", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "70",
            "label": "Acme Corp",
            "objectKey": "OBJ-70",
            "objectType": { "id": "13", "name": "Client" },
            "created": "2025-12-17T14:58:00.000Z",
            "attributes": [
                {
                    "id": "637",
                    "objectTypeAttributeId": "134",
                    "objectAttributeValues": [
                        { "value": "contact@acme.com", "displayValue": "contact@acme.com" }
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let obj = client.get_asset("ws-123", "70", true).await.unwrap();
    assert_eq!(obj.label, "Acme Corp");
    assert_eq!(obj.attributes.len(), 1);
}

#[tokio::test]
async fn get_connected_tickets_returns_tickets() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectconnectedtickets/70/tickets",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tickets": [
                {
                    "key": "PROJ-42",
                    "id": "10968",
                    "title": "VPN access not working",
                    "status": { "name": "In Progress", "colorName": "yellow" },
                    "type": { "name": "Service Request" },
                    "priority": { "name": "High" }
                }
            ],
            "allTicketsQuery": "issueFunction in assetsObject(\"objectId = 70\")"
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let resp = client.get_connected_tickets("ws-123", "70").await.unwrap();
    assert_eq!(resp.tickets.len(), 1);
    assert_eq!(resp.tickets[0].key, "PROJ-42");
    assert_eq!(resp.tickets[0].title, "VPN access not working");
    assert!(resp.all_tickets_query.is_some());
}

#[tokio::test]
async fn search_assets_paginated() {
    let server = MockServer::start().await;

    // Page 1
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 2,
            "isLast": false,
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" }
                }
            ]
        })))
        .mount(&server)
        .await;

    // Page 2
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("startAt", "25"))
        .and(query_param("maxResults", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 25,
            "maxResults": 25,
            "total": 2,
            "isLast": true,
            "values": [
                {
                    "id": "71",
                    "label": "Globex Inc",
                    "objectKey": "OBJ-71",
                    "objectType": { "id": "13", "name": "Client" }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let results = client
        .search_assets("ws-123", "objectType = Client", None, false)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].label, "Acme Corp");
    assert_eq!(results[1].label, "Globex Inc");
}

#[tokio::test]
async fn get_connected_tickets_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectconnectedtickets/99/tickets",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tickets": []
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let resp = client.get_connected_tickets("ws-123", "99").await.unwrap();
    assert!(resp.tickets.is_empty());
}
