//! S-2.07 Red Gate — auth `--output json` integration tests (AC-001, AC-001b, AC-001c, AC-003, AC-004)
//!
//! These tests exercise the MISSING `--output json` branches in the four auth
//! handler functions (`handle_login`, `handle_switch`, `handle_logout`,
//! `handle_remove`). ALL tests in this file are expected to FAIL on develop
//! because the handlers do not yet check `--output json`. They will turn GREEN
//! once the implementer threads `OutputFormat` through the handler signatures
//! and adds the JSON-emission branches (S-2.07 implementation task).
//!
//! Holdout coverage:
//! - AC-001  / H-020 / BC-7.3.004: `jr auth switch default --output json` → exit 0 + JSON ok
//! - AC-001b / H-020 / BC-7.3.004: `jr auth logout --output json` → exit 0 + JSON ok
//! - AC-001c / H-020 / BC-7.3.004: `jr auth remove --output json` → exit 0 + JSON ok
//! - AC-003  / H-020 / BC-7.3.005: `jr auth switch ghost --output json` → exit 64 + JSON error
//! - AC-004  / H-020 / BC-7.3.004: `jr auth login --api-token --output json` → exit 0 + JSON ok
//!
//! Infrastructure:
//! - Process-spawn via `assert_cmd::Command`
//! - XDG isolation via `tempfile::TempDir`
//! - `JR_SERVICE_NAME` scoped to `jr-jira-cli-test` to avoid keychain pollution
//! - Wiremock for AC-004 (`/rest/api/2/myself` mock is NOT required — `login_token`
//!   stores credentials directly and does not call the Jira API; wiremock not needed)

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a minimal valid config to `<dir>/jr/config.toml` with a single
/// `[profiles.default]` entry. Returns the TempDir (must stay alive for the
/// test's duration) and the config file path.
fn write_single_profile_config(dir: &TempDir, profile_name: &str) -> std::path::PathBuf {
    let jr_dir = dir.path().join("jr");
    std::fs::create_dir_all(&jr_dir).unwrap();
    let config_path = jr_dir.join("config.toml");
    let toml = format!(
        "default_profile = \"{profile_name}\"\n\n[profiles.{profile_name}]\nurl = \"https://test.atlassian.net\"\nauth_method = \"api_token\"\n"
    );
    std::fs::write(&config_path, toml).unwrap();
    config_path
}

/// Write a two-profile config: `default` (active) + `staging` (removable).
fn write_two_profile_config(dir: &TempDir) -> std::path::PathBuf {
    let jr_dir = dir.path().join("jr");
    std::fs::create_dir_all(&jr_dir).unwrap();
    let config_path = jr_dir.join("config.toml");
    std::fs::write(
        &config_path,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://test.atlassian.net\"\n\
         auth_method = \"api_token\"\n\n\
         [profiles.staging]\n\
         url = \"https://staging.atlassian.net\"\n\
         auth_method = \"api_token\"\n",
    )
    .unwrap();
    config_path
}

/// Build a `jr` command with full XDG isolation and test-scoped keychain
/// service name. Clears all JR_* env vars that could leak from dev shells.
fn jr_isolated(config_dir: &TempDir, cache_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("XDG_CONFIG_HOME", config_dir.path())
        .env("XDG_CACHE_HOME", cache_dir.path())
        // Scope the keychain service to avoid touching developer's real entries
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        // Clear env vars that could override config or inject credentials
        .env_remove("JR_PROFILE")
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

// ---------------------------------------------------------------------------
// AC-001 / H-020 / BC-7.3.004 — auth switch returns JSON ok
// ---------------------------------------------------------------------------

/// BC-7.3.004 postcondition: `jr auth switch default --output json` when the
/// `default` profile exists must exit 0 and emit
/// `{"profile": "default", "action": "switch", "ok": true}` on stdout.
///
/// RED-GATE: FAILS on develop because `handle_switch` does NOT check
/// `--output json` — it always calls `output::print_success(...)`.
#[test]
fn test_auth_switch_returns_json_ok() {
    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    let cwd_dir = TempDir::new().unwrap();

    write_single_profile_config(&config_dir, "default");

    let output = jr_isolated(&config_dir, &cache_dir)
        .current_dir(cwd_dir.path())
        .args(["auth", "switch", "default", "--output", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "auth switch default --output json should exit 0; stderr: {stderr}, stdout: {stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must be valid JSON: {e}\nstdout: {stdout}"));

    assert_eq!(
        parsed,
        serde_json::json!({"profile": "default", "action": "switch", "ok": true}),
        "stdout JSON must match the verb-aligned auth shape; got: {parsed}"
    );
}

// ---------------------------------------------------------------------------
// AC-001b / H-020 / BC-7.3.004 — auth logout returns JSON ok
// ---------------------------------------------------------------------------

/// BC-7.3.004 postcondition: `jr auth logout --output json --profile default`
/// when the `default` profile exists must exit 0 and emit
/// `{"profile": "default", "action": "logout", "ok": true}` on stdout.
///
/// RED-GATE: FAILS on develop because `handle_logout` does NOT check
/// `--output json`.
#[test]
fn test_auth_logout_returns_json_ok() {
    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    let cwd_dir = TempDir::new().unwrap();

    write_single_profile_config(&config_dir, "default");

    let output = jr_isolated(&config_dir, &cache_dir)
        .current_dir(cwd_dir.path())
        .args(["auth", "logout", "--profile", "default", "--output", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "auth logout --output json should exit 0; stderr: {stderr}, stdout: {stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must be valid JSON: {e}\nstdout: {stdout}"));

    assert_eq!(
        parsed,
        serde_json::json!({"profile": "default", "action": "logout", "ok": true}),
        "stdout JSON must match the verb-aligned auth shape; got: {parsed}"
    );
}

// ---------------------------------------------------------------------------
// AC-001c / H-020 / BC-7.3.004 — auth remove returns JSON ok
// ---------------------------------------------------------------------------

/// BC-7.3.004 postcondition: `jr auth remove staging --output json --no-input`
/// when `staging` is a non-active profile must exit 0 and emit
/// `{"profile": "staging", "action": "remove", "ok": true}` on stdout.
///
/// Uses a two-profile config (`default` active + `staging` removable) so the
/// active-profile guard does not fire.
///
/// RED-GATE: FAILS on develop because `handle_remove` does NOT check
/// `--output json`.
#[test]
fn test_auth_remove_returns_json_ok() {
    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    let cwd_dir = TempDir::new().unwrap();

    write_two_profile_config(&config_dir);

    let output = jr_isolated(&config_dir, &cache_dir)
        .current_dir(cwd_dir.path())
        .args([
            "auth",
            "remove",
            "staging",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "auth remove staging --output json should exit 0; stderr: {stderr}, stdout: {stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must be valid JSON: {e}\nstdout: {stdout}"));

    assert_eq!(
        parsed,
        serde_json::json!({"profile": "staging", "action": "remove", "ok": true}),
        "stdout JSON must match the verb-aligned auth shape; got: {parsed}"
    );
}

// ---------------------------------------------------------------------------
// AC-003 / H-020 / BC-7.3.005 — auth switch unknown profile returns JSON error
// ---------------------------------------------------------------------------

/// BC-7.3.005 postcondition: `jr auth switch ghost --output json` when the
/// `ghost` profile does not exist must exit 64 and emit JSON error to stderr
/// with keys `"error"` (non-empty string) and `"code": 64`.
///
/// The global main.rs error-JSON handler already emits `{"error":..,"code":..}`
/// to stderr for any propagated error when `--output json` is set. This test
/// verifies the auth subcommand participates in that path.
///
/// Note: on develop, `handle_switch` propagates `JrError::UserError` correctly
/// (exit 64 is confirmed by `auth_switch_unknown_profile_exits_64` in
/// `auth_profiles.rs`). The JSON WRAPPING of that error in stderr is handled
/// by main.rs and should already work. If this test happens to PASS on develop,
/// AC-003's holdout is satisfied by the existing main.rs error handler and no
/// additional implementation is needed.
///
/// RED-GATE expected: RED (stderr error is not yet JSON-wrapped before this story)
/// — but this may be GREEN if main.rs already wraps all errors. Investigate and
/// document the observed state in the commit message.
#[test]
fn test_auth_switch_unknown_profile_returns_json_error() {
    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    let cwd_dir = TempDir::new().unwrap();

    // Empty config — no profiles defined, so "ghost" cannot exist
    let jr_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&jr_dir).unwrap();

    let output = jr_isolated(&config_dir, &cache_dir)
        .current_dir(cwd_dir.path())
        .args(["auth", "switch", "ghost", "--output", "json"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "auth switch ghost --output json must exit 64; stdout: {stdout}, stderr: {stderr}"
    );

    // stderr must be parseable JSON
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|e| {
        panic!("stderr must be valid JSON when --output json set: {e}\nstderr: {stderr}")
    });

    assert!(
        parsed["error"].as_str().is_some_and(|s| !s.is_empty()),
        "JSON error must have non-empty 'error' field; got: {parsed}"
    );
    assert_eq!(
        parsed["code"].as_i64(),
        Some(64),
        "JSON error 'code' field must be 64; got: {parsed}"
    );
}

// ---------------------------------------------------------------------------
// AC-004 / H-020 / BC-7.3.004 — auth login emits JSON when --output json set
// ---------------------------------------------------------------------------

/// BC-7.3.004 postcondition: `jr auth login --api-token --profile testprof
/// --url https://test.atlassian.net --email test@example.com --token TOKEN
/// --no-input --output json` must exit 0 and emit
/// `{"profile": "testprof", "action": "login", "ok": true}` on stdout.
///
/// The `--api-token` flag selects the API-token flow. `login_token` stores
/// credentials into the keychain (scoped to `JR_SERVICE_NAME=jr-jira-cli-test`)
/// and does NOT call any Jira API endpoint — no wiremock needed.
///
/// This test writes credentials to the test-scoped keychain entry. If the
/// test environment has no keychain (Linux CI without secret-service), the
/// keychain write will fail BEFORE the JSON output branch is reached, so the
/// test may fail for a different reason (keychain unavailable rather than
/// missing JSON output). In that case the test is still RED for the right
/// class of reason. Production CI runs on macOS where the keychain is available.
///
/// RED-GATE: FAILS on develop because `handle_login` does NOT check
/// `--output json`.
#[test]
fn test_auth_login_emits_json_when_output_json_set() {
    let config_dir = TempDir::new().unwrap();
    let cache_dir = TempDir::new().unwrap();
    let cwd_dir = TempDir::new().unwrap();

    // No pre-existing config needed; `handle_login` creates the profile
    // when `--profile testprof --url ...` is provided
    let jr_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&jr_dir).unwrap();

    let output = jr_isolated(&config_dir, &cache_dir)
        .current_dir(cwd_dir.path())
        .args([
            "auth",
            "login",
            "--profile",
            "testprof",
            "--url",
            "https://test.atlassian.net",
            "--email",
            "test@example.com",
            "--token",
            "TEST-TOKEN",
            "--no-input",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "auth login --output json should exit 0; stderr: {stderr}, stdout: {stdout}"
    );

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must be valid JSON: {e}\nstdout: {stdout}"));

    assert_eq!(
        parsed,
        serde_json::json!({"profile": "testprof", "action": "login", "ok": true}),
        "stdout JSON must match the verb-aligned auth shape; got: {parsed}"
    );
}
