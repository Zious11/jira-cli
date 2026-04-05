# Sprint Issue Management

**Issue:** [#83](https://github.com/Zious11/jira-cli/issues/83)
**Status:** Design approved
**Date:** 2026-04-01

## Problem

`jr sprint` currently supports `list` and `current` — both read-only. There is no way to add
or remove issues from a sprint. Common automation use cases (velocity tracking, sprint planning
scripts, backlog grooming) require this capability and currently fall back to raw `curl` calls
against the Jira Agile REST API.

## Solution

Add two new subcommands: `sprint add` and `sprint remove`.

```
jr sprint add --sprint 100 FOO-1 FOO-2       # add issues to a specific sprint
jr sprint add --current FOO-1 FOO-2           # add issues to the active sprint
jr sprint remove FOO-1 FOO-2                  # move issues to backlog
```

### `src/cli/mod.rs`

**`SprintCommand::Add`**: `--sprint <ID>` flag (required unless `--current` is present),
`--current` flag (conflicts with `--sprint`), variadic positional `issues: Vec<String>`,
optional `--board` for active sprint resolution.

**`SprintCommand::Remove`**: variadic positional `issues: Vec<String>` only.

Sprint ID is a named flag (`--sprint`) rather than positional because clap's derive API cannot
handle an optional positional (`Option<u64>`) followed by a variadic positional (`Vec<String>`)
— the parser processes positionals left-to-right by fixed slot and cannot skip the optional.

### `src/cli/sprint.rs`

**`handle_add`**: If `--current`, resolve board via existing `resolve_board_id` chain
(`--board` → config → auto-discover, scrum-only), then list active sprints to get the sprint
ID. Validate issue count <= 50. Call `add_issues_to_sprint`. Output follows state-change
pattern.

**`handle_remove`**: Validate issue count <= 50. Call `move_issues_to_backlog`. Output follows
state-change pattern.

### `src/api/jira/sprints.rs`

Two new methods:

- `add_issues_to_sprint(sprint_id: u64, issues: &[String]) -> Result<()>` — calls
  `POST /rest/agile/1.0/sprint/{sprintId}/issue` with `{"issues": [...]}`. Uses
  `post_no_content` (204 response).

- `move_issues_to_backlog(issues: &[String]) -> Result<()>` — calls
  `POST /rest/agile/1.0/backlog/issue` with `{"issues": [...]}`. Uses `post_no_content`
  (204 response).

No new types needed. Request bodies are constructed inline with `serde_json::json!`.

## API details

**Add to sprint** — `POST /rest/agile/1.0/sprint/{sprintId}/issue`
- Request: `{"issues": ["FOO-1", "FOO-2"]}`
- Response: 204 No Content
- Max 50 issues per call
- Issues can only be moved to open or active sprints (400 for closed)
- Idempotent: adding an issue already in the sprint returns 204
- Sprint ID (`int64`) is globally unique — no board ID needed

**Move to backlog** — `POST /rest/agile/1.0/backlog/issue`
- Request: `{"issues": ["FOO-1", "FOO-2"]}`
- Response: 204 No Content
- Max 50 issues per call
- Equivalent to removing future and active sprints from a given set of issues
- Idempotent: moving an issue already in backlog returns 204
- No sprint ID needed

## CLI interface

```
jr sprint add --sprint <ID> <ISSUE>...
jr sprint add --current <ISSUE>...
jr sprint add --current --board <ID> <ISSUE>...
jr sprint remove <ISSUE>...
```

### Clap definitions

```rust
/// Add issues to a sprint
Add {
    /// Sprint ID (from `jr sprint list`)
    #[arg(long, required_unless_present = "current")]
    sprint: Option<u64>,
    /// Use the active sprint instead of specifying an ID
    #[arg(long, conflicts_with = "sprint")]
    current: bool,
    /// Issue keys to add (e.g. FOO-1 FOO-2)
    #[arg(required = true, num_args = 1..)]
    issues: Vec<String>,
    /// Board ID (used with --current to resolve the active sprint)
    #[arg(long)]
    board: Option<u64>,
},
/// Remove issues from sprint (moves to backlog)
Remove {
    /// Issue keys to remove (e.g. FOO-1 FOO-2)
    #[arg(required = true, num_args = 1..)]
    issues: Vec<String>,
},
```

## Output

### `sprint add`

**Table:** `Added 3 issue(s) to sprint 100` (via `output::print_success`)

**JSON:**
```json
{
  "sprint_id": 100,
  "issues": ["FOO-1", "FOO-2", "FOO-3"],
  "added": true
}
```

### `sprint remove`

**Table:** `Moved 2 issue(s) to backlog` (via `output::print_success`)

**JSON:**
```json
{
  "issues": ["FOO-1", "FOO-2"],
  "removed": true
}
```

Both commands are idempotent — the API returns 204 whether the issues were already in the
target state or not. The `added`/`removed` field is `true` on any successful API call since
the API does not distinguish "newly moved" from "already there".

## Error handling

| Scenario | Behavior |
|----------|----------|
| >50 issues provided | Client-side error before API call: "Too many issues (got N). Maximum is 50 per operation." |
| Closed sprint (`add`) | API returns 400 — pass through API error message |
| Sprint not found (`add`) | API returns 404 — pass through API error message |
| No active sprint (`--current`) | Existing error path: "No active sprint found for board N." |
| Invalid issue key | API returns 400 — pass through API error message |
| No project configured (`--current`) | Existing error path: "No project configured. Run \"jr init\" or pass --project." |
| Permission denied | API returns 403 — pass through API error message |
| Neither `--sprint` nor `--current` on `add` | Clap enforces `required_unless_present` — automatic error |
| Both `--sprint` and `--current` on `add` | Clap enforces `conflicts_with` — automatic error |

## Edge cases

| Scenario | Behavior |
|----------|----------|
| Single issue | Works: `jr sprint add --sprint 100 FOO-1` |
| 50 issues (max) | Works: all sent in one API call |
| 51 issues | Client-side error before API call |
| Issue already in sprint (`add`) | API returns 204 — reported as success |
| Issue already in backlog (`remove`) | API returns 204 — reported as success |
| `--board` without `--current` (`add`) | `--board` is silently ignored (matches existing `sprint list`/`sprint current` behavior) |
| `--board` on `remove` | Not accepted (not in the variant definition) |

## Handler flow

### `handle_add`

1. If `--current`: resolve board ID via `resolve_board_id(config, client, board, project, true)`,
   then `list_sprints(board_id, Some("active"))`. Error if no active sprint. Use first sprint's ID.
2. Validate `issues.len() <= 50`.
3. Call `client.add_issues_to_sprint(sprint_id, &issues)`.
4. Output success (JSON or table).

### `handle_remove`

1. Validate `issues.len() <= 50`.
2. Call `client.move_issues_to_backlog(&issues)`.
3. Output success (JSON or table).

## Dispatch changes

`src/cli/sprint.rs` match arm and `src/cli/mod.rs` `SprintCommand` enum need two new variants.
The `handle` function in `sprint.rs` needs new match arms. For `Add`, the board resolution
path is only needed when `--current` is used — when `--sprint` is provided, no board/config
is needed.

## Testing

- **Unit tests**: Clap validation — `--sprint` and `--current` conflict, one of them required
  for `add`, variadic issues required
- **Integration tests (wiremock)**: Mock `POST /rest/agile/1.0/sprint/{sprintId}/issue` and
  `POST /rest/agile/1.0/backlog/issue`, verify request body contains correct issue keys,
  assert 204 handling produces correct output
- **CLI smoke tests**: `sprint add --help` shows `--sprint`, `--current`, `--board` flags;
  `sprint remove --help` shows variadic `ISSUE` arg

## What doesn't change

- `sprint list` / `sprint current` — untouched
- Board resolution logic (`resolve_board_id`) — reused as-is
- Existing API client methods — no changes
- Types (`Sprint`, `SprintIssuesResult`) — no changes
- No new dependencies

## Not in scope

- Ranking options (`rankBeforeIssue`, `rankAfterIssue`) — the API supports them but they add
  complexity without clear use cases for CLI automation
- Moving issues between sprints in one command — do `add` to new sprint (API handles the move)
- Batch operations >50 issues — user can loop; auto-chunking adds complexity
