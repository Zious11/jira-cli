# ADR-0007: Multi-Profile Fields Bug Fix Strategy

## Status
Accepted

## Context

A CRITICAL correctness bug was discovered during Phase 1 brownfield analysis: all 14
handler sites that read `story_points_field_id` and `team_field_id` read from
`config.global.fields.*` — the legacy flat config structure — even after multi-profile
support was introduced. The per-profile `ProfileConfig.story_points_field_id` and
`ProfileConfig.team_field_id` fields are written correctly by `jr init` and
`jr auth login`, but are never read by any handler.

**Impact:** In a sandbox-vs-production multi-profile setup, the custom field IDs from
whichever profile was configured first persist globally. The second profile silently uses
the wrong `customfield_NNNNN` IDs when listing issues with story points or team columns.
This is not a UI issue — it causes the wrong data to appear in issue lists and sprint
views.

**Affected sites (14 total across CLI handlers):**
- `src/cli/issue/list.rs` — story points and team field reads
- `src/cli/sprint.rs` — story points column
- `src/cli/board.rs` — board view columns
- `src/cli/issue/create.rs` — story points on create and edit
- Additional sites across CLI handlers

**Two options were considered:**

**Option A (chosen):** Route all 14 hot-path read sites through
`config.active_profile()` field reads — NO fallback to `global.fields` (fallback
rejected; see Rationale). `Config::active_profile()` returns the `ProfileConfig` for
the currently active profile; call sites read `.story_points_field_id` /
`.team_field_id` from it directly. Surface a `ConfigError` (exit 78) if the profile
lacks the field IDs.

**Option B (rejected):** Keep the current behavior, document it as a known limitation,
and defer until v2. A profile-aware workaround would require users to run `jr init` once
per profile switch.

## Decision

Use **Option A**: update all 14 call sites to read field IDs via
`config.active_profile().story_points_field_id` / `config.active_profile().team_field_id`
instead of `config.global.fields.*`. No new dedicated accessor is added —
`Config::active_profile()` (the existing per-profile getter in `src/config.rs`) is the
read path.

## Rationale

- The per-profile `ProfileConfig` fields already exist and are correctly populated by
  `jr init`. The bug is exclusively on the read side — 14 sites that bypass the
  per-profile path.
- Routing through `active_profile()` centralizes the read logic with no fallback in one
  place. Future additions are automatically correct.
- The fallback to `global.fields.*` was explicitly rejected: `Config::save_global()`
  drops the `[fields]` block from disk via `#[serde(default, skip_serializing)]`. The
  fallback target does not exist post-save, making fallback a silent no-op at best and
  misleading at worst.
- Option B is not viable. CLAUDE.md explicitly states cross-profile cache leakage is "a
  correctness bug, not a UX issue." A CRITICAL-severity correctness bug cannot be
  deferred.

## Consequences

- **Fix scope:** ~30–40 lines changed across 6+ files. All changes are read-site
  replacements of `config.global.fields.*` with
  `config.active_profile().story_points_field_id` /
  `config.active_profile().team_field_id`.
- **Regression risk:** LOW. The existing behavior is incorrect for multi-profile users.
  The integration test added to `tests/auth_profiles.rs` must verify per-profile field
  isolation explicitly.
- **Migration compatibility:** `Config::active_profile()` reads the active
  `ProfileConfig` directly. There is NO fallback to `global.fields.*`. If
  `[profiles.<name>]` lacks the field IDs AND `[fields]` is also absent (post-save
  state), the caller surfaces: `"Custom field IDs not configured for profile '<name>'.
  Run 'jr init' to configure."` (exit 78, `ConfigError`).
- **BC anchor:** BC-6.3.001.

## See Also

- ADR-0006 — Embedded OAuth app (multi-profile context; profiles introduced alongside)
- `src/config.rs::Config::active_profile` — the per-profile getter used at all read sites
- `tests/auth_profiles.rs` — per-profile field isolation integration tests
