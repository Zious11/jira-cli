use anyhow::{Context, Result};
use dialoguer::{Confirm, Input, Select};

use crate::config::Config;
use crate::{api, output};

pub async fn handle() -> Result<()> {
    eprintln!("Setting up jr — Jira CLI\n");

    // Multi-profile awareness: if profiles already exist, ask whether to add
    // another one rather than overwriting the existing setup. When the user
    // opts in, JR_PROFILE_OVERRIDE is set so the rest of init scopes its
    // writes to the new profile.
    //
    // Distinguish "no config yet" (legitimate first-run) from "config exists
    // but won't load" (malformed TOML, permission-denied, etc.). The latter
    // is a real problem: silently dropping to defaults would let `jr init`
    // overwrite the user's broken-but-recoverable file. Only swallow the
    // error when the config file genuinely doesn't exist.
    let existing = match crate::config::Config::load() {
        Ok(c) => Some(c),
        Err(e) => {
            let path = crate::config::global_config_path();
            if path.exists() {
                return Err(e.context(format!(
                    "failed to load existing config at {}; fix or remove it before running 'jr init'",
                    path.display()
                )));
            }
            None
        }
    };
    if let Some(c) = existing.as_ref() {
        if !c.global.profiles.is_empty() {
            let names: Vec<String> = c.global.profiles.keys().cloned().collect();
            eprintln!("Profiles already configured: {}", names.join(", "));
            let add = Confirm::new()
                .with_prompt("Add another profile?")
                .default(false)
                .interact()
                .context("failed to prompt for additional profile")?;
            if !add {
                return Ok(());
            }
            let profile_name: String = Input::new()
                .with_prompt("Name for the new profile")
                .interact_text()
                .context("failed to read profile name")?;
            crate::config::validate_profile_name(&profile_name)?;
            // SAFETY: jr init's flow is fully serial — each prompt awaits
            // user input before the next step proceeds, and no spawned task
            // reads or writes env vars between this call and Config::load
            // below. If any future background work is added to init, this
            // needs reassessment.
            unsafe {
                std::env::set_var("JR_PROFILE_OVERRIDE", &profile_name);
            }
        }
    }

    // Step 1: Instance URL
    let url: String = Input::new()
        .with_prompt("Jira instance URL (e.g., https://yourorg.atlassian.net)")
        .interact_text()
        .context("failed to read Jira instance URL")?;
    let url = url.trim_end_matches('/').to_string();

    // Step 2: Auth method
    let auth_methods = vec!["OAuth 2.0 (recommended)", "API Token"];
    let auth_choice = Select::new()
        .with_prompt("Authentication method")
        .items(&auth_methods)
        .default(0)
        .interact()
        .context("failed to prompt for authentication method")?;

    // Determine which profile this init flow targets. The override is set
    // earlier when the user opted to add a new profile alongside an existing
    // one; otherwise we fall back to the literal "default".
    let profile_name = std::env::var("JR_PROFILE_OVERRIDE").unwrap_or_else(|_| "default".into());

    // Load any existing config, then write the URL into the target profile
    // entry. The legacy `[instance]` block is `#[serde(skip_serializing)]`
    // since the multi-profile refactor, so writes there are silently dropped
    // on save — every persisted field must live under `[profiles.<name>]`.
    //
    // Reload here (rather than reusing the `existing` we discriminated
    // above) so JR_PROFILE_OVERRIDE — which the new-profile branch may have
    // just set — is reflected in `active_profile_name`.
    //
    // Lenient because the override may name a not-yet-created profile (the
    // whole point of running `jr init` is to add it). Without lenient, the
    // strict active-profile-existence check fires and the previous
    // `unwrap_or_else(default)` fallback would silently clobber existing
    // profiles on save — flagged by Copilot review on PR #275.
    //
    // The `?` (no fallback) is safe because line ~20 above already
    // discriminated "config file is malformed/unreadable" from "no config
    // yet"; the only reachable failure here would be a fresh IO error
    // between then and now, which we want to surface, not silently swallow.
    let mut config = Config::load_lenient()?;
    config
        .global
        .profiles
        .entry(profile_name.clone())
        .or_default()
        .url = Some(url.clone());
    config.save_global()?;

    // Step 3: Authenticate. `jr init` is inherently interactive (Select
    // prompts above), so pass no_input=false and let dialoguer handle each
    // credential prompt. Flags aren't plumbed through init — users who want
    // a non-interactive setup should run `jr auth login` directly.
    if auth_choice == 0 {
        crate::cli::auth::login_oauth(&profile_name, None, None, false).await?;
    } else {
        crate::cli::auth::login_token(&profile_name, None, None, false).await?;
        let mut config = Config::load()?;
        config
            .global
            .profiles
            .entry(profile_name.clone())
            .or_default()
            .auth_method = Some("api_token".into());
        config.save_global()?;
    }

    // Step 4: Per-project setup
    let config = Config::load()?;
    let client = api::client::JiraClient::from_config(&config, false)?;

    let setup_project = Confirm::new()
        .with_prompt("Configure this directory as a Jira project?")
        .default(true)
        .interact()
        .context("failed to prompt for project setup")?;

    if setup_project {
        let boards = client.list_boards(None, None).await?;
        if boards.is_empty() {
            eprintln!("No boards found. You can configure .jr.toml manually.");
        } else {
            let board_names: Vec<String> = boards
                .iter()
                .map(|b| format!("{} ({}, {})", b.name, b.board_type, b.id))
                .collect();
            let board_choice = Select::new()
                .with_prompt("Select board")
                .items(&board_names)
                .interact()
                .context("failed to prompt for board selection")?;
            let selected_board = &boards[board_choice];

            let project_key: String = Input::new()
                .with_prompt("Project key")
                .interact_text()
                .context("failed to read project key")?;

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
        let active = config.active_profile_name.clone();
        config
            .global
            .profiles
            .entry(active)
            .or_default()
            .team_field_id = Some(team_id);
        config.save_global()?;
    }

    // Step 5b: Discover story points field
    match client.find_story_points_field_id().await {
        Ok(matches) => {
            let field_id = match matches.len() {
                0 => {
                    eprintln!(
                        "No story points field found — skipping. You can set story_points_field_id manually in config."
                    );
                    None
                }
                1 => {
                    eprintln!(
                        "Found story points field: {} ({})",
                        matches[0].1, matches[0].0
                    );
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
                        .interact()
                        .context("failed to prompt for story points field selection")?;
                    Some(matches[selection].0.clone())
                }
            };

            if let Some(id) = field_id {
                let mut config = Config::load()?;
                let active = config.active_profile_name.clone();
                config
                    .global
                    .profiles
                    .entry(active)
                    .or_default()
                    .story_points_field_id = Some(id);
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
        let active = config.active_profile_name.clone();
        let entry = config.global.profiles.entry(active).or_default();
        entry.cloud_id = Some(metadata.cloud_id.clone());
        entry.org_id = Some(metadata.org_id.clone());
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
            if let Err(err) = crate::cache::write_team_cache(&config.active_profile_name, &cached) {
                eprintln!(
                    "warning: failed to warm team cache: {err}. First `jr team list` will refetch."
                );
            }
        }
    }

    output::print_success("\njr is ready! Try \"jr issue list\" to see your tickets.");
    Ok(())
}
