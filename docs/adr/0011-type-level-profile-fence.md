# ADR-0011: Type-Level Profile Fence (Newtype)

## Status
**Accepted** (amended 2026-09-01, cycle-003 `auth-profile-dx`, DEC-317 — un-defers this ADR
in place; this is a status amendment, not a supersession, since the underlying decision does
not reverse, it confirms a documented revisit trigger was met). Originally **Deferred**
(promoted to `docs/adr/` 2026-06-24, PR #549/SC-03; the deferral itself predates the VSDD-factory
migration — see git history for the pre-promotion origin).

**Trigger met:** Condition for Revisiting #3 below — "a related refactor (e.g., a major config
overhaul) creates a natural migration window." Cycle-003's per-profile credential restructuring
(DEC-315: shared flat `email`/`api-token` keychain keys become `<profile>:email`/
`<profile>:api-token`, symmetric with the existing per-profile OAuth token pair) is exactly that
window: it is itself a `ProfileConfig`/keychain-scoping change that touches nearly every
call site this newtype would guard, so implementing the hard fence in the SAME cycle (sequenced
AFTER the credential restructuring lands — see the new combined ADR, `.factory/specs/architecture/decisions/ADR-0020-per-profile-credential-ownership-env-tagging-and-oauth-default-at-creation.md`,
§ Sequencing) means the call-site sweep covers the enlarged, post-restructuring surface exactly
once rather than twice. Condition #1 (a leakage bug in production) and Condition #2 (>5
committers) remain NOT met — DEC-317 explicitly cites #3 alone as sufficient.

> **F2-gate revision (same day, 2026-09-01):** this staged amendment was revised in place to
> resolve F2-gate adversarial findings SR-006 (newtype scope contradiction with
> architecture-delta §4 — `src/api/auth.rs`'s credential functions are now explicitly IN
> SCOPE, § Decision item 2's sub-bullet) and SR-017 (constructor rationale — see §
> Consequences' new bullet on the infallible `From<String>` constructor). The call-site
> estimate is corrected from ~50-70 to ~60-80 accordingly. No separate "v2" file exists — this
> document is the current, single source of truth.

> **Design decision, not yet a completed migration.** This amendment records the ACCEPTED
> decision to implement the hard fence. The actual `Profile(String)` newtype and its ~60-80
> call-site threading through `src/cache.rs`, `Config::active_profile_name`,
> `JiraClient::profile_name`, and `src/api/auth.rs`'s credential functions
> (`store_api_token`/`load_api_token`/`store_oauth_tokens`/`load_oauth_tokens`) are an
> **F4 implementation deliverable** of cycle-003 (story `S-cycle3-adr0011-newtype`). As of the
> STUB step of that story landing (this commit), the `Profile` newtype itself exists in
> `src/profile.rs`, but the mechanical call-site sweep has NOT started: `src/cache.rs`,
> `src/config.rs`, `src/api/client.rs`, and `src/api/auth.rs` still carry `profile: &str` /
> `profile_name: String` as of this writing — that sweep is the implementer step's TDD work.

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

**Accepted (amended).** Un-defer the type-level hard fence. Introduce a `Profile(String)`
newtype and thread it through every per-profile boundary the soft fence today protects by
convention only:

1. `pub struct Profile(String)` with `impl From<String> for Profile`, `impl AsRef<str> for
   Profile`, and a `Display` impl (call sites that currently interpolate `profile: &str`
   directly into format strings — error messages, cache-path joins, keychain key
   construction — must keep working without a wrapper-visible behavior change).
2. Every `cache::{read_*,write_*,clear_*,invalidate_*}` function signature changes
   `profile: &str` → `profile: &Profile` (16 functions in `src/cache.rs` as of this
   writing — re-verified 2026-09-02 against the post-restructuring surface, up from the
   original ~12+ estimate; the exact count is whatever `src/cache.rs` has grown to by the
   time the implementer step runs).
   - **`src/api/auth.rs`'s per-profile credential functions are IN SCOPE for the fence, on
     equal footing with the `cache.rs` functions above** (added at the F2 gate — resolves
     adversarial finding SR-006, a genuine contradiction between this document's original
     Decision enumeration, which omitted `auth.rs` entirely, and
     `.factory/cycles/cycle-003/phase-f2-spec-evolution/architecture-delta.md` §4, which
     already showed `auth.rs`'s credential functions inside the fence diagram). Every one of
     `store_api_token(profile, …)`, `load_api_token(profile)`, `store_oauth_tokens(profile,
     …)`, and `load_oauth_tokens(profile)` (all four, `src/api/auth.rs`, this cycle's own
     DEC-315 work per ADR-0020 § Decision 1) changes `profile: &str` → `profile: &Profile`.
     These are not an afterthought inclusion: they take a `profile` parameter and ARE the
     exact credential-isolation seam this hard fence is built to protect — a wrong-profile
     `&str` silently passed to `store_api_token`/`load_oauth_tokens` is precisely a
     cross-environment credential leak, the single worst-case failure mode this whole ADR
     exists to make uncompilable. Excluding them from the fence while including
     `cache::read_teams_cache` (data-only, lower stakes than a credential) would have been
     internally incoherent. `clear_profile_creds`/`clear_all_credentials`'s aggregation loops
     (same file) are included as downstream callers, not separately-typed functions.
3. `Config::active_profile_name: String` → `Profile`.
4. `JiraClient::profile_name: String` → `Profile`.
5. Fix all call sites — ADR-0011's original estimate was "~50-70 changes," scoped to a
   `cache.rs`-only sweep at pre-cycle-003 file size. DEC-317's own rationale for
   un-deferring THIS cycle is that DEC-315's credential normalization "multiplies
   cross-profile scoping call-sites," and item 2's `src/api/auth.rs` addition above is
   exactly that multiplication made concrete: adding `src/api/auth.rs`'s four per-profile
   credential functions plus their
   call sites (`JiraClient::load_auth_from_keychain`'s two branches, `login_token`,
   `clear_profile_creds`/`clear_all_credentials`'s aggregation loops, `auth remove`'s fourth
   delete step from ADR-0020 § Decision 7, and the `auth refresh`/`auth login` call sites
   reading these functions — roughly 8-12 additional call sites) revises the estimate to
   **~60-80 changes** (corrected at the F2 gate from the original ~50-70; see
   architecture-delta §4 for the reconciliation). Re-measured 2026-09-02 at the start of the
   implementation story: `src/cache.rs` has 16 of 26 `pub fn`s taking `profile: &str`;
   `src/api/auth.rs` has 17 occurrences of `profile: &str` across its credential functions
   and their internal call sites — both consistent with the ~60-80 estimate once
   `Config`/`JiraClient` and their remaining `src/cli/**` callers are included; the estimate
   is NOT revised further at this time.

**Sequencing (binding on the F4 implementation story, not just a suggestion):** the newtype
call-site sweep is sequenced to land AFTER cycle-003's per-profile credential-storage and
migration stories (`S-cycle3-percred-storage`, `S-cycle3-percred-migration`) are stable — see
`.factory/specs/architecture/decisions/ADR-0020-per-profile-credential-ownership-env-tagging-and-oauth-default-at-creation.md`
§ Sequencing for the full cross-story ordering. Landing the newtype first would mean sweeping
the call-site surface once, then re-sweeping it again once the credential restructuring adds
new per-profile call sites — the same rework this ADR's Condition #3 exists to avoid.

This is a **pure Rust type-level change with zero on-disk, keychain, or wire-format impact**
(§4.5 of the F1 delta analysis, `.factory/cycles/cycle-003/phase-f1-delta-analysis/delta-analysis.md`).
No data migration, no cache-root version bump, no keychain-namespace change. All risk is
mechanical (a large, compiler-checked call-site diff), not behavioral.

## Conditions for Revisiting (historical — retained for record)

This decision was to be revisited in v0.6.0 or later if any of the following occurred:
1. A cache cross-profile leakage bug is discovered in a released version (i.e., the soft
   fence fails in practice) — **NOT met.** No such bug has been reported as of this
   amendment.
2. The contributor count grows beyond ~5 active committers (convention enforcement weakens
   with team size) — **NOT met.**
3. A related refactor (e.g., a major config overhaul) creates a natural migration window —
   **MET.** Cycle-003 (`auth-profile-dx`, DEC-312..319) is that refactor; see Status above.

## Consequences

### Positive
- Closes NFR-SCA-2 (soft-fence, previously `DEFER` in `nfr-catalog.md`) — the F2 PRD-supplement
  pass tracks this NFR's status change alongside this ADR (F1 delta analysis §1.1/§1.5; not
  edited by this architecture pass).
- A profile-unaware cache reader, or a hardcoded-string call site, becomes a compile error
  instead of a silent cross-profile leakage risk — the exact failure mode ADR-0007
  (multi-profile fields bug) demonstrated was reachable under the soft-fence convention alone.
- The compiler becomes the primary regression safety net for cross-profile isolation going
  forward, superseding "code review is the enforcement gate" as the sole control.

### Negative / Trade-offs
- Large, mechanical diff (**~60-80 call sites**, corrected at the F2 gate from the original
  ~50-70 — see Decision §5's `src/api/auth.rs` scope addition above) — reviewable primarily
  by "does it compile and do existing cross-profile isolation tests still pass," not by
  manual per-site correctness reasoning (a WRONG-but-compiling `Profile` value substitution
  is not caught by the type system alone — F1 delta analysis §3 "Cross-profile cache leakage
  during ADR-0011 newtype threading," classified MEDIUM-mechanical-churn /
  LOW-post-landing risk).
- Adds `.0`/`AsRef<str>` friction at call sites that previously took a bare `&str` — accepted
  as the intended cost of the hard fence (see the original trade-off table above, unchanged).
- Interop with `Config`/`JiraClient` requires updating their field types in the same change —
  not a standalone `cache.rs`-only patch.
- **Infallible `From<String>` constructor was chosen over a validating `Profile::new() ->
  Result` (resolves adversarial finding SR-017).** `impl From<String> for Profile` (Decision
  §1) performs no validation — any `String`, including an empty one or one containing
  characters that would never resolve to a real config entry, constructs a `Profile` without
  error. This is a **presence-not-correctness** residual: the newtype guarantees "this value
  was passed through the profile-typed API," not "this value names a profile that actually
  exists in `config.toml`." That existence check already happens elsewhere (config lookup by
  name, which returns `None`/an error for an unknown profile) and is unaffected by this ADR —
  duplicating it inside `Profile::from` would mean either giving the constructor access to
  `Config` (a layering violation — the newtype lives below `Config`, not beside it) or
  re-validating against a data source the constructor doesn't otherwise need. A validating
  `Profile::try_new(name: &str, cfg: &Config) -> Result<Profile>` was considered as an
  alternative and is explicitly NOT adopted here: it would couple the newtype's construction
  to `Config`'s shape, working against the goal of a minimal, dependency-free wrapper type,
  and every existing call site already validates profile existence at a higher layer (CLI
  arg parsing / config resolution) before a `Profile` value would ever need to be
  constructed. The infallible constructor is therefore the correct scope for THIS ADR — it
  closes the cross-profile-leakage class of bug (wrong profile, right shape), not the
  invalid-profile-name class of bug (right profile, but the name doesn't exist), which
  remains a separate, already-handled concern. Left as a documented residual, not a gap: a
  future ADR could add a validating constructor as a strictly additive change without
  touching this one's decision.

### Status as of this amendment (2026-09-02, cycle-003 F4 implementer step — COMPLETE)
**Accepted; newtype scaffolding AND the call-site sweep have both landed.** `src/profile.rs`
defines `Profile(String)` with `From<String>`/`From<&str>` (an ergonomic sibling added
during the sweep, see Consequences below)/`AsRef<str>`/`Display`/`PartialEq<str>`/
`PartialEq<&str>`/a hand-written `Debug` (delegating to the wrapped `String`'s `Debug` so
existing `{:?}`-formatted error messages render byte-for-byte unchanged) impls.

Every `src/cache.rs` per-profile function (26 of 26, both `pub` and internal), all of
`src/api/auth.rs`'s in-scope credential/aggregation functions
(`store_api_token`/`load_api_token`/`store_oauth_tokens`/`load_oauth_tokens`/
`clear_profile_creds`/`clear_profile_oauth_pair`/`clear_all_credentials`),
`Config::active_profile_name`, and `JiraClient::profile_name`/`profile_name()` are now
`&Profile`/`Profile`-typed, per Decision items 1-4 above. The sweep also threaded `&Profile`
through the pass-through wrapper functions the fenced functions are called from
(`cli/issue/field_resolve.rs::resolve_edit_fields`, `cli/requesttype.rs::{handle_list,
handle_fields, resolve_request_type_id}`, `cli/issue/jsm_create.rs::resolve_jsm_request_type_id`,
`cli/field.rs::resolve_field_id`, `api/client.rs::load_auth_from_keychain`) so the fence
reaches every real call site, not just the immediately-named functions — everywhere else
(e.g. `oauth_login`, `refresh_oauth_token[_with_url]`, the private
`oauth_access_key`/`api_token_email_key`/etc. keychain-key builders, and CLI-layer display
helpers like `auth::list::render_list_table`) intentionally stays `&str`-typed and
constructs a `Profile` at the boundary immediately before calling into a fenced function
(`&Profile::from(profile)` / `.as_ref()` the other direction) — the minimal-footprint
reading of "thread `&Profile` through every remaining call site" the story's File Structure
Requirements table calls for, not a blanket rewrite of every `profile`-adjacent parameter in
the codebase.

**Re-measured actual scope vs. the ~60-80 estimate above:** the real diff is 22 files
changed (~583 insertions / ~356 deletions across `src/` + `tests/`), comprising 38 function
signatures converted to `&Profile`/`&crate::profile::Profile` and ~259
`Profile::from(...)`/`&Profile::from(...)` boundary constructions across production code
and test call sites (plus ~18 further `.as_ref()`/`.to_string()` adaptations at
`active_profile_name` read sites that don't construct a new `Profile`). The pre-implementation
~60-80 estimate undercounted materially — it was scoped to distinct *call sites needing a
type change*, whereas the actual mechanical diff also touches every literal-`&str`-argument
test call site across `src/cache.rs`'s and `src/api/auth.rs`'s large inline `#[cfg(test)]`
modules (the single largest contributor: cache.rs alone has well over 100 such test call
sites once its request-type/component/object-type-attr test coverage is included) and the
five integration-test files (`tests/api_token_percred_wiring.rs`,
`tests/auth_remove_logout_semantics.rs`, `tests/oauth_refresh_integration.rs`,
`tests/issue_edit_field.rs`, `tests/worklog_duration_holdouts.rs`) exercising these
functions with hardcoded profile-name literals. None of this changed the SHAPE of the sweep
(still purely mechanical, compiler-checked, zero behavior change) — only its literal size.
This paragraph supersedes the ~60-80 figure quoted earlier in this document for the purpose
of understanding actual PR diff size; the earlier figure is left in place elsewhere as a
historical record of the pre-implementation estimate, not corrected retroactively.

## See Also

- `src/profile.rs` — the `Profile(String)` newtype scaffold (this story's stub step)
- `src/cache.rs` — all per-profile cache read/write functions (soft-fence convention today;
  hard-fence target per this amendment)
- `src/config.rs::Config` — `active_profile_name` field
- `src/api/client.rs::JiraClient` — `profile_name` field and `profile_name()` accessor
- ADR-0007 — Multi-profile fields fix (parallel profile-correctness decision; the concrete
  bug class this hard fence is designed to make uncompilable)
- `.factory/specs/architecture/decisions/ADR-0020-per-profile-credential-ownership-env-tagging-and-oauth-default-at-creation.md`
  — the DEC-315 credential-restructuring ADR whose call-site growth is this amendment's
  stated trigger, and which this ADR's implementation is sequenced after
- `.factory/cycles/cycle-003/phase-f1-delta-analysis/delta-analysis.md` §1.1, §1.3, §1.5,
  §3, §4.5 — the impact analysis this amendment is grounded in
- `.factory/architecture/risk-register.md` R-L1 — cites this ADR's (formerly Deferred) status;
  flagged for a follow-up update in `.factory/cycles/cycle-003/phase-f2-spec-evolution/architecture-delta.md`
  § Flagged Follow-Ups
