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
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Profile(String);

impl From<String> for Profile {
    /// Wraps any `String` as a `Profile`, without validation (see module docs).
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl AsRef<str> for Profile {
    fn as_ref(&self) -> &str {
        &self.0
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
