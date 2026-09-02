use crate::cli::OutputFormat;
use colored::Colorize;
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use serde::Serialize;

pub fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers);

    for row in rows {
        table.add_row(row);
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
/// Strips ASCII control characters (`0x00`-`0x1F`, `0x7F`) and ANSI
/// CSI/OSC escape sequences outright (not replaced with a placeholder —
/// distinct in behavior from `cli::issue::attachments::display_sanitize_filename`,
/// which substitutes `?`; see S-cycle3-env-tag's "Current State" note on why
/// that function is not reused here), and caps the result to a fixed
/// maximum display length with a truncation marker when capped.
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
pub(crate) fn sanitize_env_display(_value: &str) -> String {
    todo!(
        "BC-1.6.046 EC-1.6.046-2 / BC-1.6.047 EC-1.6.047-3: strip ASCII \
         control chars (0x00-0x1F, 0x7F) + ANSI CSI/OSC escape sequences; \
         cap to a fixed max display length with a truncation marker when \
         capped"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
