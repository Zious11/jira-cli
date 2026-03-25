use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, ProjectCommand};
use crate::config::Config;
use crate::output;

pub async fn handle(
    command: ProjectCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    match command {
        ProjectCommand::List {
            project_type,
            limit,
            all,
        } => handle_list(client, output_format, project_type.as_deref(), limit, all).await,
        ProjectCommand::Fields { project } => {
            handle_fields(project, config, client, output_format, project_override).await
        }
    }
}

async fn handle_list(
    client: &JiraClient,
    output_format: &OutputFormat,
    project_type: Option<&str>,
    limit: Option<u32>,
    all: bool,
) -> Result<()> {
    let max_results = if all { None } else { Some(limit.unwrap_or(50)) };
    let projects = client.list_projects(project_type, max_results).await?;

    let rows: Vec<Vec<String>> = projects
        .iter()
        .map(|p| {
            vec![
                p.key.clone(),
                p.name.clone(),
                p.lead
                    .as_ref()
                    .map(|l| l.display_name.clone())
                    .unwrap_or_else(|| "\u{2014}".into()),
                p.project_type_key.clone(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["Key", "Name", "Lead", "Type"],
        &rows,
        &projects,
    )
}

async fn handle_fields(
    project: Option<String>,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No project specified. Run \"jr project list\" to see available projects."
            )
        })?;

    let issue_types = client.get_project_issue_types(&project_key).await?;
    let priorities = client.get_priorities().await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "project": project_key,
                    "issue_types": issue_types,
                    "priorities": priorities,
                })
            );
        }
        OutputFormat::Table => {
            println!("Project: {project_key}\n");
            println!("Issue Types:");
            for t in &issue_types {
                let suffix = if t.subtask == Some(true) {
                    " (subtask)"
                } else {
                    ""
                };
                println!("  - {}{}", t.name, suffix);
            }
            println!("\nPriorities:");
            for p in &priorities {
                println!("  - {}", p.name);
            }
        }
    }
    Ok(())
}
