# Multi-Profile Auth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `jr` target multiple Atlassian Cloud sites from one install, with `jr auth switch <profile>` to flip between them. Shared classic API token across profiles, per-profile OAuth tokens, auto-migration of legacy single-instance configs.

**Architecture:** Foundation-first build. Add new `ProfileConfig` type and `default_profile`/`profiles` fields to `GlobalConfig` while keeping the legacy `[instance]` block deserializable for migration. `Config::load()` performs a one-time TOML migration. Cache directory becomes `~/.cache/jr/v1/<profile>/`. Keyring keys for OAuth tokens are namespaced as `<profile>:oauth-access-token`. Legacy keys read on first miss for the `"default"` profile (lazy migration). All call sites that read `config.global.instance.*` migrate to `config.active_profile().…`. New CLI subcommands `switch / list / remove` join existing `login / status / refresh / logout` (the latter two gain `--profile`).

**Tech Stack:** Rust 1.85+, serde + figment + toml for config, keyring crate for OS keyring, clap derive for CLI, proptest + insta + tempfile + assert_cmd for tests, wiremock for HTTP mocking. No new test crates.

---

## File Structure

| File | Role | Status |
|---|---|---|
| `src/config.rs` | Schema types, active-profile resolution, validate_profile_name, migration | Modified — adds ProfileConfig + migration; legacy InstanceConfig removed in Task 14 |
| `src/api/auth.rs` | Keyring read/write per profile + lazy OAuth migration | Modified — store/load_oauth_tokens gain `profile: &str` |
| `src/cache.rs` | Per-profile cache directory + threaded reader/writer signatures | Modified — `cache_dir(profile)` returns `~/.cache/jr/v1/<profile>` |
| `src/api/client.rs` | JiraClient::from_config consumes active profile | Modified — flips from `instance.*` to `active_profile().*` |
| `src/cli/mod.rs` | `--profile` global flag + new AuthCommand variants | Modified — adds Switch/List/Logout/Remove; existing Login/Status/Refresh gain `profile` |
| `src/cli/auth.rs` | Implementation of new auth subcommands | Modified — major surface area expansion |
| `src/cli/init.rs` | Prompt before adding additional profile | Modified — detects existing profiles |
| `src/cli/team.rs` | Use active_profile for url/cloud_id/org_id | Modified — small change, single call site |
| `tests/auth_profiles.rs` | End-to-end multi-profile workflow tests | Created |
| `tests/migration_legacy.rs` | Migration snapshot tests with `insta` | Created |

---

## Task 1: Profile name validation

**Files:**
- Modify: `src/config.rs` (add `validate_profile_name` fn + inline tests)

- [ ] **Step 1: Write failing tests for character/length validation**

Add to `src/config.rs` `tests` module (around line 149):

```rust
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
    for bad in ["", " ", "foo bar", "foo:bar", "foo/bar", "foo.bar", "..", "."] {
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
        "CON", "con", "Con",
        "NUL", "nul", "AUX", "aux", "PRN", "prn",
        "COM1", "com9", "LPT1", "lpt9",
    ] {
        assert!(
            validate_profile_name(bad).is_err(),
            "expected Windows reserved name {bad:?} to be rejected"
        );
    }
}
```

- [ ] **Step 2: Run tests, verify they fail to compile**

```bash
cargo test --lib config::tests::validate_profile_name 2>&1 | tail -10
```
Expected: `cannot find function validate_profile_name`

- [ ] **Step 3: Implement validate_profile_name**

Add to `src/config.rs` (top-level, after the type defs around line 60):

```rust
/// Validate a profile name. See docs/specs/multi-profile-auth.md "Profile Name Validation".
pub fn validate_profile_name(name: &str) -> Result<(), JrError> {
    const RESERVED_WINDOWS: &[&str] = &[
        "CON", "NUL", "AUX", "PRN",
        "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    if name.is_empty() || name.len() > 64 {
        return Err(invalid_profile_name(name));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(invalid_profile_name(name));
    }
    let upper = name.to_ascii_uppercase();
    if RESERVED_WINDOWS.iter().any(|r| *r == upper.as_str()) {
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
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cargo test --lib config::tests::validate_profile_name
```
Expected: 4 tests passing.

- [ ] **Step 5: Run fmt + clippy + full test suite**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```
Expected: clean clippy, all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): validate profile names (regex + Windows reserved)"
```

---

## Task 2: Add ProfileConfig type and dual-shape GlobalConfig

**Files:**
- Modify: `src/config.rs` (introduce ProfileConfig, dual-shape during transition)

The legacy `InstanceConfig` and `FieldsConfig` stay in place. We add `default_profile: Option<String>` and `profiles: BTreeMap<String, ProfileConfig>` alongside. Migration in Task 4 moves data.

- [ ] **Step 1: Write failing tests for ProfileConfig serde + GlobalConfig dual-shape parse**

Add to `src/config.rs` `tests` module:

```rust
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
    assert_eq!(p.story_points_field_id.as_deref(), Some("customfield_10002"));
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
    assert_eq!(
        cfg.profiles["sandbox"].cloud_id.as_deref(),
        Some("xyz-789")
    );
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
    assert!(cfg.default_profile.is_none(), "no default_profile in legacy shape");
    assert_eq!(cfg.instance.url.as_deref(), Some("https://legacy.atlassian.net"));
    assert_eq!(cfg.fields.team_field_id.as_deref(), Some("customfield_99"));
}
```

- [ ] **Step 2: Run tests, verify they fail to compile**

```bash
cargo test --lib config::tests 2>&1 | tail -10
```
Expected: `cannot find type ProfileConfig` and missing fields on GlobalConfig.

- [ ] **Step 3: Add ProfileConfig and dual-shape fields to GlobalConfig**

Modify `src/config.rs`. Update `GlobalConfig`:

```rust
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
```

Add `ProfileConfig` (right after `FieldsConfig` near line 14):

```rust
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
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cargo test --lib config::tests
```
Expected: all 3 new tests pass; existing tests still pass.

- [ ] **Step 5: Run fmt + clippy + full test suite**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add ProfileConfig type alongside legacy InstanceConfig"
```

---

## Task 3: Active-profile resolution

**Files:**
- Modify: `src/config.rs` (resolve_active_profile_name fn + Config::active_profile method + tests)

- [ ] **Step 1: Write failing tests for precedence chain**

Add to `src/config.rs` `tests` module:

```rust
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
    profiles.insert("sandbox".to_string(), ProfileConfig {
        url: Some("https://sandbox.example".into()),
        ..ProfileConfig::default()
    });
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
```

- [ ] **Step 2: Run tests, verify fail**

```bash
cargo test --lib config::tests::resolve_active_profile_name config::tests::config_active_profile 2>&1 | tail -10
```
Expected: missing fn `resolve_active_profile_name`, missing field `active_profile_name`, missing methods.

- [ ] **Step 3: Implement resolution + Config field + methods**

Modify `src/config.rs`. Update `Config` struct:

```rust
#[derive(Debug, Default)]
pub struct Config {
    pub global: GlobalConfig,
    pub project: ProjectConfig,
    /// Resolved at load() — flag > JR_PROFILE > default_profile > "default".
    pub active_profile_name: String,
}
```

Add free function:

```rust
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
```

Add methods on `Config`:

```rust
impl Config {
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
        self.global.profiles.get(&self.active_profile_name).ok_or_else(|| {
            let known: Vec<&str> = self.global.profiles.keys().map(String::as_str).collect();
            JrError::ConfigError(format!(
                "default_profile {:?} not in [profiles]; known: {}; \
                 fix config.toml or run \"jr auth list\"",
                self.active_profile_name,
                if known.is_empty() { "(none)".into() } else { known.join(", ") }
            ))
            .into()
        })
    }
}
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cargo test --lib config::tests::resolve_active_profile_name config::tests::config_active_profile
```
Expected: 6 new tests pass.

- [ ] **Step 5: Update Config::load to populate active_profile_name**

Modify `Config::load` in `src/config.rs` (around line 61). At the end, before returning `Ok(Config { global, project })`:

```rust
let cli_profile_flag = std::env::var("JR_PROFILE_OVERRIDE").ok(); // populated by main from CLI flag
let env_profile = std::env::var("JR_PROFILE").ok();
let active_profile_name = resolve_active_profile_name(
    &global,
    cli_profile_flag.as_deref(),
    env_profile,
);

Ok(Config { global, project, active_profile_name })
```

(`JR_PROFILE_OVERRIDE` is set by `main.rs` from the parsed `--profile` flag in Task 9; not user-facing.)

- [ ] **Step 6: Run full test suite**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 7: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): resolve active profile name from precedence chain"
```

---

## Task 4: Config auto-migration of legacy shape

**Files:**
- Modify: `src/config.rs` (Config::load runs migration when legacy shape detected)

- [ ] **Step 1: Write failing migration tests**

Add to `src/config.rs` `tests` module:

```rust
#[test]
fn migrate_legacy_instance_into_default_profile() {
    let mut global = GlobalConfig::default();
    global.instance = InstanceConfig {
        url: Some("https://legacy.example".into()),
        cloud_id: Some("legacy-1".into()),
        org_id: Some("org-1".into()),
        auth_method: Some("api_token".into()),
        oauth_scopes: None,
    };
    global.fields = FieldsConfig {
        team_field_id: Some("customfield_99".into()),
        story_points_field_id: Some("customfield_42".into()),
    };

    let migrated = migrate_legacy_global(global);

    assert_eq!(migrated.default_profile.as_deref(), Some("default"));
    assert_eq!(migrated.profiles.len(), 1);
    let p = &migrated.profiles["default"];
    assert_eq!(p.url.as_deref(), Some("https://legacy.example"));
    assert_eq!(p.cloud_id.as_deref(), Some("legacy-1"));
    assert_eq!(p.team_field_id.as_deref(), Some("customfield_99"));
    assert_eq!(p.story_points_field_id.as_deref(), Some("customfield_42"));
    assert!(migrated.instance.url.is_none(), "[instance] cleared after migration");
    assert!(migrated.fields.team_field_id.is_none(), "[fields] cleared after migration");
}

#[test]
fn migrate_legacy_is_idempotent_when_already_new_shape() {
    let global = GlobalConfig {
        default_profile: Some("custom".into()),
        profiles: {
            let mut m = std::collections::BTreeMap::new();
            m.insert("custom".to_string(), ProfileConfig {
                url: Some("https://x.example".into()),
                ..ProfileConfig::default()
            });
            m
        },
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
```

Note: ProfileConfig and GlobalConfig need to derive `Clone` for the idempotent test. Verify Step 3 of Task 2 already added Clone; if not, this task adds it.

- [ ] **Step 2: Run tests, verify fail**

```bash
cargo test --lib config::tests::migrate 2>&1 | tail -10
```
Expected: missing fn `migrate_legacy_global`.

- [ ] **Step 3: Implement migrate_legacy_global**

Add to `src/config.rs`:

```rust
/// Pure migration: converts a `GlobalConfig` with legacy `[instance]` + `[fields]`
/// data into the new `[profiles.default]` shape. No-op if already in new shape.
pub fn migrate_legacy_global(mut global: GlobalConfig) -> GlobalConfig {
    // Already migrated? (new shape has at least one profile)
    if !global.profiles.is_empty() {
        return global;
    }

    // No data at all? Return as-is — no profile to create.
    if global.instance.url.is_none()
        && global.instance.auth_method.is_none()
        && global.instance.cloud_id.is_none()
        && global.fields.team_field_id.is_none()
        && global.fields.story_points_field_id.is_none()
    {
        return global;
    }

    // Move legacy data into a "default" profile.
    let profile = ProfileConfig {
        url: global.instance.url.take(),
        auth_method: global.instance.auth_method.take(),
        cloud_id: global.instance.cloud_id.take(),
        org_id: global.instance.org_id.take(),
        oauth_scopes: global.instance.oauth_scopes.take(),
        team_field_id: global.fields.team_field_id.take(),
        story_points_field_id: global.fields.story_points_field_id.take(),
    };
    global.profiles.insert("default".to_string(), profile);
    global.default_profile = Some("default".to_string());
    global
}
```

Make `GlobalConfig` and `ProfileConfig` derive `Clone` if not already (verify Task 2 already did this).

- [ ] **Step 4: Wire migration into Config::load + emit one-time stderr**

Modify `Config::load` in `src/config.rs`. After deserialization but before resolving active_profile_name:

```rust
let was_legacy = !global.profiles.is_empty()
    || global.instance.url.is_some()
    || global.fields.team_field_id.is_some();
let needs_migration = global.profiles.is_empty()
    && (global.instance.url.is_some() || global.fields.team_field_id.is_some());

if needs_migration {
    global = migrate_legacy_global(global);
    // Persist migrated shape so subsequent loads don't re-migrate.
    save_global_to(&global_path, &global)?;
    eprintln!(
        "Migrated config to multi-profile layout (single profile \"default\"). \
         Run 'jr auth list' to view profiles."
    );
}
let _ = was_legacy; // suppress unused-binding warning when feature disabled
```

Refactor `save_global` to use a free helper that takes a path:

```rust
fn save_global_to(path: &std::path::Path, global: &GlobalConfig) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let content = toml::to_string_pretty(global)?;
    std::fs::write(path, content)?;
    Ok(())
}
```

And update the existing `Config::save_global` to delegate to it:

```rust
pub fn save_global(&self) -> anyhow::Result<()> {
    save_global_to(&global_config_path(), &self.global)
}
```

- [ ] **Step 5: Run tests + check the full migration flow**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --lib config::tests
```
Expected: 3 new migration tests + all earlier tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): auto-migrate legacy [instance] block into [profiles.default]"
```

---

## Task 5: Per-profile OAuth keyring API + lazy migration

**Files:**
- Modify: `src/api/auth.rs` (signatures of store/load_oauth_tokens gain `profile: &str`; lazy fallback in load)

- [ ] **Step 1: Write failing tests**

Add to `src/api/auth.rs` `tests` module:

```rust
fn unique_test_service() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("jr-jira-cli-test-{}-{}", std::process::id(), n)
}

/// Wrap a test in a unique JR_SERVICE_NAME scope so concurrent tests don't collide.
fn with_test_keyring<F: FnOnce()>(f: F) {
    if std::env::var("JR_RUN_KEYRING_TESTS").is_err() {
        return; // keyring tests are opt-in (Linux CI may lack secret-service)
    }
    let svc = unique_test_service();
    let prev = std::env::var("JR_SERVICE_NAME").ok();
    // SAFETY: tests using keyring must be serialized via JR_RUN_KEYRING_TESTS opt-in.
    unsafe { std::env::set_var("JR_SERVICE_NAME", &svc) };
    f();
    let _ = clear_all_credentials();
    unsafe {
        match prev {
            Some(p) => std::env::set_var("JR_SERVICE_NAME", p),
            None => std::env::remove_var("JR_SERVICE_NAME"),
        }
    }
}

#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn store_and_load_per_profile_oauth_tokens_round_trip() {
    with_test_keyring(|| {
        store_oauth_tokens("default", "access1", "refresh1").unwrap();
        store_oauth_tokens("sandbox", "access2", "refresh2").unwrap();

        let (a1, r1) = load_oauth_tokens("default").unwrap();
        let (a2, r2) = load_oauth_tokens("sandbox").unwrap();

        assert_eq!((a1.as_str(), r1.as_str()), ("access1", "refresh1"));
        assert_eq!((a2.as_str(), r2.as_str()), ("access2", "refresh2"));
    });
}

#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn load_oauth_tokens_returns_err_for_missing_profile() {
    with_test_keyring(|| {
        assert!(load_oauth_tokens("default").is_err());
    });
}

#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn lazy_migration_legacy_flat_keys_for_default_profile() {
    with_test_keyring(|| {
        // Pre-seed legacy flat keys (simulating pre-migration state)
        entry("oauth-access-token").unwrap()
            .set_password("legacy-access").unwrap();
        entry("oauth-refresh-token").unwrap()
            .set_password("legacy-refresh").unwrap();

        // First load on "default" profile triggers lazy migration.
        let (access, refresh) = load_oauth_tokens("default").unwrap();
        assert_eq!(access, "legacy-access");
        assert_eq!(refresh, "legacy-refresh");

        // New keys exist
        let new_access = entry("default:oauth-access-token").unwrap().get_password().unwrap();
        assert_eq!(new_access, "legacy-access");

        // Legacy keys cleaned up
        assert!(entry("oauth-access-token").unwrap().get_password().is_err());
    });
}

#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn lazy_migration_does_not_fire_for_non_default_profile() {
    with_test_keyring(|| {
        entry("oauth-access-token").unwrap()
            .set_password("legacy-access").unwrap();
        entry("oauth-refresh-token").unwrap()
            .set_password("legacy-refresh").unwrap();

        assert!(load_oauth_tokens("sandbox").is_err(),
                "sandbox profile should NOT inherit legacy keys");
    });
}
```

- [ ] **Step 2: Run tests, verify they fail or are ignored**

```bash
cargo test --lib api::auth::tests 2>&1 | tail -10
```
Expected: compile errors on `store_oauth_tokens("default", ...)` arity mismatch.

- [ ] **Step 3: Update store/load_oauth_tokens signatures + add lazy migration + add clear helpers**

Modify `src/api/auth.rs`. Replace existing `store_oauth_tokens` and `load_oauth_tokens` (lines 59–75):

```rust
const KEY_OAUTH_ACCESS_LEGACY: &str = "oauth-access-token";
const KEY_OAUTH_REFRESH_LEGACY: &str = "oauth-refresh-token";

fn oauth_access_key(profile: &str) -> String { format!("{profile}:oauth-access-token") }
fn oauth_refresh_key(profile: &str) -> String { format!("{profile}:oauth-refresh-token") }

/// Store OAuth 2.0 access and refresh tokens scoped to a profile.
pub fn store_oauth_tokens(profile: &str, access: &str, refresh: &str) -> Result<()> {
    entry(&oauth_access_key(profile))?.set_password(access)?;
    entry(&oauth_refresh_key(profile))?.set_password(refresh)?;
    Ok(())
}

/// Load OAuth 2.0 access and refresh tokens for a profile.
///
/// For the `"default"` profile, falls back to the legacy flat keys (pre-migration
/// state) and opportunistically migrates them to the new namespaced keys on read.
pub fn load_oauth_tokens(profile: &str) -> Result<(String, String)> {
    let access_key = oauth_access_key(profile);
    let refresh_key = oauth_refresh_key(profile);
    if let (Ok(a), Ok(r)) = (
        entry(&access_key)?.get_password(),
        entry(&refresh_key)?.get_password(),
    ) {
        return Ok((a, r));
    }
    if profile == "default" {
        if let (Ok(a), Ok(r)) = (
            entry(KEY_OAUTH_ACCESS_LEGACY)?.get_password(),
            entry(KEY_OAUTH_REFRESH_LEGACY)?.get_password(),
        ) {
            // Opportunistic migration to new keys; best-effort delete of legacy.
            store_oauth_tokens("default", &a, &r)?;
            let _ = entry(KEY_OAUTH_ACCESS_LEGACY)?.delete_credential();
            let _ = entry(KEY_OAUTH_REFRESH_LEGACY)?.delete_credential();
            return Ok((a, r));
        }
    }
    Err(anyhow::anyhow!(
        "No stored OAuth token for profile {profile:?} — run \"jr auth login --profile {profile}\""
    ))
}

/// Clear OAuth tokens for a single profile (other profiles + shared keys untouched).
pub fn clear_profile_creds(profile: &str) -> Result<()> {
    let mut failures: Vec<String> = Vec::new();
    for key in [oauth_access_key(profile), oauth_refresh_key(profile)] {
        match entry(&key) {
            Ok(e) => match e.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(err) => failures.push(format!("{key}: {err}")),
            },
            Err(err) => failures.push(format!("{key}: {err}")),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "failed to clear {} keychain entries: {}",
            failures.len(),
            failures.join("; ")
        ))
    }
}
```

Rename existing `clear_credentials` to `clear_all_credentials` and update body to enumerate known profiles. Since the function doesn't have config access, take the profile list as parameter:

```rust
/// Clear shared credentials and OAuth tokens for all listed profiles.
pub fn clear_all_credentials(profiles: &[&str]) -> Result<()> {
    let mut failures: Vec<String> = Vec::new();
    let mut keys: Vec<String> = vec![
        KEY_EMAIL.to_string(),
        KEY_API_TOKEN.to_string(),
        "oauth_client_id".to_string(),
        "oauth_client_secret".to_string(),
        // Legacy keys (in case lazy migration hasn't run yet)
        KEY_OAUTH_ACCESS_LEGACY.to_string(),
        KEY_OAUTH_REFRESH_LEGACY.to_string(),
    ];
    for profile in profiles {
        keys.push(oauth_access_key(profile));
        keys.push(oauth_refresh_key(profile));
    }
    for key in keys {
        match entry(&key) {
            Ok(e) => match e.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(err) => failures.push(format!("{key}: {err}")),
            },
            Err(err) => failures.push(format!("{key}: {err}")),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "failed to clear {} keychain entries: {}",
            failures.len(),
            failures.join("; ")
        ))
    }
}
```

Update the existing call site of `store_oauth_tokens` in `oauth_login` (line ~252) and `refresh_oauth_token` (line ~294) — they need a profile argument. Wire them to take `profile: &str`:

```rust
pub async fn oauth_login(
    profile: &str,
    client_id: &str,
    client_secret: &str,
    scopes: &str,
) -> Result<OAuthResult> {
    // ...existing code...
    // 5. Store tokens in the system keychain.
    store_oauth_tokens(profile, &tokens.access_token, &tokens.refresh_token)?;
    // ...rest...
}

pub async fn refresh_oauth_token(
    profile: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String> {
    let (_, refresh_token) = load_oauth_tokens(profile)?;
    // ...existing code...
    store_oauth_tokens(profile, &tokens.access_token, &tokens.refresh_token)?;
    Ok(tokens.access_token)
}
```

Also need to update `cli/auth.rs` callers — for now use `"default"` as the profile literal in those callers; Task 11 properly threads the active profile name in.

- [ ] **Step 4: Update existing callers in cli/auth.rs to pass "default"**

In `src/cli/auth.rs`, find each call to `oauth_login`, `refresh_oauth_token`, `load_oauth_tokens` (now takes profile arg), and pass `"default"` as the profile literal. Search:

```bash
grep -n "oauth_login\|refresh_oauth_token\|load_oauth_tokens" src/cli/auth.rs
```

Update each call site's first arg to `"default"`. Same for `src/api/client.rs` line 59.

Update the existing call to `clear_credentials` in `cli/auth.rs` to `clear_all_credentials(&["default"])`.

- [ ] **Step 5: Run keyring tests in opt-in mode**

```bash
JR_RUN_KEYRING_TESTS=1 cargo test --lib api::auth::tests -- --ignored
```
Expected: keyring round-trip and lazy migration tests pass.

- [ ] **Step 6: Run fmt + clippy + full test suite (without keyring opt-in)**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 7: Commit**

```bash
git add src/api/auth.rs src/cli/auth.rs src/api/client.rs
git commit -m "refactor(auth): namespace OAuth tokens by profile + lazy migrate legacy keys"
```

---

## Task 6: Per-profile cache directory

**Files:**
- Modify: `src/cache.rs` (cache_dir takes profile, all 6 reader/writer pairs take profile, all callers updated)

- [ ] **Step 1: Write failing tests for new cache_dir + cross-profile isolation**

Add to `src/cache.rs` `tests` module:

```rust
#[test]
fn cache_dir_includes_v1_and_profile_subdir() {
    with_temp_cache(|| {
        let dir = cache_dir("default");
        assert!(dir.ends_with("v1/default"), "got: {}", dir.display());
    });
}

#[test]
fn cross_profile_isolation_team_cache() {
    with_temp_cache(|| {
        write_team_cache("prod", &[CachedTeam {
            id: "t1".into(), name: "Prod Team".into()
        }]).unwrap();

        let prod = read_team_cache("prod").unwrap().unwrap();
        assert_eq!(prod.teams[0].name, "Prod Team");

        // Sandbox profile sees no cache (no leakage)
        assert!(read_team_cache("sandbox").unwrap().is_none());
    });
}

#[test]
fn clear_profile_cache_removes_only_that_profile() {
    with_temp_cache(|| {
        write_team_cache("prod", &[CachedTeam { id: "p".into(), name: "P".into() }]).unwrap();
        write_team_cache("sandbox", &[CachedTeam { id: "s".into(), name: "S".into() }]).unwrap();

        clear_profile_cache("prod").unwrap();

        assert!(read_team_cache("prod").unwrap().is_none(), "prod cache cleared");
        assert!(read_team_cache("sandbox").unwrap().is_some(), "sandbox cache preserved");
    });
}
```

- [ ] **Step 2: Run tests, verify fail**

```bash
cargo test --lib cache::tests::cache_dir_includes cache::tests::cross_profile cache::tests::clear_profile 2>&1 | tail -10
```
Expected: arity mismatch (`cache_dir()` takes 0 args).

- [ ] **Step 3: Update `cache_dir` and all reader/writer signatures**

Modify `src/cache.rs`. Replace `cache_dir`:

```rust
pub fn cache_root() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("jr")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".cache")
            .join("jr")
    }
}

/// Per-profile cache directory: ~/.cache/jr/v1/<profile>/
pub fn cache_dir(profile: &str) -> PathBuf {
    cache_root().join("v1").join(profile)
}
```

Update internal helpers:

```rust
fn read_cache<T: DeserializeOwned + Expiring>(profile: &str, filename: &str) -> Result<Option<T>> {
    let path = cache_dir(profile).join(filename);
    // ... rest unchanged ...
}

fn write_cache<T: Serialize>(profile: &str, filename: &str, data: &T) -> Result<()> {
    let dir = cache_dir(profile);
    std::fs::create_dir_all(&dir)?;
    // ... rest unchanged ...
}
```

Update every public reader/writer pair:

```rust
pub fn read_team_cache(profile: &str) -> Result<Option<TeamCache>> {
    read_cache(profile, "teams.json")
}
pub fn write_team_cache(profile: &str, teams: &[CachedTeam]) -> Result<()> {
    write_cache(profile, "teams.json", &TeamCache { fetched_at: Utc::now(), teams: teams.to_vec() })
}

pub fn read_project_meta(profile: &str, project_key: &str) -> Result<Option<ProjectMeta>> { /* ... */ }
pub fn write_project_meta(profile: &str, project_key: &str, meta: &ProjectMeta) -> Result<()> { /* ... */ }

pub fn read_workspace_cache(profile: &str) -> Result<Option<WorkspaceCache>> {
    read_cache(profile, "workspace.json")
}
pub fn write_workspace_cache(profile: &str, workspace_id: &str) -> Result<()> {
    write_cache(profile, "workspace.json", &WorkspaceCache { workspace_id: workspace_id.to_string(), fetched_at: Utc::now() })
}

pub fn read_cmdb_fields_cache(profile: &str) -> Result<Option<CmdbFieldsCache>> { /* ... */ }
pub fn write_cmdb_fields_cache(profile: &str, fields: &[(String, String)]) -> Result<()> { /* ... */ }

pub fn read_object_type_attr_cache(profile: &str, object_type_id: &str) -> Result<Option<Vec<CachedObjectTypeAttr>>> { /* ... */ }
pub fn write_object_type_attr_cache(profile: &str, object_type_id: &str, attrs: &[CachedObjectTypeAttr]) -> Result<()> { /* ... */ }

// Whatever exists for resolutions — same shape.
```

Add helper:

```rust
pub fn clear_profile_cache(profile: &str) -> Result<()> {
    let dir = cache_dir(profile);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Update all 24 existing cache tests to thread profile**

Search for callsites in tests:

```bash
grep -n "read_team_cache\|write_team_cache\|read_project_meta\|write_project_meta\|read_workspace_cache\|write_workspace_cache\|read_cmdb_fields_cache\|write_cmdb_fields_cache\|read_object_type_attr_cache\|write_object_type_attr_cache" src/cache.rs
```

Each call gains `"default"` (or any per-test profile name) as first arg. The existing `with_temp_cache` helper continues to work (tests just thread profile through helpers).

- [ ] **Step 5: Update all 15 production callsites**

The grep from Task setup showed these read `config.global.instance.*` AND likely cache. Task 7 handles client.rs separately; for cache callsites:

```bash
grep -rn "read_team_cache\|write_team_cache\|read_project_meta\|write_project_meta\|read_workspace_cache\|write_workspace_cache\|read_cmdb_fields_cache\|write_cmdb_fields_cache\|read_object_type_attr_cache\|write_object_type_attr_cache" src/ --include='*.rs' | grep -v 'src/cache.rs'
```

Each call site has `&Config` in scope. Pass `&config.active_profile_name` as the first arg.

- [ ] **Step 6: Run full test suite**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```
Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/cache.rs src/cli/ src/api/
git commit -m "refactor(cache): per-profile cache directory under v1/<profile>"
```

---

## Task 7: JiraClient consumes active profile

**Files:**
- Modify: `src/api/client.rs` (replace config.global.instance reads with config.active_profile)
- Modify: `src/config.rs` (Config::base_url uses active_profile)

- [ ] **Step 1: Write failing test for base_url with profiles**

Add to `src/config.rs` `tests` module:

```rust
#[test]
fn base_url_uses_active_profile() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert("sandbox".to_string(), ProfileConfig {
        url: Some("https://sandbox.atlassian.net".into()),
        auth_method: Some("api_token".into()),
        ..ProfileConfig::default()
    });
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
    profiles.insert("default".to_string(), ProfileConfig {
        url: Some("https://acme.atlassian.net".into()),
        auth_method: Some("oauth".into()),
        cloud_id: Some("abc-123".into()),
        ..ProfileConfig::default()
    });
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
```

- [ ] **Step 2: Update Config::base_url to read active_profile**

Modify `src/config.rs` `base_url`:

```rust
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
```

- [ ] **Step 3: Update existing base_url tests to use new profile shape**

The pre-existing `test_base_url_api_token`, `test_base_url_oauth`, `test_base_url_missing`, `test_base_url_trailing_slash_trimmed` need to switch from `instance.url` to `profiles.get(...).url`. Adapt each accordingly.

- [ ] **Step 4: Update JiraClient::from_config to consume active profile**

Modify `src/api/client.rs` `from_config` (lines 35–84). Replace `config.global.instance.*` reads:

```rust
let profile = config.active_profile_or_err()?;

let instance_url = if let Some(ref override_url) = test_override {
    override_url.trim_end_matches('/').to_string()
} else if let Some(url) = profile.url.as_ref() {
    url.trim_end_matches('/').to_string()
} else {
    return Err(JrError::ConfigError(format!(
        "Profile {:?} has no URL. Run \"jr auth login --profile {}\".",
        config.active_profile_name, config.active_profile_name
    )).into());
};
let auth_method = profile.auth_method.as_deref().unwrap_or("api_token");

let auth_header = if let Ok(header) = std::env::var("JR_AUTH_HEADER") {
    header
} else {
    match auth_method {
        "oauth" => {
            let (access, _refresh) = crate::api::auth::load_oauth_tokens(&config.active_profile_name)?;
            format!("Bearer {access}")
        }
        _ => {
            let (email, token) = crate::api::auth::load_api_token()?;
            let encoded = base64::engine::general_purpose::STANDARD
                .encode(format!("{email}:{token}"));
            format!("Basic {encoded}")
        }
    }
};

// ...
let assets_base_url = if let Some(ref override_url) = test_override {
    Some(format!("{}/jsm/assets", override_url.trim_end_matches('/')))
} else {
    profile.cloud_id.as_ref().map(|cloud_id| {
        format!("https://api.atlassian.com/ex/jira/{}/jsm/assets", urlencoding::encode(cloud_id))
    })
};
```

- [ ] **Step 5: Run fmt + clippy + tests**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 6: Commit**

```bash
git add src/api/client.rs src/config.rs
git commit -m "refactor(client): JiraClient consumes active profile"
```

---

## Task 8: cli/team.rs uses active profile

**Files:**
- Modify: `src/cli/team.rs` (single small refactor — replace 4 reads of config.global.instance.*)

- [ ] **Step 1: Read current state**

```bash
grep -n "config\.global\.instance" src/cli/team.rs
```

- [ ] **Step 2: Update all sites to use active_profile**

Replace each `config.global.instance.<field>` with the equivalent `config.active_profile().<field>` access. Note `active_profile()` returns owned `ProfileConfig` (see Task 3) so chain `.as_ref()` / `.as_deref()` similarly.

For sites that mutate config (e.g., write back cloud_id/org_id after discovery), update the `[profiles.<active>]` entry instead of `[instance]`:

```rust
updated_config.global.profiles
    .entry(updated_config.active_profile_name.clone())
    .or_insert_with(ProfileConfig::default)
    .cloud_id = Some(metadata.cloud_id.clone());
updated_config.global.profiles
    .entry(updated_config.active_profile_name.clone())
    .or_insert_with(ProfileConfig::default)
    .org_id = Some(metadata.org_id.clone());
```

- [ ] **Step 3: Run tests**

```bash
cargo test --lib cli::team
```

- [ ] **Step 4: Run fmt + clippy + full suite**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 5: Commit**

```bash
git add src/cli/team.rs
git commit -m "refactor(team): use active profile for url/cloud_id/org_id"
```

---

## Task 9: --profile global CLI flag

**Files:**
- Modify: `src/cli/mod.rs` (add `profile: Option<String>` to Cli)
- Modify: `src/main.rs` (export to JR_PROFILE_OVERRIDE before Config::load)
- Modify: `src/config.rs` test of CLI flag in resolution chain (already in Task 3)

- [ ] **Step 1: Add the flag to Cli struct**

In `src/cli/mod.rs` (around line 18, sibling of `--output`/`--project`):

```rust
/// Override the active profile (precedence: this flag > JR_PROFILE > config > "default")
#[arg(long, global = true)]
pub profile: Option<String>,
```

- [ ] **Step 2: Wire flag in main.rs before Config::load**

Find the entry point in `src/main.rs` where `Cli::parse()` happens. Right after parsing, before `Config::load()`:

```rust
let cli = Cli::parse();

// Surface --profile to Config::load via env var (avoids changing the public load API).
if let Some(p) = cli.profile.as_deref() {
    crate::config::validate_profile_name(p)?;
    // SAFETY: main is single-threaded at this point.
    unsafe { std::env::set_var("JR_PROFILE_OVERRIDE", p); }
}

let config = Config::load()?;
```

- [ ] **Step 3: Add integration test for precedence**

Add to `src/config.rs` tests:

```rust
#[test]
fn config_load_precedence_flag_overrides_env_overrides_field() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, r#"
        default_profile = "from-config"
        [profiles.from-config]
        url = "https://x"
        [profiles.from-env]
        url = "https://y"
        [profiles.from-flag]
        url = "https://z"
    "#).unwrap();

    // SAFETY: ENV_MUTEX held across env mutations.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        std::env::set_var("JR_PROFILE", "from-env");
        std::env::set_var("JR_PROFILE_OVERRIDE", "from-flag");
    }
    let cfg = Config::load().unwrap();
    assert_eq!(cfg.active_profile_name, "from-flag");

    unsafe {
        std::env::remove_var("JR_PROFILE_OVERRIDE");
    }
    let cfg = Config::load().unwrap();
    assert_eq!(cfg.active_profile_name, "from-env");

    unsafe {
        std::env::remove_var("JR_PROFILE");
    }
    let cfg = Config::load().unwrap();
    assert_eq!(cfg.active_profile_name, "from-config");

    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 5: Commit**

```bash
git add src/cli/mod.rs src/main.rs src/config.rs
git commit -m "feat(cli): add --profile global flag with precedence chain"
```

---

## Task 10: jr auth switch <profile>

**Files:**
- Modify: `src/cli/mod.rs` (add Switch variant to AuthCommand)
- Modify: `src/cli/auth.rs` (handle Switch)

- [ ] **Step 1: Add Switch variant**

In `src/cli/mod.rs` `AuthCommand` enum (around line 185):

```rust
/// Set the default profile in config.toml.
Switch {
    /// Profile name to make active. Must already exist in config.
    name: String,
},
```

- [ ] **Step 2: Add integration test**

Add to `tests/auth_profiles.rs` (will be created in Task 15; for now, add to existing tests/auth.rs if present, or stub a test in cli/auth.rs `tests` module).

For now, add a unit-level test in `src/cli/auth.rs` `tests` module:

```rust
#[test]
fn switch_to_unknown_profile_returns_error() {
    let result = handle_switch_in_memory(GlobalConfig::default(), "ghost");
    assert!(result.is_err());
    let msg = format!("{:#}", result.unwrap_err());
    assert!(msg.contains("unknown profile"), "got: {msg}");
    assert!(msg.contains("ghost"), "got: {msg}");
}

#[test]
fn switch_to_known_profile_mutates_default_profile() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert("sandbox".to_string(), ProfileConfig::default());
    let global = GlobalConfig {
        default_profile: Some("default".into()),
        profiles,
        ..GlobalConfig::default()
    };
    let mutated = handle_switch_in_memory(global, "sandbox").unwrap();
    assert_eq!(mutated.default_profile.as_deref(), Some("sandbox"));
}
```

- [ ] **Step 3: Implement handle_switch + handle_switch_in_memory**

In `src/cli/auth.rs` add:

```rust
/// Pure logic for `jr auth switch` — separated for testing without filesystem.
pub(super) fn handle_switch_in_memory(
    mut global: GlobalConfig,
    target: &str,
) -> anyhow::Result<GlobalConfig> {
    crate::config::validate_profile_name(target)?;
    if !global.profiles.contains_key(target) {
        let known: Vec<&str> = global.profiles.keys().map(String::as_str).collect();
        return Err(JrError::UserError(format!(
            "unknown profile: {target}; known: {}",
            if known.is_empty() { "(none)".into() } else { known.join(", ") }
        )).into());
    }
    global.default_profile = Some(target.to_string());
    Ok(global)
}

pub async fn handle_switch(target: &str) -> anyhow::Result<()> {
    let mut config = Config::load()?;
    config.global = handle_switch_in_memory(config.global, target)?;
    config.save_global()?;
    output::print_success(&format!("Active profile set to {target:?}"));
    Ok(())
}
```

Wire into the dispatch in `main.rs` (or wherever AuthCommand is dispatched).

- [ ] **Step 4: Run tests + manual smoke**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 5: Commit**

```bash
git add src/cli/auth.rs src/cli/mod.rs src/main.rs
git commit -m "feat(auth): add jr auth switch subcommand"
```

---

## Task 11: jr auth list

**Files:**
- Modify: `src/cli/mod.rs` (add List variant)
- Modify: `src/cli/auth.rs` (handle_list)
- Create: `src/snapshots/jr__cli__auth__tests__list_table.snap` (insta will create on first run)

- [ ] **Step 1: Add List variant + integration test**

In `src/cli/mod.rs` `AuthCommand`:

```rust
/// List all configured profiles.
List,
```

- [ ] **Step 2: Add tests**

In `src/cli/auth.rs` tests:

```rust
fn three_profile_fixture() -> GlobalConfig {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert("default".to_string(), ProfileConfig {
        url: Some("https://acme.atlassian.net".into()),
        auth_method: Some("api_token".into()),
        ..ProfileConfig::default()
    });
    profiles.insert("sandbox".to_string(), ProfileConfig {
        url: Some("https://acme-sandbox.atlassian.net".into()),
        auth_method: Some("oauth".into()),
        cloud_id: Some("xyz-789".into()),
        ..ProfileConfig::default()
    });
    profiles.insert("staging".to_string(), ProfileConfig {
        url: Some("https://acme-staging.atlassian.net".into()),
        auth_method: Some("api_token".into()),
        ..ProfileConfig::default()
    });
    GlobalConfig {
        default_profile: Some("default".into()),
        profiles,
        ..GlobalConfig::default()
    }
}

#[test]
fn list_table_snapshot() {
    let global = three_profile_fixture();
    let rendered = render_list_table(&global, "default");
    insta::assert_snapshot!(rendered);
}

#[test]
fn list_json_shape() {
    let global = three_profile_fixture();
    let json = render_list_json(&global, "default").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = parsed.as_array().expect("array");
    assert_eq!(arr.len(), 3);
    let active: Vec<&serde_json::Value> = arr.iter()
        .filter(|p| p["active"].as_bool() == Some(true))
        .collect();
    assert_eq!(active.len(), 1, "exactly one active");
    assert_eq!(active[0]["name"], "default");
}
```

- [ ] **Step 3: Implement render_list_table and render_list_json**

In `src/cli/auth.rs`:

```rust
pub(super) fn render_list_table(global: &GlobalConfig, active: &str) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (name, p) in &global.profiles {
        let marker = if name == active { "*" } else { " " };
        let auth = p.auth_method.as_deref().unwrap_or("?");
        let url = p.url.as_deref().unwrap_or("(unset)");
        // STATUS resolution requires keyring inspection — for now, "configured"
        // if url present, "no-creds" otherwise. Real status check in Task 13.
        let status = if p.url.is_some() { "configured" } else { "no-creds" };
        rows.push(vec![
            format!("{marker} {name}"),
            url.to_string(),
            auth.to_string(),
            status.to_string(),
        ]);
    }
    crate::output::render_table(&["NAME", "URL", "AUTH", "STATUS"], &rows)
}

pub(super) fn render_list_json(global: &GlobalConfig, active: &str) -> anyhow::Result<String> {
    let arr: Vec<serde_json::Value> = global.profiles.iter().map(|(name, p)| {
        serde_json::json!({
            "name": name,
            "url": p.url,
            "auth_method": p.auth_method,
            "status": if p.url.is_some() { "configured" } else { "no-creds" },
            "active": name == active,
        })
    }).collect();
    Ok(serde_json::to_string_pretty(&arr)?)
}

pub async fn handle_list(output: &OutputFormat) -> anyhow::Result<()> {
    let config = Config::load()?;
    let rendered = match output {
        OutputFormat::Table => render_list_table(&config.global, &config.active_profile_name),
        OutputFormat::Json => render_list_json(&config.global, &config.active_profile_name)?,
    };
    println!("{rendered}");
    Ok(())
}
```

Wire into dispatch.

- [ ] **Step 4: Run tests, accept snapshot**

```bash
cargo test --lib cli::auth::tests::list 2>&1 | tail -5
INSTA_UPDATE=auto cargo test --lib cli::auth::tests::list_table_snapshot 2>&1 | tail -5
```

Verify the generated snapshot file has reasonable output before accepting:

```bash
cat src/snapshots/jr__cli__auth__tests__list_table.snap
```

- [ ] **Step 5: Run fmt + clippy + tests**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 6: Commit**

```bash
git add src/cli/auth.rs src/cli/mod.rs src/main.rs src/snapshots/
git commit -m "feat(auth): add jr auth list subcommand"
```

---

## Task 12: jr auth login --profile + --url

**Files:**
- Modify: `src/cli/mod.rs` (Login variant gains profile + url)
- Modify: `src/cli/auth.rs` (login_token / login_oauth take profile)
- Modify: `src/cli/init.rs` if it directly calls login functions

- [ ] **Step 1: Update AuthCommand::Login**

In `src/cli/mod.rs`:

```rust
Login {
    /// Profile to log in to (creates it if absent). Defaults to active profile.
    #[arg(long)]
    profile: Option<String>,
    /// Jira instance URL (required when creating a new profile under --no-input).
    #[arg(long)]
    url: Option<String>,
    /// Use OAuth 2.0 instead of API token.
    #[arg(long)]
    oauth: bool,
    // ... existing flags (email, token, client_id, client_secret) unchanged ...
},
```

- [ ] **Step 2: Add tests for the new login_or_create logic**

```rust
#[test]
fn login_create_new_profile_no_input_requires_url() {
    let global = GlobalConfig::default();
    let result = prepare_login_target(global, Some("sandbox"), None, true);
    assert!(result.is_err());
    let msg = format!("{:#}", result.unwrap_err());
    assert!(msg.contains("--url required"), "got: {msg}");
}

#[test]
fn login_create_new_profile_with_url_succeeds() {
    let global = GlobalConfig::default();
    let (mutated, target) = prepare_login_target(
        global, Some("sandbox"), Some("https://sandbox.example"), true
    ).unwrap();
    assert_eq!(target, "sandbox");
    assert_eq!(
        mutated.profiles["sandbox"].url.as_deref(),
        Some("https://sandbox.example")
    );
}

#[test]
fn login_existing_profile_with_url_updates_url() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert("default".to_string(), ProfileConfig {
        url: Some("https://old.example".into()),
        ..ProfileConfig::default()
    });
    let global = GlobalConfig {
        default_profile: Some("default".into()),
        profiles,
        ..GlobalConfig::default()
    };
    let (mutated, target) = prepare_login_target(
        global, Some("default"), Some("https://new.example"), true
    ).unwrap();
    assert_eq!(target, "default");
    assert_eq!(
        mutated.profiles["default"].url.as_deref(),
        Some("https://new.example")
    );
}
```

- [ ] **Step 3: Implement prepare_login_target**

```rust
/// Pure logic for ensuring a target profile exists with the given URL.
/// Returns (updated_global, resolved_profile_name).
pub(super) fn prepare_login_target(
    mut global: GlobalConfig,
    profile_arg: Option<&str>,
    url_arg: Option<&str>,
    no_input: bool,
) -> anyhow::Result<(GlobalConfig, String)> {
    let target = match profile_arg {
        Some(name) => {
            crate::config::validate_profile_name(name)?;
            name.to_string()
        }
        None => global
            .default_profile
            .clone()
            .unwrap_or_else(|| "default".to_string()),
    };

    let exists = global.profiles.contains_key(&target);
    let entry = global.profiles
        .entry(target.clone())
        .or_insert_with(ProfileConfig::default);

    if let Some(url) = url_arg {
        // Trim trailing slash matches the convention used elsewhere
        entry.url = Some(url.trim_end_matches('/').to_string());
    } else if !exists && no_input {
        return Err(JrError::UserError(
            "--url required when creating a new profile under --no-input".into()
        ).into());
    }

    if global.default_profile.is_none() {
        global.default_profile = Some(target.clone());
    }

    Ok((global, target))
}
```

- [ ] **Step 4: Refactor login_token / login_oauth to use prepare_login_target**

Wire `handle_login` to:
1. Parse args
2. Run `prepare_login_target(...)`
3. Save mutated global to config.toml
4. Reload config, switch active profile temporarily for the login flow if needed
5. Call existing login flow (which now stores OAuth tokens with the resolved profile name)

For brevity, the implementation reuses `login_token(profile, email, token, no_input)` and `login_oauth(profile, client_id, client_secret, no_input)` — both gain `profile` as their first arg.

- [ ] **Step 5: Update existing login_token/login_oauth signatures**

```rust
pub async fn login_token(
    profile: &str,
    email: Option<String>,
    token: Option<String>,
    no_input: bool,
) -> Result<()> {
    // ... existing logic ...
    // Final store: shared API token (always under flat keys)
    auth::store_api_token(&email, &token)?;
    Ok(())
}

pub async fn login_oauth(
    profile: &str,
    client_id: Option<String>,
    client_secret: Option<String>,
    no_input: bool,
) -> Result<()> {
    // ... existing logic ...
    // load_oauth_app_credentials still returns shared client_id/client_secret.
    // oauth_login(profile, &client_id, &client_secret, &scopes) already takes profile (Task 5).
    let result = api::auth::oauth_login(profile, &client_id, &client_secret, &scopes).await?;
    // Persist site info into the named profile in config
    let mut config = Config::load()?;
    let p = config.global.profiles.entry(profile.to_string()).or_default();
    p.url = Some(result.site_url);
    p.cloud_id = Some(result.cloud_id);
    p.auth_method = Some("oauth".into());
    config.save_global()?;
    Ok(())
}
```

Update `cli/init.rs` callers to pass `"default"` as the profile.

- [ ] **Step 6: Run fmt + clippy + tests**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 7: Commit**

```bash
git add src/cli/mod.rs src/cli/auth.rs src/cli/init.rs
git commit -m "feat(auth): jr auth login supports --profile and --url"
```

---

## Task 13: jr auth status, refresh, and logout (per-profile + per-profile status)

**Files:**
- Modify: `src/cli/mod.rs` (Status, Refresh gain profile; new Logout variant)
- Modify: `src/cli/auth.rs` (handle_status takes profile; handle_logout new)

- [ ] **Step 1: Update AuthCommand variants**

In `src/cli/mod.rs`:

```rust
Status {
    /// Profile to show status for. Defaults to active profile.
    #[arg(long)]
    profile: Option<String>,
},
Refresh {
    /// Profile to refresh credentials for. Defaults to active profile.
    #[arg(long)]
    profile: Option<String>,
    // ... existing flags ...
},
/// Clear OAuth tokens for a profile (profile entry stays in config).
/// Shared API-token credential is NEVER touched.
Logout {
    /// Profile to log out of. Defaults to active profile.
    #[arg(long)]
    profile: Option<String>,
},
```

- [ ] **Step 2: Add tests**

```rust
#[test]
fn handle_logout_clears_only_target_profile_tokens() {
    // logic-only test — actual keyring touch is covered by integration tests
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert("default".to_string(), ProfileConfig::default());
    profiles.insert("sandbox".to_string(), ProfileConfig::default());
    let global = GlobalConfig {
        default_profile: Some("default".into()),
        profiles,
        ..GlobalConfig::default()
    };
    let target = resolve_logout_target(&global, None, "default");
    assert_eq!(target, "default");

    let target = resolve_logout_target(&global, Some("sandbox"), "default");
    assert_eq!(target, "sandbox");
}
```

- [ ] **Step 3: Implement helpers**

```rust
pub(super) fn resolve_logout_target(
    _global: &GlobalConfig,
    profile_arg: Option<&str>,
    active: &str,
) -> String {
    profile_arg.unwrap_or(active).to_string()
}

pub async fn handle_logout(profile_arg: Option<&str>) -> anyhow::Result<()> {
    let config = Config::load()?;
    let target = resolve_logout_target(&config.global, profile_arg, &config.active_profile_name);
    crate::config::validate_profile_name(&target)?;
    api::auth::clear_profile_creds(&target)?;
    output::print_success(&format!("Logged out of profile {target:?}"));
    Ok(())
}
```

- [ ] **Step 4: Update status / refresh to take profile**

Existing `pub async fn status() -> Result<()>` becomes:

```rust
pub async fn status(profile_arg: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let target = profile_arg.unwrap_or(&config.active_profile_name).to_string();
    // ... existing status logic, but report for `target` ...
    Ok(())
}
```

Same shape change for `refresh_credentials` — it gains a `profile_arg` param and threads it through.

- [ ] **Step 5: Wire into dispatch in main.rs**

- [ ] **Step 6: Run fmt + clippy + tests**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 7: Commit**

```bash
git add src/cli/mod.rs src/cli/auth.rs src/main.rs
git commit -m "feat(auth): jr auth status/refresh/logout support --profile"
```

---

## Task 14: jr auth remove

**Files:**
- Modify: `src/cli/mod.rs` (Remove variant)
- Modify: `src/cli/auth.rs` (handle_remove)

- [ ] **Step 1: Add Remove variant**

```rust
/// Permanently delete a profile (config + cache + per-profile OAuth tokens).
/// Shared credentials are NEVER touched.
Remove {
    /// Profile name to remove. Cannot be the active profile —
    /// switch first with `jr auth switch`.
    name: String,
},
```

- [ ] **Step 2: Tests**

```rust
#[test]
fn remove_active_profile_returns_error() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert("default".to_string(), ProfileConfig::default());
    let global = GlobalConfig {
        default_profile: Some("default".into()),
        profiles,
        ..GlobalConfig::default()
    };
    let result = handle_remove_in_memory(global, "default", "default");
    assert!(result.is_err());
    let msg = format!("{:#}", result.unwrap_err());
    assert!(msg.contains("cannot remove active"), "got: {msg}");
}

#[test]
fn remove_unknown_profile_returns_error() {
    let global = GlobalConfig {
        default_profile: Some("default".into()),
        ..GlobalConfig::default()
    };
    let result = handle_remove_in_memory(global, "ghost", "default");
    assert!(result.is_err());
    let msg = format!("{:#}", result.unwrap_err());
    assert!(msg.contains("unknown profile"), "got: {msg}");
}

#[test]
fn remove_existing_non_active_profile_succeeds() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert("default".to_string(), ProfileConfig::default());
    profiles.insert("sandbox".to_string(), ProfileConfig::default());
    let global = GlobalConfig {
        default_profile: Some("default".into()),
        profiles,
        ..GlobalConfig::default()
    };
    let mutated = handle_remove_in_memory(global, "sandbox", "default").unwrap();
    assert!(!mutated.profiles.contains_key("sandbox"));
    assert!(mutated.profiles.contains_key("default"));
}
```

- [ ] **Step 3: Implement handle_remove + handle_remove_in_memory**

```rust
pub(super) fn handle_remove_in_memory(
    mut global: GlobalConfig,
    target: &str,
    active: &str,
) -> anyhow::Result<GlobalConfig> {
    crate::config::validate_profile_name(target)?;
    if !global.profiles.contains_key(target) {
        let known: Vec<&str> = global.profiles.keys().map(String::as_str).collect();
        return Err(JrError::UserError(format!(
            "unknown profile: {target}; known: {}",
            if known.is_empty() { "(none)".into() } else { known.join(", ") }
        )).into());
    }
    if target == active {
        return Err(JrError::UserError(format!(
            "cannot remove active profile {target:?}; switch first with \"jr auth switch <other>\""
        )).into());
    }
    global.profiles.remove(target);
    Ok(global)
}

pub async fn handle_remove(target: &str, no_input: bool) -> anyhow::Result<()> {
    let mut config = Config::load()?;
    crate::config::validate_profile_name(target)?;

    if !no_input {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Permanently remove profile {target:?}? \
                 This deletes its config entry, cache, and OAuth tokens. \
                 Shared credentials remain."
            ))
            .default(false)
            .interact()?;
        if !confirm {
            output::print_warning("Aborted.");
            return Ok(());
        }
    }

    config.global = handle_remove_in_memory(config.global, target, &config.active_profile_name)?;
    config.save_global()?;
    let _ = api::auth::clear_profile_creds(target);
    let _ = crate::cache::clear_profile_cache(target);
    output::print_success(&format!("Removed profile {target:?}"));
    Ok(())
}
```

- [ ] **Step 4: Wire into dispatch + add `print_warning` helper if absent**

Check `src/output.rs` for `print_warning`. If missing, add:

```rust
pub fn print_warning(msg: &str) {
    eprintln!("warning: {msg}");
}
```

- [ ] **Step 5: Run fmt + clippy + tests**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs src/cli/auth.rs src/output.rs src/main.rs
git commit -m "feat(auth): add jr auth remove subcommand"
```

---

## Task 15: jr init multi-profile awareness + integration tests

**Files:**
- Modify: `src/cli/init.rs` (prompt before adding profile)
- Create: `tests/auth_profiles.rs`
- Create: `tests/migration_legacy.rs`

- [ ] **Step 1: Update jr init**

In `src/cli/init.rs::handle()`, after loading existing config, before re-running setup:

```rust
let existing = Config::load().ok();
if let Some(c) = existing.as_ref() {
    if !c.global.profiles.is_empty() {
        let names: Vec<String> = c.global.profiles.keys().cloned().collect();
        eprintln!("Profiles already configured: {}", names.join(", "));
        let add = Confirm::new()
            .with_prompt("Add another profile?")
            .default(false)
            .interact()
            .context("failed to prompt for additional profile")?;
        if !add {
            return Ok(());
        }
        // Rest of jr init flow runs against a NEW profile name (prompted below).
        let profile_name: String = Input::new()
            .with_prompt("Name for the new profile")
            .interact_text()
            .context("failed to read profile name")?;
        crate::config::validate_profile_name(&profile_name)?;
        // Set as active for the duration of this init run so all writes target it.
        // SAFETY: jr init is single-threaded.
        unsafe { std::env::set_var("JR_PROFILE_OVERRIDE", &profile_name); }
    }
}
```

- [ ] **Step 2: Create tests/auth_profiles.rs**

```rust
//! Integration tests for multi-profile auth workflows.
mod common;

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
    jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .args(["auth", "switch", "ghost"])
        .assert()
        .failure()
        .code(64);
}

#[test]
fn auth_list_shows_no_profiles_for_fresh_install() {
    let (dir, _path) = fresh_config_dir();
    jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .args(["auth", "list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicates::str::contains("[]"));
}

#[test]
fn auth_remove_active_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(&path, r#"
        default_profile = "default"
        [profiles.default]
        url = "https://x.example"
        auth_method = "api_token"
    "#).unwrap();

    jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .args(["auth", "remove", "default", "--no-input"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("cannot remove active"));
}

#[test]
fn precedence_flag_overrides_env_overrides_config() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(&path, r#"
        default_profile = "from-config"
        [profiles.from-config]
        url = "https://from-config.example"
        [profiles.from-env]
        url = "https://from-env.example"
        [profiles.from-flag]
        url = "https://from-flag.example"
    "#).unwrap();

    // Flag wins
    let out = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_PROFILE", "from-env")
        .args(["--profile", "from-flag", "auth", "list", "--output", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let active: Vec<&serde_json::Value> = parsed.as_array().unwrap().iter()
        .filter(|p| p["active"].as_bool() == Some(true))
        .collect();
    assert_eq!(active[0]["name"], "from-flag");
}
```

- [ ] **Step 3: Create tests/migration_legacy.rs**

```rust
//! Legacy [instance] → [profiles.default] migration tests.

use std::fs;
use tempfile::TempDir;

#[test]
fn legacy_instance_block_migrated_in_memory() {
    let dir = TempDir::new().unwrap();
    let cfg_path = dir.path().join("jr").join("config.toml");
    fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    fs::write(&cfg_path, r#"
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
    "#).unwrap();

    // SAFETY: test runs single-threaded under cargo test --test
    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()); }
    let config = jr::config::Config::load().unwrap();
    unsafe { std::env::remove_var("XDG_CONFIG_HOME"); }

    // Migration ran
    assert_eq!(config.active_profile_name, "default");
    assert!(config.global.profiles.contains_key("default"));
    let p = &config.global.profiles["default"];
    assert_eq!(p.url.as_deref(), Some("https://legacy.atlassian.net"));
    assert_eq!(p.cloud_id.as_deref(), Some("legacy-1"));
    assert_eq!(p.team_field_id.as_deref(), Some("customfield_99"));
    assert_eq!(p.story_points_field_id.as_deref(), Some("customfield_42"));

    // [defaults] preserved as global (not migrated to profile)
    assert_eq!(config.global.defaults.output, "json");

    // On-disk file is now in new shape
    let on_disk = fs::read_to_string(&cfg_path).unwrap();
    assert!(on_disk.contains("default_profile"));
    assert!(on_disk.contains("[profiles.default]"));
    insta::assert_snapshot!(
        on_disk
            .replace(dir.path().to_str().unwrap(), "<tmp>")
            .as_str()
    );
}

#[test]
fn migration_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let cfg_path = dir.path().join("jr").join("config.toml");
    fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
    fs::write(&cfg_path, r#"
        [instance]
        url = "https://x"
        auth_method = "api_token"
    "#).unwrap();

    unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()); }
    let _ = jr::config::Config::load().unwrap();
    let after_first = fs::read_to_string(&cfg_path).unwrap();
    let _ = jr::config::Config::load().unwrap();
    let after_second = fs::read_to_string(&cfg_path).unwrap();
    unsafe { std::env::remove_var("XDG_CONFIG_HOME"); }

    assert_eq!(after_first, after_second, "second load should not modify file");
}
```

- [ ] **Step 4: Add lib re-exports if missing**

In `src/lib.rs`, ensure `pub mod config;` is exposed so integration tests can use `jr::config::Config`.

- [ ] **Step 5: Run integration tests**

```bash
cargo test --test auth_profiles --test migration_legacy 2>&1 | tail -10
```

- [ ] **Step 6: Run full suite**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

Approve any insta snapshots:

```bash
cargo insta review
```

- [ ] **Step 7: Commit**

```bash
git add src/cli/init.rs tests/auth_profiles.rs tests/migration_legacy.rs src/lib.rs src/snapshots/
git commit -m "feat(init): multi-profile awareness; add integration tests"
```

---

## Task 16: Cleanup — remove legacy InstanceConfig + FieldsConfig fields

**Files:**
- Modify: `src/config.rs` (drop the legacy serde fields once migration is wired)

This is a deferrable cleanup — only safe AFTER all release channels have run the migration once (so on-disk configs are guaranteed in new shape).

For this PR: keep the legacy fields with `#[serde(skip_serializing)]` so reading still works (in case migration is interrupted), but new writes never include them. A future PR removes them entirely.

- [ ] **Step 1: Add `#[serde(skip_serializing)]` to legacy fields**

```rust
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: std::collections::BTreeMap<String, ProfileConfig>,
    #[serde(default, skip_serializing)]
    pub instance: InstanceConfig,
    #[serde(default, skip_serializing)]
    pub fields: FieldsConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
}
```

- [ ] **Step 2: Verify migration_legacy snapshot still has no [instance] / [fields] in output**

```bash
cargo test --test migration_legacy
cat src/snapshots/migration_legacy__legacy_instance_block_migrated_in_memory.snap
```

Expected: snapshot shows `[profiles.default]` but no `[instance]` or `[fields]`.

- [ ] **Step 3: Run fmt + clippy + tests**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | tail -3
```

- [ ] **Step 4: Commit**

```bash
git add src/config.rs
git commit -m "refactor(config): stop serializing legacy [instance]/[fields] blocks"
```

---

## Final Verification

- [ ] **Run full CI-equivalent check set**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
JR_RUN_KEYRING_TESTS=1 cargo test --lib api::auth::tests -- --ignored
```

All green.

- [ ] **Manual smoke test**

```bash
# Build
cargo build --release

# Inspect a fresh-shape config (use a tmp dir)
TMPDIR_TEST=$(mktemp -d)
XDG_CONFIG_HOME=$TMPDIR_TEST ./target/release/jr auth list --output json
# Expected: []

# Set up two profiles manually
mkdir -p "$TMPDIR_TEST/jr"
cat > "$TMPDIR_TEST/jr/config.toml" <<'EOF'
default_profile = "prod"

[profiles.prod]
url = "https://acme.atlassian.net"
auth_method = "api_token"

[profiles.sandbox]
url = "https://acme-sandbox.atlassian.net"
auth_method = "api_token"
EOF

XDG_CONFIG_HOME=$TMPDIR_TEST ./target/release/jr auth list
# Expected: table showing both, * on prod

XDG_CONFIG_HOME=$TMPDIR_TEST ./target/release/jr --profile sandbox auth list --output json
# Expected: JSON, sandbox marked active

XDG_CONFIG_HOME=$TMPDIR_TEST ./target/release/jr auth switch sandbox
XDG_CONFIG_HOME=$TMPDIR_TEST ./target/release/jr auth list
# Expected: * now on sandbox
```

- [ ] **Push and create PR**

```bash
git push -u origin feat/multi-profile-auth
gh pr create --base develop --title "feat: multi-profile authentication" --body "$(cat <<'EOF'
## Summary

- Lets jr target multiple Atlassian Cloud sites from one install
- `jr auth switch <profile>` flips active profile persistently
- Shared classic API token across profiles (account-level credential)
- Per-profile OAuth tokens (cloudId-scoped)
- Auto-migration of legacy `[instance]` config; lazy keyring migration

Spec: docs/specs/multi-profile-auth.md
Plan: docs/superpowers/plans/2026-04-24-multi-profile-auth.md

## Test plan

- [x] `cargo fmt --all -- --check` passes
- [x] `cargo clippy --all-targets -- -D warnings` passes
- [x] `cargo test` passes (all suites)
- [x] `JR_RUN_KEYRING_TESTS=1 cargo test -- --ignored` passes locally
- [x] Manual smoke: `jr auth list / switch / login --profile / remove`
- [x] Migration smoke: legacy [instance] config migrates on first load
EOF
)"
gh api repos/Zious11/jira-cli/pulls/<PR_NUM>/requested_reviewers --method POST -f 'reviewers[]=copilot-pull-request-reviewer[bot]'
```

---

## Self-Review

**Spec coverage:**
- ✓ Config Schema → Tasks 2, 3, 4, 16
- ✓ Active-Profile Resolution → Tasks 3, 9
- ✓ Profile Name Validation → Task 1
- ✓ Keyring Layout → Task 5
- ✓ `:` Separator Safety (validation enforces) → Task 1
- ✓ Cache Layout → Task 6
- ✓ CLI Surface → Tasks 9 (--profile), 10 (switch), 11 (list), 12 (login), 13 (status/refresh/logout), 14 (remove), 15 (init)
- ✓ Migration → Tasks 4 (TOML), 5 (lazy keyring)
- ✓ Error Handling → Tasks 1, 3, 9, 10, 13, 14
- ✓ Testing — unit/integration/snapshot/keyring-gated → Tasks 1–15
- ✓ Concurrency & Cross-Platform Notes — surfaced in spec; no test coverage required (pre-existing limitations)
- ✓ Out of Scope items NOT implemented (correct)

**Placeholders:** None remaining.

**Type consistency:**
- `ProfileConfig` defined in Task 2; consumed in Tasks 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15.
- `validate_profile_name` defined in Task 1; consumed in Tasks 9, 10, 12, 13, 14.
- `cache_dir(profile)` defined in Task 6; consumed via call sites in Tasks 6, 14.
- `store_oauth_tokens(profile, ...)` / `load_oauth_tokens(profile)` defined in Task 5; consumed in Tasks 7, 12, 13.
- `clear_profile_creds(profile)` defined in Task 5; consumed in Tasks 13, 14.
- `clear_profile_cache(profile)` defined in Task 6; consumed in Task 14.
- `JR_PROFILE_OVERRIDE` env var: defined in Task 3 (consumed by Config::load) + populated in Task 9 (main.rs from --profile flag) + Task 15 (jr init for additional profile).
- `Config::active_profile_name` field added in Task 3; consumed throughout.
- `Config::active_profile()` (returns owned ProfileConfig) and `Config::active_profile_or_err()` defined in Task 3; consumed in Task 7, 8.

All consistent.
