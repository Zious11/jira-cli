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
/// Call this BEFORE any `.env(...)` calls that set the kept seams (e.g.
/// `JR_BASE_URL`, `JR_CONFIG_DIR`) — calling it after would remove the
/// values the test just set, since both operations mutate the same
/// underlying environment map and the later call wins.
pub fn scrub_ambient_jr_env<'a>(cmd: &'a mut Command, keep: &[&str]) -> &'a mut Command {
    for (key, _) in std::env::vars() {
        if key.starts_with("JR_") && !keep.contains(&key.as_str()) {
            cmd.env_remove(key);
        }
    }
    cmd
}

/// Verifies `cwd` has no `.jr.toml` in itself or any ancestor directory —
/// the hermeticity precondition (`verification-delta.md` §2 step 2) for
/// every test relying on "no `.jr.toml` ancestor" to observe the
/// configured-default fallback chain in isolation. Panics loudly, naming
/// the offending ancestor, rather than silently skipping: a Rust
/// early-return 'skip' would report PASS and hide the violated
/// precondition.
pub fn assert_no_ancestor_jr_toml(cwd: &Path) {
    let mut dir = Some(cwd);
    while let Some(d) = dir {
        let candidate = d.join(".jr.toml");
        assert!(
            !candidate.exists(),
            "hermeticity precondition violated: found unexpected .jr.toml at {candidate:?} \
             (test cwd {cwd:?} must have no ancestor .jr.toml)"
        );
        dir = d.parent();
    }
}
