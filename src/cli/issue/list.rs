use anyhow::Result;

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;

use super::format;
use super::helpers;

// ── List ──────────────────────────────────────────────────────────────

pub(super) async fn handle_list(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::List {
        jql,
        status,
        team,
        limit,
        points: show_points,
    } = command
    else {
        unreachable!()
    };

    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let extra: Vec<&str> = sp_field_id.iter().copied().collect();
    // Resolve team name to (field_id, uuid) before building JQL
    let resolved_team = if let Some(ref team_name) = team {
        Some(helpers::resolve_team_field(config, client, team_name, no_input).await?)
    } else {
        None
    };

    let effective_jql = if let Some(raw_jql) = jql {
        raw_jql
    } else {
        // Try smart defaults: detect board type and build JQL
        let board_id = config.project.board_id;
        let project_key = config.project_key(project_override);

        if let Some(bid) = board_id {
            // Detect board type
            match client.get_board_config(bid).await {
                Ok(board_config) => {
                    let board_type = board_config.board_type.to_lowercase();
                    if board_type == "scrum" {
                        // For scrum boards, find the active sprint
                        match client.list_sprints(bid, Some("active")).await {
                            Ok(sprints) if !sprints.is_empty() => {
                                let sprint = &sprints[0];
                                let mut jql_parts = vec![
                                    format!("sprint = {}", sprint.id),
                                    "assignee = currentUser()".to_string(),
                                ];
                                if let Some(ref s) = status {
                                    jql_parts.push(format!(
                                        "status = \"{}\"",
                                        crate::jql::escape_value(s)
                                    ));
                                }
                                if let Some((field_id, team_uuid)) = &resolved_team {
                                    jql_parts.push(format!(
                                        "{} = \"{}\"",
                                        field_id,
                                        crate::jql::escape_value(team_uuid)
                                    ));
                                }
                                let where_clause = jql_parts.join(" AND ");
                                format!("{} ORDER BY rank ASC", where_clause)
                            }
                            _ => build_fallback_jql(
                                project_key.as_deref(),
                                status.as_deref(),
                                resolved_team.as_ref(),
                            )?,
                        }
                    } else {
                        // Kanban: show open issues
                        let mut jql_parts: Vec<String> =
                            vec!["assignee = currentUser()".to_string()];
                        if let Some(ref pk) = project_key {
                            jql_parts
                                .push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
                        }
                        jql_parts.push("statusCategory != Done".into());
                        if let Some(ref s) = status {
                            jql_parts.push(format!("status = \"{}\"", crate::jql::escape_value(s)));
                        }
                        if let Some((field_id, team_uuid)) = &resolved_team {
                            jql_parts.push(format!(
                                "{} = \"{}\"",
                                field_id,
                                crate::jql::escape_value(team_uuid)
                            ));
                        }
                        let where_clause = jql_parts.join(" AND ");
                        format!("{} ORDER BY rank ASC", where_clause)
                    }
                }
                Err(_) => build_fallback_jql(
                    project_key.as_deref(),
                    status.as_deref(),
                    resolved_team.as_ref(),
                )?,
            }
        } else {
            build_fallback_jql(
                project_key.as_deref(),
                status.as_deref(),
                resolved_team.as_ref(),
            )?
        }
    };

    let issues = client.search_issues(&effective_jql, limit, &extra).await?;

    let effective_sp = resolve_show_points(show_points, sp_field_id);
    let rows: Vec<Vec<String>> = issues
        .iter()
        .map(|issue| format::format_issue_row(issue, effective_sp))
        .collect();
    output::print_output(
        output_format,
        &format::issue_table_headers(effective_sp.is_some()),
        &rows,
        &issues,
    )?;

    Ok(())
}

/// Resolve whether to show story points. Returns the field ID if points should
/// be shown, or None. Emits a warning to stderr if --points was requested but
/// config is missing.
fn resolve_show_points(show_points: bool, sp_field_id: Option<&str>) -> Option<&str> {
    if show_points {
        match sp_field_id {
            Some(id) => Some(id),
            None => {
                eprintln!(
                    "warning: --points ignored. Story points field not configured. \
                     Run \"jr init\" or set [fields].story_points_field_id in ~/.config/jr/config.toml"
                );
                None
            }
        }
    } else {
        None
    }
}

fn build_fallback_jql(
    project_key: Option<&str>,
    status: Option<&str>,
    resolved_team: Option<&(String, String)>,
) -> Result<String> {
    if project_key.is_none() && status.is_none() && resolved_team.is_none() {
        return Err(JrError::UserError(
            "No project or filters specified. Use --project KEY, --status STATUS, or --team NAME. \
             You can also set a default project in .jr.toml or run \"jr init\"."
                .into(),
        )
        .into());
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(pk) = project_key {
        parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
    }
    if let Some(s) = status {
        parts.push(format!("status = \"{}\"", crate::jql::escape_value(s)));
    }
    if let Some((field_id, team_uuid)) = resolved_team {
        parts.push(format!(
            "{} = \"{}\"",
            field_id,
            crate::jql::escape_value(team_uuid)
        ));
    }
    let where_clause = parts.join(" AND ");
    Ok(format!("{} ORDER BY updated DESC", where_clause))
}

// ── View ──────────────────────────────────────────────────────────────

pub(super) async fn handle_view(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::View { key } = command else {
        unreachable!()
    };

    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let extra: Vec<&str> = sp_field_id.iter().copied().collect();
    let issue = client.get_issue(&key, &extra).await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&issue)?);
        }
        OutputFormat::Table => {
            let desc_text = issue
                .fields
                .description
                .as_ref()
                .map(adf::adf_to_text)
                .unwrap_or_else(|| "(no description)".into());

            let mut rows = vec![
                vec!["Key".into(), issue.key.clone()],
                vec!["Summary".into(), issue.fields.summary.clone()],
                vec![
                    "Type".into(),
                    issue
                        .fields
                        .issue_type
                        .as_ref()
                        .map(|t| t.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Status".into(),
                    issue
                        .fields
                        .status
                        .as_ref()
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Priority".into(),
                    issue
                        .fields
                        .priority
                        .as_ref()
                        .map(|p| p.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Assignee".into(),
                    issue
                        .fields
                        .assignee
                        .as_ref()
                        .map(|a| a.display_name.clone())
                        .unwrap_or_else(|| "Unassigned".into()),
                ],
                vec![
                    "Project".into(),
                    issue
                        .fields
                        .project
                        .as_ref()
                        .map(|p| format!("{} ({})", p.name.as_deref().unwrap_or(""), p.key))
                        .unwrap_or_default(),
                ],
                vec![
                    "Labels".into(),
                    issue
                        .fields
                        .labels
                        .as_ref()
                        .filter(|l| !l.is_empty())
                        .map(|l| l.join(", "))
                        .unwrap_or_else(|| "(none)".into()),
                ],
            ];

            rows.push(vec![
                "Parent".into(),
                issue
                    .fields
                    .parent
                    .as_ref()
                    .map(|p| {
                        let summary = p
                            .fields
                            .as_ref()
                            .and_then(|f| f.summary.as_deref())
                            .unwrap_or("");
                        format!("{} ({})", p.key, summary)
                    })
                    .unwrap_or_else(|| "(none)".into()),
            ]);

            let links_display = issue
                .fields
                .issuelinks
                .as_ref()
                .filter(|links| !links.is_empty())
                .map(|links| {
                    links
                        .iter()
                        .map(|link| {
                            if let Some(ref outward) = link.outward_issue {
                                let desc = link
                                    .link_type
                                    .outward
                                    .as_deref()
                                    .unwrap_or(&link.link_type.name);
                                let summary = outward
                                    .fields
                                    .as_ref()
                                    .and_then(|f| f.summary.as_deref())
                                    .unwrap_or("");
                                format!("{} {} ({})", desc, outward.key, summary)
                            } else if let Some(ref inward) = link.inward_issue {
                                let desc = link
                                    .link_type
                                    .inward
                                    .as_deref()
                                    .unwrap_or(&link.link_type.name);
                                let summary = inward
                                    .fields
                                    .as_ref()
                                    .and_then(|f| f.summary.as_deref())
                                    .unwrap_or("");
                                format!("{} {} ({})", desc, inward.key, summary)
                            } else {
                                link.link_type.name.clone()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| "(none)".into());
            rows.push(vec!["Links".into(), links_display]);

            if let Some(field_id) = sp_field_id {
                let points_display = issue
                    .fields
                    .story_points(field_id)
                    .map(format::format_points)
                    .unwrap_or_else(|| "(none)".into());
                rows.push(vec!["Points".into(), points_display]);
            }

            rows.push(vec!["Description".into(), desc_text]);

            println!("{}", output::render_table(&["Field", "Value"], &rows));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_jql_order_by_not_joined_with_and() {
        let jql = build_fallback_jql(Some("PROJ"), None, None).unwrap();
        assert!(
            !jql.contains("AND ORDER BY"),
            "ORDER BY must not be joined with AND: {jql}"
        );
        assert!(jql.ends_with("ORDER BY updated DESC"));
    }

    #[test]
    fn fallback_jql_with_team_has_valid_order_by() {
        let team = ("customfield_10001".to_string(), "uuid-123".to_string());
        let jql = build_fallback_jql(Some("PROJ"), None, Some(&team)).unwrap();
        assert!(
            !jql.contains("AND ORDER BY"),
            "ORDER BY must not be joined with AND: {jql}"
        );
        assert!(jql.contains("customfield_10001 = \"uuid-123\""));
        assert!(jql.ends_with("ORDER BY updated DESC"));
    }

    #[test]
    fn fallback_jql_with_all_filters() {
        let team = ("customfield_10001".to_string(), "uuid-456".to_string());
        let jql = build_fallback_jql(Some("PROJ"), Some("In Progress"), Some(&team)).unwrap();
        assert!(
            !jql.contains("AND ORDER BY"),
            "ORDER BY must not be joined with AND: {jql}"
        );
        assert!(jql.contains("project = \"PROJ\""));
        assert!(jql.contains("status = \"In Progress\""));
        assert!(jql.contains("customfield_10001 = \"uuid-456\""));
        assert!(jql.ends_with("ORDER BY updated DESC"));
    }

    #[test]
    fn fallback_jql_errors_when_no_filters() {
        let result = build_fallback_jql(None, None, None);
        assert!(result.is_err(), "Expected error for unbounded query");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("--project"),
            "Error should mention --project: {err_msg}"
        );
        assert!(
            err_msg.contains(".jr.toml"),
            "Error should mention .jr.toml: {err_msg}"
        );
        assert!(
            err_msg.contains("jr init"),
            "Error should mention jr init: {err_msg}"
        );
    }

    #[test]
    fn fallback_jql_with_status_only() {
        let jql = build_fallback_jql(None, Some("Done"), None).unwrap();
        assert_eq!(jql, "status = \"Done\" ORDER BY updated DESC");
    }

    #[test]
    fn resolve_show_points_flag_false() {
        assert_eq!(resolve_show_points(false, Some("customfield_10031")), None);
        assert_eq!(resolve_show_points(false, None), None);
    }

    #[test]
    fn resolve_show_points_flag_true_config_present() {
        assert_eq!(
            resolve_show_points(true, Some("customfield_10031")),
            Some("customfield_10031")
        );
    }

    #[test]
    fn resolve_show_points_flag_true_config_missing() {
        // Warning emitted to stderr (not captured), but function returns None without error
        assert_eq!(resolve_show_points(true, None), None);
    }

    #[test]
    fn fallback_jql_escapes_special_chars_in_status() {
        let jql = build_fallback_jql(None, Some(r#"In "Progress"#), None).unwrap();
        assert!(
            jql.contains(r#"status = "In \"Progress""#),
            "Status with quotes should be escaped: {jql}"
        );
    }
}
