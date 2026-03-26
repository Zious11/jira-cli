# Asset Attribute Names Instead of Numeric IDs — Design Spec

**Issue:** #58

**Goal:** Replace raw numeric `Attribute ID` column in `jr assets view <KEY> --attributes` with human-readable attribute names by using the `/object/{id}/attributes` endpoint which returns attribute definitions inline.

## Problem

`jr assets view <KEY> --attributes` displays an `Attribute ID` column with raw numeric IDs (e.g., `61`, `80`) because the current implementation uses `GET /object/{id}?includeAttributes=true`, which returns attributes with only `objectTypeAttributeId` (a numeric string) and no attribute name.

**Current output:**
```
┌──────────────┬──────────────┐
│ Attribute ID │ Value        │
╞══════════════╪══════════════╡
│ 61           │ Acme Corp    │
│ 80           │ New York, NY │
│ 81           │ 0            │
│ 82           │ 4            │
└──────────────┴──────────────┘
```

Without cross-referencing the object type schema, users have no idea what these IDs represent.

## Expected

```
┌───────────┬──────────────┐
│ Attribute │ Value        │
╞═══════════╪══════════════╡
│ Location  │ New York, NY │
│ Seats     │ 0            │
│ Endpoints │ 4            │
└───────────┴──────────────┘
```

System attributes (Key, Created, Updated), the label attribute (Name), and hidden attributes are filtered out because they already appear in the main view table above or are not meant to be displayed.

## API Approach

The Jira Assets API provides two ways to get attribute data for an object:

1. **`GET /object/{id}?includeAttributes=true`** — Returns attributes with only `objectTypeAttributeId` (numeric ID). No attribute name. This is what the code currently uses.

2. **`GET /object/{id}/attributes`** — Returns attributes with a full `objectTypeAttribute` nested object that includes `name`, `system`, `hidden`, and `position` fields. This gives us everything we need in a single API call.

We use option 2. No second API call or caching is needed.

### `/object/{id}/attributes` Response Structure

Each entry in the response array contains:

```json
{
  "objectTypeAttribute": {
    "id": "134",
    "name": "Location",
    "system": false,
    "hidden": false,
    "position": 4
  },
  "objectTypeAttributeId": "134",
  "objectAttributeValues": [
    {
      "value": "New York, NY",
      "displayValue": "New York, NY"
    }
  ]
}
```

Key fields used:
- `objectTypeAttribute.name` — human-readable attribute name (replaces numeric ID)
- `objectTypeAttribute.system` — `true` for Key, Created, Updated (filter these out)
- `objectTypeAttribute.label` — `true` for the Name attribute (the object's display name, already shown in main view table; filter out)
- `objectTypeAttribute.hidden` — `true` for attributes hidden in the Jira UI (filter these out)
- `objectTypeAttribute.position` — display order
- `objectAttributeValues[].displayValue` — preferred display value (falls back to `value`)

## Fix

### 1. New Serde Types (`src/types/assets/object.rs`)

Add types for the richer `/object/{id}/attributes` response. These are separate from the existing `AssetAttribute` type which remains unchanged for use by `get_asset()`, search, and linked asset enrichment.

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

The existing `ObjectAttributeValue` type is reused — it already has `value` and `display_value`.

### 2. New API Method (`src/api/assets/objects.rs`)

```rust
/// Get all attributes for a single object, with full attribute definitions.
pub async fn get_object_attributes(
    &self,
    workspace_id: &str,
    object_id: &str,
) -> Result<Vec<ObjectAttribute>> {
    let path = format!("object/{}/attributes", urlencoding::encode(object_id));
    self.get_assets(workspace_id, &path).await
}
```

### 3. Update CLI (`src/cli/assets.rs`)

In `handle_view`, when `attributes` is true:

1. Call `get_object_attributes()` instead of relying on `get_asset()`'s `includeAttributes` parameter
2. Filter out system and hidden attributes
3. Sort by position
4. Render using `attr.object_type_attribute.name` as the attribute column
5. Use `displayValue` when available, fall back to `value`

**Also change line 95** from `client.get_asset(workspace_id, &object_id, attributes)` to `client.get_asset(workspace_id, &object_id, false)` — the object itself no longer needs to carry attribute data.

**Before (lines 118-139):**
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

**After:**
```rust
if attributes {
    let mut attrs = client
        .get_object_attributes(workspace_id, &object_id)
        .await?;
    // Filter out system (Key, Created, Updated), label (Name), and hidden attributes
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

### 4. JSON Output (`src/cli/assets.rs`)

When `--output json` is used with `--attributes`, emit a combined JSON object containing both the asset and its named attributes:

```rust
OutputFormat::Json => {
    if attributes {
        let mut attrs = client
            .get_object_attributes(workspace_id, &object_id)
            .await?;
        attrs.retain(|a| !a.object_type_attribute.system && !a.object_type_attribute.hidden);
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

This wraps the object and attributes in a single JSON envelope, giving consumers attribute names directly. Without `--attributes`, the JSON output is unchanged (just the object).

## Files Changed

| File | Change |
|------|--------|
| `src/types/assets/object.rs` | Add `ObjectAttribute` and `ObjectTypeAttributeDef` types |
| `src/api/assets/objects.rs` | Add `get_object_attributes()` API method |
| `src/cli/assets.rs` | Update `handle_view` to use new endpoint, filter, sort, and render names |

## What Doesn't Change

- `get_asset()` API method and `AssetObject`/`AssetAttribute` types — still used by search and linked asset enrichment
- `search --attributes` — unchanged (doesn't render individual attributes in CLI output)
- `handle_search()` — unchanged
- `handle_tickets()` — unchanged

## Testing

### Unit Tests (`src/types/assets/object.rs`)

- Deserialize `ObjectAttribute` with full nested `objectTypeAttribute` including name
- Deserialize with missing optional fields (`system`, `hidden` default to `false`)
- Verify `ObjectAttributeValue` reuse (existing type)
- Attribute with empty `objectAttributeValues` array produces zero rows (omitted from table, consistent with existing behavior)

### Integration Test (`tests/`)

- Wiremock `GET /object/{id}/attributes` returning a mix of system, hidden, and user-defined attributes
- Verify only non-system, non-hidden attributes appear in table output
- Verify attributes are sorted by position
- Verify `displayValue` is preferred over `value`

### Live Verification

`jr assets view <KEY> --attributes` shows attribute names instead of numeric IDs.

## Backward Compatibility

Table output: No breaking changes. The fix produces human-readable output where previously it produced opaque numeric IDs. The `--attributes` flag was effectively unusable without this fix.

JSON output: When using `--output json --attributes`, the `attributes` array in the root object is replaced with richer attribute entries that include `objectTypeAttribute.name` and other metadata. The root-level object schema is preserved (no envelope wrapper). Existing fields (`objectTypeAttributeId`, `objectAttributeValues`) remain present. The addition of `objectTypeAttribute` is additive. System and hidden attributes are filtered out.
