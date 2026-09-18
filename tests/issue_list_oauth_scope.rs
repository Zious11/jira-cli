#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// F-WG-1 scope expansion — `jr issue list`'s board-resolution path
// (S-cycle8-agile-scope-mismatch-error-mapping widened to internal call
// sites that make the same Agile HTTP calls as `jr board`/`jr sprint` but
// were left unwrapped by the original story).
//
// Both call sites live in `src/cli/issue/list.rs`'s config-driven
// board/sprint JQL-composition branch (no `--jql` flag, `board_id` set in
// `.jr.toml`):
//   - `client.get_board_config(bid)` (~L451) — same endpoint, same required
//     scopes, as `board.rs::handle_view`'s unconditional config fetch
//     (`read:board-scope.admin:jira-software` + `read:project:jira`).
//   - `client.list_sprints(bid, Some("active"))` (~L455) — same endpoint,
//     same required scopes, as `board.rs::handle_view`'s scrum-branch sprint
//     fetch (`read:sprint:jira-software` + `read:issue-details:jira` +
//     `read:jql:jira`).
//
// RED GATE: today BOTH call sites are bare `.await` calls whose `Err` arm
// only adds `.context(...)` (see `issue_list_errors.rs`'s
// `issue_list_board_config_server_error_propagates` /
// `issue_list_sprint_error_propagates` for the pre-existing, unrelated
// generic-error coverage) — neither routes through
// `crate::cli::board::rewrite_agile_scope_error`. Under OAuth (Bearer) auth
// with a "scope does not match" 401, both tests below currently see the
// generic, POST-framed `InsufficientScope` template (issue #185) instead of
// the granular hint, so both FAIL until the implementer wires the rewrite
// into these two call sites exactly as `board.rs`/`sprint.rs` already do.
// ---------------------------------------------------------------------------

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

/// F-WG-1 site 1: `jr issue list`'s `get_board_config` call (board-resolution
/// for board-based JQL, no `--jql`, `board_id` configured in `.jr.toml`)
/// must get the SAME combined admin-scope hint as `board.rs::handle_view`'s
/// unconditional `get_board_config` call (F-WAVE-4,
/// `read:board-scope.admin:jira-software and read:project:jira`), not the
/// generic `InsufficientScope` template — under OAuth (Bearer) auth.
///
/// RED GATE: fails until `src/cli/issue/list.rs`'s `get_board_config` call
/// site (~L451) is wrapped with `rewrite_agile_scope_error`.
#[tokio::test]
async fn test_issue_list_board_config_401_scope_mismatch_names_admin_scope() {
    let server = MockServer::start().await;

    mock_project_exists(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Unauthorized; scope does not match"]
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
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
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
        "Scope-mismatch 401 on issue list's get_board_config call should exit 2, \
         got: {:?}; stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("read:board-scope.admin:jira-software and read:project:jira"),
        "Expected combined 'read:board-scope.admin:jira-software and read:project:jira' \
         scope hint in stderr (F-WAVE-4 parity with board.rs::handle_view), got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "Expected 'jr auth login' re-consent guidance in stderr, got: {stderr}"
    );
    assert!(
        !stderr.contains("github.com/Zious11/jira-cli/issues/185"),
        "Must NOT surface the generic POST-framed InsufficientScope template \
         (issue #185) for this OAuth Agile-scope-mismatch case, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

/// F-WG-1 site 2: `jr issue list`'s `list_sprints` call (reached after
/// `get_board_config` resolves the board as scrum) must get the SAME grouped
/// sprint-scope hint as `board.rs::handle_view`'s scrum-branch sprint fetch
/// (`read:sprint:jira-software, read:issue-details:jira, and
/// read:jql:jira`), not the generic `InsufficientScope` template — under
/// OAuth (Bearer) auth.
///
/// RED GATE: fails until `src/cli/issue/list.rs`'s `list_sprints` call site
/// (~L455) is wrapped with `rewrite_agile_scope_error`.
#[tokio::test]
async fn test_issue_list_sprints_401_scope_mismatch_names_grouped_scopes() {
    let server = MockServer::start().await;

    mock_project_exists(&server).await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Unauthorized; scope does not match"]
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
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
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
        "Scope-mismatch 401 on issue list's list_sprints call should exit 2, \
         got: {:?}; stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("read:sprint:jira-software"),
        "Expected 'read:sprint:jira-software' scope hint in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("read:issue-details:jira"),
        "Expected 'read:issue-details:jira' scope hint in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("read:jql:jira"),
        "Expected 'read:jql:jira' scope hint in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "Expected 'jr auth login' re-consent guidance in stderr, got: {stderr}"
    );
    assert!(
        !stderr.contains("github.com/Zious11/jira-cli/issues/185"),
        "Must NOT surface the generic POST-framed InsufficientScope template, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
