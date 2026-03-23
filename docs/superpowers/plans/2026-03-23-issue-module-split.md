# Issue Module Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `src/cli/issue.rs` (1426 lines) into a directory module with 7 focused files.

**Architecture:** Convert `issue.rs` to `issue/` directory. `mod.rs` handles dispatch and re-exports. Handlers grouped by operation theme into submodules. `pub(super)` visibility for internal functions, `pub` for re-exported format helpers.

**Tech Stack:** Rust, clap (CLI framework)

**Spec:** `docs/superpowers/specs/2026-03-23-issue-module-split-design.md`

---

## File Structure

| File | Responsibility | Source Lines |
|------|---------------|-------------|
| `src/cli/issue/mod.rs` | Dispatch match + re-exports | New (~60 lines) |
| `src/cli/issue/format.rs` | Issue row formatting, headers, points display | Lines 12-79, 1324-1333 |
| `src/cli/issue/list.rs` | handle_list, build_fallback_jql, handle_view | Lines 134-431 |
| `src/cli/issue/create.rs` | handle_create, handle_edit | Lines 435-686 |
| `src/cli/issue/workflow.rs` | handle_move, handle_transitions, handle_assign, handle_comment, handle_open | Lines 687-1042 |
| `src/cli/issue/links.rs` | handle_link, handle_unlink, handle_link_types | Lines 1043-1255 |
| `src/cli/issue/helpers.rs` | resolve_team_field, resolve_story_points_field_id, prompt_input | Lines 1259-1354 |

---

### Task 1: Create directory module with format.rs, helpers.rs, and mod.rs — delete old issue.rs

**Files:**
- Delete: `src/cli/issue.rs`
- Create: `src/cli/issue/mod.rs`
- Create: `src/cli/issue/format.rs`
- Create: `src/cli/issue/helpers.rs`

This must be atomic — Rust cannot have both `issue.rs` and `issue/` simultaneously. We create the directory, extract `format.rs` and `helpers.rs`, put all remaining handlers temporarily in `mod.rs`, and delete the old file in one step.

- [ ] **Step 1: Create the directory**

```bash
mkdir -p src/cli/issue
```

- [ ] **Step 2: Create `format.rs`**

Create `src/cli/issue/format.rs` with formatting functions from issue.rs lines 12-79 and 1324-1333, plus the 3 `format_points` tests from lines 1407-1425.

```rust
use crate::types::jira::Issue;

/// Format issue rows for table output.
pub fn format_issue_rows_public(issues: &[Issue]) -> Vec<Vec<String>> {
    // Copy lines 13-18 exactly
}

/// Build a single table row for an issue, optionally including story points.
pub fn format_issue_row(issue: &Issue, sp_field_id: Option<&str>) -> Vec<String> {
    // Copy lines 21-68 exactly
}

/// Headers matching `format_issue_row` output.
pub fn issue_table_headers(show_points: bool) -> Vec<&'static str> {
    // Copy lines 71-79 exactly
}

/// Format a story point value for display.
pub fn format_points(value: f64) -> String {
    // Copy lines 1324-1333 exactly
}

#[cfg(test)]
mod tests {
    use super::*;

    // Copy 3 format_points tests from lines 1407-1425
}
```

- [ ] **Step 3: Create `helpers.rs`**

Create `src/cli/issue/helpers.rs` with shared utilities from lines 1259-1354. Change visibility to `pub(super)`. Note: the source uses fully-qualified paths (`crate::partial_match::MatchResult::Exact`, `crate::cache::read_team_cache`, `crate::cli::team::fetch_and_cache_teams`) — copy those as-is. Do NOT add unused imports.

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::config::Config;

pub(super) async fn resolve_team_field(
    config: &Config,
    client: &JiraClient,
    team_name: &str,
    no_input: bool,
) -> Result<(String, String)> {
    // Copy lines 1259-1323 exactly (uses crate::cache, crate::cli::team,
    // crate::partial_match, and dialoguer via fully-qualified paths)
}

pub(super) fn resolve_story_points_field_id(config: &Config) -> Result<String> {
    // Copy lines 1335-1347 exactly
}

pub(super) fn prompt_input(prompt: &str) -> Result<String> {
    // Copy lines 1348-1354 exactly
}
```

- [ ] **Step 4: Create `mod.rs` with all remaining handlers**

Create `src/cli/issue/mod.rs` with module declarations, re-exports, the `handle()` dispatch, and ALL handler functions temporarily. Update internal calls to use `helpers::` and `format::` prefixes.

```rust
mod format;
mod helpers;

pub use format::{format_issue_row, format_issue_rows_public, format_points, issue_table_headers};

use anyhow::{Result, bail};
use serde_json::json;

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::output;
use crate::partial_match::{self, MatchResult};
use crate::types::jira::Issue;

pub async fn handle(/* same signature as original */) -> Result<()> {
    // Same dispatch match — calls local functions for now
}

// ALL handlers from lines 134-1258 go here temporarily
// Replace direct calls:
//   format_points(x)  →  format::format_points(x)
//   resolve_team_field(...)  →  helpers::resolve_team_field(...)
//   resolve_story_points_field_id(...)  →  helpers::resolve_story_points_field_id(...)
//   prompt_input(...)  →  helpers::prompt_input(...)

#[cfg(test)]
mod tests {
    use super::*;
    // Copy 5 build_fallback_jql tests from lines 1360-1405
}
```

- [ ] **Step 5: Delete old file**

```bash
rm src/cli/issue.rs
```

- [ ] **Step 6: Run tests and lint**

```bash
cargo test
cargo clippy --all --all-features --tests -- -D warnings
cargo fmt --all
```
Expected: All tests pass, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add src/cli/issue/ src/cli/issue.rs
git commit -m "refactor: convert issue.rs to directory module with format.rs and helpers.rs

Extract formatting helpers to issue/format.rs and shared utilities
to issue/helpers.rs. All handlers remain in mod.rs temporarily."
```

Note: `git add src/cli/issue.rs` stages the deletion. If git doesn't track the deleted file path, use `git add -u src/cli/` instead.

---

### Task 2: Extract list.rs

**Files:**
- Create: `src/cli/issue/list.rs`
- Modify: `src/cli/issue/mod.rs`

- [ ] **Step 1: Create `list.rs`**

Create `src/cli/issue/list.rs` with `handle_list`, `build_fallback_jql`, and `handle_view` from `mod.rs`.

```rust
use anyhow::Result;
use serde_json::json;

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::output;
use crate::types::jira::Issue;

use super::format::{format_issue_row, format_points, issue_table_headers};
use super::helpers::{resolve_story_points_field_id, resolve_team_field};

pub(super) async fn handle_list(/* copy signature */) -> Result<()> {
    // Copy body from mod.rs
}

fn build_fallback_jql(/* copy signature */) -> String {
    // Copy body — stays private, only used by handle_list
}

pub(super) async fn handle_view(/* copy signature */) -> Result<()> {
    // Copy body — uses adf::adf_to_text and format_points
}

#[cfg(test)]
mod tests {
    use super::*;
    // Move 5 build_fallback_jql tests here from mod.rs
}
```

- [ ] **Step 2: Update `mod.rs`**

1. Add `mod list;`
2. Remove `handle_list`, `build_fallback_jql`, `handle_view` and their tests from mod.rs
3. Update dispatch to use `list::handle_list(...)` and `list::handle_view(...)`
4. Remove imports that are now unused in mod.rs

- [ ] **Step 3: Run tests**

```bash
cargo test
```
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue/list.rs src/cli/issue/mod.rs
git commit -m "refactor: extract handle_list and handle_view to issue/list.rs"
```

---

### Task 3: Extract create.rs

**Files:**
- Create: `src/cli/issue/create.rs`
- Modify: `src/cli/issue/mod.rs`

- [ ] **Step 1: Create `create.rs`**

```rust
use anyhow::Result;
use serde_json::json;

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::output;

use super::helpers::{prompt_input, resolve_story_points_field_id, resolve_team_field};

pub(super) async fn handle_create(/* copy signature */) -> Result<()> {
    // Copy body from mod.rs
}

pub(super) async fn handle_edit(/* copy signature */) -> Result<()> {
    // Copy body from mod.rs
}
```

- [ ] **Step 2: Update `mod.rs`**

1. Add `mod create;`
2. Remove `handle_create`, `handle_edit` from mod.rs
3. Update dispatch to use `create::handle_create(...)` and `create::handle_edit(...)`
4. Remove unused imports

- [ ] **Step 3: Run tests**

```bash
cargo test
```
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue/create.rs src/cli/issue/mod.rs
git commit -m "refactor: extract handle_create and handle_edit to issue/create.rs"
```

---

### Task 4: Extract workflow.rs

**Files:**
- Create: `src/cli/issue/workflow.rs`
- Modify: `src/cli/issue/mod.rs`

- [ ] **Step 1: Create `workflow.rs`**

```rust
use anyhow::{Result, bail};
use serde_json::json;

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::output;
use crate::partial_match::{self, MatchResult};

use super::helpers::prompt_input;

pub(super) async fn handle_move(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> { /* copy body */ }

pub(super) async fn handle_transitions(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> { /* copy body */ }

pub(super) async fn handle_assign(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> { /* copy body */ }

pub(super) async fn handle_comment(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> { /* copy body */ }

pub(super) async fn handle_open(
    command: IssueCommand,
    client: &JiraClient,
) -> Result<()> { /* copy body */ }
```

- [ ] **Step 2: Update `mod.rs`**

1. Add `mod workflow;`
2. Remove all 5 handler functions from mod.rs
3. Update dispatch to use `workflow::` prefix
4. Remove unused imports

- [ ] **Step 3: Run tests**

```bash
cargo test
```
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue/workflow.rs src/cli/issue/mod.rs
git commit -m "refactor: extract workflow handlers to issue/workflow.rs"
```

---

### Task 5: Extract links.rs

**Files:**
- Create: `src/cli/issue/links.rs`
- Modify: `src/cli/issue/mod.rs`

- [ ] **Step 1: Create `links.rs`**

```rust
use anyhow::Result;
use serde_json::json;

use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::output;
use crate::partial_match::{self, MatchResult};

pub(super) async fn handle_link_types(
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> { /* copy body */ }

pub(super) async fn handle_link(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> { /* copy body */ }

pub(super) async fn handle_unlink(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> { /* copy body */ }
```

- [ ] **Step 2: Update `mod.rs`**

1. Add `mod links;`
2. Remove all 3 link handler functions from mod.rs
3. Update dispatch to use `links::` prefix
4. Remove all unused imports — mod.rs should now only need:
```rust
use anyhow::Result;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
```

- [ ] **Step 3: Run tests and full CI validation**

```bash
cargo test
cargo clippy --all --all-features --tests -- -D warnings
cargo fmt --all -- --check
```
Expected: All tests pass, zero warnings, formatting clean.

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue/links.rs src/cli/issue/mod.rs
git commit -m "refactor: extract link handlers to issue/links.rs"
```

---

### Task 6: Final cleanup and CLAUDE.md update

**Files:**
- Verify: `src/cli/issue/mod.rs` (dispatch-only)
- Modify: `CLAUDE.md`

- [ ] **Step 1: Verify `mod.rs` is dispatch-only**

`mod.rs` should now contain only:
- 6 `mod` declarations
- 1 `pub use` line
- The `handle()` function (~50 lines of dispatch)
- No handler implementations, no helpers, no tests

If any handler code remains in mod.rs, move it to the appropriate submodule.

- [ ] **Step 2: Update CLAUDE.md architecture tree**

Replace the `issue.rs` line in the CLI section with:

```
│   ├── issue/           # issue commands (split by operation theme)
│   │   ├── mod.rs       # dispatch + re-exports
│   │   ├── format.rs    # row formatting, headers, points display
│   │   ├── list.rs      # list + view (read operations)
│   │   ├── create.rs    # create + edit (field-building)
│   │   ├── workflow.rs  # move + transitions + assign + comment + open
│   │   ├── links.rs     # link + unlink + link-types
│   │   └── helpers.rs   # team/points resolution, prompts
```

- [ ] **Step 3: Run full test suite**

```bash
cargo test
cargo clippy --all --all-features --tests -- -D warnings
cargo fmt --all
```
Expected: All tests pass, zero warnings, formatting clean.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md src/cli/issue/mod.rs
git commit -m "refactor: finalize issue module split, update CLAUDE.md

Split src/cli/issue.rs (1426 lines) into 7 focused submodules:
- format.rs (~80 lines) - row formatting and points display
- list.rs (~280 lines) - list and view handlers
- create.rs (~270 lines) - create and edit handlers
- workflow.rs (~340 lines) - move, transitions, assign, comment, open
- links.rs (~230 lines) - link, unlink, link-types
- helpers.rs (~140 lines) - shared utilities
- mod.rs (~60 lines) - dispatch and re-exports

No behavior changes."
```
