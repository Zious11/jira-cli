---
gate: phase-2-to-3
date: 2026-05-07
phase_completed: phase-2-story-decomposition
phase_pending: phase-3-tdd-implementation
verdict: approved
approved_by: human (user)
approved_at: 2026-05-07
---

# Phase 2 → Phase 3 Human Approval Gate

## Section 1: Executive Summary

Phase 2 (Story Decomposition) produced a complete, convergence-verified story corpus for jira-cli. 31 stories across 4 waves (W0:7 + W1:8 + W2:7 + W3:9) were created, cross-linked to 541 BCs, 48 holdout scenarios, and 3 resolved security decisions. The stories went through 13 passes of adversarial review (Phase 2-adv) with rotating lens axes covering BC-anchor correctness, frontmatter ↔ body ↔ WAVE-PLAN sibling consistency, and coverage completeness.

**Convergence statistics:**
- Phase 1d (spec review): 28 passes, 25 SUBSTANTIVE → 3/3 CLEAN at P26-P27-P28. Trajectory: `30→15→9→5→10→5→4→3→4→0→2→0→3→0→2→0→3→0→3→5→3→4→5→5→5→2→0→0→0`.
- Phase 2-adv (story review): 13 passes, 10 SUBSTANTIVE → 3/3 CLEAN at P11-P12-P13. Trajectory: `14→5→5→5→4→5→4→4→4→1→0→1→0`.
- Pre-gate consistency audit (fresh-context, perimeter check): PASS-WITH-NITS (96/100); 3 MEDIUM findings (CV2-001, CV2-002, CV2-003) all RESOLVED before this gate.
- Input-hash drift sweep: CLEAN — 0 true drift; 2 sentinel false-positives (live-state, expected); 1 NOINPUT (expected).

The story corpus is ready for Phase 3 TDD implementation, pending human approval.

---

## Section 2: Phase 2 Deliverables Inventory

| Artifact | Path | Version |
|----------|------|---------|
| STORY-INDEX | `.factory/stories/STORY-INDEX.md` | 1.4.2 |
| WAVE-PLAN | `.factory/stories/WAVE-PLAN.md` | 1.1.1 |
| Wave 0 stories (7) | `.factory/stories/wave-0/S-0.01..S-0.07-*.md` | per file |
| Wave 1 stories (8) | `.factory/stories/wave-1/S-1.01..S-1.08-*.md` | per file |
| Wave 2 stories (7) | `.factory/stories/wave-2/S-2.01..S-2.07-*.md` | per file |
| Wave 3 stories (9) | `.factory/stories/wave-3/S-3.01..S-3.09-*.md` | per file |
| Holdout scenarios | `.factory/specs/prd/holdout-scenarios.md` | 1.1.0 |
| SD-001 PKCE deferral | `.factory/architecture/security-decisions/SD-001-pkce.md` | RESOLVED (Option C) |
| SD-002 cfg(debug_assertions) gate | `.factory/architecture/security-decisions/SD-002-jr-auth-header-prod-gating.md` | RESOLVED (Option B-revised, canonized 2026-05-07; was Option A at gate) |
| SD-003 verbose-bodies | `.factory/architecture/security-decisions/SD-003-verbose-pii-redaction.md` | RESOLVED (Option B) |
| ADR-0013 PKCE deferral | `.factory/architecture/adr/0013-pkce-deferral.md` | Accepted |
| Pre-Phase-3 consistency audit | `.factory/cycles/cycle-001/audits/pre-phase-3-gate-consistency-audit.md` | — |
| Pre-Phase-3 input-hash drift | `.factory/cycles/cycle-001/audits/pre-phase-3-gate-input-hash-drift.md` | — |
| Phase 2-adv pass reports (13) | `.factory/cycles/cycle-001/adversarial-reviews/adv-p2-pass{1..13}.md` | — |
| Convergence trajectory (full) | `.factory/cycles/cycle-001/convergence-trajectory.md` | 1.0 |

---

## Section 3: Wave Plan Summary

### Wave 0 — MUST-FIX + Security Decisions (7 stories)

| Story | Title | Type | Notes |
|-------|-------|------|-------|
| S-0.01 | handle_open OAuth fix | MUST-FIX | BC-1.6.040; H-NEW-MP-001 |
| S-0.02 | list_worklogs truncation fix | MUST-FIX | BC-2.5.001 |
| S-0.03 | hardcoded 8h/5d fix | MUST-FIX | BC-2.5.010 |
| S-0.04 | multi-workspace HashMap fix | MUST-FIX | BC-4.1.001 |
| S-0.05 | SD-002 `#[cfg(debug_assertions)]` gate (canonized from `#[cfg(test)]`) | SD implementation | BC-X.7.001; H-NEW-AUTH-002 |
| S-0.06 | SD-003 `--verbose-bodies` | SD implementation | H-NEW-VERBOSE-001/002 |
| S-0.07 | H-NEW-AUTH-002 holdout authoring | Holdout | Authored by S-0.07; owned by S-0.05 |

Wave 0 exit gate: all 4 MUST-FIX bugs green, SD-002 and SD-003 implemented, holdouts registered.

### Wave 1 — HIGH NFR Infra (8 stories)

| Story | Title | Type | Notes |
|-------|-------|------|-------|
| S-1.01 | SHA-pinned CI/CD workflow | NFR-S-E infra | BC-1.4.025 |
| S-1.02 | OAuth token lifecycle holdout suite | Holdout | H-001..H-006 |
| S-1.03 | Rate-limit retry holdout suite | Holdout | H-007..H-012 |
| S-1.04 | Keychain error-path holdout suite | Holdout | H-013..H-019 |
| S-1.05 | CI/CD config NFR-S-E tests | Test infra | NFR-S-E |
| S-1.06 | Regression pin suite (Phase 3 entry gate) | Regression | BC-X.1.001..005 |
| S-1.07 | Multi-profile field isolation regression | Regression | BC-X.1.005 |
| S-1.08 | Retry-After integration tests | Integration | BC-1.4.025 |

### Wave 2 — MEDIUM Holdout Suites (7 stories)

| Story | Title | Type | Notes |
|-------|-------|------|-------|
| S-2.01 | Issue-read holdout suite | Holdout | BC-2.1.001..013 (9 BCs) |
| S-2.02 | Issue-write holdout suite | Holdout | H-021..H-030 |
| S-2.03 | Board/sprint holdout suite | Holdout | H-031..H-040 |
| S-2.04 | Assets/CMDB holdout suite | Holdout | 7 BCs |
| S-2.05 | Config/cache holdout suite | Holdout | NFR-O-R |
| S-2.06 | Worklog + duration + CMDB tuple | Holdout | BC-X.5.009 |
| S-2.07 | CLAUDE.md doc-as-tested | Docs | effort: medium |

### Wave 3 — LOW + DEFER + Spec-Tooling (9 stories)

| Story | Title | Type | Notes |
|-------|-------|------|-------|
| S-3.01 | Output format holdout suite | Holdout | BC-7.*.* |
| S-3.02 | JSM queue holdout suite | Holdout | |
| S-3.03 | User/team holdout suite | Holdout | |
| S-3.04 | JQL/AQL correctness holdout | Holdout | |
| S-3.05 | Error-path taxonomy holdout | Holdout | |
| S-3.06 | Numeric-claim spec checker | Spec-tooling | DRIFT-001 codification |
| S-3.07 | Auth error-path holdout | Holdout | NFR-R-F |
| S-3.08 | Doc-as-is pass | Docs | |
| S-3.09 | ADR-0013 PKCE monitoring task | Process | Added at Pass 1 |

---

## Section 4: Coverage Verification

### MUST-FIX BC Coverage

All 4 MUST-FIX correctness bugs have implementing stories in Wave 0:
- BC-1.6.040 (handle_open OAuth redirect) → S-0.01
- BC-2.5.001 (list_worklogs pagination truncation) → S-0.02
- BC-2.5.010 (hardcoded 8h/5d worklog billing) → S-0.03
- BC-4.1.001 (multi-workspace HashMap type regression) → S-0.04

### Security Decision Coverage

All 3 resolved SDs have an implementing or deferral path:
- **SD-001 PKCE** → deferred via ADR-0013 (no implementation story; reactivation trigger documented: "Atlassian announces public PKCE for 3LO Jira Cloud"). S-3.09 tracks monitoring.
- **SD-002 `#[cfg(debug_assertions)]` gate** (canonized from `#[cfg(test)]` during S-0.05 implementation) → S-0.05 implements; S-0.07 authors the associated holdout.
- **SD-003 `--verbose-bodies`** → S-0.06 implements; H-NEW-VERBOSE-001/002 registered in holdout-scenarios.md (v1.1.0); WAVE-PLAN exit gate references both holdouts.

### Holdout Coverage

All MUST-PASS holdouts (H-001..H-048 canonical + H-NEW-*) have authoring or tracing stories. H-NEW-AUTH-002 (SD-002) is authored by S-0.07. H-NEW-VERBOSE-001/002 (SD-003) are registered in holdout-scenarios.md and traced from S-0.06.

### Story Count Verification

31 story files match STORY-INDEX (v1.4.2), WAVE-PLAN (v1.1.1), and Story Manifest table (added at Pass 13). Count: W0:7 + W1:8 + W2:7 + W3:9 = 31.

---

## Section 5: Structured Questions for Human Review

Per orchestrator operating procedure, explicit questions pointing to assumptions and potential decision points:

### Q1: Scope completeness

Are 31 stories the right scope to enter Phase 3, or do you want to add/cut anything before TDD begins?

- Wave 0 contains 4 MUST-FIX bugs + 2 SD implementations + 1 holdout authoring story. Is there a known bug or security concern we missed?
- Wave 3 contains DEFER/LOW items (S-3.06..S-3.09). Should any of these promote up to Wave 0 or Wave 1 before Phase 3 starts?
- Is the "31 stories across 4 waves" scope aligned with the v0.5 hardening milestone target?

### Q2: Anchor correctness (spot-check prompts)

BCs are anchored to capabilities and to S-0.01..S-3.09. Spot-check suggestions:

- **S-2.04**: BC anchor list expanded from 3 to 7 during CV2-002 fix (consistency audit). The story body had 7 BCs but STORY-INDEX showed 3. Now aligned at 7. Is the broader anchoring intentional? See `.factory/stories/STORY-INDEX.md` S-2.04 row and `.factory/stories/wave-2/S-2.04-*.md` frontmatter.
- **S-0.06**: Given `holdout_anchors: [H-NEW-VERBOSE-001, H-NEW-VERBOSE-002]`; holdouts are now formally registered in holdout-scenarios.md v1.1.0. Is the holdout registration complete and accurate?
- **S-1.08**: Had an over-declared `depends_on: [S-0.05]` (DRIFT-003; removed at Pass 10). The story has no Wave 0 dependencies. Does this match your understanding of the implementation sequence?

### Q3: Coverage gaps

- The STORY-INDEX Gap Register lists known coverage gaps not addressed in this cycle. Is the gap list final, or are there gaps we're missing?
- The Pre-existing Test Coverage appendix in STORY-INDEX shows existing tests. Should any pre-existing test be promoted to a formal Phase 3 holdout (i.e., given an H-NNN canonical ID)?
- Does the PRD (`.factory/specs/prd/`) have BCs that are not covered by any story in STORY-INDEX? The adversary checked this through 13 passes but a human spot-check is valuable.

### Q4: Convention consistency

- Story IDs follow `S-W.NN-name`. Wave numbers 0/1/2/3. Effort in days (small/medium/large). `depends_on` graph. Holdout naming: `H-NNN` for canonical, `H-NEW-*` for added-this-cycle. Is this naming convention consistent with your long-term intent?
- Holdout-scenarios.md received its first `version:` field (1.1.0) this cycle. Should this file be canonized under the versioning scheme going forward?
- The `strict_binary` convergence rule (1 finding < 3-finding threshold = CLEAN-PASS) was applied at Pass 12. Is this threshold appropriate for Phase 3 wave adversarial reviews?

### Q5: Convergence quality

- 13 passes of Phase 2-adv adversarial review with rotating lens axes (fresh-context BC catalog walk, DRIFT-003 sibling-sweep, DRIFT-004 deep-BC, dependency-graph, coverage, semantic mis-anchor, fabricated-paraphrase) produced 0 substantive findings on Pass 13. Are you satisfied with this convergence rigor, or do you want a 14th pass with a fresh lens set?
- The pre-gate consistency-validator (fresh context, perimeter check) found 3 MEDIUM perimeter findings the adversary had missed (CV2-001/002/003). All 3 were RESOLVED before this gate. Should we institutionalize a fresh-context perimeter audit after every Phase 2-adv convergence, or is it sufficient as a gate-prep step?

---

## Section 6: Risks and Open Items Going Into Phase 3

| Risk / Open Item | Severity | Owner | Notes |
|-----------------|----------|-------|-------|
| ADR-0013 reactivation trigger | LOW | orchestrator | Atlassian announces PKCE for 3LO Jira Cloud → reactivate SD-001. S-3.09 tracks monitoring. Not blocking Phase 3. |
| DRIFT-001 codification gap | MEDIUM | Phase 3 implementer | Count/chain-length fixes must include downstream grep sweep. Codification story S-3.06 lands in Wave 3. Manual discipline required in Waves 0-2. |
| DRIFT-003 sibling-propagation pattern | MEDIUM | Phase 3 implementer | STORY-INDEX ↔ WAVE-PLAN ↔ story frontmatter triple-sync missed 8 times across Phase 2-adv. Must be a checklist item for every story edit in Phase 3. S-3.06 should expand scope to include this check. |
| Wave 3 DEFER semantics | LOW | human | Confirm whether "DEFER" means these stories skip Phase 3 entirely and land in v0.6 backlog, or enter Phase 3 as low-priority backlog items. |
| 32 process-gap items across Phase 1d/Phase 2-adv | LOW | orchestrator | All have either follow-up stories (S-3.06, S-3.09) or deferral entries per S-7.02 discipline. No open action needed before Phase 3. |
| Pre-existing tests vs. holdout promotion | LOW | human | See Q3 above. Decision needed before Phase 3 holdout evaluation gates. |

---

## Section 7: Approve / Reject / Investigate

### Pre-conditions for APPROVE

Before checking Approve, confirm:
- [ ] 31 stories are the right scope for Phase 3 entry
- [ ] BC anchor spot-checks are satisfactory (Q2)
- [ ] Coverage gaps in Gap Register are acceptable (Q3)
- [ ] Convergence rigor is sufficient (Q5)
- [ ] Wave 3 DEFER semantics are understood (Section 6)

---

## Decision

- [x] **Approve** — proceed to Phase 3 (TDD Implementation), starting with Wave 0
- [ ] **Reject** — list specific changes required before re-gate:
  - _[Enter changes here]_
- [ ] **Investigate** — list specific items to deep-dive before deciding:
  - _[Enter items here]_

---

## Approval Notes

Human reviewed structured questions (Q1–Q5) and approved without conditions on 2026-05-07. Proceed to Phase 3 TDD implementation.

---

_Gate document written by state-manager (2026-05-07). All 3 CV2 findings RESOLVED. Phase 2-adv CONVERGED 3/3. Input-hash drift CLEAN. **APPROVED by human (user) 2026-05-07.**_
