# Issue Linking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add parent/child relationships and issue linking (blocks, relates to, duplicates, etc.) to `jr`, enabling AI agents to build ticket hierarchies and peer relationships programmatically.

**Architecture:** Parent/child via the `parent` field on create/edit. Issue links via `POST /rest/api/3/issueLink` (create), `DELETE /rest/api/3/issueLink/{id}` (remove), `GET /rest/api/3/issueLinkType` (list types). New types for links/parent in `issue.rs`, new API module `links.rs`, new `delete` method on `JiraClient`.

**Tech Stack:** Rust, clap (CLI), serde (JSON), reqwest (HTTP), wiremock (tests)

**Spec:** `docs/superpowers/specs/2026-03-22-issue-linking-design.md`

---

### Task 1: Add link/parent types to `src/types/jira/issue.rs`

**Files:**
- Modify: `src/types/jira/issue.rs`

- [ ] **Step 1: Write unit tests for new type deserialization**

Add to the existing `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn parent_deserializes() {
        let json = json!({
            "summary": "test",
            "parent": {"key": "FOO-42", "fields": {"summary": "Parent epic"}}
        });
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        let parent = fields.parent.unwrap();
        assert_eq!(parent.key, "FOO-42");
        assert_eq!(parent.fields.unwrap().summary.unwrap(), "Parent epic");
    }

    #[test]
    fn parent_missing() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.parent.is_none());
    }

    #[test]
    fn issuelinks_deserializes() {
        let json = json!({
            "summary": "test",
            "issuelinks": [{
                "id": "10001",
                "type": {"name": "Blocks", "inward": "is blocked by", "outward": "blocks"},
                "outwardIssue": {"key": "FOO-2", "fields": {"summary": "Other issue"}}
            }]
        });
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        let links = fields.issuelinks.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, "10001");
        assert_eq!(links[0].link_type.name, "Blocks");
        let outward = links[0].outward_issue.as_ref().unwrap();
        assert_eq!(outward.key, "FOO-2");
    }

    #[test]
    fn issuelinks_missing() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.issuelinks.is_none());
    }

    #[test]
    fn issuelinks_empty_array() {
        let json = json!({"summary": "test", "issuelinks": []});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.issuelinks.unwrap().len(), 0);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib issue`
Expected: FAIL — types not defined

- [ ] **Step 3: Add the new types**

Add BEFORE the `IssueFields` struct in `src/types/jira/issue.rs`:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct ParentIssue {
    pub key: String,
    pub fields: Option<LinkedIssueFields>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkedIssueFields {
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueLink {
    pub id: String,
    #[serde(rename = "type")]
    pub link_type: IssueLinkType,
    #[serde(rename = "inwardIssue")]
    pub inward_issue: Option<LinkedIssue>,
    #[serde(rename = "outwardIssue")]
    pub outward_issue: Option<LinkedIssue>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkedIssue {
    pub key: String,
    pub fields: Option<LinkedIssueFields>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueLinkType {
    pub id: Option<String>,
    pub name: String,
    pub inward: Option<String>,
    pub outward: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueLinkTypesResponse {
    #[serde(rename = "issueLinkTypes")]
    pub issue_link_types: Vec<IssueLinkType>,
}
```

- [ ] **Step 4: Add `parent` and `issuelinks` fields to `IssueFields`**

Add after the `labels` field (before `#[serde(flatten)]`):

```rust
    pub parent: Option<ParentIssue>,
    pub issuelinks: Option<Vec<IssueLink>>,
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib issue`
Expected: All pass (existing + 5 new)

Run: `cargo test`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/types/jira/issue.rs
git commit -m "feat: add ParentIssue, IssueLink, and IssueLinkType types"
```

---

### Task 2: Add `delete` method to `JiraClient` and create `links.rs` API module

**Files:**
- Modify: `src/api/client.rs` (add `delete` method)
- Create: `src/api/jira/links.rs`
- Modify: `src/api/jira/mod.rs` (add `pub mod links;`)

- [ ] **Step 1: Add `delete` method to `JiraClient`**

In `src/api/client.rs`, add after the `post_no_content` method:

```rust
    /// Perform a DELETE request that returns 204 No Content on success.
    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let request = self.client.delete(&url);
        self.send(request).await?;
        Ok(())
    }
```

- [ ] **Step 2: Create `src/api/jira/links.rs`**

```rust
use crate::api::client::JiraClient;
use crate::types::jira::issue::{IssueLinkType, IssueLinkTypesResponse};
use anyhow::Result;
use serde_json::json;

impl JiraClient {
    /// Create a link between two issues.
    /// outward_key gets the "outward" label (e.g., "blocks"),
    /// inward_key gets the "inward" label (e.g., "is blocked by").
    pub async fn create_issue_link(
        &self,
        outward_key: &str,
        inward_key: &str,
        link_type: &str,
    ) -> Result<()> {
        let body = json!({
            "outwardIssue": {"key": outward_key},
            "inwardIssue": {"key": inward_key},
            "type": {"name": link_type}
        });
        self.post_no_content("/rest/api/3/issueLink", &body).await
    }

    /// Delete an issue link by its ID.
    pub async fn delete_issue_link(&self, link_id: &str) -> Result<()> {
        let path = format!("/rest/api/3/issueLink/{}", link_id);
        self.delete(&path).await
    }

    /// List all available issue link types.
    pub async fn list_link_types(&self) -> Result<Vec<IssueLinkType>> {
        let resp: IssueLinkTypesResponse = self.get("/rest/api/3/issueLinkType").await?;
        Ok(resp.issue_link_types)
    }
}
```

- [ ] **Step 3: Register the module**

In `src/api/jira/mod.rs`, add:

```rust
pub mod links;
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All pass

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 5: Commit**

```bash
git add src/api/client.rs src/api/jira/links.rs src/api/jira/mod.rs
git commit -m "feat: add delete method to JiraClient and links API module"
```

---

### Task 3: Add `parent` and `issuelinks` to `get_issue()` fields

**Files:**
- Modify: `src/api/jira/issues.rs`

- [ ] **Step 1: Add `parent,issuelinks` to the base fields string in `get_issue()`**

In `src/api/jira/issues.rs`, find the `get_issue` method. Change the base fields string from:

```rust
let mut fields = "summary,status,issuetype,priority,assignee,project,description,labels".to_string();
```

To:

```rust
let mut fields = "summary,status,issuetype,priority,assignee,project,description,labels,parent,issuelinks".to_string();
```

Do NOT add to `search_issues()` — parent/links are only needed for single-issue view, not list output.

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add src/api/jira/issues.rs
git commit -m "feat: add parent and issuelinks to get_issue fields"
```

---

### Task 4: Add CLI enum variants and `--parent` flag

**Files:**
- Modify: `src/cli/mod.rs`

- [ ] **Step 1: Add `--parent` to `Create` variant**

In the `IssueCommand::Create` enum, add after `markdown`:

```rust
        /// Parent issue key (e.g., for subtasks or stories under epics)
        #[arg(long)]
        parent: Option<String>,
```

- [ ] **Step 2: Add `--parent` to `Edit` variant**

In the `IssueCommand::Edit` enum, add after `no_points`:

```rust
        /// Parent issue key
        #[arg(long)]
        parent: Option<String>,
```

- [ ] **Step 3: Add `Link`, `Unlink`, and `LinkTypes` variants**

Add to the `IssueCommand` enum:

```rust
    /// Link two issues
    Link {
        /// First issue key (outward — e.g., the issue that "blocks")
        key1: String,
        /// Second issue key (inward — e.g., the issue that "is blocked by")
        key2: String,
        /// Link type name (partial match supported, default: "Relates")
        #[arg(long, default_value = "Relates")]
        r#type: String,
    },
    /// Remove link(s) between two issues
    Unlink {
        /// First issue key
        key1: String,
        /// Second issue key
        key2: String,
        /// Only remove links of this type (removes all if omitted)
        #[arg(long)]
        r#type: Option<String>,
    },
    /// List available link types
    LinkTypes,
```

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: May warn about unused variants — OK for now

- [ ] **Step 5: Commit**

```bash
git add src/cli/mod.rs
git commit -m "feat: add --parent flag and Link/Unlink/LinkTypes CLI variants"
```

---

### Task 5: Wire parent into create and edit handlers

**Files:**
- Modify: `src/cli/issue.rs`

- [ ] **Step 1: Update `handle_create` destructuring and payload**

In `handle_create`, add `parent` to the `let IssueCommand::Create { ... }` destructure.

After the story points block, add:

```rust
    if let Some(ref parent_key) = parent {
        fields["parent"] = json!({"key": parent_key});
    }
```

- [ ] **Step 2: Update `handle_edit` destructuring and payload**

In `handle_edit`, add `parent` to the `let IssueCommand::Edit { ... }` destructure.

After the story points/no_points blocks, add:

```rust
    if let Some(ref parent_key) = parent {
        fields["parent"] = json!({"key": parent_key});
        has_updates = true;
    }
```

- [ ] **Step 3: Update edit error message**

Change the "no fields specified" error to include `--parent`:

```rust
"No fields specified to update. Use --summary, --type, --priority, --label, --team, --points, --no-points, or --parent."
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue.rs
git commit -m "feat: wire --parent into issue create and edit"
```

---

### Task 6: Update `handle_view` to display parent and links

**Files:**
- Modify: `src/cli/issue.rs`

- [ ] **Step 1: Add `parent,issuelinks` to the extra fields request in `handle_view`**

Currently `handle_view` builds `extra` from `sp_field_id`. The `parent` and `issuelinks` fields are already in the base `get_issue` fields (from Task 3), so no API change needed here. The fields will be present in the response.

- [ ] **Step 2: Add Parent row to table output**

In `handle_view`, after the Labels row (before the Points row), add:

```rust
                rows.push(vec![
                    "Parent".into(),
                    issue
                        .fields
                        .parent
                        .as_ref()
                        .map(|p| {
                            let summary = p
                                .fields
                                .as_ref()
                                .and_then(|f| f.summary.as_deref())
                                .unwrap_or("");
                            format!("{} ({})", p.key, summary)
                        })
                        .unwrap_or_else(|| "(none)".into()),
                ]);
```

- [ ] **Step 3: Add Links row to table output**

After the Parent row, add:

```rust
                let links_display = issue
                    .fields
                    .issuelinks
                    .as_ref()
                    .filter(|links| !links.is_empty())
                    .map(|links| {
                        links
                            .iter()
                            .map(|link| {
                                if let Some(ref outward) = link.outward_issue {
                                    let desc = link.link_type.outward.as_deref().unwrap_or(&link.link_type.name);
                                    let summary = outward.fields.as_ref().and_then(|f| f.summary.as_deref()).unwrap_or("");
                                    format!("{} {} ({})", desc, outward.key, summary)
                                } else if let Some(ref inward) = link.inward_issue {
                                    let desc = link.link_type.inward.as_deref().unwrap_or(&link.link_type.name);
                                    let summary = inward.fields.as_ref().and_then(|f| f.summary.as_deref()).unwrap_or("");
                                    format!("{} {} ({})", desc, inward.key, summary)
                                } else {
                                    link.link_type.name.clone()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_else(|| "(none)".into());
                rows.push(vec!["Links".into(), links_display]);
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/cli/issue.rs
git commit -m "feat: display parent and links in issue view"
```

---

### Task 7: Add `handle_link`, `handle_unlink`, and `handle_link_types` handlers

**Files:**
- Modify: `src/cli/issue.rs`

- [ ] **Step 1: Add match arms to `handle()` dispatch**

In the `handle()` match block, add after the `Open` arm:

```rust
        IssueCommand::Link { .. } => {
            handle_link(command, output_format, client, no_input).await
        }
        IssueCommand::Unlink { .. } => {
            handle_unlink(command, output_format, client, no_input).await
        }
        IssueCommand::LinkTypes => {
            handle_link_types(output_format, client).await
        }
```

- [ ] **Step 2: Add `handle_link_types` handler**

```rust
async fn handle_link_types(
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let link_types = client.list_link_types().await?;

    let rows: Vec<Vec<String>> = link_types
        .iter()
        .map(|lt| {
            vec![
                lt.id.clone().unwrap_or_default(),
                lt.name.clone(),
                lt.outward.clone().unwrap_or_default(),
                lt.inward.clone().unwrap_or_default(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "Name", "Outward", "Inward"],
        &rows,
        &link_types,
    )?;

    Ok(())
}
```

- [ ] **Step 3: Add `handle_link` handler**

```rust
async fn handle_link(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Link {
        key1,
        key2,
        r#type: link_type_name,
    } = command
    else {
        unreachable!()
    };

    // Self-link check
    if key1.eq_ignore_ascii_case(&key2) {
        bail!("Cannot link an issue to itself.");
    }

    // Resolve link type via partial match
    let link_types = client.list_link_types().await?;
    let type_names: Vec<String> = link_types.iter().map(|lt| lt.name.clone()).collect();
    let resolved_name = match partial_match::partial_match(&link_type_name, &type_names) {
        MatchResult::Exact(name) => name,
        MatchResult::Ambiguous(matches) => {
            if no_input {
                bail!(
                    "Ambiguous link type \"{}\". Matches: {}",
                    link_type_name,
                    matches.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple types match \"{link_type_name}\""))
                .items(&matches)
                .interact()?;
            matches[selection].clone()
        }
        MatchResult::None(_) => {
            bail!(
                "Unknown link type \"{}\". Run \"jr issue link-types\" to see available types.",
                link_type_name
            );
        }
    };

    client
        .create_issue_link(&key1, &key2, &resolved_name)
        .await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "key1": key1,
                    "key2": key2,
                    "type": resolved_name,
                    "linked": true
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Linked {} → {} ({})",
                key1, key2, resolved_name
            ));
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Add `handle_unlink` handler**

```rust
async fn handle_unlink(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Unlink {
        key1,
        key2,
        r#type: link_type_filter,
    } = command
    else {
        unreachable!()
    };

    // Resolve link type filter if provided
    let resolved_type_filter = if let Some(ref type_name) = link_type_filter {
        let link_types = client.list_link_types().await?;
        let type_names: Vec<String> = link_types.iter().map(|lt| lt.name.clone()).collect();
        let resolved = match partial_match::partial_match(type_name, &type_names) {
            MatchResult::Exact(name) => name,
            MatchResult::Ambiguous(matches) => {
                if no_input {
                    bail!(
                        "Ambiguous link type \"{}\". Matches: {}",
                        type_name,
                        matches.join(", ")
                    );
                }
                let selection = dialoguer::Select::new()
                    .with_prompt(format!("Multiple types match \"{type_name}\""))
                    .items(&matches)
                    .interact()?;
                matches[selection].clone()
            }
            MatchResult::None(_) => {
                bail!(
                    "Unknown link type \"{}\". Run \"jr issue link-types\" to see available types.",
                    type_name
                );
            }
        };
        Some(resolved)
    } else {
        None
    };

    // Fetch links on key1
    let issue = client.get_issue(&key1, &[]).await?;
    let links = issue.fields.issuelinks.unwrap_or_default();

    // Find matching links to key2
    let matching_links: Vec<&crate::types::jira::issue::IssueLink> = links
        .iter()
        .filter(|link| {
            let matches_key = link
                .outward_issue
                .as_ref()
                .map(|i| i.key.eq_ignore_ascii_case(&key2))
                .unwrap_or(false)
                || link
                    .inward_issue
                    .as_ref()
                    .map(|i| i.key.eq_ignore_ascii_case(&key2))
                    .unwrap_or(false);

            let matches_type = resolved_type_filter
                .as_ref()
                .map(|t| link.link_type.name.eq_ignore_ascii_case(t))
                .unwrap_or(true);

            matches_key && matches_type
        })
        .collect();

    if matching_links.is_empty() {
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "unlinked": false,
                        "count": 0
                    }))?
                );
            }
            OutputFormat::Table => {
                output::print_success(&format!(
                    "No link found between {} and {}",
                    key1, key2
                ));
            }
        }
        return Ok(());
    }

    let count = matching_links.len();
    for link in &matching_links {
        client.delete_issue_link(&link.id).await?;
    }

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "unlinked": true,
                    "count": count
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Removed {} link(s) between {} and {}",
                count, key1, key2
            ));
        }
    }

    Ok(())
}
```

Note: `handle_unlink` accesses `issue.fields.issuelinks` — the `issuelinks` field lives on `IssueFields`, not `Issue`. Since `issuelinks` is in the base `get_issue` fields (Task 3), it will be populated.

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All pass

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 6: Commit**

```bash
git add src/cli/issue.rs
git commit -m "feat: add handle_link, handle_unlink, and handle_link_types handlers"
```

---

### Task 8: Integration tests

**Files:**
- Modify: `tests/common/fixtures.rs`
- Modify: `tests/issue_commands.rs`

- [ ] **Step 1: Add fixtures**

In `tests/common/fixtures.rs`:

```rust
pub fn link_types_response() -> Value {
    json!({
        "issueLinkTypes": [
            {
                "id": "1000",
                "name": "Blocks",
                "inward": "is blocked by",
                "outward": "blocks"
            },
            {
                "id": "1001",
                "name": "Duplicate",
                "inward": "is duplicated by",
                "outward": "duplicates"
            },
            {
                "id": "1002",
                "name": "Relates",
                "inward": "relates to",
                "outward": "relates to"
            }
        ]
    })
}

pub fn issue_with_links_response(key: &str, summary: &str) -> Value {
    json!({
        "key": key,
        "fields": {
            "summary": summary,
            "status": {"name": "To Do"},
            "issuetype": {"name": "Story"},
            "priority": {"name": "Medium"},
            "assignee": {"accountId": "abc123", "displayName": "Test User"},
            "project": {"key": key.split('-').next().unwrap_or("TEST")},
            "parent": {"key": "FOO-1", "fields": {"summary": "Parent Epic"}},
            "issuelinks": [
                {
                    "id": "20001",
                    "type": {"name": "Blocks", "inward": "is blocked by", "outward": "blocks"},
                    "outwardIssue": {"key": "FOO-3", "fields": {"summary": "Blocked issue"}}
                }
            ]
        }
    })
}
```

- [ ] **Step 2: Add integration test for list link types**

In `tests/issue_commands.rs`:

```rust
#[tokio::test]
async fn test_list_link_types() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issueLinkType"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::link_types_response()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let types = client.list_link_types().await.unwrap();
    assert_eq!(types.len(), 3);
    assert_eq!(types[0].name, "Blocks");
    assert_eq!(types[0].outward.as_deref(), Some("blocks"));
    assert_eq!(types[0].inward.as_deref(), Some("is blocked by"));
}
```

- [ ] **Step 3: Add integration test for issue with parent and links**

```rust
#[tokio::test]
async fn test_get_issue_with_parent_and_links() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_with_links_response("FOO-2", "Test issue")),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issue = client.get_issue("FOO-2", &[]).await.unwrap();

    // Parent
    let parent = issue.fields.parent.unwrap();
    assert_eq!(parent.key, "FOO-1");
    assert_eq!(
        parent.fields.unwrap().summary.unwrap(),
        "Parent Epic"
    );

    // Links
    let links = issue.fields.issuelinks.unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].link_type.name, "Blocks");
    assert_eq!(links[0].outward_issue.as_ref().unwrap().key, "FOO-3");
}
```

- [ ] **Step 4: Add integration test for create issue link**

```rust
#[tokio::test]
async fn test_create_issue_link() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issueLink"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client
        .create_issue_link("FOO-1", "FOO-2", "Blocks")
        .await
        .unwrap();
}
```

- [ ] **Step 5: Add integration test for delete issue link**

```rust
#[tokio::test]
async fn test_delete_issue_link() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issueLink/10001"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client.delete_issue_link("10001").await.unwrap();
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add tests/common/fixtures.rs tests/issue_commands.rs
git commit -m "test: add integration tests for issue linking"
```

---

### Task 9: Final verification and live smoke test

- [ ] **Step 1: Run full CI equivalent**

```bash
cargo fmt --all -- --check
cargo clippy --all --all-features --tests -- -D warnings
cargo test --all-features
```

Expected: All pass

- [ ] **Step 2: Build and verify help**

```bash
cargo build --release
./target/release/jr issue --help
./target/release/jr issue link --help
./target/release/jr issue unlink --help
./target/release/jr issue link-types --help
```

Expected: All subcommands appear with correct descriptions

- [ ] **Step 3: Live smoke test**

```bash
./target/release/jr issue link-types
./target/release/jr issue view <issue-with-parent>
```

Expected: Link types table displays correctly. View shows Parent and Links rows.

- [ ] **Step 4: Format if needed**

```bash
cargo fmt --all
git add -A
git commit -m "chore: format"
```
