#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_list_projects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::project_search_response(vec![
                common::fixtures::project_response(
                    "FOO",
                    "Project Foo",
                    "software",
                    Some("Jane Doe"),
                ),
                common::fixtures::project_response(
                    "BAR",
                    "Project Bar",
                    "service_desk",
                    Some("John Smith"),
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(None, Some(50)).await.unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].key, "FOO");
    assert_eq!(projects[0].name, "Project Foo");
    assert_eq!(projects[0].project_type_key, "software");
    assert_eq!(projects[0].lead.as_ref().unwrap().display_name, "Jane Doe");
    assert_eq!(projects[1].key, "BAR");
    assert_eq!(projects[1].project_type_key, "service_desk");
}

#[tokio::test]
async fn test_list_projects_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::project_search_response(vec![])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(None, Some(50)).await.unwrap();
    assert!(projects.is_empty());
}

#[tokio::test]
async fn test_list_projects_lead_missing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::project_search_response(vec![common::fixtures::project_response(
                "FOO",
                "Project Foo",
                "software",
                None,
            )]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(None, Some(50)).await.unwrap();
    assert_eq!(projects.len(), 1);
    assert!(projects[0].lead.is_none());
}

#[tokio::test]
async fn test_list_projects_with_type_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(query_param("typeKey", "software"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::project_search_response(vec![common::fixtures::project_response(
                "FOO",
                "Project Foo",
                "software",
                Some("Jane Doe"),
            )]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client
        .list_projects(Some("software"), Some(50))
        .await
        .unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].project_type_key, "software");
}

#[tokio::test]
async fn test_list_projects_all_paginates() {
    let server = MockServer::start().await;

    // Page 1: startAt=0, maxResults=50, total=3 (but only 2 returned → has_more=true)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [
                common::fixtures::project_response("FOO", "Project Foo", "software", Some("Jane")),
                common::fixtures::project_response("BAR", "Project Bar", "software", Some("John")),
            ],
            "startAt": 0,
            "maxResults": 2,
            "total": 3,
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Page 2: startAt=2, maxResults=2, total=3 (1 returned → last page)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(query_param("startAt", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [
                common::fixtures::project_response("BAZ", "Project Baz", "business", None),
            ],
            "startAt": 2,
            "maxResults": 2,
            "total": 3,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    // max_results=None triggers the --all pagination path
    let projects = client.list_projects(None, None).await.unwrap();
    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0].key, "FOO");
    assert_eq!(projects[1].key, "BAR");
    assert_eq!(projects[2].key, "BAZ");
}

#[tokio::test]
async fn test_get_project_statuses() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/statuses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::project_statuses_response()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.get_project_statuses("FOO").await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].name, "Task");
    assert_eq!(result[0].id, "3");
    assert_eq!(result[0].subtask, Some(false));
    assert_eq!(result[0].statuses.len(), 3);
    assert_eq!(result[0].statuses[0].name, "To Do");
    assert_eq!(result[0].statuses[0].id, "10000");
    assert_eq!(
        result[0].statuses[0].description.as_deref(),
        Some("Work that has not been started.")
    );
    assert_eq!(result[0].statuses[1].name, "In Progress");
    assert_eq!(result[0].statuses[2].name, "Done");
    assert_eq!(result[1].name, "Bug");
    assert_eq!(result[1].statuses.len(), 2);
}

#[tokio::test]
async fn test_get_project_statuses_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/statuses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.get_project_statuses("FOO").await.unwrap();
    assert!(result.is_empty());
}

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn project_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
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
        .args(["project", "list"])
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
async fn project_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
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
        .args(["project", "list"])
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
async fn project_list_network_drop_surfaces_reach_error() {
    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["project", "list"])
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
