use anyhow::{Result, bail};
use serde_json::json;

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::output;
use crate::partial_match::{self, MatchResult};
use crate::types::jira::Issue;

/// Format issue rows for table output.
pub fn format_issue_rows_public(issues: &[Issue]) -> Vec<Vec<String>> {
    issues
        .iter()
        .map(|issue| format_issue_row(issue, None))
        .collect()
}

/// Build a single table row for an issue, optionally including story points.
pub fn format_issue_row(issue: &Issue, sp_field_id: Option<&str>) -> Vec<String> {
    let col_count = if sp_field_id.is_some() { 7 } else { 6 };
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
    row.push(issue.fields.summary.clone());
    row
}

/// Headers matching `format_issue_row` output.
pub fn issue_table_headers(show_points: bool) -> Vec<&'static str> {
    if show_points {
        vec![
            "Key", "Type", "Status", "Priority", "Points", "Assignee", "Summary",
        ]
    } else {
        vec!["Key", "Type", "Status", "Priority", "Assignee", "Summary"]
    }
}

/// Handle all issue subcommands.
pub async fn handle(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    match command {
        IssueCommand::List { .. } => {
            handle_list(
                command,
                output_format,
                config,
                client,
                project_override,
                no_input,
            )
            .await
        }
        IssueCommand::View { .. } => handle_view(command, output_format, config, client).await,
        IssueCommand::Create { .. } => {
            handle_create(
                command,
                output_format,
                config,
                client,
                project_override,
                no_input,
            )
            .await
        }
        IssueCommand::Edit { .. } => {
            handle_edit(command, output_format, config, client, no_input).await
        }
        IssueCommand::Move { .. } => handle_move(command, output_format, client, no_input).await,
        IssueCommand::Transitions { .. } => {
            handle_transitions(command, output_format, client).await
        }
        IssueCommand::Assign { .. } => handle_assign(command, output_format, client).await,
        IssueCommand::Comment { .. } => handle_comment(command, output_format, client).await,
        IssueCommand::Open { .. } => handle_open(command, client).await,
        IssueCommand::Link { .. } => handle_link(command, output_format, client, no_input).await,
        IssueCommand::Unlink { .. } => {
            handle_unlink(command, output_format, client, no_input).await
        }
        IssueCommand::LinkTypes => handle_link_types(output_format, client).await,
    }
}

// ── List ──────────────────────────────────────────────────────────────

async fn handle_list(
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
        Some(resolve_team_field(config, client, team_name, no_input).await?)
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
                                    jql_parts.push(format!("status = \"{}\"", s));
                                }
                                if let Some((field_id, team_uuid)) = &resolved_team {
                                    jql_parts.push(format!("{} = \"{}\"", field_id, team_uuid));
                                }
                                let where_clause = jql_parts.join(" AND ");
                                format!("{} ORDER BY rank ASC", where_clause)
                            }
                            _ => build_fallback_jql(
                                project_key.as_deref(),
                                status.as_deref(),
                                resolved_team.as_ref(),
                            ),
                        }
                    } else {
                        // Kanban: show open issues
                        let mut jql_parts: Vec<String> =
                            vec!["assignee = currentUser()".to_string()];
                        if let Some(ref pk) = project_key {
                            jql_parts.push(format!("project = \"{}\"", pk));
                        }
                        jql_parts.push("statusCategory != Done".into());
                        if let Some(ref s) = status {
                            jql_parts.push(format!("status = \"{}\"", s));
                        }
                        if let Some((field_id, team_uuid)) = &resolved_team {
                            jql_parts.push(format!("{} = \"{}\"", field_id, team_uuid));
                        }
                        let where_clause = jql_parts.join(" AND ");
                        format!("{} ORDER BY rank ASC", where_clause)
                    }
                }
                Err(_) => build_fallback_jql(
                    project_key.as_deref(),
                    status.as_deref(),
                    resolved_team.as_ref(),
                ),
            }
        } else {
            build_fallback_jql(
                project_key.as_deref(),
                status.as_deref(),
                resolved_team.as_ref(),
            )
        }
    };

    let issues = client.search_issues(&effective_jql, limit, &extra).await?;

    let effective_sp = if show_points { sp_field_id } else { None };
    let rows: Vec<Vec<String>> = issues
        .iter()
        .map(|issue| format_issue_row(issue, effective_sp))
        .collect();
    output::print_output(
        output_format,
        &issue_table_headers(effective_sp.is_some()),
        &rows,
        &issues,
    )?;

    Ok(())
}

fn build_fallback_jql(
    project_key: Option<&str>,
    status: Option<&str>,
    resolved_team: Option<&(String, String)>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(pk) = project_key {
        parts.push(format!("project = \"{}\"", pk));
    }
    if let Some(s) = status {
        parts.push(format!("status = \"{}\"", s));
    }
    if let Some((field_id, team_uuid)) = resolved_team {
        parts.push(format!("{} = \"{}\"", field_id, team_uuid));
    }
    let where_clause = parts.join(" AND ");
    format!("{} ORDER BY updated DESC", where_clause)
}

// ── View ──────────────────────────────────────────────────────────────

async fn handle_view(
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
                    .map(format_points)
                    .unwrap_or_else(|| "(none)".into());
                rows.push(vec!["Points".into(), points_display]);
            }

            rows.push(vec!["Description".into(), desc_text]);

            println!("{}", output::render_table(&["Field", "Value"], &rows));
        }
    }

    Ok(())
}

// ── Create ────────────────────────────────────────────────────────────

async fn handle_create(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Create {
        project,
        issue_type,
        summary,
        description,
        description_stdin,
        priority,
        label: labels,
        team,
        points,
        markdown,
        parent,
    } = command
    else {
        unreachable!()
    };

    // Resolve project key
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .or_else(|| {
            if no_input {
                None
            } else {
                prompt_input("Project key").ok()
            }
        })
        .ok_or_else(|| {
            anyhow::anyhow!("Project key is required. Use --project or configure .jr.toml")
        })?;

    // Resolve issue type
    let issue_type_name = issue_type
        .or_else(|| {
            if no_input {
                None
            } else {
                prompt_input("Issue type (e.g., Task, Bug, Story)").ok()
            }
        })
        .ok_or_else(|| anyhow::anyhow!("Issue type is required. Use --type"))?;

    // Resolve summary
    let summary_text = summary
        .or_else(|| {
            if no_input {
                None
            } else {
                prompt_input("Summary").ok()
            }
        })
        .ok_or_else(|| anyhow::anyhow!("Summary is required. Use --summary"))?;

    // Resolve description
    let desc_text = if description_stdin {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
        Some(buf)
    } else {
        description
    };

    // Build fields
    let mut fields = json!({
        "project": { "key": project_key },
        "issuetype": { "name": issue_type_name },
        "summary": summary_text,
    });

    if let Some(ref text) = desc_text {
        let adf_body = if markdown {
            adf::markdown_to_adf(text)
        } else {
            adf::text_to_adf(text)
        };
        fields["description"] = adf_body;
    }

    if let Some(ref prio) = priority {
        fields["priority"] = json!({ "name": prio });
    }

    if !labels.is_empty() {
        fields["labels"] = json!(labels);
    }

    if let Some(ref team_name) = team {
        let (field_id, team_id) = resolve_team_field(config, client, team_name, no_input).await?;
        fields[&field_id] = json!(team_id);
    }

    if let Some(pts) = points {
        let field_id = resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(pts);
    }

    if let Some(ref parent_key) = parent {
        fields["parent"] = json!({"key": parent_key});
    }

    let response = client.create_issue(fields).await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        OutputFormat::Table => {
            output::print_success(&format!("Created issue {}", response.key));
        }
    }

    Ok(())
}

// ── Edit ──────────────────────────────────────────────────────────────

async fn handle_edit(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Edit {
        key,
        summary,
        issue_type,
        priority,
        label: labels,
        team,
        points,
        no_points,
        parent,
    } = command
    else {
        unreachable!()
    };

    let mut fields = json!({});
    let mut has_updates = false;

    if let Some(ref s) = summary {
        fields["summary"] = json!(s);
        has_updates = true;
    }

    if let Some(ref t) = issue_type {
        fields["issuetype"] = json!({ "name": t });
        has_updates = true;
    }

    if let Some(ref p) = priority {
        fields["priority"] = json!({ "name": p });
        has_updates = true;
    }

    if let Some(ref team_name) = team {
        let (field_id, team_id) = resolve_team_field(config, client, team_name, no_input).await?;
        fields[&field_id] = json!(team_id);
        has_updates = true;
    }

    if let Some(pts) = points {
        let field_id = resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(pts);
        has_updates = true;
    }

    if no_points {
        let field_id = resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(null);
        has_updates = true;
    }

    if let Some(ref parent_key) = parent {
        fields["parent"] = json!({"key": parent_key});
        has_updates = true;
    }

    // Handle label add:/remove: syntax
    if !labels.is_empty() {
        let mut label_update: Vec<serde_json::Value> = Vec::new();
        for l in &labels {
            if let Some(to_add) = l.strip_prefix("add:") {
                label_update.push(json!({ "add": to_add }));
            } else if let Some(to_remove) = l.strip_prefix("remove:") {
                label_update.push(json!({ "remove": to_remove }));
            } else {
                // Treat bare label as add
                label_update.push(json!({ "add": l }));
            }
        }
        if !label_update.is_empty() {
            // Labels with add:/remove: syntax use the update endpoint pattern
            // We need to use the "update" key in the request body
            let path = format!("/rest/api/3/issue/{}", urlencoding::encode(&key));
            let mut body = json!({});
            if fields != json!({}) {
                body["fields"] = fields;
            }
            body["update"] = json!({ "labels": label_update });

            client.put(&path, &body).await?;

            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({ "key": key, "updated": true }))?
                    );
                }
                OutputFormat::Table => {
                    output::print_success(&format!("Updated {}", key));
                }
            }
            return Ok(());
        }
    }

    if !has_updates {
        bail!(
            "No fields specified to update. Use --summary, --type, --priority, --label, --team, --points, --no-points, or --parent."
        );
    }

    client.edit_issue(&key, fields).await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "key": key, "updated": true }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Updated {}", key));
        }
    }

    Ok(())
}

// ── Move (Transition) ────────────────────────────────────────────────

async fn handle_move(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Move { key, status } = command else {
        unreachable!()
    };

    // Get available transitions
    let transitions_resp = client.get_transitions(&key).await?;
    let transitions = &transitions_resp.transitions;

    if transitions.is_empty() {
        bail!("No transitions available for {key}.");
    }

    // Check current status first
    let issue = client.get_issue(&key, &[]).await?;
    let current_status = issue
        .fields
        .status
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_default();

    let target_status = match status {
        Some(s) => s,
        None => {
            if no_input {
                bail!("Target status is required in non-interactive mode.");
            }
            // Show transitions and prompt
            eprintln!("Available transitions for {}:", key);
            for (i, t) in transitions.iter().enumerate() {
                let to_name =
                    t.to.as_ref()
                        .map(|s| s.name.as_str())
                        .unwrap_or("(unknown)");
                eprintln!("  {}. {} -> {}", i + 1, t.name, to_name);
            }

            prompt_input("Select transition (name or number)")?
        }
    };

    // Idempotent: if already in target status, exit 0
    if current_status.to_lowercase() == target_status.to_lowercase() {
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "key": key,
                        "status": current_status,
                        "changed": false
                    }))?
                );
            }
            OutputFormat::Table => {
                output::print_success(&format!(
                    "{} is already in status \"{}\"",
                    key, current_status
                ));
            }
        }
        return Ok(());
    }

    // Try to match by number first
    let selected_transition = if let Ok(num) = target_status.parse::<usize>() {
        if num >= 1 && num <= transitions.len() {
            Some(&transitions[num - 1])
        } else {
            None
        }
    } else {
        None
    };

    let selected_transition = if let Some(t) = selected_transition {
        t
    } else {
        // Use partial matching on transition names
        let transition_names: Vec<String> = transitions.iter().map(|t| t.name.clone()).collect();
        match partial_match::partial_match(&target_status, &transition_names) {
            MatchResult::Exact(name) => transitions.iter().find(|t| t.name == name).unwrap(),
            MatchResult::Ambiguous(matches) => {
                if no_input {
                    bail!(
                        "Ambiguous transition \"{}\". Matches: {}",
                        target_status,
                        matches.join(", ")
                    );
                }
                // Interactive disambiguation
                eprintln!(
                    "Ambiguous match for \"{}\". Did you mean one of:",
                    target_status
                );
                for (i, m) in matches.iter().enumerate() {
                    eprintln!("  {}. {}", i + 1, m);
                }
                let choice = prompt_input("Select (number)")?;
                let idx: usize = choice
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid selection"))?;
                if idx < 1 || idx > matches.len() {
                    bail!("Selection out of range");
                }
                transitions
                    .iter()
                    .find(|t| t.name == matches[idx - 1])
                    .unwrap()
            }
            MatchResult::None(all) => {
                bail!(
                    "No transition matching \"{}\". Available: {}",
                    target_status,
                    all.join(", ")
                );
            }
        }
    };

    client
        .transition_issue(&key, &selected_transition.id)
        .await?;

    let new_status = selected_transition
        .to
        .as_ref()
        .map(|s| s.name.as_str())
        .unwrap_or(&selected_transition.name);

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "key": key,
                    "status": new_status,
                    "changed": true
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Moved {} to \"{}\"", key, new_status));
        }
    }

    Ok(())
}

// ── Transitions ───────────────────────────────────────────────────────

async fn handle_transitions(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Transitions { key } = command else {
        unreachable!()
    };

    let resp = client.get_transitions(&key).await?;

    let rows: Vec<Vec<String>> = resp
        .transitions
        .iter()
        .map(|t| {
            vec![
                t.id.clone(),
                t.name.clone(),
                t.to.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "Name", "To Status"],
        &rows,
        &resp.transitions,
    )?;

    Ok(())
}

// ── Assign ────────────────────────────────────────────────────────────

async fn handle_assign(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Assign { key, to, unassign } = command else {
        unreachable!()
    };

    if unassign {
        client.assign_issue(&key, None).await?;
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "key": key,
                        "assignee": null,
                        "changed": true
                    }))?
                );
            }
            OutputFormat::Table => {
                output::print_success(&format!("Unassigned {}", key));
            }
        }
        return Ok(());
    }

    let account_id = if let Some(ref user_query) = to {
        // Assign to another user — use the provided value as account ID
        user_query.clone()
    } else {
        // Assign to self
        let me = client.get_myself().await?;

        // Idempotent: check if already assigned to self
        let issue = client.get_issue(&key, &[]).await?;
        if let Some(ref assignee) = issue.fields.assignee {
            if assignee.account_id == me.account_id {
                match output_format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "key": key,
                                "assignee": me.display_name,
                                "changed": false
                            }))?
                        );
                    }
                    OutputFormat::Table => {
                        output::print_success(&format!("{} is already assigned to you", key));
                    }
                }
                return Ok(());
            }
        }

        me.account_id
    };

    client.assign_issue(&key, Some(&account_id)).await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "key": key,
                    "assignee": account_id,
                    "changed": true
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Assigned {} to {}", key, account_id));
        }
    }

    Ok(())
}

// ── Comment ───────────────────────────────────────────────────────────

async fn handle_comment(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Comment {
        key,
        message,
        markdown,
        file,
        stdin,
    } = command
    else {
        unreachable!()
    };

    // Resolve comment text from the various sources
    let text = if stdin {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
        buf
    } else if let Some(ref path) = file {
        std::fs::read_to_string(path)?
    } else if let Some(ref msg) = message {
        msg.clone()
    } else {
        bail!("Comment text is required. Use a positional argument, --file, or --stdin.");
    };

    let text = text.trim().to_string();
    if text.is_empty() {
        bail!("Comment text cannot be empty.");
    }

    let adf_body = if markdown {
        adf::markdown_to_adf(&text)
    } else {
        adf::text_to_adf(&text)
    };

    let comment = client.add_comment(&key, adf_body).await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&comment)?);
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Added comment to {} (id: {})",
                key,
                comment.id.as_deref().unwrap_or("unknown")
            ));
        }
    }

    Ok(())
}

// ── Open ──────────────────────────────────────────────────────────────

async fn handle_open(command: IssueCommand, client: &JiraClient) -> Result<()> {
    let IssueCommand::Open { key, url_only } = command else {
        unreachable!()
    };

    let url = format!("{}/browse/{}", client.base_url(), key);

    if url_only {
        println!("{}", url);
    } else {
        open::that(&url)?;
        eprintln!("Opened {} in browser", key);
    }

    Ok(())
}

// ── Link Types ────────────────────────────────────────────────────

async fn handle_link_types(output_format: &OutputFormat, client: &JiraClient) -> Result<()> {
    let link_types = client.list_link_types().await?;

    let rows: Vec<Vec<String>> = link_types
        .iter()
        .map(|lt| {
            vec![
                lt.id.clone().unwrap_or_default(),
                lt.name.clone(),
                lt.outward.clone().unwrap_or_default(),
                lt.inward.clone().unwrap_or_default(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "Name", "Outward", "Inward"],
        &rows,
        &link_types,
    )?;

    Ok(())
}

// ── Link ──────────────────────────────────────────────────────────

async fn handle_link(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Link {
        key1,
        key2,
        r#type: link_type_name,
    } = command
    else {
        unreachable!()
    };

    if key1.eq_ignore_ascii_case(&key2) {
        bail!("Cannot link an issue to itself.");
    }

    let link_types = client.list_link_types().await?;
    let type_names: Vec<String> = link_types.iter().map(|lt| lt.name.clone()).collect();
    let resolved_name = match partial_match::partial_match(&link_type_name, &type_names) {
        MatchResult::Exact(name) => name,
        MatchResult::Ambiguous(matches) => {
            if no_input {
                bail!(
                    "Ambiguous link type \"{}\". Matches: {}",
                    link_type_name,
                    matches.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple types match \"{link_type_name}\""))
                .items(&matches)
                .interact()?;
            matches[selection].clone()
        }
        MatchResult::None(_) => {
            bail!(
                "Unknown link type \"{}\". Run \"jr issue link-types\" to see available types.",
                link_type_name
            );
        }
    };

    client
        .create_issue_link(&key1, &key2, &resolved_name)
        .await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "key1": key1,
                    "key2": key2,
                    "type": resolved_name,
                    "linked": true
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Linked {} → {} ({})", key1, key2, resolved_name));
        }
    }

    Ok(())
}

// ── Unlink ────────────────────────────────────────────────────────

async fn handle_unlink(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Unlink {
        key1,
        key2,
        r#type: link_type_filter,
    } = command
    else {
        unreachable!()
    };

    let resolved_type_filter = if let Some(ref type_name) = link_type_filter {
        let link_types = client.list_link_types().await?;
        let type_names: Vec<String> = link_types.iter().map(|lt| lt.name.clone()).collect();
        let resolved = match partial_match::partial_match(type_name, &type_names) {
            MatchResult::Exact(name) => name,
            MatchResult::Ambiguous(matches) => {
                if no_input {
                    bail!(
                        "Ambiguous link type \"{}\". Matches: {}",
                        type_name,
                        matches.join(", ")
                    );
                }
                let selection = dialoguer::Select::new()
                    .with_prompt(format!("Multiple types match \"{type_name}\""))
                    .items(&matches)
                    .interact()?;
                matches[selection].clone()
            }
            MatchResult::None(_) => {
                bail!(
                    "Unknown link type \"{}\". Run \"jr issue link-types\" to see available types.",
                    type_name
                );
            }
        };
        Some(resolved)
    } else {
        None
    };

    let issue = client.get_issue(&key1, &[]).await?;
    let links = issue.fields.issuelinks.unwrap_or_default();

    let matching_links: Vec<&crate::types::jira::issue::IssueLink> = links
        .iter()
        .filter(|link| {
            let matches_key = link
                .outward_issue
                .as_ref()
                .map(|i| i.key.eq_ignore_ascii_case(&key2))
                .unwrap_or(false)
                || link
                    .inward_issue
                    .as_ref()
                    .map(|i| i.key.eq_ignore_ascii_case(&key2))
                    .unwrap_or(false);

            let matches_type = resolved_type_filter
                .as_ref()
                .map(|t| link.link_type.name.eq_ignore_ascii_case(t))
                .unwrap_or(true);

            matches_key && matches_type
        })
        .collect();

    if matching_links.is_empty() {
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "unlinked": false,
                        "count": 0
                    }))?
                );
            }
            OutputFormat::Table => {
                output::print_success(&format!("No link found between {} and {}", key1, key2));
            }
        }
        return Ok(());
    }

    let count = matching_links.len();
    for link in &matching_links {
        client.delete_issue_link(&link.id).await?;
    }

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "unlinked": true,
                    "count": count
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Removed {} link(s) between {} and {}",
                count, key1, key2
            ));
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────

async fn resolve_team_field(
    config: &Config,
    client: &JiraClient,
    team_name: &str,
    no_input: bool,
) -> Result<(String, String)> {
    // 1. Resolve team_field_id
    let field_id = if let Some(id) = &config.global.fields.team_field_id {
        id.clone()
    } else {
        client
            .find_team_field_id()
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No \"Team\" field found on this Jira instance. This instance may not have the Team field configured."
                )
            })?
    };

    // 2. Load teams from cache (or fetch if missing/expired)
    let teams = match crate::cache::read_team_cache()? {
        Some(cached) => cached.teams,
        None => crate::cli::team::fetch_and_cache_teams(config, client).await?,
    };

    // 3. Partial match
    let team_names: Vec<String> = teams.iter().map(|t| t.name.clone()).collect();
    match crate::partial_match::partial_match(team_name, &team_names) {
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let team = teams
                .iter()
                .find(|t| t.name == matched_name)
                .expect("matched name must exist in teams");
            Ok((field_id, team.id.clone()))
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            if no_input {
                let quoted: Vec<String> = matches.iter().map(|m| format!("\"{}\"", m)).collect();
                anyhow::bail!(
                    "Multiple teams match \"{}\": {}. Use a more specific name.",
                    team_name,
                    quoted.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple teams match \"{team_name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let team = teams
                .iter()
                .find(|t| &t.name == selected_name)
                .expect("selected name must exist in teams");
            Ok((field_id, team.id.clone()))
        }
        crate::partial_match::MatchResult::None(_) => {
            anyhow::bail!(
                "No team matching \"{}\". Run \"jr team list --refresh\" to update.",
                team_name
            );
        }
    }
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

fn resolve_story_points_field_id(config: &Config) -> Result<String> {
    config
        .global
        .fields
        .story_points_field_id
        .clone()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Story points field not configured. Run \"jr init\" or set story_points_field_id under [fields] in ~/.config/jr/config.toml"
            )
        })
}

fn prompt_input(prompt: &str) -> Result<String> {
    let input: String = dialoguer::Input::new()
        .with_prompt(prompt)
        .interact_text()?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_jql_order_by_not_joined_with_and() {
        let jql = build_fallback_jql(Some("PROJ"), None, None);
        assert!(
            !jql.contains("AND ORDER BY"),
            "ORDER BY must not be joined with AND: {jql}"
        );
        assert!(jql.ends_with("ORDER BY updated DESC"));
    }

    #[test]
    fn fallback_jql_with_team_has_valid_order_by() {
        let team = ("customfield_10001".to_string(), "uuid-123".to_string());
        let jql = build_fallback_jql(Some("PROJ"), None, Some(&team));
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
        let jql = build_fallback_jql(Some("PROJ"), Some("In Progress"), Some(&team));
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
    fn fallback_jql_no_filters_still_has_order_by() {
        let jql = build_fallback_jql(None, None, None);
        assert_eq!(jql, " ORDER BY updated DESC");
    }

    #[test]
    fn fallback_jql_with_status_only() {
        let jql = build_fallback_jql(None, Some("Done"), None);
        assert_eq!(jql, "status = \"Done\" ORDER BY updated DESC");
    }

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
