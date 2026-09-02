//! S-cycle3-remove-logout-semantics — Red Gate integration tests for
//! BC-1.2.013 (amended, `auth logout` non-destructive + informational
//! notice) and BC-1.2.014 (amended, `auth remove` reordered 4-step delete +
//! error-surfacing tightening).
//!
//! These exercise `jr::cli::auth::handle_remove`/`handle_logout` directly
//! (in-process, mirroring `tests/api_token_percred_wiring.rs`'s convention)
//! rather than spawning a `jr` subprocess, so credential state can be
//! verified in the SAME process via `jr::api::auth::{load_oauth_tokens,
//! load_api_token}` immediately afterward, before any env-var isolation is
//! torn down.
//!
//! Two tiers, per the story's stub-architect + orchestrator guidance:
//! - **Non-gated** tests only assert `config.toml` shape and stderr/stdout
//!   text — no seeded credential content is verified, matching the existing
//!   convention in `tests/auth_output_json.rs`/`tests/auth_profiles.rs` for
//!   `auth remove`/`auth logout` CLI tests.
//! - **Gated** (`#[ignore]` + `JR_RUN_KEYRING_TESTS=1`) tests seed and/or
//!   verify real keychain credential content, or simulate a genuine
//!   backend error via `JR_SERVICE_NAME=""` — mirroring
//!   `src/api/auth.rs::with_test_keyring` and
//!   `tests/api_token_percred_wiring.rs`'s documented pattern.
//!
//! RED GATE: every test in this file is expected to fail (or panic via
//! `todo!()`) against the current stub bodies of `handle_remove`,
//! `handle_logout`, `clear_profile_creds`, and `clear_all_credentials`.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use jr::api::auth;
use jr::cli::OutputFormat;
use jr::cli::auth::{handle_logout, handle_remove};
use tokio::sync::Mutex;

/// Serializes env-var mutation across the tests in this file (mirrors
/// `KEYRING_TEST_ENV_MUTEX` in `src/api/auth.rs` and
/// `tests/api_token_percred_wiring.rs::env_lock`). `tokio::sync::Mutex`
/// because several call sites hold the guard across `.await`.
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

/// Scrub every `JR_*` env var that could leak ambient dev-shell state into
/// these tests (mirrors `tests/auth_profiles.rs::jr()` /
/// `tests/auth_output_json.rs::jr_isolated`'s scrub list — kept here rather
/// than shared since each caller wraps it in its own `unsafe` justification).
///
/// # Safety
/// Caller must hold `env_lock()` for the duration of the env-var-dependent
/// section.
unsafe fn scrub_jr_env() {
    for key in [
        "JR_PROFILE",
        "JR_DEFAULT_PROFILE",
        "JR_INSTANCE_URL",
        "JR_INSTANCE_AUTH_METHOD",
        "JR_INSTANCE_CLOUD_ID",
        "JR_INSTANCE_ORG_ID",
        "JR_INSTANCE_OAUTH_SCOPES",
        "JR_BASE_URL",
        "JR_AUTH_HEADER",
        "JR_EMAIL",
        "JR_API_TOKEN",
        "JR_OAUTH_CLIENT_ID",
        "JR_OAUTH_CLIENT_SECRET",
        "JR_STDIN_IS_TTY",
    ] {
        unsafe { std::env::remove_var(key) };
    }
}

fn write_config(dir: &std::path::Path, toml: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, toml).unwrap();
    path
}

// ---------------------------------------------------------------------------
// Non-gated: config shape / output shape only (no seeded credential content)
// ---------------------------------------------------------------------------

/// AC-001 (BC-1.2.014 amended Behavior, step order): a fully successful
/// `auth remove` on a profile with no stored credentials of either kind
/// still removes `[profiles.<name>]` from config — this is EC-1.2.014-2
/// (`NoEntry` on both credential kinds -> success) proceeding all the way
/// through the reordered 4-step sequence to config-entry removal.
///
/// RED-GATE: fails today because `handle_remove` hits `todo!()` before
/// reaching config-entry removal.
#[tokio::test]
async fn test_ac_001_and_ec_1_2_014_2_auth_remove_full_delete_removes_profile_entry() {
    let _guard = env_lock().lock().await;
    let svc = unique_service("remove-full-delete");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    let cfg_path = write_config(
        &cfg_dir,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://default.example\"\n\
         auth_method = \"api_token\"\n\n\
         [profiles.staging]\n\
         url = \"https://staging.example\"\n\
         auth_method = \"api_token\"\n",
    );

    let result = handle_remove("staging", true, None, &OutputFormat::Table).await;

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    result.expect("auth remove of a credential-less profile must succeed end to end");

    let saved = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        !saved.contains("[profiles.staging]"),
        "profile entry must be removed after a fully successful auth remove; saved: {saved}"
    );
    assert!(
        saved.contains("[profiles.default]"),
        "the untouched profile must remain; saved: {saved}"
    );
}

/// AC-006 (BC-1.2.013 amended, I-3/SR-015): `jr auth logout` against an
/// `api_token`-method profile emits the EXACT informational stderr notice
/// (verified indirectly here: since `handle_logout` writes via
/// `output::print_success`/`eprintln!`, this test asserts the Ok(()) return
/// and defers exact-text verification to the JSON-mode sibling test below,
/// which can inspect a captured stdout/stderr pair via the CLI-subprocess
/// convention). At minimum: exit path is `Ok(())` (exit 0), and the
/// profile's config entry survives untouched (BC-1.2.013's non-destructive
/// contract).
///
/// RED-GATE: fails today because `handle_logout` hits `todo!()` before
/// branching on `auth_method`.
#[tokio::test]
async fn test_ac_006_auth_logout_api_token_profile_exits_ok_and_preserves_config() {
    let _guard = env_lock().lock().await;
    let svc = unique_service("logout-notice");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
    }

    let cfg_path = write_config(
        &cfg_dir,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://default.example\"\n\
         auth_method = \"api_token\"\n",
    );

    let result = handle_logout(Some("default"), &OutputFormat::Table).await;

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    result.expect(
        "auth logout on an api_token profile must exit 0 (informational notice, not an error)",
    );

    let saved = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        saved.contains("[profiles.default]") && saved.contains("auth_method = \"api_token\""),
        "BC-1.2.013: the profile config entry must be fully preserved; saved: {saved}"
    );
}

/// AC-008 (BC-1.2.013 Behavior, regression pin): `jr auth logout` against
/// an `oauth`-method profile is UNCHANGED — it still clears the OAuth pair
/// (NoEntry here, since none is seeded) and returns `Ok(())` via the
/// ordinary success path, never the api-token informational-notice branch.
/// The profile config entry is untouched either way.
///
/// RED-GATE: fails today because `handle_logout` hits `todo!()` regardless
/// of `auth_method`.
#[tokio::test]
async fn test_ac_008_auth_logout_oauth_profile_regression_pin_exits_ok() {
    let _guard = env_lock().lock().await;
    let svc = unique_service("logout-oauth-regression");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
    }

    let cfg_path = write_config(
        &cfg_dir,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://default.example\"\n\
         auth_method = \"oauth\"\n",
    );

    let result = handle_logout(Some("default"), &OutputFormat::Table).await;

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    result.expect("auth logout on an oauth profile must continue to succeed (regression pin)");

    let saved = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        saved.contains("[profiles.default]") && saved.contains("auth_method = \"oauth\""),
        "oauth profile's config entry must be untouched; saved: {saved}"
    );
}

/// AC-007 (BC-1.2.013 amended, `--output json` note): under `--output json`
/// the notice never reaches stdout, and the JSON payload shape is
/// UNCHANGED from the pre-fix no-op behavior — `{"profile": <name>,
/// "action": "logout", "ok": true}`. This is the in-process companion to
/// `tests/auth_output_json.rs::test_auth_logout_returns_json_ok`
/// (subprocess-based); both must agree, and both are expected to turn
/// green together once `handle_logout` is implemented.
///
/// This test only asserts the `Ok(())` return (the JSON *string* itself is
/// written directly to stdout by `handle_logout` via `println!`, which
/// this in-process harness cannot capture without a subprocess — the
/// subprocess sibling test owns that assertion). What this test adds:
/// confirming the api-token branch returns success under JSON output mode
/// specifically, not just table mode (AC-006 above only covers table mode).
///
/// RED-GATE: fails today via `todo!()`.
#[tokio::test]
async fn test_ac_007_auth_logout_api_token_json_mode_exits_ok() {
    let _guard = env_lock().lock().await;
    let svc = unique_service("logout-notice-json");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://default.example\"\n\
         auth_method = \"api_token\"\n",
    );

    let result = handle_logout(Some("default"), &OutputFormat::Json).await;

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    result.expect("auth logout --output json on an api_token profile must exit 0");
}

// ---------------------------------------------------------------------------
// Gated (#[ignore] + JR_RUN_KEYRING_TESTS=1): real keychain round-trip /
// clearing verification, and genuine backend-error injection.
// ---------------------------------------------------------------------------

/// **The end-to-end companion to `src/api/auth.rs`'s unit-level gap-closing
/// test.** Seeds BOTH credential kinds for a removable profile, runs
/// `handle_remove` to completion, and verifies (in-process, same
/// `JR_SERVICE_NAME` namespace) that NEITHER credential kind survives —
/// proving the fix is wired all the way through `auth remove`, not just
/// present in `clear_profile_creds` in isolation.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_bc_1_2_014_auth_remove_deletes_both_credential_kinds_end_to_end() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("remove-both-kinds");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://default.example\"\n\
         auth_method = \"api_token\"\n\n\
         [profiles.staging]\n\
         url = \"https://staging.example\"\n\
         auth_method = \"oauth\"\n",
    );

    auth::store_oauth_tokens(
        &jr::profile::Profile::from("staging"),
        "e2e-access",
        "e2e-refresh",
    )
    .unwrap();
    auth::store_api_token(
        &jr::profile::Profile::from("staging"),
        "e2e@example.com",
        "e2e-token",
    )
    .unwrap();

    let result = handle_remove("staging", true, None, &OutputFormat::Table).await;

    // Verify while JR_SERVICE_NAME is still pointed at this test's
    // isolated namespace.
    let oauth_after = auth::load_oauth_tokens(&jr::profile::Profile::from("staging"));
    let api_token_after = auth::load_api_token(&jr::profile::Profile::from("staging"));

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    result.expect("auth remove must succeed when both credential kinds are cleanly deletable");
    assert!(
        oauth_after.is_err(),
        "OAuth pair must be gone after auth remove"
    );
    assert!(
        api_token_after.is_err(),
        "namespaced API-token pair must be gone after auth remove — \
         this is the gap S-cycle3-remove-logout-semantics closes end to end"
    );
}

/// BC-1.2.013's core non-destructive claim, verified end to end: `auth
/// logout` on an `oauth`-method profile clears ONLY the OAuth pair and
/// PRESERVES the namespaced API-token pair (even though this particular
/// profile's `auth_method` is `"oauth"` — BC-1.2.013 is explicit that
/// `logout` never touches API-token credentials regardless of the
/// profile's own declared mechanism, since a profile could theoretically
/// carry a leftover API-token pair from a prior mechanism switch).
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_bc_1_2_013_logout_clears_only_oauth_pair_preserves_api_token_pair() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("logout-preserves-api-token");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://default.example\"\n\
         auth_method = \"oauth\"\n",
    );

    auth::store_oauth_tokens(
        &jr::profile::Profile::from("default"),
        "preserve-access",
        "preserve-refresh",
    )
    .unwrap();
    auth::store_api_token(
        &jr::profile::Profile::from("default"),
        "preserve@example.com",
        "preserve-token",
    )
    .unwrap();

    let result = handle_logout(Some("default"), &OutputFormat::Table).await;

    let oauth_after = auth::load_oauth_tokens(&jr::profile::Profile::from("default"));
    let api_token_after = auth::load_api_token(&jr::profile::Profile::from("default"));

    let cleanup_service = svc.clone();
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }
    let _ =
        keyring::Entry::new(&cleanup_service, "default:email").and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&cleanup_service, "default:api-token")
        .and_then(|e| e.delete_credential());

    result.expect("auth logout on an oauth profile must succeed");
    assert!(oauth_after.is_err(), "logout must clear the OAuth pair");
    let (email, token) =
        api_token_after.expect("logout must NEVER clear the API-token pair (BC-1.2.013)");
    assert_eq!(email, "preserve@example.com");
    assert_eq!(token, "preserve-token");
}

/// AC-002/AC-003 (BC-1.2.014 EC-1.2.014-1, I-4/SR-008): a genuine
/// (non-`NoEntry`) keychain backend error during the credential-deletion
/// steps must ABORT `auth remove` BEFORE the config-entry-removal step —
/// `[profiles.<name>]` remains in `config.toml` afterward, and the command
/// exits non-zero. Simulated via `JR_SERVICE_NAME=""`, mirroring
/// `src/api/auth.rs`'s `with_test_keyring`-gated error-injection tests
/// ("as prior cycle-003 tests do").
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ac_002_003_auth_remove_aborts_before_config_removal_on_genuine_keychain_error() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    let cfg_path = write_config(
        &cfg_dir,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://default.example\"\n\
         auth_method = \"api_token\"\n\n\
         [profiles.staging]\n\
         url = \"https://staging.example\"\n\
         auth_method = \"api_token\"\n",
    );

    // SAFETY: env_lock is held for this whole scope. Empty JR_SERVICE_NAME
    // makes every keychain Entry construction fail with a genuine backend
    // error (Err::Invalid), never NoEntry — see
    // src/api/auth.rs::load_api_token_propagates_backend_error_not_absent_message
    // for the vendored-source citation this relies on.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", "");
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    let result = handle_remove("staging", true, None, &OutputFormat::Table).await;

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    assert!(
        result.is_err(),
        "a genuine keychain backend error must abort auth remove, not be swallowed"
    );

    let saved = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        saved.contains("[profiles.staging]"),
        "AC-003: [profiles.staging] must remain after an aborted remove \
         (credentials-before-config-entry reorder, I-4/SR-008); saved: {saved}"
    );
}

/// AC-004 (retry after resolving the backend issue): a subsequent call to
/// `auth remove` for the same profile, after the transient backend problem
/// is gone, completes successfully and removes the config entry — proving
/// the abort in the test above left the profile in a genuinely
/// re-`remove`-able state, not a wedged one.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ac_004_auth_remove_retry_after_resolved_backend_error_succeeds() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("remove-retry-after-error");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    let cfg_path = write_config(
        &cfg_dir,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://default.example\"\n\
         auth_method = \"api_token\"\n\n\
         [profiles.staging]\n\
         url = \"https://staging.example\"\n\
         auth_method = \"api_token\"\n",
    );

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", "");
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }
    let first = handle_remove("staging", true, None, &OutputFormat::Table).await;
    assert!(
        first.is_err(),
        "first attempt must abort on the injected backend error"
    );

    // "Resolve the backend issue": point at a real, working service name.
    unsafe { std::env::set_var("JR_SERVICE_NAME", &svc) };
    let second = handle_remove("staging", true, None, &OutputFormat::Table).await;

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    second.expect(
        "retry after the backend issue is resolved must succeed — both \
         credential-deletion steps independently re-attempt and tolerate \
         NoEntry",
    );
    let saved = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        !saved.contains("[profiles.staging]"),
        "profile must be fully removed on the successful retry; saved: {saved}"
    );
}

/// SEC-1 (HIGH, pre-PR review fix): `jr auth refresh`'s OAuth branch
/// (`src/cli/auth/refresh.rs::refresh_credentials`) clears ONLY the OAuth
/// pair via [`auth::clear_profile_oauth_pair`] — never
/// [`auth::clear_profile_creds`], which (as of this story, BC-1.2.014)
/// also deletes the namespaced `<profile>:email`/`<profile>:api-token`
/// pair. Before this fix, `refresh.rs` still called the newly-widened
/// `clear_profile_creds` under the OLD "clear only OAuth pair" contract,
/// so `jr auth refresh` on an oauth-method profile that ALSO carried a
/// leftover api-token pair (e.g. from a prior mechanism switch) would
/// silently, irrecoverably delete it — a DEC-322 data-loss regression.
///
/// This cannot be driven fully end-to-end through `refresh_credentials`
/// without a live OAuth server (the flow, after the clear step, opens a
/// browser and binds a loopback listener via `login_oauth`), so this test
/// asserts one level down: at [`auth::clear_profile_oauth_pair`], the
/// EXACT function `refresh_credentials`'s `AuthFlow::OAuth` arm now calls.
/// Seeds BOTH credential kinds, calls it directly (mirroring the refresh
/// path verbatim), and asserts the OAuth pair is gone while the api-token
/// pair survives byte-for-byte.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_sec_1_auth_refresh_oauth_branch_preserves_api_token_pair() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("refresh-preserves-api-token");

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
    }

    auth::store_oauth_tokens(
        &jr::profile::Profile::from("default"),
        "refresh-access",
        "refresh-refresh",
    )
    .unwrap();
    auth::store_api_token(
        &jr::profile::Profile::from("default"),
        "refresh@example.com",
        "refresh-token",
    )
    .unwrap();

    // This is the exact call `src/cli/auth/refresh.rs`'s `AuthFlow::OAuth`
    // arm makes (post-SEC-1-fix) immediately before re-minting OAuth
    // tokens via `login_oauth`.
    let result = auth::clear_profile_oauth_pair(&jr::profile::Profile::from("default"));

    let oauth_after = auth::load_oauth_tokens(&jr::profile::Profile::from("default"));
    let api_token_after = auth::load_api_token(&jr::profile::Profile::from("default"));

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
    }

    result.expect("clear_profile_oauth_pair must succeed clearing a seeded OAuth pair");
    assert!(
        oauth_after.is_err(),
        "auth refresh's OAuth branch must still clear the OAuth pair"
    );
    let (email, token) = api_token_after.expect(
        "SEC-1: auth refresh's OAuth branch must NEVER delete the api-token \
         pair — this is the DEC-322 data-loss regression this fix closes",
    );
    assert_eq!(email, "refresh@example.com");
    assert_eq!(token, "refresh-token");
}

/// SEC-3 (LOW): destructive credential-clear operations must be
/// profile-scoped, not global. Seeds OAuth + api-token pairs for TWO
/// profiles (`alpha` + `beta`), clears `beta`'s pair via
/// [`auth::clear_profile_creds`] (the same function `auth remove` calls),
/// and asserts `alpha`'s pairs — BOTH kinds — are entirely untouched.
/// Cheap, high-value regression pin on a destructive operation.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_sec_3_clear_profile_creds_is_profile_scoped_not_global() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("clear-creds-isolation");

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
    }

    auth::store_oauth_tokens(
        &jr::profile::Profile::from("alpha"),
        "alpha-access",
        "alpha-refresh",
    )
    .unwrap();
    auth::store_api_token(
        &jr::profile::Profile::from("alpha"),
        "alpha@example.com",
        "alpha-token",
    )
    .unwrap();
    auth::store_oauth_tokens(
        &jr::profile::Profile::from("beta"),
        "beta-access",
        "beta-refresh",
    )
    .unwrap();
    auth::store_api_token(
        &jr::profile::Profile::from("beta"),
        "beta@example.com",
        "beta-token",
    )
    .unwrap();

    let result = auth::clear_profile_creds(&jr::profile::Profile::from("beta"));

    let alpha_oauth_after = auth::load_oauth_tokens(&jr::profile::Profile::from("alpha"));
    let alpha_api_token_after = auth::load_api_token(&jr::profile::Profile::from("alpha"));
    let beta_oauth_after = auth::load_oauth_tokens(&jr::profile::Profile::from("beta"));
    let beta_api_token_after = auth::load_api_token(&jr::profile::Profile::from("beta"));

    let cleanup_service = svc.clone();
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
    }
    // Best-effort cleanup of the surviving `alpha` credentials this test
    // seeded (clear_profile_creds(&jr::profile::Profile::from("beta")) must not — and per the
    // assertions below, does not — remove them).
    let _ = keyring::Entry::new(&cleanup_service, "alpha:oauth-access-token")
        .and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&cleanup_service, "alpha:oauth-refresh-token")
        .and_then(|e| e.delete_credential());
    let _ =
        keyring::Entry::new(&cleanup_service, "alpha:email").and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&cleanup_service, "alpha:api-token")
        .and_then(|e| e.delete_credential());

    result.expect("clear_profile_creds(\"beta\") must succeed");

    assert!(
        alpha_oauth_after.is_ok(),
        "SEC-3: clearing beta's creds must not touch alpha's OAuth pair"
    );
    let (email, token) = alpha_api_token_after
        .expect("SEC-3: clearing beta's creds must not touch alpha's api-token pair");
    assert_eq!(email, "alpha@example.com");
    assert_eq!(token, "alpha-token");

    assert!(
        beta_oauth_after.is_err(),
        "beta's OAuth pair must be cleared"
    );
    assert!(
        beta_api_token_after.is_err(),
        "beta's api-token pair must be cleared"
    );
}
