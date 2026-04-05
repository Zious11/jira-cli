# Design: Fix `--jql` + `--project` scope composition

**Issue:** #54 — Bug: --jql filter overrides --project scope, returning cross-project results
**Date:** 2026-03-25
**Status:** Draft

## Problem

When combining `--jql` with `--project`, the JQL filter overrides the project scope entirely instead of composing with it. Users get cross-project results despite specifying `--project`.

**Root cause:** In `src/cli/issue/list.rs`, the `if let Some(raw_jql) = jql` branch (line 90) builds base parts from only the raw JQL, skipping the project-scope logic that lives in the `else` branch. The project key is never added when `--jql` is present.

**Current behavior:**
```
jr issue list --project PROJ --jql "priority = Highest"
→ JQL: priority = Highest ORDER BY updated DESC    (project missing)
```

**Expected behavior:**
```
jr issue list --project PROJ --jql "priority = Highest"
→ JQL: project = "PROJ" AND priority = Highest ORDER BY updated DESC
```

## Design Principle

Per the principle of least surprise, structured flags (`--project`, `--status`, etc.) compose additively with raw queries (`--jql`) using AND logic. This matches the behavior of `gh`, `kubectl`, and `aws cli`. The `--jql` flag provides additional filtering, not a complete override.

## Fix

Refactor the `handle_list` function in `src/cli/issue/list.rs` to resolve the project key before the `if/else` branch, so it's available regardless of whether `--jql` is provided.

### Current flow (buggy)

```
if --jql provided:
    base_parts = [user_jql]          # project key never considered
    order_by = "updated DESC"
else:
    base_parts = [project clause]    # project key only here
    order_by = board-aware logic
```

### Fixed flow

```
project_key = resolve project from --project flag or config

if --jql provided:
    stripped = strip_order_by(user_jql)
    base_parts = []
    if project_key exists:
        base_parts.push(project clause)
    if stripped is non-empty:
        base_parts.push(stripped)
    order_by = "updated DESC"
else:
    # uses same hoisted project_key, otherwise unchanged board-aware logic
```

The `else` branch is refactored to reference the hoisted `project_key` variable instead of calling `config.project_key()` inline. The logic is functionally identical.

### Generated JQL after fix

| Flags | Generated JQL |
|-------|---------------|
| `--project PROJ --jql "priority = Highest"` | `project = "PROJ" AND priority = Highest ORDER BY updated DESC` |
| `--project PROJ --jql "priority = Highest" --status "In Progress"` | `project = "PROJ" AND priority = Highest AND status = "In Progress" ORDER BY updated DESC` |
| `--jql "priority = Highest"` (no project anywhere) | `priority = Highest ORDER BY updated DESC` |
| `--project PROJ` (no jql) | Unchanged — board-aware logic applies |
| `--project PROJ --jql "project = OTHER AND type = Bug"` | `project = "PROJ" AND project = OTHER AND type = Bug ORDER BY ...` — contradictory project clauses are the user's responsibility, consistent with how `--status` + JQL `status =` behaves |
| `--project PROJ --jql "ORDER BY created DESC"` | `project = "PROJ" ORDER BY updated DESC` — empty JQL after stripping ORDER BY is skipped (requires `strip_order_by` fix, see below) |
| No flags | Unchanged — unbounded query guard triggers |

## Scope

### `strip_order_by` fix

The current `strip_order_by` in `src/jql.rs` searches for `" ORDER BY"` (with a leading space). This misses JQL that starts with `ORDER BY` at position 0 (e.g., `"ORDER BY created DESC"`). Fix: also match `ORDER BY` at the start of the string. Add a unit test for this case.

### What changes

| File | Change |
|------|--------|
| `src/cli/issue/list.rs` | Refactor `handle_list` to resolve project key before the `if/else`, include it in base parts when `--jql` is present; skip empty stripped JQL |
| `src/cli/issue/list.rs` | Add unit tests for the new composition behavior |
| `src/jql.rs` | Fix `strip_order_by` to handle ORDER BY at position 0; add unit test |
| `tests/issue_commands.rs` | Add integration test verifying `--project` + `--jql` sends correct composed JQL |

### What doesn't change

- Board-aware logic (scrum sprint detection, kanban `statusCategory != Done`) — only applies when `--jql` is absent
- All existing filter flags (`--assignee`, `--reporter`, `--status`, `--team`, `--recent`, `--open`) — already compose correctly via `build_filter_clauses`
- The `build_filter_clauses` function — untouched

## Testing

### Unit tests (in `src/cli/issue/list.rs`)

To make the JQL composition logic directly testable, extract the base-parts-building logic into a pure function that can be unit tested without async or wiremock. Tests:

- `jql_with_project_composes` — both `--jql` and project key present: project clause prepended
- `jql_without_project_unchanged` — `--jql` with no project key: JQL passes through unchanged, does not trigger unbounded guard (non-empty parts)
- `jql_order_by_only_with_project` — `--jql "ORDER BY created"` with project: stripped JQL is empty, only project clause remains
- `jql_order_by_only_no_project` — `--jql "ORDER BY created"` with no project: stripped JQL is empty, no base parts (may trigger unbounded guard depending on other filters)

### Integration test (in `tests/issue_commands.rs`)

- `test_search_issues_jql_with_project` — call `client.search_issues()` with composed JQL containing both project clause and user JQL via wiremock, verify the POST body sent to `/rest/api/3/search/jql` contains the expected composed query
