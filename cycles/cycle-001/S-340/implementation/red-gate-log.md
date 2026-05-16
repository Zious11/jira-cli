---
document_type: red-gate-log
level: ops
version: "1.0"
status: complete
producer: test-writer
timestamp: 2026-05-15T00:00:00
phase: 3
inputs:
  - .factory/code-delivery/issue-340/story.md
  - .factory/specs/prd/bc-3-issue-write.md
input-hash: "[n/a — regression-pin, no stub generation]"
traces_to: "BC-3.4.009"
stub_architect_agent: "[n/a]"
stub_compile_verified: false
test_writer_agent: "[claude-sonnet-4-6]"
red_gate_verified: true
---

# Red Gate Log: S-340

## Summary
| Story | Tests Modified | All Fail (Red)? | Gate |
|-------|---------------|-----------------|------|
| S-340 | 1 (bulk_deadline_propagation.rs) | N/A — regression-pin; mutation-based Red Gate used | MUTATION-PASSED |

## Pattern

Regression-pin test (production code already correct). Standard Red Gate
(test fails before implementation) is not applicable; substituted with
**mutation-based Red Gate**: deliberately break production code, confirm
the new assertion fails, revert, confirm tests pass.

## Stubs Created

None. This story adds assertions to an existing test — no stubs required.

## Red Gate Verification (Mutation-Based)

### S-340

- AC #1 (BC-3.4.009): `test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp` —
  mutation `{task_id}` → `<redacted>` in `src/api/jira/bulk.rs:412` caused
  `BC-3.4.009 VIOLATION` assertion to fire — FAIL (correct discrimination)

**Mutation experiment evidence:**

```
thread 'test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp' panicked at tests/bulk_deadline_propagation.rs:393:5:
BC-3.4.009 VIOLATION: expected stderr to contain task_id literal "task-333-b1-running-storm". Got stderr:
Error: Deadline exceeded: [deadline:bulk-outer] Bulk task <redacted> did not complete within 30s timeout. Check Jira for task status.
```

**Existing assertions (exit 124, "deadline" substring) were NOT affected by the mutation** — only the new BC-3.4.009 assertion fired. This proves the assertion discriminates the task_id contract specifically.

**Note on AC-001 test (`test_333_bulk_429_storm_respects_deadline_within_grace`):** The
429-storm path exercises the INNER clamp (`[deadline:429-retry]` in `send_inner`), not the
outer-loop `[deadline:bulk-outer]` site that includes `{task_id}`. The BC-3.4.009 assertion
applies only to the outer site, so it was added only to the B-1 test which exercises that path.

## Regression Check

| Existing Tests | Status |
|---------------|--------|
| `test_333_bulk_429_storm_respects_deadline_within_grace` | PASS |
| `test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp` | PASS |
| `bulk_await_timeout_release_gate` (2 tests) | PASS |
| `issue_bulk_pr2` (40 tests) | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo fmt --check` | PASS |

## Conclusion

The new assertion discriminates the BC-3.4.009 contract correctly.
Red Gate equivalent: MUTATION-PASSED.

## Evidence

- Test file: `tests/bulk_deadline_propagation.rs` (worktree `feature/S-340-bulk-poll-task-id-pin`)
- Test name: `test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp`
- New assertion lines: 393-398 (approx) in modified file
- Mutation site: `src/api/jira/bulk.rs:412`
- Worktree commit: `55331fa` (feature/S-340-bulk-poll-task-id-pin)

## Hand-Off

- Story S-340 implementation complete — regression-pin test committed to worktree branch.
- Next step: adversarial review (F5) and PR creation (F7).
- Options (a) and (b) from issue #340 (size-scaling, const-bump) are deferred to
  separate enhancement issues per F1 delta analysis.
