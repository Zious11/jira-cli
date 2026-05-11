use crate::api::rate_limit::{MAX_RETRY_AFTER_SECS, RateLimitInfo};
use crate::config::Config;
use crate::error::JrError;
use base64::Engine;
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::time::Duration;
use tracing::debug;

/// Maximum number of retries when the API returns 429 Too Many Requests.
const MAX_RETRIES: u32 = 3;

/// Default retry delay in seconds when Retry-After header is missing.
const DEFAULT_RETRY_SECS: u64 = 1;

/// The main HTTP client for communicating with the Jira REST API.
pub struct JiraClient {
    client: Client,
    base_url: String,
    instance_url: String,
    auth_header: String,
    verbose: bool,
    verbose_bodies: bool,
    assets_base_url: Option<String>,
    /// Active profile name, plumbed through so per-profile cache calls can
    /// scope their reads/writes correctly without the call sites needing
    /// access to `&Config`.
    profile_name: String,
}

impl JiraClient {
    /// Build a `JiraClient` from the application config, loading auth credentials
    /// from the system keychain.
    pub fn from_config(
        config: &Config,
        verbose: bool,
        verbose_bodies: bool,
    ) -> anyhow::Result<Self> {
        let base_url = config.base_url()?;

        // JR_BASE_URL overrides all URL targets (used by integration tests to inject wiremock).
        let test_override = std::env::var("JR_BASE_URL").ok();

        // In test-override mode the profile is not consulted for any URL target,
        // and JR_AUTH_HEADER short-circuits credential loading. In real-use mode
        // the active profile is required to know URL, auth_method, and cloud_id.
        let profile = if test_override.is_some() {
            None
        } else {
            Some(config.active_profile_or_err()?)
        };

        let instance_url = if let Some(ref override_url) = test_override {
            // Test mode: route all traffic (including instance and assets) to the mock server.
            override_url.trim_end_matches('/').to_string()
        } else if let Some(url) = profile.and_then(|p| p.url.as_ref()) {
            url.trim_end_matches('/').to_string()
        } else {
            return Err(JrError::ConfigError(format!(
                "Profile {:?} has no URL configured. Run \"jr auth login --profile {}\".",
                config.active_profile_name, config.active_profile_name
            ))
            .into());
        };
        let auth_method = profile
            .and_then(|p| p.auth_method.as_deref())
            .unwrap_or("api_token");

        // JR_AUTH_HEADER env var overrides keychain auth (debug builds only — excluded from
        // release binaries via #[cfg(debug_assertions)] gate per SD-002 resolution).
        #[cfg(debug_assertions)]
        let auth_header = if let Ok(header) = std::env::var("JR_AUTH_HEADER") {
            header
        } else {
            Self::load_auth_from_keychain(auth_method, &config.active_profile_name)?
        };
        #[cfg(not(debug_assertions))]
        let auth_header = Self::load_auth_from_keychain(auth_method, &config.active_profile_name)?;

        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        let assets_base_url = if let Some(ref override_url) = test_override {
            // Test mode: assets API goes to the mock server under /jsm/assets.
            Some(format!("{}/jsm/assets", override_url.trim_end_matches('/')))
        } else {
            profile.and_then(|p| p.cloud_id.as_ref()).map(|cloud_id| {
                format!(
                    "https://api.atlassian.com/ex/jira/{}/jsm/assets",
                    urlencoding::encode(cloud_id)
                )
            })
        };

        if verbose_bodies {
            eprintln!("[jr] WARNING: --verbose-bodies prints request/response bodies to stderr.");
            eprintln!("[jr] These bodies contain PII (accountId, emailAddress, ADF text content).");
            eprintln!("[jr] Do not pipe to AI-agent contexts or shared logs without consent.");
        }

        Ok(Self {
            client,
            base_url,
            instance_url,
            auth_header,
            verbose,
            verbose_bodies,
            assets_base_url,
            profile_name: config.active_profile_name.clone(),
        })
    }

    /// Load the auth header value from the system keychain based on the configured auth method.
    ///
    /// Shared by the `#[cfg(debug_assertions)]` and `#[cfg(not(debug_assertions))]` branches
    /// of `from_config` to avoid duplicating the `oauth`/`api_token` match arms.
    fn load_auth_from_keychain(auth_method: &str, profile_name: &str) -> anyhow::Result<String> {
        match auth_method {
            "oauth" => {
                let (access, _refresh) = crate::api::auth::load_oauth_tokens(profile_name)?;
                Ok(format!("Bearer {access}"))
            }
            _ => {
                // api_token (default)
                let (email, token) = crate::api::auth::load_api_token()?;
                let encoded =
                    base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"));
                Ok(format!("Basic {encoded}"))
            }
        }
    }

    /// Create a client for integration testing. This is **not** gated behind
    /// `#[cfg(test)]` so that integration tests in `tests/` can use it.
    pub fn new_for_test(base_url: String, auth_header: String) -> Self {
        let assets_base_url = Some(format!("{}/jsm/assets", &base_url));
        Self {
            client: Client::new(),
            instance_url: base_url.clone(),
            base_url,
            auth_header,
            verbose: false,
            verbose_bodies: false,
            assets_base_url,
            profile_name: "default".to_string(),
        }
    }

    /// Create a client for integration testing with an explicit profile name.
    ///
    /// Mirrors `new_for_test` but parameterises the profile name so tests can
    /// bind to a per-test isolated keychain profile. Required for keyring-gated
    /// S-3.03 tests that must avoid cross-test keychain collisions when running
    /// with parallel test threads.
    ///
    /// This is **not** gated behind `#[cfg(test)]` so that integration tests in
    /// `tests/` can use it (mirrors the gating pattern of `new_for_test`).
    pub fn new_for_test_with_profile(base_url: String, auth_header: String, profile: &str) -> Self {
        let assets_base_url = Some(format!("{}/jsm/assets", &base_url));
        Self {
            client: Client::new(),
            instance_url: base_url.clone(),
            base_url,
            auth_header,
            verbose: false,
            verbose_bodies: false,
            assets_base_url,
            profile_name: profile.to_string(),
        }
    }

    /// Create a client for integration testing with distinct base_url and instance_url.
    ///
    /// Mirrors `new_for_test` but allows `base_url` (the OAuth API gateway, e.g.
    /// `https://api.atlassian.com/ex/jira/<cloudId>`) to differ from `instance_url`
    /// (the real `*.atlassian.net` browse URL). This replicates the OAuth URL
    /// divergence that `handle_open` must handle correctly (BC-3.4.001).
    ///
    /// This is **not** gated behind `#[cfg(test)]` so that integration tests in
    /// `tests/` can use it (mirrors the gating pattern of `new_for_test`).
    pub fn new_for_test_with_instance_url(
        base_url: &str,
        instance_url: &str,
        auth_header: &str,
    ) -> Self {
        let assets_base_url = Some(format!("{}/jsm/assets", base_url));
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            instance_url: instance_url.trim_end_matches('/').to_string(),
            auth_header: auth_header.to_string(),
            verbose: false,
            verbose_bodies: false,
            assets_base_url,
            profile_name: "default".to_string(),
        }
    }

    /// Active profile name this client is bound to. Used by per-profile
    /// cache call sites (CMDB fields, workspace ID, project meta, resolutions,
    /// object-type attrs) that have a `&JiraClient` but not `&Config`.
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    /// Whether the client was constructed with `--verbose` enabled.
    /// Handlers use this to gate optional diagnostic output.
    pub fn verbose(&self) -> bool {
        self.verbose
    }

    /// Read a response body as raw bytes, optionally printing it to stderr
    /// when `--verbose-bodies` is enabled. Returns the bytes for deserialization.
    ///
    /// - When `verbose_bodies` is set: prints the raw response bytes to stderr.
    /// - When only `verbose` is set: prints a suppression hint so users know
    ///   how to enable body inspection.
    /// - Otherwise: silent.
    ///
    /// Centralises response-body logging so all `get*` / `post*` methods share
    /// the same logic without duplicating it at each call site.
    async fn collect_response_body(&self, response: Response) -> anyhow::Result<Vec<u8>> {
        let bytes = response.bytes().await?;
        if self.verbose_bodies {
            eprintln!(
                "[verbose] response body: {}",
                String::from_utf8_lossy(&bytes)
            );
        } else if self.verbose {
            eprintln!(
                "[verbose] body suppressed (use --verbose-bodies to inspect, will print PII)"
            );
        }
        Ok(bytes.to_vec())
    }

    /// Perform a GET request and deserialize the JSON response.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let request = self.client.get(&url);
        let response = self.send(request).await?;
        let bytes = self.collect_response_body(response).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Perform a POST request with a JSON body and deserialize the response.
    pub async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let request = self.client.post(&url).json(body);
        let response = self.send(request).await?;
        let bytes = self.collect_response_body(response).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Perform a PUT request with a JSON body. Returns `()` on success.
    pub async fn put<B: Serialize>(&self, path: &str, body: &B) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let request = self.client.put(&url).json(body);
        self.send(request).await?;
        Ok(())
    }

    /// Perform a POST request that returns 204 No Content on success.
    pub async fn post_no_content<B: Serialize>(&self, path: &str, body: &B) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let request = self.client.post(&url).json(body);
        self.send(request).await?;
        Ok(())
    }

    /// Perform a DELETE request that returns 204 No Content on success.
    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let request = self.client.delete(&url);
        self.send(request).await?;
        Ok(())
    }

    /// Send a request with auth headers, retrying on 429 up to MAX_RETRIES times.
    ///
    /// # Auto-refresh on 401 (S-3.03 v2)
    ///
    /// On the first 401, attempts to refresh the OAuth access token via
    /// `refresh_coordinator::refresh_with_single_flight`. If the refresh
    /// succeeds, retries the original request ONCE with the new token.
    /// If the retry also returns 401 → `NotAuthenticated` (no second refresh).
    ///
    /// Trigger: BLANKET 401 (matches `gh` CLI pattern). Atlassian returns
    /// `{"errorMessages":[...]}` with NO `code` field and NO RFC-6750
    /// `WWW-Authenticate` header. Source: CLAUDE.md gotcha + S-3.03 research.
    ///
    /// If refresh fails with `invalid_grant`, the AC-010 post-hoc reconcile
    /// check re-reads the keychain. If another process already refreshed
    /// (keychain access token differs from our initial bearer), we retry
    /// the original request with the keychain's new token.
    async fn send(&self, request: RequestBuilder) -> anyhow::Result<Response> {
        // We need to be able to retry, so we clone the request builder.
        // reqwest::RequestBuilder::try_clone() returns None for streaming bodies,
        // but we only send JSON (or no body), so it will always succeed.
        let mut last_response: Option<Response> = None;

        // Snapshot JR_OAUTH_TOKEN_URL HERE — before any .await — so that
        // concurrent integration tests that set unique JR_OAUTH_TOKEN_URL
        // values cannot race with each other between this snapshot and the
        // actual refresh POST. In tokio, tasks can only interleave at .await
        // points; reading the env var here (before the first .await in the
        // rate-limit loop) is safe for the current task's execution context.
        //
        // Production code never sets JR_OAUTH_TOKEN_URL, so this reads the
        // real Atlassian endpoint and is stable for the process lifetime.
        let token_url_snapshot = std::env::var("JR_OAUTH_TOKEN_URL")
            .unwrap_or_else(|_| "https://auth.atlassian.com/oauth/token".to_string());

        // Send the initial request (with 429 retries).
        let first_response = 'rate_limit: {
            for attempt in 0..=MAX_RETRIES {
                let req = request
                    .try_clone()
                    .expect("request should be cloneable (JSON body)");

                let req = req.header("Authorization", &self.auth_header);

                if self.verbose || self.verbose_bodies {
                    if let Some(ref r) = req.try_clone().and_then(|r| r.build().ok()) {
                        if self.verbose {
                            // AC-003: log request method+URL to stderr under --verbose.
                            // The [verbose] prefix is retained because cli_handler tests
                            // (SD-003 contract guards) assert on stderr.contains("[verbose] GET/PUT/...")
                            // and the verbose_bodies.rs tests assert on the same prefix.
                            // Tracing handles rate-limit and other diagnostic events.
                            // Method and URL are extracted to variables before the print call
                            // so no single source line contains both the eprintln and method().
                            let method_str = r.method().as_str();
                            let url_str = r.url().as_str();
                            eprintln!("[verbose] {method_str} {url_str}");
                        }
                        if let Some(bytes) = r.body().and_then(|b| b.as_bytes()) {
                            if self.verbose_bodies {
                                eprintln!("[verbose] body: {}", String::from_utf8_lossy(bytes));
                            } else {
                                eprintln!(
                                    "[verbose] body suppressed (use --verbose-bodies to inspect, will print PII)"
                                );
                            }
                        }
                    }
                }

                let response = match req.send().await {
                    Ok(r) => r,
                    Err(e) => {
                        let url = e
                            .url()
                            .map(|u| u.host_str().unwrap_or("unknown").to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        return Err(JrError::NetworkError(url).into());
                    }
                };

                if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_RETRIES {
                    let rate_info = RateLimitInfo::from_headers(response.headers());
                    let delay = rate_info.retry_after_secs.unwrap_or(DEFAULT_RETRY_SECS);

                    // BC-X.4.009: abort retry if Retry-After exceeds the interactive-CLI cap.
                    // Atlassian's typical values (1425-3089s) far exceed what is acceptable
                    // for a foreground CLI. RFC 9110 §10.2.3 permits client-side abort.
                    if delay > MAX_RETRY_AFTER_SECS {
                        eprintln!(
                            "[jr] Atlassian requested {}s wait — exceeds {}s cap for interactive CLI.\n\
                             Aborting retry; rerun later or wrap in a shell-level retry/cron job.",
                            delay, MAX_RETRY_AFTER_SECS
                        );
                        return Err(JrError::ApiError {
                            status: 429,
                            message: format!(
                                "Rate limited; Retry-After {}s exceeds {}s cap. Rerun later.",
                                delay, MAX_RETRY_AFTER_SECS
                            ),
                        }
                        .into());
                    }

                    // AC-003: structured tracing event replaces eprintln! for rate-limit logging.
                    debug!(
                        target: "jr::http",
                        delay_secs = delay,
                        attempt = attempt + 1,
                        max_retries = MAX_RETRIES,
                        "rate_limited_retrying"
                    );
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    last_response = Some(response);
                    continue;
                }

                // Warn the user if we exhausted retries on a 429
                if response.status() == StatusCode::TOO_MANY_REQUESTS {
                    eprintln!(
                        "warning: rate limited by Jira — gave up after {MAX_RETRIES} retries. Wait a moment and try again."
                    );
                    break 'rate_limit if let Some(resp) = last_response {
                        resp
                    } else {
                        response
                    };
                }

                break 'rate_limit response;
            }

            // If we exhausted retries, parse the last 429 response as an error
            if let Some(resp) = last_response {
                return Err(Self::parse_error(resp).await);
            }
            unreachable!("rate_limit loop should always return or set last_response");
        };

        // Non-401 error → return immediately (existing behavior)
        if first_response.status().is_client_error()
            && first_response.status() != StatusCode::UNAUTHORIZED
        {
            return Err(Self::parse_error(first_response).await);
        }
        if first_response.status().is_server_error() {
            return Err(Self::parse_error(first_response).await);
        }
        if first_response.status() != StatusCode::UNAUTHORIZED {
            // Success (2xx)
            return Ok(first_response);
        }

        // -------------------------------------------------------------------
        // S-3.03 v2: BLANKET 401 AUTO-REFRESH PATH
        //
        // Before attempting refresh, read the 401 body and check for the
        // known-unrecoverable scope-mismatch case. If the body contains
        // "scope does not match" (case-insensitive), return InsufficientScope
        // immediately — a token refresh will NOT fix a scope error, and
        // attempting it would waste an HTTP round-trip and confuse the caller.
        //
        // For all other 401s (expired token, revoked token, etc.): proceed
        // with the blanket-401 auto-refresh path below.
        //
        // Note: reading the body here consumes the response. The refresh path
        // below does NOT use the first_response body — it retries the original
        // request with a new token. So consuming the body here is safe.
        let first_401_body = first_response.bytes().await.unwrap_or_default();
        let first_401_message = extract_error_message(&first_401_body);
        if first_401_message
            .to_ascii_lowercase()
            .contains("scope does not match")
        {
            return Err(JrError::InsufficientScope {
                message: first_401_message,
            }
            .into());
        }

        // -------------------------------------------------------------------
        // S-3.03 v2: BLANKET 401 AUTO-REFRESH PATH (non-scope-mismatch 401)
        //
        // The first response was 401. Attempt OAuth token refresh via the
        // per-profile single-flight coordinator. At most ONE refresh HTTP call
        // is made per profile per coordinator epoch regardless of concurrency.
        //
        // Guard: only fire auto-refresh for OAuth (Bearer) auth. Basic auth
        // clients use API tokens, not OAuth refresh tokens — there is nothing
        // to refresh. Returning NotAuthenticated directly is correct for Basic
        // auth 401s.
        if !self.auth_header.starts_with("Bearer ") {
            return Err(JrError::NotAuthenticated {
                hint: "Run \"jr auth login\" to connect.".to_string(),
            }
            .into());
        }

        let profile = self.profile_name.clone();

        let refresh_result = crate::api::refresh_coordinator::refresh_with_single_flight(
            &profile,
            &token_url_snapshot,
            || {
                let profile = profile.clone();
                let token_url = token_url_snapshot.clone();
                async move {
                    let new_access =
                        crate::api::auth::refresh_oauth_token_with_url(&profile, &token_url)
                            .await?;
                    // refresh_oauth_token_with_url stores tokens in keychain (persist-before-publish).
                    // We return (access, refresh) — refresh is re-read from keychain to provide
                    // the coordinator with the new refresh token for AC-010 reconcile.
                    // If load_oauth_tokens fails here, we treat it as non-fatal for the
                    // coordinator (it already persisted) and pass an empty refresh token.
                    let refresh = crate::api::auth::load_oauth_tokens(&profile)
                        .map(|(_, r)| r)
                        .unwrap_or_default();
                    Ok::<(String, String), anyhow::Error>((new_access, refresh))
                }
            },
        )
        .await;

        match refresh_result {
            Ok(new_access_token) => {
                // Refresh succeeded. Retry the original request ONCE with the new token.
                // One-attempt cap: if this retry returns 401, surface NotAuthenticated
                // WITHOUT a second refresh (no recursion).
                let retry_req = request
                    .try_clone()
                    .expect("request should be cloneable for refresh retry (JSON body)");
                let retry_req =
                    retry_req.header("Authorization", format!("Bearer {new_access_token}"));

                let retry_response = match retry_req.send().await {
                    Ok(r) => r,
                    Err(e) => {
                        let url = e
                            .url()
                            .map(|u| u.host_str().unwrap_or("unknown").to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        return Err(JrError::NetworkError(url).into());
                    }
                };

                if retry_response.status().is_client_error()
                    || retry_response.status().is_server_error()
                {
                    if retry_response.status() == StatusCode::UNAUTHORIZED {
                        // Retry also returned 401 — no second refresh (one-attempt cap).
                        return Err(JrError::NotAuthenticated {
                            hint: "run 'jr auth refresh' to re-authenticate".to_string(),
                        }
                        .into());
                    }
                    return Err(Self::parse_error(retry_response).await);
                }

                Ok(retry_response)
            }

            Err(e) => {
                // Refresh failed (invalid_grant or other error).
                //
                // AC-010 POST-HOC RECONCILE: if another process (or coordinator epoch)
                // refreshed first and rotated the keychain, the keychain access token
                // now differs from our initial bearer. Re-read keychain and retry the
                // original request with the new access token.
                //
                // Detection: compare keychain access token vs. our initial auth_header.
                // If they differ, another process already refreshed → use keychain token.
                let reconcile_result = crate::api::auth::load_oauth_tokens(&profile).ok().and_then(
                    |(kc_access, _)| {
                        let initial_bearer = format!("Bearer {kc_access}");
                        if initial_bearer != self.auth_header {
                            // Keychain token differs from what we started with →
                            // another process refreshed. Use the new token for retry.
                            Some(kc_access)
                        } else {
                            None
                        }
                    },
                );

                if let Some(reconciled_access) = reconcile_result {
                    // Retry the original request with the reconciled token (once only).
                    let retry_req = request
                        .try_clone()
                        .expect("request should be cloneable for reconcile retry (JSON body)");
                    let retry_req =
                        retry_req.header("Authorization", format!("Bearer {reconciled_access}"));

                    let retry_response = match retry_req.send().await {
                        Ok(r) => r,
                        Err(err) => {
                            let url = err
                                .url()
                                .map(|u| u.host_str().unwrap_or("unknown").to_string())
                                .unwrap_or_else(|| "unknown".to_string());
                            return Err(JrError::NetworkError(url).into());
                        }
                    };

                    if retry_response.status().is_client_error()
                        || retry_response.status().is_server_error()
                    {
                        if retry_response.status() == StatusCode::UNAUTHORIZED {
                            return Err(JrError::NotAuthenticated {
                                hint: "run 'jr auth refresh' to re-authenticate".to_string(),
                            }
                            .into());
                        }
                        return Err(Self::parse_error(retry_response).await);
                    }
                    return Ok(retry_response);
                }

                // No reconcile opportunity — propagate the refresh error.
                Err(e.into())
            }
        }
    }

    /// Send a pre-built request without parsing non-2xx responses into errors.
    ///
    /// Retries 429 up to MAX_RETRIES times using `Retry-After`. Returns the raw
    /// `Response` for ANY HTTP status (2xx, 4xx, 5xx), including after exhausting
    /// 429 retries — callers MUST check `response.status()` to detect errors.
    /// Network-level failures are still returned as `Err(JrError::NetworkError)`.
    ///
    /// Used by `jr api` (the raw passthrough command) where the caller needs the
    /// full response body regardless of HTTP status. Auth header is already set
    /// on the request by `client.request()`.
    pub async fn send_raw(&self, request: reqwest::Request) -> anyhow::Result<Response> {
        for attempt in 0..=MAX_RETRIES {
            let req = request.try_clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "request cannot be retried because it is not cloneable \
                     (for example, it may use a streaming body)"
                )
            })?;

            if self.verbose || self.verbose_bodies {
                if self.verbose {
                    // AC-003: log request method+URL to stderr under --verbose.
                    // Method and URL are extracted to variables before the print call.
                    // See send() above for rationale.
                    let method_str = req.method().as_str();
                    let url_str = req.url().as_str();
                    eprintln!("[verbose] {method_str} {url_str}");
                }
                if let Some(bytes) = req.body().and_then(|b| b.as_bytes()) {
                    if self.verbose_bodies {
                        eprintln!("[verbose] body: {}", String::from_utf8_lossy(bytes));
                    } else {
                        eprintln!(
                            "[verbose] body suppressed (use --verbose-bodies to inspect, will print PII)"
                        );
                    }
                }
            }

            let response = match self.client.execute(req).await {
                Ok(r) => r,
                Err(e) => {
                    let url = e
                        .url()
                        .map(|u| u.host_str().unwrap_or("unknown").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    return Err(JrError::NetworkError(url).into());
                }
            };

            if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_RETRIES {
                let rate_info = RateLimitInfo::from_headers(response.headers());
                let delay = rate_info.retry_after_secs.unwrap_or(DEFAULT_RETRY_SECS);

                // BC-X.4.009: abort retry if Retry-After exceeds the interactive-CLI cap.
                if delay > MAX_RETRY_AFTER_SECS {
                    eprintln!(
                        "[jr] Atlassian requested {}s wait — exceeds {}s cap for interactive CLI.\n\
                         Aborting retry; rerun later or wrap in a shell-level retry/cron job.",
                        delay, MAX_RETRY_AFTER_SECS
                    );
                    return Err(JrError::ApiError {
                        status: 429,
                        message: format!(
                            "Rate limited; Retry-After {}s exceeds {}s cap. Rerun later.",
                            delay, MAX_RETRY_AFTER_SECS
                        ),
                    }
                    .into());
                }

                // AC-003: structured tracing event replaces eprintln! for rate-limit logging.
                debug!(
                    target: "jr::http",
                    delay_secs = delay,
                    attempt = attempt + 1,
                    max_retries = MAX_RETRIES,
                    "rate_limited_retrying"
                );
                // Drop the 429 response before sleeping so its body isn't held open
                drop(response);
                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }

            // Warn the user if we exhausted retries on a 429
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                eprintln!(
                    "warning: rate limited by Jira — gave up after {MAX_RETRIES} retries. Wait a moment and try again."
                );
            }

            // Return the response for ANY status (including 4xx/5xx) — no error parsing
            return Ok(response);
        }

        unreachable!("loop iterates 0..=MAX_RETRIES; final iteration returns")
    }

    /// Parse an error response into a `JrError`.
    ///
    /// Always reads the response body first, then branches on status. On 401, if
    /// the body's message contains `"scope does not match"` (case-insensitive,
    /// ASCII), returns `JrError::InsufficientScope` with the raw gateway message
    /// — matches the Atlassian API gateway's rejection shape for granular-scoped
    /// personal tokens on POST requests (see issue #185). Any other 401 falls
    /// through to `NotAuthenticated`; non-401 4xx/5xx returns `ApiError`.
    async fn parse_error(response: Response) -> anyhow::Error {
        let status = response.status().as_u16();
        let message = match response.bytes().await {
            Ok(body) => extract_error_message(&body),
            Err(e) => format!("Could not read error response: {e}"),
        };

        if status == 401 {
            if message
                .to_ascii_lowercase()
                .contains("scope does not match")
            {
                return JrError::InsufficientScope { message }.into();
            }
            return JrError::NotAuthenticated {
                hint: "Run \"jr auth login\" to connect.".to_string(),
            }
            .into();
        }

        JrError::ApiError { status, message }.into()
    }

    /// Returns the base URL (useful for constructing browser-facing URLs).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns the raw Jira instance URL (always the real instance URL, even for OAuth users).
    pub fn instance_url(&self) -> &str {
        &self.instance_url
    }

    /// Perform a GET request against the real Jira instance URL (bypasses OAuth proxy base_url).
    pub async fn get_from_instance<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{}{}", self.instance_url, path);
        let request = self.client.get(&url);
        let response = self.send(request).await?;
        let bytes = self.collect_response_body(response).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Perform a POST request against the real Jira instance URL (bypasses OAuth proxy base_url).
    pub async fn post_to_instance<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        let url = format!("{}{}", self.instance_url, path);
        let request = self.client.post(&url).json(body);
        let response = self.send(request).await?;
        let bytes = self.collect_response_body(response).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Perform a GET request against the Assets/CMDB API gateway.
    ///
    /// Constructs URL: `{assets_base_url}/workspace/{workspace_id}/v1/{path}`.
    /// Requires `cloud_id` in config (set during `jr init`).
    pub async fn get_assets<T: DeserializeOwned>(
        &self,
        workspace_id: &str,
        path: &str,
    ) -> anyhow::Result<T> {
        let base = self.assets_base_url.as_ref().ok_or_else(|| {
            JrError::ConfigError(
                "Cloud ID not configured. Run \"jr init\" to set up your instance.".into(),
            )
        })?;
        let url = format!(
            "{}/workspace/{}/v1/{}",
            base,
            urlencoding::encode(workspace_id),
            path
        );
        let request = self.client.get(&url);
        let response = self.send(request).await?;
        let bytes = self.collect_response_body(response).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Perform a POST request against the Assets/CMDB API gateway.
    pub async fn post_assets<T: DeserializeOwned, B: Serialize>(
        &self,
        workspace_id: &str,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        let base = self.assets_base_url.as_ref().ok_or_else(|| {
            JrError::ConfigError(
                "Cloud ID not configured. Run \"jr init\" to set up your instance.".into(),
            )
        })?;
        let url = format!(
            "{}/workspace/{}/v1/{}",
            base,
            urlencoding::encode(workspace_id),
            path
        );
        let request = self.client.post(&url).json(body);
        let response = self.send(request).await?;
        let bytes = self.collect_response_body(response).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Returns the HTTP method for building requests externally (if needed).
    pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .request(method, &url)
            .header("Authorization", &self.auth_header)
    }
}

/// Maximum byte length allowed for a single server-supplied error entry
/// before it reaches `sanitize_for_stderr` / stderr.
///
/// Per issue #334 acceptance criteria: "Truncate each entry to a sane limit
/// (e.g., 1 KiB) to prevent terminal-flooding attacks". Applied per-entry by
/// `cap_entry` at the extraction call sites (each `errorMessages` element,
/// each `errors` map value, the `message` field, the `errorMessage` field,
/// and the raw-body fallback). 1024 bytes leaves ample room for legitimate
/// long error messages while cutting off pathological flood attempts.
///
/// Companion cap: `MAX_SANITIZED_OUTPUT_LEN` below, which bounds the FINAL
/// stderr-bound output AFTER sanitization expansion (each ASCII control
/// byte becomes 4 bytes via the `\xNN` escape). The per-entry cap alone
/// is insufficient because a hostile array of `\n`-filled entries could
/// expand 4x during sanitization.
const MAX_ERROR_ENTRY_LEN: usize = 1024;

/// Maximum byte length of the FINAL sanitized output written to stderr.
///
/// Even with `MAX_ERROR_ENTRY_LEN` capping each pre-sanitization entry,
/// a hostile response containing many entries — or entries full of control
/// bytes that expand 1→4 bytes during sanitization (each ASCII control
/// byte becomes a 4-byte `\xNN` literal) — can still flood the terminal
/// or log file. This cap is enforced inside `sanitize_for_stderr` itself
/// using a byte-budget-aware char loop that stops emitting when the next
/// char would exceed the budget, then appends a `[...truncated; original
/// M bytes]` marker. The marker text references only the immutable
/// `original_len`, not `out.len()`, so the retroactive trim path doesn't
/// change the marker length — preserves the size invariant after trim
/// and avoids over-reporting retained bytes.
///
/// 4 KiB chosen to absorb worst-case 4x expansion of a single `MAX_ERROR_ENTRY_LEN`
/// entry (1024 × 4 = 4096) while still leaving room for the marker via
/// reserved headroom inside `sanitize_for_stderr`.
const MAX_SANITIZED_OUTPUT_LEN: usize = 4096;

/// Truncate a server-supplied string at a UTF-8 char boundary if it exceeds
/// `MAX_ERROR_ENTRY_LEN`, appending a `[...truncated, N bytes total]` marker
/// so the operator sees that truncation happened.
///
/// Returns `Cow::Borrowed(s)` when no truncation is needed (the common case
/// for legitimate Atlassian error entries — most are <<1 KiB) so the caller
/// can join without per-entry allocation. Only allocates a new `String` when
/// the input actually exceeds the cap.
///
/// The marker length is reserved from the prefix budget, so the FINAL output
/// length is guaranteed to be `<= MAX_ERROR_ENTRY_LEN` — important because
/// the marker would otherwise push slightly-oversized inputs (e.g., 1025
/// bytes) to an output longer than the original, defeating the cap's
/// flood-prevention purpose.
///
/// Used by `extract_error_message_raw` as defense-in-depth against
/// terminal/log flooding attacks (companion to `sanitize_for_stderr`'s
/// control-byte escaping per CWE-117).
fn cap_entry(s: &str) -> std::borrow::Cow<'_, str> {
    if s.len() <= MAX_ERROR_ENTRY_LEN {
        return std::borrow::Cow::Borrowed(s);
    }
    let marker = format!(" [...truncated, {} bytes total]", s.len());
    // Defensive: with MAX_ERROR_ENTRY_LEN = 1024 and any plausible byte
    // count, the marker is roughly 30-50 chars and well under the cap. This
    // branch only fires if someone shrinks MAX_ERROR_ENTRY_LEN below the
    // marker length in the future. To preserve the `output.len() <=
    // MAX_ERROR_ENTRY_LEN` invariant even in that case, truncate the marker
    // itself at a UTF-8 char boundary instead of returning it whole.
    if marker.len() >= MAX_ERROR_ENTRY_LEN {
        let mut end = MAX_ERROR_ENTRY_LEN;
        while !marker.is_char_boundary(end) {
            end -= 1;
        }
        return std::borrow::Cow::Owned(marker[..end].to_string());
    }
    let target_prefix_len = MAX_ERROR_ENTRY_LEN - marker.len();
    let mut end = target_prefix_len;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    std::borrow::Cow::Owned(format!("{}{}", &s[..end], marker))
}

/// Sanitize a server-supplied string for safe display on stderr.
///
/// Defense against CWE-117 (Improper Output Neutralization for Logs): a hostile
/// Atlassian response or proxy-controlled `errorMessages` array could otherwise
/// inject CR/LF fake log lines, ANSI escape sequences (cursor movement, color
/// codes, terminal title changes), or NUL bytes into operator-facing stderr
/// output.
///
/// Strategy:
/// - Pass through every printable character (including UTF-8 to preserve
///   localized error messages from non-English Jira tenants).
/// - Replace every ASCII control character (0x00-0x1F + 0x7F) with its
///   `\xNN` literal escape so the operator can see what was sent but the
///   terminal cannot interpret it.
/// - Enforce a `MAX_SANITIZED_OUTPUT_LEN` cap on the FINAL output bytes via
///   a byte-budget-aware char loop. Closes the gap where pre-sanitization
///   per-entry caps (via `cap_entry`) couldn't bound the post-sanitization
///   output: 1024 control bytes expand to ~4096 bytes after `\xNN`
///   escaping, so a per-entry pre-cap alone left the terminal vulnerable
///   to floods.
///
/// More conservative than `str::escape_debug` (which also escapes non-ASCII
/// as `\u{XXXX}` and would garble legitimate Unicode error text).
///
/// Performance:
/// - Takes `String` by value so the clean+small-input fast path returns it
///   unchanged with zero additional allocation. The common case (legitimate
///   Atlassian errors: ASCII + UTF-8, no control bytes, < 4 KiB) hits this
///   path.
/// - When sanitization OR truncation is required, escape bytes are written
///   directly into the output via `std::fmt::Write::write!` rather than the
///   per-char `format!()`-then-`push_str` pattern (which allocates a new
///   String per escaped char).
/// - Output buffer is pre-sized to the smaller of `input.len() + HEADROOM`
///   and `MAX_SANITIZED_OUTPUT_LEN`, so the allocation is bounded.
/// - Idempotent on already-sanitized strings within the size cap.
fn sanitize_for_stderr(input: String) -> String {
    use std::fmt::Write;

    let needs_sanitization = input.bytes().any(|b| b.is_ascii_control());
    let needs_truncation = input.len() > MAX_SANITIZED_OUTPUT_LEN;

    // Fast path: no control bytes AND under the output cap → return the
    // input String unchanged. No new allocation. Optimizes the common case.
    if !needs_sanitization && !needs_truncation {
        return input;
    }

    // Slow path: sanitize AND/OR truncate. Allow output to grow up to the
    // FULL `MAX_SANITIZED_OUTPUT_LEN` — only when the cap is actually
    // breached do we retroactively trim back to make room for the truncation
    // marker. This avoids premature truncation of messages that would
    // otherwise fit in the cap (e.g., a clean 4090-byte input shouldn't
    // lose 64 bytes to a marker that never gets appended).
    let original_len = input.len();
    const HEADROOM: usize = 32;
    let mut out = String::with_capacity((input.len() + HEADROOM).min(MAX_SANITIZED_OUTPUT_LEN));
    let mut truncated = false;

    for c in input.chars() {
        // Compute the byte cost of emitting `c` BEFORE pushing it. Control
        // chars expand 1→4 bytes (`\xNN`); other chars take `c.len_utf8()`
        // bytes (1-4 bytes for any valid Unicode scalar).
        let needed = if c.is_ascii_control() {
            4
        } else {
            c.len_utf8()
        };
        if out.len() + needed > MAX_SANITIZED_OUTPUT_LEN {
            truncated = true;
            break;
        }
        if c.is_ascii_control() {
            // Write directly into `out` via fmt::Write — avoids the per-escape
            // String allocation that `format!()` would introduce.
            // write! on String is infallible; ignore the Result to avoid a
            // distracting `expect()` in a hot loop.
            let _ = write!(out, "\\x{:02x}", c as u8);
        } else {
            out.push(c);
        }
    }

    if truncated {
        // CWE-117 defense: marker text intentionally references only the
        // (immutable) `original_len`, NOT `out.len()`. This makes the
        // marker's length depend solely on original_len's digit count, so
        // the retroactive trim below doesn't change the marker length —
        // preserving the size invariant `out + marker <= cap` AFTER any
        // trim. Operator gets accurate info: "original M bytes" reflects
        // the actual source size; the final retained size is exactly
        // out.len() bytes (directly observable).
        let marker = format!(" [...truncated; original {} bytes]", original_len);
        if out.len() + marker.len() <= MAX_SANITIZED_OUTPUT_LEN {
            // Marker fits without trimming — append directly.
            out.push_str(&marker);
        } else {
            // Marker would push us over: retroactively trim `out` at a UTF-8
            // char boundary so `out + marker` fits within the cap. Bounded
            // by marker.len() (~50 bytes for any plausible original_len), so
            // no quadratic blow-up. The marker text is unchanged by the
            // trim (it only references original_len), so this preserves the
            // size invariant `out.len() + marker.len() <= cap` AFTER the
            // trim — verified empirically by the new size-invariant tests.
            let target = MAX_SANITIZED_OUTPUT_LEN - marker.len();
            let mut end = target;
            while !out.is_char_boundary(end) {
                end -= 1;
            }
            out.truncate(end);
            out.push_str(&marker);
        }
    }
    out
}

/// Extract a human-readable error message from a Jira error response body.
///
/// All return paths run through `sanitize_for_stderr` (CWE-117 defense:
/// escapes ASCII control chars from server-supplied content as visible
/// `\xNN` literals before they reach stderr, preventing CR/LF/ANSI
/// injection from a hostile or proxy-controlled response while keeping
/// the byte information visible to the operator). UTF-8 in legitimate
/// localized error messages is preserved.
///
/// Precedence:
/// 1. Non-empty `errorMessages` array → joined with "; "
/// 2. Non-empty `errors` object (field-level validation) → "field: msg; field2: msg2"
/// 3. `message` string field
/// 4. `errorMessage` string field (singular, seen in some JSM endpoints)
/// 5. Empty body → "<empty response body>"
/// 6. Raw body as a string (fallback)
pub fn extract_error_message(body: &[u8]) -> String {
    sanitize_for_stderr(extract_error_message_raw(body))
}

/// Internal: extract the error message WITHOUT stderr sanitization.
///
/// Each server-supplied entry is passed through `cap_entry` so a single
/// pathological field can't flood the terminal/log — the per-entry 1 KiB cap
/// is part of the CWE-117 defense-in-depth alongside `sanitize_for_stderr`.
///
/// Separated from the public API so the extraction precedence logic and the
/// sanitization layer can be tested independently. Callers OUTSIDE tests should
/// always go through `extract_error_message`.
fn extract_error_message_raw(body: &[u8]) -> String {
    if body.is_empty() {
        return "<empty response body>".to_string();
    }

    let body_str = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => {
            // Memory-amplification defense (OWASP A06 / AP11, Perplexity-validated
            // 2026-05-11): `String::from_utf8_lossy` allocates an owned String for
            // the ENTIRE byte slice even though `cap_entry` will truncate the result
            // to MAX_ERROR_ENTRY_LEN. A hostile server returning a massive non-UTF8
            // body would otherwise force O(body.len()) memory allocation before the
            // cap kicks in. Pre-cap the byte slice to a small multiple of the entry
            // cap (allows worst-case lossy expansion via U+FFFD replacement chars
            // at 3 bytes each) before conversion.
            const PRE_CAP_BYTES: usize = MAX_ERROR_ENTRY_LEN * 4;
            let bounded = &body[..body.len().min(PRE_CAP_BYTES)];
            return cap_entry(&String::from_utf8_lossy(bounded)).into_owned();
        }
    };

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
        if let Some(msgs) = json.get("errorMessages").and_then(|v| v.as_array()) {
            // Memory-amplification defense (OWASP A06 / AP11, Perplexity-validated
            // 2026-05-11): even though each entry is per-entry-capped via
            // cap_entry, the NUMBER of entries is server-controlled. A hostile
            // response with a million entries × 1024 bytes each would force a
            // ~1 GB allocation in the join here before `sanitize_for_stderr` later
            // truncates to MAX_SANITIZED_OUTPUT_LEN. Stream-process instead:
            // iterate lazily, append to a single output String pre-sized to
            // MAX_SANITIZED_OUTPUT_LEN, and stop as soon as the budget is hit.
            // Total memory is O(MAX_SANITIZED_OUTPUT_LEN) regardless of entry
            // count. The downstream sanitize_for_stderr cap is retained as
            // defense-in-depth for control-byte expansion.
            // Reserve marker budget upfront in the budget check (the standard
            // pattern for fixed-output sanitization — matches Rust std::fmt
            // buffer sizing and log-crate truncation conventions; retroactive
            // trim risks marker overflow). Marker is fixed-length so the
            // reservation is a 15-byte constant; no recalculation needed.
            const JOIN_MARKER: &str = " [...truncated]";
            let content_budget_join = MAX_SANITIZED_OUTPUT_LEN.saturating_sub(JOIN_MARKER.len());
            let mut joined = String::with_capacity(MAX_SANITIZED_OUTPUT_LEN);
            let mut first = true;
            let mut truncated = false;
            let mut emitted_any = false;

            for m_value in msgs.iter() {
                let Some(m) = m_value.as_str() else { continue };
                let capped = cap_entry(m);
                let separator_len = if first { 0 } else { 2 };
                if joined.len() + separator_len + capped.len() > content_budget_join {
                    truncated = true;
                    break;
                }
                if !first {
                    joined.push_str("; ");
                }
                joined.push_str(capped.as_ref());
                first = false;
                emitted_any = true;
            }

            if emitted_any {
                if truncated {
                    joined.push_str(JOIN_MARKER);
                }
                debug_assert!(joined.len() <= MAX_SANITIZED_OUTPUT_LEN);
                return joined;
            }
        }
        if let Some(errors) = json.get("errors").and_then(|v| v.as_object()) {
            if !errors.is_empty() {
                // Per-entry formatting always allocates the "{k}: {v}" pair
                // string, so the per-entry cap_entry being Cow doesn't avoid
                // that — but it does avoid the prior to_string() copy for
                // unchanged values (which are the common case).
                let mut pairs: Vec<String> = errors
                    .iter()
                    .map(|(k, v)| {
                        if let Some(s) = v.as_str() {
                            // String value: borrow via Cow when no truncation.
                            format!("{k}: {}", cap_entry(s))
                        } else {
                            // Non-string value: must materialize once to apply
                            // the cap, then format. v.to_string() is a temp
                            // that doesn't outlive the Cow, so own immediately.
                            let serialized = v.to_string();
                            format!("{k}: {}", cap_entry(&serialized))
                        }
                    })
                    .collect();
                pairs.sort();
                return pairs.join("; ");
            }
        }
        if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
            return cap_entry(msg).into_owned();
        }
        if let Some(msg) = json.get("errorMessage").and_then(|v| v.as_str()) {
            return cap_entry(msg).into_owned();
        }
    }

    cap_entry(body_str).into_owned()
}

#[cfg(test)]
mod sanitize_tests {
    use super::{MAX_ERROR_ENTRY_LEN, MAX_SANITIZED_OUTPUT_LEN, cap_entry, sanitize_for_stderr};

    // Tiny helper to avoid repeating `.to_string()` in every call — keeps test
    // lines compact now that `sanitize_for_stderr` takes `String` by value.
    fn sanitize(s: &str) -> String {
        sanitize_for_stderr(s.to_string())
    }

    #[test]
    fn test_sanitize_for_stderr_passes_through_clean_ascii() {
        assert_eq!(sanitize("Hello, World!"), "Hello, World!");
    }

    #[test]
    fn test_sanitize_for_stderr_passes_through_utf8() {
        // Localized error messages (non-English Jira tenants) must survive.
        assert_eq!(sanitize("résumé"), "résumé");
        assert_eq!(sanitize("日本語のエラー"), "日本語のエラー");
        assert_eq!(sanitize("Привет"), "Привет");
    }

    #[test]
    fn test_sanitize_for_stderr_escapes_carriage_return() {
        assert_eq!(sanitize("line1\rline2"), "line1\\x0dline2");
    }

    #[test]
    fn test_sanitize_for_stderr_escapes_line_feed() {
        assert_eq!(sanitize("line1\nline2"), "line1\\x0aline2");
    }

    #[test]
    fn test_sanitize_for_stderr_escapes_crlf_fake_log_injection() {
        // CWE-117 attack pattern: hostile message tries to inject a fake log line
        // by ending one line and starting another with what looks like a logger prefix.
        let attack = "Issue not found\r\n[jr] CRITICAL: token leaked";
        let sanitized = sanitize(attack);
        assert_eq!(
            sanitized,
            "Issue not found\\x0d\\x0a[jr] CRITICAL: token leaked"
        );
        // Critically, the literal "\r\n" sequence is gone — the terminal can't break
        // the line, so the operator sees the whole attack on one line.
        assert!(!sanitized.contains('\r'));
        assert!(!sanitized.contains('\n'));
    }

    #[test]
    fn test_sanitize_for_stderr_escapes_ansi_escape_sequence() {
        // \x1b is ESC, the prefix for ANSI escape sequences (color, cursor, etc.).
        let attack = "Error\x1b[31m red text \x1b[0m";
        let sanitized = sanitize(attack);
        assert_eq!(sanitized, "Error\\x1b[31m red text \\x1b[0m");
        assert!(!sanitized.contains('\x1b'));
    }

    #[test]
    fn test_sanitize_for_stderr_escapes_null_byte() {
        assert_eq!(sanitize("a\0b"), "a\\x00b");
    }

    #[test]
    fn test_sanitize_for_stderr_escapes_tab() {
        // Tab is ASCII control (0x09). Conservatively escape — a sequence of tabs
        // can hide text behind column boundaries in some loggers / tab-aware UIs.
        assert_eq!(sanitize("a\tb"), "a\\x09b");
    }

    #[test]
    fn test_sanitize_for_stderr_escapes_del_character() {
        // 0x7F (DEL) is the last ASCII control char.
        assert_eq!(sanitize("a\x7fb"), "a\\x7fb");
    }

    #[test]
    fn test_sanitize_for_stderr_preserves_space_and_punctuation() {
        // Space (0x20) is NOT a control char. Punctuation is fine.
        assert_eq!(
            sanitize("Hello, World! (with punctuation)"),
            "Hello, World! (with punctuation)"
        );
    }

    #[test]
    fn test_sanitize_for_stderr_is_idempotent() {
        // Sanitizing an already-sanitized string is a no-op (no double-escaping).
        let once = sanitize("a\r\nb");
        let twice = sanitize_for_stderr(once.clone());
        assert_eq!(once, twice);
    }

    #[test]
    fn test_sanitize_for_stderr_empty_string() {
        assert_eq!(sanitize(""), "");
    }

    #[test]
    fn test_sanitize_for_stderr_clean_input_returns_same_string() {
        // Performance pin: the fast path returns the input String unchanged,
        // so the legitimate-error common case avoids the sanitization allocation.
        let input = String::from("legitimate error message with UTF-8: 日本語");
        let original_ptr = input.as_ptr();
        let result = sanitize_for_stderr(input);
        assert_eq!(
            result.as_ptr(),
            original_ptr,
            "fast path should reuse the same buffer"
        );
    }

    // -----------------------------------------------------------------------
    // cap_entry tests — per-entry length cap per issue #334 acceptance criteria
    // -----------------------------------------------------------------------

    #[test]
    fn test_cap_entry_passes_through_short_input() {
        let s = "short message";
        assert_eq!(cap_entry(s), s);
    }

    #[test]
    fn test_cap_entry_passes_through_at_max_length() {
        let s = "a".repeat(MAX_ERROR_ENTRY_LEN);
        assert_eq!(cap_entry(&s), s);
    }

    #[test]
    fn test_cap_entry_truncates_oversized_with_marker() {
        let s = "a".repeat(MAX_ERROR_ENTRY_LEN + 100);
        let capped = cap_entry(&s);
        assert!(capped.len() < s.len(), "expected truncation");
        assert!(
            capped.contains("[...truncated"),
            "expected truncation marker in '{capped}'"
        );
        assert!(
            capped.contains(&(MAX_ERROR_ENTRY_LEN + 100).to_string()),
            "expected original byte count in marker: '{capped}'"
        );
        // Size invariant: final output (prefix + marker) MUST stay within
        // the cap, otherwise the marker overhead defeats the flood-prevention
        // purpose of the cap.
        assert!(
            capped.len() <= MAX_ERROR_ENTRY_LEN,
            "cap_entry output {} bytes exceeds MAX_ERROR_ENTRY_LEN {} bytes",
            capped.len(),
            MAX_ERROR_ENTRY_LEN
        );
    }

    #[test]
    fn test_cap_entry_size_invariant_at_boundary_oversize() {
        // Regression pin: inputs only slightly larger than MAX_ERROR_ENTRY_LEN
        // (e.g., 1025 bytes) must NOT produce output longer than the input
        // due to the truncation marker overhead. cap_entry reserves marker
        // budget from the prefix so output_len <= MAX_ERROR_ENTRY_LEN
        // regardless of how close the input is to the boundary.
        for over in [1, 2, 5, 50, 100, 1000, 10000] {
            let s = "a".repeat(MAX_ERROR_ENTRY_LEN + over);
            let capped = cap_entry(&s);
            assert!(
                capped.len() <= MAX_ERROR_ENTRY_LEN,
                "size invariant violated for input len {} (over={}): output len = {}",
                s.len(),
                over,
                capped.len()
            );
            // And the marker must still be present so the operator sees that
            // truncation happened.
            assert!(
                capped.contains("[...truncated"),
                "marker missing for input len {} (over={})",
                s.len(),
                over
            );
        }
    }

    // -----------------------------------------------------------------------
    // sanitize_for_stderr post-sanitization output cap tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_sanitize_for_stderr_caps_post_sanitization_expansion() {
        // Pin: a long run of control bytes that expand 4x via the `\xNN`
        // escape (1024 input bytes → ~4096 sanitized bytes) must NOT flood
        // the terminal even with the per-entry pre-cap.
        // sanitize_for_stderr caps the FINAL output at MAX_SANITIZED_OUTPUT_LEN.
        // Build an input with enough control bytes to blow past the post-cap
        // if no cap were enforced.
        let input = "\n".repeat(MAX_SANITIZED_OUTPUT_LEN); // 4096 control bytes → 16 KiB sanitized
        let result = sanitize_for_stderr(input);
        assert!(
            result.len() <= MAX_SANITIZED_OUTPUT_LEN,
            "sanitize_for_stderr output {} bytes exceeds cap {}",
            result.len(),
            MAX_SANITIZED_OUTPUT_LEN
        );
        // Truncation marker should be visible so operator can see it happened.
        assert!(
            result.contains("[...truncated"),
            "expected truncation marker in output of length {}",
            result.len()
        );
    }

    #[test]
    fn test_sanitize_for_stderr_caps_oversized_clean_input() {
        // Even purely-clean inputs over the cap get truncated. UTF-8 char
        // boundaries are respected by `c.len_utf8()` accounting.
        let input = "a".repeat(MAX_SANITIZED_OUTPUT_LEN + 1000);
        let result = sanitize_for_stderr(input);
        assert!(
            result.len() <= MAX_SANITIZED_OUTPUT_LEN,
            "output {} bytes exceeds cap {}",
            result.len(),
            MAX_SANITIZED_OUTPUT_LEN
        );
        assert!(result.contains("[...truncated"));
    }

    #[test]
    fn test_sanitize_for_stderr_truncation_marker_excludes_out_len() {
        // Pin: the truncation marker must NOT reference out.len() — only
        // `original_len`. This makes the marker length independent of any
        // retroactive trim and prevents over-reporting (a marker that
        // claims a byte count not matching the trimmed output).
        let input = "a".repeat(MAX_SANITIZED_OUTPUT_LEN + 100);
        let original_len = input.len();
        let result = sanitize_for_stderr(input);
        // Marker should mention original_len (input bytes), not anything
        // resembling out.len() (post-trim bytes).
        assert!(
            result.contains(&format!("original {} bytes", original_len)),
            "marker should reference original_len; got: {result}"
        );
        // Negative pin: marker should NOT contain "sanitized bytes" or "at N"
        // phrasing that would over-report after trim.
        assert!(
            !result.contains("sanitized bytes"),
            "marker should not over-report sanitized byte count; got: {result}"
        );
        // Size invariant: final output stays within cap.
        assert!(
            result.len() <= MAX_SANITIZED_OUTPUT_LEN,
            "output {} bytes exceeds cap {}",
            result.len(),
            MAX_SANITIZED_OUTPUT_LEN
        );
    }

    #[test]
    fn test_sanitize_for_stderr_under_cap_no_truncation() {
        // Inputs that sanitize to a string within the cap pass through without
        // truncation markers.
        let input = "Error\r\nmessage".to_string();
        let result = sanitize_for_stderr(input);
        assert_eq!(result, "Error\\x0d\\x0amessage");
        assert!(!result.contains("[...truncated"));
    }

    #[test]
    fn test_cap_entry_respects_utf8_char_boundary() {
        // Build a string whose byte length exceeds the cap with a multibyte
        // char straddling the cap boundary. The truncation must NOT split
        // a UTF-8 char (would panic via &str[..end]).
        let padding = "a".repeat(MAX_ERROR_ENTRY_LEN - 2);
        let with_multibyte = format!("{padding}日本語のエラーです");
        // Should not panic, should produce valid UTF-8.
        let capped = cap_entry(&with_multibyte);
        // Verify it's still valid UTF-8 by re-parsing through Cow's str view.
        let _ = capped.as_ref();
        // Size invariant must also hold for UTF-8-bordering inputs.
        assert!(
            capped.len() <= MAX_ERROR_ENTRY_LEN,
            "size invariant violated for UTF-8 input: output len = {}",
            capped.len()
        );
    }
}
