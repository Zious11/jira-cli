---
document_type: story
story_id: "S-407"
title: "--label conflict block: 10 missing positive regression tests + structural meta-test (closes #407)"
wave: feature-followup
status: ready
intent: test-hardening
feature_type: test-only
scope: small
issue: 407
points: 1
priority: medium
tdd_mode: strict
estimated_effort: small
depends_on: [S-396]  # S-396 delivered FIX-F5-001: the first 2/12 positive regression tests + the conflict block production code; S-407 completes coverage to 12/12
bc_anchors:
  - BC-3.4.017
verification_properties: []
# No new VPs — this is regression coverage for existing BC-3.4.017 invariant 2.
# The structural meta-test functions as an inline mechanical witness for the invariant;
# it is not a standalone VP because it operates on source-text structure, not observable
# runtime behavior.
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f2-spec-evolution/prd-delta-407.md"
implementation_strategy: tdd
module_criticality: LOW  # tests/issue_edit_field.rs (modify — add 10 tests) + src/cli/issue/create.rs (modify — add meta-test in existing #[cfg(test)] block + add guard comment at line ~445); no production logic changes
files_modified:
  - tests/issue_edit_field.rs     # MODIFIED — Add 10 positive regression tests (one per untested --label conflict-block entry) in existing test file; est. +120–150 LOC
  - src/cli/issue/create.rs       # MODIFIED — (1) Add meta-test test_label_conflict_block_lists_every_relevant_flag in existing #[cfg(test)] block; (2) Add guard comment at ~line 445 reserving the 'conflicting' variable name for the --label block; est. +40–60 LOC in tests block + 3 LOC comment
files_created: []
breaking_change: false
assumption_validations: []
risk_mitigations: []
# BC status: BC-3.4.017 sealed since issue #396 F2 (2026-05-22, adversary pass 9).
# EC-3.4.017-14 added in issue #407 F2 (2026-05-25). No new BCs; do NOT re-edit BC files.
---

# S-407 — `--label` Conflict Block: 10 Positive Regression Tests + Structural Meta-Test

## Source of Truth

F1 delta analysis: `.factory/phase-f1-delta-analysis/issue-407/delta-analysis.md`
F2 PRD delta: `.factory/phase-f2-spec-evolution/prd-delta-407.md` (no new BCs; EC-3.4.017-14 added)
BC body: `.factory/specs/prd/bc-3-issue-write.md` §BC-3.4.017, invariant 2, EC-3.4.017-13..14.

**No new BCs. No new VPs.** BC count surfaces unchanged: 583 total, bc-3 at 103/74.

## Problem Statement

The `--label` mutual-exclusion block in `handle_edit` (`src/cli/issue/create.rs:445-492`)
enumerates 12 conflicting flags. FIX-F5-001 (PR #406, part of S-396) added the first two
positive regression tests (`test_label_plus_field_rejected_with_exit_64_no_http`,
`test_label_plus_summary_rejected_with_exit_64_no_http`). The remaining 10 entries have
zero positive regression coverage:

```
--priority, --type, --team, --points, --no-points,
--parent, --no-parent, --description, --description-stdin, --markdown
```

Any `conflicting.push("--xxx")` call for these 10 flags can be deleted and the entire test
suite passes — `cargo-mutants` would flag each deletion as a surviving mutant. Additionally,
no test enforces the structural invariant that the conflict block's enumerated set stays in
sync with the flag surface (the existing `test_343_every_edit_field_is_categorized` covers
the SELECTORS/BULK_SUPPORTED/REJECTED_IN_BULK partition only; it never reads the `--label`
conflict block).

This story completes coverage to 12/12 and adds a structural meta-test.

## Behavioral Contracts

| BC ID | File | Title | Clause(s) |
|-------|------|-------|-----------|
| BC-3.4.017 | `bc-3-issue-write.md` | `--field` multi-key/`--jql` multi-issue rejection (C-1 guard) + flag-overlap hard error + `--label` conflict block completeness | invariant 2 (partition exhaustiveness + conflict-block completeness), EC-3.4.017-13 (FIX-F5-001 -- `--label` + `--field` rejection pattern), EC-3.4.017-14 (meta-test mechanical witness) |

## Acceptance Criteria

### AC-001 — `--label` + `--priority` → exit 64, zero HTTP
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_priority_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(wiremock::matchers::any()).expect(0)` catch-all — zero HTTP calls permitted.
- Invocation: `jr issue edit TEST-1 --label add:x --priority High`
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"` (separate assertion).
- Assert stderr contains `"--priority"` (separate assertion).

### AC-002 — `--label` + `--type` → exit 64, zero HTTP
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_type_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(any()).expect(0)` catch-all.
- Invocation: `jr issue edit TEST-1 --label add:x --type Bug`
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"`.
- Assert stderr contains `"--type"`.

### AC-003 — `--label` + `--team` → exit 64, zero HTTP
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_team_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(any()).expect(0)` catch-all.
- Invocation: `jr issue edit TEST-1 --label add:x --team "Platform Core"`
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"`.
- Assert stderr contains `"--team"`.

### AC-004 — `--label` + `--points` → exit 64, zero HTTP
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_points_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(any()).expect(0)` catch-all.
- Invocation: `jr issue edit TEST-1 --label add:x --points 5`
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"`.
- Assert stderr contains `"--points"`.

### AC-005 — `--label` + `--no-points` (boolean flag) → exit 64, zero HTTP
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_no_points_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(any()).expect(0)` catch-all.
- Invocation: `jr issue edit TEST-1 --label add:x --no-points` (no value argument — boolean flag)
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"`.
- Assert stderr contains `"--no-points"`.

### AC-006 — `--label` + `--parent` → exit 64, zero HTTP
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_parent_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(any()).expect(0)` catch-all.
- Invocation: `jr issue edit TEST-1 --label add:x --parent EPIC-1`
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"`.
- Assert stderr contains `"--parent"`.

### AC-007 — `--label` + `--no-parent` (boolean flag) → exit 64, zero HTTP
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_no_parent_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(any()).expect(0)` catch-all.
- Invocation: `jr issue edit TEST-1 --label add:x --no-parent` (no value argument — boolean flag)
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"`.
- Assert stderr contains `"--no-parent"`.

### AC-008 — `--label` + `--description` → exit 64, zero HTTP
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_description_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(any()).expect(0)` catch-all.
- Invocation: `jr issue edit TEST-1 --label add:x --description "some text"`
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"`.
- Assert stderr contains `"--description"`.

### AC-009 — `--label` + `--description-stdin` (boolean flag) → exit 64, zero HTTP; no stdin pipe needed
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_description_stdin_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(any()).expect(0)` catch-all.
- Invocation: `jr issue edit TEST-1 --label add:x --description-stdin` (no stdin pipe required —
  the `--label` conflict guard at line 474 fires BEFORE the stdin read at line 882; the process
  exits 64 before any stdin I/O occurs).
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"`.
- Assert stderr contains `"--description-stdin"`.

### AC-010 — `--label` + `--markdown` (paired with `--description`) → exit 64, zero HTTP; TWO separate stderr checks
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_plus_markdown_rejected_with_exit_64_no_http` in `tests/issue_edit_field.rs`:

- `Mock::given(any()).expect(0)` catch-all.
- Invocation: `jr issue edit TEST-1 --label add:x --markdown --description "some text"`
  (MUST pair `--markdown` with `--description` — `--markdown` alone without a description
  fires an EARLIER guard at `create.rs:357-363` that exits 64 with a different message:
  `"--markdown requires --description..."`. Pairing with `--description "some text"` satisfies
  the pre-guard and lets execution reach the `--label` conflict block. The test comment MUST
  document this reason.)
- Assert exit code 64.
- Assert stderr contains `"--label cannot be combined with"` (SEPARATE assertion — do NOT
  assert `"--label cannot be combined with --markdown"` as a single concatenated substring;
  the conflict block joins both `--description` and `--markdown` into a comma-separated list,
  so the literal `"--label cannot be combined with --markdown"` does NOT appear verbatim).
- Assert stderr contains `"--markdown"` (SEPARATE assertion — confirms the `--markdown` row
  in the conflict block cannot be deleted without this test failing).

### AC-011 — Meta-test: deletion failure mode — drop any `conflicting.push` line → meta-test fails
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

`test_label_conflict_block_lists_every_relevant_flag` in `src/cli/issue/create.rs` `#[cfg(test)]`
block (approach (b) — dedicated function, no production code changes):

- Uses `include_str!("create.rs")` (file path is relative to the test module; the test lives
  IN `create.rs`, so the path is just `"create.rs"`).
- Extracts every `conflicting.push("--<flag>")` literal from the ENTIRE file via global scan
  (safe because `conflicting` is used exclusively within the `if !labels.is_empty()` block in
  `handle_edit` — guarded by the reserved-variable-name comment added at line ~445).
- Builds expected `BTreeSet<String>` (NOT `HashSet` — deterministic failure diffs) from
  `(BULK_SUPPORTED \ {"label"}) ∪ REJECTED_IN_BULK` mapped to kebab-case flag names.
- When any `conflicting.push("--xxx")` line is removed from the source, the extracted set
  loses a member, fails the equality assertion, and `cargo test` reports the failure.

### AC-012 — Meta-test: addition failure mode — new Edit field added to BULK_SUPPORTED or REJECTED_IN_BULK without extending the conflict block → meta-test fails
(traces to BC-3.4.017 invariant 2 + EC-3.4.017-14)

The same `test_label_conflict_block_lists_every_relevant_flag` covers this:

- When a new flag is added to `BULK_SUPPORTED` or `REJECTED_IN_BULK` in `create.rs`, the
  expected set grows (derived from those constants), but the extracted set does not (no new
  `conflicting.push` was added). The inequality causes the test to fail, alerting the
  developer to extend the conflict block.

### AC-013 — R2 pin: extractor parses a known-good string and produces exactly 12 members
(traces to BC-3.4.017 EC-3.4.017-14 R2 pin requirement)

At least one dedicated pin test in `src/cli/issue/create.rs` `#[cfg(test)]` block:

- Constructs a short synthetic string containing exactly the 12 current `conflicting.push`
  lines (or supplies `include_str!("create.rs")` as input and asserts `extracted.len() == 12`).
- The 12 expected members are: `"--field"`, `"--summary"`, `"--priority"`, `"--type"`,
  `"--team"`, `"--points"`, `"--no-points"`, `"--parent"`, `"--no-parent"`,
  `"--description"`, `"--description-stdin"`, `"--markdown"`. (`"--label"` is the outer
  `if` guard condition, not a pushed entry.)
- The pin test is the R2 robustness anchor: if the extractor regex/line-scan logic changes
  (e.g., formatting drift in the source), this pin will catch the regression.

### AC-014 — Guard comment at `create.rs:~445` reserves the `conflicting` variable name
(traces to BC-3.4.017 EC-3.4.017-14)

A guard comment is added at the `conflicting` variable declaration site in `create.rs` (at
approximately line 445, the start of the `if !labels.is_empty()` block):

```rust
// NOTE: the variable name 'conflicting' is reserved for this block —
// test_label_conflict_block_lists_every_relevant_flag uses a global scan of
// conflicting.push("--...") in create.rs. If a future cycle introduces a second
// 'conflicting' variable elsewhere in this file, re-scope the meta-test to
// brace-matched extraction.
```

This guard prevents silent drift if a second `conflicting` binding is introduced in an
unrelated part of `create.rs`.

### AC-015 — `--markdown` test uses two SEPARATE stderr assertions (not one concatenated substring)
(traces to BC-3.4.017 EC-3.4.017-14, prd-delta-407.md §4.3)

The `test_label_plus_markdown_rejected_with_exit_64_no_http` test MUST NOT use a single
assertion like `stderr.contains("--label cannot be combined with --markdown")`.

Rationale: the conflict block emits `"--label cannot be combined with --description, --markdown
in the same call. ..."` when both `--description` and `--markdown` are present. The substring
`"--label cannot be combined with --markdown"` does NOT appear verbatim (it is separated by
`"--description, "` in the output). The two required separate checks are:

1. `assert!(stderr.contains("--label cannot be combined with"), ...)` — verifies the right guard fired
2. `assert!(stderr.contains("--markdown"), ...)` — verifies the `--markdown` row cannot be deleted

### AC-016 — `issue_type → --type` clap-rename handled correctly in meta-test expected set
(traces to BC-3.4.017 EC-3.4.017-14, prd-delta-407.md §4.2)

The meta-test expected set for `issue_type` MUST use `"--type"` (the explicit `long = "type"`
clap annotation), NOT `"--issue-type"` (the implicit snake→kebab default for `issue_type`).

The expected set construction must account for the one non-mechanical rename: `issue_type`
field in the clap struct carries `#[arg(long = "type")]`. The other 11 fields (`summary`,
`priority`, `team`, `points`, `no_points`, `parent`, `no_parent`, `description`,
`description_stdin`, `markdown`, `field`) all use the implicit snake→kebab transform (hyphens
replace underscores). Any future field with a `long = "..."` override must be added
explicitly to the expected set in the meta-test; the R2 pin (AC-013) will catch any
enumeration drift.

## Test Deliverables Summary

All test locations and naming:

| # | Test name | Location | Type |
|---|-----------|----------|------|
| 1 | `test_label_plus_priority_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 2 | `test_label_plus_type_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 3 | `test_label_plus_team_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 4 | `test_label_plus_points_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 5 | `test_label_plus_no_points_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 6 | `test_label_plus_parent_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 7 | `test_label_plus_no_parent_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 8 | `test_label_plus_description_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 9 | `test_label_plus_description_stdin_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 10 | `test_label_plus_markdown_rejected_with_exit_64_no_http` | `tests/issue_edit_field.rs` | integration |
| 11 | `test_label_conflict_block_lists_every_relevant_flag` | `src/cli/issue/create.rs` `#[cfg(test)]` | unit (source-text meta-test) |
| 12 | `test_label_conflict_block_extractor_pin_12_members` (or similar R2 pin name) | `src/cli/issue/create.rs` `#[cfg(test)]` | unit (R2 pin) |

**Total: 12 tests** (10 integration regression tests + 1 structural meta-test + 1 R2 pin).

## Files NOT to Touch (Regression Baseline)

| File | Why Unchanged |
|------|--------------|
| `src/cli/issue/create.rs` production logic | No production behavior changes — the conflict block is already correct for all 12 flags |
| `.factory/specs/prd/bc-3-issue-write.md` | BC count unchanged (103/74); EC-3.4.017-14 already added in F2 |
| `scripts/check-spec-counts.sh` | No count changes |
| `tests/issue_edit_echo.rs` | S-398 regression baseline; must stay green |
| `tests/issue_commands.rs` | Pre-echo regressions; must remain green |
| `tests/issue_write_holdouts.rs` | Existing holdout suite; no changes |
| `tests/issue_create_jsm.rs` | JSM path; unrelated |
| `CLAUDE.md` | No new gotchas introduced by this story |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~5 k |
| F2 PRD delta (`prd-delta-407.md`) | ~5 k |
| BC body — BC-3.4.017 section in `bc-3-issue-write.md` (EC-3.4.017-13..14, invariant 2) | ~4 k |
| `src/cli/issue/create.rs` (the `--label` conflict block ~445-492 + `#[cfg(test)]` block + surrounding handle_edit context) | ~15 k |
| `tests/issue_edit_field.rs` (existing 33-test file; new tests appended) | ~18 k |
| `tests/common/` (fixture helpers; test binary launch helpers) | ~4 k |
| Tool outputs (`cargo test`, `cargo clippy`) | ~5 k |
| **Total** | **~56 k** |

Well within a single-agent context window (~200 k). No split required.
LOC delta estimate: +120–150 LOC in `tests/issue_edit_field.rs`; +40–60 LOC in
`src/cli/issue/create.rs` `#[cfg(test)]` block + 3-line guard comment = ~53 LOC.

## Tasks

- [ ] Read `prd-delta-407.md` — capture: 10-flag test list, `--markdown` special case
      (must pair with `--description "text"`), `--description-stdin` no-stdin-pipe confirmation,
      meta-test approach (b) decision, R2 pin requirement, guard comment text
- [ ] Read `bc-3-issue-write.md` §BC-3.4.017 — capture: invariant 2 (partition exhaustiveness +
      conflict-block completeness), EC-3.4.017-13 (precedent test pattern), EC-3.4.017-14
      (meta-test spec including exact guard comment text, extraction strategy, expected set
      construction, `issue_type → --type` rename, R2 pin enumeration)
- [ ] Read `tests/issue_edit_field.rs` in full — understand: existing FIX-F5-001 two regression
      tests (`test_label_plus_field_...` and `test_label_plus_summary_...`) for the EXACT pattern
      to mirror; how `Mock::given(any()).expect(0)` is mounted; how `jr_cmd` is invoked; how exit
      code and stderr are asserted
- [ ] Read `src/cli/issue/create.rs` — locate: the `if !labels.is_empty()` block (~445-492) to
      confirm the exact `conflicting.push` pattern; the `#[cfg(test)]` block at end-of-file to
      confirm insertion point for meta-test and R2 pin; the `--markdown` pre-guard at ~357-363
      (CRITICAL: confirms why `--description "text"` is required alongside `--markdown`);
      the `--description-stdin` conflict check line (~474) vs stdin read line (~882)
- [ ] Read `src/cli/mod.rs` — locate `IssueCommand::Edit` variant and find `issue_type` field with
      `#[arg(long = "type")]` annotation (confirms the one non-mechanical rename for the meta-test)
- [ ] Add 10 positive regression tests to `tests/issue_edit_field.rs` (AC-001 through AC-010):
      - Each test: `let mock = Mock::given(any()).expect(0)` catch-all mounted on the wiremock server
      - Each test: invoke `jr issue edit TEST-1 --label add:x --<flag> [<value>]` via `Command::new(jr_bin)`
      - Each test: assert exit code 64 using `assert_cmd` predicates
      - Each test: assert stderr.contains(`"--label cannot be combined with"`) (separate assertion)
      - Each test: assert stderr.contains(`"--<flag>"`) (separate assertion)
      - `--no-points` / `--no-parent` / `--description-stdin`: no value argument
      - `--markdown` test MUST use `--markdown --description "some text"` — add comment explaining why
      - `--markdown` test: TWO separate stderr assertions (NOT concatenated); see AC-010, AC-015
- [ ] Add `test_label_conflict_block_lists_every_relevant_flag` to `src/cli/issue/create.rs` `#[cfg(test)]` (AC-011, AC-012, AC-016):
      - Read source via `include_str!("create.rs")` (file-relative path since test lives in create.rs)
      - Extract `conflicting.push("--<flag>")` literals via line-by-line scan (collect flagname from each matching line)
      - Build `extracted: BTreeSet<String>`
      - Build `expected: BTreeSet<String>` from `(BULK_SUPPORTED \ {"label"}) ∪ REJECTED_IN_BULK`,
        applying `issue_type → "--type"` explicitly; all others: snake→kebab
      - `assert_eq!(extracted, expected)` with a descriptive panic message
- [ ] Add R2 pin test `test_label_conflict_block_extractor_pin_12_members` (or similar) (AC-013):
      - Supply `include_str!("create.rs")` as input (or a synthetic string with exactly 12 push lines)
      - Assert `extracted.len() == 12`
      - Enumerate the 12 expected members in a comment for human readability
- [ ] Add guard comment at the `let mut conflicting` declaration in `handle_edit` (~line 445) (AC-014):
      - Exact text from EC-3.4.017-14: "NOTE: the variable name 'conflicting' is reserved for this
        block — test_label_conflict_block_lists_every_relevant_flag uses a global scan of
        conflicting.push(\"--...\") in create.rs. If a future cycle introduces a second 'conflicting'
        variable elsewhere in this file, re-scope the meta-test to brace-matched extraction."
- [ ] Run `cargo test --test issue_edit_field` — all 10 new tests pass; existing 33 tests still pass (43 total)
- [ ] Run `cargo test --lib` — meta-test and R2 pin pass; all other unit tests green
- [ ] Run `cargo test` — full suite green; `tests/issue_edit_echo.rs`, `tests/issue_commands.rs`,
      `tests/issue_write_holdouts.rs`, `tests/issue_create_jsm.rs` unchanged (no regressions)
- [ ] Run `cargo clippy -- -D warnings` — zero warnings; no `#[allow]` suppressions
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh` — exit 0 (no count changes)
- [ ] Per-story adversary 3/3 CLEAN before push

## Previous Story Intelligence

This story is the direct continuation of S-396, specifically FIX-F5-001 (delivered as part of
PR #401, the S-396 implementation). Key lessons:

- **From FIX-F5-001 (S-396, PR #401):** The two existing positive regression tests
  (`test_label_plus_field_rejected_with_exit_64_no_http` and
  `test_label_plus_summary_rejected_with_exit_64_no_http`) are the EXACT pattern to mirror.
  Read both tests in full before writing any new test. The `Mock::given(any()).expect(0)` mount
  style and assertion chain are established and stable.

- **`--markdown` pre-guard is CRITICAL:** `--markdown` without `--description` fires a guard
  at `create.rs:357-363` BEFORE the `--label` conflict block. Every prior cycle that overlooked
  this produced a test that passes for the WRONG reason (wrong exit-64 message). Use
  `--markdown --description "some text"`. The test comment is non-optional.

- **`--description-stdin` needs no stdin pipe:** The conflict check fires at line 474
  (`if description_stdin {`), which is BEFORE the stdin read at line 882. No pipe needed.
  Confirmed by F1 delta analysis Q4 (self-answering).

- **Global extraction is safe for NOW:** The meta-test uses a global scan of `conflicting.push`
  in the entire file. This is safe ONLY because the variable name `conflicting` is used
  exclusively in the `--label` block. The guard comment at line ~445 is the mechanical
  enforcement of this assumption. Do not skip it.

- **BTreeSet, NOT HashSet:** The meta-test must use `BTreeSet<String>` for both the extracted
  and expected sets. `HashSet` produces non-deterministic failure diffs across test runs
  (different iteration order on each failure), making debugging significantly harder.
  `test_343_every_edit_field_is_categorized` (the precedent) uses BTreeSet; match that convention.

- **`issue_type → "--type"` is the one non-mechanical rename:** All other fields in
  `BULK_SUPPORTED`/`REJECTED_IN_BULK` use snake→kebab (underscores → hyphens). Only
  `issue_type` has `#[arg(long = "type")]`. Hard-code this mapping in the expected set
  construction. Any future non-mechanical rename will be caught by the R2 pin (AC-013).

## Architecture Compliance Rules

1. **NO production logic changes.** The conflict block in `handle_edit` is already correct for all
   12 flags. Do NOT modify any production code path. The only `create.rs` changes are the
   `#[cfg(test)]` block additions and the guard comment.

2. **Test isolation via `Mock::given(any()).expect(0)`.** Every integration test in this story mounts
   a catch-all mock with `expect(0)`. After the assertion, the wiremock server MUST verify the mock
   with zero calls. This is the FIX-F5-001 pattern — do not weaken it.

3. **Separate assertions for the two stderr substrings.** `"--label cannot be combined with"` and
   `"--<flag>"` are ALWAYS checked independently. Never merge them into a single `contains` call.
   The split assertion proves both components of the error are present even when the conflict block
   lists multiple flags in a single comma-separated message.

4. **`BTreeSet` in the meta-test** (see Previous Story Intelligence).

5. **`include_str!("create.rs")` path is file-relative** when used inside a test that lives in
   `create.rs`. This is correct for `#[cfg(test)]` inline tests. Do not use an absolute path or
   `concat!(env!("CARGO_MANIFEST_DIR"), "/src/cli/issue/create.rs")` — the file-relative form is
   the established convention in this codebase.

6. **No `#[allow]` suppressions.** Zero-warning policy. Refactor if clippy warns.

7. **No count-surface edits.** BC count surfaces (bc-3-issue-write.md frontmatter,
   BC-INDEX.md, CANONICAL-COUNTS.md) are all unchanged. Do NOT edit them.

## Library & Framework Requirements

No new dependencies. All test additions use tools already present as dev-dependencies:

- `wiremock` — existing dev-dependency; use same version as `tests/issue_edit_field.rs`
- `assert_cmd` — existing dev-dependency; same `Command::new(jr_bin)` + predicates pattern
- `std::collections::BTreeSet` — stdlib; no external crate
- No version pins change; no `Cargo.toml` edits required

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `tests/issue_edit_field.rs` | Modify | Add 10 integration tests at end of file; ~+130 LOC |
| `src/cli/issue/create.rs` | Modify | (1) Add guard comment at `let mut conflicting` declaration (~line 445); (2) Add `test_label_conflict_block_lists_every_relevant_flag` + R2 pin in existing `#[cfg(test)]` block at end of file; ~+50 LOC in tests block + 3 LOC comment |
| `.factory/stories/STORY-INDEX.md` | Modify | Append S-407 row to Feature Followup table; update total_stories 46→47 and last_updated |

**Files NOT to create:** No new source files. No new spec files. No new VP documents.

## Branch / PR Plan

- Branch: `feat/issue-407-label-conflict-block-coverage`
- Target: `develop`
- Commit style: `test(S-407): add 10 label-conflict regression tests + structural meta-test (closes #407)`
- PR closes #407
- CHANGELOG entry: not required (test-only; no user-visible behavior change)

**Why `breaking_change: false`:** Test-only additions. No production code behavior changes.
No previously-succeeding invocation changes its exit code, stdout shape, or stderr content.

## Per-Story Delivery Notes

- S-396 must be fully merged to `develop` before starting F4 (it created `tests/issue_edit_field.rs`
  and the conflict block that this story adds tests for). Confirm S-396 status is `completed` and
  the file exists at `tests/issue_edit_field.rs` before writing any new tests.
- BC count surfaces are unchanged — do NOT run `scripts/check-spec-counts.sh` with expectation
  of any delta. Run it to confirm zero drift (exit 0 is the expected outcome).
- Per-story adversary 3/3 CLEAN required before push.
- The meta-test reads live source via `include_str!("create.rs")`. If `create.rs` is modified
  between the time this story is dispatched and the time F4 runs, re-read the file to confirm
  the conflict block line numbers and variable name are unchanged.
