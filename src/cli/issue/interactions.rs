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

// ── Comment View Format Helpers ──────────────────────────────────────────

/// Format the `Restricted:` field (field 6) using the 4-rung ladder.
///
/// Rung ordering (non-negotiable per BC-3.5.010):
/// (a) role/group with non-empty value → `<value>`
/// (b) role/group with empty value, non-empty identifier → `id=<identifier>`
/// (c) non-role/group type — value-OR-identifier preference:
///     non-empty value → `<type>:<value>`
///     empty value + non-empty identifier → `<type>:<identifier>`
/// (d) fallback → `None`
///
/// `visibility` is `response.get("visibility")` — `Some(v)` when the key is
/// present (even if the JSON value is null), `None` when the key is absent entirely.
fn format_restricted_field(visibility: Option<&serde_json::Value>) -> String {
    match visibility {
        None => "None".to_string(),
        Some(v) => {
            let vtype = v["type"].as_str().unwrap_or("");
            let value = v["value"].as_str().unwrap_or("");
            let identifier = v["identifier"].as_str().unwrap_or("");
            match (vtype, value.is_empty(), identifier.is_empty()) {
                // Rung (a): role/group with non-empty value
                ("role" | "group", false, _) => value.to_string(),
                // Rung (b): role/group with empty value, non-empty identifier
                ("role" | "group", true, false) => format!("id={identifier}"),
                // Rung (c): non-role/group type — value-OR-identifier preference
                (t, _, _) if !t.is_empty() && !value.is_empty() => {
                    format!("{t}:{value}")
                }
                (t, _, false) if !t.is_empty() => format!("{t}:{identifier}"),
                // Rung (d): fallback
                _ => "None".to_string(),
            }
        }
    }
}

/// Format the `JSM internal:` field (field 5) from the comment's `properties` array.
///
/// Scans for `{"key":"sd.public.comment","value":{"internal":bool}}` in the
/// properties array. Returns `"Yes"` / `"No"` / `"N/A"` (BC-3.5.010 Field-5 ladder).
fn format_jsm_internal_field(properties: Option<&serde_json::Value>) -> &'static str {
    let props = match properties.and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return "N/A",
    };
    for prop in props {
        if prop["key"].as_str() == Some("sd.public.comment") {
            return match prop["value"]["internal"].as_bool() {
                Some(true) => "Yes",
                Some(false) => "No",
                None => "N/A",
            };
        }
    }
    "N/A"
}

// ── Comment Edit ─────────────────────────────────────────────────────────

/// Edit a comment body (body-only PUT; default no-visibility-flag path).
///
/// Implemented in S-577-4 (body sources + body-only PUT). S-577-5 extends this
/// handler with `--internal`/`--public` visibility flags and the `--public`
/// confirmation gate that consumes `no_input`.
///
/// Pipeline (BC-3.5.009 edit-pipeline ordering pin):
/// 1. `validate_comment_id` (EC-3.5.002-1) — exit 64 on bad charset
/// 2. Body source resolution — `--file` (NotFound → exit 64), `--stdin`, positional,
///    or no source → exit 64 "body is required"
/// 3. Empty/whitespace guard (EC-3.5.009-5) — exit 64 "comment body cannot be empty"
/// 4. ADF conversion: trim then `text_to_adf` or `markdown_to_adf`; raw pre-trim
///    input stashed for `changed_fields.body` echo (BC-3.5.005 raw echo pin)
/// 5. HTTP PUT — `update_comment(key, id, adf_body, None)` (None = body-only)
///    200 → success; 404/403 → exit 64 + two-line body surface (BC-3.5.004)
/// 6. JSON: `{changed_fields:{body:<raw>},id,key,updated:true}` via render_json (#526)
///
/// `_no_input` stays prefixed here — the body-only default path has no
/// confirmation gate. S-577-5 will un-prefix it when the `--public` gate lands.
pub(super) async fn handle_comment_edit(
    sub: CommentSubcommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    _no_input: bool,
) -> Result<()> {
    let CommentSubcommand::Edit {
        key,
        text,
        id,
        file,
        stdin,
        markdown,
        internal: _,
        public: _,
        yes: _,
    } = sub
    else {
        unreachable!("handle_comment_edit called with non-Edit variant")
    };

    // Step 1: validate --id charset (EC-3.5.002-1)
    validate_comment_id(&id)?;

    // Step 2: body source resolution (BC-3.5.009 pipeline step 2).
    // Order: --file → --stdin → positional text → no source (exit 64).
    let body = if let Some(ref path) = file {
        // EC-3.5.009-1: NotFound → exit 64 via UserError; other IO errors propagate via ?
        // (permission-denied, is-a-directory, etc. are NOT remapped to "file not found").
        std::fs::read_to_string(path).map_err(|e| -> anyhow::Error {
            if e.kind() == std::io::ErrorKind::NotFound {
                JrError::UserError(format!("file not found: {path}")).into()
            } else {
                e.into()
            }
        })?
    } else if stdin {
        // spawn_blocking isolates the blocking stdin read from the tokio runtime.
        tokio::task::spawn_blocking(|| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            Ok::<_, std::io::Error>(buf)
        })
        .await??
    } else if let Some(ref msg) = text {
        msg.clone()
    } else {
        return Err(JrError::UserError(
            "body is required — use --file, --stdin, or pass text as a positional argument.".into(),
        )
        .into());
    };

    // Step 3: empty/whitespace guard (EC-3.5.009-5)
    if body.trim().is_empty() {
        return Err(JrError::UserError("comment body cannot be empty.".into()).into());
    }

    // Step 4: stash raw pre-trim body (BC-3.5.005 raw echo pin — changed_fields.body
    // carries the raw user input, NOT the trimmed or ADF-converted value).
    let raw = body; // body moved into raw; raw is the original untrimmed content
    let trimmed = raw.trim().to_string();
    let adf_body = if markdown {
        adf::markdown_to_adf(&trimmed)?
    } else {
        adf::text_to_adf(&trimmed)
    };

    // Step 5: HTTP PUT — body-only (None = no visibility flag, this story's scope).
    // update_comment returns Result<()>; the Jira response body is discarded.
    match client.update_comment(&key, &id, adf_body, None).await {
        Ok(()) => {
            // Step 6: build response JSON from local state (NOT from Jira PUT response).
            // "updated": true is a literal boolean constant — not parsed from the API.
            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        output::render_json(&serde_json::json!({
                            "changed_fields": { "body": raw },
                            "id": id,
                            "key": key,
                            "updated": true
                        }))?
                    );
                }
                OutputFormat::Table => {
                    output::print_success(&format!("Updated comment {id} on {key}"));
                }
            }
            Ok(())
        }
        Err(e) => {
            // 404/403 → exit 64 + two-line body surface (BC-3.5.004, mirrors delete handler).
            // 404 is NOT idempotent; re-wrap as UserError so main.rs emits exit 64.
            let user_err_msg = match e.downcast_ref::<JrError>() {
                Some(JrError::ApiError { status, message }) if *status == 404 || *status == 403 => {
                    Some(format!(
                        "comment not found or permission denied: {key}#{id}\n{message}"
                    ))
                }
                _ => None,
            };
            if let Some(msg) = user_err_msg {
                return Err(JrError::UserError(msg).into());
            }
            Err(e)
        }
    }
}

// ── Comment View ─────────────────────────────────────────────────────────

/// View a single comment by ID.
///
/// Fetches `GET /rest/api/3/issue/{key}/comment/{id}?expand=properties` and renders
/// six labeled fields + an unlabeled body block (BC-3.5.010).
///
/// Pipeline:
/// 1. `validate_comment_id` (EC-3.5.002-1) — exit 64 on bad charset
/// 2. `client.get_comment(key, id)` — returns raw `serde_json::Value`
/// 3. 404/403 → exit 64 + two-line body surface (BC-3.5.004 pattern)
/// 4. `--output json` → `render_json` passthrough (EC-3.5.010-1, #526 invariant)
/// 5. Human render: 6 labeled fields + unlabeled body block via `adf_to_text`
///    (EC-3.5.010-2a: depth error → exit 64)
///
/// No `no_input` parameter — read-only handler, no interactive prompt.
pub(super) async fn handle_comment_view(
    key: String,
    id: String,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    // Step 1: validate --id charset (EC-3.5.002-1)
    validate_comment_id(&id)?;

    // Step 2: fetch the comment (returns raw serde_json::Value — no typed round-trip)
    let response: serde_json::Value = match client.get_comment(&key, &id).await {
        Ok(v) => v,
        Err(e) => {
            // 404/403 → exit 64 + two-line body surface (BC-3.5.004 pattern).
            let user_err_msg = {
                match e.downcast_ref::<JrError>() {
                    Some(JrError::ApiError { status, message })
                        if *status == 404 || *status == 403 =>
                    {
                        Some(format!(
                            "comment not found or permission denied: {key}#{id}\n{message}",
                        ))
                    }
                    _ => None,
                }
            };
            if let Some(msg) = user_err_msg {
                return Err(JrError::UserError(msg).into());
            }
            return Err(e);
        }
    };

    // Step 3: route to output format
    match output_format {
        OutputFormat::Json => {
            // EC-3.5.010-1: raw serde_json::Value passthrough — no typed round-trip
            // (#526 JSON render invariant: all JSON routes through render_json)
            println!("{}", output::render_json(&response)?);
        }
        OutputFormat::Table => {
            // 7-element human render: 6 labeled plain key-value lines + unlabeled body block.
            // Timestamps (fields 3/4) rendered as raw ISO 8601 strings — do NOT reformat.
            // BC-3.5.010 fallback tokens (normative per spec):
            //   field 1 id  → "N/A" when absent/null
            //   field 2 author.displayName → "Unknown" when author null or displayName absent
            //   fields 3/4 created/updated → "N/A" when absent/null
            let id_val = response["id"].as_str().unwrap_or("N/A");
            let author = response["author"]["displayName"]
                .as_str()
                .unwrap_or("Unknown");
            let created = response["created"].as_str().unwrap_or("N/A");
            let updated = response["updated"].as_str().unwrap_or("N/A");
            let jsm_internal = format_jsm_internal_field(response.get("properties"));
            let restricted = format_restricted_field(response.get("visibility"));

            // Fields 1–6 as plain key-value lines; blank-line separator before body block.
            print!(
                "ID: {id_val}\n\
                 Author: {author}\n\
                 Created: {created}\n\
                 Updated: {updated}\n\
                 JSM internal: {jsm_internal}\n\
                 Restricted: {restricted}\n\
                 \n"
            );

            // Body block (unlabeled, field 7): ADF → text via adf_to_text.
            // EC-3.5.010-2(a): JrError::UserError from depth guard propagates as exit 64.
            // When body key is absent, response["body"] == Value::Null → adf_to_text
            // returns Ok("") → nothing printed after the blank-line separator.
            let body_text = adf::adf_to_text(&response["body"])?;
            if !body_text.is_empty() {
                println!("{body_text}");
            }
        }
    }

    Ok(())
}

// ── Unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::format_jsm_internal_field;
    use super::format_restricted_field;
    use super::validate_comment_id;
    use crate::adf;

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

    // ── AC-007 tier (i) — EC-3.5.010-2(a) propagation (GREEN-throughout) ──────
    //
    // Verifies that `adf_to_text` returns `Err` for a >256-deep ADF node and that
    // the error kind maps to the UserError/exit-64 class.
    //
    // Strategy: programmatically BUILD (not parse) a serde_json::Value with nesting
    // depth > 256. Construction via serde_json::json! loops has no recursion limit;
    // only serde_json::from_slice (parsing) is bounded at 128. After building, we
    // call adf::adf_to_text(&deep_node) directly — no HTTP boundary involved.
    //
    // Cross-reference: tests/adf_recursion_depth.rs uses the identical programmatic
    // BUILD approach for the forward (markdown_to_adf) path
    // (e.g. test_markdown_to_adf_depth_256_blockquote_is_err).
    //
    // This test is GREEN-throughout (not a Red Gate participant): it calls
    // adf_to_text directly, which is fully implemented. The handle_comment_view
    // stub being todo!() does NOT affect this test.
    #[test]
    fn test_bc_3_5_010_ec2a_adf_error_propagates_exit64() {
        // Build a 257-deep ADF node entirely in memory via Value construction
        // (no serde_json string parsing — avoids the 128-level parse limit).
        // Depth 257 > MAX_ADF_DEPTH (256), so adf_to_text must return Err.
        let mut node = serde_json::json!({"type": "text", "text": "leaf"});
        for _ in 0..257 {
            node = serde_json::json!({
                "type": "paragraph",
                "content": [node]
            });
        }

        let result = adf::adf_to_text(&node);

        assert!(
            result.is_err(),
            "EC-3.5.010-2(a): adf_to_text must return Err for a 257-deep ADF node \
             (depth guard MAX_ADF_DEPTH=256); got Ok"
        );

        // Confirm the error maps to the UserError/exit-64 class.
        let err = result.unwrap_err();
        assert_eq!(
            err.exit_code(),
            64,
            "EC-3.5.010-2(a): adf_to_text depth-guard error must map to exit 64 \
             (JrError::UserError); got exit_code: {}; err: {err}",
            err.exit_code()
        );
    }

    // ── BC-3.5.010 pure-helper unit tests ──────────────────────────────────
    //
    // Direct calls to format_restricted_field / format_jsm_internal_field with
    // serde_json::json! fixtures.  No subprocess or mock server required.

    // (a) format_restricted_field rung (c-id):
    //     non-role/group type, empty value, non-empty identifier → "<type>:<identifier>"
    //     AC-005 worked example: type=Team, value="", identifier=AlphaTeam → "Team:AlphaTeam"
    #[test]
    fn test_bc_3_5_010_format_restricted_rung_c_id_type_plus_identifier() {
        let visibility = serde_json::json!({
            "type": "Team",
            "value": "",
            "identifier": "AlphaTeam"
        });
        let result = format_restricted_field(Some(&visibility));
        assert_eq!(
            result, "Team:AlphaTeam",
            "BC-3.5.010 rung (c-id): non-role/group type + empty value + non-empty \
             identifier must render as '<type>:<identifier>'"
        );
    }

    // (b) format_jsm_internal_field — stringly-typed internal → "N/A"
    //     BC-3.5.010 §field-5: sd.public.comment key present, but value.internal is
    //     the JSON string "true" (JSDCLOUD-9766), not a boolean.
    //     as_bool() returns None for string values → inner None arm → "N/A".
    #[test]
    fn test_bc_3_5_010_jsm_internal_stringly_typed_returns_na() {
        let props = serde_json::json!([{
            "key": "sd.public.comment",
            "value": { "internal": "true" }
        }]);
        let result = format_jsm_internal_field(Some(&props));
        assert_eq!(
            result, "N/A",
            "BC-3.5.010 §field-5: stringly-typed internal must return N/A \
             (as_bool() is None for string values)"
        );
    }

    // (c) format_jsm_internal_field — unknown-key-only properties → "N/A"
    //     EC-006: properties array present but contains no sd.public.comment entry.
    //     Loop exhausts without matching → falls through to the post-loop "N/A".
    #[test]
    fn test_bc_3_5_010_jsm_internal_unknown_key_only_returns_na() {
        let props = serde_json::json!([{
            "key": "some.other.property",
            "value": { "internal": true }
        }]);
        let result = format_jsm_internal_field(Some(&props));
        assert_eq!(
            result, "N/A",
            "BC-3.5.010 EC-006: unknown-key-only properties must return N/A \
             (loop exhausted without sd.public.comment match)"
        );
    }

    // ── Mutation-kill: format_restricted_field rung (d) fallback ───────────
    //
    // Kills mutant #3 (CI run 29299750396):
    //   interactions.rs — replace guard `!t.is_empty()` with `true`
    //   in the rung (c-id) arm `(t, _, false) if !t.is_empty()`.
    //
    // With guard → true, a visibility with type="" (empty) and a non-empty
    // identifier would match rung (c-id) and produce ":some-id" instead of
    // "None". This test asserts that rung (d) fires for empty type, forcing
    // the guard to actually check `!t.is_empty()` to survive the mutant.
    #[test]
    fn test_bc_3_5_010_format_restricted_empty_type_with_identifier_returns_none() {
        // type = "" (empty) — must fall through to rung (d) → "None"
        // If the `!t.is_empty()` guard were replaced with `true`, this would
        // match rung (c-id) and produce ":some-id" instead.
        let visibility = serde_json::json!({
            "type": "",
            "value": "",
            "identifier": "some-id"
        });
        let result = format_restricted_field(Some(&visibility));
        assert_eq!(
            result, "None",
            "BC-3.5.010 rung (d): empty type + non-empty identifier must fall \
             through to rung (d) → 'None', not ':some-id'"
        );
    }
}
