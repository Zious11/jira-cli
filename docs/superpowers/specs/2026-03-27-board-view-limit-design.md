# Board View --limit Flag — Design Spec

## Problem

`jr board view` returns unbounded output with no way to control output size. A kanban board returned **1.8MB** of table data during testing. Every other list-style command (`issue list`, `assets search`, `assets tickets`, `queue view`) supports `--limit` — `board view` is the only outlier.

This is critical for AI agent consumption: an AI calling `board view` will blow its context window with no way to cap results.

**Issue:** #69

## Solution

Add `--limit <N>` and `--all` flags to `board view`, following the established `issue list` pattern. Default limit: 30 (same as `issue list`). Both the scrum and kanban paths respect the limit with efficient early-stop pagination.

Split out `sprint current` (same unbounded problem) to #72 to keep scope tight.

### CLI Interface

```
jr board view [--board <ID>] [--limit <N>] [--all]
```

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--board` | `Option<u64>` | from config | Board ID override |
| `--limit` | `Option<u32>` | 30 | Maximum issues to return |
| `--all` | `bool` | false | Fetch all results (no limit) |

`--limit` and `--all` conflict (clap `conflicts_with`). This is bidirectional — only needs to be declared on one arg, matching `issue list`'s existing pattern.

### Shared Helper Extraction

`resolve_effective_limit()` and `DEFAULT_LIMIT` currently live in `src/cli/issue/list.rs` as private items. Since `board view` needs the same logic, extract both to `src/cli/mod.rs` as `pub(crate)`:

```rust
pub(crate) const DEFAULT_LIMIT: u32 = 30;

pub(crate) fn resolve_effective_limit(limit: Option<u32>, all: bool) -> Option<u32> {
    if all {
        None
    } else {
        Some(limit.unwrap_or(DEFAULT_LIMIT))
    }
}
```

`issue/list.rs` imports from `cli/mod.rs` instead of defining its own copy.

### API Layer: `get_sprint_issues()` Changes

The current signature:

```rust
pub async fn get_sprint_issues(
    &self,
    sprint_id: u64,
    jql: Option<&str>,
    extra_fields: &[&str],
) -> Result<Vec<Issue>>
```

Changes to:

```rust
pub async fn get_sprint_issues(
    &self,
    sprint_id: u64,
    jql: Option<&str>,
    limit: Option<u32>,
    extra_fields: &[&str],
) -> Result<SprintIssuesResult>
```

New return type, defined in `src/api/jira/sprints.rs` (alongside `get_sprint_issues()`):

```rust
pub struct SprintIssuesResult {
    pub issues: Vec<Issue>,
    pub has_more: bool,
}
```

**Why not reuse `SearchResult`?** Different pagination models — `SearchResult` comes from cursor-based search, `SprintIssuesResult` from offset-based Agile API. Separate types keep semantics clear.

**Why `has_more: bool` instead of `total: u32`?** The Jira Agile API's `total` field is unreliable — it can change mid-pagination due to concurrent modifications. The `has_more` signal (derived from `OffsetPage.has_more()`) is the reliable pagination indicator.

#### Early-Stop Pagination Logic

The pagination loop adds limit-aware early-stop. A mutable `result_has_more` tracks whether more results exist beyond what was collected:

```rust
let mut result_has_more = false;

// Inside the loop, after extending all_issues from page:
if let Some(max) = limit {
    if all_issues.len() >= max as usize {
        result_has_more = all_issues.len() > max as usize || page_has_more;
        all_issues.truncate(max as usize);
        break;
    }
}
```

This stops fetching pages as soon as enough issues are collected. For the 1.8MB case, `--limit 30` fetches a single page of 50 instead of all pages.

### Kanban Path

Already uses `search_issues()` which accepts `limit: Option<u32>`. Pass the effective limit through and capture `has_more` for the truncation hint:

```rust
// Before (unbounded):
client.search_issues(&jql, None, &[]).await?.issues

// After (limit-aware):
let result = client.search_issues(&jql, effective_limit, &[]).await?;
let has_more = result.has_more;
let issues = result.issues;
```

### Handler Changes in `board.rs`

`handle_view` gains `limit` and `all` parameters from the destructured `BoardCommand::View` enum. The flow:

1. Resolve effective limit via `resolve_effective_limit(limit, all)`
2. Fetch issues (scrum or kanban path, both limit-aware)
3. Format and print (unchanged)
4. If truncated, show hint on stderr

### Truncation Hint

When output is truncated, show a hint on stderr matching `issue list`'s pattern:

**Kanban path** (has `approximate_count()`):
```
Showing 30 of ~487 results. Use --limit or --all to see more.
```

**Scrum path** (no reliable total count):
```
Showing 30 results. Use --limit or --all to see more.
```

Both match `issue list`'s existing behavior — the kanban variant uses `approximate_count()` for the total, and the scrum variant uses the simpler message (same as `issue list`'s fallback when `approximate_count()` fails).

### Call Site Updates

All existing callers of `get_sprint_issues()` must pass the new `limit` parameter:

| File | Current Call | New Call | Notes |
|------|-------------|---------|-------|
| `src/cli/board.rs` (scrum path) | `client.get_sprint_issues(sprint.id, None, &[]).await?` returns `Vec<Issue>` | `client.get_sprint_issues(sprint.id, None, effective_limit, &[]).await?` returns `SprintIssuesResult` | Destructure: `let result = ...; let has_more = result.has_more; let issues = result.issues;` |
| `src/cli/sprint.rs` (`handle_current`) | `client.get_sprint_issues(sprint.id, None, &extra).await?` returns `Vec<Issue>` | `client.get_sprint_issues(sprint.id, None, None, &extra).await?` returns `SprintIssuesResult` | Extract `.issues`: `let result = ...; let issues = result.issues;` — no behavior change, `has_more` ignored |
| `src/cli/issue/list.rs` (scrum branch) | Not a caller of `get_sprint_issues()` — uses JQL with `sprint = {id}` | No change | — |

## What Changes

- `BoardCommand::View` gains `--limit` and `--all` flags
- `resolve_effective_limit()` and `DEFAULT_LIMIT` move to `cli/mod.rs`
- `get_sprint_issues()` gains `limit` param, returns `SprintIssuesResult`
- `handle_view` in `board.rs` passes limit to both paths, shows truncation hint
- `sprint.rs` passes `None` for the new limit param (no behavior change)

## What Doesn't Change

- Error messages or exit codes
- Board list, sprint list, or any other command
- The kanban JQL generation (`build_kanban_jql`)
- Output formatting (table/JSON)
- `search_issues()` API (already supports limit)

## Testing

### Unit Tests

- `resolve_effective_limit` tests move from `issue/list.rs` to `cli/mod.rs` (same tests, new location)
- `SprintIssuesResult` construction tests in `sprints.rs`

### Integration Tests

Using wiremock to mock Jira API responses:

1. **Scrum board with limit**: Mock sprint list + sprint issues (page of 50). Run `board view --limit 3`. Assert output has 3 rows AND wiremock received exactly 1 sprint issues request (early-stop verified).
2. **Kanban board with limit**: Mock board config + search endpoint. Run `board view --limit 5`. Assert output has 5 rows.
3. **Flag conflict**: Run `board view --limit 3 --all`. Assert exit code 2 (clap `ArgumentConflict`).
4. **Default limit**: Run `board view` (no flags). Assert output has at most 30 rows.

### Existing Tests

No existing tests should break:
- `build_kanban_jql` tests are unchanged
- `sprint.rs` tests don't call `get_sprint_issues()` (they test `compute_sprint_summary`)
- `issue/list.rs` tests for `resolve_effective_limit` move to `cli/mod.rs` (imports adjusted from `use super::*` to direct function references since they'll be in the same module)
