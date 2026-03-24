# Assets/CMDB Support — Design Spec

## Goal

Add standalone Assets/CMDB support to `jr`, enabling users to search assets via AQL, view asset details, and list connected Jira issues. Assets is workspace-scoped (not project-scoped) and works across all project types — this is Layer 1 of a two-layer design where Layer 2 (future) adds project-level integrations (e.g., filtering issues by asset custom fields).

## Background

Atlassian Assets (formerly Insight) is a CMDB that lives at the Jira Cloud site level. Asset objects (e.g., clients, servers, software) can be linked to issues across any Jira project type — Software, JSM, or Business. The Assets REST API is a separate API surface at `api.atlassian.com/ex/jira/{cloudId}/jsm/assets/workspace/{workspaceId}/v1/`, requiring a workspace ID that is site-wide and discovered via a dedicated endpoint.

The `jr` codebase uses product-namespaced modules: `api/jira/`, `api/jsm/`, `types/jira/`, `types/jsm/`. Assets adds `api/assets/` and `types/assets/` as peers.

## Architecture

### Workspace ID Discovery + Cache

Assets API calls require a `workspaceId`. This is site-wide (not per-project) and discovered once:

1. `GET {instance_url}/rest/servicedeskapi/assets/workspace` → returns a paginated `ServiceDeskPage`, e.g. `{ "values": [{ "workspaceId": "..." }] }`. Uses `get_from_instance()` (same pattern as JSM endpoints).
2. Cache in `~/.cache/jr/workspace.json` with 7-day TTL (separate from `project_meta.json` since workspace ID is site-wide).

Cache structure:

```json
{
  "workspace_id": "abc-123-def-456",
  "fetched_at": "2026-03-24T12:00:00Z"
}
```

Cache invalidation strategy (planned): on 404 from Assets endpoints, clear the workspace cache and re-fetch. The current implementation does not yet perform automatic invalidation and retry; it may be added in a future iteration.

Cache functions in `src/cache.rs` (filesystem only):

```rust
pub fn read_workspace_cache() -> Result<Option<WorkspaceCache>>
pub fn write_workspace_cache(workspace_id: &str) -> Result<()>
```

`write_workspace_cache` sets `fetched_at` internally (matching `write_team_cache` pattern).

Orchestration function in `src/api/assets/workspace.rs`:

```rust
pub async fn get_or_fetch_workspace_id(client: &JiraClient) -> Result<String>
```

### API Layer

Assets API calls live in `src/api/assets/`, peer to `api/jira/` and `api/jsm/`. Methods are implemented on the existing `JiraClient`.

**URL construction:** Assets endpoints use a third base URL pattern:

| API Surface | Base URL | Method |
|-------------|----------|--------|
| Platform (Jira) | `{instance_url}/rest/api/3/...` | `get()` |
| JSM | `{instance_url}/rest/servicedeskapi/...` | `get_from_instance()` |
| Assets | `{assets_base_url}/...` | new `get_assets()` / `post_assets()` |

**`JiraClient` changes:** Add a new `assets_base_url` field to `JiraClient`:

- In `from_config()`: If `cloud_id` is present, set `assets_base_url` to `https://api.atlassian.com/ex/jira/{cloud_id}/jsm/assets`. If `cloud_id` is absent, set to `None`.
- In `new_for_test()`: Set `assets_base_url` to `Some("{base_url}/jsm/assets")` so wiremock intercepts assets calls on the same mock server. Test paths become `/jsm/assets/workspace/{workspaceId}/v1/object/...`.
- `get_assets(workspace_id, path)` constructs: `{assets_base_url}/workspace/{workspace_id}/v1/{path}`.
- `post_assets(workspace_id, path, body)` same pattern for POST.

```rust
// Added to JiraClient
assets_base_url: Option<String>,

pub async fn get_assets<T: DeserializeOwned>(
    &self,
    workspace_id: &str,
    path: &str,
) -> anyhow::Result<T> {
    let base = self.assets_base_url.as_ref().ok_or_else(|| {
        JrError::ConfigError("Cloud ID not configured. Run \"jr init\" to set up your instance.".into())
    })?;
    let url = format!("{}/workspace/{}/v1/{}", base, workspace_id, path);
    let request = self.client.get(&url);
    let response = self.send(request).await?;
    Ok(response.json::<T>().await?)
}
```

This approach ensures:
1. Production: routes to `api.atlassian.com` via `cloud_id`
2. Tests: routes to wiremock via `base_url`
3. Missing `cloud_id`: clear error message
4. Same auth header used everywhere

**Important:** The workspace discovery endpoint still uses `get_from_instance()` (hits the real instance URL), since the discovery endpoint is at `/rest/servicedeskapi/assets/workspace` on the instance, not the API gateway.

### Pagination

The `POST /object/aql` endpoint returns:

```json
{
  "startAt": 0,
  "maxResults": 25,
  "total": 5,
  "isLast": "false",
  "values": [...]
}
```

**Critical:** `isLast` may be returned as a string (`"false"`) or boolean (`false`) depending on context. The `AssetsPage<T>` type needs a custom serde deserializer for this field.

`total` is capped at 1000. A `hasMoreResults` field indicates if more exist. For pagination, use `isLast` as the stop condition (not computed from `total`).

```rust
#[derive(Debug, Deserialize)]
pub struct AssetsPage<T> {
    #[serde(rename = "startAt", default)]
    pub start_at: u32,
    #[serde(rename = "maxResults", default)]
    pub max_results: u32,
    #[serde(default)]
    pub total: u32,
    #[serde(rename = "isLast", deserialize_with = "deserialize_bool_or_string")]
    pub is_last: bool,
    #[serde(default)]
    pub values: Vec<T>,
}
```

The `deserialize_bool_or_string` function handles both `true`/`false` and `"true"`/`"false"`.

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src/api/assets/mod.rs` | Re-exports workspace, objects, tickets |
| `src/api/assets/workspace.rs` | Workspace ID discovery, `get_assets()` helper, `get_or_fetch_workspace_id()` |
| `src/api/assets/objects.rs` | `search_assets()` (AQL), `get_asset()` (by ID), `resolve_object_key()` |
| `src/api/assets/tickets.rs` | `get_connected_tickets()` |
| `src/types/assets/mod.rs` | Re-exports object, ticket |
| `src/types/assets/object.rs` | `AssetObject`, `ObjectType`, `AssetAttribute`, `ObjectAttributeValue` |
| `src/types/assets/ticket.rs` | `ConnectedTicketsResponse`, `ConnectedTicket`, `TicketStatus`, `TicketType`, `TicketPriority` |
| `src/cli/assets.rs` | `jr assets search`, `jr assets view`, `jr assets tickets` handlers |
| `tests/assets.rs` | Integration tests with wiremock |

### Modified Files

| File | Change |
|------|--------|
| `src/api/client.rs` | Add `assets_base_url` field, `get_assets()`, `post_assets()`, update `from_config()` and `new_for_test()` |
| `src/api/mod.rs` | Add `pub mod assets;` |
| `src/types/mod.rs` | Add `pub mod assets;` |
| `src/api/pagination.rs` | Add `AssetsPage<T>` with `deserialize_bool_or_string` |
| `src/cache.rs` | Add `WorkspaceCache`, `read_workspace_cache()`, `write_workspace_cache()` |
| `src/cli/mod.rs` | Add `Assets` command variant and `AssetsCommand` enum |
| `src/main.rs` | Add dispatch for `Command::Assets` |

## Types

### AssetsPage<T>

```rust
// src/api/pagination.rs
#[derive(Debug, Deserialize)]
pub struct AssetsPage<T> {
    #[serde(rename = "startAt", default)]
    pub start_at: u32,
    #[serde(rename = "maxResults", default)]
    pub max_results: u32,
    #[serde(default)]
    pub total: u32,
    #[serde(rename = "isLast", deserialize_with = "deserialize_bool_or_string")]
    pub is_last: bool,
    #[serde(default)]
    pub values: Vec<T>,
}

impl<T> AssetsPage<T> {
    pub fn has_more(&self) -> bool {
        !self.is_last
    }

    pub fn next_start(&self) -> u32 {
        self.start_at + self.max_results
    }
}
```

### WorkspaceCache

```rust
// src/cache.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceCache {
    pub workspace_id: String,
    pub fetched_at: DateTime<Utc>,
}
```

### AssetObject

```rust
// src/types/assets/object.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct AssetObject {
    pub id: String,
    pub label: String,
    #[serde(rename = "objectKey")]
    pub object_key: String,
    #[serde(rename = "objectType")]
    pub object_type: ObjectType,
    pub created: Option<String>,
    pub updated: Option<String>,
    #[serde(default)]
    pub attributes: Vec<AssetAttribute>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectType {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetAttribute {
    pub id: String,
    #[serde(rename = "objectTypeAttributeId")]
    pub object_type_attribute_id: String,
    #[serde(rename = "objectAttributeValues", default)]
    pub values: Vec<ObjectAttributeValue>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectAttributeValue {
    pub value: Option<String>,
    #[serde(rename = "displayValue")]
    pub display_value: Option<String>,
}
```

### ConnectedTicket

Connected tickets have a different shape than standard Jira issues — `title` instead of `summary`, nested `status`/`type`/`priority` with different structures.

```rust
// src/types/assets/ticket.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectedTicketsResponse {
    #[serde(default)]
    pub tickets: Vec<ConnectedTicket>,
    #[serde(rename = "allTicketsQuery")]
    pub all_tickets_query: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectedTicket {
    pub key: String,
    pub id: String,
    pub title: String,
    pub reporter: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub status: Option<TicketStatus>,
    #[serde(rename = "type")]
    pub issue_type: Option<TicketType>,
    pub priority: Option<TicketPriority>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TicketStatus {
    pub name: String,
    #[serde(rename = "colorName")]
    pub color_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TicketType {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TicketPriority {
    pub name: String,
}
```

## API Methods

All methods are implemented on `JiraClient` in `src/api/assets/`.

### get_or_fetch_workspace_id

```
GET {instance_url}/rest/servicedeskapi/assets/workspace
```

Returns workspace ID from cache or API. Caches site-wide.

### get_assets (helper)

Constructs URL: `https://api.atlassian.com/ex/jira/{cloud_id}/jsm/assets/workspace/{workspace_id}/v1/{path}`. Sends GET request with same auth header.

### post_assets (helper)

Same URL construction as `get_assets` but for POST requests (AQL search).

### search_assets

```
POST .../v1/object/aql?startAt={start}&maxResults={limit}&includeAttributes={attrs}
Body: {"qlQuery": "<aql>"}
```

Auto-paginates using `AssetsPage<AssetObject>`. Returns `Vec<AssetObject>`.

### get_asset

```
GET .../v1/object/{id}?includeAttributes={attrs}
```

Returns single `AssetObject`.

### resolve_object_key

Resolves an object key (e.g., `OBJ-1`) to its numeric ID via AQL:

```
POST .../v1/object/aql?maxResults=1&includeAttributes=false
Body: {"qlQuery": "Key = \"OBJ-1\""}
```

Returns the ID or errors with "No asset matching ...". If the input is purely numeric, treats it as an ID directly (skip AQL).

### get_connected_tickets

```
GET .../v1/objectconnectedtickets/{objectId}/tickets
```

Returns `ConnectedTicketsResponse`. No pagination — returns all tickets in one response. The `--limit` flag on the CLI command performs client-side truncation only.

## CLI Commands

### jr assets search

```
$ jr assets search "objectType = Client"

 Key       Type     Name
 ──────────────────────────────────
 OBJ-1     Client   Acme Corp
 OBJ-2     Client   Globex Inc
 OBJ-3     Client   Initech
```

With `--attributes` adds `Created` and `Updated` columns to the table output and passes `includeAttributes=true` to the API.

JSON mode returns the full `AssetObject` array. The `--attributes` flag controls whether `includeAttributes=true` is passed to the API — this applies to both table and JSON output. Without `--attributes`, JSON output includes the object metadata but no attributes array.

### jr assets view

```
$ jr assets view OBJ-1

 Field          Value
 ──────────────────────────
 Key            OBJ-1
 Type           Client
 Name           Acme Corp
 Created        2025-12-17 14:58
 Updated        2026-01-29 19:52
```

With `--attributes` adds an attributes section showing `displayValue` for each attribute. Note: attribute names are shown by their `objectTypeAttributeId` (opaque IDs) since resolving IDs to human-readable names requires additional API calls to object type attribute metadata — this is out of scope for Layer 1 but can be added later.

Accepts either an object key (e.g., `OBJ-1`) or numeric ID. Object keys are resolved to IDs via AQL.

### jr assets tickets

```
$ jr assets tickets OBJ-1

 Key       Type              Title                                    Status        Priority
 ──────────────────────────────────────────────────────────────────────────────────────────────
 PROJ-42   Service Request   VPN access not working after update      In Progress   High
 PROJ-38   Change Issue      Update firewall rules for new subnet     Closed        Medium
```

JSON mode returns the full `ConnectedTicketsResponse` object (includes `tickets` array and `allTicketsQuery` JQL). The `--limit` flag only affects table output; JSON always returns the full response.

### CLI Enum

```rust
// src/cli/mod.rs
#[derive(Subcommand)]
pub enum Command {
    // ... existing commands ...

    /// Manage Assets/CMDB objects
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
}

#[derive(Subcommand)]
pub enum AssetsCommand {
    /// Search assets with AQL query
    Search {
        /// AQL query (e.g. "objectType = Client")
        query: String,
        /// Maximum number of results
        #[arg(long)]
        limit: Option<u32>,
        /// Include object attributes in output
        #[arg(long)]
        attributes: bool,
    },
    /// View asset details
    View {
        /// Object key (e.g. OBJ-1) or numeric ID
        key: String,
        /// Include object attributes in output
        #[arg(long)]
        attributes: bool,
    },
    /// Show Jira issues connected to an asset
    Tickets {
        /// Object key (e.g. OBJ-1) or numeric ID
        key: String,
        /// Maximum number of tickets to show
        #[arg(long)]
        limit: Option<u32>,
    },
}
```

## Error Handling

All errors follow the project convention: suggest what to do next.

| Scenario | Error Message |
|----------|--------------|
| No Assets workspace | `Error: Assets is not available on this Jira site. Assets requires Jira Service Management Premium or Enterprise.` |
| No cloud_id in config | `Error: Cloud ID not configured. Run "jr init" to set up your instance.` |
| Object not found | `Error: No asset matching "OBJ-99" found. Check the object key and try again.` |
| Invalid AQL syntax | Pass through the API error message (Jira returns descriptive AQL parse errors) |
| No connected tickets | Display "No results found." (dimmed text, not an error — same pattern as `jr issue list` with no results) |
| Workspace cache miss + API failure | Falls through to standard network error handling in `JiraClient` |

## Testing Strategy

### Unit Tests

| Test | Location |
|------|----------|
| `AssetsPage<T>` deserialization — `isLast` as boolean and as string | `src/api/pagination.rs` |
| `AssetsPage<T>` `has_more()` and `next_start()` | `src/api/pagination.rs` |
| `WorkspaceCache` read/write/TTL/expiry | `src/cache.rs` |
| `AssetObject` deserialization with/without attributes | `src/types/assets/object.rs` |
| `ConnectedTicketsResponse` deserialization | `src/types/assets/ticket.rs` |
| Object key detection (key vs numeric ID) | `src/api/assets/objects.rs` or `src/cli/assets.rs` |

### Integration Tests (wiremock)

| Test | File |
|------|------|
| `jr assets search` — returns objects | `tests/assets.rs` |
| `jr assets search` — empty results | `tests/assets.rs` |
| `jr assets search` — pagination across pages | `tests/assets.rs` |
| `jr assets view` — returns object details | `tests/assets.rs` |
| `jr assets view` — object not found | `tests/assets.rs` |
| `jr assets tickets` — returns connected tickets | `tests/assets.rs` |
| `jr assets tickets` — no connected tickets | `tests/assets.rs` |
| Workspace ID discovery — cache miss fetches from API | `tests/assets.rs` |

## Out of Scope

- Asset creation, update, or deletion (read-only for now)
- Object schema management (`/objectschema/`)
- Object type listing (`/objecttype/`)
- Layer 2 project integrations (`--client` flag on `jr issue list`, `jr queue view`)
- Attribute name resolution (would need object type attribute metadata to map IDs to names)
- Object references (showing linked assets from an asset)
- Assets import API

## Dependencies

No new crate dependencies. Uses existing `reqwest`, `serde`, `chrono`, `comfy-table`, `colored`.

## Migration

No migration needed. Purely additive — new commands, new API module, new types. Existing commands and behavior are unchanged.
