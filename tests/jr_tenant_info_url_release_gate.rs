//! Regression-guard test for `JR_TENANT_INFO_URL`: it must be gated behind
//! `#[cfg(debug_assertions)]` so it is honored only in debug binaries
//! (S-cycle4-cloud-id-correctness, ADR-0022, BC-1.2.052).
//!
//! # Threat model
//!
//! `JR_TENANT_INFO_URL` overrides the actual network target of
//! `fetch_cloud_id`'s `GET {site}/_edge/tenant_info` request while
//! `site_url` itself remains what the `https://`-prefix precondition
//! validates. In a release binary that read this env var, an attacker who
//! could set `JR_TENANT_INFO_URL=http://attacker/` (e.g., via a compromised
//! shell init, malicious wrapper script, or PaaS dashboard env override)
//! would redirect the tenant_info lookup to their own endpoint and could
//! return a well-formed `{"cloudId": "<attacker-chosen-uuid>"}` that gets
//! persisted into the victim's profile — a wrong-tenant Assets/CMDB
//! misdirection, the exact class of attack `fetch_cloud_id`'s `https://`-only
//! precondition (ADR-0022 §1, Pass-4 adversarial review Finding #4) and
//! `redirect::Policy::none()` (Finding #12) already close for the real
//! network path. This seam must not reopen that closed vector for release
//! binaries. Treated with the same care as `JR_BASE_URL` (`tests/base_url_
//! release_gate.rs`), not as a "non-security" seam.
//!
//! # Gate mechanism: `#[cfg(debug_assertions)]`
//!
//! Mirrors the existing `JR_BASE_URL` gate. Choice rationale (same as
//! `tests/base_url_release_gate.rs`):
//! - `cargo build --release` reliably disables `debug_assertions`.
//! - Compile-time elimination — the env-var read literally does not exist
//!   in the release binary, so it cannot be bypassed at runtime.
//! - Prior art in this codebase: `JR_BASE_URL`, `JR_CONFIG_DIR`,
//!   `JR_CACHE_DIR`, `JR_SERVICE_NAME`, `JR_STDIN_IS_TTY`.
//!
//! # Test inventory
//!
//! | Test | What it pins |
//! |------|----|
//! | `test_cfg_gate_present_in_tenant_source` | `#[cfg(debug_assertions)]` adjacent to the `JR_TENANT_INFO_URL` read in `src/api/jira/tenant.rs::fetch_cloud_id` |
//! | `test_debug_assertions_active_in_test_binary` | Compile-time evidence the gate is active for test binaries |

/// Verifies that `#[cfg(debug_assertions)]` appears adjacent to the
/// `JR_TENANT_INFO_URL` env-var read in
/// `src/api/jira/tenant.rs::fetch_cloud_id`. Pre-fix: no cfg gate (FAILS).
/// Post-fix: the `#[cfg(debug_assertions)]` annotation wraps the env-var
/// read (PASSES).
///
/// Strategy: look for `#[cfg(debug_assertions)]` within 5 source lines
/// BEFORE the `JR_TENANT_INFO_URL` string literal. Whitespace-tolerant.
#[test]
fn test_cfg_gate_present_in_tenant_source() {
    let source = include_str!("../src/api/jira/tenant.rs");

    let lines: Vec<&str> = source.lines().collect();
    let env_read_line = lines
        .iter()
        .position(|l| l.contains("JR_TENANT_INFO_URL") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_TENANT_INFO_URL env-var read in \
             src/api/jira/tenant.rs. Has the code been moved? Update this \
             test if the location changed.",
        );

    let window_start = env_read_line.saturating_sub(5);
    let window = &lines[window_start..=env_read_line];
    let gate_present = window
        .iter()
        .any(|l| l.contains("#[cfg(debug_assertions)]"));

    assert!(
        gate_present,
        "SECURITY VIOLATION: `#[cfg(debug_assertions)]` not found within 5 \
         lines of the `JR_TENANT_INFO_URL` env-var read at line {} of \
         src/api/jira/tenant.rs.\n\
         This env-var read MUST be gated with `#[cfg(debug_assertions)]` so \
         it is excluded from release binaries — an ungated read would let an \
         attacker who controls the process environment redirect the \
         unauthenticated tenant_info lookup to a host of their choosing and \
         have the returned cloudId persisted (see this file's module doc for \
         the full threat model).\n\
         Relevant source window:\n{}",
        env_read_line + 1,
        window.join("\n")
    );
}

/// Compile-time evidence that the `#[cfg(debug_assertions)]` gate is active
/// when this test binary is compiled. `cargo test` compiles test binaries in
/// debug mode by default, so `debug_assertions` is always set — meaning the
/// `#[cfg(debug_assertions)]` gate IS active here, which is what lets
/// `tests/cloud_id_tenant_info.rs`'s `JR_TENANT_INFO_URL`-dependent tests
/// exercise the seam at all.
#[test]
fn test_debug_assertions_active_in_test_binary() {
    const {
        assert!(
            cfg!(debug_assertions),
            "debug_assertions must be true when compiling this test binary — \
             the #[cfg(debug_assertions)] guard on JR_TENANT_INFO_URL must be \
             active in test builds so integration tests can inject a mock \
             tenant_info server."
        )
    }
}
