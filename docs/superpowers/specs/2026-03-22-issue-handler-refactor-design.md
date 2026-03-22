# Issue Handler Parameter Refactor

## Overview

Refactor `src/cli/issue.rs` to pass owned `IssueCommand` enum variants directly to handler functions instead of destructuring all fields in the `handle()` dispatch and passing them as individual parameters. This eliminates all 4 `#[allow(clippy::too_many_arguments)]` annotations.

## Problem

The current `handle()` function destructures each `IssueCommand` variant, then passes every field as a separate parameter to the handler alongside shared params (`output_format`, `config`, `client`, `project_override`, `no_input`). This results in:

| Handler | Current Params | `#[allow]` |
|---------|---------------|------------|
| `handle_list` | 10 | Yes |
| `handle_create` | 15 | Yes |
| `handle_edit` | 12 | Yes |
| `handle_comment` | 8 | Yes |

## Solution

Pass the whole `IssueCommand` enum (owned) to each handler. Handlers destructure inside with `let else { unreachable!() }`.

### Dispatch Pattern

```rust
pub async fn handle(
    command: IssueCommand,  // owned, moved into handler
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    match command {
        IssueCommand::List { .. } => {
            handle_list(command, output_format, config, client, project_override, no_input).await
        }
        IssueCommand::View { .. } => {
            handle_view(command, output_format, config, client).await
        }
        IssueCommand::Create { .. } => {
            handle_create(command, output_format, config, client, project_override, no_input).await
        }
        IssueCommand::Edit { .. } => {
            handle_edit(command, output_format, config, client, no_input).await
        }
        IssueCommand::Move { .. } => {
            handle_move(command, output_format, client, no_input).await
        }
        IssueCommand::Transitions { .. } => {
            handle_transitions(command, output_format, client).await
        }
        IssueCommand::Assign { .. } => {
            handle_assign(command, output_format, client).await
        }
        IssueCommand::Comment { .. } => {
            handle_comment(command, output_format, client).await
        }
        IssueCommand::Open { .. } => {
            handle_open(command, client).await
        }
    }
}
```

The `{ .. }` pattern checks the variant without binding fields, so the compiler allows moving `command` into the handler. Verified: this compiles with rustc.

### Handler Pattern

Each handler destructures at the top:

```rust
async fn handle_list(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::List {
        jql,
        status,
        team,
        limit,
        points,
    } = command
    else {
        unreachable!()
    };
    // ... rest of function body unchanged
}
```

### Result

| Handler | Before | After | `#[allow]` |
|---------|--------|-------|------------|
| `handle_list` | 10 params | 6 params | Removed |
| `handle_create` | 15 params | 6 params | Removed |
| `handle_edit` | 12 params | 5 params | Removed |
| `handle_comment` | 8 params | 3 params | Removed |
| `handle_view` | 4 params | 4 params | N/A |
| `handle_move` | 4 params | 4 params | N/A |
| `handle_transitions` | 3 params | 3 params | N/A |
| `handle_assign` | 4 params | 3 params | N/A |
| `handle_open` | 3 params | 2 params | N/A |

All handlers at or below clippy's 7-parameter threshold. All 4 `#[allow(clippy::too_many_arguments)]` removed.

## Scope

- **Only file modified:** `src/cli/issue.rs`
- **Pure refactor:** no behavior changes, no new features, no API changes
- **All existing tests must pass unchanged** — the function signatures are internal (not `pub` except `handle()` and `format_issue_rows_public()`)

## Why This Pattern

Validated against Rust CLI best practices (Perplexity research):

- **Idiomatic:** `let else { unreachable!() }` is stable since Rust 1.65, commonly used for single-variant guards
- **Ownership:** passing owned enum avoids cloning String fields — the command is parsed once, dispatched once, never reused
- **Precedent:** ripgrep and other Rust CLIs use explicit parameter passing over context structs; this pattern keeps params explicit while reducing count
- **Safe:** `unreachable!()` is guaranteed safe because the `match` in `handle()` already proved the variant

## Testing

- All existing unit tests in `issue.rs` pass (they test internal helpers like `build_fallback_jql` and `format_points`, not handler signatures)
- All existing integration tests pass (they call `JiraClient` methods directly, not handlers)
- No new tests needed — this is a pure signature refactor

## Files Touched

| File | Change |
|------|--------|
| `src/cli/issue.rs` | Refactor `handle()` dispatch and all 9 handler signatures |
