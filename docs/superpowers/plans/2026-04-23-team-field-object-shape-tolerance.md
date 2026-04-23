# Team field object-shape tolerance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `IssueFields::team_id` tolerant of the Atlas Teams object shape (`{"id": "uuid", "name": "..."}`) in addition to the scalar-UUID string shape. Fix silent team-row drop + misleading verbose warning.

**Architecture:** Extend the pattern-match in `team_id` to cover `serde_json::Value::Object` with a string `id` field. Downstream callers (display, list, sprint, board) are unaffected — they still receive `Option<String>` containing a UUID.

**Tech Stack:** serde_json (existing `Value::Object` / `Value::as_str`), insta for any applicable snapshots (none needed for this change), wiremock for integration.

**Spec:** `docs/specs/team-field-object-shape-tolerance.md`

---

## Task 1: Unit tests for object-shape extraction

**Files:**
- Modify: `src/types/jira/issue.rs` — add failing unit tests inside the existing `#[cfg(test)] mod tests` block.

- [ ] **Step 1: Write failing tests**

Append to the existing tests module (which already has `fields_with_extra` helper around line 218):

```rust
#[test]
fn team_id_accepts_object_shape_with_string_id() {
    let fields = fields_with_extra(
        "customfield_10001",
        json!({"id": "team-uuid-abc", "name": "Platform Team"}),
    );
    assert_eq!(
        fields.team_id("customfield_10001", false),
        Some("team-uuid-abc".to_string())
    );
}

#[test]
fn team_id_accepts_object_shape_without_name() {
    let fields = fields_with_extra(
        "customfield_10001",
        json!({"id": "team-uuid-xyz"}),
    );
    assert_eq!(
        fields.team_id("customfield_10001", false),
        Some("team-uuid-xyz".to_string())
    );
}

#[test]
fn team_id_returns_none_for_object_with_null_id() {
    let fields = fields_with_extra(
        "customfield_10001",
        json!({"id": null, "name": "Platform Team"}),
    );
    assert_eq!(fields.team_id("customfield_10001", false), None);
}

#[test]
fn team_id_returns_none_for_object_without_id_key() {
    let fields = fields_with_extra(
        "customfield_10001",
        json!({"name": "Platform Team"}),
    );
    assert_eq!(fields.team_id("customfield_10001", false), None);
}
```

Check the existing `fields_with_extra` helper signature — it takes `(key: &str, value: serde_json::Value) -> IssueFields`. If the helper name or signature differs, adapt accordingly (grep the test module to confirm).

Run: `cargo test --lib types::jira::issue::tests::team_id_accepts_object -- --nocapture`
Expected: FAIL — the two "object with string id" tests fail because the current code returns `None` for objects.

- [ ] **Step 2: Implement object-shape extraction**

Replace the body of `team_id` at `src/types/jira/issue.rs:94-111` with:

```rust
pub fn team_id(&self, field_id: &str, verbose: bool) -> Option<String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static LOGGED: AtomicBool = AtomicBool::new(false);
    let value = self.extra.get(field_id)?;
    if value.is_null() {
        return None;
    }
    // Scalar UUID shape (legacy and some tenants).
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    // Atlas Teams object shape: {"id": "<uuid>", "name": "..."}
    // Per developer.atlassian.com/platform/teams/components/team-field-in-jira-rest-api,
    // the Team custom field returns an object on GET in tenants that use the Atlas
    // Teams platform. Extract `id` as the UUID.
    if let Some(id) = value
        .as_object()
        .and_then(|obj| obj.get("id"))
        .and_then(|v| v.as_str())
    {
        return Some(id.to_string());
    }
    if verbose && !LOGGED.swap(true, Ordering::Relaxed) {
        eprintln!(
            "[verbose] team field \"{field_id}\" has unexpected shape (got {}). \
             Expected string UUID or object with string \"id\".",
            value_kind(value)
        );
    }
    None
}
```

Note: this plan originally prescribed let-chain syntax (`if let Some(obj) = ... && let Some(id) = ...`), which stabilized in Rust 1.88 and breaks the crate's MSRV of 1.85. The snippet above uses the MSRV-safe `.and_then()` chain that the final code landed. The warning text includes the "Expected string UUID or object with string" suffix added during local review.

Also update the doc comment immediately above the function to reflect the new contract:

```rust
/// Extract the team UUID from the issue's team field.
///
/// Accepts two shapes documented for Jira's Team custom field:
/// - Scalar string UUID (legacy / some tenants).
/// - Object `{"id": "<uuid>", "name": "..."}` (Atlas Teams platform).
///
/// Returns `None` when the field is missing, null, or present but not one of
/// the accepted shapes. An object whose `id` is null or not a string is
/// treated as unexpected. On genuinely unexpected shapes (bool, number,
/// array, or object without a string `id`), emits a once-per-process
/// `[verbose]` hint on stderr when `verbose` is true. The once-per-process
/// gate is module-wide: if a single run needed to warn for two distinct
/// team fields (not a supported configuration today), only the first would
/// emit.
```

- [ ] **Step 3: Run tests**

`cargo test --lib types::jira::issue::tests`
Expected: ALL pass (pre-existing tests + 4 new).

- [ ] **Step 4: Full CI-equivalent check set**

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All green.

- [ ] **Step 5: Commit**

```bash
git add src/types/jira/issue.rs
git commit -m "fix(team): accept Atlas Teams object shape in team_id extraction"
```

---

## Task 2: Integration test via wiremock

**Files:**
- Create: `tests/team_object_shape.rs` — new integration test file.

- [ ] **Step 1: Inspect existing integration tests**

Read `tests/team_column_parity.rs` and `tests/issue_view_errors.rs` to learn the local conventions:
- How to set `team_field_id` in the config file created under `$XDG_CONFIG_HOME/jr/config.toml`
- How to prime (or skip priming) the team cache — if a test doesn't prime it, does display fall back to the raw UUID, or does the team lookup call Jira?
- `JR_AUTH_HEADER` / `JR_BASE_URL` / `XDG_CACHE_HOME` / `XDG_CONFIG_HOME` env setup pattern
- Whether `#[tokio::test]` is plain or `multi_thread`

- [ ] **Step 2: Write the failing test**

Create `tests/team_object_shape.rs`. Test name: `issue_view_json_extracts_team_uuid_from_object_shape`.

Shape:
- Write config with `[fields] team_field_id = "customfield_10001"`
- Mount wiremock `/rest/api/3/field` returning an empty array (no CMDB discovery for this test)
- Mount `/rest/api/3/issue/PROJ-700` returning an issue whose `fields.customfield_10001` is the **object shape**: `{"id": "team-uuid-alpha", "name": "Platform Team"}`
- Run `jr issue view PROJ-700 --output json`
- Parse stdout. Assert `.fields.customfield_10001.id == "team-uuid-alpha"` (the raw response preserves the object, per Serde `extra: HashMap<String, Value>`)
- Assert `output.status.success()`
- Assert stderr does NOT contain `"unexpected shape"` (the warning we're fixing must not fire)

Example skeleton — adapt to actual fixture conventions found in step 1:

```rust
use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn issue_view_json_extracts_team_uuid_from_object_shape() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-700"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": "PROJ-700",
            "fields": {
                "summary": "test",
                "status": { "name": "To Do", "statusCategory": { "name": "To Do", "key": "new" } },
                "issuetype": { "name": "Task" },
                "project": { "key": "PROJ" },
                "customfield_10001": { "id": "team-uuid-alpha", "name": "Platform Team" }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    // Write config with team_field_id set. Check src/config.rs for exact TOML shape.
    std::fs::create_dir_all(config_dir.path().join("jr")).unwrap();
    std::fs::write(
        config_dir.path().join("jr/config.toml"),
        "[fields]\nteam_field_id = \"customfield_10001\"\n",
    )
    .unwrap();

    let output = Command::cargo_bin("jr").unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "view", "PROJ-700", "--output", "json", "--no-input"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let parsed: Value = serde_json::from_str(&stdout).expect("valid JSON");

    assert_eq!(
        parsed["fields"]["customfield_10001"]["id"],
        "team-uuid-alpha",
        "team field must be preserved in JSON output as object with .id"
    );
    assert!(
        !stderr.contains("unexpected shape"),
        "warning about unexpected shape must not fire on object-shape response; stderr: {stderr}"
    );
}
```

Run: `cargo test --test team_object_shape`
Expected: PASS after Task 1 changes. If it fails, diagnose and iterate.

- [ ] **Step 3: (Optional) Add a second test asserting verbose-flag path works**

If the codebase exposes `--verbose` at the CLI, add a companion test that invokes with `--verbose` and asserts the warning is NOT emitted on the object-shape response. Skip if `--verbose` isn't a CLI flag (grep `src/cli/mod.rs` for `verbose`).

- [ ] **Step 4: Full CI set**

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 5: Commit**

```bash
git add tests/team_object_shape.rs
git commit -m "test(team): cover Atlas Teams object shape end-to-end"
```

---

## Task 3: Spec + plan touch-up if diverged during implementation

- [ ] **Step 1: Re-read the spec**

`docs/specs/team-field-object-shape-tolerance.md` — check all assertions still hold against the final code.

- [ ] **Step 2: Fix any drift**

Examples of drift that matters:
- If the final warning text differs from what the spec says, update the spec.
- If the decision was made during implementation to also extract `name`, update both the spec and "out of scope" sections.

- [ ] **Step 3: Commit (if any drift)**

```bash
git add docs/specs/team-field-object-shape-tolerance.md
git commit -m "docs(spec): reconcile team-field spec with implementation"
```

Skip this task entirely if nothing drifted.

---

## Task 4: Final checks

- [ ] **Step 1: Full CI-equivalent check set**

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All green.

- [ ] **Step 2: Declare done**

Branch ready for multi-agent local review + PR.
