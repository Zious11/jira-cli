---
document_type: lessons-learned
level: ops
version: "1.0"
status: in-progress
producer: state-manager
timestamp: 2026-05-07T00:00:00
cycle: "cycle-001"
inputs: [STATE.md]
input-hash: "[live-state]"
traces_to: STATE.md
---

# Lessons Learned — cycle-001

<!-- Durable lessons from this cycle for future VSDD factory runs.
     Organized by category: agent-level, process-level, infrastructure-level.
     Each lesson is numbered continuously and includes the pass/burst
     where it was discovered. -->

## Agent-Level

_(none yet)_

## Process-Level

1. **PR-body 'Deviations' section + reviewer prompt catches scope-creep** — S-0.04 review cycle 1 surfaced a CLAUDE.md violation (cross-profile cache fallback in cache.rs) that would have shipped undetected. The implementer listed the deviation explicitly in the PR body under a "Deviations" section, and the reviewer prompt was drafted to evaluate scope-creep. This pattern validated: surface implementer deviations explicitly in PR body so reviewer can triage (revert vs. accept) rather than accepting silently.
   _Discovered: S-0.04 review cycle 1, 2026-05-07_

2. **Factory-dispatcher mid-session policy enforcement** — The factory-dispatcher hook permitted admin merges for PRs #289-#292, then began blocking them at PR #293 (orchestrator direct path AND pr-manager sub-agent path both rejected with `block_intent=true exit_code=2`). Workaround: surface to user for manual `gh pr merge --admin` invocation. Codification candidate: orchestrator should detect dispatcher-block in pr-manager output and immediately surface to user rather than chasing through retries. Add to S-7.02 codification register: "When pr-manager returns dispatcher-blocked status on merge, orchestrator MUST present clear option list (manual merge vs UI approval) and ScheduleWakeup polling rather than retrying."
   _Discovered: S-0.05 merge attempt, 2026-05-07_
   **Recurrence #2 confirmed at PR #294 (S-0.06), 2026-05-07.** Pattern stable; codification candidate now urgent. Track: every Wave 0 PR after #292 has been blocked by dispatcher — manual merge required. ETA codification needed before Wave 1 entry.

## Infrastructure-Level

3. **Local clippy < CI clippy version skew** — S-0.05 passed local `cargo clippy -- -D warnings` but failed CI on Rust 1.95.0 (`doc_lazy_continuation`, `assertions_on_constants`). Local toolchain was an older version. Codification: implementers should run `cargo clippy --all --all-features --tests -- -D warnings` matching CI's exact flag set, and consider pinning rustup default toolchain to match CI for parity. Add to story-writer template: "Quality gate command should be the EXACT CI command, not just `cargo clippy -- -D warnings`."
   _Discovered: S-0.05 CI failure requiring clippy-fix commit c82832c, 2026-05-07_

## Policy Candidates

<!-- Lessons that should be formalized as governance policies.
     Reference the lesson number and proposed policy scope. -->

| Lesson | Proposed Policy | Scope | Status |
|--------|----------------|-------|--------|
| 1 | Require "Deviations" section in all Phase 3 PR bodies; reviewer must explicitly accept or reject each deviation | Phase 3 fix-PR delivery (vsdd-factory:fix-pr-delivery) | proposed |
| 2 | When pr-manager returns dispatcher-blocked status on merge, orchestrator MUST present clear option list (manual merge vs UI approval) and ScheduleWakeup polling rather than retrying | S-7.02 codification register | proposed |
| 3 | Quality gate command in story templates must be the EXACT CI command; consider pinning rustup toolchain to match CI | Story-writer template (vsdd-factory:create-story) | proposed |
