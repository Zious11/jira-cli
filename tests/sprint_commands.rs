#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// BC-X.15.001 — `jr sprint` 401 scope-mismatch disambiguation
// (S-cycle8-agile-scope-mismatch-error-mapping)
//
// See the parallel block at the top of tests/board_commands.rs for the full
// RED-vs-boundary-guard breakdown shared by both files. Summary for the
// tests in this file:
// - `test_bc_x_15_001_sprint_list_401_scope_mismatch_names_missing_scopes`
//   and `test_bc_x_15_001_sprint_remove_401_scope_mismatch_names_missing_scope`
//   assert PRESENCE of new behavior — genuine RED against current code.
// - `test_bc_x_15_001_sprint_401_under_api_token_unaffected` asserts ABSENCE
//   of the new rewrite for Basic auth — passes today (boundary guard).
// - `test_bc_x_15_001_sprint_401_without_scope_substring_falls_through_to_refresh`
//   is keyring-gated and `#[ignore]`d for the same reason as its board.rs
//   sibling; not executed in this Red Gate's default `cargo test` pass.
// ---------------------------------------------------------------------------

/// AC-001 (BC-X.15.001 Behavior clause 1): `jr sprint list`'s 401 with a
/// case-insensitive "scope does not match" body, under OAuth (Bearer) auth,
/// must be rewritten to name `read:sprint:jira-software`,
/// `read:issue-details:jira`, and `read:jql:jira`, and direct to
/// `jr auth login` — not the generic `InsufficientScope` template.
///
/// `--board 55` bypasses project-based board auto-discovery; the board
/// configuration call is mocked to succeed (scrum) so the mocked 401 is
/// deterministically hit on the sprint-list call itself.
///
/// RED GATE: fails until the rewrite is wired into `jr sprint list`.
#[tokio::test]
async fn test_bc_x_15_001_sprint_list_401_scope_mismatch_names_missing_scopes() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/55/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/55/sprint"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["sprint", "list", "--board", "55"])
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
        "Scope-mismatch 401 should exit 2, got: {:?}; stderr: {stderr}",
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

/// AC-001 (BC-X.15.001 Behavior clause 1): `jr sprint remove`'s 401 with a
/// case-insensitive "scope does not match" body, under OAuth (Bearer) auth,
/// must be rewritten to name `write:board-scope:jira-software` and direct
/// to `jr auth login`. `sprint remove` is used as the representative
/// write-scope command (no board resolution needed — it calls
/// `move_issues_to_backlog` directly), matching AC-001's "`jr sprint
/// add`/`jr sprint remove`" family.
///
/// RED GATE: fails until the rewrite is wired into `jr sprint remove`.
#[tokio::test]
async fn test_bc_x_15_001_sprint_remove_401_scope_mismatch_names_missing_scope() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["sprint", "remove", "FOO-1"])
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
        "Scope-mismatch 401 should exit 2, got: {:?}; stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("write:board-scope:jira-software"),
        "Expected 'write:board-scope:jira-software' scope hint in stderr, got: {stderr}"
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

/// AC-004 (BC-X.15.001 Behavior clause 4): under Basic (API-token) auth, the
/// SAME "scope does not match" 401 body used above must continue to surface
/// via the pre-existing generic `InsufficientScope` path, unchanged.
///
/// BOUNDARY GUARD, not RED: passes today (no rewrite exists yet); pins the
/// invariant the implementer's change must not violate.
#[tokio::test]
async fn test_bc_x_15_001_sprint_401_under_api_token_unaffected() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["sprint", "remove", "FOO-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "Scope-mismatch 401 under Basic auth should still exit 2, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Insufficient token scope"),
        "Expected the OLD generic InsufficientScope template to fire for \
         Basic auth, got: {stderr}"
    );
    assert!(
        stderr.contains("github.com/Zious11/jira-cli/issues/185"),
        "Expected the OLD template's issue #185 reference for Basic auth, got: {stderr}"
    );
    assert!(
        !stderr.contains("write:board-scope:jira-software"),
        "The new OAuth-only granular-scope hint must NOT fire under Basic \
         auth, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

/// AC-002 (BC-X.15.001 Behavior clause 2): a 401 without the scope-mismatch
/// substring, under OAuth auth, must be left UNCHANGED to fall through to
/// the existing auto-refresh coordinator.
///
/// See the identically-reasoned board.rs sibling
/// (`test_bc_x_15_001_board_401_without_scope_substring_falls_through_to_refresh`
/// in tests/board_commands.rs) for the full keyring-risk rationale. Gated
/// behind `JR_RUN_KEYRING_TESTS=1` + `#[ignore]`; NOT executed as part of
/// this Red Gate's default `cargo test` pass.
///
/// KEYRING GATE: `JR_RUN_KEYRING_TESTS=1 JR_SERVICE_NAME=<unique> cargo test \
/// --test sprint_commands -- --ignored` to run for real.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_bc_x_15_001_sprint_401_without_scope_substring_falls_through_to_refresh() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }

    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("JR_SERVICE_NAME", "jr-s4-agile-401-test");
    }

    let server = MockServer::start().await;

    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(
            "JR_OAUTH_TOKEN_URL",
            format!("{}/oauth/token/bc-x-15-001-sprint", server.uri()),
        );
    }

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": [
                "The access token provided is expired, revoked, malformed, \
                 or invalid for other reasons."
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/oauth/token/bc-x-15-001-sprint"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant"
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["sprint", "remove", "FOO-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure (no valid refresh token available), got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !stderr.contains("write:board-scope:jira-software"),
        "The new call-site rewrite must NOT fire for a non-scope-mismatch \
         401 — it must fall through to the auto-refresh coordinator, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");

    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("JR_OAUTH_TOKEN_URL");
        std::env::remove_var("JR_SERVICE_NAME");
    }
}

// ---------------------------------------------------------------------------
// BC-X.15.001 v1.1 F1 scope expansion (2026-09-17) — AC-009(b), AC-012(a/b):
// internal board/sprint resolution call sites reachable from `jr sprint`
// commands, not just their top-level command handlers. See the story spec's
// v1.1 update (S-cycle8-agile-scope-mismatch-error-mapping) and
// BC-X.15.001's widened Behavior clause 1 call-site → hint mapping. See also
// the parallel AC-009(a)/AC-011 block in tests/board_commands.rs.
//
// RED GATE STATUS: all three tests below assert PRESENCE of the shared
// `rewrite_agile_scope_error` hint at call sites that are, as of this
// writing, genuinely UNWRAPPED (`resolve_scrum_board`'s `get_board_config`
// call and `SprintCommand::Add { current: true, .. }`'s `list_sprints`
// lookup both use a bare `.await?` with no `.map_err(...)` rewrite) —
// genuine RED against current code.
// ---------------------------------------------------------------------------

/// AC-009(b) (BC-X.15.001 Behavior clause 1, widened): `board.rs::resolve_board_id`'s
/// auto-discovery `list_boards` call, reached TRANSITIVELY via
/// `sprint.rs::resolve_scrum_board` when `jr sprint list` is invoked with no
/// `--board` override, must get the SAME hint as `jr board list` —
/// `read:board-scope:jira-software` + `read:project:jira` — on a
/// scope-mismatch 401. `require_scrum=true` on this path, so the mocked
/// `list_boards` call carries `type=scrum`.
///
/// RED GATE: `resolve_board_id`'s `list_boards` call is a bare `.await?`
/// today — fails until wired (shared fix with the `board.rs` AC-009(a)
/// sibling, since both paths go through the same helper function).
#[tokio::test]
async fn test_bc_x_15_001_sprint_resolve_board_id_401_scope_mismatch_rewrite() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["--project", "PROJ", "sprint", "list"])
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
        "Scope-mismatch 401 on resolve_board_id's list_boards call should exit 2, \
         got: {:?}; stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("read:board-scope:jira-software"),
        "Expected 'read:board-scope:jira-software' scope hint in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("read:project:jira"),
        "Expected 'read:project:jira' scope hint in stderr, got: {stderr}"
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

/// AC-012(a) (BC-X.15.001 Behavior clause 1, widened): `sprint.rs::resolve_scrum_board`'s
/// `get_board_config` call — shared by `sprint list`/`current`/`add`/`remove`
/// — must get the SAME hint as `board.rs::handle_view`'s unconditional
/// `get_board_config` call (AC-010): `read:board-scope.admin:jira-software`.
/// `--board 42` bypasses `resolve_board_id`'s own `list_boards` call so the
/// mocked 401 is deterministically hit on `resolve_scrum_board`'s
/// `get_board_config` call instead. Exercised via all three call paths that
/// route through `resolve_scrum_board` to confirm the shared helper's hint
/// is identical across each.
///
/// RED GATE: `resolve_scrum_board`'s `get_board_config` call is a bare
/// `.await?` today — fails until wired.
#[tokio::test]
async fn test_bc_x_15_001_resolve_scrum_board_get_board_config_401_scope_mismatch_rewrite() {
    async fn assert_admin_scope_hint_fires(args: &[&str]) {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/agile/1.0/board/42/configuration"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "errorMessages": ["Unauthorized; scope does not match"]
            })))
            .mount(&server)
            .await;

        let output = Command::cargo_bin("jr")
            .unwrap()
            .env("JR_BASE_URL", server.uri())
            .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
            .args(args)
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "Expected failure for args {args:?}, got stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert_eq!(
            output.status.code(),
            Some(2),
            "Scope-mismatch 401 on resolve_scrum_board's get_board_config call \
             should exit 2 for args {args:?}, got: {:?}; stderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("read:board-scope.admin:jira-software"),
            "Expected 'read:board-scope.admin:jira-software' scope hint in stderr \
             for args {args:?}, got: {stderr}"
        );
        assert!(
            stderr.contains("jr auth login"),
            "Expected 'jr auth login' re-consent guidance in stderr for args {args:?}, \
             got: {stderr}"
        );
        assert!(
            !stderr.contains("github.com/Zious11/jira-cli/issues/185"),
            "Must NOT surface the generic POST-framed InsufficientScope template \
             for args {args:?}, got: {stderr}"
        );
        assert!(
            !stderr.contains("panic"),
            "stderr leaked a panic for args {args:?}: {stderr}"
        );
    }

    assert_admin_scope_hint_fires(&["sprint", "list", "--board", "42"]).await;
    assert_admin_scope_hint_fires(&["sprint", "current", "--board", "42"]).await;
    assert_admin_scope_hint_fires(&["sprint", "add", "--current", "--board", "42", "FOO-1"]).await;
}

/// AC-012(b) (BC-X.15.001 Behavior clause 1, widened): `SprintCommand::Add {
/// current: true, .. }`'s active-sprint-id resolution `list_sprints` call —
/// a SEPARATE call from `resolve_scrum_board`'s preceding `get_board_config`
/// check in the same command invocation — must get the grouped sprint-scope
/// hint (`read:sprint:jira-software` + `read:issue-details:jira` +
/// `read:jql:jira`, EC-X.15.001-4), distinct from AC-012(a)'s admin-scope
/// hint. `get_board_config` succeeds here (scrum) so the mocked 401 lands on
/// the `list_sprints` lookup instead.
///
/// RED GATE: `SprintCommand::Add`'s `list_sprints` call is a bare `.await?`
/// today — fails until wired.
#[tokio::test]
async fn test_bc_x_15_001_sprint_add_current_list_sprints_401_scope_mismatch_rewrite() {
    let server = MockServer::start().await;

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
        .and(query_param("state", "active"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["sprint", "add", "--current", "--board", "42", "FOO-1"])
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
        "Scope-mismatch 401 on SprintCommand::Add's list_sprints lookup should \
         exit 2, got: {:?}; stderr: {stderr}",
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
        !stderr.contains("read:board-scope.admin:jira-software"),
        "Must NOT surface AC-012(a)'s admin-scope hint here — the 401 landed \
         on the list_sprints call, not the get_board_config call, got: {stderr}"
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

/// Mount prereq mocks (board list, board config, active sprint) on the server.
async fn mount_prereqs(server: &MockServer) {
    // Board auto-resolve: list boards for project PROJ, type=scrum → 1 board
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
        .mount(server)
        .await;

    // Board config → scrum
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(server)
        .await;

    // Active sprint list → one sprint
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .and(query_param("state", "active"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::sprint_list_response(vec![common::fixtures::sprint(
                100, "Sprint 1", "active",
            )]),
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn sprint_current_default_limit_caps_at_30() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(35);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show exactly 30 issues (default limit)
    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 30, "Expected 30 issues, got {issue_count}");

    // Should show "more results" hint
    assert!(
        stderr.contains("Showing 30 results"),
        "Expected 'Showing 30 results' in stderr, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_current_limit_flag() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(20);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 20)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .arg("--limit")
        .arg("5")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 5, "Expected 5 issues, got {issue_count}");

    assert!(
        stderr.contains("Showing 5 results"),
        "Expected 'Showing 5 results' in stderr, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_current_all_flag_returns_everything() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(35);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .arg("--all")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 35, "Expected 35 issues, got {issue_count}");

    assert!(
        !stderr.contains("Showing"),
        "Should NOT show 'Showing' hint with --all, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_current_under_limit_no_hint() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(10);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 10)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 10, "Expected 10 issues, got {issue_count}");

    assert!(
        !stderr.contains("Showing"),
        "Should NOT show hint when under limit, got: {stderr}"
    );
}

#[test]
fn sprint_current_limit_and_all_conflict() {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.arg("sprint")
        .arg("current")
        .arg("--limit")
        .arg("3")
        .arg("--all");

    cmd.assert().failure().code(2);
}

#[tokio::test]
async fn sprint_add_with_sprint_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .and(body_partial_json(json!({"issues": ["FOO-1", "FOO-2"]})))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["sprint", "add", "--sprint", "100", "FOO-1", "FOO-2"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, got: {:?}",
        output
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Added 2 issue(s) to sprint 100"),
        "Expected success message, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_add_json_output() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/200/issue"))
        .and(body_partial_json(json!({"issues": ["BAR-1"]})))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--output", "json", "sprint", "add", "--sprint", "200", "BAR-1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, got: {:?}",
        output
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["sprint_id"], 200);
    assert_eq!(parsed["issues"], serde_json::json!(["BAR-1"]));
    assert_eq!(parsed["added"], true);
}

#[tokio::test]
async fn sprint_remove_moves_to_backlog() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .and(body_partial_json(json!({"issues": ["FOO-1", "FOO-3"]})))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["sprint", "remove", "FOO-1", "FOO-3"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, got: {:?}",
        output
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Moved 2 issue(s) to backlog"),
        "Expected success message, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_remove_json_output() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .and(body_partial_json(json!({"issues": ["QUX-5"]})))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--output", "json", "sprint", "remove", "QUX-5"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, got: {:?}",
        output
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["issues"], serde_json::json!(["QUX-5"]));
    assert_eq!(parsed["removed"], true);
}

#[tokio::test]
async fn sprint_add_with_current_flag() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .and(body_partial_json(json!({"issues": ["TEST-1", "TEST-2"]})))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .args(["sprint", "add", "--current", "TEST-1", "TEST-2"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, got: {:?}",
        output
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Added 2 issue(s) to sprint 100"),
        "Expected success message, got: {stderr}"
    );
}

// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn sprint_current_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    // Fail on the first call in the sprint-current chain (board auto-resolve).
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--project", "PROJ", "sprint", "current"])
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
async fn sprint_current_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--project", "PROJ", "sprint", "current"])
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
async fn sprint_current_network_drop_surfaces_reach_error() {
    // Privileged port 1 — connect-refused from any unprivileged process.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--project", "PROJ", "sprint", "current"])
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

// ---------------------------------------------------------------------------
// OBS-1 mutation-coverage gap closure (S-cycle8-agile-scope-mismatch-error-mapping)
//
// `handle_add`'s `client.add_issues_to_sprint(...)` map_err (the
// `write:board-scope:jira-software` rewrite) was, until this test, exercised
// only structurally — `test_bc_x_15_001_sprint_remove_401_scope_mismatch_names_missing_scope`
// covers the sibling `move_issues_to_backlog` write-scope call, and
// `test_bc_x_15_001_sprint_add_current_list_sprints_401_scope_mismatch_rewrite`
// covers `SprintCommand::Add { current: true, .. }`'s `list_sprints` lookup,
// but no test landed a 401 directly on `add_issues_to_sprint` itself. That
// left the `handle_add` map_err's mutation coverage weaker than its sibling
// call sites — a PR-diff-scoped `cargo mutants` run over this area could plausibly
// miss a mutant deleting or short-circuiting that map_err. This test closes
// that gap directly, mirroring the `sprint remove` write-hint test's
// structure/fixtures (adversary finding OBS-1).
// ---------------------------------------------------------------------------

/// AC-001 (BC-X.15.001 Behavior clause 1): `jr sprint add`'s 401 with a
/// case-insensitive "scope does not match" body, under OAuth (Bearer) auth,
/// must be rewritten to name `write:board-scope:jira-software` and direct to
/// `jr auth login` — not the generic `InsufficientScope` template.
///
/// Uses `--sprint 55` (not `--current`) so `SprintCommand::Add`'s `current`
/// branch — which would otherwise issue its own `list_boards`/`list_sprints`
/// calls ahead of `add_issues_to_sprint` — is never reached; control passes
/// straight to `handle_add`, and the mocked 401 lands deterministically on
/// `add_issues_to_sprint` (`POST /rest/agile/1.0/sprint/55/issue`) itself, not
/// on any preceding board/sprint resolution call.
///
/// This is a direct regression test for `handle_add`'s
/// `client.add_issues_to_sprint(...).await.map_err(|e| rewrite_agile_scope_error(...))`
/// wrap in `src/cli/sprint.rs`: removing that `.map_err(...)` (reverting the
/// call to a bare `.await?`) would let the raw `add_issues_to_sprint` error
/// surface unchanged — under OAuth with a "scope does not match" body, that
/// raw error is the generic, POST-framed `InsufficientScope` (issue #185)
/// message. This test's assertions would then fail on two independent
/// fronts: the `write:board-scope:jira-software` hint would be absent, and
/// the `github.com/Zious11/jira-cli/issues/185` template WOULD be present —
/// so a mutant that deletes/no-ops this specific map_err cannot survive.
#[tokio::test]
async fn test_bc_x_15_001_sprint_add_401_scope_mismatch_names_missing_scope() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/55/issue"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["sprint", "add", "--sprint", "55", "FOO-1"])
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
        "Scope-mismatch 401 on add_issues_to_sprint should exit 2, got: {:?}; stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("write:board-scope:jira-software"),
        "Expected 'write:board-scope:jira-software' scope hint in stderr, got: {stderr}"
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
