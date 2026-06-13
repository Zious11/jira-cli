//! Regression-guard tests for S-WIN-2 (BC-6.2.017): `JR_CONFIG_DIR` and
//! `JR_CACHE_DIR` must be gated behind `#[cfg(debug_assertions)]` so they are
//! honored only in debug binaries.
//!
//! # Threat model
//!
//! `JR_CONFIG_DIR` overrides the config directory. `JR_CACHE_DIR` overrides the
//! cache root. In a release binary that read either env var, an attacker who could
//! set `JR_CONFIG_DIR=/attacker/config` (e.g., via a compromised shell init,
//! malicious wrapper script, or PaaS dashboard env override) would redirect ALL
//! config reads to their own path — potentially loading a crafted config that
//! re-points the Jira URL to an attacker-controlled endpoint, causing token leakage
//! on the next API call. This threat class is the same as `JR_BASE_URL` (SD-002).
//!
//! # Gate mechanism: `#[cfg(debug_assertions)]`
//!
//! Mirrors `tests/base_url_release_gate.rs` (SD-002 pattern, issue #335):
//! - `cargo build --release` reliably disables `debug_assertions` (no accidental
//!   activation without an explicit `[profile.release] debug-assertions = true`
//!   override in `Cargo.toml`, which would be a deliberate audit-visible change).
//! - Compile-time elimination: the env-var read literally does not exist in the
//!   release binary, so it cannot be bypassed at runtime.
//! - Both read sites must be gated. Gating only one leaves the other as a path
//!   through which the attack can still succeed — the same defect that required
//!   `JR_BASE_URL` to be gated at TWO source sites (see base_url_release_gate.rs).
//!
//! # Test inventory
//!
//! | Test | AC | What it pins |
//! |------|----|-------------|
//! | `test_jr_config_dir_seam_is_debug_gated_at_config_site` | AC-005 | `#[cfg(debug_assertions)]` adjacent to `JR_CONFIG_DIR` read in `src/config.rs::global_config_dir` |
//! | `test_jr_cache_dir_seam_is_debug_gated_at_cache_site` | AC-006 | `#[cfg(debug_assertions)]` adjacent to `JR_CACHE_DIR` read in `src/cache.rs::cache_root` |
//! | compile-time `const { assert!(cfg!(debug_assertions)) }` | AC-007 | The test file itself fails to compile under `--release`; confirms gate is active for test binaries |

// AC-007: Compile-time assertion that this test binary is compiled with
// debug_assertions active.  `cargo test` always compiles test binaries in debug
// mode, so this is a tautology for normal test runs — but it would produce a
// compile error if someone mistakenly compiled tests with `--release`, ensuring
// the source-adjacency tests below cannot give a false green in a release build.
//
// Pre-implementation Red Gate: this line compiles and passes today (debug_assertions
// IS active in a normal `cargo test` run). The Red Gate for this file comes from
// the two source-adjacency tests below, which fail because the seam code is absent.
const _: () = {
    assert!(
        cfg!(debug_assertions),
        "JR_CONFIG_DIR/JR_CACHE_DIR release gate (BC-6.2.017 AC-007): \
         debug_assertions must be true when compiling this test binary. \
         The #[cfg(debug_assertions)] guard on JR_CONFIG_DIR/JR_CACHE_DIR requires \
         debug builds for the seam to be active. If you see this error, you have \
         compiled tests with --release, which is not supported for this test file."
    )
};

/// Verifies that `#[cfg(debug_assertions)]` appears adjacent to the
/// `JR_CONFIG_DIR` env-var read in `src/config.rs::global_config_dir()`.
///
/// Strategy: search the source text for the `std::env::var("JR_CONFIG_DIR")`
/// read (or a variant containing `"JR_CONFIG_DIR"` next to `std::env::var`).
/// Then assert that `#[cfg(debug_assertions)]` appears within 5 source lines
/// BEFORE that line. Whitespace-tolerant.
///
/// Pre-implementation Red Gate: ASSERTION FAILURE — the `JR_CONFIG_DIR` env-var
/// read does not yet exist in `src/config.rs`, so `position(...)` panics with
/// "Could not locate …". This gives the correct Red Gate signal: the seam is
/// absent and the test correctly detects its absence.
///
/// Post-implementation: `global_config_dir()` begins with the seam block:
/// ```rust
/// #[cfg(debug_assertions)]
/// if let Some(dir) = std::env::var("JR_CONFIG_DIR").ok().filter(|s| !s.is_empty()) {
///     return PathBuf::from(dir);
/// }
/// ```
/// The adjacency assertion then passes.
#[test]
fn test_jr_config_dir_seam_is_debug_gated_at_config_site() {
    let source = include_str!("../src/config.rs");

    let lines: Vec<&str> = source.lines().collect();
    let seam_read_line = lines
        .iter()
        .position(|l| l.contains("JR_CONFIG_DIR") && l.contains("std::env::var"))
        .expect(
            "BC-6.2.017 AC-005 VIOLATION: Could not locate the JR_CONFIG_DIR env-var \
             read (std::env::var(\"JR_CONFIG_DIR\")) in src/config.rs. \
             The seam has not been implemented yet, or it has been moved. \
             Implement the seam in global_config_dir() per BC-6.2.017 and S-WIN-2.",
        );

    let window_start = seam_read_line.saturating_sub(5);
    let window = &lines[window_start..=seam_read_line];
    let gate_present = window
        .iter()
        .any(|l| l.contains("#[cfg(debug_assertions)]"));

    assert!(
        gate_present,
        "BC-6.2.017 AC-005 VIOLATION: `#[cfg(debug_assertions)]` not found within \
         5 lines of the `JR_CONFIG_DIR` env-var read at line {} of src/config.rs.\n\
         The env-var read MUST be gated with `#[cfg(debug_assertions)]` so it is \
         excluded from release binaries (path-injection prevention — BC-6.2.017).\n\
         Mirrors the JR_BASE_URL SD-002 gate (see tests/base_url_release_gate.rs).\n\
         Relevant source window:\n{}",
        seam_read_line + 1,
        window.join("\n")
    );
}

/// Verifies that `#[cfg(debug_assertions)]` appears adjacent to the
/// `JR_CACHE_DIR` env-var read in `src/cache.rs::cache_root()`.
///
/// This is a SEPARATE required assertion from the config site check above.
/// Both sites must be gated — gating only one leaves the other as an attack
/// vector (same defect class as the dual-site JR_BASE_URL requirement).
///
/// Strategy: identical to the config-site test above, applied to `src/cache.rs`.
///
/// Pre-implementation Red Gate: ASSERTION FAILURE — the `JR_CACHE_DIR` env-var
/// read does not yet exist in `src/cache.rs`, so `position(...)` panics with
/// "Could not locate …". Correct Red Gate signal.
///
/// Post-implementation: `cache_root()` begins with the seam block:
/// ```rust
/// #[cfg(debug_assertions)]
/// if let Some(dir) = std::env::var("JR_CACHE_DIR").ok().filter(|s| !s.is_empty()) {
///     return PathBuf::from(dir);
/// }
/// ```
/// The adjacency assertion then passes.
#[test]
fn test_jr_cache_dir_seam_is_debug_gated_at_cache_site() {
    let source = include_str!("../src/cache.rs");

    let lines: Vec<&str> = source.lines().collect();
    let seam_read_line = lines
        .iter()
        .position(|l| l.contains("JR_CACHE_DIR") && l.contains("std::env::var"))
        .expect(
            "BC-6.2.017 AC-006 VIOLATION: Could not locate the JR_CACHE_DIR env-var \
             read (std::env::var(\"JR_CACHE_DIR\")) in src/cache.rs. \
             The seam has not been implemented yet, or it has been moved. \
             Implement the seam in cache_root() per BC-6.2.017 and S-WIN-2.",
        );

    let window_start = seam_read_line.saturating_sub(5);
    let window = &lines[window_start..=seam_read_line];
    let gate_present = window
        .iter()
        .any(|l| l.contains("#[cfg(debug_assertions)]"));

    assert!(
        gate_present,
        "BC-6.2.017 AC-006 VIOLATION: `#[cfg(debug_assertions)]` not found within \
         5 lines of the `JR_CACHE_DIR` env-var read at line {} of src/cache.rs.\n\
         The env-var read MUST be gated with `#[cfg(debug_assertions)]` so it is \
         excluded from release binaries (path-injection prevention — BC-6.2.017).\n\
         Both the config site AND the cache site must be gated — gating only one \
         leaves the other as an attack vector.\n\
         Relevant source window:\n{}",
        seam_read_line + 1,
        window.join("\n")
    );
}
