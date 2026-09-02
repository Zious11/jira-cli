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

        let code = c as u32;
        if code <= 0x1F
            || code == 0x7F
            || (0x202A..=0x202E).contains(&code)
            || (0x2066..=0x2069).contains(&code)
            || code == 0x2028
            || code == 0x2029
            || code == 0x0085
        {
            continue;
        }

        out.push(c);
    }

    out
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
}
