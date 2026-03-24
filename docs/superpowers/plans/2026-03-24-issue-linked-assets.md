# Issue Linked Assets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose CMDB/Assets objects linked to Jira issues via `jr issue view`, `jr issue list --assets`, and `jr issue assets KEY`.

**Architecture:** Auto-discover CMDB custom field IDs from `GET /rest/api/3/field` (cached 7-day TTL). Parse CMDB field values adaptively from issue responses (handles both modern `{label, objectKey}` and legacy `{workspaceId, objectId}` shapes). Enrich with Assets API when only IDs are available.

**Tech Stack:** Rust, serde_json::Value for adaptive parsing, futures::future::join_all for parallel enrichment, wiremock for integration tests.

**Spec:** `docs/superpowers/specs/2026-03-24-issue-linked-assets-design.md`

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src/types/assets/linked.rs` | `LinkedAsset` struct + display formatting + JSON serialization |
| `src/api/assets/linked.rs` | Cache orchestration (`get_or_fetch_cmdb_field_ids`), adaptive parsing (`extract_linked_assets`), parallel enrichment (`enrich_assets`) |
| `src/cli/issue/assets.rs` | `handle_issue_assets()` command handler for `jr issue assets KEY` |
| `tests/cmdb_fields.rs` | Integration tests for field discovery + linked asset parsing + enrichment |

### Modified Files

| File | Change |
|------|--------|
| `src/types/assets/mod.rs` | Add `pub mod linked;` and re-export |
| `src/api/assets/mod.rs` | Add `pub mod linked;` |
| `src/api/jira/fields.rs` | Add `find_cmdb_field_ids()` |
| `src/cache.rs` | Add `CmdbFieldsCache` struct + read/write functions |
| `src/cli/mod.rs` | Add `Assets` variant to `IssueCommand`, add `--assets` flag to `List` |
| `src/cli/issue/mod.rs` | Add `mod assets;` and wire up dispatch |
| `src/cli/issue/list.rs` | Add Assets row to `handle_view`, Assets column to `handle_list` |
| `src/cli/issue/format.rs` | Add `format_issue_row_with_assets()` and headers variant |

---

### Task 1: LinkedAsset Type

**Files:**
- Create: `src/types/assets/linked.rs`
- Modify: `src/types/assets/mod.rs`

- [ ] **Step 1: Write the LinkedAsset struct and display formatting**

```rust
// src/types/assets/linked.rs
use serde::Serialize;

/// An asset reference extracted from a CMDB custom field on a Jira issue.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LinkedAsset {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub asset_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

impl LinkedAsset {
    /// Human-readable display: "OBJ-1 (Acme Corp)", "OBJ-1", or "#12345 (run "jr init" to resolve asset names)".
    pub fn display(&self) -> String {
        match (&self.key, &self.name) {
            (Some(key), Some(name)) => format!("{} ({})", key, name),
            (Some(key), None) => key.clone(),
            (None, Some(name)) => name.clone(),
            (None, None) => match &self.id {
                Some(id) => format!("#{} (run \"jr init\" to resolve asset names)", id),
                None => "(unknown)".into(),
            },
        }
    }
}

/// Format a list of linked assets for display in a table cell.
pub fn format_linked_assets(assets: &[LinkedAsset]) -> String {
    if assets.is_empty() {
        return "(none)".into();
    }
    assets
        .iter()
        .map(|a| a.display())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format for list table: first asset + count if multiple.
pub fn format_linked_assets_short(assets: &[LinkedAsset]) -> String {
    match assets.len() {
        0 => "-".into(),
        1 => assets[0].display(),
        n => format!("{} (+{} more)", assets[0].display(), n - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_key_and_name() {
        let a = LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Acme Corp".into()),
            ..Default::default()
        };
        assert_eq!(a.display(), "OBJ-1 (Acme Corp)");
    }

    #[test]
    fn display_key_only() {
        let a = LinkedAsset {
            key: Some("OBJ-1".into()),
            ..Default::default()
        };
        assert_eq!(a.display(), "OBJ-1");
    }

    #[test]
    fn display_name_only() {
        let a = LinkedAsset {
            name: Some("Acme Corp".into()),
            ..Default::default()
        };
        assert_eq!(a.display(), "Acme Corp");
    }

    #[test]
    fn display_id_fallback_with_hint() {
        let a = LinkedAsset {
            id: Some("12345".into()),
            ..Default::default()
        };
        assert_eq!(
            a.display(),
            "#12345 (run \"jr init\" to resolve asset names)"
        );
    }

    #[test]
    fn display_nothing() {
        let a = LinkedAsset::default();
        assert_eq!(a.display(), "(unknown)");
    }

    #[test]
    fn format_empty_list() {
        assert_eq!(format_linked_assets(&[]), "(none)");
    }

    #[test]
    fn format_single_asset() {
        let assets = vec![LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Acme".into()),
            ..Default::default()
        }];
        assert_eq!(format_linked_assets(&assets), "OBJ-1 (Acme)");
    }

    #[test]
    fn format_multiple_assets() {
        let assets = vec![
            LinkedAsset {
                key: Some("OBJ-1".into()),
                name: Some("Acme".into()),
                ..Default::default()
            },
            LinkedAsset {
                key: Some("OBJ-2".into()),
                name: Some("Globex".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            format_linked_assets(&assets),
            "OBJ-1 (Acme), OBJ-2 (Globex)"
        );
    }

    #[test]
    fn format_short_empty() {
        assert_eq!(format_linked_assets_short(&[]), "-");
    }

    #[test]
    fn format_short_single() {
        let assets = vec![LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Acme".into()),
            ..Default::default()
        }];
        assert_eq!(format_linked_assets_short(&assets), "OBJ-1 (Acme)");
    }

    #[test]
    fn format_short_multiple() {
        let assets = vec![
            LinkedAsset {
                key: Some("OBJ-1".into()),
                name: Some("Acme".into()),
                ..Default::default()
            },
            LinkedAsset {
                key: Some("OBJ-2".into()),
                ..Default::default()
            },
            LinkedAsset {
                key: Some("OBJ-3".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            format_linked_assets_short(&assets),
            "OBJ-1 (Acme) (+2 more)"
        );
    }

    #[test]
    fn serialize_json_skips_none() {
        let a = LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Acme".into()),
            ..Default::default()
        };
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json.get("key").unwrap(), "OBJ-1");
        assert_eq!(json.get("name").unwrap(), "Acme");
        assert!(json.get("id").is_none());
        assert!(json.get("workspace_id").is_none());
    }
}
```

- [ ] **Step 2: Register the module**

Add to `src/types/assets/mod.rs`:

```rust
pub mod linked;
pub mod object;
pub mod ticket;

pub use linked::*;
pub use object::*;
pub use ticket::*;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib types::assets::linked`
Expected: All 11 tests PASS

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 5: Commit**

```bash
git add src/types/assets/linked.rs src/types/assets/mod.rs
git commit -m "feat: add LinkedAsset type with display formatting"
```

---

### Task 2: CMDB Field Discovery

**Files:**
- Modify: `src/api/jira/fields.rs`

- [ ] **Step 1: Write the failing test**

Add to the existing `tests` module in `src/api/jira/fields.rs`:

```rust
#[test]
fn filter_cmdb_fields_finds_assets_type() {
    let fields = vec![make_field(
        "customfield_10191",
        "Client",
        true,
        "any",
        "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
    )];
    let result = filter_cmdb_fields(&fields);
    assert_eq!(result, vec!["customfield_10191"]);
}

#[test]
fn filter_cmdb_fields_ignores_non_cmdb() {
    let fields = vec![
        make_field(
            "customfield_10031",
            "Story Points",
            true,
            "number",
            "com.atlassian.jira.plugin.system.customfieldtypes:float",
        ),
        make_field(
            "customfield_10191",
            "Client",
            true,
            "any",
            "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
        ),
    ];
    let result = filter_cmdb_fields(&fields);
    assert_eq!(result, vec!["customfield_10191"]);
}

#[test]
fn filter_cmdb_fields_empty_when_no_cmdb() {
    let fields = vec![make_field(
        "customfield_10031",
        "Story Points",
        true,
        "number",
        "com.atlassian.jira.plugin.system.customfieldtypes:float",
    )];
    let result = filter_cmdb_fields(&fields);
    assert!(result.is_empty());
}

#[test]
fn filter_cmdb_fields_multiple() {
    let fields = vec![
        make_field(
            "customfield_10191",
            "Client",
            true,
            "any",
            "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
        ),
        make_field(
            "customfield_10245",
            "Server",
            true,
            "any",
            "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
        ),
    ];
    let result = filter_cmdb_fields(&fields);
    assert_eq!(result, vec!["customfield_10191", "customfield_10245"]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib api::jira::fields::tests::filter_cmdb`
Expected: FAIL — `filter_cmdb_fields` not found

- [ ] **Step 3: Implement filter_cmdb_fields and find_cmdb_field_ids**

Add to `src/api/jira/fields.rs` (after the existing `filter_story_points_fields` function):

```rust
const CMDB_SCHEMA_TYPE: &str = "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype";

pub fn filter_cmdb_fields(fields: &[Field]) -> Vec<String> {
    fields
        .iter()
        .filter(|f| {
            f.custom == Some(true)
                && f.schema
                    .as_ref()
                    .and_then(|s| s.custom.as_deref())
                    .map(|c| c == CMDB_SCHEMA_TYPE)
                    .unwrap_or(false)
        })
        .map(|f| f.id.clone())
        .collect()
}
```

Add to the `impl JiraClient` block:

```rust
pub async fn find_cmdb_field_ids(&self) -> Result<Vec<String>> {
    let fields = self.list_fields().await?;
    Ok(filter_cmdb_fields(&fields))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib api::jira::fields::tests::filter_cmdb`
Expected: All 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/jira/fields.rs
git commit -m "feat: add CMDB field discovery via schema.custom filter"
```

---

### Task 3: CMDB Fields Cache

**Files:**
- Modify: `src/cache.rs`

- [ ] **Step 1: Write the failing tests**

Add to the existing `tests` module in `src/cache.rs`:

```rust
#[test]
fn read_missing_cmdb_fields_cache_returns_none() {
    with_temp_cache(|| {
        let result = read_cmdb_fields_cache().unwrap();
        assert!(result.is_none());
    });
}

#[test]
fn write_then_read_cmdb_fields_cache() {
    with_temp_cache(|| {
        write_cmdb_fields_cache(&["customfield_10191".into(), "customfield_10245".into()])
            .unwrap();

        let cache = read_cmdb_fields_cache()
            .unwrap()
            .expect("should exist");
        assert_eq!(cache.field_ids, vec!["customfield_10191", "customfield_10245"]);
    });
}

#[test]
fn expired_cmdb_fields_cache_returns_none() {
    with_temp_cache(|| {
        let expired = CmdbFieldsCache {
            field_ids: vec!["customfield_10191".into()],
            fetched_at: Utc::now() - chrono::Duration::days(8),
        };
        let dir = cache_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let content = serde_json::to_string_pretty(&expired).unwrap();
        std::fs::write(dir.join("cmdb_fields.json"), content).unwrap();

        let result = read_cmdb_fields_cache().unwrap();
        assert!(result.is_none(), "expired cmdb fields cache should return None");
    });
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib cache::tests::cmdb`
Expected: FAIL — `CmdbFieldsCache` not found

- [ ] **Step 3: Implement CmdbFieldsCache**

Add to `src/cache.rs` (after the `WorkspaceCache` section):

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CmdbFieldsCache {
    pub field_ids: Vec<String>,
    pub fetched_at: DateTime<Utc>,
}

pub fn read_cmdb_fields_cache() -> Result<Option<CmdbFieldsCache>> {
    let path = cache_dir().join("cmdb_fields.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: CmdbFieldsCache = serde_json::from_str(&content)?;

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(Some(cache))
}

pub fn write_cmdb_fields_cache(field_ids: &[String]) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let cache = CmdbFieldsCache {
        field_ids: field_ids.to_vec(),
        fetched_at: Utc::now(),
    };

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(dir.join("cmdb_fields.json"), content)?;
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib cache::tests::cmdb`
Expected: All 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/cache.rs
git commit -m "feat: add CmdbFieldsCache with 7-day TTL"
```

---

### Task 4: Adaptive Parsing & Enrichment

**Files:**
- Create: `src/api/assets/linked.rs`
- Modify: `src/api/assets/mod.rs`

- [ ] **Step 1: Write unit tests for adaptive parsing**

Create `src/api/assets/linked.rs` with tests first:

```rust
use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;

use crate::api::assets::workspace::get_or_fetch_workspace_id;
use crate::api::client::JiraClient;
use crate::cache;
use crate::types::assets::LinkedAsset;

/// Get CMDB field IDs, using cache when available.
pub async fn get_or_fetch_cmdb_field_ids(client: &JiraClient) -> Result<Vec<String>> {
    if let Some(cached) = cache::read_cmdb_fields_cache()? {
        return Ok(cached.field_ids);
    }

    let field_ids = client.find_cmdb_field_ids().await?;
    let _ = cache::write_cmdb_fields_cache(&field_ids);
    Ok(field_ids)
}

/// Extract linked assets from issue extra fields using discovered CMDB field IDs.
pub fn extract_linked_assets(
    extra: &HashMap<String, Value>,
    cmdb_field_ids: &[String],
) -> Vec<LinkedAsset> {
    let mut assets = Vec::new();

    for field_id in cmdb_field_ids {
        let Some(value) = extra.get(field_id) else {
            continue;
        };
        if value.is_null() {
            continue;
        }

        match value {
            Value::Array(arr) => {
                for item in arr {
                    if let Some(asset) = parse_cmdb_value(item) {
                        assets.push(asset);
                    }
                }
            }
            Value::Object(_) => {
                if let Some(asset) = parse_cmdb_value(value) {
                    assets.push(asset);
                }
            }
            Value::String(s) => {
                assets.push(LinkedAsset {
                    name: Some(s.clone()),
                    ..Default::default()
                });
            }
            _ => {}
        }
    }

    assets
}

fn parse_cmdb_value(value: &Value) -> Option<LinkedAsset> {
    let obj = value.as_object()?;

    let label = obj.get("label").and_then(|v| v.as_str()).map(String::from);
    let object_key = obj
        .get("objectKey")
        .and_then(|v| v.as_str())
        .map(String::from);
    let object_id = obj.get("objectId").and_then(|v| {
        v.as_str()
            .map(String::from)
            .or_else(|| v.as_u64().map(|n| n.to_string()))
    });
    let workspace_id = obj
        .get("workspaceId")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Only create an asset if we got at least something useful.
    if label.is_none() && object_key.is_none() && object_id.is_none() {
        return None;
    }

    Some(LinkedAsset {
        key: object_key,
        name: label,
        asset_type: None,
        id: object_id,
        workspace_id,
    })
}

/// Enrich assets that only have IDs by fetching from the Assets API.
pub async fn enrich_assets(
    client: &JiraClient,
    assets: &mut [LinkedAsset],
) {
    // Only enrich assets that have an ID but are missing key/name.
    let needs_enrichment: Vec<usize> = assets
        .iter()
        .enumerate()
        .filter(|(_, a)| a.id.is_some() && a.key.is_none() && a.name.is_none())
        .map(|(i, _)| i)
        .collect();

    if needs_enrichment.is_empty() {
        return;
    }

    // Get workspace ID — required for Assets API calls.
    let workspace_id = match get_or_fetch_workspace_id(client).await {
        Ok(wid) => wid,
        Err(_) => return, // Degrade gracefully
    };

    let futures: Vec<_> = needs_enrichment
        .iter()
        .map(|&idx| {
            let wid = workspace_id.clone();
            let oid = assets[idx].id.clone().unwrap();
            async move {
                let result = client.get_asset(&wid, &oid, false).await;
                (idx, result)
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    for (idx, result) in results {
        if let Ok(obj) = result {
            assets[idx].key = Some(obj.object_key);
            assets[idx].name = Some(obj.label);
            assets[idx].asset_type = Some(obj.object_type.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_extra(field_id: &str, value: Value) -> HashMap<String, Value> {
        let mut map = HashMap::new();
        map.insert(field_id.to_string(), value);
        map
    }

    #[test]
    fn parse_modern_label_and_key() {
        let extra = make_extra(
            "customfield_10191",
            json!([{"label": "Acme Corp", "objectKey": "OBJ-1"}]),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].key.as_deref(), Some("OBJ-1"));
        assert_eq!(assets[0].name.as_deref(), Some("Acme Corp"));
        assert!(assets[0].id.is_none());
    }

    #[test]
    fn parse_legacy_ids_only() {
        let extra = make_extra(
            "customfield_10191",
            json!([{"workspaceId": "ws-1", "objectId": "88", "id": "ws-1:88"}]),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].id.as_deref(), Some("88"));
        assert_eq!(assets[0].workspace_id.as_deref(), Some("ws-1"));
        assert!(assets[0].key.is_none());
        assert!(assets[0].name.is_none());
    }

    #[test]
    fn parse_mixed_fields() {
        let extra = make_extra(
            "customfield_10191",
            json!([{
                "label": "Acme Corp",
                "objectKey": "OBJ-1",
                "workspaceId": "ws-1",
                "objectId": "88"
            }]),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].key.as_deref(), Some("OBJ-1"));
        assert_eq!(assets[0].name.as_deref(), Some("Acme Corp"));
        assert_eq!(assets[0].id.as_deref(), Some("88"));
    }

    #[test]
    fn parse_null_field_skipped() {
        let extra = make_extra("customfield_10191", Value::Null);
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert!(assets.is_empty());
    }

    #[test]
    fn parse_empty_array() {
        let extra = make_extra("customfield_10191", json!([]));
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert!(assets.is_empty());
    }

    #[test]
    fn parse_missing_field_skipped() {
        let extra = HashMap::new();
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert!(assets.is_empty());
    }

    #[test]
    fn parse_string_value_as_name() {
        let extra = make_extra("customfield_10191", json!("Some Asset"));
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].name.as_deref(), Some("Some Asset"));
    }

    #[test]
    fn parse_multiple_cmdb_fields() {
        let mut extra = HashMap::new();
        extra.insert(
            "customfield_10191".into(),
            json!([{"label": "Acme", "objectKey": "OBJ-1"}]),
        );
        extra.insert(
            "customfield_10245".into(),
            json!([{"label": "Server-1", "objectKey": "SRV-1"}]),
        );
        let field_ids = vec!["customfield_10191".into(), "customfield_10245".into()];
        let assets = extract_linked_assets(&extra, &field_ids);
        assert_eq!(assets.len(), 2);
    }

    #[test]
    fn parse_multiple_objects_in_array() {
        let extra = make_extra(
            "customfield_10191",
            json!([
                {"label": "Acme", "objectKey": "OBJ-1"},
                {"label": "Globex", "objectKey": "OBJ-2"}
            ]),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].name.as_deref(), Some("Acme"));
        assert_eq!(assets[1].name.as_deref(), Some("Globex"));
    }

    #[test]
    fn parse_single_object_not_array() {
        let extra = make_extra(
            "customfield_10191",
            json!({"label": "Acme", "objectKey": "OBJ-1"}),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].key.as_deref(), Some("OBJ-1"));
    }

    #[test]
    fn parse_empty_object_skipped() {
        let extra = make_extra("customfield_10191", json!([{}]));
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert!(assets.is_empty());
    }

    #[test]
    fn parse_numeric_object_id() {
        let extra = make_extra(
            "customfield_10191",
            json!([{"objectId": 88}]),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].id.as_deref(), Some("88"));
    }
}
```

- [ ] **Step 2: Register the module**

Add `pub mod linked;` to `src/api/assets/mod.rs`:

```rust
pub mod linked;
pub mod objects;
pub mod tickets;
pub mod workspace;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib api::assets::linked::tests`
Expected: All 12 tests PASS

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 5: Commit**

```bash
git add src/api/assets/linked.rs src/api/assets/mod.rs
git commit -m "feat: add adaptive CMDB field parsing and enrichment"
```

---

### Task 5: CLI — Add IssueCommand::Assets and --assets flag

**Files:**
- Modify: `src/cli/mod.rs`

- [ ] **Step 1: Add `Assets` variant to `IssueCommand` and `--assets` flag to `List`**

In `src/cli/mod.rs`, add to the `IssueCommand` enum:

After the existing `LinkTypes` variant, add:

```rust
/// Show assets linked to an issue
Assets {
    /// Issue key (e.g., FOO-123)
    key: String,
},
```

In the existing `List` variant, add the `--assets` flag after `points`:

```rust
/// Show linked assets column
#[arg(long)]
assets: bool,
```

- [ ] **Step 2: Do NOT commit yet** — the match will be non-exhaustive. Continue to Task 6 which adds the handler and dispatch in the same commit.

---

### Task 6: CLI — jr issue assets command handler

**Files:**
- Create: `src/cli/issue/assets.rs`
- Modify: `src/cli/issue/mod.rs`

- [ ] **Step 1: Create the command handler**

```rust
// src/cli/issue/assets.rs
use anyhow::Result;

use crate::api::assets::linked::{
    enrich_assets, extract_linked_assets, get_or_fetch_cmdb_field_ids,
};
use crate::api::client::JiraClient;
use crate::cli::OutputFormat;
use crate::error::JrError;
use crate::output;

pub(super) async fn handle_issue_assets(
    key: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let cmdb_field_ids = get_or_fetch_cmdb_field_ids(client).await?;

    if cmdb_field_ids.is_empty() {
        return Err(JrError::UserError(
            "No Assets custom fields found on this Jira instance. \
             Assets requires Jira Service Management Premium or Enterprise."
                .into(),
        )
        .into());
    }

    let extra_fields: Vec<&str> = cmdb_field_ids.iter().map(|s| s.as_str()).collect();
    let issue = client.get_issue(key, &extra_fields).await?;
    let mut assets = extract_linked_assets(&issue.fields.extra, &cmdb_field_ids);

    if assets.is_empty() {
        eprintln!("No assets linked to {}.", key);
        return Ok(());
    }

    enrich_assets(client, &mut assets).await;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&assets)?);
        }
        OutputFormat::Table => {
            let rows: Vec<Vec<String>> = assets
                .iter()
                .map(|a| {
                    vec![
                        a.key.clone().unwrap_or_else(|| {
                            a.id.as_ref()
                                .map(|id| format!("#{}", id))
                                .unwrap_or_else(|| "-".into())
                        }),
                        a.asset_type.clone().unwrap_or_else(|| "-".into()),
                        a.name.clone().unwrap_or_else(|| "-".into()),
                    ]
                })
                .collect();

            output::print_output(output_format, &["Key", "Type", "Name"], &rows, &assets)?;
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Wire up dispatch in mod.rs**

Modify `src/cli/issue/mod.rs`:

Add `mod assets;` at the top with the other module declarations.

Add to the `match command` block in the `handle` function:

```rust
IssueCommand::Assets { key } => {
    assets::handle_issue_assets(&key, output_format, client).await
}
```

- [ ] **Step 3: Run to verify it compiles**

Run: `cargo check`
Expected: Clean compile

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 5: Commit** (includes Task 5 CLI changes + Task 6 handler)

```bash
git add src/cli/mod.rs src/cli/issue/assets.rs src/cli/issue/mod.rs
git commit -m "feat: add jr issue assets command and --assets flag"
```

---

### Task 7: CLI — Assets row in jr issue view

**Files:**
- Modify: `src/cli/issue/list.rs`

- [ ] **Step 1: Add Assets row to handle_view**

In `src/cli/issue/list.rs`, modify the `handle_view` function.

Add at the top of the file:

```rust
use crate::api::assets::linked::{
    enrich_assets, extract_linked_assets, get_or_fetch_cmdb_field_ids,
};
use crate::types::assets::linked::format_linked_assets;
```

In `handle_view`, after the line that builds `extra` from `sp_field_id` (around line 258), add CMDB field discovery:

```rust
let cmdb_field_ids = get_or_fetch_cmdb_field_ids(client).await.unwrap_or_default();
let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
for f in &cmdb_field_ids {
    extra.push(f.as_str());
}
```

(Replace the existing `let extra: Vec<&str> = sp_field_id.iter().copied().collect();` line.)

Then after the Links row (around line 391, after `rows.push(vec!["Links".into(), links_display]);`), add:

```rust
if !cmdb_field_ids.is_empty() {
    let mut linked = extract_linked_assets(&issue.fields.extra, &cmdb_field_ids);
    enrich_assets(client, &mut linked).await;
    let display = if linked.is_empty() {
        "(none)".into()
    } else {
        format_linked_assets(&linked)
    };
    rows.push(vec!["Assets".into(), display]);
}
```

- [ ] **Step 2: Run to verify it compiles**

Run: `cargo check`
Expected: Clean compile

- [ ] **Step 3: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "feat: show linked assets row in jr issue view"
```

---

### Task 8: CLI — --assets column in jr issue list

**Files:**
- Modify: `src/cli/issue/list.rs`
- Modify: `src/cli/issue/format.rs`

- [ ] **Step 1: Extend format.rs to support assets column**

Modify the existing `format_issue_row` in `src/cli/issue/format.rs` to accept an optional assets parameter, avoiding duplication:

```rust
use crate::types::assets::LinkedAsset;
use crate::types::assets::linked::format_linked_assets_short;
```

Replace the existing `format_issue_row` signature and body:

```rust
/// Build a single table row for an issue, optionally including story points and/or assets.
pub fn format_issue_row(
    issue: &Issue,
    sp_field_id: Option<&str>,
    assets: Option<&[LinkedAsset]>,
) -> Vec<String> {
    let mut row = Vec::new();
    row.push(issue.key.clone());
    row.push(
        issue.fields.issue_type.as_ref().map(|t| t.name.clone()).unwrap_or_default(),
    );
    row.push(
        issue.fields.status.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
    );
    row.push(
        issue.fields.priority.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
    );
    if let Some(field_id) = sp_field_id {
        row.push(
            issue.fields.story_points(field_id).map(format_points).unwrap_or_else(|| "-".into()),
        );
    }
    row.push(
        issue.fields.assignee.as_ref().map(|a| a.display_name.clone()).unwrap_or_else(|| "Unassigned".into()),
    );
    if let Some(linked) = assets {
        row.push(format_linked_assets_short(linked));
    }
    row.push(issue.fields.summary.clone());
    row
}
```

Update `format_issue_rows_public` to pass `None` for the new parameter:

```rust
pub fn format_issue_rows_public(issues: &[Issue]) -> Vec<Vec<String>> {
    issues
        .iter()
        .map(|issue| format_issue_row(issue, None, None))
        .collect()
}
```

Update `issue_table_headers` to accept assets flag:

```rust
pub fn issue_table_headers(show_points: bool, show_assets: bool) -> Vec<&'static str> {
    let mut headers = vec!["Key", "Type", "Status", "Priority"];
    if show_points {
        headers.push("Points");
    }
    headers.push("Assignee");
    if show_assets {
        headers.push("Assets");
    }
    headers.push("Summary");
    headers
}
```

**Note:** All existing callers of `format_issue_row(issue, sp_field_id)` must be updated to `format_issue_row(issue, sp_field_id, None)`. All callers of `issue_table_headers(show_points)` must be updated to `issue_table_headers(show_points, false)`. Search for these with `cargo check` — the compiler will find them all.

- [ ] **Step 2: Update handle_list in list.rs**

In `handle_list`, extract the new `assets` flag from the command match:

Update the destructuring at the top of `handle_list` to include the new flag:

```rust
let IssueCommand::List {
    jql,
    status,
    team,
    limit,
    points: show_points,
    assets: show_assets,
} = command
```

After the `extra` fields setup, add CMDB field discovery when `--assets` is passed:

```rust
let cmdb_field_ids = if show_assets {
    let ids = get_or_fetch_cmdb_field_ids(client).await.unwrap_or_default();
    if ids.is_empty() {
        eprintln!(
            "warning: --assets ignored. No Assets custom fields found on this Jira instance."
        );
    }
    ids
} else {
    Vec::new()
};
for f in &cmdb_field_ids {
    extra.push(f.as_str());
}
```

After fetching issues and before building rows, extract and enrich assets for each issue:

```rust
let show_assets_col = show_assets && !cmdb_field_ids.is_empty();
let mut issue_assets: Vec<Vec<LinkedAsset>> = Vec::new();
if show_assets_col {
    for issue in &issues {
        let mut linked = extract_linked_assets(&issue.fields.extra, &cmdb_field_ids);
        enrich_assets(client, &mut linked).await;
        issue_assets.push(linked);
    }
}
```

Update the row building to pass assets:

```rust
let rows: Vec<Vec<String>> = issues
    .iter()
    .enumerate()
    .map(|(i, issue)| {
        let assets = if show_assets_col {
            Some(issue_assets[i].as_slice())
        } else {
            None
        };
        format::format_issue_row(issue, effective_sp, assets)
    })
    .collect();
```

Update the headers call to pass the assets flag:

```rust
let headers = format::issue_table_headers(
    effective_sp.is_some(),
    show_assets_col,
);
output::print_output(output_format, &headers, &rows, &issues)?;
```

- [ ] **Step 3: Run to verify it compiles**

Run: `cargo check`
Expected: Clean compile

- [ ] **Step 4: Run all existing tests**

Run: `cargo test`
Expected: All tests PASS (existing list tests still work)

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue/list.rs src/cli/issue/format.rs
git commit -m "feat: add --assets column to jr issue list"
```

---

### Task 9: Integration Tests

**Files:**
- Create: `tests/cmdb_fields.rs`

- [ ] **Step 1: Write integration tests**

```rust
#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fields_response_with_cmdb() -> serde_json::Value {
    json!([
        {
            "id": "summary",
            "name": "Summary",
            "custom": false,
            "schema": { "type": "string" }
        },
        {
            "id": "customfield_10191",
            "name": "Client",
            "custom": true,
            "schema": {
                "type": "any",
                "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
                "customId": 10191
            }
        },
        {
            "id": "customfield_10031",
            "name": "Story Points",
            "custom": true,
            "schema": {
                "type": "number",
                "custom": "com.atlassian.jira.plugin.system.customfieldtypes:float",
                "customId": 10031
            }
        }
    ])
}

fn fields_response_no_cmdb() -> serde_json::Value {
    json!([
        {
            "id": "summary",
            "name": "Summary",
            "custom": false,
            "schema": { "type": "string" }
        }
    ])
}

#[tokio::test]
async fn discover_cmdb_field_ids() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_response_with_cmdb()))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let ids = client.find_cmdb_field_ids().await.unwrap();
    assert_eq!(ids, vec!["customfield_10191"]);
}

#[tokio::test]
async fn discover_cmdb_field_ids_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_response_no_cmdb()))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let ids = client.find_cmdb_field_ids().await.unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn issue_with_modern_cmdb_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": "PROJ-1",
            "fields": {
                "summary": "Test issue",
                "customfield_10191": [
                    {
                        "label": "Acme Corp",
                        "objectKey": "OBJ-1"
                    }
                ]
            }
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let issue = client
        .get_issue("PROJ-1", &["customfield_10191"])
        .await
        .unwrap();

    let cmdb_ids = vec!["customfield_10191".to_string()];
    let assets =
        jr::api::assets::linked::extract_linked_assets(&issue.fields.extra, &cmdb_ids);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].key.as_deref(), Some("OBJ-1"));
    assert_eq!(assets[0].name.as_deref(), Some("Acme Corp"));
}

#[tokio::test]
async fn issue_with_null_cmdb_field() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": "PROJ-2",
            "fields": {
                "summary": "No assets",
                "customfield_10191": null
            }
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let issue = client
        .get_issue("PROJ-2", &["customfield_10191"])
        .await
        .unwrap();

    let cmdb_ids = vec!["customfield_10191".to_string()];
    let assets =
        jr::api::assets::linked::extract_linked_assets(&issue.fields.extra, &cmdb_ids);
    assert!(assets.is_empty());
}

#[tokio::test]
async fn enrichment_resolves_ids_to_names() {
    let server = MockServer::start().await;

    // Mock workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1, "start": 0, "limit": 25, "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    // Mock asset fetch
    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/88"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "88",
            "label": "Acme Corp",
            "objectKey": "OBJ-88",
            "objectType": { "id": "13", "name": "Client" }
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let mut assets = vec![jr::types::assets::LinkedAsset {
        id: Some("88".into()),
        workspace_id: Some("ws-123".into()),
        ..Default::default()
    }];

    jr::api::assets::linked::enrich_assets(&client, &mut assets).await;

    assert_eq!(assets[0].key.as_deref(), Some("OBJ-88"));
    assert_eq!(assets[0].name.as_deref(), Some("Acme Corp"));
    assert_eq!(assets[0].asset_type.as_deref(), Some("Client"));
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test linked_assets`
Expected: All 5 tests PASS

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests PASS (no regressions)

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 5: Commit**

```bash
git add tests/cmdb_fields.rs
git commit -m "test: add integration tests for linked assets"
```

---

### Task 10: Documentation & Final Verification

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update README command table**

In the Commands table in `README.md`, update the `jr issue view KEY` description and add the new command:

```
| `jr issue view KEY` | View issue details (includes story points, linked assets) |
```

Add after the `jr issue link-types` row:

```
| `jr issue assets KEY`          | Show assets linked to an issue                |
```

Add to the `jr issue list` row:

```
| `jr issue list` | List issues (smart defaults for scrum/kanban, `--team`, `--points`, `--assets`) |
```

- [ ] **Step 2: Update CLAUDE.md architecture**

In the `src/cli/issue/` section of CLAUDE.md, add:

```
│   │   ├── assets.rs    # linked assets (issue→asset lookup)
```

In the `src/api/assets/` section:

```
│   │   ├── linked.rs        # CMDB field discovery cache, adaptive parsing, enrichment
```

In the `src/types/assets/` section:

```
├── types/assets/        # Serde structs for Assets API responses (AssetObject, ConnectedTicket, LinkedAsset, etc.)
```

- [ ] **Step 3: Run full test suite one final time**

Run: `cargo test && cargo clippy --all --all-features --tests -- -D warnings && cargo fmt --all -- --check`
Expected: All pass, clean clippy, clean fmt

- [ ] **Step 4: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: add issue assets command to README and CLAUDE.md"
```
