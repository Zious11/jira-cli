# Assets Schema Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `jr assets schemas`, `jr assets types`, and `jr assets schema` commands so users and AI agents can discover the Assets data model without guessing.

**Architecture:** Three new CLI subcommands backed by two new API methods (`list_object_schemas`, `list_object_types`) and an extended existing method (`get_object_type_attributes`). New serde types for schema/object-type responses in `src/types/assets/schema.rs`. Type resolution uses the existing `partial_match` module. The existing `AssetsPage` pagination struct is reused for the schema list endpoint.

**Tech Stack:** Rust, clap (CLI), serde (JSON), wiremock (integration tests), assert_cmd (CLI smoke tests), comfy-table (table output)

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/types/assets/schema.rs` | **New** — `ObjectSchema`, `ObjectTypeEntry` serde structs |
| `src/types/assets/object.rs` | Extend `ObjectTypeAttributeDef` with `default_type`, `reference_type`, `reference_object_type`, cardinality, `editable`, `description`, `options`; add `DefaultType`, `ReferenceType`, `ReferenceObjectType` structs |
| `src/types/assets/mod.rs` | Register new `schema` module |
| `src/api/assets/schemas.rs` | **New** — `list_object_schemas` (paginated), `list_object_types` (flat) |
| `src/api/assets/mod.rs` | Register new `schemas` module |
| `src/cli/mod.rs` | Add `Schemas`, `Types`, `Schema` variants to `AssetsCommand` |
| `src/cli/assets.rs` | Add `handle_schemas`, `handle_types`, `handle_schema` handlers + type display helper |
| `tests/assets.rs` | Integration tests for new API methods and CLI commands |
| `tests/cli_smoke.rs` | Smoke tests for new subcommands |
| `CLAUDE.md` | Update `assets.rs` description |
| `README.md` | Add new commands to table |

---

### Task 1: New serde types for schemas and object type entries

**Files:**
- Create: `src/types/assets/schema.rs`
- Modify: `src/types/assets/mod.rs:1-7`

- [ ] **Step 1: Write the failing test for ObjectSchema deserialization**

Add to the bottom of the new file `src/types/assets/schema.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Object schema from GET /objectschema/list.
#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectSchema {
    pub id: String,
    pub name: String,
    #[serde(rename = "objectSchemaKey")]
    pub object_schema_key: String,
    pub description: Option<String>,
    #[serde(rename = "objectCount", default)]
    pub object_count: i64,
    #[serde(rename = "objectTypeCount", default)]
    pub object_type_count: i64,
}

/// Object type entry from GET /objectschema/{id}/objecttypes/flat.
#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectTypeEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub position: i32,
    #[serde(rename = "objectCount", default)]
    pub object_count: i64,
    #[serde(rename = "objectSchemaId")]
    pub object_schema_id: String,
    #[serde(default)]
    pub inherited: bool,
    #[serde(rename = "abstractObjectType", default)]
    pub abstract_object_type: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_object_schema_full() {
        let json = r#"{
            "id": "6",
            "name": "ITSM",
            "objectSchemaKey": "ITSM",
            "status": "Ok",
            "description": "IT assets schema",
            "objectCount": 95,
            "objectTypeCount": 34
        }"#;
        let schema: ObjectSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.id, "6");
        assert_eq!(schema.name, "ITSM");
        assert_eq!(schema.object_schema_key, "ITSM");
        assert_eq!(schema.description.as_deref(), Some("IT assets schema"));
        assert_eq!(schema.object_count, 95);
        assert_eq!(schema.object_type_count, 34);
    }

    #[test]
    fn deserialize_object_schema_minimal() {
        let json = r#"{
            "id": "1",
            "name": "HR",
            "objectSchemaKey": "HR"
        }"#;
        let schema: ObjectSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.id, "1");
        assert_eq!(schema.name, "HR");
        assert!(schema.description.is_none());
        assert_eq!(schema.object_count, 0);
        assert_eq!(schema.object_type_count, 0);
    }

    #[test]
    fn deserialize_object_type_entry() {
        let json = r#"{
            "id": "19",
            "name": "Employee",
            "position": 0,
            "objectCount": 42,
            "objectSchemaId": "1",
            "inherited": false,
            "abstractObjectType": false,
            "parentObjectTypeInherited": false
        }"#;
        let entry: ObjectTypeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "19");
        assert_eq!(entry.name, "Employee");
        assert_eq!(entry.position, 0);
        assert_eq!(entry.object_count, 42);
        assert_eq!(entry.object_schema_id, "1");
        assert!(!entry.inherited);
        assert!(!entry.abstract_object_type);
        assert!(entry.description.is_none());
    }

    #[test]
    fn deserialize_object_type_entry_with_description() {
        let json = r#"{
            "id": "23",
            "name": "Office",
            "description": "Physical office or site.",
            "position": 2,
            "objectCount": 0,
            "objectSchemaId": "6",
            "inherited": false,
            "abstractObjectType": false
        }"#;
        let entry: ObjectTypeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.description.as_deref(), Some("Physical office or site."));
        assert_eq!(entry.position, 2);
    }
}
```

- [ ] **Step 2: Register the module**

In `src/types/assets/mod.rs`, add the `schema` module. The file should become:

```rust
pub mod linked;
pub mod object;
pub mod schema;
pub mod ticket;

pub use linked::*;
pub use object::*;
pub use schema::*;
pub use ticket::*;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib types::assets::schema`
Expected: 4 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/types/assets/schema.rs src/types/assets/mod.rs
git commit -m "feat(types): add ObjectSchema and ObjectTypeEntry serde structs (#87)"
```

---

### Task 2: Extend ObjectTypeAttributeDef with new fields

**Files:**
- Modify: `src/types/assets/object.rs:54-67`

- [ ] **Step 1: Write the failing test for DefaultType deserialization**

Add to `src/types/assets/object.rs`, inside the existing `mod tests` block, after the last test:

```rust
    #[test]
    fn deserialize_attribute_def_with_default_type() {
        let json = r#"{
            "id": "135",
            "name": "Name",
            "system": false,
            "hidden": false,
            "label": true,
            "position": 1,
            "defaultType": { "id": 0, "name": "Text" },
            "minimumCardinality": 1,
            "maximumCardinality": 1,
            "editable": true,
            "description": "The name of the object"
        }"#;
        let def: ObjectTypeAttributeDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "Name");
        assert!(def.label);
        let dt = def.default_type.unwrap();
        assert_eq!(dt.id, 0);
        assert_eq!(dt.name, "Text");
        assert_eq!(def.minimum_cardinality, 1);
        assert!(def.editable);
        assert_eq!(def.description.as_deref(), Some("The name of the object"));
        assert!(def.reference_type.is_none());
        assert!(def.reference_object_type.is_none());
    }

    #[test]
    fn deserialize_attribute_def_with_reference() {
        let json = r#"{
            "id": "869",
            "name": "Service relationships",
            "system": false,
            "hidden": false,
            "label": false,
            "position": 6,
            "referenceType": { "id": "36", "name": "Depends on" },
            "referenceObjectTypeId": "122",
            "referenceObjectType": { "id": "122", "name": "Service" },
            "minimumCardinality": 0,
            "maximumCardinality": -1,
            "editable": true
        }"#;
        let def: ObjectTypeAttributeDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "Service relationships");
        assert!(def.default_type.is_none());
        let rt = def.reference_type.unwrap();
        assert_eq!(rt.name, "Depends on");
        let rot = def.reference_object_type.unwrap();
        assert_eq!(rot.name, "Service");
        assert_eq!(def.minimum_cardinality, 0);
        assert_eq!(def.maximum_cardinality, -1);
    }

    #[test]
    fn deserialize_attribute_def_select_with_options() {
        let json = r#"{
            "id": "868",
            "name": "Tier",
            "system": false,
            "hidden": false,
            "label": false,
            "position": 5,
            "defaultType": { "id": 10, "name": "Select" },
            "minimumCardinality": 1,
            "maximumCardinality": 1,
            "editable": true,
            "options": "Tier 1,Tier 2,Tier 3"
        }"#;
        let def: ObjectTypeAttributeDef = serde_json::from_str(json).unwrap();
        let dt = def.default_type.unwrap();
        assert_eq!(dt.name, "Select");
        assert_eq!(def.options.as_deref(), Some("Tier 1,Tier 2,Tier 3"));
        assert_eq!(def.minimum_cardinality, 1);
    }

    #[test]
    fn deserialize_attribute_def_backward_compat() {
        // Existing JSON without the new fields — must still deserialize
        let json = r#"{
            "id": "134",
            "name": "Key",
            "system": true,
            "hidden": false,
            "label": false,
            "position": 0
        }"#;
        let def: ObjectTypeAttributeDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.id, "134");
        assert!(def.system);
        assert!(def.default_type.is_none());
        assert!(def.reference_type.is_none());
        assert!(def.reference_object_type.is_none());
        assert_eq!(def.minimum_cardinality, 0);
        assert_eq!(def.maximum_cardinality, 0);
        assert!(!def.editable);
        assert!(def.description.is_none());
        assert!(def.options.is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib types::assets::object::tests::deserialize_attribute_def_with_default_type`
Expected: FAIL — `ObjectTypeAttributeDef` does not have field `default_type`

- [ ] **Step 3: Add new structs and extend ObjectTypeAttributeDef**

In `src/types/assets/object.rs`, add after the existing `ObjectTypeAttributeDef` struct. The full struct becomes:

```rust
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
    #[serde(rename = "defaultType")]
    pub default_type: Option<DefaultType>,
    #[serde(rename = "referenceType")]
    pub reference_type: Option<ReferenceType>,
    #[serde(rename = "referenceObjectType")]
    pub reference_object_type: Option<ReferenceObjectType>,
    #[serde(rename = "minimumCardinality", default)]
    pub minimum_cardinality: i32,
    #[serde(rename = "maximumCardinality", default)]
    pub maximum_cardinality: i32,
    #[serde(default)]
    pub editable: bool,
    pub description: Option<String>,
    pub options: Option<String>,
}

/// Attribute data type (e.g., Text, DateTime, Select).
#[derive(Debug, Deserialize, Serialize)]
pub struct DefaultType {
    pub id: i32,
    pub name: String,
}

/// Reference link type (e.g., "Depends on", "References").
#[derive(Debug, Deserialize, Serialize)]
pub struct ReferenceType {
    pub id: String,
    pub name: String,
}

/// Target object type for a reference attribute (e.g., "Service", "Employee").
#[derive(Debug, Deserialize, Serialize)]
pub struct ReferenceObjectType {
    pub id: String,
    pub name: String,
}
```

- [ ] **Step 4: Run all tests to verify they pass**

Run: `cargo test --lib types::assets::object`
Expected: All tests PASS (both new and existing)

- [ ] **Step 5: Commit**

```bash
git add src/types/assets/object.rs
git commit -m "feat(types): extend ObjectTypeAttributeDef with defaultType, reference, cardinality (#87)"
```

---

### Task 3: New API methods for schema listing

**Files:**
- Create: `src/api/assets/schemas.rs`
- Modify: `src/api/assets/mod.rs:1-4`
- Test: `tests/assets.rs`

- [ ] **Step 1: Write the integration test for list_object_schemas**

Add to the bottom of `tests/assets.rs`:

```rust
#[tokio::test]
async fn list_object_schemas_returns_schemas() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/list",
        ))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "25"))
        .and(query_param("includeCounts", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 2,
            "isLast": true,
            "values": [
                {
                    "id": "6",
                    "name": "ITSM",
                    "objectSchemaKey": "ITSM",
                    "status": "Ok",
                    "objectCount": 95,
                    "objectTypeCount": 34
                },
                {
                    "id": "1",
                    "name": "Human Resources",
                    "objectSchemaKey": "HR",
                    "description": "HR schema",
                    "status": "Ok",
                    "objectCount": 1023,
                    "objectTypeCount": 14
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let schemas = client.list_object_schemas("ws-123").await.unwrap();
    assert_eq!(schemas.len(), 2);
    assert_eq!(schemas[0].name, "ITSM");
    assert_eq!(schemas[0].object_schema_key, "ITSM");
    assert_eq!(schemas[0].object_type_count, 34);
    assert_eq!(schemas[1].name, "Human Resources");
    assert_eq!(schemas[1].description.as_deref(), Some("HR schema"));
}

#[tokio::test]
async fn list_object_types_returns_flat_array() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/6/objecttypes/flat",
        ))
        .and(query_param("includeObjectCounts", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "19",
                "name": "Employee",
                "position": 0,
                "objectCount": 42,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            },
            {
                "id": "23",
                "name": "Office",
                "description": "Physical office or site.",
                "position": 2,
                "objectCount": 5,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            }
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let types = client.list_object_types("ws-123", "6").await.unwrap();
    assert_eq!(types.len(), 2);
    assert_eq!(types[0].name, "Employee");
    assert_eq!(types[0].object_count, 42);
    assert_eq!(types[1].name, "Office");
    assert_eq!(
        types[1].description.as_deref(),
        Some("Physical office or site.")
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test assets list_object_schemas_returns_schemas`
Expected: FAIL — `list_object_schemas` method does not exist

- [ ] **Step 3: Implement list_object_schemas and list_object_types**

Create `src/api/assets/schemas.rs`:

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::AssetsPage;
use crate::types::assets::{ObjectSchema, ObjectTypeEntry};

impl JiraClient {
    /// List all object schemas in the workspace with auto-pagination.
    pub async fn list_object_schemas(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ObjectSchema>> {
        let mut all = Vec::new();
        let mut start_at = 0u32;
        let page_size = 25u32;

        loop {
            let path = format!(
                "objectschema/list?startAt={}&maxResults={}&includeCounts=true",
                start_at, page_size
            );
            let page: AssetsPage<ObjectSchema> =
                self.get_assets(workspace_id, &path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values);

            if !has_more {
                break;
            }
            start_at = next;
        }
        Ok(all)
    }

    /// List all object types for a given schema (flat, no pagination).
    pub async fn list_object_types(
        &self,
        workspace_id: &str,
        schema_id: &str,
    ) -> Result<Vec<ObjectTypeEntry>> {
        let path = format!(
            "objectschema/{}/objecttypes/flat?includeObjectCounts=true",
            urlencoding::encode(schema_id)
        );
        self.get_assets(workspace_id, &path).await
    }
}
```

- [ ] **Step 4: Register the module**

In `src/api/assets/mod.rs`, add `pub mod schemas;`. The file should become:

```rust
pub mod linked;
pub mod objects;
pub mod schemas;
pub mod tickets;
pub mod workspace;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test assets list_object_schemas_returns_schemas list_object_types_returns_flat_array`
Expected: 2 tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/api/assets/schemas.rs src/api/assets/mod.rs tests/assets.rs
git commit -m "feat(api): add list_object_schemas and list_object_types methods (#87)"
```

---

### Task 4: CLI subcommand definitions

**Files:**
- Modify: `src/cli/mod.rs:107-142`
- Test: `tests/cli_smoke.rs`

- [ ] **Step 1: Write the smoke tests**

Add to the bottom of `tests/cli_smoke.rs`:

```rust
#[test]
fn test_assets_schemas_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["assets", "schemas", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List object schemas"));
}

#[test]
fn test_assets_types_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["assets", "types", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List object types"))
        .stdout(predicate::str::contains("--schema"));
}

#[test]
fn test_assets_schema_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["assets", "schema", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show attributes"))
        .stdout(predicate::str::contains("--schema"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test cli_smoke test_assets_schemas_help`
Expected: FAIL — no `schemas` subcommand

- [ ] **Step 3: Add the three new variants to AssetsCommand**

In `src/cli/mod.rs`, add inside `pub enum AssetsCommand` (after the `Tickets` variant):

```rust
    /// List object schemas in the workspace
    Schemas,
    /// List object types (all schemas or filtered)
    Types {
        /// Filter by schema (partial name match or exact ID)
        #[arg(long)]
        schema: Option<String>,
    },
    /// Show attributes for an object type
    Schema {
        /// Object type name (partial match supported)
        name: String,
        /// Filter by schema (partial name match or exact ID)
        #[arg(long)]
        schema: Option<String>,
    },
```

- [ ] **Step 4: Add stub match arms in handle()**

In `src/cli/assets.rs`, add match arms inside the `match command` block in `handle()`. Add them after the `AssetsCommand::Tickets` arm:

```rust
        AssetsCommand::Schemas => {
            handle_schemas(&workspace_id, output_format, client).await
        }
        AssetsCommand::Types { schema } => {
            handle_types(&workspace_id, schema, output_format, client).await
        }
        AssetsCommand::Schema { name, schema } => {
            handle_schema(&workspace_id, &name, schema, output_format, client).await
        }
```

Add stub handler functions at the bottom of the file (before `#[cfg(test)]`):

```rust
async fn handle_schemas(
    _workspace_id: &str,
    _output_format: &OutputFormat,
    _client: &JiraClient,
) -> Result<()> {
    todo!("handle_schemas")
}

async fn handle_types(
    _workspace_id: &str,
    _schema: Option<String>,
    _output_format: &OutputFormat,
    _client: &JiraClient,
) -> Result<()> {
    todo!("handle_types")
}

async fn handle_schema(
    _workspace_id: &str,
    _name: &str,
    _schema: Option<String>,
    _output_format: &OutputFormat,
    _client: &JiraClient,
) -> Result<()> {
    todo!("handle_schema")
}
```

- [ ] **Step 5: Run smoke tests to verify they pass**

Run: `cargo test --test cli_smoke test_assets_schemas_help test_assets_types_help test_assets_schema_help`
Expected: 3 tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs src/cli/assets.rs tests/cli_smoke.rs
git commit -m "feat(cli): add schemas, types, schema subcommand definitions (#87)"
```

---

### Task 5: Implement handle_schemas

**Files:**
- Modify: `src/cli/assets.rs`
- Test: `tests/assets.rs`

- [ ] **Step 1: Write the integration test for schemas JSON output**

Add to `tests/assets.rs`:

```rust
#[tokio::test]
async fn schemas_json_lists_all_schemas() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/list",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 2,
            "isLast": true,
            "values": [
                {
                    "id": "6",
                    "name": "ITSM",
                    "objectSchemaKey": "ITSM",
                    "status": "Ok",
                    "objectCount": 95,
                    "objectTypeCount": 34
                },
                {
                    "id": "1",
                    "name": "Human Resources",
                    "objectSchemaKey": "HR",
                    "status": "Ok",
                    "objectCount": 1023,
                    "objectTypeCount": 14
                }
            ]
        })))
        .mount(&server)
        .await;

    // Mock workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    let _guard = set_cache_dir(&tempfile::tempdir().unwrap().into_path()).await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["assets", "schemas", "--output", "json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "ITSM");
    assert_eq!(arr[0]["objectSchemaKey"], "ITSM");
    assert_eq!(arr[1]["name"], "Human Resources");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test assets schemas_json_lists_all_schemas`
Expected: FAIL — `todo!("handle_schemas")` panics

- [ ] **Step 3: Implement handle_schemas**

In `src/cli/assets.rs`, replace the `handle_schemas` stub:

```rust
async fn handle_schemas(
    workspace_id: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let schemas = client.list_object_schemas(workspace_id).await?;

    let rows: Vec<Vec<String>> = schemas
        .iter()
        .map(|s| {
            vec![
                s.id.clone(),
                s.object_schema_key.clone(),
                s.name.clone(),
                s.description.clone().unwrap_or_else(|| "\u{2014}".into()),
                s.object_type_count.to_string(),
                s.object_count.to_string(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "Key", "Name", "Description", "Types", "Objects"],
        &rows,
        &schemas,
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test assets schemas_json_lists_all_schemas`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/cli/assets.rs tests/assets.rs
git commit -m "feat(cli): implement handle_schemas for assets schemas command (#87)"
```

---

### Task 6: Implement handle_types

**Files:**
- Modify: `src/cli/assets.rs`
- Test: `tests/assets.rs`

- [ ] **Step 1: Write the integration test for types JSON output**

Add to `tests/assets.rs`. This test needs mocks for workspace discovery, schema list, and objecttypes/flat for each schema:

```rust
#[tokio::test]
async fn types_json_lists_all_types() {
    let server = MockServer::start().await;

    // Mock workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    // Mock schema list
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/list",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [{
                "id": "6",
                "name": "ITSM",
                "objectSchemaKey": "ITSM",
                "status": "Ok",
                "objectCount": 95,
                "objectTypeCount": 2
            }]
        })))
        .mount(&server)
        .await;

    // Mock object types for schema 6
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/6/objecttypes/flat",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "19",
                "name": "Employee",
                "position": 0,
                "objectCount": 42,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            },
            {
                "id": "23",
                "name": "Office",
                "description": "Physical office.",
                "position": 2,
                "objectCount": 5,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            }
        ])))
        .mount(&server)
        .await;

    let _guard = set_cache_dir(&tempfile::tempdir().unwrap().into_path()).await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["assets", "types", "--output", "json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Employee");
    assert_eq!(arr[0]["schemaName"], "ITSM");
    assert_eq!(arr[1]["name"], "Office");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test assets types_json_lists_all_types`
Expected: FAIL — `todo!("handle_types")` panics

- [ ] **Step 3: Implement handle_types**

In `src/cli/assets.rs`, replace the `handle_types` stub. Also add the `resolve_schema` helper that will be reused by `handle_schema`:

```rust
/// Resolve a --schema flag to a single schema, matching by ID (exact) or name (partial).
fn resolve_schema<'a>(
    input: &str,
    schemas: &'a [crate::types::assets::ObjectSchema],
) -> Result<&'a crate::types::assets::ObjectSchema> {
    // Try exact ID match first
    if let Some(s) = schemas.iter().find(|s| s.id == input) {
        return Ok(s);
    }
    // Partial match on name
    let names: Vec<String> = schemas.iter().map(|s| s.name.clone()).collect();
    match partial_match::partial_match(input, &names) {
        partial_match::MatchResult::Exact(name) => {
            Ok(schemas.iter().find(|s| s.name == name).unwrap())
        }
        partial_match::MatchResult::Ambiguous(matches) => Err(JrError::UserError(format!(
            "Ambiguous schema \"{}\". Matches: {}",
            input,
            matches.join(", ")
        ))
        .into()),
        partial_match::MatchResult::None(all) => {
            let available = if all.is_empty() {
                "none".to_string()
            } else {
                all.join(", ")
            };
            Err(JrError::UserError(format!(
                "No schema matching \"{}\". Available: {}",
                input, available
            ))
            .into())
        }
    }
}

async fn handle_types(
    workspace_id: &str,
    schema_filter: Option<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let schemas = client.list_object_schemas(workspace_id).await?;
    if schemas.is_empty() {
        return Err(
            JrError::UserError("No asset schemas found in this workspace.".into()).into(),
        );
    }

    let target_schemas: Vec<&crate::types::assets::ObjectSchema> = match &schema_filter {
        Some(input) => vec![resolve_schema(input, &schemas)?],
        None => schemas.iter().collect(),
    };

    // Build a map of schema_id → schema_name for injection
    let schema_names: std::collections::HashMap<&str, &str> = schemas
        .iter()
        .map(|s| (s.id.as_str(), s.name.as_str()))
        .collect();

    let mut all_types = Vec::new();
    for schema in &target_schemas {
        let types = client
            .list_object_types(workspace_id, &schema.id)
            .await?;
        all_types.extend(types);
    }

    match output_format {
        OutputFormat::Json => {
            // Inject schemaName into each entry
            let mut json_types: Vec<serde_json::Value> = Vec::new();
            for t in &all_types {
                let mut val = serde_json::to_value(t)?;
                if let Some(map) = val.as_object_mut() {
                    let schema_name = schema_names
                        .get(t.object_schema_id.as_str())
                        .unwrap_or(&"");
                    map.insert(
                        "schemaName".to_string(),
                        serde_json::Value::String(schema_name.to_string()),
                    );
                }
                json_types.push(val);
            }
            println!("{}", output::render_json(&json_types)?);
        }
        OutputFormat::Table => {
            let rows: Vec<Vec<String>> = all_types
                .iter()
                .map(|t| {
                    let schema_name = schema_names
                        .get(t.object_schema_id.as_str())
                        .unwrap_or(&"\u{2014}");
                    vec![
                        t.id.clone(),
                        t.name.clone(),
                        schema_name.to_string(),
                        t.description
                            .clone()
                            .unwrap_or_else(|| "\u{2014}".into()),
                        t.object_count.to_string(),
                    ]
                })
                .collect();

            output::print_output(
                output_format,
                &["ID", "Name", "Schema", "Description", "Objects"],
                &rows,
                &all_types,
            )?;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test assets types_json_lists_all_types`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/cli/assets.rs tests/assets.rs
git commit -m "feat(cli): implement handle_types for assets types command (#87)"
```

---

### Task 7: Implement handle_schema (attribute display)

**Files:**
- Modify: `src/cli/assets.rs`
- Test: `tests/assets.rs`

- [ ] **Step 1: Write the integration test for schema JSON output**

Add to `tests/assets.rs`:

```rust
#[tokio::test]
async fn schema_json_shows_attributes() {
    let server = MockServer::start().await;

    // Mock workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    // Mock schema list
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/list",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [{
                "id": "6",
                "name": "ITSM",
                "objectSchemaKey": "ITSM",
                "status": "Ok",
                "objectCount": 95,
                "objectTypeCount": 2
            }]
        })))
        .mount(&server)
        .await;

    // Mock object types for schema 6
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/6/objecttypes/flat",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "23",
                "name": "Office",
                "position": 2,
                "objectCount": 5,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            }
        ])))
        .mount(&server)
        .await;

    // Mock object type attributes
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
                "defaultType": { "id": 0, "name": "Text" },
                "minimumCardinality": 1,
                "maximumCardinality": 1,
                "editable": false
            },
            {
                "id": "135",
                "name": "Name",
                "system": false,
                "hidden": false,
                "label": true,
                "position": 1,
                "defaultType": { "id": 0, "name": "Text" },
                "minimumCardinality": 1,
                "maximumCardinality": 1,
                "editable": true,
                "description": "The name of the object"
            },
            {
                "id": "869",
                "name": "Service relationships",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 6,
                "referenceType": { "id": "36", "name": "Depends on" },
                "referenceObjectType": { "id": "122", "name": "Service" },
                "minimumCardinality": 0,
                "maximumCardinality": -1,
                "editable": true
            }
        ])))
        .mount(&server)
        .await;

    let _guard = set_cache_dir(&tempfile::tempdir().unwrap().into_path()).await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["assets", "schema", "Office", "--output", "json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().unwrap();
    // JSON includes all attributes (including system)
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0]["name"], "Key");
    assert_eq!(arr[0]["system"], true);
    assert_eq!(arr[2]["name"], "Service relationships");
    assert!(arr[2].get("referenceObjectType").is_some());
}

#[tokio::test]
async fn schema_table_filters_system_attrs() {
    let server = MockServer::start().await;

    // Mock workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    // Mock schema list
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/list",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [{
                "id": "6",
                "name": "ITSM",
                "objectSchemaKey": "ITSM",
                "status": "Ok",
                "objectCount": 95,
                "objectTypeCount": 1
            }]
        })))
        .mount(&server)
        .await;

    // Mock object types for schema 6
    Mock::given(method("GET"))
        .and(path(
            "/jsm/assets/workspace/ws-123/v1/objectschema/6/objecttypes/flat",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "23",
                "name": "Office",
                "position": 2,
                "objectCount": 5,
                "objectSchemaId": "6",
                "inherited": false,
                "abstractObjectType": false
            }
        ])))
        .mount(&server)
        .await;

    // Mock object type attributes — includes system "Key" and "Created"
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
                "defaultType": { "id": 0, "name": "Text" },
                "minimumCardinality": 1,
                "editable": false
            },
            {
                "id": "135",
                "name": "Name",
                "system": false,
                "hidden": false,
                "label": true,
                "position": 1,
                "defaultType": { "id": 0, "name": "Text" },
                "minimumCardinality": 1,
                "editable": true
            },
            {
                "id": "136",
                "name": "Created",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 2,
                "defaultType": { "id": 6, "name": "DateTime" },
                "minimumCardinality": 1,
                "editable": false
            }
        ])))
        .mount(&server)
        .await;

    let _guard = set_cache_dir(&tempfile::tempdir().unwrap().into_path()).await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["assets", "schema", "Office"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    // Table output should contain the header and "Name" but not "Key" or "Created" (system)
    assert!(stdout.contains("Object Type: Office"));
    assert!(stdout.contains("Name"));
    assert!(!stdout.contains("Created"));
    // "Key" appears in the header row (column name), so check for the system attribute row
    // by checking it only appears once (as header, not as data)
    let key_count = stdout.matches("Key").count();
    // Should not appear as a data row — only zero times or once in a non-data context
    assert!(key_count <= 1, "System attribute 'Key' should be filtered from table, but found {} occurrences", key_count);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test assets schema_json_shows_attributes`
Expected: FAIL — `todo!("handle_schema")` panics

- [ ] **Step 3: Implement the attribute type display helper**

In `src/cli/assets.rs`, add this helper function (above `handle_schema`):

```rust
/// Format the Type column for an attribute definition.
fn format_attribute_type(attr: &crate::types::assets::ObjectTypeAttributeDef) -> String {
    if let Some(ref dt) = attr.default_type {
        return dt.name.clone();
    }
    if let Some(ref rot) = attr.reference_object_type {
        return format!("Reference \u{2192} {}", rot.name);
    }
    "Unknown".to_string()
}
```

- [ ] **Step 4: Implement handle_schema**

In `src/cli/assets.rs`, replace the `handle_schema` stub:

```rust
async fn handle_schema(
    workspace_id: &str,
    type_name: &str,
    schema_filter: Option<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let schemas = client.list_object_schemas(workspace_id).await?;
    if schemas.is_empty() {
        return Err(
            JrError::UserError("No asset schemas found in this workspace.".into()).into(),
        );
    }

    let target_schemas: Vec<&crate::types::assets::ObjectSchema> = match &schema_filter {
        Some(input) => vec![resolve_schema(input, &schemas)?],
        None => schemas.iter().collect(),
    };

    // Collect all object types with their schema name
    let mut candidates: Vec<(crate::types::assets::ObjectTypeEntry, String)> = Vec::new();
    for schema in &target_schemas {
        let types = client
            .list_object_types(workspace_id, &schema.id)
            .await?;
        for t in types {
            candidates.push((t, schema.name.clone()));
        }
    }

    if candidates.is_empty() {
        return Err(JrError::UserError(
            "No object types found. Run \"jr assets schemas\" to verify your workspace has schemas."
                .into(),
        )
        .into());
    }

    // Partial match on type name
    let type_names: Vec<String> = candidates.iter().map(|(t, _)| t.name.clone()).collect();
    let matched_name = match partial_match::partial_match(type_name, &type_names) {
        partial_match::MatchResult::Exact(name) => name,
        partial_match::MatchResult::Ambiguous(matches) => {
            // Include schema name for disambiguation
            let labeled: Vec<String> = matches
                .iter()
                .filter_map(|m| {
                    candidates
                        .iter()
                        .find(|(t, _)| t.name == *m)
                        .map(|(t, s)| format!("{} ({})", t.name, s))
                })
                .collect();
            return Err(JrError::UserError(format!(
                "Ambiguous type \"{}\". Matches: {}. Use --schema to narrow results.",
                type_name,
                labeled.join(", ")
            ))
            .into());
        }
        partial_match::MatchResult::None(_) => {
            return Err(JrError::UserError(format!(
                "No object type matching \"{}\". Run \"jr assets types\" to see available types.",
                type_name
            ))
            .into());
        }
    };

    let (matched_type, schema_name) = candidates
        .iter()
        .find(|(t, _)| t.name == matched_name)
        .unwrap();

    // Fetch attributes
    let attrs = client
        .get_object_type_attributes(workspace_id, &matched_type.id)
        .await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&attrs)?);
        }
        OutputFormat::Table => {
            println!(
                "Object Type: {} (Schema: {})\n",
                matched_type.name, schema_name
            );

            let mut visible: Vec<&crate::types::assets::ObjectTypeAttributeDef> = attrs
                .iter()
                .filter(|a| !a.system && !a.hidden)
                .collect();
            visible.sort_by_key(|a| a.position);

            let rows: Vec<Vec<String>> = visible
                .iter()
                .map(|a| {
                    vec![
                        a.position.to_string(),
                        a.name.clone(),
                        format_attribute_type(a),
                        if a.minimum_cardinality >= 1 {
                            "Yes".into()
                        } else {
                            "No".into()
                        },
                        if a.editable { "Yes".into() } else { "No".into() },
                    ]
                })
                .collect();

            if rows.is_empty() {
                println!("No user-defined attributes.");
            } else {
                println!(
                    "{}",
                    output::render_table(
                        &["Pos", "Name", "Type", "Required", "Editable"],
                        &rows
                    )
                );
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test assets schema_json_shows_attributes schema_table_filters_system_attrs`
Expected: 2 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/cli/assets.rs tests/assets.rs
git commit -m "feat(cli): implement handle_schema for assets schema command (#87)"
```

---

### Task 8: Unit tests for format_attribute_type and resolve_schema

**Files:**
- Modify: `src/cli/assets.rs` (add to existing `#[cfg(test)] mod tests` block)

- [ ] **Step 1: Write unit tests**

Add inside the `mod tests` block in `src/cli/assets.rs`:

```rust
    use crate::types::assets::{DefaultType, ObjectTypeAttributeDef, ReferenceObjectType, ReferenceType};

    fn make_attr_def(
        default_type: Option<DefaultType>,
        reference_object_type: Option<ReferenceObjectType>,
    ) -> ObjectTypeAttributeDef {
        ObjectTypeAttributeDef {
            id: "1".into(),
            name: "test".into(),
            system: false,
            hidden: false,
            label: false,
            position: 0,
            default_type,
            reference_type: None,
            reference_object_type,
            minimum_cardinality: 0,
            maximum_cardinality: 1,
            editable: true,
            description: None,
            options: None,
        }
    }

    #[test]
    fn format_attr_type_default_type() {
        let attr = make_attr_def(
            Some(DefaultType { id: 0, name: "Text".into() }),
            None,
        );
        assert_eq!(super::format_attribute_type(&attr), "Text");
    }

    #[test]
    fn format_attr_type_reference() {
        let attr = make_attr_def(
            None,
            Some(ReferenceObjectType { id: "122".into(), name: "Service".into() }),
        );
        assert_eq!(
            super::format_attribute_type(&attr),
            "Reference \u{2192} Service"
        );
    }

    #[test]
    fn format_attr_type_unknown() {
        let attr = make_attr_def(None, None);
        assert_eq!(super::format_attribute_type(&attr), "Unknown");
    }

    #[test]
    fn format_attr_type_default_takes_precedence() {
        let attr = make_attr_def(
            Some(DefaultType { id: 0, name: "Text".into() }),
            Some(ReferenceObjectType { id: "1".into(), name: "Svc".into() }),
        );
        assert_eq!(super::format_attribute_type(&attr), "Text");
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib cli::assets::tests::format_attr_type`
Expected: 4 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/cli/assets.rs
git commit -m "test: add unit tests for format_attribute_type (#87)"
```

---

### Task 9: Documentation updates

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

- [ ] **Step 1: Update CLAUDE.md**

In `CLAUDE.md`, find the line describing `assets.rs` in the Architecture section:

```
│   ├── assets.rs        # assets search/view/tickets (--open/--status client-side filtering, search attribute enrichment)
```

Replace with:

```
│   ├── assets.rs        # assets search/view/tickets/schemas/types/schema (search enrichment, schema discovery)
```

- [ ] **Step 2: Update README.md**

In `README.md`, find the commands table section with `jr assets search`. Add three new rows after the `jr assets tickets` row:

```markdown
| `jr assets schemas`             | List object schemas in the workspace           |
| `jr assets types [--schema]`    | List object types (all or filtered by schema)  |
| `jr assets schema <TYPE>`       | Show attributes for an object type (partial match) |
```

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

Run: `cargo clippy -- -D warnings`
Expected: No warnings

Run: `cargo fmt --all -- --check`
Expected: No formatting issues

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: add assets schema discovery commands to CLAUDE.md and README (#87)"
```

---

## Self-Review

**Spec coverage check:**
- `jr assets schemas` — Task 5 ✓
- `jr assets types [--schema]` — Task 6 ✓
- `jr assets schema <TYPE> [--schema]` — Task 7 ✓
- `ObjectSchema`, `ObjectTypeEntry` types — Task 1 ✓
- Extend `ObjectTypeAttributeDef` — Task 2 ✓
- `list_object_schemas` (paginated), `list_object_types` (flat) — Task 3 ✓
- CLI subcommand definitions — Task 4 ✓
- Type display logic (defaultType, referenceObjectType, Unknown) — Task 7 + Task 8 ✓
- Error handling (no schemas, no match, ambiguous) — Task 6 + Task 7 ✓
- JSON output (pass-through + schemaName injection) — Task 5 + Task 6 + Task 7 ✓
- Table output (filtering system/hidden) — Task 7 ✓
- Smoke tests — Task 4 ✓
- Integration tests — Tasks 3, 5, 6, 7 ✓
- Unit tests — Tasks 1, 2, 8 ✓
- Docs — Task 9 ✓

**Placeholder scan:** No TBD, TODO, or "implement later" found.

**Type consistency:** `ObjectSchema`, `ObjectTypeEntry`, `ObjectTypeAttributeDef`, `DefaultType`, `ReferenceType`, `ReferenceObjectType` used consistently across all tasks. `resolve_schema` signature matches between Task 6 (definition) and Task 7 (reuse). `format_attribute_type` signature matches between Task 7 (definition) and Task 8 (unit tests).
