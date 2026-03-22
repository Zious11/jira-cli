# Team Assignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable users to assign, filter, and list Jira teams via `jr`, resolving human-readable team names to UUIDs transparently.

**Architecture:** Add a cache layer (`~/.cache/jr/teams.json`) for team data, discovery endpoints for org metadata (cloudId → orgId → team list), and a `jr team list` command. Rewrite `resolve_team_field()` to do name→UUID resolution via cached partial matching. Update all JQL paths to use custom field IDs with UUIDs instead of display names.

**Tech Stack:** Rust, reqwest, serde, chrono (for cache TTL), dirs (for XDG cache path), dialoguer (for disambiguation prompts)

**Spec:** `docs/specs/team-assignment.md`

---

### Task 1: Add `org_id` to Config and `instance_url` to JiraClient

**Files:**
- Modify: `src/config.rs:23-28` (InstanceConfig)
- Modify: `src/api/client.rs:17-22` (JiraClient struct), `src/api/client.rs:24-58` (from_config)
- Modify: `Cargo.toml` (update `chrono` to add serde feature)
- Modify: `src/cli/init.rs:27-31` (InstanceConfig literal — add `org_id: None`)

- [ ] **Step 1: Add `org_id` field to `InstanceConfig` in `src/config.rs`**

```rust
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct InstanceConfig {
    pub url: Option<String>,
    pub cloud_id: Option<String>,
    pub org_id: Option<String>,
    pub auth_method: Option<String>,
}
```

- [ ] **Step 2: Add `instance_url` field to `JiraClient` struct in `src/api/client.rs`**

The `JiraClient` needs to store the raw instance URL separately from `base_url` (which may be an OAuth proxy URL for OAuth users). Add an `instance_url` field and an `instance_url()` accessor.

```rust
pub struct JiraClient {
    client: Client,
    base_url: String,
    instance_url: String,
    auth_header: String,
    verbose: bool,
}
```

- [ ] **Step 3: Populate `instance_url` in `from_config()`**

In `from_config()`, read the raw instance URL from `config.global.instance.url` and store it alongside `base_url`. The `instance_url` is always the raw Jira URL (e.g., `https://myorg.atlassian.net`), while `base_url` may differ for OAuth users.

```rust
pub fn from_config(config: &Config, verbose: bool) -> anyhow::Result<Self> {
    let base_url = config.base_url()?;
    let instance_url = config
        .global
        .instance
        .url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No Jira instance configured. Run \"jr init\" first."))?
        .trim_end_matches('/')
        .to_string();

    // ... auth_header unchanged ...

    Ok(Self {
        client,
        base_url,
        instance_url,
        auth_header,
        verbose,
    })
}
```

- [ ] **Step 4: Add `instance_url()` accessor and update `new_for_test()`**

```rust
pub fn instance_url(&self) -> &str {
    &self.instance_url
}

pub fn new_for_test(base_url: String, auth_header: String) -> Self {
    let instance_url = base_url.clone();
    Self {
        client: Client::new(),
        base_url,
        instance_url,
        auth_header,
        verbose: false,
    }
}
```

- [ ] **Step 5: Add `get_from_instance` and `post_to_instance` methods**

These are like `get` and `post` but use `instance_url` instead of `base_url`. Discovery endpoints (`/_edge/tenant_info`, `/gateway/api/graphql`, `/gateway/api/public/teams/...`) must use the instance URL, not the API proxy.

```rust
pub async fn get_from_instance<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
    let url = format!("{}{}", self.instance_url, path);
    let request = self.client.get(&url);
    let response = self.send(request).await?;
    let body = response.json::<T>().await?;
    Ok(body)
}

pub async fn post_to_instance<T: DeserializeOwned, B: Serialize>(
    &self,
    path: &str,
    body: &B,
) -> anyhow::Result<T> {
    let url = format!("{}{}", self.instance_url, path);
    let request = self.client.post(&url).json(body);
    let response = self.send(request).await?;
    let parsed = response.json::<T>().await?;
    Ok(parsed)
}
```

- [ ] **Step 6: Update `chrono` in `Cargo.toml` to add serde feature**

`chrono = "0.4"` already exists in `Cargo.toml` at line 32. Change it to:

```toml
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 6b: Update `InstanceConfig` struct literal in `src/cli/init.rs`**

At line 27-31 of `init.rs`, the `InstanceConfig` is constructed without `org_id`. Add it:

```rust
instance: InstanceConfig {
    url: Some(url.clone()),
    cloud_id: None,
    org_id: None,
    auth_method: None,
},
```

Also update all `InstanceConfig` struct literals in `src/config.rs` tests (lines 177-181, 195-199, 248-252, 268-272) to include `org_id: None`. Example:

```rust
instance: InstanceConfig {
    url: Some("https://myorg.atlassian.net".into()),
    cloud_id: None,
    org_id: None,
    auth_method: Some("api_token".into()),
},
```

- [ ] **Step 7: Run tests and verify compilation**

Run: `cargo test --lib`
Expected: All existing tests pass. The `org_id` field defaults to `None` via `Default`, so existing config files still load.

- [ ] **Step 8: Commit**

```bash
git add src/config.rs src/api/client.rs Cargo.toml Cargo.lock
git commit -m "feat(teams): add org_id to config and instance_url to JiraClient"
```

---

### Task 2: Create Team Types (`src/types/jira/team.rs`)

**Files:**
- Create: `src/types/jira/team.rs`
- Modify: `src/types/jira/mod.rs`

- [ ] **Step 1: Create `src/types/jira/team.rs`**

```rust
use serde::{Deserialize, Serialize};

/// Generic wrapper for Atlassian GraphQL responses
#[derive(Debug, Deserialize)]
pub struct GraphqlResponse<T> {
    pub data: Option<T>,
}

#[derive(Debug, Deserialize)]
pub struct TenantContextData {
    #[serde(rename = "tenantContexts")]
    pub tenant_contexts: Vec<TenantContext>,
}

/// A tenant context returned by the GraphQL `tenantContexts` query.
/// Contains both orgId and cloudId.
#[derive(Debug, Deserialize)]
pub struct TenantContext {
    #[serde(rename = "orgId")]
    pub org_id: String,
    #[serde(rename = "cloudId")]
    pub cloud_id: String,
}

/// A single team entry from the Teams REST API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TeamEntry {
    #[serde(rename = "teamId")]
    pub team_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

/// Response from `GET /gateway/api/public/teams/v1/org/{orgId}/teams`
#[derive(Debug, Deserialize)]
pub struct TeamsResponse {
    pub entities: Vec<TeamEntry>,
    pub cursor: Option<String>,
}
```

- [ ] **Step 2: Register module in `src/types/jira/mod.rs`**

Add `pub mod team;` and `pub use team::*;` to `src/types/jira/mod.rs`:

```rust
pub mod board;
pub mod issue;
pub mod project;
pub mod sprint;
pub mod team;
pub mod user;
pub mod worklog;

pub use board::*;
pub use issue::*;
pub use project::*;
pub use sprint::*;
pub use team::*;
pub use user::User;
pub use worklog::*;
```

- [ ] **Step 3: Run compilation check**

Run: `cargo check`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/types/jira/team.rs src/types/jira/mod.rs
git commit -m "feat(teams): add serde types for team discovery and listing"
```

---

### Task 3: Create Teams API (`src/api/jira/teams.rs`)

**Files:**
- Create: `src/api/jira/teams.rs`
- Modify: `src/api/jira/mod.rs`

- [ ] **Step 1: Create `src/api/jira/teams.rs`**

```rust
use anyhow::{Context, Result};

use crate::api::client::JiraClient;
use crate::types::jira::{
    GraphqlResponse, TeamEntry, TeamsResponse, TenantContext, TenantContextData,
};

impl JiraClient {
    /// Resolve cloudId and orgId for the Jira instance in a single GraphQL call.
    /// Uses `hostNames` parameter with the instance hostname.
    /// Uses instance_url (not base_url) since this endpoint is on the Jira instance itself.
    pub async fn get_org_metadata(&self, hostname: &str) -> Result<TenantContext> {
        let query = serde_json::json!({
            "query": format!(
                "query getOrgId {{ tenantContexts(hostNames: [\"{hostname}\"]) {{ orgId cloudId }} }}"
            )
        });
        let resp: GraphqlResponse<TenantContextData> = self
            .post_to_instance("/gateway/api/graphql", &query)
            .await
            .context("Failed to query org metadata via GraphQL")?;

        resp.data
            .and_then(|d| d.tenant_contexts.into_iter().next())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Could not resolve organization ID. Check your Jira URL and permissions, or run jr init."
                )
            })
    }

    /// List all teams in the organization, handling cursor-based pagination.
    /// Uses instance_url (not base_url).
    pub async fn list_teams(&self, org_id: &str) -> Result<Vec<TeamEntry>> {
        let mut all_teams: Vec<TeamEntry> = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut path = format!(
                "/gateway/api/public/teams/v1/org/{}/teams",
                org_id
            );
            if let Some(ref c) = cursor {
                path.push_str(&format!("?cursor={}", urlencoding::encode(c)));
            }

            let resp: TeamsResponse = self
                .get_from_instance(&path)
                .await
                .context("Failed to list teams")?;

            all_teams.extend(resp.entities);

            match resp.cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }

        Ok(all_teams)
    }
}
```

- [ ] **Step 2: Register module in `src/api/jira/mod.rs`**

```rust
pub mod boards;
pub mod fields;
pub mod issues;
pub mod projects;
pub mod sprints;
pub mod teams;
pub mod users;
pub mod worklogs;
```

- [ ] **Step 3: Run compilation check**

Run: `cargo check`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/api/jira/teams.rs src/api/jira/mod.rs
git commit -m "feat(teams): add API calls for cloud ID, org ID, and team listing"
```

---

### Task 4: Create Cache Module (`src/cache.rs`)

**Files:**
- Create: `src/cache.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing tests for cache TTL logic**

Add tests inside `src/cache.rs` that verify: reading missing cache returns None, writing then reading returns data, expired cache returns None.

```rust
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CACHE_TTL_DAYS: i64 = 7;

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedTeam {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamCache {
    pub fetched_at: DateTime<Utc>,
    pub teams: Vec<CachedTeam>,
}

/// Return the cache directory path, respecting XDG_CACHE_HOME.
pub fn cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("jr")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".cache")
            .join("jr")
    }
}

/// Read the team cache. Returns `None` if the file is missing or the cache has expired.
pub fn read_team_cache() -> Result<Option<TeamCache>> {
    let path = cache_dir().join("teams.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: TeamCache = serde_json::from_str(&content)?;

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(Some(cache))
}

/// Write the team cache to disk.
pub fn write_team_cache(teams: &[CachedTeam]) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let cache = TeamCache {
        fetched_at: Utc::now(),
        teams: teams.to_vec(),
    };

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(dir.join("teams.json"), content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_temp_cache<F: FnOnce()>(f: F) {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CACHE_HOME", dir.path());
        f();
        std::env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn read_missing_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_team_cache().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_returns_data() {
        with_temp_cache(|| {
            let teams = vec![
                CachedTeam {
                    id: "uuid-1".into(),
                    name: "Alpha".into(),
                },
                CachedTeam {
                    id: "uuid-2".into(),
                    name: "Beta".into(),
                },
            ];
            write_team_cache(&teams).unwrap();

            let cache = read_team_cache().unwrap().expect("cache should exist");
            assert_eq!(cache.teams.len(), 2);
            assert_eq!(cache.teams[0].name, "Alpha");
            assert_eq!(cache.teams[1].name, "Beta");
        });
    }

    #[test]
    fn expired_cache_returns_none() {
        with_temp_cache(|| {
            let expired = TeamCache {
                fetched_at: Utc::now() - chrono::Duration::days(8),
                teams: vec![CachedTeam {
                    id: "uuid-1".into(),
                    name: "Old".into(),
                }],
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("teams.json"), content).unwrap();

            let result = read_team_cache().unwrap();
            assert!(result.is_none(), "expired cache should return None");
        });
    }

    #[test]
    fn valid_cache_within_ttl() {
        with_temp_cache(|| {
            let recent = TeamCache {
                fetched_at: Utc::now() - chrono::Duration::days(3),
                teams: vec![CachedTeam {
                    id: "uuid-1".into(),
                    name: "Recent".into(),
                }],
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&recent).unwrap();
            std::fs::write(dir.join("teams.json"), content).unwrap();

            let cache = read_team_cache().unwrap().expect("cache should be valid");
            assert_eq!(cache.teams.len(), 1);
            assert_eq!(cache.teams[0].name, "Recent");
        });
    }
}
```

- [ ] **Step 2: Register module in `src/lib.rs`**

```rust
pub mod adf;
pub mod api;
pub mod cache;
pub mod cli;
pub mod config;
pub mod duration;
pub mod error;
pub mod output;
pub mod partial_match;
pub mod types;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib cache::tests`
Expected: All 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/cache.rs src/lib.rs
git commit -m "feat(teams): add cache module with TTL logic and tests"
```

---

### Task 5: Create `jr team list` Command

**Files:**
- Create: `src/cli/team.rs`
- Modify: `src/cli/mod.rs:1-7` (module declarations), `src/cli/mod.rs:44-87` (Command enum)
- Modify: `src/main.rs:131-136` (dispatch — add Team arm after Worklog arm)

- [ ] **Step 1: Add `TeamCommand` enum to `src/cli/mod.rs`**

Add `pub mod team;` to the module declarations at the top:

```rust
pub mod auth;
pub mod board;
pub mod init;
pub mod issue;
pub mod project;
pub mod sprint;
pub mod team;
pub mod worklog;
```

Add `Team` variant to the `Command` enum:

```rust
/// Manage teams
Team {
    #[command(subcommand)]
    command: TeamCommand,
},
```

Add `TeamCommand` enum after `WorklogCommand`:

```rust
#[derive(Subcommand)]
pub enum TeamCommand {
    /// List available teams
    List {
        /// Force refresh from API, ignoring cache
        #[arg(long)]
        refresh: bool,
    },
}
```

- [ ] **Step 2: Create `src/cli/team.rs`**

```rust
use anyhow::{Context, Result};

use crate::api::client::JiraClient;
use crate::cache::{self, CachedTeam};
use crate::cli::OutputFormat;
use crate::config::Config;
use crate::output;

use super::TeamCommand;

pub async fn handle(
    command: TeamCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    match command {
        TeamCommand::List { refresh } => handle_list(refresh, output_format, config, client).await,
    }
}

async fn handle_list(
    refresh: bool,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let teams = if refresh {
        fetch_and_cache_teams(config, client).await?
    } else {
        match cache::read_team_cache()? {
            Some(cached) => cached.teams,
            None => fetch_and_cache_teams(config, client).await?,
        }
    };

    if teams.is_empty() {
        eprintln!("No teams found.");
        return Ok(());
    }

    let rows: Vec<Vec<String>> = teams
        .iter()
        .map(|t| vec![t.name.clone(), t.id.clone()])
        .collect();

    output::print_output(output_format, &["Name", "ID"], &rows, &teams)?;
    Ok(())
}

/// Fetch teams from the API and write them to the cache.
/// Resolves org_id lazily if not in config.
pub async fn fetch_and_cache_teams(
    config: &Config,
    client: &JiraClient,
) -> Result<Vec<CachedTeam>> {
    let org_id = resolve_org_id(config, client).await?;

    let api_teams = client
        .list_teams(&org_id)
        .await
        .context("Failed to fetch teams from API")?;

    let cached: Vec<CachedTeam> = api_teams
        .into_iter()
        .map(|t| CachedTeam {
            id: t.team_id,
            name: t.display_name,
        })
        .collect();

    cache::write_team_cache(&cached)?;
    Ok(cached)
}

/// Resolve org_id: read from config, or discover via GraphQL and persist.
/// Uses hostNames-based GraphQL query to get both cloudId and orgId in one call.
pub async fn resolve_org_id(config: &Config, client: &JiraClient) -> Result<String> {
    if let Some(ref org_id) = config.global.instance.org_id {
        return Ok(org_id.clone());
    }

    // Extract hostname from instance URL
    let url = config
        .global
        .instance
        .url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No Jira instance configured. Run \"jr init\" first."))?;
    let hostname = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    // Single GraphQL call returns both cloudId and orgId
    let metadata = client.get_org_metadata(hostname).await?;

    // Persist discovered values to config for future use
    let mut updated_config = Config::load()?;
    updated_config.global.instance.cloud_id = Some(metadata.cloud_id);
    updated_config.global.instance.org_id = Some(metadata.org_id.clone());
    updated_config.save_global()?;

    Ok(metadata.org_id)
}
```

- [ ] **Step 3: Add dispatch in `src/main.rs`**

In the `match cli.command` block in the `run()` function, add the `Team` arm after the `Worklog` arm:

```rust
cli::Command::Team { command } => {
    let config = config::Config::load()?;
    let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
    cli::team::handle(command, &cli.output, &config, &client).await
}
```

- [ ] **Step 4: Run compilation check**

Run: `cargo check`
Expected: Compiles with no errors.

- [ ] **Step 5: Commit**

```bash
git add src/cli/team.rs src/cli/mod.rs src/main.rs
git commit -m "feat(teams): add jr team list command with cache"
```

---

### Task 6: Rewrite `resolve_team_field()` with Name-to-UUID Resolution

**Files:**
- Modify: `src/cli/issue.rs:884-899` (resolve_team_field)
- Modify: `src/cli/issue.rs:50-56` (handle signature — add no_input pass-through)
- Modify: `src/cli/issue.rs:110-130` (handle_edit dispatch — add no_input)
- Modify: `src/cli/issue.rs:447-457` (handle_edit signature — add no_input)

- [ ] **Step 1: Rewrite `resolve_team_field()` in `src/cli/issue.rs`**

Replace the current function (lines 884-899) with:

```rust
async fn resolve_team_field(
    config: &Config,
    client: &JiraClient,
    team_name: &str,
    no_input: bool,
) -> Result<(String, String)> {
    // 1. Resolve team_field_id
    let field_id = if let Some(id) = &config.global.fields.team_field_id {
        id.clone()
    } else {
        client
            .find_team_field_id()
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No \"Team\" field found on this Jira instance. This instance may not have the Team field configured."
                )
            })?
    };

    // 2. Load teams from cache (or fetch if missing/expired)
    let teams = match crate::cache::read_team_cache()? {
        Some(cached) => cached.teams,
        None => crate::cli::team::fetch_and_cache_teams(config, client).await?,
    };

    // 3. Partial match
    let team_names: Vec<String> = teams.iter().map(|t| t.name.clone()).collect();
    match crate::partial_match::partial_match(team_name, &team_names) {
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let team = teams
                .iter()
                .find(|t| t.name == matched_name)
                .expect("matched name must exist in teams");
            Ok((field_id, team.id.clone()))
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            if no_input {
                let quoted: Vec<String> = matches.iter().map(|m| format!("\"{}\"", m)).collect();
                anyhow::bail!(
                    "Multiple teams match \"{}\": {}. Use a more specific name.",
                    team_name,
                    quoted.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple teams match \"{team_name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let team = teams
                .iter()
                .find(|t| &t.name == selected_name)
                .expect("selected name must exist in teams");
            Ok((field_id, team.id.clone()))
        }
        crate::partial_match::MatchResult::None(_) => {
            anyhow::bail!(
                "No team matching \"{}\". Run \"jr team list --refresh\" to update.",
                team_name
            );
        }
    }
}
```

- [ ] **Step 2: Thread `no_input` into `handle_edit` signature**

Change `handle_edit` signature (currently at line 447) to accept `no_input: bool`:

```rust
async fn handle_edit(
    key: &str,
    summary: Option<String>,
    issue_type: Option<String>,
    priority: Option<String>,
    labels: Vec<String>,
    team: Option<String>,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
```

- [ ] **Step 3: Update `handle_edit` dispatch to pass `no_input`**

In the `handle()` function match arm for `IssueCommand::Edit` (currently around line 110-130), add `no_input`:

```rust
IssueCommand::Edit {
    key,
    summary,
    issue_type,
    priority,
    label,
    team,
} => {
    handle_edit(
        &key,
        summary,
        issue_type,
        priority,
        label,
        team,
        output_format,
        config,
        client,
        no_input,
    )
    .await
}
```

- [ ] **Step 4: Update `resolve_team_field` call sites to pass `no_input`**

In `handle_create` (around line 425):
```rust
if let Some(ref team_name) = team {
    let (field_id, team_id) = resolve_team_field(config, client, team_name, no_input).await?;
    fields[&field_id] = json!(team_id);
}
```

In `handle_edit` (around line 476):
```rust
if let Some(ref team_name) = team {
    let (field_id, team_id) = resolve_team_field(config, client, team_name, no_input).await?;
    fields[&field_id] = json!(team_id);
    has_updates = true;
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib`
Expected: All tests pass. The existing `build_fallback_jql` tests still pass since they don't touch `resolve_team_field`.

- [ ] **Step 6: Commit**

```bash
git add src/cli/issue.rs
git commit -m "feat(teams): rewrite resolve_team_field with name-to-UUID resolution"
```

---

### Task 7: Update JQL Paths to Use Field ID and UUID

**Files:**
- Modify: `src/cli/issue.rs:157-260` (handle_list and build_fallback_jql)

- [ ] **Step 1: Update `handle_list` to resolve team before JQL construction**

Currently `handle_list` takes `team: Option<String>` and interpolates the raw name into JQL. Change it to resolve the team name to `(field_id, uuid)` first, then pass the resolved values to the JQL construction.

Update the `handle_list` signature to accept `no_input`:

```rust
#[allow(clippy::too_many_arguments)]
async fn handle_list(
    jql: Option<String>,
    status: Option<String>,
    team: Option<String>,
    limit: Option<u32>,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
```

- [ ] **Step 2: Add team resolution at the top of `handle_list`**

After the function signature, before the JQL construction, resolve the team:

```rust
    // Resolve team name to (field_id, uuid) before building JQL
    let resolved_team = if let Some(ref team_name) = team {
        Some(resolve_team_field(config, client, team_name, no_input).await?)
    } else {
        None
    };
```

- [ ] **Step 3: Update all three JQL construction paths**

Replace all three occurrences of `format!("\"Team\" = \"{}\"", t)` with the resolved field ID and UUID.

In the **scrum** path (around line 191):
```rust
if let Some((field_id, team_uuid)) = &resolved_team {
    jql_parts.push(format!("{} = \"{}\"", field_id, team_uuid));
}
```

In the **kanban** path (around line 214):
```rust
if let Some((field_id, team_uuid)) = &resolved_team {
    jql_parts.push(format!("{} = \"{}\"", field_id, team_uuid));
}
```

Update `build_fallback_jql` to accept `resolved_team: Option<&(String, String)>` instead of `team: Option<&str>`:

```rust
fn build_fallback_jql(
    project_key: Option<&str>,
    status: Option<&str>,
    resolved_team: Option<&(String, String)>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(pk) = project_key {
        parts.push(format!("project = \"{}\"", pk));
    }
    if let Some(s) = status {
        parts.push(format!("status = \"{}\"", s));
    }
    if let Some((field_id, team_uuid)) = resolved_team {
        parts.push(format!("{} = \"{}\"", field_id, team_uuid));
    }
    let where_clause = parts.join(" AND ");
    format!("{} ORDER BY updated DESC", where_clause)
}
```

Update all call sites of `build_fallback_jql` to pass `resolved_team.as_ref()` instead of `team.as_deref()`.

- [ ] **Step 4: Update the `handle_list` dispatch call in `handle()` to pass `no_input`**

```rust
IssueCommand::List {
    jql,
    status,
    team,
    limit,
} => {
    handle_list(
        jql,
        status,
        team,
        limit,
        output_format,
        config,
        client,
        project_override,
        no_input,
    )
    .await
}
```

- [ ] **Step 5: Update `build_fallback_jql` tests**

The tests now need to pass `Option<&(String, String)>` instead of `Option<&str>`. Update them:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_jql_order_by_not_joined_with_and() {
        let jql = build_fallback_jql(Some("PROJ"), None, None);
        assert!(
            !jql.contains("AND ORDER BY"),
            "ORDER BY must not be joined with AND: {jql}"
        );
        assert!(jql.ends_with("ORDER BY updated DESC"));
    }

    #[test]
    fn fallback_jql_with_team_has_valid_order_by() {
        let team = ("customfield_10001".to_string(), "uuid-123".to_string());
        let jql = build_fallback_jql(Some("PROJ"), None, Some(&team));
        assert!(
            !jql.contains("AND ORDER BY"),
            "ORDER BY must not be joined with AND: {jql}"
        );
        assert!(jql.contains("customfield_10001 = \"uuid-123\""));
        assert!(jql.ends_with("ORDER BY updated DESC"));
    }

    #[test]
    fn fallback_jql_with_all_filters() {
        let team = ("customfield_10001".to_string(), "uuid-456".to_string());
        let jql = build_fallback_jql(Some("PROJ"), Some("In Progress"), Some(&team));
        assert!(
            !jql.contains("AND ORDER BY"),
            "ORDER BY must not be joined with AND: {jql}"
        );
        assert!(jql.contains("project = \"PROJ\""));
        assert!(jql.contains("status = \"In Progress\""));
        assert!(jql.contains("customfield_10001 = \"uuid-456\""));
        assert!(jql.ends_with("ORDER BY updated DESC"));
    }

    #[test]
    fn fallback_jql_no_filters_still_has_order_by() {
        let jql = build_fallback_jql(None, None, None);
        assert_eq!(jql, " ORDER BY updated DESC");
    }

    #[test]
    fn fallback_jql_with_status_only() {
        let jql = build_fallback_jql(None, Some("Done"), None);
        assert_eq!(jql, "status = \"Done\" ORDER BY updated DESC");
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib cli::issue::tests`
Expected: All 5 tests pass with updated signatures.

- [ ] **Step 7: Commit**

```bash
git add src/cli/issue.rs
git commit -m "feat(teams): update JQL to use field ID and UUID instead of display name"
```

---

### Task 8: Update `jr init` to Prefetch Team Metadata

**Files:**
- Modify: `src/cli/init.rs`

- [ ] **Step 1: Add prefetch steps to `src/cli/init.rs`**

After the existing Step 5 (team_field_id discovery, line 88-93), add cloud_id, org_id, and team cache prefetch. All best-effort — wrap in `if let Ok(...)`.

```rust
    // Step 5: Discover team field (already exists — unchanged)
    if let Ok(Some(team_id)) = client.find_team_field_id().await {
        let mut config = Config::load()?;
        config.global.fields.team_field_id = Some(team_id);
        config.save_global()?;
    }

    // Step 6: Prefetch cloud_id and org_id via GraphQL (single call)
    let hostname = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    if let Ok(metadata) = client.get_org_metadata(hostname).await {
        let mut config = Config::load()?;
        config.global.instance.cloud_id = Some(metadata.cloud_id);
        config.global.instance.org_id = Some(metadata.org_id.clone());
        config.save_global()?;

        // Step 7: Prefetch team list into cache
        if let Ok(api_teams) = client.list_teams(&metadata.org_id).await {
            let cached: Vec<crate::cache::CachedTeam> = api_teams
                .into_iter()
                .map(|t| crate::cache::CachedTeam {
                    id: t.team_id,
                    name: t.display_name,
                })
                .collect();
            let _ = crate::cache::write_team_cache(&cached);
        }
    }
```

- [ ] **Step 2: Run compilation check**

Run: `cargo check`
Expected: Compiles with no errors.

- [ ] **Step 3: Commit**

```bash
git add src/cli/init.rs
git commit -m "feat(teams): prefetch cloud_id, org_id, and team cache during init"
```

---

### Task 9: Integration Tests

**Files:**
- Create: `tests/team_commands.rs`
- Modify: `tests/common/fixtures.rs` (add team fixtures)
- Modify: `tests/common/mock_server.rs` (if needed)

- [ ] **Step 1: Check existing test infrastructure**

Read `tests/common/fixtures.rs` and `tests/common/mock_server.rs` to understand the test patterns.

- [ ] **Step 2: Add team fixture data to `tests/common/fixtures.rs`**

```rust
pub fn graphql_org_metadata_json() -> serde_json::Value {
    serde_json::json!({
        "data": {
            "tenantContexts": [
                { "orgId": "test-org-id-456", "cloudId": "test-cloud-id-123" }
            ]
        }
    })
}

pub fn teams_list_json() -> serde_json::Value {
    serde_json::json!({
        "entities": [
            { "teamId": "team-uuid-alpha", "displayName": "Alpha Team" },
            { "teamId": "team-uuid-beta", "displayName": "Beta Team" },
            { "teamId": "team-uuid-security", "displayName": "Security Engineering" }
        ],
        "cursor": null
    })
}
```

- [ ] **Step 3: Create `tests/team_commands.rs`**

Write integration tests using wiremock:

```rust
mod common;

use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_get_org_metadata() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/gateway/api/graphql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::graphql_org_metadata_json()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let metadata = client.get_org_metadata("test.atlassian.net").await.unwrap();
    assert_eq!(metadata.org_id, "test-org-id-456");
    assert_eq!(metadata.cloud_id, "test-cloud-id-123");
}

#[tokio::test]
async fn test_list_teams() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/gateway/api/public/teams/v1/org/.*/teams"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::teams_list_json()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let teams = client.list_teams("test-org-id-456").await.unwrap();
    assert_eq!(teams.len(), 3);
    assert_eq!(teams[0].display_name, "Alpha Team");
    assert_eq!(teams[0].team_id, "team-uuid-alpha");
}
```

- [ ] **Step 4: Run integration tests**

Run: `cargo test --test team_commands`
Expected: All 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add tests/team_commands.rs tests/common/fixtures.rs
git commit -m "test(teams): add integration tests for team discovery and listing"
```

---

### Task 10: Live Smoke Test

**Files:** None (manual testing)

- [ ] **Step 1: Build**

Run: `cargo build`

- [ ] **Step 2: Test `jr team list`**

Run: `./target/debug/jr team list`
Expected: Table showing teams from 1898andco.atlassian.net.

- [ ] **Step 3: Test `jr team list --output json`**

Run: `./target/debug/jr team list --output json`
Expected: JSON array of team objects.

- [ ] **Step 4: Test `jr team list --refresh`**

Run: `./target/debug/jr team list --refresh`
Expected: Fresh fetch, same output.

- [ ] **Step 5: Test `jr issue list --team`**

Run: `./target/debug/jr --verbose issue list --project MSSCI --team "Platform"`
Expected: Valid JQL with `customfield_10001 = "uuid"`, results or "No results found."

- [ ] **Step 6: Test `jr issue create --team` (dry run with known data)**

Run: `./target/debug/jr issue create --project MSSCI --type Task --summary "Team test ticket" --team "Platform" --output json`
Expected: Issue created with team assignment, or a meaningful error if team name doesn't match.

- [ ] **Step 7: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 8: Commit any fixups**

If any issues found during smoke testing, fix and commit.
