---
document_type: consistency-audit
audit_id: pre-phase-3-gate
phase: phase-2-adv-converged
producer: consistency-validator
version: "1.0.0"
timestamp: 2026-05-07T00:00:00
scope: "Phase 1 spec corpus + Phase 2 story corpus"
traces_to: ".factory/STATE.md"
---

# Pre-Phase-3 Gate: Consistency Audit

**Audit Date:** 2026-05-07
**Scope:** Full `.factory/` artifact set — spec corpus (Phase 1) + story corpus (Phase 2)
**Methodology:** Index-first cross-document validation, reference resolution, count verification, dependency graph analysis, sample frontmatter-body coherence checks, outside-the-perimeter inspection.
**Auditor:** consistency-validator (fresh-context)

---

## Executive Summary

**Verdict: PASS-WITH-NITS**

The artifact set is structurally sound for Phase 3 dispatch. All MUST-FIX BC coverage is confirmed, all 45 BC IDs cited in story frontmatter resolve to canonical headings, all 41 NFR IDs resolve, all 5 risk IDs resolve, all ADR references resolve, the dependency graph is acyclic with no back-edges, and story file counts match index claims exactly (31 stories, 4 waves).

**Three findings** are classified MEDIUM. None are blocking. All are documented below with precise remediation instructions. One is a carry-forward STATE.md stale count from the Phase 2 creation burst, one is a STORY-INDEX BC anchor list that is incomplete for S-2.04, and one is a pre-Phase-3 holdout gap (SD-003 holdouts not yet registered in holdout-scenarios.md, mitigated by S-0.06 inline test plan).

**Consistency score: 96/100.** Zero CRITICAL or HIGH findings. Three MEDIUM findings, all remediable in under 10 minutes total, none blocking Phase 3.

---

## Findings Table

| ID | Severity | Validation | File:Line | Description | Remediation |
|----|----------|-----------|-----------|-------------|-------------|
| CV2-001 | MEDIUM | #2 Counts | `.factory/STATE.md:54` | Phase 2 completion row says "30 stories created (W0:7 + W1:8 + W2:7 + W3:8)" — stale from initial Phase 2 creation burst before S-3.09 was added. Actual counts are 31 stories, Wave 3 = 9. STORY-INDEX and WAVE-PLAN are both correct (31/W3:9). Only STATE.md carries the stale count. | Update STATE.md line 54 to "31 stories created (W0:7 + W1:8 + W2:7 + W3:9)". One-line fix. Non-blocking for Phase 3 dispatch. |
| CV2-002 | MEDIUM | #8 AC↔BC | `.factory/stories/STORY-INDEX.md:107` | S-2.04 BC anchor list in STORY-INDEX table shows only 3 BCs (BC-5.2.001, BC-5.2.005, BC-7.2.001) but the story's actual frontmatter `bc_anchors` contains 7 BCs: BC-5.2.001, BC-5.2.005, BC-5.2.007, BC-5.2.008, BC-5.3.001, BC-5.3.002, BC-7.2.001. This is a DRIFT-003 recurrence — STORY-INDEX anchor column not updated when story was expanded. The 4 missing BCs (BC-5.2.007, BC-5.2.008, BC-5.3.001, BC-5.3.002) all resolve to real headings in bc-5-boards-sprints.md. No orphan references. | Update STORY-INDEX line 107 S-2.04 BC Anchors column to list all 7 BCs: "BC-5.2.001, BC-5.2.005, BC-5.2.007, BC-5.2.008, BC-5.3.001, BC-5.3.002, BC-7.2.001". Non-blocking. |
| CV2-003 | MEDIUM | #5 Holdout | `.factory/specs/prd/holdout-scenarios.md` (absent entries) + `.factory/stories/WAVE-PLAN.md:36` | Wave 0 exit gate in WAVE-PLAN.md requires "SD-003 verbose-bodies holdouts (2): MUST-PASS" but these two holdouts have no H-NNN identifiers and are not registered in holdout-scenarios.md. S-0.06 `holdout_anchors: []` and STORY-INDEX shows "(new holdout per SD-003)" as a placeholder. S-0.06 story body defines the two holdouts inline (lines 70-73) rather than as registered spec entries. The SD-003 resolution document specifies holdouts should be added "post-Phase-3". By contrast, H-NEW-AUTH-002 (SD-002 analog) was explicitly handled by S-0.07. There is no S-0.NX story to formalize the SD-003 holdouts in holdout-scenarios.md before Phase 3. | Two options: (A) Accept the SD-003 holdout gap as deliberate (inline in S-0.06 test plan; Phase 4 evaluator can derive from SD-003 resolution spec), noting holdout-scenarios.md `total_holdouts` will need to jump from 49 to 51 when S-0.06 ships and a formalization story mirrors S-0.07 for SD-003. (B) Create S-0.08 (or expand S-0.07 scope) to formalize SD-003 holdouts as H-NEW-VERBOSE-001/002 before Phase 3. Neither option is strictly blocking. Recommend Option A with documentation, since SD-003 specifies "post-Phase-3" timing and the inline test plan in S-0.06 is precise. |

---

## Validation Matrix

| # | Validation | Status | Notes |
|---|-----------|--------|-------|
| 1 | Reference resolution — all BC/NFR/ADR/SD/Risk/Story IDs | CLEAN | All 45 BC IDs → canonical headings in bc-*.md + cross-cutting.md. All 41 NFR IDs → nfr-catalog.md rows. All 5 Risk IDs (R-H6, R-L12, R-L13, R-M1, R-M2) → risk-register.md. All ADR refs (ADR-0003/0006/0007/0008/0013) → files in docs/adr/ or .factory/architecture/adr/. All 3 SD docs exist and are RESOLVED. |
| 2 | Total counts integrity | FINDINGS | STORY-INDEX: 31 stories ✓; per-wave 7/8/7/9 ✓; Story Manifest 31 rows ✓; WAVE-PLAN: 7/8/7/9 ✓. BC-INDEX: 541 cumulative ✓ (confirmed by per-file sum); 309 individually-bodied ✓; per-file heading counts match claims exactly. NFR: 41 ✓. Holdouts: 48 ✓. Risks: 28 ✓. **FINDING: STATE.md:54 stale count "30 stories (W3:8)" — see CV2-001.** |
| 3 | MUST-FIX BC coverage | CLEAN | BC-3.4.001 → S-0.01 (bc_anchors confirmed); BC-X.5.002 → S-0.02; BC-4.3.001 → S-0.03; BC-6.3.001 → S-0.04. All 4 MUST-FIX stories exist, have correct bc_anchors, and reference correct holdouts (H-046, H-045, H-036, H-NEW-MP-001). |
| 4 | SD/ADR coverage | CLEAN | SD-001 PKCE: RESOLVED via ADR-0013 (file exists, status Accepted, reactivation trigger present); no implementing story needed per DEC-006 decision; S-3.09 formalizes routing column update. SD-002 JR_AUTH_HEADER: RESOLVED; S-0.05 implements, S-0.07 formalizes. SD-003 --verbose bodies: RESOLVED; S-0.06 implements. All 3 SDs are status RESOLVED. ADR-0013 exists and has Reactivation section. |
| 5 | Holdout traceability | FINDINGS | H-NEW-AUTH-002 correctly handled: not yet in holdout-scenarios.md (expected — added by S-0.07 in Phase 3), frontmatter note exists, holdout spec text defined in S-0.07 body. All 36 holdout IDs in story holdout_anchors resolve to defined H-NNN entries in holdout-scenarios.md **except H-NEW-AUTH-002** (expected gap, documented). **FINDING: SD-003 has 2 promised holdouts with no H-NNN IDs registered — see CV2-003.** |
| 6 | NFR coverage breadth | CLEAN | 41 NFRs in catalog. All 41 NFR IDs are cited by at least one story nfr_anchors entry. Zero NFRs with no story coverage. Zero story nfr_anchors cite IDs not in catalog. NFR coverage: COMPLETE. |
| 7 | Wave dependency DAG | CLEAN | 4 inter-story dependencies identified: S-0.07→S-0.05 (same wave), S-1.03→S-0.06 (forward), S-1.06→S-0.05 (forward), S-3.08→S-2.05 (forward). No back-edges, no cycles. All depended-upon story IDs exist. DAG is valid. |
| 8 | AC↔BC bidirectional traceability (sample) | FINDINGS | Sampled S-0.01..S-0.07, S-1.06, S-1.08, S-2.01, S-2.04, S-3.04. For all sampled stories: every bc_anchors BC has at least one tracing AC in the story body. No orphaned bc_anchors entries. **FINDING: STORY-INDEX S-2.04 BC anchor column incomplete vs frontmatter — see CV2-002.** All 45 BC IDs cited in frontmatter across all 31 stories resolve to real headings. Zero dangling references. |
| 9 | Frontmatter↔body coherence (sample) | CLEAN | Sampled S-0.05 (bc_anchors: empty, correct per comment), S-0.06 (bc_anchors: BC-X.1.001, body confirms), S-0.07 (bc_anchors: empty, doc-only story), S-1.06 (bc_anchors BC-1.1.001/002, body confirmed), S-1.08 (depends_on: [] confirmed; body line 274 verified as "No Wave 0 dependencies..." — Pass-12 fix confirmed intact), S-2.01 (bc_anchors 9 BCs, matches STORY-INDEX), S-3.04 (bc_anchors 3 BCs, matches STORY-INDEX). OBS-13-1 resolution confirmed: zero "JiaClient" typo occurrences found in sampled stories. |
| 10 | Governance state | CLEAN | STATE.md `phase: phase-2-adv-converged`; `current_step: phase-2-adv-converged`; Phase 2-adv convergence trajectory shows 3/3 FULL CONVERGENCE at Pass 13. All 5 Drift Items in STATE.md are either RESOLVED or process-gap (non-blocking). Blocking Issues table is empty. Next Phase correctly set to `phase-3-tdd-implementation`. |
| 11 | Outside-the-perimeter check | CLEAN | CI/CD: `.github/workflows/ci.yml` and `.github/workflows/release.yml` exist (verified). `.factory/cicd-setup.md` exists (status: AUDIT-COMPLETE). DTU assessment: `.factory/architecture/dtu-assessment.md` exists; `DTU_REQUIRED: false` confirmed. ARCH-INDEX: `.factory/architecture/adr-index.md` exists with all 13 ADRs. All ADRs cross-referenced: ADR-0006 note re PKCE addendum vs new ADR-0013 is resolved per DEC-006. BC-INDEX, NFR-catalog, holdout-scenarios, risk-register, CANONICAL-COUNTS all present. No missing mandatory artifacts. |

---

## Detailed Findings

### CV2-001 (MEDIUM) — STATE.md stale story count

**Location:** `.factory/STATE.md` line 54

**Exact text:** `30 stories created (W0:7 + W1:8 + W2:7 + W3:8)`

**Actual values:** 31 stories total; Wave 3 = 9 stories (S-3.01..S-3.09).

**Root cause:** STATE.md Phase 2 completion row was written when 30 stories existed (S-3.09 was likely added in a later burst), and the sibling-sweep didn't update STATE.md. STORY-INDEX, WAVE-PLAN, and the Story Manifest all correctly reflect 31/W3:9.

**Impact:** Low. STATE.md is a human-readable pipeline state tracker. No downstream tooling reads this field programmatically. Misleading to a new reader of STATE.md only.

**Remediation:** One-line change to STATE.md:
```
| 2: Story Decomposition | **complete** (story creation phase) | 2026-05-04 | 2026-05-06 | 31 stories created (W0:7 + W1:8 + W2:7 + W3:9); Phase 2-adv pending | |
```

---

### CV2-002 (MEDIUM) — STORY-INDEX S-2.04 BC anchor column incomplete

**Location:** `.factory/stories/STORY-INDEX.md` Wave 2 table, S-2.04 row (line ~107)

**STORY-INDEX shows:** `BC-5.2.001, BC-5.2.005, BC-7.2.001` (3 BCs)

**Story frontmatter bc_anchors contains:** BC-5.2.001, BC-5.2.005, BC-5.2.007, BC-5.2.008, BC-5.3.001, BC-5.3.002, BC-7.2.001 (7 BCs)

**All 4 missing BCs verified as real:** BC-5.2.007, BC-5.2.008, BC-5.3.001, BC-5.3.002 all have `#### BC-N.N.NNN:` headings in `.factory/specs/prd/bc-5-boards-sprints.md`.

**Root cause:** DRIFT-003 pattern — BC anchor column in STORY-INDEX not updated when story frontmatter was expanded during Phase 2 creation or adversarial fixes.

**Impact:** Low. The story frontmatter is the authoritative source (BC-INDEX preamble: "The body files are canonical"). STORY-INDEX is derived. An implementer reading only STORY-INDEX would under-scope S-2.04 by 4 BCs.

**Remediation:** Update STORY-INDEX S-2.04 row BC Anchors column to:
`BC-5.2.001, BC-5.2.005, BC-5.2.007, BC-5.2.008, BC-5.3.001, BC-5.3.002, BC-7.2.001`

No WAVE-PLAN change needed — WAVE-PLAN uses `BC-5.*` wildcard for this story.

---

### CV2-003 (MEDIUM) — SD-003 holdouts not registered in holdout-scenarios.md

**Location:** `.factory/specs/prd/holdout-scenarios.md` (absent entries); `.factory/stories/WAVE-PLAN.md` line 36 (exit gate claim)

**Observation:** WAVE-PLAN Wave 0 exit gate requires "SD-003 verbose-bodies holdouts (2): MUST-PASS". SD-003 resolution document (section "Add holdout post-Phase-3") explicitly names two MUST-PASS holdouts:
1. `--verbose-bodies` MUST emit the PII warning to stderr
2. `--verbose` alone MUST NOT print body content

Neither holdout has an H-NNN identifier. Neither is registered in `holdout-scenarios.md`. The `total_holdouts` field in holdout-scenarios.md remains 48 (or 49 after S-0.07). S-0.06 `holdout_anchors: []` and defines the holdouts inline (body lines 70-73) rather than as registered spec entries.

By contrast, the SD-002 analog was explicitly handled: SD-002 resolution created DRIFT-002, DRIFT-002 queued H-NEW-AUTH-002, S-0.07 formalizes the holdout in holdout-scenarios.md. The SD-003 chain is missing the formalization step.

**Impact:** Moderate. Phase 4 holdout evaluator will encounter:
- `holdout-scenarios.md` has no SD-003 holdout entries
- WAVE-PLAN exit gate references "SD-003 verbose-bodies holdouts (2)" but cannot link to H-NNN IDs
- S-0.06 story body has the precise specs; a careful evaluator will find them there and in SD-003 resolution document

**Symmetry gap:** SD-002 got S-0.07 (spec story to formalize holdout); SD-003 has no equivalent.

**Recommended resolution (Option A — minimal):** Accept the gap as deliberate for now. Add a frontmatter comment to holdout-scenarios.md analogous to the H-NEW-AUTH-002 note:
```yaml
# Note: SD-003 verbose-bodies holdouts (2) will be added by a Phase 3 formalization step.
# S-0.06 implementation + inline test plan constitutes the spec; Phase 4 evaluator
# should parse S-0.06 body lines 70-73 and SD-003 resolution §6 for verification criteria.
# Total will become 51 once both holdouts are registered.
```

**Alternative resolution (Option B — symmetrical):** Create S-0.08 mirroring S-0.07 to formally register the two SD-003 holdouts as H-NEW-VERBOSE-001 and H-NEW-VERBOSE-002 before Phase 3. This makes Phase 4 evaluation unambiguous and consistent with the SD-002 precedent.

---

## Reference Resolution Summary

| Category | Claimed Count | Verified Count | Status |
|----------|--------------|----------------|--------|
| Story files | 31 | 31 | CLEAN |
| BC headings (individually-bodied) | 309 | 309 | CLEAN |
| BC cumulative total | 541 | 541 | CLEAN |
| NFR catalog entries | 41 | 41 | CLEAN |
| Holdout scenarios | 48 | 48 | CLEAN |
| Risk register entries | 28 | 28 | CLEAN |
| ADR documents (0001-0013) | 13 | 13 | CLEAN |
| SD documents (001-003) | 3 | 3 | CLEAN |
| BC IDs in story bc_anchors | 45 unique | 45 all resolve | CLEAN |
| NFR IDs in story nfr_anchors | 41 | 41 all resolve | CLEAN |
| Risk IDs in story risk_anchors | 5 unique | 5 all resolve | CLEAN |
| ADR IDs in story adr_refs | 5 unique | 5 all resolve | CLEAN |
| Holdout IDs in story holdout_anchors (excluding H-NEW-AUTH-002) | 35 unique | 35 all resolve | CLEAN |
| H-NEW-AUTH-002 in stories | cited | not-yet-in-spec | EXPECTED (S-0.07 pending) |
| SD-003 holdouts | 2 implied | 0 registered | CV2-003 (MEDIUM) |

---

## Wave Dependency Graph

```
S-0.01 (no deps)
S-0.02 (no deps)
S-0.03 (no deps)
S-0.04 (no deps)
S-0.05 (no deps)
S-0.06 (no deps)
S-0.07 → depends on S-0.05 [same-wave forward dep: OK]

S-1.01 (no deps)
S-1.02 (no deps)
S-1.03 → depends on S-0.06 [inter-wave forward dep: OK]
S-1.04 (no deps)
S-1.05 (no deps)
S-1.06 → depends on S-0.05 [inter-wave forward dep: OK]
S-1.07 (no deps)
S-1.08 (no deps)

S-2.01..S-2.07 (all no deps)

S-3.01..S-3.07 (all no deps)
S-3.08 → depends on S-2.05 [inter-wave forward dep: OK]
S-3.09 (no deps)
```

**Result: DAG is valid. No cycles. No back-edges.**

---

## Governance State Snapshot

| Field | Value | Status |
|-------|-------|--------|
| STATE.md phase | phase-2-adv-converged | CORRECT |
| Phase 2-adv convergence | 3/3 FULL CONVERGENCE at Pass 13 | CORRECT |
| Next phase | phase-3-tdd-implementation | CORRECT |
| Blocking Issues | 0 open | CORRECT |
| Drift Items | DRIFT-001/003/004: process-gap (non-blocking); ADV-P2-S12-001: RESOLVED; OBS-13-1: RESOLVED; OBS-13-2: RESOLVED | CORRECT |
| DEC-009 gate | APPROVED (2026-05-04) | CORRECT |
| All SD decisions resolved | SD-001 Option C, SD-002 Option A, SD-003 Option B | CORRECT |
| activation_head | dea166471e22eff55974d7675593469b37048c5f | CORRECT |
| dtu_required | false | CORRECT (verified against dtu-assessment.md) |

---

## Outside-the-Perimeter Check Results

| Area | Artifact | Status | Notes |
|------|---------|--------|-------|
| CI/CD | `.github/workflows/ci.yml`, `release.yml` | PRESENT | Verified via filesystem |
| CI/CD factory audit | `.factory/cicd-setup.md` | PRESENT | status: AUDIT-COMPLETE |
| DTU assessment | `.factory/architecture/dtu-assessment.md` | PRESENT | DTU_REQUIRED: false |
| Architecture index | `.factory/architecture/adr-index.md` | PRESENT | All 13 ADRs listed |
| ADR-0013 Reactivation section | `.factory/architecture/adr/0013-pkce-deferral.md` | PRESENT | Reactivation Trigger section confirmed |
| CANONICAL-COUNTS.md | `.factory/specs/prd/CANONICAL-COUNTS.md` | PRESENT | last_verified: Pass 17 |
| S-3.09 ADR-0013 Decision Log note | `.factory/stories/wave-3/S-3.09-pkce-decision-deferred.md` | PRESENT | adr_refs includes ADR-0013 with inline comment noting formal Decision Log entry awaits S-3.09 implementation |
| SD-001 stale Phase 3 routing | `.factory/architecture/security-decisions/SD-001-pkce.md:72` | EXPECTED | "SECURITY-DECIDE" routing in SD-001 history section refers to pre-gate language; Resolution section correctly reflects Option C. Not a defect. |
| NFR-S-A, NFR-S-B, NFR-S-C routing columns | `.factory/specs/prd/nfr-catalog.md` | EXPECTED STALE | Routing columns still say "SECURITY-DECIDE" but decisions are RESOLVED. Pre-Phase-3 state — updating these is explicitly the job of S-3.09 (NFR-S-A), S-0.05 (NFR-S-B), S-0.06 (NFR-S-C) in Phase 3. Not a gate-blocking defect. |

---

## Consistency Score

| Category | Points Available | Points Earned | Notes |
|----------|-----------------|---------------|-------|
| Reference resolution (all IDs) | 20 | 20 | All BC/NFR/ADR/SD/Risk/H-NNN IDs verified |
| Story file counts | 10 | 9 | -1: STATE.md stale count (CV2-001) |
| BC counts | 10 | 10 | 541/309 all verified |
| MUST-FIX coverage | 15 | 15 | 4/4 MUST-FIX BCs covered |
| SD/ADR coverage | 10 | 10 | 3/3 SDs resolved, ADR-0013 present |
| Holdout traceability | 10 | 8 | -2: SD-003 holdouts unregistered (CV2-003) |
| NFR coverage | 10 | 10 | 41/41 NFRs covered |
| Wave DAG validity | 5 | 5 | Acyclic, no back-edges |
| Frontmatter-body coherence | 5 | 4 | -1: S-2.04 STORY-INDEX BC column (CV2-002) |
| Governance state | 5 | 5 | All correct |

**Total: 96/100**

---

## Recommendation

**Proceed to Phase 2 → Phase 3 human gate.**

Findings CV2-001, CV2-002, and CV2-003 are MEDIUM severity. None block Phase 3 dispatch:

- **CV2-001** (STATE.md stale count): Fix-at-gate — one-line change to STATE.md.
- **CV2-002** (STORY-INDEX S-2.04 BC column): Fix-at-gate — one-row update. Story frontmatter is the authoritative source and is already correct.
- **CV2-003** (SD-003 holdouts unregistered): Documented gap. Recommended approach is Option A (frontmatter comment) now, with S-0.07-analog formalization story to be added in Phase 3 scope. The gap does not invalidate the Wave 0 exit gate requirement — the holdout specs exist in S-0.06 and SD-003.

The corpus is coherent at the perimeter level. The adversarial review caught defects within the perimeter; this audit confirms the perimeter itself is correctly drawn: 31 stories covering all 4 MUST-FIX BCs, all 3 resolved SDs, all 41 NFRs, with a valid wave DAG and clean reference graph.

**Gate recommendation: APPROVE with minor pre-gate fixes for CV2-001 and CV2-002. CV2-003 requires decision note before gate close.**
