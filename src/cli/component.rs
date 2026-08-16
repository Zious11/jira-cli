use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::OutputFormat;
use crate::cli::issue::resolve_component;
use crate::config::Config;

use super::ComponentSubcommand;

/// Top-level dispatch for `jr component` subcommands.
pub async fn handle(
    command: ComponentSubcommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_flag: Option<&str>,
) -> Result<()> {
    match command {
        ComponentSubcommand::List { project, counts } => {
            handle_list(
                project.as_deref().or(project_flag),
                counts,
                output_format,
                config,
                client,
            )
            .await
        }
    }
}

/// Handle `jr component list [--project KEY] [--output json] [--counts]`.
///
/// BC-8.1.001 — BC-8.1.004.
/// Uses `resolve_component` (BC-8.4.001 — BC-8.4.005) for --component flag
/// resolution on future `issue create/edit` paths as well as this list command.
async fn handle_list(
    _project: Option<&str>,
    _counts: bool,
    _output_format: &OutputFormat,
    _config: &Config,
    _client: &JiraClient,
) -> Result<()> {
    // resolve_component will be called here in the implementation phase
    // (BC-8.4.001 — caller passes exactly one project's candidate list).
    let _ = resolve_component;
    todo!()
}
