# Unbounded JQL Guard

## Overview

Add early validation to `issue list` (and audit other commands) to reject unbounded JQL queries before they hit the Jira API. Currently, running `jr issue list` with no project, board, status, or team produces a raw Jira API error that is unhelpful for both humans and AI agents.

Closes: GitHub issue #16.

## Problem

`build_fallback_jql(None, None, None)` produces `" ORDER BY updated DESC"` — an unbounded query. Jira rejects it with:

```json
{"code":1,"error":"API error (400): Unbounded JQL queries are not allowed here. Please add a search restriction to your query."}
```

This violates the CLAUDE.md convention: "Errors: Always suggest what to do next."

## Design

### Change 1: `build_fallback_jql` returns `Result<String>`

Change the function signature from `fn build_fallback_jql(...) -> String` to `fn build_fallback_jql(...) -> Result<String>` and bail when all three filters (`project`, `status`, `team`) are `None`.

Use `JrError::UserError` so the error maps to exit code 64 (EX_USAGE) through the existing `main.rs` error handling. A plain `anyhow::bail!("...")` would fall through to exit code 1 since `downcast_ref::<JrError>()` wouldn't match.

Error message:

```
No project or filters specified. Use --project KEY, --status STATUS, or --team NAME. You can also set a default project in .jr.toml or run "jr init".
```

### Change 2: Update callers in `handle_list`

Three call sites in `handle_list` (lines 72, 96, 103 of `list.rs`) append `?` to propagate the error:

```rust
build_fallback_jql(
    project_key.as_deref(),
    status.as_deref(),
    resolved_team.as_ref(),
)?
```

### Change 3: `board view` kanban warning

The kanban path in `board.rs` (line 56-63) is bounded by `statusCategory != Done` so it won't produce an API error. However, with no project scope it may return a large result set across all projects. After `let project_key = config.project_key(None);` at line 56, check `if project_key.is_none()` and emit a stderr warning:

```rust
if project_key.is_none() {
    eprintln!("warning: no project configured for board. Showing issues across all projects. Set project in .jr.toml to scope results.");
}
```

This is a warning (`eprintln!`), not an error — the command still succeeds. The warning only prints when no project is configured; it does not fire when a project is set.

### Change 4: Update tests

- **Replace** `fallback_jql_no_filters_still_has_order_by`: This test asserts the old unbounded behavior (`assert_eq!(jql, " ORDER BY updated DESC")`). Remove it and replace with `fallback_jql_errors_when_no_filters` which asserts the function returns an error containing actionable guidance (mentions `--project`, `.jr.toml`, and `jr init`).
- **Existing tests adapt signatures**: `fallback_jql_with_status_only`, `fallback_jql_with_all_filters`, etc. pass at least one filter. They now call `.unwrap()` on the `Result` since the function returns `Result<String>` instead of `String`. The assertions remain unchanged.

## Files Touched

| File | Change |
|------|--------|
| `src/cli/issue/list.rs` | `build_fallback_jql` returns `Result<String>`, callers add `?`, test updates |
| `src/cli/board.rs` | Add stderr warning in kanban path when no project configured |

## What Doesn't Change

- `issue create` — already has project key validation with actionable error
- `sprint *` — requires `board_id` via config, already guarded
- `worklog *` — requires issue key as clap argument, already guarded
- `project fields` — uses `.ok_or_else()`, already guarded
- User-provided `--jql` — bypasses `build_fallback_jql` entirely
- `--output json` error format — already handled by `main.rs` (wraps error in `{"error": "...", "code": 64}`)

## Audit Results

| Command | Has guard? | Action |
|---------|-----------|--------|
| `issue list` | No | Fix (this spec) |
| `issue create` | Yes (flag→config→prompt→error) | None |
| `board view` | Partial (requires board_id, kanban unbounded on project) | Warning added |
| `sprint *` | Yes (requires board_id) | None |
| `worklog *` | Yes (clap requires key) | None |
| `project fields` | Yes (`.ok_or_else()`) | None |

## Testing

No new integration tests needed. The fix is validated by:
1. Unit tests on `build_fallback_jql` (error case + existing success cases with `.unwrap()`)
2. Existing integration tests in `tests/issue_commands.rs` are unaffected — they test API-layer behavior, not JQL construction
3. Review `tests/` for any integration tests that exercise `issue list` with no filters — if any exist, they should now expect a CLI validation error rather than an API error
