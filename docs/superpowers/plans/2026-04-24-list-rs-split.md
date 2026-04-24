# Split `cli/issue/list.rs` into focused files — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce `src/cli/issue/list.rs` from 1,498 lines to ~700 by extracting `handle_view` to `view.rs`, `handle_comments` to `comments.rs`, and relocating three comment-formatting helpers + their 5 inline tests to the existing `format.rs`. No behavior changes.

**Architecture:** Pure code motion; no logic changes. The `cli/issue/` submodule already splits by operation theme (create.rs, workflow.rs, links.rs, changelog.rs, etc.) — this plan extends that pattern. Visibility stays at `pub(super)` (correct modifier for intra-module sibling access per Rust API Guidelines). Shared comment helpers go to `format.rs` rather than cross-importing between sibling handler files (the latter is a code smell per rustc/ripgrep conventions). Land as one atomic commit; intermediate tasks verify with `cargo check`/`cargo test` but do not commit until Task 6.

**Tech Stack:** Rust 1.85 MSRV, tokio async, anyhow errors. No new deps.

**Related docs:**
- Spec: `docs/specs/list-rs-split.md`
- Parent convention: `CLAUDE.md` (cli/issue/* layout, Rust conventions, test discipline)

**TDD note (pure-motion refactor):** This plan has no red-green-refactor cycles because no new behavior is added. The test-first discipline here is: (1) prove all tests green before starting (Task 1), (2) keep all tests green after each mechanical move (verification step in each task), (3) commit once the final state is also green (Task 6). Any "red" in the middle means a motion error (missing import, wrong visibility, accidentally reordered logic) — fix immediately, don't advance.

---

## File Structure (after completion)

```
src/cli/issue/
├── assets.rs          # unchanged (65 lines)
├── changelog.rs       # unchanged (847 lines)
├── comments.rs        # NEW — handle_comments only (~60 lines)
├── create.rs          # unchanged (375 lines)
├── format.rs          # gains 3 fns + 5 tests (139 → ~215 lines)
├── helpers.rs         # unchanged (812 lines)
├── json_output.rs     # unchanged (149 lines)
├── links.rs           # unchanged (293 lines)
├── list.rs            # reduced — handle_list + list-only helpers + 22 tests (1498 → ~700 lines)
├── mod.rs             # +2 lines (register modules, rewire 2 dispatch arms)
├── view.rs            # NEW — handle_view only (~740 lines)
└── workflow.rs        # unchanged (786 lines)
```

**Helper ownership:**

| Helper | Current home | Destination | Why |
|---|---|---|---|
| `extract_unique_status_names` | `list.rs:24` | stays | used only by `handle_list` |
| `build_jql_base_parts` | `list.rs:44` | stays | used only by `handle_list` |
| `FilterOptions` struct | `list.rs:602` | stays | used only by `build_filter_clauses` (list-only) |
| `build_filter_clauses` | `list.rs:617` | stays | used only by `handle_list` |
| `resolve_show_points` | `list.rs:583` | stays | used only by `handle_list` |
| `format_comment_date` | `list.rs:657` | `format.rs` | shared by `handle_view` + `handle_comments` |
| `format_comment_row` | `list.rs:673` | `format.rs` | shared by `handle_view` + `handle_comments` |
| `comment_visibility` | `list.rs:690` | `format.rs` | shared by `handle_view` + `handle_comments` |

**Inline test inventory (all 30):**

| Tests | Count | Destination |
|---|---|---|
| `resolve_show_points_*` | 3 | stay in `list.rs` |
| `build_jql_parts_*` (exercise `FilterOptions` + `build_filter_clauses`) | 15 | stay in `list.rs` |
| `build_jql_base_parts_*` | 6 | stay in `list.rs` |
| `extract_unique_status_names_*` | 2 | stay in `list.rs` |
| `format_comment_date_*` | 3 | move to `format.rs` |
| `format_comment_row_*` | 2 | move to `format.rs` |

---

## Task 1: Establish green baseline

**Files:**
- Read only — no changes.

**Why:** Any refactor must start from a green state. If tests fail now, the refactor won't be safely reversible.

- [ ] **Step 1: Verify you are on the refactor branch in a worktree off develop**

Run:
```bash
git rev-parse --show-toplevel     # should print the worktree path, not main repo
git branch --show-current         # should print refactor/list-rs-split (or similar worktree branch)
git log --oneline -3              # base should be develop's tip
```

- [ ] **Step 2: Run the full CI-equivalent check set**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all three pass, zero warnings, every test passes. If anything fails, stop and report — do not start the refactor on a red baseline.

- [ ] **Step 3: Snapshot the file sizes you're about to change**

Run:
```bash
wc -l src/cli/issue/list.rs src/cli/issue/format.rs src/cli/issue/mod.rs
```

Expected (approximate):
```
   1498 src/cli/issue/list.rs
    139 src/cli/issue/format.rs
     91 src/cli/issue/mod.rs
```

Record these — the success criteria in the spec reference the target post-refactor sizes.

**No commit.** This task is preflight only.

---

## Task 2: Move `format_comment_date`, `format_comment_row`, `comment_visibility` (and their 5 tests) from `list.rs` to `format.rs`

**Files:**
- Modify: `src/cli/issue/list.rs` — remove lines 657-702 (three fn definitions) and the 5 test fns (`format_comment_date_rfc3339`, `format_comment_date_jira_offset_no_colon`, `format_comment_date_malformed_returns_raw`, `format_comment_row_missing_author`, `format_comment_row_missing_body`) from the `#[cfg(test)] mod tests` block (lines ~1053-1088). Update two internal references inside `list.rs` (in the still-present `handle_view` and `handle_comments` bodies) to call `super::format::format_comment_date`, `super::format::format_comment_row`, `super::format::comment_visibility`.
- Modify: `src/cli/issue/format.rs` — append the three function definitions with `pub(super)` visibility and their 5 tests inside a `#[cfg(test)] mod tests` block.

- [ ] **Step 1: Read `format.rs` top section to understand its existing imports**

Run:
```bash
head -20 src/cli/issue/format.rs
```

Note: `format.rs` already imports `comfy_table`, `serde_json::Value`, and several crate-local types. Check whether `crate::types::jira::issue::Comment` is already imported. If not, you will add it in Step 3.

- [ ] **Step 2: Read the three target functions from `list.rs`**

Run:
```bash
sed -n '657,702p' src/cli/issue/list.rs
```

Copy the exact function bodies — including any doc comments. The functions are:

```rust
fn format_comment_date(iso: &str, verbose: bool) -> String { /* ~14 lines */ }

fn format_comment_row(
    author: Option<&str>,
    created: Option<&str>,
    body: Option<&str>,
    verbose: bool,
) -> String { /* ~15 lines */ }

fn comment_visibility(comment: &Comment) -> Option<&'static str> { /* ~12 lines */ }
```

- [ ] **Step 3: Append the three functions to `format.rs` with `pub(super)` visibility**

Open `src/cli/issue/format.rs`. Add at the top of the file (if not already present):

```rust
use crate::types::jira::issue::Comment;
```

At the end of the file (but **before** any existing `#[cfg(test)] mod tests` block, or append to the bottom if no test module exists yet), paste the three function bodies, prefixing each with `pub(super)`:

```rust
pub(super) fn format_comment_date(iso: &str, verbose: bool) -> String {
    // paste exact body from list.rs:657
}

pub(super) fn format_comment_row(
    author: Option<&str>,
    created: Option<&str>,
    body: Option<&str>,
    verbose: bool,
) -> String {
    // paste exact body from list.rs:673
}

pub(super) fn comment_visibility(comment: &Comment) -> Option<&'static str> {
    // paste exact body from list.rs:690
}
```

Do **not** modify the function bodies — this is pure motion. Preserve any inline comments inside the bodies.

- [ ] **Step 4: Move the 5 tests to `format.rs`**

Read the test fns from `list.rs`:

```bash
sed -n '1052,1088p' src/cli/issue/list.rs
```

The 5 tests are:
- `format_comment_date_rfc3339` (line ~1053)
- `format_comment_date_jira_offset_no_colon` (line ~1061)
- `format_comment_date_malformed_returns_raw` (line ~1069)
- `format_comment_row_missing_author` (line ~1074)
- `format_comment_row_missing_body` (line ~1080)

If `format.rs` has no `#[cfg(test)] mod tests` block yet, append this scaffold at EOF:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    // paste the 5 tests here verbatim, preserving #[test] attributes
}
```

If `format.rs` already has a test module, insert the 5 tests inside it.

- [ ] **Step 5: Remove the three functions from `list.rs`**

Delete lines 657-702 from `src/cli/issue/list.rs` (the three `fn format_comment_date`, `fn format_comment_row`, `fn comment_visibility` definitions including their doc comments). Use the Edit tool with the exact old_string spanning those lines.

- [ ] **Step 6: Remove the 5 moved tests from `list.rs`'s test module**

Delete the 5 test fns from the `#[cfg(test)] mod tests` block in `list.rs` (approximately lines 1053-1088). The other 25 tests stay.

- [ ] **Step 7: Update `handle_view` and `handle_comments` inside `list.rs` to reference the moved helpers**

`handle_view` and `handle_comments` (still in `list.rs` at this point) currently call the three helpers as unqualified names. Change each call site to `super::format::format_comment_date`, `super::format::format_comment_row`, and `super::format::comment_visibility`.

Usage sites to update (verify with grep):

```bash
grep -n 'format_comment_date\|format_comment_row\|comment_visibility' src/cli/issue/list.rs
```

Expected sites (pre-change line numbers):
- `handle_comments`: line ~682 (`format_comment_date`), ~713 (`comment_visibility`), ~728 (`comment_visibility`), ~730 (`format_comment_row`)
- `handle_view`: line ~744 (`format_comment_row`), ~884 (`format_comment_date`), ~893 (`format_comment_date`)

Replace each unqualified call (e.g., `format_comment_date(...)`) with the fully-qualified `super::format::format_comment_date(...)`.

Alternative (fewer edits): add `use super::format::{format_comment_date, format_comment_row, comment_visibility};` to the imports section at the top of `list.rs`. This preserves the current call-site syntax. Pick whichever the codebase style favors — checking other imports from `format.rs` in `list.rs` shows it already does `use super::format;`, so either style works. **Preferred: add the specific `use` statement** to minimize diff.

- [ ] **Step 8: Verify compilation and tests**

Run:
```bash
cargo check 2>&1 | tail -20
cargo test 2>&1 | tail -10
```

Expected: both pass. If `cargo check` reports "unresolved import" or "cannot find function", re-check the imports added in Step 7. If `cargo test` reports a failure in one of the 5 moved tests, re-check Step 4 (test move).

- [ ] **Step 9: Verify clippy + fmt**

Run:
```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: clippy silent (zero warnings). If clippy warns about `pub(super)` on an unreachable item, it means `list.rs` stopped using it — investigate before proceeding.

**No commit yet.** The working tree now has `format.rs` gaining three `pub(super)` fns + 5 tests, and `list.rs` losing them plus the 5 tests. `handle_view` and `handle_comments` still live in `list.rs` but call through `super::format::`.

---

## Task 3: Extract `handle_view` to `src/cli/issue/view.rs`

**Files:**
- Create: `src/cli/issue/view.rs`
- Modify: `src/cli/issue/list.rs` — remove `handle_view` definition (post-Task-2 location, starts near line ~759)
- Modify: `src/cli/issue/mod.rs` — add `mod view;` and rewire the `IssueCommand::View { .. }` dispatch arm

- [ ] **Step 1: Read the current `handle_view` body from `list.rs`**

Run:
```bash
grep -n 'async fn handle_view' src/cli/issue/list.rs
```

Note the start line. Then read from that line to the end of the function (the closing `}` at column 1). `handle_view` is ~739 lines. Read it in chunks if needed:

```bash
sed -n '759,1500p' src/cli/issue/list.rs
```

(Line numbers have shifted slightly after Task 2's deletions. Use the grep above to find the actual current start.)

- [ ] **Step 2: Identify all imports `handle_view` needs**

Inside `handle_view`, the function body references:
- crate-local types: `Comment` (from `crate::types::jira::issue::Comment`), other `crate::types::jira::issue::*` items
- `crate::api::client::JiraClient`
- `crate::cli::{IssueCommand, OutputFormat}`
- `crate::config::Config`
- `crate::error::JrError` (if error paths use typed errors)
- `crate::output`
- `crate::adf`
- `crate::api::assets::linked::{cmdb_field_ids, enrich_assets, ...}` (if `handle_view` surfaces assets)
- `super::format::{format_comment_date, format_comment_row, comment_visibility, ...}` (per Task 2 result)
- `super::helpers`
- `anyhow::Result`

Build the import list by reading the existing imports at the top of `list.rs` and keeping only the ones the moved function actually uses. When in doubt, copy all imports and let `cargo fix --allow-dirty` or Clippy's `unused_imports` trim them at Step 5.

- [ ] **Step 3: Create `src/cli/issue/view.rs` with the full function + imports**

Structure:

```rust
use anyhow::Result;

// ... exact imports handle_view needs (see Step 2)
use super::format::{format_comment_date, format_comment_row, comment_visibility /*, …*/};
use super::helpers;

pub(super) async fn handle_view(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    // paste handle_view body verbatim from list.rs
}
```

Signature must match the dispatch call in `mod.rs` — confirm by reading `mod.rs` line ~41 before pasting.

- [ ] **Step 4: Remove `handle_view` from `list.rs`**

Use the Edit tool. `old_string` is the complete `pub(super) async fn handle_view(...) -> Result<()> { ... }` from start to the matching closing `}`. `new_string` is empty (or a single blank line between the surrounding items, whichever keeps `rustfmt` happy).

- [ ] **Step 5: Register `view` module and rewire dispatch in `src/cli/issue/mod.rs`**

Read the current `mod.rs` (91 lines). Make two changes:

a) Add `mod view;` to the module registrations at the top (alphabetical placement between `mod links;` and `mod workflow;` is ideal):

```rust
mod assets;
mod changelog;
mod comments;  // added in Task 4 — add now as a no-op? No — add in Task 4 to keep diffs isolated.
mod create;
mod format;
mod helpers;
mod json_output;
mod links;
mod list;
mod view;       // <-- ADD THIS LINE in Task 3
mod workflow;
```

(Skip the `comments` entry for now — Task 4 adds it.)

b) Rewire the `View` dispatch arm:

Find:
```rust
IssueCommand::View { .. } => {
    list::handle_view(command, output_format, config, client).await
}
```

Change `list::handle_view` to `view::handle_view`:
```rust
IssueCommand::View { .. } => {
    view::handle_view(command, output_format, config, client).await
}
```

- [ ] **Step 6: Verify compilation and tests**

Run:
```bash
cargo fmt --all
cargo check 2>&1 | tail -30
cargo test 2>&1 | tail -20
```

Expected: compilation succeeds, all tests pass. If `cargo check` reports "unresolved import" for a crate module, re-examine the imports added in Step 3. If it reports "unused import", remove the stale one from `view.rs` or `list.rs`.

- [ ] **Step 7: Verify clippy**

Run:
```bash
cargo clippy --all-targets -- -D warnings
```

Expected: zero warnings. A common miss here is an unused import left behind in `list.rs` (imports that `handle_view` needed but `handle_list` doesn't). Delete them.

**No commit yet.**

---

## Task 4: Extract `handle_comments` to `src/cli/issue/comments.rs`

**Files:**
- Create: `src/cli/issue/comments.rs`
- Modify: `src/cli/issue/list.rs` — remove `handle_comments` definition
- Modify: `src/cli/issue/mod.rs` — add `mod comments;` and rewire the `IssueCommand::Comments { .. }` dispatch arm

- [ ] **Step 1: Read the current `handle_comments` body**

Run:
```bash
grep -n 'async fn handle_comments' src/cli/issue/list.rs
sed -n '$(grep -n "async fn handle_comments" src/cli/issue/list.rs | cut -d: -f1),+60p' src/cli/issue/list.rs
```

`handle_comments` is ~55 lines. Read the whole body.

- [ ] **Step 2: Identify imports `handle_comments` needs**

From the body, collect:
- `anyhow::Result`
- `crate::api::client::JiraClient`
- `crate::cli::OutputFormat`
- `crate::output`
- `crate::types::jira::issue::Comment`
- `super::format::{format_comment_date, format_comment_row, comment_visibility}`

- [ ] **Step 3: Create `src/cli/issue/comments.rs`**

Structure:

```rust
use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::OutputFormat;
use crate::output;
use crate::types::jira::issue::Comment;

use super::format::{format_comment_date, format_comment_row, comment_visibility};

pub(super) async fn handle_comments(
    key: &str,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    // paste handle_comments body verbatim from list.rs
}
```

Signature must match the dispatch call in `mod.rs` line ~73.

- [ ] **Step 4: Remove `handle_comments` from `list.rs`**

Use the Edit tool. `old_string` is the complete `pub(super) async fn handle_comments(...) -> Result<()> { ... }`. `new_string` is empty.

- [ ] **Step 5: Register `comments` module and rewire dispatch in `src/cli/issue/mod.rs`**

a) Add the module registration (alphabetical — between `mod changelog;` and `mod create;`):

```rust
mod comments;
```

b) Rewire the `Comments` arm:

Find:
```rust
IssueCommand::Comments { key, limit } => {
    list::handle_comments(&key, limit, output_format, client).await
}
```

Change to:
```rust
IssueCommand::Comments { key, limit } => {
    comments::handle_comments(&key, limit, output_format, client).await
}
```

- [ ] **Step 6: Verify compilation, tests, clippy, fmt**

Run:
```bash
cargo fmt --all
cargo check 2>&1 | tail -20
cargo test 2>&1 | tail -10
cargo clippy --all-targets -- -D warnings
```

Expected: all four clean.

**No commit yet.**

---

## Task 5: Clean up `list.rs`

**Files:**
- Modify: `src/cli/issue/list.rs` — remove any now-unused imports, verify final size

- [ ] **Step 1: Check for unused imports**

Run:
```bash
cargo clippy --all-targets -- -D warnings 2>&1 | grep -A2 "unused_imports\|unused_import"
```

If clippy already flagged unused imports in Tasks 2-4, delete them. Common candidates that `handle_list` does not need but the file used to import:
- items from `crate::api::assets::linked` if only `handle_view` used them
- `chrono::DateTime` if only a comment-rendering helper used it
- `super::format::format_comment_*` imports in `list.rs` (they're moved, but if you left the `use` there it'd be unused)

- [ ] **Step 2: Verify final file sizes match spec targets**

Run:
```bash
wc -l src/cli/issue/list.rs src/cli/issue/view.rs src/cli/issue/comments.rs src/cli/issue/format.rs src/cli/issue/mod.rs
```

Expected (approximate):
```
   ~700 src/cli/issue/list.rs      (target: ≤ 750)
   ~740 src/cli/issue/view.rs
    ~60 src/cli/issue/comments.rs
   ~215 src/cli/issue/format.rs
     ~95 src/cli/issue/mod.rs
```

If `list.rs` is significantly over 750, re-check whether `handle_view` or `handle_comments` was accidentally left in. If `view.rs` or `comments.rs` differs by more than ~10 lines from the target, re-check the move boundaries.

- [ ] **Step 3: Full CI-equivalent pre-commit check**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all green. If any fail, fix before Task 6 — do not commit a broken state.

- [ ] **Step 4: Sanity-check the dispatch**

Run a real CLI smoke (does not hit the network, just verifies the binary still links):
```bash
cargo run --quiet -- issue --help 2>&1 | head -30
cargo run --quiet -- issue view --help 2>&1 | head -15
cargo run --quiet -- issue comments --help 2>&1 | head -15
```

Expected: all three print their help text without panicking.

---

## Task 6: Atomic commit + push

**Files:** All staged changes from Tasks 2-5.

- [ ] **Step 1: Review the full diff**

Run:
```bash
git status
git diff --stat
```

Expected:
- Modified: `src/cli/issue/list.rs` (large negative line count)
- Modified: `src/cli/issue/format.rs` (small positive line count)
- Modified: `src/cli/issue/mod.rs` (small positive line count)
- New: `src/cli/issue/view.rs`
- New: `src/cli/issue/comments.rs`

No other files touched. If other files appear in the status, investigate — motion-only refactor must not have side effects elsewhere.

- [ ] **Step 2: Stage the five files**

Run:
```bash
git add src/cli/issue/list.rs src/cli/issue/format.rs src/cli/issue/mod.rs src/cli/issue/view.rs src/cli/issue/comments.rs
```

- [ ] **Step 3: Commit atomically**

Run:
```bash
git commit -m "$(cat <<'EOF'
refactor(cli): split issue/list.rs into list, view, comments, format

No behavior change. Motion only:
- src/cli/issue/view.rs (new) — handle_view + its imports
- src/cli/issue/comments.rs (new) — handle_comments + its imports
- src/cli/issue/format.rs — gains format_comment_date, format_comment_row,
  comment_visibility (+ their 5 inline tests), now shared by view and
  comments
- src/cli/issue/list.rs — reduced to handle_list + list-only helpers
  (build_jql_base_parts, build_filter_clauses, FilterOptions,
  resolve_show_points, extract_unique_status_names) + their 22 tests
- src/cli/issue/mod.rs — register view and comments modules, route
  View/Comments to them

Addresses the CLAUDE.md call-out that list.rs had grown to ~1500 lines
containing three unrelated handlers. Visibility stays at pub(super)
(validated: correct modifier for intra-module sibling access per Rust
API Guidelines). Atomic commit (validated: standard convention for
pure-motion refactors per rustc / Cargo / ripgrep contributor guides).
EOF
)"
```

Expected: commit succeeds, pre-commit hooks (if any) pass.

- [ ] **Step 4: Push and verify**

Run:
```bash
git push -u origin refactor/list-rs-split
git log --oneline -3
```

Expected: push succeeds, log shows the new commit on top of the previous develop tip.

---

## Post-implementation: PR + Review (handled outside this plan)

After Task 6, the work hands off to the orchestrator for:
1. `/pr-review-toolkit:review-pr` — iterate until clean
2. `superpowers:finishing-a-development-branch` — create PR
3. Copilot review cycle — iterate via `/zclaude:address-copilot-review` until zero new comments

No tasks in this plan cover those phases.

---

## Success Criteria Recap (from the spec) — with post-implementation outcomes

- [ ] ~~`src/cli/issue/list.rs` ≤ 750 lines~~ — **not met**: actual 1,083 lines. The spec's pre-flight estimate that `handle_view` was ~739 lines was wrong (it's ~268); `handle_list` is the real large handler. See the spec's Post-implementation note.
- [x] `src/cli/issue/view.rs` exists and owns `handle_view`
- [x] `src/cli/issue/comments.rs` exists and owns `handle_comments`
- [x] `src/cli/issue/format.rs` exports `format_comment_date`, `format_comment_row`, `comment_visibility` as `pub(super)`
- [x] `src/cli/issue/mod.rs` routes `View`/`Comments` to the new modules
- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all pass locally
- [x] Single atomic commit on branch `refactor/list-rs-split`
- [x] `jr issue --help`, `jr issue view --help`, `jr issue comments --help` all emit their usage text
