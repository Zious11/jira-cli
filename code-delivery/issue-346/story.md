---
document_type: story
story_id: "S-346"
title: "Add cargo-mutants CI job + whitelist policy for bulk + create modules"
wave: feature-followup
status: ready
priority: low
estimated_effort: small
tdd_mode: standard   # NOT strict — this is CI infrastructure, not test-driven production code
bc_anchors: []      # no BC anchor; this is meta-CI verification
holdout_anchors: []
nfr_anchors: []     # NFR-S-E (SHA-pinning hygiene), R-L12 (timeout gap) are READ-ONLY constraints, not anchors
adr_refs: []
sd_refs: []
files_modified:
  - .github/workflows/ci.yml (add `mutants` job; PR-only trigger; --in-diff <diff-file> mode; timeout-minutes: 60)
  - .gitignore (add mutants.out/)
  - CLAUDE.md (Build & Test: add `cargo mutants` invocation; AI Agent Notes: cargo-mutants is binary-only, not a dependency)
  - .cargo/mutants.toml (NEW; timeout_multiplier = 3.0; examine_globs scope for three designated files; v27 reads config from .cargo/mutants.toml, not .mutants.toml at repo root)
  - docs/specs/cargo-mutants-policy.md (NEW; #[mutants::skip] whitelist convention + justification requirement + deferral policy)
  - docs/demo-evidence/S-346/baseline-mutants-report.txt (NEW; partial baseline; follow-up #372 for completion)
test_files: []
breaking_change: false
producer: story-writer
version: "1.0.5"
last_updated: 2026-05-16
depends_on:
  - S-340     # S-340 is the immediate predecessor in the audit-followup cluster; bulk.rs is the primary mutation target
  - S-345     # S-345 built test discipline (proptest) that directly informs mutation-testing scope for create.rs
blocks: []
issue: 346
---

# S-346: Add cargo-mutants CI job + whitelist policy for bulk + create modules

## Context

The F6 hardening review of PR #110-pr2 (2026-05-10) flagged the absence of mutation
testing on the bulk and create modules. At that point the test coverage was high by
line-coverage metrics, but line coverage does not detect weak assertions — tests that
pass even when the implementation is silently broken by small code mutations (negated
conditions, removed returns, swapped operators).

S-340 (merged PR #370) and S-345 (merged PR #371) have since added inline proptest
coverage and pinned BC-3.4.009 and BC-3.4.006 respectively. S-346 builds on that
foundation by adding `cargo-mutants` as a meta-verification layer: a CI job that
mutates the designated files on every PR and enforces a 90% kill-rate target.

The three designated files are:
- `src/cli/issue/create.rs` — contains `handle_edit_bulk_labels`, `handle_edit_bulk_fields`
- `src/api/jira/bulk.rs` — contains `await_bulk_task`, polling loop, deadline propagation
- `src/types/jira/bulk.rs` — contains serde structs for bulk API responses

The `mutants` job spec is codified in `.factory/cicd-setup.md` §1.1a, which was updated
during F2 (2026-05-16) and serves as the canonical reference for implementation.

This story is an audit-followup in the same cluster as S-340 and S-345. It introduces
no production code changes. All deliverables are CI infrastructure, config, and docs.

**Post-Pass-2 state (back-sync note):** The F4 implementer empirically determined during
delivery that cargo-mutants v27's `--in-diff` requires a file path, not a git ref — the
ref-form (`--in-diff origin/<base_ref>`) fails with "No such file or directory". The
actual implementation writes the diff via `git diff origin/${{ github.base_ref }}...HEAD >
"$DIFF_FILE"` and passes `$DIFF_FILE` to cargo-mutants. Scope is enforced via
`.cargo/mutants.toml::examine_globs` rather than `--file` flags. Adversary Pass 2
findings F6 and F7 caught that cicd-setup.md and this story's AC-1 still documented the
pre-discovery forms; this v1.0.3 back-sync corrects both. Pass 2 also confirmed the 90%
kill-rate denominator semantics: `caught / (caught + missed + timeout)` with unviable
mutants excluded — this is now explicit in cicd-setup.md §1.1a.

## Behavioral Contracts

This story has no BC anchors. The 90% kill-rate target is a CI enforcement policy, not
a domain behavioral invariant. No existing BC contracts CI job shape or mutation-testing
thresholds. If surviving mutants from the baseline run reveal untested contract gaps in
`bulk.rs` or `create.rs`, new BCs would be anchored via follow-up issues at that time.

NFR-S-E (GHA SHA-pinning) and R-L12 (missing job timeouts) are READ-ONLY constraints
that this story must not worsen. The new `mutants` job satisfies R-L12 for itself by
setting `timeout-minutes: 60`. NFR-S-E is satisfied by using raw `cargo install`
in a `run:` step rather than a `uses:` action reference, introducing zero new SHA-pin
surface.

## Goal

Land a new `mutants` CI job in `.github/workflows/ci.yml` with supporting infrastructure:
`.cargo/mutants.toml` configuration, `.gitignore` exclusion for `mutants.out/`, CLAUDE.md
documentation additions, and `docs/specs/cargo-mutants-policy.md` codifying the
`#[mutants::skip]` whitelist convention. Run a baseline locally during implementation;
whitelist or defer surviving mutants per policy. Do NOT block the PR on achieving 90%
kill-rate on first baseline run.

## Acceptance Criteria

**AC-1** — New CI job `mutants` in `.github/workflows/ci.yml`:
- Triggers ONLY on `pull_request` via `if: github.event_name == 'pull_request'` at the
  job level (mirrors the existing `security` job pattern). Does NOT trigger on `push`
  to `develop` or `main`.
- Uses `taiki-e/install-action@<SHA>` with `tool: cargo-mutants` to install a prebuilt cargo-mutants binary (the cargo-mutants-specific release tag of an action ALREADY SHA-pinned elsewhere in this workflow for cargo-llvm-cov; reusing this action publisher minimizes new SHA-pin surface). Does NOT use `sourcefrog/cargo-mutants-action` (the vendor wrapper) — that would add a fundamentally new SHA. The prebuilt binary eliminates ~5-10 min cold rebuild per PR vs raw `cargo install`.
- Uses `Swatinem/rust-cache` (existing project precedent) for cargo install caching so
  the binary is not rebuilt on every CI run.
- Runs `cargo mutants --in-diff "$DIFF_FILE" --jobs 4` where `$DIFF_FILE` is
  `${{ runner.temp }}/pr-${{ github.run_id }}.diff` populated by
  `git diff origin/${{ github.base_ref }}...HEAD`. Scope enforced via
  `.cargo/mutants.toml::examine_globs` (no `--file` flags). Note: cargo-mutants v27's
  `--in-diff` requires a file path; the ref-form (`--in-diff origin/<base_ref>`) fails
  with "No such file or directory" and was rejected after F4 empirical verification.
- Enforces the 90% kill-rate target via an inline shell step using `jq` to parse `mutants.out/outcomes.json` (cargo-mutants v27 emits top-level scalar fields `caught`, `missed`, `timeout`, `unviable`, `total_mutants`). Falls back to `wc -l` on the per-status `.txt` files when `jq` is unavailable (defensive only — `jq` is pre-installed on ubuntu-latest runners). The threshold value lives in the CI YAML (not in `.cargo/mutants.toml`) for CI-artifact visibility.
- Sets `timeout-minutes: 60` at the job level (satisfies R-L12 for this job).
- All GHA actions used in the job are SHA-pinned (per StepSecurity convention from
  PR #368). Since the job uses only `actions/checkout` and `Swatinem/rust-cache` (both
  already SHA-pinned in the existing workflow), no new unpinned actions are introduced.

**AC-2** — `.cargo/mutants.toml` exists containing at minimum:
- `timeout_multiplier = 3.0` — prevents false "unviable" results on the async bulk
  code paths where mutations of `.await` chains can cause test timeouts.
- `examine_globs` TOML key lists the three designated files so local `cargo mutants`
  invocations without flags match CI behavior by default. (Note: `examine_globs` is the
  TOML config primitive in `.cargo/mutants.toml`; the equivalent CLI form would be
  `--file <path>` repeated, but the config-file form is preferred for parity between
  local and CI runs.)
- Kill-rate threshold NOT set in `.cargo/mutants.toml`; it belongs in the CI YAML step.

Note: cargo-mutants v27 reads configuration from `.cargo/mutants.toml`, not `.mutants.toml`
at the repo root. The F4 implementer adapted this per official docs; rationale captured in
the Red Gate log.

**AC-3** — `.gitignore` has a `mutants.out/` entry so local baseline runs do not
accidentally commit mutation results artifacts.

**AC-4** — `CLAUDE.md` is updated in two places:
- "Build & Test" section: new line showing the local invocation command
  (e.g., `cargo mutants --in-diff origin/develop  # Mutation testing on PR diff scope`).
- "AI Agent Notes" section: new line documenting that `cargo-mutants` is a binary tool
  installed via `cargo install cargo-mutants` in CI; it is NOT a Cargo dependency and
  MUST NOT be added to `[dev-dependencies]` or any other `Cargo.toml` section.

**AC-5** — `docs/specs/cargo-mutants-policy.md` exists and contains:
- The 90% kill-rate target and rationale (tracing back to the F6 hardening review of
  PR #110-pr2 that prompted this work).
- The `#[mutants::skip]` whitelist convention with a mandatory justification comment
  template showing the expected attribute usage (bare `#[mutants::skip]` without a
  justification comment is an anti-pattern under this policy).
- The deferral policy: surviving mutants below 90% on first baseline → file follow-up
  issues per uncovered-region cluster; whitelist only when justified (unreachable /
  defensive / performance-only mutations); do NOT block the initial S-346 PR on hitting
  the 90% threshold.
- Local invocation snippets matching the CLAUDE.md additions.

**AC-6** — Baseline `cargo mutants --in-diff origin/develop` (or scoped per file)
run executed locally during implementation. Results captured in
`docs/demo-evidence/S-346/baseline-mutants-report.txt`. If kill rate is below 90%,
follow-up issues filed per uncovered-region cluster (one issue per distinct cluster).

**AC-7** — No production source files modified. The single allowed exception: if the
implementer adds `#[mutants::skip]` annotations during baseline cleanup, those are
whitelist-pruning, not behavior changes, and must comply with the mandatory justification
comment convention in AC-5.

**AC-8** — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test` all pass with no new failures after the changes.

## Out of Scope

- Achieving 90% kill-rate on first PR (deferred to follow-up issues per F1 BA
  recommendation; the S-346 PR delivers the gate, not the pass).
- Mutation testing of any modules other than the three designated files
  (`src/cli/issue/create.rs`, `src/api/jira/bulk.rs`, `src/types/jira/bulk.rs`).
- Replacing or rearchitecting the existing test suites (mutation testing complements
  them, not replaces them).
- Modifying any existing CI jobs (fmt, clippy, test, msrv, deny, coverage, security).
  This story is purely additive.
- Extracting the `mutants` job to a separate `.github/workflows/mutants.yml` file
  (unnecessary at this scope; keeping in `ci.yml` ensures it appears in the same PR
  status check group).
- Adding `cargo-mutants` to `Cargo.toml` — it is a binary tool installed via
  `cargo install`, not a crate dependency.
- Schema correctness of any API call (tracked in issue #331 — out of scope).
- Modifying `.factory/cicd-setup.md` (F2 architect already updated this file).

## Implementation Notes

### CI YAML structure

Model the new job after the existing `security` job (same `if:` guard pattern). The
job structure is:

```yaml
mutants:
  name: Mutation testing
  runs-on: ubuntu-latest
  if: github.event_name == 'pull_request'
  timeout-minutes: 60
  steps:
    - uses: actions/checkout@<SHA>  # already pinned in ci.yml
      with:
        fetch-depth: 0              # needed for --in-diff to resolve base ref
    - uses: Swatinem/rust-cache@<SHA>  # already pinned in ci.yml
    - name: Install cargo-mutants
      run: cargo install cargo-mutants
    - name: Generate diff for --in-diff
      run: |
        DIFF_FILE="${{ runner.temp }}/pr-${{ github.run_id }}.diff"
        git diff origin/${{ github.base_ref }}...HEAD > "$DIFF_FILE"
        echo "DIFF_FILE=$DIFF_FILE" >> $GITHUB_ENV
    - name: Run mutation tests
      run: |
        cargo mutants --in-diff "$DIFF_FILE" --jobs 4
        # Scope enforced via .cargo/mutants.toml::examine_globs.
        # Note: v27 requires --in-diff to receive a file path; ref-form fails.
    - name: Check kill rate
      run: |
        caught=$(wc -l < mutants.out/caught.txt 2>/dev/null || echo 0)
        missed=$(wc -l < mutants.out/missed.txt 2>/dev/null || echo 0)
        total=$((caught + missed))
        if [ "$total" -eq 0 ]; then
          echo "No mutants generated (diff may not touch scoped files) — skip threshold check"
          exit 0
        fi
        pct=$((caught * 100 / total))
        echo "Kill rate: ${pct}% (${caught}/${total})"
        if [ "$pct" -lt 90 ]; then
          echo "ERROR: kill rate ${pct}% is below 90% threshold"
          exit 1
        fi
```

Note: `fetch-depth: 0` is required so `git diff` can resolve `origin/<base_ref>`.
Without it, shallow clones may not have the base branch ref available.

### Likely surviving mutant patterns (pre-emptive)

Based on the F1 architect input, three patterns are most likely to survive:

1. **Boolean bail-check in `handle_edit_bulk_labels`** — `if adds.is_empty() &&
   removes.is_empty()`. A mutation flipping `&&` to `||` could survive if integration
   tests do not exercise the case where exactly one of `adds`/`removes` is empty.
   The S-345 proptest uses `prop_assume!(!adds.is_empty() || !removes.is_empty())` on
   the pure builder function, but the bail check is in the caller. Verify that the
   bail-check path is reached by at least one integration test.

2. **`await_bulk_task` grace-period branch** — debug-only path exercised only by
   integration tests with `JR_BULK_UNKNOWN_GRACE_SECS` override. Mutation timeout
   sensitivity is the most likely source of a below-90% result. `timeout_multiplier =
   3.0` in `.cargo/mutants.toml` is the primary mitigation.

3. **`build_labels_edited_fields` object-vs-array branch** — `if label_ops.len() == 1`.
   The S-345 proptest should catch mutants here because it explicitly tests both the
   object-form (single-action) and array-form (both-action) output paths. Verify that
   cargo-mutants invokes proptest tests (it runs `cargo test` internally, which includes
   proptest; no special flag needed).

4. **`types/jira/bulk.rs` serde derives** — mutations to `#[serde(rename_all)]` or
   field name strings typically produce compile errors (unviable mutants), inflating the
   unviable count but not reducing the kill rate. Expect ~20-40 unviable mutants from
   this file.

### `#[mutants::skip]` whitelist convention

Per `docs/specs/cargo-mutants-policy.md` (AC-5), any `#[mutants::skip]` annotation
applied during baseline cleanup MUST have a justification comment on the same or
preceding line in the format:

```rust
// mutants::skip: <reason> (e.g., "unreachable under normal inputs; covered by property test X")
#[mutants::skip]
fn some_guard(...) { ... }
```

Bare `#[mutants::skip]` without a justification comment is prohibited by the policy.
Each whitelisted case also requires a `track-debt` entry per the deferral policy.

### `fetch-depth: 0` for `--in-diff`

The `actions/checkout` step in the `mutants` job must use `fetch-depth: 0` to enable
`git diff origin/${{ github.base_ref }}...HEAD` (used to write `$DIFF_FILE` before
cargo-mutants is invoked). A shallow clone (the default `fetch-depth: 1`) may not have
the base branch ref available for diff computation, causing the diff step to fail or
produce an empty file, which would cause cargo-mutants to generate zero mutants unexpectedly.

## TDD Plan

1. Add `.cargo/mutants.toml` with `timeout_multiplier = 3.0` and file scope.
2. Add `mutants.out/` line to `.gitignore`.
3. Install cargo-mutants locally: `cargo install cargo-mutants`.
4. Run local baseline: `cargo mutants` (scope enforced via `.cargo/mutants.toml::examine_globs`).
   For diff-scoped local runs, write a diff first:
   `DIFF=$(mktemp -t pr.diff.XXXXXX) && git diff origin/develop...HEAD > "$DIFF" && cargo mutants --in-diff "$DIFF"`.
   Capture output to `docs/demo-evidence/S-346/baseline-mutants-report.txt`.
   Note: do NOT pass a git ref directly to `--in-diff`; v27 requires a file path.
5. Review surviving mutants:
   - Genuine test gap that is easy to close: add a tighter assertion or targeted test.
   - Defensive / unreachable / performance-only: add `#[mutants::skip]` with mandatory
     justification comment per policy.
   - Non-trivial fix needed: file a follow-up GitHub issue.
6. Add the `mutants` job to `.github/workflows/ci.yml`.
7. Update `CLAUDE.md` (Build & Test + AI Agent Notes sections).
8. Write `docs/specs/cargo-mutants-policy.md`.
9. Verify locally: `cargo fmt --check && cargo clippy --all-targets -- -D warnings
   && cargo test` (satisfies AC-8).

## Token Budget Estimate

| Component | Estimated tokens |
|-----------|-----------------|
| Story spec (this file) | ~4,500 |
| `.factory/cicd-setup.md` §1.1a (reference spec for CI job) | ~600 |
| `.github/workflows/ci.yml` (read to match existing job structure + SHA pins) | ~800 |
| `CLAUDE.md` (read Build & Test + AI Agent Notes sections to find insertion points) | ~600 |
| Baseline run output (captured in baseline-mutants-report.txt) | ~500 |
| `cargo test` output verification | ~300 |
| **Total** | **~7,300** |

Well within the 20-30% agent context window budget for a small story. The baseline run
is the wall-clock wildcard (5-15 minutes), but the token cost of reading its output
is bounded.

## Previous Story Intelligence

**S-340** (merged PR #370, 2026-05-15) pinned the `task_id`-in-bulk-poll-timeout-message
behavioral contract (BC-3.4.009) with a regression test in
`tests/bulk_deadline_propagation.rs`. The `await_bulk_task` function in `src/api/jira/bulk.rs`
is one of the three mutation targets for S-346. S-340's test is part of the mutation
harness that cargo-mutants will run.

**S-345** (merged PR #371, 2026-05-16) extracted `build_labels_edited_fields` into a
pure function in `src/cli/issue/create.rs` and added an inline proptest covering BC-3.4.006
invariants 1-5. The proptest is part of the mutation harness for the `create.rs` target.
The `if label_ops.len() == 1` branch in `build_labels_edited_fields` is the key mutation
target covered by S-345's proptest — verify during baseline that cargo-mutants kills the
boolean-flip mutant for that condition.

**S-1.01** (merged PR #295) established SHA-pinning for all GHA actions in `ci.yml` and
`release.yml`. The `mutants` job must use the already-pinned SHAs for `actions/checkout`
and `Swatinem/rust-cache` rather than introducing new floating tags.

**S-1.04** (merged PR #298) added `timeout-minutes:` to all existing CI jobs. The
`mutants` job sets its own `timeout-minutes: 60`, consistent with S-1.04's convention.

## Architecture Compliance Rules

(Extracted from architect-input-346.md and existing codebase conventions.)

1. **PR-only trigger**: The `mutants` CI job MUST use `if: github.event_name ==
   'pull_request'` at the job level. It MUST NOT trigger on `push` to `develop` or
   `main`. Blast radius is bounded to the PR review phase.

2. **No new SHA-pin surface**: The `mutants` job MUST use `taiki-e/install-action` with `tool: cargo-mutants` (the cargo-mutants-specific release SHA pinned in this workflow). MUST NOT use `sourcefrog/cargo-mutants-action` (the vendor wrapper introduces a new SHA-pin surface; taiki-e is an already-pinned action publisher reused here for caching). Any other GHA action references in the job must already be SHA-pinned in the existing workflow (i.e., `actions/checkout` and `Swatinem/rust-cache`).

3. **No Cargo.toml modification**: `cargo-mutants` MUST NOT be added to `Cargo.toml`
   as a `[dev-dependencies]` entry or in any other section. It is a binary tool
   installed via `cargo install`, not a crate dependency.

4. **`timeout-minutes` required**: The new CI job MUST set `timeout-minutes: 60` at
   the job level (existing convention from S-1.04).

5. **Whitelist justification required**: The `docs/specs/cargo-mutants-policy.md` MUST
   establish that bare `#[mutants::skip]` without a justification comment is an
   anti-pattern. Any `#[mutants::skip]` annotation applied during baseline cleanup
   is a policy violation without an adjacent justification comment.

6. **Kill-rate threshold in CI YAML, not in `.cargo/mutants.toml`**: The 90% threshold value
   lives in the CI step's shell logic (most visible to reviewers), not in `.cargo/mutants.toml`.

7. **`fetch-depth: 0`**: The `actions/checkout` step in the `mutants` job MUST include
   `fetch-depth: 0` so `git diff origin/${{ github.base_ref }}...HEAD` can resolve the
   base ref when writing `$DIFF_FILE`. cargo-mutants v27 requires `--in-diff` to receive
   a file path; the git ref is consumed by the preceding diff step, not passed directly
   to cargo-mutants.

8. **No suppression**: Do not add `#[allow(...)]` to work around clippy warnings on new
   code. Per project zero-suppression policy.

## Library & Framework Requirements

No new Cargo dependencies. The story introduces only CI/CD tooling and documentation.

| Tool | Version | Install method |
|------|---------|---------------|
| `cargo-mutants` | latest stable (`cargo install`) | CI `run:` step; never `Cargo.toml` |

Existing project pins used:
| Dependency | Existing version | Notes |
|------------|-----------------|-------|
| `serde_json` | (workspace pin) | mutation target via `build_labels_edited_fields` |
| `proptest` | `"1"` (dev-dep) | already in harness; cargo-mutants runs `cargo test` which includes proptest |
| `tokio` | (workspace pin) | present in bulk async paths; `timeout_multiplier = 3.0` mitigates timeout sensitivity |

Do not add any new Cargo dependencies.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `.github/workflows/ci.yml` | MODIFY | Add `mutants` job (~40-50 lines). Job uses `if: github.event_name == 'pull_request'`; uses existing SHA-pinned checkout + rust-cache; writes diff to `$DIFF_FILE` via `git diff origin/${{ github.base_ref }}...HEAD`; installs cargo-mutants via `cargo install`; runs `--in-diff "$DIFF_FILE" --jobs 4` (v27 file-path form; scope via `.cargo/mutants.toml::examine_globs`); enforces 90% kill-rate via shell one-liner; sets `timeout-minutes: 60`. |
| `.gitignore` | MODIFY | Add `mutants.out/` line. Location: near end of file, after existing build artifact exclusions. |
| `CLAUDE.md` | MODIFY | (1) Build & Test section: new `cargo mutants` invocation line. (2) AI Agent Notes section: new line documenting cargo-mutants as binary-only, never a Cargo.toml dependency. |
| `.cargo/mutants.toml` | CREATE | Minimal config: `timeout_multiplier = 3.0` + `examine_globs` scope for three designated files. Note: v27 reads from `.cargo/mutants.toml`, not `.mutants.toml` at repo root. |
| `docs/specs/cargo-mutants-policy.md` | CREATE | Policy doc: 90% kill-rate target + rationale; `#[mutants::skip]` convention with mandatory justification template; deferral policy for initial-baseline misses; local invocation snippets. |
| `docs/demo-evidence/S-346/baseline-mutants-report.txt` | CREATE | Baseline run output captured during F4. Contains kill rate, caught/missed/unviable counts, surviving mutant list. |
| `Cargo.toml` | DO NOT TOUCH | cargo-mutants is not a Cargo dependency. |
| `src/` (all production source) | DO NOT TOUCH unless baseline reveals surviving mutants that warrant `#[mutants::skip]` with justification; even then, whitelist-only — no logic changes. |
| `tests/` (all test files) | DO NOT TOUCH | Existing test suite serves as the mutation harness without modification. |
| `.factory/cicd-setup.md` | DO NOT TOUCH | F2 architect already updated this file (commit 44e2b76). |
| `.factory/specs/prd/BC-INDEX.md` | DO NOT TOUCH | No new BC; no in-place BC extension. |

## References

- Issue #346: `chore(ci): add cargo-mutants to CI scoped to bulk + edit modules with 90% kill-rate target`
- F1 delta analysis: `.factory/phase-f1-delta-analysis/delta-analysis-346.md`
- F1 architect input: `.factory/phase-f1-delta-analysis/architect-input-346.md`
- F1 BA input: `.factory/phase-f1-delta-analysis/business-analyst-input-346.md`
- CI/CD audit (§1.1a canonical spec): `.factory/cicd-setup.md`
- S-340 predecessor (task_id pin, bulk.rs): `.factory/code-delivery/issue-340/story.md`
- S-345 predecessor (proptest, create.rs): `.factory/code-delivery/issue-345/story.md`
- S-1.01 (SHA-pinning convention): `.factory/stories/wave-1/S-1.01-pin-github-actions-shas.md`
- S-1.04 (timeout convention): `.factory/stories/wave-1/S-1.04-ci-job-timeouts.md`
- PR #110-pr2 (origin of audit-followup): `.factory/code-delivery/issue-110-pr2/`
- StepSecurity SHA-pinning precedent: PR #368
- Schema-correctness deferred: issue #331
