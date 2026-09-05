//! Regression-guard test for the `JR_FORCE_DPAPI_FALLBACK` debug seam in
//! `src/api/auth.rs::engage_dpapi_fallback` (`#[cfg(not(windows))]` arm).
//!
//! The seam lets a Linux/macOS debug build exercise `store_oauth_tokens`'s
//! DPAPI-fallback routing dispatch for testing (AC-006/AC-019, VP-AUTHDX-011
//! sub-property (2), VP-AUTHDX-012, VP-AUTHDX-022) — when
//! `JR_FORCE_DPAPI_FALLBACK=1` is set, `engage_dpapi_fallback` returns
//! `should_fallback_to_dpapi(err)` instead of the hardcoded `false` every
//! release build (and every debug build with the env var unset) uses. It
//! MUST be gated behind `#[cfg(debug_assertions)]` so release binaries never
//! read it — this is the established `JR_*` debug-only-seam convention
//! (CLAUDE.md "AI Agent Notes"; ADR-0021 §1's expanded doc-fallout note;
//! VP-AUTHDX-023).
//!
//! Mirrors `tests/jr_test_block_until_sigint_release_gate.rs` /
//! `tests/jr_stdin_is_tty_release_gate.rs` structure — window search rather
//! than an independent presence check, so a `debug_assertions` gate that
//! exists ELSEWHERE in the file (but not adjacent to the env-var read)
//! cannot pass this test spuriously.
//!
//! Story: S-cycle4-dpapi-storage-fix (AC-008, VP-AUTHDX-023)
//! BC anchor: BC-1.4.035 Invariant 3

/// Verifies that the `debug_assertions` cfg token appears within 5 source
/// lines of the `JR_FORCE_DPAPI_FALLBACK` env-var read in `src/api/auth.rs`.
///
/// Strategy: locate the `std::env::var("JR_FORCE_DPAPI_FALLBACK")` line;
/// assert `debug_assertions` appears somewhere in the 5-line window ending at
/// (and including) that line. This is a genuine token-presence check, not a
/// match against a fixed set of exact bracketed literals — it accepts any
/// spelling of the seam's gate (e.g. reordered `all(...)` forms) so long as
/// the literal token `debug_assertions` appears in the window. It still
/// catches the case that actually matters: an UN-gated read that would
/// compile into a release binary.
///
/// This is a cross-platform STATIC source-scan test (string matching over
/// `include_str!`-ed source) — it is NOT `#[cfg]`-gated and runs on every
/// platform, matching every sibling `*_release_gate.rs` test in this repo.
#[test]
fn test_jr_force_dpapi_fallback_cfg_gate_present_in_auth_source() {
    let src = include_str!("../src/api/auth.rs");
    let lines: Vec<&str> = src.lines().collect();

    let env_read_line = lines
        .iter()
        .position(|l| l.contains("JR_FORCE_DPAPI_FALLBACK") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_FORCE_DPAPI_FALLBACK env-var read (std::env::var(...)) in \
             src/api/auth.rs. `engage_dpapi_fallback`'s #[cfg(not(windows))] seam is implemented \
             as of S-cycle4-dpapi-storage-fix and is expected to read this env var — if this \
             assertion fires, the seam has been removed or renamed. This test asserts the \
             seam's #[cfg(debug_assertions)] gate is adjacent to the read.",
        );

    let window_start = env_read_line.saturating_sub(5);
    let window = &lines[window_start..=env_read_line];
    let gate_present = window.iter().any(|l| l.contains("debug_assertions"));

    assert!(
        gate_present,
        "JR_FORCE_DPAPI_FALLBACK VIOLATION: a `debug_assertions` cfg gate was not found within \
         5 lines of the `JR_FORCE_DPAPI_FALLBACK` env-var read at line {} of src/api/auth.rs.\n\
         The env-var read MUST be gated behind #[cfg(debug_assertions)] so release binaries \
         never read it — see ADR-0021 §1 and BC-1.4.035 Invariant 3.\n\
         Relevant source window:\n{}",
        env_read_line + 1,
        window.join("\n")
    );
}

/// Sibling presence check (AC-008's second half): the seam must live inside
/// the `#[cfg(not(windows))]` arm of `engage_dpapi_fallback` specifically —
/// not merely somewhere in the file. This is a coarser, best-effort
/// corroborating check (function-body window), not a full parser; it
/// complements, and does not replace, the line-window check above.
#[test]
fn test_jr_force_dpapi_fallback_read_is_inside_non_windows_engage_dpapi_fallback_arm() {
    let src = include_str!("../src/api/auth.rs");

    let not_windows_fn_start = src.find("#[cfg(not(windows))]\nfn engage_dpapi_fallback").expect(
        "Could not locate `#[cfg(not(windows))]\\nfn engage_dpapi_fallback` in src/api/auth.rs. \
         Has the function been reformatted or moved? Update this test if the shape changed.",
    );

    // Bound the search to a generous window after the fn signature (the
    // function body should be small — this is a seam-gate check, not a
    // full-file scan) so a later, unrelated JR_FORCE_DPAPI_FALLBACK
    // reference elsewhere in the file can't satisfy this assertion.
    let window_end = (not_windows_fn_start + 2000).min(src.len());
    let fn_window = &src[not_windows_fn_start..window_end];

    assert!(
        fn_window.contains("JR_FORCE_DPAPI_FALLBACK"),
        "AC-008 VIOLATION: the JR_FORCE_DPAPI_FALLBACK env-var read must live inside the \
         #[cfg(not(windows))] arm of engage_dpapi_fallback, within ~2000 bytes of its `fn` \
         signature. It was not found there."
    );
}
