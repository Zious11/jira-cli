---
document_type: adversarial-convergence
phase: F1d
cycle: 3-feature-jsm-request-types-288
target: "issue-288 F2 spec delta"
status: CONVERGED
timestamp: 2026-05-18
counter_final: 3/3
total_passes: 10
substantive_passes: 7
clean_passes: 3
---

# F1d Adversarial Convergence — Issue #288 JSM Request Types

**Phase:** F1d (Adversarial Spec Review)
**Target:** issue-288 F2 spec delta (18 BCs, 5 holdouts, 2 risks, 1 ADR)
**Final Verdict:** CONVERGED — 3/3 consecutive CLEAN-PASS
**Total Passes:** 10 (7 substantive + 3 confirmation)
**Clean Passes:** P08, P09, P10

---

## Trajectory Summary

| Pass | Verdict | Blocking | Concern | Nit | Cumulative Findings | Notes |
|------|---------|----------|---------|-----|---------------------|-------|
| P01 | FINDINGS | 4 | 6 | 3 | 13 | Initial pass; BC counts, wire shapes, intra-BC gaps |
| P02 | FINDINGS | 0 | 3 | 4 | 7 | Count arithmetic partial fix; holdout coherence gaps surfaced |
| P03 | FINDINGS | 0 | 4 | 6 | 10 | PRD-delta prose; README index stale; counter reset |
| P04 | FINDINGS | 0 | 2 | 5 | 7 | Pass-03 fixes propagated; intra-BC Behavior/Outputs divergence |
| P05 | FINDINGS | 0 | 2 | 3 | 5 | BC-X.12.001 self-contradiction; CANONICAL-COUNTS risk gap |
| P06 | FINDINGS | 0 | 2 | 3 | 5 | Frontmatter trace:/related: stale; verification-delta mis-anchor |
| P07 | FINDINGS | 0 | 1 | 3 | 4 | risk-register.md header↔body self-contradiction (DRIFT-010 CLOSED) |
| P08 | CLEAN | 0 | 0 | 0 | 0 | First CLEAN-PASS; counter 1/3 |
| P09 | CLEAN | 0 | 0 | 0 | 0 | Independent re-derivation; counter 2/3 |
| P10 | CLEAN | 0 | 0 | 0 | 0 | Final confirmation gate; counter 3/3 — CONVERGED |

**Trajectory shorthand:** 4B/6C/3N → 0B/3C/4N → 0B/4C/6N → 0B/2C/5N → 0B/2C/3N → 0B/2C/3N → 0B/1C/3N → 0B/0C/0N → 0B/0C/0N → 0B/0C/0N

---

## Findings Histogram by Category

| Category | P01 | P02 | P03 | P04 | P05 | P06 | P07 | Total |
|----------|-----|-----|-----|-----|-----|-----|-----|-------|
| Cross-doc count arithmetic | 3 | 1 | 2 | 0 | 2 | 0 | 0 | 8 |
| Prose narrative coherence | 1 | 1 | 3 | 1 | 0 | 0 | 0 | 6 |
| README/index propagation | 0 | 1 | 3 | 0 | 0 | 0 | 0 | 4 |
| Intra-BC field consistency | 4 | 1 | 0 | 4 | 2 | 0 | 0 | 11 |
| Frontmatter sibling fields | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 3 |
| Document header↔body | 0 | 0 | 0 | 0 | 0 | 1 | 4 | 5 |
| Holdout coherence | 1 | 2 | 1 | 1 | 0 | 0 | 0 | 5 |
| ADR cross-reference | 1 | 1 | 0 | 1 | 0 | 1 | 0 | 4 |
| **Total** | **13** | **7** | **10** | **7** | **5** | **5** | **4** | **51** |

Total findings across all substantive passes: **51**
Blocking: 4 (all in P01)
Concern: 20
Nit: 27

---

## Lessons Learned

### L-288-01: Remediation Propagation Drift (recurring pattern)

**Pattern:** Each remediation pass addressed findings in the most-directly-cited
artifact but failed to propagate the fix to sibling artifacts in the same artifact
family. This manifested in a different artifact class each pass:

- P02: Fixed BC body Behavior section; did not propagate to related Outputs/Effects
  columns in 3 sibling BCs.
- P03: Fixed PRD-delta summary count; did not propagate to README Document Map or
  Supplement Index (same count, different document).
- P04: Fixed intra-BC Behavior section wording; did not propagate to verification-
  delta call-site anchors.
- P05: Fixed BC-X.12.001 Behavior/Outputs divergence; did not propagate to
  CANONICAL-COUNTS risk arithmetic (orthogonal finding surfaced same pass).
- P06: Fixed bc-3 frontmatter `trace:` field; did not propagate to ADR-0014
  `related:` field (same remediation session, adjacent file).
- P07: risk-register header had been routed to STATE.md reclassification in P06
  remediation session but the source file was never patched — classic
  "document-the-fix instead of fix-the-source" anti-pattern.

**Recommendation:** After any multi-artifact remediation, run the DRIFT-008 widened
sweep (cross-doc arithmetic + prose coherence + intra-BC field consistency +
frontmatter enumeration) before declaring remediation complete. The recurrence
pattern across 6 consecutive passes (P02-P07) demonstrates that ad-hoc remediator
discipline is insufficient — tooling enforcement is required.

**Action:** Codify DRIFT-008 widening into a v0.6 scripts/ tooling pass targeting:
(a) cross-doc arithmetic for BCs, risks, holdouts, ADRs; (b) prose coherence;
(c) intra-BC Behavior↔Outputs/Effects↔Errors field consistency; (d) frontmatter
fields enumerating other artifacts (trace:, related:, sections:, traces_to:,
bc_anchors:, holdout_anchors:) with symbol-existence verification.

### L-288-02: Monotonic Decay is Genuine Convergence

Passing P01 blocking findings to remediation fully eliminates the blocking class
(no BLOCKING findings from P02 onward). Concern counts decay monotonically from
P04 (2→2→2→1→0→0→0). No oscillation after P03. This trajectory is the canonical
"well-remediated spec" signature — each class of finding, once addressed, stays
addressed. Contrast with oscillation-present trajectories (e.g., #334 19-pass
cycle) where earlier passes re-introduced drift into artifacts touched later.

The absence of oscillation in this cycle is attributed to: (1) each B finding
having a single source-of-truth fix (not a distributed pattern); (2) DRIFT-008's
progressive widening of the probe surface keeping the mandate list stable from P05
onward.

### L-288-03: risk-register Self-Consistency as a Mandated Probe

risk-register.md header↔body contradiction (DRIFT-010) was a pre-existing defect
exposed at P05 and closed at P07. This class of finding (document-internal header
claiming a count contradicted by the document's own body table) is cheap to check
but was not in the original 18-mandate list — it was added mid-cycle as a result
of a product-owner escalation.

**Recommendation:** Add "risk-register header total == Risk Summary table row count
arithmetic" as a standing mandate in the F1d probe set for all future feature
cycles. Cost: one arithmetic verification per pass. Value: prevents security-
gating document from carrying a self-contradicting total.

---

## Final Disposition

**F1d CONVERGED.** 3/3 consecutive CLEAN-PASS at passes 08, 09, 10.

The issue-288 F2 spec delta is internally consistent, cross-document-reconciled,
and implementable:

- **18 new BCs** (BC-3.8.001..010 + BC-X.12.001..008): all intra-BC fields
  consistent; all frontmatter sibling fields verified; all count references updated.
- **5 new holdouts** (H-NEW-JSM-RT-001..005): setup/expected coherent with BC
  postconditions across all 5 scenarios.
- **2 new risks** (R-H288-1, R-M288-1): both in risk-register.md body + header;
  risk-register self-consistent at 36 total.
- **ADR-0014** (dispatch-fork): frontmatter `related:` verified; body internally
  consistent; no non-existent cross-references.

**Orchestrator advances to:** Human approval gate → F3 incremental story
decomposition (if approved).

F2 spec evolution: **AWAITING HUMAN APPROVAL GATE.**
