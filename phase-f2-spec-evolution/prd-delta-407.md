---
document_type: prd-delta
issue: "#407"
title: "Test-hardening — --label conflict block: 10 missing positive regression tests + structural meta-test"
date: "2026-05-25"
phase: F2
spec_version_bump: "none — EC addition only (EC-3.4.017-14); BC counts unchanged (583 total; bc-3 103/74)"
new_bcs: []
modified_bcs:
  - BC-3.4.017  # EC-3.4.017-14 added; invariant 2 updated to reference EC-3.4.017-14
inputs:
  - .factory/phase-f1-delta-analysis/issue-407/delta-analysis.md
  - .factory/research/follow-up-validation-396.md
  - .factory/specs/prd/bc-3-issue-write.md
---

# PRD Delta — Issue #407: `--label` Conflict Block Test-Hardening

## 1. Summary of Change

Issue #407 is a **test-hardening-only** cycle. No production code changes. No new
behavioral contracts. No new verification properties.

The `--label` mutual-exclusion block in `src/cli/issue/create.rs:445-492` enumerates
12 conflicting flags. FIX-F5-001 (PR #406) added the first two positive regression
tests (`test_label_plus_field_rejected_with_exit_64_no_http`,
`test_label_plus_summary_rejected_with_exit_64_no_http`). Issue #407 completes coverage
to 12/12 and adds a structural meta-test to enforce completeness going forward.

**Coverage gap (pre-#407)**: 10 of 12 entries in the conflict block had zero positive
regression coverage:
```
--priority, --type, --team, --points, --no-points,
--parent, --no-parent, --description, --description-stdin, --markdown
```

Any `conflicting.push("--xxx")` call for these 10 flags could be deleted from the
source and the entire test suite would pass. `cargo-mutants` would tag each deletion
as a surviving mutant on every diff touching `create.rs`.

**No new BCs are needed.** The production behavior is already correct and already
contracted under BC-3.4.017 invariant 2. This cycle adds coverage to enforce that
existing invariant mechanically.

---

## 2. Why No New BCs

The F1 delta analysis (`delta-analysis.md §New BCs / VPs Required`) confirmed:

> "None. This PR adds regression coverage for the existing BC-3.4.017 invariant 2
> (conflict block completeness). No new behavioral contracts exist; the production
> behavior is already correct and already contracted."

The `--label` mutual-exclusion block is already specified in BC-3.4.017 as invariant 2
(partition exhaustiveness). EC-3.4.017-13 (FIX-F5-001) established the pattern of
documenting the `--label` + `--field` rejection. EC-3.4.017-14 (this cycle) extends
that documentation to cover the mechanical enforcement mechanism.

BC counts are unchanged: **583 total, bc-3 at 103/74**.

---

## 3. Why No New VPs

No new verification properties are created. The 10 new positive regression tests
and the structural meta-test are implementation-level test artefacts that verify
the existing VP-396-005 coverage perimeter. They do not define new observable
behaviors — they confirm that behaviors already contracted under BC-3.4.017
remain non-deletable.

The structural meta-test (`test_label_conflict_block_lists_every_relevant_flag`)
functions as an **inline verification artifact** for BC-3.4.017 invariant 2. It is
mechanically equivalent to a VP in scope (it enforces completeness of the invariant),
but it lives in the source file and executes as a unit test rather than as a
standalone VP. This is the appropriate resolution for a compile/test-time structural
invariant that requires source-text access — VPs are reserved for observable
behavioral contracts, not structural completeness of source constructs.

---

## 4. Test-Hardening Scope

### 4.1 Ten Positive Regression Tests

**Location**: `tests/issue_edit_field.rs`  
**Naming convention**: `test_label_plus_<flag>_rejected_with_exit_64_no_http`  
(mirrors the two FIX-F5-001 tests exactly)

**Pattern** (per FIX-F5-001 precedent):
- `Mock::given(any()).expect(0)` catch-all — zero HTTP calls permitted
- Invoke `jr issue edit TEST-1 --label add:x --<flag> [<value>]`
- Assert: exit code 64
- Assert: stderr contains `"--label cannot be combined with"` (separate assertion)
- Assert: stderr contains `"--<flag>"` (separate assertion — NOT as one concatenated substring; the conflict block joins all conflicting flags into a comma-separated message so `"--label cannot be combined with --<flag>"` may not appear verbatim when multiple flags conflict simultaneously)

**Naming note**: test names use snake_case substitution for kebab-case flags
(e.g., `--no-points` → `test_label_plus_no_points_...`; Rust identifiers cannot contain
hyphens).

**Ten tests, one per flag**:

| Test name | Flag | Invocation notes |
|-----------|------|-----------------|
| `test_label_plus_priority_rejected_with_exit_64_no_http` | `--priority` | Standard pattern |
| `test_label_plus_type_rejected_with_exit_64_no_http` | `--type` | Standard pattern |
| `test_label_plus_team_rejected_with_exit_64_no_http` | `--team` | Standard pattern |
| `test_label_plus_points_rejected_with_exit_64_no_http` | `--points` | Standard pattern |
| `test_label_plus_no_points_rejected_with_exit_64_no_http` | `--no-points` | Boolean flag, no value argument |
| `test_label_plus_parent_rejected_with_exit_64_no_http` | `--parent` | Standard pattern |
| `test_label_plus_no_parent_rejected_with_exit_64_no_http` | `--no-parent` | Boolean flag, no value argument |
| `test_label_plus_description_rejected_with_exit_64_no_http` | `--description` | Standard pattern |
| `test_label_plus_description_stdin_rejected_with_exit_64_no_http` | `--description-stdin` | Boolean flag; conflict guard fires at line 474 BEFORE any stdin read — no stdin pipe needed |
| `test_label_plus_markdown_rejected_with_exit_64_no_http` | `--markdown` | MUST pair with `--description "text"` (see §4.3) |

### 4.2 Structural Meta-Test (Approach b — Dedicated Function)

**Location**: `src/cli/issue/create.rs` `#[cfg(test)]` block  
**Function name**: `test_label_conflict_block_lists_every_relevant_flag`

**Human gate confirmed approach**: (b) — dedicated meta-test, no production code changes.

**Mechanism**:
1. Read the source file via `include_str!("create.rs")` (path relative to the test
   module, which lives in `create.rs` itself).
2. Extract every `conflicting.push("--<flag>")` literal using **global file scan** (not
   brace-matched extraction). This is safe because `conflicting` is used exclusively
   within the `if !labels.is_empty()` block in `handle_edit`; if a future cycle
   introduces a second `conflicting` variable anywhere in `create.rs`, the meta-test
   must be re-scoped. A guard comment MUST be added at the conflict-block declaration
   site in `create.rs` reserving the variable name for this block.
3. Build the expected set using `BTreeSet<String>` (NOT `HashSet` — deterministic
   failure diffs across runs, same as `test_343_every_edit_field_is_categorized`).
   For each field, the kebab-case CLI flag name is the explicit `long = "<literal>"`
   value when present, otherwise the field name with underscores replaced by hyphens
   (clap's implicit default). Of the 12 fields currently in scope: `issue_type`
   carries `#[arg(long = "type")]` and maps to `--type` (NOT `--issue-type`); the
   other 11 (`summary`, `priority`, `team`, `points`, `no_points`, `parent`,
   `no_parent`, `description`, `description_stdin`, `markdown`, `field`) use the
   implicit snake→kebab transform. Any future field added to
   `BULK_SUPPORTED`/`REJECTED_IN_BULK` with a non-mechanical `long = "..."` rename
   will be caught by the R2 pin's 12-flag enumeration — the extractor side and the
   expected side must be reconciled together.
4. Assert extracted `BTreeSet<String>` equals expected `BTreeSet<String>`.

**Why this is durable**: A regression that deletes any `conflicting.push` line OR adds
a new Edit field to `BULK_SUPPORTED`/`REJECTED_IN_BULK` without extending the conflict
block will fail this test at `cargo test` time. The meta-test reads the live source, not
a hard-coded list.

**R2 pin requirement**: Include at least one pin test asserting the extractor correctly
parses a known-good input string (e.g., assert the extracted set has exactly 12 members
for the current block: `--field`, `--summary`, `--priority`, `--type`, `--team`,
`--points`, `--no-points`, `--parent`, `--no-parent`, `--description`,
`--description-stdin`, `--markdown`. `--label` is the outer `if` guard condition, not
a pushed entry).

### 4.3 `--markdown` Test Special Case

`--markdown` alone (without `--description` or `--description-stdin`) triggers an
earlier guard at `src/cli/issue/create.rs:357-363`:
```rust
if markdown && description.is_none() && !description_stdin {
    return Err(JrError::UserError("--markdown requires --description ..."));
}
```
This fires BEFORE the `--label` conflict block (line 445+). To reach the conflict
block with the `--markdown` row, the test MUST pair `--markdown` with a description
flag.

**Chosen approach (F1 Q3 recommendation A)**: Use
`--label add:x --markdown --description "some text"`.

This:
1. Satisfies the pre-guard (`description.is_some()` → the early guard does NOT fire).
2. Enters the conflict block.
3. Both `--description` and `--markdown` push to the conflict vector.
4. The resulting stderr is: `"--label cannot be combined with --description, --markdown
   in the same call. ..."` — note that `--description` precedes `--markdown` in the
   joined output.

**Critical assertion pattern**: assert `stderr.contains("--markdown")` AND
`stderr.contains("--label cannot be combined with")` as TWO SEPARATE checks. Do NOT
assert `stderr.contains("--label cannot be combined with --markdown")` — that
concatenated substring does NOT appear verbatim when `--description` is listed before
`--markdown` in the joined conflict output.

The test comment MUST document why `--description` is required alongside `--markdown`.

### 4.4 `--description-stdin` Test Confirmation

The conflict guard at `create.rs:474` (`if description_stdin {`) fires BEFORE the
stdin read at line 149 (`if description_stdin { ... read ... }`). Therefore the test
does NOT need to pipe actual stdin content — the conflict guard exits 64 before any
stdin I/O occurs. Standard pattern applies (F1 Q4, self-answering).

---

## 5. Spec Amendment — EC-3.4.017-14 (New Edge Case)

A new edge case EC-3.4.017-14 is added to BC-3.4.017 in
`.factory/specs/prd/bc-3-issue-write.md` immediately after EC-3.4.017-13:

> **EC-3.4.017-14**: The `--label` conflict block at
> `src/cli/issue/create.rs::handle_edit::if !labels.is_empty()` is mechanically
> enforced complete by `test_label_conflict_block_lists_every_relevant_flag`
> (in `create.rs::tests`). The meta-test parses the conflict-block source via
> `include_str!("create.rs")`, extracts every `conflicting.push("--<flag>")` literal,
> and asserts the extracted set equals `(BULK_SUPPORTED \ {"label"}) ∪ REJECTED_IN_BULK`
> (mapped from destructured field names to kebab-case CLI flag names). A regression
> that drops any `conflicting.push` line OR adds a new Edit field to
> `BULK_SUPPORTED`/`REJECTED_IN_BULK` without extending the conflict block fails this
> meta-test at `cargo test` time. Co-author with the 10 positive regression tests in
> `tests/issue_edit_field.rs` (`test_label_plus_<flag>_rejected_with_exit_64_no_http`
> for each of: `priority`, `type`, `team`, `points`, `no-points`, `parent`, `no-parent`,
> `description`, `description-stdin`, `markdown`). [Issue #407]

BC-3.4.017 invariant 2 is also updated to cross-reference EC-3.4.017-14 as the
mechanical enforcement witness.

---

## 6. Mechanical Witness for BC-3.4.017 Invariant 2

The structural meta-test (`test_label_conflict_block_lists_every_relevant_flag`)
functions as the inline verification artifact for BC-3.4.017 invariant 2:

> Invariant 2: "The `REJECTED_IN_BULK` set partition test [...] must be updated to
> include `--field`. This ensures the partition is exhaustive."

The meta-test extends this invariant's enforcement surface from "partition is
exhaustive for `--field`" to "conflict block lists every flag from the partition."
It is not a standalone VP because it operates on source-text structure rather than
observable runtime behavior. It functions as a mutation-resistant compile/test-time
guard that makes invariant 2 self-enforcing.

No `verification-delta-407.md` is needed. The meta-test is documented here and
in EC-3.4.017-14 as the mechanical witness for the existing invariant.

---

## 7. Count Surfaces Updated

Adding an EC to an existing BC does NOT change any BC count. All count surfaces
are unchanged:

| Surface | Before | After |
|---------|--------|-------|
| bc-3-issue-write.md `total_bcs` | 103 | 103 (unchanged) |
| bc-3-issue-write.md `definitional_count` | 74 | 74 (unchanged) |
| BC-INDEX.md `total_bcs` | 583 | 583 (unchanged) |
| CANONICAL-COUNTS.md Sum | 583 | 583 (unchanged) |

**Non-count surfaces updated (4 surfaces)**:

1. **bc-3-issue-write.md `last_updated` frontmatter** (line 6): `2026-05-22 → 2026-05-25`
2. **bc-3-issue-write.md `_Last updated_` prose** (line ~2151): advanced to
   `2026-05-25 (issue #407 F2): +EC-3.4.017-14 — mechanical enforcement meta-test
   for BC-3.4.017 invariant 2; invariant 2 cross-reference added.`
3. **BC-INDEX.md `last_updated` frontmatter** (line 5): `2026-05-22 → 2026-05-25`
4. **CANONICAL-COUNTS.md `last_verified`** frontmatter: advanced to
   `2026-05-25 (F2 delta issue #407; EC-3.4.017-14 added to BC-3.4.017; BC counts unchanged)`

---

## 8. Changelog Entry

**v0.7 / issue #407 (2026-05-25):**
- Test-hardening: added 10 positive regression tests for the `--label` mutual-exclusion
  block in `handle_edit` (one per previously-uncovered conflict-block entry: `--priority`,
  `--type`, `--team`, `--points`, `--no-points`, `--parent`, `--no-parent`,
  `--description`, `--description-stdin`, `--markdown`). Uses `Mock::given(any()).expect(0)`
  catch-all to enforce zero HTTP calls, exit 64, and `"--label cannot be combined with"`
  stderr on each combination.
- Test-hardening: added `test_label_conflict_block_lists_every_relevant_flag` structural
  meta-test in `create.rs::tests`. Parses the live source via `include_str!`, extracts
  every `conflicting.push("--<flag>")` literal, and asserts the extracted set equals
  `(BULK_SUPPORTED \ {"label"}) ∪ REJECTED_IN_BULK`. Prevents silent mutation escape
  for any `conflicting.push` deletion or new-flag addition.
- Spec: EC-3.4.017-14 added to BC-3.4.017 documenting the meta-test as the mechanical
  enforcement witness for invariant 2 (conflict block completeness).
