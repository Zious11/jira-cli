---
document_type: story-index
phase: phase-2-story-decomposition
producer: story-writer
version: "1.0.0"
total_stories: 7
total_waves: 4
status: wave-0-active
last_updated: 2026-05-06
activation_head: dea1664
---

# Story Index — jira-cli (jr)

Phase 2 Story Decomposition. Activation HEAD: dea1664 (v0.5.0-dev.7).
Phase 1 converged at adversary Pass 28. Gate approved 2026-05-04.

---

## Wave Plan

| Wave | Theme | Story count | Estimated effort | Gate |
|------|-------|-------------|------------------|------|
| 0 | MUST-FIX bugs + SD-002/SD-003 security + H-NEW-AUTH-002 holdout | 7 | ~5-6 dev-days | All H-MUST-FAIL holdouts become MUST-PASS; no regression on H-001..H-047 |
| 1 | High-priority feature delivery (HIGH NFR-anchored BCs, P0/P1 features) | TBD (next burst) | TBD | NFR-S-B/C gate; wave-0 holdouts green |
| 2 | Medium-priority features (MEDIUM NFRs, issue-write/assets improvements) | TBD | TBD | NFR-P-* gate |
| 3 | Low priority + deferred (DEFER NFRs, MEDIUM/LOW ECs, PKCE per ADR-0013) | TBD | TBD | Per-story gates; no v0.5 blocking |

Story file naming: `stories/wave-W/S-W.NN-short-slug.md`
Story ID convention: `S-W.NN` (e.g., `S-0.01`, `S-1.03`)

---

## Wave 0 — MUST-FIX + Security (Active)

All Wave 0 stories are CRITICAL or HIGH priority. No v0.5 release without green on all Wave 0 holdouts.

| Story ID | Title | BC Anchors | Holdout Anchors | Status | Est. Effort |
|----------|-------|------------|-----------------|--------|-------------|
| S-0.01 | Fix `handle_open` OAuth instance URL | BC-3.4.001 | H-046 | draft | small |
| S-0.02 | Paginate `list_worklogs` | BC-X.5.002 | H-045 | draft | small |
| S-0.03 | Multi-workspace asset HashMap composite key | BC-4.3.001 | H-036 | draft | small |
| S-0.04 | Multi-profile fields active-profile migration | BC-6.3.001 | H-NEW-MP-001 | draft | medium |
| S-0.05 | Gate `JR_AUTH_HEADER` behind `#[cfg(test)]` | SD-002 / NFR-S-B | H-NEW-AUTH-002 | draft | small |
| S-0.06 | Add `--verbose-bodies` flag + PII warning | SD-003 / NFR-S-C | (new holdout per SD-003) | draft | small |
| S-0.07 | Formalize holdout H-NEW-AUTH-002 in spec | SD-002 (docs) | H-NEW-AUTH-002 | draft | xsmall |

Wave 0 story files: `stories/wave-0/S-0.NN-*.md`

---

## Wave 1 — High Priority Features (Pending — next burst)

Placeholder. Will be populated in Phase 2 Burst 2 by walking BC-INDEX for HIGH-NFR-anchored BCs and P0/P1 capability gaps.

| Story ID | Title | BC Anchors | Status |
|----------|-------|------------|--------|
| TBD | ... | ... | placeholder |

---

## Wave 2 — Medium Priority (Pending)

Placeholder. MEDIUM NFRs, issue-write improvements, assets improvements.

---

## Wave 3 — Low Priority / Deferred (Pending)

Placeholder. DEFER NFRs (e.g., NFR-O-S multi-site OAuth per H-047), PKCE per ADR-0013, MEDIUM/LOW EC entries.

---

## Cross-Reference Convention

Each story frontmatter uses:
- `bc_anchors:` — list of BC-S.SS.NNN IDs this story implements
- `holdout_anchors:` — list of H-NNN IDs (MUST-FAIL pre-fix, MUST-PASS post-fix)
- `nfr_anchors:` — NFR IDs this story satisfies
- `adr_refs:` — ADR IDs that constrain this story
- `sd_refs:` — Security Decision IDs (if applicable)
- `files_modified:` — source files touched (with line ranges)
- `test_files:` — test files to create or modify
