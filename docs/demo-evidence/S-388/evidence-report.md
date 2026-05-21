# Demo Evidence Report — S-388

**Story:** Cross-hierarchy edit --type 400 enrichment + fix fake-endpoint hint (closes #388)
**Date:** 2026-05-21
**Branch:** feat/issue-388-cross-hierarchy-type-change-error
**Binary:** `target/debug/jr` (debug build, `JR_BASE_URL` seam active)
**Mock server:** `docs/demo-evidence/S-388/mock-server.py` (Python 3 stdlib only)

---

## Coverage Summary

| AC | Title | Artifact(s) | Status |
|----|-------|-------------|--------|
| AC-1 | `IssueType` gains `subtask: Option<bool>` | Implicit — all 10 integration tests deserialize GET-issue responses | COVERED |
| AC-2 | Pure `is_cross_hierarchy_type_error` + `Classification` enum | `AC-001-*.gif/.webm`, `AC-002-*.gif/.webm`, proptest run (AC-7) | COVERED |
| AC-3 | `CROSS_HIERARCHY_HINT` const + `--no-parent` fake-endpoint hint replaced | `AC-001-*.gif/.webm`, `AC-004-*.gif/.webm` | COVERED |
| AC-4 | T-06 literal-pin strengthened in `tests/issue_edit_no_parent.rs` | Test run output (see below) | COVERED |
| AC-5 | `handle_edit` 400 dispatch: CrossHierarchy/SameCategory/Indeterminate | `AC-001..005-*.gif/.webm` (all 5 demo scenarios) | COVERED |
| AC-6 | Ten integration tests in `tests/issue_edit_type_errors.rs` | Test run output (see below) | COVERED |
| AC-7 | Inline proptest `mod is_cross_hierarchy_type_error_proptests` | Proptest run output (see below) | COVERED |

---

## VHS Recordings

Each recording demonstrates the `jr issue edit` binary against a local Python mock server
via `JR_BASE_URL` (debug-build seam, documented in CLAUDE.md). No real Jira instance is
contacted. Mock server source: `mock-server.py`.

### AC-001 — Cross-hierarchy: standard issue -> Sub-task type
**Covers:** AC-2 (classifier), AC-3 (CROSS_HIERARCHY_HINT constant), AC-5 (CrossHierarchy dispatch)
**BC anchors:** BC-3.4.010

- `AC-001-cross-hierarchy-std-to-subtask.gif`
- `AC-001-cross-hierarchy-std-to-subtask.webm`
- `AC-001-cross-hierarchy-std-to-subtask.tape` (source)

**Scenario:** `jr issue edit TEST-1 --type Sub-task`
Mock topology: PUT -> 400; GET issue -> `subtask: false` (standard Task);
GET project types -> list includes Sub-task with `subtask: true`.
Expected output on stderr: `CROSS_HIERARCHY_HINT` containing `JRACLOUD-27893`.
No `jr api /rest/api/3/issue` substring on stderr (regression pin on removed fake hint).

---

### AC-002 — Reverse cross-hierarchy: sub-task -> standard type
**Covers:** AC-2 (classifier both directions), AC-5 (CrossHierarchy dispatch)
**BC anchors:** BC-3.4.010

- `AC-002-cross-hierarchy-subtask-to-std.gif`
- `AC-002-cross-hierarchy-subtask-to-std.webm`
- `AC-002-cross-hierarchy-subtask-to-std.tape` (source)

**Scenario:** `jr issue edit SUB-1 --type Task`
Mock topology: PUT -> 400; GET issue -> `subtask: true` (Sub-task);
GET project types -> standard-only list (Task with `subtask: false`).
Expected output on stderr: `CROSS_HIERARCHY_HINT` containing `JRACLOUD-27893`.

---

### AC-003 — Same-hierarchy / typo name -> typo hint
**Covers:** AC-5 (SameCategory + unresolvable-name dispatch), BC-3.4.011
**BC anchors:** BC-3.4.011

- `AC-003-same-hierarchy-typo-hint.gif`
- `AC-003-same-hierarchy-typo-hint.webm`
- `AC-003-same-hierarchy-typo-hint.tape` (source)

**Scenario:** `jr issue edit TEST-8 --type Taks` (typo: "Taks" not in project type list)
Mock topology: PUT -> 400; GET issue -> `subtask: false`;
GET project types -> list NOT containing "Taks".
Expected output on stderr: typo hint containing `jr project types`. `JRACLOUD-27893` absent.

---

### AC-004 — `--no-parent` subtask path: context sentence + CROSS_HIERARCHY_HINT, no fake endpoint
**Covers:** AC-3 (`--no-parent` hint replacement), AC-4 (T-06 literal pins)
**BC anchors:** BC-3.4.010 (--no-parent path postcondition M-1)

- `AC-004-no-parent-context-and-hint.gif`
- `AC-004-no-parent-context-and-hint.webm`
- `AC-004-no-parent-context-and-hint.tape` (source)

**Scenario:** `jr issue edit TEST-NP --no-parent`
Mock topology: PUT -> 400 (`parent: Cannot remove parent from a sub-task.`);
GET issue -> `subtask: true` (sub-task).
Expected output on stderr:
1. "Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue."
2. `CROSS_HIERARCHY_HINT` (containing `JRACLOUD-27893`)
3. Raw 400 error
No `jr api /rest/api/3/issue` substring (negative regression pin, AC-4 assertion 2).

---

### AC-005 — Indeterminate path: raw error only, no hint
**Covers:** AC-5 (Indeterminate dispatch — Cause-1 R2: project-types 5xx)
**BC anchors:** BC-3.4.011

- `AC-005-indeterminate-raw-error.gif`
- `AC-005-indeterminate-raw-error.webm`
- `AC-005-indeterminate-raw-error.tape` (source)

**Scenario:** `jr issue edit TEST-4 --type Task`
Mock topology: PUT -> 400; GET issue -> `subtask: false`; GET project types -> HTTP 503.
Expected output on stderr: raw 400 error only. No `JRACLOUD-27893`. No typo hint.

---

## Test-Run Evidence

### AC-6 — Ten integration tests: `tests/issue_edit_type_errors.rs`

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/issue_edit_type_errors.rs (target/debug/deps/issue_edit_type_errors-...)

running 10 tests
test test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment ... ok
test test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error ... ok
test test_edit_type_unresolved_type_name_surfaces_typo_hint ... ok
test test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error ... ok
test test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint ... ok
test test_edit_type_same_hierarchy_400_surfaces_typo_hint ... ok
test test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint ... ok
test test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error ... ok
test test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error ... ok
test test_edit_type_cross_hierarchy_hint_no_fake_endpoint_literal ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.94s
```

All 10 mandatory tests pass. Test-to-AC mapping:

| Test | AC path | Demo analog |
|------|---------|-------------|
| `test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint` | BC-3.4.010 CrossHierarchy std→subtask | AC-001 |
| `test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint` | BC-3.4.010 CrossHierarchy subtask→std | AC-002 |
| `test_edit_type_same_hierarchy_400_surfaces_typo_hint` | BC-3.4.011 SameCategory | AC-003 (same-hierarchy variant) |
| `test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error` | BC-3.4.011 Indeterminate Cause-1 R2 | AC-005 |
| `test_edit_type_cross_hierarchy_hint_no_fake_endpoint_literal` | BC-3.4.010 regression pin | AC-001, AC-004 (negative pin) |
| `test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error` | BC-3.4.011 Indeterminate Cause-2 source | AC-005 variant |
| `test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error` | BC-3.4.011 Indeterminate Cause-2 target | AC-005 variant |
| `test_edit_type_unresolved_type_name_surfaces_typo_hint` | BC-3.4.011 unresolvable-name | AC-003 |
| `test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error` | BC-3.4.011 Indeterminate Cause-1 R1 | AC-005 variant |
| `test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment` | BC-3.4.010/011 R0b routing | (no demo needed — non-400 error, no visible hint) |

### AC-4 — T-06 in `tests/issue_edit_no_parent.rs` (strengthened)

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/issue_edit_no_parent.rs (...)

running 8 tests
test test_subtask_parent_clear_surfaces_400_with_convert_hint ... ok
[...7 other T-01..T-05, T-07, T-08 also ok...]

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
```

T-06 (`test_subtask_parent_clear_surfaces_400_with_convert_hint`) passes with all three new
assertions from AC-4:
1. `assert!(stderr.contains("JRACLOUD-27893"))` — literal pin
2. `assert!(!stderr.contains("jr api /rest/api/3/issue"))` — negative regression guard
3. `assert!(stderr.contains("Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue."))` — full verbatim context sentence pin

### AC-7 — Inline proptest `mod is_cross_hierarchy_type_error_proptests`

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (target/debug/deps/jr-...)

running 1 test
test cli::issue::create::is_cross_hierarchy_type_error_proptests::prop_cross_hierarchy_decided_by_subtask_flag_mismatch ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 722 filtered out; finished in 0.03s
```

Proptest `prop_cross_hierarchy_decided_by_subtask_flag_mismatch` passes, validating:
- **P1:** `Some(a), Some(b)` with `a != b` => `CrossHierarchy`
- **P2:** `Some(a), Some(b)` with `a == b` => `SameCategory`
- **P3:** either argument `None` => `Indeterminate`
- **P4:** `err` string does not influence classification

---

## AC Coverage Matrix

| AC | Description | Primary Evidence | Type |
|----|-------------|-----------------|------|
| AC-1 | `IssueType.subtask: Option<bool>` field | All 10 integration tests (implicit deserialization) | test run |
| AC-2 | `is_cross_hierarchy_type_error` + `Classification` enum | AC-001.gif + AC-002.gif + proptest | VHS + test run |
| AC-3 | `CROSS_HIERARCHY_HINT` const + `--no-parent` hint replacement | AC-001.gif + AC-004.gif | VHS |
| AC-4 | T-06 strengthened with 3 new assertions | T-06 passing in issue_edit_no_parent test run | test run |
| AC-5 | `handle_edit` 400 dispatch (5 paths) | AC-001 through AC-005 GIFs | VHS (x5) |
| AC-6 | 10 integration tests in `issue_edit_type_errors.rs` | issue_edit_type_errors test run (10/10 ok) | test run |
| AC-7 | Inline proptest `is_cross_hierarchy_type_error_proptests` | proptest run (1/1 ok) | test run |

All 7 acceptance criteria are covered by at least one artifact.
