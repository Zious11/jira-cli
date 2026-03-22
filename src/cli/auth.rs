use anyhow::Result;

use crate::api::auth;
use crate::config::Config;

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

/// Placeholder for OAuth login — not yet implemented.
pub async fn login_oauth() -> Result<()> {
    anyhow::bail!(
        "OAuth login not yet implemented. Use \"jr auth login --token\" for API token auth."
    )
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
