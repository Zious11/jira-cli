use anyhow::{Result, bail};
use serde_json::json;

use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, SprintCommand};
use crate::config::Config;
use crate::output;
use crate::types::jira::Issue;

/// Handle all sprint subcommands.
pub async fn handle(
    command: SprintCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    match command {
        SprintCommand::List { board } => {
            let board_id = resolve_scrum_board(config, client, board, project_override).await?;
            handle_list(board_id, client, output_format).await
        }
        SprintCommand::Current {
            board, limit, all, ..
        } => {
            let board_id = resolve_scrum_board(config, client, board, project_override).await?;
            handle_current(board_id, client, output_format, config, limit, all).await
        }
        SprintCommand::Add {
            sprint,
            current,
            issues,
            board,
        } => {
            if issues.len() > MAX_SPRINT_ISSUES {
                bail!(
                    "Too many issues (got {}). Maximum is {} per operation.",
                    issues.len(),
                    MAX_SPRINT_ISSUES
                );
            }
            let sprint_id = if current {
                let board_id = resolve_scrum_board(config, client, board, project_override).await?;
                let sprints = client.list_sprints(board_id, Some("active")).await?;
                if sprints.is_empty() {
                    bail!("No active sprint found for board {}.", board_id);
                }
                sprints[0].id
            } else {
                sprint.expect("clap enforces --sprint when --current is absent")
            };
            handle_add(sprint_id, issues, output_format, client).await
        }
        SprintCommand::Remove { issues } => {
            if issues.len() > MAX_SPRINT_ISSUES {
                bail!(
                    "Too many issues (got {}). Maximum is {} per operation.",
                    issues.len(),
                    MAX_SPRINT_ISSUES
                );
            }
            handle_remove(issues, output_format, client).await
        }
    }
}

/// Resolve board ID and verify it's a scrum board.
async fn resolve_scrum_board(
    config: &Config,
    client: &JiraClient,
    board: Option<u64>,
    project_override: Option<&str>,
) -> Result<u64> {
    let board_id =
        crate::cli::board::resolve_board_id(config, client, board, project_override, true).await?;

    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();
    if board_type != "scrum" {
        bail!(
            "Sprint commands are only available for scrum boards. Board {} is a {} board.",
            board_id,
            board_config.board_type
        );
    }

    Ok(board_id)
}

/// JSON response for `sprint add`.
fn sprint_add_response(sprint_id: u64, issues: &[String]) -> serde_json::Value {
    json!({
        "sprint_id": sprint_id,
        "issues": issues,
        "added": true
    })
}

/// JSON response for `sprint remove`.
fn sprint_remove_response(issues: &[String]) -> serde_json::Value {
    json!({
        "issues": issues,
        "removed": true
    })
}

const MAX_SPRINT_ISSUES: usize = 50;

/// Add issues to a sprint.
async fn handle_add(
    sprint_id: u64,
    issues: Vec<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    client.add_issues_to_sprint(sprint_id, &issues).await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                output::render_json(&sprint_add_response(sprint_id, &issues))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Added {} issue(s) to sprint {}",
                issues.len(),
                sprint_id
            ));
        }
    }

    Ok(())
}

/// Remove issues from all sprints, moving them to the backlog.
async fn handle_remove(
    issues: Vec<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    client.move_issues_to_backlog(&issues).await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&sprint_remove_response(&issues))?);
        }
        OutputFormat::Table => {
            output::print_success(&format!("Moved {} issue(s) to backlog", issues.len()));
        }
    }

    Ok(())
}

async fn handle_list(
    board_id: u64,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    let sprints = client.list_sprints(board_id, None).await?;

    let rows: Vec<Vec<String>> = sprints
        .iter()
        .map(|s| {
            vec![
                s.id.to_string(),
                s.state.clone().unwrap_or_default(),
                s.name.clone(),
                s.end_date.clone().unwrap_or_else(|| "-".into()),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "State", "Name", "End Date"],
        &rows,
        &sprints,
    )?;

    Ok(())
}

/// Compute sprint points summary: (total_points, completed_points, unestimated_count)
pub fn compute_sprint_summary(issues: &[Issue], field_id: &str) -> (f64, f64, u32) {
    let mut total_points: f64 = 0.0;
    let mut completed_points: f64 = 0.0;
    let mut unestimated_count: u32 = 0;

    for issue in issues {
        match issue.fields.story_points(field_id) {
            Some(pts) => {
                total_points += pts;
                let is_done = issue
                    .fields
                    .status
                    .as_ref()
                    .and_then(|s| s.status_category.as_ref())
                    .map(|c| c.key == "done")
                    .unwrap_or(false);
                if is_done {
                    completed_points += pts;
                }
            }
            None => {
                unestimated_count += 1;
            }
        }
    }

    (total_points, completed_points, unestimated_count)
}

async fn handle_current(
    board_id: u64,
    client: &JiraClient,
    output_format: &OutputFormat,
    config: &Config,
    limit: Option<u32>,
    all: bool,
) -> Result<()> {
    let effective_limit = crate::cli::resolve_effective_limit(limit, all);
    let sprints = client.list_sprints(board_id, Some("active")).await?;

    if sprints.is_empty() {
        bail!("No active sprint found for board {}.", board_id);
    }

    let sprint = &sprints[0];
    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let team_field_id = config.global.fields.team_field_id.as_deref();
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
    // Request the team field so handle_current can surface a Team column
    // matching `jr issue list` — per #246 parity follow-up to #191.
    if let Some(t) = team_field_id {
        extra.push(t);
    }
    let result = client
        .get_sprint_issues(sprint.id, None, effective_limit, &extra)
        .await?;
    let issues = result.issues;
    let has_more = result.has_more;
    let issue_count = issues.len();

    let sprint_summary = sp_field_id.map(|field_id| compute_sprint_summary(&issues, field_id));

    match output_format {
        OutputFormat::Json => {
            let mut data = serde_json::json!({
                "sprint": sprint,
                "issues": issues,
            });
            if let Some((total, completed, unestimated)) = sprint_summary {
                data["sprint_summary"] = serde_json::json!({
                    "completed_points": completed,
                    "total_points": total,
                    "unestimated_count": unestimated,
                });
            }
            println!("{}", output::render_json(&data)?);
        }
        OutputFormat::Table => {
            eprintln!(
                "Sprint: {} (ends {})",
                sprint.name,
                sprint.end_date.as_deref().unwrap_or("N/A")
            );

            if let Some((total, completed, unestimated)) = sprint_summary {
                let mut summary_line = format!(
                    "Points: {}/{} completed",
                    super::issue::format_points(completed),
                    super::issue::format_points(total),
                );
                if unestimated > 0 {
                    summary_line.push_str(&format!("  ({} unestimated)", unestimated));
                }
                eprintln!("{}", summary_line);
            }

            eprintln!();

            // Team column gating mirrors handle_list in src/cli/issue/list.rs
            // (per #246): show only when team_field_id is configured AND at
            // least one issue has a populated team. Build UUID→name map once
            // so per-row resolution is O(1).
            let client_verbose = client.verbose();
            let team_displays: Vec<String> = if let Some(field_id) = team_field_id {
                let uuids: Vec<Option<String>> = issues
                    .iter()
                    .map(|i| i.fields.team_id(field_id, client_verbose))
                    .collect();
                if uuids.iter().any(|u| u.is_some()) {
                    let team_map: std::collections::HashMap<String, String> =
                        crate::cache::read_team_cache(&config.active_profile_name)
                            .ok()
                            .flatten()
                            .map(|c| c.teams.into_iter().map(|t| (t.id, t.name)).collect())
                            .unwrap_or_default();
                    uuids
                        .iter()
                        .map(|u| match u {
                            Some(uuid) => {
                                team_map.get(uuid).cloned().unwrap_or_else(|| uuid.clone())
                            }
                            None => "-".to_string(),
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };
            let show_team_col = !team_displays.is_empty();

            let rows: Vec<Vec<String>> = issues
                .iter()
                .enumerate()
                .map(|(i, issue)| {
                    let team = if show_team_col {
                        Some(team_displays[i].as_str())
                    } else {
                        None
                    };
                    super::issue::format_issue_row(issue, sp_field_id, None, team)
                })
                .collect();
            output::print_output(
                output_format,
                &super::issue::issue_table_headers(sp_field_id.is_some(), false, show_team_col),
                &rows,
                &issues,
            )?;
        }
    }

    if has_more && !all {
        eprintln!(
            "Showing {} results. Use --limit or --all to see more.",
            issue_count
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::jira::{IssueFields, Status, StatusCategory};
    use std::collections::HashMap;

    fn make_issue(key: &str, status_cat_key: &str, points: Option<f64>) -> Issue {
        let mut extra = HashMap::new();
        if let Some(pts) = points {
            extra.insert("customfield_10031".to_string(), serde_json::json!(pts));
        }
        Issue {
            key: key.to_string(),
            fields: IssueFields {
                summary: "test".to_string(),
                status: Some(Status {
                    name: "status".to_string(),
                    status_category: Some(StatusCategory {
                        name: "cat".to_string(),
                        key: status_cat_key.to_string(),
                    }),
                }),
                extra,
                ..Default::default()
            },
        }
    }

    #[test]
    fn sprint_summary_mixed_issues() {
        let issues = vec![
            make_issue("FOO-1", "done", Some(5.0)),
            make_issue("FOO-2", "indeterminate", Some(3.0)),
            make_issue("FOO-3", "new", None),
        ];
        let (total, completed, unestimated) = compute_sprint_summary(&issues, "customfield_10031");
        assert_eq!(total, 8.0);
        assert_eq!(completed, 5.0);
        assert_eq!(unestimated, 1);
    }

    #[test]
    fn sprint_summary_all_done() {
        let issues = vec![
            make_issue("FOO-1", "done", Some(5.0)),
            make_issue("FOO-2", "done", Some(3.0)),
        ];
        let (total, completed, unestimated) = compute_sprint_summary(&issues, "customfield_10031");
        assert_eq!(total, 8.0);
        assert_eq!(completed, 8.0);
        assert_eq!(unestimated, 0);
    }

    #[test]
    fn sprint_summary_no_points() {
        let issues = vec![
            make_issue("FOO-1", "new", None),
            make_issue("FOO-2", "new", None),
        ];
        let (total, completed, unestimated) = compute_sprint_summary(&issues, "customfield_10031");
        assert_eq!(total, 0.0);
        assert_eq!(completed, 0.0);
        assert_eq!(unestimated, 2);
    }

    #[test]
    fn sprint_summary_empty() {
        let (total, completed, unestimated) = compute_sprint_summary(&[], "customfield_10031");
        assert_eq!(total, 0.0);
        assert_eq!(completed, 0.0);
        assert_eq!(unestimated, 0);
    }

    #[test]
    fn test_sprint_add_response() {
        insta::assert_json_snapshot!(sprint_add_response(
            100,
            &["TEST-1".to_string(), "TEST-2".to_string()]
        ));
    }

    #[test]
    fn test_sprint_remove_response() {
        insta::assert_json_snapshot!(sprint_remove_response(&[
            "TEST-1".to_string(),
            "TEST-2".to_string()
        ]));
    }
}
