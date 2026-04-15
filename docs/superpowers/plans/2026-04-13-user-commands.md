# User Search and Lookup Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `jr user search`, `jr user list --project`, and `jr user view <accountId>` commands, wrapping existing `search_users` / `search_assignable_users_by_project` API methods and one new `get_user` method.

**Architecture:** Thin CLI surface over existing `JiraClient` methods. New file `src/cli/user.rs` follows the `team.rs` / `project.rs` pattern. One new API method `get_user(account_id)`. Integration tests in a new `tests/user_commands.rs` following the existing wiremock + `assert_cmd` pattern.

**Tech Stack:** Rust, clap derive, reqwest, tokio, wiremock, assert_cmd, `comfy-table` via `output::print_output`.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `src/api/jira/users.rs` | Modify | Add `get_user(account_id)` method (~12 lines) |
| `src/cli/mod.rs` | Modify | Register `pub mod user;`, add `User { command: UserCommand }` enum variant, add `UserCommand` enum |
| `src/cli/user.rs` | Create | Command handler: dispatches `search`, `list`, `view`; formats rows; maps 404 to friendly error |
| `src/main.rs` | Modify | Dispatch `Command::User` to `cli::user::handle` |
| `tests/user_commands.rs` | Create | Wiremock integration tests for all three commands |

---

### Task 1: Add `get_user` API method (TDD)

**Files:**
- Modify: `src/api/jira/users.rs`

- [ ] **Step 1.1: Write the failing unit test**

Append to the `#[cfg(test)] mod tests { ... }` block at the end of `src/api/jira/users.rs`:

```rust
    #[test]
    fn single_user_response_deserializes() {
        let json = r#"{
            "accountId": "5b10ac8d82e05b22cc7d4349",
            "displayName": "Jane Smith",
            "emailAddress": "jane@acme.io",
            "active": true
        }"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.account_id, "5b10ac8d82e05b22cc7d4349");
        assert_eq!(user.display_name, "Jane Smith");
        assert_eq!(user.email_address.as_deref(), Some("jane@acme.io"));
        assert_eq!(user.active, Some(true));
    }

    #[test]
    fn single_user_without_email_deserializes() {
        let json = r#"{
            "accountId": "abc",
            "displayName": "Privacy User",
            "active": true
        }"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.account_id, "abc");
        assert!(user.email_address.is_none());
    }
```

Run: `cargo test --lib --quiet single_user_response_deserializes single_user_without_email_deserializes`

Expected: PASS (these tests only verify the existing `User` struct deserializes a single object — no new code required yet but they document the expected response shape).

- [ ] **Step 1.2: Add the `get_user` method**

In `src/api/jira/users.rs`, inside the `impl JiraClient` block, after `search_assignable_users_by_project` (currently the last method in the block, line 80), add:

```rust
    /// Fetch a single user by accountId.
    ///
    /// Returns a `JrError::ApiError { status: 404, .. }` when the accountId
    /// does not exist. Email may be omitted from the response based on the
    /// target user's profile-visibility settings.
    pub async fn get_user(&self, account_id: &str) -> Result<User> {
        let path = format!(
            "/rest/api/3/user?accountId={}",
            urlencoding::encode(account_id)
        );
        self.get(&path).await
    }
```

- [ ] **Step 1.3: Run unit tests**

Run: `cargo test --lib --quiet api::jira::users`

Expected: PASS — all existing tests plus the two new deserialization tests.

- [ ] **Step 1.4: Run clippy and fmt**

Run:
```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: Both clean.

- [ ] **Step 1.5: Commit**

```bash
git add src/api/jira/users.rs
git commit -m "feat: add get_user API method for accountId lookup (#114)"
```

---

### Task 2: Register `UserCommand` in CLI enum

**Files:**
- Modify: `src/cli/mod.rs`

- [ ] **Step 2.1: Register the module**

In `src/cli/mod.rs`, find the `pub mod` list at the top (lines 1-11):

```rust
pub mod api;
pub mod assets;
pub mod auth;
pub mod board;
pub mod init;
pub mod issue;
pub mod project;
pub mod queue;
pub mod sprint;
pub mod team;
pub mod worklog;
```

Add `pub mod user;` in alphabetical order (after `team`, before `worklog`):

```rust
pub mod api;
pub mod assets;
pub mod auth;
pub mod board;
pub mod init;
pub mod issue;
pub mod project;
pub mod queue;
pub mod sprint;
pub mod team;
pub mod user;
pub mod worklog;
```

- [ ] **Step 2.2: Add the `User` variant to `Command`**

In `src/cli/mod.rs`, find the `Command` enum definition. After the `Team { command: TeamCommand }` variant (currently lines 91-94), add a `User { command: UserCommand }` variant before `Queue`:

```rust
    /// Manage teams
    Team {
        #[command(subcommand)]
        command: TeamCommand,
    },
    /// Manage users
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
    /// Manage JSM queues
    Queue {
        #[command(subcommand)]
        command: QueueCommand,
    },
```

- [ ] **Step 2.3: Add the `UserCommand` enum**

In `src/cli/mod.rs`, after the `TeamCommand` enum definition (currently ends at line 509) and before the `WorklogCommand` enum, add:

```rust
#[derive(Subcommand)]
pub enum UserCommand {
    /// Search for users by display name or email
    ///
    /// Results depend on the "Browse users and groups" global permission.
    /// Empty results may indicate either no matches or missing permission.
    /// Email is hidden when the target user's privacy settings opt out.
    Search {
        /// Search string (matches displayName and emailAddress substrings)
        query: String,
        /// Maximum number of results
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all results (no default limit)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
    /// List users assignable to a project
    ///
    /// Results depend on the "Browse users and groups" global permission.
    List {
        /// Project key (e.g., FOO)
        #[arg(long, short = 'p')]
        project: String,
        /// Maximum number of results
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all results (no default limit)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
    /// Look up a user by accountId
    ///
    /// Returns full user details (displayName, email when visible, active,
    /// timeZone). Use this to resolve accountIds surfaced in JSON output
    /// from other commands (e.g., `jr issue view --output json`).
    View {
        /// Atlassian accountId (e.g., 5b10ac8d82e05b22cc7d4349)
        account_id: String,
    },
}
```

- [ ] **Step 2.4: Build to verify the enum compiles**

Run: `cargo build --quiet`

Expected: Fails with `unresolved import `crate::cli::user`` or similar — the module file doesn't exist yet. This failure confirms the enum plumbing is correct and Task 3 will resolve it.

- [ ] **Step 2.5: Do not commit yet**

Commits happen together with Task 3 once the module exists and the build is clean.

---

### Task 3: Implement `user search` + `user list` + `user view` handler

**Files:**
- Create: `src/cli/user.rs`
- Modify: `src/main.rs`

- [ ] **Step 3.1: Create the handler module**

Create `src/cli/user.rs` with this complete content:

```rust
use anyhow::{Result, anyhow};
use colored::Colorize;

use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, UserCommand, resolve_effective_limit};
use crate::error::JrError;
use crate::output;
use crate::types::jira::User;

pub async fn handle(
    command: UserCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    match command {
        UserCommand::Search { query, limit, all } => {
            handle_search(&query, limit, all, output_format, client).await
        }
        UserCommand::List {
            project,
            limit,
            all,
        } => handle_list(&project, limit, all, output_format, client).await,
        UserCommand::View { account_id } => handle_view(&account_id, output_format, client).await,
    }
}

async fn handle_search(
    query: &str,
    limit: Option<u32>,
    all: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let effective = resolve_effective_limit(limit, all);
    let mut users = client.search_users(query).await?;
    if let Some(cap) = effective {
        users.truncate(cap as usize);
    }
    print_user_list(&users, output_format)
}

async fn handle_list(
    project: &str,
    limit: Option<u32>,
    all: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let effective = resolve_effective_limit(limit, all);
    let mut users = client
        .search_assignable_users_by_project("", project)
        .await?;
    if let Some(cap) = effective {
        users.truncate(cap as usize);
    }
    print_user_list(&users, output_format)
}

async fn handle_view(
    account_id: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let user = match client.get_user(account_id).await {
        Ok(u) => u,
        Err(e) => {
            if let Some(JrError::ApiError { status, .. }) = e.downcast_ref::<JrError>() {
                if *status == 404 || *status == 400 {
                    return Err(anyhow!(
                        "User with accountId '{account_id}' not found."
                    ));
                }
            }
            return Err(e);
        }
    };

    let rows = vec![
        vec!["Account ID".into(), user.account_id.clone()],
        vec!["Display Name".into(), user.display_name.clone()],
        vec![
            "Email".into(),
            user.email_address.clone().unwrap_or_else(|| "—".into()),
        ],
        vec!["Active".into(), format_active(user.active)],
    ];

    output::print_output(output_format, &["Field", "Value"], &rows, &user)
}

fn print_user_list(users: &[User], output_format: &OutputFormat) -> Result<()> {
    let rows: Vec<Vec<String>> = users.iter().map(format_user_row).collect();
    output::print_output(
        output_format,
        &["Display Name", "Email", "Active", "Account ID"],
        &rows,
        &users,
    )
}

fn format_user_row(user: &User) -> Vec<String> {
    vec![
        user.display_name.clone(),
        user.email_address.clone().unwrap_or_else(|| "—".into()),
        format_active(user.active),
        user.account_id.clone(),
    ]
}

fn format_active(active: Option<bool>) -> String {
    match active {
        Some(true) => "✓".green().to_string(),
        Some(false) => "✗".red().to_string(),
        None => "—".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_shows_display_name_email_and_id() {
        let user = User {
            account_id: "acc-1".into(),
            display_name: "Alice".into(),
            email_address: Some("alice@acme.io".into()),
            active: Some(true),
        };
        let row = format_user_row(&user);
        assert_eq!(row[0], "Alice");
        assert_eq!(row[1], "alice@acme.io");
        assert!(row[2].contains('✓'));
        assert_eq!(row[3], "acc-1");
    }

    #[test]
    fn row_renders_dash_for_missing_email() {
        let user = User {
            account_id: "acc-2".into(),
            display_name: "Privacy User".into(),
            email_address: None,
            active: Some(true),
        };
        let row = format_user_row(&user);
        assert_eq!(row[1], "—");
    }

    #[test]
    fn active_formatter_handles_missing() {
        assert_eq!(format_active(None), "—");
    }
}
```

- [ ] **Step 3.2: Wire up dispatch in `main.rs`**

In `src/main.rs`, find the match arm for `Command::Team` (currently lines 156-160):

```rust
            cli::Command::Team { command } => {
                let config = config::Config::load()?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::team::handle(command, &cli.output, &config, &client).await
            }
```

After this arm and before `cli::Command::Queue`, add:

```rust
            cli::Command::User { command } => {
                let config = config::Config::load()?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::user::handle(command, &cli.output, &client).await
            }
```

- [ ] **Step 3.3: Build**

Run: `cargo build --quiet`

Expected: Clean build.

- [ ] **Step 3.4: Run unit tests**

Run: `cargo test --lib --quiet user`

Expected: The three tests in `src/cli/user.rs::tests` pass, alongside any existing tests that match the `user` substring.

- [ ] **Step 3.5: Run clippy and fmt**

Run:
```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: Both clean.

- [ ] **Step 3.6: Smoke-test the CLI**

Run: `cargo run --quiet -- user --help`

Expected output contains `Manage users` and the three subcommands `search`, `list`, `view`.

Run: `cargo run --quiet -- user search --help`

Expected output contains `--limit`, `--all`, and the permission caveat.

- [ ] **Step 3.7: Commit**

```bash
git add src/api/jira/users.rs src/cli/mod.rs src/cli/user.rs src/main.rs
git commit -m "feat: add user search/list/view commands (#114)"
```

---

### Task 4: Integration tests (wiremock)

**Files:**
- Create: `tests/user_commands.rs`

- [ ] **Step 4.1: Create the test file**

Create `tests/user_commands.rs` with this complete content:

```rust
#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::fixtures;

fn jr_cmd(base_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", base_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input");
    cmd
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_search_returns_matching_users() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "jane"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            fixtures::user_search_response(vec![
                ("acc-1", "Jane Smith", true),
                ("acc-2", "Jane Doe", true),
            ]),
        ))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "search", "jane"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Jane Smith"))
        .stdout(predicate::str::contains("Jane Doe"))
        .stdout(predicate::str::contains("acc-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_search_empty_result_prints_no_results() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "search", "nobody"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No results found."));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_search_json_output_is_array() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            fixtures::user_search_response(vec![("acc-1", "Jane Smith", true)]),
        ))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--output", "json", "user", "search", "jane"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON array");
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["accountId"], "acc-1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_search_limit_truncates_results() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            fixtures::user_search_response(vec![
                ("acc-1", "Alice One", true),
                ("acc-2", "Alice Two", true),
                ("acc-3", "Alice Three", true),
            ]),
        ))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--output", "json", "user", "search", "alice", "--limit", "2"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_list_requires_project_flag() {
    // No server needed — clap should fail before any HTTP call.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "user", "list"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "missing --project should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--project") || stderr.contains("required"),
        "expected error mentions missing --project, got: {stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_list_by_project_returns_users() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            fixtures::multi_project_user_search_response(vec![
                ("acc-1", "Alice"),
                ("acc-2", "Bob"),
            ]),
        ))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "list", "--project", "FOO"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("Bob"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_view_returns_detail_rows() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "acc-xyz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "acc-xyz",
            "displayName": "Jane Smith",
            "emailAddress": "jane@acme.io",
            "active": true
        })))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "view", "acc-xyz"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Jane Smith"))
        .stdout(predicate::str::contains("jane@acme.io"))
        .stdout(predicate::str::contains("acc-xyz"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_view_json_emits_user_object() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "acc-xyz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "acc-xyz",
            "displayName": "Jane Smith",
            "emailAddress": "jane@acme.io",
            "active": true
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["--output", "json", "user", "view", "acc-xyz"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["accountId"], "acc-xyz");
    assert_eq!(parsed["displayName"], "Jane Smith");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_view_404_shows_friendly_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "does-not-exist"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["User not found"]
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["user", "view", "does-not-exist"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("User with accountId 'does-not-exist' not found"),
        "expected friendly not-found message, got: {stderr}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_view_hidden_email_renders_dash() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user"))
        .and(query_param("accountId", "private-user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "private-user",
            "displayName": "Private Person",
            "active": true
        })))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["user", "view", "private-user"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Private Person"))
        .stdout(predicate::str::contains("—"));
}
```

- [ ] **Step 4.2: Run the new integration tests**

Run: `cargo test --test user_commands --quiet`

Expected: All 10 tests pass.

- [ ] **Step 4.3: Run the full test suite**

Run: `cargo test --quiet`

Expected: All tests pass — no existing tests broken by the new module/variant.

- [ ] **Step 4.4: Run clippy and fmt**

Run:
```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: Both clean.

- [ ] **Step 4.5: Commit**

```bash
git add tests/user_commands.rs
git commit -m "test: add integration tests for user commands (#114)"
```

---

### Task 5: Verify end-to-end and tidy

**Files:**
- None (verification task — catches anything the earlier tasks missed)

- [ ] **Step 5.1: Full CI-equivalent check set**

Run each in sequence:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: All three clean.

- [ ] **Step 5.2: Verify `jr user --help` reads correctly**

Run: `cargo run --quiet -- user --help`

Expected output includes:
- Top description: "Manage users"
- Three subcommands: `search`, `list`, `view`
- Global flags inherited: `--output`, `--project`, `--no-color`, `--no-input`, `--verbose`

- [ ] **Step 5.3: Verify each subcommand help text includes the GDPR caveat for `search` and `list`**

Run: `cargo run --quiet -- user search --help`

Expected contains: "Browse users and groups" and "privacy settings"

Run: `cargo run --quiet -- user list --help`

Expected contains: "Browse users and groups"

- [ ] **Step 5.4: Check for stale or dead code**

Run: `cargo clippy --all-targets -- -D warnings -W dead-code 2>&1 | head -20`

Expected: No warnings referencing `src/cli/user.rs` or `tests/user_commands.rs`.

- [ ] **Step 5.5: If anything failed, fix it — do not commit partial work**

Any failure in Steps 5.1–5.4 means returning to the failing task to correct the implementation. Do not proceed to PR review until the full check set is clean.

---

## Spec Coverage Checklist

| Spec Requirement | Task / Step |
|---|---|
| `jr user search <query>` command | Task 2 (enum), Task 3 (handler) |
| `jr user list --project KEY` command | Task 2 (enum), Task 3 (handler) |
| `jr user view <accountId>` command | Task 2 (enum), Task 3 (handler) |
| New `get_user` API method | Task 1 |
| Reuse existing `search_users` unchanged | Task 3 (Step 3.1 — `handle_search`) |
| Reuse existing `search_assignable_users_by_project` unchanged | Task 3 (Step 3.1 — `handle_list`) |
| `--limit N` / `--all` on search and list | Task 2 (Step 2.3), Task 3 (`resolve_effective_limit`) |
| Positional `<query>` and `<accountId>` args | Task 2 (Step 2.3) |
| No `--email` flag | Task 2 (Step 2.3 — deliberately absent) |
| Table columns: Display Name \| Email \| Active \| Account ID | Task 3 (`print_user_list`) |
| Full accountId (not truncated) | Task 3 (`format_user_row`) |
| `—` for missing email | Task 3 (`format_user_row`), Task 4 (Step 4.1 privacy test) |
| `✓`/`✗` for active | Task 3 (`format_active`) |
| View uses labeled Field/Value rows | Task 3 (`handle_view`) |
| JSON mode: raw user(s) | Task 3 (`print_output` passes `&user` / `&users`) |
| 404 on view → friendly "not found" error, exit 64 via `JrError::UserError` | Task 3 (`handle_view`), Task 4 (Step 4.1 404 test) |
| 400 also treated as "not found" (same exit 64) | Task 3 (`handle_view`) |
| Empty search result → "No results found." | Task 4 (Step 4.1 empty test) — uses `output::print_output` default behavior |
| Help text: GDPR caveat | Task 2 (Step 2.3 doc comments on `Search` and `List`) |
| Integration tests with wiremock | Task 4 (all) |
| Unit tests for formatters | Task 3 (`#[cfg(test)]` block in `src/cli/user.rs`) |
| Full CI-equivalent checks clean | Task 5 (Step 5.1) |
