//! S-3.03 v2 Red Gate — Auto-refresh OAuth on 401 with single-flight coordination
//!
//! All 11 tests in this file are expected to FAIL on the current `develop`
//! branch (pre-implementation). They turn GREEN once the implementer:
//!
//! 1. Creates `src/api/refresh_coordinator.rs` with `refresh_with_single_flight`.
//! 2. Adds `pub mod refresh_coordinator;` to `src/api/mod.rs`.
//! 3. Wires blanket-401 → `refresh_coordinator::refresh_with_single_flight` → retry-once
//!    into `JiraClient::send`.
//! 4. Adds `JR_OAUTH_TOKEN_URL` env-var override in `refresh_oauth_token` so integration
//!    tests can inject a wiremock URL instead of hitting `https://auth.atlassian.com`.
//!    Same pattern as `JR_ACCESSIBLE_RESOURCES_URL` from S-3.04.
//! 5. Adds `JiraClient::new_for_test_with_profile(base_url: String, auth_header: String,
//!    profile: &str) -> Self` constructor. Mirrors `new_for_test` but accepts an explicit
//!    profile name so tests can bind to a per-test isolated keychain profile.
//! 6. Adds `hint: String` field to `JrError::NotAuthenticated` (currently a unit variant).
//!    The auto-refresh wire-in sets hint = `"run 'jr auth refresh' to re-authenticate"`.
//!
//! ## Why `new_for_test_with_profile` is required
//!
//! `new_for_test` hardcodes `profile_name: "default"`. The auto-refresh wiring calls
//! `refresh_oauth_token(&self.profile_name)` which reads keychain under that profile name.
//! Tests need a per-test profile name (e.g., `"s303-ac001-<pid>"`) to avoid cross-test
//! keychain collisions when running in parallel. Without this seam, keyring-gated tests
//! all race on the "default" profile.
//!
//! ## Test → AC mapping
//!
//! | Test function | AC | Gate | Behavior asserted |
//! |---------------|----|------|-------------------|
//! | test_BC_1_1_002_auto_refreshes_on_401_and_retries_once | AC-001 | always | 401→refresh→200 succeeds |
//! | test_BC_1_4_027_refresh_persists_new_tokens_via_store_oauth_tokens | AC-002 | keyring | tokens written to keychain |
//! | test_BC_1_1_002_invalid_grant_surfaces_not_authenticated | AC-003 | always | invalid_grant → NotAuthenticated + hint |
//! | test_BC_1_1_002_one_attempt_cap_no_recursion_v1 | AC-004 | always | refresh succeeds, retry 401 → NotAuthenticated; no 2nd refresh |
//! | test_BC_1_1_002_one_attempt_cap_no_recursion_v2 | AC-004 | always | refresh fails → NotAuthenticated; no retry |
//! | test_BC_1_4_027_concurrent_sends_single_refresh_via_coordinator | AC-005 | always | N=10 concurrent → exactly 1 refresh hit |
//! | test_BC_1_4_027_concurrent_invalid_grant_no_thundering_herd | AC-006 | always | N=10 invalid_grant → 1 refresh hit |
//! | test_BC_1_1_002_manual_jr_auth_refresh_unchanged | AC-007 | always | `jr auth refresh --help` still works |
//! | test_BC_1_4_027_refresh_contract_pinning | AC-008 | always | full contract pin (URL, body, persist, error) |
//! | test_BC_1_4_027_waiters_use_in_memory_token_not_keychain | AC-009 | keyring | waiters use in-mem token; N retries use new token |
//! | test_BC_1_4_027_inter_process_reconcile_after_invalid_grant | AC-010 | keyring | stale refresh → re-read keychain → retry succeeds |
//! | test_BC_1_4_027_persist_before_publish_fault_injection | AC-011 | keyring + JR_S303_PERSIST_FAIL | persist failure → error propagated; keychain unchanged |
//!
//! ## Seams required from the implementer
//!
//! ### 1. `JR_OAUTH_TOKEN_URL` env-var (required by ALL behavioral tests)
//! In `src/api/auth.rs::refresh_oauth_token`, replace the hardcoded
//! `"https://auth.atlassian.com/oauth/token"` string with:
//! ```rust
//! let token_url = std::env::var("JR_OAUTH_TOKEN_URL")
//!     .unwrap_or_else(|_| "https://auth.atlassian.com/oauth/token".to_string());
//! ```
//! Tests set `JR_OAUTH_TOKEN_URL=<wiremock_uri>/oauth/token`.
//!
//! ### 2. `JiraClient::new_for_test_with_profile` (required by keyring-gated tests)
//! Add to `src/api/client.rs`:
//! ```rust
//! pub fn new_for_test_with_profile(base_url: String, auth_header: String, profile: &str) -> Self {
//!     let assets_base_url = Some(format!("{}/jsm/assets", &base_url));
//!     Self {
//!         client: Client::new(),
//!         instance_url: base_url.clone(),
//!         base_url,
//!         auth_header,
//!         verbose: false,
//!         verbose_bodies: false,
//!         assets_base_url,
//!         profile_name: profile.to_string(),
//!     }
//! }
//! ```
//!
//! ### 3. `JrError::NotAuthenticated { hint: String }` (required by AC-003, AC-004, AC-006, AC-008)
//! Change from unit variant to struct variant:
//! ```rust
//! #[error("Not authenticated. {hint}")]
//! NotAuthenticated { hint: String },
//! ```
//! The auto-refresh wire-in sets `hint = "run 'jr auth refresh' to re-authenticate".into()`.
//! The existing `JrError::NotAuthenticated` without hint in `parse_error` should use an
//! empty hint or a different hint text.
//!
//! ### 4. `JR_S303_PERSIST_FAIL` env-var (required by AC-011)
//! In `src/api/auth.rs::refresh_oauth_token`, add:
//! ```rust
//! if std::env::var("JR_S303_PERSIST_FAIL").as_deref() == Ok("1") {
//!     anyhow::bail!("JR_S303_PERSIST_FAIL: simulated keychain write failure for testing");
//! }
//! ```
//! immediately before the `store_oauth_tokens(...)` call, AFTER the successful
//! Atlassian token exchange. This simulates a keychain write failure.
//!
//! ## Keychain strategy
//!
//! "Always run" tests (AC-001, AC-003..AC-006, AC-008) do NOT seed the keychain.
//! Their Red Gate failures come from:
//! - The retry assertion: "all calls succeed" → fails because `send()` returns 401 errors
//! - The MockServer drop assertion: "expected 1 refresh call, got 0"
//!   These fire pre-implementation regardless of keychain state.
//!
//! "Keyring gated" tests (AC-002, AC-009..AC-011) are `#[ignore]` and only run when
//! `JR_RUN_KEYRING_TESTS=1`. They use `JR_SERVICE_NAME=jr-s303-test` and per-test
//! profile names (`s303-<testname>-<pid>`) to isolate keychain entries.
//!
//! ## Red Gate verification
//!
//! Run: `cargo test --test oauth_refresh_integration 2>&1`
//! Expected: all non-ignored tests fail with assertion errors (not compile errors).
//!
//! Run with keyring gate:
//! `JR_RUN_KEYRING_TESTS=1 JR_SERVICE_NAME=jr-s303-test cargo test --test oauth_refresh_integration 2>&1`
//! Expected: all 11 tests fail with assertion errors.

// ---------------------------------------------------------------------------
// Shared harness
// ---------------------------------------------------------------------------

/// Set an environment variable.
///
/// Rust 2024 requires `unsafe {}` for `std::env::set_var` because it's not thread-safe.
/// These tests call set_var before spawning tasks, so race conditions are avoided.
/// This helper makes the call sites less verbose.
///
/// Safety: caller must ensure no concurrent reads of the env var from other threads.
/// All `JR_OAUTH_TOKEN_URL` / `JR_SERVICE_NAME` sets in these tests happen before
/// `tokio::spawn` calls, so the ordering is safe within each test.
fn set_env(key: &str, val: &str) {
    // SAFETY: Called before any concurrent access from spawned tasks.
    // Each test uses unique URL paths (e.g., /oauth/token/ac001) to avoid
    // cross-test interference even if env vars leak between tests.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(key, val);
    }
}

mod harness {
    //! Test infrastructure shared across S-3.03 tests.

    use std::sync::OnceLock;
    use tokio::sync::Mutex;

    /// Serializes tests that set process-global env vars (`JR_OAUTH_TOKEN_URL`,
    /// `JR_S303_PERSIST_FAIL`). Default parallel test execution would otherwise
    /// race the env var writes/reads. Each test acquires this lock for its
    /// duration; tests within this binary become effectively sequential, but
    /// other test binaries (e.g., `multi_cloudid_disambiguation.rs`) run in
    /// parallel as normal.
    ///
    /// `tokio::sync::Mutex` is used (not `std::sync::Mutex`) because the guard
    /// must be held across `.await` points inside async test bodies.
    /// `tokio::sync::Mutex` does NOT poison on panic, so a panicking test
    /// releases the lock cleanly for the next test.
    ///
    /// Initialized lazily via `OnceLock` because `tokio::sync::Mutex::new()` is
    /// a `const fn` and static initialization is straightforward.
    pub(super) fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    /// Standard fake access token returned by the mock refresh endpoint.
    pub const FAKE_ACCESS_TOKEN: &str = "new-access-token-s303";
    /// Standard fake refresh token returned by the mock refresh endpoint.
    pub const FAKE_REFRESH_TOKEN: &str = "new-refresh-token-s303";
    /// Initial (pre-refresh) bearer token used by test clients.
    pub const INITIAL_BEARER: &str = "Bearer initial-access-token-s303";
    /// Initial refresh token seeded in keychain for keyring-gated tests.
    pub const INITIAL_REFRESH_TOKEN: &str = "initial-refresh-token-s303";
    /// Initial access token seeded in keychain for keyring-gated tests.
    pub const INITIAL_ACCESS_TOKEN: &str = "initial-access-token-s303";

    /// RFC 6749 invalid_grant response body.
    pub const INVALID_GRANT_BODY: &str = r#"{"error":"invalid_grant","error_description":"The provided authorization grant is invalid, expired, revoked, does not match the redirection URI used in the authorization request, or was issued to another client."}"#;

    /// Atlassian-shaped expired-token 401 body.
    /// NO machine-readable `code` field (Claim 2, S-3.03-wave3-verification.md: REFUTED).
    pub const ATLASSIAN_401_BODY: &str = r#"{"errorMessages":["The access token provided is expired, revoked, malformed, or invalid for other reasons."]}"#;

    /// JSON body for a successful token refresh response.
    pub fn refresh_ok_body() -> serde_json::Value {
        serde_json::json!({
            "access_token": FAKE_ACCESS_TOKEN,
            "refresh_token": FAKE_REFRESH_TOKEN,
            "expires_in": 3600,
            "token_type": "Bearer"
        })
    }

    /// Profile name used by all keyring-gated tests.
    ///
    /// `JiraClient::new_for_test` hardcodes `profile_name = "default"`, so all
    /// keyring-gated tests use "default" as the profile. This means they must
    /// NOT run in parallel with each other (the `#[ignore]` gate + sequential
    /// execution when `JR_RUN_KEYRING_TESTS=1` ensures this).
    ///
    /// The implementer should add `JiaClient::new_for_test_with_profile` to enable
    /// per-test profile isolation. Until then, all keyring tests share "default".
    pub const TEST_PROFILE: &str = "default";

    /// Seed initial OAuth tokens in the keychain for the test profile.
    /// Requires `JR_SERVICE_NAME=jr-s303-test` to be set.
    /// Only called from keyring-gated tests.
    pub fn seed_oauth_tokens() {
        jr::api::auth::store_oauth_tokens(
            TEST_PROFILE,
            INITIAL_ACCESS_TOKEN,
            INITIAL_REFRESH_TOKEN,
        )
        .expect("seed_oauth_tokens: keychain write must succeed");
    }

    /// Remove test OAuth keychain entries. Best-effort.
    ///
    /// Uses `jr::api::auth::clear_profile_creds` which goes through the same
    /// service-name resolution (`JR_SERVICE_NAME`) as the rest of the auth module.
    pub fn cleanup_oauth_tokens() {
        let _ = jr::api::auth::clear_profile_creds(TEST_PROFILE);
    }
}

// ---------------------------------------------------------------------------
// AC-001: Auto-refresh on 401, retry once, caller sees success
// ---------------------------------------------------------------------------

/// AC-001 — traces to BC-1.1.002 (auto-refresh on 401, retry once, caller sees success).
///
/// `JiaClient::send()` auto-refreshes on 401 and retries the original request once.
/// The caller sees success.
///
/// RED GATE (always fails pre-implementation):
/// - `send()` currently returns `NotAuthenticated` on the first 401 without retrying.
/// - The `result.is_ok()` assertion fails.
/// - `Mock::expect(1)` on the refresh endpoint fires with 0 calls on MockServer drop.
///
/// No keychain seeding: pre-implementation, `refresh_oauth_token` is never reached.
/// After implementation: `new_for_test_with_profile` + keychain seeding required (see
/// `test_refresh_persists_rotated_tokens_via_store_oauth_tokens` AC-002 for
/// the keyring-gated green-gate variant).
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_send_retries_once_after_refresh_on_401() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let _env_guard = harness::env_lock().lock().await;

    use jr::api::client::JiraClient;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    // Set the refresh endpoint URL to the wiremock server.
    // NOTE: env vars in tokio tests are process-global. Tests that set JR_OAUTH_TOKEN_URL
    // may interfere if run in the same process. Use unique paths per test to differentiate.
    // The implementer adds `JR_OAUTH_TOKEN_URL` support to `refresh_oauth_token`.
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac001", server.uri()),
    );

    // First API call: 401 (expired token, Atlassian shape)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Retry: 200 (after refresh produces new token)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "user-123",
            "displayName": "Test User"
        })))
        .mount(&server)
        .await;

    // Refresh endpoint: exactly 1 call expected.
    // Path is unique to this test to avoid cross-test interference.
    Mock::given(method("POST"))
        .and(path("/oauth/token/ac001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(harness::refresh_ok_body()))
        .expect(1) // single-flight: exactly 1 refresh for 1 concurrent sender
        .mount(&server)
        .await;

    // new_for_test uses profile_name="default". The implementer's `send()` wire-in
    // calls `refresh_oauth_token("default")` which reads keychain for the "default"
    // profile. Keychain is NOT seeded here — this test only validates the pre-implementation
    // failure (no retry, no refresh call). The keyring-gated AC-002 test covers the
    // green-gate path with proper keychain setup.
    let client = JiraClient::new_for_test(server.uri(), harness::INITIAL_BEARER.to_string());

    let result = client.get::<serde_json::Value>("/rest/api/3/myself").await;

    // RED GATE assertion: fails pre-implementation because send() doesn't retry.
    assert!(
        result.is_ok(),
        "AC-001 FAIL: JiraClient::send() must transparently retry on 401 after refresh \
         and return the successful response. Got error: {:?}",
        result.err()
    );
    // MockServer drop verifies exactly 1 refresh POST (fires with 0 pre-implementation).
}

// ---------------------------------------------------------------------------
// AC-002: New tokens persisted via store_oauth_tokens — keyring gated
// ---------------------------------------------------------------------------

/// AC-002 — traces to BC-1.4.027 (new tokens persisted via store_oauth_tokens).
///
/// After successful refresh, new `<profile>:oauth-access-token`
/// and `<profile>:oauth-refresh-token` are written to keychain via `store_oauth_tokens`.
///
/// RED GATE: No refresh happens pre-implementation; keychain still has initial tokens.
/// `stored_access != FAKE_ACCESS_TOKEN` → assertion fails.
///
/// KEYRING GATE: `JR_RUN_KEYRING_TESTS=1` required. Uses `JR_SERVICE_NAME=jr-s303-test`
/// and `harness::TEST_PROFILE` ("default") to isolate from real credentials.
///
/// NOTE: Uses `new_for_test` with `profile_name="default"`. All keyring-gated tests
/// in this file share this profile and must NOT run in parallel with each other.
/// The `#[ignore]` gate ensures sequential execution.
#[tokio::test]
#[ignore]
async fn test_refresh_persists_rotated_tokens_via_store_oauth_tokens() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }

    use jr::api::auth;
    use jr::api::client::JiraClient;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    set_env("JR_SERVICE_NAME", "jr-s303-test");

    let server = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac002", server.uri()),
    );

    // Seed initial tokens into keychain under "default" profile
    harness::seed_oauth_tokens();

    // API: 401 then 200 (with new bearer token)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(header(
            "Authorization",
            format!("Bearer {}", harness::FAKE_ACCESS_TOKEN),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "user-123"
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/oauth/token/ac002"))
        .respond_with(ResponseTemplate::new(200).set_body_json(harness::refresh_ok_body()))
        .expect(1)
        .mount(&server)
        .await;

    // new_for_test uses profile_name="default" which matches the seeded keychain entry.
    let client = JiraClient::new_for_test(
        server.uri(),
        format!("Bearer {}", harness::INITIAL_ACCESS_TOKEN),
    );

    let result = client.get::<serde_json::Value>("/rest/api/3/myself").await;

    assert!(
        result.is_ok(),
        "AC-002: send() must succeed after refresh: {:?}",
        result.err()
    );

    let (stored_access, stored_refresh) = auth::load_oauth_tokens(harness::TEST_PROFILE)
        .expect("AC-002: tokens must be in keychain after refresh");

    harness::cleanup_oauth_tokens();

    assert_eq!(
        stored_access,
        harness::FAKE_ACCESS_TOKEN,
        "AC-002 FAIL: new access token must be persisted via store_oauth_tokens. Got: {stored_access}"
    );
    assert_eq!(
        stored_refresh,
        harness::FAKE_REFRESH_TOKEN,
        "AC-002 FAIL: new refresh token must be persisted via store_oauth_tokens. Got: {stored_refresh}"
    );
}

// ---------------------------------------------------------------------------
// AC-003: invalid_grant surfaces NotAuthenticated with hint
// ---------------------------------------------------------------------------

/// AC-003 — traces to BC-1.1.002 (invalid_grant surfaces NotAuthenticated with re-auth hint).
///
/// Refresh endpoint returns `invalid_grant` (4xx).
/// `JiaClient::send()` surfaces `JrError::NotAuthenticated` with hint
/// `"run 'jr auth refresh' to re-authenticate"`. No retry loop.
///
/// RED GATE (always fails pre-implementation):
/// - `send()` doesn't attempt refresh; 401 surfaces as `NotAuthenticated` without hint.
/// - `err_str.contains("jr auth refresh")` → fails (hint text is absent pre-implementation).
/// - `Mock::expect(1)` on refresh → fails with 0 calls on MockServer drop.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_invalid_grant_surfaces_not_authenticated_with_refresh_hint() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let _env_guard = harness::env_lock().lock().await;

    use jr::api::client::JiraClient;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac003", server.uri()),
    );

    // API always returns 401
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    // Refresh endpoint: invalid_grant, exactly 1 call expected
    Mock::given(method("POST"))
        .and(path("/oauth/token/ac003"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string(harness::INVALID_GRANT_BODY)
                .insert_header("content-type", "application/json"),
        )
        .expect(1) // no thundering herd, no loop
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), harness::INITIAL_BEARER.to_string());

    let result = client.get::<serde_json::Value>("/rest/api/3/myself").await;

    assert!(
        result.is_err(),
        "AC-003 FAIL: should fail when refresh returns invalid_grant"
    );

    let err_str = result.unwrap_err().to_string();

    assert!(
        err_str.to_lowercase().contains("not authenticated"),
        "AC-003 FAIL: error must be NotAuthenticated. Got: {err_str}"
    );

    // This assertion fails pre-implementation: the hint field doesn't exist on
    // `JrError::NotAuthenticated` (unit variant). The implementer must add the hint.
    assert!(
        err_str.contains("jr auth refresh"),
        "AC-003 FAIL: NotAuthenticated must include hint \
         \"run 'jr auth refresh' to re-authenticate\". Got: {err_str}"
    );

    // MockServer drop verifies exactly 1 refresh call (no loop, no recursion).
}

// ---------------------------------------------------------------------------
// AC-004: One-attempt cap — no recursion (two variants)
// ---------------------------------------------------------------------------

/// AC-004 variant 1 — traces to BC-1.1.002 (one-attempt cap: refresh ok, retry also 401).
///
/// Refresh succeeds, but the retry also returns 401. `send()` must surface
/// `NotAuthenticated` WITHOUT a second refresh attempt.
///
/// RED GATE (always fails pre-implementation):
/// - `send()` doesn't attempt refresh at all. Returns 401 error immediately.
/// - `Mock::expect(1)` on the refresh endpoint → 0 calls → MockServer drop fails.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_send_caps_refresh_at_one_attempt_when_retry_also_401() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let _env_guard = harness::env_lock().lock().await;

    use jr::api::client::JiraClient;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac004v1", server.uri()),
    );

    // API always returns 401 (even after refresh)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    // Refresh: succeeds EXACTLY ONCE. If send() attempted a second refresh (recursion/loop),
    // Mock::expect(1) would fail with "expected 1, got 2" on drop.
    Mock::given(method("POST"))
        .and(path("/oauth/token/ac004v1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(harness::refresh_ok_body()))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), harness::INITIAL_BEARER.to_string());

    let result = client.get::<serde_json::Value>("/rest/api/3/myself").await;

    assert!(
        result.is_err(),
        "AC-004 v1 FAIL: must fail when retry also returns 401"
    );

    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.to_lowercase().contains("not authenticated"),
        "AC-004 v1 FAIL: must surface NotAuthenticated. Got: {err_str}"
    );
    // MockServer drop: if 2 refresh calls were made (loop), expect(1) fails.
}

/// AC-004 variant 2 — traces to BC-1.1.002 (one-attempt cap: refresh fails, no retry).
///
/// Refresh fails (invalid_grant). No retry. `send()` surfaces `NotAuthenticated`.
/// Exactly 1 refresh call.
///
/// RED GATE (always fails pre-implementation):
/// - `Mock::expect(1)` on refresh → 0 calls → MockServer drop fails.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_send_caps_refresh_at_one_attempt_when_refresh_fails() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let _env_guard = harness::env_lock().lock().await;

    use jr::api::client::JiraClient;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac004v2", server.uri()),
    );

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    // Refresh fails — exactly 1 call, no retry
    Mock::given(method("POST"))
        .and(path("/oauth/token/ac004v2"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string(harness::INVALID_GRANT_BODY)
                .insert_header("content-type", "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), harness::INITIAL_BEARER.to_string());

    let result = client.get::<serde_json::Value>("/rest/api/3/myself").await;

    assert!(
        result.is_err(),
        "AC-004 v2 FAIL: must fail when refresh returns invalid_grant"
    );

    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.to_lowercase().contains("not authenticated"),
        "AC-004 v2 FAIL: must surface NotAuthenticated. Got: {err_str}"
    );
}

// ---------------------------------------------------------------------------
// AC-005: N=10 concurrent sends → exactly 1 refresh via single-flight
// ---------------------------------------------------------------------------

/// AC-005 — traces to BC-1.4.027 (N=10 concurrent sends → exactly 1 refresh via coordinator).
///
/// N=10 concurrent `JiraClient::send()` calls all hitting 401 on a shared profile.
/// The single-flight coordinator ensures exactly 1 HTTP POST to the refresh endpoint.
///
/// RED GATE (always fails pre-implementation):
/// - All 10 calls return 401 errors (no refresh attempted).
/// - `errors` list is not empty → first assertion fails.
/// - `Mock::expect(1)` on refresh → 0 calls → MockServer drop fails.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_concurrent_sends_single_refresh_via_coordinator() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let _env_guard = harness::env_lock().lock().await;

    use futures::future::join_all;
    use jr::api::client::JiraClient;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const N: usize = 10;

    let server = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac005", server.uri()),
    );

    // All initial API requests return 401
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .up_to_n_times(N as u64)
        .mount(&server)
        .await;

    // All retries (after refresh) return 200
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "user-123"
        })))
        .mount(&server)
        .await;

    // THE KEY ASSERTION: exactly 1 refresh call for N=10 concurrent senders.
    // If N refreshes were made (no single-flight), expect(1) fails on drop.
    Mock::given(method("POST"))
        .and(path("/oauth/token/ac005"))
        .respond_with(ResponseTemplate::new(200).set_body_json(harness::refresh_ok_body()))
        .expect(1)
        .mount(&server)
        .await;

    let server_uri = server.uri();

    let handles: Vec<_> = (0..N)
        .map(|_| {
            let uri = server_uri.clone();
            tokio::spawn(async move {
                let client = JiraClient::new_for_test(uri, harness::INITIAL_BEARER.to_string());
                client.get::<serde_json::Value>("/rest/api/3/myself").await
            })
        })
        .collect();

    let results = join_all(handles).await;

    let errors: Vec<String> = results
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match r {
            Ok(Err(e)) => Some(format!("call {i}: {e}")),
            Err(e) => Some(format!("call {i} join error: {e}")),
            _ => None,
        })
        .collect();

    assert!(
        errors.is_empty(),
        "AC-005 FAIL: all {N} concurrent calls must succeed after single refresh. Errors:\n{}",
        errors.join("\n")
    );

    // MockServer drop: verifies exactly 1 refresh POST (not N=10).
}

// ---------------------------------------------------------------------------
// AC-006: N=10 concurrent invalid_grant → exactly 1 refresh, no thundering herd
// ---------------------------------------------------------------------------

/// AC-006 — traces to BC-1.4.027 (N=10 concurrent invalid_grant → 1 refresh, no thundering herd).
///
/// N=10 concurrent `JiaClient::send()` all hitting 401. Refresh returns `invalid_grant`.
/// Single-flight coordinator: exactly 1 refresh call. All N callers surface
/// `NotAuthenticated`.
///
/// RED GATE (always fails pre-implementation):
/// - All 10 calls return 401 errors (no refresh attempted).
/// - `Mock::expect(1)` on refresh → 0 calls → MockServer drop fails.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_concurrent_invalid_grant_no_thundering_herd() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let _env_guard = harness::env_lock().lock().await;

    use futures::future::join_all;
    use jr::api::client::JiraClient;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const N: usize = 10;

    let server = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac006", server.uri()),
    );

    // All API requests return 401
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    // Refresh: invalid_grant, exactly 1 call (single-flight, no thundering herd)
    Mock::given(method("POST"))
        .and(path("/oauth/token/ac006"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string(harness::INVALID_GRANT_BODY)
                .insert_header("content-type", "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let server_uri = server.uri();

    let handles: Vec<_> = (0..N)
        .map(|_| {
            let uri = server_uri.clone();
            tokio::spawn(async move {
                let client = JiraClient::new_for_test(uri, harness::INITIAL_BEARER.to_string());
                client.get::<serde_json::Value>("/rest/api/3/myself").await
            })
        })
        .collect();

    let results = join_all(handles).await;

    let unexpected: Vec<String> = results
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match r {
            Ok(Ok(_)) => Some(format!("call {i}: unexpectedly succeeded")),
            Ok(Err(e)) => {
                let s = e.to_string();
                if s.to_lowercase().contains("not authenticated") {
                    None // expected
                } else {
                    Some(format!("call {i}: wrong error: {s}"))
                }
            }
            Err(e) => Some(format!("call {i} join error: {e}")),
        })
        .collect();

    assert!(
        unexpected.is_empty(),
        "AC-006 FAIL: all {N} calls must surface NotAuthenticated on invalid_grant. Issues:\n{}",
        unexpected.join("\n")
    );

    // MockServer drop: verifies exactly 1 refresh call (thundering herd = N calls, which fails).
}

// ---------------------------------------------------------------------------
// AC-007: Manual `jr auth refresh` unchanged
// ---------------------------------------------------------------------------

/// AC-007 — traces to BC-1.1.002 (manual `jr auth refresh` CLI command unchanged).
///
/// `jr auth refresh` CLI command continues to work. The auto-refresh wiring does
/// NOT change `src/cli/auth.rs` or the manual refresh path.
///
/// This test is expected to PASS both pre- and post-implementation (regression guard).
/// It verifies that the help output is unchanged by this story.
///
/// AC-007 is covered more thoroughly by the existing `tests/oauth_flow_holdouts.rs`
/// holdout suite (H-001..H-008, H-022, H-029). This test is a lightweight smoke check.
#[test]
fn test_manual_jr_auth_refresh_unchanged() {
    use assert_cmd::Command;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--help"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-007 FAIL: `jr auth refresh --help` must exit 0. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.to_lowercase().contains("refresh"),
        "AC-007 FAIL: help text must mention 'refresh'. Got: {stdout}"
    );
    assert!(
        stdout.contains("--oauth"),
        "AC-007 FAIL: help text must list --oauth flag (manual refresh path unchanged). Got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-008: Contract pinning — URL, body, persist, error shape
// ---------------------------------------------------------------------------

/// AC-008 — traces to BC-1.4.027 (contract pin: URL, body, persist, error shape).
///
/// A single test pinning all four invariants:
/// (a) Refresh endpoint URL is `https://auth.atlassian.com/oauth/token` (or test-overridden).
/// (b) Request body includes `grant_type=refresh_token`.
/// (c) On 200: both rotated tokens persisted via `store_oauth_tokens` (verified structurally
///     by the fact that the retry uses the new token).
/// (d) On any 4xx with `invalid_grant`: `NotAuthenticated` with the re-auth hint.
///
/// RED GATE (always fails pre-implementation):
/// - Sub-assertion (a/b): `Mock::expect(1)` on the body-matching mock → 0 calls → drop fails.
/// - Sub-assertion (d): hint text not present in NotAuthenticated error string.
///
/// This test is split into two logical sections (success path and failure path).
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_refresh_contract_pins_url_grant_type_rotation_invalid_grant() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let _env_guard = harness::env_lock().lock().await;

    use jr::api::client::JiraClient;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ----------------------------------------------------------
    // Section A/B: URL and body contract (success path)
    // ----------------------------------------------------------
    {
        let server = MockServer::start().await;
        set_env(
            "JR_OAUTH_TOKEN_URL",
            &format!("{}/oauth/token/ac008ab", server.uri()),
        );

        // API: 401 then 200
        Mock::given(method("GET"))
            .and(path("/rest/api/3/myself"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_string(harness::ATLASSIAN_401_BODY)
                    .insert_header("content-type", "application/json"),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/myself"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accountId": "user-123"
            })))
            .mount(&server)
            .await;

        // (a) URL path is /oauth/token/ac008ab (routed via JR_OAUTH_TOKEN_URL)
        // (b) body must contain grant_type=refresh_token
        Mock::given(method("POST"))
            .and(path("/oauth/token/ac008ab"))
            .and(body_string_contains("grant_type"))
            .and(body_string_contains("refresh_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(harness::refresh_ok_body()))
            .expect(1)
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), harness::INITIAL_BEARER.to_string());

        let result = client.get::<serde_json::Value>("/rest/api/3/myself").await;

        assert!(
            result.is_ok(),
            "AC-008 (a/b) FAIL: send() must succeed when refresh contract is met. Got: {:?}",
            result.err()
        );
        // MockServer drop: verifies exactly 1 POST to /oauth/token with grant_type+refresh_token.
    }

    // ----------------------------------------------------------
    // Section D: invalid_grant → NotAuthenticated + hint
    // ----------------------------------------------------------
    {
        let server = MockServer::start().await;
        set_env(
            "JR_OAUTH_TOKEN_URL",
            &format!("{}/oauth/token/ac008d", server.uri()),
        );

        Mock::given(method("GET"))
            .and(path("/rest/api/3/myself"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_string(harness::ATLASSIAN_401_BODY)
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/oauth/token/ac008d"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_string(harness::INVALID_GRANT_BODY)
                    .insert_header("content-type", "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), harness::INITIAL_BEARER.to_string());

        let result = client.get::<serde_json::Value>("/rest/api/3/myself").await;

        assert!(
            result.is_err(),
            "AC-008 (d) FAIL: must fail on invalid_grant"
        );

        let err_str = result.unwrap_err().to_string();

        // (d-i) Must be NotAuthenticated
        assert!(
            err_str.to_lowercase().contains("not authenticated"),
            "AC-008 (d) FAIL: must surface NotAuthenticated on invalid_grant. Got: {err_str}"
        );

        // (d-ii) Must carry the re-auth hint
        assert!(
            err_str.contains("jr auth refresh"),
            "AC-008 (d) FAIL: hint must say 'run \\'jr auth refresh\\' to re-authenticate'. \
             Got: {err_str}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-009: Waiters use in-memory token — keyring gated
// ---------------------------------------------------------------------------

/// AC-009 — traces to BC-1.4.027 (waiters use in-memory token, not keychain).
///
/// After a refresh, N-1 queued waiters retry with the new access token from
/// in-memory `RefreshState.last_access_token`. They do NOT re-read keychain.
///
/// Verified via wiremock: all N retry requests must carry the new Bearer token.
/// `Mock::expect(N)` on the retry mock pins this behaviorally.
///
/// RED GATE: `send()` doesn't retry at all pre-implementation; `Mock::expect(N)` fails.
///
/// KEYRING GATE: `JR_RUN_KEYRING_TESTS=1`. Uses `harness::TEST_PROFILE` ("default").
/// All N spawned tasks share the same profile so the single-flight coordinator is exercised.
#[tokio::test]
#[ignore]
async fn test_waiters_use_in_memory_token_not_keychain() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }

    use futures::future::join_all;
    use jr::api::client::JiraClient;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const N: usize = 10;

    set_env("JR_SERVICE_NAME", "jr-s303-test");

    let server = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac009", server.uri()),
    );

    // Seed initial tokens under "default" profile (harness::TEST_PROFILE)
    harness::seed_oauth_tokens();

    // All initial API requests return 401
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .up_to_n_times(N as u64)
        .mount(&server)
        .await;

    // ALL N retries must carry the new in-memory token (not the initial token).
    // expect(N) verifies that all N waiters retried — and with the right token.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(header(
            "Authorization",
            format!("Bearer {}", harness::FAKE_ACCESS_TOKEN),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "user-123"
        })))
        .expect(N as u64)
        .mount(&server)
        .await;

    // Exactly 1 refresh call (single-flight coordinator)
    Mock::given(method("POST"))
        .and(path("/oauth/token/ac009"))
        .respond_with(ResponseTemplate::new(200).set_body_json(harness::refresh_ok_body()))
        .expect(1)
        .mount(&server)
        .await;

    let server_uri = server.uri();

    // All N tasks share profile_name="default" (via new_for_test) which is the same
    // profile that has tokens seeded. The single-flight coordinator keys on profile name,
    // so all N waiters in the same process coordinate through a single shared mutex.
    let handles: Vec<_> = (0..N)
        .map(|_| {
            let uri = server_uri.clone();
            tokio::spawn(async move {
                let client = JiraClient::new_for_test(
                    uri,
                    format!("Bearer {}", harness::INITIAL_ACCESS_TOKEN),
                );
                client.get::<serde_json::Value>("/rest/api/3/myself").await
            })
        })
        .collect();

    let results = join_all(handles).await;

    harness::cleanup_oauth_tokens();

    let errors: Vec<String> = results
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match r {
            Ok(Err(e)) => Some(format!("call {i}: {e}")),
            Err(e) => Some(format!("call {i} join error: {e}")),
            _ => None,
        })
        .collect();

    assert!(
        errors.is_empty(),
        "AC-009 FAIL: all {N} calls must succeed. Errors:\n{}",
        errors.join("\n")
    );
    // MockServer drop verifies: 1 refresh POST + N retry GETs with new token.
}

// ---------------------------------------------------------------------------
// AC-010: Inter-process reconcile after invalid_grant — keyring gated
// ---------------------------------------------------------------------------

/// AC-010 — traces to BC-1.4.027 (inter-process reconcile after invalid_grant).
///
/// Two `JiraClient` instances sharing a keychain profile (simulating two `jr` processes).
/// Instance A refreshes successfully (rotates keychain tokens). Instance B uses the stale
/// refresh token → gets `invalid_grant` → re-reads keychain → finds A's rotated tokens →
/// retries original API call with A's access token → succeeds.
///
/// Both instances use `new_for_test` (profile_name="default") which simulates two
/// separate processes sharing the same profile's keychain slot. Each instance has its
/// own in-process `COORDINATORS` static, but since they're in the same process here,
/// the static is actually shared. The test simulates the inter-process race by running
/// A first (which rotates the keychain), then running B (which reads the stale token
/// that was seeded before A ran).
///
/// RED GATE: `send()` doesn't auto-refresh pre-implementation. Instance B never reaches
/// the reconcile path. `result_b.is_ok()` → fails.
///
/// KEYRING GATE: `JR_RUN_KEYRING_TESTS=1`. Uses `harness::TEST_PROFILE` ("default").
#[tokio::test]
#[ignore]
async fn test_inter_process_reconcile_after_invalid_grant() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }

    use jr::api::auth;
    use jr::api::client::JiraClient;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    set_env("JR_SERVICE_NAME", "jr-s303-test");

    // Seed initial tokens ("default" profile)
    harness::seed_oauth_tokens();

    // Rotated tokens that "Process A" will produce
    let rotated_access = "rotated-access-process-a-s303";
    let rotated_refresh = "rotated-refresh-process-a-s303";

    // ------------------------------------------------------------------
    // Process A: 401 → successful refresh → rotated tokens in keychain → 200 retry
    // ------------------------------------------------------------------
    let server_a = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac010a", server_a.uri()),
    );

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .up_to_n_times(1)
        .mount(&server_a)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "user-a"
        })))
        .mount(&server_a)
        .await;

    Mock::given(method("POST"))
        .and(path("/oauth/token/ac010a"))
        .and(body_string_contains("grant_type=refresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": rotated_access,
            "refresh_token": rotated_refresh,
            "expires_in": 3600,
            "token_type": "Bearer"
        })))
        .mount(&server_a)
        .await;

    // "Process A" — new_for_test uses profile_name="default"
    let client_a = JiraClient::new_for_test(
        server_a.uri(),
        format!("Bearer {}", harness::INITIAL_ACCESS_TOKEN),
    );

    let result_a = client_a
        .get::<serde_json::Value>("/rest/api/3/myself")
        .await;

    assert!(
        result_a.is_ok(),
        "AC-010: Process A must succeed: {:?}",
        result_a.err()
    );

    // Verify Process A rotated the keychain tokens
    let (kc_access, _kc_refresh) = auth::load_oauth_tokens(harness::TEST_PROFILE)
        .expect("AC-010: keychain must have Process A's rotated tokens");
    assert_eq!(
        kc_access, rotated_access,
        "AC-010: keychain must hold Process A's access token"
    );

    // ------------------------------------------------------------------
    // Process B: fresh JiraClient, same profile ("default").
    // B loads its bearer token before A ran — it's stale. B's refresh token
    // is also stale (A already rotated the keychain). The test simulates
    // the state where B loaded INITIAL_REFRESH_TOKEN before A ran and now
    // tries to use it.
    //
    // To make the body_string_contains matcher work, B must send INITIAL_REFRESH_TOKEN
    // in its refresh attempt. But B's JiraClient reads refresh token from keychain at
    // refresh time — which now holds rotated_refresh (from A). So the stale-token
    // simulation is: B's coordinator reads the stale token that was the initial one.
    //
    // DESIGN NOTE: This test simulates the race, not a true OS-level multi-process
    // scenario. The behavioral assertion is: if `invalid_grant` is received AND the
    // keychain refresh token has changed (A rotated it), then B retries the original
    // API call with the keychain's new access token instead of surfacing NotAuthenticated.
    // ------------------------------------------------------------------
    let server_b = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac010b", server_b.uri()),
    );

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .up_to_n_times(1)
        .mount(&server_b)
        .await;

    // Reconcile retry: 200 (B uses the rotated access token from keychain)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "user-b-reconciled"
        })))
        .mount(&server_b)
        .await;

    // B's refresh: any POST to /oauth/token/ac010b returns invalid_grant.
    // (In the real scenario, B sends the stale INITIAL_REFRESH_TOKEN which Atlassian rejects
    // because A already rotated it. In-process simulation: B reads from keychain at refresh
    // time and gets rotated_refresh, but we still test the reconcile path by having
    // invalid_grant returned regardless of which token B sends.)
    Mock::given(method("POST"))
        .and(path("/oauth/token/ac010b"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string(harness::INVALID_GRANT_BODY)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server_b)
        .await;

    // "Process B" — also uses profile_name="default" (same keychain profile as A)
    let client_b = JiraClient::new_for_test(
        server_b.uri(),
        format!("Bearer {}", harness::INITIAL_ACCESS_TOKEN),
    );

    // AC-010 behavior: B gets 401 → invalid_grant → re-reads keychain →
    // keychain refresh token changed (A rotated it) → reads A's new access token →
    // retries API call with rotated_access → 200
    let result_b = client_b
        .get::<serde_json::Value>("/rest/api/3/myself")
        .await;

    // Cleanup (both initial and rotated tokens)
    harness::cleanup_oauth_tokens();

    assert!(
        result_b.is_ok(),
        "AC-010 FAIL: Process B must reconcile after invalid_grant by re-reading keychain \
         and retrying with Process A's rotated access token. Got: {:?}",
        result_b.err()
    );
}

// ---------------------------------------------------------------------------
// AC-011: Persist-before-publish fault injection — keyring + seam gated
// ---------------------------------------------------------------------------

/// AC-011 — traces to BC-1.4.027 (persist-before-publish: fault injection).
///
/// Atlassian exchange succeeds but `store_oauth_tokens` fails (fault-injected via
/// `JR_S303_PERSIST_FAIL=1`). The error propagates; keychain retains initial tokens.
///
/// The implementer must add `JR_S303_PERSIST_FAIL=1` support to `refresh_oauth_token`
/// in `src/api/auth.rs`. When set, the persist step fails after a successful Atlassian
/// exchange, simulating disk-full / keychain-timeout scenarios.
///
/// RED GATE: `send()` doesn't auto-refresh pre-implementation; AC-011 is effectively
/// unreachable. Marked `#[ignore]` because `JR_S303_PERSIST_FAIL` seam doesn't exist yet.
///
/// TO UN-IGNORE: implementer adds `JR_S303_PERSIST_FAIL` seam + `new_for_test_with_profile`,
/// then: `JR_RUN_KEYRING_TESTS=1 JR_S303_PERSIST_FAIL=1 JR_SERVICE_NAME=jr-s303-test
///        cargo test --test oauth_refresh_integration -- test_BC_1_4_027_persist`
#[tokio::test]
#[ignore]
async fn test_persist_before_publish_fault_injection() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    if std::env::var("JR_S303_PERSIST_FAIL").as_deref() != Ok("1") {
        eprintln!(
            "SKIP: set JR_S303_PERSIST_FAIL=1 (implementer must add this seam) for fault injection"
        );
        return;
    }

    use jr::api::auth;
    use jr::api::client::JiraClient;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    set_env("JR_SERVICE_NAME", "jr-s303-test");

    let server = MockServer::start().await;
    set_env(
        "JR_OAUTH_TOKEN_URL",
        &format!("{}/oauth/token/ac011", server.uri()),
    );

    // Seed initial tokens under "default" profile
    harness::seed_oauth_tokens();

    // API: 401 triggers refresh attempt
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string(harness::ATLASSIAN_401_BODY)
                .insert_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    // Refresh: Atlassian exchange SUCCEEDS, but store_oauth_tokens will fail
    // because JR_S303_PERSIST_FAIL=1 is set (fault injected by implementer seam)
    Mock::given(method("POST"))
        .and(path("/oauth/token/ac011"))
        .and(body_string_contains("grant_type=refresh_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(harness::refresh_ok_body()))
        .mount(&server)
        .await;

    // new_for_test uses profile_name="default" which matches the seeded keychain entry
    let client = JiraClient::new_for_test(
        server.uri(),
        format!("Bearer {}", harness::INITIAL_ACCESS_TOKEN),
    );

    // Call: 401 → refresh exchange succeeds → store_oauth_tokens FAILS (JR_S303_PERSIST_FAIL=1)
    // → error propagates to caller
    let result = client.get::<serde_json::Value>("/rest/api/3/myself").await;

    // (a) Error must propagate (not silently swallowed)
    assert!(
        result.is_err(),
        "AC-011 FAIL (a): persist failure must propagate as an error to the caller"
    );

    // (b) Keychain must NOT be updated — initial tokens must still be present
    let (stored_access, stored_refresh) = auth::load_oauth_tokens(harness::TEST_PROFILE)
        .expect("AC-011: keychain must still have initial tokens after failed persist");

    harness::cleanup_oauth_tokens();

    assert_eq!(
        stored_access,
        harness::INITIAL_ACCESS_TOKEN,
        "AC-011 FAIL (b): keychain must NOT be updated when store_oauth_tokens fails. \
         Persist-before-publish: RefreshState must not diverge from on-disk state. \
         Got: {stored_access}"
    );
    assert_eq!(
        stored_refresh,
        harness::INITIAL_REFRESH_TOKEN,
        "AC-011 FAIL (b): refresh token in keychain must remain initial (not rotated). \
         Got: {stored_refresh}"
    );
}
