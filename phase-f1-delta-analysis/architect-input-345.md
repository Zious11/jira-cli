# F1 Architect Input — Issue #345

Issue: refactor(bulk): extract label-coalesce JSON builder into pure function with proptest coverage
Source: F6 hardening review of PR #110-pr2, 2026-05-10.
Date: 2026-05-15

---

## Impact Boundary

| Component | File | Change Type | Justification |
|-----------|------|-------------|---------------|
| Label bulk handler | `src/cli/issue/create.rs` | MODIFIED | Both the new pure function (`build_labels_edited_fields`) and the call-site change inside `handle_edit_bulk_labels` (lines 873-898) live here. The 26-line JSON-builder block is extracted; the async handler retains parsing, bail-check, and the `bulk_edit_fields` / `await_bulk_task` calls. Estimated net change: +~30 lines (new fn + proptest block) and -0 lines from handler (body replaced by a single call), so overall file grows from ~1,456 to ~1,490 lines. |
| Inline unit test block | `src/cli/issue/create.rs` (`mod tests` + sibling `mod proptests`) | MODIFIED | Proptest block added inline using the `mod proptests` pattern from `src/duration.rs`, `src/jql.rs`, and `src/partial_match.rs`. No new test file needed. The existing `mod tests` block (line 1085) is untouched; a separate `mod proptests` sub-module is appended at end-of-file following the codebase style. |
| Label integration tests | `tests/issue_bulk_pr2.rs` | NO CHANGE | All label tests (`test_label_add_remove_coalesce_emits_one_bulk_call`, single-ADD, single-REMOVE, `--dry-run` variants) use `body_string_contains` / `body_partial_json` matchers and `.expect(1)` wiremock guards. The refactor preserves byte-for-byte JSON output (the extracted fn returns the same `serde_json::Value`), so these tests remain untouched and continue to serve as the regression baseline. |
| Non-label bulk handler | `src/cli/issue/create.rs::handle_edit_bulk_fields` | NO CHANGE | This handler (line 934+) uses its own field-building logic for `--summary`, `--priority`, `--type`. Issue #345 extracts only the labels builder. `handle_edit_bulk_fields` is neither modified nor needs updating. |
| Workflow bulk handler | `src/cli/issue/workflow.rs` | NO CHANGE | `handle_move_bulk` calls `bulk_edit_fields` for transitions, not for label coalescing. Unaffected by this refactor. |
| Bulk API client | `src/api/jira/bulk.rs` | NO CHANGE | `bulk_edit_fields` and `await_bulk_task` signatures are unchanged; the extracted function constructs the `edited_fields` value that is passed to `bulk_edit_fields`, same as before. |
| BC spec anchors | `.factory/specs/prd/bc-3-issue-write.md` | NO CHANGE | Behavior is identical; no new BC is created. The coalesce-to-one-POST invariant is already pinned by PR2 `.expect(1)` guard. |
| CLAUDE.md | `CLAUDE.md` | NO CHANGE | No new `JR_*` env var. No change to documented behavior, output channels, or schema notes referencing this path. |

---

## Architecture Delta

This is a pure internal refactor with no structural change. The 26-line inline JSON-builder block inside the async `handle_edit_bulk_labels` function is lifted into a synchronous, side-effect-free module-level function `build_labels_edited_fields(&adds, &removes) -> serde_json::Value`. The handler replaces those 26 lines with a single call to the new function and passes the result directly to `bulk_edit_fields` as before. No new modules, no new traits or interfaces, no changes to the public API surface of `src/cli/issue/create.rs`, and no change to the async boundary or the caller chain from `handle_edit` down. The extracted function is private (no `pub`), lives in the same file, and is exercised by a new inline `mod proptests` block following the pattern already established in `src/duration.rs`, `src/jql.rs`, and `src/partial_match.rs`. The integration test surface is entirely unchanged — integration tests validate the CLI binary's observable behavior (JSON body shape, HTTP call count, exit code), and the refactor does not alter any of those observable properties.

---

## Regression Risk

| Module | Risk | Rationale |
|--------|------|-----------|
| `src/cli/issue/create.rs` — JSON shape | MEDIUM | The only regression vector is if the extracted function produces a different `serde_json::Value` than the inline block did. The object-vs-array branch (`label_ops.len() == 1` → object form; else array form) must be reproduced exactly. The PR2 `body_string_contains` matchers and `body_partial_json` matchers catch any divergence at the HTTP body level. Risk is bounded: if extraction is done by literal copy with no logic change, tests go green immediately. |
| `src/cli/issue/create.rs` — `.expect(1)` invariant | LOW | The single-POST coalesce invariant is enforced by the caller, not by the extracted function. `handle_edit_bulk_labels` calls `bulk_edit_fields` exactly once (line 902), which is unchanged by the refactor. There is no way to accidentally add a second call by extracting a pure JSON builder. |
| `tests/issue_bulk_pr2.rs` | LOW | Integration tests are unchanged. They exercise the binary end-to-end via subprocess. If the JSON shape is preserved byte-for-byte, all `.expect(1)` guards and `body_string_contains` matchers remain green. The only risk is a test-discovery issue if a new `#[tokio::test]` is accidentally added to the integration suite — not applicable here since the proptest lives inline in the library. |
| Proptest coverage itself | LOW | New property tests are purely additive. The `proptest!` macro in `src/cli/issue/create.rs` runs in `cargo test --lib`. A failing proptest will fail the test suite loudly; it cannot silently regress existing behavior. The proptest strategy (`vec("[a-z]{1,10}", 0..5)`) with `prop_assume!(!adds.is_empty() || !removes.is_empty())` covers the full space the production code exercises. |
| `src/api/jira/bulk.rs` | NONE | No changes. |
| `src/cli/issue/workflow.rs` | NONE | No changes. |
| Security surface | NONE | `build_labels_edited_fields` performs pure JSON construction from caller-supplied `Vec<String>` values that have already been parsed and validated upstream in the label-prefix loop. There is no I/O, no credential handling, and no external input deserialisation in the extracted function. No new attack surface is introduced. |

---

## Recommendation

**Scope: REFACTOR-ONLY**

Extract `build_labels_edited_fields` as a private pure function in `src/cli/issue/create.rs`; add a `mod proptests` block in the same file covering the three invariants specified in the issue body (top-level `"labels"` key always present; ADD/REMOVE entries appear iff respective input non-empty; the object-vs-array branch is determined by operand count). No integration test changes, no BC additions, no new files.

Rationale:

1. The behavioral contract (one bulk POST, coalesced ADD+REMOVE) is already pinned by PR2 integration tests. Adding more integration tests would duplicate coverage rather than add value.

2. The proptest inline pattern is already established in three sibling modules (`duration.rs`, `jql.rs`, `partial_match.rs`). A new file for a three-invariant property test is not warranted.

3. The `schema note` in the existing rustdoc (`best-guess pending #331`) should be preserved verbatim in the extracted function's doc comment so the schema-accuracy TODO is not silently lost.

4. The refactor does not touch `handle_edit_bulk_fields`, `workflow.rs`, the BC catalog, or CLAUDE.md. Total estimated diff: ~35 lines across one file (`src/cli/issue/create.rs`) — new pure function (~15 lines), call-site replacement (~1 line replacing 26), and proptest block (~20 lines).

**Trivial-scope eligible? YES**

Single-file change. No interface changes. No BC additions. No new env vars. No CLAUDE.md updates. Entire delta is contained inside `src/cli/issue/create.rs`: one new private function, one call-site substitution, one new `mod proptests` block. Qualifies as trivial under F1 criteria.
