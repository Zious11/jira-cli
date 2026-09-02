//! S-cycle3-percred-storage — wiring tests for BC-1.4.031's namespaced
//! per-profile API-token storage, exercising two call sites end to end
//! against the REAL OS keychain backend:
//!
//! - AC-009 (BC-1.1.009/BC-1.1.010/BC-1.2.017 Effects clauses):
//!   `login_token` writes the namespaced `<profile>:email`/
//!   `<profile>:api-token` pair via `store_api_token`, never the
//!   shared/flat `email`/`api-token` keys.
//! - AC-003 (BC-1.4.031 postcondition 3): `JiraClient::from_config`'s
//!   `api_token` branch reads via `load_api_token` (namespaced), never the
//!   old flat-key reader — proven by showing a profile with ONLY the
//!   legacy flat pair stored (and no namespaced pair) fails to
//!   authenticate, which would NOT be the case if the old flat reader were
//!   still consulted.
//!
//! KEYRING GATE: `JR_RUN_KEYRING_TESTS=1` required (real OS keychain; see
//! CLAUDE.md "Keyring round-trip tests are gated..."). Both tests here are
//! `#[ignore]`d AND independently no-op (return immediately) when the gate
//! env var isn't set to `"1"`, mirroring `src/api/auth.rs`'s
//! `with_test_keyring` helper and `tests/oauth_refresh_integration.rs`'s
//! documented pattern (`JR_RUN_KEYRING_TESTS=1 cargo test --test
//! api_token_percred_wiring -- --include-ignored`).

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use jr::api::auth;
use jr::api::client::JiraClient;
use jr::config::{Config, GlobalConfig, ProjectConfig};
use tokio::sync::Mutex;

/// Serializes env-var mutation across the tests in this file (mirrors
/// `KEYRING_TEST_ENV_MUTEX` in `src/api/auth.rs`). `tokio::sync::Mutex` (not
/// `std::sync::Mutex`) because the guard must be held across `.await` in
/// `test_login_token_writes_namespaced_pair_not_shared_flat` — same
/// rationale as `tests/oauth_refresh_integration.rs::harness::env_lock`.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn keyring_gate_active() -> bool {
    std::env::var("JR_RUN_KEYRING_TESTS").as_deref() == Ok("1")
}

fn unique_service(tag: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("jr-jira-cli-test-{tag}-{}-{}", std::process::id(), n)
}

/// Best-effort cleanup of a profile's namespaced api-token pair. Uses the
/// raw `keyring` crate directly (not `jr::api::auth::entry`, which is
/// private) — targets whatever `JR_SERVICE_NAME` currently resolves to.
fn cleanup_api_token(service: &str, profile: &str) {
    let _ = keyring::Entry::new(service, &format!("{profile}:email"))
        .and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(service, &format!("{profile}:api-token"))
        .and_then(|e| e.delete_credential());
}

/// AC-009 (BC-1.1.009/BC-1.1.010/BC-1.2.017): `jr auth login --profile
/// <new>` (exercised here via the `login_token` handler directly, with
/// flag-equivalent args so no prompt is needed) writes `<new>:email` /
/// `<new>:api-token` — never a shared/flat pair.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_login_token_writes_namespaced_pair_not_shared_flat() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;

    let svc = unique_service("login-wiring");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    std::fs::create_dir_all(&cfg_dir).unwrap();

    // SAFETY: ENV_LOCK is held for this whole scope, so no other test in
    // this binary can observe a half-applied env state.
    unsafe {
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
    }

    let profile = "sandbox";
    let login_result = jr::cli::auth::login_token(
        profile,
        Some("wired@example.com".to_string()),
        Some("wired-token-123".to_string()),
        true,
    )
    .await
    .map_err(|e| format!("{e:#}"));

    // Read back while JR_SERVICE_NAME/JR_CONFIG_DIR are still pointed at
    // this test's isolated namespace.
    let namespaced = auth::load_api_token(profile);
    let legacy = auth::load_legacy_flat_api_token();

    cleanup_api_token(&svc, profile);
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    login_result.expect("login_token must succeed");

    let (email, token) = namespaced.expect("namespaced pair must be readable after login_token");
    assert_eq!(email, "wired@example.com");
    assert_eq!(token, "wired-token-123");

    assert!(
        legacy.is_err(),
        "login_token must not write the shared/flat email/api-token keys"
    );
}

/// AC-003 (BC-1.4.031 postcondition 3): `JiraClient::from_config`'s
/// `api_token` branch must read exclusively via `load_api_token`
/// (namespaced), never `load_legacy_flat_api_token`.
///
/// Proven two ways in one test: (1) a profile with the namespaced pair
/// stored authenticates successfully; (2) a DIFFERENT profile with ONLY
/// the legacy flat pair stored (no namespaced pair at all) fails —
/// if `from_config` still consulted the legacy flat reader, this second
/// case would incorrectly succeed.
#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn test_from_config_api_token_branch_reads_namespaced_never_legacy_flat() {
    if !keyring_gate_active() {
        return;
    }
    // Plain sync test (no tokio runtime), so `blocking_lock` rather than
    // `.lock().await`.
    let _guard = env_lock().blocking_lock();

    let svc = unique_service("client-wiring");

    // SAFETY: ENV_LOCK is held for this whole scope.
    unsafe {
        std::env::set_var("JR_SERVICE_NAME", &svc);
        // Route from_config's URL resolution to a placeholder; from_config
        // never makes an HTTP call, so this doesn't need to be reachable.
        std::env::set_var("JR_BASE_URL", "http://127.0.0.1:1");
        std::env::remove_var("JR_AUTH_HEADER");
    }

    // Positive case: namespaced pair present -> from_config succeeds.
    auth::store_api_token("with-namespaced", "ac3@example.com", "ac3-token").unwrap();
    let config_ok = Config {
        global: GlobalConfig::default(),
        project: ProjectConfig::default(),
        active_profile_name: "with-namespaced".to_string(),
    };
    let ok_result = JiraClient::from_config(&config_ok, false, false);

    // Negative case: ONLY the legacy flat pair present (a different
    // profile, no namespaced pair) -> from_config must FAIL.
    auth::store_legacy_flat_api_token("legacy@example.com", "legacy-token").unwrap();
    let config_legacy_only = Config {
        global: GlobalConfig::default(),
        project: ProjectConfig::default(),
        active_profile_name: "legacy-only-profile".to_string(),
    };
    let legacy_result = JiraClient::from_config(&config_legacy_only, false, false);

    cleanup_api_token(&svc, "with-namespaced");
    let _ = keyring::Entry::new(&svc, "email").and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&svc, "api-token").and_then(|e| e.delete_credential());
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_BASE_URL");
    }

    assert!(
        ok_result.is_ok(),
        "from_config must succeed when the namespaced pair is present: {:?}",
        ok_result.err().map(|e| format!("{e:#}"))
    );
    assert!(
        legacy_result.is_err(),
        "from_config's api_token branch must NOT fall back to the legacy flat pair \
         (AC-003) — it must read exclusively via load_api_token"
    );
}
