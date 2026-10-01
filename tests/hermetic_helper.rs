//! Unit tests for `tests/common/hermetic.rs`'s pure helpers (CR3-004,
//! FIX-P5-004). These previously lived in an inline `#[cfg(test)] mod tests`
//! inside the shared helper, which every integration crate that does
//! `mod common;` recompiled and re-ran; they now run exactly once, here.

#[allow(dead_code)]
mod common;

use common::hermetic::is_scrubbable;

#[test]
fn test_is_scrubbable_matches_jr_prefix_case_insensitively() {
    assert!(is_scrubbable("JR_X", &[]));
    assert!(is_scrubbable("jr_x", &[]));
    assert!(is_scrubbable("Jr_Profile", &[]));
}

/// `is_scrubbable` itself does NOT trim: its input is documented as already
/// trimmed (`scrub_ambient_jr_env` calls `.trim()` before it). So an
/// untrimmed key with leading whitespace is not matched, while the same key
/// after the caller's trim is. (This pins the predicate's contract; it does
/// not exercise `scrub_ambient_jr_env`'s own trim, which would require
/// mutating the process environment.)
#[test]
fn test_is_scrubbable_does_not_trim_its_own_input() {
    assert!(!is_scrubbable(" JR_X", &[]));
    assert!(is_scrubbable(" JR_X".trim(), &[]));
}

#[test]
fn test_is_scrubbable_keeps_kept_keys_case_insensitively() {
    assert!(!is_scrubbable("JR_CONFIG_DIR", &["JR_CONFIG_DIR"]));
    assert!(!is_scrubbable("jr_config_dir", &["JR_CONFIG_DIR"]));
    assert!(!is_scrubbable("JR_CONFIG_DIR", &["jr_config_dir"]));
}

#[test]
fn test_is_scrubbable_rejects_non_jr_prefix() {
    assert!(!is_scrubbable("NOTJR_X", &[]));
    assert!(!is_scrubbable("JR", &[]));
    assert!(!is_scrubbable("", &[]));
}
