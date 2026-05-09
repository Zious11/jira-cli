use crate::cli::OutputFormat;
use crate::config::Config;
use crate::error::JrError;

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
/// Order of operations:
/// 1. Confirm with the user (skipped under `--no-input`).
/// 2. Mutate config in-memory via [`handle_remove_in_memory`] (validates name,
///    refuses active profile, refuses unknown profile).
/// 3. Persist config first so a subsequent keychain/cache failure can't
///    leave the profile listed in `config.toml` after its credentials are
///    gone.
/// 4. Best-effort wipe of per-profile OAuth tokens and cache directory; both
///    are intentionally non-fatal — a missing keychain entry or cache dir is
///    the expected steady state for an already-cleaned profile, not an error.
pub async fn handle_remove(
    target: &str,
    no_input: bool,
    cli_profile: Option<&str>,
    output: &OutputFormat,
) -> anyhow::Result<()> {
    let mut config = Config::load_with(cli_profile)?;
    crate::config::validate_profile_name(target)?;

    // Pre-validate against a clone before prompting so a typo or
    // unremovable target (active profile, default_profile target) doesn't
    // make the user click through a confirmation dialog only to error
    // afterward. The actual mutation runs below against the real config.
    let _ = handle_remove_in_memory(config.global.clone(), target, &config.active_profile_name)?;

    if !no_input {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Permanently remove profile {target:?}? \
                 This deletes its config entry, cache, and OAuth tokens. \
                 Shared credentials remain."
            ))
            .default(false)
            .interact()?;
        if !confirm {
            crate::output::print_warning("Aborted.");
            return Ok(());
        }
    }

    config.global = handle_remove_in_memory(config.global, target, &config.active_profile_name)?;
    config.save_global()?;

    // Config entry is gone — that's the persistent state. The keychain
    // and cache cleanup is best-effort: failures here (permission denied,
    // locked keychain, IO) shouldn't unwind the config write, but the
    // user does need to know they have leftover state to clean up
    // manually. Surface as warnings; report overall success.
    if let Err(e) = crate::api::auth::clear_profile_creds(target) {
        crate::output::print_warning(&format!(
            "removed config entry but failed to clear OAuth tokens for {target:?}: {e}. \
             Remove the entries manually via your OS keychain UI."
        ));
    }
    if let Err(e) = crate::cache::clear_profile_cache(target) {
        let cache_path = crate::cache::cache_dir(target);
        crate::output::print_warning(&format!(
            "removed config entry but failed to clear cache for {target:?}: {e}. \
             Remove {} manually if disk space matters.",
            cache_path.display()
        ));
    }
    if matches!(output, OutputFormat::Json) {
        println!(
            "{}",
            serde_json::to_string_pretty(&auth_json_response(target, "remove"))
                .expect("auth JSON response serialization cannot fail")
        );
    } else {
        crate::output::print_success(&format!("Removed profile {target:?}"));
    }
    Ok(())
}
