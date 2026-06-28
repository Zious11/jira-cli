# cargo-mutants Policy

## Purpose

Mutation testing as a meta-verification layer on the bulk, create, ADF, and supporting modules.
Reference: F6 hardening review of PR #110-pr2 (2026-05-10); closes audit-followup #346.

Mutation testing catches a class of defect that line-coverage metrics miss: tests that
pass even when the implementation is silently broken by small code mutations (negated
conditions, removed returns, swapped operators). The scoped modules designated below had
high line coverage but untested assertion strength at the time of the F6 review.

## Scope

`cargo-mutants` runs against:
- `src/adf.rs` — ADF conversion core (`markdown_to_adf`, `adf_to_text`, `text_to_adf`); largest
  behavior-dense module with high weak-assertion surface across node normalization, pruning,
  mark deduplication, and the Algorithm B HTML block path (added F6 hardening)
- `src/cli/issue/create.rs` — `handle_edit_bulk_labels`, `handle_edit_bulk_fields`, `handle_jsm_create`, `parse_field_kv`
- `src/api/jira/bulk.rs` — `await_bulk_task`, polling loop, deadline propagation
- `src/types/jira/bulk.rs` — serde structs for bulk API responses
- `src/api/jsm/requests.rs` — `JsmRequestBuilder::build` (JSM POST body construction) (added S-288-pr4)
- `src/api/jsm/request_types.rs` — `list_request_types`, `get_request_type_fields` (added S-288-pr4)
- `src/cli/requesttype.rs` — `handle_list`, `handle_fields`, `resolve_request_type_id` (added S-288-pr4)
- `src/api/jira/issues.rs` — `search_issues`, `search_issue_keys` (anti-loop guard, `seen_keys` dedup,
  `has_more` sentinel, cursor-vs-offset pagination branch); `list_comments` (added MAINT-MUTANTS-GLOBS-01)
- `src/cache.rs` — TTL logic, per-profile path construction, model-a vs model-b error-handling split
  (`write_cmdb_fields_cache` / `write_object_type_attr_cache` swallow errors; others propagate)
  (added MAINT-MUTANTS-GLOBS-01)

Configured in `.cargo/mutants.toml::examine_globs`. The CI job relies on this
configuration alone (no `--file` CLI flags) for scope enforcement; `--in-diff` further
narrows to lines changed in the PR diff.

Note: cargo-mutants v27+ reads its config from `.cargo/mutants.toml` (not `.mutants.toml`
at repo root). This is the canonical config location for this project.

### Sibling Candidates Considered and Deferred (MAINT-MUTANTS-GLOBS-01)

These files were evaluated when `issues.rs` and `cache.rs` were added. Their dispositions
are recorded here so future reviewers know they were considered, not overlooked.

| File | Disposition | Rationale |
|------|-------------|-----------|
| `src/api/pagination.rs` | EXCLUDE | Simple serde structs + `items()` field accessor. No conditional logic or error-handling branches worth mutating; survivors would be caught by the broad integration test suite. Low payoff relative to baseline cost. |
| `src/jql.rs` | EXCLUDE | Already property-tested inline with proptest. Mutation survivors in JQL escaping/validation would almost certainly be caught by existing proptest strategies. |
| `src/api/jira/users.rs` | DEFER | Contains the `USER_PAGE_SIZE`-advance pagination workaround (JRACLOUD-71293 fix). Good candidate in principle, but test coverage via `tests/user_commands.rs` is limited — adding it without targeted pagination tests risks a noisy first-run kill rate. Revisit in a dedicated "users pagination hardening" cycle. |

## Kill-Rate Target

**90% on the PR diff scope.** The CI `mutants` job fails if the kill rate is below 90%.

Rationale: with the inline proptest from S-345 (BC-3.4.006) and the integration tests
in `tests/issue_bulk_pr2.rs` and `tests/issue_bulk.rs`, the bulk + create paths have
strong existing coverage. Mutation testing surfaces gaps where assertions are too loose.

The 90% threshold lives in the CI YAML `Check kill rate` step (not in `.cargo/mutants.toml`)
for CI-artifact visibility: reviewers can read the threshold without parsing TOML.

## Timeout Parameters (MUTATION-CI-TIMEOUT, 2026-06-28; corrected F5 adversarial pass)

### CONFIRMED CRITICAL — Previous Config Was Inverted

The F5 adversarial review pass identified a CRITICAL error in the previous version of this
section: `minimum_test_timeout` is a **floor** (lower bound), not a ceiling. Setting it to
120 could only *lengthen* timeouts for fast baselines — it cannot cap them. The corrective
analysis is in `.factory/research/cargo-mutants-timeout-keys-verification-2026-06-28.md`.

Verified facts from cargo-mutants 27.x source (verbatim doc-comment):
- `minimum_test_timeout` — *"Minimum test timeout, in seconds, as a floor on the autoset
  value."* Default is **20s**. It is a FLOOR, not a ceiling.
- The ONLY absolute per-mutant ceiling is the **`--timeout <SECS>` CLI flag**. There is
  NO `.cargo/mutants.toml` key for it (`test_timeout`/`timeout` do not exist as toml keys
  in 27.x; the field `test_timeout` in `Options` is populated solely from `args.timeout`).
- `--timeout` **supersedes** `timeout_multiplier` entirely (book: *"The multiplier only
  has an effect if the baseline is not skipped and if `--timeout` is not specified."*).
  Once `--timeout` is in the CI invocation, `timeout_multiplier` is dead config.
- Per-mutant timeout WITHOUT `--timeout`, with baseline measured:
  `effective = max(baseline × timeout_multiplier, minimum_test_timeout)` — unbounded above.

### Corrected Configuration

The fix removes the dead/misleading config keys and moves the ceiling to the CI invocation:

**`.cargo/mutants.toml`** — REMOVE both `minimum_test_timeout` and `timeout_multiplier`.
Both become dead config once `--timeout` is added to the invocation. Removing them avoids
misleading future readers into thinking either key controls a ceiling.

**CI invocation (`.github/workflows/ci.yml`, `Run mutation tests on PR diff` step)** —
ADD `--timeout 180` to the `cargo mutants` command line:

```
cargo mutants --in-diff "${DIFF_FILE}" --jobs 4 --timeout 180
```

**Local invocation (`CLAUDE.md` Build & Test section)** — add `--timeout 180` to match:

```
cargo mutants --in-diff "$DIFF_FILE" --jobs 4 --timeout 180
```

### Root Cause: Real Wall-Clock Sleeps in `bulk.rs` Scope

The `mutants` CI job for PR #553 (SEC-001, ADF recursion guard) was cancelled at exactly
60 minutes after evaluating 36 mutants from `src/adf.rs`. The root cause was **not** the
mutant count — it was the interaction between `src/api/jira/bulk.rs` being in
`examine_globs` and `tests/bulk_deadline_propagation.rs` using real wall-clock sleeps.

Key facts:
- `tests/bulk_deadline_propagation.rs` is a subprocess test (`assert_cmd::Command`) that
  drives `jr issue edit` against a wiremock server returning `HTTP 429 Retry-After: 60`
  indefinitely. It does this to test deadline propagation across a process boundary. It
  deliberately cannot use `tokio::time::pause` because `time::pause` is incompatible with
  subprocess + wiremock (tokio #4522, documented in the test file's module-level comment).
- The test's wall-clock budget is approximately 30–40 seconds per run.
- The per-mutant cost is **a full baseline test suite run**, not just the slowest single
  test. cargo-mutants runs the entire test suite per mutant (with `--all-features`).
- With a ~90s baseline and old `timeout_multiplier = 3.0`, the auto-derived per-mutant
  ceiling was ~270s. With no `--timeout`, the multiplier result was unbounded above as the
  baseline grows.

Detail: `.factory/phase-f1-delta-analysis/MUTATION-CI-TIMEOUT-delta-analysis.md` §2.1.

### Absolute Timeout Ceiling: `--timeout 180`

`--timeout 180` is passed on the `cargo mutants` command line as the absolute per-mutant
test ceiling.

**Value derivation:**
- The per-mutant cost approximates the FULL baseline test suite duration (~90s assumed).
- GitHub Actions ubuntu-latest runner performance variability adds ~10–20%: worst-case
  legitimate run ≈ 90s × 1.2 = ~108s.
- `--timeout 180` gives approximately 1.6–2× headroom over the worst-case legitimate run,
  which is adequate to avoid false-timeout flakiness on a now-REQUIRED gate.
- 180s still kills genuine async hangs far below the previous uncapped scenarios (~270s+).
- **Calibration caveat:** the 90s baseline assumption has not been confirmed on the GitHub
  Actions ubuntu-latest runner under `--all-features`. The first CI run on this branch
  will reveal the true baseline. If the "Check kill rate" step shows many `timeout`
  outcomes on otherwise-healthy mutants, increase `--timeout` incrementally (e.g., to 240).
  If the job consistently finishes in under 20 minutes, a tighter value (e.g., 120) could
  reduce average job time on large PRs — but prefer false-safety until the live baseline
  is confirmed.

**Why not a larger value (e.g., 300)?**
300s is the `--baseline=skip` fallback, which was the uncapped state we are trying to
escape. 180s provides a meaningful cap while remaining conservative enough not to false-
timeout on the bulk deadline propagation test.

**Why not a smaller value (e.g., 120)?**
120s was the previous (incorrect) belief about what `minimum_test_timeout` did. The
actual full-suite baseline is ~90s; with CI variance a 120s hard cap leaves only ~30s
headroom — tight enough to produce false timeouts on a slow runner.

### Multiplier Decision: REMOVE `timeout_multiplier` (Was 2.0)

`timeout_multiplier` is removed from `.cargo/mutants.toml` because:
- It is dead config once `--timeout 180` is in the CLI invocation (book: superseded).
- Retaining it creates a documentation debt: readers see a multiplier and assume it has
  effect, but it does not. Future maintainers may then reason incorrectly about the
  timeout model (the exact failure mode that caused the original CRITICAL).
- There is no "baseline-proportional fallback" value to preserve: the fallback when
  `--timeout` is absent is `--baseline=skip`'s 300s default, not `timeout_multiplier`.

If `--timeout` is ever removed from the invocation (e.g., during a Path B sharding
migration), reinstate `timeout_multiplier` at that time with a comment explaining its
role and its interaction with `--timeout`.

### Floor Decision: REMOVE `minimum_test_timeout` (Was 120)

`minimum_test_timeout = 120` is removed from `.cargo/mutants.toml` because:
- It is a FLOOR, not a ceiling. Setting it to 120 can only *lengthen* per-mutant timeouts
  when the baseline is tiny — a 120s floor is actively harmful on a fast-baseline project.
- The default floor (20s) is correct for this project: no legitimate test takes fewer than
  20s to fail a mutant, and we do not need to raise the floor.
- The combination of `minimum_test_timeout = 120` (floor) and no `--timeout` (no ceiling)
  was precisely the config that allowed the original unbounded hang: every mutant was
  guaranteed at least 120s to run, with no upper bound.

### CI Budget Model

With `--timeout 180`, `--jobs 4`, and the `--in-diff` PR diff scope:

Per-mutant cost is approximately `min(actual_suite_duration, 180)` seconds. For normal
non-hanging mutants the suite finishes in ~90s; hanging mutants are capped at 180s.

| PR scenario | Estimated mutants | Estimated wall-clock |
|-------------|-------------------|----------------------|
| SEC-001 scale (adf.rs recursion guards) | ~36 | ~14 min |
| Typical adf.rs PR | ~80 | ~30 min |
| Large adf.rs + bulk.rs PR | ~120 | ~45 min |
| Very large / multiple scoped files | ~200 | ~75 min (budget signal) |

Formula: `mutants / 4 jobs × ~90s avg` — the 90s average assumes a full-suite baseline;
hanging mutants add up to 180s each (capped), so a PR with many async hangs will skew
toward the upper bound. The 80-minute row above uses 90s avg (not 180s cap) because
most mutants in the adf.rs + cache.rs scope do not produce async hangs.

The CI job `timeout-minutes` is set to **90 minutes**. A PR generating 200+ mutants that
approaches or exceeds this budget is a signal to split the PR. See **Oversized-Diff
Signal** below.

### F-2: Cancelled Job Semantics on a Required Gate

A PR that generates 200+ mutants and causes the 90-minute job to be cancelled by GitHub
Actions produces a `cancelled` job status. The `ci-gate` condition checks for `failure`
OR `cancelled` — **both block merge**.

This is **intentional and correct** for a REQUIRED gate. A `cancelled` outcome means:
- The mutation run was incomplete.
- The 90% kill rate was never verified against the full diff scope.
- Merging would allow unverified mutations to reach `develop`.

The correct response to a `cancelled` mutants job is:
1. **Split the PR** into smaller, more targeted changes that generate fewer mutants.
2. **Admin bypass** (for emergency or release-branch situations) — the admin can merge
   over the ci-gate with the GitHub "Require approvals" bypass, acknowledging the
   incomplete mutation run explicitly in the PR description.

Do NOT increase `timeout-minutes` beyond 90 to accommodate oversized diffs. Do NOT
treat a budget-exceeded cancellation as a flaky check.

### `--baseline=skip` and Path B

When cargo-mutants runs with `--baseline=skip` (required for sharding), `timeout_multiplier`
is silently ignored and the test timeout falls back to **300s** per mutant (book verbatim:
*"The multiplier timeout options cannot be used when the baseline is skipped ... the test
timeout default of 300 seconds will be used."*).

**This Path-A design (retained baseline run) MUST NOT use `--baseline=skip`.** The
baseline run is required for `--timeout` to override the multiplier correctly (though
with `--timeout 180` in the invocation, this distinction is moot — `--timeout` applies
regardless of whether the baseline is skipped or not).

A future sharding effort (Path B) MUST pass `--timeout 180` (or a tuned value) explicitly
on every shard command, since `timeout_multiplier` is not available under `--baseline=skip`.
See research: `.factory/research/mutation-ci-perf-2026-06-28.md` §4.

## CI Gate: Required Check (MUTATION-CI-TIMEOUT, 2026-06-28)

### Promotion to Hard-Required

The `mutants` job is now **HARD-REQUIRED** via `ci-gate.needs`. Per DEC-096/097 and the
convention in CLAUDE.md, new required jobs are added to `ci-gate.needs` — never wired
directly into branch protection. This prevents the matrix-rename fragility class.

`ci-gate.needs` now includes `mutants`:

```yaml
needs: [fmt, clippy, test, msrv, deny, spec-guard, check-signing-workflow-injection, mutants]
```

### Push-Event Safety

The `mutants` job has `if: github.event_name == 'pull_request'`. On a push event to
`develop` or `main`, the job does not run and its result is `skipped`. The ci-gate
condition checks for `failure` or `cancelled` only — `skipped` is neither, so ci-gate
passes on push events. Push-to-develop behavior is unchanged.

### Oversized-Diff Signal

A PR that generates 200+ mutants and times out the 90-minute job **is not a flakiness
event** — it is a forcing function to keep PR diffs focused, consistent with the
`--in-diff` philosophy. The correct response is to split the PR into smaller, more
targeted changes. Do not increase `timeout-minutes` beyond 90 to accommodate oversized
diffs; do not treat a budget-exceeded cancellation as a flaky check.

### Timeout Semantics: Timeouts Count as Survived

Per cargo-mutants v27 convention, `timeout` outcomes are counted as survived mutants in
the kill-rate denominator. A 90%-kill-rate gate under this convention means: if async
hangs cause many timeouts on a large PR, the kill rate may fall below 90% even if all
reachable mutants are caught. This is the correct and intended behavior — it provides an
incentive to resolve async hang mutations (via `#[mutants::skip]` with justification, or
by refactoring the code to be mutation-testable) rather than silently ignoring them.

### F-3: Positive-Coverage Assertion (IN-SCOPE, MUTATION-CI-TIMEOUT)

**Problem (F-3 MEDIUM [process-gap]):** When the PR diff resolves empty via base-ref
drift (e.g., the feature branch was rebased but `git diff origin/...HEAD` produces an
empty diff against the resolved merge base), `--in-diff` generates 0 mutants, `cargo
mutants` exits 0 with no `outcomes.json`, and the current gate logic returns "OK: 0
mutants — clean PR". This is a **false-green** on a now-REQUIRED gate: it passes without
verifying any mutation coverage, even if the PR actually modifies `examine_globs` files.

**Decision: IN-SCOPE for this cycle.** The gate is now REQUIRED. A false-green from
base-ref drift on a scoped-file-touching PR is a correctness hole in a safety guarantee,
not a cosmetic issue. The harden is mechanical.

**Gate-logic addition to `ci.yml` (devops to implement):**

In the `Run mutation tests on PR diff` step, BEFORE calling `cargo mutants`, add a
positive-coverage check:

```bash
# Count how many lines in the diff touch examine_globs files.
# If the diff is non-empty but zero lines touch scoped files, this is a
# clean-scope PR (e.g. pure docs). If the diff IS non-empty AND touches
# scoped files, require outcomes.json to appear after cargo-mutants runs.
SCOPED_DIFF_LINES=$(git diff origin/${{ github.base_ref }}...HEAD -- \
    src/adf.rs \
    src/api/jira/bulk.rs \
    src/types/jira/bulk.rs \
    "src/cli/issue/create.rs" \
    src/api/jsm/requests.rs \
    src/api/jsm/request_types.rs \
    src/cli/requesttype.rs \
    src/api/jira/issues.rs \
    src/cache.rs \
  | wc -l)
echo "Scoped-file diff lines: ${SCOPED_DIFF_LINES}"
```

Then in the `Check kill rate` step, modify the zero-`outcomes.json` branch:

```bash
if [ ! -f mutants.out/outcomes.json ]; then
  if [ "$run_outcome" = "success" ]; then
    if [ "${SCOPED_DIFF_LINES:-0}" -gt 0 ]; then
      echo "FAIL: cargo-mutants exited 0 with no outcomes.json BUT diff touches scoped files."
      echo "      Possible base-ref drift (--in-diff resolved empty despite scoped-file changes)."
      echo "      Re-run the job, or check that the diff file was generated against the correct base."
      exit 1
    fi
    echo "OK: 0 mutants — diff does not touch any examine_globs files (clean/docs-only PR)."
    exit 0
  else
    echo "FAIL: cargo-mutants exited non-zero AND outcomes.json missing — harness crash."
    exit 1
  fi
fi
```

`SCOPED_DIFF_LINES` must be exported from the first step and made available to the second
(use `$GITHUB_ENV` or inline the count check within the same step block).

This check adds zero network calls and negligible time. It fires only on the degenerate
case of base-ref drift coinciding with scoped-file changes. False positives (legitimate
zero-mutant runs) are impossible when the file-list matches `examine_globs` exactly.

**Maintenance:** if `examine_globs` in `.cargo/mutants.toml` is updated, the file list
in the `SCOPED_DIFF_LINES` git diff command above MUST be updated in the same commit.

### Flakiness Risk Assessment

The flakiness risk of a required `mutants` job is moderate:

1. **GitHub Actions runner performance variability:** ubuntu-latest runners vary in CPU
   speed by ~10–20%. The `--timeout 180` absolute cap provides adequate headroom for the
   estimated ~90s baseline on the slowest plausible runner (~90s × 1.2 = ~108s, well under
   180s). If the live baseline proves higher than 90s, tune `--timeout` upward.
2. **crates.io download reliability:** `taiki-e/install-action` downloads cargo-mutants;
   `Swatenim/rust-cache` caches the binary after first install, limiting exposure.
3. **Parallel wiremock port contention:** parallel mutant runs start their own test
   processes and wiremock servers. This is the pre-existing behavior at `--jobs 4`; no
   new risk introduced by making the job required.
4. **Very large diffs (200+ mutants):** this causes a legitimate budget-exceeded
   cancellation, not flakiness. Treat as a split-PR signal (see Oversized-Diff Signal).

## Whitelist Convention

When a mutant cannot reasonably be killed — defensive guard, unreachable code, or a
performance-only change with identical observable behavior — annotate the function with
`#[mutants::skip]` AND include a justification comment IMMEDIATELY ABOVE the attribute.

Required format:

```rust
// mutants::skip: <one-line reason>
// Example: "defensive guard against impossible state; debug_assert! covers runtime invariant"
#[mutants::skip]
fn some_guard(...) { ... }
```

**Rules:**
- Bare `#[mutants::skip]` without a justification comment is **forbidden**. Code review
  MUST reject any PR that adds a bare whitelist attribute.
- The justification comment must be on the line(s) immediately preceding `#[mutants::skip]`.
- Valid justification categories:
  - Defensive guard for unreachable state (e.g., error branch that cannot be triggered
    through the public API under test)
  - Performance-only optimization (e.g., `with_capacity` hint) where the observable
    behavior is identical whether the hint is present or not
  - Debug-only assertion (e.g., `debug_assert!`) that does not run in release builds

Invalid justifications:
- "Tests don't cover this" — that is a gap to close, not a reason to skip
- "It's hard to test" — that is a refactoring opportunity, not a reason to skip

## Deferral Policy

The initial baseline PR (S-346) MUST NOT block on achieving 90% kill-rate on first run.
The intent is to land the CI gate; incremental improvement follows.

When the baseline reveals surviving mutants below 90%:

1. **Whitelist clearly-defensive mutants** per the convention above with justification comments.
2. **File one follow-up GitHub issue per uncovered-region cluster** (not per individual
   mutant). Title pattern: `chore(mutants): close surviving-mutant gap in <module> — N mutants`
3. **Track deferred follow-ups** via GitHub issues labeled `audit-followup` with
   issue numbers, links, and surviving mutant descriptions in the issue body.
4. **Subsequent PRs** incrementally close the gap by tightening assertions, adding
   targeted test cases, or whitelisting genuinely unkillable mutants.

The CI `mutants` job enforces 90% on the PR diff scope going forward. A PR that touches
the scoped files and scores below 90% on changed lines will fail CI.

## Local Invocation

Install (one-time):

```bash
cargo install cargo-mutants --locked
```

Full baseline on scoped files (uses `.cargo/mutants.toml` automatically):

```bash
cargo mutants --jobs 4 --timeout 180
```

PR-diff-equivalent run (matches CI scope):

```bash
DIFF_FILE=$(mktemp -t pr.diff.XXXXXX)
trap 'rm -f "$DIFF_FILE"' EXIT
git diff origin/develop...HEAD > "$DIFF_FILE"
cargo mutants --in-diff "$DIFF_FILE" --jobs 4 --timeout 180
```

Note: the `--file` flags are omitted above because `.cargo/mutants.toml` already
scopes via `examine_globs`. The `--in-diff` flag further narrows to lines changed in
the diff. Using both is redundant (CI uses `--in-diff` only).

The `--timeout 180` flag sets the absolute per-mutant test ceiling to 180 seconds.
This is the same value used in CI. See **Timeout Parameters** above for the derivation.

Single-file inspection:

```bash
cargo mutants --file src/api/jira/bulk.rs --jobs 4 --timeout 180
```

Results land in `mutants.out/` (excluded from git via `.gitignore`).

## CI Integration

The `mutants` job in `.github/workflows/ci.yml` runs on PRs only (not pushes to
`develop` / `main`). This is consistent with the `security` job pattern and keeps
mutation testing cost bounded to the PR review phase.

The canonical `cargo mutants` invocation is:

```
cargo mutants --in-diff "${DIFF_FILE}" --jobs 4 --timeout 180
```

- `--in-diff` scopes mutations to lines changed in the PR diff.
- `--jobs 4` runs four mutants in parallel.
- `--timeout 180` sets the absolute per-mutant test ceiling to 180 seconds (CLI-only; no
  equivalent `.cargo/mutants.toml` key exists for this parameter in cargo-mutants 27.x).

The job also includes a positive-coverage assertion (`SCOPED_DIFF_LINES` pre-check) that
guards against base-ref drift producing a false-green zero-mutant result. See
**F-3: Positive-Coverage Assertion** above for the exact gate logic.

Only mutants in code **changed by the PR** AND **in the scoped files** are tested
(`.cargo/mutants.toml::examine_globs` provides the file-scope; `--in-diff` narrows to
changed lines within those files). PRs that do not touch the scoped files generate zero
mutants; the kill-rate check exits 0 provided the positive-coverage assertion also passes.

The job `timeout-minutes` is set to **90** (increased from 60 in MUTATION-CI-TIMEOUT,
2026-06-28). See **CI Budget Model** above.

See `.factory/cicd-setup.md` §1.1a for the canonical CI job specification.

## Spec Anchor

The mutation gate is governed solely by this policy document (`docs/specs/cargo-mutants-policy.md`).
There is no dedicated BC (Behavioral Contract) for the mutation gate. The MUTATION-CI-TIMEOUT
drift item in STATE.md tracked the promotion-to-required change.

A BC could be authored — e.g., `BC-X.14.001: mutation-gate-required-check` — but the
policy spec provides sufficient governance for a CI-only behavior. The human explicitly
chose not to author a BC for this cycle (F1 §8, Q3 resolution: policy-doc-only). If the
mutation gate invariants need formal traceability in a future cycle, author a BC at that
time. For F7 traceability: the governing artifact is this file at the `docs/specs/` path,
not a PRD BC.

## Future Path: Job Sharding (Path B)

If a future cycle needs to further reduce CI wall-clock time (e.g., for very large
ADF-touching PRs or a widened `examine_globs` scope), the recommended approach is
job sharding via `--shard k/n` across a GitHub Actions matrix. Key requirements for
Path B, informed by research (`.factory/research/mutation-ci-perf-2026-06-28.md` §3–4):

1. Run a dedicated `mutants-baseline` job first (`cargo test --locked`) to prove the
   suite is green; shards then run with `--baseline=skip`.
2. Under `--baseline=skip`, `timeout_multiplier` is ignored and the test timeout falls
   back to 300s per mutant (book verbatim). Pass `--timeout 180` (or the current tuned
   value) explicitly on every shard command — do NOT rely on the multiplier or the
   `minimum_test_timeout` floor under `--baseline=skip`.
3. Wire a single **shard-aggregator job** (`needs: [all shards]`) into `ci-gate.needs`
   per DEC-096/097 — not the individual shard matrix jobs.
4. Pass the **same diff file** to every shard for correct `--in-diff` behavior.
5. The positive-coverage assertion (F-3, `SCOPED_DIFF_LINES`) must run in the
   aggregator job, not per-shard — only the aggregate outcomes.json reflects the full
   run.

Path B is deferred until Path A's 90-minute budget proves insufficient in practice.

## Changelog

| Date | Cycle | Change |
|------|-------|--------|
| 2026-06-28 | MUTATION-CI-TIMEOUT (F5 adversarial correction) | CRITICAL: corrected inverted timeout-mechanism documentation. `minimum_test_timeout` is a FLOOR not a ceiling; it and `timeout_multiplier` are REMOVED from `.cargo/mutants.toml` (dead config once `--timeout` is set). Moved the absolute per-mutant ceiling to `--timeout 180` on the CLI invocation. Derived 180s value with explicit reasoning (baseline ~90s + runner variance headroom). Documented F-2 (cancelled = blocking, intentional). Added F-3 positive-coverage assertion (in-scope: gate is now required; base-ref drift false-green is a correctness hole). Corrected budget model to use `--timeout 180` / `~90s avg`. Corrected Path B sharding guidance to remove `minimum_test_timeout` references. Updated Local Invocation commands to add `--timeout 180`. |
| 2026-06-28 | MUTATION-CI-TIMEOUT | Promoted `mutants` job to hard-required via `ci-gate.needs`. Raised job `timeout-minutes: 60 → 90`. Added (incorrectly) `minimum_test_timeout = 120` and `timeout_multiplier = 2.0` in `.cargo/mutants.toml` — both superseded by F5 correction above. |
| 2026-05-10 | F6 / S-346 | Initial policy established. Scope: bulk + create modules. Kill-rate target: 90%. `timeout_multiplier = 3.0`. Non-required (advisory) CI job. |
