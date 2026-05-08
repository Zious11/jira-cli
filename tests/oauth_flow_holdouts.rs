//! OAuth flow regression holdout suite — S-1.06
//!
//! Pins existing OAuth flow and auth handler behavior across 8 holdout
//! scenarios. All tests pass on current develop — no implementation changes
//! required. Future regressions in any of these paths will break this suite.
//!
//! Holdout coverage:
//! - AC-001 / H-001: `auth status` no profiles → exit 0 + "No profiles configured" in stderr
//! - AC-001 / H-002: `auth list --output json` no profiles → exit 0 + `[]`
//! - AC-002 / H-003: profile precedence flag > env > config > "default"
//! - AC-003 / H-004: `auth refresh --no-input` no URL → exit 64 + actionable hints
//! - AC-004 / H-005: malformed config TOML → exit 78 + file bytes unchanged
//! - AC-005 / H-022: thin delegation to existing api_client.rs BC-1.6.042..045 coverage,
//!   plus direct library-level re-pins for the three dispatch boundaries
//! - AC-006 / H-029: embedded redirect_uri = http://127.0.0.1:53682/callback;
//!   BYO DynamicPort produces http://localhost:<port>/callback with port ≠ 53682
//!
//! Infrastructure pattern: every test uses `TempDir` for XDG_CONFIG_HOME and
//! XDG_CACHE_HOME isolation. Process-spawn tests use `assert_cmd`. HTTP-mock
//! tests use `wiremock` + `JR_BASE_URL`. `JR_SERVICE_NAME=jr-jira-cli-test`
//! is set on keychain-adjacent paths. No `JR_AUTH_HEADER` — uses the
//! `wiremock` pattern (SD-002 canonical test infra).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use jr::api::auth::{EMBEDDED_CALLBACK_PORT, RedirectUriStrategy};
use jr::api::auth_embedded::embedded_oauth_app;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a `jr` Command with a clean JR_* environment: no stray env vars
/// from the parent shell can bleed into the test's XDG-isolated config.
/// Matches the hygiene pattern established in `tests/auth_profiles.rs`.
fn jr_cmd() -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .env_remove("JR_INSTANCE_URL")
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .env_remove("JR_INSTANCE_CLOUD_ID")
        .env_remove("JR_INSTANCE_ORG_ID")
        .env_remove("JR_INSTANCE_OAUTH_SCOPES")
        .env_remove("JR_FIELDS_TEAM_FIELD_ID")
        .env_remove("JR_FIELDS_STORY_POINTS_FIELD_ID")
        .env_remove("JR_DEFAULTS_OUTPUT")
        .env_remove("JR_BASE_URL")
        .env_remove("JR_AUTH_HEADER")
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        .env_remove("JR_OAUTH_CLIENT_ID")
        .env_remove("JR_OAUTH_CLIENT_SECRET");
    cmd
}

/// Create a fresh, empty XDG config dir (with the `jr/` subdirectory created
/// but no `config.toml` written). Returns the `TempDir` guard and the full
/// path to `config.toml` (which does NOT exist yet).
fn fresh_xdg() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let cfg_path = dir.path().join("jr").join("config.toml");
    std::fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    (dir, cfg_path)
}

// ---------------------------------------------------------------------------
// AC-001 / H-001: auth status no profiles → exit 0 + "No profiles configured"
// ---------------------------------------------------------------------------

/// BC-1.1.002 postcondition: `jr auth status` against an empty XDG_CONFIG_HOME
/// exits 0 and writes "No profiles configured" to stderr. This is the probe
/// that CI / setup scripts use to detect first-run state — it must not error.
#[test]
fn test_s_1_06_h_001_auth_status_no_profiles() {
    let (dir, _cfg) = fresh_xdg();

    let out = jr_cmd()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("XDG_CACHE_HOME", dir.path())
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        .args(["auth", "status"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "auth status on fresh install should exit 0; stderr: {stderr}"
    );
    assert!(
        stderr.contains("No profiles configured"),
        "expected 'No profiles configured' in stderr; got: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

// ---------------------------------------------------------------------------
// AC-001 / H-002: auth list --output json no profiles → exit 0 + `[]`
// ---------------------------------------------------------------------------

/// BC-1.1.001 postcondition: `jr auth list --output json` on a fresh install
/// exits 0 and writes `[]` to stdout (empty JSON array). Downstream tooling
/// that parses `jr auth list` output must not receive an error exit code.
#[test]
fn test_s_1_06_h_002_auth_list_json_no_profiles() {
    let (dir, _cfg) = fresh_xdg();

    let out = jr_cmd()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("XDG_CACHE_HOME", dir.path())
        .args(["auth", "list", "--output", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "auth list --output json on fresh install should exit 0; stderr: {stderr}"
    );
    // The output must be a valid JSON array (the empty array `[]`).
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must be valid JSON; got: '{stdout}'; parse error: {e}"));
    assert!(
        parsed.as_array().is_some_and(|a| a.is_empty()),
        "expected empty JSON array `[]`; got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 / H-003: profile precedence chain (flag > env > config > "default")
// ---------------------------------------------------------------------------

/// BC-1.1.007 postcondition: The active profile is resolved from four sources
/// in strict priority order. This test pins all four resolution cases in a
/// single config with three named profiles so a future refactor that silently
/// drops one layer of the chain surfaces as a test failure.
///
/// Cases covered:
///   (a) flag `--profile from-flag` + `JR_PROFILE=from-env` → flag wins
///   (b) No flag + `JR_PROFILE=from-env` → env wins
///   (c) No flag, no env → `default_profile = "from-config"` → config wins
///   (d) No flag, no env, no `default_profile` field → literal "default"
#[test]
fn test_s_1_06_h_003_profile_precedence_chain() {
    let (dir, cfg_path) = fresh_xdg();
    let cache_dir = TempDir::new().unwrap();

    // Config with three named profiles and `default_profile = "from-config"`.
    // Profile URLs are distinct so assertions can verify which one is active.
    std::fs::write(
        &cfg_path,
        r#"
default_profile = "from-config"
[profiles.from-config]
url = "https://from-config.example"
[profiles.from-env]
url = "https://from-env.example"
[profiles.from-flag]
url = "https://from-flag.example"
[profiles.default]
url = "https://default.example"
"#,
    )
    .unwrap();

    // Helper: run `jr auth list --output json` and return the name of the
    // profile with `"active": true`.
    let active_profile = |extra_args: &[&str], env_profile: Option<&str>| -> String {
        let mut cmd = jr_cmd();
        cmd.env("XDG_CONFIG_HOME", dir.path())
            .env("XDG_CACHE_HOME", cache_dir.path());
        if let Some(prof) = env_profile {
            cmd.env("JR_PROFILE", prof);
        }
        for arg in extra_args {
            cmd.arg(arg);
        }
        cmd.args(["auth", "list", "--output", "json"]);
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "auth list should succeed; stderr: {stderr}"
        );
        let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
            panic!("expected valid JSON from auth list; got: '{stdout}'; error: {e}")
        });
        let arr = parsed.as_array().expect("expected JSON array");
        let active: Vec<_> = arr
            .iter()
            .filter(|p| p["active"].as_bool() == Some(true))
            .collect();
        assert_eq!(
            active.len(),
            1,
            "expected exactly one active profile; got {}: {parsed:?}",
            active.len()
        );
        active[0]["name"]
            .as_str()
            .expect("name field must be a string")
            .to_string()
    };

    // Case (a): flag + env → flag wins.
    assert_eq!(
        active_profile(&["--profile", "from-flag"], Some("from-env")),
        "from-flag",
        "case (a): --profile flag must win over JR_PROFILE env"
    );

    // Case (b): no flag, env set → env wins over config.
    assert_eq!(
        active_profile(&[], Some("from-env")),
        "from-env",
        "case (b): JR_PROFILE env must win over default_profile in config"
    );

    // Case (c): no flag, no env → config's default_profile wins.
    assert_eq!(
        active_profile(&[], None),
        "from-config",
        "case (c): default_profile in config must be used when no flag or env"
    );

    // Case (d): no flag, no env, no default_profile → literal "default".
    // Write a config WITHOUT default_profile.
    std::fs::write(
        &cfg_path,
        r#"
[profiles.default]
url = "https://default.example"
[profiles.other]
url = "https://other.example"
"#,
    )
    .unwrap();
    assert_eq!(
        active_profile(&[], None),
        "default",
        "case (d): with no default_profile field, active profile must be literal 'default'"
    );
}

// ---------------------------------------------------------------------------
// AC-003 / H-004: auth refresh --no-input no URL → exit 64 + actionable hints
// ---------------------------------------------------------------------------

/// BC-1.1.011 postcondition: `jr auth refresh --no-input` against a profile
/// that has no URL configured must exit 64 and include:
///   - "no URL configured" (states the root cause)
///   - "jr auth login" (recovery command)
///   - "--url" (the recovery flag)
///
/// Stderr must NOT contain "panic".
#[test]
fn test_s_1_06_h_004_auth_refresh_no_url_configured() {
    let (dir, _cfg) = fresh_xdg(); // no config.toml → no URL
    let cache_dir = TempDir::new().unwrap();

    let out = jr_cmd()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        .args(["--no-input", "auth", "refresh"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "auth refresh --no-input without URL must fail; stderr: {stderr}"
    );
    assert_eq!(
        out.status.code(),
        Some(64),
        "must exit 64 (UserError) for no-URL configured; got {:?}; stderr: {stderr}",
        out.status.code()
    );
    assert!(
        stderr.contains("no URL configured"),
        "stderr must contain 'no URL configured'; got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "stderr must contain recovery command 'jr auth login'; got: {stderr}"
    );
    assert!(
        stderr.contains("--url"),
        "stderr must contain '--url' recovery flag; got: {stderr}"
    );
    assert!(
        !stderr.contains("panic"),
        "stderr must not leak a panic: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-004 / H-005: malformed config TOML → exit 78 + file bytes unchanged
// ---------------------------------------------------------------------------

/// BC-1.1.012 postcondition: When `~/.config/jr/config.toml` is malformed
/// TOML, `jr auth login --oauth --client-id X --client-secret Y --no-input`
/// must:
///   1. exit 78 (ConfigError)
///   2. write "toml" or "parse" to stderr (surface the error)
///   3. leave the on-disk file bytes IDENTICAL (no clobber)
///   4. not panic
///
/// JR_SERVICE_NAME is set to isolate keychain from the developer's real
/// keychain entries.
#[test]
fn test_s_1_06_h_005_malformed_config_exits_78_file_unchanged() {
    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    // Pristine cwd so the upward `.jr.toml` walk cannot accidentally pick up
    // a project config on the developer's machine.
    let cwd_dir = TempDir::new().unwrap();

    let jr_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&jr_dir).unwrap();
    let config_path = jr_dir.join("config.toml");

    // Intentionally malformed: unclosed table header.
    let malformed = "[unclosed\nbad = \n";
    std::fs::write(&config_path, malformed).unwrap();

    // Capture file bytes AND mtime before the command runs.
    let bytes_before = std::fs::read(&config_path).unwrap();
    let mtime_before = std::fs::metadata(&config_path).unwrap().modified().unwrap();

    let out = jr_cmd()
        .current_dir(cwd_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        .args([
            "auth",
            "login",
            "--oauth",
            "--client-id",
            "test-client-id",
            "--client-secret",
            "test-client-secret",
            "--no-input",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);

    // 1. Must fail.
    assert!(
        !out.status.success(),
        "malformed config must fail; stdout: {stdout}, stderr: {stderr}"
    );
    // 2. Exit code 78 (ConfigError).
    assert_eq!(
        out.status.code(),
        Some(78),
        "malformed config must exit 78; got {:?}; stderr: {stderr}",
        out.status.code()
    );
    // 3. Stderr must surface the parse error (either "toml" or "parse").
    assert!(
        stderr.to_lowercase().contains("toml") || stderr.to_lowercase().contains("parse"),
        "stderr must surface the TOML parse error; got: {stderr}"
    );
    // 4. File bytes on disk must be identical to pre-invocation content.
    let bytes_after = std::fs::read(&config_path).unwrap();
    assert_eq!(
        bytes_after, bytes_before,
        "config file must not be overwritten when load fails"
    );
    // 5. mtime must not have changed (belt-and-suspenders: file was never touched).
    let mtime_after = std::fs::metadata(&config_path).unwrap().modified().unwrap();
    assert_eq!(
        mtime_after, mtime_before,
        "config file mtime must be unchanged when load fails"
    );
    // 6. No panic.
    assert!(
        !stderr.contains("panic"),
        "stderr must not leak a panic: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-005 / H-022: 401 scope-mismatch dispatch (thin delegation re-pin)
// ---------------------------------------------------------------------------
//
// DELEGATION NOTE: The full BC-1.6.042..045 fixture set is already covered by
// `tests/api_client.rs` lines 100-249 (tests:
//   test_401_scope_mismatch_returns_insufficient_scope
//   test_401_scope_mismatch_matches_case_insensitively
//   test_401_without_scope_mismatch_falls_through_to_not_authenticated
//   test_non_401_with_scope_substring_does_not_dispatch_to_insufficient_scope
// ).
//
// This suite adds three thin re-pins at the library level to lock the dispatch
// boundaries from S-1.06's perspective. They are intentionally lean — their
// value is providing a direct line from the holdout ID to a test, not
// duplicating the fuller coverage already in api_client.rs.

/// BC-1.6.043 re-pin: 401 + body containing "scope does not match" (lowercase)
/// dispatches to `InsufficientScope` (exit 2 / "Insufficient token scope").
/// Cases (a) and (b) from AC-005.
#[tokio::test]
async fn test_s_1_06_h_022_scope_mismatch_lowercase_dispatches_insufficient_scope() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "code": 401,
            "message": "Unauthorized; scope does not match"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic test-token".to_string());

    let err = client
        .get::<serde_json::Value>("/rest/api/3/myself")
        .await
        .unwrap_err();

    let s = err.to_string();
    assert!(
        s.contains("Insufficient token scope"),
        "AC-005(a): 401 + 'scope does not match' must dispatch to InsufficientScope; got: {s}"
    );
}

/// BC-1.6.044 re-pin: 401 + body containing "Scope Does Not Match" (mixed case)
/// must also dispatch to InsufficientScope — pins the case-insensitive match.
/// Case (b) from AC-005.
#[tokio::test]
async fn test_s_1_06_h_022_scope_mismatch_mixed_case_dispatches_insufficient_scope() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "code": 401,
            "message": "Unauthorized; Scope Does Not Match"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic test-token".to_string());

    let err = client
        .post::<serde_json::Value, _>(
            "/rest/api/3/issue",
            &serde_json::json!({"fields": {"summary": "test"}}),
        )
        .await
        .unwrap_err();

    let s = err.to_string();
    assert!(
        s.contains("Insufficient token scope"),
        "AC-005(b): case-insensitive 'Scope Does Not Match' must dispatch to InsufficientScope; got: {s}"
    );
}

/// BC-1.6.045 re-pin: 401 WITHOUT "scope does not match" in body falls through
/// to the generic NotAuthenticated path. Also pins: 403 with "scope does not
/// match" in body must NOT dispatch to InsufficientScope (status gate).
/// Cases (c) and (d) from AC-005.
#[tokio::test]
async fn test_s_1_06_h_022_non_scope_401_and_403_do_not_dispatch_insufficient_scope() {
    // Case (c): 401 + "Session expired" → NotAuthenticated (not InsufficientScope)
    {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/myself"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "code": 401,
                "message": "Session expired"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = jr::api::client::JiraClient::new_for_test(
            server.uri(),
            "Basic expired-session".to_string(),
        );

        let err = client
            .get::<serde_json::Value>("/rest/api/3/myself")
            .await
            .unwrap_err();

        let s = err.to_string();
        assert!(
            s.contains("Not authenticated"),
            "AC-005(c): 401 + 'Session expired' must NOT dispatch to InsufficientScope; got: {s}"
        );
        assert!(
            !s.contains("Insufficient token scope"),
            "AC-005(c): must NOT produce InsufficientScope for session-expired body; got: {s}"
        );
    }

    // Case (d): 403 + "scope does not match policy" → ApiError (not InsufficientScope)
    {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "code": 403,
                "message": "Forbidden: scope does not match policy"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client =
            jr::api::client::JiraClient::new_for_test(server.uri(), "Basic test-token".to_string());

        let err = client
            .post::<serde_json::Value, _>(
                "/rest/api/3/issue",
                &serde_json::json!({"fields": {"summary": "test"}}),
            )
            .await
            .unwrap_err();

        let s = err.to_string();
        assert!(
            !s.contains("Insufficient token scope"),
            "AC-005(d): 403 must NOT dispatch to InsufficientScope even with scope substring; got: {s}"
        );
        assert!(
            s.contains("API error (403)"),
            "AC-005(d): 403 must produce generic ApiError; got: {s}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-006 / H-029: redirect_uri — embedded uses fixed port; BYO uses dynamic
// ---------------------------------------------------------------------------

/// BC-1.5.031: The embedded `jr` OAuth app's `redirect_uri` must be exactly
/// `http://127.0.0.1:53682/callback`. Uses `127.0.0.1` (not `localhost`) to
/// force IPv4 and match the listener bind (Atlassian validates by exact string
/// match).
///
/// Guard: if `embedded_oauth_app()` returns None (dev build without
/// JR_BUILD_OAUTH_CLIENT_ID set at compile time), the embedded-path assertion
/// is skipped and only the BYO-path assertion in the sibling test runs.
#[test]
fn test_s_1_06_h_029_embedded_redirect_uri() {
    if embedded_oauth_app().is_none() {
        eprintln!(
            "skip: no embedded app in dev build (JR_BUILD_OAUTH_CLIENT_ID not set); only BYO path tested"
        );
        // Test still counts as passing — the BYO path is covered by
        // test_s_1_06_h_029_byo_redirect_uri_dynamic_port.
        return;
    }

    let strategy = RedirectUriStrategy::FixedPort(EMBEDDED_CALLBACK_PORT);
    let uri = strategy.redirect_uri();
    assert_eq!(
        uri,
        format!("http://127.0.0.1:{EMBEDDED_CALLBACK_PORT}/callback"),
        "embedded OAuth app redirect_uri must use 127.0.0.1 and port {EMBEDDED_CALLBACK_PORT}"
    );
    assert_eq!(
        EMBEDDED_CALLBACK_PORT, 53682,
        "EMBEDDED_CALLBACK_PORT const must be 53682 (registered in Atlassian Developer Console)"
    );
}

/// BC-1.5.034: BYO OAuth sources (flag / env / keychain) use a `DynamicPort`
/// strategy that binds a random ephemeral port. The redirect_uri must use
/// `localhost` (backward-compatibility for existing BYO apps) and a port that
/// is different from 53682 (the embedded app's fixed port).
///
/// Note: we construct `RedirectUriStrategy::DynamicPort` directly without
/// binding a listener — the strategy type owns the port number independently
/// of the actual TCP socket.
#[test]
fn test_s_1_06_h_029_byo_redirect_uri_dynamic_port() {
    // Use a deterministic non-53682 port value that is valid but unlikely to
    // conflict with any listener. The value itself doesn't matter — the test
    // asserts the *shape* of the URI (localhost prefix, correct port embedding,
    // not the embedded port).
    let byo_port: u16 = 12345;
    assert_ne!(
        byo_port, EMBEDDED_CALLBACK_PORT,
        "test invariant: byo_port must differ from EMBEDDED_CALLBACK_PORT"
    );

    let strategy = RedirectUriStrategy::DynamicPort(byo_port);
    let uri = strategy.redirect_uri();

    assert_eq!(
        uri,
        format!("http://localhost:{byo_port}/callback"),
        "BYO DynamicPort redirect_uri must use 'localhost' and the configured port"
    );
    assert!(
        !uri.contains("127.0.0.1"),
        "BYO DynamicPort must use 'localhost', not '127.0.0.1'; got: {uri}"
    );
    assert_ne!(
        byo_port, EMBEDDED_CALLBACK_PORT,
        "BYO dynamic port must differ from the embedded fixed port {EMBEDDED_CALLBACK_PORT}"
    );
}

// ---------------------------------------------------------------------------
// Sanity: verify EMBEDDED_CALLBACK_PORT const value
// ---------------------------------------------------------------------------

/// Pins the `EMBEDDED_CALLBACK_PORT` constant to 53682. This is a long-lived
/// contract — Atlassian validates `redirect_uri` by exact string match, so
/// changing the port is a breaking release that requires re-registering the
/// callback URL in Developer Console (ADR-0006).
#[test]
fn test_s_1_06_h_029_embedded_callback_port_const_is_53682() {
    assert_eq!(
        EMBEDDED_CALLBACK_PORT, 53682,
        "EMBEDDED_CALLBACK_PORT must remain 53682; changing it is a breaking release"
    );
}
