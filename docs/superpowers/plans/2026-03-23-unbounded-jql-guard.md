# Unbounded JQL Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reject unbounded JQL queries in `issue list` before they hit the Jira API, with an actionable error message (closes GitHub issue #16).

**Architecture:** Change `build_fallback_jql` from `-> String` to `-> Result<String>`, returning `JrError::UserError` (exit code 64) when all filters are `None`. Add a stderr warning to `board view` kanban path when no project is configured.

**Tech Stack:** Rust, anyhow, thiserror (JrError)

**Spec:** `docs/superpowers/specs/2026-03-23-unbounded-jql-guard-design.md`

---

## File Structure

| File | Responsibility | Change |
|------|---------------|--------|
| `src/cli/issue/list.rs` | JQL construction, list/view handlers, unit tests | `build_fallback_jql` returns `Result<String>`, callers add `?`, test updates |
| `src/cli/board.rs` | Board list/view handlers | Add kanban warning when no project configured |

---

### Task 1: Update `build_fallback_jql` to reject unbounded queries

**Files:**
- Modify: `src/cli/issue/list.rs:1-2` (imports)
- Modify: `src/cli/issue/list.rs:128-145` (function body)
- Modify: `src/cli/issue/list.rs:313-364` (tests)

- [ ] **Step 1: Write the failing test**

Replace the test `fallback_jql_no_filters_still_has_order_by` (lines 353-357) and add an error content assertion. Also update `use` import at line 315 since tests now need `crate::error::JrError`. The new test block for the error case:

In `src/cli/issue/list.rs`, replace lines 353-357:

```rust
    #[test]
    fn fallback_jql_errors_when_no_filters() {
        let result = build_fallback_jql(None, None, None);
        assert!(result.is_err(), "Expected error for unbounded query");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("--project"),
            "Error should mention --project: {err_msg}"
        );
        assert!(
            err_msg.contains(".jr.toml"),
            "Error should mention .jr.toml: {err_msg}"
        );
        assert!(
            err_msg.contains("jr init"),
            "Error should mention jr init: {err_msg}"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --lib fallback_jql_errors_when_no_filters
```
Expected: FAIL — `build_fallback_jql` currently returns `String`, not `Result<String>`, so the test won't compile.

- [ ] **Step 3: Implement the fix**

In `src/cli/issue/list.rs`:

**3a.** Add `use crate::error::JrError;` to the imports at the top of the file. The existing import line is:

```rust
use anyhow::Result;
```

Change it to:

```rust
use anyhow::Result;

use crate::error::JrError;
```

**3b.** Change the function signature and add the guard. Replace the entire `build_fallback_jql` function (lines 128-145) with:

```rust
fn build_fallback_jql(
    project_key: Option<&str>,
    status: Option<&str>,
    resolved_team: Option<&(String, String)>,
) -> Result<String> {
    if project_key.is_none() && status.is_none() && resolved_team.is_none() {
        return Err(JrError::UserError(
            "No project or filters specified. Use --project KEY, --status STATUS, or --team NAME. \
             You can also set a default project in .jr.toml or run \"jr init\"."
                .into(),
        )
        .into());
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(pk) = project_key {
        parts.push(format!("project = \"{}\"", pk));
    }
    if let Some(s) = status {
        parts.push(format!("status = \"{}\"", s));
    }
    if let Some((field_id, team_uuid)) = resolved_team {
        parts.push(format!("{} = \"{}\"", field_id, team_uuid));
    }
    let where_clause = parts.join(" AND ");
    Ok(format!("{} ORDER BY updated DESC", where_clause))
}
```

Note: `Err(JrError::UserError(...).into())` converts the `JrError` into `anyhow::Error` so it matches the `Result<String>` (which is `anyhow::Result<String>`). This ensures `main.rs` can `downcast_ref::<JrError>()` to get exit code 64.

**3c.** Update the three callers in `handle_list` to add `?`:

Line 72-76 — change:
```rust
                            _ => build_fallback_jql(
                                project_key.as_deref(),
                                status.as_deref(),
                                resolved_team.as_ref(),
                            ),
```
to:
```rust
                            _ => build_fallback_jql(
                                project_key.as_deref(),
                                status.as_deref(),
                                resolved_team.as_ref(),
                            )?,
```

Line 96-100 — change:
```rust
                Err(_) => build_fallback_jql(
                    project_key.as_deref(),
                    status.as_deref(),
                    resolved_team.as_ref(),
                ),
```
to:
```rust
                Err(_) => build_fallback_jql(
                    project_key.as_deref(),
                    status.as_deref(),
                    resolved_team.as_ref(),
                )?,
```

Line 103-107 — change:
```rust
            build_fallback_jql(
                project_key.as_deref(),
                status.as_deref(),
                resolved_team.as_ref(),
            )
```
to:
```rust
            build_fallback_jql(
                project_key.as_deref(),
                status.as_deref(),
                resolved_team.as_ref(),
            )?
```

**3d.** Update the existing tests that call `build_fallback_jql` to unwrap the `Result`. Four tests need `.unwrap()`:

`fallback_jql_order_by_not_joined_with_and` (line 319) — change:
```rust
        let jql = build_fallback_jql(Some("PROJ"), None, None);
```
to:
```rust
        let jql = build_fallback_jql(Some("PROJ"), None, None).unwrap();
```

`fallback_jql_with_team_has_valid_order_by` (line 330) — change:
```rust
        let jql = build_fallback_jql(Some("PROJ"), None, Some(&team));
```
to:
```rust
        let jql = build_fallback_jql(Some("PROJ"), None, Some(&team)).unwrap();
```

`fallback_jql_with_all_filters` (line 342) — change:
```rust
        let jql = build_fallback_jql(Some("PROJ"), Some("In Progress"), Some(&team));
```
to:
```rust
        let jql = build_fallback_jql(Some("PROJ"), Some("In Progress"), Some(&team)).unwrap();
```

`fallback_jql_with_status_only` (line 361) — change:
```rust
        let jql = build_fallback_jql(None, Some("Done"), None);
```
to:
```rust
        let jql = build_fallback_jql(None, Some("Done"), None).unwrap();
```

- [ ] **Step 4: Run all tests**

```bash
cargo test --lib
```
Expected: All tests pass, including the new `fallback_jql_errors_when_no_filters`.

- [ ] **Step 5: Run clippy and fmt**

```bash
cargo clippy --all --all-features --tests -- -D warnings
cargo fmt --all
```
Expected: Zero warnings, formatting clean.

- [ ] **Step 6: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "fix: reject unbounded JQL in issue list with actionable error

build_fallback_jql now returns Result<String> and errors with exit
code 64 when no project, status, or team filter is provided.

Closes #16"
```

---

### Task 2: Add kanban warning to `board view`

**Files:**
- Modify: `src/cli/board.rs:54-65` (kanban path)

- [ ] **Step 1: Add the warning**

In `src/cli/board.rs`, after line 56 (`let project_key = config.project_key(None);`), add:

```rust
        if project_key.is_none() {
            eprintln!("warning: no project configured for board. Showing issues across all projects. Set project in .jr.toml to scope results.");
        }
```

The full kanban block (lines 54-65) should now read:

```rust
    } else {
        // Kanban: search for issues not in Done status category
        let project_key = config.project_key(None);
        if project_key.is_none() {
            eprintln!("warning: no project configured for board. Showing issues across all projects. Set project in .jr.toml to scope results.");
        }
        let mut jql_parts: Vec<String> = Vec::new();
        if let Some(ref pk) = project_key {
            jql_parts.push(format!("project = \"{}\"", pk));
        }
        jql_parts.push("statusCategory != Done".into());
        jql_parts.push("ORDER BY rank ASC".into());
        let jql = jql_parts.join(" AND ");
        client.search_issues(&jql, None, &[]).await?
    };
```

- [ ] **Step 2: Run all tests**

```bash
cargo test
```
Expected: All tests pass (no board integration tests are affected).

- [ ] **Step 3: Run clippy and fmt**

```bash
cargo clippy --all --all-features --tests -- -D warnings
cargo fmt --all
```
Expected: Zero warnings, formatting clean.

- [ ] **Step 4: Commit**

```bash
git add src/cli/board.rs
git commit -m "fix: warn when board view kanban has no project scope

Emits a stderr warning when no project is configured, since the
query returns issues across all projects which may be a large set."
```
