# `jr issue create --output json` full-shape Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `jr issue create --output json` return the same full `Issue` payload as `jr issue view --output json`, plus `url`. On follow-up GET failure, degrade gracefully to `{key, url}` with a stderr warning.

**Architecture:** After successful POST, call `get_issue(&key, &extra)` using the same `extra_fields` composition as `handle_view`. Extract the composition into a shared helper. Table output path is unchanged (no extra GET).

**Tech Stack:** reqwest (existing `JiraClient`), serde_json, wiremock for integration tests.

**Spec:** `docs/specs/issue-create-json-full-shape.md`

**Known collision check:** `handle_view` already does this composition at `src/cli/issue/list.rs:759-770`. The helper must be reusable by both handlers without cyclic imports. Put the helper in `src/cli/issue/helpers.rs` alongside the existing team/points/user resolvers.

---

## Task 1: Extract shared `extra_fields_for_issue` helper

**Files:**
- Modify: `src/cli/issue/helpers.rs` — add `pub(super) async fn extra_fields_for_issue(client: &JiraClient, config: &Config) -> (Vec<String>, Vec<(String,String)>)`. Returns owned `Vec<String>` (story-points + cmdb ids + team) AND the raw `cmdb_fields` `Vec<(id, name)>` so the view path can still do per-field asset enrichment.
- Modify: `src/cli/issue/list.rs:759-770` — replace inline composition in `handle_view` with a call to the helper.
- Test: `src/cli/issue/helpers.rs` — inline `#[cfg(test)]` unit test exercising the composition with mocked inputs (no network — drive via a `Config` fixture and a stub cmdb list).

**Rationale:** DRY — both `handle_view` (existing) and `handle_create` (new) need the exact same extra-fields list.

- [ ] **Step 1: Write the failing unit test**

File: `src/cli/issue/helpers.rs` (append to existing `#[cfg(test)] mod tests`):

```rust
#[test]
fn extra_fields_for_issue_composes_sp_team_and_cmdb() {
    use crate::config::{Config, Fields, GlobalConfig};

    let mut config = Config::default();
    config.global.fields.story_points_field_id = Some("customfield_10016".into());
    config.global.fields.team_field_id = Some("customfield_10001".into());
    let cmdb_fields = vec![
        ("customfield_12345".to_string(), "Affected Services".to_string()),
        ("customfield_67890".to_string(), "Deployed To".to_string()),
    ];

    let extra = compose_extra_fields(&config, &cmdb_fields);

    assert!(extra.contains(&"customfield_10016".to_string()), "sp present");
    assert!(extra.contains(&"customfield_10001".to_string()), "team present");
    assert!(extra.contains(&"customfield_12345".to_string()), "cmdb 1 present");
    assert!(extra.contains(&"customfield_67890".to_string()), "cmdb 2 present");
    assert_eq!(extra.len(), 4);
}

#[test]
fn extra_fields_for_issue_omits_unset_optionals() {
    use crate::config::Config;

    let config = Config::default();
    let cmdb_fields: Vec<(String, String)> = vec![];

    let extra = compose_extra_fields(&config, &cmdb_fields);
    assert!(extra.is_empty());
}
```

Run: `cargo test -p jr --lib issue::helpers::tests::extra_fields_for_issue -- --nocapture`
Expected: FAIL — `compose_extra_fields` not yet defined.

- [ ] **Step 2: Write minimal implementation**

Add to `src/cli/issue/helpers.rs`:

```rust
/// Compose the `extra` fields list that both `handle_view` and `handle_create`
/// pass to `JiraClient::get_issue`. Order is: story-points, cmdb ids, team.
pub(super) fn compose_extra_fields(
    config: &Config,
    cmdb_fields: &[(String, String)],
) -> Vec<String> {
    let mut extra: Vec<String> = Vec::new();
    if let Some(sp) = config.global.fields.story_points_field_id.as_deref() {
        extra.push(sp.to_string());
    }
    for (id, _) in cmdb_fields {
        extra.push(id.clone());
    }
    if let Some(t) = config.global.fields.team_field_id.as_deref() {
        extra.push(t.to_string());
    }
    extra
}
```

Add the import `use crate::config::Config;` at the top of the file if missing.

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p jr --lib issue::helpers::tests::extra_fields_for_issue -- --nocapture`
Expected: PASS (2/2).

- [ ] **Step 4: Replace inline composition in handle_view**

Edit `src/cli/issue/list.rs` around lines 759–770. Old code:

```rust
let sp_field_id = config.global.fields.story_points_field_id.as_deref();
let team_field_id: Option<&str> = config.global.fields.team_field_id.as_deref();
let cmdb_fields = get_or_fetch_cmdb_fields(client).await.unwrap_or_default();
let cmdb_field_id_list = cmdb_field_ids(&cmdb_fields);
let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
for f in &cmdb_field_id_list {
    extra.push(f.as_str());
}
if let Some(t) = team_field_id {
    extra.push(t);
}
let mut issue = client.get_issue(&key, &extra).await?;
```

New code:

```rust
let cmdb_fields = get_or_fetch_cmdb_fields(client).await.unwrap_or_default();
let extra_owned = super::helpers::compose_extra_fields(config, &cmdb_fields);
let extra: Vec<&str> = extra_owned.iter().map(String::as_str).collect();
let mut issue = client.get_issue(&key, &extra).await?;
```

- [ ] **Step 5: Run full test suite**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: all green. handle_view tests/snapshots unchanged.

- [ ] **Step 6: Commit**

```bash
git add src/cli/issue/helpers.rs src/cli/issue/list.rs
git commit -m "refactor(issue): extract compose_extra_fields helper"
```

---

## Task 2: Integration test — happy path (full shape)

**Files:**
- Create: `tests/issue_create_json.rs` — new wiremock integration test file.

- [ ] **Step 1: Write the failing integration test**

Create `tests/issue_create_json.rs`:

```rust
mod common;

use assert_cmd::Command;
use common::fixtures;
use serde_json::Value;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn issue_create_json_returns_full_shape() {
    let server = MockServer::start().await;

    // POST returns minimal Atlassian shape: {id, key, self}
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Follow-up GET returns full issue
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
            "fields": {
                "summary": "test summary",
                "status": { "name": "To Do", "statusCategory": { "name": "To Do", "key": "new" } },
                "issuetype": { "name": "Task" },
                "project": { "key": "PROJ" }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // List fields (find_* helpers) — minimal stub
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_EMAIL", "test@example.com")
        .env("JR_AUTH_TOKEN", "fake-token")
        .env("JR_CACHE_DIR", fixtures::temp_cache_dir())
        .env("JR_CONFIG_DIR", fixtures::temp_config_dir())
        .args([
            "issue", "create",
            "--project", "PROJ",
            "--issue-type", "Task",
            "--summary", "test summary",
            "--output", "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "expected success: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Value = serde_json::from_str(&stdout).expect("valid JSON");

    assert_eq!(parsed["key"], "PROJ-123");
    assert!(parsed["url"].as_str().unwrap().ends_with("/browse/PROJ-123"));
    assert_eq!(parsed["fields"]["summary"], "test summary");
    assert_eq!(parsed["fields"]["status"]["name"], "To Do");
}
```

**Note:** Before writing this, check `tests/common/fixtures.rs` for existing helpers (`temp_cache_dir`, `temp_config_dir`, env setup). If names differ, use the existing helpers — don't fabricate.

Run: `cargo test --test issue_create_json issue_create_json_returns_full_shape -- --nocapture`
Expected: FAIL — output has only `{key, url}`, missing `.fields`.

- [ ] **Step 2: Implement JSON path in handle_create**

Edit `src/cli/issue/create.rs` around lines 138–156:

```rust
let response = client.create_issue(fields).await?;

let browse_url = format!(
    "{}/browse/{}",
    client.instance_url().trim_end_matches('/'),
    response.key
);

match output_format {
    OutputFormat::Json => {
        let cmdb_fields = crate::cli::issue::assets::get_or_fetch_cmdb_fields(client)
            .await
            .unwrap_or_default();
        let extra_owned = helpers::compose_extra_fields(config, &cmdb_fields);
        let extra: Vec<&str> = extra_owned.iter().map(String::as_str).collect();

        match client.get_issue(&response.key, &extra).await {
            Ok(issue) => {
                let mut issue_json = serde_json::to_value(&issue)?;
                if let Some(obj) = issue_json.as_object_mut() {
                    obj.insert("url".into(), serde_json::Value::String(browse_url.clone()));
                }
                println!("{}", serde_json::to_string_pretty(&issue_json)?);
            }
            Err(err) => {
                eprintln!(
                    "[warn] issue created ({}) but follow-up fetch failed: {err}",
                    response.key
                );
                let mut json_response = serde_json::to_value(&response)?;
                json_response["url"] = json!(browse_url);
                println!("{}", serde_json::to_string_pretty(&json_response)?);
            }
        }
    }
    OutputFormat::Table => {
        output::print_success(&format!("Created issue {}", response.key));
        eprintln!("{}", browse_url);
    }
}
```

Adjust the `get_or_fetch_cmdb_fields` import path: it lives in `src/cli/issue/assets.rs`. Check the existing `use` block at the top of `create.rs` — may need to add the import.

- [ ] **Step 3: Run the integration test**

Run: `cargo test --test issue_create_json issue_create_json_returns_full_shape`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue/create.rs tests/issue_create_json.rs
git commit -m "feat(issue): issue create --output json returns full shape"
```

---

## Task 3: Integration test — degraded path (GET fails after POST)

**Files:**
- Modify: `tests/issue_create_json.rs` — add second test.

- [ ] **Step 1: Write the failing test**

Append to `tests/issue_create_json.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn issue_create_json_degrades_when_follow_up_get_fails() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-124",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-124"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_EMAIL", "test@example.com")
        .env("JR_AUTH_TOKEN", "fake-token")
        .env("JR_CACHE_DIR", fixtures::temp_cache_dir())
        .env("JR_CONFIG_DIR", fixtures::temp_config_dir())
        .args([
            "issue", "create",
            "--project", "PROJ",
            "--issue-type", "Task",
            "--summary", "degraded test",
            "--output", "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    // Create succeeded even though follow-up GET failed → exit 0.
    assert!(output.status.success(), "expected success (create did succeed): stderr={}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    let parsed: Value = serde_json::from_str(&stdout).expect("valid JSON on fallback");
    assert_eq!(parsed["key"], "PROJ-124");
    assert!(parsed["url"].as_str().unwrap().ends_with("/browse/PROJ-124"));
    assert!(parsed.get("fields").is_none(), "fallback shape must not include fields");

    assert!(stderr.contains("[warn]"), "expected warning on stderr, got: {stderr}");
    assert!(stderr.contains("PROJ-124"), "warning should mention key");
}
```

Run: `cargo test --test issue_create_json issue_create_json_degrades_when_follow_up_get_fails`
Expected: PASS (the previous implementation already handles this via the `Err(err)` arm). If it fails, iterate the fallback code in Task 2 Step 2 until green.

- [ ] **Step 2: Commit**

```bash
git add tests/issue_create_json.rs
git commit -m "test(issue): cover follow-up GET failure on create --output json"
```

---

## Task 4: Integration test — table output path unchanged (no extra GET)

**Files:**
- Modify: `tests/issue_create_json.rs` — add third test.

- [ ] **Step 1: Write the failing test**

Append to `tests/issue_create_json.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn issue_create_table_does_not_trigger_follow_up_get() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-125",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // A GET on the issue MUST NOT be made on the table path.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-125"))
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_EMAIL", "test@example.com")
        .env("JR_AUTH_TOKEN", "fake-token")
        .env("JR_CACHE_DIR", fixtures::temp_cache_dir())
        .env("JR_CONFIG_DIR", fixtures::temp_config_dir())
        .args([
            "issue", "create",
            "--project", "PROJ",
            "--issue-type", "Task",
            "--summary", "table test",
            "--no-input",
        ]) // no --output json → defaults to table
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Created issue PROJ-125"), "stdout: {stdout}");
    // wiremock's .expect(0) will fail the test on drop if the GET was called.
}
```

Run: `cargo test --test issue_create_json issue_create_table_does_not_trigger_follow_up_get`
Expected: PASS.

- [ ] **Step 2: Commit**

```bash
git add tests/issue_create_json.rs
git commit -m "test(issue): verify table path does not trigger follow-up GET"
```

---

## Task 5: Final checks + doc touch-up

- [ ] **Step 1: Run the full CI-equivalent check set**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Every check must pass.

- [ ] **Step 2: Verify README / help text accuracy**

`grep -n "create" README.md` to find any place documenting `issue create --output json` shape. If the README shows the old `{key, url}` output shape as an example, update it to the new full-shape example. If no such example exists, skip.

`cargo run -- issue create --help` should still work (flag list is unchanged). Verify.

- [ ] **Step 3: Commit any doc changes (only if README was updated)**

```bash
git add README.md
git commit -m "docs: update issue create JSON example to full shape"
```

- [ ] **Step 4: Declare done**

All 4 previous tasks complete + this final check = ready for local review pass.
