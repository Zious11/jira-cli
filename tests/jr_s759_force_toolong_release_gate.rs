//! Regression-guard test for the `JR_S759_FORCE_TOOLONG` debug seam in
//! `src/api/auth.rs::forced_toolong` (called from `store_oauth_tokens`).
//!
//! The seam lets a non-Windows debug build simulate a
//! `keyring::Error::TooLong` on the access or refresh `set_password` call
//! inside `store_oauth_tokens` (AC-001-006/AC-019) — no real keyring backend
//! on macOS/Linux can actually produce this error, so without this seam the
//! `TooLong`-routing arms would be untestable off Windows. It MUST be gated
//! behind `#[cfg(debug_assertions)]` so release binaries never read it —
//! this is the established `JR_*` debug-only-seam convention (CLAUDE.md "AI
//! Agent Notes"; models `JR_S303_PERSIST_FAIL`'s shape).
//!
//! Mirrors `tests/jr_force_dpapi_fallback_release_gate.rs` structure —
//! window search rather than an independent presence check, so a
//! `debug_assertions` gate that exists ELSEWHERE in the file (but not
//! adjacent to the env-var read) cannot pass this test spuriously.
//!
//! Story: S-cycle4-dpapi-storage-fix (AC-001-006, AC-019)
//! BC anchor: BC-1.4.035 postcondition 5, invariant 2

/// Verifies that the `debug_assertions` cfg token appears within 5 source
/// lines of the `JR_S759_FORCE_TOOLONG` env-var read in `src/api/auth.rs`.
///
/// Strategy: locate the `std::env::var("JR_S759_FORCE_TOOLONG")` line;
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
fn test_jr_s759_force_toolong_cfg_gate_present_in_auth_source() {
    let src = include_str!("../src/api/auth.rs");
    let lines: Vec<&str> = src.lines().collect();

    let env_read_line = lines
        .iter()
        .position(|l| l.contains("JR_S759_FORCE_TOOLONG") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_S759_FORCE_TOOLONG env-var read (std::env::var(...)) in \
             src/api/auth.rs. If this seam has been removed or renamed, delete/update this \
             test in the same change.",
        );

    let window_start = env_read_line.saturating_sub(5);
    let window = &lines[window_start..=env_read_line];
    let gate_present = window.iter().any(|l| l.contains("debug_assertions"));

    assert!(
        gate_present,
        "JR_S759_FORCE_TOOLONG VIOLATION: a `debug_assertions` cfg gate was not found within \
         5 lines of the `JR_S759_FORCE_TOOLONG` env-var read at line {} of src/api/auth.rs.\n\
         The env-var read MUST be gated behind #[cfg(debug_assertions)] so release binaries \
         never read it.\n\
         Relevant source window:\n{}",
        env_read_line + 1,
        window.join("\n")
    );
}

/// Sibling presence check: the seam must live inside the `forced_toolong`
/// helper specifically — not merely somewhere in the file. This is a
/// coarser, best-effort corroborating check (function-body window), not a
/// full parser; it complements, and does not replace, the line-window check
/// above.
#[test]
fn test_jr_s759_force_toolong_read_is_inside_forced_toolong_fn() {
    let src = include_str!("../src/api/auth.rs");

    let fn_start = src.find("fn forced_toolong").expect(
        "Could not locate `fn forced_toolong` in src/api/auth.rs. Has the function been \
         renamed or moved? Update this test if the shape changed.",
    );

    // Bound the search to a generous window after the fn signature (the
    // function body should be small — this is a seam-gate check, not a
    // full-file scan) so a later, unrelated JR_S759_FORCE_TOOLONG
    // reference elsewhere in the file can't satisfy this assertion.
    let window_end = (fn_start + 1000).min(src.len());
    let fn_window = &src[fn_start..window_end];

    assert!(
        fn_window.contains("JR_S759_FORCE_TOOLONG"),
        "AC-001-006/AC-019 VIOLATION: the JR_S759_FORCE_TOOLONG env-var read must live inside \
         the forced_toolong helper, within ~1000 bytes of its `fn` signature. It was not found \
         there."
    );
}
