# Adversarial Review — Phase 1d Pass 9

**Convergence trajectory**: 30 → 15 → 9 → 5 → 10 → 5 → 4 → 3 → 4

## §1: Findings — 4 (0 CRITICAL / 0 HIGH / 4 MEDIUM / 0 LOW)

### MEDIUM

**ADV-P9-001 — risk-register Risk Summary action-breakdown mismatch**
Hand-count of body entries vs Summary claims at risk-register.md:71-73:
- HIGH (6): actual 5×FIX, 1×SEC-DECIDE; summary claims 4×FIX, 2×SEC-DECIDE
- MEDIUM (8): actual 4×DEFER, 1×DOCUMENT-AS-IS, 1×FIX, 2×SEC-DECIDE; summary 3×DEFER, 2×DOCUMENT-AS-IS, 1×FIX, 2×SEC-DECIDE
- LOW (11): actual 8×DOCUMENT-AS-IS, 2×DEFER, 1×POLICY-DECISION; summary 7×DOCUMENT-AS-IS, 3×DEFER, 1×POLICY-DECISION
- Action: recount each row and replace.

**ADV-P9-002 — NFR-S-F site path drift**
nfr-catalog.md:66 site says `.cargo/deny.toml`. Actual file is `deny.toml` at project root.
- Action: Change site to `deny.toml, .github/workflows/ci.yml`.

**ADV-P9-003 — NFR-S-F cross-ref to R-H6 wrong (should R-H5)**
risk-register.md:28-29: R-H5 = NFR-S-F (supply-chain); R-H6 = NFR-S-E (SHA pinning). nfr-catalog.md:66 cites R-H6 — wrong row.
- Action: Change R-H6 → R-H5 in both NFR-S-F references.

**ADV-P9-004 — MatchResult::Ambiguous description in arch contradicts source**
arch cross-cutting.md:148: "multiple items contain the needle substring". Source partial_match.rs:39-42 + test `test_partial_match_single_substring_is_ambiguous`: SINGLE substring match also returns Ambiguous (fail-closed).
- Action: Replace with "one or more items contain the needle substring (single substring hit is also Ambiguous — fail-closed). See PRD error-taxonomy.md §5."

## §2: Strengths

1. NFR routing arithmetic now reconciles (10+3+3+13+12=41) — ADV-P8-001 closed
2. BC-3.4.001 + BC-X.5.002 source-line evidence verified exact
3. ADR cross-references (0007..0012) all resolve to MUST-FIX BC anchors

## §3: Routing
product-owner: ADV-P9-002, ADV-P9-003
architect: ADV-P9-001, ADV-P9-004

## §4: Verdict — FINDINGS (4)

Counter 0/3. Trajectory plateau in 3-5 range. Drift concentrates in summary arithmetic and small cross-doc anchors.

## §5: Convergence note

Findings real, non-overlapping with prior passes, but small-blast-radius. Deeper BC bodies and severity totals stable.

Phase 1d adversary Pass 9 complete.
