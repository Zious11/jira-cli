# Adversarial Review — Phase 1d Pass 11

**Convergence trajectory**: 30 → 15 → 9 → 5 → 10 → 5 → 4 → 3 → 4 → 0 → 2
**Counter regression**: 1/3 → 0/3

## §1: Findings — 2 (0 CRITICAL / 1 HIGH / 1 MEDIUM / 0 LOW)

### HIGH

**ADV-P11-001 — `tracing` falsely claimed as existing dep**
- nfr-catalog.md:90 "`tracing` is already a dep."
- architecture/cross-cutting.md:186 "`tracing` is in `[dependencies]` — but not wired to any subscriber"
- L2 cross-cutting.md:136 (correct) "no tracing/log crate"
- Cargo.toml lines 14-37: `tracing` does NOT appear
- Action: Fix PRD and architecture to match L2 + Cargo.toml ground truth

### MEDIUM

**ADV-P11-002 — Cache count "7 distinct" vs actual 6**
- L2 state-machines.md:300 "7 distinct; 5 Expiring + 2 keyed-map"
- architecture/state-machines.md:269 header "Cache types (7 distinct)" but table has 6 rows
- Source cache.rs: 6 cache files. object_type_attrs is BOTH Expiring AND keyed-map (hybrid). Actual breakdown: 4 pure-Expiring + 1 pure keyed-map + 1 hybrid = 6.
- Action: Update L2 + arch to "6 distinct"

## §2: Routing
product-owner: ADV-P11-001 (PRD nfr-catalog), ADV-P11-002 (L2 state-machines)
architect: ADV-P11-001 (arch cross-cutting), ADV-P11-002 (arch state-machines)

## §3: Verdict — FINDINGS (2)

Counter regresses 1/3 → 0/3. Pass 10's clean on count/anchor lenses didn't catch dep-fact contradiction or cache-count semantic.

Phase 1d adversary Pass 11 complete.
