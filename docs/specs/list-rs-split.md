# Refactor: Split `cli/issue/list.rs` into focused files

## Goal

Reduce `src/cli/issue/list.rs` from 1,498 lines to ~700 by extracting the two unrelated handlers it currently hosts (`handle_view`, `handle_comments`) into sibling files, and relocating comment-formatting helpers to the existing shared `format.rs` module. No behavioral changes, no public API changes.

## Motivation

`CLAUDE.md` already flags `list.rs` as the largest file in the crate and explicitly warns that it "contains both `handle_list` and `handle_view` plus all JQL composition logic." That warning is a smell the audit confirmed: three distinct `pub(super) async fn` handlers plus ~12 private helpers live in one file because the file grew organically, not because the handlers are coupled.

Benefits of splitting:

- Each handler becomes reviewable as a self-contained unit.
- Cross-file navigation matches the existing `cli/issue/*` convention (`create.rs` for create+edit, `workflow.rs` for state-changing commands, `links.rs` for link/unlink, `changelog.rs` for changelog — one file per theme).
- Drops the largest file to below `changelog.rs` (847 lines) and `helpers.rs` (812 lines), aligning the whole `cli/issue/` directory to a similar size band.
- Reveals that `handle_view` (739 lines) is actually larger than `handle_list` (646 lines) — currently hidden by file co-location.

## Scope

- **In scope:** Pure code motion. Extract `handle_view` + its tests to `src/cli/issue/view.rs`. Extract `handle_comments` + its tests to `src/cli/issue/comments.rs`. Move three comment-formatting helpers (`format_comment_date`, `format_comment_row`, `comment_visibility`) and their tests to `src/cli/issue/format.rs`. Register new modules in `src/cli/issue/mod.rs` and rewire dispatch.
- **Out of scope:** Any logic changes. Renaming `list.rs`. Extracting JQL/filter helpers into a dedicated module (YAGNI — only `handle_list` uses them today; Perplexity-validated guidance is to wait for a second caller). Extracting `handle_list` itself. Splitting the test suite; existing tests travel with their subject function.

## Affinity Analysis

Before deciding file ownership, we mapped helper usage:

| Helper | Used by `handle_list` | Used by `handle_view` | Used by `handle_comments` |
|---|---|---|---|
| `extract_unique_status_names` | ✓ (line 209) | | |
| `build_jql_base_parts` | ✓ (line 273) | | |
| `build_filter_clauses` + `FilterOptions` | ✓ (line 257) | | |
| `resolve_show_points` | ✓ (line 387) | | |
| `format_comment_date` | | ✓ (lines 884, 893) | ✓ (line 682) |
| `format_comment_row` | | ✓ (line 744) | ✓ (line 730) |
| `comment_visibility` | | ✓ (line 728) | ✓ (line 713) |

The four list-only helpers stay in `list.rs`. The three comment helpers are shared between `handle_view` and `handle_comments` — they move to `format.rs` (which is already the shared formatting module — it already exports `format_issue_row`, `format_issue_rows_public`, `format_points`, `issue_table_headers`).

Cross-sibling imports (e.g., `view.rs` importing from `comments.rs`) are deliberately avoided. Per Rust community guidance validated with Perplexity, sibling-handler-imports-from-sibling-handler is a code smell; shared helpers belong in a shared module.

## File Structure (After)

```
src/cli/issue/
├── mod.rs          # adds `mod view;` `mod comments;`, routes two dispatch arms
├── assets.rs       # unchanged (65 lines)
├── changelog.rs    # unchanged (847 lines)
├── comments.rs     # NEW — handle_comments + its tests (~80 lines)
├── create.rs       # unchanged (375 lines)
├── format.rs       # gains 3 comment helpers + their tests (139 → ~210 lines)
├── helpers.rs      # unchanged (812 lines)
├── json_output.rs  # unchanged (149 lines)
├── links.rs        # unchanged (293 lines)
├── list.rs         # shrinks — handle_list + list-only helpers + tests (1498 → ~700 lines)
├── view.rs         # NEW — handle_view + its tests (~750 lines)
└── workflow.rs     # unchanged (786 lines)
```

## Helper Ownership (Mapping)

### Stay in `list.rs`

- `fn extract_unique_status_names(issue_types: &[IssueTypeWithStatuses]) -> Vec<String>` (currently line 24)
- `fn build_jql_base_parts(jql: &str, project_key: Option<&str>) -> (Vec<String>, &'static str)` (line 44)
- `struct FilterOptions<'a>` (line 602)
- `fn build_filter_clauses(opts: FilterOptions<'_>) -> Vec<String>` (line 617)
- `fn resolve_show_points(show_points: bool, sp_field_id: Option<&str>) -> Option<&str>` (line 583)
- All their colocated `#[cfg(test)] mod tests` entries (tests for each stay with the fn).

### Move to `format.rs`

- `fn format_comment_date(iso: &str, verbose: bool) -> String` (line 657)
- `fn format_comment_row(...)` (line 673 — 4 params: author, created, body, verbose)
- `fn comment_visibility(comment: &Comment) -> Option<&'static str>` (line 690)
- Their colocated tests: `format_comment_date_rfc3339`, `format_comment_row_missing_author`, etc.

Visibility: `pub(super)` in `format.rs` so both `view.rs` and `comments.rs` (its parent-module siblings) can call them. Per Perplexity-validated guidance, `pub(super)` is the correct modifier — `pub(crate)` would be too broad; `pub(in crate::cli::issue)` is equivalent but more verbose.

### Move to `view.rs`

- `pub(super) async fn handle_view(command, output_format, config, client) -> Result<()>` (line 759 to EOF of `handle_view`).
- No inline tests to move — `handle_view` has no `#[cfg(test)]` tests in the current file; its coverage is exercised through integration tests in `tests/`.

### Move to `comments.rs`

- `pub(super) async fn handle_comments(key, limit, output_format, client) -> Result<()>` (line 704).
- No inline tests to move — same situation as `handle_view`.

### Test inventory (all 30 inline tests in `list.rs`) — exact destinations

- Stays in `list.rs` (22 tests): `resolve_show_points_*` (×3), `build_jql_parts_*` (×15 — these exercise `FilterOptions` + `build_filter_clauses`), `build_jql_base_parts_*` (×6), `extract_unique_status_names_*` (×2). Each test stays with the fn it exercises.
- Moves to `format.rs` (5 tests): `format_comment_date_*` (×3), `format_comment_row_*` (×2).

Every test belongs in a `#[cfg(test)] mod tests { ... }` block at the bottom of its destination file, preserving the Rust convention of colocating unit tests with the code they test.

## Dispatch Changes (`cli/issue/mod.rs`)

Two lines change at the top (module registrations):

```rust
mod comments;  // NEW
mod view;      // NEW
```

Two dispatch arms change (function path only — the argument signatures stay identical):

```rust
// Before
IssueCommand::View { .. } => list::handle_view(command, output_format, config, client).await,
IssueCommand::Comments { key, limit } => list::handle_comments(&key, limit, output_format, client).await,

// After
IssueCommand::View { .. } => view::handle_view(command, output_format, config, client).await,
IssueCommand::Comments { key, limit } => comments::handle_comments(&key, limit, output_format, client).await,
```

No other file in `src/` changes. No `lib.rs` re-export change (these are `pub(super)`, never crossed the `cli::issue` boundary).

## Commit Strategy

**Single atomic commit.** Per Perplexity-validated rustc/Cargo/ripgrep contributor guidance, a pure-motion refactor that extracts multiple modules should land as one bisectable commit, not a chain of intermediate states. Intermediate commits would force each step to be independently green — possible but gains nothing for a cut/paste operation, and pollutes `git log` / `git blame`.

Commit message (Conventional Commits):

```
refactor(cli): split issue/list.rs into list, view, comments, format

No behavior change. Motion only:
- src/cli/issue/view.rs (new) — handle_view + tests
- src/cli/issue/comments.rs (new) — handle_comments + tests
- src/cli/issue/format.rs — gains format_comment_date, format_comment_row,
  comment_visibility (+ tests), now shared between view and comments
- src/cli/issue/list.rs — reduced to handle_list + list-only JQL/filter
  helpers (build_jql_base_parts, build_filter_clauses, FilterOptions,
  resolve_show_points, extract_unique_status_names) + their tests
- src/cli/issue/mod.rs — register new modules, route View/Comments to them

Validated with Perplexity: pub(super) is the correct visibility for intra-
module sibling access; atomic commit is preferred for pure-motion refactors
per rustc/Cargo/ripgrep contributor guidelines.
```

## Testing Strategy

- **No new tests.** This is code motion. Every existing test continues to exercise the same behavior through the same call path, it just lives in a different file.
- **Verification** (all three must pass before commit):
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- **Bisectability check:** once committed, `git bisect` between the parent and the refactor commit must not produce a broken state — the atomic-commit discipline guarantees this.

## Risk Assessment

| Risk | Probability | Mitigation |
|---|---|---|
| Missed import after cut/paste | Medium | Compiler catches it immediately; `cargo check` in the loop |
| Visibility gaffe (`pub(super)` → `pub(crate)` or missing entirely) | Low | Compiler catches unreachable callers; clippy flags overly-public items |
| Test that silently relied on same-file privacy breaks | Low | `cargo test` catches it; if it happens, it means the fn needs `pub(super)` visibility and the test is fine |
| `clippy::pedantic` flags new visibility items | Low | Default clippy config doesn't enable pedantic; CI uses plain `clippy -D warnings` |
| Behavior regression | Near-zero | Pure motion; no logic changes whatsoever |

## Success Criteria

- `src/cli/issue/list.rs` ≤ 750 lines (target: ~700).
- `src/cli/issue/view.rs` exists and owns `handle_view` + its tests.
- `src/cli/issue/comments.rs` exists and owns `handle_comments` + its tests.
- `src/cli/issue/format.rs` exports `format_comment_date`, `format_comment_row`, `comment_visibility` with `pub(super)` visibility.
- `src/cli/issue/mod.rs` routes `View`/`Comments` to the new modules.
- Full CI-equivalent check set passes locally (`fmt`, `clippy -D warnings`, `test`).
- PR CI stays green.
- No functional-level test changes; only file relocations within `cli/issue/`.

## Out-of-Scope Follow-ups (Deferred)

If future work shows a need, file separate issues for:

- Extracting JQL/filter helpers (`build_jql_base_parts`, `build_filter_clauses`, `FilterOptions`, `extract_unique_status_names`) into `cli/issue/filter.rs` — only worth doing once a second handler uses them.
- Splitting `handle_view` itself if it accumulates further responsibility (currently 739 lines of one cohesive async fn is within the accepted single-responsibility band per Rust community norms).
- Re-evaluating `helpers.rs` (812 lines) and `changelog.rs` (847 lines) under the same criteria.
