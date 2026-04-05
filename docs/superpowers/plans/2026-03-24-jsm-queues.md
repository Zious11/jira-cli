# JSM Queue Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `jr queue list` and `jr queue view` commands for Jira Service Management projects, with project type auto-detection and caching.

**Architecture:** New `src/api/jsm/` and `src/types/jsm/` sibling modules alongside existing `jira/` directories. Project type is auto-detected via platform API, cached per-project with 7-day TTL. JSM API calls use `get_from_instance()` (not `get()`) to hit `/rest/servicedeskapi/` on the real instance URL.

**Tech Stack:** Rust, reqwest, serde, chrono, comfy-table, wiremock (tests), clap

**Spec:** `docs/superpowers/specs/2026-03-24-jsm-queues-design.md`

---

## File Structure

| File | Responsibility | Change |
|------|---------------|--------|
| `src/api/pagination.rs` | Add `ServiceDeskPage<T>` | Modify |
| `src/cache.rs` | Add `ProjectMeta`, `read_project_meta()`, `write_project_meta()` | Modify |
| `src/types/jsm/mod.rs` | Re-exports for JSM types | Create |
| `src/types/jsm/servicedesk.rs` | `ServiceDesk` struct | Create |
| `src/types/jsm/queue.rs` | `Queue`, `QueueIssue`, `QueueIssueFields` | Create |
| `src/types/mod.rs` | Add `pub mod jsm;` | Modify |
| `src/api/jsm/mod.rs` | Re-exports for JSM API | Create |
| `src/api/jsm/servicedesks.rs` | `list_service_desks()`, `get_or_fetch_project_meta()` | Create |
| `src/api/jsm/queues.rs` | `list_queues()`, `get_queue_issues()` | Create |
| `src/api/mod.rs` | Add `pub mod jsm;` | Modify |
| `src/cli/mod.rs` | Add `Queue` command + `QueueCommand` enum | Modify |
| `src/cli/queue.rs` | `handle()`, `handle_list()`, `handle_view()` | Create |
| `src/main.rs` | Add dispatch for `Command::Queue` | Modify |
| `tests/queue.rs` | Integration tests for queue commands | Create |
| `tests/project_meta.rs` | Integration tests for project type detection | Create |

---

### Task 1: ServiceDeskPage pagination type

**Files:**
- Modify: `src/api/pagination.rs`

- [ ] **Step 1: Write unit tests for `ServiceDeskPage<T>`**

Add these tests at the bottom of the existing `#[cfg(test)] mod tests` block in `src/api/pagination.rs`:

```rust
    #[test]
    fn test_service_desk_page_has_more() {
        let page: ServiceDeskPage<String> = ServiceDeskPage {
            size: 5,
            start: 0,
            limit: 50,
            is_last_page: false,
            values: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
        };
        assert!(page.has_more());
        assert_eq!(page.next_start(), 5);
    }

    #[test]
    fn test_service_desk_page_last_page() {
        let page: ServiceDeskPage<String> = ServiceDeskPage {
            size: 3,
            start: 10,
            limit: 50,
            is_last_page: true,
            values: vec!["a".into(), "b".into(), "c".into()],
        };
        assert!(!page.has_more());
        assert_eq!(page.next_start(), 13);
    }

    #[test]
    fn test_service_desk_page_empty() {
        let page: ServiceDeskPage<String> = ServiceDeskPage {
            size: 0,
            start: 0,
            limit: 50,
            is_last_page: true,
            values: vec![],
        };
        assert!(!page.has_more());
        assert_eq!(page.next_start(), 0);
        assert!(page.values.is_empty());
    }

    #[test]
    fn test_service_desk_page_deserialize() {
        let json = r#"{
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": false,
            "values": ["item1", "item2"]
        }"#;
        let page: ServiceDeskPage<String> = serde_json::from_str(json).unwrap();
        assert_eq!(page.size, 2);
        assert_eq!(page.values.len(), 2);
        assert!(!page.is_last_page);
    }
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo test --lib pagination -- --nocapture
```

Expected: compilation errors — `ServiceDeskPage` doesn't exist yet.

- [ ] **Step 3: Implement `ServiceDeskPage<T>`**

Add this after the `CursorPage` impl block (before the `#[cfg(test)]` module) in `src/api/pagination.rs`:

```rust
/// Offset-based pagination used by Jira Service Management `/rest/servicedeskapi/` endpoints.
///
/// Uses different field names than `OffsetPage`: `size` (items in page) instead of `total`,
/// `isLastPage` boolean instead of computed from startAt+maxResults, and `start`/`limit`
/// instead of `startAt`/`maxResults`.
#[derive(Debug, Deserialize)]
pub struct ServiceDeskPage<T> {
    /// Count of items in the current page.
    pub size: u32,
    /// Zero-based starting index.
    pub start: u32,
    /// Maximum items per page.
    pub limit: u32,
    /// Whether this is the last page of results.
    #[serde(rename = "isLastPage")]
    pub is_last_page: bool,
    /// The items in this page.
    #[serde(default)]
    pub values: Vec<T>,
}

impl<T> ServiceDeskPage<T> {
    /// Returns true if there are more pages after this one.
    pub fn has_more(&self) -> bool {
        !self.is_last_page
    }

    /// Returns the `start` value for the next page.
    pub fn next_start(&self) -> u32 {
        self.start + self.size
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo test --lib pagination -- --nocapture
```

Expected: all pagination tests pass including the 4 new ones.

- [ ] **Step 5: Commit**

```bash
git add src/api/pagination.rs
git commit -m "feat: add ServiceDeskPage pagination type for JSM API

ServiceDeskPage<T> handles the PagedDTO format used by
/rest/servicedeskapi/ endpoints which differs from the platform API's
OffsetPage format (isLastPage boolean, size/start/limit fields)."
```

---

### Task 2: ProjectMeta cache

**Files:**
- Modify: `src/cache.rs`

- [ ] **Step 1: Write unit tests for `ProjectMeta` cache**

Add these tests inside the existing `#[cfg(test)] mod tests` block in `src/cache.rs`:

```rust
    #[test]
    fn read_missing_project_meta_returns_none() {
        with_temp_cache(|| {
            let result = read_project_meta("NOEXIST").unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_project_meta() {
        with_temp_cache(|| {
            let meta = ProjectMeta {
                project_type: "service_desk".into(),
                simplified: false,
                project_id: "10042".into(),
                service_desk_id: Some("15".into()),
                fetched_at: Utc::now(),
            };
            write_project_meta("HELPDESK", &meta).unwrap();

            let loaded = read_project_meta("HELPDESK").unwrap().expect("should exist");
            assert_eq!(loaded.project_type, "service_desk");
            assert_eq!(loaded.service_desk_id.as_deref(), Some("15"));
            assert_eq!(loaded.project_id, "10042");
            assert!(!loaded.simplified);
        });
    }

    #[test]
    fn expired_project_meta_returns_none() {
        with_temp_cache(|| {
            let meta = ProjectMeta {
                project_type: "service_desk".into(),
                simplified: false,
                project_id: "10042".into(),
                service_desk_id: Some("15".into()),
                fetched_at: Utc::now() - chrono::Duration::days(8),
            };
            write_project_meta("HELPDESK", &meta).unwrap();

            let result = read_project_meta("HELPDESK").unwrap();
            assert!(result.is_none(), "expired project meta should return None");
        });
    }

    #[test]
    fn project_meta_multiple_projects() {
        with_temp_cache(|| {
            let jsm = ProjectMeta {
                project_type: "service_desk".into(),
                simplified: false,
                project_id: "10042".into(),
                service_desk_id: Some("15".into()),
                fetched_at: Utc::now(),
            };
            let software = ProjectMeta {
                project_type: "software".into(),
                simplified: true,
                project_id: "10001".into(),
                service_desk_id: None,
                fetched_at: Utc::now(),
            };
            write_project_meta("HELPDESK", &jsm).unwrap();
            write_project_meta("DEV", &software).unwrap();

            let jsm_loaded = read_project_meta("HELPDESK").unwrap().expect("should exist");
            assert_eq!(jsm_loaded.project_type, "service_desk");

            let sw_loaded = read_project_meta("DEV").unwrap().expect("should exist");
            assert_eq!(sw_loaded.project_type, "software");
            assert!(sw_loaded.service_desk_id.is_none());
        });
    }
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo test --lib cache -- --nocapture
```

Expected: compilation errors — `ProjectMeta`, `read_project_meta`, `write_project_meta` don't exist yet.

- [ ] **Step 3: Implement `ProjectMeta` and cache functions**

Add to `src/cache.rs`, after the existing `write_team_cache` function (before the `#[cfg(test)]` block):

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub project_type: String,
    pub simplified: bool,
    pub project_id: String,
    pub service_desk_id: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

pub fn read_project_meta(project_key: &str) -> Result<Option<ProjectMeta>> {
    let path = cache_dir().join("project_meta.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let map: HashMap<String, ProjectMeta> = serde_json::from_str(&content)?;

    match map.get(project_key) {
        Some(meta) => {
            let age = Utc::now() - meta.fetched_at;
            if age.num_days() >= CACHE_TTL_DAYS {
                Ok(None)
            } else {
                Ok(Some(meta.clone()))
            }
        }
        None => Ok(None),
    }
}

pub fn write_project_meta(project_key: &str, meta: &ProjectMeta) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("project_meta.json");

    // Read existing map or start fresh
    let mut map: HashMap<String, ProjectMeta> = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    map.insert(project_key.to_string(), meta.clone());

    let content = serde_json::to_string_pretty(&map)?;
    std::fs::write(&path, content)?;
    Ok(())
}
```

Also add `use std::collections::HashMap;` to the top of the file if not already present.

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo test --lib cache -- --nocapture
```

Expected: all cache tests pass including the 4 new ones.

- [ ] **Step 5: Run clippy**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/cache.rs
git commit -m "feat: add ProjectMeta cache for project type detection

Per-project cache in ~/.cache/jr/project_meta.json stores
projectTypeKey, simplified flag, projectId, and serviceDeskId
with 7-day TTL. Supports multiple projects in the same cache file."
```

---

### Task 3: JSM types (ServiceDesk, Queue, QueueIssue)

**Files:**
- Create: `src/types/jsm/mod.rs`
- Create: `src/types/jsm/servicedesk.rs`
- Create: `src/types/jsm/queue.rs`
- Modify: `src/types/mod.rs`

- [ ] **Step 1: Create `src/types/jsm/servicedesk.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ServiceDesk {
    pub id: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(rename = "projectName")]
    pub project_name: String,
}
```

- [ ] **Step 2: Create `src/types/jsm/queue.rs`**

```rust
use serde::{Deserialize, Serialize};

use crate::types::jira::issue::{IssueType, Priority, Status};
use crate::types::jira::User;

#[derive(Debug, Deserialize, Serialize)]
pub struct Queue {
    pub id: String,
    pub name: String,
    pub jql: Option<String>,
    pub fields: Option<Vec<String>>,
    #[serde(rename = "issueCount")]
    pub issue_count: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QueueIssue {
    pub key: String,
    pub fields: QueueIssueFields,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QueueIssueFields {
    pub summary: Option<String>,
    pub status: Option<Status>,
    pub issuetype: Option<IssueType>,
    pub priority: Option<Priority>,
    pub assignee: Option<User>,
    pub reporter: Option<User>,
    pub created: Option<String>,
}
```

- [ ] **Step 3: Create `src/types/jsm/mod.rs`**

```rust
pub mod queue;
pub mod servicedesk;

pub use queue::*;
pub use servicedesk::*;
```

- [ ] **Step 4: Add `pub mod jsm;` to `src/types/mod.rs`**

The file currently contains only `pub mod jira;`. Add:

```rust
pub mod jira;
pub mod jsm;
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo build
```

Expected: compiles with no errors. There may be "unused" warnings — that's fine, they'll be used in the next task.

- [ ] **Step 6: Write deserialization unit test**

Add to the bottom of `src/types/jsm/queue.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_queue_with_all_fields() {
        let json = r#"{
            "id": "10",
            "name": "Triage",
            "jql": "project = HELPDESK AND status = New",
            "fields": ["issuetype", "issuekey", "summary", "status"],
            "issueCount": 12
        }"#;
        let queue: Queue = serde_json::from_str(json).unwrap();
        assert_eq!(queue.id, "10");
        assert_eq!(queue.name, "Triage");
        assert_eq!(queue.issue_count, Some(12));
        assert!(queue.jql.is_some());
    }

    #[test]
    fn deserialize_queue_without_optional_fields() {
        let json = r#"{
            "id": "20",
            "name": "All open"
        }"#;
        let queue: Queue = serde_json::from_str(json).unwrap();
        assert_eq!(queue.id, "20");
        assert!(queue.issue_count.is_none());
        assert!(queue.jql.is_none());
        assert!(queue.fields.is_none());
    }

    #[test]
    fn deserialize_queue_issue_minimal() {
        let json = r#"{
            "key": "HELPDESK-42",
            "fields": {
                "summary": "VPN not working"
            }
        }"#;
        let issue: QueueIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.key, "HELPDESK-42");
        assert_eq!(issue.fields.summary.as_deref(), Some("VPN not working"));
        assert!(issue.fields.status.is_none());
        assert!(issue.fields.assignee.is_none());
    }

    #[test]
    fn deserialize_queue_issue_full() {
        let json = r#"{
            "key": "HELPDESK-42",
            "fields": {
                "summary": "VPN not working",
                "status": { "name": "New", "statusCategory": { "name": "To Do", "key": "new" } },
                "issuetype": { "name": "Service Request" },
                "priority": { "name": "High" },
                "assignee": { "accountId": "abc123", "displayName": "Jane D." },
                "reporter": { "accountId": "def456", "displayName": "John S." },
                "created": "2026-03-24T10:00:00.000+0000"
            }
        }"#;
        let issue: QueueIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.key, "HELPDESK-42");
        assert_eq!(issue.fields.status.as_ref().unwrap().name, "New");
        assert_eq!(
            issue.fields.assignee.as_ref().unwrap().display_name,
            "Jane D."
        );
    }
}
```

- [ ] **Step 7: Run tests**

```bash
cargo test --lib types::jsm -- --nocapture
```

Expected: all 4 tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/types/jsm/ src/types/mod.rs
git commit -m "feat: add JSM types for ServiceDesk, Queue, and QueueIssue

First JSM type definitions in the new types/jsm/ sibling module.
Queue issues use a limited field set (only queue-configured fields),
reusing Status, IssueType, Priority, and User from types/jira/."
```

---

### Task 4: JSM API — service desks + project meta orchestration

**Files:**
- Create: `src/api/jsm/mod.rs`
- Create: `src/api/jsm/servicedesks.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Create `src/api/jsm/mod.rs`**

```rust
pub mod queues;
pub mod servicedesks;
```

- [ ] **Step 2: Add `pub mod jsm;` to `src/api/mod.rs`**

The file currently contains:

```rust
pub mod auth;
pub mod client;
pub mod jira;
pub mod pagination;
pub mod rate_limit;
```

Add `pub mod jsm;` after `pub mod jira;`:

```rust
pub mod auth;
pub mod client;
pub mod jira;
pub mod jsm;
pub mod pagination;
pub mod rate_limit;
```

- [ ] **Step 3: Create `src/api/jsm/servicedesks.rs`**

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::ServiceDeskPage;
use crate::cache::{self, ProjectMeta};
use crate::error::JrError;
use crate::types::jsm::ServiceDesk;
use chrono::Utc;

impl JiraClient {
    /// List all service desks, auto-paginating.
    pub async fn list_service_desks(&self) -> Result<Vec<ServiceDesk>> {
        let mut all = Vec::new();
        let mut start = 0u32;
        let page_size = 50u32;

        loop {
            let path = format!(
                "/rest/servicedeskapi/servicedesk?start={}&limit={}",
                start, page_size
            );
            let page: ServiceDeskPage<ServiceDesk> =
                self.get_from_instance(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values);
            if !has_more {
                break;
            }
            start = next;
        }
        Ok(all)
    }
}

/// Fetch project metadata, using cache when available.
///
/// 1. Check cache for project_key — return if fresh.
/// 2. GET /rest/api/3/project/{key} — extract projectTypeKey, simplified, id.
/// 3. If service_desk: list service desks, match by projectId to find serviceDeskId.
/// 4. Write to cache and return.
pub async fn get_or_fetch_project_meta(
    client: &JiraClient,
    project_key: &str,
) -> Result<ProjectMeta> {
    // Check cache first
    if let Some(cached) = cache::read_project_meta(project_key)? {
        return Ok(cached);
    }

    // Fetch project details from platform API
    let project: serde_json::Value = client
        .get(&format!(
            "/rest/api/3/project/{}",
            urlencoding::encode(project_key)
        ))
        .await?;

    let project_type = project
        .get("projectTypeKey")
        .and_then(|v| v.as_str())
        .unwrap_or("software")
        .to_string();

    let simplified = project
        .get("simplified")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let project_id = project
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // If it's a service desk, resolve the serviceDeskId
    let service_desk_id = if project_type == "service_desk" {
        let desks = client.list_service_desks().await?;
        desks
            .iter()
            .find(|d| d.project_id == project_id)
            .map(|d| d.id.clone())
    } else {
        None
    };

    let meta = ProjectMeta {
        project_type,
        simplified,
        project_id,
        service_desk_id,
        fetched_at: Utc::now(),
    };

    // Write to cache (best-effort — don't fail the command if cache write fails)
    let _ = cache::write_project_meta(project_key, &meta);

    Ok(meta)
}

/// Require the project to be a JSM service desk. Returns the serviceDeskId or errors.
pub async fn require_service_desk(
    client: &JiraClient,
    project_key: &str,
) -> Result<String> {
    let meta = get_or_fetch_project_meta(client, project_key).await?;

    if meta.project_type != "service_desk" {
        let type_label = match meta.project_type.as_str() {
            "software" => "Jira Software",
            "business" => "Jira Work Management",
            _ => "Jira",
        };
        return Err(JrError::UserError(format!(
            "\"{}\" is a {} project. Queue commands require a Jira Service Management project. \
             Run \"jr project fields {}\" to see available commands.",
            project_key, type_label, project_key
        ))
        .into());
    }

    meta.service_desk_id.ok_or_else(|| {
        JrError::UserError(format!(
            "No service desk found for project \"{}\". \
             The project may not be configured as a service desk.",
            project_key
        ))
        .into()
    })
}
```

- [ ] **Step 4: Create placeholder `src/api/jsm/queues.rs`**

Create an empty placeholder so the `mod.rs` compiles:

```rust
// Queue API methods — implemented in Task 5.
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo build
```

Expected: compiles. Warnings about unused imports/functions are expected until CLI wiring.

- [ ] **Step 6: Commit**

```bash
git add src/api/jsm/ src/api/mod.rs
git commit -m "feat: add JSM service desk API + project meta orchestration

list_service_desks() auto-paginates through /rest/servicedeskapi/servicedesk.
get_or_fetch_project_meta() resolves projectTypeKey and serviceDeskId with
7-day cache. require_service_desk() gates JSM commands with helpful errors.

Uses get_from_instance() to hit instance URL directly (not OAuth proxy)."
```

---

### Task 5: JSM API — queue methods

**Files:**
- Modify: `src/api/jsm/queues.rs`

- [ ] **Step 1: Write integration tests for queue API methods**

Create `tests/queue.rs`:

```rust
#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_queues_returns_all_queues() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .and(query_param("includeCount", "true"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "10", "name": "Triage", "jql": "project = HELPDESK AND status = New", "issueCount": 12 },
                { "id": "20", "name": "In Progress", "jql": "project = HELPDESK AND status = \"In Progress\"", "issueCount": 7 }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let queues = client.list_queues("15").await.unwrap();
    assert_eq!(queues.len(), 2);
    assert_eq!(queues[0].name, "Triage");
    assert_eq!(queues[0].issue_count, Some(12));
    assert_eq!(queues[1].name, "In Progress");
}

#[tokio::test]
async fn list_queues_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 0,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": []
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let queues = client.list_queues("15").await.unwrap();
    assert!(queues.is_empty());
}

#[tokio::test]
async fn get_queue_issues_returns_issues() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                {
                    "key": "HELPDESK-42",
                    "fields": {
                        "summary": "VPN not working",
                        "status": { "name": "New", "statusCategory": { "name": "To Do", "key": "new" } },
                        "issuetype": { "name": "Service Request" },
                        "priority": { "name": "High" },
                        "assignee": null
                    }
                },
                {
                    "key": "HELPDESK-41",
                    "fields": {
                        "summary": "Need license renewal",
                        "status": { "name": "New", "statusCategory": { "name": "To Do", "key": "new" } },
                        "issuetype": { "name": "Service Request" },
                        "assignee": { "accountId": "abc", "displayName": "Jane D." }
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let issues = client.get_queue_issues("15", "10", None).await.unwrap();
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].key, "HELPDESK-42");
    assert!(issues[0].fields.assignee.is_none());
    assert_eq!(
        issues[1].fields.assignee.as_ref().unwrap().display_name,
        "Jane D."
    );
}

#[tokio::test]
async fn get_queue_issues_with_limit() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 1,
            "isLastPage": false,
            "values": [
                {
                    "key": "HELPDESK-42",
                    "fields": {
                        "summary": "VPN not working"
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let issues = client.get_queue_issues("15", "10", Some(1)).await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].key, "HELPDESK-42");
}

#[tokio::test]
async fn get_queue_issues_paginated() {
    let server = MockServer::start().await;

    // Page 1
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 1,
            "isLastPage": false,
            "values": [
                { "key": "HELPDESK-2", "fields": { "summary": "Issue A" } }
            ]
        })))
        .mount(&server)
        .await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .and(query_param("start", "1"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 1,
            "limit": 1,
            "isLastPage": true,
            "values": [
                { "key": "HELPDESK-1", "fields": { "summary": "Issue B" } }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let issues = client.get_queue_issues("15", "10", None).await.unwrap();
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].key, "HELPDESK-2");
    assert_eq!(issues[1].key, "HELPDESK-1");
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo test --test queue -- --nocapture
```

Expected: compilation errors — `list_queues` and `get_queue_issues` don't exist yet.

- [ ] **Step 3: Implement queue API methods**

Replace `src/api/jsm/queues.rs` with:

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::ServiceDeskPage;
use crate::types::jsm::Queue;
use crate::types::jsm::QueueIssue;

impl JiraClient {
    /// List all queues for a service desk, auto-paginating.
    pub async fn list_queues(&self, service_desk_id: &str) -> Result<Vec<Queue>> {
        let base = format!(
            "/rest/servicedeskapi/servicedesk/{}/queue",
            service_desk_id
        );
        let mut all = Vec::new();
        let mut start = 0u32;
        let page_size = 50u32;

        loop {
            let path = format!(
                "{}?includeCount=true&start={}&limit={}",
                base, start, page_size
            );
            let page: ServiceDeskPage<Queue> =
                self.get_from_instance(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values);
            if !has_more {
                break;
            }
            start = next;
        }
        Ok(all)
    }

    /// Get issues in a queue, with optional limit and auto-pagination.
    pub async fn get_queue_issues(
        &self,
        service_desk_id: &str,
        queue_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<QueueIssue>> {
        let base = format!(
            "/rest/servicedeskapi/servicedesk/{}/queue/{}/issue",
            service_desk_id, queue_id
        );
        let mut all = Vec::new();
        let mut start = 0u32;
        let max_page_size = 50u32;

        loop {
            let page_size = match limit {
                Some(cap) => {
                    let remaining = cap.saturating_sub(all.len() as u32);
                    if remaining == 0 {
                        break;
                    }
                    remaining.min(max_page_size)
                }
                None => max_page_size,
            };
            let path = format!(
                "{}?start={}&limit={}",
                base, start, page_size
            );
            let page: ServiceDeskPage<QueueIssue> =
                self.get_from_instance(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values);

            if let Some(cap) = limit {
                if all.len() >= cap as usize {
                    all.truncate(cap as usize);
                    break;
                }
            }
            if !has_more {
                break;
            }
            start = next;
        }
        Ok(all)
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo test --test queue -- --nocapture
```

Expected: all 5 queue integration tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/api/jsm/queues.rs tests/queue.rs
git commit -m "feat: add JSM queue API methods with auto-pagination

list_queues() and get_queue_issues() wrap /rest/servicedeskapi/
queue endpoints with ServiceDeskPage pagination. get_queue_issues
supports --limit with dynamic page sizing (same pattern as
list_comments). Includes 5 wiremock integration tests."
```

---

### Task 6: CLI — queue commands

**Files:**
- Create: `src/cli/queue.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add `QueueCommand` enum to `src/cli/mod.rs`**

Add `pub mod queue;` to the top of `src/cli/mod.rs` after the existing module declarations:

```rust
pub mod auth;
pub mod board;
pub mod init;
pub mod issue;
pub mod project;
pub mod queue;
pub mod sprint;
pub mod team;
pub mod worklog;
```

Add the `Queue` variant to the `Command` enum (after `Team`):

```rust
    /// Manage JSM queues
    Queue {
        #[command(subcommand)]
        command: QueueCommand,
    },
```

Add the `QueueCommand` enum after `WorklogCommand`:

```rust
#[derive(Subcommand)]
pub enum QueueCommand {
    /// List queues for the service desk
    List,
    /// View issues in a queue
    View {
        /// Queue name (partial match supported)
        name: Option<String>,
        /// Queue ID (use if name is ambiguous)
        #[arg(long)]
        id: Option<String>,
        /// Maximum number of issues to return
        #[arg(long)]
        limit: Option<u32>,
    },
}
```

- [ ] **Step 2: Create `src/cli/queue.rs`**

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::jsm::servicedesks;
use crate::cli::{OutputFormat, QueueCommand};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};

pub async fn handle(
    command: QueueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
) -> Result<()> {
    let project_key = config
        .project_key(project_override)
        .ok_or_else(|| {
            JrError::UserError(
                "No project configured. Run \"jr init\" or pass --project.".into(),
            )
        })?;

    let service_desk_id =
        servicedesks::require_service_desk(client, &project_key).await?;

    match command {
        QueueCommand::List => handle_list(&service_desk_id, output_format, client).await,
        QueueCommand::View { name, id, limit } => {
            handle_view(&service_desk_id, name, id, limit, output_format, client).await
        }
    }
}

async fn handle_list(
    service_desk_id: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let queues = client.list_queues(service_desk_id).await?;

    let rows: Vec<Vec<String>> = queues
        .iter()
        .map(|q| {
            vec![
                q.name.clone(),
                q.issue_count
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "—".into()),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["Queue", "Issues"],
        &rows,
        &queues,
    )
}

async fn handle_view(
    service_desk_id: &str,
    name: Option<String>,
    id: Option<String>,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    // Resolve the queue ID
    let queue_id = match id {
        Some(id) => id,
        None => {
            let name = name.ok_or_else(|| {
                JrError::UserError(
                    "Specify a queue name or use --id. \
                     Run \"jr queue list\" to see available queues."
                        .into(),
                )
            })?;
            resolve_queue_by_name(service_desk_id, &name, client).await?
        }
    };

    let issues = client
        .get_queue_issues(service_desk_id, &queue_id, limit)
        .await?;

    let rows: Vec<Vec<String>> = issues
        .iter()
        .map(|i| {
            vec![
                i.key.clone(),
                i.fields
                    .issuetype
                    .as_ref()
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "—".into()),
                i.fields
                    .summary
                    .clone()
                    .unwrap_or_else(|| "—".into()),
                i.fields
                    .status
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "—".into()),
                i.fields
                    .assignee
                    .as_ref()
                    .map(|u| u.display_name.clone())
                    .unwrap_or_else(|| "—".into()),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["Key", "Type", "Summary", "Status", "Assignee"],
        &rows,
        &issues,
    )
}

async fn resolve_queue_by_name(
    service_desk_id: &str,
    name: &str,
    client: &JiraClient,
) -> Result<String> {
    let queues = client.list_queues(service_desk_id).await?;
    let names: Vec<String> = queues.iter().map(|q| q.name.clone()).collect();

    match partial_match::partial_match(name, &names) {
        MatchResult::Exact(matched_name) => {
            // Find matching queues by this exact name
            let matching: Vec<&crate::types::jsm::Queue> =
                queues.iter().filter(|q| q.name == matched_name).collect();

            if matching.len() > 1 {
                // Duplicate queue names — need --id
                let ids: Vec<String> = matching.iter().map(|q| q.id.clone()).collect();
                Err(JrError::UserError(format!(
                    "Multiple queues named \"{}\" found (IDs: {}). Use --id {} to specify.",
                    matched_name,
                    ids.join(", "),
                    ids[0]
                ))
                .into())
            } else {
                Ok(matching[0].id.clone())
            }
        }
        MatchResult::Ambiguous(matches) => Err(JrError::UserError(format!(
            "\"{}\" matches multiple queues: {}. Be more specific or use --id.",
            name,
            matches
                .iter()
                .map(|m| format!("\"{}\"", m))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into()),
        MatchResult::None(_) => Err(JrError::UserError(format!(
            "No queue matching \"{}\" found. \
             Run \"jr queue list\" to see available queues.",
            name
        ))
        .into()),
    }
}
```

- [ ] **Step 3: Add dispatch in `src/main.rs`**

Add the `Queue` arm to the match block in the `run` function, after the `Team` arm:

```rust
            cli::Command::Queue { command } => {
                let config = config::Config::load()?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::queue::handle(
                    command,
                    &cli.output,
                    &config,
                    &client,
                    cli.project.as_deref(),
                )
                .await
            }
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo build
```

Expected: compiles with no errors.

- [ ] **Step 5: Run all tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 7: Run formatter**

```bash
cargo fmt --all -- --check
```

Expected: no formatting issues. If there are, run `cargo fmt --all`.

- [ ] **Step 8: Commit**

```bash
git add src/cli/queue.rs src/cli/mod.rs src/main.rs
git commit -m "feat: add jr queue list and jr queue view commands

New top-level queue command for JSM service desks.
- jr queue list: shows all queues with issue counts
- jr queue view <name>: shows issues in a queue with partial name matching
- --id flag for disambiguation when queue names are duplicates
- Auto-detects project type and errors for non-JSM projects
- JSON output support via --output json"
```

---

### Task 7: Update README and CLAUDE.md

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update README.md command table**

Add a row for the queue commands in the command table in `README.md`. Find the existing command table and add after the `worklog` entries:

```markdown
| `jr queue list`                  | List JSM queues for the project's service desk |
| `jr queue view <name>`           | View issues in a queue (partial name match)    |
```

- [ ] **Step 2: Update CLAUDE.md architecture section**

In the `src/` tree in `CLAUDE.md`:

Add under `├── cli/`:
```
│   ├── queue.rs         # queue list/view (JSM service desks)
```

Add new directory entries:
```
├── api/
│   ├── jsm/             # JSM-specific API call implementations
│   │   ├── servicedesks.rs  # list service desks, project meta orchestration
│   │   └── queues.rs        # list queues, get queue issues
├── types/jsm/           # Serde structs for JSM API responses (ServiceDesk, Queue, etc.)
```

- [ ] **Step 3: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: add queue commands to README and CLAUDE.md

Document jr queue list and jr queue view. Update architecture
section with new api/jsm/ and types/jsm/ directories."
```

---

### Task 8: Queue name matching unit tests

**Files:**
- Modify: `src/cli/queue.rs`

- [ ] **Step 1: Add unit tests for queue name resolution**

Add at the bottom of `src/cli/queue.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::types::jsm::Queue;

    fn make_queue(id: &str, name: &str) -> Queue {
        Queue {
            id: id.into(),
            name: name.into(),
            jql: None,
            fields: None,
            issue_count: None,
        }
    }

    fn find_queue_id(name: &str, queues: &[Queue]) -> Result<String, String> {
        let names: Vec<String> = queues.iter().map(|q| q.name.clone()).collect();
        match crate::partial_match::partial_match(name, &names) {
            crate::partial_match::MatchResult::Exact(matched_name) => {
                let matching: Vec<&Queue> =
                    queues.iter().filter(|q| q.name == matched_name).collect();
                if matching.len() > 1 {
                    Err(format!("duplicate: {}", matching.len()))
                } else {
                    Ok(matching[0].id.clone())
                }
            }
            crate::partial_match::MatchResult::Ambiguous(m) => {
                Err(format!("ambiguous: {}", m.len()))
            }
            crate::partial_match::MatchResult::None(_) => Err("none".into()),
        }
    }

    #[test]
    fn exact_match() {
        let queues = vec![make_queue("10", "Triage"), make_queue("20", "In Progress")];
        assert_eq!(find_queue_id("Triage", &queues).unwrap(), "10");
    }

    #[test]
    fn partial_match() {
        let queues = vec![make_queue("10", "Triage"), make_queue("20", "In Progress")];
        assert_eq!(find_queue_id("tri", &queues).unwrap(), "10");
    }

    #[test]
    fn ambiguous_match() {
        let queues = vec![
            make_queue("10", "Escalated - Client"),
            make_queue("20", "Escalated - External"),
        ];
        let err = find_queue_id("esc", &queues).unwrap_err();
        assert!(err.starts_with("ambiguous"));
    }

    #[test]
    fn no_match() {
        let queues = vec![make_queue("10", "Triage")];
        let err = find_queue_id("nonexistent", &queues).unwrap_err();
        assert_eq!(err, "none");
    }

    #[test]
    fn duplicate_names() {
        let queues = vec![make_queue("10", "Triage"), make_queue("20", "Triage")];
        let err = find_queue_id("Triage", &queues).unwrap_err();
        assert!(err.starts_with("duplicate"));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --lib cli::queue -- --nocapture
```

Expected: all 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/cli/queue.rs
git commit -m "test: add unit tests for queue name matching

Tests exact match, partial match, ambiguous match, no match,
and duplicate queue name scenarios."
```

---

### Task 9: Project meta integration tests

**Files:**
- Create: `tests/project_meta.rs`

- [ ] **Step 1: Create integration tests**

Create `tests/project_meta.rs`:

```rust
#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn project_meta_cache_miss_fetches_from_api() {
    let server = MockServer::start().await;

    // Mock platform API for project details
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/HELPDESK"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10042",
            "key": "HELPDESK",
            "name": "Help Desk",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Mock servicedeskapi for service desk list
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [
                { "id": "15", "projectId": "10042", "projectName": "Help Desk" }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let meta = jr::api::jsm::servicedesks::get_or_fetch_project_meta(&client, "HELPDESK")
        .await
        .unwrap();

    assert_eq!(meta.project_type, "service_desk");
    assert_eq!(meta.project_id, "10042");
    assert_eq!(meta.service_desk_id.as_deref(), Some("15"));
    assert!(!meta.simplified);
}

#[tokio::test]
async fn project_meta_software_project_has_no_service_desk_id() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10001",
            "key": "DEV",
            "name": "Development",
            "projectTypeKey": "software",
            "simplified": true
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let meta = jr::api::jsm::servicedesks::get_or_fetch_project_meta(&client, "DEV")
        .await
        .unwrap();

    assert_eq!(meta.project_type, "software");
    assert!(meta.service_desk_id.is_none());
    assert!(meta.simplified);
}

#[tokio::test]
async fn require_service_desk_errors_for_software_project() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/DEV"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10001",
            "key": "DEV",
            "name": "Development",
            "projectTypeKey": "software",
            "simplified": true
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let result = jr::api::jsm::servicedesks::require_service_desk(&client, "DEV").await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Jira Software project"));
    assert!(err.contains("Queue commands require"));
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --test project_meta -- --nocapture
```

Expected: all 3 tests pass.

Note: These tests use `new_for_test` which sets `instance_url == base_url`, so `get_from_instance` and `get` both hit the wiremock server. This correctly tests the behavior.

- [ ] **Step 3: Commit**

```bash
git add tests/project_meta.rs
git commit -m "test: add integration tests for project meta detection

Tests cache-miss-to-API flow for JSM and software projects,
and require_service_desk error for non-JSM projects."
```
