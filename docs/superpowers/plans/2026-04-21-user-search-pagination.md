# User Search Pagination Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `jr user search --all` and `jr user list --all` paginate through all server pages (up to Jira's documented 1000-user hard cap) instead of returning only the first API call's ~50 results.

**Architecture:** Add per-endpoint `_all` wrapper methods to `JiraClient` (mirroring octocrab's `all_pages()` idiom) that loop a private `_page(start, max)` helper until the API returns an empty array. CLI `--all` path switches to the `_all` variant; everything else stays on the existing single-call path. Spec: `docs/specs/user-search-pagination.md`.

**Tech Stack:** Rust, reqwest (via existing `JiraClient::get()`), wiremock for integration tests, assert_cmd for end-to-end CLI tests, clap for help text.

---

## File Structure

| File | Change type | Responsibility |
|------|-------------|----------------|
| `src/api/jira/users.rs` | Modify | Add `search_users_all`, `search_users_page`, `search_assignable_users_by_project_all`, `search_assignable_users_by_project_page` + module-private constants `USER_PAGE_SIZE`, `USER_PAGINATION_SAFETY_CAP`. Keep existing single-call methods unchanged. |
| `src/cli/user.rs` | Modify | Branch `handle_search` and `handle_list` on the `all` flag: call `_all` variant when true, existing single-call when false. |
| `src/cli/mod.rs` | Modify | Update `--all` doc comments on `UserCommand::Search` and `UserCommand::List` to describe real pagination. |
| `tests/user_pagination.rs` | Create | 7 integration tests: 4 library-level `_all` method tests against wiremock, 3 end-to-end CLI tests. Includes a `jr_cmd_json` helper mirroring `tests/all_flag_behavior.rs`. |

No new source files. `JiraClient` gains four methods in an existing file that already has the same structure (three sibling `search_*` methods). The existing deserialization pattern (flat-array-or-object-with-values) is preserved inside the new `_page` helpers.

---

### Task 1: `/user/search` pagination (happy path)

**Files:**
- Create: `tests/user_pagination.rs`
- Modify: `src/api/jira/users.rs:1-95` (add constants + 2 new methods)

- [ ] **Step 1: Write the failing integration test**

Create `tests/user_pagination.rs`:

```rust
//! End-to-end coverage for `--all` true pagination on user search and
//! user list (#189). Each library-level test asserts that `_all` variants
//! loop the endpoint until an empty page is returned and that pages are
//! concatenated in order. CLI-level tests verify the flag wiring.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use jr::api::client::JiraClient;

/// Build a `jr` command pre-configured for non-interactive JSON output
/// against a mock server. Matches the pattern used in tests/all_flag_behavior.rs.
fn jr_cmd_json(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "--output", "json"]);
    cmd
}

fn users_page(count: usize, prefix: &str) -> Value {
    let users: Vec<(&str, &str, bool)> = (0..count)
        .map(|i| {
            let acc = Box::leak(format!("{prefix}-acc-{i:03}").into_boxed_str()) as &str;
            let name = Box::leak(format!("{prefix} User {i:03}").into_boxed_str()) as &str;
            (acc, name, true)
        })
        .collect();
    common::fixtures::user_search_response(users)
}

/// `search_users_all` paginates three sequential pages (100 + 100 + 27)
/// and returns 227 users concatenated in order.
#[tokio::test]
async fn search_users_all_paginates_and_concatenates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "100"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "200"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(27, "p3")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "227"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(&server.uri(), "Basic dGVzdDp0ZXN0");
    let users = client.search_users_all("u").await.expect("pagination must succeed");
    assert_eq!(users.len(), 227, "expected 227 users across 3 pages");
    assert_eq!(users[0].display_name, "p1 User 000");
    assert_eq!(users[100].display_name, "p2 User 000");
    assert_eq!(users[200].display_name, "p3 User 000");
    assert_eq!(users[226].display_name, "p3 User 026");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --test user_pagination search_users_all_paginates_and_concatenates 2>&1 | tail -20
```

Expected: compile error — `no method named 'search_users_all' found for struct 'JiraClient'`.

- [ ] **Step 3: Add constants and the private `_page` helper to `src/api/jira/users.rs`**

Insert immediately after `use anyhow::Result;` at the top of the file:

```rust
/// Maximum users requested per page. Atlassian's effective server-side cap
/// for `/user/search` and related endpoints is 100 — requesting more is ignored.
const USER_PAGE_SIZE: u32 = 100;

/// Safety bound on the pagination loop. Atlassian documents a 1000-user hard
/// cap on these endpoints, so at USER_PAGE_SIZE=100 we need at most 10
/// iterations; 15 leaves 50% headroom against pathological server behavior.
const USER_PAGINATION_SAFETY_CAP: u32 = 15;
```

Inside `impl JiraClient`, add a private helper just after `search_users`:

```rust
/// Single-page variant of `search_users` with explicit `startAt` / `maxResults`.
/// Private — used only by `search_users_all` to implement the loop.
async fn search_users_page(
    &self,
    query: &str,
    start_at: u32,
    max_results: u32,
) -> Result<Vec<User>> {
    let path = format!(
        "/rest/api/3/user/search?query={}&startAt={}&maxResults={}",
        urlencoding::encode(query),
        start_at,
        max_results,
    );
    let raw: serde_json::Value = self.get(&path).await?;
    let users: Vec<User> = if raw.is_array() {
        serde_json::from_value(raw)?
    } else if let Some(values) = raw.get("values") {
        serde_json::from_value(values.clone())?
    } else {
        anyhow::bail!(
            "Unexpected response from user search API. Expected a JSON array or object with \"values\" key."
        );
    };
    Ok(users)
}
```

- [ ] **Step 4: Add the public `search_users_all` loop**

Inside `impl JiraClient`, immediately after `search_users_page`:

```rust
/// Paginate `/rest/api/3/user/search` until exhausted.
///
/// The endpoint returns a flat JSON array with no `isLast` / `total`
/// metadata, and its docs note responses "usually return fewer users
/// than specified in `maxResults`" due to post-page filtering. The only
/// reliable termination signal is an empty response.
pub async fn search_users_all(&self, query: &str) -> Result<Vec<User>> {
    let mut all: Vec<User> = Vec::new();
    let mut start_at: u32 = 0;
    for _ in 0..USER_PAGINATION_SAFETY_CAP {
        let page = self
            .search_users_page(query, start_at, USER_PAGE_SIZE)
            .await?;
        if page.is_empty() {
            break;
        }
        all.extend(page);
        start_at = start_at.saturating_add(USER_PAGE_SIZE);
    }
    Ok(all)
}
```

- [ ] **Step 5: Run fmt + clippy + the new test**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --test user_pagination search_users_all_paginates_and_concatenates
```

Expected: all three pass.

- [ ] **Step 6: Full test suite check**

```bash
cargo test
```

Expected: all tests pass (the added method is additive; nothing else should regress).

- [ ] **Step 7: Commit**

```bash
git add src/api/jira/users.rs tests/user_pagination.rs
git commit -m "feat(api): paginate /user/search with search_users_all (#189)"
```

---

### Task 2: `/user/search` pagination edge cases

**Files:**
- Modify: `tests/user_pagination.rs` (add 3 tests after the Task 1 test)

- [ ] **Step 1: Add stop-on-empty test**

Append to `tests/user_pagination.rs`:

```rust
/// Loop stops as soon as a page comes back empty; subsequent startAt
/// windows are not requested.
#[tokio::test]
async fn search_users_all_stops_on_empty_page() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    // Any request past startAt=100 would fail against this strict mock set —
    // wiremock rejects unmatched requests by default, which asserts the
    // loop actually stopped.

    let client = JiraClient::new_for_test(&server.uri(), "Basic dGVzdDp0ZXN0");
    let users = client.search_users_all("u").await.expect("must succeed");
    assert_eq!(users.len(), 100);
}
```

- [ ] **Step 2: Add safety-cap test**

Append to `tests/user_pagination.rs`:

```rust
/// If the API never returns an empty page (pathological behavior), the loop
/// stops at USER_PAGINATION_SAFETY_CAP iterations = 15 requests.
#[tokio::test]
async fn search_users_all_respects_safety_cap() {
    let server = MockServer::start().await;

    // Unbounded responder for any startAt; .expect(15) pins the iteration cap.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "cap")))
        .expect(15)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(&server.uri(), "Basic dGVzdDp0ZXN0");
    let users = client.search_users_all("u").await.expect("must succeed");
    assert_eq!(users.len(), 1500, "15 iterations * 100 per page = 1500");
}
```

- [ ] **Step 3: Add error-propagation test**

Append to `tests/user_pagination.rs`:

```rust
/// If a page request fails mid-pagination, the error is propagated and the
/// loop does not silently return partial results.
#[tokio::test]
async fn search_users_all_propagates_error_mid_pagination() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(&server.uri(), "Basic dGVzdDp0ZXN0");
    let result = client.search_users_all("u").await;
    assert!(result.is_err(), "500 on page 2 must propagate");
}
```

- [ ] **Step 4: Run the three new tests**

```bash
cargo test --test user_pagination \
  search_users_all_stops_on_empty_page \
  search_users_all_respects_safety_cap \
  search_users_all_propagates_error_mid_pagination
```

Expected: all three pass with the Task 1 implementation (termination, safety cap, and `?` propagation all work correctly).

- [ ] **Step 5: fmt + clippy + full suite**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add tests/user_pagination.rs
git commit -m "test: search_users_all termination, safety cap, and error propagation (#189)"
```

---

### Task 3: `/user/assignable/multiProjectSearch` pagination

**Files:**
- Modify: `src/api/jira/users.rs` (add 2 new methods after the `search_assignable_users_by_project` definition)
- Modify: `tests/user_pagination.rs` (add 1 happy-path test)

- [ ] **Step 1: Write the failing integration test**

Append to `tests/user_pagination.rs`:

```rust
/// `search_assignable_users_by_project_all` paginates the assignable-users
/// endpoint and concatenates pages in order.
#[tokio::test]
async fn search_assignable_users_by_project_all_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("query", ""))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "100"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(40, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "140"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(&server.uri(), "Basic dGVzdDp0ZXN0");
    let users = client
        .search_assignable_users_by_project_all("", "FOO")
        .await
        .expect("pagination must succeed");
    assert_eq!(users.len(), 140);
    assert_eq!(users[0].display_name, "p1 User 000");
    assert_eq!(users[100].display_name, "p2 User 000");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --test user_pagination search_assignable_users_by_project_all_paginates 2>&1 | tail -10
```

Expected: compile error — `no method named 'search_assignable_users_by_project_all'`.

- [ ] **Step 3: Add the private `_page` helper in `src/api/jira/users.rs`**

Insert inside `impl JiraClient`, immediately after the existing `search_assignable_users_by_project` method:

```rust
/// Single-page variant of `search_assignable_users_by_project`.
/// Private — used only by `search_assignable_users_by_project_all`.
async fn search_assignable_users_by_project_page(
    &self,
    query: &str,
    project_key: &str,
    start_at: u32,
    max_results: u32,
) -> Result<Vec<User>> {
    let path = format!(
        "/rest/api/3/user/assignable/multiProjectSearch?query={}&projectKeys={}&startAt={}&maxResults={}",
        urlencoding::encode(query),
        urlencoding::encode(project_key),
        start_at,
        max_results,
    );
    let raw: serde_json::Value = self.get(&path).await?;
    let users: Vec<User> = if raw.is_array() {
        serde_json::from_value(raw)?
    } else if let Some(values) = raw.get("values") {
        serde_json::from_value(values.clone())?
    } else {
        anyhow::bail!(
            "Unexpected response from assignable user search API. Expected a JSON array or object with \"values\" key."
        );
    };
    Ok(users)
}
```

- [ ] **Step 4: Add the public `_all` method**

Immediately after `search_assignable_users_by_project_page`:

```rust
/// Paginate `/rest/api/3/user/assignable/multiProjectSearch` until exhausted.
///
/// Same termination rules as `search_users_all`: empty response is the only
/// reliable end-of-data signal.
pub async fn search_assignable_users_by_project_all(
    &self,
    query: &str,
    project_key: &str,
) -> Result<Vec<User>> {
    let mut all: Vec<User> = Vec::new();
    let mut start_at: u32 = 0;
    for _ in 0..USER_PAGINATION_SAFETY_CAP {
        let page = self
            .search_assignable_users_by_project_page(query, project_key, start_at, USER_PAGE_SIZE)
            .await?;
        if page.is_empty() {
            break;
        }
        all.extend(page);
        start_at = start_at.saturating_add(USER_PAGE_SIZE);
    }
    Ok(all)
}
```

- [ ] **Step 5: fmt + clippy + tests**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --test user_pagination
```

Expected: all user_pagination tests pass.

- [ ] **Step 6: Full suite**

```bash
cargo test
```

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add src/api/jira/users.rs tests/user_pagination.rs
git commit -m "feat(api): paginate /user/assignable/multiProjectSearch via search_assignable_users_by_project_all (#189)"
```

---

### Task 4: CLI wiring + help text + end-to-end tests

**Files:**
- Modify: `src/cli/user.rs:28-58` (branch `handle_search` and `handle_list` on `all`)
- Modify: `src/cli/mod.rs:584-617` (update `--all` doc comments on `UserCommand::Search` and `UserCommand::List`)
- Modify: `tests/user_pagination.rs` (add 3 end-to-end CLI tests)

- [ ] **Step 1: Write the failing CLI test — `user search --all` paginates**

Append to `tests/user_pagination.rs`:

```rust
/// End-to-end: `jr user search --all` paginates and emits all users as JSON.
#[tokio::test]
async fn user_search_all_cli_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(50, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .and(query_param("startAt", "150"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "search", "u", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user search --all JSON is an array");
    assert_eq!(arr.len(), 150, "--all should paginate to 150 users");
}
```

- [ ] **Step 2: Write the failing CLI test — `user list --all` paginates**

Append to `tests/user_pagination.rs`:

```rust
/// End-to-end: `jr user list --all --project FOO` paginates.
#[tokio::test]
async fn user_list_all_cli_paginates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(100, "p1")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(users_page(35, "p2")))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("startAt", "135"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_search_response(vec![])))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "list", "--project", "FOO", "--all"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user list --all JSON is an array");
    assert_eq!(arr.len(), 135);
}
```

- [ ] **Step 3: Write the failing CLI test — without `--all`, single request**

Append to `tests/user_pagination.rs`:

```rust
/// Without `--all`, `jr user search` must still make exactly one API request
/// (the existing single-call path) — no accidental pagination.
#[tokio::test]
async fn user_search_no_all_issues_single_request() {
    let server = MockServer::start().await;

    // No startAt/maxResults constraints — proves the legacy single-call path
    // (which doesn't send those params) is still in use.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .and(query_param("query", "u"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::user_search_response(
                (0..50)
                    .map(|i| {
                        let acc = Box::leak(format!("acc-{i:03}").into_boxed_str()) as &str;
                        let name = Box::leak(format!("User {i:03}").into_boxed_str()) as &str;
                        (acc, name, true)
                    })
                    .collect(),
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd_json(&server.uri())
        .args(["user", "search", "u"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("user search JSON is an array");
    assert_eq!(arr.len(), 30, "default cap should truncate to 30, got {}", arr.len());
}
```

- [ ] **Step 4: Run the three new CLI tests to verify they fail**

```bash
cargo test --test user_pagination \
  user_search_all_cli_paginates \
  user_list_all_cli_paginates \
  user_search_no_all_issues_single_request 2>&1 | tail -20
```

Expected: `user_search_all_cli_paginates` and `user_list_all_cli_paginates` fail (CLI path doesn't call the new `_all` variants yet, so it still does a single unbounded call — the strict mock with `startAt=0` query_param matcher will reject the unmatched request). `user_search_no_all_issues_single_request` should already pass.

- [ ] **Step 5: Wire `handle_search` to branch on `all`**

Edit `src/cli/user.rs`, replacing the body of `handle_search` (currently at lines 28–41):

```rust
async fn handle_search(
    query: &str,
    limit: Option<u32>,
    all: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let effective = resolve_effective_limit(limit, all);
    let mut users = if all {
        client.search_users_all(query).await?
    } else {
        client.search_users(query).await?
    };
    if let Some(cap) = effective {
        users.truncate(cap as usize);
    }
    print_user_list(&users, output_format)
}
```

- [ ] **Step 6: Wire `handle_list` to branch on `all`**

Edit `src/cli/user.rs`, replacing the body of `handle_list` (currently at lines 43–58):

```rust
async fn handle_list(
    project: &str,
    limit: Option<u32>,
    all: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let effective = resolve_effective_limit(limit, all);
    let mut users = if all {
        client
            .search_assignable_users_by_project_all("", project)
            .await?
    } else {
        client
            .search_assignable_users_by_project("", project)
            .await?
    };
    if let Some(cap) = effective {
        users.truncate(cap as usize);
    }
    print_user_list(&users, output_format)
}
```

- [ ] **Step 7: Update `--all` help text in `src/cli/mod.rs`**

Change the doc comment on `UserCommand::Search`'s `all` field (currently at lines 597–600):

```rust
/// Fetch all matching users by paginating through every API page
/// (up to Jira's documented 1000-user hard cap). Overrides the default
/// local cap.
#[arg(long, conflicts_with = "limit")]
all: bool,
```

Change the doc comment on `UserCommand::List`'s `all` field (currently at lines 613–616):

```rust
/// Fetch all assignable users by paginating through every API page
/// (up to Jira's documented 1000-user hard cap). Overrides the default
/// local cap.
#[arg(long, conflicts_with = "limit")]
all: bool,
```

- [ ] **Step 8: Re-run CLI tests — should now pass**

```bash
cargo test --test user_pagination \
  user_search_all_cli_paginates \
  user_list_all_cli_paginates \
  user_search_no_all_issues_single_request
```

Expected: all three pass.

- [ ] **Step 9: fmt + clippy + full suite**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: no warnings, all tests pass. In particular, existing tests in `tests/user_commands.rs` (or wherever the `user search` / `user list` single-call path is already covered) continue to pass because the public signatures of `search_users` and `search_assignable_users_by_project` are unchanged.

- [ ] **Step 10: Commit**

```bash
git add src/cli/user.rs src/cli/mod.rs tests/user_pagination.rs
git commit -m "feat(cli): wire --all to paginate user search and list (#189)"
```

---

## Self-Review

**Spec coverage (against `docs/specs/user-search-pagination.md`):**

| Spec requirement | Task |
|---|---|
| `search_users_all` public method | Task 1 |
| `search_users_page` private helper | Task 1 |
| `search_assignable_users_by_project_all` public method | Task 3 |
| `search_assignable_users_by_project_page` private helper | Task 3 |
| `USER_PAGE_SIZE = 100` constant | Task 1 |
| `USER_PAGINATION_SAFETY_CAP = 15` constant | Task 1 |
| Empty-response termination | Tasks 1 (impl) + 2 (test) |
| `startAt` advanced by requested `USER_PAGE_SIZE` (fixed-window pagination), not by returned count | Task 1 (impl `start_at.saturating_add(USER_PAGE_SIZE)`) |
| Abort-on-error semantics | Task 2 (test) |
| CLI branch on `--all` for `user search` | Task 4 |
| CLI branch on `--all` for `user list` | Task 4 |
| Help text update for both `--all` flags | Task 4 |
| Integration tests: paginate+concat | Tasks 1, 3 |
| Integration tests: stop-on-empty | Task 2 |
| Integration tests: safety cap | Task 2 |
| Integration tests: error propagation | Task 2 |
| Integration tests: CLI end-to-end (3) | Task 4 |
| Single-call path unchanged for helpers.rs callers | Preserved (public `search_users` / `search_assignable_users_by_project` signatures untouched in all tasks) |

All spec requirements mapped to a task.

**Placeholder scan:** clean — no TBD/TODO/"add appropriate". Every code step has a complete code block; every test step has complete test code.

**Type consistency:**
- Method names used consistently across tasks: `search_users_all`, `search_users_page`, `search_assignable_users_by_project_all`, `search_assignable_users_by_project_page`.
- Constants used consistently: `USER_PAGE_SIZE`, `USER_PAGINATION_SAFETY_CAP`.
- Test helper `users_page(count, prefix)` defined in Task 1, reused in Tasks 2/3/4.
- `jr_cmd_json(server_uri)` helper defined in Task 1, reused in Task 4.
- Fixture `common::fixtures::user_search_response` used with the expected `Vec<(&str, &str, bool)>` shape throughout.
