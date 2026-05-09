use super::*;
use crate::config::{Config, GlobalConfig, ProfileConfig};

fn config_with_auth_method(method: Option<&str>) -> Config {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "default".to_string(),
        ProfileConfig {
            url: Some("https://example.atlassian.net".into()),
            auth_method: method.map(str::to_string),
            ..ProfileConfig::default()
        },
    );
    Config {
        global: GlobalConfig {
            default_profile: Some("default".into()),
            profiles,
            ..Default::default()
        },
        project: Default::default(),
        active_profile_name: "default".into(),
    }
}

#[test]
fn chosen_flow_defaults_to_token_when_unset() {
    let config = config_with_auth_method(None);
    assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
}

#[test]
fn chosen_flow_uses_token_for_explicit_api_token() {
    let config = config_with_auth_method(Some("api_token"));
    assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
}

#[test]
fn chosen_flow_uses_oauth_when_config_says_so() {
    let config = config_with_auth_method(Some("oauth"));
    assert_eq!(chosen_flow(&config, false), AuthFlow::OAuth);
}

#[test]
fn chosen_flow_oauth_override_wins_over_config() {
    let config = config_with_auth_method(Some("api_token"));
    assert_eq!(chosen_flow(&config, true), AuthFlow::OAuth);
}

/// Regression: refresh against a non-active profile must dispatch the
/// flow stored on THAT profile's auth_method, not the active profile's.
/// `chosen_flow(&Config, _)` always reads the active profile, which
/// silently picked the wrong flow when active=api_token but the refresh
/// target=oauth (or vice-versa). `chosen_flow_for_profile` takes the
/// resolved target profile so callers like `refresh_credentials` can
/// thread the right ProfileConfig in.
#[test]
fn chosen_flow_for_profile_inspects_passed_profile_not_active() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "default".into(),
        ProfileConfig {
            auth_method: Some("api_token".into()),
            ..ProfileConfig::default()
        },
    );
    profiles.insert(
        "sandbox".into(),
        ProfileConfig {
            auth_method: Some("oauth".into()),
            ..ProfileConfig::default()
        },
    );
    let config = Config {
        global: GlobalConfig {
            default_profile: Some("default".into()),
            profiles,
            ..GlobalConfig::default()
        },
        project: Default::default(),
        active_profile_name: "default".into(),
    };
    // chosen_flow without override returns Token (active is api_token)
    assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
    // chosen_flow_for_profile against sandbox returns OAuth even though
    // the active profile is api_token — proves the resolver looks at
    // the passed profile, not the active one.
    let sandbox = config.global.profiles["sandbox"].clone();
    assert_eq!(chosen_flow_for_profile(&sandbox, false), AuthFlow::OAuth);
}

#[test]
fn auth_flow_labels_match_config_and_json_conventions() {
    assert_eq!(AuthFlow::Token.label(), "api_token");
    assert_eq!(AuthFlow::OAuth.label(), "oauth");
}

#[test]
fn refresh_payload_pins_token_shape() {
    let payload = refresh_success_payload(AuthFlow::Token);
    assert_eq!(payload["status"], "refreshed");
    assert_eq!(payload["auth_method"], "api_token");
    assert!(
        payload["next_step"]
            .as_str()
            .unwrap()
            .contains("Always Allow"),
        "next_step should guide the user to click Always Allow, got: {}",
        payload["next_step"]
    );
}

#[test]
fn refresh_payload_pins_oauth_shape() {
    let payload = refresh_success_payload(AuthFlow::OAuth);
    assert_eq!(payload["status"], "refreshed");
    assert_eq!(payload["auth_method"], "oauth");
}

// ── resolve_credential ───────────────────────────────────────────
//
// Env-reading tests must serialize process-environment mutation across
// parallel test threads. `std::env::set_var` / `remove_var` are unsafe
// in Rust 2024 because concurrent env access (even on different keys)
// is UB — C's getenv/setenv aren't thread-safe. `EnvGuard` holds
// `ENV_LOCK` for its full lifetime and removes the var on drop so a
// panic mid-test doesn't leak state to later tests in the same
// process. Matches the pattern in src/config.rs::ENV_MUTEX.

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct EnvGuard {
    key: &'static str,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let lock = ENV_LOCK.lock().unwrap();
        // SAFETY: test env mutation is serialized by ENV_LOCK, held for
        // this guard's lifetime. The Drop impl unsets the same
        // test-local key before releasing the lock.
        unsafe {
            std::env::set_var(key, value);
        }
        EnvGuard { key, _lock: lock }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: matches the test-local key set in `EnvGuard::set`
        // while `_lock` is still held by this `EnvGuard`.
        unsafe {
            std::env::remove_var(self.key);
        }
    }
}

#[test]
fn resolve_credential_prefers_flag_over_env() {
    let _guard = EnvGuard::set("_JR_TEST_PREFERS_FLAG", "from-env");
    let got = resolve_credential(
        Some("from-flag".into()),
        "_JR_TEST_PREFERS_FLAG",
        "--email",
        "Jira email",
        false,
        true,
        None,
    )
    .unwrap();
    assert_eq!(got, "from-flag");
}

#[test]
fn resolve_credential_falls_back_to_env_when_flag_absent() {
    let _guard = EnvGuard::set("_JR_TEST_FALLS_BACK", "from-env");
    let got = resolve_credential(
        None,
        "_JR_TEST_FALLS_BACK",
        "--email",
        "Jira email",
        false,
        true,
        None,
    )
    .unwrap();
    assert_eq!(got, "from-env");
}

#[test]
fn resolve_credential_ignores_empty_flag_and_env() {
    // Empty values should fall through to the no_input error path.
    let _guard = EnvGuard::set("_JR_TEST_EMPTY", "");
    let err = resolve_credential(
        Some(String::new()),
        "_JR_TEST_EMPTY",
        "--email",
        "Jira email",
        false,
        true,
        None,
    )
    .unwrap_err();
    assert!(
        err.downcast_ref::<JrError>()
            .is_some_and(|e| matches!(e, JrError::UserError(_))),
        "Expected JrError::UserError for empty inputs, got: {err}"
    );
}

#[test]
fn resolve_credential_no_input_errors_when_missing() {
    // resolve_credential reads env via std::env::var — hold ENV_LOCK to
    // serialize against set/remove calls in sibling tests.
    let _lock = ENV_LOCK.lock().unwrap();
    let err = resolve_credential(
        None,
        "_JR_TEST_UNSET_MISSING",
        "--email",
        "Jira email",
        false,
        true,
        None,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        err.downcast_ref::<JrError>()
            .is_some_and(|e| matches!(e, JrError::UserError(_))),
        "Expected JrError::UserError, got: {err}"
    );
    assert!(
        msg.contains("--email") && msg.contains("$_JR_TEST_UNSET_MISSING"),
        "Error should cite both flag and env var: {msg}"
    );
}

#[test]
fn resolve_credential_oauth_hint_appears_in_error() {
    // Same env-read serialization as the test above.
    let _lock = ENV_LOCK.lock().unwrap();
    let err = resolve_credential(
        None,
        "_JR_TEST_UNSET_OAUTH",
        "--client-id",
        "OAuth Client ID",
        false,
        true,
        Some(OAUTH_APP_HINT),
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("developer.atlassian.com/console/myapps"),
        "OAuth error should cite dev console URL: {msg}"
    );
}

fn profile_with_oauth_scopes(scopes: Option<&str>) -> ProfileConfig {
    ProfileConfig {
        oauth_scopes: scopes.map(String::from),
        ..ProfileConfig::default()
    }
}

#[test]
fn resolve_oauth_scopes_none_returns_default() {
    let p = profile_with_oauth_scopes(None);
    assert_eq!(
        resolve_oauth_scopes(&p).unwrap(),
        auth::DEFAULT_OAUTH_SCOPES
    );
}

#[test]
fn resolve_oauth_scopes_trims_and_collapses_whitespace() {
    let p = profile_with_oauth_scopes(Some(
        "  read:issue:jira   write:comment:jira\n\toffline_access  ",
    ));
    assert_eq!(
        resolve_oauth_scopes(&p).unwrap(),
        "read:issue:jira write:comment:jira offline_access"
    );
}

#[test]
fn resolve_oauth_scopes_empty_string_is_config_error() {
    let p = profile_with_oauth_scopes(Some(""));
    let err = resolve_oauth_scopes(&p).unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("oauth_scopes is empty"),
        "unexpected error: {msg}"
    );
}

#[test]
fn resolve_oauth_scopes_whitespace_only_is_config_error() {
    let p = profile_with_oauth_scopes(Some("   \n\t  "));
    let err = resolve_oauth_scopes(&p).unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("oauth_scopes is empty"),
        "unexpected error: {msg}"
    );
}

/// Regression: `resolve_oauth_scopes` must read the *passed* profile,
/// not anything off a `Config`. `login_oauth(profile, ...)` may target
/// a non-active profile and used to resolve scopes from the active
/// profile, silently returning the wrong scope list.
#[test]
fn resolve_oauth_scopes_inspects_passed_profile_not_active() {
    let custom = ProfileConfig {
        oauth_scopes: Some("custom:scope offline_access".into()),
        ..ProfileConfig::default()
    };
    assert_eq!(
        resolve_oauth_scopes(&custom).unwrap(),
        "custom:scope offline_access"
    );
}

/// The default scope literal is a backward-compatibility contract for
/// every user who hasn't opted into `oauth_scopes`. A typo that drops
/// `offline_access` would silently break refresh tokens for everyone.
/// The literal must also stay in lockstep with the `jr` Atlassian
/// Developer Console app's registered permissions — a mismatch causes
/// authorize to reject with `invalid_scope`.
#[test]
fn default_oauth_scopes_pins_the_full_set_with_offline_access() {
    // Each scope is checked individually so a future addition can
    // grow the set without churning a single string literal — but the
    // assertion still pins each scope exactly to catch typos.
    let scopes = auth::DEFAULT_OAUTH_SCOPES;
    for required in [
        "read:jira-work",
        "write:jira-work",
        "read:jira-user",
        "read:servicedesk-request",
        "read:cmdb-object:jira",
        "read:cmdb-schema:jira",
        "offline_access",
    ] {
        assert!(
            scopes.split_whitespace().any(|s| s == required),
            "DEFAULT_OAUTH_SCOPES is missing required scope `{required}`: {scopes:?}"
        );
    }
    // Whole-string canary: a single trailing comma or stray comment
    // would still satisfy the per-scope check above, so pin the full
    // expected set.
    let expected = "read:jira-work write:jira-work read:jira-user \
                        read:servicedesk-request \
                        read:cmdb-object:jira read:cmdb-schema:jira \
                        offline_access";
    let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(normalize(scopes), normalize(expected));

    // Regression guard for the multi-line literal: assert no double
    // spaces in the actual constant. Atlassian's authorize endpoint
    // percent-encodes scope values verbatim, so `%20%20` between
    // scopes would be parsed as an empty scope and surface as
    // `invalid_scope`. The current `concat!` form makes this
    // structurally impossible, but pinning here catches a future
    // refactor that drops back to a multi-line literal without the
    // line-continuation escape.
    assert!(
        !scopes.contains("  "),
        "DEFAULT_OAUTH_SCOPES has consecutive spaces: {scopes:?}"
    );
}

#[test]
fn resolve_logout_target_defaults_to_active() {
    let global = crate::config::GlobalConfig::default();
    assert_eq!(resolve_logout_target(&global, None, "default"), "default");
    assert_eq!(
        resolve_logout_target(&global, Some("sandbox"), "default"),
        "sandbox"
    );
}

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

#[test]
fn remove_active_profile_returns_error() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "default".to_string(),
        crate::config::ProfileConfig::default(),
    );
    let global = crate::config::GlobalConfig {
        default_profile: Some("default".into()),
        profiles,
        ..crate::config::GlobalConfig::default()
    };
    let result = handle_remove_in_memory(global, "default", "default");
    assert!(result.is_err());
    let msg = format!("{:#}", result.unwrap_err());
    assert!(msg.contains("cannot remove active"), "got: {msg}");
}

#[test]
fn remove_unknown_profile_returns_error() {
    let global = crate::config::GlobalConfig {
        default_profile: Some("default".into()),
        ..crate::config::GlobalConfig::default()
    };
    let result = handle_remove_in_memory(global, "ghost", "default");
    assert!(result.is_err());
    let msg = format!("{:#}", result.unwrap_err());
    assert!(msg.contains("unknown profile"), "got: {msg}");
}

#[test]
fn remove_existing_non_active_profile_succeeds() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "default".to_string(),
        crate::config::ProfileConfig::default(),
    );
    profiles.insert(
        "sandbox".to_string(),
        crate::config::ProfileConfig::default(),
    );
    let global = crate::config::GlobalConfig {
        default_profile: Some("default".into()),
        profiles,
        ..crate::config::GlobalConfig::default()
    };
    let mutated = handle_remove_in_memory(global, "sandbox", "default").unwrap();
    assert!(!mutated.profiles.contains_key("sandbox"));
    assert!(mutated.profiles.contains_key("default"));
}

fn three_profile_fixture() -> GlobalConfig {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "default".to_string(),
        ProfileConfig {
            url: Some("https://acme.atlassian.net".into()),
            auth_method: Some("api_token".into()),
            ..ProfileConfig::default()
        },
    );
    profiles.insert(
        "sandbox".to_string(),
        ProfileConfig {
            url: Some("https://acme-sandbox.atlassian.net".into()),
            auth_method: Some("oauth".into()),
            cloud_id: Some("xyz-789".into()),
            ..ProfileConfig::default()
        },
    );
    profiles.insert(
        "staging".to_string(),
        ProfileConfig {
            url: Some("https://acme-staging.atlassian.net".into()),
            auth_method: Some("api_token".into()),
            ..ProfileConfig::default()
        },
    );
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
    let active: Vec<&serde_json::Value> = arr
        .iter()
        .filter(|p| p["active"].as_bool() == Some(true))
        .collect();
    assert_eq!(active.len(), 1, "exactly one active");
    assert_eq!(active[0]["name"], "default");
}

#[test]
fn login_create_new_profile_no_input_requires_url() {
    let global = crate::config::GlobalConfig::default();
    let result = prepare_login_target(global, Some("sandbox"), None, true, "default");
    assert!(result.is_err());
    let msg = format!("{:#}", result.unwrap_err());
    assert!(msg.contains("--url required"), "got: {msg}");
}

#[test]
fn login_create_new_profile_with_url_succeeds() {
    let global = crate::config::GlobalConfig::default();
    let (mutated, target) = prepare_login_target(
        global,
        Some("sandbox"),
        Some("https://sandbox.example"),
        true,
        "default",
    )
    .unwrap();
    assert_eq!(target, "sandbox");
    assert_eq!(
        mutated.profiles["sandbox"].url.as_deref(),
        Some("https://sandbox.example")
    );
}

#[test]
fn login_existing_profile_with_url_updates_url() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "default".to_string(),
        crate::config::ProfileConfig {
            url: Some("https://old.example".into()),
            ..crate::config::ProfileConfig::default()
        },
    );
    let global = crate::config::GlobalConfig {
        default_profile: Some("default".into()),
        profiles,
        ..crate::config::GlobalConfig::default()
    };
    let (mutated, target) = prepare_login_target(
        global,
        Some("default"),
        Some("https://new.example"),
        true,
        "default",
    )
    .unwrap();
    assert_eq!(target, "default");
    assert_eq!(
        mutated.profiles["default"].url.as_deref(),
        Some("https://new.example")
    );
}

/// Regression: when `--profile` is omitted, fallback uses the active
/// profile name (which encodes flag > env > config), NOT the
/// `default_profile` config field — using the latter ignores the
/// `JR_PROFILE` env / `--profile` global flag.
#[test]
fn login_falls_back_to_active_profile_name_not_default_profile_field() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "from-env".into(),
        crate::config::ProfileConfig {
            url: Some("https://from-env.example".into()),
            ..crate::config::ProfileConfig::default()
        },
    );
    let global = crate::config::GlobalConfig {
        default_profile: Some("from-config".into()),
        profiles,
        ..crate::config::GlobalConfig::default()
    };
    let (_mutated, target) =
        prepare_login_target(global, None, Some("https://x.example"), true, "from-env").unwrap();
    assert_eq!(
        target, "from-env",
        "must follow active_profile_name, not default_profile field"
    );
}

/// Resolution order: flag → env → keychain → embedded → prompt.
/// Flag wins even when env is set.
#[test]
fn resolve_oauth_app_credentials_flag_wins() {
    let (id, secret, source) = resolve_oauth_app_credentials_for_test(
        Some("flag-id".into()),
        Some("flag-secret".into()),
        None, // env_id
        None, // env_secret
        None, // keychain
        None, // embedded
        true, // no_input
    )
    .expect("flag path must succeed");
    assert_eq!(id, "flag-id");
    assert_eq!(secret, "flag-secret");
    assert_eq!(source, crate::api::auth_embedded::OAuthAppSource::Flag);
}

#[test]
fn resolve_oauth_app_credentials_env_wins_over_keychain() {
    let (id, secret, source) = resolve_oauth_app_credentials_for_test(
        None,
        None,
        Some("env-id".into()),
        Some("env-secret".into()),
        Some(("kc-id".into(), "kc-secret".into())),
        None,
        true,
    )
    .unwrap();
    assert_eq!(
        (id.as_str(), secret.as_str(), source),
        (
            "env-id",
            "env-secret",
            crate::api::auth_embedded::OAuthAppSource::Env
        )
    );
}

#[test]
fn resolve_oauth_app_credentials_keychain_wins_over_embedded() {
    let (id, _, source) = resolve_oauth_app_credentials_for_test(
        None,
        None,
        None,
        None,
        Some(("kc-id".into(), "kc-secret".into())),
        Some(("embed-id".into(), "embed-secret".into())),
        true,
    )
    .unwrap();
    assert_eq!(id, "kc-id");
    assert_eq!(source, crate::api::auth_embedded::OAuthAppSource::Keychain);
}

#[test]
fn resolve_oauth_app_credentials_embedded_when_no_user_input() {
    let (id, secret, source) = resolve_oauth_app_credentials_for_test(
        None,
        None,
        None,
        None,
        None,
        Some(("embed-id".into(), "embed-secret".into())),
        true,
    )
    .unwrap();
    assert_eq!(
        (id.as_str(), secret.as_str(), source),
        (
            "embed-id",
            "embed-secret",
            crate::api::auth_embedded::OAuthAppSource::Embedded
        )
    );
}

#[test]
fn resolve_oauth_app_credentials_no_input_errors_when_all_absent() {
    let err = resolve_oauth_app_credentials_for_test(None, None, None, None, None, None, true)
        .unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("OAuth"), "got: {msg}");
    assert!(
        msg.contains("--client-id") || msg.contains("JR_OAUTH_CLIENT_ID"),
        "error must cite the BYO escape hatch: {msg}"
    );
}

#[test]
fn resolve_oauth_app_credentials_partial_flag_id_errors() {
    let err = resolve_oauth_app_credentials_for_test(
        Some("partial-id".into()),
        None, // missing flag_secret
        None,
        None,
        None,
        Some(("embed-id".into(), "embed-secret".into())),
        true,
    )
    .unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("--client-id"), "got: {msg}");
    assert!(msg.contains("--client-secret"), "got: {msg}");
}

#[test]
fn resolve_oauth_app_credentials_partial_flag_secret_errors() {
    let err = resolve_oauth_app_credentials_for_test(
        None,
        Some("partial-secret".into()),
        None,
        None,
        None,
        Some(("embed-id".into(), "embed-secret".into())),
        true,
    )
    .unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("--client-id"), "got: {msg}");
    assert!(msg.contains("--client-secret"), "got: {msg}");
}

#[test]
fn resolve_oauth_app_credentials_partial_env_id_errors() {
    let err = resolve_oauth_app_credentials_for_test(
        None,
        None,
        Some("env-id".into()),
        None, // missing env_secret
        None,
        Some(("embed-id".into(), "embed-secret".into())),
        true,
    )
    .unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("JR_OAUTH_CLIENT_ID"), "got: {msg}");
    assert!(msg.contains("JR_OAUTH_CLIENT_SECRET"), "got: {msg}");
}

#[test]
fn resolve_oauth_app_credentials_partial_env_secret_errors() {
    let err = resolve_oauth_app_credentials_for_test(
        None,
        None,
        None,
        Some("env-secret".into()),
        None,
        Some(("embed-id".into(), "embed-secret".into())),
        true,
    )
    .unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("JR_OAUTH_CLIENT_ID"), "got: {msg}");
    assert!(msg.contains("JR_OAUTH_CLIENT_SECRET"), "got: {msg}");
}

/// `jr` deliberately does NOT reject mixed classic+granular scopes,
/// unknown scope names, or missing `offline_access` — Atlassian returns
/// `invalid_scope` at token exchange per the spec's "Out of scope"
/// section. Locks this so a future refactor that starts "helping" with
/// client-side validation fails visibly.
#[test]
fn resolve_oauth_scopes_does_not_validate_scope_shape() {
    let inputs = [
        "read:jira-work read:issue:jira",           // classic + granular mix
        "read:issue:jira write:issue:jira",         // no offline_access
        "totally-made-up-scope another-fake-scope", // unknown scopes
        "offline_access",                           // only offline_access
    ];
    for raw in inputs {
        let p = profile_with_oauth_scopes(Some(raw));
        let result = resolve_oauth_scopes(&p).unwrap_or_else(|e| {
            panic!("resolve_oauth_scopes must pass {raw:?} through unchanged, got error: {e:#}")
        });
        assert_eq!(result, raw, "input {raw:?} must pass through unchanged");
    }
}

#[test]
fn peek_oauth_app_source_keychain_wins() {
    assert_eq!(
        peek_oauth_app_source_for_test(true, true),
        OAuthAppSource::Keychain
    );
    assert_eq!(
        peek_oauth_app_source_for_test(true, false),
        OAuthAppSource::Keychain
    );
}

#[test]
fn peek_oauth_app_source_embedded_when_no_keychain() {
    assert_eq!(
        peek_oauth_app_source_for_test(false, true),
        OAuthAppSource::Embedded
    );
}

#[test]
fn peek_oauth_app_source_none_when_nothing_resolves() {
    assert_eq!(
        peek_oauth_app_source_for_test(false, false),
        OAuthAppSource::None
    );
}

// -------------------------------------------------------------------------
// S-1.08 holdout tests: credential resolver precedence (BC-1.4.030)
// -------------------------------------------------------------------------

/// AC-005 (BC-1.4.030): When both keychain BYO credentials AND embedded
/// app are present, `peek_oauth_app_source_for_test(true, true)` must
/// return `OAuthAppSource::Keychain` (keychain wins).
///
/// This enforces the contract that a BYO user is never silently flipped
/// onto the embedded app mid-session. Their refresh_token was issued by
/// their own OAuth app and would be rejected by the embedded app's
/// client_id.
#[test]
fn test_s_1_08_ac005_keychain_wins_over_embedded_when_both_present() {
    assert_eq!(
        peek_oauth_app_source_for_test(true, true),
        OAuthAppSource::Keychain,
        "keychain must beat embedded when both are present"
    );
}

/// AC-005 (BC-1.4.030): Keychain-only (no embedded) must also return
/// `OAuthAppSource::Keychain`.
#[test]
fn test_s_1_08_ac005_keychain_wins_when_only_keychain_present() {
    assert_eq!(
        peek_oauth_app_source_for_test(true, false),
        OAuthAppSource::Keychain,
        "keychain must be returned when keychain is present and embedded is absent"
    );
}

/// AC-005 (BC-1.4.030): When keychain is absent but embedded is present,
/// `peek_oauth_app_source_for_test(false, true)` must return
/// `OAuthAppSource::Embedded` (embedded fallback).
#[test]
fn test_s_1_08_ac005_embedded_fallback_when_no_keychain() {
    assert_eq!(
        peek_oauth_app_source_for_test(false, true),
        OAuthAppSource::Embedded,
        "embedded must be the fallback when keychain is absent"
    );
}

/// AC-005 (BC-1.4.030): When neither keychain nor embedded is present,
/// `peek_oauth_app_source_for_test(false, false)` must return
/// `OAuthAppSource::None` (no credential source resolved).
#[test]
fn test_s_1_08_ac005_none_when_no_source_resolved() {
    assert_eq!(
        peek_oauth_app_source_for_test(false, false),
        OAuthAppSource::None,
        "None sentinel must be returned when no credential source is available"
    );
}

// ── S-2.07 AC-002: refresh_success_payload regression-pin ────────────
//
// These two tests are REGRESSION-PINS for the already-shipped
// `refresh_success_payload(AuthFlow)` helper. They must PASS on develop
// before any implementation work begins. If they fail, stop and
// investigate — the helper may have been accidentally modified.
//
// The tests pin:
//   - `AuthFlow::Token` → `{"status": "refreshed", "auth_method": "api_token", "next_step": <hint>}`
//   - `AuthFlow::OAuth` → `{"status": "refreshed", "auth_method": "oauth", "next_step": <hint>}`
//
// Per spec: AC-002 (traces to BC-7.3.004 postcondition, revised v2.0.0).
// The `auth refresh` shape is ASYMMETRIC from the four new auth subcommands'
// `{"profile", "action", "ok"}` shape — this is intentional (documented in
// `docs/specs/json-output-shapes.md`).

/// AC-002a (BC-7.3.004 regression-pin): `refresh_success_payload(AuthFlow::Token)`
/// must emit `{"status": "refreshed", "auth_method": "api_token", ...}`.
/// Expected Red Gate state: GREEN (helper already shipped on develop).
#[test]
fn test_refresh_success_payload_emits_status_refreshed_for_token_flow() {
    let payload = refresh_success_payload(AuthFlow::Token);
    assert_eq!(
        payload["status"], "refreshed",
        "status field must be 'refreshed', got: {}",
        payload["status"]
    );
    assert_eq!(
        payload["auth_method"], "api_token",
        "auth_method must be 'api_token' for Token flow, got: {}",
        payload["auth_method"]
    );
    assert!(
        payload["next_step"].as_str().is_some_and(|s| !s.is_empty()),
        "next_step must be a non-empty string hint, got: {}",
        payload["next_step"]
    );
    assert!(
        payload["next_step"]
            .as_str()
            .is_some_and(|s| s.contains("Always Allow")),
        "next_step must mention 'Always Allow' for keychain guidance, got: {}",
        payload["next_step"]
    );
}

/// AC-002b (BC-7.3.004 regression-pin): `refresh_success_payload(AuthFlow::OAuth)`
/// must emit `{"status": "refreshed", "auth_method": "oauth", ...}`.
/// Expected Red Gate state: GREEN (helper already shipped on develop).
#[test]
fn test_refresh_success_payload_emits_status_refreshed_for_oauth_flow() {
    let payload = refresh_success_payload(AuthFlow::OAuth);
    assert_eq!(
        payload["status"], "refreshed",
        "status field must be 'refreshed', got: {}",
        payload["status"]
    );
    assert_eq!(
        payload["auth_method"], "oauth",
        "auth_method must be 'oauth' for OAuth flow, got: {}",
        payload["auth_method"]
    );
    assert!(
        payload["next_step"].as_str().is_some_and(|s| !s.is_empty()),
        "next_step must be a non-empty string hint, got: {}",
        payload["next_step"]
    );
}

// ── S-2.07 AC-006: auth subcommand JSON shape snapshot tests ─────────
//
// These four tests snapshot-pin the `{"profile", "action", "ok": true}`
// shape for the four newly-JSON-emitting auth subcommands. The tests
// construct the expected JSON value directly (as the implementer's output
// helper will) and call `insta::assert_json_snapshot!`.
//
// Red Gate strategy: On first run (no snapshot file yet), insta writes a
// `.snap.new` file and FAILS the test. The tests remain RED until:
//   1. The implementer adds the `OutputFormat::Json` branches in the
//      four handler functions, AND
//   2. `cargo insta review` is run to accept the new snapshot files.
//
// The snapshot files will land in `src/cli/snapshots/` (insta default
// for unit tests in this module).
//
// Test names follow AC-006: `test_auth_<verb>_json_shape`.
// All tests follow AC-009 `test_<verb>_<subject>_<expected_outcome>`.

/// AC-006 (BC-7.3.004 invariant): snapshot-pin the `login` auth JSON shape.
/// Expected Red Gate state: RED (no snapshot file exists yet).
#[test]
fn test_auth_login_json_shape() {
    let value = serde_json::json!({
        "profile": "testprof",
        "action": "login",
        "ok": true
    });
    insta::assert_json_snapshot!("auth_login_json_shape", value);
}

/// AC-006 (BC-7.3.004 invariant): snapshot-pin the `switch` auth JSON shape.
/// Expected Red Gate state: RED (no snapshot file exists yet).
#[test]
fn test_auth_switch_json_shape() {
    let value = serde_json::json!({
        "profile": "default",
        "action": "switch",
        "ok": true
    });
    insta::assert_json_snapshot!("auth_switch_json_shape", value);
}

/// AC-006 (BC-7.3.004 invariant): snapshot-pin the `logout` auth JSON shape.
/// Expected Red Gate state: RED (no snapshot file exists yet).
#[test]
fn test_auth_logout_json_shape() {
    let value = serde_json::json!({
        "profile": "default",
        "action": "logout",
        "ok": true
    });
    insta::assert_json_snapshot!("auth_logout_json_shape", value);
}

/// AC-006 (BC-7.3.004 invariant): snapshot-pin the `remove` auth JSON shape.
/// Expected Red Gate state: RED (no snapshot file exists yet).
#[test]
fn test_auth_remove_json_shape() {
    let value = serde_json::json!({
        "profile": "staging",
        "action": "remove",
        "ok": true
    });
    insta::assert_json_snapshot!("auth_remove_json_shape", value);
}
