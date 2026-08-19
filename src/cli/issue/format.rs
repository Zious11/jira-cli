use crate::types::assets::LinkedAsset;
use crate::types::assets::linked::format_linked_assets_short;
use crate::types::jira::Issue;
use crate::types::jira::issue::Comment;

/// Format issue rows for table output.
pub fn format_issue_rows_public(issues: &[Issue]) -> Vec<Vec<String>> {
    issues
        .iter()
        .map(|issue| format_issue_row(issue, None, None, None, None))
        .collect()
}

/// Build a single table row for an issue, optionally including a due date,
/// story points, linked assets, and team.
///
/// `duedate` follows the same shown/hidden convention as `sp_field_id`:
/// callers pass `None` to hide the column entirely, or `Some(value)` (where
/// `value` may be an empty string) to show it — `render_due_date` maps an
/// empty/absent value to `"-"`. See BC-2.2.032.
///
/// `team` is a per-row pre-resolved display string: caller looks up the team
/// UUID in the cache and passes the human-readable name or a fallback. When
/// the enclosing column is not shown (the `show_team` flag in
/// `issue_table_headers`), callers pass `None` and the slot is skipped.
pub fn format_issue_row(
    issue: &Issue,
    duedate: Option<&str>,
    sp_field_id: Option<&str>,
    assets: Option<&[LinkedAsset]>,
    team: Option<&str>,
) -> Vec<String> {
    let col_count = 6
        + if duedate.is_some() { 1 } else { 0 }
        + if sp_field_id.is_some() { 1 } else { 0 }
        + if assets.is_some() { 1 } else { 0 }
        + if team.is_some() { 1 } else { 0 };
    let mut row = Vec::with_capacity(col_count);
    row.push(issue.key.clone());
    row.push(
        issue
            .fields
            .issue_type
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_default(),
    );
    row.push(
        issue
            .fields
            .status
            .as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_default(),
    );
    row.push(
        issue
            .fields
            .priority
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default(),
    );
    if let Some(dd) = duedate {
        row.push(render_due_date(Some(dd)));
    }
    if let Some(field_id) = sp_field_id {
        row.push(
            issue
                .fields
                .story_points(field_id)
                .map(format_points)
                .unwrap_or_else(|| "-".into()),
        );
    }
    row.push(
        issue
            .fields
            .assignee
            .as_ref()
            .map(|a| a.display_name.clone())
            .unwrap_or_else(|| "Unassigned".into()),
    );
    if let Some(team_display) = team {
        row.push(team_display.to_string());
    }
    if let Some(linked) = assets {
        row.push(format_linked_assets_short(linked));
    }
    row.push(issue.fields.summary.clone());
    row
}

/// Headers matching `format_issue_row` output. `show_team` mirrors the
/// per-row `team` option: when true, each row must supply a `team` string.
/// `show_duedate` mirrors the per-row `duedate` option (BC-2.2.032); column
/// position is immediately after Priority, before Points.
pub fn issue_table_headers(
    show_duedate: bool,
    show_points: bool,
    show_assets: bool,
    show_team: bool,
) -> Vec<&'static str> {
    let mut headers = vec!["Key", "Type", "Status", "Priority"];
    if show_duedate {
        headers.push("Due Date");
    }
    if show_points {
        headers.push("Points");
    }
    headers.push("Assignee");
    if show_team {
        headers.push("Team");
    }
    if show_assets {
        headers.push("Assets");
    }
    headers.push("Summary");
    headers
}

pub fn format_points(value: f64) -> String {
    if !value.is_finite() {
        return "-".to_string();
    }
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}

/// Render a `duedate` value for display: the raw string verbatim when
/// present and non-empty, `"-"` otherwise (BC-2.2.032 / BC-2.3.039).
///
/// Deliberately NOT `format_comment_date` — that formatter parses RFC3339
/// **datetime** strings (`created`/`updated`); `duedate` is date-only
/// (`YYYY-MM-DD`) and gets no parser at all (verbatim-render design,
/// v1.3.179). Shared by both `view.rs` (always-on detail row) and
/// `format_issue_row` (opt-in `issue list --duedate` column) — AC-10.
pub(super) fn render_due_date(duedate: Option<&str>) -> String {
    match duedate {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => "-".to_string(),
    }
}

pub(super) fn format_comment_date(iso: &str, verbose: bool) -> String {
    use std::sync::atomic::AtomicBool;
    static LOGGED: AtomicBool = AtomicBool::new(false);
    match chrono::DateTime::parse_from_rfc3339(iso)
        .or_else(|_| chrono::DateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.3f%z"))
    {
        Ok(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => {
            // Label is "date" (not "comment") because this formatter is also
            // used by `handle_view` for the Created/Updated timestamp rows.
            crate::observability::log_parse_failure_once(&LOGGED, "date", iso, verbose);
            iso.to_string()
        }
    }
}

pub(super) fn format_comment_row(
    author_name: Option<&str>,
    created: Option<&str>,
    body_text: Option<&str>,
    verbose: bool,
) -> Vec<String> {
    vec![
        author_name.unwrap_or("(unknown)").to_string(),
        created
            .map(|c| format_comment_date(c, verbose))
            .unwrap_or_else(|| "-".into()),
        body_text.unwrap_or("(no content)").to_string(),
    ]
}

/// Extract the internal/external visibility from a comment's `sd.public.comment` property.
/// Returns `Some("Internal")` or `Some("External")` if the property exists, `None` otherwise.
pub(super) fn comment_visibility(comment: &Comment) -> Option<&'static str> {
    comment
        .properties
        .iter()
        .find(|p| p.key == "sd.public.comment")
        .map(|p| {
            if p.value.get("internal") == Some(&serde_json::Value::Bool(true)) {
                "Internal"
            } else {
                "External"
            }
        })
}

/// A single normalized `--component` add:/remove: entry (BC-3.4.022,
/// BC-3.4.012/BC-3.4.021 amendments). Bare `--component X` input (no prefix)
/// always normalizes to `Add` — it is NEVER rendered as a bare name in any
/// echo/preview surface (BC-3.4.012 EC-3.4.012-17, BC-3.4.021 amendment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ComponentAction {
    Add,
    Remove,
}

impl ComponentAction {
    /// Uppercase wire-verb string used both in the native `update`-verb PUT
    /// body's `add`/`remove` keys (BC-3.4.022 Postcondition 1) and the
    /// `--dry-run --output json` `plannedChanges.components[].action` field
    /// (BC-3.4.021 amendment, AC-016) — `"ADD"` / `"REMOVE"`.
    pub(super) fn as_wire_str(&self) -> &'static str {
        match self {
            ComponentAction::Add => "ADD",
            ComponentAction::Remove => "REMOVE",
        }
    }
}

/// Whether a resolved `--component` value is a component NAME or a numeric
/// component ID (Step-4.5 Round 3, F1 fix).
///
/// BC-8.4.001's numeric bypass means all-ASCII-digit `--component` input is
/// ALWAYS a component id, never a name (BC-8.1.008 -- identical to the
/// `component edit`/`delete`/`rename` command family convention). This
/// discriminator carries that fact from parse time (`for_input`, which
/// delegates to `helpers::is_numeric_component_id`, the single source of
/// truth for the bypass predicate -- FIX-F5) through resolution to the
/// wire-body construction site, so a numeric value is wired as `{"id": ...}`
/// and a name is wired as `{"name": ...}` -- never the reverse. Accepted edge
/// (matches BC-8.1.008's established gap): a component literally NAMED e.g.
/// `"10001"` is unreachable by name via `--component` -- numeric input is
/// always treated as an id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ComponentRefKind {
    /// Wired as `{"name": <value>}`.
    Name,
    /// Wired as `{"id": <value>}` (BC-8.4.001 numeric bypass).
    Id,
}

impl ComponentRefKind {
    /// Determine the ref kind directly from a raw `--component` value's
    /// text (BEFORE resolution) -- non-empty and all-ASCII-digit → `Id`,
    /// otherwise `Name`. Delegates to `helpers::is_numeric_component_id`,
    /// the single source of truth for the BC-8.4.001 step-1 bypass predicate
    /// (FIX-F5).
    pub(super) fn for_input(value: &str) -> Self {
        if super::helpers::is_numeric_component_id(value) {
            ComponentRefKind::Id
        } else {
            ComponentRefKind::Name
        }
    }
}

/// One normalized `--component` change: an action (ADD/REMOVE) paired with
/// the (not-yet-resolved, pre-`ref_kind`-determination) component name as
/// supplied on the CLI. `ref_kind` is determined at construction time from
/// the raw input text (see [`ComponentRefKind::for_input`]) and carried
/// through resolution unchanged -- resolution only replaces `name` with the
/// resolved canonical string; a numeric value's `name` is unchanged by
/// resolution (BC-8.4.001's numeric bypass is a no-op pass-through).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ComponentChange {
    pub(super) action: ComponentAction,
    pub(super) name: String,
    pub(super) ref_kind: ComponentRefKind,
}

/// Parse the repeatable `--component` flag's raw values into normalized
/// add:/remove: entries (BC-3.4.022).
///
/// - `add:<NAME>` / `remove:<NAME>` — explicit prefix grammar (edit-only;
///   `issue create` does NOT interpret this grammar — see
///   `create::resolve_create_components`).
/// - A bare value (no prefix) normalizes to `Add` (BC-3.4.022 Edge Case
///   EC-3.4.022-2 / BC-3.4.012 amendment EC-3.4.012-17).
/// - The returned `Vec` PRESERVES CLI input order (Step-4.5 Round 1, F2 fix
///   — BC-3.4.012/BC-3.4.013 amendments require the echo/dry-run preview to
///   render in CLI input order, e.g. `remove:Y, add:X` for
///   `--component remove:Y --component add:X`, mirroring labels'
///   EC-3.4.020-8 precedent: the live PUT wire body reorders to
///   ADD-before-REMOVE regardless of CLI order, but the wire is the ONLY
///   surface that reorders — it does so at its own construction site
///   (`edit_issue_components`'s `adds`/`removes` filters, merged into ONE
///   combined PUT by `handle_edit` alongside other field changes since
///   Step-4.5 Round 7's MEDIUM-1 fix), never here). Callers that need
///   the wire's ADD-before-REMOVE grouping must derive it themselves by
///   filtering this function's CLI-order output by `action` — do not expect
///   this function to have already grouped it.
pub(super) fn normalize_component_changes(values: &[String]) -> Vec<ComponentChange> {
    values
        .iter()
        .map(|value| {
            if let Some(name) = value.strip_prefix("add:") {
                ComponentChange {
                    action: ComponentAction::Add,
                    ref_kind: ComponentRefKind::for_input(name),
                    name: name.to_string(),
                }
            } else if let Some(name) = value.strip_prefix("remove:") {
                ComponentChange {
                    action: ComponentAction::Remove,
                    ref_kind: ComponentRefKind::for_input(name),
                    name: name.to_string(),
                }
            } else {
                // Bare value (no prefix) normalizes to Add (EC-3.4.022-2).
                ComponentChange {
                    action: ComponentAction::Add,
                    ref_kind: ComponentRefKind::for_input(value),
                    name: value.clone(),
                }
            }
        })
        .collect()
}

/// Render normalized component changes as the comma-joined echo string used
/// by BOTH the live table/JSON echo (BC-3.4.012/BC-3.4.013 amendments,
/// AC-011/AC-012/AC-013) and the `--dry-run` table preview (BC-3.4.021
/// amendment, AC-016/AC-017) — ONE shared formatter, per the story's
/// Architecture Compliance Rules ("identical string, one shared formatting
/// function"). Example: `"add:Backend, remove:Frontend"`.
pub(super) fn format_component_changes_echo(changes: &[ComponentChange]) -> String {
    changes
        .iter()
        .map(|c| match c.action {
            ComponentAction::Add => format!("add:{}", c.name),
            ComponentAction::Remove => format!("remove:{}", c.name),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render normalized component changes as the STRUCTURED
/// `plannedChanges.components` array shape used ONLY by
/// `--dry-run --output json` (BC-3.4.021 amendment, AC-016):
/// `[{"action":"ADD","name":"X"},{"action":"REMOVE","name":"Y"}]`. This is a
/// DIFFERENT shape from the comma-joined live-echo string built by
/// [`format_component_changes_echo`] — see BC-3.4.021 Behavior Summary.
pub(super) fn component_changes_dry_run_json(
    changes: &[ComponentChange],
) -> Vec<serde_json::Value> {
    changes
        .iter()
        .map(|c| serde_json::json!({"action": c.action.as_wire_str(), "name": c.name}))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_points_whole_number() {
        assert_eq!(format_points(5.0), "5");
        assert_eq!(format_points(13.0), "13");
        assert_eq!(format_points(0.0), "0");
    }

    #[test]
    fn format_points_decimal() {
        assert_eq!(format_points(3.5), "3.5");
        assert_eq!(format_points(0.5), "0.5");
    }

    #[test]
    fn format_points_non_finite() {
        assert_eq!(format_points(f64::NAN), "-");
        assert_eq!(format_points(f64::INFINITY), "-");
        assert_eq!(format_points(f64::NEG_INFINITY), "-");
    }

    #[test]
    fn format_comment_date_rfc3339() {
        assert_eq!(
            format_comment_date("2026-03-20T14:32:00+00:00", false),
            "2026-03-20 14:32"
        );
    }

    #[test]
    fn format_comment_date_jira_offset_no_colon() {
        assert_eq!(
            format_comment_date("2026-03-20T14:32:00.000+0000", false),
            "2026-03-20 14:32"
        );
    }

    #[test]
    fn format_comment_date_malformed_returns_raw() {
        assert_eq!(format_comment_date("not-a-date", false), "not-a-date");
    }

    #[test]
    fn format_comment_row_missing_author() {
        let row = format_comment_row(None, Some("2026-03-20T14:32:00+00:00"), None, false);
        assert_eq!(row[0], "(unknown)");
    }

    // AC-10 (BC-2.2.032 / BC-2.3.039): direct unit coverage on the shared
    // `render_due_date` helper, which is fully implemented (see the
    // function above — verbatim string when present, "-" otherwise).
    #[test]
    fn test_render_due_date_returns_verbatim_string_when_present() {
        assert_eq!(render_due_date(Some("2027-07-30")), "2027-07-30");
    }

    #[test]
    fn test_render_due_date_returns_dash_when_none() {
        assert_eq!(render_due_date(None), "-");
    }

    #[test]
    fn test_render_due_date_returns_dash_when_empty_string() {
        // EC-4 / defensive-only clause: Jira never actually emits an
        // empty-string duedate, but the helper must still treat it as
        // absent, matching the EC-2.7.001-3 empty-string convention.
        assert_eq!(render_due_date(Some("")), "-");
    }

    #[test]
    fn format_comment_row_missing_body() {
        let row = format_comment_row(
            Some("Jane Smith"),
            Some("2026-03-20T14:32:00+00:00"),
            None,
            false,
        );
        assert_eq!(row[2], "(no content)");
    }

    // ── BC-2.2.032 F1 Open Question #2: normative column order, with Due
    // Date and Points BOTH present ──────────────────────────────────────
    //
    // No prior test exercised `issue_table_headers`/`format_issue_row` with
    // every optional column shown simultaneously, so a swapped Due
    // Date/Points position would have gone undetected. These two tests
    // close that gap directly against the implementation, independent of
    // any CLI/config/cache plumbing.

    #[test]
    fn test_issue_table_headers_full_order_with_all_optional_columns() {
        let headers = issue_table_headers(true, true, true, true);
        assert_eq!(
            headers,
            vec![
                "Key", "Type", "Status", "Priority", "Due Date", "Points", "Assignee", "Team",
                "Assets", "Summary",
            ],
            "BC-2.2.032 F1 Open Question #2: the implementer MUST follow the \
             exact ordering Priority, Due Date, Points, Assignee, Team, \
             Assets, Summary — got: {headers:?}"
        );
    }

    #[test]
    fn test_format_issue_row_all_optional_columns_present_matches_header_order() {
        let issue: crate::types::jira::Issue = serde_json::from_value(serde_json::json!({
            "key": "PROJ-1",
            "fields": {
                "summary": "Ship the widget",
                "issuetype": {"name": "Task"},
                "status": {"name": "To Do"},
                "priority": {"name": "High"},
                "assignee": {"accountId": "abc123", "displayName": "Jane Smith"},
                "duedate": "2027-07-30",
                "customfield_10031": 5.0
            }
        }))
        .unwrap();

        let assets = vec![LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Acme Corp".into()),
            ..Default::default()
        }];

        let row = format_issue_row(
            &issue,
            Some("2027-07-30"),
            Some("customfield_10031"),
            Some(&assets),
            Some("Platform"),
        );
        let headers = issue_table_headers(true, true, true, true);

        assert_eq!(
            row.len(),
            headers.len(),
            "row/header length must match with all optional columns shown: \
             headers={headers:?}, row={row:?}"
        );
        // Positional order must match issue_table_headers' order exactly:
        // Key, Type, Status, Priority, Due Date, Points, Assignee, Team,
        // Assets, Summary.
        assert_eq!(row[0], "PROJ-1", "Key");
        assert_eq!(row[1], "Task", "Type");
        assert_eq!(row[2], "To Do", "Status");
        assert_eq!(row[3], "High", "Priority");
        assert_eq!(row[4], "2027-07-30", "Due Date");
        assert_eq!(row[5], "5", "Points");
        assert_eq!(row[6], "Jane Smith", "Assignee");
        assert_eq!(row[7], "Platform", "Team");
        assert_eq!(row[8], "Acme Corp", "Assets");
        assert_eq!(row[9], "Ship the widget", "Summary");
    }
}
