# Handler-Level Tests for --to me and Idempotent Name Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 3 handler-level integration tests covering `--to me` keyword resolution in assign and create, plus idempotent assign when account ID comes from name search.

**Architecture:** All tests follow the existing pattern in `tests/cli_handler.rs`: wiremock MockServer for API mocking, `jr_cmd()` helper for command execution, JSON output assertions. No production code changes — these are pure test additions covering untested sub-paths.

**Tech Stack:** Rust, wiremock, assert_cmd, predicates, tokio (multi_thread)

---

## File Structure

| File | Role | Action |
|------|------|--------|
| `tests/cli_handler.rs` | Handler-level integration tests | Modify: add 3 new test functions after existing tests (line 323) |

---

### Task 1: Add handler-level tests for --to me and idempotent name resolution

**Files:**
- Modify: `tests/cli_handler.rs:323` (append after `test_handler_create_basic`)

- [ ] **Step 1: Add `test_handler_assign_to_me`**

Append this test after the closing `}` of `test_handler_create_basic` (line 323):

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_assign_to_me() {
    let server = MockServer::start().await;

    // Mock GET myself — the "me" keyword resolves via get_myself()
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_response()))
        .mount(&server)
        .await;

    // Mock GET issue — currently unassigned
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-6"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("HDL-6", "Assign to me test", None),
        ))
        .mount(&server)
        .await;

    // Mock PUT assignee
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-6/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": "abc123"
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-6", "--to", "me"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed\": true"))
        .stdout(predicate::str::contains("\"assignee\": \"Test User\""))
        .stdout(predicate::str::contains(
            "\"assignee_account_id\": \"abc123\"",
        ));
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --test cli_handler test_handler_assign_to_me -- --exact`

Expected: PASS. The test exercises the `--to me` → `resolve_assignee` → `is_me_keyword` → `get_myself()` code path, which is already implemented — we're just adding coverage.

- [ ] **Step 3: Add `test_handler_create_to_me`**

Append this test after `test_handler_assign_to_me`:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_create_to_me() {
    let server = MockServer::start().await;

    // Mock GET myself — the "me" keyword resolves via get_myself()
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_response()))
        .mount(&server)
        .await;

    // Mock POST create issue — verify assignee uses accountId from get_myself()
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "HDL"},
                "issuetype": {"name": "Task"},
                "summary": "Created with --to me",
                "assignee": {"accountId": "abc123"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("HDL-200")),
        )
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args([
            "issue",
            "create",
            "-p",
            "HDL",
            "-t",
            "Task",
            "-s",
            "Created with --to me",
            "--to",
            "me",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\": \"HDL-200\""));
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --test cli_handler test_handler_create_to_me -- --exact`

Expected: PASS. The test exercises `--to me` → `resolve_assignee_by_project` → `is_me_keyword` → `get_myself()` during issue creation.

- [ ] **Step 5: Add `test_handler_assign_idempotent_with_name_search`**

Append this test after `test_handler_create_to_me`:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_assign_idempotent_with_name_search() {
    let server = MockServer::start().await;

    // Mock assignable user search — returns Jane Doe
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .and(query_param("query", "Jane"))
        .and(query_param("issueKey", "HDL-7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![("acc-jane-456", "Jane Doe", true)]),
        ))
        .mount(&server)
        .await;

    // Mock GET issue — already assigned to Jane Doe (same account ID)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee(
                "HDL-7",
                "Already assigned to Jane",
                Some(("acc-jane-456", "Jane Doe")),
            ),
        ))
        .mount(&server)
        .await;

    // PUT assignee should NOT be called — already assigned to target
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-7/assignee"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "assign", "HDL-7", "--to", "Jane"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed\": false"));
}
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test --test cli_handler test_handler_assign_idempotent_with_name_search -- --exact`

Expected: PASS. The test exercises `--to Jane` → `resolve_assignee` → user search → single result → idempotent check → already assigned → returns `changed: false` without calling PUT.

- [ ] **Step 7: Run all handler tests together**

Run: `cargo test --test cli_handler`

Expected: All 11 tests pass (8 existing + 3 new).

- [ ] **Step 8: Run clippy**

Run: `cargo clippy --tests -- -D warnings`

Expected: Zero warnings.

- [ ] **Step 9: Commit**

```bash
git add tests/cli_handler.rs
git commit -m "test: add handler-level tests for --to me keyword and idempotent name search (#148)"
```
