---
document_type: story
story_id: "S-400-B"
title: "Count-guard extension: add BC-INDEX Coverage Statistics, bc-*.md JSON Output Shape Contracts, footer, and CANONICAL-COUNTS Breakdown-bullet guards (closes #400 PG items)"
wave: feature-followup
status: ready
intent: infrastructure
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
# No BC anchor — all items are script/process-guard changes. No new behavioral contracts.
# The script extension enforces invariants that ALREADY exist in BC-INDEX.md and CANONICAL-COUNTS.md;
# it does not define new behavioral contracts. BC status: no BC authorship required.
# Status=ready because: F1 delta analysis accepted the decomposition for issue #400;
# no new BCs gate this story; all design decisions are locked.
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
  - PG-398-1
  - PG-398-2
  - issue-400
files_modified:
  - scripts/check-bc-cumulative-counts.sh  # MODIFIED — add Surface I (BC-INDEX Coverage Statistics table), Surface J (bc-*.md JSON Output Shape Contracts tables), Surface K (bc-*.md _Last updated footer), Surface L (CANONICAL-COUNTS.md Breakdown-bullet numeric literals)
files_created: []
breaking_change: false
assumption_validations: []
risk_mitigations: []
---

# S-400-B — Count-Guard Extension: BC-INDEX Coverage Statistics, JSON Output Shape Contracts, File Footers, and Breakdown Bullets

## Source of Truth

GitHub issue: https://github.com/Zious11/jira-cli/issues/400
F1 delta analysis: `.factory/phase-f1-delta-analysis/issue-400/delta-analysis.md`
Predecessor story: S-392 (PR that introduced the original `check-bc-cumulative-counts.sh`)
Process-gap origin: `.factory/phase-f2-spec-evolution/prd-delta-398.md` lines 331–338

## Goal

Extend `scripts/check-bc-cumulative-counts.sh` to guard four additional surfaces that drifted
during the S-398 cycle and were caught only by the consistency-validator rather than by the
script — turning a human-review catch into an automated script catch:

- **Surface I:** BC-INDEX.md Coverage Statistics body table (cumulative and individually-bodied
  counts per section must agree with per-file `total_bcs` and `definitional_count` data)
- **Surface J:** `bc-*.md` JSON Output Shape Contracts tables (present in `bc-3-issue-write.md`
  only; the table's `edit` row must reference `changed_fields` in a way that can be
  structurally verified)
- **Surface K:** `bc-*.md` trailing `_Last updated` footer (when present, the file's
  frontmatter total_bcs and definitional_count must appear in the footer text)
- **Surface L:** CANONICAL-COUNTS.md Breakdown-bullet numeric literals (the three
  bullet-point integers — total canonical count, individually-bodied count, range-collapsed
  count — must be self-consistent: individually-bodied + range-collapsed = canonical total)

The `last_verified` datestamp in CANONICAL-COUNTS.md (line 5) is explicitly NOT automated
by this story — see Out of Scope.

No production code changes. No BC file content changes.

## Background

`scripts/check-bc-cumulative-counts.sh` was introduced by S-392 and currently guards
8 surfaces (Surfaces A–H, with H added in an adversarial pass). The script exits 0 if all
guarded surfaces agree on BC counts.

During the S-398 cycle, the consistency-validator caught several drifts that `check-bc-cumulative-counts.sh`
did not catch, because those surfaces were outside the script's scope:

1. **BC-INDEX.md Coverage Statistics table** (`## Coverage Statistics` section, currently
   at line 686): a 9-row table that carries cumulative and individually-bodied counts per
   section. The process-gap note at `BC-INDEX.md:704` documents this as "a 9th surface with
   no automated guard." The table's **Total** row (`| **Total** | **583** | **351** |`) must
   agree with the grand-total surfaces already guarded (Surfaces E/F/G). The per-section
   rows must agree with per-file Surface A values.

2. **`bc-3-issue-write.md` JSON Output Shape Contracts table** (`## JSON Output Shape Contracts`
   section, at line 2212): a table listing JSON shapes for all write operations. The `edit`
   row must reference `changed_fields` (the BC-3.4.012/013 invariant). This is the only
   `bc-*.md` file that currently has this table. Guard: verify the table exists in `bc-3-issue-write.md`
   and that the `edit` row contains `changed_fields` — structural presence guard, not count guard.

3. **`bc-*.md` trailing `_Last updated` footer**: some bc files (confirmed: `bc-3-issue-write.md`
   line 2231) end with a `_Last updated ...` line. When present, this line describes BC count
   changes. Guard: verify that when a `_Last updated` line is present in a bc file, the
   total_bcs value from its frontmatter (Surface A) appears as a number somewhere in that footer
   text — e.g., the footer `... BC counts unchanged: 583 total, bc-3 103/74 ...` must contain
   the number `103` matching `bc-3-issue-write.md`'s `total_bcs: 103`. This prevents a stale
   footer from surviving a BC count bump.

4. **CANONICAL-COUNTS.md Breakdown bullets**: the three bullet-point integers in the
   `Breakdown:` section (currently at line 59–64) are:
   - `583 = sum of per-file total_bcs values` (must equal Surface F/G)
   - `351 of 583 are individually-bodied` (must equal sum of per-file `definitional_count` values)
   - `232 are range-collapsed` (must equal 583 − 351)
   Guard: extract these three numbers, verify they are internally consistent
   (individually-bodied + range-collapsed = total), and verify the total matches TOTAL_SUM.

## Scope

### Files modified
- `scripts/check-bc-cumulative-counts.sh` — add Surfaces I, J, K, L as new guard blocks,
  following the script's existing style and exit-code conventions

### Files NOT modified
- `src/` — zero production code changes
- `tests/` — zero test file changes (test strategy uses the script itself run against
  current `.factory/` state plus a brief negative fixture, both without creating new test files)
- `.factory/specs/prd/` — zero BC content changes
- `CLAUDE.md` — no new `JR_*` env vars introduced; no doc update required

## Acceptance Criteria

### AC-001 — Script passes on current `.factory/` state (Surface I, II, K, L all green)

`bash scripts/check-bc-cumulative-counts.sh` exits 0 against the current committed state
of `.factory/specs/prd/` files.

Verification:
```
bash scripts/check-bc-cumulative-counts.sh
```
Must exit 0 with output ending in `OK: all cumulative BC counts verified (...)`.

This verifies that the new Surface I/J/K/L guards do not produce false positives against
the current correctly-maintained state.

### AC-002 — Surface I: BC-INDEX.md Coverage Statistics **Total** row is checked

The script validates that the `| **Total** | **N** | **M** |` row in BC-INDEX.md's
`## Coverage Statistics` section satisfies:
- N (the cumulative total in the Total row) equals TOTAL_SUM (the sum of per-file Surface A values)
- M (the individually-bodied total in the Total row) equals the sum of per-file `definitional_count` frontmatter values

On a deliberate mismatch (e.g., the Total row is manually edited to `| **Total** | **999** | **351** |`),
the script must exit 1 with an error message that names `BC-INDEX.md Coverage Statistics Total row`
and identifies the mismatch.

Verification:
```
grep 'Coverage Statistics' scripts/check-bc-cumulative-counts.sh
```
Must return at least one match (the guard block is present in the script).

Manual negative test (reverting after): temporarily change BC-INDEX.md Coverage Statistics
**Total** cumulative count to a wrong value, run the script, observe exit 1 with a descriptive
error, then revert the change.

### AC-003 — Surface J: bc-3-issue-write.md JSON Output Shape Contracts table structural guard

The script verifies that `bc-3-issue-write.md` contains a `## JSON Output Shape Contracts`
section heading and that the section contains a table row for `edit` that includes the string
`changed_fields`.

On a missing section or missing `changed_fields` in the edit row, the script must exit 1
with an error message that names `bc-3-issue-write.md JSON Output Shape Contracts`.

Verification:
```
grep 'JSON Output Shape Contracts\|changed_fields' scripts/check-bc-cumulative-counts.sh
```
Must return at least one match (the guard block references these strings or patterns).

Note: Surface J is a structural presence guard, not a count guard. It asserts that the
`edit` row with `changed_fields` has not been accidentally deleted or renamed. It does NOT
count rows or validate all operation shapes.

### AC-004 — Surface K: bc-*.md _Last updated footer total_bcs consistency

For each `bc-*.md` and `cross-cutting.md` file that contains a line beginning with
`_Last updated`, the script verifies that the line's text contains the file's `total_bcs`
frontmatter value as a number.

Specifically: if `bc-3-issue-write.md` has `total_bcs: 103` and a footer line beginning
with `_Last updated`, then `103` must appear somewhere in that footer line. A footer that
says `no BC count changes (103/74 unchanged)` satisfies this guard.

On a mismatch (footer references a stale total_bcs value), the script must exit 1 with an
error message that names `Surface K: _Last updated footer` for the specific file.

Files without a `_Last updated` line are silently skipped (the footer is present only in
some bc files; absence is not an error).

Verification:
```
grep '_Last updated\|Surface K' scripts/check-bc-cumulative-counts.sh
```
Must return at least one match.

### AC-005 — Surface L: CANONICAL-COUNTS.md Breakdown bullets are self-consistent

The script extracts the three integers from the `Breakdown:` section of CANONICAL-COUNTS.md:
- `B_TOTAL` — the leading integer of the bullet `N = sum of per-file total_bcs values`
- `B_INDIV` — the integer from `N of B_TOTAL are individually-bodied`
- `B_RANGE` — the integer from `N are range-collapsed`

The script asserts:
1. `B_TOTAL` equals TOTAL_SUM (the computed sum of per-file Surface A values)
2. `B_INDIV + B_RANGE` equals `B_TOTAL` (internal consistency)

On any mismatch, the script exits 1 with an error message that names
`Surface L: CANONICAL-COUNTS.md Breakdown bullets` and identifies which assertion failed.

The `last_verified` datestamp on line 5 of CANONICAL-COUNTS.md is NOT checked by this
surface — see Out of Scope.

Verification:
```
grep 'Breakdown\|Surface L' scripts/check-bc-cumulative-counts.sh
```
Must return at least one match.

Manual negative test (reverting after): temporarily change the Breakdown `583 = sum ...`
bullet to `999 = sum ...`, run the script, observe exit 1 with `Surface L` in the error,
then revert.

### AC-006 — Script summary line updated

The `OK:` summary line emitted by the script (currently `OK: all cumulative BC counts verified
(N total across M files; Surface H footer checked where present).`) is updated to mention the
new surfaces, e.g.: `OK: all cumulative BC counts verified (N total across M files; Surfaces H/I/J/K/L
also verified).`

Verification:
```
grep 'Surfaces H' scripts/check-bc-cumulative-counts.sh
```
Must return 1 match (the updated summary line).

### AC-007 — Script comment block updated

The header comment block in `check-bc-cumulative-counts.sh` (lines 1–27 in the current file)
lists the guarded surfaces. It must be updated to document Surfaces I, J, K, and L with the
same style as the existing surface descriptions.

Verification:
```
grep 'Surface I\|Surface J\|Surface K\|Surface L' scripts/check-bc-cumulative-counts.sh
```
Must return at least 4 matches (one or more per surface).

### AC-008 — Existing Surfaces A–H are unmodified

The existing guard logic for Surfaces A through H is not broken or removed.

Verification:
```
bash scripts/check-bc-cumulative-counts.sh
```
Must exit 0 (AC-001 already covers this, but stated explicitly to emphasize that
existing behavior is preserved).

```
grep 'Surface A\|Surface B\|Surface C\|Surface D\|Surface E\|Surface F\|Surface G\|Surface H' scripts/check-bc-cumulative-counts.sh
```
Must return at least 8 matches.

### AC-009 — `cargo test`, fmt, clippy scripts pass

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
bash scripts/check-spec-counts.sh
bash scripts/check-bc-cumulative-counts.sh
bash scripts/check-bc-no-numeric-test-counts.sh
```
All exit 0. `cargo test` is not applicable (the script is bash, not Rust). The three
spec-count scripts confirm no spec drift from this story's changes.

## Out of Scope

- **`last_verified` datestamp automation** — `CANONICAL-COUNTS.md` line 5 contains
  `last_verified: "2026-05-25 (F2 delta issue #407; ...)"`. This is a free-text datestamp
  with no computable invariant. Its correctness depends on the author updating it when
  running the F2 cycle. It cannot be validated by script. Author discipline only;
  not automatable. Explicitly excluded from Surface L.

- **PG-398-3 (VP-count tri-surface guard)** — the VP-count invariant (new_vps: frontmatter
  length == `### VP-` heading count == VP-to-BC mapping-table row count in verification-delta
  files) is deferred. It would require a new `scripts/check-vp-delta-counts.sh` that parses
  `.factory/phase-f2-spec-evolution/verification-delta-*.md` files. That guard only triggers
  during F2 authoring and provides value only if wired into CI or the F7 checklist — a
  separate decision. Tracked at issue #400 but not delivered here.

- **PG-398-4 (adversary dispatch protocol)** — engine-scoped. The adversarial pass
  worktree-path misresolution is an engine-internal behavior. There is no jira-cli source
  artifact to modify. Must be tracked in the engine repo, not this PR.

- **PG-398-5 (F2 delta-authoring identifier validation)** — engine-scoped. The validation
  of function-name tokens in CLAUDE.md Gotcha entries is a factory engine concern. Must be
  tracked in the engine repo.

- **Per-section BC-INDEX.md Coverage Statistics row validation beyond the Total row** —
  Surface I checks only the `**Total**` row (global sum check). Validating each individual
  section row (e.g., "Section 3: Issue Write | 103 | 74") against its bc-N.md values would
  require a more complex section-name-to-file mapping. The Total row check covers the most
  common drift (a BC count change that updates per-file values but misses the Coverage
  Statistics total); per-section validation is deferred to a future script pass.

- **Story A test changes** — `tests/issue_edit_echo.rs` and `tests/issue_create_echo.rs`
  are addressed exclusively in S-400-A. This story touches only `scripts/`.

## Implementation Strategy

**Ordered sequence:**

1. Create branch `chore/S-400-B-count-guard-extension` from `develop`.

2. **Read `scripts/check-bc-cumulative-counts.sh` in full** before editing. Confirm the
   current structure: the per-file loop (Surfaces A–D + body prose + Surface H), the
   grand-total block (Surfaces E–G), and the `OK:` summary line. Note the exact bash
   style used (variable naming, `awk`, `sed`, `grep` patterns, ERRORS increment pattern).

3. **Read relevant sections of BC-INDEX.md** to understand the exact regex patterns needed:
   - `## Coverage Statistics` section: confirm the table format
     `| **Total** | **583** | **351** |`
   - Verify the section heading is literal `## Coverage Statistics`

4. **Read CANONICAL-COUNTS.md** Breakdown section (around lines 59–64) to confirm the
   exact bullet format: `- 583 = sum of per-file \`total_bcs\` values`,
   `- 351 of 583 are individually-bodied`, `- 232 are range-collapsed`.

5. **Read `bc-3-issue-write.md`** around lines 2212–2231 to confirm:
   - The section heading `## JSON Output Shape Contracts` exists
   - The `edit` row contains `changed_fields`
   - The `_Last updated` footer line exists and contains `103` (matching `total_bcs: 103`)

6. **Add Surface I guard** — after the existing grand-total block (Surfaces E–G) and
   before the `OK:` summary line. Extract the Coverage Statistics **Total** row's cumulative
   count and individually-bodied count; compare to TOTAL_SUM and sum of definitional_counts.

7. **Compute DEF_COUNT_SUM** — the script already accumulates TOTAL_SUM (sum of Surface A
   values) in the per-file loop. Add a parallel `DEF_COUNT_SUM` accumulation for
   `definitional_count` values to support Surface I and Surface L checks.

8. **Add Surface J guard** — a targeted check on `bc-3-issue-write.md` specifically:
   verify `## JSON Output Shape Contracts` heading exists AND a line containing
   `| \`edit\`` also contains `changed_fields`.

9. **Add Surface K guard** — in the per-file loop, after the existing Surface H check,
   add a check for `_Last updated` lines. When present, verify the file's `total_bcs`
   value appears in the footer line.

10. **Add Surface L guard** — extract the three Breakdown bullet integers from
    CANONICAL-COUNTS.md. Verify B_TOTAL equals TOTAL_SUM, and B_INDIV + B_RANGE equals
    B_TOTAL.

11. **Update header comment block** — add Surface I, J, K, L descriptions.

12. **Update `OK:` summary line** — mention new surfaces.

13. **Run the script** against current `.factory/` state: `bash scripts/check-bc-cumulative-counts.sh`
    must exit 0 (AC-001).

14. **Perform negative fixture test** (do not commit):
    - Temporarily change BC-INDEX.md Coverage Statistics **Total** cumulative count to 999
    - Run script: must exit 1 with `Surface I` or `Coverage Statistics` in error
    - Revert the change
    - Temporarily change CANONICAL-COUNTS.md Breakdown `583 =` to `999 =`
    - Run script: must exit 1 with `Surface L` or `Breakdown` in error
    - Revert the change
    Both negative tests confirm the guards actually fire.

15. **Verify existing Surfaces A–H** still pass (AC-008).

16. **Commit:**
    ```
    chore(scripts): extend check-bc-cumulative-counts.sh with Coverage Statistics, JSON Shape, footer, and Breakdown guards (closes #400 PG)
    ```

17. Open PR targeting `develop`; body includes `Closes #400` (or partial reference
    matching Story A's PR).

## Test Strategy

This story has no Rust tests. The "test" is the script itself run against the live
`.factory/` state.

| Test type | How | Expected outcome |
|-----------|-----|-----------------|
| Positive fixture (AC-001) | `bash scripts/check-bc-cumulative-counts.sh` on committed state | Exit 0 |
| Negative fixture — Surface I | Temporarily corrupt BC-INDEX.md Coverage Statistics **Total** cumulative count, run script, revert | Exit 1, error names Coverage Statistics |
| Negative fixture — Surface L | Temporarily corrupt CANONICAL-COUNTS.md Breakdown total bullet, run script, revert | Exit 1, error names Breakdown |
| Existing Surfaces A–H regression | `bash scripts/check-bc-cumulative-counts.sh` (same as AC-001 positive fixture) | Exit 0 — all 8 surfaces still pass |

The negative fixture tests are run locally during implementation (step 14) to confirm the
guards actually fire. They are NOT committed — the temporary corruption is reverted before
committing. The positive fixture is the CI-visible proof of correctness.

## Quality Gate Self-Check

| Criterion | Required | Verification command |
|-----------|----------|----------------------|
| Script exits 0 on current state | AC-001 | `bash scripts/check-bc-cumulative-counts.sh` |
| Surface I guard block present | AC-002 | `grep 'Coverage Statistics' scripts/check-bc-cumulative-counts.sh` → match |
| Surface J guard block present | AC-003 | `grep 'JSON Output Shape Contracts\|changed_fields' scripts/check-bc-cumulative-counts.sh` → match |
| Surface K guard block present | AC-004 | `grep '_Last updated\|Surface K' scripts/check-bc-cumulative-counts.sh` → match |
| Surface L guard block present | AC-005 | `grep 'Breakdown\|Surface L' scripts/check-bc-cumulative-counts.sh` → match |
| Summary line updated | AC-006 | `grep 'Surfaces H' scripts/check-bc-cumulative-counts.sh` → 1 match |
| Header comment updated | AC-007 | `grep 'Surface I\|Surface J\|Surface K\|Surface L' scripts/check-bc-cumulative-counts.sh` → ≥4 matches |
| Existing Surfaces A–H present | AC-008 | `grep 'Surface A\|Surface B\|Surface C\|Surface D\|Surface E\|Surface F\|Surface G\|Surface H' scripts/check-bc-cumulative-counts.sh` → ≥8 matches |
| `cargo fmt --check` exits 0 | AC-009 | `cargo fmt --all -- --check` (no Rust changes; should be trivially clean) |
| spec-count scripts exit 0 | AC-009 | `bash scripts/check-spec-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` |
| No src/ files modified | (scope) | `git diff --name-only \| grep '^src/'` → no output |
| No .factory/specs/prd/ files modified | (scope) | `git diff --name-only \| grep '\.factory/specs/prd'` → no output |
| Negative fixture — Surface I fires | AC-002 | Manual step: corrupt Coverage Statistics Total row → script exits 1 → revert |
| Negative fixture — Surface L fires | AC-005 | Manual step: corrupt Breakdown total bullet → script exits 1 → revert |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~5 k |
| `scripts/check-bc-cumulative-counts.sh` (full, ~252 LOC) | ~4 k |
| `BC-INDEX.md` (targeted: Coverage Statistics section ~lines 686–705) | ~1 k |
| `CANONICAL-COUNTS.md` (full, ~90 LOC) | ~2 k |
| `bc-3-issue-write.md` (targeted: JSON Output Shape Contracts + footer ~lines 2212–2231) | ~1 k |
| Tool outputs (script runs, grep verifications) | ~2 k |
| **Total** | **~15 k** |

Well within a single-agent context window (~200 k). No split required.
LOC delta estimate: `scripts/check-bc-cumulative-counts.sh` +~60 LOC (4 new guard blocks
~12–15 LOC each, updated header comment ~5 LOC, updated summary line ~2 LOC).

## Tasks

- [ ] Create branch `chore/S-400-B-count-guard-extension` from `develop`
- [ ] Read `scripts/check-bc-cumulative-counts.sh` in full — confirm current structure, variable naming conventions, ERRORS increment pattern, position of grand-total block and `OK:` summary line
- [ ] Read `BC-INDEX.md` `## Coverage Statistics` section (around line 686) — confirm table format and exact heading text
- [ ] Read `CANONICAL-COUNTS.md` Breakdown section (around lines 59–64) — confirm bullet format: `- 583 = sum ...`, `- 351 of 583 are individually-bodied`, `- 232 are range-collapsed`
- [ ] Read `bc-3-issue-write.md` around lines 2212–2231 — confirm `## JSON Output Shape Contracts` heading, `edit` row `changed_fields` content, and `_Last updated` footer format
- [ ] Add `DEF_COUNT_SUM` accumulation to the per-file loop (parallel to `TOTAL_SUM`) — sum of `definitional_count` frontmatter values; needed for Surface I and L checks
- [ ] Add Surface K guard in per-file loop: after Surface H block, check for `_Last updated` line; when present, assert `total_bcs` value appears in the footer text (AC-004)
- [ ] Add Surface I guard in grand-total block: extract Coverage Statistics `**Total**` row cumulative count and individually-bodied count; compare to TOTAL_SUM and DEF_COUNT_SUM respectively (AC-002)
- [ ] Add Surface J guard in grand-total block (or as standalone check): verify `bc-3-issue-write.md` contains `## JSON Output Shape Contracts` heading and an `edit` row with `changed_fields` (AC-003)
- [ ] Add Surface L guard in grand-total block: extract three Breakdown bullet integers; assert B_TOTAL == TOTAL_SUM and B_INDIV + B_RANGE == B_TOTAL (AC-005)
- [ ] Update header comment block to document Surfaces I, J, K, L (AC-007)
- [ ] Update `OK:` summary line to mention new surfaces (AC-006)
- [ ] Run `bash scripts/check-bc-cumulative-counts.sh` against current state — must exit 0 (AC-001)
- [ ] Negative fixture — Surface I: temporarily corrupt BC-INDEX.md Coverage Statistics **Total** cumulative count to 999; verify script exits 1 with Coverage Statistics in error; revert (AC-002)
- [ ] Negative fixture — Surface L: temporarily corrupt CANONICAL-COUNTS.md Breakdown total bullet from `583` to `999`; verify script exits 1 with Breakdown or Surface L in error; revert (AC-005)
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` — all exit 0 (AC-009)
- [ ] Run `cargo fmt --all -- --check` — exits 0 (no Rust changes; trivially clean) (AC-009)
- [ ] Commit: `chore(scripts): extend check-bc-cumulative-counts.sh with Coverage Statistics, JSON Shape, footer, and Breakdown guards (closes #400 PG)`
- [ ] Open PR targeting `develop`; body includes `Closes #400`

## Previous Story Intelligence

**Direct predecessor: S-392** introduced `check-bc-cumulative-counts.sh` with Surfaces A–G.
An adversarial pass later added Surface H (footer `## Total BCs in this file:` checks).
This story follows the same pattern: each extension adds a new lettered surface with its own
error diagnostic, matching the established script style.

**Lesson from S-398 PG-398-1 diagnosis (F1 delta analysis):** The Coverage Statistics table
drifted in the S-398 cycle and was caught only by the consistency-validator. Scripts must
guard surfaces that humans frequently update manually. The BC-INDEX.md Coverage Statistics
**Total** row is updated manually every time BC counts change — it is exactly the kind of
surface that benefits from automated checking.

**Script style discipline:** Follow the existing `check-bc-cumulative-counts.sh` conventions:
- Use `awk`, `sed`, and `grep` inline for parsing (no Python or jq dependency)
- Each guard block increments `ERRORS=$((ERRORS+1))` and emits an `echo "ERROR: ..."` line
- Each guard block has an `if [ -z "$extracted_val" ]` absence check before the comparison
- Use `|| true` on grep/sed pipe stages to prevent `set -e` from aborting on no-match
- The `if [[ "$val" =~ ^[0-9]+$ ]]` integer-parse check is used before numeric comparisons

**Surface J is structural, not count-based.** Unlike all other surfaces, Surface J asserts
presence of content (the `changed_fields` string in a table row) rather than a count equality.
This is intentional — the JSON Output Shape Contracts table is a contract document, not a
count surface. Grep-based presence check is the right tool.

**PG-398-2 `last_verified` partial scope:** The F1 analysis confirmed that `last_verified`
on CANONICAL-COUNTS.md line 5 is a datestamp with no computable invariant. Surface L
covers the Breakdown bullet integers only; it does NOT attempt to validate the datestamp.
Document this explicitly in the script comment block so future maintainers do not attempt
to add a `last_verified` check.

## Architecture Compliance Rules

1. **Shell script only.** No Rust, Python, or other language changes. The script must remain
   a pure bash script (`#!/usr/bin/env bash`) compatible with the existing CI environment.

2. **No new script file.** All Surface I/J/K/L guards are added to
   `scripts/check-bc-cumulative-counts.sh`. Do not create a sibling script.

3. **Existing Surfaces A–H logic is not modified.** The extension adds new guard blocks;
   it does NOT refactor, rename, or alter existing guard logic. If a refactor would make
   the new blocks cleaner, the refactor is out of scope.

4. **`set -euo pipefail` is preserved.** The script currently starts with `set -euo pipefail`.
   New guard blocks must use `|| true` on any grep/sed/awk pipeline that may return non-zero
   (no-match) without intending to abort.

5. **Integer-safe parsing.** Every numeric value extracted from a file must pass through
   the `[[ "$val" =~ ^[0-9]+$ ]]` pattern check before arithmetic comparison, matching
   the existing Surface A–H style.

6. **No external tool dependencies.** The script must not introduce `jq`, `python`, `node`,
   or any tool not already used by the existing script. Parsing uses `awk`, `sed`, `grep`.

7. **Negative fixture tests are ephemeral.** The temporary corruptions made during negative
   fixture testing (step 14 of Implementation Strategy) are NOT committed. The committed state
   of BC-INDEX.md and CANONICAL-COUNTS.md is unchanged.

8. **No BC content changes.** `.factory/specs/prd/bc-*.md`, `BC-INDEX.md`, and
   `CANONICAL-COUNTS.md` contents are not modified. The script only reads those files.

## Library & Framework Requirements

No new dependencies. The script extension uses only:
- `bash` (already required)
- `awk`, `sed`, `grep` (already used throughout the script)
- Standard POSIX utilities: `test`, `[[ ... ]]` bash conditionals

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `scripts/check-bc-cumulative-counts.sh` | Modify | Add Surfaces I, J, K, L; update header comment; update summary line |

**Files NOT to create:** No new scripts, no new test files, no new spec files.

**Files NOT to touch:** all `src/` files, all `tests/` files, all `.factory/specs/prd/`
files, `CLAUDE.md`, `STORY-INDEX.md` (state-manager handles that), `Cargo.toml`,
`deny.toml`, `CHANGELOG.md` (no user-visible behavior change).

## Branch / PR Plan

- Branch: `chore/S-400-B-count-guard-extension`
- Target: `develop`
- Commit style: `chore(scripts): extend check-bc-cumulative-counts.sh with Coverage Statistics, JSON Shape, footer, and Breakdown guards (closes #400 PG)`
- PR closes: `Closes #400` (or partial reference matching Story A's PR)
- CHANGELOG entry: not required (infrastructure script; no user-visible behavior change)
