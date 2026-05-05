---
title: "BC-06: Configuration & Cache"
version: "1.0.0"
snapshot_sha: "dea166471e22eff55974d7675593469b37048c5f"
traces_to: "README.md"
source_passes: "Pass 2 broad §2a.2 Config+Cache + R1 §3.4,3.5 T-05,T-07 + Pass 8 §2.2 BC#11,12"
entity_count: 22
invariant_count: 26
bc_count: 39
risk_level: HIGH
---

# BC-06: Configuration & Cache

The Configuration & Cache bounded context owns all persistent non-credential state: the figment-layered `config.toml`, the per-project `.jr.toml`, and the per-profile JSON cache directory. Both sub-contexts share the "active profile name" as their primary isolation key.

---

## §1 Ubiquitous Language

| Term | Definition |
|------|-----------|
| **Active profile** | The resolved profile name for the current invocation. Precedence: `--profile` flag > `JR_PROFILE` env > `default_profile` config field > `"default"`. Threaded as a parameter, not an env-var seam. |
| **figment layering** | Config is assembled by figment: Defaults → `~/.config/jr/config.toml` → `./.jr.toml` → `JR_*` env. Highest priority wins. |
| **Per-profile cache boundary** | Every cache read/write function takes `profile: &str` as its first argument. Convention-enforced; not type-fenced. |
| **Whole-file cache** | Cache entries where the entire JSON file is one struct (e.g., `teams.json`, `workspace.json`). Replaced atomically on write. |
| **Map cache** | Cache entries where the JSON is a `HashMap<key, entry>` (e.g., `project_meta.json`, `object_type_attrs.json`). Per-key TTL for `project_meta`; file-level TTL for `object_type_attrs`. |
| **Cache miss** | `Ok(None)` — returned for NotFound, expired (≥7d), or deserialization failure. NEVER `Err` for these cases. |
| **Cache corruption** | Deserialization failure → cache miss + stderr warning. Corrupt file remains on disk; next write replaces it. |
| **Legacy migration** | On first load of config with `[instance]`/`[fields]` top-level keys, maps them to `[profiles.default]`. Write-back uses file-only baseline (env vars don't bleed). |
| **`CACHE_TTL_DAYS = 7`** | Universal TTL for all cache entries. Not configurable. |
| **`save_global`** | `Config` write method. Overlays only `default_profile + profiles` from in-memory onto file-only baseline. Other figment sources (env, project) don't bleed into saved file. |

---

## §2 Entities

### Configuration Entities

| Entity | Module | Key Fields | Notes |
|--------|--------|-----------|-------|
| `Config` | `config.rs:82-88` | `global: GlobalConfig`, `project: ProjectConfig`, `active_profile_name: String` | Built by `Config::load_with(cli_profile)`. Active profile name resolved at construction. Every network-touching handler holds one. |
| `GlobalConfig` | `config.rs:27-52` | `default_profile: Option<String>`, `profiles: BTreeMap<String, ProfileConfig>`, `instance: InstanceConfig` (legacy, skip-on-serialize), `fields: FieldsConfig` (legacy), `defaults: DefaultsConfig` | `BTreeMap` for stable ordering in `jr auth list` and serialized config. |
| `ProfileConfig` | `config.rs:16-25` | `url: Option<String>`, `auth_method: Option<String>`, `cloud_id: Option<String>`, `org_id: Option<String>`, `oauth_scopes: Option<String>`, `team_field_id: Option<String>`, `story_points_field_id: Option<String>` | The fundamental unit of multi-profile isolation. Per-profile `team_field_id`/`story_points_field_id` exist but are NOT read by CLI handlers (NFR-R-D CRITICAL). |
| `ProjectConfig` | `config.rs:76-80` | `project: Option<String>`, `board_id: Option<u64>` | Loaded from `.jr.toml` walked up from cwd. Not merged into `GlobalConfig`. |
| `FieldsConfig` (legacy) | `config.rs:10-14` | `team_field_id: Option<String>`, `story_points_field_id: Option<String>` | Read-only; drained into `ProfileConfig` during migration. Skip-on-serialize after migration. |
| `InstanceConfig` (legacy) | `config.rs:54-61` | `url`, `cloud_id`, `org_id`, `auth_method`, `oauth_scopes` | Same migration semantics as `FieldsConfig`. |
| `DefaultsConfig` | `config.rs:63-74` | `output: String` (default `"table"`) | Single-field extension point. |

### Cache Entities

| Entity | Module | Key Fields | Notes |
|--------|--------|-----------|-------|
| `Expiring` (trait) | `cache.rs:10-12` | `fn fetched_at(&self) -> DateTime<Utc>` | The TTL contract. Every cached struct implements this. |
| `CachedTeam` | `cache.rs:45-49` | `id: String`, `name: String` | Persisted shape inside `TeamCache`. |
| `TeamCache` | `cache.rs:51-61` | `fetched_at: DateTime<Utc>`, `teams: Vec<CachedTeam>` | Whole-file `teams.json`. |
| `ProjectMeta` | `cache.rs:105-112` | `project_type: String`, `simplified: bool`, `project_id: String`, `service_desk_id: Option<String>`, `fetched_at: DateTime<Utc>` | Map cache `project_meta.json` (key: project key). Per-entry TTL. Drives JSM detection. |
| `WorkspaceCache` | `cache.rs:175-185` | `workspace_id: String`, `fetched_at: DateTime<Utc>` | Whole-file `workspace.json`. |
| `CachedResolution` | `cache.rs:202-208` | `id: String` (non-optional), `name: String`, `description: Option<String>` | `id` non-optional — entries without id dropped on write (stderr warning). |
| `ResolutionsCache` | `cache.rs:210-220` | `resolutions: Vec<CachedResolution>`, `fetched_at: DateTime<Utc>` | Whole-file `resolutions.json`. |
| `CmdbFieldsCache` | `cache.rs:237-247` | `fields: Vec<(String, String)>` (id, name tuples), `fetched_at: DateTime<Utc>` | Whole-file `cmdb_fields.json`. Tuple shape is the format-change point — old ID-only format → deserialization failure → cache miss. |
| `CachedObjectTypeAttr` | `cache.rs:264-276` | `id: String`, `name: String`, `system: bool`, `hidden: bool`, `label: bool`, `position: i32` | Thin projection of `ObjectTypeAttributeDef`. Enough for `jr assets schema` display. |
| `ObjectTypeAttrCache` | `cache.rs:278-282` | `fetched_at: DateTime<Utc>`, `types: HashMap<String, Vec<CachedObjectTypeAttr>>` | Map cache `object_type_attrs.json` (key: object-type id). File-level TTL. |

---

## §3 Value Objects & Enums

- **`CACHE_TTL_DAYS = 7`**: universal, hardcoded, non-configurable.
- **Cache path template**: `~/.cache/jr/v1/<profile>/<file>.json`. Versioned root `v1/` allows future schema bump to orphan stale files.
- **Cache filenames** (whole-file): `teams.json`, `workspace.json`, `resolutions.json`, `cmdb_fields.json`.
- **Cache filenames** (map): `project_meta.json`, `object_type_attrs.json`.
- **Config path**: `~/.config/jr/config.toml` (global) + `./.jr.toml` (per-project, walked up from cwd).
- **Profile name charset**: `[A-Za-z0-9_-]{1,64}` excluding reserved Windows names.

---

## §4 Operations

| Operation | Effect |
|-----------|--------|
| `Config::load_with(cli_profile)` | Builds `Config` from figment layers. Resolves active profile. Runs legacy migration if needed. Validates all profile names. |
| `Config::load_lenient_with` | Same but does NOT error on unknown profile (used for commands that don't need a fully configured profile). |
| `cache::read_*_cache(profile, ...)` | Returns `Ok(None)` for miss/expired/corrupt. Returns `Ok(Some(T))` for fresh hit. NEVER `Err` for those cases. |
| `cache::write_*_cache(profile, data)` | Writes JSON to profile cache dir. Non-atomic (no temp-file rename — LOW risk; self-heals on corrupt via miss path). |
| `cache::clear_profile_cache(name)` | Deletes `~/.cache/jr/v1/<name>/`. No-op when directory doesn't exist. |
| `Config::save_global()` | Overlays `default_profile + profiles` from in-memory onto file-only baseline. Env-var overlay does NOT bleed. |
| `jr init` | Creates/updates default profile, prefetches org metadata + team cache + story-points field. Idempotent (re-running rediscovers and overwrites). |

---

## §5 Business Rules & Invariants

| ID | Invariant | Source |
|----|----------|--------|
| INV-CONFIG-001 | Active profile resolution precedence: `--profile` flag > `JR_PROFILE` env > `config.default_profile` > `"default"`. Threaded as parameter via `Config::load_with(cli_profile)`. NOT an env-var seam. | `config.rs:95-110`, BC-007 |
| INV-CONFIG-002 | Profile name must match `[A-Za-z0-9_-]{1,64}` and must not be a reserved Windows name (`CON`, `NUL`, `AUX`, `PRN`, `COM1-9`, `LPT1-9`). Validated at three boundaries: CLI flag, TOML key, resolved active name. | `config.rs:113-140`, BC-019/NEW-INV-008 |
| INV-CONFIG-003 | Profile name flows into cache paths AND keychain key prefixes. A value like `foo:bar` or `../etc/passwd` would corrupt both. Path-traversal prevention. | `config.rs:113-140` |
| INV-CONFIG-004 | `save_global` overlays ONLY `default_profile + profiles` from in-memory onto file-only baseline. `JR_*` env vars do NOT bleed into the saved file. | `config.rs:416-446`, NEW-INV-11 |
| INV-CONFIG-005 | Migration write-back uses file-only baseline (not env-overlaid). Transient `JR_*` vars cannot corrupt `config.toml` during migration. | `config.rs:240-264`, NEW-INV-10 |
| INV-CONFIG-006 | Malformed TOML → `JrError::ConfigError` (exit 78). File is NOT overwritten via `unwrap_or_default()`. Fail-loud safety. | BC-1139 |
| INV-CONFIG-007 | NFR-R-D (CRITICAL): CLI handlers read `config.global.fields.story_points_field_id` / `team_field_id` (legacy path) at 12+ sites. Per-profile `ProfileConfig.story_points_field_id`/`team_field_id` exist but are never read. Sandbox vs prod field IDs silently collide. | `cli/issue/list.rs:147-148`, `cli/sprint.rs:232-233`, `cli/board.rs:192-193`, `cli/issue/create.rs:128,277,283` |
| INV-CONFIG-008 | Active-profile existence check is gated on `!profiles.is_empty() && strict`. Empty profiles map → no check (lenient mode). | `config.rs:318-328`, NEW-INV-13 |
| INV-CONFIG-009 | `JR_BASE_URL` env var completely overrides the active profile's URL. Intended for tests. Bypasses URL field on the profile. | `config.rs:351-353`, `client.rs:37-65` |
| INV-CONFIG-010 | `Config::active_profile_or_err()` returns borrowed `&ProfileConfig` (hot path). `Config::active_profile()` returns owned `ProfileConfig` (cold path). Two distinct accessors. | `config.rs` |
| INV-CACHE-001 | Cache reads return `Ok(None)` — never `Err` — for missing, expired (≥7d), or deserialized-failed files. `NotFound` → `Ok(None)`. Deser failure → `eprintln!` warning + `Ok(None)`. | `cache.rs:14-34` |
| INV-CACHE-002 | Cache miss policy: "corrupt = miss." A corrupt file stays on disk until next write. Self-healing with no operator intervention required. | `cache.rs:14-34` |
| INV-CACHE-003 | TTL is exactly 7 days. Not configurable. Applies uniformly across all 7 cache categories. | `cache.rs:7` |
| INV-CACHE-004 | Per-profile cache boundary enforced by convention: every read/write function takes `profile: &str` as first argument. No compile-time fence. Future contributors could break this. | `cache.rs`, NEW-INV-08 |
| INV-CACHE-005 | `clear_profile_cache(name)` is no-op when `~/.cache/jr/v1/<name>/` does not exist. | `cache.rs:82-88` |
| INV-CACHE-006 | Map-cache writes (`project_meta`, `object_type_attrs`) on deserialization failure of the existing file silently destroy ALL other entries in that map. Not a partial update — the file is completely replaced with just the new entry. | `cache.rs:158-162,330-338`, NEW-INV-07 |
| INV-CACHE-007 | Cache versioned root `v1/` allows future schema bump to orphan stale files cleanly (e.g., changing `cmdb_fields.json` format from ID-only to `(id,name)` tuples would bump to `v2/`). | CLAUDE.md, `cache.rs:30` |
| INV-CACHE-008 | Cache test scaffolding mutates `XDG_CACHE_HOME` env under a static `ENV_MUTEX` to isolate test profiles. Not a cache behavior invariant; a test isolation contract. | `cache.rs:362-379`, NEW-INV-09 |
| INV-CACHE-009 | Non-atomic cache writes: `std::fs::write` directly (no temp-file + atomic rename). Crash between write-start and write-end leaves a partially-written file, which self-heals on next read as a cache miss. LOW risk (no data-loss beyond the 7-day TTL). | `cache.rs:36-43`, §6.2 anti-pattern |
| INV-CACHE-010 | `CmdbFieldsCache` stores `Vec<(String, String)>` (id, name tuples). Old format (ID-only) → deserialization failure → cache miss. Format change requires either migration or `v2/` root bump. | `cache.rs:237-247` |
| INV-CACHE-011 | `ProjectMeta` map cache uses per-entry TTL (each entry has its own `fetched_at`). `ObjectTypeAttrCache` map cache uses file-level TTL (single `fetched_at` for all entries). Asymmetric design. | `cache.rs:105-112, 278-282` |
| INV-CACHE-012 | `CachedResolution.id` is non-optional. Resolutions without id from Jira API are dropped on cache write with a stderr count warning. | `cache.rs:202-208`, `cli/issue/workflow.rs:117-133` |
| INV-CONFIG-011 | `JR_PROFILE` env var selects the active profile per-call (combine with direnv to scope a repo to a sandbox). Overridden by `--profile` flag. | CLAUDE.md, `config.rs` |
| INV-CONFIG-012 | `ProjectConfig` (from `.jr.toml`) is kept as a separate field on `Config` — NOT merged into `GlobalConfig`. This is the "what does this repository talk to" layer. | `config.rs:76-80` |
| INV-CONFIG-013 | figment layering order (lowest-to-highest priority): defaults → `~/.config/jr/config.toml` → `./.jr.toml` → `JR_*` env. Higher priority wins on conflict. | Pass 8 §2.4 |

---

## §6 Aggregate Boundaries

- **Configuration aggregate**: `Config → GlobalConfig + ProjectConfig`. `GlobalConfig → BTreeMap<String, ProfileConfig>`. `Config::active_profile_name` is a resolved projection (not a stored field on disk).
- **Cache aggregate**: no in-memory aggregate; each cached entity is a self-contained per-profile file. The implicit aggregate root is the per-profile cache directory `~/.cache/jr/v1/<profile>/`.

---

## §7 Cross-Context Dependencies

| Depends on | Reason |
|-----------|--------|
| **Auth (BC-01)** | `Config::save_global()` writes profile changes (including `cloud_id`, `org_id` after OAuth). `auth remove` calls `cache::clear_profile_cache`. |
| **All other contexts** | Every handler reads `Config` and passes `profile: &str` to cache functions. Config is the dependency root. |
