use crate::api::rate_limit::RateLimitInfo;
use crate::config::Config;
use crate::error::JrError;
use base64::Engine;
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
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
}

impl JiraClient {
    /// Build a `JiraClient` from the application config, loading auth credentials
    /// from the system keychain.
    pub fn from_config(config: &Config, verbose: bool) -> anyhow::Result<Self> {
        let base_url = config.base_url()?;
        let instance_url = config
            .global
            .instance
            .url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No Jira instance configured. Run \"jr init\" first."))?
            .trim_end_matches('/')
            .to_string();
        let auth_method = config
            .global
            .instance
            .auth_method
            .as_deref()
            .unwrap_or("api_token");

        let auth_header = match auth_method {
            "oauth" => {
                let (access, _refresh) = crate::api::auth::load_oauth_tokens()?;
                format!("Bearer {access}")
            }
            _ => {
                // api_token (default)
                let (email, token) = crate::api::auth::load_api_token()?;
                let encoded =
                    base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"));
                format!("Basic {encoded}")
            }
        };

        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        Ok(Self {
            client,
            base_url,
            instance_url,
            auth_header,
            verbose,
        })
    }

    /// Create a client for integration testing. This is **not** gated behind
    /// `#[cfg(test)]` so that integration tests in `tests/` can use it.
    pub fn new_for_test(base_url: String, auth_header: String) -> Self {
        Self {
            client: Client::new(),
            instance_url: base_url.clone(),
            base_url,
            auth_header,
            verbose: false,
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

    /// Parse an error response into a `JrError`.
    async fn parse_error(response: Response) -> anyhow::Error {
        let status = response.status().as_u16();

        if status == 401 {
            return JrError::NotAuthenticated.into();
        }

        // Try to extract errorMessages from the JSON body
        let message = match response.text().await {
            Ok(body) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    // Jira returns { "errorMessages": ["..."] } or { "message": "..." }
                    if let Some(msgs) = json.get("errorMessages").and_then(|v| v.as_array()) {
                        let messages: Vec<&str> = msgs.iter().filter_map(|m| m.as_str()).collect();
                        if !messages.is_empty() {
                            messages.join("; ")
                        } else {
                            body
                        }
                    } else if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
                        msg.to_string()
                    } else {
                        body
                    }
                } else {
                    body
                }
            }
            Err(_) => "Unknown error".to_string(),
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

    /// Returns the HTTP method for building requests externally (if needed).
    pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .request(method, &url)
            .header("Authorization", &self.auth_header)
    }
}
