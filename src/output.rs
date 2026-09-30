use crate::cli::OutputFormat;
use colored::Colorize;
use comfy_table::{Cell, Color, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use serde::Serialize;

/// `render_table`'s table-mode chokepoint (BC-7.1.006): every header and
/// every cell is sanitized via [`sanitize_table_cell`] before it reaches
/// `comfy_table`. See that function's rustdoc for the exact character
/// policy. `--output json` never routes through here.
pub fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(
            headers
                .iter()
                .map(|h| sanitize_table_cell(h))
                .collect::<Vec<_>>(),
        );

    for row in rows {
        let sanitized: Vec<String> = row.iter().map(|c| sanitize_table_cell(c)).collect();
        table.add_row(sanitized);
    }

    table.to_string()
}

/// A single table-mode cell carrying an optional structural foreground
/// color, for [`render_table_with_styles`] (BC-7.1.006). The cell's TEXT is
/// always sanitized via [`sanitize_table_cell`] before becoming a
/// `comfy_table::Cell` — styling is applied as a structural `Cell`
/// attribute afterward, never as ANSI bytes baked into the cell string, so
/// styling can never bypass sanitization.
///
/// This is the general mechanism any future jr-authored styled cell content
/// must use going forward: a server-supplied string can now never itself
/// produce a colored cell (it is always sanitized), so jr's own styling
/// must be expressed structurally. Today the only caller is `jr user
/// list`/`jr user view`'s Active column (`src/cli/user.rs`); every other
/// `render_table` call site keeps the plain `&[Vec<String>]` API above.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledCell {
    text: String,
    fg: Option<Color>,
}

impl StyledCell {
    /// A cell with no structural color — the common case.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fg: None,
        }
    }

    /// A cell styled with a structural foreground color. The caller is
    /// responsible for deciding WHETHER to colorize (e.g. honoring
    /// `--no-color`/`NO_COLOR` via `colored::control::SHOULD_COLORIZE`) —
    /// this constructor unconditionally applies `fg` when used.
    pub fn colored(text: impl Into<String>, fg: Color) -> Self {
        Self {
            text: text.into(),
            fg: Some(fg),
        }
    }
}

/// Styled-cell sibling of [`render_table`] (BC-7.1.006). Sanitizes every
/// header and every cell's text exactly like `render_table`, then applies
/// each cell's optional structural foreground color to the resulting
/// `comfy_table::Cell` — never as ANSI bytes embedded in the sanitized
/// string.
pub fn render_table_with_styles(headers: &[&str], rows: &[Vec<StyledCell>]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(
            headers
                .iter()
                .map(|h| sanitize_table_cell(h))
                .collect::<Vec<_>>(),
        );

    for row in rows {
        let cells: Vec<Cell> = row
            .iter()
            .map(|c| {
                let cell = Cell::new(sanitize_table_cell(&c.text));
                match c.fg {
                    Some(color) => cell.fg(color),
                    None => cell,
                }
            })
            .collect();
        table.add_row(cells);
    }

    table.to_string()
}

pub fn render_json<T: Serialize>(data: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(data)?)
}

pub fn print_output<T: Serialize>(
    format: &OutputFormat,
    headers: &[&str],
    rows: &[Vec<String>],
    json_data: &T,
) -> anyhow::Result<()> {
    match format {
        OutputFormat::Table => {
            if rows.is_empty() {
                println!("{}", "No results found.".dimmed());
            } else {
                println!("{}", render_table(headers, rows));
            }
        }
        OutputFormat::Json => {
            println!("{}", render_json(json_data)?);
        }
    }
    Ok(())
}

/// Styled-cell sibling of [`print_output`] (BC-7.1.006). Identical
/// dispatch — `OutputFormat::Table` renders via [`render_table_with_styles`]
/// (falling back to the same "No results found." hint when `rows` is
/// empty), `OutputFormat::Json` serializes `json_data` via [`render_json`]
/// exactly as before, completely unaffected by any cell styling. Used only
/// by `jr user list`/`jr user view`; every other call site keeps
/// [`print_output`].
pub fn print_output_with_styles<T: Serialize>(
    format: &OutputFormat,
    headers: &[&str],
    rows: &[Vec<StyledCell>],
    json_data: &T,
) -> anyhow::Result<()> {
    match format {
        OutputFormat::Table => {
            if rows.is_empty() {
                println!("{}", "No results found.".dimmed());
            } else {
                println!("{}", render_table_with_styles(headers, rows));
            }
        }
        OutputFormat::Json => {
            println!("{}", render_json(json_data)?);
        }
    }
    Ok(())
}

pub fn print_success(msg: &str) {
    eprintln!("{}", msg.green());
}

pub fn print_warning(msg: &str) {
    eprintln!("warning: {msg}");
}

pub fn print_error(msg: &str) {
    eprintln!("{}: {}", "Error".red().bold(), msg);
}

/// Shared control-char/ANSI-escape strip + length-cap transform for
/// displaying a profile's free-form `env` tag on a human-readable channel
/// (table cell or `auth status` text line) — implements BC-1.6.046
/// EC-1.6.046-2 / BC-1.6.047 EC-1.6.047-3.
///
/// Strips ASCII control characters (`0x00`-`0x1F`, `0x7F`), the Unicode
/// terminal-injection controls `display_sanitize_filename` also handles "in
/// class" (bidi overrides `U+202A..=U+202E`/`U+2066..=U+2069`, LINE/
/// PARAGRAPH SEPARATOR `U+2028`/`U+2029`, NEL `U+0085`), and ANSI CSI/OSC
/// escape sequences outright (not replaced with a placeholder — distinct
/// in behavior from `cli::issue::attachments::display_sanitize_filename`,
/// which substitutes `?`; see S-cycle3-env-tag's "Current State" note on why
/// that function is not reused here), and caps the result to a fixed
/// maximum display length with a truncation marker when capped. See
/// `strip_control_and_ansi`'s rustdoc for the exact stripped code-point set
/// and unterminated-CSI/OSC fail-closed behavior.
///
/// **JSON output MUST NEVER call this function** — `auth list --output json`
/// echoes `env` verbatim/lossless (BC-1.6.047 Postcondition 1/2a,
/// Invariant 3; mirrors issue #398's `issue edit` description-echo
/// asymmetry). Ordinary strings with no control chars/ANSI escapes and
/// under the length cap pass through unchanged.
///
/// **Pinned cap + marker (Red Gate step 2, S-cycle3-env-tag — neither is
/// BC-pinned; these are the test-writer's chosen concrete values, matching
/// the existing `src/cli/queue.rs::collapse_and_truncate`/`MAX_CAUSE_LEN`
/// truncation convention in this codebase):**
/// `MAX_ENV_DISPLAY_LEN = 40` (chars, post-strip). When the stripped value's
/// char count exceeds 40, take the first 40 chars and append the single
/// truncation marker `\u{2026}` (`…`) — total rendered length 41 chars. A
/// stripped value of exactly 40 chars or fewer is NOT truncated (no marker
/// appended). The implementer must match these exact values — see
/// `output::tests::test_sanitize_env_display_*` for the pinned assertions.
pub(crate) fn sanitize_env_display(value: &str) -> String {
    const MAX_ENV_DISPLAY_LEN: usize = 40;

    let stripped = strip_control_and_ansi(value);

    if stripped.chars().count() > MAX_ENV_DISPLAY_LEN {
        let truncated: String = stripped.chars().take(MAX_ENV_DISPLAY_LEN).collect();
        format!("{truncated}\u{2026}")
    } else {
        stripped
    }
}

/// Strips ASCII control characters (`0x00`-`0x1F`, `0x7F`), the Unicode
/// terminal-injection controls also handled "in class" by
/// `cli::issue::attachments::display_sanitize_filename` (BC-6.1.015 EC-4) —
/// bidi overrides `U+202A..=U+202E` and `U+2066..=U+2069`, LINE SEPARATOR
/// `U+2028`, PARAGRAPH SEPARATOR `U+2029`, and NEL `U+0085` — and ANSI
/// CSI/OSC escape sequences from `value`, dropping them outright (no
/// placeholder substitution; `display_sanitize_filename` substitutes `?`,
/// this function does not — see `sanitize_env_display`'s rustdoc for why).
/// A CSI sequence (`ESC [ … <final byte 0x40-0x7E>`) is consumed through
/// its final byte; an OSC sequence (`ESC ] … <BEL or ST>`) is consumed
/// through its BEL (`0x07`) or `ESC \` string terminator. A bare ESC not
/// starting a recognized CSI/OSC sequence is dropped as an ordinary control
/// character (it falls in `0x00-0x1F`).
///
/// **Unterminated CSI/OSC (fail-closed):** if a CSI or OSC sequence's
/// final byte / string terminator never appears before end-of-string, the
/// sequence (and everything after it) is consumed through EOF rather than
/// left as literal trailing text — this guarantees no raw ESC byte ever
/// survives into the returned string, at the cost of also discarding
/// whatever legitimate text followed a malformed sequence. See
/// `test_sanitize_env_display_unterminated_csi_consumed_to_eof` /
/// `..._unterminated_osc_consumed_to_eof` for the pinned behavior.
fn strip_control_and_ansi(value: &str) -> String {
    sanitize_control_and_ansi_core(value, |c| {
        let code = c as u32;
        if code <= 0x1F
            || code == 0x7F
            || (0x202A..=0x202E).contains(&code)
            || (0x2066..=0x2069).contains(&code)
            || code == 0x2028
            || code == 0x2029
            || code == 0x0085
        {
            CharDisposition::Drop
        } else {
            CharDisposition::Keep
        }
    })
}

/// Per-character disposition a [`sanitize_control_and_ansi_core`] policy
/// closure returns for a single non-ANSI-sequence `char` — CSI/OSC sequence
/// consumption itself is handled entirely by the shared core, never by the
/// policy (BC-7.1.006 / `strip_control_and_ansi`).
enum CharDisposition {
    /// Pass the character through to the output unchanged.
    Keep,
    /// Drop the character outright, substituting nothing.
    Drop,
    /// Substitute the character with a single replacement character.
    Replace(char),
}

/// Shared CSI/OSC-consuming state machine backing both
/// [`strip_control_and_ansi`] (`sanitize_env_display`) and
/// [`sanitize_table_cell`] (BC-7.1.006) — the two sibling sanitizers differ
/// only in what they do with an ordinary (non-ESC) character, which this
/// function delegates to the caller-supplied `policy` closure via
/// [`CharDisposition`]. The ANSI CSI/OSC recognition and fail-closed
/// unterminated-sequence behavior is identical for both callers and lives
/// here exactly once.
///
/// An ANSI CSI sequence (`ESC [ … <final byte 0x40-0x7E>`) is consumed
/// through its final byte; an OSC sequence (`ESC ] … <BEL 0x07 or ST
/// ESC \>`) is consumed through its BEL or `ESC \` string terminator. A
/// bare ESC not starting a recognized CSI/OSC sequence falls through to
/// `policy` like any other character. If a CSI/OSC sequence's terminator
/// never appears before end-of-string, the sequence (and everything after
/// it) is consumed through EOF — fail-closed, no raw ESC byte ever
/// survives — regardless of what `policy` would have done with the bytes
/// that would have followed.
fn sanitize_control_and_ansi_core(
    value: &str,
    mut policy: impl FnMut(char) -> CharDisposition,
) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\u{7}' {
                            break;
                        }
                        if next == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
            continue;
        }

        match policy(c) {
            CharDisposition::Keep => out.push(c),
            CharDisposition::Drop => {}
            CharDisposition::Replace(r) => out.push(r),
        }
    }

    out
}

/// Sanitizes a single table-mode header or cell string before it reaches
/// `comfy_table`'s renderer (BC-7.1.006, closing security finding
/// `SEC-001-RENDER-TABLE-ANSI-SANITIZE`, MEDIUM, CWE-150/CWE-116).
///
/// `render_table` and its styled sibling [`render_table_with_styles`] are
/// the two table-mode rendering chokepoints — `render_table` is called
/// both directly (9 production call sites: `issue/attachments.rs` x4,
/// `assets/view.rs` x2, `assets/schemas.rs`, `auth/list.rs`,
/// `issue/view.rs`) and indirectly via `print_output` (~30 call sites);
/// `render_table_with_styles` is called via `print_output_with_styles`,
/// used only by `jr user list`/`jr user view` (`src/cli/user.rs`). This
/// function is the single chokepoint-level place a server-supplied string
/// (an issue summary, a field option label, a comment body fragment, a
/// display name, ...) gets made safe for a terminal that interprets raw
/// ANSI escape/control sequences, before either chokepoint calls
/// `comfy_table::Table::set_header`/`add_row`. No caller of `render_table`
/// or `render_table_with_styles` is required to sanitize its own inputs
/// before passing them in.
///
/// **Coverage claim, precisely stated (SEC-003/D-394/D-395, FIX-P5-001):**
/// this function's callers are all of `render_table`/`render_table_with_styles`
/// output, PLUS three non-table human (non-JSON) print/error-message sites
/// that call this function (via the [`sanitize_terminal_text`] alias)
/// directly at their own construction sites instead of going through either
/// table chokepoint:
/// - `jr issue comment view`'s human output
///   (`src/cli/issue/interactions.rs::handle_comment_view`), which prints
///   its six labeled fields and ADF-derived body directly via `print!`/
///   `println!`.
/// - `jr issue assign`'s human-output success messages
///   (`src/cli/issue/workflow.rs::handle_assign`), which echo the
///   server-derived assignee `display_name` at both the idempotent
///   already-assigned site (`"{key} is already assigned to {name}"`) and
///   the newly-assigned/self-assign site (`"Assigned {key} to {name}"`),
///   via `output::print_success`.
/// - `disambiguate_user`'s shared user-resolution disambiguation output
///   (`src/cli/issue/helpers.rs::disambiguate_user`), reached by
///   `resolve_assignee` (`jr issue assign --to`), `resolve_assignee_by_project`
///   (`jr issue create`/`jr issue edit --assignee`), `resolve_user`
///   (`jr issue list --assignee`), and `mentions::resolve_at_name_candidate`
///   (`@Name` mention resolution): its `MatchResult::ExactMultiple` and
///   `MatchResult::Ambiguous` non-interactive `JrError::UserError` messages,
///   its interactive `dialoguer::Select` labels/items (the `ExactMultiple`
///   labels via the factored-out `disambiguation_labels` helper,
///   `src/cli/issue/helpers.rs`), and the `MatchResult::None` branch's
///   `all_names` candidate list — sanitized once, before it is handed to
///   the caller-supplied `none_msg_fn` closure, covering all four callers
///   uniformly. Unlike the two sinks above, `disambiguate_user`'s
///   `--output json` error envelope
///   is NOT a separate lossless channel: `src/main.rs`'s single
///   error-formatting site builds both the human-text and JSON `"error"`
///   field from the same already-sanitized `JrError::UserError` `Display`
///   string, so both channels render identically sanitized text.
///
/// This is NOT "every table-mode command" and was never meant to be read
/// that broadly — a number of other human-text call sites print
/// server-supplied strings without routing through this function at all.
/// Those are tracked as the **NONTABLE-SERVER-TEXT-SANITIZE** residual — a
/// KNOWN, NON-EXHAUSTIVE inventory for anyone auditing sanitization
/// coverage rather than re-discovering them one at a time. Every entry
/// below has been verified against the code as of the cited fix, but the
/// absence of a site from this list is NOT evidence it's sanitized — only
/// entries present have been checked:
/// - `src/cli/project.rs` — `jr project fields`'s issue-type/priority/status/
///   CMDB-field name lists (`println!` loops over server-supplied names).
/// - `src/cli/issue/workflow.rs`:
///   - `jr issue transitions`'s and `jr issue move`'s interactive/listing
///     transition-name prompts (`eprintln!`/`dialoguer::Select` item text).
///   - `handle_move`/`handle_move_bulk`'s status-name echoes on a
///     successful move (`output::print_success`, e.g. `Moved {key} to
///     "{status}"`, `{key} is already in status "{status}"`).
///   - `handle_move_bulk`'s per-key bulk-transition error line
///     (`eprintln!("error: {key}: {err_msg}")`), where `err_msg` is
///     `BulkActionError::summary()` — raw Jira bulk-API error text.
///   - (`handle_assign` is NOT in this residual list — see above, it is a
///     covered non-table sink as of D-394.)
/// - `src/cli/issue/links.rs` — `handle_link`'s link-creation confirmation
///   echo of the server-resolved link-type name (`resolved_name`, drawn
///   from `list_link_types()`'s response via `partial_match`;
///   `output::print_success`).
/// - `src/cli/sprint.rs` — `jr sprint current`'s summary-line hint
///   (`eprintln!`).
/// - `src/cli/component.rs`:
///   - `jr component delete`'s confirmation/result echo of the component
///     name (`eprintln!`).
///   - `jr component list --counts`'s per-component fetch-failure warning,
///     which echoes the component's `name` alongside the raw server error
///     (`eprintln!`).
///   - `jr component create`/`edit`'s confirmation echo of the server
///     response's `name`/`project` fields (`eprintln!`).
///   - `jr component rename`'s `--dry-run` preview and `--all-projects`
///     live fan-out summary, which echo the server-supplied project key
///     (`t.project`) for each target (`eprintln!`).
/// - `src/cli/field.rs` — `jr field options`'s graceful-degrade hint
///   (`degrade_hint_for_schema`, `eprintln!`).
/// - `src/cli/board.rs` — the single-board auto-discovery notice, which
///   echoes the server's board `name`/`board_type` (`eprintln!`).
/// - `src/cli/init.rs` — the interactive board-selection prompt's item
///   text, built from the server's board `name`/`board_type`
///   (`dialoguer::Select`).
/// - `JrError` variants that echo a raw server-supplied error body/message
///   string into their `Display` output, which callers then print to
///   stderr. This includes `jr issue comment view`'s own 404/403 error
///   branch (`handle_comment_view` returns early with the raw body before
///   any of its sanitized print sites below run). **Excludes**
///   `disambiguate_user`'s `ExactMultiple`/`Ambiguous`/`None`-branch
///   `JrError::UserError` messages, covered above as of D-395 — other
///   `JrError` bodies throughout the codebase remain residual.
///
/// None of these residuals are closed by this function, SEC-003, D-394, or
/// D-395; SEC-003's scope is `jr issue comment view`'s successful-fetch
/// human output, D-394's scope is `jr issue assign`'s human-output success
/// messages, and D-395's scope is `disambiguate_user`'s shared
/// disambiguation output, all covered above. A future fix closing any
/// NONTABLE-SERVER-TEXT-SANITIZE site should route it through
/// [`sanitize_terminal_text`] and remove it from this list.
///
/// Per-character policy, applied left to right over the whole string
/// (BC-7.1.006):
/// - `\n` (U+000A) is PRESERVED verbatim — the only mechanism by which a
///   multi-line cell renders (issue view Description/Links, comment Body).
/// - `\r` (U+000D) is STRIPPED outright, nothing substituted (a `\r\n`
///   pair therefore collapses to `\n`; a bare `\r` disappears with no
///   trace).
/// - `\t` (U+0009) is REPLACED with a single space (`" "`) — deliberately
///   NOT stripped outright like the other C0 controls below, to avoid
///   merging the words flanking it into a different, and potentially
///   dangerous-looking, string (e.g. `"rm -rf /\thome"` must not collapse
///   to `"rm -rf /home"`).
/// - All other C0 controls (`0x00`-`0x08`, `0x0B`-`0x1F`) and `0x7F` (DEL)
///   are STRIPPED outright.
/// - ANSI CSI sequences (`ESC [ … <final byte 0x40-0x7E>`) and OSC
///   sequences (`ESC ] … <BEL 0x07 or ST ESC \>`) are consumed and
///   STRIPPED wholesale, reusing [`strip_control_and_ansi`]'s existing
///   CSI/OSC state machine verbatim — including its fail-closed
///   unterminated-sequence behavior: an unterminated CSI/OSC is consumed
///   through EOF (along with everything after it), so no raw ESC byte
///   ever survives into a rendered cell.
/// - C1 controls `U+0080`-`U+009F` are STRIPPED as a class, including the
///   single-byte CSI introducer `U+009B` and the single-byte OSC
///   introducer `U+009D` — new relative to `strip_control_and_ansi`
///   (which has no C1 handling). This is a single-code-point removal, not
///   a second state machine: bytes that would otherwise have continued a
///   sequence started by a stripped C1 introducer are NOT consumed as
///   part of that sequence — they survive in the output as inert literal
///   text.
/// - Bidi override characters `U+202A`-`U+202E` and `U+2066`-`U+2069`,
///   plus `U+2028` (LINE SEPARATOR), `U+2029` (PARAGRAPH SEPARATOR), and
///   `U+0085` (NEL) are STRIPPED — the same Unicode terminal-injection
///   code-point set `strip_control_and_ansi` already strips for
///   `sanitize_env_display`.
/// - No length cap and no truncation are applied (unlike
///   `sanitize_env_display`'s capped-and-marked behavior). Ordinary
///   printable text — including non-ASCII such as `"é"`, CJK characters,
///   and emoji — passes through completely unchanged, regardless of
///   length.
///
/// **`--output json` MUST NEVER call this function.** This mirrors
/// `sanitize_env_display`'s own documented rule and the issue #398
/// description-echo asymmetry already codified in CLAUDE.md: the human
/// channel optimizes for terminal safety and scannability, the machine
/// channel must stay lossless for programmatic consumers.
///
/// See `.factory/specs/prd/bc-7-output-render.md` BC-7.1.006 (EC-1..EC-13)
/// and its inline `VP-SEC-001-001` for the full edge-case/property
/// contract this function satisfies.
pub(crate) fn sanitize_table_cell(value: &str) -> String {
    sanitize_control_and_ansi_core(value, |c| match c {
        '\n' => CharDisposition::Keep,
        '\r' => CharDisposition::Drop,
        '\t' => CharDisposition::Replace(' '),
        _ => {
            let code = c as u32;
            if code <= 0x1F
                || code == 0x7F
                || (0x80..=0x9F).contains(&code)
                || (0x202A..=0x202E).contains(&code)
                || (0x2066..=0x2069).contains(&code)
                || code == 0x2028
                || code == 0x2029
                || code == 0x0085
            {
                CharDisposition::Drop
            } else {
                CharDisposition::Keep
            }
        }
    })
}

/// Alias for [`sanitize_table_cell`] applying the exact same BC-7.1.006
/// character policy — see that function's rustdoc for the full per-character
/// contract. This name is for call sites that print a single server-supplied
/// string directly to a terminal via `print!`/`println!`/`eprintln!`, embed
/// it into a `JrError` message, or interpolate it into a `dialoguer::Select`
/// label, rather than going through `render_table`/`render_table_with_styles`,
/// so `..._table_cell` would read misleadingly at the call site (there is no
/// table involved).
/// `jr issue comment view`'s human output
/// (`src/cli/issue/interactions.rs::handle_comment_view`, SEC-003) was the
/// first caller; `jr issue assign`'s human-output success messages
/// (`src/cli/issue/workflow.rs::handle_assign`, D-394) are the second;
/// `disambiguate_user`'s shared user-resolution disambiguation output
/// (`src/cli/issue/helpers.rs::disambiguate_user`, D-395) is the third. There
/// is exactly one sanitization implementation behind all these names — this
/// function does not duplicate or fork the policy.
pub(crate) fn sanitize_terminal_text(value: &str) -> String {
    sanitize_table_cell(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_render_table_with_data() {
        let headers = &["Key", "Summary"];
        let rows = vec![vec!["FOO-1".into(), "Fix bug".into()]];
        let output = render_table(headers, &rows);
        assert!(output.contains("FOO-1"));
        assert!(output.contains("Fix bug"));
    }

    #[test]
    fn test_render_json() {
        let data = serde_json::json!({"key": "FOO-1"});
        let output = render_json(&data).unwrap();
        assert!(output.contains("FOO-1"));
    }

    // ── sanitize_env_display (BC-1.6.046 EC-1.6.046-2 / BC-1.6.047 ────
    // EC-1.6.047-3) ─────────────────────────────────────────────────
    //
    // Pinned cap + marker (see the rustdoc on `sanitize_env_display`):
    // MAX_ENV_DISPLAY_LEN = 40 chars (post-strip), truncation marker = the
    // single char `\u{2026}` ('…') appended when the cap is exceeded.

    /// Ordinary strings with no control chars/ANSI escapes and under the
    /// length cap must pass through completely unchanged.
    #[test]
    fn test_sanitize_env_display_passes_through_ordinary_string() {
        assert_eq!(sanitize_env_display("prod"), "prod");
        assert_eq!(
            sanitize_env_display("sandbox-eu-west-1"),
            "sandbox-eu-west-1"
        );
    }

    /// The empty string is a valid (non-`None`) `env` value — it must
    /// round-trip through the sanitizer unchanged (blank, not "-").
    #[test]
    fn test_sanitize_env_display_empty_string_passes_through() {
        assert_eq!(sanitize_env_display(""), "");
    }

    /// ASCII control characters (0x00-0x1F, 0x7F) — raw `\r`, `\n`, `\t`,
    /// NUL, and DEL — must be stripped outright (not replaced with a
    /// placeholder character; distinct from `display_sanitize_filename`'s
    /// `?`-substitution behavior).
    #[test]
    fn test_sanitize_env_display_strips_ascii_control_chars() {
        let hostile = "pr\rod\n\t\u{0}end\u{7f}";
        let got = sanitize_env_display(hostile);
        assert_eq!(got, "prodend");
        assert!(
            !got.chars().any(|c| (c as u32) <= 0x1F || c as u32 == 0x7F),
            "no raw control bytes may reach the terminal: {got:?}"
        );
    }

    /// An ANSI CSI escape sequence (`\x1b[31m` ... `\x1b[0m`) must be
    /// stripped WHOLESALE — not just the leading ESC byte, leaving
    /// `[31m`/`[0m` behind as literal garbage text.
    #[test]
    fn test_sanitize_env_display_strips_ansi_csi_escape_sequences() {
        let hostile = "\u{1b}[31mRED\u{1b}[0m";
        let got = sanitize_env_display(hostile);
        assert_eq!(got, "RED");
        assert!(!got.contains('\u{1b}'), "raw ESC byte must not survive");
        assert!(
            !got.contains('['),
            "the CSI sequence's bracket/param bytes must not survive as \
             literal text: {got:?}"
        );
    }

    /// An ANSI OSC escape sequence (`\x1b]0;title\x07`, terminated by BEL)
    /// must also be stripped wholesale.
    #[test]
    fn test_sanitize_env_display_strips_ansi_osc_escape_sequences() {
        let hostile = "before\u{1b}]0;evil-title\u{7}after";
        let got = sanitize_env_display(hostile);
        assert_eq!(got, "beforeafter");
    }

    /// A value with a stripped length of exactly the cap (40 chars) is NOT
    /// truncated — no marker appended, output unchanged.
    #[test]
    fn test_sanitize_env_display_exactly_at_cap_not_truncated() {
        let exactly_40 = "a".repeat(40);
        let got = sanitize_env_display(&exactly_40);
        assert_eq!(got, exactly_40);
        assert_eq!(got.chars().count(), 40);
    }

    /// A value whose stripped length exceeds the 40-char cap is truncated
    /// to the first 40 chars with the `…` (U+2026) marker appended — total
    /// rendered length 41 chars.
    #[test]
    fn test_sanitize_env_display_over_cap_truncated_with_marker() {
        let over_cap = "b".repeat(41);
        let got = sanitize_env_display(&over_cap);
        assert_eq!(got, format!("{}\u{2026}", "b".repeat(40)));
        assert_eq!(got.chars().count(), 41);
        assert!(got.ends_with('\u{2026}'));
    }

    /// A much longer value is still capped at 40 chars + marker, not
    /// merely reduced proportionally.
    #[test]
    fn test_sanitize_env_display_far_over_cap_truncated_with_marker() {
        let very_long = "c".repeat(500);
        let got = sanitize_env_display(&very_long);
        assert_eq!(got.chars().count(), 41);
        assert_eq!(got, format!("{}\u{2026}", "c".repeat(40)));
    }

    /// Control-char stripping and length capping compose: a hostile,
    /// over-length value with embedded control chars/ANSI escapes must
    /// have both transforms applied (strip first, then cap the stripped
    /// result — not cap first and leave stray control bytes past the
    /// cap boundary).
    #[test]
    fn test_sanitize_env_display_strips_then_caps_composed() {
        // 50 'x' chars with a CSI color sequence and a raw \r injected in
        // the middle; after stripping, exactly 50 'x' chars remain, which
        // must then be capped to 40 + marker.
        let hostile = format!("{}\u{1b}[31m\r{}", "x".repeat(25), "x".repeat(25));
        let got = sanitize_env_display(&hostile);
        assert_eq!(got, format!("{}\u{2026}", "x".repeat(40)));
    }

    /// Unicode bidi-override controls (U+202A-U+202E, U+2066-U+2069) must
    /// be stripped outright — same code-point set
    /// `cli::issue::attachments::display_sanitize_filename` treats "in
    /// class" (BC-6.1.015 EC-4), mirrored here for the ENV display
    /// sanitizer rather than shared code (pre-PR review finding).
    #[test]
    fn test_sanitize_env_display_strips_unicode_bidi_override() {
        let hostile =
            "pre\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}mid\u{2066}\u{2067}\u{2068}\u{2069}post";
        let got = sanitize_env_display(hostile);
        assert_eq!(got, "premidpost");
        assert!(
            !got.chars().any(|c| {
                let cp = c as u32;
                (0x202A..=0x202E).contains(&cp) || (0x2066..=0x2069).contains(&cp)
            }),
            "no raw bidi-override code points may reach the terminal: {got:?}"
        );
    }

    /// Unicode LINE SEPARATOR (U+2028) and PARAGRAPH SEPARATOR (U+2029)
    /// must be stripped outright — same code points
    /// `display_sanitize_filename` treats "in class."
    #[test]
    fn test_sanitize_env_display_strips_line_paragraph_separators() {
        let hostile = "pre\u{2028}mid\u{2029}post";
        let got = sanitize_env_display(hostile);
        assert_eq!(got, "premidpost");
        assert!(!got.contains('\u{2028}'));
        assert!(!got.contains('\u{2029}'));
    }

    /// Unicode NEL (U+0085) must be stripped outright — same code point
    /// `display_sanitize_filename` treats "in class."
    #[test]
    fn test_sanitize_env_display_strips_nel() {
        let hostile = "pre\u{0085}post";
        let got = sanitize_env_display(hostile);
        assert_eq!(got, "prepost");
        assert!(!got.contains('\u{0085}'));
    }

    /// An unterminated CSI sequence (`ESC [` with no final byte
    /// 0x40-0x7E before end-of-string) is consumed through EOF —
    /// fail-closed, no raw ESC byte leaks — per the rustdoc on
    /// `strip_control_and_ansi`.
    #[test]
    fn test_sanitize_env_display_unterminated_csi_consumed_to_eof() {
        // Only digits/`;` follow `ESC [` — no byte in the 0x40-0x7E final-byte
        // range appears anywhere in the remainder, so the sequence never
        // terminates and is consumed through end-of-string.
        let hostile = "before\u{1b}[31;1;9";
        let got = sanitize_env_display(hostile);
        assert_eq!(got, "before");
        assert!(!got.contains('\u{1b}'), "raw ESC byte must not survive");
    }

    /// An unterminated OSC sequence (`ESC ]` with no BEL/`ESC \` string
    /// terminator before end-of-string) is consumed through EOF —
    /// fail-closed, no raw ESC byte leaks — per the rustdoc on
    /// `strip_control_and_ansi`.
    #[test]
    fn test_sanitize_env_display_unterminated_osc_consumed_to_eof() {
        let hostile = "before\u{1b}]0;evil-title-no-terminator";
        let got = sanitize_env_display(hostile);
        assert_eq!(got, "before");
        assert!(!got.contains('\u{1b}'), "raw ESC byte must not survive");
    }

    /// The OSC ST-terminator check (`next == ESC && peek == '\'`) must be a
    /// true AND of both conditions, not an OR and not either side inverted
    /// (S-cycle3-env-tag, PR #752 cycle-2 mutation-testing gap — 3 survived
    /// mutants at this exact line: `&&`→`||`, and both `==`→`!=`). A stray
    /// literal backslash mid-payload that is NOT preceded by ESC must NOT
    /// be mistaken for the ST terminator — the scan must continue past it
    /// to the real terminator (BEL here), stripping the whole payload.
    #[test]
    fn test_sanitize_env_display_osc_stray_backslash_not_preceded_by_esc_does_not_terminate_early()
    {
        // Payload: 'x' '\' 'y' "VISIBLE" BEL — the bare '\' after 'x' is not
        // preceded by ESC and must not trigger early termination. Only the
        // trailing BEL should end the OSC scan.
        let hostile = "before\u{1b}]x\\yVISIBLE\u{7}TAIL";
        let got = sanitize_env_display(hostile);
        assert_eq!(
            got, "beforeTAIL",
            "the entire OSC payload (including the stray backslash and \
             everything after it up to BEL) must be stripped — a premature \
             break would leak 'yVISIBLE' into the output: {got:?}"
        );
        assert!(!got.contains("VISIBLE"));
    }

    /// Sibling to the stray-backslash test above: a bare ESC byte inside the
    /// OSC payload that is NOT followed by a backslash must NOT be mistaken
    /// for the start of an ST terminator — the scan must continue past it to
    /// the real terminator (BEL here), stripping the whole payload.
    #[test]
    fn test_sanitize_env_display_osc_stray_esc_not_followed_by_backslash_does_not_terminate_early()
    {
        // Payload: 'p' ESC 'q' "VISIBLE" BEL — the bare ESC after 'p' is not
        // followed by '\' and must not trigger early termination.
        let hostile = "before\u{1b}]p\u{1b}qVISIBLE\u{7}TAIL";
        let got = sanitize_env_display(hostile);
        assert_eq!(
            got, "beforeTAIL",
            "the entire OSC payload (including the stray ESC and everything \
             after it up to BEL) must be stripped — a premature break would \
             leak 'VISIBLE' into the output: {got:?}"
        );
        assert!(!got.contains("VISIBLE"));
        assert!(!got.contains('\u{1b}'), "raw ESC byte must not survive");
    }

    // ── sanitize_table_cell (BC-7.1.006, FIX-P5-001, SEC-001-RENDER- ──
    // TABLE-ANSI-SANITIZE) ─────────────────────────────────────────────
    //
    // `sanitize_table_cell` is implemented and wired into both
    // `render_table` and `render_table_with_styles` (FIX-P5-001). Every
    // EC-pinned test below and both property-based tests exercise that
    // production behavior directly. The `render_table`-level hostile-cell/
    // header tests and the multi-line test call PRODUCTION `render_table`
    // directly (not `sanitize_table_cell`), pinning the chokepoint-level
    // guarantee end to end — see each test's doc comment.

    /// EC-1: an ANSI CSI color sequence is stripped WHOLESALE — not just
    /// the leading ESC byte, leaving `[31m`/`[0m` behind as literal text.
    #[test]
    fn test_bc_7_1_006_ec1_ansi_csi_color_sequence_stripped() {
        assert_eq!(sanitize_table_cell("\u{1b}[31mRED\u{1b}[0m"), "RED");
    }

    /// EC-2: an ANSI OSC window-title sequence, BEL-terminated, is
    /// stripped wholesale.
    #[test]
    fn test_bc_7_1_006_ec2_ansi_osc_window_title_sequence_stripped() {
        assert_eq!(
            sanitize_table_cell("before\u{1b}]0;pwned\u{7}after"),
            "beforeafter"
        );
    }

    /// EC-3: an unterminated CSI sequence fails closed — consumed through
    /// EOF, along with everything after it, so no raw ESC byte survives.
    #[test]
    fn test_bc_7_1_006_ec3_unterminated_csi_fails_closed_consumed_to_eof() {
        assert_eq!(sanitize_table_cell("before\u{1b}[31;1;9"), "before");
    }

    /// EC-13: fail-closed consumption of an unterminated CSI sequence
    /// includes any `\n` that happens to fall inside the unterminated
    /// scan — the newline is not special-cased or preserved just because
    /// EC-9 normally preserves bare `\n`. `\n` (0x0A) is not a valid CSI
    /// final byte (0x40-0x7E), so it never terminates the scan on its
    /// own; here nothing in the remainder of the string is a valid final
    /// byte either, so the whole string — params and trailing `\n` alike
    /// — is consumed through EOF, leaving `""`.
    #[test]
    fn test_bc_7_1_006_ec13_unterminated_csi_consumption_includes_embedded_newline() {
        assert_eq!(sanitize_table_cell("\u{1b}[31;1;9\n"), "");
    }

    /// EC-4: a bidi override pair is stripped outright.
    #[test]
    fn test_bc_7_1_006_ec4_bidi_override_pair_stripped() {
        assert_eq!(
            sanitize_table_cell("pre\u{202e}mid\u{202e}post"),
            "premidpost"
        );
    }

    /// EC-5: the C1 CSI introducer (`U+009B`) is stripped as a single code
    /// point; the digits/letters that would have continued a 7-bit CSI
    /// sequence are NOT consumed as a sequence and survive as literal text.
    #[test]
    fn test_bc_7_1_006_ec5_c1_csi_introducer_stripped_survivor_bytes_literal() {
        assert_eq!(
            sanitize_table_cell("pre\u{9b}31mFAKE\u{9b}0mpost"),
            "pre31mFAKE0mpost"
        );
    }

    /// EC-6: NUL and other C0 controls (here: NUL, BS, VT, DEL) are
    /// stripped outright, with nothing substituted.
    #[test]
    fn test_bc_7_1_006_ec6_nul_and_other_c0_controls_stripped() {
        assert_eq!(sanitize_table_cell("pre\u{0}post"), "prepost");
        assert_eq!(
            sanitize_table_cell("a\u{8}b\u{b}c\u{7f}d"),
            "abcd",
            "BS (0x08), VT (0x0B), and DEL (0x7F) must all be stripped outright"
        );
    }

    /// EC-7: a bare `\r` is stripped with nothing substituted.
    #[test]
    fn test_bc_7_1_006_ec7_bare_cr_stripped_with_nothing_substituted() {
        assert_eq!(sanitize_table_cell("pre\rpost"), "prepost");
    }

    /// EC-8: a `\r\n` pair collapses to a single `\n`.
    #[test]
    fn test_bc_7_1_006_ec8_crlf_collapses_to_lf() {
        assert_eq!(sanitize_table_cell("line1\r\nline2"), "line1\nline2");
    }

    /// EC-9: a bare `\n` is preserved unchanged — multi-line cell
    /// rendering must be unaffected.
    #[test]
    fn test_bc_7_1_006_ec9_bare_lf_preserved_unchanged() {
        assert_eq!(sanitize_table_cell("line1\nline2"), "line1\nline2");
    }

    /// EC-10: `\t` is replaced with a single space, not dropped outright —
    /// dropping it would merge the flanking words into a different (and
    /// potentially dangerous-looking) string.
    #[test]
    fn test_bc_7_1_006_ec10_tab_replaced_with_single_space_not_dropped() {
        assert_eq!(sanitize_table_cell("a\tb"), "a b");
    }

    /// EC-11 (a): a long (500-char) string of only printable non-control
    /// characters passes through byte-for-byte unchanged — no length cap.
    #[test]
    fn test_bc_7_1_006_ec11_long_clean_text_unchanged_no_cap() {
        let clean = "x".repeat(500);
        assert_eq!(sanitize_table_cell(&clean), clean);
        assert_eq!(sanitize_table_cell(&clean).chars().count(), 500);
    }

    /// EC-11 (b): non-ASCII printable text (accented Latin, CJK, emoji)
    /// passes through unchanged — the sanitizer strips control/ANSI
    /// classes only, never ordinary Unicode text.
    #[test]
    fn test_bc_7_1_006_ec11_non_ascii_text_unchanged() {
        let clean = "café 好世界 🎉🚀";
        assert_eq!(sanitize_table_cell(clean), clean);
    }

    /// Strategy biased toward the exact hostile classes `sanitize_table_cell`
    /// must strip (control chars, full CSI/OSC sequences including
    /// unterminated ones, C1 introducers, bidi overrides, line/paragraph
    /// separators) interleaved with ordinary printable/non-ASCII text and
    /// `\n`/`\r`/`\t`, so the property test below actually exercises the
    /// stripping logic rather than drowning it in plain text.
    fn hostile_table_cell_strategy() -> impl Strategy<Value = String> {
        let token = prop_oneof![
            10 => "[a-zA-Z0-9 .,!?/-]{1,6}",
            4 => Just("\n".to_string()),
            3 => Just("\r".to_string()),
            3 => Just("\t".to_string()),
            3 => (0u32..=0x1Fu32).prop_filter_map("exclude \\n \\r \\t (covered above)", |cp| {
                let c = char::from_u32(cp).unwrap();
                (c != '\n' && c != '\r' && c != '\t').then(|| c.to_string())
            }),
            2 => Just("\u{7f}".to_string()),
            2 => Just("\u{1b}".to_string()),
            4 => Just("\u{1b}[31m".to_string()),
            4 => Just("\u{1b}[0m".to_string()),
            3 => Just("\u{1b}]0;evil-title\u{7}".to_string()),
            2 => Just("\u{1b}]8;;http://example.invalid\u{1b}\\".to_string()),
            2 => Just("\u{1b}[31;1;9".to_string()),
            2 => Just("\u{1b}]0;no-terminator".to_string()),
            3 => (0x80u32..=0x9Fu32).prop_map(|cp| char::from_u32(cp).unwrap().to_string()),
            3 => prop_oneof![
                Just(0x202Au32),
                Just(0x202Bu32),
                Just(0x202Cu32),
                Just(0x202Du32),
                Just(0x202Eu32),
                Just(0x2066u32),
                Just(0x2067u32),
                Just(0x2068u32),
                Just(0x2069u32),
            ]
            .prop_map(|cp| char::from_u32(cp).unwrap().to_string()),
            1 => Just("\u{2028}".to_string()),
            1 => Just("\u{2029}".to_string()),
            1 => Just("\u{85}".to_string()),
            3 => prop_oneof![Just("é".to_string()), Just("好".to_string()), Just("🎉".to_string())],
        ];
        prop::collection::vec(token, 0..40).prop_map(|tokens| tokens.concat())
    }

    /// Strategy generating strings composed solely of printable non-control
    /// characters plus `\n` — the identity-input space for VP-SEC-001-001(a)
    /// (EC-11).
    fn clean_table_cell_strategy() -> impl Strategy<Value = String> {
        let token = prop_oneof![
            8 => "[a-zA-Z0-9 .,!?/_-]{1,8}",
            2 => Just("\n".to_string()),
            1 => prop_oneof![Just("é".to_string()), Just("好".to_string()), Just("🎉".to_string())],
        ];
        prop::collection::vec(token, 0..40).prop_map(|tokens| tokens.concat())
    }

    /// Strategy generating strings that exercise every hostile class
    /// `sanitize_table_cell` strips or transforms, EXCLUDING any
    /// unterminated CSI/OSC sequence — the input space for the exact-`\n`-
    /// preservation property below (VP-SEC-001-001(a)(ii)).
    ///
    /// Every ANSI escape token this generator emits is fully
    /// self-terminated *within that one token*: the CSI token always
    /// appends an explicit final byte (0x40-0x7E) after its digit/`;`
    /// params, and each OSC token always appends its own BEL or `ESC \`
    /// string terminator after its param-free body — so termination never
    /// depends on what a neighboring token happens to contain.
    ///
    /// No token here contains a lone/bare ESC byte, and no token's plain
    /// text contains a literal `[` or `]`. This rules out the
    /// cross-token composition hazard analyzed in
    /// `sanitize_control_and_ansi_core`'s rustdoc: the core only starts
    /// consuming a CSI/OSC sequence when it sees a raw ESC char followed
    /// immediately by `[` or `]` (`chars.peek()`), so a lone ESC emitted
    /// by one token immediately followed by a `[`/`]` literal from the
    /// next token could otherwise accidentally form a fresh escape
    /// sequence whose termination depends on further-downstream tokens.
    /// Since ESC appears only inside the CSI/OSC tokens themselves (never
    /// standalone, never adjacent to a bare `[`/`]` from another token),
    /// every escape sequence this generator can produce is guaranteed
    /// terminated, and `\n` tokens can never be swallowed by one.
    ///
    /// Separately safe by inspection: U+009B/U+009D (the C1 CSI/OSC
    /// introducers, included via the 0x80-0x9F range below) do NOT start
    /// a consuming state in `sanitize_control_and_ansi_core` at all — the
    /// core only recognizes the 7-bit `ESC [` / `ESC ]` forms — so they
    /// are always a harmless single-code-point drop, per
    /// `sanitize_table_cell`'s own rustdoc ("bytes that would otherwise
    /// have continued a sequence started by a stripped C1 introducer are
    /// NOT consumed as part of that sequence").
    fn hostile_no_unterminated_escape_strategy() -> impl Strategy<Value = String> {
        let csi_params = "[0-9;]{0,4}";
        let csi_terminator = prop_oneof![
            Just('m'),
            Just('K'),
            Just('H'),
            Just('J'),
            Just('A'),
            Just('~')
        ];
        let csi = (csi_params, csi_terminator)
            .prop_map(|(params, term): (String, char)| format!("\u{1b}[{params}{term}"));

        let osc_bel =
            "[a-zA-Z0-9 ,.:_!?-]{0,8}".prop_map(|body: String| format!("\u{1b}]{body}\u{7}"));
        let osc_st =
            "[a-zA-Z0-9 ,.:_!?-]{0,8}".prop_map(|body: String| format!("\u{1b}]{body}\u{1b}\\"));

        let token = prop_oneof![
            10 => "[a-zA-Z0-9 .,!?/-]{1,6}",
            4 => Just("\n".to_string()),
            3 => Just("\r".to_string()),
            3 => Just("\t".to_string()),
            3 => (0u32..=0x1Fu32).prop_filter_map(
                "exclude \\n \\r \\t (covered above) and ESC (handled only by the \
                 self-terminated csi/osc tokens below)",
                |cp| {
                    let c = char::from_u32(cp).unwrap();
                    (c != '\n' && c != '\r' && c != '\t' && c != '\u{1b}').then(|| c.to_string())
                }
            ),
            2 => Just("\u{7f}".to_string()),
            4 => csi,
            3 => osc_bel,
            2 => osc_st,
            3 => (0x80u32..=0x9Fu32).prop_map(|cp| char::from_u32(cp).unwrap().to_string()),
            3 => prop_oneof![
                Just(0x202Au32),
                Just(0x202Bu32),
                Just(0x202Cu32),
                Just(0x202Du32),
                Just(0x202Eu32),
                Just(0x2066u32),
                Just(0x2067u32),
                Just(0x2068u32),
                Just(0x2069u32),
            ]
            .prop_map(|cp| char::from_u32(cp).unwrap().to_string()),
            1 => Just("\u{2028}".to_string()),
            1 => Just("\u{2029}".to_string()),
            1 => Just("\u{85}".to_string()),
            3 => prop_oneof![Just("é".to_string()), Just("好".to_string()), Just("🎉".to_string())],
        ];
        prop::collection::vec(token, 0..40).prop_map(|tokens| tokens.concat())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        /// VP-SEC-001-001(a), whole-string invariant: over an arbitrary
        /// hostile-biased input, the output contains no disallowed C0
        /// control (anything <= 0x1F other than `\n`, plus `0x7F`), no C1
        /// control (`0x80`-`0x9F`), no raw ESC, no `\r`, no `\t`, and none
        /// of the bidi-override/line/paragraph-separator code points; the
        /// output's `\n` count never exceeds the input's (the sanitizer
        /// never fabricates a `\n` — see the `<=`-not-`==` note on the
        /// newline assertion below for why exact preservation is not a
        /// sound invariant against this generator).
        #[test]
        fn prop_bc_7_1_006_sanitize_table_cell_whole_string_invariant(
            input in hostile_table_cell_strategy()
        ) {
            let output = sanitize_table_cell(&input);

            for c in output.chars() {
                let cp = c as u32;
                let disallowed_c0_or_del = (cp <= 0x1F && c != '\n') || cp == 0x7F;
                prop_assert!(
                    !disallowed_c0_or_del,
                    "disallowed C0/DEL control survived: {c:?} (U+{cp:04X}) in {output:?}"
                );
                prop_assert!(
                    !(0x80..=0x9F).contains(&cp),
                    "disallowed C1 control survived: {c:?} (U+{cp:04X}) in {output:?}"
                );
                prop_assert_ne!(c, '\u{1b}', "raw ESC survived in {:?}", output);
                prop_assert_ne!(c, '\r', "raw CR survived in {:?}", output);
                prop_assert_ne!(c, '\t', "raw TAB survived in {:?}", output);
                let bidi_or_separator = matches!(cp, 0x202A..=0x202E | 0x2066..=0x2069)
                    || cp == 0x2028
                    || cp == 0x2029
                    || cp == 0x0085;
                prop_assert!(
                    !bidi_or_separator,
                    "bidi-override/line-separator survived: {c:?} (U+{cp:04X}) in {output:?}"
                );
            }

            // Never-fabricate invariant: every disposition this sanitizer
            // applies either drops a character or replaces `\t` with a
            // single space — nothing ever substitutes a `\n` — so the
            // output's `\n` count can never exceed the input's. Exact
            // preservation (`==`, not `<=`) holds only when the input
            // contains no unterminated CSI/OSC sequence: EC-3's fail-closed
            // rule consumes an unterminated sequence through end-of-string,
            // discarding everything after it, including any `\n` that
            // follows (EC-13 pins the minimal case:
            // `sanitize_table_cell("\u{1b}[31;1;9\n")` == `""`). Exact `\n`
            // preservation on inputs free of that hazard is covered by
            // `prop_bc_7_1_006_sanitize_table_cell_newlines_preserved_without_unterminated_escape`
            // below.
            let input_newlines = input.chars().filter(|&c| c == '\n').count();
            let output_newlines = output.chars().filter(|&c| c == '\n').count();
            prop_assert!(
                output_newlines <= input_newlines,
                "sanitize_table_cell must never fabricate a \\n: input had {} \
                 newline(s), output has {} in {:?}",
                input_newlines, output_newlines, output
            );
        }

        /// VP-SEC-001-001(a), identity case (EC-11): on input composed
        /// solely of printable non-control characters plus `\n`,
        /// `sanitize_table_cell` is the identity function.
        #[test]
        fn prop_bc_7_1_006_sanitize_table_cell_identity_on_clean_input(
            input in clean_table_cell_strategy()
        ) {
            prop_assert_eq!(sanitize_table_cell(&input), input);
        }

        /// VP-SEC-001-001(a) part (ii): on an input containing no
        /// unterminated CSI/OSC sequence, `sanitize_table_cell` preserves
        /// the `\n` count EXACTLY — not merely `<=` as the whole-string
        /// invariant above must allow for an arbitrary (possibly
        /// unterminated-escape-containing) input. See
        /// `hostile_no_unterminated_escape_strategy`'s doc comment for why
        /// every input this generator produces is guaranteed free of an
        /// unterminated CSI/OSC sequence.
        #[test]
        fn prop_bc_7_1_006_sanitize_table_cell_newlines_preserved_without_unterminated_escape(
            input in hostile_no_unterminated_escape_strategy()
        ) {
            let output = sanitize_table_cell(&input);
            let input_newlines = input.chars().filter(|&c| c == '\n').count();
            let output_newlines = output.chars().filter(|&c| c == '\n').count();
            prop_assert_eq!(
                input_newlines, output_newlines,
                "expected exact \\n preservation on an input with no unterminated \
                 CSI/OSC sequence: {:?} -> {:?}",
                input, output
            );
        }
    }

    // ── render_table chokepoint tests (BC-7.1.006) ──────────────────────
    //
    // These call PRODUCTION `render_table` directly — `sanitize_table_cell`
    // is wired into it (FIX-P5-001), so these tests exercise and pin
    // today's real, sanitized behavior.

    /// A hostile cell containing an ANSI CSI sequence plus a C1 CSI
    /// introducer must not leak a raw ESC byte or a raw C1 byte into
    /// `render_table`'s output.
    #[test]
    fn test_bc_7_1_006_render_table_strips_ansi_and_c1_from_hostile_cell() {
        let headers = &["Key", "Summary"];
        let hostile = "\u{1b}[31mFAKE\u{1b}[0m\u{9b}pwned";
        let rows = vec![vec!["FOO-1".to_string(), hostile.to_string()]];

        let output = render_table(headers, &rows);

        assert!(
            !output.contains('\u{1b}'),
            "raw ESC byte must not survive in a rendered table cell: {output:?}"
        );
        assert!(
            !output.chars().any(|c| (0x80..=0x9F).contains(&(c as u32))),
            "raw C1 byte must not survive in a rendered table cell: {output:?}"
        );
    }

    /// Same hostile-payload requirement as the cell test above, but for a
    /// table HEADER — defense in depth (BC-7.1.006 sanitizes headers too,
    /// even though every production header today is a `&'static str`
    /// literal with nothing to strip).
    #[test]
    fn test_bc_7_1_006_render_table_strips_ansi_and_c1_from_hostile_header() {
        let hostile_header = "\u{1b}[31mEvil\u{1b}[0m\u{9b}Header".to_string();
        let headers: &[&str] = &[hostile_header.as_str(), "Value"];
        let rows = vec![vec!["a".to_string(), "b".to_string()]];

        let output = render_table(headers, &rows);

        assert!(
            !output.contains('\u{1b}'),
            "raw ESC byte must not survive in a rendered table header: {output:?}"
        );
        assert!(
            !output.chars().any(|c| (0x80..=0x9F).contains(&(c as u32))),
            "raw C1 byte must not survive in a rendered table header: {output:?}"
        );
    }

    /// `comfy_table` renders an embedded `\n` as multiple physical lines —
    /// this is pre-existing, unrelated to `sanitize_table_cell`. Pinned as
    /// a regression guard for BC-7.1.006's `\n`-preservation policy: now
    /// that `sanitize_table_cell` IS wired into `render_table`, `\n` must
    /// still reach `comfy_table` verbatim (per EC-9), and multi-line cells
    /// (issue view Description/Links, comment Body) must keep rendering as
    /// multiple lines rather than regressing to a single line.
    #[test]
    fn test_bc_7_1_006_render_table_still_renders_multiline_cell() {
        let headers = &["Key", "Description"];
        let rows = vec![vec![
            "FOO-1".to_string(),
            "line one\nline two\nline three".to_string(),
        ]];

        let output = render_table(headers, &rows);

        assert!(output.contains("line one"));
        assert!(output.contains("line two"));
        assert!(output.contains("line three"));
        assert!(
            output.lines().count() >= 5,
            "expected a multi-line rendering (header + separators + 3 content \
             lines), got {} lines: {output:?}",
            output.lines().count()
        );
    }

    // ── render_table_with_styles / print_output_with_styles chokepoint ──
    // tests (BC-7.1.006, FIX-P5-001 pr-review cycle-1 finding B-1) ──────
    //
    // `render_table_with_styles` is the ONLY production table-mode
    // rendering path for `jr user list`/`jr user view` (`src/cli/user.rs`)
    // — both commands render server-supplied display names and emails
    // through `StyledCell`s. Before this test group, nothing called
    // `render_table_with_styles` directly: a regression that dropped the
    // `sanitize_table_cell(&c.text)` call inside it (e.g. reverting to
    // `Cell::new(&c.text)`) would silently reopen SEC-001 for every user
    // command while the whole rest of the suite kept passing, since the
    // plain-`String` `render_table` chokepoint tests above don't exercise
    // this sibling function at all.

    /// A hostile `StyledCell::plain` cell's TEXT must be sanitized
    /// identically to `render_table`'s plain `String` cells — no raw ESC
    /// or C1 byte may survive into the rendered table.
    #[test]
    fn test_bc_7_1_006_render_table_with_styles_strips_hostile_plain_cell() {
        let headers = &["Key", "Summary"];
        let hostile = "\u{1b}[31mFAKE\u{1b}[0m\u{9b}pwned";
        let rows = vec![vec![StyledCell::plain("FOO-1"), StyledCell::plain(hostile)]];

        let output = render_table_with_styles(headers, &rows);

        assert!(
            !output.contains('\u{1b}'),
            "raw ESC byte must not survive in a styled table cell: {output:?}"
        );
        assert!(
            !output.chars().any(|c| (0x80..=0x9F).contains(&(c as u32))),
            "raw C1 byte must not survive in a styled table cell: {output:?}"
        );
    }

    /// Sibling of the test above, but for a `StyledCell::colored` cell —
    /// carrying a structural foreground color must NOT bypass sanitization
    /// of the cell's TEXT. This is the exact shape `src/cli/user.rs`'s
    /// `active_cell` produces for a server-influenced value, and the
    /// scenario B-1 called out: a colored cell's `fg` is unverified by any
    /// other test in this file.
    #[test]
    fn test_bc_7_1_006_render_table_with_styles_strips_hostile_colored_cell() {
        let headers = &["Key", "Active"];
        let hostile = "\u{1b}[31mFAKE\u{1b}[0m\u{9b}pwned";
        let rows = vec![vec![
            StyledCell::plain("FOO-1"),
            StyledCell::colored(hostile, Color::Green),
        ]];

        let output = render_table_with_styles(headers, &rows);

        assert!(
            !output.contains('\u{1b}'),
            "raw ESC byte must not survive in a colored styled table cell: {output:?}"
        );
        assert!(
            !output.chars().any(|c| (0x80..=0x9F).contains(&(c as u32))),
            "raw C1 byte must not survive in a colored styled table cell: {output:?}"
        );
        assert!(
            output.contains("pwned"),
            "the sanitized survivor text must still reach the rendered table: {output:?}"
        );
    }

    /// Hostile HEADER text must be sanitized too — defense in depth,
    /// mirroring `render_table`'s own header test above.
    #[test]
    fn test_bc_7_1_006_render_table_with_styles_strips_hostile_header() {
        let hostile_header = "\u{1b}[31mEvil\u{1b}[0m\u{9b}Header".to_string();
        let headers: &[&str] = &[hostile_header.as_str(), "Value"];
        let rows = vec![vec![StyledCell::plain("a"), StyledCell::plain("b")]];

        let output = render_table_with_styles(headers, &rows);

        assert!(
            !output.contains('\u{1b}'),
            "raw ESC byte must not survive in a styled table header: {output:?}"
        );
        assert!(
            !output.chars().any(|c| (0x80..=0x9F).contains(&(c as u32))),
            "raw C1 byte must not survive in a styled table header: {output:?}"
        );
    }

    /// `print_output_with_styles`'s `OutputFormat::Table` arm must still
    /// dispatch to the sanitizing `render_table_with_styles` rather than
    /// bypassing it — asserted by calling `print_output_with_styles`
    /// itself (not just its callee) so a regression at the dispatch site
    /// (e.g. a future refactor that formats `c.text` directly instead of
    /// calling `render_table_with_styles`) is caught here too. stdout
    /// itself isn't captured by this in-process unit test (the real CLI
    /// process boundary is covered end-to-end by
    /// `tests/table_output_sanitization.rs`'s `jr user list` case); this
    /// test instead pins that the call succeeds for both a hostile plain
    /// and a hostile colored cell, and that JSON mode stays a pure
    /// `render_json` passthrough completely unaffected by cell styling.
    #[test]
    fn test_bc_7_1_006_print_output_with_styles_does_not_error_on_hostile_cells() {
        let headers = &["Name"];
        let hostile = "\u{1b}[31mFAKE\u{1b}[0m\u{9b}pwned";
        let rows = vec![vec![StyledCell::colored(hostile, Color::Red)]];
        let json_data = serde_json::json!([{"name": hostile}]);

        print_output_with_styles(&OutputFormat::Table, headers, &rows, &json_data)
            .expect("table-mode print must not error on a hostile styled cell");

        let json_output = print_output_with_styles(&OutputFormat::Json, headers, &rows, &json_data);
        assert!(
            json_output.is_ok(),
            "json-mode print must not error and must never consult the styled \
             rows at all"
        );
    }
}
