use anyhow::Result;

use crate::api::auth;
use crate::api::auth_embedded::OAuthAppSource;
use crate::error::JrError;

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
        .unwrap_or_else(|| config.active_profile_name.clone());
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

    let method = profile
        .and_then(|p| p.auth_method.as_deref())
        .unwrap_or("(not configured)");
    println!("Auth method: {method}");

    // Credential probe: API-token creds are shared (one per host); OAuth
    // tokens are per-profile and namespaced by the profile name.
    let creds_ok = match method {
        "oauth" => auth::load_oauth_tokens(&target).is_ok(),
        _ => auth::load_api_token().is_ok(),
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
