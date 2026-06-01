# Phase F5 — Adversarial Convergence Summary: issue #331 (issueType bulk-edit delta)

- **Feature:** `fix(bulk): issueType bulk wire schema — camelCase key, issueTypeId value, cross-project guard`
- **Branch:** `fix/issue-331-issuetype-bulk`
- **Base:** `origin/develop @ 4fd91f1`
- **Implementation commits reviewed:**
  - `3cff3c7` — wire shape fix + cross-project guard + resolver
  - `affc33a` — F5 fixes: dry-run guard hoist + createmeta pagination + surface pin
  - `ee3dbeb` — F5 pass-4 mutation coverage gaps
- **Convergence achieved:** 3 consecutive clean passes (passes 5 / 6 / 7)
- **Date:** 2026-06-01

---

## Pass Trajectory

| Pass | Lens | Findings | Status | Fix commit |
|------|------|----------|--------|-----------|
| P1 | Standard adversarial (full delta) | 1 CRITICAL + 3 INFORMATIONAL | BLOCKED → REMEDIATED | `affc33a` |
| P2 | Standard re-review (post-affc33a) | 0 | CLEAN | — |
| P3 | Regression/specification alignment | 0 | CLEAN | — |
| P4 | Test-efficacy lens | 3 findings (F-1 + F-2 + F-3) | BLOCKED → REMEDIATED | `ee3dbeb` |
| P5 | Standard re-review (post-ee3dbeb) | 0 | **CLEAN** (streak start) | — |
| P6 | BC traceability lens | 0 | **CLEAN** | — |
| P7 | Regression + integration lens | 0 | **CLEAN** | — |

**Converged at pass 7 (3 consecutive clean: P5/P6/P7).**

---

## Pass 1 — Standard Adversarial Review

**Verdict: BLOCKED**

Findings:

- **C-1 (CRITICAL): EC-3.4.019-5 — dry-run cross-project guard missing** — the
  `--type` cross-project guard fired correctly on the live-API path but was absent
  from the dry-run builder block. A user running `jr issue edit FOO-1 BAR-2 --type
  Bug --dry-run` would receive no error and see a cross-project dry-run plan that
  would fail on actual execution. Guard was hoisted to run before the dry-run branch.

- **I-1 (INFORMATIONAL): createmeta pagination gap** — `get_issue_types_for_project`
  returned only the first page of issue types (no `isLast`-loop or empty-page
  terminator). Projects with many issue types could return incomplete results.
  Fixed with a proper offset-pagination loop mirroring the existing pattern in
  `issues.rs`.

- **I-2 (INFORMATIONAL): unknown-type error wording** — the error message for an
  unrecognised `--type` value did not list valid type names, violating the AC-002
  spec contract. Fixed to list valid types in the error output.

- **I-3 (INFORMATIONAL): CLI surface guard not updated** — `tests/e2e_cli_surface_guard.rs`
  SURFACE table was not updated to register the new `jr issue edit` bulk `--type`
  invocation added in `tests/e2e_live.rs`. Added the missing registration.
  [process-gap]: the adversary noted the guard only covers used-flags ⊆ listed-flags
  direction; the reverse (listed-flags ⊆ used-flags) is not enforced — deferred to
  maintenance sweep / engine scope.

Fix commit: `affc33a`

---

## Pass 2 — Standard Re-review (post-affc33a)

**Verdict: CLEAN**

No new findings. All P1 issues verified resolved. The dry-run guard, pagination,
error wording, and surface-guard registration all confirmed correct.

---

## Pass 3 — Regression / Specification Alignment

**Verdict: CLEAN**

No findings. Single-key path (BC-3.4.003/010/011) regression-verified untouched.
BC-3.4.018 / BC-3.4.019 ACs reviewed against implementation — all ACs satisfied.

---

## Pass 4 — Test-Efficacy Lens (Mutation-Gap Analysis)

**Verdict: BLOCKED**

Findings:

- **F-1 (HIGH): AC-005 case-insensitive resolution untested + false coverage claim**
  — AC-005 (case-insensitive `--type bug` → `Bug` resolution) was listed in the story
  test plan as covered by the rewritten `test_multi_key_type_update_body_uses_issue_type_id`,
  but the rewritten test used `--type Bug` (exact match), not `--type bug` (lowercase).
  A dedicated test `test_bulk_issuetype_resolves_type_name_case_insensitively` was
  required and added.

- **F-2 (HIGH): AC-007 dry-run camelCase key coverage gap** — no test asserted that
  the dry-run builder outputs `"issueType"` (camelCase) in `plannedChanges`. The
  dry-run builder key fix (from P1 I-2 remediation) was untested by any existing
  test. Added `test_bulk_issuetype_dry_run_uses_camelcase_key`.

- **F-3 (MEDIUM): helper edge cases** — `project_key_from_issue_key` unit tests
  covered only the happy-path cases (FOO-1, PROJ2-100). Edge cases (no-hyphen key,
  trailing-hyphen key, single-char project key) were not tested. Added unit tests for
  these cases (as BC-3.4.019 invariant 4 has no explicit handling requirement for
  invalid key formats — the function's documented contract was made explicit).

Fix commit: `ee3dbeb`

---

## Pass 5 — Standard Re-review (post-ee3dbeb)

**Verdict: CLEAN** ← streak pass 1

No new findings. All P4 issues verified resolved. Case-insensitive resolution,
dry-run key, and helper edge cases all confirmed correct and tested.

---

## Pass 6 — BC Traceability Lens

**Verdict: CLEAN** ← streak pass 2

All BC-3.4.018 postconditions and invariants (1–5) and BC-3.4.019 postconditions and
invariants (1–4) traced to specific tests. No gaps in traceability. Both BCs fully
covered.

---

## Pass 7 — Regression + Integration Lens

**Verdict: CLEAN** ← streak pass 3

Full test suite (`cargo test`, 1568/0/67) confirmed passing. Single-key path
regression tests (BC-3.4.003, BC-3.4.010, BC-3.4.011) unmodified and passing.
Multi-key non-type bulk edits (summary, priority) unaffected by new guard.
Integration across the full `issue_bulk_pr2.rs` suite confirmed.

---

## Convergence: ACHIEVED

3 consecutive clean passes (P5/P6/P7). CONVERGED at pass 7.

---

## Process Lesson (F5 pass-5 wrong-tree incident)

During one adversary dispatch (original P1 and the re-attempt that became P5),
the adversary initially reviewed the MAIN repository `develop` branch rather than
the worktree `fix/issue-331-issuetype-bulk`. This caused findings to reference
the pre-fix state rather than the actual delta under review.

**Root cause:** adversary dispatch prompt did not include the delta diff or a
mandatory self-check step confirming the correct worktree/branch.

**Mitigation applied for P5 onwards:** the adversary was fed the captured diff
(`/tmp/s331-delta.diff`) as explicit context AND required to include a
self-check line in its output confirming `HEAD` matches the expected commit.
This eliminated the wrong-tree misread pattern for all subsequent passes.

**Codified as a process lesson** (see `cycles/cycle-001/lessons.md`): adversary
dispatch MUST include either a diff attachment or an explicit HEAD-check step to
prevent wrong-tree analysis. Deferred to engine improvement (see Drift Items in
STATE.md).

---

## Statistics

| Metric | Value |
|--------|-------|
| Total passes | 7 |
| BLOCKED passes | 2 (P1, P4) |
| CLEAN passes | 5 (P2, P3, P5, P6, P7) |
| Total findings addressed | 1 CRITICAL + 3 INFORMATIONAL (P1) + 3 HIGH/MEDIUM (P4) = 7 findings |
| Convergence streak | 3 (P5/P6/P7) |
| Implementation commits | 3 (`3cff3c7`, `affc33a`, `ee3dbeb`) |
