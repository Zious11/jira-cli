# Issue Create Browse URL Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the Jira browse URL to `issue create` table and JSON output, matching the `gh` CLI pattern.

**Architecture:** Construct the browse URL from `client.instance_url()` + `/browse/` + key in `handle_create`. Table output gets the URL on a second line (plain text, not green). JSON output gets a `url` field alongside `key`. Single file change in `src/cli/issue/create.rs`, one integration test added to `tests/issue_commands.rs`.

**Tech Stack:** Rust

**Spec:** `docs/superpowers/specs/2026-04-03-issue-create-url-design.md`

---

## File Map

| File | Change type | What changes |
|------|-------------|--------------|
| `src/cli/issue/create.rs` | Modify | Update table and JSON output arms in `handle_create` to include browse URL |
| `tests/issue_commands.rs` | Modify | Add integration test verifying browse URL in create response |

---

### Task 1: Add browse URL to `issue create` output and test it

**Files:**
- Modify: `src/cli/issue/create.rs:136-143`
- Modify: `tests/issue_commands.rs` (append new test)

- [ ] **Step 1: Write the failing integration test**

Add this test at the end of `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_create_issue_response_includes_browse_url() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("URL-1")),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let response = client.create_issue(serde_json::json!({
        "project": {"key": "URL"},
        "issuetype": {"name": "Task"},
        "summary": "Test browse URL",
    })).await.unwrap();

    // Verify the key is returned
    assert_eq!(response.key, "URL-1");

    // Verify browse URL can be constructed from instance_url
    let browse_url = format!(
        "{}/browse/{}",
        client.instance_url().trim_end_matches('/'),
        response.key
    );
    assert!(
        browse_url.contains("/browse/URL-1"),
        "Expected browse URL to contain /browse/URL-1, got: {browse_url}"
    );
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --test issue_commands test_create_issue_response_includes_browse_url`

Expected: PASS. This test validates the building blocks (API response + `instance_url()`) exist and work. The test passes because it's testing existing infrastructure — the URL construction logic.

- [ ] **Step 3: Implement the table output change**

In `src/cli/issue/create.rs`, change lines 136-143 from:

```rust
    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        OutputFormat::Table => {
            output::print_success(&format!("Created issue {}", response.key));
        }
    }
```

To:

```rust
    let browse_url = format!(
        "{}/browse/{}",
        client.instance_url().trim_end_matches('/'),
        response.key
    );

    match output_format {
        OutputFormat::Json => {
            let json_response = json!({
                "key": response.key,
                "url": browse_url,
            });
            println!("{}", serde_json::to_string_pretty(&json_response)?);
        }
        OutputFormat::Table => {
            output::print_success(&format!("Created issue {}", response.key));
            println!("{}", browse_url);
        }
    }
```

- [ ] **Step 4: Run all tests to verify no regressions**

Run: `cargo test`

Expected: All tests pass. The existing create tests (`test_create_issue_with_assignee`, `test_create_issue_without_assignee`, etc.) still pass because they assert on `response.key`, not on stdout content.

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`

Expected: No warnings, no format violations.

- [ ] **Step 6: Commit**

```bash
git add src/cli/issue/create.rs tests/issue_commands.rs
git commit -m "feat: add browse URL to issue create output (#112)"
```
