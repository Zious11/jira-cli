use anyhow::Result;

use crate::api::auth;
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
///
/// # STUB — Red Gate scaffold (S-cycle7-auth-status-json)
///
/// This stub exists so the B2 test suite can compile during the Red Gate phase.
/// The implementer replaces `todo!()` with the real construction logic.
#[allow(unused_variables)]
pub(crate) fn build_status_json(
    profile: &str,
    url: Option<&str>,
    env: Option<&str>,
    auth_method: Option<&str>,
    matching_kind_present: bool,
    oauth_app: Option<&str>,
) -> anyhow::Result<String> {
    todo!("S-cycle7-auth-status-json: build_status_json not yet implemented")
}

/// Show authentication status: instance URL, auth method, credential availability.
///
/// When `profile_arg` is `Some`, reports for that profile. Otherwise reports
/// for the active profile (resolved via the usual flag → env → config →
/// "default" precedence chain at `Config::load` time).
pub async fn status(profile_arg: Option<&str>) -> Result<()> {
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
    let url = profile
        .and_then(|p| p.url.as_deref())
        .unwrap_or("(not configured)");
    println!("Profile:     {target}");
    println!("Instance:    {url}");
    println!(
        "Env:         {}",
        render_env_line(profile.and_then(|p| p.env.as_deref()))
    );

    let method = profile
        .and_then(|p| p.auth_method.as_deref())
        .unwrap_or("(not configured)");
    println!("Auth method: {method}");

    // Credential probe: both API-token and OAuth creds are namespaced by
    // profile as of S-cycle3-percred-storage (BC-1.4.031) — mirrors
    // `load_oauth_tokens`'s per-profile lookup.
    let target_profile = Profile::from(target.clone());
    let creds_ok = match method {
        "oauth" => auth::load_oauth_tokens(&target_profile).is_ok(),
        _ => auth::load_api_token(&target_profile).is_ok(),
    };
    if creds_ok {
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

    Ok(())
}
