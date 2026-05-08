//! Rate-limit regression holdout suite — S-1.07
//!
//! Pins existing rate-limit retry semantics and Retry-After header parsing
//! behavior. All 5 tests MUST PASS on current develop — no implementation
//! changes are required or expected. Future regressions in any of these paths
//! will break this suite.
//!
//! Holdout coverage:
//! - AC-001 / H-013: `send_raw` on persistent 429 returns Ok(429) after
//!   MAX_RETRIES=3 (4 total HTTP calls); stderr "gave up" warning emitted.
//! - AC-002 / H-027: `RateLimitInfo::from_headers` with `Retry-After: 86400`
//!   returns `retry_after_secs == Some(86400)` — NO cap. (KNOWN-GAP until
//!   NFR-R-NEW-1 is implemented in S-3.07.)
//! - AC-003: `send` (via `get`) recovers after 2 retries (429, 429, 200) →
//!   Ok(200) + exactly 3 HTTP calls.
//! - AC-004: `RateLimitInfo::from_headers` with `Retry-After: 0` returns
//!   `retry_after_secs == Some(0)` (boundary value preserved).
//! - AC-005: `RateLimitInfo::from_headers` with `Retry-After: abc` returns
//!   `retry_after_secs == None` (parse failure; DEFAULT_RETRY_SECS is applied
//!   at call site, not inside from_headers).
//!
//! Import path verdict: `jr::api::rate_limit::RateLimitInfo` is accessible
//! from integration tests. `rate_limit` is `pub mod` in `api/mod.rs` and
//! `RateLimitInfo` / `from_headers` are both `pub`. No inline fallback needed.
//!
//! Infrastructure pattern: wiremock for HTTP-mock tests (AC-001, AC-003).
//! Direct struct construction for header-parsing unit tests (AC-002, AC-004,
//! AC-005). Process-spawn (assert_cmd) for AC-001 stderr verification.
//! `JR_AUTH_HEADER` + `JR_BASE_URL` for process-level tests (SD-002 canonical
//! test infra). `--no-input` on all process-spawn calls. Tests MUST NOT be
//! `#[ignore]`-gated — both must run in standard `cargo test`.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use jr::api::client::JiraClient;
use jr::api::rate_limit::RateLimitInfo;
use reqwest::Method;
use reqwest::header::{HeaderMap, HeaderValue};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a `jr` Command with a clean JR_* environment and test-specific
/// JR_BASE_URL + JR_AUTH_HEADER bypasses. Matches the hygiene pattern used
/// in `tests/verbose_bodies.rs` and `tests/oauth_flow_holdouts.rs`.
fn jr_cmd_with_mock(server_uri: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CONFIG_HOME", config_dir)
        .env("XDG_CACHE_HOME", config_dir)
        .env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .env_remove("JR_OAUTH_CLIENT_ID")
        .env_remove("JR_OAUTH_CLIENT_SECRET");
    cmd
}

// ---------------------------------------------------------------------------
// AC-001 / H-013: send_raw on persistent 429 — library assertion (count + status)
// ---------------------------------------------------------------------------

/// BC-X.1.005 postcondition: `send_raw` on a server that returns 429 for ALL
/// requests returns `Ok(Response)` with status 429 after exhausting MAX_RETRIES=3.
/// Wiremock `.expect(4)` enforces exactly 4 HTTP calls (1 initial + 3 retries).
///
/// Pins H-013. The companion process-level test
/// `test_s_1_07_h_013_send_raw_gave_up_warning_in_stderr` asserts the stderr
/// "gave up" warning, which cannot be captured at the library level.
#[tokio::test]
async fn test_s_1_07_h_013_send_raw_persistent_429_returns_429_after_max_retries() {
    let server = MockServer::start().await;

    // All requests return 429 with Retry-After: 0 so the test runs instantly.
    // `.expect(4)` is enforced on MockServer drop: fails the test if the call
    // count is not exactly 4 (1 initial + MAX_RETRIES=3 retries).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .expect(4) // 1 initial + 3 retries = MAX_RETRIES=3
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // Build a GET /rest/api/3/myself request via the public `request()` builder.
    let req = client
        .request(Method::GET, "/rest/api/3/myself")
        .build()
        .unwrap();

    let response = client.send_raw(req).await;

    // send_raw must return Ok (not Err) even after exhausting retries.
    assert!(
        response.is_ok(),
        "AC-001 / H-013: send_raw must return Ok(Response) not Err after persistent 429; \
         got: {:?}",
        response.err().map(|e| e.to_string())
    );

    let status = response.unwrap().status().as_u16();
    assert_eq!(
        status, 429,
        "AC-001 / H-013: send_raw must return the 429 response to the caller (not convert to error)"
    );

    // MockServer drop verifies exactly 4 calls (enforced by .expect(4)).
}

// ---------------------------------------------------------------------------
// AC-001 / H-013: send_raw persistent 429 — process-level stderr verification
// ---------------------------------------------------------------------------

/// BC-X.1.005 postcondition (stderr arm): the `jr api` passthrough command
/// emits the literal warning text to stderr after exhausting retries.
///
/// Process-level test required because `eprintln!` in library code cannot be
/// captured by `#[tokio::test]` without an additional crate. Uses `assert_cmd`
/// with `JR_BASE_URL` + `JR_AUTH_HEADER` (SD-002 canonical test infra).
///
/// Pins H-013. The library-level test
/// `test_s_1_07_h_013_send_raw_persistent_429_returns_429_after_max_retries`
/// asserts the status and call count.
#[tokio::test]
async fn test_s_1_07_h_013_send_raw_gave_up_warning_in_stderr() {
    let server = MockServer::start().await;

    // All requests return 429 — the CLI call will exhaust retries and print
    // the "gave up" warning to stderr. No `.expect()` here because the mock
    // server is shared with the CLI process which drives its own call count.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .mount(&server)
        .await;

    let config_dir = TempDir::new().unwrap();

    // `jr api /rest/api/3/myself` (GET is the default method) uses send_raw internally.
    let out = jr_cmd_with_mock(&server.uri(), config_dir.path())
        .args(["api", "/rest/api/3/myself"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        stderr.contains("rate limited by Jira") && stderr.contains("gave up after 3 retries"),
        "AC-001 / H-013: stderr must contain 'rate limited by Jira — gave up after 3 retries'; \
         got: {stderr}"
    );
    assert!(
        !stderr.contains("panic"),
        "AC-001 / H-013: stderr must not leak a panic; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 / H-027: Retry-After 86400 — no cap (KNOWN-GAP pin)
// ---------------------------------------------------------------------------

/// BC-X.4.002 postcondition: `RateLimitInfo::from_headers` with
/// `Retry-After: 86400` returns `retry_after_secs == Some(86400)`.
/// The value is NOT clamped to any maximum.
///
/// KNOWN-GAP: H-027. When NFR-R-NEW-1 (MAX_RETRY_AFTER_SECS=60 cap) is
/// implemented in S-3.07, change this assertion to:
///     assert_eq!(info.retry_after_secs, Some(60));
/// and update the holdout status from MUST-PASS (KNOWN-GAP) to
/// MUST-PASS-AFTER-FIX.
#[test]
fn test_s_1_07_h_027_retry_after_86400_no_cap() {
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", HeaderValue::from_static("86400"));

    let info = RateLimitInfo::from_headers(&headers);

    // KNOWN-GAP: H-027. When NFR-R-NEW-1 (MAX_RETRY_AFTER_SECS=60) is implemented,
    // change this assertion to: assert_eq!(info.retry_after_secs, Some(60));
    // and update the holdout status from MUST-PASS to MUST-FAIL-BEFORE-FIX /
    // MUST-PASS-AFTER-FIX.
    assert_eq!(
        info.retry_after_secs,
        Some(86400),
        "AC-002 / H-027: RateLimitInfo must preserve Retry-After value without upper-bound cap \
         (KNOWN-GAP until NFR-R-NEW-1 lands in S-3.07)"
    );
}

// ---------------------------------------------------------------------------
// AC-003: send recovers after 2 retries (429, 429, 200) — regression guard
// ---------------------------------------------------------------------------

/// BC-X.1.005 postcondition: the standard `send` path (via `get`) retries on
/// 429 and succeeds when the server eventually returns 200.
///
/// Sequence: 429 → 429 → 200. Expects exactly 3 HTTP calls.
/// Wiremock enforces: `.up_to_n_times(2)` for the 429 mock (2 firings) and
/// `.expect(1)` for the 200 mock (1 firing). MockServer drop verifies counts.
///
/// This is the recovery-happy-path regression guard. It verifies that the
/// retry loop DOES succeed when the server eventually responds 200, as
/// opposed to AC-001 which pins the persistent-failure path.
#[tokio::test]
async fn test_s_1_07_h_013_send_recovers_after_2_retries() {
    let server = MockServer::start().await;

    // First two requests return 429 with Retry-After: 0 (no delay).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(2) // fires for the first 2 matching requests
        .expect(2) // must fire exactly 2 times
        .mount(&server)
        .await;

    // Third request returns 200.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "test-account-id",
            "displayName": "Test User"
        })))
        .expect(1) // must fire exactly once
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // `get` calls the private `send` method internally.
    let response: anyhow::Result<serde_json::Value> = client.get("/rest/api/3/myself").await;

    assert!(
        response.is_ok(),
        "AC-003: send must return Ok after recovering from 2 retries; \
         got: {:?}",
        response.err().map(|e| e.to_string())
    );

    // MockServer drop verifies: 2 calls to the 429 mock + 1 call to the 200 mock
    // = 3 total HTTP calls.
}

// ---------------------------------------------------------------------------
// AC-004: Retry-After 0 — boundary value preserved
// ---------------------------------------------------------------------------

/// BC-X.4.002 invariant: `RateLimitInfo::from_headers` with `Retry-After: 0`
/// returns `retry_after_secs == Some(0)`. Zero is a valid value (no delay before
/// retry) and must not be coerced to None or DEFAULT_RETRY_SECS.
#[test]
fn test_s_1_07_retry_after_0_boundary() {
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", HeaderValue::from_static("0"));

    let info = RateLimitInfo::from_headers(&headers);

    assert_eq!(
        info.retry_after_secs,
        Some(0),
        "AC-004: Retry-After: 0 must return Some(0); boundary value must not be coerced to \
         None or DEFAULT_RETRY_SECS"
    );
}

// ---------------------------------------------------------------------------
// AC-005: Retry-After "abc" — unparseable returns None
// ---------------------------------------------------------------------------

/// BC-X.4.002 invariant: `RateLimitInfo::from_headers` with a non-integer
/// `Retry-After: abc` header returns `retry_after_secs == None`. The fallback
/// to DEFAULT_RETRY_SECS (1s) is applied at the call site via
/// `.unwrap_or(DEFAULT_RETRY_SECS)`, NOT inside `from_headers` itself.
///
/// Per NFR-SCA-1: HTTP-date format (`Retry-After: Thu, 01 Jan 2026 00:00:00 GMT`)
/// is treated the same way — the parser falls through to None on any non-integer
/// value. DEFAULT_RETRY_SECS is applied by the caller.
#[test]
fn test_s_1_07_retry_after_unparseable_returns_none() {
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", HeaderValue::from_static("abc"));

    let info = RateLimitInfo::from_headers(&headers);

    assert_eq!(
        info.retry_after_secs, None,
        "AC-005: Retry-After: abc (non-integer) must return None; DEFAULT_RETRY_SECS is applied \
         at call site, not inside from_headers"
    );
}
