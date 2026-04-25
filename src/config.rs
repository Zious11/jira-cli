use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::JrError;

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct FieldsConfig {
    pub team_field_id: Option<String>,
    pub story_points_field_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct ProfileConfig {
    pub url: Option<String>,
    pub auth_method: Option<String>,
    pub cloud_id: Option<String>,
    pub org_id: Option<String>,
    pub oauth_scopes: Option<String>,
    pub team_field_id: Option<String>,
    pub story_points_field_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct GlobalConfig {
    /// New-shape: name of the active profile.
    /// Resolved precedence: --profile > JR_PROFILE > this field > "default".
    /// `Option` because legacy configs don't have it.
    #[serde(default)]
    pub default_profile: Option<String>,

    /// New-shape: named profiles.
    #[serde(default)]
    pub profiles: std::collections::BTreeMap<String, ProfileConfig>,

    /// Legacy single-instance config — read for migration only.
    /// Kept on the in-memory struct so callers reading legacy fields keep
    /// working during the transition. Skipped on serialize so saved configs
    /// only contain the new shape.
    #[serde(default, skip_serializing)]
    pub instance: InstanceConfig,

    /// Legacy global custom-field IDs — read for migration only.
    #[serde(default, skip_serializing)]
    pub fields: FieldsConfig,

    #[serde(default)]
    pub defaults: DefaultsConfig,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct InstanceConfig {
    pub url: Option<String>,
    pub cloud_id: Option<String>,
    pub org_id: Option<String>,
    pub auth_method: Option<String>,
    pub oauth_scopes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DefaultsConfig {
    pub output: String,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            output: "table".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ProjectConfig {
    pub project: Option<String>,
    pub board_id: Option<u64>,
}

#[derive(Debug, Default)]
pub struct Config {
    pub global: GlobalConfig,
    pub project: ProjectConfig,
    /// Resolved at load() — flag > JR_PROFILE > default_profile > "default".
    pub active_profile_name: String,
}

/// Resolve the active profile name from precedence chain:
/// 1. cli_flag (--profile)
/// 2. env var (JR_PROFILE)
/// 3. config.default_profile field
/// 4. literal "default"
pub fn resolve_active_profile_name(
    config: &GlobalConfig,
    cli_flag: Option<&str>,
    env_var: Option<String>,
) -> String {
    if let Some(name) = cli_flag {
        return name.to_string();
    }
    if let Some(name) = env_var {
        return name;
    }
    if let Some(name) = config.default_profile.as_ref() {
        return name.clone();
    }
    "default".to_string()
}

/// Validate a profile name. See docs/specs/multi-profile-auth.md "Profile Name Validation".
pub fn validate_profile_name(name: &str) -> Result<(), JrError> {
    const RESERVED_WINDOWS: &[&str] = &[
        "CON", "NUL", "AUX", "PRN", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    if name.is_empty() || name.len() > 64 {
        return Err(invalid_profile_name(name));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(invalid_profile_name(name));
    }
    let upper = name.to_ascii_uppercase();
    if RESERVED_WINDOWS.contains(&upper.as_str()) {
        return Err(invalid_profile_name(name));
    }
    Ok(())
}

fn invalid_profile_name(name: &str) -> JrError {
    JrError::UserError(format!(
        "invalid profile name {name:?}; allowed: A-Z a-z 0-9 _ - up to 64 chars; \
         reserved Windows names (CON, NUL, AUX, PRN, COM1-9, LPT1-9) excluded"
    ))
}

/// Pure migration: copies a `GlobalConfig`'s legacy `[instance]` + `[fields]`
/// data into a new `[profiles.default]` entry. No-op if already in new shape.
///
/// Legacy fields are intentionally preserved during the transition (Tasks 4-15)
/// so callers that still read `global.instance.*` / `global.fields.*` keep
/// working until Tasks 7/8 migrate them to read `active_profile()` instead.
/// Task 16 stops serializing the legacy fields, so they fall off disk on the
/// next save.
pub fn migrate_legacy_global(mut global: GlobalConfig) -> GlobalConfig {
    if !global.profiles.is_empty() {
        return global;
    }

    if global.instance.url.is_none()
        && global.instance.auth_method.is_none()
        && global.instance.cloud_id.is_none()
        && global.instance.org_id.is_none()
        && global.instance.oauth_scopes.is_none()
        && global.fields.team_field_id.is_none()
        && global.fields.story_points_field_id.is_none()
    {
        return global;
    }

    let profile = ProfileConfig {
        url: global.instance.url.clone(),
        auth_method: global.instance.auth_method.clone(),
        cloud_id: global.instance.cloud_id.clone(),
        org_id: global.instance.org_id.clone(),
        oauth_scopes: global.instance.oauth_scopes.clone(),
        team_field_id: global.fields.team_field_id.clone(),
        story_points_field_id: global.fields.story_points_field_id.clone(),
    };
    global.profiles.insert("default".to_string(), profile);
    global.default_profile = Some("default".to_string());
    global
}

fn save_global_to(path: &std::path::Path, global: &GlobalConfig) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let content = toml::to_string_pretty(global)?;
    std::fs::write(path, content)?;
    Ok(())
}

impl Config {
    /// Strict load — used by every command except `jr auth login`.
    /// Errors if the resolved active profile isn't in `[profiles]`.
    pub fn load() -> anyhow::Result<Self> {
        Self::load_with(None)
    }

    /// Variant that accepts a CLI-flag profile override.
    ///
    /// Threading the `--profile` value as a parameter (instead of through an
    /// env-var seam like the legacy `JR_PROFILE_OVERRIDE`) avoids
    /// `unsafe { std::env::set_var(...) }` under `#[tokio::main]`, where
    /// worker threads exist before the async-main body runs and POSIX
    /// `setenv` is not thread-safe.
    pub fn load_with(cli_profile: Option<&str>) -> anyhow::Result<Self> {
        Self::load_inner(cli_profile, true)
    }

    /// Lenient load — used by `jr auth login` only, which legitimately
    /// creates profiles on demand. Skips the active-profile existence
    /// check; otherwise identical to [`Config::load`].
    pub fn load_lenient() -> anyhow::Result<Self> {
        Self::load_lenient_with(None)
    }

    /// Lenient variant that accepts a CLI-flag profile override. See
    /// [`Config::load_with`] for the threading rationale.
    pub fn load_lenient_with(cli_profile: Option<&str>) -> anyhow::Result<Self> {
        Self::load_inner(cli_profile, false)
    }

    fn load_inner(cli_profile: Option<&str>, strict: bool) -> anyhow::Result<Self> {
        let global_path = global_config_path();
        let mut global: GlobalConfig = Figment::new()
            .merge(Serialized::defaults(GlobalConfig::default()))
            .merge(Toml::file(&global_path))
            .merge(Env::prefixed("JR_"))
            .extract()?;

        let needs_migration = global.profiles.is_empty()
            && (global.instance.url.is_some()
                || global.instance.auth_method.is_some()
                || global.instance.cloud_id.is_some()
                || global.instance.org_id.is_some()
                || global.instance.oauth_scopes.is_some()
                || global.fields.team_field_id.is_some()
                || global.fields.story_points_field_id.is_some());

        if needs_migration {
            global = migrate_legacy_global(global);
            save_global_to(&global_path, &global)?;
            eprintln!(
                "Migrated config to multi-profile layout (single profile \"default\"). \
                 Run 'jr auth list' to view profiles."
            );
        }

        // Validate every profile name in the map. A hand-edited config with
        // quoted/invalid keys (e.g. `[profiles."foo:bar"]`) would otherwise
        // deserialize fine but produce names that can't be targeted by
        // switch/remove/logout/status (which validate input) AND would create
        // unsafe cache / keyring namespaces if used downstream. Placed after
        // the migration block (so the synthetic "default" key from migration
        // is also covered) and before resolving `active_profile_name` (so a
        // fresh first-run with empty profiles isn't gated).
        for name in global.profiles.keys() {
            validate_profile_name(name).map_err(|_| {
                JrError::UserError(format!(
                    "invalid profile name {name:?} in config.toml; allowed: \
                     A-Z a-z 0-9 _ - up to 64 chars; reserved Windows names \
                     (CON, NUL, AUX, PRN, COM1-9, LPT1-9) excluded"
                ))
            })?;
        }

        let project = Self::find_project_config()
            .map(|path| -> anyhow::Result<ProjectConfig> {
                Ok(Figment::new()
                    .merge(Toml::file(path))
                    .extract::<ProjectConfig>()?)
            })
            .transpose()?
            .unwrap_or_default();

        // The `--profile` CLI flag is threaded in as a parameter rather than
        // via an env-var seam. Earlier rounds used `JR_PROFILE_OVERRIDE`, but
        // setting it inside `#[tokio::main]` requires `unsafe { set_var }` at
        // a point where tokio worker threads already exist — POSIX `setenv`
        // is not thread-safe, so the cleaner fix is to drop the env-var seam
        // entirely. JR_PROFILE remains the user-facing env var.
        let env_profile = std::env::var("JR_PROFILE").ok();
        let active_profile_name = resolve_active_profile_name(&global, cli_profile, env_profile);
        // Validate the resolved name. JR_PROFILE / --profile / default_profile
        // all flow into cache paths and keyring keys, so a bad value (e.g.
        // "foo:bar" or path separators) must be rejected at the config boundary.
        validate_profile_name(&active_profile_name)?;

        // Verify the resolved active profile exists in [profiles] (when any
        // profiles are configured). A fresh install with no profiles yet is
        // allowed: jr init / jr auth login will create the first one.
        //
        // Skipped for `load_lenient` (used only by `jr auth login`), which
        // legitimately creates the target profile on demand and would
        // otherwise be locked out of `--profile newprof --url ...`.
        //
        // UserError (exit 64) instead of ConfigError (exit 78) because the
        // invalid input source is the user (--profile flag, JR_PROFILE env,
        // or a hand-edited default_profile field) — not a malformed config
        // file. Matches the wording used by switch/remove/logout/status.
        if strict
            && !global.profiles.is_empty()
            && !global.profiles.contains_key(&active_profile_name)
        {
            let known: Vec<&str> = global.profiles.keys().map(String::as_str).collect();
            return Err(JrError::UserError(format!(
                "unknown profile: {active_profile_name}; known: {}",
                known.join(", ")
            ))
            .into());
        }

        Ok(Config {
            global,
            project,
            active_profile_name,
        })
    }

    fn find_project_config() -> Option<PathBuf> {
        let mut dir = std::env::current_dir().ok()?;
        loop {
            let candidate = dir.join(".jr.toml");
            if candidate.exists() {
                return Some(candidate);
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    pub fn base_url(&self) -> anyhow::Result<String> {
        if let Ok(override_url) = std::env::var("JR_BASE_URL") {
            return Ok(override_url.trim_end_matches('/').to_string());
        }
        let profile = self.global.profiles.get(&self.active_profile_name).ok_or_else(|| {
            JrError::ConfigError(format!(
                "No Jira instance configured for profile {:?}. Run \"jr auth login --profile {}\" or \"jr init\".",
                self.active_profile_name, self.active_profile_name
            ))
        })?;
        let url = profile.url.as_ref().ok_or_else(|| {
            JrError::ConfigError(format!(
                "Profile {:?} has no URL configured. Run \"jr auth login --profile {}\".",
                self.active_profile_name, self.active_profile_name
            ))
        })?;
        if let Some(cloud_id) = &profile.cloud_id {
            if profile.auth_method.as_deref() == Some("oauth") {
                return Ok(format!("https://api.atlassian.com/ex/jira/{cloud_id}"));
            }
        }
        Ok(url.trim_end_matches('/').to_string())
    }

    pub fn project_key(&self, cli_override: Option<&str>) -> Option<String> {
        cli_override
            .map(String::from)
            .or_else(|| self.project.project.clone())
    }

    pub fn board_id(&self, cli_override: Option<u64>) -> Option<u64> {
        cli_override.or(self.project.board_id)
    }

    /// Look up the active profile. Returns a default-empty `ProfileConfig` if
    /// the active profile isn't in the map (legacy migration path runs before
    /// most callers reach this; tests can also exercise the empty case).
    pub fn active_profile(&self) -> ProfileConfig {
        self.global
            .profiles
            .get(&self.active_profile_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Strict variant — errors if the active profile isn't configured.
    pub fn active_profile_or_err(&self) -> anyhow::Result<&ProfileConfig> {
        self.global
            .profiles
            .get(&self.active_profile_name)
            .ok_or_else(|| {
                let known: Vec<&str> = self.global.profiles.keys().map(String::as_str).collect();
                JrError::ConfigError(format!(
                    "active profile {:?} not in [profiles]; known: {}; \
                     fix config.toml or run \"jr auth list\"",
                    self.active_profile_name,
                    if known.is_empty() {
                        "(none)".into()
                    } else {
                        known.join(", ")
                    }
                ))
                .into()
            })
    }

    pub fn save_global(&self) -> anyhow::Result<()> {
        save_global_to(&global_config_path(), &self.global)
    }
}

pub fn global_config_dir() -> PathBuf {
    // Use XDG_CONFIG_HOME if set, otherwise ~/.config (matches spec: ~/.config/jr/)
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("jr")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".config")
            .join("jr")
    }
}

pub fn global_config_path() -> PathBuf {
    global_config_dir().join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Guards tests that mutate process-global env vars so they don't
    /// interfere with other tests running in parallel.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let config = GlobalConfig::default();
        assert_eq!(config.defaults.output, "table");
        assert!(config.instance.url.is_none());
    }

    #[test]
    fn test_project_config_parsing() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join(".jr.toml");
        fs::write(&config_path, "project = \"FOO\"\nboard_id = 42\n").unwrap();

        let config: ProjectConfig = Figment::new()
            .merge(Toml::file(config_path))
            .extract()
            .unwrap();

        assert_eq!(config.project.as_deref(), Some("FOO"));
        assert_eq!(config.board_id, Some(42));
    }

    #[test]
    fn test_base_url_api_token() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                url: Some("https://myorg.atlassian.net".into()),
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );
        let config = Config {
            global: GlobalConfig {
                default_profile: Some("default".into()),
                profiles,
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: "default".into(),
        };
        assert_eq!(config.base_url().unwrap(), "https://myorg.atlassian.net");
    }

    #[test]
    fn test_base_url_oauth() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                url: Some("https://myorg.atlassian.net".into()),
                cloud_id: Some("abc-123".into()),
                auth_method: Some("oauth".into()),
                ..ProfileConfig::default()
            },
        );
        let config = Config {
            global: GlobalConfig {
                default_profile: Some("default".into()),
                profiles,
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: "default".into(),
        };
        assert_eq!(
            config.base_url().unwrap(),
            "https://api.atlassian.com/ex/jira/abc-123"
        );
    }

    #[test]
    fn test_base_url_missing() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let config = Config {
            global: GlobalConfig::default(),
            project: ProjectConfig::default(),
            active_profile_name: String::new(),
        };
        assert!(config.base_url().is_err());
    }

    #[test]
    fn base_url_uses_active_profile() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "sandbox".to_string(),
            ProfileConfig {
                url: Some("https://sandbox.atlassian.net".into()),
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );
        let config = Config {
            global: GlobalConfig {
                default_profile: Some("sandbox".into()),
                profiles,
                ..GlobalConfig::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: "sandbox".into(),
        };
        assert_eq!(config.base_url().unwrap(), "https://sandbox.atlassian.net");
    }

    #[test]
    fn base_url_uses_active_profile_oauth_path() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                url: Some("https://acme.atlassian.net".into()),
                auth_method: Some("oauth".into()),
                cloud_id: Some("abc-123".into()),
                ..ProfileConfig::default()
            },
        );
        let config = Config {
            global: GlobalConfig {
                default_profile: Some("default".into()),
                profiles,
                ..GlobalConfig::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: "default".into(),
        };
        assert_eq!(
            config.base_url().unwrap(),
            "https://api.atlassian.com/ex/jira/abc-123"
        );
    }

    #[test]
    fn test_project_key_cli_override() {
        let config = Config {
            global: GlobalConfig::default(),
            project: ProjectConfig {
                project: Some("FOO".into()),
                board_id: None,
            },
            active_profile_name: String::new(),
        };
        assert_eq!(config.project_key(Some("BAR")), Some("BAR".into()));
        assert_eq!(config.project_key(None), Some("FOO".into()));
    }

    #[test]
    fn test_board_id_cli_override() {
        let config = Config {
            global: GlobalConfig::default(),
            project: ProjectConfig {
                project: None,
                board_id: Some(42),
            },
            active_profile_name: String::new(),
        };
        // CLI override wins
        assert_eq!(config.board_id(Some(99)), Some(99));
        // Config fallback
        assert_eq!(config.board_id(None), Some(42));
        // Neither set
        let empty = Config::default();
        assert_eq!(empty.board_id(None), None);
    }

    #[test]
    fn test_base_url_env_override() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // SAFETY: test holds ENV_MUTEX, so no concurrent env access.
        unsafe { std::env::set_var("JR_BASE_URL", "http://localhost:8080") };
        let config = Config::default();
        assert_eq!(config.base_url().unwrap(), "http://localhost:8080");
        unsafe { std::env::remove_var("JR_BASE_URL") };
    }

    #[test]
    fn test_base_url_trailing_slash_trimmed() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                url: Some("https://myorg.atlassian.net/".into()),
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );
        let config = Config {
            global: GlobalConfig {
                default_profile: Some("default".into()),
                profiles,
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: "default".into(),
        };
        assert_eq!(config.base_url().unwrap(), "https://myorg.atlassian.net");
    }

    #[test]
    fn test_save_and_load_global_config() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");

        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                url: Some("https://test.atlassian.net".into()),
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );

        let config = Config {
            global: GlobalConfig {
                default_profile: Some("default".into()),
                profiles,
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: "default".into(),
        };

        // Write config to temp path
        let content = toml::to_string_pretty(&config.global).unwrap();
        fs::write(&config_path, &content).unwrap();

        // Legacy [instance]/[fields] blocks must not appear in serialized output
        assert!(!content.contains("[instance]"));
        assert!(!content.contains("[fields]"));

        // Read it back
        let loaded: GlobalConfig = Figment::new()
            .merge(Toml::file(&config_path))
            .extract()
            .unwrap();

        let p = loaded.profiles.get("default").expect("default profile");
        assert_eq!(p.url.as_deref(), Some("https://test.atlassian.net"));
        assert_eq!(p.auth_method.as_deref(), Some("api_token"));
    }

    #[test]
    fn instance_config_parses_oauth_scopes_from_toml() {
        let toml = r#"
            [instance]
            url = "https://example.atlassian.net"
            auth_method = "oauth"
            oauth_scopes = "read:issue:jira write:issue:jira offline_access"
        "#;

        let config: GlobalConfig = Figment::new().merge(Toml::string(toml)).extract().unwrap();

        assert_eq!(
            config.instance.oauth_scopes.as_deref(),
            Some("read:issue:jira write:issue:jira offline_access")
        );
    }

    #[test]
    fn instance_config_oauth_scopes_missing_is_none() {
        let toml = r#"
            [instance]
            url = "https://example.atlassian.net"
            auth_method = "oauth"
        "#;

        let config: GlobalConfig = Figment::new().merge(Toml::string(toml)).extract().unwrap();

        assert!(config.instance.oauth_scopes.is_none());
    }

    #[test]
    fn validate_profile_name_accepts_alphanumeric_dash_underscore() {
        assert!(validate_profile_name("default").is_ok());
        assert!(validate_profile_name("sandbox-uat").is_ok());
        assert!(validate_profile_name("team_a").is_ok());
        assert!(validate_profile_name("Prod1").is_ok());
        assert!(validate_profile_name("a").is_ok());
        assert!(validate_profile_name(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn validate_profile_name_rejects_invalid_chars() {
        for bad in [
            "", " ", "foo bar", "foo:bar", "foo/bar", "foo.bar", "..", ".",
        ] {
            assert!(
                validate_profile_name(bad).is_err(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn validate_profile_name_rejects_too_long() {
        let too_long = "a".repeat(65);
        assert!(validate_profile_name(&too_long).is_err());
    }

    #[test]
    fn validate_profile_name_rejects_windows_reserved_names_case_insensitive() {
        for bad in [
            "CON", "con", "Con", "NUL", "nul", "AUX", "aux", "PRN", "prn", "COM1", "com9", "LPT1",
            "lpt9",
        ] {
            assert!(
                validate_profile_name(bad).is_err(),
                "expected Windows reserved name {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn profile_config_roundtrip() {
        let toml = r#"
            url = "https://acme.atlassian.net"
            auth_method = "oauth"
            cloud_id = "abc-123"
            org_id = "def-456"
            oauth_scopes = "read:jira-work offline_access"
            team_field_id = "customfield_10001"
            story_points_field_id = "customfield_10002"
        "#;
        let p: ProfileConfig = toml::from_str(toml).unwrap();
        assert_eq!(p.url.as_deref(), Some("https://acme.atlassian.net"));
        assert_eq!(p.auth_method.as_deref(), Some("oauth"));
        assert_eq!(p.cloud_id.as_deref(), Some("abc-123"));
        assert_eq!(p.org_id.as_deref(), Some("def-456"));
        assert_eq!(p.team_field_id.as_deref(), Some("customfield_10001"));
        assert_eq!(
            p.story_points_field_id.as_deref(),
            Some("customfield_10002")
        );
    }

    #[test]
    fn global_config_parses_new_shape() {
        let toml = r#"
            default_profile = "default"

            [profiles.default]
            url = "https://acme.atlassian.net"
            auth_method = "api_token"

            [profiles.sandbox]
            url = "https://acme-sandbox.atlassian.net"
            auth_method = "oauth"
            cloud_id = "xyz-789"
        "#;
        let cfg: GlobalConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.default_profile.as_deref(), Some("default"));
        assert_eq!(cfg.profiles.len(), 2);
        assert!(cfg.profiles.contains_key("default"));
        assert!(cfg.profiles.contains_key("sandbox"));
        assert_eq!(cfg.profiles["sandbox"].cloud_id.as_deref(), Some("xyz-789"));
    }

    #[test]
    fn resolve_active_profile_name_uses_cli_flag_when_set() {
        let cfg = GlobalConfig {
            default_profile: Some("config-default".into()),
            ..GlobalConfig::default()
        };
        let name = resolve_active_profile_name(&cfg, Some("flag-value"), None);
        assert_eq!(name, "flag-value");
    }

    #[test]
    fn resolve_active_profile_name_uses_env_when_no_flag() {
        let cfg = GlobalConfig {
            default_profile: Some("config-default".into()),
            ..GlobalConfig::default()
        };
        let name = resolve_active_profile_name(&cfg, None, Some("env-value".into()));
        assert_eq!(name, "env-value");
    }

    #[test]
    fn resolve_active_profile_name_uses_config_when_no_flag_or_env() {
        let cfg = GlobalConfig {
            default_profile: Some("config-default".into()),
            ..GlobalConfig::default()
        };
        let name = resolve_active_profile_name(&cfg, None, None);
        assert_eq!(name, "config-default");
    }

    #[test]
    fn resolve_active_profile_name_falls_back_to_default_literal() {
        let cfg = GlobalConfig::default();
        let name = resolve_active_profile_name(&cfg, None, None);
        assert_eq!(name, "default");
    }

    #[test]
    fn config_active_profile_returns_resolved_profile() {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "sandbox".to_string(),
            ProfileConfig {
                url: Some("https://sandbox.example".into()),
                ..ProfileConfig::default()
            },
        );
        let cfg = Config {
            global: GlobalConfig {
                default_profile: Some("sandbox".into()),
                profiles,
                ..GlobalConfig::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: "sandbox".into(),
        };
        assert_eq!(
            cfg.active_profile().url.as_deref(),
            Some("https://sandbox.example")
        );
    }

    #[test]
    fn config_active_profile_unknown_profile_returns_error() {
        let cfg = Config {
            global: GlobalConfig::default(),
            project: ProjectConfig::default(),
            active_profile_name: "ghost".into(),
        };
        assert!(cfg.active_profile_or_err().is_err());
    }

    #[test]
    fn migrate_legacy_instance_into_default_profile() {
        let global = GlobalConfig {
            instance: InstanceConfig {
                url: Some("https://legacy.example".into()),
                cloud_id: Some("legacy-1".into()),
                org_id: Some("org-1".into()),
                auth_method: Some("api_token".into()),
                oauth_scopes: None,
            },
            fields: FieldsConfig {
                team_field_id: Some("customfield_99".into()),
                story_points_field_id: Some("customfield_42".into()),
            },
            ..GlobalConfig::default()
        };

        let migrated = migrate_legacy_global(global);

        assert_eq!(migrated.default_profile.as_deref(), Some("default"));
        assert_eq!(migrated.profiles.len(), 1);
        let p = &migrated.profiles["default"];
        assert_eq!(p.url.as_deref(), Some("https://legacy.example"));
        assert_eq!(p.cloud_id.as_deref(), Some("legacy-1"));
        assert_eq!(p.team_field_id.as_deref(), Some("customfield_99"));
        assert_eq!(p.story_points_field_id.as_deref(), Some("customfield_42"));
        // Legacy fields are intentionally preserved during the transition so
        // callers that still read them keep working until Tasks 7/8 migrate.
        assert_eq!(
            migrated.instance.url.as_deref(),
            Some("https://legacy.example"),
            "[instance] preserved during transition"
        );
        assert_eq!(
            migrated.fields.team_field_id.as_deref(),
            Some("customfield_99"),
            "[fields] preserved during transition"
        );
    }

    #[test]
    fn migrate_legacy_is_idempotent_when_already_new_shape() {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "custom".to_string(),
            ProfileConfig {
                url: Some("https://x.example".into()),
                ..ProfileConfig::default()
            },
        );
        let global = GlobalConfig {
            default_profile: Some("custom".into()),
            profiles,
            ..GlobalConfig::default()
        };
        let migrated = migrate_legacy_global(global.clone());
        assert_eq!(migrated.default_profile.as_deref(), Some("custom"));
        assert_eq!(migrated.profiles.len(), 1);
        assert_eq!(
            migrated.profiles["custom"].url.as_deref(),
            Some("https://x.example")
        );
    }

    #[test]
    fn migrate_legacy_with_no_data_yields_empty_new_shape() {
        let global = GlobalConfig::default();
        let migrated = migrate_legacy_global(global);
        assert!(migrated.profiles.is_empty());
        assert!(migrated.default_profile.is_none());
    }

    #[test]
    fn migrate_legacy_with_only_org_id_set_creates_profile() {
        let global = GlobalConfig {
            instance: InstanceConfig {
                org_id: Some("org-only".into()),
                ..InstanceConfig::default()
            },
            ..GlobalConfig::default()
        };
        let migrated = migrate_legacy_global(global);
        assert_eq!(migrated.default_profile.as_deref(), Some("default"));
        assert_eq!(
            migrated.profiles["default"].org_id.as_deref(),
            Some("org-only")
        );
    }

    #[test]
    fn migrate_legacy_with_only_oauth_scopes_set_creates_profile() {
        let global = GlobalConfig {
            instance: InstanceConfig {
                oauth_scopes: Some("read:jira-work offline_access".into()),
                ..InstanceConfig::default()
            },
            ..GlobalConfig::default()
        };
        let migrated = migrate_legacy_global(global);
        assert_eq!(migrated.default_profile.as_deref(), Some("default"));
        assert_eq!(
            migrated.profiles["default"].oauth_scopes.as_deref(),
            Some("read:jira-work offline_access")
        );
    }

    #[test]
    fn config_load_precedence_flag_overrides_env_overrides_field() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let config_path = cfg_dir.join("config.toml");
        std::fs::write(
            &config_path,
            r#"
                default_profile = "from-config"
                [profiles.from-config]
                url = "https://x"
                [profiles.from-env]
                url = "https://y"
                [profiles.from-flag]
                url = "https://z"
            "#,
        )
        .unwrap();

        // SAFETY: ENV_MUTEX held across env mutations.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_PROFILE", "from-env");
        }
        // CLI flag wins over env var.
        let cfg = Config::load_with(Some("from-flag")).unwrap();
        assert_eq!(cfg.active_profile_name, "from-flag");

        // Without the CLI flag, JR_PROFILE wins over the config field.
        let cfg = Config::load_with(None).unwrap();
        assert_eq!(cfg.active_profile_name, "from-env");

        unsafe {
            std::env::remove_var("JR_PROFILE");
        }
        // With neither flag nor env, the config field wins.
        let cfg = Config::load_with(None).unwrap();
        assert_eq!(cfg.active_profile_name, "from-config");

        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[test]
    fn config_load_errors_when_jr_profile_targets_unknown_profile() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::write(
            cfg_dir.join("config.toml"),
            r#"
                default_profile = "default"
                [profiles.default]
                url = "https://x"
            "#,
        )
        .unwrap();

        // SAFETY: ENV_MUTEX held.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_PROFILE", "ghost");
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("JR_PROFILE");
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        let err = result.expect_err("ghost profile should fail Config::load");
        let je = err.downcast_ref::<JrError>().expect("should be JrError");
        assert!(
            matches!(je, JrError::UserError(_)),
            "expected UserError, got {je:?}"
        );
        let msg = format!("{err:#}");
        assert!(msg.contains("unknown profile"), "got: {msg}");
        assert!(msg.contains("ghost"), "got: {msg}");
        assert!(msg.contains("default"), "got: {msg}");
    }

    #[test]
    fn config_load_rejects_invalid_profile_name_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        // SAFETY: ENV_MUTEX held.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_PROFILE", "evil:profile");
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("JR_PROFILE");
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        assert!(
            result.is_err(),
            "JR_PROFILE with invalid char should reject"
        );
    }

    #[test]
    fn config_load_lenient_succeeds_when_active_profile_unknown() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::write(
            cfg_dir.join("config.toml"),
            r#"
                default_profile = "default"
                [profiles.default]
                url = "https://x"
            "#,
        )
        .unwrap();

        // SAFETY: ENV_MUTEX held.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_PROFILE", "ghost");
        }
        let strict = Config::load();
        let lenient = Config::load_lenient();
        unsafe {
            std::env::remove_var("JR_PROFILE");
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        assert!(strict.is_err(), "strict load should reject unknown profile");
        assert!(
            lenient.is_ok(),
            "lenient load should accept unknown profile"
        );
        let cfg = lenient.unwrap();
        assert_eq!(cfg.active_profile_name, "ghost");
        assert_eq!(cfg.global.profiles.len(), 1, "profile map untouched");
    }

    #[test]
    fn config_load_rejects_invalid_profile_key_in_config() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::write(
            cfg_dir.join("config.toml"),
            r#"
                default_profile = "default"
                [profiles.default]
                url = "https://x"
                [profiles."bad:name"]
                url = "https://y"
            "#,
        )
        .unwrap();

        // SAFETY: ENV_MUTEX held.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        let err = result.expect_err("invalid profile key should reject");
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid profile name"), "got: {msg}");
        assert!(msg.contains("bad:name"), "got: {msg}");
    }

    #[test]
    fn global_config_parses_legacy_shape_into_legacy_fields() {
        let toml = r#"
            [instance]
            url = "https://legacy.atlassian.net"
            auth_method = "api_token"
            cloud_id = "legacy-1"

            [fields]
            team_field_id = "customfield_99"
            story_points_field_id = "customfield_42"
        "#;
        let cfg: GlobalConfig = toml::from_str(toml).unwrap();
        assert!(cfg.profiles.is_empty(), "no [profiles] in legacy shape");
        assert!(
            cfg.default_profile.is_none(),
            "no default_profile in legacy shape"
        );
        assert_eq!(
            cfg.instance.url.as_deref(),
            Some("https://legacy.atlassian.net")
        );
        assert_eq!(cfg.fields.team_field_id.as_deref(), Some("customfield_99"));
    }
}
