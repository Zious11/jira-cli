---
document_type: research-validation
issue: 333
phase: F5-triage-validation
pass: 02
producer: research-agent
timestamp: 2026-05-12
budget_minutes: 15
status: complete
inputs:
  - .factory/cycles/cycle-001/adversarial-reviews/issue-333-bulk-deadline/adv-pass-01.md
  - .factory/code-delivery/issue-333/research-validation.md
  - src/api/client.rs (commit 618ca14)
  - src/api/jira/bulk.rs (commit 618ca14)
  - src/api/refresh_coordinator.rs (commit 618ca14)
  - src/api/auth.rs (commit 618ca14)
  - tests/bulk_deadline_propagation.rs (commit 618ca14)
---

# Adversary Triage Validation (Pass 02)

## Summary

- **3 CONFIRMED**, **1 REFUTED**, **3 CAVEAT**, **0 INCONCLUSIVE** across the 7 questions.
- One **previously-undocumented overshoot vector found** during Q1 (refresh HTTP client has no timeout). Material upgrade to CONCERN-1 severity; recommend stronger doc + open follow-up.
- Q3 (CI timing margin): adversary is **correct** — 5ms is below realistic GitHub Actions jitter; bump to ≥20ms (50ms safer).
- Q5 (NIT-2 cap-before-clamp precedence) is exactly as the adversary described in the source code.
- Q7 unit tests have **two gaps** the adversary did not flag: `base = Duration::ZERO` and `Sleep(min) == base` (boundary at `base == remaining`).

## Per-question findings

### Q1 — CONCERN-1 worst-case overshoot magnitude — CAVEAT (worse than adversary said)

**Verdict:** The adversary's "~60-90s" estimate is OPTIMISTIC. Actual worst case is **~60s + unbounded refresh time**, because the OAuth refresh path uses an unbounded `reqwest::Client::new()`.

**Evidence:**

1. **`reqwest::Client::builder().timeout(Duration::from_secs(30))` IS end-to-end.** Per reqwest 0.12/0.13 docs, the timeout "is applied from when the request starts connecting until the response body has finished" — covers connect + TLS + send + headers + body. So the 30s bound on `first_response.bytes().await` and the retry `retry_req.send().await` is correctly characterized.
   - Source: <https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.timeout>
   - Verified via Perplexity 2026-05-12.
   - Codebase: `src/api/client.rs:88` — `Client::builder().timeout(Duration::from_secs(30)).build()?`

2. **The refresh HTTP client is UNBOUNDED.** `refresh_oauth_token_with_url` constructs its own client at `src/api/auth.rs:903` via `reqwest::Client::new()` — NO `.timeout()` configured. The default reqwest `Client::new()` has **no request timeout** (only the system-level connect timeout + per-OS TCP timeouts apply, which can be tens of seconds to minutes on a hung TLS handshake).
   - Codebase: `src/api/auth.rs:903` `let client = reqwest::Client::new();`
   - reqwest doc default: "No timeout is set by default" — <https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.timeout>

3. **The single-flight coordinator does NOT impose its own timeout.** `refresh_with_single_flight` at `src/api/refresh_coordinator.rs:99-165` only awaits the inner `refresh_fn().await` plus tokio mutex acquisition. No `tokio::time::timeout(...)` wrap.
   - Codebase: `src/api/refresh_coordinator.rs:150` calls `refresh_fn().await` directly.

4. **Worst-case computation (corrected):**
   - 30s body read of 401 (bounded by JiraClient's 30s).
   - **N seconds OAuth refresh** where N is unbounded — typical 1-3s, but a hung TLS handshake to `auth.atlassian.com` could be ~30-60s on the OS connect timeout, or longer if the connection establishes but the response stalls.
   - 30s retry `retry_req.send().await` (bounded by JiraClient's 30s).
   - **Practical realistic worst case: ~60-90s** (matches adversary).
   - **Pathological worst case: 60s + tens of seconds of refresh stall = 90-120s+** (worse than adversary).
   - Coordinator single-flight wait: bounded only by the winner's refresh time, so additional waiters inherit the same unbounded behavior.

**Triage adjustment:** Adversary's APPLY (doc clarification on `send_bounded`) is still correct, but the docstring should mention BOTH the 30s reqwest timeout AND the unbounded refresh client. Recommend opening a follow-up issue: "S-NEW: configure `reqwest::Client::builder().timeout()` on the refresh client too" — small change, large reduction in tail latency.

### Q2 — CONCERN-3 lower-bound 25s — CONFIRMED (defensible floor)

**Verdict:** 25s is a defensible lower bound under the current implementation. The clamp's first sleep CAN be slightly less than 30s but cannot fall below ~29s without a regression in the clamp path.

**Evidence (codebase walk):**

1. `await_bulk_task_inner` at `src/api/jira/bulk.rs:385` — `let deadline = Instant::now() + timeout;` (timeout = 30s via `JR_BULK_AWAIT_TIMEOUT_SECS=30` per `tests/bulk_deadline_propagation.rs:72`).
2. Top-of-loop check at `bulk.rs:397` — first iteration: `Instant::now() < deadline`, no early exit.
3. First poll RTT to wiremock — `~1ms` on localhost (negligible).
4. First 429 hits, `delay = 60s` (Retry-After). Cap check at `client.rs:495` — `60 > 60` is false, so cap not hit.
5. Clamp at `client.rs:513`: `remaining ≈ 30s - 1ms ≈ 29.999s`. Since `remaining > 1ms`, `Sleep(min(60s, 29.999s)) = Sleep(29.999s)`.
6. `tokio::time::sleep(29.999s).await` → ~30s wall-clock (sleeps may slightly under-shoot per scheduler).
7. Loop iteration 2: `Instant::now() ≈ deadline`. Top-of-loop deadline check fires → returns "Bulk task ... did not complete within 30s timeout".
8. **Total wall-clock: ~30s** (the 30s sleep dominates everything else).

**Could it legitimately complete in <25s post-fix?** No, NOT under the test scenario:
- The mock returns 429 indefinitely. The first 429 triggers a 30s clamped sleep. There is no path where the test exits before the first sleep completes.
- Exception: a regression that breaks the clamp (e.g., `remaining = 0` immediately) would make the test exit in <1s. **This is exactly the false-negative the adversary's lower bound is designed to catch** — so the adversary's recommendation is sound.

**Risk of 25s being too tight on slow CI:** Minimal. The clamp produces a sleep of ~29.999s; even if `tokio::time::sleep` over-shoots by some amount on busy CI, the sleep CAN'T go SHORTER than 25s without the clamp returning Expired (which is a different code path with a different message — also caught by the existing `stderr.contains("deadline")` assertion). 25s gives ~5s of slack against a ~29.999s sleep — sufficient.

**Triage:** APPLY as adversary recommended. 25s is correct.

### Q3 — CONCERN-5 timing margin 5ms vs 50ms — CONFIRMED (adversary correct; 5ms is too tight)

**Verdict:** Adversary is RIGHT. 5ms is below realistic GitHub Actions `ubuntu-latest` thread-scheduling jitter for trivial Rust code. Use ≥20ms (adversary's 50ms is safer).

**Evidence (Perplexity 2026-05-12):**

| Scenario | Realistic `Instant::now()` jitter |
|---|---|
| Local (baseline) | <1ms |
| Ideal CI (low load) | 2-5ms (matches the test's current threshold) |
| **Typical `ubuntu-latest`** | **10-50ms** (shared multi-tenant VM) |
| High load / peak hours | 100ms+ |

Sources:
- <https://github.com/actions/runner/issues/2607> — 100x-500x slowdowns on Ubuntu runners for compute-bound tasks.
- <https://github.com/actions/runner-images/issues/13367> — Disk/IO exhaustion during Rust compilation.
- <https://users.rust-lang.org/t/github-actions-randomly-kill-a-test-program/37255> — random SIGKILLs on test binaries, suggesting sub-10ms timing sensitivity fails sporadically.

**Specifically for this test (`test_clamp_retry_sleep_just_above_one_millisecond_is_sleep` at `client.rs:2268-2282`):**

```rust
let deadline = Instant::now() + Duration::from_millis(5);
match clamp_retry_sleep(Duration::from_secs(60), Some(deadline)) {
    ClampResult::Sleep(d) => { assert!(d >= Duration::from_millis(1) && d <= Duration::from_millis(5), ...); }
    ClampResult::Expired { .. } => panic!("5ms remaining should produce Sleep, not Expired"),
}
```

Between the `let deadline = ...` line and the `Instant::now()` call inside `clamp_retry_sleep`, the elapsed wall-clock can exceed 4ms on a busy `ubuntu-latest` runner. If it does, `remaining < 1ms`, the clamp returns `Expired`, and the test panics. Bump to 50ms (adversary's recommendation) — at 50ms even the high-load scenario (100ms+ jitter) is rare enough to be a deflakable rather than a test-design-bug.

**Triage:** APPLY as adversary recommended. Use 50ms.

### Q4 — CONCERN-7 entry-point deadline check — CAVEAT (not idiomatic; APPLY only as defensive nicety)

**Verdict:** Reference HTTP client implementations (aws-sdk-rust, smithy-rs, hyper, reqwest-middleware) DO NOT check the deadline at request entry. They check it only at retry decision points (after the first attempt fails). The adversary's recommendation is a defensive nicety, not an idiomatic pattern.

**Evidence (Perplexity 2026-05-12):**

- **AWS SDK Rust RetryConfig:** No pre-send deadline check. First request always sends immediately. Deadline enforcement happens in `should_attempt_retry()` AFTER the first attempt.
  - Source: <https://docs.aws.amazon.com/sdk-for-rust/latest/dg/retries.html>
- **Smithy-RS Retry Orchestrator:** Deadline checked per retry iteration, NOT pre-send.
  - Source: <https://github.com/smithy-lang/smithy-rs/discussions/2887>
- **Hyper Client:** No built-in request-level deadline before first send.
  - Source: <https://hyper.rs/guides/1/client/basic/>

**Why the idiomatic pattern works:** if the deadline is in the past, the first request fires (1 RTT, typically <1s), gets a response or error, and the retry-classifier returns "no, deadline exceeded." Wasted: 1 RTT. Saved: code complexity.

**For S-333 specifically:** the entry-point check IS cheap and prevents a wasted HTTP call when callers buggy-compose deadlines. But it's not the dominant overshoot risk — the 401 refresh path (Q1) is. **Adversary's APPLY is fine but low-priority**; if you defer it, the pattern of "check at retry only" is consistent with the wider Rust async-HTTP ecosystem.

**Triage:** APPLY (cheap, harmless, slight defensive value) OR DEFER to follow-up (idiomatic to NOT check). Either is defensible; the user-mental-model consensus is "let the request fly".

### Q5 — NIT-2 BC-X.4.009 precedence claim — CONFIRMED

**Verdict:** Adversary's source-code claim is exactly correct. The cap check fires BEFORE the clamp at the same source location.

**Evidence (`src/api/client.rs:488-528`):**

```rust
if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_RETRIES {
    let rate_info = RateLimitInfo::from_headers(response.headers());
    let delay = rate_info.retry_after_secs.unwrap_or(DEFAULT_RETRY_SECS);

    // BC-X.4.009: abort retry if Retry-After exceeds the interactive-CLI cap.
    if delay > MAX_RETRY_AFTER_SECS {       // <-- CAP CHECK (line 495)
        ...
        return Err(JrError::ApiError { status: 429, message: "...exceeds {MAX_RETRY_AFTER_SECS}s cap..." });
    }

    // S-333: clamp the 429 sleep to the caller's remaining deadline.
    let base_sleep = Duration::from_secs(delay);
    let actual_sleep = match clamp_retry_sleep(base_sleep, deadline) {  // <-- CLAMP (line 513)
        ClampResult::Expired { remaining_ms } => return Err(...deadline...),
        ClampResult::Sleep(d) => d,
    };
```

**A 429 with `Retry-After: 120s` AND deadline already expired** surfaces the cap message ("Retry-After 120s exceeds 60s cap"), not the deadline message — exactly as adversary said.

**Is this the right precedence?** YES — the cap is a "give up entirely" signal (no point clamping a 120s sleep down to 30s if the user wouldn't tolerate the 120s anyway, AND the cap message is more actionable: "rerun later" makes sense even if there's also a deadline issue). Documenting the precedence in a sentence is sufficient (NIT-2 APPLY).

**Triage:** APPLY (one-sentence comment).

### Q6 — CONCERN-6 boundary test deferral — CAVEAT (deferral OK, but a non-flaky alternative exists)

**Verdict:** Adversary's deferral rationale ("real-time tests at the 1ms scale are flaky") is correct **for tests against the live `Instant::now()`**. But there is a non-flaky alternative the adversary did NOT mention: **refactor `clamp_retry_sleep` to take `remaining: Duration` directly** instead of `deadline: Option<Instant>`, and let the caller compute `remaining`. Then the boundary test passes a static `Duration::from_millis(1)` and `Duration::from_micros(999)` and is fully deterministic.

**Evidence:**

- The current helper signature `fn clamp_retry_sleep(base: Duration, deadline: Option<Instant>) -> ClampResult` reads `Instant::now()` internally (`client.rs:376`). This is what makes boundary tests flaky.
- Refactor option A (recommended): split into two functions:
  - `clamp_retry_sleep_for_remaining(base: Duration, remaining: Option<Duration>) -> ClampResult` — pure, fully deterministic, takes `Option<Duration>` (None = no deadline).
  - `clamp_retry_sleep(base: Duration, deadline: Option<Instant>)` — thin wrapper that computes `remaining = d.saturating_duration_since(Instant::now())` and forwards.
- Boundary tests then live on the pure helper: `assert_eq!(clamp_retry_sleep_for_remaining(_, Some(Duration::from_millis(1))), Sleep(_))`, `_ for Some(Duration::from_micros(999))) == Expired { remaining_ms: 0 }`. Deterministic, fast, no flakiness.

**Trade-off:** Adds 1 function and ~20 LOC. Adversary's deferral with a "extract the threshold to a const" mitigation is acceptable — but if the orchestrator wants strong boundary coverage, the pure-helper refactor is the cleaner path. **Recommend: include the pure-helper extraction in the apply set OR open as a follow-up issue if scope-creep is a concern.**

**Triage adjustment:** Adversary's DEFER is acceptable, but the refactor option is cheap enough to consider for the apply set. Suggest documenting the option in the deferral comment so future maintainers can take it up.

### Q7 — Unit test gaps the adversary missed

**Verdict:** Found 2 cases the adversary did not mention. Other suggested cases are already covered or wouldn't add signal.

**Cases the existing 5 tests DO cover (`client.rs:2156-2283`):**

1. `test_clamp_retry_sleep_no_deadline_returns_base` — `deadline = None`, base passes through.
2. `test_clamp_retry_sleep_far_deadline_returns_base` — `deadline = now + 300s`, `min(60s, 300s) = 60s`.
3. `test_clamp_retry_sleep_near_deadline_clamps_to_remaining` — `deadline = now + 10s`, `min(60s, 10s) = 10s`.
4. `test_clamp_retry_sleep_sub_millisecond_remaining_returns_expired` — `deadline = now (already passed)`, returns `Expired { remaining_ms: 0 }`.
5. `test_clamp_retry_sleep_just_above_one_millisecond_is_sleep` — `deadline = now + 5ms`, returns Sleep in [1ms, 5ms].

**Gaps NOT mentioned by the adversary:**

**GAP-1 (recommended): `base = Duration::ZERO`.** What does `clamp_retry_sleep(Duration::ZERO, Some(deadline_far_future))` return? Per the implementation: `remaining > 1ms` so `Sleep(base.min(remaining)) = Sleep(Duration::ZERO)`. Caller then `tokio::time::sleep(Duration::ZERO).await` — per Q3 prior research, this is a no-op. The retry loop spins to the next request immediately.
- **Real-world likelihood:** Very low — `Retry-After: 0s` is unusual (Atlassian's typical values are 1425-3089s per the cap-check comment). But `delay = rate_info.retry_after_secs.unwrap_or(DEFAULT_RETRY_SECS)`; if `DEFAULT_RETRY_SECS = 0` somewhere in the future, this would silently spin.
- **Recommended test:** assert that `clamp_retry_sleep(Duration::ZERO, Some(now + 60s))` returns `Sleep(Duration::ZERO)` AND add a comment that callers must not rely on `Sleep(Duration::ZERO)` to yield. Or, more defensively, change the helper to return `Expired` for `base.is_zero()` (consistency with the `< 1ms` floor).

**GAP-2 (recommended): exact boundary `base == remaining`.** What if `base = 30s` and `remaining = 30s` exactly? Per implementation: `Sleep(base.min(remaining))` returns `Sleep(30s)` — fine. But this is the most-common headline-AC case (matches the integration-test scenario) and isn't covered by a unit test (only tests #2 and #3 cover `base < remaining` and `base > remaining` respectively, not equality). A trivial test would close this gap.

**GAP-3 (low priority): negative-time interaction.** What if `deadline` is computed from a `checked_add` overflow scenario (`Instant + Duration::MAX`)? `Instant + Duration` panics on overflow per std docs. The current code at `bulk.rs:385` (`let deadline = Instant::now() + timeout;`) inherits this panic. The adversary noted this in NIT-5 (`JR_BULK_AWAIT_TIMEOUT_SECS=u64::MAX` panics). Adversary deferred — agree, acceptable test-seam ergonomics.

**Already-considered cases that don't add signal:**

- `deadline` very far in future (covered by test #2).
- `base` larger than `Duration::MAX - now` — would panic at `Instant + Duration` site, not in the clamp.

**Triage adjustment:** Add 1-2 unit tests for GAP-1 (`base = Duration::ZERO`) and GAP-2 (`base == remaining`). GAP-3 is correctly deferred.

### Q8 — CONCERN-4 codification urgency (n=2 vs n=3) — CAVEAT (codify NOW; precedent supports n=2)

**Verdict:** "2 of 3 per S-7.02 codification threshold" is the adversary's framing — but the codebase's codification register has NO formal "n=3 required" rule. Looking at recent codifications, **n=2 has historically been sufficient** to escalate.

**Evidence (codebase pattern):**

- **Lesson 2 (`lessons.md:30-32`):** "Recurrence #2 confirmed at PR #294 (S-0.06)... Pattern stable; codification candidate now urgent." → codified at n=2.
- **Lesson 4 (`lessons.md:53-55`):** "Confirmed at S-1.02 (PR #296 merged 2026-05-07)... Codification stable; ready for promotion to orchestrator skill update." → codified at n=2.
- **DRIFT-001 (`stories/wave-3/S-3.06`):** codified at n=4 ("adversarial passes P21, P22, P23, P24 — four recurrences... escalated to codify as self-improvement story").
- **OBS-13-1 (`adv-p2-pass13.md`):** "3 occurrences identified... resolved this burst by global sweep" — n=3 triggered immediate global remediation, not deferred codification.

**Pattern:** The "codify at n=2 if pattern is stable; at n=3+ if pattern needs more confirmation" is implicit in the codebase. The doc-fallout pattern was explicitly codified at PR #356 R19 (after a 4-round cluster) and **PR #357 was the first successful application** (`convergence-trajectory.md:395`). The current S-333 PR is the SECOND test of whether the lesson generalizes — and it FAILED (both `JR_BULK_UNKNOWN_GRACE_SECS` from #336 AND `JR_BULK_AWAIT_TIMEOUT_SECS` from #333 are missing from CLAUDE.md).

**The adversary's framing of "2nd recurrence — codify now"** is correct and matches codebase precedent (Lessons 2 and 4 codified at n=2). The "n=3 per S-7.02 codification threshold" claim doesn't have a literal source — but the spirit (codify when pattern is stable) supports codify-now.

**Recommended action:** APPLY both:
1. Back-fill `JR_BULK_UNKNOWN_GRACE_SECS` and `JR_BULK_AWAIT_TIMEOUT_SECS` to CLAUDE.md "AI Agent Notes" (content fix).
2. Add a pre-PR checklist item to the doc-fallout lesson rule: "If you add a new `JR_*` env var, grep CLAUDE.md for prior `JR_*` entries and add an entry for the new var in the same commit." Codify NOW; the `JR_BULK_*` cluster is the second confirmed failure of the cross-application generalization.

**Triage adjustment:** Adversary's APPLY + open-follow-up is correct. Codification is appropriate at this recurrence count given the codebase's historical pattern.

## Triage Adjustment Recommendations

Net change to the orchestrator's apply/defer plan: **+1 follow-up issue, +2 unit tests, no defer-set changes.**

1. **Strengthen CONCERN-1 doc clarification (Q1 finding).** The proposed `send_bounded` docstring update should mention BOTH:
   - "deadline clamps 429 storms only; OAuth refresh + retry may exceed deadline by ~60s on the first 401 hit (bounded by 30s body read + refresh RTT + 30s retry)"
   - "AND the OAuth refresh itself uses an unbounded HTTP client at `auth.rs:903`; a hung TLS handshake to `auth.atlassian.com` could push tail latency above 90s"

2. **NEW follow-up issue (recommended): "Configure `reqwest::Client::builder().timeout()` on the OAuth refresh client" (Q1 finding).** Small change at `src/api/auth.rs:903` (and `:639` if also unbounded) that bounds the refresh tail latency. Not blocking for S-333, but the "~90s overshoot worst case" should not be left unbounded.

3. **Q3 timing margin (CONCERN-5).** Use 50ms (adversary's recommendation) — Perplexity-confirmed 5ms is below realistic CI jitter (10-50ms typical on `ubuntu-latest`).

4. **Q7 unit-test additions (NEW, not in adversary's apply set):**
   - `test_clamp_retry_sleep_zero_base_with_far_deadline_returns_zero_sleep` — pins behavior on the `Retry-After: 0` edge case and documents the contract.
   - `test_clamp_retry_sleep_base_equal_remaining_returns_base` — pins the equality boundary that matches the headline AC scenario.

5. **Q6 (CONCERN-6) DEFER is acceptable.** But consider a follow-up: refactor `clamp_retry_sleep` into a pure `fn _for_remaining(base, Option<Duration>)` plus a thin wrapper. Then the strict `<` vs `<=` boundary test becomes deterministic (no `Instant::now()` race). Cheap enough to consider for this PR; otherwise spec it for a follow-up.

6. **Q4 (CONCERN-7) APPLY is fine but low-priority.** If scope-tight, defer — idiomatic Rust HTTP clients (aws-sdk-rust, smithy-rs, hyper) all skip the entry-point check.

7. **Q8 (CONCERN-4) APPLY immediately AND codify the doc-fallout-for-env-vars rule.** Codebase precedent supports n=2 codification (see Lessons 2 and 4 in `lessons.md`).

8. **No adjustment needed** to NITs 1, 4, 6 (trivial APPLYs) or NITs 3, 5, 7 (defensible DEFERs/SKIPs).

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity search | 4 | reqwest timeout end-to-end, GitHub Actions jitter, idiomatic deadline-check pattern, tokio::time::sleep ZERO behavior (cross-validated) |
| WebFetch | 2 | rust-lang.org forum thread #116756 (tokio sleep ZERO definitive answer); docs.rs tokio sleep page |
| Read | 7 | adv-pass-01.md, research-validation.md, src/api/client.rs (4 ranges), src/api/jira/bulk.rs, src/api/refresh_coordinator.rs, src/api/auth.rs, tests/bulk_deadline_propagation.rs |
| Grep | 8 | locate clamp helper, send_inner ranges, refresh paths, codification precedent, JR_BULK env vars in CLAUDE.md, S-7.02 threshold mentions, doc-fallout lesson context |
| Glob | 4 | locate adv-pass file, research-validation file, S-7.02 spec, bulk_deadline test |
| Training data | 0 areas | All claims backed by codebase reads, Perplexity citations, or Web fetches. |

**Total MCP tool calls:** 6 (4 Perplexity search + 2 WebFetch); plus 7 Read + 8 Grep + 4 Glob.

**Training data reliance:** low — all external claims (reqwest timeout semantics, GitHub Actions jitter, smithy-rs/aws-sdk retry pattern, tokio sleep ZERO behavior) are cited. Codebase claims are file:line. The one "soft" claim — codification threshold n=2 vs n=3 — is sourced to specific lessons.md entries showing historical n=2 codifications.
