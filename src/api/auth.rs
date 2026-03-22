use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "jr-jira-cli";

/// Key names stored in the system keychain.
const KEY_EMAIL: &str = "email";
const KEY_API_TOKEN: &str = "api-token";
const KEY_OAUTH_ACCESS: &str = "oauth-access-token";
const KEY_OAUTH_REFRESH: &str = "oauth-refresh-token";

fn entry(key: &str) -> Result<Entry> {
    Entry::new(SERVICE_NAME, key).context("Failed to access keychain")
}

/// Store an API token and associated email in the system keychain.
pub fn store_api_token(email: &str, token: &str) -> Result<()> {
    entry(KEY_EMAIL)?.set_password(email)?;
    entry(KEY_API_TOKEN)?.set_password(token)?;
    Ok(())
}

/// Load the stored API token and email from the system keychain.
/// Returns `(email, token)`.
pub fn load_api_token() -> Result<(String, String)> {
    let email = entry(KEY_EMAIL)?
        .get_password()
        .context("No stored email — run \"jr auth login\"")?;
    let token = entry(KEY_API_TOKEN)?
        .get_password()
        .context("No stored API token — run \"jr auth login\"")?;
    Ok((email, token))
}

/// Store OAuth 2.0 access and refresh tokens in the system keychain.
pub fn store_oauth_tokens(access: &str, refresh: &str) -> Result<()> {
    entry(KEY_OAUTH_ACCESS)?.set_password(access)?;
    entry(KEY_OAUTH_REFRESH)?.set_password(refresh)?;
    Ok(())
}

/// Load OAuth 2.0 access and refresh tokens from the system keychain.
/// Returns `(access_token, refresh_token)`.
pub fn load_oauth_tokens() -> Result<(String, String)> {
    let access = entry(KEY_OAUTH_ACCESS)?
        .get_password()
        .context("No stored OAuth token — run \"jr auth login\"")?;
    let refresh = entry(KEY_OAUTH_REFRESH)?
        .get_password()
        .context("No stored OAuth refresh token — run \"jr auth login\"")?;
    Ok((access, refresh))
}

/// Remove all stored credentials from the system keychain.
pub fn clear_credentials() {
    for key in [
        KEY_EMAIL,
        KEY_API_TOKEN,
        KEY_OAUTH_ACCESS,
        KEY_OAUTH_REFRESH,
    ] {
        if let Ok(e) = entry(key) {
            let _ = e.delete_credential();
        }
    }
}
