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

// ─── 401 + net-drop error coverage (#187) ──────────────────────────────────

#[tokio::test]
async fn issue_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    // Fail the first call (project-exists check) with 401.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
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
async fn issue_list_network_drop_surfaces_reach_error() {
    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
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

// ── partial_match single-substring rejection (issue #193) ────────────

/// Asserts `issue list --status <substring>` rejects a single-hit
/// substring with a disambiguation error and exit code 64, without
/// issuing a JQL search. Locks the handler-level guarantee from the
/// strict-matching rollout (unit-tested in src/partial_match.rs).
#[tokio::test]
async fn issue_list_status_single_substring_rejected() {
    let server = MockServer::start().await;

    // Project statuses response — candidates include "In Progress"; "prog"
    // is a single-hit substring → routes through Ambiguous.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ/statuses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::project_statuses_response()),
        )
        .mount(&server)
        .await;

    // Assert no JQL search fires — ambiguous status must short-circuit.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issues": [], "nextPageToken": null
        })))
        .expect(0)
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["--no-input", "issue", "list", "--status", "prog"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure on ambiguous substring, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Ambiguous status should exit 64 (UserError), got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous status"),
        "Expected 'Ambiguous status' in stderr: {stderr}"
    );
    assert!(
        stderr.contains("In Progress"),
        "Expected matched candidate 'In Progress' in stderr: {stderr}"
    );
}

// ── S-575-1: `--fields <CSV>` pre-HTTP validation (BC-2.2.033) ─────────────

/// AC-004 / BC-2.2.033 Precondition 2 / Edge Case EC-2.2.033-3: `--fields`
/// combined with table mode (default output, no `--output json`) exits 64
/// PRE-HTTP with the canonical hint, and issues zero HTTP calls.
#[tokio::test]
async fn issue_list_fields_table_mode_exits_64() {
    let server = MockServer::start().await;

    // Zero HTTP calls expected — the CSV/output-format gate must fire
    // before any request. `.expect(0)` makes wiremock itself fail the test
    // (on Drop) if the search endpoint is hit despite the guard.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issues": [], "nextPageToken": null
        })))
        .expect(0)
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list", "--fields", "summary,status"])
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
        Some(64),
        "table-mode --fields should exit 64 (UserError), got: {:?} (stderr: {stderr})",
        output.status.code()
    );
    assert!(
        stderr.contains("--fields requires --output json."),
        "Expected canonical hint in stderr, got: {stderr}"
    );
}

/// AC-005 / BC-2.2.033 Edge Cases EC-2.2.033-4/EC-2.2.033-5: an empty
/// (`""`), all-empty (`","`), or embedded-empty-segment (`"summary,,status"`)
/// `--fields` CSV is rejected PRE-HTTP with exit 64 — an empty segment is
/// REJECTED, not silently dropped — and issues zero HTTP calls.
#[tokio::test]
async fn issue_list_fields_empty_csv_exits_64_pre_http() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issues": [], "nextPageToken": null
        })))
        .expect(0)
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    for bad_csv in ["", ",", "summary,,status"] {
        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .current_dir(project_dir.path())
            .args(["issue", "list", "--fields", bad_csv, "--output", "json"])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "--fields {bad_csv:?} should exit 64 pre-HTTP, got: {:?} (stderr: {stderr})",
            output.status.code()
        );
    }
}

/// AC-004 / BC-2.1.024 Postcondition 2 / EC-2.1.024-3..7 (S-588-1): every
/// malformed `--sort` shape -- missing `:`, empty field segment, empty
/// direction segment, an invalid direction, and a second `:` embedded in the
/// direction segment -- exits 64 (`JrError::UserError`) PRE-HTTP (before any
/// board/sprint/project resolution or issue search) with the exact pinned
/// stderr literal. Zero HTTP calls of any kind fire (both GET and POST are
/// `.expect(0)`-pinned).
#[tokio::test]
async fn issue_list_sort_malformed_input_exits_64_pre_http() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issues": [], "nextPageToken": null
        })))
        .expect(0)
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    for bad in [
        "updated",
        ":desc",
        "updated:",
        "updated:sideways",
        "updated:desc:extra",
    ] {
        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .current_dir(project_dir.path())
            .args(["--no-input", "issue", "list", "--sort", bad])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "--sort {bad:?} should exit 64 pre-HTTP, got: {:?} (stderr: {stderr})",
            output.status.code()
        );
        assert!(
            stderr.contains(&format!(
                "Invalid --sort \"{bad}\". Use <field>:asc or <field>:desc (e.g., updated:desc)."
            )),
            "AC-004: expected the exact pinned stderr literal for --sort \
             {bad:?}, got: {stderr}"
        );
    }
}

/// AC-008 / BC-2.1.025 Edge Case EC-2.1.025-5: `--sort` performs NO local
/// field-name rejection -- an unknown/unorderable field (e.g.
/// `customfield_10099`) is passed through to Jira unvalidated, the
/// `POST /rest/api/3/search/jql` call IS made, and Jira's 400 response
/// propagates as `JrError::ApiError { status: 400, .. }` (exit 1) via the
/// existing generic HTTP-error path -- not a local pre-HTTP rejection.
#[tokio::test]
async fn issue_list_sort_unknown_field_propagates_jira_400() {
    let server = MockServer::start().await;
    mock_project_exists(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": [
                "The value 'customfield_10099' does not exist for the field 'sort'."
            ]
        })))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--sort",
            "customfield_10099:desc",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "AC-008: an unknown/unorderable --sort field must propagate Jira's \
         400 as JrError::ApiError (exit 1), NOT a local pre-HTTP rejection \
         (exit 64); got: {:?} (stderr: {stderr})",
        output.status.code()
    );

    let received = server.received_requests().await.unwrap();
    let search_calls = received
        .iter()
        .filter(|r| {
            r.method == wiremock::http::Method::POST && r.url.path() == "/rest/api/3/search/jql"
        })
        .count();
    assert_eq!(
        search_calls, 1,
        "AC-008: exactly one search call must fire -- BC-2.1.025 \
         Precondition 1 forbids a local field-name allowlist, so the request \
         must reach Jira for the 400 to be observed at all"
    );
}
