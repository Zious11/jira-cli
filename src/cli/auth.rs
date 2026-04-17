use anyhow::{Context, Result};
use dialoguer::{Input, Password};

use crate::api::auth;
use crate::config::Config;
use crate::output;

/// Which auth flow `jr auth refresh` should dispatch to.
///
/// Internal detail of the `refresh` command; kept module-private so it
/// isn't part of the crate's public library API surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthFlow {
    Token,
    OAuth,
}

impl AuthFlow {
    /// Canonical string form used in config (`auth_method`) and in the
    /// `--output json` success payload. Single source of truth for the label
    /// so a future rename (e.g., `"api_token"` → `"basic"`) has one edit site.
    fn label(self) -> &'static str {
        match self {
            AuthFlow::Token => "api_token",
            AuthFlow::OAuth => "oauth",
        }
    }
}

/// Decide which login flow to run based on config + explicit override.
///
/// Order of precedence:
/// 1. `oauth_override = true` → always OAuth (user passed `--oauth`).
/// 2. Config `auth_method == "oauth"` → OAuth.
/// 3. Anything else (including unset) → Token. Matches the `api_token`
///    default that `JiraClient::from_config` applies when no method is set.
fn chosen_flow(config: &Config, oauth_override: bool) -> AuthFlow {
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

/// Post-refresh guidance shown to humans (stderr, Table mode) and embedded
/// in the JSON payload (`next_step`). Click "Always Allow" on the keychain
/// write prompts so future commands run silently.
const REFRESH_HELP_LINE: &str = "If prompted to allow keychain access, choose \"Always Allow\" so future commands run silently.";

/// Build the `--output json` success payload. Extracted for unit-testing the
/// shape (status key, auth_method label, next_step guidance) without needing
/// to drive the full login flow.
fn refresh_success_payload(flow: AuthFlow) -> serde_json::Value {
    serde_json::json!({
        "status": "refreshed",
        "auth_method": flow.label(),
        "next_step": REFRESH_HELP_LINE,
    })
}

/// Clear all stored credentials and re-run the login flow so the current
/// binary re-registers as the creator of fresh keychain entries.
///
/// On macOS this is the recovery path for the legacy Keychain ACL/partition
/// invalidation that occurs after `jr` is replaced at its installed path
/// (e.g., `brew upgrade`). See spec at
/// `docs/superpowers/specs/2026-04-17-keychain-prompts-207-design.md`.
///
/// Ordering is clear-then-login. If the login step fails (e.g., EOF on stdin,
/// network error during OAuth), the user is warned that credentials are gone
/// and told exactly which `jr auth login` invocation will restore them,
/// before the error is propagated.
pub async fn refresh_credentials(
    oauth_override: bool,
    output: &crate::cli::OutputFormat,
) -> Result<()> {
    let config = Config::load()?;
    let flow = chosen_flow(&config, oauth_override);

    auth::clear_credentials().context(
        "failed to clear stored credentials before refresh — keychain may still hold stale entries",
    )?;

    let login_result = match flow {
        AuthFlow::Token => login_token().await,
        AuthFlow::OAuth => login_oauth().await,
    };

    if let Err(err) = login_result {
        let login_cmd = match flow {
            AuthFlow::Token => "jr auth login",
            AuthFlow::OAuth => "jr auth login --oauth",
        };
        eprintln!(
            "Credentials were cleared, but the login flow did not complete. \
             Run `{login_cmd}` to restore access."
        );
        return Err(err);
    }

    match output {
        crate::cli::OutputFormat::Json => {
            let payload = serde_json::to_string_pretty(&refresh_success_payload(flow))
                .context("failed to serialize refresh success payload as JSON")?;
            println!("{payload}");
        }
        crate::cli::OutputFormat::Table => {
            eprintln!("Credentials refreshed. {REFRESH_HELP_LINE}");
        }
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

    #[test]
    fn auth_flow_labels_match_config_and_json_conventions() {
        assert_eq!(AuthFlow::Token.label(), "api_token");
        assert_eq!(AuthFlow::OAuth.label(), "oauth");
    }

    #[test]
    fn refresh_payload_pins_token_shape() {
        let payload = refresh_success_payload(AuthFlow::Token);
        assert_eq!(payload["status"], "refreshed");
        assert_eq!(payload["auth_method"], "api_token");
        assert!(
            payload["next_step"]
                .as_str()
                .unwrap()
                .contains("Always Allow"),
            "next_step should guide the user to click Always Allow, got: {}",
            payload["next_step"]
        );
    }

    #[test]
    fn refresh_payload_pins_oauth_shape() {
        let payload = refresh_success_payload(AuthFlow::OAuth);
        assert_eq!(payload["status"], "refreshed");
        assert_eq!(payload["auth_method"], "oauth");
    }
}
