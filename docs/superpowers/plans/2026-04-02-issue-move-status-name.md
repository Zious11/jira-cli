# Issue Move: Accept Target Status Name — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow `jr issue move KEY "Completed"` to match on target status name (not just transition name), and improve the error message to show both.

**Architecture:** Replace the single-pass transition-name matching in `handle_move` with a unified candidate pool that includes both transition names and target status names, deduplicated case-insensitively. Update the error message to show `"TransitionName (→ StatusName)"` format. Update the `transitions_response` fixture to include `to` status objects.

**Tech Stack:** Rust, wiremock, assert_cmd, serde_json

---

### Task 1: Update the transitions fixture to include target status

The existing `transitions_response` fixture in `tests/common/fixtures.rs` only includes `id` and `name` — no `to` status object. The new matching logic needs `to.name`, so the fixture must be updated. A new fixture variant is needed that accepts `(id, transition_name, status_name)` triples.

**Files:**
- Modify: `tests/common/fixtures.rs:39-43`

- [ ] **Step 1: Add the new fixture function**

Add `transitions_response_with_status` below the existing `transitions_response` function (after line 43) in `tests/common/fixtures.rs`:

```rust
/// Transitions response with target status names.
/// Each tuple is (transition_id, transition_name, target_status_name).
pub fn transitions_response_with_status(transitions: Vec<(&str, &str, &str)>) -> Value {
    json!({
        "transitions": transitions.iter().map(|(id, name, status_name)| json!({
            "id": id,
            "name": name,
            "to": {"name": status_name}
        })).collect::<Vec<_>>()
    })
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo test --test issue_commands --no-run`
Expected: Compiles successfully (new function is unused but that's fine — `fixtures.rs` has `#[allow(dead_code)]` via the `mod common` declaration).

- [ ] **Step 3: Commit**

```bash
git add tests/common/fixtures.rs
git commit -m "test: add transitions_response_with_status fixture (#108)"
```

---

### Task 2: Implement unified candidate pool matching in handle_move

Replace the transition-name-only matching logic in `handle_move` (`src/cli/issue/workflow.rs` lines 98–139) with a unified candidate pool that includes both transition names and target status names, deduplicated case-insensitively.

**Files:**
- Modify: `src/cli/issue/workflow.rs:96-139`

- [ ] **Step 1: Replace the matching block**

In `src/cli/issue/workflow.rs`, replace lines 96–139 (the `let selected_transition = if let Some(t) ...` block through the closing `};`) with:

```rust
    let selected_transition = if let Some(t) = selected_transition {
        t
    } else {
        // Build unified candidate pool: transition names + target status names.
        // Each candidate maps to its transition index.
        let mut candidates: Vec<(String, usize)> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (i, t) in transitions.iter().enumerate() {
            let t_lower = t.name.to_lowercase();
            if seen.insert(t_lower) {
                candidates.push((t.name.clone(), i));
            }
            if let Some(ref status) = t.to {
                let s_lower = status.name.to_lowercase();
                if seen.insert(s_lower) {
                    candidates.push((status.name.clone(), i));
                }
            }
        }

        let candidate_names: Vec<String> = candidates.iter().map(|(name, _)| name.clone()).collect();
        match partial_match::partial_match(&target_status, &candidate_names) {
            MatchResult::Exact(name) => {
                let idx = candidates
                    .iter()
                    .find(|(n, _)| n == &name)
                    .map(|(_, i)| *i)
                    .unwrap();
                &transitions[idx]
            }
            MatchResult::Ambiguous(matches) => {
                if no_input {
                    bail!(
                        "Ambiguous transition \"{}\". Matches: {}",
                        target_status,
                        matches.join(", ")
                    );
                }
                // Interactive disambiguation
                eprintln!(
                    "Ambiguous match for \"{}\". Did you mean one of:",
                    target_status
                );
                for (i, m) in matches.iter().enumerate() {
                    eprintln!("  {}. {}", i + 1, m);
                }
                let choice = helpers::prompt_input("Select (number)")?;
                let idx: usize = choice
                    .parse()
                    .map_err(|_| JrError::UserError("Invalid selection".into()))?;
                if idx < 1 || idx > matches.len() {
                    return Err(JrError::UserError("Selection out of range".into()).into());
                }
                let selected_name = &matches[idx - 1];
                let tidx = candidates
                    .iter()
                    .find(|(n, _)| n == selected_name)
                    .map(|(_, i)| *i)
                    .unwrap();
                &transitions[tidx]
            }
            MatchResult::None(_) => {
                let labels: Vec<String> = transitions
                    .iter()
                    .map(|t| {
                        match t.to.as_ref() {
                            Some(status) => format!("{} (→ {})", t.name, status.name),
                            None => t.name.clone(),
                        }
                    })
                    .collect();
                bail!(
                    "No transition matching \"{}\". Available: {}",
                    target_status,
                    labels.join(", ")
                );
            }
        }
    };
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully.

- [ ] **Step 3: Run existing tests**

Run: `cargo test`
Expected: All tests pass. The existing `test_get_transitions` test doesn't exercise `handle_move` directly, so it's unaffected.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue/workflow.rs
git commit -m "feat: accept target status name in issue move matching (#108)"
```

---

### Task 3: Add integration tests for the new matching behavior

Add end-to-end integration tests in `tests/issue_commands.rs` that exercise `handle_move` via the CLI binary. These tests use `assert_cmd::Command::cargo_bin("jr")` with `JR_BASE_URL` and `JR_AUTH_HEADER` env vars against wiremock.

Each test needs two mocks: GET transitions (to list available transitions) and GET issue (for the idempotency check / current status). A POST transitions mock is needed for tests that succeed.

**Files:**
- Modify: `tests/issue_commands.rs` (append new tests at end of file)

- [ ] **Step 1: Add test for matching by transition name (existing behavior preserved)**

Append to `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_move_by_transition_name() {
    let server = MockServer::start().await;

    // Mock transitions: "Complete" → "Completed", "Review" → "In Review"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Complete", "Completed"),
                ("31", "Review", "In Review"),
            ]),
        ))
        .mount(&server)
        .await;

    // Mock get issue (current status: "To Do")
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_response("FOO-1", "Test issue", "To Do")),
        )
        .mount(&server)
        .await;

    // Mock POST transition
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .and(body_partial_json(serde_json::json!({"transition": {"id": "21"}})))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Complete")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "Expected success, stderr: {stderr}");
    assert!(
        stderr.contains("Moved FOO-1"),
        "Expected move confirmation in stderr: {stderr}"
    );
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test test_move_by_transition_name -- --nocapture`
Expected: PASS

- [ ] **Step 3: Add test for matching by status name (new behavior)**

Append to `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_move_by_status_name() {
    let server = MockServer::start().await;

    // Transition name "Complete" differs from status name "Completed"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Complete", "Completed"),
                ("31", "Review", "In Review"),
            ]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_response("FOO-1", "Test issue", "To Do")),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .and(body_partial_json(serde_json::json!({"transition": {"id": "21"}})))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Completed")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "Expected success, stderr: {stderr}");
    assert!(
        stderr.contains("Moved FOO-1"),
        "Expected move confirmation in stderr: {stderr}"
    );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_move_by_status_name -- --nocapture`
Expected: PASS

- [ ] **Step 5: Add test for deduplication (transition name == status name)**

Append to `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_move_dedup_same_transition_and_status_name() {
    let server = MockServer::start().await;

    // Transition name matches status name (default Jira workflow pattern)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "In Progress", "In Progress"),
                ("31", "Done", "Done"),
            ]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_response("FOO-1", "Test issue", "To Do")),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .and(body_partial_json(serde_json::json!({"transition": {"id": "31"}})))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Done")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "Expected success, stderr: {stderr}");
    assert!(
        stderr.contains("Moved FOO-1"),
        "Expected move confirmation in stderr: {stderr}"
    );
}
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test test_move_dedup_same_transition_and_status_name -- --nocapture`
Expected: PASS

- [ ] **Step 7: Add test for ambiguous match across transition and status names**

Append to `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_move_ambiguous_across_transition_and_status_names() {
    let server = MockServer::start().await;

    // "Re" partially matches both "Reopen" (transition) and "Review" (transition)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Reopen", "Open"),
                ("31", "Review", "In Review"),
            ]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response("FOO-1", "Test issue", "Closed"),
        ))
        .mount(&server)
        .await;

    // No POST mock — should not reach transition

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Re")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, stderr: {stderr}");
    assert!(
        stderr.contains("Ambiguous"),
        "Expected ambiguity error in stderr: {stderr}"
    );
}
```

- [ ] **Step 8: Run test to verify it passes**

Run: `cargo test test_move_ambiguous_across_transition_and_status_names -- --nocapture`
Expected: PASS — exits non-zero with "Ambiguous" error.

- [ ] **Step 9: Add test for error message format**

Append to `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_move_no_match_shows_status_names() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Complete", "Completed"),
                ("31", "Review", "In Review"),
            ]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_response("FOO-1", "Test issue", "To Do")),
        )
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Nonexistent")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected failure, stderr: {stderr}");
    assert!(
        stderr.contains("Complete (→ Completed)"),
        "Expected enriched error format in stderr: {stderr}"
    );
    assert!(
        stderr.contains("Review (→ In Review)"),
        "Expected enriched error format in stderr: {stderr}"
    );
}
```

- [ ] **Step 10: Run test to verify it passes**

Run: `cargo test test_move_no_match_shows_status_names -- --nocapture`
Expected: PASS — exits non-zero with enriched error message.

- [ ] **Step 11: Add test for idempotent move with status name input**

Append to `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_move_idempotent_with_status_name() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Complete", "Completed"),
            ]),
        ))
        .mount(&server)
        .await;

    // Issue is already in "Completed" status
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response("FOO-1", "Test issue", "Completed"),
        ))
        .mount(&server)
        .await;

    // No POST mock — should not reach transition

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Completed")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "Expected success (idempotent), stderr: {stderr}");
    assert!(
        stderr.contains("already in status"),
        "Expected idempotent message in stderr: {stderr}"
    );
}
```

- [ ] **Step 12: Run test to verify it passes**

Run: `cargo test test_move_idempotent_with_status_name -- --nocapture`
Expected: PASS — exits 0 with "already in status" message.

- [ ] **Step 13: Run the full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 14: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, no formatting issues.

- [ ] **Step 15: Commit**

```bash
git add tests/issue_commands.rs
git commit -m "test: add integration tests for issue move status name matching (#108)"
```
