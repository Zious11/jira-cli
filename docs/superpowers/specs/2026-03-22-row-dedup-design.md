# Issue Row Building Deduplication

## Overview

Extract the duplicated table row building pattern into a shared `format_issue_row()` helper. The same `vec![issue.key.clone(), issue.fields.issue_type...unwrap_or_default(), ...]` code appears 3 times across `issue.rs` and `sprint.rs`, with the only variation being an optional Points column.

## Problem

Three call sites build nearly identical horizontal table rows:

| Location | File | Points column |
|----------|------|---------------|
| `format_issue_rows_public()` | `src/cli/issue.rs:13-47` | No |
| `handle_list` with `--points` | `src/cli/issue.rs:200-235` | Yes |
| `handle_current` with points | `src/cli/sprint.rs:155-190` | Yes |

Each is ~20 lines of the same field extraction pattern. Adding a new column (e.g., labels) would require updating all 3.

Note: `handle_view` (issue.rs:305) uses a different vertical key-value format and is NOT part of this deduplication.

## Solution

Two new `pub` functions in `src/cli/issue.rs`:

```rust
/// Build a table row for an issue, optionally including story points.
pub fn format_issue_row(issue: &Issue, sp_field_id: Option<&str>) -> Vec<String> {
    let col_count = if sp_field_id.is_some() { 7 } else { 6 };
    let mut row = Vec::with_capacity(col_count);
    row.push(issue.key.clone());
    row.push(
        issue.fields.issue_type.as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_default(),
    );
    row.push(
        issue.fields.status.as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_default(),
    );
    row.push(
        issue.fields.priority.as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default(),
    );
    if let Some(field_id) = sp_field_id {
        row.push(
            issue.fields.story_points(field_id)
                .map(format_points)
                .unwrap_or_else(|| "-".into()),
        );
    }
    row.push(
        issue.fields.assignee.as_ref()
            .map(|a| a.display_name.clone())
            .unwrap_or_else(|| "Unassigned".into()),
    );
    row.push(issue.fields.summary.clone());
    row
}

/// Headers matching `format_issue_row` output.
pub fn issue_table_headers(show_points: bool) -> Vec<&'static str> {
    if show_points {
        vec!["Key", "Type", "Status", "Priority", "Points", "Assignee", "Summary"]
    } else {
        vec!["Key", "Type", "Status", "Priority", "Assignee", "Summary"]
    }
}
```

**Sync guarantee:** Both `format_issue_row` and `issue_table_headers` branch on the same `sp_field_id` / `show_points` value at each call site, ensuring headers always match row content.

## Callers Updated

### 1. `format_issue_rows_public()` (issue.rs)

```rust
pub fn format_issue_rows_public(issues: &[Issue]) -> Vec<Vec<String>> {
    issues.iter().map(|issue| format_issue_row(issue, None)).collect()
}
```

Becomes a 1-line thin wrapper. Kept for backward compatibility since it's `pub` and used by `board.rs` and `sprint.rs` (no-points fallback path).

### 2. `handle_list` (issue.rs)

Replace the 30-line inline block with:

```rust
let rows: Vec<Vec<String>> = issues.iter()
    .map(|issue| format_issue_row(issue, sp_field_id))
    .collect();
output::print_output(output_format, &issue_table_headers(sp_field_id.is_some()), &rows, &issues)?;
```

Removes the `if show_points && sp_field_id.is_some()` / `else` branch entirely — `format_issue_row` handles both cases.

### 3. `handle_current` (sprint.rs)

Replace the 30-line inline block with:

```rust
let rows: Vec<Vec<String>> = issues.iter()
    .map(|issue| super::issue::format_issue_row(issue, sp_field_id))
    .collect();
output::print_output(
    output_format,
    &super::issue::issue_table_headers(sp_field_id.is_some()),
    &rows,
    &issues,
)?;
```

Removes the `if let Some(field_id) = sp_field_id` / `else` branch entirely.

## Scope

- **Files modified:** `src/cli/issue.rs`, `src/cli/sprint.rs`
- **Pure refactor:** no behavior changes
- **All existing tests pass unchanged**
- **Net code reduction:** ~60 lines removed, ~25 added

## Testing

- Existing unit tests for `format_points` still pass
- Existing integration tests (`test_search_issues`, `test_get_issue`) still pass
- Existing sprint summary unit tests still pass
- No new tests needed — helper is exercised through existing callers and tests

## Files Touched

| File | Change |
|------|--------|
| `src/cli/issue.rs` | Add `format_issue_row()`, `issue_table_headers()`. Simplify `format_issue_rows_public()` and `handle_list`. |
| `src/cli/sprint.rs` | Replace inline row building in `handle_current` with `format_issue_row()` call. |
