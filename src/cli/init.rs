use anyhow::Result;
use dialoguer::{Confirm, Input, Select};

use crate::config::{
    Config, DefaultsConfig, FieldsConfig, GlobalConfig, InstanceConfig, ProjectConfig,
};
use crate::{api, output};

pub async fn handle() -> Result<()> {
    println!("Setting up jr — Jira CLI\n");

    // Step 1: Instance URL
    let url: String = Input::new()
        .with_prompt("Jira instance URL (e.g., https://yourorg.atlassian.net)")
        .interact_text()?;
    let url = url.trim_end_matches('/').to_string();

    // Step 2: Auth method
    let auth_methods = vec!["OAuth 2.0 (recommended)", "API Token"];
    let auth_choice = Select::new()
        .with_prompt("Authentication method")
        .items(&auth_methods)
        .default(0)
        .interact()?;

    let global = GlobalConfig {
        instance: InstanceConfig {
            url: Some(url.clone()),
            cloud_id: None,
            org_id: None,
            auth_method: None,
        },
        defaults: DefaultsConfig::default(),
        fields: FieldsConfig::default(),
    };

    // Save initial config so auth can use it
    let config = Config {
        global,
        project: ProjectConfig::default(),
    };
    config.save_global()?;

    // Step 3: Authenticate
    if auth_choice == 0 {
        crate::cli::auth::login_oauth().await?;
    } else {
        crate::cli::auth::login_token().await?;
        let mut config = Config::load()?;
        config.global.instance.auth_method = Some("api_token".into());
        config.save_global()?;
    }

    // Step 4: Per-project setup
    let config = Config::load()?;
    let client = api::client::JiraClient::from_config(&config, false)?;

    let setup_project = Confirm::new()
        .with_prompt("Configure this directory as a Jira project?")
        .default(true)
        .interact()?;

    if setup_project {
        let boards = client.list_boards().await?;
        if boards.is_empty() {
            println!("No boards found. You can configure .jr.toml manually.");
        } else {
            let board_names: Vec<String> = boards
                .iter()
                .map(|b| format!("{} ({}, {})", b.name, b.board_type, b.id))
                .collect();
            let board_choice = Select::new()
                .with_prompt("Select board")
                .items(&board_names)
                .interact()?;
            let selected_board = &boards[board_choice];

            let project_key: String = Input::new().with_prompt("Project key").interact_text()?;

            let project_config = format!(
                "project = \"{}\"\nboard_id = {}\n",
                project_key, selected_board.id,
            );
            std::fs::write(".jr.toml", &project_config)?;
            output::print_success("Created .jr.toml");
        }
    }

    // Step 5: Discover team field
    if let Ok(Some(team_id)) = client.find_team_field_id().await {
        let mut config = Config::load()?;
        config.global.fields.team_field_id = Some(team_id);
        config.save_global()?;
    }

    // Step 5b: Discover story points field
    match client.find_story_points_field_id().await {
        Ok(matches) => {
            let field_id = match matches.len() {
                0 => {
                    eprintln!("No story points field found — skipping. You can set story_points_field_id manually in config.");
                    None
                }
                1 => {
                    eprintln!("Found story points field: {} ({})", matches[0].1, matches[0].0);
                    Some(matches[0].0.clone())
                }
                _ => {
                    let names: Vec<String> = matches
                        .iter()
                        .map(|(id, name)| format!("{} ({})", name, id))
                        .collect();
                    let selection = Select::new()
                        .with_prompt("Multiple story points fields found. Select one")
                        .items(&names)
                        .interact()?;
                    Some(matches[selection].0.clone())
                }
            };

            if let Some(id) = field_id {
                let mut config = Config::load()?;
                config.global.fields.story_points_field_id = Some(id);
                config.save_global()?;
            }
        }
        Err(e) => {
            eprintln!("Could not discover story points field: {}. Skipping.", e);
        }
    }

    // Step 6: Prefetch cloud_id and org_id via GraphQL (single call)
    let hostname = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    if let Ok(metadata) = client.get_org_metadata(hostname).await {
        let mut config = Config::load()?;
        config.global.instance.cloud_id = Some(metadata.cloud_id);
        config.global.instance.org_id = Some(metadata.org_id.clone());
        config.save_global()?;

        // Step 7: Prefetch team list into cache
        if let Ok(api_teams) = client.list_teams(&metadata.org_id).await {
            let cached: Vec<crate::cache::CachedTeam> = api_teams
                .into_iter()
                .map(|t| crate::cache::CachedTeam {
                    id: t.team_id,
                    name: t.display_name,
                })
                .collect();
            let _ = crate::cache::write_team_cache(&cached);
        }
    }

    output::print_success("\njr is ready! Try \"jr issue list\" to see your tickets.");
    Ok(())
}
