# Design: `jr project list` for Project Discovery

**Issue:** #47 — Add 'jr project list' to discover available projects
**Date:** 2026-03-25
**Status:** Draft

## Problem

There is no way to discover which Jira projects are available from the CLI. `jr project` only has a `fields` subcommand that requires already knowing the project key. New users and AI agents have no programmatic way to look up valid project keys.

For AI agents specifically, project discovery is a critical primitive. Agents need to verify valid parameter values before constructing commands. Without `jr project list`, agents must guess project keys or rely on out-of-band documentation, leading to avoidable API errors and recovery loops.

## Scope

Three deliverables:

1. **`jr project list` command** — list accessible projects with key, name, lead, and type
2. **`--type` filter** — filter by project type (software, service_desk, business)
3. **Error message enhancement** — suggest valid projects when an invalid key is used

## API Endpoint

**Chosen: `GET /rest/api/3/project/search`** over `/rest/api/3/project`.

The `/project/search` endpoint is purpose-built for listing: it supports server-side pagination (`startAt` + `maxResults`), server-side filtering (`typeKey`), ordering (`orderBy`), and returns richer per-project data (`projectTypeKey`, `lead` with `displayName`). The simpler `/project` endpoint returns all projects in a flat array with no pagination — unsuitable for large instances.

### Response Structure

The response uses the standard offset-based pagination envelope (`startAt`, `maxResults`, `total`) with projects in a `values` array. This matches the existing `OffsetPage<T>` generic in `src/api/pagination.rs`.

```json
{
  "values": [
    {
      "key": "FOO",
      "name": "Project Alpha",
      "projectTypeKey": "software",
      "lead": {
        "accountId": "abc-123",
        "displayName": "Jane Doe"
      }
    }
  ],
  "startAt": 0,
  "maxResults": 50,
  "total": 12
}
```

## Command Design

### Flag Definition

```
jr project list [--type <TYPE>] [--limit <N>] [--all]
```

| Flag | Type | Description |
|------|------|-------------|
| `--type` | `Option<String>` | Filter by project type: `software`, `service_desk`, `business` |
| `--limit` | `Option<u32>` | Maximum results (default: 50, API max: 50) |
| `--all` | `bool` | Fetch all projects (paginate through all pages) |

- **`--all` flag:** The API caps at 50 results per page. Instances with >50 projects need pagination. `--all` loops through pages using `startAt` offsets until all projects are returned. Conflicts with `--limit`.
- **`--type` validation:** No client-side validation. Invalid values produce an HTTP 400 from the API with an error message. This is consistent with how `--status` works on `jr issue list`.
- **`--limit` clamping:** Values above 50 are clamped to 50 before sending to the API (the API's maximum per page).

### Table Output

```
Key    Name                    Lead          Type
ABC    Project Alpha           Jane Doe      software
DEF    Operations Desk         John Smith    service_desk
GHI    Platform Engineering    Alex Jones    software
```

### JSON Output

`--output json` returns a JSON array of project objects with camelCase keys (matching Jira's native field names, consistent with how other types in this codebase use per-field `#[serde(rename)]`):

```json
[
  {
    "key": "ABC",
    "name": "Project Alpha",
    "projectTypeKey": "software",
    "lead": { "displayName": "Jane Doe", "accountId": "abc-123" }
  }
]
```

## Types

New types in `src/types/jira/project.rs` alongside the existing `Project` struct:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectSummary {
    pub key: String,
    pub name: String,
    #[serde(rename = "projectTypeKey")]
    pub project_type_key: String,
    pub lead: Option<ProjectLead>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectLead {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
}
```

The existing `Project` struct (just `key` + `name`) remains unchanged — it's used in other contexts.

No custom response wrapper needed — the `list_projects` API method uses `OffsetPage<ProjectSummary>` from `src/api/pagination.rs` to deserialize the paginated response, then extracts `.items()`.

## API Method

New method on `JiraClient` in `src/api/jira/projects.rs`:

```rust
pub async fn list_projects(
    &self,
    type_key: Option<&str>,
    max_results: Option<u32>,
) -> Result<Vec<ProjectSummary>>
```

- Calls `GET /rest/api/3/project/search` with query params: `orderBy=key`, optional `typeKey`, optional `maxResults` (clamped to 50)
- Deserializes as `OffsetPage<ProjectSummary>`, returns `.items().to_vec()`
- When `max_results` is `None` (the `--all` case), paginates using `startAt` offsets until `OffsetPage::has_more()` returns false, collecting all pages into a single `Vec`

## Error Message Enhancement

Static `Run "jr project list" to see available projects.` hints are appended to error messages where project keys are missing or invalid. This avoids the complexity of dynamic project matching (which would require parsing Jira's error responses to distinguish project-specific 404s from other failures) while still guiding users to the discovery command.

### Touchpoints

1. **`src/cli/project.rs`** — `"No project specified"` error includes `jr project list` hint
2. **`src/cli/issue/create.rs`** — `"Project key is required"` error includes `jr project list` hint
3. **`src/cli/queue.rs`** — `"No project configured"` error includes `jr project list` hint

The issue list command (`src/cli/issue/list.rs`) doesn't hard-fail on invalid projects — it passes the key into JQL and Jira returns empty results. No enhancement needed there.

## Files Changed

| File | Change |
|------|--------|
| `src/types/jira/project.rs` | Add `ProjectSummary`, `ProjectLead` types |
| `src/api/jira/projects.rs` | Add `list_projects` method using `OffsetPage<ProjectSummary>` |
| `src/cli/mod.rs` | Add `List` variant to `ProjectCommand` with `--type`, `--limit`, `--all` |
| `src/cli/project.rs` | Add `handle_list` handler, enhance "No project specified" error |
| `src/cli/issue/create.rs` | Enhance "Project key is required" error with `jr project list` hint |
| `src/cli/queue.rs` | Enhance "No project configured" error with `jr project list` hint |
| `README.md` | Add `jr project list` to command table and quick start |

No new runtime modules or API endpoints beyond the single `/project/search` call.

## Testing

### Integration Tests

In `tests/project_commands.rs`:
- `test_list_projects` — mock `/rest/api/3/project/search`, verify response parsing and field mapping
- `test_list_projects_with_type_filter` — verify `typeKey` param passed to API
- `test_list_projects_empty` — verify empty result handling (prints "No results found.")
- `test_list_projects_lead_missing` — verify graceful handling when `lead` is null
