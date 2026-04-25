use anyhow::{Context, Result};

use crate::api::client::JiraClient;
use crate::config::Config;
use crate::error::JrError;
use crate::types::jira::User;

/// Detect Atlassian team UUID format: 36 chars, hex digits split into
/// 8-4-4-4-12 groups by hyphens. Case-insensitive on hex.
///
/// Used to short-circuit the cache-name-match path for agents that already
/// know a team's ID — `--team <uuid>` sends the value straight to the
/// customfield without a cache lookup or name match.
fn is_team_uuid(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if *b != b'-' {
                    return false;
                }
            }
            _ => {
                if !b.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

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

    // 2. UUID pass-through: if the caller already has a team UUID (agents,
    // scripts), skip cache + name-match entirely. The customfield accepts
    // the UUID directly — no lookup needed. Pre-cache-load so a cold cache
    // doesn't force a teams fetch just to validate an ID we already have.
    if is_team_uuid(team_name) {
        if client.verbose() {
            eprintln!("[verbose] team resolved via UUID pass-through: {team_name}");
        }
        return Ok((field_id, team_name.to_string()));
    }

    // 3. Load teams from cache (or fetch if missing/expired). `cache_was_fresh`
    // tells step 5 whether an auto-refresh-on-miss is worth attempting —
    // no point re-fetching a list we just fetched.
    let (teams, cache_was_fresh) = match crate::cache::read_team_cache(&config.active_profile_name)?
    {
        Some(cached) => (cached.teams, false),
        None => (
            crate::cli::team::fetch_and_cache_teams(config, client).await?,
            true,
        ),
    };

    // 4. Partial match
    let team_names: Vec<String> = teams.iter().map(|t| t.name.clone()).collect();
    let match_result = crate::partial_match::partial_match(team_name, &team_names);

    // 5. Auto-refresh on miss: if the cache came from disk and the name
    // didn't match anything, the team was likely added upstream since the
    // last refresh. Fetch once transparently and retry. Bounded to a
    // single retry via `cache_was_fresh` — no infinite-refresh loop.
    let (teams, match_result, retry_fetched) =
        if matches!(match_result, crate::partial_match::MatchResult::None(_)) && !cache_was_fresh {
            if client.verbose() {
                eprintln!("[verbose] team \"{team_name}\" not in cache, refreshing from server...");
            }
            let fresh = crate::cli::team::fetch_and_cache_teams(config, client).await?;
            let fresh_names: Vec<String> = fresh.iter().map(|t| t.name.clone()).collect();
            let retry = crate::partial_match::partial_match(team_name, &fresh_names);
            (fresh, retry, true)
        } else {
            (teams, match_result, false)
        };

    // Any fetch during this call (either the initial cold-cache fetch at
    // step 3 or the retry above) means the bail message at step 6 can
    // avoid the stale "run `jr team list --refresh`" advice — the user
    // just effectively did.
    let fetched_fresh = cache_was_fresh || retry_fetched;

    match match_result {
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
                return Err(JrError::UserError(format!(
                    "Multiple teams named \"{}\" found:\n{}\nUse a more specific name.",
                    team_name,
                    lines.join("\n")
                ))
                .into());
            }

            let labels: Vec<String> = duplicates
                .iter()
                .map(|t| format!("{} ({})", t.name, t.id))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple teams named \"{}\"", team_name))
                .items(&labels)
                .interact()
                .context("failed to prompt for team selection")?;
            Ok((field_id, duplicates[selection].id.clone()))
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            if no_input {
                let quoted: Vec<String> = matches.iter().map(|m| format!("\"{}\"", m)).collect();
                return Err(JrError::UserError(format!(
                    "Multiple teams match \"{}\": {}. Use a more specific name.",
                    team_name,
                    quoted.join(", ")
                ))
                .into());
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple teams match \"{team_name}\""))
                .items(&matches)
                .interact()
                .context("failed to prompt for team selection")?;
            let selected_name = &matches[selection];
            let idx = teams
                .iter()
                .position(|t| t.name == *selected_name)
                .expect("selected name must exist in teams");
            Ok((field_id, teams[idx].id.clone()))
        }
        crate::partial_match::MatchResult::None(_) => {
            // Any fresh fetch this call (cold-cache or retry) means advising
            // "run jr team list --refresh" would be misleading — we just did.
            if fetched_fresh {
                return Err(JrError::UserError(format!(
                    "No team matching \"{}\" (checked a fresh team list). \
                     Verify the team name or check access permissions.",
                    team_name
                ))
                .into());
            }
            Err(JrError::UserError(format!(
                "No team matching \"{}\". Run \"jr team list --refresh\" to update.",
                team_name
            ))
            .into())
        }
    }
}

/// Compose the `extra` fields list that both `handle_view` and `handle_create`
/// pass to `JiraClient::get_issue`. Order: story-points, cmdb ids, team.
pub(super) fn compose_extra_fields(
    config: &Config,
    cmdb_fields: &[(String, String)],
) -> Vec<String> {
    let mut extra: Vec<String> = Vec::new();
    if let Some(sp) = config.global.fields.story_points_field_id.as_deref() {
        extra.push(sp.to_string());
    }
    for (id, _) in cmdb_fields {
        extra.push(id.clone());
    }
    if let Some(t) = config.global.fields.team_field_id.as_deref() {
        extra.push(t.to_string());
    }
    extra
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
        .interact_text()
        .with_context(|| format!("failed to read {}", prompt))?;
    Ok(input)
}

/// Check if a user input string is the "me" keyword (case-insensitive).
pub(super) fn is_me_keyword(input: &str) -> bool {
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
        return Err(JrError::UserError(empty_msg.to_string()).into());
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
                return Err(JrError::UserError(format!(
                    "Multiple users named \"{}\" found:\n{}\nSpecify the accountId directly or use a more specific name.",
                    name,
                    lines.join("\n")
                ))
                .into());
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
                .interact()
                .context("failed to prompt for user selection")?;
            Ok((
                duplicates[selection].account_id.clone(),
                duplicates[selection].display_name.clone(),
            ))
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            if no_input {
                return Err(JrError::UserError(format!(
                    "Multiple users match \"{}\": {}. Use a more specific name.",
                    name,
                    matches.join(", ")
                ))
                .into());
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()
                .context("failed to prompt for user selection")?;
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
            Err(JrError::UserError(none_msg_fn(&all_names)).into())
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
        return Err(JrError::UserError(format!(
            "No assets matching \"{}\" found. Check the name and try again.",
            input
        ))
        .into());
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
                return Err(JrError::UserError(format!(
                    "Multiple assets match \"{}\":\n{}\nUse a more specific name or pass the object key directly.",
                    input,
                    lines.join("\n")
                ))
                .into());
            }

            let items: Vec<String> = duplicates
                .iter()
                .map(|a| format!("{} ({})", a.object_key, a.label))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple assets match \"{}\"", input))
                .items(&items)
                .interact()
                .context("failed to prompt for asset selection")?;
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
                return Err(JrError::UserError(format!(
                    "Multiple assets match \"{}\":\n{}\nUse a more specific name or pass the object key directly.",
                    input,
                    lines.join("\n")
                ))
                .into());
            }

            let items: Vec<String> = filtered
                .iter()
                .map(|a| format!("{} ({})", a.object_key, a.label))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple assets match \"{}\"", input))
                .items(&items)
                .interact()
                .context("failed to prompt for asset selection")?;
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
            Err(JrError::UserError(format!(
                "No assets with a name matching \"{}\" found. Similar results:\n{}\nUse the object key directly.",
                input,
                lines.join("\n")
            ))
            .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_team_uuid_accepts_standard_uuid() {
        assert!(is_team_uuid("36885b3c-1bf0-4f85-a357-c5b858c31de4"));
    }

    #[test]
    fn is_team_uuid_accepts_uppercase_hex() {
        // Atlassian UUIDs are typically lowercase but the format is
        // case-insensitive — accept either so users don't get bitten by
        // copy-paste casing.
        assert!(is_team_uuid("36885B3C-1BF0-4F85-A357-C5B858C31DE4"));
    }

    #[test]
    fn is_team_uuid_accepts_mixed_case_hex() {
        assert!(is_team_uuid("36885B3c-1bF0-4F85-a357-c5B858c31De4"));
    }

    #[test]
    fn is_team_uuid_rejects_wrong_length() {
        assert!(!is_team_uuid("36885b3c-1bf0-4f85-a357-c5b858c31de"));
        assert!(!is_team_uuid("36885b3c-1bf0-4f85-a357-c5b858c31de44"));
        assert!(!is_team_uuid(""));
    }

    #[test]
    fn is_team_uuid_rejects_missing_hyphen() {
        // Right length, wrong separator at position 8
        assert!(!is_team_uuid("36885b3c11bf0-4f85-a357-c5b858c31de4"));
    }

    #[test]
    fn is_team_uuid_rejects_non_hex() {
        // 'g' is not a hex digit
        assert!(!is_team_uuid("g6885b3c-1bf0-4f85-a357-c5b858c31de4"));
    }

    #[test]
    fn is_team_uuid_rejects_plausible_team_name_with_hyphens() {
        // Plausible team names shouldn't pass as UUIDs. 37 chars and 12 chars
        // — both rejected by the length check (complements the 36-char
        // hyphen-position test below).
        assert!(!is_team_uuid("backend-platform-team-lead-group-main")); // 37
        assert!(!is_team_uuid("Platform Ops")); // 12
    }

    #[test]
    fn is_team_uuid_rejects_hyphens_in_wrong_position_at_36_chars() {
        // 36-char hyphenated string with hyphens at non-UUID positions
        // (12, 17, 22, 27 vs the required 8, 13, 18, 23). Exercises the
        // per-position validation rather than the length gate.
        assert!(!is_team_uuid("platform-ops-team-lead-group-mainxxx"));
    }

    #[test]
    fn is_team_uuid_rejects_non_hex_at_hex_position() {
        // 36-char UUID-shaped string with correct hyphen positions but
        // non-hex ('x') in the final group. Exercises the hex-slot check
        // at indices past all hyphen positions.
        assert!(!is_team_uuid("aaaaaaaa-aaaa-aaaa-aaaa-xxxxxxxxxxxx"));
    }

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

    // ── compose_extra_fields tests ────────────────────────────────────

    #[test]
    fn extra_fields_for_issue_composes_sp_team_and_cmdb() {
        use crate::config::Config;

        let mut config = Config::default();
        config.global.fields.story_points_field_id = Some("customfield_10016".into());
        config.global.fields.team_field_id = Some("customfield_10001".into());
        let cmdb_fields = vec![
            (
                "customfield_12345".to_string(),
                "Affected Services".to_string(),
            ),
            ("customfield_67890".to_string(), "Deployed To".to_string()),
        ];

        let extra = compose_extra_fields(&config, &cmdb_fields);

        // compose_extra_fields documents the order: story-points first, CMDB
        // ids preserved in slice order, team last. Asserting the full vector
        // (not just membership) pins that contract so a refactor that changes
        // order — which could break downstream callers relying on it — trips
        // this test instead of escaping into production.
        assert_eq!(
            extra,
            vec![
                "customfield_10016".to_string(), // SP first
                "customfield_12345".to_string(), // CMDB preserved in slice order
                "customfield_67890".to_string(), // CMDB preserved in slice order
                "customfield_10001".to_string(), // team last
            ],
        );
    }

    #[test]
    fn extra_fields_for_issue_omits_unset_optionals() {
        use crate::config::Config;
        let config = Config::default();
        let cmdb_fields: Vec<(String, String)> = vec![];
        let extra = compose_extra_fields(&config, &cmdb_fields);
        assert!(extra.is_empty());
    }
}
