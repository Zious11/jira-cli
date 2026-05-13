---
document_type: research-validation
issue: 333
phase: F3-pre-approval
producer: research-agent
timestamp: 2026-05-12
budget_minutes: 10
status: complete
---

# S-333 Research Validation

## Summary

- **6 of 8 questions CONFIRMED** with citations. Story's core technical premises (Instant monotonicity, saturating semantics, wiremock+subprocess pattern, deadline-clamp design) are sound.
- **1 question has a HARD CAVEAT (Q3, fixes a real bug in the story).** `tokio::time::sleep(Duration::ZERO).await` is documented as a NO-OP that does NOT yield to the executor. The implementation sketch in the story calls `tokio::time::sleep(actual_sleep).await` after computing `actual_sleep = base_sleep.min(remaining)`. If `remaining` is positive but < 1ms (sub-millisecond window between deadline checks under contention), the sleep silently no-ops and the loop spins. The early-return-on-zero path in the sketch is correct, but should be tightened to early-return when `remaining < 1ms` (tokio timer floor) — not only when `remaining.is_zero()`.
- **1 question is INCONCLUSIVE (Q8).** Atlassian does NOT publish endpoint-specific rate-limit documentation for `/rest/api/3/bulk/queue/{taskId}`. We are extrapolating from the general Jira Cloud rate-limit contract (which applies across all v3 REST endpoints). Treat the 429-on-poll assumption as defensive-coding, not documented behavior. This does not block F3 approval, but should be added to story `## Risk / Notes`.

## Per-question findings

### Q1: Wiremock + subprocess + Retry-After — CONFIRMED

**Verdict:** The pattern is correct and idiomatic. `MockServer::start()` binds to `127.0.0.1:<random>`, accessible from subprocesses launched by `assert_cmd::Command`. Wiremock 0.6 passes the `Retry-After` header verbatim as a string — no parsing/transformation, so reqwest 0.13 on the subprocess side sees `Retry-After: 60` exactly as configured.

**Citations:**
- `https://docs.rs/wiremock/latest/wiremock/struct.MockServer.html` — `MockServer::start()` binds to 127.0.0.1 random port; `mock_server.uri()` returns the full URL.
- `https://github.com/LukeMathWalker/wiremock-rs` — example patterns for subprocess testing via env var injection.
- `https://oneuptime.com/blog/post/2026-01-07-rust-wiremock-mocking/view` — `register_as_scoped` pattern with `MockGuard` for auto-verification.

**Lifecycle gotcha (low-risk for this story):** `MockServer` must remain alive (i.e., not dropped) until the subprocess completes. The standard `Command::output().await` / `.assert()` pattern in `tests/` already handles this — drop order keeps the server alive across the await. No action needed if you follow the existing `tests/bulk_*.rs` patterns.

**Confirmed against codebase:** The existing `tests/` directory uses exactly this pattern via `JR_BASE_URL`. The pattern in the story is consistent.

### Q2: `Instant::saturating_duration_since` semantics — CONFIRMED

**Verdict:** All three sub-questions confirmed.

1. **Monotonicity on macOS/Linux/Windows (Tier 1):** Yes. Linux uses `CLOCK_MONOTONIC`, macOS uses `mach_absolute_time`, Windows uses `QueryPerformanceCounter`. All are documented as monotonic non-decreasing in `std::time::Instant`'s Rust std docs.
2. **`saturating_duration_since` returns `Duration::ZERO`:** Confirmed. Per Rust std docs: *"Returns the amount of time elapsed from another instant to this one, or zero duration if that instant is later than this one."* Cannot panic.
3. **No race-condition path where `Instant::now() > deadline` would NOT immediately produce `Duration::ZERO`:** None. `Instant::now()` is monotonic across threads on Tier 1 platforms. Even if you read `Instant::now()` twice and the second is "stale" (older value visible due to thread scheduling), `saturating_duration_since` will still return `Duration::ZERO` because the saturation is computed at call-time, not capture-time.

**Note on Rust 1.60+:** Since 1.60.0, even `Instant::duration_since` is saturating (no longer panics). `saturating_duration_since` is preferred because it makes intent explicit. The story's AC-004 mandating `saturating_duration_since` is correct.

**Citations:**
- `https://doc.rust-lang.org/std/time/struct.Instant.html#method.saturating_duration_since` — official std docs.
- `https://github.com/rust-lang/rust/issues/133525` — `duration_since` is now saturating; `saturating_duration_since` remains the explicit-intent option.
- `https://releases.rs/docs/1.60.0/` — Rust 1.60 release notes documenting the saturating change.

### Q3: `tokio::time::sleep(Duration::ZERO)` behavior — REFUTED (story has a latent bug)

**Verdict:** The story implementation sketch is subtly wrong. `tokio::time::sleep(Duration::ZERO).await` is a NO-OP that does NOT yield to the executor.

**Evidence:**
- `tokio` Sleep implementation: `Duration::ZERO` (and past `Instant` values in `sleep_until`) immediately return `Poll::Ready` — no scheduling, no yield. Source: tokio's internal `Sleep` poll implementation (see linked discussions below).
- Tokio's timer wheel has a **1ms resolution floor**; sub-millisecond sleeps are treated as instant.
- For guaranteed yielding, use `tokio::task::yield_now().await`.

**Citations:**
- `https://users.rust-lang.org/t/does-awaiting-tokio-sleep-duration-zero-yield-back-to-the-runtime-or-poll-ready-immediately/116756` — direct Q&A confirming Poll::Ready immediately for `Duration::ZERO`.
- `https://github.com/tokio-rs/tokio/discussions/5996` — tokio maintainer discussion of timer-wheel resolution.
- `https://docs.rs/tokio/latest/tokio/time/fn.sleep.html` — official sleep docs.

**Impact on story:** The current sketch:

```rust
let remaining = d.saturating_duration_since(Instant::now());
if remaining.is_zero() {
    return Err(...);
}
base_sleep.min(remaining)  // could be 1µs!
```

If `remaining` is positive but sub-millisecond (e.g., 500µs because two `Instant::now()` calls between the top-of-loop check and the clamp computation straddled the deadline), the resulting `actual_sleep` is < 1ms. `tokio::time::sleep` no-ops, the loop spins back to the next request, that fires immediately, gets another 429, recomputes `remaining` = 0, and finally returns Err. Spinning 1–2 extra retries is technically a correctness violation of the "bounded overshoot" BC.

**Recommendation:** Change the clamp to:

```rust
let remaining = d.saturating_duration_since(Instant::now());
if remaining < Duration::from_millis(1) {
    return Err(...);
}
base_sleep.min(remaining)
```

OR equivalently, gate on `remaining.is_zero() || remaining < tokio_timer_floor` (1ms is the safe floor). The story's AC-005 message is fine; just tighten the threshold from "is_zero" to "< 1ms" in both the sketch and AC-004 test.

### Q4: Test seam — env var with `#[cfg(debug_assertions)]` — CONFIRMED + REFINED

**Verdict:** The proposed `JR_BULK_AWAIT_TIMEOUT_SECS` debug-only env var is the right choice. It mirrors three existing patterns in the codebase:

- `JR_BASE_URL` (debug-only, `src/config.rs:363` + `src/api/client.rs:47` — double-gated against token-leak vector)
- `JR_AUTH_HEADER` (debug-only, `src/api/client.rs:79` — SD-002)
- `JR_BULK_UNKNOWN_GRACE_SECS` (debug-only, `src/api/jira/bulk.rs:111` — closest precedent; same module, same purpose)

**Why this beats alternatives:**

| Pattern | Pro | Con |
|---|---|---|
| `#[cfg(debug_assertions)]` env var | Zero release-build footprint; matches existing module conventions; no public API churn | Requires `#[cfg(debug_assertions)]` discipline at BOTH read sites if there are multiple |
| `await_bulk_task_with_deadline_for_test` constructor | Compile-time only; no env var to forget | Forces test-only public symbol; harder to use from the subprocess path (subprocess can't call Rust APIs directly) |
| Public-path env var | Could be useful for ops debugging | Threat model: someone could set it in CI/prod and silently break long-running bulk ops |

**Refinement:** Pin the gate with a regression test analogous to `tests/base_url_release_gate.rs` (which already pins `JR_BASE_URL`'s debug-only gate). Call it `tests/bulk_await_timeout_release_gate.rs`. This keeps the test-seam contract immutable across releases.

**Citation:** Codebase evidence — `src/api/jira/bulk.rs:106-113` documents the `JR_BULK_UNKNOWN_GRACE_SECS` pattern explicitly as "mirrors the existing `JR_BASE_URL` and `JR_AUTH_HEADER` debug-only patterns." `S-333` should follow the same pattern verbatim.

### Q5: Tokio time-pause + wiremock — CONFIRMED (story's choice to avoid is correct)

**Verdict:** The story is right to avoid `tokio::time::pause` + `advance` for this test. The well-documented failure mode is:

1. `tokio::time::pause()` enables auto-advance to the next timer when the runtime becomes idle.
2. Wiremock's `MockServer` holds a real TCP listener on the test runtime, performing real `accept()` / `read()` I/O.
3. The I/O keeps the runtime non-idle, so auto-advance never fires.
4. `tokio::time::advance(N)` manually advances the test runtime's timer wheel — but **the subprocess has its own clock and runtime**. Manual advance in the test process has zero effect on the subprocess's sleeps.

**For a subprocess-driven test, time-pause is entirely the wrong tool.** The story's "real-time wall-clock test, accept ~30-35s" is the correct tradeoff. To shorten test runtime, the alternative is to make the test's TIMEOUT shorter (e.g., set `JR_BULK_AWAIT_TIMEOUT_SECS=5` and assert exit within 7s) — but then you're not exercising the full 429-storm clamp at all. The story's 30s budget is the right pick.

**Citations:**
- `https://github.com/tokio-rs/tokio/issues/4522` — canonical "auto-advance blocks on real I/O" issue.
- `https://github.com/tokio-rs/tokio/discussions/7237` — same problem, more recent.
- `https://tokio.rs/tokio/topics/testing` — official testing docs (mentions paused time but does NOT recommend it with real I/O).
- `https://www.ditto.com/blog/mocking-time-in-async-rust` — explicit recommendation: "pure mocks" only, not real-network mocks like wiremock.

**Refinement:** Document in the story's `## Risk / Notes` that "test runtime ~30s wall-clock is intentional; time-pause is incompatible with subprocess + wiremock per tokio #4522."

### Q6: Error message taxonomy — CAVEAT (reuse `JrError::ApiError` is best, but with a clear hint)

**Verdict:** Do NOT introduce a new `JrError` variant. The existing `JrError::ApiError { status: 429, message: ... }` is the right home. Adding a `DeadlineExhausted` variant adds taxonomy churn for one error site (~hundreds of call sites would now need to consider it) for no consumer benefit — exit code is 1 either way, JSON output already serializes the message.

**Recommendation:** Use `JrError::ApiError { status: 429, message: ... }` with a message that:
1. Contains the substring `"deadline"` (for AC-005 test).
2. Distinguishes from the existing `"Retry-After ... exceeds Ns cap"` message.
3. Suggests next action (rerun with larger timeout).

Suggested message:

```
"Bulk poll deadline exceeded during 429 retry (Retry-After {N}s, remaining budget {M}ms before clamp). \
 Atlassian rate-limit pressure consumed the caller-supplied timeout. \
 Rerun with a larger --timeout, or wait for rate-limit pressure to subside."
```

This:
- Self-describes which budget ran out (deadline, not the 60s cap).
- Mentions both `Retry-After` and `remaining budget` so operators can correlate against Atlassian's response and our internal timeout.
- Suggests two actionable next steps.

**Why not a new variant:**
- `JrError::ApiError` already carries `status: 429` — adequate classification.
- A `DeadlineExhausted` variant would require updating every `match` on `JrError` in the codebase (search for `JrError::ApiError` shows it's matched in error formatting + JSON serialization). Not worth it for one new site.
- The codebase comment in `src/error.rs:30-34` reserves `Internal` for "should never happen" bugs — deadline exhaustion is an expected operational state, not a bug, so `Internal` is wrong.

**Citation:** Codebase evidence — `src/error.rs:21-22` defines `ApiError { status: u16, message: String }`. The existing `"Rate limited; Retry-After Ns exceeds Ns cap"` message uses this variant, so a sibling message for deadline-exhaustion is the consistent choice.

### Q7: Simpler design — CONFIRMED (Option 2 is correct; no missed pattern)

**Verdict:** No fifth pattern is materially simpler given the constraint (near-zero churn at ~hundreds of `self.send(req)` call sites). Examined alternatives:

| Pattern | Verdict | Why rejected |
|---|---|---|
| Reduce `MAX_RETRIES` for bulk-poll path | Rejected | Same fundamental issue — even 1 retry × 60s = 60s overshoot on a 30s deadline. Doesn't solve the bounded-overshoot BC. |
| `RetryPolicy` struct passed instead of deadline | Rejected | More expressive but more API surface. `Option<Instant>` is the smallest sufficient abstraction. |
| `tower-retry` + `tower-timeout` composition | Rejected | Would require wrapping the entire `reqwest::Client` in a tower::Service stack — a multi-day refactor of the HTTP layer. Out of scope. `tower-timeout` also cancels the future at deadline rather than clamping the sleep, which is the wrong semantic per the delta-analysis's Option-4-rejection note. |
| `governor` crate (rate-limit-aware client) | Rejected | `governor` is a client-side rate-limiter (proactive), not a server-driven retry policy. Wrong tool. |
| `aws-smithy-runtime` OperationTimeoutConfig pattern | Already canonical for Option 2 | This IS what Option 2 mirrors per the delta-analysis. |
| `reqwest-retry` / `reqwest-middleware` | Rejected | Adds a dependency for one method's retry policy. Not zero-churn at call sites — would change `reqwest::Client` construction. |

**Recommendation:** Approve Option 2 as designed. No simpler pattern exists at this constraint level.

### Q8: Atlassian-specific bulk-poll 429 documentation — INCONCLUSIVE

**Verdict:** Atlassian publishes a single general rate-limiting page that applies to all v3 REST API endpoints. There is NO endpoint-specific documentation for `/rest/api/3/bulk/queue/{taskId}` confirming 429 behavior or `Retry-After` semantics. The bulk POST/PUT submit endpoints are documented as rate-limited; the polling endpoint (GET) is in the same API group and therefore inherits the contract by implication, but this is not explicit.

**What IS documented (general v3 REST API):**
- All v3 endpoints can return HTTP 429.
- 429 includes a `Retry-After` header in seconds.
- `RateLimit-Reason` may be `jira-burst-based` (per-endpoint token bucket) or other.
- Bulk endpoints share a token bucket; high concurrency on the poll endpoint could exhaust it.

**What is NOT documented:**
- Whether the polling endpoint specifically applies separate rate-limit accounting from the bulk submit.
- The maximum `Retry-After` value Atlassian will send (we assume bounded by 60s based on observation, but this is not an SLA).
- Whether 429 on the poll path can be retried indefinitely (vs. e.g., a max-burst quota that requires a longer cool-down).

**Citations:**
- `https://developer.atlassian.com/cloud/jira/platform/rate-limiting/` — general rate-limit doc.
- `https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/` — bulk ops API group reference (does not document polling-specific rate limits in fetchable content).

**Impact on story:** None blocking. The fix is defensive — IF the polling endpoint returns 429 with `Retry-After`, we bound the overshoot. If it never does in practice, the new code path is dead. Either outcome is correct. Add to `## Risk / Notes`:

> Atlassian does not publish endpoint-specific 429 behavior for `/rest/api/3/bulk/queue/{taskId}`. The fix is defensive against the general v3 REST API rate-limit contract. If the polling endpoint never returns 429 in production, the new clamp code path is exercised only by tests — that is acceptable; the regression invariant (AC-002) ensures no behavior change on the non-429 path.

## Recommendations

### Before F3 approval (MUST-fix in story)

1. **Q3 fix — change zero-threshold to 1ms-threshold.** Edit the implementation sketch and AC-004:
   - **Sketch (line 136):** Change `if remaining.is_zero()` to `if remaining < Duration::from_millis(1)` (tokio timer-wheel floor).
   - **AC-004:** Update test name / assertion to cover sub-millisecond remaining → immediate `Err`, not just zero.
   - Add a sentence to `## Risk / Notes`: "tokio::time::sleep treats Duration < 1ms as a no-op (no yield, no actual sleep). The clamp uses < 1ms (not == 0) as the early-return threshold to avoid a spin-loop in the sub-millisecond edge case."

2. **Q4 refinement — pin the gate with a release-test.** Add to `## Tasks` between step 6 and 7:
   - "**Green.** Add `tests/bulk_await_timeout_release_gate.rs` asserting the env var is ignored in release builds (mirrors `tests/base_url_release_gate.rs`)."

3. **Q6 refinement — adopt `JrError::ApiError`, not a new variant.** Edit AC-005 to explicitly say "Reuse `JrError::ApiError { status: 429, ... }` — do NOT introduce a new variant. Message MUST contain the substring `\"deadline\"`."

### Risks to flag if approved as-is (NICE-to-have additions to `## Risk / Notes`)

4. **Q5 documentation — note tokio-time-pause incompatibility.** Add: "Time-pause optimization is intentionally rejected; tokio #4522 confirms it does not work with subprocess + wiremock. Test runtime ~30s wall-clock is the intended budget."

5. **Q8 documentation — note Atlassian-spec inconclusiveness.** Add the paragraph from Q8 above.

### No-change items (story is correct as-is)

- **Q1, Q2:** Story's wiremock/subprocess pattern and `Instant::saturating_duration_since` choice are both correct.
- **Q7:** Option 2 design (`Option<Instant>` deadline param threaded into `send_inner`) is the cleanest possible at this constraint. No fifth pattern was missed.

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity search | 5 | Instant docs, tokio sleep behavior, wiremock subprocess, time-pause interaction, Jira bulk rate limits |
| Perplexity reason | 2 | Multi-part wiremock+subprocess+Retry-After (off-topic results, fell back to search); Instant + tokio sleep (knowledge-cutoff-limited, cross-validated against search) |
| WebFetch | 1 | Atlassian bulk ops API group page (content truncated, not useful) |
| Read | 2 | story.md + delta-analysis.md (the inputs under validation) |
| Grep | 3 | Verify codebase patterns for env-var gates, `#[cfg(debug_assertions)]`, `JrError` enum shape |
| Training data | 0 areas | All claims backed by Perplexity / Context7 / Grep against the codebase |

**Total MCP tool calls:** 7 (5 Perplexity search + 2 Perplexity reason); plus 1 WebFetch + 2 Read + 3 Grep.

**Training data reliance:** low — all answers are cited to Rust std docs, tokio docs, wiremock docs, codebase evidence, or explicit "INCONCLUSIVE" verdicts (Q8). The Perplexity-reason pass on Q1/Q2/Q3 returned off-topic results, so those answers were re-verified via Perplexity-search queries that returned correctly-sourced citations.
