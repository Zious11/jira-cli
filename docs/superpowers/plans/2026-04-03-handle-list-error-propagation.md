# handle_list Error Propagation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Propagate board/sprint API errors in `handle_list` instead of silently swallowing them, with actionable error messages.

**Architecture:** Replace two error-swallowing match arms in `handle_list` with proper error propagation. 404 from `get_board_config` becomes a `JrError::UserError` (exit code 64). Other errors propagate with `anyhow::Context`. No changes to the API layer.

**Tech Stack:** Rust, anyhow, thiserror (JrError), wiremock + assert_cmd (tests)

---

### Task 1: Fix `get_board_config` error handling

**Files:**
- Modify: `src/cli/issue/list.rs:255-261`

- [ ] **Step 1: Write the failing integration test for board config 404**

Create test in `tests/issue_list_errors.rs`:

```rust
#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn issue_list_board_config_404_reports_error() {
    let server = MockServer::start().await;

    // Board config returns 404 (board deleted or no access)
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["Board does not exist or you do not have permission to see it."]
        })))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on board config 404, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("Board 42 not found or not accessible"),
        "Should mention board ID and accessibility, got: {stderr}"
    );
    assert!(
        stderr.contains("board_id"),
        "Should suggest removing board_id from config, got: {stderr}"
    );
    assert!(
        stderr.contains("--jql"),
        "Should suggest --jql as alternative, got: {stderr}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test issue_list_errors issue_list_board_config_404_reports_error -- --nocapture`

Expected: FAIL — the current code silently swallows the 404 and falls back to project JQL, so the command either succeeds or fails with a misleading "No project or filters specified" message.

- [ ] **Step 3: Write the failing integration test for board config non-404 error**

Add to `tests/issue_list_errors.rs`:

```rust
#[tokio::test]
async fn issue_list_board_config_server_error_propagates() {
    let server = MockServer::start().await;

    // Board config returns 500
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"]
        })))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on board config 500, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("Failed to fetch config for board 42"),
        "Should include board ID and context, got: {stderr}"
    );
    assert!(
        stderr.contains("--jql"),
        "Should suggest --jql as alternative, got: {stderr}"
    );
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test --test issue_list_errors issue_list_board_config_server_error_propagates -- --nocapture`

Expected: FAIL — same silent swallowing behavior as step 2.

- [ ] **Step 5: Implement `get_board_config` error propagation**

In `src/cli/issue/list.rs`, replace lines 255-261:

```rust
// BEFORE (lines 255-261):
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
```

With:

```rust
Err(e) => {
    if let Some(JrError::ApiError { status: 404, .. }) =
        e.downcast_ref::<JrError>()
    {
        return Err(JrError::UserError(format!(
                "Board {} not found or not accessible. \
                 Verify the board exists and you have permission, \
                 or remove board_id from .jr.toml. \
                 Use --jql to query directly.",
                bid
        )).into());
    }
    return Err(e.context(format!(
        "Failed to fetch config for board {}. \
         Remove board_id from .jr.toml or use --jql to query directly",
        bid
    )));
}
```

Note: `JrError` is already imported at the top of `list.rs` (`use crate::error::JrError;`).

- [ ] **Step 6: Run both board config tests to verify they pass**

Run: `cargo test --test issue_list_errors -- --nocapture`

Expected: Both `issue_list_board_config_404_reports_error` and `issue_list_board_config_server_error_propagates` PASS.

- [ ] **Step 7: Run full test suite to check for regressions**

Run: `cargo test`

Expected: All tests pass. No regressions — the happy path and the "no board_id configured" path are unchanged.

- [ ] **Step 8: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: No warnings.

- [ ] **Step 9: Commit**

```bash
git add src/cli/issue/list.rs tests/issue_list_errors.rs
git commit -m "fix: propagate get_board_config errors in handle_list (#32)"
```

### Task 2: Fix `list_sprints` error handling

**Files:**
- Modify: `src/cli/issue/list.rs:234-243`

- [ ] **Step 1: Write the failing integration test for sprint list error**

Add to `tests/issue_list_errors.rs`:

```rust
#[tokio::test]
async fn issue_list_sprint_error_propagates() {
    let server = MockServer::start().await;

    // Board config succeeds → scrum board
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(&server)
        .await;

    // Sprint list returns 500
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"]
        })))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on sprint list error, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("Failed to list sprints for board 42"),
        "Should mention board ID and sprints, got: {stderr}"
    );
    assert!(
        stderr.contains("--jql"),
        "Should suggest --jql as alternative, got: {stderr}"
    );
}
```

- [ ] **Step 2: Write the test for no-active-sprint fallback (existing behavior)**

Add to `tests/issue_list_errors.rs`:

```rust
use wiremock::matchers::query_param;

#[tokio::test]
async fn issue_list_no_active_sprint_falls_back_to_project_jql() {
    let server = MockServer::start().await;

    // Board config succeeds → scrum board
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(&server)
        .await;

    // Sprint list returns empty (no active sprint)
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .and(query_param("state", "active"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_list_response(vec![])),
        )
        .mount(&server)
        .await;

    // Search endpoint returns issues (fallback JQL works)
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(vec![
                    common::fixtures::issue_response("PROJ-1", "Test Issue", "To Do"),
                ])),
        )
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Should succeed with fallback JQL, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("PROJ-1"),
        "Should show fallback results, got: {stdout}"
    );
}
```

- [ ] **Step 3: Run the sprint error test to verify it fails**

Run: `cargo test --test issue_list_errors issue_list_sprint_error_propagates -- --nocapture`

Expected: FAIL — current code swallows the error via `_ =>` catch-all.

- [ ] **Step 4: Run the no-active-sprint test to verify it already passes**

Run: `cargo test --test issue_list_errors issue_list_no_active_sprint_falls_back -- --nocapture`

Expected: PASS — this is existing behavior that must be preserved.

- [ ] **Step 5: Implement `list_sprints` error propagation**

In `src/cli/issue/list.rs`, replace lines 234-243:

```rust
// BEFORE (lines 234-243):
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
```

With two explicit match arms:

```rust
Ok(_) => {
    // No active sprint — fall back to project-scoped JQL
    let mut parts = Vec::new();
    if let Some(ref pk) = project_key {
        parts.push(format!(
            "project = \"{}\"",
            crate::jql::escape_value(pk)
        ));
    }
    (parts, "updated DESC")
}
Err(e) => {
    return Err(e.context(format!(
        "Failed to list sprints for board {}. \
         Use --jql to query directly",
        bid
    )));
}
```

- [ ] **Step 6: Run all issue_list_errors tests**

Run: `cargo test --test issue_list_errors -- --nocapture`

Expected: All 4 tests pass:
- `issue_list_board_config_404_reports_error` — PASS
- `issue_list_board_config_server_error_propagates` — PASS
- `issue_list_sprint_error_propagates` — PASS
- `issue_list_no_active_sprint_falls_back_to_project_jql` — PASS

- [ ] **Step 7: Run full test suite**

Run: `cargo test`

Expected: All tests pass, no regressions.

- [ ] **Step 8: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: No warnings.

- [ ] **Step 9: Commit**

```bash
git add src/cli/issue/list.rs tests/issue_list_errors.rs
git commit -m "fix: propagate list_sprints errors in handle_list (#32)"
```
