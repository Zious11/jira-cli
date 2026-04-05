# Assets Search Attribute Names Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enrich `assets search --attributes` output with human-readable attribute names via per-object-type definitions, cached locally.

**Architecture:** Fetch attribute definitions from `GET /objecttype/{id}/attributes` (one call per unique object type, cached 7 days). Build a HashMap mapping `objectTypeAttributeId` → definition. Inject `objectTypeAttribute` with `name` and `position` into each search result attribute. Table output gains an "Attributes" column with inline `Name: Value` pairs.

**Tech Stack:** Rust, serde_json::Value manipulation, wiremock for integration tests, existing XDG cache infrastructure

---

### Task 1: Cache Layer — `ObjectTypeAttrCache` Read/Write

**Files:**
- Modify: `src/cache.rs`

- [ ] **Step 1: Write the failing tests for object type attribute cache**

Add these tests to the existing `mod tests` block at the bottom of `src/cache.rs` (after the `expired_cmdb_fields_cache_returns_none` test at line ~442):

```rust
#[test]
fn read_missing_object_type_attr_cache_returns_none() {
    with_temp_cache(|| {
        let result = read_object_type_attr_cache("23").unwrap();
        assert!(result.is_none());
    });
}

#[test]
fn write_then_read_object_type_attr_cache() {
    with_temp_cache(|| {
        let attrs = vec![
            CachedObjectTypeAttr {
                id: "134".into(),
                name: "Key".into(),
                system: true,
                hidden: false,
                label: false,
                position: 0,
            },
            CachedObjectTypeAttr {
                id: "135".into(),
                name: "Name".into(),
                system: false,
                hidden: false,
                label: true,
                position: 1,
            },
        ];
        write_object_type_attr_cache("23", &attrs).unwrap();

        let loaded = read_object_type_attr_cache("23")
            .unwrap()
            .expect("should exist");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "Key");
        assert!(loaded[0].system);
        assert_eq!(loaded[1].name, "Name");
        assert!(loaded[1].label);
    });
}

#[test]
fn expired_object_type_attr_cache_returns_none() {
    with_temp_cache(|| {
        let expired = ObjectTypeAttrCache {
            fetched_at: Utc::now() - chrono::Duration::days(8),
            types: {
                let mut m = HashMap::new();
                m.insert(
                    "23".to_string(),
                    vec![CachedObjectTypeAttr {
                        id: "134".into(),
                        name: "Key".into(),
                        system: true,
                        hidden: false,
                        label: false,
                        position: 0,
                    }],
                );
                m
            },
        };
        let dir = cache_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let content = serde_json::to_string_pretty(&expired).unwrap();
        std::fs::write(dir.join("object_type_attrs.json"), content).unwrap();

        let result = read_object_type_attr_cache("23").unwrap();
        assert!(result.is_none(), "expired cache should return None");
    });
}

#[test]
fn object_type_attr_cache_multiple_types() {
    with_temp_cache(|| {
        let attrs_a = vec![CachedObjectTypeAttr {
            id: "134".into(),
            name: "Key".into(),
            system: true,
            hidden: false,
            label: false,
            position: 0,
        }];
        let attrs_b = vec![CachedObjectTypeAttr {
            id: "200".into(),
            name: "Hostname".into(),
            system: false,
            hidden: false,
            label: false,
            position: 3,
        }];
        write_object_type_attr_cache("23", &attrs_a).unwrap();
        write_object_type_attr_cache("45", &attrs_b).unwrap();

        let loaded_a = read_object_type_attr_cache("23")
            .unwrap()
            .expect("type 23 should exist");
        assert_eq!(loaded_a[0].name, "Key");

        let loaded_b = read_object_type_attr_cache("45")
            .unwrap()
            .expect("type 45 should exist");
        assert_eq!(loaded_b[0].name, "Hostname");
    });
}

#[test]
fn object_type_attr_cache_corrupt_returns_none() {
    with_temp_cache(|| {
        let dir = cache_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("object_type_attrs.json"), "not json").unwrap();

        let result = read_object_type_attr_cache("23").unwrap();
        assert!(result.is_none(), "corrupt cache should return None");
    });
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib cache::tests -- object_type_attr`
Expected: FAIL — `read_object_type_attr_cache`, `write_object_type_attr_cache`, `ObjectTypeAttrCache`, `CachedObjectTypeAttr` not found.

- [ ] **Step 3: Implement the cache types and read/write functions**

Add these types and functions to `src/cache.rs`, after the existing `CmdbFieldsCache` block (after line ~190, before `#[cfg(test)]`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedObjectTypeAttr {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub label: bool,
    #[serde(default)]
    pub position: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectTypeAttrCache {
    pub fetched_at: DateTime<Utc>,
    pub types: HashMap<String, Vec<CachedObjectTypeAttr>>,
}

pub fn read_object_type_attr_cache(object_type_id: &str) -> Result<Option<Vec<CachedObjectTypeAttr>>> {
    let path = cache_dir().join("object_type_attrs.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: ObjectTypeAttrCache = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(cache.types.get(object_type_id).cloned())
}

pub fn write_object_type_attr_cache(object_type_id: &str, attrs: &[CachedObjectTypeAttr]) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("object_type_attrs.json");

    let mut cache: ObjectTypeAttrCache = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or(ObjectTypeAttrCache {
            fetched_at: Utc::now(),
            types: HashMap::new(),
        })
    } else {
        ObjectTypeAttrCache {
            fetched_at: Utc::now(),
            types: HashMap::new(),
        }
    };

    cache.types.insert(object_type_id.to_string(), attrs.to_vec());
    cache.fetched_at = Utc::now();

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(&path, content)?;
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib cache::tests -- object_type_attr`
Expected: All 5 new tests PASS.

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 6: Commit**

```bash
git add src/cache.rs
git commit -m "feat: add object type attribute cache for search enrichment (#86)"
```

---

### Task 2: API Method — `get_object_type_attributes`

**Files:**
- Modify: `src/api/assets/objects.rs`
- Test: `tests/assets.rs`

- [ ] **Step 1: Write the failing integration test**

Add this test at the end of `tests/assets.rs`:

```rust
#[tokio::test]
async fn get_object_type_attributes_returns_definitions() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/23/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134",
                "name": "Key",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 0,
                "editable": false,
                "sortable": true
            },
            {
                "id": "135",
                "name": "Name",
                "system": false,
                "hidden": false,
                "label": true,
                "position": 1,
                "editable": true,
                "sortable": true
            },
            {
                "id": "140",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 5,
                "editable": true,
                "sortable": true
            }
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let attrs = client
        .get_object_type_attributes("ws-123", "23")
        .await
        .unwrap();

    assert_eq!(attrs.len(), 3);
    assert_eq!(attrs[0].id, "134");
    assert_eq!(attrs[0].name, "Key");
    assert!(attrs[0].system);
    assert_eq!(attrs[1].id, "135");
    assert_eq!(attrs[1].name, "Name");
    assert!(attrs[1].label);
    assert_eq!(attrs[2].id, "140");
    assert_eq!(attrs[2].name, "Location");
    assert_eq!(attrs[2].position, 5);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test assets get_object_type_attributes_returns_definitions`
Expected: FAIL — `get_object_type_attributes` method not found on `JiraClient`.

- [ ] **Step 3: Implement the API method**

Add this method to the `impl JiraClient` block in `src/api/assets/objects.rs`, after the existing `get_object_attributes` method (after line ~86):

```rust
/// Get all attribute definitions for an object type.
///
/// Returns schema-level metadata (name, system, hidden, label, position)
/// for every attribute defined on the type. Used to enrich search results
/// where only `objectTypeAttributeId` is present.
pub async fn get_object_type_attributes(
    &self,
    workspace_id: &str,
    object_type_id: &str,
) -> Result<Vec<ObjectTypeAttributeDef>> {
    let path = format!("objecttype/{}/attributes", urlencoding::encode(object_type_id));
    self.get_assets(workspace_id, &path).await
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test assets get_object_type_attributes_returns_definitions`
Expected: PASS.

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 6: Commit**

```bash
git add src/api/assets/objects.rs tests/assets.rs
git commit -m "feat: add get_object_type_attributes API method (#86)"
```

---

### Task 3: Enrichment Function — `enrich_search_attributes`

**Files:**
- Modify: `src/api/assets/objects.rs`
- Test: `tests/assets.rs`

- [ ] **Step 1: Write the failing integration test for enrichment**

Add this test at the end of `tests/assets.rs`:

```rust
#[tokio::test]
async fn enrich_search_attributes_injects_names() {
    let server = MockServer::start().await;

    // Mock: object type 13 attribute definitions
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/13/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134",
                "name": "Key",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 0
            },
            {
                "id": "140",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 5
            },
            {
                "id": "141",
                "name": "Secret",
                "system": false,
                "hidden": true,
                "label": false,
                "position": 6
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    // Simulate search results with inline attributes (no names)
    let mut objects = vec![jr::types::assets::AssetObject {
        id: "70".into(),
        label: "Acme Corp".into(),
        object_key: "OBJ-70".into(),
        object_type: jr::types::assets::ObjectType {
            id: "13".into(),
            name: "Client".into(),
            description: None,
        },
        created: None,
        updated: None,
        attributes: vec![
            jr::types::assets::AssetAttribute {
                id: "637".into(),
                object_type_attribute_id: "140".into(),
                values: vec![jr::types::assets::ObjectAttributeValue {
                    value: Some("New York".into()),
                    display_value: Some("New York".into()),
                }],
            },
            jr::types::assets::AssetAttribute {
                id: "638".into(),
                object_type_attribute_id: "141".into(),
                values: vec![jr::types::assets::ObjectAttributeValue {
                    value: Some("secret".into()),
                    display_value: Some("secret".into()),
                }],
            },
        ],
    }];

    let enriched = jr::api::assets::objects::enrich_search_attributes(
        &client, "ws-123", &mut objects,
    )
    .await
    .unwrap();

    // Returns the attribute definition map for use in output formatting
    assert!(enriched.contains_key("140"));
    assert_eq!(enriched["140"].name, "Location");
    assert!(enriched.contains_key("141"));
    assert_eq!(enriched["141"].name, "Secret");
    assert!(enriched["141"].hidden);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test assets enrich_search_attributes_injects_names`
Expected: FAIL — `enrich_search_attributes` function not found.

- [ ] **Step 3: Implement the enrichment function**

Add this function to `src/api/assets/objects.rs`, outside the `impl JiraClient` block (after the `resolve_object_key` function, before `#[cfg(test)]`):

```rust
use std::collections::HashMap;
use crate::cache::{self, CachedObjectTypeAttr};
use crate::types::assets::ObjectTypeAttributeDef;

/// Enrich search results by resolving attribute definitions for each unique object type.
///
/// Returns a HashMap mapping `objectTypeAttributeId` → `CachedObjectTypeAttr` for use
/// in output formatting (filtering system/hidden, sorting by position, displaying names).
///
/// Fetches definitions from cache first, falling back to the API. Results are cached
/// for 7 days per object type.
pub async fn enrich_search_attributes(
    client: &JiraClient,
    workspace_id: &str,
    objects: &mut [AssetObject],
) -> Result<HashMap<String, CachedObjectTypeAttr>> {
    // Collect unique object type IDs
    let mut type_ids: Vec<String> = objects
        .iter()
        .map(|o| o.object_type.id.clone())
        .collect();
    type_ids.sort();
    type_ids.dedup();

    let mut attr_map: HashMap<String, CachedObjectTypeAttr> = HashMap::new();

    for type_id in &type_ids {
        // Try cache first
        let attrs = match cache::read_object_type_attr_cache(type_id) {
            Ok(Some(cached)) => cached,
            _ => {
                // Cache miss — fetch from API
                match client.get_object_type_attributes(workspace_id, type_id).await {
                    Ok(defs) => {
                        let cached: Vec<CachedObjectTypeAttr> = defs
                            .iter()
                            .map(|d| CachedObjectTypeAttr {
                                id: d.id.clone(),
                                name: d.name.clone(),
                                system: d.system,
                                hidden: d.hidden,
                                label: d.label,
                                position: d.position,
                            })
                            .collect();
                        // Best-effort cache write
                        let _ = cache::write_object_type_attr_cache(type_id, &cached);
                        cached
                    }
                    Err(_) => {
                        // Graceful degradation: skip this type
                        eprintln!(
                            "Warning: could not fetch attribute definitions for object type {}",
                            type_id
                        );
                        continue;
                    }
                }
            }
        };

        for attr in attrs {
            attr_map.insert(attr.id.clone(), attr);
        }
    }

    Ok(attr_map)
}
```

- [ ] **Step 4: Make the function public in lib.rs**

Check that `src/api/assets/objects.rs` module and the function are accessible from integration tests. The function is already `pub` and the module path `jr::api::assets::objects` should be accessible via `src/lib.rs`. Verify by checking that `src/api/mod.rs` has `pub mod assets;` and `src/api/assets/mod.rs` has `pub mod objects;`.

Run: `cargo test --test assets enrich_search_attributes_injects_names`
Expected: PASS.

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 6: Commit**

```bash
git add src/api/assets/objects.rs tests/assets.rs
git commit -m "feat: add enrich_search_attributes function (#86)"
```

---

### Task 4: Update `handle_search` — JSON Enrichment

**Files:**
- Modify: `src/cli/assets.rs`
- Test: `tests/assets.rs`

- [ ] **Step 1: Write the failing integration test for enriched JSON output**

Add this test at the end of `tests/assets.rs`:

```rust
#[tokio::test]
async fn search_attributes_json_includes_names() {
    let server = MockServer::start().await;

    // Mock: AQL search with attributes
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("includeAttributes", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" },
                    "attributes": [
                        {
                            "id": "637",
                            "objectTypeAttributeId": "134",
                            "objectAttributeValues": [
                                { "value": "OBJ-70", "displayValue": "OBJ-70" }
                            ]
                        },
                        {
                            "id": "638",
                            "objectTypeAttributeId": "140",
                            "objectAttributeValues": [
                                { "value": "New York", "displayValue": "New York" }
                            ]
                        }
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    // Mock: object type attribute definitions
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/13/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134",
                "name": "Key",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 0
            },
            {
                "id": "140",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 5
            }
        ])))
        .mount(&server)
        .await;

    // Mock: workspace discovery (needed for CLI command)
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--output", "json",
            "assets", "search", "--attributes",
            "objectType = Client",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let objects = parsed.as_array().expect("array of objects");
    assert_eq!(objects.len(), 1);

    let attrs = objects[0]["attributes"].as_array().expect("attributes array");
    // System attribute (Key) should be filtered out
    // Only Location should remain
    assert_eq!(attrs.len(), 1);
    assert_eq!(attrs[0]["objectTypeAttribute"]["name"], "Location");
    assert_eq!(attrs[0]["objectTypeAttribute"]["position"], 5);
    assert_eq!(attrs[0]["objectAttributeValues"][0]["displayValue"], "New York");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test assets search_attributes_json_includes_names`
Expected: FAIL — current JSON output doesn't contain `objectTypeAttribute.name` and doesn't filter system attributes.

- [ ] **Step 3: Update `handle_search` in `src/cli/assets.rs`**

Replace the `handle_search` function (lines 57-100) with:

```rust
async fn handle_search(
    workspace_id: &str,
    query: &str,
    limit: Option<u32>,
    attributes: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let mut objects = client
        .search_assets(workspace_id, query, limit, attributes)
        .await?;

    if attributes {
        let attr_map =
            crate::api::assets::objects::enrich_search_attributes(client, workspace_id, &mut objects)
                .await?;

        match output_format {
            OutputFormat::Json => {
                // Serialize to Value, inject objectTypeAttribute, filter system/hidden
                let mut json_objects: Vec<serde_json::Value> = Vec::new();
                for obj in &objects {
                    let mut obj_value = serde_json::to_value(obj)?;
                    if let Some(attrs_array) = obj_value
                        .get_mut("attributes")
                        .and_then(|a| a.as_array_mut())
                    {
                        // Inject objectTypeAttribute into each attribute
                        for attr_value in attrs_array.iter_mut() {
                            if let Some(attr_id) = attr_value
                                .get("objectTypeAttributeId")
                                .and_then(|v| v.as_str())
                            {
                                if let Some(def) = attr_map.get(attr_id) {
                                    if let Some(map) = attr_value.as_object_mut() {
                                        map.insert(
                                            "objectTypeAttribute".to_string(),
                                            serde_json::json!({
                                                "name": def.name,
                                                "position": def.position,
                                            }),
                                        );
                                    }
                                }
                            }
                        }
                        // Filter out system and hidden attributes
                        attrs_array.retain(|attr| {
                            let attr_id = attr
                                .get("objectTypeAttributeId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            match attr_map.get(attr_id) {
                                Some(def) => !def.system && !def.hidden,
                                None => true, // keep unknown attributes
                            }
                        });
                        // Sort by position
                        attrs_array.sort_by_key(|attr| {
                            let attr_id = attr
                                .get("objectTypeAttributeId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            attr_map.get(attr_id).map(|d| d.position).unwrap_or(i32::MAX)
                        });
                    }
                    json_objects.push(obj_value);
                }
                println!("{}", output::render_json(&json_objects)?);
            }
            OutputFormat::Table => {
                let rows: Vec<Vec<String>> = objects
                    .iter()
                    .map(|o| {
                        let attr_str = format_inline_attributes(&o.attributes, &attr_map);
                        vec![
                            o.object_key.clone(),
                            o.object_type.name.clone(),
                            o.label.clone(),
                            attr_str,
                        ]
                    })
                    .collect();
                output::print_output(
                    output_format,
                    &["Key", "Type", "Name", "Attributes"],
                    &rows,
                    &objects,
                )?;
            }
        }
        Ok(())
    } else {
        let rows: Vec<Vec<String>> = objects
            .iter()
            .map(|o| {
                vec![
                    o.object_key.clone(),
                    o.object_type.name.clone(),
                    o.label.clone(),
                ]
            })
            .collect();
        output::print_output(output_format, &["Key", "Type", "Name"], &rows, &objects)
    }
}
```

- [ ] **Step 4: Add the `format_inline_attributes` helper**

Add this function to `src/cli/assets.rs`, before `handle_search` (or after it, before `handle_view`):

```rust
use crate::cache::CachedObjectTypeAttr;
use crate::types::assets::AssetAttribute;

/// Format attributes as inline `Name: Value` pairs for table display.
///
/// Filters out system, hidden, and label attributes. Sorts by position.
/// Multi-value attributes use the first displayValue (or value as fallback).
fn format_inline_attributes(
    attributes: &[AssetAttribute],
    attr_map: &std::collections::HashMap<String, CachedObjectTypeAttr>,
) -> String {
    let mut displayable: Vec<(&AssetAttribute, &CachedObjectTypeAttr)> = attributes
        .iter()
        .filter_map(|a| {
            attr_map.get(&a.object_type_attribute_id).and_then(|def| {
                if def.system || def.hidden || def.label {
                    None
                } else {
                    Some((a, def))
                }
            })
        })
        .collect();
    displayable.sort_by_key(|(_, def)| def.position);

    displayable
        .iter()
        .filter_map(|(attr, def)| {
            let value = attr.values.first().and_then(|v| {
                v.display_value
                    .as_deref()
                    .or(v.value.as_deref())
            });
            value.map(|v| format!("{}: {}", def.name, v))
        })
        .collect::<Vec<_>>()
        .join(" | ")
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test assets search_attributes_json_includes_names`
Expected: PASS.

- [ ] **Step 6: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 7: Commit**

```bash
git add src/cli/assets.rs tests/assets.rs
git commit -m "feat: enrich assets search JSON with attribute names (#86)"
```

---

### Task 5: Table Output Integration Test

**Files:**
- Test: `tests/assets.rs`

- [ ] **Step 1: Write the integration test for enriched table output**

Add this test at the end of `tests/assets.rs`:

```rust
#[tokio::test]
async fn search_attributes_table_shows_inline_values() {
    let server = MockServer::start().await;

    // Mock: AQL search with attributes
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("includeAttributes", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" },
                    "attributes": [
                        {
                            "id": "637",
                            "objectTypeAttributeId": "134",
                            "objectAttributeValues": [
                                { "value": "OBJ-70", "displayValue": "OBJ-70" }
                            ]
                        },
                        {
                            "id": "639",
                            "objectTypeAttributeId": "142",
                            "objectAttributeValues": [
                                { "value": "10", "displayValue": "10" }
                            ]
                        },
                        {
                            "id": "638",
                            "objectTypeAttributeId": "140",
                            "objectAttributeValues": [
                                { "value": "New York", "displayValue": "New York" }
                            ]
                        }
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    // Mock: object type attribute definitions
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objecttype/13/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "134",
                "name": "Key",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 0
            },
            {
                "id": "142",
                "name": "Seats",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 4
            },
            {
                "id": "140",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 5
            }
        ])))
        .mount(&server)
        .await;

    // Mock: workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "assets", "search", "--attributes",
            "objectType = Client",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Table should contain the Attributes column with inline values
    // Seats (position 4) comes before Location (position 5)
    assert!(
        stdout.contains("Seats: 10"),
        "Expected 'Seats: 10' in table, got: {stdout}"
    );
    assert!(
        stdout.contains("Location: New York"),
        "Expected 'Location: New York' in table, got: {stdout}"
    );
    // System attribute Key should NOT appear
    assert!(
        !stdout.contains("Key: OBJ-70"),
        "System attribute Key should be filtered, got: {stdout}"
    );
    // Should have Attributes header instead of Created/Updated
    assert!(
        stdout.contains("Attributes"),
        "Expected 'Attributes' header in table, got: {stdout}"
    );
    assert!(
        !stdout.contains("Created"),
        "Should not have Created column, got: {stdout}"
    );
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --test assets search_attributes_table_shows_inline_values`
Expected: PASS (the handler was already updated in Task 4).

- [ ] **Step 3: Run the full test suite**

Run: `cargo test`
Expected: All tests pass, including existing tests that should be unaffected.

- [ ] **Step 4: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 5: Commit**

```bash
git add tests/assets.rs
git commit -m "test: add integration test for enriched table output (#86)"
```

---

### Task 6: Documentation Updates

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

- [ ] **Step 1: Update CLAUDE.md architecture comment**

In `CLAUDE.md`, find the line describing `assets.rs`:
```
│   ├── assets.rs        # assets search/view/tickets (--open/--status client-side filtering)
```

Change it to:
```
│   ├── assets.rs        # assets search/view/tickets (--open/--status client-side filtering, search attribute enrichment)
```

- [ ] **Step 2: Update README.md assets search description**

In `README.md`, find the command table row for `jr assets search`:
```
| `jr assets search <AQL>`        | Search assets via AQL query                    |
```

Change it to:
```
| `jr assets search <AQL>`        | Search assets via AQL query (`--attributes` resolves names) |
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: update CLAUDE.md and README for search attribute enrichment (#86)"
```
