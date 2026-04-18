use anyhow::Result;
use chrono::DateTime;
use serde::Serialize;

use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::output;
use crate::types::jira::ChangelogEntry;

use super::helpers;

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
        limit,
        all,
        field,
        author,
        reverse,
    } = command
    else {
        unreachable!("handler only called for IssueCommand::Changelog")
    };

    // Resolve --author "me" (case-insensitive, shared with other commands
    // via `helpers::is_me_keyword`) up-front; other forms compare directly.
    let author_needle = match author.as_deref() {
        Some(raw) if helpers::is_me_keyword(raw) => Some(AuthorNeedle::AccountId(
            client.get_myself().await?.account_id,
        )),
        Some(raw) => Some(classify_author(raw)),
        None => None,
    };

    let mut entries = client.get_changelog(&key).await?;

    // Sort chronologically by parsed `created`. Unparseable entries fall
    // back to lexicographic comparison on the raw string, preserving a
    // deterministic order across re-runs even if a future API response
    // uses a format we don't recognize yet.
    let cmp = |a: &ChangelogEntry, b: &ChangelogEntry| match (
        parse_created(&a.created),
        parse_created(&b.created),
    ) {
        (Some(ax), Some(bx)) => ax.cmp(&bx),
        _ => a.created.cmp(&b.created),
    };
    if reverse {
        entries.sort_by(cmp);
    } else {
        entries.sort_by(|a, b| cmp(b, a));
    }

    // --author filter: entries with no author never match (we don't
    // support matching against null explicitly).
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

    // Truncate to cap rows (one row per ChangelogItem), unless --all is set.
    // `--limit` applies to ROWS, not entries — a user asking for `--limit 10`
    // expects 10 rows in the table. Defaults to `cli::DEFAULT_LIMIT` (30).
    if let Some(n) = crate::cli::resolve_effective_limit(limit, all) {
        truncate_to_rows(&mut entries, n as usize);
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
/// accountId if it either contains a colon, or is ≥12 chars of
/// `[A-Za-z0-9_-]` containing at least one digit. Otherwise it's a
/// name substring.
///
/// The API's accountId format varies (`public cloud` uses
/// `557058:...`-style strings; older formats are opaque 24+ char
/// hex-like blobs). Both documented formats guarantee digits, so the
/// digit requirement distinguishes them from long digit-free display
/// names like `AlexanderGreene` or `jean-pierre-dupont`. Residual
/// edge: a 12+ char single-word name that incidentally contains a
/// digit (e.g. `User12345Name`) still classifies as accountId; see
/// issue #213 for the rationale.
fn classify_author(raw: &str) -> AuthorNeedle {
    let trimmed = raw.trim();
    let looks_like_account_id = trimmed.contains(':')
        || (trimmed.len() >= 12
            && trimmed.chars().any(|c| c.is_ascii_digit())
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

/// Parse a Jira ISO-8601 `created` timestamp. Returns `None` if neither
/// RFC3339 (`+00:00`) nor the Jira-style compact-offset (`+0000`) format
/// matches. Shared by `format_date` (rendering) and the sort comparator
/// so mixed offset formats in a single response sort chronologically
/// rather than lexicographically.
fn parse_created(iso: &str) -> Option<DateTime<chrono::FixedOffset>> {
    DateTime::parse_from_rfc3339(iso)
        .or_else(|_| DateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.3f%z"))
        .ok()
}

/// Render a Jira ISO-8601 timestamp as `YYYY-MM-DD HH:MM` in the user's
/// local time zone. Falls back to the raw string if parsing fails.
fn format_date(iso: &str) -> String {
    parse_created(iso)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| iso.to_string())
}

/// Truncate entries so the total row count (sum of items across all
/// surviving entries) does not exceed `cap`. Trims the last entry's
/// items if necessary rather than dropping a whole entry with only
/// some items over the cap.
fn truncate_to_rows(entries: &mut Vec<ChangelogEntry>, cap: usize) {
    if cap == 0 {
        entries.clear();
        return;
    }
    let mut running = 0usize;
    for i in 0..entries.len() {
        let n = entries[i].items.len();
        if running + n <= cap {
            running += n;
            continue;
        }
        // Partially trim this entry, drop everything after.
        let keep = cap - running;
        entries[i].items.truncate(keep);
        entries.truncate(if keep == 0 { i } else { i + 1 });
        return;
    }
}

/// Prefer the human-readable string; fall back to the raw id; default to
/// the em-dash null marker for empty/missing values.
fn from_to_display(string: Option<&str>, raw: Option<&str>) -> String {
    // Treat empty/whitespace strings as "absent" so an empty `fromString`
    // falls through to the raw `from` (or vice-versa) before rendering the
    // null glyph. Without this, `Some("")` would be "picked" and rendered
    // as `—`, hiding a meaningful raw value.
    let s = string.map(str::trim).filter(|t| !t.is_empty());
    let r = raw.map(str::trim).filter(|t| !t.is_empty());
    match s.or(r) {
        Some(value) => value.to_string(),
        None => NULL_GLYPH.to_string(),
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
    fn classify_author_long_alpha_only_name_is_substring() {
        // 15 chars, no digits — regression guard for #213.
        match classify_author("AlexanderGreene") {
            AuthorNeedle::NameSubstring(s) => assert_eq!(s, "alexandergreene"),
            other => panic!("expected NameSubstring, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_long_compound_name_is_substring() {
        // 18 chars, no digits — regression guard for #213.
        match classify_author("JoseMariaRodriguez") {
            AuthorNeedle::NameSubstring(s) => assert_eq!(s, "josemariarodriguez"),
            other => panic!("expected NameSubstring, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_long_hyphenated_name_is_substring() {
        // 18 chars with dashes, no digits — regression guard for #213.
        match classify_author("jean-pierre-dupont") {
            AuthorNeedle::NameSubstring(s) => assert_eq!(s, "jean-pierre-dupont"),
            other => panic!("expected NameSubstring, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_old_hex_accountid_is_accountid() {
        // 24-char hex — contains digits, no colon.
        match classify_author("5b10ac8d82e05b22cc7d4ef5") {
            AuthorNeedle::AccountId(s) => assert_eq!(s, "5b10ac8d82e05b22cc7d4ef5"),
            other => panic!("expected AccountId, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_colon_forces_accountid_regardless_of_heuristics() {
        // Colon wins the branch regardless of length/digits.
        match classify_author("557058:f58131cb-b67d-43c7") {
            AuthorNeedle::AccountId(s) => assert_eq!(s, "557058:f58131cb-b67d-43c7"),
            other => panic!("expected AccountId, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_long_name_with_digit_is_accountid() {
        // 13 chars with a digit — documented residual edge. Stays AccountId.
        match classify_author("User12345Name") {
            AuthorNeedle::AccountId(s) => assert_eq!(s, "User12345Name"),
            other => panic!("expected AccountId, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_short_hyphenated_name_is_substring() {
        // 11 chars — below the length gate, unaffected by the digit rule.
        match classify_author("jean-pierre") {
            AuthorNeedle::NameSubstring(s) => assert_eq!(s, "jean-pierre"),
            other => panic!("expected NameSubstring, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_unknown_placeholder_is_substring() {
        // 7-char "unknown" — the Jira stub for deleted/migrated users.
        // Below the length gate; NameSubstring path already matches it
        // via case-insensitive account_id containment.
        match classify_author("unknown") {
            AuthorNeedle::NameSubstring(s) => assert_eq!(s, "unknown"),
            other => panic!("expected NameSubstring, got {other:?}"),
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
    fn from_to_display_empty_string_falls_back_to_raw() {
        // `Some("")` on the string side must not block the fallback to raw —
        // empty/whitespace strings should be treated as absent.
        assert_eq!(from_to_display(Some(""), Some("10000")), "10000");
        assert_eq!(from_to_display(Some("   "), Some("10000")), "10000");
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
    fn parse_created_accepts_both_offset_formats() {
        // Jira-style compact offset.
        let jira = parse_created("2026-04-16T14:02:11.000+0000").unwrap();
        // RFC3339 colon offset, same instant.
        let rfc = parse_created("2026-04-16T14:02:11.000+00:00").unwrap();
        assert_eq!(jira, rfc);
    }

    #[test]
    fn sort_uses_parsed_datetime_across_mixed_offset_formats() {
        // Two entries one minute apart, but using different offset
        // formats. Lexicographic comparison of `created` would misorder
        // them (':' > '0' so "+00:00" sorts after "+0000"), but parsed
        // DateTime orders them correctly.
        let older = entry(
            "older",
            "2026-04-16T14:02:00.000+0000",
            Some("A"),
            vec![item("status", Some("To Do"), Some("Done"))],
        );
        let newer = entry(
            "newer",
            "2026-04-16T14:03:00.000+00:00",
            Some("A"),
            vec![item("status", Some("Done"), Some("Reopened"))],
        );

        // Start with older before newer (API order).
        let mut entries = [older.clone(), newer.clone()];

        // Apply the same comparator the handler uses.
        let cmp = |a: &ChangelogEntry, b: &ChangelogEntry| match (
            parse_created(&a.created),
            parse_created(&b.created),
        ) {
            (Some(ax), Some(bx)) => ax.cmp(&bx),
            _ => a.created.cmp(&b.created),
        };

        // DESC (default): newer first.
        entries.sort_by(|a, b| cmp(b, a));
        assert_eq!(entries[0].id, "newer");
        assert_eq!(entries[1].id, "older");

        // ASC (--reverse): older first.
        entries.sort_by(cmp);
        assert_eq!(entries[0].id, "older");
        assert_eq!(entries[1].id, "newer");
    }

    #[test]
    fn truncate_to_rows_handles_cap_zero() {
        let mut entries = vec![entry(
            "1",
            "2026-04-16T14:02:00.000+0000",
            Some("A"),
            vec![item("status", None, Some("Done"))],
        )];
        truncate_to_rows(&mut entries, 0);
        assert!(entries.is_empty());
    }

    #[test]
    fn truncate_to_rows_trims_last_entry_partially() {
        let mut entries = vec![
            entry(
                "1",
                "2026-04-16T14:02:00.000+0000",
                Some("A"),
                vec![
                    item("status", None, Some("Done")),
                    item("resolution", None, Some("Fixed")),
                ],
            ),
            entry(
                "2",
                "2026-04-15T00:00:00.000+0000",
                Some("A"),
                vec![item("labels", None, Some("x"))],
            ),
        ];
        // cap = 2 → keep both items of entry 1, drop entry 2 entirely.
        truncate_to_rows(&mut entries, 2);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].items.len(), 2);
    }

    #[test]
    fn truncate_to_rows_partial_trim_inside_entry() {
        let mut entries = vec![entry(
            "1",
            "2026-04-16T14:02:00.000+0000",
            Some("A"),
            vec![
                item("status", None, Some("Done")),
                item("resolution", None, Some("Fixed")),
                item("labels", None, Some("x")),
            ],
        )];
        truncate_to_rows(&mut entries, 2);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].items.len(), 2);
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
