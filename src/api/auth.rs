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
/// Pre-multi-profile flat OAuth keys. Read-only on the migration path inside
/// [`load_oauth_tokens`] for the `"default"` profile; new writes always use
/// the namespaced `<profile>:oauth-*-token` keys.
const KEY_OAUTH_ACCESS_LEGACY: &str = "oauth-access-token";
const KEY_OAUTH_REFRESH_LEGACY: &str = "oauth-refresh-token";

fn oauth_access_key(profile: &str) -> String {
    format!("{profile}:oauth-access-token")
}
fn oauth_refresh_key(profile: &str) -> String {
    format!("{profile}:oauth-refresh-token")
}

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

/// Store OAuth 2.0 access and refresh tokens scoped to a profile.
///
/// Tokens are written to the namespaced keys `<profile>:oauth-access-token`
/// and `<profile>:oauth-refresh-token` so multiple Jira sites can coexist
/// in a single keychain.
pub fn store_oauth_tokens(profile: &str, access: &str, refresh: &str) -> Result<()> {
    entry(&oauth_access_key(profile))?.set_password(access)?;
    entry(&oauth_refresh_key(profile))?.set_password(refresh)?;
    Ok(())
}

/// Load OAuth 2.0 access and refresh tokens for a profile.
///
/// Returns `(access_token, refresh_token)`.
///
/// For the `"default"` profile, falls back to the legacy flat keys
/// (`oauth-access-token` / `oauth-refresh-token`, the pre-multi-profile
/// layout) and opportunistically migrates them to the new namespaced keys
/// on read: writes the namespaced copies, then deletes the legacy ones.
/// This means existing single-profile users transparently survive the
/// upgrade without re-authenticating. Non-`"default"` profiles never
/// inherit legacy keys — that would silently cross-pollinate credentials
/// across distinct Jira sites.
pub fn load_oauth_tokens(profile: &str) -> Result<(String, String)> {
    let access_key = oauth_access_key(profile);
    let refresh_key = oauth_refresh_key(profile);
    let access = read_keyring_optional(&access_key)?;
    let refresh = read_keyring_optional(&refresh_key)?;

    match (access, refresh) {
        (Some(a), Some(r)) => Ok((a, r)),
        (None, None) => {
            // Both namespaced keys absent — try legacy fallback for the
            // "default" profile (lazy-migration path). Non-default
            // profiles never inherit legacy keys; that would silently
            // cross-pollinate credentials across distinct Jira sites.
            if profile == "default" {
                let legacy_access = read_keyring_optional(KEY_OAUTH_ACCESS_LEGACY)?;
                let legacy_refresh = read_keyring_optional(KEY_OAUTH_REFRESH_LEGACY)?;
                if let (Some(a), Some(r)) = (legacy_access, legacy_refresh) {
                    store_oauth_tokens("default", &a, &r)?;
                    let _ = entry(KEY_OAUTH_ACCESS_LEGACY)?.delete_credential();
                    let _ = entry(KEY_OAUTH_REFRESH_LEGACY)?.delete_credential();
                    return Ok((a, r));
                }
            }
            Err(anyhow::anyhow!(
                "No stored OAuth token for profile {profile:?} — \
                 run \"jr auth login --profile {profile}\""
            ))
        }
        // Partial state: one half of the namespaced pair is missing. For
        // the "default" profile, try recovering from a still-intact
        // legacy pair before erroring — this handles interrupted lazy
        // migrations and partial writes that left the namespaced entries
        // inconsistent while the legacy flat keys still contain valid
        // tokens. Non-default profiles must NEVER inherit legacy keys
        // (that would cross-pollinate credentials across Jira sites).
        //
        // If the legacy pair isn't complete either, surface the partial
        // state with explicit recovery instructions rather than masking
        // the corruption with a generic "no token" message.
        _ => {
            if profile == "default" {
                let legacy_access = read_keyring_optional(KEY_OAUTH_ACCESS_LEGACY)?;
                let legacy_refresh = read_keyring_optional(KEY_OAUTH_REFRESH_LEGACY)?;
                if let (Some(a), Some(r)) = (legacy_access, legacy_refresh) {
                    store_oauth_tokens("default", &a, &r)?;
                    let _ = entry(KEY_OAUTH_ACCESS_LEGACY)?.delete_credential();
                    let _ = entry(KEY_OAUTH_REFRESH_LEGACY)?.delete_credential();
                    return Ok((a, r));
                }
            }
            Err(anyhow::anyhow!(
                "OAuth keychain entries for profile {profile:?} are partial \
                 (one of access/refresh present, the other missing). \
                 Run \"jr auth logout --profile {profile}\" then \
                 \"jr auth login --profile {profile}\" to restore a clean state."
            ))
        }
    }
}

/// Read an optional keychain entry, distinguishing "not present" (`NoEntry`)
/// from real backend failures.
///
/// `keyring::Entry::get_password().ok()` collapses every error to `None` —
/// so a permission-denied, locked-keyring, or platform error looks identical
/// to a missing entry. That silently triggers fallbacks (legacy migration,
/// generic "no token" messages) and hides the real problem from the user.
/// This helper instead matches `keyring::Error::NoEntry` as the only
/// "absent" case and propagates everything else up the call stack so the
/// CLI can surface actionable diagnostics.
fn read_keyring_optional(key: &str) -> Result<Option<String>> {
    match entry(key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
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

/// Clear OAuth tokens for a single profile (other profiles + shared keys
/// such as email / api-token / oauth_client_id are untouched).
///
/// For the `"default"` profile, this also deletes the legacy flat OAuth
/// keys (`oauth-access-token` / `oauth-refresh-token`). Without that step,
/// a user mid-migration would see `jr auth logout --profile default`
/// "succeed" while the legacy keys remained — and the next
/// `load_oauth_tokens("default")` would lazy-migrate them back into the
/// namespaced slots, effectively undoing the logout. Non-`"default"`
/// profiles never inherit legacy keys, so this clause stays scoped to
/// `"default"` to avoid stomping on another profile's migration window.
///
/// `NoEntry` results are treated as success (the entry was already absent).
/// Any other failure (permission denied, ACL mismatch, platform error) is
/// aggregated and returned so callers can surface partial-failure details
/// rather than reporting success while stale entries remain.
pub fn clear_profile_creds(profile: &str) -> Result<()> {
    let mut failures: Vec<String> = Vec::new();
    let mut keys: Vec<String> = vec![oauth_access_key(profile), oauth_refresh_key(profile)];
    // For the "default" profile, also clear the legacy flat OAuth keys
    // that load_oauth_tokens("default") would otherwise lazy-migrate
    // back into existence on the next read — defeating logout.
    if profile == "default" {
        keys.push(KEY_OAUTH_ACCESS_LEGACY.to_string());
        keys.push(KEY_OAUTH_REFRESH_LEGACY.to_string());
    }
    for key in keys {
        match entry(&key) {
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
            "failed to clear {} keychain entries: {}",
            failures.len(),
            failures.join("; ")
        ))
    }
}

/// Remove shared credentials and OAuth tokens for every listed profile from
/// the system keychain.
///
/// Always clears the shared / single-tenant keys (`email`, `api-token`,
/// `oauth_client_id`, `oauth_client_secret`) plus the legacy flat OAuth
/// keys. Per-profile OAuth tokens (`<profile>:oauth-*-token`) are cleared
/// only for the profiles in `profiles` — callers know their own profile
/// list (from config) and pass it in.
///
/// `NoEntry` results are treated as success (the entry was already absent,
/// which is the expected case on a fresh install or after a prior clear).
/// Any other failure (permission denied, ACL mismatch, platform error) is
/// aggregated and returned so callers can decide whether to proceed — for
/// example, `jr auth refresh` needs to know if the clear actually happened
/// before reporting the refresh as successful.
pub fn clear_all_credentials(profiles: &[&str]) -> Result<()> {
    let mut failures: Vec<String> = Vec::new();
    let mut keys: Vec<String> = vec![
        KEY_EMAIL.to_string(),
        KEY_API_TOKEN.to_string(),
        "oauth_client_id".to_string(),
        "oauth_client_secret".to_string(),
        KEY_OAUTH_ACCESS_LEGACY.to_string(),
        KEY_OAUTH_REFRESH_LEGACY.to_string(),
    ];
    for profile in profiles {
        keys.push(oauth_access_key(profile));
        keys.push(oauth_refresh_key(profile));
    }
    for key in keys {
        match entry(&key) {
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
            "failed to clear {} keychain entries: {}",
            failures.len(),
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
    profile: &str,
    client_id: &str,
    client_secret: &str,
    scopes: &str,
) -> Result<OAuthResult> {
    // 1. Find an available port for the local callback server.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);

    let redirect_uri = format!("http://localhost:{port}/callback");
    let state = generate_state()?;

    let auth_url = build_authorize_url(client_id, scopes, &redirect_uri, &state);

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
    store_oauth_tokens(profile, &tokens.access_token, &tokens.refresh_token)?;

    Ok(OAuthResult {
        cloud_id: resource.id.clone(),
        site_url: resource.url.clone(),
        site_name: resource.name.clone(),
    })
}

/// Refresh the OAuth 2.0 access token using the stored refresh token.
/// Returns the new access token on success.
///
/// Intentionally takes no `scopes` parameter: the `refresh_token` grant
/// inherits scopes from the original authorization per RFC 6749 §6. To
/// pick up a changed `[profiles.<name>].oauth_scopes` in config.toml,
/// the user must re-run `jr auth login --oauth` (refresh alone will
/// keep the old scope set).
pub async fn refresh_oauth_token(
    profile: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String> {
    let (_, refresh_token) = load_oauth_tokens(profile)?;

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
    store_oauth_tokens(profile, &tokens.access_token, &tokens.refresh_token)?;
    Ok(tokens.access_token)
}

/// Build the Atlassian OAuth 2.0 authorize URL with all dynamic parameters
/// percent-encoded uniformly.
///
/// All four dynamic values (`client_id`, `scopes`, `redirect_uri`, `state`)
/// are passed through `urlencoding::encode`, which applies RFC 3986
/// percent-encoding — spaces become `%20`, not `+`. Atlassian's authorize
/// endpoint requires `%20` for space-separated scopes, NOT the
/// application/x-www-form-urlencoded `+` form that `url::Url::query_pairs_mut`
/// would produce (confirmed against Atlassian's documented example URLs).
///
/// Uniform encoding is a defense-in-depth measure: it prevents a
/// pathological `client_id` containing `&`, `=`, `#`, or `?` from reshaping
/// the query string — e.g., `real_id&redirect_uri=evil.example` becomes
/// `real_id%26redirect_uri%3Devil.example` and is treated as a single
/// scalar value by Atlassian (which then rejects it as an unknown client).
///
/// The static constants (`audience`, `response_type`, `prompt`) are not
/// user-controlled so they are not encoded here.
fn build_authorize_url(client_id: &str, scopes: &str, redirect_uri: &str, state: &str) -> String {
    format!(
        "https://auth.atlassian.com/authorize\
         ?audience=api.atlassian.com\
         &client_id={}\
         &scope={}\
         &redirect_uri={}\
         &state={}\
         &response_type=code\
         &prompt=consent",
        urlencoding::encode(client_id),
        urlencoding::encode(scopes),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(state),
    )
}

/// Generate a cryptographically random state parameter for CSRF protection
/// of the OAuth 2.0 authorization-code flow (RFC 6749 §10.12).
///
/// 32 random bytes read directly from the operating system CSPRNG via
/// `rand::rngs::OsRng` (which is a thin wrapper over the `getrandom` crate
/// and calls `getrandom(2)` / `BCryptGenRandom` on each invocation — no
/// user-space reseeding state, unlike `rand::rng()` / `ThreadRng`).
/// Rendered as 64 hex characters. 256 bits of entropy far exceeds the
/// ~30 bits offered by the previous wall-clock-nanosecond implementation,
/// closing the attack window where an attacker with local access could
/// observe the authorize URL and race the 127.0.0.1 callback listener
/// with a forged code.
///
/// Returns `Err` when the OS CSPRNG is unavailable — a rare but non-
/// panicking failure mode (sandboxed environments without `/dev/urandom`,
/// early-boot situations, or OS-level seccomp denials). The caller
/// bubbles this up through `oauth_login` so `jr auth login` fails with
/// an actionable error rather than aborting the process (the release
/// profile uses `panic = "abort"`).
fn generate_state() -> Result<String> {
    use rand::TryRngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.try_fill_bytes(&mut bytes).context(
        "Failed to read from OS CSPRNG when generating OAuth state. \
         Check OS entropy availability or sandbox/seccomp restrictions \
         that may block getrandom(2) / BCryptGenRandom.",
    )?;
    Ok(bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
        s
    }))
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
        let state = generate_state().expect("OS CSPRNG available in tests");
        assert!(!state.is_empty());
        assert!(state.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// 256-bit CSPRNG output rendered as hex must always be 64 characters.
    /// Pinning the length guards against a regression to any lower-entropy
    /// source (e.g., timestamp-hex, truncated UUIDs) that would still pass
    /// the is_hex check.
    #[test]
    fn test_generate_state_is_64_hex_chars() {
        let state = generate_state().expect("OS CSPRNG available in tests");
        assert_eq!(
            state.len(),
            64,
            "expected 32 bytes = 64 hex chars, got: {state}"
        );
    }

    /// `generate_state` must produce 8 distinct values across 8 calls. A
    /// deterministic or low-entropy regression (reintroduced `as_nanos`
    /// state, a constant, etc.) collapses outputs and trips this check.
    /// With 256 bits of true entropy the birthday-bound collision
    /// probability across 8 samples is C(8,2) / 2^256 ≈ 2^-253, so
    /// requiring all 8 to be distinct is rigorously not a flake source.
    #[test]
    fn test_generate_state_is_not_deterministic() {
        let samples: std::collections::HashSet<String> = (0..8)
            .map(|_| generate_state().expect("OS CSPRNG available in tests"))
            .collect();
        assert_eq!(
            samples.len(),
            8,
            "expected 8 distinct values from 8 generate_state() calls, \
             got {} distinct: {samples:?}",
            samples.len()
        );
    }

    /// Happy path: a well-formed `client_id` + scopes + redirect_uri + state
    /// produce an authorize URL with all Atlassian-required static params,
    /// scope spaces rendered as `%20` (Atlassian rejects `+`-encoded spaces).
    #[test]
    fn test_build_authorize_url_happy_path() {
        let url = build_authorize_url(
            "normal-client-id",
            "read:jira-work offline_access",
            "http://localhost:12345/callback",
            "deadbeef",
        );

        assert!(url.starts_with("https://auth.atlassian.com/authorize?"));
        assert!(url.contains("audience=api.atlassian.com"));
        assert!(url.contains("&client_id=normal-client-id"));
        assert!(
            url.contains("&scope=read%3Ajira-work%20offline_access"),
            "scope must be %20-encoded, not +-encoded (Atlassian requires %20): {url}"
        );
        assert!(url.contains("&redirect_uri=http%3A%2F%2Flocalhost%3A12345%2Fcallback"));
        assert!(url.contains("&state=deadbeef"));
        assert!(url.contains("&response_type=code"));
        assert!(url.contains("&prompt=consent"));
    }

    /// A pathological `client_id` containing query-string reserved chars
    /// (`&`, `=`, `#`) must be fully escaped so it cannot reshape the query
    /// string. Without uniform encoding, `real_id&redirect_uri=evil.example`
    /// would silently override the redirect_uri parameter.
    #[test]
    fn test_build_authorize_url_escapes_hostile_client_id() {
        let url = build_authorize_url(
            "real_id&redirect_uri=evil.example#frag",
            "read:jira-work",
            "http://localhost:12345/callback",
            "deadbeef",
        );

        assert!(
            !url.contains("&redirect_uri=evil.example"),
            "hostile client_id must not be able to inject a redirect_uri override: {url}"
        );
        assert!(
            url.contains("client_id=real_id%26redirect_uri%3Devil.example%23frag"),
            "client_id reserved chars must be percent-encoded: {url}"
        );
    }

    /// Scope values containing `+` (unlikely but not impossible — some
    /// granular scopes are under evolution) must have the `+` escaped to
    /// `%2B`. Unescaped `+` in a form-urlencoded context means "space",
    /// which would silently corrupt the scope list.
    #[test]
    fn test_build_authorize_url_escapes_plus_in_scope() {
        let url = build_authorize_url(
            "client",
            "scope:with+plus",
            "http://localhost:12345/callback",
            "deadbeef",
        );

        assert!(
            url.contains("scope=scope%3Awith%2Bplus"),
            "+ in scope must be encoded as %2B: {url}"
        );
        assert!(
            !url.contains("scope:with+plus"),
            "raw + must not appear in the URL: {url}"
        );
    }

    fn unique_test_service() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("jr-jira-cli-test-{}-{}", std::process::id(), n)
    }

    /// Serializes JR_SERVICE_NAME mutation across concurrent keyring tests so
    /// no test observes a service name set by another in-flight test (which
    /// would point its keychain operations at the wrong namespace).
    static KEYRING_TEST_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Wrap a test in a unique JR_SERVICE_NAME scope so concurrent tests don't collide.
    fn with_test_keyring<F: FnOnce()>(f: F) {
        if std::env::var("JR_RUN_KEYRING_TESTS").is_err() {
            return;
        }
        // Hold the mutex across env mutation + body + cleanup so no other
        // `with_test_keyring` invocation can race the JR_SERVICE_NAME
        // set/unset and observe a half-applied state. Recover from a
        // poisoned lock — a panicking test still leaves the env in a
        // recoverable state because we restore JR_SERVICE_NAME at scope
        // exit, and a unique service-name namespace per call already
        // isolates keychain entries.
        let _guard = KEYRING_TEST_ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let svc = unique_test_service();
        let prev = std::env::var("JR_SERVICE_NAME").ok();
        // SAFETY: KEYRING_TEST_ENV_MUTEX is held for the duration of this
        // scope, so no other test in this binary can race the env mutation.
        // The opt-in `JR_RUN_KEYRING_TESTS` gate further keeps these tests
        // off the default test path.
        unsafe { std::env::set_var("JR_SERVICE_NAME", &svc) };
        f();
        let _ = clear_all_credentials(&["default", "sandbox"]);
        // SAFETY: still holding KEYRING_TEST_ENV_MUTEX.
        unsafe {
            match prev {
                Some(p) => std::env::set_var("JR_SERVICE_NAME", p),
                None => std::env::remove_var("JR_SERVICE_NAME"),
            }
        }
    }

    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn store_and_load_per_profile_oauth_tokens_round_trip() {
        with_test_keyring(|| {
            store_oauth_tokens("default", "access1", "refresh1").unwrap();
            store_oauth_tokens("sandbox", "access2", "refresh2").unwrap();

            let (a1, r1) = load_oauth_tokens("default").unwrap();
            let (a2, r2) = load_oauth_tokens("sandbox").unwrap();

            assert_eq!((a1.as_str(), r1.as_str()), ("access1", "refresh1"));
            assert_eq!((a2.as_str(), r2.as_str()), ("access2", "refresh2"));
        });
    }

    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_oauth_tokens_returns_err_for_missing_profile() {
        with_test_keyring(|| {
            assert!(load_oauth_tokens("default").is_err());
        });
    }

    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn lazy_migration_legacy_flat_keys_for_default_profile() {
        with_test_keyring(|| {
            entry("oauth-access-token")
                .unwrap()
                .set_password("legacy-access")
                .unwrap();
            entry("oauth-refresh-token")
                .unwrap()
                .set_password("legacy-refresh")
                .unwrap();

            let (access, refresh) = load_oauth_tokens("default").unwrap();
            assert_eq!(access, "legacy-access");
            assert_eq!(refresh, "legacy-refresh");

            let new_access = entry("default:oauth-access-token")
                .unwrap()
                .get_password()
                .unwrap();
            assert_eq!(new_access, "legacy-access");

            assert!(entry("oauth-access-token").unwrap().get_password().is_err());
        });
    }

    /// Regression: `clear_profile_creds("default")` must also remove the
    /// legacy flat OAuth keys. Otherwise `jr auth logout --profile default`
    /// leaves the legacy entries in place and the next `load_oauth_tokens`
    /// call resurrects them via the lazy-migration path — silently undoing
    /// the logout for a user mid-migration.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn clear_profile_creds_default_also_clears_legacy_flat_keys() {
        with_test_keyring(|| {
            // Pre-seed legacy flat keys.
            entry(KEY_OAUTH_ACCESS_LEGACY)
                .unwrap()
                .set_password("legacy-access")
                .unwrap();
            entry(KEY_OAUTH_REFRESH_LEGACY)
                .unwrap()
                .set_password("legacy-refresh")
                .unwrap();

            clear_profile_creds("default").unwrap();

            // Legacy keys must be gone — otherwise lazy migration would
            // resurrect them on the next load_oauth_tokens call.
            assert!(
                entry(KEY_OAUTH_ACCESS_LEGACY)
                    .unwrap()
                    .get_password()
                    .is_err()
            );
            assert!(
                entry(KEY_OAUTH_REFRESH_LEGACY)
                    .unwrap()
                    .get_password()
                    .is_err()
            );
        });
    }

    /// Companion to the test above: clearing a non-default profile must NOT
    /// touch the legacy flat keys, since those belong to the `"default"`
    /// profile's lazy-migration window.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn clear_profile_creds_non_default_leaves_legacy_keys_alone() {
        with_test_keyring(|| {
            entry(KEY_OAUTH_ACCESS_LEGACY)
                .unwrap()
                .set_password("legacy-access")
                .unwrap();

            clear_profile_creds("sandbox").unwrap();

            // Legacy keys belong to the "default" profile's lazy migration;
            // logging out of "sandbox" must not touch them.
            let access = entry(KEY_OAUTH_ACCESS_LEGACY)
                .unwrap()
                .get_password()
                .unwrap();
            assert_eq!(access, "legacy-access");
        });
    }

    /// Regression: `load_oauth_tokens` must distinguish (None, None) from
    /// partial state (Some, None) / (None, Some). A pair lookup that
    /// retried via the legacy fallback on partial state would either
    /// silently resurrect a stale legacy pair or return the generic
    /// "no token" error — both of which hide data loss / corruption.
    /// Partial state should surface as an explicit error pointing to
    /// logout+login recovery.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_oauth_tokens_errors_on_partial_state() {
        with_test_keyring(|| {
            // Pre-seed only the access key (missing refresh).
            entry(&oauth_access_key("sandbox"))
                .unwrap()
                .set_password("access-only")
                .unwrap();

            let result = load_oauth_tokens("sandbox");
            let err = result.expect_err("partial state should error");
            let msg = format!("{err:#}");
            assert!(msg.contains("partial"), "got: {msg}");
        });
    }

    /// Edge case: an interrupted lazy migration could leave the namespaced
    /// pair in a partial state for the `default` profile while the legacy
    /// flat keys still hold a complete pair. `load_oauth_tokens("default")`
    /// should recover from the intact legacy pair rather than stranding
    /// users with a partial-state error.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_oauth_tokens_default_partial_recovers_from_legacy() {
        with_test_keyring(|| {
            // Partial namespaced state for the default profile.
            entry(&oauth_access_key("default"))
                .unwrap()
                .set_password("stale-partial")
                .unwrap();
            // Complete legacy pair.
            entry(KEY_OAUTH_ACCESS_LEGACY)
                .unwrap()
                .set_password("legacy-access")
                .unwrap();
            entry(KEY_OAUTH_REFRESH_LEGACY)
                .unwrap()
                .set_password("legacy-refresh")
                .unwrap();

            let (a, r) = load_oauth_tokens("default").unwrap();
            assert_eq!(a, "legacy-access");
            assert_eq!(r, "legacy-refresh");

            // The recovered legacy values overwrote the namespaced pair
            // (both halves now match the legacy tokens).
            let recovered_access = entry(&oauth_access_key("default"))
                .unwrap()
                .get_password()
                .unwrap();
            let recovered_refresh = entry(&oauth_refresh_key("default"))
                .unwrap()
                .get_password()
                .unwrap();
            assert_eq!(recovered_access, "legacy-access");
            assert_eq!(recovered_refresh, "legacy-refresh");

            // Legacy flat keys cleaned up after migration.
            assert!(
                entry(KEY_OAUTH_ACCESS_LEGACY)
                    .unwrap()
                    .get_password()
                    .is_err()
            );
            assert!(
                entry(KEY_OAUTH_REFRESH_LEGACY)
                    .unwrap()
                    .get_password()
                    .is_err()
            );
        });
    }

    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn lazy_migration_does_not_fire_for_non_default_profile() {
        with_test_keyring(|| {
            entry("oauth-access-token")
                .unwrap()
                .set_password("legacy-access")
                .unwrap();
            entry("oauth-refresh-token")
                .unwrap()
                .set_password("legacy-refresh")
                .unwrap();

            assert!(
                load_oauth_tokens("sandbox").is_err(),
                "sandbox profile should NOT inherit legacy keys"
            );
        });
    }
}
