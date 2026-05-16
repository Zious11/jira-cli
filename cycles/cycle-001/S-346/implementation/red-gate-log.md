# Red Gate Log — S-346

## Story
S-346: cargo-mutants CI job + whitelist policy for bulk + create modules

## Pattern
N/A — CI infrastructure delivery. No production code modified.
The "red gate" for THIS story is conceptual: future PRs that fail the
kill-rate threshold cause the new CI job to fail, blocking the merge.
The discriminator is exercised in future cycles, not this one.

## Baseline Outcome
- cargo-mutants version: 27.0.0 (installed via `cargo install cargo-mutants --locked`)
- Baseline run scope: 115 mutants found across 3 files
  (src/api/jira/bulk.rs, src/types/jira/bulk.rs, src/cli/issue/create.rs)
- Test suite baseline: 31s build + 112s test
- Auto-set test timeout: 338s (3x multiplier on 112s baseline test time)
- Partial results at commit time (full run still in progress as background task):
  - 26 caught, 0 missed, 4 timeout, 5 unviable out of 37 processed
  - Kill rate on killable mutants (caught/caught+missed): 26/26 = 100%
- Timeout pattern: mutations in async bulk polling loop cause
  global_profile_flag_targets_auth_status integration test (which calls
  live jr binary making live network calls) to hit the 338s timeout.
  This is the expected behavior documented in story spec's "likely surviving
  mutant patterns" section — timeout_multiplier=3.0 absorbs async sensitivity.
- Full report captured in worktree: docs/demo-evidence/S-346/baseline-mutants-report.txt
- Whitelist additions: 0 (#[mutants::skip] annotations) — no surviving mutants
  observed in the partial run.
- Follow-up issues filed: none — partial kill rate is 100% on observed mutants

## Key Config Decisions
- Config placed at `.cargo/mutants.toml` (cargo-mutants v27 default location)
  rather than `.mutants.toml` at repo root (story spec named it `.mutants.toml`
  but the tool's actual default path is `.cargo/mutants.toml`; adjusted per
  official docs per story spec's "verify against official docs" instruction).
- CI job uses `git diff ... > /tmp/pr.diff && cargo mutants --in-diff /tmp/pr.diff`
  (file-redirect form) instead of process-substitution `<(git diff ...)` for
  bash portability compatibility.

## Evidence
- docs/demo-evidence/S-346/baseline-mutants-report.txt (worktree)
- No deferred-followups.md needed (0 surviving mutants in partial run)
- worktree branch: feature/S-346-cargo-mutants-ci

## Adversary Pass 1 Fixes (applied 2026-05-16)

### CONCERN findings addressed

- **F1+F8:** Replaced `cargo install cargo-mutants --locked` (5-10 min cold rebuild per PR)
  with `taiki-e/install-action@aae1387a` (cargo-mutants-specific SHA, no `with:` needed).
  Same pattern as the existing cargo-llvm-cov install (e5de28ab). Zero new SHA surface.

- **F2:** Replaced `/tmp/pr.diff` (race-prone shared path on concurrent CI jobs) with
  `${{ runner.temp }}/pr-${{ github.run_id }}.diff` (unique per-run path).
  Note: cargo-mutants v27 `--in-diff` accepts file paths only, not git refs
  (tested locally — `cargo mutants --in-diff origin/develop` fails with "No such file").
  The file-redirect form is correct; the "ref form" in the adversary finding requires
  a file intermediate. Added `--jobs 4` (ubuntu-latest is 4 vCPU).

- **F3+F4 (POL-11 false-green elimination):** Added positive-coverage assertion.
  When PR diff touches scoped files but cargo-mutants generates 0 mutants, CI now
  fails explicitly with "FAIL: PR diff touches scoped files but cargo-mutants generated 0 mutants."
  Switched to `outcomes.json` parsing (jq) with fallback to `grep -c '' || true`
  for file counting (added `|| true` to suppress `grep` exit-1 on empty files under
  `bash -eo pipefail` which is GitHub Actions' default shell).

- **F5:** `--jobs 4` added to CI invocation.

- **F15:** CLAUDE.md Build & Test snippet updated from `<(git diff ...)` process
  substitution to `git diff ... > /tmp/pr.diff && cargo mutants --in-diff /tmp/pr.diff --jobs 4`.
  Portable across bash/zsh; process substitution doesn't work in fish.

- **F21:** fetch-depth comment corrected to describe `git diff origin/<base_ref>...HEAD`
  as the consumer of full history (not "cargo mutants --in-diff" which was misleading).

### NIT findings addressed

- **grep -c empty-file edge case (discovered during F3 implementation):** `grep -c ''`
  on an empty file exits 1 (no matches) even though it prints "0". GitHub Actions uses
  `bash -eo pipefail`; this would abort the step. Fixed with `|| true`. Not in the
  adversary's NIT list but discovered and fixed proactively.

- **taiki-e/install-action SHA reuse issue (discovered during F1 implementation):**
  The SHA `e5de28ab` for cargo-llvm-cov has `default: cargo-llvm-cov` in its action.yml.
  Using it with `tool: cargo-mutants` would override the default but is semantically
  fragile. Used the cargo-mutants-specific tag SHA `aae1387a` instead (same pattern
  as the coverage job, no `with:` needed).

### F6+F7 — Baseline evidence re-capture

- Fresh full baseline run initiated: `cargo mutants --jobs 4` at 2026-05-16T14:05:50Z
- Baseline test duration: 31s build + 272s test (higher than prior 112s due to parallel
  mutation jobs competing for CPU)
- Auto-set test timeout: 819s (3x of 272s)
- Interim at capture time (16/115 processed, ~9 min elapsed):
  - 16 caught, 0 missed, 0 timeout, 0 unviable
  - Kill rate: 16/16 = 100%
- PARTIAL run — follow-up issue #372 filed to complete the full 115/115 baseline
- Evidence files:
  - `docs/demo-evidence/S-346/baseline-mutants-report.txt` (updated with partial output + follow-up note)

### Verification

- cargo fmt --check: PASS
- cargo clippy --all-targets -- -D warnings: PASS
- cargo test: PASS (all tests green)
- YAML parse: PASS (yq + ruby)

## Adversary Pass 2 Fixes (applied 2026-05-16)

### BLOCKER findings addressed

- **F1 (BLOCKER):** Implemented actual 90% kill-rate gate. Previous logic used
  `if [ "${missed}" -gt 0 ] → exit 1` which is implicitly 100%. New logic:
  `kill_rate = (caught * 100) / (caught + missed + timeout)`. Timeouts count as
  survived per cargo-mutants v27 convention. If `killable == 0` (all unviable or
  no mutants), gate is skipped with OK message.

- **F2 (BLOCKER):** "Check kill rate" was dead code in the failure path. Fixed by
  adding `continue-on-error: true` to "Run mutation tests" step and `if: always()`
  to "Check kill rate" step. "Check kill rate" is now the sole pass/fail arbiter.

### CONCERN findings addressed

- **F3:** timeout and unviable now extracted from outcomes.json (primary path) and
  from `mutants.out/timeout.txt` / `mutants.out/unviable.txt` (fallback). Arithmetic
  uses `survived = missed + timeout`, `killable = caught + survived`.

- **F4+F5:** Replaced fragile file-presence regex heuristic (duplicated examine_globs)
  with `cargo mutants --in-diff "${DIFF_FILE}" --list 2>/dev/null | wc -l` for the
  positive-coverage assertion. This respects .cargo/mutants.toml examine_globs and
  only counts genuinely mutable lines — comment/doc-only PRs no longer trigger false
  positives. Verified locally: `--list --in-diff` is supported in v27 and returns
  empty output (0 lines) for diffs that don't touch scoped Rust code.

- **F8:** CLAUDE.md Build & Test snippet and docs/specs/cargo-mutants-policy.md
  Local Invocation section now use `mktemp -t pr.diff.XXXXXX` + `trap 'rm -f'`
  instead of `/tmp/pr.diff` (race-prone on concurrent shells). CI was already using
  `${{ runner.temp }}/pr-${{ github.run_id }}.diff`; docs are now in sync.

### NIT findings addressed

- **F9:** Added `command -v jq >/dev/null || { echo "FATAL: jq not found on PATH"; exit 1; }`
  guard at top of "Check kill rate" bash block, with comment noting jq is pre-installed
  on ubuntu-latest runner images.

- **F10:** baseline-mutants-report.txt NOTES section expanded with explicit partial-run
  caveat, prior-run summary, and explanation of new kill-rate formula post-F1+F2 fixes.

- **F11:** `${var:-0}` explicit defaults added after all jq/grep extractions to harden
  against empty command substitutions under `set -e` / `bash -eo pipefail`.

### Verification

- cargo fmt --check: PASS
- cargo clippy --all-targets -- -D warnings: PASS
- cargo test: PASS (all tests green)
- YAML parse (ruby): PASS
- `cargo mutants --list --in-diff $DIFF_FILE`: verified locally; 0 for docs-only diff

## Adversary Pass 3 Fixes (applied 2026-05-16)

### CONCERN findings addressed

- **F1 (CONCERN):** `docs/specs/cargo-mutants-policy.md` was self-contradictory.
  Lines 20-21 and 128 claimed CI used `--file` belt-and-suspenders scope enforcement,
  but Pass 2 removed `--file` in favour of `examine_globs` only (lines 108-110 correctly
  reflected the post-Pass-2 state). Removed the stale claims; policy doc is now
  internally consistent.

- **F2 (CONCERN):** Eliminated POL-11 false-green vector. The previous Check step
  re-invoked `cargo mutants --list --in-diff "${DIFF_FILE}"` to compute
  `expected_mutants`. If `git diff`, the `cargo mutants` binary, or `--list` mode
  silently failed, `expected_mutants=0` would bypass the harness-health gate — the same
  failure mode as the Run step. Replaced with a direct `outcomes.json` existence check:
  if `mutants.out/outcomes.json` is absent, the harness did not complete — CI now fails
  loudly. If it exists with `total_outcomes == 0`, the diff produced no mutants (clean
  PR — exit 0). Otherwise enforce the 90% kill-rate gate. The `cargo mutants --list`
  re-invocation is removed entirely.

### Verification

- cargo fmt --check: PASS
- cargo clippy --all-targets -- -D warnings: PASS
- cargo test: PASS (all tests green)
- YAML parse (ruby): PASS
- `grep -n '\-\-file' docs/specs/cargo-mutants-policy.md`: 3 lines, all legitimate
  (line 21 = new corrected claim "no --file CLI flags"; line 109 = local-invocation
  note; line 116 = single-file inspection example command)

## Adversary Pass 4 Fixes (applied 2026-05-16)

### CONCERN findings addressed

- **F1 (CONCERN):** Pass 3's `outcomes.json`-existence check conflated harness crash with
  zero-mutants clean PR — both produce missing `outcomes.json` on cargo-mutants v27.
  Replaced with 2x2 decision matrix on `(steps.run-mutants.outcome, outcomes.json)`:
  - `success + present`  → enforce 90% gate
  - `success + absent`   → clean PR (exit 0)
  - `failure + present`  → enforce 90% gate (gate catches missed)
  - `failure + absent`   → harness crash (FAIL, exit 1)
  Added `id: run-mutants` to "Run mutation tests" step so `steps.run-mutants.outcome`
  is addressable from the "Check kill rate" step.

- **F2 (CONCERN):** Stale comment in "Check kill rate" step described the removed
  `cargo mutants --list` positive-coverage mechanism (Pass 2 artefact). Replaced with
  comment describing the actual `steps.run-mutants.outcome` + `outcomes.json` gate logic.

- **Obs-3:** `cicd-setup.md:81` claim "configurable in `.cargo/mutants.toml`" dispatched
  to state-manager — not touched from worktree per task constraints.

### Verification

- cargo fmt --check: PASS
- cargo clippy --all-targets -- -D warnings: PASS
- cargo test: PASS (all tests green)
- YAML parse (ruby): PASS

## Adversary Pass 5 Fixes (applied 2026-05-16)

### Finding status

- **F1 (EMPIRICALLY REFUTED):** Claimed jq schema mismatch — that outcomes.json uses
  nested `.outcomes[] | select(.kind=="caught")` rather than top-level `.caught` scalar.
  Refuted by inspecting a local cargo-mutants v27 outcomes.json baseline: the file has
  top-level `caught`, `missed`, `timeout`, `unviable`, `total_mutants` scalars. The jq
  queries `.caught // 0` etc. are correct. No code change.

- **F2 (HIGH — fixed):** Malformed outcomes.json (truncated or OOM-killed mid-write)
  would silently default all counts to 0 via `jq '.caught // 0'` → false-green.
  Added `jq empty mutants.out/outcomes.json` parseability check immediately before the
  count queries. Malformed JSON now hard-fails with a clear "FAIL: malformed JSON" message.

- **F5 (MEDIUM, latent — fixed):** `grep -c '' file` on empty file prints "0\n" then
  exits 1. Under `bash -eo pipefail`, this triggered `|| echo 0`, producing a
  newline-embedded `"0\n0"` value that breaks subsequent bash arithmetic. Replaced with
  `wc -l < file` (exits 0 on empty file, prints clean "0"). Added `tr -d ' '` whitespace
  trim for macOS compatibility (wc -l pads with spaces on macOS, not on Linux).

- **F3+F4:** Dispatched to state-manager (spec back-sync tasks, not worktree scope).

### Verification

- cargo fmt --check: PASS
- cargo clippy --all-targets -- -D warnings: PASS
- cargo test: PASS (all tests green)
- YAML parse (ruby): PASS

## Worktree Commits
1. chore(S-346): add .gitignore + .cargo/mutants.toml config (3c35bdc)
2. chore(S-346): add mutants CI job (PR-only, --in-diff, scoped) (68466f5)
3. chore(S-346): cargo-mutants baseline run on scoped files (b9a85d8)
4. chore(S-346): adversary Pass 1 fixes — cached install + safer diff path + positive-coverage assertion (9329f3c)
5. fix(S-346): use cargo-mutants-specific SHA for taiki-e/install-action (b253f29)
6. fix(S-346): suppress grep -c exit-1 on empty files under bash -eo pipefail (7ec38ef)
7. chore(S-346): re-capture baseline evidence (Pass 1 F6+F7) — partial run (73be70b)
8. ci(S-346): adversary Pass 2 BLOCKERs + CONCERNs — 90% gate, dead-diagnostic-step fix, timeout arithmetic, mktemp safety (1b0bd3e)
9. ci(S-346): adversary Pass 3 fixes — policy.md --file dedupe + harness-health gate (no shared-failure false-green) (315f16b)
10. ci(S-346): adversary Pass 4 F1+F2 fixes — distinguish harness crash from zero-mutant clean PR (b3e0acd)
11. ci(S-346): adversary Pass 5 F2+F5 — JSON parseability check + wc -l fallback (6548050)

---

## Cycle Close-out (2026-05-16)

- PR #373 squash-merged to develop at SHA d909e65 on 2026-05-16T14:49:25Z
- **Total adversary passes:** 8 (Passes 1–8)
- **Fix rounds:** 5 (Passes 1–5 each required fixes; Passes 6/7/8 were consecutive CLEAN)
- **Final convergence:** 3 consecutive CLEAN passes (Passes 6, 7, 8)
- **Finding trajectory:** 0/6/14 → 2/6/4 → 0/3/3 → 0/2/4 → 2/3/3 (1 REFUTED) → 0/0/3 CLEAN → 0/0/0 CLEAN → 0/0/0 CLEAN
- **Pass 5 F1 empirical refutation:** Adversary CRITICAL claimed cargo-mutants v27 `outcomes.json` uses nested per-outcome objects (`.outcomes[] | select(.kind=="caught")`) rather than top-level scalars. Directly inspecting a locally produced `outcomes.json` via `jq 'keys'` showed top-level keys `caught`, `missed`, `timeout`, `unviable`, `total_mutants` — matching the existing jq queries. REFUTED; no code change.
- **Mutation-based Red Gate outcome:** S-346 is CI infrastructure (no production code changed). The "red gate" is conceptual — future PRs that fail the 90% kill-rate threshold will cause the new `mutants` CI job to fail and block the merge. All 10 CI checks on the PR itself (including the new `mutants` job running on its own diff) passed; the clean-PR path produced 0 mutants from the CI-only file change, which correctly exits 0 per the zero-mutants branch in the kill-rate step.
- **Copilot Review:** R1 = APPROVE; 2 non-blocking findings (S1 persist-credentials note, TD1 long CLAUDE.md line)
- **CI:** 10/10 green (fmt, clippy, test, msrv, deny, coverage, security, spec-guard, mutants, build)
- **Issue #346 CLOSED** via PR's `Closes #346` footer; confirmed CLOSED/COMPLETED
- **Follow-up #372 filed** to complete the partial mutation baseline (16/115 caught at PR merge; 99 mutants unprocessed)
- **Audit-followup cluster status:** ALL 3 CLEARED — #340 (PR #370), #345 (PR #371), #346 (PR #373)
