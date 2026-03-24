#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn project_meta_cache_miss_fetches_from_api() {
    let cache_dir = tempfile::tempdir().unwrap();
    // SAFETY: tests run in separate processes (each integration test file is its own
    // binary), so mutating XDG_CACHE_HOME here does not race with other test binaries.
    // Within this binary the three tests are async and tokio runs them on a single
    // thread by default, so there is no intra-binary data race on the env.
    unsafe { std::env::set_var("XDG_CACHE_HOME", cache_dir.path()) };

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELPDESK"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10042",
            "key": "HELPDESK",
            "name": "Help Desk",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "15", "projectId": "10042", "projectName": "Help Desk" }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let meta = jr::api::jsm::servicedesks::get_or_fetch_project_meta(&client, "HELPDESK")
        .await
        .unwrap();

    assert_eq!(meta.project_type, "service_desk");
    assert_eq!(meta.project_id, "10042");
    assert_eq!(meta.service_desk_id.as_deref(), Some("15"));
    assert!(!meta.simplified);
}

#[tokio::test]
async fn project_meta_software_project_has_no_service_desk_id() {
    let cache_dir = tempfile::tempdir().unwrap();
    // SAFETY: see project_meta_cache_miss_fetches_from_api for rationale.
    unsafe { std::env::set_var("XDG_CACHE_HOME", cache_dir.path()) };

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10001",
            "key": "DEV",
            "name": "Development",
            "projectTypeKey": "software",
            "simplified": true
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let meta = jr::api::jsm::servicedesks::get_or_fetch_project_meta(&client, "DEV")
        .await
        .unwrap();

    assert_eq!(meta.project_type, "software");
    assert!(meta.service_desk_id.is_none());
    assert!(meta.simplified);
}

#[tokio::test]
async fn require_service_desk_errors_for_software_project() {
    let cache_dir = tempfile::tempdir().unwrap();
    // SAFETY: see project_meta_cache_miss_fetches_from_api for rationale.
    unsafe { std::env::set_var("XDG_CACHE_HOME", cache_dir.path()) };

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10001",
            "key": "DEV",
            "name": "Development",
            "projectTypeKey": "software",
            "simplified": true
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = jr::api::jsm::servicedesks::require_service_desk(&client, "DEV").await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Jira Software project"));
    assert!(err.contains("Queue commands require"));
}
