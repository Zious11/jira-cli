#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fields_response_with_cmdb() -> serde_json::Value {
    json!([
        {
            "id": "summary",
            "name": "Summary",
            "custom": false,
            "schema": { "type": "string" }
        },
        {
            "id": "customfield_10191",
            "name": "Client",
            "custom": true,
            "schema": {
                "type": "any",
                "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
                "customId": 10191
            }
        },
        {
            "id": "customfield_10031",
            "name": "Story Points",
            "custom": true,
            "schema": {
                "type": "number",
                "custom": "com.atlassian.jira.plugin.system.customfieldtypes:float",
                "customId": 10031
            }
        }
    ])
}

fn fields_response_no_cmdb() -> serde_json::Value {
    json!([
        {
            "id": "summary",
            "name": "Summary",
            "custom": false,
            "schema": { "type": "string" }
        }
    ])
}

#[tokio::test]
async fn discover_cmdb_field_ids() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_response_with_cmdb()))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let ids = client.find_cmdb_field_ids().await.unwrap();
    assert_eq!(ids, vec!["customfield_10191"]);
}

#[tokio::test]
async fn discover_cmdb_field_ids_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_response_no_cmdb()))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let ids = client.find_cmdb_field_ids().await.unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn issue_with_modern_cmdb_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": "PROJ-1",
            "fields": {
                "summary": "Test issue",
                "customfield_10191": [
                    {
                        "label": "Acme Corp",
                        "objectKey": "OBJ-1"
                    }
                ]
            }
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let issue = client
        .get_issue("PROJ-1", &["customfield_10191"])
        .await
        .unwrap();

    let cmdb_ids = vec!["customfield_10191".to_string()];
    let assets =
        jr::api::assets::linked::extract_linked_assets(&issue.fields.extra, &cmdb_ids);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].key.as_deref(), Some("OBJ-1"));
    assert_eq!(assets[0].name.as_deref(), Some("Acme Corp"));
}

#[tokio::test]
async fn issue_with_null_cmdb_field() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": "PROJ-2",
            "fields": {
                "summary": "No assets",
                "customfield_10191": null
            }
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let issue = client
        .get_issue("PROJ-2", &["customfield_10191"])
        .await
        .unwrap();

    let cmdb_ids = vec!["customfield_10191".to_string()];
    let assets =
        jr::api::assets::linked::extract_linked_assets(&issue.fields.extra, &cmdb_ids);
    assert!(assets.is_empty());
}

#[tokio::test]
async fn enrichment_resolves_ids_to_names() {
    let server = MockServer::start().await;

    // Mock workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    // Mock asset fetch
    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/88"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "88",
            "label": "Acme Corp",
            "objectKey": "OBJ-88",
            "objectType": { "id": "13", "name": "Client" }
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let mut assets = vec![jr::types::assets::LinkedAsset {
        id: Some("88".into()),
        workspace_id: Some("ws-123".into()),
        ..Default::default()
    }];

    jr::api::assets::linked::enrich_assets(&client, &mut assets).await;

    assert_eq!(assets[0].key.as_deref(), Some("OBJ-88"));
    assert_eq!(assets[0].name.as_deref(), Some("Acme Corp"));
    assert_eq!(assets[0].asset_type.as_deref(), Some("Client"));
}
