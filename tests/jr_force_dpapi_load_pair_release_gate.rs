//! Regression-guard test for the `JR_FORCE_DPAPI_LOAD_PAIR` debug seam in
//! `src/api/auth_windows_store.rs::load_pair` (`#[cfg(not(windows))]` arm).
//!
//! The seam lets a non-Windows debug build exercise `load_oauth_tokens`'s
//! DPAPI-fallback read-path branches (AC-009/AC-010/AC-011, BC-1.4.036) —
//! `load_pair` is hardcoded `Ok(None)` on `#[cfg(not(windows))]` once the
//! path-traversal guard passes, so without this seam the "DPAPI file
//! present" and "DPAPI file corrupt" read-path shapes would be untestable
//! off Windows. It MUST be gated behind `#[cfg(debug_assertions)]` so
//! release binaries never read it — this is the established `JR_*`
//! debug-only-seam convention (CLAUDE.md "AI Agent Notes"; mirrors
//! `JR_FORCE_DPAPI_FALLBACK`/`JR_S759_FORCE_TOOLONG`'s shape byte-for-byte).
//!
//! Mirrors `tests/jr_force_dpapi_fallback_release_gate.rs` structure —
//! window search rather than an independent presence check, so a
//! `debug_assertions` gate that exists ELSEWHERE in the file (but not
//! adjacent to the env-var read) cannot pass this test spuriously.
//!
//! Story: S-cycle4-dpapi-storage-fix (AC-009, AC-010, AC-011)
//! BC anchor: BC-1.4.036 postconditions 2/3, invariants 1/3

/// Verifies that the `debug_assertions` cfg token appears within 5 source
/// lines of the `JR_FORCE_DPAPI_LOAD_PAIR` env-var read in
/// `src/api/auth_windows_store.rs`.
///
/// Strategy: locate the `std::env::var("JR_FORCE_DPAPI_LOAD_PAIR")` line;
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
fn test_jr_force_dpapi_load_pair_cfg_gate_present_in_auth_windows_store_source() {
    let src = include_str!("../src/api/auth_windows_store.rs");
    let lines: Vec<&str> = src.lines().collect();

    let env_read_line = lines
        .iter()
        .position(|l| l.contains("JR_FORCE_DPAPI_LOAD_PAIR") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_FORCE_DPAPI_LOAD_PAIR env-var read (std::env::var(...)) \
             in src/api/auth_windows_store.rs. If this seam has been removed or renamed, \
             delete/update this test in the same change.",
        );

    let window_start = env_read_line.saturating_sub(5);
    let window = &lines[window_start..=env_read_line];
    let gate_present = window.iter().any(|l| l.contains("debug_assertions"));

    assert!(
        gate_present,
        "JR_FORCE_DPAPI_LOAD_PAIR VIOLATION: a `debug_assertions` cfg gate was not found \
         within 5 lines of the `JR_FORCE_DPAPI_LOAD_PAIR` env-var read at line {} of \
         src/api/auth_windows_store.rs.\n\
         The env-var read MUST be gated behind #[cfg(debug_assertions)] so release binaries \
         never read it.\n\
         Relevant source window:\n{}",
        env_read_line + 1,
        window.join("\n")
    );
}

/// Sibling presence check: the seam must live inside the `#[cfg(not(windows))]`
/// arm of `load_pair` specifically — not merely somewhere in the file. This
/// is a coarser, best-effort corroborating check (function-body window), not
/// a full parser; it complements, and does not replace, the line-window
/// check above.
#[test]
fn test_jr_force_dpapi_load_pair_read_is_inside_non_windows_load_pair_arm() {
    let src = include_str!("../src/api/auth_windows_store.rs");

    let not_windows_fn_start = src.find("#[cfg(not(windows))]\npub fn load_pair").expect(
        "Could not locate `#[cfg(not(windows))]\\npub fn load_pair` in \
         src/api/auth_windows_store.rs. Has the function been reformatted or moved? Update \
         this test if the shape changed.",
    );

    // Bound the search to a generous window after the fn signature (the
    // function body should be small — this is a seam-gate check, not a
    // full-file scan) so a later, unrelated JR_FORCE_DPAPI_LOAD_PAIR
    // reference elsewhere in the file can't satisfy this assertion.
    let window_end = (not_windows_fn_start + 1500).min(src.len());
    let fn_window = &src[not_windows_fn_start..window_end];

    assert!(
        fn_window.contains("JR_FORCE_DPAPI_LOAD_PAIR"),
        "AC-009/AC-010/AC-011 VIOLATION: the JR_FORCE_DPAPI_LOAD_PAIR env-var read must live \
         inside the #[cfg(not(windows))] arm of load_pair, within ~1500 bytes of its `fn` \
         signature. It was not found there."
    );
}
