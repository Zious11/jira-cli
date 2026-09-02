use crate::cli::OutputFormat;

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
/// changes: when the target profile's `auth_method` is `"api_token"` (or,
/// by the intrinsic-mechanism read, any profile with no OAuth pair to
/// clear), `logout` no longer silently no-ops — it prints an
/// INFORMATIONAL, non-error stderr notice and exits 0:
///
/// `"This profile uses API-token auth — nothing to log out; use \`jr auth
/// remove <profile>\` to delete stored credentials."` (profile name
/// interpolated).
///
/// This notice is stderr-only and NEVER appears on stdout in any mode;
/// under `--output json` the stdout payload shape is unchanged from the
/// pre-fix no-op behavior (AC-006/AC-007). On an `oauth`-method profile,
/// behavior is UNCHANGED — the OAuth pair is deleted via
/// [`crate::api::auth::clear_profile_creds`] and the ordinary success
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

    let _ = output;
    todo!(
        "S-cycle3-remove-logout-semantics (BC-1.2.013): branch on \
         {target:?}'s auth_method — on \"oauth\", unchanged behavior \
         (delete OAuth pair via clear_profile_creds, ordinary success \
         message/JSON envelope); on \"api_token\", print the exact \
         informational stderr notice ('This profile uses API-token auth — \
         nothing to log out; use `jr auth remove <profile>` to delete \
         stored credentials.') and exit 0, skipping clear_profile_creds \
         and the generic success message but preserving the JSON payload \
         shape unchanged from the pre-fix no-op behavior."
    )
}
