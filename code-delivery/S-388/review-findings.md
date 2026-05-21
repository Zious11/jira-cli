# Review Findings — S-388

## Convergence Table

| Cycle | Findings | Blocking | Suggestions | Fixed | Remaining | Verdict |
|-------|----------|----------|-------------|-------|-----------|---------|
| 1 | 1 | 0 | 1 | 0 | 0 | APPROVE |

## Cycle 1 — PR #397

**Date:** 2026-05-21
**Reviewer:** pr-review-triage (cycle 1)
**Verdict:** APPROVE

### Findings

| ID | Severity | Category | Description | Route | Status |
|----|----------|----------|-------------|-------|--------|
| F-1-1 | nit | description | Test #6 `expect(1)` on project-types call: notes that the implementation correctly fetches project types even when src_subtask is None — the `expect(1)` is valid and the implementation is correct. No change needed. | N/A | No action required |

### AC Verification Results

All 7 ACs verified PASS:
- AC-1: `IssueType.subtask: Option<bool>` with `#[serde(default)]` — PASS
- AC-2: Pure `is_cross_hierarchy_type_error` + `Classification` enum — PASS
- AC-3: `CROSS_HIERARCHY_HINT` const + `--no-parent` fake-endpoint hint replaced — PASS
- AC-4: T-06 strengthened with 3 new assertions — PASS
- AC-5: `handle_edit` 400 dispatch (3-way classification + unresolvable-name) — PASS
- AC-6: 10 integration tests in `tests/issue_edit_type_errors.rs` — PASS
- AC-7: Inline proptest `mod is_cross_hierarchy_type_error_proptests` (P1-P4) — PASS

### BC Contract Verification

All BC contracts PASS:
- BC-3.4.010 pinned strings: verbatim byte-match confirmed
- BC-3.4.011 pinned strings: verbatim byte-match confirmed
- Dual-gate precedence (`--type` before `--no-parent`): confirmed
- Call ordering (`get_issue` before `get_project_issue_types`): confirmed
- `is_err()` fetch-failure gate: confirmed
- No fake-endpoint hint at any call site: confirmed

**Status: CONVERGED in 1 cycle.**

## Post-Merge Summary

- **PR #397 merged:** 2026-05-21T23:03:43Z
- **Squash commit SHA:** e0ea24b007f5a949c79c995ec0467a63bd410eda
- **Branch:** `feature/issue-388-cross-hierarchy-type-error` (remote deleted; local cleanup pending worktree removal)
- **CI fix:** Added `test_no_parent_non_subtask_400_does_not_surface_cross_hierarchy_hint` to kill the `replace && with ||` mutant at create.rs:898 (kill rate went from 85% → ≥90%)
- **Final CI state:** All 10 checks PASS on run 26257634863
