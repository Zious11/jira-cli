---
document_type: story
story_id: "S-333"
title: "Propagate await_bulk_task deadline into JiraClient::send to prevent 429-storm overshoot"
wave: feature-followup
status: draft
priority: high
estimated_effort: small
tdd_mode: strict
bc_anchors:
  - BC-bulk.poll.deadline-bounded
holdout_anchors:
  - H-NEW-BULK-DEADLINE-001
nfr_anchors:
  - NFR-R-NEW-3
adr_refs: []
sd_refs: []
files_modified:
  - src/api/client.rs (extract send_inner; add send_bounded; add get_bounded)
  - src/api/jira/bulk.rs (poll_bulk_task_with_deadline; thread deadline through await_bulk_task_inner)
test_files:
  - tests/bulk_deadline_propagation.rs (new wiremock integration test)
  - src/api/client.rs (unit tests for send_inner clamp logic in inline #[cfg(test)] mod)
breaking_change: false
producer: orchestrator
version: "1.0.0"
last_updated: 2026-05-12
depends_on:
  - S-2.07
  - S-3.07
blocks: []
issue: 333
---

# S-333: Propagate `await_bulk_task` deadline into `JiraClient::send` to prevent 429-storm overshoot

## Context

`src/api/jira/bulk.rs::await_bulk_task_inner` checks the deadline at the top of
each polling loop iteration (line 344). Each poll round goes through
`self.poll_bulk_task` → `self.get` → `self.send`. `JiraClient::send` retries up
to `MAX_RETRIES=3` on HTTP 429 with sleeps capped at `MAX_RETRY_AFTER_SECS=60`.

If a 429-storm hits just before the deadline, a single poll can sleep up to
`3 × 60 = 180s` past the deadline before the next deadline check fires. For a
300s contract, real-world worst case is ~480s elapsed.

Issue #333 is the audit-followup tracking this regression.

This fix introduces a deadline parameter that propagates from `await_bulk_task`
through to the 429 retry sleep, clamping each sleep by
`min(retry_after_secs, deadline.saturating_duration_since(now()))`.

## Behavioral Contracts

**BC-bulk.poll.deadline-bounded** (new — defined in F2 spec evolution).
`await_bulk_task(timeout=T)` MUST return within `T + ε` where ε excludes 429
sleep time. The 429 sleep inside `send` MUST be clamped by the remaining
deadline; if the clamp produces zero, `send` MUST return `Err(JrError::...)`
rather than enter the sleep.

**Regression invariant.** All `send()` callers that do NOT pass a deadline
(i.e., all ~hundreds of existing callers using `self.send(request).await?`)
MUST observe **identical** behavior to today. No silent semantic shift on the
`None` path.

## Acceptance Criteria

**AC-001** (traces to BC-bulk.poll.deadline-bounded). Wiremock test
`tests/bulk_deadline_propagation.rs::test_bulk_429_storm_respects_deadline`:
mounts a `/rest/api/3/bulk/queue/{taskId}` endpoint that returns HTTP 429 with
`Retry-After: 60` indefinitely. The CLI invokes `jr issue edit` configured with
a 30-second deadline (test seam, e.g., `--timeout 30` if exposed, or a debug-only
env var like `JR_BULK_AWAIT_TIMEOUT_SECS=30`, gated `#[cfg(debug_assertions)]`).
Assertion: the command exits non-zero within 35 seconds of wall-clock time
(deadline + one in-flight poll RTT + small ε), not 180s+.

**AC-002** (traces to regression invariant). Existing test
`src/api/jira/bulk.rs::tests::test_await_bulk_task_*` continues to pass
unchanged. Specifically, the wiremock-driven `with_grace_for_test` tests
covering success/failure/grace paths show **zero behavioral change** on the
non-deadline path. Confirm by running the existing test suite green.

**AC-003** (traces to BC-bulk.poll.deadline-bounded — documentation arm).
`JiraClient::send_bounded` (or whichever public method exposes deadline
awareness) has rustdoc that:
1. References `BC-bulk.poll.deadline-bounded` by name.
2. States the clamp behavior: `sleep = min(retry_after, deadline - now)`.
3. Specifies the return semantics when the clamp produces zero
   (returns `Err`, NOT a zero sleep that proceeds to retry).

**AC-004** (traces to BC-bulk.poll.deadline-bounded — clock + tokio-timer safety).
Use `Instant::saturating_duration_since` (not `Instant::duration_since` or `-`)
when computing remaining time. **Threshold for early-return Err is
`remaining < Duration::from_millis(1)`**, NOT `remaining.is_zero()` — `tokio::time::sleep`
treats `Duration::ZERO` and all sub-millisecond values as a no-op (no yield, no
actual sleep) per tokio timer-wheel 1ms resolution floor (Q3 research-validation,
2026-05-12). Without the 1ms threshold, a sub-millisecond `remaining` produces
a spin-loop. Unit test:
`test_send_inner_clamp_on_sub_millisecond_remaining` calls `send_inner` with a
deadline that is 500µs in the future, mounts a 429 response, and asserts the
function returns `Err` IMMEDIATELY (< 100ms wall) without entering the sleep.

**AC-005** (traces to BC-bulk.poll.deadline-bounded — error message). When
`send_inner` aborts due to deadline-exhaustion mid-retry, the returned `Err`
contains text that distinguishes it from the existing
`"rate limited; Retry-After ... exceeds cap"` error. **Reuse
`JrError::ApiError { status: 429, message: ... }` — do NOT introduce a new
`JrError` variant** (Q6 research-validation, 2026-05-12: adding a variant
churns every match site for zero consumer benefit; `status: 429` already
classifies the error). Message MUST contain the substring `"deadline"`.
Suggested format:
`"Bulk poll deadline exceeded during 429 retry (Retry-After {N}s, remaining budget {M}ms before clamp). Rerun with a larger timeout, or wait for rate-limit pressure to subside."`.
Test `test_send_inner_deadline_exhaustion_error_distinguishable` asserts the
message contains `"deadline"` and the error variant is `JrError::ApiError`
with `status == 429`.

> ### ⚠️ SUPERSEDED — AC-005-v2 (effective, post-F5 pass-02)
>
> AC-005 above (the "do NOT introduce a new variant" decision) was REVERSED
> during F5 adversarial review pass-02. The original reasoning ("hundreds of
> match sites would need updating") was empirically incorrect — actual count
> is 6 src match arms, all on `404`, none on `429`. External CLI precedent
> (kubectl, gh, aws-cli, doctl, fly) unanimously uses a dedicated variant
> for client-side deadlines, NOT 4xx-code overloading.
>
> **Effective contract:**
> 1. New variant `JrError::DeadlineExceeded { remaining_ms: u64, message: String }`,
>    exit code **124** (POSIX `timeout(1)` convention).
> 2. Used at THREE call sites, all with site-tag prefix in the message:
>    - `[deadline:send-entry]` — `src/api/client.rs::send_inner` entry check
>    - `[deadline:429-retry]` — `src/api/client.rs::send_inner` clamp in 429 loop
>    - `[deadline:bulk-outer]` — `src/api/jira/bulk.rs::await_bulk_task_inner` top-of-loop
> 3. The 429-retry clamp fires BEFORE the BC-X.4.009 cap-vs-Retry-After
>    abort, so a 429 with `Retry-After > 60s` AND expired deadline surfaces
>    as `DeadlineExceeded` (exit 124), NOT `ApiError(429)` (exit 1). Per Q2
>    research-validation pass-04.
> 4. Test contract: `tests/bulk_deadline_propagation.rs::test_333_*` assert
>    `output.status.code() == Some(124)` AND stderr contains `"deadline"`.
>    Unit tests in `src/error::tests` pin the variant's Display + exit code.
>
> References:
> - `.factory/code-delivery/issue-333/research-validation-pass-03.md` Q2
>   (the reversal rationale).
> - `.factory/code-delivery/issue-333/research-validation-pass-04.md` Q1/Q2
>   (the propagation completion + precedence reorder).
> - `.factory/cycles/cycle-001/adversarial-reviews/issue-333-bulk-deadline/adv-pass-02.md`
>   C-2 (the original gap-finding).
> - `.factory/cycles/cycle-001/adversarial-reviews/issue-333-bulk-deadline/adv-pass-03.md`
>   C-1, C-2, C-3 (the propagation cluster).

**AC-006** (release-gate). `cargo test`, `cargo clippy -- -D warnings`,
`cargo fmt --check` all pass.

## Holdout Strategy

**H-NEW-BULK-DEADLINE-001** is the must-pass holdout. The wiremock probe in
AC-001 is the holdout-evaluator's primary check. The evaluator runs the bulk
command under a 60s wall-clock kill and asserts:
- Process exited within 40s with non-zero status, AND
- stderr contains a recognizable timeout message that mentions the task id
  (already produced by the existing top-of-loop deadline check).

## Implementation Sketch

```rust
// src/api/client.rs

async fn send_inner(
    &self,
    request: RequestBuilder,
    deadline: Option<Instant>,
) -> anyhow::Result<Response> {
    // ... existing 429 retry loop, but on the sleep branch:
    let base_sleep = Duration::from_secs(delay);
    let actual_sleep = match deadline {
        Some(d) => {
            let remaining = d.saturating_duration_since(Instant::now());
            // tokio timer wheel has a 1ms resolution floor — `sleep(Duration < 1ms)`
            // no-ops without yielding (tokio #4522). Early-return on sub-millisecond
            // remaining to prevent a spin-loop where the clamp produces a no-op
            // sleep and the retry fires immediately. Per Q3 research-validation.
            if remaining < Duration::from_millis(1) {
                let remaining_ms = remaining.as_micros() as u64 / 1000;
                return Err(JrError::ApiError {
                    status: 429,
                    message: format!(
                        "Bulk poll deadline exceeded during 429 retry \
                         (Retry-After {}s, remaining budget {}ms before clamp). \
                         Atlassian rate-limit pressure consumed the caller-supplied \
                         timeout. Rerun with a larger timeout, or wait for \
                         rate-limit pressure to subside.",
                        delay, remaining_ms
                    ),
                }.into());
            }
            base_sleep.min(remaining)
        }
        None => base_sleep,
    };
    tokio::time::sleep(actual_sleep).await;
    // ... existing loop continues
}

pub async fn send(&self, req: RequestBuilder) -> anyhow::Result<Response> {
    self.send_inner(req, None).await
}

pub async fn send_bounded(
    &self,
    req: RequestBuilder,
    deadline: Instant,
) -> anyhow::Result<Response> {
    self.send_inner(req, Some(deadline)).await
}

pub async fn get_bounded<T: DeserializeOwned>(
    &self,
    path: &str,
    deadline: Instant,
) -> anyhow::Result<T> {
    let url = format!("{}{}", self.base_url, path);
    let response = self.send_bounded(self.client.get(&url), deadline).await?;
    let bytes = self.collect_response_body(response).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
```

```rust
// src/api/jira/bulk.rs — poll_bulk_task variant + thread through

pub async fn poll_bulk_task_with_deadline(
    &self,
    task_id: &str,
    deadline: Instant,
) -> anyhow::Result<BulkOperationProgress> {
    validate_task_id(task_id)?;
    let path = format!("/rest/api/3/bulk/queue/{}", urlencoding::encode(task_id));
    self.get_bounded(&path, deadline).await
}

// await_bulk_task_inner: compute deadline once, pass to poll
async fn await_bulk_task_inner(&self, task_id: &str, timeout: Duration, ...) -> ... {
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() >= deadline { return Err(...) }
        let progress = self.poll_bulk_task_with_deadline(task_id, deadline).await?;
        // ... existing post-poll handling
    }
}
```

## Tasks (TDD order)

1. **Red.** Write failing test `tests/bulk_deadline_propagation.rs` covering AC-001
   (wiremock 429-storm + 30s deadline → < 35s exit).
2. **Red.** Write failing inline unit test in `src/api/client.rs` covering AC-004
   (already-expired deadline → immediate `Err`).
3. **Green.** Extract `send_inner(req, deadline: Option<Instant>)`; keep `send(req)` as
   thin wrapper. Add `send_bounded`, `get_bounded`.
4. **Green.** Add `poll_bulk_task_with_deadline`. Thread `deadline` through
   `await_bulk_task_inner`. Keep `poll_bulk_task` as deadline-unaware wrapper for
   any other (non-bulk) callers.
5. **Green.** Add the distinct error message per AC-005.
6. **Refactor.** Update rustdoc on `send_bounded` per AC-003 (reference
   `BC-bulk.poll.deadline-bounded`, document clamp semantics including the
   1ms-floor early-return).
7. **Green.** Add release-gate test `tests/bulk_await_timeout_release_gate.rs`
   asserting the `JR_BULK_AWAIT_TIMEOUT_SECS` env var is ignored in release
   builds (mirrors `tests/base_url_release_gate.rs`). This pins the
   `#[cfg(debug_assertions)]` gate against regression. Per Q4
   research-validation.
8. **Regress.** Run full test suite + clippy + fmt. AC-002 + AC-006.
9. **Verify.** Manually inspect the 429-storm test runtime to confirm it converges
   in ~30-35s (not 0s — that would mean clamp short-circuited too eagerly; not
   180s+ — that would mean clamp didn't fire).

## Files Modified

- `src/api/client.rs` — extract `send_inner`; add `send_bounded`, `get_bounded`;
  update rustdoc.
- `src/api/jira/bulk.rs` — add `poll_bulk_task_with_deadline`; thread deadline
  through `await_bulk_task_inner`; update inline rustdoc.

## Files NOT Modified (Regression Baseline)

- `send_raw` (escape hatch for `jr api`) — out of scope.
- All 5 HTTP-method wrappers (`get`, `post`, `put`, `post_no_content`, `delete`) —
  zero churn; they continue to call `self.send(request)` which is now a 1-line
  wrapper around `send_inner(req, None)`.
- All ~hundreds of CLI call sites that use these wrappers — zero churn.

## Risk / Notes

- **Refactor preservation.** `send_inner` must preserve ALL existing semantics:
  - 401 blanket auto-refresh path (S-3.03 v2)
  - `JR_OAUTH_TOKEN_URL` snapshot-before-await race-safety
  - `--verbose` / `--verbose-bodies` logging branches
  - `MAX_RETRY_AFTER_SECS=60` cap → abort path (BC-X.4.009)
  - Network error mapping via `JrError::NetworkError`
- **Test-time determinism.** Wiremock's `Retry-After: 60` response means the
  test could in theory sleep up to 60s on the FIRST attempt before the deadline
  fires. The clamp must ensure the first sleep is `min(60, 30) = 30s`. Test
  asserts `< 35s` (not `< 60s`).
- **Tokio time-pause potential optimization (REJECTED).** `tokio::time::pause` +
  `advance` is intentionally NOT used. Wiremock's `MockServer` holds a real TCP
  listener; the I/O keeps the runtime non-idle so auto-advance never fires
  (tokio #4522). For subprocess-driven tests, manual `advance` in the test
  process has zero effect on the subprocess's clock anyway. Real-time wall-clock
  (~30-35s) is the intended budget. Per Q5 research-validation.
- **Tokio timer-wheel 1ms floor.** `tokio::time::sleep(Duration::ZERO)` and any
  sub-millisecond `Duration` are no-ops that do NOT yield. The clamp uses
  `remaining < Duration::from_millis(1)` (not `is_zero()`) as the early-return
  threshold to avoid a spin-loop in the sub-millisecond edge case. Per Q3
  research-validation.
- **Atlassian-spec inconclusiveness.** Atlassian does NOT publish endpoint-specific
  429 behavior for `/rest/api/3/bulk/queue/{taskId}`. The fix is defensive
  against the general v3 REST API rate-limit contract. If the polling endpoint
  never returns 429 in production, the new clamp code path is exercised only
  by tests — that is acceptable; the regression invariant (AC-002) ensures no
  behavior change on the non-429 path. Per Q8 research-validation.
