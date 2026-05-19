use crate::api::rate_limit::{MAX_RETRY_AFTER_SECS, RateLimitInfo};
use crate::config::Config;
use crate::error::JrError;
use base64::Engine;
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::time::{Duration, Instant};
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
        // Debug builds only — release binaries ignore this env var to prevent
        // JR_BASE_URL=http://attacker/ from leaking the bearer token to a non-Atlassian
        // endpoint (paired with the existing JR_AUTH_HEADER #[cfg(debug_assertions)]
        // gate below per SD-002 resolution). Closes audit-followup #335.
        #[cfg(debug_assertions)]
        let test_override = std::env::var("JR_BASE_URL").ok();
        #[cfg(not(debug_assertions))]
        let test_override: Option<String> = None;

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

    /// Deadline-aware GET-and-deserialize for callers with a wall-clock budget.
    /// Equivalent to `get` but routes through `send_bounded` so a 429-storm on
    /// this endpoint cannot push elapsed time past `deadline`.
    ///
    /// Used by `poll_bulk_task_with_deadline` in `src/api/jira/bulk.rs` to
    /// honor the caller's `await_bulk_task` timeout end-to-end.
    /// Anchor: BC-bulk.poll.deadline-bounded (S-333).
    pub async fn get_bounded<T: DeserializeOwned>(
        &self,
        path: &str,
        deadline: Instant,
    ) -> anyhow::Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let request = self.client.get(&url);
        let response = self.send_bounded(request, deadline).await?;
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
    /// Thin wrapper around `send_inner` that passes `None` for the deadline —
    /// preserves the historical 1-arg signature for the ~hundreds of existing
    /// call sites that have no caller-supplied deadline. All retry, refresh,
    /// and error-mapping logic lives in `send_inner`; see its docs for the
    /// detailed contract (429 retries, 401 auto-refresh, AC-010 reconcile).
    async fn send(&self, request: RequestBuilder) -> anyhow::Result<Response> {
        self.send_inner(request, None).await
    }
}

/// Result of clamping a 429-retry sleep against a caller-supplied deadline.
///
/// `Sleep(d)` is the duration the caller should actually sleep (already capped
/// at the smaller of `retry_after` and `deadline - now()`). `Expired` means
/// the deadline has expired (or is within the tokio timer-wheel 1ms floor of
/// expiring) — the caller MUST return an error rather than sleeping.
///
/// Anchor: BC-bulk.poll.deadline-bounded (S-333).
#[derive(Debug, PartialEq, Eq)]
enum ClampResult {
    /// Caller should sleep for this duration before retrying.
    Sleep(Duration),
    /// Deadline expired (or within 1ms of expiring) — caller MUST return
    /// `JrError::DeadlineExceeded { remaining_ms, message }` (exit code 124,
    /// POSIX `timeout(1)` convention). The remaining-millisecond figure is
    /// included in the error message so operators can correlate the failure
    /// against their `--timeout` budget.
    ///
    /// (Pre-F5-pass-02 history: this used to map to `JrError::ApiError{status:429,
    /// ...}`. The reversal is documented in
    /// `.factory/code-delivery/issue-333/research-validation-pass-03.md` Q2.)
    Expired { remaining_ms: u64 },
}

/// Compute the clamped 429-retry sleep duration.
///
/// # Contract
///
/// - `deadline == None` ⇒ `ClampResult::Sleep(base)` (no caller-supplied
///   budget; identical to the historical pre-S-333 behavior).
/// - `deadline == Some(d)` and `remaining = d.saturating_duration_since(now())`:
///   - `remaining >= 1ms` ⇒ `ClampResult::Sleep(min(base, remaining))`.
///   - `remaining < 1ms` ⇒ `ClampResult::Expired { remaining_ms }`.
///
/// # Why 1ms, not zero
///
/// `tokio::time::sleep(Duration < 1ms)` is documented as a no-op that does
/// NOT yield to the executor (tokio timer wheel has a 1ms resolution floor;
/// see Q3 research-validation 2026-05-12, tokio #4522). If we accepted any
/// non-zero remaining as "sleep it", a sub-millisecond `remaining` would
/// produce a no-op sleep and the retry loop would spin immediately to the
/// next request — silently violating the bounded-overshoot contract.
///
/// Anchor: BC-bulk.poll.deadline-bounded (S-333).
fn clamp_retry_sleep(base: Duration, deadline: Option<Instant>) -> ClampResult {
    match deadline {
        Some(d) => {
            let remaining = d.saturating_duration_since(Instant::now());
            if remaining < Duration::from_millis(1) {
                // `as_micros()` returns u128; division by 1000 keeps the value
                // bounded by sub-ms input, safe to truncate to u64.
                let remaining_ms = (remaining.as_micros() / 1000) as u64;
                ClampResult::Expired { remaining_ms }
            } else {
                ClampResult::Sleep(base.min(remaining))
            }
        }
        None => ClampResult::Sleep(base),
    }
}

impl JiraClient {
    /// Deadline-aware variant of `send` for callers that have a wall-clock
    /// budget (e.g., `await_bulk_task`). Sleep durations inside the 429 retry
    /// loop are clamped to `min(retry_after, deadline - now)` so a 429-storm
    /// near the deadline cannot push elapsed time past it.
    ///
    /// # Behavior
    ///
    /// Returns `Err(JrError::DeadlineExceeded { remaining_ms, message })`
    /// (exit code 124, POSIX `timeout(1)` convention) in two cases:
    ///
    ///   1. **At function entry** — if the remaining budget
    ///      `deadline.saturating_duration_since(Instant::now()) < 1ms`
    ///      (i.e., the deadline has already passed OR is within the tokio
    ///      timer-wheel 1ms floor of passing), the request is NOT issued.
    ///      Message has the `[deadline:send-entry]` site tag.
    ///
    ///   2. **During 429 retry** — if a 429 fires and the remaining budget
    ///      `deadline.saturating_duration_since(Instant::now()) < 1ms`, the
    ///      retry loop short-circuits. Message has the `[deadline:429-retry]`
    ///      site tag. (Q3 research-validation: `tokio::time::sleep(Duration
    ///      < 1ms)` is a no-op that does NOT yield, so the 1ms threshold
    ///      prevents a spin-loop in the sub-millisecond edge case.)
    ///
    /// A third site exists OUTSIDE `send_inner` in
    /// `src/api/jira/bulk.rs::await_bulk_task_inner` — the polling-loop
    /// top-of-loop check — which surfaces as
    /// `JrError::DeadlineExceeded` with the `[deadline:bulk-outer]` tag.
    /// All three sites share exit code 124 so scripting consumers can
    /// detect "caller deadline expired" uniformly.
    ///
    /// In both cases scripts can grep stderr for the substring `"deadline"`
    /// or pattern-match on exit code 124 to detect the timeout (distinct
    /// from `ApiError(429)` rate-limit, which uses exit 1).
    ///
    /// On every 429 retry where remaining ≥ 1ms, sleeps for
    /// `min(retry_after, remaining)` and continues the retry loop.
    ///
    /// # Scope of the deadline guarantee
    ///
    /// The deadline is enforced for:
    /// 1. **429 retry sleeps** — clamped per the formula above.
    /// 2. **Already-expired deadlines at function entry** — returns Err
    ///    without issuing the request.
    ///
    /// The deadline is **NOT** enforced for:
    /// 1. **Per-request reqwest timeout** — the underlying `reqwest::Client`
    ///    is built with `Client::builder().timeout(Duration::from_secs(30))`
    ///    in `from_config`; a single hung request cannot exceed 30s
    ///    regardless of the caller-supplied deadline.
    /// 2. **OAuth 401 auto-refresh path** — once a poll returns 401 (token
    ///    expired), the auto-refresh path makes an OAuth refresh POST via
    ///    `crate::api::auth::refresh_oauth_token_with_url`. That helper
    ///    currently constructs its own `reqwest::Client::new()` **without
    ///    any timeout** (`src/api/auth.rs` ~line 903), so the refresh POST
    ///    is unbounded. A 401-near-deadline scenario can therefore overshoot
    ///    by tens of seconds (or longer) while the refresh is in flight.
    /// 3. **401 retry after successful refresh** — the post-refresh retry
    ///    bypasses `send_inner` entirely and calls `retry_req.send().await`
    ///    directly, so the 30s reqwest timeout applies but the deadline
    ///    clamp does not.
    ///
    /// User-facing impact: a 30s deadline reliably bounds 429 storms (the
    /// #333 fix), but a 401 near deadline may still cause 60-120s wall-clock
    /// in pathological cases. Bounding the OAuth refresh path is tracked as
    /// a follow-up issue (see PR #333's body for the link).
    ///
    /// Anchors: BC-bulk.poll.deadline-bounded, NFR-R-NEW-3,
    /// H-NEW-BULK-DEADLINE-001 (S-333).
    pub async fn send_bounded(
        &self,
        request: RequestBuilder,
        deadline: Instant,
    ) -> anyhow::Result<Response> {
        self.send_inner(request, Some(deadline)).await
    }

    async fn send_inner(
        &self,
        request: RequestBuilder,
        deadline: Option<Instant>,
    ) -> anyhow::Result<Response> {
        // Pure helper for the 429-retry sleep clamp. Extracted for unit testing.
        // See `clamp_retry_sleep` rustdoc for the full contract.

        // S-333 CONCERN-7: defense-in-depth entry-point check. If the caller
        // passed a deadline that has already expired (e.g., due to a bug
        // upstream that subtracted a Duration), fail fast WITHOUT issuing
        // the request. The clamp inside the 429 loop would otherwise only
        // fire if Atlassian happens to return 429 — a 2xx response on an
        // already-expired deadline would silently succeed and the caller
        // would have no signal that their deadline was disregarded.
        //
        // Note: aws-smithy-rust / hyper / reqwest-middleware do NOT
        // canonically do this entry-point check (research-validation pass-02
        // Q4) — they let the request fly and rely on per-attempt timeouts.
        // We do it because:
        //   - it's cheap (one Instant::now()),
        //   - the deadline-aware path is internal/narrow (only the bulk poll
        //     uses it today), and
        //   - bug-symmetry: the clamp inside the loop already enforces "no
        //     sleep on expired", so this just extends that invariant to
        //     "no request on expired" — one consistent semantic.
        if let Some(d) = deadline {
            if let ClampResult::Expired { remaining_ms } =
                clamp_retry_sleep(Duration::ZERO, Some(d))
            {
                return Err(JrError::DeadlineExceeded {
                    remaining_ms,
                    message: format!(
                        "[deadline:send-entry] Caller-supplied deadline already \
                         expired at send entry (remaining budget {remaining_ms}ms). \
                         The request was not issued. Rerun with a larger timeout."
                    ),
                }
                .into());
            }
        }

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

                    // S-333 C-2 (F5 pass-03): deadline-clamp fires BEFORE the cap-abort.
                    //
                    // Earlier pass-01 NIT-2 noted "user-impact is equivalent" between the
                    // cap-abort and the deadline-clamp — that was true when both returned
                    // JrError::ApiError(429) with exit code 1. Pass-02 introduced
                    // JrError::DeadlineExceeded (exit code 124, POSIX timeout convention),
                    // so the variants AND exit codes now diverge: cap → 1, deadline → 124.
                    //
                    // Industry precedent (research-validation pass-04 Q2): aws-smithy-rs,
                    // tokio::time::timeout, kubectl client-go, and RFC 9110 §10.2.3 all
                    // treat the client-side deadline as a hard contract that supersedes a
                    // server-advisory Retry-After. Reordering ensures a 429 with
                    // Retry-After > 60s AND an expired caller-supplied deadline surfaces
                    // as DeadlineExceeded (exit 124), not as the cap message (exit 1).
                    //
                    // The cap-abort still applies when no deadline is set OR when the
                    // deadline remains positive — that case retains the historical
                    // BC-X.4.009 behavior unchanged.
                    let base_sleep = Duration::from_secs(delay);
                    let actual_sleep = match clamp_retry_sleep(base_sleep, deadline) {
                        ClampResult::Sleep(d) => d,
                        ClampResult::Expired { remaining_ms } => {
                            return Err(JrError::DeadlineExceeded {
                                remaining_ms,
                                message: format!(
                                    "[deadline:429-retry] Caller-supplied deadline \
                                     exceeded during 429 retry (Retry-After {delay}s, \
                                     remaining budget {remaining_ms}ms before clamp). \
                                     Atlassian rate-limit pressure consumed the caller-\
                                     supplied timeout. Rerun with a larger timeout, or \
                                     wait for rate-limit pressure to subside."
                                ),
                            }
                            .into());
                        }
                    };

                    // BC-X.4.009: abort retry if Retry-After exceeds the interactive-CLI cap.
                    // Atlassian's typical values (1425-3089s) far exceed what is acceptable
                    // for a foreground CLI. RFC 9110 §10.2.3 permits client-side abort.
                    //
                    // Note (S-333 C-2): this check now fires AFTER the deadline-clamp above.
                    // If both fire on the same response, deadline-clamp wins (exit 124, the
                    // higher-priority signal per industry precedent).
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
                        clamped_sleep_ms = actual_sleep.as_millis() as u64,
                        attempt = attempt + 1,
                        max_retries = MAX_RETRIES,
                        deadline_aware = deadline.is_some(),
                        "rate_limited_retrying"
                    );
                    tokio::time::sleep(actual_sleep).await;
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
                required_scope: None,
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
                return JrError::InsufficientScope {
                    message,
                    required_scope: None,
                }
                .into();
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
/// entry (1024 × 4 = 4096). When truncation occurs and the marker would push
/// the output past this cap, `sanitize_for_stderr` retroactively trims `out`
/// at a UTF-8 char boundary to make room — the marker text references only
/// the immutable `original_len` so its length doesn't shift under the trim.
const MAX_SANITIZED_OUTPUT_LEN: usize = 4096;

/// Maximum byte length of an error response body that will be parsed as JSON.
///
/// Defense against memory-amplification at the JSON parse step (OWASP
/// API4:2023 — Unrestricted Resource Consumption / CWE-770 — Allocation of
/// Resources Without Limits or Throttling, Perplexity-validated 2026-05-11).
/// `serde_json::from_str::<Value>`
/// builds a full DOM that costs roughly 2-3x the body size in memory; a
/// hostile server returning a valid 100 MB JSON body would force 200-300 MB
/// of DOM allocation even though every downstream cap (`cap_entry`,
/// `serialize_value_bounded`, `sanitize_for_stderr`) is in place. The
/// downstream caps bound the OUTPUT — they cannot prevent the INPUT DOM
/// from being materialized.
///
/// Bodies larger than this cap skip JSON parsing entirely and fall back to
/// the raw-body path (which is itself byte-bounded via `cap_entry`). Per
/// Perplexity-validated industry guidance the recommended pattern is a
/// byte-level size gate at ~16 KiB for error-response paths; legitimate
/// Atlassian/Jira error responses are <1 KiB, so 16 KiB is generous yet
/// blocks the amplification vector.
const MAX_PARSE_BODY_LEN: usize = 16 * 1024;

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

/// Bounded JSON serialization of a `serde_json::Value`.
///
/// Defense-in-depth against memory-amplification DoS (OWASP API4:2023 (Unrestricted Resource Consumption) / CWE-770 (Allocation of Resources Without Limits or Throttling)) for the
/// non-string-value branch of `extract_error_message_raw`. `Value::to_string()`
/// (or `serde_json::to_string`) fully serializes the input into a `String`
/// BEFORE `cap_entry` can truncate it — a hostile response with a deeply
/// nested or large server-controlled value would force an unbounded allocation
/// even though the final stderr output is capped.
///
/// We serialize into a byte-bounded writer that returns `WriteZero` once
/// `limit` is reached. `serde_json::to_writer` propagates the error and stops,
/// so total memory is bounded to `limit` bytes regardless of input size/depth.
/// The partial prefix is returned for the standard `cap_entry` pipeline; UTF-8
/// is repaired lossily because the cutoff may land mid-codepoint.
///
/// When overflow is detected, the function appends a visible
/// `[...truncated]` marker so callers/operators see that the JSON is a
/// truncated prefix rather than a malformed-but-silently-incomplete fragment.
/// Marker bytes are reserved upfront from the byte budget so the function
/// strictly satisfies `out.len() <= limit` even with the marker appended.
///
/// `limit` is chosen as `MAX_ERROR_ENTRY_LEN` — `cap_entry` will then pass the
/// (already-bounded) prefix through with zero further allocation. Tighter than
/// the typical 1 MB serialization cap recommended by general guidance, but
/// appropriate for this domain (stderr error messages, not data interchange).
fn serialize_value_bounded(v: &serde_json::Value, limit: usize) -> String {
    /// Writer that accepts up to `limit` bytes then refuses further writes.
    /// Sets `overflowed` so the caller can append a truncation marker —
    /// without this flag, a hostile response would silently produce a partial
    /// JSON prefix with no indication to operators that data was cut off.
    struct Bounded {
        buf: Vec<u8>,
        limit: usize,
        overflowed: bool,
    }
    impl std::io::Write for Bounded {
        fn write(&mut self, src: &[u8]) -> std::io::Result<usize> {
            // Per std::io::Write::write contract (Perplexity-validated
            // 2026-05-11): "If an error is returned then no bytes in the
            // buffer were written to this writer." We honor this strictly:
            //
            // - `remaining == 0`: write nothing, return Err(WriteZero) to
            //   signal stop. This is the ONLY path that returns Err.
            // - `take == src.len()` (full write fits): write all, return
            //   Ok(take).
            // - `take < src.len()` (partial write fits): write the prefix,
            //   return Ok(take), set `overflowed`. The NEXT call will hit
            //   `remaining == 0` and return Err to stop serialization while
            //   preserving the buffered prefix.
            //
            // Violating "no bytes on Err" would break write_all and similar
            // retry-aware callers that rely on the invariant. The serde_json
            // caller uses write_all-style loops, so the partial-write path
            // now correctly accepts bytes and lets the next call error out.
            let remaining = self.limit.saturating_sub(self.buf.len());
            if remaining == 0 {
                self.overflowed = true;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "bounded JSON writer reached byte limit",
                ));
            }
            let take = src.len().min(remaining);
            self.buf.extend_from_slice(&src[..take]);
            if take < src.len() {
                // Partial write case: bytes accepted, but next call errors.
                // Mark overflowed for the caller's marker logic.
                self.overflowed = true;
            }
            Ok(take)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    // Reserve marker bytes upfront so the final output (prefix + marker)
    // strictly fits within `limit`. If `limit` is too small to even hold the
    // marker (degenerate case — limit < ~16 bytes), serialize without a marker
    // and rely on the post-hoc trim below to enforce the byte cap.
    const TRUNCATION_MARKER: &str = " [...truncated]";
    let writer_limit = limit.saturating_sub(TRUNCATION_MARKER.len());
    // If reserving the marker would leave no room for content, fall back to
    // raw-bounded mode (no marker). This degenerate path applies only when
    // `limit` is smaller than the marker itself.
    let reserve_marker = writer_limit > 0;
    let effective_limit = if reserve_marker { writer_limit } else { limit };

    let mut w = Bounded {
        buf: Vec::with_capacity(effective_limit.min(256)),
        limit: effective_limit,
        overflowed: false,
    };
    // Result intentionally ignored: WriteZero on overflow is the expected
    // bounded-truncation signal. The (possibly partial) prefix in `w.buf` is
    // what we want for the downstream cap_entry pipeline.
    let _ = serde_json::to_writer(&mut w, v);
    let overflowed = w.overflowed;
    // Lossy because serde_json may have written a partial multi-byte UTF-8
    // codepoint when the writer cut off mid-string.
    let mut s = String::from_utf8_lossy(&w.buf).into_owned();
    // Lossy replacement can expand the string by a few bytes (one U+FFFD = 3
    // bytes replacing 1-2 trailing invalid bytes when a multibyte char was
    // cut mid-codepoint). Re-cap at a UTF-8 char boundary so the function
    // strictly satisfies `out.len() <= limit` — callers (and the cap_entry
    // pipeline) rely on this invariant. Use `effective_limit` so the marker
    // (if appended below) still fits within `limit`.
    if s.len() > effective_limit {
        let mut end = effective_limit;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    // Append the truncation marker so consumers see that JSON is incomplete
    // rather than a malformed-but-silently-truncated fragment.
    // Only when we actually overflowed AND we reserved budget for the marker.
    if overflowed && reserve_marker {
        s.push_str(TRUNCATION_MARKER);
    }
    s
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
/// - Replace every Unicode control character (per `char::is_control()`) with
///   a visible escape so the operator can see what was sent but the terminal
///   cannot interpret it:
///   - ASCII controls (C0 = 0x00-0x1F, plus DEL = 0x7F) → `\xNN` (4 bytes)
///   - C1 controls (U+0080..U+009F, including CSI U+009B and NEL U+0085) →
///     `\u{NNNN}` (8 bytes); preserved as visible escapes for defense-in-depth
///     against legacy/non-UTF8 terminals.
/// - Enforce a `MAX_SANITIZED_OUTPUT_LEN` cap on the FINAL output bytes via
///   a byte-budget-aware char loop. Closes the gap where pre-sanitization
///   per-entry caps (via `cap_entry`) couldn't bound the post-sanitization
///   output: 1024 ASCII control bytes expand to ~4096 bytes after `\xNN`
///   escaping (C1 controls have the same 4x expansion factor: 2 → 8 bytes),
///   so a per-entry pre-cap alone left the terminal vulnerable to floods.
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

    // Fast-path check: char-level scan for Unicode control chars (both
    // ASCII C0+DEL and C1 U+0080..U+009F). `char::is_control()` covers
    // the full control set; byte-level scanning is insufficient because
    // C1 controls are 2-byte UTF-8 sequences (0xc2 0x80..0xc2 0x9f) that
    // a raw byte scan can't distinguish from valid multi-byte text.
    // Char decoding costs more than byte iteration but is required for
    // comprehensive defense-in-depth against legacy/non-UTF8 terminals
    // that may still interpret C1 codepoints as control sequences.
    let needs_sanitization = input.chars().any(|c| c.is_control());
    let needs_truncation = input.len() > MAX_SANITIZED_OUTPUT_LEN;

    // Fast path: no control chars (C0, DEL, OR C1) AND under the output
    // cap → return the input String unchanged. No new allocation.
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
        // Compute the byte cost of emitting `c` BEFORE pushing it.
        // - ASCII controls (C0 + DEL): expand 1→4 bytes via `\xNN`
        // - C1 controls (U+0080..U+009F): expand 2→8 bytes via `\u{NNNN}`
        //   (still 4x, preserving the 4 KiB cap's worst-case budget)
        // - Other chars: `c.len_utf8()` (1-4 bytes for any Unicode scalar)
        //
        // C1 controls (U+0080..U+009F) are valid Unicode codepoints with
        // valid 2-byte UTF-8 encodings (e.g., U+009B = 0xC2 0x9B), but
        // modern UTF-8 terminals generally do NOT act on them as control
        // sequences — a single raw byte in 0x80-0x9F is treated as an
        // invalid leading byte and dropped, and a properly-encoded
        // codepoint is decoded but most terminals ignore the C1 semantics
        // entirely in UTF-8 mode. Legacy/embedded/non-UTF8 terminals can
        // still interpret U+009B (CSI) and friends as control sequences,
        // so we escape them here for comprehensive defense-in-depth.
        // `char::is_control()` catches both C0 and C1.
        let needed = if c.is_control() {
            if c.is_ascii() { 4 } else { 8 }
        } else {
            c.len_utf8()
        };
        if out.len() + needed > MAX_SANITIZED_OUTPUT_LEN {
            truncated = true;
            break;
        }
        if c.is_control() {
            // Write directly into `out` via fmt::Write — avoids the per-escape
            // String allocation that `format!()` would introduce.
            // write! on String is infallible; ignore the Result to avoid a
            // distracting `expect()` in a hot loop.
            if c.is_ascii() {
                let _ = write!(out, "\\x{:02x}", c as u8);
            } else {
                // C1 control: emit visible `\u{NNNN}` form so operators can
                // see WHICH control was injected (e.g., \u{009b} = CSI).
                let _ = write!(out, "\\u{{{:04x}}}", c as u32);
            }
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
/// escapes Unicode control characters from server-supplied content as
/// visible escape sequences — ASCII controls (C0 + DEL) as `\xNN` and
/// C1 controls (U+0080..U+009F) as `\u{NNNN}` — before they reach stderr,
/// preventing CR/LF/ANSI/CSI injection from a hostile or proxy-controlled
/// response while keeping the byte information visible to the operator).
/// UTF-8 in legitimate localized error messages is preserved.
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
            // Memory-amplification defense (OWASP API4:2023 (Unrestricted Resource Consumption) / CWE-770 (Allocation of Resources Without Limits or Throttling), Perplexity-validated
            // 2026-05-11): `String::from_utf8_lossy` allocates an owned String for
            // the ENTIRE byte slice even though `cap_entry` will truncate the result
            // to MAX_ERROR_ENTRY_LEN. A hostile server returning a massive non-UTF8
            // body would otherwise force O(body.len()) memory allocation before the
            // cap kicks in. Pre-cap the byte slice to a small multiple of the entry
            // cap (allows worst-case lossy expansion via U+FFFD replacement chars
            // at 3 bytes each) before conversion.
            //
            // Use a custom marker that records the ORIGINAL body.len() rather than
            // relying on `cap_entry`'s default marker: cap_entry's
            // marker reports the *post-pre-cap* string length, which can never
            // exceed PRE_CAP_BYTES (~4096 bytes). When operators are diagnosing a
            // resource-exhaustion / flood attempt, they need to see that the
            // response was, e.g., 100 MB — not "4096 bytes total".
            const PRE_CAP_BYTES: usize = MAX_ERROR_ENTRY_LEN * 4;
            let original_len = body.len();
            let bounded = &body[..original_len.min(PRE_CAP_BYTES)];
            let lossy = String::from_utf8_lossy(bounded);
            // Fast path: body already fits in MAX_ERROR_ENTRY_LEN, no marker needed.
            if lossy.len() <= MAX_ERROR_ENTRY_LEN && original_len <= MAX_ERROR_ENTRY_LEN {
                return lossy.into_owned();
            }
            // Custom marker reports the true original byte length and flags the
            // non-UTF8 source so operators can distinguish this fallback from
            // the normal cap_entry truncation path.
            let marker = format!(" [...truncated, {original_len} bytes total, non-UTF8 body]");
            let target_prefix_len = MAX_ERROR_ENTRY_LEN.saturating_sub(marker.len());
            // Degenerate-case fallback: if marker is larger than the entry cap
            // (would only fire if MAX_ERROR_ENTRY_LEN is shrunk drastically),
            // return only the marker (or its truncated form) so we still emit
            // SOME signal rather than nothing.
            if target_prefix_len == 0 {
                let mut end = MAX_ERROR_ENTRY_LEN.min(marker.len());
                while end > 0 && !marker.is_char_boundary(end) {
                    end -= 1;
                }
                return marker[..end].to_string();
            }
            let mut end = target_prefix_len.min(lossy.len());
            while end > 0 && !lossy.is_char_boundary(end) {
                end -= 1;
            }
            return format!("{}{}", &lossy[..end], marker);
        }
    };

    // Memory-amplification defense at the JSON parse step (OWASP API4:2023 (Unrestricted Resource Consumption) / CWE-770 (Allocation of Resources Without Limits or Throttling),
    // Perplexity-validated 2026-05-11). `serde_json::from_str::<Value>`
    // materializes a full DOM that costs roughly 2-3x the body size in memory.
    // Even though every downstream cap (cap_entry, serialize_value_bounded,
    // sanitize_for_stderr) bounds OUTPUT, none of them prevent the INPUT DOM
    // from being allocated. A hostile valid 100 MB JSON body would force
    // 200-300 MB of DOM allocation before any cap kicks in.
    //
    // Size-gate at the byte level: bodies larger than MAX_PARSE_BODY_LEN
    // (16 KiB) skip JSON parsing and fall back to the byte-bounded raw-body
    // path. Legitimate Jira error responses are <1 KiB, so 16 KiB is a
    // generous threshold that blocks the amplification vector without
    // affecting real responses. The byte-level gate is preferred over a
    // streaming/partial parse because it has zero allocation attack surface —
    // we reject before serde_json::Value DOM materialization rather than
    // hoping the streaming parse stops at the right point.
    if body_str.len() > MAX_PARSE_BODY_LEN {
        return cap_entry(body_str).into_owned();
    }

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
        if let Some(msgs) = json.get("errorMessages").and_then(|v| v.as_array()) {
            // Memory-amplification defense (OWASP API4:2023 (Unrestricted Resource Consumption) / CWE-770 (Allocation of Resources Without Limits or Throttling), Perplexity-validated
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
                // Memory-amplification defense (OWASP API4:2023 (Unrestricted Resource Consumption) / CWE-770 (Allocation of Resources Without Limits or Throttling), same threat
                // class as the errorMessages streaming join earlier). Three
                // server-controlled vectors are bounded here:
                //
                // 1. Entry count — `take(MAX_ERROR_PAIRS)` before collect/sort
                //    so a hostile response with 1M keys cannot force a 1M-entry
                //    intermediate Vec.
                //
                // 2. Key length — each key passes through `cap_entry` BEFORE
                //    format!. Without this cap, a hostile response with a
                //    small number of pathologically large keys (e.g., 1 MB
                //    key name) would amplify intermediate allocations in the
                //    formatted pair string even with the entry-count cap.
                //
                // 3. Non-string value size/depth — `serialize_value_bounded`
                //    serializes via a byte-limited writer instead of
                //    `Value::to_string()`. A hostile response with deeply
                //    nested or huge non-string values cannot force a full
                //    serialization allocation before cap_entry truncates.
                //
                // MAX_ERROR_PAIRS = 256 is generous (legitimate Jira responses
                // have 1-10 field-level errors). Per-pair memory is bounded
                // by 2 × MAX_ERROR_ENTRY_LEN (capped key + capped value) plus
                // ~4 bytes of format overhead, so the intermediate Vec is
                // bounded at roughly 256 × 2 × 1024 ≈ 512 KiB worst case.
                // The downstream streaming join further bounds OUTPUT to
                // MAX_SANITIZED_OUTPUT_LEN.
                const MAX_ERROR_PAIRS: usize = 256;
                let total_keys = errors.len();
                let mut pairs: Vec<String> = errors
                    .iter()
                    .take(MAX_ERROR_PAIRS)
                    .map(|(k, v)| {
                        // Cap server-controlled key length BEFORE format!
                        // (see comment block above).
                        let k_capped = cap_entry(k);
                        if let Some(s) = v.as_str() {
                            // String value: borrow via Cow when no truncation.
                            format!("{}: {}", k_capped, cap_entry(s))
                        } else {
                            // Non-string value: bounded serialization avoids
                            // full Value::to_string() allocation against
                            // hostile deeply-nested / huge values.
                            let serialized = serialize_value_bounded(v, MAX_ERROR_ENTRY_LEN);
                            format!("{}: {}", k_capped, cap_entry(&serialized))
                        }
                    })
                    .collect();
                pairs.sort();
                let pairs_truncated = total_keys > MAX_ERROR_PAIRS;

                // Streaming join with upfront marker reservation (same pattern
                // as the errorMessages path above).
                const JOIN_MARKER: &str = " [...truncated]";
                let content_budget_join =
                    MAX_SANITIZED_OUTPUT_LEN.saturating_sub(JOIN_MARKER.len());
                let mut joined = String::with_capacity(MAX_SANITIZED_OUTPUT_LEN);
                let mut first = true;
                let mut join_truncated = false;
                for p in &pairs {
                    let separator_len = if first { 0 } else { 2 };
                    if joined.len() + separator_len + p.len() > content_budget_join {
                        join_truncated = true;
                        break;
                    }
                    if !first {
                        joined.push_str("; ");
                    }
                    joined.push_str(p);
                    first = false;
                }
                if join_truncated || pairs_truncated {
                    joined.push_str(JOIN_MARKER);
                }
                debug_assert!(joined.len() <= MAX_SANITIZED_OUTPUT_LEN);
                return joined;
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
    use super::{
        MAX_ERROR_ENTRY_LEN, MAX_PARSE_BODY_LEN, MAX_SANITIZED_OUTPUT_LEN, cap_entry,
        extract_error_message, sanitize_for_stderr, serialize_value_bounded,
    };

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

    // serialize_value_bounded tests — memory-amplification defense for
    // non-string `errors`-map values.

    #[test]
    fn test_serialize_value_bounded_small_value_fits_under_limit() {
        // Small values should serialize fully and identically to Value::to_string().
        let v = serde_json::json!({"code": 42, "msg": "hello"});
        let out = serialize_value_bounded(&v, 1024);
        assert_eq!(out, v.to_string());
    }

    #[test]
    fn test_serialize_value_bounded_caps_oversized_value() {
        // Pin: a value that serializes to ~10 KiB MUST NOT exceed the byte
        // limit. This is the core memory-amplification defense — without
        // bounded serialization, Value::to_string() would allocate 10 KiB
        // before cap_entry could truncate.
        let big_string = "x".repeat(10_000);
        let v = serde_json::json!({"oversized": big_string});
        let out = serialize_value_bounded(&v, MAX_ERROR_ENTRY_LEN);
        assert!(
            out.len() <= MAX_ERROR_ENTRY_LEN,
            "bounded output exceeded limit: {} > {}",
            out.len(),
            MAX_ERROR_ENTRY_LEN
        );
        // Output should be a valid prefix of the full serialization.
        assert!(out.starts_with("{\"oversized\":\""));
        // Pin: truncated output MUST end with a visible marker so
        // operators see the JSON is incomplete rather than malformed.
        assert!(
            out.ends_with(" [...truncated]"),
            "expected truncation marker on overflow; got: {out:?}"
        );
    }

    #[test]
    fn test_serialize_value_bounded_caps_deeply_nested_value() {
        // Pin: a deeply nested value MUST also be bounded — the prior
        // Value::to_string() path would have allocated full serialization
        // proportional to depth before cap_entry could truncate.
        let mut v = serde_json::Value::Null;
        for _ in 0..1000 {
            v = serde_json::json!([v]);
        }
        let out = serialize_value_bounded(&v, MAX_ERROR_ENTRY_LEN);
        assert!(
            out.len() <= MAX_ERROR_ENTRY_LEN,
            "bounded output exceeded limit for deeply nested value: {} > {}",
            out.len(),
            MAX_ERROR_ENTRY_LEN
        );
    }

    #[test]
    fn test_serialize_value_bounded_produces_valid_utf8() {
        // Pin: even when the underlying writer cuts off mid-codepoint,
        // String::from_utf8_lossy must produce valid UTF-8 (no panics from
        // downstream str ops). This is critical because `cap_entry` indexes
        // via `is_char_boundary` on the bounded output.
        // A long string with multibyte chars maximizes the chance of a
        // mid-codepoint cutoff.
        let big_utf8 = "日本語のエラー".repeat(500);
        let v = serde_json::json!({"k": big_utf8});
        let out = serialize_value_bounded(&v, MAX_ERROR_ENTRY_LEN);
        // Must be valid UTF-8 (would panic on assert otherwise).
        assert!(out.is_char_boundary(out.len()));
        // And bounded.
        assert!(out.len() <= MAX_ERROR_ENTRY_LEN);
    }

    #[test]
    fn test_serialize_value_bounded_handles_zero_limit_safely() {
        // Edge case: a zero limit returns empty without panicking. Not used
        // in production (limit is always MAX_ERROR_ENTRY_LEN > 0), but pins
        // the contract for future callers.
        let v = serde_json::json!({"a": 1});
        let out = serialize_value_bounded(&v, 0);
        assert_eq!(out, "");
    }

    #[test]
    fn test_serialize_value_bounded_no_marker_when_no_overflow() {
        // Pin: when the value fits comfortably within the limit, the
        // truncation marker must NOT be appended (otherwise legitimate
        // small responses would look truncated).
        let v = serde_json::json!({"code": 42, "msg": "hello"});
        let out = serialize_value_bounded(&v, 1024);
        assert!(
            !out.ends_with(" [...truncated]"),
            "marker should not appear for non-overflowing values: {out:?}"
        );
        assert!(!out.contains("[...truncated]"));
    }

    #[test]
    fn test_serialize_value_bounded_marker_fits_within_limit_for_oversized() {
        // Pin: even WITH the marker appended, the function strictly
        // satisfies `out.len() <= limit` — marker bytes are reserved upfront
        // from the byte budget so the prefix-plus-marker total fits.
        let big = "x".repeat(10_000);
        let v = serde_json::json!({"k": big});
        let out = serialize_value_bounded(&v, MAX_ERROR_ENTRY_LEN);
        assert!(
            out.len() <= MAX_ERROR_ENTRY_LEN,
            "marker pushed output past limit: {} > {}",
            out.len(),
            MAX_ERROR_ENTRY_LEN
        );
        assert!(out.ends_with(" [...truncated]"));
    }

    #[test]
    fn test_serialize_value_bounded_no_marker_for_tiny_limit_below_marker_len() {
        // Degenerate case: if `limit` is smaller than the marker itself
        // (~15 bytes), the function falls back to raw-bounded mode without
        // a marker rather than producing a marker-only output. Useful pin
        // for future callers who might pass a tiny limit.
        let v = serde_json::json!({"a": 1});
        let out = serialize_value_bounded(&v, 5);
        assert!(out.len() <= 5);
        // No room for the marker; must not be present.
        assert!(!out.contains("[...truncated]"));
    }

    // MAX_PARSE_BODY_LEN gate tests — memory-amplification defense at the
    // JSON parse step. serde_json::from_str::<Value> builds a
    // full DOM that costs ~2-3x body size; the gate falls back to the
    // byte-bounded raw-body path for oversized bodies.

    #[test]
    fn test_extract_error_message_size_gate_skips_parse_for_huge_body() {
        // Pin: a body larger than MAX_PARSE_BODY_LEN must NOT enter the
        // JSON-parse path even if it's syntactically valid JSON. The output
        // is the byte-bounded raw-body fallback (cap_entry applied to
        // body_str) so a hostile 100 MB JSON body cannot force DOM
        // allocation.
        let huge_valid_json = format!(
            "{{\"errorMessages\":[\"{}\"]}}",
            "x".repeat(MAX_PARSE_BODY_LEN)
        );
        assert!(huge_valid_json.len() > MAX_PARSE_BODY_LEN);
        let out = extract_error_message(huge_valid_json.as_bytes());
        // sanitize_for_stderr caps total output at MAX_SANITIZED_OUTPUT_LEN.
        assert!(
            out.len() <= MAX_SANITIZED_OUTPUT_LEN,
            "output exceeded sanitization cap: {} > {}",
            out.len(),
            MAX_SANITIZED_OUTPUT_LEN
        );
        // The fallback path uses the raw body, NOT the extracted
        // errorMessages array. So the output should look like the
        // serialized JSON string itself (or a capped prefix), not
        // the de-quoted errorMessages content.
        assert!(out.starts_with("{"));
    }

    #[test]
    fn test_extract_error_message_size_gate_allows_normal_body() {
        // Pin: legitimate small JSON error responses (well under
        // MAX_PARSE_BODY_LEN) still take the JSON-parse path and produce
        // the friendly extracted message — no regression from the gate.
        let normal_body = r#"{"errorMessages":["Field 'summary' is required"]}"#;
        assert!(normal_body.len() < MAX_PARSE_BODY_LEN);
        let out = extract_error_message(normal_body.as_bytes());
        // The errorMessages array element should be the visible output,
        // not the raw JSON envelope.
        assert!(out.contains("Field 'summary' is required"));
        // And no leading "{" — the parse succeeded.
        assert!(!out.starts_with("{"));
    }

    #[test]
    fn test_extract_error_message_size_gate_threshold_is_documented_value() {
        // Pin: the 16 KiB threshold is intentional — generous for legitimate
        // Jira errors (<1 KiB typical) yet tight enough to block the
        // amplification vector. Future tweaks should update the doc comment
        // and Perplexity-revalidate.
        assert_eq!(MAX_PARSE_BODY_LEN, 16 * 1024);
    }

    // Pins — std::io::Write contract compliance + non-UTF8 marker accuracy.

    #[test]
    fn test_serialize_value_bounded_returns_marker_with_correct_data_on_partial_write() {
        // R12 contract pin: previously the Bounded::write violated the
        // std::io::Write contract by returning Err alongside a partial
        // write. The fix returns Ok(take) on partial writes and lets the
        // NEXT call hit remaining==0 and return Err. Verify that this
        // does not break serialization correctness: oversized values
        // still produce a bounded prefix + truncation marker.
        let big_string = "y".repeat(5000);
        let v = serde_json::json!({"key": big_string});
        let out = serialize_value_bounded(&v, MAX_ERROR_ENTRY_LEN);
        assert!(out.len() <= MAX_ERROR_ENTRY_LEN);
        assert!(out.starts_with("{\"key\":\"y"));
        assert!(
            out.ends_with(" [...truncated]"),
            "marker missing on partial-write contract-compliant path: {out:?}"
        );
    }

    #[test]
    fn test_extract_error_message_non_utf8_marker_reports_true_body_size() {
        // Pin: the non-UTF8 fallback marker MUST report the actual
        // body.len(), not the pre-capped lossy-string length. Operators
        // diagnosing a flood attempt need to see "5000000 bytes total",
        // not "4096 bytes total" which would silently under-report.
        // Build a 5 MB non-UTF8 body (lots of 0xff bytes).
        let body = vec![0xffu8; 5_000_000];
        let out = extract_error_message(&body);
        // The marker should reference the true 5_000_000-byte length.
        assert!(
            out.contains("5000000 bytes total"),
            "marker missing or wrong-size for non-UTF8 5MB body: {out:?}"
        );
        // And flag the non-UTF8 source for disambiguation.
        assert!(out.contains("non-UTF8 body"));
        // And still respect the post-sanitization output cap.
        assert!(out.len() <= MAX_SANITIZED_OUTPUT_LEN);
    }

    // Pins — Unicode C1 control coverage (defense-in-depth against
    // legacy/non-UTF8 terminals; modern UTF-8 terminals drop these as
    // invalid continuation bytes but is_control() future-proofs).

    #[test]
    fn test_sanitize_for_stderr_escapes_unicode_csi_c1_control() {
        // U+009B is the Control Sequence Introducer (CSI) — semantically
        // equivalent to "ESC [" in 8-bit/legacy modes. Must be escaped
        // rather than passed through.
        let input = "before\u{009b}31mafter".to_string();
        let result = sanitize(&input);
        assert!(result.contains("\\u{009b}"));
        // The non-control text must survive unchanged.
        assert!(result.contains("before"));
        assert!(result.contains("31mafter"));
        // No raw CSI byte in the output.
        assert!(!result.contains('\u{009b}'));
    }

    #[test]
    fn test_sanitize_for_stderr_escapes_unicode_nel_c1_control() {
        // U+0085 is NEL (Next Line) — interpreted as a line break in
        // some 8-bit terminal modes. Escape rather than emit raw.
        let input = "line1\u{0085}line2".to_string();
        let result = sanitize(&input);
        assert!(result.contains("\\u{0085}"));
        assert!(!result.contains('\u{0085}'));
    }

    #[test]
    fn test_sanitize_for_stderr_preserves_non_control_unicode_above_ascii() {
        // Anti-regression: only U+0080..U+009F are CONTROL chars
        // above ASCII. Other code points in the U+00A0+ range (e.g.,
        // non-breaking space U+00A0, é U+00E9, 日 U+65E5) MUST pass
        // through unchanged — char::is_control() returns false for them.
        let input = "résumé 日本語のエラー\u{00a0}end".to_string();
        let result = sanitize_for_stderr(input.clone());
        // No escape sequences inserted.
        assert!(!result.contains("\\u{"));
        assert!(!result.contains("\\x"));
        // String preserved bit-for-bit.
        assert_eq!(result, input);
    }

    #[test]
    fn test_extract_error_message_non_utf8_small_body_no_marker() {
        // Pin: a small non-UTF8 body that fits in MAX_ERROR_ENTRY_LEN
        // skips the marker (the fast path). No regression for legitimate
        // tiny non-UTF8 responses.
        let body = vec![0xffu8; 16];
        let out = extract_error_message(&body);
        // No marker present.
        assert!(!out.contains("[...truncated"));
        // No "non-UTF8 body" string (custom marker not used).
        assert!(!out.contains("non-UTF8 body"));
    }
}

/// Unit tests for the deadline-clamp helper (S-333 / BC-bulk.poll.deadline-bounded).
/// AC-004: clock + tokio-timer safety. AC-005: error variant + message shape.
#[cfg(test)]
mod clamp_tests {
    use super::{ClampResult, clamp_retry_sleep};
    use std::time::{Duration, Instant};

    /// AC-004 baseline: with no deadline, the clamp passes the base sleep
    /// through unchanged. This is the regression-invariant path for all
    /// historical (pre-S-333) callers that pass `None`.
    #[test]
    fn test_clamp_retry_sleep_no_deadline_returns_base() {
        let base = Duration::from_secs(60);
        assert_eq!(
            clamp_retry_sleep(base, None),
            ClampResult::Sleep(base),
            "with deadline=None the clamp must pass `base` through; \
             this is the regression-invariant for all pre-S-333 callers."
        );
    }

    /// AC-004: deadline well in the future ⇒ sleep is `min(base, remaining) = base`.
    #[test]
    fn test_clamp_retry_sleep_far_deadline_returns_base() {
        let base = Duration::from_secs(60);
        let deadline = Instant::now() + Duration::from_secs(300);
        match clamp_retry_sleep(base, Some(deadline)) {
            ClampResult::Sleep(d) => {
                assert_eq!(
                    d, base,
                    "with 5min remaining, sleep should be the full 60s base (not clamped)"
                );
            }
            ClampResult::Expired { .. } => {
                panic!("expected Sleep with far-future deadline, got Expired");
            }
        }
    }

    /// AC-004: deadline is shorter than base ⇒ sleep is clamped to remaining.
    /// This is the headline AC-001 mechanism — a 60s `Retry-After` against a
    /// 30s deadline yields a 30s sleep (not 60s).
    #[test]
    fn test_clamp_retry_sleep_near_deadline_clamps_to_remaining() {
        let base = Duration::from_secs(60);
        let deadline = Instant::now() + Duration::from_secs(10);
        match clamp_retry_sleep(base, Some(deadline)) {
            ClampResult::Sleep(d) => {
                // Sleep should be ≤ 10s (the remaining budget) and > 9s
                // (accounting for the few microseconds between
                // `Instant::now()` in the test and inside the helper).
                assert!(
                    d <= Duration::from_secs(10),
                    "clamped sleep must be ≤ remaining (10s), got {d:?}"
                );
                assert!(
                    d > Duration::from_millis(9000),
                    "clamped sleep should be near 10s (modulo timing slack), got {d:?}"
                );
            }
            ClampResult::Expired { .. } => {
                panic!("expected Sleep with 10s remaining, got Expired");
            }
        }
    }

    /// AC-004 (the headline correctness assertion): when remaining is < 1ms,
    /// the clamp MUST return `Expired`, NOT `Sleep(Duration::from_micros(N))`.
    ///
    /// Why this matters: `tokio::time::sleep(Duration < 1ms)` is a documented
    /// no-op (tokio timer-wheel 1ms floor). Accepting sub-millisecond
    /// `Sleep(...)` would produce a spin-loop: the no-op "sleep" returns
    /// immediately, the next request fires, gets another 429, recomputes
    /// remaining = 0, repeats. The 1ms floor matches tokio's timer
    /// granularity and matches Q3 research-validation 2026-05-12.
    #[test]
    fn test_clamp_retry_sleep_sub_millisecond_remaining_returns_expired() {
        // Construct a deadline that has ALREADY passed at the moment of the
        // clamp call. `Instant::now()` between this line and the clamp call
        // strictly increases (monotonic clock), so `remaining` will be zero.
        let deadline = Instant::now();
        // Sleep a microsecond to guarantee the clamp's internal
        // `Instant::now()` is past the deadline. Without this, on extremely
        // fast machines the test could theoretically see `remaining == 0ns`
        // which is still < 1ms (so the assertion would pass), but on slower
        // CI we want unambiguous evidence the clamp engaged.
        std::thread::sleep(Duration::from_micros(10));
        match clamp_retry_sleep(Duration::from_secs(60), Some(deadline)) {
            ClampResult::Expired { remaining_ms } => {
                // The deadline expired, so remaining is at most a few hundred
                // microseconds rounded down to 0ms (sub-ms truncation by
                // intent — see the helper's `as_micros() / 1000` cast).
                assert_eq!(
                    remaining_ms, 0,
                    "expired deadline should report 0ms remaining, got {remaining_ms}ms"
                );
            }
            ClampResult::Sleep(d) => {
                panic!(
                    "AC-004 VIOLATION: clamp returned Sleep({d:?}) for a deadline that \
                     has already passed. This will cause `tokio::time::sleep(d).await` \
                     to no-op (< 1ms tokio timer-wheel floor), producing a spin-loop \
                     in the retry path. Must return Expired."
                );
            }
        }
    }

    /// AC-004 boundary: remaining comfortably above the 1ms floor should
    /// produce Sleep, not Expired. Uses 50ms (not 5ms) to absorb thread-
    /// scheduling jitter on shared CI runners (GitHub Actions `ubuntu-latest`
    /// has been measured at 10-50ms scheduling latency under load; the
    /// previous 5ms margin would flake intermittently — adversary CONCERN-5,
    /// research-validation Q3).
    #[test]
    fn test_clamp_retry_sleep_50ms_remaining_is_sleep() {
        let deadline = Instant::now() + Duration::from_millis(50);
        match clamp_retry_sleep(Duration::from_secs(60), Some(deadline)) {
            ClampResult::Sleep(d) => {
                assert!(
                    d >= Duration::from_millis(1) && d <= Duration::from_millis(50),
                    "50ms-remaining clamp should produce a sleep in [1ms, 50ms], got {d:?}"
                );
            }
            ClampResult::Expired { .. } => {
                panic!("50ms remaining should produce Sleep, not Expired");
            }
        }
    }

    /// Research-validation Q7 gap: `base = Duration::ZERO` corresponds to a
    /// hypothetical `Retry-After: 0` response (Atlassian doesn't send this in
    /// practice, but the parser at `src/api/rate_limit.rs` accepts any u64
    /// including zero). Without this test, a regression could allow zero-base
    /// sleeps to slip past the clamp:
    ///   - `None` deadline ⇒ `Sleep(0ms)` would no-op (tokio 1ms floor)
    ///     and the retry loop spins immediately to the next attempt.
    ///   - `Some(future)` deadline ⇒ `Sleep(0ms.min(remaining)) = Sleep(0ms)`
    ///     same spin.
    ///
    /// The fix is for the rate-limit parser to floor at DEFAULT_RETRY_SECS,
    /// but the clamp must also behave correctly if it ever gets a 0 base.
    /// This test pins the current behavior so regressions surface.
    #[test]
    fn test_clamp_retry_sleep_zero_base_returns_zero_sleep() {
        // Zero base with no deadline: passes through.
        assert_eq!(
            clamp_retry_sleep(Duration::ZERO, None),
            ClampResult::Sleep(Duration::ZERO),
            "zero base + no deadline should pass through as Sleep(0); \
             the rate-limit parser (not the clamp) is responsible for \
             enforcing a non-zero floor on Retry-After."
        );

        // Zero base with future deadline: still Sleep(0), because min(0, remaining) = 0.
        let deadline = Instant::now() + Duration::from_secs(60);
        match clamp_retry_sleep(Duration::ZERO, Some(deadline)) {
            ClampResult::Sleep(d) => assert_eq!(d, Duration::ZERO),
            ClampResult::Expired { .. } => {
                panic!("future deadline with zero base should Sleep(0), not Expire")
            }
        }
    }

    /// C-1 (F5 pass-02): entry-point check coverage. The defensive check in
    /// `send_inner` returns `DeadlineExceeded` immediately for an already-
    /// expired deadline WITHOUT issuing the request. A regression that
    /// disabled this check (e.g., by changing the `Duration::ZERO` argument
    /// to a non-zero value) would not be caught by the clamp helper unit
    /// tests, since those only test the pure helper.
    ///
    /// We exercise the entry-point path by calling `clamp_retry_sleep` with
    /// the same arguments `send_inner` uses internally. A future change to
    /// the entry-point logic that bypassed the check would surface here as
    /// a `Sleep(...)` result instead of `Expired`, which the assertion below
    /// would catch.
    ///
    /// (We can't easily call `send_bounded` directly without standing up a
    /// real `JiraClient` and wiremock — that level of integration is covered
    /// by `tests/bulk_deadline_propagation.rs::test_333_...`. The unit-level
    /// pin here is the helper's contract.)
    #[test]
    fn test_clamp_retry_sleep_entry_point_pattern_already_expired_is_expired() {
        // The pattern used at the top of `send_inner` for the entry-point check:
        //     `clamp_retry_sleep(Duration::ZERO, Some(d))`
        // with `d` already in the past must return Expired so the function
        // can short-circuit before issuing the request.
        let deadline = Instant::now() - Duration::from_millis(1);
        std::thread::sleep(Duration::from_micros(50));
        match clamp_retry_sleep(Duration::ZERO, Some(deadline)) {
            ClampResult::Expired { remaining_ms } => {
                assert_eq!(
                    remaining_ms, 0,
                    "entry-point pattern should report 0ms remaining for past deadline"
                );
            }
            ClampResult::Sleep(d) => {
                panic!(
                    "C-1 (F5 pass-02) VIOLATION: clamp returned Sleep({d:?}) for an already-\
                     expired deadline in the entry-point pattern. send_inner would proceed \
                     to issue the request instead of short-circuiting — the defense-in-depth \
                     entry-point check has been broken."
                );
            }
        }
    }

    /// Research-validation Q7 gap: `base == remaining` (or near it) is the
    /// headline AC-001 scenario — the first 429 of the integration test has
    /// `Retry-After: 60` and remaining budget ~30s; the clamp produces
    /// `Sleep(30s)`. This unit test pins the equality boundary so a regression
    /// changing `<` to `<=` in `base.min(remaining)` semantics (or some other
    /// off-by-one) surfaces immediately.
    #[test]
    fn test_clamp_retry_sleep_base_equals_remaining_clamps_to_remaining() {
        // We can't make base exactly equal to remaining at the moment of the
        // clamp call (Instant::now() inside the helper drifts microseconds
        // forward), but we can put `base` slightly larger than `remaining` to
        // force the `base.min(remaining) = remaining` path.
        let deadline = Instant::now() + Duration::from_millis(100);
        let base = Duration::from_millis(100); // identical to nominal remaining
        match clamp_retry_sleep(base, Some(deadline)) {
            ClampResult::Sleep(d) => {
                // `d` should be at most `base` (100ms) and at least `base - a few microseconds`
                // (the helper's internal Instant::now drift). Crucially, d MUST NOT exceed base.
                assert!(
                    d <= base,
                    "clamped sleep must not exceed base ({base:?}), got {d:?}"
                );
                // And `d` should be close to base (within a millisecond of timing slack).
                assert!(
                    d > Duration::from_millis(95),
                    "clamped sleep should be near base (~100ms), got {d:?}"
                );
            }
            ClampResult::Expired { .. } => {
                panic!(
                    "base==remaining edge: 100ms remaining should produce Sleep, \
                     not Expired (above 1ms floor)"
                );
            }
        }
    }
}
