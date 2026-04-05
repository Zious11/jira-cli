# assets view default attributes

**Issue:** [#85](https://github.com/Zious11/jira-cli/issues/85)
**Status:** Design approved
**Date:** 2026-04-01

## Problem

`jr assets view OBJ-1` returns `"attributes": []` in JSON and shows no attribute data in table
mode unless `--attributes` is explicitly passed. The default output is missing the most valuable
information about an asset. An AI agent or user viewing an asset for the first time would
reasonably expect to see its attributes.

## Root cause

The `handle_view` function only calls `get_object_attributes()` when the `--attributes` flag is
passed. Without it, the object is fetched with `includeAttributes=false`, which returns an empty
attributes array. Even `includeAttributes=true` on the object endpoint only returns numeric
`objectTypeAttributeId` values without human-readable names — the separate
`GET /object/{id}/attributes` endpoint is required to get named attributes with `system`,
`hidden`, `label`, and `position` metadata.

## Solution

Flip the default for `assets view`: always fetch and display attributes unless `--no-attributes`
is passed.

### `src/cli/mod.rs`

**`AssetsCommand::View`**: Replace `#[arg(long)] attributes: bool` with
`#[arg(long)] no_attributes: bool`.

This matches the existing `--no-color` and `--no-input` patterns in the codebase. Clap
auto-generates `--no-attributes` from the field name.

**`AssetsCommand::Search`**: No change. `--attributes` remains opt-in. Search returns multiple
objects, and each would require a separate `/object/{id}/attributes` call — too expensive for a
default.

### `src/cli/assets.rs`

**`handle_view`**: Invert the condition — `!no_attributes` replaces `attributes` as the guard
for the `get_object_attributes()` call.

- **Table output**: No structural change. The second attributes table already renders correctly
  when attributes are fetched. It now renders by default.
- **JSON output**: No structural change. The `attributes` array injection into the object JSON
  already works. It now runs by default.

### API calls

No changes to `get_asset()` or `get_object_attributes()`. The object is still fetched with
`includeAttributes=false` (the inline attributes lack human-readable names and are not useful).
The named attributes come from the separate `/object/{id}/attributes` endpoint.

**Call pattern per `assets view` invocation:**
- Default (3 calls): resolve key → get object → get object attributes
- With `--no-attributes` (2 calls): resolve key → get object

## What doesn't change

- `assets search` — `--attributes` remains opt-in
- `assets tickets` — no attributes involved
- API client methods — no changes
- Types (`AssetObject`, `ObjectAttribute`, etc.) — no changes
- Attribute filtering logic (exclude system, hidden, label for table; exclude system, hidden
  for JSON) — no changes

## JSON output impact

**Before** (default): `{"attributes": []}` — empty, useless
**After** (default): `{"attributes": [{...}]}` — full named attributes

This is a breaking change to the default JSON output, but the previous default was unusable.
Consumers that parse `attributes` will now get the data they expected. The `--no-attributes`
flag provides an escape hatch for the minimal view.

## CLI breaking change

The `--attributes` flag on `assets view` is removed and replaced with `--no-attributes`. This
is a pre-1.0 breaking change. There is no clean alias path because the semantics are inverted
(old flag meant "include", new default already includes). Scripts using `jr assets view --attributes`
will fail with an unrecognized flag error — the fix is to remove the flag (attributes are now
included by default).

## Edge cases

| Scenario | Behavior |
|----------|----------|
| Object with no custom attributes | Attributes table omitted (existing behavior: `if !attrs.is_empty()`) |
| Object with many attributes | All returned in single API call (endpoint is not paginated) |
| `--no-attributes` passed | Skip attributes fetch, same as old default behavior |
| `--output json --no-attributes` | Returns bare object with `"attributes": []` (same as old default) |

## Testing

- **Integration test**: API-layer simulation verifying JSON filter excludes system and hidden attributes
- **CLI smoke test**: `assets view --help` shows `--no-attributes`, not `--attributes`

## Not in scope

- Changing `assets search` default (keep `--attributes` opt-in)
- Adding new flags to `assets view`
- Caching attribute definitions
- Changing the attribute filtering logic (system/hidden/label exclusion)
