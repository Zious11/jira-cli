//! Regression-guard tests for issue #333: `JR_BULK_AWAIT_TIMEOUT_SECS` must be
//! gated behind `#[cfg(debug_assertions)]` so it is honored only in debug
//! binaries.
//!
//! # Why gate it at all?
//!
//! Unlike `JR_BASE_URL` / `JR_AUTH_HEADER` (which are security-critical because
//! they can redirect authenticated requests and leak bearer tokens), the
//! await-timeout knob is not security-critical: at worst a misuse shortens
//! or lengthens the bulk-poll wall-clock budget, both bounded by the caller's
//! operational tolerance. There is no token-leak vector and no exfil path.
//!
//! Even so, we gate it to:
//!   1. Keep the production CLI behavior deterministic — the documented
//!      `DEFAULT_BULK_AWAIT_TIMEOUT_SECS` (300s / 5min) is what users get,
//!      full stop.
//!   2. Mirror the established CLAUDE.md pattern for test-seam env vars
//!      (`JR_BASE_URL`, `JR_AUTH_HEADER`, `JR_BULK_UNKNOWN_GRACE_SECS` are all
//!      `#[cfg(debug_assertions)]`). Consistency reduces audit cost — one
//!      rule for all test-seam env vars.
//!   3. Make audit-visible that the env var is a TEST SEAM, not a tunable.
//!      An operator reading the source can immediately see "release builds
//!      ignore this" without grepping for runtime checks.
//!
//! # Test inventory
//!
//! | Test | What it pins |
//! |------|----|
//! | `test_333_cfg_gate_present_in_bulk_source` | `#[cfg(debug_assertions)]` adjacent to `JR_BULK_AWAIT_TIMEOUT_SECS` read in `src/api/jira/bulk.rs::resolve_bulk_await_timeout` |
//! | `test_333_debug_assertions_active_in_test_binary` | Compile-time evidence the gate is active during `cargo test` |

/// Verifies that `#[cfg(debug_assertions)]` appears adjacent to the
/// `JR_BULK_AWAIT_TIMEOUT_SECS` env-var read in `src/api/jira/bulk.rs`.
/// Pre-fix (no env var support at all): FAILS to locate the read. Post-fix:
/// the `#[cfg(debug_assertions)]` annotation wraps the env-var block (PASSES).
///
/// Strategy: look for `#[cfg(debug_assertions)]` within 5 source lines BEFORE
/// the `JR_BULK_AWAIT_TIMEOUT_SECS` string literal. Whitespace-tolerant.
/// Mirrors the strategy of `tests/base_url_release_gate.rs` and
/// `tests/bulk_unknown_grace_release_gate.rs`.
#[test]
fn test_333_cfg_gate_present_in_bulk_source() {
    let source = include_str!("../src/api/jira/bulk.rs");

    let lines: Vec<&str> = source.lines().collect();
    let env_read_line = lines
        .iter()
        .position(|l| l.contains("JR_BULK_AWAIT_TIMEOUT_SECS") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_BULK_AWAIT_TIMEOUT_SECS env-var read in \
             src/api/jira/bulk.rs. Has the code been moved or removed? Update \
             this test if the location changed.",
        );

    let window_start = env_read_line.saturating_sub(5);
    let window = &lines[window_start..=env_read_line];
    let gate_present = window
        .iter()
        .any(|l| l.contains("#[cfg(debug_assertions)]"));

    assert!(
        gate_present,
        "Issue #333 VIOLATION: `#[cfg(debug_assertions)]` not found within 5 lines of the \
         `JR_BULK_AWAIT_TIMEOUT_SECS` env-var read at line {} of src/api/jira/bulk.rs.\n\
         The env-var read MUST be gated with `#[cfg(debug_assertions)]` so release \
         binaries ignore it (test-seam discipline — mirrors JR_BASE_URL / JR_AUTH_HEADER / \
         JR_BULK_UNKNOWN_GRACE_SECS).\n\
         Relevant source window:\n{}",
        env_read_line + 1,
        window.join("\n")
    );
}

/// Compile-time evidence that the `#[cfg(debug_assertions)]` gate is active
/// when this test binary is compiled. Mirrors the equivalent assertion in
/// `tests/base_url_release_gate.rs` and `tests/bulk_unknown_grace_release_gate.rs`
/// — see those files for the full rationale.
#[test]
fn test_333_debug_assertions_active_in_test_binary() {
    const {
        assert!(
            cfg!(debug_assertions),
            "debug_assertions must be true when compiling this test binary — \
             issue #333 requires the #[cfg(debug_assertions)] guard on \
             JR_BULK_AWAIT_TIMEOUT_SECS to be active in test builds so the \
             tests/bulk_deadline_propagation.rs integration test can drive \
             the 429-storm clamp through the binary quickly (typically by \
             setting it to '30' so the wall-clock is ~30s instead of ~300s)."
        )
    }
}
