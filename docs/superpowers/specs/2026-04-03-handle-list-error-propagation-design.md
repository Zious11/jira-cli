# Design: Propagate board/sprint API errors in handle_list

**Issue:** [#32](https://github.com/Zious11/jira-cli/issues/32)
**Date:** 2026-04-03
**Status:** Draft

## Problem

In `src/cli/issue/list.rs`, `handle_list` silently swallows API errors from two board-related calls when no `--jql` is provided and a `board_id` is configured in `.jr.toml`:

1. **`get_board_config` error (line 255):** `Err(_) =>` discards the error and falls back to generic project-scoped JQL. Auth failures, network timeouts, and 404s are all hidden.
2. **`list_sprints` catch-all (line 234):** `_ =>` treats API errors identically to "no active sprint found", falling back to project-scoped JQL.

Both paths may ultimately hit the "No project or filters specified" guard if no other filters are set, producing a misleading `UserError` that masks the real cause.

## Design

Propagate all errors from `get_board_config` and `list_sprints` with contextual messages and actionable recovery suggestions. No silent fallbacks.

### Change 1: `get_board_config` error handling

Replace the `Err(_) =>` catch-all with error propagation. Distinguish 404 (ambiguous in Jira's API — covers both "deleted" and "no permission" per [JRACLOUD-97947](https://jira.atlassian.com/browse/JRACLOUD-97947)) from other errors:

- **404:** `JrError::UserError` — "Board {id} not found or not accessible. Verify the board exists and you have permission, or remove board_id from .jr.toml. Use --jql to query directly."
- **Other errors:** Propagate with context via `anyhow::Context` — "Failed to fetch config for board {id}. Remove board_id from .jr.toml or use --jql to query directly."

404 uses `UserError` (exit code 64) because it indicates a configuration problem. Other errors propagate as-is (exit code 1) since they may be transient.

### Change 2: `list_sprints` error handling

Replace the `_ =>` catch-all with explicit match arms:

- **`Ok(sprints) if !sprints.is_empty()`** → Use sprint JQL (unchanged).
- **`Ok(_)`** → No active sprint. Fall back to project-scoped JQL (unchanged — this is the correct behavior for a scrum board between sprints).
- **`Err(e)`** → Propagate with context: "Failed to list sprints for board {id}: {error}. Use --jql to query directly."

### What stays the same

- No active sprint on a scrum board → falls back to project-scoped JQL with `updated DESC` ordering. This is legitimate, not an error.
- Kanban board path → unchanged (filters `statusCategory != Done`, orders by `rank ASC`).
- No `board_id` configured → unchanged (goes straight to project-scoped JQL).
- All JQL composition logic downstream → untouched.
- `--jql` provided → board_id path is skipped entirely, unaffected.

### API behavior note

Jira's board agile API (`/rest/agile/1.0/board/{id}/configuration` and `/rest/agile/1.0/board/{id}/sprint`) returns 404 for both "board does not exist" and "user lacks permission." There is no 403 response. This is a known Atlassian limitation. Error messages must account for this ambiguity.

## Error message examples

```
# Board deleted or no access
Error: Board 42 not found or not accessible. Verify the board exists and you
have permission, or remove board_id from .jr.toml. Use --jql to query directly.

# Network/server error on board config
Error: Failed to fetch config for board 42. Remove board_id from .jr.toml or
use --jql to query directly.

Caused by: connection timed out

# Sprint list failure
Error: Failed to list sprints for board 42. Use --jql to query directly.

Caused by: 500 Internal Server Error
```

## Edge cases

- **Board type changed after config written:** If a board was scrum when `board_id` was configured but later changed to kanban, `get_board_config` succeeds and returns the new type. The code already reads `board_type` from the response and branches on it, so it will correctly take the kanban path. No sprint call is made. This is handled without changes.
- **Board in trash:** Returns 404, handled by the board config 404 path.
- **Permissions revoked after config written:** Returns 404 (per JRACLOUD-97947), handled by the board config 404 path.

## Testing

1. **Unit test: board config 404** — Mock `get_board_config` returning `JrError::ApiError { status: 404, .. }`. Verify error message contains board ID and suggests removing `board_id` from config.
2. **Unit test: board config other error** — Mock `get_board_config` returning a generic error. Verify error propagates with "Failed to fetch config for board" context.
3. **Unit test: list_sprints error** — Mock `list_sprints` returning an error on a scrum board. Verify error propagates with context and suggests `--jql`.
4. **Unit test: list_sprints empty (no active sprint)** — Mock scrum board with empty sprint list. Verify fallback to project-scoped JQL (existing behavior preserved).
5. **Existing tests** — All current tests continue to pass since no happy-path behavior changes.

## Scope

- **Files modified:** `src/cli/issue/list.rs` (the two match arms, ~20 lines changed)
- **No new dependencies.**
- **No new CLI flags or config options.**
- **No changes to API client layer.**
