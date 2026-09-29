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
/// STUB (Red Gate, BC-5.38.001): real body lands in a later story task; this
/// is a `pub(crate)` pure resolver mirroring
/// `src/cli/field.rs::resolve_m2_project`'s signature style, per BC-X.7.002
/// Fix step 4.
pub(crate) fn resolve_user_list_project(
    cli_project: Option<&str>,
    config: &Config,
) -> Option<String> {
    let _ = (cli_project, config);
    todo!()
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
    // STUB wiring (Task 1, Red Gate): a `Some(p)` post-clap value (local
    // flag, global flag, or both — clap propagation has already resolved
    // local-vs-global precedence) short-circuits straight into the existing
    // HTTP path unchanged. `resolve_user_list_project` (still `todo!()`) is
    // only reached on the `None` arm. This short-circuit is removed once the
    // resolver's real body lands (BC-X.7.002 Fix step 4).
    let resolved_project = match project {
        Some(p) => p.to_string(),
        None => resolve_user_list_project(project, config).unwrap_or_default(),
    };

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
