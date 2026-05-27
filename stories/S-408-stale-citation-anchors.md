---
document_type: story
story_id: "S-408"
title: "Fix 5 stale line-anchor citations in CLAUDE.md + bc-3-issue-write.md; adopt symbol-form going forward (refs #408)"
wave: feature-followup
status: ready
intent: bug-fix
feature_type: documentation
scope: trivial
severity: low
trivial_scope: true
issue: 408
points: 1
priority: low
tdd_mode: strict
estimated_effort: xsmall
depends_on: []
bc_anchors: []
# No BC anchor — documentation-only change. No production code, no behavioral contract modification.
# No new BCs. No new VPs. No new ADR.
# Status=ready is set because no BC authorship is required: this story changes only spec prose
# and CLAUDE.md. The gate criterion (behavioral_contracts must be non-empty before ready)
# does not apply to documentation-only stories per the spec-first gate commentary — the
# empty bc_anchors is intentional and explicitly justified here.
verification_properties: []
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: "github.com/Zious11/jira-cli/issues/408"
implementation_strategy: tdd
module_criticality: LOW
files_modified:
  - CLAUDE.md                                    # MODIFIED — re-anchor 2 stale citations (AC-001, AC-002); add AI Agent Notes bullet for going-forward convention (AC-006)
  - .factory/specs/prd/bc-3-issue-write.md       # MODIFIED — re-anchor 3 stale citations (AC-003, AC-004, AC-005); lands via state-manager direct commit on factory-artifacts branch
files_created: []
breaking_change: false
assumption_validations: []
risk_mitigations: []
---

# S-408 — Fix 5 Stale Line-Anchor Citations; Adopt Symbol-Form Convention

## Source of Truth

GitHub issue: https://github.com/Zious11/jira-cli/issues/408

**No new BCs. No new VPs. No new ADR. No production code changes.**

## Goal

Re-anchor 5 known stale line-number citations to symbol-form references so they survive
future refactors, and codify a going-forward convention in CLAUDE.md so the same class of
drift does not recur silently.

## Problem Statement

Five citations in two files use bare line numbers that have already drifted from the actual
code they describe:

| Location | Stale citation | Actual target |
|----------|---------------|---------------|
| `CLAUDE.md` line 334 | `create.rs:~445 (the --label mutual-exclusion block)` | `create.rs::handle_edit` — `--label` mutual-exclusion block (function-scope anchor, no stable line) |
| `CLAUDE.md` line 336 | `create.rs:~835` | `create.rs::handle_edit § "Route: labels → bulk API"` (comment at actual line 842) |
| `bc-3-issue-write.md` line 982 | `create.rs:1982-1997` | `create.rs::parse_field_kv` (function now at line 2245) |
| `bc-3-issue-write.md` line 1511 | `create.rs:1982-1997` (duplicate) | same |
| `bc-3-issue-write.md` line 1535 | `create.rs:~835` | `create.rs::handle_edit § "Route: labels → bulk API"` |

Issue #408 proposed three remediation strategies. The chosen strategy is **option 1:
targeted fix + symbol-form convention going forward** — no mass sweep of existing
citations, no CI guard script (deferred per issue discussion).

## Behavioral Contracts

No BC anchor — documentation-only. No user-visible behavior changes.

## Acceptance Criteria

### AC-001 — CLAUDE.md: re-anchor `create.rs:~445` citation

The text `create.rs:~445 (the --label mutual-exclusion block)` in `CLAUDE.md`
(currently around line 334, inside the `--field` constraints bullet) is replaced with a
symbol-form reference: `create.rs::handle_edit` (namespace anchor) plus an inline
description of the guard. The tilde-line-number form is removed entirely. The sentence
meaning and context are preserved.

Verification: `grep -n "create.rs:~445\|create.rs:445-489" CLAUDE.md` returns ZERO
matches.

### AC-002 — CLAUDE.md: re-anchor `create.rs:~835` citation

The text `create.rs:~835` in `CLAUDE.md` (currently around line 336, same bullet) is
replaced with the symbol-form reference `create.rs::handle_edit § "Route: labels →
bulk API"` (the section comment at the actual routing fork). The tilde-line-number form
is removed. Sentence meaning preserved.

Verification: `grep -n "create.rs:~835" CLAUDE.md` returns ZERO matches.

### AC-003 — bc-3-issue-write.md: re-anchor `create.rs:1982-1997` at line 982

The text `create.rs:1982-1997` in `.factory/specs/prd/bc-3-issue-write.md` (around
line 982) is replaced with `create.rs::parse_field_kv`. The function name is the stable
anchor; the exact line is dropped.

Verification: `grep -n "create.rs:1982-1997" .factory/specs/prd/bc-3-issue-write.md`
returns ZERO matches after both AC-003 and AC-004 are applied.

### AC-004 — bc-3-issue-write.md: re-anchor `create.rs:1982-1997` at line 1511

The second occurrence of `create.rs:1982-1997` in `bc-3-issue-write.md` (around line
1511) is replaced with `create.rs::parse_field_kv`. Same replacement as AC-003.

### AC-005 — bc-3-issue-write.md: re-anchor `create.rs:~835` at line 1535

The text `create.rs:~835` in `bc-3-issue-write.md` (around line 1535) is replaced with
`create.rs::handle_edit § "Route: labels → bulk API"` (consistent with AC-002).

Verification: `grep -n "create.rs:~835" .factory/specs/prd/bc-3-issue-write.md` returns
ZERO matches.

### AC-006 — CLAUDE.md: going-forward citation convention documented

A new bullet is added to the **AI Agent Notes** section of `CLAUDE.md` documenting the
citation convention:

> **Citation anchor discipline for source references in CLAUDE.md and spec files:**
> prefer symbol-form (`<file>::<function>` or `<file>::<function> § "<comment>"`) over
> bare line numbers. Use `<file>:~NN` only as a last resort for callsites that have no
> stable function or comment anchor. Clean up stale bare citations opportunistically when
> touching a file — no sweep PR required.

The bullet must be positioned in or adjacent to the existing "Citation discipline"
cluster in AI Agent Notes (near the `JRACLOUD-*` citation discipline bullet), not in an
unrelated section.

### AC-007 — Convergence grep exits ZERO

After both file halves land (CLAUDE.md via PR, bc-3-issue-write.md via state-manager
direct commit), the following grep returns ZERO matches across both files:

```
grep -n "create.rs:~445\|create.rs:~835\|create.rs:445-489\|create.rs:1982-1997" \
  CLAUDE.md .factory/specs/prd/bc-3-issue-write.md
```

This is the definitive convergence check for this story. Run it before marking the story
as done.

## Implementation Strategy

This story requires NO production code changes. Both files contain only prose updates.

**Ordered sequence:**

1. **Create branch** `fix/S-408-stale-citation-anchors` from `develop`.

2. **Read `CLAUDE.md`** around lines 330-340 (the `--field` constraints bullet in the
   Gotchas section) to locate the two stale citations. Verify exact surrounding text
   before editing.

3. **Read `src/cli/issue/create.rs`** to confirm the current location of:
   - The `--label` mutual-exclusion block (was line 445; find its actual line)
   - The `handle_edit` routing fork for labels (was line 835/842; find its actual line)
   - `parse_field_kv` function definition (was line 1982; find its actual line)
   This read is required — do NOT use the stale line numbers from the citations as ground
   truth for writing the replacement text.

4. **Edit `CLAUDE.md`:**
   - Replace the `create.rs:~445 (the --label mutual-exclusion block)` fragment (AC-001)
   - Replace the `create.rs:~835` fragment in the same bullet (AC-002)
   - Add the going-forward convention bullet in AI Agent Notes (AC-006)

5. **Edit `.factory/specs/prd/bc-3-issue-write.md`:**
   - Replace both `create.rs:1982-1997` occurrences (AC-003, AC-004)
   - Replace the `create.rs:~835` occurrence (AC-005)

6. **Run the AC-007 convergence grep** — must return ZERO matches.

7. **Run `bash scripts/check-spec-counts.sh`** — must exit 0 (no BC frontmatter changed).

8. **Run `bash scripts/check-bc-cumulative-counts.sh`** — must exit 0 (no count surfaces
   touched).

9. **Run `bash scripts/check-bc-no-numeric-test-counts.sh`** — must exit 0.

10. **Commit CLAUDE.md changes** on `fix/S-408-stale-citation-anchors` with:
    `docs: re-anchor 5 stale line-number citations to symbol-form (refs #408)`

11. **Open PR** targeting `develop`. The bc-3-issue-write.md change lands separately
    via state-manager direct commit to the factory-artifacts branch (not via the PR).

## Out of Scope

- **Mass sweep of the remaining ~822 line-anchor citations** in CLAUDE.md, bc-*.md, and
  other spec files. Deferred — convention adoption via AC-006 is the mechanism; no
  sweep PR is planned.
- **CI guard script** that enforces symbol-form at commit time (issue #408 option 2).
  Deferred as too high a setup cost for the benefit at this project scale.
- **Any production code changes.** `src/` is read-only for this story.
- **Adding or modifying BCs, VPs, or holdouts.** Documentation-only change; no spec
  count surfaces are touched.

## Test Coverage

N/A — documentation-only. The convergence check is AC-007 (grep returning ZERO matches),
run manually before marking the story done. No `cargo test` changes required or expected.

`cargo test` must still exit 0 after the edits (no test file is modified, so this is a
smoke confirmation that the Rust codebase is unaffected).

## Quality Gate Self-Check

| Criterion | Required | Notes |
|-----------|----------|-------|
| AC-007 convergence grep returns ZERO matches | MUST | Run across both files after edits |
| `cargo test` exits 0 | smoke | Rust codebase unaffected; quick sanity |
| `bash scripts/check-spec-counts.sh` exits 0 | invariant | No BC frontmatter touched |
| `bash scripts/check-bc-cumulative-counts.sh` exits 0 | invariant | No count surfaces touched |
| `bash scripts/check-bc-no-numeric-test-counts.sh` exits 0 | invariant | No BC bodies with numeric test counts changed |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~3 k |
| `CLAUDE.md` (relevant lines 320-360 + AI Agent Notes section) | ~4 k |
| `.factory/specs/prd/bc-3-issue-write.md` (lines ~975-995, ~1505-1545) | ~2 k |
| `src/cli/issue/create.rs` (scan for anchor functions — read excerpts only) | ~4 k |
| Tool outputs (grep AC-007, script exits) | ~1 k |
| **Total** | **~14 k** |

Well within a single-agent context window. No split required.
LOC delta: CLAUDE.md ~+5 LOC (net: 2 line replacements + new convention bullet);
bc-3-issue-write.md ~0 net LOC (3 in-place text substitutions, same line count).

## Tasks

- [ ] Create branch `fix/S-408-stale-citation-anchors` from `develop`
- [ ] Read `CLAUDE.md` lines 325-345 — locate exact text of both stale citations in Gotchas `--field` constraints bullet
- [ ] Read `src/cli/issue/create.rs` — find current location of `--label` mutual-exclusion block, `handle_edit` labels routing fork, and `parse_field_kv` function
- [ ] Edit `CLAUDE.md`: replace `create.rs:~445 (the --label mutual-exclusion block)` with symbol-form (AC-001)
- [ ] Edit `CLAUDE.md`: replace `create.rs:~835` tilde citation with symbol-form (AC-002)
- [ ] Edit `CLAUDE.md`: add going-forward citation convention bullet in AI Agent Notes (AC-006)
- [ ] Edit `.factory/specs/prd/bc-3-issue-write.md`: replace `create.rs:1982-1997` at line ~982 (AC-003)
- [ ] Edit `.factory/specs/prd/bc-3-issue-write.md`: replace `create.rs:1982-1997` at line ~1511 (AC-004)
- [ ] Edit `.factory/specs/prd/bc-3-issue-write.md`: replace `create.rs:~835` at line ~1535 (AC-005)
- [ ] Run AC-007 convergence grep — must return ZERO matches
- [ ] Run `bash scripts/check-spec-counts.sh` — exits 0
- [ ] Run `bash scripts/check-bc-cumulative-counts.sh` — exits 0
- [ ] Run `bash scripts/check-bc-no-numeric-test-counts.sh` — exits 0
- [ ] Run `cargo test` — exits 0 (smoke; no test files changed)
- [ ] Commit CLAUDE.md changes with `docs:` prefix; open PR targeting `develop`
- [ ] Coordinate with state-manager to commit bc-3-issue-write.md on factory-artifacts branch

## Previous Story Intelligence

No direct predecessor story. The closest structural precedent is S-2.05 (CLAUDE.md
documentation update), which established the pattern of CLAUDE.md edits landing via
PR and factory-artifact spec edits landing via state-manager direct commit to the
factory-artifacts branch.

Key lesson from S-407 (--label conflict block coverage): when referencing source file
line numbers in spec prose, always verify the current line number against the actual
file before writing the citation. Line numbers drift with every PR — a line number that
was correct at spec-authoring time may be wrong by implementation time.

Key lesson from issue #361 / PR #364 (JRACLOUD citation discipline): citations that
look plausible but are wrong create maintenance debt that survives multiple review
rounds. Symbol-form citations (`::function_name`) are self-describing and survive
refactors far better than `~NNN` tilde references.

## Architecture Compliance Rules

1. **No production code changes.** `src/` is read-only for this story. If a production
   change is needed, STOP and escalate.

2. **No BC count surface edits.** bc-3-issue-write.md prose edits must NOT touch any
   frontmatter count fields, BC header numbering, or body structure that `check-spec-counts.sh`
   or `check-bc-cumulative-counts.sh` validates. The edits are strictly in-line text
   replacements within existing BC body prose.

3. **Symbol-form anchor must be verified before writing.** Before replacing a stale
   citation, read the actual source file to confirm the function name exists and the
   comment phrase is accurate. Do not copy the suggested replacement from this story
   verbatim without verifying — function names can be renamed between story authoring
   and implementation.

4. **CLAUDE.md convention bullet placement.** The new AC-006 bullet must be inserted
   within the existing AI Agent Notes section, adjacent to the citation-discipline
   cluster. It must NOT be placed in the Gotchas section or the Build & Test section.

5. **No BC numeric test count additions.** The bc-3-issue-write.md edits must not
   introduce any numeric test counts (e.g., "tested by 3 tests") — enforced by
   `check-bc-no-numeric-test-counts.sh`.

## Library & Framework Requirements

No new dependencies. No version changes. Documentation-only change.

## File Structure Requirements

| File | Action | Branch | Notes |
|------|--------|--------|-------|
| `CLAUDE.md` | Modify | `fix/S-408-stale-citation-anchors` → PR → `develop` | Re-anchor 2 citations + add convention bullet (~+5 LOC) |
| `.factory/specs/prd/bc-3-issue-write.md` | Modify | `factory-artifacts` (state-manager direct commit) | Re-anchor 3 citations (~0 net LOC) |

**Files NOT to create:** No new source files, no new spec files, no new VP documents, no
new ADR, no new test files.

**Files NOT to touch:** All of `src/`, `Cargo.toml`, `deny.toml`, `STORY-INDEX.md`
(state-manager updates that), all BC count surfaces (frontmatter, BC-INDEX.md,
CANONICAL-COUNTS.md).

## Branch / PR Plan

- Branch: `fix/S-408-stale-citation-anchors`
- Target: `develop`
- Commit style: `docs: re-anchor 5 stale line-number citations to symbol-form (refs #408)`
- PR references: `Refs #408` (NOT `Closes #408` — the closer happens when state-manager
  confirms both halves have landed: the PR merge for CLAUDE.md and the direct commit for
  bc-3-issue-write.md)
- CHANGELOG entry: not required (documentation-only; no user-visible behavior change)
