# Issue Edit Description Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--description`, `--description-stdin`, and `--markdown` flags to `jr issue edit` so users can update issue descriptions without leaving the CLI.

**Architecture:** Three new fields on the `Edit` clap variant, description-to-ADF conversion in `handle_edit` (same pattern as `handle_create`), integration tests verifying the PUT body sent to Jira.

**Tech Stack:** Rust, clap (derive), serde_json, wiremock, assert_cmd

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/cli/mod.rs` | Modify (lines 230-257) | Add 3 fields to `IssueCommand::Edit` |
| `src/cli/issue/create.rs` | Modify (lines 140-265) | Add description handling to `handle_edit` |
| `tests/issue_commands.rs` | Modify (append) | Integration tests for edit + description |

---

### Task 1: Add CLI flags to `IssueCommand::Edit`

**Files:**
- Modify: `src/cli/mod.rs:230-257`

- [ ] **Step 1: Add the three new fields to the `Edit` variant**

In `src/cli/mod.rs`, add `description`, `description_stdin`, and `markdown` fields to `IssueCommand::Edit`, after the existing `parent` field:

```rust
    /// Edit issue fields
    Edit {
        /// Issue key
        key: String,
        /// New summary
        #[arg(long)]
        summary: Option<String>,
        /// New issue type
        #[arg(long = "type")]
        issue_type: Option<String>,
        /// New priority
        #[arg(long)]
        priority: Option<String>,
        /// Add or remove labels (e.g., --label add:backend --label remove:frontend)
        #[arg(long)]
        label: Vec<String>,
        /// Team assignment
        #[arg(long)]
        team: Option<String>,
        /// Story points
        #[arg(long, conflicts_with = "no_points")]
        points: Option<f64>,
        /// Clear story points
        #[arg(long, conflicts_with = "points")]
        no_points: bool,
        /// Parent issue key
        #[arg(long)]
        parent: Option<String>,
        /// Description
        #[arg(short, long, conflicts_with = "description_stdin")]
        description: Option<String>,
        /// Read description from stdin (for piping)
        #[arg(long, conflicts_with = "description")]
        description_stdin: bool,
        /// Interpret description as Markdown
        #[arg(long)]
        markdown: bool,
    },
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -30`

Expected: Compilation error in `create.rs` because `handle_edit` destructuring doesn't include the new fields yet. This is expected — we'll fix it in Task 2.

- [ ] **Step 3: Commit**

```bash
git add src/cli/mod.rs
git commit -m "feat: add --description, --description-stdin, --markdown flags to issue edit CLI (#82)"
```

---

### Task 2: Wire description handling into `handle_edit`

**Files:**
- Modify: `src/cli/issue/create.rs:140-265`

- [ ] **Step 1: Update the destructuring in `handle_edit` to include the new fields**

In `src/cli/issue/create.rs`, change the `let IssueCommand::Edit { ... }` destructuring (around line 147) to include the three new fields:

```rust
    let IssueCommand::Edit {
        key,
        summary,
        issue_type,
        priority,
        label: labels,
        team,
        points,
        no_points,
        parent,
        description,
        description_stdin,
        markdown,
    } = command
    else {
        unreachable!()
    };
```

- [ ] **Step 2: Add description resolution and ADF conversion**

Insert the following block after `let mut has_updates = false;` (line 163) and before the `if let Some(ref s) = summary {` block (line 165):

```rust
    // Resolve description
    let desc_text = if description_stdin {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
        Some(buf)
    } else {
        description
    };

    if let Some(ref text) = desc_text {
        let adf_body = if markdown {
            adf::markdown_to_adf(text)
        } else {
            adf::text_to_adf(text)
        };
        fields["description"] = adf_body;
        has_updates = true;
    }
```

- [ ] **Step 3: Update the "no fields specified" error message**

Change the bail message (around line 246) from:

```rust
        bail!(
            "No fields specified to update. Use --summary, --type, --priority, --label, --team, --points, --no-points, or --parent."
        );
```

To:

```rust
        bail!(
            "No fields specified to update. Use --summary, --type, --priority, --label, --team, --points, --no-points, --parent, --description, or --description-stdin."
        );
```

- [ ] **Step 4: Verify it compiles and existing tests pass**

Run: `cargo build && cargo test --lib 2>&1 | tail -5`

Expected: Build succeeds, all existing tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue/create.rs
git commit -m "feat: wire description handling into issue edit handler (#82)"
```

---

### Task 3: Integration test — edit with plain text description

**Files:**
- Modify: `tests/issue_commands.rs` (append)

- [ ] **Step 1: Write the integration test**

Append to `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_edit_issue_with_description() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-10"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "description": {
                    "version": 1,
                    "type": "doc",
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [
                                { "type": "text", "text": "Updated description" }
                            ]
                        }
                    ]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client
        .edit_issue(
            "FOO-10",
            serde_json::json!({
                "description": jr::adf::text_to_adf("Updated description")
            }),
        )
        .await
        .unwrap();
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --test issue_commands test_edit_issue_with_description -- --exact 2>&1 | tail -5`

Expected: PASS — this validates that `edit_issue` sends the correct PUT body with ADF description.

- [ ] **Step 3: Commit**

```bash
git add tests/issue_commands.rs
git commit -m "test: add integration test for edit issue with description (#82)"
```

---

### Task 4: Integration test — edit with markdown description

**Files:**
- Modify: `tests/issue_commands.rs` (append)

- [ ] **Step 1: Write the integration test**

Append to `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_edit_issue_with_markdown_description() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-11"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "description": {
                    "version": 1,
                    "type": "doc",
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "bold text",
                                    "marks": [{"type": "strong"}]
                                }
                            ]
                        }
                    ]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client
        .edit_issue(
            "FOO-11",
            serde_json::json!({
                "description": jr::adf::markdown_to_adf("**bold text**")
            }),
        )
        .await
        .unwrap();
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --test issue_commands test_edit_issue_with_markdown_description -- --exact 2>&1 | tail -5`

Expected: PASS — validates that markdown-to-ADF produces the expected bold markup in the PUT body.

- [ ] **Step 3: Commit**

```bash
git add tests/issue_commands.rs
git commit -m "test: add integration test for edit issue with markdown description (#82)"
```

---

### Task 5: Integration test — edit with description combined with other fields

**Files:**
- Modify: `tests/issue_commands.rs` (append)

- [ ] **Step 1: Write the integration test**

Append to `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_edit_issue_description_with_other_fields() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-12"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "summary": "New summary",
                "description": {
                    "version": 1,
                    "type": "doc",
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [
                                { "type": "text", "text": "New description" }
                            ]
                        }
                    ]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client
        .edit_issue(
            "FOO-12",
            serde_json::json!({
                "summary": "New summary",
                "description": jr::adf::text_to_adf("New description")
            }),
        )
        .await
        .unwrap();
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --test issue_commands test_edit_issue_description_with_other_fields -- --exact 2>&1 | tail -5`

Expected: PASS — validates that description and summary can coexist in the same PUT body.

- [ ] **Step 3: Commit**

```bash
git add tests/issue_commands.rs
git commit -m "test: add integration test for edit issue description with other fields (#82)"
```

---

### Task 6: CLI-level test — clap rejects conflicting flags

**Files:**
- Modify: `tests/cli_smoke.rs` (append)

- [ ] **Step 1: Write the CLI-level test for conflicting flags**

Append to `tests/cli_smoke.rs`:

```rust
#[test]
fn test_edit_description_and_description_stdin_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "edit", "FOO-1", "--description", "text", "--description-stdin"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --test cli_smoke test_edit_description_and_description_stdin_conflict -- --exact 2>&1 | tail -5`

Expected: PASS — clap rejects the conflicting flags at parse time.

- [ ] **Step 3: Commit**

```bash
git add tests/cli_smoke.rs
git commit -m "test: add CLI test for --description and --description-stdin conflict (#82)"
```

---

### Task 7: Final verification

- [ ] **Step 1: Run all tests**

Run: `cargo test 2>&1 | tail -10`

Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings 2>&1 | tail -10`

Expected: Zero warnings.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all -- --check 2>&1 | tail -5`

Expected: No formatting issues.
