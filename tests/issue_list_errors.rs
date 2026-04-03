#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn mock_project_exists(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ",
            "id": "10000",
            "name": "Test Project"
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn issue_list_board_config_404_reports_error() {
    let server = MockServer::start().await;

    // Project exists check passes
    mock_project_exists(&server).await;

    // Board config returns 404 (board deleted or no access)
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["Board does not exist or you do not have permission to see it."]
        })))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on board config 404, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("Board 42 not found or not accessible"),
        "Should mention board ID and accessibility, got: {stderr}"
    );
    assert!(
        stderr.contains("board_id"),
        "Should suggest removing board_id from config, got: {stderr}"
    );
    assert!(
        stderr.contains("--jql"),
        "Should suggest --jql as alternative, got: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Board-not-found should exit with UserError code 64, got: {:?}",
        output.status.code()
    );
}

#[tokio::test]
async fn issue_list_board_config_server_error_propagates() {
    let server = MockServer::start().await;

    // Project exists check passes
    mock_project_exists(&server).await;

    // Board config returns 500
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"]
        })))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on board config 500, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("Failed to fetch config for board 42"),
        "Should include board ID and context, got: {stderr}"
    );
    assert!(
        stderr.contains("--jql"),
        "Should suggest --jql as alternative, got: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "Server error should exit with code 1, got: {:?}",
        output.status.code()
    );
}

#[tokio::test]
async fn issue_list_sprint_error_propagates() {
    let server = MockServer::start().await;

    // Project exists check passes
    mock_project_exists(&server).await;

    // Board config succeeds → scrum board
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(&server)
        .await;

    // Sprint list returns 500
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"]
        })))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on sprint list error, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("Failed to list sprints for board 42"),
        "Should mention board ID and sprints, got: {stderr}"
    );
    assert!(
        stderr.contains("--jql"),
        "Should suggest --jql as alternative, got: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "Sprint list error should exit with code 1, got: {:?}",
        output.status.code()
    );
}

#[tokio::test]
async fn issue_list_no_active_sprint_falls_back_to_project_jql() {
    let server = MockServer::start().await;

    // Project exists check passes
    mock_project_exists(&server).await;

    // Board config succeeds → scrum board
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(&server)
        .await;

    // Sprint list returns empty (no active sprint)
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .and(query_param("state", "active"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_list_response(vec![])),
        )
        .mount(&server)
        .await;

    // Search endpoint returns issues (fallback JQL works)
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1",
                "Test Issue",
                "To Do",
            )]),
        ))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Should succeed with fallback JQL, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("PROJ-1"),
        "Should show fallback results, got: {stdout}"
    );
}
