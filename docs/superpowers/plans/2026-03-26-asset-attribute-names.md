# Asset Attribute Names — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace raw numeric `Attribute ID` column in `jr assets view <KEY> --attributes` with human-readable attribute names.

**Architecture:** Add new serde types for the richer `/object/{id}/attributes` API response (which includes attribute names inline), add a new `get_object_attributes()` API method, and update the CLI `handle_view` to use the new endpoint with filtering/sorting. Existing types and methods remain unchanged for search and linked asset enrichment.

**Tech Stack:** Rust, serde, reqwest, comfy-table

**Spec:** `docs/superpowers/specs/2026-03-26-asset-attribute-names-design.md`

---

### Task 1: Add new serde types with tests

**Files:**
- Modify: `src/types/assets/object.rs:38` (add new types before `#[cfg(test)]` block)

- [ ] **Step 1: Write the failing tests**

In `src/types/assets/object.rs`, add these tests inside the existing `#[cfg(test)] mod tests` block (after line 86, before the closing `}`):

```rust
    #[test]
    fn deserialize_object_attribute_with_name() {
        let json = r#"{
            "id": "637",
            "objectTypeAttributeId": "134",
            "objectTypeAttribute": {
                "id": "134",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 4
            },
            "objectAttributeValues": [
                { "value": "New York, NY", "displayValue": "New York, NY" }
            ]
        }"#;
        let attr: ObjectAttribute = serde_json::from_str(json).unwrap();
        assert_eq!(attr.id, "637");
        assert_eq!(attr.object_type_attribute_id, "134");
        assert_eq!(attr.object_type_attribute.name, "Location");
        assert!(!attr.object_type_attribute.system);
        assert!(!attr.object_type_attribute.hidden);
        assert!(!attr.object_type_attribute.label);
        assert_eq!(attr.object_type_attribute.position, 4);
        assert_eq!(attr.values.len(), 1);
        assert_eq!(
            attr.values[0].display_value.as_deref(),
            Some("New York, NY")
        );
    }

    #[test]
    fn deserialize_object_attribute_defaults() {
        // system, hidden, label, position all absent — should default to false/0
        let json = r#"{
            "id": "640",
            "objectTypeAttributeId": "135",
            "objectTypeAttribute": {
                "id": "135",
                "name": "Name"
            },
            "objectAttributeValues": []
        }"#;
        let attr: ObjectAttribute = serde_json::from_str(json).unwrap();
        assert_eq!(attr.object_type_attribute.name, "Name");
        assert!(!attr.object_type_attribute.system);
        assert!(!attr.object_type_attribute.hidden);
        assert!(!attr.object_type_attribute.label);
        assert_eq!(attr.object_type_attribute.position, 0);
        assert!(attr.values.is_empty());
    }

    #[test]
    fn deserialize_object_attribute_system() {
        let json = r#"{
            "id": "638",
            "objectTypeAttributeId": "136",
            "objectTypeAttribute": {
                "id": "136",
                "name": "Created",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 2
            },
            "objectAttributeValues": [
                { "value": "2021-02-16T20:04:41.527Z", "displayValue": "16/Feb/21 8:04 PM" }
            ]
        }"#;
        let attr: ObjectAttribute = serde_json::from_str(json).unwrap();
        assert!(attr.object_type_attribute.system);
        assert_eq!(
            attr.values[0].display_value.as_deref(),
            Some("16/Feb/21 8:04 PM")
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib -- assets::object::tests::deserialize_object_attribute`
Expected: FAIL with "cannot find type `ObjectAttribute` in this scope"

- [ ] **Step 3: Implement the new types**

In `src/types/assets/object.rs`, add these types after line 38 (after the `ObjectAttributeValue` struct, before the `#[cfg(test)]` block):

```rust
/// A single attribute entry from `GET /object/{id}/attributes`.
/// Includes the full attribute definition with name, unlike `AssetAttribute`
/// which only has the numeric `objectTypeAttributeId`.
#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectAttribute {
    pub id: String,
    #[serde(rename = "objectTypeAttributeId")]
    pub object_type_attribute_id: String,
    #[serde(rename = "objectTypeAttribute")]
    pub object_type_attribute: ObjectTypeAttributeDef,
    #[serde(rename = "objectAttributeValues", default)]
    pub values: Vec<ObjectAttributeValue>,
}

/// Attribute definition from the object type schema.
#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectTypeAttributeDef {
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib -- assets::object::tests::deserialize_object_attribute`
Expected: All 3 tests PASS.

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 6: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: No warnings, no format issues.

- [ ] **Step 7: Commit**

```bash
git add src/types/assets/object.rs
git commit -m "feat: add ObjectAttribute and ObjectTypeAttributeDef types (#58)"
```

---

### Task 2: Add `get_object_attributes()` API method

**Files:**
- Modify: `src/api/assets/objects.rs:6` (add import)
- Modify: `src/api/assets/objects.rs:75` (add method after `get_asset`)

- [ ] **Step 1: Add the import**

In `src/api/assets/objects.rs`, change line 6:

```rust
use crate::types::assets::AssetObject;
```

To:

```rust
use crate::types::assets::{AssetObject, ObjectAttribute};
```

- [ ] **Step 2: Add the API method**

In `src/api/assets/objects.rs`, add this method inside the `impl JiraClient` block, after `get_asset` (after line 75, before the closing `}`):

```rust
    /// Get all attributes for a single object, with full attribute definitions
    /// including human-readable names.
    pub async fn get_object_attributes(
        &self,
        workspace_id: &str,
        object_id: &str,
    ) -> Result<Vec<ObjectAttribute>> {
        let path = format!("object/{}/attributes", urlencoding::encode(object_id));
        self.get_assets(workspace_id, &path).await
    }
```

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 4: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: No warnings, no format issues.

- [ ] **Step 5: Commit**

```bash
git add src/api/assets/objects.rs
git commit -m "feat: add get_object_attributes() API method (#58)"
```

---

### Task 3: Update `handle_view` to display attribute names

**Files:**
- Modify: `src/cli/assets.rs:94-95` (change `get_asset` call)
- Modify: `src/cli/assets.rs:98-141` (replace both `Json` and `Table` branches)

- [ ] **Step 1: Change `get_asset` to not fetch attributes**

In `src/cli/assets.rs`, replace line 94-95:

```rust
    let object = client
        .get_asset(workspace_id, &object_id, attributes)
```

With:

```rust
    let object = client
        .get_asset(workspace_id, &object_id, false)
```

- [ ] **Step 2: Replace the JSON output branch**

In `src/cli/assets.rs`, replace lines 99-101:

```rust
        OutputFormat::Json => {
            println!("{}", output::render_json(&object)?);
        }
```

With:

```rust
        OutputFormat::Json => {
            if attributes {
                let mut attrs = client
                    .get_object_attributes(workspace_id, &object_id)
                    .await?;
                // JSON: filter system and hidden only (keep label for programmatic consumers)
                attrs.retain(|a| {
                    !a.object_type_attribute.system
                        && !a.object_type_attribute.hidden
                });
                attrs.sort_by_key(|a| a.object_type_attribute.position);
                let combined = serde_json::json!({
                    "object": object,
                    "attributes": attrs,
                });
                println!("{}", serde_json::to_string_pretty(&combined)?);
            } else {
                println!("{}", output::render_json(&object)?);
            }
        }
```

- [ ] **Step 3: Replace the attribute rendering block in the Table branch**

In `src/cli/assets.rs`, replace lines 118-139:

```rust
            if attributes && !object.attributes.is_empty() {
                println!();
                let attr_rows: Vec<Vec<String>> = object
                    .attributes
                    .iter()
                    .flat_map(|attr| {
                        attr.values.iter().map(move |v| {
                            vec![
                                attr.object_type_attribute_id.clone(),
                                v.display_value
                                    .clone()
                                    .or_else(|| v.value.clone())
                                    .unwrap_or_default(),
                            ]
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    output::render_table(&["Attribute ID", "Value"], &attr_rows)
                );
            }
```

With:

```rust
            if attributes {
                let mut attrs = client
                    .get_object_attributes(workspace_id, &object_id)
                    .await?;
                attrs.retain(|a| {
                    !a.object_type_attribute.system
                        && !a.object_type_attribute.hidden
                        && !a.object_type_attribute.label
                });
                attrs.sort_by_key(|a| a.object_type_attribute.position);

                if !attrs.is_empty() {
                    println!();
                    let attr_rows: Vec<Vec<String>> = attrs
                        .iter()
                        .flat_map(|attr| {
                            attr.values.iter().map(move |v| {
                                vec![
                                    attr.object_type_attribute.name.clone(),
                                    v.display_value
                                        .clone()
                                        .or_else(|| v.value.clone())
                                        .unwrap_or_default(),
                                ]
                            })
                        })
                        .collect();
                    println!(
                        "{}",
                        output::render_table(&["Attribute", "Value"], &attr_rows)
                    );
                }
            }
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 5: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: No warnings, no format issues.

- [ ] **Step 6: Commit**

```bash
git add src/cli/assets.rs
git commit -m "feat: display attribute names instead of numeric IDs in assets view (#58)"
```

---

### Task 4: Add integration test for `get_object_attributes()`

**Files:**
- Modify: `tests/assets.rs:284` (add test at end of file)

- [ ] **Step 1: Write the integration test**

In `tests/assets.rs`, add this test after `get_connected_tickets_empty` (after line 284):

```rust
#[tokio::test]
async fn get_object_attributes_returns_named_attributes() {
    let server = MockServer::start().await;

    // Mock returns a mix of system, label, hidden, and user-defined attributes
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/object/88/attributes",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "637",
                "objectTypeAttributeId": "134",
                "objectTypeAttribute": {
                    "id": "134",
                    "name": "Key",
                    "system": true,
                    "hidden": false,
                    "label": false,
                    "position": 0
                },
                "objectAttributeValues": [
                    { "value": "OBJ-88", "displayValue": "OBJ-88" }
                ]
            },
            {
                "id": "640",
                "objectTypeAttributeId": "135",
                "objectTypeAttribute": {
                    "id": "135",
                    "name": "Name",
                    "system": false,
                    "hidden": false,
                    "label": true,
                    "position": 1
                },
                "objectAttributeValues": [
                    { "value": "Acme Corp", "displayValue": "Acme Corp" }
                ]
            },
            {
                "id": "641",
                "objectTypeAttributeId": "140",
                "objectTypeAttribute": {
                    "id": "140",
                    "name": "Location",
                    "system": false,
                    "hidden": false,
                    "label": false,
                    "position": 5
                },
                "objectAttributeValues": [
                    { "value": "New York, NY", "displayValue": "New York, NY" }
                ]
            },
            {
                "id": "642",
                "objectTypeAttributeId": "141",
                "objectTypeAttribute": {
                    "id": "141",
                    "name": "Internal Notes",
                    "system": false,
                    "hidden": true,
                    "label": false,
                    "position": 6
                },
                "objectAttributeValues": [
                    { "value": "secret", "displayValue": "secret" }
                ]
            },
            {
                "id": "643",
                "objectTypeAttributeId": "142",
                "objectTypeAttribute": {
                    "id": "142",
                    "name": "Seats",
                    "system": false,
                    "hidden": false,
                    "label": false,
                    "position": 4
                },
                "objectAttributeValues": [
                    { "value": "10", "displayValue": "10" }
                ]
            }
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let attrs = client
        .get_object_attributes("ws-123", "88")
        .await
        .unwrap();

    // All 5 attributes returned from API
    assert_eq!(attrs.len(), 5);

    // Verify attribute names are present
    assert_eq!(attrs[0].object_type_attribute.name, "Key");
    assert!(attrs[0].object_type_attribute.system);

    // Verify label attribute
    assert_eq!(attrs[1].object_type_attribute.name, "Name");
    assert!(attrs[1].object_type_attribute.label);

    // Verify hidden attribute
    assert_eq!(attrs[3].object_type_attribute.name, "Internal Notes");
    assert!(attrs[3].object_type_attribute.hidden);

    // Simulate the CLI filter: exclude system, hidden, label
    let mut visible: Vec<_> = attrs
        .into_iter()
        .filter(|a| {
            !a.object_type_attribute.system
                && !a.object_type_attribute.hidden
                && !a.object_type_attribute.label
        })
        .collect();
    visible.sort_by_key(|a| a.object_type_attribute.position);

    // Only user-defined, non-hidden attributes remain
    assert_eq!(visible.len(), 2);
    // Sorted by position: Seats (4) before Location (5)
    assert_eq!(visible[0].object_type_attribute.name, "Seats");
    assert_eq!(visible[0].object_type_attribute.position, 4);
    assert_eq!(visible[1].object_type_attribute.name, "Location");
    assert_eq!(visible[1].object_type_attribute.position, 5);

    // Verify displayValue is available
    assert_eq!(
        visible[1].values[0].display_value.as_deref(),
        Some("New York, NY")
    );
}
```

- [ ] **Step 2: Run the new test**

Run: `cargo test --test assets -- get_object_attributes_returns_named_attributes`
Expected: PASS

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/assets.rs
git commit -m "test: add integration test for get_object_attributes (#58)"
```

---

### Task 5: Final verification

**Files:**
- All modified files from Tasks 1-3

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Zero warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt --all -- --check`
Expected: No format issues.
