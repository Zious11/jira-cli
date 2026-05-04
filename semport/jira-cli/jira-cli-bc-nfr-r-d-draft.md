# Draft Behavioral Contract: BC-NFR-R-D-001 (multi-profile fields silent regression)

## Status

DRAFT — pending product-owner formalization in Phase 1 PRD.

## Severity

**CRITICAL (P0 correctness bug, hot-path)** — per Pass 4 R4. Fires on every `jr issue list`, `jr issue view`, `jr issue create`, `jr issue edit`, `jr sprint current`, `jr sprint list`, `jr board view`, not edge-case.

## Anchors

- **NEW-INV-12** (Pass 2 R1) — first observation that legacy `[fields]` block is read at runtime but skipped on save
- **NEW-INV-143** (Pass 2 R3) — expanded enumeration of read sites; verified migration drains the in-memory legacy block but on-disk persistence is one-way
- **NFR-R-D** (Pass 4 R1/R2/R3/R4) — final "Reliability — Deferred / non-functional regression" framing; Pass 4 R4 escalated to CRITICAL
- **Cross-references in Pass 8 deep synthesis** — listed under "Critical Design Risks" / spec crystallization recommendations

## Description

After legacy-config migration, every runtime read of `story_points_field_id` and `team_field_id` still goes through `config.global.fields.*` (the legacy `[fields]` block) instead of `config.active_profile().{story_points,team}_field_id` (the migration target). Because `GlobalConfig::fields` is annotated `#[serde(default, skip_serializing)]` (`src/config.rs:43-48`), the legacy block is **read** from `config.toml` but never **written** back. The first invocation that triggers `Config::save_global()` (e.g., `jr auth login`, `jr auth switch`, `jr init`) drops the `[fields]` block from disk. From that point forward all 11 production read sites observe `None` and the points/team columns silently disappear, despite the migrated `[profiles.<name>].{story_points,team}_field_id` values being correctly populated and present on disk.

This is a **silent functional regression with no error surface**: the columns just go missing, no warning, no exit-code change.

## Source citations

### Migration logic (correct — populates per-profile target)

- `src/config.rs:11-14` — `FieldsConfig` struct (legacy block)
- `src/config.rs:17-25` — `ProfileConfig` struct (migration target; has `team_field_id` + `story_points_field_id`)
- `src/config.rs:43-48` — **THE BUG SITE**: `#[serde(default, skip_serializing)]` on legacy `instance` and `fields`. They are read but not persisted.
- `src/config.rs:142-178` — `migrate_legacy_global()` correctly copies `global.fields.{team,story_points}_field_id` → `ProfileConfig.{team,story_points}_field_id` and inserts under `profiles["default"]`
- `src/config.rs:148` — Embedded comment confirming the bug-by-design state: *"Task 16 stops serializing the legacy fields, so they fall off disk on the next save."* This is the gotcha — Tasks 7/8 (the read-site migrations referenced on line 147) were never completed.
- `src/config.rs:240-259` — `load_inner` triggers migration write-back on first load when needed; but write-back only persists `[profiles.*]`, not `[fields]`
- `src/config.rs:387-393` — `Config::active_profile()` — the correct read target (returns cloned `ProfileConfig`)
- `src/config.rs:396-414` — `Config::active_profile_or_err()` — strict variant

### Read sites (the 11 hot-path bugs)

| # | File:Line | Field read | Used for |
|---|-----------|------------|----------|
| 1 | `src/cli/issue/list.rs:147` | `story_points_field_id` | `--points` column on `jr issue list` |
| 2 | `src/cli/issue/list.rs:148` | `team_field_id` | Team column gating + `extra` field on issue search |
| 3 | `src/cli/issue/view.rs:28` | `story_points_field_id` | Single-issue points display via `compose_extra_fields` (also reads it transitively) |
| 4 | `src/cli/issue/view.rs:29` | `team_field_id` | Single-issue team display |
| 5 | `src/cli/issue/helpers.rs:43` | `team_field_id` | `resolve_team_field()` short-circuit before `find_team_field_id()` API fallback |
| 6 | `src/cli/issue/helpers.rs:194` | `story_points_field_id` | `compose_extra_fields()` — used by view + create |
| 7 | `src/cli/issue/helpers.rs:200` | `team_field_id` | `compose_extra_fields()` |
| 8 | `src/cli/issue/helpers.rs:209` | `story_points_field_id` | `resolve_story_points_field_id()` — used by `jr issue create --points` and `jr issue edit --points` (3 call sites in `create.rs:128, 277, 283`) |
| 9 | `src/cli/sprint.rs:232` | `story_points_field_id` | Sprint issue points column |
| 10 | `src/cli/sprint.rs:233` | `team_field_id` | Sprint issue team column |
| 11 | `src/cli/board.rs:192` | `team_field_id` | Board view team column gating |

Plus a test fixture site that mocks the bug and must be updated alongside the fix:

- `src/cli/issue/helpers.rs:777-778` — unit test for `compose_extra_fields` writes to `config.global.fields.*` directly, will need to write to `config.global.profiles[active].* _field_id` instead

### Write sites (correctly write per-profile — proves migration is one-sided)

- `src/cli/init.rs:201` — sets `entry.team_field_id` under `profiles[active]`
- `src/cli/init.rs:244` — sets `entry.story_points_field_id` under `profiles[active]`
- (No code anywhere writes `config.global.fields.*` other than migration test fixtures.)

## Expected behavior (BC contract)

> After legacy config migration, every runtime read of `story_points_field_id` and `team_field_id` MUST come from `config.active_profile().{story_points,team}_field_id` (or the strict variant `config.active_profile_or_err()?.{...}`), NOT from `config.global.fields.*`. The legacy `[fields]` block must be treated as read-only-historical, consulted only by `migrate_legacy_global()` during migration. Once migration has run, the per-profile fields are the sole source of truth for these custom-field IDs, and they MUST round-trip correctly across `Config::save_global()` calls so subsequent invocations observe the same field IDs.

**Round-trip invariant (the holdout assertion):**

```text
For all profiles P and all field-id pairs (sp, team) where:
  - config.global.profiles[P].story_points_field_id == Some(sp)
  - config.global.profiles[P].team_field_id == Some(team)

After config.save_global() followed by Config::load_with(Some(P)):
  - config.active_profile().story_points_field_id MUST == Some(sp)
  - config.active_profile().team_field_id MUST == Some(team)

AND every read site enumerated in the source-citations table above MUST observe the SAME (sp, team) values that the active profile reports — not None, not the legacy `[fields]` value (which no longer exists on disk).
```

## Current behavior (the bug, verbatim from source)

```rust
// src/config.rs:43-48
#[serde(default, skip_serializing)]
pub instance: InstanceConfig,

/// Legacy global custom-field IDs — read for migration only.
#[serde(default, skip_serializing)]
pub fields: FieldsConfig,
```

```rust
// src/config.rs:148 — embedded acknowledgement of the bug-by-design state
// Task 16 stops serializing the legacy fields, so they fall off disk on the
// next save.
```

```rust
// src/cli/issue/list.rs:147-148 — STILL reads from legacy block
let sp_field_id = config.global.fields.story_points_field_id.as_deref();
let team_field_id = config.global.fields.team_field_id.as_deref();
```

The migration block-comment at `src/config.rs:142-149` explicitly notes:

> *"Legacy fields are intentionally preserved during the transition (Tasks 4-15) so callers that still read `global.instance.*` / `global.fields.*` keep working until **Tasks 7/8 migrate them to read `active_profile()` instead**. Task 16 stops serializing the legacy fields, so they fall off disk on the next save."*

**Tasks 7/8 were never completed.** The migration is one-sided — write-back populates `[profiles.<name>]` but reads were never moved off `[fields]`. Result: the legacy `[fields]` block is now a lit fuse — present on disk for legacy users, drained the moment any save occurs, leaving every read at `None`.

## User-visible symptoms

After the first save-triggering command runs against a legacy config:

1. **`jr issue list --points`** — story_points column shows blank for every row (was: numeric points value)
2. **`jr issue list`** (default columns) — Team column disappears entirely from the rendered table even when issues have populated team UUIDs (column is gated on `team_field_id.is_some()`)
3. **`jr issue view <KEY>`** — points and team fields drop off the rendered detail panel
4. **`jr sprint current`** — points + team columns missing from sprint issue table
5. **`jr sprint list`** — points + team columns missing
6. **`jr board view`** — team column missing
7. **`jr issue create --points 5`** — fails with `JrError::ConfigError("Story points field not configured. Run \"jr init\" or set story_points_field_id under [fields] in ~/.config/jr/config.toml")` (the suggested-fix message itself points users back at the broken legacy block)
8. **`jr issue edit --points 5`** — same failure as create
9. **`jr issue create --team <name>`** — `resolve_team_field()` falls through to the API discovery branch (`client.find_team_field_id().await`); works but adds a network round-trip and may fail with `"No \"Team\" field found on this Jira instance"` even when the team field IS configured per-profile

**Multi-profile-specific severity multiplier:** Users with multiple profiles (sandbox + prod) where each Jira site has different custom-field IDs (`customfield_10016` vs `customfield_10042`, etc.) will silently lose ALL multi-profile correctness. The symptom is identical to single-profile users (column blank), but the impact is more dangerous: even if a user manually re-adds `[fields]` to repair the visible breakage, they'll be using the WRONG field IDs on whichever profile didn't match the legacy block.

## Reproduction

**Setup:**

```bash
# Use a v0.5.0-dev.7 jr binary on a host with a legacy-shaped ~/.config/jr/config.toml:
cat > ~/.config/jr/config.toml <<'EOF'
[instance]
url = "https://example.atlassian.net"
auth_method = "oauth"

[fields]
story_points_field_id = "customfield_10016"
team_field_id = "customfield_10001"
EOF
```

**Steps:**

1. `jr issue list --points` — observe story_points column populated (works, reads `[fields]`)
2. `jr issue view <KEY>` — observe team field present (works)
3. Run any command that triggers `Config::save_global()`. Easiest: `jr auth switch default` (no-op switch still saves) or `jr auth login` or `jr init`. Any save path performs the migration write-back.
4. `cat ~/.config/jr/config.toml` — observe `[fields]` block has DISAPPEARED. New shape:

   ```toml
   default_profile = "default"

   [defaults]
   output = "table"

   [profiles.default]
   url = "https://example.atlassian.net"
   auth_method = "oauth"
   team_field_id = "customfield_10001"
   story_points_field_id = "customfield_10016"
   ```
   (Legacy `[instance]` and `[fields]` correctly migrated INTO `[profiles.default]`.)

5. `jr issue list --points` — story_points column NOW BLANK, team column GONE.
6. `jr issue create --points 3` — exits with the legacy-block error message.

**Time-to-failure:** ZERO commands after the first save. Worst case: a user runs `jr auth login` to refresh OAuth, immediately runs `jr issue list --points`, sees the regression — and has no diagnostic linking the two.

## Recommended fix scope

**Pattern:** Replace every `config.global.fields.X` read with `config.active_profile().X` (or `config.active_profile_or_err()?.X` where errors should be hard-surfaced). Helpers should accept `&ProfileConfig` directly to avoid repeated lookups.

### Per-file Edit map (Phase 2 story anchor)

| File:Line | Old | New |
|-----------|-----|-----|
| `src/cli/issue/list.rs:147` | `config.global.fields.story_points_field_id.as_deref()` | `config.active_profile().story_points_field_id.as_deref().map(str::to_owned)` (or pre-bind `let profile = config.active_profile();` once at top of function) |
| `src/cli/issue/list.rs:148` | `config.global.fields.team_field_id.as_deref()` | (same pattern, team_field_id) |
| `src/cli/issue/view.rs:28-29` | (same legacy reads) | (same fix) |
| `src/cli/issue/helpers.rs:43` | `&config.global.fields.team_field_id` | `&config.active_profile().team_field_id` (note: function already has `&Config`) |
| `src/cli/issue/helpers.rs:194,200` | `config.global.fields.{sp,team}_field_id.as_deref()` | per-profile equivalent |
| `src/cli/issue/helpers.rs:206-217` | `resolve_story_points_field_id` reads `config.global.fields.story_points_field_id` | read `config.active_profile().story_points_field_id` and update the error message at line 213-214 to drop the misleading `"set story_points_field_id under [fields]"` advice (replace with `"under [profiles.<name>]"` or run `jr init`) |
| `src/cli/sprint.rs:232-233` | (same legacy reads) | (same fix) |
| `src/cli/board.rs:192` | (same legacy read) | (same fix) |

### Test fixture migration

| File:Line | Old | New |
|-----------|-----|-----|
| `src/cli/issue/helpers.rs:777-778` | `config.global.fields.{sp,team}_field_id = Some(...)` | `config.global.profiles.entry("default".into()).or_default().{sp,team}_field_id = Some(...); config.active_profile_name = "default".into();` |

### Cleanup (post-fix verification, optional but recommended for v0.6.0+)

- Once all read sites are migrated, the `instance: InstanceConfig` + `fields: FieldsConfig` fields on `GlobalConfig` (`src/config.rs:43-48`) should be **removed** entirely (not just `skip_serializing`d). Keep `migrate_legacy_global()` reading them via a one-shot deserialize-then-discard helper, OR fence the legacy struct behind a `#[cfg(...)]` migration-only module. Otherwise the dead `Default::default()` field is a permanent footgun for the next contributor who reads it as a "still works" path.
- Update the gotcha section of root `CLAUDE.md` to flag the per-profile read invariant alongside the existing "Multi-profile boundary" caching gotcha.

## Holdout candidate

**H-NEW-MP-001: Per-profile field IDs survive `Config::save_global()` round-trip and are observed by all hot-path read sites**

Asserts the round-trip invariant defined under "Expected behavior" above. Should be exercisable as both:

1. A pure `config.rs` integration test (no Jira API) — load a legacy `[fields]` config, call `save_global()`, reload, assert `active_profile().story_points_field_id == Some(<original>)` AND assert `global.fields.story_points_field_id` is `None` (because skip_serializing dropped it).
2. A wiremock-backed CLI integration test in `tests/` — `JR_BASE_URL` mock + tempdir HOME, run `jr issue list --points` against fixture issues with `customfield_10016 = 5`, run `jr auth switch default`, run `jr issue list --points` again, assert points column shows `5` in BOTH outputs.

If introduced as a holdout NOW (Phase 1), it must initially **fail** against the v0.5.0-dev.7 binary — proves the bug exists and gives Phase 2 a precise green-bar target.

## Test strategy

### Unit (in `src/config.rs`)

- `migration_round_trip_preserves_per_profile_field_ids()` — given a legacy-shape TOML on disk, after `Config::load_with(None)` then `save_global()` then `Config::load_with(None)` again, the per-profile fields equal the original `[fields]` values AND the on-disk file no longer has `[fields]` AND the on-disk file DOES have `[profiles.default].{story_points,team}_field_id`.

### Unit (in `src/cli/issue/helpers.rs`)

- Update existing `compose_extra_fields` test (line 770+) to write per-profile fields, not legacy fields. Run BEFORE migrating production reads to confirm the test currently fails — that failure is the canary.
- Add `resolve_story_points_field_id_reads_active_profile()` — assert the resolver returns `Some` for a config with only `[profiles.<name>].story_points_field_id` set, no `[fields]` block.

### Integration (in `tests/`)

- `multi_profile_field_isolation.rs` — two profiles `prod` and `sandbox`, each with different `customfield_NNN` IDs in their `[profiles.*].story_points_field_id`. With wiremock returning different point values per field-id query, `--profile prod jr issue list --points` and `--profile sandbox jr issue list --points` MUST show different (correct-per-profile) values. This catches both the regression AND any future cross-profile leakage.

### Snapshot / property

- Property test on `Config` round-trip: for any randomly generated `GlobalConfig` with N profiles, `serialize → deserialize → migrate → serialize` is idempotent after the first migration write-back. Proptest already used in repo (`docs/conventions` confirms).

### Manual smoke

- Bisect: confirm bug exists at `dea1664` (v0.5.0-dev.7, current `develop` HEAD per gitStatus) and is absent on the post-fix branch.

## Phase 2 story anchor (suggested split)

This BC is large enough that Phase 2 product-owner may want to split into 2-3 stories:

- **Story A:** Migrate hot-path read sites (issue list, view, sprint, board) → `active_profile()`. Includes test-fixture migration. Smallest viable cut that resolves the user-visible regression.
- **Story B:** Migrate `helpers.rs` resolvers (`resolve_team_field`, `resolve_story_points_field_id`, `compose_extra_fields`) and update error-message advice to point at `[profiles.<name>]` not `[fields]`. Touches create/edit paths.
- **Story C (cleanup):** Remove the `instance` and `fields` fields from `GlobalConfig` entirely; refactor `migrate_legacy_global()` to a one-shot deserialize. Defer to v0.6.0 — gated on Stories A+B + at least one minor-version of users having migrated.

Recommended bundling: A+B in a single PR (the user-visible bug isn't fixed by A alone — `jr issue create --points` still fails). C as a follow-up.

## Status checkpoint

```yaml
artifact: bc-nfr-r-d-draft
status: DRAFT — ready for Phase 1 PRD formalization
severity: CRITICAL
read_sites_confirmed: 11 (production) + 1 (test fixture) + 4 (migration-internal, correct)
files_affected: 5 (cli/issue/list.rs, cli/issue/view.rs, cli/issue/helpers.rs, cli/sprint.rs, cli/board.rs) + config.rs cleanup
holdout_candidate: H-NEW-MP-001
phase: 0 → 1 handoff
timestamp: 2026-05-04T16:00:00Z
```
