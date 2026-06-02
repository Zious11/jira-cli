---
document_type: story
story_id: "S-E2E-FORK-1"
title: "Fork-safe E2E CI enablement gate + README E2E badge"
wave: feature-followup
status: delivered
intent: enhancement
feature_type: infrastructure
scope: trivial
severity: small
trivial_scope: false
issue: TBD
points: 3
priority: P2
tdd_mode: facade
estimated_effort: small
mode: feature
depends_on: []
blocks: []
bc_anchors: []
# BC delta: EMPTY — this story adds CI infrastructure (workflow gate + badge) and
# documentation fallout only. No product behavioral contracts are introduced or modified.
# The gate is a GitHub Actions YAML conditional; no Rust code changes.
# BC status: no BC authorship required.
# Status=draft: the spec-first gate (S-7.01) does not block dispatch for
# infrastructure-only stories with explicit zero-BC justification above.
bcs: []
verification_properties: []
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
parent_phase: F3-story-decomposition
spec_source: "docs/specs/e2e-fork-safe-ci-enablement.md"
implementation_strategy: tdd
module_criticality: LOW
acceptance_criteria_count: 7
created: "2026-06-01"
traceability_note: >
  BC delta is EMPTY. This story is pure CI/docs work: a job-level gate in e2e.yml,
  a preflight assertion step, a README badge, and documentation fallout in CLAUDE.md
  and docs/specs/e2e-live-jira-testing.md. All traces are to VER-E2E-FORK-1..4 from
  docs/specs/e2e-fork-safe-ci-enablement.md §7. No BC-S.SS.NNN clause is verified.
files_modified:
  - .github/workflows/e2e.yml     # MODIFIED — add JR_E2E_ENABLED gate + preflight step
  - README.md                     # MODIFIED — add E2E status badge (2nd badge in row)
  - CLAUDE.md                     # MODIFIED — add JR_E2E_ENABLED entry + two-layer model note
  - docs/specs/e2e-live-jira-testing.md  # MODIFIED — §5 YAML, §6 fork-gate para, §8 env-var row
breaking_change: false
assumption_validations: []
risk_mitigations: []
last_updated: "2026-06-01"
changelog:
  - date: "2026-06-01"
    phase: F3-story-decomposition
    author: story-writer
    summary: Initial story creation. Feature F3 for Fork-safe E2E CI Enablement.
---

# S-E2E-FORK-1 — Fork-safe E2E CI Enablement Gate + README E2E Badge

## Source of Truth

Design spec: `/Users/zious/Documents/GITHUB/jira-cli/docs/specs/e2e-fork-safe-ci-enablement.md`
Sections: §2 (Design) — §2.1 Two-Layer Model, §2.2 Gate Expression, §2.3 Repo Variable,
          §2.4 Preflight Step, §2.5 README Badge; §6 CLAUDE.md doc-fallout; §9 F4 touch-point list.
F1 delta analysis: `.factory/phase-f1-delta-analysis/e2e-fork-safe-enablement/delta-analysis.md`
Locked decisions: LOCK-A (repo variable job-level gate), LOCK-B (badge scoped to develop nightly).

**No new BCs. Changes touch `.github/workflows/e2e.yml`, `README.md`, `CLAUDE.md`, and
`docs/specs/e2e-live-jira-testing.md`. Zero `src/` changes. Zero `tests/` changes.**

## Story Narrative

As a fork contributor,
I want the `e2e:` CI job to skip silently when I push to my fork (which has no `jira-e2e` secrets),
so that I do not see red CI runs I cannot fix or suppress.

As the canonical repo maintainer,
I want the `e2e:` CI job to continue running normally after I create a `JR_E2E_ENABLED=true`
repository variable, and I want the README to display an E2E status badge reflecting nightly health.

## Dependency Justification

**S-E2E-FORK-1 has `depends_on: []`** — it is entirely standalone CI infrastructure.
All four files it modifies (e2e.yml, README.md, CLAUDE.md, e2e-live-jira-testing.md) are
independent of any in-progress story. No ordering dependency with S-E2E-1 through S-E2E-5.

**`blocks: []`** — no current story depends on the fork-gate being present to proceed.
The gate is additive CI infrastructure; it does not alter any test-binary behavior.

## Goal

1. Add a single job-level gate to `e2e.yml` so the `e2e:` job is skipped on forks lacking
   the `JR_E2E_ENABLED` repository variable.
2. Add a preflight step inside the now-gated job that fails fast with clear per-variable
   messages if required config is missing.
3. Add an E2E status badge to `README.md` (2nd badge in the badge row, after `[![CI]...]`).
4. Document the two-layer model (`JR_E2E_ENABLED` CI gate vs `JR_RUN_E2E` test-binary gate)
   in `CLAUDE.md` and update `docs/specs/e2e-live-jira-testing.md` accordingly.
5. Document the canonical-repo rollout step: maintainer must create a repository variable
   `JR_E2E_ENABLED=true` (else E2E skips on canonical after merge).

## Traceability

| Traceability target | Type | Description |
|--------------------|------|-------------|
| VER-E2E-FORK-1 | Verification property | Fork/variable-unset → job SKIPPED (not failed) |
| VER-E2E-FORK-2 | Verification property | Canonical repo with `JR_E2E_ENABLED=true` → job runs normally |
| VER-E2E-FORK-3 | Verification property | Skipped run produces green "passing" badge |
| VER-E2E-FORK-4 | Verification property | Preflight fails loud on missing required config |
| Design spec §2.1 | Architecture | Two-layer model definition |
| Design spec §2.2 | Gate expression | `if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'` |
| Design spec §2.3 | Constraint | `JR_E2E_ENABLED` MUST be repository variable, not environment-scoped |
| Design spec §2.4 | Preflight | `${VAR:?message}` pattern; required vars BASE_URL, PROJECT, API_TOKEN, EMAIL |
| Design spec §2.5 | Badge | Exact markdown normative; 2nd badge in badge row |
| Design spec §6 | Doc-fallout | CLAUDE.md `JR_*` table + two-layer note |

## Behavioral Contracts

None — pure CI/docs. BC delta is EMPTY (confirmed in F1 delta analysis and F2 spec).

## Acceptance Criteria

### AC-001 — Job-level gate expression is correct (VER-E2E-FORK-1, VER-E2E-FORK-2)

`.github/workflows/e2e.yml` job `e2e:` declares:

```yaml
if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'
```

**Verbatim match required.** The existing single condition
`if: github.event_name != 'pull_request'` is replaced by this combined expression.
No `>-` block-scalar form; single-line form per spec §2.2.

**Verification:**
```
grep -n "vars.JR_E2E_ENABLED" .github/workflows/e2e.yml
```
Produces exactly one match that contains the full expression on a single line.

**Fork behavior:** a fork without `JR_E2E_ENABLED` set receives empty string from
`vars.JR_E2E_ENABLED`; empty string `!= 'true'` → condition false → job SKIPPED, not failed.
The `environment: jira-e2e` declaration on the job is RETAINED — it continues to supply
secrets and environment-scoped variables after the gate passes. (spec §2.3)

### AC-002 — Fork / unset-variable → job SKIPPED (VER-E2E-FORK-1)

On any repository (fork or otherwise) where `JR_E2E_ENABLED` is not set as a repository
variable, the `e2e:` job conclusion is `skipped` and the workflow run conclusion is `success`.
This is verified by:

1. Reading the updated `e2e.yml` and confirming AC-001's gate expression matches verbatim.
2. (F6 empirical) Inspecting a CI run on develop immediately after the PR is merged but
   before the maintainer creates the repository variable; confirming the job shows as
   "skipped" in the GitHub Actions run summary — not "failed".

**Expected outcome:** job conclusion = skipped; workflow run conclusion = success (green).

### AC-003 — Canonical repo with `JR_E2E_ENABLED=true` → job runs normally (VER-E2E-FORK-2)

After merging the F4 PR and creating a **repository variable** (NOT an environment variable)
named `JR_E2E_ENABLED` with value `true` in the canonical repo settings
(`https://github.com/Zious11/jira-cli/settings/variables/actions`):

- The `e2e:` job starts normally on the next `push`, `schedule`, or `workflow_dispatch` run.
- The `environment: jira-e2e` declaration is retained; secrets and environment-scoped variables
  load as before after the gate passes.
- The preflight step (AC-004) runs and passes.
- The Rust build and live E2E tests execute.

The story MUST document the rollout step in the rollout note (below): until the maintainer
creates `JR_E2E_ENABLED=true` as a repository variable, the `e2e:` job will be skipped even
on the canonical repo. This is expected and intentional — the variable is the explicit opt-in.

**Verification (F6):** trigger a `workflow_dispatch` run on `develop` after creating the
repository variable; confirm the job runs (not skipped).

### AC-004 — Preflight step present and correct (VER-E2E-FORK-4)

A preflight step is added as the **first step** of the `e2e:` job (after `Harden the runner`
if that step is present). It uses the `${VAR:?message}` bash idiom to assert all required
E2E config is present before any Rust build begins.

**Required assertions (spec §2.4):**

```bash
: ${JR_E2E_BASE_URL:?"JR_E2E_BASE_URL is required — set it as a secret in the jira-e2e GitHub Environment"}
: ${JR_E2E_PROJECT:?"JR_E2E_PROJECT is required — set it as a variable in the jira-e2e GitHub Environment"}
: ${JR_E2E_API_TOKEN:?"JR_E2E_API_TOKEN is required — set it as a secret in the jira-e2e GitHub Environment"}
: ${JR_E2E_EMAIL:?"JR_E2E_EMAIL is required — set it as a secret in the jira-e2e GitHub Environment"}
```

The step's `env:` block MUST reference these variables from their respective secret/variable
sources (e.g., `JR_E2E_BASE_URL: ${{ secrets.JR_E2E_BASE_URL }}`, etc.).

**Verification:**
```
grep -n "JR_E2E_BASE_URL\|JR_E2E_PROJECT\|JR_E2E_API_TOKEN\|JR_E2E_EMAIL" .github/workflows/e2e.yml
```
Produces ≥ 4 matches in a preflight step block.

**F6 empirical test:** in a `workflow_dispatch` run where one required secret is temporarily
renamed/absent, the preflight step fails with the `${VAR:?message}` error visible in the
step log; the job does not proceed to the Rust build.

### AC-005 — README E2E badge added in correct position (VER-E2E-FORK-3)

`README.md` contains the following markdown **verbatim** as the second badge in the badge row
(after the existing `[![CI]...]` badge, before any release or other badge):

```markdown
[![E2E](https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml/badge.svg?branch=develop)](https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml)
```

**Verification:**
```
grep -n "e2e.yml/badge.svg" README.md
```
Produces exactly one match with the exact URL above.

**Badge truthfulness:** the badge URL is hardcoded to the canonical repo `Zious11/jira-cli`.
When the `e2e:` job is skipped (fork or pre-variable-creation), the workflow run concludes
`success` → badge renders green "passing" (documented GitHub Actions behavior: all-skipped
workflow = success conclusion). On the canonical repo with `JR_E2E_ENABLED=true`, green =
tests passed; red = tests failed. The badge is truthful for the canonical README.

### AC-006 — CLAUDE.md and runbook updated (doc-fallout, spec §6 + §3)

**CLAUDE.md updates (AI Agent Notes E2E section):**

1. A new `JR_E2E_ENABLED` entry is added to the `JR_*` env-var table, containing:
   - Identifies it as a GitHub Actions **repository variable** (`vars.JR_E2E_ENABLED`)
   - States it gates the `e2e:` job at scheduling time
   - States it is NOT a Rust env var; never read by `src/` code
   - Notes forks with this variable unset skip cleanly
   - Notes the canonical repo sets `JR_E2E_ENABLED=true` as a repository variable
     in GitHub repo settings (not environment-scoped)
   - References `docs/specs/e2e-fork-safe-ci-enablement.md §2.3`

2. A two-layer model note is added near the `JR_RUN_E2E` entry explaining:
   - `JR_E2E_ENABLED` (workflow level, repo variable) determines whether the CI job starts
   - `JR_RUN_E2E` (test-binary level, process env) determines whether `#[ignore]` tests run
   - The workflow sets `JR_RUN_E2E=1` only when the job starts
   - Each layer independently provides fail-safe behavior

**`docs/specs/e2e-live-jira-testing.md` updates (spec §3):**

- **§5 (CI workflow YAML pseudocode):** update the `if:` expression on the `e2e:` job; add
  preflight step to the YAML pseudocode.
- **§6 (Secret safety):** add a paragraph on `JR_E2E_ENABLED` as the primary fork-safety
  mechanism; explain it is a repository variable (not environment-scoped), evaluated before
  the job starts.
- **§8 (Configuration inventory):** add the `JR_E2E_ENABLED` row per the spec §4 table
  (Kind: Repository variable; Where Set: Repository level — not environment-scoped; Required:
  Yes for canonical repo; forks default OFF).

**Verification:**
```
grep -n "JR_E2E_ENABLED" CLAUDE.md
```
Produces ≥ 2 matches (the table entry + the two-layer note).
```
grep -n "JR_E2E_ENABLED" docs/specs/e2e-live-jira-testing.md
```
Produces ≥ 3 matches (§5 YAML, §6 paragraph, §8 table row).

### AC-007 — Zero `src/` and zero `tests/` changes (architecture boundary)

`git diff --name-only HEAD` does NOT include any file under `src/` or `tests/`.

All changes are confined to:
- `.github/workflows/e2e.yml`
- `README.md`
- `CLAUDE.md`
- `docs/specs/e2e-live-jira-testing.md`

**Verification:**
```
git diff --name-only HEAD | grep -E "^src/|^tests/"
```
Returns empty output (0 matches).

No `cargo test` gate is required for the delta itself (zero Rust code changed). Running
`cargo test` confirms no accidental source changes.

## Rollout Note (Canonical Repo Operational Step)

**This is a required release/rollout task — document in the PR description.**

After the F4 PR is merged, the `e2e:` job will be SKIPPED even on the canonical repo
until the maintainer performs the following one-time step:

1. Navigate to `https://github.com/Zious11/jira-cli/settings/variables/actions`.
2. Click **"New repository variable"**.
3. Name: `JR_E2E_ENABLED`, Value: `true`.
4. Save. (This is a **repository variable**, NOT an environment variable — do not create it
   inside the `jira-e2e` Environment settings page.)
5. Trigger a `workflow_dispatch` run on `develop` to verify the job runs (not skipped).

Until step 3 is completed, the `e2e:` job skips on every push and nightly cron on the
canonical repo. This is the intended behavior — the variable is the explicit opt-in.
The CI badge will show green (skipped run = success conclusion) until the variable is set
and a full test run completes.

The `jira-e2e` GitHub Environment is NOT changed. It continues to hold the secrets and
environment-scoped variables as documented in `docs/specs/e2e-live-jira-testing.md §8`.

## Out of Scope

- Any change to `tests/e2e_live.rs` or other test files — the gate is workflow-level only.
- Any change to `ci.yml`, `release.yml`, or `e2e-sweeper.yml`.
- Adding `NFR-CI-001` to `nfr-catalog.md` — confirmed OMIT at F1 gate (D-7: CI implementation
  detail too fine-grained for NFR catalog).
- Any `Cargo.toml`, `Cargo.lock`, or `deny.toml` changes.
- Fork opt-in documentation beyond what is already in `docs/specs/e2e-fork-safe-ci-enablement.md §5.2`.

## Implementation Strategy

**Zero-src delivery order (no Red Gate, no failing-test-first, no demo phase):**

1. **Read `e2e.yml` in full** — understand current structure before editing.
2. **Edit `.github/workflows/e2e.yml`:**
   - Replace `if: github.event_name != 'pull_request'` with the combined expression (AC-001).
   - Add preflight step as the first job step (AC-004), with `env:` block referencing secrets/vars.
3. **Edit `README.md`** — add E2E badge in 2nd position in badge row (AC-005).
4. **Edit `CLAUDE.md`** — add `JR_E2E_ENABLED` table entry + two-layer note (AC-006).
5. **Edit `docs/specs/e2e-live-jira-testing.md`** — §5, §6, §8 updates (AC-006).
6. **Verify AC-007** — `git diff --name-only HEAD | grep -E "^src/|^tests/"` → empty.
7. **Run `cargo test`** — confirm exits 0 (confirms zero accidental src/ changes).
8. **Run the three spec-count guards** — confirm exits 0 (no BC/NFR drift).
9. **Commit and push.**

**Branch:** `ci/e2e-fork-safe-gate` (or similar `ci/` prefix consistent with CI infrastructure work).

**Commit message:**
```
ci(e2e): add JR_E2E_ENABLED repo-var gate + preflight + README badge + docs
```

**PR target:** `develop`.

## Quality Gate Self-Check

| Criterion | AC | Verification Command |
|-----------|----|---------------------|
| Gate expression present verbatim | AC-001 | `grep -n "vars.JR_E2E_ENABLED" .github/workflows/e2e.yml` → 1 match |
| Old single-condition `if:` removed | AC-001 | `grep -n "github.event_name != 'pull_request'$" .github/workflows/e2e.yml` → 0 matches (no bare single-condition line) |
| `environment: jira-e2e` retained | AC-003 | `grep -n "environment: jira-e2e" .github/workflows/e2e.yml` → ≥1 match |
| Preflight step present with all 4 vars | AC-004 | `grep -n "JR_E2E_BASE_URL\|JR_E2E_PROJECT\|JR_E2E_API_TOKEN\|JR_E2E_EMAIL" .github/workflows/e2e.yml` → ≥4 matches |
| Preflight uses `${VAR:?message}` pattern | AC-004 | `grep -n ':\${' .github/workflows/e2e.yml` → ≥4 matches |
| README badge URL verbatim | AC-005 | `grep -n "e2e.yml/badge.svg" README.md` → 1 match with exact URL |
| `JR_E2E_ENABLED` in CLAUDE.md | AC-006 | `grep -n "JR_E2E_ENABLED" CLAUDE.md` → ≥2 matches |
| `JR_RUN_E2E` two-layer note in CLAUDE.md | AC-006 | `grep -n "two-layer\|Two-layer" CLAUDE.md` → ≥1 match |
| `JR_E2E_ENABLED` in runbook §8 | AC-006 | `grep -n "JR_E2E_ENABLED" docs/specs/e2e-live-jira-testing.md` → ≥3 matches |
| Zero `src/` or `tests/` changes | AC-007 | `git diff --name-only HEAD \| grep -E "^src/\|^tests/"` → empty |
| `cargo test` exits 0 | smoke | confirms no accidental Rust changes |
| `cargo fmt --all -- --check` exits 0 | lint | no format drift |
| `cargo clippy --all-targets -- -D warnings` exits 0 | lint | zero warnings |
| `bash scripts/check-spec-counts.sh` exits 0 | invariant | no BC frontmatter changed |
| `bash scripts/check-bc-cumulative-counts.sh` exits 0 | invariant | no count surfaces touched |
| `bash scripts/check-bc-no-numeric-test-counts.sh` exits 0 | invariant | no BC bodies with numeric counts |

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~6 k |
| Design spec `docs/specs/e2e-fork-safe-ci-enablement.md` (full) | ~5 k |
| F1 delta analysis (relevant sections OQ-1..OQ-6) | ~4 k |
| `.github/workflows/e2e.yml` current state (~200 LOC to read + modify) | ~4 k |
| `README.md` badge row section (~20 LOC) | ~1 k |
| `CLAUDE.md` E2E section (~60 LOC relevant) | ~2 k |
| `docs/specs/e2e-live-jira-testing.md` (§5, §6, §8 to update; ~80 LOC relevant) | ~3 k |
| Tool outputs (`cargo test`, `cargo clippy`, grep verifications, script exits) | ~3 k |
| BC files: 0 (none loaded — BC delta empty) | 0 |
| **Total** | **~28 k** |

Well within a single-agent context window (~200 k). No split required.
LOC delta: `e2e.yml` +~25 LOC (gate + preflight step); `README.md` +1 LOC (badge);
`CLAUDE.md` +~10 LOC; `docs/specs/e2e-live-jira-testing.md` +~15 LOC. Zero `src/` LOC changes.

## Tasks

- [ ] Read `.github/workflows/e2e.yml` in full — understand current `if:`, step order, `environment:`, and secrets usage before any edits
- [ ] Read `docs/specs/e2e-fork-safe-ci-enablement.md` §2 (gate spec) and §9 (F4 touch-point list) — source of truth for exact expressions
- [ ] Read `README.md` badge row — identify exact position where 2nd badge is inserted
- [ ] Read `CLAUDE.md` AI Agent Notes E2E section — identify exact location for `JR_E2E_ENABLED` table entry and two-layer note
- [ ] Read `docs/specs/e2e-live-jira-testing.md` §5, §6, §8 — identify exact insertion points
- [ ] Edit `.github/workflows/e2e.yml`: replace `if: github.event_name != 'pull_request'` with `if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'` on the `e2e:` job (AC-001)
- [ ] Edit `.github/workflows/e2e.yml`: add preflight step as the first job step with `${VAR:?message}` assertions for `JR_E2E_BASE_URL`, `JR_E2E_PROJECT`, `JR_E2E_API_TOKEN`, `JR_E2E_EMAIL` (AC-004)
- [ ] Verify `grep -n "vars.JR_E2E_ENABLED" .github/workflows/e2e.yml` → 1 match (AC-001)
- [ ] Verify `grep -n "environment: jira-e2e" .github/workflows/e2e.yml` → ≥1 match (AC-003 — env retained)
- [ ] Verify `grep -n ':\${' .github/workflows/e2e.yml` → ≥4 matches (AC-004 — all 4 vars)
- [ ] Edit `README.md`: add E2E badge `[![E2E](...badge.svg?branch=develop)](...e2e.yml)` as 2nd badge in the badge row (AC-005)
- [ ] Verify `grep -n "e2e.yml/badge.svg" README.md` → 1 match with exact URL (AC-005)
- [ ] Edit `CLAUDE.md`: add `JR_E2E_ENABLED` entry to the `JR_*` env-var table in the AI Agent Notes E2E section (AC-006)
- [ ] Edit `CLAUDE.md`: add two-layer model note near the `JR_RUN_E2E` entry (AC-006)
- [ ] Verify `grep -n "JR_E2E_ENABLED" CLAUDE.md` → ≥2 matches (AC-006)
- [ ] Edit `docs/specs/e2e-live-jira-testing.md` §5: update `if:` expression in YAML pseudocode; add preflight step (AC-006)
- [ ] Edit `docs/specs/e2e-live-jira-testing.md` §6: add fork-safety paragraph on `JR_E2E_ENABLED` as primary mechanism (AC-006)
- [ ] Edit `docs/specs/e2e-live-jira-testing.md` §8: add `JR_E2E_ENABLED` row to configuration inventory table (AC-006)
- [ ] Verify `grep -n "JR_E2E_ENABLED" docs/specs/e2e-live-jira-testing.md` → ≥3 matches (AC-006)
- [ ] Verify `git diff --name-only HEAD | grep -E "^src/|^tests/"` → empty output (AC-007)
- [ ] Run `cargo test` — exits 0 (confirms zero accidental Rust changes)
- [ ] Run `cargo fmt --all -- --check` — exits 0
- [ ] Run `cargo clippy --all-targets -- -D warnings` — exits 0
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh && bash scripts/check-bc-no-numeric-test-counts.sh` — all exit 0
- [ ] Confirm rollout note is included in PR description: maintainer must create repository variable `JR_E2E_ENABLED=true` post-merge
- [ ] Commit: `ci(e2e): add JR_E2E_ENABLED repo-var gate + preflight + README badge + docs`

## Previous Story Intelligence

**Predecessor: S-E2E-1 (PR #433)** — established `e2e.yml` structure: the `e2e:` job, the
`environment: jira-e2e` declaration, the `concurrency: jira-e2e` group, step ordering
(harden-runner → checkout → rust-cache → build → compose-auth → run-tests → classify-failure
→ teardown), and the first `if: github.event_name != 'pull_request'` condition. The AC-001
edit replaces that condition in-place without restructuring the job.

**Predecessor: S-E2E-5 (PR #440+#441+#442)** — added `e2e-sweeper.yml` (daily sweeper) and
a `if: failure()` failure-classification step in `e2e.yml`. The preflight step (AC-004) in
this story is inserted BEFORE the failure-classification step; do not reorder the existing
`if: failure()` and `if: always()` steps.

**Key lesson from S-E2E-1/S-E2E-5:** the `environment: jira-e2e` declaration is CRITICAL —
it is what makes secrets from the `jira-e2e` GitHub Environment available to the job steps.
The AC-001 gate line comes just before or after the `environment:` declaration at the job
level; do not remove `environment: jira-e2e`.

**Architecture constraint (spec §2.3):** `vars.JR_E2E_ENABLED` in `jobs.e2e.if:` is evaluated
BEFORE the job starts and BEFORE any environment loads. This means even though
`environment: jira-e2e` is declared, environment-scoped variables are NOT available at
`if:` evaluation time. `JR_E2E_ENABLED` MUST be set as a repository-level variable, not as
an environment variable. Do not confuse these two variable scopes.

## Architecture Compliance Rules

1. **Zero `src/` changes.** If any `src/` file is added to the diff, STOP and escalate.
   This story is entirely CI YAML + Markdown + documentation.

2. **`environment: jira-e2e` MUST be retained** on the `e2e:` job. It supplies secrets
   and environment-scoped variables to all steps. Removing it would break the live E2E run.

3. **Gate is job-level, not step-level.** The `if:` MUST be on the `jobs.e2e` key, not on
   any individual step. A step-level `if:` would still provision a runner and execute the job.

4. **`JR_E2E_ENABLED` MUST be a repository variable** (not environment-scoped). This is
   a load-bearing constraint from spec §2.3 — environment-level variables are not available
   in `jobs.<id>.if:` at scheduling time. Do not add instructions to create it as an
   environment variable.

5. **Preflight step is the FIRST step after `Harden the runner`** (if harden-runner is
   present). It runs before the Rust build so that missing-config failures do not waste
   runner minutes on compilation.

6. **Do NOT add `if:` conditions to existing steps.** The existing `if: failure()` and
   `if: always()` steps are already fully suppressed by the job-level gate when the job
   skips (no runner is provisioned; no steps run). No per-step changes are needed.

7. **`docs/specs/e2e-fork-safe-ci-enablement.md` is NOT modified** by this story. It is
   the F2 spec (source of truth); implementation (F4) only modifies the 4 files listed in
   `files_modified`. The spec is an input artifact, not an output.

## Library & Framework Requirements

No new `Cargo.toml` dependencies. Zero Rust changes.

| Tool/File | Already available | Usage in this story |
|-----------|------------------|---------------------|
| GitHub Actions `vars` context | Yes (built-in) | `vars.JR_E2E_ENABLED` in job-level `if:` |
| bash `${VAR:?message}` idiom | Yes (bash built-in) | Preflight assertions — zero new dependencies |
| `actions/checkout@v4` | Yes (existing) | Unchanged (no new workflow files) |

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `.github/workflows/e2e.yml` | MODIFY | Add gate expression to `jobs.e2e.if:`; add preflight step as first job step |
| `README.md` | MODIFY | Add E2E badge as 2nd badge in badge row (after `[![CI]...]`) |
| `CLAUDE.md` | MODIFY | Add `JR_E2E_ENABLED` to AI Agent Notes `JR_*` table; add two-layer model note near `JR_RUN_E2E` |
| `docs/specs/e2e-live-jira-testing.md` | MODIFY | §5 YAML pseudocode update; §6 fork-gate paragraph; §8 new table row |

**Files NOT to create or touch:** All of `src/`; all of `tests/`; `Cargo.toml`; `Cargo.lock`;
`deny.toml`; `.github/workflows/ci.yml`; `.github/workflows/release.yml`;
`.github/workflows/e2e-sweeper.yml`; `.cargo/mutants.toml`; `scripts/`;
`docs/specs/e2e-fork-safe-ci-enablement.md` (spec is input artifact, not modified);
`BC-INDEX.md`; `CANONICAL-COUNTS.md`; any `.factory/specs/` files.

## Branch / PR Plan

- Branch: `ci/e2e-fork-safe-gate`
- Target: `develop`
- Commit: `ci(e2e): add JR_E2E_ENABLED repo-var gate + preflight + README badge + docs`
- PR body: reference this story (S-E2E-FORK-1), design spec §2 and §9, and the rollout
  note (maintainer must create `JR_E2E_ENABLED=true` as a repository variable post-merge)
- CHANGELOG entry: Add under `[Unreleased]` — "Added `JR_E2E_ENABLED` repository variable
  gate to `e2e.yml`: E2E job skips on forks and repos without the variable set; preflight
  step asserts required config before Rust build; README E2E status badge added.
  Maintainer: create repo variable `JR_E2E_ENABLED=true` post-merge to re-enable nightly
  E2E on the canonical repo."
- **PR description MUST include the rollout note**: maintainer must create repository variable
  `JR_E2E_ENABLED=true` after merge or the E2E job will skip on all runs including canonical.
