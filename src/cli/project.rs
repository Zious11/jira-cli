use anyhow::Result;

use crate::api::assets::linked::get_or_fetch_cmdb_fields;
use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, ProjectCommand};
use crate::config::Config;
use crate::error::JrError;
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
        ProjectCommand::Fields => {
            handle_fields(config, client, output_format, project_override).await
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
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    let project_key = config.project_key(project_override).ok_or_else(|| {
        JrError::UserError(
            "No project specified. Use --project <KEY> or configure a default project in .jr.toml. \
             Run \"jr project list\" to see available projects."
                .into(),
        )
    })?;

    let issue_types = client.get_project_issue_types(&project_key).await?;
    let priorities = client.get_priorities().await?;
    let statuses = client.get_project_statuses(&project_key).await?;
    let cmdb_fields = get_or_fetch_cmdb_fields(client).await.unwrap_or_default();

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "project": project_key,
                    "issue_types": issue_types,
                    "priorities": priorities,
                    "statuses_by_issue_type": statuses,
                    "asset_fields": cmdb_fields.iter().map(|(id, name)| {
                        serde_json::json!({"id": id, "name": name})
                    }).collect::<Vec<_>>(),
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
            let has_statuses = statuses.iter().any(|it| !it.statuses.is_empty());
            if has_statuses {
                println!("\nStatuses by Issue Type:");
                for it in &statuses {
                    if it.statuses.is_empty() {
                        continue;
                    }
                    println!("  {}:", it.name);
                    for s in &it.statuses {
                        println!("    - {}", s.name);
                    }
                }
            }
            if !cmdb_fields.is_empty() {
                println!("\nCustom Fields (Assets) \u{2014} instance-wide:");
                for (id, name) in &cmdb_fields {
                    println!("  - {} ({})", name, id);
                }
            }
        }
    }
    Ok(())
}
