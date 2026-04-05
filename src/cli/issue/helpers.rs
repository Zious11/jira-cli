use anyhow::Result;

use crate::api::client::JiraClient;
use crate::config::Config;
use crate::error::JrError;
use crate::types::jira::User;

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
            let idx = teams
                .iter()
                .position(|t| t.name == matched_name)
                .expect("matched name must exist in teams");
            Ok((field_id, teams[idx].id.clone()))
        }
        crate::partial_match::MatchResult::ExactMultiple(_) => {
            let name_lower = team_name.to_lowercase();
            let duplicates: Vec<&crate::cache::CachedTeam> = teams
                .iter()
                .filter(|t| t.name.to_lowercase() == name_lower)
                .collect();

            if no_input {
                let lines: Vec<String> = duplicates
                    .iter()
                    .map(|t| format!("  {} (id: {})", t.name, t.id))
                    .collect();
                anyhow::bail!(
                    "Multiple teams named \"{}\" found:\n{}\nUse a more specific name.",
                    team_name,
                    lines.join("\n")
                );
            }

            let labels: Vec<String> = duplicates
                .iter()
                .map(|t| format!("{} ({})", t.name, t.id))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple teams named \"{}\"", team_name))
                .items(&labels)
                .interact()?;
            Ok((field_id, duplicates[selection].id.clone()))
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
            let idx = teams
                .iter()
                .position(|t| t.name == *selected_name)
                .expect("selected name must exist in teams");
            Ok((field_id, teams[idx].id.clone()))
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

// ── Shared user disambiguation ──────────────────────────────────────

/// Disambiguate a list of users by display name using partial matching.
///
/// Handles: empty list, single result, exact match, duplicate display names,
/// ambiguous substring match, and no match. In interactive mode, prompts the
/// user to choose when ambiguous.
///
/// Returns `(account_id, display_name)` of the selected user.
fn disambiguate_user(
    users: &[User],
    name: &str,
    no_input: bool,
    empty_msg: &str,
    none_msg_fn: impl Fn(&[String]) -> String,
) -> Result<(String, String)> {
    if users.is_empty() {
        anyhow::bail!("{}", empty_msg);
    }

    if users.len() == 1 {
        return Ok((users[0].account_id.clone(), users[0].display_name.clone()));
    }

    let display_names: Vec<String> = users.iter().map(|u| u.display_name.clone()).collect();
    match crate::partial_match::partial_match(name, &display_names) {
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let idx = users
                .iter()
                .position(|u| u.display_name == matched_name)
                .expect("matched name must exist in users");
            Ok((
                users[idx].account_id.clone(),
                users[idx].display_name.clone(),
            ))
        }
        crate::partial_match::MatchResult::ExactMultiple(_) => {
            let name_lower = name.to_lowercase();
            let duplicates: Vec<&User> = users
                .iter()
                .filter(|u| u.display_name.to_lowercase() == name_lower)
                .collect();

            if no_input {
                let lines: Vec<String> = duplicates
                    .iter()
                    .map(|u| match &u.email_address {
                        Some(email) => format!(
                            "  {} ({}, account: {})",
                            u.display_name, email, u.account_id
                        ),
                        None => {
                            format!("  {} (account: {})", u.display_name, u.account_id)
                        }
                    })
                    .collect();
                anyhow::bail!(
                    "Multiple users named \"{}\" found:\n{}\nSpecify the accountId directly or use a more specific name.",
                    name,
                    lines.join("\n")
                );
            }

            let labels: Vec<String> = duplicates
                .iter()
                .map(|u| match &u.email_address {
                    Some(email) => format!("{} ({})", u.display_name, email),
                    None => format!("{} ({})", u.display_name, u.account_id),
                })
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users named \"{}\"", name))
                .items(&labels)
                .interact()?;
            Ok((
                duplicates[selection].account_id.clone(),
                duplicates[selection].display_name.clone(),
            ))
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
            let idx = users
                .iter()
                .position(|u| u.display_name == *selected_name)
                .expect("selected name must exist in users");
            Ok((
                users[idx].account_id.clone(),
                users[idx].display_name.clone(),
            ))
        }
        crate::partial_match::MatchResult::None(all_names) => {
            anyhow::bail!("{}", none_msg_fn(&all_names));
        }
    }
}

// ── Public resolve functions ─────────────────────────────────────────

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

    let (account_id, _) = disambiguate_user(
        &active_users,
        name,
        no_input,
        &format!(
            "No active user found matching \"{}\". The user may be deactivated.",
            name
        ),
        |_all_names| {
            format!(
                "No active user found matching \"{}\". The user may be deactivated.",
                name
            )
        },
    )?;
    Ok(account_id)
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

    disambiguate_user(
        &users,
        name,
        no_input,
        &format!(
            "No assignable user matching \"{}\" on issue {}. The user may not exist or may lack permission for this project. Try a different name or check spelling.",
            name, issue_key,
        ),
        |all_names| {
            format!(
                "No assignable user with a name matching \"{}\" on issue {}. Found: {}",
                name,
                issue_key,
                all_names.join(", "),
            )
        },
    )
}

/// Resolve a user flag value to an (account_id, display_name) tuple for assignment by project.
///
/// - `"me"` (case-insensitive) → `get_myself()` (no search API call)
/// - Any other value → assignable user search API scoped to project, disambiguate via partial_match
///
/// Unlike `resolve_assignee` (which takes an issue key), this takes a project key
/// and uses the `multiProjectSearch` endpoint. Used during issue creation when no
/// issue key exists yet.
pub(super) async fn resolve_assignee_by_project(
    client: &JiraClient,
    name: &str,
    project_key: &str,
    no_input: bool,
) -> Result<(String, String)> {
    if is_me_keyword(name) {
        let me = client.get_myself().await?;
        return Ok((me.account_id, me.display_name));
    }

    // The multiProjectSearch endpoint returns only users eligible for assignment,
    // which should exclude deactivated users. No client-side active filter needed
    // (consistent with resolve_assignee for issue-scoped search).
    let users = client
        .search_assignable_users_by_project(name, project_key)
        .await?;

    disambiguate_user(
        &users,
        name,
        no_input,
        &format!(
            "No assignable user matching \"{}\" in project {}. The user may not exist or may lack permission for this project. Try a different name or check spelling.",
            name, project_key,
        ),
        |all_names| {
            format!(
                "No assignable user with a name matching \"{}\" in project {}. Found: {}",
                name,
                project_key,
                all_names.join(", "),
            )
        },
    )
}

/// Resolve an `--asset` flag value to an object key.
///
/// - Value matches `SCHEMA-NUMBER` key pattern → return as-is (no API call)
/// - Otherwise → search Assets by name via AQL, disambiguate if multiple matches
///
/// Returns the resolved object key (e.g., `"OBJ-18"`).
pub(super) async fn resolve_asset(
    client: &JiraClient,
    input: &str,
    no_input: bool,
) -> Result<String> {
    // Key pattern → passthrough (no API call)
    if crate::jql::validate_asset_key(input).is_ok() {
        return Ok(input.to_string());
    }

    // Name search: fetch workspace ID, then AQL search
    let workspace_id = crate::api::assets::workspace::get_or_fetch_workspace_id(client).await?;
    let escaped = crate::jql::escape_value(input);
    let aql = format!("Name like \"{}\"", escaped);
    let results = client
        .search_assets(&workspace_id, &aql, Some(25), false)
        .await?;

    if results.is_empty() {
        anyhow::bail!(
            "No assets matching \"{}\" found. Check the name and try again.",
            input
        );
    }

    if results.len() == 1 {
        return Ok(results.into_iter().next().unwrap().object_key);
    }

    // Multiple results — disambiguate via partial_match on labels
    let labels: Vec<String> = results.iter().map(|a| a.label.clone()).collect();
    match crate::partial_match::partial_match(input, &labels) {
        crate::partial_match::MatchResult::Exact(matched_label) => {
            let asset = results
                .iter()
                .find(|a| a.label == matched_label)
                .expect("matched label must exist in results");
            Ok(asset.object_key.clone())
        }
        crate::partial_match::MatchResult::ExactMultiple(_) => {
            // Multiple assets with same label — need key to disambiguate
            let label_lower = input.to_lowercase();
            let duplicates: Vec<_> = results
                .iter()
                .filter(|a| a.label.to_lowercase() == label_lower)
                .collect();

            if no_input {
                let lines: Vec<String> = duplicates
                    .iter()
                    .map(|a| format!("  {} ({})", a.object_key, a.label))
                    .collect();
                anyhow::bail!(
                    "Multiple assets match \"{}\":\n{}\nUse a more specific name or pass the object key directly.",
                    input,
                    lines.join("\n")
                );
            }

            let items: Vec<String> = duplicates
                .iter()
                .map(|a| format!("{} ({})", a.object_key, a.label))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple assets match \"{}\"", input))
                .items(&items)
                .interact()?;
            Ok(duplicates[selection].object_key.clone())
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            let filtered: Vec<_> = results
                .iter()
                .filter(|a| matches.contains(&a.label))
                .collect();

            if no_input {
                let lines: Vec<String> = filtered
                    .iter()
                    .map(|a| format!("  {} ({})", a.object_key, a.label))
                    .collect();
                anyhow::bail!(
                    "Multiple assets match \"{}\":\n{}\nUse a more specific name or pass the object key directly.",
                    input,
                    lines.join("\n")
                );
            }

            let items: Vec<String> = filtered
                .iter()
                .map(|a| format!("{} ({})", a.object_key, a.label))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple assets match \"{}\"", input))
                .items(&items)
                .interact()?;
            Ok(filtered[selection].object_key.clone())
        }
        crate::partial_match::MatchResult::None(_) => {
            // AQL returned results but partial_match found no substring match.
            // This shouldn't normally happen (AQL already filtered by Name like),
            // but handle gracefully.
            let lines: Vec<String> = results
                .iter()
                .map(|a| format!("  {} ({})", a.object_key, a.label))
                .collect();
            anyhow::bail!(
                "No assets with a name matching \"{}\" found. Similar results:\n{}\nUse the object key directly.",
                input,
                lines.join("\n")
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

    // ── disambiguate_user tests ──────────────────────────────────────

    fn make_user(account_id: &str, display_name: &str) -> User {
        User {
            account_id: account_id.to_string(),
            display_name: display_name.to_string(),
            email_address: None,
            active: Some(true),
        }
    }

    fn make_user_with_email(account_id: &str, display_name: &str, email: &str) -> User {
        User {
            account_id: account_id.to_string(),
            display_name: display_name.to_string(),
            email_address: Some(email.to_string()),
            active: Some(true),
        }
    }

    fn dummy_none_msg(all_names: &[String]) -> String {
        format!("No match. Found: {}", all_names.join(", "))
    }

    #[test]
    fn disambiguate_empty_list_returns_error() {
        let result = disambiguate_user(&[], "Jane", true, "No users found", dummy_none_msg);
        let err = result.unwrap_err();
        assert!(err.to_string().contains("No users found"));
    }

    #[test]
    fn disambiguate_single_user_returns_directly() {
        let users = vec![make_user("acc-1", "Jane Doe")];
        let (id, name) = disambiguate_user(&users, "Jane", true, "empty", dummy_none_msg).unwrap();
        assert_eq!(id, "acc-1");
        assert_eq!(name, "Jane Doe");
    }

    #[test]
    fn disambiguate_exact_match() {
        let users = vec![
            make_user("acc-1", "Jane Doe"),
            make_user("acc-2", "Janet Smith"),
        ];
        let (id, name) =
            disambiguate_user(&users, "Jane Doe", true, "empty", dummy_none_msg).unwrap();
        assert_eq!(id, "acc-1");
        assert_eq!(name, "Jane Doe");
    }

    #[test]
    fn disambiguate_exact_multiple_no_input_errors_with_details() {
        let users = vec![
            make_user_with_email("acc-1", "Jane Doe", "jane1@example.com"),
            make_user("acc-2", "Jane Doe"),
        ];
        let result = disambiguate_user(&users, "Jane Doe", true, "empty", dummy_none_msg);
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Multiple users named \"Jane Doe\""));
        assert!(msg.contains("jane1@example.com"));
        assert!(msg.contains("acc-2"));
    }

    #[test]
    fn disambiguate_ambiguous_no_input_errors_with_candidates() {
        let users = vec![
            make_user("acc-1", "Jane Doe"),
            make_user("acc-2", "Jane Smith"),
        ];
        let result = disambiguate_user(&users, "Jane", true, "empty", dummy_none_msg);
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Multiple users match \"Jane\""));
        assert!(msg.contains("Jane Doe"));
        assert!(msg.contains("Jane Smith"));
    }

    #[test]
    fn disambiguate_no_match_uses_none_msg_fn() {
        let users = vec![make_user("acc-1", "Alice"), make_user("acc-2", "Bob")];
        let result = disambiguate_user(&users, "Zara", true, "empty", dummy_none_msg);
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("No match. Found:"));
        assert!(msg.contains("Alice"));
        assert!(msg.contains("Bob"));
    }
}
