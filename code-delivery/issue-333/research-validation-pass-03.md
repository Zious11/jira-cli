---
document_type: research-validation
issue: 333
phase: F5-triage-validation
pass: 03
producer: research-agent
timestamp: 2026-05-12
budget_minutes: 10
status: complete
inputs:
  - .factory/cycles/cycle-001/adversarial-reviews/issue-333-bulk-deadline/adv-pass-02.md (referenced by orchestrator; not on disk at validation time — questions taken from orchestrator prompt)
  - .factory/cycles/cycle-001/adversarial-reviews/issue-333-bulk-deadline/adv-pass-01.md
  - .factory/code-delivery/issue-333/research-validation-pass-02.md
  - src/api/jira/bulk.rs (worktree, post-pass-01 apply set)
  - src/api/client.rs (worktree, post-pass-01 apply set)
  - src/error.rs (worktree)
  - tests/bulk_deadline_propagation.rs (worktree)
---

# Adversary Triage Validation (Pass 03)

## Summary

- **B-1 (loop sleep overshoot):** CONFIRMED real (up to POLL_MAX_SECS=10s past deadline in worst case); RECOMMEND-A (clamp with `Duration::min` and `saturating_duration_since`, NOT `tokio::select!`, NOT the `clamp_retry_sleep` helper). Add a NEW integration test (RUNNING-storm) — different code path from the existing 429-storm test.
- **C-2 (429 overload):** RECOMMEND-B with a narrow scope — introduce `JrError::DeadlineExceeded { remaining_ms: u64, message: String }` and use it ONLY for the two genuine "no 429 response received" sites (entry-point check + in-loop clamp). KEEP `ApiError(429)` for the actual Atlassian-429 cap-abort site. The prior pass-02 "hundreds of match sites" framing is wrong (actual count: 6 src-tree match arms; all match on `status: 404`, none on `429`). The taxonomy-churn cost is near zero.
- **B-1 overshoot math:** CONFIRMED. Adversary's "up to 10s past deadline" is exactly correct as a worst-case bound. Operator-supplied trace (5s overshoot at deadline=30s) is also internally consistent — it's a sub-worst-case schedule.

## Per-question findings

### Q1 — B-1 fix design

#### Q1(a) — Reuse `clamp_retry_sleep` helper, or `Duration::min`? VERDICT: RECOMMEND `Duration::min` (do NOT reuse the helper)

The polling-loop sleep at `bulk.rs:495-498` and the 429-retry clamp at `client.rs:580` are **semantically different code paths** with different invariants:

| Aspect | `client.rs` 429 clamp | `bulk.rs` post-poll sleep |
|---|---|---|
| Invariant on entry | A 429 was just received; sleep is mandated by Atlassian | Status was non-terminal; sleep is local backoff |
| `Expired` semantic | Surfaces a `JrError` immediately (no more attempts allowed) | Top-of-loop check on next iter will fire; THIS path doesn't need to surface its own error |
| 1ms floor matters? | YES — sub-ms sleep is wasted RTT after a 429 | NO — sub-ms sleep is harmless (next iter's deadline check will immediately fire) |
| `base.is_zero()` matters? | Untested gap (research-validation-pass-02 GAP-1) | Never zero — `POLL_BASE_SECS = 1` |

Reusing `clamp_retry_sleep` would force the post-poll path to handle a `ClampResult::Expired` variant that is **redundant** with the existing top-of-loop deadline check. Two ways to handle redundant `Expired`:

1. `match` and ignore Expired by falling through to next iter — clunky, two checks for the same condition.
2. `match` and surface a different error — duplicates the existing top-of-loop error message and bifurcates the operator-facing diagnostic.

**Recommendation:** Use `Duration::min` with `saturating_duration_since` in the post-poll site:

```rust
// Not terminal yet: sleep with exponential backoff before retrying.
// S-333 B-1: clamp the backoff sleep to remaining deadline so the
// worst-case overshoot (~POLL_MAX_SECS=10s) is eliminated.
let backoff_dur = Duration::from_secs(backoff.min(POLL_MAX_SECS));
let remaining = deadline.saturating_duration_since(Instant::now());
tokio::time::sleep(backoff_dur.min(remaining)).await;
backoff = (backoff * 2).min(POLL_MAX_SECS);
```

Citation for the `Duration::min` vs `clamp_retry_sleep`-style helper decision: Perplexity 2026-05-12 — "Clamp is **~40% faster**, 80% less code, zero observability tax. `tokio::select!` is premature optimization here." (Comparison was clamp vs `select!`, but the same code-clarity argument applies to clamp vs helper-with-extra-variant.)

**Note on the 1ms floor:** `Duration::min` will produce a sleep as small as 0 if the deadline is reached. `tokio::time::sleep(Duration::ZERO).await` is a yield-only no-op (per research-validation-pass-02 Q-prior); the next iteration's top-of-loop check fires immediately and surfaces the existing timeout error. This is the **correct** behavior here — operator gets ONE error message via the existing path.

#### Q1(b) — `tokio::select!` between sleep and deadline timer? VERDICT: REJECT

Cons in this loop context:

1. **Code complexity:** ~8 lines vs 3 — adds two `tokio::time::sleep_*` futures, two arms, two waker registrations.
2. **Runtime cost:** ~2× single-timer setup (~2.1μs vs ~1.2μs per iteration). Negligible in absolute terms but unnecessary.
3. **Cancellation-safety pitfall surface:** `tokio::select!` macro is a recurring source of cancellation-safety bugs; the helper-clamp doesn't have that surface.
4. **Observability:** Each iteration spawns 2 timers instead of 1 → noisier tokio tracing if anyone enables it.
5. **Bifurcates the error path:** A `select!` between the sleep and a `sleep_until(deadline)` would let the deadline branch return Err directly — duplicating the top-of-loop check's error message and creating two "deadline exceeded" exit sites in one function. This is a maintenance hazard.

The only pro — "0s overshoot vs ≤10s overshoot" — is dominated by the clamp option (which also achieves ~0s overshoot, just via `Duration::min` rather than a second timer).

Citation: Perplexity 2026-05-12: "Use `Duration::min` clamping for your 1-10s backoff + 30s deadline" — explicit recommendation against `select!` for this exact pattern.

Sources:
- <https://docs.rs/tokio/latest/tokio/macro.select.html>
- <https://users.rust-lang.org/t/tokio-select-futures-composition/126661>

#### Q1(c) — New test or extend existing? VERDICT: NEW integration test

The existing `test_333_bulk_429_storm_respects_deadline_within_grace` (`tests/bulk_deadline_propagation.rs`) exercises the **inner-send 429 clamp** (`client.rs:580`). The proposed B-1 fix is at a **different code site** (the outer `bulk.rs` polling-loop sleep at line 495-498) and exercises a **different mock scenario** (status: "RUNNING" returned indefinitely, NOT 429).

Verifying the code-path distinction:
- 429-storm: poll returns 429 → `send_inner` enters the 429 loop → `clamp_retry_sleep` at `client.rs:580` fires.
- RUNNING-storm: poll returns `{"status": "RUNNING"}` → `is_terminal()` false → control returns to `bulk.rs:495-498` for the post-poll sleep.

These are **two different overshoot vectors**. The 429-storm test would NOT catch a regression in the post-poll clamp, and vice versa. Each needs its own positive-coverage test.

**Recommended new test** (sketch):

```rust
//! NEW: tests/bulk_running_storm_respects_deadline.rs
//!
//! AC-NEW-B-1: when the bulk task returns "RUNNING" indefinitely, the polling
//! loop's post-poll backoff sleep must clamp to remaining-deadline so total
//! wall-clock cannot overshoot the deadline by more than POLL_MAX_SECS=10s.
//! Without the clamp, the schedule 1+2+4+8+10+10 reaches t=35 for a 30s
//! deadline — overshoot of 5s in this schedule, up to 10s in adversarial
//! schedules. With the clamp, the final sleep is shortened so the loop exits
//! within ~30-31s.

const WALL_CLOCK_BUDGET_SECS: u64 = 35;   // 30s deadline + ~5s CI slack
const WALL_CLOCK_FLOOR_SECS: u64 = 25;    // Guards against regression to instant-exit

// Mock returns status="RUNNING" 200 OK for every GET.
// JR_BULK_AWAIT_TIMEOUT_SECS=30 drives the timeout.
// Expect: jr issue edit subprocess exits non-zero in [25s, 35s].
```

**Test placement:** Sibling file `tests/bulk_running_storm_respects_deadline.rs` (not in the existing 429 test) — separation matches the existing pattern of one-AC-per-test-file in this repo (`bulk_unknown_grace_release_gate.rs`, `bulk_await_timeout_release_gate.rs`).

**Recommended next action:**
1. Apply Q1(a) clamp.
2. Add new integration test per Q1(c).
3. Update `bulk.rs:495-498` comment block to cross-reference B-1 / AC-NEW-B-1 anchors.

### Q2 — C-2 status-code-429 overload

**VERDICT: RECOMMEND-B (introduce `JrError::DeadlineExceeded`), with two corrections to prior pass-02 reasoning.**

#### Correction 1: The "hundreds of match sites" claim is wrong

Earlier pass-02 reasoning cited "~hundreds of match sites" as the taxonomy-churn cost. Actual count in `src/` (verified by grep `JrError::ApiError`):

| Site type | Count | File:line(s) |
|---|---|---|
| Construction (`return Err(JrError::ApiError { ... })`) | ~7 | `client.rs:470`, `client.rs:564`, `client.rs:583`, `client.rs:946`, plus a few in API layer |
| Match-arm downcast (`if let Some(JrError::ApiError { ... })`) | 6 in src + 5 in tests | `assets/workspace.rs:29`, `cli/user.rs:78`, `cli/issue/list.rs:210,316`, `jira/projects.rs:78` |
| Documentation reference (rustdoc) | ~4 | `jira/users.rs:240`, `client.rs:334,387` |

Of the **6 src-tree match arms**, ALL match on `status: 404` (resource not found). **None** of them match on `status: 429`. So introducing a `DeadlineExceeded` variant requires changing exactly the construction sites that were going to be touched anyway, plus updating any test that pattern-matches on the deadline-exceeded path.

The pass-02 "taxonomy churn" cost is essentially zero. The earlier decision to "stay with 429" was based on a miscount.

#### Correction 2: External precedent strongly favors a distinct variant

Perplexity 2026-05-12 survey of CLI patterns:

| Tool | Behavior |
|---|---|
| **kubectl** | Surfaces `context deadline exceeded` — NOT an HTTP code; exit 1 |
| **gh CLI** | Surfaces `context deadline exceeded` (from Go `context.DeadlineExceeded`); exit 1 |
| **aws-cli** | `Client.Timeout exceeded while awaiting headers` — client-side error, NOT 4xx |
| **doctl, fly CLI** | Same Go pattern |

> "Reusing 429 for 'deadline expired at request entry' is misleading — 429 implies *server-side* rate limiting after receipt. Use a distinct variant. Follow kube-rs/k8s-go pattern: `DeadlineExceeded` + exit code 1 + clear message. Never map to 429 (server received+rejected)."

Sources:
- <https://pkg.go.dev/k8s.io/apimachinery/pkg/api/errors> (k8s DeadlineExceeded variant)
- <https://github.com/kubernetes/apimachinery/blob/master/pkg/api/errors/errors.go>
- <https://github.com/kube-rs/kube-rs/issues/624>

The **adversary is correct** that "API error 429" for a request that never reached the server is misleading. Operators who script against `--output json` and see `"status": 429` in the JSON error envelope will reasonably assume Atlassian rate-limited them and check Atlassian status pages — when the actual cause was a client-side timeout misconfiguration.

#### Correction 3: Hybrid is the right scope

The adversary listed three sites:
1. **Real 429 cap-abort** (`client.rs:564`): Atlassian's response had `Retry-After > 60s`. This IS a real 429 — KEEP `ApiError(429)`.
2. **Entry-point check** (`client.rs:470`): No request issued. NO server interaction. Use `DeadlineExceeded`.
3. **In-loop clamp** (`client.rs:583`): A 429 was received, but we abandon the retry because deadline ran out. The 429 response is a fact, but the **error surfaced to the caller** is "we ran out of time", not "Atlassian rate-limited you fatally". Use `DeadlineExceeded`.

The recommendation is **(c) Hybrid** from the adversary's option list:

- KEEP `ApiError(429)` for the actual rate-limit cap-abort (`client.rs:564`).
- USE `JrError::DeadlineExceeded { remaining_ms: u64, message: String }` for the entry-point check (`client.rs:470`) AND the in-loop deadline-clamp (`client.rs:583`).

This matches operator mental-model: "429 = server told us to back off"; "DeadlineExceeded = our budget ran out".

**Exit code mapping:**
```rust
JrError::DeadlineExceeded { .. } => 1,  // Generic failure; matches kubectl/gh/aws-cli
```
or, if we want a dedicated exit code:
```rust
JrError::DeadlineExceeded { .. } => 124,  // POSIX `timeout(1)` convention
```
Either is defensible. POSIX 124 (the convention used by `coreutils timeout`) is operator-friendly for shell pipelines.

**Schema impact on `--output json`:**
The current shape:
```json
{"error": {"status": 429, "message": "Caller-supplied deadline already expired ..."}}
```
becomes:
```json
{"error": {"kind": "DeadlineExceeded", "remaining_ms": 0, "message": "Caller-supplied deadline already expired ..."}}
```
Note: per CLAUDE.md, JSON output has no `_meta: {version: N}` envelope yet (NFR-O-P deferred). A new variant here is technically a schema change visible to any script parsing the error JSON, but no script could possibly rely on this *new* behavior (it ships in S-333 — pre-fix there was no `429` here at all because the entry-point check didn't exist). So zero compatibility risk.

**Recommended next action:**
1. Add `JrError::DeadlineExceeded { remaining_ms: u64, message: String }` to `src/error.rs`.
2. Map exit code (recommend 124 to match POSIX `timeout(1)`).
3. Update `client.rs:470` and `client.rs:583` to construct `DeadlineExceeded` instead of `ApiError(429)`.
4. **Leave `client.rs:564` (the cap-abort) as `ApiError(429)`** — that one IS a real server-mediated 429.
5. Update test assertions in `tests/bulk_deadline_propagation.rs` (search for `.contains("429")` or `.contains("deadline")` patterns).
6. Update `send_bounded` rustdoc to mention the new error variant.

### Q3 — B-1 worst-case overshoot magnitude

**VERDICT: CONFIRMED. Adversary's "up to 10s past deadline" is correct as a worst-case bound. Operator-supplied trace produces 5s in this specific schedule, but the theoretical bound is exactly POLL_MAX_SECS=10s.**

#### Trace verification (operator-supplied schedule, deadline=30s)

| Wall-clock | Top-of-loop check | Action | `backoff` after |
|---|---|---|---|
| t=0 | 0 < 30 ✓ | poll, sleep 1s | 2 |
| t=1 | 1 < 30 ✓ | poll, sleep 2s | 4 |
| t=3 | 3 < 30 ✓ | poll, sleep 4s | 8 |
| t=7 | 7 < 30 ✓ | poll, sleep 8s | 10 |
| t=15 | 15 < 30 ✓ | poll, sleep 10s | 10 |
| t=25 | 25 < 30 ✓ | poll, **sleep 10s** | 10 |
| t=35 | 35 ≥ 30 ✗ | return `Err` | — |

**Overshoot in this schedule: 35 − 30 = 5s.** (Not 10s — the operator's prompt note was slightly conservative.)

#### Theoretical worst-case overshoot

The maximum possible overshoot occurs when the deadline check passes by an infinitesimal margin (`now = deadline − ε`) just before the longest sleep:

- Check at `t = D − ε` (passes, since D − ε < D).
- Sleep for POLL_MAX_SECS = 10s.
- Next check at `t = D − ε + 10 ≈ D + 10` → fires → return Err.
- **Overshoot ≈ 10s = POLL_MAX_SECS.**

Example: deadline = 21s. Schedule reaches t=15 (15 < 21 ✓), poll, sleep 10s, wake at t=25, check (25 ≥ 21 ✗), return. Overshoot = 4s. With deadline = 16s: t=15 check (15 < 16 ✓), sleep 10s, wake at t=25, return. Overshoot = 9s — approaches the bound. With deadline = 25.001s: same path, return at t=25, overshoot = 9.999s.

#### Key technical fact

`tokio::time::sleep` is **NOT cancellable mid-sleep** without `tokio::select!` or `tokio::time::timeout`. Once the sleep starts it runs to completion regardless of any subsequent state change. This is the architectural reason the overshoot bound is POLL_MAX_SECS, not zero.

Citation: Perplexity 2026-05-12, traced step-by-step:
> "Your code is correct as written for scenarios where a small overshoot is tolerable (e.g., background polling, soft timeouts). For hard deadlines, wrap with `tokio::select!`." (We're choosing the `Duration::min` clamp instead per Q1.)

(One earlier Perplexity reply claimed "0s overshoot" — that reply was incorrect; the corrected reasoning above was confirmed in a follow-up `reason` query that walked through the timeline iteration-by-iteration.)

**Recommended next action:** The adversary's "up to 10s" framing is exactly right. In the post-fix `bulk.rs:495-498` with the `Duration::min` clamp, overshoot drops to ~0s (bounded only by `tokio::time::sleep`'s own scheduler jitter, typically <1ms). The new integration test (Q1(c)) should assert `elapsed < 35s` (30s deadline + 5s CI slack) and `elapsed >= 25s` (lower-bound regression guard).

## Triage Adjustment Recommendations

Apply set delta vs the adversary's pass-02 proposal:

1. **B-1 fix:** Use `Duration::min` + `saturating_duration_since`, NOT `clamp_retry_sleep`, NOT `tokio::select!`.
2. **B-1 test:** Add **NEW** integration test `tests/bulk_running_storm_respects_deadline.rs` (RUNNING-storm path) — distinct from existing 429-storm test.
3. **C-2 fix:** REVERSE the pass-02 "stay with ApiError(429)" decision. Introduce `JrError::DeadlineExceeded { remaining_ms: u64, message: String }`, use it at TWO of the three sites (entry-point check + in-loop clamp), keep `ApiError(429)` at the cap-abort site (real Atlassian 429).
4. **C-2 exit code:** Recommend POSIX 124 (`timeout(1)` convention).
5. **C-2 doc update:** Update `send_bounded` rustdoc to enumerate possible error variants; cross-reference S-333 anchors.

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity search | 3 | Idiomatic clamp pattern (backoff/reqwest-retry/aws-sdk/kube-rs); CLI tool precedent for client-side timeout vs HTTP 429; `tokio::select!` vs `Duration::min` cost comparison |
| Perplexity reason | 1 | Step-by-step timeline walk to verify B-1 overshoot math; corrected an incorrect earlier Perplexity answer claiming 0s overshoot |
| Read | 5 | adv-pass-01.md, research-validation-pass-02.md, bulk.rs ranges (300-500), client.rs ranges (450-610), error.rs, bulk_deadline_propagation.rs (1-80) |
| Grep | 5 | Match-arm sites for `JrError::ApiError` in src + tests; ClampResult/clamp_retry_sleep references; POLL_BASE_SECS/POLL_MAX_SECS constants |
| Glob | 3 | Locate adv-pass-02 file (not present), enumerate bulk tests, enumerate prior research |
| Training data | 0 areas | All external claims cited; codebase claims are file:line. The "POLL_MAX_SECS=10s overshoot" reasoning is purely algebraic from the code constants. |

**Total MCP tool calls:** 4 Perplexity (3 search + 1 reason).

**Training data reliance:** low — external claims (CLI precedent, `tokio::select!` cost, k8s DeadlineExceeded variant) are all Perplexity-cited with URLs. Codebase claims are file:line from grep + Read. The math in Q3 is from first-principles (loop trace) and was independently re-verified by a second Perplexity `reason` call after the first reply was wrong.
