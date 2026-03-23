use anyhow::{Result, bail};

use crate::api::client::JiraClient;
use crate::cli::{BoardCommand, OutputFormat};
use crate::config::Config;
use crate::output;

/// Handle all board subcommands.
pub async fn handle(
    command: BoardCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    match command {
        BoardCommand::List => handle_list(client, output_format).await,
        BoardCommand::View => handle_view(config, client, output_format).await,
    }
}

async fn handle_list(client: &JiraClient, output_format: &OutputFormat) -> Result<()> {
    let boards = client.list_boards().await?;

    let rows: Vec<Vec<String>> = boards
        .iter()
        .map(|b| vec![b.id.to_string(), b.board_type.clone(), b.name.clone()])
        .collect();

    output::print_output(output_format, &["ID", "Type", "Name"], &rows, &boards)?;

    Ok(())
}

async fn handle_view(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    let board_id = config.project.board_id.ok_or_else(|| {
        anyhow::anyhow!("No board_id configured. Set board_id in .jr.toml or run \"jr init\".")
    })?;

    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();

    let issues = if board_type == "scrum" {
        // For scrum boards, fetch the active sprint's issues
        let sprints = client.list_sprints(board_id, Some("active")).await?;
        if sprints.is_empty() {
            bail!("No active sprint found for board {}.", board_id);
        }
        let sprint = &sprints[0];
        client.get_sprint_issues(sprint.id, None, &[]).await?
    } else {
        // Kanban: search for issues not in Done status category
        let project_key = config.project_key(None);
        if project_key.is_none() {
            eprintln!(
                "warning: no project configured for board. Showing issues across all projects. Set project in .jr.toml to scope results."
            );
        }
        let mut jql_parts: Vec<String> = Vec::new();
        if let Some(ref pk) = project_key {
            jql_parts.push(format!("project = \"{}\"", pk));
        }
        jql_parts.push("statusCategory != Done".into());
        jql_parts.push("ORDER BY rank ASC".into());
        let jql = jql_parts.join(" AND ");
        client.search_issues(&jql, None, &[]).await?
    };

    let rows = super::issue::format_issue_rows_public(&issues);

    output::print_output(
        output_format,
        &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
        &rows,
        &issues,
    )?;

    Ok(())
}
