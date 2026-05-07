---
document_type: adversarial-review
phase: phase-1-spec-adversarial
pass: 21
producer: adversary
timestamp: 2026-05-04T00:00:00Z
fresh_context: true
inputs_reviewed:
  - .factory/specs/prd/*.md
  - .factory/specs/domain-spec/*.md
  - .factory/architecture/*.md
  - src/cli/board.rs, src/cli/sprint.rs (Lens 5)
finding_count: 4
severity_distribution: "0C/1H/2M/1L"
final_assessment: "SUBSTANTIVE"
---

# Phase 1 Spec Adversarial Review — Pass 21

## Final Assessment
SUBSTANTIVE

Counter regress: 0/3 → 0/3. Pass 20 fixes verified 3/3 clean. 4 new findings.

## Pass 20 Fix Verification
- ADV-P20-001 (G-EO1 dep statement "tracing crate not present in Cargo.toml"): VERIFIED at edge-case-catalog.md:305
- ADV-P20-002 (G-EO1 "at 2 sites"): VERIFIED at edge-case-catalog.md:305
- ADV-P20-003 (EC-CFG-005 + EC-ASSET-006 holdout cross-refs): VERIFIED at edge-case-catalog.md:108, :205

## Findings

### ADV-P21-001: BC-7 cumulative count arithmetic — sub-section 7.2 claims 54 BCs but sum forces 83 ≠ frontmatter total 80
- Severity: HIGH
- Confidence: HIGH
- Lens: 2 (NFR/Holdout count audit) + cross-doc count math
- Locations:
  - bc-7-output-render.md:4 — `total_bcs: 80`
  - bc-7-output-render.md:73 — "### 7.2 ADF Rendering (54 contracts)"
  - BC-INDEX.md:447 — "### 7.2 ADF Rendering ... 54 BCs cumulative"
  - BC-INDEX.md:456 — "BC-7.2.006..054 | range-collapsed"
  - CANONICAL-COUNTS.md:50 — "bc-7-output-render.md | 80"
- Evidence: 5+54+9+12+3 = 83 ≠ 80. Definitional 5+5+9+12+3 = 34 ✓. To force 80 cumulative, BC-7.2 should claim 51 (range BC-7.2.006..051).
- Expected: Reconcile arithmetic. Grand-total 541 cross-referenced in many places — preserve 541 by shrinking BC-7.2 to 51.
- Suggested fix: Change "54 contracts" → "51 contracts"; range "BC-7.2.006..054" → "BC-7.2.006..051" in both bc-7-output-render.md and BC-INDEX.md.
- Tag: [content-defect]
- Routing: product-owner

### ADV-P21-002: component-graph.md missing nodes for cli/board.rs and cli/sprint.rs
- Severity: MEDIUM
- Confidence: HIGH
- Lens: 5 (component-graph nodes vs BC-to-Module map)
- Locations:
  - architecture/README.md:89 — BC-5.* maps cli/board.rs, cli/sprint.rs
  - architecture/component-graph.md:11-114 — Mermaid graph has cli_auth, cli_assets, cli_issue subgraph but NO cli_board / cli_sprint
- Evidence: P18 fix added cli/assets.rs as a node but sibling L2 handlers (board, sprint) were not added. Their downstream (api/jira/boards.rs, api/jira/sprints.rs) appear in api_jira subgraph but upstream callers are missing.
- Expected: cli_board and cli_sprint nodes with edges to api_jira (boards_impl, sprints_impl).
- Suggested fix: Add cli_board["cli/board.rs\n<LOC>"] and cli_sprint["cli/sprint.rs\n<LOC>"] mermaid nodes parallel to cli_auth/cli_assets.
- Tag: [content-defect]
- Routing: product-owner

### ADV-P21-003: EC-AUTH-009 anchor mis-target — BC and holdout describe different behaviors
- Severity: MEDIUM
- Confidence: HIGH
- Lens: 1 (EC catalog semantic anchor verification)
- Locations:
  - edge-case-catalog.md:78 — `Status: Covered by BC-1.6.044; holdout H-012`
  - BC-INDEX.md:122 — BC-1.6.042 = basic InsufficientScope detection
  - BC-INDEX.md:124 — BC-1.6.044 = case-insensitive scope match
  - holdout-scenarios.md:131 — H-012 BC refs: BC-1.6.042, BC-X.3.005 (basic detection)
  - holdout-scenarios.md:223 — H-022 BC refs: BC-1.6.043,044,045 (case-sensitivity)
- Evidence: EC-AUTH-009 prose ("exact substring") describes basic detection. BC anchor (1.6.044) is case-insensitivity. Holdout (H-012) is basic detection. Anchors mismatch.
- Expected: Anchor should be BC-1.6.042 (matches H-012 + prose intent).
- Suggested fix: Change "Covered by BC-1.6.044" → "Covered by BC-1.6.042".
- Tag: [content-defect]
- Routing: product-owner

### ADV-P21-004: 6 non-MUST-FIX ECs miss extant holdout citations (P20-003 propagation continuation)
- Severity: LOW
- Confidence: HIGH
- Lens: 1 (EC catalog completeness)
- Locations:
  - EC-ASSET-003 (line ~189): missing H-038
  - EC-ASSET-004 (line ~194): missing H-038
  - EC-ASSET-007 (line ~209): missing H-037
  - EC-SPRINT-001 (line ~219): missing H-042
  - EC-SPRINT-004 (line ~234): missing H-040
  - EC-SPRINT-005 (line ~239): missing H-041
  - EC-OUT-001 (line ~248): missing H-043
- Evidence: P20-003 fixed MUST-FIX ECs only. Non-MUST-FIX ECs with extant anchoring holdouts also follow canonical "Covered by BC; holdout H-NNN" pattern but were not propagated.
- Expected: Each EC should append "; holdout H-NNN".
- Suggested fix: Append holdout citations as listed above.
- Tag: [content-defect]
- Routing: product-owner

## Lens Coverage Summary
- Lens 1 (EC catalog sweep): 2 findings (ADV-P21-003, ADV-P21-004)
- Lens 2 (NFR count audit): 0 findings — 41 NFRs verified
- Lens 3 (Holdout count audit): 0 findings — 48 holdouts verified
- Lens 4 (cross-cutting subdomain): 0 findings
- Lens 5 (component-graph completeness): 1 finding (ADV-P21-002)
- Lens 6 (L2 ↔ L3 parity): 0 findings
- Lens 7 (P20 fix verification): 0 findings — all clean
- BC-7 count math: 1 finding (ADV-P21-001)

## Verdict
SUBSTANTIVE (4 findings). Trajectory 30→15→9→5→10→5→4→3→4→0→2→0→3→0→2→0→3→0→3→5→3→4. Asymptotic regime persists.
