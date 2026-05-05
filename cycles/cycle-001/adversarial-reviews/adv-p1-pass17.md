# Adversarial Review — Phase 1d Pass 17

**Convergence trajectory**: 30→15→9→5→10→5→4→3→4→0→2→0→3→0→2→0→3
**Counter regression**: 1/3 → 0/3 (4th reset)

## §1: Findings — 3 (0 CRITICAL / 1 HIGH / 2 MEDIUM / 0 LOW)

### HIGH

**ADV-P17-001 — SD-003 mis-anchors stale R-H3 (canonical is R-M0)**
SD-003-verbose-pii-redaction.md:6 "References: ... R-H3 (risk-register.md)". Risk-register.md:37 R-M0 is canonical for verbose body PII. Current R-H3 is the unrelated handle_open URL bug. Mis-anchor leads reader to wrong context.
- Action: Update SD-003 reference R-H3 → R-M0.

### MEDIUM

**ADV-P17-002 — state-machines.md cites phantom NFR-R-NEW-3**
domain-spec/state-machines.md:148 "future 401 auto-refresh integration (NFR-R-NEW-3)". Catalog only has NFR-R-NEW-1 and NFR-R-NEW-2. Canonical reference is NFR-O-B (refresh_oauth_token zero-callers).
- Action: Replace NFR-R-NEW-3 → NFR-O-B.

**ADV-P17-003 — L2 ↔ L3 bc_count drift**
- bc-04: L2 bc_count 44 vs L3 total_bcs 32 (delta 12)
- bc-06: L2 bc_count 38 vs L3 total_bcs 39 (delta 1)
- bc-07: L2 bc_count 126 vs L3 total_bcs 80 (delta 46)
- bc-01/02/03/05 align (suggests intent is alignment)
- Action: Reconcile L2 bc_count to match L3 total_bcs OR document intentional difference in CANONICAL-COUNTS.md.

## §2: Routing
architect: ADV-P17-001 (SD-003)
product-owner: ADV-P17-002 (L2 state-machines), ADV-P17-003 (L2 bc-* frontmatter)

## §3: Verdict — FINDINGS (3)

Counter regress 1/3 → 0/3 (4th reset across 17 passes). Convergence is asymptotic — each fresh adversary applies different lenses, finds different small drifts.

Phase 1d adversary Pass 17 complete.
