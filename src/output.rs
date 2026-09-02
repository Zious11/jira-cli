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
}
