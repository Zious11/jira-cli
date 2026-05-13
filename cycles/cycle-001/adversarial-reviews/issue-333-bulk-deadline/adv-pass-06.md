---
document_type: adversarial-review
phase: F5
issue: 333
commit: 9c985e6
branch: fix/issue-333-bulk-deadline
pass: 06
producer: vsdd-factory:adversary
timestamp: 2026-05-12
status: CLEAN-PASS-3-of-3-CONVERGED
---

# Adversarial Review — Issue #333 Bulk Deadline (Pass 06)

**Verdict: CLEAN-PASS — counter 2 → 3. F5 PHASE CONVERGED.**

0 BLOCKING + 0 CONCERN + 2 NIT (cosmetic, novel) + 1 observation (non-actionable).

## NITs (novel, cosmetic)

### NIT-1 — Site-tag strings not pinned by any test

`[deadline:send-entry]`, `[deadline:429-retry]`, `[deadline:bulk-outer]` appear in source strings + rustdoc but no test asserts on them. A future refactor renaming these tags would not trip CI.

Why not CONCERN: Both integration tests pin exit 124 AND stderr "deadline" substring — broad regression class covered. Specific bracketed-tag wording is documented intent, not a behavioral contract.

### NIT-2 — `Instant::now() + timeout` unprotected against u64::MAX overflow

`bulk.rs:385` uses raw `+`. Debug-only path via `JR_BULK_AWAIT_TIMEOUT_SECS=18000000000` typo could panic. Production safe (DEFAULT=300s; env var `#[cfg(debug_assertions)]`-gated). Pass-01 NIT-5 deferred-by-design.

## Observation (non-actionable)

### B-1 pre-fix detection margin is tight

Pre-fix outer-clamp regression could elapse 35-40s; post-fix budget 40s. AC-001 has 5× gap (210s → 30s vs 40s budget); B-1 has no such gap. Detection is non-deterministic at the boundary. Not actionable — tightening would risk false positives under CI load.

## Cross-checks completed CLEAN

| Q# | Check | Verdict |
|---|---|---|
| Q3 | Rate-limit forward-compat (missing/malformed Retry-After) | CLEAN — falls back to DEFAULT_RETRY_SECS=1 |
| Q4 | Test isolation under nextest | CLEAN — `assert_cmd::Command::env()` per-child; wiremock random port |
| Q5 | MAX_RETRIES × deadline-aware iteration count | CLEAN — iterates exactly 2 times in 429-storm test |
| Q6 | AC traceability via tests | CLEAN — all 3 sites + exit 124 have pinning tests |
| Q8 | CLAUDE.md gotchas already present | CLEAN — JR_BULK_AWAIT_TIMEOUT_SECS documented; meta-rule for new JR_* vars present |
| Q2 | Rollback safety | CLEAN — reverting either clamp commit exposes a pinning test |

Q1 (Windows cross-platform) and Q7 (single end-to-end design summary) are NOT in scope: jr does not target Windows (verified `release.yml` + `ci.yml`); the orchestrator owns the PR-body summary doc which is not yet written at adversary-review time.

## Convergence Trajectory

| Pass | BLOCKING | CONCERN | NIT | Verdict | Counter |
|---|---|---|---|---|---|
| 01 | 0 | 7 | 7 | not clean | 0 |
| 02 | 1 | 2 | 4 | not clean | 0 |
| 03 | 0 | 3 | 5 | not clean | 0 |
| 04 | 0 | 0 | 2 | CLEAN | 1/3 |
| 05 | 0 | 0 | 2 | CLEAN | 2/3 |
| **06** | **0** | **0** | **2** | **CLEAN** | **3/3 — CONVERGED** |

## Novelty Assessment

LOW. NIT-1 and NIT-2 are net-new at pass-06 depth. Pass-05 NITs (JSON envelope, tracing asymmetry) are still extant and still cosmetic, not re-listed.

The fix has converged on substance. Remaining findings are the texture expected at pass-06 depth in any well-reviewed change.

## Final Convergence Verdict

**F5 ADVERSARIAL PHASE: CONVERGED.**

5-commit chain (`618ca14`, `dbb4e5f`, `0439243`, `d5334fa`, `9c985e6`) is ready for F6 → F7 → PR.
