# Add Statuses to `project fields` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix bug #55 — add project statuses grouped by issue type to `jr project fields` output (table and JSON).

**Architecture:** Add `get_project_statuses` API method calling `/rest/api/3/project/{key}/statuses`, then update `handle_fields` CLI handler to fetch and render statuses in both table and JSON formats.

**Tech Stack:** Rust, serde, wiremock (tests)

**Spec:** `docs/superpowers/specs/2026-03-25-project-fields-statuses-design.md`

---

### Task 1: Add `get_project_statuses` API method with integration test

**Files:**
- Modify: `src/api/jira/projects.rs:1-72` (add structs after line 19, add method inside `impl JiraClient` block)
- Modify: `tests/common/fixtures.rs:204` (add fixture at end of file)
- Modify: `tests/project_commands.rs:157` (add test at end of file)

- [ ] **Step 1: Add the fixture helper**

In `tests/common/fixtures.rs`, add at the end of the file (after the `project_response` function, before the final newline):

```rust
/// Project statuses response — top-level array of issue types with nested statuses.
pub fn project_statuses_response() -> Value {
    json!([
        {
            "id": "3",
            "name": "Task",
            "self": "https://test.atlassian.net/rest/api/3/issueType/3",
            "subtask": false,
            "statuses": [
                {
                    "id": "10000",
                    "name": "To Do",
                    "description": "Work that has not been started.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/open.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10000"
                },
                {
                    "id": "10001",
                    "name": "In Progress",
                    "description": "The issue is currently being worked on.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/inprogress.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10001"
                },
                {
                    "id": "10002",
                    "name": "Done",
                    "description": "Work has been completed.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/closed.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10002"
                }
            ]
        },
        {
            "id": "1",
            "name": "Bug",
            "self": "https://test.atlassian.net/rest/api/3/issueType/1",
            "subtask": false,
            "statuses": [
                {
                    "id": "10000",
                    "name": "To Do",
                    "description": "Work that has not been started.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/open.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10000"
                },
                {
                    "id": "10002",
                    "name": "Done",
                    "description": "Work has been completed.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/closed.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10002"
                }
            ]
        }
    ])
}
```

- [ ] **Step 2: Write the failing integration test**

In `tests/project_commands.rs`, add at the end of the file:

```rust
#[tokio::test]
async fn test_get_project_statuses() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/statuses"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::project_statuses_response()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.get_project_statuses("FOO").await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].name, "Task");
    assert_eq!(result[0].id, "3");
    assert_eq!(result[0].subtask, Some(false));
    assert_eq!(result[0].statuses.len(), 3);
    assert_eq!(result[0].statuses[0].name, "To Do");
    assert_eq!(result[0].statuses[0].id, "10000");
    assert_eq!(
        result[0].statuses[0].description.as_deref(),
        Some("Work that has not been started.")
    );
    assert_eq!(result[0].statuses[1].name, "In Progress");
    assert_eq!(result[0].statuses[2].name, "Done");
    assert_eq!(result[1].name, "Bug");
    assert_eq!(result[1].statuses.len(), 2);
}

#[tokio::test]
async fn test_get_project_statuses_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/statuses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.get_project_statuses("FOO").await.unwrap();
    assert!(result.is_empty());
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --test project_commands test_get_project_statuses`
Expected: FAIL — both `test_get_project_statuses` and `test_get_project_statuses_empty` fail because `get_project_statuses` method does not exist yet.

- [ ] **Step 4: Add serde structs and API method**

In `src/api/jira/projects.rs`, add the two new structs after `PriorityMetadata` and before the `impl JiraClient` block:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct StatusMetadata {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueTypeWithStatuses {
    pub id: String,
    pub name: String,
    pub subtask: Option<bool>,
    pub statuses: Vec<StatusMetadata>,
}
```

Then add the new method inside the `impl JiraClient` block, after `get_priorities` and before `list_projects`:

```rust
    pub async fn get_project_statuses(
        &self,
        project_key: &str,
    ) -> Result<Vec<IssueTypeWithStatuses>> {
        self.get(&format!("/rest/api/3/project/{project_key}/statuses"))
            .await
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --test project_commands test_get_project_statuses`
Expected: Both `test_get_project_statuses` and `test_get_project_statuses_empty` PASS.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All tests PASS (no regressions).

- [ ] **Step 7: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no format issues.

- [ ] **Step 8: Commit**

```bash
git add src/api/jira/projects.rs tests/common/fixtures.rs tests/project_commands.rs
git commit -m "feat: add get_project_statuses API method (#55)"
```

---

### Task 2: Update `handle_fields` to fetch and render statuses

**Files:**
- Modify: `src/cli/project.rs` (the `handle_fields` function)

**Context:** The `handle_fields` function currently fetches issue types and priorities, then renders them in table or JSON format. We need to add a third fetch for statuses, then render them in both output formats. The spec requires:
- Table: "Statuses by Issue Type" section after Priorities, skip issue types with empty statuses, omit section if all empty
- JSON: `statuses_by_issue_type` field containing the full `Vec<IssueTypeWithStatuses>`
- Error handling: `?` propagation (hard fail), not `unwrap_or_default()`

**Note on testing:** No handler-level test is added for the rendering changes. The handler output is `println!` to stdout — testing it would require stdout capture, which is not done anywhere in this codebase. The API-layer test in Task 1 covers deserialization correctness. The rendering logic is trivial iteration; visual correctness will be verified during live testing.

- [ ] **Step 1: Add the statuses fetch**

In `src/cli/project.rs`, in the `handle_fields` function, add after `let priorities = client.get_priorities().await?;`:

```rust
    let statuses = client.get_project_statuses(&project_key).await?;
```

- [ ] **Step 2: Update the JSON output**

In `src/cli/project.rs`, replace the JSON branch (the `OutputFormat::Json` arm) with:

```rust
            println!(
                "{}",
                serde_json::json!({
                    "project": project_key,
                    "issue_types": issue_types,
                    "priorities": priorities,
                    "statuses_by_issue_type": statuses,
                })
            );
```

- [ ] **Step 3: Update the table output**

In `src/cli/project.rs`, replace the Table branch (the `OutputFormat::Table` arm) with:

```rust
        OutputFormat::Table => {
            println!("Project: {project_key}\n");
            println!("Issue Types:");
            for t in &issue_types {
                let suffix = if t.subtask == Some(true) {
                    " (subtask)"
                } else {
                    ""
                };
                println!("  - {}{}", t.name, suffix);
            }
            println!("\nPriorities:");
            for p in &priorities {
                println!("  - {}", p.name);
            }
            let has_statuses = statuses.iter().any(|it| !it.statuses.is_empty());
            if has_statuses {
                println!("\nStatuses by Issue Type:");
                for it in &statuses {
                    if it.statuses.is_empty() {
                        continue;
                    }
                    println!("  {}:", it.name);
                    for s in &it.statuses {
                        println!("    - {}", s.name);
                    }
                }
            }
        }
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 5: Run clippy and format**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no format issues.

- [ ] **Step 6: Commit**

```bash
git add src/cli/project.rs
git commit -m "fix: add statuses to project fields output (#55)"
```

---

### Task 3: Format and final verification

**Files:**
- All modified files from Tasks 1-2

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
git add src/api/jira/projects.rs src/cli/project.rs tests/common/fixtures.rs tests/project_commands.rs
git commit -m "style: format code"
```
