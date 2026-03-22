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

    output::print_success("\njr is ready! Try \"jr issue list\" to see your tickets.");
    Ok(())
}
