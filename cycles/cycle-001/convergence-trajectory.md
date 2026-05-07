---
document_type: convergence-trajectory
level: ops
version: "1.0"
status: in-progress
producer: state-manager
timestamp: 2026-05-04T00:00:00
cycle: "cycle-001"
inputs: [adversarial-reviews/]
input-hash: "[live-state]"
traces_to: STATE.md
---

# Convergence Trajectory — cycle-001

## Finding Progression

| Pass | Date | Total | CRIT | HIGH | MED | LOW | Counter | Verdict |
|------|------|-------|------|------|-----|-----|---------|---------|
| 1 | 2026-05-04 | 30 | 4 | 11 | 12 | 3 | 0/3 | FINDINGS_REMAIN |
| 2 | 2026-05-04 | 15 | 0 | 6 | 6 | 3 | 0/3 | FINDINGS_REMAIN |
| 3 | 2026-05-04 | 9 | 1 | 3 | 3 | 2 | 0/3 | FINDINGS_REMAIN |
| 4 | 2026-05-04 | 5 | 0 | 0 | 4 | 1 | 0/3 | FINDINGS_REMAIN |
| 5 | 2026-05-04 | 10 | 0 | 0 | 7 | 3 | 0/3 | REGRESSION |
| 6 | 2026-05-04 | 5 | 0 | 1 | 3 | 1 | 0/3 | FINDINGS_REMAIN |
| 7 | 2026-05-04 | 4 | 0 | 0 | 3 | 1 | 0/3 | FINDINGS_REMAIN |
| 8 | 2026-05-04 | 3 | 0 | 1 | 2 | 0 | 0/3 | FINDINGS_REMAIN |
| 9 | 2026-05-04 | 4 | 0 | 0 | 4 | 0 | 0/3 | PLATEAU |
| 10 | 2026-05-04 | 0 | 0 | 0 | 0 | 0 | 1/3 | CLEAN-PASS |
| 11 | 2026-05-04 | 2 | 0 | 1 | 1 | 0 | 0/3 | REGRESSION |
| 12 | 2026-05-04 | 0 | 0 | 0 | 0 | 0 | 1/3 | CLEAN-PASS |
| 13 | 2026-05-04 | 3 | 0 | 0 | 3 | 0 | 0/3 | REGRESSION |
| 14 | 2026-05-04 | 0 | 0 | 0 | 0 | 0 | 1/3 | CLEAN-PASS |

## Trajectory Shorthand

`30→15→9→5→10→5→4→3→4→0→2→0→3→0`

## Per-Pass Details

### Pass 1 (2026-05-04)

**Findings:** 30 (4C/11H/12M/3L)
**Convergence counter:** 0 of 3

BC-INDEX rebuilt from canonical body files (CRITICAL). 3 SD-NNN security decision artifacts created. 29 of 30 findings addressed; 1 deferred (ADV-P1-030 — orchestrator process-gap, policies.yaml — codification task post Phase 1).

---

### Pass 2 (2026-05-04)

**Findings:** 15 (0C/6H/6M/3L)
**Convergence counter:** 0 of 3

Key HIGH: extract_error_message 3-way contradiction (ADV-P2-001); ≥11 holdout BC anchors incorrect after rebuild (ADV-P2-002); NFR-R-NEW-1 missing from catalog (ADV-P2-003); NFR-S-E severity inconsistent (ADV-P2-004); NFR catalog count 4-way disagreement (ADV-P2-005); DTU holdout count 47 vs 48 (ADV-P2-006).

---

### Pass 3 (2026-05-04)

**Findings:** 9 (1C/3H/3M/2L)
**Convergence counter:** 0 of 3

CRITICAL: site count canonicalized to 14 across 4 docs. HIGH: ADR-0007 fallback clause struck; cross-cutting.md error chain replaced with PRD-canonical 7-level table; NFR catalog total reconciled to 42 after NFR-S-F addition.

---

### Pass 4 (2026-05-04)

**Findings:** 5 (0C/0H/4M/1L)
**Convergence counter:** 0 of 3

MEDIUM: H-004 BC anchor corrected; H-005 BC anchor corrected; H-012 BC anchors corrected; architecture README risk count refreshed 26→27. LOW: nfr-catalog routing arithmetic corrected.

---

### Pass 5 (2026-05-04)

**Findings:** 10 (0C/0H/7M/3L)
**Convergence counter:** 0 of 3

REGRESSION from 5→10. Root cause: anchor tables in supplement files not subjected to same audit as BC bodies in prior passes. 10 cited + 4 sweep additionals all fixed. Count manifest: 542 BCs / 42 NFRs / 48 holdouts / 27 risks.

---

### Pass 6 (2026-05-04)

**Findings:** 5 (0C/1H/3M/1L)
**Convergence counter:** 0 of 3

HIGH: MatchResult enum corrected in arch cross-cutting.md (Exact/ExactMultiple/Ambiguous/None). MEDIUM: 7-step extract_error_message table removed from arch cross-cutting.md; NFR-R-NEW-1/2 moved to correct LOW section; R-H3 demoted MEDIUM. LOW: arch README risk arithmetic corrected.

---

### Pass 7 (2026-05-04)

**Findings:** 4 (0C/0H/3M/1L)
**Convergence counter:** 0 of 3

ADV-P7-001 CLOSED (false alarm — BC count 542 correct). MEDIUM: NFR-O-K merged into NFR-S-D; NFR total 42→41; cross-cutting.md definitional_count 63→64. LOW: arch cross-cutting.md MatchResult::ExactMultiple description rewritten.

---

### Pass 8 (2026-05-04)

**Findings:** 3 (0C/1H/2M/0L)
**Convergence counter:** 0 of 3

HIGH: nfr-catalog routing summary DEFER count corrected 17→12. MEDIUM: adr-index ADR-0009 anchor corrected §R-H4→§R-H3; R-M3 merged into R-L11 (duplicate Retry-After concern). Risk total 27→26.

---

### Pass 9 (2026-05-04)

**Findings:** 4 (0C/0H/4M/0L)
**Convergence counter:** 0 of 3

PLATEAU. MEDIUM: risk-register action breakdown recounted; NFR-S-F site path corrected `.cargo/deny.toml`→`deny.toml`; NFR-S-F cross-ref R-H6→R-H5; arch cross-cutting MatchResult::Ambiguous description corrected.

---

### Pass 10 (2026-05-04)

**Findings:** 0 — CLEAN-PASS
**Convergence counter:** 1 of 3

All Pass 9 fixes verified propagated cleanly. NFR 41, risks 26, BC 542, holdouts 48 all reconcile. MUST-FIX register consistent across 5+ docs. 5 BC source-line spot-checks exact.

---

### Pass 11 (2026-05-04)

**Findings:** 2 (0C/1H/1M/0L)
**Convergence counter:** 0 of 3 (REGRESSION from 1/3)

HIGH: tracing not a current dep — nfr-catalog.md + arch cross-cutting.md corrected. MEDIUM: cache count corrected "7 distinct"→"6 distinct" in L2 + arch state-machines.md.

---

### Pass 12 (2026-05-04)

**Findings:** 0 — CLEAN-PASS
**Convergence counter:** 1 of 3

Pass 11 regression healed. tracing dep claim consistent across 4 docs; cache count = 6 distinct consistent across L2 + arch state-machines.md. No new findings.

---

### Pass 13 (2026-05-04)

**Findings:** 3 (0C/0H/3M/0L)
**Convergence counter:** 0 of 3 (REGRESSION from 1/3)

MEDIUM: BC grand total 542→541 (double-count corrected in BC-INDEX footnote); NFR-O-G LOC 970→1,083; cicd-setup.md path ref in risk-register corrected. Comprehensive 4-sweep audit completed. CANONICAL-COUNTS.md created.

---

### Pass 14 (2026-05-04)

**Findings:** 0 — CLEAN-PASS
**Convergence counter:** 1 of 3

Comprehensive sweep + CANONICAL-COUNTS.md adoption healed Pass 13 regression. 4/4 source-truth spot checks exact. CANONICAL-COUNTS = 541/41/48/26 stable. 2 nitpicks demoted (holdout Group 1 label; "12+" vs "14" in L2 README — non-contradictory).

---

### Pass 15 (2026-05-04)

**Findings:** 2 (0C/1H/1M/0L)
**Convergence counter:** 0 of 3 (REGRESSION from 1/3; 5th counter reset)

bc-3 body 'Total:40'→'48 individually-bodied'; bc-3 subdomain 8→7; bc-1 sweep drift fixed (5→6 subdomains).

---

### Pass 16 (2026-05-04)

**Findings:** 0 — CLEAN-PASS
**Convergence counter:** 1 of 3

bc-*.md body sweep effective; CANONICAL-COUNTS adoption stable; MUST-FIX P0 register integrity holding.

---

### Pass 17 (2026-05-04)

**Findings:** 3 (0C/1H/2M/0L)
**Convergence counter:** 0 of 3 (REGRESSION; 4th counter reset across 17 passes)

SD-003 R-H3→R-M0; state-machines NFR-R-NEW-3→NFR-O-B; L2 bc_count sync bc-04/06/07.

---

### Pass 18 (2026-05-04)

**Findings:** 3 (0C/0H/2M/1L)
**Convergence counter:** 0 of 3 (5th counter reset)

BC-INDEX:630 line-440 sync; arch BC-4 map adds cli/assets.rs; H-046 fixture mechanism specified.

---

### Pass 19 (2026-05-04)

**Findings:** 5 (1C/1H/3M/0L)
**Convergence counter:** 0 of 3 (REGRESSION)

5 findings via rotated lenses (state-machine↔BC, cache audit, holdout↔BC bidirectional). CRITICAL SM-5 BC-X.8.001→BC-X.8.003. HIGH cache count drift 7→6. Partial-fix propagation pattern.
