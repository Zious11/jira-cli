# Add Statuses to `project fields` — Design Spec

**Issue:** #55 — `project fields` omits statuses despite help text promising them

**Goal:** Add project statuses grouped by issue type to `jr project fields` output (table and JSON).

## Problem

`jr project fields` help text says "Show valid issue types, priorities, and statuses" but the implementation only fetches and displays issue types and priorities. Statuses are missing from both table and JSON output. Users must use `jr issue transitions <KEY>` on an existing issue to discover statuses, which requires already knowing an issue key.

## API

**Endpoint:** `GET /rest/api/3/project/{projectIdOrKey}/statuses`

- Accepts both project key (e.g., `PROJ`) and numeric project ID
- Permission: Browse Projects (same as existing project calls)
- Authentication: required (handled by `JiraClient`)

**Response:** Top-level array of issue type objects, each with a nested `statuses` array:

```json
[
  {
    "id": "3",
    "name": "Task",
    "self": "https://your-domain.atlassian.net/rest/api/3/issueType/3",
    "subtask": false,
    "statuses": [
      {
        "id": "10000",
        "name": "In Progress",
        "description": "The issue is currently being worked on.",
        "iconUrl": "https://your-domain.atlassian.net/images/icons/progress.gif",
        "self": "https://your-domain.atlassian.net/rest/api/3/status/10000"
      }
    ]
  }
]
```

## Design

### Serde types (`src/api/jira/projects.rs`)

Two new structs following the existing `IssueTypeMetadata` / `PriorityMetadata` pattern:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct StatusMetadata {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueTypeWithStatuses {
    pub id: String,
    pub name: String,
    pub subtask: Option<bool>,
    pub statuses: Vec<StatusMetadata>,
}
```

`IssueTypeWithStatuses` is a purpose-specific struct for the `/statuses` endpoint response — it is distinct from the existing `IssueTypeMetadata` (which comes from the `/project/{key}` endpoint and has no `statuses` field). No consolidation needed.

`subtask` is `Option<bool>` for defensive deserialization, matching the existing `IssueTypeMetadata` pattern — the API always returns it as a boolean, but `Option` prevents hard failures on unexpected responses.

Only fields needed for display are included (`id`, `name`, `description`). Unknown fields (`iconUrl`, `self`, potential `statusCategory`) are silently ignored by serde's default behavior, matching the existing codebase pattern.

### API method (`src/api/jira/projects.rs`)

```rust
pub async fn get_project_statuses(&self, project_key: &str) -> Result<Vec<IssueTypeWithStatuses>> {
    self.get(&format!("/rest/api/3/project/{project_key}/statuses")).await
}
```

### CLI handler (`src/cli/project.rs`)

Add a third fetch in `handle_fields` after priorities:

```rust
let statuses = client.get_project_statuses(&project_key).await?;
```

### Table output

Append a "Statuses by Issue Type" section after Priorities. Skip issue types that have an empty statuses list (newly created projects or unconfigured workflows):

```
Project: PROJ

Issue Types:
  - Task
  - Bug
  - Story (subtask)

Priorities:
  - Highest
  - High
  - Medium
  - Low
  - Lowest

Statuses by Issue Type:
  Task:
    - To Do
    - In Progress
    - Done
  Bug:
    - Open
    - In Progress
    - Closed
```

If the API returns an empty array (no issue types with statuses), omit the "Statuses by Issue Type" section entirely.

### JSON output

Add a `statuses_by_issue_type` field to the JSON object. The key is named `statuses_by_issue_type` (not `statuses`) because the value is an array of issue type objects with nested statuses — the name prevents confusion with a flat list of status objects:

```json
{
  "project": "PROJ",
  "issue_types": [...],
  "priorities": [...],
  "statuses_by_issue_type": [
    {
      "id": "3",
      "name": "Task",
      "subtask": false,
      "statuses": [
        {"id": "10000", "name": "In Progress", "description": "..."}
      ]
    }
  ]
}
```

The JSON output preserves the full API response structure (including `id`, `description`) for scripting use cases.

## Error handling

The new fetch uses `?` propagation (hard fail), unlike the existing `get_project_issue_types` which uses `.unwrap_or_default()` (soft fail on parse error). This is intentional: if the statuses endpoint fails due to permission issues or network errors, the user should see a clear error rather than silently missing data — which is the exact bug we're fixing.

## Testing

- **Integration test** in `tests/project_commands.rs`: mock `/rest/api/3/project/FOO/statuses`, call `get_project_statuses`, verify deserialization of issue types and nested statuses. Tests go in `project_commands.rs` (not `project_meta.rs`, which tests JSM service desk metadata caching).
- **Fixture helper** `project_statuses_response` in `tests/common/fixtures.rs`: returns a realistic response with 2 issue types, each having 2-3 statuses.

## Files Changed

| File | Change |
|------|--------|
| `src/api/jira/projects.rs` | Add `StatusMetadata`, `IssueTypeWithStatuses`, `get_project_statuses` |
| `src/cli/project.rs` | Fetch statuses, render in table + JSON |
| `tests/common/fixtures.rs` | Add `project_statuses_response` fixture |
| `tests/project_commands.rs` | Add integration test for `get_project_statuses` |

## Non-goals

- Parallel fetching with `tokio::join!` (unnecessary for infrequent command)
- Merging statuses into the issue types section (changes existing output format)
- Caching statuses (no caching exists for issue types or priorities today)
