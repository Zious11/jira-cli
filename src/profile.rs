//! Type-level profile fence (ADR-0011, un-deferred by DEC-317).
//!
//! `Profile` is a minimal, dependency-free newtype wrapper around a profile name. Its
//! purpose is to make a profile-unaware call site (a bare `&str`, or a hardcoded string
//! literal, passed where a real active-profile name is expected) a compile error instead
//! of a silent cross-profile leakage risk.
//!
//! # Scaffolding-only, as of this commit
//!
//! This story lands in two steps: this stub step introduces the newtype itself, fully
//! implemented (it is trivial — presence-not-correctness, see below). The mechanical
//! call-site sweep through `src/cache.rs`'s 16 per-profile functions, `src/api/auth.rs`'s
//! 4 credential functions, `Config::active_profile_name`, and `JiraClient::profile_name`
//! is the implementer step's TDD work and has NOT happened yet — those signatures still
//! read `profile: &str` / `profile_name: String` as of this commit. See
//! `docs/adr/0011-type-level-profile-fence.md` for the full accepted design and
//! `.factory/cycles/cycle-003/phase-f2-spec-evolution/adr-0011-amendment-staged.md` for the
//! F2-gate record this ADR amendment was applied from.
//!
//! # Infallible by design (ADR-0011 § Consequences, SR-017)
//!
//! `Profile::from(String)` performs NO validation. Any `String` — including an empty one,
//! or one that will never resolve to a real `config.toml` entry — constructs a `Profile`
//! without error. This is intentional: the newtype guarantees "this value was passed
//! through the profile-typed API," not "this value names a profile that actually exists."
//! Existence validation already happens elsewhere (config lookup by name) and stays there —
//! giving this constructor access to `Config` would be a layering violation (the newtype
//! lives below `Config`, not beside it). Do NOT add a validating `Profile::try_new(name,
//! cfg) -> Result<Profile>` — ADR-0011 explicitly rejects that shape for this type.

use std::fmt;

/// A validated-elsewhere, type-tagged profile name.
///
/// See the module docs above for the accepted design (ADR-0011) and what this newtype
/// deliberately does NOT guarantee (that the wrapped name resolves to a real profile).
///
/// # Compile-fail fence demonstration (AC-005, BC-6.2.015)
///
/// This is the Red Gate driver for BC-6.2.015's hard-fence guarantee: a function that
/// requires `&Profile` must reject a bare `&str` / hardcoded string literal at compile
/// time. The block below is a `compile_fail` doctest -- `cargo test --doc` treats a
/// doctest that FAILS TO COMPILE as a PASS. If this ever starts compiling, the fence
/// has regressed and BC-6.2.015 is broken.
///
/// This demonstrates the *pattern* the fence relies on (any `&Profile`-typed parameter
/// rejects a bare `&str`), independent of whether `src/cache.rs`'s and
/// `src/api/auth.rs`'s own functions have been swept to `&Profile` yet (AC-002/AC-003 --
/// a separate, mechanical call-site sweep not yet done as of this stub commit). The
/// newtype itself already enforces the fence for any caller that uses it, which is what
/// this doctest pins.
///
/// ```compile_fail
/// use jr::profile::Profile;
///
/// fn requires_profile(p: &Profile) -> String {
///     p.as_ref().to_string()
/// }
///
/// let hardcoded = "sandbox"; // a profile-unaware call site
/// requires_profile(hardcoded); // must NOT compile: `&str` does not coerce to `&Profile`
/// ```
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Profile(String);

impl fmt::Debug for Profile {
    /// Delegates to the wrapped `String`'s `Debug` (quoted, no `Profile(...)`
    /// tuple-struct wrapper) so every pre-existing `{:?}`-formatted error
    /// message (e.g. `"unknown profile: {active_profile_name:?}"`) renders
    /// byte-for-byte identically to before this newtype was threaded through —
    /// a derived `Debug` would print `Profile("name")` instead of `"name"`,
    /// which is a real behavior change this story's AC-006 forbids.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl From<String> for Profile {
    /// Wraps any `String` as a `Profile`, without validation (see module docs).
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl From<&str> for Profile {
    /// Ergonomic sibling of `From<String>` (also infallible, no validation) —
    /// lets call sites/tests write `"prod".into()` / `Profile::from("prod")`
    /// instead of `Profile::from("prod".to_string())`. Does not weaken the
    /// AC-005 fence: a bare `&str` still does not coerce to `&Profile` at a
    /// function boundary: this impl only helps *construct* an owned
    /// `Profile` value explicitly, the same as `From<String>` already did.
    fn from(name: &str) -> Self {
        Self(name.to_string())
    }
}

impl AsRef<str> for Profile {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for Profile {
    /// Lets existing test assertions keep comparing a `Profile` directly
    /// against a `&str`/string literal (`assert_eq!(cfg.active_profile_name,
    /// "prod")`) without call-site changes beyond construction — mirrors the
    /// ergonomic `From<&str>` impl above, not a validation relaxation.
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Profile {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl fmt::Display for Profile {
    /// Renders identically to the wrapped string — no bracket/quote decoration — so
    /// existing call sites that interpolate a profile name directly into format strings
    /// (error messages, cache-path joins, keychain key construction) see no behavior
    /// change once threaded through this type.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------------------
// AC-001 regression pins (BC-6.2.015, S-cycle3-adr0011-newtype).
//
// These tests pin `Profile`'s trait impls against the newtype that ALREADY exists in
// this file (landed by the stub-architect commit `023509db`) -- they are expected to
// PASS today, as regression pins for the AC-001 slice of BC-6.2.015 that is already
// implemented. They are NOT the Red Gate driver for the story as a whole: AC-002/003/004
// (threading `&Profile` through `src/cache.rs`, `src/api/auth.rs`,
// `Config::active_profile_name`, `JiraClient::profile_name`) is a separate, purely
// mechanical call-site sweep that the implementer step still owns, and no new test here
// depends on that sweep having happened. The `compile_fail` doctest on `Profile` above is
// this story's actual Red Gate driver for AC-005 -- see this file's doc comment on
// `Profile` for why a doctest was chosen over pulling in `trybuild` (not currently a
// dev-dependency) or a review-only verification note.
// ---------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bc_6_2_015_display_renders_identically_to_wrapped_string() {
        let raw = "sandbox";
        let p = Profile::from(raw.to_string());
        assert_eq!(p.to_string(), raw);
    }

    #[test]
    fn test_bc_6_2_015_display_and_as_ref_agree_for_various_inputs() {
        for raw in ["default", "prod", "sandbox-1", "team_2", ""] {
            let p = Profile::from(raw.to_string());
            assert_eq!(
                p.to_string(),
                raw,
                "Display must render identically to the wrapped string, no decoration"
            );
            assert_eq!(
                p.as_ref(),
                raw,
                "AsRef<str> must return the wrapped string verbatim"
            );
        }
    }

    #[test]
    fn test_bc_6_2_015_from_string_is_infallible_for_empty_string() {
        // AC-001 + ADR-0011 SR-017: From<String> performs NO validation -- an empty
        // string constructs a Profile without error (presence-not-correctness).
        let p = Profile::from(String::new());
        assert_eq!(p.as_ref(), "");
        assert_eq!(p.to_string(), "");
    }

    #[test]
    fn test_bc_6_2_015_from_string_is_infallible_for_non_existent_profile_name() {
        // EC-newtype-1 (staged ADR-0011 amendment): a Profile that names a profile
        // that will never resolve to a real config.toml entry still constructs
        // without error -- existence validation is deliberately NOT this type's job
        // (see ADR-0011 Consequences / SR-017; a validating `Profile::try_new` was
        // explicitly rejected).
        let p = Profile::from("this-profile-will-never-exist-in-any-config".to_string());
        assert_eq!(p.as_ref(), "this-profile-will-never-exist-in-any-config");
    }

    #[test]
    fn test_bc_6_2_015_display_has_no_bracket_or_quote_decoration() {
        let p = Profile::from("prod".to_string());
        let rendered = p.to_string();
        assert!(!rendered.contains('"'), "Display must not quote-decorate");
        assert!(!rendered.contains('['), "Display must not bracket-decorate");
        assert_eq!(rendered, "prod");
    }

    #[test]
    fn test_bc_6_2_015_clone_and_eq_preserve_wrapped_value() {
        // Regression pin for the derived Clone/PartialEq/Eq -- exercised throughout
        // cache.rs/auth.rs call sites once threaded (e.g. cache-key comparisons,
        // stale-heal retry guards keyed by profile identity).
        let a = Profile::from("prod".to_string());
        let b = a.clone();
        assert_eq!(a, b);
        assert_ne!(a, Profile::from("sandbox".to_string()));
    }

    #[test]
    fn test_bc_6_2_015_ord_matches_underlying_string_ord() {
        // Config/cache code may sort or dedupe profile lists; Ord must mirror
        // String's Ord since Profile is a plain newtype wrapper, not a
        // semantically-reordered type.
        let mut profiles = [
            Profile::from("sandbox".to_string()),
            Profile::from("default".to_string()),
            Profile::from("prod".to_string()),
        ];
        profiles.sort();
        let rendered: Vec<String> = profiles.iter().map(|p| p.to_string()).collect();
        assert_eq!(rendered, vec!["default", "prod", "sandbox"]);
    }
}
