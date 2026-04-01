# Sprint Issue Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `sprint add` and `sprint remove` subcommands so users can add issues to a sprint and move issues to the backlog.

**Architecture:** Two new API methods (`add_issues_to_sprint`, `move_issues_to_backlog`) using the existing `post_no_content` client method. Two new `SprintCommand` variants with handlers following the existing state-change output pattern. `--current` resolves the active sprint by reusing the existing board resolution + sprint listing code.

**Tech Stack:** Rust, clap 4 (derive API), wiremock, assert_cmd, serde_json

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/api/jira/sprints.rs` | Modify | Add `add_issues_to_sprint` and `move_issues_to_backlog` methods |
| `src/cli/mod.rs:400-420` | Modify | Add `Add` and `Remove` variants to `SprintCommand` enum |
| `src/cli/sprint.rs:1-45` | Modify | Update `handle` dispatch, add `handle_add` and `handle_remove` functions |
| `tests/cli_smoke.rs` | Modify | Add smoke tests for `sprint add --help` and `sprint remove --help` |
| `tests/sprint_commands.rs` | Modify | Add integration tests for add and remove commands |

---

### Task 1: Add API methods for sprint issue management

**Files:**
- Modify: `src/api/jira/sprints.rs`

- [ ] **Step 1: Add `add_issues_to_sprint` method**

Append the following methods after the closing `}` of `get_sprint_issues` (after line 87) but before the closing `}` of the `impl JiraClient` block:

```rust
    /// Add issues to a sprint. Max 50 issues per call.
    /// POST /rest/agile/1.0/sprint/{sprintId}/issue → 204 No Content
    pub async fn add_issues_to_sprint(
        &self,
        sprint_id: u64,
        issues: &[String],
    ) -> Result<()> {
        let path = format!("/rest/agile/1.0/sprint/{}/issue", sprint_id);
        let body = serde_json::json!({ "issues": issues });
        self.post_no_content(&path, &body).await
    }

    /// Move issues to the backlog (removes from all sprints). Max 50 issues per call.
    /// POST /rest/agile/1.0/backlog/issue → 204 No Content
    pub async fn move_issues_to_backlog(&self, issues: &[String]) -> Result<()> {
        let path = "/rest/agile/1.0/backlog/issue";
        let body = serde_json::json!({ "issues": issues });
        self.post_no_content(path, &body).await
    }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds with no errors.

- [ ] **Step 3: Commit**

```bash
git add src/api/jira/sprints.rs
git commit -m "$(cat <<'EOF'
feat: add API methods for sprint issue management (#83)

Add add_issues_to_sprint and move_issues_to_backlog methods using
the existing post_no_content client method for 204 responses.
EOF
)"
```

---

### Task 2: Add `Add` and `Remove` variants to `SprintCommand`

**Files:**
- Modify: `src/cli/mod.rs:400-420`

- [ ] **Step 1: Add the new variants to `SprintCommand`**

In `src/cli/mod.rs`, change lines 400-420 from:

```rust
#[derive(Subcommand)]
pub enum SprintCommand {
    /// List sprints
    List {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
    },
    /// Show current sprint issues
    Current {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
        /// Maximum number of issues to return
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all results (no default limit)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
}
```

to:

```rust
#[derive(Subcommand)]
pub enum SprintCommand {
    /// List sprints
    List {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
    },
    /// Show current sprint issues
    Current {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
        /// Maximum number of issues to return
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all results (no default limit)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
    /// Add issues to a sprint
    Add {
        /// Sprint ID (from `jr sprint list`)
        #[arg(long, required_unless_present = "current")]
        sprint: Option<u64>,
        /// Use the active sprint instead of specifying an ID
        #[arg(long, conflicts_with = "sprint")]
        current: bool,
        /// Issue keys to add (e.g. FOO-1 FOO-2)
        #[arg(required = true, num_args = 1..)]
        issues: Vec<String>,
        /// Board ID (used with --current to resolve the active sprint)
        #[arg(long)]
        board: Option<u64>,
    },
    /// Remove issues from sprint (moves to backlog)
    Remove {
        /// Issue keys to remove (e.g. FOO-1 FOO-2)
        #[arg(required = true, num_args = 1..)]
        issues: Vec<String>,
    },
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Build fails with non-exhaustive match errors in `src/cli/sprint.rs` (the `handle` function doesn't cover `Add` and `Remove` yet). This is expected — Task 3 fixes it.

- [ ] **Step 3: Commit**

```bash
git add src/cli/mod.rs
git commit -m "$(cat <<'EOF'
feat: add Add and Remove variants to SprintCommand (#83)

Add clap definitions for sprint add (--sprint/--current + variadic
issues) and sprint remove (variadic issues). Handler not yet wired.
EOF
)"
```

---

### Task 3: Implement `handle_add` and `handle_remove` in sprint.rs

**Files:**
- Modify: `src/cli/sprint.rs:1-45`

- [ ] **Step 1: Add `serde_json` import**

In `src/cli/sprint.rs`, change line 1 from:

```rust
use anyhow::{Result, bail};
```

to:

```rust
use anyhow::{Result, bail};
use serde_json::json;
```

- [ ] **Step 2: Update the `handle` function dispatch**

Replace the `handle` function (lines 10-45) with:

```rust
/// Handle all sprint subcommands.
pub async fn handle(
    command: SprintCommand,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    match command {
        SprintCommand::List { board } => {
            let board_id = resolve_scrum_board(config, client, board, project_override).await?;
            handle_list(board_id, client, output_format).await
        }
        SprintCommand::Current {
            board, limit, all, ..
        } => {
            let board_id = resolve_scrum_board(config, client, board, project_override).await?;
            handle_current(board_id, client, output_format, config, limit, all).await
        }
        SprintCommand::Add {
            sprint,
            current,
            issues,
            board,
        } => {
            handle_add(sprint, current, board, issues, config, client, output_format, project_override)
                .await
        }
        SprintCommand::Remove { issues } => {
            handle_remove(issues, output_format, client).await
        }
    }
}

/// Resolve board ID and verify it's a scrum board.
async fn resolve_scrum_board(
    config: &Config,
    client: &JiraClient,
    board: Option<u64>,
    project_override: Option<&str>,
) -> Result<u64> {
    let board_id =
        crate::cli::board::resolve_board_id(config, client, board, project_override, true)
            .await?;

    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();
    if board_type != "scrum" {
        bail!(
            "Sprint commands are only available for scrum boards. Board {} is a {} board.",
            board_id,
            board_config.board_type
        );
    }

    Ok(board_id)
}
```

- [ ] **Step 3: Add the `handle_add` function**

Append after the `handle` function (before `handle_list`):

```rust
const MAX_SPRINT_ISSUES: usize = 50;

async fn handle_add(
    sprint: Option<u64>,
    current: bool,
    board: Option<u64>,
    issues: Vec<String>,
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    project_override: Option<&str>,
) -> Result<()> {
    if issues.len() > MAX_SPRINT_ISSUES {
        bail!(
            "Too many issues (got {}). Maximum is {} per operation.",
            issues.len(),
            MAX_SPRINT_ISSUES
        );
    }

    let sprint_id = if current {
        let board_id = resolve_scrum_board(config, client, board, project_override).await?;
        let sprints = client.list_sprints(board_id, Some("active")).await?;
        if sprints.is_empty() {
            bail!("No active sprint found for board {}.", board_id);
        }
        sprints[0].id
    } else {
        sprint.expect("clap enforces --sprint when --current is absent")
    };

    client.add_issues_to_sprint(sprint_id, &issues).await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "sprint_id": sprint_id,
                    "issues": issues,
                    "added": true
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Added {} issue(s) to sprint {}",
                issues.len(),
                sprint_id
            ));
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Add the `handle_remove` function**

Append after `handle_add`:

```rust
async fn handle_remove(
    issues: Vec<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    if issues.len() > MAX_SPRINT_ISSUES {
        bail!(
            "Too many issues (got {}). Maximum is {} per operation.",
            issues.len(),
            MAX_SPRINT_ISSUES
        );
    }

    client.move_issues_to_backlog(&issues).await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "issues": issues,
                    "removed": true
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Moved {} issue(s) to backlog", issues.len()));
        }
    }

    Ok(())
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds with no errors.

- [ ] **Step 6: Run existing sprint tests to verify nothing breaks**

Run: `cargo test --lib -- sprint 2>&1 | tail -20`
Expected: All existing `sprint` unit tests pass (sprint_summary tests are unaffected).

Run: `cargo test --test sprint_commands 2>&1 | tail -20`
Expected: All existing integration tests pass (they test `list`/`current`, not `add`/`remove`).

- [ ] **Step 7: Commit**

```bash
git add src/cli/sprint.rs
git commit -m "$(cat <<'EOF'
feat: implement sprint add and remove handlers (#83)

Wire handle_add (--sprint/--current with board resolution) and
handle_remove (move to backlog) with state-change output pattern.
Validates max 50 issues per operation.
EOF
)"
```

---

### Task 4: Add CLI smoke tests

**Files:**
- Modify: `tests/cli_smoke.rs`

- [ ] **Step 1: Add smoke test for `sprint add --help`**

Append the following test to `tests/cli_smoke.rs`:

```rust
#[test]
fn test_sprint_add_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["sprint", "add", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Add issues to a sprint"))
        .stdout(predicate::str::contains("--sprint"))
        .stdout(predicate::str::contains("--current"))
        .stdout(predicate::str::contains("--board"));
}
```

- [ ] **Step 2: Add smoke test for `sprint remove --help`**

Append the following test to `tests/cli_smoke.rs`:

```rust
#[test]
fn test_sprint_remove_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["sprint", "remove", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Remove issues from sprint"))
        .stdout(predicate::str::contains("ISSUES"));
}
```

- [ ] **Step 3: Add conflict test for `--sprint` and `--current`**

Append the following test to `tests/cli_smoke.rs`:

```rust
#[test]
fn test_sprint_add_sprint_and_current_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["sprint", "add", "--sprint", "100", "--current", "FOO-1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}
```

- [ ] **Step 4: Add test that `add` requires `--sprint` or `--current`**

Append the following test to `tests/cli_smoke.rs`:

```rust
#[test]
fn test_sprint_add_requires_sprint_or_current() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["sprint", "add", "FOO-1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--sprint"));
}
```

- [ ] **Step 5: Run the smoke tests**

Run: `cargo test --test cli_smoke 2>&1 | tail -20`
Expected: All tests pass, including the 4 new ones.

- [ ] **Step 6: Commit**

```bash
git add tests/cli_smoke.rs
git commit -m "$(cat <<'EOF'
test: add CLI smoke tests for sprint add and remove (#83)

Verify help output, --sprint/--current conflict, and required
flag enforcement.
EOF
)"
```

---

### Task 5: Add integration tests for sprint add and remove

**Files:**
- Modify: `tests/sprint_commands.rs`

- [ ] **Step 1: Add integration test for `sprint add --sprint`**

Append the following test to `tests/sprint_commands.rs`:

```rust
#[tokio::test]
async fn sprint_add_with_sprint_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["sprint", "add", "--sprint", "100", "FOO-1", "FOO-2"])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Added 2 issue(s) to sprint 100"),
        "Expected success message, got: {stdout}"
    );
}
```

- [ ] **Step 2: Add integration test for `sprint add --sprint` with JSON output**

Append the following test to `tests/sprint_commands.rs`:

```rust
#[tokio::test]
async fn sprint_add_json_output() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/200/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--output", "json",
            "sprint", "add", "--sprint", "200", "BAR-1",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["sprint_id"], 200);
    assert_eq!(parsed["issues"], serde_json::json!(["BAR-1"]));
    assert_eq!(parsed["added"], true);
}
```

- [ ] **Step 3: Add integration test for `sprint remove`**

Append the following test to `tests/sprint_commands.rs`:

```rust
#[tokio::test]
async fn sprint_remove_moves_to_backlog() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["sprint", "remove", "FOO-1", "FOO-3"])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Moved 2 issue(s) to backlog"),
        "Expected success message, got: {stdout}"
    );
}
```

- [ ] **Step 4: Add integration test for `sprint remove` with JSON output**

Append the following test to `tests/sprint_commands.rs`:

```rust
#[tokio::test]
async fn sprint_remove_json_output() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/backlog/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--output", "json",
            "sprint", "remove", "QUX-5",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["issues"], serde_json::json!(["QUX-5"]));
    assert_eq!(parsed["removed"], true);
}
```

- [ ] **Step 5: Add integration test for `sprint add --current`**

Append the following test to `tests/sprint_commands.rs`:

```rust
#[tokio::test]
async fn sprint_add_with_current_flag() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    Mock::given(method("POST"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .args(["sprint", "add", "--current", "TEST-1", "TEST-2"])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected success, got: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Added 2 issue(s) to sprint 100"),
        "Expected success message, got: {stdout}"
    );
}
```

- [ ] **Step 6: Run the integration tests**

Run: `cargo test --test sprint_commands 2>&1 | tail -30`
Expected: All tests pass, including the 5 new ones and the 5 existing ones.

- [ ] **Step 7: Commit**

```bash
git add tests/sprint_commands.rs
git commit -m "$(cat <<'EOF'
test: add integration tests for sprint add and remove (#83)

Test add with --sprint flag (table + JSON), remove (table + JSON),
and add with --current flag using board resolution prereqs.
EOF
)"
```

---

### Task 6: Run full test suite and lint

**Files:** None (verification only)

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -- -D warnings 2>&1 | tail -20`
Expected: No warnings or errors.

- [ ] **Step 2: Run format check**

Run: `cargo fmt --all -- --check 2>&1 | tail -10`
Expected: No formatting issues.

- [ ] **Step 3: Run full test suite**

Run: `cargo test 2>&1 | tail -30`
Expected: All tests pass. Key tests to verify:
- `tests/cli_smoke.rs::test_sprint_add_help` — PASS
- `tests/cli_smoke.rs::test_sprint_remove_help` — PASS
- `tests/cli_smoke.rs::test_sprint_add_sprint_and_current_conflict` — PASS
- `tests/cli_smoke.rs::test_sprint_add_requires_sprint_or_current` — PASS
- `tests/sprint_commands.rs::sprint_add_with_sprint_id` — PASS
- `tests/sprint_commands.rs::sprint_add_json_output` — PASS
- `tests/sprint_commands.rs::sprint_remove_moves_to_backlog` — PASS
- `tests/sprint_commands.rs::sprint_remove_json_output` — PASS
- `tests/sprint_commands.rs::sprint_add_with_current_flag` — PASS
- All existing sprint tests — PASS (unchanged)
- All existing unit tests — PASS (unchanged)
