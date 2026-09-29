//! Shared hermeticity helpers for `jr user list` integration tests
//! (cycle-014 STORY-A, `S-cycle14-user-list-project-resolution`, #862,
//! Step 4.5 adversarial pass 1 findings F-002/F-005).
//!
//! Both helpers below were previously duplicated (with a stale, fixed-list
//! variant) across `tests/user_commands.rs`, `tests/user_list_project_resolution.rs`,
//! and `tests/user_pagination.rs`. They are consolidated here so a single
//! location expresses what "hermetic" means for this test surface.

use assert_cmd::Command;
use std::path::Path;

/// Scrubs every ambient `JR_`-prefixed environment variable from `cmd`,
/// except the names listed in `keep`.
///
/// `Config::load_inner` (`src/config.rs` ~L262) merges
/// `Env::prefixed("JR_")` directly onto `GlobalConfig`/`ProfileConfig` with
/// no closed enumeration of recognized keys — ANY ambient `JR_*` variable
/// that maps onto a config field (e.g. `JR_PROFILES`, `JR_DEFAULTS`,
/// `JR_INSTANCE`, `JR_FIELDS`, or any future field) can leak a
/// developer/CI environment's configured value into a test's resolved
/// `Config`. A fixed name list therefore silently misses new seams; this
/// helper instead removes every `JR_`-prefixed variable currently present
/// in the process environment, except an explicit caller-supplied
/// keep-list of seams the test itself deliberately sets.
///
/// Call this BEFORE any `.env(...)` calls that set a NON-kept `JR_*`
/// variable — ordering only matters for a `JR_*` key a test sets via
/// `.env(...)` that is NOT in `keep`: the scrub must run first, or the
/// value the test just set would be removed. Kept keys are never removed
/// by this function regardless of call order.
///
/// Iterates `vars_os()` (not `vars()`) so a non-UTF-8 ambient env var
/// cannot panic the scrub, and matches the `JR_` prefix (and the `keep`
/// list) ASCII-case-insensitively, mirroring figment's
/// `Env::prefixed("JR_")` (src/config.rs ~L262; figment 0.10
/// `providers/env.rs`), which matches the prefix case-insensitively on a
/// trimmed key. A case-sensitive, UTF-8-only scrub would leave a variable
/// like `jr_profiles` or `Jr_Profile` in place even though figment (and
/// therefore `jr` itself) still reads it.
pub fn scrub_ambient_jr_env<'a>(cmd: &'a mut Command, keep: &[&str]) -> &'a mut Command {
    for (key_os, _) in std::env::vars_os() {
        let key_lossy = key_os.to_string_lossy();
        if is_scrubbable(key_lossy.trim(), keep) {
            cmd.env_remove(key_os);
        }
    }
    cmd
}

/// Pure predicate backing [`scrub_ambient_jr_env`]: does `key` (already
/// trimmed) start with `JR_`, ASCII-case-insensitively, and is it absent
/// from `keep` (also compared ASCII-case-insensitively)?
///
/// Extracted as a standalone pure function so the matching logic — the
/// part that must mirror figment's case-insensitive `Env::prefixed`
/// behavior — can be unit-tested directly without spawning a subprocess,
/// and so a caller that cannot use [`scrub_ambient_jr_env`]'s
/// `assert_cmd::Command` signature (e.g. a raw `std::process::Command`)
/// can still share this one implementation rather than hand-rolling an
/// equivalent (and possibly divergent) scrub — see
/// `tests/api_query_param.rs::Harness::std_cmd`.
pub fn is_scrubbable(key: &str, keep: &[&str]) -> bool {
    key.len() >= 3
        && key.as_bytes()[..3].eq_ignore_ascii_case(b"JR_")
        && !keep.iter().any(|k| k.eq_ignore_ascii_case(key))
}

/// Verifies `cwd` has no `.jr.toml` in itself or any ancestor directory —
/// the hermeticity precondition (`verification-delta.md` §2 step 2) for
/// every test relying on "no `.jr.toml` ancestor" to observe the
/// configured-default fallback chain in isolation. Panics loudly, naming
/// the offending ancestor, rather than silently skipping: a Rust
/// early-return 'skip' would report PASS and hide the violated
/// precondition.
///
/// `cwd` is canonicalized (`std::fs::canonicalize`) before walking
/// ancestors, so this walks the same path `find_project_config`'s
/// `std::env::current_dir()` actually sees at runtime — on macOS, for
/// example, `/var` resolves to `/private/var`, so a symlinked prefix in
/// the caller-supplied `cwd` could otherwise walk a different ancestor
/// chain than the one `jr` itself resolves against. Falls back to the
/// given `cwd` unchanged if canonicalization fails (e.g. the directory
/// does not exist yet), so the precondition check still runs rather than
/// silently no-op'ing.
pub fn assert_no_ancestor_jr_toml(cwd: &Path) {
    let canonical = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let mut dir = Some(canonical.as_path());
    while let Some(d) = dir {
        let candidate = d.join(".jr.toml");
        assert!(
            !candidate.exists(),
            "hermeticity precondition violated: found unexpected .jr.toml at {candidate:?} \
             (test cwd {cwd:?}, canonicalized to {canonical:?}, must have no ancestor .jr.toml)"
        );
        dir = d.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::is_scrubbable;

    #[test]
    fn test_is_scrubbable_matches_jr_prefix_case_insensitively() {
        assert!(is_scrubbable("JR_X", &[]));
        assert!(is_scrubbable("jr_x", &[]));
        assert!(is_scrubbable("Jr_Profile", &[]));
    }

    #[test]
    fn test_is_scrubbable_respects_trimmed_leading_whitespace() {
        // Caller is expected to trim before calling; a pre-trimmed key
        // with no surrounding whitespace still matches.
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
}
