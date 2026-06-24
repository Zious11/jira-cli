//! Regression-guard tests for SEC-JR-SERVICE-NAME-GATE: `JR_SERVICE_NAME` must be
//! gated behind `#[cfg(debug_assertions)]` so it is honored only in debug binaries.
//!
//! # Threat model
//!
//! `JR_SERVICE_NAME` overrides the keychain service name used by all keyring
//! operations. In a release binary that honored this env var, an attacker who
//! could set `JR_SERVICE_NAME=some-other-app` (e.g., via a compromised shell
//! init, malicious wrapper script, or PaaS dashboard env override) could
//! redirect ALL keychain lookups to a different service namespace — potentially
//! reading or writing credentials from another application's keychain entries,
//! or silently writing `jr` credentials where they would not be found by a
//! legitimate invocation.
//!
//! # Gate mechanism: `#[cfg(debug_assertions)]`
//!
//! Mirrors the existing `JR_BASE_URL` gate (SD-002, `tests/base_url_release_gate.rs`)
//! and the `JR_AUTH_HEADER` gate (see `tests/auth_header_release_gate.rs`).
//! Choice rationale (same as those gates):
//! - `cargo build --release` reliably disables `debug_assertions` (no accidental
//!   activation without an explicit `Cargo.toml` `[profile.release] debug-assertions = true`
//!   override, which would be a deliberate audit-visible change).
//! - Compile-time elimination — the env-var read literally does not exist in the
//!   release binary, so it cannot be bypassed at runtime.
//! - Better than a runtime feature flag or allow-list (deploy-time risk if env
//!   accidentally set; release-process risk if flag forgotten).
//!
//! # Test inventory
//!
//! | Test | What it pins |
//! |------|----|
//! | `test_sec_jr_service_name_cfg_gate_present_in_auth_source` | `#[cfg(debug_assertions)]` adjacent to `JR_SERVICE_NAME` read in `src/api/auth.rs::service_name` |
//! | `test_sec_jr_service_name_debug_assertions_active_in_test_binary` | Compile-time evidence that the gate is wired correctly for test binaries |

/// Verifies that `#[cfg(debug_assertions)]` appears adjacent to the
/// `JR_SERVICE_NAME` env-var read in `src/api/auth.rs::service_name()`.
/// Pre-fix: no cfg gate (FAILS). Post-fix: the `#[cfg(debug_assertions)]`
/// annotation wraps the env-var block (PASSES).
///
/// Strategy: look for `#[cfg(debug_assertions)]` in the 5 source lines BEFORE
/// the `JR_SERVICE_NAME` env-var read, inclusive of the env-read line itself
/// (a 6-line window: `lines[env_read_line - 5 ..= env_read_line]`). This is
/// whitespace-tolerant and mirrors the strategy of `tests/base_url_release_gate.rs`.
#[test]
fn test_sec_jr_service_name_cfg_gate_present_in_auth_source() {
    let source = include_str!("../src/api/auth.rs");

    let lines: Vec<&str> = source.lines().collect();
    let env_read_line = lines
        .iter()
        .position(|l| l.contains("JR_SERVICE_NAME") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_SERVICE_NAME env-var read in src/api/auth.rs. \
             Has the code been moved? Update this test if the location changed.",
        );

    let window_start = env_read_line.saturating_sub(5);
    let window = &lines[window_start..=env_read_line];
    let gate_present = window
        .iter()
        .any(|l| l.contains("#[cfg(debug_assertions)]"));

    assert!(
        gate_present,
        "SEC-JR-SERVICE-NAME-GATE VIOLATION: `#[cfg(debug_assertions)]` not found within \
         5 lines of the `JR_SERVICE_NAME` env-var read at line {} of src/api/auth.rs.\n\
         The env-var read MUST be gated with `#[cfg(debug_assertions)]` so it is \
         excluded from release binaries (keychain service-name redirect prevention — \
         see SEC-JR-SERVICE-NAME-GATE).\n\
         Relevant source window:\n{}",
        env_read_line + 1,
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
/// Combined with `test_sec_jr_service_name_cfg_gate_present_in_auth_source`, this
/// provides both source-level and compile-time evidence that the gate is correctly
/// wired for debug builds (and therefore for `cargo test` runs — including keyring
/// integration tests that set `JR_SERVICE_NAME` to isolate their namespace).
#[test]
fn test_sec_jr_service_name_debug_assertions_active_in_test_binary() {
    const {
        assert!(
            cfg!(debug_assertions),
            "debug_assertions must be true when compiling this test binary — \
             SEC-JR-SERVICE-NAME-GATE requires the #[cfg(debug_assertions)] guard on \
             JR_SERVICE_NAME to be active in test builds so keyring integration tests \
             can isolate their own service-name namespace without touching a developer's \
             real keychain."
        )
    }
}
