# Snapshot Tests for Write Command JSON Output Schemas

**Issue:** #135
**Date:** 2026-04-05

## Goal

Protect `--output json` schemas on write commands from accidental drift by extracting inline `json!({...})` construction into named builder functions and pinning them with `insta::assert_json_snapshot!` tests.

## Background

Write command handlers (move, assign, edit, link, unlink, sprint add/remove) construct JSON output inline using `serde_json::json!({...})`. These schemas have no compile-time enforcement — a typo or field rename silently changes the contract for downstream consumers. Snapshot tests catch this: any schema change requires an explicit `cargo insta review` approval.

Commands that serialize full API response structs (create, comment, worklog) are excluded — their schemas are already enforced by the struct definition.

## Approach

**Extract + snapshot.** Move inline `json!()` calls into pure builder functions, call them from handlers, snapshot-test the builders directly.

This was validated with Perplexity as idiomatic Rust for CLI JSON schema protection. It complements the existing handler-level integration tests in `cli_handler.rs` which test the full request/response path.

### Why not handler-level snapshot tests?

Handler tests require async runtime + wiremock mocks for every test. The JSON builders are pure functions — testing them directly is faster, simpler, and more focused. The handler tests already cover the wiring.

## File Structure

### New files

- `src/cli/issue/json_output.rs` — Builder functions for issue command JSON responses + snapshot tests
- `src/cli/issue/snapshots/` — insta snapshot files for issue command tests

### Modified files

- `src/cli/issue/mod.rs` — Add `mod json_output;`
- `src/cli/issue/workflow.rs` — Replace inline `json!()` in `handle_move` and `handle_assign` with calls to `json_output::*`
- `src/cli/issue/create.rs` — Replace inline `json!()` in `handle_edit` with call to `json_output::edit_response`
- `src/cli/issue/links.rs` — Replace inline `json!()` in `handle_link` and `handle_unlink` with calls to `json_output::*`
- `src/cli/sprint.rs` — Replace inline `json!()` in `handle_add` and `handle_remove` with calls to sprint response builders (defined in same file or a small helper)

## Builder Functions

All functions return `serde_json::Value`.

### Issue commands (`src/cli/issue/json_output.rs`)

```rust
pub(crate) fn move_response(key: &str, status: &str, changed: bool) -> Value

pub(crate) fn assign_changed_response(key: &str, display_name: &str, account_id: &str) -> Value

pub(crate) fn assign_unchanged_response(key: &str, display_name: &str, account_id: &str) -> Value

pub(crate) fn unassign_response(key: &str) -> Value

pub(crate) fn edit_response(key: &str) -> Value

pub(crate) fn link_response(key1: &str, key2: &str, link_type: &str) -> Value

pub(crate) fn unlink_response(unlinked: bool, count: usize) -> Value
```

### Sprint commands (inline in `src/cli/sprint.rs`)

```rust
fn sprint_add_response(sprint_id: u64, issues: &[String]) -> Value

fn sprint_remove_response(issues: &[String]) -> Value
```

Sprint builders stay in `sprint.rs` as private functions with `#[cfg(test)]`-gated snapshot tests at the bottom of the file, following the pattern used by `src/adf.rs`.

## Snapshot Test Pattern

Each builder gets one snapshot test with representative values:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_json_snapshot;

    #[test]
    fn test_move_response_changed() {
        assert_json_snapshot!(move_response("TEST-1", "In Progress", true));
    }

    #[test]
    fn test_move_response_unchanged() {
        assert_json_snapshot!(move_response("TEST-1", "Done", false));
    }
}
```

No redactions needed — all inputs are deterministic test values.

Snapshot files land in `src/cli/issue/snapshots/` (insta auto-creates this directory based on the source file location).

## Schemas

### `move_response`
```json
{
  "key": "TEST-1",
  "status": "In Progress",
  "changed": true
}
```

### `assign_changed_response`
```json
{
  "key": "TEST-1",
  "assignee": "Jane Doe",
  "assignee_account_id": "abc123",
  "changed": true
}
```

### `assign_unchanged_response`
```json
{
  "key": "TEST-1",
  "assignee": "Jane Doe",
  "assignee_account_id": "abc123",
  "changed": false
}
```

### `unassign_response`
```json
{
  "key": "TEST-1",
  "assignee": null,
  "changed": true
}
```

### `edit_response`
```json
{
  "key": "TEST-1",
  "updated": true
}
```

### `link_response`
```json
{
  "key1": "TEST-1",
  "key2": "TEST-2",
  "type": "Blocks",
  "linked": true
}
```

### `unlink_response` (success)
```json
{
  "unlinked": true,
  "count": 2
}
```

### `unlink_response` (no match)
```json
{
  "unlinked": false,
  "count": 0
}
```

### `sprint_add_response`
```json
{
  "sprint_id": 100,
  "issues": ["TEST-1", "TEST-2"],
  "added": true
}
```

### `sprint_remove_response`
```json
{
  "issues": ["TEST-1", "TEST-2"],
  "removed": true
}
```

## Out of Scope

- `issue create` JSON output — serializes the API response struct (`CreateIssueResponse`) with a `url` field appended. Schema is struct-enforced.
- `issue comment` JSON output — serializes the full `Comment` struct. Struct-enforced.
- `worklog add` JSON output — serializes the full `Worklog` struct. Struct-enforced.
- Read commands (list, view, etc.) — different concern, not requested.
