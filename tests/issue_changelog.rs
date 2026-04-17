#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_changelog_single_page_returns_entries() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 100,
            "total": 1,
            "isLast": true,
            "values": [
                {
                    "id": "10000",
                    "author": { "accountId": "abc", "displayName": "Alice", "active": true },
                    "created": "2026-04-16T14:02:11.000+0000",
                    "items": [{
                        "field": "status", "fieldtype": "jira",
                        "from": "1", "fromString": "To Do",
                        "to": "3", "toString": "In Progress"
                    }]
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let entries = client.get_changelog("FOO-1").await.unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "10000");
    assert_eq!(entries[0].items[0].field, "status");
}

#[tokio::test]
async fn get_changelog_auto_paginates_across_pages() {
    let server = MockServer::start().await;

    // Page 1 (startAt=0, total=2, has_more because startAt+maxResults < total)
    // Use maxResults=1 to force a second page; client asks maxResults=100 but
    // the server can cap it — simulate that by returning total=2 with a
    // single entry in values[].
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2/changelog"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 1,
            "total": 2,
            "isLast": false,
            "values": [{
                "id": "1", "author": null,
                "created": "2026-04-10T00:00:00.000+0000",
                "items": [{"field": "status", "fieldtype": "jira",
                           "from": "1", "fromString": "To Do",
                           "to": "2", "toString": "In Progress"}]
            }]
        })))
        .mount(&server)
        .await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2/changelog"))
        .and(query_param("startAt", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 1,
            "maxResults": 1,
            "total": 2,
            "isLast": true,
            "values": [{
                "id": "2", "author": null,
                "created": "2026-04-11T00:00:00.000+0000",
                "items": [{"field": "status", "fieldtype": "jira",
                           "from": "2", "fromString": "In Progress",
                           "to": "3", "toString": "Done"}]
            }]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let entries = client.get_changelog("FOO-2").await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "1");
    assert_eq!(entries[1].id, "2");
}
