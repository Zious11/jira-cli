# Date Filter Flags Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--created-after`, `--created-before`, `--updated-after`, `--updated-before` date filter flags to `jr issue list`.

**Architecture:** Four new clap args on `IssueCommand::List` that generate JQL date clauses via `build_filter_clauses`. Date validation uses chrono in `jql.rs`. The `--before` flags add +1 day and use `<` to handle JQL's midnight semantics correctly.

**Tech Stack:** Rust, clap (derive), chrono::NaiveDate, wiremock (tests), assert_cmd/predicates (smoke tests)

---

## File Structure

| File | Responsibility | Change type |
|------|---------------|-------------|
| `src/jql.rs` | Date validation (`validate_date`) | Add function + unit tests |
| `src/cli/mod.rs` | CLI arg definitions for `IssueCommand::List` | Add 4 args |
| `src/cli/issue/list.rs` | Early validation, JQL clause generation | Modify `handle_list` + `build_filter_clauses` |
| `tests/cli_smoke.rs` | Clap conflict smoke tests | Add 1 test |
| `tests/cli_handler.rs` | Handler-level tests with wiremock | Add 2 tests |

---

### Task 1: Date Validation in `jql.rs`

**Files:**
- Modify: `src/jql.rs`

- [ ] **Step 1: Write failing unit tests for `validate_date`**

Add these tests to the existing `#[cfg(test)] mod tests` block at the bottom of `src/jql.rs` (after the last existing test, before the closing `}`):

```rust
    #[test]
    fn validate_date_valid_simple() {
        let d = validate_date("2026-03-18").unwrap();
        assert_eq!(d.to_string(), "2026-03-18");
    }

    #[test]
    fn validate_date_valid_leap_day() {
        let d = validate_date("2024-02-29").unwrap();
        assert_eq!(d.to_string(), "2024-02-29");
    }

    #[test]
    fn validate_date_invalid_format_slash() {
        let err = validate_date("2026/03/18").unwrap_err();
        assert!(err.contains("Invalid date"));
        assert!(err.contains("YYYY-MM-DD"));
    }

    #[test]
    fn validate_date_invalid_format_us() {
        let err = validate_date("03-18-2026").unwrap_err();
        assert!(err.contains("Invalid date"));
    }

    #[test]
    fn validate_date_impossible_feb30() {
        let err = validate_date("2026-02-30").unwrap_err();
        assert!(err.contains("Invalid date"));
    }

    #[test]
    fn validate_date_impossible_month13() {
        let err = validate_date("2026-13-01").unwrap_err();
        assert!(err.contains("Invalid date"));
    }

    #[test]
    fn validate_date_empty() {
        let err = validate_date("").unwrap_err();
        assert!(err.contains("Invalid date"));
    }

    #[test]
    fn validate_date_non_leap_feb29() {
        let err = validate_date("2026-02-29").unwrap_err();
        assert!(err.contains("Invalid date"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --lib jql::tests::validate_date 2>&1 | tail -20`

Expected: FAIL — `validate_date` does not exist yet.

- [ ] **Step 3: Implement `validate_date`**

Add this function to `src/jql.rs`, after the existing `validate_asset_key` function (before the `/// Strip ORDER BY` doc comment around line 84):

```rust
/// Validate and parse an absolute date string in ISO 8601 format (YYYY-MM-DD).
///
/// Returns the parsed `NaiveDate` on success. The caller needs the parsed date
/// to compute +1 day for `--before` flag JQL generation.
pub fn validate_date(s: &str) -> Result<chrono::NaiveDate, String> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
        format!("Invalid date \"{s}\". Expected format: YYYY-MM-DD (e.g., 2026-03-18).")
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --lib jql::tests::validate_date 2>&1 | tail -20`

Expected: All 8 new tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/jql.rs
git commit -m "feat: add validate_date function for date filter flags (#113)"
```

---

### Task 2: Add CLI Flags to `mod.rs`

**Files:**
- Modify: `src/cli/mod.rs:173-212` (the `IssueCommand::List` variant)

- [ ] **Step 1: Add the four date filter args**

In `src/cli/mod.rs`, inside the `List` variant of `IssueCommand`, add these four fields after the `asset` field (before the closing `}` of the `List` variant, around line 211):

```rust
        /// Show issues created on or after this date (YYYY-MM-DD)
        #[arg(long, conflicts_with = "recent")]
        created_after: Option<String>,
        /// Show issues created on or before this date (YYYY-MM-DD)
        #[arg(long)]
        created_before: Option<String>,
        /// Show issues updated on or after this date (YYYY-MM-DD)
        #[arg(long)]
        updated_after: Option<String>,
        /// Show issues updated on or before this date (YYYY-MM-DD)
        #[arg(long)]
        updated_before: Option<String>,
```

- [ ] **Step 2: Verify it compiles**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo check 2>&1 | tail -20`

Expected: Compiler error about exhaustive pattern match in `list.rs` — the `IssueCommand::List` destructuring doesn't include the new fields yet. This confirms the fields were added.

- [ ] **Step 3: Commit**

```bash
git add src/cli/mod.rs
git commit -m "feat: add date filter CLI flags to issue list (#113)"
```

---

### Task 3: Wire Date Flags into `list.rs`

**Files:**
- Modify: `src/cli/issue/list.rs:57-96` (destructuring + early validation)
- Modify: `src/cli/issue/list.rs:209-218` (`build_filter_clauses` call)
- Modify: `src/cli/issue/list.rs:291-305` (unbounded query error message)
- Modify: `src/cli/issue/list.rs:491-523` (`build_filter_clauses` function)

- [ ] **Step 1: Update the `IssueCommand::List` destructuring**

In `src/cli/issue/list.rs`, in the `handle_list` function, update the destructuring (around line 65-81) to include the new fields. Add after `asset: asset_key,`:

```rust
        created_after,
        created_before,
        updated_after,
        updated_before,
```

- [ ] **Step 2: Add early date validation**

In `src/cli/issue/list.rs`, after the `--asset` validation block (after line 96), add validation for all four date flags:

```rust
    // Validate date filter flags early
    let created_after_date = if let Some(ref d) = created_after {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
    let created_before_date = if let Some(ref d) = created_before {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
    let updated_after_date = if let Some(ref d) = updated_after {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
    let updated_before_date = if let Some(ref d) = updated_before {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
```

- [ ] **Step 3: Build date JQL clauses**

After the date validation block (Step 2), compute the JQL clause strings. The `--after` flags use `>=` directly; the `--before` flags add +1 day and use `<`:

```rust
    // Build date filter JQL clauses
    let created_after_clause = created_after_date.map(|d| format!("created >= \"{}\"", d));
    let created_before_clause = created_before_date.map(|d| {
        let next_day = d + chrono::Days::new(1);
        format!("created < \"{}\"", next_day)
    });
    let updated_after_clause = updated_after_date.map(|d| format!("updated >= \"{}\"", d));
    let updated_before_clause = updated_before_date.map(|d| {
        let next_day = d + chrono::Days::new(1);
        format!("updated < \"{}\"", next_day)
    });
```

- [ ] **Step 4: Update `build_filter_clauses` signature and call**

Update the `build_filter_clauses` function signature in `src/cli/issue/list.rs` (around line 491) to accept the four new clauses:

```rust
fn build_filter_clauses(
    assignee_jql: Option<&str>,
    reporter_jql: Option<&str>,
    status: Option<&str>,
    team_clause: Option<&str>,
    recent: Option<&str>,
    open: bool,
    asset_clause: Option<&str>,
    created_after_clause: Option<&str>,
    created_before_clause: Option<&str>,
    updated_after_clause: Option<&str>,
    updated_before_clause: Option<&str>,
) -> Vec<String> {
```

Add these lines at the end of the function body, before the `parts` return (after the `asset_clause` block):

```rust
    if let Some(c) = created_after_clause {
        parts.push(c.to_string());
    }
    if let Some(c) = created_before_clause {
        parts.push(c.to_string());
    }
    if let Some(c) = updated_after_clause {
        parts.push(c.to_string());
    }
    if let Some(c) = updated_before_clause {
        parts.push(c.to_string());
    }
```

Update the call site (around line 210) to pass the new clauses:

```rust
    let filter_parts = build_filter_clauses(
        assignee_jql.as_deref(),
        reporter_jql.as_deref(),
        resolved_status.as_deref(),
        team_clause.as_deref(),
        recent.as_deref(),
        open,
        asset_clause.as_deref(),
        created_after_clause.as_deref(),
        created_before_clause.as_deref(),
        updated_after_clause.as_deref(),
        updated_before_clause.as_deref(),
    );
```

- [ ] **Step 5: Update the unbounded query error message**

In the guard against unbounded query (around line 298-305), update the error message to mention the new flags:

```rust
    if all_parts.is_empty() {
        return Err(JrError::UserError(
            "No project or filters specified. Use --project, --assignee, --reporter, --status, --open, --team, --recent, --created-after, --created-before, --updated-after, --updated-before, --asset, or --jql. \
             You can also set a default project in .jr.toml or run \"jr init\"."
                .into(),
        )
        .into());
    }
```

- [ ] **Step 6: Verify it compiles and existing tests pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo check 2>&1 | tail -10 && cargo test --lib 2>&1 | tail -10`

Expected: Compiles. All existing tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "feat: wire date filter flags into JQL generation (#113)"
```

---

### Task 4: Smoke Test for `--created-after` / `--recent` Conflict

**Files:**
- Modify: `tests/cli_smoke.rs`

- [ ] **Step 1: Write the conflict smoke test**

Add this test to `tests/cli_smoke.rs`, after the last existing test:

```rust
#[test]
fn test_issue_list_created_after_and_recent_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "list",
            "--created-after",
            "2026-03-18",
            "--recent",
            "7d",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --test cli_smoke test_issue_list_created_after_and_recent_conflict 2>&1 | tail -10`

Expected: PASS — clap enforces the conflict declared in Task 2.

- [ ] **Step 3: Commit**

```bash
git add tests/cli_smoke.rs
git commit -m "test: add smoke test for --created-after/--recent conflict (#113)"
```

---

### Task 5: Handler Tests for Date Flags

**Files:**
- Modify: `tests/cli_handler.rs`

These tests verify that the date flags produce correct JQL when the handler runs against a wiremock server.

- [ ] **Step 1: Write handler test for `--created-after`**

Add this test to `tests/cli_handler.rs`, after the last existing test:

```rust
#[tokio::test]
async fn test_handler_list_created_after() {
    let server = MockServer::start().await;

    // The search endpoint should receive JQL with the date clause
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "project = \"PROJ\" AND created >= \"2026-03-18\" ORDER BY updated DESC"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1",
                "Test issue",
                "To Do",
            )]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--created-after",
            "2026-03-18",
            "--no-input",
        ])
        .assert()
        .success();
}
```

- [ ] **Step 2: Write handler test for `--created-before` (verifies +1 day)**

```rust
#[tokio::test]
async fn test_handler_list_created_before() {
    let server = MockServer::start().await;

    // --created-before 2026-03-18 should produce created < "2026-03-19" (next day)
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "project = \"PROJ\" AND created < \"2026-03-19\" ORDER BY updated DESC"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1",
                "Test issue",
                "To Do",
            )]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--created-before",
            "2026-03-18",
            "--no-input",
        ])
        .assert()
        .success();
}
```

- [ ] **Step 3: Run handler tests**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --test cli_handler test_handler_list_created 2>&1 | tail -20`

Expected: Both tests PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/cli_handler.rs
git commit -m "test: add handler tests for date filter flags (#113)"
```

---

### Task 6: Format and Lint Check

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
git commit -m "style: format date filter implementation (#113)"
```

(Skip commit if no changes.)
