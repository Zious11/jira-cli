#[allow(dead_code)]
mod common;

use serde_json::json;
use tokio::sync::Mutex;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Serialize project_meta tests — they share the XDG_CACHE_HOME env var.
/// The guard MUST be held for the entire test body, not just the set_var call,
/// to prevent another test from changing XDG_CACHE_HOME while async work is in progress.
static ENV_MUTEX: Mutex<()> = Mutex::const_new(());

/// Acquire the env mutex and set XDG_CACHE_HOME. Caller MUST hold the returned
/// guard for the duration of the test to prevent env var races.
async fn set_cache_dir(dir: &std::path::Path) -> tokio::sync::MutexGuard<'static, ()> {
    let guard = ENV_MUTEX.lock().await;
    // SAFETY: ENV_MUTEX guard is held by caller for the entire test duration,
    // and all tests use current_thread flavor so no concurrent env mutation.
    unsafe { std::env::set_var("XDG_CACHE_HOME", dir) };
    guard
}

#[tokio::test(flavor = "current_thread")]
async fn project_meta_cache_miss_fetches_from_api() {
    let cache_dir = tempfile::tempdir().unwrap();
    let _env_guard = set_cache_dir(cache_dir.path()).await;

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

#[tokio::test(flavor = "current_thread")]
async fn project_meta_software_project_has_no_service_desk_id() {
    let cache_dir = tempfile::tempdir().unwrap();
    let _env_guard = set_cache_dir(cache_dir.path()).await;

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
    let meta = jr::api::jsm::servicedesks::get_or_fetch_project_meta(&client, "DEV")
        .await
        .unwrap();

    assert_eq!(meta.project_type, "software");
    assert!(meta.service_desk_id.is_none());
    assert!(meta.simplified);
}

#[tokio::test(flavor = "current_thread")]
async fn require_service_desk_errors_for_software_project() {
    let cache_dir = tempfile::tempdir().unwrap();
    let _env_guard = set_cache_dir(cache_dir.path()).await;

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
