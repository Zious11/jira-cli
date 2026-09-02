use crate::cli::OutputFormat;
use crate::error::JrError;
use crate::output;
use crate::profile::Profile;

use super::auth_json_response;

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
///
/// **Residual (SEC-2):** if `config.save_global()` fails AFTER
/// `clear_profile_creds` has already succeeded (step 2/3), the profile's
/// credentials are gone but `[profiles.<name>]` remains in `config.toml` —
/// this self-heals via the credential-absence-guard's exit-64 on the
/// profile's next use (S-cycle3-credential-absence-guard, BC-1.4.032/033),
/// rather than leaving it silently broken.
pub async fn handle_remove(
    target: &str,
    no_input: bool,
    cli_profile: Option<&str>,
    output: &OutputFormat,
) -> anyhow::Result<()> {
    let mut config = crate::config::Config::load_with(cli_profile)?;
    crate::config::validate_profile_name(target)?;
    // Boundary construction (BC-6.2.015, ADR-0011): `target` is a raw,
    // already-validated `&str` from the CLI; wrap it once here for every
    // downstream per-profile cache/credential call in this function.
    let target_profile = Profile::from(target);

    // Pre-validate against a clone before prompting so a typo or
    // unremovable target (active profile, default_profile target) doesn't
    // make the user click through a confirmation dialog only to error
    // afterward. Unaffected by this story's reorder — kept intact; this
    // call site is also what keeps `handle_remove_in_memory` reachable
    // outside `#[cfg(test)]`.
    let _ = handle_remove_in_memory(
        config.global.clone(),
        target,
        config.active_profile_name.as_ref(),
    )?;

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
            output::print_warning("Aborted.");
            return Ok(());
        }
    }

    // Steps 1/2 (BC-1.2.014 amended): OAuth-pair delete then
    // namespaced-API-token-pair delete, both via `clear_profile_creds`'s
    // amended contract. A genuine (non-`NoEntry`) keychain error propagates
    // via `?` HERE, before step 3 (cache clear) or step 4 (config-entry
    // removal) ever run — `[profiles.<target>]` remains in config.toml,
    // and a re-run of `jr auth remove <target>` is the documented recovery
    // path (AC-002/AC-003).
    crate::api::auth::clear_profile_creds(&target_profile)?;

    // Step 3 (best-effort, unchanged): cache-directory removal. A failure
    // here is surfaced as a warning, not an abort — a missing/unwritable
    // cache dir must not block the credential clear that already succeeded
    // above from reaching config-entry removal.
    if let Err(e) = crate::cache::clear_profile_cache(&target_profile) {
        let cache_path = crate::cache::cache_dir(&target_profile);
        output::print_warning(&format!(
            "cleared credentials but failed to clear cache for {target:?}: {e}. \
             Remove {} manually if disk space matters.",
            cache_path.display()
        ));
    }

    // Step 4 (LAST, BC-1.2.014 amended): config-entry removal, only after
    // steps 1/2 succeeded (or reported NoEntry).
    config.global =
        handle_remove_in_memory(config.global, target, config.active_profile_name.as_ref())?;
    config.save_global()?;

    if matches!(output, OutputFormat::Json) {
        println!(
            "{}",
            output::render_json(&auth_json_response(target, "remove"))?
        );
    } else {
        output::print_success(&format!("Removed profile {target:?}"));
    }
    Ok(())
}
