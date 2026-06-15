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

/// Verifies that `#[cfg(debug_assertions)]` appears on the immediately preceding
/// non-blank, non-comment source line before the `JR_CONFIG_DIR` env-var read in
/// `src/config.rs::global_config_dir()`.
///
/// Strategy: search the source text for the `std::env::var("JR_CONFIG_DIR")` read.
/// Then walk backward from that line, skipping blank lines and comment lines
/// (`//`-prefixed), and assert that the first non-blank non-comment line is
/// `#[cfg(debug_assertions)]`. This prevents an unrelated `#[cfg(debug_assertions)]`
/// elsewhere in the same function from falsely satisfying the gate check — only
/// the attribute on the line immediately controlling this env-var read counts.
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

    // Walk backward from the seam read line, skipping blank lines and comment lines,
    // and find the immediately preceding non-blank, non-comment line.
    let preceding_gate_line = lines[..seam_read_line]
        .iter()
        .rev()
        .find(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//")
        })
        .copied();

    let gate_present = preceding_gate_line
        .map(|l| l.contains("#[cfg(debug_assertions)]"))
        .unwrap_or(false);

    assert!(
        gate_present,
        "BC-6.2.017 AC-005 VIOLATION: `#[cfg(debug_assertions)]` is not on the \
         immediately preceding non-blank, non-comment source line before the \
         `JR_CONFIG_DIR` env-var read at line {} of src/config.rs.\n\
         The attribute must appear directly above the env-var read (skipping only \
         blank lines and comments) — an unrelated nearby attribute does not count.\n\
         The env-var read MUST be gated with `#[cfg(debug_assertions)]` so it is \
         excluded from release binaries (path-injection prevention — BC-6.2.017).\n\
         Mirrors the JR_BASE_URL SD-002 gate (see tests/base_url_release_gate.rs).\n\
         Preceding non-blank/non-comment line found: {:?}",
        seam_read_line + 1,
        preceding_gate_line
    );
}

/// Verifies that `#[cfg(debug_assertions)]` appears on the immediately preceding
/// non-blank, non-comment source line before the `JR_CACHE_DIR` env-var read in
/// `src/cache.rs::cache_root()`.
///
/// This is a SEPARATE required assertion from the config site check above.
/// Both sites must be gated — gating only one leaves the other as an attack
/// vector (same defect class as the dual-site JR_BASE_URL requirement).
///
/// Strategy: identical to the config-site test above, applied to `src/cache.rs`.
/// Walk backward from the seam read line, skip blank and comment lines, and
/// assert the first non-blank non-comment line is `#[cfg(debug_assertions)]`.
/// This prevents an unrelated nearby `#[cfg(debug_assertions)]` from falsely
/// satisfying the gate.
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

    // Walk backward from the seam read line, skipping blank lines and comment lines,
    // and find the immediately preceding non-blank, non-comment line.
    let preceding_gate_line = lines[..seam_read_line]
        .iter()
        .rev()
        .find(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//")
        })
        .copied();

    let gate_present = preceding_gate_line
        .map(|l| l.contains("#[cfg(debug_assertions)]"))
        .unwrap_or(false);

    assert!(
        gate_present,
        "BC-6.2.017 AC-006 VIOLATION: `#[cfg(debug_assertions)]` is not on the \
         immediately preceding non-blank, non-comment source line before the \
         `JR_CACHE_DIR` env-var read at line {} of src/cache.rs.\n\
         The attribute must appear directly above the env-var read (skipping only \
         blank lines and comments) — an unrelated nearby attribute does not count.\n\
         The env-var read MUST be gated with `#[cfg(debug_assertions)]` so it is \
         excluded from release binaries (path-injection prevention — BC-6.2.017).\n\
         Both the config site AND the cache site must be gated — gating only one \
         leaves the other as an attack vector.\n\
         Preceding non-blank/non-comment line found: {:?}",
        seam_read_line + 1,
        preceding_gate_line
    );
}

/// Verifies that `struct GlobalConfig` in `src/config.rs` does NOT declare a field
/// named `config_dir` or `cache_dir`.
///
/// # Security invariant (SEC-PATH-1 re-entry guard)
///
/// `JR_CONFIG_DIR` and `JR_CACHE_DIR` are intentionally debug-only seams, gated by
/// `#[cfg(debug_assertions)]` in `global_config_dir()` and `cache_root()`.  However,
/// figment's `Env::prefixed("JR_")` (used in `Config::load_inner`) would silently
/// honor `JR_CONFIG_DIR` / `JR_CACHE_DIR` as RELEASE-build overrides if
/// `GlobalConfig` gained a field of that name — because figment maps env-var suffixes
/// to struct fields case-insensitively.  An attacker who could set
/// `JR_CONFIG_DIR=/attacker/config` (e.g., via a compromised shell init) would
/// redirect ALL config reads to an attacker-controlled path, re-opening SEC-PATH-1.
///
/// This threat is documented at `src/config.rs::load_inner` in the GUARD comment.
/// This test makes the GUARD machine-enforceable.
///
/// # Scope
///
/// The assertion is intentionally scoped to the `struct GlobalConfig { ... }` body.
/// It will NOT false-match:
/// - The GUARD comment (`config_dir` / `cache_dir` appear there as prose)
/// - The free functions `global_config_dir()` / `cache_root()` (different identifiers)
/// - The `JR_CACHE_DIR` / `JR_CONFIG_DIR` env-var string literals
/// - Any other struct that legitimately carries path fields (e.g., `ProjectConfig`)
///
/// # Pass condition
///
/// PASSES today — `struct GlobalConfig` has no `config_dir` or `cache_dir` field.
/// FAILS if someone adds either field (the guard comment explicitly prohibits this;
/// this test enforces it mechanically).
#[test]
fn test_global_config_struct_has_no_path_override_field() {
    let source = include_str!("../src/config.rs");

    // Locate the start of the `struct GlobalConfig` definition.
    let struct_start = source.find("struct GlobalConfig").unwrap_or_else(|| {
        panic!(
            "SEC-PATH-1 GUARD: Could not locate `struct GlobalConfig` in src/config.rs.\n\
             Has the struct been renamed? Update this test to match the new name."
        )
    });

    // Find the closing brace of the struct body.  We scan forward from the opening
    // brace, tracking brace depth, to correctly handle nested braces in doc-comments
    // or attribute macros.
    let after_name = &source[struct_start..];
    let open_brace = after_name.find('{').unwrap_or_else(|| {
        panic!(
            "SEC-PATH-1 GUARD: Found `struct GlobalConfig` in src/config.rs but could \
             not find its opening brace `{{`. Unexpected source shape."
        )
    });

    let body_start = struct_start + open_brace;
    let body_src = &source[body_start..];

    // Walk forward counting brace depth to find the matching `}`.
    let mut depth: usize = 0;
    let mut body_end = body_start; // will be updated to the closing brace offset
    for (i, ch) in body_src.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = body_start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    let struct_body = &source[body_start..body_end];

    // Assert neither field name appears as a word boundary inside the struct body.
    // We use a simple "field declaration" heuristic: a field name appears after
    // optional pub/visibility and whitespace at the start of a declaration.
    // Checking for `config_dir` and `cache_dir` as substrings is sufficient here
    // because these identifiers do not appear elsewhere in the struct body (the GUARD
    // comment is outside the struct body; function names use `global_config_dir` /
    // `cache_root`, not `config_dir` or `cache_dir` bare).
    assert!(
        !struct_body.contains("config_dir"),
        "SEC-PATH-1 GUARD VIOLATION: `struct GlobalConfig` in src/config.rs declares \
         a field named `config_dir` (or a field whose name contains `config_dir`).\n\
         \n\
         This is PROHIBITED. If `GlobalConfig` gains a `config_dir` field, figment's \
         `Env::prefixed(\"JR_\")` will honor `JR_CONFIG_DIR` as a RELEASE-build override, \
         bypassing the `#[cfg(debug_assertions)]` gate in `global_config_dir()` and \
         re-opening the path-injection vector SEC-PATH-1.\n\
         \n\
         The GUARD comment at src/config.rs::load_inner explains the threat in prose. \
         This test enforces it mechanically.\n\
         \n\
         If you need to surface a config-dir path in GlobalConfig, do NOT use a \
         figment-deserialized field — use a method that reads the same \
         `#[cfg(debug_assertions)]`-gated seam as `global_config_dir()`."
    );

    assert!(
        !struct_body.contains("cache_dir"),
        "SEC-PATH-1 GUARD VIOLATION: `struct GlobalConfig` in src/config.rs declares \
         a field named `cache_dir` (or a field whose name contains `cache_dir`).\n\
         \n\
         This is PROHIBITED. If `GlobalConfig` gains a `cache_dir` field, figment's \
         `Env::prefixed(\"JR_\")` will honor `JR_CACHE_DIR` as a RELEASE-build override, \
         bypassing the `#[cfg(debug_assertions)]` gate in `cache_root()` and \
         re-opening the path-injection vector SEC-PATH-1.\n\
         \n\
         The GUARD comment at src/config.rs::load_inner explains the threat in prose. \
         This test enforces it mechanically.\n\
         \n\
         If you need to surface a cache-dir path in GlobalConfig, do NOT use a \
         figment-deserialized field — use a method that reads the same \
         `#[cfg(debug_assertions)]`-gated seam as `cache_root()`."
    );
}
