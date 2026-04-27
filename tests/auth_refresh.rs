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
    // Pin: `jr auth refresh --no-input` against an unconfigured profile
    // (empty config / no URL) must fail with a UserError (exit 64) that
    // tells the user to use `jr auth login --url ...` instead. Refresh
    // assumes the profile is already set up; rotating credentials on a
    // URL-less profile would leave it unusable for actual API calls.
    //
    // Round-16 of the multi-profile-auth review tightened this contract:
    // pre-fix, refresh would clear credentials and then ask for an email
    // (via --email / $JR_EMAIL), giving the user a misleading recovery
    // path. Post-fix, the error names the actual root cause (no profile
    // URL).
    //
    // `JR_SERVICE_NAME` scopes the keychain service so `auth::clear_*`
    // inside the subprocess never touches the developer's real
    // `jr-jira-cli` entries when `cargo test` runs locally.
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        // Config::load() merges JR_* via figment's Env::prefixed. Any
        // JR_INSTANCE_* env vars from the parent shell would flow into
        // the loaded config (e.g., JR_INSTANCE_URL would make the
        // empty-config look configured; JR_INSTANCE_AUTH_METHOD=oauth
        // would flip the flow). Clear the full set so the test pins
        // the unconfigured-profile path on every machine.
        .env_remove("JR_INSTANCE_URL")
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .env_remove("JR_INSTANCE_CLOUD_ID")
        .env_remove("JR_INSTANCE_ORG_ID")
        .env_remove("JR_INSTANCE_OAUTH_SCOPES")
        .env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .args(["--no-input", "auth", "refresh"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "auth refresh --no-input without setup should fail, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Refresh against unconfigured profile should exit 64 (UserError), got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("no URL configured")
            && stderr.contains("jr auth login")
            && stderr.contains("--url"),
        "Error should explain the missing URL and point at jr auth login --url: {stderr}"
    );
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
