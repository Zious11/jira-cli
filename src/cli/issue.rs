use anyhow::{bail, Result};
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
        .map(|issue| {
            vec![
                issue.key.clone(),
                issue
                    .fields
                    .issue_type
                    .as_ref()
                    .map(|t| t.name.clone())
                    .unwrap_or_default(),
                issue
                    .fields
                    .status
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_default(),
                issue
                    .fields
                    .priority
                    .as_ref()
                    .map(|p| p.name.clone())
                    .unwrap_or_default(),
                issue
                    .fields
                    .assignee
                    .as_ref()
                    .map(|a| a.display_name.clone())
                    .unwrap_or_else(|| "Unassigned".into()),
                issue.fields.summary.clone(),
            ]
        })
        .collect()
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
        IssueCommand::List {
            jql,
            status,
            team,
            limit,
            points: _points,
        } => {
            handle_list(
                jql,
                status,
                team,
                limit,
                output_format,
                config,
                client,
                project_override,
                no_input,
            )
            .await
        }

        IssueCommand::View { key } => handle_view(&key, output_format, config, client).await,

        IssueCommand::Create {
            project,
            issue_type,
            summary,
            description,
            description_stdin,
            priority,
            label,
            team,
            points,
            markdown,
        } => {
            handle_create(
                project,
                issue_type,
                summary,
                description,
                description_stdin,
                priority,
                label,
                team,
                points,
                markdown,
                output_format,
                config,
                client,
                project_override,
                no_input,
            )
            .await
        }

        IssueCommand::Edit {
            key,
            summary,
            issue_type,
            priority,
            label,
            team,
            points,
            no_points,
        } => {
            handle_edit(
                &key,
                summary,
                issue_type,
                priority,
                label,
                team,
                points,
                no_points,
                output_format,
                config,
                client,
                no_input,
            )
            .await
        }

        IssueCommand::Move { key, status } => {
            handle_move(&key, status, output_format, client, no_input).await
        }

        IssueCommand::Transitions { key } => handle_transitions(&key, output_format, client).await,

        IssueCommand::Assign { key, to, unassign } => {
            handle_assign(&key, to, unassign, output_format, client).await
        }

        IssueCommand::Comment {
            key,
            message,
            markdown,
            file,
            stdin,
        } => handle_comment(&key, message, markdown, file, stdin, output_format, client).await,

        IssueCommand::Open { key, url_only } => handle_open(&key, url_only, client).await,
    }
}

// ── List ──────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn handle_list(
    jql: Option<String>,
    status: Option<String>,
    team: Option<String>,
    limit: Option<u32>,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
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

    let issues = client.search_issues(&effective_jql, limit, &[]).await?;
    let rows = format_issue_rows_public(&issues);

    output::print_output(
        output_format,
        &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
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

async fn handle_view(key: &str, output_format: &OutputFormat, config: &Config, client: &JiraClient) -> Result<()> {
    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let extra: Vec<&str> = sp_field_id.iter().copied().collect();
    let issue = client.get_issue(key, &extra).await?;

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

#[allow(clippy::too_many_arguments)]
async fn handle_create(
    project: Option<String>,
    issue_type: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    description_stdin: bool,
    priority: Option<String>,
    labels: Vec<String>,
    team: Option<String>,
    points: Option<f64>,
    markdown: bool,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
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

#[allow(clippy::too_many_arguments)]
async fn handle_edit(
    key: &str,
    summary: Option<String>,
    issue_type: Option<String>,
    priority: Option<String>,
    labels: Vec<String>,
    team: Option<String>,
    points: Option<f64>,
    no_points: bool,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
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
            let path = format!("/rest/api/3/issue/{}", urlencoding::encode(key));
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
            "No fields specified to update. Use --summary, --type, --priority, --label, --team, --points, or --no-points."
        );
    }

    client.edit_issue(key, fields).await?;

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
    key: &str,
    status: Option<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    // Get available transitions
    let transitions_resp = client.get_transitions(key).await?;
    let transitions = &transitions_resp.transitions;

    if transitions.is_empty() {
        bail!("No transitions available for {key}.");
    }

    // Check current status first
    let issue = client.get_issue(key, &[]).await?;
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
        .transition_issue(key, &selected_transition.id)
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
    key: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let resp = client.get_transitions(key).await?;

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
    key: &str,
    to: Option<String>,
    unassign: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    if unassign {
        client.assign_issue(key, None).await?;
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
        let issue = client.get_issue(key, &[]).await?;
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

    client.assign_issue(key, Some(&account_id)).await?;

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

#[allow(clippy::too_many_arguments)]
async fn handle_comment(
    key: &str,
    message: Option<String>,
    markdown: bool,
    file: Option<String>,
    stdin: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
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

    let comment = client.add_comment(key, adf_body).await?;

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

async fn handle_open(key: &str, url_only: bool, client: &JiraClient) -> Result<()> {
    let url = format!("{}/browse/{}", client.base_url(), key);

    if url_only {
        println!("{}", url);
    } else {
        open::that(&url)?;
        eprintln!("Opened {} in browser", key);
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
