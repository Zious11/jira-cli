mod create;
mod format;
mod helpers;
mod links;
mod list;
mod workflow;

pub use format::{format_issue_row, format_issue_rows_public, format_points, issue_table_headers};

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;

/// Handle all issue subcommands.
pub async fn handle(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    match command {
        IssueCommand::List { .. } => {
            list::handle_list(
                command,
                output_format,
                config,
                client,
                project_override,
                no_input,
            )
            .await
        }
        IssueCommand::View { .. } => {
            list::handle_view(command, output_format, config, client).await
        }
        IssueCommand::Create { .. } => {
            create::handle_create(
                command,
                output_format,
                config,
                client,
                project_override,
                no_input,
            )
            .await
        }
        IssueCommand::Edit { .. } => {
            create::handle_edit(command, output_format, config, client, no_input).await
        }
        IssueCommand::Move { .. } => {
            workflow::handle_move(command, output_format, client, no_input).await
        }
        IssueCommand::Transitions { .. } => {
            workflow::handle_transitions(command, output_format, client).await
        }
        IssueCommand::Assign { .. } => {
            workflow::handle_assign(command, output_format, client).await
        }
        IssueCommand::Comment { .. } => {
            workflow::handle_comment(command, output_format, client).await
        }
        IssueCommand::Open { .. } => workflow::handle_open(command, client).await,
        IssueCommand::Link { .. } => {
            links::handle_link(command, output_format, client, no_input).await
        }
        IssueCommand::Unlink { .. } => {
            links::handle_unlink(command, output_format, client, no_input).await
        }
        IssueCommand::LinkTypes => links::handle_link_types(output_format, client).await,
    }
}
