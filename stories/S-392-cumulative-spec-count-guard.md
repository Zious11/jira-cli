---
document_type: story
story_id: "S-392"
title: "Add cumulative BC-count CI guard: check-bc-cumulative-counts.sh + DRIFT-002 (closes #392)"
wave: feature-followup
status: ready
intent: enhancement
feature_type: infrastructure
scope: standard
issue: 392
points: 4
priority: medium
tdd_mode: strict
estimated_effort: medium
estimated_days: 1.5
depends_on: []
bc_anchors: []
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f1-delta-analysis/issue-392/design.md"
implementation_strategy: tdd
module_criticality: LOW  # scripts/ tooling, no product source code
files_modified:
  - scripts/check-bc-cumulative-counts.sh
  - tests/spec-count-fixtures/run-tests.sh
  - tests/spec-count-fixtures/known-good/.factory/specs/prd/bc-1-auth-identity.md
  - tests/spec-count-fixtures/known-good/.factory/specs/prd/bc-2-issue-read.md
  - tests/spec-count-fixtures/known-good/.factory/specs/prd/BC-INDEX.md
  - tests/spec-count-fixtures/known-good/.factory/specs/prd/CANONICAL-COUNTS.md
  - tests/spec-count-fixtures/bc-drift-total/.factory/specs/prd/
  - tests/spec-count-fixtures/bc-drift-prose/.factory/specs/prd/
  - tests/spec-count-fixtures/bc-drift-grandtotal/.factory/specs/prd/
  - .github/workflows/ci.yml
  - CLAUDE.md
  - .factory/specs/prd/bc-2-issue-read.md
test_files:
  - tests/spec-count-fixtures/run-tests.sh
breaking_change: false
# BC status: N/A — CI tooling, no product behavioral contracts.
# This story implements process infrastructure; bcs field is intentionally empty.
---

# S-392 — Cumulative BC-Count CI Guard (DRIFT-002)

## Source of Truth

F1 design: `.factory/phase-f1-delta-analysis/issue-392/design.md` (all Q1-Q6 answered).
Existing guard for pattern reference: `scripts/check-spec-counts.sh` (DRIFT-001 sibling).
Original scoping: `.factory/research/issue-383-deferred-followups-validation.md` (Candidate 3).

## Problem Statement

`scripts/check-spec-counts.sh` guards per-file `definitional_count` (actual `#### BC-`
headings), `total_nfrs`, and `total_holdouts`. It does NOT guard the cumulative `total_bcs`
frontmatter, the grand-total sum across files, or the `BC-INDEX.md` Section-N header counts.
These same numbers are restated across 6+ locations with no machine check that they agree.

A live drift exists today (DRIFT-BC2-PROSE in STATE.md): `bc-2-issue-read.md` frontmatter
says `total_bcs: 93` (line 4) but its body preamble (line 17) still reads "92 behavioral
contracts". The +1 BC-2.6.051 added 2026-05-14 bumped the frontmatter but not the prose.
This story adds a sibling script `check-bc-cumulative-counts.sh` (DRIFT-002 mitigation)
that catches this class, plus corrects the bc-2 prose drift so the guard exits 0 on first run.

## Behavioral Contracts

N/A — CI tooling, no product behavioral contracts. This story implements process
infrastructure (scripts, fixtures, CI wiring) and a one-line spec prose correction.

## Must-Agree Invariant Set (from design Q2)

The new guard validates 7 surfaces across 4 categories:

**Per-file surfaces (for each bounded context N):**
- Surface A: `bc-N.md` frontmatter `total_bcs:` value
- Surface B: `BC-INDEX.md` `## Section N:` header cumulative count (e.g., `— 93 BCs cumulative`)
- Surface C: `BC-INDEX.md` frontmatter `sections:` line count (e.g., `(93 BCs cumulative; ...)`)
- Surface D: `CANONICAL-COUNTS.md` per-file `total_bcs` table row

**Per-file prose surface:**
- Body preamble "N behavioral contracts" prose vs Surface A (catches the live bc-2 drift)

**Grand-total surfaces (must equal `sum(A[1..8])`):**
- Surface E: `BC-INDEX.md` frontmatter `total_bcs:`
- Surface F: `CANONICAL-COUNTS.md` `**Sum**` bold row
- Surface G: `CANONICAL-COUNTS.md` `**Canonical grand total: N**` prose line

**Explicit carve-out (from design Q3):** The `### L2 domain-spec bc_count vs L3 total_bcs
alignment` table in CANONICAL-COUNTS.md is skipped entirely. It documents intentional,
tracked L2-vs-L3 divergence (PENDING rows). The guard uses `awk '/^### Per-file
total_bcs/,/^### Grand total/'` to extract only the invariant portion of
CANONICAL-COUNTS.md and never touches the L2-alignment section.

## Acceptance Criteria

- **AC-1** (guard script exists + validates 7+prose surfaces): `scripts/check-bc-cumulative-counts.sh`
  exists, is executable, and when run from repo root against the real `.factory/specs/prd/`
  directory validates all of the following for each bounded context N:
  - Surface A (`bc-N.md` frontmatter `total_bcs:`) equals Surface B (BC-INDEX.md Section N header)
  - Surface A equals Surface C (BC-INDEX.md frontmatter sections: line for that file)
  - Surface A equals Surface D (CANONICAL-COUNTS.md per-file table row)
  - Surface A equals the body-preamble prose count (`N behavioral contracts` line)
  - `sum(A[1..8])` equals Surface E (BC-INDEX.md frontmatter `total_bcs:`)
  - `sum(A[1..8])` equals Surface F (CANONICAL-COUNTS.md `**Sum**` row)
  - `sum(A[1..8])` equals Surface G (CANONICAL-COUNTS.md grand-total prose line)
  The script exits 0 on agreement, exits 1 with specific error messages on any mismatch.

- **AC-2** (PENDING carve-out verified by fixture): The guard skips the
  `### L2 domain-spec bc_count vs L3 total_bcs alignment` table in CANONICAL-COUNTS.md
  entirely. A `bc-drift-total` fixture where a PENDING-style row is intentionally mismatched
  in the L2-alignment section does NOT cause the guard to exit 1 — it exits 0 because the
  per-file `total_bcs` table is consistent. (This fixture variant can be the `bc-drift-total`
  fixture used for AC-3 — the key assertion is that only the L3-internal consistency matters.)

- **AC-3** (fixture suite — 4 mini trees): `tests/spec-count-fixtures/` contains a
  `run-tests.sh` harness and exactly 4 fixture trees:
  - `known-good/` — all surfaces consistent; guard exits 0 (labeled "known-good exits 0")
  - `bc-drift-total/` — `bc-N.md` `total_bcs:` differs from BC-INDEX Section header; guard
    exits 1 (labeled "total_bcs drift exits 1")
  - `bc-drift-prose/` — body preamble "N behavioral contracts" differs from `total_bcs:`
    frontmatter; guard exits 1 (labeled "prose count drift exits 1")
  - `bc-drift-grandtotal/` — CANONICAL-COUNTS.md `**Sum**` row disagrees with sum of per-file
    values; guard exits 1 (labeled "grand-total drift exits 1")
  The `run-tests.sh` runner exits 0 only if all 4 fixture cases yield their expected exit code;
  it prints `PASS:` / `FAIL:` per case and a summary `Results: N passed, N failed`.

- **AC-4** (failure messages identify surface + file + expected-vs-actual): When the guard
  exits 1, each error line matches the format from design Q6:
  ```
  ERROR: bc-2-issue-read.md: total_bcs frontmatter=93 but BC-INDEX.md Section 2 header=92
  ERROR: bc-2-issue-read.md: total_bcs frontmatter=93 but body prose="92 behavioral contracts"
  ERROR: bc-2-issue-read.md: total_bcs frontmatter=93 but CANONICAL-COUNTS.md table row=92
  ERROR: BC-INDEX.md frontmatter total_bcs=568 but computed sum of per-file total_bcs=569
  ERROR: CANONICAL-COUNTS.md **Sum** row=568 but computed sum of per-file total_bcs=569
  ERROR: CANONICAL-COUNTS.md grand-total prose=568 but computed sum of per-file total_bcs=569
  FAIL: N spec cumulative count mismatch(es). Fix frontmatter, BC-INDEX.md, or
        CANONICAL-COUNTS.md before merging.
  ```
  On success: `OK: all cumulative BC counts verified (569 total across 8 files).`

- **AC-5** (bc-2-issue-read.md prose fix — DRIFT-BC2-PROSE): `.factory/specs/prd/bc-2-issue-read.md`
  line 17 is corrected from "92 behavioral contracts" to "93 behavioral contracts" so the guard
  exits 0 against the real repo. This is a one-line prose fix; no frontmatter changes needed
  (`total_bcs: 93` on line 4 is already correct).

- **AC-6** (CI wiring — 2 new steps in spec-guard job): `.github/workflows/ci.yml` has 2 new
  steps added to the existing `spec-guard` job, in this order: first the fixture self-test step,
  then the real-repo DRIFT-002 step:
  ```yaml
  - name: check-bc-cumulative-counts self-test (fixture suite)
    run: bash tests/spec-count-fixtures/run-tests.sh
  - name: check-bc-cumulative-counts (DRIFT-002)
    run: bash scripts/check-bc-cumulative-counts.sh
  ```
  The fixture self-test runs BEFORE the real-repo check so a guard bug (false positive)
  surfaces first at the self-test step, not by blocking on real spec files.

- **AC-7** (guard exits 0 against real repo at story-completion state): After AC-5 (the
  bc-2 prose fix) is applied, running `bash scripts/check-bc-cumulative-counts.sh` from
  repo root against the real `.factory/specs/prd/` directory exits 0 with the success message
  `OK: all cumulative BC counts verified (569 total across 8 files).` This is the regression-
  proof acceptance criterion: all 7 surfaces + prose counts currently agree after the bc-2
  fix, and no other live drift exists that would block the guard's first CI run.

- **AC-8** (CLAUDE.md updated with DRIFT-002 reference): `CLAUDE.md` AI Agent Notes section
  references the new guard script parallel to the existing DRIFT-001 mention:
  > "Run `scripts/check-bc-cumulative-counts.sh` after any edit to BC-INDEX.md, CANONICAL-COUNTS.md,
  > or `total_bcs:` frontmatter values. Exits 0 if all 7 cumulative surfaces agree.
  > Exits 1 with specific mismatch details if drift is detected (DRIFT-002 mitigation)."

## Live Drift Scan

Before the guard is wired to CI, all 7 surfaces must agree against the real repo (post
bc-2 prose fix). Surfaces verified from design + file reads at story-creation time (2026-05-19):

| Surface | Current value | Notes |
|---------|---------------|-------|
| `bc-2-issue-read.md` frontmatter `total_bcs:` | 93 | Correct |
| `bc-2-issue-read.md` body prose | "92 behavioral contracts" | **DRIFT — AC-5 fixes this** |
| BC-INDEX.md Section 2 header count | 93 | Correct (pending verification) |
| BC-INDEX.md frontmatter sections: bc-2 line | 93 | Correct (pending verification) |
| CANONICAL-COUNTS.md bc-2 table row | 93 | Correct (line 44) |
| CANONICAL-COUNTS.md **Sum** row | 569 | Correct (line 51) |
| BC-INDEX.md frontmatter `total_bcs:` | 569 | Correct (pending verification) |

The implementer MUST run `bash scripts/check-bc-cumulative-counts.sh` after applying the
bc-2 prose fix and confirm exit 0 before opening the PR. If additional drifts surface
at that point, fix them within scope of this story before pushing.

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~3 k |
| design.md (F1, primary input for script logic) | ~5 k |
| `scripts/check-spec-counts.sh` (pattern reference) | ~1 k |
| `bc-2-issue-read.md` (read prose line to fix) | ~1 k |
| BC-INDEX.md (read to understand Section/frontmatter format) | ~8 k |
| CANONICAL-COUNTS.md (read to understand table format) | ~3 k |
| `.github/workflows/ci.yml` (read existing spec-guard job, identify insertion point) | ~4 k |
| `CLAUDE.md` (read existing DRIFT-001 mention, identify insertion point) | ~5 k |
| Fixture mini-files (4 trees × 4 files, ~20-30 lines each) | ~3 k |
| `scripts/check-bc-cumulative-counts.sh` (write, ~100-150 LOC bash) | ~4 k |
| `tests/spec-count-fixtures/run-tests.sh` (write, ~30 LOC) | ~1 k |
| Tool outputs (fixture run, real guard run) | ~2 k |
| **Total** | **~40 k** |

Well within single-agent context (Claude context window ~200k tokens). No split required.
LOC delta: ~120-150 lines bash (script) + ~30 lines (runner) + ~120 lines (fixtures,
4 trees × ~30 lines) + ~3 lines ci.yml + ~2 lines CLAUDE.md + 1 line bc-2 prose fix.

## Tasks

### Red Gate (TDD setup — fixtures before script)

- [ ] Read `scripts/check-spec-counts.sh` to understand the structural pattern (already done at story-creation)
- [ ] Read `bc-2-issue-read.md` lines 1-20 to confirm the prose fix target (line 17: "92 behavioral contracts")
- [ ] Read `BC-INDEX.md` lines 1-30 (frontmatter + first Section header) to understand format
- [ ] Read `CANONICAL-COUNTS.md` lines 39-60 (per-file table + grand-total section) to understand format
- [ ] Create fixture tree `tests/spec-count-fixtures/known-good/.factory/specs/prd/` with 4 minimal files:
  - `bc-1-auth-identity.md` — frontmatter `total_bcs: 10`, body prose "10 behavioral contracts", no BC-INDEX or CANONICAL-COUNTS interaction needed for per-file check
  - `bc-2-issue-read.md` — frontmatter `total_bcs: 20`, body prose "20 behavioral contracts"
  - `BC-INDEX.md` — frontmatter `total_bcs: 30` (=10+20), `sections:` block with both files, two `## Section N:` headers claiming 10 and 20 cumulative
  - `CANONICAL-COUNTS.md` — per-file table rows (10, 20), `**Sum** | **30**`, grand-total prose `**Canonical grand total: 30**`; no L2-alignment table (or an intentionally-PENDING one to verify carve-out)
- [ ] Create `bc-drift-total/` fixture — identical to known-good except BC-INDEX.md Section 2 header reads `15 BCs cumulative` instead of 20
- [ ] Create `bc-drift-prose/` fixture — identical to known-good except `bc-2-issue-read.md` body reads "15 behavioral contracts" instead of 20
- [ ] Create `bc-drift-grandtotal/` fixture — identical to known-good except CANONICAL-COUNTS.md `**Sum** | **25**` (instead of 30)
- [ ] Write `tests/spec-count-fixtures/run-tests.sh` runner (exact structure from design Q4)
- [ ] Run `bash tests/spec-count-fixtures/run-tests.sh` — expect ALL 4 tests to FAIL (Red Gate: script does not exist yet)
- [ ] Confirm Red Gate is red: runner exits 1, all 4 fixture cases fail with "no such file" or similar

### Implementation

- [ ] Read `.github/workflows/ci.yml` — locate the `spec-guard` job, identify line after the existing `check-bc-no-numeric-test-counts` step where the 2 new steps will be inserted
- [ ] Write `scripts/check-bc-cumulative-counts.sh` per design Q2 parse rules:
  - Repo-root guard: `[ -d ".factory" ] || { echo "ERROR: Run from repo root"; exit 1; }`
  - Parse each `bc-*.md` and `cross-cutting.md` for Surface A (`grep '^total_bcs:'`)
  - Parse BC-INDEX.md Section headers for Surface B (`grep '^## Section'` + sed extract)
  - Parse BC-INDEX.md frontmatter sections: block for Surface C (grep by filename + sed)
  - Parse CANONICAL-COUNTS.md per-file table for Surface D using `awk '/^### Per-file total_bcs/,/^### Grand total/'` carve-out
  - Parse body-preamble prose for each file (`grep -m1 'behavioral contracts'` + sed)
  - Compare A to B, C, D, prose per file; emit `ERROR:` lines per Q6 format on mismatch
  - Compute `sum(A[1..8])`; compare to Surface E, F, G; emit `ERROR:` lines on mismatch
  - Exit 1 + `FAIL: N mismatch(es)...` if any errors; exit 0 + `OK: all cumulative BC counts verified (N total across 8 files).` on success
- [ ] `chmod +x scripts/check-bc-cumulative-counts.sh`
- [ ] Run `bash tests/spec-count-fixtures/run-tests.sh` — expect all 4 PASS (Green Gate)
- [ ] Apply the bc-2 prose fix: change line 17 of `.factory/specs/prd/bc-2-issue-read.md` from "92 behavioral contracts" to "93 behavioral contracts"
- [ ] Run `bash scripts/check-bc-cumulative-counts.sh` from repo root — expect exit 0 (AC-7 verified)
  - If exit 1: read the error messages, identify any additional live drifts, fix them within scope before proceeding
- [ ] Update `.github/workflows/ci.yml`: add the 2 new steps after the `check-bc-no-numeric-test-counts (PG-365-1)` step per design Q5 YAML
- [ ] Update `CLAUDE.md` AI Agent Notes: add the DRIFT-002 note referencing `check-bc-cumulative-counts.sh` (AC-8), parallel to the existing DRIFT-001 line
- [ ] Run `bash scripts/check-spec-counts.sh` — confirm DRIFT-001 guard still exits 0 (no regression)

### Verification

- [ ] Run `bash tests/spec-count-fixtures/run-tests.sh` — confirm `Results: 4 passed, 0 failed`
- [ ] Run `bash scripts/check-bc-cumulative-counts.sh` — confirm `OK: all cumulative BC counts verified (569 total across 8 files).`
- [ ] Run `bash scripts/check-spec-counts.sh` — confirm still exits 0 (DRIFT-001 guard unbroken)
- [ ] Run `bash scripts/check-bc-no-numeric-test-counts.sh` — confirm still exits 0 (PG-365-1 guard unbroken)
- [ ] Per-story adversary 3/3 CLEAN before push

## Previous Story Intelligence

This story is the direct follow-up to S-3.06 (which added `check-spec-counts.sh`, DRIFT-001)
and S-383 (which surfaced DEFER-383-3, the original scoping for this work). Key lessons:

- **From S-3.06 (check-spec-counts.sh delivery):** The POSIX-portable bash pattern (`#!/usr/bin/env bash`, `set -euo pipefail`, `grep ... | awk '{print $2}'`) is the established project style for spec-guard scripts. Mirror this pattern exactly. The repo-root guard (`[ -d ".factory" ] || exit 1`) must be first.

- **From S-3.06:** The existing guard has no fixture-based self-test. S-392 introduces fixtures first — this is the correct TDD order. Write fixtures, confirm they fail (Red Gate), THEN write the script. The fixture runner (`run-tests.sh`) must itself be runnable as `bash tests/spec-count-fixtures/run-tests.sh` from repo root without any additional setup.

- **From S-383 / issue-383-deferred-followups-validation.md:** The PENDING carve-out is the single highest-risk aspect of the new guard. A naive grep over CANONICAL-COUNTS.md would false-positive on the L2-vs-L3 alignment table (lines 74-82 of the current file) which documents intentional `bc-02` and `bc-03` divergence. The design Q3 `awk` range extraction (`/^### Per-file total_bcs/,/^### Grand total/`) is the safe implementation — it never touches the PENDING section. Do NOT implement the carve-out as a per-row `PENDING` substring check; the section-header approach is structurally more robust.

- **From issue-383 adversarial cycle:** Surface B parse correctness (BC-INDEX.md Section header integer extraction) was identified as the primary false-positive risk. The design specifies `sed 's/.*— \([0-9]*\) BCs cumulative.*/\1/'` — use this exactly. Test it against the `bc-drift-total` fixture before calling the implementation done.

- **Known safe shorthand:** the design Q2 specifies `cross-cutting.md` is one of the 8 files with `total_bcs:` frontmatter (surface A = 138). Treat it identically to `bc-N.md` files in the loop. The `for f in .factory/specs/prd/bc-*.md .factory/specs/prd/cross-cutting.md` loop from `check-spec-counts.sh` is the right expansion.

## Architecture Compliance Rules

- Script lives at `scripts/check-bc-cumulative-counts.sh` (sibling to `check-spec-counts.sh` and `check-bc-no-numeric-test-counts.sh` per design Q1 and established convention).
- Fixtures live at `tests/spec-count-fixtures/` (design Q4 recommendation; no Rust test runner wrapping needed — the runner is a standalone bash script).
- The script must be POSIX-portable bash (`#!/usr/bin/env bash`, `set -euo pipefail`). No bash 4+ associative arrays; use simple shell loops and grep/awk/sed.
- No new CI dependencies (no bats, no external tools). The fixture runner requires only bash and the script itself.
- The script references `.factory/specs/prd/` as the canonical data directory — same as `check-spec-counts.sh`. It MUST be run from repo root; the repo-root guard enforces this.
- Two new CI steps are added to the EXISTING `spec-guard` job — NOT a new job. The fixture self-test step runs first, the real-repo step runs second (design Q5).
- The spec-guard job already has `timeout-minutes: 5` and the `.factory` worktree checked out; no additional setup needed.
- Do NOT modify `check-spec-counts.sh` — this is a sibling, not an extension. DRIFT-001 and DRIFT-002 are intentionally isolated (design Q1 rationale: blast-radius isolation).

## Library & Framework Requirements

- No new dependencies. Bash, grep, awk, sed — all stdlib on ubuntu-latest.
- No Rust changes; no `Cargo.toml` changes; no `Cargo.lock` changes.
- The fixture runner (`run-tests.sh`) uses only POSIX shell built-ins.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `scripts/check-bc-cumulative-counts.sh` | CREATE | ~120-150 LOC bash; chmod +x required |
| `tests/spec-count-fixtures/run-tests.sh` | CREATE | ~30 LOC bash runner; chmod +x required |
| `tests/spec-count-fixtures/known-good/.factory/specs/prd/bc-1-auth-identity.md` | CREATE | Minimal fixture: frontmatter + prose |
| `tests/spec-count-fixtures/known-good/.factory/specs/prd/bc-2-issue-read.md` | CREATE | Minimal fixture: frontmatter + prose |
| `tests/spec-count-fixtures/known-good/.factory/specs/prd/BC-INDEX.md` | CREATE | Minimal fixture: frontmatter + Section headers |
| `tests/spec-count-fixtures/known-good/.factory/specs/prd/CANONICAL-COUNTS.md` | CREATE | Minimal fixture: per-file table + Sum + grand-total prose |
| `tests/spec-count-fixtures/bc-drift-total/.factory/specs/prd/` | CREATE | 4 files, drifted BC-INDEX Section header |
| `tests/spec-count-fixtures/bc-drift-prose/.factory/specs/prd/` | CREATE | 4 files, drifted body prose count |
| `tests/spec-count-fixtures/bc-drift-grandtotal/.factory/specs/prd/` | CREATE | 4 files, drifted CANONICAL-COUNTS Sum row |
| `.github/workflows/ci.yml` | MODIFY | +2 steps to spec-guard job after PG-365-1 step |
| `CLAUDE.md` | MODIFY | +2 lines in AI Agent Notes (DRIFT-002 reference) |
| `.factory/specs/prd/bc-2-issue-read.md` | MODIFY | 1-line prose fix: "92" → "93 behavioral contracts" |
| `.factory/stories/STORY-INDEX.md` | MODIFY | Register S-392 in Feature Followup table + Story Manifest |
| `.factory/sprint-state.yaml` | MODIFY | Add S-392 entry under `feature_followup_standalone` |

## Branch / PR Plan

- Branch: `ci/issue-392-cumulative-bc-count-guard`
- Target: `develop`
- Commit style: `ci(spec-guard): add check-bc-cumulative-counts.sh + fixture suite + bc-2 prose fix (DRIFT-002) (#392)`
- PR closes #392

## Per-Story Delivery Notes

- Demos (Step 5) are LOCAL-ONLY per `docs/demo-evidence/` gitignore convention.
- Per-story adversary 3/3 CLEAN required before push.
- The `.factory/specs/prd/bc-2-issue-read.md` fix is a factory-artifacts branch file, not a develop branch file. The implementer must check out or worktree the factory-artifacts branch to apply the 1-line fix, OR coordinate with the orchestrator to apply it on factory-artifacts directly. If the file is in a git worktree (as per the CI checkout), the fix must go to factory-artifacts; if working locally with the `.factory/` dir present in the working tree, apply and commit directly.
- After applying the bc-2 fix, immediately run `bash scripts/check-bc-cumulative-counts.sh` to confirm exit 0 before writing more code. If the guard reveals additional drifts, fix them in this story before pushing.
- The fixture files are tiny (20-40 lines each). Produce them all before writing the guard script (Red Gate discipline).
