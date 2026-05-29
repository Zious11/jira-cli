---
issue: 400
title: "Test-hardening + process-gap follow-ups from #398 (S-398) VSDD cycle"
phase: F1
date: 2026-05-28
analyst: architect
status: draft — awaiting human gate
---

# F1 Delta Analysis — GitHub Issue #400

## 0. Source Grounding

All claims below are verified against actual file content, not against the issue
text alone. Sources read:

- `tests/issue_edit_echo.rs` (full file, 957 LOC)
- `tests/issue_create_echo.rs` (full file, 916 LOC)
- `scripts/check-bc-cumulative-counts.sh` (full file, 252 LOC)
- `scripts/check-spec-counts.sh` (full file, 62 LOC)
- `scripts/check-bc-no-numeric-test-counts.sh` (full file, 79 LOC)
- `src/cli/issue/create.rs` line 849–868 (multi-key routing fork)
- `src/api/jira/bulk.rs` lines 241–270 (bulk endpoint: `POST /rest/api/3/bulk/issues/fields`)
- `src/api/jira/bulk.rs` lines 297–325 (task poll: `GET /rest/api/3/bulk/queue/{taskId}`)
- `.factory/phase-f2-spec-evolution/prd-delta-398.md` lines 331–338
  (origin of PG-398-1/2/3 process-gap notices)

---

## 1. Per-Item Impact Boundary

### TH-398-1 — dry-run exclusion guard test is near-vacuous

**File:** `/Users/zious/Documents/GITHUB/jira-cli/tests/issue_edit_echo.rs`
**Function:** `test_bc_3_4_012_edit_echo_does_not_fire_on_dry_run` (lines 635–662)
**Change type:** test-only

**Verified claim:** The test at lines 658–661 asserts only:
```
!stderr.contains("  summary → X")
```
It does not assert that the success confirmation `"Updated TEST-1"` is absent.
The test comment (lines 629–632) acknowledges this: "Note: --dry-run requires a JQL or
multiple keys; single key exits before PUT." The comment is therefore accurate about
what the test does. However the issue's complaint is also accurate: a test that only
checks for the absence of a specific echo line can pass vacuously before and after
implementation via any exit path, including a non-zero exit for unrelated reasons.

**Fix scope:** Add a second assertion in the same test function:
```rust
assert!(
    !stderr.contains("Updated TEST-1"),
    "dry-run must not reach the success confirmation path; stderr={stderr}"
);
```
No new test function needed; the existing function is the right home.
Single-file, single-function edit. Zero BC impact.

---

### TH-398-2 — bulk-exclusion guard test passes on an error path

**File:** `/Users/zious/Documents/GITHUB/jira-cli/tests/issue_edit_echo.rs`
**Function:** `test_bc_3_4_012_edit_echo_excluded_for_bulk_multi_key` (lines 673–709)
**Change type:** test-only (but requires mock setup for two additional endpoints)

**Verified claim — the routing is confirmed in source:**

`src/cli/issue/create.rs` lines 857–867:
```
if effective_keys.len() > 1 {
    return handle_edit_bulk_fields(&effective_keys, ...).await;
}
```

`handle_edit_bulk_fields` (`src/cli/issue/create.rs` line 1249) calls
`client.bulk_edit_fields(...)` which POSTs to `/rest/api/3/bulk/issues/fields`
(`src/api/jira/bulk.rs` line 267) and then polls via
`GET /rest/api/3/bulk/queue/{taskId}` (line 306).

The test at lines 676–688 mounts:
- `PUT /rest/api/3/issue/TEST-1` → 204
- `PUT /rest/api/3/issue/TEST-2` → 204

Neither of these is ever called for a two-key edit. The code routes through the bulk
path immediately. Because neither mock is registered with `.expect(0)` or `.expect(1)`,
wiremock does not assert on them at all — they are merely mounted and ignored. The
command fails because `POST /rest/api/3/bulk/issues/fields` is not mocked (wiremock
returns 404), and the test passes because `stderr.contains("  summary →")` is false
on an error path.

**Fix scope:** The test needs to be rewritten to:
1. Remove the two unmounted PUT mocks (or keep them with `.expect(0)` as explicit
   "must-not-call" sentinels).
2. Mount `POST /rest/api/3/bulk/issues/fields` returning a 200 with a `taskId`.
3. Mount `GET /rest/api/3/bulk/queue/{taskId}` returning a terminal status (e.g.,
   `{"status":"COMPLETE","progress":{"total":2,"processed":2,"successful":2}}`).
4. Assert `output.status.success()` so the test exercises the success path.
5. Assert the current negative: `!stderr.contains("  summary →")`.

This is a test rewrite (same function, expanded mock setup). The bulk task-status
shape used by `await_bulk_task_inner` must be matched; look at existing bulk tests
in `tests/` for the correct wiremock shape.

**Complexity note:** This is the most non-trivial of the four TH items because
getting the bulk mock response shape right requires understanding both
`BulkSubmitResponse` (the taskId extraction) and `BulkTaskStatus` (the polling
loop termination). But it is still test-only — no production code changes.

---

### TH-398-3 — mislabeled identifier in a test comment (cosmetic)

**File:** `/Users/zious/Documents/GITHUB/jira-cli/tests/issue_create_echo.rs`
**Location:** Line 773, test `test_bc_3_4_014_create_jsm_request_type_path_no_field_echo`
**Change type:** test-only (comment edit)

**Verified claim:** Line 773 reads:
```
// BC-3.4.014 EC-014; VP-398-AC-014.
```
The identifier `VP-398-AC-014` is malformed. VPs in this project use the form
`VP-398-001` through `VP-398-006` (six VPs exist for S-398). `AC-014` is an
acceptance-condition identifier, not a VP number. The correct citation is `AC-014`
alone (i.e., drop the `VP-398-` prefix), as the section header on line 771–772
already correctly labels it: `"Test 21 — AC-014"`.

The fix is a single-line comment change: `VP-398-AC-014` → `AC-014`.
Zero behavioral impact; zero BC impact.

---

### TH-398-4 — `issue create --description-stdin` echo has no test

**File:** `/Users/zious/Documents/GITHUB/jira-cli/tests/issue_create_echo.rs`
**Change type:** test-only (new test function)

**Verified claim:** Scanning all test function names in `issue_create_echo.rs`:

- `test_bc_3_4_014_create_all_fields_echo_alphabetical_order` — uses `--description`
- `test_bc_3_4_014_create_description_echo_is_updated_marker` — uses `--description` (line 334)
- No test in the file uses `--description-stdin`

On the edit side (`issue_edit_echo.rs`), `--description-stdin` is covered by
`test_bc_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields`
(lines 476–509), which uses `.write_stdin("My description\n")`.

The parity gap is real. A create-side `--description-stdin` test would verify that
(a) `  description → (updated)` appears in stderr (table mode), and (b) the `(updated)`
marker — not content — is emitted. The test function name under the project naming
convention should be:
`test_bc_3_4_014_create_description_stdin_echo_is_updated_marker`

This is a new test function in an existing test file. Mock setup mirrors
`test_bc_3_4_014_create_description_echo_is_updated_marker` but adds
`.write_stdin(...)` and swaps `--description` for `--description-stdin`.

---

## 2. Repo-Boundary Split (CRITICAL)

| Item | Classification | Rationale |
|------|---------------|-----------|
| TH-398-1 | IN-SCOPE | `tests/issue_edit_echo.rs` — jira-cli test file |
| TH-398-2 | IN-SCOPE | `tests/issue_edit_echo.rs` — jira-cli test file |
| TH-398-3 | IN-SCOPE | `tests/issue_create_echo.rs` — jira-cli test file |
| TH-398-4 | IN-SCOPE | `tests/issue_create_echo.rs` — jira-cli test file |
| PG-398-1 | IN-SCOPE | `scripts/check-bc-cumulative-counts.sh` — jira-cli repo script |
| PG-398-2 | PARTIALLY OUT-OF-SCOPE | See below |
| PG-398-3 | PARTIALLY OUT-OF-SCOPE | See below |
| PG-398-4 | OUT-OF-SCOPE | Dark-factory engine adversary dispatch protocol |
| PG-398-5 | OUT-OF-SCOPE | Dark-factory engine F2 delta-authoring validation |

**PG-398-2 detail:** The issue says the "delta-authoring checklist" should be extended
to include CANONICAL-COUNTS.md Breakdown-bullet numeric literals and the frontmatter
`last_verified` field. The `last_verified` field and Breakdown bullets live in
`.factory/specs/prd/CANONICAL-COUNTS.md` in the jira-cli repo, but the
"delta-authoring checklist" itself is a factory engine prompt/protocol that drives
F2 spec-evolution agents — it does not live in the jira-cli repo. The jira-cli
deliverable is: add a guard to `scripts/check-bc-cumulative-counts.sh` that checks
the Breakdown-bullet numeric literals in `CANONICAL-COUNTS.md`. The `last_verified`
field is a datestamp with no computed invariant, so it cannot be automatically
validated by a script (it is always correct by construction when the author updates
it). The engine-side deliverable is: update the F2 checklist prompt. Those are two
separate concerns.

**PG-398-3 detail:** The VP-count tri-surface invariant (`new_vps:` frontmatter length
== `### VP-` heading count == VP-to-BC mapping-table row count) lives in verification-
delta documents under `.factory/phase-f2-spec-evolution/`. Adding an automated guard
script would require a new script in the jira-cli `scripts/` directory that parses
`.factory/phase-f2-spec-evolution/verification-delta-*.md` files. That is technically
in-scope for the jira-cli repo (the files are checked in here). However, the trigger
for this guard is F2 authoring, not standard development workflow — so the guard is
useful only if it is also wired into CI or the F7 convergence checklist. This is
borderline; the human gate should decide whether to include a new guard script or
treat this as a factory-engine-only protocol change.

**PG-398-4 detail:** The adversary dispatch protocol (how the F5 adversarial agent
resolves its worktree path before reviewing code) is an engine-internal behavior.
The bug described — "adversarial pass returned a false NOT-CLEAN because it
mis-resolved the worktree path and reviewed the develop baseline" — lives in the
engine's agent prompts or orchestration logic. There is no jira-cli source artifact
to modify. This must be tracked in the engine repo, not closed via a jira-cli PR.

**PG-398-5 detail:** The F2 delta-authoring identifier validation (checking that
pinned function-name tokens respect the project's `non_snake_case` + zero-warning
policy) is a factory engine concern: it describes what the F2 spec-evolution agent
should validate before locking CLAUDE.md Gotcha entries. There is no jira-cli
script or file that implements or could implement this validation generically.
This must be tracked in the engine repo.

---

## 3. Recommended Decomposition

### Option A: Two stories (recommended)

**Story A — Test hardening (TH-398-1..4)** — pure test changes

- Files: `tests/issue_edit_echo.rs`, `tests/issue_create_echo.rs`
- No production code, no spec files, no scripts
- All four items are test-only; a single PR keeps the diff easy to review
- TH-398-2 is the most work (mock rewrite) but still bounded to one function
- Mutation coverage: TH-398-1/2 strengthen existing guards; TH-398-3 is cosmetic;
  TH-398-4 adds a new test. No new mutants are expected to escape.
- BC impact: none
- Suggested branch: `test/issue-400-th-398-hardening`

**Story B — Count-guard extension (PG-398-1, and optionally PG-398-3)** — scripts only

- Files: `scripts/check-bc-cumulative-counts.sh`; optionally a new
  `scripts/check-vp-delta-counts.sh`
- No production code, no test files
- PG-398-1 extends an existing script; PG-398-3 adds a new script if in-scope
- These are infrastructure; keeping them separate from the test PR reduces noise
- Suggested branch: `chore/issue-400-pg-398-count-guards`

**Items deferred to engine or closed as not-jira-cli:**

- PG-398-2 (last_verified field): cannot be automatically validated; the
  Breakdown-bullet numeric literal extension should be lumped into Story B if the
  human approves adding it to `check-bc-cumulative-counts.sh`; the `last_verified`
  datestamp portion should be closed as engine-side checklist only.
- PG-398-4: close as engine-tracked; not actionable in this repo.
- PG-398-5: close as engine-tracked; not actionable in this repo.

### Option B: Single story

All in-scope items (TH-398-1..4 + PG-398-1) in one PR. Lower overhead if the
human prefers fewer issues to close. The tradeoff is a wider diff (tests + scripts
in the same PR), which slightly complicates targeted code review. Recommended only
if the team wants to burn this down in a single maintenance pass.

### Not recommended: One story per item

Seven individual PRs for what are essentially two classes of change would create
excessive overhead. TH items are naturally batched; PG items that are in-scope are
naturally batched.

---

## 4. Intent / Scope / Severity Classification

| Item | Intent | Change class | Severity | Blocking? |
|------|--------|-------------|----------|-----------|
| TH-398-1 | Strengthen near-vacuous negative guard | test-debt | Trivial | No |
| TH-398-2 | Replace error-path pass with success-path pass | test-debt | Non-trivial (mock rewrite) | No |
| TH-398-3 | Fix mislabeled VP comment reference | cosmetic | Trivial | No |
| TH-398-4 | Add missing parity test | test-debt | Trivial–low | No |
| PG-398-1 | Extend guard script coverage | infrastructure | Non-trivial (script logic) | No |
| PG-398-2 | Update delta-authoring checklist | infrastructure / engine | Trivial | No |
| PG-398-3 | Add VP-count tri-surface guard | infrastructure | Non-trivial (new script) | No |
| PG-398-4 | Codify adversary worktree sanity gate | engine protocol | N/A in this repo | No |
| PG-398-5 | Validate identifiers before locking spec | engine protocol | N/A in this repo | No |

All nine items are confirmed non-blocking per the issue. The #398 cycle converged:
mutation 100%, 0 CRITICAL/HIGH findings. No shipped behavior was affected.

TH-398-2 is the only "non-trivial" test item because correctly mocking the
`POST /rest/api/3/bulk/issues/fields` + `GET /rest/api/3/bulk/queue/{taskId}` chain
requires understanding the `BulkSubmitResponse` / `BulkTaskStatus` JSON shapes. All
other TH items are trivial additions or comment fixes.

PG-398-1 is non-trivial if it adds guards for the Coverage Statistics table in
`BC-INDEX.md` (those sections have no current extraction pattern in the script)
and the `bc-*.md` JSON Output Shape Contracts tables (which have no structured
frontmatter anchors). The script author will need to decide on parsing strategies.
PG-398-3 is non-trivial because it requires a new script with its own argument
parsing to validate `.factory/phase-f2-spec-evolution/verification-delta-*.md`
frontmatter against body heading counts.

---

## 5. BC Impact

**No BC changes.** Confirmed by review of all nine items:

- TH-398-1..4 are test file changes. No modification to any `.factory/specs/prd/`
  BC file, no new behavioral contracts, no VP updates.
- PG-398-1/2/3 are script or checklist changes. They validate existing BC count
  surfaces — they are tooling changes, not behavioral contract changes. Extending
  `check-bc-cumulative-counts.sh` to cover additional surfaces tightens enforcement
  of existing BCs; it does not define new ones.
- BC-3.4.012/013/014 (the description-echo contracts from S-398) are unchanged.
- VP-398-001..006 are unchanged.

The description-echo asymmetry invariant documented in the CLAUDE.md Gotcha
("`issue edit` description echo asymmetry (issue #398)") — where table-mode emits
`(updated)` and JSON carries the raw input string — is already pinned by tests
`test_bc_3_4_012_description_echo_is_updated_marker_not_content` (edit file,
line 233) and `test_bc_3_4_013_description_echo_is_raw_input_string_not_marker`
(edit file, line 428). TH-398-4 adds the create-side stdin parity test but does
not modify any existing assertion.

---

## 6. Open Questions for the F1 Human-Approval Gate

**Q1 (CRITICAL — repo boundary):** PG-398-4 and PG-398-5 cannot be delivered in
the jira-cli repo. Should they be tracked as separate issues in the engine repo, or
closed on #400 with a comment noting they are engine-scope? The human must decide
before F2 to avoid a PR that incorrectly claims to close all nine items.

**Q2 (scope decision):** PG-398-3 (VP-count tri-surface guard) is borderline
in-scope. The verification-delta files live in this repo, so a guard script is
technically possible. However, the guard only triggers during F2 authoring and
provides value only when wired into CI or the F7 checklist. Should it be in-scope
for Story B (adding a new script) or deferred to an engine-side checklist update?

**Q3 (decomposition):** One story (Option B) or two stories (Option A)? Two stories
is recommended to keep the test-only PR atomic and separately reviewable from the
script/infrastructure PR, but the human may prefer to close #400 with a single PR.

**Q4 (PG-398-2 split):** The `last_verified` datestamp cannot be validated by script
(it is a free-text field with no computable invariant). Should the Breakdown-bullet
portion be added to Story B and the `last_verified` portion be noted as "no
automated guard possible, author discipline only"?

**Q5 (TH-398-2 complexity acknowledgement):** The bulk mock rewrite is the most
substantial test change. Before assigning, the implementer should review the shape of
`BulkSubmitResponse` in `src/types/jira/bulk.rs` and look at existing bulk tests in
`tests/` for the wiremock response format. This is a known complexity item, not a
blocker, but worth flagging before estimation.

---

## Summary

**Recommended scope split:**

- **Story A** (in-scope, test-only): TH-398-1, TH-398-2, TH-398-3, TH-398-4 — all
  in `tests/issue_edit_echo.rs` and `tests/issue_create_echo.rs`. Pure test changes,
  no BC impact, no production code.
- **Story B** (in-scope, infrastructure): PG-398-1 (extend `check-bc-cumulative-counts.sh`),
  optionally PG-398-3 (new VP-delta guard script, subject to Q2 decision above).
  PG-398-2 partially in-scope (Breakdown-bullet guard only; `last_verified` is not
  automatable).
- **Engine-tracked:** PG-398-4, PG-398-5. These must be referred to the dark-factory
  engine repo. They cannot be delivered as jira-cli artifacts.

**Proposed PRs:** 2 (one for TH items, one for PG items), or 1 if the human prefers
to consolidate. Engine items tracked separately.

**Key open questions for gate:** repo-boundary decision for PG-398-4/5 (Q1), and
whether PG-398-3 is in-scope for this issue (Q2).
