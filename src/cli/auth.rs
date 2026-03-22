use anyhow::Result;

use crate::api::auth;
use crate::config::Config;
use crate::output;

/// Prompt for email and API token, then store in keychain.
pub async fn login_token() -> Result<()> {
    let email: String = dialoguer::Input::new()
        .with_prompt("Jira email")
        .interact_text()?;

    let token: String = dialoguer::Password::new()
        .with_prompt("API token")
        .interact()?;

    auth::store_api_token(&email, &token)?;
    println!("Credentials stored in keychain.");
    Ok(())
}

/// Run the OAuth 2.0 (3LO) login flow and persist site configuration.
pub async fn login_oauth() -> Result<()> {
    let result = crate::api::auth::oauth_login().await?;

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
