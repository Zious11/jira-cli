//! S-1.08: Keychain layout holdout suite (H-016, BC-1.4.025..BC-1.4.030)
//!
//! This file contains the integration test for AC-006 (H-016): the destructive
//! operation guard that prevents removing the active profile. The test verifies:
//! - exit code 64
//! - stderr contains "cannot remove active"
//! - the config file on disk is byte-identical before and after the invocation
//!
//! The inline unit tests for AC-001..005 (key naming + profile boundary logic)
//! live in `src/api/auth.rs#[cfg(test)]` and `src/cli/auth.rs#[cfg(test)]`
//! to avoid requiring any visibility promotion of private functions.

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

/// Helper to build a `jr` Command with all JR_* env vars scrubbed (matches
/// the pattern from `tests/auth_profiles.rs` to avoid dev-machine pollution).
fn jr() -> Command {
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

/// Helper to create a fresh XDG_CONFIG_HOME temp dir with a config.toml at
/// `<dir>/jr/config.toml`.
fn fresh_config_dir_with_content(content: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("jr").join("config.toml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(&config_path, content).unwrap();
    (dir, config_path)
}

/// AC-006 (H-016, BC-1.1.006): Removing the active profile must be rejected
/// with exit code 64 and `cannot remove active` in stderr, and the config
/// file on disk must be byte-identical before and after the invocation.
///
/// This test extends the existing `auth_remove_active_profile_exits_64` coverage
/// in `tests/auth_profiles.rs` with the file-byte-identity assertion — the
/// key addition specified in the H-016 holdout requirement.
///
/// `JR_SERVICE_NAME=jr-jira-cli-test` is set to isolate from the developer's
/// real keychain per the architecture compliance rule for process-spawn tests.
#[test]
fn test_s_1_08_ac006_h016_remove_active_profile_rejected_and_config_unchanged() {
    let config_content = r#"default_profile = "default"
[profiles.default]
url = "https://example.atlassian.net"
auth_method = "api_token"
"#;
    let (dir, config_path) = fresh_config_dir_with_content(config_content);

    // Capture the config bytes BEFORE the command runs.
    let bytes_before =
        std::fs::read(&config_path).expect("config file must be readable before command");

    let output = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        // Isolate keychain from the developer's real keychain service.
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        .args(["--no-input", "auth", "remove", "default"])
        .output()
        .expect("jr command must spawn");

    // Exit code 64 (UserError / usage error).
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64, got: {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    // Stderr must contain the guard message.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot remove active"),
        "stderr must contain 'cannot remove active'; got: {stderr}"
    );

    // KEY ADDITION over auth_profiles.rs::auth_remove_active_profile_exits_64:
    // the config file must be byte-identical — no partial write, no truncation.
    let bytes_after =
        std::fs::read(&config_path).expect("config file must still be readable after command");
    assert_eq!(
        bytes_before, bytes_after,
        "config file must be byte-identical after rejected auth remove; \
         any mutation (even a no-op rewrite) violates the safety contract"
    );
}

/// AC-006 (H-016) — variant: the `--no-input` flag is not required; the
/// guard fires before any interactive prompt is shown. Verify the rejection
/// also works without `--no-input` (stdin closed, simulating a non-TTY pipe).
///
/// This test complements the primary test above and confirms the guard is
/// evaluated unconditionally at command dispatch, not gated behind prompt flow.
#[test]
fn test_s_1_08_ac006_h016_remove_active_profile_rejected_without_no_input_flag() {
    let config_content = r#"default_profile = "default"
[profiles.default]
url = "https://example.atlassian.net"
auth_method = "api_token"
"#;
    let (dir, config_path) = fresh_config_dir_with_content(config_content);

    let bytes_before =
        std::fs::read(&config_path).expect("config file must be readable before command");

    // stdin() is not set, so it defaults to Stdio::inherit; the CLI
    // will detect non-TTY (pipe) and treat it as --no-input automatically.
    let output = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        .args(["auth", "remove", "default"])
        .output()
        .expect("jr command must spawn");

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 without --no-input flag; got: {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot remove active"),
        "stderr must contain 'cannot remove active' without --no-input; got: {stderr}"
    );

    let bytes_after =
        std::fs::read(&config_path).expect("config file must still be readable after command");
    assert_eq!(
        bytes_before, bytes_after,
        "config file must be byte-identical even without --no-input flag"
    );
}

/// AC-006 (H-016) — second profile present: the guard still fires even when
/// another non-active profile exists. The restriction is "cannot remove the
/// ACTIVE profile", not "cannot remove when only one profile exists".
///
/// File-byte-identity is verified for the multi-profile config variant.
#[test]
fn test_s_1_08_ac006_h016_remove_active_profile_rejected_with_second_profile_present() {
    let config_content = r#"default_profile = "default"
[profiles.default]
url = "https://example.atlassian.net"
auth_method = "api_token"
[profiles.staging]
url = "https://staging.atlassian.net"
auth_method = "oauth"
"#;
    let (dir, config_path) = fresh_config_dir_with_content(config_content);

    let bytes_before =
        std::fs::read(&config_path).expect("config file must be readable before command");

    let output = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_SERVICE_NAME", "jr-jira-cli-test")
        .args(["--no-input", "auth", "remove", "default"])
        .output()
        .expect("jr command must spawn");

    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 with second profile present; got: {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot remove active"),
        "guard must fire regardless of other profiles; got: {stderr}"
    );

    let bytes_after =
        std::fs::read(&config_path).expect("config file must still be readable after command");
    assert_eq!(
        bytes_before, bytes_after,
        "multi-profile config must be byte-identical after rejected remove"
    );
}
