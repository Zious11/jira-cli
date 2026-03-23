# Issue Module Split

## Overview

Split `src/cli/issue.rs` (1426 lines, 21 functions) into a directory module with 7 focused files (60-340 lines each). Pure structural refactor — no behavior changes.

## Motivation

`issue.rs` is the largest file in the codebase by 4x. It contains 13 async functions, 4 public helpers, and 3 private utilities (plus `handle` which becomes the dispatch in `mod.rs`). This makes it difficult to navigate, increases merge conflict risk, and reduces AI agent effectiveness (agents perform better with 200-450 line files than monolithic ones).

## File Structure

```
src/cli/issue/
├── mod.rs       # handle() dispatch + re-exports (~60 lines)
├── format.rs    # formatting helpers used by sibling modules (~80 lines)
├── list.rs      # handle_list, build_fallback_jql, handle_view (~280 lines)
├── create.rs    # handle_create, handle_edit (~270 lines)
├── workflow.rs  # handle_move, handle_transitions, handle_assign, handle_comment, handle_open (~340 lines)
├── links.rs     # handle_link, handle_unlink, handle_link_types (~230 lines)
└── helpers.rs   # resolve_team_field, resolve_story_points_field_id, prompt_input (~140 lines)
```

### Grouping Rationale

| File | Theme | Functions |
|------|-------|-----------|
| `list.rs` | Read operations — JQL building and display | `handle_list`, `build_fallback_jql`, `handle_view` |
| `create.rs` | Field-building operations — JSON payload construction | `handle_create`, `handle_edit` |
| `workflow.rs` | State-changing operations on existing issues | `handle_move`, `handle_transitions`, `handle_assign`, `handle_comment`, `handle_open` |
| `links.rs` | Relationship operations | `handle_link`, `handle_unlink`, `handle_link_types` |
| `format.rs` | Output formatting used by sprint.rs and board.rs | `format_issue_row`, `format_issue_rows_public`, `issue_table_headers`, `format_points` |
| `helpers.rs` | Shared utilities used across multiple handlers | `resolve_team_field`, `resolve_story_points_field_id`, `prompt_input` |

## Module Visibility

| Scope | Visibility | Example |
|-------|-----------|---------|
| Handlers called by `mod.rs` dispatch | `pub(super)` | `pub(super) async fn handle_list(...)` |
| Helpers called by sibling submodules | `pub(super)` | `pub(super) fn resolve_team_field(...)` |
| Format functions re-exported for external modules | `pub` | `pub fn format_issue_row(...)` |

## `mod.rs` Structure

```rust
mod create;
mod format;
mod helpers;
mod links;
mod list;
mod workflow;

pub use format::{format_issue_row, format_issue_rows_public, format_points, issue_table_headers};

pub async fn handle(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    match command {
        IssueCommand::List { .. } => list::handle_list(command, output_format, config, client, project_override, no_input).await,
        IssueCommand::View { .. } => list::handle_view(command, output_format, config, client).await,
        IssueCommand::Create { .. } => create::handle_create(command, output_format, config, client, project_override, no_input).await,
        IssueCommand::Edit { .. } => create::handle_edit(command, output_format, config, client, no_input).await,
        IssueCommand::Move { .. } => workflow::handle_move(command, output_format, client, no_input).await,
        IssueCommand::Transitions { .. } => workflow::handle_transitions(command, output_format, client).await,
        IssueCommand::Assign { .. } => workflow::handle_assign(command, output_format, client).await,
        IssueCommand::Comment { .. } => workflow::handle_comment(command, output_format, client).await,
        IssueCommand::Open { .. } => workflow::handle_open(command, client).await,
        IssueCommand::Link { .. } => links::handle_link(command, output_format, client, no_input).await,
        IssueCommand::Unlink { .. } => links::handle_unlink(command, output_format, client, no_input).await,
        IssueCommand::LinkTypes => links::handle_link_types(output_format, client).await,
    }
}
```

## Import Paths

Callers are unchanged:

| Caller | Import | Status |
|--------|--------|--------|
| `main.rs` | `cli::issue::handle()` | Unchanged |
| `sprint.rs` | `super::issue::format_issue_row` | Unchanged (re-exported) |
| `sprint.rs` | `super::issue::format_points` | Unchanged (re-exported) |
| `sprint.rs` | `super::issue::issue_table_headers` | Unchanged (re-exported) |
| `board.rs` | `super::issue::format_issue_rows_public` | Unchanged (re-exported) |

Sibling submodule imports use `super::` pattern:
```rust
// In list.rs
use super::helpers::{resolve_team_field, resolve_story_points_field_id};

// In list.rs (handle_view uses format_points)
use super::format::format_points;
```

## Test Migration

| Current Location | New Location |
|-----------------|--------------|
| `build_fallback_jql` tests (5) | `list.rs` `#[cfg(test)] mod tests` |
| `format_points` tests (3) | `format.rs` `#[cfg(test)] mod tests` |

Integration tests in `tests/issue_commands.rs` are unaffected — they test the API layer, not CLI handlers.

No new tests. This is a pure structural refactor validated by existing tests passing.

## Convention

This establishes the pattern for splitting CLI modules:
- Convert `src/cli/{resource}.rs` → `src/cli/{resource}/mod.rs` + submodules
- Group handlers by operation theme (reads, writes, state changes, relationships)
- Shared formatting in `format.rs`, shared utilities in `helpers.rs`
- Re-export public API through `mod.rs`
- Apply when a CLI module exceeds ~500 lines

Other CLI modules (`sprint.rs` at 239 lines, `board.rs` at 77 lines) do not need splitting today.

## Files Touched

| File | Change |
|------|--------|
| `src/cli/issue.rs` | Deleted — replaced by directory |
| `src/cli/issue/mod.rs` | New — dispatch + re-exports |
| `src/cli/issue/format.rs` | New — formatting helpers |
| `src/cli/issue/list.rs` | New — list + view handlers |
| `src/cli/issue/create.rs` | New — create + edit handlers |
| `src/cli/issue/workflow.rs` | New — move + transitions + assign + comment + open |
| `src/cli/issue/links.rs` | New — link + unlink + link-types |
| `src/cli/issue/helpers.rs` | New — shared utilities |
| `CLAUDE.md` | Update architecture tree |
