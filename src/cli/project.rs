use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, ProjectCommand};
use crate::config::Config;

pub async fn handle(
    command: ProjectCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    match command {
        ProjectCommand::Fields { project } => {
            let project_key = project
                .or_else(|| config.project_key(project_override))
                .ok_or_else(|| anyhow::anyhow!("No project specified"))?;

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
    }
}
