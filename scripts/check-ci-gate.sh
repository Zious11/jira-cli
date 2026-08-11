#!/usr/bin/env bash
# check-ci-gate.sh — S-CIGATE-2 fail-closed `ci-gate` needs-result evaluator
#
# PURPOSE: `.github/workflows/ci.yml :: ci-gate` is the sole required
# branch-protection status check on `develop`/`main`. Its old inline
# condition (`contains(needs.*.result, 'failure') ||
# contains(needs.*.result, 'cancelled')`) is an ALLOWLIST OF KNOWN-BAD
# VALUES: `needs.<job>.result` can be `success`, `failure`, `cancelled`, or
# `skipped` today, and `skipped` satisfies neither `contains()` call — so a
# job that never ran (e.g. `mutants` on a push event, by design) silently
# makes the gate report green. Confirmed reachable on every push via live CI
# run 30465686049. This script replaces that inline condition (Option C,
# S-CIGATE-2, human-approved over the rejected Option A/Option B — see
# `.factory/stories/S-CIGATE-2-skipped-status-false-green.md`).
#
# FAIL-CLOSED DESIGN: only `success`, or `skipped` for a job named in the
# restrictive `ALLOWED_SKIPS` allowlist below, passes. Every other value —
# `failure`, `cancelled`, an unlisted `skipped`, or any result string this
# script has never seen before (a hypothetical future GitHub Actions
# conclusion type) — fails via a DEFAULT ARM, not an enumerated list of
# known-bad values. This is the structural inversion that fixes the shape
# of the original defect: an allowlist of known-bad values lets unknown
# future values pass through unnoticed; a default-fail arm cannot.
#
# ALLOWED_SKIPS is restrictive, not blanket: a listed job still fails the
# gate on `failure`/`cancelled` — the carve-out tolerates `skipped` ONLY.
# It currently contains `mutants` only (the sole `ci-gate.needs` member that
# carries a job-level `if: github.event_name == 'pull_request'` today, and
# therefore reports `skipped` on every push by design — see
# `.github/workflows/ci.yml :: mutants`, unchanged by this story).
#
# TOOLING CHOICE: `jq` — pre-installed on `ubuntu-latest`, already an assumed
# dependency of this exact file (`.github/workflows/ci.yml :: mutants §
# "Check kill rate"` uses it). No new tooling assumption is introduced.
#
# INPUT: JSON payload shaped like GitHub Actions' `toJSON(needs)`, e.g.:
#   {"fmt":{"result":"success",...},"mutants":{"result":"skipped",...}}
# Read from stdin in normal (non-self-test) mode.
#
# EXIT CODES:
#   0 — every job passed (success, or allowlisted-skipped)
#   1 — at least one job failed the gate (see per-job OK/FAIL log lines), or
#       the `needs` JSON was empty
#   2 — missing `jq`, malformed input (not valid JSON), input JSON that is
#       valid but not a top-level object (e.g. an array), or an internal jq
#       failure while extracting job names (should not happen once the two
#       checks above pass — reported distinctly rather than folded into the
#       empty-needs case)
#
# SELF-TEST: pass --self-test to run the built-in fixture suite (proves the
# decision logic is not a no-op; modeled on
# scripts/check-signing-workflow-injection.sh's --self-test convention,
# DEC-148/DEC-150 pattern). Wired into `spec-guard` (NOT `ci-gate` — a gate
# cannot depend on a job that depends on it).
#
# --print-allowed-skips: emits one job name per line from the ALLOWED_SKIPS
# array, exactly as bash itself evaluates it (declaration, any `+=`
# appends, quoting style — whatever bash actually honors), and nothing
# else. `tests/ci_gate_completeness.rs` shells out to this instead of
# re-parsing this file's source text with a bespoke parser, because a
# form-specific text parser (regex/line-matcher for one declaration shape)
# can desync from what bash actually does with a DIFFERENT valid
# declaration form (`+=` append, multi-line, `${OTHER[@]}` expansion, etc.)
# — there is no second parser to disagree with bash if bash is the one
# asked.
#
# USAGE:
#   echo "$NEEDS_JSON" | scripts/check-ci-gate.sh   # canonical CI invocation
#   scripts/check-ci-gate.sh --self-test            # offline fixture suite
#   scripts/check-ci-gate.sh --print-allowed-skips  # emit ALLOWED_SKIPS, one per line

set -euo pipefail

# Explicit syntax self-check — repo convention shared with
# scripts/check-signing-workflow-injection.sh, check-bc-citation-symbols.sh,
# and check-cargo-mutants-policy-citations.sh (the latter two even pin this
# line's presence via their own --self-test grep count). Honest note: for
# THIS script's own control flow it is not strictly load-bearing — every
# function below is fully defined (and therefore syntax-checked by bash's
# own sequential parser) before `main "$@"` at the bottom ever runs, so a
# syntax error anywhere in this file would surface before any real work
# happens even without this line. Kept for consistency with the sibling
# scripts' convention and because it gives one explicit, unambiguous
# syntax-error message up front rather than relying on that incidental
# ordering.
bash -n "${BASH_SOURCE[0]}"

# ---------------------------------------------------------------------------
# Self-test fixed-denominator pins (S-626-1 ADV-P61-INFO-006).
#
# EXPECTED_FIXTURES and EXPECTED_JQ_TRUST_CHECKS were previously declared
# `readonly` from INSIDE run_self_test()/run_jq_trust_self_test() without
# `local` — a bare `readonly NAME=val` inside a bash function still
# creates a GLOBAL readonly variable, not a function-local one. That is
# harmless today only because `main --self-test` calls each function
# exactly once per process; a second call in the same shell (e.g. a
# future test harness invoking these functions directly, the way this
# file's own doc comments already describe `is_trusted_jq_dir()` being
# unit-tested directly) would abort on the second `readonly` assignment
# under `set -e`. Declared here at file scope instead — assigned exactly
# once when this file is parsed, regardless of how many times any
# function below is called.
readonly EXPECTED_FIXTURES=13
readonly EXPECTED_JQ_TRUST_CHECKS=17

# ---------------------------------------------------------------------------
# ALLOWED_SKIPS — restrictive per-job carve-out (S-CIGATE-2 AC-002).
#
# A job named here may ADDITIONALLY report `skipped` and still pass the
# gate. It still fails the gate on `failure`/`cancelled` — this list does
# NOT grant blanket immunity, only tolerance for the one specific `skipped`
# outcome.
#
# When adding a new job to `ci-gate.needs` in ci.yml that can legitimately
# report `skipped` (e.g. a future PR-only or repo-variable-gated job), add
# it here too — otherwise the gate will (correctly) start failing that
# job's push-event runs. See CLAUDE.md's `ci-gate` Conventions bullet.
#
# THIS ARRAY IS A TRUST BOUNDARY, not a convenience list: every job named
# here is granted permission to report `skipped` and still pass the sole
# required branch-protection check. Widening it to a job with no legitimate
# reason to skip (e.g. `test`, `deny`, `clippy`) would make this gate
# STRICTLY WEAKER than the retired inline condition it replaced — the
# mirror image of the false-green defect this script exists to fix.
#
# PR #671 REVIEW HISTORY (rounds 3-6) — read before touching this array.
# The enforcement is BEHAVIORAL (round 3): `tests/ci_gate_completeness.rs::
# test_ci_gate_decision_matches_job_level_if_for_every_needs_member` runs
# THIS SCRIPT (a real bash subprocess) against a synthesized payload for
# every `ci-gate.needs` job and asserts the gate's actual exit code
# matches an EXPECTED decision. That part has survived every round intact.
#
# What CHANGED across rounds 4-6 is how "expected decision" is computed.
# Every attempt to answer "does this job's `ci.yml` `if:` expression MEAN
# something legitimate?" from source text was bypassed:
#   - presence-only (any job-level `if:` at all) — accepted a no-op like
#     `if: ${{ always() }}`.
#   - no-op blacklist + "references an event/config marker" requirement
#     (round 5, "M1") — a trailing YAML comment defeated BOTH halves at
#     once (`if: ${{ always() }}  # not gated on vars.SOMETHING`), AND
#     independently, the blacklist only covered always-TRUE no-ops —
#     `if: ${{ github.ref == 'refs/heads/does-not-exist' }}` (an
#     always-FALSE, permanently-skipped condition) was structurally
#     unreachable for it.
# CONCLUSION (round 6): that question is undecidable from source text by
# pattern-matching. `tests/ci_gate_completeness.rs` no longer asks it.
# Instead, `PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` there holds one
# human-reviewed, EXACT `if:` expression per job permitted to be in
# ALLOWED_SKIPS; the "expected decision" for a job is now "does its
# ci.yml `if:` text match its pin, byte-for-byte after narrow
# normalization?" — a decidable string comparison, not a judgment about
# meaning. Adding a job here REQUIRES adding a matching pinned entry in
# the SAME change, or the behavioral test fails loudly, naming the job.
#
# Two earlier, weaker guards remain as fast diagnostics (they point
# directly at ALLOWED_SKIPS when it IS the cause) but are NOT sufficient on
# their own — round 3 proved both bypassable by ordinary constructions
# (a subscripted `ALLOWED_SKIPS[9]=...` assignment inside `evaluate_needs`,
# and a parallel array read alongside `"${ALLOWED_SKIPS[@]}"` inside
# `is_allowed_skip`) that never touch either guard's textual pattern:
# `tests/ci_gate_completeness.rs::test_allowed_skips_members_require_job_level_conditional_in_ci_yml`
# shells out to `--print-allowed-skips` (asks bash for the array's PRINTED
# value, which a control-flow-based bypass can desync from), and
# `test_allowed_skips_has_exactly_three_code_level_references` counts three
# specific textual shapes (`ALLOWED_SKIPS=`, `ALLOWED_SKIPS+=`,
# `${ALLOWED_SKIPS`) in the source, which misses e.g. `ALLOWED_SKIPS[9]=`,
# `declare -n`, `read -a`, or `mapfile -t`.
#
# SCOPE BOUNDARY (PR #671 review round 10, reasoned not run): this script
# and its behavioral test both execute this file's actual bytes, so no
# mutation of THIS script's logic can differ between a local `cargo test`
# run and the real gate. A mutation keyed on something that legitimately
# DIFFERS between those two environments is outside that guarantee — e.g.
# `[ "${GITHUB_JOB:-}" = "ci-gate" ] && return 0` inside `is_allowed_skip`
# would pass every local test (where `GITHUB_JOB` is unset, or set to
# `test`, by the Rust test harness) yet fire in the real `ci-gate` job
# (where `GITHUB_JOB=ci-gate`). No artifact in this suite claims to cover
# that class, so this is a stated scope boundary, not a false claim like
# the CRITICALs above — recorded here so it stays a documented boundary
# rather than an implicit gap someone has to rediscover.
# ---------------------------------------------------------------------------
ALLOWED_SKIPS=("mutants")

# is_allowed_skip <job_name> — returns 0 (true) if job_name is in
# ALLOWED_SKIPS, 1 (false) otherwise.
is_allowed_skip() {
    local job="$1"
    local allowed
    for allowed in "${ALLOWED_SKIPS[@]}"; do
        if [ "${allowed}" = "${job}" ]; then
            return 0
        fi
    done
    return 1
}

# print_allowed_skips — emits one job name per line from ALLOWED_SKIPS, as
# bash itself evaluates it at the point this function runs (i.e. after
# every declaration/append that executed before this call). Lets an
# external caller (the Rust test suite) ask bash directly, instead of
# re-parsing this file's source text with a form-specific parser that can
# desync from what bash actually honors.
print_allowed_skips() {
    printf '%s\n' "${ALLOWED_SKIPS[@]}"
}

# ---------------------------------------------------------------------------
# evaluate_needs <json> — the fail-closed decision function (S-CIGATE-2
# AC-001..AC-004).
#
#   - jq-parses the JSON payload into a job -> result map
#   - fails closed (returns 1) on an empty `needs` object (AC-004) — a gate
#     with nothing to check must not vacuously pass
#   - for each job: `success` -> OK; `skipped` + `is_allowed_skip` -> OK;
#     anything else (including `failure`, `cancelled`, an unlisted
#     `skipped`, or any result string never seen before) -> FAIL via a
#     default arm, not an enumerated list of known-bad values (AC-002/AC-003)
#   - prints one `OK  <job> = <result>` or `FAIL  <job> = <result>` line per
#     job so a gate failure is diagnosable from the ci-gate job's own log
#   - returns 0 only if every job passed
#
# LOAD-BEARING LOG FORMAT (PR #671 review, round 6; byte-count corrected
# round 7 — verify actual bytes before rewriting this comment again): the
# exact strings `OK  <job> = <result>` and `FAIL  <job> = <result>` — TWO
# spaces after `OK`, TWO spaces after `FAIL` (not one — grep the `echo`
# lines below yourself if this claim is ever in doubt), ` = ` around the
# result — are asserted verbatim (as a substring match against this
# function's stdout) by
# `tests/ci_gate_completeness.rs::test_ci_gate_decision_matches_job_level_if_for_every_needs_member`,
# which checks for `OK  <job> = skipped` / `FAIL  <job> = skipped` to
# confirm the gate's per-job decision, not merely its exit code. Changing
# this format's spacing/wording without updating that test would break
# its assertions LOUDLY (the test fails) — but `--self-test` (which only
# checks exit codes via `check_fixture`'s rc comparison, not message
# content — except fixture 9's dedicated substring check) would NOT catch
# it, so the failure would only surface in `cargo test`, not in this
# script's own `--self-test` run.
# ---------------------------------------------------------------------------

# trusted_jq_dirs_for <runner_os> — single source of truth for the trusted
# system jq directory allowlist, keyed by GitHub Actions' own $RUNNER_OS
# value ("Linux" | "macOS" | "Windows"). One directory per line on
# stdout; empty output for an OS this guard does not (yet) model. See
# resolve_trusted_jq's "S-626-1 CI-BREAK-1" comment below for why this is
# a directory ALLOWLIST rather than a single-path pin or a writable-
# location denylist.
#
# Only Linux and macOS are populated — the only two RUNNER_OS values
# under which this script actually executes today (`ubuntu-latest` for
# `spec-guard`/`ci-gate`; `ubuntu-latest` AND `macos-latest` for the
# `#[cfg(unix)]` subprocess tests in `tests/ci_gate_completeness.rs`; no
# `windows-latest` job invokes this script — those tests are
# `#[cfg(unix)]`-gated and do not exist on that leg). A future job that
# runs this script on `windows-latest` must add a `Windows` entry here
# FIRST, deliberately, not discover the gap via another production CI
# break of the kind this whole function exists to prevent recurring.
#
# Linux entries are ONE physical directory under two names, not two
# independent trust grants (S-626-1 research pass, 2026-08-10; corrected
# ADV-P675-MEDIUM-001, 2026-08-10 — the original "first LTS with usrmerge"
# framing was wrong): `/bin` is a symlink to `/usr/bin` on `ubuntu-latest`
# (Ubuntu 24.04) — `/usr/bin/jq` and `/bin/jq` are the same inode. Ubuntu
# has shipped merged-`/usr` for new installs since 18.10 Cosmic, so 20.04
# and 22.04 LTS were ALSO already usrmerged; "24.04 is the first LTS with
# usrmerge" is false as a general Ubuntu-installations claim — the
# Rockcraft source for that phrasing scopes it to Ubuntu as a *base system
# inside rocks/container images*, not installations generally. The
# operative conclusion (one physical directory, no canonicalization
# needed) is unaffected: it holds on every LTS the GitHub-hosted runner
# fleet has offered. CONFIRM against Canonical/Debian docs still applies —
# to the merged-`/usr`-since-18.10 property, not the retracted "first LTS"
# claim. No equivalence gap exists for the `/bin` entry to close, and
# `/bin` is kept only as free, defensive redundancy against a future
# `PATH` reordering. Do NOT add `realpath`/`readlink` canonicalization to
# `is_trusted_jq_dir` to "resolve" this — there is nothing to resolve, and
# doing so would reintroduce an external-binary dependency on the decision
# path, undoing the point of `736fea28`.
#
# macOS entries — read as a COMPATIBILITY assertion, not a security one
# (S-626-1 research pass, 2026-08-10): `/opt/homebrew/bin` is (INFERRED,
# high confidence — Homebrew chowns its prefix to the installing user; no
# primary source states the hosted image's mode bits) owned by `runner`
# and writable without `sudo`, unlike `/usr/bin`/`/bin` above, whose
# root-only status rests on the same class of inference (ADV-P675-MEDIUM-002:
# neither half of this comparison is independently CONFIRMED against a
# primary source — see `.factory/research/ci-gate-shell-trust-assumptions-2026-08-10.md`
# Q1b/Q3b). This entry exists solely so the runner's own real Homebrew `jq`
# (Apple Silicon `macos-latest`) is accepted rather than falsely rejected
# — see the CI-BREAK-1 comment on `resolve_trusted_jq` below, the
# production break this entry was added to fix. It provides no meaningful
# security value on this leg (an earlier step can `cp` a shim there with
# no privilege escalation) but this is NOT the decision path — `ci-gate`
# and `spec-guard` both run on `ubuntu-latest`; `/opt/homebrew/bin` is
# reached only by the `test` job's `macos-latest` leg via the
# `#[cfg(unix)]` subprocess tests. Do NOT remove this entry — that
# reproduces CI-BREAK-1 verbatim.
trusted_jq_dirs_for() {
    case "$1" in
        Linux)
            printf '%s\n' "/usr/bin" "/bin"
            ;;
        macOS)
            printf '%s\n' "/usr/bin" "/bin" "/usr/local/bin" "/opt/homebrew/bin"
            ;;
        *)
            ;;
    esac
}

# is_trusted_jq_dir <runner_os> <dir> — true (rc=0) iff <dir> is exactly
# one of trusted_jq_dirs_for(<runner_os>)'s lines. Pure string
# comparison — no filesystem access — so every supported
# (runner_os, dir) pair is directly unit-testable with synthetic inputs
# in `run_jq_trust_self_test` without needing a real jq binary (or
# anything else) to actually exist at the candidate path.
is_trusted_jq_dir() {
    local os="$1" dir="$2" candidate
    while IFS= read -r candidate; do
        [ -z "${candidate}" ] && continue
        [ "${dir}" = "${candidate}" ] && return 0
    done <<EOF
$(trusted_jq_dirs_for "${os}")
EOF
    return 1
}

# resolve_trusted_jq — S-626-1 pass-59 (ADV-P59-LOW-001): every decision
# value this script produces is jq-derived, resolved via a bare `command -v
# jq` lookup. `$GITHUB_PATH` is a documented GitHub Actions mechanism that
# lets ANY earlier step in the same job prepend a directory to `PATH` for
# every subsequent step — a `jq` shim placed there and printing
# `"success"` for every `.result` query (or, worse, `command -v jq`
# resolving to a step-written shim after a `sudo`-writable-`/usr/bin`
# compromise) drives `evaluate_needs` to exit 0 while ALSO printing a
# manufactured `OK  <job> = success` line per job — a fabricated clean
# record, worse than a bare `|| true` appended to the run line (that
# vector is pinned by `tests/ci_gate_completeness.rs`'s M2-i; this one is
# a PATH/binary-identity vector, not a `run:` line vector, and sits
# entirely outside every existing byte-pin in that file). CLAUDE.md's
# round-13 IMPORTANT-2 note previously described this exposure as
# `$GITHUB_ENV` -> `BASH_ENV` (an environment-variable model); a second,
# independent channel is `PATH` -> WHICH BINARY RUNS, a different
# mechanism no env-surface pin or future YAML-parser rewrite would touch.
# INFERRED, not independently run against a live runner by this pass: the
# two channels are both live and do not supersede one another — CLAUDE.md's
# round-12 `env:`-key-set pins (M2-o, workflow-level) read `ci.yml` and so
# cannot see a `BASH_ENV` value an earlier step writes at runtime via
# `$GITHUB_ENV`, which still reaches this script's process environment
# (see the "WHY RUNNER_OS" ordering discussion below for the mechanism).
# This is not a new exposure — it requires the same earlier-step arbitrary-
# execution capability that already yields the `sudo cp .../usr/bin/jq`
# vector in the HONEST SCOPE paragraph below, so it adds nothing to the
# attacker's reach. It is also narrower than it first looks: a `BASH_ENV`
# shim defining a `jq` shell FUNCTION rather than a file is independently
# rejected here regardless — `command -v jq` then returns the bare string
# `jq`, which the absolute-path check just below refuses.
#
# Resolved once per `evaluate_needs` call and reused for every jq
# invocation inside it (rather than re-resolving per call), so a single
# TOCTOU-style PATH mutation mid-function cannot make different
# invocations within the same decision see different binaries.
#
# Enforced STRICTLY (the resolved path's directory must be a member of
# trusted_jq_dirs_for($RUNNER_OS) — see that function immediately below)
# whenever `RUNNER_OS` is non-empty. Outside that (local `--self-test`
# runs under any package manager's jq, where `RUNNER_OS` is normally
# unset), every path still goes through the absolute/executable check
# immediately below — the ONLY thing NOT enforced outside strict mode is
# directory membership in trusted_jq_dirs_for($RUNNER_OS).
#
# S-626-1 ADV-P61 follow-up research — WHY `RUNNER_OS`, NOT `GITHUB_ACTIONS`
# (this superseded an earlier revision of this fix that gated on
# `GITHUB_ACTIONS=true` OR `RUNNER_OS` non-empty, treating a mismatch as
# an error): a GitHub-supplied source-level fact search of `actions/runner`
# found `FileCommandManager.cs`'s `$GITHUB_ENV` write blocklist is
# verbatim `{ "NODE_OPTIONS" }` — no `GITHUB_*`/`RUNNER_*` prefix filter —
# so an earlier step in the SAME job can very likely overwrite
# `GITHUB_ACTIONS` to anything it wants (this specific claim is a
# high-confidence INFERENCE from source reading, not independently run
# against a live workflow). Do NOT lean on GitHub's own docs sentence,
# "You can't overwrite the value of the default environment variables
# named `GITHUB_*` and `RUNNER_*`" (Variables reference), as the reason
# either variable is trustworthy here — that claim is true IN EFFECT for
# `run:` steps, per the write-ordering mechanism below, but the
# `$GITHUB_ENV` write-time BLOCKLIST that would make it true by
# construction is verbatim `{ "NODE_OPTIONS" }`, with no `GITHUB_*`/
# `RUNNER_*` prefix filter — which is precisely why it does NOT by itself
# protect `GITHUB_ACTIONS`. Docs and source disagree at the blocklist
# layer; do not cite the docs sentence as if it settled the question.
#
# `RUNNER_OS`, by contrast, IS confirmed protected — and by a stronger,
# more general mechanism than "regeneration with no allowlist gap" alone
# (S-626-1 research pass, 2026-08-10, NEWLY-RESEARCHED against primary
# source): `actions/runner :: src/Runner.Worker/Handlers/ScriptHandler.cs`
# assembles a `run:` step's process environment by applying runtime
# contexts — every `RunnerContext` key, `RUNNER_OS` included — LAST, and
# the write ITSELF is unconditional once that per-context type check
# passes (ADV-P675-LOW-001: the line below binding `runtimeContext` was
# previously dropped from this quotation without an elision marker,
# reading as though the write were unconditional on context TYPE too —
# it is not; the filter is on which `ExpressionValues` entry qualifies,
# not on which keys get written once it does):
#   foreach (var context in ExecutionContext.ExpressionValues)
#       if (context.Value is IEnvironmentContextData runtimeContext && runtimeContext != null)
#           foreach (var env in runtimeContext.GetRuntimeEnvironmentVariables())
#               Environment[env.Key] = env.Value;
# — a plain assignment, not `TryAdd`, no null guard, no allowlist or
# denylist on which keys get written. This runs AFTER the inherited
# global environment (where `$GITHUB_ENV` writes accumulate) and the
# step's own `env:` are already in place, so it OVERWRITES — not merely
# "regenerates around" — whatever an earlier step's `$GITHUB_ENV` write,
# or a workflow/job/step `env:` block, set `RUNNER_OS` to. That is WRITE
# ORDERING, a stronger property than "no allowlist gap": it defeats
# `$GITHUB_ENV`, workflow `env:`, job `env:`, AND step `env:`
# simultaneously, all by the same single mechanism, not one channel at a
# time. Keying strict mode on `RUNNER_OS` alone (not `GITHUB_ACTIONS`, and
# not a mismatch between the two) uses the one signal in this pair that is
# actually known-trustworthy, for this reason.
#
# ADV-P61-LOW-003 (fixed, not just documented): this comment previously
# claimed "only an absolute, existing path is required" outside strict
# mode, but no such check existed — a relative path like `./jq` from cwd
# was accepted and executed unconditionally. The absolute+executable check
# a few lines below now makes this comment's claim true instead of
# weakening the comment to match the gap.
#
# HONEST SCOPE — WHAT THIS CANNOT CLOSE (S-626-1 ADV-P61 follow-up
# research, do not remove or soften this paragraph; see "THE TRAP" review
# guidance this story's own commit history already cites): both
# `ubuntu-latest` and `macos-latest` GitHub-hosted runners grant the job
# PASSWORDLESS sudo (CONFIRMED — GitHub Docs "GitHub-hosted runners" §
# "Administrative privileges"; corroborated by `actions/runner-images`
# issue #10484 showing `/etc/sudoers.d/runner` grants
# `runner ALL=(root) NOPASSWD:ALL`). An attacker with this story's modeled
# capability — arbitrary execution in an EARLIER STEP of the same job —
# does not need a PATH shim at all: `sudo cp /tmp/shim /usr/bin/jq`
# replaces the TRUSTED system binary in place. No directory allowlist,
# however refined, can detect that — the shim now IS the trusted path.
# This function's checks (dirname-immune directory allowlisting, absolute
# +executable, RUNNER_OS-keyed strict mode) close the cheaper PATH-shim
# vector and are worth keeping, but they do not achieve, and must never be
# described as achieving, "an attacker cannot forge the gate's decision"
# — that property is unreachable from inside this script. The actual
# control for the sudo-replacement vector is not running untrusted code in
# an earlier step of the `ci-gate` job at all, which is the `uses:`-value
# pinning question CLAUDE.md's CI Gate history already records as a
# knowing, deliberate scope decision (out of this story, which is about
# the pass/fail decision path once inputs are trusted, not supply-chain
# pinning of what runs before it).
#
# S-626-1 CI-BREAK-1 (real CI run 31406705091 on commit a17939e2): the
# ORIGINAL version of this function pinned exactly one path,
# `/usr/bin/jq` — correct for `ubuntu-latest` (where `spec-guard`/
# `ci-gate` actually run this script) but WRONG for `macos-latest`, where
# `tests/ci_gate_completeness.rs`'s `#[cfg(unix)]` subprocess tests
# invoke `evaluate_needs()` and inherit the runner's own real
# `GITHUB_ACTIONS=true` from the job environment — Homebrew installs
# `jq` at `/opt/homebrew/bin/jq` (Apple Silicon `macos-latest`, the
# current default) or `/usr/local/bin/jq` (Intel), never `/usr/bin/jq`.
# The single-path pin rejected the runner's own LEGITIMATE jq on every
# macOS `Test` leg, breaking `CI Gate` downstream (13/14 jobs succeeded
# for real; `Test (macos-latest)` failed on this false rejection;
# `CI Gate` correctly failed as a consequence of that failure). Why this
# was invisible locally before merging: strict mode only engages when
# `RUNNER_OS` is non-empty (ADV-P675-LOW-002, 2026-08-10: corrected from
# `GITHUB_ACTIONS=true`, stale since the S-626-1 re-key documented above),
# which is unset on a developer machine by default, so `--self-test` alone
# never reached this branch — see `run_jq_trust_self_test` below, added
# specifically to close that gap by exercising the strict branch
# deterministically regardless of where `--self-test` runs.
#
# Why a DIRECTORY ALLOWLIST keyed by $RUNNER_OS, not a denylist of
# writable locations ($GITHUB_WORKSPACE/$RUNNER_TEMP/$HOME/...): this
# repo's established CI-gate posture (see CLAUDE.md's CI-Gate history
# above `evaluate_needs`) is default-deny — an allowlist of known-trusted
# values that fails closed on anything new, not a denylist of known-bad
# locations that silently passes anything not yet enumerated (the exact
# allowlist-of-known-bad-values shape this whole guard exists to avoid
# repeating, per the module-level PURPOSE comment). A writable-location
# denylist is itself an open enumeration of the same kind that defeated
# the round-3/round-5 `if:`-legitimacy predicates documented above. A
# directory allowlist keyed by the runner's own reported OS is closed by
# construction: a jq shim in ANY directory not in that OS's list is
# rejected, including a location this comment's author never
# anticipated.
resolve_trusted_jq() {
    local resolved
    if ! resolved=$(command -v jq 2>/dev/null); then
        echo "ERROR: jq is required but was not found on PATH." >&2
        return 2
    fi

    # ADV-P61-LOW-003: applies in EVERY mode, not just strict — a resolved
    # jq must be an absolute, existing, executable path. `command -v`
    # ordinarily returns an absolute path for a PATH-resolved binary, but
    # if PATH contains a relative entry (e.g. "." — plausible if an
    # earlier step cd's somewhere and prepends it, or simply a developer's
    # own shell config) it returns exactly what would be executed,
    # relative-ness included. Reject that outright rather than silently
    # trusting whatever the current working directory happens to be.
    case "${resolved}" in
        /*) ;;
        *)
            echo "ERROR: jq resolved to a non-absolute path '${resolved}'." >&2
            echo "       Refusing to trust a jq found via a relative PATH" >&2
            echo "       entry (e.g. '.')." >&2
            return 2
            ;;
    esac
    if [ ! -x "${resolved}" ]; then
        echo "ERROR: jq resolved to '${resolved}', which is not an" >&2
        echo "       executable file." >&2
        return 2
    fi

    # S-626-1 ADV-P61-MEDIUM-002, RE-KEYED per follow-up research (see the
    # HONEST SCOPE / "WHY RUNNER_OS, NOT GITHUB_ACTIONS" comment block
    # above this function): strict mode is gated on `RUNNER_OS` alone.
    # `RUNNER_OS` is CONFIRMED regenerated by the runner's own context
    # machinery every step, with no override path found — unlike
    # `GITHUB_ACTIONS`, which prior research found very likely
    # attacker-writable via an earlier step's `$GITHUB_ENV` write (no
    # `GITHUB_*`/`RUNNER_*` prefix filter on that blocklist). An earlier
    # revision of this fix additionally gated on `GITHUB_ACTIONS=true` and
    # treated a mismatch against `RUNNER_OS` as an error; that is removed
    # here — `GITHUB_ACTIONS`'s value carries no security-relevant
    # information once `RUNNER_OS` alone is the trigger, so checking it
    # would add complexity without adding assurance.
    local os="${RUNNER_OS:-}"
    if [ -n "${os}" ]; then
        local dir
        # Pure-bash dirname (S-626-1 ADV-P61-HIGH-001): `dirname` is
        # itself resolved via PATH, so calling the external binary here
        # let the SAME $GITHUB_PATH shim that supplies a malicious `jq`
        # also supply a `dirname` that unconditionally prints a trusted
        # directory (e.g. "/usr/bin"), defeating the directory-allowlist
        # check below with a jq shim that was never actually in a
        # trusted location. Reproduced end-to-end pre-fix: a two-file
        # shim directory (`jq` + `dirname`, the latter always printing
        # `/usr/bin`) prepended to PATH made resolve_trusted_jq() accept
        # the shim under GITHUB_ACTIONS=true. Computing the directory
        # with bash parameter expansion instead closes this by
        # construction — there is no external `dirname` left on the
        # decision path to shim. See run_jq_trust_self_test's
        # "reject-dirname-shim..." check for the regression pin.
        if [ "${resolved}" = "${resolved#*/}" ]; then
            dir=""                       # no slash at all -> not absolute
        else
            dir="${resolved%/*}"
            [ -z "${dir}" ] && dir="/"   # "/jq" -> "/"
        fi
        if ! is_trusted_jq_dir "${os}" "${dir}"; then
            echo "ERROR: jq resolved to '${resolved}' (directory" >&2
            echo "       '${dir}') under RUNNER_OS='${os}', which is not" >&2
            echo "       one of the trusted system jq directories for" >&2
            echo "       that runner. Refusing to trust a jq binary found" >&2
            echo "       elsewhere on PATH inside a GitHub Actions job" >&2
            echo "       (possible PATH-prepend shim attack via" >&2
            echo "       \$GITHUB_PATH — see resolve_trusted_jq's comment" >&2
            echo "       in this file). Trusted directories for" >&2
            echo "       RUNNER_OS='${os}':" >&2
            local trusted_line
            while IFS= read -r trusted_line; do
                [ -z "${trusted_line}" ] && continue
                echo "         ${trusted_line}" >&2
            done <<EOF
$(trusted_jq_dirs_for "${os}")
EOF
            return 2
        fi
    fi
    printf '%s\n' "${resolved}"
}

evaluate_needs() {
    local json="$1"

    local jq_bin
    if ! jq_bin=$(resolve_trusted_jq); then
        return 2
    fi

    # Dedicated empty/whitespace-only-input check, ahead of the JSON
    # validity check below. Without this, empty/whitespace stdin would
    # still correctly fail closed (rc=2) via the JSON-validity check (an
    # empty string is not valid JSON), but with a message ("input is not
    # valid JSON") that points a debugger at the wrong problem — the input
    # wasn't malformed JSON, there was no input at all (e.g. `ci-gate`'s
    # `toJSON(needs)` env var somehow came through empty). Give that case
    # its own message instead.
    if [ -z "${json//[[:space:]]/}" ]; then
        echo "ERROR: input is empty (or whitespace-only) — expected a" >&2
        echo "       toJSON(needs)-shaped JSON object on stdin." >&2
        return 2
    fi

    if ! echo "${json}" | "${jq_bin}" empty >/dev/null 2>&1; then
        echo "ERROR: input is not valid JSON." >&2
        return 2
    fi

    # Shape check: `jq empty` above validates JSON *syntax* only — `[1,2,3]`
    # and `"a string"` are both syntactically valid JSON but neither is a
    # `toJSON(needs)`-shaped job->result map. Without this check, a
    # non-object payload reaches the `jq -r --arg j ... '.[$j].result'` call
    # below and crashes with jq's own "Cannot index array/string with
    # string" error under `set -e` (jq exit 5) — outside this script's
    # documented 0/1/2 exit contract and surfacing a raw jq trace instead of
    # a clean ERROR: message. Fail closed through the documented path
    # instead.
    if ! echo "${json}" | "${jq_bin}" -e 'type == "object"' >/dev/null 2>&1; then
        echo "ERROR: input JSON is valid but is not an object (expected a" >&2
        echo "       job -> result map shaped like toJSON(needs))." >&2
        return 2
    fi

    # jq_status branch below: not currently reachable by any known input.
    # By this point the input has already passed the empty/whitespace,
    # JSON-validity, and object-shape checks above, so `jq -r 'keys[]'` on
    # a confirmed-valid JSON object has no known way to fail. Kept as a
    # defensive distinct-error branch (rather than folding a hypothetical
    # jq failure into the empty-needs case below) so a future reader who
    # DOES find a triggering input gets a diagnosis pointing at jq, not at
    # "needs was empty" — not because a specific input is known to reach
    # it today.
    local jobs
    local jq_status=0
    jobs=$(echo "${json}" | "${jq_bin}" -r 'keys[]' 2>/dev/null) || jq_status=$?

    if [ "${jq_status}" -ne 0 ]; then
        echo "ERROR: jq failed while extracting job names from the needs" >&2
        echo "       object (exit ${jq_status}). This should not happen" >&2
        echo "       after the JSON-validity and object-shape checks above" >&2
        echo "       passed — please report this as a bug." >&2
        return 2
    fi

    if [ -z "${jobs}" ]; then
        echo "FAIL: needs JSON is empty — the gate has nothing to verify (AC-004)."
        echo "      A gate with nothing to check must not vacuously pass; this"
        echo "      most likely means ci-gate.needs was lost, not that every job"
        echo "      passed."
        return 1
    fi

    local overall_rc=0
    local job result
    while IFS= read -r job; do
        [ -z "${job}" ] && continue
        # -c keeps the extracted value on a single line even if `.result`
        # is ever something other than a plain string (an object/array),
        # so one job never garbles the OK/FAIL log into multiple lines.
        result=$(echo "${json}" | "${jq_bin}" -rc --arg j "${job}" '.[$j].result')

        # Exact match against ALLOWED_SKIPS only (is_allowed_skip) — no
        # substring/prefix matching, so e.g. a job named `mutants-extra`
        # does not inherit the `mutants` carve-out.
        case "${result}" in
            success)
                echo "OK  ${job} = ${result}"
                ;;
            skipped)
                if is_allowed_skip "${job}"; then
                    echo "OK  ${job} = ${result} (allowlisted in ALLOWED_SKIPS)"
                else
                    echo "FAIL  ${job} = ${result} (not in ALLOWED_SKIPS — see" \
                         "scripts/check-ci-gate.sh's ALLOWED_SKIPS comment)"
                    overall_rc=1
                fi
                ;;
            *)
                # Default arm (AC-003): catches `failure`, `cancelled`, and
                # any result value this script has never seen before —
                # including an allowlisted job's non-`skipped` result (the
                # allowlist tolerates `skipped` ONLY, never any other
                # non-`success` value; AC-002).
                echo "FAIL  ${job} = ${result}"
                overall_rc=1
                ;;
        esac
    done <<<"${jobs}"

    return "${overall_rc}"
}

# ---------------------------------------------------------------------------
# jq-trust self-test (S-626-1 CI-BREAK-1, real CI run 31406705091 on
# a17939e2): exercises resolve_trusted_jq's STRICT branch (RUNNER_OS
# non-empty — ADV-P675-LOW-002, 2026-08-10: corrected from
# `GITHUB_ACTIONS=true`, stale since the S-626-1 re-key; item 3 below
# already documented the re-key correctly, making this opening line the
# odd one out) locally. Without this, the strict directory allowlist
# branch is unreachable from ANY local or CI `--self-test` run —
# `run_self_test` above only ever calls evaluate_needs(), which never
# sets RUNNER_OS itself, so a developer running `--self-test` on a laptop
# never touches the strict branch at all. That is exactly
# the defect class that shipped broken: the ORIGINAL single-path pin
# (`/usr/bin/jq` only) was correct for `ubuntu-latest` but wrong for
# `macos-latest`'s real Homebrew jq location, and nothing running
# locally could have caught it because the branch it lived in only
# engages under a real GitHub Actions job. Each check below forces
# GITHUB_ACTIONS/RUNNER_OS/PATH for the duration of an isolated `$( … )`
# subshell (a real subprocess fork, not the running script's own
# environment), so none of these overrides leak into this script's own
# execution or into any check that runs after it.
#
# Covers, per the story's minimum bar:
#   1. ACCEPT — checks 1-6 below assert is_trusted_jq_dir() directly for
#      every (RUNNER_OS, dir) pair trusted_jq_dirs_for() currently
#      returns (Linux -> /usr/bin, /bin; macOS -> /usr/bin, /bin,
#      /usr/local/bin, /opt/homebrew/bin). This is pure string
#      comparison (see is_trusted_jq_dir's own doc comment) — no
#      filesystem access, so it exercises every accept branch exactly,
#      regardless of what jq happens to be installed on the machine
#      running this suite, and directly reproduces the shape of
#      CI-BREAK-1 itself (a single wrong pinned path). Check 13 adds one
#      live, host-adaptive end-to-end call through resolve_trusted_jq()
#      itself — using THIS machine's own real `command -v jq` result
#      against its own real OS (mapped to RUNNER_OS) — to prove the
#      wiring from resolve_trusted_jq through dirname/is_trusted_jq_dir
#      is correct, not just the predicate in isolation. (Exhaustive
#      path-exact end-to-end coverage of all four trusted directories
#      would require placing a real binary at e.g. /usr/bin, which needs
#      root and/or disabling SIP on macOS — out of reach for an offline,
#      no-sudo self-test; checks 1-6 already cover those paths exactly
#      at the predicate level, which is where CI-BREAK-1's actual defect
#      lived.)
#   2. REJECT — checks 7-10 assert is_trusted_jq_dir() rejects a
#      dir/RUNNER_OS combination that is trusted for a DIFFERENT OS, an
#      arbitrary writable directory, and an unmodeled RUNNER_OS (the
#      empty-output `*` arm of trusted_jq_dirs_for). Check 11 is the
#      filesystem-backed end-to-end case: a real shim executable in a
#      throwaway `mktemp -d` directory, prepended to PATH, is refused by
#      resolve_trusted_jq() under GITHUB_ACTIONS=true — the actual
#      ADV-P59-LOW-001 security property this whole guard exists to
#      enforce (meaningless as a pure string check, since the property
#      under test is specifically "a PATH-prepend shim is rejected").
#   3. FAIL-CLOSED (as originally written) — check 12 asserted
#      GITHUB_ACTIONS=true with RUNNER_OS unset refuses. THIS WAS CHANGED
#      by the ADV-P61 follow-up research below: check 12 now asserts the
#      opposite (ACCEPT, non-strict) for that exact input, because strict
#      mode is no longer gated on GITHUB_ACTIONS at all — see its own
#      comment at the check site for why this is correct, not a
#      regression.
#
# S-626-1 ADV-P61 additions (checks 14-17) — a targeted review found the
# jq-identity pin above did NOT, by itself, achieve "an attacker cannot
# forge the gate's decision": every OTHER external binary this script (or
# resolve_trusted_jq specifically) touched was still PATH-resolved and
# unpinned, so the same $GITHUB_PATH-shim capability that motivated
# pinning jq also defeated the pin itself. See CLAUDE.md's CI Gate
# history and this story's commit for the honest, non-overclaiming scope
# statement — these checks raise attacker cost, they do not claim a fully
# closed decision path (that would require pinning EVERY external binary
# call transitively reachable from ci.yml's `ci-gate` job, which is out
# of this story's scope — and, per the follow-up research documented on
# resolve_trusted_jq itself, GitHub-hosted runners' passwordless sudo
# means no in-script check can fully close this class regardless of
# scope):
#   14. reject-dirname-shim-lying-about-trusted-dir (ADV-P61-HIGH-001) —
#       a shim directory supplying BOTH a fake `jq` and a `dirname` that
#       always prints a trusted path is still refused, because
#       resolve_trusted_jq no longer calls external `dirname` at all.
#   15. reject-relative-path-jq-regardless-of-mode (ADV-P61-LOW-003) — the
#       absolute+executable check applies in every mode, not just strict.
#   12 (repurposed) / 16 (ADV-P61-MEDIUM-002, RE-KEYED after follow-up
#       research replaced the GITHUB_ACTIONS-mismatch design with a
#       RUNNER_OS-only trigger — see resolve_trusted_jq's own comment):
#       12 now proves RUNNER_OS-unset is genuinely non-strict regardless
#       of GITHUB_ACTIONS (ACCEPT, an untrusted-dir shim); 16 proves
#       RUNNER_OS-set alone still triggers strict rejection with
#       GITHUB_ACTIONS entirely unset (REJECT, same untrusted-dir shim).
#       Together they show GITHUB_ACTIONS's value no longer has any
#       bearing on the decision either way.
#   17. reject-cat-shim-for-main-stdin-read (ADV-P61-INFO-005) — the ONLY
#       check in this suite that spawns the WHOLE script as a real
#       subprocess (via `bash "${BASH_SOURCE[0]}"`, not `--self-test`),
#       because the `cat` vector lives in main()'s stdin read, not in
#       resolve_trusted_jq(); proves the real piped-in JSON drove the
#       decision despite a hostile `cat` earlier on PATH.
# ---------------------------------------------------------------------------
run_jq_trust_self_test() {
    echo
    echo "=== check-ci-gate.sh JQ-TRUST SELF-TEST (S-626-1 CI-BREAK-1) ==="
    echo

    # Same fixed-denominator pin rationale as EXPECTED_FIXTURES — without
    # it, silently deleting a check (e.g. the FAIL-CLOSED one) would still
    # print "N/N checks matched" as success. EXPECTED_JQ_TRUST_CHECKS is
    # declared at file scope (ADV-P61-INFO-006) — see that declaration's
    # comment near the top of this file.
    local jq_trust_total=0
    local jq_trust_mismatches=0

    # check_trusted_dir <desc> <os> <dir> <expected: "trusted"|"untrusted">
    #
    # Direct, filesystem-free assertion against is_trusted_jq_dir() —
    # covers the ACCEPT/REJECT decision surface exactly, independent of
    # what jq is actually installed on the machine running this suite.
    check_trusted_dir() {
        local desc="$1" os="$2" dir="$3" expected="$4"
        jq_trust_total=$((jq_trust_total + 1))
        local actual="untrusted"
        is_trusted_jq_dir "${os}" "${dir}" && actual="trusted"
        if [ "${actual}" = "${expected}" ]; then
            echo "[PASS] ${desc} (expected=${expected}, actual=${actual})"
        else
            echo "[FAIL] ${desc} (expected=${expected}, actual=${actual})"
            jq_trust_mismatches=$((jq_trust_mismatches + 1))
        fi
    }

    # record_resolve_check <desc> <rc> <expected: "pass"|"fail:<rc>"> <output>
    #                       [expected_substring]
    #
    # Same shape as run_self_test's check_fixture, applied to
    # resolve_trusted_jq()'s output/exit-code instead of
    # evaluate_needs()'s — the caller runs resolve_trusted_jq in its own
    # isolated `$( … )` subshell (so env overrides never leak) and passes
    # the captured rc/output in.
    record_resolve_check() {
        local desc="$1" rc="$2" expected="$3" output="$4" expected_substring="${5:-}"
        jq_trust_total=$((jq_trust_total + 1))

        local actual
        if [ "${rc}" -eq 0 ]; then
            actual="pass"
        else
            actual="fail:${rc}"
        fi

        local rc_ok=true
        [ "${actual}" = "${expected}" ] || rc_ok=false

        local substring_ok=true
        if [ -n "${expected_substring}" ] && ! grep -qF -- "${expected_substring}" <<<"${output}"; then
            substring_ok=false
        fi

        if [ "${rc_ok}" = true ] && [ "${substring_ok}" = true ]; then
            echo "[PASS] ${desc} (expected=${expected}, actual=${actual})"
        else
            echo "[FAIL] ${desc} (expected=${expected}, actual=${actual})"
            if [ "${substring_ok}" = false ]; then
                echo "       expected output to contain: \"${expected_substring}\""
            fi
            echo "       --- resolve_trusted_jq output ---"
            while IFS= read -r line; do
                echo "       ${line}"
            done <<<"${output}"
            jq_trust_mismatches=$((jq_trust_mismatches + 1))
        fi
    }

    # --- ACCEPT: every (RUNNER_OS, dir) pair trusted_jq_dirs_for() lists (1-6) ---
    check_trusted_dir "linux-usr-bin-trusted" "Linux" "/usr/bin" "trusted"
    check_trusted_dir "linux-bin-trusted" "Linux" "/bin" "trusted"
    check_trusted_dir "macos-usr-bin-trusted" "macOS" "/usr/bin" "trusted"
    check_trusted_dir "macos-bin-trusted" "macOS" "/bin" "trusted"
    check_trusted_dir "macos-usr-local-bin-trusted" "macOS" "/usr/local/bin" "trusted"
    # macos-opt-homebrew-bin-trusted: a COMPATIBILITY assertion (CI-BREAK-1
    # — accepts the runner's real, unprivileged-writable Homebrew jq), not
    # a security one — see trusted_jq_dirs_for's macOS-branch comment above.
    check_trusted_dir "macos-opt-homebrew-bin-trusted" "macOS" "/opt/homebrew/bin" "trusted"

    # --- REJECT: cross-OS and unmodeled-OS predicate checks (7-10) ---
    check_trusted_dir \
        "linux-rejects-macos-only-homebrew-dir" "Linux" "/opt/homebrew/bin" "untrusted"
    check_trusted_dir \
        "linux-rejects-arbitrary-writable-dir" "Linux" "/tmp" "untrusted"
    check_trusted_dir \
        "macos-rejects-arbitrary-writable-dir" "macOS" "/tmp" "untrusted"
    check_trusted_dir \
        "unmodeled-os-rejects-every-dir" "Windows" "/usr/bin" "untrusted"

    # --- REJECT: filesystem-backed end-to-end shim rejection (11) ---
    local scratch untrusted_dir out rc
    scratch=$(mktemp -d)
    untrusted_dir="${scratch}/untrusted"
    mkdir -p "${untrusted_dir}"
    cat >"${untrusted_dir}/jq" <<'JQSHIM'
#!/usr/bin/env bash
echo '{}'
JQSHIM
    chmod +x "${untrusted_dir}/jq"

    rc=0
    out=$(
        {
            export PATH="${untrusted_dir}:${PATH}"
            export GITHUB_ACTIONS=true
            export RUNNER_OS=Linux
            resolve_trusted_jq
        } 2>&1
    ) || rc=$?
    record_resolve_check \
        "reject-path-prepend-shim-in-untrusted-dir" "${rc}" "fail:2" "${out}" \
        "one of the trusted system jq directories"

    rm -rf "${scratch}"

    # --- REJECT: dirname-shim variant of the same attack (14, ADV-P61-HIGH-001) ---
    #
    # Same shape as check 11, but the shim directory ALSO supplies a
    # `dirname` that unconditionally prints a trusted directory
    # ("/usr/bin") regardless of its argument — the exact construction
    # that defeated the pre-fix version of resolve_trusted_jq(), which
    # called the external `dirname` binary (itself PATH-resolved) to
    # compute the jq shim's own directory. If this ever regresses back to
    # calling external `dirname`, this check fails because the shimmed
    # `dirname` would misreport the untrusted directory as trusted.
    local dn_scratch dn_untrusted_dir dn_out dn_rc
    dn_scratch=$(mktemp -d)
    dn_untrusted_dir="${dn_scratch}/untrusted"
    mkdir -p "${dn_untrusted_dir}"
    cat >"${dn_untrusted_dir}/jq" <<'JQSHIM'
#!/usr/bin/env bash
echo '{}'
JQSHIM
    chmod +x "${dn_untrusted_dir}/jq"
    cat >"${dn_untrusted_dir}/dirname" <<'DIRNAMESHIM'
#!/usr/bin/env bash
echo "/usr/bin"
DIRNAMESHIM
    chmod +x "${dn_untrusted_dir}/dirname"

    dn_rc=0
    dn_out=$(
        {
            export PATH="${dn_untrusted_dir}:${PATH}"
            export GITHUB_ACTIONS=true
            export RUNNER_OS=Linux
            resolve_trusted_jq
        } 2>&1
    ) || dn_rc=$?
    record_resolve_check \
        "reject-dirname-shim-lying-about-trusted-dir" "${dn_rc}" "fail:2" "${dn_out}" \
        "one of the trusted system jq directories"

    rm -rf "${dn_scratch}"

    # --- REJECT: resolved jq must be absolute + executable, in EVERY mode (15, ADV-P61-LOW-003) ---
    local rel_scratch rel_out rel_rc
    rel_scratch=$(mktemp -d)
    cat >"${rel_scratch}/jq" <<'RELSHIM'
#!/usr/bin/env bash
echo '{}'
RELSHIM
    chmod +x "${rel_scratch}/jq"

    rel_rc=0
    rel_out=$(
        {
            cd "${rel_scratch}" && PATH=".:${PATH}" resolve_trusted_jq
        } 2>&1
    ) || rel_rc=$?
    record_resolve_check \
        "reject-relative-path-jq-regardless-of-mode" "${rel_rc}" "fail:2" "${rel_out}" \
        "non-absolute path"

    rm -rf "${rel_scratch}"

    # --- ACCEPT (intentional, non-strict): RUNNER_OS unset skips directory
    # enforcement regardless of GITHUB_ACTIONS (12, re-keyed per ADV-P61
    # follow-up research — see resolve_trusted_jq's "WHY RUNNER_OS, NOT
    # GITHUB_ACTIONS" comment) ---
    #
    # A jq shim planted in an arbitrary UNTRUSTED directory is still
    # ACCEPTED here, on purpose: with GITHUB_ACTIONS=true set but RUNNER_OS
    # genuinely unset, this script has no directory allowlist to check
    # against (there is no RUNNER_OS to key trusted_jq_dirs_for() with) —
    # this is the documented local/non-CI posture, not a gap, because
    # RUNNER_OS's absence in a real GitHub Actions job is not attacker-
    # reachable (CONFIRMED regenerated every step, no override path
    # found). Proves the earlier (superseded) mismatch-based design's
    # "GITHUB_ACTIONS=true but RUNNER_OS unset -> hard error" branch is
    # gone: this state now just means non-strict mode, same as a plain
    # developer laptop.
    local unset_scratch unset_dir unset_out unset_rc
    unset_scratch=$(mktemp -d)
    unset_dir="${unset_scratch}/wherever"
    mkdir -p "${unset_dir}"
    cat >"${unset_dir}/jq" <<'JQSHIM'
#!/usr/bin/env bash
echo '{}'
JQSHIM
    chmod +x "${unset_dir}/jq"
    unset_rc=0
    unset_out=$(
        {
            export PATH="${unset_dir}:${PATH}"
            export GITHUB_ACTIONS=true
            unset RUNNER_OS
            resolve_trusted_jq
        } 2>&1
    ) || unset_rc=$?
    record_resolve_check \
        "non-strict-accepts-untrusted-dir-jq-when-runner-os-unset" "${unset_rc}" "pass" "${unset_out}"
    rm -rf "${unset_scratch}"

    # --- REJECT: RUNNER_OS alone triggers strict mode, independent of
    # GITHUB_ACTIONS (16, ADV-P61-MEDIUM-002 re-keyed) ---
    #
    # A jq shim in an UNTRUSTED directory is still REFUSED here even with
    # GITHUB_ACTIONS entirely unset — proving RUNNER_OS is genuinely the
    # sole trigger, not merely an additional condition alongside
    # GITHUB_ACTIONS. This is the concrete, evidence-backed improvement
    # over the superseded GITHUB_ACTIONS-mismatch design: an attacker who
    # rewrites GITHUB_ACTIONS via $GITHUB_ENV (plausible per source
    # reading) gains nothing, because this check no longer looks at it.
    local ro_scratch ro_dir ro_out ro_rc
    ro_scratch=$(mktemp -d)
    ro_dir="${ro_scratch}/untrusted"
    mkdir -p "${ro_dir}"
    cat >"${ro_dir}/jq" <<'JQSHIM'
#!/usr/bin/env bash
echo '{}'
JQSHIM
    chmod +x "${ro_dir}/jq"
    ro_rc=0
    ro_out=$(
        {
            export PATH="${ro_dir}:${PATH}"
            unset GITHUB_ACTIONS
            export RUNNER_OS=Linux
            resolve_trusted_jq
        } 2>&1
    ) || ro_rc=$?
    record_resolve_check \
        "strict-mode-triggers-on-runner-os-alone" "${ro_rc}" "fail:2" "${ro_out}" \
        "one of the trusted system jq directories"
    rm -rf "${ro_scratch}"

    # --- ACCEPT: live, host-adaptive end-to-end wiring proof (13) ---
    #
    # Maps this machine's own `uname -s` to the RUNNER_OS value a real
    # GitHub Actions runner of that OS would set, then calls
    # resolve_trusted_jq() with GITHUB_ACTIONS=true against whatever jq
    # is genuinely first on PATH. This script's own TOOLING CHOICE
    # comment already treats a present, working `jq` as a precondition
    # for running ANY self-test (every fixture above depends on it via
    # evaluate_needs()) — this check additionally requires that jq sit in
    # a directory this script currently trusts for the host's own OS,
    # which is true for both the `apt`-installed `/usr/bin/jq` on Linux
    # and the Homebrew-installed `/opt/homebrew/bin/jq` /
    # `/usr/local/bin/jq` on macOS — the two package managers this repo's
    # own CI and documented developer workflow assume. Scoped to
    # Linux/macOS deliberately (see trusted_jq_dirs_for's own doc
    # comment on why Windows is unmodeled: no windows-latest job ever
    # invokes this script).
    local host_os runner_os
    host_os=$(uname -s)
    case "${host_os}" in
        Darwin) runner_os="macOS" ;;
        Linux) runner_os="Linux" ;;
        *) runner_os="" ;;
    esac

    rc=0
    out=$(
        {
            export GITHUB_ACTIONS=true
            export RUNNER_OS="${runner_os}"
            resolve_trusted_jq
        } 2>&1
    ) || rc=$?
    record_resolve_check \
        "accept-real-host-jq-in-trusted-dir (uname=${host_os}, RUNNER_OS=${runner_os:-<unmapped>})" \
        "${rc}" "pass" "${out}"

    # --- REJECT: main()'s stdin read must not go through an external
    # `cat` on PATH (17, ADV-P61-INFO-005) ---
    #
    # Distinct from every check above: this exercises the WHOLE script as
    # a real subprocess (not just resolve_trusted_jq()), because the `cat`
    # vector lives in main()'s stdin read, not in jq trust. A `cat` shim
    # fabricating a payload proves nothing by itself — the meaningful
    # assertion is that the REAL stdin JSON piped in is what actually
    # drove the decision, despite a hostile `cat` sitting first on PATH.
    #
    # GITHUB_ACTIONS/RUNNER_OS are explicitly UNSET for this subprocess
    # (deliberately decoupled from strict jq-directory trust, which checks
    # 1-16 already cover) so this check's outcome depends only on the
    # cat-shim property under test, never on this machine's own ambient
    # environment or where its real jq happens to live.
    local cat_scratch cat_shim_dir cat_out cat_rc
    cat_scratch=$(mktemp -d)
    cat_shim_dir="${cat_scratch}/shim"
    mkdir -p "${cat_shim_dir}"
    cat >"${cat_shim_dir}/cat" <<'CATSHIM'
#!/usr/bin/env bash
echo '{"bogus":{"result":"success"}}'
CATSHIM
    chmod +x "${cat_shim_dir}/cat"

    cat_rc=0
    cat_out=$(
        {
            unset GITHUB_ACTIONS
            unset RUNNER_OS
            PATH="${cat_shim_dir}:${PATH}" \
                bash "${BASH_SOURCE[0]}" <<<'{"fmt":{"result":"success"}}'
        } 2>&1
    ) || cat_rc=$?
    jq_trust_total=$((jq_trust_total + 1))
    if [ "${cat_rc}" -eq 0 ] && grep -qF 'OK  fmt = success' <<<"${cat_out}" \
        && ! grep -qF 'bogus' <<<"${cat_out}"; then
        echo "[PASS] reject-cat-shim-for-main-stdin-read" \
             "(real stdin honored despite \$PATH cat shim)"
    else
        echo "[FAIL] reject-cat-shim-for-main-stdin-read" \
             "(expected the real piped stdin JSON to be read via a bash" \
             "builtin, unaffected by a \$PATH cat shim; got rc=${cat_rc}," \
             "output: ${cat_out})"
        jq_trust_mismatches=$((jq_trust_mismatches + 1))
    fi
    rm -rf "${cat_scratch}"

    echo
    echo "${jq_trust_total}/${EXPECTED_JQ_TRUST_CHECKS} jq-trust checks run," \
         "${jq_trust_mismatches} mismatch(es)."

    if [ "${jq_trust_mismatches}" -ne 0 ]; then
        echo "FAIL: ${jq_trust_mismatches} jq-trust check(s) disagreed with" \
             "resolve_trusted_jq()."
        return 1
    fi

    if [ "${jq_trust_total}" != "${EXPECTED_JQ_TRUST_CHECKS}" ]; then
        echo "SELF-TEST-FIXTURE-COUNT: expected ${EXPECTED_JQ_TRUST_CHECKS}" \
             "jq-trust checks, got ${jq_trust_total}. A check was added or" \
             "removed without updating EXPECTED_JQ_TRUST_CHECKS."
        return 1
    fi

    echo "PASS: all jq-trust checks matched their expected outcome."
    return 0
}

# ---------------------------------------------------------------------------
# Self-test fixture suite (S-CIGATE-2 AC-002/AC-003/AC-004/AC-005).
#
# Each fixture asserts an EXPECTED outcome against evaluate_needs():
# "pass" (exit 0), or "fail:<rc>" pinning the EXACT exit code (1 = a
# decision failure — see the per-job OK/FAIL lines; 2 = the input itself
# was rejected before any per-job decision was made: missing jq, malformed
# JSON, or valid-but-non-object JSON). Distinguishing rc=1 from rc=2 is
# what a maintainer debugging a red gate actually needs — folding both into
# a single "fail" would hide whether the gate rejected the payload's SHAPE
# or made a real per-job FAIL decision.
#
# This proves the decision logic is not a no-op: every fixture below was
# independently proven RED against the Red Gate stub (which always
# returned 0 regardless of input) before the real fail-closed logic in
# evaluate_needs() was written.
# ---------------------------------------------------------------------------
run_self_test() {
    echo "=== check-ci-gate.sh SELF-TEST (S-CIGATE-2) ==="
    echo

    # FIXTURE-COUNT PIN (PR #671 review round 10, IMPORTANT 1): without
    # this, the suite's own summary line ("N/N fixtures matched") reports
    # its OWN shrunken denominator as success — deleting a fixture (e.g.
    # fixture 3 or fixture 13, the only two that reject an unlisted skip)
    # silently degrades coverage while still printing "PASS: all fixtures
    # matched". Reproduced: deleting fixture 13 alone -> "12/12 PASS";
    # deleting both 3 and 13 -> "11/11 PASS", with the Rust suite (which
    # does not derive its expectations from this count) staying 14/14
    # throughout. Same fixed-denominator pattern already used by
    # scripts/check-bc-citation-symbols.sh and
    # scripts/check-cargo-mutants-policy-citations.sh (both pin
    # EXPECTED_FIXTURES against a `fixtures_run` counter) — mirrored here
    # rather than invented fresh. Declared at file scope (ADV-P61-INFO-006)
    # — see that declaration's comment near the top of this file.
    local total=0
    local mismatches=0

    # check_fixture <description> <json> <expected: "pass" | "fail:<rc>">
    #              [expected_substring]
    #
    # The optional 4th argument discriminates BETWEEN fixtures that produce
    # the same exit code via different code paths — without it, two
    # fixtures with the same expected rc are indistinguishable from each
    # other's perspective, so deleting the more specific check (e.g. the
    # empty/whitespace-input pre-check, which shares rc=2 with the
    # malformed-JSON and non-object-JSON checks) would not be caught by
    # exit code alone.
    check_fixture() {
        local desc="$1"
        local json="$2"
        local expected="$3"
        local expected_substring="${4:-}"

        total=$((total + 1))

        local output
        local rc=0
        output=$(evaluate_needs "${json}" 2>&1) || rc=$?

        local actual
        if [ "${rc}" -eq 0 ]; then
            actual="pass"
        else
            actual="fail:${rc}"
        fi

        local rc_ok=true
        [ "${actual}" = "${expected}" ] || rc_ok=false

        local substring_ok=true
        if [ -n "${expected_substring}" ] && ! grep -qF -- "${expected_substring}" <<<"${output}"; then
            substring_ok=false
        fi

        if [ "${rc_ok}" = true ] && [ "${substring_ok}" = true ]; then
            echo "[PASS] ${desc} (expected=${expected}, actual=${actual})"
        else
            echo "[FAIL] ${desc} (expected=${expected}, actual=${actual})"
            if [ "${substring_ok}" = false ]; then
                echo "       expected output to contain: \"${expected_substring}\""
            fi
            echo "       --- evaluate_needs output ---"
            while IFS= read -r line; do
                echo "       ${line}"
            done <<<"${output}"
            mismatches=$((mismatches + 1))
        fi
    }

    # Fixture 1 — all jobs success -> PASS.
    check_fixture \
        "all-success" \
        '{"fmt":{"result":"success"},"clippy":{"result":"success"},"test":{"result":"success"}}' \
        "pass"

    # Fixture 2 — one job failure -> FAIL (rc=1: a real per-job decision).
    check_fixture \
        "one-job-failure" \
        '{"fmt":{"result":"failure"},"clippy":{"result":"success"}}' \
        "fail:1"

    # Fixture 3 — an UNLISTED job reports skipped -> FAIL (only ALLOWED_SKIPS
    # members may tolerate skipped).
    check_fixture \
        "unlisted-job-skipped" \
        '{"fmt":{"result":"skipped"},"clippy":{"result":"success"}}' \
        "fail:1"

    # Fixture 4 — mutants (allowlisted) reports skipped -> PASS.
    check_fixture \
        "mutants-skipped-allowlisted" \
        '{"mutants":{"result":"skipped"},"fmt":{"result":"success"}}' \
        "pass"

    # Fixture 5 — mutants reports failure -> FAIL (allowlist tolerates
    # `skipped` ONLY, never any other non-success value — proves the
    # carve-out is restrictive, not a blanket exemption).
    check_fixture \
        "mutants-failure-allowlist-is-restrictive" \
        '{"mutants":{"result":"failure"},"fmt":{"result":"success"}}' \
        "fail:1"

    # Fixture 6 — a job reports cancelled -> FAIL.
    check_fixture \
        "job-cancelled" \
        '{"fmt":{"result":"cancelled"},"clippy":{"result":"success"}}' \
        "fail:1"

    # Fixture 7 — a job reports an invented/unknown result string -> FAIL via
    # the default arm (the structural fix: today's condition allowlists
    # known-bad values, so an unrecognized future value must not pass
    # through unnoticed).
    check_fixture \
        "unrecognized-result-value" \
        '{"fmt":{"result":"action_required"},"clippy":{"result":"success"}}' \
        "fail:1"

    # Fixture 8 — empty needs context -> FAIL closed (a gate with nothing to
    # check must not vacuously pass). rc=1: this is a real "nothing to
    # verify" decision, distinct from the input-rejection rc=2 fixtures
    # below.
    check_fixture \
        "empty-needs" \
        '{}' \
        "fail:1"

    # Fixture 9 — empty/whitespace-only input -> FAIL closed with rc=2, via
    # its own dedicated check, not the object-shape check it would
    # otherwise fall through to. Verified empirically: `jq empty` on empty
    # or whitespace-only input exits 0 (it parses zero JSON values, which
    # jq does not treat as a syntax error), so without the dedicated
    # pre-check this input would silently pass the JSON-validity check and
    # instead be caught by the LATER object-shape check (`jq -e 'type ==
    # "object"'`, which does fail on empty input) — same rc=2, but with the
    # "input JSON is valid but is not an object" message, which is a
    # confusing diagnosis for input that was never JSON at all. The 4th
    # `check_fixture` argument below asserts the dedicated pre-check's OWN
    # message actually fires (discriminates this fixture from the
    # malformed-JSON and non-object-JSON fixtures below, which all share
    # rc=2 but must fire through different code paths).
    check_fixture \
        "empty-or-whitespace-input" \
        '   ' \
        "fail:2" \
        "input is empty (or whitespace-only)"

    # Fixture 10 — syntactically invalid JSON -> FAIL closed with rc=2
    # (input rejected before any per-job decision is made).
    #
    # ADV-P56-LOW-001: without a discriminating 4th argument, this fixture
    # was indistinguishable-by-assertion from "empty-or-whitespace-input"
    # (fixture 9) and "non-object-json-array" (fixture 11) — all three
    # share rc=2. Deleting the dedicated `jq empty` validity check (the
    # code path this fixture exists to pin) would leave `not json` falling
    # through to the LATER object-shape check instead, which also exits 2
    # (jq's `type == "object"` predicate fails on unparseable input too) —
    # so rc-only comparison stayed 13/13 green with that check deleted.
    # Pinning the dedicated check's own message closes that gap, mirroring
    # the 4th-argument discrimination already used on fixture 9.
    check_fixture \
        "malformed-json" \
        'not json' \
        "fail:2" \
        "input is not valid JSON"

    # Fixture 11 — syntactically VALID JSON that is not an object (a bare
    # array) -> FAIL closed with rc=2. `jq empty` alone is not sufficient
    # here: it validates JSON *syntax*, not *shape*, and `[1,2,3]` passes
    # it. Without the dedicated object-shape check this fixture pins, this
    # payload would instead crash the per-job `jq -r --arg j ... .result`
    # call with jq's own "Cannot index array with string" error (jq exit
    # 5) — outside this script's documented 0/1/2 exit contract.
    check_fixture \
        "non-object-json-array" \
        '[1,2,3]' \
        "fail:2"

    # Fixture 12 — a realistic multi-line toJSON(needs) payload, modeled on
    # the actual shape of live CI run 30465686049 (the run that first
    # exposed this story's defect): pretty-printed, multi-line, and each
    # job carries an `outputs` sibling beyond `result` — not the
    # single-line minimal `{"job":{"result":"..."}}` shape every other
    # fixture above uses. Per GitHub's contexts reference
    # (https://docs.github.com/en/actions/learn-github-actions/contexts
    # -> "needs context"), `needs.<job_id>` has exactly two properties,
    # `result` and `outputs` — there is no `outcome`. `outcome` belongs to
    # the STEPS context (`steps.<id>.outcome`, "result before
    # continue-on-error is applied") and is never present on a `needs`
    # entry; an earlier round of this fixture modeled `outcome` here by
    # mistake (unverified assumption, corrected PR #671 review round 8 —
    # see tests/ci_gate_completeness.rs for the reproduction of the
    # phantom-field blind spot this caused). jq itself is
    # whitespace-insensitive, so this fixture is not needed to catch a
    # single-line-only jq bug (that bug class does not exist here) — its
    # value is being the only fixture exercising the FULL real 8-job
    # ci-gate.needs set with the real sibling field alongside `result`, as
    # an end-to-end shape check on top of the deliberately minimal
    # fixtures above. NOTE (PR #671 review round 10, drift class
    # documented, not fixed; round 11 corrected an inaccurate cross-
    # reference here — fixture 13 below hardcodes only THREE jobs (`fmt`,
    # `clippy`, `mutants`), not this fixture's full eight): this fixture
    # hardcodes the 8-job list as a JSON literal; nothing pins that the
    # literal still matches the REAL `ci-gate.needs` set in ci.yml if it
    # changes. No false-green results from this today (the Rust suite
    # derives its own job list from ci.yml at runtime via
    # `parse_needs_set`, so a drift here would only stale this bash
    # fixture's realism, not silently widen what the gate tolerates) —
    # same drift CLASS `NEEDS_CONTEXT_JOB_KEYS` closed for payload keys,
    # left open here for job identities. Expected: PASS (mutants skipped
    # + allowlisted, every other required job
    # succeeded — the actual shape of a legitimate
    # push-event run).
    check_fixture \
        "realistic-multiline-toJSON-needs-payload" \
        '{
  "fmt": {
    "result": "success",
    "outputs": {}
  },
  "clippy": {
    "result": "success",
    "outputs": {}
  },
  "test": {
    "result": "success",
    "outputs": {}
  },
  "msrv": {
    "result": "success",
    "outputs": {}
  },
  "deny": {
    "result": "success",
    "outputs": {}
  },
  "spec-guard": {
    "result": "success",
    "outputs": {}
  },
  "check-signing-workflow-injection": {
    "result": "success",
    "outputs": {}
  },
  "mutants": {
    "result": "skipped",
    "outputs": {}
  }
}' \
        "pass"

    # Fixture 13 — an UNLISTED job (fmt) skipped, in the SAME
    # production-shaped payload as fixture 12 (every job carries an
    # `outputs` sibling, not just `result` — see fixture 12's comment for
    # why `outcome` is NOT modeled) -> FAIL closed (rc=1). CRITICAL-3, PR
    # #671 review round 7: fixture 12 exercises the production shape only
    # for a PASS case (mutants, legitimately allowlisted) — a mutation
    # keying tolerance on the mere PRESENCE of an `outputs` sibling field
    # (e.g. `is_allowed_skip "${job}" || ... has("outputs")`) rather than
    # on `is_allowed_skip` alone would pass every other fixture here,
    # since none of them combine an UNLISTED skipped job with
    # production-shaped sibling fields. Reproduced: without this fixture,
    # that exact mutation left --self-test at 12/12 while the real gate
    # accepted any skipped job carrying an `outputs` key (i.e. every real
    # job, since `outputs` is always present).
    check_fixture \
        "unlisted-job-skipped-full-production-shape" \
        '{
  "fmt": {
    "result": "skipped",
    "outputs": {}
  },
  "clippy": {
    "result": "success",
    "outputs": {}
  },
  "mutants": {
    "result": "success",
    "outputs": {}
  }
}' \
        "fail:1"

    echo
    echo "Self-test summary: $((total - mismatches))/${total} fixtures matched their expected outcome."

    if [ "${mismatches}" -gt 0 ]; then
        echo "FAIL: ${mismatches} fixture(s) disagreed with evaluate_needs()."
        echo "      This means the fail-closed decision logic in evaluate_needs()"
        echo "      does not match the expected outcome for one or more fixtures"
        echo "      above — see the [FAIL] line(s) for which fixture(s) and why."
        return 1
    fi

    # Post-fixture self-assertion (NOT a fixture; does not affect `total`).
    # See the EXPECTED_FIXTURES comment above `total=0` for why this exists.
    if [ "${total}" != "${EXPECTED_FIXTURES}" ]; then
        echo "SELF-TEST-FIXTURE-COUNT: expected ${EXPECTED_FIXTURES} fixtures," \
             "got ${total}. A fixture was added or removed without updating" \
             "EXPECTED_FIXTURES — every fixture here is load-bearing (each" \
             "was independently proven RED before the logic it pins was" \
             "written); update EXPECTED_FIXTURES ONLY after confirming no" \
             "fixture was silently dropped."
        return 1
    fi

    echo "PASS: all fixtures matched their expected outcome."
    return 0
}

main() {
    if [ "${1:-}" = "--self-test" ]; then
        # Both suites always run (never short-circuited) so a developer
        # sees every failure in one pass rather than fixing one suite at
        # a time; the combined exit reflects EITHER suite failing (S-626-1
        # CI-BREAK-1 — see run_jq_trust_self_test's module comment for why
        # this second suite exists alongside the original decision-fixture
        # one above it).
        local decision_rc=0 jq_trust_rc=0
        run_self_test || decision_rc=$?
        run_jq_trust_self_test || jq_trust_rc=$?
        if [ "${decision_rc}" -ne 0 ] || [ "${jq_trust_rc}" -ne 0 ]; then
            exit 1
        fi
        exit 0
    fi

    if [ "${1:-}" = "--print-allowed-skips" ]; then
        print_allowed_skips
        exit $?
    fi

    local json
    # S-626-1 ADV-P61-INFO-005: `json="$(cat)"` shelled out to an
    # external `cat` on PATH to read this script's own decision input —
    # a `cat` shim planted via $GITHUB_PATH could return an arbitrary,
    # fabricated payload (e.g. an all-`"success"` map) regardless of the
    # real `toJSON(needs)` piped in, driving evaluate_needs() to a false
    # PASS with no dependency on jq at all. Verified: a `cat` shim alone
    # (real, correctly-trusted jq otherwise untouched) reproduced this.
    # `$(<...)` is a bash builtin fast-path for reading a file/fd into a
    # variable — no external process is spawned, so there is nothing on
    # PATH left to shim for this read. See run_jq_trust_self_test's
    # "reject-cat-shim..." check for the regression pin.
    json="$(</dev/stdin)"
    evaluate_needs "${json}"
    exit $?
}

main "$@"
