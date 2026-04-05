# Snapshot Tests for Write Command JSON Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract inline `json!({...})` output construction from write command handlers into named builder functions and pin them with `insta::assert_json_snapshot!` tests to protect `--output json` schemas from accidental drift.

**Architecture:** Pure builder functions in `src/cli/issue/json_output.rs` for issue commands (move, assign, edit, link, unlink) and private functions in `src/cli/sprint.rs` for sprint commands. Handlers call these builders instead of constructing JSON inline. Each builder gets a snapshot test using `insta::assert_json_snapshot!`.

**Tech Stack:** Rust, serde_json, insta (with `json` feature — already a dev-dependency)

---

## File Structure

| File | Role | Action |
|------|------|--------|
| `src/cli/issue/json_output.rs` | Builder functions for issue command JSON responses + snapshot tests | Create |
| `src/cli/issue/mod.rs` | Issue module declarations | Modify: add `pub(crate) mod json_output;` |
| `src/cli/issue/workflow.rs` | Move + assign handlers | Modify: replace inline `json!()` with builder calls |
| `src/cli/issue/create.rs` | Edit handler | Modify: replace inline `json!()` with builder call |
| `src/cli/issue/links.rs` | Link + unlink handlers | Modify: replace inline `json!()` with builder calls |
| `src/cli/sprint.rs` | Sprint add/remove handlers | Modify: extract builders, add snapshot tests |

---

### Task 1: Create json_output.rs with builder functions and snapshot tests

**Files:**
- Create: `src/cli/issue/json_output.rs`
- Modify: `src/cli/issue/mod.rs:1`

- [ ] **Step 1: Create `src/cli/issue/json_output.rs` with all builder functions and snapshot tests**

```rust
use serde_json::{Value, json};

/// JSON response for `issue move` — both changed and idempotent cases.
pub(crate) fn move_response(key: &str, status: &str, changed: bool) -> Value {
    json!({
        "key": key,
        "status": status,
        "changed": changed
    })
}

/// JSON response for `issue assign` when the assignment changed.
pub(crate) fn assign_changed_response(key: &str, display_name: &str, account_id: &str) -> Value {
    json!({
        "key": key,
        "assignee": display_name,
        "assignee_account_id": account_id,
        "changed": true
    })
}

/// JSON response for `issue assign` when already assigned to the target user.
pub(crate) fn assign_unchanged_response(
    key: &str,
    display_name: &str,
    account_id: &str,
) -> Value {
    json!({
        "key": key,
        "assignee": display_name,
        "assignee_account_id": account_id,
        "changed": false
    })
}

/// JSON response for `issue assign --unassign`.
pub(crate) fn unassign_response(key: &str) -> Value {
    json!({
        "key": key,
        "assignee": null,
        "changed": true
    })
}

/// JSON response for `issue edit`.
pub(crate) fn edit_response(key: &str) -> Value {
    json!({
        "key": key,
        "updated": true
    })
}

/// JSON response for `issue link`.
pub(crate) fn link_response(key1: &str, key2: &str, link_type: &str) -> Value {
    json!({
        "key1": key1,
        "key2": key2,
        "type": link_type,
        "linked": true
    })
}

/// JSON response for `issue unlink` — covers both success and no-match cases.
pub(crate) fn unlink_response(unlinked: bool, count: usize) -> Value {
    json!({
        "unlinked": unlinked,
        "count": count
    })
}

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

    #[test]
    fn test_assign_changed() {
        assert_json_snapshot!(assign_changed_response("TEST-1", "Jane Doe", "abc123"));
    }

    #[test]
    fn test_assign_unchanged() {
        assert_json_snapshot!(assign_unchanged_response("TEST-1", "Jane Doe", "abc123"));
    }

    #[test]
    fn test_unassign() {
        assert_json_snapshot!(unassign_response("TEST-1"));
    }

    #[test]
    fn test_edit() {
        assert_json_snapshot!(edit_response("TEST-1"));
    }

    #[test]
    fn test_link() {
        assert_json_snapshot!(link_response("TEST-1", "TEST-2", "Blocks"));
    }

    #[test]
    fn test_unlink_success() {
        assert_json_snapshot!(unlink_response(true, 2));
    }

    #[test]
    fn test_unlink_no_match() {
        assert_json_snapshot!(unlink_response(false, 0));
    }
}
```

- [ ] **Step 2: Add the module declaration to `src/cli/issue/mod.rs`**

In `src/cli/issue/mod.rs`, add `pub(crate) mod json_output;` after the existing module declarations. The top of the file should become:

```rust
mod assets;
mod create;
mod format;
mod helpers;
pub(crate) mod json_output;
mod links;
mod list;
mod workflow;
```

- [ ] **Step 3: Run the snapshot tests to generate initial snapshot files**

Run: `cargo test --lib json_output`

Expected: All 9 tests **FAIL** — this is expected on first run. insta writes `.snap.new` files to `src/cli/issue/snapshots/` but does not auto-accept them.

Accept the new snapshots:

Run: `cargo insta accept`

(If `cargo-insta` is not installed, use: `INSTA_UPDATE=always cargo test --lib json_output`)

Verify snapshots exist:

Run: `ls src/cli/issue/snapshots/`

Expected: 9 `.snap` files like `jr__cli__issue__json_output__tests__test_move_response_changed.snap`.

Re-run to confirm tests pass with accepted snapshots:

Run: `cargo test --lib json_output`

Expected: All 9 tests PASS.

- [ ] **Step 4: Run full test suite to verify no regressions**

Run: `cargo test`

Expected: All tests pass, including the 9 new snapshot tests.

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue/json_output.rs src/cli/issue/mod.rs src/cli/issue/snapshots/
git commit -m "feat: add JSON output builder functions with snapshot tests (#135)"
```

---

### Task 2: Wire issue command handlers to use builder functions

**Files:**
- Modify: `src/cli/issue/workflow.rs:2,76-84,220-229,294-303,328-338,353-363`
- Modify: `src/cli/issue/create.rs:273-276,296-299`
- Modify: `src/cli/issue/links.rs:2,92-102,189-197,211-219`

This task replaces all inline `json!({...})` output construction in the issue command handlers with calls to the builder functions from Task 1.

- [ ] **Step 1: Update `src/cli/issue/workflow.rs`**

**Replace the import** on line 2. Change:

```rust
use serde_json::json;
```

to:

```rust
use super::json_output;
```

**Replace the move idempotent JSON** (lines 79-83). Change:

```rust
                    serde_json::to_string_pretty(&json!({
                        "key": key,
                        "status": current_status,
                        "changed": false
                    }))?
```

to:

```rust
                    serde_json::to_string_pretty(&json_output::move_response(
                        &key,
                        &current_status,
                        false,
                    ))?
```

**Replace the move changed JSON** (lines 224-228). Change:

```rust
                serde_json::to_string_pretty(&json!({
                    "key": key,
                    "status": new_status,
                    "changed": true
                }))?
```

to:

```rust
                serde_json::to_string_pretty(&json_output::move_response(
                    &key, new_status, true,
                ))?
```

**Replace the unassign JSON** (lines 298-302). Change:

```rust
                    serde_json::to_string_pretty(&json!({
                        "key": key,
                        "assignee": null,
                        "changed": true
                    }))?
```

to:

```rust
                    serde_json::to_string_pretty(&json_output::unassign_response(&key))?
```

**Replace the assign idempotent JSON** (lines 332-336). Change:

```rust
                        serde_json::to_string_pretty(&json!({
                            "key": key,
                            "assignee": display_name,
                            "assignee_account_id": account_id,
                            "changed": false
                        }))?
```

to:

```rust
                        serde_json::to_string_pretty(
                            &json_output::assign_unchanged_response(
                                &key,
                                &display_name,
                                &account_id,
                            ),
                        )?
```

**Replace the assign changed JSON** (lines 357-361). Change:

```rust
                serde_json::to_string_pretty(&json!({
                    "key": key,
                    "assignee": display_name,
                    "assignee_account_id": account_id,
                    "changed": true
                }))?
```

to:

```rust
                serde_json::to_string_pretty(&json_output::assign_changed_response(
                    &key,
                    &display_name,
                    &account_id,
                ))?
```

- [ ] **Step 2: Update `src/cli/issue/create.rs`**

Add an import after line 11 (`use super::helpers;`):

```rust
use super::json_output;
```

**Replace both `handle_edit` JSON outputs.** There are two identical occurrences (lines 275 and 298). In both places, change:

```rust
                        serde_json::to_string_pretty(&json!({ "key": key, "updated": true }))?
```

to:

```rust
                        serde_json::to_string_pretty(&json_output::edit_response(&key))?
```

Note: `use serde_json::json;` on line 2 must stay — `handle_create` uses `json!()` extensively for building request bodies.

- [ ] **Step 3: Update `src/cli/issue/links.rs`**

**Replace the import** on line 2. Change:

```rust
use serde_json::json;
```

to:

```rust
use super::json_output;
```

**Replace the link JSON** (lines 96-101). Change:

```rust
                serde_json::to_string_pretty(&json!({
                    "key1": key1,
                    "key2": key2,
                    "type": resolved_name,
                    "linked": true
                }))?
```

to:

```rust
                serde_json::to_string_pretty(&json_output::link_response(
                    &key1,
                    &key2,
                    &resolved_name,
                ))?
```

**Replace the unlink no-match JSON** (lines 193-196). Change:

```rust
                    serde_json::to_string_pretty(&json!({
                        "unlinked": false,
                        "count": 0
                    }))?
```

to:

```rust
                    serde_json::to_string_pretty(&json_output::unlink_response(false, 0))?
```

**Replace the unlink success JSON** (lines 215-218). Change:

```rust
                serde_json::to_string_pretty(&json!({
                    "unlinked": true,
                    "count": count
                }))?
```

to:

```rust
                serde_json::to_string_pretty(&json_output::unlink_response(true, count))?
```

- [ ] **Step 4: Run full test suite**

Run: `cargo test`

Expected: All tests pass — the existing handler-level tests in `cli_handler.rs` and `issue_commands.rs` validate that the refactored handlers produce identical output.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: Zero warnings. Check that removed `json` imports don't trigger `unused_import` warnings and that new imports don't trigger `unused` warnings.

- [ ] **Step 6: Commit**

```bash
git add src/cli/issue/workflow.rs src/cli/issue/create.rs src/cli/issue/links.rs
git commit -m "refactor: replace inline json!() with builder functions in issue handlers (#135)"
```

---

### Task 3: Add sprint response builders and snapshot tests

**Files:**
- Modify: `src/cli/sprint.rs:1-2,100-110,131-140,295-367`

- [ ] **Step 1: Add builder functions to `src/cli/sprint.rs`**

Add two private functions right before the existing `const MAX_SPRINT_ISSUES` (line 90). Insert:

```rust
fn sprint_add_response(sprint_id: u64, issues: &[String]) -> serde_json::Value {
    json!({
        "sprint_id": sprint_id,
        "issues": issues,
        "added": true
    })
}

fn sprint_remove_response(issues: &[String]) -> serde_json::Value {
    json!({
        "issues": issues,
        "removed": true
    })
}
```

- [ ] **Step 2: Wire `handle_add` to use the builder**

In `handle_add` (around line 105 after the insertion), change the JSON output from:

```rust
                output::render_json(&json!({
                    "sprint_id": sprint_id,
                    "issues": issues,
                    "added": true
                }))?
```

to:

```rust
                output::render_json(&sprint_add_response(sprint_id, &issues))?
```

- [ ] **Step 3: Wire `handle_remove` to use the builder**

In `handle_remove` (around line 144 after the insertion), change the JSON output from:

```rust
                output::render_json(&json!({
                    "issues": issues,
                    "removed": true
                }))?
```

to:

```rust
                output::render_json(&sprint_remove_response(&issues))?
```

- [ ] **Step 4: Add snapshot tests to the existing test module**

At the end of `src/cli/sprint.rs`, inside the existing `#[cfg(test)] mod tests { ... }`, add before the closing `}`:

```rust
    #[test]
    fn test_sprint_add_response() {
        insta::assert_json_snapshot!(sprint_add_response(
            100,
            &["TEST-1".to_string(), "TEST-2".to_string()]
        ));
    }

    #[test]
    fn test_sprint_remove_response() {
        insta::assert_json_snapshot!(sprint_remove_response(&[
            "TEST-1".to_string(),
            "TEST-2".to_string()
        ]));
    }
```

- [ ] **Step 5: Run the snapshot tests and accept**

Run: `cargo test --lib sprint`

Expected: The 2 new snapshot tests **FAIL** (first run, no `.snap` files yet). Existing sprint tests pass.

Accept the new snapshots:

Run: `cargo insta accept`

(If `cargo-insta` is not installed, use: `INSTA_UPDATE=always cargo test --lib sprint`)

Verify: `ls src/cli/snapshots/` should show 2 new `.snap` files.

Re-run to confirm:

Run: `cargo test --lib sprint`

Expected: All sprint tests PASS.

- [ ] **Step 6: Run full test suite and clippy**

Run: `cargo test && cargo clippy -- -D warnings`

Expected: All tests pass, zero clippy warnings.

- [ ] **Step 7: Commit**

```bash
git add src/cli/sprint.rs src/cli/snapshots/
git commit -m "refactor: extract sprint JSON output builders with snapshot tests (#135)"
```
