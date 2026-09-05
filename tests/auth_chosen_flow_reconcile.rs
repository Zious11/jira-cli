//! S-cycle3-chosen-flow-reconcile — Two-Step Red Gate TESTS (step 2 of 2,
//! BC-5.38.001) for DEC-321's `chosen_flow_for_profile` override removal
//! and BC-1.2.051 Invariant 2 (I-6)'s "relogin-then-replace" ordering fix.
//!
//! Stubs (`todo!()`) were committed at `b2a0e1a4`:
//! - `cli::auth::chosen_flow_for_profile` (now 1-arg, no `oauth_override`)
//! - `cli::auth::refresh_credentials`'s relogin-then-replace credential
//!   obtain/store sequence and its terminal success/error output.
//!
//! At that point everything in this file drove `refresh_credentials` end
//! to end and was therefore RED: `chosen_flow_for_profile` panicked via
//! `todo!()` on its very first call inside `refresh_credentials`, before
//! any of the already-implemented (non-`todo!()`'d) control flow below it —
//! the BC-1.1.016 non-interactive OAuth guard, the BC-1.2.049/050 notices,
//! the URL-completeness check — was ever reached.
//!
//! **Status (FIX-F5-CYCLE4-2 doc sweep):** `chosen_flow_for_profile` and
//! the relogin-then-replace sequence shipped per BC-1.2.048/BC-1.2.051, and
//! this file's tests have since carried the `test_ac_007_...` cloud_id
//! reconciliation coverage added by S-cycle4-cloud-id-correctness plus
//! review fixes from the cycle-004 F5 scoped adversarial review. Every test
//! in this file is green today (keyring-gated tests remain `#[ignore]`d
//! pending `JR_RUN_KEYRING_TESTS=1`, by design). The RED narrative above is
//! retained as historical record, not current behavior.
//!
//! # Why the non-gated tests below never touch a real browser or the OS
//! keychain
//!
//! Every non-gated case in this file fixes `no_input: true` and supplies
//! neither `--email`/`--token` nor `$JR_EMAIL`/`$JR_API_TOKEN`, so:
//! - Any case that resolves to `AuthFlow::OAuth` hits BC-1.1.016's airtight
//!   non-interactive guard (`check_noninteractive_oauth_guard`, already
//!   real, not `todo!()`'d) BEFORE any OAuth app-credential resolution,
//!   listener bind, or browser-open call.
//! - Any case that resolves to `AuthFlow::Token` fails at
//!   `login_token`'s `resolve_credential(email, ...)` no-input/missing-
//!   value check BEFORE `auth::store_api_token` (the only keychain WRITE
//!   in this path) is ever reached.
//!
//! Both failure points are reached only by fully implementing this
//! story's stubs — until then, the panic in `chosen_flow_for_profile`
//! pre-empts both.
//!
//! Real-keychain I-6 (relogin-then-replace) verification lives in the
//! gated tier at the bottom of this file (`JR_RUN_KEYRING_TESTS=1`).

use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use assert_cmd::Command as AssertCommand;
use jr::api::auth;
use jr::cli::OutputFormat;
use jr::cli::auth::{NONINTERACTIVE_OAUTH_GUARD_MESSAGE, RefreshArgs, refresh_credentials};
use jr::error::JrError;
use jr::profile::Profile;
use proptest::prelude::*;
use tempfile::TempDir;
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Shared helpers (kept local, not tests/common — mirrors
// tests/auth_oauth_default_creation.rs's stated rationale: each caller
// wraps its own `unsafe` justification).
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

// ---------------------------------------------------------------------------
// Tier 1: subprocess, no keychain reachable — AC-002/003/004/008.
// ---------------------------------------------------------------------------

/// Runs `jr --no-input auth refresh [flags] --profile sandbox` against an
/// isolated config containing exactly one profile ("sandbox") with the
/// given stored `auth_method`, and returns (exit_code, stderr).
fn run_refresh_subprocess(stored_auth_method: &str, extra_flags: &[&str]) -> (i32, String) {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    write_config(
        &config_dir.path().join("jr"),
        &format!(
            "default_profile = \"sandbox\"\n\n\
             [profiles.sandbox]\n\
             url = \"https://example.atlassian.net\"\n\
             auth_method = \"{stored_auth_method}\"\n"
        ),
    );

    let mut cmd = AssertCommand::cargo_bin("jr").unwrap();
    cmd.env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .env("JR_SERVICE_NAME", unique_service("subprocess"))
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        .env_remove("JR_OAUTH_CLIENT_ID")
        .env_remove("JR_OAUTH_CLIENT_SECRET")
        .env_remove("JR_INSTANCE_URL")
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .env_remove("JR_INSTANCE_CLOUD_ID")
        .env_remove("JR_INSTANCE_ORG_ID")
        .env_remove("JR_INSTANCE_OAUTH_SCOPES")
        .env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .args(["--no-input", "auth", "refresh", "--profile", "sandbox"])
        .args(extra_flags);

    let output = cmd.output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// AC-002 (BC-1.2.051 EC-1.2.051-1): `refresh --oauth` on an `api_token`
/// profile proceeds as an api-token refresh — no OAuth browser flow. Proven
/// by showing the failure is the Token-flow's missing-email error, never
/// BC-1.1.016's OAuth guard message (which would only fire if `--oauth`
/// still forced `AuthFlow::OAuth`, i.e. the removed override).
#[test]
fn test_ac_002_refresh_oauth_flag_on_api_token_profile_uses_token_flow_not_oauth() {
    let (code, stderr) = run_refresh_subprocess("api_token", &["--oauth"]);

    assert_eq!(
        code, 64,
        "must exit 64 (UserError) from the Token flow's missing-email check, \
         not clap exit 2 (AC-008: --oauth stays syntactically accepted) or a \
         panic. stderr: {stderr}"
    );
    assert!(
        !stderr.contains(NONINTERACTIVE_OAUTH_GUARD_MESSAGE),
        "DEC-321: --oauth must NOT force AuthFlow::OAuth on an api_token \
         profile — the guard message must never appear here. stderr: {stderr}"
    );
    assert!(
        stderr.contains("--email") && stderr.contains("JR_EMAIL"),
        "the api-token relogin path (login_token) must have been reached, \
         proving the flow resolved to Token despite --oauth. stderr: {stderr}"
    );
}

/// AC-003 (BC-1.2.048 VP-AUTHDX-003, specific instance): `refresh
/// --api-token` on an `oauth` profile proceeds as an OAuth relogin — no
/// api-token credential prompt. Proven by showing the failure IS
/// BC-1.1.016's OAuth guard message (proving flow resolved to OAuth
/// despite `--api-token`), never the Token-flow's missing-email error.
#[test]
fn test_ac_003_refresh_api_token_flag_on_oauth_profile_uses_oauth_flow_not_token() {
    let (code, stderr) = run_refresh_subprocess("oauth", &["--api-token"]);

    assert_eq!(
        code, 64,
        "must exit 64 (UserError) from BC-1.1.016's non-interactive OAuth \
         guard, not clap exit 2 (AC-008: --api-token stays syntactically \
         accepted) or a panic. stderr: {stderr}"
    );
    assert!(
        stderr.contains(NONINTERACTIVE_OAUTH_GUARD_MESSAGE),
        "DEC-321: --api-token must NOT force AuthFlow::Token on an oauth \
         profile — the flow must still resolve to OAuth and hit the \
         non-interactive guard. stderr: {stderr}"
    );
    assert!(
        !(stderr.contains("--email") && stderr.contains("JR_EMAIL")),
        "no api-token credential prompt/error should ever be reached here. \
         stderr: {stderr}"
    );
}

/// AC-004 (BC-1.2.051 EC-1.2.051-2), api_token half: `refresh` with no flag
/// at all is unaffected — regression pin against the api_token profile.
#[test]
fn test_ac_004_refresh_no_flag_on_api_token_profile_is_unaffected() {
    let (code, stderr) = run_refresh_subprocess("api_token", &[]);

    assert_eq!(code, 64, "stderr: {stderr}");
    assert!(
        !stderr.contains(NONINTERACTIVE_OAUTH_GUARD_MESSAGE),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("--email") && stderr.contains("JR_EMAIL"),
        "stderr: {stderr}"
    );
}

/// AC-004 (BC-1.2.051 EC-1.2.051-2), oauth half: same regression pin
/// against the oauth profile.
#[test]
fn test_ac_004_refresh_no_flag_on_oauth_profile_is_unaffected() {
    let (code, stderr) = run_refresh_subprocess("oauth", &[]);

    assert_eq!(code, 64, "stderr: {stderr}");
    assert!(
        stderr.contains(NONINTERACTIVE_OAUTH_GUARD_MESSAGE),
        "stderr: {stderr}"
    );
}

/// AC-008 (BC-1.2.051 postcondition 2): both flags remain syntactically
/// accepted on `refresh` regardless of the profile's actual mechanism — no
/// clap error (exit 2) is ever produced by the flag itself. The four tests
/// above already assert exit code 64 (not 2) for every generated
/// mechanism/flag pairing that reaches application logic; this test pins
/// the pure clap-parsing half in isolation (`--help` never touches config
/// or keychain at all).
#[test]
fn test_ac_008_refresh_oauth_and_api_token_flags_both_parse_with_help() {
    for flag in ["--oauth", "--api-token"] {
        AssertCommand::cargo_bin("jr")
            .unwrap()
            .args(["auth", "refresh", flag, "--help"])
            .assert()
            .success();
    }
}

// ---------------------------------------------------------------------------
// Tier 2: in-process, no keychain reachable — AC-005 / VP-AUTHDX-003
// property test (2x3 mechanism/flag matrix).
// ---------------------------------------------------------------------------

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoredMethod {
    ApiToken,
    OAuth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestedFlag {
    None,
    Oauth,
    ApiToken,
}

/// Drives `refresh_credentials` in-process for one (stored auth_method,
/// requested flag) combination against an isolated, freshly-scoped
/// `JR_CONFIG_DIR`/`JR_SERVICE_NAME`, and returns the resulting error text.
/// Every combination is engineered to fail before any keychain write (see
/// the module doc comment) — safe to run un-gated.
fn run_case(stored: StoredMethod, flag: RequestedFlag) -> String {
    // Not inside any async execution context here (this is a plain sync
    // fn, called directly from the proptest closure below) — safe to use
    // `blocking_lock` on the shared `tokio::sync::Mutex`.
    let _guard = env_lock().blocking_lock();

    let tmp = TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let svc = unique_service("vp003");
    let method_str = match stored {
        StoredMethod::ApiToken => "api_token",
        StoredMethod::OAuth => "oauth",
    };
    write_config(
        &cfg_dir,
        &format!(
            "default_profile = \"sandbox\"\n\n\
             [profiles.sandbox]\n\
             url = \"https://example.atlassian.net\"\n\
             auth_method = \"{method_str}\"\n"
        ),
    );

    // SAFETY: env_lock is held for this whole scope.
    unsafe {
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        for key in [
            "JR_EMAIL",
            "JR_API_TOKEN",
            "JR_OAUTH_CLIENT_ID",
            "JR_OAUTH_CLIENT_SECRET",
            "JR_PROFILE",
            "JR_DEFAULT_PROFILE",
        ] {
            std::env::remove_var(key);
        }
    }

    let (oauth_flag, api_token_flag) = match flag {
        RequestedFlag::None => (false, false),
        RequestedFlag::Oauth => (true, false),
        RequestedFlag::ApiToken => (false, true),
    };

    let output = OutputFormat::Table;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(refresh_credentials(RefreshArgs {
        profile: Some("sandbox"),
        oauth: oauth_flag,
        api_token: api_token_flag,
        email: None,
        token: None,
        client_id: None,
        client_secret: None,
        no_input: true,
        output: &output,
    }));

    // SAFETY: still under env_lock.
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    let err = result.expect_err(
        "every generated (stored, flag) case is engineered to fail before any \
         keychain write — see run_case's doc comment. A successful \
         refresh_credentials() here means something reached out to the real \
         network or keychain, which must not happen in this test.",
    );
    let jr_err = err
        .downcast_ref::<JrError>()
        .expect("refresh_credentials errors must be JrError so they map to a real exit code");
    jr_err.to_string()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]

    /// VP-AUTHDX-003 (BC-1.2.048): for the full 2x3 cross product of
    /// {profile's stored auth_method} x {flag passed to `auth refresh`},
    /// the mechanism actually used is ALWAYS the profile's stored
    /// auth_method, never the flag — proven via each flow's distinct,
    /// mutually-exclusive failure signature (the OAuth guard message vs.
    /// the Token flow's missing-email error), which only one flow can ever
    /// produce.
    #[test]
    fn test_vp_authdx_003_refresh_mechanism_never_follows_the_flag_only_the_profile(
        stored_idx in 0u8..2,
        flag_idx in 0u8..3,
    ) {
        let stored = if stored_idx == 0 { StoredMethod::ApiToken } else { StoredMethod::OAuth };
        let flag = match flag_idx {
            0 => RequestedFlag::None,
            1 => RequestedFlag::Oauth,
            _ => RequestedFlag::ApiToken,
        };

        let msg = run_case(stored, flag);

        match stored {
            StoredMethod::OAuth => {
                prop_assert_eq!(
                    &msg,
                    NONINTERACTIVE_OAUTH_GUARD_MESSAGE,
                    "profile.auth_method=oauth (requested flag={:?}) must ALWAYS resolve \
                     to AuthFlow::OAuth regardless of the requested flag",
                    flag
                );
            }
            StoredMethod::ApiToken => {
                prop_assert_ne!(
                    &msg,
                    NONINTERACTIVE_OAUTH_GUARD_MESSAGE,
                    "profile.auth_method=api_token (requested flag={:?}) must NEVER hit \
                     the OAuth guard — that would mean the flag overrode the stored \
                     mechanism",
                    flag
                );
                prop_assert!(
                    msg.contains("--email") && msg.contains("JR_EMAIL"),
                    "profile.auth_method=api_token (requested flag={:?}) must ALWAYS \
                     resolve to AuthFlow::Token, proven by login_token's missing-email \
                     error. Got: {}",
                    flag,
                    msg
                );
            }
        }
    }
}

/// Fixed regression seed 1 of 2 (BC-1.2.051 Verification Properties
/// paragraph): "an api_token profile with refresh --oauth" — same
/// assertion as `test_ac_002_...` above, kept as an explicit, named,
/// non-parameterized pin independent of the property test's random draws.
#[test]
fn test_vp_authdx_003_fixed_seed_api_token_profile_with_oauth_flag() {
    let msg = run_case(StoredMethod::ApiToken, RequestedFlag::Oauth);
    assert_ne!(msg, NONINTERACTIVE_OAUTH_GUARD_MESSAGE);
    assert!(msg.contains("--email") && msg.contains("JR_EMAIL"), "{msg}");
}

/// Fixed regression seed 2 of 2: "an oauth profile with refresh
/// --api-token".
#[test]
fn test_vp_authdx_003_fixed_seed_oauth_profile_with_api_token_flag() {
    let msg = run_case(StoredMethod::OAuth, RequestedFlag::ApiToken);
    assert_eq!(msg, NONINTERACTIVE_OAUTH_GUARD_MESSAGE);
}

// ---------------------------------------------------------------------------
// Tier 3 (gated, #[ignore] + JR_RUN_KEYRING_TESTS=1): real keychain
// round-trip verification of BC-1.2.051 Invariant 2 (I-6)
// "relogin-then-replace" — mirrors tests/api_token_percred_wiring.rs's and
// tests/auth_oauth_default_creation.rs's gated-tier conventions.
// ---------------------------------------------------------------------------

/// AC-006 positive path: a SUCCESSFUL relogin replaces the stored pair with
/// the newly-obtained values (proving relogin-then-replace's "replace"
/// half actually happens on success, not just that failure is safe).
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ac_006_successful_relogin_replaces_existing_api_token_pair() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;

    let svc = unique_service("relogin-replace-success");
    let tmp = TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");

    // SAFETY: env_lock (a tokio::sync::Mutex, safe to hold across the
    // .await below — see this file's `use tokio::sync::Mutex`) is held
    // for this whole scope.
    unsafe {
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        for key in [
            "JR_OAUTH_CLIENT_ID",
            "JR_OAUTH_CLIENT_SECRET",
            "JR_PROFILE",
            "JR_DEFAULT_PROFILE",
        ] {
            std::env::remove_var(key);
        }
    }

    write_config(
        &cfg_dir,
        "default_profile = \"sandbox\"\n\n\
         [profiles.sandbox]\n\
         url = \"https://example.atlassian.net\"\n\
         auth_method = \"api_token\"\n",
    );

    auth::store_api_token(&Profile::from("sandbox"), "old@example.com", "old-token").unwrap();

    let output = OutputFormat::Table;
    let result = refresh_credentials(RefreshArgs {
        profile: Some("sandbox"),
        oauth: false,
        api_token: false,
        email: Some("new@example.com".to_string()),
        token: Some("new-token".to_string()),
        client_id: None,
        client_secret: None,
        no_input: true,
        output: &output,
    })
    .await;

    let after = auth::load_api_token(&Profile::from("sandbox"));

    let _ = keyring::Entry::new(&svc, "sandbox:email").and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&svc, "sandbox:api-token").and_then(|e| e.delete_credential());
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    result.expect("relogin with valid flag-supplied credentials must succeed");
    let (email, token) =
        after.expect("namespaced pair must be readable after a successful refresh");
    assert_eq!(
        email, "new@example.com",
        "relogin-then-replace must REPLACE on success"
    );
    assert_eq!(
        token, "new-token",
        "relogin-then-replace must REPLACE on success"
    );
}

/// AC-007 (BC-1.2.051 Invariant 2 / I-6, EC-1.2.051-3) — THE LOAD-BEARING
/// SAFETY TEST: a refresh that fails to obtain a usable replacement
/// credential must leave the profile's existing, still-working credential
/// pair COMPLETELY INTACT. Simulated failure mode: non-interactive relogin
/// with no `--email`/`--token`/env supplied — `resolve_credential`'s
/// documented no-input/missing-value failure, one of the concrete failure
/// modes EC-1.2.051-3 names ("an interactive re-prompt is cancelled" —
/// here, no-input makes that cancellation unconditional and deterministic
/// rather than depending on a live TTY).
///
/// This test is engineered so `auth::store_api_token` (the one keychain
/// WRITE in this path) is NEVER reached: relogin-then-replace's contract
/// (BC-1.2.051 Invariant 2 option (a)) is "obtain/confirm the new value
/// FIRST, then store — never a separate delete step beforehand," so a
/// failure to obtain must mean zero keychain writes occurred at all, not
/// merely that a delete-then-store sequence rolled back.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ac_007_failed_relogin_preserves_existing_api_token_pair() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;

    let svc = unique_service("relogin-replace-failure");
    let tmp = TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");

    unsafe {
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        for key in [
            "JR_EMAIL",
            "JR_API_TOKEN",
            "JR_OAUTH_CLIENT_ID",
            "JR_OAUTH_CLIENT_SECRET",
            "JR_PROFILE",
            "JR_DEFAULT_PROFILE",
        ] {
            std::env::remove_var(key);
        }
    }

    write_config(
        &cfg_dir,
        "default_profile = \"sandbox\"\n\n\
         [profiles.sandbox]\n\
         url = \"https://example.atlassian.net\"\n\
         auth_method = \"api_token\"\n",
    );

    // Seed the profile's PRE-refresh, still-working credential pair.
    auth::store_api_token(&Profile::from("sandbox"), "old@example.com", "old-token").unwrap();

    let output = OutputFormat::Table;
    // no_input=true, no --email/--token, no $JR_EMAIL/$JR_API_TOKEN ->
    // login_token's resolve_credential(email) fails immediately, before
    // store_api_token is ever called.
    let result = refresh_credentials(RefreshArgs {
        profile: Some("sandbox"),
        oauth: false,
        api_token: false,
        email: None,
        token: None,
        client_id: None,
        client_secret: None,
        no_input: true,
        output: &output,
    })
    .await;

    // THE assertion: read back the pair while still in the isolated
    // keychain namespace, BEFORE any cleanup.
    let after = auth::load_api_token(&Profile::from("sandbox"));

    let _ = keyring::Entry::new(&svc, "sandbox:email").and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&svc, "sandbox:api-token").and_then(|e| e.delete_credential());
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    assert!(
        result.is_err(),
        "a refresh with no obtainable replacement credential must fail, not \
         silently succeed"
    );
    let err = result.unwrap_err();
    let jr_err = err
        .downcast_ref::<JrError>()
        .expect("must be a JrError so it maps to a real (non-panic) exit code");
    assert!(
        matches!(jr_err, JrError::UserError(_)),
        "must be a UserError (exit 64), got: {jr_err:?}"
    );
    // I-6 (adversary pass-2 fix L-1): the OLD "clear-then-relogin" framing
    // told the user credentials were gone. The corrected relogin-then-
    // replace contract must never say this, because it must not be true.
    let msg = jr_err.to_string();
    assert!(
        !msg.to_lowercase().contains("cleared"),
        "relogin-then-replace must never claim credentials were cleared — \
         they must never have been touched on a failed relogin. Got: {msg}"
    );

    // THE load-bearing assertion (AC-006/AC-007): the pre-refresh pair is
    // completely intact — proving relogin-then-replace's ordering, not just
    // documenting it.
    let (email, token) = after.expect(
        "the namespaced <profile>:email/<profile>:api-token pair must still be \
         readable after a FAILED refresh — relogin-then-replace must never \
         have deleted it. A missing/unreadable pair here is exactly the \
         I-6 data-loss regression this test exists to catch.",
    );
    assert_eq!(
        email, "old@example.com",
        "a failed refresh must leave the pre-refresh email completely unchanged"
    );
    assert_eq!(
        token, "old-token",
        "a failed refresh must leave the pre-refresh api-token completely unchanged"
    );
}

// ---------------------------------------------------------------------------
// Tier 4 (gated, #[ignore] + JR_RUN_KEYRING_TESTS=1): S-cycle4-cloud-id-
// correctness — OPTIONAL keyring-gated confirmation tail for AC-007/008
// (BC-1.2.053 mechanism-switch refresh-not-clear, VP-AUTHDX-020).
//
// VP-AUTHDX-020's PRIMARY oracle is the config-layer test inline in
// `src/cli/auth/login.rs::cloud_id_fallback_chain_tests` (wiremock +
// in-memory `Config`, no real keychain — `resolve_and_apply_cloud_id` is
// module-private and unreachable from this file). This tail exists only to
// confirm the full, real-keychain `login_token` path (the function `jr auth
// login`'s mechanism-switch dispatch actually calls, per BC-1.2.053
// Invariant 1 — no separate switch-detection code) behaves identically end
// to end, per the story's own File Structure Requirements row for this
// file ("Extend with the config-layer mechanism-switch scenario (AC-007/008)
// and an optional keyring-gated confirmation tail").
//
// Only the PRESERVE-on-failure branch is exercised here (a non-https
// profile url deterministically triggers fetch_cloud_id's own https-only
// precondition skip — EC-1.2.052-2 — with zero network dependency). The
// OVERWRITE-on-success branch needs a real HTTPS-reachable tenant_info
// endpoint (or the `JR_TENANT_INFO_URL` seam documented in
// `tests/cloud_id_tenant_info.rs`'s header, not yet implemented) and is
// therefore NOT duplicated here — see that file's header comment for the
// full explanation of why `wiremock` cannot produce a genuine success
// response under `fetch_cloud_id`'s `https://` precondition.
// ---------------------------------------------------------------------------

/// AC-007 (BC-1.2.053 Postcondition 2, VP-AUTHDX-020), real-keychain tail:
/// a profile holding a stale OAuth-era `cloud_id` that undergoes a
/// mechanism switch (simulated here by calling `login_token` directly —
/// the exact function `handle_login`'s switch dispatch calls, per
/// Invariant 1) keeps that stale value intact when the `tenant_info` fetch
/// fails, AND the real keychain pair is still written (the login itself
/// succeeds — the fetch failure is ancillary, never blocking).
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ac_007_mechanism_switch_preserves_stale_cloud_id_e2e_real_keychain() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;

    let svc = unique_service("cloud-id-switch-preserve");
    let tmp = TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");

    unsafe {
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
    }

    // A profile that just switched FROM oauth: auth_method already flipped
    // to api_token by the point login_token runs (handle_login updates
    // auth_method as part of the very call under test — mirrored here by
    // seeding it directly, since this test drives login_token in
    // isolation), still holding the stale OAuth-era cloud_id, and a
    // non-https url so the tenant_info fetch deterministically fails.
    write_config(
        &cfg_dir,
        "default_profile = \"sandbox\"\n\n\
         [profiles.sandbox]\n\
         url = \"http://not-https.example.net\"\n\
         auth_method = \"oauth\"\n\
         cloud_id = \"stale-oauth-era-uuid\"\n",
    );

    let login_result = jr::cli::auth::login_token(
        "sandbox",
        Some("switch@example.com".to_string()),
        Some("switch-token".to_string()),
        None,
        true,
        OutputFormat::Table,
    )
    .await;

    let saved = std::fs::read_to_string(cfg_dir.join("config.toml")).unwrap();
    let namespaced = auth::load_api_token(&Profile::from("sandbox"));

    let _ = keyring::Entry::new(&svc, "sandbox:email").and_then(|e| e.delete_credential());
    let _ = keyring::Entry::new(&svc, "sandbox:api-token").and_then(|e| e.delete_credential());
    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
    }

    login_result.expect(
        "AC-007: a tenant_info fetch failure during a mechanism switch must \
         never abort login_token",
    );
    assert!(
        saved.contains("stale-oauth-era-uuid"),
        "AC-007/BC-1.2.053 Postcondition 2: the stale cloud_id must be \
         PRESERVED (never bare-cleared) when the fetch fails. Saved:\n{saved}"
    );
    assert!(
        namespaced.is_ok(),
        "the real keychain pair must still be written despite the \
         cloud_id fetch failure"
    );
}
