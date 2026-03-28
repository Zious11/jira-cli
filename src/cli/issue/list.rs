use anyhow::Result;

use crate::adf;
use crate::api::assets::linked::{
    enrich_assets, extract_linked_assets, get_or_fetch_cmdb_field_ids,
};
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat, resolve_effective_limit};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::types::assets::LinkedAsset;
use crate::types::assets::linked::format_linked_assets;

use super::format;
use super::helpers;

use crate::api::jira::projects::IssueTypeWithStatuses;
use crate::partial_match::{self, MatchResult};

/// Extract unique status names from project-scoped statuses response (deduplicated, sorted).
fn extract_unique_status_names(issue_types: &[IssueTypeWithStatuses]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();
    for it in issue_types {
        for s in &it.statuses {
            if seen.insert(s.name.clone()) {
                names.push(s.name.clone());
            }
        }
    }
    names.sort();
    names
}

// ── List ──────────────────────────────────────────────────────────────

/// Build base JQL parts when `--jql` is provided.
///
/// Returns `(base_parts, order_by)`. Strips any trailing `ORDER BY` clause
/// from `jql` and prepends the project scope if `project_key` is set.
fn build_jql_base_parts(jql: &str, project_key: Option<&str>) -> (Vec<String>, &'static str) {
    let stripped = crate::jql::strip_order_by(jql);
    let mut parts = Vec::new();

    if let Some(pk) = project_key {
        parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
    }
    if !stripped.is_empty() {
        parts.push(format!("({})", stripped));
    }

    (parts, "updated DESC")
}

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
        all,
        assignee,
        reporter,
        recent,
        open,
        points: show_points,
        assets: show_assets,
    } = command
    else {
        unreachable!()
    };

    let effective_limit = resolve_effective_limit(limit, all);

    // Validate --recent duration format early
    if let Some(ref d) = recent {
        crate::jql::validate_duration(d).map_err(JrError::UserError)?;
    }

    // Resolve --assignee and --reporter to JQL values
    let assignee_jql = if let Some(ref name) = assignee {
        Some(helpers::resolve_user(client, name, no_input).await?)
    } else {
        None
    };
    let reporter_jql = if let Some(ref name) = reporter {
        Some(helpers::resolve_user(client, name, no_input).await?)
    } else {
        None
    };

    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();

    // Resolve team name to (field_id, uuid) before building JQL
    let resolved_team = if let Some(ref team_name) = team {
        Some(helpers::resolve_team_field(config, client, team_name, no_input).await?)
    } else {
        None
    };

    // Build pre-formatted team clause for build_filter_clauses
    let team_clause = resolved_team.as_ref().map(|(field_id, team_uuid)| {
        format!("{} = \"{}\"", field_id, crate::jql::escape_value(team_uuid))
    });

    // Resolve project key once, before validation and JQL building
    let project_key = config.project_key(project_override);

    // Validate --project exists
    if let Some(ref pk) = project_key {
        // Skip if --status is set (project will be validated via statuses endpoint below)
        if status.is_none() && !client.project_exists(pk).await? {
            return Err(JrError::UserError(format!(
                "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
                pk
            ))
            .into());
        }
    }

    // Validate --status and resolve to exact name
    let resolved_status: Option<String> = if let Some(ref status_input) = status {
        let valid_statuses = if let Some(ref pk) = project_key {
            // Project-scoped: also validates project existence (404 = not found)
            match client.get_project_statuses(pk).await {
                Ok(issue_types) => extract_unique_status_names(&issue_types),
                Err(e) => {
                    if let Some(JrError::ApiError { status: 404, .. }) = e.downcast_ref::<JrError>()
                    {
                        return Err(JrError::UserError(format!(
                            "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
                            pk
                        ))
                        .into());
                    }
                    return Err(e);
                }
            }
        } else {
            client.get_all_statuses().await?
        };

        match partial_match::partial_match(status_input, &valid_statuses) {
            MatchResult::Exact(name) => Some(name),
            MatchResult::Ambiguous(matches) => {
                return Err(JrError::UserError(format!(
                    "Ambiguous status \"{}\". Matches: {}",
                    status_input,
                    matches.join(", ")
                ))
                .into());
            }
            MatchResult::None(all) => {
                let available = all.join(", ");
                let scope = if let Some(ref pk) = project_key {
                    format!(" for project {}", pk)
                } else {
                    String::new()
                };
                return Err(JrError::UserError(format!(
                    "No status matching \"{}\"{scope}. Available: {available}",
                    status_input,
                ))
                .into());
            }
        }
    } else {
        None
    };

    // Build filter clauses from all flag values
    let filter_parts = build_filter_clauses(
        assignee_jql.as_deref(),
        reporter_jql.as_deref(),
        resolved_status.as_deref(),
        team_clause.as_deref(),
        recent.as_deref(),
        open,
    );

    // Build base JQL + order by
    let (base_parts, order_by): (Vec<String>, &str) = if let Some(ref raw_jql) = jql {
        build_jql_base_parts(raw_jql, project_key.as_deref())
    } else {
        let board_id = config.project.board_id;

        if let Some(bid) = board_id {
            match client.get_board_config(bid).await {
                Ok(board_config) => {
                    let board_type = board_config.board_type.to_lowercase();
                    if board_type == "scrum" {
                        match client.list_sprints(bid, Some("active")).await {
                            Ok(sprints) if !sprints.is_empty() => {
                                let sprint = &sprints[0];
                                (vec![format!("sprint = {}", sprint.id)], "rank ASC")
                            }
                            _ => {
                                let mut parts = Vec::new();
                                if let Some(ref pk) = project_key {
                                    parts.push(format!(
                                        "project = \"{}\"",
                                        crate::jql::escape_value(pk)
                                    ));
                                }
                                (parts, "updated DESC")
                            }
                        }
                    } else {
                        // Kanban: statusCategory != Done, no implicit assignee
                        let mut parts = Vec::new();
                        if let Some(ref pk) = project_key {
                            parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
                        }
                        parts.push("statusCategory != Done".into());
                        (parts, "rank ASC")
                    }
                }
                Err(_) => {
                    let mut parts = Vec::new();
                    if let Some(ref pk) = project_key {
                        parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
                    }
                    (parts, "updated DESC")
                }
            }
        } else {
            let mut parts = Vec::new();
            if let Some(ref pk) = project_key {
                parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
            }
            (parts, "updated DESC")
        }
    };

    // Combine base + filters
    let mut all_parts = base_parts;
    all_parts.extend(filter_parts);

    // Guard against unbounded query
    if all_parts.is_empty() {
        return Err(JrError::UserError(
            "No project or filters specified. Use --project, --assignee, --reporter, --status, --open, --team, --recent, or --jql. \
             You can also set a default project in .jr.toml or run \"jr init\"."
                .into(),
        )
        .into());
    }

    let where_clause = all_parts.join(" AND ");
    let effective_jql = format!("{where_clause} ORDER BY {order_by}");

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

    let search_result = client
        .search_issues(&effective_jql, effective_limit, &extra)
        .await?;
    let has_more = search_result.has_more;
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

    if has_more && !all {
        let count_jql = crate::jql::strip_order_by(&effective_jql);
        match client.approximate_count(count_jql).await {
            Ok(total) if total > 0 => {
                eprintln!(
                    "Showing {} of ~{} results. Use --limit or --all to see more.",
                    issues.len(),
                    total
                );
            }
            Ok(_) | Err(_) => {
                eprintln!(
                    "Showing {} results. Use --limit or --all to see more.",
                    issues.len()
                );
            }
        }
    }

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

/// Build JQL filter clauses from resolved flag values.
fn build_filter_clauses(
    assignee_jql: Option<&str>,
    reporter_jql: Option<&str>,
    status: Option<&str>,
    team_clause: Option<&str>,
    recent: Option<&str>,
    open: bool,
) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(a) = assignee_jql {
        parts.push(format!("assignee = {a}"));
    }
    if let Some(r) = reporter_jql {
        parts.push(format!("reporter = {r}"));
    }
    if let Some(s) = status {
        parts.push(format!("status = \"{}\"", crate::jql::escape_value(s)));
    }
    if open {
        parts.push("statusCategory != Done".to_string());
    }
    if let Some(t) = team_clause {
        parts.push(t.to_string());
    }
    if let Some(d) = recent {
        parts.push(format!("created >= -{d}"));
    }
    parts
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
                    "Reporter".into(),
                    issue
                        .fields
                        .reporter
                        .as_ref()
                        .map(|r| r.display_name.clone())
                        .unwrap_or_else(|| "(none)".into()),
                ],
                vec![
                    "Created".into(),
                    issue
                        .fields
                        .created
                        .as_deref()
                        .map(format_comment_date)
                        .unwrap_or_else(|| "-".into()),
                ],
                vec![
                    "Updated".into(),
                    issue
                        .fields
                        .updated
                        .as_deref()
                        .map(format_comment_date)
                        .unwrap_or_else(|| "-".into()),
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

    #[test]
    fn build_jql_parts_assignee_me() {
        let parts = build_filter_clauses(Some("currentUser()"), None, None, None, None, false);
        assert_eq!(parts, vec!["assignee = currentUser()"]);
    }

    #[test]
    fn build_jql_parts_reporter_account_id() {
        let parts = build_filter_clauses(
            None,
            Some("5b10ac8d82e05b22cc7d4ef5"),
            None,
            None,
            None,
            false,
        );
        assert_eq!(parts, vec!["reporter = 5b10ac8d82e05b22cc7d4ef5"]);
    }

    #[test]
    fn build_jql_parts_recent() {
        let parts = build_filter_clauses(None, None, None, None, Some("7d"), false);
        assert_eq!(parts, vec!["created >= -7d"]);
    }

    #[test]
    fn build_jql_parts_all_filters() {
        let parts = build_filter_clauses(
            Some("currentUser()"),
            Some("currentUser()"),
            Some("In Progress"),
            Some(r#"customfield_10001 = "uuid-123""#),
            Some("30d"),
            false,
        );
        assert_eq!(parts.len(), 5);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"reporter = currentUser()".to_string()));
        assert!(parts.contains(&"status = \"In Progress\"".to_string()));
        assert!(parts.contains(&r#"customfield_10001 = "uuid-123""#.to_string()));
        assert!(parts.contains(&"created >= -30d".to_string()));
    }

    #[test]
    fn build_jql_parts_empty() {
        let parts = build_filter_clauses(None, None, None, None, None, false);
        assert!(parts.is_empty());
    }

    #[test]
    fn build_jql_parts_jql_plus_status_compose() {
        let filter = build_filter_clauses(None, None, Some("Done"), None, None, false);
        let mut all_parts = vec!["type = Bug".to_string()];
        all_parts.extend(filter);
        let jql = all_parts.join(" AND ");
        assert_eq!(jql, r#"type = Bug AND status = "Done""#);
    }

    #[test]
    fn build_jql_parts_status_escaping() {
        let parts =
            build_filter_clauses(None, None, Some(r#"He said "hi" \o/"#), None, None, false);
        assert_eq!(parts, vec![r#"status = "He said \"hi\" \\o/""#.to_string()]);
    }

    #[test]
    fn build_jql_parts_open() {
        let parts = build_filter_clauses(None, None, None, None, None, true);
        assert_eq!(parts, vec!["statusCategory != Done"]);
    }

    #[test]
    fn build_jql_parts_open_with_assignee() {
        let parts = build_filter_clauses(Some("currentUser()"), None, None, None, None, true);
        assert_eq!(parts.len(), 2);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"statusCategory != Done".to_string()));
    }

    #[test]
    fn build_jql_parts_all_filters_with_open() {
        let parts = build_filter_clauses(
            Some("currentUser()"),
            Some("currentUser()"),
            None, // status conflicts with open, so None here
            Some(r#"customfield_10001 = "uuid-123""#),
            Some("30d"),
            true,
        );
        assert_eq!(parts.len(), 5);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"reporter = currentUser()".to_string()));
        assert!(parts.contains(&"statusCategory != Done".to_string()));
        assert!(parts.contains(&r#"customfield_10001 = "uuid-123""#.to_string()));
        assert!(parts.contains(&"created >= -30d".to_string()));
    }

    #[test]
    fn build_jql_base_parts_jql_with_project() {
        let (parts, order_by) = build_jql_base_parts("priority = Highest", Some("PROJ"));
        assert_eq!(
            parts,
            vec![
                "project = \"PROJ\"".to_string(),
                "(priority = Highest)".to_string(),
            ]
        );
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_without_project() {
        let (parts, order_by) = build_jql_base_parts("priority = Highest", None);
        assert_eq!(parts, vec!["(priority = Highest)".to_string()]);
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_with_order_by_and_project() {
        let (parts, order_by) =
            build_jql_base_parts("priority = Highest ORDER BY created DESC", Some("PROJ"));
        assert_eq!(
            parts,
            vec![
                "project = \"PROJ\"".to_string(),
                "(priority = Highest)".to_string(),
            ]
        );
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_or_with_project_preserves_scope() {
        let (parts, order_by) =
            build_jql_base_parts("priority = Highest OR status = Done", Some("PROJ"));
        assert_eq!(
            parts,
            vec![
                "project = \"PROJ\"".to_string(),
                "(priority = Highest OR status = Done)".to_string(),
            ]
        );
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_order_by_only_with_project() {
        let (parts, order_by) = build_jql_base_parts("ORDER BY created DESC", Some("PROJ"));
        assert_eq!(parts, vec!["project = \"PROJ\"".to_string()]);
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_order_by_only_no_project() {
        let (parts, order_by) = build_jql_base_parts("ORDER BY created DESC", None);
        assert!(parts.is_empty());
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn extract_unique_status_names_deduplicates_and_sorts() {
        use crate::api::jira::projects::{IssueTypeWithStatuses, StatusMetadata};
        let issue_types = vec![
            IssueTypeWithStatuses {
                id: "1".into(),
                name: "Task".into(),
                subtask: None,
                statuses: vec![
                    StatusMetadata {
                        id: "10".into(),
                        name: "To Do".into(),
                        description: None,
                    },
                    StatusMetadata {
                        id: "20".into(),
                        name: "In Progress".into(),
                        description: None,
                    },
                    StatusMetadata {
                        id: "30".into(),
                        name: "Done".into(),
                        description: None,
                    },
                ],
            },
            IssueTypeWithStatuses {
                id: "2".into(),
                name: "Bug".into(),
                subtask: None,
                statuses: vec![
                    StatusMetadata {
                        id: "10".into(),
                        name: "To Do".into(),
                        description: None,
                    },
                    StatusMetadata {
                        id: "30".into(),
                        name: "Done".into(),
                        description: None,
                    },
                ],
            },
        ];
        let names = extract_unique_status_names(&issue_types);
        assert_eq!(names, vec!["Done", "In Progress", "To Do"]);
    }

    #[test]
    fn extract_unique_status_names_empty() {
        let names = extract_unique_status_names(&[]);
        assert!(names.is_empty());
    }
}
