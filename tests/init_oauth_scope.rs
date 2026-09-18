//! F-WG-1 scope expansion — `jr init`'s project-setup `list_boards` call
//! (S-cycle8-agile-scope-mismatch-error-mapping widened to internal call
//! sites that make the same Agile HTTP calls as `jr board`/`jr sprint` but
//! were left unwrapped by the original story).
//!
//! Call site: `src/cli/init.rs` ~L180, `client.list_boards(None, None).await?`
//! inside the "Configure this directory as a Jira project?" branch. Same
//! endpoint, same required scopes, as `board.rs::handle_list`'s
//! `list_boards` call (`read:board-scope:jira-software` +
//! `read:project:jira`) — currently a bare `.await?` with zero error
//! mapping, so an OAuth "scope does not match" 401 here surfaces the
//! generic, POST-framed `InsufficientScope` template (issue #185) instead
//! of the granular hint.
//!
//! ## Why this is the hardest of the three F-WG-1 sites to reach
//!
//! Unlike `jr issue list` / `jr board` / `jr sprint`, `jr init` is
//! unconditionally interactive (`src/cli/init.rs` doc comment: "Flags
//! aren't plumbed through init") and has no `--no-input` / debug-seam path
//! that reaches the `list_boards` call directly. Getting a subprocess to
//! `list_boards` requires driving THREE separate prompts in sequence
//! (instance-URL `Input`, auth-method `Select`, "configure this
//! directory?" `Confirm`) plus a full OAuth 2.0 authorization-code exchange
//! (`login_oauth` → `api::auth::oauth_login`) — which itself binds a real
//! loopback TCP listener and writes to the OS keychain.
//!
//! This test reaches it anyway by combining two already-established,
//! independently-proven seams in this test suite:
//!   1. `tests/multi_cloudid_disambiguation.rs`'s pattern for driving a full
//!      `--oauth` login non-interactively: `JR_OAUTH_CODE` (skip
//!      browser-open + TCP accept), `JR_OAUTH_TOKEN_URL` /
//!      `JR_ACCESSIBLE_RESOURCES_URL` (redirect the token exchange +
//!      accessible-resources lookup to wiremock), `JR_OAUTH_CLIENT_ID` /
//!      `JR_OAUTH_CLIENT_SECRET` (bypass the BYO-app credential prompt),
//!      and a unique per-test `JR_SERVICE_NAME` (keychain isolation) inside
//!      fully-isolated `JR_CONFIG_DIR`/`JR_CACHE_DIR` temp directories.
//!   2. `write_stdin` to answer the three plain-line prompts in order (the
//!      same technique `multi_cloudid_disambiguation.rs::
//!      test_interactive_select_via_stdin_picks_second_resource` uses to
//!      drive a `dialoguer::Select` via piped, non-TTY stdin).
//!
//! `JR_BASE_URL` (gated identically to every other call site in this repo)
//! then redirects the post-login `list_boards` call itself to the same
//! wiremock server, regardless of the URL string typed at the interactive
//! prompt.
//!
//! ## RED-gate status
//!
//! Like `tests/multi_cloudid_disambiguation.rs`'s equivalent
//! `#[ignore]`d keyring tests, this test is NOT executed as part of this
//! Red Gate's default `cargo test` pass, and was not run to capture a live
//! failure transcript — running it touches the real OS keychain (via
//! `store_oauth_app_credentials`/`store_oauth_tokens`), which on macOS can
//! block on a one-time access-grant dialog outside an interactive terminal
//! session (the exact reason `JR_RUN_KEYRING_TESTS` gating exists per
//! CLAUDE.md's "AI Agent Notes" / existing keyring-gated test convention).
//! It is reasoned RED the same way its sibling is: `src/cli/init.rs`'s
//! `list_boards` call (~L180) is a bare `.await?` today with no
//! `rewrite_agile_scope_error` wiring at all, so the mocked "scope does not
//! match" 401 below can only surface via the generic, POST-framed
//! `InsufficientScope` template (issue #185) — the assertions for the
//! granular hint substrings will fail until the implementer wires the
//! rewrite into this call site.
//!
//! Run explicitly once wired: `JR_RUN_KEYRING_TESTS=1 cargo test --test
//! init_oauth_scope -- --ignored`.
//!
//! ## Seam note for the implementer
//!
//! If this test proves too fragile/slow in CI (real TCP bind + real
//! keychain I/O), an acceptable alternative — consistent with this
//! codebase's existing pattern of extracting a pure/testable helper out of
//! `workflow.rs`'s interactive prompts (`resolve_interactive_choice`,
//! `refuse_noninteractive`) rather than driving the full dialoguer flow —
//! is to apply `rewrite_agile_scope_error` at the `list_boards` call site
//! mechanically (identical to every other site in `board.rs`/`sprint.rs`)
//! without a dedicated new regression test, on the reasoning that the fix
//! is a one-line, already-15×-repeated pattern and this file still exists
//! as documentation + a best-effort regression test once
//! `JR_RUN_KEYRING_TESTS=1` is exercised (e.g. in a manual smoke pass or a
//! keyring-capable CI runner).

use assert_cmd::Command;
use tempfile::TempDir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Unique per-invocation keychain service name so parallel/repeated runs of
/// this test don't collide when writing OAuth app credentials + tokens to
/// the system keychain. Mirrors
/// `multi_cloudid_disambiguation.rs::unique_test_service_name`.
fn unique_test_service_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!(
        "jr-test-init-scope-{pid}-{nanos}",
        pid = std::process::id(),
    )
}

/// Build a `jr init` command with full XDG/keychain isolation. Mirrors
/// `multi_cloudid_disambiguation.rs::jr_isolated`.
fn jr_init_isolated(config_dir: &TempDir, cache_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("JR_SERVICE_NAME", unique_test_service_name())
        .env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        .env_remove("JR_AUTH_HEADER");
    cmd
}

/// F-WG-1 site 3: `jr init`'s "Configure this directory as a Jira project?"
/// branch calls `client.list_boards(None, None)` (~L180). Under OAuth
/// (Bearer) auth with a "scope does not match" 401, this must surface the
/// SAME granular hint `board.rs::handle_list`'s `list_boards` call already
/// gets (`read:board-scope:jira-software and read:project:jira`), not the
/// generic `InsufficientScope` template.
///
/// See the module doc comment above for why this is `#[ignore]`d and not
/// run by default, and for the RED-gate reasoning.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_init_list_boards_401_scope_mismatch_names_missing_scopes() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }

    let server = MockServer::start().await;

    // 1. Token exchange (JR_OAUTH_CODE skips the real browser/TCP-accept
    //    step, but the code-for-token POST still happens).
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "test-init-access-token",
            "refresh_token": "test-init-refresh-token",
            "token_type": "Bearer",
            "expires_in": 3600,
        })))
        .mount(&server)
        .await;

    // 2. Accessible-resources — a single site, so `resolve_cloud_id`
    //    auto-selects with no further interactive prompt.
    Mock::given(method("GET"))
        .and(path("/oauth/token/accessible-resources"))
        .and(header("Authorization", "Bearer test-init-access-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{
            "id": "cloud-init-scope-test",
            "name": "Init Scope Test Co",
            "url": "https://init-scope-test.atlassian.net",
        }])))
        .mount(&server)
        .await;

    // 3. `list_boards(None, None)` — the call site under test.
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Unauthorized; scope does not match"]
        })))
        .mount(&server)
        .await;

    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    let project_dir = TempDir::new().unwrap();

    // Prompt sequence for `jr init`, in order:
    //   1. "Jira instance URL" (Input) — value is irrelevant to the HTTP
    //      calls below since JR_BASE_URL overrides the actual request host,
    //      but must be non-empty.
    //   2. "Authentication method" (Select, items = ["OAuth 2.0
    //      (recommended)", "API Token"], default index 0) — "1\n" picks
    //      index 0 (OAuth), mirroring the 1-based numeric-jump convention
    //      `multi_cloudid_disambiguation.rs` already proved works over
    //      piped stdin for `dialoguer::Select`.
    //   3. "Configure this directory as a Jira project?" (Confirm,
    //      default true) — "y\n" confirms, reaching the `list_boards` call.
    let stdin = "https://init-scope-test.atlassian.net\n1\ny\n";

    let output = jr_init_isolated(&config_dir, &cache_dir)
        .current_dir(project_dir.path())
        .args(["init"])
        .env("JR_BASE_URL", server.uri())
        .env("JR_OAUTH_CODE", "test-init-scope-auth-code")
        .env("JR_OAUTH_TOKEN_URL", format!("{}/oauth/token", server.uri()))
        .env(
            "JR_ACCESSIBLE_RESOURCES_URL",
            format!("{}/oauth/token/accessible-resources", server.uri()),
        )
        .env("JR_OAUTH_CLIENT_ID", "test-init-client-id")
        .env("JR_OAUTH_CLIENT_SECRET", "test-init-client-secret")
        .write_stdin(stdin)
        .timeout(std::time::Duration::from_secs(15))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "Expected jr init to fail on the mocked list_boards 401, \
         got success. stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "Scope-mismatch 401 on init's list_boards call should exit 2, \
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
        "Must NOT surface the generic POST-framed InsufficientScope template \
         (issue #185) for this OAuth Agile-scope-mismatch case, got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
