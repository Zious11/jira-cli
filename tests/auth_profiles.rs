//! Integration tests for multi-profile auth workflows.

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

fn jr() -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env_remove("JR_PROFILE")
        .env_remove("JR_PROFILE_OVERRIDE");
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
