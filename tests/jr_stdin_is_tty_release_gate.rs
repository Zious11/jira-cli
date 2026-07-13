//! Regression-guard test for the `JR_STDIN_IS_TTY` debug seam in `src/main.rs`.
//!
//! The seam suppresses the auto-`--no-input` flip when `JR_STDIN_IS_TTY=1` is
//! set in debug builds, enabling interactive-confirmation tests (AC-004/AC-009
//! in `tests/comment_delete.rs`) to run even when stdin is piped. The env var
//! MUST be gated behind `#[cfg(debug_assertions)]` so release binaries ignore
//! it entirely.
//!
//! Mirrors `tests/base_url_release_gate.rs` structure — window search rather
//! than independent presence checks. Two independent checks cannot detect an
//! un-gated read where `#[cfg(debug_assertions)]` appears elsewhere in the file
//! but not adjacent to the env-var read.
//!
//! Story: S-577-3, GitHub issue #577
//! BC anchor: BC-3.5.003 delivery obligation, BC-3.5.006 item (c)

/// Verifies that `#[cfg(debug_assertions)]` appears adjacent to the
/// `JR_STDIN_IS_TTY` env-var read in `src/main.rs`.
///
/// Strategy: locate the `std::env::var("JR_STDIN_IS_TTY")` line; assert
/// `#[cfg(debug_assertions)]` exists within 5 source lines before it.
/// Whitespace-tolerant.
///
/// Red Gate: fails because the JR_STDIN_IS_TTY seam has not been added to
/// src/main.rs yet — `.expect(...)` panics with "Could not locate the
/// JR_STDIN_IS_TTY env-var read in src/main.rs."
#[test]
fn test_jr_stdin_is_tty_cfg_gate_present_in_main_source() {
    let src = include_str!("../src/main.rs");
    let lines: Vec<&str> = src.lines().collect();

    let env_read_line = lines
        .iter()
        .position(|l| l.contains("JR_STDIN_IS_TTY") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_STDIN_IS_TTY env-var read in src/main.rs. \
             Has the code been moved? Update this test if the location changed.",
        );

    let window_start = env_read_line.saturating_sub(5);
    let window = &lines[window_start..=env_read_line];
    let gate_present = window
        .iter()
        .any(|l| l.contains("#[cfg(debug_assertions)]"));

    assert!(
        gate_present,
        "JR_STDIN_IS_TTY VIOLATION: `#[cfg(debug_assertions)]` not found within 5 lines \
         of the `JR_STDIN_IS_TTY` env-var read at line {} of src/main.rs.\n\
         The env-var read MUST be gated so it is excluded from release binaries.\n\
         Relevant source window:\n{}",
        env_read_line + 1,
        window.join("\n")
    );
}
