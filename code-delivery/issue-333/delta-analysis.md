---
document_type: delta-analysis
level: feature
phase: F1
issue: 333
producer: orchestrator
inputs:
  - "https://github.com/Zious11/jira-cli/issues/333"
  - "src/api/jira/bulk.rs"
  - "src/api/client.rs"
  - "src/api/rate_limit.rs"
input-hash: "[live-state]"
intent: bug-fix
severity: HIGH
feature_type: backend
scope: standard
regression_risk: HIGH
status: draft
timestamp: 2026-05-12
---

# Delta Analysis — Issue #333

**Title:** fix(bulk): propagate `await_bulk_task` deadline into `JiraClient::send` to prevent ~3min overshoot

## Problem Statement (Verified Against Code)

| Claim from #333 | Verification | Result |
|---|---|---|
| `await_bulk_task` checks deadline at TOP of each loop iteration | `src/api/jira/bulk.rs:344` — `if Instant::now() >= deadline { return Err(...) }` is the first statement in the loop | ✓ TRUE |
| Poll internally calls `JiraClient::send` | `bulk.rs:269` calls `self.get(path)` → `client.rs:248` calls `self.send(request)` | ✓ TRUE |
| `send()` retries up to 3× on 429 with `MAX_RETRY_AFTER_SECS=60` cap | `client.rs:12` `MAX_RETRIES=3`, `rate_limit.rs:12` `MAX_RETRY_AFTER_SECS=60` | ✓ TRUE |
| Worst-case overshoot ~180s past deadline | 3 retries × 60s sleep = 180s of sleep inside `send()` before deadline-check at top of next loop iteration | ✓ TRUE |
| Real-world worst case ~480s for 300s contract | 300s deadline + 180s 429-storm = 480s | ✓ TRUE (within order of magnitude) |

**Conclusion:** The bug premise is empirically correct.

## Intent & Severity Classification

| Field | Value |
|---|---|
| Intent | `bug-fix` |
| Severity | `HIGH` — major behavior violation (~60% overshoot of stated deadline) with workaround (avoid 429 storms) |
| Feature type | `backend` |
| Scope | `standard` (NOT trivial — touches core HTTP layer) |

## Impact Boundary

### Components MODIFIED

| File | Change |
|---|---|
| `src/api/client.rs` | Add `send_inner(req, deadline: Option<Instant>)` private helper. Keep `send(req)` thin wrapper (passes `None`). Add new public `send_bounded(req, deadline: Instant)`. |
| `src/api/jira/bulk.rs` | Replace `self.get(&path)` in `poll_bulk_task` with deadline-aware path. Plumb `deadline: Instant` through `await_bulk_task_inner` → poll. |

### Components NEW

None — minimal-blast-radius approach: introduce one new internal helper and one new public method.

### Components DEPENDENT (unchanged but in path)

| File | Reason |
|---|---|
| `src/api/jira/bulk.rs` (`bulk_edit_fields`, `bulk_transition`) | These call `self.post`. Not affected — they're the SUBMIT path, not the polling path. |
| `src/cli/issue/bulk.rs` (or wherever CLI handlers consume `await_bulk_task`) | Caller already supplies `timeout: Duration`. No signature change at CLI layer. |

### Components NOT CHANGED (regression baseline)

All other `self.send(request)` callers — confirmed 5 wrappers (`get`/`post`/`put`/`post_no_content`/`delete`) and they continue calling existing `send(req)` unchanged. **~Hundreds of call sites are zero-churn** because `send()` keeps its 1-arg signature.

`send_raw` (used by `cli/api.rs`) is NOT changed in this delta — it's the raw escape hatch for `jr api`, not a bulk-polling path. Defer to follow-up if `send_raw` ever feeds a deadline-aware path.

## Affected Artifact Map

### Behavioral Contracts

| BC | Status | Rationale |
|---|---|---|
| `BC-3.bulk.NNN` (new — proposed in F2) | NEW | "When `await_bulk_task` is given a `timeout`, the total elapsed wall-clock from invocation to error MUST NOT exceed `timeout + ε` where ε accounts for one final poll round-trip + control overhead. Specifically, 429 retry sleeps inside `JiraClient::send` MUST be bounded by `min(retry_after, deadline - now)`." |
| `NFR-R-D` (timeout-bounded behavior) | TIGHTENED | Existing NFR allows overshoot; tighten to assert bounded overshoot per new BC. |

### Stories Affected

| Story | Risk Zone |
|---|---|
| S-110-bulk (Wave 3, MERGED PR #348) | DEPENDENT — uses `await_bulk_task`. Regression test required. |
| WV2-SEC-01 / S-2.07 / others using `send()` | DEPENDENT but zero-churn (no signature change at `send`). |

### Verification Properties

| VP | Status |
|---|---|
| (new) VP-deadline-bounded-bulk-poll | NEW — wiremock-driven: 429 storm + 30s deadline → error within ~35s. |

### Existing Tests in Risk Zone

| Test | File | Reason |
|---|---|---|
| `tests/bulk_*` (all bulk integration tests) | `tests/` | Polling path covered; must still pass after signature change. |
| `await_bulk_task_with_grace_for_test` inline tests | `src/api/jira/bulk.rs:608+` | Direct callers of polling logic. |
| `send` 429-retry unit tests (if any in `client.rs` test module) | `src/api/client.rs` | Verify `None` deadline path is unchanged. |

Estimate: ~30 tests touch the polling path; ~5 directly touch retry logic. All MUST still pass.

## Recommended Design (Perplexity-validated 2026-05-12)

**Pattern:** Option 2 (`deadline: Option<Instant>` on the internal helper), Perplexity-validated as canonical for async Rust HTTP clients (cf. aws-smithy-runtime's `OperationTimeoutConfig`; *not* tower::timeout which only bounds per-request, not retry-sleep total).

**Why not Option 4 (outer `tokio::time::timeout` wrap):** insufficient — wrapping the whole `send` future cancels at deadline but doesn't prevent a 60s sleep from starting at `deadline - 1s`. The sleep itself must be bounded by `min(retry_after, remaining)`.

```rust
// New private helper — single place that knows about deadlines
async fn send_inner(&self, req: RequestBuilder, deadline: Option<Instant>) -> Result<Response> {
    // ... existing retry loop, but on 429:
    let actual_sleep = match deadline {
        Some(d) => {
            let remaining = d.saturating_duration_since(Instant::now());
            if remaining.is_zero() { return Err(deadline_exceeded(...)); }
            Duration::from_secs(delay).min(remaining)
        }
        None => Duration::from_secs(delay),
    };
    tokio::time::sleep(actual_sleep).await;
}

// Existing public API — zero churn
pub async fn send(&self, req: RequestBuilder) -> Result<Response> {
    self.send_inner(req, None).await
}

// New public method — only used by bulk poll
pub async fn send_bounded(&self, req: RequestBuilder, deadline: Instant) -> Result<Response> {
    self.send_inner(req, Some(deadline)).await
}

// New JSON-GET variant for the polling path
pub async fn get_bounded<T: DeserializeOwned>(&self, path: &str, deadline: Instant) -> Result<T> {
    let url = format!("{}{}", self.base_url, path);
    let response = self.send_bounded(self.client.get(&url), deadline).await?;
    let bytes = self.collect_response_body(response).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
```

**Bulk side:** `poll_bulk_task_with_deadline(task_id: &str, deadline: Instant)` calls `self.get_bounded(path, deadline)`. `await_bulk_task_inner` computes `deadline = Instant::now() + timeout` once and threads it.

## Edge Cases (Perplexity-flagged)

1. **Clock skew** — use `Instant::saturating_duration_since` (`Instant` arithmetic uses monotonic clock, so skew is bounded, but `saturating` defends against logical underflow).
2. **Deadline expired during 429 sleep** — return `JrError::Timeout` (or wrap as `DeadlineExceeded`) rather than letting the next request fire pointlessly.
3. **Deadline = 0 (immediate)** — first poll fires (retains existing semantic that the timeout-validate-task_id-error-message check still runs), then any 429 sleep is clamped to zero → immediate return.

## Risk Assessment

| Dimension | Risk | Rationale |
|---|---|---|
| Regression | **HIGH** | Core HTTP layer; `send_inner` refactor must preserve all existing 429 + 401 auto-refresh + verbose-logging semantics. |
| Architecture | **LOW** | No new modules; internal helper only. |
| Security | **LOW** | No new attack surface; if anything, prevents resource-starvation DoS via 429-driven overshoot. |
| Test infra | **MEDIUM** | New wiremock pattern needed: simulated 429 storm with `Retry-After: 60` repeated. Existing wiremock harness supports this. |

## Recommended Scope for F2–F7

- **F2:** 1 new BC (`BC-bulk.deadline-bounded-poll`); tighten NFR-R-D; new ADR for `Option<Instant>` propagation pattern if any (small ADR or none).
- **F3:** 1 new story (`S-333-deadline-propagation`) with 1 BC, 3 ACs (matches issue acceptance criteria).
- **F4:** TDD in worktree. Failing tests first (430-storm wiremock + normal-path regression).
- **F5:** Scoped adversarial — 1 axis (timing safety + clock-skew + retry-invariant) + 1 axis (regression on `None` deadline path).
- **F6:** Full regression suite + cargo clippy/fmt.
- **F7:** 5-dim convergence on delta + regression.

## Quality Gate Checklist

- [x] All affected components identified with change type (NEW/MODIFIED/DEPENDENT)
- [x] Regression risk assessed (HIGH on `send`; LOW elsewhere)
- [x] Existing tests in risk zone enumerated
- [x] Files NOT changed listed as regression baseline (`send_raw`, all 5 HTTP-method wrappers)
- [x] Feature type classified (`backend`)
- [x] Intent classified (`bug-fix`)
- [x] Severity classified (`HIGH`)
- [x] Trivial scope assessed (`standard`, NOT trivial)
- [x] Approach Perplexity-validated against tower/smithy/reqwest-middleware patterns
- [ ] **Human approves scope (pending)**
