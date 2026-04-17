use anyhow::{Context, Result};
use dialoguer::{Input, Password};

use crate::api::auth;
use crate::config::Config;
use crate::output;

/// Which auth flow `jr auth refresh` should dispatch to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFlow {
    Token,
    OAuth,
}

/// Decide which login flow to run based on config + explicit override.
///
/// Order of precedence:
/// 1. `oauth_override = true` → always OAuth (user passed `--oauth`).
/// 2. Config `auth_method == "oauth"` → OAuth.
/// 3. Anything else (including unset, which matches `JiraClient::from_config`'s
///    `api_token` default at `src/api/client.rs:51`) → Token.
pub fn chosen_flow(config: &Config, oauth_override: bool) -> AuthFlow {
    if oauth_override {
        return AuthFlow::OAuth;
    }
    match config.global.instance.auth_method.as_deref() {
        Some("oauth") => AuthFlow::OAuth,
        _ => AuthFlow::Token,
    }
}

/// Prompt for email and API token, then store in keychain.
pub async fn login_token() -> Result<()> {
    let email: String = dialoguer::Input::new()
        .with_prompt("Jira email")
        .interact_text()
        .context("failed to read Jira email")?;

    let token: String = dialoguer::Password::new()
        .with_prompt("API token")
        .interact()
        .context("failed to read API token")?;

    auth::store_api_token(&email, &token)?;
    eprintln!("Credentials stored in keychain.");
    Ok(())
}

/// Run the OAuth 2.0 (3LO) login flow and persist site configuration.
/// Prompts the user for their own OAuth app credentials.
pub async fn login_oauth() -> Result<()> {
    eprintln!("OAuth 2.0 requires your own Atlassian OAuth app.");
    eprintln!("Create one at: https://developer.atlassian.com/console/myapps/\n");

    let client_id: String = Input::new()
        .with_prompt("OAuth Client ID")
        .interact_text()
        .context("failed to read OAuth client ID")?;

    let client_secret: String = Password::new()
        .with_prompt("OAuth Client Secret")
        .interact()
        .context("failed to read OAuth client secret")?;

    // Store OAuth app credentials in keychain
    crate::api::auth::store_oauth_app_credentials(&client_id, &client_secret)?;

    let result = crate::api::auth::oauth_login(&client_id, &client_secret).await?;

    let mut config = Config::load().unwrap_or_default();
    config.global.instance.url = Some(result.site_url);
    config.global.instance.cloud_id = Some(result.cloud_id);
    config.global.instance.auth_method = Some("oauth".into());
    config.save_global()?;

    output::print_success(&format!("Authenticated with {}", result.site_name));
    Ok(())
}

/// Show authentication status: instance URL, auth method, credential availability.
pub async fn status() -> Result<()> {
    let config = Config::load()?;

    let url = config
        .global
        .instance
        .url
        .as_deref()
        .unwrap_or("(not configured)");
    println!("Instance:    {url}");

    let method = config
        .global
        .instance
        .auth_method
        .as_deref()
        .unwrap_or("(not configured)");
    println!("Auth method: {method}");

    let creds_ok = auth::load_api_token().is_ok();
    if creds_ok {
        println!("Credentials: stored in keychain");
    } else {
        println!("Credentials: not found");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, GlobalConfig, InstanceConfig};

    fn config_with_auth_method(method: Option<&str>) -> Config {
        Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://example.atlassian.net".into()),
                    cloud_id: None,
                    org_id: None,
                    auth_method: method.map(str::to_string),
                },
                ..Default::default()
            },
            project: Default::default(),
        }
    }

    #[test]
    fn chosen_flow_defaults_to_token_when_unset() {
        let config = config_with_auth_method(None);
        assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
    }

    #[test]
    fn chosen_flow_uses_token_for_explicit_api_token() {
        let config = config_with_auth_method(Some("api_token"));
        assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
    }

    #[test]
    fn chosen_flow_uses_oauth_when_config_says_so() {
        let config = config_with_auth_method(Some("oauth"));
        assert_eq!(chosen_flow(&config, false), AuthFlow::OAuth);
    }

    #[test]
    fn chosen_flow_oauth_override_wins_over_config() {
        let config = config_with_auth_method(Some("api_token"));
        assert_eq!(chosen_flow(&config, true), AuthFlow::OAuth);
    }
}
