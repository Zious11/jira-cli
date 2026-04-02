use anyhow::Result;

use crate::api::client::JiraClient;
use crate::config::Config;
use crate::error::JrError;

pub(super) async fn resolve_team_field(
    config: &Config,
    client: &JiraClient,
    team_name: &str,
    no_input: bool,
) -> Result<(String, String)> {
    // 1. Resolve team_field_id
    let field_id = if let Some(id) = &config.global.fields.team_field_id {
        id.clone()
    } else {
        client
            .find_team_field_id()
            .await?
            .ok_or_else(|| {
                JrError::ConfigError(
                    "No \"Team\" field found on this Jira instance. This instance may not have the Team field configured.".into(),
                )
            })?
    };

    // 2. Load teams from cache (or fetch if missing/expired)
    let teams = match crate::cache::read_team_cache()? {
        Some(cached) => cached.teams,
        None => crate::cli::team::fetch_and_cache_teams(config, client).await?,
    };

    // 3. Partial match
    let team_names: Vec<String> = teams.iter().map(|t| t.name.clone()).collect();
    match crate::partial_match::partial_match(team_name, &team_names) {
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let team = teams
                .iter()
                .find(|t| t.name == matched_name)
                .expect("matched name must exist in teams");
            Ok((field_id, team.id.clone()))
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            if no_input {
                let quoted: Vec<String> = matches.iter().map(|m| format!("\"{}\"", m)).collect();
                anyhow::bail!(
                    "Multiple teams match \"{}\": {}. Use a more specific name.",
                    team_name,
                    quoted.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple teams match \"{team_name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let team = teams
                .iter()
                .find(|t| &t.name == selected_name)
                .expect("selected name must exist in teams");
            Ok((field_id, team.id.clone()))
        }
        crate::partial_match::MatchResult::None(_) => {
            anyhow::bail!(
                "No team matching \"{}\". Run \"jr team list --refresh\" to update.",
                team_name
            );
        }
    }
}

pub(super) fn resolve_story_points_field_id(config: &Config) -> Result<String> {
    Ok(config
        .global
        .fields
        .story_points_field_id
        .clone()
        .ok_or_else(|| {
            JrError::ConfigError(
                "Story points field not configured. Run \"jr init\" or set story_points_field_id under [fields] in ~/.config/jr/config.toml".into(),
            )
        })?)
}

pub(super) fn prompt_input(prompt: &str) -> Result<String> {
    let input: String = dialoguer::Input::new()
        .with_prompt(prompt)
        .interact_text()?;
    Ok(input)
}

/// Check if a user input string is the "me" keyword (case-insensitive).
fn is_me_keyword(input: &str) -> bool {
    input.eq_ignore_ascii_case("me")
}

/// Resolve a user flag value to a JQL fragment.
///
/// - `"me"` (case-insensitive) → `"currentUser()"` (no API call)
/// - Any other value → search users API, filter active, disambiguate via partial_match
///
/// Returns the JQL value to use (either `"currentUser()"` or an unquoted accountId).
pub(super) async fn resolve_user(
    client: &JiraClient,
    name: &str,
    no_input: bool,
) -> Result<String> {
    if is_me_keyword(name) {
        return Ok("currentUser()".to_string());
    }

    let users = client.search_users(name).await?;
    let active_users: Vec<_> = users
        .into_iter()
        .filter(|u| u.active == Some(true))
        .collect();

    if active_users.is_empty() {
        anyhow::bail!(
            "No active user found matching \"{}\". The user may be deactivated.",
            name
        );
    }

    if active_users.len() == 1 {
        return Ok(active_users[0].account_id.clone());
    }

    // Multiple matches — disambiguate
    let display_names: Vec<String> = active_users
        .iter()
        .map(|u| u.display_name.clone())
        .collect();
    match crate::partial_match::partial_match(name, &display_names) {
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let user = active_users
                .iter()
                .find(|u| u.display_name == matched_name)
                .expect("matched name must exist in active_users");
            Ok(user.account_id.clone())
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            if no_input {
                anyhow::bail!(
                    "Multiple users match \"{}\": {}. Use a more specific name.",
                    name,
                    matches.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let user = active_users
                .iter()
                .find(|u| &u.display_name == selected_name)
                .expect("selected name must exist in active_users");
            Ok(user.account_id.clone())
        }
        crate::partial_match::MatchResult::None(_) => {
            anyhow::bail!(
                "No active user found matching \"{}\". The user may be deactivated.",
                name
            );
        }
    }
}

/// Resolve a user flag value to an (account_id, display_name) tuple for assignment.
///
/// - `"me"` (case-insensitive) → `get_myself()` (no search API call)
/// - Any other value → assignable user search API scoped to issue, disambiguate via partial_match
///
/// Unlike `resolve_user` (which returns JQL fragments), this returns concrete
/// account details for the `PUT /assignee` API.
pub(super) async fn resolve_assignee(
    client: &JiraClient,
    name: &str,
    issue_key: &str,
    no_input: bool,
) -> Result<(String, String)> {
    if is_me_keyword(name) {
        let me = client.get_myself().await?;
        return Ok((me.account_id, me.display_name));
    }

    let users = client.search_assignable_users(name, issue_key).await?;

    if users.is_empty() {
        anyhow::bail!(
            "No assignable user matching \"{}\" on issue {}. The user may not exist or may lack permission for this project. Try a different name or check spelling.",
            name,
            issue_key,
        );
    }

    if users.len() == 1 {
        return Ok((users[0].account_id.clone(), users[0].display_name.clone()));
    }

    // Multiple matches — disambiguate
    let display_names: Vec<String> = users.iter().map(|u| u.display_name.clone()).collect();
    match crate::partial_match::partial_match(name, &display_names) {
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let user = users
                .iter()
                .find(|u| u.display_name == matched_name)
                .expect("matched name must exist in users");
            Ok((user.account_id.clone(), user.display_name.clone()))
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            if no_input {
                anyhow::bail!(
                    "Multiple users match \"{}\": {}. Use a more specific name.",
                    name,
                    matches.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let user = users
                .iter()
                .find(|u| &u.display_name == selected_name)
                .expect("selected name must exist in users");
            Ok((user.account_id.clone(), user.display_name.clone()))
        }
        crate::partial_match::MatchResult::None(all_names) => {
            anyhow::bail!(
                "No assignable user with a name matching \"{}\" on issue {}. Found: {}",
                name,
                issue_key,
                all_names.join(", "),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_me_keyword_lowercase() {
        assert!(is_me_keyword("me"));
    }

    #[test]
    fn is_me_keyword_uppercase() {
        assert!(is_me_keyword("ME"));
    }

    #[test]
    fn is_me_keyword_mixed_case() {
        assert!(is_me_keyword("Me"));
    }

    #[test]
    fn is_me_keyword_not_me() {
        assert!(!is_me_keyword("Jane"));
    }
}
