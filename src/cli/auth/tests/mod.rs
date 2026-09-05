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
    assert_eq!(
        chosen_flow_for_profile(&config.active_profile()),
        AuthFlow::Token
    );
}

#[test]
fn chosen_flow_uses_token_for_explicit_api_token() {
    let config = config_with_auth_method(Some("api_token"));
    assert_eq!(
        chosen_flow_for_profile(&config.active_profile()),
        AuthFlow::Token
    );
}

#[test]
fn chosen_flow_uses_oauth_when_config_says_so() {
    let config = config_with_auth_method(Some("oauth"));
    assert_eq!(
        chosen_flow_for_profile(&config.active_profile()),
        AuthFlow::OAuth
    );
}

// `chosen_flow_oauth_override_wins_over_config` (pre-S-cycle3-chosen-flow-
// reconcile) asserted the EXACT behavior DEC-321/BC-1.2.051 postcondition 3
// removes: an `--oauth` override forcing `AuthFlow::OAuth` on an api_token
// profile. `chosen_flow_for_profile` no longer takes an override parameter
// at all (compile-time enforced — there is no argument left to pass "true"
// for), so this test's premise no longer exists; it is removed rather than
// rewritten at this unit level, since the three tests above already fully
// cover AC-001 ("resolves solely from profile.auth_method"). The
// behavioral guarantee the old test's NAME implied to a reader — that a
// per-invocation flag cannot force a different mechanism — is now covered
// at the `refresh_credentials` integration level, where the flag actually
// lives: see `tests/auth_chosen_flow_reconcile.rs`'s
// `test_ac_002_refresh_oauth_flag_on_api_token_profile_uses_token_flow_not_oauth`
// and `test_vp_authdx_003_*` (VP-AUTHDX-003, BC-1.2.048).

/// Regression: refresh against a non-active profile must dispatch the
/// flow stored on THAT profile's auth_method, not the active profile's.
/// `chosen_flow_for_profile(&config.active_profile())` always reads the
/// active profile, which would silently pick the wrong flow if a caller
/// passed it when active=api_token but the refresh target=oauth (or
/// vice-versa). `chosen_flow_for_profile` takes the resolved target
/// profile so callers like `refresh_credentials` can thread the right
/// `ProfileConfig` in directly.
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
    // chosen_flow_for_profile(&config.active_profile()) returns Token
    // (active is api_token).
    assert_eq!(
        chosen_flow_for_profile(&config.active_profile()),
        AuthFlow::Token
    );
    // chosen_flow_for_profile against sandbox returns OAuth even though
    // the active profile is api_token — proves the resolver looks at
    // the passed profile, not the active one.
    let sandbox = config.global.profiles["sandbox"].clone();
    assert_eq!(chosen_flow_for_profile(&sandbox), AuthFlow::OAuth);
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
        "write:servicedesk-request",
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
                        read:servicedesk-request write:servicedesk-request \
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
            env: Some("prod".into()),
            ..ProfileConfig::default()
        },
    );
    profiles.insert(
        "staging".to_string(),
        ProfileConfig {
            url: Some("https://acme-staging.atlassian.net".into()),
            auth_method: Some("api_token".into()),
            env: Some("".into()),
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

// ── S-cycle3-env-tag: BC-6.1.015 / BC-1.6.046 / BC-1.6.047 — ENV tag ──
//
// Post-Green note: `render_env_column`/`sanitize_env_display` are now
// implemented, and `three_profile_fixture` (above) has been extended so
// `sandbox` carries `env: Some("prod")` (an ordinary value) and `staging`
// carries `env: Some("")` (the blank-cell case, distinct from `default`'s
// `env: None` `-` placeholder) — `list_table_snapshot`'s pinned insta
// snapshot was regenerated against this 5-column fixture via
// `cargo insta test --accept` and inspected to confirm the ENV column
// renders the real value, a blank cell, and the `-` placeholder correctly.
// The ENV column's behavior is additionally pinned below via direct,
// independent assertion tests (not just the snapshot).

/// BC-1.6.046 EC-1.6.046-1 / AC-005: `None` renders the "-" placeholder.
#[test]
fn test_render_env_column_none_renders_dash_placeholder() {
    assert_eq!(render_env_column(None), "-");
}

/// BC-1.6.046 EC-1.6.046-1 / AC-005: `Some("")` renders a BLANK cell —
/// zero visible characters — and must NOT be conflated with `None`'s "-"
/// placeholder.
#[test]
fn test_render_env_column_some_empty_renders_blank_not_dash() {
    let got = render_env_column(Some(""));
    assert_eq!(got, "");
    assert_ne!(
        got, "-",
        "Some(\"\") must not render as the None placeholder"
    );
}

/// BC-1.6.046 postcondition: an ordinary env value renders unchanged.
#[test]
fn test_render_env_column_ordinary_value_renders_unchanged() {
    assert_eq!(render_env_column(Some("prod")), "prod");
}

/// BC-1.6.046 EC-1.6.046-2 / AC-006: a hostile env value (control chars,
/// ANSI escapes) reaching the table column must be routed through the
/// shared `output::sanitize_env_display` transform — this pins the
/// CONTRACT (delegate to the shared sanitizer, byte-for-byte identical
/// output), while `output::tests::test_sanitize_env_display_*` pin the
/// transform's own exact stripping/capping behavior.
#[test]
fn test_render_env_column_hostile_value_delegates_to_shared_sanitizer() {
    let hostile = "\u{1b}[31mBAD\r\n\u{0}";
    let got = render_env_column(Some(hostile));
    assert_eq!(
        got,
        crate::output::sanitize_env_display(hostile),
        "table cell must use the identical shared sanitizer output"
    );
    assert!(!got.contains('\u{1b}'), "no raw ANSI escape in table cell");
    assert!(
        !got.chars().any(|c| (c as u32) <= 0x1F || c as u32 == 0x7F),
        "no raw control bytes in table cell: {got:?}"
    );
}

/// BC-1.6.046 AC-004: `jr auth list` (table mode) prints headers
/// `NAME, URL, ENV, AUTH, STATUS` in that order, ENV between URL and
/// AUTH. Uses an EMPTY profile map so this assertion is independent of
/// `render_env_column`'s (not-yet-implemented) per-row body — header
/// order is a property of `render_list_table` itself, not of any single
/// row's rendering, and must not be gated behind the Red stub.
#[test]
fn test_render_list_table_headers_include_env_between_url_and_auth() {
    let empty = GlobalConfig::default();
    let rendered = render_list_table(&empty, "default");
    let name_pos = rendered.find("NAME").expect("NAME header present");
    let url_pos = rendered.find("URL").expect("URL header present");
    let env_pos = rendered.find("ENV").expect("ENV header present");
    let auth_pos = rendered.find("AUTH").expect("AUTH header present");
    let status_pos = rendered.find("STATUS").expect("STATUS header present");
    assert!(
        name_pos < url_pos && url_pos < env_pos && env_pos < auth_pos && auth_pos < status_pos,
        "expected header order NAME < URL < ENV < AUTH < STATUS, got positions: \
         NAME={name_pos} URL={url_pos} ENV={env_pos} AUTH={auth_pos} STATUS={status_pos}\n{rendered}"
    );
}

/// BC-1.6.047 AC-007 / Postcondition 1: `auth list --output json` includes
/// "env" verbatim/lossless for every profile — the raw configured string
/// (including any control chars/ANSI escapes, unmodified) when set,
/// `null` when unset, and the key is NEVER omitted. This channel
/// deliberately differs from the table channel (issue #398's JSON-
/// verbatim/human-sanitized split, cited by BC-1.6.047 Invariant 3).
#[test]
fn test_render_list_json_env_key_is_verbatim_and_never_omitted() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "tagged".to_string(),
        ProfileConfig {
            url: Some("https://acme.atlassian.net".into()),
            env: Some("\u{1b}[31mprod\r\n".into()),
            ..ProfileConfig::default()
        },
    );
    profiles.insert(
        "untagged".to_string(),
        ProfileConfig {
            url: Some("https://acme2.atlassian.net".into()),
            env: None,
            ..ProfileConfig::default()
        },
    );
    profiles.insert(
        "empty-env".to_string(),
        ProfileConfig {
            url: Some("https://acme3.atlassian.net".into()),
            env: Some(String::new()),
            ..ProfileConfig::default()
        },
    );
    let global = GlobalConfig {
        default_profile: Some("tagged".into()),
        profiles,
        ..GlobalConfig::default()
    };
    let json = render_list_json(&global, "tagged").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = parsed.as_array().expect("array");
    assert_eq!(arr.len(), 3);

    for p in arr {
        assert!(
            p.get("env").is_some(),
            "\"env\" key must never be omitted from a profile object: {p}"
        );
    }

    let find = |name: &str| -> &serde_json::Value {
        arr.iter()
            .find(|p| p["name"] == name)
            .unwrap_or_else(|| panic!("profile {name} missing from JSON array"))
    };

    let tagged = find("tagged");
    assert_eq!(
        tagged["env"],
        serde_json::Value::String("\u{1b}[31mprod\r\n".to_string()),
        "hostile env value must be echoed byte-for-byte, no stripping/truncation"
    );

    let untagged = find("untagged");
    assert_eq!(
        untagged["env"],
        serde_json::Value::Null,
        "None must serialize to JSON null"
    );

    let empty_env = find("empty-env");
    assert_eq!(
        empty_env["env"],
        serde_json::Value::String(String::new()),
        "Some(\"\") must serialize to JSON \"\" (empty string), not null"
    );
}

/// BC-1.6.047 AC-008 / Postcondition 2b: `auth status`'s env line uses the
/// identical `None` -> "-" placeholder convention as the table's ENV
/// column (BC-1.6.046 EC-1.6.046-1).
#[test]
fn test_render_env_line_none_renders_dash_placeholder() {
    assert_eq!(render_env_line(None), "-");
}

/// BC-1.6.047 EC-1.6.047-3: `Some("")` renders blank, never "-" — same
/// convention as the table's ENV column.
#[test]
fn test_render_env_line_some_empty_renders_blank_not_dash() {
    let got = render_env_line(Some(""));
    assert_eq!(got, "");
    assert_ne!(got, "-");
}

/// BC-1.6.047 postcondition: an ordinary env value renders unchanged.
#[test]
fn test_render_env_line_ordinary_value_renders_unchanged() {
    assert_eq!(render_env_line(Some("uat")), "uat");
}

/// BC-1.6.047 EC-1.6.047-3: identical control-char/ANSI-strip + length-cap
/// transform as the table's ENV column — same shared sanitizer, same
/// contract, exercised at this call site too. Directly pins the
/// "one shared sanitizer, two call sites" architecture rule by asserting
/// `render_env_line` and `render_env_column` produce byte-identical
/// output for the same hostile input.
#[test]
fn test_render_env_line_hostile_value_delegates_to_shared_sanitizer() {
    let hostile = "\u{1b}[31mBAD\r\n\u{0}";
    let got = render_env_line(Some(hostile));
    assert_eq!(got, crate::output::sanitize_env_display(hostile));
    assert_eq!(
        got,
        render_env_column(Some(hostile)),
        "auth list's table cell and auth status's text line must use the \
         IDENTICAL transform output for the same hostile input — \"one \
         shared sanitizer, two call sites\""
    );
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

/// AC-016 (BC-1.3.023): `write:servicedesk-request` must be present in
/// `DEFAULT_OAUTH_SCOPES` so JSM request creation works for OAuth users.
///
/// RED GATE: This test FAILS until Step 4 adds the scope literal to
/// `DEFAULT_OAUTH_SCOPES` in `src/api/auth.rs`.
///
/// The existing `default_oauth_scopes_pins_the_full_set_with_offline_access`
/// test also covers the full set and will fail independently once the scope
/// is added without updating the expected string there.
#[test]
fn test_default_oauth_scopes_include_servicedesk_request() {
    assert!(
        crate::api::auth::DEFAULT_OAUTH_SCOPES
            .split_whitespace()
            .any(|s| s == "write:servicedesk-request"),
        "BC-1.3.023: DEFAULT_OAUTH_SCOPES must include write:servicedesk-request for JSM dispatch; got: {:?}",
        crate::api::auth::DEFAULT_OAUTH_SCOPES
    );
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

// ---------------------------------------------------------------------------
// PR #771 review Finding B-1 (BC-1.4.039): Site 1's honest-fail message
// recommends `jr auth logout`/`jr auth remove` as the DEFAULT cleanup for a
// brand-new profile's failed OAuth login -- but before this fix,
// `auth_method` was written only on a SUCCESSFUL `oauth_login`, so a
// profile whose login failed at the credential-store step was left with
// `auth_method: None`, which `jr auth logout` (via `auth_method_is_api_token`)
// treats as an api-token profile. These tests exercise the RECOMMENDED
// COMMANDS' ACTUAL ROUTING DECISIONS for that exact scenario, not merely
// string presence in the error message (the gap the PR #771 review found in
// `src/api/auth.rs::honest_fail_message_tests::
// test_bc_1_4_039_site1_dpapi_fallback_failed_recommends_scoped_cleanup_by_default`).
// ---------------------------------------------------------------------------

#[test]
fn should_mark_auth_method_before_attempt_true_for_brand_new_profile() {
    assert!(should_mark_auth_method_before_attempt(None, false));
}

#[test]
fn should_mark_auth_method_before_attempt_false_when_switching_from_established_method() {
    // FIX-F5-login-switch territory: a profile with a WORKING, different
    // mechanism on record must not be pre-marked -- see
    // `should_mark_auth_method_before_attempt`'s doc comment for why. This
    // must hold regardless of `has_stored_credentials`, since a `Some(_)`
    // `current_auth_method` short-circuits before that parameter is even
    // consulted.
    assert!(!should_mark_auth_method_before_attempt(
        Some("api_token"),
        false
    ));
    assert!(!should_mark_auth_method_before_attempt(
        Some("oauth"),
        false
    ));
    assert!(!should_mark_auth_method_before_attempt(
        Some("api_token"),
        true
    ));
}

/// PR #771 fresh-context re-review Finding NEW-1 (S-cycle4-honest-fail-message,
/// BC-1.4.039): `current_auth_method.is_none()` alone is an unsafe proxy for
/// "brand-new profile, nothing to protect" -- a profile migrated from the
/// legacy `[instance]` config shape can carry `auth_method: None` while
/// STILL holding working credentials under some label. This predicate must
/// NOT pre-mark such a profile even though its `current_auth_method` is
/// `None`.
#[test]
fn should_mark_auth_method_before_attempt_false_when_none_labelled_profile_has_stored_credentials()
{
    assert!(!should_mark_auth_method_before_attempt(None, true));
}

#[test]
fn mark_auth_method_if_new_sets_method_for_brand_new_profile() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "fresh".to_string(),
        crate::config::ProfileConfig {
            url: Some("https://fresh.example".into()),
            auth_method: None,
            ..crate::config::ProfileConfig::default()
        },
    );
    let global = crate::config::GlobalConfig {
        default_profile: Some("fresh".into()),
        profiles,
        ..crate::config::GlobalConfig::default()
    };
    let mutated = mark_auth_method_if_new(global, "fresh", None, "oauth", false);
    assert_eq!(
        mutated.profiles["fresh"].auth_method.as_deref(),
        Some("oauth")
    );
}

#[test]
fn mark_auth_method_if_new_leaves_switching_profile_untouched() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "existing".to_string(),
        crate::config::ProfileConfig {
            url: Some("https://existing.example".into()),
            auth_method: Some("api_token".into()),
            ..crate::config::ProfileConfig::default()
        },
    );
    let global = crate::config::GlobalConfig {
        default_profile: Some("existing".into()),
        profiles,
        ..crate::config::GlobalConfig::default()
    };
    let mutated = mark_auth_method_if_new(global, "existing", Some("api_token"), "oauth", false);
    assert_eq!(
        mutated.profiles["existing"].auth_method.as_deref(),
        Some("api_token"),
        "a mechanism SWITCH must not be pre-marked -- the profile's still-working prior \
         mechanism must remain on record until the new login actually succeeds"
    );
}

/// PR #771 fresh-context re-review Finding NEW-1 (S-cycle4-honest-fail-message,
/// BC-1.4.039): reproduces the exact regression the review found in the B-1
/// fix. A profile migrated from the legacy `[instance]` config shape can
/// have `auth_method: None` while STILL holding working api-token
/// credentials in the keychain (`profile_has_stored_credentials` would
/// return `true` for it). Before this fix, `mark_auth_method_if_new` would
/// still pre-mark such a profile as `"oauth"` ahead of a `jr auth login
/// --oauth` switch attempt -- if that attempt then failed partway through,
/// the profile was left labelled `"oauth"` with no OAuth credentials,
/// breaking it even though its pre-existing api-token credentials were
/// still perfectly valid. This test proves `auth_method` now stays `None`
/// (not mislabelled) in exactly that scenario, so the profile keeps working
/// via its existing credentials after a failed switch.
#[test]
fn mark_auth_method_if_new_leaves_legacy_none_labelled_profile_with_stored_credentials_untouched() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "legacy".to_string(),
        crate::config::ProfileConfig {
            url: Some("https://legacy.example".into()),
            // Legacy `[instance]`-migrated profile: `auth_method` was never
            // tracked, so it copies through as `None` -- even though the
            // profile still has working api-token credentials in the
            // keychain (simulated here via `has_stored_credentials = true`).
            auth_method: None,
            ..crate::config::ProfileConfig::default()
        },
    );
    let global = crate::config::GlobalConfig {
        default_profile: Some("legacy".into()),
        profiles,
        ..crate::config::GlobalConfig::default()
    };

    // Simulate `handle_login`'s probe finding working credentials already
    // stored under some label, then the OAuth switch attempt failing
    // partway through (e.g. at the credential-store step).
    let mutated = mark_auth_method_if_new(global, "legacy", None, "oauth", true);

    assert_eq!(
        mutated.profiles["legacy"].auth_method, None,
        "NEW-1 regression: a legacy None-labelled profile with WORKING credentials under \
         some label must not be pre-marked -- doing so mislabels the profile as \"oauth\" \
         with no OAuth credentials, orphaning its still-working api-token credentials, if \
         the switch attempt then fails"
    );
}

#[test]
fn auth_method_is_api_token_none_treated_as_api_token() {
    // BC-1.1.015: this predicate's `None` behavior is correct and
    // unchanged -- it is exactly the bug PR #771 review Finding B-1 traced:
    // before the fix, a brand-new profile whose OAuth login failed at the
    // store step was left with `auth_method: None`, which is (correctly,
    // per BC-1.1.015) treated as an api-token profile here. The fix is
    // `handle_login` pre-marking `auth_method` BEFORE attempting the flow
    // (see `mark_auth_method_if_new` above), not a change to this predicate.
    assert!(auth_method_is_api_token(None));
}

#[test]
fn auth_method_is_api_token_false_for_oauth() {
    assert!(!auth_method_is_api_token(Some("oauth")));
}

#[test]
fn auth_method_is_api_token_true_for_explicit_api_token() {
    assert!(auth_method_is_api_token(Some("api_token")));
}

/// End-to-end regression for PR #771 review Finding B-1 (BC-1.4.039):
/// simulates `handle_login`'s pre-mark step firing for a brand-new profile
/// (active AND `default_profile`, per `prepare_login_target`) whose
/// `--oauth` login then fails (e.g. Site 1's `DpapiFallbackFailed`), and
/// proves the message's TWO recommended cleanup commands' actual outcomes:
/// `jr auth logout` now routes to the OAuth-clear branch (not the
/// misleading "nothing to log out" branch), while `jr auth remove` is
/// STILL refused -- documenting exactly why the corrected message text
/// (see `src/api/auth.rs::site1_login_store_failure_message`) notes the
/// "not your active profile" caveat rather than recommending `jr auth
/// remove` unconditionally.
#[test]
fn b1_brand_new_oauth_profile_login_failure_logout_routes_to_oauth_branch() {
    let mut profiles = std::collections::BTreeMap::new();
    profiles.insert(
        "fresh".to_string(),
        crate::config::ProfileConfig {
            url: Some("https://fresh.example".into()),
            auth_method: None, // brand-new profile: no prior login ever completed
            ..crate::config::ProfileConfig::default()
        },
    );
    let global = crate::config::GlobalConfig {
        // `prepare_login_target` promotes a brand-new target to default_profile.
        default_profile: Some("fresh".into()),
        profiles,
        ..crate::config::GlobalConfig::default()
    };

    // Simulate handle_login's pre-mark step firing before the OAuth flow is
    // attempted, then the flow failing partway through (e.g. at the
    // credential-store step). `has_stored_credentials = false` because this
    // is a genuinely brand-new profile -- no credentials exist under any
    // label yet (distinguishing it from the NEW-1 regression scenario
    // covered by
    // `mark_auth_method_if_new_leaves_legacy_none_labelled_profile_with_stored_credentials_untouched`
    // above).
    let global = mark_auth_method_if_new(global, "fresh", None, "oauth", false);

    let auth_method = global.profiles["fresh"].auth_method.as_deref();
    assert!(
        !auth_method_is_api_token(auth_method),
        "B-1 regression: `jr auth logout` must route to the OAuth clear branch for a \
         profile whose OAuth login just failed -- got auth_method={auth_method:?}"
    );

    let remove_result = handle_remove_in_memory(global, "fresh", "fresh");
    assert!(
        remove_result.is_err(),
        "documents the residual: `jr auth remove` still refuses a profile that is both \
         active and default_profile, which a brand-new profile always is -- the corrected \
         message must not recommend `jr auth remove` unconditionally for this exact scenario"
    );
}
