---
context: bc-6
title: "Configuration & Cache"
total_bcs: 39   # cumulative claim (incl. range-collapsed; now +1 for BC-6.2.015)
definitional_count: 29   # count of `#### BC-` headings in this file (added BC-6.2.015)
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/bc-06-config-cache.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.10-2.11
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §3.5-3.7
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.7
  - NFR draft: .factory/semport/jira-cli/jira-cli-bc-nfr-r-d-draft.md
---

# BC-6 — Configuration & Cache

39 behavioral contracts across 3 subdomains: Configuration (6.1), Cache (6.2),
Multi-profile fields — MUST-FIX (6.3).

---

## Subdomains

### 6.1 Configuration

#### BC-6.1.001: Legacy `[instance]/[fields]` blocks migrate to `[profiles.default]` on first load

**Confidence**: HIGH
**Source**: `tests/migration_legacy.rs:93-143`
**Subject**: Config & Cache
**Behavior**: After load, `config.global.profiles["default"]` carries url, cloud_id, team_field_id, story_points_field_id. On-disk file no longer contains `[instance]` or `[fields]` headers.
**Trace**: Pass 3 BC-901

---

#### BC-6.1.002: Migration is idempotent: second load produces byte-identical file

**Confidence**: HIGH
**Source**: `tests/migration_legacy.rs:145-172`
**Behavior**: `after_first == after_second` (byte equality). BC-149 (R1).
**Trace**: Pass 3 BC-902

---

#### BC-6.1.003: Migration write-back uses file-only baseline (no env overlay bleeds to disk)

**Confidence**: HIGH
**Source**: `src/config.rs:240-264`
**Subject**: Config & Cache
**Behavior**: `JR_DEFAULTS_OUTPUT=json jr auth login` for the first time after upgrade does NOT permanently save `output = "json"` to config.
**Trace**: Pass 3 BC-903; BC-153 (R1)

---

#### BC-6.1.004: `validate_profile_name` rejects: empty, >64 chars, non-`[A-Za-z0-9_-]`, reserved Windows names (case-insensitive)

**Confidence**: HIGH
**Source**: `src/config.rs:113-140`
**Subject**: Config & Cache
**Behavior**: Rejected names → `JrError::UserError`. Error message: `"invalid profile name {name:?}; allowed: A-Z a-z 0-9 _ - up to 64 chars; reserved Windows names (CON, NUL, AUX, PRN, COM1-9, LPT1-9) excluded"`. Boundary: `:` rejected; `.` rejected; `/` rejected; `prod-1` allowed; `sandbox_2` allowed.
**Trace**: Pass 3 BC-904; BC-904-R (R1)

---

#### BC-6.1.005: Profile-name validation runs at THREE boundaries: TOML key iteration, resolved active name, CLI flag

**Confidence**: HIGH
**Source**: `src/config.rs:269-282, 308-310`
**Subject**: Config & Cache
**Behavior**: Pass 1: iterates `global.profiles.keys()` after migration. Pass 2: after `resolve_active_profile_name`. Both call `validate_profile_name`. Hand-edited `[profiles."foo:bar"]` fails at pass 1 with context.
**Trace**: Pass 3 BC-152 (R1)

---

#### BC-6.1.006: `resolve_active_profile_name` precedence: cli_flag → env_var → global.default_profile → "default"

**Confidence**: HIGH
**Source**: `src/config.rs::resolve_active_profile_name`
**Behavior**: Each `if let Some(name) = X` checks in order, returning early. Strictly hierarchical.
**Trace**: Pass 3 BC-905; BC-905-R (R1)

---

#### BC-6.1.007: `Config::load_with(cli_profile)` strict — errors with `"unknown profile: <X>; known: <list>"`

**Confidence**: HIGH
**Source**: `src/config.rs:319-328`
**Subject**: Config & Cache
**Behavior**: `if strict && !global.profiles.is_empty() && !global.profiles.contains_key(&name)` → `JrError::UserError`. Fresh install (empty profiles) is allowed.
**Trace**: Pass 3 BC-906; BC-906-R (R1)

---

#### BC-6.1.008: `Config::load_lenient_with` skips active-profile existence check (used ONLY by `jr auth login`)

**Confidence**: HIGH
**Source**: `src/config.rs:285-289`
**Trace**: Pass 3 BC-907; BC-907-R (R1)

---

#### BC-6.1.009: Default `[defaults] output = "table"`

**Confidence**: HIGH
**Source**: `src/config.rs:63-74`
**Trace**: Pass 3 BC-908

---

#### BC-6.1.010: `JR_BASE_URL` env completely overrides profile URL (test/power-user)

**Confidence**: HIGH
**Source**: `src/config.rs:351-353`; `src/api/client.rs:37-65`
**Trace**: Pass 3 BC-909

---

#### BC-6.1.011: `find_project_config()` walks up cwd to filesystem root looking for `.jr.toml`; returns first match

**Confidence**: HIGH
**Source**: `src/config.rs:340-353`
**Subject**: Config & Cache
**Behavior**: `loop { if candidate.exists() { return Some } if !dir.pop() { return None } }`. No XDG fallback.
**Edge case (documented limitation)**: The filesystem walk uses `Path::exists()` which follows symlinks. If `.jr.toml` is a symlink pointing to another file, it is followed without loop detection. A symlink cycle (e.g., `a -> b -> a`) could cause an OS-level error which propagates as an IO error. This is a known limitation of the canonical-path-agnostic design — not a bug to fix in v1.
**Trace**: Pass 3 BC-911; BC-911-R (R1)

---

#### BC-6.1.012: User-facing migration message emitted to stderr exactly once per process

**Confidence**: HIGH
**Source**: `src/config.rs:262-265`
**Behavior**: `"Migrated config to multi-profile layout (single profile \"default\"). Run 'jr auth list' to view profiles."` — only when migration triggers.
**Trace**: Pass 3 BC-151 (R1)

---

#### BC-6.1.013: `JR_PROFILE` env override for active profile; scrubbed by tests to prevent direnv pollution

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:9-32`; `src/config.rs:307`
**Behavior**: 16 env vars scrubbed per test. `JR_PROFILE_OVERRIDE` historical env seam replaced by parameter threading (unsafe POSIX `setenv` avoidance).
**Trace**: Pass 3 BC-154 (R1)

---

### 6.2 Cache

#### BC-6.2.001: `read_cache<T>` returns `Ok(None)` for NotFound; propagates other I/O errors

**Confidence**: HIGH
**Source**: `src/cache.rs:14-34`
**Subject**: Config & Cache
**Behavior**: `NotFound` → `Ok(None)`. Permission-denied → `Err(io::Error)`. Only missing-file maps to None.
**Trace**: Pass 3 BC-1001; BC-1001-R (R1)

---

#### BC-6.2.002: `read_cache<T>` returns `Ok(None)` AND stderr warning for parse failure

**Confidence**: HIGH
**Source**: `src/cache.rs:23-26`; `tests/issue_view_errors.rs:142-206`
**Subject**: Config & Cache
**Behavior**: Literal stderr: `"warning: cache file <name> unreadable (<err>); will refetch"`. No deletion. Single warning per (process, filename). Corrupt file remains until next write.
**Trace**: Pass 3 BC-1002; BC-1002-R (R1)

---

#### BC-6.2.003: TTL check: `(Utc::now() - fetched_at).num_days() >= CACHE_TTL_DAYS (7)`; exactly 7 days is expired

**Confidence**: HIGH
**Source**: `src/cache.rs:7, 30-32`
**Subject**: Config & Cache
**Behavior**: `const CACHE_TTL_DAYS: i64 = 7`. `>= 7` → expired. `7-day-old` is expired. Unit test writes 8-day-old entry.
**Trace**: Pass 3 BC-1003; BC-1003-R (R1)

---

#### BC-6.2.004: Per-profile cache directory: `~/.cache/jr/v1/<profile>/`

**Confidence**: HIGH
**Source**: `src/cache.rs:7, 30, 76-78`
**Behavior**: Versioned root `v1/` allows future schema-bump cleanup. New schema → bump to `v2/`, old files orphan harmlessly.
**Trace**: Pass 3 BC-1004

---

#### BC-6.2.005: `clear_profile_cache(name)` is no-op when directory doesn't exist (does NOT error)

**Confidence**: HIGH
**Source**: `src/cache.rs:82-88`
**Behavior**: `if dir.exists() { remove_dir_all(dir)? }` — exists() check short-circuits.
**Trace**: Pass 3 BC-1005; BC-1005-R (R1)

---

#### BC-6.2.006: `cmdb_fields.json` stores (id, name) tuples; old ID-only format → cache miss (graceful)

**Confidence**: HIGH
**Source**: `src/cache.rs:237-247`; CLAUDE.md
**Behavior**: Old format → deserialization failure → `Ok(None)` (treated as miss). Auto-expiry via 7d TTL.
**Trace**: Pass 3 BC-1006

---

#### BC-6.2.007: `ProjectMeta` map cache `project_meta.json` keyed by project key; per-entry TTL

**Confidence**: HIGH
**Source**: `src/cache.rs:105-143`; `tests/project_meta.rs`
**Trace**: Pass 3 BC-1007

---

#### BC-6.2.008: `ResolutionsCache` drops resolutions without `id` on write + stderr warning

**Confidence**: HIGH
**Source**: `src/cli/issue/workflow.rs:117-133`
**Behavior**: stderr: `"warning: N resolution(s) lacked an id and were not cached"`.
**Trace**: Pass 3 BC-1008

---

#### BC-6.2.009: Cross-profile isolation: writing `prod` cache does NOT make `sandbox` cache visible

**Confidence**: HIGH
**Source**: `src/cache.rs:389-406`
**Subject**: Config & Cache
**Behavior**: `read_team_cache("sandbox")` returns `None` after writing `prod` team cache. Path construction: `cache_dir(profile) = cache_root().join("v1").join(profile)`.
**Trace**: Pass 3 BC-1011 (R1)

---

#### BC-6.2.010: `clear_profile_cache("prod")` does NOT delete `sandbox` data

**Confidence**: HIGH
**Source**: `src/cache.rs:408-439`
**Behavior**: Write both prod + sandbox; clear prod; assert prod is None AND sandbox is Some.
**Trace**: Pass 3 BC-1012 (R1)

---

#### BC-6.2.011: Corrupt cache files (garbage data + valid-JSON-wrong-shape) both return `Ok(None)`

**Confidence**: HIGH
**Source**: `src/cache.rs:808-861`
**Subject**: Config & Cache
**Behavior**: Two corruption modes: (1) `"not json"` and (2) `{"unexpected": true}`. Both → `Ok(None)`. Format-change resilience.
**Trace**: Pass 3 BC-1013 (R1)

---

#### BC-6.2.012: `write_project_meta` MERGES into existing map; corruption recovery → fresh start + stderr warning

**Confidence**: HIGH
**Source**: `src/cache.rs:146-173`; unit test `project_meta_multiple_projects` (`:563-594`)
**Behavior**: Read-modify-write semantics. Corruption → `"warning: project_meta.json unreadable (<err>); starting fresh — other cached projects will be lost"`.
**Trace**: Pass 3 BC-1014 (R1)

---

#### BC-6.2.013: `write_object_type_attr_cache` MERGES into existing per-type map; same corruption recovery pattern

**Confidence**: HIGH
**Source**: `src/cache.rs:318-354`; unit test `object_type_attr_cache_multiple_types` (`:762-794`)
**Behavior**: Corruption → `"warning: object_type_attrs.json unreadable (<err>); starting fresh — other cached object types will be lost"`.
**Trace**: Pass 3 BC-1015 (R1)

---

#### BC-6.2.014: Cache write is non-atomic (`std::fs::write`); crash mid-write leaves truncated file; read-side resilient

**Confidence**: HIGH
**Source**: `src/cache.rs:42, 171, 351`
**Subject**: Config & Cache
**Behavior**: `std::fs::write(path, content)` — no temp-file + rename, no fsync. Crash → truncated file → next read returns `Ok(None)` + stderr warning → re-fetch proceeds. This is the documented contract, not a bug.
**Trace**: Pass 3 BC-1016 (R1)

---

#### BC-6.2.015: Every cache reader/writer takes `profile: &str` as its first parameter (soft-fence convention)

**Confidence**: HIGH
**Source**: `src/cache.rs` (all public functions); NFR-SCA-2
**Subject**: Config & Cache
**Behavior**: Architectural convention: ALL cache read/write functions accept `profile: &str` as their first positional argument. No profile-unaware cache function exists. This is a soft fence (convention, not type system). Enforcement pattern: `grep -n 'fn read_cache\|fn write_cache\|fn read_team\|fn write_team\|fn read_project\|fn write_project' src/cache.rs` should show `profile: &str` as first non-self parameter in every result.
**Verification test pattern**: `grep -E 'fn (read|write)_\w+\((?!.*profile)' src/cache.rs` should return zero matches.
**Related**: NFR-SCA-2 (compile-time enforcement deferred — `Profile(String)` newtype P1 priority).
**Trace**: NFR-SCA-2; Pass 4 R4; CLAUDE.md "Multi-profile boundary" gotcha

---

### 6.3 Multi-Profile Fields — MUST-FIX (NFR-R-D)

#### BC-6.3.001: Per-profile `story_points_field_id` and `team_field_id` survive `Config::save_global()` and are read by ALL hot-path read sites [MUST-FIX: NFR-R-D — CRITICAL]

**Confidence**: HIGH
**Source**: Multiple sites (BUG: 14 hot-path read sites use `config.global.fields.*`)

> **MUST-FIX (CRITICAL — NFR-R-D):** Current code has 14 hot-path read sites still reading
> from `config.global.fields.story_points_field_id` / `config.global.fields.team_field_id`
> (the legacy `[fields]` block). After ANY `Config::save_global()` call (e.g., `jr auth login`,
> `jr auth switch`, `jr init`), the `[fields]` block is dropped from disk (due to
> `#[serde(default, skip_serializing)]` at `src/config.rs:43-48`). All 14 read sites
> then observe `None` — columns silently disappear. This contract describes the FIXED behavior.

**Spec contract (fixed behavior — round-trip invariant):**
```
For all profiles P and all field-id pairs (sp, team) where:
  config.global.profiles[P].story_points_field_id == Some(sp) AND
  config.global.profiles[P].team_field_id == Some(team)

After config.save_global() followed by Config::load_with(Some(P)):
  config.active_profile().story_points_field_id MUST == Some(sp)
  config.active_profile().team_field_id MUST == Some(team)

AND every read site in the table below MUST observe (sp, team) from active_profile(),
not from global.fields.* (which no longer exists on disk post-save).
```

**Hot-path read sites that MUST be migrated:**

| # | File:Line | Field | Used for |
|---|-----------|-------|----------|
| 1 | `src/cli/issue/list.rs:147` | `story_points_field_id` | `--points` column |
| 2 | `src/cli/issue/list.rs:148` | `team_field_id` | Team column gating |
| 3 | `src/cli/issue/view.rs:28` | `story_points_field_id` | Points display |
| 4 | `src/cli/issue/view.rs:29` | `team_field_id` | Team display |
| 5 | `src/cli/issue/helpers.rs:43` | `team_field_id` | `resolve_team_field()` short-circuit |
| 6 | `src/cli/issue/helpers.rs:194` | `story_points_field_id` | `compose_extra_fields()` |
| 7 | `src/cli/issue/helpers.rs:200` | `team_field_id` | `compose_extra_fields()` |
| 8 | `src/cli/issue/helpers.rs:209` | `story_points_field_id` | `resolve_story_points_field_id()` |
| 9 | `src/cli/sprint.rs:232` | `story_points_field_id` | Sprint issue points |
| 10 | `src/cli/sprint.rs:233` | `team_field_id` | Sprint issue team |
| 11 | `src/cli/board.rs:192` | `team_field_id` | Board view team gating |
| 12 | `src/cli/issue/create.rs:128` | `story_points_field_id` | `--points` field injection in create body |
| 13 | `src/cli/issue/create.rs:277` | `team_field_id` | Team field injection in create body |
| 14 | `src/cli/issue/create.rs:283` | `story_points_field_id` | Points field injection in create body (second site) |

**Fix mechanism (per ADR-0007):** Route all field reads through the `Config::field_id(FieldKind, profile)` accessor introduced in ADR-0007. This accessor resolves to `config.global.profiles[profile].story_points_field_id` (or `team_field_id`), not to the deprecated `config.global.fields.*` path. No fallback to `global.fields.*` is permitted after the fix is applied.

**Fix pattern (equivalent shorthand):** Replace every `config.global.fields.X` read with `config.active_profile().X` (or `config.active_profile_or_err()?.X`). The `Config::field_id` accessor is the preferred call site for new code.

**User-visible symptoms (current bug state):**
1. `jr issue list --points` — points column blank after first save
2. `jr issue list` — Team column disappears
3. `jr issue view <KEY>` — points and team drop off
4. `jr sprint current` — points + team missing
5. `jr board view` — team column missing
6. `jr issue create --points 5` — fails with error pointing at broken legacy block
7. Multi-profile users: silently use wrong field IDs across profiles

**Holdout:** H-NEW-MP-001 — Per-profile field IDs survive `Config::save_global()` round-trip and are observed by all hot-path read sites.

**Error taxonomy:** `JrError::ConfigError("Story points field not configured. Run \"jr init\" or set story_points_field_id under [profiles.<name>] in ~/.config/jr/config.toml")` — note: error message must be updated to reference `[profiles.<name>]` not `[fields]`.

**Error-message strings to update (Phase 3 implementation checklist):**

The following BCs contain pinned stderr/error-message text that references the deprecated `[fields]` section and MUST be updated as part of the NFR-R-D fix:

| BC | File | Current (wrong) text | Required (fixed) text |
|----|------|---------------------|----------------------|
| BC-2.2.021 | `src/cli/issue/list.rs:756-770` | `set [fields].story_points_field_id in ~/.config/jr/config.toml` | `set story_points_field_id under [profiles.<name>] in ~/.config/jr/config.toml` |
| BC-6.3.001 (this) | `src/cli/issue/helpers.rs` | any reference to `[fields]` in ConfigError messages | `[profiles.<name>]` |

These pinned-text changes are load-bearing for the holdout H-NEW-MP-001 verification step.

**Trace**: NFR-R-D; NEW-INV-12; NEW-INV-143; `jira-cli-bc-nfr-r-d-draft.md`; Pass 8 §5.2

---

## Key Invariants

- Profile-name max 64 chars; charset `[A-Za-z0-9_-]`; Windows reserved names blocked
- Migration write-back uses file-only baseline
- Cache TTL: 7 days; `>= 7` is expired (not `> 7`)
- Cache directory: `~/.cache/jr/v1/<profile>/`
- Non-atomic writes are the documented contract; self-heal on read
- Cross-profile cache isolation enforced by naming convention (not type system)
- `config.active_profile()` is the SOLE source of truth for per-profile custom field IDs post-fix
