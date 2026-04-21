use anyhow::{Result, bail};

use crate::api::client::JiraClient;
use crate::cli::{BoardCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;

/// Resolve a board ID from CLI override, config, or auto-discovery.
///
/// Resolution order:
/// 1. CLI `--board` override
/// 2. Config `board_id` from `.jr.toml`
/// 3. Auto-discover via Jira API using project key
pub async fn resolve_board_id(
    config: &Config,
    client: &JiraClient,
    board_override: Option<u64>,
    project_override: Option<&str>,
    require_scrum: bool,
) -> Result<u64> {
    // Step 1: CLI override
    if let Some(id) = board_override {
        return Ok(id);
    }

    // Step 2: Config
    if let Some(id) = config.project.board_id {
        return Ok(id);
    }

    // Step 3: Auto-discover
    let project_key = config.project_key(project_override).ok_or_else(|| {
        JrError::ConfigError(
            "No board configured and no project specified. \
             Use --board <ID>, set board_id in .jr.toml, or specify --project to auto-discover."
                .into(),
        )
    })?;

    let type_filter = if require_scrum { Some("scrum") } else { None };
    let boards = client.list_boards(Some(&project_key), type_filter).await?;

    match boards.len() {
        0 => {
            let board_kind = if require_scrum {
                "scrum boards"
            } else {
                "boards"
            };
            bail!(
                "No {} found for project {}. \
                 The project key may be incorrect, or the project may not have any {}. \
                 Run \"jr board list --project {}\" to inspect available boards.",
                board_kind,
                project_key,
                board_kind,
                project_key,
            );
        }
        1 => {
            let board = &boards[0];
            eprintln!(
                "Using board {} - {} ({})",
                board.id, board.name, board.board_type
            );
            Ok(board.id)
        }
        _ => {
            let board_kind = if require_scrum {
                "scrum boards"
            } else {
                "boards"
            };
            let mut msg = format!(
                "Multiple {} found for project {}:\n",
                board_kind, project_key
            );
            for b in &boards {
                if require_scrum {
                    msg.push_str(&format!("  {}  {}\n", b.id, b.name));
                } else {
                    msg.push_str(&format!("  {}  {}  {}\n", b.id, b.board_type, b.name));
                }
            }
            msg.push_str("Use --board <ID> to select one, or set board_id in .jr.toml.");
            bail!("{}", msg);
        }
    }
}

/// Handle all board subcommands.
pub async fn handle(
    command: BoardCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    match command {
        BoardCommand::List { board_type } => {
            handle_list(
                client,
                output_format,
                project_override,
                board_type.as_deref(),
            )
            .await
        }
        BoardCommand::View { board, limit, all } => {
            handle_view(
                config,
                client,
                output_format,
                board,
                limit,
                all,
                project_override,
            )
            .await
        }
    }
}

async fn handle_list(
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
    board_type_filter: Option<&str>,
) -> Result<()> {
    let boards = client
        .list_boards(project_override, board_type_filter)
        .await?;

    let rows: Vec<Vec<String>> = boards
        .iter()
        .map(|b| {
            let project = b
                .location
                .as_ref()
                .and_then(|loc| loc.project_key.as_deref())
                .unwrap_or("-");
            vec![
                b.id.to_string(),
                b.board_type.clone(),
                project.to_string(),
                b.name.clone(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "Type", "Project", "Name"],
        &rows,
        &boards,
    )?;

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
    limit: Option<u32>,
    all: bool,
    project_override: Option<&str>,
) -> Result<()> {
    let effective_limit = crate::cli::resolve_effective_limit(limit, all);

    let board_id =
        resolve_board_id(config, client, board_override, project_override, false).await?;

    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();

    // Request the team field alongside issues so handle_view can surface a
    // Team column matching `jr issue list` — per #246 parity follow-up.
    let team_field_id = config.global.fields.team_field_id.as_deref();
    let extra: Vec<&str> = team_field_id.iter().copied().collect();

    let (issues, has_more) = if board_type == "scrum" {
        // For scrum boards, fetch the active sprint's issues
        let sprints = client.list_sprints(board_id, Some("active")).await?;
        if sprints.is_empty() {
            bail!("No active sprint found for board {}.", board_id);
        }
        let sprint = &sprints[0];
        let result = client
            .get_sprint_issues(sprint.id, None, effective_limit, &extra)
            .await?;
        (result.issues, result.has_more)
    } else {
        let project_key = config.project_key(project_override);
        if project_key.is_none() {
            eprintln!(
                "warning: no project configured for board. Showing issues across all projects. Set project in .jr.toml to scope results."
            );
        }
        let jql = build_kanban_jql(project_key.as_deref());
        let result = client.search_issues(&jql, effective_limit, &extra).await?;
        (result.issues, result.has_more)
    };

    // Team column gating mirrors handle_list in src/cli/issue/list.rs
    // (per #246): show only when team_field_id is configured AND at least
    // one issue has a populated team.
    let client_verbose = client.verbose();
    let team_displays: Vec<String> = if let Some(field_id) = team_field_id {
        let uuids: Vec<Option<String>> = issues
            .iter()
            .map(|i| i.fields.team_id(field_id, client_verbose))
            .collect();
        if uuids.iter().any(|u| u.is_some()) {
            let team_map: std::collections::HashMap<String, String> =
                crate::cache::read_team_cache()
                    .ok()
                    .flatten()
                    .map(|c| c.teams.into_iter().map(|t| (t.id, t.name)).collect())
                    .unwrap_or_default();
            uuids
                .iter()
                .map(|u| match u {
                    Some(uuid) => team_map.get(uuid).cloned().unwrap_or_else(|| uuid.clone()),
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
            super::issue::format_issue_row(issue, None, None, team)
        })
        .collect();
    let headers = super::issue::issue_table_headers(false, false, show_team_col);
    output::print_output(output_format, &headers, &rows, &issues)?;

    if has_more && !all {
        if board_type != "scrum" {
            // Kanban: try to get approximate total via JQL count
            let project_key = config.project_key(project_override);
            let jql = build_kanban_jql(project_key.as_deref());
            let count_jql = crate::jql::strip_order_by(&jql);
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
        } else {
            // Scrum: no reliable total count from Agile API
            eprintln!(
                "Showing {} results. Use --limit or --all to see more.",
                issues.len()
            );
        }
    }

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
}
