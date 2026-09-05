use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::JrError;
use crate::profile::Profile;

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
    /// Default project key for this profile. Overridden by --project flag and .jr.toml.
    pub project: Option<String>,
    /// Free-form environment label for this profile (e.g. "prod", "sandbox",
    /// "uat") — illustrative only, no validation/allowlist (BC-6.1.015).
    /// Additive, tolerant-reader: absent in a pre-existing `config.toml`
    /// deserializes to `None`, matching the sibling `Option<String>` fields
    /// on this struct (no `#[serde(default)]` needed — same pattern).
    /// Storage stays verbatim; display-layer sanitization lives in
    /// `output::sanitize_env_display` (BC-6.1.015 EC-4).
    pub env: Option<String>,
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
    pub active_profile_name: Profile,
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

    // BC-6.1.004 (AC-006): length check first so the error is unambiguous when
    // both conditions fail. UserError (exit 64) because the name comes from
    // user-supplied input (--profile flag, JR_PROFILE env, default_profile
    // field) — not from a malformed config file.
    if name.is_empty() {
        return Err(JrError::UserError(
            "Profile name must not be empty".to_string(),
        ));
    }
    if name.len() > 64 {
        return Err(JrError::UserError(
            "Profile name too long (max 64 characters)".to_string(),
        ));
    }
    // BC-6.1.004 (AC-007): distinct message for charset violations.
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(JrError::UserError(
            "Profile name contains invalid characters (use a-z, 0-9, -, _)".to_string(),
        ));
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
        env: None,
        project: None,
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

        // Read with env-overlay for in-memory use. The rest of the program
        // sees `JR_*` env overrides applied on top of `config.toml`.
        //
        // GUARD: GlobalConfig MUST NEVER gain a `config_dir` or `cache_dir`
        // field. If it did, figment's `Env::prefixed("JR_")` would honor
        // `JR_CONFIG_DIR` / `JR_CACHE_DIR` in RELEASE builds, bypassing the
        // `#[cfg(debug_assertions)]` gate in `global_config_dir()` /
        // `cache_root()` and re-opening the path-injection vector (SEC-PATH-1).
        // Those env vars are intentionally debug-only seams (BC-6.2.017).
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
            // Write-back uses file-only data (NO env overlay) so transient
            // `JR_*` env vars set for one invocation (e.g.,
            // `JR_DEFAULTS_OUTPUT=json`) can never bleed into the migrated
            // `config.toml`. Without this, an env value set at upgrade time
            // would be silently baked into the user's on-disk config and
            // persist across future invocations even when the env var is
            // unset.
            //
            // In-memory `global` still gets migrated below so callers see
            // the new `[profiles.default]` entry; this two-source pattern
            // (env-merged for in-memory + file-only for save) keeps the
            // migration transparent without polluting the saved file.
            let file_only_global: GlobalConfig = Figment::new()
                .merge(Serialized::defaults(GlobalConfig::default()))
                .merge(Toml::file(&global_path))
                .extract()?;
            let to_save = migrate_legacy_global(file_only_global);
            save_global_to(&global_path, &to_save)?;
            global = migrate_legacy_global(global);
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
            // map_err supplies a file-locating message; keep even though validate_profile_name now returns UserError.
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
        let active_profile_name_raw =
            resolve_active_profile_name(&global, cli_profile, env_profile);
        // Validate the resolved name. JR_PROFILE / --profile / default_profile
        // all flow into cache paths and keyring keys, so a bad value (e.g.
        // "foo:bar" or path separators) must be rejected at the config boundary.
        validate_profile_name(&active_profile_name_raw)?;

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
            && !global.profiles.contains_key(&active_profile_name_raw)
        {
            let known: Vec<&str> = global.profiles.keys().map(String::as_str).collect();
            return Err(JrError::UserError(format!(
                "unknown profile: {active_profile_name_raw}; known: {}",
                known.join(", ")
            ))
            .into());
        }

        // Boundary construction (BC-6.2.015, ADR-0011): the raw profile-name
        // `String` resolved above becomes a type-fenced `Profile` here, right
        // where it first becomes available, so every downstream consumer of
        // `Config::active_profile_name` is compile-time guaranteed a real
        // `Profile` rather than a profile-unaware bare string.
        let active_profile_name = Profile::from(active_profile_name_raw);

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
        // JR_BASE_URL override is debug builds only — release binaries ignore this
        // env var to prevent JR_BASE_URL=http://attacker/ from becoming `self.base_url`
        // and redirecting authenticated requests to a non-Atlassian host (token-leak
        // vector). Mirrors the SD-002 JR_AUTH_HEADER gate in src/api/client.rs.
        // Closes audit-followup #335 (Config-layer half of the fix).
        #[cfg(debug_assertions)]
        if let Ok(override_url) = std::env::var("JR_BASE_URL") {
            return Ok(override_url.trim_end_matches('/').to_string());
        }
        let profile = self.global.profiles.get(self.active_profile_name.as_ref()).ok_or_else(|| {
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
            .or_else(|| self.active_profile().project.clone())
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
            .get(self.active_profile_name.as_ref())
            .cloned()
            .unwrap_or_default()
    }

    /// Strict variant — errors if the active profile isn't configured.
    pub fn active_profile_or_err(&self) -> anyhow::Result<&ProfileConfig> {
        self.global
            .profiles
            .get(self.active_profile_name.as_ref())
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
        let path = global_config_path();
        // Read the file-only baseline (no `JR_*` env overlay) so transient
        // env overrides on the invocation that mutates a profile (e.g.,
        // `JR_DEFAULTS_OUTPUT=json jr auth switch sandbox`) can't leak
        // into the saved config.toml. The in-memory `self.global` has env
        // overlays applied for the duration of this process; we want to
        // persist only the structural multi-profile changes (default
        // profile + profiles map), preserving everything else from disk.
        //
        // If the file doesn't exist yet (fresh install), start with the
        // default-empty `GlobalConfig` — legitimate first-run case where
        // `defaults`/etc. have nothing to preserve from disk anyway.
        let mut to_save: GlobalConfig = if path.exists() {
            Figment::new()
                .merge(Serialized::defaults(GlobalConfig::default()))
                .merge(Toml::file(&path))
                .extract()?
        } else {
            GlobalConfig::default()
        };

        // Overlay only the multi-profile fields. These are what callers
        // of `save_global` mutate (handle_switch, handle_remove,
        // handle_login, login_token/oauth, jr init). Other fields like
        // `defaults.output` are preserved from the file-only baseline.
        to_save.default_profile = self.global.default_profile.clone();
        to_save.profiles = self.global.profiles.clone();

        save_global_to(&path, &to_save)
    }
}

/// Pure fallback for a Windows `%APPDATA%`/`%LOCALAPPDATA%`-style env path when the
/// `dirs` crate returns `None`. Accepts the raw `env::var(NAME).ok()` value so the
/// logic can be tested on any platform without a `#[cfg(windows)]` gate.
///
/// Rules (BC-6.1.014 EC-1, EC-3):
/// - `Some(s)` where `s` is non-empty → `PathBuf::from(s)`
/// - `Some(s)` where `s` is empty → `PathBuf::from(".")` (treated as unset)
/// - `None` → `PathBuf::from(".")`
///
/// Called from the `#[cfg(windows)]` production branch in `global_config_dir()`
/// and directly from cross-platform unit tests.
pub fn config_appdata_fallback(env_val: Option<String>) -> PathBuf {
    env_val
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn global_config_dir() -> PathBuf {
    // JR_CONFIG_DIR override is debug builds only — release binaries ignore this env
    // var to prevent path-injection attacks (BC-6.2.017). Seam must be first in body,
    // before any OS-branch logic, so it fires on all platforms (S-WIN-2 prerequisite).
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var("JR_CONFIG_DIR")
        .ok()
        .filter(|s| !s.is_empty())
    {
        return PathBuf::from(dir);
    }

    #[cfg(windows)]
    {
        // Windows: %APPDATA%\jr  (e.g., C:\Users\Alice\AppData\Roaming\jr)
        // BC-6.1.014: dirs::config_dir() maps to %APPDATA% (Roaming) on Windows.
        // APPDATA fallback filters empty string: unset and empty both route to ".".
        dirs::config_dir()
            .unwrap_or_else(|| config_appdata_fallback(std::env::var("APPDATA").ok()))
            .join("jr")
    }

    #[cfg(not(windows))]
    {
        // Unix: $XDG_CONFIG_HOME/jr or ~/.config/jr (unchanged)
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg).join("jr")
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".config")
                .join("jr")
        }
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

    /// Set `var` to `value`, run `f`, then unconditionally remove `var` — even if
    /// `f` panics. Mirrors the `with_temp_cache` pattern in `cache.rs`.
    ///
    /// # Safety / threading
    /// Caller must hold `ENV_MUTEX` for the duration of the call (acquired by this
    /// helper itself). Two env-var names may need to be cleared *before* calling
    /// `f` (e.g. XDG_CONFIG_HOME); pass a separate `unsafe { remove_var }` call
    /// before this helper and keep it inside the same mutex guard scope.
    #[cfg(debug_assertions)]
    fn with_env_var<F: FnOnce() -> R, R>(var: &str, value: &str, f: F) -> R {
        // Recover from mutex poison — a prior test that panicked inside set_var..remove_var
        // will have poisoned the mutex; we recover so subsequent tests can still run.
        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_MUTEX is held for the duration; no concurrent env access occurs.
        unsafe { std::env::set_var(var, value) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        unsafe { std::env::remove_var(var) };
        drop(guard);
        match result {
            Ok(v) => v,
            Err(e) => std::panic::resume_unwind(e),
        }
    }

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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let config = Config {
            global: GlobalConfig::default(),
            project: ProjectConfig::default(),
            active_profile_name: Profile::default(),
        };
        assert!(config.base_url().is_err());
    }

    #[test]
    fn base_url_uses_active_profile() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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
            active_profile_name: Profile::default(),
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
            active_profile_name: Profile::default(),
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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: test holds ENV_MUTEX, so no concurrent env access.
        unsafe { std::env::set_var("JR_BASE_URL", "http://localhost:8080") };
        let config = Config::default();
        assert_eq!(config.base_url().unwrap(), "http://localhost:8080");
        unsafe { std::env::remove_var("JR_BASE_URL") };
    }

    #[test]
    fn test_base_url_trailing_slash_trimmed() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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

    /// BC-6.1.015 AC-001 / EC-3: a pre-existing `config.toml` written before
    /// the `env` field existed (no `env` key under `[profiles.x]`) must
    /// deserialize with `env: None` — no error, no warning. This is the
    /// tolerant-reader contract (DEC-314): `env` is purely additive.
    #[test]
    fn test_profile_config_env_absent_key_deserializes_to_none() {
        let toml = r#"
            url = "https://acme.atlassian.net"
            auth_method = "oauth"
        "#;
        let p: ProfileConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            p.env, None,
            "a config.toml with no `env` key must deserialize to env: None"
        );
    }

    /// BC-6.1.015 postcondition: a profile with `env = "prod"` set
    /// deserializes to `Some("prod")`.
    #[test]
    fn test_profile_config_env_present_deserializes_to_some() {
        let toml = r#"
            url = "https://acme.atlassian.net"
            env = "prod"
        "#;
        let p: ProfileConfig = toml::from_str(toml).unwrap();
        assert_eq!(p.env.as_deref(), Some("prod"));
    }

    /// BC-6.1.015 AC-002 / EC-2: `env = ""` (present but empty) must
    /// deserialize distinctly from an absent key — `Some(String::new())`,
    /// never collapsed to `None`. The `Some("")` vs `None` distinction is
    /// spec-fixed (BC-1.6.046 EC-1.6.046-1) and depends on this round-trip
    /// holding at the storage layer.
    #[test]
    fn test_profile_config_env_empty_string_deserializes_to_some_empty() {
        let toml = r#"
            url = "https://acme.atlassian.net"
            env = ""
        "#;
        let p: ProfileConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            p.env,
            Some(String::new()),
            "env = \"\" must deserialize to Some(\"\"), never None"
        );
    }

    /// BC-6.1.015 EC-4: storage stays verbatim — no allowlist/enum
    /// validation on `env`. Any string, including one with control
    /// characters or unicode, round-trips unmodified through serialize ->
    /// deserialize. (Display-layer sanitization is a separate concern,
    /// owned by `output::sanitize_env_display`.)
    #[test]
    fn test_profile_config_env_accepts_arbitrary_string_no_validation() {
        let p = ProfileConfig {
            env: Some("not-a-real-enum-value \u{1b}[31m\x00".into()),
            ..ProfileConfig::default()
        };
        let serialized = toml::to_string(&p).unwrap();
        let round_tripped: ProfileConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(round_tripped.env, p.env);
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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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
        //
        // JR_CONFIG_DIR is the cross-platform debug seam (BC-6.2.017): on Windows,
        // global_config_dir() uses %APPDATA% and ignores XDG_CONFIG_HOME, so we must
        // also set JR_CONFIG_DIR = dir/jr to keep all platforms reading the same config.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
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
            std::env::remove_var("JR_CONFIG_DIR");
        }
    }

    #[test]
    fn config_load_errors_when_jr_profile_targets_unknown_profile() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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
        //
        // JR_CONFIG_DIR is the cross-platform debug seam (BC-6.2.017): on Windows,
        // global_config_dir() uses %APPDATA% and ignores XDG_CONFIG_HOME.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
            std::env::set_var("JR_PROFILE", "ghost");
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("JR_PROFILE");
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("JR_CONFIG_DIR");
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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        // SAFETY: ENV_MUTEX held.
        //
        // JR_CONFIG_DIR is the cross-platform debug seam (BC-6.2.017): on Windows,
        // global_config_dir() uses %APPDATA% and ignores XDG_CONFIG_HOME.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
            std::env::set_var("JR_PROFILE", "evil:profile");
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("JR_PROFILE");
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("JR_CONFIG_DIR");
        }
        let err = result.expect_err("JR_PROFILE with invalid char should reject");
        let je = err.downcast_ref::<JrError>().expect("should be JrError");
        // H-019: JR_PROFILE is user-supplied input → must be UserError (exit 64),
        // not ConfigError (exit 78). Previously exited 78 (ConfigError); fixed by H-019.
        assert!(
            matches!(je, JrError::UserError(_)),
            "H-019: JR_PROFILE invalid charset must produce UserError, got {je:?}"
        );
        assert_eq!(
            je.exit_code(),
            64,
            "H-019: exit code must be 64 (EX_USAGE), got {}",
            je.exit_code()
        );
    }

    // -----------------------------------------------------------------------
    // H-019: Invalid profile name via --profile flag → UserError (exit 64)
    //
    // BC-6.1.004 contract: the input source is the user (--profile flag), so
    // charset/length violations must produce JrError::UserError (exit 64),
    // not JrError::ConfigError (exit 78).
    //
    // Red Gate: currently validate_profile_name returns ConfigError for
    // charset and length violations, propagated raw via `?` at load_inner
    // line ~321, producing exit 78. These tests fail until the fix is applied.
    // -----------------------------------------------------------------------

    /// H-019 (BC-6.1.004): `Config::load_with(Some("foo:bar"))` must return
    /// `Err` with `JrError::UserError` (exit 64). The colon is an invalid
    /// charset character; the input comes from the `--profile` flag, not a
    /// config file.
    ///
    /// Previously exited 78 (ConfigError) because `validate_profile_name`
    /// returned `JrError::ConfigError` for charset violations and `load_inner`
    /// propagated it raw via `?`. Fixed by H-019.
    #[test]
    fn test_load_with_invalid_charset_profile_flag_returns_user_error() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        // SAFETY: ENV_MUTEX held.
        //
        // JR_CONFIG_DIR is the cross-platform debug seam (BC-6.2.017).
        // Clear JR_PROFILE so only the cli_flag path is exercised.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
            std::env::remove_var("JR_PROFILE");
        }
        let result = Config::load_with(Some("foo:bar"));
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("JR_CONFIG_DIR");
        }
        let err = result.expect_err("--profile foo:bar should reject (colon is invalid charset)");
        let je = err.downcast_ref::<JrError>().expect("should be JrError");
        assert!(
            matches!(je, JrError::UserError(_)),
            "H-019: --profile flag invalid charset must produce UserError, got {je:?}"
        );
        assert_eq!(
            je.exit_code(),
            64,
            "H-019: exit code must be 64 (EX_USAGE), got {}",
            je.exit_code()
        );
    }

    /// H-019 (BC-6.1.004): `Config::load_with(Some(""))` must return
    /// `Err` with `JrError::UserError` (exit 64). An empty profile name
    /// supplied via `--profile ""` is a user error.
    ///
    /// Previously exited 78 (ConfigError) because `validate_profile_name`
    /// returned `JrError::ConfigError` for the empty-name branch. Fixed by H-019.
    #[test]
    fn test_load_with_empty_profile_flag_returns_user_error() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        // SAFETY: ENV_MUTEX held.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
            std::env::remove_var("JR_PROFILE");
        }
        let result = Config::load_with(Some(""));
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("JR_CONFIG_DIR");
        }
        let err = result.expect_err("--profile \"\" should reject (empty name)");
        let je = err.downcast_ref::<JrError>().expect("should be JrError");
        assert!(
            matches!(je, JrError::UserError(_)),
            "H-019: --profile flag empty name must produce UserError, got {je:?}"
        );
        assert_eq!(
            je.exit_code(),
            64,
            "H-019: exit code must be 64 (EX_USAGE), got {}",
            je.exit_code()
        );
    }

    /// H-019 (BC-6.1.004): `Config::load_with(Some(&"a".repeat(65)))` must
    /// return `Err` with `JrError::UserError` (exit 64). A 65-char profile
    /// name supplied via `--profile` is a user error (too long).
    ///
    /// Previously exited 78 (ConfigError) because `validate_profile_name`
    /// returned `JrError::ConfigError` for the too-long branch. Fixed by H-019.
    #[test]
    fn test_load_with_overlength_profile_flag_returns_user_error() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        // SAFETY: ENV_MUTEX held.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
            std::env::remove_var("JR_PROFILE");
        }
        let long_name = "a".repeat(65);
        let result = Config::load_with(Some(long_name.as_str()));
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("JR_CONFIG_DIR");
        }
        let err = result.expect_err("--profile with 65-char name should reject (too long)");
        let je = err.downcast_ref::<JrError>().expect("should be JrError");
        assert!(
            matches!(je, JrError::UserError(_)),
            "H-019: --profile flag too-long name must produce UserError, got {je:?}"
        );
        assert_eq!(
            je.exit_code(),
            64,
            "H-019: exit code must be 64 (EX_USAGE), got {}",
            je.exit_code()
        );
    }

    /// H-019 (BC-6.1.004): `JR_PROFILE=""` (empty string) must return
    /// `Err` with `JrError::UserError` (exit 64). An empty profile name
    /// from the env var is a user error.
    ///
    /// Previously exited 78 (ConfigError) because `validate_profile_name`
    /// returned `JrError::ConfigError` for the empty-name branch, propagated
    /// raw by `load_inner`. Fixed by H-019.
    #[test]
    fn test_load_jr_profile_env_empty_returns_user_error() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        // SAFETY: ENV_MUTEX held.
        //
        // JR_CONFIG_DIR is the cross-platform debug seam (BC-6.2.017).
        // Set JR_PROFILE="" so the env-var path exercises the empty-name guard.
        // std::env::var("JR_PROFILE") returns Ok("") → Some("") → resolves to "".
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
            std::env::set_var("JR_PROFILE", "");
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("JR_PROFILE");
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("JR_CONFIG_DIR");
        }
        let err = result.expect_err("JR_PROFILE=\"\" should reject (empty name)");
        let je = err.downcast_ref::<JrError>().expect("should be JrError");
        assert!(
            matches!(je, JrError::UserError(_)),
            "H-019: JR_PROFILE empty name must produce UserError, got {je:?}"
        );
        assert_eq!(
            je.exit_code(),
            64,
            "H-019: exit code must be 64 (EX_USAGE), got {}",
            je.exit_code()
        );
    }

    /// H-019 (BC-6.1.004): `Config::load_with(Some("valid-name"))` returns `Ok` when the
    /// config has a `[profiles.valid-name]` entry. Guards the `load_with(cli_flag)` wiring
    /// and kills the "charset check → unconditional reject" mutant.
    #[test]
    fn test_load_with_valid_profile_flag_returns_ok() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        let cfg_dir = dir.path().join("jr");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::write(
            cfg_dir.join("config.toml"),
            r#"
                default_profile = "valid-name"
                [profiles.valid-name]
                url = "https://example.atlassian.net"
                auth_method = "api_token"
            "#,
        )
        .unwrap();

        // SAFETY: ENV_MUTEX held.
        //
        // JR_CONFIG_DIR is the cross-platform debug seam (BC-6.2.017).
        // Clear JR_PROFILE so only the cli_flag path is exercised.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
            std::env::remove_var("JR_PROFILE");
        }
        let result = Config::load_with(Some("valid-name"));
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("JR_CONFIG_DIR");
        }

        let cfg = result.expect("load_with(valid-name) must return Ok for a known, valid profile");
        assert_eq!(
            cfg.active_profile_name, "valid-name",
            "active_profile_name must match the cli_flag value"
        );
    }

    #[test]
    fn config_load_lenient_succeeds_when_active_profile_unknown() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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
        //
        // JR_CONFIG_DIR is the cross-platform debug seam (BC-6.2.017): on Windows,
        // global_config_dir() uses %APPDATA% and ignores XDG_CONFIG_HOME.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
            std::env::set_var("JR_PROFILE", "ghost");
        }
        let strict = Config::load();
        let lenient = Config::load_lenient();
        unsafe {
            std::env::remove_var("JR_PROFILE");
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("JR_CONFIG_DIR");
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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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
        //
        // JR_CONFIG_DIR is the cross-platform debug seam (BC-6.2.017): on Windows,
        // global_config_dir() uses %APPDATA% and ignores XDG_CONFIG_HOME.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
            std::env::set_var("JR_CONFIG_DIR", dir.path().join("jr"));
        }
        let result = Config::load();
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("JR_CONFIG_DIR");
        }

        let err = result.expect_err("invalid profile key should reject");
        // Regression guard (H-019): config-file boundary must emit UserError (exit 64),
        // not ConfigError (exit 78). This already works via the .map_err wrapper in
        // load_inner; assert explicitly to prevent future regression.
        let je = err.downcast_ref::<JrError>().expect("should be JrError");
        assert!(
            matches!(je, JrError::UserError(_)),
            "config-file invalid profile key must produce UserError (exit 64), got {je:?}"
        );
        assert_eq!(
            je.exit_code(),
            64,
            "config-file invalid profile key exit code must be 64, got {}",
            je.exit_code()
        );
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

    // -----------------------------------------------------------------------
    // AC-006: profile name too long → error message contains "too long" or "max 64"
    //
    // BC-6.1.004 invariant.
    //
    // Pre-implementation Red Gate: ASSERTION ERROR — `validate_profile_name` currently
    // calls `invalid_profile_name()` which emits the generic message
    // "invalid profile name ...; allowed: A-Z a-z 0-9 _ - up to 64 chars; reserved ..."
    // This message does NOT contain "too long" or "max 64" as a distinct substring
    // in a way that differentiates length from charset violations. The assertion fails
    // because neither "too long" nor "max 64" appears in the current error message.
    //
    // Post-implementation: `validate_profile_name` returns a distinct message for the
    // length case, e.g. "Profile name too long (max 64 characters)".
    // -----------------------------------------------------------------------

    /// BC-6.1.004 invariant (AC-006): `validate_profile_name` with a 65-char name
    /// returns an error whose message contains `"too long"` or `"max 64"`.
    /// The message must be DISTINCT from the charset-violation message (AC-007).
    ///
    /// Pre-implementation Red Gate: ASSERTION ERROR — generic message doesn't
    /// contain "too long" or "max 64".
    #[test]
    fn test_validate_profile_name_too_long_message() {
        let long_name = "a".repeat(65);
        let result = validate_profile_name(&long_name);
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("too long") || msg.contains("max 64"),
            "AC-006 (BC-6.1.004 invariant): error for a 65-char profile name must \
             contain 'too long' or 'max 64'. Got: {msg:?}"
        );
    }

    // -----------------------------------------------------------------------
    // AC-007: profile name with space → error message contains "invalid characters"
    //         or "a-z, 0-9"
    //
    // BC-6.1.004 invariant.
    //
    // Pre-implementation Red Gate: ASSERTION ERROR — current generic message
    // "invalid profile name ...; allowed: A-Z a-z 0-9 _ - up to 64 chars; reserved ..."
    // does NOT contain "invalid characters" as a distinct phrase (it says "allowed:" not
    // "invalid characters"). The assertion fails.
    //
    // Post-implementation: `validate_profile_name` returns a distinct message for the
    // charset case, e.g. "Profile name contains invalid characters (use a-z, 0-9, -, _)".
    // -----------------------------------------------------------------------

    /// BC-6.1.004 invariant (AC-007): `validate_profile_name` with a name containing
    /// a space returns an error whose message contains `"invalid characters"` or
    /// `"a-z, 0-9"`. The message must be DISTINCT from the length-violation message
    /// (AC-006). Length is checked first: a 5-char name with a space is NOT too long,
    /// so the charset branch fires.
    ///
    /// Pre-implementation Red Gate: ASSERTION ERROR — generic message doesn't
    /// contain "invalid characters" or "a-z, 0-9".
    #[test]
    fn test_validate_profile_name_with_space_message() {
        // 7 chars (within 64-char limit) with a space — length check passes,
        // charset check fires, distinct message expected.
        let result = validate_profile_name("foo bar");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid characters") || msg.contains("a-z, 0-9"),
            "AC-007 (BC-6.1.004 invariant): error for a profile name with a space must \
             contain 'invalid characters' or 'a-z, 0-9'. Got: {msg:?}"
        );
    }

    // -----------------------------------------------------------------------
    // BC-6.2.017 — JR_CONFIG_DIR debug-only path-isolation seam
    //
    // These tests pin AC-001 and AC-003 from S-WIN-2.
    //
    // Pre-implementation Red Gate: the seam does not exist in global_config_dir()
    // yet. AC-001 will FAIL because global_config_dir() does not read JR_CONFIG_DIR
    // at all — it returns the XDG/home-dir path regardless.
    // AC-003 will FAIL because with no seam, setting JR_CONFIG_DIR="" changes
    // nothing about the returned path — the assertion that PathBuf::from("") is NOT
    // returned trivially passes, but the assertion that the seam is absent (i.e. the
    // function does NOT short-circuit to the env var value) is the load-bearing check
    // for AC-001. Both tests are structured so they require the seam to exist.
    // -----------------------------------------------------------------------

    /// BC-6.2.017 postcondition (debug path) — AC-001.
    ///
    /// In a debug build, `global_config_dir()` must return `PathBuf::from(value)`
    /// when `JR_CONFIG_DIR` is set to a non-empty string. The XDG/home-dir logic
    /// must be bypassed entirely.
    ///
    /// Pre-implementation Red Gate: ASSERTION FAILURE — `global_config_dir()` does
    /// not read `JR_CONFIG_DIR` so it returns the XDG or home-dir path instead of
    /// the seam value.
    #[cfg(debug_assertions)]
    #[test]
    fn test_bc_6_2_017_config_dir_seam_overrides_path() {
        // Use a distinctive path that cannot coincidentally match any XDG or home
        // directory the test environment might have configured.
        let seam_path = "/tmp/jr-seam-test-config-dir-overrides-path";
        // Clear XDG_CONFIG_HOME before entering with_env_var so the non-seam branch
        // cannot accidentally produce the seam path value via a coincidental XDG value.
        // ENV_MUTEX is acquired inside with_env_var; remove_var here is safe because
        // this thread is the only one that will touch the env during this test
        // (with_env_var's mutex acquisition guarantees mutual exclusion).
        let result = with_env_var("JR_CONFIG_DIR", seam_path, || {
            // SAFETY: ENV_MUTEX held by with_env_var for the duration of this closure.
            unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
            global_config_dir()
        });
        assert_eq!(
            result,
            std::path::PathBuf::from(seam_path),
            "AC-001 (BC-6.2.017): global_config_dir() must return the JR_CONFIG_DIR \
             value as-is (no .join(\"jr\") suffix) when the seam is set in a debug build. \
             Got: {}",
            result.display()
        );
    }

    /// BC-6.2.017 EC-1 — AC-003.
    ///
    /// When `JR_CONFIG_DIR` is set to an empty string in a debug build, the seam
    /// is treated as unset. `global_config_dir()` must NOT return `PathBuf::from("")`.
    /// It must proceed to OS-branch logic (XDG / home-dir), returning a non-empty path.
    ///
    /// This test is load-bearing: it pins the `.filter(|s| !s.is_empty())` guard in
    /// `global_config_dir()` (AC-003, BC-6.2.017 EC-1). Without that filter, setting
    /// `JR_CONFIG_DIR=""` would cause the seam to return `PathBuf::from("")` whose
    /// `as_os_str().is_empty()` is true — the `assert_ne!` below would then fail.
    /// Dropping the filter is exactly the mutation this test kills.
    #[cfg(debug_assertions)]
    #[test]
    fn test_bc_6_2_017_empty_config_dir_uses_os_path() {
        let result = with_env_var("JR_CONFIG_DIR", "", global_config_dir);
        assert_ne!(
            result,
            std::path::PathBuf::from(""),
            "AC-003 (BC-6.2.017 EC-1): global_config_dir() must NOT return \
             PathBuf::from(\"\") when JR_CONFIG_DIR is set to the empty string. \
             The empty-string filter must treat it as unset and proceed to OS logic. \
             Got: {}",
            result.display()
        );
        // Additionally assert the path is non-empty (OS logic must have fired).
        assert!(
            !result.as_os_str().is_empty(),
            "AC-003 (BC-6.2.017 EC-1): OS-branch result must be a non-empty path. \
             Got: {}",
            result.display()
        );
    }

    // -------------------------------------------------------------------------
    // BC-6.1.014 — Windows AppData path resolution tests (S-WIN-1)
    // All tests below are `#[cfg(windows)]`-gated. They compile out on macOS/Linux
    // (zero impact on Unix CI) and run only on a Windows runner (S-WIN-5).
    //
    // RED GATE RATIONALE: `global_config_dir()` currently has NO `#[cfg(windows)]`
    // branch — on a Windows build it falls through to the XDG/home_dir Unix path,
    // so `dirs::config_dir()` (= %APPDATA%) is never consulted. Every assertion
    // below would therefore FAIL on a Windows runner against the current code.
    // The implementation in S-WIN-1 adds the `#[cfg(windows)]` branch that makes
    // these tests pass.
    // -------------------------------------------------------------------------

    /// AC-001 / BC-6.1.014 postcondition — on Windows, `global_config_dir()` returns
    /// `dirs::config_dir().join("jr")` which resolves to `%APPDATA%\jr` (Roaming).
    ///
    /// The test cannot call `dirs::config_dir()` and directly inject its return value
    /// (it's an OS call). Instead it verifies the structural postcondition: the returned
    /// path ends with the `jr` component, and its parent equals `dirs::config_dir()`.
    ///
    /// Uses PathBuf component comparison (not string literals with `/`) per
    /// F-WIN-F3-005: on Windows `PathBuf::join` produces `\`-separated paths.
    ///
    /// Traces: BC-6.1.014 postcondition, AC-001.
    #[cfg(windows)]
    #[test]
    fn test_bc_6_1_014_windows_config_dir_uses_appdata() {
        // On Windows, dirs::config_dir() returns Some(%APPDATA% Roaming path).
        // The function under test must return dirs::config_dir().unwrap().join("jr").
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Scrub the debug seam vars so they cannot short-circuit global_config_dir()
        // and perturb the assertion (RB-102).
        // SAFETY: ENV_MUTEX is held for the duration; no concurrent env access occurs.
        unsafe {
            std::env::remove_var("JR_CONFIG_DIR");
            std::env::remove_var("JR_CACHE_DIR");
        }
        let expected_parent = dirs::config_dir()
            .expect("dirs::config_dir() must return Some on a Windows system with a user profile");
        let expected = expected_parent.join("jr");
        let result = global_config_dir();
        assert_eq!(
            result,
            expected,
            "AC-001 (BC-6.1.014): on Windows, global_config_dir() must return \
             dirs::config_dir().join(\"jr\") = %APPDATA%\\jr. \
             Expected: {}, got: {}",
            expected.display(),
            result.display()
        );
        // Structural assertion: must end with component "jr"
        assert!(
            result.ends_with("jr"),
            "AC-001 (BC-6.1.014): path must end with 'jr' component, got: {}",
            result.display()
        );
    }

    /// AC-002 / BC-6.1.014 EC-1 — `config_appdata_fallback` pure helper.
    ///
    /// Exercises the extracted `config_appdata_fallback` helper directly so that
    /// mutations to the production fallback (e.g. dropping `.filter(|s| !s.is_empty())`
    /// or changing `PathBuf::from(".")`) are caught on every platform, not only on a
    /// Windows CI runner.
    ///
    /// The helper is un-gated (no `#[cfg(windows)]`) so this test compiles and runs
    /// on macOS/Linux in CI, genuinely killing the empty-filter/default mutants.
    ///
    /// Traces: BC-6.1.014 EC-1, EC-3, AC-002.
    #[test]
    fn test_bc_6_1_014_appdata_env_fallback() {
        // EC-3: empty string → treated as unset → PathBuf::from(".")
        assert_eq!(
            config_appdata_fallback(Some(String::new())),
            PathBuf::from("."),
            "EC-3: empty APPDATA must yield PathBuf::from(\".\")"
        );

        // EC-1: None (unset APPDATA) → PathBuf::from(".")
        assert_eq!(
            config_appdata_fallback(None),
            PathBuf::from("."),
            "EC-1: None (unset APPDATA) must yield PathBuf::from(\".\")"
        );

        // Happy-path: non-empty value is passed through unchanged
        assert_eq!(
            config_appdata_fallback(Some("C:\\Users\\Alice\\AppData\\Roaming".into())),
            PathBuf::from("C:\\Users\\Alice\\AppData\\Roaming"),
            "non-empty APPDATA must be returned as-is"
        );
    }

    /// AC-003 / BC-6.1.014 invariant — XDG_CONFIG_HOME must NOT affect `global_config_dir()`
    /// on Windows. The `#[cfg(windows)]` branch calls `dirs::config_dir()` unconditionally
    /// and never reads `XDG_CONFIG_HOME`.
    ///
    /// This test sets `XDG_CONFIG_HOME` to a sentinel value and asserts that the returned
    /// path does NOT contain that sentinel — confirming XDG is ignored on the Windows path.
    ///
    /// Uses ENV_MUTEX to serialize env-var mutation.
    ///
    /// Traces: BC-6.1.014 invariant, EC-5, AC-003.
    #[cfg(windows)]
    #[test]
    fn test_bc_6_1_014_xdg_ignored_on_windows() {
        let sentinel = "C:\\SENTINEL_XDG_SHOULD_BE_IGNORED_ON_WINDOWS";
        // with_env_var is #[cfg(debug_assertions)]-gated in config.rs.
        // On a Windows runner in CI we may be in release mode; use ENV_MUTEX directly.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_MUTEX is held for the duration; no concurrent env access occurs.
        // Scrub the debug seam vars so they cannot short-circuit global_config_dir()
        // and perturb the assertion (RB-102).
        unsafe {
            std::env::remove_var("JR_CONFIG_DIR");
            std::env::remove_var("JR_CACHE_DIR");
            std::env::set_var("XDG_CONFIG_HOME", sentinel);
        }
        let result = global_config_dir();
        // SAFETY: ENV_MUTEX still held.
        unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
        drop(_guard);

        // The result must NOT contain the sentinel — XDG is not consulted on Windows.
        assert!(
            !result
                .to_string_lossy()
                .contains("SENTINEL_XDG_SHOULD_BE_IGNORED_ON_WINDOWS"),
            "AC-003 (BC-6.1.014 invariant): XDG_CONFIG_HOME must be ignored on Windows. \
             global_config_dir() must return the APPDATA-derived path, not the XDG sentinel. \
             Got: {}",
            result.display()
        );
        // The result must still end with the 'jr' component (correct APPDATA path).
        assert!(
            result.ends_with("jr"),
            "AC-003 (BC-6.1.014 invariant): path must still end with 'jr' component \
             when XDG_CONFIG_HOME is set. Got: {}",
            result.display()
        );
    }

    // -----------------------------------------------------------------------
    // R1-003 — Unix XDG_CONFIG_HOME branch integration coverage
    //
    // Characterizes the existing Unix production path: when XDG_CONFIG_HOME is
    // set and JR_CONFIG_DIR is absent, `global_config_dir()` resolves through
    // the XDG branch (PathBuf::from(xdg).join("jr")).
    //
    // This is a GREEN test (passes against current code) that pins pre-migration
    // Unix path coverage so a future refactor can't silently drop the XDG branch.
    // NOT `#[cfg(debug_assertions)]` — the XDG branch is NOT gated on debug_assertions
    // (unlike JR_CONFIG_DIR which is), so this test runs in both debug and release
    // builds on Unix.
    // -----------------------------------------------------------------------

    /// R1-003 — On Unix, `global_config_dir()` resolves through `XDG_CONFIG_HOME`
    /// when that variable is set and `JR_CONFIG_DIR` is absent.
    ///
    /// Sets `XDG_CONFIG_HOME` to a tempdir and explicitly removes `JR_CONFIG_DIR`,
    /// then asserts the returned path equals `<tempdir>/jr`.
    ///
    /// This test PASSES against current code — it characterizes existing behavior
    /// and prevents regression of the Unix XDG path during Windows-build refactors.
    ///
    /// Traces: `global_config_dir()` Unix branch (`#[cfg(not(windows))]`); FIX-F5-001 R1-003.
    #[cfg(not(windows))]
    #[test]
    fn test_global_config_dir_resolves_through_xdg_on_unix() {
        let dir = TempDir::new().unwrap();
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        // SAFETY: ENV_MUTEX is held for the duration; no concurrent env access occurs.
        // Explicitly remove JR_CONFIG_DIR so the debug seam cannot short-circuit
        // to a different path (even though JR_CONFIG_DIR is debug-only, removing it
        // is defense-in-depth for all build configurations).
        unsafe {
            std::env::remove_var("JR_CONFIG_DIR");
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
        }

        let result = global_config_dir();

        // SAFETY: ENV_MUTEX still held.
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        drop(_guard);

        let expected = dir.path().join("jr");
        assert_eq!(
            result,
            expected,
            "R1-003: global_config_dir() must resolve through XDG_CONFIG_HOME when \
             set (and JR_CONFIG_DIR is absent). Expected: {}, got: {}",
            expected.display(),
            result.display()
        );
        // Structural invariant: path ends with 'jr' component.
        assert!(
            result.ends_with("jr"),
            "R1-003: resolved path must end with 'jr' component. Got: {}",
            result.display()
        );
    }

    // -----------------------------------------------------------------------
    // M-6 — Unix XDG fallback branch coverage (XDG_CONFIG_HOME unset)
    //
    // Pins the `else` branch of `global_config_dir()` on Unix: when neither
    // XDG_CONFIG_HOME nor JR_CONFIG_DIR is set, the function falls back to
    // `dirs::home_dir().join(".config").join("jr")`.  A refactor that changed
    // the fallback suffix (e.g. to ".cache/jr") would silently break without
    // this pin test.
    // -----------------------------------------------------------------------

    /// M-6 — On Unix, `global_config_dir()` falls back to `~/.config/jr` when
    /// `XDG_CONFIG_HOME` is unset and `JR_CONFIG_DIR` is absent.
    ///
    /// Removes both env vars, calls `global_config_dir()`, and asserts the
    /// returned path ends with `.config/jr`.  The full prefix is `home_dir()`
    /// which varies by user, so only the suffix is checked.
    ///
    /// Traces: `global_config_dir()` Unix else-branch (`#[cfg(not(windows))]`);
    /// FIX-F5-001 M-6.
    #[cfg(not(windows))]
    #[test]
    fn test_global_config_dir_falls_back_to_home_config_on_unix_when_xdg_unset() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        // SAFETY: ENV_MUTEX is held for the duration; no concurrent env access occurs.
        // Remove both overrides so the production else-branch fires.
        // Note: cleanup is not panic-safe (consistent with adjacent R1-003 test);
        // global_config_dir() on a standard system cannot panic.
        unsafe {
            std::env::remove_var("JR_CONFIG_DIR");
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        let result = global_config_dir();
        drop(_guard);

        // The fallback path must end with ".config/jr" (the Unix default).
        // We cannot assert the full path (home dir varies), so check the suffix.
        let result_str = result.to_string_lossy();
        assert!(
            result_str.ends_with("/.config/jr"),
            "M-6: global_config_dir() fallback (XDG_CONFIG_HOME unset) must end \
             with '/.config/jr'. Got: {}",
            result.display()
        );
        // Structural invariant: path ends with 'jr' component.
        assert!(
            result.ends_with("jr"),
            "M-6: resolved path must end with 'jr' component. Got: {}",
            result.display()
        );
    }

    /// S-cycle4-cloud-id-correctness — Two-Step Red Gate TEST (step 2 of 2)
    /// for AC-009 (BC-1.2.054 Postcondition 1, Invariant 1/2; VP-AUTHDX-021):
    /// `Config::base_url()` selects the OAuth gateway URL IFF
    /// `auth_method == Some("oauth")` AND `cloud_id` is present — for the
    /// FULL cross product of `auth_method` in {oauth, api_token,
    /// unset/None} x `cloud_id` in {present, absent}. A "stale" vs
    /// "correct" cloud_id makes no observable difference to this pure,
    /// config-derived function — it is opaque UUID text substituted
    /// verbatim into the gateway URL string — so {present, absent} is the
    /// complete cross product this property needs, not a three-way
    /// {correct, stale, absent} split (the story's own AC-009 text lists
    /// "present-correct/present-stale/absent" as the conceptual states a
    /// human might reason about; this pin covers "present" generically
    /// since the two present-cases are byte-identical from `base_url()`'s
    /// point of view).
    ///
    /// This is a REGRESSION PIN on already-correct, PRE-EXISTING behavior
    /// (ADR-0022 §4) — no `src/config.rs` code change is made by this
    /// story. It must FAIL LOUD if a future change removes the `oauth`
    /// gate from `base_url()`.
    ///
    /// This test currently PASSES against `base_url()`'s existing,
    /// unmodified implementation (confirmed: `cargo test
    /// prop_base_url_selects_gateway_iff_oauth_and_cloud_id_present` is
    /// green today) — it is a regression pin on already-correct code, not a
    /// Red Gate test for new production code. Included here per Task 12 /
    /// AC-009 so the pin exists from this story's first commit onward,
    /// consistent with the "no code change to either function" contract.
    mod proptests_ac_009_base_url_gateway_guard {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(256))]

            #[test]
            fn prop_base_url_selects_gateway_iff_oauth_and_cloud_id_present(
                auth_method in prop_oneof![
                    Just(Some("oauth".to_string())),
                    Just(Some("api_token".to_string())),
                    Just(None::<String>),
                ],
                cloud_id_present in any::<bool>(),
                cloud_id in "[a-f0-9-]{8,36}",
            ) {
                let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
                // SAFETY: ENV_MUTEX held for this whole property test body —
                // base_url() consults JR_BASE_URL first (debug builds only)
                // and must not be short-circuited by a leaking env var from
                // another test.
                unsafe {
                    std::env::remove_var("JR_BASE_URL");
                }

                let mut profiles = std::collections::BTreeMap::new();
                profiles.insert(
                    "sandbox".to_string(),
                    ProfileConfig {
                        url: Some("https://sandbox.atlassian.net".into()),
                        auth_method: auth_method.clone(),
                        cloud_id: if cloud_id_present { Some(cloud_id.clone()) } else { None },
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

                let result = config.base_url().unwrap();
                let expects_gateway = auth_method.as_deref() == Some("oauth") && cloud_id_present;

                if expects_gateway {
                    prop_assert_eq!(
                        &result,
                        &format!("https://api.atlassian.com/ex/jira/{cloud_id}")
                    );
                } else {
                    prop_assert_eq!(&result, &"https://sandbox.atlassian.net".to_string());
                }
            }
        }
    }
}

/// VP-AUTHDX-009 (BC-6.1.015 AC-003): tolerant-reader + round-trip property
/// coverage for `ProfileConfig.env` across arbitrary field combinations.
#[cfg(test)]
mod proptests_env_tag {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        /// Property 1: a TOML profile block with NO `env` key (arbitrary
        /// other fields present or absent) always deserializes to
        /// `env: None`. Covers BC-6.1.015 EC-3 / AC-001 across randomized
        /// sibling-field combinations, not just the single fixed case in
        /// `test_profile_config_env_absent_key_deserializes_to_none`.
        #[test]
        fn prop_profile_config_env_absent_key_always_none(
            url in proptest::option::of("[a-zA-Z0-9.:/-]{0,40}"),
            auth_method in proptest::option::of("[a-z_]{0,20}"),
        ) {
            let mut toml = String::new();
            if let Some(u) = &url {
                toml.push_str(&format!("url = {u:?}\n"));
            }
            if let Some(a) = &auth_method {
                toml.push_str(&format!("auth_method = {a:?}\n"));
            }
            let p: ProfileConfig = toml::from_str(&toml).unwrap();
            prop_assert_eq!(p.env, None);
        }

        /// Property 2: for ANY string `s` (including empty and strings with
        /// control/unicode characters — the `(?s)` flag makes `.` match
        /// every character, including `\n`, so this genuinely covers
        /// newlines and not just other control bytes), constructing
        /// `env: Some(s.clone())` and round-tripping through serialize ->
        /// deserialize returns `Some(s)` unchanged. Covers BC-6.1.015 EC-4
        /// (storage stays verbatim, no validation/mutation at the storage
        /// layer).
        #[test]
        fn prop_profile_config_env_some_round_trips(s in "(?s).*") {
            let p = ProfileConfig {
                env: Some(s.clone()),
                ..ProfileConfig::default()
            };
            let serialized = toml::to_string(&p)
                .expect("ProfileConfig with arbitrary env string must serialize");
            let round_tripped: ProfileConfig = toml::from_str(&serialized)
                .expect("serialized ProfileConfig must deserialize");
            prop_assert_eq!(round_tripped.env, Some(s));
        }

        /// Property 3: `env: None` always round-trips as `None` (never
        /// promoted to `Some("")` or any other value) through a
        /// serialize -> deserialize cycle, for any combination of sibling
        /// `Option<String>` fields.
        #[test]
        fn prop_profile_config_env_none_round_trips_as_none(
            url in proptest::option::of("[a-zA-Z0-9.:/-]{0,40}"),
        ) {
            let p = ProfileConfig {
                url,
                env: None,
                ..ProfileConfig::default()
            };
            let serialized = toml::to_string(&p)
                .expect("ProfileConfig must serialize");
            let round_tripped: ProfileConfig = toml::from_str(&serialized)
                .expect("serialized ProfileConfig must deserialize");
            prop_assert_eq!(round_tripped.env, None);
        }
    }
}
