//! Legacy [instance] -> [profiles.default] migration tests.

use std::fs;
use tempfile::TempDir;

#[test]
fn legacy_instance_block_migrated_in_memory() {
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

    // SAFETY: test runs single-threaded under cargo test --test
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
