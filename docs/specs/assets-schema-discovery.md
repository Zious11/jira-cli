# Assets Schema Discovery

**Issue:** [#87](https://github.com/Zious11/jira-cli/issues/87)

## Problem

There is no way to discover what asset object types exist (Customer, Location, Hardware, Service) or what attributes each type has. Users and AI agents must guess AQL queries and inspect results to learn the schema. This is the assets equivalent of `project fields` — without a discovery command, the schema is invisible.

## Root Cause

The Jira Assets data model is hierarchical: **schemas → object types → attributes**. The existing `jr assets` commands (`search`, `view`, `tickets`) operate on objects but provide no way to explore the schema that defines them.

## Solution

Add three new subcommands to `jr assets`:

| Command | Purpose | API Endpoint |
|---------|---------|-------------|
| `jr assets schemas` | List all object schemas | `GET /objectschema/list?includeCounts=true` |
| `jr assets types [--schema ID\|NAME]` | List object types (all or filtered by schema) | `GET /objectschema/{id}/objecttypes/flat` per schema |
| `jr assets schema <TYPE> [--schema ID\|NAME]` | Show attributes for an object type | `GET /objecttype/{id}/attributes` (existing method) |

## API Details

### `GET /objectschema/list`

Paginated with `startAt`/`maxResults`/`isLast` — same envelope as `AssetsPage`. Pass `includeCounts=true` to get `objectCount` and `objectTypeCount`.

Response fields per schema entry:
- `id`, `name`, `objectSchemaKey` ("ITSM", "HR"), `description` (optional), `status`
- `objectCount`, `objectTypeCount` (when `includeCounts=true`)

### `GET /objectschema/{id}/objecttypes/flat`

Returns a **bare JSON array** (no pagination envelope). Pass `includeObjectCounts=true` to populate `objectCount`. Each entry contains:
- `id`, `name`, `description` (optional), `position`, `objectCount`, `objectSchemaId`
- `inherited`, `abstractObjectType`, `parentObjectTypeInherited`

### `GET /objecttype/{id}/attributes`

Already implemented as `JiraClient::get_object_type_attributes()`. Returns a bare JSON array. Each entry contains:
- `id`, `name`, `position`, `system`, `hidden`, `label`, `editable`
- `minimumCardinality`, `maximumCardinality` — `minimumCardinality >= 1` means required
- `defaultType` (optional): `{ id: 0, name: "Text" }` — present for non-reference attributes
- `referenceType` (optional): `{ id, name }` — present for reference attributes (e.g., "Depends on")
- `referenceObjectType` (optional): `{ id, name, ... }` — target object type for references (e.g., "Service")
- `options` — comma-separated options for Select type attributes

Known `defaultType` values: `0 = Text`, `6 = DateTime`, `10 = Select`. Reference attributes have `referenceType`/`referenceObjectType` instead of `defaultType`.

## New Types

### `src/types/assets/schema.rs`

```rust
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
```

### Extend `ObjectTypeAttributeDef` (in `src/types/assets/object.rs`)

Add optional fields with `#[serde(default)]` so the existing enrichment code (issue #86) is unaffected:

```rust
pub struct ObjectTypeAttributeDef {
    // existing: id, name, system, hidden, label, position
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

#[derive(Debug, Deserialize, Serialize)]
pub struct DefaultType {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReferenceType {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReferenceObjectType {
    pub id: String,
    pub name: String,
}
```

## New API Methods

### `src/api/assets/schemas.rs`

```rust
impl JiraClient {
    /// List all object schemas in the workspace.
    pub async fn list_object_schemas(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ObjectSchema>>

    /// List all object types for a given schema (flat).
    pub async fn list_object_types(
        &self,
        workspace_id: &str,
        schema_id: &str,
    ) -> Result<Vec<ObjectTypeEntry>>
}
```

`list_object_schemas` auto-paginates using the existing `AssetsPage` pattern with `startAt`/`maxResults`/`isLast`. Passes `includeCounts=true`.

`list_object_types` returns the bare array directly (no pagination).

## CLI Commands

### `jr assets schemas`

No arguments or flags.

**Table output:**

| ID | Key | Name | Description | Types | Objects |
|----|-----|------|-------------|-------|---------|
| 6  | ITSM | ITSM | — | 34 | 95 |
| 1  | HR | Human Resources | — | 14 | 1023 |

Columns: ID, Key, Name, Description (truncated or "—"), Types (`objectTypeCount`), Objects (`objectCount`).

**JSON output:** Pass through the API array directly.

### `jr assets types [--schema ID|NAME]`

Optional `--schema` flag filters to a single schema. Supports partial match on schema name or exact match on schema ID.

Without `--schema`: iterate all schemas from `list_object_schemas`, call `list_object_types` for each, inject schema name into results.

**Table output:**

| ID | Name | Schema | Description | Objects |
|----|------|--------|-------------|---------|
| 19 | Employee | Human Resources | — | 0 |
| 23 | Office | ITSM | Lorem ipsum... | 0 |

**JSON output:** Flat array of object type entries. Each entry has an injected `schemaName` field for cross-schema context.

### `jr assets schema <TYPE> [--schema ID|NAME]`

`<TYPE>` is resolved via partial match on object type name. Resolution searches across all schemas (or scoped if `--schema` is provided).

**Table output:**

```
Object Type: Office (Schema: ITSM)

┌─────┬──────────────────────┬───────────────────┬──────────┬──────────┐
│ Pos ┆ Name                 ┆ Type              ┆ Required ┆ Editable │
╞═════╪══════════════════════╪═══════════════════╪══════════╪══════════╡
│ 1   ┆ Name                 ┆ Text              ┆ Yes      ┆ Yes      │
│ 4   ┆ Location             ┆ Text              ┆ No       ┆ Yes      │
│ 5   ┆ Tier                 ┆ Select            ┆ Yes      ┆ Yes      │
│ 6   ┆ Service relationships┆ Reference → Service┆ No      ┆ Yes      │
└─────┴──────────────────────┴───────────────────┴──────────┴──────────┘
```

Type column logic:
1. Has `defaultType` → show `defaultType.name` ("Text", "DateTime", "Select")
2. Has `referenceObjectType` → show `"Reference → {referenceObjectType.name}"`
3. Neither → show "Unknown"

Required: `minimumCardinality >= 1` → "Yes", otherwise "No".

System and hidden attributes are **filtered out** in table mode. All attributes are included in JSON output.

**JSON output:** Full attribute definitions array from the API, unfiltered.

## Type Resolution

Type name resolution for `jr assets schema <TYPE>`:

1. Fetch all schemas via `list_object_schemas`
2. If `--schema` provided, resolve to a single schema (partial match on name, exact on ID)
3. Fetch object types for target schema(s) via `list_object_types`
4. Partial match `<TYPE>` against all collected type names using `partial_match.rs`
5. If ambiguous across schemas, include schema name in disambiguation: "Matches: Employee (HR), Employee (ITSM)"
6. If ambiguous within same schema, standard disambiguation

## Error Handling

| Scenario | Message |
|----------|---------|
| No schemas found | "No asset schemas found in this workspace." |
| `--schema` no match | "No schema matching \"{input}\". Available: ITSM, HR, Services" |
| `--schema` ambiguous | "Ambiguous schema \"{input}\". Matches: {list}" |
| `<TYPE>` no match | "No object type matching \"{input}\". Run \"jr assets types\" to see available types." |
| `<TYPE>` ambiguous | "Ambiguous type \"{input}\". Matches: {list}. Use --schema to narrow results." |
| Assets unavailable | Already handled by `workspace.rs` (404/403 → user-friendly error) |

## Files Changed

| File | Change |
|------|--------|
| `src/types/assets/schema.rs` | **New** — `ObjectSchema`, `ObjectTypeEntry` |
| `src/types/assets/object.rs` | Extend `ObjectTypeAttributeDef` with `default_type`, `reference_type`, `reference_object_type`, `minimum_cardinality`, `maximum_cardinality`, `editable`, `description`, `options` |
| `src/types/assets/mod.rs` | Add `pub mod schema; pub use schema::*;` |
| `src/api/assets/schemas.rs` | **New** — `list_object_schemas`, `list_object_types` |
| `src/api/assets/mod.rs` | Add `pub mod schemas;` |
| `src/cli/mod.rs` | Add `Schemas`, `Types`, `Schema` variants to `AssetsCommand` |
| `src/cli/assets.rs` | Add `handle_schemas`, `handle_types`, `handle_schema` handlers |
| `CLAUDE.md` | Update `assets.rs` description |
| `README.md` | Add new commands to table |
| `tests/assets.rs` | Integration tests for new API methods and CLI commands |
| `tests/cli_smoke.rs` | Smoke tests for new subcommands |

## Testing

- **Unit tests:** Serde deserialization for `ObjectSchema`, `ObjectTypeEntry`, extended `ObjectTypeAttributeDef` (with and without reference fields)
- **Unit tests:** Type display logic (defaultType, referenceObjectType, neither)
- **Integration tests (wiremock):** `list_object_schemas` with pagination, `list_object_types` flat response, `get_object_type_attributes` with extended fields
- **CLI integration tests:** End-to-end `jr assets schemas`, `jr assets types`, `jr assets schema <TYPE>` with both table and JSON output
- **CLI smoke tests:** Verify subcommands parse correctly (`--help` exit 0)

## Edge Cases

- **Single schema workspace:** All commands work. `--schema` is optional but accepted.
- **Empty schema:** `list_object_types` returns `[]`. `jr assets types` shows "No object types found."
- **Abstract object types:** Included in `types` output but not directly instantiable. No special handling needed — users can still inspect their attributes.
- **Type name collision across schemas:** Disambiguation message includes schema name parenthetically.
- **Extending `ObjectTypeAttributeDef`:** New fields use `Option<T>` or `#[serde(default)]`. Existing callers (search enrichment from #86) only read `id`, `name`, `system`, `hidden`, `label`, `position` — all still present. The `CachedObjectTypeAttr` cache struct is unchanged since `schema` fetches attributes directly, not through the cache.
