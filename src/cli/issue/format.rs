use crate::types::assets::LinkedAsset;
use crate::types::assets::linked::format_linked_assets_short;
use crate::types::jira::Issue;

/// Format issue rows for table output.
pub fn format_issue_rows_public(issues: &[Issue]) -> Vec<Vec<String>> {
    issues
        .iter()
        .map(|issue| format_issue_row(issue, None, None, None))
        .collect()
}

/// Build a single table row for an issue, optionally including story points,
/// linked assets, and team.
///
/// `team` is a per-row pre-resolved display string: caller looks up the team
/// UUID in the cache and passes the human-readable name or a fallback. When
/// the enclosing column is not shown (the `show_team` flag in
/// `issue_table_headers`), callers pass `None` and the slot is skipped.
pub fn format_issue_row(
    issue: &Issue,
    sp_field_id: Option<&str>,
    assets: Option<&[LinkedAsset]>,
    team: Option<&str>,
) -> Vec<String> {
    let col_count = 6
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
pub fn issue_table_headers(
    show_points: bool,
    show_assets: bool,
    show_team: bool,
) -> Vec<&'static str> {
    let mut headers = vec!["Key", "Type", "Status", "Priority"];
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
}
