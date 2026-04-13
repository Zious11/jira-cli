use crate::api::rate_limit::RateLimitInfo;
use crate::config::Config;
use crate::error::JrError;
use base64::Engine;
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::time::Duration;

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
    assets_base_url: Option<String>,
}

impl JiraClient {
    /// Build a `JiraClient` from the application config, loading auth credentials
    /// from the system keychain.
    pub fn from_config(config: &Config, verbose: bool) -> anyhow::Result<Self> {
        let base_url = config.base_url()?;

        // JR_BASE_URL overrides all URL targets (used by integration tests to inject wiremock).
        let test_override = std::env::var("JR_BASE_URL").ok();

        let instance_url = if let Some(ref override_url) = test_override {
            // Test mode: route all traffic (including instance and assets) to the mock server.
            override_url.trim_end_matches('/').to_string()
        } else if let Some(url) = config.global.instance.url.as_ref() {
            url.trim_end_matches('/').to_string()
        } else {
            return Err(JrError::ConfigError(
                "No Jira instance configured. Run \"jr init\" first.".into(),
            )
            .into());
        };
        let auth_method = config
            .global
            .instance
            .auth_method
            .as_deref()
            .unwrap_or("api_token");

        // JR_AUTH_HEADER env var overrides keychain auth (used by tests to inject mock auth)
        let auth_header = if let Ok(header) = std::env::var("JR_AUTH_HEADER") {
            header
        } else {
            match auth_method {
                "oauth" => {
                    let (access, _refresh) = crate::api::auth::load_oauth_tokens()?;
                    format!("Bearer {access}")
                }
                _ => {
                    // api_token (default)
                    let (email, token) = crate::api::auth::load_api_token()?;
                    let encoded = base64::engine::general_purpose::STANDARD
                        .encode(format!("{email}:{token}"));
                    format!("Basic {encoded}")
                }
            }
        };

        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        let assets_base_url = if let Some(ref override_url) = test_override {
            // Test mode: assets API goes to the mock server under /jsm/assets.
            Some(format!("{}/jsm/assets", override_url.trim_end_matches('/')))
        } else {
            config.global.instance.cloud_id.as_ref().map(|cloud_id| {
                format!(
                    "https://api.atlassian.com/ex/jira/{}/jsm/assets",
                    urlencoding::encode(cloud_id)
                )
            })
        };

        Ok(Self {
            client,
            base_url,
            instance_url,
            auth_header,
            verbose,
            assets_base_url,
        })
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
            assets_base_url,
        }
    }

    /// Perform a GET request and deserialize the JSON response.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let request = self.client.get(&url);
        let response = self.send(request).await?;
        let body = response.json::<T>().await?;
        Ok(body)
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
        let parsed = response.json::<T>().await?;
        Ok(parsed)
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
    async fn send(&self, request: RequestBuilder) -> anyhow::Result<Response> {
        // We need to be able to retry, so we clone the request builder.
        // reqwest::RequestBuilder::try_clone() returns None for streaming bodies,
        // but we only send JSON (or no body), so it will always succeed.
        let mut last_response: Option<Response> = None;

        for attempt in 0..=MAX_RETRIES {
            let req = request
                .try_clone()
                .expect("request should be cloneable (JSON body)");

            let req = req.header("Authorization", &self.auth_header);

            if self.verbose {
                if let Some(ref r) = req.try_clone().and_then(|r| r.build().ok()) {
                    eprintln!("[verbose] {} {}", r.method(), r.url());
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
                if self.verbose {
                    eprintln!(
                        "[verbose] Rate limited (429). Retrying in {delay}s (attempt {}/{})",
                        attempt + 1,
                        MAX_RETRIES
                    );
                }
                tokio::time::sleep(Duration::from_secs(delay)).await;
                last_response = Some(response);
                continue;
            }

            // Warn the user if we exhausted retries on a 429
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                eprintln!("warning: rate limited by Jira — gave up after {MAX_RETRIES} retries");
            }

            // For non-429 errors, parse and return the error
            if response.status().is_client_error() || response.status().is_server_error() {
                return Err(Self::parse_error(response).await);
            }

            return Ok(response);
        }

        // If we exhausted retries, parse the last 429 response as an error
        if let Some(resp) = last_response {
            return Err(Self::parse_error(resp).await);
        }

        unreachable!("retry loop should always return or set last_response");
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

            if self.verbose {
                eprintln!("[verbose] {} {}", req.method(), req.url());
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
                if self.verbose {
                    eprintln!(
                        "[verbose] Rate limited (429). Retrying in {delay}s (attempt {}/{})",
                        attempt + 1,
                        MAX_RETRIES
                    );
                }
                // Drop the 429 response before sleeping so its body isn't held open
                drop(response);
                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }

            // Warn the user if we exhausted retries on a 429
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                eprintln!("warning: rate limited by Jira — gave up after {MAX_RETRIES} retries");
            }

            // Return the response for ANY status (including 4xx/5xx) — no error parsing
            return Ok(response);
        }

        unreachable!("loop iterates 0..=MAX_RETRIES; final iteration returns")
    }

    /// Parse an error response into a `JrError`.
    async fn parse_error(response: Response) -> anyhow::Error {
        let status = response.status().as_u16();

        if status == 401 {
            return JrError::NotAuthenticated.into();
        }

        let message = match response.bytes().await {
            Ok(body) => extract_error_message(&body),
            Err(e) => format!("Could not read error response: {e}"),
        };

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
        let body = response.json::<T>().await?;
        Ok(body)
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
        let parsed = response.json::<T>().await?;
        Ok(parsed)
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
        Ok(response.json::<T>().await?)
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
        Ok(response.json::<T>().await?)
    }

    /// Returns the HTTP method for building requests externally (if needed).
    pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .request(method, &url)
            .header("Authorization", &self.auth_header)
    }
}

/// Extract a human-readable error message from a Jira error response body.
///
/// Precedence:
/// 1. Non-empty `errorMessages` array → joined with "; "
/// 2. Non-empty `errors` object (field-level validation) → "field: msg; field2: msg2"
/// 3. `message` string field
/// 4. `errorMessage` string field (singular, seen in some JSM endpoints)
/// 5. Empty body → "<empty response body>"
/// 6. Raw body as a string (fallback)
pub fn extract_error_message(body: &[u8]) -> String {
    if body.is_empty() {
        return "<empty response body>".to_string();
    }

    let body_str = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => return String::from_utf8_lossy(body).into_owned(),
    };

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
        if let Some(msgs) = json.get("errorMessages").and_then(|v| v.as_array()) {
            let messages: Vec<&str> = msgs.iter().filter_map(|m| m.as_str()).collect();
            if !messages.is_empty() {
                return messages.join("; ");
            }
        }
        if let Some(errors) = json.get("errors").and_then(|v| v.as_object()) {
            if !errors.is_empty() {
                let mut pairs: Vec<String> = errors
                    .iter()
                    .map(|(k, v)| {
                        if let Some(s) = v.as_str() {
                            format!("{k}: {s}")
                        } else {
                            format!("{k}: {v}")
                        }
                    })
                    .collect();
                pairs.sort();
                return pairs.join("; ");
            }
        }
        if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
            return msg.to_string();
        }
        if let Some(msg) = json.get("errorMessage").and_then(|v| v.as_str()) {
            return msg.to_string();
        }
    }

    body_str.to_string()
}
