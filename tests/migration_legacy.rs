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

/// Set of `JR_*` env vars that `Config::load` reads via figment's
/// `Env::prefixed("JR_")` or via direct `std::env::var` lookups
/// (`JR_PROFILE`, `JR_BASE_URL`). A developer with any of these
/// exported (commonly via direnv) would otherwise have their values
/// flow into the test's tempdir-scoped config and either trigger
/// legacy migration unexpectedly or shift the resolved active profile
/// — making the assertions about migration shape and idempotency
/// flaky. The guard scrubs them all on `set()` and restores prior
/// values on drop.
const JR_ENV_VARS_TO_SCRUB: &[&str] = &[
    "JR_PROFILE",
    "JR_DEFAULT_PROFILE",
    "JR_INSTANCE_URL",
    "JR_INSTANCE_AUTH_METHOD",
    "JR_INSTANCE_CLOUD_ID",
    "JR_INSTANCE_ORG_ID",
    "JR_INSTANCE_OAUTH_SCOPES",
    "JR_FIELDS_TEAM_FIELD_ID",
    "JR_FIELDS_STORY_POINTS_FIELD_ID",
    "JR_DEFAULTS_OUTPUT",
    "JR_BASE_URL",
    "JR_AUTH_HEADER",
];

/// RAII helper: sets `XDG_CONFIG_HOME` to `value` and scrubs `JR_*`
/// env vars for the duration of the guard's lifetime, then restores
/// every prior value (or unsets if none) on drop. Drop runs even if
/// the test panics, so a `Config::load` that unwraps unsuccessfully
/// never leaks state into the next test in the same binary. Also
/// avoids unconditionally clobbering a pre-existing `XDG_CONFIG_HOME`
/// or `JR_*` from the parent environment that the developer relied on
/// outside the test runner.
struct XdgConfigGuard {
    previous_xdg: Option<std::ffi::OsString>,
    previous_jr_vars: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl XdgConfigGuard {
    fn set(value: &std::path::Path) -> Self {
        let previous_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let previous_jr_vars: Vec<(&'static str, Option<std::ffi::OsString>)> =
            JR_ENV_VARS_TO_SCRUB
                .iter()
                .map(|&name| (name, std::env::var_os(name)))
                .collect();
        // SAFETY: tests in this binary serialize env mutation via
        // ENV_MUTEX; no concurrent access.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", value);
            for &name in JR_ENV_VARS_TO_SCRUB {
                std::env::remove_var(name);
            }
        }
        Self {
            previous_xdg,
            previous_jr_vars,
        }
    }
}

impl Drop for XdgConfigGuard {
    fn drop(&mut self) {
        // SAFETY: same as set() — caller must hold ENV_MUTEX while the
        // guard is alive; no concurrent access.
        unsafe {
            match self.previous_xdg.take() {
                Some(prev) => std::env::set_var("XDG_CONFIG_HOME", prev),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            for (name, prev) in std::mem::take(&mut self.previous_jr_vars) {
                match prev {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

#[test]
fn legacy_instance_block_migrated_in_memory() {
    let _env_lock = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
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

    let _xdg = XdgConfigGuard::set(dir.path());
    let config = jr::config::Config::load().unwrap();

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
    // _xdg dropped here — restores prior XDG_CONFIG_HOME (or unsets).
}

#[test]
fn migration_is_idempotent() {
    let _env_lock = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
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

    let _xdg = XdgConfigGuard::set(dir.path());
    let _ = jr::config::Config::load().unwrap();
    let after_first = fs::read_to_string(&cfg_path).unwrap();
    let _ = jr::config::Config::load().unwrap();
    let after_second = fs::read_to_string(&cfg_path).unwrap();

    assert_eq!(
        after_first, after_second,
        "second load should not modify file"
    );
    // _xdg dropped here — restores prior XDG_CONFIG_HOME (or unsets).
}
