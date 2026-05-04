# ADR-0007: Multi-Profile Fields Bug Fix Strategy

## Status
Accepted

## Context

A CRITICAL correctness bug was discovered during Phase 1 brownfield analysis (NFR-R-D, BC-6.3.001): all 12+ handler sites that read `story_points_field_id` and `team_field_id` read from `config.global.fields.*` — the legacy flat config structure — even after multi-profile support was introduced. The per-profile `ProfileConfig.story_points_field_id` and `ProfileConfig.team_field_id` fields are written correctly by `jr init` and `jr auth login`, but are never read by any handler.

**Impact:** In a sandbox-vs-production multi-profile setup, the custom field IDs from whichever profile was configured first persist globally. The second profile silently uses the wrong `customfield_NNNNN` IDs when listing issues with story points or team columns. This is not a UI issue — it causes the wrong data to appear in issue lists and sprint views.

**12+ affected sites (Pass 4 R1):**
- `src/cli/issue/list.rs:147-148`
- `src/cli/sprint.rs:232-233`
- `src/cli/board.rs:192-193`
- `src/cli/issue/create.rs:128,277,283`
- (7+ additional sites across CLI handlers)

**Two options were considered:**

**Option A (Recommended):** Add a `Config::field_id(FieldKind, profile)` accessor that reads from `active_profile()` first, falling back to `global.fields` for migration compatibility. Route all 12+ sites through this accessor. Add an integration test in `tests/auth_profiles.rs` to enforce per-profile isolation.

**Option B:** Keep the current behavior, document it as a known limitation, and defer until v2. A profile-aware workaround would require users to run `jr init` once per profile switch.

## Decision

Use **Option A**: add a single `Config::field_id(FieldKind, profile)` accessor and update all 12+ call sites.

## Rationale

- The per-profile `ProfileConfig` fields already exist and are correctly populated by `jr init`. The bug is exclusively on the read side — 12+ sites that bypass the per-profile path.
- A single accessor centralizes the read logic, including the legacy fallback, in one place. Future additions are automatically correct.
- The fix surface is well-defined: replace `config.global.fields.story_points_field_id` and `config.global.fields.team_field_id` reads with `config.field_id(FieldKind::StoryPoints, &config.active_profile_name)` and `config.field_id(FieldKind::Team, &config.active_profile_name)` at all 12+ sites.
- Option B is not viable: CLAUDE.md explicitly states cross-profile leakage is "a correctness bug, not a UX issue." A CRITICAL-severity correctness bug cannot be deferred.

## Consequences

- **Fix scope:** ~30–40 lines changed across 6+ files. Moderate scope but well-bounded — all changes are read-site replacements.
- **Regression risk:** LOW. The existing behavior is incorrect for multi-profile users; any profile-aware test that currently passes is testing the wrong field. The integration test added to `tests/auth_profiles.rs` must verify per-profile field isolation explicitly.
- **Migration compatibility:** `Config::field_id()` reads `active_profile().story_points_field_id` / `active_profile().team_field_id` directly. There is NO fallback to `global.fields.*`: BC-6.3.001 confirms that `Config::save_global()` drops the `[fields]` block from disk via `#[serde(default, skip_serializing)]`, so the fallback target does not exist post-save. If `[profiles.<name>]` lacks the field IDs AND `[fields]` is also absent (post-save state), `Config::field_id()` returns `None` and the caller surfaces: `"Custom field IDs not configured for profile '<name>'. Run 'jr init' to configure."` (exit 78, `ConfigError`).
- **BC anchor:** BC-6.3.001 (MUST-FIX forward-looking spec).

## References

- NFR-R-D (nfr-catalog.md)
- BC-6.3.001 (bc-6-config-cache.md)
- Pass 4 R1 finding (jira-cli-pass-4-deep-r1.md)
- risk-register.md §R-C1
