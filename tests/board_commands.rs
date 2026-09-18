#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// BC-X.15.001 — `jr board` 401 scope-mismatch disambiguation
// (S-cycle8-agile-scope-mismatch-error-mapping)
//
// RED GATE STATUS (see also the sibling block in tests/sprint_commands.rs and
// the handback report for the full breakdown):
//
// - `test_bc_x_15_001_board_list_401_scope_mismatch_names_missing_scopes`,
//   `test_bc_x_15_001_board_view_401_scope_mismatch_names_admin_scope`, and
//   `test_bc_x_15_001_board_401_composite_body_scope_mismatch_wins` assert
//   PRESENCE of new behavior (a granular-scope hint that does not exist in
//   `src/error.rs`/`src/api/client.rs` today) — these FAIL against current
//   code (genuine RED).
// - `test_bc_x_15_001_board_401_under_api_token_unaffected` and
//   `test_bc_x_15_001_board_401_unrecognized_body_falls_through_unchanged`
//   assert ABSENCE of the new rewrite for cases the story's call-site
//   rewrite must never intercept (Basic-auth scope-mismatch; unrecognized
//   body). Since no rewrite exists yet at all, these two currently PASS
//   (vacuously true pre-implementation) — they are regression/boundary
//   guards, not RED tests, and must keep passing after the implementer's
//   change lands. This is intentional per AC-004/AC-006's own wording ("the
//   OLD generic path fires" / "falls through unchanged") and is called out
//   explicitly rather than forced into a false RED.
// ---------------------------------------------------------------------------

/// AC-001 (BC-X.15.001 Behavior clause 1): `jr board list`'s 401 with a
/// case-insensitive "scope does not match" body, under OAuth (Bearer) auth,
/// must be rewritten to a `NotAuthenticated` hint naming the exact granular
/// scopes `read:board-scope:jira-software` + `read:project:jira` and
/// directing to `jr auth login` — NOT the generic, POST-framed
/// `InsufficientScope` template (issue #185).
///
/// RED GATE: today `send_inner`'s existing scope-mismatch check (BC-1.6.043)
/// returns the generic `InsufficientScope` Display template unconditionally;
/// `board.rs` has no call-site rewrite. This test fails until the rewrite is
/// wired into `jr board list`.
#[tokio::test]
async fn test_bc_x_15_001_board_list_401_scope_mismatch_names_missing_scopes() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
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
        "Scope-mismatch 401 should exit 2, got: {:?}; stderr: {stderr}",
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
        "Must NOT surface the generic POST-framed InsufficientScope template \
         (issue #185) for this OAuth Agile-scope-mismatch case, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

/// AC-001 (BC-X.15.001 Behavior clause 1): `jr board view`'s config-fetch
/// (`GET /rest/agile/1.0/board/{id}/configuration`, which requires
/// `read:board-scope.admin:jira-software`) must get the same disambiguated
/// rewrite. `--board <ID>` is used to bypass board auto-discovery so the
/// mocked 401 is deterministically hit on the configuration call.
///
/// Note: the story text labels this command-family case "`jr board view
/// --config`", but `BoardCommand::View` has no `--config` flag in the
/// current CLI surface (`src/cli/mod.rs`) — `jr board view` unconditionally
/// calls `get_board_config` as part of resolving board type, so that plain
/// invocation is what this test exercises. Flagged as a clarification for
/// the implementer/orchestrator, not treated as a blocking gap.
///
/// RED GATE: fails until the rewrite is wired into `jr board view`'s
/// config-fetch call site.
#[tokio::test]
async fn test_bc_x_15_001_board_view_401_scope_mismatch_names_admin_scope() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/99/configuration"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["board", "view", "--board", "99"])
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
    // F-WAVE-4: `get_board_config` requires BOTH
    // `read:board-scope.admin:jira-software` AND `read:project:jira`
    // (per oauth-scope-matrix.md #53) — the hint must name both, not just
    // the admin scope. RED until board.rs::handle_view's hint is widened.
    assert!(
        stderr.contains("read:board-scope.admin:jira-software and read:project:jira"),
        "Expected combined 'read:board-scope.admin:jira-software and read:project:jira' \
         scope hint in stderr (F-WAVE-4), got: {stderr}"
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

/// AC-005 (BC-X.15.001 EC-X.15.001-1): a 401 body containing BOTH the
/// scope-mismatch substring AND expired-token-shaped text must still
/// dispatch to the scope-mismatch rewrite (same precedence BC-X.3.005/
/// BC-1.6.044 already establish at the generic-detection level — there is
/// only one substring check, so scope-mismatch always wins when present).
///
/// RED GATE: fails today for the same reason as the AC-001 tests above — no
/// call-site rewrite exists yet, so the generic `InsufficientScope` template
/// surfaces instead of the granular hint.
#[tokio::test]
async fn test_bc_x_15_001_board_401_composite_body_scope_mismatch_wins() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": [
                "Unauthorized; scope does not match. The access token provided is \
                 expired, revoked, malformed, or invalid for other reasons."
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["board", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "Composite scope-mismatch body should still exit 2, got: {:?}; stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("read:board-scope:jira-software"),
        "Scope-mismatch substring must win over the expired-token-shaped text \
         in a composite body, got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "Expected 'jr auth login' re-consent guidance in stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

/// AC-004 (BC-X.15.001 Behavior clause 4): under Basic (API-token) auth,
/// `is_oauth_auth() == false` must short-circuit the new rewrite entirely —
/// the SAME "scope does not match" 401 body used in the AC-001 test above
/// must continue to surface via the pre-existing generic `InsufficientScope`
/// path, unchanged.
///
/// BOUNDARY GUARD, not RED: since no call-site rewrite exists yet at all,
/// this assertion is already true today (Basic auth was never in scope for
/// the rewrite). It documents and pins the invariant the implementer's
/// change must not violate, and must keep passing after the change lands.
#[tokio::test]
async fn test_bc_x_15_001_board_401_under_api_token_unaffected() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Unauthorized; scope does not match"]
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
        !stderr.contains("read:board-scope:jira-software"),
        "The new OAuth-only granular-scope hint must NOT fire under Basic \
         auth, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

/// AC-006 (BC-X.15.001 EC-X.15.001-3): a 401 body with neither the
/// scope-mismatch nor a recognizable expired-token substring (an
/// unrecognized/malformed body) must fall through unchanged to the
/// pre-existing generic 401 handling (`NotAuthenticated`).
///
/// Uses Basic auth deliberately: under Bearer/OAuth auth, an
/// unrecognized-body 401 enters the real auto-refresh coordinator
/// (`src/api/refresh_coordinator.rs`), which reads the OS keychain and can
/// prompt for keychain access on macOS for a novel service name — the same
/// risk class the project's own `JR_RUN_KEYRING_TESTS=1`-gated tests avoid
/// (see `tests/oauth_refresh_integration.rs`). Basic auth reaches the same
/// "no rewrite fires" outcome deterministically and without touching the
/// keychain, since the rewrite this story adds is gated on
/// `is_oauth_auth()` regardless of which non-scope-mismatch 401 shape is
/// observed.
///
/// BOUNDARY GUARD, not RED: passes today (no rewrite exists yet) and must
/// keep passing after the implementer's change lands.
#[tokio::test]
async fn test_bc_x_15_001_board_401_unrecognized_body_falls_through_unchanged() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Some unexpected authorization failure"]
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
    assert_eq!(
        output.status.code(),
        Some(2),
        "Unrecognized 401 body should exit 2, got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Not authenticated"),
        "Expected the generic 'Not authenticated' path to fire, got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "Expected 'jr auth login' suggestion in stderr, got: {stderr}"
    );
    assert!(
        !stderr.contains("Insufficient token scope"),
        "Must NOT dispatch to InsufficientScope for an unrecognized body, got: {stderr}"
    );
    assert!(
        !stderr.contains("read:board-scope:jira-software"),
        "The new granular-scope hint must NOT fire for an unrecognized body, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

/// AC-002 (BC-X.15.001 Behavior clause 2): a 401 without the scope-mismatch
/// substring, under OAuth auth, must be left UNCHANGED to fall through to
/// the existing auto-refresh coordinator — this story's rewrite must not
/// intercept it.
///
/// Gated behind `JR_RUN_KEYRING_TESTS=1` + `#[ignore]`, mirroring
/// `tests/oauth_refresh_integration.rs`'s existing convention for any test
/// that drives the real OAuth auto-refresh path: `send_inner`'s blanket-401
/// auto-refresh calls `refresh_oauth_token_with_url`, which reads the OS
/// keychain via `load_oauth_tokens`/`resolve_refresh_app_credentials` even
/// under an isolated `JR_SERVICE_NAME` — on macOS this can prompt for
/// keychain access on a novel service name, which would hang an
/// unattended run. NOT executed as part of this Red Gate's default
/// `cargo test` pass (consistent with the existing `#[ignore]`-gated
/// keyring suite); the weaker assertion below (rewrite does not fire) is
/// exactly what AC-002's own text sanctions when full refresh-flow mocking
/// is out of scope for a unit-level test.
///
/// KEYRING GATE: `JR_RUN_KEYRING_TESTS=1 JR_SERVICE_NAME=<unique> cargo test \
/// --test board_commands -- --ignored` to run for real.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_bc_x_15_001_board_401_without_scope_substring_falls_through_to_refresh() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }

    // SAFETY: process-global env var mutation, matching the documented
    // pattern in tests/oauth_refresh_integration.rs. This test is
    // `#[ignore]`d and requires an explicit opt-in env var, so it never
    // races other tests in a default `cargo test` run.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("JR_SERVICE_NAME", "jr-s4-agile-401-test");
    }

    let server = MockServer::start().await;

    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(
            "JR_OAUTH_TOKEN_URL",
            format!("{}/oauth/token/bc-x-15-001", server.uri()),
        );
    }

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": [
                "The access token provided is expired, revoked, malformed, \
                 or invalid for other reasons."
            ]
        })))
        .mount(&server)
        .await;

    // Refresh fails fast (invalid_grant) — no stored refresh token for this
    // isolated service name, so no successful retry is expected; this test
    // only cares that the NEW rewrite does not intercept the outcome.
    Mock::given(method("POST"))
        .and(path("/oauth/token/bc-x-15-001"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant"
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["board", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure (no valid refresh token available), got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !stderr.contains("read:board-scope:jira-software"),
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
// BC-X.15.001 v1.1 F1 scope expansion (2026-09-17) — AC-009, AC-011: internal
// board/sprint resolution call sites reachable from `jr board view`, not just
// its top-level command handler. See the story spec's v1.1 update
// (S-cycle8-agile-scope-mismatch-error-mapping) and BC-X.15.001's widened
// Behavior clause 1 call-site → hint mapping.
//
// RED GATE STATUS:
// - `test_bc_x_15_001_resolve_board_id_401_scope_mismatch_rewrite` (AC-009a),
//   `test_bc_x_15_001_board_view_scrum_list_sprints_401_scope_mismatch_rewrite`
//   (AC-011a), and
//   `test_bc_x_15_001_board_view_scrum_get_sprint_issues_401_scope_mismatch_rewrite`
//   (AC-011b) assert PRESENCE of the shared `rewrite_agile_scope_error` hint
//   at call sites that are, as of this writing, genuinely UNWRAPPED
//   (`resolve_board_id`'s `list_boards` call, and `handle_view`'s scrum-branch
//   `list_sprints`/`get_sprint_issues` calls both use a bare `.await?` with no
//   `.map_err(...)` rewrite) — these are genuine RED against current code.
// - AC-010 (`handle_view`'s unconditional `get_board_config` call) is ALREADY
//   wrapped in current code and is already covered by the pre-existing
//   `test_bc_x_15_001_board_view_401_scope_mismatch_names_admin_scope` test
//   above (which uses `--board 99` to bypass `resolve_board_id` and hit
//   `get_board_config` deterministically) — no new test added for AC-010 to
//   avoid duplicating that coverage.
// ---------------------------------------------------------------------------

/// AC-009(a) (BC-X.15.001 Behavior clause 1, widened): `board.rs::resolve_board_id`'s
/// auto-discovery `list_boards` call, reached via `jr board view` with no
/// `--board` override and no configured `board_id` (forcing the
/// `--project`-driven auto-discovery branch), must get the SAME hint as
/// `jr board list` — `read:board-scope:jira-software` + `read:project:jira`
/// — on a scope-mismatch 401.
///
/// RED GATE: `resolve_board_id`'s `list_boards` call is a bare `.await?`
/// today with no `rewrite_agile_scope_error` wrapping — fails until wired.
#[tokio::test]
async fn test_bc_x_15_001_resolve_board_id_401_scope_mismatch_rewrite() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["--project", "PROJ", "board", "view"])
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

/// AC-011(a) (BC-X.15.001 Behavior clause 1, widened): `handle_view`'s scrum
/// branch `list_sprints` call must get the SAME grouped hint `jr sprint
/// list`/`jr sprint current` use — `read:sprint:jira-software` +
/// `read:issue-details:jira` + `read:jql:jira` (EC-X.15.001-4) — on a
/// scope-mismatch 401. `--board 42` bypasses `resolve_board_id` so the board
/// config call succeeds deterministically (scrum) and the mocked 401 lands
/// on `list_sprints`.
///
/// RED GATE: `handle_view`'s `list_sprints` call is a bare `.await?` today —
/// fails until wired.
#[tokio::test]
async fn test_bc_x_15_001_board_view_scrum_list_sprints_401_scope_mismatch_rewrite() {
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
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["board", "view", "--board", "42"])
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
        "Scope-mismatch 401 on handle_view's list_sprints call should exit 2, \
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

/// AC-011(b) (BC-X.15.001 Behavior clause 1, widened): `handle_view`'s scrum
/// branch `get_sprint_issues` call — distinct from AC-011(a)'s `list_sprints`
/// call — must get the SAME grouped sprint-scope hint on a scope-mismatch
/// 401. `list_sprints` succeeds here (returns one active sprint) so the
/// mocked 401 is deterministically hit on `get_sprint_issues` instead.
///
/// RED GATE: `handle_view`'s `get_sprint_issues` call is a bare `.await?`
/// today — fails until wired.
#[tokio::test]
async fn test_bc_x_15_001_board_view_scrum_get_sprint_issues_401_scope_mismatch_rewrite() {
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
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::sprint_list_response(vec![common::fixtures::sprint(
                100, "Sprint 1", "active",
            )]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Bearer test-oauth-access-token")
        .args(["board", "view", "--board", "42"])
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
        "Scope-mismatch 401 on handle_view's get_sprint_issues call should exit 2, \
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
