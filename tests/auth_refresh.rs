#[allow(dead_code)]
mod common;

use assert_cmd::Command;

#[test]
fn auth_refresh_help_mentions_refresh_and_oauth() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "--help should exit 0");
    assert!(
        stdout.to_lowercase().contains("refresh"),
        "help text should mention 'refresh': {stdout}"
    );
    assert!(
        stdout.contains("--oauth"),
        "help text should list --oauth flag: {stdout}"
    );
}

#[test]
fn auth_refresh_oauth_help_is_accepted() {
    // clap should accept `--oauth --help` as well as `--help --oauth`.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--oauth", "--help"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "--oauth --help should exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn auth_refresh_no_input_fails_with_clear_message() {
    // Pin: `jr auth refresh --no-input` without any credential flags must
    // fail with a UserError (exit 64) that names the missing flag and env
    // var. Enabled by #211 — login flows now resolve credentials via
    // flag → env → prompt and error explicitly under --no-input.
    //
    // Replaces the pre-#211 test that asserted "fails without panic" when
    // stdin was closed; the new contract is stronger (specific exit code +
    // actionable message) so scripts/agents can recover.
    //
    // `JR_SERVICE_NAME` scopes the keychain service so `auth::clear_credentials()`
    // inside the subprocess never touches the developer's real `jr-jira-cli`
    // entries when `cargo test` runs locally.
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        // Config::load() merges JR_* via figment's Env::prefixed at
        // src/config.rs:65 — JR_INSTANCE_AUTH_METHOD=oauth in the parent
        // shell would flip refresh to the OAuth path and our email/JR_EMAIL
        // stderr assertions would fail. Explicitly clear it to pin the
        // api_token flow for this test.
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .args(["--no-input", "auth", "refresh"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "auth refresh --no-input without flags should fail, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Missing credentials under --no-input should exit 64 (UserError), got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("--email") && stderr.contains("$JR_EMAIL"),
        "Error should cite --email flag and $JR_EMAIL env var: {stderr}"
    );
    // The clear-then-login ordering means credentials *are* cleared before
    // the login failure bubbles up. The recovery hint tells users exactly
    // how to get back to a working state — pinning it here so a future
    // refactor can't silently drop the guidance.
    assert!(
        stderr.contains("Credentials were cleared"),
        "Error should include recovery hint after cleared credentials: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
