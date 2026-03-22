use anyhow::{bail, Result};

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
) -> Result<()> {
    let board_id = config.project.board_id.ok_or_else(|| {
        anyhow::anyhow!("No board_id configured. Set board_id in .jr.toml or run \"jr init\".")
    })?;

    // Guard: sprints only make sense for scrum boards
    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();
    if board_type != "scrum" {
        bail!(
            "Sprint commands are only available for scrum boards. Board {} is a {} board.",
            board_id,
            board_config.board_type
        );
    }

    match command {
        SprintCommand::List => handle_list(board_id, client, output_format).await,
        SprintCommand::Current => handle_current(board_id, client, output_format, config).await,
    }
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
) -> Result<()> {
    let sprints = client.list_sprints(board_id, Some("active")).await?;

    if sprints.is_empty() {
        bail!("No active sprint found for board {}.", board_id);
    }

    let sprint = &sprints[0];
    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let extra: Vec<&str> = sp_field_id.iter().copied().collect();
    let issues = client.get_sprint_issues(sprint.id, None, &extra).await?;

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

            if let Some(field_id) = sp_field_id {
                let rows: Vec<Vec<String>> = issues
                    .iter()
                    .map(|issue| {
                        let pts = issue
                            .fields
                            .story_points(field_id)
                            .map(super::issue::format_points)
                            .unwrap_or_else(|| "-".into());
                        vec![
                            issue.key.clone(),
                            issue.fields.issue_type.as_ref().map(|t| t.name.clone()).unwrap_or_default(),
                            issue.fields.status.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
                            issue.fields.priority.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                            pts,
                            issue.fields.assignee.as_ref().map(|a| a.display_name.clone()).unwrap_or_else(|| "Unassigned".into()),
                            issue.fields.summary.clone(),
                        ]
                    })
                    .collect();

                output::print_output(
                    output_format,
                    &["Key", "Type", "Status", "Priority", "Points", "Assignee", "Summary"],
                    &rows,
                    &issues,
                )?;
            } else {
                let rows = super::issue::format_issue_rows_public(&issues);
                output::print_output(
                    output_format,
                    &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
                    &rows,
                    &issues,
                )?;
            }
        }
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
}
