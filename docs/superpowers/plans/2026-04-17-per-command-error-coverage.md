# Per-Command Error-Path Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 32 integration tests across 11 files (10 existing + 2 new) that pin per-command error propagation for 5xx, 401, and network-drop. No production code changes.

**Architecture:** Each test spawns the `jr` binary via `assert_cmd::Command`, injects a wiremock URL via `JR_BASE_URL` + a dummy Basic auth header via `JR_AUTH_HEADER`, mounts one error mock (or drops the server for net-drop), and asserts `status.code()` + stderr contains/NOT-contains. Canonical Jira error body shape is `{"errorMessages": ["..."], "errors": {}}`. No shared test harness — tests follow the same skeleton inline for grep-ability, matching the existing `tests/issue_list_errors.rs` precedent.

**Tech Stack:** Rust 2024, wiremock 0.6, assert_cmd 2, tempfile 3, tokio (test runtime).

**Spec:** `docs/superpowers/specs/2026-04-17-per-command-error-coverage.md` (HEAD `e1f5e81` on develop).

---

## File Structure

11 files to modify or create:

| # | File | Action | Tests | Representative command |
| --- | --- | --- | --- | --- |
| 1 | `tests/board_commands.rs` | modify | +3 | `jr board list` |
| 2 | `tests/sprint_commands.rs` | modify | +3 | `jr sprint current` |
| 3 | `tests/user_commands.rs` | modify | +3 | `jr user search test` |
| 4 | `tests/worklog_commands.rs` | modify | +3 | `jr worklog list PROJ-1` |
| 5 | `tests/team_commands.rs` | modify | +3 | `jr team list` |
| 6 | `tests/queue.rs` | modify | +3 | `jr queue list` |
| 7 | `tests/project_commands.rs` | modify | +3 | `jr project list` |
| 8 | `tests/comments.rs` | modify | +3 | `jr issue comments PROJ-1` |
| 9 | `tests/issue_list_errors.rs` | modify | +2 | `jr issue list` (5xx already covered) |
| 10 | `tests/assets_errors.rs` | CREATE | +3 | `jr assets search "Key = X"` |
| 11 | `tests/issue_view_errors.rs` | CREATE | +3 | `jr issue view PROJ-1` |

**Commit cadence:** one commit per file (11 commits total). Each commit message follows Conventional Commits (`test: add error-path coverage for <cmd> (#187)`).

**Shared assertion pattern:** every test asserts these four things:
1. `output.status.code() == Some(EXIT_CODE)` where `EXIT_CODE` is 1 for 5xx + net-drop, 2 for 401.
2. `stderr.contains(POSITIVE_MATCH)` — for 5xx: `"API error (500)"`; for 401: `"Not authenticated"` AND `"jr auth login"`; for net-drop: `"Could not reach"` AND `"check your connection"`.
3. `!stderr.contains("panic")` — regression guard against Rust panic leaking through to stderr.
4. `!output.status.success()` — redundant with (1) but documents intent.

---

## Task 1: board_commands.rs — error tests for `jr board list`

**Files:**
- Modify: `tests/board_commands.rs` (append three test functions at end of file)

`jr board list` hits `GET /rest/agile/1.0/board` with no prereqs. The simplest single-endpoint shape in the audit.

- [ ] **Step 1: Add three failing tests to the end of `tests/board_commands.rs`**

```rust
// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn board_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["board", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn board_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["board", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn board_list_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server); // Port closes; reqwest gets connection refused.

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["board", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test --test board_commands -- --test-threads=4 board_list_server_error_surfaces_friendly_message board_list_unauthorized_dispatches_reauth_message board_list_network_drop_surfaces_reach_error
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/board_commands.rs
git commit -m "test(board): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 2: sprint_commands.rs — error tests for `jr sprint current`

**Files:**
- Modify: `tests/sprint_commands.rs` (append three test functions at end of file)

`jr sprint current` requires `--project PROJ` (or config) + goes through board auto-resolve + board config + sprint list + sprint issues. The existing `mount_prereqs` helper sets up the 200 prereqs. For error tests, the **first** call in the chain is `GET /rest/agile/1.0/board?projectKeyOrId=PROJ&type=scrum` (from `mount_prereqs`). Failing that first call is the minimal shape — no prereqs needed.

- [ ] **Step 1: Add three failing tests to the end of `tests/sprint_commands.rs`**

```rust
// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn sprint_current_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    // Fail on the first call in the sprint-current chain (board auto-resolve).
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--project", "PROJ", "sprint", "current"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn sprint_current_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--project", "PROJ", "sprint", "current"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn sprint_current_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--project", "PROJ", "sprint", "current"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test --test sprint_commands -- --test-threads=4 sprint_current_server_error_surfaces_friendly_message sprint_current_unauthorized_dispatches_reauth_message sprint_current_network_drop_surfaces_reach_error
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/sprint_commands.rs
git commit -m "test(sprint): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 3: user_commands.rs — error tests for `jr user search`

**Files:**
- Modify: `tests/user_commands.rs` (append three test functions at end of file; reuse existing `jr_cmd` helper at line 11)

`jr user search test` hits `GET /rest/api/3/user/search?query=test`. No prereqs. The file already has a `jr_cmd(base_url)` helper that wraps env setup — reuse it.

- [ ] **Step 1: Add three failing tests to the end of `tests/user_commands.rs`**

```rust
// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn user_search_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["user", "search", "test"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn user_search_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args(["user", "search", "test"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn user_search_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);

    let output = jr_cmd(&uri)
        .args(["user", "search", "test"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test --test user_commands -- user_search_server_error_surfaces_friendly_message user_search_unauthorized_dispatches_reauth_message user_search_network_drop_surfaces_reach_error
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/user_commands.rs
git commit -m "test(user): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 4: worklog_commands.rs — error tests for `jr worklog list`

**Files:**
- Modify: `tests/worklog_commands.rs` (append three test functions at end of file)

`jr worklog list PROJ-1` hits `GET /rest/api/3/issue/PROJ-1/worklog`. No prereqs.

- [ ] **Step 1: Add three failing tests to the end of `tests/worklog_commands.rs`**

```rust
// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn worklog_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["worklog", "list", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn worklog_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["worklog", "list", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn worklog_list_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["worklog", "list", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

**Note:** `tests/worklog_commands.rs` currently only imports `assert_cmd::Command` via test function inspection — verify the file already imports `Command`, `Mock`, `MockServer`, `ResponseTemplate`, `method`, `path`. If not, add these at the top:

```rust
use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test --test worklog_commands -- worklog_list_server_error_surfaces_friendly_message worklog_list_unauthorized_dispatches_reauth_message worklog_list_network_drop_surfaces_reach_error
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/worklog_commands.rs
git commit -m "test(worklog): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 5: team_commands.rs — error tests for `jr team list`

**Files:**
- Modify: `tests/team_commands.rs` (append three test functions; add imports for `assert_cmd::Command`, `tempfile`)

`jr team list` goes through a cache layer first. To force the cache miss, redirect `XDG_CACHE_HOME` to a temporary directory. After cache miss, the first network call is `POST /gateway/api/graphql` for org metadata. Fail that call.

- [ ] **Step 1: Confirm imports at top of `tests/team_commands.rs`**

Current imports (per file contents at plan time):
```rust
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};
```

Add after these:
```rust
use assert_cmd::Command;
```

(Keep `#[allow(dead_code)] mod common;` at top unchanged.)

- [ ] **Step 2: Add three failing tests to the end of the file**

```rust
// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn team_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    // Fail the first call in the team-list chain (GraphQL org metadata).
    Mock::given(method("POST"))
        .and(path("/gateway/api/graphql"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args(["team", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn team_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/gateway/api/graphql"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args(["team", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn team_list_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);
    let cache_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args(["team", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

**Subagent-verify:** if the binary does not honor `XDG_CACHE_HOME` on macOS (dirs-rs may fall back to `~/Library/Caches/jr`), check how `tests/assets.rs:31` `set_cache_dir` (existing pattern) handles it. If that pattern exists, mirror it here instead of the `XDG_CACHE_HOME` env. This is the only task where a platform-specific cache-dir override matters.

- [ ] **Step 3: Run the new tests**

```bash
cargo test --test team_commands -- team_list_server_error_surfaces_friendly_message team_list_unauthorized_dispatches_reauth_message team_list_network_drop_surfaces_reach_error
```

Expected: 3 passed. If tests instead hit the real cache (flaky pass/fail), adjust the cache-dir mechanism per the subagent-verify note.

- [ ] **Step 4: Commit**

```bash
git add tests/team_commands.rs
git commit -m "test(team): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 6: queue.rs — error tests for `jr queue list`

**Files:**
- Modify: `tests/queue.rs` (append three test functions; ensure `tempfile` + `assert_cmd::Command` imports)

`jr queue list` requires a project key (from `.jr.toml` or `--project`). `require_service_desk` then hits `GET /rest/servicedeskapi/servicedesk` (list service desks) and `GET /rest/api/3/project/{key}` (project meta). The first call to fail is `GET /rest/servicedeskapi/servicedesk`.

Tests write a minimal `.jr.toml` with `project = "PROJ"` into a tempdir and set `current_dir` so the config is picked up.

- [ ] **Step 1: Ensure imports at top of `tests/queue.rs`**

Existing tests in the file use `assert_cmd::Command` and wiremock — imports are likely present. If not, add:
```rust
use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
```

- [ ] **Step 2: Add three failing tests to the end of the file**

```rust
// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn queue_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;
    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    // Fail the first call in the queue-list chain (list service desks).
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["queue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn queue_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;
    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["queue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn queue_list_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);
    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(project_dir.path().join(".jr.toml"), "project = \"PROJ\"\n").unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["queue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

**Subagent-verify:** `require_service_desk` (`src/api/jsm/servicedesks.rs:101`) may call `/rest/api/3/project/{key}` before listing service desks — check order. If project meta is fetched first, mount the error on `/rest/api/3/project/PROJ` instead. The test assertions are unchanged either way; only the mocked endpoint shifts.

- [ ] **Step 3: Run the new tests**

```bash
cargo test --test queue -- queue_list_server_error_surfaces_friendly_message queue_list_unauthorized_dispatches_reauth_message queue_list_network_drop_surfaces_reach_error
```

Expected: 3 passed.

- [ ] **Step 4: Commit**

```bash
git add tests/queue.rs
git commit -m "test(queue): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 7: project_commands.rs — error tests for `jr project list`

**Files:**
- Modify: `tests/project_commands.rs` (append three test functions at end of file)

`jr project list` hits `GET /rest/api/3/project/search` (no prereqs, no project key required).

- [ ] **Step 1: Add three failing tests to the end of `tests/project_commands.rs`**

```rust
// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn project_list_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["project", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn project_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["project", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn project_list_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["project", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test --test project_commands -- project_list_server_error_surfaces_friendly_message project_list_unauthorized_dispatches_reauth_message project_list_network_drop_surfaces_reach_error
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/project_commands.rs
git commit -m "test(project): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 8: comments.rs — error tests for `jr issue comments`

**Files:**
- Modify: `tests/comments.rs` (append three test functions at end of file)

`jr issue comments PROJ-1` hits `GET /rest/api/3/issue/PROJ-1/comment`. No prereqs.

- [ ] **Step 1: Add three failing tests to the end of `tests/comments.rs`**

```rust
// ─── Error-path coverage (#187) ─────────────────────────────────────────────

#[tokio::test]
async fn issue_comments_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn issue_comments_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1/comment"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn issue_comments_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test --test comments -- issue_comments_server_error_surfaces_friendly_message issue_comments_unauthorized_dispatches_reauth_message issue_comments_network_drop_surfaces_reach_error
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/comments.rs
git commit -m "test(comments): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 9: issue_list_errors.rs — add 401 + net-drop tests for `jr issue list`

**Files:**
- Modify: `tests/issue_list_errors.rs` (append two tests; 5xx already covered by `issue_list_board_config_server_error_propagates` and `issue_list_sprint_error_propagates`)

The existing file uses `mock_project_exists` helper + board_id config. For a 401 or net-drop on the first call (project-exists check at `/rest/api/3/project/PROJ`), no helper is needed — that first call directly fails.

- [ ] **Step 1: Add two failing tests to the end of `tests/issue_list_errors.rs`**

```rust
// ─── 401 + net-drop error coverage (#187) ──────────────────────────────────

#[tokio::test]
async fn issue_list_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    // Fail the first call (project-exists check).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
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
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn issue_list_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\nboard_id = 42\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test --test issue_list_errors -- issue_list_unauthorized_dispatches_reauth_message issue_list_network_drop_surfaces_reach_error
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/issue_list_errors.rs
git commit -m "test(issue list): add 401 + net-drop error coverage (#187)"
```

---

## Task 10: tests/assets_errors.rs — NEW file with `jr assets search` error tests

**Files:**
- Create: `tests/assets_errors.rs`

`jr assets search "Key = X"` first discovers the Assets workspace ID via `GET /rest/servicedeskapi/assets/workspace`. To avoid workspace-cache hits, the existing `tests/assets.rs:31` `set_cache_dir(&tempdir)` pattern is used — mirror it here.

- [ ] **Step 1: Create `tests/assets_errors.rs` with three failing tests**

We do NOT need the `ENV_MUTEX` / `unsafe set_var` pattern from `tests/assets.rs:31` (that pattern exists because those tests run the library in-process and mutate the test process's environment). Our new tests spawn the `jr` binary as a subprocess — we can pass `XDG_CACHE_HOME` into the subprocess via `.env()` on the `Command` builder, which is simpler and thread-safe without a mutex.

```rust
#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn assets_search_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    // Fail on the workspace-discovery call (first in the assets chain).
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args(["assets", "search", "Key = X"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn assets_search_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args(["assets", "search", "Key = X"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn assets_search_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);
    let cache_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args(["assets", "search", "Key = X"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test --test assets_errors
```

Expected: 3 passed. No `--test-threads=1` needed because each test gets its own `TempDir` and the env var is only set on the child process.

- [ ] **Step 3: Commit**

```bash
git add tests/assets_errors.rs
git commit -m "test(assets): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 11: tests/issue_view_errors.rs — NEW file with `jr issue view` error tests

**Files:**
- Create: `tests/issue_view_errors.rs`

`jr issue view PROJ-1` hits `GET /rest/api/3/issue/PROJ-1?fields=...`. No prereqs, no cache.

- [ ] **Step 1: Create `tests/issue_view_errors.rs` with three failing tests**

```rust
#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn issue_view_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "view", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "5xx should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("API error (500)"), "Expected 'API error (500)' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn issue_view_unauthorized_dispatches_reauth_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "view", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(2), "401 should exit 2, got: {:?}", output.status.code());
    assert!(stderr.contains("Not authenticated"), "Expected 'Not authenticated' in stderr, got: {stderr}");
    assert!(stderr.contains("jr auth login"), "Expected 'jr auth login' suggestion in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn issue_view_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server);

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "view", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, got stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(output.status.code(), Some(1), "Net-drop should exit 1, got: {:?}", output.status.code());
    assert!(stderr.contains("Could not reach"), "Expected 'Could not reach' in stderr, got: {stderr}");
    assert!(stderr.contains("check your connection"), "Expected 'check your connection' in stderr, got: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test --test issue_view_errors
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/issue_view_errors.rs
git commit -m "test(issue view): add 5xx, 401, net-drop error coverage (#187)"
```

---

## Task 12: Full CI gate

After all 11 commits land on the feature branch, run the full CI-equivalent check set to catch fmt/clippy/cross-test regressions before review.

- [ ] **Step 1: Run the full check set**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected:
- `fmt`: no output (formatting OK).
- `clippy`: no warnings, exit 0.
- `test`: all tests pass. Total count should be `prev + 32` (prev was 674 after #185 merge; expect around 706 depending on any concurrent changes).

- [ ] **Step 2: If any check fails**

Fix the root cause (not the symptom). Common issues and their fixes:
- **fmt drift:** run `cargo fmt --all` then re-stage the changed files into the relevant per-file commit (`git commit --amend` on the most recent; for earlier files, do an interactive note and fix via a followup commit — do not amend an already-pushed commit without explicit approval).
- **clippy warning on test code:** refactor the test to fix the warning. Never add `#[allow(...)]` without discussing with the user first (per CLAUDE.md: "No lint suppression without refactoring").
- **unrelated test failures:** investigate — could be caused by a test-ordering issue (e.g., env-var leak across tests in the same process). If caused by #187 work, fix; if pre-existing, note for the PR description.

- [ ] **Step 3: After full CI gate is green, ready to push and PR.**

---

## Self-Review Notes

**Spec coverage:** every file in the spec's coverage matrix has a task. 5xx + 401 + net-drop for 10 files = 30 tests; 401 + net-drop for `issue_list_errors.rs` = 2 tests. Total 32. Matches spec.

**Placeholder scan:** tasks 5, 6, 10 include "**Subagent-verify:**" notes that flag specific assumptions for the implementer to confirm by reading existing code (`tests/assets.rs:31`, `src/api/jsm/servicedesks.rs:101`, platform cache behavior). These are intentional — they call out the *one* thing the implementer should check rather than blindly copying — and are NOT "TBD" placeholders. Every task has full concrete code.

**Type consistency:** `method` + `path` + `ResponseTemplate` + `Mock` + `MockServer` used identically across tasks. `assert_cmd::Command::cargo_bin("jr")` + `JR_BASE_URL`/`JR_AUTH_HEADER` env injection pattern identical. Canonical error body shape `{"errorMessages": [...], "errors": {}}` identical. Assertions use same positive-and-negative-contains pattern. No drift.
