---
document_type: brainstorming-report
topic: fork-safe E2E CI enablement + status badge
date: 2026-06-02
mode: feature
techniques: [reverse-brainstorming]
status: direction-selected
recommended_next_step: phase-f1-delta-analysis
---

# Brainstorming Report — Fork-safe E2E CI Enablement + Status Badge

## Session Summary

- **Facilitator:** orchestrator (brainstorming skill)
- **Date:** 2026-06-02
- **Technique:** Reverse brainstorming ("how would this feature quietly fail or cause harm?" → invert into safeguards)
- **Trigger:** Human request — "create a flag for the e2e CI so if a variable is missing it is just skipped in github ci," reframed during session to the real problem (below).

## Problem (as reframed during the session)

The initial framing was "skip a test when a variable is missing." Grounding in the codebase revealed the suite ALREADY has per-test skip behavior (optional vars `match`-skip; required vars `.expect()`-fail). The human reframed the actual pain:

> On forked repos the E2E workflow is FAILING (forks lack the `jira-e2e` secrets/variables), causing downstream issues. We want a flag we set to ENABLE E2E testing (so forks default off), plus a repo status badge for E2E.

## Current State (grounding)

- `e2e.yml` already guards `if: github.event_name != 'pull_request'`, but push-to-develop/main and the nightly `cron` can still fire on a fork, where missing secrets make required-var `.expect()` calls panic → red run.
- Two existing in-test skip patterns: REQUIRED vars (`JR_E2E_BASE_URL`, `JR_E2E_PROJECT`) fail loud via `.expect()`; OPTIONAL vars (`JR_E2E_BOARD_ID`, `JR_E2E_JSM_PROJECT`, `JR_E2E_EMAIL`, `JR_E2E_ISSUE_TYPE_ALT`) clean-skip via `match env::var`.
- 11 distinct `JR_E2E_*` vars referenced in tests/e2e_live.rs.

## Ideas Generated (reverse-brainstorm: failure mode → safeguard)

1. Gate inside the Rust test → fork still starts the job, burns CI minutes, badge goes green having tested nothing. → SAFEGUARD: gate at the workflow/job `if:` level so the job never starts on a fork.
2. Hardcoded `true` in YAML → fork inherits and tries to run. → SAFEGUARD: gate on a REPO VARIABLE; forks do not inherit variables/secrets, so they default OFF.
3. Owner-string gate (`github.repository == ...`) rots on rename/transfer and forbids a fork from opting in. → SAFEGUARD: prefer the variable flag (optionally combine with owner check).
4. Blanket "missing var ⇒ skip everywhere" also silences a real misconfig in the canonical repo → silent coverage erosion, badge stays green. → SAFEGUARD: two-tier — the WORKFLOW GATE decides whether E2E runs at all (fork vs canonical); once running, REQUIRED vars still fail loud.
5. Badge lies (green because the job was skipped / ran 0 tests). → SAFEGUARD: pin badge to canonical repo + e2e.yml + `?branch=develop`; it reflects the upstream nightly, not a fork's skipped run.
6. Scheduled cron on forks still fires. → SAFEGUARD: the variable gate covers ALL triggers (push, schedule, dispatch), not just PRs.
7. No escape hatch for a serious fork maintainer with their own sandbox. → SAFEGUARD: variable-flag design lets a fork opt in with its own `JR_E2E_ENABLED=true` + secrets.

## Themes

- **Theme 1 — Fork-safety via a workflow-level enable flag** (core fix).
- **Theme 2 — Visibility via a pinned README status badge.**

## Selected Direction

**Fork-safe E2E enablement + status badge.**

- **Solution:**
  1. Gate the `e2e.yml` job with `if: vars.JR_E2E_ENABLED == 'true'` (combined with the existing `github.event_name != 'pull_request'` guard). Forks default OFF (variables not inherited); canonical repo sets `JR_E2E_ENABLED=true` once; forks may opt in with their own var + secrets. Required-var loud-fail preserved WHEN enabled.
  2. Add a README status badge pinned to `e2e.yml` on `develop`.
- **Audience:** fork contributors (no spurious red), maintainers (clean opt-in control), README readers (visible E2E health).
- **Differentiator:** gates at the workflow layer (not inside tests) → forks burn zero CI and the badge cannot be faked; variable-based so forks auto-disable yet can opt in.
- **Decisions locked in session:** (a) gate mechanism = repo variable `JR_E2E_ENABLED` (chosen over owner-check and over the belt-and-suspenders combo); (b) badge scope = develop nightly.

## Open Questions for Phase F1 (Delta Analysis)

1. Exact `if:` expression and how it composes with the existing `if: github.event_name != 'pull_request'` and the push / `workflow_dispatch` / `cron` triggers. Job-level vs per-step gate.
2. Must `JR_E2E_ENABLED` also guard the `if: failure()` auth-check step and the `if: always()` cleanup step (both reference secrets)? If the gated job is skipped, do those steps still attempt to run?
3. Badge rendering semantics: when the job is gated OFF, does the run conclude "success"/"skipped"/absent, and how does `badge.svg?branch=develop` render that? Verify before claiming the badge is truthful.
4. Optional hardening: an always-on (when enabled) "E2E config preflight" that asserts all REQUIRED vars are present and fails loud — closes the silent-coverage-erosion gap (failure mode #4). In scope or defer?
5. Naming: `JR_E2E_ENABLED` vs `ENABLE_E2E` vs `RUN_E2E`. Keep distinct from the existing test-binary seam `JR_RUN_E2E` (CI gate vs binary gate) and DOCUMENT the relationship.
6. Scope confirmation: is this purely a CI-workflow + README change (zero `src/`, zero `tests/e2e_live.rs` change)? If so it is a lightweight Feature Mode cycle (like prior E2E-infra work), likely skipping Red Gate/implementer/demo.
7. Documentation fallout: CLAUDE.md E2E section + docs/specs/e2e-live-jira-testing.md env-var table should describe `JR_E2E_ENABLED` and the fork-opt-in flow.

## Recommended Next Step

Proceed to **Phase F1 — Delta Analysis** (`/vsdd-factory:phase-f1-delta-analysis`) for the feature "Fork-safe E2E CI enablement flag (`JR_E2E_ENABLED`) + README E2E status badge." Expect LOW regression risk and a likely zero-`src/` scope (CI workflow + README + docs only).
