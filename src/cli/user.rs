use anyhow::Result;
use colored::Colorize;

use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, UserCommand, resolve_effective_limit};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::types::jira::User;

/// Resolves `jr user list`'s effective project key from the post-clap
/// `--project` value (already local-vs-global resolved by clap's own
/// global-value propagation) plus the configured default fallback chain.
///
/// `pub(crate)` pure resolver mirroring
/// `src/cli/field.rs::resolve_m2_project`'s signature style, per BC-X.7.002
/// Fix step 4: delegates entirely to `Config::project_key`'s existing
/// fallback chain (`.jr.toml` project, then the active profile's configured
/// `project` default) — no new `Config`/`ProfileConfig` accessor is added.
pub(crate) fn resolve_user_list_project(
    cli_project: Option<&str>,
    config: &Config,
) -> Option<String> {
    config.project_key(cli_project)
}

pub async fn handle(
    command: UserCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    match command {
        UserCommand::Search { query, limit, all } => {
            handle_search(&query, limit, all, output_format, client).await
        }
        UserCommand::List {
            project,
            limit,
            all,
        } => {
            handle_list(
                project.as_deref(),
                limit,
                all,
                output_format,
                config,
                client,
            )
            .await
        }
        UserCommand::View { account_id } => handle_view(&account_id, output_format, client).await,
    }
}

async fn handle_search(
    query: &str,
    limit: Option<u32>,
    all: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let effective = resolve_effective_limit(limit, all);
    let mut users = if all {
        client.search_users_all(query).await?
    } else {
        client.search_users(query).await?
    };
    if let Some(cap) = effective {
        users.truncate(cap as usize);
    }
    print_user_list(&users, output_format)
}

async fn handle_list(
    project: Option<&str>,
    limit: Option<u32>,
    all: bool,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    // BC-X.7.002 Fix step 4: unconditionally resolve via local flag > global
    // flag (both already reflected in the post-clap `project` value) >
    // configured default > exit 64, before any HTTP call.
    let resolved_project = resolve_user_list_project(project, config).ok_or_else(|| {
        JrError::UserError(
            "No project configured. Run \"jr init\" or pass --project. \
             Run \"jr project list\" to see available projects."
                .into(),
        )
    })?;

    let effective = resolve_effective_limit(limit, all);
    let mut users = if all {
        client
            .search_assignable_users_by_project_all("", &resolved_project)
            .await?
    } else {
        client
            .search_assignable_users_by_project("", &resolved_project)
            .await?
    };
    if let Some(cap) = effective {
        users.truncate(cap as usize);
    }
    print_user_list(&users, output_format)
}

async fn handle_view(
    account_id: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let user = match client.get_user(account_id).await {
        Ok(u) => u,
        Err(e) => {
            if let Some(JrError::ApiError { status, .. }) = e.downcast_ref::<JrError>()
                && (*status == 404 || *status == 400)
            {
                return Err(JrError::UserError(format!(
                    "User with accountId '{account_id}' not found."
                ))
                .into());
            }
            return Err(e);
        }
    };

    let rows = vec![
        vec!["Account ID".into(), user.account_id.clone()],
        vec!["Display Name".into(), user.display_name.clone()],
        vec![
            "Email".into(),
            user.email_address.clone().unwrap_or_else(|| "—".into()),
        ],
        vec!["Active".into(), format_active(user.active)],
    ];

    output::print_output(output_format, &["Field", "Value"], &rows, &user)
}

fn print_user_list(users: &[User], output_format: &OutputFormat) -> Result<()> {
    let rows: Vec<Vec<String>> = users.iter().map(format_user_row).collect();
    output::print_output(
        output_format,
        &["Display Name", "Email", "Active", "Account ID"],
        &rows,
        &users,
    )
}

fn format_user_row(user: &User) -> Vec<String> {
    vec![
        user.display_name.clone(),
        user.email_address.clone().unwrap_or_else(|| "—".into()),
        format_active(user.active),
        user.account_id.clone(),
    ]
}

fn format_active(active: Option<bool>) -> String {
    match active {
        Some(true) => "✓".green().to_string(),
        Some(false) => "✗".red().to_string(),
        None => "—".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GlobalConfig, ProfileConfig, ProjectConfig};
    use proptest::prelude::*;

    /// Builds a `Config` with the active profile named `"default"`, an
    /// optional `.jr.toml`-equivalent `project` value, and an optional
    /// profile-level `project` default — the two independently-controllable
    /// sources `Config::project_key` falls back through.
    fn make_config(jr_toml_project: Option<String>, profile_project: Option<String>) -> Config {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                project: profile_project,
                ..ProfileConfig::default()
            },
        );
        Config {
            global: GlobalConfig {
                default_profile: Some("default".to_string()),
                profiles,
                ..GlobalConfig::default()
            },
            project: ProjectConfig {
                project: jr_toml_project,
                ..ProjectConfig::default()
            },
            active_profile_name: "default".into(),
        }
    }

    proptest! {
        /// AC-003 / AC-006 (BC-X.7.002 Fix step 4, VP-USER-LIST-PROJECT-001(b)):
        /// `resolve_user_list_project` over the full presence space of
        /// `cli_project` (`Some(C)`, `Some("")` — EC-X.7.002-6, `None`)
        /// crossed with the four configured-source cells (neither,
        /// `.jr.toml`-only, profile-only, both — `.jr.toml` wins over the
        /// profile default, EC-X.7.002-5's caveat).
        #[test]
        fn test_bc_x_7_002_resolve_user_list_project_presence_space(
            c in "c-[a-zA-Z0-9]{1,8}",
            j in "j-[a-zA-Z0-9]{1,8}",
            p in "p-[a-zA-Z0-9]{1,8}",
        ) {
            let cells: [(Option<String>, Option<String>, Option<String>); 4] = [
                (None, None, None),
                (Some(j.clone()), None, Some(j.clone())),
                (None, Some(p.clone()), Some(p.clone())),
                (Some(j.clone()), Some(p.clone()), Some(j.clone())),
            ];
            for (jr_toml, profile, expected_when_cli_absent) in cells {
                let config = make_config(jr_toml, profile);

                // cli_project = Some(C) -> Some(C) in every configured cell.
                prop_assert_eq!(
                    resolve_user_list_project(Some(c.as_str()), &config),
                    Some(c.clone())
                );

                // EC-X.7.002-6: cli_project = Some("") -> Some(""), configured default not consulted.
                prop_assert_eq!(
                    resolve_user_list_project(Some(""), &config),
                    Some(String::new())
                );

                // cli_project = None -> configured fallback chain (.jr.toml > profile > None).
                prop_assert_eq!(
                    resolve_user_list_project(None, &config),
                    expected_when_cli_absent
                );
            }
        }
    }

    #[test]
    fn row_shows_display_name_email_and_id() {
        let user = User {
            account_id: "acc-1".into(),
            display_name: "Alice".into(),
            email_address: Some("alice@acme.io".into()),
            active: Some(true),
        };
        let row = format_user_row(&user);
        assert_eq!(row[0], "Alice");
        assert_eq!(row[1], "alice@acme.io");
        assert!(row[2].contains('✓'));
        assert_eq!(row[3], "acc-1");
    }

    #[test]
    fn row_renders_dash_for_missing_email() {
        let user = User {
            account_id: "acc-2".into(),
            display_name: "Privacy User".into(),
            email_address: None,
            active: Some(true),
        };
        let row = format_user_row(&user);
        assert_eq!(row[1], "—");
    }

    #[test]
    fn active_formatter_handles_missing() {
        assert_eq!(format_active(None), "—");
    }
}
