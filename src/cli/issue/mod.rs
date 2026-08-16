mod assets;
pub mod attachments;
mod changelog;
mod comments;
mod create;
mod edit;
mod field_resolve;
mod format;
mod helpers;
pub mod interactions;
mod jsm_create;
mod json_output;
mod links;
mod list;
mod view;
pub mod workflow;

pub use format::{format_issue_row, format_issue_rows_public, format_points, issue_table_headers};
// Re-exported for use by cli::component (BC-8.4.001 — resolve_component is the
// shared resolver; sibling cli modules cannot reach into cli::issue::helpers directly).
pub(crate) use helpers::resolve_component;

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{AttachmentSubcommand, CommentSubcommand, IssueCommand, OutputFormat};
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
            view::handle_view(command, output_format, config, client).await
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
            edit::handle_edit(command, output_format, config, client, no_input).await
        }
        IssueCommand::Move { .. } => {
            workflow::handle_move(command, output_format, client, no_input).await
        }
        IssueCommand::Transitions { .. } => {
            workflow::handle_transitions(command, output_format, client).await
        }
        IssueCommand::Resolutions { refresh } => {
            workflow::handle_resolutions(refresh, output_format, client).await
        }
        IssueCommand::Assign { .. } => {
            workflow::handle_assign(command, output_format, client, no_input).await
        }
        IssueCommand::Comment { command: sub } => match sub {
            // Pass the whole Add variant so handle_comment_add can destructure it
            // without exceeding the clippy::too_many_arguments threshold (mirrors
            // handle_comment_edit / handle_move / handle_assign pattern).
            sub @ CommentSubcommand::Add { .. } => {
                interactions::handle_comment_add(sub, output_format, client).await
            }
            CommentSubcommand::Delete { key, id, yes } => {
                interactions::handle_comment_delete(key, id, yes, output_format, client, no_input)
                    .await
            }
            // Pass the whole Edit variant so handle_comment_edit can destructure
            // individual fields (body-only shipped S-577-4; visibility flags consumed in S-577-5).
            sub @ CommentSubcommand::Edit { .. } => {
                interactions::handle_comment_edit(sub, output_format, client, no_input).await
            }
            CommentSubcommand::View { key, id } => {
                interactions::handle_comment_view(key, id, output_format, client).await
            }
        },
        IssueCommand::Comments { key, limit } => {
            comments::handle_comments(&key, limit, output_format, client).await
        }
        IssueCommand::Changelog { .. } => changelog::handle(command, output_format, client).await,
        IssueCommand::Open { .. } => workflow::handle_open(command, client).await,
        IssueCommand::Link { .. } => {
            links::handle_link(command, output_format, client, no_input).await
        }
        IssueCommand::Unlink { .. } => {
            links::handle_unlink(command, output_format, client, no_input).await
        }
        IssueCommand::RemoteLink { .. } => {
            links::handle_remote_link(command, output_format, client).await
        }
        IssueCommand::LinkTypes => links::handle_link_types(output_format, client).await,
        IssueCommand::Assets { key } => {
            assets::handle_issue_assets(&key, output_format, client).await
        }
        IssueCommand::Attachment { command: sub } => match sub {
            AttachmentSubcommand::List { key, filter } => {
                attachments::handle_attachment_list(&key, &filter, output_format, client).await
            }
            // S-576-2: single/batch/newest download + CWE-22 sanitization.
            // Pass the whole Download variant so handle_attachment_download can
            // destructure it without exceeding the clippy::too_many_arguments
            // threshold (mirrors handle_comment_add / handle_comment_edit pattern).
            sub @ AttachmentSubcommand::Download { .. } => {
                attachments::handle_attachment_download(sub, output_format, client).await
            }
            // S-576-3: multipart upload, --replace-existing, --dry-run path-c.
            // Pass the whole Upload variant so handle_attachment_upload can destructure
            // without exceeding the clippy::too_many_arguments threshold (mirrors
            // handle_attachment_download / handle_comment_edit pattern).
            sub @ AttachmentSubcommand::Upload { .. } => {
                attachments::handle_attachment_upload(sub, output_format, client, no_input).await
            }
            // S-576-4: single/bulk/older-than delete + --dry-run (EC-3.9.020-1/2/3).
            sub @ AttachmentSubcommand::Delete { .. } => {
                attachments::handle_attachment_delete(sub, output_format, client, no_input).await
            }
        },
    }
}
