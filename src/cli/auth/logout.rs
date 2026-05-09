use crate::cli::OutputFormat;
use crate::error::JrError;

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
pub async fn handle_logout(profile_arg: Option<&str>, output: &OutputFormat) -> anyhow::Result<()> {
    let config = crate::config::Config::load_with(profile_arg)?;
    let target = resolve_logout_target(&config.global, profile_arg, &config.active_profile_name);
    crate::config::validate_profile_name(&target)?;
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
    crate::api::auth::clear_profile_creds(&target)?;
    if matches!(output, OutputFormat::Json) {
        println!(
            "{}",
            serde_json::to_string_pretty(&auth_json_response(&target, "logout"))
                .expect("auth JSON response serialization cannot fail")
        );
    } else {
        crate::output::print_success(&format!("Logged out of profile {target:?}"));
    }
    Ok(())
}
