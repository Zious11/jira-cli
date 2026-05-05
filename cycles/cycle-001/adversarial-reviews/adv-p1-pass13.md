# Adversarial Review — Phase 1d Pass 13

**Convergence trajectory**: 30 → 15 → 9 → 5 → 10 → 5 → 4 → 3 → 4 → 0 → 2 → 0 → 3
**Counter regression**: 1/3 → 0/3

## §1: Findings — 3 (0 CRITICAL / 0 HIGH / 3 MEDIUM / 0 LOW)

### MEDIUM

**ADV-P13-001 — BC grand total off-by-one**
PRD README:35-44 Document Map sums 57+91+77+32+35+39+80+130 = 541. README:51 + BC-INDEX:4 claim 542. Off-by-one arithmetic divergence.
- Root cause: BC-X.4.009 already counted inside cross-cutting=130 (Pass 10 fix); BC-INDEX:648 erroneously claimed "+1 NEW = 542" as a second count.
- Action: Fix to 541 across PRD README + BC-INDEX.

**ADV-P13-002 — NFR-O-G stale LOC**
nfr-catalog.md:107 NFR-O-G says cli/issue/list.rs is 970 LOC. Actual = 1,083 (verified by wc -l). component-graph.md, risk-register R-M5, docs/specs/list-rs-split.md all correctly say 1,083.
- Action: Update NFR-O-G to "1,083 LOC".

**ADV-P13-003 — Dangling cicd-setup.md path refs in arch**
risk-register.md:29 R-H6 references "cicd-setup.md GAP-1" but file lives at .factory/cicd-setup.md (root, not arch/). Relative path doesn't resolve.
- Action: Update path to ../cicd-setup.md OR add cicd-setup.md to arch README Document Map.

## §2: Verified clean lenses

- ADR-0006 alignment with code
- L2 ↔ L3 entity coverage (7 BCs match)
- Holdouts H-001..H-005, H-018, H-NEW-MP-001 (sampled): Setup/Action/Expected concrete
- Error taxonomy 11 JrError variants × exit codes consistent
- State machine BC anchors (SM-1, SM-3, SM-5)
- Risk-register summary arithmetic 1+6+8+11=26
- Pass 11/12 fixes still propagated (tracing absent, cache=6)
- MUST-FIX register integrity (4 BCs across 5+ docs)

## §3: Routing
product-owner: ADV-P13-001, ADV-P13-002
architect: ADV-P13-003

## §4: Verdict — FINDINGS (3 MEDIUM)

Counter regress 1/3 → 0/3. Pass 13 broke convergence streak. Novelty MEDIUM — three new content-drift items. None nitpicks.

Phase 1d adversary Pass 13 complete.
