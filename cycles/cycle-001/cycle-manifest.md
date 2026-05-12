---
document_type: cycle-manifest
cycle_id: cycle-001
cycle_type: brownfield-activation
version: v0.5.x
status: in-progress
started: 2026-05-04T00:00:00
completed:
producer: orchestrator
---

# Cycle Manifest: cycle-001 (Brownfield Activation)

## Purpose

Brownfield activation of jira-cli at v0.5.0-dev.7. Goals:
1. Formalize existing codebase with VSDD specs, behavioral contracts, verification properties, and holdout scenarios.
2. Establish VSDD pipeline for all post-v0.5.0 feature work.

## Delivered

| Metric | Value |
|--------|-------|
| Stories delivered | 32 (Wave 0: 7, Wave 1: 8, Wave 2: 7, Wave 3: 10) + Feature Mode #110-pr2 |
| BCs created | 541 (at Phase 1 convergence) |
| VPs created | (pending — Phase 4 not started) |
| Holdout scenarios | 51 (48 at Phase 1 gate + 3 additions: H-NEW-VERBOSE-001/002, H-NEW-AUTH-002) |
| Adversarial passes | Phase 1d: 28; Phase 2-adv: 13; Wave 2 gate: 2; Feature Mode F5: 5 + multiple PR Copilot rounds |
| Final holdout satisfaction | (pending — Phase 4 not started) |
| Release version | (pending) |

## Delivered PRs (Feature Mode + Audit-Followup)

| PR | Branch | Issue | SHA | Date | Notes |
|----|--------|-------|-----|------|-------|
| #289–#294 | feat/s-0.0N-* | S-0.01–S-0.06 | various | 2026-05-07 | Wave 0: MUST-FIX bugs + security gates |
| #295–#302 | feat/s-1.0N-* | S-1.01–S-1.08 | various | 2026-05-07/08 | Wave 1: CI/config/observability/holdouts |
| #303–#309 | feat/s-2.0N-* | S-2.01–S-2.07 | various | 2026-05-08 | Wave 2: issue-list, pagination, worklog, JSON |
| #310 | fix/wv2-sec-01 | WV2-SEC-01 | 6cb9994 | 2026-05-08 | CWE-400 parse_duration_validate cap |
| #313–#321 | feat/s-3.NN-* | S-3.01–S-3.10 | various | 2026-05-09 | Wave 3: shard splits, auto-refresh, cursor guard |
| #322 | chore/v0.5.0-dev.9 | (version bump) | — | 2026-05-09 | v0.5.0-dev.9 version bump post-Wave 3 |
| #325 | feat/issue-110-pr1 | #110 PR1 | — | 2026-05-10 | Bulk label ops PR1 |
| #348 | feat/issue-110-pr2 | #110 PR2 | e480ff2 | 2026-05-11 | Bulk edit PR2 (F1-F7 Feature Mode pioneer) |
| #351 | chore/test-hygiene-339-344 | #339+#344 | 3216ec2 | 2026-05-11 | Test hygiene audit-followup |
| #352 | chore/docs-cleanup-337-341-347 | #337+#341+#347 | 57cc0ae | 2026-05-11 | Docs cleanup audit-followup |
| #353 | chore/bulk-max-keys-338 | #338 | 7fbf14d | 2026-05-11 | Consolidate bulk-max-keys constants |
| #354 | chore/labels-shape-doc-342 | #342 | 4e14849 | 2026-05-11 | Labels dry-run shape documentation |
| #355 | chore/task-id-validation-332 | #332 | 448c568 | 2026-05-11 | Task ID validation (CWE-117 pre-validation) |
| #356 | chore/sanitize-errors-334 | #334 | 9acf01d | 2026-05-12 | CWE-117 sanitize_for_stderr (19 rounds) |
| #357 | chore/release-gate-jr-base-url-335 | #335 | d208a6d | 2026-05-12 | Release-gate JR_BASE_URL (2 rounds; fastest) |

## Activation Context

| Field | Value |
|-------|-------|
| Activation HEAD | dea166471e22eff55974d7675593469b37048c5f |
| Activation version | v0.5.0-dev.7 |
| factory-artifacts seed SHA | b8f66501d12a37f7669e01cc95cdb24029a1b4b2 |
| Reference snapshot | .reference/jira-cli/ |

## Spec Changes

| Artifact | Change | Before | After |
|----------|--------|--------|-------|
| (none yet) | | | |

## Living Spec Snapshot

Captured at: (pending — will be tagged on factory-artifacts branch at convergence)

## Tech Debt Created

| ID | Description | Priority | Source |
|----|-------------|----------|--------|
| (none yet) | | | |

## Notes

- Pre-VSDD docs treatment decision (DEC-001) deferred to Phase 0 → Phase 1 gate.
- dx-engineer running env preflight in parallel with state initialization.
