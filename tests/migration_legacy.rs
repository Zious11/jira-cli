//! Legacy [instance] -> [profiles.default] migration tests.

use std::fs;
use std::sync::Mutex;
use tempfile::TempDir;

/// Both tests in this file mutate process-global env vars (XDG_CONFIG_HOME).
/// Cargo runs tests within a single integration-test binary in parallel by
/// default, so without serialization they race against each other and produce
/// flaky results. Cross-file races are out of scope here — each `tests/*.rs`
/// runs as its own binary with its own process.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn legacy_instance_block_migrated_in_memory() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let dir = TempDir::new().unwrap();
    let cfg_path = dir.path().join("jr").join("config.toml");
    fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    fs::write(
        &cfg_path,
        r#"
[instance]
url = "https://legacy.atlassian.net"
auth_method = "api_token"
cloud_id = "legacy-1"
org_id = "org-1"

[fields]
team_field_id = "customfield_99"
story_points_field_id = "customfield_42"

[defaults]
output = "json"
"#,
    )
    .unwrap();

    // SAFETY: ENV_MUTEX is held across the env-var mutation and the
    // Config::load that depends on it; no other code path in this test
    // binary mutates XDG_CONFIG_HOME concurrently.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
    }
    let config = jr::config::Config::load().unwrap();
    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    assert_eq!(config.active_profile_name, "default");
    assert!(config.global.profiles.contains_key("default"));
    let p = &config.global.profiles["default"];
    assert_eq!(p.url.as_deref(), Some("https://legacy.atlassian.net"));
    assert_eq!(p.cloud_id.as_deref(), Some("legacy-1"));
    assert_eq!(p.team_field_id.as_deref(), Some("customfield_99"));
    assert_eq!(p.story_points_field_id.as_deref(), Some("customfield_42"));

    assert_eq!(config.global.defaults.output, "json");

    let on_disk = fs::read_to_string(&cfg_path).unwrap();
    assert!(on_disk.contains("default_profile"));
    assert!(on_disk.contains("[profiles.default]"));
    assert!(
        !on_disk.contains("[instance]"),
        "[instance] should not be serialized"
    );
    assert!(
        !on_disk.contains("[fields]"),
        "[fields] should not be serialized"
    );
}

#[test]
fn migration_is_idempotent() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let dir = TempDir::new().unwrap();
    let cfg_path = dir.path().join("jr").join("config.toml");
    fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    fs::write(
        &cfg_path,
        r#"
[instance]
url = "https://x"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // SAFETY: ENV_MUTEX is held across the env-var mutation and the
    // Config::load that depends on it; no other code path in this test
    // binary mutates XDG_CONFIG_HOME concurrently.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
    }
    let _ = jr::config::Config::load().unwrap();
    let after_first = fs::read_to_string(&cfg_path).unwrap();
    let _ = jr::config::Config::load().unwrap();
    let after_second = fs::read_to_string(&cfg_path).unwrap();
    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    assert_eq!(
        after_first, after_second,
        "second load should not modify file"
    );
}
