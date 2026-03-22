use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct FieldsConfig {
    pub team_field_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub instance: InstanceConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub fields: FieldsConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct InstanceConfig {
    pub url: Option<String>,
    pub cloud_id: Option<String>,
    pub org_id: Option<String>,
    pub auth_method: Option<String>,
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

        Ok(Config { global, project })
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
            anyhow::anyhow!("No Jira instance configured. Run \"jr init\" first.")
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
                    cloud_id: None,
                    org_id: None,
                    auth_method: Some("api_token".into()),
                },
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
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
                    org_id: None,
                    auth_method: Some("oauth".into()),
                },
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
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
        };
        assert_eq!(config.project_key(Some("BAR")), Some("BAR".into()));
        assert_eq!(config.project_key(None), Some("FOO".into()));
    }

    #[test]
    fn test_base_url_env_override() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("JR_BASE_URL", "http://localhost:8080");
        let config = Config::default();
        assert_eq!(config.base_url().unwrap(), "http://localhost:8080");
        std::env::remove_var("JR_BASE_URL");
    }

    #[test]
    fn test_base_url_trailing_slash_trimmed() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let config = Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://myorg.atlassian.net/".into()),
                    cloud_id: None,
                    org_id: None,
                    auth_method: Some("api_token".into()),
                },
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
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
                    cloud_id: None,
                    org_id: None,
                    auth_method: Some("api_token".into()),
                },
                defaults: DefaultsConfig::default(),
                ..Default::default()
            },
            project: ProjectConfig::default(),
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
}
