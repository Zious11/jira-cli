use anyhow::{Context, Result};

use crate::api::client::JiraClient;
use crate::cache::{self, CachedTeam};
use crate::cli::OutputFormat;
use crate::config::Config;
use crate::error::JrError;
use crate::output;

use super::TeamCommand;

pub async fn handle(
    command: TeamCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    match command {
        TeamCommand::List { refresh } => handle_list(refresh, output_format, config, client).await,
    }
}

async fn handle_list(
    refresh: bool,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let teams = if refresh {
        fetch_and_cache_teams(config, client).await?
    } else {
        match cache::read_team_cache()? {
            Some(cached) => cached.teams,
            None => fetch_and_cache_teams(config, client).await?,
        }
    };

    if teams.is_empty() {
        eprintln!("No teams found.");
        return Ok(());
    }

    let rows: Vec<Vec<String>> = teams
        .iter()
        .map(|t| vec![t.name.clone(), t.id.clone()])
        .collect();

    output::print_output(output_format, &["Name", "ID"], &rows, &teams)?;
    Ok(())
}

/// Fetch teams from the API and write them to the cache.
/// Resolves org_id lazily if not in config.
pub async fn fetch_and_cache_teams(
    config: &Config,
    client: &JiraClient,
) -> Result<Vec<CachedTeam>> {
    let org_id = resolve_org_id(config, client).await?;

    let api_teams = client
        .list_teams(&org_id)
        .await
        .context("Failed to fetch teams from API")?;

    let cached: Vec<CachedTeam> = api_teams
        .into_iter()
        .map(|t| CachedTeam {
            id: t.team_id,
            name: t.display_name,
        })
        .collect();

    cache::write_team_cache(&cached)?;
    Ok(cached)
}

/// Resolve org_id: read from config, or discover via GraphQL and persist.
/// Uses hostNames-based GraphQL query to get both cloudId and orgId in one call.
pub async fn resolve_org_id(config: &Config, client: &JiraClient) -> Result<String> {
    if let Some(ref org_id) = config.global.instance.org_id {
        return Ok(org_id.clone());
    }

    // Extract hostname from instance URL
    let url = config.global.instance.url.as_ref().ok_or_else(|| {
        JrError::ConfigError("No Jira instance configured. Run \"jr init\" first.".into())
    })?;
    let hostname = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    // Single GraphQL call returns both cloudId and orgId
    let metadata = client.get_org_metadata(hostname).await?;

    // Persist discovered values to config for future use
    let mut updated_config = Config::load()?;
    updated_config.global.instance.cloud_id = Some(metadata.cloud_id);
    updated_config.global.instance.org_id = Some(metadata.org_id.clone());
    updated_config.save_global()?;

    Ok(metadata.org_id)
}
