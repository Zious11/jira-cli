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
use crate::error::JrError;
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

// ── Shared Validation ─────────────────────────────────────────────────────

/// Validates a comment ID argument per EC-3.5.002-1.
///
/// Accepts IDs matching `^[0-9A-Za-z_-]+$`; returns `Err(JrError::UserError)`
/// on mismatch.
///
/// No `regex` or `once_cell` crate needed — the pattern is fully covered by
/// `is_ascii_alphanumeric() || c == '_' || c == '-'` with an explicit
/// empty-string guard.
///
/// Shared by `handle_comment_delete`, `handle_comment_edit`, and
/// `handle_comment_view`.
fn validate_comment_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(JrError::UserError(format!("invalid comment id: {id}")).into());
    }
    Ok(())
}

// ── Comment Delete ───────────────────────────────────────────────────────

/// Delete a comment by ID — requires `--yes` or interactive confirmation.
///
/// Pipeline (BC-3.5.003 delete-pipeline ordering pin):
/// 1. `validate_comment_id` (EC-3.5.002-1) — exit 64 on bad charset
/// 2. Confirmation gate (BC-3.5.003):
///    - `--yes` → skip prompt
///    - `no_input && !yes` → exit 64 with pinned refusal wording
///    - else → interactive `y/N`: `eprint!` prompt to stderr + `io::stdin().lock().read_line()`.
///      Do NOT switch to `dialoguer::interact_on` — console's `is_term()` gate returns
///      `NotConnected` on piped stderr (DEC-174; empirically proven).
/// 3. HTTP DELETE — 204 → success; 404/403 → exit 64 + two-line body surface (BC-3.5.004)
///
/// `no_input` is the final parameter (confirmation-gate contract, mirrors
/// `handle_move`/`handle_assign`/`handle_edit` at `src/cli/issue/mod.rs`).
/// Test `no_input` ALONE — never re-derive `is_terminal()` here; the flag is
/// already resolved by `src/main.rs` with the `JR_STDIN_IS_TTY` seam applied.
pub(super) async fn handle_comment_delete(
    key: String,
    id: String,
    yes: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    // Step 1: validate --id charset (EC-3.5.002-1)
    validate_comment_id(&id)?;

    // Step 2: confirmation gate (BC-3.5.003)
    if no_input && !yes {
        return Err(JrError::UserError(format!(
            "Delete comment {id} on {key}? Use --yes to confirm."
        ))
        .into());
    }

    if !yes {
        // Interactive path: write prompt to stderr, read response from stdin.
        //
        // BC-3.5.003 delivery obligation: the prompt MUST go to stderr (not
        // stdout or /dev/tty) so it is captured in `--output json` subprocess
        // tests. Reading is done directly from `io::stdin()` rather than via
        // `dialoguer::Confirm::interact_on(&Term::stderr())` because the console
        // crate's `_interact_on` checks `term.is_term()` upfront and returns
        // `NotConnected` when stderr is piped (as it is in all subprocess tests),
        // making the dialoguer path non-functional for piped stdin/stderr.
        // Direct stdin reading achieves the identical behavioral contract:
        // prompt → stderr, y/N response from stdin, EOF → JrError::Interrupted.
        use std::io::BufRead;
        eprint!("Delete comment {id} on {key}? [y/N] ");
        let _ = std::io::Write::flush(&mut std::io::stderr());

        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => {
                // EOF or I/O error → Interrupted (EC-3.5.003-3, exit 130)
                return Err(JrError::Interrupted.into());
            }
            Ok(_) => {
                let answer = line.trim().to_ascii_lowercase();
                if answer != "y" && answer != "yes" {
                    // User cancelled (N, empty, any non-y input — default is N)
                    match output_format {
                        OutputFormat::Json => {
                            println!(
                                "{}",
                                output::render_json(&serde_json::json!({
                                    "cancelled": true,
                                    "deleted": false
                                }))?
                            );
                        }
                        OutputFormat::Table => {}
                    }
                    return Ok(());
                }
                // answer is "y" or "yes" → fall through to HTTP DELETE
            }
        }
    }

    // Step 3: HTTP DELETE
    match client.delete_comment(&key, &id).await {
        Ok(()) => {
            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        output::render_json(&serde_json::json!({
                            "deleted": true,
                            "id": id,
                            "key": key
                        }))?
                    );
                }
                OutputFormat::Table => {
                    output::print_success(&format!("Deleted comment {id} on {key}"));
                }
            }
            Ok(())
        }
        Err(e) => {
            // 404/403 → exit 64 + two-line body surface (BC-3.5.004, DEC-168 ruling 3).
            // 404 is NOT idempotent; re-wrap as UserError so main.rs emits exit 64.
            let user_err_msg = {
                match e.downcast_ref::<JrError>() {
                    Some(JrError::ApiError { status, message })
                        if *status == 404 || *status == 403 =>
                    {
                        Some(format!(
                            "comment not found or permission denied: {key}#{id}\n{}",
                            message
                        ))
                    }
                    _ => None,
                }
            };
            if let Some(msg) = user_err_msg {
                return Err(JrError::UserError(msg).into());
            }
            Err(e)
        }
    }
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

// ── Unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::validate_comment_id;

    // EC-3.5.002-1 charset validation — exercises every accepted token class
    // and all three arms of the `.all()` closure so mutation testing can kill
    // `||` → `&&` replacements in the condition.

    #[test]
    fn test_validate_comment_id_accepts_numeric() {
        assert!(validate_comment_id("10001").is_ok());
    }

    #[test]
    fn test_validate_comment_id_accepts_alphanumeric() {
        assert!(validate_comment_id("abc123").is_ok());
    }

    #[test]
    fn test_validate_comment_id_accepts_underscore() {
        // Exercises the `c == '_'` branch — kills `|| c == '_'` → `&& c == '_'` mutant.
        assert!(validate_comment_id("comment_id").is_ok());
    }

    #[test]
    fn test_validate_comment_id_accepts_hyphen() {
        // Exercises the `c == '-'` branch — kills `|| c == '-'` → `&& c == '-'` mutant.
        assert!(validate_comment_id("comment-id-1").is_ok());
    }

    #[test]
    fn test_validate_comment_id_accepts_mixed() {
        assert!(validate_comment_id("FOO-123_bar").is_ok());
    }

    #[test]
    fn test_validate_comment_id_rejects_empty() {
        // Exercises the `id.is_empty()` guard — kills `|| !id.chars()...` → `&& !id.chars()...`.
        assert!(validate_comment_id("").is_err());
    }

    #[test]
    fn test_validate_comment_id_rejects_slash() {
        assert!(validate_comment_id("../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_comment_id_rejects_space() {
        assert!(validate_comment_id("bad id").is_err());
    }

    #[test]
    fn test_validate_comment_id_rejects_dot() {
        assert!(validate_comment_id("id.with.dots").is_err());
    }
}
