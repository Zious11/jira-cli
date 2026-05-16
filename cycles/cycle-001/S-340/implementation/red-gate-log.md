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

---

## Pass 1 Nit Fixes — Re-Verification (2026-05-15)

### Changes applied (worktree commit `49f6303`)

Two adversary Pass 1 NIT findings applied to `tests/bulk_deadline_propagation.rs`:

**Finding 1 — Citation precision:**
- Before: `// at src/api/jira/bulk.rs:412.`
- After: `// at src/api/jira/bulk.rs:408-417 (return site of JrError::DeadlineExceeded).`
- Rationale: single-line citation is brittle under whitespace reflow; line range covers
  the entire `if Instant::now() >= deadline { return Err(...) }` block.

**Finding 2 — Assertion scope tightening:**
- Before: `assert!(stderr.contains(task_id), ...)`
- After: `assert!(stderr.contains(&expected_fragment), ...)` where
  `expected_fragment = format!("[deadline:bulk-outer] Bulk task {task_id} did not")`
- Rationale: bare `task_id` substring match would pass if task_id appeared anywhere in
  stderr (e.g., a future `--verbose` GET URL log). Full fragment match proves task_id
  is interpolated INSIDE the `[deadline:bulk-outer]` message specifically.

### Mutation re-verification

Same mutation as original Red Gate: `{task_id}` → `<redacted>` in `src/api/jira/bulk.rs:412`.

**Tightened assertion failure (FAIL as expected):**
```
thread 'test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp' panicked at tests/bulk_deadline_propagation.rs:393:5:
BC-3.4.009 VIOLATION: expected stderr to contain the deadline-message fragment
"[deadline:bulk-outer] Bulk task task-333-b1-running-storm did not" (proves task_id
is interpolated inside the [deadline:bulk-outer] message, not coincidentally elsewhere
in stderr — e.g., a --verbose request URL log). Got stderr:
Error: Deadline exceeded: [deadline:bulk-outer] Bulk task <redacted> did not complete within 30s timeout. Check Jira for task status.
```

Mutation reverted. `git diff src/api/jira/bulk.rs` confirmed empty.

**Post-revert test run:** PASS (2 tests, 0 failures).

### Full suite verification

| Check | Result |
|-------|--------|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo test --test bulk_deadline_propagation` | PASS (2/2) |

Red Gate equivalent for Pass 1 nits: MUTATION-PASSED (re-verified).

---

## Pass 2 Nit Fixes — Re-Verification (2026-05-15)

### Changes applied (worktree commit `d63b2eb`)

Four adversary Pass 2 NIT findings applied to `tests/bulk_deadline_propagation.rs`:

**F1 — Line range correction:**
- Before: `// at src/api/jira/bulk.rs:408-417 (return site of JrError::DeadlineExceeded).`
- After: `// at src/api/jira/bulk.rs:411-415 (the message format inside the JrError::DeadlineExceeded Err-return block at lines 408-418).`
- Rationale: 408-417 was off-by-one on the closing brace (line 418); 411-415 is the
  precise `format!(...)` expression that interpolates `{task_id}`.

**F2 — Loose assertion restored:**
- Added `assert!(stderr.contains(task_id), "BC-3.4.009 VIOLATION (loose): ...")` BEFORE the strict fragment assertion.
- Rationale: the strict assertion alone deviates from story S-340 AC #1 literal text
  `"stderr.contains(task_id)"`. Both forms coexist: loose satisfies the story AC literal;
  strict is the false-positive guard.

**F3 — Case-insensitive strict assertion:**
- Changed `stderr.contains(&expected_fragment)` to
  `stderr.to_lowercase().contains(&expected_fragment.to_lowercase())`.
- Rationale: harmonizes with existing tertiary assertion style (line 381:
  `stderr.to_lowercase().contains("deadline")`). `cargo fmt` reformatted the chain
  to the idiomatic multi-line form.

**F4 — BC ↔ test index comment:**
- Added a 7-line `// BC ↔ Test index` block after the module-level doc-comments and
  before `#[allow(dead_code)]`. Maps `BC-3.4.009 → test_333_b1_*` with production
  site citation (`src/api/jira/bulk.rs:411-415`) for audit-followup discoverability.

### Mutation re-verification (third pass)

Mutation: `{task_id}` → `<redacted>` in `src/api/jira/bulk.rs:412`.

**Loose assertion failure (fires first):**
```
thread 'test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp' panicked at tests/bulk_deadline_propagation.rs:403:5:
BC-3.4.009 VIOLATION (loose): expected stderr to contain task_id literal "task-333-b1-running-storm". Got stderr:
Error: Deadline exceeded: [deadline:bulk-outer] Bulk task <redacted> did not complete within 30s timeout. Check Jira for task status.
```

The strict assertion would also fire (task_id absent from fragment) — Rust panics on first
failure so only the loose is visible, but both discriminate the contract independently.

Mutation reverted. `git diff src/api/jira/bulk.rs` confirmed empty.

**Post-revert test run:** PASS (2 tests, 0 failures).

### Full suite verification

| Check | Result |
|-------|--------|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo test --test bulk_deadline_propagation` | PASS (2/2) |

Red Gate equivalent for Pass 2 nits: MUTATION-PASSED (re-verified).
