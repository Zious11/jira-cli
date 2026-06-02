---
document_type: f1-delta-analysis
phase: phase-f1-delta-analysis
producer: architect
feature: e2e-fork-safe-enablement
status: awaiting-human-approval
timestamp: 2026-06-01
project: jira-cli
mode: BROWNFIELD
intent: enhancement
feature_type: infrastructure (CI-only)
trivial_scope: false
regression_risk: low
bc_delta: empty
src_delta: zero
tests_delta: zero
---

# Delta Analysis Report — Fork-safe E2E CI Enablement Flag + README Badge

**Feature:** Fork-safe E2E CI enablement flag (`JR_E2E_ENABLED`) + README E2E status badge
**Brainstorming report:** `.factory/planning/brainstorming-report.md` (direction-selected, 2026-06-02)
**Mode:** BROWNFIELD / Feature Mode (F1-F7)
**Date:** 2026-06-01

---

## Classification

| Dimension | Value |
|-----------|-------|
| Feature type | infrastructure (CI workflow + docs only) |
| Intent | enhancement — fork-safety guard + visibility |
| Trivial scope? | NO -- CLAUDE.md + docs/specs changes require F2 doc discipline; full F1-F7 |
| BC delta | EMPTY (no new or modified BCs; does not touch any product behavior) |
| src/ delta | ZERO (not one line of Rust changes; zero `src/` files touched) |
| tests/ delta | ZERO (no change to `tests/e2e_live.rs` or any other test file; gate is workflow-level, not test-binary-level) |
| Architecture change | NO (no ADR update; no .factory/specs/architecture change) |

---

## Problem Being Solved

The existing `e2e.yml` workflow runs on `push` to `develop`/`main`, nightly `cron`, and
`workflow_dispatch`. It already guards against PR runs with `if: github.event_name != 'pull_request'`
at the job level. However, the workflow still fires on forks when:

- A fork contributor pushes to their own `develop`/`main`.
- The fork's nightly `cron` triggers (schedule events inherit the workflow from the fork's branch).

On a fork the `jira-e2e` environment secrets and variables are not present. Required-var
`.expect()` calls inside `tests/e2e_live.rs` then panic, producing a red run that the
fork contributor cannot fix or suppress. This feature adds a single opt-in repo variable
that defaults OFF on forks and can be set ON by any fork maintainer who configures their
own `jira-e2e` environment.

---

## Locked Decisions (from brainstorming session)

Both decisions are locked and are NOT open for re-deliberation at the F1 gate:

| Decision ID | Decision |
|-------------|----------|
| LOCK-A | Gate mechanism = repo VARIABLE `JR_E2E_ENABLED` checked at the workflow JOB-level `if:` (NOT inside the Rust tests; NOT a `github.repository` owner-string check) |
| LOCK-B | Badge scope = `develop` nightly (badge URL pinned to `?branch=develop` on the canonical repo) |

---

## Open Question Resolutions

The 7 open questions from the brainstorming report are resolved below, grounded in the
actual files read during this analysis.

### OQ-1: Exact `if:` expression composing the new var guard with the existing guard

**Current job `if:`** (line 21 of `e2e.yml`):
```yaml
if: github.event_name != 'pull_request'
```

**Proposed replacement:**
```yaml
if: >-
  github.event_name != 'pull_request' &&
  vars.JR_E2E_ENABLED == 'true'
```

Rationale for the YAML block-scalar form: GitHub Actions evaluates job `if:` expressions
as a single logical string; multi-line YAML with `>-` (folded, strip trailing newlines)
produces a single-line expression. Alternatively the expression fits on one line:

```yaml
if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'
```

Both forms are equivalent. The single-line form is preferred for readability given the
expression length.

This guard applies to ALL three triggers (push, schedule, workflow_dispatch) because it
is at the JOB level, not inside any `on:` branch filter or per-step conditional.

The `vars.` context in GitHub Actions refers to repository variables (and
environment-level variables when an `environment:` key is declared on the job — which
`e2e.yml` already has: `environment: jira-e2e`). Variables defined on the `jira-e2e`
environment are accessible as `vars.JR_E2E_ENABLED` within that job. Forks that have
not set this variable receive an empty string, which does not equal `'true'`, so the
condition evaluates to `false` and the job is skipped.

**Resolution: SINGLE-LINE COMBINED EXPRESSION. No per-step changes needed (see OQ-2).**

### OQ-2: Do the `if: failure()` and `if: always()` steps run when the job is skipped?

**GitHub Actions semantics (definitive):**

A skipped job never starts a runner. None of its steps execute — not even steps with
`if: always()`, `if: failure()`, or `if: success()`. Step-level `if:` expressions are
evaluated only inside a running job. When the job-level `if:` is `false`, the entire
job is skipped at the scheduler layer; the runner is never provisioned and no steps run.

This means:
- The `if: failure()` "Classify failure" step (lines 98-134 of `e2e.yml`) will NOT run.
- The `if: always()` "Teardown — close leftover E2E issues" step (lines 136-180 of
  `e2e.yml`) will NOT run.
- No secrets from the `jira-e2e` environment are accessed.
- No runner minutes are consumed.

**Implication:** gating at the JOB level is the correct and complete solution. No
per-step changes are required. The single `if:` addition to the `e2e:` job block
disables the entire job — harden-runner, checkout, Rust install, rust-cache, all test
steps, classify-failure, AND teardown — on any fork or repo where
`JR_E2E_ENABLED != 'true'`.

**Resolution: JOB-LEVEL GATE IS COMPLETE. ZERO per-step changes needed.**

### OQ-3: Badge rendering truth — what does the badge show when the job is skipped?

**Badge URL (proposed):**
```
https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml/badge.svg?branch=develop
```

**GitHub Actions badge rendering semantics:**

The `badge.svg` endpoint reflects the CONCLUSION of the most recent workflow RUN (not
job) on the specified branch. The possible conclusions and their badge rendering are:

| Workflow run conclusion | Badge renders |
|------------------------|---------------|
| `success` (all jobs passed or were skipped) | green "passing" |
| `failure` (any required job failed) | red "failing" |
| `cancelled` | grey "cancelled" |
| No run exists on branch | grey "no status" |

When the `e2e:` job is skipped (job-level `if:` is `false`), the workflow RUN itself
concludes as `success` (a workflow with all jobs skipped or passed is a successful run).
The badge renders **green "passing"**.

**Truthfulness analysis:**

On the canonical repo (`Zious11/jira-cli`), `JR_E2E_ENABLED=true` will be set, so the
job runs normally and the badge reflects actual test results. Green = tests passed; Red
= tests failed. The badge is truthful.

On a fork that has NOT set `JR_E2E_ENABLED=true`, the job is skipped and the workflow
run is green. However, the README badge URL is hardcoded to `Zious11/jira-cli` — it
always displays the canonical repo's badge regardless of which repo's README is being
viewed. A fork viewer sees the canonical repo's E2E status, not their fork's.

**Edge case — fork with `JR_E2E_ENABLED=true`:** A fork that opts in will run E2E
tests and its badge (if the fork's README is viewed) reflects the fork's results. This
is the correct behavior — an opted-in fork should show its own E2E status.

**Resolution: BADGE IS TRUTHFUL. Add badge to README pinned to develop. Badge URL
is `https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml/badge.svg?branch=develop`
with link target `https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml`.**

### OQ-4: "E2E config preflight" — in scope or defer?

A preflight step would be added inside the `e2e:` job (running only when `JR_E2E_ENABLED==true`)
to assert all REQUIRED environment variables (`JR_E2E_BASE_URL`, `JR_E2E_EMAIL`,
`JR_E2E_API_TOKEN`, `JR_E2E_PROJECT`) are non-empty, failing fast with a descriptive
error before consuming runner minutes building Rust.

**Recommendation: DEFER to a future enhancement (not in scope for this feature).**

Rationale:
1. The feature's primary goal is fork-safety (the job never starts on a fork). Once that
   is achieved, misconfig on the canonical repo produces a loud failure at the first
   `cargo test` invocation anyway — the test binary panics with `.expect()` messages
   naming the missing variables. The signal is not silent.
2. The preflight adds a new step with its own `env:` block referencing secrets, which
   increases the review surface for this feature beyond what is warranted for a guard
   that is already partially covered.
3. A preflight is more valuable as a SEPARATE story/enhancement once the fork-safe gate
   is stable, when the exact set of required variables and desired error UX can be
   designed cleanly.

**Decision to lock at F1 gate: DEFER preflight. Not in scope for this feature.**

### OQ-5: Naming — `JR_E2E_ENABLED` vs alternatives; relationship to `JR_RUN_E2E`

**Confirmed name: `JR_E2E_ENABLED`** (as selected in brainstorming).

The two identifiers serve entirely distinct purposes and MUST remain distinct:

| Identifier | Where it lives | What it controls |
|------------|---------------|------------------|
| `JR_E2E_ENABLED` | GitHub Actions repo/environment variable (`vars.JR_E2E_ENABLED`) | WHETHER the `e2e:` CI job starts at all. Evaluated by the GitHub Actions runner at job-scheduling time. Never read by Rust code. |
| `JR_RUN_E2E` | Process environment variable passed to `cargo test` subprocess | WHETHER individual `#[ignore]` test functions early-return instead of running. Evaluated by the Rust test binary at test-execution time. |

The relationship: `JR_E2E_ENABLED == 'true'` at the workflow level is what causes the
workflow to set `JR_RUN_E2E=1` in the "Run live E2E tests" step's `env:` block and
then invoke `cargo test --include-ignored`. If the job never starts, `JR_RUN_E2E` is
never set. If somehow the job starts but `JR_RUN_E2E` is missing from the step `env:`,
the `#[ignore]` gate inside the test binary provides a second layer of protection.

The two-layer design is belt-and-suspenders: workflow-level for fork-safety and CI
resource savings; test-binary-level for local developer correctness. They are
complementary, not redundant.

**Resolution: KEEP `JR_E2E_ENABLED` (repo var, CI gate) DISTINCT from `JR_RUN_E2E`
(test binary gate). Document the relationship in CLAUDE.md and e2e-live-jira-testing.md.**

### OQ-6: Scope confirmation — is this ZERO `src/`, ZERO `tests/e2e_live.rs` change?

**Confirmed: YES. This feature touches ZERO Rust source files.**

The gate is implemented entirely at the GitHub Actions workflow-YAML level. No change is
needed to:
- `src/` (no binary behavior change)
- `tests/e2e_live.rs` (no test-binary gate change)
- `tests/e2e_cli_surface_guard.rs` (no new CLI surface)
- `Cargo.toml` / `Cargo.lock`
- `scripts/`
- `.cargo/mutants.toml`

The complete set of files touched by this feature:

| File | Change type |
|------|-------------|
| `.github/workflows/e2e.yml` | MODIFIED — add `JR_E2E_ENABLED` check to job `if:` |
| `README.md` | MODIFIED — add E2E status badge to badge row |
| `CLAUDE.md` | MODIFIED — add `JR_E2E_ENABLED` to AI-Agent-Notes E2E section; document the relationship with `JR_RUN_E2E` |
| `docs/specs/e2e-live-jira-testing.md` | MODIFIED — add `JR_E2E_ENABLED` to §8 config inventory table; update §6 Secret Safety; update §5 CI workflow description |

**F4 implication: zero-code implementation.** F4 has no Red Gate invocation, no
implementer story for Rust code, and no demo story. The "implementation" phase
consists of the CI YAML edit, README edit, and two documentation edits. These can be
delivered as a single story or two stories (see Story Breakdown below).

**Resolution: ZERO `src/`, ZERO `tests/` change CONFIRMED. F4 is documentation +
YAML only.**

### OQ-7: Documentation fallout list

The following documentation files MUST be updated as part of this feature (not deferred):

1. **CLAUDE.md** — E2E AI-Agent-Notes section currently documents `JR_RUN_E2E` as the
   test-binary seam and the full env-var table. Add a new entry for `JR_E2E_ENABLED`
   explaining it is a CI gate (repo variable, workflow-level), its relationship to
   `JR_RUN_E2E`, and the fork-opt-in flow. Apply the doc-fallout rule codified by
   PR #335/#357: new `JR_*` seam documented in the same commit as the code change.

2. **`docs/specs/e2e-live-jira-testing.md`** — Multiple sections need updating:
   - **§5 CI workflow** — Update the YAML pseudocode to show the new `if:` expression.
   - **§6 Secret Safety** — Add a paragraph on the `JR_E2E_ENABLED` variable as the
     primary fork-safety mechanism (currently §6 only describes the `pull_request`
     guard and the environment deployment-branch policy).
   - **§8 Configuration inventory** — Add a row for `JR_E2E_ENABLED` (kind: variable,
     where set: repo level or `jira-e2e` environment, required: yes for canonical repo,
     notes: forks default OFF).

**Resolution: DOC FALLOUT = CLAUDE.md (E2E section) + docs/specs/e2e-live-jira-testing.md
(§5, §6, §8). Both MANDATORY, not deferred.**

---

## Impact Assessment

| Layer | Impact |
|-------|--------|
| PRD / BCs | No change. This feature does not alter any product behavior or API contract. |
| Architecture | No change. No ADR, no .factory/specs/architecture update. |
| UX | N/A (CI infrastructure only). README badge is additive, not a behavior change. |
| Stories | 1 story recommended (see Story Breakdown). |
| Tests | ZERO change. Gate is workflow-level; e2e_live.rs is not modified. |
| Verification | Not applicable (no new code to verify; the gate is a YAML conditional). |
| CI | `e2e.yml` MODIFIED (one `if:` line addition to job definition). |
| Docs | CLAUDE.md + docs/specs/e2e-live-jira-testing.md MODIFIED. |
| README | Badge row MODIFIED (one badge added). |

### Impact Boundary

The impact boundary is strictly limited to:
- The `e2e:` job in `.github/workflows/e2e.yml` — one changed line.
- The badge row in `README.md` — one added badge.
- Two documentation files updated to reflect the new variable.

Nothing outside this boundary is touched. The existing `ci.yml`, `release.yml`,
`e2e-sweeper.yml` workflows are unchanged. The test binary, all `src/` files,
`Cargo.toml`, `Cargo.lock`, and all non-E2E documentation are unchanged.

---

## Component Impact Table

| File | Change Type | Scope of Change |
|------|-------------|-----------------|
| `.github/workflows/e2e.yml` | MODIFIED | Add `&& vars.JR_E2E_ENABLED == 'true'` to the `e2e:` job `if:` expression (1 logical change, currently on line 21). No step-level changes. |
| `README.md` | MODIFIED | Add one E2E badge to the existing badge row (lines 3-8). Match badge markdown style of the existing CI badge. |
| `CLAUDE.md` | MODIFIED | AI-Agent-Notes E2E section: add `JR_E2E_ENABLED` entry to `JR_*` env-var table; document CI-gate vs test-binary-gate relationship; describe fork opt-in flow. |
| `docs/specs/e2e-live-jira-testing.md` | MODIFIED | §5 YAML pseudocode update; §6 paragraph on variable-based fork gate; §8 new table row for `JR_E2E_ENABLED`. |

**Files confirmed NOT changed:**
- `src/` (all files)
- `tests/e2e_live.rs`
- `tests/e2e_cli_surface_guard.rs`
- `tests/common/`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `.github/workflows/e2e-sweeper.yml`
- `Cargo.toml`, `Cargo.lock`, `deny.toml`
- `.cargo/mutants.toml`
- `scripts/`
- Any `.factory/specs/` files (no BC, PRD, or architecture change)

---

## BC / NFR Coverage Map

### BC Delta: EMPTY

This feature introduces no new behavioral contracts and modifies no existing ones.
No BC files are created or edited. BC-INDEX.md is unchanged.

### NFR Impact: CI Infrastructure NFR (proposed, not written in F1)

The feature addresses a CI reliability and correctness property that could be expressed
as a new NFR in `prd-supplements/nfr-catalog.md`. A proposed identifier and description
for the F2 spec author to evaluate:

**Proposed NFR-CI-001 (Fork-safety):**
> The E2E CI workflow MUST NOT start on forks lacking the `JR_E2E_ENABLED` variable.
> Forks MUST default OFF and MUST be able to opt in by setting `JR_E2E_ENABLED=true`
> in their own repo/environment configuration.

Whether to formally add this NFR, or to treat the feature as a CI implementation detail
beneath the NFR catalog's granularity threshold, is a decision for the F1 human gate
and F2 spec author.

---

## Regression Risk Assessment

| Risk | Level | Detail |
|------|-------|--------|
| Breaking the existing `e2e:` job on the canonical repo | LOW | The `&&` addition to the `if:` expression changes only the fork-fork behavior. On the canonical repo where `JR_E2E_ENABLED=true` is set, the expression evaluates identically to the current guard. |
| Badge-row markdown breakage in README | LOW | Adding one badge to an existing badge row is a minimal, well-understood change. Risk of malformed markdown is negligible with review. |
| CLAUDE.md doc errors | LOW | Documentation prose; no CI/test behavior depends on it. |
| `vars.` context unavailability | LOW | `vars.JR_E2E_ENABLED` is a GitHub Actions repository/environment variable. The `vars` context is available in job `if:` expressions. It evaluates to empty string (falsy) when the variable is not set — the safe default. Confirmed semantics via GitHub Actions documentation. |
| Silent coverage erosion (OQ-4 risk) | MEDIUM (ACCEPTED) | If `JR_E2E_ENABLED=true` is set but required secrets are missing/expired, the job starts and fails at the Rust test step. This is the pre-existing behavior — not a regression introduced by this feature. The preflight OQ-4 is deferred; the risk is accepted at this scope level. |
| `ci.yml` or `release.yml` impact | NONE | Neither workflow references `e2e.yml` or `JR_E2E_ENABLED`. Completely independent. |
| `tests/e2e_live.rs` test-gate correctness | NONE | Test file is not modified. The existing `JR_RUN_E2E` gate inside the binary is unchanged. |

**Overall Regression Risk: LOW.**

---

## Recommended Story Breakdown

This feature is small enough to deliver as a single story. Given the zero-`src/`
scope there is no Red Gate phase, no separate implementer story, and no demo.

| Story ID | Scope | Effort |
|----------|-------|--------|
| S-E2E-FORK-1 | CI YAML edit + README badge + CLAUDE.md + docs/specs update | 2 SP |

**Total: 1 story, 2 SP.**

The single story delivers all four file changes atomically, which is appropriate because:
- The YAML change and the documentation changes are tightly coupled (CLAUDE.md must
  document what the YAML change introduces, in the same commit per the doc-fallout rule).
- The README badge change is trivial and belongs in the same PR.
- No ordering dependency exists between the four changes.

**F4 delivery note:** Because there is no Rust code, F4 runs without invoking the Red
Gate, the implementer agent, or the demo step. The deliverable is a single PR touching
only YAML, Markdown, and documentation files.

---

## F1 Decisions to Lock at Human Gate

The following decisions require explicit human confirmation before F2 proceeds. Items
marked LOCKED were decided in the brainstorming session and are included here for
confirmation only; the remaining items are new F1 decisions.

| # | Decision | Options | Recommendation |
|---|----------|---------|----------------|
| D-1 (LOCKED) | Gate mechanism | repo variable `JR_E2E_ENABLED` job-level `if:` | LOCK: repo variable, job-level `if:` |
| D-2 (LOCKED) | Badge scope | develop nightly | LOCK: develop nightly |
| D-3 | `if:` expression form | single-line vs block-scalar `>-` | RECOMMEND: single-line: `if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'` |
| D-4 | Preflight step | in scope vs defer | RECOMMEND: DEFER (rationale in OQ-4) |
| D-5 | Badge placement | insert after existing CI badge | RECOMMEND: insert as 2nd badge in the existing badge row, after `[![CI](...)]` and before `[![Release](...)]` |
| D-6 | Badge link target | actions/workflows/e2e.yml page | RECOMMEND: `https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml` |
| D-7 | NFR-CI-001 formalization | add to nfr-catalog.md vs omit | RECOMMEND: OMIT from nfr-catalog (CI implementation detail; too fine-grained for NFR catalog) — F2 confirms |
| D-8 | Story count | 1 story vs 2 stories | RECOMMEND: 1 story (2 SP); all changes are co-dependent and atomic |

---

## Recommended Scope for F2-F7

| Phase | Recommended Scope |
|-------|------------------|
| F2 (spec evolution) | EMPTY BC delta confirmed — no PRD/BC change. Confirm D-7 NFR decision. Lightweight: record zero-BC delta, note documentation fallout list as implementation constraint. |
| F3 (incremental stories) | Author S-E2E-FORK-1 per breakdown above. Include explicit acceptance criteria for: (a) job skips on fork (test by inspecting the `if:` expression, not by running a fork); (b) badge appears in README and renders correctly; (c) CLAUDE.md and docs updated; (d) canonical repo unaffected (if-expression evaluates true when var is set). |
| F4 (delta implementation) | Deliver S-E2E-FORK-1. YAML edit + README edit + doc edits. No Rust compilation needed for the delta itself, but run `cargo test` to confirm no accidental changes snuck in. No Red Gate. No demo. |
| F5 (scoped adversarial) | Review the `if:` expression for correctness, the badge URL for accuracy, and the CLAUDE.md entry for completeness. Low surface; expected clean. |
| F6 (targeted hardening) | Security scan: confirm `vars.JR_E2E_ENABLED` cannot be set by a fork contributor to bypass the gate (GitHub repo variables are owned by the repo admin — fork contributors cannot set them on the upstream repo). No mutation testing (zero src/). |
| F7 (delta convergence) | Confirm CI passes (ci.yml green), badge renders, E2E nightly still runs on canonical. No cargo-mutants run (zero src/). |

---

## Regression Baseline (files NOT changed)

All of `src/`; all of `tests/`; `Cargo.toml`; `Cargo.lock`; `deny.toml`;
`.github/workflows/ci.yml`; `.github/workflows/release.yml`;
`.github/workflows/e2e-sweeper.yml`; `scripts/`; `.cargo/mutants.toml`;
`BC-INDEX.md`; `CANONICAL-COUNTS.md`; `.factory/specs/`.
