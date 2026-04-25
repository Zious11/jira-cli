//! Integration tests for multi-profile auth workflows.

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

fn jr() -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env_remove("JR_PROFILE");
    cmd
}

fn fresh_config_dir() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("jr").join("config.toml");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    (dir, cfg)
}

#[test]
fn auth_switch_unknown_profile_exits_64() {
    let (dir, _path) = fresh_config_dir();
    jr().env("XDG_CONFIG_HOME", dir.path())
        .args(["auth", "switch", "ghost"])
        .assert()
        .failure()
        .code(64);
}

#[test]
fn auth_list_shows_no_profiles_for_fresh_install() {
    let (dir, _path) = fresh_config_dir();
    jr().env("XDG_CONFIG_HOME", dir.path())
        .args(["auth", "list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicates::str::contains("[]"));
}

/// Regression: `jr auth status` against a fresh install (no [profiles]
/// in config — or no config at all) must succeed with a "not configured"
/// message, not error with "unknown profile". Setup scripts and CI use
/// `auth status` as a first-run probe before deciding whether to call
/// `jr init` or `jr auth login`.
#[test]
fn auth_status_fresh_install_no_profiles_succeeds() {
    let (dir, _path) = fresh_config_dir(); // no config.toml written
    jr().env("XDG_CONFIG_HOME", dir.path())
        .args(["auth", "status"])
        .assert()
        .success()
        .stderr(predicates::str::contains("No profiles configured"));
}

#[test]
fn auth_status_unknown_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://x.example"
auth_method = "api_token"
"#,
    )
    .unwrap();
    jr().env("XDG_CONFIG_HOME", dir.path())
        .args(["auth", "status", "--profile", "ghost"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"));
}

#[test]
fn auth_logout_unknown_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://x.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .args(["auth", "logout", "--profile", "ghost"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"));
}

#[test]
fn auth_remove_active_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://x.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .args(["auth", "remove", "default", "--no-input"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("cannot remove active"));
}

#[test]
fn precedence_flag_overrides_env_overrides_config() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "from-config"
[profiles.from-config]
url = "https://from-config.example"
[profiles.from-env]
url = "https://from-env.example"
[profiles.from-flag]
url = "https://from-flag.example"
"#,
    )
    .unwrap();

    let out = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_PROFILE", "from-env")
        .args(["--profile", "from-flag", "auth", "list", "--output", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|_| panic!("parse: {stdout}"));
    let active: Vec<&serde_json::Value> = parsed
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["active"].as_bool() == Some(true))
        .collect();
    assert_eq!(
        active.len(),
        1,
        "expected exactly one active profile, got {}: {parsed:?}",
        active.len()
    );
    assert_eq!(active[0]["name"], "from-flag");
}

/// Regression (Copilot round-10): the global `--profile` flag was being
/// dropped by `auth status`, `auth login`, `auth refresh`, and `auth logout`
/// because each handler reloaded config internally and only saw the
/// subcommand-level `--profile`. main.rs now composes an effective profile
/// (`subcmd.profile.or(cli.profile)`) so the global flag propagates.
#[test]
fn global_profile_flag_targets_auth_status() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.sandbox]
url = "https://sandbox.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // Global `--profile sandbox` without subcommand-level `--profile`.
    // Status output must reflect sandbox, not default.
    let out = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .args(["--profile", "sandbox", "auth", "status"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("sandbox") || combined.contains("https://sandbox.example"),
        "global --profile flag should target sandbox; got: {combined}"
    );
}

/// Regression: round-4's unified active-profile existence check at
/// `Config::load` time broke `jr auth login --profile newprof --url ...`
/// because the profile didn't exist yet. `handle_login` now uses
/// `Config::load_lenient` to skip that check, restoring the documented
/// "login creates profile if absent" behavior.
///
/// Gated behind `JR_RUN_KEYRING_TESTS=1` because `login_token` writes the
/// shared API token to the keyring, which Linux CI may not have.
#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn auth_login_creates_new_profile_with_url() {
    if std::env::var("JR_RUN_KEYRING_TESTS").is_err() {
        return;
    }
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://existing.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // login --profile newprof should succeed and create the profile,
    // even though newprof isn't in [profiles] yet at load time.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_EMAIL", "user@example.com")
        .env("JR_API_TOKEN", "token-value")
        .args([
            "auth",
            "login",
            "--profile",
            "newprof",
            "--url",
            "https://newprof.example",
            "--no-input",
        ])
        .assert()
        .success();

    // Verify the profile was added to config.
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("[profiles.newprof]"), "saved: {saved}");
    assert!(saved.contains("https://newprof.example"), "saved: {saved}");
}

/// Regression: when `JR_PROFILE` points at a profile that doesn't exist
/// in `[profiles]` AND the user runs `jr auth login --profile <other>`
/// to create that other profile, login must still succeed. Round-5 found
/// that `login_token`/`login_oauth` reloaded config via strict
/// `Config::load()` after `handle_login`'s lenient load, which re-fired
/// the unknown-active-profile check on the unrelated `JR_PROFILE` value
/// and aborted the in-flight creation. Both internal reloads now use
/// `load_lenient` to match the orchestrator.
#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn auth_login_with_jr_profile_pointing_to_unrelated_profile_still_creates_target() {
    if std::env::var("JR_RUN_KEYRING_TESTS").is_err() {
        return;
    }
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://existing.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // JR_PROFILE points to a non-existent profile, but --profile points
    // to a different new profile that login should create. Login must
    // succeed despite the JR_PROFILE mismatch — login uses lenient load
    // throughout so the strict active-profile existence check never
    // fires for the in-flight creation.
    Command::cargo_bin("jr")
        .unwrap()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_PROFILE", "ghost")
        .env("JR_EMAIL", "user@example.com")
        .env("JR_API_TOKEN", "token-value")
        .args([
            "auth",
            "login",
            "--profile",
            "fresh",
            "--url",
            "https://fresh.example",
            "--no-input",
        ])
        .assert()
        .success();

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("[profiles.fresh]"), "saved: {saved}");
}
