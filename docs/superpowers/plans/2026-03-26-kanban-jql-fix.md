# Fix Kanban JQL `AND ORDER BY` Bug — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix invalid JQL in `board view` kanban path where `ORDER BY rank ASC` is incorrectly joined with predicates via `AND`, producing a 400 error from Jira Cloud.

**Architecture:** Extract kanban JQL construction into a pure helper function `build_kanban_jql()`, fix the ORDER BY separation, and add unit tests. The handler calls the helper instead of building JQL inline. Note: the spec shows an inline fix in its "Code Change" section, but the spec's Testing section prescribes extracting a helper for testability — this plan follows the helper approach.

**Tech Stack:** Rust, clap 4 (derive API)

**Spec:** `docs/superpowers/specs/2026-03-26-kanban-jql-fix-design.md`

---

### Task 1: Extract helper, fix JQL, and add tests

**Files:**
- Modify: `src/cli/board.rs`

- [ ] **Step 1: Write the failing tests**

In `src/cli/board.rs`, add a `#[cfg(test)]` module at the end of the file (after line 82):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_kanban_jql_with_project() {
        let jql = build_kanban_jql(Some("FOO"));
        assert_eq!(
            jql,
            "project = \"FOO\" AND statusCategory != Done ORDER BY rank ASC"
        );
    }

    #[test]
    fn build_kanban_jql_without_project() {
        let jql = build_kanban_jql(None);
        assert_eq!(jql, "statusCategory != Done ORDER BY rank ASC");
    }

    #[test]
    fn build_kanban_jql_escapes_special_characters() {
        let jql = build_kanban_jql(Some("FOO\"BAR"));
        assert_eq!(
            jql,
            "project = \"FOO\\\"BAR\" AND statusCategory != Done ORDER BY rank ASC"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib -- board::tests`
Expected: FAIL with "cannot find function `build_kanban_jql`"

- [ ] **Step 3: Implement `build_kanban_jql` and update `handle_view`**

In `src/cli/board.rs`, add this function before `handle_view` (after `handle_list`, around line 33). Note: `crate::jql::escape_value` is used as a full path intentionally — there is no `use crate::jql` import in this file, matching the existing inline usage pattern.

```rust
/// Build JQL for kanban board view: all non-Done issues, ordered by rank.
fn build_kanban_jql(project_key: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(pk) = project_key {
        parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
    }
    parts.push("statusCategory != Done".into());
    let where_clause = parts.join(" AND ");
    format!("{where_clause} ORDER BY rank ASC")
}
```

Then in `handle_view`, find the kanban JQL construction block (lines 62-69, inside the `else` branch). Replace these lines:

```rust
        let mut jql_parts: Vec<String> = Vec::new();
        if let Some(ref pk) = project_key {
            jql_parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
        }
        jql_parts.push("statusCategory != Done".into());
        jql_parts.push("ORDER BY rank ASC".into());
        let jql = jql_parts.join(" AND ");
        client.search_issues(&jql, None, &[]).await?.issues
```

With:

```rust
        let jql = build_kanban_jql(project_key.as_deref());
        client.search_issues(&jql, None, &[]).await?.issues
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib -- board::tests`
Expected: All 3 tests PASS.

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 6: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: No warnings, no format issues.

- [ ] **Step 7: Commit**

```bash
git add src/cli/board.rs
git commit -m "fix: kanban board view builds valid JQL (ORDER BY separated from predicates) (#31)"
```

---

### Task 2: Final verification

**Files:**
- All modified files from Task 1

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Zero warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt --all -- --check`
Expected: No format issues.
