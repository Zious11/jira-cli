use crate::cli::OutputFormat;
use crate::output;

use super::auth_json_response;

/// Pure resolver for `jr auth logout`. Defaults to the active profile when
/// the user passes no `--profile`. Kept module-private and split out so the
/// CLI default behavior is unit-testable without filesystem or keychain.
pub(crate) fn resolve_logout_target(
    _global: &crate::config::GlobalConfig,
    profile_arg: Option<&str>,
    active: &str,
) -> String {
    profile_arg.unwrap_or(active).to_string()
}

/// `jr auth logout [--profile <name>]` — clear OAuth tokens for the target
/// profile. The profile entry in `config.toml` is left in place so a follow-up
/// `jr auth login --profile <name>` re-authenticates without losing site
/// metadata. The shared API-token credential is intentionally NOT cleared
/// (it's keyed by host, not profile, so wiping it would log every profile
/// out of API-token mode).
///
/// **AMENDED by S-cycle3-remove-logout-semantics (BC-1.2.013, DEC-322).**
/// `logout` remains OAuth-specific by design — this story does NOT extend
/// it to clear the target profile's API-token credentials (that is
/// `jr auth remove`'s job; see [`super::remove::handle_remove`]). What
/// changes: when the target profile's `auth_method` is `"api_token"`,
/// `logout` no longer silently no-ops — it prints an INFORMATIONAL,
/// non-error stderr notice and exits 0:
///
/// `"This profile uses API-token auth — nothing to log out; use \`jr auth
/// remove <profile>\` to delete stored credentials."` (profile name
/// interpolated).
///
/// This notice is stderr-only and NEVER appears on stdout in any mode;
/// under `--output json` the stdout payload shape is unchanged from the
/// pre-fix no-op behavior (AC-006/AC-007). On an `oauth`-method profile,
/// behavior is UNCHANGED — the OAuth pair is deleted via
/// [`crate::api::auth::clear_profile_oauth_pair`] (NOT
/// [`crate::api::auth::clear_profile_creds`], which also clears the
/// API-token pair — `logout` must never touch that) and the ordinary success
/// message/JSON envelope is emitted, exactly as before (AC-008, regression
/// pin). This notice text must stay consistent with
/// `S-cycle3-credential-absence-guard`'s BC-1.4.033 SR-009 remediation-text
/// fix, which deliberately never names `jr auth logout` as a remediation —
/// this notice is the reason why: `logout` is a no-op for API-token
/// profiles.
pub async fn handle_logout(profile_arg: Option<&str>, output: &OutputFormat) -> anyhow::Result<()> {
    let config = crate::config::Config::load_with(profile_arg)?;
    let target = resolve_logout_target(&config.global, profile_arg, &config.active_profile_name);
    crate::config::validate_profile_name(&target)?;
    if !config.global.profiles.contains_key(&target) {
        let known: Vec<&str> = config.global.profiles.keys().map(String::as_str).collect();
        return Err(crate::error::JrError::UserError(format!(
            "unknown profile: {target}; known: {}",
            if known.is_empty() {
                "(none)".into()
            } else {
                known.join(", ")
            }
        ))
        .into());
    }

    let is_api_token_profile = config
        .global
        .profiles
        .get(&target)
        .and_then(|p| p.auth_method.as_deref())
        == Some("api_token");

    if is_api_token_profile {
        // BC-1.2.013 amended (DEC-322): api-token profiles have no OAuth
        // session to clear. Informational, non-error notice — exit 0.
        // Stderr-only; never appears on stdout in any output mode
        // (AC-006/AC-007). Do NOT call clear_profile_creds here — that
        // would clear the API-token pair too, which is `auth remove`'s
        // job, not `logout`'s (BC-1.2.013 non-destructive contract).
        eprintln!(
            "This profile uses API-token auth — nothing to log out; use \
             `jr auth remove {target}` to delete stored credentials."
        );
    } else {
        // Unchanged behavior for oauth (and any other/unset) profiles
        // (AC-008 regression pin). Uses clear_profile_oauth_pair, NOT
        // clear_profile_creds — logout must never clear the API-token
        // pair, even if this profile happens to carry a leftover one
        // from a prior mechanism switch (BC-1.2.013 non-destructive
        // contract).
        crate::api::auth::clear_profile_oauth_pair(&target)?;
    }

    if matches!(output, OutputFormat::Json) {
        // JSON payload shape is unchanged from the pre-fix no-op behavior
        // (AC-007) regardless of which branch above ran.
        println!(
            "{}",
            output::render_json(&auth_json_response(&target, "logout"))?
        );
    } else if !is_api_token_profile {
        output::print_success(&format!("Logged out of profile {target:?}"));
    }
    Ok(())
}
