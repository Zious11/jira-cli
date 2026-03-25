use anyhow::Result;

use crate::adf;
use crate::api::assets::linked::{
    enrich_assets, extract_linked_assets, get_or_fetch_cmdb_field_ids,
};
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::types::assets::LinkedAsset;
use crate::types::assets::linked::format_linked_assets;

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
        assets: show_assets,
    } = command
    else {
        unreachable!()
    };

    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
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

    let cmdb_field_ids = if show_assets {
        let ids = get_or_fetch_cmdb_field_ids(client)
            .await
            .unwrap_or_default();
        if ids.is_empty() {
            eprintln!(
                "warning: --assets ignored. No Assets custom fields found on this Jira instance."
            );
        }
        ids
    } else {
        Vec::new()
    };
    for f in &cmdb_field_ids {
        extra.push(f.as_str());
    }

    let search_result = client.search_issues(&effective_jql, limit, &extra).await?;
    let issues = search_result.issues;

    let effective_sp = resolve_show_points(show_points, sp_field_id);
    let show_assets_col = show_assets && !cmdb_field_ids.is_empty();
    let mut issue_assets: Vec<Vec<LinkedAsset>> = Vec::new();
    if show_assets_col {
        // Extract linked assets for all issues first.
        for issue in &issues {
            issue_assets.push(extract_linked_assets(&issue.fields.extra, &cmdb_field_ids));
        }

        // Collect unique (workspace_id, object_id) pairs that need enrichment,
        // then resolve them all in one batch to avoid redundant API calls.
        use std::collections::HashMap as StdHashMap;
        let mut to_enrich: StdHashMap<(String, String), ()> = StdHashMap::new();
        let mut enrich_indices: Vec<(usize, usize)> = Vec::new(); // (issue_idx, asset_idx)

        for (i, assets) in issue_assets.iter().enumerate() {
            for (j, asset) in assets.iter().enumerate() {
                if asset.id.is_some() && asset.key.is_none() && asset.name.is_none() {
                    let wid = asset.workspace_id.clone().unwrap_or_default();
                    let oid = asset.id.clone().unwrap();
                    let key = (wid, oid);
                    to_enrich.entry(key.clone()).or_insert(());
                    enrich_indices.push((i, j));
                }
            }
        }

        if !to_enrich.is_empty() {
            // Get workspace ID for assets that don't carry their own.
            let fallback_wid = crate::api::assets::workspace::get_or_fetch_workspace_id(client)
                .await
                .ok();

            let futures: Vec<_> = to_enrich
                .keys()
                .map(|(wid, oid)| {
                    let wid = if wid.is_empty() {
                        fallback_wid.clone().unwrap_or_default()
                    } else {
                        wid.clone()
                    };
                    let oid = oid.clone();
                    async move {
                        let result = client.get_asset(&wid, &oid, false).await;
                        (oid, result)
                    }
                })
                .collect();

            let results = futures::future::join_all(futures).await;
            let mut resolved: StdHashMap<String, (String, String, String)> = StdHashMap::new();
            for (oid, result) in results {
                if let Ok(obj) = result {
                    resolved.insert(oid, (obj.object_key, obj.label, obj.object_type.name));
                }
            }

            // Apply enrichment back to assets.
            for (i, j) in &enrich_indices {
                if let Some(oid) = &issue_assets[*i][*j].id.clone() {
                    if let Some((key, name, asset_type)) = resolved.get(oid) {
                        issue_assets[*i][*j].key = Some(key.clone());
                        issue_assets[*i][*j].name = Some(name.clone());
                        issue_assets[*i][*j].asset_type = Some(asset_type.clone());
                    }
                }
            }
        }
    }
    let rows: Vec<Vec<String>> = issues
        .iter()
        .enumerate()
        .map(|(i, issue)| {
            let assets = if show_assets_col {
                Some(issue_assets[i].as_slice())
            } else {
                None
            };
            format::format_issue_row(issue, effective_sp, assets)
        })
        .collect();
    let headers = format::issue_table_headers(effective_sp.is_some(), show_assets_col);
    output::print_output(output_format, &headers, &rows, &issues)?;

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

// ── Comments ─────────────────────────────────────────────────────────

fn format_comment_date(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .or_else(|_| chrono::DateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.3f%z"))
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

fn format_comment_row(
    author_name: Option<&str>,
    created: Option<&str>,
    body_text: Option<&str>,
) -> Vec<String> {
    vec![
        author_name.unwrap_or("(unknown)").to_string(),
        created
            .map(format_comment_date)
            .unwrap_or_else(|| "-".into()),
        body_text.unwrap_or("(no content)").to_string(),
    ]
}

pub(super) async fn handle_comments(
    key: &str,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let comments = client.list_comments(key, limit).await?;

    match output_format {
        OutputFormat::Json => {
            output::print_output(output_format, &["Author", "Date", "Body"], &[], &comments)?;
        }
        OutputFormat::Table => {
            let rows: Vec<Vec<String>> = comments
                .iter()
                .map(|c| {
                    let author = c.author.as_ref().map(|a| a.display_name.as_str());
                    let created = c.created.as_deref();
                    let body_text = c.body.as_ref().map(adf::adf_to_text);
                    format_comment_row(author, created, body_text.as_deref())
                })
                .collect();

            output::print_output(output_format, &["Author", "Date", "Body"], &rows, &comments)?;
        }
    }

    Ok(())
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
    let cmdb_field_ids = get_or_fetch_cmdb_field_ids(client)
        .await
        .unwrap_or_default();
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
    for f in &cmdb_field_ids {
        extra.push(f.as_str());
    }
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

            if !cmdb_field_ids.is_empty() {
                let mut linked = extract_linked_assets(&issue.fields.extra, &cmdb_field_ids);
                enrich_assets(client, &mut linked).await;
                let display = if linked.is_empty() {
                    "(none)".into()
                } else {
                    format_linked_assets(&linked)
                };
                rows.push(vec!["Assets".into(), display]);
            }

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

    #[test]
    fn format_comment_date_rfc3339() {
        assert_eq!(
            format_comment_date("2026-03-20T14:32:00+00:00"),
            "2026-03-20 14:32"
        );
    }

    #[test]
    fn format_comment_date_jira_offset_no_colon() {
        assert_eq!(
            format_comment_date("2026-03-20T14:32:00.000+0000"),
            "2026-03-20 14:32"
        );
    }

    #[test]
    fn format_comment_date_malformed_returns_raw() {
        assert_eq!(format_comment_date("not-a-date"), "not-a-date");
    }

    #[test]
    fn format_comment_row_missing_author() {
        let row = format_comment_row(None, Some("2026-03-20T14:32:00+00:00"), None);
        assert_eq!(row[0], "(unknown)");
    }

    #[test]
    fn format_comment_row_missing_body() {
        let row = format_comment_row(Some("Jane Smith"), Some("2026-03-20T14:32:00+00:00"), None);
        assert_eq!(row[2], "(no content)");
    }
}
