---
document_type: adversarial-review
issue: "#407"
pass: 1
date: "2026-05-25"
phase: F2
verdict: NOT-CLEAN → CLEAN (after fixes applied in this pass)
findings_count: 7
findings_resolved: 7
findings_deferred: 0
guard_check_spec_counts: exit 0
guard_check_bc_cumulative_counts: exit 0
---

# Adversarial Review — Issue #407 F2, Pass 1

## Summary

Pass 1 returned NOT-CLEAN: 2 HIGH + 4 MEDIUM + 1 LOW. All 7 findings addressed
in this pass. One process-gap (F-L-003) recorded for the cycle-closing checklist
with no code action required.

All edits confined to:
- `.factory/specs/prd/bc-3-issue-write.md` (EC-3.4.017-14 body; invariant 2; frontmatter `last_updated`)
- `.factory/specs/prd/BC-INDEX.md` (frontmatter `last_updated`)
- `.factory/phase-f2-spec-evolution/prd-delta-407.md` (§4.1, §4.2, §4.3, §7)

BC counts unchanged: **583 total, bc-3 103/74**.

---

## Findings and Resolutions

### F-H-001 (HIGH) — `issue_type → --type` clap exception missing from EC-3.4.017-14

**File**: `bc-3-issue-write.md` EC-3.4.017-14 (line ~1545-1546)

**Finding**: "mapped from destructured field names to kebab-case CLI flag names" was
misleading. `issue_type` carries `#[arg(long = "type")]` — a mechanical snake→kebab
transform would produce `--issue-type` (wrong). An implementer following the literal
rule would produce a meta-test that fails on first run.

**Resolution**: EC-3.4.017-14 rewritten to:
1. State explicitly: "Flag names are derived from the `#[arg(long = "...")]` attribute
   on each field, NOT by mechanical snake→kebab transform."
2. Call out the critical exception: "`issue_type` carries `#[arg(long = "type")]` — it
   maps to `--type`, NOT `--issue-type`. An implementer who applies the mechanical
   transform rule produces `--issue-type` and the meta-test fails."
3. Recommend using the clap `long` attribute as the authoritative source for all
   kebab-case names.

`prd-delta-407.md §4.2` step 3 updated with the same callout.

---

### F-H-002 (HIGH) — `last_updated` frontmatter stayed at 2026-05-22 in two files

**Files**: `bc-3-issue-write.md` line 6; `BC-INDEX.md` line 5

**Finding**: The `_Last updated_` prose footer was advanced to 2026-05-25 but the
`last_updated:` frontmatter fields in both files remained at `2026-05-22`. Two date
surfaces in disagreement within the same files.

**Resolution**:
- `bc-3-issue-write.md` frontmatter `last_updated: 2026-05-22` → `2026-05-25`
- `BC-INDEX.md` frontmatter `last_updated: 2026-05-22` → `2026-05-25`
- `prd-delta-407.md §7` expanded from 2 non-count surfaces to 4, explicitly listing
  both frontmatter fields as touched surfaces.

---

### F-M-001 (MEDIUM) — snake_case substitution for kebab-case flag names not stated

**File**: `prd-delta-407.md §4.1`

**Finding**: The test naming convention uses `--no-points` → `test_label_plus_no_points_...`
(snake_case substitution) without explaining why. A casual reader could infer
`test_label_plus_no-points_...` (invalid Rust identifier).

**Resolution**: Added a naming note immediately after the pattern block in §4.1:
"Test names use snake_case substitution for kebab-case flags (e.g., `--no-points` →
`test_label_plus_no_points_...`; Rust identifiers cannot contain hyphens)."

---

### F-M-002 (MEDIUM) — EC-3.4.017-14 doesn't specify `BTreeSet<String>` container type

**File**: `bc-3-issue-write.md` EC-3.4.017-14

**Finding**: EC said "asserts the extracted set equals (...)" without specifying the
container type. `prd-delta-407.md §4.2` mentioned `BTreeSet` but the BC body is the
canonical source. `HashSet` iteration order is hash-seed-dependent; test failure diffs
would be unstable.

**Resolution**: EC-3.4.017-14 updated to state: "Use `BTreeSet<String>` for both sides
(mirrors `test_343` — deterministic failure diffs across runs)." The requirement
`BTreeSet<String>` now appears explicitly in both the BC body and `prd-delta-407.md §4.2`.

---

### F-M-003 (MEDIUM) — invariant 2 cited stale `create.rs:1435+` line anchor

**File**: `bc-3-issue-write.md` BC-3.4.017 invariant 2 (~line 1469)

**Finding**: Invariant 2 cited the partition meta-test at `create.rs:1435+` (pre-existing
drift). Actual location of `test_343_every_edit_field_is_categorized` is line 1523.
The #407 pass explicitly touched invariant 2 (adding the EC-3.4.017-14 cross-reference).
Partial-fix discipline: sibling stale references in a touched region are corrected in
the same pass.

**Resolution**: Removed the stale line-anchor `create.rs:1435+` entirely. Replaced with
the durable test name `test_343_every_edit_field_is_categorized`. Updated text now reads:
"The `REJECTED_IN_BULK` set partition test (the compile-time assertion in
`test_343_every_edit_field_is_categorized` that partitions flags into `SELECTORS`,
`BULK_SUPPORTED`, and `REJECTED_IN_BULK`) must be updated to include `--field`."

---

### F-M-004 (MEDIUM) — extraction strategy not specified (global vs. brace-matched)

**File**: `bc-3-issue-write.md` EC-3.4.017-14; `prd-delta-407.md §4.2 step 2`

**Finding**: The spec said "extracts every `conflicting.push(...)` literal from within
the `if !labels.is_empty() { ... }` block" without specifying whether this means global
file scan or brace-matched extraction. Two options with different maintenance implications.

**Resolution**: Endorsed global extraction (option a, simpler, appropriate for a 1-point
task) in both EC-3.4.017-14 and `prd-delta-407.md §4.2`. Added:
- Explicit endorsement: "global extraction of `conflicting.push("--...")` literals from
  the entire file"
- Safety rationale: "safe because `conflicting` is used exclusively within the
  `if !labels.is_empty()` block"
- Future-drift guard: "if a future cycle introduces a second `conflicting` variable
  anywhere in `create.rs`, the meta-test must be re-scoped to brace-matched extraction"
- Required guard comment: a comment MUST be added at the conflict-block declaration site
  in `create.rs` reserving the variable name for this block
- R2 pin specification: at least one pin test asserting the extractor parses a
  known-good input string and extracts exactly 12 members (`--field`, `--summary`,
  `--priority`, `--type`, `--team`, `--points`, `--no-points`, `--parent`,
  `--no-parent`, `--description`, `--description-stdin`, `--markdown`)

---

### F-L-001 (LOW) — `--markdown` test assertion would fail as concatenated substring

**File**: `bc-3-issue-write.md` EC-3.4.017-14; `prd-delta-407.md §4.3`

**Finding**: When `--markdown --description "text"` is used with `--label`, BOTH
`--description` and `--markdown` push to the conflict vector. The resulting stderr is
`"--label cannot be combined with --description, --markdown in the same call. ..."`.
An implementer asserting `stderr.contains("--label cannot be combined with --markdown")`
would fail because `--description` precedes `--markdown` in the joined output and the
concatenated literal does not appear verbatim.

**Resolution**: EC-3.4.017-14 updated to state explicitly: "Assert
`stderr.contains("--markdown")` AND `stderr.contains("--label cannot be combined with")`
as two separate checks, NOT `stderr.contains("--label cannot be combined with --markdown")`
(that concatenation does not appear verbatim when `--description` precedes `--markdown`
in the joined output)."

Same instruction added to `prd-delta-407.md §4.3` with the verbatim stderr output shown:
`"--label cannot be combined with --description, --markdown in the same call. ..."`.

Also propagated the separate-assertion pattern to the general test pattern in §4.1 with
a note that applies to all 10 tests.

---

### F-L-003 (Process-gap — deferred, no action in this pass)

**Finding**: Test naming `test_label_plus_<flag>_rejected_with_exit_64_no_http` doesn't
match CLAUDE.md's `test_<verb>_<subject>_<expected_outcome>` convention. The two
FIX-F5-001 tests already use this pattern, so #407 is precedent-following, not a
regression.

**Disposition**: Recorded here for the cycle-closing checklist. No spec edit. Two options
for a future cycle:
- (a) Update CLAUDE.md to accept `test_<scenario>_<outcome>_<modifier>` as a documented
  guard-test-family convention.
- (b) Plan a future rename pass aligning these tests to the standard convention.
Neither option is urgent; the tests are named correctly for their family (guard tests
with a unique identity pattern) and a rename creates unnecessary churn without behavioral
improvement.

---

## Count Surfaces — Final State

| Surface | Value | Changed? |
|---------|-------|---------|
| bc-3-issue-write.md `total_bcs` | 103 | No |
| bc-3-issue-write.md `definitional_count` | 74 | No |
| bc-3-issue-write.md `last_updated` | 2026-05-25 | Yes (was 2026-05-22) |
| BC-INDEX.md `total_bcs` | 583 | No |
| BC-INDEX.md `last_updated` | 2026-05-25 | Yes (was 2026-05-22) |
| CANONICAL-COUNTS.md Sum | 583 | No |
| CANONICAL-COUNTS.md `last_verified` | 2026-05-25 (F2 #407) | Yes |

---

## Guard Script Results

```
bash scripts/check-spec-counts.sh
→ OK: all spec counts verified.
→ EXIT: 0

bash scripts/check-bc-cumulative-counts.sh
→ OK: all cumulative BC counts verified (583 total across 8 files; Surface H footer checked where present).
→ EXIT: 0
```

Both guards pass. Pass 1 findings fully resolved. Ready for pass 2 or implementer handoff.
