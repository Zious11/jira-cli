---
document_type: adversarial-review
issue: "#407"
pass: 4
date: "2026-05-25"
phase: F2
verdict: CLEAN (after fixes applied in this pass)
findings_count: 2
findings_resolved: 2
findings_deferred: 0
guard_check_spec_counts: exit 0
guard_check_bc_cumulative_counts: exit 0
---

# Adversarial Review — Issue #407 F2, Pass 4

## Summary

Pass 4 returned 2 LOW observations carried from pass 3 as actionable items. Both
resolved in this pass. Convergence trajectory: 7 → 2 → 1 → 2 (both resolved). Spec is
implementer-ready for F3/F4.

Files edited:
- `.factory/specs/prd/bc-3-issue-write.md` (EC-3.4.017-14 flag-derivation rule)
- `.factory/phase-f2-spec-evolution/prd-delta-407.md` (§4.2 step 3)
- `.factory/specs/prd/CANONICAL-COUNTS.md` (line 57 parenthetical)

BC counts unchanged: **583 total, bc-3 103/74**.

---

## Findings and Resolutions

### O-1 (LOW) — Flag-name derivation rule misleading for 10 implicit-transform fields

**Files**: `bc-3-issue-write.md` EC-3.4.017-14 Expected set construction section;
`prd-delta-407.md §4.2 step 3`

**Finding**: The rule "Flag names are derived from the `#[arg(long = "...")]` attribute
on each field, NOT by mechanical snake→kebab transform" was misleading. In
`src/cli/mod.rs` lines 433-474, only 2 of the 12 conflict-block fields carry an
explicit `long = "<literal>"`:
- `issue_type`: `#[arg(long = "type")]` (genuine rename)
- `field`: `#[arg(long = "field", ...)]` (mechanical-equivalent, explicit)

The other 10 (`summary`, `priority`, `team`, `points`, `no_points`, `parent`,
`no_parent`, `description`, `description_stdin`, `markdown`) use `#[arg(long)]` with
NO literal value — clap derives those via snake→kebab. An implementer reading the
absolute "NOT by mechanical snake→kebab" rule could build an extractor that requires
explicit `long = "..."` on every field and emits a parse error for the 10 implicit
cases.

**Resolution**: Both the EC body and `prd-delta-407.md §4.2 step 3` rewritten to
the corrected two-part rule:

> "For each field, the kebab-case CLI flag name is the explicit `long = "<literal>"`
> value when present, otherwise the field name with underscores replaced by hyphens
> (clap's implicit default). Of the 12 fields currently in scope: `issue_type` carries
> `#[arg(long = "type")]` and maps to `--type` (NOT `--issue-type`); the other 11
> (`summary`, `priority`, `team`, `points`, `no_points`, `parent`, `no_parent`,
> `description`, `description_stdin`, `markdown`, `field`) use the implicit
> snake→kebab transform. Any future field added to `BULK_SUPPORTED`/`REJECTED_IN_BULK`
> with a non-mechanical `long = "..."` rename will be caught by the R2 pin's 12-flag
> enumeration — the extractor side and the expected side must be reconciled together."

---

### O-2 (LOW) — CANONICAL-COUNTS.md line 57 parenthetical stale delta text

**File**: `CANONICAL-COUNTS.md` line 57

**Finding**: The `last_verified` frontmatter was bumped to 2026-05-25 by #407 but the
prose parenthetical on line 57 still read "(+3 since last verified: BC-3.4.015..017
added at F2 issue #396 2026-05-22)". That delta was against the old 2026-05-22
baseline. After the 2026-05-25 verification the delta-since-last-verified is +0 BCs
(#407 added EC-3.4.017-14 only, no BC count change).

**Resolution**: Updated line 57 parenthetical to:

> "(+0 BC count change since last verified — issue #407 F2 added EC-3.4.017-14 only;
> the prior +3 was BC-3.4.015..017 from #396 F2 2026-05-22)"

Preserves the audit trail of the prior +3 while accurately reflecting the current
delta-since-last-verified.

---

## Count Surfaces — Final State

| Surface | Value | Changed in pass 4? |
|---------|-------|--------------------|
| bc-3-issue-write.md `total_bcs` | 103 | No |
| bc-3-issue-write.md `definitional_count` | 74 | No |
| bc-3-issue-write.md `last_updated` | 2026-05-25 | No (set in pass 1) |
| BC-INDEX.md `total_bcs` | 583 | No |
| BC-INDEX.md `last_updated` | 2026-05-25 | No (set in pass 1) |
| CANONICAL-COUNTS.md Sum | 583 | No |
| CANONICAL-COUNTS.md `last_verified` | 2026-05-25 (F2 #407) | No (set in pass 1) |
| CANONICAL-COUNTS.md line 57 parenthetical | +0 since verified | Yes (O-2 fix) |

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

---

## Convergence Assessment

Trajectory: 7 (pass 1 NOT-CLEAN) → 2 (pass 2 CLEAN, informational) →
1 (pass 3 CLEAN, informational carry) → 2 (pass 4 CLEAN after fixes).

Both pass-4 findings were LOW observations that do not block implementer correctness
but were swept to prevent recurrence in the consistency-validator pass. Spec is
implementer-ready for F3/F4.

Deferred to cycle-closing checklist: F-L-003 (test naming convention divergence from
CLAUDE.md `test_<verb>_<subject>_<expected_outcome>`; precedent-following, not a
regression; recorded in pass 1).
