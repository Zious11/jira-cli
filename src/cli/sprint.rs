use anyhow::{bail, Result};

use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, SprintCommand};
use crate::config::Config;
use crate::output;

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
        SprintCommand::Current => handle_current(board_id, client, output_format).await,
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

async fn handle_current(
    board_id: u64,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    let sprints = client.list_sprints(board_id, Some("active")).await?;

    if sprints.is_empty() {
        bail!("No active sprint found for board {}.", board_id);
    }

    let sprint = &sprints[0];
    let issues = client.get_sprint_issues(sprint.id, None).await?;

    match output_format {
        OutputFormat::Json => {
            let data = serde_json::json!({
                "sprint": sprint,
                "issues": issues,
            });
            println!("{}", output::render_json(&data)?);
        }
        OutputFormat::Table => {
            eprintln!(
                "Sprint: {} (ends {})",
                sprint.name,
                sprint.end_date.as_deref().unwrap_or("N/A")
            );
            eprintln!();

            let rows = super::issue::format_issue_rows_public(&issues);
            output::print_output(
                output_format,
                &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
                &rows,
                &issues,
            )?;
        }
    }

    Ok(())
}
