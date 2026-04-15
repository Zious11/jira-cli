use anyhow::Result;

use crate::adf;
use crate::api::assets::linked::{
    cmdb_field_ids, enrich_assets, enrich_json_assets, extract_linked_assets,
    get_or_fetch_cmdb_fields,
};
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat, resolve_effective_limit};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::types::assets::LinkedAsset;
use crate::types::assets::linked::format_linked_assets;
use crate::types::jira::issue::Comment;

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
        asset: asset_key,
        created_after,
        created_before,
        updated_after,
        updated_before,
    } = command
    else {
        unreachable!()
    };

    let effective_limit = resolve_effective_limit(limit, all);

    // Auto-enable assets display column when filtering by asset
    let show_assets = show_assets || asset_key.is_some();

    // Validate --recent duration format early
    if let Some(ref d) = recent {
        crate::jql::validate_duration(d).map_err(JrError::UserError)?;
    }

    // Validate date filter flags early (before any network calls)
    let created_after_date = if let Some(ref d) = created_after {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
    let created_before_date = if let Some(ref d) = created_before {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
    let updated_after_date = if let Some(ref d) = updated_after {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
    let updated_before_date = if let Some(ref d) = updated_before {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };

    // Build date filter JQL clauses
    let created_after_clause = created_after_date.map(|d| format!("created >= \"{}\"", d));
    let created_before_clause = created_before_date.map(|d| {
        let next_day = d + chrono::Days::new(1);
        format!("created < \"{}\"", next_day)
    });
    let updated_after_clause = updated_after_date.map(|d| format!("updated >= \"{}\"", d));
    let updated_before_clause = updated_before_date.map(|d| {
        let next_day = d + chrono::Days::new(1);
        format!("updated < \"{}\"", next_day)
    });

    // Resolve --asset: key passthrough or name → key via AQL search
    let asset_key = if let Some(raw) = asset_key {
        Some(helpers::resolve_asset(client, &raw, no_input).await?)
    } else {
        None
    };

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

    // Resolve CMDB fields for --asset filter (needs field names for aqlFunction)
    let (asset_clause, asset_cmdb_fields) = if let Some(ref key) = asset_key {
        let cmdb_fields = get_or_fetch_cmdb_fields(client).await?;
        if cmdb_fields.is_empty() {
            return Err(JrError::UserError(
                "--asset requires Assets custom fields on this Jira instance. \
                 Assets requires a paid Jira Service Management plan."
                    .into(),
            )
            .into());
        }
        let clause = crate::jql::build_asset_clause(key, &cmdb_fields);
        (Some(clause), Some(cmdb_fields))
    } else {
        (None, None)
    };

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
            // Case-sensitive dedup upstream; treat like Exact if case-variant duplicates slip through
            MatchResult::ExactMultiple(name) => Some(name),
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
    let filter_parts = build_filter_clauses(FilterOptions {
        assignee_jql: assignee_jql.as_deref(),
        reporter_jql: reporter_jql.as_deref(),
        status: resolved_status.as_deref(),
        team_clause: team_clause.as_deref(),
        recent: recent.as_deref(),
        open,
        asset_clause: asset_clause.as_deref(),
        created_after_clause: created_after_clause.as_deref(),
        created_before_clause: created_before_clause.as_deref(),
        updated_after_clause: updated_after_clause.as_deref(),
        updated_before_clause: updated_before_clause.as_deref(),
    });

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
                            Ok(_) => {
                                // No active sprint — fall back to project-scoped JQL
                                let mut parts = Vec::new();
                                if let Some(ref pk) = project_key {
                                    parts.push(format!(
                                        "project = \"{}\"",
                                        crate::jql::escape_value(pk)
                                    ));
                                }
                                (parts, "updated DESC")
                            }
                            Err(e) => {
                                return Err(e.context(format!(
                                    "Failed to list sprints for board {}. \
                                     Use --jql to query directly.",
                                    bid
                                )));
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
                Err(e) => {
                    if let Some(JrError::ApiError { status: 404, .. }) = e.downcast_ref::<JrError>()
                    {
                        return Err(JrError::UserError(format!(
                            "Board {} not found or not accessible. \
                             Verify the board exists and you have permission, \
                             or remove board_id from .jr.toml. \
                             Use --jql to query directly.",
                            bid
                        ))
                        .into());
                    }
                    return Err(e.context(format!(
                        "Failed to fetch config for board {}. \
                         Remove board_id from .jr.toml or use --jql to query directly.",
                        bid
                    )));
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
            "No project or filters specified. Use --project, --assignee, --reporter, --status, --open, --team, --recent, --created-after, --created-before, --updated-after, --updated-before, --asset, or --jql. \
             You can also set a default project in .jr.toml or run \"jr init\"."
                .into(),
        )
        .into());
    }

    let where_clause = all_parts.join(" AND ");
    let effective_jql = format!("{where_clause} ORDER BY {order_by}");

    let cmdb_fields = if show_assets {
        if let Some(fields) = asset_cmdb_fields {
            fields
        } else {
            let fields = get_or_fetch_cmdb_fields(client).await.unwrap_or_default();
            if fields.is_empty() {
                eprintln!(
                    "warning: --assets ignored. No Assets custom fields found on this Jira instance."
                );
            }
            fields
        }
    } else {
        Vec::new()
    };
    let cmdb_field_id_list = cmdb_field_ids(&cmdb_fields);
    for f in &cmdb_field_id_list {
        extra.push(f.as_str());
    }

    let search_result = client
        .search_issues(&effective_jql, effective_limit, &extra)
        .await?;
    let has_more = search_result.has_more;
    let mut issues = search_result.issues;

    let effective_sp = resolve_show_points(show_points, sp_field_id);
    let show_assets_col = show_assets && !cmdb_field_id_list.is_empty();
    let mut issue_assets: Vec<Vec<LinkedAsset>> = Vec::new();
    if show_assets_col {
        // Extract linked assets for all issues first.
        for issue in &issues {
            issue_assets.push(extract_linked_assets(
                &issue.fields.extra,
                &cmdb_field_id_list,
            ));
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

    // For JSON output with --assets, inject enriched data back into issue JSON
    if show_assets_col && matches!(output_format, OutputFormat::Json) {
        for (i, issue) in issues.iter_mut().enumerate() {
            if issue_assets[i].is_empty() {
                continue;
            }
            // Build per-field-id enrichment: re-extract per field to get grouping,
            // then match by position to enriched issue_assets[i]
            let mut per_field_by_id: Vec<(String, Vec<LinkedAsset>)> = Vec::new();
            let mut offset = 0;
            for field_id in &cmdb_field_id_list {
                let count =
                    extract_linked_assets(&issue.fields.extra, std::slice::from_ref(field_id))
                        .len();
                if count > 0 && offset + count <= issue_assets[i].len() {
                    let enriched = issue_assets[i][offset..offset + count].to_vec();
                    per_field_by_id.push((field_id.clone(), enriched));
                }
                offset += count;
            }
            enrich_json_assets(&mut issue.fields.extra, &per_field_by_id);
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

/// Options bag for `build_filter_clauses` — groups all resolved JQL filter
/// fragments so the function stays within clippy's argument-count limit.
struct FilterOptions<'a> {
    assignee_jql: Option<&'a str>,
    reporter_jql: Option<&'a str>,
    status: Option<&'a str>,
    team_clause: Option<&'a str>,
    recent: Option<&'a str>,
    open: bool,
    asset_clause: Option<&'a str>,
    created_after_clause: Option<&'a str>,
    created_before_clause: Option<&'a str>,
    updated_after_clause: Option<&'a str>,
    updated_before_clause: Option<&'a str>,
}

/// Build JQL filter clauses from resolved flag values.
fn build_filter_clauses(opts: FilterOptions<'_>) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(a) = opts.assignee_jql {
        parts.push(format!("assignee = {a}"));
    }
    if let Some(r) = opts.reporter_jql {
        parts.push(format!("reporter = {r}"));
    }
    if let Some(s) = opts.status {
        parts.push(format!("status = \"{}\"", crate::jql::escape_value(s)));
    }
    if opts.open {
        parts.push("statusCategory != Done".to_string());
    }
    if let Some(t) = opts.team_clause {
        parts.push(t.to_string());
    }
    if let Some(d) = opts.recent {
        parts.push(format!("created >= -{d}"));
    }
    if let Some(a) = opts.asset_clause {
        parts.push(a.to_string());
    }
    if let Some(c) = opts.created_after_clause {
        parts.push(c.to_string());
    }
    if let Some(c) = opts.created_before_clause {
        parts.push(c.to_string());
    }
    if let Some(c) = opts.updated_after_clause {
        parts.push(c.to_string());
    }
    if let Some(c) = opts.updated_before_clause {
        parts.push(c.to_string());
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

/// Extract the internal/external visibility from a comment's `sd.public.comment` property.
/// Returns `Some("Internal")` or `Some("External")` if the property exists, `None` otherwise.
fn comment_visibility(comment: &Comment) -> Option<&'static str> {
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

pub(super) async fn handle_comments(
    key: &str,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let comments = client.list_comments(key, limit).await?;

    // Show Visibility column only if any comment has sd.public.comment property
    let has_visibility = comments.iter().any(|c| comment_visibility(c).is_some());

    match output_format {
        OutputFormat::Json => {
            output::print_output(output_format, &[], &[], &comments)?;
        }
        OutputFormat::Table => {
            let (headers, rows) = if has_visibility {
                let rows: Vec<Vec<String>> = comments
                    .iter()
                    .map(|c| {
                        let author = c.author.as_ref().map(|a| a.display_name.as_str());
                        let created = c.created.as_deref();
                        let body_text = c.body.as_ref().map(adf::adf_to_text);
                        let visibility = comment_visibility(c).unwrap_or("External");
                        let mut row = format_comment_row(author, created, body_text.as_deref());
                        // Insert Visibility before Body (index 2)
                        row.insert(2, visibility.to_string());
                        row
                    })
                    .collect();
                (vec!["Author", "Date", "Visibility", "Body"], rows)
            } else {
                let rows: Vec<Vec<String>> = comments
                    .iter()
                    .map(|c| {
                        let author = c.author.as_ref().map(|a| a.display_name.as_str());
                        let created = c.created.as_deref();
                        let body_text = c.body.as_ref().map(adf::adf_to_text);
                        format_comment_row(author, created, body_text.as_deref())
                    })
                    .collect();
                (vec!["Author", "Date", "Body"], rows)
            };

            output::print_output(output_format, &headers, &rows, &comments)?;
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
    let team_field_id: Option<&str> = config.global.fields.team_field_id.as_deref();
    let cmdb_fields = get_or_fetch_cmdb_fields(client).await.unwrap_or_default();
    let cmdb_field_id_list = cmdb_field_ids(&cmdb_fields);
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
    for f in &cmdb_field_id_list {
        extra.push(f.as_str());
    }
    if let Some(t) = team_field_id {
        extra.push(t);
    }
    let mut issue = client.get_issue(&key, &extra).await?;

    // Extract and enrich assets per-field (shared by both JSON and table paths).
    // Iterate cmdb_fields directly so we always have (field_id, field_name) together —
    // avoids any name-based reverse lookups that could break with duplicate field names.
    let per_field_enriched: Vec<(String, String, Vec<LinkedAsset>)> = if !cmdb_fields.is_empty() {
        // Extract per-field, keeping both ID and name
        let mut per_field: Vec<(String, String, Vec<LinkedAsset>)> = Vec::new();
        for (field_id, field_name) in &cmdb_fields {
            let assets = extract_linked_assets(&issue.fields.extra, std::slice::from_ref(field_id));
            if !assets.is_empty() {
                per_field.push((field_id.clone(), field_name.clone(), assets));
            }
        }

        // Collect all assets for batch enrichment
        let mut all_assets: Vec<LinkedAsset> = per_field
            .iter()
            .flat_map(|(_, _, assets)| assets.clone())
            .collect();
        enrich_assets(client, &mut all_assets).await;

        // Redistribute enriched assets back
        let mut enriched = Vec::new();
        let mut offset = 0;
        for (field_id, field_name, original_assets) in &per_field {
            let count = original_assets.len();
            let assets = all_assets[offset..offset + count].to_vec();
            offset += count;
            enriched.push((field_id.clone(), field_name.clone(), assets));
        }
        enriched
    } else {
        Vec::new()
    };

    match output_format {
        OutputFormat::Json => {
            // Inject enriched data back into JSON before printing
            if !per_field_enriched.is_empty() {
                let per_field_by_id: Vec<(String, Vec<LinkedAsset>)> = per_field_enriched
                    .iter()
                    .map(|(id, _, assets)| (id.clone(), assets.clone()))
                    .collect();
                enrich_json_assets(&mut issue.fields.extra, &per_field_by_id);
            }
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

            // Per-field asset rows (replaces the old single "Assets" row)
            for (_, field_name, assets) in &per_field_enriched {
                let display = format_linked_assets(assets);
                rows.push(vec![field_name.clone(), display]);
            }

            if let Some(field_id) = sp_field_id {
                let points_display = issue
                    .fields
                    .story_points(field_id)
                    .map(format::format_points)
                    .unwrap_or_else(|| "(none)".into());
                rows.push(vec!["Points".into(), points_display]);
            }

            if let Some(field_id) = team_field_id {
                if let Some(team_uuid) = issue.fields.team_id(field_id) {
                    let team_display = match crate::cache::read_team_cache()
                        .ok()
                        .flatten()
                        .and_then(|c| c.teams.into_iter().find(|t| t.id == team_uuid))
                    {
                        Some(cached) => cached.name,
                        None => format!(
                            "{} (name not cached — run 'jr team list --refresh')",
                            team_uuid
                        ),
                    };
                    rows.push(vec!["Team".into(), team_display]);
                }
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
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["assignee = currentUser()"]);
    }

    #[test]
    fn build_jql_parts_reporter_account_id() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: Some("5b10ac8d82e05b22cc7d4ef5"),
            status: None,
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["reporter = 5b10ac8d82e05b22cc7d4ef5"]);
    }

    #[test]
    fn build_jql_parts_recent() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: Some("7d"),
            open: false,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["created >= -7d"]);
    }

    #[test]
    fn build_jql_parts_all_filters() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: Some("currentUser()"),
            status: Some("In Progress"),
            team_clause: Some(r#"customfield_10001 = "uuid-123""#),
            recent: Some("30d"),
            open: false,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 5);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"reporter = currentUser()".to_string()));
        assert!(parts.contains(&"status = \"In Progress\"".to_string()));
        assert!(parts.contains(&r#"customfield_10001 = "uuid-123""#.to_string()));
        assert!(parts.contains(&"created >= -30d".to_string()));
    }

    #[test]
    fn build_jql_parts_empty() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert!(parts.is_empty());
    }

    #[test]
    fn build_jql_parts_jql_plus_status_compose() {
        let filter = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: Some("Done"),
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        let mut all_parts = vec!["type = Bug".to_string()];
        all_parts.extend(filter);
        let jql = all_parts.join(" AND ");
        assert_eq!(jql, r#"type = Bug AND status = "Done""#);
    }

    #[test]
    fn build_jql_parts_status_escaping() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: Some(r#"He said "hi" \o/"#),
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec![r#"status = "He said \"hi\" \\o/""#.to_string()]);
    }

    #[test]
    fn build_jql_parts_open() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            open: true,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["statusCategory != Done"]);
    }

    #[test]
    fn build_jql_parts_open_with_assignee() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            open: true,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 2);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"statusCategory != Done".to_string()));
    }

    #[test]
    fn build_jql_parts_all_filters_with_open() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: Some("currentUser()"),
            status: None, // status conflicts with open, so None here
            team_clause: Some(r#"customfield_10001 = "uuid-123""#),
            recent: Some("30d"),
            open: true,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 5);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"reporter = currentUser()".to_string()));
        assert!(parts.contains(&"statusCategory != Done".to_string()));
        assert!(parts.contains(&r#"customfield_10001 = "uuid-123""#.to_string()));
        assert!(parts.contains(&"created >= -30d".to_string()));
    }

    #[test]
    fn build_jql_parts_asset_clause() {
        let clause = r#""Client" IN aqlFunction("Key = \"CUST-5\"")"#;
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: Some(clause),
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec![clause.to_string()]);
    }

    #[test]
    fn build_jql_parts_asset_with_assignee() {
        let clause = r#""Client" IN aqlFunction("Key = \"CUST-5\"")"#;
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: Some(clause),
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 2);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&clause.to_string()));
    }

    #[test]
    fn build_jql_parts_created_after_clause() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: None,
            created_after_clause: Some("created >= \"2026-03-18\""),
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["created >= \"2026-03-18\""]);
    }

    #[test]
    fn build_jql_parts_updated_after_and_before_clauses() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: None,
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: Some("updated >= \"2026-03-01\""),
            updated_before_clause: Some("updated < \"2026-04-01\""),
        });
        assert_eq!(parts.len(), 2);
        assert!(parts.contains(&"updated >= \"2026-03-01\"".to_string()));
        assert!(parts.contains(&"updated < \"2026-04-01\"".to_string()));
    }

    #[test]
    fn build_jql_parts_created_date_range() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            open: false,
            asset_clause: None,
            created_after_clause: Some("created >= \"2026-03-01\""),
            created_before_clause: Some("created < \"2026-04-01\""),
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 2);
        assert!(parts.contains(&"created >= \"2026-03-01\"".to_string()));
        assert!(parts.contains(&"created < \"2026-04-01\"".to_string()));
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
