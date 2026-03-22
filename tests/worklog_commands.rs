#[allow(dead_code)]
mod common;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_add_worklog() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "12345",
            "timeSpentSeconds": 7200,
            "timeSpent": "2h",
            "author": {"accountId": "abc", "displayName": "Test User"}
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let worklog = client.add_worklog("FOO-1", 7200, None).await.unwrap();
    assert_eq!(worklog.time_spent_seconds, Some(7200));
}

#[tokio::test]
async fn test_list_worklogs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/worklog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0, "maxResults": 50, "total": 1,
            "worklogs": [{
                "id": "12345",
                "timeSpentSeconds": 3600,
                "timeSpent": "1h",
                "author": {"accountId": "abc", "displayName": "Test User"},
                "started": "2026-03-21T10:00:00.000+0000"
            }]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let worklogs = client.list_worklogs("FOO-1").await.unwrap();
    assert_eq!(worklogs.len(), 1);
    assert_eq!(worklogs[0].time_spent_seconds, Some(3600));
}
