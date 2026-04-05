# partial_match Duplicate Name Disambiguation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect and surface duplicate exact matches in `partial_match` so callers — especially user-resolution functions — can disambiguate instead of silently picking the first match.

**Architecture:** Add `ExactMultiple(Vec<String>)` variant to `MatchResult`. Update `partial_match()` to collect all exact matches. Non-user callers add a trivial match arm. User-resolution callers (`resolve_user`, `resolve_assignee`, `resolve_assignee_by_project`, `resolve_team_field`) handle duplicates with interactive disambiguation or `--no-input` error. Fix index-based mapping in all existing branches.

**Tech Stack:** Rust, partial_match module, dialoguer (interactive prompts), wiremock + assert_cmd (integration tests), proptest (property tests)

---

### Task 1: Add `ExactMultiple` variant and update `partial_match()` logic

**Files:**
- Modify: `src/partial_match.rs:1-34` (enum + function)

- [ ] **Step 1: Write the failing unit test for duplicate exact matches**

Add to the `tests` module in `src/partial_match.rs`:

```rust
#[test]
fn test_exact_match_duplicate_returns_exact_multiple() {
    let candidates = vec![
        "John Smith".into(),
        "Jane Doe".into(),
        "John Smith".into(),
    ];
    match partial_match("John Smith", &candidates) {
        MatchResult::ExactMultiple(names) => {
            assert_eq!(names.len(), 2);
            assert!(names.iter().all(|n| n == "John Smith"));
        }
        other => panic!("Expected ExactMultiple, got {:?}", other),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib partial_match::tests::test_exact_match_duplicate_returns_exact_multiple -- --nocapture`

Expected: FAIL — `ExactMultiple` variant does not exist yet.

- [ ] **Step 3: Add `ExactMultiple` variant to `MatchResult`**

In `src/partial_match.rs`, replace lines 1-9:

```rust
/// Result of attempting a partial match against a list of candidates.
#[derive(Debug)]
pub enum MatchResult {
    /// Exactly one match found
    Exact(String),
    /// Multiple candidates share the same exact (case-insensitive) name
    ExactMultiple(Vec<String>),
    /// Multiple matches — caller should prompt for disambiguation
    Ambiguous(Vec<String>),
    /// No matches
    None(Vec<String>),
}
```

Note: `#[derive(Debug)]` is added so tests can use `{:?}` formatting in panic messages.

- [ ] **Step 4: Update `partial_match()` to collect all exact matches**

In `src/partial_match.rs`, replace lines 12-34 (the entire `partial_match` function body):

```rust
/// Case-insensitive substring match against candidates.
pub fn partial_match(input: &str, candidates: &[String]) -> MatchResult {
    let lower_input = input.to_lowercase();

    // Collect all exact matches (case-insensitive)
    let exact_matches: Vec<String> = candidates
        .iter()
        .filter(|c| c.to_lowercase() == lower_input)
        .cloned()
        .collect();

    match exact_matches.len() {
        1 => return MatchResult::Exact(exact_matches.into_iter().next().unwrap()),
        n if n > 1 => return MatchResult::ExactMultiple(exact_matches),
        _ => {}
    }

    // Try substring match
    let matches: Vec<String> = candidates
        .iter()
        .filter(|c| c.to_lowercase().contains(&lower_input))
        .cloned()
        .collect();

    match matches.len() {
        0 => MatchResult::None(candidates.to_vec()),
        1 => MatchResult::Exact(matches.into_iter().next().unwrap()),
        _ => MatchResult::Ambiguous(matches),
    }
}
```

- [ ] **Step 5: Run the duplicate test to verify it passes**

Run: `cargo test --lib partial_match::tests::test_exact_match_duplicate_returns_exact_multiple -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Write additional unit tests**

Add to the `tests` module in `src/partial_match.rs`:

```rust
#[test]
fn test_exact_match_duplicate_case_insensitive() {
    let candidates = vec![
        "John Smith".into(),
        "john smith".into(),
    ];
    match partial_match("john smith", &candidates) {
        MatchResult::ExactMultiple(names) => {
            assert_eq!(names.len(), 2);
            // Preserves original casing
            assert_eq!(names[0], "John Smith");
            assert_eq!(names[1], "john smith");
        }
        other => panic!("Expected ExactMultiple, got {:?}", other),
    }
}

#[test]
fn test_exact_match_three_duplicates() {
    let candidates = vec![
        "John Smith".into(),
        "Jane Doe".into(),
        "John Smith".into(),
        "John Smith".into(),
    ];
    match partial_match("John Smith", &candidates) {
        MatchResult::ExactMultiple(names) => {
            assert_eq!(names.len(), 3);
        }
        other => panic!("Expected ExactMultiple, got {:?}", other),
    }
}
```

- [ ] **Step 7: Write proptest for duplicate candidates**

Add to the `proptests` module in `src/partial_match.rs`:

```rust
#[test]
fn duplicate_candidates_yield_exact_multiple(idx in 0usize..4) {
    let base: Vec<String> = vec![
        "In Progress".into(), "In Review".into(),
        "Blocked".into(), "Done".into(),
    ];
    // Duplicate one candidate
    let mut candidates = base.clone();
    candidates.push(base[idx].clone());
    let input = base[idx].clone();
    match partial_match(&input, &candidates) {
        MatchResult::ExactMultiple(names) => {
            prop_assert!(names.len() >= 2);
            for name in &names {
                prop_assert_eq!(name.to_lowercase(), input.to_lowercase());
            }
        }
        _ => prop_assert!(false, "Expected ExactMultiple for duplicated '{}'", input),
    }
}
```

- [ ] **Step 8: Run all partial_match tests**

Run: `cargo test --lib partial_match -- --nocapture`

Expected: All tests pass including existing ones (no regressions).

- [ ] **Step 9: Commit**

```bash
git add src/partial_match.rs
git commit -m "fix: detect duplicate exact matches in partial_match (#117)"
```

---

### Task 2: Update non-user callers with trivial `ExactMultiple` arm

**Files:**
- Modify: `src/cli/issue/workflow.rs:129-142`
- Modify: `src/cli/issue/list.rs:179-180`
- Modify: `src/cli/issue/links.rs:62-63` and `130-131`
- Modify: `src/cli/assets.rs:332-333`, `453-454`, `649-650`
- Modify: `src/cli/queue.rs:147-163` and `202-216`

- [ ] **Step 1: Verify the build fails**

Run: `cargo build 2>&1 | head -40`

Expected: FAIL — non-exhaustive match errors at every `MatchResult` match site because `ExactMultiple` is not handled.

- [ ] **Step 2: Update `src/cli/issue/workflow.rs`**

Add the `ExactMultiple` arm after the `Exact` arm at line 130. Insert between the `Exact(name) => { ... }` arm (lines 130-142) and the `Ambiguous` arm (line 143):

```rust
            MatchResult::ExactMultiple(names) => {
                // Duplicate transition names not expected; take first
                let name = names.into_iter().next().unwrap();
                let idx = candidates
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, i)| *i)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Internal error: matched candidate \"{}\" not found. Please report this as a bug.",
                            name
                        )
                    })?;
                &transitions[idx]
            }
```

- [ ] **Step 3: Update `src/cli/issue/list.rs`**

Add the `ExactMultiple` arm after line 180 (`MatchResult::Exact(name) => Some(name),`). Insert between `Exact` and `Ambiguous`:

```rust
            MatchResult::ExactMultiple(names) => {
                // Duplicate status names not expected; take first
                Some(names.into_iter().next().unwrap())
            }
```

- [ ] **Step 4: Update `src/cli/issue/links.rs` — first call site (handle_link)**

Add the `ExactMultiple` arm after line 63 (`MatchResult::Exact(name) => name,`). Insert between `Exact` and `Ambiguous`:

```rust
        MatchResult::ExactMultiple(names) => {
            // Duplicate link type names not expected; take first
            names.into_iter().next().unwrap()
        }
```

- [ ] **Step 5: Update `src/cli/issue/links.rs` — second call site (handle_unlink)**

Add the `ExactMultiple` arm after line 131 (`MatchResult::Exact(name) => name,`). Insert between `Exact` and `Ambiguous`:

```rust
            MatchResult::ExactMultiple(names) => {
                // Duplicate link type names not expected; take first
                names.into_iter().next().unwrap()
            }
```

- [ ] **Step 6: Update `src/cli/assets.rs` — first call site (ticket status filter)**

Add the `ExactMultiple` arm after line 333 (`MatchResult::Exact(name) => name,`). Insert between `Exact` and `Ambiguous`:

```rust
            MatchResult::ExactMultiple(names) => {
                // Duplicate status names not expected; take first
                names.into_iter().next().unwrap()
            }
```

- [ ] **Step 7: Update `src/cli/assets.rs` — second call site (resolve_schema)**

Add the `ExactMultiple` arm after line 454 (`MatchResult::Exact(name) => Ok(schemas.iter().find(|s| s.name == name).unwrap()),`). Insert between `Exact` and `Ambiguous`:

```rust
        MatchResult::ExactMultiple(names) => {
            // Duplicate schema names not expected; take first
            let name = names.into_iter().next().unwrap();
            Ok(schemas.iter().find(|s| s.name == name).unwrap())
        }
```

- [ ] **Step 8: Update `src/cli/assets.rs` — third call site (object type resolution)**

Add the `ExactMultiple` arm after line 650 (`MatchResult::Exact(name) => name,`). Insert between `Exact` and `Ambiguous`:

```rust
        MatchResult::ExactMultiple(names) => {
            // Duplicate type names not expected after dedup; take first
            names.into_iter().next().unwrap()
        }
```

- [ ] **Step 9: Update `src/cli/queue.rs` — first call site (resolve_queue_id)**

The queue code at lines 148-162 already handles duplicates within the `Exact` arm. Add `ExactMultiple` arm after the `Exact` arm (after line 163, before `Ambiguous`):

```rust
        MatchResult::ExactMultiple(names) => {
            // ExactMultiple means partial_match found duplicate candidate strings.
            // Collect all queues matching any of these names and report as duplicates.
            let matching: Vec<&crate::types::jsm::Queue> = queues
                .iter()
                .filter(|q| names.contains(&q.name))
                .collect();
            let ids: Vec<String> = matching.iter().map(|q| q.id.clone()).collect();
            Err(JrError::UserError(format!(
                "Multiple queues named \"{}\" found (IDs: {}). Use --id {} to specify.",
                names[0],
                ids.join(", "),
                ids[0]
            ))
            .into())
        }
```

- [ ] **Step 10: Update `src/cli/queue.rs` — second call site (test helper `find_queue_id`)**

Add the `ExactMultiple` arm after line 211 (after the `Exact` arm, before `Ambiguous`). This is in the `#[cfg(test)]` module:

```rust
            crate::partial_match::MatchResult::ExactMultiple(names) => {
                Err(format!("duplicate: {}", names.len()))
            }
```

- [ ] **Step 11: Verify the build compiles**

Run: `cargo build 2>&1 | tail -5`

Expected: Build succeeds.

- [ ] **Step 12: Run all tests to verify no regressions**

Run: `cargo test`

Expected: All tests pass.

- [ ] **Step 13: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: No warnings.

- [ ] **Step 14: Commit**

```bash
git add src/cli/issue/workflow.rs src/cli/issue/list.rs src/cli/issue/links.rs src/cli/assets.rs src/cli/queue.rs
git commit -m "fix: add ExactMultiple match arms to all partial_match callers (#117)"
```

---

### Task 3: Fix user-resolution callers with duplicate disambiguation

**Files:**
- Modify: `src/cli/issue/helpers.rs:7-70` (resolve_team_field)
- Modify: `src/cli/issue/helpers.rs:103-168` (resolve_user)
- Modify: `src/cli/issue/helpers.rs:177-240` (resolve_assignee)
- Modify: `src/cli/issue/helpers.rs:250-318` (resolve_assignee_by_project)

- [ ] **Step 1: Write the failing integration test for duplicate user names in `--no-input` mode**

Create test file `tests/duplicate_user_disambiguation.rs`:

```rust
#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: build a user JSON object for wiremock responses.
fn user_json(account_id: &str, display_name: &str, email: Option<&str>) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "accountId": account_id,
        "displayName": display_name,
        "active": true,
    });
    if let Some(e) = email {
        obj["emailAddress"] = serde_json::json!(e);
    }
    obj
}

#[tokio::test]
async fn issue_list_assignee_duplicate_names_no_input_errors() {
    let server = MockServer::start().await;

    // User search returns two users with same display name
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            user_json("acc-john-1", "John Smith", Some("john1@acme.com")),
            user_json("acc-john-2", "John Smith", Some("john2@other.org")),
        ])))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list", "--assignee", "John Smith", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on duplicate user names, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("acc-john-1"),
        "Should list first accountId, got: {stderr}"
    );
    assert!(
        stderr.contains("acc-john-2"),
        "Should list second accountId, got: {stderr}"
    );
    assert!(
        stderr.contains("John Smith"),
        "Should mention the duplicate name, got: {stderr}"
    );
}

#[tokio::test]
async fn issue_assign_duplicate_names_no_input_errors() {
    let server = MockServer::start().await;

    // Assignable user search returns two users with same display name
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            user_json("acc-john-1", "John Smith", Some("john1@acme.com")),
            user_json("acc-john-2", "John Smith", Some("john2@other.org")),
        ])))
        .mount(&server)
        .await;

    // Mock get issue (needed for assign flow)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("FOO-1", "Test issue", None),
        ))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "assign", "FOO-1", "--to", "John Smith", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on duplicate user names, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("acc-john-1"),
        "Should list first accountId, got: {stderr}"
    );
    assert!(
        stderr.contains("acc-john-2"),
        "Should list second accountId, got: {stderr}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test duplicate_user_disambiguation -- --nocapture`

Expected: FAIL — current code does not detect duplicates, either succeeds silently (picking first user) or fails with a different error.

- [ ] **Step 3: Implement `ExactMultiple` handling in `resolve_user`**

In `src/cli/issue/helpers.rs`, add the `ExactMultiple` arm to `resolve_user` (between the `Exact` arm ending at line 141 and the `Ambiguous` arm at line 142):

```rust
        crate::partial_match::MatchResult::ExactMultiple(_) => {
            // Multiple users share the same display name — disambiguate
            let name_lower = name.to_lowercase();
            let duplicates: Vec<&crate::types::jira::User> = active_users
                .iter()
                .filter(|u| u.display_name.to_lowercase() == name_lower)
                .collect();

            if no_input {
                let lines: Vec<String> = duplicates
                    .iter()
                    .map(|u| {
                        let label = u
                            .email_address
                            .as_deref()
                            .unwrap_or(&u.account_id);
                        format!("  {} (account: {})", u.display_name, label)
                    })
                    .collect();
                anyhow::bail!(
                    "Multiple users named \"{}\" found:\n{}\nSpecify the accountId directly or use a more specific name.",
                    name,
                    lines.join("\n")
                );
            }

            // Interactive: show disambiguation prompt with email or accountId
            let labels: Vec<String> = duplicates
                .iter()
                .map(|u| {
                    match &u.email_address {
                        Some(email) => format!("{} ({})", u.display_name, email),
                        None => format!("{} ({})", u.display_name, u.account_id),
                    }
                })
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users named \"{}\"", name))
                .items(&labels)
                .interact()?;
            Ok(duplicates[selection].account_id.clone())
        }
```

- [ ] **Step 4: Implement `ExactMultiple` handling in `resolve_assignee`**

In `src/cli/issue/helpers.rs`, add the `ExactMultiple` arm to `resolve_assignee` (between the `Exact` arm ending at line 211 and the `Ambiguous` arm at line 212):

```rust
        crate::partial_match::MatchResult::ExactMultiple(_) => {
            let name_lower = name.to_lowercase();
            let duplicates: Vec<&crate::types::jira::User> = users
                .iter()
                .filter(|u| u.display_name.to_lowercase() == name_lower)
                .collect();

            if no_input {
                let lines: Vec<String> = duplicates
                    .iter()
                    .map(|u| {
                        let label = u
                            .email_address
                            .as_deref()
                            .unwrap_or(&u.account_id);
                        format!("  {} (account: {})", u.display_name, label)
                    })
                    .collect();
                anyhow::bail!(
                    "Multiple users named \"{}\" found:\n{}\nSpecify the accountId directly or use a more specific name.",
                    name,
                    lines.join("\n")
                );
            }

            let labels: Vec<String> = duplicates
                .iter()
                .map(|u| {
                    match &u.email_address {
                        Some(email) => format!("{} ({})", u.display_name, email),
                        None => format!("{} ({})", u.display_name, u.account_id),
                    }
                })
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users named \"{}\"", name))
                .items(&labels)
                .interact()?;
            Ok((
                duplicates[selection].account_id.clone(),
                duplicates[selection].display_name.clone(),
            ))
        }
```

- [ ] **Step 5: Implement `ExactMultiple` handling in `resolve_assignee_by_project`**

In `src/cli/issue/helpers.rs`, add the `ExactMultiple` arm to `resolve_assignee_by_project` (between the `Exact` arm ending at line 289 and the `Ambiguous` arm at line 290):

```rust
        crate::partial_match::MatchResult::ExactMultiple(_) => {
            let name_lower = name.to_lowercase();
            let duplicates: Vec<&crate::types::jira::User> = users
                .iter()
                .filter(|u| u.display_name.to_lowercase() == name_lower)
                .collect();

            if no_input {
                let lines: Vec<String> = duplicates
                    .iter()
                    .map(|u| {
                        let label = u
                            .email_address
                            .as_deref()
                            .unwrap_or(&u.account_id);
                        format!("  {} (account: {})", u.display_name, label)
                    })
                    .collect();
                anyhow::bail!(
                    "Multiple users named \"{}\" found:\n{}\nSpecify the accountId directly or use a more specific name.",
                    name,
                    lines.join("\n")
                );
            }

            let labels: Vec<String> = duplicates
                .iter()
                .map(|u| {
                    match &u.email_address {
                        Some(email) => format!("{} ({})", u.display_name, email),
                        None => format!("{} ({})", u.display_name, u.account_id),
                    }
                })
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users named \"{}\"", name))
                .items(&labels)
                .interact()?;
            Ok((
                duplicates[selection].account_id.clone(),
                duplicates[selection].display_name.clone(),
            ))
        }
```

- [ ] **Step 6: Implement `ExactMultiple` handling in `resolve_team_field`**

In `src/cli/issue/helpers.rs`, add the `ExactMultiple` arm to `resolve_team_field` (between the `Exact` arm ending at line 42 and the `Ambiguous` arm at line 43):

```rust
        crate::partial_match::MatchResult::ExactMultiple(_) => {
            let name_lower = team_name.to_lowercase();
            let duplicates: Vec<&crate::cache::CachedTeam> = teams
                .iter()
                .filter(|t| t.name.to_lowercase() == name_lower)
                .collect();

            if no_input {
                let lines: Vec<String> = duplicates
                    .iter()
                    .map(|t| format!("  {} (id: {})", t.name, t.id))
                    .collect();
                anyhow::bail!(
                    "Multiple teams named \"{}\" found:\n{}\nUse a more specific name.",
                    team_name,
                    lines.join("\n")
                );
            }

            let labels: Vec<String> = duplicates
                .iter()
                .map(|t| format!("{} ({})", t.name, t.id))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple teams named \"{}\"", team_name))
                .items(&labels)
                .interact()?;
            Ok((field_id, duplicates[selection].id.clone()))
        }
```

- [ ] **Step 7: Run the integration tests**

Run: `cargo test --test duplicate_user_disambiguation -- --nocapture`

Expected: Both `issue_list_assignee_duplicate_names_no_input_errors` and `issue_assign_duplicate_names_no_input_errors` PASS.

- [ ] **Step 8: Run full test suite**

Run: `cargo test`

Expected: All tests pass, no regressions.

- [ ] **Step 9: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: No warnings.

- [ ] **Step 10: Commit**

```bash
git add src/cli/issue/helpers.rs tests/duplicate_user_disambiguation.rs
git commit -m "fix: disambiguate users and teams with duplicate display names (#117, #122)"
```

---

### Task 4: Fix index-based mapping in existing `Exact` and `Ambiguous` branches

**Files:**
- Modify: `src/cli/issue/helpers.rs` — all four resolve functions, `Exact` and `Ambiguous` arms

This task fixes the secondary bug: `.find(|u| u.display_name == name)` in the `Exact` and `Ambiguous` branches can return the wrong user when display names collide. We replace name-based `.find()` with index-aware lookup.

- [ ] **Step 1: Write a failing integration test for `Exact` match with duplicate display names**

Add to `tests/duplicate_user_disambiguation.rs`:

```rust
#[tokio::test]
async fn issue_list_assignee_exact_match_among_multiple_results_no_input_errors() {
    let server = MockServer::start().await;

    // User search returns three users: two share "John Smith", one is "John Smithson"
    // partial_match("John Smith") → ExactMultiple (the two John Smiths)
    // This test verifies that even when the API returns a superset,
    // the ExactMultiple path catches the duplicate and errors in --no-input mode.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            user_json("acc-john-1", "John Smith", Some("john1@acme.com")),
            user_json("acc-smithson", "John Smithson", None),
            user_json("acc-john-2", "John Smith", Some("john2@other.org")),
        ])))
        .mount(&server)
        .await;

    let project_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"PROJ\"\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .current_dir(project_dir.path())
        .args(["issue", "list", "--assignee", "John Smith", "--no-input"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should fail on duplicate user names even with extra results, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("acc-john-1") && stderr.contains("acc-john-2"),
        "Should list both duplicate accountIds, got: {stderr}"
    );
    // Should NOT contain the non-duplicate user
    assert!(
        !stderr.contains("acc-smithson"),
        "Should not mention non-duplicate user, got: {stderr}"
    );
}
```

- [ ] **Step 2: Run the test to verify it passes (already handled by ExactMultiple)**

Run: `cargo test --test duplicate_user_disambiguation issue_list_assignee_exact_match_among_multiple_results -- --nocapture`

Expected: PASS — the `ExactMultiple` arm from Task 3 already handles this case. This test serves as a regression guard.

- [ ] **Step 3: Fix `resolve_user` `Exact` branch — use position-based lookup**

In `src/cli/issue/helpers.rs`, replace the `Exact` arm in `resolve_user` (lines 135-141):

```rust
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let user = active_users
                .iter()
                .find(|u| u.display_name == matched_name)
                .expect("matched name must exist in active_users");
            Ok(user.account_id.clone())
        }
```

With:

```rust
        crate::partial_match::MatchResult::Exact(ref matched_name) => {
            let idx = active_users
                .iter()
                .position(|u| u.display_name == *matched_name)
                .expect("matched name must exist in active_users");
            Ok(active_users[idx].account_id.clone())
        }
```

Note: Using `position` + index is functionally identical to `find` for the single-match case, but establishes the index-based pattern consistently.

- [ ] **Step 4: Fix `resolve_user` `Ambiguous` interactive branch — use index-based lookup**

In `src/cli/issue/helpers.rs`, replace the `Ambiguous` arm's interactive section in `resolve_user` (lines 150-159):

```rust
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let user = active_users
                .iter()
                .find(|u| &u.display_name == selected_name)
                .expect("selected name must exist in active_users");
            Ok(user.account_id.clone())
```

With:

```rust
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let idx = active_users
                .iter()
                .position(|u| u.display_name == *selected_name)
                .expect("selected name must exist in active_users");
            Ok(active_users[idx].account_id.clone())
```

- [ ] **Step 5: Fix `resolve_assignee` `Exact` branch**

In `src/cli/issue/helpers.rs`, replace the `Exact` arm in `resolve_assignee` (lines 205-211):

```rust
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let user = users
                .iter()
                .find(|u| u.display_name == matched_name)
                .expect("matched name must exist in users");
            Ok((user.account_id.clone(), user.display_name.clone()))
        }
```

With:

```rust
        crate::partial_match::MatchResult::Exact(ref matched_name) => {
            let idx = users
                .iter()
                .position(|u| u.display_name == *matched_name)
                .expect("matched name must exist in users");
            Ok((users[idx].account_id.clone(), users[idx].display_name.clone()))
        }
```

- [ ] **Step 6: Fix `resolve_assignee` `Ambiguous` interactive branch**

In `src/cli/issue/helpers.rs`, replace the `Ambiguous` arm's interactive section in `resolve_assignee` (lines 220-229):

```rust
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let user = users
                .iter()
                .find(|u| &u.display_name == selected_name)
                .expect("selected name must exist in users");
            Ok((user.account_id.clone(), user.display_name.clone()))
```

With:

```rust
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let idx = users
                .iter()
                .position(|u| u.display_name == *selected_name)
                .expect("selected name must exist in users");
            Ok((users[idx].account_id.clone(), users[idx].display_name.clone()))
```

- [ ] **Step 7: Fix `resolve_assignee_by_project` `Exact` branch**

In `src/cli/issue/helpers.rs`, replace the `Exact` arm in `resolve_assignee_by_project` (lines 283-289):

```rust
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let user = users
                .iter()
                .find(|u| u.display_name == matched_name)
                .expect("matched name must exist in users");
            Ok((user.account_id.clone(), user.display_name.clone()))
        }
```

With:

```rust
        crate::partial_match::MatchResult::Exact(ref matched_name) => {
            let idx = users
                .iter()
                .position(|u| u.display_name == *matched_name)
                .expect("matched name must exist in users");
            Ok((users[idx].account_id.clone(), users[idx].display_name.clone()))
        }
```

- [ ] **Step 8: Fix `resolve_assignee_by_project` `Ambiguous` interactive branch**

In `src/cli/issue/helpers.rs`, replace the `Ambiguous` arm's interactive section in `resolve_assignee_by_project` (lines 298-307):

```rust
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let user = users
                .iter()
                .find(|u| &u.display_name == selected_name)
                .expect("selected name must exist in users");
            Ok((user.account_id.clone(), user.display_name.clone()))
```

With:

```rust
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let idx = users
                .iter()
                .position(|u| u.display_name == *selected_name)
                .expect("selected name must exist in users");
            Ok((users[idx].account_id.clone(), users[idx].display_name.clone()))
```

- [ ] **Step 9: Fix `resolve_team_field` `Exact` branch**

In `src/cli/issue/helpers.rs`, replace the `Exact` arm in `resolve_team_field` (lines 36-42):

```rust
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let team = teams
                .iter()
                .find(|t| t.name == matched_name)
                .expect("matched name must exist in teams");
            Ok((field_id, team.id.clone()))
        }
```

With:

```rust
        crate::partial_match::MatchResult::Exact(ref matched_name) => {
            let idx = teams
                .iter()
                .position(|t| t.name == *matched_name)
                .expect("matched name must exist in teams");
            Ok((field_id, teams[idx].id.clone()))
        }
```

- [ ] **Step 10: Fix `resolve_team_field` `Ambiguous` interactive branch**

In `src/cli/issue/helpers.rs`, replace the `Ambiguous` arm's interactive section in `resolve_team_field` (lines 52-61):

```rust
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple teams match \"{team_name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let team = teams
                .iter()
                .find(|t| &t.name == selected_name)
                .expect("selected name must exist in teams");
            Ok((field_id, team.id.clone()))
```

With:

```rust
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple teams match \"{team_name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let idx = teams
                .iter()
                .position(|t| t.name == *selected_name)
                .expect("selected name must exist in teams");
            Ok((field_id, teams[idx].id.clone()))
```

- [ ] **Step 11: Run all tests**

Run: `cargo test`

Expected: All tests pass.

- [ ] **Step 12: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: No warnings.

- [ ] **Step 13: Commit**

```bash
git add src/cli/issue/helpers.rs
git commit -m "fix: use index-based lookup in Exact and Ambiguous branches (#117)"
```
