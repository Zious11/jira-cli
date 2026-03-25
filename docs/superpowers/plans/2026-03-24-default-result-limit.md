# Default Result Limit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a default 30-result limit to `jr issue list` with `--all` flag for unlimited, and a truncation message showing approximate total count.

**Architecture:** Modify `search_issues()` to return `SearchResult { issues, has_more }`, add `approximate_count()` API method, add `--all` flag to CLI with `conflicts_with = "limit"`, and print truncation hint to stderr when results are capped. Add `strip_order_by()` helper to the `jql` module for count queries.

**Tech Stack:** Rust, clap 4 (derive), reqwest, serde, wiremock (tests), assert_cmd + predicates (integration tests)

**Spec:** `docs/superpowers/specs/2026-03-24-default-result-limit-design.md`

---

## File Structure

| File | Responsibility | Change |
|------|---------------|--------|
| `src/api/jira/issues.rs` | Issue search + count API methods | Add `SearchResult` struct, change `search_issues()` return type, add `approximate_count()` + `ApproximateCountResponse` |
| `src/jql.rs` | JQL string utilities | Add `strip_order_by()` function |
| `src/cli/mod.rs` | CLI argument definitions | Add `all: bool` to `IssueCommand::List` |
| `src/cli/issue/list.rs` | Issue list handler | Add `DEFAULT_LIMIT`, effective limit resolution, truncation message logic |
| `src/cli/board.rs` | Board view handler | Update `search_issues()` caller to destructure `SearchResult` |
| `tests/issue_commands.rs` | Integration tests for issue API | Update `search_issues()` callers, add new tests |
| `tests/common/fixtures.rs` | Test fixtures | Add `issue_search_response_with_next_page()` and `approximate_count_response()` helpers |

**Not changed:** `src/cli/issue/assets.rs` — verified it calls `get_issue()`, not `search_issues()`, so no changes needed.

---

### Task 1: Add `SearchResult` struct, update `search_issues()`, and fix all callers

This task must be atomic — changing the return type and updating all callers in one commit to maintain a compilable state.

**Files:**
- Modify: `src/api/jira/issues.rs`
- Modify: `src/cli/issue/list.rs`
- Modify: `src/cli/board.rs`
- Modify: `tests/issue_commands.rs`

- [ ] **Step 1: Write unit tests for `SearchResult`**

Add at the bottom of `src/api/jira/issues.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_result_has_more_false_when_no_truncation() {
        let result = SearchResult {
            issues: vec![],
            has_more: false,
        };
        assert!(!result.has_more);
    }

    #[test]
    fn search_result_has_more_true_when_truncated() {
        let result = SearchResult {
            issues: vec![],
            has_more: true,
        };
        assert!(result.has_more);
    }
}
```

- [ ] **Step 2: Add `SearchResult` struct and update `search_issues()`**

Add the struct above the `impl JiraClient` block in `src/api/jira/issues.rs`:

```rust
/// Result of a paginated issue search, including whether more results exist.
pub struct SearchResult {
    pub issues: Vec<Issue>,
    pub has_more: bool,
}
```

Change the `search_issues()` signature from `Result<Vec<Issue>>` to `Result<SearchResult>`.

Replace the entire function body with:

```rust
    pub async fn search_issues(
        &self,
        jql: &str,
        limit: Option<u32>,
        extra_fields: &[&str],
    ) -> Result<SearchResult> {
        let max_per_page = limit.unwrap_or(50).min(100);
        let mut all_issues: Vec<Issue> = Vec::new();
        let mut next_page_token: Option<String> = None;

        let mut fields = vec![
            "summary",
            "status",
            "issuetype",
            "priority",
            "assignee",
            "project",
            "description",
        ];
        fields.extend_from_slice(extra_fields);

        let mut more_available = false;

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": max_per_page,
                "fields": fields
            });

            if let Some(ref token) = next_page_token {
                body["nextPageToken"] = serde_json::json!(token);
            }

            let page: CursorPage<Issue> = self.post("/rest/api/3/search/jql", &body).await?;

            let page_has_more = page.has_more();
            let token = page.next_page_token.clone();
            all_issues.extend(page.issues);

            if let Some(max) = limit {
                if all_issues.len() >= max as usize {
                    more_available = all_issues.len() > max as usize || page_has_more;
                    all_issues.truncate(max as usize);
                    break;
                }
            }

            if !page_has_more {
                break;
            }

            next_page_token = token;
        }

        Ok(SearchResult {
            issues: all_issues,
            has_more: more_available,
        })
    }
```

- [ ] **Step 3: Update all callers**

In `src/cli/issue/list.rs` at line 147, change:

```rust
    let issues = client.search_issues(&effective_jql, limit, &extra).await?;
```

to:

```rust
    let search_result = client.search_issues(&effective_jql, limit, &extra).await?;
    let issues = search_result.issues;
```

In `src/cli/board.rs` at line 69, change:

```rust
        client.search_issues(&jql, None, &[]).await?
```

to:

```rust
        client.search_issues(&jql, None, &[]).await?.issues
```

In `tests/issue_commands.rs`, update `test_search_issues` (lines 24-29):

```rust
    let result = client
        .search_issues("assignee = currentUser()", None, &[])
        .await
        .unwrap();
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].key, "FOO-1");
    assert!(!result.has_more);
```

Update `test_search_issues_with_story_points` (lines 99-109):

```rust
    let result = client
        .search_issues("project = FOO", None, &["customfield_10031"])
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 2);
    assert_eq!(
        result.issues[0].fields.story_points("customfield_10031"),
        Some(5.0)
    );
    assert_eq!(result.issues[1].fields.story_points("customfield_10031"), None);
    assert!(!result.has_more);
```

- [ ] **Step 4: Run all tests to verify compilation and correctness**

Run: `cargo test --all-features`
Expected: ALL PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 6: Commit**

```bash
git add src/api/jira/issues.rs src/cli/issue/list.rs src/cli/board.rs tests/issue_commands.rs
git commit -m "feat: add SearchResult struct, update search_issues return type and all callers"
```

---

### Task 2: Add `strip_order_by()` JQL helper

**Files:**
- Modify: `src/jql.rs`

- [ ] **Step 1: Write unit tests for `strip_order_by()`**

Add these tests inside the first `mod tests` block in `src/jql.rs` (after the existing `trailing_backslash` test, before the closing `}`). The `strip_order_by` function should be placed after the `escape_value` function (after line 8, before the first `#[cfg(test)]` at line 10).

```rust
    #[test]
    fn strip_order_by_removes_clause() {
        assert_eq!(
            strip_order_by("project = PROJ ORDER BY updated DESC"),
            "project = PROJ"
        );
    }

    #[test]
    fn strip_order_by_no_clause() {
        assert_eq!(strip_order_by("project = PROJ"), "project = PROJ");
    }

    #[test]
    fn strip_order_by_case_insensitive() {
        assert_eq!(
            strip_order_by("project = PROJ order by rank ASC"),
            "project = PROJ"
        );
    }

    #[test]
    fn strip_order_by_trims_whitespace() {
        assert_eq!(
            strip_order_by("project = PROJ   ORDER BY rank ASC"),
            "project = PROJ"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib strip_order_by`
Expected: FAIL — `strip_order_by` not found

- [ ] **Step 3: Implement `strip_order_by()`**

Add to `src/jql.rs` after the `escape_value` function (after line 8), before the first `#[cfg(test)]` block (line 10):

```rust
/// Strip `ORDER BY` clause from JQL for use with count-only endpoints.
///
/// The approximate-count endpoint only needs the WHERE clause. ORDER BY is
/// meaningless for a count and may cause issues with bounded-JQL validation.
pub fn strip_order_by(jql: &str) -> &str {
    let upper = jql.to_uppercase();
    if let Some(pos) = upper.find(" ORDER BY") {
        jql[..pos].trim_end()
    } else {
        jql
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib strip_order_by`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/jql.rs
git commit -m "feat: add strip_order_by JQL helper for count queries"
```

---

### Task 3: Add `approximate_count()` API method

**Files:**
- Modify: `src/api/jira/issues.rs`

- [ ] **Step 1: Write unit tests for `ApproximateCountResponse` deserialization**

Add to the existing `mod tests` block in `src/api/jira/issues.rs`:

```rust
    #[test]
    fn approximate_count_response_deserializes() {
        let json = r#"{"count": 1234}"#;
        let resp: ApproximateCountResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.count, 1234);
    }

    #[test]
    fn approximate_count_response_zero() {
        let json = r#"{"count": 0}"#;
        let resp: ApproximateCountResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.count, 0);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib approximate_count_response`
Expected: FAIL — `ApproximateCountResponse` not found

- [ ] **Step 3: Add `ApproximateCountResponse` and `approximate_count()` method**

Add `use serde::Deserialize;` to the file-level imports at the top of `src/api/jira/issues.rs` (alongside the existing `use` statements).

Add the response struct above the `impl JiraClient` block (file-private — no `pub`):

```rust
#[derive(Deserialize)]
struct ApproximateCountResponse {
    count: u64,
}
```

Add the method inside the `impl JiraClient` block, after `search_issues()`:

```rust
    /// Get an approximate count of issues matching a JQL query.
    ///
    /// Uses the dedicated count endpoint which is lightweight (no issue data fetched).
    /// The JQL should not include ORDER BY — use `jql::strip_order_by()` before calling.
    pub async fn approximate_count(&self, jql: &str) -> Result<u64> {
        let body = serde_json::json!({ "jql": jql });
        let resp: ApproximateCountResponse = self
            .post("/rest/api/3/search/approximate-count", &body)
            .await?;
        Ok(resp.count)
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib approximate_count_response`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/jira/issues.rs
git commit -m "feat: add approximate_count API method"
```

---

### Task 4: Add `--all` flag to CLI and default limit logic

**Files:**
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/issue/list.rs`

- [ ] **Step 1: Write unit tests for effective limit resolution**

Add to the `mod tests` block in `src/cli/issue/list.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib effective_limit`
Expected: FAIL — `resolve_effective_limit` not found

- [ ] **Step 3: Add `--all` flag to `IssueCommand::List`**

In `src/cli/mod.rs`, inside the `List` variant of `IssueCommand` (after the `limit` field at line 165), add:

```rust
        /// Fetch all results (no default limit)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
```

- [ ] **Step 4: Implement `resolve_effective_limit()` and update `handle_list()`**

Add to `src/cli/issue/list.rs` near the other helper functions (e.g., near `resolve_show_points`):

```rust
const DEFAULT_LIMIT: u32 = 30;

/// Resolve the effective limit from CLI flags.
fn resolve_effective_limit(limit: Option<u32>, all: bool) -> Option<u32> {
    if all {
        None
    } else {
        Some(limit.unwrap_or(DEFAULT_LIMIT))
    }
}
```

Update the destructuring at line 28 to include `all`:

```rust
    let IssueCommand::List {
        jql,
        status,
        team,
        limit,
        all,
        points: show_points,
        assets: show_assets,
    } = command
    else {
        unreachable!()
    };
```

Add after destructuring, before `let sp_field_id`:

```rust
    let effective_limit = resolve_effective_limit(limit, all);
```

Update the `search_issues` call to use `effective_limit` instead of `limit`:

```rust
    let search_result = client
        .search_issues(&effective_jql, effective_limit, &extra)
        .await?;
    let issues = search_result.issues;
```

- [ ] **Step 5: Add truncation message logic**

After the `output::print_output(...)` call (the last statement before `Ok(())`), add:

```rust
    if search_result.has_more && !all {
        let count_jql = crate::jql::strip_order_by(&effective_jql);
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
    }
```

Note on ownership: `let issues = search_result.issues;` moves `issues` out of `search_result`, but `search_result.has_more` is `bool` (`Copy`), so accessing it after the partial move is valid Rust.

- [ ] **Step 6: Run all tests**

Run: `cargo test --all-features`
Expected: ALL PASS

- [ ] **Step 7: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 8: Commit**

```bash
git add src/cli/mod.rs src/cli/issue/list.rs
git commit -m "feat: add --all flag and default 30-result limit to issue list (#43)"
```

---

### Task 5: Integration tests

**Files:**
- Modify: `tests/common/fixtures.rs`
- Modify: `tests/issue_commands.rs`

- [ ] **Step 1: Add test fixture helpers**

In `tests/common/fixtures.rs`, add:

```rust
/// Search response with `nextPageToken` set (indicating more results exist).
pub fn issue_search_response_with_next_page(issues: Vec<Value>) -> Value {
    json!({ "issues": issues, "nextPageToken": "next-page-token-abc" })
}

/// Response for the approximate-count endpoint.
pub fn approximate_count_response(count: u64) -> Value {
    json!({ "count": count })
}
```

- [ ] **Step 2: Write integration tests**

In `tests/issue_commands.rs`, add:

```rust
#[tokio::test]
async fn test_search_issues_has_more_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response_with_next_page(vec![
                common::fixtures::issue_response("FOO-1", "Test issue", "To Do"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client
        .search_issues("project = FOO", Some(1), &[])
        .await
        .unwrap();
    assert_eq!(result.issues.len(), 1);
    assert!(result.has_more);
}

#[tokio::test]
async fn test_search_issues_no_more_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response("FOO-1", "Test issue", "To Do"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client
        .search_issues("project = FOO", Some(10), &[])
        .await
        .unwrap();
    assert_eq!(result.issues.len(), 1);
    assert!(!result.has_more);
}

#[tokio::test]
async fn test_search_issues_no_limit_fetches_all() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response("FOO-1", "Issue 1", "To Do"),
                common::fixtures::issue_response("FOO-2", "Issue 2", "To Do"),
                common::fixtures::issue_response("FOO-3", "Issue 3", "To Do"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client
        .search_issues("project = FOO", None, &[])
        .await
        .unwrap();
    assert_eq!(result.issues.len(), 3);
    assert!(!result.has_more);
}

#[tokio::test]
async fn test_approximate_count() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/approximate-count"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::approximate_count_response(42)),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let count = client.approximate_count("project = FOO").await.unwrap();
    assert_eq!(count, 42);
}

#[tokio::test]
async fn test_approximate_count_zero() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/approximate-count"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::approximate_count_response(0)),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let count = client.approximate_count("project = FOO").await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_approximate_count_server_error_returns_err() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/approximate-count"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.approximate_count("project = FOO").await;
    assert!(result.is_err());
}
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test issue_commands`
Expected: ALL PASS

- [ ] **Step 4: Run full test suite + clippy**

Run: `cargo test --all-features && cargo clippy --all --all-features --tests -- -D warnings`
Expected: ALL PASS, no warnings

- [ ] **Step 5: Commit**

```bash
git add tests/common/fixtures.rs tests/issue_commands.rs
git commit -m "test: add integration tests for SearchResult, approximate_count, and truncation"
```

---

### Task 6: Final verification and format check

**Files:** None (verification only)

- [ ] **Step 1: Run cargo fmt**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues (if there are, run `cargo fmt --all` and include in commit)

- [ ] **Step 2: Run full CI-equivalent check**

Run: `cargo fmt --all -- --check && cargo clippy --all --all-features --tests -- -D warnings && cargo test --all-features`
Expected: All three pass

- [ ] **Step 3: Fix any issues found, commit if needed**

If `cargo fmt` requires changes:
```bash
cargo fmt --all
git add -u
git commit -m "style: format code"
```
