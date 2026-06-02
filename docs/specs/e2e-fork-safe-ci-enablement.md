# Feature Spec: Fork-safe E2E CI Enablement Flag + README E2E Status Badge

**Status:** Approved (F1 human gate passed 2026-06-01)
**Author:** jr maintainers (F2 spec evolution 2026-06-01)
**Extends:** `docs/specs/e2e-live-jira-testing.md` — cross-reference, not duplication.
  Sections of that spec updated by this feature are called out explicitly in §9 below.
**Research:** `.factory/planning/brainstorming-report.md` + `.factory/phase-f1-delta-analysis/e2e-fork-safe-enablement/delta-analysis.md`
**Tracking:** GitHub issue TBD (filed during F4 delivery)

---

## 1. Problem and Context

The existing `e2e.yml` CI workflow runs on `push` to `develop`/`main`, nightly `cron`
(`0 6 * * *` UTC), and `workflow_dispatch`. It already guards against PR-triggered runs
via `if: github.event_name != 'pull_request'` at the job level.

However the workflow still fires on **forks** when:
- A fork contributor pushes to their own `develop` or `main` branch.
- The fork's nightly `cron` triggers (schedule events inherit the workflow from the
  fork's default branch).

On a fork the `jira-e2e` GitHub Environment secrets and variables are absent. Required-var
`.expect()` calls inside `tests/e2e_live.rs` then panic, producing a **red CI run that the
fork contributor cannot fix or suppress**. This creates noise, wastes runner minutes, and
may mislead contributors into thinking the project is broken.

**Goal:** Add a single opt-in repository variable (`JR_E2E_ENABLED`) that:
1. Defaults OFF on any fork (variables are not inherited by forks).
2. Can be set ON by any fork maintainer who provisions their own `jira-e2e` environment.
3. Defaults ON for the canonical repo (`Zious11/jira-cli`) once the maintainer creates the variable.
4. Enables an E2E status badge in `README.md` that reflects the canonical repo's nightly health.

**BC and NFR corpora unchanged.** This feature addresses CI infrastructure only and does
not touch any product behavior. The BC corpus (585 BCs) and NFR corpus (41 NFRs) are
EXPLICITLY UNCHANGED by this spec.

---

## 2. Design

### 2.1 The Two-Layer Model

Two distinct identifiers control E2E execution at different layers. They are
**complementary, not redundant**, and MUST remain distinct:

| Identifier | Layer | Where it lives | What it controls |
|------------|-------|---------------|------------------|
| `JR_E2E_ENABLED` | CI workflow | GitHub Actions **repository variable** (`vars.JR_E2E_ENABLED`) | Whether the `e2e:` job starts at all. Evaluated by the GitHub Actions scheduler at job-scheduling time. **Never read by Rust code.** |
| `JR_RUN_E2E` | Test binary | Process environment variable passed to `cargo test` | Whether individual `#[ignore]` test functions early-return instead of running. Evaluated by the Rust test binary at test-execution time. |

The relationship: when `JR_E2E_ENABLED == 'true'` passes the job-level gate, the workflow
sets `JR_RUN_E2E=1` in the "Run live E2E tests" step `env:` block and then invokes
`cargo test --include-ignored`. If the job never starts, `JR_RUN_E2E` is never set.
If the job starts but `JR_RUN_E2E` is somehow absent from the step `env:`, the
`#[ignore]` gate inside the test binary provides a second layer of protection.

**Belt-and-suspenders design:** workflow-level gate for fork-safety and CI resource
savings; test-binary-level gate for local developer correctness. See §6 for the CLAUDE.md
doc-fallout obligation on `JR_*` entries.

### 2.2 Job-Level Gate Expression

The `e2e:` job in `.github/workflows/e2e.yml` currently declares:

```yaml
if: github.event_name != 'pull_request'
```

This spec requires replacing that single condition with the combined expression:

```yaml
if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'
```

**Why single-line form:** the expression fits on one line and is fully readable at that
length. A `>-` block-scalar form produces the same logical result but is unnecessarily
verbose for this expression length.

**Why job-level, not step-level:** a skipped job never provisions a runner. None of its
steps execute, including steps with `if: failure()` or `if: always()`. The existing
"Classify failure" (`if: failure()`) and "Teardown" (`if: always()`) steps are therefore
**fully suppressed** by the job-level gate — no per-step changes are needed.

**Why this gate covers all three triggers:** the `if:` is at the `jobs.e2e` level, not
inside any `on:` branch filter or per-step conditional. It applies uniformly to `push`,
`schedule`, and `workflow_dispatch` events.

### 2.3 `JR_E2E_ENABLED` MUST Be a Repository Variable, Not an Environment Variable

**This is a load-bearing architectural constraint. Read carefully before implementing.**

GitHub Actions distinguishes two `vars` scopes:

| Scope | Available in `jobs.<job_id>.if` | When available |
|-------|--------------------------------|----------------|
| Repository variable | YES | Evaluated before the job starts |
| Environment variable (scoped to a GitHub Environment) | NO | Only available on the runner *after* the job starts executing |

The GitHub Actions documentation states explicitly:
> "Environment-level variables are only available on the runner after the job starts
> executing."
> — docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/variables#defining-variables-for-multiple-workflows

Because `jobs.e2e.if:` is evaluated **before** the job starts and therefore **before**
any environment is loaded, `vars.JR_E2E_ENABLED` MUST be set as a **repository-level
variable** (or organization-level variable), NOT as a variable scoped to the `jira-e2e`
environment.

The existing `environment: jira-e2e` declaration is **retained** on the `e2e:` job. It
continues to supply the `JR_E2E_BASE_URL`, `JR_E2E_EMAIL`, and `JR_E2E_API_TOKEN`
**secrets** (which load after the job starts, after the gate passes) and the
environment-scoped **variables** (`JR_E2E_PROJECT`, `JR_E2E_BOARD_ID`, etc.). The
`JR_E2E_ENABLED` gate variable is additive at the repo level; it does not replace the
environment.

**Why secrets cannot be used in `if:`:** GitHub Actions secrets are withheld from
`jobs.<job_id>.if` expressions entirely — using `secrets.SOMETHING` in a job-level `if:`
always evaluates to an empty string, making it useless as a gate. The `vars` context is
the correct mechanism (cited above).

**Fork behavior:** a fork that has not created the `JR_E2E_ENABLED` repository variable
receives an empty string when `vars.JR_E2E_ENABLED` is evaluated. An empty string does
not equal `'true'`, so the condition evaluates to `false` and the job is skipped cleanly.
Scheduled workflows on forks also skip — `vars.JR_E2E_ENABLED` is absent on every fork
that has not opted in.

### 2.4 Preflight Step (In Scope — Specified, Delivered in F4)

A preflight step MUST be added as a step **before** the `Run live E2E tests` step (so it
fails before the expensive Rust build/test), inside the gated job. It runs only when
`JR_E2E_ENABLED == 'true'` passes the gate.

**Purpose:** collect ALL missing required values and fail once with a GitHub error
annotation listing every absent variable, so an operator fixing a fresh environment does
not hit them one re-run at a time.

**Required variables to assert:** `JR_E2E_BASE_URL`, `JR_E2E_PROJECT`,
`JR_E2E_API_TOKEN`, `JR_E2E_EMAIL`.

**Specified implementation pattern:**

```bash
missing=()
[ -z "${JR_E2E_BASE_URL:-}" ]  && missing+=("JR_E2E_BASE_URL (secret, jira-e2e environment)")
[ -z "${JR_E2E_PROJECT:-}" ]   && missing+=("JR_E2E_PROJECT (variable)")
[ -z "${JR_E2E_EMAIL:-}" ]     && missing+=("JR_E2E_EMAIL (secret)")
[ -z "${JR_E2E_API_TOKEN:-}" ] && missing+=("JR_E2E_API_TOKEN (secret)")
if [ ${#missing[@]} -gt 0 ]; then
  printf '::error::E2E preflight failed — missing required config:\n'
  printf '  - %s\n' "${missing[@]}"
  exit 1
fi
echo "E2E preflight OK — all required config present."
```

The step's `env:` block must reference these variables from their respective secret/variable
sources. The exact step YAML is written in F4 (not in this spec) per ADR-0004.

**Why collect-and-fail over fail-on-first:** the `${VAR:?message}` idiom aborts on the
first missing variable; an operator fixing a fresh environment hits them one re-run at a
time. Collecting all missing values into an array and emitting a single `::error::` annotation
surfaces every gap at once, minimising round-trips.

### 2.5 README E2E Status Badge

A GitHub Actions status badge for `e2e.yml` pinned to the `develop` branch is added to
`README.md` as the **second badge in the badge row**, after the existing `[![CI]...]` badge
and before any release or other badge.

**Exact markdown (normative — implement verbatim):**

```markdown
[![E2E](https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml/badge.svg?branch=develop)](https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml)
```

**Badge truthfulness analysis:**

When the `e2e:` job is skipped (job-level `if:` is `false`), the workflow RUN itself
concludes as `success` (a workflow with all jobs skipped or passed is a successful run).
The `badge.svg` endpoint renders **green "passing"** for a success conclusion.

On the canonical repo (`Zious11/jira-cli`), `JR_E2E_ENABLED=true` will be set, so the
job runs normally and the badge reflects actual test results. Green = tests passed;
Red = tests failed. The badge is truthful on the canonical repo.

The README badge URL is hardcoded to `Zious11/jira-cli` — fork viewers always see the
canonical repo's badge regardless of which repo's README they are viewing.

On a fork that has NOT set `JR_E2E_ENABLED=true`, the job is skipped and the workflow
run is green. Since the README badge is hardcoded to the canonical repo, this does not
affect badge accuracy for users reading the canonical README.

---

## 3. Relationship to Existing E2E Spec

This spec **extends** `docs/specs/e2e-live-jira-testing.md`. It does NOT duplicate that
spec's content. Cross-references:

| This spec section | Updates in `e2e-live-jira-testing.md` (F4 doc-fallout) |
|-------------------|---------------------------------------------------------|
| §2.1 Two-Layer Model | §5 CI workflow YAML pseudocode — update the `if:` line; add preflight step; add `JR_E2E_ENABLED` note to the `vars`/`env` block. |
| §2.3 Repository Variable requirement | §6 Secret safety — add paragraph: `JR_E2E_ENABLED` is the primary fork-safety mechanism; explain it is a repository variable (not environment-scoped), evaluated before the job starts. |
| §2.3 + §4 Configuration table | §8 Configuration inventory — add a new row for `JR_E2E_ENABLED`. |

The updates are specified fully in §9 (F4 touch-point list) below.

---

## 4. Updated Configuration Inventory

The following row is added to the §8 table in `docs/specs/e2e-live-jira-testing.md` as part of F4 doc-fallout:

| Name | Kind | Where Set | Required | Notes |
|------|------|-----------|----------|-------|
| `JR_E2E_ENABLED` | Repository variable (`vars.*`) | **Repository level** (not environment-scoped) | Yes — canonical repo; forks default OFF | `vars.JR_E2E_ENABLED == 'true'` gates **both** `e2e.yml` and `e2e-sweeper.yml` jobs at scheduling time. MUST be a repository variable, not an environment variable (environment variables are not available in `jobs.<id>.if:`). Forks with this variable unset receive empty string → job skips cleanly on both workflows. Fork maintainers who provision their own `jira-e2e` environment may set this variable on their fork to opt in to both workflows. |

---

## 5. Rollout and Operational Notes

### 5.1 Canonical Repo Setup (one-time, by maintainer)

After merging the F4 PR:
1. Navigate to `https://github.com/Zious11/jira-cli/settings/variables/actions`.
2. Click **"New repository variable"**.
3. Name: `JR_E2E_ENABLED`, Value: `true`.
4. Save. (This is a **repository variable** — Settings → Secrets and variables → Actions →
   Variables tab → Repository variables. Do NOT create it inside the `jira-e2e` Environment
   settings page; environment-scoped variables are not available in `jobs.<id>.if:`.)
5. Verify by triggering a `workflow_dispatch` run on `develop` — the `e2e:` job (and the
   sweeper's `sweep:` job) should now start rather than showing "skipped".
6. The badge will reflect the first completed run result.

**Until step 4 is completed**, the `e2e:` and `sweep:` jobs skip on every push and nightly
cron even on the canonical repo. This is intended — the variable is the explicit opt-in.
The CI badge will show green (skipped run = success conclusion) until the variable is set
and a full test run completes.

**No change to the `jira-e2e` GitHub Environment** is required. It continues to hold the
secrets and environment-scoped variables as documented in `docs/specs/e2e-live-jira-testing.md §8`.

### 5.2 Fork Opt-In Flow

A fork maintainer who wants to run E2E against their own sandbox Jira instance:
1. Create a `jira-e2e` GitHub Environment on their fork with the secrets/variables from
   `docs/specs/e2e-live-jira-testing.md §8 + §10 (provisioning runbook)`.
2. Create a **Repository variable** `JR_E2E_ENABLED=true` in their fork's repo settings.
3. The E2E workflow will now run on their fork's `develop`/`main` pushes, nightly cron,
   and `workflow_dispatch`. They see their own E2E status, not the canonical repo's.

A fork that does NOT complete both steps continues to skip cleanly (zero CI minutes, green
skipped run).

### 5.3 Badge Token Rotation Interaction

The badge continues to reflect the most recent workflow run on `develop`. When the
`JR_E2E_API_TOKEN` secret expires (Atlassian 1-year cap, documented in
`docs/specs/e2e-live-jira-testing.md §9`), the nightly run will fail and the badge turns
red. This is the intended loud signal. The failure classification step in `e2e.yml` (the
`if: failure()` classify-failure step) provides the remediation message.

---

## 6. CLAUDE.md Doc-Fallout (F4 obligation)

The CLAUDE.md `AI Agent Notes` E2E section documents the `JR_*` environment variable table.
Per the doc-fallout rule codified by PRs #335/#357: **any new `JR_*` seam must be documented
in CLAUDE.md in the same commit as the code change.**

The following entry MUST be added to the `JR_*` table in CLAUDE.md's E2E section (F4):

> `JR_E2E_ENABLED` — GitHub Actions **repository variable** (`vars.JR_E2E_ENABLED`). Gates
> the `e2e:` job at scheduling time. NOT a Rust env var; never read by `src/` code. Forks
> with this variable unset skip cleanly (empty string != `'true'`). The canonical repo sets
> `JR_E2E_ENABLED=true` as a repository variable in GitHub repo settings (not environment-
> scoped). See `docs/specs/e2e-fork-safe-ci-enablement.md §2.3`.

Additionally document the two-layer relationship near the `JR_RUN_E2E` entry:

> **Two-layer E2E gate:** `JR_E2E_ENABLED` (workflow level, repo variable) determines
> whether the CI job starts; `JR_RUN_E2E` (test-binary level, process env) determines
> whether individual `#[ignore]` tests run. The workflow sets `JR_RUN_E2E=1` only when
> the job starts. Each layer independently provides fail-safe behavior.

---

## 7. Verification Properties

This feature introduces no new Rust code, no new behavioral contracts, and no new
verification properties in the formal sense (no VP-NNN). The verification obligations
for F4 and F6 are empirical CI confirmation tests:

### VER-E2E-FORK-1: Fork skip (gate OFF)

**Condition:** The `e2e:` job `if:` evaluates to `false` when `vars.JR_E2E_ENABLED` is
absent or not equal to `'true'`.

**Verification method (F4/F6):** Read the updated `e2e.yml` and confirm the `if:`
expression matches `github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'`
verbatim. Optionally: inspect a CI run on a fork or branch where the variable is unset;
confirm the job shows as "skipped" (not "failed") in the GitHub Actions run summary.

**Expected outcome:** job conclusion = skipped; workflow run = success (green badge).

### VER-E2E-FORK-2: Canonical repo runs (gate ON)

**Condition:** The `e2e:` job runs normally on the canonical repo when
`vars.JR_E2E_ENABLED=true` is set as a repository variable.

**Verification method (F4/F6):** After merging F4 PR and creating the repository variable,
trigger a `workflow_dispatch` run on `develop`. Confirm the job runs (not skipped), the
preflight step passes, and the full test suite executes.

**Expected outcome:** job runs, preflight passes, tests execute (may pass or fail
depending on live Jira state — the gate behavior is what is being verified, not test
results).

### VER-E2E-FORK-3: Skipped run produces passing badge

**Condition:** When the job is skipped (fork without `JR_E2E_ENABLED`), the workflow run
concludes as `success` and the badge renders green.

**Verification method (F6):** Inspect a CI run on develop immediately after the PR is
merged but before the repository variable is set (window of 0 before create). Confirm
badge state. Alternatively: review the GitHub Actions documentation confirming that an
all-skipped workflow run concludes as `success`. This is documented behavior, not an edge
case.

**Expected outcome:** badge URL renders green "passing" for a skipped run.

### VER-E2E-FORK-4: Preflight fails loud on missing config

**Condition:** When the job starts (gate passes) but a required variable is missing, the
preflight step fails immediately with a clear per-variable message.

**Verification method (F6):** In a test run (e.g., a `workflow_dispatch` with the
repository variable set but with one or more secrets temporarily renamed or absent),
confirm the preflight step exits 1 and the job step log shows a GitHub
`::error::E2E preflight failed — missing required config:` annotation followed by a
bulleted list of every absent variable. Confirm the job does not proceed to the Rust
build (the `Run live E2E tests` step is skipped/never reached).

**Expected outcome:** step fails (exit 1) with a `::error::` annotation listing ALL
missing variables in one shot; runner usage is minimal (no Rust compile). The
`Classify failure` step does NOT run (it is gated on `steps.run_e2e.conclusion == 'failure'`,
which is never reached when the preflight aborts).

---

## 8. F1 Decisions Encoded in This Spec

The following decisions were locked at the F1 human gate and are fully encoded above:

| Decision | Encoded in | Value |
|----------|-----------|-------|
| Gate mechanism = repo variable `JR_E2E_ENABLED` | §2.1, §2.2, §2.3 | `vars.JR_E2E_ENABLED == 'true'` at job level |
| `JR_E2E_ENABLED` MUST be repository-level variable (not environment-scoped) | §2.3 | Cited: GitHub docs, environment-level `vars` not available in `if:` |
| Secrets cannot gate `if:` | §2.3 | Secrets always empty in job-level `if:`; `vars` is correct mechanism |
| Final gate expression | §2.2 | `if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'` |
| Preflight step IS in scope | §2.4 | collect-all `missing=()` array + single `::error::` annotation listing all missing vars; required vars: BASE_URL, PROJECT, API_TOKEN, EMAIL |
| Badge is 2nd badge in README badge row | §2.5 | After `[![CI]...]`, before `[![Release]...]` |
| Badge markdown (normative) | §2.5 | `[![E2E](...badge.svg?branch=develop)](…/e2e.yml)` |
| BC corpus unchanged | §1 | 585 BCs; no new or modified BCs |
| NFR corpus unchanged | §1 | 41 NFRs; no new NFR (CI implementation detail below NFR catalog granularity) |
| All-skipped run = green badge | §2.5 | Confirmed — workflow run concludes `success`; badge renders passing |

---

## 9. F4 Implementation Touch-Point List

The following files are modified in F4. This list is normative for the implementing story.
No other files are touched.

| File | Change | Spec reference |
|------|--------|---------------|
| `.github/workflows/e2e.yml` | Replace `if: github.event_name != 'pull_request'` with `if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'` on the `e2e:` job. Add preflight step asserting `JR_E2E_BASE_URL`, `JR_E2E_PROJECT`, `JR_E2E_API_TOKEN`, `JR_E2E_EMAIL` via a collect-all `missing=()` array that emits a single `::error::` annotation listing every absent variable before exiting 1. Add `id: run_e2e` to the test step; gate classify-failure step on `steps.run_e2e.conclusion == 'failure'`. | §2.2, §2.4 |
| `.github/workflows/e2e-sweeper.yml` | Replace `if: github.event_name != 'pull_request'` with `if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'` on the `sweep:` job. Shares the same gate variable — no preflight needed (sweeper has no required-but-missing config path that wastes build minutes). | §2.2, §2.3 |
| `README.md` | Add `[![E2E](https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml/badge.svg?branch=develop)](https://github.com/Zious11/jira-cli/actions/workflows/e2e.yml)` as the 2nd badge in the badge row (after CI badge, before any release badge). | §2.5 |
| `CLAUDE.md` | Add `JR_E2E_ENABLED` entry to the `JR_*` env-var table in the AI Agent Notes E2E section. Add two-layer model note near `JR_RUN_E2E` entry. | §6 |
| `docs/specs/e2e-fork-safe-ci-enablement.md` | This file — copied into the branch so cross-references resolve on merge. §4 and §9 updated to include `e2e-sweeper.yml`. | (this spec) |
| `docs/specs/e2e-live-jira-testing.md` | §5 (YAML pseudocode): update job `if:` expression; show preflight step. §6 (Secret safety): add paragraph on `JR_E2E_ENABLED` as primary fork-safety mechanism; explain repository-variable requirement. §8 (Configuration inventory): add `JR_E2E_ENABLED` row per §4 table above; note that sweeper shares the gate. §10 (Provisioning runbook): add explicit step to create `JR_E2E_ENABLED=true` repository variable. | §3, §4 |
| `CHANGELOG.md` | Add `[Unreleased]` entry describing the fork-safe gate, sweeper gate, README badge, and maintainer rollout step. | §5.1 |

**Files NOT touched (confirmed):**
- `src/` (all files — zero Rust changes)
- `tests/e2e_live.rs`, `tests/e2e_cli_surface_guard.rs`, `tests/common/`
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- `Cargo.toml`, `Cargo.lock`, `deny.toml`, `.cargo/mutants.toml`
- `scripts/`, `.factory/specs/` (no BC, PRD, or architecture change)
- `BC-INDEX.md`, `CANONICAL-COUNTS.md`

**F4 delivery notes:**
- Zero Rust compilation is required for the delta itself. Running `cargo test` (non-E2E) confirms no accidental source changes.
- No Red Gate invocation; no demo story; no mutation testing run.
- Delivery is a single story (`S-E2E-FORK-1`, 2 SP) touching only YAML, Markdown, and documentation files.

---

## 10. References

- F1 delta analysis: `.factory/phase-f1-delta-analysis/e2e-fork-safe-enablement/delta-analysis.md`
- Brainstorming report: `.factory/planning/brainstorming-report.md`
- Existing E2E spec: `docs/specs/e2e-live-jira-testing.md`
- GitHub Actions — Variables (environment-level availability): https://docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/variables#defining-variables-for-multiple-workflows
- ADR-0004: Per-feature specs, not a growing master spec: `docs/adr/0004-per-feature-specs.md`
