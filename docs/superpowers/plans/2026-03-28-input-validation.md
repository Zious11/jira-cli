# Input Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Validate `--project` and `--status` flags in `jr issue list` before building JQL, so invalid values produce actionable errors instead of silent empty results.

**Architecture:** Two pre-flight validation checks in `handle_list`: (1) project existence via `GET /rest/api/3/project/{key}`, (2) status name via partial_match against project-scoped or global status lists. When both flags are set, a single API call validates both. A new `resolved_status` variable carries the matched name into `build_filter_clauses`.

**Tech Stack:** Rust, serde, wiremock (tests), existing `partial_match` module

---

### Task 1: Add `project_exists()` API method

**Files:**
- Modify: `src/api/jira/projects.rs`

- [ ] **Step 1: Write the integration test**

Create `tests/input_validation.rs`:

```rust
#[allow(dead_code)]
mod common;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn project_exists_returns_true_for_valid_project() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10000",
            "key": "PROJ",
            "name": "My Project"
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    assert!(client.project_exists("PROJ").await.unwrap());
}

#[tokio::test]
async fn project_exists_returns_false_for_invalid_project() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/NOPE"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["No project could be found with key 'NOPE'."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    assert!(!client.project_exists("NOPE").await.unwrap());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test input_validation -- --nocapture`
Expected: FAIL — `project_exists` doesn't exist

- [ ] **Step 3: Implement `project_exists()`**

Add to `src/api/jira/projects.rs`, inside the `impl JiraClient` block (after `get_project_statuses`):

```rust
    /// Check whether a project with the given key exists.
    ///
    /// Returns `Ok(true)` if the project is accessible, `Ok(false)` if the API
    /// returns 404, and propagates any other error (auth, network, etc.).
    pub async fn project_exists(&self, key: &str) -> Result<bool> {
        let path = format!("/rest/api/3/project/{}", urlencoding::encode(key));
        match self.get::<serde_json::Value>(&path).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if let Some(crate::error::JrError::ApiError { status: 404, .. }) =
                    e.downcast_ref::<crate::error::JrError>()
                {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test input_validation -- --nocapture && cargo test --lib`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/api/jira/projects.rs tests/input_validation.rs
git commit -m "feat: add project_exists API method (#71)"
```

---

### Task 2: Add `get_all_statuses()` API method

**Files:**
- Create: `src/api/jira/statuses.rs`
- Modify: `src/api/jira/mod.rs`

- [ ] **Step 1: Write the integration test**

Add to `tests/input_validation.rs`:

```rust
#[tokio::test]
async fn get_all_statuses_returns_status_names() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": "1", "name": "To Do", "statusCategory": {"key": "new"}},
            {"id": "2", "name": "In Progress", "statusCategory": {"key": "indeterminate"}},
            {"id": "3", "name": "Done", "statusCategory": {"key": "done"}}
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses = client.get_all_statuses().await.unwrap();
    assert_eq!(statuses.len(), 3);
    assert!(statuses.contains(&"To Do".to_string()));
    assert!(statuses.contains(&"In Progress".to_string()));
    assert!(statuses.contains(&"Done".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test input_validation get_all_statuses -- --nocapture`
Expected: FAIL — `get_all_statuses` doesn't exist

- [ ] **Step 3: Create `src/api/jira/statuses.rs`**

```rust
use crate::api::client::JiraClient;
use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct StatusEntry {
    name: String,
}

impl JiraClient {
    /// Fetch all statuses from active workflows (global, not project-scoped).
    ///
    /// Returns a flat list of unique status names. The endpoint is not paginated.
    pub async fn get_all_statuses(&self) -> Result<Vec<String>> {
        let entries: Vec<StatusEntry> = self.get("/rest/api/3/status").await?;
        let names: Vec<String> = entries.into_iter().map(|e| e.name).collect();
        Ok(names)
    }
}
```

- [ ] **Step 4: Register the module in `src/api/jira/mod.rs`**

Add `pub mod statuses;` to `src/api/jira/mod.rs` (after `pub mod sprints;`).

- [ ] **Step 5: Run tests**

Run: `cargo test --test input_validation -- --nocapture && cargo test --lib`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/api/jira/statuses.rs src/api/jira/mod.rs tests/input_validation.rs
git commit -m "feat: add get_all_statuses API method (#71)"
```

---

### Task 3: Add `extract_unique_status_names()` helper and wire validation into `handle_list`

This is the core task — adds the validation logic, reorders `handle_list`, and introduces `resolved_status`.

**Files:**
- Modify: `src/cli/issue/list.rs`

- [ ] **Step 1: Write the unit test for `extract_unique_status_names`**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src/cli/issue/list.rs`:

```rust
    #[test]
    fn extract_unique_status_names_deduplicates_and_sorts() {
        use crate::api::jira::projects::{IssueTypeWithStatuses, StatusMetadata};
        let issue_types = vec![
            IssueTypeWithStatuses {
                id: "1".into(),
                name: "Task".into(),
                subtask: None,
                statuses: vec![
                    StatusMetadata { id: "10".into(), name: "To Do".into(), description: None },
                    StatusMetadata { id: "20".into(), name: "In Progress".into(), description: None },
                    StatusMetadata { id: "30".into(), name: "Done".into(), description: None },
                ],
            },
            IssueTypeWithStatuses {
                id: "2".into(),
                name: "Bug".into(),
                subtask: None,
                statuses: vec![
                    StatusMetadata { id: "10".into(), name: "To Do".into(), description: None },
                    StatusMetadata { id: "30".into(), name: "Done".into(), description: None },
                ],
            },
        ];
        let names = extract_unique_status_names(&issue_types);
        assert_eq!(names, vec!["Done", "In Progress", "To Do"]);
    }

    #[test]
    fn extract_unique_status_names_empty() {
        let names = extract_unique_status_names(&[]);
        assert!(names.is_empty());
    }
```

- [ ] **Step 2: Write `extract_unique_status_names`**

Add this function in `src/cli/issue/list.rs` (before `handle_list`, after the imports):

```rust
use crate::api::jira::projects::IssueTypeWithStatuses;

/// Extract unique status names from project-scoped statuses response (deduplicated, sorted).
fn extract_unique_status_names(issue_types: &[IssueTypeWithStatuses]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();
    for it in issue_types {
        for s in &it.statuses {
            if seen.insert(s.name.clone()) {
                names.push(s.name.clone());
            }
        }
    }
    names.sort();
    names
}
```

- [ ] **Step 3: Run unit tests**

Run: `cargo test --lib issue::list::tests -- --nocapture`
Expected: All pass (including the 2 new tests)

- [ ] **Step 4: Write integration tests for validation**

Add to `tests/input_validation.rs`:

```rust
fn project_statuses_response(statuses: Vec<&str>) -> serde_json::Value {
    let status_objects: Vec<serde_json::Value> = statuses
        .iter()
        .enumerate()
        .map(|(i, name)| {
            serde_json::json!({
                "id": format!("{}", i + 1),
                "name": name,
                "description": null
            })
        })
        .collect();
    serde_json::json!([{
        "id": "1",
        "name": "Task",
        "subtask": false,
        "statuses": status_objects
    }])
}

fn global_statuses_response(statuses: Vec<&str>) -> serde_json::Value {
    let entries: Vec<serde_json::Value> = statuses
        .iter()
        .enumerate()
        .map(|(i, name)| {
            serde_json::json!({
                "id": format!("{}", i + 1),
                "name": name,
                "statusCategory": {"key": "new"}
            })
        })
        .collect();
    serde_json::json!(entries)
}

#[tokio::test]
async fn invalid_status_with_project_returns_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ/statuses"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(project_statuses_response(vec!["To Do", "In Progress", "Done"])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses_response = client.get_project_statuses("PROJ").await.unwrap();

    // Extract unique names and test partial match
    let names: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        let mut n = Vec::new();
        for it in &statuses_response {
            for s in &it.statuses {
                if seen.insert(s.name.clone()) {
                    n.push(s.name.clone());
                }
            }
        }
        n.sort();
        n
    };

    let result = jr::partial_match::partial_match("Nonexistant", &names);
    assert!(matches!(result, jr::partial_match::MatchResult::None(_)));
}

#[tokio::test]
async fn valid_status_partial_match_resolves() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ/statuses"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(project_statuses_response(vec!["To Do", "In Progress", "Done"])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses_response = client.get_project_statuses("PROJ").await.unwrap();
    let names: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        let mut n = Vec::new();
        for it in &statuses_response {
            for s in &it.statuses {
                if seen.insert(s.name.clone()) {
                    n.push(s.name.clone());
                }
            }
        }
        n.sort();
        n
    };

    let result = jr::partial_match::partial_match("in prog", &names);
    match result {
        jr::partial_match::MatchResult::Exact(name) => assert_eq!(name, "In Progress"),
        other => panic!("Expected Exact, got {:?}", std::mem::discriminant(&other)),
    }
}

#[tokio::test]
async fn ambiguous_status_returns_multiple_matches() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ/statuses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            project_statuses_response(vec!["In Progress", "In Review", "Done"]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses_response = client.get_project_statuses("PROJ").await.unwrap();
    let names: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        let mut n = Vec::new();
        for it in &statuses_response {
            for s in &it.statuses {
                if seen.insert(s.name.clone()) {
                    n.push(s.name.clone());
                }
            }
        }
        n.sort();
        n
    };

    let result = jr::partial_match::partial_match("in", &names);
    match result {
        jr::partial_match::MatchResult::Ambiguous(matches) => {
            assert!(matches.contains(&"In Progress".to_string()));
            assert!(matches.contains(&"In Review".to_string()));
        }
        other => panic!("Expected Ambiguous, got {:?}", std::mem::discriminant(&other)),
    }
}

#[tokio::test]
async fn status_validation_with_global_statuses() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": "1", "name": "Open", "statusCategory": {"key": "new"}},
            {"id": "2", "name": "Closed", "statusCategory": {"key": "done"}}
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses = client.get_all_statuses().await.unwrap();

    let result = jr::partial_match::partial_match("Nonexistant", &statuses);
    assert!(matches!(result, jr::partial_match::MatchResult::None(_)));
}

#[tokio::test]
async fn project_statuses_404_means_project_not_found() {
    // Spec test 7: when both --project and --status are set, project statuses 404
    // should surface as a project-not-found error
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/NOPE/statuses"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["No project could be found with key 'NOPE'."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.get_project_statuses("NOPE").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    // The error should be a 404 ApiError
    assert!(err
        .downcast_ref::<jr::error::JrError>()
        .is_some_and(|e| matches!(e, jr::error::JrError::ApiError { status: 404, .. })));
}
```

- [ ] **Step 5: Wire validation into `handle_list`**

In `src/cli/issue/list.rs`, make the following changes to `handle_list`:

**5a.** Add imports at the top of the file (after existing imports):

```rust
use crate::partial_match::{self, MatchResult};
```

**5b.** Move `project_key` resolution up. Cut this line from its current position (line 108):
```rust
let project_key = config.project_key(project_override);
```
And paste it right after the `team_clause` block (after line 95), before the current `build_filter_clauses` call.

**5c.** Replace the `build_filter_clauses` call (currently lines 98-105) with the validation block + new `build_filter_clauses` call. The old code:
```rust
    // Build filter clauses from all flag values
    let filter_parts = build_filter_clauses(
        assignee_jql.as_deref(),
        reporter_jql.as_deref(),
        status.as_deref(),
        team_clause.as_deref(),
        recent.as_deref(),
        open,
    );
```

Replace with:

```rust
    // Validate --project exists
    if let Some(ref pk) = project_key {
        // Skip if --status is set (project will be validated via statuses endpoint below)
        if status.is_none() && !client.project_exists(pk).await? {
            return Err(JrError::UserError(format!(
                "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
                pk
            ))
            .into());
        }
    }

    // Validate --status and resolve to exact name
    let resolved_status: Option<String> = if let Some(ref status_input) = status {
        let valid_statuses = if let Some(ref pk) = project_key {
            // Project-scoped: also validates project existence (404 = not found)
            match client.get_project_statuses(pk).await {
                Ok(issue_types) => extract_unique_status_names(&issue_types),
                Err(e) => {
                    if let Some(JrError::ApiError { status: 404, .. }) =
                        e.downcast_ref::<JrError>()
                    {
                        return Err(JrError::UserError(format!(
                            "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
                            pk
                        ))
                        .into());
                    }
                    return Err(e);
                }
            }
        } else {
            client.get_all_statuses().await?
        };

        match partial_match::partial_match(status_input, &valid_statuses) {
            MatchResult::Exact(name) => Some(name),
            MatchResult::Ambiguous(matches) => {
                return Err(JrError::UserError(format!(
                    "Ambiguous status \"{}\". Matches: {}",
                    status_input,
                    matches.join(", ")
                ))
                .into());
            }
            MatchResult::None(all) => {
                let available = all.join(", ");
                let scope = if let Some(ref pk) = project_key {
                    format!(" for project {}", pk)
                } else {
                    String::new()
                };
                return Err(JrError::UserError(format!(
                    "No status matching \"{}\"{scope}. Available: {available}",
                    status_input,
                ))
                .into());
            }
        }
    } else {
        None
    };

    // Build filter clauses from all flag values
    let filter_parts = build_filter_clauses(
        assignee_jql.as_deref(),
        reporter_jql.as_deref(),
        resolved_status.as_deref(),
        team_clause.as_deref(),
        recent.as_deref(),
        open,
    );
```

**5d.** Remove the now-duplicate `let project_key = ...` line from its old position (which was after the `build_filter_clauses` call). It was already moved up in step 5b.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 7: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: Clean

- [ ] **Step 8: Commit**

```bash
git add src/cli/issue/list.rs tests/input_validation.rs
git commit -m "feat: validate --project and --status in issue list (#71)"
```

---

### Task 4: Update README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update the issue list command description**

In the Commands table, find:
```
| `jr issue list` | List issues (`--assignee`, `--reporter`, `--recent`, `--status`, `--open`, `--team`, `--jql`, `--limit`/`--all`, `--points`, `--assets`) |
```

The description already mentions `--status`. No change needed to the flag list — the validation is transparent to the user. The behavior change (errors instead of silent empty results) doesn't need a README flag update.

**Instead, add an example to Quick Start** after the `jr issue list --assignee me --open` line:

```bash
# Issues in a specific status
jr issue list --project FOO --status "In Progress"
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test && cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: All pass, clean

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add status filter example to README (#71)"
```
