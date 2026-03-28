# Board Auto-Resolve Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-resolve board IDs from project keys so `jr sprint list --project PROJ` just works, and add `--type` filter to `jr board list`.

**Architecture:** Three layers of change: (1) types — add `BoardLocation` struct, (2) API — add filter params to `list_boards()`, (3) CLI — add `resolve_board_id()` helper, wire it into sprint/board handlers, add `--type` to board list, thread global `--project` flag.

**Tech Stack:** Rust, clap (derive), serde, wiremock (tests), tokio

---

### Task 1: Add `BoardLocation` type and update `Board` struct

**Files:**
- Modify: `src/types/jira/board.rs`

- [ ] **Step 1: Write the unit test for `BoardLocation` deserialization**

Add a test module at the bottom of `src/types/jira/board.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_deserializes_with_location() {
        let json = r#"{
            "id": 42,
            "name": "My Board",
            "type": "scrum",
            "location": {
                "projectKey": "PROJ",
                "projectName": "My Project"
            }
        }"#;
        let board: Board = serde_json::from_str(json).unwrap();
        assert_eq!(board.id, 42);
        assert_eq!(board.board_type, "scrum");
        let loc = board.location.unwrap();
        assert_eq!(loc.project_key.as_deref(), Some("PROJ"));
        assert_eq!(loc.project_name.as_deref(), Some("My Project"));
    }

    #[test]
    fn board_deserializes_without_location() {
        let json = r#"{
            "id": 99,
            "name": "No Location Board",
            "type": "kanban"
        }"#;
        let board: Board = serde_json::from_str(json).unwrap();
        assert_eq!(board.id, 99);
        assert!(board.location.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib board::tests -- --nocapture`
Expected: FAIL — `Board` struct has no `location` field

- [ ] **Step 3: Implement `BoardLocation` and update `Board`**

Replace the entire content of `src/types/jira/board.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Board {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type")]
    pub board_type: String,
    #[serde(default)]
    pub location: Option<BoardLocation>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct BoardLocation {
    #[serde(default, rename = "projectKey")]
    pub project_key: Option<String>,
    #[serde(default, rename = "projectName")]
    pub project_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BoardConfig {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type", default)]
    pub board_type: String,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib board::tests -- --nocapture`
Expected: PASS (2 tests)

- [ ] **Step 5: Run full test suite to check nothing broke**

Run: `cargo test`
Expected: All tests pass — existing code ignores the new `location` field

- [ ] **Step 6: Commit**

```bash
git add src/types/jira/board.rs
git commit -m "feat: add BoardLocation type to Board struct (#70)"
```

---

### Task 2: Add filter parameters to `list_boards()` API

**Files:**
- Modify: `src/api/jira/boards.rs`

- [ ] **Step 1: Write the integration test for filtered board listing**

Create `tests/board_commands.rs`:

```rust
#[allow(dead_code)]
mod common;

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn board_response(id: u64, name: &str, board_type: &str, project_key: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": name,
        "type": board_type,
        "location": {
            "projectKey": project_key,
            "projectName": format!("{} Project", project_key)
        }
    })
}

fn board_list_response(boards: Vec<serde_json::Value>) -> serde_json::Value {
    let total = boards.len() as u32;
    serde_json::json!({
        "values": boards,
        "startAt": 0,
        "maxResults": 50,
        "total": total
    })
}

#[tokio::test]
async fn list_boards_with_project_and_type_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            board_list_response(vec![
                board_response(42, "My Board", "scrum", "PROJ"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let boards = client.list_boards(Some("PROJ"), Some("scrum")).await.unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, 42);
    assert_eq!(boards[0].name, "My Board");
}

#[tokio::test]
async fn list_boards_without_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            board_list_response(vec![
                board_response(1, "Board A", "scrum", "FOO"),
                board_response(2, "Board B", "kanban", "BAR"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let boards = client.list_boards(None, None).await.unwrap();
    assert_eq!(boards.len(), 2);
}

#[tokio::test]
async fn list_boards_empty_result() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "NOPE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            board_list_response(vec![]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let boards = client.list_boards(Some("NOPE"), None).await.unwrap();
    assert!(boards.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test board_commands -- --nocapture`
Expected: FAIL — `list_boards` doesn't accept arguments

- [ ] **Step 3: Implement filtered `list_boards()`**

Replace the `list_boards` method in `src/api/jira/boards.rs`:

```rust
use crate::api::client::JiraClient;
use crate::api::pagination::OffsetPage;
use crate::types::jira::{Board, BoardConfig};
use anyhow::Result;

impl JiraClient {
    /// List boards accessible to the authenticated user, optionally filtered.
    pub async fn list_boards(
        &self,
        project_key: Option<&str>,
        board_type: Option<&str>,
    ) -> Result<Vec<Board>> {
        let mut all_boards: Vec<Board> = Vec::new();
        let mut start_at: u32 = 0;
        let max_results: u32 = 50;

        loop {
            let mut path = format!(
                "/rest/agile/1.0/board?startAt={}&maxResults={}",
                start_at, max_results
            );
            if let Some(pk) = project_key {
                path.push_str(&format!("&projectKeyOrId={pk}"));
            }
            if let Some(bt) = board_type {
                path.push_str(&format!("&type={bt}"));
            }
            let page: OffsetPage<Board> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all_boards.extend(page.values.unwrap_or_default());

            if !has_more {
                break;
            }
            start_at = next;
        }

        Ok(all_boards)
    }

    /// Get the configuration for a specific board.
    pub async fn get_board_config(&self, board_id: u64) -> Result<BoardConfig> {
        let path = format!("/rest/agile/1.0/board/{}/configuration", board_id);
        self.get(&path).await
    }
}
```

- [ ] **Step 4: Fix the only existing caller — `board.rs` `handle_list`**

In `src/cli/board.rs`, change line 23 from:
```rust
let boards = client.list_boards().await?;
```
to:
```rust
let boards = client.list_boards(None, None).await?;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test board_commands -- --nocapture && cargo test --lib`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/api/jira/boards.rs src/cli/board.rs tests/board_commands.rs
git commit -m "feat: add project and type filters to list_boards API (#70)"
```

---

### Task 3: Add `resolve_board_id()` helper and wire into sprint/board handlers

This is the core task. It adds the auto-resolve helper, threads `--project` through main.rs, updates both handlers, and adds `--type` to `BoardCommand::List`.

**Files:**
- Modify: `src/cli/mod.rs` (BoardCommand::List enum variant)
- Modify: `src/cli/board.rs` (add resolve_board_id, update handle/handle_list/handle_view)
- Modify: `src/cli/sprint.rs` (update handle to use resolve_board_id)
- Modify: `src/main.rs` (thread cli.project to board/sprint)

- [ ] **Step 1: Write integration tests for auto-resolve scenarios**

Add to `tests/board_commands.rs`:

```rust
fn board_config_response(id: u64, board_type: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": "Board Config",
        "type": board_type
    })
}

fn sprint_list_response(sprints: Vec<serde_json::Value>) -> serde_json::Value {
    let total = sprints.len() as u32;
    serde_json::json!({
        "values": sprints,
        "startAt": 0,
        "maxResults": 50,
        "total": total
    })
}

fn sprint_response(id: u64, name: &str, state: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": name,
        "state": state,
        "startDate": "2026-03-20T00:00:00.000Z",
        "endDate": "2026-04-03T00:00:00.000Z"
    })
}

fn sprint_issues_response(issues: Vec<serde_json::Value>) -> serde_json::Value {
    let total = issues.len() as u32;
    serde_json::json!({
        "issues": issues,
        "startAt": 0,
        "maxResults": 50,
        "total": total
    })
}

#[tokio::test]
async fn resolve_board_auto_discovers_single_scrum_board() {
    let server = MockServer::start().await;

    // list_boards filtered by project+scrum returns 1 board
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            board_list_response(vec![
                board_response(42, "PROJ Scrum Board", "scrum", "PROJ"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    // resolve_board_id with no --board, no config, but project_override="PROJ" and require_scrum=true
    let board_id =
        jr::cli::board::resolve_board_id(&config, &client, None, Some("PROJ"), true)
            .await
            .unwrap();
    assert_eq!(board_id, 42);
}

#[tokio::test]
async fn resolve_board_errors_on_multiple_boards() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            board_list_response(vec![
                board_response(42, "Board A", "scrum", "PROJ"),
                board_response(99, "Board B", "scrum", "PROJ"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    let err = jr::cli::board::resolve_board_id(&config, &client, None, Some("PROJ"), true)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Multiple scrum boards"), "got: {msg}");
    assert!(msg.contains("42"), "should list board ID 42, got: {msg}");
    assert!(msg.contains("99"), "should list board ID 99, got: {msg}");
}

#[tokio::test]
async fn resolve_board_errors_on_no_boards() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "NOPE"))
        .and(query_param("type", "scrum"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(board_list_response(vec![])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    let err = jr::cli::board::resolve_board_id(&config, &client, None, Some("NOPE"), true)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("No scrum boards found"), "got: {msg}");
    assert!(msg.contains("NOPE"), "should mention project key, got: {msg}");
}

#[tokio::test]
async fn resolve_board_uses_explicit_board_override() {
    // No server mocks needed — resolve_board_id should return immediately
    let server = MockServer::start().await;
    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    let board_id =
        jr::cli::board::resolve_board_id(&config, &client, Some(42), None, true)
            .await
            .unwrap();
    assert_eq!(board_id, 42);
    // No HTTP requests should have been made
}

#[tokio::test]
async fn resolve_board_errors_without_project_or_board() {
    let server = MockServer::start().await;
    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let config = jr::config::Config::default();

    let err = jr::cli::board::resolve_board_id(&config, &client, None, None, true)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("No board configured"), "got: {msg}");
    assert!(msg.contains("--project"), "should suggest --project, got: {msg}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test board_commands resolve_board -- --nocapture`
Expected: FAIL — `resolve_board_id` doesn't exist

- [ ] **Step 3: Update `BoardCommand::List` to struct variant with `--type`**

In `src/cli/mod.rs`, replace lines 361-370:

```rust
#[derive(Subcommand)]
pub enum BoardCommand {
    /// List boards
    List {
        /// Filter by board type
        #[arg(long = "type", value_parser = clap::builder::PossibleValuesParser::new(["scrum", "kanban"]))]
        board_type: Option<String>,
    },
    /// View current board issues
    View {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
    },
}
```

- [ ] **Step 4: Thread `cli.project` to board and sprint handlers in `main.rs`**

In `src/main.rs`, change line 129 from:
```rust
cli::board::handle(command, &config, &client, &cli.output).await
```
to:
```rust
cli::board::handle(command, &config, &client, &cli.output, cli.project.as_deref()).await
```

Change line 134 from:
```rust
cli::sprint::handle(command, &config, &client, &cli.output).await
```
to:
```rust
cli::sprint::handle(command, &config, &client, &cli.output, cli.project.as_deref()).await
```

- [ ] **Step 5: Implement `resolve_board_id()` and update board handlers**

Replace the full content of `src/cli/board.rs`. Note: this removes the `missing_board_id_returns_config_error` unit test, which is now covered by the `resolve_board_errors_without_project_or_board` integration test added in Step 1.

```rust
use anyhow::{Result, bail};

use crate::api::client::JiraClient;
use crate::cli::{BoardCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;

/// Resolve a board ID from CLI override, config, or auto-discovery.
///
/// Resolution order:
/// 1. CLI `--board` override
/// 2. Config `board_id` from `.jr.toml`
/// 3. Auto-discover via Jira API using project key
pub async fn resolve_board_id(
    config: &Config,
    client: &JiraClient,
    board_override: Option<u64>,
    project_override: Option<&str>,
    require_scrum: bool,
) -> Result<u64> {
    // Step 1: CLI override
    if let Some(id) = board_override {
        return Ok(id);
    }

    // Step 2: Config
    if let Some(id) = config.project.board_id {
        return Ok(id);
    }

    // Step 3: Auto-discover
    let project_key = config.project_key(project_override).ok_or_else(|| {
        JrError::ConfigError(
            "No board configured and no project specified. \
             Use --board <ID>, set board_id in .jr.toml, or specify --project to auto-discover."
                .into(),
        )
    })?;

    let type_filter = if require_scrum { Some("scrum") } else { None };
    let boards = client.list_boards(Some(&project_key), type_filter).await?;

    match boards.len() {
        0 => {
            let board_kind = if require_scrum { "scrum boards" } else { "boards" };
            bail!(
                "No {} found for project {}. \
                 Verify the project key is correct, then try \"jr board list --project {}\".",
                board_kind,
                project_key,
                project_key,
            );
        }
        1 => {
            let board = &boards[0];
            eprintln!("Using board {} - {} ({})", board.id, board.name, board.board_type);
            Ok(board.id)
        }
        _ => {
            let board_kind = if require_scrum { "scrum boards" } else { "boards" };
            let mut msg = format!("Multiple {} found for project {}:\n", board_kind, project_key);
            for b in &boards {
                if require_scrum {
                    msg.push_str(&format!("  {}  {}\n", b.id, b.name));
                } else {
                    msg.push_str(&format!("  {}  {}  {}\n", b.id, b.board_type, b.name));
                }
            }
            msg.push_str("Use --board <ID> to select one, or set board_id in .jr.toml.");
            bail!("{}", msg);
        }
    }
}

/// Handle all board subcommands.
pub async fn handle(
    command: BoardCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    match command {
        BoardCommand::List { board_type } => {
            handle_list(client, output_format, project_override, board_type.as_deref()).await
        }
        BoardCommand::View { board } => {
            handle_view(config, client, output_format, board, project_override).await
        }
    }
}

async fn handle_list(
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
    board_type_filter: Option<&str>,
) -> Result<()> {
    let boards = client.list_boards(project_override, board_type_filter).await?;

    let rows: Vec<Vec<String>> = boards
        .iter()
        .map(|b| {
            let project = b
                .location
                .as_ref()
                .and_then(|loc| loc.project_key.as_deref())
                .unwrap_or("-");
            vec![
                b.id.to_string(),
                b.board_type.clone(),
                project.to_string(),
                b.name.clone(),
            ]
        })
        .collect();

    output::print_output(output_format, &["ID", "Type", "Project", "Name"], &rows, &boards)?;

    Ok(())
}

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

async fn handle_view(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    board_override: Option<u64>,
    project_override: Option<&str>,
) -> Result<()> {
    let board_id = resolve_board_id(config, client, board_override, project_override, false).await?;

    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();

    let issues = if board_type == "scrum" {
        // For scrum boards, fetch the active sprint's issues
        let sprints = client.list_sprints(board_id, Some("active")).await?;
        if sprints.is_empty() {
            bail!("No active sprint found for board {}.", board_id);
        }
        let sprint = &sprints[0];
        client.get_sprint_issues(sprint.id, None, &[]).await?
    } else {
        // Kanban: search for issues not in Done status category
        let project_key = config.project_key(project_override);
        if project_key.is_none() {
            eprintln!(
                "warning: no project configured for board. Showing issues across all projects. Set project in .jr.toml to scope results."
            );
        }
        let jql = build_kanban_jql(project_key.as_deref());
        client.search_issues(&jql, None, &[]).await?.issues
    };

    let rows = super::issue::format_issue_rows_public(&issues);

    output::print_output(
        output_format,
        &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
        &rows,
        &issues,
    )?;

    Ok(())
}

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

- [ ] **Step 6: Update sprint handler to use `resolve_board_id()`**

Replace `src/cli/sprint.rs` handler function (lines 10-47) with:

```rust
/// Handle all sprint subcommands.
pub async fn handle(
    command: SprintCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    let board_override = match &command {
        SprintCommand::List { board } => *board,
        SprintCommand::Current { board } => *board,
    };

    let board_id =
        crate::cli::board::resolve_board_id(config, client, board_override, project_override, true)
            .await?;

    // Guard: sprints only make sense for scrum boards.
    // When resolve_board_id auto-discovers (step 3), it already filters to scrum.
    // This guard catches the case where --board or config provides a kanban board directly.
    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();
    if board_type != "scrum" {
        bail!(
            "Sprint commands are only available for scrum boards. Board {} is a {} board.",
            board_id,
            board_config.board_type
        );
    }

    match command {
        SprintCommand::List { .. } => handle_list(board_id, client, output_format).await,
        SprintCommand::Current { .. } => {
            handle_current(board_id, client, output_format, config).await
        }
    }
}
```

- [ ] **Step 7: Run all tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 8: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: Clean

- [ ] **Step 9: Commit**

```bash
git add src/cli/mod.rs src/cli/board.rs src/cli/sprint.rs src/main.rs tests/board_commands.rs
git commit -m "feat: add board auto-resolve with --project and --type filters (#70)"
```

---

### Task 4: Add test fixtures to `tests/common/fixtures.rs`

Move the inline fixture helpers from `tests/board_commands.rs` into the shared fixtures file for reuse.

**Files:**
- Modify: `tests/common/fixtures.rs`
- Modify: `tests/board_commands.rs`

- [ ] **Step 1: Add board fixture helpers to `tests/common/fixtures.rs`**

Append to the end of `tests/common/fixtures.rs`:

```rust
pub fn board_response(id: u64, name: &str, board_type: &str, project_key: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "type": board_type,
        "location": {
            "projectKey": project_key,
            "projectName": format!("{} Project", project_key)
        }
    })
}

pub fn board_list_response(boards: Vec<Value>) -> Value {
    let total = boards.len() as u32;
    json!({
        "values": boards,
        "startAt": 0,
        "maxResults": 50,
        "total": total
    })
}

pub fn board_config_response(id: u64, board_type: &str) -> Value {
    json!({
        "id": id,
        "name": "Board Config",
        "type": board_type
    })
}

pub fn sprint_response(id: u64, name: &str, state: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "state": state,
        "startDate": "2026-03-20T00:00:00.000Z",
        "endDate": "2026-04-03T00:00:00.000Z"
    })
}

pub fn sprint_list_response(sprints: Vec<Value>) -> Value {
    let total = sprints.len() as u32;
    json!({
        "values": sprints,
        "startAt": 0,
        "maxResults": 50,
        "total": total
    })
}

pub fn sprint_issues_response(issues: Vec<Value>) -> Value {
    let total = issues.len() as u32;
    json!({
        "issues": issues,
        "startAt": 0,
        "maxResults": 50,
        "total": total
    })
}
```

- [ ] **Step 2: Update `tests/board_commands.rs` to use shared fixtures**

Remove the local `board_response`, `board_list_response`, `board_config_response`, `sprint_list_response`, `sprint_response`, and `sprint_issues_response` functions from `tests/board_commands.rs` and replace all calls with `common::fixtures::board_response(...)`, `common::fixtures::board_list_response(...)`, etc.

- [ ] **Step 3: Run tests to verify everything still passes**

Run: `cargo test --test board_commands -- --nocapture`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add tests/common/fixtures.rs tests/board_commands.rs
git commit -m "refactor: move board test fixtures to shared fixtures file (#70)"
```

---

### Task 5: Update README and verify

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update the board list and sprint list entries in the Commands table**

In `README.md`, update the `jr board list` row (line 110) from:
```
| `jr board list` | List boards |
```
to:
```
| `jr board list` | List boards (`--project`, `--type scrum\|kanban`) |
```

Update the `jr sprint list` row (line 112) from:
```
| `jr sprint list --board 42` | List sprints (`--board` or config, scrum only) |
```
to:
```
| `jr sprint list --board 42` | List sprints (`--board` or config or auto-discover, scrum only) |
```

- [ ] **Step 2: Add auto-discover example to Quick Start**

After the existing `jr issue list --project FOO` line (line 58), add:

```bash
# Sprint list (auto-discovers scrum board for project)
jr sprint list --project FOO
```

- [ ] **Step 3: Run full test suite one final time**

Run: `cargo test && cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: All pass, clean

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: update README with board auto-resolve and --type flag (#70)"
```
