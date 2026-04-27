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
