---
document_type: story
story_id: "S-388"
title: "Cross-hierarchy edit --type 400 enrichment + fix fake-endpoint hint (closes #388)"
wave: feature-followup
status: ready
intent: enhancement
feature_type: backend
scope: standard
issue: 388
points: 5
priority: medium
tdd_mode: strict
estimated_effort: medium
depends_on: []  # builds on develop HEAD; no dependency on open stories
bc_anchors:
  - BC-3.4.010
  - BC-3.4.011
  - BC-3.4.003  # annotation-only cross-reference; no behavioral implementation required
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f2-spec-evolution/prd-delta-388.md"
implementation_strategy: tdd
module_criticality: HIGH  # src/cli/issue/create.rs — core edit command error path; shared with all issue-write BCs
files_modified:
  - src/types/jira/issue.rs        # MODIFIED — add subtask: Option<bool> with #[serde(default)] to IssueType struct
  - src/cli/issue/create.rs        # MODIFIED — add CROSS_HIERARCHY_HINT const; add Classification enum; add is_cross_hierarchy_type_error pure helper; add CrossHierarchy/SameCategory/Indeterminate dispatch in handle_edit's 400 error block; replace fake-endpoint hint at ~line 834 (--no-parent path); add is_cross_hierarchy_type_error_proptests inline proptest module
  - tests/issue_edit_no_parent.rs  # MODIFIED — strengthen T-06: add JRACLOUD-27893 literal-pin + negative pin on jr api /rest/api/3/issue + full context-sentence pin
  - tests/common/fixtures.rs       # MODIFIED — add subtask: None to every IssueType struct-literal construction
test_files:
  - tests/issue_edit_type_errors.rs  # NEW — 10 integration tests for all edit --type 400 enrichment paths
breaking_change: false
# BC status: BC-3.4.010 and BC-3.4.011 produced in F2 (2026-05-20, 7 adversarial passes, CONVERGED).
# BC-3.4.003 annotation-only update sealed in F2. All BCs sealed; do NOT re-edit unless adversary
# finds implementation-BC discrepancy.
# F3 story produced after F2 convergence confirmed complete (guard scripts all exit 0).
---

# S-388 — Cross-Hierarchy Edit --type 400 Enrichment + Fix Fake-Endpoint Hint

## Source of Truth

F1 delta analysis: `.factory/phase-f1-delta-analysis/issue-388/delta-analysis.md` (approved).
F2 PRD delta: `.factory/phase-f2-spec-evolution/prd-delta-388.md` (CONVERGED, 7 adversarial passes, 2026-05-20).
F2 verification delta: `.factory/phase-f2-spec-evolution/verification-delta-388.md` (CONVERGED, 2026-05-20).
BC bodies: `.factory/specs/prd/bc-3-issue-write.md` §BC-3.4.010 and §BC-3.4.011.

**Authoritative test count: TEN (10) integration tests** for `tests/issue_edit_type_errors.rs`. The
F1 delta analysis figure of five (5) is superseded by the F2 delta. Tests #6–#7 added in F2 Pass-3
(finding M-2); test #8 in F2 Pass-6 (finding MAJOR-3); tests #9–#10 in F2 Pass-7 (findings O-1/O-2).
The F4 implementer MUST use ten as the authoritative count. Do not revert to eight or five.

## Problem Statement

`jr issue edit KEY --type X` that crosses the standard ↔ sub-task hierarchy boundary returns HTTP 400
with no actionable guidance. The Jira Cloud REST API does not support cross-hierarchy type conversion
(JRACLOUD-27893, open since 2012, unresolved). The current error gives the user nothing useful — they
cannot distinguish a cross-hierarchy rejection from a plain type-name typo.

A bundled bug is present at `src/cli/issue/create.rs:834`: the `--no-parent` subtask-error hint tells
users to run `jr api /rest/api/3/issue/{key}/convert -X put -d ...`, referencing a non-existent Jira
Cloud endpoint (confirmed REFUTED by F1 research Q2). Both the hint bug and the new enrichment logic
share the same `CROSS_HIERARCHY_HINT` constant; they must ship atomically.

## Behavioral Contracts

| BC ID | File | Title | Clause(s) |
|-------|------|-------|-----------|
| BC-3.4.010 | `bc-3-issue-write.md` | `edit --type` HTTP 400 + cross-hierarchy subtask-flag mismatch → exit 1, `CROSS_HIERARCHY_HINT` on stderr (JRACLOUD-27893) | preconditions 1-5, postconditions 1-4, invariants 1-3 |
| BC-3.4.011 | `bc-3-issue-write.md` | `edit --type` HTTP 400 + same-hierarchy OR indeterminate → exit 1, typo hint or raw error (no JRACLOUD-27893) | preconditions, all sub-path postconditions, invariants 1-3 |
| BC-3.4.003 | `bc-3-issue-write.md` | `issue edit` success path — Errors cross-reference added for BC-3.4.010/011 | annotation-only; no new implementation |

### Verbatim Pinned Strings (BC-authoritative — copy byte-for-byte)

**`CROSS_HIERARCHY_HINT` constant** (shared constant name; co-located with `is_subtask_parent_error` in `src/cli/issue/create.rs`):

```
The Jira Cloud REST API does not support changing the standard / sub-task hierarchy level via this endpoint (see JRACLOUD-27893). To convert it, open the issue in the Jira web UI and use the action menu to find the Convert option.
```

**`--no-parent` context sentence** (prepended BEFORE `CROSS_HIERARCHY_HINT` on the `--no-parent` path only; NOT on the `edit --type` path):

```
Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue.
```

**Typo hint** (emitted on SameCategory and unresolvable-name sub-paths):

```
Jira rejected the type change. If the type name is wrong, run `jr project types` to list valid types; the change may also be blocked by workflow or scheme constraints.
```

## Acceptance Criteria

### AC-1 — `IssueType` struct gains `subtask: Option<bool>` field
(traces to BC-3.4.010 precondition: classifier receives `src_subtask` from `issue.fields.issuetype.as_ref().and_then(|t| t.subtask)`)

`src/types/jira/issue.rs` `IssueType` struct MUST gain `subtask: Option<bool>` with `#[serde(default)]`.
This is an additive field; backward-compatible — any existing API response without the `subtask` key
deserializes to `subtask: None`.

All struct-literal `IssueType { name: "..." }` constructions in `tests/common/fixtures.rs` MUST have
`subtask: None` added to them. Verify by searching for `IssueType {` in that file and updating every
occurrence.

**Test:** verified implicitly by all integration tests that deserialize GET-issue responses with and
without the `subtask` field.

### AC-2 — Pure `is_cross_hierarchy_type_error` classifier with `Classification` enum
(traces to BC-3.4.010 invariant 1: subtask-flag mismatch is the primary, locale-independent classifier)

Add the `Classification` enum (deriving `PartialEq + Debug` for `prop_assert_eq!`) and the pure
function `is_cross_hierarchy_type_error` in `src/cli/issue/create.rs`, co-located near
`is_subtask_parent_error`:

```
enum Classification { CrossHierarchy, SameCategory, Indeterminate }

fn is_cross_hierarchy_type_error(
    src_subtask: Option<bool>,
    tgt_subtask: Option<bool>,
    err: &str,
) -> Classification
```

Classification rules (pure, locale-independent):
- Both `Some(a)` and `Some(b)` with `a != b` → `CrossHierarchy`
- Both `Some(a)` and `Some(b)` with `a == b` → `SameCategory`
- Either argument `None` → `Indeterminate`

The `err: &str` argument MUST NOT influence the return value (P4 property — pins the architectural
constraint from research addendum A1/A2; prevents false-positive on English substrings). The parameter
exists for potential future hint composition only.

**Test:** inline proptest module `#[cfg(test)] mod is_cross_hierarchy_type_error_proptests` (AC-7).

### AC-3 — `CROSS_HIERARCHY_HINT` constant added and `--no-parent` fake-endpoint hint replaced
(traces to BC-3.4.010 invariant 2: shared constant used at both call sites; BC-3.4.010 postcondition: stderr does NOT contain `jr api /rest/api/3/issue`)

Add `CROSS_HIERARCHY_HINT: &str` as a named constant in `src/cli/issue/create.rs` with the verbatim
text pinned in BC-3.4.010 (quoted in "Verbatim Pinned Strings" above).

At `src/cli/issue/create.rs` ~line 834, REPLACE the ENTIRE prior `--no-parent` hint block:
- The old block is a multi-line `format!` spanning approximately lines 830-836 containing FOUR sentences
  (fake endpoint reference, two framing sentences, and a parenthetical). NONE of these four old
  sentences are retained.
- The new block is EXACTLY: `eprintln!("{}", <context sentence>)` followed by
  `eprintln!("{}", CROSS_HIERARCHY_HINT)` — and nothing else.
- The context sentence is the pinned string "Sub-tasks are structurally bound to a parent; clearing it
  requires converting the sub-task to a standard issue." — followed immediately by `CROSS_HIERARCHY_HINT`.

On the `edit --type` error path, `CROSS_HIERARCHY_HINT` is emitted directly with NO prepended sentence.

**Test:** T-06 strengthening in `tests/issue_edit_no_parent.rs` (AC-4); test #5 in
`tests/issue_edit_type_errors.rs` (AC-5 regression pin).

### AC-4 — T-06 literal-pin strengthened in `tests/issue_edit_no_parent.rs`
(traces to BC-3.4.010 `--no-parent` path postcondition M-1)

`test_subtask_parent_clear_surfaces_400_with_convert_hint` MUST be strengthened with three new assertions
(in addition to any existing checks — do NOT remove existing assertions):

1. `assert!(stderr.contains("JRACLOUD-27893"))` — literal-pin on the citation.
2. `assert!(!stderr.contains("jr api /rest/api/3/issue"))` — negative regression guard on the removed
   fake-endpoint hint (the exact prior text was `jr api /rest/api/3/issue/{key}/convert -X put -d ...`;
   the pin substring `jr api /rest/api/3/issue` uniquely identifies the removed fake hint without
   over-matching legitimate content).
3. `assert!(stderr.contains("Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue."))` — full verbatim context sentence pin (PINNED STRING — exact bytes required; do not paraphrase).

### AC-5 — `handle_edit` 400 error block: CrossHierarchy/SameCategory/Indeterminate dispatch
(traces to BC-3.4.010 precondition: HTTP-400 gate via downcast to `JrError::ApiError { status: 400, .. }`; call-ordering precondition; BC-3.4.011 all sub-path postconditions)

When `edit_issue`'s `anyhow::Error` downcasts to `JrError::ApiError { status: 400, .. }` AND `--type`
is set, `handle_edit`'s `if let Err(ref e) = edit_result` block MUST:

**Step A — HTTP-400 gate:** Downcast to `JrError::ApiError { status: 400, .. }`. If the error is
NOT a 400 (401, 403, 5xx, network error, etc.) — the R0b routing row — NO enrichment occurs and the
raw error is propagated unchanged.

**Step B — `--type` arm evaluated FIRST (dual-gate precedence):** When both `--type` and `--no-parent`
are set and `edit_issue` returns 400, the `--type` arm (this AC) is evaluated FIRST. Only if it does
NOT emit a hint (SameCategory or Indeterminate without hint emission) does the `--no-parent` arm
(`no_parent && is_subtask_parent_error`) evaluate. First-hint-wins.

**Step C — Call ordering:**
1. Call `get_issue(key, &[])` (using empty extra_fields; `issuetype` is already in `BASE_ISSUE_FIELDS`).
   On `Err` → Indeterminate immediately (`get_project_issue_types` is NOT called). Detection gate:
   `Result::is_err()` — ANY `Err` variant (not a downcast); this covers `JrError::NotAuthenticated`
   (401), `JrError::InsufficientScope` (403), `ApiError { 5xx }`, and all other variants.
2. Extract `src_subtask` via `issue.fields.issue_type.as_ref().and_then(|t| t.subtask)` (NOTE for
   F4 implementer: the Rust field on `IssueFields` is `issue_type` with `#[serde(rename="issuetype")]`,
   NOT `issuetype` — see F4 implementer notes).
3. Call `get_project_issue_types(project_key)` (project_key from `issue.fields.project.key`).
   On `Err` → Indeterminate. Detection gate: `Result::is_err()` (same rationale as above).
4. Look up the user-supplied `--type` name in the returned list using **case-insensitive exact match on
   the `name` field** (not substring, not `id`-based). If the name is NOT found → typo hint (the
   unresolvable-name sub-path; the classifier is NOT invoked on this path).
5. If the name IS found: extract `tgt_subtask` from the matched type's `subtask` field.
6. Call `is_cross_hierarchy_type_error(src_subtask, tgt_subtask, err)` where `err` is the 400 message.

**Step D — Dispatch per classification:**
- `CrossHierarchy` → `eprintln!("{}", CROSS_HIERARCHY_HINT)`, exit 1.
- `SameCategory` → emit typo hint (verbatim above) + `extract_error_message`-processed 400 message text
  from `JrError::ApiError.message`, exit 1.
- Indeterminate (from either `is_err()` fetch gate OR from `None` subtask field) → surface the
  `extract_error_message`-processed 400 message text from `JrError::ApiError.message`; NO enrichment
  hint of any kind, exit 1.
- Unresolvable-name (typo path; name absent from 200 response) → same output as SameCategory.

**Important note on `get_project_issue_types` deserialization:** `get_project_issue_types` at
`src/api/jira/projects.rs:47-51` uses `.and_then(|v| from_value::<Vec<IssueTypeMetadata>>(v).ok()).unwrap_or_default()`.
A 200 response with a malformed body or absent `issueTypes` key returns `Ok(vec![])`, NOT `Err`. A
200 with unparseable body → empty list → name not found → typo hint (NOT Indeterminate). Only an
actual HTTP/network error on the underlying GET returns `Err`.

### AC-6 — Ten integration tests in `tests/issue_edit_type_errors.rs` (new file)
(traces to BC-3.4.010 and BC-3.4.011 test sub-path mappings)

A new file `tests/issue_edit_type_errors.rs` with the following ten named test functions (all mandatory):

| # | Test function name | BC path | Key assertions |
|---|-------------------|---------|----------------|
| 1 | `test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint` | BC-3.4.010 CrossHierarchy std→subtask | GET issue `subtask: Some(false)`, GET types target `subtask: Some(true)`, PUT 400 → exit 1; stderr `contains("JRACLOUD-27893")`; stderr `!contains("jr api /rest/api/3/issue")` |
| 2 | `test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint` | BC-3.4.010 CrossHierarchy subtask→std | Reverse of #1 (`subtask: Some(true)` → `Some(false)`); same assertions |
| 3 | `test_edit_type_same_hierarchy_400_surfaces_typo_hint` | BC-3.4.011 SameCategory | Both flags `subtask: Some(false)`, target name found; PUT 400 → exit 1; stderr `contains("jr project types")`; stderr `contains(<extracted-400-msg-substring>)` (plain-ASCII only, not raw JSON envelope); stderr `!contains("JRACLOUD-27893")`; stderr `!contains("jr api /rest/api/3/issue")` |
| 4 | `test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error` | BC-3.4.011 Indeterminate Cause-1 R2 | GET issue succeeds (`subtask: Some(false)`), GET project types → 5xx; PUT 400 → exit 1; stderr `contains(<extracted-400-msg-substring>)`; stderr `!contains("JRACLOUD-27893")`; stderr `!contains("jr api /rest/api/3/issue")` |
| 5 | `test_edit_type_cross_hierarchy_hint_no_fake_endpoint_literal` | BC-3.4.010 regression pin | CrossHierarchy 400 path → stderr `!contains("jr api /rest/api/3/issue")` (literal pin on removed fake hint) |
| 6 | `test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error` | BC-3.4.011 Indeterminate Cause-2 source-side (EC-3.4.011-5) | GET issue returns 200 with `issuetype` object present but `subtask` key OMITTED; `subtask: None` after deser; PUT 400 → exit 1; stderr `contains(<extracted-400-msg-substring>)`; no hint; `!contains("JRACLOUD-27893")`; `!contains("jr api /rest/api/3/issue")` |
| 7 | `test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error` | BC-3.4.011 Indeterminate Cause-2 target-side (EC-3.4.011-6) | GET issue returns 200 with source `subtask: Some(false)`; GET project types returns 200 with target type's `subtask` key OMITTED; `tgt_subtask: None`; PUT 400 → exit 1; same assertions as #6 |
| 8 | `test_edit_type_unresolved_type_name_surfaces_typo_hint` | BC-3.4.011 unresolvable-name sub-path (EC-3.4.011-3/7) | GET issue returns 200 with `subtask: Some(false)`; GET project types returns 200 with list that does NOT contain `--type` value; PUT 400 → exit 1; stderr `contains("jr project types")`; `!contains("JRACLOUD-27893")`; `!contains("jr api /rest/api/3/issue")`. Classifier is NOT invoked. |
| 9 | `test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error` | BC-3.4.011 Indeterminate Cause-1 R1 (EC-3.4.011-4) | PUT 400; GET issue → 5xx or 503 → `is_err()` → Indeterminate immediately; `get_project_issue_types` NOT called (no mock mounted); exit nonzero; raw error on stderr; no hint; `!contains("JRACLOUD-27893")`; `!contains("jr api /rest/api/3/issue")`. Distinct from test #4 (R2). |
| 10 | `test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment` | BC-3.4.010/011 R0b routing row | PUT → 403 (non-400); exit nonzero; raw error on stderr; no hint; `!contains("JRACLOUD-27893")`; `!contains("jr api /rest/api/3/issue")`. No enrichment fetch (`get_issue` and `get_project_issue_types` mocks NOT mounted — use `expect(0)` pattern). |

**Wiremock topology notes:**
- Tests #1, #2, #3, #5 mount mocks for: PUT `/rest/api/3/issue/{key}` → 400; GET `/rest/api/3/issue/{key}` → 200 with full `issuetype` object including `subtask`; GET `/rest/api/3/project/{key}` → 200 with type list.
- Test #4 mounts: PUT → 400; GET issue → 200 with `subtask: Some(false)`; GET project-types → 5xx.
- Test #6 mounts: PUT → 400; GET issue → 200 with `issuetype: {"name":"Task"}` (no `subtask` key); GET project-types mock OPTIONAL (may not be reached if Indeterminate fires early).
- Test #7 mounts: PUT → 400; GET issue → 200 with `subtask: Some(false)`; GET project-types → 200 with target type's `subtask` key absent.
- Test #8 mounts: PUT → 400; GET issue → 200 with `subtask: Some(false)`; GET project-types → 200 with list NOT containing `--type` value.
- Test #9 mounts: PUT → 400; GET issue → 5xx. Do NOT mount GET project-types mock.
- Test #10 mounts: PUT → 403 only. Do NOT mount GET issue or GET project-types mocks.

**Test substrings from `extract_error_message`-processed 400 body:** Use plain-ASCII substrings only.
`extract_error_message` = `sanitize_for_stderr(extract_error_message_raw(body))`; `sanitize_for_stderr`
is a no-op for plain-ASCII content. Choose substrings from the extracted message (e.g., `The issue type
selected is invalid` survives extraction). Do NOT assert raw JSON envelope keys such as `{"errors"` or
`"issuetype":` (these do not survive extraction). Do NOT assert control characters or multibyte sequences.

### AC-7 — Inline proptest `mod is_cross_hierarchy_type_error_proptests` in `src/cli/issue/create.rs`
(traces to BC-3.4.010 invariant 1; BC-3.4.011 invariants 1-3; verification-delta-388.md §2 P1–P4)

Add `#[cfg(test)] mod is_cross_hierarchy_type_error_proptests` in `src/cli/issue/create.rs` (a NEW
dedicated submodule, NOT the bare `mod tests` which already exists). This follows the established
convention of `build_labels_proptests` and `parse_field_kv_proptests` in the same file.

Properties to assert (all four are mandatory):

- **P1:** `Some(a), Some(b)` with `a != b` ⟹ result == `Classification::CrossHierarchy`
- **P2:** `Some(a), Some(b)` with `a == b` ⟹ result == `Classification::SameCategory`
- **P3:** either argument `None` ⟹ result == `Classification::Indeterminate`
- **P4 (load-bearing):** for any `err` string, re-running with a fixed contrasting message (`""`)
  yields the same result — the `err` argument NEVER changes the classification.

Strategy (verbatim from verification-delta-388.md §2 — F4 implementer copies this):

```rust
use proptest::prelude::*;

fn opt_bool() -> impl Strategy<Value = Option<bool>> {
    prop_oneof![Just(None), Just(Some(true)), Just(Some(false))]
}

proptest! {
    #[test]
    fn prop_cross_hierarchy_decided_by_subtask_flag_mismatch(
        src in opt_bool(),
        tgt in opt_bool(),
        err in prop_oneof![
            ".*",
            Just("issue type selected is invalid".to_string()),
            Just(String::new()),
        ],
    ) {
        let result = is_cross_hierarchy_type_error(src, tgt, &err);

        match (src, tgt) {
            (Some(a), Some(b)) if a != b => {
                prop_assert_eq!(result, Classification::CrossHierarchy);  // P1
            }
            (Some(a), Some(b)) => {
                let _ = (a, b);
                prop_assert_eq!(result, Classification::SameCategory);    // P2
            }
            _ => {
                prop_assert_eq!(result, Classification::Indeterminate);   // P3
            }
        }

        // P4: err must not change the verdict
        let baseline = is_cross_hierarchy_type_error(src, tgt, "");
        prop_assert_eq!(
            is_cross_hierarchy_type_error(src, tgt, &err),
            baseline,
        );
    }
}
```

`Classification` MUST derive `PartialEq + Debug` for `prop_assert_eq!` to compile.
`opt_bool()` covers the full 9-state `Option<bool> × Option<bool>` domain (`None`, `Some(true)`,
`Some(false)`).

## Required Test Deliverables Summary

| # | Test | File | Type | AC | Mandatory |
|---|------|------|------|----|-----------|
| 1 | `test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint` | `tests/issue_edit_type_errors.rs` | NEW integration | AC-6 | YES |
| 2 | `test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint` | `tests/issue_edit_type_errors.rs` | NEW integration | AC-6 | YES |
| 3 | `test_edit_type_same_hierarchy_400_surfaces_typo_hint` | `tests/issue_edit_type_errors.rs` | NEW integration | AC-6 | YES |
| 4 | `test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error` | `tests/issue_edit_type_errors.rs` | NEW integration | AC-6 | YES |
| 5 | `test_edit_type_cross_hierarchy_hint_no_fake_endpoint_literal` | `tests/issue_edit_type_errors.rs` | NEW regression pin | AC-6 | YES |
| 6 | `test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error` | `tests/issue_edit_type_errors.rs` | NEW integration | AC-6 | YES |
| 7 | `test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error` | `tests/issue_edit_type_errors.rs` | NEW integration | AC-6 | YES |
| 8 | `test_edit_type_unresolved_type_name_surfaces_typo_hint` | `tests/issue_edit_type_errors.rs` | NEW integration | AC-6 | YES |
| 9 | `test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error` | `tests/issue_edit_type_errors.rs` | NEW integration | AC-6 | YES |
| 10 | `test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment` | `tests/issue_edit_type_errors.rs` | NEW integration | AC-6 | YES |
| T-06 | `test_subtask_parent_clear_surfaces_400_with_convert_hint` | `tests/issue_edit_no_parent.rs` | STRENGTHENED — 3 new assertions | AC-4 | YES |
| prop | `prop_cross_hierarchy_decided_by_subtask_flag_mismatch` | `src/cli/issue/create.rs` | inline proptest (mod is_cross_hierarchy_type_error_proptests) | AC-7 | YES |

## Files NOT to Touch (Regression Baseline)

These files MUST NOT be modified. All their tests must continue to pass unchanged.

| File | Why Unchanged |
|------|--------------|
| `src/api/jira/issues.rs` | `get_issue` and `edit_issue` signatures unchanged; `issuetype` already in `BASE_ISSUE_FIELDS` |
| `src/api/jira/projects.rs` | `get_project_issue_types` already returns `Vec<IssueTypeMetadata>` with `subtask: Option<bool>`; no signature change |
| `src/api/client.rs` | No new predicate method needed; no HTTP layer change |
| `src/error.rs` | `JrError::ApiError { status, message }` is the existing downcast target; no variant change |
| `src/cli/mod.rs` | No new CLI flags; no clap definition change |
| `src/cli/issue/create.rs` lines 267–736 | Field-building, bulk routing, JQL resolution — outside the error block; byte-for-byte unchanged |
| `src/cli/issue/create.rs` `handle_create` / `handle_jsm_create` | Different command path; zero code overlap with `handle_edit` |
| `src/cli/issue/workflow.rs` | `move` / `transition` / `assign` / `open`; unrelated |
| `src/cli/issue/list.rs` | Read path; unrelated |
| `tests/issue_bulk_pr2.rs` | Bulk edit path — does NOT include CrossHierarchy enrichment; unchanged |
| `tests/issue_bulk.rs` | Bulk path; unrelated |
| `tests/issue_commands.rs` | BC-3.3.x, BC-3.4.x success paths; unrelated |
| `tests/issue_create_jsm.rs` | JSM create path; `is_cross_hierarchy_type_error` logic not on JSM dispatch |
| `tests/issue_write_holdouts.rs` | Holdout suite; `handle_edit` success paths unchanged |
| `tests/issue_edit_no_parent.rs` T-01..T-05, T-07, T-08 | Non-T-06 tests are regression pins; MUST remain green unmodified |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~6 k |
| F2 PRD delta (`.factory/phase-f2-spec-evolution/prd-delta-388.md`) | ~9 k |
| F2 verification delta (`.factory/phase-f2-spec-evolution/verification-delta-388.md`) | ~7 k |
| BC files (BC-3.4.010 and BC-3.4.011 sections in `bc-3-issue-write.md`) | ~7 k |
| `src/cli/issue/create.rs` (read `handle_edit` + `is_subtask_parent_error` vicinity ~lines 800-900 + 1100-1200; `mod tests` end for proptest insertion site) | ~10 k |
| `src/types/jira/issue.rs` (scan for `IssueType` struct, `IssueFields` struct) | ~3 k |
| `tests/common/fixtures.rs` (scan for `IssueType {` constructions) | ~4 k |
| `tests/issue_edit_no_parent.rs` (read T-06 for strengthening context) | ~4 k |
| Tool outputs + `cargo test` + `cargo clippy` output | ~6 k |
| **Total** | **~56 k** |

Well within single-agent context. No split required. LOC delta estimate: ~25 lines in `issue.rs`
(additive struct field); ~120 lines in `create.rs` (enum + classifier + const + dispatch block +
`--no-parent` hint replacement + proptest module); ~400 lines in `tests/issue_edit_type_errors.rs`
(10 new test functions); ~6 lines in `tests/issue_edit_no_parent.rs` (3 new assertions on T-06);
~N lines in `tests/common/fixtures.rs` (1 field per `IssueType` struct-literal).

## Tasks

- [ ] Read F2 PRD delta (`.factory/phase-f2-spec-evolution/prd-delta-388.md`) — capture all ten test
      names, verbatim pinned strings (`CROSS_HIERARCHY_HINT`, context sentence, typo hint), dual-gate
      precedence rules, call-ordering preconditions, and the `get_project_issue_types` deserialization
      behavior note (CRITICAL-2)
- [ ] Read F2 verification delta (`.factory/phase-f2-spec-evolution/verification-delta-388.md`) §2 —
      capture P1–P4 proptest strategy and `opt_bool()` implementation
- [ ] Read `bc-3-issue-write.md` §BC-3.4.010 and §BC-3.4.011 — extract exact preconditions,
      postconditions, invariants, and edge case IDs for story compliance verification
- [ ] Read `src/types/jira/issue.rs` — locate `IssueType` struct; confirm current field list; also
      locate `IssueFields` to confirm the Rust field name is `issue_type` (with `#[serde(rename="issuetype")]`)
- [ ] Read `tests/common/fixtures.rs` — search for `IssueType {` to find all struct-literal
      constructions requiring `subtask: None`
- [ ] Read `src/cli/issue/create.rs` — locate `is_subtask_parent_error` (~line 1159) as anchor for
      new `is_cross_hierarchy_type_error` placement; locate `handle_edit` function + its error block;
      locate ~line 834 `--no-parent` hint block (the four-sentence `format!` to replace); locate
      existing `mod tests` + proptest modules (`build_labels_proptests`, `parse_field_kv_proptests`)
      for placement of new `is_cross_hierarchy_type_error_proptests`
- [ ] Read `tests/issue_edit_no_parent.rs` — locate T-06
      (`test_subtask_parent_clear_surfaces_400_with_convert_hint`) and read its full assertion block
- [ ] Add `subtask: Option<bool>` with `#[serde(default)]` to `IssueType` struct in
      `src/types/jira/issue.rs`
- [ ] Add `subtask: None` to every `IssueType { ... }` struct-literal in
      `tests/common/fixtures.rs`
- [ ] Add `Classification` enum (deriving `PartialEq + Debug`) to `src/cli/issue/create.rs` near
      `is_subtask_parent_error`
- [ ] Add `CROSS_HIERARCHY_HINT: &str` constant with verbatim text from BC-3.4.010 (the constant in
      this story is the CANONICAL SOURCE; copy exact bytes from BC body)
- [ ] Add pure function `is_cross_hierarchy_type_error(src_subtask: Option<bool>, tgt_subtask: Option<bool>, err: &str) -> Classification`
      implementing P1–P3 rules; ensure `err` has zero influence on the return value (P4)
- [ ] Replace ENTIRE `--no-parent` hint block at `create.rs:834` (the multi-line `format!` spanning
      ~lines 830-836 containing four old sentences) with: `eprintln!` of context sentence + `eprintln!`
      of `CROSS_HIERARCHY_HINT` constant — NOTHING ELSE; line 837 (`bail!`) is unchanged
- [ ] Wire the `handle_edit` 400 error dispatch block: HTTP-400 gate via downcast to
      `JrError::ApiError { status: 400, .. }`; `--type` arm evaluated FIRST (before `--no-parent` arm);
      call `get_issue` first, then `get_project_issue_types` (call-ordering precondition from BC-3.4.010);
      use `Result::is_err()` (NOT a downcast) for enrichment-fetch failure detection; case-insensitive
      exact match on `name` field for type lookup; emit per-classification as per AC-5 Step D
- [ ] Strengthen T-06 in `tests/issue_edit_no_parent.rs` with three new assertions as specified in
      AC-4 (literal-pin on `JRACLOUD-27893`; negative pin on `jr api /rest/api/3/issue`; full
      verbatim context sentence pin)
- [ ] Add `#[cfg(test)] mod is_cross_hierarchy_type_error_proptests` in `src/cli/issue/create.rs`
      with `prop_cross_hierarchy_decided_by_subtask_flag_mismatch` function per AC-7 strategy
- [ ] Add new file `tests/issue_edit_type_errors.rs` with all ten named integration test functions
      per AC-6 table; mount wiremock topologies as specified
- [ ] Run `cargo test --test issue_edit_type_errors` — all 10 tests pass
- [ ] Run `cargo test --test issue_edit_no_parent` — T-06 and all other tests pass
- [ ] Run `cargo test --lib` — inline proptest passes; no regression in `create.rs` unit tests
- [ ] Run `cargo test` — full suite green; `tests/issue_write_holdouts.rs` unchanged; `tests/issue_bulk_pr2.rs` unchanged
- [ ] Run `cargo clippy -- -D warnings` — zero warnings; no `#[allow]` suppressions; refactor if needed
- [ ] Run `cargo build --release` — succeeds
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` — all three exit 0
- [ ] Per-story adversary 3/3 CLEAN before push

## Previous Story Intelligence

This story follows S-385 (JSM input validation UX polish, PR #395) and is within the same
`src/cli/issue/create.rs` file. Key lessons carried forward:

- **From S-384 (JSM 401 hints):** Both S-384 and S-388 improve error surfacing on command paths in
  `create.rs`. The same file-read-before-edit discipline applies: read the full function (`handle_edit`)
  before making any changes, not just the error-block vicinity. Context around line 834 matters
  significantly for the `--no-parent` hint replacement.

- **From S-385 (JSM UX polish):** The "replace entire block" pattern is critical — the O-08-07 lesson
  applies here too. Replacing only part of the old `--no-parent` hint block (lines 830-836) leaves
  stranded sentences. Verify by inspecting the full `format!` span in source — the four-sentence old
  block must be completely replaced, not partially.

- **From S-382 (InsufficientScope refactor):** Pure helper functions near sibling helpers is the
  established pattern. `is_cross_hierarchy_type_error` co-locates with `is_subtask_parent_error`
  exactly as `is_oauth_auth` in S-384 co-located with other auth predicates.

- **From S-345 (label-coalesce proptest):** The inline proptest submodule naming pattern is
  `<function_name>_proptests` (see `build_labels_proptests`, `parse_field_kv_proptests`). Use
  `is_cross_hierarchy_type_error_proptests` — do NOT add to the existing bare `mod tests`.

- **Verbatim string discipline (from multiple prior stories):** Copy pinned strings BYTE-FOR-BYTE
  from the BC body in `bc-3-issue-write.md`. The `CROSS_HIERARCHY_HINT` constant text, the context
  sentence, and the typo hint are all pinned verbatim in the BC. Any character deviation causes
  adversarial failures (the integration tests assert exact content).

- **Rust field name discipline (F4 implementer notes — see below):** The BC prose writes the JSON
  field path as `issue.fields.issuetype`; the actual Rust field on `IssueFields` is `issue_type`
  with `#[serde(rename="issuetype")]`. Use `issue.fields.issue_type.as_ref().and_then(|t| t.subtask)`
  in the implementation, NOT `issue.fields.issuetype`.

## Architecture Compliance Rules

Extracted from `bc-3-issue-write.md` §BC-3.4.010 and §BC-3.4.011 and CLAUDE.md conventions:

1. **Single-key path only.** The CrossHierarchy/SameCategory/Indeterminate enrichment applies
   EXCLUSIVELY to the single-key `handle_edit` path. The bulk `--type` path (`handle_edit_bulk_fields`)
   at `create.rs:736` returns before the single-key error block and MUST NOT receive this enrichment.

2. **`--type` arm evaluated before `--no-parent` arm (dual-gate precedence, CRITICAL-4).** Both flags
   can be set simultaneously (no clap `conflicts_with` between them). The `--type` arm executes first
   in `handle_edit`'s `if let Err(ref e) = edit_result` block. The `--no-parent` arm only fires if
   the `--type` arm emits no hint. First-hint-wins. This ordering MUST be enforced in code, not just
   documented.

3. **`is_err()` gate for enrichment-fetch failures, NOT a downcast.** `get_issue` returning 401
   produces `JrError::NotAuthenticated`, not `JrError::ApiError{401}`. A downcast gate would miss it.
   Use `Result::is_err()` for both `get_issue` and `get_project_issue_types` fetch-failure detection.
   This is deliberately distinct from the HTTP-400 gate on `edit_issue`'s error, which IS a structured
   downcast.

4. **`is_cross_hierarchy_type_error` does NOT replace `is_subtask_parent_error`.** The two helpers
   address distinct errors and MUST remain separate (F1 §Open Questions 3). `is_subtask_parent_error`
   remains the gate for the `--no-parent` parent-clear 400 (a subtask refusing to lose its parent).
   Do not merge them or modify `is_subtask_parent_error`.

5. **No `#[allow]` suppressions.** Per CLAUDE.md: if clippy warns, refactor to fix the root cause.
   The `Classification` enum and the new helper should not trigger clippy, but confirm with
   `cargo clippy -- -D warnings` before finalizing.

6. **No new modules, no new public API.** The additive changes are: 1 struct field, 1 const, 1 enum,
   1 pure helper function, 1 dispatch block, 1 proptest module. All within existing files.

7. **`get_project_issue_types` 200+malformed-body behavior is a known-correct graceful path.** Do
   NOT modify `src/api/jira/projects.rs` to change the `.ok().unwrap_or_default()` behavior. A 200
   with malformed body → `Ok([])` → typo hint is intentional and acceptable per F2 CRITICAL-2 note.

## Library & Framework Requirements

No new dependencies. All changes use stdlib + existing project types.

- `proptest` is already a dev-dependency in `Cargo.toml` (used by `build_labels_proptests` and
  `parse_field_kv_proptests` in the same file). Use the same import pattern and version for the
  new inline proptest module.
- `wiremock` is already a dev-dependency — use the same version and import pattern as existing
  integration tests in `tests/issue_edit_no_parent.rs` for the ten new tests.
- `assert_cmd` + `predicates` are already present for binary-level assertions.
- No version pins change; no `Cargo.toml` edits.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/types/jira/issue.rs` | Modify | Add `subtask: Option<bool>` with `#[serde(default)]` to `IssueType` struct; ~3 LOC net |
| `src/cli/issue/create.rs` | Modify | Add `Classification` enum + `CROSS_HIERARCHY_HINT` const + `is_cross_hierarchy_type_error` helper + dispatch block in `handle_edit` + `--no-parent` hint replacement + `is_cross_hierarchy_type_error_proptests` module; ~120 LOC net |
| `tests/common/fixtures.rs` | Modify | Add `subtask: None` to each `IssueType` struct-literal; ~N LOC net (depends on count of struct-literals) |
| `tests/issue_edit_no_parent.rs` | Modify | Strengthen T-06 with 3 new assertions; ~6 LOC net |
| `tests/issue_edit_type_errors.rs` | Create new | 10 new integration test functions; ~400 LOC net |
| `.factory/stories/STORY-INDEX.md` | Modify | Append S-388 row to Story Manifest + Feature Followup table; update total_stories (43→44) and last_updated |
| `.factory/sprint-state.yaml` | Modify | Append S-388 entry under `feature_followup_standalone` block (if file exists) |

**Files NOT to create:** No new source modules. No VP-NNN documents (project standard is BC-level
anchoring; confirmed by verification-delta-388.md §1). No new spec files — BC files are sealed.

## Branch / PR Plan

- Branch: `feat/issue-388-cross-hierarchy-type-change-error`
- Target: `develop`
- Commit style: `feat(issue): cross-hierarchy edit --type 400 enrichment + fix --no-parent fake-endpoint hint (closes #388)`
- PR closes #388
- CHANGELOG entry recommended: error-message improvements for `jr issue edit --type` and `jr issue edit --no-parent` are user-visible changes

**Why `breaking_change: false`:** No previously-successful `jr issue edit` invocation changes
outcome. The enrichment logic is on the error path only (HTTP 400 from Jira); the success path
(PUT 204) is byte-for-byte unchanged. The `--no-parent` hint change replaces a wrong hint
(referencing a non-existent endpoint) with a correct one — still an error, better message. No
success-path behavior changes.

## Per-Story Delivery Notes

- Demos (Step 5) are LOCAL-ONLY per `docs/demo-evidence/` gitignore convention.
- Per-story adversary 3/3 CLEAN required before push.
- F2 is CONVERGED (7 adversarial passes, 2026-05-20) — BC files are sealed. Do NOT re-edit BC files
  unless the adversary finds a discrepancy between the BC body and the implementation.
- The `check-bc-cumulative-counts.sh` guard MUST exit 0 post-edit. F2 already committed the +2
  definitional BC delta (BC-3.4.010 and BC-3.4.011). If the BC files were already updated in F2,
  both guard scripts should already exit 0 without additional spec-file edits.
- AUTHORITATIVE TEST COUNT IS TEN. Do not revert to five (F1 figure) or eight (F2 Pass-6 figure).
  Tests #9 and #10 are MANDATORY (F2 Pass-7 findings O-1 and O-2).

## F4 Implementer Notes

### CRITICAL: Rust Field Name vs. BC Prose Field Name

The F2 PRD delta and BC bodies write the JSON path as `issue.fields.issuetype` (the JSON wire name).
The ACTUAL RUST FIELD on `IssueFields` is `issue_type` with `#[serde(rename="issuetype")]`.

In the implementation, ALWAYS use:
```rust
issue.fields.issue_type.as_ref().and_then(|t| t.subtask)
```
NOT:
```rust
issue.fields.issuetype.as_ref().and_then(|t| t.subtask)  // WRONG — compile error
```

This field-name discrepancy is a known non-blocking item from F1 delta analysis reconciliation point
2 (and carried forward by instruction). The BC prose is descriptive (JSON path); the Rust code uses
the struct field name. Verify by reading `src/types/jira/issue.rs` `IssueFields` before implementing.

### `--no-parent` Hint Replacement Scope (CRITICAL-3)

The prior hint block at `src/cli/issue/create.rs:830-837` contains:
- A multi-line `format!` spanning approximately lines 830-836 with FOUR old sentences including the
  fake `jr api /rest/api/3/issue/{key}/convert -X put -d '{"type":{"name":"Task"}}'` line.
- Line 837: a separate `bail!` statement.

Replace the `format!` (lines 830-836) with the two-part new output. Do NOT modify line 837 (`bail!`).
Do NOT retain any of the four old sentences.

### `get_project_issue_types` vs. `get_issue` Error Handling Asymmetry

`get_issue` deserialization failure returns `Err` (feeds into Indeterminate via `is_err()` gate).
`get_project_issue_types` deserialization failure returns `Ok(vec![])` (feeds into typo-hint via
unresolvable-name path). This asymmetry is in the live code and is intentional. Do NOT try to align
the two by modifying `projects.rs`.

### Proptest `mod` Name Collision

The file `src/cli/issue/create.rs` already has a `#[cfg(test)] mod tests { ... }` block. Adding the
proptest to that block would cause name collisions. Use a SEPARATE `#[cfg(test)] mod is_cross_hierarchy_type_error_proptests { ... }` block, matching the `build_labels_proptests` / `parse_field_kv_proptests` convention.
