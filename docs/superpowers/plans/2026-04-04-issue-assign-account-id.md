# Issue Assign/Create --account-id Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--account-id` flag to `issue assign` and `issue create` as a mutually exclusive alternative to `--to` that bypasses name search and passes the accountId directly to the Jira API.

**Architecture:** Add an `account_id: Option<String>` field with `conflicts_with` to both CLI enum variants. In the handlers, branch on `account_id` before the existing `to` branch — if present, skip all user resolution and use the ID directly. Also fix the `issue create` assignee field from `{"id": ...}` to `{"accountId": ...}` (the documented Jira Cloud v3 format).

**Tech Stack:** Rust, clap (derive), wiremock, serde_json

**Spec:** `docs/superpowers/specs/2026-04-04-issue-assign-account-id-design.md`

---

## File Map

| File | Change type | What changes |
|------|-------------|--------------|
| `src/cli/mod.rs` | Modify | Add `account_id` field to `Assign` and `Create` enum variants |
| `src/cli/issue/workflow.rs` | Modify | Branch on `account_id` in `handle_assign`, update destructuring |
| `src/cli/issue/create.rs` | Modify | Branch on `account_id` in `handle_create`, fix `id` → `accountId`, update destructuring |
| `tests/issue_commands.rs` | Modify | Update two existing tests (`{"id":...}` → `{"accountId":...}`), add two new integration tests |

---

### Task 1: Add `--account-id` to `issue assign` with integration test

**Files:**
- Modify: `src/cli/mod.rs:308-317`
- Modify: `src/cli/issue/workflow.rs:282,307-312`
- Modify: `tests/issue_commands.rs` (append new test)

- [ ] **Step 1: Add `account_id` field to the `Assign` variant**

In `src/cli/mod.rs`, change lines 308-317 from:

```rust
    Assign {
        /// Issue key
        key: String,
        /// Assign to this user (omit to assign to self)
        #[arg(long)]
        to: Option<String>,
        /// Remove assignee
        #[arg(long)]
        unassign: bool,
    },
```

To:

```rust
    Assign {
        /// Issue key
        key: String,
        /// Assign to this user (omit to assign to self)
        #[arg(long, conflicts_with = "account_id")]
        to: Option<String>,
        /// Assign to this Jira accountId directly (bypasses name search)
        #[arg(long, conflicts_with_all = ["to", "unassign"])]
        account_id: Option<String>,
        /// Remove assignee
        #[arg(long)]
        unassign: bool,
    },
```

- [ ] **Step 2: Update destructuring and add `account_id` branch in `handle_assign`**

In `src/cli/issue/workflow.rs`, change line 282 from:

```rust
    let IssueCommand::Assign { key, to, unassign } = command else {
```

To:

```rust
    let IssueCommand::Assign { key, to, account_id, unassign } = command else {
```

Then change lines 307-312 from:

```rust
    // Resolve account ID and display name
    let (account_id, display_name) = if let Some(ref user_query) = to {
        helpers::resolve_assignee(client, user_query, &key, no_input).await?
    } else {
        let me = client.get_myself().await?;
        (me.account_id, me.display_name)
    };
```

To:

```rust
    // Resolve account ID and display name
    let (account_id, display_name) = if let Some(ref id) = account_id {
        (id.clone(), id.clone())
    } else if let Some(ref user_query) = to {
        helpers::resolve_assignee(client, user_query, &key, no_input).await?
    } else {
        let me = client.get_myself().await?;
        (me.account_id, me.display_name)
    };
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -20`

Expected: Build succeeds (or only warnings, no errors).

- [ ] **Step 4: Write integration test for assign with `--account-id`**

Append this test at the end of `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_assign_issue_with_account_id() {
    let server = MockServer::start().await;

    // Mock GET issue — currently unassigned
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ACC-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_response_with_assignee(
                    "ACC-1",
                    "Test assign by accountId",
                    None,
                )),
        )
        .mount(&server)
        .await;

    // Mock PUT assignee — verify accountId in request body
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/ACC-1/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": "direct-account-id-456"
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    // Assign directly by accountId — no user search mock needed
    client
        .assign_issue("ACC-1", Some("direct-account-id-456"))
        .await
        .unwrap();

    // Verify idempotent check works: mock issue as already assigned
    let server2 = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ACC-2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_response_with_assignee(
                    "ACC-2",
                    "Already assigned",
                    Some(("direct-account-id-456", "direct-account-id-456")),
                )),
        )
        .mount(&server2)
        .await;

    let client2 =
        jr::api::client::JiraClient::new_for_test(server2.uri(), "Basic dGVzdDp0ZXN0".into());

    let issue = client2.get_issue("ACC-2", &[]).await.unwrap();
    let assignee = issue.fields.assignee.unwrap();
    assert_eq!(assignee.account_id, "direct-account-id-456");
}
```

- [ ] **Step 5: Run the new test**

Run: `cargo test --test issue_commands test_assign_issue_with_account_id`

Expected: PASS. The test exercises the API layer directly (same pattern as existing create-with-assignee tests). No user search mocks are registered, confirming the accountId path bypasses name resolution.

- [ ] **Step 6: Run all tests to verify no regressions**

Run: `cargo test`

Expected: All tests pass.

- [ ] **Step 7: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`

Expected: No warnings, no format violations.

- [ ] **Step 8: Commit**

```bash
git add src/cli/mod.rs src/cli/issue/workflow.rs tests/issue_commands.rs
git commit -m "feat: add --account-id flag to issue assign (#115)"
```

---

### Task 2: Add `--account-id` to `issue create`, fix `id` → `accountId`, and update existing tests

**Files:**
- Modify: `src/cli/mod.rs:248-250`
- Modify: `src/cli/issue/create.rs:21-34,127-131`
- Modify: `tests/issue_commands.rs:994-999,1047-1049,1027` (update existing tests, append new test)

- [ ] **Step 1: Add `account_id` field to the `Create` variant**

In `src/cli/mod.rs`, change lines 248-250 from:

```rust
        /// Assign to user (name/email, or "me" for self)
        #[arg(long)]
        to: Option<String>,
    },
```

To:

```rust
        /// Assign to user (name/email, or "me" for self)
        #[arg(long, conflicts_with = "account_id")]
        to: Option<String>,
        /// Assign to this Jira accountId directly (bypasses name search)
        #[arg(long, conflicts_with = "to")]
        account_id: Option<String>,
    },
```

- [ ] **Step 2: Update destructuring and add `account_id` branch in `handle_create`**

In `src/cli/issue/create.rs`, change lines 21-34 from:

```rust
    let IssueCommand::Create {
        project,
        issue_type,
        summary,
        description,
        description_stdin,
        priority,
        label: labels,
        team,
        points,
        markdown,
        parent,
        to,
    } = command
```

To:

```rust
    let IssueCommand::Create {
        project,
        issue_type,
        summary,
        description,
        description_stdin,
        priority,
        label: labels,
        team,
        points,
        markdown,
        parent,
        to,
        account_id,
    } = command
```

Then change lines 127-131 from:

```rust
    if let Some(ref user_query) = to {
        let (account_id, _display_name) =
            helpers::resolve_assignee_by_project(client, user_query, &project_key, no_input)
                .await?;
        fields["assignee"] = json!({"id": account_id});
    }
```

To:

```rust
    if let Some(ref id) = account_id {
        fields["assignee"] = json!({"accountId": id});
    } else if let Some(ref user_query) = to {
        let (acct_id, _display_name) =
            helpers::resolve_assignee_by_project(client, user_query, &project_key, no_input)
                .await?;
        fields["assignee"] = json!({"accountId": acct_id});
    }
```

- [ ] **Step 3: Update existing test `test_create_issue_with_assignee`**

In `tests/issue_commands.rs`, change the body matcher at line 994-999 from:

```rust
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "FOO"},
                "issuetype": {"name": "Task"},
                "summary": "Test with assignee",
                "assignee": {"id": "acc-jane-123"}
            }
        })))
```

To:

```rust
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "FOO"},
                "issuetype": {"name": "Task"},
                "summary": "Test with assignee",
                "assignee": {"accountId": "acc-jane-123"}
            }
        })))
```

And change the field assignment at line 1027 from:

```rust
    fields["assignee"] = serde_json::json!({"id": users[0].account_id});
```

To:

```rust
    fields["assignee"] = serde_json::json!({"accountId": users[0].account_id});
```

- [ ] **Step 4: Update existing test `test_create_issue_with_assignee_me`**

In `tests/issue_commands.rs`, change the body matcher at line 1047-1049 from:

```rust
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "assignee": {"id": "abc123"}
            }
        })))
```

To:

```rust
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "assignee": {"accountId": "abc123"}
            }
        })))
```

And change the field assignment at line 1072 from:

```rust
    fields["assignee"] = serde_json::json!({"id": me.account_id});
```

To:

```rust
    fields["assignee"] = serde_json::json!({"accountId": me.account_id});
```

- [ ] **Step 5: Write integration test for create with `--account-id`**

Append this test at the end of `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_create_issue_with_account_id() {
    let server = MockServer::start().await;

    // Mock create issue — verify assignee uses accountId format
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "FOO"},
                "issuetype": {"name": "Task"},
                "summary": "Assigned by accountId",
                "assignee": {"accountId": "direct-acct-789"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("FOO-200")),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // Build fields with accountId directly — no user search mock needed
    let mut fields = serde_json::json!({
        "project": {"key": "FOO"},
        "issuetype": {"name": "Task"},
        "summary": "Assigned by accountId",
    });
    fields["assignee"] = serde_json::json!({"accountId": "direct-acct-789"});

    let response = client.create_issue(fields).await.unwrap();
    assert_eq!(response.key, "FOO-200");
}
```

- [ ] **Step 6: Run the new test**

Run: `cargo test --test issue_commands test_create_issue_with_account_id`

Expected: PASS.

- [ ] **Step 7: Run all tests to verify no regressions**

Run: `cargo test`

Expected: All tests pass. The updated existing tests (`test_create_issue_with_assignee`, `test_create_issue_with_assignee_me`) pass because both the production code and test assertions now use `{"accountId": ...}`.

- [ ] **Step 8: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`

Expected: No warnings, no format violations.

- [ ] **Step 9: Commit**

```bash
git add src/cli/mod.rs src/cli/issue/create.rs tests/issue_commands.rs
git commit -m "feat: add --account-id flag to issue create, fix assignee field to accountId (#115)"
```
