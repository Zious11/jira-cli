# Cache Deduplication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deduplicate 5 repetitive cache read/write function pairs in `src/cache.rs` using generic free functions and a minimal `Expiring` trait, normalizing corrupt-file handling to `Ok(None)` across all caches.

**Architecture:** Extract `read_cache<T>` and `write_cache<T>` generic functions plus a one-method `Expiring` trait. Apply to 3 whole-file caches (teams, workspace, cmdb_fields). Keep 2 keyed caches (project_meta, object_type_attrs) as explicit functions with corrupt-handling fix only. Public API unchanged.

**Tech Stack:** Rust, serde, serde_json, chrono, anyhow

**Spec:** `docs/superpowers/specs/2026-04-02-cache-dedup-design.md`
**Issue:** #104

---

### Task 1: Add corrupt-file tests for caches that currently lack them

Before refactoring, establish the new behavior as tests. Currently `read_team_cache`, `read_workspace_cache`, and `read_project_meta` propagate deserialization errors. These tests will initially fail (they expect `Ok(None)` but get `Err`). We write them first so the refactor in later tasks makes them pass.

**Files:**
- Modify: `src/cache.rs:266-636` (test module)

- [ ] **Step 1: Add `corrupt_team_cache_returns_none` test**

Add this test at the end of the `mod tests` block (before the closing `}`), after the `object_type_attr_cache_corrupt_returns_none` test:

```rust
    #[test]
    fn corrupt_team_cache_returns_none() {
        with_temp_cache(|| {
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();

            // Garbage data
            std::fs::write(dir.join("teams.json"), "not json").unwrap();
            let result = read_team_cache().unwrap();
            assert!(result.is_none(), "garbage data should return None");

            // Valid JSON, wrong shape
            std::fs::write(dir.join("teams.json"), r#"{"unexpected": true}"#).unwrap();
            let result = read_team_cache().unwrap();
            assert!(result.is_none(), "wrong-shape JSON should return None");
        });
    }
```

- [ ] **Step 2: Add `corrupt_workspace_cache_returns_none` test**

Add immediately after the previous test:

```rust
    #[test]
    fn corrupt_workspace_cache_returns_none() {
        with_temp_cache(|| {
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();

            // Garbage data
            std::fs::write(dir.join("workspace.json"), "not json").unwrap();
            let result = read_workspace_cache().unwrap();
            assert!(result.is_none(), "garbage data should return None");

            // Valid JSON, wrong shape
            std::fs::write(dir.join("workspace.json"), r#"{"unexpected": true}"#).unwrap();
            let result = read_workspace_cache().unwrap();
            assert!(result.is_none(), "wrong-shape JSON should return None");
        });
    }
```

- [ ] **Step 3: Add `corrupt_project_meta_returns_none` test**

Add immediately after the previous test:

```rust
    #[test]
    fn corrupt_project_meta_returns_none() {
        with_temp_cache(|| {
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();

            // Garbage data
            std::fs::write(dir.join("project_meta.json"), "not json").unwrap();
            let result = read_project_meta("ANY").unwrap();
            assert!(result.is_none(), "garbage data should return None");

            // Valid JSON, wrong shape
            std::fs::write(dir.join("project_meta.json"), r#"{"unexpected": true}"#).unwrap();
            let result = read_project_meta("ANY").unwrap();
            assert!(result.is_none(), "wrong-shape JSON should return None");
        });
    }
```

- [ ] **Step 4: Run the new tests to verify they fail**

Run: `~/.cargo/bin/cargo test --lib -- cache::tests::corrupt_team_cache_returns_none cache::tests::corrupt_workspace_cache_returns_none cache::tests::corrupt_project_meta_returns_none 2>&1`

Expected: 3 FAILURES. `corrupt_team_cache_returns_none` and `corrupt_workspace_cache_returns_none` fail because `read_team_cache` / `read_workspace_cache` use `?` on `serde_json::from_str`, propagating the error instead of returning `Ok(None)`. `corrupt_project_meta_returns_none` fails for the same reason on `read_project_meta`.

- [ ] **Step 5: Commit the failing tests**

```bash
git add src/cache.rs
git commit -m "test: add corrupt-file tests for team, workspace, project_meta caches (#104)"
```

---

### Task 2: Add `Expiring` trait and generic `read_cache` / `write_cache` functions

Introduce the core abstractions. No callers changed yet — this is additive only.

**Files:**
- Modify: `src/cache.rs:1-10` (imports and top of file)

- [ ] **Step 1: Add the `Expiring` trait after the `CACHE_TTL_DAYS` constant**

Insert after line 7 (`const CACHE_TTL_DAYS: i64 = 7;`):

```rust
/// Implemented by cache structs that carry a timestamp for TTL checks.
pub(crate) trait Expiring {
    fn fetched_at(&self) -> DateTime<Utc>;
}
```

- [ ] **Step 2: Add `DeserializeOwned` to imports**

Change the serde import line from:

```rust
use serde::{Deserialize, Serialize};
```

to:

```rust
use serde::{de::DeserializeOwned, Deserialize, Serialize};
```

- [ ] **Step 3: Add `read_cache` generic function after the `Expiring` trait**

Insert after the `Expiring` trait definition:

```rust
/// Read a whole-file cache. Returns `Ok(None)` on missing, expired, or corrupt files.
fn read_cache<T: DeserializeOwned + Expiring>(filename: &str) -> Result<Option<T>> {
    let path = cache_dir().join(filename);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let cache: T = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => return Ok(None),
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

- [ ] **Step 4: Verify existing tests still pass (no regressions)**

Run: `~/.cargo/bin/cargo test --lib -- cache::tests 2>&1`

Expected: The 19 existing tests pass (the 3 new corrupt tests still fail — that's expected). The new trait and functions are unused so far, which is fine — no dead-code warning because they'll be used in the next task.

- [ ] **Step 5: Verify it compiles cleanly with clippy**

Run: `~/.cargo/bin/cargo clippy --lib -- -D warnings 2>&1`

Expected: Clean (possible dead-code warning for `read_cache`/`write_cache` — if so, add `#[allow(dead_code)]` temporarily and remove in Task 3).

- [ ] **Step 6: Commit**

```bash
git add src/cache.rs
git commit -m "refactor: add Expiring trait and generic read_cache/write_cache (#104)"
```

---

### Task 3: Migrate 3 whole-file caches to use generics

Replace the bodies of the 6 whole-file read/write functions with calls to `read_cache`/`write_cache`. Add `Expiring` impls for each struct.

**Files:**
- Modify: `src/cache.rs:9-190` (struct definitions and function bodies)

- [ ] **Step 1: Add `Expiring` impl for `TeamCache`**

Insert immediately after the `TeamCache` struct definition (after line 19):

```rust
impl Expiring for TeamCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}
```

- [ ] **Step 2: Replace `read_team_cache` body**

Replace the entire `read_team_cache` function:

```rust
pub fn read_team_cache() -> Result<Option<TeamCache>> {
    read_cache("teams.json")
}
```

- [ ] **Step 3: Replace `write_team_cache` body**

Replace the entire `write_team_cache` function:

```rust
pub fn write_team_cache(teams: &[CachedTeam]) -> Result<()> {
    write_cache(
        "teams.json",
        &TeamCache {
            fetched_at: Utc::now(),
            teams: teams.to_vec(),
        },
    )
}
```

- [ ] **Step 4: Add `Expiring` impl for `WorkspaceCache`**

Insert immediately after the `WorkspaceCache` struct definition:

```rust
impl Expiring for WorkspaceCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}
```

- [ ] **Step 5: Replace `read_workspace_cache` body**

Replace the entire `read_workspace_cache` function:

```rust
pub fn read_workspace_cache() -> Result<Option<WorkspaceCache>> {
    read_cache("workspace.json")
}
```

- [ ] **Step 6: Replace `write_workspace_cache` body**

Replace the entire `write_workspace_cache` function:

```rust
pub fn write_workspace_cache(workspace_id: &str) -> Result<()> {
    write_cache(
        "workspace.json",
        &WorkspaceCache {
            workspace_id: workspace_id.to_string(),
            fetched_at: Utc::now(),
        },
    )
}
```

- [ ] **Step 7: Add `Expiring` impl for `CmdbFieldsCache`**

Insert immediately after the `CmdbFieldsCache` struct definition:

```rust
impl Expiring for CmdbFieldsCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}
```

- [ ] **Step 8: Replace `read_cmdb_fields_cache` body**

Replace the entire `read_cmdb_fields_cache` function:

```rust
pub fn read_cmdb_fields_cache() -> Result<Option<CmdbFieldsCache>> {
    read_cache("cmdb_fields.json")
}
```

- [ ] **Step 9: Replace `write_cmdb_fields_cache` body**

Replace the entire `write_cmdb_fields_cache` function:

```rust
pub fn write_cmdb_fields_cache(fields: &[(String, String)]) -> Result<()> {
    write_cache(
        "cmdb_fields.json",
        &CmdbFieldsCache {
            fields: fields.to_vec(),
            fetched_at: Utc::now(),
        },
    )
}
```

- [ ] **Step 10: Run all cache tests**

Run: `~/.cargo/bin/cargo test --lib -- cache::tests 2>&1`

Expected: All 19 original tests PASS. The 3 new corrupt tests from Task 1 now also PASS (22 total pass). The `read_cache` generic function handles corrupt files as `Ok(None)`, which is exactly what those tests assert.

- [ ] **Step 11: Run clippy**

Run: `~/.cargo/bin/cargo clippy --lib -- -D warnings 2>&1`

Expected: Clean. Remove any `#[allow(dead_code)]` added in Task 2 if present.

- [ ] **Step 12: Commit**

```bash
git add src/cache.rs
git commit -m "refactor: migrate 3 whole-file caches to generic read_cache/write_cache (#104)"
```

---

### Task 4: Normalize corrupt-file handling in keyed caches

Fix `read_project_meta` to treat deserialization errors as cache misses. `read_object_type_attr_cache` already does this. Add doc comments explaining why these functions are not genericized.

**Files:**
- Modify: `src/cache.rs` (keyed cache functions)

- [ ] **Step 1: Fix `read_project_meta` corrupt-file handling and add doc comment**

Replace the entire `read_project_meta` function with:

```rust
/// Read cached project metadata for a specific project key.
///
/// Keyed cache — not genericized because TTL is checked per-entry
/// (`ProjectMeta.fetched_at`), unlike whole-file caches.
pub fn read_project_meta(project_key: &str) -> Result<Option<ProjectMeta>> {
    let path = cache_dir().join("project_meta.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let map: HashMap<String, ProjectMeta> = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };

    match map.get(project_key) {
        Some(meta) => {
            let age = Utc::now() - meta.fetched_at;
            if age.num_days() >= CACHE_TTL_DAYS {
                Ok(None)
            } else {
                Ok(Some(meta.clone()))
            }
        }
        None => Ok(None),
    }
}
```

The key changes from the original:
- `serde_json::from_str(&content)?` becomes `match serde_json::from_str(...) { Ok(m) => m, Err(_) => return Ok(None) }`
- Doc comment added explaining why this is not genericized

- [ ] **Step 2: Add doc comment to `write_project_meta`**

Add a doc comment above the existing `write_project_meta` function:

```rust
/// Write cached project metadata for a specific project key.
///
/// Merges into the existing map file, preserving entries for other projects.
pub fn write_project_meta(project_key: &str, meta: &ProjectMeta) -> Result<()> {
```

The function body stays unchanged.

- [ ] **Step 3: Add doc comment to `read_object_type_attr_cache`**

Add a doc comment above the existing `read_object_type_attr_cache` function:

```rust
/// Read cached attributes for a specific object type.
///
/// Keyed cache — not genericized because TTL is checked per-file
/// (`ObjectTypeAttrCache.fetched_at`) but lookup is per-key, with a different
/// return type (`Vec<CachedObjectTypeAttr>`) than the stored wrapper struct.
pub fn read_object_type_attr_cache(
```

The function body stays unchanged.

- [ ] **Step 4: Add doc comment to `write_object_type_attr_cache`**

Add a doc comment above the existing `write_object_type_attr_cache` function:

```rust
/// Write cached attributes for a specific object type.
///
/// Merges into the existing map file, preserving entries for other object types.
pub fn write_object_type_attr_cache(
```

The function body stays unchanged.

- [ ] **Step 5: Run all cache tests**

Run: `~/.cargo/bin/cargo test --lib -- cache::tests 2>&1`

Expected: All 22 tests PASS. The `corrupt_project_meta_returns_none` test from Task 1 now passes because `read_project_meta` returns `Ok(None)` on corrupt data.

- [ ] **Step 6: Run clippy**

Run: `~/.cargo/bin/cargo clippy -- -D warnings 2>&1`

Expected: Clean.

- [ ] **Step 7: Commit**

```bash
git add src/cache.rs
git commit -m "fix: normalize corrupt-file handling in keyed caches, add doc comments (#104)"
```

---

### Task 5: Final validation

Run the full test suite and clippy to ensure nothing is broken across the entire crate.

**Files:** None (read-only verification)

- [ ] **Step 1: Run the full test suite**

Run: `~/.cargo/bin/cargo test 2>&1`

Expected: All tests pass — unit tests, integration tests, proptests, snapshot tests.

- [ ] **Step 2: Run clippy on the full crate**

Run: `~/.cargo/bin/cargo clippy -- -D warnings 2>&1`

Expected: Clean, zero warnings.

- [ ] **Step 3: Run format check**

Run: `~/.cargo/bin/cargo fmt --all -- --check 2>&1`

Expected: Clean, no formatting issues.

- [ ] **Step 4: Verify line count reduction**

Run: `wc -l src/cache.rs`

Expected: Roughly 550-580 lines total (down from 637). Production code portion should be ~190 lines (down from ~265).
