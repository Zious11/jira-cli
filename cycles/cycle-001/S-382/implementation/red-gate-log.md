---
story_id: S-382
cycle: cycle-001
red_gate_date: 2026-05-19
verified_by: orchestrator (independent re-run after test-writer dispatch)
verdict: PASSED
---

# S-382 Red Gate Log

## Context

S-382 is a refactor of `JrError::InsufficientScope` to add a `required_scope: Option<String>` field and parameterize the Display template. Quick-dev route (no F2/F3), F1d converged 3/3 at pass-08.

## Red Gate Discipline Applied

**Step 2 violation caught + corrected:** The initial Step 2 dispatch applied BOTH the variant signature widening AND the Display template parameterization. Orchestrator detected the Red Gate violation (no implementation before failing tests) and dispatched a fix to revert the Display template change. Stub state restored: variant has both fields, Display ignores `required_scope` field and renders literal `"write:jira-work"`. Commit: `950aefb`.

## Tests Added in Step 3 (commit `bab5b4b`)

- `test_insufficient_scope_display_uses_required_scope_when_some` (AC-3)
- `test_insufficient_scope_display_empty_some_falls_back` (AC-4)

## Red Gate Verification Results

Command: `cd .worktrees/S-382 && cargo test --lib insufficient_scope`

| Test | Result | Notes |
|---|---|---|
| `insufficient_scope_exit_code` (existing) | PASS | Existing behavior preserved (exit code 2 via wildcard match) |
| `insufficient_scope_display_includes_workarounds` (existing) | PASS | Existing behavior preserved (literal template still emits required substrings) |
| `test_insufficient_scope_display_uses_required_scope_when_some` (AC-3, NEW) | **FAIL** | Assertion error: "Display should contain the call-site-supplied scope name". Template renders literal `"write:jira-work"`, not the `Some("write:servicedesk-request")` value. |
| `test_insufficient_scope_display_empty_some_falls_back` (AC-4, NEW) | PASS | Literal template accidentally satisfies the empty-Some fallback contract — design pin holds for BOTH literal and parameterized templates. |

## Red Gate Outcome: **PASSED**

- AC-3 test FAILS with assertion error (not panic, not build error)
- Failure message references behavior under test (Display content vs expected scope name)
- AC-4 test passes (correctly pins fallback behavior for both current and target Display implementations)
- Existing tests preserved (None-fallback design correctly retains all literal `"write:jira-work"` assertions)
- 713 other tests unaffected (filtered out by `insufficient_scope` test name filter)

Step 4 (TDD implementation) authorized: implementer must change the Display template to use `scope_hint` expression-arg so AC-3 passes.

## Compliance Notes

- IRON LAW: "NO IMPLEMENTATION WITHOUT RED GATE VERIFICATION FIRST" — initially violated by Step 2 over-reach; corrected via revert dispatch.
- Failing tests verified BEFORE Step 4 dispatch — proper sequence restored.
- Test naming convention applied (`test_<verb>_<subject>_<expected_outcome>`).
- Two-part assertion in AC-3 (positive + negation) per pass-05 L-01 fix.
- No `||` / `.or_else()` / accept-either patterns (L-288-pr2-02 compliant).
