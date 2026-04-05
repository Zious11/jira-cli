# Sprint Current --limit — Design Spec

## Problem

`jr sprint current` fetches all issues from the active sprint with no way to control output size. The Jira Agile API sprint issues endpoint (`/rest/agile/1.0/sprint/{sprintId}/issue`) pages at max 50 items — `get_sprint_issues()` paginates through ALL pages with no early-stop when no limit is provided.

Every other list-style command in jr has `--limit` — `sprint current` is the remaining outlier. AI agents cannot safely call it without risking context overflow.

**Issue:** #72

## Solution

Add `--limit <N>` and `--all` flags to `sprint current`, matching the pattern already used by `issue list` and `board view`.

The infrastructure already exists:
- `resolve_effective_limit(limit, all)` in `cli/mod.rs` (defaults to 30 when neither flag is set)
- `get_sprint_issues()` already accepts `limit: Option<u32>` and returns `SprintIssuesResult { issues, has_more }`
- `handle_current` currently passes `None` as the limit (unbounded)

This is a wiring change — no new API calls, no new utility functions.

### CLI Changes (`src/cli/mod.rs`)

Add `--limit` and `--all` to `SprintCommand::Current`:

```rust
Current {
    /// Board ID (overrides board_id in .jr.toml)
    #[arg(long)]
    board: Option<u64>,
    /// Maximum number of issues to return
    #[arg(long)]
    limit: Option<u32>,
    /// Fetch all results (no default limit)
    #[arg(long, conflicts_with = "limit")]
    all: bool,
},
```

Matches `BoardCommand::View` exactly.

### Handler Changes (`src/cli/sprint.rs`)

**`handle` function:**

Extract `limit` and `all` from `SprintCommand::Current` in the match arms:

```rust
let board_override = match &command {
    SprintCommand::List { board } => *board,
    SprintCommand::Current { board, .. } => *board,
};
```

Pass `limit` and `all` to `handle_current`:

```rust
SprintCommand::Current { limit, all, .. } => {
    handle_current(board_id, client, output_format, config, limit, all).await
}
```

**`handle_current` function:**

Add `limit: Option<u32>` and `all: bool` parameters. Compute effective limit and pass to API:

```rust
async fn handle_current(
    board_id: u64,
    client: &JiraClient,
    output_format: &OutputFormat,
    config: &Config,
    limit: Option<u32>,
    all: bool,
) -> Result<()> {
    let effective_limit = crate::cli::resolve_effective_limit(limit, all);
    // ...
    let result = client
        .get_sprint_issues(sprint.id, None, effective_limit, &extra)
        .await?;
    let issues = result.issues;
    let has_more = result.has_more;
    // ...
```

After printing the table (both Table and Json paths), print the "more results" hint:

```rust
if has_more && !all {
    eprintln!(
        "Showing {} results. Use --limit or --all to see more.",
        issues.len()
    );
}
```

The hint uses the same wording as `board view`'s scrum path. While the Agile API sprint issues endpoint returns a `total` field (used by `OffsetPage.has_more()` for pagination), the hint only reports how many issues were fetched — not the full sprint total — since the purpose of `--limit` is to avoid fetching everything.

### Sprint Points Summary on Limited Results

When `--limit` caps results, `compute_sprint_summary` runs on the limited set. This means points shown may not reflect the full sprint. This is the same tradeoff `board view` makes — fetching all issues just for a summary would defeat the purpose of `--limit`. The sprint header still shows the sprint name and end date (from the sprint object, not issues), so the user retains context.

## What Changes

- `SprintCommand::Current` gains `limit: Option<u32>` and `all: bool` fields in `src/cli/mod.rs`
- `handle` in `sprint.rs` updates both match sites: the `board_override` extraction match (add `..` to ignore new fields) and the dispatch match (extract `limit` and `all`, pass to `handle_current`)
- `handle_current` in `sprint.rs` computes `effective_limit` and passes it to `get_sprint_issues`, prints "more results" hint

## What Doesn't Change

- `get_sprint_issues()` API method — already supports limits
- `resolve_effective_limit()` — reused as-is
- `SprintCommand::List` — no limit needed (sprints are few)
- `compute_sprint_summary` — unchanged (operates on whatever issues it receives)
- `handle_list` — unchanged
- Sprint header output (name, end date) — unchanged
- JSON output structure — unchanged (just fewer issues when limited)

## Testing

### Integration Tests

Using wiremock:

1. **Default limit caps at 30**: Mock sprint issues with 35 results. Assert only 30 returned and "Showing 30 results" hint printed to stderr.
2. **--limit flag**: Mock sprint issues with 20 results. Call `--limit 5`. Assert 5 returned and hint printed.
3. **--all flag**: Mock sprint issues with 35 results. Call `--all`. Assert all 35 returned and no hint.
4. **Under default limit**: Mock sprint issues with 10 results. Assert all 10 returned and no hint.

### Existing Tests

- `compute_sprint_summary` unit tests — unchanged
- `build_kanban_jql` unit tests — unchanged
- Board integration tests — unchanged
