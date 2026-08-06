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
# mirror image of the false-green defect this script exists to fix. This
# holds regardless of HOW the array is widened: a second `ALLOWED_SKIPS=(...)`
# declaration, an `ALLOWED_SKIPS+=(...)` append, or any other valid bash
# array-construction form bash accepts — all are equally a widening.
#
# `tests/ci_gate_completeness.rs::test_allowed_skips_members_require_job_level_conditional_in_ci_yml`
# enforces the semantic reason a job may be here: it shells out to THIS
# SCRIPT's `--print-allowed-skips` mode (not a source-text parser) to ask
# bash itself what it considers ALLOWED_SKIPS to be, then asserts every
# named job's `ci.yml` block carries a job-level `if:` condition (the same
# structural fact that makes `mutants`'s `skipped` result on push
# legitimate). A second, syntax-independent guard
# (`test_allowed_skips_has_exactly_three_code_level_references`) counts the
# non-comment-line occurrences of the `ALLOWED_SKIPS` identifier in this
# file and asserts exactly 3 (the declaration below, the read in
# `is_allowed_skip`, and the read in `print_allowed_skips`) — belt-and-
# braces protection against a new code-level reference (e.g. an `+=`
# append line) appearing anywhere in this file, independent of whether
# `--print-allowed-skips` itself would reflect it.
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
# ---------------------------------------------------------------------------
evaluate_needs() {
    local json="$1"

    if ! command -v jq >/dev/null 2>&1; then
        echo "ERROR: jq is required but was not found on PATH." >&2
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

    if ! echo "${json}" | jq empty >/dev/null 2>&1; then
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
    if ! echo "${json}" | jq -e 'type == "object"' >/dev/null 2>&1; then
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
    jobs=$(echo "${json}" | jq -r 'keys[]' 2>/dev/null) || jq_status=$?

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
        result=$(echo "${json}" | jq -rc --arg j "${job}" '.[$j].result')

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

    local total=0
    local mismatches=0

    # check_fixture <description> <json> <expected: "pass" | "fail:<rc>">
    check_fixture() {
        local desc="$1"
        local json="$2"
        local expected="$3"

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

        if [ "${actual}" = "${expected}" ]; then
            echo "[PASS] ${desc} (expected=${expected}, actual=${actual})"
        else
            echo "[FAIL] ${desc} (expected=${expected}, actual=${actual})"
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
    # its own dedicated check (not the generic "not valid JSON" message —
    # empty input isn't malformed JSON, it's no input at all).
    check_fixture \
        "empty-or-whitespace-input" \
        '   ' \
        "fail:2"

    # Fixture 10 — syntactically invalid JSON -> FAIL closed with rc=2
    # (input rejected before any per-job decision is made).
    check_fixture \
        "malformed-json" \
        'not json' \
        "fail:2"

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
    # job carries additional fields (`outputs`, `outcome`) beyond `result`
    # — not the single-line minimal `{"job":{"result":"..."}}` shape every
    # other fixture above uses. jq itself is whitespace-insensitive, so
    # this fixture is not needed to catch a single-line-only jq bug (that
    # bug class does not exist here) — its value is being the only fixture
    # exercising the FULL real 8-job ci-gate.needs set with realistic
    # sibling fields alongside `result`, as an end-to-end shape check on
    # top of the deliberately minimal fixtures above. Expected: PASS
    # (mutants skipped + allowlisted, every other required job succeeded —
    # the actual shape of a legitimate push-event run).
    check_fixture \
        "realistic-multiline-toJSON-needs-payload" \
        '{
  "fmt": {
    "result": "success",
    "outputs": {},
    "outcome": "success"
  },
  "clippy": {
    "result": "success",
    "outputs": {},
    "outcome": "success"
  },
  "test": {
    "result": "success",
    "outputs": {},
    "outcome": "success"
  },
  "msrv": {
    "result": "success",
    "outputs": {},
    "outcome": "success"
  },
  "deny": {
    "result": "success",
    "outputs": {},
    "outcome": "success"
  },
  "spec-guard": {
    "result": "success",
    "outputs": {},
    "outcome": "success"
  },
  "check-signing-workflow-injection": {
    "result": "success",
    "outputs": {},
    "outcome": "success"
  },
  "mutants": {
    "result": "skipped",
    "outputs": {},
    "outcome": "skipped"
  }
}' \
        "pass"

    echo
    echo "Self-test summary: $((total - mismatches))/${total} fixtures matched their expected outcome."

    if [ "${mismatches}" -gt 0 ]; then
        echo "FAIL: ${mismatches} fixture(s) disagreed with evaluate_needs()."
        echo "      This means the fail-closed decision logic in evaluate_needs()"
        echo "      does not match the expected outcome for one or more fixtures"
        echo "      above — see the [FAIL] line(s) for which fixture(s) and why."
        return 1
    fi

    echo "PASS: all fixtures matched their expected outcome."
    return 0
}

main() {
    if [ "${1:-}" = "--self-test" ]; then
        run_self_test
        exit $?
    fi

    if [ "${1:-}" = "--print-allowed-skips" ]; then
        print_allowed_skips
        exit $?
    fi

    local json
    json="$(cat)"
    evaluate_needs "${json}"
    exit $?
}

main "$@"
