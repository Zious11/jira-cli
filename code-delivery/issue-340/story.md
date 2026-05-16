---
document_type: story
story_id: "S-340"
title: "Pin task_id-in-bulk-poll-timeout-message contract with regression test"
wave: feature-followup
status: ready
priority: low
estimated_effort: small
tdd_mode: strict
bc_anchors:
  - BC-3.4.009
holdout_anchors: []
nfr_anchors:
  - NFR-R-NEW-3
adr_refs: []
sd_refs: []
files_modified: []
test_files:
  - tests/bulk_deadline_propagation.rs (extend test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp with task_id assertion)
breaking_change: false
producer: story-writer
version: "1.0.1"
last_updated: 2026-05-15
depends_on:
  - S-333
blocks: []
issue: 340
---

# S-340: Pin `task_id`-in-bulk-poll-timeout-message contract with regression test

## Context

Option (c) of issue #340 — including the `task_id` literal in the bulk-poll
timeout error message — is already implemented in production code. The relevant
site is `src/api/jira/bulk.rs:412`, where `await_bulk_task_inner` emits:

```
[deadline:bulk-outer] Bulk task {task_id} did not complete within {N}s timeout.
Check Jira for task status.
```

This was delivered via PR #360 as part of the S-333 bulk-deadline-propagation
work. The production code correctly satisfies BC-3.4.009, which was formally
anchored in F2 spec evolution (2026-05-15).

What is missing is a behavioral-contract test assertion that pins this property
as a regression guard. Without the pin, a future refactor of the error message
format could silently drop the `task_id` and the test suite would not catch it.
The existing B-1 test (`test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp`)
in `tests/bulk_deadline_propagation.rs` already exercises the exact code path
— it just lacks a `stderr.contains(task_id)` assertion.

This story adds that assertion. No production code is changed.

## Behavioral Contracts

**BC-3.4.009** (F2 addition, 2026-05-15). When `await_bulk_task(task_id, timeout)`
exhausts its deadline without the bulk task completing, the
`JrError::DeadlineExceeded` error message emitted to stderr MUST contain the
literal value of `task_id`. The message format is:

```
[deadline:bulk-outer] Bulk task <task_id> did not complete within <N>s timeout.
Check Jira for task status.
```

The `task_id` value in the message MUST match the `taskId` returned by the
initial bulk POST response. It MUST pass `validate_task_id` before insertion
(CWE-117 log-injection guard — audited in PR #355).

**VP Extension** (from BC-3.4.009 spec body): Extends
`BC-bulk.poll.deadline-bounded` (S-333 working label) by adding the requirement
that `task_id` appears in the stderr output in addition to the existing
wall-clock bound and `"deadline"` substring assertions.

## Acceptance Criteria

**AC-001** (traces to BC-3.4.009 postcondition — task_id in message). The test
`test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp` in
`tests/bulk_deadline_propagation.rs` is extended with a new assertion:

```rust
assert!(
    stderr.contains(task_id),
    "BC-3.4.009 VIOLATION: expected stderr to contain the task_id literal \
     '{task_id}' in the bulk-poll timeout message. Got stderr:\n{stderr}",
);
```

where `task_id = "task-333-b1-running-storm"` (the wiremock fixture literal
already used by that test). This is additive — the assertion is appended after
the existing `stderr.contains("deadline")` check at the end of the test.

**AC-002** (traces to BC-3.4.009 invariant — additive only). All existing
assertions in `test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp`
continue to pass unchanged:
- Wall-clock elapsed `< WALL_CLOCK_BUDGET_SECS` (timing contract)
- Wall-clock elapsed `>= 25s` (false-positive guard)
- Exit code `== Some(124)` (JrError::DeadlineExceeded exit code)
- `stderr.to_lowercase().contains("deadline")` (existing substring assertion)

**AC-003** (traces to BC-3.4.009 invariant — no regression). No regression on
any other test in `tests/bulk_deadline_propagation.rs`:
- `test_333_bulk_429_storm_respects_deadline_within_grace` (AC-001 429-storm
  test from S-333) passes without modification.
- All other tests in the file are unaffected.

**AC-004** (traces to BC-3.4.009 — release gate). The following commands all
pass after the assertion is added:

```
cargo test --test bulk_deadline_propagation
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

## Out of Scope

The following options from issue #340 are explicitly deferred to separate future
issues per the F1 delta analysis (2026-05-15):

**Option (a) — size-scaling formula** (`timeout = 300 + keys.len() * 2`,
capped at 1800s): Requires a resolver signature change propagating to call sites
in `src/cli/issue/create.rs` and `src/cli/issue/workflow.rs`, plus one
release-gate test update and a new unit test for the formula. File as a new
enhancement issue when operational data shows the 300s fixed default is
insufficient for real-world bulk operations.

**Option (b) — const-bump** (300s → 900s): Simpler than (a), but no empirical
data justifies the bump today. File as a new enhancement issue with operational
justification (e.g., field reports of timeout failures on large bulk edits)
before acting.

Neither (a) nor (b) is in scope for this story. The only change is the additive
test assertion for (c), which is already correctly implemented in production.

## Implementation Notes

**Production code site (no change needed):**

```
src/api/jira/bulk.rs:408-418
```

The relevant block is:

```rust
if Instant::now() >= deadline {
    return Err(crate::error::JrError::DeadlineExceeded {
        remaining_ms: 0,
        message: format!(
            "[deadline:bulk-outer] Bulk task {task_id} did not \
             complete within {}s timeout. Check Jira for task status.",
            timeout.as_secs()
        ),
    }
    .into());
}
```

The `{task_id}` interpolation at line 412 is what BC-3.4.009 pins. This line
already satisfies the contract — the test assertion is the missing piece.

**Test site:**

```
tests/bulk_deadline_propagation.rs
```

The B-1 test begins at approximately line 289:
`test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp`.

The wiremock fixture uses `task_id = "task-333-b1-running-storm"` (defined at
line 299 of the test). The new assertion must reference this same literal to
ensure the test is checking the correct task_id propagation.

The test currently ends at line 385 after the `stderr.contains("deadline")`
check. The new assertion is appended immediately after that check, using the
already-bound `stderr` variable and the already-bound `task_id` variable (both
in scope at the end of the test).

**BC anchor:** BC-3.4.009 (anchored in `.factory/specs/prd/bc-3-issue-write.md`
at the BC-3.4 Edit+Open subdomain).

## TDD Plan

This story is a test-pin for existing behavior, not a new behavioral
implementation. The standard TDD red-green-refactor flow applies with one
important note: the test assertion will be **green on first run** because
production code already satisfies the contract. This is acceptable — the value
of the pin is as a regression guard against future refactors, not to drive
implementation.

**Step 1 — Extend the test (add the assertion).**

Open `tests/bulk_deadline_propagation.rs` and locate the end of
`test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp` (after the
`stderr.contains("deadline")` assertion, approximately line 381-385). Append:

```rust
assert!(
    stderr.contains(task_id),
    "BC-3.4.009 VIOLATION: expected stderr to contain the task_id literal \
     '{task_id}' in the bulk-poll timeout message. Got stderr:\n{stderr}",
);
```

**Step 2 — Confirm it passes (green on first run).**

```
cargo test --test bulk_deadline_propagation test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp
```

Expected result: PASS. The production code at `bulk.rs:412` interpolates
`{task_id}` directly into the message, so the assertion is trivially satisfied.
The test should complete in ~30-31 seconds (wall-clock budget).

Note explicitly: this is a green-on-first-run test by design. The production
contract exists; the test pin is the missing artifact. Accepting a green-on-first-run
here is correct because the purpose is behavioral anchoring, not behavioral driving.

**Step 3 — Red gate verification (mandatory; revert after).**

Temporarily mutate `src/api/jira/bulk.rs:412` to remove `{task_id}` from the
format string:

```rust
// Mutant (do NOT commit):
message: format!(
    "[deadline:bulk-outer] Bulk task did not complete within {}s timeout. \
     Check Jira for task status.",
    timeout.as_secs()
),
```

Run the test again:

```
cargo test --test bulk_deadline_propagation test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp
```

Expected result: FAIL with the BC-3.4.009 VIOLATION message. This confirms the
assertion is actually testing the right thing and not vacuously passing.

Revert the mutation. Confirm the test is green again.

**Step 4 — Full suite.**

```
cargo test --test bulk_deadline_propagation
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

All must pass. This satisfies AC-004.

## Token Budget Estimate

| Component | Estimated tokens |
|-----------|-----------------|
| Story spec (this file) | ~2,500 |
| tests/bulk_deadline_propagation.rs (read full file) | ~3,000 |
| src/api/jira/bulk.rs (read lines 395-420) | ~400 |
| BC-3.4.009 spec text | ~300 |
| Test output / cargo output | ~200 |
| **Total** | **~6,400** |

Well within the 20-30% agent context window budget for a small story.

## Previous Story Intelligence

**S-333** (parent) delivered:
- `JrError::DeadlineExceeded { remaining_ms, message }` variant (exit code 124)
- `src/api/client.rs::send_inner` with deadline clamping of 429 retry sleeps
- `src/api/jira/bulk.rs::await_bulk_task_inner` with `[deadline:bulk-outer]`
  site-tagged error message including `{task_id}`
- `tests/bulk_deadline_propagation.rs` with the B-1 RUNNING-storm test

The B-1 test infrastructure (wiremock fixtures, helper functions
`jr_cmd_with_30s_deadline`, `jql_search_response_one`, `bulk_enqueued`,
`WALL_CLOCK_BUDGET_SECS`) is already in place and does not need to be
re-created. The task_id fixture string `"task-333-b1-running-storm"` is already
bound in the test. The `stderr` variable is already bound at the end of the test.
The new assertion simply extends the existing test body.

## Architecture Compliance Rules

- No production code changes. This story is test-only.
- Do NOT introduce a new test function. Extend the existing B-1 test with an
  additive assertion. A new sibling test is acceptable only if the test author
  determines the existing test's setup cannot accommodate the new assertion
  without restructuring (unlikely given the existing `stderr` binding).
- The new assertion MUST use `task_id` (the already-bound variable) as the
  needle, not a hardcoded string literal. This keeps the assertion self-consistent
  with the wiremock fixture and prevents future drift if the fixture task_id
  ever changes.
- Assertion message prefix MUST include `"BC-3.4.009 VIOLATION:"` to enable
  grep-based BC coverage tracking in CI.

## Library & Framework Requirements

Same as `tests/bulk_deadline_propagation.rs` at HEAD:
- `wiremock` — same version as `Cargo.toml` (no version change)
- `tokio` with `multi_thread` flavor (already configured in the test)
- No new test dependencies

Do not add new Cargo dependencies for this story.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `tests/bulk_deadline_propagation.rs` | MODIFY (additive) | Append one `assert!` block after the existing `stderr.contains("deadline")` check at the end of `test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp` |
| `src/api/jira/bulk.rs` | DO NOT TOUCH | Production code already satisfies BC-3.4.009 |

No new files are created. No other files are modified.

## References

- Issue #340 (parent): `chore(bulk): scale await_bulk_task timeout with bulk size or include task_id in timeout error`
- BC-3.4.009: `.factory/specs/prd/bc-3-issue-write.md` (F2 addition 2026-05-15)
- S-333 (parent story): `.factory/code-delivery/issue-333/story.md`
- PR #360: Source of the production implementation — `[deadline:bulk-outer] Bulk task {task_id} did not complete within...`
- F1 delta analysis: `.factory/phase-f1-delta-analysis/delta-analysis.md`
