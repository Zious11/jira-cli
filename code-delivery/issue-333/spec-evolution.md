---
document_type: spec-evolution
level: feature
phase: F2
issue: 333
producer: orchestrator
inputs:
  - ".factory/code-delivery/issue-333/delta-analysis.md"
  - ".factory/specs/prd/nfr-catalog.md"
  - ".factory/specs/prd/holdout-scenarios.md"
input-hash: "[live-state]"
status: draft
timestamp: 2026-05-12
---

# Spec Evolution — Issue #333

This is a delta document. PRD shards (`nfr-catalog.md`, `holdout-scenarios.md`) are
updated by **appending** the rows defined here at merge-time, not by full rewrite.
Mirrors the convention used by issue-110-pr2.

## New Behavioral Contract

### BC-bulk.poll.deadline-bounded

**Statement.** When a caller invokes `JiraClient::await_bulk_task(task_id, timeout)`,
the total elapsed wall-clock from invocation to a successful or `Err` return
MUST satisfy:

```
elapsed ≤ timeout + ε
```

where `ε` is the bounded overhead of:
1. ONE in-flight HTTP poll round-trip already-issued at the moment the deadline
   expires (cannot be cancelled mid-flight without dropping the response we may
   want to surface), AND
2. control-flow latency (sleep tick granularity, message construction).

In practice, `ε ≤ POLL_MAX_SECS + small constants`. **`ε` MUST NOT include any
429-driven `tokio::time::sleep` time inside `JiraClient::send`.**

**Rationale.** The current implementation checks the deadline at the top of the
polling loop but allows `JiraClient::send` to internally sleep up to
`MAX_RETRIES * MAX_RETRY_AFTER_SECS = 180s` on a 429-storm AFTER the deadline
check passes. A 30s deadline with a 429-storm at the boundary can elapse 210s+
in practice — a 7× overshoot.

**Anchor.** `src/api/jira/bulk.rs::await_bulk_task_inner`, `src/api/client.rs::send_inner`.

**Subsystem.** Bulk operations (introduced in #110 PR1+PR2; this BC tightens the
existing implicit timeout contract).

## Tightened NFR

### NFR-R-NEW-3 — Bulk-task wall-clock is bounded by `timeout`

| Field | Value |
|---|---|
| **Severity** | HIGH |
| **Anchor** | `src/api/jira/bulk.rs:288-442` (`await_bulk_task` + inner) and `src/api/client.rs:307+` (`send` retry loop) |
| **Description** | `await_bulk_task(timeout=T)` MUST return within `T + ε` where `ε ≤ POLL_MAX_SECS + control overhead`. 429 retry sleeps in `JiraClient::send` MUST be clamped by `min(retry_after_secs, deadline.saturating_duration_since(now()))`. If the clamp produces zero, the call MUST return `Err` rather than enter the sleep. |
| **Plan** | **FIX-IN-S-333**: Add `send_inner(req, deadline: Option<Instant>)` private helper; expose `send_bounded` and `get_bounded` public methods; `poll_bulk_task` becomes deadline-aware via a sibling `poll_bulk_task_with_deadline` (or, alternatively, deadline is threaded through the existing method by adding a 2nd parameter — F4 design will pick one). |
| **BC** | BC-bulk.poll.deadline-bounded |

**Note on related NFR-R-NEW-1.** `NFR-R-NEW-1` was about `Retry-After: 86400`-style
mega-values and was COMPLETED in S-3.07 by capping `MAX_RETRY_AFTER_SECS=60`. That
fix bounded *single-sleep magnitude*; it did **not** bound *cumulative sleep
within send()* (3 retries × 60s = 180s) and did not propagate any caller-side
deadline. NFR-R-NEW-3 fills that gap.

## New Holdout Scenario

### H-NEW-BULK-DEADLINE-001 — Bulk poll deadline NOT exceeded by 429 storm

**Status:** DRAFT (will be MUST-PASS upon merge)
**Verification:** Behavioral test (wiremock-driven; deterministic)
**NFR/BC:** NFR-R-NEW-3 / BC-bulk.poll.deadline-bounded

**Scenario.**
> A bulk-edit operation is invoked with `--max 50 --jql "..."` against a Jira
> instance that returns HTTP 202 + valid `taskId`, then for every poll request
> (`GET /bulk/queue/{taskId}`) returns HTTP 429 with `Retry-After: 60`. The
> caller's bulk command is configured (via existing CLI default or test seam)
> with a 30-second total deadline.
>
> **Expected:** `jr issue edit ...` exits non-zero with a timeout error message
> within ~35 seconds wall-clock (≤ deadline + one in-flight poll round-trip).
>
> **Current behavior (bug):** the command stays alive for 180s+ as `send()`
> sleeps through up to 3 × 60s retries before returning a 429 to the polling
> loop, which only then re-checks the deadline.

**Holdout-evaluator probe.**
1. Set up wiremock with the storm pattern above.
2. Run the bulk command in a child process under a 60s wall-clock kill.
3. Assert: process exited within 40s with non-zero status, AND stderr contains
   a recognizable timeout message that mentions the task id.

## ADR Decision

**No new ADR.** The change is an internal helper refactor (`send_inner` extracted
from `send`), not an architectural change. The pattern (`Option<Instant>`
threaded through retry loops) is documented inline in `send_inner`'s rustdoc
referencing this BC.

## Traceability

| New Artifact | Anchored to | Story to be created in F3 |
|---|---|---|
| BC-bulk.poll.deadline-bounded | NFR-R-NEW-3, H-NEW-BULK-DEADLINE-001 | S-333-bulk-deadline-propagation |
| NFR-R-NEW-3 | BC-bulk.poll.deadline-bounded | S-333-bulk-deadline-propagation |
| H-NEW-BULK-DEADLINE-001 | BC-bulk.poll.deadline-bounded, NFR-R-NEW-3 | S-333-bulk-deadline-propagation |

## Merge-Time Edits Required

When this delivery merges, the following PRD-shard edits will be applied (NOT
done in this F2 doc — done by state-manager at PR-merge):

1. `.factory/specs/prd/nfr-catalog.md` — append NFR-R-NEW-3 row in the Reliability
   section near NFR-R-NEW-1; bump corpus count from 40 → 41.
2. `.factory/specs/prd/holdout-scenarios.md` — append H-NEW-BULK-DEADLINE-001 in
   "Group 6: Reliability / MUST-FIX Pins" using H-NEW-* extended format.
3. `.factory/specs/prd/CANONICAL-COUNTS.md` — bump NFR count, holdout count.
4. `.factory/specs/prd/bc-3-issue-write.md` — add BC-bulk.poll.deadline-bounded
   to the bulk subsection (or, if no bulk subsection exists, add one referencing
   #110 lineage).

## Quality Gate Checklist

- [x] New BC defined with anchor + rationale
- [x] NFR identified (new NFR-R-NEW-3) with severity + plan
- [x] Holdout scenario defined with deterministic probe
- [x] No ADR needed (internal-helper refactor); decision documented
- [x] Traceability matrix present
- [x] Merge-time PRD edits enumerated
- [ ] **Human approves spec deltas (pending)**
