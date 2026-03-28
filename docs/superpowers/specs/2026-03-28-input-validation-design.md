# Input Validation for issue list — Design Spec

## Problem

`jr issue list --project NONEXISTENT` returns "No results found" with exit code 0. The same happens with `--status "Nonexistant"`. Invalid filter values silently produce empty results instead of errors, making them indistinguishable from legitimate empty results.

An AI agent or script cannot tell the difference between "this project exists but has no matching issues" and "this project does not exist." Compare with `issue move` which correctly validates statuses and suggests alternatives.

**Issue:** #71

## Solution

Add pre-flight validation to `issue list` for two inputs:

1. **`--project` validation**: Before building JQL, check that the project exists via `GET /rest/api/3/project/{key}`. If 404, error with a suggestion to run `jr project list`.
2. **`--status` validation**: Before building JQL, fetch valid statuses and partial-match the `--status` value. If no match, error listing valid statuses. If ambiguous, error listing matches.

Both validations happen in `handle_list` (CLI layer), after resolving the project key but before building JQL. They are independent — neither requires the other.

### Project Validation

**New API method in `src/api/jira/projects.rs`:**

```rust
pub async fn project_exists(&self, key: &str) -> Result<bool>
```

Calls `GET /rest/api/3/project/{key}`. Returns `Ok(true)` on 200, `Ok(false)` on 404 (by catching `JrError::ApiError { status: 404, .. }`), propagates other errors (401, 500, network).

**Validation logic in `handle_list`:**

After resolving `project_key` (line 108 in `list.rs`) and before building JQL:

```rust
if let Some(ref pk) = project_key {
    if !client.project_exists(pk).await? {
        bail!(
            "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
            pk
        );
    }
}
```

**Error message:**
```
Project "NONEXISTENT" not found. Run "jr project list" to see available projects.
```

Exit code 1 (anyhow::bail, same as other runtime errors).

### Status Validation

**Strategy depends on whether `--project` is set:**

| `--project` set? | Endpoint | Why |
|-------------------|----------|-----|
| Yes | `GET /rest/api/3/project/{key}/statuses` | More precise — only statuses valid for that project's workflows. Also validates the project (404 if invalid), combining both checks into one API call. |
| No | `GET /rest/api/3/status` | Global list of all statuses in the instance. Still catches typos. |

**When `--project` IS set:** The project-scoped endpoint returns statuses grouped by issue type with duplicates across types. Extract unique status names into a `Vec<String>` (deduplicate by name). This replaces the separate `project_exists()` call — if this endpoint returns 404, the project doesn't exist.

**When `--project` is NOT set:** The global endpoint returns a flat array of `StatusDetails` with `name` fields. No deduplication needed.

**New API method in `src/api/jira/projects.rs`:**

```rust
pub async fn get_all_statuses(&self) -> Result<Vec<String>>
```

Calls `GET /rest/api/3/status`, returns a flat list of unique status names.

**New helper in `src/cli/issue/list.rs`:**

```rust
fn extract_unique_status_names(issue_types: &[IssueTypeWithStatuses]) -> Vec<String>
```

Extracts and deduplicates status names from the project-scoped response. Uses a `HashSet` for deduplication, returns sorted `Vec<String>`.

**Partial matching:**

Reuse the existing `crate::partial_match::find_match()` function (already used by `issue move` for transitions and `queue view` for queue names). It handles:
- Exact match (case-insensitive) → use it
- Single substring match → use it
- Multiple matches → error listing matches
- No match → error listing valid values

The matched status name replaces the user's input in JQL, ensuring correct casing.

**Validation logic in `handle_list`:**

```rust
if let Some(ref status_input) = status {
    let valid_statuses = if let Some(ref pk) = project_key {
        // Project-scoped: also validates project existence (404 = not found)
        match client.get_project_statuses(pk).await {
            Ok(issue_types) => extract_unique_status_names(&issue_types),
            Err(e) => {
                if let Some(JrError::ApiError { status: 404, .. }) = e.downcast_ref::<JrError>() {
                    bail!("Project \"{}\" not found. ...", pk);
                }
                return Err(e);
            }
        }
    } else {
        client.get_all_statuses().await?
    };

    let status_names: Vec<&str> = valid_statuses.iter().map(|s| s.as_str()).collect();
    let matched = crate::partial_match::find_match(status_input, &status_names)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    // Replace status with matched name for JQL
    status = Some(matched.to_string());
}
```

**Error messages (from `partial_match::find_match`):**

| Scenario | Message |
|----------|---------|
| No match (with project) | `No match for "Nonexistant". Available: Done, In Progress, To Do` |
| No match (no project) | Same format, but from global list |
| Ambiguous | `Ambiguous match for "in": In Progress, In Review` |

These messages come from the existing `partial_match` module. The "Available:" list is already sorted and formatted.

### Combined Flow

When both `--project` and `--status` are set, the validation is efficient:

1. Call `GET /rest/api/3/project/{key}/statuses` — one API call
2. If 404 → project not found error
3. If 200 → extract unique statuses, partial-match `--status`
4. If match fails → error with available statuses
5. If match succeeds → proceed with resolved status name in JQL

When only `--project` is set (no `--status`):
1. Call `GET /rest/api/3/project/{key}` — one API call
2. If 404 → project not found error
3. If 200 → proceed

When only `--status` is set (no `--project`):
1. Call `GET /rest/api/3/status` — one API call
2. Partial-match against global list
3. If match fails → error
4. If match succeeds → proceed

**API call overhead: 0 extra calls when neither flag is set, 1 call when either or both are set.**

### Scope

**Only `issue list` gets validation.** Other commands that accept `--project`:
- `board list/view` and `sprint list/current` — already validated via `resolve_board_id()` which calls `list_boards()` with the project key (returns 400 for invalid projects)
- `issue create` — uses `get_project_issue_types()` which returns 404 for invalid projects
- `project fields` — uses `get_project_statuses()` which returns 404 for invalid projects
- `queue list/view` — uses service desk lookup which fails for invalid projects

`issue list` is the only command that passes `--project` directly into JQL without any API validation.

## What Changes

- New `project_exists()` method in `src/api/jira/projects.rs`
- New `get_all_statuses()` method in `src/api/jira/projects.rs`
- New `extract_unique_status_names()` helper in `src/cli/issue/list.rs`
- `handle_list` in `list.rs` gains validation blocks before JQL construction
- `status` variable becomes mutable (to replace with matched name)

## What Doesn't Change

- JQL construction logic (`build_jql_base_parts`, `build_filter_clauses`)
- Output formatting
- Exit codes for other error scenarios
- Any command other than `issue list`
- The `--status` flag's position in JQL (still uses `status = "name"`)
- Behavior when neither `--project` nor `--status` is set

## Testing

### Unit Tests

- `extract_unique_status_names` — deduplicates statuses across issue types, returns sorted
- `project_exists` — returns true on 200, false on 404, propagates other errors

### Integration Tests

Using wiremock:

1. **Invalid project**: Mock project endpoint returning 404. Assert error contains "not found" and "jr project list".
2. **Valid project proceeds**: Mock project endpoint returning 200. Mock search endpoint. Assert search runs normally.
3. **Invalid status with project**: Mock project statuses returning valid statuses. Assert error from partial_match with available statuses listed.
4. **Valid status partial match**: Mock project statuses with "In Progress", "To Do", "Done". Call `--status "in prog"`. Assert the resolved JQL uses the full "In Progress" name.
5. **Ambiguous status**: Mock statuses with "In Progress" and "In Review". Call `--status "in"`. Assert error lists both matches.
6. **Status without project**: Mock global status endpoint. Call `--status "Nonexistant"` with `--assignee me`. Assert error lists available statuses.
7. **Both project and status invalid**: Mock project statuses returning 404. Assert project error (project validated first).
8. **No validation when flags absent**: Call `--assignee me` (no --project, no --status). Assert no project/status API calls made.

### Existing Tests

- `build_kanban_jql` tests — unchanged
- `build_filter_clauses` tests — unchanged (status string already resolved before reaching these)
- `resolve_effective_limit` tests — unchanged
- All other integration tests — unchanged (they don't use invalid project keys or status names)
