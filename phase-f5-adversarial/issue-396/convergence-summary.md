---
document_type: f5-convergence-summary
feature: issue-396 (jr issue edit --field NAME=VALUE)
phase: F5 Scoped Adversarial Review
date: 2026-05-25
verdict: CONVERGED
passes_total: 4
convergence_at: passes 2/3/4 (3 consecutive CLEAN)
fix_prs: [FIX-F5-001]
---

# F5 Convergence Summary — Issue #396

## Outcome

**CONVERGED.** 4 passes total. Convergence at passes 2/3/4 (3 consecutive CLEAN as
required by VSDD). Feature cleared for release.

## Pass Trajectory

| Pass | Date | Verdict | HIGH | MEDIUM | LOW | Notes |
|------|------|---------|------|--------|-----|-------|
| 1 | 2026-05-25 | NOT-CLEAN | 1 | 0 | 4 | HIGH-1: silent-drop `--label`+`--field`; routed to FIX-F5-001 |
| 2 | 2026-05-25 | CLEAN | 0 | 0 | 4 | FIX-F5-001 resolved; 4 LOW carry-forwards |
| 3 | 2026-05-25 | CLEAN | 0 | 0 | 4 | No new findings; 4 LOW pre-existing/cosmetic |
| 4 | 2026-05-25 | CLEAN | 0 | 0 | 0 | Informational confirmations only; CONVERGENCE DECLARED |

Shorthand trajectory: `1→0→0→0` (HIGH count per pass).

## Pass 1 HIGH Finding

**HIGH-1 — Silent-drop of `--label` + `--field` on platform non-JSM path**

When both `--label` and `--field` flags are supplied on a standard (non-JSM) `jr issue
edit` invocation, `--label` triggers early dispatch to `handle_edit_bulk_labels`, which
does not accept `field_pairs`. The `--label` conflict block listed 11 forbidden-sibling
flags but omitted `--field`. The block's own comment documented the data-loss risk
("Combining them would silently drop the non-label fields (exit 0, data loss). Reject
the combination HERE."), yet `--field` was absent.

Result: user sees exit 0, label applied, `--field` value silently discarded.

## FIX-F5-001 Resolution

- **Branch:** `fix/F5-001-label-field-silent-drop`
- **PR:** #406
- **Merge commit:** `699a5fd` (squash-merge to develop, 2026-05-25)
- **Spec amendment:** EC-3.4.017-13 added to `bc-3-issue-write.md:1529`
  (factory-artifacts commit `9e61c05`)
- **Implementation:** `--field` added to `--label` conflict block in
  `src/cli/issue/workflow.rs` (or equivalent handler); exit 64 guard
- **Test:** Integration test pinning exit 64 for `--label` + `--field` combination

## Residual LOW Drift Items (for cycle close, NOT blockers)

The following LOW items from passes 2/3/4 are recorded as drift items. None block
convergence or release.

| ID | Description | Source Pass |
|----|-------------|-------------|
| DI-396-F5-1 | `--label` conflict-block negative regression coverage debt: 10 of 12 entries untested | Pass 2 L-3, Pass 3 L-2 |
| DI-396-F5-2 | Process-gap: no structural/meta-test enforces every BULK_SUPPORTED-minus-`label` and REJECTED_IN_BULK flag appears in the `--label` conflict block | Pass 2 L-3 (structural implication) |
| DI-396-F5-3 | clap help text for `--field` does not mention `--label` exclusion (UX papercut) | Pass 3 L-4 |
| DI-396-F5-4 | EC-3.4.017-13 line-anchor citation will drift as bc-3 is edited (same class as PG-385-3) | Pass 2 L-4, Pass 3 L-3 |
| R2-C4 | (carry-forward from F4) test 38 reimplements wire-serialization inline rather than exercising production code | F4 Copilot R2 |

## Related Artifacts

- Pass reports: `pass-1.md`, `pass-2.md`, `pass-3.md`, `pass-4.md` (this directory)
- Spec amendment: `.factory/specs/prd/bc-3-issue-write.md` EC-3.4.017-13
- Research artifacts: `.factory/research/f5-issue-396-label-field-silent-drop.md`
- FIX-F5-001 delivery: develop @ `699a5fd` (PR #406)
- Factory-artifacts spec commit: `9e61c05`
