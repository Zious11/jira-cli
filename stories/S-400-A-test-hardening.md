---
document_type: story
story_id: "S-400-A"
title: "Test hardening: strengthen dry-run guard, rewrite bulk-path test, fix VP comment, add --description-stdin create parity (closes #400 TH items)"
wave: feature-followup
status: ready
intent: test-debt
feature_type: infrastructure
scope: non-trivial
severity: low
trivial_scope: false
issue: 400
points: 3
priority: medium
tdd_mode: strict
estimated_effort: small
depends_on: []
bc_anchors: []
# No BC anchor — all four items are test-only. The underlying behavioral contracts
# BC-3.4.012 / BC-3.4.013 / BC-3.4.014 are unchanged. Their tests were strengthened,
# not altered. BC status: no BC authorship required.
# Status=ready because: F1 delta analysis accepted the decomposition for issue #400;
# no new BCs gate this story; all design decisions are locked per F1 analysis.
verification_properties: []
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f1-delta-analysis/issue-400/delta-analysis.md"
implementation_strategy: tdd
module_criticality: LOW
traces_to:
  - TH-398-1
  - TH-398-2
  - TH-398-3
  - TH-398-4
  - issue-400
files_modified:
  - tests/issue_edit_echo.rs   # MODIFIED — TH-398-1: add positive guard to dry-run test; TH-398-2: rewrite bulk-path test with correct bulk endpoint mocks
  - tests/issue_create_echo.rs # MODIFIED — TH-398-3: fix mislabeled VP comment; TH-398-4: add new --description-stdin create test
files_created: []
breaking_change: false
assumption_validations: []
risk_mitigations: []
---

# S-400-A — Test Hardening: Dry-Run Guard, Bulk-Path Mock Rewrite, VP Comment Fix, `--description-stdin` Create Parity

## Source of Truth

GitHub issue: https://github.com/Zious11/jira-cli/issues/400
F1 delta analysis: `.factory/phase-f1-delta-analysis/issue-400/delta-analysis.md`
Predecessor story: S-398 (PR that shipped BC-3.4.012/013/014 and the issue-edit/create echo feature)

## Goal

Harden the four weakest tests left by the S-398 cycle:
- One near-vacuous dry-run guard strengthened with a positive assertion (TH-398-1)
- One test that passed on an error path rewritten to succeed on the bulk success path (TH-398-2)
- One cosmetic comment malformed identifier fixed (TH-398-3)
- One missing `--description-stdin` parity test added on the `issue create` side (TH-398-4)

No production code changes. No spec file changes.

## Background

The S-398 cycle shipped BC-3.4.012 (edit table-mode echo), BC-3.4.013 (edit JSON echo with
`changed_fields`), and BC-3.4.014 (create all-fields echo). The F1 delta analysis for issue
#400 verified four test quality gaps:

**TH-398-1** (`tests/issue_edit_echo.rs::test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run`,
currently at line 635): The test asserts only `!stderr.contains("  summary → X")`.
This passes regardless of exit path — including a non-zero exit for unrelated reasons — because
the dry-run path exits before any PUT. The missing positive guard is: `"Updated TEST-1"` must
also be absent, confirming the success confirmation path was never reached.

**TH-398-2** (`tests/issue_edit_echo.rs::test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key`,
currently at lines 673–709): The test mounts `PUT /rest/api/3/issue/TEST-{1,2}` mocks that are
never called. Two-key edit routes through
`src/cli/issue/create.rs::handle_edit_bulk_fields` → `POST /rest/api/3/bulk/issues/fields`
(`src/api/jira/bulk.rs`) → `GET /rest/api/3/bulk/queue/{taskId}`. Neither PUT mock is registered
with `.expect(0)` as a sentinel, and the bulk endpoint is unmocked, so wiremock returns 404 on
the POST, the command fails, and the assertion `!stderr.contains("  summary →")` passes vacuously
on the error path. The test must be rewritten to mock the actual bulk chain and assert
`output.status.success()` before making the negative echo assertion.

**TH-398-3** (`tests/issue_create_echo.rs`, line 773 comment): The comment reads
`// BC-3.4.014 EC-014; VP-398-AC-014.` — the identifier `VP-398-AC-014` is malformed.
Valid VPs are `VP-398-001` through `VP-398-006`. `AC-014` is an acceptance-condition
reference, not a VP. Fix: drop the `VP-398-` prefix → `// BC-3.4.014 EC-014; AC-014.`

**TH-398-4** (`tests/issue_create_echo.rs`): The `issue create --description-stdin` path
has no test. The edit side has
`test_bc_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields` (which
uses `.write_stdin("My description\n")` in JSON mode). The create side needs a table-mode
parallel: assert that `--description-stdin` with stdin `"Some stdin description\n"` causes
`  description → (updated)` to appear in stderr and the content to NOT appear.

## Scope

### Files modified
- `tests/issue_edit_echo.rs` — TH-398-1 (single assertion addition) + TH-398-2 (function body rewrite)
- `tests/issue_create_echo.rs` — TH-398-3 (one-line comment fix) + TH-398-4 (new test function)

### Files NOT modified
- `src/` — zero production code changes
- `.factory/specs/prd/` — zero BC file changes
- `scripts/` — zero script changes (Story B handles scripts)
- `CLAUDE.md` — not modified by this story

## Acceptance Criteria

### AC-001 — dry-run test gains a positive no-success-confirmation guard (TH-398-1)

`tests/issue_edit_echo.rs::test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run` contains
both of the following assertions in its body, in the order shown:

1. `!stderr.contains("  summary \u{2192} X")` (existing, preserved verbatim)
2. `!stderr.contains("Updated TEST-1")` (new, added by this story)

The second assertion must include a descriptive message argument explaining the intent:
"dry-run must not reach the success confirmation path".

Verification:
```
grep -c '"Updated TEST-1"' tests/issue_edit_echo.rs
```
Must return at least 1 (the new assertion; existing tests may also reference this string).

```
grep -A 10 'fn test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run' tests/issue_edit_echo.rs | grep 'Updated TEST-1'
```
Must return a match (the assertion is inside the dry-run test function).

### AC-002 — bulk-path test mocks the correct endpoints and asserts success (TH-398-2)

`tests/issue_edit_echo.rs::test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key` is
rewritten so that:

(a) The two `PUT /rest/api/3/issue/TEST-{1,2}` mocks are replaced or converted to
    `.expect(0)` sentinel mounts that assert "must NOT be called".

(b) `POST /rest/api/3/bulk/issues/fields` is mocked to return HTTP 200 with a JSON body
    matching the `BulkSubmitResponse` shape (field: `taskId: "task-bulk-echo-001"`).
    The response shape must match `src/types/jira/bulk.rs::BulkSubmitResponse` which
    deserializes `taskId` from camelCase.

(c) `GET /rest/api/3/bulk/queue/task-bulk-echo-001` is mocked to return HTTP 200 with
    a terminal `BulkOperationProgress` response where `status: "COMPLETE"`,
    `processedAccessibleIssues: ["TEST-1", "TEST-2"]`, and `failedAccessibleIssues: {}`.
    This shape matches `src/types/jira/bulk.rs::BulkOperationProgress`.

(d) The test asserts `output.status.success()` is true (exit 0).

(e) The existing negative assertion `!stderr.contains("  summary \u{2192}")` is retained.

The mock shape for (b) and (c) must follow the same helper pattern established in
`tests/issue_bulk.rs::bulk_task_enqueued_response` and `bulk_task_complete_response`:
those helpers are the canonical fixture shapes; replicate or inline the relevant JSON
structure.

Verification:
```
grep -n 'bulk/issues/fields\|bulk/queue' tests/issue_edit_echo.rs
```
Must return at least 2 matches (one for the POST mock, one for the GET mock path).

```
grep -n 'status.success' tests/issue_edit_echo.rs | grep 'bulk_multi_key'
```
Must return 0 matches (the assertion is not on the same line as the function name);
instead verify the function body contains `output.status.success()`:
```
grep -A 30 'fn test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key' tests/issue_edit_echo.rs | grep 'status.success'
```
Must return a match.

### AC-003 — malformed VP comment fixed (TH-398-3)

`tests/issue_create_echo.rs` does not contain the string `VP-398-AC-014` anywhere.

The comment that previously read `// BC-3.4.014 EC-014; VP-398-AC-014.` now reads
`// BC-3.4.014 EC-014; AC-014.`

Verification:
```
grep 'VP-398-AC-014' tests/issue_create_echo.rs
```
Must return no output (exit 1 from grep — no match).

```
grep 'BC-3.4.014 EC-014; AC-014' tests/issue_create_echo.rs
```
Must return exactly one match.

### AC-004 — new `--description-stdin` create parity test added (TH-398-4)

`tests/issue_create_echo.rs` contains a new test function named:
`test_bc_3_4_014_create_description_stdin_echo_is_updated_marker`

The test:
- Uses `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`
- Mounts `POST /rest/api/3/issue` returning 201 with key `"PROJ-1"` (using `mount_post_201` or
  equivalent inline mock matching `tests/issue_create_echo.rs::mount_post_201`)
- Invokes `jr issue create --project PROJ --type Task --summary X --description-stdin`
  with `.write_stdin("Some stdin description\n")`
- Asserts `output.status.success()` is true
- Asserts `stderr.contains("  description \u{2192} (updated)")` — the `(updated)` marker
  must be present
- Asserts `!stderr.contains("Some stdin description")` — the stdin content must NOT
  appear in stderr
- Asserts `stdout.is_empty()` — table mode; no stdout output

Verification:
```
grep -c 'fn test_bc_3_4_014_create_description_stdin_echo_is_updated_marker' tests/issue_create_echo.rs
```
Must return exactly 1.

```
grep -A 30 'fn test_bc_3_4_014_create_description_stdin_echo_is_updated_marker' tests/issue_create_echo.rs | grep 'description-stdin'
```
Must return a match (the flag is passed to the command).

### AC-005 — `cargo test` exits 0

`cargo test` exits 0 with no special environment variables set. All tests in
`tests/issue_edit_echo.rs` and `tests/issue_create_echo.rs` pass.

Verification:
```
cargo test --test issue_edit_echo --test issue_create_echo
```
Must exit 0.

### AC-006 — formatting and lint clean

`cargo fmt --all -- --check` exits 0.
`cargo clippy --all-targets -- -D warnings` exits 0.
No `#[allow(...)]` attributes added.

### AC-007 — spec-count scripts pass

```
bash scripts/check-spec-counts.sh
bash scripts/check-bc-cumulative-counts.sh
bash scripts/check-bc-no-numeric-test-counts.sh
```
All three exit 0. No BC files are modified by this story.

## Out of Scope

- **Production code changes** — zero changes to `src/`. The tests are the deliverable.
- **BC file changes** — BC-3.4.012, BC-3.4.013, BC-3.4.014, BC-INDEX.md, and
  CANONICAL-COUNTS.md are NOT touched.
- **Script changes** — `scripts/check-bc-cumulative-counts.sh` and related scripts
  are addressed in Story B (S-400-B).
- **PG-398-1 through PG-398-5** — all process-gap items are in Story B or engine-tracked.
- **New test for `issue edit --description-stdin` table-mode echo** — the edit side already
  has `test_bc_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields` in
  JSON mode (`tests/issue_edit_echo.rs`). A table-mode parallel for edit-side stdin is not
  required by this story; TH-398-4 covers only the create side.
- **Mutation testing scope change** — TH-398-2's rewrite does not change the mutant
  escape rate; the story does not trigger a mutants run beyond what CI already gates.
- **Issue #429 or any other keychain-related work** — not related to this story.

## Implementation Strategy

**Ordered sequence:**

1. Create branch `test/S-400-A-echo-test-hardening` from `develop`.

2. **Read `tests/issue_edit_echo.rs` in full** before editing. Confirm the exact current
   body of `test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run` (around line 635) and
   `test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key` (around lines 673–709).
   Do NOT use line numbers from this story as ground truth — verify against the actual file.

3. **Read `tests/issue_bulk.rs` helper functions** `bulk_task_enqueued_response` and
   `bulk_task_complete_response` to confirm the exact JSON shapes. These are the canonical
   fixture helpers; the bulk-path test rewrite must match their shape.

4. **TH-398-1:** In `test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run`, add after
   the existing `assert!(!stderr.contains("  summary \u{2192} X"), ...)` assertion:
   ```rust
   assert!(
       !stderr.contains("Updated TEST-1"),
       "dry-run must not reach the success confirmation path; stderr={stderr}"
   );
   ```

5. **TH-398-2:** Rewrite `test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key`:
   - Remove or convert the two `PUT` mocks to `.expect(0)` sentinels
   - Add `POST /rest/api/3/bulk/issues/fields` mock: HTTP 200, body with
     `{"taskId": "task-bulk-echo-001", "status": "ENQUEUED", "progressPercent": 0,
      "totalIssueCount": 0, "processedAccessibleIssues": [], "failedAccessibleIssues": {},
      "invalidOrInaccessibleIssueCount": 0}`
   - Add `GET /rest/api/3/bulk/queue/task-bulk-echo-001` mock: HTTP 200, body with
     `{"taskId": "task-bulk-echo-001", "status": "COMPLETE", "progressPercent": 100,
      "totalIssueCount": 2, "processedAccessibleIssues": ["TEST-1", "TEST-2"],
      "failedAccessibleIssues": {}, "invalidOrInaccessibleIssueCount": 0}`
   - Add `assert!(output.status.success(), "Expected exit 0 for bulk path; stderr={stderr}");`
   - Retain `assert!(!stderr.contains("  summary \u{2192}"), ...);`

6. **TH-398-3:** In `tests/issue_create_echo.rs`, find the comment `// BC-3.4.014 EC-014;
   VP-398-AC-014.` (line 773 in the F1-analyzed version) and change `VP-398-AC-014` to
   `AC-014`. This is a one-token comment change.

7. **TH-398-4:** In `tests/issue_create_echo.rs`, add after the existing test functions
   a new test function `test_bc_3_4_014_create_description_stdin_echo_is_updated_marker`.
   The function mirrors `test_bc_3_4_014_create_description_echo_is_updated_marker`
   (the `--description` variant already in the file) but uses `--description-stdin` with
   `.write_stdin("Some stdin description\n")` instead of `--description "..."`.

8. **Compile:** `cargo build` exits 0 before running tests.

9. **Run:** `cargo test --test issue_edit_echo --test issue_create_echo` exits 0.

10. **Run:** `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`
    both exit 0.

11. **Run:** all three spec-count scripts exit 0.

12. Commit:
    ```
    test(echo): harden dry-run guard, fix bulk-path test, VP comment, add stdin create parity (closes #400 TH)
    ```
    All four files in one commit (two test files). No separate commits.

13. Open PR targeting `develop`; body includes `Closes #400` (shared with Story B — use
    the same issue reference; one of the two PRs will close it last, or the issue is closed
    manually when both PRs merge).

## Test Coverage Strategy

| Test type | Function | Location | What it tests |
|-----------|----------|----------|---------------|
| Strengthened negative guard (TH-398-1) | `test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run` | `tests/issue_edit_echo.rs` | Dry-run exits without reaching success-confirmation path |
| Rewritten success-path test (TH-398-2) | `test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key` | `tests/issue_edit_echo.rs` | Bulk path completes successfully and emits NO field-echo lines |
| Comment fix (TH-398-3) | n/a (cosmetic) | `tests/issue_create_echo.rs` line ~773 | No behavioral impact |
| New parity test (TH-398-4) | `test_bc_3_4_014_create_description_stdin_echo_is_updated_marker` | `tests/issue_create_echo.rs` | `issue create --description-stdin` echoes `(updated)` marker, not content |

Net suite delta: +1 test function (TH-398-4), 0 test functions deleted. The dry-run test
gains one assertion (TH-398-1). The bulk test is rewritten in-place (TH-398-2).

## Quality Gate Self-Check

| Criterion | Required | Verification command |
|-----------|----------|----------------------|
| TH-398-1: new assertion present in dry-run test | AC-001 | `grep -A 10 'fn test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run' tests/issue_edit_echo.rs \| grep 'Updated TEST-1'` → match |
| TH-398-2: bulk POST mock present | AC-002 | `grep 'bulk/issues/fields' tests/issue_edit_echo.rs` → match |
| TH-398-2: bulk GET poll mock present | AC-002 | `grep 'bulk/queue' tests/issue_edit_echo.rs` → match |
| TH-398-2: success assertion present | AC-002 | `grep -A 30 'fn test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key' tests/issue_edit_echo.rs \| grep 'status.success'` → match |
| TH-398-3: malformed VP identifier removed | AC-003 | `grep 'VP-398-AC-014' tests/issue_create_echo.rs` → no output |
| TH-398-3: corrected comment present | AC-003 | `grep 'BC-3.4.014 EC-014; AC-014' tests/issue_create_echo.rs` → 1 match |
| TH-398-4: new test function present | AC-004 | `grep -c 'fn test_bc_3_4_014_create_description_stdin_echo_is_updated_marker' tests/issue_create_echo.rs` → 1 |
| TH-398-4: --description-stdin flag used | AC-004 | `grep -A 20 'fn test_bc_3_4_014_create_description_stdin_echo_is_updated_marker' tests/issue_create_echo.rs \| grep 'description-stdin'` → match |
| TH-398-4: (updated) marker asserted | AC-004 | `grep -A 30 'fn test_bc_3_4_014_create_description_stdin_echo_is_updated_marker' tests/issue_create_echo.rs \| grep 'updated'` → match |
| `cargo test` exits 0 | AC-005 | `cargo test --test issue_edit_echo --test issue_create_echo` |
| `cargo fmt --check` exits 0 | AC-006 | `cargo fmt --all -- --check` |
| `cargo clippy` exits 0 | AC-006 | `cargo clippy --all-targets -- -D warnings` |
| spec-count scripts exit 0 | AC-007 | `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` |
| No src/ files modified | (scope) | `git diff --name-only \| grep '^src/'` → no output |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~5 k |
| `tests/issue_edit_echo.rs` (full, ~957 LOC) | ~12 k |
| `tests/issue_create_echo.rs` (full, ~916 LOC) | ~12 k |
| `tests/issue_bulk.rs` (targeted: helper functions only, ~lines 116–172) | ~2 k |
| `src/types/jira/bulk.rs` (full, ~285 LOC) | ~4 k |
| Tool outputs (`cargo test`, `cargo clippy`, grep verifications) | ~3 k |
| **Total** | **~38 k** |

Well within a single-agent context window (~200 k). No split required.
LOC delta estimate: `tests/issue_edit_echo.rs` net +10 LOC (1 assertion + bulk mock expansion).
`tests/issue_create_echo.rs` net +35 LOC (1 comment change + 1 new test function ~35 LOC).

## Tasks

- [ ] Create branch `test/S-400-A-echo-test-hardening` from `develop`
- [ ] Read `tests/issue_edit_echo.rs` in full — confirm exact current body of `test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run` and `test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key` (do NOT use line numbers from this story as anchors — verify in the actual file)
- [ ] Read `tests/issue_bulk.rs` helpers `bulk_task_enqueued_response` and `bulk_task_complete_response` — confirm exact JSON shapes for bulk mock bodies
- [ ] Read `src/types/jira/bulk.rs` — confirm `BulkSubmitResponse` uses `task_id` deserialized from camelCase `taskId` and `BulkOperationProgress` field names
- [ ] TH-398-1: add `!stderr.contains("Updated TEST-1")` assertion to `test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run` with descriptive message (AC-001)
- [ ] TH-398-2: rewrite `test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key` — replace/convert PUT mocks to `.expect(0)`, add bulk POST mock returning taskId `"task-bulk-echo-001"`, add bulk GET poll mock returning COMPLETE, add `output.status.success()` assertion, retain `!stderr.contains("  summary \u{2192}")` (AC-002)
- [ ] Read `tests/issue_create_echo.rs` — locate the comment `VP-398-AC-014` (around line 773 in the test `test_bc_3_4_014_create_jsm_request_type_path_no_field_echo`) and the existing `test_bc_3_4_014_create_description_echo_is_updated_marker` function for use as a template
- [ ] TH-398-3: change comment `VP-398-AC-014` → `AC-014` (AC-003)
- [ ] TH-398-4: add `test_bc_3_4_014_create_description_stdin_echo_is_updated_marker` — `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`, mount POST 201, call with `--description-stdin` + `.write_stdin("Some stdin description\n")`, assert `status.success()`, assert `stderr.contains("  description \u{2192} (updated)")`, assert `!stderr.contains("Some stdin description")`, assert `stdout.is_empty()` (AC-004)
- [ ] `cargo build` exits 0
- [ ] `cargo test --test issue_edit_echo --test issue_create_echo` exits 0 (AC-005)
- [ ] `cargo fmt --all -- --check` exits 0 (AC-006)
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0 (AC-006)
- [ ] `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` — all exit 0 (AC-007)
- [ ] Commit atomically (both test files in one commit): `test(echo): harden dry-run guard, fix bulk-path test, VP comment, add stdin create parity (closes #400 TH)`
- [ ] Open PR targeting `develop`; body includes `Closes #400`

## Previous Story Intelligence

**Direct predecessor: S-398** shipped BC-3.4.012/013/014 and the changed-fields echo feature.
The four items in this story are quality-level follow-ups from that cycle — identified by the
F1 delta analysis for issue #400 after the S-398 cycle converged.

**Lesson from S-428 (L-410-1):** Always read the test file in full before editing — do not
navigate by line numbers from the delta analysis, which may have drifted since the F1 analysis
was written. Verify function names and surrounding context against the actual file.

**Lesson from S-398 TH-398-2 diagnosis (F1 delta analysis):** The bulk-path routing fork is
confirmed at `src/cli/issue/create.rs::handle_edit` — when `effective_keys.len() > 1`, the
function returns `handle_edit_bulk_fields(...)` immediately, bypassing any single-key PUT path.
The PUT mocks mounted by the original test were structurally impossible to call. The fix must
mock the actual chain: `POST /rest/api/3/bulk/issues/fields` → `GET /rest/api/3/bulk/queue/{taskId}`.

**Lesson from PR #356 R14-R18:** doc/comment changes should be in the same commit as the
functional changes they accompany. Here: the VP comment fix (TH-398-3) travels in the same
commit as TH-398-4. No separate cosmetic-fix commit.

**Key invariant from CLAUDE.md #398 Gotcha:** The `  description → (updated)` vs raw-string
asymmetry is intentional and load-bearing. TH-398-4 must assert the `(updated)` marker in
table mode and must NOT assert the raw content. The mirror test on edit side
(`test_bc_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields`) operates in
JSON mode and carries the raw string — that is a different surface, not a contradiction.

## Architecture Compliance Rules

1. **Zero production code changes.** `src/` is not modified. Any implementation path that
   requires changing production code is a scope violation — stop and escalate.

2. **No BC file changes.** `.factory/specs/prd/bc-*.md`, `BC-INDEX.md`, and
   `CANONICAL-COUNTS.md` are not modified. The three spec-count scripts must pass with
   the current file state.

3. **Bulk mock shapes must match `src/types/jira/bulk.rs`.** The `BulkSubmitResponse`
   JSON field is `taskId` (camelCase); the `#[serde(rename_all = "camelCase")]` attribute
   on the struct means the mock response body must use `"taskId"` not `"task_id"`.
   `BulkOperationProgress` similarly uses camelCase field names.

4. **No `#[allow(...)]` suppressions.** If clippy warns on any new test code, refactor
   the test to satisfy the lint rather than suppressing it.

5. **Test naming convention.** TH-398-4 uses `test_<verb>_<subject>_<expected_outcome>`
   per `docs/specs/test-naming-convention.md`. The name
   `test_bc_3_4_014_create_description_stdin_echo_is_updated_marker` follows this convention
   (verb=create, subject=description_stdin_echo, outcome=is_updated_marker).

6. **Write stdin with `.write_stdin()`** (the `assert_cmd` API). Do not use
   `std::process::Stdio::piped()` directly — the harness uses `assert_cmd::Command` which
   exposes `.write_stdin()` as a builder method.

7. **No new helper functions in test files** unless the pattern is used more than once. The
   new TH-398-4 test function should inline its mock setup using `mount_post_201` (already
   defined in `tests/issue_create_echo.rs`) rather than introducing a new private helper.

## Library & Framework Requirements

No new dependencies. The rewrite uses only:
- `wiremock` + `assert_cmd` (already in dev-dependencies)
- `serde_json::json!` macro (already in dev-dependencies)
- `tempfile::tempdir()` if XDG isolation is needed (already in dev-dependencies; not
  required for TH-398-2 since the bulk-path test does not involve teams or story-points fields)

The new `--description-stdin` test (TH-398-4) does not need `jr_cmd_with_xdg` or custom
XDG dirs — description-stdin reads from stdin, not from a config field.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `tests/issue_edit_echo.rs` | Modify | TH-398-1: +1 assertion in dry-run test; TH-398-2: rewrite bulk-path test body |
| `tests/issue_create_echo.rs` | Modify | TH-398-3: fix one comment token; TH-398-4: add one test function |

**Files NOT to create:** No new test files, no new source files, no new spec files.

**Files NOT to touch:** all `src/` files, all `.factory/specs/prd/` files, `scripts/`,
`CLAUDE.md`, `STORY-INDEX.md` (state-manager handles that), `Cargo.toml`, `deny.toml`,
`CHANGELOG.md` (no user-visible behavior change).

## Branch / PR Plan

- Branch: `test/S-400-A-echo-test-hardening`
- Target: `develop`
- Commit style: `test(echo): harden dry-run guard, fix bulk-path test, VP comment, add stdin create parity (closes #400 TH)`
- PR closes: `Closes #400` (or a partial reference if Story B has a separate PR)
- CHANGELOG entry: not required (test infrastructure; no user-visible behavior change)
