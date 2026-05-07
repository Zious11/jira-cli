---
document_type: wave-plan
phase: phase-2-story-decomposition
producer: story-writer
version: "1.1.0"
last_updated: 2026-05-04
activation_head: dea1664
---

# Wave Plan — jira-cli (jr)

Rationale for wave sizing, ordering, and exit gates.

---

## Wave 0: MUST-FIX + Security (7 stories)

### Why Wave 0 exists

Wave 0 contains work that MUST ship before any v0.5 release can be cut. It is defined by two criteria:

1. **MUST-FIX bugs**: The holdout evaluation (Phase 4) would fail on production bugs that exist at activation HEAD dea1664. These are real regressions for existing users, not speculative improvements. They are flagged in the PRD as NFR-R-A (HIGH), NFR-R-B (HIGH), NFR-R-D (CRITICAL), NFR-R-E (HIGH).

2. **Security decisions resolved at Phase 1 gate**: SD-002 and SD-003 were resolved at the Phase 1→2 gate (2026-05-04). The implementation of those decisions is Wave 0 work because:
   - SD-002 resolves DRIFT-002 (NFR-S-B holdout), which was undefinable until the decision was made.
   - SD-003 introduces a breaking change (`--verbose` behavior) that must land early to minimize migration surface.

### Wave 0 exit gate

All of the following must be true before Wave 1 dispatch:

- H-045 (`list_worklogs` pagination): MUST-PASS
- H-046 (`jr issue open` instance URL): MUST-PASS
- H-036 (multi-workspace asset HashMap): MUST-PASS
- H-NEW-MP-001 (multi-profile fields): MUST-PASS
- H-NEW-AUTH-002 (`JR_AUTH_HEADER` not honored in release): MUST-PASS
- SD-003 verbose-bodies holdouts (2): MUST-PASS
- No regression on H-001..H-044, H-047 (the non-MUST-FIX holdouts)

### Story ordering within Wave 0

Stories S-0.01 through S-0.04 are independent (no inter-story deps) — can be implemented in parallel by separate agents. S-0.05 and S-0.07 are paired (S-0.07 formalizes the holdout that S-0.05 must satisfy). S-0.06 is independent.

Recommended parallel groups:
- **Group A (parallel)**: S-0.01, S-0.02, S-0.03
- **Group B (parallel)**: S-0.04, S-0.06
- **Group C (sequential)**: S-0.05 then S-0.07 (S-0.07 documents what S-0.05 implements)

---

## Wave 1 — HIGH-priority security posture, supply-chain hardening, structured logging, regression pinning (8 stories)

| Story | Title | BC/NFR Anchors | Depends on | Status | Effort |
|-------|-------|---------------|------------|--------|--------|
| S-1.01 | Pin GitHub Actions SHAs | NFR-S-E, R-H6 | — | draft | small |
| S-1.02 | cargo-deny supply-chain audit | NFR-S-F | — | draft | small |
| S-1.03 | tracing/observability wire-up | NFR-O-A | S-0.06 | draft | medium |
| S-1.04 | CI job timeouts | R-L12 | — | draft | xsmall |
| S-1.05 | GitHub secret scanning | NFR-S-E, R-L13 | — | draft | small |
| S-1.06 | OAuth flow holdout suite | BC-1.1.001/002, H-001..H-006, H-022, H-029 | S-0.05 | draft | medium |
| S-1.07 | Rate-limit holdout suite | BC-X.4.002, H-013, H-027 | — | draft | small |
| S-1.08 | Keychain round-trip holdout | BC-1.4.027, H-016 | — | draft | small |

**Wave 1 parallel groups:** {S-1.01, S-1.02, S-1.04, S-1.05} CI infra (parallel); {S-1.03} blocked by S-0.06; {S-1.07, S-1.08} regression pins (parallel); {S-1.06} OAuth holdout suite (depends on S-0.05 — start after S-0.05 merges).

**Exit gate:** All 8 Wave 1 stories merged + holdouts written + SHA pinning + cargo-deny + tracing layer landed.

---

## Wave 2 — MEDIUM-priority NFRs requiring code work + BC holdout regression suites (7 stories)

| Story | Title | BC/NFR Anchors | Depends on | Status | Effort |
|-------|-------|---------------|------------|--------|--------|
| S-2.01 | BC-2 issue-read holdout suite (incl. H-021) | BC-2.1.*, H-030..H-035, H-021 | — | draft | medium |
| S-2.02 | BC-3 issue-write holdout suite | BC-3.*, H-007, H-008, H-014 | — | draft | medium |
| S-2.03 | BC-4 asset enrichment holdout suite | BC-4.*, H-037, H-038, H-039 | (S-0.03 recommended first) | draft | medium |
| S-2.04 | BC-5 boards/sprints holdout suite | BC-5.*, H-040..H-044 | — | draft | medium |
| S-2.05 | CLAUDE.md documentation update | NFR-O-L/M/O/V/R, NFR-R-F | — | draft | small |
| S-2.06 | Worklog duration config + CMDB cache tuple | NFR-R-C, BC-X.5.009, BC-6.2.013 | — | draft | medium |
| S-2.07 | JSON output policy + test naming | NFR-O-F/J/W, H-020 | — | draft | small |

**Wave 2 parallel groups:** {S-2.01, S-2.02, S-2.03, S-2.04} BC holdout suites (parallel); {S-2.05, S-2.07} doc/policy (parallel); {S-2.06} new endpoint + cache.

**Exit gate:** All 7 Wave 2 stories merged + BC-2/3/4/5/7 regression suites in place + CLAUDE.md updated.

---

## Wave 3 — LOW-priority + DEFER + DRIFT codification + cleanup (9 stories)

| Story | Title | BC/NFR Anchors | Depends on | Status | Effort |
|-------|-------|---------------|------------|--------|--------|
| S-3.01 | Refactor src/cli/auth.rs shard split (1,998 LOC) | NFR-O-D, R-M6 | — | draft | medium |
| S-3.02 | Refactor src/cli/assets.rs shard split (1,055 LOC) | NFR-O-D | — | draft | small |
| S-3.03 | refresh_oauth_token investigation | NFR-O-B | — | draft | medium |
| S-3.04 | Multi-cloudId disambiguation | NFR-O-S, BC-1.5.038, BC-1.1.007, BC-1.5.031, H-047 | — | draft | medium |
| S-3.05 | Asset enrichment concurrency cap | NFR-P-NEW-1 | — | draft | small |
| S-3.06 | Spec numeric-claim checker (DRIFT-001) | DRIFT-001 process-gap | — | draft | small |
| S-3.07 | LOW NFR code cleanup (4 parts bundled) | NFR-R-NEW-1/2, ... | — | draft | small |
| S-3.08 | LOW NFR DOCUMENT-AS-IS | 15 LOW NFRs | — | draft | small |
| S-3.09 | PKCE deferral formal record | NFR-S-A, ADR-0013, SD-001 | — | draft | xsmall |

**Wave 3 parallel groups:** {S-3.01, S-3.02} refactors (parallel); {S-3.03, S-3.04, S-3.05} feature/investigation (parallel); {S-3.06, S-3.07, S-3.08, S-3.09} cleanup/doc (parallel).

**Exit gate:** Phase 3 starts upon Wave 0 + Wave 1 + Wave 2 exit. Wave 3 stories merge during steady-state v0.6 cycle.

---

## Wave Scheduling Principles

1. **DTU clones first**: No external service dependencies identified in `dtu-assessment.md` for Wave 0 work (all Wave 0 bugs are internal logic fixes). DTU clone stories are not required in Wave 0.

2. **Security before features**: SD-002 and SD-003 land in Wave 0 because security regressions are worse than missing features.

3. **Atomic bugs**: Each MUST-FIX story (S-0.01..S-0.04) touches exactly one bug site. They are kept atomic to minimize merge risk and enable parallel implementation.

4. **Breaking changes isolated**: S-0.06 (SD-003 `--verbose-bodies`) is isolated as its own story with `breaking_change: true` so it can be tracked in release notes separately.

5. **Spec-first**: S-0.07 formalizes the H-NEW-AUTH-002 holdout in `holdout-scenarios.md` so Phase 4 evaluation has a concrete spec to evaluate against. This is a doc/fixture story, not a code story.
