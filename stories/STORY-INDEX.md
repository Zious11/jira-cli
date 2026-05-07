---
document_type: story-index
phase: phase-2-story-decomposition
producer: story-writer
version: "1.1.0"
total_stories: 15
total_waves: 4
status: wave-1-added
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

## Wave 1 — High Priority Infrastructure (Added 2026-05-06)

Wave 1 covers HIGH-priority security posture, supply-chain hardening, structured logging,
and critical regression-pinning holdouts. All stories are independent of each other
(except S-1.03 depends on S-0.06) and can be implemented in parallel groups.

Parallel group A: S-1.01, S-1.02, S-1.04, S-1.05 (CI/CD hardening, no code deps)
Parallel group B: S-1.06, S-1.07, S-1.08 (holdout test suites, each independent)
Sequential: S-1.03 after S-0.06 merges (tracing depends on --verbose-bodies flag)

| Story ID | Title | NFR/BC Anchors | Holdout Anchors | Status | Est. Effort |
|----------|-------|----------------|-----------------|--------|-------------|
| S-1.01 | Pin GitHub Actions to full commit SHAs | NFR-S-E, R-H6 | — | draft | small |
| S-1.02 | cargo-deny supply chain hardening | NFR-S-F | — | draft | small |
| S-1.03 | Add tracing + wire structured logging | NFR-O-A | — | draft | medium |
| S-1.04 | Add timeout-minutes to all CI/CD jobs | R-L12 | — | draft | xsmall |
| S-1.05 | GitHub secret scanning + gitleaks CI | NFR-S-B, R-L13 | — | draft | small |
| S-1.06 | OAuth flow holdout suite | BC-1.1.001, BC-1.1.002 | H-001..H-008, H-022, H-029 | draft | medium |
| S-1.07 | Rate-limit holdout suite | BC-X.1.005, BC-X.4.002 | H-013, H-027 | draft | small |
| S-1.08 | Keychain per-profile layout holdout | BC-1.4.027, BC-1.4.025 | H-016 | draft | small |

Wave 1 story files: `stories/wave-1/S-1.NN-*.md`

### Wave 1 exit gate

All of the following must be true before Wave 2 dispatch:
- H-001, H-002, H-003, H-004, H-005, H-022, H-029 MUST-PASS (S-1.06 test suite green)
- H-013, H-027 MUST-PASS (S-1.07 test suite green)
- H-016 MUST-PASS (S-1.08 test suite green)
- All Wave 0 holdouts remain green (no regression)
- NFR-S-E: no floating action tags in `.github/workflows/` (S-1.01)
- NFR-S-F: `cargo deny check bans` exits 0 (S-1.02)
- NFR-S-B: gitleaks CI job passes (S-1.05)
- S-1.03 (tracing): `cargo test --all-features` green; verbose behavior unchanged

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
