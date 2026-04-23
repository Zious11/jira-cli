//! Pin that `jr auth login --oauth` surfaces malformed-config errors
//! instead of silently overwriting the broken file with defaults (#258).
//!
//! Before the fix: `src/cli/auth.rs` used `Config::load().unwrap_or_default()`,
//! which swallowed TOML parse errors, permission errors, etc. The subsequent
//! `save_global()` then wrote a `Config::default()` payload over the broken
//! file, silently discarding the user's settings (story-points field id,
//! team field id, etc.).
//!
//! After the fix: `Config::load()?` propagates, wrapped in
//! `JrError::ConfigError` for exit code 78 + an actionable message.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;

#[test]
fn auth_login_oauth_surfaces_malformed_config_without_overwriting() {
    // Arrange: write a malformed TOML file to the config dir and capture
    // its contents so we can assert it's unchanged after the command runs.
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    // Pristine working directory so Config::load()'s upward walk for a
    // per-project `.jr.toml` cannot pick up an unrelated file on the
    // developer's machine and turn the test's intended global-config
    // failure into a project-config failure (or a silent pass).
    let cwd_dir = tempfile::tempdir().unwrap();
    let jr_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&jr_dir).unwrap();
    let config_path = jr_dir.join("config.toml");
    // Intentionally malformed: unclosed table header.
    let malformed = "[unclosed\nbad = \n";
    std::fs::write(&config_path, malformed).unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        // Defense-in-depth env hygiene. Figment's extract() is fatal on a
        // TOML parse error, so the malformed global config above will fail
        // load regardless of JR_INSTANCE_* values in the parent shell. We
        // still clear them so this test stays correct if the failure case
        // is later broadened to valid-but-schema-wrong configs, where env
        // vars WOULD contribute to the merged Config.
        .env_remove("JR_INSTANCE_URL")
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .env_remove("JR_INSTANCE_OAUTH_SCOPES")
        .env_remove("JR_INSTANCE_CLOUD_ID")
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Exit non-zero with ConfigError exit code (78) so scripts can branch.
    assert!(
        !output.status.success(),
        "malformed config must fail the command, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(78),
        "malformed config should exit 78 (ConfigError), got: {:?}, stderr: {stderr}",
        output.status.code()
    );

    // Error message must surface the underlying parse failure so users can
    // find and fix the broken TOML.
    assert!(
        stderr.to_lowercase().contains("toml") || stderr.to_lowercase().contains("parse"),
        "stderr should surface the TOML parse error: {stderr}"
    );

    // The malformed file on disk must be unchanged — the bug was that a
    // successful `save_global()` overwrote it with defaults, silently
    // deleting the user's other config.
    let after = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(
        after, malformed,
        "config file must not be overwritten when load fails"
    );

    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
