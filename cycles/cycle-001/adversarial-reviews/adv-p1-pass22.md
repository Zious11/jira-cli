---
document_type: adversarial-review
phase: phase-1-spec-adversarial
pass: 22
producer: adversary
timestamp: 2026-05-04T00:00:00Z
fresh_context: true
inputs_reviewed:
  - .factory/specs/prd/*.md
  - .factory/specs/domain-spec/*.md
  - .factory/architecture/*.md
  - src/api/client.rs (Lens 2 for H-027)
finding_count: 5
severity_distribution: "0C/0H/4M/1L"
final_assessment: "SUBSTANTIVE"
---

# Phase 1 Spec Adversarial Review — Pass 22

## Final Assessment
SUBSTANTIVE

Counter regress: 0/3 → 0/3. Pass 21 fixes 4/4 verified at primary targets. 5 new findings — 3 are Pass 21 propagation gaps surfaced by lens rotation.

## Pass 21 Fix Verification
- ADV-P21-001 (BC-7.2 count 54→51): VERIFIED at bc-7-output-render.md:73, BC-INDEX.md:447,456. Math 5+51+9+12+3=80 ✓
- ADV-P21-002 (cli_board/cli_sprint mermaid nodes): VERIFIED at component-graph.md:52-53
- ADV-P21-003 (EC-AUTH-009 anchor BC-1.6.044→BC-1.6.042): VERIFIED at edge-case-catalog.md:78
- ADV-P21-004 (6 EC holdout citations): VERIFIED at edge-case-catalog.md:190,195,210,219,234,239,248

## Findings

### ADV-P22-001: H-044 stale BC range BC-7.2.001..BC-7.2.054 (P21 propagation gap)
- Severity: MEDIUM
- Lens: 2 (holdout round-trip) + P21 fix propagation
- Locations: holdout-scenarios.md:436
- Evidence: P21 reduced BC-7.2 cumulative max from 054 to 051. H-044 still cites old range.
- Suggested fix: `BC-7.2.001..BC-7.2.054` → `BC-7.2.001..BC-7.2.051`
- Tag: [content-defect]
- Routing: product-owner

### ADV-P22-002: L2 bc-07-output-render.md two stale "54 BCs" refs (P21 propagation gap)
- Severity: MEDIUM
- Lens: 1 (README synthesis) + P21 propagation
- Locations: bc-07-output-render.md:15 and :127 — both say "54 BCs"
- Evidence: L3 PRD reduced ADF count to 51. L2 not updated.
- Suggested fix: Both occurrences `54 BCs` → `51 BCs`
- Tag: [content-defect]
- Routing: product-owner

### ADV-P22-003: component-graph.md Mermaid says "extract_error_message 6-level chain" — should be 7-level
- Severity: MEDIUM
- Lens: 3 (Mermaid render correctness)
- Locations:
  - architecture/component-graph.md:133 — Mermaid label says "6-level"
  - architecture/cross-cutting.md:32 — "7-level precedence chain"
  - bc-7-output-render.md:124 — "7-step precedence chain"
- Evidence: ADV-P2-001 corrected count from 6 to 7 in PRD/architecture cross-cutting; component-graph mermaid label was missed.
- Suggested fix: `extract_error_message 6-level chain` → `extract_error_message 7-level chain`
- Tag: [content-defect]
- Routing: architect

### ADV-P22-004: H-027 internal contradiction — Retry-After:86400 + Mock expect=2 + 5s window inconsistent
- Severity: MEDIUM
- Lens: 2 (holdout description ↔ setup ↔ verification)
- Locations: holdout-scenarios.md:269-274
- Evidence: With Retry-After=86400 and 5s window, only 1 call fires; Mock::expect(2) would fail at teardown. Expected stderr "warning: rate limited by Jira" only fires after MAX_RETRIES=3 exhaustion (~72h sleep). Cannot fire in 5s window.
- Suggested fix: Reframe H-027 as unit test on `RateLimitInfo::from_headers` parsing the literal 86400 value (no actual retry loop) — proves "no upper bound applied" without needing real-time clock. Update setup, action, expected accordingly.
- Tag: [content-defect]
- Routing: product-owner

### ADV-P22-005: CANONICAL-COUNTS.md MEDIUM list contradicts count and nfr-catalog.md
- Severity: LOW
- Lens: 6 (CANONICAL-COUNTS internal consistency)
- Locations:
  - CANONICAL-COUNTS.md:103 — MEDIUM list has 17 items but says count 15; includes NFR-R-NEW-1, NFR-R-NEW-2
  - nfr-catalog.md:54-55 — both NFR-R-NEW-1, NFR-R-NEW-2 listed as LOW
- Evidence: NFR-R-NEW-1 and NFR-R-NEW-2 are LOW per nfr-catalog. Count of 15 is correct; list has 17 items.
- Suggested fix: Remove "NFR-R-NEW-1, NFR-R-NEW-2, " from CANONICAL-COUNTS.md:103 MEDIUM list.
- Tag: [content-defect]
- Routing: product-owner

## Observations
- ADV-P22-OBS-001: component-graph.md:52-53 has potential mermaid forward-reference fragility for boards_impl/sprints_impl
- ADV-P22-OBS-002: L2 bc-07-output-render.md:35 describes 6-level chain (matches stale source doc-comment, may be intentional faithfulness)
- ADV-P22-OBS-003: H-044 propagation gap demonstrates P21 fix verification was scoped too narrowly — recommend follow-up grep sweep for `BC-7\.2\.0[5-9][0-9]` references [process-gap candidate]

## Lens Coverage Summary
- Lens 1 (README synthesis vs detail): 1 finding (ADV-P22-002)
- Lens 2 (holdout round-trip): 2 findings (ADV-P22-001, ADV-P22-004)
- Lens 3 (Mermaid render): 1 finding (ADV-P22-003)
- Lens 4 (frontmatter version sync): 0 findings
- Lens 5 (NFR ↔ holdout coverage): 0 findings
- Lens 6 (CANONICAL-COUNTS internal): 1 finding (ADV-P22-005)
- Lens 7 (P21 fix verification): 0 findings at primary targets; 3 propagation gaps caught via Lens 1, 2

## Verdict
SUBSTANTIVE (5 findings + 3 observations). Trajectory ...→3→5→3→4→5. Asymptotic regime persists. P21 propagation gaps demonstrate need for downstream-grep sweep on every count-changing fix.
