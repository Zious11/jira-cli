---
document_type: adversarial-review
phase: phase-2-adv-story-corpus
pass: 8
producer: adversary
timestamp: 2026-05-04T00:00:00Z
fresh_context: true
finding_count: 4
severity_distribution: "0C/1H/1M/2L"
final_assessment: "SUBSTANTIVE"
---

# Phase 2 Story Adversarial Review — Pass 8

## Final Assessment
SUBSTANTIVE — 4 findings. Trajectory 14→5→5→5→4→5→4→4. Counter 0/3.

## Pass 7 Fix Verification — 4/4 CLEAN

## Findings

### ADV-P2-S8-001 (HIGH): STORY-INDEX:195 H-009 row mis-anchors BC-X.8.001
- Severity: HIGH (sibling-sweep miss from Pass 2 fix family)
- Locations: STORY-INDEX:195, holdout-scenarios.md:108 (H-009 BC ref = BC-2.3.035), cross-cutting.md:459 (BC-X.8.001 = project_exists, unrelated)
- Evidence: H-009 description says "corrupt teams.json graceful degrade" but BC-X.8.001 is `project_exists(key)`. Actual BC = BC-2.3.035 per holdout-scenarios.md.
- Suggested fix: STORY-INDEX:195 H-009 row + STORY-INDEX:215 GAP-H-001 row: BC-X.8.001 → BC-2.3.035
- Routing: story-writer

### ADV-P2-S8-002 (MED): S-1.05 anchors NFR-S-B but doesn't address it
- Severity: MEDIUM
- Locations: S-1.05 frontmatter line 12-13, STORY-INDEX:71
- Evidence: NFR-S-B = JR_AUTH_HEADER #[cfg(test)] gate (owned by S-0.05). S-1.05 = secret-scanning. Mentions JR_AUTH_HEADER rhetorically but doesn't resolve NFR-S-B. Risk-register.md:64 maps R-L13 → NFR-S-E (CI/CD config), not NFR-S-B.
- Suggested fix: Replace `nfr_anchors: NFR-S-B` with `NFR-S-E`. Update STORY-INDEX:71 to match.
- Routing: story-writer

### ADV-P2-S8-003 (LOW): H-NEW-AUTH-002 still absent from holdout-scenarios.md
- Severity: LOW (Pass 7 OBS-2 carry-forward, normalized)
- Locations: holdout-scenarios.md (frontmatter total_holdouts:48, no H-NEW-AUTH-002 entry)
- Evidence: Wave 0 exit gate (WAVE-PLAN:36) names H-NEW-AUTH-002 MUST-PASS. S-0.07 will add it during implementation.
- Suggested fix: Annotate holdout-scenarios.md frontmatter that H-NEW-AUTH-002 will be added by S-0.07 implementation.
- Routing: story-writer

### ADV-P2-S8-004 (LOW): H-NEW-MP-001 structural format divergent
- Severity: LOW
- Locations: holdout-scenarios.md:470-491 vs. H-001..H-047 dominant format
- Evidence: H-NEW-MP-001 places `**NFR source**` and `**BC**` at TOP, uses numbered Setup, adds `**Status**` and `**Verification**` fields not present in H-001..H-047.
- Suggested fix: Re-format H-NEW-MP-001 to match dominant H-001..H-047 shape, OR document the dual shape as deliberate in file preamble.
- Routing: story-writer

## Observations
- OBS-1 [process-gap]: S-3.07 still bundles 4 unrelated parts (carry-forward)
- OBS-2 [process-gap]: Pre-existing Test Coverage + Gap Register need full sibling-sweep audit (orchestrator should proactively audit all 22 rows)
- OBS-3 [process-gap]: Adversary has no filesystem-listing tool — recommend STORY-INDEX add story-filename → story-id manifest

## Verdict
SUBSTANTIVE. Trajectory 14→5→5→5→4→5→4→4. Counter 0/3. Pass 9 next.
