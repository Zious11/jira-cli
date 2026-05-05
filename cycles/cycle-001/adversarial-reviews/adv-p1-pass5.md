# Adversarial Review — Phase 1d Pass 5

**Reviewer**: adversary (fresh-context)
**Date**: 2026-05-04
**Convergence trajectory**: 30 → 15 → 9 → 5 → 10 (REGRESSION)

## §1: Findings — 10 (2 CRITICAL / 3 HIGH / 4 MEDIUM / 1 LOW)

### CRITICAL

**ADV-P5-001 — extract_error_message chain depth contradicts canonical [HIGH confidence]**
PRD README:106 "6-level"; canonical (error-taxonomy.md §2 + arch cross-cutting.md) "7-Step Precedence Chain".

**ADV-P5-002 — EC-OUT-005 stale empty-body precedence text not propagated [HIGH]**
edge-case-catalog.md:265-268 EC-OUT-005 says "Returns None → caller uses status-code message"; canonical post-ADV-P2-001: empty-body returns literal "<empty response body>".

### HIGH

**ADV-P5-003 — BC-6.3.001 internal contradiction "11" vs "14" read sites [HIGH]**
bc-6-config-cache.md:304 says "11 read sites"; same BC line 298, 300, 321 + ADR-0007 + R-C1 + NFR-R-D + BC-INDEX MUST-FIX register all say "14".

**ADV-P5-004 — bc-6 frontmatter ↔ body BC count mismatch [HIGH]**
bc-6-config-cache.md:4 frontmatter total_bcs:39; line 18 body says "38 behavioral contracts".

**ADV-P5-005 — PRD README Competitive Differentiators table 4 mis-anchors [HIGH]**
- Multi-profile → BC-X.5.* (Worklogs, wrong)
- Embedded OAuth → BC-X.9.001 (JQL escape, wrong)
- Non-interactive → BC-X.4.001 (rate limit, wrong)
- Actionable errors → BC-X.3.012 (does not exist)

### MEDIUM

**ADV-P5-006** — EC-OUT-007 mis-anchors BC-7.4.012; should be BC-7.3.005
**ADV-P5-007** — Two competing accountings of "542 BC total" between PRD README and BC-INDEX
**ADV-P5-008** — bc-7 frontmatter definitional_count: 33 vs BC-INDEX: 34
**ADV-P5-009** — NFR-R-NEW-1 routing: DOCUMENT-AS-IS in catalog vs FIX-IN-PHASE-3 in BC-X.4.009

### LOW

**ADV-P5-010** — dtu-assessment.md:170 "all 14 bounded context response types"; should be 7

## §2: Strengths

1. State-machine and ADR pinning consistent (port 53682, ADR-0006 callback URL)
2. MUST-FIX register propagation solid for 4 NFRs (only inline "11 read sites" leak in BC-6.3.001 mars this)
3. extract_error_message single-source-of-truth working (arch defers to PRD; both byte-identical)

## §3: Routing

product-owner: 9 (P5-001..009)
architect: 1 (P5-010)

## §4: Verdict — FINDINGS

10 substantive (2 CRIT/3 HIGH/4 MED/1 LOW). REGRESSION from Pass 4. Cause: anchor tables in supplements (Competitive Differentiators, edge-case-catalog) not subjected to same audit as BC bodies.

## §5: Follow-ups [process-gap]

- Adversary checklist must include cross-reference tables in index/README files
- After CONV-ABS or ADV-Pn correction, scan supplements/ for affected concept

Phase 1d adversary Pass 5 complete.
