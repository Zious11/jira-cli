# Fix Kanban JQL `AND ORDER BY` Bug — Design Spec

**Issue:** #31

**Goal:** Fix invalid JQL syntax in `board view` kanban path where `ORDER BY rank ASC` is joined with predicates using `AND`, producing `AND ORDER BY` which returns HTTP 400 from Jira Cloud.

## Problem

In `src/cli/board.rs`, the kanban branch (find by the `"ORDER BY rank ASC"` push into `jql_parts`) builds JQL by pushing `ORDER BY` as a predicate and joining with `" AND "`:

```rust
jql_parts.push("statusCategory != Done".into());
jql_parts.push("ORDER BY rank ASC".into());
let jql = jql_parts.join(" AND ");
```

This produces:

```
project = "FOO" AND statusCategory != Done AND ORDER BY rank ASC
```

`AND ORDER BY` is invalid JQL. Jira Cloud's `/rest/api/3/search` returns 400 Bad Request.

The scrum branch (`board_type == "scrum"`) is **not affected** — it calls `client.get_sprint_issues()` which uses the Agile API directly rather than building JQL.

## Expected

```
project = "FOO" AND statusCategory != Done ORDER BY rank ASC
```

`ORDER BY` is a separate clause — not a predicate — and must be appended after all WHERE predicates without `AND`.

## Fix

Remove `ORDER BY rank ASC` from `jql_parts`. Join only filter predicates with `" AND "`. Append `ORDER BY rank ASC` separately via `format!()`.

This matches the existing pattern in `src/cli/issue/list.rs`:

```rust
let where_clause = all_parts.join(" AND ");
let effective_jql = format!("{where_clause} ORDER BY {order_by}");
```

Note: `jql_parts` is never empty in this path — `"statusCategory != Done"` is pushed unconditionally, so no empty-string guard is needed.

### Code Change

**`src/cli/board.rs` — kanban JQL construction block:**

Before:
```rust
let mut jql_parts: Vec<String> = Vec::new();
if let Some(ref pk) = project_key {
    jql_parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
}
jql_parts.push("statusCategory != Done".into());
jql_parts.push("ORDER BY rank ASC".into());
let jql = jql_parts.join(" AND ");
```

After:
```rust
let mut jql_parts: Vec<String> = Vec::new();
if let Some(ref pk) = project_key {
    jql_parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
}
jql_parts.push("statusCategory != Done".into());
let where_clause = jql_parts.join(" AND ");
let jql = format!("{where_clause} ORDER BY rank ASC");
```

Note: In the final implementation, this logic was extracted into a helper function `fn build_kanban_jql(project_key: Option<&str>) -> String` for testability, and `handle_view` calls that helper. The inline example above illustrates the core JQL construction logic.

## Files Changed

| File | Change |
|------|--------|
| `src/cli/board.rs` | Fix JQL construction: remove ORDER BY from predicates, append separately |

## Testing

Extract the kanban JQL construction into a testable helper function `build_kanban_jql(project_key: Option<&str>) -> String`. Add unit tests asserting:

- With project: `project = "FOO" AND statusCategory != Done ORDER BY rank ASC`
- Without project: `statusCategory != Done ORDER BY rank ASC`
- Project key with special characters is escaped correctly

Live verification: `jr board view --board <kanban-board-id>` returns issues without 400 error.

## Backward Compatibility

No breaking changes. The fix produces correct JQL where previously it produced invalid JQL (400 error). Any workflow that was hitting this code path was already broken.
