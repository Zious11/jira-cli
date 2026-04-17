#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: build N issues for testing.
fn make_issues(count: usize) -> Vec<serde_json::Value> {
    (1..=count)
        .map(|i| {
            common::fixtures::issue_response(
                &format!("TEST-{}", i),
                &format!("Issue {}", i),
                "In Progress",
            )
        })
        .collect()
}

// --- Board view --limit tests (from PR #73) ---

#[tokio::test]
async fn get_sprint_issues_with_limit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(make_issues(5), 5)),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = client
        .get_sprint_issues(100, None, Some(3), &[])
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 3);
    assert!(result.has_more);
    assert_eq!(result.issues[0].key, "TEST-1");
    assert_eq!(result.issues[2].key, "TEST-3");
}

#[tokio::test]
async fn get_sprint_issues_no_limit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(make_issues(5), 5)),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = client
        .get_sprint_issues(100, None, None, &[])
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 5);
    assert!(!result.has_more);
}

#[tokio::test]
async fn search_issues_with_limit() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response_with_next_page(make_issues(5)),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = client
        .search_issues("statusCategory != Done ORDER BY rank ASC", Some(3), &[])
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 3);
    assert!(result.has_more);
}

#[test]
fn board_view_limit_and_all_conflict() {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.arg("board")
        .arg("view")
        .arg("--limit")
        .arg("3")
        .arg("--all");

    cmd.assert().failure().code(2);
}

// --- Board auto-resolve tests (from #70) ---

#[tokio::test]
async fn list_boards_with_project_and_type_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![common::fixtures::board_response(
                42, "My Board", "scrum", "PROJ",
            )]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let boards = client
        .list_boards(Some("PROJ"), Some("scrum"))
        .await
        .unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, 42);
    assert_eq!(boards[0].name, "My Board");
}

#[tokio::test]
async fn list_boards_without_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![
                common::fixtures::board_response(1, "Board A", "scrum", "FOO"),
                common::fixtures::board_response(2, "Board B", "kanban", "BAR"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let boards = client.list_boards(None, None).await.unwrap();
    assert_eq!(boards.len(), 2);
}

#[tokio::test]
async fn list_boards_empty_result() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "NOPE"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::board_list_response(vec![])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let boards = client.list_boards(Some("NOPE"), None).await.unwrap();
    assert!(boards.is_empty());
}

#[tokio::test]
async fn resolve_board_auto_discovers_single_scrum_board() {
    let server = MockServer::start().await;

    // list_boards filtered by project+scrum returns 1 board
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![common::fixtures::board_response(
                42,
                "PROJ Scrum Board",
                "scrum",
                "PROJ",
            )]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    let board_id = jr::cli::board::resolve_board_id(&config, &client, None, Some("PROJ"), true)
        .await
        .unwrap();
    assert_eq!(board_id, 42);
}

#[tokio::test]
async fn resolve_board_errors_on_multiple_boards() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![
                common::fixtures::board_response(42, "Board A", "scrum", "PROJ"),
                common::fixtures::board_response(99, "Board B", "scrum", "PROJ"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    let err = jr::cli::board::resolve_board_id(&config, &client, None, Some("PROJ"), true)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Multiple scrum boards"), "got: {msg}");
    assert!(msg.contains("42"), "should list board ID 42, got: {msg}");
    assert!(msg.contains("99"), "should list board ID 99, got: {msg}");
}

#[tokio::test]
async fn resolve_board_errors_on_no_boards() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "NOPE"))
        .and(query_param("type", "scrum"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::board_list_response(vec![])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    let err = jr::cli::board::resolve_board_id(&config, &client, None, Some("NOPE"), true)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("No scrum boards found"), "got: {msg}");
    assert!(
        msg.contains("NOPE"),
        "should mention project key, got: {msg}"
    );
}

#[tokio::test]
async fn resolve_board_uses_explicit_board_override() {
    let server = MockServer::start().await;
    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    let board_id = jr::cli::board::resolve_board_id(&config, &client, Some(42), None, true)
        .await
        .unwrap();
    assert_eq!(board_id, 42);
}

#[tokio::test]
async fn resolve_board_errors_without_project_or_board() {
    let server = MockServer::start().await;
    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    let err = jr::cli::board::resolve_board_id(&config, &client, None, None, true)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("No board configured"), "got: {msg}");
    assert!(
        msg.contains("--project"),
        "should suggest --project, got: {msg}"
    );
}

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn board_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
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
        .args(["board", "list"])
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
async fn board_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
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
        .args(["board", "list"])
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
async fn board_list_network_drop_surfaces_reach_error() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["board", "list"])
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
