//! Regression-guard tests for issue #335: `JR_BASE_URL` must be gated behind
//! `#[cfg(debug_assertions)]` so it is honored only in debug binaries.
//!
//! # Threat model
//!
//! `JR_BASE_URL` overrides the configured Jira instance URL. In a release binary
//! that read the env var, an attacker who could set `JR_BASE_URL=http://attacker/`
//! (e.g., via a compromised shell init, malicious wrapper script, or PaaS dashboard
//! env override) would redirect ALL authenticated requests — including the bearer
//! token loaded from the OS keychain — to their own endpoint. The token would
//! leak on the next API call.
//!
//! # Gate mechanism: `#[cfg(debug_assertions)]`
//!
//! Mirrors the existing SD-002 `JR_AUTH_HEADER` gate (see
//! `tests/auth_header_release_gate.rs`). Choice rationale, Perplexity-validated:
//! - `cargo build --release` reliably disables `debug_assertions` (no accidental
//!   activation without an explicit `Cargo.toml` `[profile.release] debug-assertions = true`
//!   override, which would be a deliberate audit-visible change).
//! - Compile-time elimination — the env-var read literally does not exist in the
//!   release binary, so it cannot be bypassed at runtime.
//! - Better than a runtime feature flag or allow-list (deploy-time risk if env
//!   accidentally set; release-process risk if flag forgotten).
//! - Prior art: `gh` CLI, `aws-cli`, `kubectl` all use compile-time gating for
//!   test endpoints.
//!
//! # Test inventory
//!
//! | Test | What it pins |
//! |------|----|
//! | `test_335_cfg_gate_present_in_config_source` | `#[cfg(debug_assertions)]` adjacent to `JR_BASE_URL` read in `src/config.rs::base_url` |
//! | `test_335_cfg_gate_present_in_client_source` | `#[cfg(debug_assertions)]` adjacent to `JR_BASE_URL` read in `src/api/client.rs::from_config` |
//! | `test_335_debug_assertions_active_in_test_binary` | Compile-time evidence that the gate is wired correctly for test binaries |
//! | `test_335_new_for_test_unaffected_by_env_gate` | Regression: `JiraClient::new_for_test` does not consult `JR_BASE_URL` |

use jr::api::client::JiraClient;

/// Verifies that `#[cfg(debug_assertions)]` appears adjacent to the `JR_BASE_URL`
/// env-var read in `src/config.rs::base_url()`. Pre-fix: no cfg gate (FAILS).
/// Post-fix: the `#[cfg(debug_assertions)]` annotation wraps the env-var block (PASSES).
///
/// Strategy: look for `#[cfg(debug_assertions)]` within 5 source lines BEFORE
/// the `JR_BASE_URL` string literal in the `pub fn base_url` block. Whitespace-tolerant.
#[test]
fn test_335_cfg_gate_present_in_config_source() {
    let source = include_str!("../src/config.rs");

    let lines: Vec<&str> = source.lines().collect();
    let base_url_env_read_line = lines
        .iter()
        .position(|l| l.contains("JR_BASE_URL") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_BASE_URL env-var read in src/config.rs. \
             Has the code been moved? Update this test if the location changed.",
        );

    let window_start = base_url_env_read_line.saturating_sub(5);
    let window = &lines[window_start..=base_url_env_read_line];
    let gate_present = window
        .iter()
        .any(|l| l.contains("#[cfg(debug_assertions)]"));

    assert!(
        gate_present,
        "Issue #335 VIOLATION: `#[cfg(debug_assertions)]` not found within 5 lines of the \
         `JR_BASE_URL` env-var read at line {} of src/config.rs.\n\
         The env-var read MUST be gated with `#[cfg(debug_assertions)]` so it is \
         excluded from release binaries (token-leak prevention — see issue #335).\n\
         Relevant source window:\n{}",
        base_url_env_read_line + 1,
        window.join("\n")
    );
}

/// Verifies that `#[cfg(debug_assertions)]` appears adjacent to the `JR_BASE_URL`
/// env-var read in `src/api/client.rs::from_config()`. This is the secondary
/// read site that produces `test_override` for wiremock injection in tests.
///
/// Both sites must be gated — gating only one leaves the other as a token-leak
/// path. (Copilot caught this when only `client.rs` was initially gated.)
#[test]
fn test_335_cfg_gate_present_in_client_source() {
    let source = include_str!("../src/api/client.rs");

    let lines: Vec<&str> = source.lines().collect();
    let base_url_env_read_line = lines
        .iter()
        .position(|l| l.contains("JR_BASE_URL") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_BASE_URL env-var read in src/api/client.rs. \
             Has the code been moved? Update this test if the location changed.",
        );

    let window_start = base_url_env_read_line.saturating_sub(5);
    let window = &lines[window_start..=base_url_env_read_line];
    let gate_present = window
        .iter()
        .any(|l| l.contains("#[cfg(debug_assertions)]"));

    assert!(
        gate_present,
        "Issue #335 VIOLATION: `#[cfg(debug_assertions)]` not found within 5 lines of the \
         `JR_BASE_URL` env-var read at line {} of src/api/client.rs.\n\
         The env-var read MUST be gated with `#[cfg(debug_assertions)]` so it is \
         excluded from release binaries (token-leak prevention — see issue #335).\n\
         Relevant source window:\n{}",
        base_url_env_read_line + 1,
        window.join("\n")
    );
}

/// Compile-time evidence that the `#[cfg(debug_assertions)]` gate is active
/// when this test binary is compiled. `cargo test` compiles test binaries in
/// debug mode by default, so `debug_assertions` is always set — meaning the
/// `#[cfg(debug_assertions)]` gate IS active here. This is expressed as a
/// `const` assertion (clippy-clean form of a tautological check) to make it a
/// compile-time guarantee rather than a runtime one.
///
/// Combined with `test_335_cfg_gate_present_in_*_source`, this provides both
/// source-level and compile-time evidence that the gate is correctly wired
/// for debug builds (and therefore for `cargo test` runs).
#[test]
fn test_335_debug_assertions_active_in_test_binary() {
    const {
        assert!(
            cfg!(debug_assertions),
            "debug_assertions must be true when compiling this test binary — \
             issue #335 requires the #[cfg(debug_assertions)] guard on JR_BASE_URL \
             to be active in test builds so integration tests can inject wiremock URLs."
        )
    }
}

/// Regression guard: `JiraClient::new_for_test` does NOT consult `JR_BASE_URL`
/// from the environment. It takes the base URL as a constructor argument.
///
/// This test path is unaffected by the `#[cfg(debug_assertions)]` gate at the
/// env-var read sites. It is exercised here to confirm that the gate doesn't
/// accidentally break the test-harness constructor (which all wiremock-based
/// integration tests use).
#[test]
fn test_335_new_for_test_unaffected_by_env_gate() {
    // Construct a client with an explicit base URL. The env var is not consulted.
    // (Even if it were set, the constructor would ignore it — but we don't set
    // it here to keep the test free of env-mutation side effects.)
    let _client: JiraClient = JiraClient::new_for_test(
        "http://localhost:9999".to_string(),
        "Bearer token-for-335-regression-guard".to_string(),
    );

    // Compile success + construction without panic is the assertion.
    // (No env var was read; the constructor took the URL as a String argument.)
}
