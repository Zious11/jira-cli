---
document_type: consistency-audit
issue: "#407"
title: "F2 spec delta consistency audit — EC-3.4.017-14"
date: "2026-05-25"
auditor: consistency-validator
scope: pre-human-approval-gate
---

# Consistency Audit — Issue #407 F2 Spec Delta

## Audit Scope

Six documents audited for cross-document consistency:
1. `.factory/specs/prd/bc-3-issue-write.md` — BC-3.4.017 invariant 2 + EC-3.4.017-14
2. `.factory/specs/prd/BC-INDEX.md`
3. `.factory/specs/prd/CANONICAL-COUNTS.md`
4. `.factory/phase-f2-spec-evolution/prd-delta-407.md`
5. `.factory/phase-f2-spec-evolution/prd-delta-396.md` — navigation pointer amendment
6. `.factory/phase-f1-delta-analysis/issue-407/delta-analysis.md`

Source spot-checks: `src/cli/issue/create.rs` (lines 445-492, 1523-1722) and
`src/cli/mod.rs` (line 437).

---

## Guard Script Results

```
scripts/check-spec-counts.sh      EXIT: 0   (OK — all spec counts verified)
scripts/check-bc-cumulative-counts.sh  EXIT: 0   (OK — 583 total, bc-3 103/74)
```

Both guards pass. No mechanical count drift.

---

## Check 1 — Count-Surface Consistency (9 surfaces + 2 frontmatter dates)

| Surface | Expected | Actual | Match? |
|---------|----------|--------|--------|
| bc-3 `total_bcs` frontmatter | 103 | 103 | PASS |
| bc-3 `definitional_count` frontmatter | 74 | 74 | PASS |
| bc-3 preamble prose ("103 behavioral contracts") | 103 | 103 | PASS |
| BC-INDEX `total_bcs` frontmatter | 583 | 583 | PASS |
| BC-INDEX section 3.4 header count | 17 BCs | 17 BCs | PASS |
| CANONICAL-COUNTS.md per-file table (bc-3 row) | 103 | 103 | PASS |
| CANONICAL-COUNTS.md Sum row | 583 | 583 | PASS |
| CANONICAL-COUNTS.md grand-total prose | 583 | 583 | PASS |
| bc-3 `_Last updated_` prose footer (Surface H) | 2026-05-25 / #407 | present, correct | PASS |
| bc-3 `last_updated` frontmatter | 2026-05-25 | 2026-05-25 | PASS |
| BC-INDEX `last_updated` frontmatter | 2026-05-25 | 2026-05-25 | PASS |
| CANONICAL-COUNTS `last_verified` frontmatter | "2026-05-25 (F2 delta issue #407; EC-3.4.017-14...)" | matches | PASS |
| prd-delta-407 surface summary table | unchanged (103/74; 583) | consistent with above | PASS |

All count surfaces: CONSISTENT.

---

## Check 2 — Broken or Dangling Cross-References

### EC-3.4.017-14 internal references

- `test_label_conflict_block_lists_every_relevant_flag` — referenced as the enforcement
  mechanism. No such function exists in `src/cli/issue/create.rs` yet (confirmed by grep).
  This is correct and expected: this is an F2 SPEC delta; the test is an F3 implementation
  artifact. The spec documents what will exist, not what exists today.

- `include_str!("create.rs")` path convention — the EC specifies path relative to the test
  module which lives in `create.rs` itself. This is consistent with the existing
  `include_str!("../mod.rs")` usage in `test_343_every_edit_field_is_categorized` (line 1525),
  adjusted for the fact that this new meta-test will live inside `create.rs` itself rather
  than reading a sibling file.

- `BULK_SUPPORTED \ {"label"}) ∪ REJECTED_IN_BULK` set reference — the actual sets are
  defined in `test_343_every_edit_field_is_categorized` (lines 1543-1569 of `create.rs`).
  The EC points to these as the expected-set source. The set contents in the source match
  the 12-flag enumeration specified in EC-3.4.017-14:
  BULK_SUPPORTED: `{summary, issue_type, priority, label}` → minus `label` = 3 entries
  REJECTED_IN_BULK: `{parent, no_parent, team, points, no_points, description, description_stdin, markdown, field}` = 9 entries
  Total: 3 + 9 = 12 entries. CONSISTENT with spec claim.

- BC-3.4.017 invariant 2 cross-reference to EC-3.4.017-14 — invariant 2 body (line 1473-1474):
  "The `--label` conflict block's completeness against that partition is mechanically
  enforced by `test_label_conflict_block_lists_every_relevant_flag` (see EC-3.4.017-14)."
  This bidirectional cross-reference is present and correct.

- `[Issue #407]` tag in EC-3.4.017-14 body (line 1593) — present, correct.

### prd-delta-396 navigation pointer

- Line 321: `EC-3.4.017-1 through EC-3.4.017-14 (EC-3.4.017-13 added FIX-F5-001;
  EC-3.4.017-14 added issue #407 F2)` — correct. EC-3.4.017-14 IS the last entry in the
  bc-3 file's BC-3.4.017 edge cases section. Navigation pointer is accurate.

### Other spec corpus — does anything else need to reference EC-3.4.017-14?

- `nfr-catalog.md` — test-only change; no NFR updated. CONSISTENT (no update needed).
- `holdout-scenarios.md` — no new production behaviors; no holdout needed. CONSISTENT.
- `module-criticality.md` — no module changes. CONSISTENT.
- STORY-INDEX.md — no S-407 story exists yet (F3 creates it). Absence is correct.
- `tests/issue_edit_field.rs` — the two FIX-F5-001 predecessor tests
  (`test_label_plus_field_rejected_with_exit_64_no_http` at line 2818,
  `test_label_plus_summary_rejected_with_exit_64_no_http` at line 2869) exist. The 10
  new tests and the meta-test do NOT yet exist — this is expected (F3 implementation).

---

## Check 3 — Naming / Taxonomy Consistency

### EC numbering monotonicity

EC-3.4.017 sequence: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14.
Monotonic: PASS. EC-3.4.017-14 immediately follows EC-3.4.017-13 in the body file.

### Test naming convention

prd-delta-407 specifies: `test_label_plus_<flag>_rejected_with_exit_64_no_http`.
This mirrors the two existing FIX-F5-001 tests exactly. Convention: CONSISTENT.

### Snake_case substitution rule

EC-3.4.017-14 specifies: "Test names use snake_case substitution for kebab-case flags
(e.g., `--no-points` → `test_label_plus_no_points_...`)."
prd-delta-407 §4.1 table enumerates all 10 test names using snake_case.
Convention stated twice (EC and delta): CONSISTENT.

### Meta-test function name

`test_label_conflict_block_lists_every_relevant_flag` — named in both EC-3.4.017-14 and
prd-delta-407 §4.2. CONSISTENT across both documents.

---

## Check 4 — Scope / Perimeter Gaps

### No S-407 story file exists prematurely

Confirmed: no `.factory/stories/S-407-*.md` file exists. STORY-INDEX.md has no S-407
entry. This is correct — story creation is F3 scope. PASS.

### F1 delta-analysis "No changes required" inconsistency with actual F2 output

**Finding (LOW severity — documentation drift, not a blocking gap):**
The F1 delta-analysis §No changes required (line 172) lists:
> `.factory/specs/prd/bc-3-issue-write.md` — no new BCs

The actual F2 spec evolution DID modify `bc-3-issue-write.md` (EC-3.4.017-14 added,
invariant 2 updated). This is not an error — the F1 analysis was written before the
human-gate decision to document the meta-test as EC-3.4.017-14, and the F1 "no changes"
referred to BC count changes. The prd-delta-407 §2 and §5 correctly document the actual
scope. This is an F1 forecast vs F2 outcome divergence, expected in the VSDD workflow
and not a consistency violation that blocks the gate.

### F1 delta-analysis meta-test location vs F2 actual location

F1 delta-analysis §Modified files listed the 1 structural meta-test under
`tests/issue_edit_field.rs` (before Q1 was resolved), with `create.rs` modification
only if approach (ii)/(iii) chosen.

prd-delta-407 (post-human-gate, approach b selected) places the meta-test in
`src/cli/issue/create.rs #[cfg(test)]` block, and the 10 positive tests in
`tests/issue_edit_field.rs`.

This is a correctly-resolved Q1 decision. The F1 analysis pre-dates the gate; the
prd-delta-407 is authoritative. CONSISTENT (resolved at gate).

---

## Check 5 — Consistency with CLAUDE.md Conventions and F1 Design Decisions

### Approach (b) confirmed

Human gate selected approach (b): dedicated meta-test, no production code changes.
prd-delta-407 §4.2 and EC-3.4.017-14 both reflect approach (b). CONSISTENT.

### `include_str!` + `BTreeSet<String>` pattern

EC-3.4.017-14 specifies `BTreeSet<String>` (not `HashSet`) for deterministic failure
diffs. This mirrors the existing `test_343_every_edit_field_is_categorized` pattern
which also uses `BTreeSet`. CONSISTENT with established precedent.

### `issue_type → --type` clap rename callout

EC-3.4.017-14 explicitly calls out: "`issue_type` carries `#[arg(long = \"type\")]`
and maps to `--type` (NOT `--issue-type`)."
Source confirms `#[arg(long = "type")]` on the Edit variant `issue_type` field (mod.rs
line 437). CONSISTENT.

### `--markdown` two-assertion pattern

EC-3.4.017-14 and prd-delta-407 §4.3 both specify: assert `stderr.contains("--markdown")`
AND `stderr.contains("--label cannot be combined with")` as TWO SEPARATE checks. Do NOT
assert the concatenated substring. CONSISTENT between documents.

### R2 pin requirement for the extractor

EC-3.4.017-14 requires at least one R2 pin test for the `conflicting.push` extraction
heuristic. This mirrors the three existing R2 pin tests for `extract_edit_field_names`
(lines 1644-1689 of create.rs). CONSISTENT with established pattern.

---

## Check 6 — Spec-vs-Source Spot-Checks

### `src/cli/issue/create.rs` lines 445-492 — conflict block entry count

Source has exactly 12 `conflicting.push(...)` entries (confirmed by grep count = 12):
```
--summary, --priority, --type, --team, --points, --no-points,
--parent, --no-parent, --description, --description-stdin, --markdown, --field
```
EC-3.4.017-14 R2 pin specifies exactly these 12 members. CONSISTENT.

### `src/cli/mod.rs` line 437 — `#[arg(long = "type")]` on `issue_type`

Confirmed: `#[arg(long = "type")]` on the Edit variant `issue_type` field at mod.rs
lines 437-438. CONSISTENT with EC-3.4.017-14 claim.

### 10 other Edit fields — `#[arg(long)]` with no literal value

Inspected mod.rs lines 430-475 (full Edit variant). Fields summary, priority, label,
team, points, no_points, parent, no_parent, description, description_stdin, markdown
all use `#[arg(long)]` (no literal override) confirming implicit snake→kebab applies.
`field` uses `#[arg(long = "field", ...)]` — explicit but identical to the implicit
default (`field` has no underscores). The `long = "field"` annotation exists because
it is paired with `action = clap::ArgAction::Append` rather than for name override.
EC-3.4.017-14 groups `field` with the "implicit snake→kebab" 11 — this is substantively
correct (the CLI name is `--field` either way). CONSISTENT.

### `src/cli/issue/create.rs` lines 1523-1722 — `test_343_every_edit_field_is_categorized`

Confirmed: function exists at line 1524 with the `include_str!("../mod.rs")` pattern
at line 1525. `extract_edit_field_names` helper defined at line 1713. `REJECTED_IN_BULK`
set at lines 1557-1569 includes `field` (added for BC-3.4.017 Gate A). R2 pin tests
present at lines 1644-1689. CONSISTENT.

### Guard comment in `create.rs` — F3 obligation, not yet present

EC-3.4.017-14 specifies that a guard comment MUST be added in `create.rs` at the
`conflicting` variable declaration (line 446): the comment must reserve the variable
name `conflicting` for the `test_label_conflict_block_lists_every_relevant_flag` global
scan. This comment is NOT present in the current source.

This is a **F3 implementation obligation**, not a spec inconsistency. The EC documents
what the implementation MUST include when the tests are written. The spec itself is
consistent; the source is pre-implementation. No blocking gap in the spec.

---

## Summary of Findings

| Finding | Severity | Classification | Status |
|---------|----------|----------------|--------|
| Guard script `check-spec-counts.sh` exit 0 | — | PASS | No action |
| Guard script `check-bc-cumulative-counts.sh` exit 0 | — | PASS | No action |
| All 9 count surfaces consistent | — | PASS | No action |
| Both `last_updated` frontmatter dates at 2026-05-25 | — | PASS | No action |
| EC-3.4.017-14 numbering monotonic (12→13→14) | — | PASS | No action |
| EC-3.4.017-14 body content internally consistent | — | PASS | No action |
| BC-3.4.017 invariant 2 cross-references EC-3.4.017-14 | — | PASS | No action |
| prd-delta-396 navigation pointer reflects EC-3.4.017-14 | — | PASS | No action |
| 12-flag set in source matches EC-3.4.017-14 R2 pin enumeration | — | PASS | No action |
| `issue_type #[arg(long = "type")]` confirmed | — | PASS | No action |
| meta-test not yet in create.rs (F3, expected absence) | — | EXPECTED | No action |
| 10 positive tests not yet in issue_edit_field.rs (F3, expected absence) | — | EXPECTED | No action |
| bc-3 frontmatter `trace:` block has NO entry for 2026-05-25 / #407 | LOW | OBSERVATION | See note below |
| F1 delta-analysis §No changes listed bc-3, but F2 did modify it | LOW | F1/F2 drift | Expected, not blocking |

### Note on missing frontmatter trace entry

Every prior EC addition in bc-3 (EC-3.4.012-16 at adversary round 8, EC-3.4.013-13
at adversary round 8, EC-3.4.017-2/10/11/12 at adversary pass 1, etc.) has a
corresponding entry in the frontmatter `trace:` block. The 2026-05-25 addition of
EC-3.4.017-14 does NOT have a `trace:` entry; the `_Last updated_` prose footer
(Surface H) was updated instead. prd-delta-407 §7 listed only 4 non-count surfaces
(all of which were updated) and did not include the `trace:` block.

This is a LOW-severity observation — it is a convention gap, not a correctness error.
The `_Last updated_` footer at line 2207 carries equivalent information. The `trace:`
block is strictly optional for EC-only additions (unlike new-BC additions). No blocking
issue; the human approver may choose to add a trace entry for uniformity with prior
cycles, but this is stylistic.

**Suggested (non-blocking) trace entry to add after line 59 in bc-3-issue-write.md:**
```
  - F2 modified (2026-05-25): BC-3.4.017 — EC-3.4.017-14 added (mechanical enforcement meta-test for invariant 2 completeness); invariant 2 cross-reference added (issue #407 F2)
```

---

## Verdict

**CONSISTENT**

All count surfaces verified by automated guards (both exit 0). No broken cross-references.
EC-3.4.017-14 body is internally consistent and correctly cross-referenced by BC-3.4.017
invariant 2. The prd-delta-396 navigation pointer accurately reflects the new EC range.
Source spot-checks confirm 12 conflict-block entries matching the spec enumeration,
`issue_type` using `#[arg(long = "type")]`, and the `test_343` precedent pattern in place.

One LOW-severity observation: the `trace:` frontmatter block in bc-3-issue-write.md was
not updated with a 2026-05-25 / #407 entry (unlike all prior EC additions). This is
stylistic and does not block approval. The gate-critical surfaces (guard scripts, count
surfaces, cross-reference integrity, navigation pointers) are all clean.
