# Sprint Current --limit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--limit` and `--all` flags to `jr sprint current` so output size is bounded by default (30 issues), matching `issue list` and `board view`.

**Architecture:** Pure wiring change — `SprintCommand::Current` gains two clap fields, `handle_current` computes `effective_limit` via existing `resolve_effective_limit()`, and passes it to the existing `get_sprint_issues()` which already supports limits. A "more results" hint prints to stderr when results are truncated.

**Tech Stack:** Rust, clap (derive), wiremock (integration tests), assert_cmd

---

### File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src/cli/mod.rs:382-396` | Add `limit` and `all` fields to `SprintCommand::Current` |
| Modify | `src/cli/sprint.rs:17-44` | Update both match sites in `handle`, add params to `handle_current` |
| Modify | `src/cli/sprint.rs:106-178` | Wire `effective_limit` into `handle_current`, add "more results" hint |
| Create | `tests/sprint_commands.rs` | Integration tests for limit behavior |

No new API methods, no new utility functions, no new fixture helpers needed.

---

### Task 1: Add --limit/--all flags to SprintCommand::Current

**Files:**
- Modify: `src/cli/mod.rs:382-396`

- [ ] **Step 1: Add `limit` and `all` fields to `SprintCommand::Current`**

In `src/cli/mod.rs`, change `SprintCommand::Current` from:

```rust
    /// Show current sprint issues
    Current {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
    },
```

To:

```rust
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
```

This matches `BoardCommand::View` exactly (lines 370-379 in the same file).

- [ ] **Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -20`

Expected: Compile errors in `src/cli/sprint.rs` because the match arms don't account for the new fields yet. That's expected — we'll fix them in Task 2.

- [ ] **Step 3: Commit**

```bash
git add src/cli/mod.rs
git commit -m "feat: add --limit and --all flags to sprint current CLI definition (#72)"
```

---

### Task 2: Wire limit into handle_current

**Files:**
- Modify: `src/cli/sprint.rs:1-178`

- [ ] **Step 1: Update the board_override extraction match**

In `src/cli/sprint.rs`, change line 19 from:

```rust
        SprintCommand::Current { board } => *board,
```

To:

```rust
        SprintCommand::Current { board, .. } => *board,
```

The `..` ignores the new `limit` and `all` fields when we only need `board`.

- [ ] **Step 2: Update the dispatch match to extract and pass limit/all**

In `src/cli/sprint.rs`, change lines 41-43 from:

```rust
        SprintCommand::Current { .. } => {
            handle_current(board_id, client, output_format, config).await
        }
```

To:

```rust
        SprintCommand::Current { limit, all, .. } => {
            handle_current(board_id, client, output_format, config, limit, all).await
        }
```

- [ ] **Step 3: Update handle_current signature and wire effective_limit**

Change the `handle_current` function signature (line 106) from:

```rust
async fn handle_current(
    board_id: u64,
    client: &JiraClient,
    output_format: &OutputFormat,
    config: &Config,
) -> Result<()> {
```

To:

```rust
async fn handle_current(
    board_id: u64,
    client: &JiraClient,
    output_format: &OutputFormat,
    config: &Config,
    limit: Option<u32>,
    all: bool,
) -> Result<()> {
    let effective_limit = crate::cli::resolve_effective_limit(limit, all);
```

Then change the `get_sprint_issues` call (lines 121-124) from:

```rust
    let issues = client
        .get_sprint_issues(sprint.id, None, None, &extra)
        .await?
        .issues;
```

To:

```rust
    let result = client
        .get_sprint_issues(sprint.id, None, effective_limit, &extra)
        .await?;
    let issues = result.issues;
    let has_more = result.has_more;
```

- [ ] **Step 4: Capture issue count before the match block, add "more results" hint after it**

The `OutputFormat::Json` branch moves `issues` into `serde_json::json!` (`"issues": issues`), so `issues.len()` would be a use-after-move if placed after the match. Capture the count first.

Add this line immediately after `let has_more = result.has_more;`:

```rust
    let issue_count = issues.len();
```

Then after the closing brace of `match output_format { ... }` (line 175), add before the final `Ok(())`:

```rust
    if has_more && !all {
        eprintln!(
            "Showing {} results. Use --limit or --all to see more.",
            issue_count
        );
    }
```

- [ ] **Step 5: Verify it compiles and existing tests pass**

Run: `cargo build && cargo test --lib`

Expected: Build succeeds. All existing unit tests pass (the `compute_sprint_summary` tests don't touch `handle_current`).

- [ ] **Step 6: Commit**

```bash
git add src/cli/sprint.rs
git commit -m "feat: wire --limit/--all into sprint current handler (#72)"
```

---

### Task 3: Integration tests

**Files:**
- Create: `tests/sprint_commands.rs`

- [ ] **Step 1: Write integration tests**

Create `tests/sprint_commands.rs` with the following content:

```rust
#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: build N issues for testing.
fn make_issues(count: usize) -> Vec<serde_json::Value> {
    (1..=count)
        .map(|i| {
            common::fixtures::issue_response(
                &format!("TEST-{}", i),
                &format!("Issue {}", i),
                "In Progress",
            )
        })
        .collect()
}

/// Mount prereq mocks (board list, board config, active sprint) on the server.
async fn mount_prereqs(server: &MockServer) {
    // Board auto-resolve: list boards for project PROJ, type=scrum → 1 board
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![common::fixtures::board_response(
                42,
                "PROJ Scrum Board",
                "scrum",
                "PROJ",
            )]),
        ))
        .mount(server)
        .await;

    // Board config → scrum
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(server)
        .await;

    // Active sprint list → one sprint
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .and(query_param("state", "active"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::sprint_list_response(vec![common::fixtures::sprint(
                100,
                "Sprint 1",
                "active",
            )]),
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn sprint_current_default_limit_caps_at_30() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    // Sprint issues: 35 results in one page
    let issues = make_issues(35);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show exactly 30 issues (default limit)
    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 30, "Expected 30 issues, got {issue_count}");

    // Should show "more results" hint
    assert!(
        stderr.contains("Showing 30 results"),
        "Expected 'Showing 30 results' in stderr, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_current_limit_flag() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(20);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 20)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .arg("--limit")
        .arg("5")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 5, "Expected 5 issues, got {issue_count}");

    assert!(
        stderr.contains("Showing 5 results"),
        "Expected 'Showing 5 results' in stderr, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_current_all_flag_returns_everything() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(35);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 35)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .arg("--all")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 35, "Expected 35 issues, got {issue_count}");

    assert!(
        !stderr.contains("Showing"),
        "Should NOT show 'Showing' hint with --all, got: {stderr}"
    );
}

#[tokio::test]
async fn sprint_current_under_limit_no_hint() {
    let server = MockServer::start().await;
    mount_prereqs(&server).await;

    let issues = make_issues(10);
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(issues, 10)),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--project")
        .arg("PROJ")
        .arg("sprint")
        .arg("current")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let issue_count = stdout.lines().filter(|l| l.contains("TEST-")).count();
    assert_eq!(issue_count, 10, "Expected 10 issues, got {issue_count}");

    assert!(
        !stderr.contains("Showing"),
        "Should NOT show hint when under limit, got: {stderr}"
    );
}

#[test]
fn sprint_current_limit_and_all_conflict() {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.arg("sprint")
        .arg("current")
        .arg("--limit")
        .arg("3")
        .arg("--all");

    cmd.assert().failure().code(2);
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --test sprint_commands`

Expected: All 5 tests pass (the CLI changes from Tasks 1-2 are already in place).

If any test fails, fix the implementation in `sprint.rs` — not the tests. The tests encode the spec requirements.

**TDD note:** Tests come after implementation here because the integration tests invoke the binary with `--limit`/`--all` flags via `assert_cmd`. Clap rejects unknown flags with exit 2 before any handler code runs, so writing these tests before the flag definition gives false failures (clap parse error), not meaningful red-green-refactor. The test-first discipline applies at the unit level; at the CLI integration level, the flag must exist first.

- [ ] **Step 3: Run the full test suite to verify nothing is broken**

Run: `cargo test`

Expected: All tests pass (unit + integration).

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: Zero warnings.

- [ ] **Step 5: Run formatter**

Run: `cargo fmt --all -- --check`

Expected: No formatting changes needed.

- [ ] **Step 6: Commit**

```bash
git add tests/sprint_commands.rs
git commit -m "test: add integration tests for sprint current --limit (#72)"
```

---

### Task 4: Format and final verification

- [ ] **Step 1: Run full test suite one final time**

Run: `cargo test`

Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: Zero warnings.

- [ ] **Step 3: Run formatter and commit if needed**

Run: `cargo fmt --all`

If any files changed:

```bash
git add -A
git commit -m "style: format code"
```
