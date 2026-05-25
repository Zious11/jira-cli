---
document_type: f1-delta-analysis
phase: phase-f1-delta-analysis
producer: architect
issue: 407
status: draft
created: 2026-05-25
project: jira-cli
mode: BROWNFIELD
intent: test-hardening
bundled_fix: false
feature_type: test-only (with optional minor production refactor)
trivial_scope: true
scope: small
regression_risk: low
severity: medium
inputs:
  - "tests/issue_edit_field.rs"
  - "src/cli/issue/create.rs"
  - ".factory/research/follow-up-validation-396.md"
  - ".factory/specs/prd/bc-3-issue-write.md"
  - ".factory/phase-f1-delta-analysis/issue-396/delta-analysis.md"
---

# F1 Delta Analysis — Issue #407

## Feature

- **Name:** Test-hardening — `--label` conflict block: 10 missing positive regression tests + structural meta-test
- **Issue link:** https://github.com/Zious11/jira-cli/issues/407
- **Research source:** `.factory/research/follow-up-validation-396.md` (Items 1 + 2, CONFIRMED MEDIUM)
- **Closest precedent:** FIX-F5-001 (PR #406) — added the first 2/12 positive regression tests using
  the `Mock::given(any()).expect(0)` catch-all pattern

---

## Problem Statement

The `--label` mutual-exclusion block in `handle_edit` (`src/cli/issue/create.rs:445-492`)
enumerates 12 conflicting flags. FIX-F5-001 (PR #406) added the first two positive regression
tests (`test_label_plus_field_rejected_with_exit_64_no_http`,
`test_label_plus_summary_rejected_with_exit_64_no_http`). The remaining 10 entries have zero
positive regression coverage:

```
--priority, --type, --team, --points, --no-points,
--parent, --no-parent, --description, --description-stdin, --markdown
```

Because no test fires for these entries, any `conflicting.push("--xxx")` call for the 10
untested flags can be deleted (or its condition flipped) and the entire test suite passes.
`cargo-mutants` would flag these as survival mutants on every diff that touches `create.rs`.

Additionally, no test enforces the structural invariant that the conflict block's enumerated
set stays in sync with the flag surface. The existing
`test_343_every_edit_field_is_categorized` (line 1524) only enforces the
SELECTORS / BULK_SUPPORTED / REJECTED_IN_BULK partition. It never reads the `--label` conflict
block and cannot catch a deletion there.

---

## Scope

### In scope

- Add 10 positive regression tests in `tests/issue_edit_field.rs`, one per untested flag in
  the conflict block: `--priority`, `--type`, `--team`, `--points`, `--no-points`, `--parent`,
  `--no-parent`, `--description`, `--description-stdin`, `--markdown` (see Interaction Analysis
  section for the `--markdown` special case).
- Add one structural meta-test (see Q1 decision) that enforces the conflict block's enumerated
  flags match the expected set derived from BULK_SUPPORTED and REJECTED_IN_BULK.

### Out of scope

- Changing any production code (unless approach (ii)/(iii) is chosen for Q1 — see below).
- Adding new behavioral contracts (this is regression coverage for existing BC-3.4.017 invariant 2,
  not a new contract).
- The 10 untested entries already have correct production behavior; this is purely a coverage gap.
- Fixing the stale line-anchor citations in `bc-3-issue-write.md` (tracked separately by Item 4
  of the research validation).
- Fixing `test_38` inline wire-serialization duplication (tracked separately by Item 5).

---

## Interaction Analysis: Flag-Specific Test Concerns

### `--no-points` and `--no-parent`

Boolean flags (clap presence flags). Invoke as `--no-points` / `--no-parent` with no value
argument. The conflict block checks these as `if no_points {` / `if no_parent {`, so they trip
correctly. No special test handling needed beyond the standard pattern.

### `--description-stdin`

A boolean flag that signals "read description from stdin." In integration tests using
`Command::new(jr_bin)`, stdin is not piped by default (the process inherits the test runner's
stdin, which is not a TTY). The conflict block fires at line 474 (`if description_stdin {`) which
is BEFORE the stdin read at line 149 (`if description_stdin { ... read ... }`). Therefore the
test does NOT need to pipe actual stdin content — the conflict guard exits 64 before any stdin
read occurs. Standard pattern applies.

### `--markdown` — CRITICAL INTERACTION

`--markdown` alone (without `--description` or `--description-stdin`) fires an EARLIER guard at
line 357-363:

```rust
if markdown && description.is_none() && !description_stdin {
    return Err(JrError::UserError(
        "--markdown requires --description or --description-stdin to take effect. ...".into(),
    ).into());
}
```

This guard runs BEFORE the `--label` conflict block (which starts at line 445). Therefore:

- `jr issue edit TEST-1 --label add:x --markdown` exits 64 at the `--markdown` guard, NOT the
  conflict block. The stderr would say `"--markdown requires --description"`, NOT
  `"--label cannot be combined with"`.
- To reach the `--label` conflict block for `--markdown`, the test MUST pair `--markdown` with a
  description flag: `--label add:x --markdown --description "some text"` OR
  `--label add:x --markdown --description-stdin`.

Two sub-cases arise:

**Option A (simpler):** `--label add:x --markdown --description "text"` — this reliably hits the
`--label` conflict block because `description.is_some()` satisfies the `--markdown` guard AND
the conflict block fires for `--description` (not `--markdown`). The `--markdown` entry would
not be the one listed in stderr unless `--description` is NOT also present and the test is
specifically probing `--markdown`'s own row.

**Option B (true probe for `--markdown` row):** Use `--label add:x --markdown --description-stdin`
with no stdin piped — if `description_stdin = true` satisfies the `--markdown` guard (yes, it
does: `description.is_none() && !description_stdin` becomes false), AND `description_stdin` trips
its OWN row in the conflict block AND `--markdown` also trips its row. Both `--description-stdin`
and `--markdown` would appear in the stderr output for this invocation.

**Recommended approach:** For the `--markdown` test, use `--label add:x --markdown --description
"some text"`. This:
1. Satisfies the pre-guard (description is Some).
2. Enters the conflict block.
3. Both `--description` and `--markdown` push to the conflict vector.
4. The test asserts stderr contains `"--markdown"` — the primary objective.

This means the `--markdown` test is also implicitly a dual-flag test (`--description` + `--markdown`).
That is acceptable because the test's purpose is to prove the `--markdown` row in the conflict
block cannot be deleted without breaking a test.

Alternatively, use `--label add:x --markdown --description-stdin` (without piping stdin), which
also satisfies the pre-guard because `description_stdin=true` makes `!description_stdin = false`.
Then only `--description-stdin` and `--markdown` appear in the conflict output (both rows trip).

The implementation author should document which choice was made in the test comment.

---

## Affected Files

### Modified files

| Path | What changes |
|------|-------------|
| `tests/issue_edit_field.rs` | Add 10 positive regression tests (one per untested conflict-block entry) + 1 structural meta-test (see Q1). Est. +100–200 LOC. |
| `src/cli/issue/create.rs` | ONLY if Q1 approach (ii)/(iii) is chosen — extract the conflict-block flag list to a `const &[&str]` consumed by both the runtime block and the meta-test. If approach (a) or (b) is chosen, NO production code changes. |

### No changes required (all scenarios)

- `src/cli/mod.rs` — no flag additions
- `src/api/jira/` — no API changes
- `src/types/jira/` — no type changes
- `CLAUDE.md` — no new gotchas (the conflict block is already documented)
- `.factory/specs/prd/bc-3-issue-write.md` — no new BCs (this is existing BC-3.4.017 invariant 2)
- `scripts/check-spec-counts.sh` — no count changes

---

## Q1: Meta-Test Approach Decision

Three candidates. This is the primary open question for the human gate.

### Approach (a): Extend `test_343_every_edit_field_is_categorized`

Add a fourth assertion block at the bottom of the existing test that:
1. Parses `create.rs` source text (via `include_str!("create.rs")` inside the unit test module)
   to extract the `conflicting.push(...)` lines from within the `if !labels.is_empty() { ... }`
   block.
2. Builds the expected set: `(BULK_SUPPORTED \ {label}) ∪ REJECTED_IN_BULK` mapped from
   snake_case to kebab-case flag names.
3. Asserts the extracted set equals the expected set (same BTreeSet pattern as the existing test).

Pros: all partition + conflict-block invariants in one test; one place to update when flags change.
Cons: the test grows to ~80+ assertions across two concerns; adding a new source parser for the
conflict block increases test maintenance surface. The existing extractor targets `mod.rs`; the
new one would target `create.rs` — different file, different pattern.

### Approach (b): Add a new dedicated meta-test

Add `test_label_conflict_block_lists_every_relevant_flag` as a separate test function in
`src/cli/issue/create.rs` (in the same `#[cfg(test)]` block). It parses `create.rs` via
`include_str!` (the file path for this would be `"create.rs"` relative to the test module,
since the test lives IN `create.rs`), extracts `conflicting.push(...)` calls from the
`if !labels.is_empty()` block, and asserts equality against the expected set.

Pros: single-responsibility; the test name and doc comment clearly express "conflict block must
stay complete." Easier to read in isolation. Does not grow `test_343`.
Cons: one more test function; the expected set is defined in a second location (also in Q1(a)
actually, since the expected set references `BULK_SUPPORTED` / `REJECTED_IN_BULK` names which
live in `test_343`).

### Approach (ii)/(iii): Extract conflict list to a `const` slice

Extract the 12 conflict-block flags into a `const LABEL_CONFLICT_FLAGS: &[(&str, bool)]` or
similar, consumed by both the runtime block (iterate over it) and a simple meta-test (assert
the const slice has exactly the expected length/members).

Pros: DRY — the runtime code and the test share a single source of truth; no source-text parsing
needed; the meta-test is 5-10 lines of assertion against a Rust slice. Cannot drift without a
compile error.
Cons: touches production code (`create.rs`), widening scope. The runtime loop must reconstruct
the conditional checks (`if summary.is_some()` etc.), which are heterogeneous — some are
`is_some()` on `Option`, some are `bool` flags, some are `!vec.is_empty()`. A uniform `const`
over flag names cannot encode these conditions without boxing lambdas or using a different
abstraction. A pure list of names requires the loop to use `match name { ... }` — a different
refactor shape that may itself require careful review.

**F1 recommendation:** Approach (b) — dedicated meta-test, no production code changes.

Rationale: The source-text parsing pattern is already proven and robust in this codebase
(the `extract_edit_field_names` extractor has R2 pin tests for formatting drift). Adding a
parallel extractor scoped to the `if !labels.is_empty()` block is the same shape of work,
lower risk than a production refactor. Approach (b) preserves single-responsibility while
keeping production code untouched. Approach (ii)/(iii) is cleaner architecturally but the
heterogeneous condition types make the uniform-iteration refactor non-trivial; that trade-off
is not warranted for a pure test-hardening cycle.

---

## Open Questions for Human Gate

| # | Question | Recommendation | Blocking? |
|---|----------|---------------|-----------|
| Q1 | Meta-test approach: (a) extend `test_343`, (b) dedicated meta-test, or (ii)/(iii) extract to `const`? | (b) dedicated meta-test, no production code changes | YES — determines which files are modified |
| Q2 | Test naming: confirm `test_label_plus_<flag>_rejected_with_exit_64_no_http` convention for the 10 new tests? | YES — mirrors the two existing FIX-F5-001 tests exactly | No |
| Q3 | For `--markdown` test: (A) use `--markdown --description "text"` so description is present (hits both `--description` and `--markdown` rows), OR (B) use `--markdown --description-stdin` (hits both `--description-stdin` and `--markdown` rows)? | Either works; recommend (A) since it avoids any stdin-plumbing question | No |
| Q4 | For `--description-stdin` test: confirm stdin does NOT need to be piped (conflict guard fires at line 474, before the stdin read at line 149)? | CONFIRMED by code reading — no stdin pipe needed | No (self-answering) |

---

## Estimation

| Dimension | Value |
|-----------|-------|
| Story points | 1 (test-only; 10 near-copy tests + 1 meta-test) |
| Module criticality | LOW — test file only; no production paths change |
| Estimated LOC | +120–200 in `tests/issue_edit_field.rs`; +0–30 in `src/cli/issue/create.rs` if approach (ii)/(iii) chosen |
| Strategy | Plain test-add (TDD not applicable; the production behavior already exists and is correct) |

---

## Regression Risk Assessment

**Risk: None on production behavior.** All 10 new tests exercise pre-existing guards. The production
conflict block already fires correctly for all 12 flags; the tests merely add detection so that a
future accidental deletion is caught.

**Risk: Meta-test implementation.** The structural meta-test uses source-text parsing
(`include_str!` + line-scanning), which is the same approach as the existing
`extract_edit_field_names` extractor. That extractor has dedicated R2 pin tests for
formatting-tolerance; the new extractor should follow the same pattern and have at least one
R2 pin test for the `conflicting.push` detection heuristic.

**Risk: `--markdown` test requires paired `--description`.** If the test is written as
`--label add:x --markdown` alone (without `--description`), it will exit 64 with the wrong
error message ("--markdown requires --description") rather than the conflict-block message.
This must be documented clearly in the test comment, and the test must assert on
`"--label cannot be combined with"` AND `"--markdown"` being present in stderr — confirming
the RIGHT guard fired.

---

## New BCs / VPs Required

None. This PR adds regression coverage for the existing BC-3.4.017 invariant 2 (conflict
block completeness). No new behavioral contracts exist; the production behavior is already
correct and already contracted. `bc-3-issue-write.md` count fields do not change.

---

## Summary for Human Gate

This is a **small, test-hardening-only** change. The production conflict block in
`handle_edit` already works correctly for all 12 flags; the gap is purely in test coverage
and structural enforcement. FIX-F5-001 brought coverage from 0/12 to 2/12; this cycle
completes it to 12/12 and adds a meta-test to prevent future regression.

The critical decision is **Q1**: which meta-test approach. The recommendation is **(b)** — a
new dedicated test function using the same `include_str!` + source-text parsing pattern
already proven in `test_343`, with no production code changes.

The one non-obvious implementation concern is **`--markdown`**: it cannot be tested with
`--label --markdown` alone because a prior guard fires first (line 357). The test must use
`--label --markdown --description "text"`, and the test comment must explain why. This is
documented in the Interaction Analysis section above.

Q2 (naming), Q3 (`--markdown` pairing choice), and Q4 (`--description-stdin` stdin-pipe question)
are all low-stakes and self-answering; they are listed for completeness but should not block the gate.
