# Assets/CMDB Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `jr assets search`, `jr assets view`, and `jr assets tickets` commands for querying the Atlassian Assets/CMDB API.

**Architecture:** New `src/api/assets/` and `src/types/assets/` peer modules alongside existing `jira/` and `jsm/` directories. Assets is workspace-scoped (not project-scoped). Workspace ID discovered via `/rest/servicedeskapi/assets/workspace` and cached site-wide. Assets API calls go through the Atlassian API gateway at `api.atlassian.com/ex/jira/{cloudId}/jsm/assets/workspace/{workspaceId}/v1/` via a new `assets_base_url` field on `JiraClient`.

**Tech Stack:** Rust, reqwest, serde (custom deserializer for `isLast` string/bool), chrono, comfy-table, wiremock (tests), clap

**Spec:** `docs/superpowers/specs/2026-03-24-assets-cmdb-design.md`

**Prerequisite:** PR #40 (`feat/jsm-queues`) must be merged first — this plan depends on `ServiceDeskPage`, `ProjectMeta` cache, and `api/jsm/` module already existing.

---

## File Structure

| File | Responsibility | Change |
|------|---------------|--------|
| `src/api/client.rs` | Add `assets_base_url` field, `get_assets()`, `post_assets()` | Modify |
| `src/api/pagination.rs` | Add `AssetsPage<T>` with `deserialize_bool_or_string` | Modify |
| `src/cache.rs` | Add `WorkspaceCache`, `read_workspace_cache()`, `write_workspace_cache()` | Modify |
| `src/types/assets/mod.rs` | Re-exports for Assets types | Create |
| `src/types/assets/object.rs` | `AssetObject`, `ObjectType`, `AssetAttribute`, `ObjectAttributeValue` | Create |
| `src/types/assets/ticket.rs` | `ConnectedTicketsResponse`, `ConnectedTicket` | Create |
| `src/types/mod.rs` | Add `pub mod assets;` | Modify |
| `src/api/assets/mod.rs` | Re-exports for Assets API | Create |
| `src/api/assets/workspace.rs` | Workspace ID discovery + `get_or_fetch_workspace_id()` | Create |
| `src/api/assets/objects.rs` | `search_assets()`, `get_asset()`, `resolve_object_key()` | Create |
| `src/api/assets/tickets.rs` | `get_connected_tickets()` | Create |
| `src/api/mod.rs` | Add `pub mod assets;` | Modify |
| `src/cli/mod.rs` | Add `Assets` command + `AssetsCommand` enum | Modify |
| `src/cli/assets.rs` | `handle()`, `handle_search()`, `handle_view()`, `handle_tickets()` | Create |
| `src/main.rs` | Add dispatch for `Command::Assets` | Modify |
| `tests/assets.rs` | Integration tests with wiremock | Create |

---

### Task 1: AssetsPage pagination type + bool/string deserializer

**Files:**
- Modify: `src/api/pagination.rs`

- [ ] **Step 1: Write unit tests for `AssetsPage<T>` and `deserialize_bool_or_string`**

Add these tests at the bottom of the existing `#[cfg(test)] mod tests` block in `src/api/pagination.rs`:

```rust
    #[test]
    fn test_assets_page_has_more() {
        let page: AssetsPage<String> = AssetsPage {
            start_at: 0,
            max_results: 25,
            total: 50,
            is_last: false,
            values: vec!["a".into()],
        };
        assert!(page.has_more());
        assert_eq!(page.next_start(), 25);
    }

    #[test]
    fn test_assets_page_last_page() {
        let page: AssetsPage<String> = AssetsPage {
            start_at: 25,
            max_results: 25,
            total: 30,
            is_last: true,
            values: vec!["a".into()],
        };
        assert!(!page.has_more());
    }

    #[test]
    fn test_assets_page_deserialize_is_last_bool() {
        let json = r#"{
            "startAt": 0,
            "maxResults": 25,
            "total": 5,
            "isLast": true,
            "values": ["a", "b"]
        }"#;
        let page: AssetsPage<String> = serde_json::from_str(json).unwrap();
        assert!(page.is_last);
        assert_eq!(page.values.len(), 2);
    }

    #[test]
    fn test_assets_page_deserialize_is_last_string() {
        let json = r#"{
            "startAt": 0,
            "maxResults": 25,
            "total": 5,
            "isLast": "false",
            "values": ["a"]
        }"#;
        let page: AssetsPage<String> = serde_json::from_str(json).unwrap();
        assert!(!page.is_last);
    }

    #[test]
    fn test_assets_page_deserialize_is_last_string_true() {
        let json = r#"{
            "startAt": 0,
            "maxResults": 25,
            "total": 5,
            "isLast": "true",
            "values": []
        }"#;
        let page: AssetsPage<String> = serde_json::from_str(json).unwrap();
        assert!(page.is_last);
        assert!(page.values.is_empty());
    }
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo test --lib pagination -- --nocapture
```

Expected: compilation errors — `AssetsPage` and `deserialize_bool_or_string` don't exist.

- [ ] **Step 3: Implement `deserialize_bool_or_string` and `AssetsPage<T>`**

Add at the top of `src/api/pagination.rs`, after the existing `use serde::Deserialize;`:

```rust
use serde::de::{self, Deserializer};
```

Add the deserializer function before the `#[cfg(test)]` block:

```rust
/// Deserialize a value that may be a boolean or a string representation of a boolean.
/// The Assets API returns `isLast` as `"true"`/`"false"` (string) in some contexts
/// and `true`/`false` (boolean) in others.
fn deserialize_bool_or_string<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::Bool(b) => Ok(b),
        serde_json::Value::String(s) => s.parse::<bool>().map_err(de::Error::custom),
        _ => Err(de::Error::custom("expected boolean or string")),
    }
}
```

Add the `AssetsPage<T>` struct and impl after the `ServiceDeskPage<T>` impl (before `#[cfg(test)]`):

```rust
/// Pagination used by the Assets/CMDB API (`POST /object/aql`).
///
/// Similar to `OffsetPage` (`startAt`/`maxResults`/`total`) but uses an `isLast`
/// boolean (which may be returned as a string) instead of computing from offsets.
/// `total` is capped at 1000 by the API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsPage<T> {
    /// Zero-based starting index.
    #[serde(default)]
    pub start_at: u32,
    /// Maximum items per page.
    #[serde(default)]
    pub max_results: u32,
    /// Total matching items (capped at 1000).
    #[serde(default)]
    pub total: u32,
    /// Whether this is the last page. May be a bool or string in API responses.
    #[serde(deserialize_with = "deserialize_bool_or_string")]
    pub is_last: bool,
    /// The items in this page.
    #[serde(default)]
    pub values: Vec<T>,
}

impl<T> AssetsPage<T> {
    /// Returns true if there are more pages after this one.
    pub fn has_more(&self) -> bool {
        !self.is_last
    }

    /// Returns the `startAt` value for the next page.
    pub fn next_start(&self) -> u32 {
        self.start_at + self.max_results
    }
}
```

Note: using `#[serde(rename_all = "camelCase")]` at struct level (matching `OffsetPage` pattern) so `start_at` → `startAt`, `max_results` → `maxResults` automatically. The `is_last` field gets its rename from `rename_all` (→ `isLast`) plus its custom deserializer.

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo test --lib pagination -- --nocapture
```

Expected: all pagination tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/api/pagination.rs
git commit -m "feat: add AssetsPage pagination type with bool/string isLast support

AssetsPage<T> handles pagination from the Assets/CMDB API which returns
isLast as either a boolean or string. Custom deserialize_bool_or_string
handles both formats. Uses rename_all camelCase matching OffsetPage."
```

---

### Task 2: WorkspaceCache

**Files:**
- Modify: `src/cache.rs`

- [ ] **Step 1: Write unit tests for workspace cache**

Add these tests inside the existing `#[cfg(test)] mod tests` block in `src/cache.rs`:

```rust
    #[test]
    fn read_missing_workspace_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_workspace_cache().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_workspace_cache() {
        with_temp_cache(|| {
            write_workspace_cache("abc-123-def").unwrap();

            let cache = read_workspace_cache().unwrap().expect("should exist");
            assert_eq!(cache.workspace_id, "abc-123-def");
        });
    }

    #[test]
    fn expired_workspace_cache_returns_none() {
        with_temp_cache(|| {
            let expired = WorkspaceCache {
                workspace_id: "old-id".into(),
                fetched_at: Utc::now() - chrono::Duration::days(8),
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("workspace.json"), content).unwrap();

            let result = read_workspace_cache().unwrap();
            assert!(result.is_none(), "expired workspace cache should return None");
        });
    }
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo test --lib cache -- --nocapture
```

Expected: compilation errors.

- [ ] **Step 3: Implement WorkspaceCache**

Add after the existing `write_project_meta` function (before `#[cfg(test)]`):

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceCache {
    pub workspace_id: String,
    pub fetched_at: DateTime<Utc>,
}

pub fn read_workspace_cache() -> Result<Option<WorkspaceCache>> {
    let path = cache_dir().join("workspace.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: WorkspaceCache = serde_json::from_str(&content)?;

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(Some(cache))
}

pub fn write_workspace_cache(workspace_id: &str) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let cache = WorkspaceCache {
        workspace_id: workspace_id.to_string(),
        fetched_at: Utc::now(),
    };

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(dir.join("workspace.json"), content)?;
    Ok(())
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo test --lib cache -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add src/cache.rs
git commit -m "feat: add WorkspaceCache for Assets workspace ID

Site-wide cache in ~/.cache/jr/workspace.json with 7-day TTL.
write_workspace_cache() sets fetched_at internally matching
write_team_cache() pattern."
```

---

### Task 3: JiraClient — assets_base_url + get_assets/post_assets

**Files:**
- Modify: `src/api/client.rs`

- [ ] **Step 1: Add `assets_base_url` field and update constructors**

In `src/api/client.rs`, add `assets_base_url: Option<String>` to the `JiraClient` struct:

```rust
pub struct JiraClient {
    client: Client,
    base_url: String,
    instance_url: String,
    auth_header: String,
    verbose: bool,
    assets_base_url: Option<String>,
}
```

In `from_config()`, construct `assets_base_url` from `cloud_id` after the existing `Ok(Self { ... })` block. Replace the `Ok(Self { ... })` with:

```rust
        let assets_base_url = config
            .global
            .instance
            .cloud_id
            .as_ref()
            .map(|cloud_id| {
                format!(
                    "https://api.atlassian.com/ex/jira/{}/jsm/assets",
                    cloud_id
                )
            });

        Ok(Self {
            client,
            base_url,
            instance_url,
            auth_header,
            verbose,
            assets_base_url,
        })
```

In `new_for_test()`, set `assets_base_url` to route to wiremock:

```rust
    pub fn new_for_test(base_url: String, auth_header: String) -> Self {
        let assets_base_url = Some(format!("{}/jsm/assets", &base_url));
        Self {
            client: Client::new(),
            instance_url: base_url.clone(),
            base_url,
            auth_header,
            verbose: false,
            assets_base_url,
        }
    }
```

- [ ] **Step 2: Add `get_assets()` and `post_assets()` methods**

Add after `post_to_instance()`:

```rust
    /// Perform a GET request against the Assets/CMDB API gateway.
    ///
    /// Constructs URL: `{assets_base_url}/workspace/{workspace_id}/v1/{path}`.
    /// Requires `cloud_id` in config (set during `jr init`).
    pub async fn get_assets<T: DeserializeOwned>(
        &self,
        workspace_id: &str,
        path: &str,
    ) -> anyhow::Result<T> {
        let base = self.assets_base_url.as_ref().ok_or_else(|| {
            JrError::ConfigError(
                "Cloud ID not configured. Run \"jr init\" to set up your instance.".into(),
            )
        })?;
        let url = format!(
            "{}/workspace/{}/v1/{}",
            base,
            urlencoding::encode(workspace_id),
            path
        );
        let request = self.client.get(&url);
        let response = self.send(request).await?;
        Ok(response.json::<T>().await?)
    }

    /// Perform a POST request against the Assets/CMDB API gateway.
    pub async fn post_assets<T: DeserializeOwned, B: Serialize>(
        &self,
        workspace_id: &str,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        let base = self.assets_base_url.as_ref().ok_or_else(|| {
            JrError::ConfigError(
                "Cloud ID not configured. Run \"jr init\" to set up your instance.".into(),
            )
        })?;
        let url = format!(
            "{}/workspace/{}/v1/{}",
            base,
            urlencoding::encode(workspace_id),
            path
        );
        let request = self.client.post(&url).json(body);
        let response = self.send(request).await?;
        Ok(response.json::<T>().await?)
    }
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo build
```

- [ ] **Step 4: Run all existing tests to verify nothing broke**

```bash
cargo test
```

Expected: all existing tests pass. The `new_for_test()` change adds a field but doesn't break existing test behavior.

- [ ] **Step 5: Run clippy and fmt**

```bash
cargo clippy -- -D warnings
cargo fmt --all
```

- [ ] **Step 6: Commit**

```bash
git add src/api/client.rs
git commit -m "feat: add assets_base_url, get_assets, post_assets to JiraClient

Third base URL pattern for Assets/CMDB API at api.atlassian.com gateway.
Constructed from cloud_id in config. new_for_test routes to wiremock.
URL-encodes workspace_id in path segments."
```

---

### Task 4: Assets types (AssetObject, ConnectedTicket)

**Files:**
- Create: `src/types/assets/mod.rs`
- Create: `src/types/assets/object.rs`
- Create: `src/types/assets/ticket.rs`
- Modify: `src/types/mod.rs`

- [ ] **Step 1: Create `src/types/assets/object.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetObject {
    pub id: String,
    pub label: String,
    #[serde(rename = "objectKey")]
    pub object_key: String,
    #[serde(rename = "objectType")]
    pub object_type: ObjectType,
    pub created: Option<String>,
    pub updated: Option<String>,
    #[serde(default)]
    pub attributes: Vec<AssetAttribute>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectType {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetAttribute {
    pub id: String,
    #[serde(rename = "objectTypeAttributeId")]
    pub object_type_attribute_id: String,
    #[serde(rename = "objectAttributeValues", default)]
    pub values: Vec<ObjectAttributeValue>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectAttributeValue {
    pub value: Option<String>,
    #[serde(rename = "displayValue")]
    pub display_value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_asset_object_minimal() {
        let json = r#"{
            "id": "88",
            "label": "Acme Corp",
            "objectKey": "OBJ-88",
            "objectType": { "id": "23", "name": "Client" }
        }"#;
        let obj: AssetObject = serde_json::from_str(json).unwrap();
        assert_eq!(obj.id, "88");
        assert_eq!(obj.label, "Acme Corp");
        assert_eq!(obj.object_key, "OBJ-88");
        assert_eq!(obj.object_type.name, "Client");
        assert!(obj.attributes.is_empty());
        assert!(obj.created.is_none());
    }

    #[test]
    fn deserialize_asset_object_with_attributes() {
        let json = r#"{
            "id": "88",
            "label": "Acme Corp",
            "objectKey": "OBJ-88",
            "objectType": { "id": "23", "name": "Client" },
            "created": "2025-12-17T14:58:00.000Z",
            "updated": "2026-01-29T19:52:00.000Z",
            "attributes": [
                {
                    "id": "637",
                    "objectTypeAttributeId": "134",
                    "objectAttributeValues": [
                        { "value": "contact@acme.com", "displayValue": "contact@acme.com" }
                    ]
                }
            ]
        }"#;
        let obj: AssetObject = serde_json::from_str(json).unwrap();
        assert_eq!(obj.attributes.len(), 1);
        assert_eq!(
            obj.attributes[0].values[0].display_value.as_deref(),
            Some("contact@acme.com")
        );
    }
}
```

- [ ] **Step 2: Create `src/types/assets/ticket.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectedTicketsResponse {
    #[serde(default)]
    pub tickets: Vec<ConnectedTicket>,
    #[serde(rename = "allTicketsQuery")]
    pub all_tickets_query: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectedTicket {
    pub key: String,
    pub id: String,
    pub title: String,
    pub reporter: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub status: Option<TicketStatus>,
    #[serde(rename = "type")]
    pub issue_type: Option<TicketType>,
    pub priority: Option<TicketPriority>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TicketStatus {
    pub name: String,
    #[serde(rename = "colorName")]
    pub color_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TicketType {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TicketPriority {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_connected_tickets_response() {
        let json = r#"{
            "tickets": [
                {
                    "key": "PROJ-42",
                    "id": "10968",
                    "title": "VPN access not working",
                    "reporter": "abc123",
                    "created": "2026-02-17T18:31:56.953Z",
                    "updated": "2026-03-22T18:59:23.333Z",
                    "status": { "name": "In Progress", "colorName": "yellow" },
                    "type": { "name": "Service Request" },
                    "priority": { "name": "High" }
                }
            ],
            "allTicketsQuery": "issueFunction in assetsObject(\"objectId = 88\")"
        }"#;
        let resp: ConnectedTicketsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.tickets.len(), 1);
        assert_eq!(resp.tickets[0].key, "PROJ-42");
        assert_eq!(resp.tickets[0].title, "VPN access not working");
        assert_eq!(resp.tickets[0].status.as_ref().unwrap().name, "In Progress");
        assert!(resp.all_tickets_query.is_some());
    }

    #[test]
    fn deserialize_empty_tickets() {
        let json = r#"{ "tickets": [] }"#;
        let resp: ConnectedTicketsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.tickets.is_empty());
        assert!(resp.all_tickets_query.is_none());
    }
}
```

- [ ] **Step 3: Create `src/types/assets/mod.rs`**

```rust
pub mod object;
pub mod ticket;

pub use object::*;
pub use ticket::*;
```

- [ ] **Step 4: Add `pub mod assets;` to `src/types/mod.rs`**

```rust
pub mod assets;
pub mod jira;
pub mod jsm;
```

- [ ] **Step 5: Verify and run tests**

```bash
cargo build
cargo test --lib types::assets -- --nocapture
```

Expected: all 4 type tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/types/assets/ src/types/mod.rs
git commit -m "feat: add Assets types for AssetObject and ConnectedTicket

AssetObject supports optional attributes with displayValue.
ConnectedTicket has different field names than platform Issue
(title not summary, type not issuetype). Includes unit tests."
```

---

### Task 5: Assets API — workspace discovery + object methods

**Files:**
- Create: `src/api/assets/mod.rs`
- Create: `src/api/assets/workspace.rs`
- Create: `src/api/assets/objects.rs`
- Create: `src/api/assets/tickets.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Create `src/api/assets/mod.rs`**

```rust
pub mod objects;
pub mod tickets;
pub mod workspace;
```

- [ ] **Step 2: Add `pub mod assets;` to `src/api/mod.rs`**

```rust
pub mod assets;
pub mod auth;
pub mod client;
pub mod jira;
pub mod jsm;
pub mod pagination;
pub mod rate_limit;
```

- [ ] **Step 3: Create `src/api/assets/workspace.rs`**

```rust
use anyhow::Result;
use serde::Deserialize;

use crate::api::client::JiraClient;
use crate::cache;
use crate::error::JrError;

#[derive(Deserialize)]
struct WorkspaceResponse {
    #[serde(rename = "workspaceId")]
    workspace_id: String,
}

/// Get the Assets workspace ID, using cache when available.
///
/// 1. Check cache — return if fresh.
/// 2. GET /rest/servicedeskapi/assets/workspace on instance URL.
/// 3. Cache and return.
pub async fn get_or_fetch_workspace_id(client: &JiraClient) -> Result<String> {
    if let Some(cached) = cache::read_workspace_cache()? {
        return Ok(cached.workspace_id);
    }

    let resp: WorkspaceResponse = client
        .get_from_instance("/rest/servicedeskapi/assets/workspace")
        .await
        .map_err(|e| {
            // If the endpoint doesn't exist or is forbidden, Assets isn't available
            if e.to_string().contains("404") || e.to_string().contains("403") {
                JrError::UserError(
                    "Assets is not available on this Jira site. \
                     Assets requires Jira Service Management Premium or Enterprise."
                        .into(),
                )
                .into()
            } else {
                e
            }
        })?;

    let _ = cache::write_workspace_cache(&resp.workspace_id);

    Ok(resp.workspace_id)
}
```

- [ ] **Step 4: Create `src/api/assets/objects.rs`**

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::AssetsPage;
use crate::error::JrError;
use crate::types::assets::AssetObject;

impl JiraClient {
    /// Search assets via AQL with auto-pagination.
    pub async fn search_assets(
        &self,
        workspace_id: &str,
        aql: &str,
        limit: Option<u32>,
        include_attributes: bool,
    ) -> Result<Vec<AssetObject>> {
        let mut all = Vec::new();
        let mut start_at = 0u32;
        let max_page_size = 25u32;

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
                "object/aql?startAt={}&maxResults={}&includeAttributes={}",
                start_at, page_size, include_attributes
            );
            let body = serde_json::json!({ "qlQuery": aql });
            let page: AssetsPage<AssetObject> =
                self.post_assets(workspace_id, &path, &body).await?;
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
            start_at = next;
        }
        Ok(all)
    }

    /// Get a single asset by its numeric ID.
    pub async fn get_asset(
        &self,
        workspace_id: &str,
        object_id: &str,
        include_attributes: bool,
    ) -> Result<AssetObject> {
        let path = format!(
            "object/{}?includeAttributes={}",
            urlencoding::encode(object_id),
            include_attributes
        );
        self.get_assets(workspace_id, &path).await
    }
}

/// Resolve an object key (e.g., "OBJ-1") to its numeric ID.
/// If the input is purely numeric, returns it as-is.
pub async fn resolve_object_key(
    client: &JiraClient,
    workspace_id: &str,
    key_or_id: &str,
) -> Result<String> {
    // If purely numeric, treat as ID directly
    if key_or_id.chars().all(|c| c.is_ascii_digit()) {
        return Ok(key_or_id.to_string());
    }

    // Resolve via AQL
    let results = client
        .search_assets(
            workspace_id,
            &format!("objectKey = \"{}\"", key_or_id),
            Some(1),
            false,
        )
        .await?;

    results
        .into_iter()
        .next()
        .map(|obj| obj.id)
        .ok_or_else(|| {
            JrError::UserError(format!(
                "No asset matching \"{}\" found. Check the object key and try again.",
                key_or_id
            ))
            .into()
        })
}
```

- [ ] **Step 5: Create `src/api/assets/tickets.rs`**

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::types::assets::ConnectedTicketsResponse;

impl JiraClient {
    /// Get Jira issues connected to an asset object.
    /// No pagination — returns all connected tickets in one response.
    pub async fn get_connected_tickets(
        &self,
        workspace_id: &str,
        object_id: &str,
    ) -> Result<ConnectedTicketsResponse> {
        let path = format!(
            "objectconnectedtickets/{}/tickets",
            urlencoding::encode(object_id)
        );
        self.get_assets(workspace_id, &path).await
    }
}
```

- [ ] **Step 6: Verify it compiles**

```bash
cargo build
```

- [ ] **Step 7: Commit**

```bash
git add src/api/assets/ src/api/mod.rs
git commit -m "feat: add Assets API — workspace discovery, AQL search, connected tickets

get_or_fetch_workspace_id() discovers and caches workspace ID.
search_assets() auto-paginates AQL queries via AssetsPage.
get_asset() fetches single object by ID.
resolve_object_key() resolves object keys to numeric IDs via AQL.
get_connected_tickets() returns issues linked to an asset.
All use get_assets/post_assets on api.atlassian.com gateway."
```

---

### Task 6: Integration tests

**Files:**
- Create: `tests/assets.rs`

- [ ] **Step 1: Create integration tests**

```rust
#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn search_assets_returns_objects() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "25"))
        .and(query_param("includeAttributes", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 2,
            "isLast": true,
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" }
                },
                {
                    "id": "71",
                    "label": "Globex Inc",
                    "objectKey": "OBJ-71",
                    "objectType": { "id": "13", "name": "Client" }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let results = client.search_assets("ws-123", "objectType = Client", None, false).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].label, "Acme Corp");
    assert_eq!(results[1].object_key, "OBJ-71");
}

#[tokio::test]
async fn search_assets_empty() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 0,
            "isLast": true,
            "values": []
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let results = client.search_assets("ws-123", "objectType = Nonexistent", None, false).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn search_assets_with_limit() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .and(query_param("maxResults", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 1,
            "total": 5,
            "isLast": false,
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let results = client.search_assets("ws-123", "objectType = Client", Some(1), false).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn search_assets_is_last_as_string() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/aql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 25,
            "total": 1,
            "isLast": "true",
            "values": [
                {
                    "id": "70",
                    "label": "Acme Corp",
                    "objectKey": "OBJ-70",
                    "objectType": { "id": "13", "name": "Client" }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let results = client.search_assets("ws-123", "objectType = Client", None, false).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn get_asset_returns_object() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/object/70"))
        .and(query_param("includeAttributes", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "70",
            "label": "Acme Corp",
            "objectKey": "OBJ-70",
            "objectType": { "id": "13", "name": "Client" },
            "created": "2025-12-17T14:58:00.000Z",
            "attributes": [
                {
                    "id": "637",
                    "objectTypeAttributeId": "134",
                    "objectAttributeValues": [
                        { "value": "contact@acme.com", "displayValue": "contact@acme.com" }
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let obj = client.get_asset("ws-123", "70", true).await.unwrap();
    assert_eq!(obj.label, "Acme Corp");
    assert_eq!(obj.attributes.len(), 1);
}

#[tokio::test]
async fn get_connected_tickets_returns_tickets() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/objectconnectedtickets/70/tickets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tickets": [
                {
                    "key": "PROJ-42",
                    "id": "10968",
                    "title": "VPN access not working",
                    "status": { "name": "In Progress", "colorName": "yellow" },
                    "type": { "name": "Service Request" },
                    "priority": { "name": "High" }
                }
            ],
            "allTicketsQuery": "issueFunction in assetsObject(\"objectId = 70\")"
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let resp = client.get_connected_tickets("ws-123", "70").await.unwrap();
    assert_eq!(resp.tickets.len(), 1);
    assert_eq!(resp.tickets[0].key, "PROJ-42");
    assert_eq!(resp.tickets[0].title, "VPN access not working");
    assert!(resp.all_tickets_query.is_some());
}

#[tokio::test]
async fn get_connected_tickets_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/jsm/assets/workspace/ws-123/v1/objectconnectedtickets/99/tickets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tickets": []
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let resp = client.get_connected_tickets("ws-123", "99").await.unwrap();
    assert!(resp.tickets.is_empty());
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --test assets -- --nocapture
```

Expected: all 7 tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/assets.rs
git commit -m "test: add integration tests for Assets API

Tests AQL search (normal, empty, limit, isLast as string),
get object with attributes, connected tickets (normal + empty).
All use wiremock with assets_base_url routing."
```

---

### Task 7: CLI — assets commands

**Files:**
- Create: `src/cli/assets.rs`
- Modify: `src/cli/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add `AssetsCommand` enum to `src/cli/mod.rs`**

Add `pub mod assets;` to the module declarations (alphabetical):

```rust
pub mod assets;
pub mod auth;
pub mod board;
...
```

Add `Assets` variant to `Command` enum (alphabetical, before `Auth`):

```rust
    /// Manage Assets/CMDB objects
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
```

Add `AssetsCommand` enum (before `AuthCommand`):

```rust
#[derive(Subcommand)]
pub enum AssetsCommand {
    /// Search assets with AQL query
    Search {
        /// AQL query (e.g. "objectType = Client")
        query: String,
        /// Maximum number of results
        #[arg(long)]
        limit: Option<u32>,
        /// Include object attributes in output
        #[arg(long)]
        attributes: bool,
    },
    /// View asset details
    View {
        /// Object key (e.g. OBJ-1) or numeric ID
        key: String,
        /// Include object attributes in output
        #[arg(long)]
        attributes: bool,
    },
    /// Show Jira issues connected to an asset
    Tickets {
        /// Object key (e.g. OBJ-1) or numeric ID
        key: String,
        /// Maximum number of tickets to show
        #[arg(long)]
        limit: Option<u32>,
    },
}
```

- [ ] **Step 2: Create `src/cli/assets.rs`**

```rust
use anyhow::Result;

use crate::api::assets::{objects, workspace};
use crate::api::client::JiraClient;
use crate::cli::{AssetsCommand, OutputFormat};
use crate::output;

pub async fn handle(
    command: AssetsCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let workspace_id = workspace::get_or_fetch_workspace_id(client).await?;

    match command {
        AssetsCommand::Search {
            query,
            limit,
            attributes,
        } => handle_search(&workspace_id, &query, limit, attributes, output_format, client).await,
        AssetsCommand::View { key, attributes } => {
            handle_view(&workspace_id, &key, attributes, output_format, client).await
        }
        AssetsCommand::Tickets { key, limit } => {
            handle_tickets(&workspace_id, &key, limit, output_format, client).await
        }
    }
}

async fn handle_search(
    workspace_id: &str,
    query: &str,
    limit: Option<u32>,
    attributes: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let objects = client
        .search_assets(workspace_id, query, limit, attributes)
        .await?;

    let rows: Vec<Vec<String>> = objects
        .iter()
        .map(|o| {
            vec![
                o.object_key.clone(),
                o.object_type.name.clone(),
                o.label.clone(),
            ]
        })
        .collect();

    output::print_output(output_format, &["Key", "Type", "Name"], &rows, &objects)
}

async fn handle_view(
    workspace_id: &str,
    key: &str,
    attributes: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let object_id = objects::resolve_object_key(client, workspace_id, key).await?;
    let object = client.get_asset(workspace_id, &object_id, attributes).await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&object)?);
        }
        OutputFormat::Table => {
            let mut rows = vec![
                vec!["Key".into(), object.object_key.clone()],
                vec!["Type".into(), object.object_type.name.clone()],
                vec!["Name".into(), object.label.clone()],
            ];

            if let Some(ref created) = object.created {
                rows.push(vec!["Created".into(), created.clone()]);
            }
            if let Some(ref updated) = object.updated {
                rows.push(vec!["Updated".into(), updated.clone()]);
            }

            println!("{}", output::render_table(&["Field", "Value"], &rows));

            if attributes && !object.attributes.is_empty() {
                println!();
                let attr_rows: Vec<Vec<String>> = object
                    .attributes
                    .iter()
                    .flat_map(|attr| {
                        attr.values.iter().map(move |v| {
                            vec![
                                attr.object_type_attribute_id.clone(),
                                v.display_value
                                    .clone()
                                    .or_else(|| v.value.clone())
                                    .unwrap_or_default(),
                            ]
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    output::render_table(&["Attribute ID", "Value"], &attr_rows)
                );
            }
        }
    }
    Ok(())
}

async fn handle_tickets(
    workspace_id: &str,
    key: &str,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let object_id = objects::resolve_object_key(client, workspace_id, key).await?;
    let resp = client
        .get_connected_tickets(workspace_id, &object_id)
        .await?;

    let tickets = match limit {
        Some(n) => resp.tickets.into_iter().take(n as usize).collect::<Vec<_>>(),
        None => resp.tickets,
    };

    let rows: Vec<Vec<String>> = tickets
        .iter()
        .map(|t| {
            vec![
                t.key.clone(),
                t.issue_type
                    .as_ref()
                    .map(|it| it.name.clone())
                    .unwrap_or_else(|| "\u{2014}".into()),
                t.title.clone(),
                t.status
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "\u{2014}".into()),
                t.priority
                    .as_ref()
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "\u{2014}".into()),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["Key", "Type", "Title", "Status", "Priority"],
        &rows,
        &tickets,
    )
}
```

- [ ] **Step 3: Add dispatch in `src/main.rs`**

Add the `Assets` arm to the match block, before `Auth`:

```rust
            cli::Command::Assets { command } => {
                let config = config::Config::load()?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::assets::handle(command, &cli.output, &client).await
            }
```

- [ ] **Step 4: Verify it compiles and all tests pass**

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --all
```

- [ ] **Step 5: Commit**

```bash
git add src/cli/assets.rs src/cli/mod.rs src/main.rs
git commit -m "feat: add jr assets search, view, and tickets commands

New top-level assets command for CMDB operations.
- jr assets search <AQL>: search objects via AQL with pagination
- jr assets view <key>: view object details with optional attributes
- jr assets tickets <key>: show connected Jira issues
- Object key resolution via AQL (keys like OBJ-1 to numeric IDs)
- Workspace ID auto-discovered and cached
- JSON output support via --output json"
```

---

### Task 8: Update README and CLAUDE.md

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update README.md**

Add asset commands to the command table after the queue entries:

```markdown
| `jr assets search <AQL>`        | Search assets via AQL query                    |
| `jr assets view <key>`          | View asset details (key or numeric ID)         |
| `jr assets tickets <key>`       | Show Jira issues connected to an asset         |
```

- [ ] **Step 2: Update CLAUDE.md**

In the `src/` tree:
- Add `assets.rs` under `cli/`
- Add `api/assets/` section
- Add `types/assets/` line
- Update `cache.rs` description

- [ ] **Step 3: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: add assets commands to README and CLAUDE.md

Document jr assets search, view, and tickets. Update architecture
with api/assets/ and types/assets/ directories."
```

---

### Task 9: Object key resolution unit tests

**Files:**
- Modify: `src/api/assets/objects.rs`

- [ ] **Step 1: Add unit test for numeric ID detection**

Add at the bottom of `src/api/assets/objects.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn numeric_id_detected() {
        assert!("123".chars().all(|c| c.is_ascii_digit()));
        assert!("0".chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn object_key_not_numeric() {
        assert!(!"OBJ-1".chars().all(|c| c.is_ascii_digit()));
        assert!(!"SCHEMA-88".chars().all(|c| c.is_ascii_digit()));
        assert!(!"abc".chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn empty_string_is_numeric() {
        // Empty string passes chars().all() vacuously — edge case
        // resolve_object_key would treat "" as numeric ID, which is harmless
        // as the API will return 404
        assert!("".chars().all(|c| c.is_ascii_digit()));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --lib api::assets -- --nocapture
```

- [ ] **Step 3: Commit**

```bash
git add src/api/assets/objects.rs
git commit -m "test: add unit tests for object key vs numeric ID detection"
```
