# Feature Spec: Unknown Bulk Task Status — Warn + Grace Period

**Issue:** #336 (audit-followup, enhancement)
**Source:** Cross-PR audit of #325
**Status:** Implementing (post-VSDD spec)
**Author:** Zious (via assistant under DEC-018)
**Date:** 2026-05-12

## Problem

`BulkOperationProgress::is_terminal` matches a closed set of status strings
(`COMPLETE | FAILED | CANCELLED | DEAD | COMPLETED`). The polling loop in
`JiraClient::await_bulk_task` treats any non-match as "in progress" and keeps
sleeping until the 5-minute timeout.

If Atlassian introduces a new status (e.g., `PARTIAL_FAILURE`,
`PROCESSED_WITH_ERRORS`) — or if the API is misconfigured behind a proxy —
the user sees no diagnostic for 5 minutes, then gets a generic timeout error
with no hint that the underlying cause was an unrecognized status.

## Threat model / failure scenarios

1. **Atlassian deploys a new status** (most likely): API response includes a
   novel string the client doesn't recognize. Loop polls silently for 5
   minutes.
2. **Proxy / man-in-the-middle malformed body** modifies `status` to a
   gibberish value. Same silent-poll behavior.
3. **Server-side bug** that emits the wrong status string. Same.

All three are equivalent from the client's perspective: an opaque,
unrecognized status. The fix is the same regardless of root cause.

## Acceptance criteria (from issue #336)

- Test mounts a poll response with an unrecognized status string and verifies
  the warn+escalate path. The original issue example was `PARTIAL_FAILURE`,
  which the issue author treated as "made-up" — Perplexity validation
  2026-05-12 has since confirmed `PARTIAL_FAILURE` is actually a documented
  Atlassian enum addition, so this PR places it in the known/terminal set
  and uses `MYSTERY_STATUS_FAKE` (a deliberately-novel placeholder that
  won't collide with future enum additions) as the test driver instead.
- Stderr emits a warning identifying the unrecognized status.
- Either: (a) loop continues until deadline with warning visible, OR (b)
  caller gets `Err` with the status string after a grace period.

## Design decisions

### Decision 1: option (a) vs option (b)

Chose **option (b)** — warn on first sighting, escalate to `Err` after a grace
period.

**Rationale:**
- Option (a) is simpler but leaves the original problem (5-minute silent wait)
  partially in place: the warning fires once, then nothing for ~5 minutes.
- Option (b) gives a definitive, fast failure mode (~30s) when the unknown
  status is genuinely terminal, while still tolerating transient/new statuses
  that resolve to known values.
- The issue title explicitly says "treat ... as terminal-with-warning **after
  grace period**" — aligns with option (b).

### Decision 2: grace period = 30 seconds

Tunable via a constant `DEFAULT_UNKNOWN_STATUS_GRACE`. 30s is chosen because:
- The polling backoff is exponential (1s → 2s → 4s → 8s → 10s cap). 30s
  absorbs ~3-5 poll cycles, enough to ride out a single transient unknown
  status that resolves on the next poll.
- Well under the 5-minute timeout, so users see the diagnostic in time to
  retry or investigate.
- Long enough that a deploy mid-poll (rare but possible) where status
  flickers to a transient unknown won't trigger a spurious error.

### Decision 3: reset tracker on known-status return

If polling sees `unknown → known → unknown`, the tracker resets to the second
unknown sighting. Rationale: a deploy/rollback that transitions through an
unknown status briefly shouldn't accumulate against the grace period across a
known-status recovery.

### Decision 4: per-distinct-status tracking

If polling sees `UNKNOWN_A → UNKNOWN_B`, the tracker treats it as a new
sighting (reset timestamp + remember new status). Rationale: distinct unknown
statuses likely indicate genuine state transitions, not stuck-in-novel-status.

### Decision 5: known-status set boundary

**Perplexity-validated 2026-05-12**: Atlassian's BulkOperationProgress
enum has GROWN since this client was originally written. The current
authoritative set per developer.atlassian.com OpenAPI:

- **Terminal (no further progress expected):**
  - `COMPLETE` — operation succeeded
  - `COMPLETED` — empirical alias (some live responses use this form)
  - `FAILED` — operation failed
  - `CANCELLED` — operation cancelled
  - `DEAD` — operation reached a dead state
  - `PARTIAL_FAILURE` — **NEW (2026)** — operation completed but some
    items failed
  - `PROCESSED_WITH_ERRORS` — **NEW (2026)** — operation processed all
    items but with errors
- **Non-terminal (continue polling):**
  - `ENQUEUED` — waiting in queue
  - `RUNNING` — actively processing
  - `CANCEL_REQUESTED` — cancellation requested, awaiting actual cancel

`is_known_status()` returns `true` for the union. `is_terminal()` is
EXPANDED to include `PARTIAL_FAILURE` and `PROCESSED_WITH_ERRORS` so
the polling loop stops correctly on these statuses (otherwise they'd
fall through to "unknown" path and wait the grace period unnecessarily).

For the success/failure routing in `await_bulk_task`:
- `COMPLETE`, `COMPLETED` → `Ok(progress)` (caller checks `is_success()`)
- `PARTIAL_FAILURE`, `PROCESSED_WITH_ERRORS` → `Ok(progress)` (same — the
  operation finished, callers use `failed_accessible_issues` and
  `is_success()` to see partial-failure detail)
- `FAILED`, `CANCELLED`, `DEAD` → `Err` with hint
- Anything else (genuinely unknown) → warn + grace-period escalation

This preserves the existing caller contract: callers who currently check
`is_success()` on `Ok` will see `false` for `PARTIAL_FAILURE` because
`failed_accessible_issues` is non-empty in those responses.

Note: the bulk.rs unit test currently iterates `PENDING` and `IN_PROGRESS`
through `is_terminal()` to verify they return `false`. After #336, these
strings still return `false` from `is_terminal()` (unchanged), and they
return `false` from `is_known_status()` (treated as novel). Neither is
observed in production Jira responses per the OpenAPI spec — they were in
the test to assert non-terminal behavior, not because Atlassian emits them.
If Atlassian ever does emit them, the warn+grace path will surface them
and we can add them to the known set in a follow-up.

### Decision 6: test pacing — `#[cfg(test)]` variant

Production callers use a 30s grace. Tests can't wait 30s per assertion.
Solution: keep `await_bulk_task(task_id, timeout)` signature unchanged for
production callers, add a `#[cfg(test)] pub async fn await_bulk_task_with_grace_for_test(task_id, timeout, unknown_grace)` that
delegates to the same inner implementation. Tests pass a 200ms grace +
short polling intervals via wiremock — assertion runs in ~1s.

### Decision 7: warning channel

Warning emits to **stderr** (consistent with the codebase's mixed/symmetric
output-channel profiles). Format:

```
warning: bulk task <id> returned unrecognized status <status> — treating
as non-terminal. If the operation hangs, this may be a new Atlassian
status; report at <repo-issues-url>.
```

After grace period escalation, the Err message includes the same status
plus the grace duration so the operator sees the timeline:

```
Bulk task <id> polled unrecognized status <status> for >= <N>s; treating
as terminal-with-error. If this status indicates progress (not failure),
please report at <repo-issues-url> so we can update the known-status list.
```

## Public API impact

- `BulkOperationProgress::is_terminal()` — **signature unchanged**; recognized
  set **expanded** to include `PARTIAL_FAILURE` and `PROCESSED_WITH_ERRORS`
  (Atlassian enum additions verified Perplexity 2026-05-12). Existing callers
  that route by `is_terminal()` now exit the poll loop on these statuses
  instead of falling through to the unknown-status warn+grace path.
- `BulkOperationProgress::is_known_status()` — **new** public method covering
  the full documented enum (terminal ∪ non-terminal).
- `JiraClient::await_bulk_task(task_id, timeout)` — **unchanged** signature;
  internal behavior gains the warn + escalate paths. Uses
  `DEFAULT_UNKNOWN_STATUS_GRACE_SECS` (30s).
- `JiraClient::await_bulk_task_with_grace_for_test(...)` — **new**, gated
  `#[cfg(test)]`. Not part of release API surface. Used by in-lib wiremock
  tests in `src/api/jira/bulk.rs`.
- `JR_BULK_UNKNOWN_GRACE_SECS` env var — **new**, gated
  `#[cfg(debug_assertions)]` so release binaries ignore it (mirrors the
  `JR_BASE_URL` / `JR_AUTH_HEADER` debug-only pattern). Lets CLI-level
  integration tests drive the warn+escalate path through the binary without
  waiting 30 seconds. Not security-sensitive (no token-leak vector) — at worst
  a misuse shortens or lengthens the grace, both bounded by the 5-minute
  overall timeout.

## Test plan

1. **Unit tests** in `mod tests` of `src/types/jira/bulk.rs`:
   - `test_336_is_known_status_recognizes_documented_set` — all documented
     statuses (terminal ∪ non-terminal) return `true`; novel/unknown values
     return `false`.
   - `is_terminal` test updated to cover the two new terminal statuses
     `PARTIAL_FAILURE` and `PROCESSED_WITH_ERRORS`.
2. **In-lib wiremock tests** in `mod unknown_status_grace_tests` of
   `src/api/jira/bulk.rs` — drive `await_bulk_task_with_grace_for_test` end-
   to-end against `wiremock::MockServer`:
   - `test_336_persistent_unknown_status_escalates_to_err_after_grace` —
     server returns `MYSTERY_STATUS_FAKE` (a deliberately-novel string that
     won't collide with future enum additions) indefinitely; with
     `timeout=10s, grace=200ms` the call returns `Err` containing the status
     string. Asserts the escalation contract via return value only — stderr
     emission is verified at the CLI-level integration test below, not here.
   - `test_336_known_status_sequence_returns_ok_without_escalation` — server
     returns `ENQUEUED → COMPLETE`; call returns `Ok(progress)`.
   - `test_336_transient_unknown_then_known_resets_tracker` — server returns
     `TRANSIENT_FAKE_STATUS → COMPLETE`; the tracker resets on the known
     status and the call returns `Ok(progress)` rather than escalating.

   These tests verify the escalation/reset contract via the function return
   value. They do **not** assert on stderr (capturing `eprintln!` from an
   async unit test requires a `tracing` migration that is out of scope for
   #336 — Perplexity-validated 2026-05-12).
3. **CLI-level integration test** in `tests/issue_bulk_pr2.rs` —
   `test_336_cli_unknown_status_emits_warning_and_escalates` drives the path
   through `jr issue move` via `assert_cmd`. Mocks the bulk POST + queue GET
   to return `MYSTERY_STATUS_FAKE`, sets `JR_BULK_UNKNOWN_GRACE_SECS=0` so
   the test completes in ~1s, and captures the binary's stderr to assert:
   - the unknown-status warning text is emitted;
   - the eventual escalation error mentions the status and the grace.
   This closes Copilot R1 finding: warning emission is verified at the
   process boundary, where it actually reaches operators.

## Out of scope

- Allow-listing arbitrary new statuses via config (premature).
- Configurable warning destination (always stderr — consistent).
- Telemetry / report-back to a metrics endpoint (no infrastructure for it).

## Non-goals

- Catching every possible novel status configuration. The fix surfaces
  diagnostic information; it does not enumerate every possible future
  Atlassian status.

## Migration / compatibility

No breaking changes. Behavior is strictly additive: known statuses behave
exactly as before; only unknown statuses see the new warn + escalate paths.
Callers do not need code changes.
