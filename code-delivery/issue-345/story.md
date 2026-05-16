---
document_type: story
story_id: "S-345"
title: "Extract label-coalesce JSON builder into pure function with proptest coverage"
wave: feature-followup
status: completed
priority: low
estimated_effort: small
tdd_mode: strict
bc_anchors:
  - BC-3.4.006
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
files_modified:
  - src/cli/issue/create.rs (extract build_labels_edited_fields; call-site change in handle_edit_bulk_labels; add inline #[cfg(test)] mod proptests)
test_files:
  - src/cli/issue/create.rs (inline #[cfg(test)] mod proptests — proptest covering JSON shape invariants)
breaking_change: false
producer: story-writer
version: "1.1.0"
last_updated: 2026-05-16
depends_on:
  - S-110-pr2   # original bulk-edit feature delivery; S-345 is an audit-followup refactor of that work
blocks: []
issue: 345
---

# S-345: Extract label-coalesce JSON builder into pure function with proptest coverage

## Context

BC-3.4.006 was extended in-place during F2 spec evolution (issue #345, 2026-05-16) to
formally pin the two JSON wire shapes produced by `handle_edit_bulk_labels`:

- **Object-form** (single-action — only ADD, or only REMOVE):
  ```json
  {"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}}
  ```
- **Array-form** (both ADD and REMOVE — coalesced into one bulk POST):
  ```json
  {"labels": [
    {"labelsAction": "ADD",    "labels": [{"name": "foo"}]},
    {"labelsAction": "REMOVE", "labels": [{"name": "bar"}]}
  ]}
  ```

The existing `handle_edit_bulk_labels` in `src/cli/issue/create.rs` (lines 841–910) builds
this JSON inline at lines 873–898, mixing pure construction logic with the surrounding async
I/O context (`bulk_edit_fields`, `await_bulk_task`). This co-location of pure JSON
construction with async I/O makes unit-level property testing impossible without a wiremock
harness.

This story extracts the 26-line inline JSON-builder block into a named, private,
synchronous pure function `build_labels_edited_fields(adds: &[String], removes: &[String])
-> serde_json::Value`, then adds an inline `#[cfg(test)] mod proptests` block covering the
shape invariants. No behavioral change is introduced. The existing integration tests in
`tests/issue_bulk_pr2.rs` and `tests/issue_bulk.rs` remain the regression baseline and
MUST NOT be modified.

This is an audit-followup from the F6 hardening review of PR #110-pr2 (2026-05-10).

## Behavioral Contracts

| BC ID | Title | Role in this story |
|-------|-------|--------------------|
| BC-3.4.006 | `issue edit --label add:foo --label remove:bar` interprets prefix and emits correct JSON wire shape | The pure function extracted by this story is the production code that BC-3.4.006 pins. The proptest directly validates BC-3.4.006 invariants 1–5. |

## Goal

Extract `build_labels_edited_fields(adds: &[String], removes: &[String]) -> serde_json::Value`
as a private, synchronous, pure function in `src/cli/issue/create.rs`. Refactor
`handle_edit_bulk_labels` to call it at the point where the inline block currently lives
(lines 873–898). Add an inline `#[cfg(test)] mod proptests` (or `mod build_labels_proptests`)
block in the same file that uses the `proptest!` macro to cover the JSON shape invariants
defined in BC-3.4.006.

## Acceptance Criteria

**AC-001** (traces to BC-3.4.006 — pure function extraction).
New private synchronous function `build_labels_edited_fields(adds: &[String], removes: &[String])
-> serde_json::Value` exists in `src/cli/issue/create.rs`. It is pure: no I/O, no `async`,
no `&JiraClient`, no network access, no side effects.

**AC-002** (traces to BC-3.4.006 — call-site replacement).
`handle_edit_bulk_labels` calls `build_labels_edited_fields(&adds, &removes)` to construct
the `edited_fields` value, replacing the inline `label_ops` building block (current lines
873–898). The result is passed to `bulk_edit_fields` exactly as before. The handler retains
its parsing loop, bail-on-empty check, and the `bulk_edit_fields` / `await_bulk_task` calls
unchanged.

**AC-003** (traces to BC-3.4.006 invariant 1–5 — byte-for-byte preservation).
Behavior is preserved byte-for-byte: all existing tests pass without modification:
- `tests/issue_bulk_pr2.rs::test_label_add_remove_coalesce_emits_one_bulk_call` (`.expect(1)` coalesce guard)
- `tests/issue_bulk_pr2.rs` — all dry-run label variants and rejection tests
- `tests/issue_bulk.rs::test_edit_label_remove_sends_remove_action_in_bulk_payload`
- `tests/issue_bulk.rs::test_edit_multi_key_issues_one_bulk_post_then_polls_to_complete`

No test in `tests/issue_bulk_pr2.rs` or `tests/issue_bulk.rs` is modified.

**AC-004** (traces to BC-3.4.006 invariants 1–5 — proptest coverage).
Inline `#[cfg(test)] mod proptests` (or `mod build_labels_proptests`) added to
`src/cli/issue/create.rs` at end of file. Contains a `proptest!` block with test
`build_labels_edited_fields_invariants` covering:

1. Top-level `"labels"` key is always present in the returned `serde_json::Value`
   (BC-3.4.006 invariant — labels key always present).
2. ADD entry (`"labelsAction": "ADD"`) appears in the output if and only if `adds`
   is non-empty (BC-3.4.006 invariant 1).
3. REMOVE entry (`"labelsAction": "REMOVE"`) appears in the output if and only if
   `removes` is non-empty (BC-3.4.006 invariant 2).
4. Single-action path (only ADD or only REMOVE) → object-form: `result["labels"]`
   is a JSON object (not array) with a `"labelsAction"` key (BC-3.4.006 invariant 5).
5. Both-action path (ADD + REMOVE both non-empty) → array-form: `result["labels"]`
   is a JSON array of length 2, with ADD entry at index 0 and REMOVE entry at index 1
   (BC-3.4.006 invariant 4).
6. `prop_assume!(!adds.is_empty() || !removes.is_empty())` guards the strategy so the
   proptest does not exercise the bail-on-empty path (which is enforced by the caller,
   not by the extracted function — BC-3.4.006 invariant 3).

**AC-005** (traces to BC-3.4.006 — release gate).
`cargo test`, `cargo fmt --all -- --check`, and `cargo clippy --all-targets -- -D warnings`
all pass after the changes.

## Out of Scope

- Schema correctness of the array-form (`[{labelsAction: "ADD", ...}, ...]`) against
  Atlassian's formally documented bulk API schema is tracked in issue #331 and is
  explicitly deferred. This story pins the **current** shape, not the canonical
  Atlassian shape. The `Schema note` rustdoc comment in the inline block (lines 869–872)
  MUST be preserved verbatim in the extracted function's doc comment.
- No behavioral change to `handle_edit_bulk_labels` (parsing loop, bail-on-empty,
  `bulk_edit_fields` call, `await_bulk_task` call, `render_bulk_edit_results` call).
- No changes to `handle_edit_bulk_fields`, `handle_move_bulk`, `workflow.rs`, or
  `src/api/jira/bulk.rs`.
- No changes to `CLAUDE.md` (no new `JR_*` env var; no documented behavior change).
- No changes to `tests/issue_bulk_pr2.rs` or `tests/issue_bulk.rs`.

## Implementation Notes

**Extraction site:**

Lines 873–898 of `src/cli/issue/create.rs` (inside `handle_edit_bulk_labels`) contain
the inline JSON-builder block:

```rust
let mut label_ops: Vec<serde_json::Value> = Vec::new();
if !adds.is_empty() {
    let add_entries: Vec<serde_json::Value> = adds.iter().map(|n| json!({"name": n})).collect();
    label_ops.push(json!({
        "labelsAction": "ADD",
        "labels": add_entries
    }));
}
if !removes.is_empty() {
    let remove_entries: Vec<serde_json::Value> =
        removes.iter().map(|n| json!({"name": n})).collect();
    label_ops.push(json!({
        "labelsAction": "REMOVE",
        "labels": remove_entries
    }));
}
// When only one action is present, unwrap to the simpler object form
// for backward compatibility with PR1 tests (body_partial_json matchers).
let edited_fields = if label_ops.len() == 1 {
    let op = label_ops.remove(0);
    json!({ "labels": op })
} else {
    // Both ADD and REMOVE: use the coalesced array form.
    json!({ "labels": label_ops })
};
```

This block becomes the body of `build_labels_edited_fields`. The handler replaces
these 26 lines with one call:

```rust
let edited_fields = build_labels_edited_fields(&adds, &removes);
```

**Two output shapes to preserve:**

- Single-action (object-form): `{"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}}`
- Dual-action (array-form): `{"labels": [{"labelsAction": "ADD", ...}, {"labelsAction": "REMOVE", ...}]}`

**`.expect(1)` single-POST coalesce invariant:**

The single-POST coalesce invariant is enforced by `handle_edit_bulk_labels` (which calls
`bulk_edit_fields` exactly once, unchanged). The extracted function is purely a JSON
constructor — it is called once per `handle_edit_bulk_labels` invocation, preserving the
single-POST invariant. Extracting the pure function cannot accidentally add a second
`bulk_edit_fields` call.

**Proptest style precedent:**

Follow the pattern established in:
- `src/duration.rs` — `mod proptests` with `proptest!` macro, `prop_assume!` guard
- `src/jql.rs` — inline proptest block at end of file
- `src/partial_match.rs` — inline proptest at end of file

Strategy suggestion: `vec("[a-z]{1,10}", 0..5)` for both `adds` and `removes`, with
`prop_assume!(!adds.is_empty() || !removes.is_empty())` to exclude the bail-on-empty path.

**Schema note preservation:**

The extracted function's doc comment MUST include the same schema caveat currently
in lines 869–872 of `handle_edit_bulk_labels`:

```rust
/// Shape is best-guess (unverified against live Atlassian API; tracked at #331).
/// PR2 test asserts .expect(1) on bulk POST to ensure ADD+REMOVE coalesce into ONE call,
/// but the exact JSON nesting matches a loose `body_string_contains` matcher — schema
/// accuracy is the work being deferred to #331.
```

## TDD Plan

**Step 1 — Red Gate (write failing proptest).**

Add `#[cfg(test)] mod proptests` at the end of `src/cli/issue/create.rs` with a
`proptest!` block referencing `build_labels_edited_fields`. Since the function does
not yet exist as a named symbol, `cargo test --lib` MUST fail to compile. This is the
Red Gate — a compile error counts as a red test. Confirm failure:

```
cargo test --lib 2>&1 | grep "cannot find function"
# Expected: error[E0425]: cannot find function `build_labels_edited_fields`
```

**Step 2 — Green (implement the function).**

Extract the inline block from lines 873–898 into `build_labels_edited_fields`. Do so
by literal copy (no logic change). The function signature:

```rust
fn build_labels_edited_fields(adds: &[String], removes: &[String]) -> serde_json::Value {
    // ... literal copy of lines 873-898 logic ...
}
```

Replace the inline block in `handle_edit_bulk_labels` with:

```rust
let edited_fields = build_labels_edited_fields(&adds, &removes);
```

Confirm proptest passes: `cargo test --lib`. The proptest MUST go green.

**Step 3 — Regression check.**

Run the full test suite including integration tests:

```
cargo test
```

All existing tests in `tests/issue_bulk_pr2.rs` and `tests/issue_bulk.rs` MUST pass
without modification. If any fail, the extraction diverged from the original inline
logic — revisit the copy.

**Step 4 — Full local check set.**

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All MUST pass. This satisfies AC-005.

## Token Budget Estimate

| Component | Estimated tokens |
|-----------|-----------------|
| Story spec (this file) | ~3,000 |
| `src/cli/issue/create.rs` (read lines 841–910 for extraction site) | ~800 |
| `src/cli/issue/create.rs` (read proptest mod location at end of file) | ~400 |
| `src/duration.rs` (proptest style precedent — skim proptest block) | ~300 |
| BC-3.4.006 spec text (read from bc-3-issue-write.md) | ~400 |
| `cargo test --lib` + `cargo test` output | ~400 |
| **Total** | **~5,300** |

Well within the 20–30% agent context window budget for a small story.
Net diff is approximately 35 lines across one file (new function ~15 lines, call-site
replacement ~1 line replacing 26, proptest block ~20 lines).

## Previous Story Intelligence

**S-110-pr2** (parent, delivered 2026-05-10) delivered `handle_edit_bulk_labels` with the
inline JSON-builder block (lines 873–898) and the `.expect(1)` coalesce guard in
`tests/issue_bulk_pr2.rs::test_label_add_remove_coalesce_emits_one_bulk_call`. That test
uses `body_string_contains` loose matchers — it validates HTTP call count and key substrings
in the body, not the exact JSON shape. The proptest in this story is the first artifact
to assert the exact output shapes deterministically.

**S-333** (completed, PR #360) wired `await_bulk_task` deadline propagation. This story
is orthogonal to S-333 and has no dependency on it beyond both touching `create.rs` at
different function locations.

**S-340** (completed, PR #370) added a task_id assertion to `tests/bulk_deadline_propagation.rs`.
No overlap with S-345.

No previous feature-followup story has touched `handle_edit_bulk_labels` or the label
JSON-builder path. The proptest pattern to follow is `src/duration.rs` (most recent and
cleanest example in the codebase).

## Architecture Compliance Rules

(Extracted from architect-input-345.md and existing codebase conventions.)

1. **Private function**: The new function MUST be `fn build_labels_edited_fields(...)`,
   NOT `pub fn`. It is an implementation detail of `handle_edit_bulk_labels` and must
   not be exposed outside the module.
2. **Synchronous**: The new function MUST NOT be `async`. It has no I/O; `async` would
   add overhead and prevent direct proptest invocation without a tokio runtime.
3. **Slice parameters**: The new function MUST take `&[String]` (not `Vec<String>`) for
   both parameters, per Rust slice idioms (`fn f(v: &[T])` is preferred over `fn f(v: &Vec<T>)`
   for read-only access).
4. **No I/O dependency**: The new function MUST NOT take `&JiraClient`, `&Config`, or any
   other I/O dependency. It is a pure value transformation.
5. **Inline proptest**: The `mod proptests` block MUST be inline in
   `src/cli/issue/create.rs` under `#[cfg(test)]`, NOT in a separate file under `tests/`.
   This follows the pattern in `src/duration.rs`, `src/jql.rs`, and `src/partial_match.rs`.
6. **No new files**: The entire delta is in `src/cli/issue/create.rs`. No new source
   files, no new test files.
7. **No Cargo.toml changes**: `proptest = "1"` is already in `[dev-dependencies]`.
   Do not add or change any dependency.
8. **No suppression**: Do not add `#[allow(...)]` attributes to work around clippy
   warnings. If clippy warns on anything in the new code, fix the root cause per
   the project's zero-suppression policy.

## Library & Framework Requirements

Same as `src/cli/issue/create.rs` at HEAD — no version changes:

| Dependency | Current version (Cargo.toml) | Notes |
|------------|------------------------------|-------|
| `serde_json` | (workspace pin) | Already used in `handle_edit_bulk_labels`; `json!` macro available |
| `proptest` | `"1"` (dev-dependency) | Already present; no version change |
| `tokio` | (workspace pin) | NOT required in the extracted function or proptest (synchronous) |

Do not add any new Cargo dependencies.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/cli/issue/create.rs` | MODIFY | (1) Add `fn build_labels_edited_fields(adds: &[String], removes: &[String]) -> serde_json::Value` as a private synchronous function near (but outside of) `handle_edit_bulk_labels`; (2) replace inline lines 873–898 in `handle_edit_bulk_labels` with a single call; (3) append `#[cfg(test)] mod proptests { ... }` at end of file |
| `tests/issue_bulk_pr2.rs` | DO NOT TOUCH | Regression baseline — must not be modified |
| `tests/issue_bulk.rs` | DO NOT TOUCH | Regression baseline — must not be modified |
| `Cargo.toml` | DO NOT TOUCH | No dependency changes required |
| `CLAUDE.md` | DO NOT TOUCH | No new `JR_*` env var; no behavioral change |
| `.factory/specs/prd/bc-3-issue-write.md` | DO NOT TOUCH | F2 already updated BC-3.4.006 in-place (product-owner owns F2) |

No new files are created. The entire delta is one file: `src/cli/issue/create.rs`.

## References

- Issue #345: `refactor(bulk): extract label-coalesce JSON builder into pure function with proptest coverage`
- BC-3.4.006 (extended in F2): `.factory/specs/prd/bc-3-issue-write.md`
- S-110-pr2 (parent delivery): `.factory/code-delivery/issue-110-pr2/`
- F1 delta analysis: `.factory/phase-f1-delta-analysis/delta-analysis-345.md`
- F1 architect input: `.factory/phase-f1-delta-analysis/architect-input-345.md`
- F1 BA input: `.factory/phase-f1-delta-analysis/business-analyst-input-345.md`
- Proptest style precedent: `src/duration.rs`, `src/jql.rs`, `src/partial_match.rs`
- Schema-correctness deferred: issue #331
