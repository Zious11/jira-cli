# Queue Case-Insensitive Duplicate Name Test Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an integration test that exercises the case-insensitive `to_lowercase()` filter in `resolve_queue_by_name` using mixed-case queue names.

**Architecture:** One wiremock-backed integration test appended to `tests/queue.rs`. Mocks two queues with different casing ("Triage" and "TRIAGE"), calls `resolve_queue_by_name` with lowercase input ("triage"), and asserts on the error message. No production code changes.

**Tech Stack:** Rust, wiremock, tokio

**Spec:** `docs/superpowers/specs/2026-04-03-queue-case-insensitive-test-design.md`

---

## File Map

| File | Change type | What changes |
|------|-------------|--------------|
| `tests/queue.rs` | Modify | Append one integration test |

---

### Task 1: Add case-insensitive duplicate queue name integration test

**Files:**
- Modify: `tests/queue.rs:229` (append after existing test)

- [ ] **Step 1: Write the integration test**

Add this test at the end of `tests/queue.rs`:

```rust
#[tokio::test]
async fn resolve_queue_mixed_case_duplicate_names_error_message() {
    let server = MockServer::start().await;

    // Two queues with the same name but different casing
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "30", "name": "Triage", "issueCount": 5 },
                { "id": "40", "name": "TRIAGE", "issueCount": 3 }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    // Lowercase input — matches neither stored name exactly,
    // forcing both sides of to_lowercase() to do work
    let result = jr::cli::queue::resolve_queue_by_name("15", "triage", &client).await;

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Multiple queues named \"Triage\""),
        "Expected queue name in error, got: {msg}"
    );
    assert!(
        msg.contains("30, 40"),
        "Expected both queue IDs in error, got: {msg}"
    );
    assert!(
        msg.contains("Use --id 30 to specify"),
        "Expected --id suggestion in error, got: {msg}"
    );
}
```

**Why these assertions work:** `partial_match("triage", &["Triage", "TRIAGE"])` returns `ExactMultiple("Triage")` — the first candidate whose `to_lowercase()` matches the input. The error message uses this matched name. Then `resolve_queue_by_name` filters queues via `q.name.to_lowercase() == "triage"`, collecting both IDs `["30", "40"]`.

- [ ] **Step 2: Run the new test to verify it passes**

Run: `cargo test --test queue resolve_queue_mixed_case_duplicate_names_error_message`

Expected: PASS. The production code at `src/cli/queue.rs:155-158` already handles case-insensitive filtering correctly.

- [ ] **Step 3: Verify the test would fail without `to_lowercase()`**

This is a mental verification, not a code change. If `resolve_queue_by_name` used `q.name == name` instead of `q.name.to_lowercase() == name_lower`, the filter at line 158 would match zero queues when called with `"triage"` against `"Triage"` and `"TRIAGE"`, causing a panic on `ids[0]` (empty vector). This confirms the test exercises the `to_lowercase()` logic.

- [ ] **Step 4: Run all tests to verify no regressions**

Run: `cargo test`

Expected: All tests pass. The existing `resolve_queue_duplicate_names_error_message` test is unchanged.

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`

Expected: No warnings, no format violations.

- [ ] **Step 6: Commit**

```bash
git add tests/queue.rs
git commit -m "test: add case-insensitive duplicate queue name integration test (#131)"
```
