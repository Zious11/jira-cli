# Story Points Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add full CRUD for story points with configurable custom field ID, display in view/list, and sprint point summaries.

**Architecture:** Story points are a Jira custom field (varies per instance). Discovery via `/rest/api/3/field` during `jr init`, persisted to config. Runtime access via `#[serde(flatten)]` on `IssueFields` with a `story_points(field_id)` helper. API functions gain `extra_fields` parameter for dynamic custom field inclusion.

**Tech Stack:** Rust, clap (CLI), serde (JSON), reqwest (HTTP), wiremock (tests), comfy-table (output)

**Spec:** `docs/superpowers/specs/2026-03-22-story-points-design.md`

---

### Task 1: Add `story_points_field_id` to Config

**Files:**
- Modify: `src/config.rs:9-11` (FieldsConfig struct)
- Modify: `src/config.rs:140-300` (existing tests — ensure no regressions)

- [ ] **Step 1: Add the field to FieldsConfig**

In `src/config.rs`, add `story_points_field_id` to `FieldsConfig`:

```rust
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct FieldsConfig {
    pub team_field_id: Option<String>,
    pub story_points_field_id: Option<String>,
}
```

- [ ] **Step 2: Run existing tests**

Run: `cargo test --lib config`
Expected: All existing config tests pass (the new field defaults to `None`)

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "feat: add story_points_field_id to FieldsConfig"
```

---

### Task 2: Add `FieldSchema` and `find_story_points_field_id()`

**Files:**
- Modify: `src/api/jira/fields.rs:1-25` (Field struct + new FieldSchema + new method)

- [ ] **Step 1: Write unit tests for discovery filtering**

Add inline tests at the bottom of `src/api/jira/fields.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_field(id: &str, name: &str, custom: bool, schema_type: &str, schema_custom: &str) -> Field {
        Field {
            id: id.to_string(),
            name: name.to_string(),
            custom: Some(custom),
            schema: Some(FieldSchema {
                field_type: schema_type.to_string(),
                custom: Some(schema_custom.to_string()),
            }),
        }
    }

    #[test]
    fn filter_finds_classic_story_points() {
        let fields = vec![
            make_field("customfield_10031", "Story Points", true, "number", "com.atlassian.jira.plugin.system.customfieldtypes:float"),
            make_field("customfield_10042", "Task progress", true, "number", "com.atlassian.jira.plugin.system.customfieldtypes:float"),
        ];
        let result = filter_story_points_fields(&fields);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "customfield_10031");
    }

    #[test]
    fn filter_finds_jsw_story_point_estimate() {
        let fields = vec![
            make_field("customfield_10016", "Story point estimate", true, "number", "com.pyxis.greenhopper.jira:jsw-story-points"),
        ];
        let result = filter_story_points_fields(&fields);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "customfield_10016");
    }

    #[test]
    fn filter_finds_both_variants() {
        let fields = vec![
            make_field("customfield_10031", "Story Points", true, "number", "com.atlassian.jira.plugin.system.customfieldtypes:float"),
            make_field("customfield_10016", "Story point estimate", true, "number", "com.pyxis.greenhopper.jira:jsw-story-points"),
        ];
        let result = filter_story_points_fields(&fields);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_ignores_non_custom_fields() {
        let fields = vec![
            Field {
                id: "timeestimate".to_string(),
                name: "Remaining Estimate".to_string(),
                custom: Some(false),
                schema: Some(FieldSchema {
                    field_type: "number".to_string(),
                    custom: None,
                }),
            },
        ];
        let result = filter_story_points_fields(&fields);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_ignores_non_number_fields() {
        let fields = vec![
            make_field("customfield_10099", "Story Points", true, "string", "com.atlassian.jira.plugin.system.customfieldtypes:textfield"),
        ];
        let result = filter_story_points_fields(&fields);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_case_insensitive_name_match() {
        let fields = vec![
            make_field("customfield_10031", "STORY POINTS", true, "number", "com.atlassian.jira.plugin.system.customfieldtypes:float"),
        ];
        let result = filter_story_points_fields(&fields);
        assert_eq!(result.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib fields`
Expected: FAIL — `FieldSchema`, `filter_story_points_fields` not defined

- [ ] **Step 3: Update Field struct and add FieldSchema**

Replace the `Field` struct and add `FieldSchema` in `src/api/jira/fields.rs`:

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::api::client::JiraClient;

#[derive(Debug, Deserialize, Serialize)]
pub struct Field {
    pub id: String,
    pub name: String,
    pub custom: Option<bool>,
    pub schema: Option<FieldSchema>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FieldSchema {
    #[serde(rename = "type")]
    pub field_type: String,
    pub custom: Option<String>,
}
```

- [ ] **Step 4: Add `filter_story_points_fields()` function**

Add below the `JiraClient` impl block:

```rust
/// Known schema.custom types for story points fields.
const KNOWN_SP_SCHEMA_TYPES: &[&str] = &[
    "com.atlassian.jira.plugin.system.customfieldtypes:float",
    "com.pyxis.greenhopper.jira:jsw-story-points",
];

/// Filter a field list to find story points candidates.
/// Returns vec of (field_id, field_name).
pub fn filter_story_points_fields(fields: &[Field]) -> Vec<(String, String)> {
    let known_names: &[&str] = &["story points", "story point estimate"];

    fields
        .iter()
        .filter(|f| {
            let is_custom = f.custom == Some(true);
            let is_number = f
                .schema
                .as_ref()
                .map(|s| s.field_type == "number")
                .unwrap_or(false);
            let name_matches = known_names
                .iter()
                .any(|n| f.name.to_lowercase() == *n);
            is_custom && is_number && name_matches
        })
        .collect::<Vec<_>>()
        .into_iter()
        // Sort: prefer known schema.custom types first
        .map(|f| {
            let has_known_schema = f
                .schema
                .as_ref()
                .and_then(|s| s.custom.as_deref())
                .map(|c| KNOWN_SP_SCHEMA_TYPES.contains(&c))
                .unwrap_or(false);
            (f, has_known_schema)
        })
        .sorted_by(|a, b| b.1.cmp(&a.1))
        .map(|(f, _)| (f.id.clone(), f.name.clone()))
        .collect()
}
```

Wait — `sorted_by` requires `itertools`. Simpler approach without extra dependency:

```rust
pub fn filter_story_points_fields(fields: &[Field]) -> Vec<(String, String)> {
    let known_names: &[&str] = &["story points", "story point estimate"];

    let mut matches: Vec<(String, String, bool)> = fields
        .iter()
        .filter(|f| {
            let is_custom = f.custom == Some(true);
            let is_number = f
                .schema
                .as_ref()
                .map(|s| s.field_type == "number")
                .unwrap_or(false);
            let name_matches = known_names
                .iter()
                .any(|n| f.name.to_lowercase() == *n);
            is_custom && is_number && name_matches
        })
        .map(|f| {
            let has_known_schema = f
                .schema
                .as_ref()
                .and_then(|s| s.custom.as_deref())
                .map(|c| KNOWN_SP_SCHEMA_TYPES.contains(&c))
                .unwrap_or(false);
            (f.id.clone(), f.name.clone(), has_known_schema)
        })
        .collect();

    // Sort: known schema types first
    matches.sort_by(|a, b| b.2.cmp(&a.2));
    matches.into_iter().map(|(id, name, _)| (id, name)).collect()
}
```

- [ ] **Step 5: Add `find_story_points_field_id()` to JiraClient impl**

Add to the existing `impl JiraClient` block:

```rust
    pub async fn find_story_points_field_id(&self) -> Result<Vec<(String, String)>> {
        let fields = self.list_fields().await?;
        Ok(filter_story_points_fields(&fields))
    }
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib fields`
Expected: All 6 new tests pass

- [ ] **Step 7: Run full test suite for regressions**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add src/api/jira/fields.rs
git commit -m "feat: add FieldSchema and story points field discovery"
```

---

### Task 3: Add `#[serde(flatten)]` to `IssueFields` and `story_points()` helper

**Files:**
- Modify: `src/types/jira/issue.rs:1-24` (IssueFields struct)

- [ ] **Step 1: Write unit tests**

Add inline tests at the bottom of `src/types/jira/issue.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn story_points_present() {
        let json = json!({
            "summary": "test",
            "customfield_10031": 5.0
        });
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), Some(5.0));
    }

    #[test]
    fn story_points_missing() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), None);
    }

    #[test]
    fn story_points_null() {
        let json = json!({"summary": "test", "customfield_10031": null});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), None);
    }

    #[test]
    fn story_points_wrong_type() {
        let json = json!({"summary": "test", "customfield_10031": "not a number"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), None);
    }

    #[test]
    fn story_points_decimal() {
        let json = json!({"summary": "test", "customfield_10031": 3.5});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), Some(3.5));
    }

    #[test]
    fn story_points_integer_value() {
        let json = json!({"summary": "test", "customfield_10031": 13});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), Some(13.0));
    }

    #[test]
    fn flatten_does_not_break_labels_null() {
        let json = json!({"summary": "test", "labels": null});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.labels, None);
    }

    #[test]
    fn flatten_does_not_break_labels_present() {
        let json = json!({"summary": "test", "labels": ["bug", "frontend"]});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.labels, Some(vec!["bug".to_string(), "frontend".to_string()]));
    }

    #[test]
    fn flatten_does_not_break_labels_missing() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.labels, None);
    }

    #[test]
    fn flatten_does_not_break_description_null() {
        let json = json!({"summary": "test", "description": null});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.description.is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib issue`
Expected: FAIL — `story_points` method not defined, `extra` field not present

- [ ] **Step 3: Add `flatten` and `story_points()` helper**

In `src/types/jira/issue.rs`, add the import at the top:

```rust
use std::collections::HashMap;
```

Add the `extra` field to `IssueFields` after `labels`:

```rust
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
```

Add an `impl` block after the struct:

```rust
impl IssueFields {
    pub fn story_points(&self, field_id: &str) -> Option<f64> {
        self.extra.get(field_id)?.as_f64()
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib issue`
Expected: All 10 new tests pass

- [ ] **Step 5: Run full test suite for serde regressions**

Run: `cargo test`
Expected: All tests pass (including existing integration tests)

- [ ] **Step 6: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 7: Commit**

```bash
git add src/types/jira/issue.rs
git commit -m "feat: add serde flatten for custom fields and story_points() helper"
```

---

### Task 4: Add `extra_fields` parameter to API functions

**Files:**
- Modify: `src/api/jira/issues.rs:9-55` (search_issues, get_issue)
- Modify: `src/api/jira/sprints.rs:36-61` (get_sprint_issues)
- Modify: `src/cli/issue.rs:246` (search_issues call site)
- Modify: `src/cli/board.rs:53,64` (both call sites)
- Modify: `src/cli/sprint.rs:77` (get_sprint_issues call site)
- Modify: `tests/issue_commands.rs:25` (test call site)

- [ ] **Step 1: Update `search_issues()` signature and implementation**

In `src/api/jira/issues.rs`, change `search_issues`:

```rust
    pub async fn search_issues(&self, jql: &str, limit: Option<u32>, extra_fields: &[&str]) -> Result<Vec<Issue>> {
        let max_per_page = limit.unwrap_or(50).min(100);
        let mut all_issues: Vec<Issue> = Vec::new();
        let mut next_page_token: Option<String> = None;

        let mut fields = vec![
            "summary", "status", "issuetype", "priority", "assignee", "project", "description",
        ];
        fields.extend_from_slice(extra_fields);

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": max_per_page,
                "fields": fields
            });
```

The rest of the function body stays the same.

- [ ] **Step 2: Update `get_issue()` to accept extra fields**

In `src/api/jira/issues.rs`, change `get_issue`:

```rust
    pub async fn get_issue(&self, key: &str, extra_fields: &[&str]) -> Result<Issue> {
        let mut fields = "summary,status,issuetype,priority,assignee,project,description,labels".to_string();
        for f in extra_fields {
            fields.push(',');
            fields.push_str(f);
        }
        let path = format!(
            "/rest/api/3/issue/{}?fields={}",
            urlencoding::encode(key),
            fields
        );
        self.get(&path).await
    }
```

- [ ] **Step 3: Update `get_sprint_issues()` to accept extra fields**

In `src/api/jira/sprints.rs`, change `get_sprint_issues`:

```rust
    pub async fn get_sprint_issues(&self, sprint_id: u64, jql: Option<&str>, extra_fields: &[&str]) -> Result<Vec<Issue>> {
        let mut all_issues: Vec<Issue> = Vec::new();
        let mut start_at: u32 = 0;
        let max_results: u32 = 50;

        loop {
            let mut path = format!(
                "/rest/agile/1.0/sprint/{}/issue?startAt={}&maxResults={}",
                sprint_id, start_at, max_results
            );
            // Always send fields parameter to keep serde(flatten) extra map small
            let mut fields = "summary,status,issuetype,priority,assignee,project".to_string();
            for f in extra_fields {
                fields.push(',');
                fields.push_str(f);
            }
            path.push_str(&format!("&fields={}", fields));
            if let Some(q) = jql {
                path.push_str(&format!("&jql={}", urlencoding::encode(q)));
            }
```

The rest of the function body stays the same.

- [ ] **Step 4: Update all call sites to pass `&[]`**

In `src/cli/issue.rs` line 246:
```rust
    let issues = client.search_issues(&effective_jql, limit, &[]).await?;
```

In `src/cli/board.rs` line 53:
```rust
        client.get_sprint_issues(sprint.id, None, &[]).await?
```

In `src/cli/board.rs` line 64:
```rust
        client.search_issues(&jql, None, &[]).await?
```

In `src/cli/sprint.rs` line 77:
```rust
    let issues = client.get_sprint_issues(sprint.id, None, &[]).await?;
```

In `src/cli/issue.rs`, all `client.get_issue(key)` calls must become `client.get_issue(key, &[])`. Search for `.get_issue(` in the file and update each one:
- `handle_view`: `client.get_issue(key, &[]).await?`
- `handle_move`: `client.get_issue(key, &[]).await?`
- `handle_assign`: `client.get_issue(key, &[]).await?`

In `tests/issue_commands.rs` line 25:
```rust
        .search_issues("assignee = currentUser()", None, &[])
```

In `tests/issue_commands.rs` line 49:
```rust
    let issue = client.get_issue("FOO-1", &[]).await.unwrap();
```

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 6: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 7: Commit**

```bash
git add src/api/jira/issues.rs src/api/jira/sprints.rs src/cli/issue.rs src/cli/board.rs src/cli/sprint.rs tests/issue_commands.rs
git commit -m "refactor: add extra_fields parameter to search_issues and get_sprint_issues"
```

---

### Task 5: Add `format_points()` helper and `--points` CLI flags

**Files:**
- Modify: `src/cli/issue.rs` (add format_points, resolve_story_points_field_id)
- Modify: `src/cli/mod.rs:108-226` (IssueCommand enum)

- [ ] **Step 1: Write unit tests for `format_points()`**

Add to the existing `#[cfg(test)] mod tests` block in `src/cli/issue.rs`:

```rust
    #[test]
    fn format_points_whole_number() {
        assert_eq!(format_points(5.0), "5");
        assert_eq!(format_points(13.0), "13");
        assert_eq!(format_points(0.0), "0");
    }

    #[test]
    fn format_points_decimal() {
        assert_eq!(format_points(3.5), "3.5");
        assert_eq!(format_points(0.5), "0.5");
    }

    #[test]
    fn format_points_non_finite() {
        assert_eq!(format_points(f64::NAN), "-");
        assert_eq!(format_points(f64::INFINITY), "-");
        assert_eq!(format_points(f64::NEG_INFINITY), "-");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib issue`
Expected: FAIL — `format_points` not defined

- [ ] **Step 3: Add `format_points()` function**

In `src/cli/issue.rs`, add above the `prompt_input` helper. Make it `pub` from the start since `sprint.rs` will use it:

```rust
pub fn format_points(value: f64) -> String {
    if !value.is_finite() {
        return "-".to_string();
    }
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}
```

- [ ] **Step 4: Add `resolve_story_points_field_id()` helper**

In `src/cli/issue.rs`, add above `prompt_input`:

```rust
fn resolve_story_points_field_id(config: &Config) -> Result<String> {
    config
        .global
        .fields
        .story_points_field_id
        .clone()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Story points field not configured. Run \"jr init\" or set story_points_field_id under [fields] in ~/.config/jr/config.toml"
            )
        })
}
```

- [ ] **Step 5: Add `--points` flags to CLI enums**

In `src/cli/mod.rs`, add to `IssueCommand::Create`:

```rust
        /// Story points
        #[arg(long)]
        points: Option<f64>,
```

Add to `IssueCommand::Edit`:

```rust
        /// Story points
        #[arg(long, conflicts_with = "no_points")]
        points: Option<f64>,
        /// Clear story points
        #[arg(long, conflicts_with = "points")]
        no_points: bool,
```

Add to `IssueCommand::List`:

```rust
        /// Show story points column
        #[arg(long)]
        points: bool,
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib issue`
Expected: All format_points tests pass

- [ ] **Step 7: Run clippy and fix any warnings**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: May warn about unused fields in enum variants — that's fine, they'll be wired in the next tasks

- [ ] **Step 8: Commit**

```bash
git add src/cli/issue.rs src/cli/mod.rs
git commit -m "feat: add format_points helper, resolve_story_points_field_id, and --points CLI flags"
```

---

### Task 6: Wire story points into `issue view`

**Files:**
- Modify: `src/cli/issue.rs` (handle_view function, ~line 280-361)

- [ ] **Step 1: Update `handle_view` to accept config and show story points**

Change `handle_view` signature to accept `config`:

```rust
async fn handle_view(key: &str, output_format: &OutputFormat, config: &Config, client: &JiraClient) -> Result<()> {
```

Update the call site in `handle()` match arm (around line 79):

```rust
        IssueCommand::View { key } => handle_view(&key, output_format, config, client).await,
```

Inside `handle_view`, resolve the story points field ID (optional — don't error if not configured):

```rust
    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let extra: Vec<&str> = sp_field_id.iter().copied().collect();
    let issue = client.get_issue(key, &extra).await?;
```

Add the Points row to the table output, after the Labels row:

```rust
                // Story points row (only if field is configured)
                if let Some(ref field_id) = sp_field_id {
                    let points_display = issue
                        .fields
                        .story_points(field_id)
                        .map(format_points)
                        .unwrap_or_else(|| "(none)".into());
                    rows.push(vec!["Points".into(), points_display]);
                }
```

Insert this before the Description row.

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/cli/issue.rs
git commit -m "feat: show story points in issue view"
```

---

### Task 7: Wire story points into `issue create`

**Files:**
- Modify: `src/cli/issue.rs` (handle_create function, ~line 366-468)
- Modify: `src/cli/issue.rs` (handle() match arm, ~line 81-109)

- [ ] **Step 1: Thread `points` through handle() to handle_create**

Update the `IssueCommand::Create` match arm to include `points`:

```rust
        IssueCommand::Create {
            project,
            issue_type,
            summary,
            description,
            description_stdin,
            priority,
            label,
            team,
            markdown,
            points,
        } => {
```

Add `points` parameter to `handle_create` signature:

```rust
async fn handle_create(
    ...
    team: Option<String>,
    points: Option<f64>,
    markdown: bool,
    ...
```

- [ ] **Step 2: Add story points to create payload**

In `handle_create`, after the team assignment block:

```rust
    if let Some(pts) = points {
        let field_id = resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(pts);
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue.rs
git commit -m "feat: support --points on issue create"
```

---

### Task 8: Wire story points into `issue edit`

**Files:**
- Modify: `src/cli/issue.rs` (handle_edit function, ~line 472-570)
- Modify: `src/cli/issue.rs` (handle() match arm, ~line 111-132)

- [ ] **Step 1: Thread `points` and `no_points` through handle() to handle_edit**

Update the `IssueCommand::Edit` match arm:

```rust
        IssueCommand::Edit {
            key,
            summary,
            issue_type,
            priority,
            label,
            team,
            points,
            no_points,
        } => {
            handle_edit(
                &key,
                summary,
                issue_type,
                priority,
                label,
                team,
                points,
                no_points,
                ...
```

Add `points: Option<f64>` and `no_points: bool` to `handle_edit` signature.

- [ ] **Step 2: Add story points to edit payload**

In `handle_edit`, after the team assignment block:

```rust
    if let Some(pts) = points {
        let field_id = resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(pts);
        has_updates = true;
    }

    if no_points {
        let field_id = resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(null);
        has_updates = true;
    }
```

- [ ] **Step 3: Update error message**

Change the "no fields specified" error:

```rust
    if !has_updates {
        bail!(
            "No fields specified to update. Use --summary, --type, --priority, --label, --team, --points, or --no-points."
        );
    }
```

Note: If Jira returns a 400 error with a message about the field not being on the edit screen (e.g., `"Field 'customfield_XXXXX' cannot be set"`), the existing `JrError` propagation will surface it to the user. No special handling is needed — Jira's error message is already descriptive enough. If the error is observed to be unclear in practice, a follow-up can add field-specific error mapping.

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue.rs
git commit -m "feat: support --points and --no-points on issue edit"
```

---

### Task 9: Wire story points into `issue list`

**Files:**
- Modify: `src/cli/issue.rs` (handle_list function, format_issue_rows_public)
- Modify: `src/cli/issue.rs` (handle() match arm)

- [ ] **Step 1: Thread `points` flag through handle() to handle_list**

Update the `IssueCommand::List` match arm to include `points`:

```rust
        IssueCommand::List {
            jql,
            status,
            team,
            limit,
            points,
        } => {
            handle_list(
                jql,
                status,
                team,
                limit,
                points,
                ...
```

Add `show_points: bool` parameter to `handle_list` signature.

- [ ] **Step 2: Pass story points field to search_issues**

In `handle_list`, resolve the field ID (optional) and pass to search:

```rust
    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let extra: Vec<&str> = sp_field_id.iter().copied().collect();
    // ... later:
    let issues = client.search_issues(&effective_jql, limit, &extra).await?;
```

Replace all `client.search_issues(..., &[])` calls in `handle_list` with `client.search_issues(..., &extra)`.

- [ ] **Step 3: Add Points column when `--points` flag is set**

Replace the format_issue_rows_public call and table rendering with conditional logic:

```rust
    if show_points && sp_field_id.is_some() {
        let field_id = sp_field_id.unwrap();
        let rows: Vec<Vec<String>> = issues
            .iter()
            .map(|issue| {
                let pts = issue
                    .fields
                    .story_points(field_id)
                    .map(format_points)
                    .unwrap_or_else(|| "-".into());
                vec![
                    issue.key.clone(),
                    issue.fields.issue_type.as_ref().map(|t| t.name.clone()).unwrap_or_default(),
                    issue.fields.status.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
                    issue.fields.priority.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                    pts,
                    issue.fields.assignee.as_ref().map(|a| a.display_name.clone()).unwrap_or_else(|| "Unassigned".into()),
                    issue.fields.summary.clone(),
                ]
            })
            .collect();

        output::print_output(
            output_format,
            &["Key", "Type", "Status", "Priority", "Points", "Assignee", "Summary"],
            &rows,
            &issues,
        )?;
    } else {
        let rows = format_issue_rows_public(&issues);
        output::print_output(
            output_format,
            &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
            &rows,
            &issues,
        )?;
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue.rs
git commit -m "feat: support --points column in issue list"
```

---

### Task 10: Sprint points summary

**Files:**
- Modify: `src/cli/sprint.rs` (handle_current function)

- [ ] **Step 1: Thread `config` into `handle_current`**

`handle()` already receives `config: &Config`. Thread it into `handle_current`:

```rust
async fn handle_current(
    board_id: u64,
    client: &JiraClient,
    output_format: &OutputFormat,
    config: &Config,
) -> Result<()> {
```

Update the call in `handle()`:

```rust
        SprintCommand::Current => handle_current(board_id, client, output_format, config).await,
```

- [ ] **Step 2: Fetch sprint issues with story points field**

```rust
    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let extra: Vec<&str> = sp_field_id.iter().copied().collect();
    let issues = client.get_sprint_issues(sprint.id, None, &extra).await?;
```

- [ ] **Step 3: Add `compute_sprint_summary` function with unit tests**

Extract the sprint computation into a testable `pub` function in `src/cli/sprint.rs`:

```rust
use crate::types::jira::Issue;

/// Compute sprint points summary: (total_points, completed_points, unestimated_count)
pub fn compute_sprint_summary(issues: &[Issue], field_id: &str) -> (f64, f64, u32) {
    let mut total_points: f64 = 0.0;
    let mut completed_points: f64 = 0.0;
    let mut unestimated_count: u32 = 0;

    for issue in issues {
        match issue.fields.story_points(field_id) {
            Some(pts) => {
                total_points += pts;
                let is_done = issue
                    .fields
                    .status
                    .as_ref()
                    .and_then(|s| s.status_category.as_ref())
                    .map(|c| c.key == "done")
                    .unwrap_or(false);
                if is_done {
                    completed_points += pts;
                }
            }
            None => {
                unestimated_count += 1;
            }
        }
    }

    (total_points, completed_points, unestimated_count)
}
```

Add inline unit tests at the bottom of `src/cli/sprint.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::jira::{Issue, IssueFields, Status, StatusCategory};
    use std::collections::HashMap;

    fn make_issue(key: &str, status_cat_key: &str, points: Option<f64>) -> Issue {
        let mut extra = HashMap::new();
        if let Some(pts) = points {
            extra.insert("customfield_10031".to_string(), serde_json::json!(pts));
        }
        Issue {
            key: key.to_string(),
            fields: IssueFields {
                summary: "test".to_string(),
                status: Some(Status {
                    name: "status".to_string(),
                    status_category: Some(StatusCategory {
                        name: "cat".to_string(),
                        key: status_cat_key.to_string(),
                    }),
                }),
                extra,
                ..Default::default()
            },
        }
    }

    #[test]
    fn sprint_summary_mixed_issues() {
        let issues = vec![
            make_issue("FOO-1", "done", Some(5.0)),
            make_issue("FOO-2", "indeterminate", Some(3.0)),
            make_issue("FOO-3", "new", None),
        ];
        let (total, completed, unestimated) = compute_sprint_summary(&issues, "customfield_10031");
        assert_eq!(total, 8.0);
        assert_eq!(completed, 5.0);
        assert_eq!(unestimated, 1);
    }

    #[test]
    fn sprint_summary_all_done() {
        let issues = vec![
            make_issue("FOO-1", "done", Some(5.0)),
            make_issue("FOO-2", "done", Some(3.0)),
        ];
        let (total, completed, unestimated) = compute_sprint_summary(&issues, "customfield_10031");
        assert_eq!(total, 8.0);
        assert_eq!(completed, 8.0);
        assert_eq!(unestimated, 0);
    }

    #[test]
    fn sprint_summary_no_points() {
        let issues = vec![
            make_issue("FOO-1", "new", None),
            make_issue("FOO-2", "new", None),
        ];
        let (total, completed, unestimated) = compute_sprint_summary(&issues, "customfield_10031");
        assert_eq!(total, 0.0);
        assert_eq!(completed, 0.0);
        assert_eq!(unestimated, 2);
    }

    #[test]
    fn sprint_summary_empty() {
        let (total, completed, unestimated) = compute_sprint_summary(&[], "customfield_10031");
        assert_eq!(total, 0.0);
        assert_eq!(completed, 0.0);
        assert_eq!(unestimated, 0);
    }
}
```

Then in `handle_current`, call the extracted function:

```rust
    let sprint_summary = sp_field_id.map(|field_id| compute_sprint_summary(&issues, field_id));
```

- [ ] **Step 4: Update table output with Points column and summary line**

Replace the existing `OutputFormat::Table` arm:

```rust
        OutputFormat::Table => {
            eprintln!(
                "Sprint: {} (ends {})",
                sprint.name,
                sprint.end_date.as_deref().unwrap_or("N/A")
            );

            if let Some((total, completed, unestimated)) = sprint_summary {
                let mut summary_line = format!(
                    "Points: {}/{} completed",
                    super::issue::format_points(completed),
                    super::issue::format_points(total),
                );
                if unestimated > 0 {
                    summary_line.push_str(&format!("  ({} unestimated)", unestimated));
                }
                eprintln!("{}", summary_line);
            }

            eprintln!();

            if sp_field_id.is_some() {
                let field_id = sp_field_id.unwrap();
                let rows: Vec<Vec<String>> = issues
                    .iter()
                    .map(|issue| {
                        let pts = issue
                            .fields
                            .story_points(field_id)
                            .map(super::issue::format_points)
                            .unwrap_or_else(|| "-".into());
                        vec![
                            issue.key.clone(),
                            issue.fields.issue_type.as_ref().map(|t| t.name.clone()).unwrap_or_default(),
                            issue.fields.status.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
                            issue.fields.priority.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                            pts,
                            issue.fields.assignee.as_ref().map(|a| a.display_name.clone()).unwrap_or_else(|| "Unassigned".into()),
                            issue.fields.summary.clone(),
                        ]
                    })
                    .collect();

                output::print_output(
                    output_format,
                    &["Key", "Type", "Status", "Priority", "Points", "Assignee", "Summary"],
                    &rows,
                    &issues,
                )?;
            } else {
                let rows = super::issue::format_issue_rows_public(&issues);
                output::print_output(
                    output_format,
                    &["Key", "Type", "Status", "Priority", "Assignee", "Summary"],
                    &rows,
                    &issues,
                )?;
            }
        }
```

- [ ] **Step 5: Update JSON output with sprint_summary**

Replace the existing `OutputFormat::Json` arm:

```rust
        OutputFormat::Json => {
            let mut data = serde_json::json!({
                "sprint": sprint,
                "issues": issues,
            });
            if let Some((total, completed, unestimated)) = sprint_summary {
                data["sprint_summary"] = serde_json::json!({
                    "completed_points": completed,
                    "total_points": total,
                    "unestimated_count": unestimated,
                });
            }
            println!("{}", output::render_json(&data)?);
        }
```

Note: `format_points` was already declared `pub` in Task 5. Sprint code references it as `super::issue::format_points()`.

- [ ] **Step 7: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 8: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 9: Commit**

```bash
git add src/cli/sprint.rs src/cli/issue.rs
git commit -m "feat: add story points column and summary to sprint current"
```

---

### Task 11: Add story points discovery to `jr init`

**Files:**
- Modify: `src/cli/init.rs:89-118` (after team field discovery)

- [ ] **Step 1: Add story points field discovery after team field discovery**

In `src/cli/init.rs`, after the Step 5 block (team field discovery, line 90-94), add:

```rust
    // Step 5b: Discover story points field
    match client.find_story_points_field_id().await {
        Ok(matches) => {
            let field_id = match matches.len() {
                0 => {
                    eprintln!("No story points field found — skipping. You can set story_points_field_id manually in config.");
                    None
                }
                1 => {
                    eprintln!("Found story points field: {} ({})", matches[0].1, matches[0].0);
                    Some(matches[0].0.clone())
                }
                _ => {
                    let names: Vec<String> = matches
                        .iter()
                        .map(|(id, name)| format!("{} ({})", name, id))
                        .collect();
                    let selection = Select::new()
                        .with_prompt("Multiple story points fields found. Select one")
                        .items(&names)
                        .interact()?;
                    Some(matches[selection].0.clone())
                }
            };

            if let Some(id) = field_id {
                let mut config = Config::load()?;
                config.global.fields.story_points_field_id = Some(id);
                config.save_global()?;
            }
        }
        Err(e) => {
            eprintln!("Could not discover story points field: {}. Skipping.", e);
        }
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/cli/init.rs
git commit -m "feat: discover story points field during jr init"
```

---

### Task 12: Integration tests

**Files:**
- Modify: `tests/issue_commands.rs` (add new test)
- Modify: `tests/common/fixtures.rs` (add fixtures)

- [ ] **Step 1: Add fixture for issue with story points**

In `tests/common/fixtures.rs`, add:

```rust
pub fn issue_response_with_points(key: &str, summary: &str, status: &str, points: Option<f64>) -> Value {
    let mut fields = json!({
        "summary": summary,
        "status": {
            "name": status,
            "statusCategory": {"name": status, "key": if status == "Done" { "done" } else { "new" }}
        },
        "issuetype": {"name": "Story"},
        "priority": {"name": "Medium"},
        "assignee": {"accountId": "abc123", "displayName": "Test User"},
        "project": {"key": key.split('-').next().unwrap_or("TEST")}
    });
    if let Some(pts) = points {
        fields["customfield_10031"] = json!(pts);
    }
    json!({
        "key": key,
        "fields": fields
    })
}

pub fn fields_response_with_story_points() -> Value {
    json!([
        {
            "id": "summary",
            "name": "Summary",
            "custom": false,
            "schema": {"type": "string"}
        },
        {
            "id": "customfield_10031",
            "name": "Story Points",
            "custom": true,
            "schema": {
                "type": "number",
                "custom": "com.atlassian.jira.plugin.system.customfieldtypes:float",
                "customId": 10031
            }
        }
    ])
}
```

- [ ] **Step 2: Add integration test for search with story points in extra map**

In `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_search_issues_with_story_points() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_points("FOO-1", "Story A", "To Do", Some(5.0)),
                common::fixtures::issue_response_with_points("FOO-2", "Story B", "Done", None),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issues = client
        .search_issues("project = FOO", None, &["customfield_10031"])
        .await
        .unwrap();

    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].fields.story_points("customfield_10031"), Some(5.0));
    assert_eq!(issues[1].fields.story_points("customfield_10031"), None);
}
```

- [ ] **Step 3: Add integration test for field discovery**

```rust
#[tokio::test]
async fn test_find_story_points_field_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::fields_response_with_story_points()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let matches = client.find_story_points_field_id().await.unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0, "customfield_10031");
    assert_eq!(matches[0].1, "Story Points");
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy --all --all-features --tests -- -D warnings && cargo fmt --all -- --check`
Expected: No warnings, formatting clean

- [ ] **Step 6: Commit**

```bash
git add tests/issue_commands.rs tests/common/fixtures.rs
git commit -m "test: add integration tests for story points"
```

---

### Task 13: Final verification

- [ ] **Step 1: Run the full CI equivalent locally**

```bash
cargo fmt --all -- --check
cargo clippy --all --all-features --tests -- -D warnings
cargo test --all-features
```

Expected: All pass

- [ ] **Step 2: Verify no regressions with existing fixtures**

Run: `cargo test --test issue_commands`
Run: `cargo test --test team_commands`
Expected: All pass

- [ ] **Step 3: Final commit if any formatting fixes needed**

```bash
cargo fmt --all
git add -A
git commit -m "chore: format"
```
