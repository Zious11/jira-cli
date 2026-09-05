//! S-cycle4-cloud-id-correctness — Two-Step Red Gate TESTS (step 2 of 2) for
//! `src/api/jira/tenant.rs::fetch_cloud_id`'s contract (BC-1.2.052
//! Postcondition 2, Invariants 1/3; VP-AUTHDX-019; ADR-0022 §1) and, in the
//! keyring-gated tier, `login_token`/`refresh_credentials`'s use of it
//! (AC-001, AC-002, AC-003, AC-004, AC-006).
//!
//! Stub landed at commit `6a8b17c0`: `fetch_cloud_id` was `todo!()`. Every
//! test in this file was expected to FAIL at that point by panicking on
//! that `todo!()` — not by a compile error, and not vacuously passing.
//!
//! **Status (FIX-F5-CYCLE4-2 doc sweep):** `fetch_cloud_id` shipped in
//! S-cycle4-cloud-id-correctness (ADR-0022, BC-1.2.052/053/054) and has
//! since been through the cycle-004 F5 scoped adversarial review plus its
//! FIX-F5-CYCLE4-1/-2 fix rounds. Every test in this file is green today
//! (the keyring-gated tier remains `#[ignore]`d pending
//! `JR_RUN_KEYRING_TESTS=1`, by design — not because it fails). The
//! Two-Step Red Gate narrative below is retained as historical record of
//! how these tests were authored, not a description of current behavior.
//!
//! ## Why `wiremock` cannot exercise a genuine HTTPS success/status-shape
//! response here, and the `JR_TENANT_INFO_URL` seam this depends on
//!
//! `fetch_cloud_id` REQUIRES `site_url` to start with `https://` (ADR-0022
//! §1, Pass-4 adversarial review Finding #4) — a real security invariant
//! (closes an on-path plaintext wrong-tenant-`cloudId` vector), not a
//! testing inconvenience. `wiremock` 0.6.5 has no HTTPS/TLS support at all
//! (verified against its public `MockServerBuilder` API — only `.listener()`
//! and `.disable_request_recording()` exist), so a `site_url` that
//! genuinely satisfies the `https://` precondition can never be routed to a
//! real `wiremock` server: the TLS handshake against wiremock's plaintext
//! HTTP listener fails before any HTTP response is ever exchanged. This
//! story's own Library & Framework Requirements table forbids adding a new
//! dependency (e.g. a TLS-capable mock server crate) to work around this.
//!
//! Every test below that needs `fetch_cloud_id` to actually reach a mock
//! HTTP response (success parsing, non-2xx handling, missing-field
//! handling, no-redirect-follow, no-Authorization-header, bare-path/
//! no-query-string) therefore assumes a debug-only `JR_TENANT_INFO_URL` env
//! var override: when set, the function's ACTUAL GET request goes to
//! `format!("{}/_edge/tenant_info", env::var("JR_TENANT_INFO_URL").unwrap())`
//! instead of `format!("{}/_edge/tenant_info", site_url)`, while `site_url`
//! itself remains what the `https://`-prefix precondition validates. This
//! mirrors the already-established `JR_BASE_URL`
//! (`src/config.rs::Config::base_url`) and `JR_ACCESSIBLE_RESOURCES_URL`
//! (`src/api/auth.rs::oauth_login`) seam family documented in CLAUDE.md's
//! "AI Agent Notes" — and the exact same "write tests against a seam the
//! implementer has not added yet" precedent
//! `tests/multi_cloudid_disambiguation.rs` used for
//! `JR_ACCESSIBLE_RESOURCES_URL`/`JR_OAUTH_TOKEN_URL` before those existed.
//!
//! Tests that only need a *failure* (soft-fail / preserve) do NOT need this
//! seam: an `http://`/scheme-less `site_url` deterministically triggers
//! `fetch_cloud_id`'s own https-only precondition skip, itself a
//! legitimate, spec-documented failure shape (EC-1.2.052-2).
//!
//! This gap (and the exact seam shape assumed) is flagged prominently in
//! this story's Test Writer report as the single highest-priority open
//! question for the implementer, not left for a future reader to
//! rediscover silently.
//!
//! ## Trace: AC/EC/VP -> test mapping
//!
//! | AC/EC/VP           | Test function(s)                                                    |
//! |---------------------|----------------------------------------------------------------------|
//! | AC-002 (https-only)| `test_fetch_cloud_id_makes_zero_requests_for_http_scheme`             |
//! | AC-002 (https-only)| `test_fetch_cloud_id_makes_zero_requests_for_scheme_less_url`         |
//! | EC-1.2.052-4        | `test_fetch_cloud_id_soft_fails_on_unresolvable_host`                 |
//! | EC-1.2.052-4        | `test_fetch_cloud_id_soft_fails_on_connection_refused`                |
//! | AC-002 (parse)      | `test_fetch_cloud_id_success_parses_cloud_id_and_ignores_unknown_fields` (seam) |
//! | AC-002 (no auth hdr)| `test_fetch_cloud_id_sends_no_authorization_header` (seam)            |
//! | AC-002 (bare path)  | `test_fetch_cloud_id_request_path_has_no_query_string` (seam)         |
//! | EC-1.2.052-2        | `test_fetch_cloud_id_soft_fails_on_404` (seam)                        |
//! | EC-1.2.052-3        | `test_fetch_cloud_id_soft_fails_on_missing_cloud_id_field` (seam)     |
//! | EC-1.2.052-2 redirect | `test_fetch_cloud_id_does_not_follow_redirect` (seam)               |
//! | AC-001, keyring     | `test_login_token_persists_explicit_cloud_id_override_e2e`           |
//! | AC-003, keyring     | `test_login_token_soft_fails_and_still_succeeds_on_fetch_failure`     |
//! | AC-006, keyring     | `test_refresh_credentials_on_api_token_profile_still_succeeds_with_non_https_url` |

use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use jr::api::jira::tenant::fetch_cloud_id;
use tokio::sync::Mutex;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Shared helpers (kept local, not tests/common — mirrors
// tests/auth_chosen_flow_reconcile.rs's / tests/multi_cloudid_disambiguation.rs's
// stated rationale: each caller wraps its own `unsafe` justification).
// ---------------------------------------------------------------------------

fn unique_service(tag: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("jr-jira-cli-test-{tag}-{}-{}", std::process::id(), n)
}

fn write_config(dir: &Path, toml: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, toml).unwrap();
    path
}

fn keyring_gate_active() -> bool {
    std::env::var("JR_RUN_KEYRING_TESTS").as_deref() == Ok("1")
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// ---------------------------------------------------------------------------
// `fetch_cloud_id` direct contract tests — no keychain, no config file.
// ---------------------------------------------------------------------------

/// AC-002 (BC-1.2.052 Postcondition 2, https-only precondition; Pass-4
/// adversarial review Finding #4; VP-AUTHDX-019): an `http://` `site_url`
/// skips the fetch ENTIRELY — zero network requests reach the server, and
/// `fetch_cloud_id` returns `Err`.
#[tokio::test]
async fn test_fetch_cloud_id_makes_zero_requests_for_http_scheme() {
    let server = MockServer::start().await;
    // No mock mounted at all — any request that reaches the server 404s by
    // wiremock's own default, but the point of this test is that NO
    // request reaches it in the first place.
    let site_url = server.uri(); // "http://127.0.0.1:PORT"

    let result = fetch_cloud_id(&site_url).await;

    assert!(
        result.is_err(),
        "an http:// site_url must be treated as a fetch failure (soft-fail \
         path), not Ok"
    );
    let received = server.received_requests().await.unwrap_or_default();
    assert!(
        received.is_empty(),
        "AC-002: fetch_cloud_id must make ZERO network requests for a \
         non-https site_url. Received: {received:?}"
    );
}

/// AC-002, case-insensitivity / scheme-less variant: a bare hostname (no
/// scheme at all) is likewise skipped with zero requests.
#[tokio::test]
async fn test_fetch_cloud_id_makes_zero_requests_for_scheme_less_url() {
    let server = MockServer::start().await;
    // Deliberately malformed: no scheme prefix at all.
    let host_only = server
        .uri()
        .strip_prefix("http://")
        .expect("wiremock uri always starts with http://")
        .to_string();

    let result = fetch_cloud_id(&host_only).await;

    assert!(result.is_err());
    let received = server.received_requests().await.unwrap_or_default();
    assert!(
        received.is_empty(),
        "a scheme-less site_url must also make ZERO requests. Received: {received:?}"
    );
}

/// EC-1.2.052-4: a network-level error (here, DNS resolution failure
/// against a reserved, non-resolvable TLD) is treated as an ordinary
/// soft-fail — `Err`, never a panic. Bounded by the documented 10s timeout
/// (not separately asserted here to keep this test fast; DNS failure for
/// `.invalid` resolves near-instantly on virtually all resolvers).
///
/// ADV MED-1 (test-isolation race → CI flakiness): this test's `https://`
/// `site_url` reaches `fetch_cloud_id`'s debug-only `JR_TENANT_INFO_URL`
/// read (see `tenant.rs::fetch_cloud_id`) even though it never sets that
/// var itself. A concurrently-running seam test in this same file (e.g.
/// `test_fetch_cloud_id_success_parses_cloud_id_and_ignores_unknown_fields`)
/// sets `JR_TENANT_INFO_URL` to a live wiremock URI for the duration of its
/// own `env_lock` critical section — without also holding `env_lock` here,
/// this test's request could be retargeted mid-flight to that mock server,
/// which responds `Ok`, flipping the `assert!(result.is_err())` below.
/// Acquiring `env_lock` for this test's scope, and removing any stray
/// `JR_TENANT_INFO_URL` before calling `fetch_cloud_id`, closes that race —
/// this test then deterministically exercises the real DNS-failure path,
/// exactly as intended.
#[tokio::test]
async fn test_fetch_cloud_id_soft_fails_on_unresolvable_host() {
    let _guard = env_lock().lock().await;
    // SAFETY: env_lock held for this whole scope.
    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    let result = fetch_cloud_id("https://this-host-does-not-exist.invalid").await;
    assert!(
        result.is_err(),
        "an unresolvable host must be a soft-fail (Err), never Ok or a panic"
    );
}

/// EC-1.2.052-4: a connection-refused error (a closed local TCP port under
/// an `https://` scheme, so the request also exercises a genuine TLS
/// handshake attempt against nothing listening) is likewise a soft-fail.
///
/// ADV MED-1: see the sibling `test_fetch_cloud_id_soft_fails_on_unresolvable_host`
/// doc comment above for why this test must hold `env_lock` and clear
/// `JR_TENANT_INFO_URL` before calling `fetch_cloud_id` — the same seam-read
/// race applies here.
#[tokio::test]
async fn test_fetch_cloud_id_soft_fails_on_connection_refused() {
    let _guard = env_lock().lock().await;
    // SAFETY: env_lock held for this whole scope.
    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    // Port 0 binds to an ephemeral port and closes it, freeing a port
    // number that is (barring an extraordinarily unlucky race) not
    // listening for the immediately-following request.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let result = fetch_cloud_id(&format!("https://127.0.0.1:{port}")).await;
    assert!(
        result.is_err(),
        "a connection-refused target must be a soft-fail (Err), never Ok or a panic"
    );
}

/// AC-002 (bare-path/no-auth-header/cloudId-only-parse contract) — SUCCESS
/// shape. Requires the `JR_TENANT_INFO_URL` seam documented in this file's
/// header; see that comment for exactly what is expected and why plain
/// `wiremock` cannot exercise this any other way given the `https://`
/// precondition.
#[tokio::test]
async fn test_fetch_cloud_id_success_parses_cloud_id_and_ignores_unknown_fields() {
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_edge/tenant_info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "cloudId": "the-real-cloud-id",
            "baseUrl": "https://example.atlassian.net",
            "activation": "activated"
        })))
        .mount(&server)
        .await;
    // SAFETY: env_lock held for this whole scope.
    unsafe {
        std::env::set_var("JR_TENANT_INFO_URL", server.uri());
    }

    let result = fetch_cloud_id("https://plausible-real-site.example").await;

    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    assert_eq!(
        result.expect("a 2xx response with a valid cloudId field must be Ok"),
        "the-real-cloud-id",
        "only the cloudId field is parsed; unknown fields (baseUrl, activation) \
         must be ignored, not rejected"
    );
}

/// AC-002 (no `Authorization` header attached, ever — the endpoint is
/// unauthenticated by design). Seam-dependent, see file header.
#[tokio::test]
async fn test_fetch_cloud_id_sends_no_authorization_header() {
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_edge/tenant_info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "cloudId": "irrelevant-for-this-test"
        })))
        .mount(&server)
        .await;
    unsafe {
        std::env::set_var("JR_TENANT_INFO_URL", server.uri());
    }

    let _ = fetch_cloud_id("https://plausible-real-site.example").await;

    let received = server.received_requests().await.unwrap_or_default();

    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    assert_eq!(
        received.len(),
        1,
        "expected exactly one request to reach the mock"
    );
    assert!(
        received[0].headers.get("Authorization").is_none(),
        "AC-002: fetch_cloud_id must never attach an Authorization header. \
         Headers seen: {:?}",
        received[0].headers
    );
}

/// AC-002 (bare path, no query string — a trailing `?_r=...` cache-buster
/// has been observed to 403 per ADR-0022 §1). Seam-dependent, see file
/// header.
#[tokio::test]
async fn test_fetch_cloud_id_request_path_has_no_query_string() {
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_edge/tenant_info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "cloudId": "irrelevant-for-this-test"
        })))
        .mount(&server)
        .await;
    unsafe {
        std::env::set_var("JR_TENANT_INFO_URL", server.uri());
    }

    let _ = fetch_cloud_id("https://plausible-real-site.example").await;

    let received = server.received_requests().await.unwrap_or_default();

    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    assert_eq!(received.len(), 1);
    assert_eq!(received[0].url.path(), "/_edge/tenant_info");
    assert!(
        received[0].url.query().is_none(),
        "AC-002: no query string may ever be appended. Got query: {:?}",
        received[0].url.query()
    );
}

/// EC-1.2.052-2: a non-2xx status (404) is an ordinary soft-fail — `Err`,
/// not a panic, and not specially distinguished from any other non-2xx
/// status. Seam-dependent, see file header.
#[tokio::test]
async fn test_fetch_cloud_id_soft_fails_on_404() {
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_edge/tenant_info"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    unsafe {
        std::env::set_var("JR_TENANT_INFO_URL", server.uri());
    }

    let result = fetch_cloud_id("https://plausible-real-site.example").await;

    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    assert!(result.is_err(), "a 404 response must be a soft-fail (Err)");
}

/// EC-1.2.052-3: a 200 response whose body is valid JSON but omits
/// `cloudId` is a soft-fail (serde missing-required-field deserialization
/// failure), never a panic. Seam-dependent, see file header.
#[tokio::test]
async fn test_fetch_cloud_id_soft_fails_on_missing_cloud_id_field() {
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_edge/tenant_info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "baseUrl": "https://example.atlassian.net"
        })))
        .mount(&server)
        .await;
    unsafe {
        std::env::set_var("JR_TENANT_INFO_URL", server.uri());
    }

    let result = fetch_cloud_id("https://plausible-real-site.example").await;

    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    assert!(
        result.is_err(),
        "a 200 response missing the cloudId field must be a soft-fail (Err), \
         never a panic"
    );
}

/// FIX-F5-CYCLE4-2 hardening #1a: a 200 response whose `cloudId` field is
/// present but EMPTY must be treated as a fetch failure (soft-fail path,
/// BC-1.2.053 Postcondition 2/3) — an empty cloudId is not a plausible
/// value and must never be persisted. Seam-dependent, see file header.
#[tokio::test]
async fn test_fetch_cloud_id_soft_fails_on_empty_cloud_id_field() {
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_edge/tenant_info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "cloudId": ""
        })))
        .mount(&server)
        .await;
    unsafe {
        std::env::set_var("JR_TENANT_INFO_URL", server.uri());
    }

    let result = fetch_cloud_id("https://plausible-real-site.example").await;

    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    assert!(
        result.is_err(),
        "FIX-F5-CYCLE4-2: an empty cloudId field must be a soft-fail (Err), \
         never persisted as a valid value"
    );
}

/// FIX-F5-CYCLE4-2 hardening #1a: a `cloudId` value containing whitespace
/// is not a plausible cloudId shape and must be rejected the same way as an
/// empty one. Seam-dependent, see file header.
#[tokio::test]
async fn test_fetch_cloud_id_soft_fails_on_whitespace_cloud_id_field() {
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_edge/tenant_info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "cloudId": "not a real cloud id"
        })))
        .mount(&server)
        .await;
    unsafe {
        std::env::set_var("JR_TENANT_INFO_URL", server.uri());
    }

    let result = fetch_cloud_id("https://plausible-real-site.example").await;

    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    assert!(
        result.is_err(),
        "FIX-F5-CYCLE4-2: a cloudId containing whitespace must be a \
         soft-fail (Err), never persisted as a valid value"
    );
}

/// FIX-F5-CYCLE4-2 hardening #1c: a response body larger than the bounded
/// read cap must be a soft-fail, never an unbounded-memory read. Uses a
/// legitimately-shaped JSON body padded with a huge unrelated field so the
/// only thing under test is the size cap, not JSON validity. Seam-dependent,
/// see file header.
#[tokio::test]
async fn test_fetch_cloud_id_soft_fails_on_oversized_response_body() {
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;
    // Comfortably larger than any reasonable bound (well over 64 KiB).
    let huge_padding = "x".repeat(2 * 1024 * 1024);
    Mock::given(method("GET"))
        .and(path("/_edge/tenant_info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "cloudId": "the-real-cloud-id",
            "padding": huge_padding
        })))
        .mount(&server)
        .await;
    unsafe {
        std::env::set_var("JR_TENANT_INFO_URL", server.uri());
    }

    let result = fetch_cloud_id("https://plausible-real-site.example").await;

    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    assert!(
        result.is_err(),
        "FIX-F5-CYCLE4-2: an oversized response body must be a soft-fail \
         (Err), not an unbounded read"
    );
}

/// EC-1.2.052-2 (Pass-1 adversarial review Finding #12): a 3xx redirect
/// response is NEVER followed cross-host — `redirect::Policy::none()`
/// means the 3xx status itself is surfaced as an ordinary non-2xx status
/// (soft-fail), and the redirect Location target receives NO second
/// request. Seam-dependent, see file header.
#[tokio::test]
async fn test_fetch_cloud_id_does_not_follow_redirect() {
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/_edge/tenant_info"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", "https://attacker.example/_edge/tenant_info"),
        )
        .mount(&server)
        .await;
    unsafe {
        std::env::set_var("JR_TENANT_INFO_URL", server.uri());
    }

    let result = fetch_cloud_id("https://plausible-real-site.example").await;

    let received = server.received_requests().await.unwrap_or_default();

    unsafe {
        std::env::remove_var("JR_TENANT_INFO_URL");
    }

    assert!(
        result.is_err(),
        "a 3xx redirect must be treated as an ordinary soft-fail, never Ok"
    );
    assert_eq!(
        received.len(),
        1,
        "the redirect must never be followed — exactly one request (to the \
         original mock), never a second request anywhere. Received: {received:?}"
    );
}

// ---------------------------------------------------------------------------
// Keyring-gated tier: `login_token`/`refresh_credentials` end-to-end
// (#[ignore] + JR_RUN_KEYRING_TESTS=1), mirroring
// tests/api_token_percred_wiring.rs's and
// tests/auth_chosen_flow_reconcile.rs's gated-tier conventions.
// ---------------------------------------------------------------------------

/// AC-001, end-to-end: `jr auth login`'s API-token branch (`login_token`
/// directly) with an explicit `--cloud-id`-equivalent override persists
/// that value to `config.toml`'s `[profiles.<name>].cloud_id` — even
/// though the profile's `url` could never resolve a real fetch (proving
/// the fetch really was skipped, not merely lucky).
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_login_token_persists_explicit_cloud_id_override_e2e() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;

    let svc = unique_service("cloud-id-override");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    write_config(
        &cfg_dir,
        "default_profile = \"sandbox\"\n\n\
         [profiles.sandbox]\n\
         url = \"not-a-real-url\"\n\
         auth_method = \"api_token\"\n",
    );

    // SAFETY: env_lock held for this whole scope.
    unsafe {
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
    }

    let login_result = jr::cli::auth::login_token(
        "sandbox",
        Some("override-e2e@example.com".to_string()),
        Some("override-e2e-token".to_string()),
        Some("override-e2e-uuid"),
        true,
        jr::cli::OutputFormat::Table,
    )
    .await;

    let saved = std::fs::read_to_string(cfg_dir.join("config.toml")).unwrap();

    let _ = keyring::Entry::new(&svc, "sandbox:email").and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&svc, "sandbox:api-token").and_then(|e| e.delete_credential());
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    login_result.expect("login_token must succeed even with an unresolvable url, since the override skips the fetch entirely");
    assert!(
        saved.contains("override-e2e-uuid"),
        "AC-001: the explicit override must be persisted to config.toml. Saved:\n{saved}"
    );
}

/// AC-003, end-to-end: a fetch failure (non-https profile url) must not
/// abort `login_token` — the login still succeeds and the keychain pair is
/// still written, even though no cloud_id was acquired.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_login_token_soft_fails_and_still_succeeds_on_fetch_failure() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;

    let svc = unique_service("cloud-id-soft-fail");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    write_config(
        &cfg_dir,
        "default_profile = \"sandbox\"\n\n\
         [profiles.sandbox]\n\
         url = \"http://not-https.example.net\"\n\
         auth_method = \"api_token\"\n",
    );

    unsafe {
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
    }

    let login_result = jr::cli::auth::login_token(
        "sandbox",
        Some("soft-fail@example.com".to_string()),
        Some("soft-fail-token".to_string()),
        None,
        true,
        jr::cli::OutputFormat::Table,
    )
    .await;

    let namespaced = jr::api::auth::load_api_token(&jr::profile::Profile::from("sandbox"));
    let saved = std::fs::read_to_string(cfg_dir.join("config.toml")).unwrap();

    let _ = keyring::Entry::new(&svc, "sandbox:email").and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&svc, "sandbox:api-token").and_then(|e| e.delete_credential());
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    login_result.expect(
        "AC-003: a fetch failure must never abort login_token — login must \
         still succeed",
    );
    assert!(
        namespaced.is_ok(),
        "the keychain pair must still be written despite the cloud_id fetch failure"
    );
    assert!(
        !saved.contains("cloud_id"),
        "no cloud_id should have been written on a fetch failure for a \
         brand-new profile. Saved:\n{saved}"
    );
}

/// AC-006 (BC-1.2.052 Invariant 3): `jr auth refresh` on an api_token
/// profile is a genuine third trigger site for the same fallback chain —
/// proven here by showing a non-https profile url produces the SAME
/// soft-fail-without-aborting behavior as `login_token` does directly
/// (i.e., `refresh_credentials` really does route through `login_token`,
/// not a refresh-specific, fetch-free code path of its own).
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_refresh_credentials_on_api_token_profile_still_succeeds_with_non_https_url() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;

    let svc = unique_service("cloud-id-refresh");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    write_config(
        &cfg_dir,
        "default_profile = \"sandbox\"\n\n\
         [profiles.sandbox]\n\
         url = \"http://not-https.example.net\"\n\
         auth_method = \"api_token\"\n",
    );

    unsafe {
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
    }

    // Seed a pre-existing credential pair so refresh_credentials has
    // something to (re)obtain against.
    jr::api::auth::store_api_token(
        &jr::profile::Profile::from("sandbox"),
        "pre-refresh@example.com",
        "pre-refresh-token",
    )
    .unwrap();

    let output = jr::cli::OutputFormat::Table;
    let result = jr::cli::auth::refresh_credentials(jr::cli::auth::RefreshArgs {
        profile: Some("sandbox"),
        oauth: false,
        api_token: false,
        email: Some("refreshed@example.com".to_string()),
        token: Some("refreshed-token".to_string()),
        client_id: None,
        client_secret: None,
        no_input: true,
        output: &output,
    })
    .await;

    let saved = std::fs::read_to_string(cfg_dir.join("config.toml")).unwrap();

    let _ = keyring::Entry::new(&svc, "sandbox:email").and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&svc, "sandbox:api-token").and_then(|e| e.delete_credential());
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    result.expect(
        "AC-006: auth refresh on an api_token profile must succeed even when \
         the tenant_info fetch fails (non-https url) — refresh routes through \
         the same login_token fallback chain as auth login",
    );
    assert!(
        !saved.contains("cloud_id"),
        "no cloud_id should have been written on a fetch failure. Saved:\n{saved}"
    );
}
