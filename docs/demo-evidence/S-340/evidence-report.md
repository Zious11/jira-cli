# S-340 Demo Evidence Report

**Story:** S-340 — Pin task_id-in-bulk-poll-timeout-message contract (BC-3.4.009)
**Issue:** #340
**Worktree branch:** feature/S-340-bulk-poll-task-id-pin
**HEAD:** d63b2eba51f4ca740e5550be3c352ee301a1b7fe
**Recorded:** 2026-05-15

## Type
TEST-ONLY DELIVERY — production code unchanged; behavior pinned via regression test.

## AC-1 / AC-2 — task_id assertions fire in pass state
**Evidence:** ac1-ac2-b1-test-passes.txt
**Verdict:** PASS — last line: `test result: ok. 1 passed`.

## AC-3 — Full bulk_deadline_propagation suite passes (no regression)
**Evidence:** ac3-full-bulk-deadline-suite-passes.txt
**Verdict:** PASS — last line: `test result: ok. 2 passed`.

## AC-4a — cargo fmt --check
**Evidence:** ac4a-fmt-check.txt
**Verdict:** PASS — no output / zero exit.

## AC-4b — cargo clippy --all-targets -- -D warnings
**Evidence:** ac4b-clippy.txt
**Verdict:** PASS — no warnings denied.

## Red Gate mutation evidence
**Mutation:** `Bulk task {task_id}` → `Bulk task <redacted>` at src/api/jira/bulk.rs:412
**Evidence:** red-gate-mutation-fails.txt
**Verdict:** FAILS WITH BC-3.4.009 VIOLATION (loose) — the contract assertion fires under mutation. Production code reverted via `mv src/api/jira/bulk.rs.bak src/api/jira/bulk.rs` + `touch` to force recompile; post-revert test run green (2 passed).

## Files
- `tests/bulk_deadline_propagation.rs` (+36 lines, additive only — BC↔test index comment + loose + strict assertions in existing B-1 test)
- `src/api/jira/bulk.rs` — UNCHANGED (mutation was temporary, reverted)

## Trace
- BC-3.4.009 pinned
- Story S-340 AC-1, AC-2, AC-3, AC-4 satisfied
- Mutation Red Gate substituted for green-on-first-run regression-pin pattern (see /Users/zious/Documents/GITHUB/jira-cli/.factory/cycles/cycle-001/S-340/implementation/red-gate-log.md)
