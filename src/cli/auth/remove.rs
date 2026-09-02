use crate::cli::OutputFormat;
use crate::error::JrError;

/// Pure logic for `jr auth remove` — separated for testing without filesystem
/// or keychain. Returns the mutated `GlobalConfig` with `target` removed from
/// `profiles`. Refuses to remove the active profile (caller must switch first)
/// or unknown profiles. The cache directory and per-profile OAuth tokens are
/// cleared by [`handle_remove`] after the in-memory mutation succeeds; this
/// function only owns the config-shape transition.
pub(crate) fn handle_remove_in_memory(
    mut global: crate::config::GlobalConfig,
    target: &str,
    active: &str,
) -> anyhow::Result<crate::config::GlobalConfig> {
    crate::config::validate_profile_name(target)?;
    if !global.profiles.contains_key(target) {
        let known: Vec<&str> = global.profiles.keys().map(String::as_str).collect();
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
    if target == active {
        return Err(JrError::UserError(format!(
            "cannot remove active profile {target:?}; switch first with \"jr auth switch <other>\""
        ))
        .into());
    }
    // Also refuse if `target` is the persisted default_profile, even when
    // not the *current* active (e.g., `jr --profile sandbox auth remove
    // default` where active=sandbox but default_profile=default). Removing
    // the profile that default_profile points to leaves config.toml in a
    // broken state — strict Config::load() afterward would error with
    // "active profile 'default' not in [profiles]" until the user manually
    // edits the file.
    if global.default_profile.as_deref() == Some(target) {
        return Err(JrError::UserError(format!(
            "cannot remove profile {target:?}: it is the default_profile in config. \
             Switch the default first with \"jr auth switch <other>\"."
        ))
        .into());
    }
    global.profiles.remove(target);
    Ok(global)
}

/// `jr auth remove <name>` — permanently delete a profile.
///
/// **AMENDED by S-cycle3-remove-logout-semantics (BC-1.2.014, DEC-322) —
/// REORDERED, and a genuine (non-`NoEntry`) keychain error now ABORTS the
/// command instead of degrading to a warning.**
///
/// New order of operations (credentials BEFORE config entry, I-4/SR-008 —
/// this is the OPPOSITE of the prior ordering documented below):
/// 1. Confirm with the user (skipped under `--no-input`).
/// 2. OAuth-pair delete (via [`crate::api::auth::clear_profile_creds`]).
/// 3. API-token-pair delete (namespaced `<profile>:email`/`<profile>:api-token`,
///    also via [`crate::api::auth::clear_profile_creds`]'s amended contract).
/// 4. Cache-directory removal — stays best-effort (unchanged from the prior
///    behavior; only the two credential-deletion steps gained the
///    abort-on-genuine-error tightening).
/// 5. Config-entry removal — LAST, only after steps 2/3 succeed (or report
///    `NoEntry`). A genuine keychain error on step 2 or 3 aborts BEFORE
///    steps 4/5 run, surfaced to the user, non-zero exit — `[profiles.<name>]`
///    remains in `config.toml`, and a re-run of `jr auth remove <name>` is
///    the documented recovery path (AC-002/AC-003/AC-004/AC-005).
///
/// **Historical rationale (PRE-AMENDMENT, retained for context — no longer
/// the operative behavior):** the original ordering persisted the config
/// removal FIRST "so a subsequent keychain/cache failure can't leave the
/// profile listed after its credentials are gone." BC-1.2.014 (amended)
/// deliberately reverses this priority: a partial failure must now leave
/// the profile config entry intact and RE-REMOVABLE, rather than un-listed
/// with orphaned credentials.
pub async fn handle_remove(
    target: &str,
    no_input: bool,
    cli_profile: Option<&str>,
    output: &OutputFormat,
) -> anyhow::Result<()> {
    let config = crate::config::Config::load_with(cli_profile)?;
    crate::config::validate_profile_name(target)?;

    // Pre-validate against a clone before prompting so a typo or
    // unremovable target (active profile, default_profile target) doesn't
    // make the user click through a confirmation dialog only to error
    // afterward. Unaffected by this story's reorder — kept intact; this
    // call site is also what keeps `handle_remove_in_memory` reachable
    // outside `#[cfg(test)]`.
    let _ = handle_remove_in_memory(config.global.clone(), target, &config.active_profile_name)?;

    if !no_input {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Permanently remove profile {target:?}? \
                 This deletes its config entry, cache, and stored \
                 credentials (OAuth and API-token). Shared credentials \
                 remain."
            ))
            .default(false)
            .interact()?;
        if !confirm {
            crate::output::print_warning("Aborted.");
            return Ok(());
        }
    }

    let _ = (config, output);
    todo!(
        "S-cycle3-remove-logout-semantics (BC-1.2.014): reorder to \
         (1) OAuth-pair delete, (2) API-token-pair delete, \
         (3) cache clear (best-effort), (4) config-entry removal LAST; \
         a genuine (non-NoEntry) keychain error from step 1 or 2 must \
         abort before steps 3/4 run, surfaced to the user (non-zero exit), \
         leaving [profiles.{target}] intact for a re-run."
    )
}
