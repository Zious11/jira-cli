use std::net::TcpListener;

use anyhow::{Context, Result};
use keyring::Entry;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener as AsyncTcpListener;

/// Default keychain service name for `jr` credentials. `JR_SERVICE_NAME`
/// can override this at runtime; it is primarily used by tests to avoid
/// touching a developer's real keychain.
const DEFAULT_SERVICE_NAME: &str = "jr-jira-cli";

/// Resolve the keychain service name, honoring `JR_SERVICE_NAME` whenever
/// it is set. All keychain operations go through this, so changing it also
/// changes where credentials are stored and loaded (for example, tests can
/// scope their own namespace with `"jr-jira-cli-test"`).
fn service_name() -> String {
    std::env::var("JR_SERVICE_NAME").unwrap_or_else(|_| DEFAULT_SERVICE_NAME.to_string())
}

/// Key names stored in the system keychain.
const KEY_EMAIL: &str = "email";
const KEY_API_TOKEN: &str = "api-token";
const KEY_OAUTH_ACCESS: &str = "oauth-access-token";
const KEY_OAUTH_REFRESH: &str = "oauth-refresh-token";

/// Default OAuth 2.0 scopes used when `oauth_scopes` is not set in
/// config.toml. Matches Atlassian's "classic" scope recommendation for
/// Jira Platform apps. Users who configured their Developer Console app
/// with granular scopes (e.g., for least-privilege agent use) should
/// override via `[instance].oauth_scopes` in config.toml.
pub const DEFAULT_OAUTH_SCOPES: &str =
    "read:jira-work write:jira-work read:jira-user offline_access";

fn entry(key: &str) -> Result<Entry> {
    Entry::new(&service_name(), key).context("Failed to access keychain")
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

/// Store OAuth app credentials (client_id and client_secret) in the system keychain.
pub fn store_oauth_app_credentials(client_id: &str, client_secret: &str) -> Result<()> {
    let service = service_name();
    let entry = Entry::new(&service, "oauth_client_id")?;
    entry.set_password(client_id)?;
    let entry = Entry::new(&service, "oauth_client_secret")?;
    entry.set_password(client_secret)?;
    Ok(())
}

/// Load OAuth app credentials (client_id and client_secret) from the system keychain.
pub fn load_oauth_app_credentials() -> Result<(String, String)> {
    let service = service_name();
    let id_entry = Entry::new(&service, "oauth_client_id")?;
    let id = id_entry
        .get_password()
        .context("No OAuth app credentials found. Run \"jr auth login --oauth\" and provide your client_id and client_secret.")?;
    let secret_entry = Entry::new(&service, "oauth_client_secret")?;
    let secret = secret_entry
        .get_password()
        .context("No OAuth app credentials found.")?;
    Ok((id, secret))
}

/// Remove all stored credentials from the system keychain.
///
/// `NoEntry` results are treated as success (the entry was already absent,
/// which is the expected case on a fresh install or after a prior clear).
/// Any other failure (permission denied, ACL mismatch, platform error) is
/// aggregated and returned so callers can decide whether to proceed — for
/// example, `jr auth refresh` needs to know if the clear actually happened
/// before reporting the refresh as successful.
pub fn clear_credentials() -> Result<()> {
    let mut failures: Vec<String> = Vec::new();
    for key in [
        KEY_EMAIL,
        KEY_API_TOKEN,
        KEY_OAUTH_ACCESS,
        KEY_OAUTH_REFRESH,
        "oauth_client_id",
        "oauth_client_secret",
    ] {
        match entry(key) {
            Ok(e) => match e.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(err) => failures.push(format!("{key}: {err}")),
            },
            Err(err) => failures.push(format!("{key}: {err}")),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "failed to clear {} keychain {}: {}",
            failures.len(),
            if failures.len() == 1 {
                "entry"
            } else {
                "entries"
            },
            failures.join("; ")
        ))
    }
}

/// Result of a successful OAuth login containing site information.
pub struct OAuthResult {
    pub cloud_id: String,
    pub site_url: String,
    pub site_name: String,
}

/// Run the full OAuth 2.0 (3LO) authorization code flow:
/// 1. Open browser to Atlassian authorization page requesting `scopes`
/// 2. Listen on a local port for the callback
/// 3. Exchange the authorization code for tokens
/// 4. Fetch accessible resources to get the cloud ID
/// 5. Store tokens in the system keychain
///
/// `scopes` is a space-separated scope string (URL-encoded internally).
/// Callers should use [`DEFAULT_OAUTH_SCOPES`] when no user override is set.
/// Note: [`refresh_oauth_token`] does NOT take a scope parameter — the
/// `refresh_token` grant inherits scopes from the original authorization.
pub async fn oauth_login(
    client_id: &str,
    client_secret: &str,
    scopes: &str,
) -> Result<OAuthResult> {
    // 1. Find an available port for the local callback server.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);

    let redirect_uri = format!("http://localhost:{port}/callback");
    let state = generate_state();

    let auth_url = format!(
        "https://auth.atlassian.com/authorize\
         ?audience=api.atlassian.com\
         &client_id={client_id}\
         &scope={}\
         &redirect_uri={redirect_uri}\
         &state={state}\
         &response_type=code\
         &prompt=consent",
        urlencoding::encode(scopes),
    );

    eprintln!("Opening browser for authorization...");
    eprintln!("If browser doesn't open, visit: {auth_url}");
    let _ = open::that(&auth_url);

    // 2. Listen for the OAuth callback.
    let async_listener = AsyncTcpListener::bind(format!("127.0.0.1:{port}")).await?;
    let (mut stream, _) = async_listener.accept().await?;

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    let code = extract_query_param(&request, "code")
        .ok_or_else(|| anyhow::anyhow!("No authorization code received"))?;
    let returned_state = extract_query_param(&request, "state")
        .ok_or_else(|| anyhow::anyhow!("No state parameter received"))?;

    if returned_state != state {
        anyhow::bail!("State mismatch — possible CSRF attack");
    }

    // Send a success page back to the browser.
    let response = "HTTP/1.1 200 OK\r\n\
                    Content-Type: text/html\r\n\r\n\
                    <html><body>\
                    <h2>Authorization successful!</h2>\
                    <p>You can close this tab.</p>\
                    </body></html>";
    stream.write_all(response.as_bytes()).await?;

    // 3. Exchange the authorization code for tokens.
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://auth.atlassian.com/oauth/token")
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": client_id,
            "client_secret": client_secret,
            "code": code,
            "redirect_uri": redirect_uri,
        }))
        .send()
        .await?;

    if !token_response.status().is_success() {
        let body = token_response.text().await?;
        anyhow::bail!("Token exchange failed: {body}");
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
    }
    let tokens: TokenResponse = token_response.json().await?;

    // 4. Fetch accessible resources to discover cloud ID and site info.
    #[derive(serde::Deserialize)]
    struct AccessibleResource {
        id: String,
        url: String,
        name: String,
    }
    let resources: Vec<AccessibleResource> = client
        .get("https://api.atlassian.com/oauth/token/accessible-resources")
        .bearer_auth(&tokens.access_token)
        .send()
        .await?
        .json()
        .await?;

    let resource = resources
        .first()
        .ok_or_else(|| anyhow::anyhow!("No accessible Jira sites found"))?;

    // 5. Store tokens in the system keychain.
    store_oauth_tokens(&tokens.access_token, &tokens.refresh_token)?;

    Ok(OAuthResult {
        cloud_id: resource.id.clone(),
        site_url: resource.url.clone(),
        site_name: resource.name.clone(),
    })
}

/// Refresh the OAuth 2.0 access token using the stored refresh token.
/// Returns the new access token on success.
pub async fn refresh_oauth_token(client_id: &str, client_secret: &str) -> Result<String> {
    let (_, refresh_token) = load_oauth_tokens()?;

    let client = reqwest::Client::new();
    let response = client
        .post("https://auth.atlassian.com/oauth/token")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": client_id,
            "client_secret": client_secret,
            "refresh_token": refresh_token,
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Token refresh failed. Run \"jr auth login\" to re-authenticate.");
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
    }
    let tokens: TokenResponse = response.json().await?;
    store_oauth_tokens(&tokens.access_token, &tokens.refresh_token)?;
    Ok(tokens.access_token)
}

/// Generate a unique state parameter for CSRF protection.
fn generate_state() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{:x}", t.as_nanos())
}

/// Extract a query parameter value from a raw HTTP request string.
fn extract_query_param(request: &str, param: &str) -> Option<String> {
    let query_start = request.find('?')?;
    let query_end = request[query_start..]
        .find(' ')
        .map(|i| query_start + i)
        .unwrap_or(request.len());
    let query = &request[query_start + 1..query_end];
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            if key == param {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_query_param_found() {
        let request = "GET /callback?code=abc123&state=xyz HTTP/1.1\r\n";
        assert_eq!(
            extract_query_param(request, "code"),
            Some("abc123".to_string())
        );
        assert_eq!(
            extract_query_param(request, "state"),
            Some("xyz".to_string())
        );
    }

    #[test]
    fn test_extract_query_param_not_found() {
        let request = "GET /callback?code=abc123 HTTP/1.1\r\n";
        assert_eq!(extract_query_param(request, "state"), None);
    }

    #[test]
    fn test_extract_query_param_no_query() {
        let request = "GET /callback HTTP/1.1\r\n";
        assert_eq!(extract_query_param(request, "code"), None);
    }

    #[test]
    fn test_generate_state_is_hex() {
        let state = generate_state();
        assert!(!state.is_empty());
        assert!(state.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
