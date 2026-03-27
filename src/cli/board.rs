use anyhow::{Result, bail};

use crate::api::client::JiraClient;
use crate::cli::{BoardCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
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
        BoardCommand::View { board } => handle_view(config, client, output_format, board).await,
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

/// Build JQL for kanban board view: all non-Done issues, ordered by rank.
fn build_kanban_jql(project_key: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(pk) = project_key {
        parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
    }
    parts.push("statusCategory != Done".into());
    let where_clause = parts.join(" AND ");
    format!("{where_clause} ORDER BY rank ASC")
}

async fn handle_view(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    board_override: Option<u64>,
) -> Result<()> {
    let board_id = config.board_id(board_override).ok_or_else(|| {
        JrError::ConfigError(
            "No board configured. Use --board <ID> or set board_id in .jr.toml.\n\
             Run \"jr board list\" to see available boards."
                .into(),
        )
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
        client
            .get_sprint_issues(sprint.id, None, None, &[])
            .await?
            .issues
    } else {
        // Kanban: search for issues not in Done status category
        let project_key = config.project_key(None);
        if project_key.is_none() {
            eprintln!(
                "warning: no project configured for board. Showing issues across all projects. Set project in .jr.toml to scope results."
            );
        }
        let jql = build_kanban_jql(project_key.as_deref());
        client.search_issues(&jql, None, &[]).await?.issues
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_kanban_jql_with_project() {
        let jql = build_kanban_jql(Some("FOO"));
        assert_eq!(
            jql,
            "project = \"FOO\" AND statusCategory != Done ORDER BY rank ASC"
        );
    }

    #[test]
    fn build_kanban_jql_without_project() {
        let jql = build_kanban_jql(None);
        assert_eq!(jql, "statusCategory != Done ORDER BY rank ASC");
    }

    #[test]
    fn build_kanban_jql_escapes_special_characters() {
        let jql = build_kanban_jql(Some("FOO\"BAR"));
        assert_eq!(
            jql,
            "project = \"FOO\\\"BAR\" AND statusCategory != Done ORDER BY rank ASC"
        );
    }

    #[test]
    fn missing_board_id_returns_config_error() {
        let result: Option<u64> = None;
        let err = result
            .ok_or_else(|| {
                crate::error::JrError::ConfigError(
                    "No board configured. Use --board <ID> or set board_id in .jr.toml.\n\
                     Run \"jr board list\" to see available boards."
                        .into(),
                )
            })
            .unwrap_err();
        assert_eq!(err.exit_code(), 78);
        assert!(err.to_string().contains("No board configured"));
    }
}
