use anyhow::Result;
use chrono::DateTime;
use serde::Serialize;

use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::output;
use crate::types::jira::ChangelogEntry;

const NULL_GLYPH: &str = "—";
const SYSTEM_AUTHOR: &str = "(system)";

/// Shape of the JSON output body. Keeps the `key` alongside entries so
/// consumers always know which issue a response belongs to.
#[derive(Serialize)]
struct ChangelogOutput<'a> {
    key: &'a str,
    entries: &'a [ChangelogEntry],
}

pub(super) async fn handle(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Changelog {
        key,
        limit: _,
        all: _,
        field,
        author,
        reverse,
    } = command
    else {
        unreachable!("handler only called for IssueCommand::Changelog")
    };

    // Resolve --author "me" up-front; other forms compare directly.
    let author_needle = match author.as_deref() {
        Some("me") => Some(AuthorNeedle::AccountId(
            client.get_myself().await?.account_id,
        )),
        Some(raw) => Some(classify_author(raw)),
        None => None,
    };

    let mut entries = client.get_changelog(&key).await?;

    // Sort.
    if reverse {
        entries.sort_by(|a, b| a.created.cmp(&b.created));
    } else {
        entries.sort_by(|a, b| b.created.cmp(&a.created));
    }

    // --author filter: drops entries with no author when set, unless
    // the needle matches the null placeholder (we don't support that).
    if let Some(needle) = &author_needle {
        entries.retain(|e| author_matches(e.author.as_ref(), needle));
    }

    // --field filter: drop items, then empty entries.
    if !field.is_empty() {
        let needles: Vec<String> = field.iter().map(|f| f.to_lowercase()).collect();
        for entry in entries.iter_mut() {
            entry.items.retain(|it| {
                let h = it.field.to_lowercase();
                needles.iter().any(|n| h.contains(n))
            });
        }
        entries.retain(|e| !e.items.is_empty());
    }

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                output::render_json(&ChangelogOutput {
                    key: &key,
                    entries: &entries
                })?
            );
        }
        OutputFormat::Table => {
            let headers = &["DATE", "AUTHOR", "FIELD", "FROM", "TO"];
            let rows = build_rows(&entries);
            output::print_output(output_format, headers, &rows, &entries)?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum AuthorNeedle {
    /// Exact accountId match (literal input or resolved from "me").
    AccountId(String),
    /// Case-insensitive substring match against `displayName` or `accountId`.
    NameSubstring(String),
}

/// Classify a user-supplied `--author` value. We treat a value as an
/// accountId if it looks like one (no whitespace, has a colon or is
/// entirely alphanumeric+dashes and ≥12 chars). Otherwise it's a name
/// substring.
///
/// The API's accountId format varies (`public cloud` uses
/// `557058:...`-style strings; older formats are opaque 24+ char
/// hex-like blobs). The heuristic below is conservative: a plain English
/// name like "alice" is always a substring; anything with a colon or
/// a long alphanumeric blob is treated as literal.
fn classify_author(raw: &str) -> AuthorNeedle {
    let trimmed = raw.trim();
    let looks_like_account_id = trimmed.contains(':')
        || (trimmed.len() >= 12
            && trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    if looks_like_account_id {
        AuthorNeedle::AccountId(trimmed.to_string())
    } else {
        AuthorNeedle::NameSubstring(trimmed.to_lowercase())
    }
}

fn author_matches(author: Option<&crate::types::jira::User>, needle: &AuthorNeedle) -> bool {
    let Some(a) = author else { return false };
    match needle {
        AuthorNeedle::AccountId(id) => a.account_id == *id,
        AuthorNeedle::NameSubstring(n) => {
            a.display_name.to_lowercase().contains(n) || a.account_id.to_lowercase().contains(n)
        }
    }
}

/// Flatten `entries` into one row per `ChangelogItem`, preserving the
/// caller's sort order. Each row becomes `[date, author, field, from, to]`.
fn build_rows(entries: &[ChangelogEntry]) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for entry in entries {
        let date = format_date(&entry.created);
        let author = entry
            .author
            .as_ref()
            .map(|a| a.display_name.clone())
            .unwrap_or_else(|| SYSTEM_AUTHOR.to_string());
        for item in &entry.items {
            rows.push(vec![
                date.clone(),
                author.clone(),
                item.field.clone(),
                from_to_display(item.from_string.as_deref(), item.from.as_deref()),
                from_to_display(item.to_string.as_deref(), item.to.as_deref()),
            ]);
        }
    }
    rows
}

/// Parse a Jira ISO-8601 timestamp and render as `YYYY-MM-DD HH:MM` in the
/// user's local time zone. Falls back to the raw string if parsing fails.
fn format_date(iso: &str) -> String {
    DateTime::parse_from_rfc3339(iso)
        .or_else(|_| DateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.3f%z"))
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| iso.to_string())
}

/// Prefer the human-readable string; fall back to the raw id; default to
/// the em-dash null marker for empty/missing values.
fn from_to_display(string: Option<&str>, raw: Option<&str>) -> String {
    let pick = string.or(raw).map(str::trim).unwrap_or("");
    if pick.is_empty() {
        NULL_GLYPH.to_string()
    } else {
        pick.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::jira::{ChangelogItem, User};

    #[test]
    fn classify_author_treats_short_name_as_substring() {
        match classify_author("alice") {
            AuthorNeedle::NameSubstring(s) => assert_eq!(s, "alice"),
            other => panic!("expected NameSubstring, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_treats_colon_string_as_accountid() {
        match classify_author("557058:abc-123") {
            AuthorNeedle::AccountId(s) => assert_eq!(s, "557058:abc-123"),
            other => panic!("expected AccountId, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_treats_long_hex_blob_as_accountid() {
        match classify_author("abcdef0123456789deadbeef") {
            AuthorNeedle::AccountId(s) => assert_eq!(s, "abcdef0123456789deadbeef"),
            other => panic!("expected AccountId, got {other:?}"),
        }
    }

    #[test]
    fn author_matches_respects_account_id_exact() {
        let user = User {
            account_id: "557058:abc".into(),
            display_name: "Alice".into(),
            email_address: None,
            active: Some(true),
        };
        assert!(author_matches(
            Some(&user),
            &AuthorNeedle::AccountId("557058:abc".into())
        ));
        assert!(!author_matches(
            Some(&user),
            &AuthorNeedle::AccountId("other".into())
        ));
    }

    #[test]
    fn author_matches_null_author_always_false() {
        assert!(!author_matches(
            None,
            &AuthorNeedle::NameSubstring("alice".into())
        ));
    }

    fn entry(
        id: &str,
        created: &str,
        author: Option<&str>,
        items: Vec<ChangelogItem>,
    ) -> ChangelogEntry {
        ChangelogEntry {
            id: id.to_string(),
            author: author.map(|name| User {
                account_id: format!("acc-{name}"),
                display_name: name.to_string(),
                email_address: None,
                active: Some(true),
            }),
            created: created.to_string(),
            items,
        }
    }

    fn item(field: &str, from_s: Option<&str>, to_s: Option<&str>) -> ChangelogItem {
        ChangelogItem {
            field: field.to_string(),
            fieldtype: "jira".into(),
            from: None,
            from_string: from_s.map(String::from),
            to: None,
            to_string: to_s.map(String::from),
        }
    }

    #[test]
    fn build_rows_flattens_items_in_order() {
        let entries = vec![entry(
            "1",
            "2026-04-16T14:02:00.000+0000",
            Some("Alice"),
            vec![
                item("status", Some("To Do"), Some("In Progress")),
                item("resolution", None, Some("Done")),
            ],
        )];
        let rows = build_rows(&entries);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][2], "status");
        assert_eq!(rows[1][2], "resolution");
    }

    #[test]
    fn build_rows_uses_system_for_null_author() {
        let entries = vec![entry(
            "1",
            "2026-04-16T14:02:00.000+0000",
            None,
            vec![item("assignee", None, Some("Alice"))],
        )];
        let rows = build_rows(&entries);
        assert_eq!(rows[0][1], SYSTEM_AUTHOR);
    }

    #[test]
    fn from_to_display_renders_em_dash_for_empty() {
        assert_eq!(from_to_display(None, None), NULL_GLYPH);
        assert_eq!(from_to_display(Some(""), None), NULL_GLYPH);
        assert_eq!(from_to_display(None, Some("")), NULL_GLYPH);
    }

    #[test]
    fn from_to_display_prefers_string_over_raw() {
        assert_eq!(from_to_display(Some("Done"), Some("10000")), "Done");
        assert_eq!(from_to_display(None, Some("10000")), "10000");
    }

    #[test]
    fn format_date_converts_rfc3339_to_local() {
        // Just verify the shape; actual local conversion depends on runner TZ.
        let formatted = format_date("2026-04-16T14:02:11.000+0000");
        // YYYY-MM-DD HH:MM is 16 chars.
        assert_eq!(formatted.len(), 16, "got: {formatted}");
        assert!(formatted.starts_with("2026-04-"), "got: {formatted}");
    }

    #[test]
    fn format_date_falls_back_to_raw_on_parse_failure() {
        let garbage = "not-a-date";
        assert_eq!(format_date(garbage), garbage);
    }

    #[test]
    fn field_filter_semantics_at_item_level() {
        // Directly test the closure-equivalent logic by building entries.
        let mut entries = vec![entry(
            "1",
            "2026-04-16T14:02:00.000+0000",
            Some("Alice"),
            vec![
                item("status", Some("To Do"), Some("Done")),
                item("resolution", None, Some("Fixed")),
            ],
        )];

        // Simulate the filter logic.
        let needles = ["status"];
        for e in entries.iter_mut() {
            e.items.retain(|it| {
                let h = it.field.to_lowercase();
                needles.iter().any(|n| h.contains(&n.to_lowercase()))
            });
        }
        entries.retain(|e| !e.items.is_empty());

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].items.len(), 1);
        assert_eq!(entries[0].items[0].field, "status");
    }
}
