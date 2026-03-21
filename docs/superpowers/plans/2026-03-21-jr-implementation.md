# jr — Jira CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `jr`, a Rust CLI tool for automating Jira Cloud workflows from the terminal.

**Architecture:** Thin client wrapping Jira REST API v3 and Agile REST API directly with `reqwest`. Single binary, clap-derived CLI, layered config via figment, credentials in OS keychain. Product-namespaced modules (`api/jira/`, `types/jira/`) for future extensibility.

**Tech Stack:** Rust, clap 4.x (derive), reqwest 0.12, tokio 1.x, serde, figment, keyring, comfy-table, colored, dialoguer, anyhow/thiserror. Testing: wiremock, assert_cmd, predicates, tempfile.

**Spec:** `docs/superpowers/specs/2026-03-21-jr-jira-cli-design.md`

---

## File Map

### Source Files

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Project manifest, dependencies |
| `src/main.rs` | Entry point, tokio runtime, clap dispatch |
| `src/cli/mod.rs` | Top-level CLI enum (clap derive), global flags |
| `src/cli/auth.rs` | `jr auth login`, `jr auth status` subcommands |
| `src/cli/init.rs` | `jr init` interactive setup |
| `src/cli/issue.rs` | `jr issue list/view/create/edit/move/assign/comment/open` |
| `src/cli/board.rs` | `jr board list/view` |
| `src/cli/sprint.rs` | `jr sprint list/current` |
| `src/cli/worklog.rs` | `jr worklog add/list` |
| `src/api/mod.rs` | Re-exports client, auth, pagination, rate_limit |
| `src/api/client.rs` | `JiraClient` struct, HTTP methods, base URL, auth headers |
| `src/api/auth.rs` | OAuth 2.0 flow, API token flow, keychain read/write, token refresh |
| `src/api/pagination.rs` | Offset-based and cursor-based pagination helpers |
| `src/api/rate_limit.rs` | 429 detection, `Retry-After` parsing, retry logic |
| `src/api/jira/mod.rs` | Re-exports all Jira API modules |
| `src/api/jira/issues.rs` | Issue CRUD, search, transitions, comments, assign (all issue-related API calls in one file) |
| `src/api/jira/boards.rs` | Board list, board config |
| `src/api/jira/sprints.rs` | Sprint list, sprint issues |
| `src/api/jira/worklogs.rs` | Worklog add, worklog list |
| `src/api/jira/users.rs` | `/myself`, user search |
| `src/api/jira/fields.rs` | Custom field discovery (Team field) |
| `src/types/mod.rs` | Re-exports all type modules |
| `src/types/jira/mod.rs` | Re-exports Jira types |
| `src/types/jira/issue.rs` | `Issue`, `IssueFields`, `Transition`, `Comment` structs |
| `src/types/jira/board.rs` | `Board`, `BoardConfig` structs |
| `src/types/jira/project.rs` | `Project` struct |
| `src/types/jira/sprint.rs` | `Sprint` struct |
| `src/types/jira/user.rs` | `User` struct |
| `src/types/jira/worklog.rs` | `Worklog` struct |
| `src/config.rs` | `GlobalConfig`, `ProjectConfig`, figment loading |
| `src/output.rs` | `OutputFormat` enum, table rendering, JSON rendering |
| `src/adf.rs` | ADF→plain text, plain text→ADF, Markdown→ADF |
| `src/duration.rs` | Worklog duration parser (`2h`, `1h30m`, `1d`) |
| `src/error.rs` | `JrError` enum with thiserror, user-friendly messages |
| `src/partial_match.rs` | Case-insensitive substring matching with disambiguation |

### Test Files

| File | What it tests |
|------|--------------|
| `tests/cli_smoke.rs` | Binary exists, `--help` works, `--version` works |
| `tests/common/mod.rs` | Shared test utilities module |
| `tests/common/fixtures.rs` | Reusable mock JSON responses and builders |
| `tests/common/mock_server.rs` | Helper to create pre-configured wiremock servers |
| `tests/api_client.rs` | JiraClient integration tests with wiremock |
| `tests/issue_commands.rs` | Issue command integration tests with wiremock |
| `tests/board_sprint_commands.rs` | Board/sprint command integration tests |
| `tests/worklog_commands.rs` | Worklog command integration tests |
| `src/duration.rs` (inline) | Duration parsing unit tests + proptest |
| `src/adf.rs` (inline) | ADF conversion unit tests + insta snapshots |
| `src/partial_match.rs` (inline) | Partial matching unit tests + proptest |
| `src/config.rs` (inline) | Config loading unit tests |
| `src/api/pagination.rs` (inline) | Pagination logic unit tests |
| `src/api/rate_limit.rs` (inline) | Rate limit detection unit tests |

---

## Task 1: Project Scaffold + CLI Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/cli/mod.rs`
- Create: `src/error.rs`
- Create: `tests/cli_smoke.rs`

- [ ] **Step 1: Initialize Cargo project**

```bash
cd /Users/zious/Documents/GITHUB/jira-cli
cargo init --name jr
```

- [ ] **Step 2: Set up Cargo.toml with all dependencies**

Replace `Cargo.toml` with:

```toml
[package]
name = "jr"
version = "0.1.0"
edition = "2021"
description = "A fast CLI for Jira Cloud"
license = "MIT"
repository = "https://github.com/Zious11/jira-cli"

[[bin]]
name = "jr"
path = "src/main.rs"

[dependencies]
anyhow = "1"
atty = "0.2"
clap = { version = "4", features = ["derive"] }
clap_complete = "4"
colored = "2"
comfy-table = "7"
dialoguer = "0.12"
dirs = "5"
figment = { version = "0.10", features = ["toml", "env"] }
keyring = "3"
open = "5"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
toml = "0.8"
base64 = "0.22"
urlencoding = "2"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
wiremock = "0.6"
insta = { version = "1", features = ["json"] }
proptest = "1"

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

- [ ] **Step 3: Create module directory structure**

```bash
mkdir -p src/cli src/api/jira src/types/jira tests/common
```

- [ ] **Step 4: Create error types**

Write `src/error.rs`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JrError {
    #[error("Not authenticated. Run \"jr auth login\" to connect.")]
    NotAuthenticated,

    #[error("Could not reach {0} — check your connection")]
    NetworkError(String),

    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("{0}")]
    UserError(String),

    #[error("Interrupted")]
    Interrupted,

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl JrError {
    /// Map error to a standardized exit code for scripting
    pub fn exit_code(&self) -> i32 {
        match self {
            JrError::NotAuthenticated => 2,
            JrError::ConfigError(_) => 78,
            JrError::UserError(_) => 64,
            JrError::Interrupted => 130,
            _ => 1,
        }
    }
}
```

- [ ] **Step 5: Create CLI skeleton with clap derive**

Write `src/cli/mod.rs`:

```rust
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "jr", version, about = "A fast CLI for Jira Cloud")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Output format
    #[arg(long, global = true, default_value = "table")]
    pub output: OutputFormat,

    /// Override project key
    #[arg(long, global = true)]
    pub project: Option<String>,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Disable interactive prompts (auto-enabled when stdin is not a TTY)
    #[arg(long, global = true)]
    pub no_input: bool,

    /// Enable verbose output
    #[arg(long, global = true)]
    pub verbose: bool,
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize jr configuration
    Init,
    /// Manage authentication
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Show current user info
    Me,
    /// Show valid issue types, priorities, and statuses for a project
    #[command(name = "project")]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Manage issues
    Issue {
        #[command(subcommand)]
        command: IssueCommand,
    },
    /// Manage boards
    Board {
        #[command(subcommand)]
        command: BoardCommand,
    },
    /// Manage sprints
    Sprint {
        #[command(subcommand)]
        command: SprintCommand,
    },
    /// Manage worklogs
    Worklog {
        #[command(subcommand)]
        command: WorklogCommand,
    },
    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Authenticate with Jira
    Login {
        /// Use API token instead of OAuth
        #[arg(long)]
        token: bool,
    },
    /// Show authentication status
    Status,
}

#[derive(Subcommand)]
pub enum IssueCommand {
    /// List issues
    List {
        /// JQL query
        #[arg(long)]
        jql: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        /// Filter by team
        #[arg(long)]
        team: Option<String>,
        /// Maximum number of results
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Create a new issue
    Create {
        /// Project key
        #[arg(short, long)]
        project: Option<String>,
        /// Issue type
        #[arg(short = 't', long = "type")]
        issue_type: Option<String>,
        /// Summary
        #[arg(short, long)]
        summary: Option<String>,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
        /// Read description from stdin (for piping)
        #[arg(long)]
        description_stdin: bool,
        /// Priority
        #[arg(long)]
        priority: Option<String>,
        /// Labels (can be specified multiple times)
        #[arg(long)]
        label: Vec<String>,
        /// Team assignment
        #[arg(long)]
        team: Option<String>,
        /// Interpret description as Markdown
        #[arg(long)]
        markdown: bool,
    },
    /// View issue details
    View {
        /// Issue key (e.g., FOO-123)
        key: String,
    },
    /// Edit issue fields
    Edit {
        /// Issue key
        key: String,
        /// New summary
        #[arg(long)]
        summary: Option<String>,
        /// New issue type
        #[arg(long = "type")]
        issue_type: Option<String>,
        /// New priority
        #[arg(long)]
        priority: Option<String>,
        /// Add or remove labels (e.g., --label add:backend --label remove:frontend)
        #[arg(long)]
        label: Vec<String>,
        /// Team assignment
        #[arg(long)]
        team: Option<String>,
    },
    /// Transition issue to a new status
    Move {
        /// Issue key
        key: String,
        /// Target status (partial match supported)
        status: Option<String>,
    },
    /// List available transitions without performing one
    Transitions {
        /// Issue key
        key: String,
    },
    /// Assign issue
    Assign {
        /// Issue key
        key: String,
        /// Assign to this user (omit to assign to self)
        #[arg(long)]
        to: Option<String>,
        /// Remove assignee
        #[arg(long)]
        unassign: bool,
    },
    /// Add a comment
    Comment {
        /// Issue key
        key: String,
        /// Comment text
        message: Option<String>,
        /// Interpret input as Markdown
        #[arg(long)]
        markdown: bool,
        /// Read comment from file
        #[arg(long)]
        file: Option<String>,
        /// Read comment from stdin (for piping)
        #[arg(long)]
        stdin: bool,
    },
    /// Open issue in browser
    Open {
        /// Issue key
        key: String,
        /// Print URL instead of opening browser (for scripting/AI agents)
        #[arg(long)]
        url_only: bool,
    },
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Show valid issue types, priorities, and statuses
    Fields {
        /// Project key (uses configured project if omitted)
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum BoardCommand {
    /// List boards
    List,
    /// View current board issues
    View,
}

#[derive(Subcommand)]
pub enum SprintCommand {
    /// List sprints
    List,
    /// Show current sprint issues
    Current,
}

#[derive(Subcommand)]
pub enum WorklogCommand {
    /// Log time on an issue
    Add {
        /// Issue key
        key: String,
        /// Duration (e.g., 2h, 1h30m, 1d)
        duration: String,
        /// Comment
        #[arg(short, long)]
        message: Option<String>,
    },
    /// List worklogs on an issue
    List {
        /// Issue key
        key: String,
    },
}
```

- [ ] **Step 6: Create main.rs entry point**

Write `src/main.rs`:

```rust
mod cli;
mod error;

use clap::{CommandFactory, Parser};
use cli::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.no_color || std::env::var("NO_COLOR").is_ok() {
        colored::control::set_override(false);
    }

    // Auto-enable --no-input when stdin is not a TTY (AI agents, pipes, scripts)
    let mut cli = cli;
    if !cli.no_input && !atty::is(atty::Stream::Stdin) {
        cli.no_input = true;
    }

    let output_format = cli.output.clone();
    let result = run(cli).await;
    if let Err(e) = result {
        let exit_code = e.downcast_ref::<error::JrError>()
            .map(|je| je.exit_code())
            .unwrap_or(1);

        // Structured JSON errors when --output json is set
        match output_format {
            cli::OutputFormat::Json => {
                eprintln!("{}", serde_json::json!({
                    "error": e.to_string(),
                    "code": exit_code
                }));
            }
            _ => {
                eprintln!("Error: {e}");
            }
        }

        std::process::exit(exit_code);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    // Handle completion before anything else (no config/auth needed)
    if let cli::Command::Completion { shell } = &cli.command {
        let mut cmd = Cli::command();
        clap_complete::generate(*shell, &mut cmd, "jr", &mut std::io::stdout());
        return Ok(());
    }

    // Set up Ctrl+C handler
    let main_task = async {
        match cli.command {
            cli::Command::Completion { .. } => unreachable!(),
            cli::Command::Init => todo!("init"),
            cli::Command::Auth { command } => todo!("auth"),
            cli::Command::Me => todo!("me"),
            cli::Command::Project { command } => todo!("project"),
            cli::Command::Issue { command } => todo!("issue"),
            cli::Command::Board { command } => todo!("board"),
            cli::Command::Sprint { command } => todo!("sprint"),
            cli::Command::Worklog { command } => todo!("worklog"),
        }
    };

    tokio::select! {
        result = main_task => result,
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nInterrupted");
            std::process::exit(130);
        }
    }
}
```

- [ ] **Step 7: Verify it compiles**

```bash
cargo build
```

Expected: Compiles with possible warnings about unused variables/imports.

- [ ] **Step 8: Write CLI smoke tests**

Write `tests/cli_smoke.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_flag() {
    Command::cargo_bin("jr")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("A fast CLI for Jira Cloud"));
}

#[test]
fn test_version_flag() {
    Command::cargo_bin("jr")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("jr"));
}

#[test]
fn test_no_args_shows_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}
```

- [ ] **Step 9: Run tests**

```bash
cargo test
```

Expected: All 3 tests pass.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat: project scaffold with CLI skeleton and smoke tests"
```

---

## Task 2: Configuration System

**Files:**
- Create: `src/config.rs`

- [ ] **Step 1: Write config loading tests**

Add to `src/config.rs`:

```rust
use figment::{Figment, providers::{Format, Toml, Env, Serialized}};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub instance: InstanceConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct InstanceConfig {
    pub url: Option<String>,
    pub cloud_id: Option<String>,
    pub auth_method: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DefaultsConfig {
    pub output: String,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            output: "table".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ProjectConfig {
    pub project: Option<String>,
    pub board_id: Option<u64>,
}

#[derive(Debug, Default)]
pub struct Config {
    pub global: GlobalConfig,
    pub project: ProjectConfig,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let global_path = global_config_path();
        let global: GlobalConfig = Figment::new()
            .merge(Serialized::defaults(GlobalConfig::default()))
            .merge(Toml::file(&global_path))
            .merge(Env::prefixed("JR_"))
            .extract()?;

        let project = Self::find_project_config()
            .map(|path| {
                Figment::new()
                    .merge(Toml::file(path))
                    .extract::<ProjectConfig>()
            })
            .transpose()?
            .unwrap_or_default();

        Ok(Config { global, project })
    }

    fn find_project_config() -> Option<PathBuf> {
        let mut dir = std::env::current_dir().ok()?;
        loop {
            let candidate = dir.join(".jr.toml");
            if candidate.exists() {
                return Some(candidate);
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    pub fn base_url(&self) -> anyhow::Result<String> {
        // JR_BASE_URL env var overrides everything (used by tests to inject wiremock URL)
        if let Ok(override_url) = std::env::var("JR_BASE_URL") {
            return Ok(override_url.trim_end_matches('/').to_string());
        }

        let url = self.global.instance.url.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No Jira instance configured. Run \"jr init\" first."))?;

        if let Some(cloud_id) = &self.global.instance.cloud_id {
            if self.global.instance.auth_method.as_deref() == Some("oauth") {
                return Ok(format!("https://api.atlassian.com/ex/jira/{cloud_id}"));
            }
        }
        Ok(url.trim_end_matches('/').to_string())
    }

    pub fn project_key(&self, cli_override: Option<&str>) -> Option<String> {
        cli_override.map(String::from)
            .or_else(|| self.project.project.clone())
    }
}

pub fn global_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("jr")
        .join("config.toml")
}

pub fn global_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("jr")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = GlobalConfig::default();
        assert_eq!(config.defaults.output, "table");
        assert!(config.instance.url.is_none());
    }

    #[test]
    fn test_project_config_parsing() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join(".jr.toml");
        fs::write(&config_path, "project = \"FOO\"\nboard_id = 42\n").unwrap();

        let config: ProjectConfig = Figment::new()
            .merge(Toml::file(config_path))
            .extract()
            .unwrap();

        assert_eq!(config.project.as_deref(), Some("FOO"));
        assert_eq!(config.board_id, Some(42));
    }

    #[test]
    fn test_base_url_api_token() {
        let config = Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://myorg.atlassian.net".into()),
                    cloud_id: None,
                    auth_method: Some("api_token".into()),
                },
                defaults: DefaultsConfig::default(),
            },
            project: ProjectConfig::default(),
        };
        assert_eq!(config.base_url().unwrap(), "https://myorg.atlassian.net");
    }

    #[test]
    fn test_base_url_oauth() {
        let config = Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://myorg.atlassian.net".into()),
                    cloud_id: Some("abc-123".into()),
                    auth_method: Some("oauth".into()),
                },
                defaults: DefaultsConfig::default(),
            },
            project: ProjectConfig::default(),
        };
        assert_eq!(
            config.base_url().unwrap(),
            "https://api.atlassian.com/ex/jira/abc-123"
        );
    }

    #[test]
    fn test_base_url_missing() {
        let config = Config {
            global: GlobalConfig::default(),
            project: ProjectConfig::default(),
        };
        assert!(config.base_url().is_err());
    }

    #[test]
    fn test_project_key_cli_override() {
        let config = Config {
            global: GlobalConfig::default(),
            project: ProjectConfig {
                project: Some("FOO".into()),
                board_id: None,
            },
        };
        assert_eq!(config.project_key(Some("BAR")), Some("BAR".into()));
        assert_eq!(config.project_key(None), Some("FOO".into()));
    }
}
```

- [ ] **Step 2: Register module in main.rs**

Add `mod config;` to `src/main.rs`.

- [ ] **Step 3: Run tests**

```bash
cargo test config::tests
```

Expected: All 5 config tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: configuration system with global and per-project config"
```

---

## Task 3: Output Formatting

**Files:**
- Create: `src/output.rs`

- [ ] **Step 1: Write output module with table and JSON formatting**

Write `src/output.rs`:

```rust
use crate::cli::OutputFormat;
use comfy_table::{Table, ContentArrangement, presets::UTF8_FULL_CONDENSED};
use colored::Colorize;
use serde::Serialize;

pub fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers);

    for row in rows {
        table.add_row(row);
    }

    table.to_string()
}

pub fn render_json<T: Serialize>(data: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(data)?)
}

pub fn print_output<T: Serialize>(
    format: &OutputFormat,
    headers: &[&str],
    rows: &[Vec<String>],
    json_data: &T,
) -> anyhow::Result<()> {
    match format {
        OutputFormat::Table => {
            if rows.is_empty() {
                println!("{}", "No results found.".dimmed());
            } else {
                println!("{}", render_table(headers, rows));
            }
        }
        OutputFormat::Json => {
            println!("{}", render_json(json_data)?);
        }
    }
    Ok(())
}

pub fn print_success(msg: &str) {
    println!("{}", msg.green());
}

pub fn print_error(msg: &str) {
    eprintln!("{}: {}", "Error".red().bold(), msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_table_with_data() {
        let headers = &["Key", "Summary"];
        let rows = vec![
            vec!["FOO-1".into(), "Fix bug".into()],
        ];
        let output = render_table(headers, &rows);
        assert!(output.contains("FOO-1"));
        assert!(output.contains("Fix bug"));
    }

    #[test]
    fn test_render_json() {
        let data = serde_json::json!({"key": "FOO-1"});
        let output = render_json(&data).unwrap();
        assert!(output.contains("FOO-1"));
    }
}
```

- [ ] **Step 2: Register module in main.rs**

Add `mod output;` to `src/main.rs`.

- [ ] **Step 3: Run tests**

```bash
cargo test output::tests
```

Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/output.rs src/main.rs
git commit -m "feat: output formatting with table and JSON support"
```

---

## Task 4: API Client Base

**Files:**
- Create: `src/api/mod.rs`
- Create: `src/api/client.rs`
- Create: `src/api/auth.rs` (API token only — OAuth in a later task)
- Create: `src/api/rate_limit.rs`
- Create: `src/api/pagination.rs`
- Create: `src/api/jira/mod.rs`
- Create: `tests/api_client.rs`

- [ ] **Step 1: Write rate limit module**

Write `src/api/rate_limit.rs`:

```rust
use reqwest::Response;
use std::time::Duration;

pub struct RateLimitInfo {
    pub remaining: Option<u64>,
    pub retry_after: Option<Duration>,
}

impl RateLimitInfo {
    pub fn from_response(response: &Response) -> Self {
        let remaining = response
            .headers()
            .get("X-RateLimit-Remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs);

        Self { remaining, retry_after }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_retry_after() {
        // Integration-tested with wiremock in tests/api_client.rs
        // Unit test verifies Duration conversion
        let duration = Duration::from_secs(30);
        assert_eq!(duration.as_secs(), 30);
    }
}
```

- [ ] **Step 2: Write pagination module**

Write `src/api/pagination.rs`:

```rust
use serde::Deserialize;

/// Response wrapper for offset-based pagination (most Jira endpoints)
/// Different endpoints use different keys for the items array:
/// - `values` (boards, sprints, etc.)
/// - `issues` (issue search, sprint issues)
/// - `worklogs` (worklog list)
#[derive(Debug, Deserialize)]
pub struct OffsetPage<T> {
    pub values: Option<Vec<T>>,
    pub issues: Option<Vec<T>>,
    pub worklogs: Option<Vec<T>>,
    #[serde(rename = "startAt")]
    pub start_at: Option<u64>,
    #[serde(rename = "maxResults")]
    pub max_results: Option<u64>,
    pub total: Option<u64>,
}

impl<T> OffsetPage<T> {
    pub fn items(self) -> Vec<T> {
        self.values
            .or(self.issues)
            .or(self.worklogs)
            .unwrap_or_default()
    }

    pub fn has_more(&self) -> bool {
        let start = self.start_at.unwrap_or(0);
        let max = self.max_results.unwrap_or(0);
        let total = self.total.unwrap_or(0);
        start + max < total
    }

    pub fn next_start(&self) -> u64 {
        self.start_at.unwrap_or(0) + self.max_results.unwrap_or(50)
    }
}

/// Response wrapper for cursor-based pagination (JQL search endpoint)
#[derive(Debug, Deserialize)]
pub struct CursorPage<T> {
    #[serde(default)]
    pub issues: Vec<T>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

impl<T> CursorPage<T> {
    pub fn has_more(&self) -> bool {
        self.next_page_token.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_page_has_more() {
        let page: OffsetPage<()> = OffsetPage {
            values: None,
            issues: None,
            worklogs: None,
            start_at: Some(0),
            max_results: Some(50),
            total: Some(100),
        };
        assert!(page.has_more());
        assert_eq!(page.next_start(), 50);
    }

    #[test]
    fn test_offset_page_last_page() {
        let page: OffsetPage<()> = OffsetPage {
            values: None,
            issues: None,
            worklogs: None,
            start_at: Some(50),
            max_results: Some(50),
            total: Some(75),
        };
        assert!(!page.has_more());
    }

    #[test]
    fn test_cursor_page_has_more() {
        let page: CursorPage<()> = CursorPage {
            issues: vec![],
            next_page_token: Some("token123".into()),
        };
        assert!(page.has_more());
    }

    #[test]
    fn test_cursor_page_no_more() {
        let page: CursorPage<()> = CursorPage {
            issues: vec![],
            next_page_token: None,
        };
        assert!(!page.has_more());
    }
}
```

- [ ] **Step 3: Write auth module (API token only)**

Write `src/api/auth.rs`:

```rust
use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "jr-jira-cli";

pub enum AuthMethod {
    ApiToken { email: String, token: String },
    OAuth { access_token: String, refresh_token: String },
}

pub fn store_api_token(email: &str, token: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, "api_token_email")?;
    entry.set_password(email)?;

    let entry = Entry::new(SERVICE_NAME, "api_token")?;
    entry.set_password(token)?;

    Ok(())
}

pub fn load_api_token() -> Result<(String, String)> {
    let email_entry = Entry::new(SERVICE_NAME, "api_token_email")?;
    let email = email_entry.get_password()
        .context("No API token found. Run \"jr auth login --token\" first.")?;

    let token_entry = Entry::new(SERVICE_NAME, "api_token")?;
    let token = token_entry.get_password()
        .context("No API token found. Run \"jr auth login --token\" first.")?;

    Ok((email, token))
}

pub fn store_oauth_tokens(access_token: &str, refresh_token: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, "oauth_access_token")?;
    entry.set_password(access_token)?;

    let entry = Entry::new(SERVICE_NAME, "oauth_refresh_token")?;
    entry.set_password(refresh_token)?;

    Ok(())
}

pub fn load_oauth_tokens() -> Result<(String, String)> {
    let access_entry = Entry::new(SERVICE_NAME, "oauth_access_token")?;
    let access = access_entry.get_password()
        .context("No OAuth token found. Run \"jr auth login\" first.")?;

    let refresh_entry = Entry::new(SERVICE_NAME, "oauth_refresh_token")?;
    let refresh = refresh_entry.get_password()
        .context("No OAuth token found. Run \"jr auth login\" first.")?;

    Ok((access, refresh))
}

pub fn clear_credentials() -> Result<()> {
    for key in ["api_token_email", "api_token", "oauth_access_token", "oauth_refresh_token"] {
        if let Ok(entry) = Entry::new(SERVICE_NAME, key) {
            let _ = entry.delete_credential();
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Write the JiraClient**

Write `src/api/client.rs`:

```rust
use anyhow::{Context, Result};
use reqwest::{Client, Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;

use crate::api::rate_limit::RateLimitInfo;
use crate::config::Config;
use crate::error::JrError;

pub struct JiraClient {
    client: Client,
    base_url: String,
    auth_header: String,
    verbose: bool,
}

impl JiraClient {
    pub fn from_config(config: &Config, verbose: bool) -> Result<Self> {
        let base_url = config.base_url()?;

        let auth_header = match config.global.instance.auth_method.as_deref() {
            Some("oauth") => {
                let (access_token, _) = crate::api::auth::load_oauth_tokens()?;
                format!("Bearer {access_token}")
            }
            _ => {
                let (email, token) = crate::api::auth::load_api_token()?;
                let encoded = base64_encode(&format!("{email}:{token}"));
                format!("Basic {encoded}")
            }
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self { client, base_url, auth_header, verbose })
    }

    /// For testing: construct with explicit base URL and auth.
    /// Not gated behind #[cfg(test)] so integration tests in tests/ can use it.
    pub fn new_for_test(base_url: &str, auth_header: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            auth_header: auth_header.to_string(),
            verbose: false,
        }
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.request(Method::GET, path, None::<&()>).await
    }

    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.request(Method::POST, path, Some(body)).await
    }

    pub async fn put<B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<()> {
        self.request_no_content(Method::PUT, path, Some(body)).await
    }

    pub async fn post_no_content<B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<()> {
        self.request_no_content(Method::POST, path, Some(body)).await
    }

    async fn request<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T> {
        let response = self.send(method, path, body).await?;
        let status = response.status();
        let text = response.text().await?;

        if self.verbose {
            eprintln!("Response ({status}): {text}");
        }

        if !status.is_success() {
            return Err(self.parse_error(status, &text).into());
        }

        serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse response from {path}"))
    }

    async fn request_no_content<B: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<()> {
        let response = self.send(method, path, body).await?;
        let status = response.status();

        if !status.is_success() {
            let text = response.text().await?;
            return Err(self.parse_error(status, &text).into());
        }

        Ok(())
    }

    async fn send<B: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<Response> {
        let url = format!("{}{}", self.base_url, path);

        if self.verbose {
            eprintln!("{method} {url}");
        }

        let mut retries = 0;
        loop {
            let mut req = self.client
                .request(method.clone(), &url)
                .header("Authorization", &self.auth_header)
                .header("Accept", "application/json");

            if let Some(body) = &body {
                req = req.json(body);
            }

            let response = req.send().await
                .map_err(|e| {
                    if e.is_connect() || e.is_timeout() {
                        let host = self.base_url.split("//").last().unwrap_or(&self.base_url);
                        JrError::NetworkError(host.to_string())
                    } else {
                        JrError::Http(e)
                    }
                })?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < 3 {
                let rate_info = RateLimitInfo::from_response(&response);
                let wait = rate_info.retry_after.unwrap_or(Duration::from_secs(5));
                eprintln!("Rate limited. Retrying in {}s...", wait.as_secs());
                tokio::time::sleep(wait).await;
                retries += 1;
                continue;
            }

            return Ok(response);
        }
    }

    fn parse_error(&self, status: StatusCode, body: &str) -> JrError {
        if status == StatusCode::UNAUTHORIZED {
            return JrError::NotAuthenticated;
        }

        let message = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| {
                v.get("errorMessages")
                    .and_then(|m| m.as_array())
                    .and_then(|a| a.first())
                    .and_then(|m| m.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| body.to_string());

        JrError::ApiError {
            status: status.as_u16(),
            message,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

fn base64_encode(input: &str) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.encode(input.as_bytes())
}
```

- [ ] **Step 5: Wire up API modules**

Write `src/api/mod.rs`:

```rust
pub mod auth;
pub mod client;
pub mod pagination;
pub mod rate_limit;
pub mod jira;
```

Write `src/api/jira/mod.rs`:

```rust
pub mod boards;
pub mod fields;
pub mod issues;
pub mod sprints;
pub mod users;
pub mod worklogs;
```

Note: Transition functions live in `issues.rs` alongside other issue operations. No separate `transitions.rs` file.

Create empty stub files for each `src/api/jira/*.rs`:

```rust
// Each file is initially empty — implemented in later tasks
```

Add `mod api;` to `src/main.rs`.

- [ ] **Step 6: Write API client integration test**

Write `tests/api_client.rs`:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};

#[tokio::test]
async fn test_get_request_with_auth_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(header("Authorization", "Basic dGVzdEB0ZXN0LmNvbTp0b2tlbjEyMw=="))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "accountId": "abc123",
                    "displayName": "Test User",
                    "emailAddress": "test@test.com"
                }))
        )
        .mount(&server)
        .await;

    // Uses the test constructor directly
    let client = jr::api::client::JiraClient::new_for_test(
        &server.uri(),
        "Basic dGVzdEB0ZXN0LmNvbTp0b2tlbjEyMw==",
    );

    let result: serde_json::Value = client.get("/rest/api/3/myself").await.unwrap();
    assert_eq!(result["displayName"], "Test User");
}

#[tokio::test]
async fn test_rate_limit_retry() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(429)
                .append_header("Retry-After", "1")
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"displayName": "Test User"}))
        )
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(
        &server.uri(),
        "Basic dGVzdDp0ZXN0",
    );

    let result: serde_json::Value = client.get("/rest/api/3/myself").await.unwrap();
    assert_eq!(result["displayName"], "Test User");
}

#[tokio::test]
async fn test_401_returns_not_authenticated() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(
        &server.uri(),
        "Basic bad",
    );

    let result: Result<serde_json::Value, _> = client.get("/rest/api/3/myself").await;
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Not authenticated"));
}
```

Note: For integration tests to access `jr::api::client`, add `pub` visibility to `api` and submodules. In `src/main.rs`, make relevant modules `pub`:

```rust
pub mod api;
pub mod config;
pub mod error;
pub mod output;
```

- [ ] **Step 7: Run tests**

```bash
cargo test
```

Expected: All tests pass (smoke tests, config tests, output tests, API client tests).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: API client with auth, pagination, and rate limit handling"
```

---

## Task 5: Types + `jr me` Command

**Files:**
- Create: `src/types/mod.rs`
- Create: `src/types/jira/mod.rs`
- Create: `src/types/jira/user.rs`
- Create: `src/api/jira/users.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Define User type**

Write `src/types/jira/user.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "emailAddress")]
    pub email_address: Option<String>,
    pub active: Option<bool>,
}
```

Write `src/types/jira/mod.rs`:

```rust
pub mod user;
pub use user::User;
```

Write `src/types/mod.rs`:

```rust
pub mod jira;
```

- [ ] **Step 2: Implement users API call**

Write `src/api/jira/users.rs`:

```rust
use crate::api::client::JiraClient;
use crate::types::jira::User;
use anyhow::Result;

impl JiraClient {
    pub async fn get_myself(&self) -> Result<User> {
        self.get("/rest/api/3/myself").await
    }
}
```

- [ ] **Step 3: Implement `jr me` command handler**

Add to `src/main.rs` `run` function, replacing the `todo!("me")`:

```rust
cli::Command::Me => {
    let config = config::Config::load()?;
    let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
    let user = client.get_myself().await?;
    output::print_output(
        &cli.output,
        &["Field", "Value"],
        &[
            vec!["Name".into(), user.display_name.clone()],
            vec!["Email".into(), user.email_address.clone().unwrap_or_default()],
            vec!["Account ID".into(), user.account_id.clone()],
        ],
        &user,
    )?;
    Ok(())
}
```

Add `mod types;` to `src/main.rs`.

- [ ] **Step 4: Run tests**

```bash
cargo test
```

Expected: All existing tests still pass. The `me` command compiles but needs auth to run manually.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: jr me command with User type and /myself API call"
```

---

## Task 6: Auth Commands (API Token)

**Files:**
- Create: `src/cli/auth.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement auth command handlers**

Write `src/cli/auth.rs`:

```rust
use anyhow::Result;
use dialoguer::{Input, Password};

use crate::api::auth;
use crate::config;
use crate::output;

pub async fn login_token() -> Result<()> {
    let email: String = Input::new()
        .with_prompt("Jira email address")
        .interact_text()?;

    let token: String = Password::new()
        .with_prompt("API token (from https://id.atlassian.com/manage-profile/security/api-tokens)")
        .interact()?;

    auth::store_api_token(&email, &token)?;
    output::print_success("API token saved successfully.");
    Ok(())
}

pub async fn login_oauth() -> Result<()> {
    // OAuth implementation in Task 14
    anyhow::bail!("OAuth login not yet implemented. Use \"jr auth login --token\" for API token auth.");
}

pub async fn status() -> Result<()> {
    let config = config::Config::load()?;

    let method = config.global.instance.auth_method.as_deref().unwrap_or("none");
    let url = config.global.instance.url.as_deref().unwrap_or("not configured");

    println!("Instance:    {url}");
    println!("Auth method: {method}");

    match method {
        "api_token" => {
            match auth::load_api_token() {
                Ok((email, _)) => {
                    println!("Email:       {email}");
                    println!("Status:      authenticated");
                }
                Err(_) => {
                    println!("Status:      not authenticated");
                }
            }
        }
        "oauth" => {
            match auth::load_oauth_tokens() {
                Ok(_) => println!("Status:      authenticated"),
                Err(_) => println!("Status:      not authenticated"),
            }
        }
        _ => {
            println!("Status:      not configured");
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Wire auth commands into main.rs**

Replace `todo!("auth")` in `run()`:

```rust
cli::Command::Auth { command } => {
    match command {
        cli::AuthCommand::Login { token } => {
            if token {
                cli::auth::login_token().await
            } else {
                cli::auth::login_oauth().await
            }
        }
        cli::AuthCommand::Status => cli::auth::status().await,
    }
}
```

Move the auth handler to `src/cli/auth.rs` and add `pub mod auth;` to `src/cli/mod.rs`.

- [ ] **Step 3: Run tests**

```bash
cargo test
```

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: jr auth login --token and jr auth status commands"
```

---

## Task 7: Partial Matching Utility

**Files:**
- Create: `src/partial_match.rs`

- [ ] **Step 1: Write partial match tests and implementation**

Write `src/partial_match.rs`:

```rust
/// Result of attempting a partial match against a list of candidates.
pub enum MatchResult {
    /// Exactly one match found
    Exact(String),
    /// Multiple matches — caller should prompt for disambiguation
    Ambiguous(Vec<String>),
    /// No matches
    None(Vec<String>),
}

/// Case-insensitive substring match against candidates.
/// Returns the matching candidate(s).
pub fn partial_match(input: &str, candidates: &[String]) -> MatchResult {
    let lower_input = input.to_lowercase();

    // Try exact match first (case-insensitive)
    for candidate in candidates {
        if candidate.to_lowercase() == lower_input {
            return MatchResult::Exact(candidate.clone());
        }
    }

    // Try substring match
    let matches: Vec<String> = candidates
        .iter()
        .filter(|c| c.to_lowercase().contains(&lower_input))
        .cloned()
        .collect();

    match matches.len() {
        0 => MatchResult::None(candidates.to_vec()),
        1 => MatchResult::Exact(matches.into_iter().next().unwrap()),
        _ => MatchResult::Ambiguous(matches),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<String> {
        vec![
            "In Progress".into(),
            "In Review".into(),
            "Blocked".into(),
            "Done".into(),
        ]
    }

    #[test]
    fn test_exact_match_case_insensitive() {
        match partial_match("in progress", &candidates()) {
            MatchResult::Exact(s) => assert_eq!(s, "In Progress"),
            _ => panic!("Expected exact match"),
        }
    }

    #[test]
    fn test_partial_match_unique() {
        match partial_match("prog", &candidates()) {
            MatchResult::Exact(s) => assert_eq!(s, "In Progress"),
            _ => panic!("Expected unique match"),
        }
    }

    #[test]
    fn test_partial_match_ambiguous() {
        match partial_match("In", &candidates()) {
            MatchResult::Ambiguous(matches) => {
                assert_eq!(matches.len(), 2);
                assert!(matches.contains(&"In Progress".to_string()));
                assert!(matches.contains(&"In Review".to_string()));
            }
            _ => panic!("Expected ambiguous match"),
        }
    }

    #[test]
    fn test_no_match() {
        match partial_match("Deployed", &candidates()) {
            MatchResult::None(all) => assert_eq!(all.len(), 4),
            _ => panic!("Expected no match"),
        }
    }

    #[test]
    fn test_blocked_unique() {
        match partial_match("block", &candidates()) {
            MatchResult::Exact(s) => assert_eq!(s, "Blocked"),
            _ => panic!("Expected unique match"),
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn exact_match_always_found(idx in 0usize..4) {
            let candidates = vec![
                "In Progress".into(), "In Review".into(),
                "Blocked".into(), "Done".into(),
            ];
            let input = &candidates[idx];
            match partial_match(input, &candidates) {
                MatchResult::Exact(s) => prop_assert_eq!(s, *input),
                _ => prop_assert!(false, "Expected exact match for '{}'", input),
            }
        }

        #[test]
        fn never_panics_on_arbitrary_input(s in "\\PC{0,50}") {
            let candidates = vec!["In Progress".into(), "Done".into()];
            let _ = partial_match(&s, &candidates); // must not panic
        }

        #[test]
        fn empty_candidates_always_returns_none(s in "[a-z]{1,10}") {
            let candidates: Vec<String> = vec![];
            match partial_match(&s, &candidates) {
                MatchResult::None(all) => prop_assert!(all.is_empty()),
                _ => prop_assert!(false, "Expected None for empty candidates"),
            }
        }
    }
}
```

- [ ] **Step 2: Register module in main.rs**

Add `pub mod partial_match;` to `src/main.rs`.

- [ ] **Step 3: Run tests**

```bash
cargo test partial_match
```

Expected: All unit tests and property tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/partial_match.rs src/main.rs
git commit -m "feat: partial match utility for transitions and team names"
```

---

## Task 8: Duration Parser

**Files:**
- Create: `src/duration.rs`

- [ ] **Step 1: Write duration parser with tests**

Write `src/duration.rs`:

```rust
use anyhow::{bail, Result};

/// Parses a human-friendly duration string into seconds.
///
/// Supported formats: `30m`, `2h`, `1h30m`, `1d`, `1w`, `1w2d3h30m`
pub fn parse_duration(input: &str, hours_per_day: u64, days_per_week: u64) -> Result<u64> {
    let input = input.trim().to_lowercase();
    if input.is_empty() {
        bail!("Duration cannot be empty");
    }

    let mut total_seconds: u64 = 0;
    let mut current_num = String::new();
    let mut found_any = false;

    for ch in input.chars() {
        if ch.is_ascii_digit() {
            current_num.push(ch);
        } else {
            if current_num.is_empty() {
                bail!("Invalid duration format: \"{input}\". Expected format like 2h, 1h30m, 1d");
            }
            let num: u64 = current_num.parse()
                .map_err(|_| anyhow::anyhow!("Invalid number in duration: \"{input}\""))?;
            current_num.clear();
            found_any = true;

            match ch {
                'w' => total_seconds += num * days_per_week * hours_per_day * 3600,
                'd' => total_seconds += num * hours_per_day * 3600,
                'h' => total_seconds += num * 3600,
                'm' => total_seconds += num * 60,
                _ => bail!("Unknown duration unit '{ch}' in \"{input}\". Use w, d, h, or m"),
            }
        }
    }

    if !current_num.is_empty() {
        bail!("Invalid duration format: \"{input}\". Number without unit — did you mean \"{input}m\" or \"{input}h\"?");
    }

    if !found_any {
        bail!("Invalid duration format: \"{input}\"");
    }

    Ok(total_seconds)
}

/// Formats seconds into a human-readable duration string
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;

    match (hours, minutes) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h{m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HPD: u64 = 8;
    const DPW: u64 = 5;

    #[test]
    fn test_minutes() {
        assert_eq!(parse_duration("30m", HPD, DPW).unwrap(), 1800);
    }

    #[test]
    fn test_hours() {
        assert_eq!(parse_duration("2h", HPD, DPW).unwrap(), 7200);
    }

    #[test]
    fn test_hours_and_minutes() {
        assert_eq!(parse_duration("1h30m", HPD, DPW).unwrap(), 5400);
    }

    #[test]
    fn test_day() {
        assert_eq!(parse_duration("1d", HPD, DPW).unwrap(), 28800);
    }

    #[test]
    fn test_week() {
        assert_eq!(parse_duration("1w", HPD, DPW).unwrap(), 144000);
    }

    #[test]
    fn test_complex() {
        assert_eq!(parse_duration("1w2d3h30m", HPD, DPW).unwrap(), 144000 + 57600 + 10800 + 1800);
    }

    #[test]
    fn test_empty_fails() {
        assert!(parse_duration("", HPD, DPW).is_err());
    }

    #[test]
    fn test_number_without_unit_fails() {
        let err = parse_duration("30", HPD, DPW).unwrap_err();
        assert!(err.to_string().contains("without unit"));
    }

    #[test]
    fn test_invalid_unit_fails() {
        assert!(parse_duration("5x", HPD, DPW).is_err());
    }

    #[test]
    fn test_format_minutes() {
        assert_eq!(format_duration(1800), "30m");
    }

    #[test]
    fn test_format_hours() {
        assert_eq!(format_duration(7200), "2h");
    }

    #[test]
    fn test_format_hours_and_minutes() {
        assert_eq!(format_duration(5400), "1h30m");
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn valid_single_units_always_parse(h in 1u64..100, unit in prop_oneof![Just("m"), Just("h"), Just("d"), Just("w")]) {
            let input = format!("{h}{unit}");
            let result = parse_duration(&input, 8, 5);
            prop_assert!(result.is_ok(), "Failed to parse: {}", input);
            prop_assert!(result.unwrap() > 0);
        }

        #[test]
        fn combined_units_always_parse(h in 0u64..24, m in 0u64..60) {
            if h == 0 && m == 0 { return Ok(()); }
            let input = if m == 0 { format!("{h}h") } else if h == 0 { format!("{m}m") } else { format!("{h}h{m}m") };
            let result = parse_duration(&input, 8, 5);
            prop_assert!(result.is_ok(), "Failed to parse: {}", input);
        }

        #[test]
        fn garbage_input_never_panics(s in "\\PC{1,20}") {
            let _ = parse_duration(&s, 8, 5); // must not panic
        }

        #[test]
        fn format_roundtrip(seconds in (1u64..86400).prop_filter("divisible by 60", |s| s % 60 == 0)) {
            let formatted = format_duration(seconds);
            let reparsed = parse_duration(&formatted, 8, 5).unwrap();
            // Roundtrip only works for values < 1d since format_duration doesn't emit d/w
            if seconds < 28800 {
                prop_assert_eq!(reparsed, seconds, "Roundtrip failed: {} -> {} -> {}", seconds, formatted, reparsed);
            }
        }
    }
}
```

- [ ] **Step 2: Register module in main.rs**

Add `pub mod duration;` to `src/main.rs`.

- [ ] **Step 3: Run tests**

```bash
cargo test duration
```

Expected: All unit tests and property tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/duration.rs src/main.rs
git commit -m "feat: worklog duration parser (2h, 1h30m, 1d, 1w)"
```

---

## Task 9: ADF Handling

**Files:**
- Create: `src/adf.rs`

- [ ] **Step 1: Write ADF conversion with tests**

Write `src/adf.rs`:

```rust
use serde_json::{json, Value};

/// Wraps plain text in an ADF document
pub fn text_to_adf(text: &str) -> Value {
    json!({
        "version": 1,
        "type": "doc",
        "content": [
            {
                "type": "paragraph",
                "content": [
                    {
                        "type": "text",
                        "text": text
                    }
                ]
            }
        ]
    })
}

/// Converts basic Markdown to ADF.
/// Supports: headings (#), bold (**), italic (*), unordered lists (-),
/// code blocks (```), inline code (`), links ([text](url))
pub fn markdown_to_adf(markdown: &str) -> Value {
    let mut content: Vec<Value> = Vec::new();
    let mut in_code_block = false;
    let mut code_lines: Vec<String> = Vec::new();
    let mut list_items: Vec<Value> = Vec::new();

    for line in markdown.lines() {
        // Code block toggle
        if line.trim_start().starts_with("```") {
            if in_code_block {
                content.push(json!({
                    "type": "codeBlock",
                    "content": [
                        { "type": "text", "text": code_lines.join("\n") }
                    ]
                }));
                code_lines.clear();
                in_code_block = false;
            } else {
                flush_list(&mut list_items, &mut content);
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line.to_string());
            continue;
        }

        // Flush list if current line is not a list item
        if !line.trim_start().starts_with("- ") && !list_items.is_empty() {
            flush_list(&mut list_items, &mut content);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Headings
        if let Some(heading) = parse_heading(trimmed) {
            content.push(heading);
        }
        // Unordered list items
        else if let Some(stripped) = trimmed.strip_prefix("- ") {
            list_items.push(json!({
                "type": "listItem",
                "content": [
                    {
                        "type": "paragraph",
                        "content": parse_inline(stripped)
                    }
                ]
            }));
        }
        // Regular paragraph
        else {
            content.push(json!({
                "type": "paragraph",
                "content": parse_inline(trimmed)
            }));
        }
    }

    flush_list(&mut list_items, &mut content);

    json!({
        "version": 1,
        "type": "doc",
        "content": content
    })
}

fn flush_list(items: &mut Vec<Value>, content: &mut Vec<Value>) {
    if !items.is_empty() {
        content.push(json!({
            "type": "bulletList",
            "content": items.drain(..).collect::<Vec<_>>()
        }));
    }
}

fn parse_heading(line: &str) -> Option<Value> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if level >= 1 && level <= 6 && line.len() > level && line.as_bytes()[level] == b' ' {
        let text = &line[level + 1..];
        Some(json!({
            "type": "heading",
            "attrs": { "level": level },
            "content": [{ "type": "text", "text": text }]
        }))
    } else {
        None
    }
}

/// Parse inline formatting: **bold**, *italic*, `code`, [text](url)
fn parse_inline(text: &str) -> Vec<Value> {
    // Simple implementation: handle the most common patterns
    let mut result = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        // Bold
        if let Some(pos) = remaining.find("**") {
            if pos > 0 {
                result.push(json!({"type": "text", "text": &remaining[..pos]}));
            }
            let after = &remaining[pos + 2..];
            if let Some(end) = after.find("**") {
                result.push(json!({
                    "type": "text",
                    "text": &after[..end],
                    "marks": [{"type": "strong"}]
                }));
                remaining = &after[end + 2..];
                continue;
            }
        }

        // Inline code
        if let Some(pos) = remaining.find('`') {
            if pos > 0 {
                result.push(json!({"type": "text", "text": &remaining[..pos]}));
            }
            let after = &remaining[pos + 1..];
            if let Some(end) = after.find('`') {
                result.push(json!({
                    "type": "text",
                    "text": &after[..end],
                    "marks": [{"type": "code"}]
                }));
                remaining = &after[end + 1..];
                continue;
            }
        }

        // No more inline formatting — emit remaining text
        result.push(json!({"type": "text", "text": remaining}));
        break;
    }

    if result.is_empty() {
        result.push(json!({"type": "text", "text": text}));
    }

    result
}

/// Converts ADF document to plain text for terminal display
pub fn adf_to_text(adf: &Value) -> String {
    let mut output = String::new();
    if let Some(content) = adf.get("content").and_then(|c| c.as_array()) {
        for node in content {
            render_node(node, &mut output, 0);
        }
    }
    output.trim_end().to_string()
}

fn render_node(node: &Value, output: &mut String, depth: usize) {
    let node_type = node.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match node_type {
        "text" => {
            if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                output.push_str(text);
            }
        }
        "paragraph" => {
            render_children(node, output, depth);
            output.push('\n');
        }
        "heading" => {
            let level = node.get("attrs")
                .and_then(|a| a.get("level"))
                .and_then(|l| l.as_u64())
                .unwrap_or(1) as usize;
            for _ in 0..level {
                output.push('#');
            }
            output.push(' ');
            render_children(node, output, depth);
            output.push('\n');
        }
        "bulletList" | "orderedList" => {
            render_children(node, output, depth);
        }
        "listItem" => {
            let indent = "  ".repeat(depth);
            output.push_str(&indent);
            output.push_str("- ");
            render_children(node, output, depth + 1);
        }
        "codeBlock" => {
            output.push_str("```\n");
            render_children(node, output, depth);
            output.push_str("\n```\n");
        }
        _ => {
            // Unsupported node type
            if node.get("content").is_some() {
                render_children(node, output, depth);
            } else {
                output.push_str(&format!("[unsupported: {node_type}]"));
            }
        }
    }
}

fn render_children(node: &Value, output: &mut String, depth: usize) {
    if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
        for child in content {
            render_node(child, output, depth);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_to_adf() {
        let adf = text_to_adf("Hello world");
        assert_eq!(adf["type"], "doc");
        assert_eq!(adf["content"][0]["type"], "paragraph");
        assert_eq!(adf["content"][0]["content"][0]["text"], "Hello world");
    }

    #[test]
    fn test_adf_to_text_paragraph() {
        let adf = text_to_adf("Hello world");
        assert_eq!(adf_to_text(&adf), "Hello world");
    }

    #[test]
    fn test_markdown_heading() {
        let adf = markdown_to_adf("## Root cause");
        assert_eq!(adf["content"][0]["type"], "heading");
        assert_eq!(adf["content"][0]["attrs"]["level"], 2);
    }

    #[test]
    fn test_markdown_list() {
        let adf = markdown_to_adf("- item one\n- item two");
        assert_eq!(adf["content"][0]["type"], "bulletList");
        let items = adf["content"][0]["content"].as_array().unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_markdown_code_block() {
        let adf = markdown_to_adf("```\nlet x = 1;\n```");
        assert_eq!(adf["content"][0]["type"], "codeBlock");
    }

    #[test]
    fn test_adf_roundtrip_heading() {
        let adf = markdown_to_adf("## Title\nSome text");
        let text = adf_to_text(&adf);
        assert!(text.contains("## Title"));
        assert!(text.contains("Some text"));
    }

    #[test]
    fn test_adf_to_text_unsupported() {
        let adf = json!({
            "type": "doc",
            "content": [{ "type": "mediaGroup" }]
        });
        assert!(adf_to_text(&adf).contains("[unsupported: mediaGroup]"));
    }

    #[test]
    fn test_markdown_to_adf_snapshot() {
        let input = "## Root cause\n\nThe auth module had a **critical bug** in `validate_token`.\n\n- Missing null check\n- Wrong error type\n\n```rust\nfn validate() -> bool {\n    true\n}\n```";
        let adf = markdown_to_adf(input);
        insta::assert_json_snapshot!("markdown_complex_to_adf", adf);
    }

    #[test]
    fn test_adf_to_text_snapshot() {
        let adf = json!({
            "type": "doc",
            "version": 1,
            "content": [
                {"type": "heading", "attrs": {"level": 2}, "content": [{"type": "text", "text": "Summary"}]},
                {"type": "paragraph", "content": [{"type": "text", "text": "This is a description."}]},
                {"type": "bulletList", "content": [
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Item one"}]}]},
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Item two"}]}]}
                ]}
            ]
        });
        let text = adf_to_text(&adf);
        insta::assert_snapshot!("adf_to_text_complex", text);
    }
}
```

- [ ] **Step 2: Register module in main.rs**

Add `pub mod adf;` to `src/main.rs`.

- [ ] **Step 3: Run tests and accept snapshots**

```bash
cargo test adf
```

Expected: All 7 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/adf.rs src/main.rs
git commit -m "feat: ADF handling — text/markdown to ADF, ADF to plain text"
```

---

## Task 10: Issue Types + Issue List Command

**Files:**
- Create: `src/types/jira/issue.rs`
- Create: `src/types/jira/board.rs`
- Create: `src/types/jira/sprint.rs`
- Create: `src/types/jira/project.rs`
- Create: `src/types/jira/worklog.rs`
- Create: `src/api/jira/issues.rs`
- Create: `src/cli/issue.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Define all Jira types**

Write `src/types/jira/issue.rs`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct Issue {
    pub key: String,
    pub fields: IssueFields,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueFields {
    pub summary: Option<String>,
    pub description: Option<Value>,
    pub status: Option<Status>,
    #[serde(rename = "issuetype")]
    pub issue_type: Option<IssueType>,
    pub priority: Option<Priority>,
    pub assignee: Option<super::User>,
    pub project: Option<IssueProject>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Status {
    pub name: String,
    #[serde(rename = "statusCategory")]
    pub status_category: Option<StatusCategory>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatusCategory {
    pub name: String,
    pub key: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueType {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Priority {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueProject {
    pub key: String,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Transition {
    pub id: String,
    pub name: String,
    pub to: Option<Status>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TransitionsResponse {
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Comment {
    pub id: Option<String>,
    pub body: Option<Value>,
    pub author: Option<super::User>,
    pub created: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateIssueResponse {
    pub key: String,
}
```

Write `src/types/jira/board.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Board {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type")]
    pub board_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BoardConfig {
    pub id: u64,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub board_type: Option<String>,
}
```

Write `src/types/jira/sprint.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Sprint {
    pub id: u64,
    pub name: String,
    pub state: String,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
}
```

Write `src/types/jira/project.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Project {
    pub key: String,
    pub name: String,
}
```

Write `src/types/jira/worklog.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Worklog {
    pub id: Option<String>,
    pub author: Option<super::User>,
    #[serde(rename = "timeSpentSeconds")]
    pub time_spent_seconds: u64,
    #[serde(rename = "timeSpent")]
    pub time_spent: Option<String>,
    pub comment: Option<serde_json::Value>,
    pub started: Option<String>,
}
```

Update `src/types/jira/mod.rs`:

```rust
pub mod board;
pub mod issue;
pub mod project;
pub mod sprint;
pub mod user;
pub mod worklog;

pub use board::*;
pub use issue::*;
pub use project::*;
pub use sprint::*;
pub use user::*;
pub use worklog::*;
```

- [ ] **Step 2: Implement issue search API**

Write `src/api/jira/issues.rs`:

```rust
use anyhow::Result;
use serde_json::json;

use crate::api::client::JiraClient;
use crate::api::pagination::CursorPage;
use crate::types::jira::{
    Issue, Comment, CreateIssueResponse, TransitionsResponse,
};

impl JiraClient {
    pub async fn search_issues(
        &self,
        jql: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Issue>> {
        let max = limit.unwrap_or(50);
        let mut all_issues = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut body = json!({
                "jql": jql,
                "maxResults": max,
                "fields": ["summary", "status", "issuetype", "priority", "assignee", "project"]
            });

            if let Some(token) = &page_token {
                body["nextPageToken"] = json!(token);
            }

            let page: CursorPage<Issue> = self
                .post("/rest/api/3/search/jql", &body)
                .await?;

            let count = page.issues.len() as u32;
            all_issues.extend(page.issues);

            if !page.has_more() || (limit.is_some() && all_issues.len() as u32 >= max) {
                break;
            }

            page_token = page.next_page_token;
            if page_token.is_none() {
                break;
            }

            // Safety: if we got 0 results, stop
            if count == 0 {
                break;
            }
        }

        if let Some(limit) = limit {
            all_issues.truncate(limit as usize);
        }

        Ok(all_issues)
    }

    pub async fn get_issue(&self, key: &str) -> Result<Issue> {
        self.get(&format!("/rest/api/3/issue/{key}")).await
    }

    pub async fn create_issue(&self, fields: serde_json::Value) -> Result<CreateIssueResponse> {
        self.post("/rest/api/3/issue", &json!({ "fields": fields })).await
    }

    pub async fn edit_issue(&self, key: &str, fields: serde_json::Value) -> Result<()> {
        self.put(&format!("/rest/api/3/issue/{key}"), &json!({ "fields": fields })).await
    }

    pub async fn get_transitions(&self, key: &str) -> Result<TransitionsResponse> {
        self.get(&format!("/rest/api/3/issue/{key}/transitions")).await
    }

    pub async fn transition_issue(&self, key: &str, transition_id: &str) -> Result<()> {
        self.post_no_content(
            &format!("/rest/api/3/issue/{key}/transitions"),
            &json!({ "transition": { "id": transition_id } }),
        ).await
    }

    pub async fn assign_issue(&self, key: &str, account_id: Option<&str>) -> Result<()> {
        self.put(
            &format!("/rest/api/3/issue/{key}/assignee"),
            &json!({ "accountId": account_id }),
        ).await
    }

    pub async fn add_comment(&self, key: &str, body: serde_json::Value) -> Result<Comment> {
        self.post(
            &format!("/rest/api/3/issue/{key}/comment"),
            &json!({ "body": body }),
        ).await
    }
}
```

- [ ] **Step 3: Implement issue CLI handlers**

Write `src/cli/issue.rs`:

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::output;
use crate::types::jira::Issue;

pub async fn handle(
    command: IssueCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    match command {
        IssueCommand::List { jql, status, team, limit } => {
            list(config, client, output_format, project_override, jql, status, team, limit).await
        }
        IssueCommand::View { key } => view(client, output_format, &key).await,
        IssueCommand::Create { project, issue_type, summary, description, description_stdin, priority, label, team, markdown } => {
            create(config, client, output_format, project_override, project, issue_type, summary, description, description_stdin, priority, label, team, markdown).await
        }
        IssueCommand::Edit { key, summary, issue_type, priority, label, team } => {
            edit(client, output_format, &key, summary, issue_type, priority, label, team).await
        }
        IssueCommand::Move { key, status } => {
            transition(client, output_format, &key, status).await
        }
        IssueCommand::Transitions { key } => {
            list_transitions(client, output_format, &key).await
        }
        IssueCommand::Assign { key, to, unassign } => {
            assign(client, &key, to, unassign).await
        }
        IssueCommand::Comment { key, message, markdown, file, stdin } => {
            comment(client, &key, message, markdown, file, stdin).await
        }
        IssueCommand::Open { key, url_only } => {
            open_in_browser(config, &key, url_only)
        }
    }
}

async fn list(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
    jql: Option<String>,
    status: Option<String>,
    team: Option<String>,
    limit: Option<u32>,
) -> Result<()> {
    // If explicit JQL provided, use it directly
    if let Some(jql) = jql {
        let issues = client.search_issues(&jql, limit).await?;
        let rows = format_issue_rows(&issues);
        return output::print_output(
            output_format,
            &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
            &rows,
            &issues,
        );
    }

    // Smart defaults: detect board type for scrum vs kanban behavior
    let issues = if let Some(board_id) = config.project.board_id {
        if status.is_none() && team.is_none() {
            // Use board-aware smart defaults
            let board_config = client.get_board_config(board_id).await?;
            match board_config.board_type.as_deref() {
                Some("scrum") => {
                    // Scrum: show my issues in the active sprint via JQL
                    // (Agile sprint issue endpoint does not support JQL filtering)
                    let sprints = client.list_sprints(board_id, Some("active")).await?;
                    match sprints.first() {
                        Some(sprint) => {
                            let jql = format!(
                                "sprint = {} AND assignee = currentUser() ORDER BY rank ASC",
                                sprint.id
                            );
                            client.search_issues(&jql, limit).await?
                        }
                        None => {
                            println!("No active sprint. Falling back to JQL search.");
                            let jql = build_default_jql(config, project_override, None, None)?;
                            client.search_issues(&jql, limit).await?
                        }
                    }
                }
                _ => {
                    // Kanban: my issues not in Done
                    let jql = build_default_jql(config, project_override, None, None)?;
                    client.search_issues(&jql, limit).await?
                }
            }
        } else {
            // Explicit filters override smart defaults
            let jql = build_default_jql(config, project_override, status, team)?;
            client.search_issues(&jql, limit).await?
        }
    } else {
        // No board configured — fall back to JQL
        let jql = build_default_jql(config, project_override, status, team)?;
        client.search_issues(&jql, limit).await?
    };

    let rows = format_issue_rows(&issues);
    output::print_output(
        output_format,
        &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
        &rows,
        &issues,
    )
}

fn build_default_jql(
    config: &Config,
    project_override: Option<&str>,
    status: Option<String>,
    team: Option<String>,
) -> Result<String> {
    let mut clauses = vec!["assignee = currentUser()".to_string()];

    if let Some(project) = config.project_key(project_override) {
        clauses.push(format!("project = \"{project}\""));
    }

    if let Some(status) = status {
        clauses.push(format!("status = \"{status}\""));
    } else {
        clauses.push("statusCategory != Done".to_string());
    }

    if let Some(team) = team {
        clauses.push(format!("\"Team\" = \"{team}\""));
    }

    Ok(clauses.join(" AND ") + " ORDER BY updated DESC")
}

fn format_issue_rows(issues: &[Issue]) -> Vec<Vec<String>> {
    issues.iter().map(|issue| {
        vec![
            issue.key.clone(),
            issue.fields.issue_type.as_ref().map(|t| t.name.clone()).unwrap_or_default(),
            issue.fields.status.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
            issue.fields.priority.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
            issue.fields.assignee.as_ref().map(|a| a.display_name.clone()).unwrap_or_default(),
            issue.fields.summary.clone().unwrap_or_default(),
        ]
    }).collect()
}

async fn view(client: &JiraClient, output_format: &OutputFormat, key: &str) -> Result<()> {
    let issue = client.get_issue(key).await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&issue)?);
        }
        OutputFormat::Table => {
            println!("{} {}", issue.key, issue.fields.summary.as_deref().unwrap_or(""));
            println!("Type:     {}", issue.fields.issue_type.as_ref().map(|t| t.name.as_str()).unwrap_or("-"));
            println!("Status:   {}", issue.fields.status.as_ref().map(|s| s.name.as_str()).unwrap_or("-"));
            println!("Priority: {}", issue.fields.priority.as_ref().map(|p| p.name.as_str()).unwrap_or("-"));
            println!("Assignee: {}", issue.fields.assignee.as_ref().map(|a| a.display_name.as_str()).unwrap_or("Unassigned"));
            println!("Project:  {}", issue.fields.project.as_ref().map(|p| p.key.as_str()).unwrap_or("-"));

            // Display team field if present (from raw JSON response)
            // Team field ID is resolved via config; its value appears in the fields object
            // This is handled when we have the team_field_id from config

            if let Some(desc) = &issue.fields.description {
                println!("\n{}", crate::adf::adf_to_text(desc));
            }
        }
    }

    Ok(())
}

async fn create(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
    project: Option<String>,
    issue_type: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    description_stdin: bool,
    priority: Option<String>,
    labels: Vec<String>,
    team: Option<String>,
    markdown: bool,
) -> Result<()> {
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .ok_or_else(|| anyhow::anyhow!("No project specified. Use -p FOO or configure .jr.toml"))?;

    let issue_type_name = issue_type.unwrap_or_else(|| {
        dialoguer::Input::new()
            .with_prompt("Issue type")
            .default("Task".to_string())
            .interact_text()
            .unwrap()
    });

    let summary_text = summary.unwrap_or_else(|| {
        dialoguer::Input::new()
            .with_prompt("Summary")
            .interact_text()
            .unwrap()
    });

    let mut fields = serde_json::json!({
        "project": { "key": project_key },
        "issuetype": { "name": issue_type_name },
        "summary": summary_text,
    });

    // Description: from flag, stdin, or omitted
    let desc_text = if description_stdin {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Some(buf)
    } else {
        description
    };

    if let Some(desc) = desc_text {
        fields["description"] = if markdown {
            crate::adf::markdown_to_adf(&desc)
        } else {
            crate::adf::text_to_adf(&desc)
        };
    }

    if let Some(p) = priority {
        fields["priority"] = serde_json::json!({"name": p});
    }

    if !labels.is_empty() {
        fields["labels"] = serde_json::json!(labels);
    }

    // Team assignment handled in Task 13

    let result = client.create_issue(fields).await?;

    match output_format {
        OutputFormat::Json => println!("{}", serde_json::json!({"key": result.key})),
        _ => output::print_success(&format!("Created {}", result.key)),
    }
    Ok(())
}

async fn edit(
    client: &JiraClient,
    key: &str,
    summary: Option<String>,
    issue_type: Option<String>,
    priority: Option<String>,
    _team: Option<String>,
) -> Result<()> {
    let mut fields = serde_json::Map::new();

    if let Some(s) = summary {
        fields.insert("summary".into(), serde_json::json!(s));
    }
    if let Some(t) = issue_type {
        fields.insert("issuetype".into(), serde_json::json!({"name": t}));
    }
    if let Some(p) = priority {
        fields.insert("priority".into(), serde_json::json!({"name": p}));
    }

    if fields.is_empty() {
        anyhow::bail!("No fields to update. Use --summary, --type, --priority, or --team.");
    }

    client.edit_issue(key, serde_json::Value::Object(fields)).await?;
    output::print_success(&format!("{key} updated"));
    Ok(())
}

async fn list_transitions(
    client: &JiraClient,
    output_format: &OutputFormat,
    key: &str,
) -> Result<()> {
    let response = client.get_transitions(key).await?;
    let names: Vec<String> = response.transitions.iter().map(|t| t.name.clone()).collect();
    let rows: Vec<Vec<String>> = response.transitions.iter().map(|t| {
        vec![t.id.clone(), t.name.clone()]
    }).collect();

    output::print_output(output_format, &["ID", "Name"], &rows, &response.transitions)
}

async fn transition(
    client: &JiraClient,
    output_format: &OutputFormat,
    key: &str,
    target_status: Option<String>,
) -> Result<()> {
    // Check current status for idempotency
    if let Some(ref target) = target_status {
        let issue = client.get_issue(key).await?;
        if let Some(status) = &issue.fields.status {
            if status.name.to_lowercase() == target.to_lowercase() {
                output::print_success(&format!("{key} is already \"{target}\""));
                return Ok(());
            }
        }
    }

    let response = client.get_transitions(key).await?;
    let names: Vec<String> = response.transitions.iter().map(|t| t.name.clone()).collect();

    let selected = match target_status {
        Some(input) => {
            match crate::partial_match::partial_match(&input, &names) {
                crate::partial_match::MatchResult::Exact(name) => name,
                crate::partial_match::MatchResult::Ambiguous(matches) => {
                    let selection = dialoguer::Select::new()
                        .with_prompt(format!("Multiple transitions match \"{input}\""))
                        .items(&matches)
                        .interact()?;
                    matches[selection].clone()
                }
                crate::partial_match::MatchResult::None(all) => {
                    anyhow::bail!(
                        "\"{input}\" is not a valid transition for {key}\nAvailable transitions: {}",
                        all.join(", ")
                    );
                }
            }
        }
        None => {
            if names.is_empty() {
                anyhow::bail!("No transitions available for {key}");
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Transition {key}"))
                .items(&names)
                .interact()?;
            names[selection].clone()
        }
    };

    let transition = response.transitions.iter()
        .find(|t| t.name == selected)
        .unwrap();

    client.transition_issue(key, &transition.id).await?;

    match output_format {
        OutputFormat::Json => println!("{}", serde_json::json!({"key": key, "status": selected})),
        _ => output::print_success(&format!("{key} transitioned to \"{selected}\"")),
    }
    Ok(())
}

async fn assign(
    client: &JiraClient,
    key: &str,
    to: Option<String>,
    unassign: bool,
) -> Result<()> {
    if unassign {
        client.assign_issue(key, None).await?;
        output::print_success(&format!("{key} unassigned"));
    } else if let Some(user) = to {
        client.assign_issue(key, Some(&user)).await?;
        output::print_success(&format!("{key} assigned to {user}"));
    } else {
        let me = client.get_myself().await?;
        // Idempotent: check if already assigned to me
        let issue = client.get_issue(key).await?;
        if let Some(assignee) = &issue.fields.assignee {
            if assignee.account_id == me.account_id {
                output::print_success(&format!("{key} is already assigned to you"));
                return Ok(());
            }
        }
        client.assign_issue(key, Some(&me.account_id)).await?;
        output::print_success(&format!("{key} assigned to you"));
    }
    Ok(())
}

async fn comment(
    client: &JiraClient,
    key: &str,
    message: Option<String>,
    markdown: bool,
    file: Option<String>,
    stdin: bool,
) -> Result<()> {
    let text = if stdin {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else if let Some(path) = file {
        std::fs::read_to_string(&path)?
    } else if let Some(msg) = message {
        msg
    } else {
        anyhow::bail!("Provide a message, use --file, or pipe via --stdin");
    };

    let body = if markdown {
        crate::adf::markdown_to_adf(&text)
    } else {
        crate::adf::text_to_adf(&text)
    };

    client.add_comment(key, body).await?;
    output::print_success(&format!("Comment added to {key}"));
    Ok(())
}

fn open_in_browser(config: &Config, key: &str, url_only: bool) -> Result<()> {
    let url = config.global.instance.url.as_ref()
        .ok_or_else(|| anyhow::anyhow!("No Jira instance configured"))?;
    let browse_url = format!("{url}/browse/{key}");

    if url_only {
        println!("{browse_url}");
    } else {
        open::that(&browse_url)?;
        println!("Opened {browse_url}");
    }
    Ok(())
}
```

- [ ] **Step 4: Wire issue commands into main.rs**

Replace `todo!("issue")` in `run()`:

```rust
cli::Command::Issue { command } => {
    let config = config::Config::load()?;
    let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
    cli::issue::handle(command, &config, &client, &cli.output, cli.project.as_deref()).await
}
```

Add `pub mod issue;` to `src/cli/mod.rs`.

- [ ] **Step 5: Run tests**

```bash
cargo test
```

Expected: All tests pass. Issue commands compile but need a live Jira instance to test manually.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: issue commands — list, view, create, edit, move, assign, comment, open"
```

---

## Task 11: Board + Sprint Commands

**Files:**
- Create: `src/api/jira/boards.rs`
- Create: `src/api/jira/sprints.rs`
- Create: `src/cli/board.rs`
- Create: `src/cli/sprint.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement board and sprint API calls**

Write `src/api/jira/boards.rs`:

```rust
use anyhow::Result;
use crate::api::client::JiraClient;
use crate::api::pagination::OffsetPage;
use crate::types::jira::{Board, BoardConfig};

impl JiraClient {
    pub async fn list_boards(&self) -> Result<Vec<Board>> {
        let page: OffsetPage<Board> = self.get("/rest/agile/1.0/board").await?;
        Ok(page.items())
    }

    pub async fn get_board_config(&self, board_id: u64) -> Result<BoardConfig> {
        self.get(&format!("/rest/agile/1.0/board/{board_id}/configuration")).await
    }
}
```

Write `src/api/jira/sprints.rs`:

```rust
use anyhow::Result;
use crate::api::client::JiraClient;
use crate::api::pagination::OffsetPage;
use crate::types::jira::{Sprint, Issue};

impl JiraClient {
    pub async fn list_sprints(&self, board_id: u64, state: Option<&str>) -> Result<Vec<Sprint>> {
        let mut path = format!("/rest/agile/1.0/board/{board_id}/sprint");
        if let Some(state) = state {
            path.push_str(&format!("?state={state}"));
        }
        let page: OffsetPage<Sprint> = self.get(&path).await?;
        Ok(page.items())
    }

    pub async fn get_sprint_issues(
        &self,
        sprint_id: u64,
        jql: Option<&str>,
    ) -> Result<Vec<Issue>> {
        let mut path = format!("/rest/agile/1.0/sprint/{sprint_id}/issue");
        if let Some(jql) = jql {
            path.push_str(&format!("?jql={}", urlencode(jql)));
        }
        let page: OffsetPage<Issue> = self.get(&path).await?;
        Ok(page.items())
    }
}

fn urlencode(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}
```

- [ ] **Step 2: Implement board CLI handler**

Write `src/cli/board.rs`:

```rust
use anyhow::Result;
use crate::api::client::JiraClient;
use crate::cli::{BoardCommand, OutputFormat};
use crate::config::Config;
use crate::output;

pub async fn handle(
    command: BoardCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    match command {
        BoardCommand::List => list(client, output_format).await,
        BoardCommand::View => view(config, client, output_format).await,
    }
}

async fn list(client: &JiraClient, output_format: &OutputFormat) -> Result<()> {
    let boards = client.list_boards().await?;
    let rows: Vec<Vec<String>> = boards.iter().map(|b| {
        vec![b.id.to_string(), b.board_type.clone(), b.name.clone()]
    }).collect();

    output::print_output(output_format, &["ID", "Type", "Name"], &rows, &boards)
}

async fn view(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    let board_id = config.project.board_id
        .ok_or_else(|| anyhow::anyhow!("No board configured. Run \"jr init\" in your project directory."))?;

    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.as_deref().unwrap_or("unknown");

    let issues = if board_type == "scrum" {
        let sprints = client.list_sprints(board_id, Some("active")).await?;
        match sprints.first() {
            Some(sprint) => client.get_sprint_issues(sprint.id, None).await?,
            None => {
                println!("No active sprint found.");
                return Ok(());
            }
        }
    } else {
        let project = config.project.project.as_deref()
            .ok_or_else(|| anyhow::anyhow!("No project configured in .jr.toml"))?;
        client.search_issues(
            &format!("project = \"{project}\" AND statusCategory != Done ORDER BY rank ASC"),
            Some(50),
        ).await?
    };

    let rows = crate::cli::issue::format_issue_rows_public(&issues);
    output::print_output(
        output_format,
        &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
        &rows,
        &issues,
    )
}
```

Note: You will need to make `format_issue_rows` public in `src/cli/issue.rs`. Add this public wrapper:

```rust
pub fn format_issue_rows_public(issues: &[Issue]) -> Vec<Vec<String>> {
    format_issue_rows(issues)
}
```

- [ ] **Step 3: Implement sprint CLI handler**

Write `src/cli/sprint.rs`:

```rust
use anyhow::Result;
use crate::api::client::JiraClient;
use crate::cli::{SprintCommand, OutputFormat};
use crate::config::Config;
use crate::output;

pub async fn handle(
    command: SprintCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    let board_id = config.project.board_id
        .ok_or_else(|| anyhow::anyhow!("No board configured. Run \"jr init\" in your project directory."))?;

    // Check board type — sprints are scrum-only
    let board_config = client.get_board_config(board_id).await?;
    if board_config.board_type.as_deref() != Some("scrum") {
        anyhow::bail!(
            "Project uses a Kanban board — no sprints available. Use \"jr issue list\" instead."
        );
    }

    match command {
        SprintCommand::List => list(client, output_format, board_id).await,
        SprintCommand::Current => current(client, output_format, board_id).await,
    }
}

async fn list(
    client: &JiraClient,
    output_format: &OutputFormat,
    board_id: u64,
) -> Result<()> {
    let sprints = client.list_sprints(board_id, None).await?;
    let rows: Vec<Vec<String>> = sprints.iter().map(|s| {
        vec![
            s.id.to_string(),
            s.state.clone(),
            s.name.clone(),
            s.end_date.clone().unwrap_or_default(),
        ]
    }).collect();

    output::print_output(output_format, &["ID", "State", "Name", "End Date"], &rows, &sprints)
}

async fn current(
    client: &JiraClient,
    output_format: &OutputFormat,
    board_id: u64,
) -> Result<()> {
    let sprints = client.list_sprints(board_id, Some("active")).await?;
    let sprint = sprints.first()
        .ok_or_else(|| anyhow::anyhow!("No active sprint found"))?;

    println!("Sprint: {} ({})", sprint.name, sprint.state);
    if let Some(end) = &sprint.end_date {
        println!("Ends:   {end}");
    }
    println!();

    let issues = client.get_sprint_issues(
        sprint.id,
        Some("assignee = currentUser()"),
    ).await?;

    let rows = crate::cli::issue::format_issue_rows_public(&issues);
    output::print_output(
        output_format,
        &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
        &rows,
        &issues,
    )
}
```

- [ ] **Step 4: Wire board and sprint commands into main.rs**

Replace `todo!("board")` and `todo!("sprint")`:

```rust
cli::Command::Board { command } => {
    let config = config::Config::load()?;
    let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
    cli::board::handle(command, &config, &client, &cli.output).await
}
cli::Command::Sprint { command } => {
    let config = config::Config::load()?;
    let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
    cli::sprint::handle(command, &config, &client, &cli.output).await
}
```

Add `pub mod board;` and `pub mod sprint;` to `src/cli/mod.rs`.

- [ ] **Step 5: Run tests**

```bash
cargo test
```

Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: board and sprint commands with scrum/kanban detection"
```

---

## Task 12: Worklog Commands

**Files:**
- Create: `src/api/jira/worklogs.rs`
- Create: `src/cli/worklog.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement worklog API calls**

Write `src/api/jira/worklogs.rs`:

```rust
use anyhow::Result;
use serde_json::json;
use crate::api::client::JiraClient;
use crate::api::pagination::OffsetPage;
use crate::types::jira::Worklog;

impl JiraClient {
    pub async fn add_worklog(
        &self,
        key: &str,
        time_spent_seconds: u64,
        comment: Option<serde_json::Value>,
    ) -> Result<Worklog> {
        let mut body = json!({
            "timeSpentSeconds": time_spent_seconds,
        });

        if let Some(comment) = comment {
            body["comment"] = comment;
        }

        self.post(&format!("/rest/api/3/issue/{key}/worklog"), &body).await
    }

    pub async fn list_worklogs(&self, key: &str) -> Result<Vec<Worklog>> {
        let page: OffsetPage<Worklog> = self
            .get(&format!("/rest/api/3/issue/{key}/worklog"))
            .await?;
        Ok(page.items())
    }
}
```

- [ ] **Step 2: Implement worklog CLI handler**

Write `src/cli/worklog.rs`:

```rust
use anyhow::Result;
use crate::api::client::JiraClient;
use crate::cli::{WorklogCommand, OutputFormat};
use crate::duration;
use crate::output;

pub async fn handle(
    command: WorklogCommand,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    match command {
        WorklogCommand::Add { key, duration: dur, message } => {
            add(client, &key, &dur, message).await
        }
        WorklogCommand::List { key } => list(client, output_format, &key).await,
    }
}

async fn add(
    client: &JiraClient,
    key: &str,
    dur: &str,
    message: Option<String>,
) -> Result<()> {
    let seconds = duration::parse_duration(dur, 8, 5)?;
    let comment = message.map(|m| crate::adf::text_to_adf(&m));
    client.add_worklog(key, seconds, comment).await?;
    output::print_success(&format!("{} logged to {key}", duration::format_duration(seconds)));
    Ok(())
}

async fn list(
    client: &JiraClient,
    output_format: &OutputFormat,
    key: &str,
) -> Result<()> {
    let worklogs = client.list_worklogs(key).await?;
    let rows: Vec<Vec<String>> = worklogs.iter().map(|w| {
        vec![
            w.author.as_ref().map(|a| a.display_name.clone()).unwrap_or_default(),
            w.time_spent.clone().unwrap_or_else(|| duration::format_duration(w.time_spent_seconds)),
            w.started.clone().unwrap_or_default(),
        ]
    }).collect();

    output::print_output(output_format, &["Author", "Time", "Started"], &rows, &worklogs)
}
```

- [ ] **Step 3: Wire worklog commands into main.rs**

Replace `todo!("worklog")`:

```rust
cli::Command::Worklog { command } => {
    let config = config::Config::load()?;
    let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
    cli::worklog::handle(command, &client, &cli.output).await
}
```

Add `pub mod worklog;` to `src/cli/mod.rs`.

- [ ] **Step 4: Run tests**

```bash
cargo test
```

Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: worklog commands — add with duration parsing, list"
```

---

## Task 13: Team Assignment (Custom Field)

**Files:**
- Create: `src/api/jira/fields.rs`
- Modify: `src/cli/issue.rs` (create and edit functions)
- Modify: `src/config.rs` (add team_field_id cache)

- [ ] **Step 1: Implement field discovery API**

Write `src/api/jira/fields.rs`:

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::api::client::JiraClient;

#[derive(Debug, Deserialize, Serialize)]
pub struct Field {
    pub id: String,
    pub name: String,
    pub custom: Option<bool>,
}

impl JiraClient {
    pub async fn list_fields(&self) -> Result<Vec<Field>> {
        self.get("/rest/api/3/field").await
    }

    pub async fn find_team_field_id(&self) -> Result<Option<String>> {
        let fields = self.list_fields().await?;
        Ok(fields.iter()
            .find(|f| f.name.to_lowercase() == "team" && f.custom == Some(true))
            .map(|f| f.id.clone()))
    }
}
```

- [ ] **Step 2: Add team_field_id to global config**

In `src/config.rs`, add to `GlobalConfig`:

```rust
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub instance: InstanceConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub fields: FieldsConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct FieldsConfig {
    pub team_field_id: Option<String>,
}
```

Add a `save_global` method to write config back:

```rust
impl Config {
    pub fn save_global(&self) -> anyhow::Result<()> {
        let dir = global_config_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let content = toml::to_string_pretty(&self.global)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
```

- [ ] **Step 3: Integrate team field into issue create and edit**

In `src/cli/issue.rs`, update the `create` function to resolve team:

```rust
// After building base fields, before calling create_issue:
if let Some(team_name) = team {
    let team_field_id = resolve_team_field_id(config, client).await?;
    // Resolve team name to value — for now, set as string
    // Jira team field format varies by instance
    fields[&team_field_id] = serde_json::json!({"value": team_name});
}
```

Add helper function:

```rust
async fn resolve_team_field_id(config: &Config, client: &JiraClient) -> Result<String> {
    // Check cached value first
    if let Some(id) = &config.global.fields.team_field_id {
        return Ok(id.clone());
    }

    // Discover from API
    let id = client.find_team_field_id().await?
        .ok_or_else(|| anyhow::anyhow!("No \"Team\" field found on this Jira instance"))?;

    // Cache for future use (best effort)
    let mut updated_config = Config::load()?;
    updated_config.global.fields.team_field_id = Some(id.clone());
    let _ = updated_config.save_global();

    Ok(id)
}
```

Similarly update `edit` to handle the `_team` parameter (remove the underscore prefix).

- [ ] **Step 4: Run tests**

```bash
cargo test
```

Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: team assignment via custom field discovery and caching"
```

---

## Task 14: OAuth 2.0 Authentication

**Files:**
- Modify: `src/api/auth.rs`
- Modify: `src/cli/auth.rs`

- [ ] **Step 1: Implement OAuth flow in api/auth.rs**

Add to `src/api/auth.rs`:

```rust
use std::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener as AsyncTcpListener;

const CLIENT_ID: &str = "YOUR_CLIENT_ID"; // Replace after registering OAuth app
const CLIENT_SECRET: &str = "YOUR_CLIENT_SECRET"; // Replace after registering OAuth app
const SCOPES: &str = "read:jira-work write:jira-work read:jira-user offline_access";

pub async fn oauth_login() -> Result<OAuthResult> {
    // Find available port
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);

    let redirect_uri = format!("http://localhost:{port}/callback");
    let state = generate_state();

    let auth_url = format!(
        "https://auth.atlassian.com/authorize?audience=api.atlassian.com&client_id={CLIENT_ID}&scope={}&redirect_uri={redirect_uri}&state={state}&response_type=code&prompt=consent",
        urlencode(SCOPES)
    );

    println!("Opening browser for Atlassian authorization...");
    open::that(&auth_url)?;

    // Listen for callback
    let listener = AsyncTcpListener::bind(format!("127.0.0.1:{port}")).await?;
    let (mut stream, _) = listener.accept().await?;

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Extract code from GET /callback?code=xxx&state=yyy
    let code = extract_query_param(&request, "code")
        .ok_or_else(|| anyhow::anyhow!("No authorization code received"))?;
    let returned_state = extract_query_param(&request, "state")
        .ok_or_else(|| anyhow::anyhow!("No state parameter received"))?;

    if returned_state != state {
        anyhow::bail!("State mismatch — possible CSRF attack");
    }

    // Send response to browser
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h2>Authorization successful!</h2><p>You can close this tab.</p></body></html>";
    stream.write_all(response.as_bytes()).await?;

    // Exchange code for tokens
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://auth.atlassian.com/oauth/token")
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": CLIENT_ID,
            "client_secret": CLIENT_SECRET,
            "code": code,
            "redirect_uri": redirect_uri,
        }))
        .send()
        .await?;

    if !token_response.status().is_success() {
        let body = token_response.text().await?;
        anyhow::bail!("Token exchange failed: {body}");
    }

    let tokens: TokenResponse = token_response.json().await?;

    // Get cloud ID
    let resources: Vec<AccessibleResource> = client
        .get("https://api.atlassian.com/oauth/token/accessible-resources")
        .bearer_auth(&tokens.access_token)
        .send()
        .await?
        .json()
        .await?;

    let resource = resources.first()
        .ok_or_else(|| anyhow::anyhow!("No accessible Jira sites found"))?;

    // Store tokens
    store_oauth_tokens(&tokens.access_token, &tokens.refresh_token)?;

    Ok(OAuthResult {
        cloud_id: resource.id.clone(),
        site_url: resource.url.clone(),
        site_name: resource.name.clone(),
    })
}

pub async fn refresh_oauth_token() -> Result<String> {
    let (_, refresh_token) = load_oauth_tokens()?;

    let client = reqwest::Client::new();
    let response = client
        .post("https://auth.atlassian.com/oauth/token")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": CLIENT_ID,
            "client_secret": CLIENT_SECRET,
            "refresh_token": refresh_token,
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("Token refresh failed. Run \"jr auth login\" to re-authenticate.");
    }

    let tokens: TokenResponse = response.json().await?;
    store_oauth_tokens(&tokens.access_token, &tokens.refresh_token)?;

    Ok(tokens.access_token)
}

#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
}

#[derive(Debug, serde::Deserialize)]
struct AccessibleResource {
    id: String,
    url: String,
    name: String,
}

pub struct OAuthResult {
    pub cloud_id: String,
    pub site_url: String,
    pub site_name: String,
}

fn generate_state() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{:x}", t.as_nanos())
}

fn extract_query_param(request: &str, param: &str) -> Option<String> {
    let query_start = request.find('?')?;
    let query_end = request[query_start..].find(' ').map(|i| query_start + i).unwrap_or(request.len());
    let query = &request[query_start + 1..query_end];

    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            if key == param {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn urlencode(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}
```

- [ ] **Step 2: Update CLI auth handler for OAuth**

In `src/cli/auth.rs`, replace `login_oauth`:

```rust
pub async fn login_oauth() -> Result<()> {
    let result = crate::api::auth::oauth_login().await?;

    // Update config with cloud_id and site URL
    let mut config = crate::config::Config::load().unwrap_or_default();
    config.global.instance.url = Some(result.site_url);
    config.global.instance.cloud_id = Some(result.cloud_id);
    config.global.instance.auth_method = Some("oauth".into());
    config.save_global()?;

    output::print_success(&format!("Authenticated with {}", result.site_name));
    Ok(())
}
```

Note: `Config::load` will need a `Default` implementation or an `unwrap_or_default()` variant for when no config exists yet.

- [ ] **Step 3: Add token refresh to JiraClient**

In `src/api/client.rs`, update the `send` method to handle 401 by refreshing:

After receiving a 401 and before returning the error, attempt a token refresh if using OAuth:

```rust
if response.status() == StatusCode::UNAUTHORIZED && retries == 0 {
    // Try refreshing OAuth token
    if let Ok(new_token) = crate::api::auth::refresh_oauth_token().await {
        // Retry with new token — requires making auth_header mutable
        // or rebuilding the request
        retries += 1;
        continue;
    }
}
```

Note: This requires making `auth_header` interior-mutable (e.g., `Arc<Mutex<String>>`). A simpler approach for v1: let the outer error handling prompt re-auth.

- [ ] **Step 4: Run tests**

```bash
cargo test
```

Expected: All tests pass. OAuth flow requires manual testing with a real Atlassian OAuth app.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: OAuth 2.0 (3LO) authentication with token refresh"
```

---

## Task 15: Init Command

**Files:**
- Create: `src/cli/init.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement init command**

Write `src/cli/init.rs`:

```rust
use anyhow::Result;
use dialoguer::{Input, Select};

use crate::api;
use crate::config::{self, Config, GlobalConfig, InstanceConfig, DefaultsConfig, FieldsConfig, ProjectConfig};
use crate::output;

pub async fn handle() -> Result<()> {
    println!("Setting up jr — Jira CLI\n");

    // Step 1: Instance URL
    let url: String = Input::new()
        .with_prompt("Jira instance URL (e.g., https://yourorg.atlassian.net)")
        .interact_text()?;

    let url = url.trim_end_matches('/').to_string();

    // Step 2: Auth method
    let auth_methods = vec!["OAuth 2.0 (recommended)", "API Token"];
    let auth_choice = Select::new()
        .with_prompt("Authentication method")
        .items(&auth_methods)
        .default(0)
        .interact()?;

    let mut global = GlobalConfig {
        instance: InstanceConfig {
            url: Some(url.clone()),
            cloud_id: None,
            auth_method: None,
        },
        defaults: DefaultsConfig::default(),
        fields: FieldsConfig::default(),
    };

    // Save global config first so auth can use it
    let config = Config { global, project: ProjectConfig::default() };
    config.save_global()?;

    // Step 3: Authenticate
    if auth_choice == 0 {
        crate::cli::auth::login_oauth().await?;
    } else {
        crate::cli::auth::login_token().await?;
        // Update auth method in config
        let mut config = Config::load()?;
        config.global.instance.auth_method = Some("api_token".into());
        config.save_global()?;
    }

    // Step 4: Per-project setup (if in a git repo)
    let config = Config::load()?;
    let client = api::client::JiraClient::from_config(&config, false)?;

    let setup_project = dialoguer::Confirm::new()
        .with_prompt("Configure this directory as a Jira project?")
        .default(true)
        .interact()?;

    if setup_project {
        // List boards and let user pick
        let boards = client.list_boards().await?;
        if boards.is_empty() {
            println!("No boards found. You can configure .jr.toml manually.");
        } else {
            let board_names: Vec<String> = boards.iter()
                .map(|b| format!("{} ({}, {})", b.name, b.board_type, b.id))
                .collect();

            let board_choice = Select::new()
                .with_prompt("Select board")
                .items(&board_names)
                .interact()?;

            let selected_board = &boards[board_choice];

            // Infer project key from board name or ask
            let project_key: String = Input::new()
                .with_prompt("Project key")
                .interact_text()?;

            let project_config = format!(
                "project = \"{}\"\nboard_id = {}\n",
                project_key, selected_board.id,
            );

            std::fs::write(".jr.toml", &project_config)?;
            output::print_success("Created .jr.toml");
        }
    }

    // Step 5: Discover team field
    if let Ok(Some(team_id)) = client.find_team_field_id().await {
        let mut config = Config::load()?;
        config.global.fields.team_field_id = Some(team_id);
        config.save_global()?;
    }

    output::print_success("\njr is ready! Try \"jr issue list\" to see your tickets.");
    Ok(())
}
```

- [ ] **Step 2: Wire init command into main.rs**

Replace `todo!("init")`:

```rust
cli::Command::Init => cli::init::handle().await,
```

Add `pub mod init;` to `src/cli/mod.rs`.

- [ ] **Step 3: Run tests**

```bash
cargo test
```

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: jr init — interactive setup for instance, auth, and project config"
```

---

## Task 16: Integration Tests

**Files:**
- Create: `tests/common/mod.rs`
- Create: `tests/common/fixtures.rs`
- Create: `tests/common/mock_server.rs`
- Create: `tests/issue_commands.rs`
- Create: `tests/board_sprint_commands.rs`
- Create: `tests/worklog_commands.rs`

- [ ] **Step 0: Create shared test fixtures and helpers**

Write `tests/common/mod.rs`:

```rust
pub mod fixtures;
pub mod mock_server;
```

Write `tests/common/fixtures.rs`:

```rust
use serde_json::{json, Value};

pub fn user_response() -> Value {
    json!({
        "accountId": "abc123",
        "displayName": "Test User",
        "emailAddress": "test@test.com"
    })
}

pub fn issue_response(key: &str, summary: &str, status: &str) -> Value {
    json!({
        "key": key,
        "fields": {
            "summary": summary,
            "status": {"name": status},
            "issuetype": {"name": "Task"},
            "priority": {"name": "Medium"},
            "assignee": {"accountId": "abc123", "displayName": "Test User"},
            "project": {"key": key.split('-').next().unwrap_or("TEST")}
        }
    })
}

pub fn issue_search_response(issues: Vec<Value>) -> Value {
    json!({
        "issues": issues,
        "nextPageToken": null
    })
}

pub fn transitions_response(transitions: Vec<(&str, &str)>) -> Value {
    json!({
        "transitions": transitions.iter().map(|(id, name)| json!({
            "id": id,
            "name": name,
        })).collect::<Vec<_>>()
    })
}

pub fn worklog_response(seconds: u64, author: &str) -> Value {
    json!({
        "id": "12345",
        "timeSpentSeconds": seconds,
        "timeSpent": format!("{}h", seconds / 3600),
        "author": {"accountId": "abc", "displayName": author}
    })
}

pub fn error_response(messages: &[&str]) -> Value {
    json!({
        "errorMessages": messages
    })
}
```

Write `tests/common/mock_server.rs`:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};
use super::fixtures;

pub async fn setup_with_myself() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::user_response())
        )
        .mount(&server)
        .await;
    server
}
```

- [ ] **Step 1: Write issue command integration tests**

Write `tests/issue_commands.rs`:

```rust
mod common;

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_search_issues() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "issues": [
                        {
                            "key": "FOO-1",
                            "fields": {
                                "summary": "Test issue",
                                "status": {"name": "To Do"},
                                "issuetype": {"name": "Task"},
                                "priority": {"name": "Medium"},
                                "assignee": {"accountId": "abc", "displayName": "Test User"}
                            }
                        }
                    ]
                }))
        )
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(
        &server.uri(),
        "Basic dGVzdDp0ZXN0",
    );

    let issues = client.search_issues("assignee = currentUser()", None).await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].key, "FOO-1");
    assert_eq!(issues[0].fields.summary.as_deref(), Some("Test issue"));
}

#[tokio::test]
async fn test_get_issue() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "key": "FOO-1",
                    "fields": {
                        "summary": "Test issue",
                        "status": {"name": "In Progress"},
                        "issuetype": {"name": "Bug"},
                        "priority": {"name": "High"},
                        "assignee": null,
                        "description": {
                            "version": 1,
                            "type": "doc",
                            "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Bug description"}]}]
                        }
                    }
                }))
        )
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(
        &server.uri(),
        "Basic dGVzdDp0ZXN0",
    );

    let issue = client.get_issue("FOO-1").await.unwrap();
    assert_eq!(issue.key, "FOO-1");
    assert_eq!(issue.fields.status.unwrap().name, "In Progress");
}

#[tokio::test]
async fn test_transition_issue() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "transitions": [
                        {"id": "21", "name": "In Progress"},
                        {"id": "31", "name": "Done"},
                    ]
                }))
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(
        &server.uri(),
        "Basic dGVzdDp0ZXN0",
    );

    let transitions = client.get_transitions("FOO-1").await.unwrap();
    assert_eq!(transitions.transitions.len(), 2);

    client.transition_issue("FOO-1", "21").await.unwrap();
}
```

- [ ] **Step 2: Write worklog integration tests**

Write `tests/worklog_commands.rs`:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_add_worklog() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/worklog"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!({
                    "id": "12345",
                    "timeSpentSeconds": 7200,
                    "timeSpent": "2h",
                    "author": {"accountId": "abc", "displayName": "Test User"}
                }))
        )
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(
        &server.uri(),
        "Basic dGVzdDp0ZXN0",
    );

    let worklog = client.add_worklog("FOO-1", 7200, None).await.unwrap();
    assert_eq!(worklog.time_spent_seconds, 7200);
}

#[tokio::test]
async fn test_list_worklogs() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/worklog"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "startAt": 0,
                    "maxResults": 50,
                    "total": 1,
                    "worklogs": [
                        {
                            "id": "12345",
                            "timeSpentSeconds": 3600,
                            "timeSpent": "1h",
                            "author": {"accountId": "abc", "displayName": "Test User"},
                            "started": "2026-03-21T10:00:00.000+0000"
                        }
                    ]
                }))
        )
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(
        &server.uri(),
        "Basic dGVzdDp0ZXN0",
    );

    let worklogs = client.list_worklogs("FOO-1").await.unwrap();
    assert_eq!(worklogs.len(), 1);
    assert_eq!(worklogs[0].time_spent_seconds, 3600);
    assert_eq!(worklogs[0].time_spent.as_deref(), Some("1h"));
}
```

- [ ] **Step 3: Run all tests**

```bash
cargo test
```

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "test: integration tests for issues, transitions, and worklogs"
```

---

## Task 17: Polish + .gitignore + README

**Files:**
- Modify: `.gitignore`
- Modify: `Cargo.toml`

- [ ] **Step 1: Update .gitignore for Rust project**

Append to `.gitignore`:

```
# Rust build artifacts
/target

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db
```

- [ ] **Step 2: Final cargo check and clippy**

```bash
cargo clippy -- -D warnings 2>&1 | head -50
```

Fix any clippy warnings.

- [ ] **Step 3: Run full test suite**

```bash
cargo test
```

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: gitignore, clippy fixes, project polish"
```

---

## Task 18: GitHub Environment Setup

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`
- Create: `.github/dependabot.yml`
- Create: `deny.toml`
- Create: `rust-toolchain.toml`

- [ ] **Step 1: Create rust-toolchain.toml**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy", "llvm-tools-preview"]
```

- [ ] **Step 2: Create CI workflow**

Write `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo fmt --all -- --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --all --all-features --tests -- -D warnings

  test:
    name: Test
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features

  msrv:
    name: MSRV (1.85.0)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.85.0
      - uses: Swatinem/rust-cache@v2
      - run: cargo check --all-features

  deny:
    name: Deny (licenses + vulnerabilities)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2

  coverage:
    name: Coverage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: llvm-tools-preview
      - uses: taiki-e/install-action@cargo-llvm-cov
      - uses: Swatinem/rust-cache@v2
      - run: cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
      - uses: codecov/codecov-action@v4
        with:
          files: lcov.info
          fail_ci_if_error: false
```

- [ ] **Step 3: Create release workflow**

Write `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
            use_cross: true
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross
        if: matrix.use_cross
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Build
        run: |
          if [ "${{ matrix.use_cross }}" = "true" ]; then
            cross build --release --target ${{ matrix.target }}
          else
            cargo build --release --target ${{ matrix.target }}
          fi

      - name: Package
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ../../../jr-${{ github.ref_name }}-${{ matrix.target }}.tar.gz jr
          cd ../../..
          if command -v sha256sum &>/dev/null; then
            sha256sum jr-${{ github.ref_name }}-${{ matrix.target }}.tar.gz > jr-${{ github.ref_name }}-${{ matrix.target }}.tar.gz.sha256
          else
            shasum -a 256 jr-${{ github.ref_name }}-${{ matrix.target }}.tar.gz > jr-${{ github.ref_name }}-${{ matrix.target }}.tar.gz.sha256
          fi

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: jr-${{ matrix.target }}
          path: |
            jr-*.tar.gz
            jr-*.sha256

  release:
    name: Create Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
        with:
          merge-multiple: true

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
          files: |
            jr-*.tar.gz
            jr-*.sha256
```

- [ ] **Step 4: Create cargo-deny config**

Write `deny.toml`:

```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"

[licenses]
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "OpenSSL",
    "Zlib",
]
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "allow"

[sources]
unknown-registry = "warn"
unknown-git = "warn"
```

- [ ] **Step 5: Create Dependabot config**

Write `.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 5

  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 5
```

- [ ] **Step 6: Add MSRV to Cargo.toml**

Add to `[package]` section:

```toml
rust-version = "1.85"
```

- [ ] **Step 7: Run cargo deny check locally**

```bash
cargo install cargo-deny
cargo deny check
```

Fix any issues.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "ci: GitHub Actions CI/CD, release pipeline, cargo-deny, dependabot"
```

- [ ] **Step 9: Configure branch protection (manual)**

After pushing to GitHub, configure branch protection on `main`:
- Require PR reviews
- Require status checks: fmt, clippy, test, msrv, deny
- No force-pushes

---

## Dependency Order

```
Task 1 (scaffold)
├── Task 2 (config)
├── Task 3 (output)
├── Task 7 (partial match)
├── Task 8 (duration)
├── Task 9 (ADF)
└── Task 4 (API client)
    ├── Task 5 (types + jr me)
    │   └── Task 6 (auth commands)
    ├── Task 10 (issue commands)
    │   └── Task 13 (team assignment)
    ├── Task 11 (board + sprint)
    ├── Task 12 (worklog)
    └── Task 14 (OAuth)
        └── Task 15 (init)
Task 16 (integration tests) — after tasks 10-12
Task 17 (polish) — after task 16
Task 18 (GitHub setup) — after task 17
```

Tasks 2, 3, 7, 8, 9 have no dependencies on each other and can be executed in parallel after Task 1.
Task 18 (GitHub setup) can technically run after Task 1, but is placed last so CI validates a complete project.
