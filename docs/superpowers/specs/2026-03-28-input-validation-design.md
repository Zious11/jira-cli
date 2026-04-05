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
        return Err(JrError::UserError(
            format!(
                "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
                pk
            )
        ).into());
    }
}
```

**Error message:**
```
Project "NONEXISTENT" not found. Run "jr project list" to see available projects.
```

Exit code 64 via `JrError::UserError` — consistent with how `handle_list` already treats bad user input (e.g., `validate_duration` on line 67 uses `JrError::UserError`).

### Status Validation

**Strategy depends on whether `--project` is set:**

| `--project` set? | Endpoint | Why |
|-------------------|----------|-----|
| Yes | `GET /rest/api/3/project/{key}/statuses` | More precise — only statuses valid for that project's workflows. Also validates the project (404 if invalid), combining both checks into one API call. |
| No | `GET /rest/api/3/status` | Global list of all statuses in the instance. Still catches typos. |

**When `--project` IS set:** The project-scoped endpoint returns statuses grouped by issue type with duplicates across types. Extract unique status names into a `Vec<String>` (deduplicate by name). This replaces the separate `project_exists()` call — if this endpoint returns 404, the project doesn't exist.

**When `--project` is NOT set:** The global endpoint returns a flat array of `StatusDetails` with `name` fields. No deduplication needed.

**New API method in `src/api/jira/statuses.rs`** (new file — `GET /rest/api/3/status` is a global endpoint, not project-scoped, so it gets its own file following the one-file-per-resource convention):

```rust
pub async fn get_all_statuses(&self) -> Result<Vec<String>>
```

Calls `GET /rest/api/3/status` (not paginated — returns all statuses for active workflows), extracts unique status names, returns as a flat `Vec<String>`.

**New helper in `src/cli/issue/list.rs`:**

```rust
fn extract_unique_status_names(issue_types: &[IssueTypeWithStatuses]) -> Vec<String>
```

Extracts and deduplicates status names from the project-scoped response. Uses a `HashSet` for deduplication, returns sorted `Vec<String>`.

**Partial matching:**

Reuse the existing `crate::partial_match::partial_match()` function (already used by `issue move` for transitions and `queue view` for queue names). It takes `(input: &str, candidates: &[String])` and returns a `MatchResult` enum:
- `MatchResult::Exact(String)` — exact or single substring match
- `MatchResult::Ambiguous(Vec<String>)` — multiple matches
- `MatchResult::None(Vec<String>)` — no match (contains all candidates for error messages)

The caller constructs error messages from the `MatchResult` variants, following the same pattern used in `workflow.rs` and `helpers.rs`.

**Validation placement in `handle_list`:**

The validation must run BEFORE `build_filter_clauses()` is called (currently line 98), because the resolved status name needs to reach `build_filter_clauses`. The validation also requires `project_key`, which is currently resolved at line 108. Reorder: move `let project_key = config.project_key(project_override);` up from line 108 to after the team clause (line 95), then insert validation, then call `build_filter_clauses`. Use a separate `resolved_status` variable rather than mutating the destructured `status`:

```rust
// After team_clause (line 95), before build_filter_clauses (line 98):

// Validate --project exists
if let Some(ref pk) = project_key {
    // Skip if --status is set (project will be validated via statuses endpoint below)
    if status.is_none() && !client.project_exists(pk).await? {
        return Err(JrError::UserError(format!(
            "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
            pk
        )).into());
    }
}

// Validate --status and resolve to exact name
let resolved_status: Option<String> = if let Some(ref status_input) = status {
    let valid_statuses = if let Some(ref pk) = project_key {
        // Project-scoped: also validates project existence (404 = not found)
        match client.get_project_statuses(pk).await {
            Ok(issue_types) => extract_unique_status_names(&issue_types),
            Err(e) => {
                if let Some(JrError::ApiError { status: 404, .. }) = e.downcast_ref::<JrError>() {
                    return Err(JrError::UserError(format!(
                        "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
                        pk
                    )).into());
                }
                return Err(e);
            }
        }
    } else {
        client.get_all_statuses().await?
    };

    match crate::partial_match::partial_match(status_input, &valid_statuses) {
        crate::partial_match::MatchResult::Exact(name) => Some(name),
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            return Err(JrError::UserError(format!(
                "Ambiguous status \"{}\". Matches: {}",
                status_input,
                matches.join(", ")
            )).into());
        }
        crate::partial_match::MatchResult::None(all) => {
            let available = all.join(", ");
            let scope = if project_key.is_some() {
                format!(" for project {}", project_key.as_ref().unwrap())
            } else {
                String::new()
            };
            return Err(JrError::UserError(format!(
                "No status matching \"{}\"{scope}. Available: {available}",
                status_input,
            )).into());
        }
    }
} else {
    None
};

// Use resolved_status in build_filter_clauses instead of status
let filter_parts = build_filter_clauses(
    assignee_jql.as_deref(),
    reporter_jql.as_deref(),
    resolved_status.as_deref(),  // <-- resolved name, not raw input
    team_clause.as_deref(),
    recent.as_deref(),
    open,
);
```

This approach:
- Uses a new `resolved_status` variable instead of mutating the destructured `status`
- Moves `build_filter_clauses` after validation (passes `resolved_status` instead of `status`)
- Uses `JrError::UserError` for exit code 64
- Constructs error messages from `MatchResult` variants (matching the pattern in `workflow.rs`)
- Passes `&[String]` to `partial_match()` (correct type)

**Error messages (constructed by the caller from `MatchResult` variants):**

| Scenario | Message |
|----------|---------|
| No match (with project) | `No status matching "Nonexistant" for project PROJ. Available: Done, In Progress, To Do` |
| No match (no project) | `No status matching "Nonexistant". Available: Done, In Progress, To Do, ...` |
| Ambiguous | `Ambiguous status "in". Matches: In Progress, In Review` |

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
- New `get_all_statuses()` method in `src/api/jira/statuses.rs` (new file)
- New `extract_unique_status_names()` helper in `src/cli/issue/list.rs`
- `handle_list` in `list.rs` gains validation blocks before `build_filter_clauses`
- New `resolved_status` variable replaces raw `status` in `build_filter_clauses` call
- `project_key` resolution moves up from line 108 to before validation block
- `build_filter_clauses` call moves after validation (receives resolved status name)

## What Doesn't Change

- JQL construction logic (`build_jql_base_parts`, `build_filter_clauses` — function unchanged, only call site moves after validation)
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
