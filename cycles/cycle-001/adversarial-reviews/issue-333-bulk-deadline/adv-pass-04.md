---
document_type: adversarial-review
phase: F5
issue: 333
commit: d5334fa
branch: fix/issue-333-bulk-deadline
pass: 04
producer: vsdd-factory:adversary
timestamp: 2026-05-12
status: CLEAN-PASS-1-of-3
---

# Adversarial Review — Issue #333 Bulk Deadline (Pass 04)

Commit reviewed: `d5334fa` (pass-03 apply set on top of `0439243` / `dbb4e5f` / `618ca14`).
**Verdict: CLEAN-PASS — counter 0 → 1.**

0 BLOCKING + 0 CONCERN + 2 NIT (cosmetic, subjective) + 1 carry-over [process-gap] tracked in pass-03.

## Critical / Important Findings

NONE. The pass-03 propagation cluster fixes hold. All three deadline-exceeded sites use the same variant + exit code + site-tag prefix family.

## Reorder Regression Verification (PASS)

The pass-03 cap-vs-deadline reorder was verified against three regression scenarios:

| Scenario | Outcome | Status |
|---|---|---|
| `deadline=None`, `Retry-After > 60s` | clamp returns Sleep(base); cap-check fires; ApiError(429) exit 1 | ✓ baseline preserved |
| `deadline=Some(future)`, `Retry-After > 60s` | clamp returns Sleep(min); cap-check fires; ApiError(429) exit 1 | ✓ pass-04 research caveat applies |
| `deadline=Some(expired)`, `Retry-After > 60s` | clamp returns Expired; DeadlineExceeded exit 124 | ✓ headline pass-03 C-2 fix |

## Site-tag Convention Audit (PASS)

The `[deadline:<site>]` family is novel but coherent with existing `[jr]` and `[verbose]` bracket-prefix conventions. No collision.

## Existing-test Exit-Code Regression Scan (PASS)

No test, doc, script, or spec asserts on exit code 1 for a bulk-task timeout that is now exit 124. `tests/issue_bulk_pr2.rs` failed-status tests use loose `!output.status.success()` (any non-zero passes). No regression surface.

## Observations (NIT)

### N-1 (LOW) — Stale section heading "(existing, unchanged)" after pass-03 added rows

**File/line:** `docs/superpowers/specs/2026-03-26-jrerror-exit-codes-design.md:19`

H3 heading reads `### Exit Code Mapping (existing, unchanged)`. Pass-03 N-4 added two rows. The parenthetical is now factually wrong.

**Fix:** Drop the parenthetical or rename to `### Exit Code Mapping`.

### N-2 (LOW) — Site-tag terminology divergence (impl `bulk-outer` vs research `bulk-poll`)

**File/line:** `src/api/jira/bulk.rs:412` uses `[deadline:bulk-outer]`; research-validation-pass-04 recommended `bulk-poll`.

Both names are semantically defensible. Story.md was updated to match the implementation. All live documents are internally consistent.

**Recommendation:** No change. Document divergence noted for the codebase-history record. (Adversary self-classified as "no change needed".)

## [process-gap] Carried Over From Pass-03

### G-1 — JrError variant + exit-code 4-step propagation checklist not yet codified

Pass-03 deferred codification to a follow-up factory-artifacts commit. The deferral itself IS documented (in adv-pass-03.md). Codified-lessons file does not yet contain the entry. This is a known follow-up commit, not a gap.

**Action:** Cycle-closing checklist sweeps this `[process-gap]` tag during cycle-001 close. No additional action in #333's PR.

## Convergence Trajectory

| Pass | BLOCKING | CONCERN | NIT | Verdict | Counter |
|---|---|---|---|---|---|
| 01 | 0 | 7 | 7 | not clean | 0 |
| 02 | 1 | 2 | 4 | not clean | 0 |
| 03 | 0 | 3 | 5 | not clean | 0 |
| **04** | **0** | **0** | **2 (cosmetic)** | **CLEAN-PASS** | **1/3** |

## Novelty Assessment

All findings are NEW:
- Reorder verification: pass-04 is first to verify the reorder doesn't break baseline.
- Site-tag audit: pass-04 is first to compare against existing prefixes.
- Existing-test scan: pass-04 is first to scan full test corpus.
- N-1: created by pass-03 N-4 itself; not noticed in pass-03.
- N-2: created by orchestrator's choice of `bulk-outer`; not noticed in pass-03.

## Verdict

**CLEAN-PASS — counter 0 → 1.** Need 2 more consecutive CLEAN passes from a fresh-context adversary to converge.
