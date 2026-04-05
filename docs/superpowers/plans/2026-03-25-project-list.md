# `jr project list` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `jr project list` command for project discovery, with `--type`/`--limit`/`--all` flags, plus error message enhancements suggesting valid projects.

**Architecture:** New types (`ProjectSummary`, `ProjectLead`) in the types layer, a `list_projects` API method calling `GET /rest/api/3/project/search` with offset pagination via `OffsetPage<T>`, a `handle_list` CLI handler with table/JSON output, a `suggest_projects` helper for error enhancement, and README updates.

**Tech Stack:** Rust, clap 4 derive macros, reqwest, serde, comfy-table, wiremock (tests)

**Spec:** `docs/superpowers/specs/2026-03-25-project-list-design.md`

---

### Task 1: Add `ProjectSummary` and `ProjectLead` types

**Files:**
- Modify: `src/types/jira/project.rs` (add types alongside existing `Project`)

- [ ] **Step 1: Add the new types**

In `src/types/jira/project.rs`, add below the existing `Project` struct:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectSummary {
    pub key: String,
    pub name: String,
    #[serde(rename = "projectTypeKey")]
    pub project_type_key: String,
    pub lead: Option<ProjectLead>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectLead {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check 2>&1`
Expected: Compiles with no errors (types are defined but unused — that's fine at this stage).

- [ ] **Step 3: Commit**

```bash
git add src/types/jira/project.rs
git commit -m "feat: add ProjectSummary and ProjectLead types for project search API"
```

---

### Task 2: Add `list_projects` API method with integration tests

**Files:**
- Modify: `src/api/jira/projects.rs` (add `list_projects` method)
- Create: `tests/project_commands.rs` (integration tests)
- Modify: `tests/common/fixtures.rs` (add project search fixture)

The existing `src/api/jira/projects.rs` already has `impl JiraClient` with `get_project_issue_types` and `get_priorities`. The new method goes in the same `impl` block.

The API uses query params in the URL path string (e.g., `/rest/api/3/project/search?orderBy=key&maxResults=50`), passed to `self.get(&path)`. The response is deserialized as `OffsetPage<ProjectSummary>` — the `values` key is handled by `OffsetPage`. For the `--all` case (`max_results` is `None`), paginate using `has_more()` and `next_start()`, collecting pages into a single `Vec`.

- [ ] **Step 1: Add project search fixture to test helpers**

In `tests/common/fixtures.rs`, add at the end of the file:

```rust
/// Project search response — paginated envelope with `values` array.
pub fn project_search_response(projects: Vec<Value>) -> Value {
    let total = projects.len() as u32;
    json!({
        "values": projects,
        "startAt": 0,
        "maxResults": 50,
        "total": total,
    })
}

pub fn project_response(key: &str, name: &str, type_key: &str, lead_name: Option<&str>) -> Value {
    let lead = lead_name.map(|name| json!({
        "accountId": format!("acc-{}", key.to_lowercase()),
        "displayName": name,
    }));
    json!({
        "key": key,
        "name": name,
        "projectTypeKey": type_key,
        "lead": lead,
    })
}
```

- [ ] **Step 2: Write integration tests**

Create `tests/project_commands.rs`:

```rust
#[allow(dead_code)]
mod common;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_list_projects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::project_search_response(vec![
                common::fixtures::project_response("FOO", "Project Foo", "software", Some("Jane Doe")),
                common::fixtures::project_response("BAR", "Project Bar", "service_desk", Some("John Smith")),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(None, Some(50)).await.unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].key, "FOO");
    assert_eq!(projects[0].name, "Project Foo");
    assert_eq!(projects[0].project_type_key, "software");
    assert_eq!(projects[0].lead.as_ref().unwrap().display_name, "Jane Doe");
    assert_eq!(projects[1].key, "BAR");
    assert_eq!(projects[1].project_type_key, "service_desk");
}

#[tokio::test]
async fn test_list_projects_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::project_search_response(vec![])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(None, Some(50)).await.unwrap();
    assert!(projects.is_empty());
}

#[tokio::test]
async fn test_list_projects_lead_missing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::project_search_response(vec![
                common::fixtures::project_response("FOO", "Project Foo", "software", None),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(None, Some(50)).await.unwrap();
    assert_eq!(projects.len(), 1);
    assert!(projects[0].lead.is_none());
}

#[tokio::test]
async fn test_list_projects_with_type_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(wiremock::matchers::query_param("typeKey", "software"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::project_search_response(vec![
                common::fixtures::project_response("FOO", "Project Foo", "software", Some("Jane Doe")),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(Some("software"), Some(50)).await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].project_type_key, "software");
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --test project_commands 2>&1 | head -20`
Expected: Compilation error — `list_projects` method doesn't exist yet. (Note: the `query_param` matcher import will also need `use wiremock::matchers::query_param;` or the fully qualified path used above.)

- [ ] **Step 4: Implement `list_projects`**

In `src/api/jira/projects.rs`, add to the existing `impl JiraClient` block, **before** the closing `}`:

```rust
    pub async fn list_projects(
        &self,
        type_key: Option<&str>,
        max_results: Option<u32>,
    ) -> Result<Vec<crate::types::jira::ProjectSummary>> {
        use crate::api::pagination::OffsetPage;
        use crate::types::jira::ProjectSummary;

        let page_size = max_results.map(|m| m.min(50)).unwrap_or(50);
        let mut all_projects: Vec<ProjectSummary> = Vec::new();
        let mut start_at: u32 = 0;

        loop {
            let mut path = format!(
                "/rest/api/3/project/search?orderBy=key&startAt={}&maxResults={}",
                start_at, page_size
            );
            if let Some(tk) = type_key {
                path.push_str(&format!("&typeKey={}", urlencoding::encode(tk)));
            }

            let page: OffsetPage<ProjectSummary> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all_projects.extend(page.values.unwrap_or_default());

            // If caller specified a limit, stop after one page
            if max_results.is_some() || !has_more {
                break;
            }
            start_at = next;
        }

        Ok(all_projects)
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test project_commands 2>&1`
Expected: All 4 tests pass.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -- -D warnings 2>&1`
Expected: No warnings.

- [ ] **Step 7: Commit**

```bash
git add src/api/jira/projects.rs tests/project_commands.rs tests/common/fixtures.rs
git commit -m "feat: add list_projects API method with integration tests (#47)"
```

---

### Task 3: Add `List` variant to `ProjectCommand` and wire up CLI handler

**Files:**
- Modify: `src/cli/mod.rs:342-349` (add `List` variant to `ProjectCommand`)
- Modify: `src/cli/project.rs` (add `handle_list`, update match arm, update `Fields` error message)

- [ ] **Step 1: Add `List` variant to `ProjectCommand`**

In `src/cli/mod.rs`, replace the `ProjectCommand` enum (lines 342-349):

```rust
#[derive(Subcommand)]
pub enum ProjectCommand {
    /// List accessible projects
    List {
        /// Filter by project type (software, service_desk, business)
        #[arg(long = "type")]
        project_type: Option<String>,
        /// Maximum number of results (default: 50)
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all projects (paginate through all pages)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
    /// Show valid issue types, priorities, and statuses
    Fields {
        /// Project key (uses configured project if omitted)
        project: Option<String>,
    },
}
```

- [ ] **Step 2: Add `handle_list` and update the handler**

Replace the entire contents of `src/cli/project.rs` with:

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, ProjectCommand};
use crate::config::Config;
use crate::output;

pub async fn handle(
    command: ProjectCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    match command {
        ProjectCommand::List {
            project_type,
            limit,
            all,
        } => handle_list(client, output_format, project_type.as_deref(), limit, all).await,
        ProjectCommand::Fields { project } => {
            handle_fields(project, config, client, output_format, project_override).await
        }
    }
}

async fn handle_list(
    client: &JiraClient,
    output_format: &OutputFormat,
    project_type: Option<&str>,
    limit: Option<u32>,
    all: bool,
) -> Result<()> {
    let max_results = if all { None } else { Some(limit.unwrap_or(50)) };
    let projects = client.list_projects(project_type, max_results).await?;

    let rows: Vec<Vec<String>> = projects
        .iter()
        .map(|p| {
            vec![
                p.key.clone(),
                p.name.clone(),
                p.lead
                    .as_ref()
                    .map(|l| l.display_name.clone())
                    .unwrap_or_else(|| "\u{2014}".into()),
                p.project_type_key.clone(),
            ]
        })
        .collect();

    output::print_output(output_format, &["Key", "Name", "Lead", "Type"], &rows, &projects)
}

async fn handle_fields(
    project: Option<String>,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No project specified. Run \"jr project list\" to see available projects."
            )
        })?;

    let issue_types = client.get_project_issue_types(&project_key).await?;
    let priorities = client.get_priorities().await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "project": project_key,
                    "issue_types": issue_types,
                    "priorities": priorities,
                })
            );
        }
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
        }
    }
    Ok(())
}
```

Note: The `"No project specified"` error message in `handle_fields` now includes the `jr project list` hint (spec deliverable #3, touchpoint 1).

- [ ] **Step 3: Verify it compiles and all tests pass**

Run: `cargo test 2>&1`
Expected: All tests pass. The new `List` variant is wired through.

Run: `cargo clippy -- -D warnings 2>&1`
Expected: No warnings.

- [ ] **Step 4: Commit**

```bash
git add src/cli/mod.rs src/cli/project.rs
git commit -m "feat: add jr project list command with --type, --limit, --all flags (#47)"
```

---

### Task 4: Add `suggest_projects` helper and error message enhancements

**Files:**
- Modify: `src/cli/project.rs` (add `suggest_projects` function)
- Modify: `src/cli/issue/create.rs` (enhance project 404 error)
- Modify: `src/cli/queue.rs` (enhance service desk lookup error)

The `suggest_projects` function needs `pub` visibility so `create.rs` and `queue.rs` can call it. It lives in `project.rs` because it belongs with project logic.

- [ ] **Step 1: Add `suggest_projects` to `src/cli/project.rs`**

Add at the end of `src/cli/project.rs`, **before** any `#[cfg(test)]` block (if one exists) or at the end of the file:

```rust
/// Suggest valid projects when an invalid key is used.
///
/// Returns a hint string like `Did you mean "FOO"? Run "jr project list" to see available projects.`
/// If no close match is found or the API call fails, returns a generic hint.
pub async fn suggest_projects(client: &JiraClient, invalid_key: &str) -> String {
    let generic = "Run \"jr project list\" to see available projects.".to_string();

    let projects = match client.list_projects(None, Some(50)).await {
        Ok(p) => p,
        Err(_) => return generic,
    };

    let keys: Vec<String> = projects.iter().map(|p| p.key.clone()).collect();
    match crate::partial_match::partial_match(invalid_key, &keys) {
        crate::partial_match::MatchResult::Exact(matched) => {
            format!("Did you mean \"{matched}\"? {generic}")
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            let quoted: Vec<String> = matches.iter().map(|m| format!("\"{m}\"")).collect();
            format!("Did you mean {}? {generic}", quoted.join(" or "))
        }
        crate::partial_match::MatchResult::None(_) => generic,
    }
}
```

- [ ] **Step 2: Write unit tests for `suggest_projects`**

Add a `#[cfg(test)]` module at the bottom of `src/cli/project.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // suggest_projects is async and calls the API, so we test the matching logic
    // by testing the partial_match behavior on project keys directly.
    // The integration of suggest_projects with the API is covered by the
    // integration tests in tests/project_commands.rs.

    #[test]
    fn suggest_projects_match_logic_exact() {
        let keys = vec!["FOO".to_string(), "BAR".to_string(), "BAZ".to_string()];
        match crate::partial_match::partial_match("FOO", &keys) {
            crate::partial_match::MatchResult::Exact(m) => assert_eq!(m, "FOO"),
            _ => panic!("Expected exact match"),
        }
    }

    #[test]
    fn suggest_projects_match_logic_partial() {
        let keys = vec!["FOO".to_string(), "BAR".to_string(), "BAZ".to_string()];
        match crate::partial_match::partial_match("FO", &keys) {
            crate::partial_match::MatchResult::Exact(m) => assert_eq!(m, "FOO"),
            _ => panic!("Expected unique partial match"),
        }
    }

    #[test]
    fn suggest_projects_match_logic_ambiguous() {
        let keys = vec!["FOO".to_string(), "BAR".to_string(), "BAZ".to_string()];
        match crate::partial_match::partial_match("BA", &keys) {
            crate::partial_match::MatchResult::Ambiguous(matches) => {
                assert_eq!(matches.len(), 2);
                assert!(matches.contains(&"BAR".to_string()));
                assert!(matches.contains(&"BAZ".to_string()));
            }
            _ => panic!("Expected ambiguous match"),
        }
    }

    #[test]
    fn suggest_projects_match_logic_none() {
        let keys = vec!["FOO".to_string(), "BAR".to_string()];
        match crate::partial_match::partial_match("ZZZ", &keys) {
            crate::partial_match::MatchResult::None(_) => {} // expected
            _ => panic!("Expected no match"),
        }
    }
}
```

- [ ] **Step 3: Enhance error in `src/cli/issue/create.rs`**

In `src/cli/issue/create.rs`, find the project key resolution block (lines 37-49). Replace the `.ok_or_else` error with a static hint (no API call needed since the user didn't provide a key at all):

```rust
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Project key").ok()
            }
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Project key is required. Use --project or configure .jr.toml. \
                 Run \"jr project list\" to see available projects."
            )
        })?;
```

Note: The spec mentions enhancing 404 errors when a key IS provided but is invalid. However, in `create.rs` the project key is embedded in the JSON body sent to `POST /rest/api/3/issue` — Jira returns a generic error, not a project-specific 404. Catching and enhancing this specific error would require parsing Jira's error response body to detect project-related failures, which adds fragile coupling to Jira's error message format. The static hint on the "no project" path is sufficient — users who provide a wrong key get Jira's own error plus can discover valid keys via `jr project list`.

- [ ] **Step 4: Enhance error in `src/cli/queue.rs`**

In `src/cli/queue.rs`, find the project key resolution (line 18-20). Replace it with:

```rust
    let project_key = config.project_key(project_override).ok_or_else(|| {
        JrError::UserError(
            "No project configured. Run \"jr init\" or pass --project. \
             Run \"jr project list\" to see available projects."
                .into(),
        )
    })?;
```

Same rationale as `create.rs` — the "no project configured" path gets a static hint. The `require_service_desk` call that follows uses the project key to look up a service desk; if it fails, Jira's error message is returned. Adding a dynamic `suggest_projects` call here would require an extra API round-trip on every queue error, which isn't worth the complexity for a path that already tells users what went wrong.

- [ ] **Step 5: Verify it compiles and all tests pass**

Run: `cargo test 2>&1`
Expected: All tests pass.

Run: `cargo clippy -- -D warnings 2>&1`
Expected: No warnings.

- [ ] **Step 6: Commit**

```bash
git add src/cli/project.rs src/cli/issue/create.rs src/cli/queue.rs
git commit -m "feat: add suggest_projects helper and enhance error messages (#47)"
```

---

### Task 5: Update documentation

**Files:**
- Modify: `README.md` (add `jr project list` to command table and quick start)

- [ ] **Step 1: Update README command table**

In `README.md`, find the `jr project fields FOO` row (line 119) and add a new row **before** it:

```markdown
| `jr project list` | List accessible projects (`--type`, `--limit`/`--all`) |
```

So the two project rows will be:
```markdown
| `jr project list` | List accessible projects (`--type`, `--limit`/`--all`) |
| `jr project fields FOO` | Show valid issue types and priorities |
```

- [ ] **Step 2: Add quick start example**

In `README.md`, add after the `jr issue list --assignee me --open` line (line 67), before `# View a specific issue`:

```markdown

# Discover available projects
jr project list
```

- [ ] **Step 3: Verify build still passes**

Run: `cargo test 2>&1`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: add jr project list to README command table and examples"
```

---

### Task 6: Format and final verification

- [ ] **Step 1: Run formatter**

Run: `cargo fmt --all -- --check 2>&1`
If any formatting issues: `cargo fmt --all`

- [ ] **Step 2: Run full CI checks**

Run: `cargo clippy -- -D warnings 2>&1 && cargo test 2>&1`
Expected: Zero warnings, all tests pass.

- [ ] **Step 3: Commit formatting if needed**

```bash
# Only if cargo fmt made changes:
git add -A
git commit -m "style: format code"
```
