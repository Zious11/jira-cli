---
document_type: adversarial-review
phase: phase-2-adv-story-corpus
pass: 1
producer: adversary
timestamp: 2026-05-04T00:00:00Z
fresh_context: true
inputs_reviewed:
  - .factory/stories/STORY-INDEX.md
  - .factory/stories/WAVE-PLAN.md
  - .factory/stories/wave-0/*.md (7 stories)
  - .factory/stories/wave-1/*.md (8 stories)
  - .factory/stories/wave-2/*.md (7 stories)
  - .factory/stories/wave-3/*.md (8 stories — pre-fix)
  - .factory/specs/prd/*.md (referential)
  - .factory/architecture/*.md (referential)
  - src/{api,cli}/auth.rs LOC verification
finding_count: 14
severity_distribution: "2C/5H/5M/2L"
final_assessment: "SUBSTANTIVE"
---

# Phase 2 Story Adversarial Review — Pass 1

## Final Assessment
SUBSTANTIVE — 14 findings, all FIXED in subsequent burst.

## Findings Summary
- ADV-P2.S1-001 (CRITICAL): S-3.01 anchored wrong file — `src/api/auth.rs` (1,397) vs `src/cli/auth.rs` (1,998). NFR-O-D + R-M6 confirm cli/auth.rs.
- ADV-P2.S1-002 (CRITICAL): S-1.06 title/STORY-INDEX claim H-001..H-008 but frontmatter only H-001..H-006; H-007/H-008 in S-2.02.
- ADV-P2.S1-003 (HIGH): H-021 unanchored; S-2.02 falsely cites S-1.06.
- ADV-P2.S1-004 (HIGH): 15 holdouts unanchored (H-009..H-047 various) — needs anchor or pre-existing-test appendix.
- ADV-P2.S1-005 (HIGH): NFR-S-A (PKCE) promised in WAVE-PLAN as Wave 3 story; no story exists.
- ADV-P2.S1-006 (MED): S-0.05 BC-X.1.001 quote may be paraphrase.
- ADV-P2.S1-007 (MED): S-3.03 contradicts CLAUDE.md `refresh_oauth_token` signature gotcha.
- ADV-P2.S1-008 (MED): `risk_anchors:` non-canonical frontmatter field.
- ADV-P2.S1-009 (MED): S-3.04 implements H-047 fix but holdout_anchors empty.
- ADV-P2.S1-010 (LOW): S-2.03 precautionary cross-wave dependency.
- ADV-P2.S1-011 (LOW): S-1.03 priority MEDIUM but Wave 1 = HIGH.
- ADV-P2.S1-012 (MED): S-3.07 bundles 4 unrelated NFRs (sizing).
- ADV-P2.S1-013 (MED): S-3.04 BC mis-anchor (BC-1.1.007 vs BC-1.5.038).
- ADV-P2.S1-014 (LOW): NFR-R-NEW-1 anchored in S-1.07 (pin) AND S-3.07 (implement).

## Resolution

All 14 findings FIXED. New story S-3.09 added (PKCE-deferral). STORY-INDEX bumped to v1.4.0, total_stories 30 → 31. Pre-existing-Test-Coverage appendix added. Gap Register added for 10 holdouts deferred to v0.6 (GAP-H-001..010).

## Lens Coverage Summary
- Lens 1 (AC traceability): 3 findings
- Lens 2 (Story sizing): 1 finding
- Lens 3 (MUST-FIX coverage): 0 findings (all 4 anchored correctly)
- Lens 4 (Frontmatter completeness): 3 findings
- Lens 5 (Wave ordering): 2 findings
- Lens 6 (Files-modified): 2 findings
- Lens 7 (STORY-INDEX completeness): 0 (parity OK)
- Lens 8 (Breaking change): 0 (correctly flagged)
- Lens 9 (NFR coverage): 1 finding
- Lens 10 (Holdout coverage walk): 2 findings

## Verdict
SUBSTANTIVE. Counter 0/3. Pass 2 next.
