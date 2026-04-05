# Assets View Default Attributes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `assets view` include attributes by default, replacing `--attributes` (opt-in) with `--no-attributes` (opt-out).

**Architecture:** Change the CLI flag from `--attributes` to `--no-attributes` on `AssetsCommand::View`, invert the condition in `handle_view`, and update tests. No API or type changes.

**Tech Stack:** Rust, clap (derive API), wiremock, assert_cmd

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/cli/mod.rs:120-127` | Modify | Change `View` variant: `attributes: bool` → `no_attributes: bool` |
| `src/cli/assets.rs:34-35` | Modify | Update dispatch: pass `no_attributes` instead of `attributes` |
| `src/cli/assets.rs:103-185` | Modify | Invert condition in `handle_view`: `!no_attributes` replaces `attributes` |
| `tests/cli_smoke.rs` | Modify | Add `assets view --help` smoke test |
| `tests/assets.rs` | Modify | Add integration test for default attributes fetch |

---

### Task 1: Change CLI flag from `--attributes` to `--no-attributes`

**Files:**
- Modify: `src/cli/mod.rs:120-127`
- Modify: `src/cli/assets.rs:34-35`
- Modify: `src/cli/assets.rs:103-185`

- [ ] **Step 1: Update the `AssetsCommand::View` variant in `src/cli/mod.rs`**

Change lines 120-127 from:

```rust
    /// View asset details
    View {
        /// Object key (e.g. OBJ-1) or numeric ID
        key: String,
        /// Include object attributes in output
        #[arg(long)]
        attributes: bool,
    },
```

to:

```rust
    /// View asset details
    View {
        /// Object key (e.g. OBJ-1) or numeric ID
        key: String,
        /// Omit object attributes from output
        #[arg(long)]
        no_attributes: bool,
    },
```

- [ ] **Step 2: Update the dispatch in `src/cli/assets.rs`**

Change lines 34-35 from:

```rust
        AssetsCommand::View { key, attributes } => {
            handle_view(&workspace_id, &key, attributes, output_format, client).await
```

to:

```rust
        AssetsCommand::View { key, no_attributes } => {
            handle_view(&workspace_id, &key, no_attributes, output_format, client).await
```

- [ ] **Step 3: Invert the condition in `handle_view` in `src/cli/assets.rs`**

Change the function signature at line 103 from:

```rust
async fn handle_view(
    workspace_id: &str,
    key: &str,
    attributes: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
```

to:

```rust
async fn handle_view(
    workspace_id: &str,
    key: &str,
    no_attributes: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
```

Then change the two `if attributes {` guards (lines 115 and 150) to `if !no_attributes {`:

Line 115:
```rust
            if !no_attributes {
```

Line 150:
```rust
            if !no_attributes {
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds with no errors.

- [ ] **Step 5: Run existing tests to verify nothing breaks**

Run: `cargo test --lib -- assets 2>&1 | tail -20`
Expected: All existing `assets` unit tests pass (filter_tickets tests are unaffected).

Run: `cargo test --test assets 2>&1 | tail -20`
Expected: All existing integration tests pass (they test API methods, not the CLI flag).

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs src/cli/assets.rs
git commit -m "$(cat <<'EOF'
fix: show attributes by default in assets view (#85)

Replace --attributes (opt-in) with --no-attributes (opt-out) on
assets view. The default output now includes the attributes table
and populated attributes in JSON, matching user expectations.
EOF
)"
```

---

### Task 2: Add CLI smoke test for `assets view --help`

**Files:**
- Modify: `tests/cli_smoke.rs`

- [ ] **Step 1: Write the smoke test**

Add the following test to `tests/cli_smoke.rs`:

```rust
#[test]
fn test_assets_view_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["assets", "view", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--no-attributes"));
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --test cli_smoke test_assets_view_help 2>&1 | tail -10`
Expected: PASS. The `--no-attributes` flag appears in help output.

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test --test cli_smoke test_assets_view_help 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/cli_smoke.rs
git commit -m "$(cat <<'EOF'
test: add CLI smoke test for assets view --no-attributes (#85)
EOF
)"
```

---

### Task 3: Add integration test verifying default attributes fetch

**Files:**
- Modify: `tests/assets.rs`

- [ ] **Step 1: Write the integration test**

Add the following test to `tests/assets.rs`. This tests that `get_object_attributes` is called and returns named attributes — validating the API layer that the default view path now exercises.

```rust
#[tokio::test]
async fn get_object_attributes_filters_system_and_hidden() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/88/attributes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "637",
                "objectTypeAttributeId": "134",
                "objectTypeAttribute": {
                    "id": "134",
                    "name": "Key",
                    "system": true,
                    "hidden": false,
                    "label": false,
                    "position": 0
                },
                "objectAttributeValues": [
                    { "value": "OBJ-88", "displayValue": "OBJ-88" }
                ]
            },
            {
                "id": "640",
                "objectTypeAttributeId": "135",
                "objectTypeAttribute": {
                    "id": "135",
                    "name": "Name",
                    "system": false,
                    "hidden": false,
                    "label": true,
                    "position": 1
                },
                "objectAttributeValues": [
                    { "value": "Acme Corp", "displayValue": "Acme Corp" }
                ]
            },
            {
                "id": "641",
                "objectTypeAttributeId": "140",
                "objectTypeAttribute": {
                    "id": "140",
                    "name": "Location",
                    "system": false,
                    "hidden": false,
                    "label": false,
                    "position": 5
                },
                "objectAttributeValues": [
                    { "value": "New York, NY", "displayValue": "New York, NY" }
                ]
            },
            {
                "id": "642",
                "objectTypeAttributeId": "141",
                "objectTypeAttribute": {
                    "id": "141",
                    "name": "Internal Notes",
                    "system": false,
                    "hidden": true,
                    "label": false,
                    "position": 6
                },
                "objectAttributeValues": [
                    { "value": "secret", "displayValue": "secret" }
                ]
            }
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let mut attrs = client.get_object_attributes("ws-123", "88").await.unwrap();

    // Apply the same filter used by handle_view for JSON output
    attrs.retain(|a| !a.object_type_attribute.system && !a.object_type_attribute.hidden);
    attrs.sort_by_key(|a| a.object_type_attribute.position);

    // System (Key) and hidden (Internal Notes) are excluded
    assert_eq!(attrs.len(), 2);
    assert_eq!(attrs[0].object_type_attribute.name, "Name");
    assert_eq!(attrs[1].object_type_attribute.name, "Location");
    assert_eq!(
        attrs[1].values[0].display_value.as_deref(),
        Some("New York, NY")
    );
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --test assets get_object_attributes_filters 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/assets.rs
git commit -m "$(cat <<'EOF'
test: add integration test for attributes filtering (#85)
EOF
)"
```

---

### Task 4: Run full test suite and lint

**Files:** None (verification only)

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -- -D warnings 2>&1 | tail -20`
Expected: No warnings or errors.

- [ ] **Step 2: Run format check**

Run: `cargo fmt --all -- --check 2>&1 | tail -10`
Expected: No formatting issues.

- [ ] **Step 3: Run full test suite**

Run: `cargo test 2>&1 | tail -30`
Expected: All tests pass. Key tests to verify:
- `tests/cli_smoke.rs::test_assets_view_help` — PASS
- `tests/assets.rs::get_object_attributes_filters_system_and_hidden` — PASS
- `tests/assets.rs::get_object_attributes_returns_named_attributes` — PASS (existing, unchanged)
- All `cli::assets::tests::filter_*` unit tests — PASS (unchanged)
