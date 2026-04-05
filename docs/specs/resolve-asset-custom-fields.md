# Resolve Asset-Typed Custom Fields from Jira Field Metadata

**Issue:** #90
**Date:** 2026-04-01
**Status:** Design

## Problem

jr already discovers all CMDB-typed custom fields and displays linked assets in table output. However:

1. **JSON output is opaque** — `issue view --output json` returns raw Jira API data where CMDB custom fields contain only `objectId` and `workspaceId`, not human-readable keys or labels. Table output enriches these, but JSON doesn't.
2. **Assets are lumped together** — `issue view` shows a single "Assets" row combining all CMDB fields. When an issue has multiple asset fields (e.g., "Customer Site" and "Affected Service"), there's no way to distinguish which field each asset belongs to.
3. **No custom field visibility** — `jr project fields` shows issue types, priorities, and statuses, but no information about CMDB custom fields. Users can't discover what asset fields exist.

## Solution

Three changes, all building on the existing CMDB field discovery infrastructure:

1. Enrich CMDB custom fields in JSON output for `issue view` and `issue list`
2. Show per-field asset rows in `issue view` table output
3. Add CMDB custom fields to `project fields` output

## 1. JSON Enrichment

### Current behavior

`issue view --output json` dumps the raw Jira API response. CMDB fields appear as:

```json
"customfield_10191": [
  {"objectId": "18", "workspaceId": "e28955f0-..."}
]
```

### New behavior

After fetching the issue, detect CMDB fields in `fields.extra`, resolve them via the existing `enrich_assets` pipeline, then inject `objectKey`, `label`, and `objectType` back into the JSON before printing:

```json
"customfield_10191": [
  {
    "objectId": "18",
    "workspaceId": "e28955f0-...",
    "objectKey": "CUST-5",
    "label": "Acme Corp",
    "objectType": "Client"
  }
]
```

This is an **additive change** — existing fields (`objectId`, `workspaceId`) are preserved. New fields are injected alongside them.

### Scope

- **`issue view --output json`:** Always enrich CMDB fields (single issue, bounded API calls).
- **`issue list --output json`:** Enrich when `--assets` is active (same condition as table mode). Uses the same batch-deduplication logic that table mode already uses.

### API constraint

The Jira issue API never returns `objectKey` or `label` for CMDB fields — this is a documented limitation (JSDCLOUD-15201). Resolution requires separate Assets API calls (`GET /object/{id}`). There is no bulk endpoint; `jr` already handles this with concurrent `join_all` calls and deduplication.

## 2. Per-Field Asset Rows in `issue view`

### Current behavior

`issue view` shows a single "Assets" row:

```
Assets    CUST-5 (Acme Corp), SRV-42 (Email Server)
```

### New behavior

Each CMDB field is shown as its own row using the field's configured name:

```
Customer Site       CUST-5 (Acme Corp)
Affected Service    SRV-42 (Email Server)
```

This requires the `(id, name)` pairs from the CMDB field cache (available from #88's `get_or_fetch_cmdb_fields`). Instead of extracting all assets at once, iterate per-field: extract assets for each field ID, enrich, then display as a separate row using the field name.

If a CMDB field has no linked assets on the current issue, skip the row entirely.

If only one CMDB field exists on the instance, continue showing it by its field name (not "Assets").

### JSON output (`issue view --output json`)

The per-field structure is already present in JSON — each custom field is a separate key in `fields`. The enrichment from section 1 handles this. No additional JSON changes needed.

## 3. CMDB Custom Fields in `project fields`

### Current behavior

`jr project fields` shows issue types, priorities, and statuses. No custom field information.

### New behavior

Add a "Custom Fields (Assets)" section listing discovered CMDB fields:

```
Custom Fields (Assets) — instance-wide:
  - Client (customfield_10191)
  - Affected Service (customfield_10245)
```

The "instance-wide" qualifier is important: `GET /rest/api/3/field` returns instance-level fields, not project-scoped ones. Project-to-field mapping requires admin permissions and is out of scope.

### JSON output

```json
{
  "project": "PROJ",
  "issue_types": [...],
  "priorities": [...],
  "statuses_by_issue_type": [...],
  "asset_fields": [
    {"id": "customfield_10191", "name": "Client"},
    {"id": "customfield_10245", "name": "Affected Service"}
  ]
}
```

If no CMDB fields exist, `asset_fields` is an empty array in JSON. In table output, the section is omitted entirely.

## Implementation Notes

### Shared infrastructure

All three features depend on `get_or_fetch_cmdb_fields` (returns `Vec<(String, String)>` of id/name pairs) and the existing `extract_linked_assets` + `enrich_assets` pipeline. No new API integrations are needed.

### JSON enrichment mechanics

For `issue view`, the flow is:
1. Fetch issue via `get_issue`
2. Discover CMDB field IDs (cached)
3. For each CMDB field, extract linked assets from `fields.extra`
4. Enrich all assets via `enrich_assets` (concurrent resolution)
5. For JSON: write enriched data back into the `serde_json::Value` at each `customfield_NNNNN` key
6. For table: display per-field rows

For `issue list`, enrichment already happens for table mode in `handle_list`. The change is to also apply the resolved data to JSON output when `--assets` is active.

### Performance

- `issue view`: Single issue, typically 1-3 CMDB fields with 1-5 assets each. Negligible overhead.
- `issue list`: Already does concurrent enrichment with deduplication for table mode. JSON enrichment piggybacks on the same resolved data — no additional API calls.
- Future optimization: AQL batch resolution (`POST /object/aql` with `objectId IN (...)`) could replace per-object `GET /object/{id}` calls. This is out of scope for this feature but noted as a follow-up.

## Error Handling

- If enrichment fails for an asset (e.g., 404 from Assets API), leave the original raw data in place — don't error, don't remove the field. This matches the current table-mode behavior (graceful degradation).
- If no CMDB fields exist on the instance, all three features degrade gracefully: JSON has no enrichment needed, `issue view` shows no asset rows, `project fields` omits the section.

## Testing Strategy

### Unit tests

- JSON enrichment: given a `serde_json::Value` with raw CMDB fields, verify enriched fields are injected
- Per-field extraction: given `fields.extra` with multiple CMDB fields, verify per-field asset extraction
- `project fields` JSON: verify `asset_fields` array is present/absent based on CMDB field discovery

### Integration tests (wiremock)

- `issue view --output json` with CMDB fields returns enriched JSON
- `issue view` table output shows per-field rows
- `project fields` with CMDB fields shows the custom fields section
- `project fields` without CMDB fields omits the section

## Changes by File

| File | Change |
|------|--------|
| `src/cli/issue/list.rs` | `handle_view`: per-field rows in table, JSON enrichment; `handle_list`: JSON enrichment when `--assets` |
| `src/api/assets/linked.rs` | Add `extract_linked_assets_per_field` (returns `Vec<(field_name, Vec<LinkedAsset>)>`) and `enrich_json_assets` (injects enriched data into `serde_json::Value`) |
| `src/cli/project.rs` | `handle_fields`: add CMDB fields section |
| `tests/` | Integration tests for all three features |

## API Constraints (Validated)

- Jira issue API returns only `objectId` and `workspaceId` for CMDB fields — never `objectKey` or `label` (confirmed, JSDCLOUD-15201)
- Separate Assets API call required per object to resolve — no bulk endpoint exists
- `GET /rest/api/3/field` returns instance-wide fields, not project-scoped (confirmed)
- Project-to-field context mapping requires admin permissions (out of scope)
- Available on all paid JSM plans: Standard, Premium, Enterprise
