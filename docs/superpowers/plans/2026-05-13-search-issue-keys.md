# `search_issue_keys` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a lightweight `JiraClient::search_issue_keys` method that posts `/rest/api/3/search/jql` with body `fields: ["key"]`, and migrate the single keys-only caller at `src/cli/issue/create.rs:386` to use it — reducing the JQL-bulk-edit selection wire payload from ~1 MB to ~100 bytes per page.

**Architecture:** Mirror the existing `search_issues` method exactly, swapping `Issue` → new private `IssueKeyRow { key: String }` and `SearchResult` → new public `KeySearchResult { keys: Vec<String>, has_more: bool }`. Pagination, cursor handling, JRACLOUD-94632 anti-loop guard, and `.min(100)` clamp are byte-for-byte copies. Caller migration is a 5-line diff at one site.

**Tech Stack:** Rust 1.85+, reqwest 0.13.x, serde 1, tokio 1.52.x, wiremock 0.6.5 (test-only), assert_cmd (subprocess tests).

**Spec:** `docs/specs/2026-05-13-search-issue-keys.md`
**Story (factory-internal):** `.factory/code-delivery/issue-350/story.md`
**Research backing:** `.factory/research/issue-350-search-issue-keys-design.md`

---

## File Plan

**New files:**
- `tests/search_issue_keys.rs` — wiremock integration tests for the new method (12 tests).

**Modified files:**
- `src/api/jira/issues.rs` — add `KeySearchResult` struct, `IssueKeyRow` private deserialization helper, `search_issue_keys` method.
- `src/cli/issue/create.rs` — migrate `handle_edit::effective_keys` (lines 374-409) from `search_issues` to `search_issue_keys`.
- `tests/issue_bulk_pr2.rs` — add one caller-level regression test alongside existing `test_jql_*` cases.
- `CLAUDE.md` — one-line update to the `src/api/jira/issues.rs` file blurb.
- `.factory/specs/prd/bc-2-issue-read.md` — add BC-2.6.050 in subdomain 2.6; bump frontmatter `definitional_count` 49 → 50.
- `.factory/specs/prd/BC-INDEX.md` — bump frontmatter `total_bcs` 545 → 546 AND `sections:` line for `bc-2-issue-read.md` (`49 individually-bodied` → `50`).

Each task below is one TDD micro-cycle. The plan assumes work happens in the worktree at `.worktrees/issue-350-search-keys` (already created off `origin/develop` @ `1ffc332`). All commit commands target the feature branch `feat/issue-350-search-issue-keys`. Doc/spec updates inside `.factory/` commit to the separate `factory-artifacts` orphan branch via the `.factory/` worktree.

---

## Task 1: Scaffold the new test file

**Files:**
- Create: `tests/search_issue_keys.rs`

- [ ] **Step 1: Create the test file with imports and helpers**

```rust
// tests/search_issue_keys.rs
//
// Integration tests for `JiraClient::search_issue_keys` (issue #350).
//
// Pins BC-2.6.050 — keys-only JQL search posts body `fields: ["key"]`,
// deserializes only the top-level `key`, and signals caller-side
// truncation via `KeySearchResult { keys, has_more }`.
//
// Library-level tests use `jr::api::client::JiraClient::new_for_test`
// (no subprocess wiring). Pattern mirrors `tests/issue_read_holdouts.rs`.

use jr::api::client::JiraClient;
use jr::api::jira::issues::KeySearchResult;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a `JiraClient` pointing at the mock server.
fn test_client(server: &MockServer) -> JiraClient {
    JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string())
}

/// Build a `/rest/api/3/search/jql` response with the given keys and an
/// optional next-page cursor. Mirrors the minimal shape `search_issue_keys`
/// expects (top-level `key`, possibly-empty `fields {}`, cursor metadata).
fn jql_keys_response(keys: &[&str], next_page_token: Option<&str>, is_last: bool) -> serde_json::Value {
    let issues: Vec<serde_json::Value> = keys
        .iter()
        .map(|k| {
            serde_json::json!({
                "id": "10000",
                "key": k,
                "self": format!("https://example.atlassian.net/rest/api/3/issue/{}", k),
                "fields": {}
            })
        })
        .collect();
    let mut body = serde_json::json!({
        "issues": issues,
        "isLast": is_last,
    });
    if let Some(t) = next_page_token {
        body["nextPageToken"] = serde_json::json!(t);
    }
    body
}
```

- [ ] **Step 2: Verify the file compiles with no tests yet**

```bash
cd .worktrees/issue-350-search-keys
cargo build --tests 2>&1 | tail -10
```

Expected: compile error pointing at `use jr::api::jira::issues::KeySearchResult` (the struct does not exist yet). Confirms imports are wired correctly and the missing piece is exactly what later tasks add.

- [ ] **Step 3: Commit the scaffold**

```bash
git add tests/search_issue_keys.rs
git commit -m "test(search-issue-keys): scaffold wiremock test file for issue #350"
```

---

## Task 2: Write the 5 core failing tests (AC-001 happy + pagination)

**Files:**
- Modify: `tests/search_issue_keys.rs`

- [ ] **Step 1: Append the 5 core tests**

Append to `tests/search_issue_keys.rs` after the helpers:

```rust
// ---------------------------------------------------------------------------
// AC-001 (BC-2.6.050 §1) — request body asks for ONLY the `key` field.
//
// IMPORTANT — wiremock's `body_partial_json` uses
// `assert_json_diff::assert_json_include` which has SUBSET semantics for
// arrays: a matcher built from `["key"]` would ALSO match a request whose
// `fields` array was `["key", "summary", "description"]`. That would silently
// pass while BASE_ISSUE_FIELDS leaked back in. To get true length-strict
// matching on the array, we inspect the captured request post-hoc via
// `MockServer::received_requests()` and compare with `assert_eq!` on the
// serde_json::Value, which IS length-strict for arrays.
//
// Verified via Perplexity 2026-05-13 against wiremock 0.6.5 source.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_sends_fields_key_only() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_keys_response(&["FOO-1"], None, true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("project = FOO", Some(50))
        .await
        .expect("happy path must succeed");

    assert_eq!(result.keys, vec!["FOO-1".to_string()]);
    assert!(!result.has_more);

    // Length-strict assertion on `fields`: prove BASE_ISSUE_FIELDS is NOT sent.
    let requests = server
        .received_requests()
        .await
        .expect("wiremock must record requests");
    assert_eq!(requests.len(), 1, "exactly one request expected");
    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("body must be valid JSON");
    let fields = body.get("fields").expect("body must include `fields` key");
    assert_eq!(
        fields,
        &serde_json::json!(["key"]),
        "request body `fields` must be EXACTLY [\"key\"] (length-strict), got: {fields}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 (BC-2.6.050 §2) — deserialization reads only top-level `key`.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_happy_path() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_keys_response(&["FOO-1", "FOO-2", "FOO-3"], None, true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("project = FOO", None)
        .await
        .expect("happy path must succeed");

    assert_eq!(
        result.keys,
        vec!["FOO-1".to_string(), "FOO-2".to_string(), "FOO-3".to_string()],
    );
    assert!(!result.has_more, "pure exhaustion must report has_more=false");
}

// ---------------------------------------------------------------------------
// AC-003 (BC-2.6.050 §3) — paginates via nextPageToken across two pages.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_paginates_with_next_page_token() {
    let server = MockServer::start().await;

    // Page 1 — has cursor.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({"jql": "q"})))
        // No nextPageToken in body → matches the first request only because
        // we mount page 2 with a higher-specificity matcher below.
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_keys_response(&["P1-A", "P1-B"], Some("cursor-2"), false)),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Page 2 — terminal.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({"nextPageToken": "cursor-2"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_keys_response(&["P2-A"], None, true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", None).await.expect("ok");

    assert_eq!(result.keys, vec!["P1-A", "P1-B", "P2-A"]);
    assert!(!result.has_more);
}

// ---------------------------------------------------------------------------
// AC-004 (BC-2.6.050 §4) — has_more=true when limit is hit before exhaustion.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_truncates_at_limit_and_sets_has_more() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jql_keys_response(
                &["FOO-1", "FOO-2", "FOO-3"],
                None,
                true,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("project = FOO", Some(2))
        .await
        .expect("ok");

    assert_eq!(result.keys.len(), 2);
    assert!(result.has_more, "limit was hit → has_more must be true");
}

// ---------------------------------------------------------------------------
// AC-004 (BC-2.6.050 §4) — empty result is empty + has_more=false.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_returns_empty_for_no_matches() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jql_keys_response(&[], None, true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("project = NOWHERE", None)
        .await
        .expect("ok");

    assert!(result.keys.is_empty());
    assert!(!result.has_more);
}
```

- [ ] **Step 2: Verify all 5 fail with the expected compile error**

```bash
cargo test --test search_issue_keys 2>&1 | tail -20
```

Expected: compile error — `KeySearchResult` and `search_issue_keys` are not defined. Red Gate satisfied.

- [ ] **Step 3: Commit**

```bash
git add tests/search_issue_keys.rs
git commit -m "test(search-issue-keys): red — 5 core wiremock tests for AC-001..004"
```

---

## Task 3: Write the 5 remaining failing tests (edge cases + regression)

**Files:**
- Modify: `tests/search_issue_keys.rs`

- [ ] **Step 1: Append edge-case tests**

Append to `tests/search_issue_keys.rs`:

```rust
// ---------------------------------------------------------------------------
// AC-003 (BC-2.6.050 §3) — JRACLOUD-94632 repeated-cursor abort.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_repeated_cursor_aborts_with_warning() {
    let server = MockServer::start().await;

    // Two pages, both return the SAME nextPageToken `"loop"`.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_keys_response(&["X-1"], Some("loop"), false)),
        )
        .expect(1..=3) // at least one, at most 3 (defensive — guard should fire on 2nd)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client
        .search_issue_keys("q", None)
        .await
        .expect("guard must abort gracefully");

    // Keys from at least page 1 are kept; loop is broken before runaway.
    assert!(!result.keys.is_empty());
    // The eprintln! is hard to capture inside a library test without
    // forking stderr; the behavior contract is "loop aborts", which we
    // verify above by the `.expect(1..=3)` bound. The literal stderr
    // assertion is covered by a subprocess test in tests/search_issue_keys.rs
    // (test_search_issue_keys_stderr_emits_jracloud_94632_literal — added during pass-01 F-02 fix), so we don't duplicate it here.
}

// ---------------------------------------------------------------------------
// Defense-in-depth: Apr 2025 Atlassian maxResults regression
// (community.developer.atlassian.com thread 88287). Server returns more
// rows than asked for; our `limit` truncate must still hold.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_apr2025_regression_bound() {
    let server = MockServer::start().await;

    // Server returns 500 rows in a single page despite maxResults=10.
    let many: Vec<String> = (0..500).map(|i| format!("REG-{}", i)).collect();
    let many_refs: Vec<&str> = many.iter().map(String::as_str).collect();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_keys_response(&many_refs, None, true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", Some(10)).await.expect("ok");

    assert_eq!(result.keys.len(), 10, "caller-side truncate must hold");
    assert!(result.has_more, "got more than limit → has_more=true");
}

// ---------------------------------------------------------------------------
// AC-002 (BC-2.6.050 §2) — unknown top-level fields are silently ignored.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_ignores_unknown_fields() {
    let server = MockServer::start().await;

    let resp = serde_json::json!({
        "issues": [
            {
                "id": "10001",
                "key": "FOO-1",
                "self": "https://example/issue/10001",
                "fields": {},
                "expand": "names",                      // unknown top-level
                "securityLevel": {"name": "Public"}     // future-hypothetical
            }
        ],
        "isLast": true,
        "expand": "names,schema"                        // unknown response-level
    });

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resp))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", None).await.expect("ok");

    assert_eq!(result.keys, vec!["FOO-1"]);
}

// ---------------------------------------------------------------------------
// AC-003 (BC-2.6.050 §3) — 401 mid-pagination propagates as Err.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_401_mid_pagination_propagates() {
    let server = MockServer::start().await;

    // Page 1: 200 with cursor.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({"jql": "q"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_keys_response(&["P1-A"], Some("c2"), false)),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Page 2: 401.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({"nextPageToken": "c2"})))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": ["Authentication required"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", None).await;

    assert!(result.is_err(), "401 on page 2 must propagate as Err");
}

// ---------------------------------------------------------------------------
// Malformed JSON body on page 1 → serde error propagates.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issue_keys_malformed_json_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"issues": [{"key": "#), // truncated mid-string
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = test_client(&server);
    let result = client.search_issue_keys("q", None).await;

    assert!(result.is_err(), "malformed JSON must propagate as Err");
}
```

- [ ] **Step 2: Verify all 10 fail to compile**

```bash
cargo test --test search_issue_keys 2>&1 | tail -10
```

Expected: same compile error as Task 2 — `KeySearchResult` and `search_issue_keys` undefined.

- [ ] **Step 3: Commit**

```bash
git add tests/search_issue_keys.rs
git commit -m "test(search-issue-keys): red — 5 edge-case tests (cursor loop, Apr2025 bound, 401 mid-page, malformed JSON, unknown fields)"
```

---

## Task 4: Add `KeySearchResult` struct

**Files:**
- Modify: `src/api/jira/issues.rs` (after the existing `SearchResult` struct at lines 31-35)

- [ ] **Step 1: Add the struct + doc comment**

Insert after `pub struct SearchResult { ... }` (around line 35):

```rust
/// Result of a keys-only paginated issue search.
///
/// Parallel to [`SearchResult`]. The field name `keys` mirrors the `issues`
/// field name on `SearchResult` (domain-named, not generic `items`) per the
/// Rust SDK precedent surveyed in
/// `.factory/research/issue-350-search-issue-keys-design.md`.
///
/// `has_more` is `true` iff the caller's `limit` was hit AND the upstream
/// API still had more rows available; pure cursor exhaustion always
/// returns `has_more = false`. Same semantics as [`SearchResult::has_more`].
///
/// Traces to BC-2.6.050.
#[derive(Debug, Clone, PartialEq)]
pub struct KeySearchResult {
    pub keys: Vec<String>,
    pub has_more: bool,
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | tail -5
```

Expected: clean build (zero warnings, no errors).

- [ ] **Step 3: Commit**

```bash
git add src/api/jira/issues.rs
git commit -m "feat(search): add KeySearchResult struct (BC-2.6.050)"
```

---

## Task 5: Add `IssueKeyRow` private helper

**Files:**
- Modify: `src/api/jira/issues.rs`

- [ ] **Step 1: Add private deserialization helper above the `impl JiraClient` block**

Insert just before `impl JiraClient {` (around line 42):

```rust
/// Minimal deserialization target for `search_issue_keys`.
///
/// Reads ONLY the top-level `key` field per issue. Atlassian guarantees
/// `key` is at the top level regardless of the request body's `fields`
/// value; the `fields` map itself is intentionally not deserialized here
/// to keep the payload contract narrow.
///
/// Does NOT use `#[serde(deny_unknown_fields)]` — Atlassian routinely
/// adds top-level response fields and the SDK convention is to ignore
/// unknowns silently.
#[derive(Deserialize)]
struct IssueKeyRow {
    key: String,
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | tail -5
```

Expected: warning `struct IssueKeyRow is never constructed`. That is fine — the next task uses it.

- [ ] **Step 3: Commit**

```bash
git add src/api/jira/issues.rs
git commit -m "feat(search): add private IssueKeyRow deserialization helper (BC-2.6.050)"
```

---

## Task 6: Add `search_issue_keys` method

**Files:**
- Modify: `src/api/jira/issues.rs` (inside `impl JiraClient`, after `search_issues`)

- [ ] **Step 1: Add the method body**

Insert inside `impl JiraClient`, after the closing brace of `search_issues` (around line 117) and before `approximate_count`:

```rust
    /// Search issues using JQL and return ONLY the matching issue keys.
    ///
    /// Lightweight variant of [`Self::search_issues`] — requests
    /// `fields: ["key"]` in the POST body and deserializes only the
    /// top-level `key`, avoiding the ~10 KB-per-issue payload that
    /// `BASE_ISSUE_FIELDS` incurs.
    ///
    /// Use this when the caller only needs keys (e.g., JQL-driven
    /// bulk-edit selection at `cli/issue/create.rs::handle_edit`). For
    /// body-bearing reads use [`Self::search_issues`].
    ///
    /// `has_more` is `true` iff the caller's `limit` was hit AND the
    /// upstream API still had rows available; pure cursor exhaustion
    /// returns `has_more = false`. The per-page clamp `.min(100)` is a
    /// conservative client-side choice for parity with `search_issues`;
    /// Atlassian docs note that id/key-only requests can return more
    /// rows per page than full-body requests.
    ///
    /// Traces to BC-2.6.050.
    pub async fn search_issue_keys(
        &self,
        jql: &str,
        limit: Option<u32>,
    ) -> Result<KeySearchResult> {
        let max_per_page = limit.unwrap_or(50).min(100);
        let mut all_keys: Vec<String> = Vec::new();
        let mut next_page_token: Option<String> = None;

        let mut more_available = false;

        // Anti-loop guard: Jira Cloud /rest/api/3/search/jql intermittently returns
        // the same nextPageToken twice, causing infinite pagination loops. This is a
        // confirmed upstream bug — JRACLOUD-94632, JRACLOUD-92049, JRACLOUD-85546
        // (also reported in atlassian/atlassian-mcp-server#118 and
        // ankitpokhrel/jira-cli#898). Mirrors the guard in `search_issues`.
        let mut prev_cursor: Option<String> = None;

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": max_per_page,
                "fields": ["key"]
            });

            if let Some(ref token) = next_page_token {
                body["nextPageToken"] = serde_json::json!(token);
            }

            let page: CursorPage<IssueKeyRow> =
                self.post("/rest/api/3/search/jql", &body).await?;

            let page_has_more = page.has_more();
            let next_cursor = page.next_page_token.clone();
            all_keys.extend(page.issues.into_iter().map(|r| r.key));

            if let Some(max) = limit {
                if all_keys.len() >= max as usize {
                    more_available = all_keys.len() > max as usize || page_has_more;
                    all_keys.truncate(max as usize);
                    break;
                }
            }

            if !page_has_more {
                break;
            }

            // GUARD: detect repeated cursor token (next == prev) → abort + warn.
            // Mirrors `search_issues` line 100-107.
            if next_cursor.is_some() && next_cursor == prev_cursor {
                eprintln!(
                    "[jr] WARNING: Atlassian /rest/api/3/search/jql returned the same \
                     nextPageToken twice — aborting pagination loop. Some results may \
                     be missing. Upstream bug: JRACLOUD-94632."
                );
                break;
            }

            prev_cursor = next_cursor.clone();
            next_page_token = next_cursor;
        }

        Ok(KeySearchResult {
            keys: all_keys,
            has_more: more_available,
        })
    }
```

- [ ] **Step 2: Verify all 12 wiremock tests pass**

```bash
cargo test --test search_issue_keys 2>&1 | tail -15
```

Expected: `test result: ok. 12 passed; 0 failed`.

- [ ] **Step 3: Verify full test suite still green**

```bash
cargo test 2>&1 | tail -10
```

Expected: zero failures across all suites (every existing test still passes — this is purely additive).

- [ ] **Step 4: Commit**

```bash
git add src/api/jira/issues.rs
git commit -m "feat(search): add search_issue_keys method (closes red gate for BC-2.6.050)

Posts /rest/api/3/search/jql with body fields=[\"key\"] and deserializes
only the top-level key, avoiding the ~10KB-per-issue BASE_ISSUE_FIELDS
payload. Mirrors search_issues pagination, cursor handling, and the
JRACLOUD-94632 anti-loop guard verbatim.

Caller migration in src/cli/issue/create.rs lands in a follow-up commit."
```

---

## Task 7: Migrate the caller in `handle_edit::effective_keys`

**Files:**
- Modify: `src/cli/issue/create.rs:374-409`

- [ ] **Step 1: Apply the migration diff**

Replace lines 384-409 (the `--jql` branch of `effective_keys`):

```rust
        // --dry-run with --jql: search is read-only, allowed.
        let search_result = client
            .search_issue_keys(jql_str, Some(effective_max + 1))
            .await?;
        let matched_keys = search_result.keys;

        if matched_keys.is_empty() {
            return Err(JrError::UserError(format!(
                "JQL '{}' matched 0 issues. Refine your query or pass keys directly.",
                jql_str,
            ))
            .into());
        }

        if matched_keys.len() > effective_max as usize {
            return Err(JrError::UserError(format!(
                "JQL matched at least {} issues, which exceeds --max {}. \
                 Use --max <N> to allow up to {} issues, or refine your JQL.",
                matched_keys.len(),
                effective_max,
                BULK_MAX_KEYS,
            ))
            .into());
        }

        matched_keys
```

Three substantive changes:
1. `.search_issues(jql_str, Some(effective_max + 1), &[])` → `.search_issue_keys(jql_str, Some(effective_max + 1))`.
2. `let matched = search_result.issues;` → `let matched_keys = search_result.keys;` (variable rename + new field).
3. `matched.into_iter().map(|i| i.key).collect()` → `matched_keys` (no adapter — already `Vec<String>`).

Error-message text is unchanged. The `+1` lookahead is unchanged.

- [ ] **Step 2: Verify build is clean**

```bash
cargo build 2>&1 | tail -5
```

Expected: zero warnings, no errors.

- [ ] **Step 3: Verify regression: existing JQL bulk-edit tests still pass**

```bash
cargo test --test issue_bulk_pr2 2>&1 | tail -10
```

Expected: all existing `test_jql_*` cases pass. The existing tests mock `/rest/api/3/search/jql` by method+path only (no body matcher), so the new request shape (`fields: ["key"]`) is accepted; and the mock response includes top-level `key` on every issue, so `IssueKeyRow` deserialization succeeds.

- [ ] **Step 4: Verify full test suite green**

```bash
cargo test 2>&1 | tail -10
```

Expected: zero failures.

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue/create.rs
git commit -m "feat(bulk): migrate JQL bulk-edit selection to search_issue_keys (closes #350 wire-perf goal)

handle_edit::effective_keys now calls search_issue_keys, dropping the
~1MB-per-100-issues BASE_ISSUE_FIELDS payload from the bulk-edit
selection round-trip. The +1 lookahead is preserved so the exact-count
\"matched at least N\" error message is unchanged."
```

---

## Task 8: Add caller-level regression test in `tests/issue_bulk_pr2.rs`

**Files:**
- Modify: `tests/issue_bulk_pr2.rs` — append a new test after the existing `test_jql_max_above_1000_clamps_or_errors` (around line 396).

- [ ] **Step 1: Write the regression test**

Append to `tests/issue_bulk_pr2.rs`:

```rust
// ---------------------------------------------------------------------------
// Issue #350 regression: caller migrated to search_issue_keys; the
// truncation-error path must still fire with the exact existing message.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_handle_edit_jql_truncation_error_still_triggers_after_migration() {
    let server = MockServer::start().await;

    // Server returns 7 keys; user passes --max 5 → expect the
    // "matched at least N exceeds --max M" error with the +1 lookahead
    // count (6, because we request effective_max+1 = 6).
    let returned_keys = ["X-1", "X-2", "X-3", "X-4", "X-5", "X-6"];
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jql_search_response(&returned_keys)))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri())
        .args([
            "issue", "edit",
            "--jql", "project = X",
            "--max", "5",
            "--label", "add:foo",
            "--yes",
        ])
        .output()
        .expect("jr must spawn");

    assert!(
        !output.status.success(),
        "truncation error must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("matched at least 6 issues"),
        "stderr missing exact truncation count: {}",
        stderr
    );
    assert!(
        stderr.contains("exceeds --max 5"),
        "stderr missing --max 5 reference: {}",
        stderr
    );

    // Pin the perf goal at the caller level too — inspect the captured
    // request body and assert `fields` is exactly `["key"]`. This proves
    // the migration actually swapped the wire shape (not just the code
    // path). Length-strict assertion via `assert_eq!` on serde_json::Value
    // — body_partial_json arrays are subset-matched and would falsely pass
    // if BASE_ISSUE_FIELDS were leaked.
    let requests = server
        .received_requests()
        .await
        .expect("wiremock must record requests");
    let search_req = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("must have hit /search/jql");
    let body: serde_json::Value =
        serde_json::from_slice(&search_req.body).expect("body must be valid JSON");
    assert_eq!(
        body.get("fields").expect("fields key in body"),
        &serde_json::json!(["key"]),
        "post-migration: request body `fields` must be EXACTLY [\"key\"]"
    );
}
```

- [ ] **Step 2: Verify it passes**

```bash
cargo test --test issue_bulk_pr2 test_handle_edit_jql_truncation_error_still_triggers_after_migration 2>&1 | tail -10
```

Expected: `test result: ok. 1 passed`.

- [ ] **Step 3: Commit**

```bash
git add tests/issue_bulk_pr2.rs
git commit -m "test(bulk): regression-pin truncation error path after search_issue_keys migration (issue #350)"
```

---

## Task 9: Add BC-2.6.050 to the L3 PRD

**Files:**
- Modify: `.factory/specs/prd/bc-2-issue-read.md` — add a new `#### BC-2.6.050` heading after BC-2.6.049 (around line 481), AND bump frontmatter `definitional_count` from 49 → 50.
- Modify: `.factory/specs/prd/BC-INDEX.md` — bump frontmatter `total_bcs` 545 → 546 AND update the `sections:` line for `bc-2-issue-read.md` (`49 individually-bodied` → `50`).

**Note:** these files live on the `factory-artifacts` orphan branch via the `.factory/` worktree. Commit there, not on the feature branch.

- [ ] **Step 1: Read the area around BC-2.6.049 to match style**

```bash
sed -n '475,500p' /Users/zious/Documents/GITHUB/jira-cli/.factory/specs/prd/bc-2-issue-read.md
```

This shows the format of existing BC entries in the same subdomain. Match that format for the new entry.

- [ ] **Step 2: Insert BC-2.6.050 in `bc-2-issue-read.md`**

After the BC-2.6.049 block (and before the next subdomain or end-of-file), insert:

```markdown
#### BC-2.6.050: `client.search_issue_keys(jql, limit)` posts `/rest/api/3/search/jql` with body `fields: ["key"]` and returns `KeySearchResult { keys, has_more }`

**Source:** issue #350 (audit-followup from PR #348 / issue #110 PR2 Copilot review round 7); spec at `docs/specs/2026-05-13-search-issue-keys.md`; research at `.factory/research/issue-350-search-issue-keys-design.md`.

**Contract:**
- POST `/rest/api/3/search/jql` MUST send body `fields: ["key"]` (and ONLY `key` — never `BASE_ISSUE_FIELDS` content).
- Deserializes only the top-level `key` per issue; ignores `fields {}` body content and unknown top-level fields.
- Paginates via `nextPageToken` cursor identically to `search_issues`, including the JRACLOUD-94632 repeated-cursor anti-loop guard with the same stderr warning text.
- Returns `KeySearchResult { keys: Vec<String>, has_more: bool }`. `has_more = true` iff caller-side truncation occurred (the `limit` argument was hit AND the upstream API still had rows); pure cursor exhaustion always returns `has_more = false`.
- Clamps `maxResults` per page to `.min(100)` for parity with `search_issues` (NOT an API limit; conservative client-side choice).

**Implementation:** `src/api/jira/issues.rs::search_issue_keys`.

**Caller:** `src/cli/issue/create.rs::handle_edit::effective_keys` (the JQL-driven bulk-edit selection path).

**Tests:** `tests/search_issue_keys.rs` (12 wiremock tests pinning the contract above — 11 library tokio + 1 subprocess) and `tests/issue_bulk_pr2.rs::test_handle_edit_jql_truncation_error_still_triggers_after_migration` (caller-level regression).
```

- [ ] **Step 3: Bump frontmatter `definitional_count` in `bc-2-issue-read.md`**

Change the frontmatter line `definitional_count: 49` → `definitional_count: 50`.

- [ ] **Step 4: Bump `total_bcs` in `BC-INDEX.md`**

Change `total_bcs: 545` → `total_bcs: 546` AND update the comment that follows it. Add a note: `# +1 added 2026-05-13 (BC-2.6.050, issue #350)`.

- [ ] **Step 5: Bump the sections line in `BC-INDEX.md`**

Change `- bc-2-issue-read.md (91 BCs cumulative; 49 individually-bodied)` → `- bc-2-issue-read.md (91 BCs cumulative; 50 individually-bodied)`.

- [ ] **Step 6: Verify counts are consistent**

```bash
bash /Users/zious/Documents/GITHUB/jira-cli/scripts/check-spec-counts.sh
```

Expected: exit 0 with no drift output.

- [ ] **Step 7: Commit on factory-artifacts**

```bash
cd /Users/zious/Documents/GITHUB/jira-cli/.factory
git add specs/prd/bc-2-issue-read.md specs/prd/BC-INDEX.md
git commit -m "spec(prd): add BC-2.6.050 — search_issue_keys keys-only API (issue #350)"
```

---

## Task 10: Update `CLAUDE.md` blurb

**Files:**
- Modify: `CLAUDE.md` — the `src/api/jira/issues.rs` line in the architecture tree (currently `issues.rs    # search, get, create, edit, list comments`).

- [ ] **Step 1: Update the line**

Change the line:

```
│       ├── issues.rs    # search, get, create, edit, list comments
```

to:

```
│       ├── issues.rs    # search (full + keys-only), get, create, edit, list comments
```

- [ ] **Step 2: Verify the change**

```bash
grep -n "issues.rs" /Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-350-search-keys/CLAUDE.md | head -2
```

Expected: matching line shows the new text.

- [ ] **Step 3: Commit on feature branch**

```bash
cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-350-search-keys
git add CLAUDE.md
git commit -m "docs(claude-md): note search_issue_keys alongside search_issues in issues.rs blurb (issue #350)"
```

---

## Task 11: Final release-gate check

**Files:** none modified — verification only.

- [ ] **Step 1: Run `cargo fmt --check`**

```bash
cargo fmt --all -- --check 2>&1 | tail -5
```

Expected: no output (no formatting violations). Stale formatting is the #1 CI failure mode — DO NOT skip this step.

- [ ] **Step 2: Run `cargo clippy -- -D warnings`**

```bash
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: zero warnings. If clippy fires, fix the root cause — DO NOT add `#[allow]` without explicit user approval (project rule from CLAUDE.md).

- [ ] **Step 3: Run the full test suite**

```bash
cargo test 2>&1 | tail -15
```

Expected: all suites green, including the 12 new `tests/search_issue_keys.rs` cases (11 library tokio + 1 subprocess) and the 1 new regression in `tests/issue_bulk_pr2.rs`.

- [ ] **Step 4: Run `scripts/check-spec-counts.sh`**

```bash
bash /Users/zious/Documents/GITHUB/jira-cli/scripts/check-spec-counts.sh
```

Expected: exit 0 (BC counts are aligned).

- [ ] **Step 5: Optional — verify the perf win is real**

```bash
cargo test --test search_issue_keys test_search_issue_keys_sends_fields_key_only -- --nocapture 2>&1 | tail -5
```

Expected: pass. This test uses `MockServer::received_requests()` and `assert_eq!` on `serde_json::Value` to enforce length-strict matching that `body["fields"] == json!(["key"])` exactly — proving the wire path sends ONLY `key` and not `BASE_ISSUE_FIELDS`. Note: `body_partial_json` was NOT used here because its array semantics are subset-matching (`assert_json_include`), which would silently pass even if `BASE_ISSUE_FIELDS` leaked into the request. See spec §Risks for the full retraction note on the `body_partial_json` claim.

- [ ] **Step 6: No commit required — proceed to PR creation**

---

## Task 12: Push and open PR

**Files:** none modified.

- [ ] **Step 1: Verify branch is in sync with develop**

```bash
cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-350-search-keys
git fetch origin develop --quiet
git log --oneline origin/develop..HEAD | head -10
```

Expected: a short list of feature commits, all on top of `origin/develop`.

- [ ] **Step 2: Push the branch**

```bash
git push -u origin feat/issue-350-search-issue-keys
```

- [ ] **Step 3: Open the PR with Copilot review requested**

```bash
gh pr create --base develop --title "feat(bulk): add search_issue_keys lightweight API for JQL bulk-edit selection (closes #350)" --body "$(cat <<'EOF'
## Summary

Adds `JiraClient::search_issue_keys(jql, limit) -> Result<KeySearchResult>` —
a lightweight variant of `search_issues` that posts `/rest/api/3/search/jql`
with body `fields: ["key"]` and deserializes only the top-level `key`.

Migrates the single keys-only caller at `src/cli/issue/create.rs:386`
(the JQL-driven bulk-edit selection path) to use it.

Wire payload for the JQL bulk-edit selection round-trip drops from
~1 MB/100 issues to ~100 bytes/page.

## Spec & Story

- Public spec: `docs/specs/2026-05-13-search-issue-keys.md`
- VSDD F3 story: `.factory/code-delivery/issue-350/story.md`
- Research: `.factory/research/issue-350-search-issue-keys-design.md`
  (includes Q1–Q8 adversarial-pass addendum)
- New BC-2.6.050 (subdomain 2.6 in `bc-2-issue-read.md`)

## Test plan

- [x] `cargo fmt --check` passes
- [x] `cargo clippy --all-targets -- -D warnings` passes
- [x] `cargo test` — full suite green (all existing + 12 new wiremock tests
      in `tests/search_issue_keys.rs` (11 library tokio + 1 subprocess) + 1 regression test in `tests/issue_bulk_pr2.rs`)
- [x] `scripts/check-spec-counts.sh` exits 0
- [x] `test_search_issue_keys_sends_fields_key_only` proves wire body sent
      `fields: ["key"]` exactly (length-strict via `MockServer::received_requests()` + `assert_eq!` on `serde_json::Value`, NOT wiremock's `body_partial_json` which is subset-matching)
- [x] Existing `test_jql_*` cases pass unchanged after caller migration

## Follow-up issue

A separate small PR will tighten the JRACLOUD-94632 stderr warning text —
the current wording overclaims "server bug" but live-data drift (JRACLOUD-95368)
can also legitimately trigger it. Inherited verbatim by this PR; file
separately to keep diff focused.
EOF
)"
```

- [ ] **Step 4: Request Copilot review**

```bash
PR=$(gh pr view --json number -q .number)
gh api repos/Zious11/jira-cli/pulls/$PR/requested_reviewers \
  --method POST -f 'reviewers[]=copilot-pull-request-reviewer[bot]'
```

- [ ] **Step 5: Capture PR URL for the orchestrator**

```bash
gh pr view --json url -q .url
```

Hand the URL back to the orchestrator. Phase 8 (Copilot review iterate-until-clean) takes over from here.

---

## Task 13: File the JRACLOUD-94632 follow-up issue

**Files:** none modified.

- [ ] **Step 1: File the follow-up issue**

```bash
gh issue create --title "chore(search): tighten JRACLOUD-94632 stderr warning — repeated cursors can be live-data drift, not just server bug" --label "enhancement" --body "$(cat <<'EOF'
**Source:** issue #350 adversarial-pass addendum §Q4 — the existing JRACLOUD-94632 stderr warning in `src/api/jira/issues.rs::search_issues` (lines 101-105) and the new `search_issue_keys` (inherited verbatim) overclaim "server bug" as the trigger condition for repeated `nextPageToken` aborts.

Per JRACLOUD-95368 community thread, repeated cursors can also legitimately arise from live-data drift (issues added or removed between page fetches), not just the JRACLOUD-94632 server-side echo bug. The current wording misattributes user-visible loop aborts to a server-only cause when ~5% of real-world triggers are data drift.

**Suggested fix:** Reword to "aborting pagination loop. Repeated cursors can be caused by upstream bug JRACLOUD-94632 OR by live data changes during the search. Some results may be missing." Apply at both call sites (`search_issues` AND `search_issue_keys`).

**Acceptance criteria:**
- Both call sites use the same updated wording.
- The literal substring "JRACLOUD-94632" remains (so users still have a copy-pasteable upstream search term).
- An additional substring covering data-drift is present.
- Update any existing test that asserts on the stderr text.

**Effort:** ~15 minutes — one-line wording change, two tests to update.
EOF
)"
```

- [ ] **Step 2: Reference the new issue in the PR body** (if not already referenced)

If the issue number isn't already in the PR body's "Follow-up issue" section, edit the PR:

```bash
gh pr edit $(gh pr view --json number -q .number) --body "$(gh pr view --json body -q .body | sed 's/A separate small PR will tighten/See linked follow-up issue #<NEW> — tightens/')"
```

(Where `<NEW>` is the issue number printed by step 1.)

---

## Self-Review Summary

| Spec section | Plan task(s) |
|---|---|
| New `KeySearchResult` struct (spec §Design API shape) | Task 4 |
| New `IssueKeyRow` private helper (spec §Wire shape) | Task 5 |
| `search_issue_keys` method body + pagination + anti-loop (spec §Design + §Pagination loop) | Task 6 |
| Caller migration at `create.rs:386` (spec §Caller migration) | Task 7 |
| Test `..._sends_fields_key_only` (AC-001) | Task 2 |
| Tests `..._happy_path`, `..._paginates_*`, `..._truncates_*`, `..._returns_empty_*` (AC-002..004) | Tasks 2, 3 |
| Tests `..._repeated_cursor_*`, `..._apr2025_*`, `..._ignores_unknown_*`, `..._401_*`, `..._malformed_*` | Task 3 |
| Caller-level regression test (AC-005) | Task 8 |
| BC-2.6.050 + count bumps + check-spec-counts.sh (spec §Doc and spec fallout) | Task 9 |
| CLAUDE.md blurb update | Task 10 |
| Rustdoc per AC-006 | Tasks 4, 5, 6 (rustdoc written inline with each declaration) |
| Release-gate (cargo test, clippy, fmt) — AC-007 | Task 11 |
| Push + open PR + request Copilot | Task 12 |
| File JRACLOUD-94632 follow-up (spec §Out of Scope) | Task 13 |

Every AC traces to at least one task. No placeholders. Method signatures and field names match across tasks (`KeySearchResult.keys`, `IssueKeyRow.key`, `search_issue_keys`).
