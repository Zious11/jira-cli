# Board View --limit Flag Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--limit` and `--all` flags to `jr board view` so output is bounded by default (30 issues), with both scrum and kanban paths respecting the limit via early-stop pagination.

**Architecture:** Extract shared `resolve_effective_limit()` helper to `cli/mod.rs`. Add `limit` parameter to `get_sprint_issues()` with early-stop pagination returning `SprintIssuesResult`. Wire both paths in `board.rs` handler to pass effective limit and show truncation hints.

**Tech Stack:** Rust, clap 4 (derive), wiremock (integration tests), anyhow

---

### Task 1: Extract shared `resolve_effective_limit` to `cli/mod.rs`

**Files:**
- Modify: `src/cli/mod.rs:433` (add helper at end of file)
- Modify: `src/cli/issue/list.rs:310-318` (remove local copy, import shared)

- [ ] **Step 1: Write failing tests in `cli/mod.rs`**

Add a `#[cfg(test)]` module at the bottom of `src/cli/mod.rs`:

```rust
pub(crate) const DEFAULT_LIMIT: u32 = 30;

/// Resolve the effective limit from CLI flags.
///
/// Returns `None` when `--all` is set (no limit), otherwise returns the
/// explicit `--limit` value or the default.
pub(crate) fn resolve_effective_limit(limit: Option<u32>, all: bool) -> Option<u32> {
    if all {
        None
    } else {
        Some(limit.unwrap_or(DEFAULT_LIMIT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_limit_defaults_to_30() {
        assert_eq!(resolve_effective_limit(None, false), Some(30));
    }

    #[test]
    fn effective_limit_respects_explicit_limit() {
        assert_eq!(resolve_effective_limit(Some(50), false), Some(50));
    }

    #[test]
    fn effective_limit_all_returns_none() {
        assert_eq!(resolve_effective_limit(None, true), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib cli::tests`
Expected: All 3 tests PASS.

- [ ] **Step 3: Remove local copy from `issue/list.rs` and import shared version**

In `src/cli/issue/list.rs`, remove lines 310-318 (the `const DEFAULT_LIMIT` and `fn resolve_effective_limit` definitions). Then update line 63 where `resolve_effective_limit` is called — it will now resolve through the parent module. Add this import near the top of the file:

```rust
use crate::cli::resolve_effective_limit;
```

Also remove the 3 `resolve_effective_limit` tests from the `list.rs` test module (lines 692-705) since they now live in `cli/mod.rs`.

- [ ] **Step 4: Run all tests to verify nothing broke**

Run: `cargo test`
Expected: All tests pass. The moved tests pass from their new location, and `issue list` uses the shared function.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs src/cli/issue/list.rs
git commit -m "refactor: extract resolve_effective_limit to cli/mod.rs (#69)"
```

---

### Task 2: Add `SprintIssuesResult` and limit-aware `get_sprint_issues()`

**Files:**
- Modify: `src/api/jira/sprints.rs:35-73`

- [ ] **Step 1: Add `SprintIssuesResult` struct**

Add to the bottom of `src/api/jira/sprints.rs` (after the `impl JiraClient` block's closing brace):

```rust
/// Result of fetching sprint issues with optional limit.
pub struct SprintIssuesResult {
    pub issues: Vec<Issue>,
    pub has_more: bool,
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles (struct is defined but not yet used).

- [ ] **Step 3: Add `limit` parameter and early-stop to `get_sprint_issues()`**

Replace the current `get_sprint_issues` function (lines 35-72) with:

```rust
    /// Get issues in a specific sprint, with optional JQL filter and limit.
    pub async fn get_sprint_issues(
        &self,
        sprint_id: u64,
        jql: Option<&str>,
        limit: Option<u32>,
        extra_fields: &[&str],
    ) -> Result<SprintIssuesResult> {
        let mut all_issues: Vec<Issue> = Vec::new();
        let mut start_at: u32 = 0;
        let max_results: u32 = 50;
        let mut result_has_more = false;

        loop {
            let mut path = format!(
                "/rest/agile/1.0/sprint/{}/issue?startAt={}&maxResults={}",
                sprint_id, start_at, max_results
            );
            let mut fields_str = "summary,status,issuetype,priority,assignee,project".to_string();
            for f in extra_fields {
                fields_str.push(',');
                fields_str.push_str(f);
            }
            path.push_str(&format!("&fields={}", fields_str));
            if let Some(q) = jql {
                path.push_str(&format!("&jql={}", urlencoding::encode(q)));
            }
            let page: OffsetPage<Issue> = self.get(&path).await?;
            let page_has_more = page.has_more();
            let next = page.next_start();
            all_issues.extend(page.issues.unwrap_or_default());

            // Early-stop: if we have enough issues, truncate and break
            if let Some(max) = limit {
                if all_issues.len() >= max as usize {
                    result_has_more = all_issues.len() > max as usize || page_has_more;
                    all_issues.truncate(max as usize);
                    break;
                }
            }

            if !page_has_more {
                break;
            }
            start_at = next;
        }

        Ok(SprintIssuesResult {
            issues: all_issues,
            has_more: result_has_more,
        })
    }
```

- [ ] **Step 5: Fix compile errors in callers**

Two callers need updating:

**`src/cli/board.rs:70`** — change:
```rust
client.get_sprint_issues(sprint.id, None, &[]).await?
```
to:
```rust
client.get_sprint_issues(sprint.id, None, None, &[]).await?.issues
```

**`src/cli/sprint.rs:123`** — change:
```rust
let issues = client.get_sprint_issues(sprint.id, None, &extra).await?;
```
to:
```rust
let issues = client.get_sprint_issues(sprint.id, None, None, &extra).await?.issues;
```

Both pass `None` for limit (preserving current unbounded behavior) and extract `.issues` from the result.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All tests pass. No behavior change — both callers pass `None` for limit.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Clean.

- [ ] **Step 8: Commit**

```bash
git add src/api/jira/sprints.rs src/cli/board.rs src/cli/sprint.rs
git commit -m "feat: add limit parameter to get_sprint_issues with early-stop (#69)"
```

---

### Task 3: Add `--limit`/`--all` flags and wire into both board view paths

**Files:**
- Modify: `src/cli/mod.rs:360-370` (BoardCommand enum)
- Modify: `src/cli/board.rs:18, 46-93` (match arm + handle_view rewrite)

This task combines the flag addition and wiring to avoid an intermediate commit with an unused variable (which would fail `cargo clippy -- -D warnings`).

- [ ] **Step 1: Add flags to `BoardCommand::View` enum**

In `src/cli/mod.rs`, replace lines 364-369:

```rust
    /// View current board issues
    View {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
    },
```

with:

```rust
    /// View current board issues
    View {
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

- [ ] **Step 2: Update match arm in `board.rs` handle function**

In `src/cli/board.rs:18`, change:

```rust
        BoardCommand::View { board } => handle_view(config, client, output_format, board).await,
```

to:

```rust
        BoardCommand::View { board, limit, all } => {
            handle_view(config, client, output_format, board, limit, all).await
        }
```

- [ ] **Step 3: Rewrite `handle_view` with limit support and truncation hints**

Replace the entire `handle_view` function (lines 46-93) with:

```rust
async fn handle_view(
    config: &Config,
    client: &JiraClient,
    output_format: &OutputFormat,
    board_override: Option<u64>,
    limit: Option<u32>,
    all: bool,
) -> Result<()> {
    let effective_limit = crate::cli::resolve_effective_limit(limit, all);

    let board_id = config.board_id(board_override).ok_or_else(|| {
        JrError::ConfigError(
            "No board configured. Use --board <ID> or set board_id in .jr.toml.\n\
             Run \"jr board list\" to see available boards."
                .into(),
        )
    })?;

    let board_config = client.get_board_config(board_id).await?;
    let board_type = board_config.board_type.to_lowercase();

    let (issues, has_more) = if board_type == "scrum" {
        // For scrum boards, fetch the active sprint's issues
        let sprints = client.list_sprints(board_id, Some("active")).await?;
        if sprints.is_empty() {
            bail!("No active sprint found for board {}.", board_id);
        }
        let sprint = &sprints[0];
        let result = client
            .get_sprint_issues(sprint.id, None, effective_limit, &[])
            .await?;
        (result.issues, result.has_more)
    } else {
        // Kanban: search for issues not in Done status category
        let project_key = config.project_key(None);
        if project_key.is_none() {
            eprintln!(
                "warning: no project configured for board. Showing issues across all projects. Set project in .jr.toml to scope results."
            );
        }
        let jql = build_kanban_jql(project_key.as_deref());
        let result = client.search_issues(&jql, effective_limit, &[]).await?;
        (result.issues, result.has_more)
    };

    let rows = super::issue::format_issue_rows_public(&issues);

    output::print_output(
        output_format,
        &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
        &rows,
        &issues,
    )?;

    if has_more && !all {
        if board_type != "scrum" {
            // Kanban: try to get approximate total via JQL count
            let project_key = config.project_key(None);
            let jql = build_kanban_jql(project_key.as_deref());
            let count_jql = crate::jql::strip_order_by(&jql);
            match client.approximate_count(count_jql).await {
                Ok(total) if total > 0 => {
                    eprintln!(
                        "Showing {} of ~{} results. Use --limit or --all to see more.",
                        issues.len(),
                        total
                    );
                }
                Ok(_) | Err(_) => {
                    eprintln!(
                        "Showing {} results. Use --limit or --all to see more.",
                        issues.len()
                    );
                }
            }
        } else {
            // Scrum: no reliable total count from Agile API
            eprintln!(
                "Showing {} results. Use --limit or --all to see more.",
                issues.len()
            );
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs src/cli/board.rs
git commit -m "feat: add --limit/--all to board view with truncation hints (#69)"
```

---

### Task 4: Add integration tests for board view limit

**Files:**
- Modify: `tests/common/fixtures.rs` (add board/sprint fixtures)
- Create: `tests/board_commands.rs`

All existing integration tests use `JiraClient::new_for_test()` to test API methods directly (there is no `JR_AUTH_HEADER` env var — auth is loaded from keychain in `from_config()`). The flag conflict test can use `cargo_bin` since clap rejects args before auth is checked.

- [ ] **Step 1: Add board and sprint fixture helpers to `tests/common/fixtures.rs`**

Add these functions at the bottom of `tests/common/fixtures.rs`:

```rust
/// Board configuration response.
pub fn board_config_response(board_type: &str) -> Value {
    json!({
        "id": 382,
        "name": "Test Board",
        "type": board_type
    })
}

/// Sprint list response (offset-paginated).
pub fn sprint_list_response(sprints: Vec<Value>) -> Value {
    let total = sprints.len() as u32;
    json!({
        "startAt": 0,
        "maxResults": 50,
        "total": total,
        "values": sprints
    })
}

/// Single sprint object.
pub fn sprint(id: u64, name: &str, state: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "state": state,
        "startDate": "2026-03-20T00:00:00.000Z",
        "endDate": "2026-04-03T00:00:00.000Z"
    })
}

/// Sprint issues response (offset-paginated).
pub fn sprint_issues_response(issues: Vec<Value>, total: u32) -> Value {
    json!({
        "startAt": 0,
        "maxResults": 50,
        "total": total,
        "issues": issues
    })
}
```

- [ ] **Step 2: Create `tests/board_commands.rs` with API-level tests**

Create `tests/board_commands.rs`:

```rust
#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
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

#[tokio::test]
async fn get_sprint_issues_with_limit() {
    let server = MockServer::start().await;

    // Mock sprint issues — return 5 issues with total=5
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(make_issues(5), 5)),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = client
        .get_sprint_issues(100, None, Some(3), &[])
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 3);
    assert!(result.has_more);
    assert_eq!(result.issues[0].key, "TEST-1");
    assert_eq!(result.issues[2].key, "TEST-3");
}

#[tokio::test]
async fn get_sprint_issues_no_limit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_issues_response(make_issues(5), 5)),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = client
        .get_sprint_issues(100, None, None, &[])
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 5);
    assert!(!result.has_more);
}

#[tokio::test]
async fn search_issues_with_limit() {
    let server = MockServer::start().await;

    // Return 5 issues with a next page token (indicating more exist)
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response_with_next_page(make_issues(5)),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = client
        .search_issues("statusCategory != Done ORDER BY rank ASC", Some(3), &[])
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 3);
    assert!(result.has_more);
}

#[test]
fn board_view_limit_and_all_conflict() {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.arg("board")
        .arg("view")
        .arg("--limit")
        .arg("3")
        .arg("--all");

    cmd.assert().failure().code(2);
}
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test board_commands`
Expected: All 4 tests pass.

- [ ] **Step 4: Run all tests (unit + integration)**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add tests/board_commands.rs tests/common/fixtures.rs
git commit -m "test: add integration tests for board view --limit (#69)"
```

---

### Task 5: Final verification and format

**Files:**
- None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass (unit, integration, proptest, snapshots).

- [ ] **Step 2: Run clippy with strict warnings**

Run: `cargo clippy -- -D warnings`
Expected: Zero warnings.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all -- --check`
Expected: Clean.

- [ ] **Step 4: Verify the feature works manually (optional)**

Test the new flags parse correctly:

```bash
cargo run -- board view --help
```

Expected output includes `--limit <LIMIT>` and `--all` in the help text, with `--all` noted as conflicting with `--limit`.
