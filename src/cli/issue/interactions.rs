//! Interaction handlers: comment add/delete/edit/view.
//!
//! Extracted from `workflow.rs` by ADR-0012 Seam extraction (S-577-1 / PF-017).
//! `handle_comment_delete` and `handle_comment_edit` take `no_input: bool` as
//! their final parameter — these handlers have confirmation gates that consult
//! the no-input flag, mirroring the `handle_move`/`handle_assign`/`handle_edit`
//! pattern at `src/cli/issue/mod.rs`.

use anyhow::{Result, bail};

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{CommentSubcommand, OutputFormat};
use crate::output;

// ── Comment Add ──────────────────────────────────────────────────────────

/// Add a comment to an issue.
///
/// Relocated from `workflow.rs::handle_comment`; renamed to `handle_comment_add`
/// to match the new `CommentSubcommand::Add` variant (S-577-1).
///
/// Takes the full `CommentSubcommand::Add` variant so callers avoid exceeding
/// the `clippy::too_many_arguments` threshold (≤7 params), mirroring the
/// `handle_comment_edit` / `handle_move` / `handle_assign` pattern.
pub(super) async fn handle_comment_add(
    sub: CommentSubcommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let (key, message, markdown, file, stdin, internal) = match sub {
        CommentSubcommand::Add {
            key,
            message,
            markdown,
            file,
            stdin,
            internal,
        } => (key, message, markdown, file, stdin, internal),
        _ => unreachable!("handle_comment_add called with non-Add variant"),
    };

    // Resolve comment text from the various sources. spawn_blocking isolates
    // the blocking stdin read from the tokio runtime.
    let text = if stdin {
        tokio::task::spawn_blocking(|| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            Ok::<_, std::io::Error>(buf)
        })
        .await??
    } else if let Some(ref path) = file {
        std::fs::read_to_string(path)?
    } else if let Some(ref msg) = message {
        msg.clone()
    } else {
        bail!("Comment text is required. Use a positional argument, --file, or --stdin.");
    };

    let text = text.trim().to_string();
    if text.is_empty() {
        bail!("Comment text cannot be empty.");
    }

    let adf_body = if markdown {
        adf::markdown_to_adf(&text)?
    } else {
        adf::text_to_adf(&text)
    };

    let comment = client.add_comment(&key, adf_body, internal).await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&comment)?);
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Added comment to {} (id: {})",
                key,
                comment.id.as_deref().unwrap_or("unknown")
            ));
        }
    }

    Ok(())
}

// ── Comment Delete ───────────────────────────────────────────────────────

/// Delete a comment by ID — requires `--yes` or interactive confirmation.
///
/// Stub. Implementation delivered in S-577-3.
/// `_no_input` is the final parameter (confirmation-gate contract, mirrors
/// `handle_move`/`handle_assign`/`handle_edit` at `src/cli/issue/mod.rs`).
pub(super) async fn handle_comment_delete(
    _key: String,
    _id: String,
    _yes: bool,
    _output_format: &OutputFormat,
    _client: &JiraClient,
    _no_input: bool,
) -> Result<()> {
    todo!("comment delete — implemented in S-577-3")
}

// ── Comment Edit ─────────────────────────────────────────────────────────

/// Edit a comment body (optionally set visibility).
///
/// Stub. Implementation delivered in S-577-4/5.
/// Takes `_sub: CommentSubcommand` (the full `Edit` variant) so S-577-4/5 can
/// destructure individual fields from the owned enum value without a signature
/// change. Using a single enum parameter keeps the parameter count below the
/// `clippy::too_many_arguments` threshold (mirrors the `handle_move` /
/// `handle_assign` pattern in `workflow.rs`, which each receive the full
/// `IssueCommand`).
/// `_no_input` is the final parameter (confirmation-gate contract for the
/// `--public` visibility-change confirmation prompt).
pub(super) async fn handle_comment_edit(
    _sub: CommentSubcommand,
    _output_format: &OutputFormat,
    _client: &JiraClient,
    _no_input: bool,
) -> Result<()> {
    todo!("comment edit — implemented in S-577-4/S-577-5")
}

// ── Comment View ─────────────────────────────────────────────────────────

/// View a single comment by ID.
///
/// Stub. Implementation delivered in S-577-6.
/// No `no_input` parameter — read-only handler, no interactive prompt.
pub(super) async fn handle_comment_view(
    _key: String,
    _id: String,
    _output_format: &OutputFormat,
    _client: &JiraClient,
) -> Result<()> {
    todo!("comment view — implemented in S-577-6")
}
