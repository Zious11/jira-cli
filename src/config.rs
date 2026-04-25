use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::JrError;

#[derive(Debug, Deserialize, Serialize, Default)]
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

#[derive(Debug, Deserialize, Serialize, Default)]
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
    /// Removed in cleanup task once migration is fully wired.
    #[serde(default)]
    pub instance: InstanceConfig,

    /// Legacy global custom-field IDs — read for migration only.
    /// Migration moves these into the default profile.
    #[serde(default)]
    pub fields: FieldsConfig,

    #[serde(default)]
    pub defaults: DefaultsConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct InstanceConfig {
    pub url: Option<String>,
    pub cloud_id: Option<String>,
    pub org_id: Option<String>,
    pub auth_method: Option<String>,
    pub oauth_scopes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
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

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let global_path = global_config_path();
        let global: GlobalConfig = Figment::new()
            .merge(Serialized::defaults(GlobalConfig::default()))
            .merge(Toml::file(&global_path))
            .merge(Env::prefixed("JR_"))
            .extract()?;

        let project = Self::find_project_config()
            .map(|path| -> anyhow::Result<ProjectConfig> {
                Ok(Figment::new()
                    .merge(Toml::file(path))
                    .extract::<ProjectConfig>()?)
            })
            .transpose()?
            .unwrap_or_default();

        // JR_PROFILE_OVERRIDE is an INTERNAL seam set by main.rs from the parsed
        // --profile flag *before* Config::load runs. It MUST NOT be set by users
        // directly — JR_PROFILE is the user-facing env var. Task 9 wires the
        // `unsafe { std::env::set_var(...) }` call from main.
        let cli_profile_flag = std::env::var("JR_PROFILE_OVERRIDE").ok();
        let env_profile = std::env::var("JR_PROFILE").ok();
        let active_profile_name =
            resolve_active_profile_name(&global, cli_profile_flag.as_deref(), env_profile);

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
        // JR_BASE_URL env var overrides everything (used by tests to inject wiremock URL)
        if let Ok(override_url) = std::env::var("JR_BASE_URL") {
            return Ok(override_url.trim_end_matches('/').to_string());
        }

        let url = self.global.instance.url.as_ref().ok_or_else(|| {
            JrError::ConfigError("No Jira instance configured. Run \"jr init\" first.".into())
        })?;

        if let Some(cloud_id) = &self.global.instance.cloud_id {
            if self.global.instance.auth_method.as_deref() == Some("oauth") {
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
        let dir = global_config_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let content = toml::to_string_pretty(&self.global)?;
        std::fs::write(path, content)?;
        Ok(())
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
        let config = Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://myorg.atlassian.net".into()),
                    auth_method: Some("api_token".into()),
                    ..InstanceConfig::default()
                },
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: String::new(),
        };
        assert_eq!(config.base_url().unwrap(), "https://myorg.atlassian.net");
    }

    #[test]
    fn test_base_url_oauth() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let config = Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://myorg.atlassian.net".into()),
                    cloud_id: Some("abc-123".into()),
                    auth_method: Some("oauth".into()),
                    ..InstanceConfig::default()
                },
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: String::new(),
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
        let config = Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://myorg.atlassian.net/".into()),
                    auth_method: Some("api_token".into()),
                    ..InstanceConfig::default()
                },
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: String::new(),
        };
        assert_eq!(config.base_url().unwrap(), "https://myorg.atlassian.net");
    }

    #[test]
    fn test_save_and_load_global_config() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");

        let config = Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://test.atlassian.net".into()),
                    auth_method: Some("api_token".into()),
                    ..InstanceConfig::default()
                },
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: String::new(),
        };

        // Write config to temp path
        let content = toml::to_string_pretty(&config.global).unwrap();
        fs::write(&config_path, &content).unwrap();

        // Read it back
        let loaded: GlobalConfig = Figment::new()
            .merge(Toml::file(&config_path))
            .extract()
            .unwrap();

        assert_eq!(
            loaded.instance.url.as_deref(),
            Some("https://test.atlassian.net")
        );
        assert_eq!(loaded.instance.auth_method.as_deref(), Some("api_token"));
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
