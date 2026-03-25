# Design: `--open` Flag for `jr issue list`

**Issue:** #45 — Add 'jr my issues' shortcut for daily workflow
**Date:** 2026-03-25
**Status:** Draft

## Problem

Issue #45 requested `jr my issues` as a shortcut for the daily "what's on my plate?" query. With PRs #50 (default result limit) and #51 (common filter flags), the gap is smaller than originally scoped:

```bash
jr issue list --assignee me              # works cross-project today
jr issue list --reporter me --recent 7d  # composable
```

The remaining gap: there's no way to exclude Done/Closed issues without raw JQL. Users must write `--jql "statusCategory != Done"` to filter out completed work.

## Decision: `--open` Flag, Not `jr my` Subcommand

**Rejected: `jr my issues` subcommand.** Adding a second path to list issues doubles the surface area for AI agents (primary consumer) without adding composability. Agents don't benefit from shorter syntax — they generate commands programmatically.

**Chosen: `--open` boolean flag on `jr issue list`.** This follows the existing composable-flags pattern, adds zero new concepts, and slots into the established JQL composition pipeline.

Precedent: GitHub CLI uses `--state {open|closed|all}` on `gh issue list` (open is the default). Our `--open` is simpler because Jira's status model differs — we already have `--status` for exact status matching, so `--open` covers the broad "not done" filter.

## Design

### Flag Definition

```
--open    Show only issues in open status categories (excludes Done)
```

- Type: `bool`
- Conflicts with: `--status` (mutual exclusion — use `--status` for specific status, `--open` for broad "not done")
- Composes with: `--assignee`, `--reporter`, `--recent`, `--team`, `--jql`, `--project`

### JQL Clause

`--open` appends a single clause to the filter:

```
statusCategory != "Done"
```

This uses Jira's built-in `statusCategory` field (not a custom field — same on every instance). The three valid status categories are "To Do", "In Progress", and "Done". All statuses mapped to the Done category (Done, Closed, Resolved, etc.) are excluded.

### Composition

`--open` slots into the existing `build_filter_clauses()` function as one more optional clause:

```
assignee = currentUser()           <-- --assignee me
AND reporter = currentUser()       <-- --reporter me
AND statusCategory != "Done"       <-- --open
AND {team_field} = "{uuid}"        <-- --team
AND created >= -7d                 <-- --recent
```

All AND together, same pipeline as today. No changes to JQL builder, ORDER BY handling, or board detection logic.

Edge case with `--jql`: `--open` composes freely. If user's `--jql` already has a statusCategory clause, the AND produces a redundant-but-valid query. No special handling needed (Jira evaluates clauses independently).

### Conflicts

`--open` conflicts with `--status`. Rationale:
- `--status "In Progress"` is already in the open category — `--open` would be redundant
- `--status "Done" --open` is contradictory (zero results)
- Clean separation: `--status` for specific, `--open` for broad

## Files Changed

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add `--open` bool arg with `conflicts_with = "status"` to `IssueCommand::List` |
| `src/cli/issue/list.rs` | Destructure `open`, pass to `build_filter_clauses()`, add clause |
| `README.md` | Add `--open` to command table |

No new runtime/code modules or API calls; only updates to existing CLI code and documentation.

Additionally, the unbounded query guard error message (`list.rs:151`) must be updated to include `--open` in the list of available flags.

## Testing

Unit tests in `src/cli/issue/list.rs`:
- `build_jql_parts_open` — `--open` alone produces `statusCategory != "Done"`
- `build_jql_parts_open_with_assignee` — composes correctly
- `build_jql_parts_all_filters_with_open` — all flags together

Existing tests unchanged.

## Outcome

After this change, the daily workflow from issue #45 becomes:

```bash
jr issue list --assignee me --open    # "what's on my plate?" — cross-project, excludes done
```

This closes issue #45 without a new subcommand, new config, or new concepts for AI agents to learn.
