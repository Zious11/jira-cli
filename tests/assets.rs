#[allow(dead_code)]
mod common;

use serde_json::json;
use tokio::sync::Mutex;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Serialize tests that touch XDG_CACHE_HOME — all tests in this file that
/// manipulate the env var must hold this mutex for the entire test duration.
static ENV_MUTEX: Mutex<()> = Mutex::const_new(());

/// RAII guard that restores XDG_CACHE_HOME to its previous value on drop.
struct CacheDirGuard {
    prev: Option<std::ffi::OsString>,
    _lock: tokio::sync::MutexGuard<'static, ()>,
}

impl Drop for CacheDirGuard {
    fn drop(&mut self) {
        // SAFETY: _lock (ENV_MUTEX) is still held while we restore the env var.
        unsafe {
            match &self.prev {
                Some(prev) => std::env::set_var("XDG_CACHE_HOME", prev),
                None => std::env::remove_var("XDG_CACHE_HOME"),
            }
        }
    }
}

async fn set_cache_dir(dir: &std::path::Path) -> CacheDirGuard {
    let guard = ENV_MUTEX.lock().await;
    // SAFETY: ENV_MUTEX guard is held for the entire test duration via CacheDirGuard.
    let prev = std::env::var_os("XDG_CACHE_HOME");
    unsafe { std::env::set_var("XDG_CACHE_HOME", dir) };
    CacheDirGuard { prev, _lock: guard }
}

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

#[tokio::test]
async fn get_object_attributes_returns_named_attributes() {
    let server = MockServer::start().await;

    // Mock returns a mix of system, label, hidden, and user-defined attributes
    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/88/attributes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "637",
                "objectTypeAttributeId": "134",
                "objectTypeAttribute": {
                    "id": "134",
                    "name": "Key",
                    "system": true,
                    "hidden": false,
                    "label": false,
                    "position": 0
                },
                "objectAttributeValues": [
                    { "value": "OBJ-88", "displayValue": "OBJ-88" }
                ]
            },
            {
                "id": "640",
                "objectTypeAttributeId": "135",
                "objectTypeAttribute": {
                    "id": "135",
                    "name": "Name",
                    "system": false,
                    "hidden": false,
                    "label": true,
                    "position": 1
                },
                "objectAttributeValues": [
                    { "value": "Acme Corp", "displayValue": "Acme Corp" }
                ]
            },
            {
                "id": "641",
                "objectTypeAttributeId": "140",
                "objectTypeAttribute": {
                    "id": "140",
                    "name": "Location",
                    "system": false,
                    "hidden": false,
                    "label": false,
                    "position": 5
                },
                "objectAttributeValues": [
                    { "value": "New York, NY", "displayValue": "New York, NY" }
                ]
            },
            {
                "id": "642",
                "objectTypeAttributeId": "141",
                "objectTypeAttribute": {
                    "id": "141",
                    "name": "Internal Notes",
                    "system": false,
                    "hidden": true,
                    "label": false,
                    "position": 6
                },
                "objectAttributeValues": [
                    { "value": "secret", "displayValue": "secret" }
                ]
            },
            {
                "id": "643",
                "objectTypeAttributeId": "142",
                "objectTypeAttribute": {
                    "id": "142",
                    "name": "Seats",
                    "system": false,
                    "hidden": false,
                    "label": false,
                    "position": 4
                },
                "objectAttributeValues": [
                    { "value": "10", "displayValue": "10" }
                ]
            }
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let attrs = client.get_object_attributes("ws-123", "88").await.unwrap();

    // All 5 attributes returned from API
    assert_eq!(attrs.len(), 5);

    // Verify attribute names are present
    assert_eq!(attrs[0].object_type_attribute.name, "Key");
    assert!(attrs[0].object_type_attribute.system);

    // Verify label attribute
    assert_eq!(attrs[1].object_type_attribute.name, "Name");
    assert!(attrs[1].object_type_attribute.label);

    // Verify hidden attribute
    assert_eq!(attrs[3].object_type_attribute.name, "Internal Notes");
    assert!(attrs[3].object_type_attribute.hidden);

    // Simulate the CLI filter: exclude system, hidden, label
    let mut visible: Vec<_> = attrs
        .into_iter()
        .filter(|a| {
            !a.object_type_attribute.system
                && !a.object_type_attribute.hidden
                && !a.object_type_attribute.label
        })
        .collect();
    visible.sort_by_key(|a| a.object_type_attribute.position);

    // Only user-defined, non-hidden attributes remain
    assert_eq!(visible.len(), 2);
    // Sorted by position: Seats (4) before Location (5)
    assert_eq!(visible[0].object_type_attribute.name, "Seats");
    assert_eq!(visible[0].object_type_attribute.position, 4);
    assert_eq!(visible[1].object_type_attribute.name, "Location");
    assert_eq!(visible[1].object_type_attribute.position, 5);

    // Verify displayValue is available
    assert_eq!(
        visible[1].values[0].display_value.as_deref(),
        Some("New York, NY")
    );
}

#[tokio::test]
async fn get_object_type_attributes_returns_definitions() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/23/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134",
                "name": "Key",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 0,
                "editable": false,
                "sortable": true
            },
            {
                "id": "135",
                "name": "Name",
                "system": false,
                "hidden": false,
                "label": true,
                "position": 1,
                "editable": true,
                "sortable": true
            },
            {
                "id": "140",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 5,
                "editable": true,
                "sortable": true
            }
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let attrs = client
        .get_object_type_attributes("ws-123", "23")
        .await
        .unwrap();

    assert_eq!(attrs.len(), 3);
    assert_eq!(attrs[0].id, "134");
    assert_eq!(attrs[0].name, "Key");
    assert!(attrs[0].system);
    assert!(!attrs[0].hidden);
    assert_eq!(attrs[1].id, "135");
    assert_eq!(attrs[1].name, "Name");
    assert!(attrs[1].label);
    assert_eq!(attrs[2].id, "140");
    assert_eq!(attrs[2].name, "Location");
    assert_eq!(attrs[2].position, 5);
}

#[tokio::test(flavor = "current_thread")]
async fn enrich_search_attributes_injects_names() {
    let cache_dir = tempfile::tempdir().unwrap();
    let _env_guard = set_cache_dir(cache_dir.path()).await;

    let server = MockServer::start().await;

    // Mock: object type 13 attribute definitions
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/13/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134",
                "name": "Key",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 0
            },
            {
                "id": "140",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 5
            },
            {
                "id": "141",
                "name": "Secret",
                "system": false,
                "hidden": true,
                "label": false,
                "position": 6
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    // Simulate search results with inline attributes (no names)
    let objects = vec![jr::types::assets::AssetObject {
        id: "70".into(),
        label: "Acme Corp".into(),
        object_key: "OBJ-70".into(),
        object_type: jr::types::assets::ObjectType {
            id: "13".into(),
            name: "Client".into(),
            description: None,
        },
        created: None,
        updated: None,
        attributes: vec![
            jr::types::assets::AssetAttribute {
                id: "637".into(),
                object_type_attribute_id: "140".into(),
                values: vec![jr::types::assets::ObjectAttributeValue {
                    value: Some("New York".into()),
                    display_value: Some("New York".into()),
                }],
            },
            jr::types::assets::AssetAttribute {
                id: "638".into(),
                object_type_attribute_id: "141".into(),
                values: vec![jr::types::assets::ObjectAttributeValue {
                    value: Some("secret".into()),
                    display_value: Some("secret".into()),
                }],
            },
        ],
    }];

    let enriched = jr::api::assets::objects::enrich_search_attributes(&client, "ws-123", &objects)
        .await
        .unwrap();

    // Returns the attribute definition map for use in output formatting
    assert!(enriched.contains_key("140"));
    assert_eq!(enriched["140"].name, "Location");
    assert!(enriched.contains_key("141"));
    assert_eq!(enriched["141"].name, "Secret");
    assert!(enriched["141"].hidden);
}

#[tokio::test(flavor = "current_thread")]
async fn search_attributes_json_includes_names() {
    let cache_dir = tempfile::tempdir().unwrap();
    let _env_guard = set_cache_dir(cache_dir.path()).await;

    let server = MockServer::start().await;

    // Mock: AQL search with attributes
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("includeAttributes", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" },
                    "attributes": [
                        {
                            "id": "637",
                            "objectTypeAttributeId": "134",
                            "objectAttributeValues": [
                                { "value": "OBJ-70", "displayValue": "OBJ-70" }
                            ]
                        },
                        {
                            "id": "638",
                            "objectTypeAttributeId": "140",
                            "objectAttributeValues": [
                                { "value": "New York", "displayValue": "New York" }
                            ]
                        }
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    // Mock: object type attribute definitions
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/13/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134",
                "name": "Key",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 0
            },
            {
                "id": "140",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 5
            }
        ])))
        .mount(&server)
        .await;

    // Mock: workspace discovery (needed for CLI command)
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args([
            "--output",
            "json",
            "assets",
            "search",
            "--attributes",
            "objectType = Client",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let objects = parsed.as_array().expect("array of objects");
    assert_eq!(objects.len(), 1);

    let attrs = objects[0]["attributes"]
        .as_array()
        .expect("attributes array");
    // System attribute (Key) should be filtered out
    // Only Location should remain
    assert_eq!(attrs.len(), 1);
    assert_eq!(attrs[0]["objectTypeAttribute"]["name"], "Location");
    assert_eq!(attrs[0]["objectTypeAttribute"]["position"], 5);
    assert_eq!(
        attrs[0]["objectAttributeValues"][0]["displayValue"],
        "New York"
    );
}

#[tokio::test]
async fn cli_json_filter_excludes_system_and_hidden_attributes() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/88/attributes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "637",
                "objectTypeAttributeId": "134",
                "objectTypeAttribute": {
                    "id": "134",
                    "name": "Key",
                    "system": true,
                    "hidden": false,
                    "label": false,
                    "position": 0
                },
                "objectAttributeValues": [
                    { "value": "OBJ-88", "displayValue": "OBJ-88" }
                ]
            },
            {
                "id": "640",
                "objectTypeAttributeId": "135",
                "objectTypeAttribute": {
                    "id": "135",
                    "name": "Name",
                    "system": false,
                    "hidden": false,
                    "label": true,
                    "position": 1
                },
                "objectAttributeValues": [
                    { "value": "Acme Corp", "displayValue": "Acme Corp" }
                ]
            },
            {
                "id": "641",
                "objectTypeAttributeId": "140",
                "objectTypeAttribute": {
                    "id": "140",
                    "name": "Location",
                    "system": false,
                    "hidden": false,
                    "label": false,
                    "position": 5
                },
                "objectAttributeValues": [
                    { "value": "New York, NY", "displayValue": "New York, NY" }
                ]
            },
            {
                "id": "642",
                "objectTypeAttributeId": "141",
                "objectTypeAttribute": {
                    "id": "141",
                    "name": "Internal Notes",
                    "system": false,
                    "hidden": true,
                    "label": false,
                    "position": 6
                },
                "objectAttributeValues": [
                    { "value": "secret", "displayValue": "secret" }
                ]
            }
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let mut attrs = client.get_object_attributes("ws-123", "88").await.unwrap();

    // Apply the same filter used by handle_view for JSON output
    attrs.retain(|a| !a.object_type_attribute.system && !a.object_type_attribute.hidden);
    attrs.sort_by_key(|a| a.object_type_attribute.position);

    // System (Key) and hidden (Internal Notes) are excluded
    assert_eq!(attrs.len(), 2);
    assert_eq!(attrs[0].object_type_attribute.name, "Name");
    assert_eq!(attrs[1].object_type_attribute.name, "Location");
    assert_eq!(
        attrs[1].values[0].display_value.as_deref(),
        Some("New York, NY")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn search_attributes_table_shows_inline_values() {
    let cache_dir = tempfile::tempdir().unwrap();
    let _env_guard = set_cache_dir(cache_dir.path()).await;

    let server = MockServer::start().await;

    // Mock: AQL search with attributes
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("includeAttributes", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" },
                    "attributes": [
                        {
                            "id": "637",
                            "objectTypeAttributeId": "134",
                            "objectAttributeValues": [
                                { "value": "OBJ-70", "displayValue": "OBJ-70" }
                            ]
                        },
                        {
                            "id": "639",
                            "objectTypeAttributeId": "142",
                            "objectAttributeValues": [
                                { "value": "10", "displayValue": "10" }
                            ]
                        },
                        {
                            "id": "638",
                            "objectTypeAttributeId": "140",
                            "objectAttributeValues": [
                                { "value": "New York", "displayValue": "New York" }
                            ]
                        }
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    // Mock: object type attribute definitions
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/13/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134",
                "name": "Key",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 0
            },
            {
                "id": "142",
                "name": "Seats",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 4
            },
            {
                "id": "140",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 5
            }
        ])))
        .mount(&server)
        .await;

    // Mock: workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args(["assets", "search", "--attributes", "objectType = Client"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Table should contain the Attributes column with inline values
    // Seats (position 4) comes before Location (position 5)
    assert!(
        stdout.contains("Seats: 10"),
        "Expected 'Seats: 10' in table, got: {stdout}"
    );
    assert!(
        stdout.contains("Location: New York"),
        "Expected 'Location: New York' in table, got: {stdout}"
    );
    // System attribute Key should NOT appear
    assert!(
        !stdout.contains("Key: OBJ-70"),
        "System attribute Key should be filtered, got: {stdout}"
    );
    // Should have Attributes header instead of Created/Updated
    assert!(
        stdout.contains("Attributes"),
        "Expected 'Attributes' header in table, got: {stdout}"
    );
    assert!(
        !stdout.contains("Created"),
        "Should not have Created column, got: {stdout}"
    );
}

#[tokio::test]
async fn list_object_schemas_returns_schemas() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/objectschema/list"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "25"))
        .and(query_param("includeCounts", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 2,
            "isLast": true,
            "values": [
                {
                    "id": "6",
                    "name": "ITSM",
                    "objectSchemaKey": "ITSM",
                    "status": "Ok",
                    "objectCount": 95,
                    "objectTypeCount": 34
                },
                {
                    "id": "1",
                    "name": "Human Resources",
                    "objectSchemaKey": "HR",
                    "description": "HR schema",
                    "status": "Ok",
                    "objectCount": 1023,
                    "objectTypeCount": 14
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let schemas = client.list_object_schemas("ws-123").await.unwrap();
    assert_eq!(schemas.len(), 2);
    assert_eq!(schemas[0].name, "ITSM");
    assert_eq!(schemas[0].object_schema_key, "ITSM");
    assert_eq!(schemas[0].object_type_count, 34);
    assert_eq!(schemas[1].name, "Human Resources");
    assert_eq!(schemas[1].description.as_deref(), Some("HR schema"));
}

#[tokio::test]
async fn list_object_types_returns_flat_array() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/6/objecttypes/flat",
        ))
        .and(query_param("includeObjectCounts", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "19",
                "name": "Employee",
                "position": 0,
                "objectCount": 42,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            },
            {
                "id": "23",
                "name": "Office",
                "description": "Physical office or site.",
                "position": 2,
                "objectCount": 5,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            }
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let types = client.list_object_types("ws-123", "6").await.unwrap();
    assert_eq!(types.len(), 2);
    assert_eq!(types[0].name, "Employee");
    assert_eq!(types[0].object_count, 42);
    assert_eq!(types[1].name, "Office");
    assert_eq!(
        types[1].description.as_deref(),
        Some("Physical office or site.")
    );
}

#[tokio::test]
async fn schemas_json_lists_all_schemas() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/objectschema/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 2,
            "isLast": true,
            "values": [
                {
                    "id": "6",
                    "name": "ITSM",
                    "objectSchemaKey": "ITSM",
                    "status": "Ok",
                    "objectCount": 95,
                    "objectTypeCount": 34
                },
                {
                    "id": "1",
                    "name": "Human Resources",
                    "objectSchemaKey": "HR",
                    "status": "Ok",
                    "objectCount": 1023,
                    "objectTypeCount": 14
                }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let _guard = set_cache_dir(cache_dir.path()).await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["assets", "schemas", "--output", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "ITSM");
    assert_eq!(arr[0]["objectSchemaKey"], "ITSM");
    assert_eq!(arr[1]["name"], "Human Resources");
}

#[tokio::test]
async fn types_json_lists_all_types() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/objectschema/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [{
                "id": "6",
                "name": "ITSM",
                "objectSchemaKey": "ITSM",
                "status": "Ok",
                "objectCount": 95,
                "objectTypeCount": 2
            }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/6/objecttypes/flat",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "19",
                "name": "Employee",
                "position": 0,
                "objectCount": 42,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            },
            {
                "id": "23",
                "name": "Office",
                "description": "Physical office.",
                "position": 2,
                "objectCount": 5,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            }
        ])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let _guard = set_cache_dir(cache_dir.path()).await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["assets", "types", "--output", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Employee");
    assert_eq!(arr[0]["schemaName"], "ITSM");
    assert_eq!(arr[1]["name"], "Office");
}

#[tokio::test]
async fn schema_json_shows_attributes() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 50, "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/objectschema/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 25, "total": 1, "isLast": true,
            "values": [{
                "id": "6", "name": "ITSM", "objectSchemaKey": "ITSM",
                "status": "Ok", "objectCount": 95, "objectTypeCount": 2
            }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/6/objecttypes/flat",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "23", "name": "Office", "position": 2,
                "objectCount": 5, "objectSchemaId": "6",
                "inherited": false, "abstractObjectType": false
            }
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/23/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134", "name": "Key", "system": true, "hidden": false,
                "label": false, "position": 0,
                "defaultType": { "id": 0, "name": "Text" },
                "minimumCardinality": 1, "maximumCardinality": 1, "editable": false
            },
            {
                "id": "135", "name": "Name", "system": false, "hidden": false,
                "label": true, "position": 1,
                "defaultType": { "id": 0, "name": "Text" },
                "minimumCardinality": 1, "maximumCardinality": 1, "editable": true,
                "description": "The name of the object"
            },
            {
                "id": "869", "name": "Service relationships", "system": false,
                "hidden": false, "label": false, "position": 6,
                "referenceType": { "id": "36", "name": "Depends on" },
                "referenceObjectType": { "id": "122", "name": "Service" },
                "minimumCardinality": 0, "maximumCardinality": -1, "editable": true
            }
        ])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let _guard = set_cache_dir(cache_dir.path()).await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["assets", "schema", "Office", "--output", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0]["name"], "Key");
    assert_eq!(arr[0]["system"], true);
    assert_eq!(arr[2]["name"], "Service relationships");
    assert!(arr[2].get("referenceObjectType").is_some());
}

#[tokio::test]
async fn schema_table_filters_system_attrs() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 50, "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/objectschema/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 25, "total": 1, "isLast": true,
            "values": [{
                "id": "6", "name": "ITSM", "objectSchemaKey": "ITSM",
                "status": "Ok", "objectCount": 95, "objectTypeCount": 1
            }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/6/objecttypes/flat",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "23", "name": "Office", "position": 2,
                "objectCount": 5, "objectSchemaId": "6",
                "inherited": false, "abstractObjectType": false
            }
        ])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/23/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134", "name": "Key", "system": true, "hidden": false,
                "label": false, "position": 0,
                "defaultType": { "id": 0, "name": "Text" },
                "minimumCardinality": 1, "editable": false
            },
            {
                "id": "135", "name": "Name", "system": false, "hidden": false,
                "label": true, "position": 1,
                "defaultType": { "id": 0, "name": "Text" },
                "minimumCardinality": 1, "editable": true
            },
            {
                "id": "136", "name": "Created", "system": true, "hidden": false,
                "label": false, "position": 2,
                "defaultType": { "id": 6, "name": "DateTime" },
                "minimumCardinality": 1, "editable": false
            }
        ])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let _guard = set_cache_dir(cache_dir.path()).await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["assets", "schema", "Office"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("Object Type: Office"));
    assert!(stdout.contains("Name"));
    // System attrs "Key" and "Created" should be filtered out
    assert!(!stdout.contains("Created"));
}

/// Single-substring hit on `--schema` must route through Ambiguous and
/// error with exit 64. Locks the resolve_schema branch at assets.rs:471.
/// No object-type listing or object-schema-attribute fetch should occur.
#[tokio::test]
async fn schema_single_substring_schema_filter_rejected() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 50, "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/objectschema/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 25, "total": 2, "isLast": true,
            "values": [
                { "id": "6", "name": "ITSM", "objectSchemaKey": "ITSM",
                  "status": "Ok", "objectCount": 95, "objectTypeCount": 2 },
                { "id": "7", "name": "Office", "objectSchemaKey": "OFF",
                  "status": "Ok", "objectCount": 50, "objectTypeCount": 3 }
            ]
        })))
        .mount(&server)
        .await;

    // Assert no object-type listing fires — schema resolution must
    // short-circuit.
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/6/objecttypes/flat",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/7/objecttypes/flat",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let _guard = set_cache_dir(cache_dir.path()).await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--no-input",
            "assets",
            "schema",
            "AnyType",
            "--schema",
            "its",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure on ambiguous schema filter, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Ambiguous schema should exit 64 (UserError), got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous schema"),
        "Expected 'Ambiguous schema' in stderr: {stderr}"
    );
    assert!(
        stderr.contains("ITSM"),
        "Expected matched schema 'ITSM' in stderr: {stderr}"
    );
}
