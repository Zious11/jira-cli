# Issue View Standard Fields — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `created`, `updated`, `reporter`, `resolution`, `components`, and `fixVersions` to issue view JSON output, and show `created`, `updated`, `reporter` in the table view.

**Architecture:** Add 3 new serde types (`Resolution`, `Component`, `Version`) and 6 new fields to `IssueFields`. Update the API field lists in both `get_issue()` and `search_issues()`. Add Reporter/Created/Updated rows to the table view. JSON output requires no code change — serde derives handle it.

**Tech Stack:** Rust, serde, reqwest, comfy-table, wiremock

**Spec:** `docs/superpowers/specs/2026-03-26-issue-view-fields-design.md`

---

### Task 1: Add new types and fields to `IssueFields` with tests

**Files:**
- Modify: `src/types/jira/issue.rs:56-72` (add new types before `IssueFields`, add fields to `IssueFields`)

- [ ] **Step 1: Write the failing tests**

In `src/types/jira/issue.rs`, add these tests inside the existing `#[cfg(test)] mod tests` block (after the closing `}` of `issuelinks_empty_array` at line 262, before the module's closing `}` at line 263):

```rust
    #[test]
    fn new_fields_present() {
        let json = json!({
            "summary": "test",
            "created": "2026-03-20T14:32:00.000+0000",
            "updated": "2026-03-25T09:15:22.000+0000",
            "reporter": {"accountId": "abc123", "displayName": "Jane Smith"},
            "resolution": {"name": "Fixed"},
            "components": [{"name": "Backend"}, {"name": "API"}],
            "fixVersions": [{"name": "v2.0", "released": false, "releaseDate": "2026-04-01"}]
        });
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.created.as_deref(), Some("2026-03-20T14:32:00.000+0000"));
        assert_eq!(fields.updated.as_deref(), Some("2026-03-25T09:15:22.000+0000"));
        let reporter = fields.reporter.unwrap();
        assert_eq!(reporter.display_name, "Jane Smith");
        assert_eq!(reporter.account_id, "abc123");
        assert_eq!(fields.resolution.unwrap().name, "Fixed");
        let components = fields.components.unwrap();
        assert_eq!(components.len(), 2);
        assert_eq!(components[0].name, "Backend");
        assert_eq!(components[1].name, "API");
        let versions = fields.fix_versions.unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].name, "v2.0");
        assert_eq!(versions[0].released, Some(false));
        assert_eq!(versions[0].release_date.as_deref(), Some("2026-04-01"));
        // New typed fields should NOT appear in extra
        assert!(!fields.extra.contains_key("created"));
        assert!(!fields.extra.contains_key("updated"));
        assert!(!fields.extra.contains_key("reporter"));
        assert!(!fields.extra.contains_key("resolution"));
        assert!(!fields.extra.contains_key("components"));
        assert!(!fields.extra.contains_key("fixVersions"));
    }

    #[test]
    fn new_fields_absent() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.created.is_none());
        assert!(fields.updated.is_none());
        assert!(fields.reporter.is_none());
        assert!(fields.resolution.is_none());
        assert!(fields.components.is_none());
        assert!(fields.fix_versions.is_none());
    }

    #[test]
    fn new_fields_null() {
        let json = json!({
            "summary": "test",
            "created": null,
            "updated": null,
            "reporter": null,
            "resolution": null,
            "components": null,
            "fixVersions": null
        });
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.created.is_none());
        assert!(fields.updated.is_none());
        assert!(fields.reporter.is_none());
        assert!(fields.resolution.is_none());
        assert!(fields.components.is_none());
        assert!(fields.fix_versions.is_none());
    }

    #[test]
    fn components_empty_array() {
        let json = json!({"summary": "test", "components": []});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.components, Some(vec![]));
    }

    #[test]
    fn fix_versions_empty_array() {
        let json = json!({"summary": "test", "fixVersions": []});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.fix_versions, Some(vec![]));
    }

    #[test]
    fn version_optional_fields_absent() {
        let json = json!({"summary": "test", "fixVersions": [{"name": "v1.0"}]});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        let v = &fields.fix_versions.unwrap()[0];
        assert_eq!(v.name, "v1.0");
        assert!(v.released.is_none());
        assert!(v.release_date.is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib -- jira::issue::tests::new_fields`
Expected: FAIL with "unknown field" or "cannot find type" errors because `Resolution`, `Component`, `Version` types and the 6 new fields on `IssueFields` don't exist yet.

- [ ] **Step 3: Implement the new types and fields**

In `src/types/jira/issue.rs`, add these 3 types after `IssueProject` (after line 107, before `Transition`). Note: `PartialEq` is added beyond the spec's derives — it is required for the `assert_eq!(fields.components, Some(vec![]))` assertions in the empty-array tests:

```rust
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Resolution {
    pub name: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Component {
    pub name: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Version {
    pub name: String,
    pub released: Option<bool>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
}
```

Then modify `IssueFields` (lines 56-72) to add the 6 new fields. Replace the entire struct:

```rust
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct IssueFields {
    pub summary: String,
    pub description: Option<Value>,
    pub status: Option<Status>,
    #[serde(rename = "issuetype")]
    pub issue_type: Option<IssueType>,
    pub priority: Option<Priority>,
    pub assignee: Option<User>,
    pub reporter: Option<User>,
    pub project: Option<IssueProject>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub components: Option<Vec<Component>>,
    #[serde(rename = "fixVersions", default)]
    pub fix_versions: Option<Vec<Version>>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    pub parent: Option<ParentIssue>,
    pub issuelinks: Option<Vec<IssueLink>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib -- jira::issue::tests::new_fields`
Expected: All 6 new tests PASS.

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 6: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: No warnings, no format issues.

- [ ] **Step 7: Commit**

```bash
git add src/types/jira/issue.rs
git commit -m "feat: add Resolution, Component, Version types and new fields to IssueFields (#59)"
```

---

### Task 2: Update API field lists

**Files:**
- Modify: `src/api/jira/issues.rs:96-97` (`get_issue` field string)
- Modify: `src/api/jira/issues.rs:31-39` (`search_issues` field vec)

**TDD note:** The unit tests in Task 1 already verify that `IssueFields` correctly deserializes the 6 new fields. The API field list changes here are purely additive (telling the Jira API to return more fields) — the deserialization is already tested. Integration tests in Task 4 verify end-to-end behavior with wiremock.

- [ ] **Step 1: Update `get_issue()` field string**

In `src/api/jira/issues.rs`, replace lines 96-97:

```rust
        let mut fields =
            "summary,status,issuetype,priority,assignee,project,description,labels,parent,issuelinks".to_string();
```

With:

```rust
        let mut fields =
            "summary,status,issuetype,priority,assignee,reporter,project,description,labels,parent,issuelinks,created,updated,resolution,components,fixVersions".to_string();
```

- [ ] **Step 2: Update `search_issues()` field vec**

In `src/api/jira/issues.rs`, replace lines 31-39:

```rust
        let mut fields = vec![
            "summary",
            "status",
            "issuetype",
            "priority",
            "assignee",
            "project",
            "description",
        ];
```

With:

```rust
        let mut fields = vec![
            "summary",
            "status",
            "issuetype",
            "priority",
            "assignee",
            "reporter",
            "project",
            "description",
            "created",
            "updated",
            "resolution",
            "components",
            "fixVersions",
        ];
```

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 4: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: No warnings, no format issues.

- [ ] **Step 5: Commit**

```bash
git add src/api/jira/issues.rs
git commit -m "feat: add standard fields to get_issue and search_issues API requests (#59)"
```

---

### Task 3: Add Reporter, Created, Updated to table view

**Files:**
- Modify: `src/cli/issue/list.rs:441-499` (`handle_view` table rows)

**TDD note:** `handle_view` is async and prints to stdout via `println!`, making direct unit testing impractical without stdout capture. The table row construction uses the same typed fields tested in Task 1, and `format_comment_date` is already tested in existing unit tests (lines 614-631 of `list.rs`). End-to-end table verification is done via live testing in Task 5.

- [ ] **Step 1: Restructure the `rows` vec in `handle_view`**

In `src/cli/issue/list.rs`, replace lines 441-499 (the initial `let mut rows = vec![...]` block through the closing `];`):

```rust
            let mut rows = vec![
                vec!["Key".into(), issue.key.clone()],
                vec!["Summary".into(), issue.fields.summary.clone()],
                vec![
                    "Type".into(),
                    issue
                        .fields
                        .issue_type
                        .as_ref()
                        .map(|t| t.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Status".into(),
                    issue
                        .fields
                        .status
                        .as_ref()
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Priority".into(),
                    issue
                        .fields
                        .priority
                        .as_ref()
                        .map(|p| p.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Assignee".into(),
                    issue
                        .fields
                        .assignee
                        .as_ref()
                        .map(|a| a.display_name.clone())
                        .unwrap_or_else(|| "Unassigned".into()),
                ],
                vec![
                    "Reporter".into(),
                    issue
                        .fields
                        .reporter
                        .as_ref()
                        .map(|r| r.display_name.clone())
                        .unwrap_or_else(|| "(none)".into()),
                ],
                vec![
                    "Created".into(),
                    issue
                        .fields
                        .created
                        .as_deref()
                        .map(format_comment_date)
                        .unwrap_or_default(),
                ],
                vec![
                    "Updated".into(),
                    issue
                        .fields
                        .updated
                        .as_deref()
                        .map(format_comment_date)
                        .unwrap_or_default(),
                ],
                vec![
                    "Project".into(),
                    issue
                        .fields
                        .project
                        .as_ref()
                        .map(|p| format!("{} ({})", p.name.as_deref().unwrap_or(""), p.key))
                        .unwrap_or_default(),
                ],
                vec![
                    "Labels".into(),
                    issue
                        .fields
                        .labels
                        .as_ref()
                        .filter(|l| !l.is_empty())
                        .map(|l| l.join(", "))
                        .unwrap_or_else(|| "(none)".into()),
                ],
            ];
```

This inserts Reporter, Created, Updated between Assignee and Project. The code after line 499 (Parent push, Links push, Assets, Points, Description) remains unchanged.

- [ ] **Step 2: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 3: Run clippy and format**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: No warnings, no format issues.

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "feat: display reporter, created, updated in issue view table (#59)"
```

---

### Task 4: Add integration test for new fields

**Files:**
- Modify: `tests/issue_commands.rs` (add test at end of file)
- Modify: `tests/common/fixtures.rs` (add fixture helper)

- [ ] **Step 1: Add fixture helper for issue with all fields**

In `tests/common/fixtures.rs`, add this function at the end of the file (after `project_response`):

```rust
pub fn issue_response_with_standard_fields(key: &str, summary: &str) -> Value {
    json!({
        "key": key,
        "fields": {
            "summary": summary,
            "status": {"name": "In Progress", "statusCategory": {"name": "In Progress", "key": "indeterminate"}},
            "issuetype": {"name": "Bug"},
            "priority": {"name": "High"},
            "assignee": {"accountId": "abc123", "displayName": "John Doe"},
            "reporter": {"accountId": "def456", "displayName": "Jane Smith"},
            "project": {"key": key.split('-').next().unwrap_or("TEST"), "name": "Test Project"},
            "created": "2026-03-20T14:32:00.000+0000",
            "updated": "2026-03-25T09:15:22.000+0000",
            "resolution": {"name": "Fixed"},
            "components": [{"name": "Backend"}, {"name": "API"}],
            "fixVersions": [{"name": "v2.0", "released": false, "releaseDate": "2026-04-01"}],
            "labels": ["bug"],
            "parent": null,
            "issuelinks": []
        }
    })
}
```

- [ ] **Step 2: Write the integration test**

In `tests/issue_commands.rs`, add this test at the end of the file:

```rust
#[tokio::test]
async fn get_issue_includes_standard_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-42"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_response_with_standard_fields(
                    "FOO-42",
                    "Test with all fields",
                )),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issue = client.get_issue("FOO-42", &[]).await.unwrap();

    // Verify new fields are deserialized
    assert_eq!(
        issue.fields.created.as_deref(),
        Some("2026-03-20T14:32:00.000+0000")
    );
    assert_eq!(
        issue.fields.updated.as_deref(),
        Some("2026-03-25T09:15:22.000+0000")
    );

    let reporter = issue.fields.reporter.as_ref().unwrap();
    assert_eq!(reporter.display_name, "Jane Smith");
    assert_eq!(reporter.account_id, "def456");

    assert_eq!(issue.fields.resolution.as_ref().unwrap().name, "Fixed");

    let components = issue.fields.components.as_ref().unwrap();
    assert_eq!(components.len(), 2);
    assert_eq!(components[0].name, "Backend");
    assert_eq!(components[1].name, "API");

    let versions = issue.fields.fix_versions.as_ref().unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].name, "v2.0");
    assert_eq!(versions[0].released, Some(false));
    assert_eq!(versions[0].release_date.as_deref(), Some("2026-04-01"));

    // Verify JSON serialization includes the new fields
    let json_str = serde_json::to_string(&issue).unwrap();
    assert!(json_str.contains("\"created\""));
    assert!(json_str.contains("\"reporter\""));
    assert!(json_str.contains("\"resolution\""));
    assert!(json_str.contains("\"components\""));
    assert!(json_str.contains("\"fixVersions\""));
}

#[tokio::test]
async fn get_issue_null_standard_fields() {
    let server = MockServer::start().await;

    // Issue with all new fields null/absent
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-43"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-43",
                "Minimal issue",
                "To Do",
            )),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issue = client.get_issue("FOO-43", &[]).await.unwrap();

    // All new fields should be None (the fixture doesn't include them)
    assert!(issue.fields.created.is_none());
    assert!(issue.fields.updated.is_none());
    assert!(issue.fields.reporter.is_none());
    assert!(issue.fields.resolution.is_none());
    assert!(issue.fields.components.is_none());
    assert!(issue.fields.fix_versions.is_none());
}
```

- [ ] **Step 3: Run the new tests**

Run: `cargo test --test issue_commands -- get_issue_includes_standard_fields && cargo test --test issue_commands -- get_issue_null_standard_fields`
Expected: Both PASS.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/issue_commands.rs tests/common/fixtures.rs
git commit -m "test: add integration tests for standard issue fields (#59)"
```

---

### Task 5: Final verification

**Files:**
- All modified files from Tasks 1-4

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Zero warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt --all -- --check`
Expected: No format issues.
