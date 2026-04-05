# `--asset` Name Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `--asset` accept asset names in addition to object keys, resolving names via AQL search with disambiguation.

**Architecture:** New `resolve_asset` function in `helpers.rs` follows the existing `resolve_user` pattern. Detects key vs name via `validate_asset_key`, resolves names via AQL `Name like "value"`, disambiguates with `partial_match`. The resolved key feeds into the existing `build_asset_clause` JQL generation unchanged.

**Tech Stack:** Rust, clap (derive), wiremock (tests), assert_cmd/predicates (tests)

---

## File Structure

| File | Responsibility | Change type |
|------|---------------|-------------|
| `src/cli/issue/helpers.rs` | Asset name → key resolution with disambiguation | Add `resolve_asset` function |
| `src/cli/issue/list.rs` | Handler integration — replace `validate_asset_key` with `resolve_asset` | Modify lines 97-100 |
| `tests/cli_handler.rs` | Handler-level tests for name resolution | Add 2 tests |

---

### Task 1: Implement `resolve_asset` in `helpers.rs`

**Files:**
- Modify: `src/cli/issue/helpers.rs`

- [ ] **Step 1: Add the `resolve_asset` function**

Add this function to `src/cli/issue/helpers.rs`, after the `resolve_assignee_by_project` function (before the `#[cfg(test)]` block, around line 357):

```rust
/// Resolve an `--asset` flag value to an object key.
///
/// - Value matches `SCHEMA-NUMBER` key pattern → return as-is (no API call)
/// - Otherwise → search Assets by name via AQL, disambiguate if multiple matches
///
/// Returns the resolved object key (e.g., `"OBJ-18"`).
pub(super) async fn resolve_asset(
    client: &JiraClient,
    input: &str,
    no_input: bool,
) -> Result<String> {
    // Key pattern → passthrough (no API call)
    if crate::jql::validate_asset_key(input).is_ok() {
        return Ok(input.to_string());
    }

    // Name search: fetch workspace ID, then AQL search
    let workspace_id =
        crate::api::assets::workspace::get_or_fetch_workspace_id(client).await?;
    let escaped = crate::jql::escape_value(input);
    let aql = format!("Name like \"{}\"", escaped);
    let results = client
        .search_assets(&workspace_id, &aql, Some(25), false)
        .await?;

    if results.is_empty() {
        anyhow::bail!(
            "No assets matching \"{}\" found. Check the name and try again.",
            input
        );
    }

    if results.len() == 1 {
        return Ok(results.into_iter().next().unwrap().object_key);
    }

    // Multiple results — disambiguate via partial_match on labels
    let labels: Vec<String> = results.iter().map(|a| a.label.clone()).collect();
    match crate::partial_match::partial_match(input, &labels) {
        crate::partial_match::MatchResult::Exact(matched_label) => {
            let asset = results
                .iter()
                .find(|a| a.label == matched_label)
                .expect("matched label must exist in results");
            Ok(asset.object_key.clone())
        }
        crate::partial_match::MatchResult::ExactMultiple(_) => {
            // Multiple assets with same label — need key to disambiguate
            let label_lower = input.to_lowercase();
            let duplicates: Vec<_> = results
                .iter()
                .filter(|a| a.label.to_lowercase() == label_lower)
                .collect();

            if no_input {
                let lines: Vec<String> = duplicates
                    .iter()
                    .map(|a| format!("  {} ({})", a.object_key, a.label))
                    .collect();
                anyhow::bail!(
                    "Multiple assets match \"{}\":\n{}\nUse a more specific name or pass the object key directly.",
                    input,
                    lines.join("\n")
                );
            }

            let items: Vec<String> = duplicates
                .iter()
                .map(|a| format!("{} ({})", a.object_key, a.label))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple assets match \"{}\"", input))
                .items(&items)
                .interact()?;
            Ok(duplicates[selection].object_key.clone())
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            if no_input {
                // Build lines with key + label for each ambiguous match
                let lines: Vec<String> = results
                    .iter()
                    .filter(|a| matches.contains(&a.label))
                    .map(|a| format!("  {} ({})", a.object_key, a.label))
                    .collect();
                anyhow::bail!(
                    "Multiple assets match \"{}\":\n{}\nUse a more specific name or pass the object key directly.",
                    input,
                    lines.join("\n")
                );
            }

            let items: Vec<String> = results
                .iter()
                .filter(|a| matches.contains(&a.label))
                .map(|a| format!("{} ({})", a.object_key, a.label))
                .collect();
            let filtered: Vec<_> = results
                .iter()
                .filter(|a| matches.contains(&a.label))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple assets match \"{}\"", input))
                .items(&items)
                .interact()?;
            Ok(filtered[selection].object_key.clone())
        }
        crate::partial_match::MatchResult::None(_) => {
            // AQL returned results but partial_match found no substring match.
            // This shouldn't normally happen (AQL already filtered by Name like),
            // but handle gracefully.
            let lines: Vec<String> = results
                .iter()
                .map(|a| format!("  {} ({})", a.object_key, a.label))
                .collect();
            anyhow::bail!(
                "No assets with a name matching \"{}\" found. Similar results:\n{}\nUse the object key directly.",
                input,
                lines.join("\n")
            );
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo check 2>&1 | tail -10`

Expected: Compiles (function is defined but not yet called).

- [ ] **Step 3: Commit**

```bash
git add src/cli/issue/helpers.rs
git commit -m "feat: add resolve_asset function for name-based asset lookup (#101)"
```

---

### Task 2: Wire `resolve_asset` into `list.rs`

**Files:**
- Modify: `src/cli/issue/list.rs:97-100`

- [ ] **Step 1: Replace `validate_asset_key` with `resolve_asset`**

In `src/cli/issue/list.rs`, replace the `validate_asset_key` block (lines 97-100):

```rust
    // Validate --asset key format early
    if let Some(ref key) = asset_key {
        crate::jql::validate_asset_key(key).map_err(JrError::UserError)?;
    }
```

With:

```rust
    // Resolve --asset: key passthrough or name → key via AQL search
    let asset_key = if let Some(raw) = asset_key {
        Some(helpers::resolve_asset(client, &raw, no_input).await?)
    } else {
        None
    };
```

- [ ] **Step 2: Verify it compiles and existing tests pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo check 2>&1 | tail -10 && cargo test --lib 2>&1 | tail -10`

Expected: Compiles. All existing tests pass. The `asset_key` variable is now a `Some(String)` with the resolved key — the downstream code (`build_asset_clause`) already takes `ref key` from it, so no further changes needed.

- [ ] **Step 3: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "feat: wire resolve_asset into issue list handler (#101)"
```

---

### Task 3: Handler Tests

**Files:**
- Modify: `tests/cli_handler.rs`

These tests verify the full name resolution flow end-to-end against a wiremock server. The handler test for `--asset "Acme"` needs these mocks:

1. **Workspace discovery** — `GET /rest/servicedeskapi/assets/workspace`
2. **AQL search** — `POST /jsm/assets/workspace/ws-123/v1/object/aql` with `Name like "Acme"`
3. **CMDB fields** — `GET /rest/api/3/field` (returns a CMDB field)
4. **Project check** — `GET /rest/api/3/project/PROJ`
5. **Issue search** — `POST /rest/api/3/search/jql` (verifies the JQL contains the resolved key)

- [ ] **Step 1: Write handler test for name → single match**

Add this test to `tests/cli_handler.rs`, after the last existing test:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_list_asset_name_resolves_to_key() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    // 1. Workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    // 2. AQL search — returns single match
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": true,
            "values": [{
                "id": "70",
                "label": "Acme Corp",
                "objectKey": "OBJ-70",
                "objectType": { "id": "13", "name": "Client" }
            }]
        })))
        .mount(&server)
        .await;

    // 3. CMDB fields discovery
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": "customfield_10191",
                "name": "Client",
                "custom": true,
                "schema": {
                    "type": "any",
                    "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
                    "customId": 10191
                }
            }
        ])))
        .mount(&server)
        .await;

    // 4. Project check
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ",
            "id": "10000",
            "name": "Test Project"
        })))
        .mount(&server)
        .await;

    // 5. Issue search — verify JQL uses resolved key OBJ-70
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": "project = \"PROJ\" AND \"Client\" IN aqlFunction(\"Key = \\\"OBJ-70\\\"\") ORDER BY updated DESC"
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
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--asset",
            "Acme",
            "--no-input",
        ])
        .assert()
        .success();
}
```

- [ ] **Step 2: Write handler test for name → no match error**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_list_asset_name_no_match_errors() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    // 1. Workspace discovery
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/assets/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{ "workspaceId": "ws-123" }]
        })))
        .mount(&server)
        .await;

    // 2. AQL search — returns zero matches
    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 0,
            "isLast": true,
            "values": []
        })))
        .mount(&server)
        .await;

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .args([
            "issue",
            "list",
            "--project",
            "PROJ",
            "--asset",
            "Nonexistent",
            "--no-input",
        ])
        .assert()
        .failure()
        .stderr(predicates::prelude::predicate::str::contains(
            "No assets matching \"Nonexistent\" found",
        ));
}
```

- [ ] **Step 3: Run handler tests**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --test cli_handler test_handler_list_asset_name 2>&1 | tail -20`

Expected: Both tests PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/cli_handler.rs
git commit -m "test: add handler tests for asset name resolution (#101)"
```

---

### Task 4: Format and Lint Check

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
git commit -m "style: format asset name resolution implementation (#101)"
```

(Skip commit if no changes.)
