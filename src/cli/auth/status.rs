use anyhow::Result;

use crate::api::auth;
use crate::api::auth::derive_auth_state;
use crate::api::auth_embedded::OAuthAppSource;
use crate::error::JrError;
use crate::profile::Profile;

/// Render a profile's `env` tag for `auth status`'s text `env` line
/// (BC-1.6.047 Postcondition 2b, EC-1.6.047-3): identical control-char/
/// ANSI-strip + length-cap transform and `-`/blank placeholder convention
/// as `auth list`'s `ENV` table column (BC-1.6.046 EC-1.6.046-1) — see
/// `cli::auth::list::render_env_column`'s doc comment for the full
/// contract. Shares the sanitizer with that call site (`output::
/// sanitize_env_display`) per the "one shared sanitizer, two call sites"
/// architecture rule.
pub(crate) fn render_env_line(env: Option<&str>) -> String {
    match env {
        None => "-".to_string(),
        Some(s) => crate::output::sanitize_env_display(s),
    }
}

/// Inspect — without consuming or modifying — which source would supply
/// OAuth app credentials on the next `refresh_oauth_token` call. Mirrors
/// the resolver order in `api/auth.rs::resolve_refresh_app_credentials`.
///
/// On keychain probe failure (locked keychain, permission denied) emits
/// a stderr warning and falls through to the next source in the chain.
/// The status row may therefore display `embedded` when the keychain is
/// merely temporarily inaccessible — that's defensible for a status
/// surface (display non-blocking, keep `auth status` usable) but it
/// diverges from `resolve_refresh_app_credentials`, which hard-errors on
/// the same condition. The stderr warning is the user's signal that the
/// row may be incomplete.
fn peek_oauth_app_source() -> OAuthAppSource {
    let keychain_present = match crate::api::auth::try_load_oauth_app_credentials() {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            eprintln!(
                "warning: could not read keychain for OAuth app credentials ({e:#}); \
                 status report may be incomplete."
            );
            false
        }
    };
    let embedded_present = crate::api::auth_embedded::embedded_oauth_app_present();
    peek_oauth_app_source_for_test(keychain_present, embedded_present)
}

/// Pure helper for testing the precedence chain. Match the runtime
/// resolver: keychain wins, embedded falls back, otherwise returns
/// `OAuthAppSource::None` (the explicit sentinel variant for "no source
/// resolved", not Rust's `Option::None`).
pub(crate) fn peek_oauth_app_source_for_test(
    keychain_present: bool,
    embedded_present: bool,
) -> OAuthAppSource {
    if keychain_present {
        return OAuthAppSource::Keychain;
    }
    if embedded_present {
        return OAuthAppSource::Embedded;
    }
    OAuthAppSource::None
}

/// Select and invoke the kind-specific keychain probe for `auth status`.
///
/// `auth_method == "oauth"` → `load_oauth_tokens(profile).is_ok()`;
/// anything else (including `"api_token"`, `None`/legacy) →
/// `load_api_token(profile).is_ok()`.
///
/// This function is EFFECTFUL — it reads the OS keychain. It has no
/// in-memory injection seam and is NOT default-CI-testable by DEFAULT-CI
/// tests. It is therefore `exclude_re`'d from `cargo-mutants` reporting
/// (S-cycle7-auth-status-json B2 Mutation Testing Scope).
///
/// Named function (not anonymous closure) specifically so the `exclude_re`
/// can be file+function-name anchored — mirrors `list.rs`'s
/// `probe_matching_kind_credential` pattern.
fn probe_matching_kind_credential(profile: &Profile, method: &str) -> bool {
    if method == "oauth" {
        auth::load_oauth_tokens(profile).is_ok()
    } else {
        auth::load_api_token(profile).is_ok()
    }
}

/// Build the 6-key JSON object for `auth status --output json` (BC-1.6.050).
///
/// **PURE** — performs NO keychain access and NO config load. All inputs are
/// pre-resolved by the effectful `status()` caller: it computes
/// `matching_kind_present` ONCE via `probe_matching_kind_credential` (the
/// same single kind-specific probe that drives the human-text `Credentials:`
/// line), then feeds it here alongside the other pre-resolved fields.
///
/// Fields emitted (set-equality, BC-1.6.050 postcondition 1):
/// - `profile`:     the active profile name
/// - `url`:         `null` when URL is unset, else the verbatim URL string
/// - `env`:         `null` when env is unset, else verbatim (no sanitization)
/// - `auth_method`: `null` when not configured, else the stored method string
/// - `status`:      3-state vocabulary from `derive_auth_state`
/// - `oauth_app`:   `null` when `auth_method != "oauth"`, else the source label
///
/// Output is pretty-printed via `output::render_json` (the #526 invariant).
pub(crate) fn build_status_json(
    profile: &str,
    url: Option<&str>,
    env: Option<&str>,
    auth_method: Option<&str>,
    matching_kind_present: bool,
    oauth_app: Option<&str>,
) -> anyhow::Result<String> {
    let state = derive_auth_state(url, matching_kind_present);
    let obj = serde_json::json!({
        "profile": profile,
        "url": url,
        "env": env,
        "auth_method": auth_method,
        "status": state,
        "oauth_app": oauth_app,
    });
    crate::output::render_json(&obj)
}

/// Show authentication status: instance URL, auth method, credential availability.
///
/// When `profile_arg` is `Some`, reports for that profile. Otherwise reports
/// for the active profile (resolved via the usual flag → env → config →
/// "default" precedence chain at `Config::load` time).
pub async fn status(profile_arg: Option<&str>, output: &crate::cli::OutputFormat) -> Result<()> {
    // `profile_arg` is the explicit per-subcommand override (`--profile`
    // on `auth status`); when absent we still let Config::load apply the
    // standard precedence chain (env > default_profile > "default").
    // Passing `profile_arg` here also doubles as the CLI-flag override
    // for `Config::load_with`, ensuring a `jr auth status --profile X`
    // against an unconfigured X surfaces a clear "unknown profile" error
    // from the strict load instead of silently falling back to the
    // active profile.
    let config = crate::config::Config::load_with(profile_arg)?;
    let target = profile_arg
        .map(str::to_string)
        .unwrap_or_else(|| config.active_profile_name.to_string());
    crate::config::validate_profile_name(&target)?;

    // Special-case: fresh install with no profiles yet AND no explicit
    // `--profile` was passed. `jr auth status` is a legitimate probe
    // used by setup scripts / CI / agents to detect first-run state.
    // Erroring here would block that probe — the user hasn't configured
    // anything yet, so "unknown profile" would be misleading.
    //
    // BUT if the user explicitly named a profile via `--profile X`, take
    // the strict path below — they're asserting X exists, and silently
    // succeeding with a generic "no profiles configured" message would
    // hide the mismatch. Matches the strict behavior of switch/remove/
    // logout for explicit profile targets.
    if config.global.profiles.is_empty() && profile_arg.is_none() {
        eprintln!(
            "No profiles configured. Run `jr init` or \
             `jr auth login --profile <NAME>` to set up."
        );
        return Ok(());
    }

    // Refuse to "succeed" against a profile the user never configured —
    // matches the strict behavior of switch/remove/logout. Without this,
    // `jr auth status --profile typo` printed "(not configured)" for
    // every field and exited 0, hiding the typo.
    if !config.global.profiles.contains_key(&target) {
        let known: Vec<&str> = config.global.profiles.keys().map(String::as_str).collect();
        return Err(JrError::UserError(format!(
            "unknown profile: {target}; known: {}",
            if known.is_empty() {
                "(none)".into()
            } else {
                known.join(", ")
            }
        ))
        .into());
    }

    let profile = config.global.profiles.get(&target);
    let url_str = profile.and_then(|p| p.url.as_deref());
    let env_str = profile.and_then(|p| p.env.as_deref());
    let method_str = profile.and_then(|p| p.auth_method.as_deref());
    let method = method_str.unwrap_or("(not configured)");

    // Credential probe: both API-token and OAuth creds are namespaced by
    // profile as of S-cycle3-percred-storage (BC-1.4.031) — mirrors
    // `load_oauth_tokens`'s per-profile lookup.
    let target_profile = Profile::from(target.clone());
    let matching_kind_present = probe_matching_kind_credential(&target_profile, method);

    match output {
        crate::cli::OutputFormat::Json => {
            // Report which OAuth app credentials would be used for the next
            // refresh. Only relevant (and non-null in JSON) for oauth profiles.
            let oauth_app_label = if method == "oauth" {
                Some(peek_oauth_app_source().label())
            } else {
                None
            };
            let json = build_status_json(
                &target,
                url_str,
                env_str,
                method_str,
                matching_kind_present,
                oauth_app_label,
            )?;
            println!("{json}");
        }
        crate::cli::OutputFormat::Table => {
            // Human-text output — byte-for-byte unchanged (BC-1.6.050
            // Postcondition 6). The `Credentials:` line is a 2-valued
            // projection of `matching_kind_present`; the `url:None` +
            // credential-present divergence (EC-1.6.050-4) is intentional.
            let url = url_str.unwrap_or("(not configured)");
            println!("Profile:     {target}");
            println!("Instance:    {url}");
            println!(
                "Env:         {}",
                render_env_line(env_str)
            );
            println!("Auth method: {method}");
            if matching_kind_present {
                println!("Credentials: stored in keychain");
            } else {
                println!("Credentials: not found");
            }
            // Report which OAuth app credentials would be used for the next refresh.
            // This is the *future* source — same resolver as `refresh_oauth_token`.
            if method == "oauth" {
                let source = peek_oauth_app_source();
                println!("OAuth app:   {}", source.label());
            }
        }
    }

    Ok(())
}
