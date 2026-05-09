use anyhow::Result;

use crate::cli::OutputFormat;
use crate::config::Config;
use crate::error::JrError;
use crate::output;

use super::auth_json_response;

/// Pure logic for `jr auth switch` — separated for testing without filesystem.
pub(crate) fn handle_switch_in_memory(
    mut global: crate::config::GlobalConfig,
    target: &str,
) -> Result<crate::config::GlobalConfig> {
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
    global.default_profile = Some(target.to_string());
    Ok(global)
}

/// `jr auth switch <name>` — set the default profile in `config.toml`.
pub async fn handle_switch(
    target: &str,
    cli_profile: Option<&str>,
    output: &OutputFormat,
) -> Result<()> {
    let mut config = Config::load_with(cli_profile)?;
    config.global = handle_switch_in_memory(config.global, target)?;
    config.save_global()?;
    if matches!(output, OutputFormat::Json) {
        println!(
            "{}",
            serde_json::to_string_pretty(&auth_json_response(target, "switch"))
                .expect("auth JSON response serialization cannot fail")
        );
    } else {
        output::print_success(&format!("Active profile set to {target:?}"));
    }
    Ok(())
}
