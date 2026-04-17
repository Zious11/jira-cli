# Issue Changelog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `jr issue changelog <KEY>` — a new subcommand that fetches an issue's audit history from Jira Cloud, sorts/filters client-side, and renders it as a flat table or nested JSON.

**Architecture:** New handler file `src/cli/issue/changelog.rs` + new type file `src/types/jira/changelog.rs`. Adds one method (`get_changelog`) to the existing `impl JiraClient` in `src/api/jira/issues.rs`. Fetches all pages of `GET /rest/api/3/issue/{key}/changelog` (offset-paginated under `values[]`), then sorts/filters/truncates in-memory.

**Tech Stack:** Rust 2024, clap derive, reqwest, serde, anyhow, tokio (async). Tests use wiremock + `assert_cmd` + `insta` (JSON snapshots only — table output uses `contains` assertions, matching the project pattern).

**Spec:** `docs/specs/issue-changelog.md`

**Branch:** `feat/issue-changelog` (already created, spec committed).

**Lint/format policy:** clippy `-D warnings`; `cargo fmt --check` must pass. `cargo deny check` runs in CI but rarely needs attention for a pure-Rust feature.

---

## File Structure

```
src/
├── types/jira/
│   ├── changelog.rs        (NEW)  ChangelogEntry, ChangelogItem + unit tests
│   └── mod.rs              (MOD)  pub mod changelog; pub use changelog::*;
├── api/jira/
│   └── issues.rs           (MOD)  add get_changelog method to impl JiraClient
├── cli/
│   ├── mod.rs              (MOD)  add IssueCommand::Changelog variant
│   └── issue/
│       ├── mod.rs          (MOD)  dispatch IssueCommand::Changelog
│       └── changelog.rs    (NEW)  handler, formatting, filters, sort + unit tests
└── tests/
    └── issue_changelog.rs  (NEW)  integration tests (wiremock + assert_cmd)
```

---

## Task 1: Add `ChangelogEntry` / `ChangelogItem` types

**Files:**
- Create: `src/types/jira/changelog.rs`
- Modify: `src/types/jira/mod.rs`

- [ ] **Step 1: Write the failing deserialization tests**

Create `src/types/jira/changelog.rs` with only the tests (no struct yet) so the compile fails.

```rust
// src/types/jira/changelog.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_standard_entry() {
        let json = r#"{
            "id": "10000",
            "author": {
                "accountId": "abc",
                "displayName": "Alice",
                "emailAddress": "a@test.com",
                "active": true
            },
            "created": "2026-04-16T14:02:11.000+0000",
            "items": [
                {
                    "field": "status",
                    "fieldtype": "jira",
                    "from": "1",
                    "fromString": "To Do",
                    "to": "3",
                    "toString": "In Progress"
                }
            ]
        }"#;
        let entry: ChangelogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "10000");
        assert_eq!(entry.author.as_ref().unwrap().display_name, "Alice");
        assert_eq!(entry.items.len(), 1);
        assert_eq!(entry.items[0].field, "status");
        assert_eq!(entry.items[0].from_string.as_deref(), Some("To Do"));
        assert_eq!(entry.items[0].to_string.as_deref(), Some("In Progress"));
    }

    #[test]
    fn deserializes_null_author_for_automation() {
        let json = r#"{
            "id": "10001",
            "author": null,
            "created": "2026-04-14T11:10:00.000+0000",
            "items": [
                { "field": "assignee", "fieldtype": "jira",
                  "from": null, "fromString": null,
                  "to": "abc", "toString": "Alice" }
            ]
        }"#;
        let entry: ChangelogEntry = serde_json::from_str(json).unwrap();
        assert!(entry.author.is_none());
        assert_eq!(entry.items[0].from, None);
        assert_eq!(entry.items[0].from_string, None);
    }

    #[test]
    fn deserializes_missing_from_to_strings() {
        // fromString/toString may be absent entirely for some fields
        let json = r#"{
            "id": "10002",
            "author": {
                "accountId": "abc",
                "displayName": "Alice",
                "active": true
            },
            "created": "2026-04-15T09:00:00.000+0000",
            "items": [
                { "field": "labels", "fieldtype": "jira",
                  "from": "", "to": "backend" }
            ]
        }"#;
        let entry: ChangelogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.items[0].from_string, None);
        assert_eq!(entry.items[0].to_string, None);
    }

    #[test]
    fn deserializes_multiple_items_in_one_entry() {
        let json = r#"{
            "id": "10003",
            "author": { "accountId": "abc", "displayName": "Alice", "active": true },
            "created": "2026-04-16T14:02:11.000+0000",
            "items": [
                { "field": "status", "fieldtype": "jira",
                  "from": "1", "fromString": "To Do",
                  "to": "3", "toString": "Done" },
                { "field": "resolution", "fieldtype": "jira",
                  "from": null, "fromString": null,
                  "to": "10000", "toString": "Done" }
            ]
        }"#;
        let entry: ChangelogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.items.len(), 2);
        assert_eq!(entry.items[1].field, "resolution");
    }
}
```

- [ ] **Step 2: Verify compile failure**

```bash
cargo test --lib types::jira::changelog 2>&1 | head -20
```

Expected: `cannot find type ChangelogEntry in this scope`.

- [ ] **Step 3: Write the minimal struct definitions**

Prepend to `src/types/jira/changelog.rs`:

```rust
use crate::types::jira::User;
use serde::{Deserialize, Serialize};

/// A single entry in an issue's changelog — one actor, one timestamp,
/// one or more field-level changes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChangelogEntry {
    pub id: String,
    /// May be `null` for automation, workflow post-functions, or migrated data.
    #[serde(default)]
    pub author: Option<User>,
    /// ISO-8601 timestamp as returned by the API.
    pub created: String,
    #[serde(default)]
    pub items: Vec<ChangelogItem>,
}

/// A single field-level change within a `ChangelogEntry`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogItem {
    pub field: String,
    pub fieldtype: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub from_string: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub to_string: Option<String>,
}
```

Serialize is derived so the JSON output path can reuse `serde_json::to_string_pretty` via `output::render_json`.

- [ ] **Step 4: Register the new module**

Edit `src/types/jira/mod.rs`. Add alongside existing `pub mod ...;` lines:

```rust
pub mod changelog;
```

And alongside existing `pub use ...::*;` re-exports:

```rust
pub use changelog::*;
```

Keep the alphabetical ordering the file already uses (board, changelog, issue, ...).

- [ ] **Step 5: Run the tests**

```bash
cargo test --lib types::jira::changelog -- --nocapture
```

Expected: 4 tests pass.

- [ ] **Step 6: Run clippy to confirm no warnings**

```bash
cargo clippy --lib --all-targets -- -D warnings
```

Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add src/types/jira/changelog.rs src/types/jira/mod.rs
git commit -m "feat(types): add ChangelogEntry and ChangelogItem for issue changelog API (#200)"
```

---

## Task 2: Add `get_changelog` method to JiraClient

**Files:**
- Modify: `src/api/jira/issues.rs`
- Test: `tests/issue_changelog.rs` (new file — integration test)

The test goes in `tests/` so it uses the public `jr::api::client::JiraClient::new_for_test(...)` helper (matching `tests/comments.rs`). Unit tests inside `src/api/jira/issues.rs` cannot easily use wiremock because the existing inline `#[cfg(test)]` module only covers pure logic; wiremock-backed tests live in `tests/*.rs`.

- [ ] **Step 1: Write the failing integration test — happy path, single page**

Create `tests/issue_changelog.rs`:

```rust
#[allow(dead_code)]
mod common;

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_changelog_single_page_returns_entries() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 100,
            "total": 1,
            "isLast": true,
            "values": [
                {
                    "id": "10000",
                    "author": { "accountId": "abc", "displayName": "Alice", "active": true },
                    "created": "2026-04-16T14:02:11.000+0000",
                    "items": [{
                        "field": "status", "fieldtype": "jira",
                        "from": "1", "fromString": "To Do",
                        "to": "3", "toString": "In Progress"
                    }]
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let entries = client.get_changelog("FOO-1").await.unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "10000");
    assert_eq!(entries[0].items[0].field, "status");
}
```

- [ ] **Step 2: Verify it fails**

```bash
cargo test --test issue_changelog get_changelog_single_page -- --nocapture
```

Expected: compile error — no method `get_changelog` on `JiraClient`.

- [ ] **Step 3: Implement `get_changelog` on the existing impl block**

In `src/api/jira/issues.rs`, inside the existing `impl JiraClient { ... }`, add:

```rust
/// Fetch the full audit changelog for an issue.
///
/// Offset-paginated under `values[]` (`OffsetPage::items()` already prefers
/// this key). Always fetches every page; sort/filter/truncate are the
/// caller's responsibility — the Jira changelog endpoint supports no
/// server-side filters and does not guarantee sort order.
pub async fn get_changelog(
    &self,
    key: &str,
) -> Result<Vec<crate::types::jira::ChangelogEntry>> {
    let base = format!("/rest/api/3/issue/{}/changelog", urlencoding::encode(key));
    let mut all = Vec::new();
    let mut start_at = 0u32;
    let max_page_size: u32 = 100;

    loop {
        let path = format!("{}?startAt={}&maxResults={}", base, start_at, max_page_size);
        let page: OffsetPage<crate::types::jira::ChangelogEntry> = self.get(&path).await?;
        let has_more = page.has_more();
        let next = page.next_start();
        all.extend(page.values.unwrap_or_default());

        if !has_more {
            break;
        }
        start_at = next;
    }

    Ok(all)
}
```

Note: `OffsetPage` is already imported at the top of the file. If it isn't, add `use crate::api::pagination::OffsetPage;`.

- [ ] **Step 4: Run the test**

```bash
cargo test --test issue_changelog get_changelog_single_page -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Write the failing multi-page test**

Append to `tests/issue_changelog.rs`:

```rust
#[tokio::test]
async fn get_changelog_auto_paginates_across_pages() {
    let server = MockServer::start().await;

    // Page 1 (startAt=0, total=2, has_more because startAt+maxResults < total)
    // Use maxResults=1 to force a second page; client asks maxResults=100 but
    // the server can cap it — simulate that by returning total=2 with a
    // single entry in values[].
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2/changelog"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0,
            "maxResults": 1,
            "total": 2,
            "isLast": false,
            "values": [{
                "id": "1", "author": null,
                "created": "2026-04-10T00:00:00.000+0000",
                "items": [{"field": "status", "fieldtype": "jira",
                           "from": "1", "fromString": "To Do",
                           "to": "2", "toString": "In Progress"}]
            }]
        })))
        .mount(&server)
        .await;

    // Page 2
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2/changelog"))
        .and(query_param("startAt", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 1,
            "maxResults": 1,
            "total": 2,
            "isLast": true,
            "values": [{
                "id": "2", "author": null,
                "created": "2026-04-11T00:00:00.000+0000",
                "items": [{"field": "status", "fieldtype": "jira",
                           "from": "2", "fromString": "In Progress",
                           "to": "3", "toString": "Done"}]
            }]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let entries = client.get_changelog("FOO-2").await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "1");
    assert_eq!(entries[1].id, "2");
}
```

- [ ] **Step 6: Run — should pass (pagination loop already implemented)**

```bash
cargo test --test issue_changelog get_changelog_auto_paginates -- --nocapture
```

Expected: PASS. If FAIL, the paginator's `has_more()` / `next_start()` wiring is off — re-check against the `list_comments` loop in `issues.rs`.

- [ ] **Step 7: Clippy + fmt**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```

Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add src/api/jira/issues.rs tests/issue_changelog.rs
git commit -m "feat(api): add get_changelog method to JiraClient (#200)"
```

---

## Task 3: Add `IssueCommand::Changelog` variant + dispatch stub

**Files:**
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/issue/mod.rs`
- Create: `src/cli/issue/changelog.rs` (stub only)

- [ ] **Step 1: Write a failing CLI smoke test**

Append to `tests/issue_changelog.rs`:

```rust
use assert_cmd::Command;

#[test]
fn changelog_help_lists_subcommand() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "changelog", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "--help should exit 0, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--limit"), "help missing --limit: {stdout}");
    assert!(stdout.contains("--all"), "help missing --all: {stdout}");
    assert!(stdout.contains("--field"), "help missing --field: {stdout}");
    assert!(stdout.contains("--author"), "help missing --author: {stdout}");
    assert!(stdout.contains("--reverse"), "help missing --reverse: {stdout}");
}
```

- [ ] **Step 2: Run — expect failure**

```bash
cargo test --test issue_changelog changelog_help_lists_subcommand -- --nocapture
```

Expected: compile error — `IssueCommand::Changelog` doesn't exist, or FAIL if it compiles but the subcommand isn't wired.

- [ ] **Step 3: Add the `Changelog` variant to `IssueCommand`**

In `src/cli/mod.rs`, inside `pub enum IssueCommand { ... }`, add (placing it between `Comments` and `Open` to keep the audit commands visually grouped):

```rust
    /// Show an issue's audit changelog (status/field changes)
    Changelog {
        /// Issue key (e.g., FOO-123)
        key: String,
        /// Maximum number of rows (default 30). Applies post-filter.
        #[arg(long, conflicts_with = "all")]
        limit: Option<u32>,
        /// No output truncation (still always fetches all pages)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
        /// Filter by field name; repeatable (case-insensitive substring)
        #[arg(long = "field")]
        field: Vec<String>,
        /// Filter by author ("me", display name substring, or accountId)
        #[arg(long)]
        author: Option<String>,
        /// Render oldest-first instead of default newest-first
        #[arg(long)]
        reverse: bool,
    },
```

- [ ] **Step 4: Create the handler stub file**

Create `src/cli/issue/changelog.rs`:

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};

pub(super) async fn handle(
    _command: IssueCommand,
    _output_format: &OutputFormat,
    _client: &JiraClient,
) -> Result<()> {
    // Implemented in Task 4.
    unimplemented!("changelog handler — see Task 4")
}
```

- [ ] **Step 5: Register the submodule + dispatch**

Edit `src/cli/issue/mod.rs`:

Add to the module list at the top:

```rust
mod changelog;
```

(alphabetical ordering: goes between `assets` and `create`).

Add the dispatch arm inside the `match command { ... }` block in `handle`, between `Comments { .. }` and `Open { .. }`:

```rust
        IssueCommand::Changelog { .. } => {
            changelog::handle(command, output_format, client).await
        }
```

Note: `_config`, `_project_override`, `_no_input` are not forwarded — the handler does not need them (no config lookups; no prompts).

- [ ] **Step 6: Run the help test — should now pass**

```bash
cargo test --test issue_changelog changelog_help_lists_subcommand -- --nocapture
```

Expected: PASS. The handler is still `unimplemented!` but `--help` never reaches it.

- [ ] **Step 7: Clippy + fmt**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```

Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add src/cli/mod.rs src/cli/issue/mod.rs src/cli/issue/changelog.rs
git commit -m "feat(cli): register 'issue changelog' subcommand (#200)"
```

---

## Task 4: Implement minimal handler — fetch + sort DESC + flat render (table + JSON)

**Files:**
- Modify: `src/cli/issue/changelog.rs`
- Test: `tests/issue_changelog.rs` (add happy-path integration tests)

This task wires the smallest end-to-end slice. Later tasks layer filtering and `--reverse` on top.

- [ ] **Step 1: Write the failing happy-path table test**

Append to `tests/issue_changelog.rs`:

```rust
#[tokio::test]
async fn changelog_table_renders_flat_rows_newest_first() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "alice", "displayName": "Alice", "active": true },
                    "created": "2026-04-14T16:02:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "backend"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "alice", "displayName": "Alice", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [
                        {"field": "status", "fieldtype": "jira",
                         "from": "1", "fromString": "To Do",
                         "to": "3", "toString": "In Progress"},
                        {"field": "resolution", "fieldtype": "jira",
                         "from": null, "fromString": null,
                         "to": "10000", "toString": "Done"}
                    ]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1"])
        .output()
        .unwrap();

    assert!(output.status.success(),
        "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Newest-first: status row (from entry id=2) appears before labels row.
    let status_idx = stdout.find("status").expect("status row missing");
    let labels_idx = stdout.find("labels").expect("labels row missing");
    assert!(status_idx < labels_idx,
        "expected status (newer) before labels (older), got:\n{stdout}");

    // Flat rows: entry id=2 produces TWO rows (status + resolution).
    assert!(stdout.contains("resolution"), "resolution row missing: {stdout}");

    // From/to rendering: "To Do" → "In Progress"; null rendered as em dash.
    assert!(stdout.contains("To Do"), "fromString missing: {stdout}");
    assert!(stdout.contains("In Progress"), "toString missing: {stdout}");
    assert!(stdout.contains("—"), "em-dash null marker missing: {stdout}");
}
```

- [ ] **Step 2: Write the failing JSON test**

Append:

```rust
#[tokio::test]
async fn changelog_json_preserves_nested_structure() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "10000",
                "author": { "accountId": "alice", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:11.000+0000",
                "items": [
                    {"field": "status", "fieldtype": "jira",
                     "from": "1", "fromString": "To Do",
                     "to": "3", "toString": "In Progress"},
                    {"field": "resolution", "fieldtype": "jira",
                     "from": null, "fromString": null,
                     "to": "10000", "toString": "Done"}
                ]
            }]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--output", "json"])
        .output()
        .unwrap();

    assert!(output.status.success(),
        "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("expected valid JSON");

    assert_eq!(parsed["key"], "FOO-1");
    let entries = parsed["entries"].as_array().expect("entries must be array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], "10000");
    assert_eq!(entries[0]["items"].as_array().unwrap().len(), 2);
    // Nested structure preserved — item[0].field is accessible directly.
    assert_eq!(entries[0]["items"][0]["field"], "status");
}
```

- [ ] **Step 3: Run — expect both to fail with `unimplemented!`**

```bash
cargo test --test issue_changelog changelog_table_renders changelog_json_preserves -- --nocapture
```

Expected: FAIL with `not implemented: changelog handler — see Task 4`.

- [ ] **Step 4: Replace the stub with the minimal handler**

Overwrite `src/cli/issue/changelog.rs`:

```rust
use anyhow::Result;
use chrono::DateTime;
use serde::Serialize;

use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::output;
use crate::types::jira::ChangelogEntry;

const NULL_GLYPH: &str = "—";
const SYSTEM_AUTHOR: &str = "(system)";

/// Shape of the JSON output body. Keeps the `key` alongside entries so
/// consumers always know which issue a response belongs to.
#[derive(Serialize)]
struct ChangelogOutput<'a> {
    key: &'a str,
    entries: &'a [ChangelogEntry],
}

pub(super) async fn handle(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Changelog {
        key,
        limit: _,
        all: _,
        field: _,
        author: _,
        reverse: _,
    } = command
    else {
        unreachable!("handler only called for IssueCommand::Changelog")
    };

    let mut entries = client.get_changelog(&key).await?;

    // Sort newest-first (default). Stable sort keeps original API order as a
    // tiebreaker when `created` timestamps tie.
    entries.sort_by(|a, b| b.created.cmp(&a.created));

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                output::render_json(&ChangelogOutput { key: &key, entries: &entries })?
            );
        }
        OutputFormat::Table => {
            let headers = &["DATE", "AUTHOR", "FIELD", "FROM", "TO"];
            let rows = build_rows(&entries);
            output::print_output(output_format, headers, &rows, &entries)?;
        }
    }

    Ok(())
}

/// Flatten `entries` into one row per `ChangelogItem`, preserving the
/// caller's sort order. Each row becomes `[date, author, field, from, to]`.
fn build_rows(entries: &[ChangelogEntry]) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for entry in entries {
        let date = format_date(&entry.created);
        let author = entry
            .author
            .as_ref()
            .map(|a| a.display_name.clone())
            .unwrap_or_else(|| SYSTEM_AUTHOR.to_string());
        for item in &entry.items {
            rows.push(vec![
                date.clone(),
                author.clone(),
                item.field.clone(),
                from_to_display(item.from_string.as_deref(), item.from.as_deref()),
                from_to_display(item.to_string.as_deref(), item.to.as_deref()),
            ]);
        }
    }
    rows
}

/// Parse a Jira ISO-8601 timestamp and render as `YYYY-MM-DD HH:MM` in the
/// user's local time zone. Falls back to the raw string if parsing fails.
fn format_date(iso: &str) -> String {
    DateTime::parse_from_rfc3339(iso)
        .or_else(|_| DateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.3f%z"))
        .map(|dt| dt.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

/// Prefer the human-readable string; fall back to the raw id; default to
/// the em-dash null marker for empty/missing values.
fn from_to_display(string: Option<&str>, raw: Option<&str>) -> String {
    let pick = string.or(raw).map(str::trim).unwrap_or("");
    if pick.is_empty() {
        NULL_GLYPH.to_string()
    } else {
        pick.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::jira::{ChangelogItem, User};

    fn entry(id: &str, created: &str, author: Option<&str>, items: Vec<ChangelogItem>) -> ChangelogEntry {
        ChangelogEntry {
            id: id.to_string(),
            author: author.map(|name| User {
                account_id: format!("acc-{name}"),
                display_name: name.to_string(),
                email_address: None,
                active: Some(true),
            }),
            created: created.to_string(),
            items,
        }
    }

    fn item(field: &str, from_s: Option<&str>, to_s: Option<&str>) -> ChangelogItem {
        ChangelogItem {
            field: field.to_string(),
            fieldtype: "jira".into(),
            from: None,
            from_string: from_s.map(String::from),
            to: None,
            to_string: to_s.map(String::from),
        }
    }

    #[test]
    fn build_rows_flattens_items_in_order() {
        let entries = vec![entry(
            "1",
            "2026-04-16T14:02:00.000+0000",
            Some("Alice"),
            vec![
                item("status", Some("To Do"), Some("In Progress")),
                item("resolution", None, Some("Done")),
            ],
        )];
        let rows = build_rows(&entries);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][2], "status");
        assert_eq!(rows[1][2], "resolution");
    }

    #[test]
    fn build_rows_uses_system_for_null_author() {
        let entries = vec![entry(
            "1",
            "2026-04-16T14:02:00.000+0000",
            None,
            vec![item("assignee", None, Some("Alice"))],
        )];
        let rows = build_rows(&entries);
        assert_eq!(rows[0][1], SYSTEM_AUTHOR);
    }

    #[test]
    fn from_to_display_renders_em_dash_for_empty() {
        assert_eq!(from_to_display(None, None), NULL_GLYPH);
        assert_eq!(from_to_display(Some(""), None), NULL_GLYPH);
        assert_eq!(from_to_display(None, Some("")), NULL_GLYPH);
    }

    #[test]
    fn from_to_display_prefers_string_over_raw() {
        assert_eq!(from_to_display(Some("Done"), Some("10000")), "Done");
        assert_eq!(from_to_display(None, Some("10000")), "10000");
    }

    #[test]
    fn format_date_converts_rfc3339_to_local() {
        // Just verify the shape; actual local conversion depends on runner TZ.
        let formatted = format_date("2026-04-16T14:02:11.000+0000");
        // YYYY-MM-DD HH:MM is 16 chars.
        assert_eq!(formatted.len(), 16, "got: {formatted}");
        assert!(formatted.starts_with("2026-04-"), "got: {formatted}");
    }

    #[test]
    fn format_date_falls_back_to_raw_on_parse_failure() {
        let garbage = "not-a-date";
        assert_eq!(format_date(garbage), garbage);
    }
}
```

- [ ] **Step 5: Run the integration tests**

```bash
cargo test --test issue_changelog changelog_table_renders changelog_json_preserves -- --nocapture
```

Expected: both PASS.

- [ ] **Step 6: Run the unit tests**

```bash
cargo test --lib cli::issue::changelog -- --nocapture
```

Expected: 6 tests pass (`build_rows_flattens_items_in_order`, `build_rows_uses_system_for_null_author`, `from_to_display_*` x2, `format_date_*` x2).

- [ ] **Step 7: Clippy + fmt**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```

Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add src/cli/issue/changelog.rs tests/issue_changelog.rs
git commit -m "feat(cli): implement minimal 'issue changelog' handler (table + JSON, DESC) (#200)"
```

---

## Task 5: Add `--reverse` flag (ASC ordering)

**Files:**
- Modify: `src/cli/issue/changelog.rs`
- Test: `tests/issue_changelog.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/issue_changelog.rs`:

```rust
#[tokio::test]
async fn changelog_reverse_renders_oldest_first() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "newer",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "In Progress"}]
                },
                {
                    "id": "older",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-14T16:02:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "backend"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--reverse"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let status_idx = stdout.find("status").expect("missing status row");
    let labels_idx = stdout.find("labels").expect("missing labels row");
    // With --reverse, oldest (labels) comes first.
    assert!(labels_idx < status_idx,
        "expected labels (older) before status (newer), got:\n{stdout}");
}
```

- [ ] **Step 2: Run — expect failure**

```bash
cargo test --test issue_changelog changelog_reverse_renders -- --nocapture
```

Expected: FAIL — `--reverse` is declared but not yet honored; newest-first always.

- [ ] **Step 3: Bind the flag and flip the sort**

In `src/cli/issue/changelog.rs`, replace the destructuring and sort call:

```rust
    let IssueCommand::Changelog {
        key,
        limit: _,
        all: _,
        field: _,
        author: _,
        reverse,
    } = command
    else {
        unreachable!("handler only called for IssueCommand::Changelog")
    };

    let mut entries = client.get_changelog(&key).await?;

    if reverse {
        entries.sort_by(|a, b| a.created.cmp(&b.created));
    } else {
        entries.sort_by(|a, b| b.created.cmp(&a.created));
    }
```

- [ ] **Step 4: Run the test**

```bash
cargo test --test issue_changelog changelog_reverse_renders -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Clippy + fmt + commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check
git add src/cli/issue/changelog.rs tests/issue_changelog.rs
git commit -m "feat(cli): honor --reverse on 'issue changelog' (#200)"
```

---

## Task 6: Add `--field` filter (item-level, repeatable)

**Files:**
- Modify: `src/cli/issue/changelog.rs`
- Test: `tests/issue_changelog.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/issue_changelog.rs`:

```rust
#[tokio::test]
async fn changelog_field_filter_keeps_only_matching_items() {
    let server = MockServer::start().await;

    // Single entry with TWO items; --field status should keep only one row.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "1",
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:00.000+0000",
                "items": [
                    {"field": "status", "fieldtype": "jira",
                     "from": "1", "fromString": "To Do",
                     "to": "3", "toString": "In Progress"},
                    {"field": "resolution", "fieldtype": "jira",
                     "from": null, "fromString": null,
                     "to": "10000", "toString": "Done"}
                ]
            }]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--field", "status"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("status"), "status row missing: {stdout}");
    assert!(!stdout.contains("resolution"),
        "resolution row should be filtered out: {stdout}");
}

#[tokio::test]
async fn changelog_field_filter_is_case_insensitive_and_substring() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "1",
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:00.000+0000",
                "items": [
                    {"field": "Story Points", "fieldtype": "custom",
                     "from": null, "fromString": "3", "to": null, "toString": "5"}
                ]
            }]
        })))
        .mount(&server)
        .await;

    // "points" matches "Story Points" via case-insensitive substring.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--field", "points"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Story Points"));
}
```

- [ ] **Step 2: Run — expect failures**

```bash
cargo test --test issue_changelog changelog_field_filter -- --nocapture
```

Expected: both FAIL — filter is ignored.

- [ ] **Step 3: Implement field filtering**

In `src/cli/issue/changelog.rs`, update the destructuring + add the filter step:

```rust
    let IssueCommand::Changelog {
        key,
        limit: _,
        all: _,
        field,
        author: _,
        reverse,
    } = command
    else {
        unreachable!("handler only called for IssueCommand::Changelog")
    };

    let mut entries = client.get_changelog(&key).await?;

    // Sort first (same as before).
    if reverse {
        entries.sort_by(|a, b| a.created.cmp(&b.created));
    } else {
        entries.sort_by(|a, b| b.created.cmp(&a.created));
    }

    // Field filter: drop items whose field doesn't match any --field needle.
    // An entry with zero surviving items is dropped entirely.
    if !field.is_empty() {
        let needles: Vec<String> = field.iter().map(|f| f.to_lowercase()).collect();
        for entry in entries.iter_mut() {
            entry.items.retain(|it| {
                let haystack = it.field.to_lowercase();
                needles.iter().any(|n| haystack.contains(n))
            });
        }
        entries.retain(|e| !e.items.is_empty());
    }
```

Add a unit test alongside the others in the file:

```rust
    #[test]
    fn field_filter_semantics_at_item_level() {
        // Directly test the closure-equivalent logic by building entries.
        let mut entries = vec![entry(
            "1",
            "2026-04-16T14:02:00.000+0000",
            Some("Alice"),
            vec![
                item("status", Some("To Do"), Some("Done")),
                item("resolution", None, Some("Fixed")),
            ],
        )];

        // Simulate the filter logic.
        let needles = ["status"];
        for e in entries.iter_mut() {
            e.items.retain(|it| {
                let h = it.field.to_lowercase();
                needles.iter().any(|n| h.contains(&n.to_lowercase()))
            });
        }
        entries.retain(|e| !e.items.is_empty());

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].items.len(), 1);
        assert_eq!(entries[0].items[0].field, "status");
    }
```

- [ ] **Step 4: Run the tests**

```bash
cargo test --test issue_changelog changelog_field_filter -- --nocapture
cargo test --lib cli::issue::changelog::tests::field_filter -- --nocapture
```

Expected: all pass.

- [ ] **Step 5: Clippy + fmt + commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check
git add src/cli/issue/changelog.rs tests/issue_changelog.rs
git commit -m "feat(cli): implement --field filter for 'issue changelog' (#200)"
```

---

## Task 7: Add `--author` filter (me | name | accountId)

**Files:**
- Modify: `src/cli/issue/changelog.rs`
- Test: `tests/issue_changelog.rs`

- [ ] **Step 1: Write the failing tests**

Append:

```rust
#[tokio::test]
async fn changelog_author_me_resolves_via_myself() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": "me-acc",
            "displayName": "Me User",
            "emailAddress": "me@test.com",
            "active": true
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "me-acc", "displayName": "Me User", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "other", "displayName": "Someone Else", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "me"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Me User"));
    assert!(!stdout.contains("Someone Else"));
}

#[tokio::test]
async fn changelog_author_name_substring_case_insensitive() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "a", "displayName": "Alice Smith", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "b", "displayName": "Bob Jones", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "alice"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Alice Smith"));
    assert!(!stdout.contains("Bob Jones"));
}

#[tokio::test]
async fn changelog_author_accountid_matches_literal() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "abc123", "displayName": "Alice", "active": true },
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "def456", "displayName": "Bob", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "abc123"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Alice"));
    assert!(!stdout.contains("Bob"));
}

#[tokio::test]
async fn changelog_author_null_filtered_out_when_flag_set() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "1", "author": null,
                    "created": "2026-04-16T14:02:00.000+0000",
                    "items": [{"field": "status", "fieldtype": "jira",
                               "from": "1", "fromString": "To Do",
                               "to": "3", "toString": "Done"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-15T10:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "x"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--author", "alice"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // System entry is dropped when --author is set and doesn't match.
    assert!(!stdout.contains("(system)"),
        "(system) row should be filtered when --author set: {stdout}");
    assert!(stdout.contains("Alice"));
}
```

- [ ] **Step 2: Run — expect failures**

```bash
cargo test --test issue_changelog changelog_author -- --nocapture
```

Expected: all FAIL.

- [ ] **Step 3: Implement author filtering**

Update `src/cli/issue/changelog.rs`. First, change the `get_current_user` lookup — the existing API method is `get_myself` (see `src/api/jira/users.rs`). Then modify the destructure + add resolution + filter logic.

Replace the current handler body with:

```rust
pub(super) async fn handle(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Changelog {
        key,
        limit: _,
        all: _,
        field,
        author,
        reverse,
    } = command
    else {
        unreachable!("handler only called for IssueCommand::Changelog")
    };

    // Resolve --author "me" up-front; other forms compare directly.
    let author_needle = match author.as_deref() {
        Some("me") => Some(AuthorNeedle::AccountId(
            client.get_myself().await?.account_id,
        )),
        Some(raw) => Some(classify_author(raw)),
        None => None,
    };

    let mut entries = client.get_changelog(&key).await?;

    // Sort.
    if reverse {
        entries.sort_by(|a, b| a.created.cmp(&b.created));
    } else {
        entries.sort_by(|a, b| b.created.cmp(&a.created));
    }

    // --author filter: drops entries with no author when set, unless
    // the needle matches the null placeholder (we don't support that).
    if let Some(needle) = &author_needle {
        entries.retain(|e| author_matches(e.author.as_ref(), needle));
    }

    // --field filter: drop items, then empty entries.
    if !field.is_empty() {
        let needles: Vec<String> = field.iter().map(|f| f.to_lowercase()).collect();
        for entry in entries.iter_mut() {
            entry.items.retain(|it| {
                let h = it.field.to_lowercase();
                needles.iter().any(|n| h.contains(n))
            });
        }
        entries.retain(|e| !e.items.is_empty());
    }

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                output::render_json(&ChangelogOutput { key: &key, entries: &entries })?
            );
        }
        OutputFormat::Table => {
            let headers = &["DATE", "AUTHOR", "FIELD", "FROM", "TO"];
            let rows = build_rows(&entries);
            output::print_output(output_format, headers, &rows, &entries)?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum AuthorNeedle {
    /// Exact accountId match (literal input or resolved from "me").
    AccountId(String),
    /// Case-insensitive substring match against `displayName`.
    NameSubstring(String),
}

/// Classify a user-supplied `--author` value. We treat a value as an
/// accountId if it looks like one (no whitespace, has a colon or is
/// entirely alphanumeric+dashes and ≥12 chars). Otherwise it's a name
/// substring.
///
/// The API's accountId format varies (`public cloud` uses
/// `557058:...`-style strings; older formats are opaque 24+ char
/// hex-like blobs). The heuristic below is conservative: a plain English
/// name like "alice" is always a substring; anything with a colon or
/// a long alphanumeric blob is treated as literal.
fn classify_author(raw: &str) -> AuthorNeedle {
    let trimmed = raw.trim();
    let looks_like_account_id = trimmed.contains(':')
        || (trimmed.len() >= 12
            && trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    if looks_like_account_id {
        AuthorNeedle::AccountId(trimmed.to_string())
    } else {
        AuthorNeedle::NameSubstring(trimmed.to_lowercase())
    }
}

fn author_matches(author: Option<&crate::types::jira::User>, needle: &AuthorNeedle) -> bool {
    let Some(a) = author else { return false };
    match needle {
        AuthorNeedle::AccountId(id) => a.account_id == *id,
        AuthorNeedle::NameSubstring(n) => a.display_name.to_lowercase().contains(n),
    }
}
```

Add corresponding unit tests in the same file:

```rust
    #[test]
    fn classify_author_treats_short_name_as_substring() {
        match classify_author("alice") {
            AuthorNeedle::NameSubstring(s) => assert_eq!(s, "alice"),
            other => panic!("expected NameSubstring, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_treats_colon_string_as_accountid() {
        match classify_author("557058:abc-123") {
            AuthorNeedle::AccountId(s) => assert_eq!(s, "557058:abc-123"),
            other => panic!("expected AccountId, got {other:?}"),
        }
    }

    #[test]
    fn classify_author_treats_long_hex_blob_as_accountid() {
        match classify_author("abcdef0123456789deadbeef") {
            AuthorNeedle::AccountId(s) => assert_eq!(s, "abcdef0123456789deadbeef"),
            other => panic!("expected AccountId, got {other:?}"),
        }
    }

    #[test]
    fn author_matches_respects_account_id_exact() {
        let user = User {
            account_id: "557058:abc".into(),
            display_name: "Alice".into(),
            email_address: None,
            active: Some(true),
        };
        assert!(author_matches(
            Some(&user),
            &AuthorNeedle::AccountId("557058:abc".into())
        ));
        assert!(!author_matches(
            Some(&user),
            &AuthorNeedle::AccountId("other".into())
        ));
    }

    #[test]
    fn author_matches_null_author_always_false() {
        assert!(!author_matches(
            None,
            &AuthorNeedle::NameSubstring("alice".into())
        ));
    }
```

- [ ] **Step 4: Run the tests**

```bash
cargo test --test issue_changelog changelog_author -- --nocapture
cargo test --lib cli::issue::changelog -- --nocapture
```

Expected: all pass.

- [ ] **Step 5: Clippy + fmt + commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check
git add src/cli/issue/changelog.rs tests/issue_changelog.rs
git commit -m "feat(cli): implement --author filter (me/name/accountId) for 'issue changelog' (#200)"
```

---

## Task 8: Add `--limit` / `--all` truncation (post-filter)

**Files:**
- Modify: `src/cli/issue/changelog.rs`
- Test: `tests/issue_changelog.rs`

Default is 30; `--all` disables truncation; `--limit 0` returns empty.

- [ ] **Step 1: Write the failing tests**

Append:

```rust
#[tokio::test]
async fn changelog_limit_truncates_after_sort() {
    let server = MockServer::start().await;

    // Three entries; we expect --limit 2 to keep the two newest.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 3, "isLast": true,
            "values": [
                {
                    "id": "1",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-10T00:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "oldest"}]
                },
                {
                    "id": "2",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-15T00:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "middle"}]
                },
                {
                    "id": "3",
                    "author": { "accountId": "a", "displayName": "Alice", "active": true },
                    "created": "2026-04-17T00:00:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "newest"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--limit", "2"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("newest"), "missing newest row: {stdout}");
    assert!(stdout.contains("middle"), "missing middle row: {stdout}");
    assert!(!stdout.contains("oldest"),
        "oldest row should be truncated: {stdout}");
}

#[tokio::test]
async fn changelog_limit_zero_renders_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 1, "isLast": true,
            "values": [{
                "id": "1",
                "author": { "accountId": "a", "displayName": "Alice", "active": true },
                "created": "2026-04-16T14:02:00.000+0000",
                "items": [{"field": "status", "fieldtype": "jira",
                           "from": "1", "fromString": "To Do",
                           "to": "3", "toString": "Done"}]
            }]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--limit", "0"])
        .output()
        .unwrap();

    assert!(output.status.success(), "--limit 0 should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The project's `print_output` prints "No results found." for empty tables.
    assert!(stdout.contains("No results found"), "got: {stdout}");
}

#[tokio::test]
async fn changelog_all_disables_truncation() {
    // Generate 40 entries so the default 30 would truncate — verify --all keeps all.
    let server = MockServer::start().await;

    let values: Vec<serde_json::Value> = (0..40)
        .map(|i| json!({
            "id": format!("{}", i),
            "author": { "accountId": "a", "displayName": "Alice", "active": true },
            "created": format!("2026-04-{:02}T00:00:00.000+0000", (i % 28) + 1),
            "items": [{"field": "labels", "fieldtype": "jira",
                       "from": "", "to": format!("v{}", i)}]
        }))
        .collect();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 40, "isLast": true,
            "values": values
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Row count includes header + separator lines; just look for v0 and v39.
    assert!(stdout.contains("v0"), "missing v0: first 32 chars:\n{}", &stdout[..stdout.len().min(120)]);
    assert!(stdout.contains("v39"), "missing v39 — --all did not disable limit");
}

#[tokio::test]
async fn changelog_default_limit_is_thirty() {
    let server = MockServer::start().await;

    let values: Vec<serde_json::Value> = (0..40)
        .map(|i| json!({
            "id": format!("{}", i),
            "author": { "accountId": "a", "displayName": "Alice", "active": true },
            "created": format!("2026-04-{:02}T00:00:00.000+0000", (i % 28) + 1),
            "items": [{"field": "labels", "fieldtype": "jira",
                       "from": "", "to": format!("v{}", i)}]
        }))
        .collect();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 40, "isLast": true,
            "values": values
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Default cap = 30 rows → we should NOT see all 40 values.
    // Count occurrences of "v" followed by digit to estimate.
    let v_count = stdout.matches("v").filter(|m| !m.is_empty()).count();
    // Not an exact count (comfy-table decorations use dashes/bars), but a rough
    // upper bound: should be ≤ ~30 + a handful of decorations.
    assert!(v_count <= 35, "default limit not applied, saw {v_count} 'v' occurrences");
}
```

- [ ] **Step 2: Run — expect failures**

```bash
cargo test --test issue_changelog changelog_limit changelog_all changelog_default_limit -- --nocapture
```

Expected: all FAIL.

- [ ] **Step 3: Implement truncation**

In `src/cli/issue/changelog.rs`, update the destructuring (bind `limit` and `all`) and apply truncation after filtering:

```rust
    let IssueCommand::Changelog {
        key,
        limit,
        all,
        field,
        author,
        reverse,
    } = command
    else {
        unreachable!("handler only called for IssueCommand::Changelog")
    };

    // ... (author resolution, fetch, sort, filters unchanged) ...

    // Flatten rows for the cap check by computing total post-filter rows.
    // `--limit` applies to ROWS (one per item), not entries — a user asking
    // for `--limit 10` expects 10 rows in the table.
    let cap = if all {
        None
    } else {
        Some(limit.unwrap_or(DEFAULT_LIMIT))
    };

    if let Some(n) = cap {
        truncate_to_rows(&mut entries, n as usize);
    }
```

Add the `DEFAULT_LIMIT` constant near the top (matches the value used elsewhere in the CLI) and the `truncate_to_rows` helper:

```rust
/// Same default as `cli::DEFAULT_LIMIT`, copied here to avoid leaking
/// that `pub(crate)` into an unrelated module.
const DEFAULT_LIMIT: u32 = 30;

/// Truncate entries so the total row count (sum of items across all
/// surviving entries) does not exceed `cap`. Trims the last entry's
/// items if necessary rather than dropping a whole entry with only
/// some items over the cap.
fn truncate_to_rows(entries: &mut Vec<ChangelogEntry>, cap: usize) {
    if cap == 0 {
        entries.clear();
        return;
    }
    let mut running = 0usize;
    for i in 0..entries.len() {
        let n = entries[i].items.len();
        if running + n <= cap {
            running += n;
            continue;
        }
        // Partially trim this entry, drop everything after.
        let keep = cap - running;
        entries[i].items.truncate(keep);
        entries.truncate(if keep == 0 { i } else { i + 1 });
        return;
    }
}
```

Also import the type:

```rust
use crate::types::jira::{ChangelogEntry /* existing uses */};
```

(Already imported — just double-check.)

Add unit tests:

```rust
    #[test]
    fn truncate_to_rows_handles_cap_zero() {
        let mut entries = vec![entry("1", "2026-04-16T14:02:00.000+0000", Some("A"),
            vec![item("status", None, Some("Done"))])];
        truncate_to_rows(&mut entries, 0);
        assert!(entries.is_empty());
    }

    #[test]
    fn truncate_to_rows_trims_last_entry_partially() {
        let mut entries = vec![
            entry("1", "2026-04-16T14:02:00.000+0000", Some("A"),
                vec![item("status", None, Some("Done")),
                     item("resolution", None, Some("Fixed"))]),
            entry("2", "2026-04-15T00:00:00.000+0000", Some("A"),
                vec![item("labels", None, Some("x"))]),
        ];
        // cap = 2 → keep both items of entry 1, drop entry 2 entirely.
        truncate_to_rows(&mut entries, 2);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].items.len(), 2);
    }

    #[test]
    fn truncate_to_rows_partial_trim_inside_entry() {
        let mut entries = vec![entry("1", "2026-04-16T14:02:00.000+0000", Some("A"),
            vec![
                item("status", None, Some("Done")),
                item("resolution", None, Some("Fixed")),
                item("labels", None, Some("x")),
            ])];
        truncate_to_rows(&mut entries, 2);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].items.len(), 2);
    }
```

- [ ] **Step 4: Run the tests**

```bash
cargo test --test issue_changelog changelog_limit changelog_all changelog_default_limit -- --nocapture
cargo test --lib cli::issue::changelog::tests::truncate_to_rows -- --nocapture
```

Expected: all pass.

- [ ] **Step 5: Clippy + fmt + commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check
git add src/cli/issue/changelog.rs tests/issue_changelog.rs
git commit -m "feat(cli): implement --limit / --all truncation for 'issue changelog' (#200)"
```

---

## Task 9: Error paths (404 / 403 / network drop)

**Files:**
- Test: `tests/issue_changelog.rs`

The `JiraClient::get` path already maps HTTP status codes to `JrError::ApiError` / `JrError::NotAuthenticated`, and `main.rs` maps those to exit codes and renders the message. These tests lock down that behavior for the new subcommand.

- [ ] **Step 1: Write the failing 404 test**

Append:

```rust
#[tokio::test]
async fn changelog_404_surfaces_not_found() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-999/changelog"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "errorMessages": ["Issue does not exist or you do not have permission to see it."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-999"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("404"), "expected status in stderr: {stderr}");
    assert!(!stderr.contains("panic"), "stderr leaked a panic: {stderr}");
}

#[tokio::test]
async fn changelog_401_suggests_reauth() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errorMessages": ["Client must be authenticated to access this resource."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2),
        "401 should exit 2, got: {:?}", output.status.code());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Not authenticated"),
        "expected 'Not authenticated' in stderr: {stderr}");
    assert!(stderr.contains("jr auth login"));
}

#[tokio::test]
async fn changelog_network_drop_surfaces_reach_error() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "PROJ-1"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Could not reach"),
        "expected 'Could not reach' in stderr: {stderr}");
}
```

- [ ] **Step 2: Run — expect passes (no code change needed)**

```bash
cargo test --test issue_changelog changelog_404 changelog_401 changelog_network_drop -- --nocapture
```

Expected: all PASS. If any fail, the `JiraClient::get` error translation must be wrong for this endpoint — investigate (should not happen; reused infra).

- [ ] **Step 3: Write the failing empty-response test**

Append:

```rust
#[tokio::test]
async fn changelog_empty_response_exit_zero_with_empty_table() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 0, "isLast": true,
            "values": []
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1"])
        .output()
        .unwrap();

    assert!(output.status.success(), "empty response should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No results found"),
        "expected empty-state message: {stdout}");
}

#[tokio::test]
async fn changelog_empty_response_json_has_empty_entries() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 0, "isLast": true,
            "values": []
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--output", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(parsed["key"], "FOO-1");
    assert_eq!(parsed["entries"].as_array().unwrap().len(), 0);
}
```

- [ ] **Step 4: Run — expect pass**

```bash
cargo test --test issue_changelog changelog_empty -- --nocapture
```

Expected: both pass. If the table case fails because "No results found" isn't emitted, it means `output::print_output` wasn't called with empty rows — double-check `build_rows` returns `Vec::new()` when entries is empty.

- [ ] **Step 5: Clippy + fmt + commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check
git add tests/issue_changelog.rs
git commit -m "test: lock error + empty-path behavior for 'issue changelog' (#200)"
```

---

## Task 10: JSON snapshot test (insta)

**Files:**
- Test: `tests/issue_changelog.rs`
- Create: `tests/snapshots/` (insta creates this automatically)

Uses `insta::assert_json_snapshot!` to pin the JSON output shape. Matches the project's existing use in `src/cli/sprint.rs`. Running `cargo insta review` is the standard way to accept a new snapshot.

- [ ] **Step 1: Add the snapshot test**

Append to `tests/issue_changelog.rs`:

```rust
#[tokio::test]
async fn changelog_json_output_snapshot() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "startAt": 0, "maxResults": 100, "total": 2, "isLast": true,
            "values": [
                {
                    "id": "2",
                    "author": { "accountId": "alice", "displayName": "Alice Smith", "active": true },
                    "created": "2026-04-16T14:02:11.000+0000",
                    "items": [
                        {"field": "status", "fieldtype": "jira",
                         "from": "1", "fromString": "To Do",
                         "to": "3", "toString": "In Progress"},
                        {"field": "resolution", "fieldtype": "jira",
                         "from": null, "fromString": null,
                         "to": "10000", "toString": "Done"}
                    ]
                },
                {
                    "id": "1", "author": null,
                    "created": "2026-04-14T16:02:00.000+0000",
                    "items": [{"field": "labels", "fieldtype": "jira",
                               "from": "", "to": "backend"}]
                }
            ]
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "FOO-1", "--output", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    insta::assert_json_snapshot!(parsed);
}
```

- [ ] **Step 2: Run the test (first pass — creates a pending snapshot)**

```bash
cargo test --test issue_changelog changelog_json_output_snapshot -- --nocapture
```

Expected: FAIL with `snapshot file does not match ...`. Review the pending `.snap.new` file.

- [ ] **Step 3: Review and accept the snapshot**

If `cargo-insta` is installed:

```bash
cargo insta review
```

Alternative (inspect and rename manually):

```bash
ls tests/snapshots/issue_changelog__changelog_json_output_snapshot.snap.new
# Inspect the file contents. Once satisfied:
mv tests/snapshots/issue_changelog__changelog_json_output_snapshot.snap.new \
   tests/snapshots/issue_changelog__changelog_json_output_snapshot.snap
```

- [ ] **Step 4: Re-run to confirm pass**

```bash
cargo test --test issue_changelog changelog_json_output_snapshot
```

Expected: PASS.

- [ ] **Step 5: Clippy + fmt + commit**

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check
git add tests/issue_changelog.rs tests/snapshots/
git commit -m "test: add JSON snapshot for 'issue changelog' output (#200)"
```

---

## Task 11: Full CI-equivalent local check + final sanity

- [ ] **Step 1: Run every CI check locally**

Run these in the order CI runs them. Do not skip:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all pass. If `cargo test` fails for a test outside this feature, investigate — this plan may have touched shared state inadvertently.

- [ ] **Step 2: Smoke test `--help` locally**

```bash
cargo run --quiet -- issue changelog --help
```

Expected: lists the subcommand summary + all flags (`--limit`, `--all`, `--field`, `--author`, `--reverse`, global flags).

- [ ] **Step 3: Review the full diff against the spec**

```bash
git diff origin/develop...feat/issue-changelog -- ':!docs/'
```

Walk through every section of `docs/specs/issue-changelog.md` and confirm each requirement is implemented:

- Flags all present with correct names and conflicts
- Field filter at item level (not entry level)
- `(system)` placeholder for null author
- `—` (em dash) for null from/to values
- Local-time table dates, ISO-8601 in JSON
- Default limit 30; `--all` disables truncation
- `--reverse` flips sort
- New files in expected locations; `list.rs` untouched

- [ ] **Step 4: Confirm nothing unrelated was committed**

```bash
git log origin/develop..feat/issue-changelog --oneline
```

Expected: commits touch only the files listed in the plan. No stray `.claude/scheduled_tasks.lock`, no unrelated tweaks.

- [ ] **Step 5: If everything green, stop. This plan is complete.**

Next phase (out of scope for this plan): PR review loop via `/pr-review-toolkit:review-pr`, then PR creation + Copilot review. The orchestrator (`/validated-feature-lifecycle`) drives those.

---

## Self-Review Summary

**Spec coverage:**

- CLI surface (flags, conflicts, defaults) → Task 3 + Tasks 5–8 cover each flag individually.
- Types (ChangelogEntry, ChangelogItem, Option<User>, camelCase) → Task 1.
- API method on existing impl block → Task 2.
- File layout (changelog.rs, types/jira/changelog.rs) → Tasks 1, 3.
- Fetch/filter/sort/limit algorithm → Tasks 2 (fetch), 4 (sort), 6 (field), 7 (author), 8 (limit).
- Output formats: flat table, nested JSON → Task 4 (baseline), Task 10 (snapshot).
- Error paths 404 / 403 / 401 / network → Task 9.
- `(system)` placeholder, `—` null glyph → Task 4.
- Empty response → Task 9.
- `--limit 0` behavior → Task 8.

**Placeholder scan:** No TBD / TODO / "similar to Task N" references. Code in every step. Exact commands for test runs.

**Type consistency:**

- `ChangelogEntry`, `ChangelogItem`, `AuthorNeedle`, `truncate_to_rows`, `build_rows`, `format_date`, `from_to_display`, `classify_author`, `author_matches` — all defined in the task that introduces them and used consistently thereafter.
- Method name `get_myself` (not `get_current_user`) — confirmed against `src/api/jira/users.rs`.
- `User::account_id` / `User::display_name` / `User::email_address` / `User::active` — confirmed against `src/types/jira/user.rs`.
- `OffsetPage::values` field and `OffsetPage::has_more()` / `next_start()` — confirmed against `src/api/pagination.rs`.
- `urlencoding::encode` for path-segment escaping — matches other methods in `issues.rs`.
- `output::print_output` / `output::render_json` — confirmed signatures in `src/output.rs`.
- `JrError::exit_code` mapping (401 → 2, 5xx → 1) — confirmed via `src/error.rs` and the existing `tests/comments.rs` error cases.

Spec fidelity is intact; no gaps.
