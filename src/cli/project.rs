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

/// Suggest valid projects when an invalid key is used.
///
/// Returns a hint string like `Did you mean "FOO"? Run "jr project list" to see available projects.`
/// If no close match is found or the API call fails, returns a generic hint.
pub async fn suggest_projects(client: &JiraClient, invalid_key: &str) -> String {
    let generic = "Run \"jr project list\" to see available projects.".to_string();

    let projects = match client.list_projects(None, Some(50)).await {
        Ok(p) => p,
        Err(_) => return generic,
    };

    let keys: Vec<String> = projects.iter().map(|p| p.key.clone()).collect();
    match crate::partial_match::partial_match(invalid_key, &keys) {
        crate::partial_match::MatchResult::Exact(matched) => {
            format!("Did you mean \"{matched}\"? {generic}")
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            let quoted: Vec<String> = matches.iter().map(|m| format!("\"{m}\"")).collect();
            format!("Did you mean {}? {generic}", quoted.join(" or "))
        }
        crate::partial_match::MatchResult::None(_) => generic,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn suggest_projects_match_logic_exact() {
        let keys = vec!["FOO".to_string(), "BAR".to_string(), "BAZ".to_string()];
        match crate::partial_match::partial_match("FOO", &keys) {
            crate::partial_match::MatchResult::Exact(m) => assert_eq!(m, "FOO"),
            _ => panic!("Expected exact match"),
        }
    }

    #[test]
    fn suggest_projects_match_logic_partial() {
        let keys = vec!["FOO".to_string(), "BAR".to_string(), "BAZ".to_string()];
        match crate::partial_match::partial_match("FO", &keys) {
            crate::partial_match::MatchResult::Exact(m) => assert_eq!(m, "FOO"),
            _ => panic!("Expected unique partial match"),
        }
    }

    #[test]
    fn suggest_projects_match_logic_ambiguous() {
        let keys = vec!["FOO".to_string(), "BAR".to_string(), "BAZ".to_string()];
        match crate::partial_match::partial_match("BA", &keys) {
            crate::partial_match::MatchResult::Ambiguous(matches) => {
                assert_eq!(matches.len(), 2);
                assert!(matches.contains(&"BAR".to_string()));
                assert!(matches.contains(&"BAZ".to_string()));
            }
            _ => panic!("Expected ambiguous match"),
        }
    }

    #[test]
    fn suggest_projects_match_logic_none() {
        let keys = vec!["FOO".to_string(), "BAR".to_string()];
        match crate::partial_match::partial_match("ZZZ", &keys) {
            crate::partial_match::MatchResult::None(_) => {}
            _ => panic!("Expected no match"),
        }
    }
}
