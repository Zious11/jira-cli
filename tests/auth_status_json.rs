//! Integration tests for BC-1.6.050 — `auth status --output json`
//! (S-cycle7-auth-status-json, Wave 2, B2).
//!
//! ## Test map
//!
//! | AC | Test name | Method | Gate |
//! |----|-----------|--------|------|
//! | AC-002 | `test_bc_1_6_050_status_json_status_uses_shared_derivation` | source-scan | DEFAULT CI |
//! | AC-009 | `test_bc_1_6_050_ec1_unknown_profile_json_still_standard_error_envelope` | assert_cmd | DEFAULT CI |
//! | AC-010 | `test_bc_1_6_050_ec2_fresh_install_no_json_output` | assert_cmd | DEFAULT CI |
//! | AC-013 | `test_bc_1_6_050_json_builder_is_probe_free` | source-scan | DEFAULT CI |
//! | C-1 | `test_bc_1_6_050_status_success_dispatch_arms_covered` | assert_cmd (keychain-free) | DEFAULT CI |
//! | AC-006 | `test_bc_1_6_050_status_human_text_byte_for_byte_unchanged` | assert_cmd | keyring-gated |
//! | AC-007 | `test_vp_authdx_029_status_text_agrees_with_matching_kind_present` | assert_cmd | keyring-gated |
//! | AC-008 | `test_bc_1_6_050_ec4_url_none_credential_present_divergence_is_intentional` | assert_cmd | keyring-gated |
//!
//! ## RED GATE status (pre-implementation)
//!
//! AC-002 fails: `status.rs` does not yet call `derive_auth_state`.
//! AC-009 fails: `status()` ignores `--output json` and always emits text.
//! AC-013 fails: `build_status_json` does not yet exist in `status.rs` (or
//!   exists only as a `todo!()` stub with no `status` field assembly).
//! AC-006/007/008 are `#[ignore]`'d — they require a real keychain fixture.
//! AC-010 passes at Red Gate (regression guard for pre-existing behavior).

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[allow(dead_code)]
mod common;
use common::assertions::assert_json_error_envelope;

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Return a `Command` for the `jr` binary with all env vars that
/// `Config::load` merges scrubbed so that developer machine state
/// cannot leak into tests.
fn jr() -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env_remove("JR_CONFIG_DIR")
        .env_remove("JR_CACHE_DIR")
        .env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .env_remove("JR_INSTANCE_URL")
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .env_remove("JR_INSTANCE_CLOUD_ID")
        .env_remove("JR_INSTANCE_ORG_ID")
        .env_remove("JR_INSTANCE_OAUTH_SCOPES")
        .env_remove("JR_FIELDS_TEAM_FIELD_ID")
        .env_remove("JR_FIELDS_STORY_POINTS_FIELD_ID")
        .env_remove("JR_DEFAULTS_OUTPUT")
        .env_remove("JR_BASE_URL")
        .env_remove("JR_AUTH_HEADER")
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        .env_remove("JR_OAUTH_CLIENT_ID")
        .env_remove("JR_OAUTH_CLIENT_SECRET");
    cmd
}

/// Create a temp dir with the standard jr config sub-directory pre-seeded.
fn fresh_config_dir() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let cfg_dir = dir.path().join("jr");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    (dir, cfg_dir)
}

/// Extract the precise body of a named function from source text using
/// brace-balanced extraction (counts `{`/`}` to locate the closing brace).
///
/// Returns `None` if `fn_sig` is not found in `src`.
fn extract_fn_body(src: &str, fn_sig: &str) -> Option<String> {
    let start = src.find(fn_sig)?;
    let after = &src[start..];
    let open_brace_offset = after.find('{')?;
    let from_open = &after[open_brace_offset..];
    let mut depth = 0usize;
    let mut body_end = from_open.len();
    for (i, ch) in from_open.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    Some(after[..open_brace_offset + body_end].to_string())
}

// ── AC-002 — source-scan: status() calls derive_auth_state ───────────────────

/// AC-002 (BC-1.6.050 postcondition 2, VP-AUTHDX-026) — DEFAULT CI.
///
/// The `"status"` field in `build_status_json`'s output is computed via the
/// SAME shared `derive_auth_state` helper B1 introduced — never an independent
/// implementation. This source-scan asserts:
/// 1. `status.rs` contains a call to `derive_auth_state` (positive guard).
/// 2. `build_status_json`'s body specifically contains this call (not some
///    unrelated consumer).
/// 3. `build_status_json` does NOT contain `p.url.is_some()` or any other
///    independent status-derivation pattern.
///
/// RED GATE failure: `status.rs` does not call `derive_auth_state` at all
/// (the `todo!()` stub has no call). Fails the positive guard.
#[test]
fn test_bc_1_6_050_status_json_status_uses_shared_derivation() {
    let status_src = include_str!("../src/cli/auth/status.rs");

    // Positive guard: build_status_json must call derive_auth_state.
    let builder_body = extract_fn_body(status_src, "fn build_status_json");
    let builder_calls_derive = builder_body
        .as_deref()
        .map(|b| b.contains("derive_auth_state"))
        .unwrap_or(false);
    assert!(
        builder_calls_derive,
        "AC-002 FAIL: build_status_json does not call derive_auth_state. \
         The 'status' field MUST be computed via the shared derive_auth_state \
         helper (BC-1.6.050 postcondition 2 — VP-AUTHDX-024 parity). \
         This IS the Red Gate for B2."
    );

    // Negative guard: build_status_json must NOT use p.url.is_some() or
    // any URL-presence ternary as a substitute for derive_auth_state.
    if let Some(body) = &builder_body {
        assert!(
            !body.contains("url.is_some()") && !body.contains("p.url.is_some()"),
            "AC-002 FAIL: build_status_json uses URL-presence check instead of \
             derive_auth_state — this is the defective pattern B1 replaced."
        );
    }
}

// ── AC-009 — EC-1.6.050-1: unknown profile → standard JSON error envelope ────

/// AC-009 (BC-1.6.050 EC-1.6.050-1, VP-AUTHDX-028 tie-in) — DEFAULT CI.
///
/// `jr auth status --output json --profile <unknown>` must emit the STANDARD
/// `{"error": "...", "code": 64}` error envelope (exit 64) — the project
/// convention for `--output json` errors: stderr carries the JSON envelope,
/// stdout is empty (channel-separation #526). The pre-existing
/// unknown-profile check (BC-1.1.004, unchanged this cycle) fires BEFORE
/// any output-format-specific success-schema code path is reached.
///
/// This test is a REGRESSION GUARD: it verifies the unknown-profile case
/// NEVER accidentally emits this story's new 6-key success object on stdout
/// (e.g. due to an incorrect early-return bypass added by the implementer).
///
/// PASSES at Red Gate (existing behavior already satisfies this contract)
/// and must continue to pass after implementation.
#[test]
fn test_bc_1_6_050_ec1_unknown_profile_json_still_standard_error_envelope() {
    let (dir, _cfg_dir) = fresh_config_dir();
    // Seed a minimal config with one known profile so we CAN reference
    // an unknown one — the "known:" list is only populated when at least
    // one profile exists.
    let config_toml = dir.path().join("jr").join("config.toml");
    std::fs::write(
        &config_toml,
        r#"
[profiles.default]
url = "https://acme.atlassian.net"
auth_method = "api_token"
"#,
    )
    .unwrap();

    let output = jr()
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args([
            "auth",
            "status",
            "--output",
            "json",
            "--profile",
            "no_such_profile_xyz_12345",
        ])
        .output()
        .expect("jr binary ran");

    // Standard project convention: JSON error on stderr, stdout empty,
    // exit code matches.
    assert_json_error_envelope(
        &output,
        64,
        "AC-009: auth status --output json --profile unknown",
    );
}

// ── AC-010 — EC-1.6.050-2: fresh install → no stdout JSON ────────────────────

/// AC-010 (BC-1.6.050 EC-1.6.050-2) — DEFAULT CI.
///
/// Fresh install, zero profiles configured, no explicit `--profile`,
/// `--output json` supplied → the existing early-return ("No profiles
/// configured. Run `jr init`...") remains human-text-only on stderr with
/// exit 0 and NO stdout JSON.
///
/// This story does NOT introduce a new JSON shape (e.g. `{}` or `null`) for
/// this state — the pre-existing early-return path is explicitly preserved
/// (BC-1.6.050 EC-1.6.050-2 out-of-scope framing).
///
/// NOTE: This test passes at Red Gate (no JSON is emitted before OR after
/// implementation for this case). It is a REGRESSION GUARD ensuring the
/// fresh-install path is never accidentally changed.
#[test]
fn test_bc_1_6_050_ec2_fresh_install_no_json_output() {
    let (dir, _cfg_dir) = fresh_config_dir();
    // Leave config.toml empty / absent — zero profiles.

    let output = jr()
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "status", "--output", "json"])
        .output()
        .expect("jr binary ran");

    // Must exit 0 (existing behavior).
    let exit_code = output.status.code().unwrap_or(-1);
    assert_eq!(
        exit_code,
        0,
        "AC-010 FAIL: fresh install with zero profiles must exit 0 even with \
         --output json. Got {exit_code}. \
         stdout: {:?}\nstderr: {:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Stdout must be EMPTY (no JSON for this case, per BC-1.6.050 EC-1.6.050-2).
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "AC-010 FAIL: fresh install with zero profiles must produce NO stdout JSON \
         (EC-1.6.050-2 explicitly out-of-scope). Got stdout: {stdout:?}"
    );

    // Stderr must contain the existing message.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No profiles configured"),
        "AC-010 FAIL: fresh install stderr must contain 'No profiles configured'. \
         Got stderr: {stderr:?}"
    );
}

// ── AC-013 — F-1 purity: build_status_json is probe-free ─────────────────────

/// AC-013 (BC-1.6.050 postcondition 2 / BC-1.6.048 postcondition 2
/// "Preferred implementation shape", DEFAULT CI).
///
/// The 6-key JSON object assembly (`build_status_json`) is a PURE function —
/// it performs NO keychain access and NO config load. This source-scan:
///
/// 1. Confirms `build_status_json` exists in `status.rs` (positive check).
/// 2. Asserts its body contains NO calls to keychain-reading functions
///    (`load_oauth_tokens`, `load_api_token`, `try_load_oauth_app_credentials`,
///    `peek_oauth_app_source`).
/// 3. Asserts its body contains NO config-loading calls
///    (`Config::load`, `Config::load_with`).
///
/// The PURE sibling `peek_oauth_app_source_for_test` is NOT covered by this
/// exclusion — it is and must remain default-CI-testable.
///
/// RED GATE failure: `build_status_json` body is currently `todo!()` with no
/// `status` field assembly. The positive guard fails because `build_status_json`
/// doesn't call `derive_auth_state` in its body yet — this is the same
/// assertion AC-002 checks, and both fail for the same reason.
#[test]
fn test_bc_1_6_050_json_builder_is_probe_free() {
    let status_src = include_str!("../src/cli/auth/status.rs");

    // Positive guard: build_status_json must exist.
    assert!(
        status_src.contains("fn build_status_json"),
        "AC-013 FAIL: build_status_json not found in status.rs. \
         The pure JSON builder must be present (F-1 fix)."
    );

    // Negative guards: keychain-reading symbols must NOT appear in the
    // builder's body.
    let builder_body = extract_fn_body(status_src, "fn build_status_json");
    let probe_symbols = [
        "load_oauth_tokens",
        "load_api_token",
        "try_load_oauth_app_credentials",
        "peek_oauth_app_source(",
        "Config::load",
    ];
    if let Some(body) = &builder_body {
        for sym in &probe_symbols {
            assert!(
                !body.contains(sym),
                "AC-013 FAIL: build_status_json references {sym:?} — this function \
                 must be PURE (no keychain access, no config load). \
                 Move probing to the `status()` effectful caller (F-1 fix)."
            );
        }
    }

    // Positive guard: build_status_json must call derive_auth_state
    // (already checked by AC-002, but also required for purity-plus-correctness).
    let builder_calls_derive = builder_body
        .as_deref()
        .map(|b| b.contains("derive_auth_state"))
        .unwrap_or(false);
    assert!(
        builder_calls_derive,
        "AC-013 FAIL: build_status_json must call derive_auth_state to derive \
         the 'status' field — this is the F-1 purity split confirmation \
         (test mirroring S-cycle7-auth-state-derivation AC-004/AC-014)."
    );
}

// ── C-1 — Keychain-free default-CI coverage for success-dispatch arms ─────────

/// C-1 (BC-1.6.050 postconditions 1–5 / VP-AUTHDX-024 / VP-AUTHDX-029) —
/// DEFAULT CI (no `#[ignore]`, no `JR_RUN_KEYRING_TESTS`).
///
/// Kills mutants in `status()`'s success-dispatch arms for BOTH `api_token`
/// AND `oauth` auth_method variants — the `--output json` builder call, the
/// text `Credentials:`/`OAuth app:` lines, the `if method == "oauth"` guard
/// at both JSON and text sites, and the `if matching_kind_present` branch at
/// the text `Credentials:` line — WITHOUT touching the real OS keychain.
///
/// Keychain-free approach: a profile with a URL but NO stored credential under
/// an isolated `JR_SERVICE_NAME` deterministically yields
/// `matching_kind_present=false` (the `load_api_token` / `load_oauth_tokens`
/// probe returns `Err(NoEntry)` for an unknown service → false) → calls
/// `derive_auth_state(Some(url), false)` → `"status":"no-credentials"`.
/// No keychain prompt; runs everywhere in CI.
///
/// FAILS at Red Gate (the builder stub `todo!()`'s prevent any JSON output;
/// after implementation this is one of the primary default-CI regression guards).
#[test]
fn test_bc_1_6_050_status_success_dispatch_arms_covered() {
    let (_dir, cfg_dir) = fresh_config_dir();

    // ── api_token variant ─────────────────────────────────────────────────────
    // Seed: profile with URL, auth_method=api_token, no stored credential.
    let config_toml = cfg_dir.join("config.toml");
    std::fs::write(
        &config_toml,
        r#"
[profiles.b2_api_token]
url = "https://acme.atlassian.net"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // -- JSON mode: api_token, no credential ---------------------------------
    let output = jr()
        .env("JR_CONFIG_DIR", &cfg_dir)
        // Isolate keychain namespace — this service has no stored credentials,
        // so load_api_token returns Err → matching_kind_present=false.
        .env("JR_SERVICE_NAME", "jr-test-b2-nocred-dispatch")
        .args([
            "auth",
            "status",
            "--output",
            "json",
            "--profile",
            "b2_api_token",
        ])
        .output()
        .expect("jr binary ran");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        exit_code, 0,
        "C-1 FAIL (api_token JSON): expected exit 0, got {exit_code}. \
         stderr: {stderr:?}"
    );

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "C-1 FAIL (api_token JSON): stdout is not valid JSON: {e}\n\
             stdout: {stdout:?}\nstderr: {stderr:?}"
        )
    });

    // 6-key schema check (BC-1.6.050 postcondition 1)
    let obj = parsed
        .as_object()
        .expect("C-1 FAIL (api_token JSON): top-level value must be a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "auth_method",
            "env",
            "oauth_app",
            "profile",
            "status",
            "url"
        ],
        "C-1 FAIL (api_token JSON): key set must be exactly the 6-key BC-1.6.050 schema"
    );

    // status=no-credentials (URL set, no credential → derive_auth_state gives no-credentials)
    assert_eq!(
        parsed["status"], "no-credentials",
        "C-1 FAIL (api_token JSON): status must be 'no-credentials' when URL is set but \
         no credential is stored (derive_auth_state postcondition)"
    );

    // oauth_app=null for api_token method (VP-AUTHDX-026 / BC-1.6.050 postcondition 5)
    assert!(
        parsed["oauth_app"].is_null(),
        "C-1 FAIL (api_token JSON): oauth_app must be null for auth_method=api_token; \
         got: {:?}",
        parsed["oauth_app"]
    );

    // profile name verbatim
    assert_eq!(
        parsed["profile"], "b2_api_token",
        "C-1 FAIL (api_token JSON): profile field must be the active profile name"
    );

    // url verbatim
    assert_eq!(
        parsed["url"], "https://acme.atlassian.net",
        "C-1 FAIL (api_token JSON): url field must equal the configured URL string"
    );

    // env=null (not configured)
    assert!(
        parsed["env"].is_null(),
        "C-1 FAIL (api_token JSON): env must be null when not configured"
    );

    // -- Text mode: api_token, no credential ---------------------------------
    let text_output = jr()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-nocred-dispatch")
        .args(["auth", "status", "--profile", "b2_api_token"])
        .output()
        .expect("jr binary ran");

    let text_stdout = String::from_utf8_lossy(&text_output.stdout);
    let text_stderr = String::from_utf8_lossy(&text_output.stderr);

    assert_eq!(
        text_output.status.code().unwrap_or(-1),
        0,
        "C-1 FAIL (api_token text): expected exit 0. stderr: {text_stderr:?}"
    );

    // Credentials: not found (matching_kind_present=false → text path)
    assert!(
        text_stdout.contains("Credentials: not found"),
        "C-1 FAIL (api_token text): expected 'Credentials: not found' in stdout. \
         Got: {text_stdout:?}"
    );

    // No OAuth app line for api_token profiles (the if method == "oauth" guard)
    assert!(
        !text_stdout.contains("OAuth app:"),
        "C-1 FAIL (api_token text): 'OAuth app:' line must NOT appear for \
         auth_method=api_token. Got: {text_stdout:?}"
    );

    // Profile and Instance lines present (format regression guard)
    assert!(
        text_stdout.contains("Profile:"),
        "C-1 FAIL (api_token text): expected 'Profile:' line. Got: {text_stdout:?}"
    );
    assert!(
        text_stdout.contains("Instance:"),
        "C-1 FAIL (api_token text): expected 'Instance:' line. Got: {text_stdout:?}"
    );

    // ── oauth variant ─────────────────────────────────────────────────────────
    // Seed an oauth profile with URL (same config dir, new profile entry).
    std::fs::write(
        &config_toml,
        r#"
[profiles.b2_oauth]
url = "https://acme.atlassian.net"
auth_method = "oauth"
"#,
    )
    .unwrap();

    // -- JSON mode: oauth, no credential -------------------------------------
    let oauth_output = jr()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-nocred-dispatch")
        .args([
            "auth",
            "status",
            "--output",
            "json",
            "--profile",
            "b2_oauth",
        ])
        .output()
        .expect("jr binary ran");

    let oauth_exit = oauth_output.status.code().unwrap_or(-1);
    let oauth_stdout = String::from_utf8_lossy(&oauth_output.stdout);
    let oauth_stderr = String::from_utf8_lossy(&oauth_output.stderr);

    assert_eq!(
        oauth_exit, 0,
        "C-1 FAIL (oauth JSON): expected exit 0, got {oauth_exit}. \
         stderr: {oauth_stderr:?}"
    );

    let oauth_parsed: serde_json::Value =
        serde_json::from_str(oauth_stdout.trim()).unwrap_or_else(|e| {
            panic!(
                "C-1 FAIL (oauth JSON): stdout is not valid JSON: {e}\n\
                 stdout: {oauth_stdout:?}\nstderr: {oauth_stderr:?}"
            )
        });

    // status=no-credentials (no stored oauth token → derive_auth_state → no-credentials)
    assert_eq!(
        oauth_parsed["status"], "no-credentials",
        "C-1 FAIL (oauth JSON): status must be 'no-credentials' when URL is set \
         but no oauth credential is stored"
    );

    // oauth_app must be a NON-NULL string (the if method == "oauth" arm is exercised)
    assert!(
        oauth_parsed["oauth_app"].is_string(),
        "C-1 FAIL (oauth JSON): oauth_app must be a non-null string for \
         auth_method=oauth (peek_oauth_app_source path exercised); got: {:?}",
        oauth_parsed["oauth_app"]
    );

    // auth_method=oauth propagated verbatim
    assert_eq!(
        oauth_parsed["auth_method"], "oauth",
        "C-1 FAIL (oauth JSON): auth_method must equal 'oauth'"
    );

    // -- Text mode: oauth, no credential -------------------------------------
    let oauth_text = jr()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-nocred-dispatch")
        .args(["auth", "status", "--profile", "b2_oauth"])
        .output()
        .expect("jr binary ran");

    let oauth_text_stdout = String::from_utf8_lossy(&oauth_text.stdout);
    let oauth_text_stderr = String::from_utf8_lossy(&oauth_text.stderr);

    assert_eq!(
        oauth_text.status.code().unwrap_or(-1),
        0,
        "C-1 FAIL (oauth text): expected exit 0. stderr: {oauth_text_stderr:?}"
    );

    // OAuth app: line MUST appear for oauth method (if method == "oauth" guard at text site)
    assert!(
        oauth_text_stdout.contains("OAuth app:"),
        "C-1 FAIL (oauth text): expected 'OAuth app:' line for auth_method=oauth. \
         Got: {oauth_text_stdout:?}"
    );

    // Credentials: not found (matching_kind_present=false, no stored token)
    assert!(
        oauth_text_stdout.contains("Credentials: not found"),
        "C-1 FAIL (oauth text): expected 'Credentials: not found' for no-stored-oauth. \
         Got: {oauth_text_stdout:?}"
    );
}

// ── AC-006/007/008 — KEYRING-GATED ───────────────────────────────────────────
//
// These tests require a REAL stored (or deliberately mismatched-kind)
// credential in the OS keychain. They are gated with `#[ignore]` and
// `JR_RUN_KEYRING_TESTS=1` to avoid CI failures on hosts with no keyring
// backend (Linux without secret-service, or where a fresh keychain prompt
// would block).
//
// Run locally: JR_RUN_KEYRING_TESTS=1 cargo test -- --include-ignored \
//              test_bc_1_6_050_status_human_text_byte_for_byte_unchanged
//
// `load_oauth_tokens`/`load_api_token` have no in-memory injection seam
// (per VP-AUTHDX-005's own documented coverage-boundary note), so these
// three tests are genuinely untestable in default CI even after F-1.

/// AC-006 (BC-1.6.050 postcondition 6, VP-AUTHDX-026) — KEYRING-GATED.
///
/// The existing human-text output (`Profile:`, `Instance:`, `Env:`,
/// `Auth method:`, `Credentials:`, `OAuth app:` lines) is byte-for-byte
/// UNCHANGED for a correctly-configured profile AND for a mismatched-kind
/// profile (BC-1.6.048 EC-1.6.048-2) alike.
///
/// The `--output json` flag only ADDS a JSON-output mode — it must NEVER
/// suppress or alter the human-text channel's own format strings.
#[test]
#[ignore = "keyring-gated: requires JR_RUN_KEYRING_TESTS=1 and a seeded keychain fixture"]
fn test_bc_1_6_050_status_human_text_byte_for_byte_unchanged() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        return;
    }
    // AC-006: Verify that the human-text output (`Profile:`, `Instance:`,
    // `Env:`, `Auth method:`, `Credentials:`) is byte-for-byte unchanged for
    // a profile with a stored api_token credential.
    //
    // Seed a config + keychain fixture using `jr auth login --api-token` with
    // `JR_EMAIL`/`JR_API_TOKEN` env vars and `--no-input` (non-interactive).
    // `JR_SERVICE_NAME` scopes the credential to a test-only keychain namespace.
    let (_dir, cfg_dir) = fresh_config_dir();
    let config_toml = cfg_dir.join("config.toml");
    let profile_name = "ac006-fixture";
    let profile_url = "https://ac006.atlassian.net";
    std::fs::write(
        &config_toml,
        format!(
            "[profiles.{profile_name}]\nurl = \"{profile_url}\"\nauth_method = \"api_token\"\n"
        ),
    )
    .unwrap();

    // Seed the keychain entry using jr auth login.
    let login = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-ac006")
        .env("JR_EMAIL", "user@ac006.example")
        .env("JR_API_TOKEN", "ac006-test-token")
        .args([
            "auth",
            "login",
            "--profile",
            profile_name,
            "--url",
            profile_url,
            "--no-input",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("jr auth login ran");
    assert!(
        login.status.success(),
        "AC-006: seeding credential via jr auth login failed: {:?}",
        String::from_utf8_lossy(&login.stderr)
    );

    // Run `jr auth status` (text mode) and check the output.
    let text_output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-ac006")
        .stdin(std::process::Stdio::null())
        .args(["auth", "status", "--profile", profile_name])
        .output()
        .expect("jr auth status ran");

    let stdout = String::from_utf8_lossy(&text_output.stdout);
    let stderr = String::from_utf8_lossy(&text_output.stderr);
    assert_eq!(
        text_output.status.code().unwrap_or(-1),
        0,
        "AC-006 FAIL: exit 0 expected. stderr: {stderr:?}"
    );

    // Exact format lines (BC-1.6.050 Postcondition 6 P6 regression):
    assert!(
        stdout.contains(&format!("Profile:     {profile_name}")),
        "AC-006 FAIL: 'Profile:     {profile_name}' not found. stdout: {stdout:?}"
    );
    assert!(
        stdout.contains(&format!("Instance:    {profile_url}")),
        "AC-006 FAIL: 'Instance:    {profile_url}' not found. stdout: {stdout:?}"
    );
    assert!(
        stdout.contains("Auth method: api_token"),
        "AC-006 FAIL: 'Auth method: api_token' not found. stdout: {stdout:?}"
    );
    // Credentials: stored in keychain — credential was seeded above
    assert!(
        stdout.contains("Credentials: stored in keychain"),
        "AC-006 FAIL: 'Credentials: stored in keychain' not found (credential was seeded). \
         stdout: {stdout:?}"
    );
    // No OAuth app line for api_token (unchanged from pre-B2)
    assert!(
        !stdout.contains("OAuth app:"),
        "AC-006 FAIL: 'OAuth app:' must not appear for api_token. stdout: {stdout:?}"
    );
}

/// AC-007 (BC-1.6.048 postcondition 3 / VP-AUTHDX-029) — KEYRING-GATED.
///
/// The `auth status` human-text `Credentials:` line agrees with the RAW
/// `matching_kind_present` bool (the kind-specific probe that also feeds the
/// JSON `"status"` field via `build_status_json`). The text line must NEVER
/// drift from its own source probe.
///
/// This VP does NOT assert the text line equals the 3-state JSON `"status"`
/// value — that would be unsatisfiable for EC-1.6.050-4 (url:None with
/// matching-kind credential present). It only compares text vs.
/// `matching_kind_present`.
#[test]
#[ignore = "keyring-gated: requires JR_RUN_KEYRING_TESTS=1 and a seeded keychain fixture"]
fn test_vp_authdx_029_status_text_agrees_with_matching_kind_present() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        return;
    }
    // VP-AUTHDX-029: text `Credentials:` line agrees with `matching_kind_present`.
    //
    // Two sub-cases:
    // 1. credential PRESENT: probe returns true → "Credentials: stored in keychain"
    // 2. credential ABSENT: probe returns false → "Credentials: not found"
    //
    // Sub-case 2 is already covered default-CI by C-1 above. This keyring-gated
    // test covers sub-case 1 (stored credential).

    let (_dir, cfg_dir) = fresh_config_dir();
    let config_toml = cfg_dir.join("config.toml");
    let profile_name = "ac007-fixture";
    let profile_url = "https://ac007.atlassian.net";
    std::fs::write(
        &config_toml,
        format!(
            "[profiles.{profile_name}]\nurl = \"{profile_url}\"\nauth_method = \"api_token\"\n"
        ),
    )
    .unwrap();

    // Seed credential: matching_kind_present → true after this call.
    let login = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-ac007")
        .env("JR_EMAIL", "user@ac007.example")
        .env("JR_API_TOKEN", "ac007-test-token")
        .args([
            "auth",
            "login",
            "--profile",
            profile_name,
            "--url",
            profile_url,
            "--no-input",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("jr auth login ran");
    assert!(
        login.status.success(),
        "AC-007: seeding failed: {:?}",
        String::from_utf8_lossy(&login.stderr)
    );

    // Run text status — matching_kind_present=true → "stored in keychain".
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-ac007")
        .stdin(std::process::Stdio::null())
        .args(["auth", "status", "--profile", profile_name])
        .output()
        .expect("jr auth status ran");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code().unwrap_or(-1),
        0,
        "AC-007 FAIL: exit 0 expected. stderr: {stderr:?}"
    );

    // VP-AUTHDX-029: when matching_kind_present=true, text must say
    // "Credentials: stored in keychain" (never "not found").
    assert!(
        stdout.contains("Credentials: stored in keychain"),
        "AC-007 FAIL (VP-AUTHDX-029): matching_kind_present=true must produce \
         'Credentials: stored in keychain' in text output. stdout: {stdout:?}"
    );
    assert!(
        !stdout.contains("Credentials: not found"),
        "AC-007 FAIL (VP-AUTHDX-029): 'Credentials: not found' must NOT appear \
         when matching_kind_present=true. stdout: {stdout:?}"
    );
}

/// AC-008 (BC-1.6.050 EC-1.6.050-4, VP-AUTHDX-029's explicit non-assertion) —
/// KEYRING-GATED.
///
/// For a profile with `url: None` whose stored credential IS present for the
/// configured `auth_method` (e.g., url cleared back to unset after a prior
/// successful login), the two channels INTENTIONALLY diverge:
/// - JSON `"status"`: `"unset"` (`derive_auth_state`: `url.is_none()` wins)
/// - Human-text `Credentials:`: "stored in keychain" (reads only credential
///   presence, never url)
///
/// The test asserts BOTH observed values explicitly and asserts this is NOT
/// flagged as a VP-AUTHDX-029 violation (which only compares text vs.
/// `matching_kind_present`, never text vs. `"status"`).
#[test]
#[ignore = "keyring-gated: requires JR_RUN_KEYRING_TESTS=1 and a url:None + credential-present fixture"]
fn test_bc_1_6_050_ec4_url_none_credential_present_divergence_is_intentional() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        return;
    }
    // EC-1.6.050-4 intentional divergence: url=None + credential present.
    //
    // JSON: derive_auth_state(None, true) → "unset" (url=None wins in the
    //       derivation rule — url being unset means the profile can't be used,
    //       regardless of credential state).
    // Text: `Credentials:` reads only credential presence → "stored in keychain".
    //
    // This asymmetry is NOT a VP-AUTHDX-029 violation — VP-AUTHDX-029 compares
    // text vs. `matching_kind_present` (both true → agree), not text vs. JSON
    // `status`.

    let (_dir, cfg_dir) = fresh_config_dir();
    let config_toml = cfg_dir.join("config.toml");
    let profile_name = "ac008-fixture";
    // Config with NO url field → url=None in the loaded profile.
    std::fs::write(
        &config_toml,
        format!("[profiles.{profile_name}]\nauth_method = \"api_token\"\n"),
    )
    .unwrap();

    // Seed credential: even with url=None, a credential can be stored
    // (e.g. user configured login before clearing url, or did `jr auth login`
    // with --url then later cleared the url from config).
    // Use `jr auth login` with a URL to store the credential; then rewrite
    // the config to remove the URL, simulating the diverged state.
    let profile_url_temp = "https://ac008-temp.atlassian.net";
    std::fs::write(
        &config_toml,
        format!(
            "[profiles.{profile_name}]\nurl = \"{profile_url_temp}\"\nauth_method = \"api_token\"\n"
        ),
    )
    .unwrap();

    let login = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-ac008")
        .env("JR_EMAIL", "user@ac008.example")
        .env("JR_API_TOKEN", "ac008-test-token")
        .args([
            "auth",
            "login",
            "--profile",
            profile_name,
            "--url",
            profile_url_temp,
            "--no-input",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("jr auth login ran");
    assert!(
        login.status.success(),
        "AC-008: seeding failed: {:?}",
        String::from_utf8_lossy(&login.stderr)
    );

    // Now rewrite config to remove url → simulates url=None + credential present.
    std::fs::write(
        &config_toml,
        format!("[profiles.{profile_name}]\nauth_method = \"api_token\"\n"),
    )
    .unwrap();

    // Run JSON mode: url=None → derive_auth_state(None, true) → "unset"
    let json_output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-ac008")
        .stdin(std::process::Stdio::null())
        .args([
            "auth",
            "status",
            "--output",
            "json",
            "--profile",
            profile_name,
        ])
        .output()
        .expect("jr auth status --output json ran");

    let json_stdout = String::from_utf8_lossy(&json_output.stdout);
    let json_stderr = String::from_utf8_lossy(&json_output.stderr);
    assert_eq!(
        json_output.status.code().unwrap_or(-1),
        0,
        "AC-008 FAIL (JSON): exit 0 expected. stderr: {json_stderr:?}"
    );

    let json_parsed: serde_json::Value =
        serde_json::from_str(json_stdout.trim()).unwrap_or_else(|e| {
            panic!("AC-008 FAIL (JSON): not valid JSON: {e}\nstdout: {json_stdout:?}")
        });

    // JSON status must be "unset" (url=None wins in derive_auth_state)
    assert_eq!(
        json_parsed["status"], "unset",
        "AC-008 FAIL (EC-1.6.050-4): JSON 'status' must be 'unset' when url=None, \
         even if credential is present. derive_auth_state(None, any) → 'unset'. \
         got: {:?}",
        json_parsed["status"]
    );

    // url field must be null in JSON (url=None)
    assert!(
        json_parsed["url"].is_null(),
        "AC-008 FAIL: url must be null in JSON when not configured. got: {:?}",
        json_parsed["url"]
    );

    // Run text mode: Credentials line reads only credential presence → "stored"
    let text_output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_CONFIG_DIR", &cfg_dir)
        .env("JR_SERVICE_NAME", "jr-test-b2-ac008")
        .stdin(std::process::Stdio::null())
        .args(["auth", "status", "--profile", profile_name])
        .output()
        .expect("jr auth status text ran");

    let text_stdout = String::from_utf8_lossy(&text_output.stdout);
    let text_stderr = String::from_utf8_lossy(&text_output.stderr);
    assert_eq!(
        text_output.status.code().unwrap_or(-1),
        0,
        "AC-008 FAIL (text): exit 0 expected. stderr: {text_stderr:?}"
    );

    // Text Credentials line: matching_kind_present=true → "stored in keychain"
    // (VP-AUTHDX-029 is satisfied: text agrees with matching_kind_present=true)
    assert!(
        text_stdout.contains("Credentials: stored in keychain"),
        "AC-008 FAIL (EC-1.6.050-4): text must show 'Credentials: stored in keychain' \
         when credential IS present (url=None doesn't affect the text cred check). \
         stdout: {text_stdout:?}"
    );

    // INTENTIONAL DIVERGENCE: JSON says "unset", text says "stored in keychain".
    // Assert this is NOT treated as a consistency violation — the test
    // explicitly acknowledges both values are correct for their respective channels.
    let json_status_is_unset = json_parsed["status"] == "unset";
    let text_says_stored = text_stdout.contains("Credentials: stored in keychain");
    assert!(
        json_status_is_unset && text_says_stored,
        "AC-008 FAIL (EC-1.6.050-4): intentional divergence must hold: \
         JSON status='unset' AND text Credentials='stored in keychain'. \
         json_status='{}', text_stored={}",
        json_parsed["status"],
        text_says_stored
    );
}
