# Assets Search Attribute Names

**Issue:** [#86](https://github.com/Zious11/jira-cli/issues/86)
**Status:** Design approved
**Date:** 2026-04-01

## Problem

`jr assets search --attributes --output json` returns attribute data with only a numeric
`objectTypeAttributeId` — no human-readable name. In contrast, `jr assets view --attributes
--output json` includes the full `objectTypeAttribute` object with `name`, `position`, and
filtering metadata (`system`, `hidden`, `label`).

An AI agent running a search gets attributes it cannot interpret without a follow-up
`assets view` call per result.

Additionally, `assets search --attributes` in table mode only adds Created/Updated columns
but does not display actual attribute values.

## Root Cause

The AQL search endpoint (`POST /object/aql?includeAttributes=true`) returns inline
`AssetAttribute` structs that contain `objectTypeAttributeId` (a numeric string) but no
`objectTypeAttribute` object with the name. The per-object endpoint
(`GET /object/{id}/attributes`) returns the full `ObjectAttribute` struct with names — this is
what `assets view` uses.

There is no search endpoint that returns attribute names. Enrichment requires additional API
calls.

## Solution

Resolve attribute definitions by fetching them per object type via
`GET /objecttype/{id}/attributes`, cache the results, and inject `objectTypeAttribute` into
each search result's attributes. For table mode, display attribute values as inline
`Name: Value` pairs in an "Attributes" column.

## API Details

**Fetch attribute definitions** — `GET /objecttype/{id}/attributes`
- Response: array of `ObjectTypeAttributeDef` with `id`, `name`, `system`, `hidden`, `label`,
  `position` (plus other fields serde ignores)
- One call per unique object type in search results (typically 1-3)
- Cacheable — schema-level data that rarely changes

**Existing search endpoint** — `POST /object/aql?includeAttributes=true`
- Response includes `attributes` array per object, each with `objectTypeAttributeId` and
  `objectAttributeValues` but no `objectTypeAttribute`
- No changes to how we call this endpoint

## Enrichment Flow

### `enrich_search_attributes`

New function in `src/api/assets/objects.rs`:

1. Collect unique `objectType.id` values from search results
2. For each unique type ID:
   - Check `~/.cache/jr/object_type_attrs.json` for a cached entry
   - Cache miss → call `GET /objecttype/{typeId}/attributes` → write to cache
3. Build `HashMap<String, ObjectTypeAttributeDef>` mapping `objectTypeAttributeId` → definition
4. For each object's attributes, inject `objectTypeAttribute` from the map (skip attributes
   with no match — defensive against schema drift)

### Cost

- **Without enrichment:** 0 extra API calls, but opaque IDs
- **With enrichment:** K calls where K = unique object types (typically 1, rarely >3)
- **With cache (after first call):** 0 extra API calls

## Cache

### Structure

`~/.cache/jr/object_type_attrs.json`:

```rust
pub struct ObjectTypeAttrCache {
    pub fetched_at: DateTime<Utc>,
    pub types: HashMap<String, Vec<CachedObjectTypeAttr>>,
}

pub struct CachedObjectTypeAttr {
    pub id: String,
    pub name: String,
    pub system: bool,
    pub hidden: bool,
    pub label: bool,
    pub position: i32,
}
```

### Behavior

- 7-day TTL (consistent with all other caches)
- Keyed by object type ID — multiple types coexist in one file
- Deserialization failure → treat as miss (consistent with `cmdb_fields.json` pattern)
- New types are merged into existing cache, not replaced
- The `CachedObjectTypeAttr` struct stores only the fields needed for enrichment and filtering,
  not the full API response

## New API Method

### `src/api/assets/objects.rs`

```rust
pub async fn get_object_type_attributes(
    &self,
    workspace_id: &str,
    object_type_id: &str,
) -> Result<Vec<ObjectTypeAttributeDef>>
```

Calls `GET /objecttype/{object_type_id}/attributes`. The existing `ObjectTypeAttributeDef`
struct can deserialize this response — serde ignores extra fields (`editable`, `sortable`,
`objectType`, etc.) that are present in the API response but absent from the struct.

## Output Changes

### JSON (`--output json`)

When `--attributes` is passed, after enrichment each attribute gains an `objectTypeAttribute`
field. System and hidden attributes are filtered out (matching `assets view` behavior).
Attributes are sorted by position.

**Before (current):**
```json
{
  "id": "88",
  "label": "Acme Corp",
  "objectKey": "OBJ-88",
  "objectType": {"id": "23", "name": "Client"},
  "attributes": [
    {
      "id": "81",
      "objectTypeAttributeId": "81",
      "objectAttributeValues": [{"value": "0", "displayValue": "0"}]
    }
  ]
}
```

**After (enriched):**
```json
{
  "id": "88",
  "label": "Acme Corp",
  "objectKey": "OBJ-88",
  "objectType": {"id": "23", "name": "Client"},
  "attributes": [
    {
      "id": "81",
      "objectTypeAttributeId": "81",
      "objectTypeAttribute": {"name": "Managed_Devices", "position": 5},
      "objectAttributeValues": [{"value": "0", "displayValue": "0"}]
    }
  ]
}
```

The enrichment is additive — `objectTypeAttribute` is injected into the existing attribute
JSON via `serde_json::Value` manipulation (insert key into the attribute object map). Only
`name` and `position` are included in the injected object — these are the fields consumers
need. The root-level object schema is preserved (same approach `assets view` uses for
injecting richer attributes).

### Table (`--output table`)

When `--attributes` is passed, replace the current Created/Updated columns with a single
"Attributes" column containing inline `Name: Value` pairs, pipe-delimited. Filter
system/hidden/label attributes, sort by position.

**Before (current):**
```
Key      Type     Name        Created                  Updated
OBJ-88   Client   Acme Corp   2025-12-17T14:58:00Z     2026-01-29T19:52:00Z
```

**After (enriched):**
```
Key      Type     Name        Attributes
OBJ-88   Client   Acme Corp   Location: New York, NY | Managed_Devices: 0
```

Multi-value attributes use the first `displayValue` (or `value` as fallback). Attributes
with no values are omitted from the inline display.

### Without `--attributes`

No change. The default table (Key, Type, Name) and JSON output remain identical.

## Handler Changes

### `src/cli/assets.rs` — `handle_search`

When `attributes` is `true`:
1. Fetch search results with `include_attributes=true` (existing)
2. Call `enrich_search_attributes(client, workspace_id, &mut objects)` (new)
3. For JSON: serialize enriched objects with `objectTypeAttribute` injected, filter
   system/hidden, sort by position
4. For table: build "Attributes" column from enriched data

The enrichment function is called once after search, before output formatting.

## Files Changed

| File | Change |
|------|--------|
| `src/api/assets/objects.rs` | Add `get_object_type_attributes`, `enrich_search_attributes` |
| `src/types/assets/object.rs` | No changes — reuse `ObjectTypeAttributeDef` |
| `src/cache.rs` | Add `ObjectTypeAttrCache`, `CachedObjectTypeAttr`, read/write functions |
| `src/cli/assets.rs` | Update `handle_search` for enriched JSON and table output |

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Object type attributes fetch fails (401/404/500) | Log warning, skip enrichment for that type — attributes remain un-enriched (graceful degradation) |
| Attribute ID not found in type definitions | Skip that attribute in enriched output — defensive against schema drift |
| Cache file corrupt / old format | Treat as miss, re-fetch (consistent with `cmdb_fields.json`) |
| `--attributes` without `--output json` | Enrichment still runs for table mode |

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| Mixed object types in results | Each type's definitions fetched/cached independently |
| No attributes on an object | "Attributes" column shows empty string |
| All attributes are system/hidden | "Attributes" column shows empty string |
| Multi-value attribute | Show first `displayValue` or `value` |
| Search with `--attributes` returns 0 results | No enrichment needed, empty table/JSON |
| `assets view` (existing) | Unchanged — still uses `GET /object/{id}/attributes` directly |

## Testing

- **Unit tests**: Cache read/write, enrichment logic (inject names, filter system/hidden,
  handle missing IDs, sort by position), table column formatting
- **Integration tests (wiremock)**: Mock `POST /object/aql` + `GET /objecttype/{id}/attributes`,
  verify enriched JSON output contains `objectTypeAttribute.name`, verify table output contains
  inline attribute values
- **CLI smoke tests**: `assets search --help` still shows `--attributes` flag

## What Doesn't Change

- `assets view` — still uses `GET /object/{id}/attributes` directly
- `assets tickets` — no attributes involved
- `AssetObject`, `AssetAttribute`, `ObjectAttribute`, `ObjectTypeAttributeDef` types — no
  changes to existing structs
- Search without `--attributes` — completely unchanged
- No new CLI flags or subcommands
- No new dependencies
