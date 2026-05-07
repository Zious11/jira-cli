---
document_type: wave-plan
phase: phase-2-story-decomposition
producer: story-writer
version: "1.0.0"
last_updated: 2026-05-06
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

## Wave 1: High-Priority Features (TBD — next burst)

Wave 1 covers HIGH-priority capability gaps and NFR-anchored BCs that do not represent existing bugs but represent missing or suboptimal behavior for existing users:

- BC-1.x: OAuth multi-site disambiguation (when ADR-0013 PKCE is ready — depends on Wave 0 SD-002 being merged)
- BC-2.x, BC-3.x: Issue read/write improvements anchored to HIGH NFRs
- BC-5.x: Sprint/board improvements
- BC-6.x: Config improvements (beyond the MUST-FIX already in Wave 0)
- BC-7.x: Output rendering improvements

Wave 1 stories will be decomposed in Phase 2 Burst 2 by walking the BC-INDEX against:
- `nfr-catalog.md` for P0/P1 NFRs not yet covered
- `edge-case-catalog.md` for HIGH-severity edge cases
- H-001..H-047 for MUST-PASS holdouts not already covered by Wave 0

### Wave 1 exit gate

- All Wave 1 MUST-PASS holdouts green
- NFR-S-B and NFR-S-C satisfied (carried from Wave 0)
- Performance NFRs NFR-P-* baseline met

---

## Wave 2: Medium Priority (TBD)

Wave 2 covers MEDIUM-priority capabilities: secondary issue-write improvements, assets schema improvements, JSM queue improvements, worklog UX improvements.

Stories decomposed in Phase 2 Burst 3.

### Wave 2 exit gate

- All Wave 2 story-level acceptance criteria passing
- No P0/P1 NFR violations introduced

---

## Wave 3: Low Priority / Deferred (TBD)

Wave 3 covers:

- **PKCE OAUTH** (ADR-0013 deferred, SD-001 DEFERRED): when team is ready to implement PKCE, Wave 3 provides the story
- **NFR-O-S** (multi-site OAuth `--cloud-id` flag): H-047 currently pinned as KNOWN-GAP; Wave 3 flips it to MUST-FAIL
- MEDIUM/LOW edge cases from `edge-case-catalog.md`
- Deferred NFRs (LOW priority)
- `--verbose-bodies` migration aid tooling (if needed post-SD-003 breaking change)

### Wave 3 exit gate

Per-story acceptance criteria passing. No v0.5 release dependency.

---

## Wave Scheduling Principles

1. **DTU clones first**: No external service dependencies identified in `dtu-assessment.md` for Wave 0 work (all Wave 0 bugs are internal logic fixes). DTU clone stories are not required in Wave 0.

2. **Security before features**: SD-002 and SD-003 land in Wave 0 because security regressions are worse than missing features.

3. **Atomic bugs**: Each MUST-FIX story (S-0.01..S-0.04) touches exactly one bug site. They are kept atomic to minimize merge risk and enable parallel implementation.

4. **Breaking changes isolated**: S-0.06 (SD-003 `--verbose-bodies`) is isolated as its own story with `breaking_change: true` so it can be tracked in release notes separately.

5. **Spec-first**: S-0.07 formalizes the H-NEW-AUTH-002 holdout in `holdout-scenarios.md` so Phase 4 evaluation has a concrete spec to evaluate against. This is a doc/fixture story, not a code story.
