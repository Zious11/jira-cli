# Adversarial Review — Phase 1d Pass 8

**Convergence trajectory**: 30 → 15 → 9 → 5 → 10 → 5 → 4 → 3

## §1: Findings — 3 (0 CRITICAL / 1 HIGH / 2 MEDIUM / 0 LOW)

### HIGH

**ADV-P8-001 — nfr-catalog.md routing summary arithmetic mismatch**
Routing totals claim: FIX-IN-PHASE-3=10, SECURITY-DECIDE=3, POLICY-DECISION=3, DOCUMENT-AS-IS=13, DEFER=17. Sum = 46 ≠ 41. Hand-count of DEFER rows = 12, not 17.
- Action: Update DEFER count to 12.

### MEDIUM

**ADV-P8-002 — adr-index ADR-0009 mis-anchor**
adr-index.md:21 says ADR-0009 → §R-H4. R-H4 is list_worklogs (ADR-0010). R-H3 is handle_open (ADR-0009).
- Action: Change §R-H4 → §R-H3 on ADR-0009 row.

**ADV-P8-003 — risk-register Retry-After duplicate**
R-M3 (MEDIUM, line 40) and R-L11 (LOW, line 63) both describe same Retry-After integer-only gap, both reference NFR-SCA-1 (LOW), both route DOCUMENT-AS-IS. NFR-SCA-1 authoritative severity is LOW.
- Action: Merge R-M3 into R-L11 (Pass 7 NFR-O-K pattern). Update Risk Summary: 1C/6H/8M/12L=26.

## §2: Strengths

1. BC source-line evidence verified across 5 random samples — all exact matches
2. State-machine BC anchors solid
3. NFR severity totals (1C/6H/15M/19L=41) reconcile

## §3: Routing

product-owner: ADV-P8-001, ADV-P8-003
architect: ADV-P8-002, ADV-P8-003

## §4: Verdict — FINDINGS (3)

Counter 0/3. Trajectory continues asymptotic decay.

## §5: Follow-ups

Recount routing totals after fix. Verify 26 risk total after R-M3/R-L11 merge.

Phase 1d adversary Pass 8 complete.
