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
    // This test requires:
    // 1. A config profile with both url and a stored api_token credential.
    // 2. The implementer runs `jr auth status` WITHOUT --output json (baseline)
    //    and WITH --output json, then asserts the stdout of the text run is
    //    unchanged. Because `JR_SERVICE_NAME` can isolate the keychain
    //    namespace, a well-isolated fixture profile can be seeded, tested,
    //    and cleaned up without touching the developer's real credentials.
    //
    // TODO (implementer): flesh out this test body once `status()` has the
    // `&OutputFormat` parameter and `probe_matching_kind_credential` is extracted.
    unimplemented!(
        "AC-006: keyring-gated human-text unchanged regression test — \
         requires seeded keychain fixture (S-cycle7-auth-status-json Task 3)"
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
    // TODO (implementer): flesh out once `probe_matching_kind_credential`
    // is extracted and the text `Credentials:` line is re-sourced through it.
    // The test runs `jr auth status` (text), captures stdout, checks that
    // "Credentials: stored in keychain" appears iff the kind-specific probe
    // returns true for the seeded fixture profile.
    unimplemented!(
        "AC-007: VP-AUTHDX-029 text-vs-probe parity — keyring-gated \
         (S-cycle7-auth-status-json Task 4)"
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
    // TODO (implementer): seed a profile with url=None but a stored api_token,
    // run both `jr auth status` (text) and `jr auth status --output json`,
    // assert text says "Credentials: stored in keychain" and JSON says
    // "status": "unset", and assert no other test or VP treats this divergence
    // as a failure.
    unimplemented!(
        "AC-008: EC-1.6.050-4 intentional text-vs-machine divergence — keyring-gated \
         (S-cycle7-auth-status-json Task 4)"
    );
}
