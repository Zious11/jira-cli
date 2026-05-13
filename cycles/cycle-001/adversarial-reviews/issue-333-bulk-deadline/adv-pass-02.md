---
document_type: adversarial-review
phase: F5
issue: 333
commit: dbb4e5f
branch: fix/issue-333-bulk-deadline
pass: 02
producer: vsdd-factory:adversary
timestamp: 2026-05-12
status: applied
---

# Adversarial Review — Issue #333 Bulk Deadline (Pass 02)

Commit reviewed: `dbb4e5f` (the pass-01 fix commit on top of `618ca14`)
Verdict: NOT CLEAN-PASS — 1 BLOCKING + 2 CONCERN + 4 NIT.

## Critical Findings (BLOCKING)

### B-1 — Exponential-backoff sleep in `await_bulk_task_inner` is NOT clamped to deadline (HIGH)

`bulk.rs:495-498` — after a successful (non-terminal) poll response, the polling loop sleeps `backoff` seconds (1→2→4→8→10) WITHOUT consulting the deadline. A 30s deadline + RUNNING-storm responses can overshoot by up to POLL_MAX_SECS=10s before the next top-of-loop check fires.

Pass-01 fix focused exclusively on the 429-retry sleep INSIDE `send_inner`. The exponential-backoff sleep BETWEEN polls was missed.

**Fix applied:** clamp `backoff_sleep` by `deadline.saturating_duration_since(Instant::now())`. Per Q1(a) research-validation pass-03: use `Duration::min` directly (not the `clamp_retry_sleep` helper) — the top-of-loop check on the next iteration catches sub-millisecond residuals without needing an Expired short-circuit.

**Test added:** `tests/bulk_deadline_propagation.rs::test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp` — RUNNING-storm + 30s deadline → exit within 40s (vs ~40s+ pre-fix).

## Important Findings (CONCERN)

### C-1 — `send_inner` entry-point check is untested (MEDIUM)

The pass-01 fix added a defense-in-depth entry-point check that returns Err for already-expired deadlines. No test pinned this behavior.

**Test added:** `src/api/client.rs::clamp_tests::test_clamp_retry_sleep_entry_point_pattern_already_expired_is_expired` — exercises the same `clamp_retry_sleep(Duration::ZERO, Some(d))` pattern used in `send_inner`'s entry check. A regression that disabled the check would surface as Sleep instead of Expired.

### C-2 — Status code 429 used for both 429-rate-limit AND deadline-exceeded (MEDIUM)

Both the entry-point check and the in-loop clamp wrapped the deadline error in `JrError::ApiError { status: 429, ... }`. Misleading: the entry-point case has no 429 *response*; scripts grepping for "429" to detect rate-limit pressure would false-positive on deadline-exceeded.

**Prior decision reversed (research-validation pass-03 Q2):** earlier we kept `ApiError(429)` based on flawed "hundreds of match sites" reasoning. Actual count: 6 src match arms, all on 404 (zero on 429). External CLI precedent (kubectl, gh, aws-cli, doctl, fly) ALL use a dedicated variant for client-side deadlines.

**Fix applied:** new `JrError::DeadlineExceeded { remaining_ms, message }` variant with exit code 124 (POSIX `timeout(1)` convention). Both deadline-exceeded sites in `client.rs` updated to use it. Two unit tests added (`error::tests::deadline_exceeded_exit_code_is_124`, `error::tests::deadline_exceeded_display_format`).

## Observations (NIT)

### N-1 — `Retry-After: 0` produces tight spin-loop within MAX_RETRIES (LOW) — DEFERRED

The clamp test `test_clamp_retry_sleep_zero_base_returns_zero_sleep` already pins this behavior. A separate fix in the rate-limit parser to floor at `DEFAULT_RETRY_SECS=1` is the proper remedy. Defer to follow-up.

### N-2 — `Expired::remaining_ms` always reports 0 (LOW) — DEFERRED

By construction, `remaining < 1ms` truncates to 0ms. The field is structurally constant. Could remove or change to nanoseconds. Defer — minor structural observation.

### N-3 — Docstring on `send_bounded` only documents the 429-retry message (LOW) — APPLIED

**Fix applied:** rewrote `send_bounded` rustdoc to explicitly document BOTH messages (entry-point + in-loop), the new `DeadlineExceeded` variant, and exit code 124.

### N-4 — `bulk_deadline_propagation.rs` 429 mock has no `.expect(...)` (LOW) — DEFERRED

The wall-clock budget (25-40s) plus the new RUNNING-storm test cover the behavior end-to-end. `.expect_range(1..=4)` would tighten further but adds maintenance friction (clamp implementation could legitimately fire more or fewer requests under varying CI load). Defer.

## Triage Summary

| ID | Verdict | Action |
|---|---|---|
| B-1 | APPLIED | Outer-loop clamp + RUNNING-storm test |
| C-1 | APPLIED | New unit test for entry-point pattern |
| C-2 | APPLIED | New `JrError::DeadlineExceeded` variant + exit 124 + 2 unit tests |
| N-1 | DEFERRED | Pinned by existing test; rate-limit parser fix is separate |
| N-2 | DEFERRED | Cosmetic; field is documented as constant |
| N-3 | APPLIED | Docstring rewrite |
| N-4 | DEFERRED | Adds maintenance friction; covered by wall-clock + new test |

Apply set: 4 fixes (B-1, C-1, C-2, N-3). Defer set: 3 (N-1, N-2, N-4).

## Novelty Assessment

| Finding | Novel vs pass-01? |
|---|---|
| B-1 | YES — wholly new finding; pass-01 missed the outer-loop sleep entirely. |
| C-1 | YES — pass-01 raised the entry-point check AS A FIX, not as a coverage gap. |
| C-2 | YES — pass-01 chose to reuse `ApiError(429)` based on flawed analysis; pass-02 reversed correctly. |
| N-1, N-2, N-3, N-4 | Mostly NEW — pass-01 NIT-2 was a related but distinct precedence note. |

All findings are first-pass observations; pass-02 substantially improved the fix.

## Convergence Trajectory

Pass 01: 14 findings (0+7+7).
Pass 02: 7 findings (1+2+4).
Trend: dropping, but B-1 indicates the search was not thorough enough in pass-01 (covered only the 429 sleep, not the backoff sleep).

Counter reset to 0 (BLOCKING found). Need 3 consecutive CLEAN-PASSes from a fresh-context adversary to converge.
