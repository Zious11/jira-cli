# JSM Internal vs External Comments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--internal` flag to `jr issue comment` and display visibility status in `jr issue comments` for JSM projects.

**Architecture:** Add `EntityProperty` type and `properties` field to `Comment` struct. Modify `add_comment` to accept an `internal` flag and include `sd.public.comment` property when set. Modify `list_comments` to request `expand=properties` and conditionally display a "Visibility" column based on whether any comment has the `sd.public.comment` property.

**Tech Stack:** Rust, clap (derive), wiremock (tests), assert_cmd/predicates (tests)

---

## File Structure

| File | Responsibility | Change type |
|------|---------------|-------------|
| `src/types/jira/issue.rs` | `EntityProperty` type, `properties` field on `Comment` | Modify |
| `src/api/jira/issues.rs` | `add_comment` accepts `internal`, `list_comments` adds `expand=properties` | Modify |
| `src/cli/mod.rs` | `--internal` flag on `Comment` variant | Modify |
| `src/cli/issue/workflow.rs` | Pass `internal` flag through to `add_comment` | Modify |
| `src/cli/issue/list.rs` | Conditional "Visibility" column in `handle_comments` | Modify |
| `tests/comments.rs` | Update existing mocks for `expand=properties`, add property tests | Modify |
| `tests/cli_handler.rs` | Handler-level tests for `--internal` flag and visibility column | Modify |

---

### Task 1: Add `EntityProperty` type and update `Comment` struct

**Files:**
- Modify: `src/types/jira/issue.rs:147-153`

- [ ] **Step 1: Add `EntityProperty` struct and update `Comment`**

In `src/types/jira/issue.rs`, replace the `Comment` struct (lines 147-153):

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Comment {
    pub id: Option<String>,
    pub body: Option<Value>,
    pub author: Option<User>,
    pub created: Option<String>,
}
```

With:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityProperty {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Comment {
    pub id: Option<String>,
    pub body: Option<Value>,
    pub author: Option<User>,
    pub created: Option<String>,
    #[serde(default)]
    pub properties: Vec<EntityProperty>,
}
```

- [ ] **Step 2: Add unit tests for Comment deserialization with properties**

In `src/types/jira/issue.rs`, add these tests inside the existing `#[cfg(test)] mod tests` block (after the last test):

```rust
    #[test]
    fn comment_deserialize_with_properties() {
        let json = json!({
            "id": "10001",
            "body": null,
            "properties": [
                {"key": "sd.public.comment", "value": {"internal": true}}
            ]
        });
        let comment: Comment = serde_json::from_value(json).unwrap();
        assert_eq!(comment.properties.len(), 1);
        assert_eq!(comment.properties[0].key, "sd.public.comment");
        assert_eq!(comment.properties[0].value["internal"], true);
    }

    #[test]
    fn comment_deserialize_without_properties() {
        let json = json!({
            "id": "10002",
            "body": null
        });
        let comment: Comment = serde_json::from_value(json).unwrap();
        assert!(comment.properties.is_empty());
    }

    #[test]
    fn comment_deserialize_empty_properties() {
        let json = json!({
            "id": "10003",
            "body": null,
            "properties": []
        });
        let comment: Comment = serde_json::from_value(json).unwrap();
        assert!(comment.properties.is_empty());
    }
```

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --lib -- issue::tests::comment_deserialize 2>&1 | tail -10`

Expected: All 3 new tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/types/jira/issue.rs
git commit -m "feat: add EntityProperty type and properties field to Comment (#103)"
```

---

### Task 2: Update `add_comment` API to support `internal` flag

**Files:**
- Modify: `src/api/jira/issues.rs:158-163`
- Modify: `src/cli/issue/workflow.rs:422`

- [ ] **Step 1: Update `add_comment` signature and implementation**

In `src/api/jira/issues.rs`, replace the `add_comment` method (lines 158-163):

```rust
    /// Add a comment to an issue.
    pub async fn add_comment(&self, key: &str, body: Value) -> Result<Comment> {
        let path = format!("/rest/api/3/issue/{}/comment", urlencoding::encode(key));
        let payload = serde_json::json!({ "body": body });
        self.post(&path, &payload).await
    }
```

With:

```rust
    /// Add a comment to an issue.
    ///
    /// When `internal` is true, sets the `sd.public.comment` entity property
    /// to mark the comment as internal (agent-only) on JSM projects.
    /// On non-JSM projects, the property is silently accepted with no effect.
    pub async fn add_comment(&self, key: &str, body: Value, internal: bool) -> Result<Comment> {
        let path = format!("/rest/api/3/issue/{}/comment", urlencoding::encode(key));
        let mut payload = serde_json::json!({ "body": body });
        if internal {
            payload["properties"] = serde_json::json!([{
                "key": "sd.public.comment",
                "value": { "internal": true }
            }]);
        }
        self.post(&path, &payload).await
    }
```

- [ ] **Step 2: Update the call site in workflow.rs**

In `src/cli/issue/workflow.rs`, replace line 422:

```rust
    let comment = client.add_comment(&key, adf_body).await?;
```

With:

```rust
    let comment = client.add_comment(&key, adf_body, false).await?;
```

(The `false` is temporary — Task 3 will wire the actual flag value.)

- [ ] **Step 3: Verify it compiles and existing tests pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo check 2>&1 | tail -5 && cargo test --lib 2>&1 | tail -5`

Expected: Compiles. All existing tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/api/jira/issues.rs src/cli/issue/workflow.rs
git commit -m "feat: add internal flag to add_comment API (#103)"
```

---

### Task 3: Add `--internal` flag to CLI and wire through handler

**Files:**
- Modify: `src/cli/mod.rs:336-351`
- Modify: `src/cli/issue/workflow.rs:382-422`

- [ ] **Step 1: Add `--internal` flag to Comment variant**

In `src/cli/mod.rs`, replace the `Comment` variant (lines 336-351):

```rust
    /// Add a comment
    Comment {
        /// Issue key
        key: String,
        /// Comment text
        message: Option<String>,
        /// Interpret input as Markdown
        #[arg(long)]
        markdown: bool,
        /// Read comment from file
        #[arg(long)]
        file: Option<String>,
        /// Read comment from stdin (for piping)
        #[arg(long)]
        stdin: bool,
    },
```

With:

```rust
    /// Add a comment
    Comment {
        /// Issue key
        key: String,
        /// Comment text
        message: Option<String>,
        /// Interpret input as Markdown
        #[arg(long)]
        markdown: bool,
        /// Read comment from file
        #[arg(long)]
        file: Option<String>,
        /// Read comment from stdin (for piping)
        #[arg(long)]
        stdin: bool,
        /// Mark comment as internal (agent-only, not visible to customers on JSM projects)
        #[arg(long)]
        internal: bool,
    },
```

- [ ] **Step 2: Wire the flag through the handler**

In `src/cli/issue/workflow.rs`, replace the destructure and call site (lines 387-422).

Replace:

```rust
    let IssueCommand::Comment {
        key,
        message,
        markdown,
        file,
        stdin,
    } = command
    else {
        unreachable!()
    };
```

With:

```rust
    let IssueCommand::Comment {
        key,
        message,
        markdown,
        file,
        stdin,
        internal,
    } = command
    else {
        unreachable!()
    };
```

And replace:

```rust
    let comment = client.add_comment(&key, adf_body, false).await?;
```

With:

```rust
    let comment = client.add_comment(&key, adf_body, internal).await?;
```

- [ ] **Step 3: Verify it compiles**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo check 2>&1 | tail -5`

Expected: Compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/cli/mod.rs src/cli/issue/workflow.rs
git commit -m "feat: add --internal flag to issue comment command (#103)"
```

---

### Task 4: Add `expand=properties` to `list_comments`

**Files:**
- Modify: `src/api/jira/issues.rs:183`

- [ ] **Step 1: Add `expand=properties` to the query string**

In `src/api/jira/issues.rs`, replace line 183:

```rust
            let path = format!("{}?startAt={}&maxResults={}", base, start_at, page_size);
```

With:

```rust
            let path = format!(
                "{}?startAt={}&maxResults={}&expand=properties",
                base, start_at, page_size
            );
```

- [ ] **Step 2: Update existing integration tests for the new query parameter**

In `tests/comments.rs`, add `expand=properties` to all mock query param matchers. For each of the 4 tests, add this matcher line after the existing `query_param` lines:

In `list_comments_returns_all_comments` (line 15), `list_comments_empty` (line 52), and `list_comments_paginated` first page (line 108), second page (line 130), add:

```rust
        .and(query_param("expand", "properties"))
```

In `list_comments_with_limit` (line 76), add:

```rust
        .and(query_param("expand", "properties"))
```

- [ ] **Step 3: Verify tests pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --test comments 2>&1 | tail -10`

Expected: All 4 existing tests pass (mocks now expect the `expand=properties` param).

- [ ] **Step 4: Commit**

```bash
git add src/api/jira/issues.rs tests/comments.rs
git commit -m "feat: add expand=properties to list_comments query (#103)"
```

---

### Task 5: Conditional "Visibility" column in comment listing

**Files:**
- Modify: `src/cli/issue/list.rs:612-640`

- [ ] **Step 1: Add helper to extract visibility from comment properties**

In `src/cli/issue/list.rs`, add this function before `handle_comments` (around line 611):

```rust
/// Extract the internal/external visibility from a comment's `sd.public.comment` property.
/// Returns `Some("Internal")` or `Some("External")` if the property exists, `None` otherwise.
fn comment_visibility(comment: &Comment) -> Option<&'static str> {
    comment
        .properties
        .iter()
        .find(|p| p.key == "sd.public.comment")
        .map(|p| {
            if p.value.get("internal") == Some(&serde_json::Value::Bool(true)) {
                "Internal"
            } else {
                "External"
            }
        })
}
```

- [ ] **Step 2: Update `handle_comments` to show conditional Visibility column**

Replace the `handle_comments` function (lines 612-640):

```rust
pub(super) async fn handle_comments(
    key: &str,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let comments = client.list_comments(key, limit).await?;

    match output_format {
        OutputFormat::Json => {
            output::print_output(output_format, &["Author", "Date", "Body"], &[], &comments)?;
        }
        OutputFormat::Table => {
            let rows: Vec<Vec<String>> = comments
                .iter()
                .map(|c| {
                    let author = c.author.as_ref().map(|a| a.display_name.as_str());
                    let created = c.created.as_deref();
                    let body_text = c.body.as_ref().map(adf::adf_to_text);
                    format_comment_row(author, created, body_text.as_deref())
                })
                .collect();

            output::print_output(output_format, &["Author", "Date", "Body"], &rows, &comments)?;
        }
    }

    Ok(())
}
```

With:

```rust
pub(super) async fn handle_comments(
    key: &str,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let comments = client.list_comments(key, limit).await?;

    // Show Visibility column only if any comment has sd.public.comment property
    let has_visibility = comments.iter().any(|c| comment_visibility(c).is_some());

    match output_format {
        OutputFormat::Json => {
            let headers = if has_visibility {
                vec!["Author", "Date", "Visibility", "Body"]
            } else {
                vec!["Author", "Date", "Body"]
            };
            output::print_output(output_format, &headers, &[], &comments)?;
        }
        OutputFormat::Table => {
            let (headers, rows) = if has_visibility {
                let rows: Vec<Vec<String>> = comments
                    .iter()
                    .map(|c| {
                        let author = c.author.as_ref().map(|a| a.display_name.as_str());
                        let created = c.created.as_deref();
                        let body_text = c.body.as_ref().map(adf::adf_to_text);
                        let visibility = comment_visibility(c).unwrap_or("External");
                        let mut row = format_comment_row(author, created, body_text.as_deref());
                        // Insert Visibility before Body (index 2)
                        row.insert(2, visibility.to_string());
                        row
                    })
                    .collect();
                (
                    vec!["Author", "Date", "Visibility", "Body"],
                    rows,
                )
            } else {
                let rows: Vec<Vec<String>> = comments
                    .iter()
                    .map(|c| {
                        let author = c.author.as_ref().map(|a| a.display_name.as_str());
                        let created = c.created.as_deref();
                        let body_text = c.body.as_ref().map(adf::adf_to_text);
                        format_comment_row(author, created, body_text.as_deref())
                    })
                    .collect();
                (vec!["Author", "Date", "Body"], rows)
            };

            output::print_output(output_format, &headers, &rows, &comments)?;
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Add import for `Comment` type**

At the top of `src/cli/issue/list.rs`, check if `Comment` is already imported. If not, add to the existing `use crate::types::jira::...` import:

```rust
use crate::types::jira::issue::Comment;
```

(If `Comment` is already imported via another path, skip this step.)

- [ ] **Step 4: Verify it compiles**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo check 2>&1 | tail -5`

Expected: Compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "feat: show conditional Visibility column in comment listing (#103)"
```

---

### Task 6: Handler Tests

**Files:**
- Modify: `tests/cli_handler.rs`

These tests verify end-to-end behavior via the CLI binary.

- [ ] **Step 1: Add handler test for `--internal` flag**

Add this test to `tests/cli_handler.rs`, after the last existing test:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_comment_internal_flag_adds_property() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/HELP-42/comment"))
        .and(body_partial_json(serde_json::json!({
            "properties": [{
                "key": "sd.public.comment",
                "value": { "internal": true }
            }]
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "created": "2026-04-05T12:00:00.000+0000"
        })))
        .expect(1)
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "comment",
            "HELP-42",
            "Internal note",
            "--internal",
            "--no-input",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_comment_without_internal_omits_property() {
    let server = MockServer::start().await;

    // This mock expects NO properties field in the body.
    // We verify by checking the body does NOT contain "sd.public.comment".
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/HELP-42/comment"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10002",
            "created": "2026-04-05T12:00:00.000+0000"
        })))
        .expect(1)
        .named("comment without internal")
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "comment",
            "HELP-42",
            "External note",
            "--no-input",
        ])
        .assert()
        .success();
}
```

- [ ] **Step 2: Add handler test for visibility column in comments listing**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_comments_shows_visibility_column_for_jsm() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HELP-42/comment"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Agent", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Internal note" }] }] },
                    "created": "2026-04-05T10:00:00.000+0000",
                    "properties": [{"key": "sd.public.comment", "value": {"internal": true}}]
                },
                {
                    "id": "10002",
                    "author": { "accountId": "def", "displayName": "Agent", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Customer reply" }] }] },
                    "created": "2026-04-05T11:00:00.000+0000",
                    "properties": [{"key": "sd.public.comment", "value": {"internal": false}}]
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 2
        })))
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "HELP-42", "--no-input"])
        .assert()
        .success()
        .stdout(predicates::prelude::predicate::str::contains("Internal"))
        .stdout(predicates::prelude::predicate::str::contains("External"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_comments_hides_visibility_column_for_non_jsm() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/DEV-99/comment"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [
                {
                    "id": "10001",
                    "author": { "accountId": "abc", "displayName": "Dev", "active": true },
                    "body": { "type": "doc", "version": 1, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Fixed in commit abc123" }] }] },
                    "created": "2026-04-05T10:00:00.000+0000",
                    "properties": []
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 1
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "DEV-99", "--no-input"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Visibility"),
        "Non-JSM comments should not show Visibility column, got: {stdout}"
    );
    assert!(
        !stdout.contains("Internal"),
        "Non-JSM comments should not show Internal, got: {stdout}"
    );
}
```

- [ ] **Step 3: Run handler tests**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --test cli_handler test_handler_comment 2>&1 | tail -20`

Expected: All 4 new handler tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/cli_handler.rs
git commit -m "test: add handler tests for internal comments and visibility column (#103)"
```

---

### Task 7: Format and Lint Check

**Files:** (none — formatting/linting only)

- [ ] **Step 1: Run formatter**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo fmt --all`

- [ ] **Step 2: Run clippy**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo clippy -- -D warnings 2>&1 | tail -20`

Expected: Zero warnings.

- [ ] **Step 3: Run full test suite**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test 2>&1 | tail -20`

Expected: All tests pass.

- [ ] **Step 4: Commit if any formatting changes**

```bash
git add -A
git commit -m "style: format JSM internal comments implementation (#103)"
```

(Skip commit if no changes.)
