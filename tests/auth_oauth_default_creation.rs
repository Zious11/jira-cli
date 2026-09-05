//! S-cycle3-oauth-default-creation — Two-Step Red Gate TESTS (step 2 of 2,
//! BC-5.38.001) for the OAuth-default-at-creation picker, the airtight
//! non-interactive OAuth guard, and the `--oauth`/`--api-token` flag
//! symmetry (BC-1.1.013/014/015/016, BC-1.2.049/050, DEC-313/323).
//!
//! Stubs (`todo!()`) were committed at `5490780b`:
//! - `cli::auth::check_noninteractive_oauth_guard`      (BC-1.1.016)
//! - `cli::auth::emit_oauth_deprecation_notice`          (BC-1.2.049)
//! - `cli::auth::emit_api_token_inert_on_refresh_notice` (BC-1.2.050)
//! - `cli::auth::prompt_auth_method_picker`              (BC-1.1.013)
//! - `cli::auth::clear_outgoing_mechanism_on_switch`     (BC-1.1.013 EC-1.1.013-2 /
//!   BC-1.1.014 EC-1.1.014-4)
//!
//! At that point none of the five were wired into `handle_login`'s or
//! `refresh_credentials`'s control flow yet (implementer scope) — so most
//! of the assertions below exercised either (a) the stub function directly
//! (panicking via `todo!()` -> RED), or (b) the observable end-to-end
//! behavior of `handle_login`/`refresh_credentials`, which at the time
//! still took the PRE-story code path and therefore produced a *different*
//! outcome than the BC required (RED for a content-mismatch reason, not a
//! panic).
//!
//! **Status (FIX-F5-CYCLE4-2 doc sweep):** all five are implemented and
//! wired in per BC-1.1.013/014/015/016 and BC-1.2.049/050, and this file
//! has since accreted `clear_outgoing_mechanism_on_switch`'s FIX-F5-login-
//! switch relogin-then-replace coverage plus review fixes from the
//! cycle-004 F5 scoped adversarial review. Every test in this file is
//! green today. The RED narrative above is retained as historical record
//! of how these tests were authored, not current behavior.
//!
//! # Safety note — why nothing here can reach a real browser / network call
//!
//! Several tests below drive `handle_login`/`refresh_credentials` end-to-end
//! with `--oauth`/`auth_method: "oauth"` under `no_input: true`, in the
//! CURRENT (unwired) codebase, where the BC-1.1.016 guard does not yet
//! short-circuit first. This is deliberately safe:
//!
//! - `JR_SERVICE_NAME` is scoped per-test to a freshly generated, never-seen
//!   service name (`unique_service`), so `resolve_oauth_app_credentials`'s
//!   keychain probe (`try_load_oauth_app_credentials`) always resolves to
//!   `None` (a fresh `NoEntry`, not a real secret read).
//! - No `--client-id`/`--client-secret` flags or `JR_OAUTH_CLIENT_ID`/
//!   `JR_OAUTH_CLIENT_SECRET` env vars are ever set, so the flag/env layers
//!   never resolve either.
//! - `embedded_oauth_app_present()` is checked at the top of every such test;
//!   if this build happens to have embedded OAuth credentials baked in (only
//!   true for an official release build with `JR_BUILD_OAUTH_CLIENT_ID`/
//!   `_SECRET` set at compile time — never true for a normal `cargo test`),
//!   the test skips itself with a loud `eprintln!` rather than risk reaching
//!   `oauth_login()` (which binds a real listener and opens a real browser).
//! - With no source resolving, `resolve_oauth_app_credentials` returns
//!   `Err(JrError::UserError("OAuth app credentials are required. ..."))`
//!   under `no_input: true` — `src/cli/auth/keychain.rs::resolve_oauth_app_credentials_for_test`,
//!   the `if no_input { return Err(...) }` branch — BEFORE `login_oauth` ever
//!   calls `crate::api::auth::oauth_login` (the function that binds the
//!   listener / opens the browser).
//!
//! This is also the mechanism used to PROVE the BC-1.1.016 guard's ordering
//! invariant without a mock: today (unwired), a non-interactive `--oauth`
//! attempt fails with the ABOVE "OAuth app credentials are required..."
//! message (reached only after the credential-resolution/keychain-probe
//! code has already run). Once the guard is correctly wired as the FIRST
//! statement, the SAME invocation must instead fail with the verbatim
//! `NONINTERACTIVE_OAUTH_GUARD_MESSAGE`, with the keychain probe never
//! reached. Asserting the exact message text (not merely "it errored") is
//! therefore a real ordering proof: a guard inserted even one statement too
//! late would still let `resolve_oauth_app_credentials` win the race and
//! produce the OTHER message.
//!
//! # Two tiers, matching this story's own convention (S-cycle3-remove-logout-semantics)
//! - **Non-gated**: pure function calls / clap-level parsing / self-relaunch
//!   subprocess captures of stderr-only notice text. Zero keychain I/O.
//! - **Gated** (`#[ignore]` + `JR_RUN_KEYRING_TESTS=1`): seed and/or verify
//!   real keychain credential content via `handle_login`/`refresh_credentials`/
//!   `clear_outgoing_mechanism_on_switch` directly, in-process (mirrors
//!   `tests/auth_remove_logout_semantics.rs`).

#[allow(dead_code)]
mod common;

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use assert_cmd::Command as AssertCommand;
use jr::api::auth;
use jr::cli::OutputFormat;
use jr::cli::auth::{
    LoginArgs, NONINTERACTIVE_OAUTH_GUARD_MESSAGE, RefreshArgs, check_noninteractive_oauth_guard,
    clear_outgoing_mechanism_on_switch, emit_api_token_inert_on_refresh_notice,
    emit_oauth_deprecation_notice, handle_login, prompt_auth_method_picker, refresh_credentials,
};
use jr::error::JrError;
use jr::profile::Profile;
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Shared helpers (kept local, not in tests/common — mirrors
// tests/auth_remove_logout_semantics.rs's stated rationale: each caller
// wraps its own `unsafe` justification).
// ---------------------------------------------------------------------------

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

/// Guard used by every test that drives `handle_login`/`refresh_credentials`
/// through the `--oauth`/`auth_method: "oauth"` + `no_input: true` path in
/// the CURRENT (pre-guard-wiring) codebase. See the module-level safety note
/// for why this is otherwise safe; this is the one residual check that
/// depends on the *build*, not the test's own env isolation.
fn skip_if_embedded_oauth_present() -> bool {
    if jr::api::auth_embedded::embedded_oauth_app_present() {
        eprintln!(
            "[SKIP] this build has embedded OAuth app credentials baked in \
             (JR_BUILD_OAUTH_CLIENT_ID/_SECRET were set at compile time) — \
             skipping to avoid reaching real oauth_login() (listener bind + \
             browser open) in the current, pre-guard-wiring codebase."
        );
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Tier 1a: BC-1.1.016 airtight non-interactive OAuth guard — pure function,
// zero I/O, always safe to run.
// ---------------------------------------------------------------------------

#[test]
fn test_bc_1_1_016_guard_message_constant_is_exact() {
    assert_eq!(
        NONINTERACTIVE_OAUTH_GUARD_MESSAGE,
        "OAuth requires an interactive terminal; use --api-token for non-interactive auth.",
        "BC-1.1.016 Postcondition 2 pins this exact, non-interpolated stderr string"
    );
}

#[test]
fn test_bc_1_1_016_guard_rejects_noninteractive_explicit_oauth() {
    // Precondition 2a: no_input && oauth_selected -> Err, exit 64, exact message.
    let err = check_noninteractive_oauth_guard(true, true)
        .expect_err("non-interactive + oauth_selected must be rejected");
    let jr_err = err
        .downcast_ref::<JrError>()
        .expect("guard must return a JrError, not an opaque anyhow error");
    assert!(
        matches!(jr_err, JrError::UserError(_)),
        "guard rejection must be JrError::UserError so it maps to exit 64, got: {jr_err:?}"
    );
    assert_eq!(
        jr_err.to_string(),
        NONINTERACTIVE_OAUTH_GUARD_MESSAGE,
        "guard error text must be byte-for-byte the pinned constant, no interpolation"
    );
    assert_eq!(jr_err.exit_code(), 64);
}

#[test]
fn test_bc_1_1_016_guard_allows_noninteractive_non_oauth() {
    // no_input && !oauth_selected -> Ok: the ordinary token-first
    // non-interactive default (BC-1.1.014) is never blocked by this guard.
    check_noninteractive_oauth_guard(true, false)
        .expect("non-interactive api_token selection must not be blocked");
}

#[test]
fn test_bc_1_1_016_guard_allows_interactive_oauth() {
    // Interactive sessions may always choose OAuth, explicitly or via the
    // picker — the guard only fires under non-interactive triggers.
    check_noninteractive_oauth_guard(false, true).expect("interactive oauth must not be blocked");
}

#[test]
fn test_bc_1_1_016_guard_allows_interactive_non_oauth() {
    check_noninteractive_oauth_guard(false, false)
        .expect("interactive api_token must not be blocked");
}

// ---------------------------------------------------------------------------
// Tier 1b: BC-1.2.050 clap `conflicts_with` mutual exclusion — clap-level
// only, zero I/O. (Landed in the same stub commit as the flag itself —
// expected GREEN already; kept here as the AC-015 regression pin so a
// future accidental removal of `conflicts_with` is caught by THIS story's
// suite, not only by whichever suite happens to notice.)
// ---------------------------------------------------------------------------

#[test]
fn test_ac_015_login_oauth_and_api_token_together_exits_2() {
    AssertCommand::cargo_bin("jr")
        .unwrap()
        .args(["auth", "login", "--oauth", "--api-token"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_ac_015_refresh_oauth_and_api_token_together_exits_2() {
    AssertCommand::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--oauth", "--api-token"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_bc_1_2_050_login_help_lists_api_token_flag() {
    let output = AssertCommand::cargo_bin("jr")
        .unwrap()
        .args(["auth", "login", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("--api-token"),
        "auth login --help should list --api-token: {stdout}"
    );
}

#[test]
fn test_bc_1_2_050_refresh_help_lists_api_token_flag() {
    let output = AssertCommand::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("--api-token"),
        "auth refresh --help should list --api-token: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Tier 1c: BC-1.1.015 regression pin (VP-AUTHDX-002) — pure, in-memory,
// no I/O. Documents the `.unwrap_or("api_token")` invariant over arbitrary
// ProfileConfig field combinations with `auth_method: None` fixed. A
// keychain-gated end-to-end companion (proving the actual `from_config`
// code path, not just the `Option` semantics) lives in the gated tier below.
// ---------------------------------------------------------------------------

proptest::proptest! {
    #[test]
    fn test_vp_authdx_002_profile_config_auth_method_none_falls_back_to_api_token(
        cloud_id in proptest::option::of("[a-zA-Z0-9-]{0,12}"),
        org_id in proptest::option::of("[a-zA-Z0-9-]{0,12}"),
        oauth_scopes in proptest::option::of("[a-zA-Z0-9: _-]{0,40}"),
        team_field_id in proptest::option::of("customfield_[0-9]{5}"),
        story_points_field_id in proptest::option::of("customfield_[0-9]{5}"),
        project in proptest::option::of("[A-Z]{2,6}"),
        env in proptest::option::of("[a-z]{0,10}"),
    ) {
        let profile = jr::config::ProfileConfig {
            url: Some("https://example.atlassian.net".to_string()),
            auth_method: None, // BC-1.1.015: fixed absent for every case
            cloud_id,
            org_id,
            oauth_scopes,
            team_field_id,
            story_points_field_id,
            project,
            env,
        };
        // This is the exact fallback expression BC-1.1.015 pins in
        // `src/api/client.rs::JiraClient::from_config` — CLAUDE.md's Known
        // Size Deviations / Gotchas entries flag this line as MUST-NOT-CHANGE.
        let resolved = profile.auth_method.as_deref().unwrap_or("api_token");
        proptest::prop_assert_eq!(resolved, "api_token");
    }
}

// ---------------------------------------------------------------------------
// Tier 1d (MED-1, pre-PR review, CWE-400): `prompt_auth_method_picker`'s
// independent, `no_input`-blind terminal check. Pure, zero I/O, always safe
// to run — proves the interactive picker is unreachable-by-construction
// when stdin is not a real TTY, regardless of `no_input`.
// ---------------------------------------------------------------------------

/// MED-1: `cargo test`'s own stdin is not a real interactive terminal (it is
/// either piped or, in this sandboxed/CI environment, fully detached), and
/// this test does not set `JR_STDIN_IS_TTY=1` — so `prompt_auth_method_picker`
/// must take its non-TTY early-return path and return the token-first
/// default (`Ok(false)`) WITHOUT ever calling `dialoguer::Select::interact()`.
/// Before the fix, this call would hang (or error unpredictably against a
/// closed/empty stdin) instead of returning promptly — this test would never
/// complete under the pre-fix code, which is itself the regression this test
/// guards against.
#[test]
fn test_med1_picker_defaults_to_token_first_when_stdin_not_a_tty() {
    // Defense-in-depth: make sure a leaked JR_STDIN_IS_TTY=1 from another
    // (improperly isolated) test can't force this one green for the wrong
    // reason. Safe to touch unconditionally — this test does not run
    // concurrently with anything that depends on this var (nothing else in
    // this file calls `prompt_auth_method_picker` directly), and it only
    // ever removes, never sets, a value.
    unsafe {
        std::env::remove_var("JR_STDIN_IS_TTY");
    }

    let result = prompt_auth_method_picker();
    assert!(
        !result.expect("non-TTY path must return Ok, never propagate a prompt error"),
        "MED-1 (CWE-400): prompt_auth_method_picker must default to the \
         API-token selection (false) when stdin is not a real TTY, \
         independent of any caller's no_input value — never call \
         Select::interact() without a real interactive terminal"
    );
}

// ---------------------------------------------------------------------------
// Tier 1e (MED-2, pre-PR review): VP-AUTHDX-001 — BC-1.1.014 tier-2
// non-interactive default (`oauth: false, api_token: false, no_input:
// true`). Proves this resolves to the API-TOKEN path (`login_token`, NOT
// `login_oauth`) across arbitrary credential-completeness, and never reaches
// OAuth app-credential resolution, an OAuth callback-listener bind, or a
// browser-open attempt.
//
// Deliberately NOT gated behind JR_RUN_KEYRING_TESTS: every generated case
// leaves at least one of email/token absent, so `login_token`'s
// `resolve_credential` call fails BEFORE `handle_login` ever reaches
// `auth::store_api_token` — the property is proven from the SHAPE of the
// resulting error, not from a persisted credential. Each case also targets a
// brand-new profile name that has never been declared before, so
// `clear_outgoing_mechanism_on_switch` takes its `current_auth_method ==
// None` early return and never calls `auth::clear_profile_creds` either.
// `JR_CONFIG_DIR`/`JR_CACHE_DIR` are still scoped to a fresh per-case temp
// dir (filesystem-only isolation — no OS keyring is ever touched by any
// path this test exercises).
//
// The story's own `{--no-input, non-TTY}` wording collapses to a single
// `no_input: bool` by the time `handle_login` is invoked directly — both
// `--no-input` and non-TTY auto-detection are resolved to this same flag by
// `src/main.rs` before dispatch (see `main.rs`'s auto-`--no-input` flip) —
// so `no_input: true` here exercises both triggers identically; the
// credential-completeness axis is what this proptest actually varies.
proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(20))]

    #[test]
    fn test_vp_authdx_001_noninteractive_default_reaches_token_path_not_oauth(
        variant in 0u8..3,
        cred_value in "[a-zA-Z0-9._@-]{1,20}",
    ) {
        // `prop_assert!`/`prop_assert_eq!` early-return `Err(TestCaseError)`
        // from their IMMEDIATELY enclosing function — they must run at this
        // outer (proptest-macro-wrapped) scope, not inside the `async` block
        // below, or the block's inferred return type stops matching `rt
        // .block_on`'s call site. So the async block only computes and
        // returns the plain error-message `String`; every `prop_assert*!`
        // call happens after it, out here.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let msg = rt.block_on(async {
            let _guard = env_lock().lock().await;
            let tmp = tempfile::TempDir::new().unwrap();
            let cfg_dir = tmp.path().join("jr");
            let cache_dir = tmp.path().join("cache");
            unsafe {
                scrub_jr_env();
                std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
                std::env::set_var("JR_CACHE_DIR", &cache_dir);
            }

            // Fresh, never-before-declared profile name per case — keeps
            // `current_auth_method` at `None` so the outgoing-credential
            // clear path (the one real keychain-touching branch inside
            // `handle_login` outside of `login_token`/`login_oauth`
            // themselves) is provably not taken.
            let profile_name = unique_service("vpauthdx001");

            // At least one of email/token is always absent, so
            // `resolve_credential` fails before `auth::store_api_token` is
            // ever called — this is what keeps the whole case keychain-free.
            let (email, token) = match variant {
                0 => (None, None),
                1 => (Some(cred_value.clone()), None),
                _ => (None, Some(cred_value.clone())),
            };

            let result = handle_login(LoginArgs {
                profile: Some(profile_name.clone()),
                url: Some("https://vp-authdx-001.example.atlassian.net".to_string()),
                oauth: false,
                api_token: false,
                email,
                token,
                client_id: None,
                client_secret: None,
                cloud_id: None,
                no_input: true,
                output: OutputFormat::Table,
            })
            .await;

            unsafe {
                std::env::remove_var("JR_CONFIG_DIR");
                std::env::remove_var("JR_CACHE_DIR");
            }

            match result {
                Ok(()) => panic!(
                    "VP-AUTHDX-001: with credentials incomplete under no_input, \
                     handle_login must fail via resolve_credential, not succeed"
                ),
                Err(e) => e.to_string(),
            }
        });

        proptest::prop_assert_ne!(
            &msg,
            NONINTERACTIVE_OAUTH_GUARD_MESSAGE,
            "VP-AUTHDX-001: the OAuth guard must never fire here — oauth was \
             never selected on this call, so a guard firing would mean the \
             bare non-interactive default wrongly resolved to OAuth: {}",
            msg
        );
        proptest::prop_assert!(
            !msg.contains("OAuth"),
            "VP-AUTHDX-001: the error must come from the api_token credential \
             resolver, never from OAuth app-credential resolution — this proves \
             login_oauth was never reached, so no callback-listener bind or \
             browser-open was ever attempted: {}",
            msg
        );
        proptest::prop_assert!(
            msg.contains("is required. Provide --email or set $JR_EMAIL")
                || msg.contains("is required. Provide --token or set $JR_API_TOKEN"),
            "VP-AUTHDX-001: the error must be exactly resolve_credential's \
             email/token message, proving login_token (not login_oauth) was \
             the path reached: {}",
            msg
        );
    }
}

// ---------------------------------------------------------------------------
// Tier 2: BC-1.2.049 / BC-1.2.050 notice functions — self-relaunch the test
// binary to capture real stderr from a `#[ignore]`d child test that calls
// only the notice function under test. Zero keychain/network I/O (these
// functions are pure stderr side effects), so this tier is NOT gated behind
// JR_RUN_KEYRING_TESTS.
// ---------------------------------------------------------------------------

/// Re-invoke THIS SAME test binary (`std::env::current_exe()`), running
/// exactly one `#[ignore]`d test by exact name, and return its captured
/// output. Each integration test file compiles to its own binary, so this
/// reliably targets a helper test defined further down in this same file.
fn relaunch_self(exact_test_name: &str) -> std::process::Output {
    let exe = std::env::current_exe().expect("current_exe must resolve for a test binary");
    // --nocapture is load-bearing: libtest's DEFAULT capture mode swallows
    // both println!/eprintln! output AND the panic hook's message into an
    // internal buffer, discarding it entirely when the test passes and only
    // ever surfacing it (bundled into the harness's own SUMMARY, printed to
    // the child's real stdout, never stderr) when the test fails. Without
    // --nocapture, `output.stderr` would come back empty in BOTH the RED
    // (panicking) state and, worse, the GREEN (correctly-implemented,
    // passing) state — defeating this whole mechanism. --nocapture routes
    // real eprintln!/panic output straight to the child's real fds.
    std::process::Command::new(exe)
        .args(["--exact", exact_test_name, "--ignored", "--nocapture"])
        .output()
        .expect("failed to relaunch test binary")
}

#[test]
fn test_bc_1_2_049_deprecation_notice_prints_to_stderr_in_table_mode() {
    let output = relaunch_self("helper_emit_oauth_flag_notice_table");
    assert!(
        output.status.success(),
        "helper child must not panic (todo!() -> RED today); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.trim().is_empty(),
        "BC-1.2.049 Postcondition 2: a deprecation notice must be printed to \
         stderr in human (Table) output mode; got empty stderr"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("deprecat"),
        "the notice must go to stderr, never stdout: stdout was: {stdout}"
    );
}

#[test]
// Deliberately NOT named with the substring "deprecat" — this function's
// name is echoed to the child process's own stdout by libtest's "test
// <name> ... ok" summary line (unavoidable even under --nocapture), and the
// parent test asserts stdout never contains "deprecat" (the notice itself
// must be stderr-only). A helper name containing that substring would fail
// the parent's assertion regardless of emit_oauth_deprecation_notice's
// actual (correct) stderr-only behavior — a false negative from the test
// harness's own output, not a production bug.
#[ignore = "invoked only via relaunch_self from test_bc_1_2_049_deprecation_notice_prints_to_stderr_in_table_mode"]
fn helper_emit_oauth_flag_notice_table() {
    emit_oauth_deprecation_notice(OutputFormat::Table);
}

#[test]
fn test_ec_1_2_049_1_deprecation_notice_silent_under_json_mode() {
    let output = relaunch_self("helper_emit_oauth_deprecation_notice_json");
    assert!(
        output.status.success(),
        "helper child must not panic (todo!() -> RED today); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim().is_empty(),
        "EC-1.2.049-1: no deprecation notice on stderr under --output json, got: {stderr}"
    );
}

#[test]
#[ignore = "invoked only via relaunch_self from test_ec_1_2_049_1_deprecation_notice_silent_under_json_mode"]
fn helper_emit_oauth_deprecation_notice_json() {
    emit_oauth_deprecation_notice(OutputFormat::Json);
}

#[test]
fn test_bc_1_2_050_inert_notice_prints_to_stderr_in_table_mode() {
    let output = relaunch_self("helper_emit_api_token_inert_notice_table");
    assert!(
        output.status.success(),
        "helper child must not panic (todo!() -> RED today); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.trim().is_empty(),
        "BC-1.2.050 Postcondition 3 (O-2/CV-2): the inert-on-refresh notice \
         must be printed to stderr in human (Table) output mode; got empty stderr"
    );
}

#[test]
#[ignore = "invoked only via relaunch_self from test_bc_1_2_050_inert_notice_prints_to_stderr_in_table_mode"]
fn helper_emit_api_token_inert_notice_table() {
    emit_api_token_inert_on_refresh_notice(OutputFormat::Table);
}

#[test]
fn test_bc_1_2_050_inert_notice_silent_under_json_mode() {
    let output = relaunch_self("helper_emit_api_token_inert_notice_json");
    assert!(
        output.status.success(),
        "helper child must not panic (todo!() -> RED today); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim().is_empty(),
        "mirrors BC-1.2.049's EC-1.2.049-1 output-channel rule (gated on \
         OUTPUT FORMAT, not TTY-ness) for the inert notice: got: {stderr}"
    );
}

#[test]
#[ignore = "invoked only via relaunch_self from test_bc_1_2_050_inert_notice_silent_under_json_mode"]
fn helper_emit_api_token_inert_notice_json() {
    emit_api_token_inert_on_refresh_notice(OutputFormat::Json);
}

// ---------------------------------------------------------------------------
// Tier 3 (gated, #[ignore] + JR_RUN_KEYRING_TESTS=1): real keychain
// round-trip verification via direct in-process calls to
// `clear_outgoing_mechanism_on_switch`, `handle_login`, `refresh_credentials`
// — mirrors tests/auth_remove_logout_semantics.rs's convention.
// ---------------------------------------------------------------------------

/// BC-1.1.013 EC-1.1.013-2 / BC-1.1.014 EC-1.1.014-4 (M-1), direct function
/// contract, AMENDED by FIX-F5-login-switch: given an outgoing mechanism
/// that differs from the incoming one, `clear_outgoing_mechanism_on_switch`
/// MUST clear ONLY the outgoing mechanism's credential pair — NOT both
/// kinds. This is a corrected expectation from the function's pre-fix
/// behavior (which called the combined `clear_profile_creds` and cleared
/// both kinds unconditionally): under `handle_login`'s new
/// relogin-then-replace ordering, this function now runs AFTER the new
/// mechanism's credentials are already stored, so clearing both kinds would
/// delete the just-stored new credentials too. This test seeds BOTH kinds
/// (the API-token pair standing in for a freshly-stored `new_method` pair)
/// and asserts only the outgoing OAuth pair is gone afterward — the
/// API-token pair must survive untouched.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ec_1_1_013_2_clear_outgoing_mechanism_on_switch_clears_only_outgoing_kind() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("switch-clear-only-outgoing");
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

    auth::store_oauth_tokens(&Profile::from("staging"), "sw-access", "sw-refresh").unwrap();
    // Stands in for the NEW mechanism's credentials, already stored by the
    // time this function runs under the corrected ordering.
    auth::store_api_token(&Profile::from("staging"), "sw@example.com", "sw-token").unwrap();

    let result = clear_outgoing_mechanism_on_switch(
        &Profile::from("staging"),
        Some("oauth"),
        "api_token",
        false,
    );

    let oauth_after = auth::load_oauth_tokens(&Profile::from("staging"));
    let api_token_after = auth::load_api_token(&Profile::from("staging"));

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    result.expect(
        "clear_outgoing_mechanism_on_switch must succeed when the outgoing \
         credential kind is cleanly deletable",
    );
    assert!(
        oauth_after.is_err(),
        "outgoing OAuth pair must be gone after the mechanism-switch clear"
    );
    let (email_after, token_after) =
        api_token_after.expect("the NEW mechanism's api-token pair must survive untouched");
    assert_eq!(email_after, "sw@example.com");
    assert_eq!(token_after, "sw-token");
}

#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ec_1_1_013_2_clear_outgoing_mechanism_on_switch_with_notice_still_succeeds() {
    // BC-1.1.014 EC-1.1.014-4: the SHOULD-level stderr notice must not turn
    // a successful clear into an error. Per the story's own Architecture
    // Compliance Rule ("Non-interactive mechanism-switch stderr notice is a
    // SHOULD, not a MUST... do not over-engineer it as a hard requirement
    // with its own error path"), this test intentionally does not assert on
    // the notice's exact wording — only that requesting it does not change
    // the function's success/credential-clearing contract.
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("switch-clear-with-notice");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

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
         auth_method = \"oauth\"\n",
    );
    auth::store_oauth_tokens(&Profile::from("default"), "n-access", "n-refresh").unwrap();

    let result = clear_outgoing_mechanism_on_switch(
        &Profile::from("default"),
        Some("oauth"),
        "api_token",
        true,
    );
    let oauth_after = auth::load_oauth_tokens(&Profile::from("default"));

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    result.expect("emit_switch_notice=true must not change the success contract");
    assert!(oauth_after.is_err(), "OAuth pair must still be cleared");
}

/// AC-007 (BC-1.1.014 EC-1.1.014-4): a NON-interactive `handle_login`
/// re-declaration that switches mechanism (existing `oauth` profile,
/// login now selects `api_token`) must clear the outgoing OAuth pair.
///
/// RED today: the mechanism-switch clear is unwired, so `login_token`'s
/// plain overwrite runs and the stale OAuth pair survives.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ac_007_handle_login_noninteractive_mechanism_switch_clears_outgoing_oauth_creds() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("login-switch-clears-oauth");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"staging\"\n\n\
         [profiles.staging]\n\
         url = \"https://staging.example\"\n\
         auth_method = \"oauth\"\n",
    );
    auth::store_oauth_tokens(&Profile::from("staging"), "login-access", "login-refresh").unwrap();

    let result = handle_login(LoginArgs {
        profile: Some("staging".to_string()),
        url: None,
        oauth: false,
        api_token: true,
        email: Some("switch@example.com".to_string()),
        token: Some("switch-token".to_string()),
        client_id: None,
        client_secret: None,
        cloud_id: None,
        no_input: true,
        output: OutputFormat::Table,
    })
    .await;

    let oauth_after = auth::load_oauth_tokens(&Profile::from("staging"));

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    result.expect("non-interactive mechanism-switch login must still succeed");
    assert!(
        oauth_after.is_err(),
        "AC-007: the outgoing OAuth pair must be cleared as part of the same \
         non-interactive `auth login` invocation that switches to api_token"
    );
}

// ---------------------------------------------------------------------------
// FIX-F5-login-switch (Wave-5 adversary finding, MEDIUM data loss): a
// mechanism-switching `auth login` must never delete the outgoing
// mechanism's working credentials before the new mechanism's credentials
// are confirmed obtained and stored (relogin-then-replace ordering, mirrors
// I-6 / BC-1.2.051's fix to `refresh_credentials`). Before this fix,
// `handle_login` called `clear_outgoing_mechanism_on_switch` BEFORE
// `login_oauth`/`login_token`, so a failed switch (browser cancel, network
// error, a missing `--no-input` value) left the profile credential-less —
// worse than its pre-command state.
// ---------------------------------------------------------------------------

/// FIX-F5-login-switch, failing half: a mechanism switch (`api_token` ->
/// `oauth`) whose new-mechanism login FAILS must leave the profile's prior
/// working `api_token` credentials completely intact.
///
/// The OAuth failure is forced deterministically, with zero network/browser
/// I/O: passing `--client-id` without `--client-secret` makes
/// `resolve_oauth_app_credentials_for_test`'s pair-gate hard-error
/// (`JrError::UserError("--client-id was provided without --client-secret...")`)
/// before `login_oauth` ever touches the keychain, `oauth_login`'s listener
/// bind, or a real browser — the same "fails before any I/O beyond the
/// flag/env layer" property the module-level safety note already relies on
/// for the guard-ordering tests above, applied here to force a login
/// failure instead of a guard rejection. `no_input: false` is required so
/// this reaches `login_oauth` at all — the airtight BC-1.1.016 guard
/// (Precondition 2a) would otherwise reject `--oauth` under `no_input: true`
/// before ANY clear/login code runs, which cannot exercise this ordering
/// bug in the first place.
///
/// RED pre-fix: the old code cleared `<profile>:email`/`<profile>:api-token`
/// via `clear_profile_creds` BEFORE calling `login_oauth`, so by the time
/// `login_oauth` fails, the original credentials are already gone — this
/// assertion fails against the pre-fix ordering.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_fix_f5_failed_login_switch_preserves_outgoing_api_token_creds() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("login-switch-fail-preserves");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"prod\"\n\n\
         [profiles.prod]\n\
         url = \"https://prod.example\"\n\
         auth_method = \"api_token\"\n",
    );
    auth::store_api_token(&Profile::from("prod"), "orig@example.com", "orig-token").unwrap();

    let result = handle_login(LoginArgs {
        profile: Some("prod".to_string()),
        url: None,
        oauth: true,
        api_token: false,
        email: None,
        token: None,
        // Partial OAuth-app-credential pair -> resolve_oauth_app_credentials
        // hard-errors deterministically, with zero keychain/network/browser
        // I/O, before login_oauth ever reaches a store_* call.
        client_id: Some("only-id".to_string()),
        client_secret: None,
        cloud_id: None,
        no_input: false,
        output: OutputFormat::Table,
    })
    .await;

    let api_token_after = auth::load_api_token(&Profile::from("prod"));

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    let err = result.expect_err(
        "a --client-id-without-secret OAuth switch attempt must fail \
         deterministically, not succeed",
    );
    let jr_err = err
        .downcast_ref::<JrError>()
        .expect("must be a JrError so it maps to a real exit code");
    assert!(
        jr_err.to_string().contains("--client-secret"),
        "must fail via the OAuth app credential pair-gate, not some other \
         path — got: {jr_err}"
    );

    let (email_after, token_after) = api_token_after.expect(
        "FIX-F5-login-switch: a FAILED mechanism-switch login must leave the \
         profile's prior working api_token credentials completely intact — \
         they must never be cleared before the new mechanism's login has \
         succeeded (relogin-then-replace)",
    );
    assert_eq!(
        email_after, "orig@example.com",
        "original email must survive a failed switch unchanged"
    );
    assert_eq!(
        token_after, "orig-token",
        "original token must survive a failed switch unchanged"
    );
}

/// FIX-F5-login-switch, success half: a mechanism switch (`oauth` ->
/// `api_token`) whose new-mechanism login SUCCEEDS must (a) store the new
/// `api_token` credentials, AND (b) leave no orphaned outgoing `oauth`
/// credentials behind — the switch semantics from BC-1.1.013 EC-1.1.013-2 /
/// BC-1.1.014 EC-1.1.014-4 are preserved by the reorder, not weakened.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_fix_f5_successful_login_switch_stores_new_and_clears_outgoing_oauth_creds() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("login-switch-success-no-orphan");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"staging2\"\n\n\
         [profiles.staging2]\n\
         url = \"https://staging2.example\"\n\
         auth_method = \"oauth\"\n",
    );
    auth::store_oauth_tokens(&Profile::from("staging2"), "old-access", "old-refresh").unwrap();

    let result = handle_login(LoginArgs {
        profile: Some("staging2".to_string()),
        url: None,
        oauth: false,
        api_token: true,
        email: Some("new@example.com".to_string()),
        token: Some("new-token".to_string()),
        client_id: None,
        client_secret: None,
        cloud_id: None,
        no_input: true,
        output: OutputFormat::Table,
    })
    .await;

    let oauth_after = auth::load_oauth_tokens(&Profile::from("staging2"));
    let api_token_after = auth::load_api_token(&Profile::from("staging2"));

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    result.expect("successful non-interactive mechanism-switch login must succeed");

    let (email_after, token_after) =
        api_token_after.expect("new api_token credentials must be stored on success");
    assert_eq!(email_after, "new@example.com");
    assert_eq!(token_after, "new-token");

    assert!(
        oauth_after.is_err(),
        "FIX-F5-login-switch: a SUCCESSFUL switch must leave NO orphaned \
         outgoing oauth credentials behind"
    );
}

// ---------------------------------------------------------------------------
// FIX-F5-CYCLE4-1 LOW-1 (F5-scoped adversarial review, cycle-004,
// credential-orphan on legacy-`None`-mechanism switch): the symmetric other
// half of PR #771's NEW-1 fix. NEW-1 protected the PRE-login label (a legacy
// `auth_method: None` profile with stored credentials under some label must
// not be pre-marked ahead of an attempt). This story closes the POST-login
// gap NEW-1 left open: such a profile's `switching` flag is `false` (it's
// computed only from `current_auth_method.as_deref().is_some_and(...)`,
// which is always `false` when `current_auth_method` is `None`), and
// `clear_outgoing_mechanism_on_switch` itself early-returns whenever
// `current_auth_method` is `None` by design — so before this fix, a
// SUCCESSFUL login under a DIFFERENT mechanism on such a profile stored the
// new pair and set `auth_method`, but never cleared the still-present old
// credential, silently orphaning it in the keychain.
// ---------------------------------------------------------------------------

/// Reproduction: a profile migrated from the legacy `[instance]` config
/// shape (`auth_method` absent from the TOML — copies through as `None`)
/// that already holds a working OAuth pair. A non-interactive `jr auth
/// login --api-token` on it must SUCCEED (storing the new api-token pair)
/// AND clear the now-orphaned outgoing OAuth pair — not leave it behind.
///
/// RED pre-fix: `switching` is `false` (computed only against
/// `current_auth_method.as_deref()`, which is `None` here), and
/// `clear_outgoing_mechanism_on_switch` takes its own `current_auth_method
/// == None` early return regardless — so the pre-fix code stores the new
/// api-token pair but never touches the stale OAuth pair, which survives.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_fix_f5_cycle4_1_low1_none_labelled_profile_with_stored_oauth_clears_orphan_on_successful_switch()
 {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("cycle4-low1-orphan-clear");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    // Deliberately NO `auth_method` line — mirrors a profile migrated from
    // the legacy `[instance]` config shape, which copies `auth_method`
    // through verbatim and can leave it absent (`None`) even though the
    // profile still holds working credentials.
    write_config(
        &cfg_dir,
        "default_profile = \"legacy\"\n\n\
         [profiles.legacy]\n\
         url = \"https://legacy.example\"\n",
    );
    auth::store_oauth_tokens(&Profile::from("legacy"), "orphan-access", "orphan-refresh").unwrap();

    let result = handle_login(LoginArgs {
        profile: Some("legacy".to_string()),
        url: None,
        oauth: false,
        api_token: true,
        email: Some("cycle4@example.com".to_string()),
        token: Some("cycle4-token".to_string()),
        client_id: None,
        client_secret: None,
        cloud_id: None,
        no_input: true,
        output: OutputFormat::Table,
    })
    .await;

    let oauth_after = auth::load_oauth_tokens(&Profile::from("legacy"));
    let api_token_after = auth::load_api_token(&Profile::from("legacy"));

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    result.expect(
        "a non-interactive api-token login on a legacy None-labelled profile \
         with stored OAuth credentials must succeed",
    );

    let (email_after, token_after) =
        api_token_after.expect("the new api_token credentials must be stored on success");
    assert_eq!(email_after, "cycle4@example.com");
    assert_eq!(token_after, "cycle4-token");

    assert!(
        oauth_after.is_err(),
        "FIX-F5-CYCLE4-1 LOW-1: a SUCCESSFUL login on a legacy None-labelled \
         profile must clear the now-orphaned outgoing OAuth pair, not leave \
         it behind in the keychain"
    );
}

/// Guard: a genuinely brand-new profile (no `auth_method`, no stored
/// credentials under any label) must reach exactly the same successful
/// outcome as before this fix — no spurious clear attempt, no error, and
/// only the newly-stored api-token pair present afterward.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_fix_f5_cycle4_1_low1_brand_new_none_labelled_profile_clears_nothing() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("cycle4-low1-brand-new");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"brandnew\"\n\n\
         [profiles.brandnew]\n\
         url = \"https://brandnew.example\"\n",
    );
    // Deliberately: no seeded credentials of any kind.

    let result = handle_login(LoginArgs {
        profile: Some("brandnew".to_string()),
        url: None,
        oauth: false,
        api_token: true,
        email: Some("fresh@example.com".to_string()),
        token: Some("fresh-token".to_string()),
        client_id: None,
        client_secret: None,
        cloud_id: None,
        no_input: true,
        output: OutputFormat::Table,
    })
    .await;

    let oauth_after = auth::load_oauth_tokens(&Profile::from("brandnew"));
    let api_token_after = auth::load_api_token(&Profile::from("brandnew"));

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    result.expect("a brand-new profile's first login must succeed cleanly");

    let (email_after, token_after) =
        api_token_after.expect("the new api_token credentials must be stored");
    assert_eq!(email_after, "fresh@example.com");
    assert_eq!(token_after, "fresh-token");
    assert!(
        oauth_after.is_err(),
        "a brand-new profile must never end up with an OAuth pair when only \
         api_token was ever selected — the None-case reconcile step must not \
         have fabricated or touched anything"
    );
}

/// A FAILED mechanism-switch login on a legacy None-labelled profile that
/// already holds stored credentials must leave BOTH the old credentials AND
/// `auth_method` completely untouched — the reconcile step must never run
/// before a successful login (relogin-then-replace, mirrors
/// FIX-F5-login-switch's failing-half test for the `Some(_)` case).
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_fix_f5_cycle4_1_low1_failed_login_on_none_labelled_profile_leaves_old_creds_and_auth_method_untouched()
 {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("cycle4-low1-failed-login");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"legacy2\"\n\n\
         [profiles.legacy2]\n\
         url = \"https://legacy2.example\"\n",
    );
    auth::store_oauth_tokens(&Profile::from("legacy2"), "keep-access", "keep-refresh").unwrap();

    // Deterministic OAuth failure, zero keychain/network/browser I/O beyond
    // the flag/env layer — mirrors
    // `test_fix_f5_failed_login_switch_preserves_outgoing_api_token_creds`.
    let result = handle_login(LoginArgs {
        profile: Some("legacy2".to_string()),
        url: None,
        oauth: true,
        api_token: false,
        email: None,
        token: None,
        client_id: Some("only-id".to_string()),
        client_secret: None,
        cloud_id: None,
        no_input: false,
        output: OutputFormat::Table,
    })
    .await;

    let oauth_after = auth::load_oauth_tokens(&Profile::from("legacy2"));

    // Read back the persisted config to confirm `auth_method` was never
    // written for this profile either — the pre-mark guard (PR #771 NEW-1)
    // and this story's post-login reconcile step must both stay inert on a
    // failed login.
    let saved_config = std::fs::read_to_string(cfg_dir.join("config.toml")).unwrap();

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    let err = result.expect_err(
        "a --client-id-without-secret OAuth switch attempt must fail deterministically",
    );
    let jr_err = err
        .downcast_ref::<JrError>()
        .expect("must be a JrError so it maps to a real exit code");
    assert!(
        jr_err.to_string().contains("--client-secret"),
        "must fail via the OAuth app credential pair-gate, not some other \
         path — got: {jr_err}"
    );

    let (access_after, refresh_after) = oauth_after.expect(
        "FIX-F5-CYCLE4-1 LOW-1: a FAILED login on a legacy None-labelled \
         profile must leave its prior stored OAuth credentials completely \
         intact — the reconcile step must never run before a successful \
         login",
    );
    assert_eq!(access_after, "keep-access");
    assert_eq!(refresh_after, "keep-refresh");
    assert!(
        !saved_config.contains("auth_method"),
        "a failed login on a legacy None-labelled profile must not write \
         auth_method at all — got config:\n{saved_config}"
    );
}

/// AC-010 (BC-1.1.016 Postcondition 1/2/3, Precondition 2a) — see the
/// module-level "Safety note" for exactly why this cannot reach real
/// network/browser code, and why the exact-message assertion below is the
/// ordering proof.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ac_010_handle_login_noninteractive_explicit_oauth_fails_with_guard_message_not_credentials_message()
 {
    if !keyring_gate_active() {
        return;
    }
    if skip_if_embedded_oauth_present() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("login-guard-ordering");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }
    std::fs::create_dir_all(&cfg_dir).unwrap();

    let result = handle_login(LoginArgs {
        profile: Some("guardtest".to_string()),
        url: Some("https://guardtest.example".to_string()),
        oauth: true,
        api_token: false,
        email: None,
        token: None,
        client_id: None,
        client_secret: None,
        cloud_id: None,
        no_input: true,
        output: OutputFormat::Table,
    })
    .await;

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    let err = result.expect_err(
        "explicit --oauth under --no-input must fail (today via the credentials- \
         required fallback; after the guard lands, via the guard itself)",
    );
    let jr_err = err
        .downcast_ref::<JrError>()
        .expect("must be a JrError so it maps to a real exit code");
    assert!(matches!(jr_err, JrError::UserError(_)));
    assert_eq!(jr_err.exit_code(), 64);
    assert_eq!(
        jr_err.to_string(),
        NONINTERACTIVE_OAUTH_GUARD_MESSAGE,
        "AC-012 ordering proof: the guard must win the race against \
         resolve_oauth_app_credentials — today (unwired) this fails with \
         \"OAuth app credentials are required...\" instead, proving the \
         guard has not yet been inserted before the credential-resolution \
         code (RED). Got: {jr_err}"
    );
}

/// AC-011 (BC-1.1.016 Postcondition 1/2/3, Precondition 2b) — the implicit
/// case: `refresh` against a profile whose `auth_method == "oauth"`, no
/// explicit `--oauth` needed. Same ordering-proof mechanism as AC-010.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ac_011_refresh_noninteractive_implicit_oauth_profile_fails_with_guard_message_not_credentials_message()
 {
    if !keyring_gate_active() {
        return;
    }
    if skip_if_embedded_oauth_present() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("refresh-guard-ordering");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"staging\"\n\n\
         [profiles.staging]\n\
         url = \"https://staging.example\"\n\
         auth_method = \"oauth\"\n",
    );

    let output = OutputFormat::Table;
    let result = refresh_credentials(RefreshArgs {
        profile: Some("staging"),
        oauth: false, // implicit: profile's own auth_method already says oauth
        api_token: false,
        email: None,
        token: None,
        client_id: None,
        client_secret: None,
        no_input: true,
        output: &output,
    })
    .await;

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    let err = result.expect_err("implicit oauth-profile refresh under --no-input must fail");
    let jr_err = err
        .downcast_ref::<JrError>()
        .expect("must be a JrError so it maps to a real exit code");
    assert!(matches!(jr_err, JrError::UserError(_)));
    assert_eq!(jr_err.exit_code(), 64);
    assert_eq!(
        jr_err.to_string(),
        NONINTERACTIVE_OAUTH_GUARD_MESSAGE,
        "EC-1.1.016-2/3 ordering proof: today (unwired) refresh clears the \
         OAuth pair then fails via the credentials-required fallback \
         instead of the guard — RED. Got: {jr_err}"
    );
}

/// EC-1.1.016-3: `refresh --api-token` on an oauth-method profile,
/// non-interactive, must ALSO fail fast — the inert `--api-token` flag has
/// no override power over BC-1.1.016's guard.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ec_1_1_016_3_refresh_api_token_flag_does_not_bypass_oauth_guard() {
    if !keyring_gate_active() {
        return;
    }
    if skip_if_embedded_oauth_present() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("refresh-guard-inert-flag");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    write_config(
        &cfg_dir,
        "default_profile = \"staging\"\n\n\
         [profiles.staging]\n\
         url = \"https://staging.example\"\n\
         auth_method = \"oauth\"\n",
    );

    let output = OutputFormat::Table;
    let result = refresh_credentials(RefreshArgs {
        profile: Some("staging"),
        oauth: false,
        api_token: true, // inert per BC-1.2.051 — must not override the guard
        email: None,
        token: None,
        client_id: None,
        client_secret: None,
        no_input: true,
        output: &output,
    })
    .await;

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    let err = result.expect_err("--api-token must not bypass the non-interactive OAuth guard");
    let jr_err = err
        .downcast_ref::<JrError>()
        .expect("must be a JrError so it maps to a real exit code");
    assert_eq!(
        jr_err.to_string(),
        NONINTERACTIVE_OAUTH_GUARD_MESSAGE,
        "EC-1.1.016-3: got: {jr_err}"
    );
}

/// AC-009 / VP-AUTHDX-002, end-to-end companion to the in-memory proptest
/// above: proves `JiraClient::from_config`'s actual code path (not just the
/// `Option` semantics) still resolves an absent `auth_method` to
/// `"api_token"`. Seeds ONLY an api-token pair; if the fallback silently
/// changed to `"oauth"`, `from_config` would instead try to load OAuth
/// tokens (unseeded) and fail.
///
/// This is a REGRESSION PIN, not new functionality — `src/api/client.rs` is
/// untouched by this story. Expected GREEN already (documented, not a RED
/// gate failure) since the literal `.unwrap_or("api_token")` predates this
/// story.
#[tokio::test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
async fn test_ac_009_from_config_resolves_api_token_when_auth_method_absent_end_to_end() {
    if !keyring_gate_active() {
        return;
    }
    let _guard = env_lock().lock().await;
    let svc = unique_service("from-config-fallback");
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg_dir = tmp.path().join("jr");
    let cache_dir = tmp.path().join("cache");

    unsafe {
        scrub_jr_env();
        std::env::set_var("JR_SERVICE_NAME", &svc);
        std::env::set_var("JR_CONFIG_DIR", &cfg_dir);
        std::env::set_var("JR_CACHE_DIR", &cache_dir);
    }

    // `auth_method` deliberately OMITTED from this profile entry.
    write_config(
        &cfg_dir,
        "default_profile = \"default\"\n\n\
         [profiles.default]\n\
         url = \"https://default.example\"\n",
    );
    auth::store_api_token(&Profile::from("default"), "fc@example.com", "fc-token").unwrap();

    let config = jr::config::Config::load_with(None).unwrap();
    let result = jr::api::client::JiraClient::from_config(&config, false, false);

    unsafe {
        std::env::remove_var("JR_SERVICE_NAME");
        std::env::remove_var("JR_CONFIG_DIR");
        std::env::remove_var("JR_CACHE_DIR");
    }

    result.expect(
        "BC-1.1.015: an absent auth_method must resolve to api_token at runtime — \
         if this fails, from_config tried to load OAuth tokens instead, meaning \
         the .unwrap_or(\"api_token\") fallback regressed",
    );
}
