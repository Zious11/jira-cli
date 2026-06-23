# ADR-0011: Type-Level Profile Fence (Newtype) — Deferred

## Status
Deferred — Target version: v0.6.0 or later

## Context

Per-profile cache isolation is a critical correctness invariant in `jr`. Every cache
reader and writer takes `profile: &str` as its first argument. `JiraClient` carries
`profile_name: String` and exposes `profile_name()` for modules that have a client but
not a config.

This is a **convention-enforced (soft-fence) boundary.** There is no compile-time
enforcement preventing a future contributor from:
- Adding a new cache-reading function that does not take a `profile` parameter
- Calling `cache::read_*` with a hardcoded string instead of the active profile name
- Adding a new resource impl that fetches and stores data without the profile qualifier

**Newtype proposal:** Introduce a `Profile(String)` newtype that would make
profile-unaware cache calls a compile error:

```rust
// Current (soft fence — compiles but silently wrong)
pub fn read_teams_cache(profile: &str) -> Result<Option<Vec<TeamEntry>>> { ... }

// Proposed (hard fence — profile must be an explicit Profile wrapper)
pub fn read_teams_cache(profile: &Profile) -> Result<Option<Vec<TeamEntry>>> { ... }
```

**Trade-off summary:**

| Aspect | Newtype (hard fence) | Current convention (soft fence) |
|--------|---------------------|--------------------------------|
| Compile-time safety | Yes — wrong profile type doesn't compile | No — any `&str` accepted |
| Refactoring scope | Large — all 12+ cache fns + all callers must change type | Zero |
| Code verbosity | Adds `.0` dereferences and `Profile::from` coercions | Cleaner call sites |
| Interop with Config | `active_profile_name` would change from `String` to `Profile` | No change |
| Discovery cost | New cache fn callers are guided by the type | Contributor may accidentally omit the profile arg |

## Decision

**Deferred.** The convention-enforced approach is sufficient for v0.5.x. A full audit
found 100% conformance across all cache read/write sites — every site passes the active
profile name explicitly. The explicit CLAUDE.md gotcha documentation provides adequate
protection for the current team size.

## Conditions for Revisiting

This decision should be revisited in v0.6.0 or later if any of the following occur:
1. A cache cross-profile leakage bug is discovered in a released version (i.e., the soft
   fence fails in practice)
2. The contributor count grows beyond ~5 active committers (convention enforcement weakens
   with team size)
3. A related refactor (e.g., a major config overhaul) creates a natural migration window

## Consequences

- The soft fence remains the correctness boundary. The CLAUDE.md Gotchas section is the
  primary enforcement mechanism ("Multi-profile boundary: every cache reader/writer takes
  `profile: &str` as its first arg").
- Any new cache function added must follow the `profile: &str` first-arg convention. Code
  review is the enforcement gate.
- If this ADR is eventually un-deferred, the implementation would be:
  1. Add `pub struct Profile(String)` with `impl From<String> for Profile` and
     `impl AsRef<str> for Profile`
  2. Update all `cache::{read_*,write_*,clear_*}` signatures to take `profile: &Profile`
  3. Update `Config.active_profile_name: String` → `Profile`
  4. Update `JiraClient.profile_name: String` → `Profile`
  5. Fix all call sites (estimated ~50–70 changes)
- The open scalability NFR (soft-fence per-profile cache isolation) remains tracked in the factory NFR catalog.

## See Also

- `src/cache.rs` — all per-profile cache read/write functions (soft-fence convention)
- `src/config.rs::Config` — `active_profile_name` field
- `src/api/client.rs::JiraClient` — `profile_name` field and `profile_name()` accessor
- ADR-0007 — Multi-profile fields fix (parallel profile-correctness decision)
