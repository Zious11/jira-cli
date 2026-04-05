# JSM Queue Support — Design Spec

## Goal

Add Jira Service Management (JSM) queue support to `jr`, enabling service desk agents to list queues and view queue contents from the CLI. This is the first JSM feature, establishing the foundational infrastructure (project type detection, JSM API module, servicedeskapi pagination) that future JSM features (SLAs, request types, customers) will build on.

## Background

The `jr` CLI currently wraps Jira REST API v3 and Agile REST API. JSM projects use JSM-specific issue types and workflows, but the JSM-specific features — queues, SLAs, request types, customers — are only accessible through the separate `/rest/servicedeskapi/` API surface.

The codebase was designed for multi-product expansion: `src/api/jira/` and `src/types/jira/` are product-namespaced so future products add sibling directories.

## Architecture

### Project Type Detection + Cache

JSM commands need to know whether a project is a service desk and what its `serviceDeskId` is. This is resolved transparently at runtime:

1. `GET /rest/api/3/project/{key}` → extract `projectTypeKey` (`software` | `service_desk` | `business`) and `simplified` (team-managed vs company-managed) and `id` (numeric project ID)
2. If `service_desk`: `GET /rest/servicedeskapi/servicedesk` → paginate through results, match by `projectId` to find `serviceDeskId`
3. Cache result in `~/.cache/jr/project_meta.json` with 7-day TTL

Cache structure:

```json
{
  "HELPDESK": {
    "project_type": "service_desk",
    "simplified": false,
    "project_id": "10042",
    "service_desk_id": "15",
    "fetched_at": "2026-03-24T12:00:00Z"
  }
}
```

Cache invalidation: on 404 from JSM endpoints (project may have been recreated), clear entry and re-fetch.

Cache functions in `src/cache.rs` (filesystem only — no API calls):

```rust
pub fn read_project_meta(project_key: &str) -> Result<Option<ProjectMeta>>
pub fn write_project_meta(project_key: &str, meta: &ProjectMeta) -> Result<()>
```

Orchestration function in `src/api/jsm/servicedesks.rs` (calls API on cache miss):

```rust
pub async fn get_or_fetch_project_meta(client: &JiraClient, project_key: &str) -> Result<ProjectMeta>
```

### API Layer

JSM API calls live in a new `src/api/jsm/` module, sibling to `src/api/jira/`. Methods are implemented on the existing `JiraClient` — same auth, same HTTP infrastructure, different base path (`/rest/servicedeskapi/` on the instance URL instead of `/rest/api/3`).

**Important:** JSM methods MUST use the existing `get_from_instance()` / `post_to_instance()` methods on `JiraClient` (not `get()` / `post()` which use `base_url`). The `get_from_instance` methods construct URLs from `instance_url`, which is correct for both API-token and OAuth auth flows. Using `get()` would break OAuth users because `base_url` points to the API proxy (`https://api.atlassian.com/ex/jira/{cloudId}`) which does not serve `/rest/servicedeskapi/`.

### Pagination

The servicedeskapi uses a `PagedDTO` pagination format distinct from the platform API:

```json
{
  "size": 5,
  "start": 0,
  "limit": 50,
  "isLastPage": false,
  "values": [...],
  "_links": { "next": "...", "self": "..." }
}
```

| Field | Meaning |
|-------|---------|
| `size` | Count of items in current page |
| `start` | Zero-based starting index |
| `limit` | Max items per page |
| `isLastPage` | Whether this is the last page |
| `values` | Array of result objects |
| `_links` | Navigation links (self, next, prev) |

This is a new `ServiceDeskPage<T>` struct in `src/api/pagination.rs`, separate from the existing `OffsetPage<T>`. Includes `has_more()` and `next_start()` helper methods matching the `OffsetPage<T>` pattern.

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src/api/jsm/mod.rs` | Re-exports servicedesks, queues |
| `src/api/jsm/servicedesks.rs` | `list_service_desks()` — resolve serviceDeskId from project |
| `src/api/jsm/queues.rs` | `list_queues()`, `get_queue_issues()` |
| `src/types/jsm/mod.rs` | Re-exports servicedesk, queue |
| `src/types/jsm/servicedesk.rs` | `ServiceDesk` struct |
| `src/types/jsm/queue.rs` | `Queue`, `QueueIssue`, `QueueIssueFields` structs |
| `src/cli/queue.rs` | `jr queue list`, `jr queue view` handlers |
| `tests/queue.rs` | Integration tests with wiremock |
| `tests/project_meta.rs` | Integration tests for project type detection + cache |

### Modified Files

| File | Change |
|------|--------|
| `src/api/mod.rs` | Add `pub mod jsm;` |
| `src/types/mod.rs` | Add `pub mod jsm;` |
| `src/api/pagination.rs` | Add `ServiceDeskPage<T>` struct |
| `src/api/client.rs` | Add `pub fn instance_url(&self) -> &str` accessor |
| `src/cache.rs` | Add `ProjectMeta` struct, `read_project_meta()`, `write_project_meta()` with TTL logic |
| `src/cli/mod.rs` | Add `Queue` command variant and `QueueCommand` enum |
| `src/main.rs` | Add dispatch for `Command::Queue` |

## Types

### ServiceDeskPage<T>

```rust
// src/api/pagination.rs
#[derive(Debug, Deserialize)]
pub struct ServiceDeskPage<T> {
    pub size: u32,
    pub start: u32,
    pub limit: u32,
    #[serde(rename = "isLastPage")]
    pub is_last_page: bool,
    #[serde(default)]
    pub values: Vec<T>,
}

impl<T> ServiceDeskPage<T> {
    pub fn has_more(&self) -> bool {
        !self.is_last_page
    }

    pub fn next_start(&self) -> u32 {
        self.start + self.size
    }
}
```

### ProjectMeta

```rust
// src/cache.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub project_type: String,
    pub simplified: bool,
    pub project_id: String,
    pub service_desk_id: Option<String>,
    pub fetched_at: DateTime<Utc>,
}
```

Note: `project_id` and `service_desk_id` are `String` (not numeric) to match Jira's convention — all Jira IDs are strings. See `Transition.id` in `types/jira/issue.rs` for precedent.

All JSM-specific errors use `JrError::UserError(String)` (exit code 64), consistent with other user-facing validation errors in the codebase.

### ServiceDesk

```rust
// src/types/jsm/servicedesk.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct ServiceDesk {
    pub id: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(rename = "projectName")]
    pub project_name: String,
}
```

### Queue

```rust
// src/types/jsm/queue.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct Queue {
    pub id: String,
    pub name: String,
    pub jql: Option<String>,
    pub fields: Option<Vec<String>>,
    #[serde(rename = "issueCount")]
    pub issue_count: Option<u64>,
}
```

### QueueIssue

Queue issues return a limited field set — only the fields configured for that queue, not full Jira issue objects.

```rust
// src/types/jsm/queue.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct QueueIssue {
    pub key: String,
    pub fields: QueueIssueFields,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QueueIssueFields {
    pub summary: Option<String>,
    pub status: Option<Status>,
    pub issuetype: Option<IssueType>,
    pub priority: Option<Priority>,
    pub assignee: Option<User>,
    pub reporter: Option<User>,
    pub created: Option<String>,
}
```

`Status`, `IssueType`, `Priority`, and `User` are reused from `src/types/jira/issue.rs` and `src/types/jira/user.rs`.

Note: The existing `User` struct has `account_id: String` as a required field. If the servicedeskapi returns user objects without `accountId`, the `User` struct may need `account_id` changed to `Option<String>` or a separate `QueueUser` type. Verify during implementation by checking actual API responses.

## API Methods

All methods are implemented on `JiraClient` in the `src/api/jsm/` module.

### list_service_desks

```
GET {instance_url}/rest/servicedeskapi/servicedesk?start={start}&limit=50
```

Auto-paginates using `ServiceDeskPage<ServiceDesk>`. Returns `Vec<ServiceDesk>`.

### list_queues

```
GET {instance_url}/rest/servicedeskapi/servicedesk/{serviceDeskId}/queue?includeCount=true&start={start}&limit=50
```

Auto-paginates. The `includeCount=true` parameter includes `issueCount` per queue. Returns `Vec<Queue>`.

### get_queue_issues

```
GET {instance_url}/rest/servicedeskapi/servicedesk/{serviceDeskId}/queue/{queueId}/issue?start={start}&limit={limit}
```

Paginates with optional user-specified limit (same pattern as `list_comments`). Returns `Vec<QueueIssue>`.

## CLI Commands

### jr queue list

Lists all queues for the current project's service desk.

```
$ jr queue list --project HELPDESK

 Queue                          Issues
 ─────────────────────────────────────
 Triage                             12
 In Progress                         7
 Escalated                           3
 Waiting for Customer                2
 Resolution Request                  1
 All open                           25
```

JSON mode returns array of queue objects with `id`, `name`, `issue_count`, `jql`.

### jr queue view

Shows issues in a specific queue. Queue name supports partial matching via existing `partial_match.rs`.

```
$ jr queue view triage --project HELPDESK

 Key           Type              Summary                                  Status    Assignee
 ────────────────────────────────────────────────────────────────────────────────────────────
 HELPDESK-42   Service Request   VPN access not working after update      New       —
 HELPDESK-41   Service Request   Need software license renewal            New       —
 HELPDESK-40   Change Issue      Update firewall rules for new subnet     New       Jane D.
```

Supports `--limit N` for pagination and `--id N` as escape hatch for duplicate queue names.

### CLI Enum

```rust
// src/cli/mod.rs
#[derive(Subcommand)]
pub enum Command {
    // ... existing commands ...

    /// Manage JSM queues
    Queue {
        #[command(subcommand)]
        command: QueueCommand,
    },
}

#[derive(Subcommand)]
pub enum QueueCommand {
    /// List queues for the service desk
    List,
    /// View issues in a queue
    View {
        /// Queue name (partial match supported)
        name: Option<String>,
        /// Queue ID (use if name is ambiguous)
        #[arg(long)]
        id: Option<String>,
        /// Maximum number of issues to return
        #[arg(long)]
        limit: Option<u32>,
    },
}
```

If neither `name` nor `--id` is provided, error with: `Error: Specify a queue name or use --id. Run "jr queue list" to see available queues.`

**JSON output for `jr queue view`:** Returns a JSON array of `QueueIssue` objects, consistent with `jr issue list` returning an array of issues.

```json
[
  {"key": "PROJ-100", "fields": {"summary": "...", "status": {"name": "New", ...}, ...}},
  ...
]
```

## Error Handling

All errors follow the project convention: suggest what to do next.

| Scenario | Error Message |
|----------|--------------|
| Non-JSM project | `Error: "{key}" is a Jira Software project. Queue commands require a Jira Service Management project. Run "jr project fields {key}" to see available commands.` |
| Not an agent | `Error: You don't have agent access to the "{name}" service desk. Contact your JSM administrator.` |
| No matching queue | `Error: No queue matching "{name}" found. Run "jr queue list" to see available queues.` |
| Ambiguous queue name | `Error: "{name}" matches multiple queues: "{q1}", "{q2}". Be more specific or use --id.` |
| Duplicate queue names | `Error: Multiple queues named "{name}" found (IDs: {id1}, {id2}). Use --id {id1} to specify.` |
| Service desk not found | `Error: No service desk found for project "{key}". The project may not be configured as a service desk.` |
| Cache miss + API failure | Falls through to standard network error handling in `JiraClient` |

## Testing Strategy

### Unit Tests

| Test | Location |
|------|----------|
| `ServiceDeskPage<T>` deserialization (empty, single page, multiple items) | `src/api/pagination.rs` |
| `ProjectMeta` cache: write, read, TTL expiry, cache miss | `src/cache.rs` |
| Queue partial matching (exact, prefix, ambiguous, no match, duplicate names) | `src/cli/queue.rs` |
| `QueueIssue` deserialization with missing optional fields | `src/types/jsm/queue.rs` |

### Integration Tests (wiremock)

| Test | File |
|------|------|
| `jr queue list` — returns queues with issue counts | `tests/queue.rs` |
| `jr queue list` — empty queues | `tests/queue.rs` |
| `jr queue view` — returns queue issues | `tests/queue.rs` |
| `jr queue view --limit` — respects limit | `tests/queue.rs` |
| `jr queue view` — pagination across pages | `tests/queue.rs` |
| `jr queue view` — partial name match | `tests/queue.rs` |
| `jr queue` on non-JSM project — error message | `tests/queue.rs` |
| Project type detection — cache miss fetches from API | `tests/project_meta.rs` |
| Project type detection — cache hit skips API call | `tests/project_meta.rs` |
| Project type detection — expired TTL re-fetches | `tests/project_meta.rs` |

### Snapshot Tests (insta)

Table output for `jr queue list` and `jr queue view` to catch formatting regressions.

## Out of Scope

- SLA tracking (`/rest/servicedeskapi/request/{key}/sla`) — future spec
- Request types (`/rest/servicedeskapi/servicedesk/{id}/requesttype`) — future spec
- Customer/organization management — future spec
- Assets/CMDB — future spec
- Changes to `jr init` — project type is auto-detected at runtime
- Queue write operations (create/delete/reorder queues) — read-only for now
- `--full` flag on `jr queue view` to re-query via platform API — future enhancement

## Dependencies

No new crate dependencies. Uses existing `reqwest`, `serde`, `chrono`, `comfy-table`, `colored`.

## Migration

No migration needed. This is purely additive — new commands, new API module, new types. Existing commands and behavior are unchanged.
