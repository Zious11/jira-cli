//! Phase F6 property verification for the Windows-build feature delta.
//!
//! Targets the two PURE path-fallback helpers introduced by S-WIN-1:
//!   - `jr::config::config_appdata_fallback(Option<String>) -> PathBuf`
//!   - `jr::cache::cache_localappdata_fallback(Option<String>) -> PathBuf`
//!
//! Both take the raw `env::var(NAME).ok()` value as a parameter, so they are
//! pure (no `std::env` access inside) and verifiable on any platform without a
//! `#[cfg(windows)]` gate.
//!
//! Invariants proven (BC-6.1.014 EC-1/EC-3, BC-6.2.016 EC-1/EC-4):
//!   P1. `None`           -> `PathBuf::from(".")`   (relative fallback)
//!   P2. `Some("")`       -> `PathBuf::from(".")`   (empty treated as unset)
//!   P3. `Some(non-empty)`-> `PathBuf::from(that)`  (pass-through, byte-exact)
//!   P4. output is NEVER an empty path (the "" sentinel never escapes)
//!
//! Kani note: a tractability probe (`tests`-external) confirmed Kani CAN prove
//! the `None`/empty class but bounded-string symbolic execution adds little over
//! an exhaustive-class proptest here (input space is 3 equivalence classes; no
//! parsing, no arithmetic, no indexing). The crate is not wired for Kani (no
//! `kani` dependency / harness). Property tests are the recorded method; see
//! `.factory/phase-f6-hardening/win-build/property-results.md`.

use jr::cache::cache_localappdata_fallback;
use jr::config::config_appdata_fallback;
use proptest::prelude::*;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Exhaustive equivalence-class coverage (deterministic, both helpers).
// ---------------------------------------------------------------------------

#[test]
fn test_config_appdata_fallback_none_is_dot() {
    assert_eq!(config_appdata_fallback(None), PathBuf::from("."));
}

#[test]
fn test_config_appdata_fallback_empty_is_dot() {
    assert_eq!(
        config_appdata_fallback(Some(String::new())),
        PathBuf::from(".")
    );
}

#[test]
fn test_cache_localappdata_fallback_none_is_dot() {
    assert_eq!(cache_localappdata_fallback(None), PathBuf::from("."));
}

#[test]
fn test_cache_localappdata_fallback_empty_is_dot() {
    assert_eq!(
        cache_localappdata_fallback(Some(String::new())),
        PathBuf::from(".")
    );
}

// ---------------------------------------------------------------------------
// Property: non-empty input is passed through byte-exact (P3) and the empty
// sentinel never escapes (P4). proptest fans out over arbitrary strings.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// P3: any NON-EMPTY string is returned verbatim as the path component.
    #[test]
    fn prop_config_nonempty_passthrough(s in "\\PC{1,256}") {
        prop_assume!(!s.is_empty());
        prop_assert_eq!(config_appdata_fallback(Some(s.clone())), PathBuf::from(&s));
    }

    /// P3 (cache twin).
    #[test]
    fn prop_cache_nonempty_passthrough(s in "\\PC{1,256}") {
        prop_assume!(!s.is_empty());
        prop_assert_eq!(cache_localappdata_fallback(Some(s.clone())), PathBuf::from(&s));
    }

    /// P4: the output is never the empty path, for ANY input (None, "", or any
    /// non-empty string). This pins that `""` is never returned as a sentinel —
    /// the load-bearing invariant relied on by the release-gate / OS-branch logic.
    #[test]
    fn prop_config_output_never_empty(opt in proptest::option::of("\\PC{0,256}")) {
        let out = config_appdata_fallback(opt);
        prop_assert!(!out.as_os_str().is_empty());
    }

    /// P4 (cache twin).
    #[test]
    fn prop_cache_output_never_empty(opt in proptest::option::of("\\PC{0,256}")) {
        let out = cache_localappdata_fallback(opt);
        prop_assert!(!out.as_os_str().is_empty());
    }

    /// Cross-helper equivalence: both fallbacks implement the SAME pure rule, so
    /// they must agree on every input. Pins that a future edit to one helper that
    /// diverges from the other is caught (they are byte-for-byte identical logic).
    #[test]
    fn prop_both_helpers_agree(opt in proptest::option::of("\\PC{0,256}")) {
        prop_assert_eq!(
            config_appdata_fallback(opt.clone()),
            cache_localappdata_fallback(opt)
        );
    }
}
