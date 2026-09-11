use std::collections::HashMap;

use anyhow::Result;

use crate::api::auth::{AuthState, derive_auth_state, load_api_token, load_oauth_tokens};
use crate::output;
use crate::profile::Profile;

/// Render a profile's `env` tag for the `auth list` table's `ENV` column
/// (BC-1.6.046 EC-1.6.046-1): `None` -> `-` placeholder; `Some(s)` -> the
/// shared `output::sanitize_env_display` transform (`Some("")` renders as a
/// blank cell — zero visible characters — never the `-` placeholder; the
/// two are NOT conflated). Shares the sanitizer with `auth status`'s text
/// `env` line (`status.rs`) — see BC-1.6.046 Ownership clause / BC-1.6.047
/// EC-1.6.047-3: "one shared sanitizer, two call sites."
pub(crate) fn render_env_column(env: Option<&str>) -> String {
    match env {
        None => "-".to_string(),
        Some(s) => crate::output::sanitize_env_display(s),
    }
}

/// Select and invoke the kind-specific keychain probe for a profile.
///
/// `auth_method == "oauth"` → `load_oauth_tokens(profile).is_ok()`;
/// anything else (including `"api_token"`, `None`/legacy) →
/// `load_api_token(profile).is_ok()`.
///
/// This function is EFFECTFUL — it reads the OS keychain. It has no
/// in-memory injection seam and is NOT default-CI-testable by this story's
/// own tests (AC-005/006/009/010 inject `probe_results` or a counting
/// closure DIRECTLY, deliberately bypassing this function's body). It is
/// therefore `exclude_re`'d from `cargo-mutants` reporting per
/// S-cycle7-auth-state-derivation's Mutation Testing Scope section.
///
/// Named function (not anonymous closure) specifically so the `exclude_re`
/// can be file+function-name anchored (F3 adversary pass-3, finding MED-1 —
/// mirrors Story A's `load_api_token` exclusion design).
pub(crate) fn probe_matching_kind_credential(profile: &str, auth_method: &str) -> bool {
    let p = Profile::from(profile);
    if auth_method == "oauth" {
        load_oauth_tokens(&p).is_ok()
    } else {
        load_api_token(&p).is_ok()
    }
}

/// Collect per-profile `matching_kind_present` booleans by invoking the
/// given probe for each profile that has a URL configured.
///
/// Profiles with `url: None` are NEVER probed (BC-1.6.049 invariant 1).
///
/// `probe` is an injectable parameter — production code passes
/// `probe_matching_kind_credential`; tests pass a call-counting closure,
/// making the call-count / gating logic DEFAULT-CI-testable (AC-009).
///
/// Returns a `HashMap<String, bool>` keyed by profile name. Profiles with
/// `url: None` are absent from the map; the renderers treat an absent entry
/// as `matching_kind_present=false` (yielding `Unset` via
/// `derive_auth_state`, which is correct since `url` is `None` for those
/// profiles).
pub(crate) fn collect_probe_results<F>(
    global: &crate::config::GlobalConfig,
    probe: F,
) -> HashMap<String, bool>
where
    F: Fn(&str, &str) -> bool,
{
    let mut results = HashMap::new();
    for (name, p) in &global.profiles {
        if p.url.is_some() {
            let auth_method = p.auth_method.as_deref().unwrap_or("");
            let present = probe(name.as_str(), auth_method);
            results.insert(name.clone(), present);
        }
    }
    results
}

/// Render the table-form output of `jr auth list`. The active profile is
/// marked with a leading `*`; others get a leading space so column widths
/// stay stable across rows.
///
/// `probe_results` is a pre-computed per-profile `matching_kind_present`
/// bool map produced by `collect_probe_results` in `handle_list`. Profiles
/// absent from the map (URL=None profiles are never probed) receive
/// `matching_kind_present=false`, which `derive_auth_state` maps to
/// `Unset` (since url is also None for those profiles).
///
/// This renderer is PURE — it performs NO keychain access. All probing
/// happens in `handle_list` / `collect_probe_results` (F-1 fix,
/// S-cycle7-auth-state-derivation, BC-1.6.048 postcondition 2).
pub(crate) fn render_list_table(
    global: &crate::config::GlobalConfig,
    active: &str,
    probe_results: &HashMap<String, bool>,
) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (name, p) in &global.profiles {
        let marker = if name == active { "*" } else { " " };
        let auth = p.auth_method.as_deref().unwrap_or("?");
        let url = p.url.as_deref().unwrap_or("(unset)");
        let matching_kind_present = probe_results.get(name.as_str()).copied().unwrap_or(false);
        let status = match derive_auth_state(p.url.as_deref(), matching_kind_present) {
            AuthState::Unset => "unset",
            AuthState::NoCredentials => "no-credentials",
            AuthState::Configured => "configured",
        };
        rows.push(vec![
            format!("{marker} {name}"),
            url.to_string(),
            render_env_column(p.env.as_deref()),
            auth.to_string(),
            status.to_string(),
        ]);
    }
    crate::output::render_table(&["NAME", "URL", "ENV", "AUTH", "STATUS"], &rows)
}

/// Render the `--output json` form of `jr auth list`: an array of profile
/// objects keyed by name, with `active: true` on exactly one entry.
///
/// `probe_results` is a pre-computed per-profile `matching_kind_present`
/// bool map produced by `collect_probe_results` in `handle_list`. Profiles
/// absent from the map (URL=None profiles are never probed) receive
/// `matching_kind_present=false`.
///
/// This renderer is PURE — it performs NO keychain access. All probing
/// happens in `handle_list` / `collect_probe_results` (F-1 fix,
/// S-cycle7-auth-state-derivation, BC-1.6.048 postcondition 2).
pub(crate) fn render_list_json(
    global: &crate::config::GlobalConfig,
    active: &str,
    probe_results: &HashMap<String, bool>,
) -> Result<String> {
    let arr: Vec<serde_json::Value> = global
        .profiles
        .iter()
        .map(|(name, p)| {
            let matching_kind_present = probe_results.get(name.as_str()).copied().unwrap_or(false);
            let state = derive_auth_state(p.url.as_deref(), matching_kind_present);
            serde_json::json!({
                "name": name,
                "url": &p.url,
                "env": &p.env,
                "auth_method": &p.auth_method,
                "status": state,
                "active": name == active,
            })
        })
        .collect();
    output::render_json(&arr)
}

/// `jr auth list` — print every configured profile, marking the active one.
pub async fn handle_list(
    output: &crate::cli::OutputFormat,
    cli_profile: Option<&str>,
) -> Result<()> {
    let config = crate::config::Config::load_with(cli_profile)?;
    let probe_results = collect_probe_results(&config.global, probe_matching_kind_credential);
    let rendered = match output {
        crate::cli::OutputFormat::Table => render_list_table(
            &config.global,
            config.active_profile_name.as_ref(),
            &probe_results,
        ),
        crate::cli::OutputFormat::Json => render_list_json(
            &config.global,
            config.active_profile_name.as_ref(),
            &probe_results,
        )?,
    };
    println!("{rendered}");
    Ok(())
}
