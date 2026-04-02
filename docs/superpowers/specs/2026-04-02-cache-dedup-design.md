# Cache Deduplication Refactor

**Issue:** #104
**Date:** 2026-04-02

## Problem

`src/cache.rs` has 5 nearly identical read/write function pairs that repeat the
same pattern: build path, check exists, read + deserialize JSON, check TTL,
return `Ok(None)` if expired/missing, handle corrupt files. ~260 lines of
production code is mostly this boilerplate.

Additionally, 3 of the 5 read functions propagate deserialization errors instead
of treating them as cache misses, which can surface user-facing errors for a
non-critical cache.

## Design

### Approach: Generic free functions + minimal trait

Extract two internal generic functions and a one-method trait. Keep the existing
public API unchanged — all callers continue using the same function signatures.

### New internal abstractions

All `pub(crate)` or private to `cache.rs`:

```rust
/// Implemented by cache structs that carry a timestamp for TTL checks.
pub(crate) trait Expiring {
    fn fetched_at(&self) -> DateTime<Utc>;
}

/// Read a whole-file cache. Returns Ok(None) on missing, expired, or corrupt files.
fn read_cache<T: DeserializeOwned + Expiring>(filename: &str) -> Result<Option<T>> {
    let path = cache_dir().join(filename);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let cache: T = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => return Ok(None), // corrupt = cache miss
    };
    if (Utc::now() - cache.fetched_at()).num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }
    Ok(Some(cache))
}

/// Write a whole-file cache. Creates the cache directory if needed.
fn write_cache<T: Serialize>(filename: &str, data: &T) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;
    let content = serde_json::to_string_pretty(data)?;
    std::fs::write(dir.join(filename), content)?;
    Ok(())
}
```

### Whole-file caches (fully deduplicated)

Three caches share an identical structure and collapse to thin wrappers:

| Cache | File | Struct |
|-------|------|--------|
| Teams | `teams.json` | `TeamCache` |
| Workspace | `workspace.json` | `WorkspaceCache` |
| CMDB fields | `cmdb_fields.json` | `CmdbFieldsCache` |

Each struct gains a one-liner `Expiring` impl:

```rust
impl Expiring for TeamCache {
    fn fetched_at(&self) -> DateTime<Utc> { self.fetched_at }
}
```

Public functions collapse to:

```rust
pub fn read_team_cache() -> Result<Option<TeamCache>> {
    read_cache("teams.json")
}

pub fn write_team_cache(teams: &[CachedTeam]) -> Result<()> {
    write_cache("teams.json", &TeamCache {
        fetched_at: Utc::now(),
        teams: teams.to_vec(),
    })
}
```

### Keyed caches (kept explicit)

Two caches use `HashMap<String, T>` with different TTL semantics and are not
worth genericizing:

| Cache | File | TTL model |
|-------|------|-----------|
| Project meta | `project_meta.json` | Per-entry (`ProjectMeta.fetched_at`) |
| Object type attrs | `object_type_attrs.json` | Per-file (`ObjectTypeAttrCache.fetched_at`) |

These stay as explicit functions with a doc comment explaining why they are not
genericized (different TTL semantics). The only change is normalizing corrupt-file
handling: `read_project_meta` currently propagates deserialization errors via `?`.
After this refactor it will return `Ok(None)` on corrupt data, matching
`read_cmdb_fields_cache` and `read_object_type_attr_cache` which already do this.

### Behavior changes

1. **Corrupt-file handling normalized:** All 5 read functions treat
   deserialization failures as cache misses (`Ok(None)`). Previously, 3 of 5
   (teams, workspace, project_meta) propagated the error. This aligns with how
   Cargo handles corrupt caches (silently skip/ignore).

2. **No on-disk format changes.** All existing structs keep their field names and
   shapes. The `Expiring` trait is a Rust-side abstraction only with no serde
   impact.

### What does NOT change

- Public function signatures
- Caller code in `api/`, `cli/`
- On-disk JSON format
- Cache file names or paths
- TTL duration (7 days)
- Existing test assertions

### New tests

Add 3 corrupt-file tests for caches that previously lacked them:

- `corrupt_team_cache_returns_none`
- `corrupt_workspace_cache_returns_none`
- `corrupt_project_meta_returns_none`

Each corrupt test should cover both garbage data (`"not json"`) and valid JSON
with a wrong shape (e.g., `{"unexpected": true}`) to exercise the `Expiring`
deserialization path.

### Estimated impact

- ~60-70 lines of production code eliminated (from ~260 to ~190)
- 3 new tests added
- 5 one-liner trait impls added
- Net reduction: ~40-50 lines

## Validation

Design decisions validated against Rust community best practices via research:

- **Corrupt = cache miss:** Cargo silently ignores cache errors. Consensus for
  non-critical caches is to treat deserialization failures as misses.
- **Trait over closure:** A one-method trait (`Expiring`) is more idiomatic than
  passing a `FnOnce` closure for simple field access on generic functions.
- **Rule of Three:** Only the 3 identical whole-file caches are genericized. The
  2 keyed caches have different TTL semantics and don't justify a shared
  abstraction.
- **Trait has no serde impact:** Adding a non-serde trait impl to a struct with
  `#[derive(Serialize, Deserialize)]` has zero effect on serialization.
