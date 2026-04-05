# Issue List `--asset` Filter

**Issue:** #88
**Date:** 2026-04-01
**Status:** Design

## Problem

There is no way to filter `issue list` by linked asset. The only path from asset to issues is `jr assets tickets OBJ-1`, which returns a simpler response format (no labels, points, links) and cannot be composed with other `issue list` filters like `--status`, `--assignee`, `--team`, or `--open`.

## Solution

Add `--asset <KEY>` to `issue list` that generates an `aqlFunction()` JQL clause, composable with all existing filters.

```
jr issue list --project PROJ --asset CUST-5
jr issue list --asset CUST-5 --status "In Progress" --assignee me
jr issue list --asset CUST-5 --open --points
```

## CLI Surface

Add `--asset <KEY>` to `IssueCommand::List`:

- Accepts an asset object key (e.g., `CUST-5`, `SRV-42`) — the `SCHEMA-NUMBER` format used by Jira Assets
- Composes with all existing filters (`--status`, `--assignee`, `--reporter`, `--team`, `--recent`, `--open`, `--jql`)
- Automatically enables the `--assets` display column when `--asset` is used
- Fully non-interactive: no prompts, no `--no-input` concern

## JQL Construction

### aqlFunction() — the JQL Function

The Jira Assets JQL function used here is **`aqlFunction()`**. It accepts an AQL (Assets Query Language) string and returns matching objects. It must be used with the **human-readable custom field name**, not the `cf[ID]` shorthand or `customfield_NNNNN` format.

Supported operators: `IS`, `IS NOT`, `IN`, `NOT IN`.

### Single CMDB Field (Common Case)

```
"Client" IN aqlFunction("Key = \"CUST-5\"")
```

### Multiple CMDB Fields

When the instance has multiple Assets custom fields (e.g., "Client" and "Server"), OR them:

```
("Client" IN aqlFunction("Key = \"CUST-5\"") OR "Server" IN aqlFunction("Key = \"CUST-5\""))
```

### Escaping

- **Field name:** Wrapped in double quotes. Field names are admin-defined strings; quote defensively.
- **Asset key inside AQL:** Escaped with `jql::escape_value`, then nested inside the JQL string with backslash-escaped quotes. Asset keys follow `SCHEMA-NUMBER` format (alphanumeric + hyphen), so special characters are unlikely but handled defensively.

```rust
// Per CMDB field, build:
//   "Client" IN aqlFunction("Key = \"CUST-5\"")
format!(
    "\"{}\" IN aqlFunction(\"Key = \\\"{}\\\"\")",
    escape_value(&field_name),
    escape_value(&asset_key),
)
```

Multiple fields are joined with ` OR ` and wrapped in parentheses when there are 2+.

## CMDB Field Discovery Changes

### Current State

`filter_cmdb_fields` returns `Vec<String>` (IDs only). The cache stores `CmdbFieldsCache { field_ids: Vec<String> }`.

### Required Change

`filter_cmdb_fields` returns `Vec<(String, String)>` — `(field_id, field_name)` pairs. The cache stores `CmdbFieldsCache { fields: Vec<(String, String)> }`.

Stale caches auto-expire after 7 days. On deserialization failure of an old-format cache, it is treated as a cache miss (existing behavior: `serde_json::from_str` returns `Err`, propagated as `None`).

Existing callers that only need IDs extract with `.iter().map(|(id, _)| id.clone()).collect()`.

## Error Handling

### No CMDB fields found

If `get_or_fetch_cmdb_field_ids` returns empty and `--asset` was specified:

```
Error: --asset requires Assets custom fields on this Jira instance.
Assets requires Jira Service Management Premium or Enterprise.
```

### Invalid asset key format

Validate the key matches `SCHEMA-NUMBER` pattern before building JQL:

```
Error: Invalid asset key "foo". Expected format: SCHEMA-NUMBER (e.g., CUST-5, SRV-42).
```

### aqlFunction returns no matches

Not an error. JQL returns zero issues, same as any other filter with no matches.

### `--asset` without project scope

Allowed at the CLI level — the asset clause counts as a valid filter for the "no project or filters" guard. If Jira's API rejects a project-less `aqlFunction()` query, the API error propagates naturally through existing error handling. In practice, most users have a default project configured via `.jr.toml` or `--project`.

## Automatic `--assets` Display Column

When `--asset` is used, automatically enable the `--assets` display column without requiring the user to also pass `--assets`. Implementation: if `--asset` is set, treat `show_assets` as `true` regardless of whether the `--assets` display flag was passed.

## Testing Strategy

### Unit Tests

- `build_asset_clause` with single CMDB field, multiple CMDB fields
- `build_filter_clauses` with asset clause composed alongside other filters
- Asset key validation (valid keys, malformed keys)
- `filter_cmdb_fields` returning `(id, name)` tuples

### Integration Tests (wiremock)

- `issue list --asset CUST-5` produces correct JQL in the search request
- `--asset` combined with `--status`, `--assignee` composes correctly
- `--asset` with no CMDB fields returns the appropriate error
- `--asset` auto-enables assets display column

## Changes by File

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add `--asset` field to `IssueCommand::List` |
| `src/api/jira/fields.rs` | `filter_cmdb_fields` returns `Vec<(String, String)>` |
| `src/cache.rs` | `CmdbFieldsCache` stores `Vec<(String, String)>` |
| `src/api/assets/linked.rs` | `get_or_fetch_cmdb_field_ids` returns `Vec<(String, String)>`, add helper to extract IDs only |
| `src/cli/issue/list.rs` | Build asset JQL clause, auto-enable assets column, update filter guard |
| `src/cli/issue/assets.rs` | Adapt to new `(id, name)` tuple return |
| `tests/` | Integration tests for `--asset` flag |

## API Constraints (Validated)

- `aqlFunction()` requires the human-readable field **name**, not `cf[ID]` or `customfield_NNNNN` (Atlassian support docs, community confirmed)
- `aqlFunction()` composes with other JQL clauses via `AND` (confirmed)
- AQL attribute for object key is **`Key`** (a reserved AQL keyword) — e.g., `Key = "CUST-5"`. Note: `objectKey` is the JSON field name in REST API responses, but `Key` is the AQL query attribute (confirmed via Atlassian AQL docs)
- Asset object keys follow `SCHEMA-NUMBER` format (confirmed)
- Empty AQL results produce empty JQL results, not errors (confirmed)
- Available on all paid JSM plans: Standard, Premium, and Enterprise (confirmed via Atlassian docs)
- Deprecated functions like `attributeValue()` should be avoided; `aqlFunction()` is the current standard
