# Issue Linked Assets — Design Spec

**Goal:** Expose CMDB/Assets objects linked to Jira issues, bridging the gap between the issue and asset domains.

**Problem:** `jr issue view` does not show connected CMDB/Assets objects. The only way to discover which asset is linked to a ticket is to reverse-search by iterating through assets using `jr assets tickets <ASSET_KEY>` — impractical at scale. There is no issue→asset lookup, only asset→issue.

**Addresses:** [GitHub Issue #46](https://github.com/Zious11/jira-cli/issues/46)

---

## Architecture

Assets are linked to Jira issues via CMDB custom fields (`com.atlassian.jira.plugins.cmdb:cmdb-object-cftype`). These fields are returned inline in the issue response when explicitly requested. The feature has three layers:

1. **Field Discovery** — auto-discover which custom fields are CMDB type
2. **Adaptive Parsing** — extract asset references from varying response shapes
3. **Commands** — three new/modified commands to surface linked assets

No new API endpoints are introduced. The feature reuses the existing `GET /rest/api/3/field` endpoint for discovery and the existing `get_asset()` method for enrichment.

---

## 1. Field Discovery & Caching

### Discovery

Use `GET /rest/api/3/field` (already called by `list_fields()` in `api/jira/fields.rs`) to find CMDB fields:

```
Filter: schema.custom == "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"
Result: Vec<String> of field IDs, e.g., ["customfield_10191", "customfield_10245"]
```

This endpoint requires no special permissions — basic Jira access (`read:jira-work` scope) is sufficient. It works on all Jira Cloud plans. Assets itself requires JSM Premium/Enterprise, but the field endpoint is platform-level.

### Caching

Cache discovered field IDs in `~/.cache/jr/cmdb_fields.json` with 7-day TTL. Same pattern as teams, project meta, and workspace caches.

```json
{
  "field_ids": ["customfield_10191"],
  "cached_at": "2026-03-24T12:00:00Z"
}
```

### Lazy Initialization

Discovery runs on first use of a CMDB-dependent command (`jr issue view`, `jr issue list --assets`, `jr issue assets`), not during `jr init`. Users who don't use Assets never pay the cost. The unpaginated `GET /rest/api/3/field` is used (the paginated `GET /rest/api/3/field/search` requires admin permissions).

---

## 2. Adaptive CMDB Field Parsing

### Problem

The CMDB custom field response shape varies across Jira instances:

| Instance type | Response shape |
|---------------|----------------|
| Modern Assets | `[{"label": "Acme Corp", "objectKey": "OBJ-1", ...}]` |
| Legacy Insight | `[{"workspaceId": "...", "objectId": "123", "id": "wid:123"}]` |
| Mixed | May include some or all fields |

### Parsed Representation

```rust
pub struct LinkedAsset {
    pub object_key: Option<String>,   // e.g., "OBJ-1"
    pub label: Option<String>,        // e.g., "Acme Corp"
    pub object_id: Option<String>,    // numeric ID
    pub workspace_id: Option<String>, // workspace ID
}
```

### Parsing Strategy

Extract CMDB values from `IssueFields.extra` (the `#[serde(flatten)] HashMap<String, Value>`):

1. For each discovered CMDB field ID, look up the key in `extra`
2. If value is `null` or absent → no linked assets for this field, skip
3. If value is an array → parse each element:
   - Extract `label` and `objectKey` if present (best case)
   - Fall back to `objectId` and `workspaceId` if that's all we get
   - If value is an unexpected shape (string, number), store as `label`
4. Collect all `LinkedAsset` values across all CMDB fields on the issue

### Enrichment Fallback

When `object_id` is present but `label`/`objectKey` are missing, enrichment requires two preconditions:

1. **`assets_base_url` is configured** — cloud_id was set during `jr init`
2. **Workspace ID is available** — obtained via `get_or_fetch_workspace_id()` in `api/assets/workspace.rs` (already cached with 7-day TTL, handles 403/404 gracefully)

If both are satisfied:
- Call `get_asset(workspace_id, object_id, false)` — reuses existing method in `api/assets/objects.rs`
- Extract `label` and `object_key` from the returned `AssetObject`
- Enrichment calls are parallelized via `futures::future::join_all` (both for `view` and `list`)

If either precondition fails:
- `assets_base_url` not configured → display raw ID with hint: `#12345 (run "jr init" to resolve asset names)`
- Workspace discovery fails (403/404) → same degradation, display raw ID
- Individual object fetch fails (deleted, permissions) → skip that asset, display raw ID — don't fail the command

### Display Format

| Scenario | Display |
|----------|---------|
| Key + label available | `OBJ-1 (Acme Corp)` |
| Key only, no label | `OBJ-1` |
| ID only, after enrichment | `OBJ-1 (Acme Corp)` |
| ID only, no Assets API | `#12345` |
| Multiple assets | `OBJ-1 (Acme Corp), OBJ-2 (Other Inc)` |

---

## 3. Command Changes

### 3a. `jr issue view` — Assets Row

Add an "Assets" row to the key-value table, after "Links" and before "Points":

```
Key:       PROJ-123
Type:      Event Alert
Status:    In Progress
Assignee:  Jane Doe
Links:     blocks PROJ-456 (Config review)
Assets:    OBJ-1 (Acme Corp), OBJ-2 (Other Inc)
Points:    5
```

**Behavior:**
- The Assets row is shown **automatically** whenever CMDB fields are discovered (no `--assets` flag needed for `view`). This mirrors the Points pattern: always shown in `view`, opt-in via flag in `list`.
- CMDB field IDs are added to `extra_fields` when fetching the issue
- Values extracted from `extra`, parsed adaptively, enriched if needed (parallelized via `join_all`)
- If no CMDB fields discovered on this instance → row omitted silently
- If CMDB fields exist but issue has no linked assets → Assets row shows "(none)"
- JSON output: no change needed — raw issue JSON already includes CMDB fields in `extra`

### 3b. `jr issue list --assets` — Assets Column

Add `--assets` flag (same pattern as existing `--points`):

```
Key       Type         Status       Assets                 Summary
PROJ-123  Event Alert  In Progress  OBJ-1 (Acme Corp)     Config change detected
PROJ-124  Task         To Do        OBJ-2 (Other Inc)     Review alert tuning
PROJ-125  Bug          Done         -                      Fix login page
```

**Behavior:**
- When `--assets` is passed, add discovered CMDB field IDs to `extra_fields` in the search request
- Search API returns CMDB field values identically to single-issue GET (confirmed via Perplexity)
- Add "Assets" column to table after "Assignee"
- Display first asset + count if multiple: `OBJ-1 (+2 more)`
- If `--assets` passed but no CMDB fields discovered → warn to stderr (same pattern as `--points`)

**Performance:**
- Enrichment batched per page: collect unique `objectId` values, resolve in parallel via `futures::future::join_all`
- Capped at page size (max 50 issues × 1-3 fields = ~150 enrichment calls worst case)
- 429 retry already handled by `JiraClient.send()`

### 3c. `jr issue assets KEY` — New Subcommand

Show assets linked to a specific issue:

```
$ jr issue assets PROJ-123
Key     Type    Name
OBJ-1   Client  Acme Corp
OBJ-2   Server  Web-Prod-01
```

**Rationale:** A separate subcommand rather than overloading `jr assets tickets` because:
- Asset keys and issue keys have the same `PREFIX-123` format — ambiguous if shared
- CLI best practice (gh, kubectl) is separate subcommands per resource type
- Each direction has its own command: `jr issue assets PROJ-123` (issue→assets) and `jr assets tickets OBJ-1` (asset→issues)

**Behavior:**
- Fetch the issue with CMDB field IDs in `extra_fields`
- Extract and parse linked assets, enrich if needed (parallelized)
- Table output: Key, Type (objectType.name), Name (label)
- JSON output: array of asset objects with all available fields:
  ```json
  [
    {"key": "OBJ-1", "type": "Client", "name": "Acme Corp", "id": "88"},
    {"key": "OBJ-2", "type": "Server", "name": "Web-Prod-01", "id": "92"}
  ]
  ```
  When enrichment provides additional data, it is included. When only IDs are available, `key`/`type`/`name` may be `null`.
- If no assets linked → `"No assets linked to PROJ-123."`

---

## 4. Error Handling & Edge Cases

### No CMDB Fields on Instance

- `find_cmdb_field_ids()` returns empty list
- `jr issue view`: Assets row omitted silently
- `jr issue list --assets`: warn to stderr: `"warning: --assets ignored. No Assets custom fields found on this Jira instance."`
- `jr issue assets KEY`: error: `"No Assets custom fields found on this Jira instance. Assets requires Jira Service Management Premium or Enterprise."`

### CMDB Fields Exist but Issue Has No Linked Assets

- Custom field value is `null`, absent, or empty array `[]`
- `jr issue view`: Assets row shows "(none)"
- `jr issue list --assets`: Assets column shows "-"
- `jr issue assets KEY`: `"No assets linked to PROJ-123."`

### Enrichment Failures

- **Assets API not configured** (`assets_base_url` is `None`): display raw IDs with hint
- **Single object fetch fails** (deleted, permissions): skip that asset's enrichment, display raw ID — don't fail the entire command
- **Rate limiting**: handled by existing 429 retry in `JiraClient.send()`

### Cache Staleness

- Admin adds/removes CMDB fields — cache has wrong IDs
- 7-day TTL handles this naturally
- If a cached field ID returns `null` on an issue, treated as "no linked assets" — harmless

---

## 5. File Structure

### Modified Files

| File | Change |
|------|--------|
| `src/api/jira/fields.rs` | Add `find_cmdb_field_ids()` — filter by `schema.custom`. Lives here because it queries `GET /rest/api/3/field` (Jira platform endpoint), alongside existing `find_story_points_field_id()` and `find_team_field_id()`. |
| `src/cache.rs` | Add `CmdbFieldsCache` with read/write + 7-day TTL |
| `src/cli/mod.rs` | Add `Assets` variant to `IssueCommand`, add `--assets` flag to `List` |
| `src/cli/issue/mod.rs` | Wire up `IssueCommand::Assets` dispatch |
| `src/cli/issue/list.rs` | Modify `handle_view` for Assets row, `handle_list` for `--assets` column |

### New Files

| File | Purpose |
|------|---------|
| `src/api/assets/linked.rs` | `get_or_fetch_cmdb_field_ids()` (cache orchestration wrapping `find_cmdb_field_ids()`), `extract_linked_assets()` (adaptive parsing), `enrich_assets()` (parallel enrichment via `get_asset()`). Lives in `api/assets/` because it orchestrates Assets-domain caching and enrichment, even though discovery calls the Jira field endpoint. |
| `src/cli/issue/assets.rs` | `handle_issue_assets()` — the `jr issue assets KEY` command handler. Lives in `cli/issue/` because it operates on issues, consistent with the issue subcommand pattern. |
| `src/types/assets/linked.rs` | `LinkedAsset` struct — represents an asset reference extracted from an issue. Lives in `types/assets/` because it represents asset data, even though it is parsed from issue responses. |
| `tests/cmdb_fields.rs` | Integration tests for field discovery + linked asset extraction |

### Not Changed

- `src/api/client.rs` — no new HTTP methods needed
- `src/api/assets/objects.rs` — `get_asset()` already exists for enrichment
- `src/cli/assets.rs` — `jr assets tickets` stays as-is
- `src/config.rs` — no new config entries
- `main.rs` — routing already handles `IssueCommand` variants

---

## 6. Testing Strategy

### Unit Tests

- `find_cmdb_field_ids()` — filters correctly by schema.custom, ignores non-CMDB fields
- `extract_linked_assets()` — parses `{label, objectKey}` shape, `{workspaceId, objectId}` shape, null, empty array, unexpected shapes
- `LinkedAsset` display formatting — all display scenarios
- Cache read/write/expiry for `CmdbFieldsCache`

### Integration Tests (wiremock)

- Field discovery returns CMDB field IDs
- Issue fetch with CMDB fields in `extra_fields` returns asset values
- Enrichment fallback — mock Assets API to return `AssetObject`
- Search with `--assets` includes CMDB fields in request
- No CMDB fields on instance — graceful degradation

### Manual Testing

- `jr issue view` on an issue with linked assets
- `jr issue view` on an issue without linked assets
- `jr issue list --assets` with mixed assets/no-assets issues
- `jr issue assets KEY` on issue with and without assets

---

## Validation Sources

| Decision | Validated by |
|----------|-------------|
| CMDB field type: `com.atlassian.jira.plugins.cmdb:cmdb-object-cftype` | Perplexity (Atlassian community + developer docs) |
| `GET /rest/api/3/field` returns `schema.custom` | Perplexity + Context7 (official API docs) |
| No special permissions for field endpoint | Perplexity + Context7 |
| Search API returns CMDB fields same as single-issue GET | Perplexity |
| CMDB field response varies across instances | Perplexity (legacy Insight vs modern Assets) |
| No reverse lookup (issue→assets via AQL) | Perplexity |
| Empty custom fields return `null` in response | Context7 (JSM API examples) |
| Separate subcommands > try-and-fallback | Perplexity (CLI design best practices, gh/kubectl patterns) |
| Lazy discovery best practice for optional features | Perplexity |
| Assets API object has `label`, `objectKey`, `objectType` | Existing `AssetObject` type tested against real API |
