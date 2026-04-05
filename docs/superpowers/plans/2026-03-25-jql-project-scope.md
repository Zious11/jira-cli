# Fix `--jql` + `--project` Scope Composition — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix bug #54 where `--jql` overrides `--project` scope, so that project and JQL compose with AND logic.

**Architecture:** Extract project-key resolution before the JQL vs board-aware branch in `handle_list`. Fix `strip_order_by` to handle position-0 ORDER BY. Add a pure `build_jql_base_parts` function for testability.

**Tech Stack:** Rust, clap 4, wiremock (tests)

**Spec:** `docs/superpowers/specs/2026-03-25-jql-project-scope-design.md`

---

### Task 1: Fix `strip_order_by` to handle ORDER BY at position 0

**Files:**
- Modify: `src/jql.rs:40-47` (the `strip_order_by` function)
- Test: `src/jql.rs` (inline `#[cfg(test)]` module, after line 105)

- [ ] **Step 1: Write the failing test**

In `src/jql.rs`, inside the existing `mod tests` block, after the `strip_order_by_trims_whitespace` test (around line 105), add:

```rust
    #[test]
    fn strip_order_by_at_position_zero() {
        assert_eq!(strip_order_by("ORDER BY created DESC"), "");
    }

    #[test]
    fn strip_order_by_at_position_zero_lowercase() {
        assert_eq!(strip_order_by("order by rank ASC"), "");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib jql::tests::strip_order_by_at_position`
Expected: FAIL — both tests fail because the current implementation only searches for `" ORDER BY"` (with leading space) and misses `"ORDER BY"` at position 0.

- [ ] **Step 3: Fix `strip_order_by`**

Replace the current `strip_order_by` function in `src/jql.rs:40-47` with:

```rust
pub fn strip_order_by(jql: &str) -> &str {
    let upper = jql.to_ascii_uppercase();
    if let Some(pos) = upper.find(" ORDER BY") {
        jql[..pos].trim_end()
    } else if upper.starts_with("ORDER BY") {
        ""
    } else {
        jql
    }
}
```

- [ ] **Step 4: Run all `jql` tests to verify everything passes**

Run: `cargo test --lib jql::tests`
Expected: All tests PASS (existing tests unchanged, new tests pass).

- [ ] **Step 5: Commit**

```bash
git add src/jql.rs
git commit -m "fix: handle ORDER BY at position 0 in strip_order_by"
```

---

### Task 2: Extract `build_jql_base_parts` and fix project scoping with `--jql`

**Files:**
- Modify: `src/cli/issue/list.rs:89-144` (the `if let Some(raw_jql) = jql` block)
- Test: `src/cli/issue/list.rs` (inline `#[cfg(test)]` module)

**Context:** The current code in `handle_list` resolves the project key *inside* the `else` branch (line 96). The fix hoists `project_key` resolution above the `if/else` so it's available in both branches. To make the JQL composition directly testable, extract the `--jql` branch logic into a pure function.

- [ ] **Step 1: Write failing unit tests**

In `src/cli/issue/list.rs`, inside the existing `mod tests` block, add these tests at the end (before the closing `}`):

```rust
    #[test]
    fn build_jql_base_parts_jql_with_project() {
        let (parts, order_by) =
            build_jql_base_parts(Some("priority = Highest"), Some("PROJ"));
        assert_eq!(
            parts,
            vec![
                "project = \"PROJ\"".to_string(),
                "priority = Highest".to_string(),
            ]
        );
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_without_project() {
        let (parts, order_by) =
            build_jql_base_parts(Some("priority = Highest"), None);
        assert_eq!(parts, vec!["priority = Highest".to_string()]);
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_with_order_by_and_project() {
        let (parts, order_by) =
            build_jql_base_parts(Some("priority = Highest ORDER BY created DESC"), Some("PROJ"));
        assert_eq!(
            parts,
            vec![
                "project = \"PROJ\"".to_string(),
                "priority = Highest".to_string(),
            ]
        );
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_order_by_only_with_project() {
        let (parts, order_by) =
            build_jql_base_parts(Some("ORDER BY created DESC"), Some("PROJ"));
        assert_eq!(parts, vec!["project = \"PROJ\"".to_string()]);
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_order_by_only_no_project() {
        let (parts, order_by) =
            build_jql_base_parts(Some("ORDER BY created DESC"), None);
        assert!(parts.is_empty());
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_no_jql() {
        let (parts, order_by) = build_jql_base_parts(None, Some("PROJ"));
        assert!(parts.is_empty());
        assert_eq!(order_by, "");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib cli::issue::list::tests::build_jql_base_parts`
Expected: FAIL — `build_jql_base_parts` function does not exist yet.

- [ ] **Step 3: Implement `build_jql_base_parts`**

In `src/cli/issue/list.rs`, add this function above `handle_list` (after the imports, around line 17):

```rust
/// Build base JQL parts when `--jql` is provided.
///
/// Returns `(base_parts, order_by)`. When `jql` is `None` (no `--jql` flag),
/// returns empty parts and an empty order_by — the caller handles the
/// board-aware logic in that case.
fn build_jql_base_parts(
    jql: Option<&str>,
    project_key: Option<&str>,
) -> (Vec<String>, &'static str) {
    let Some(raw_jql) = jql else {
        return (Vec::new(), "");
    };

    let stripped = crate::jql::strip_order_by(raw_jql);
    let mut parts = Vec::new();

    if let Some(pk) = project_key {
        parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
    }
    if !stripped.is_empty() {
        parts.push(stripped.to_string());
    }

    (parts, "updated DESC")
}
```

- [ ] **Step 4: Run unit tests to verify they pass**

Run: `cargo test --lib cli::issue::list::tests::build_jql_base_parts`
Expected: All 6 tests PASS.

- [ ] **Step 5: Refactor `handle_list` to use `build_jql_base_parts`**

In `src/cli/issue/list.rs`, replace lines 89-144 (the entire `// Build base JQL + order by` block) with:

```rust
    // Resolve project key once, before the JQL vs board-aware branch
    let project_key = config.project_key(project_override);

    // Build base JQL + order by
    let (base_parts, order_by): (Vec<String>, &str) = {
        let (jql_parts, jql_order) =
            build_jql_base_parts(jql.as_deref(), project_key.as_deref());
        if jql.is_some() {
            (jql_parts, jql_order)
        } else {
            let board_id = config.project.board_id;

            if let Some(bid) = board_id {
                match client.get_board_config(bid).await {
                    Ok(board_config) => {
                        let board_type = board_config.board_type.to_lowercase();
                        if board_type == "scrum" {
                            match client.list_sprints(bid, Some("active")).await {
                                Ok(sprints) if !sprints.is_empty() => {
                                    let sprint = &sprints[0];
                                    (vec![format!("sprint = {}", sprint.id)], "rank ASC")
                                }
                                _ => {
                                    let mut parts = Vec::new();
                                    if let Some(ref pk) = project_key {
                                        parts.push(format!(
                                            "project = \"{}\"",
                                            crate::jql::escape_value(pk)
                                        ));
                                    }
                                    (parts, "updated DESC")
                                }
                            }
                        } else {
                            // Kanban: statusCategory != Done, no implicit assignee
                            let mut parts = Vec::new();
                            if let Some(ref pk) = project_key {
                                parts.push(format!(
                                    "project = \"{}\"",
                                    crate::jql::escape_value(pk)
                                ));
                            }
                            parts.push("statusCategory != Done".into());
                            (parts, "rank ASC")
                        }
                    }
                    Err(_) => {
                        let mut parts = Vec::new();
                        if let Some(ref pk) = project_key {
                            parts.push(format!(
                                "project = \"{}\"",
                                crate::jql::escape_value(pk)
                            ));
                        }
                        (parts, "updated DESC")
                    }
                }
            } else {
                let mut parts = Vec::new();
                if let Some(ref pk) = project_key {
                    parts.push(format!(
                        "project = \"{}\"",
                        crate::jql::escape_value(pk)
                    ));
                }
                (parts, "updated DESC")
            }
        }
    };
```

Note: The `else` branch is structurally the same as before — it now references the hoisted `project_key` variable instead of calling `config.project_key(project_override)` inline.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All tests PASS (unit + integration). No behavior change for non-`--jql` paths.

- [ ] **Step 7: Run clippy and format**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no format issues.

- [ ] **Step 8: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "fix: compose --jql with --project scope using AND logic (#54)"
```

---

### Task 3: Add integration test for `--jql` + `--project` composition

**Files:**
- Modify: `tests/issue_commands.rs` (add import + new test at end of file)

**Context:** The existing integration tests call `client.search_issues(jql, ...)` with a pre-built JQL string. This test verifies the composed JQL is correctly sent in the POST body to the API using `body_partial_json` to match the `jql` field in the request body, ensuring the mock only matches if the correct composed JQL is sent.

- [ ] **Step 1: Add `body_partial_json` import**

In `tests/issue_commands.rs`, update the import line (line 4) from:

```rust
use wiremock::matchers::{method, path};
```

to:

```rust
use wiremock::matchers::{body_partial_json, method, path};
```

- [ ] **Step 2: Write the integration test**

Add at the end of `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_search_issues_jql_with_project_scope() {
    let server = MockServer::start().await;

    // The mock only matches if the POST body contains the expected composed JQL
    let expected_jql = r#"project = "PROJ" AND priority = Highest ORDER BY updated DESC"#;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": expected_jql
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1",
                "High priority issue",
                "To Do",
            )]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // This is the JQL that handle_list would compose when given
    // --project PROJ --jql "priority = Highest"
    let result = client.search_issues(expected_jql, Some(30), &[]).await.unwrap();
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].key, "PROJ-1");
}
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test --test issue_commands test_search_issues_jql_with_project_scope`
Expected: PASS — wiremock matches the POST body containing the composed JQL.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/issue_commands.rs
git commit -m "test: add integration test for --jql + --project composition (#54)"
```

---

### Task 4: Format and final verification

**Files:**
- All modified files from Tasks 1-3

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Zero warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt --all`

- [ ] **Step 4: If formatting changed anything, commit**

Run: `cargo fmt --all -- --check`
If it reports changes:
```bash
git add src/jql.rs src/cli/issue/list.rs tests/issue_commands.rs
git commit -m "style: format code"
```
