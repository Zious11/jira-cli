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
#   2 — malformed input (not valid JSON) or missing `jq`
#
# SELF-TEST: pass --self-test to run the built-in fixture suite (proves the
# decision logic is not a no-op; modeled on
# scripts/check-signing-workflow-injection.sh's --self-test convention,
# DEC-148/DEC-150 pattern). Wired into `spec-guard` (NOT `ci-gate` — a gate
# cannot depend on a job that depends on it).
#
# USAGE:
#   echo "$NEEDS_JSON" | scripts/check-ci-gate.sh   # canonical CI invocation
#   scripts/check-ci-gate.sh --self-test            # offline fixture suite

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
export REPO_ROOT # referenced defensively; keeps parity with sibling scripts' layout

# Validate script syntax on every invocation (catches accidental bash syntax
# errors in this file itself before any logic runs).
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

    if ! echo "${json}" | jq empty >/dev/null 2>&1; then
        echo "ERROR: input is not valid JSON." >&2
        return 2
    fi

    local jobs
    jobs=$(echo "${json}" | jq -r 'keys[]' 2>/dev/null || true)

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
        result=$(echo "${json}" | jq -r --arg j "${job}" '.[$j].result')

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
# Each fixture asserts an EXPECTED outcome (pass = exit 0, fail = non-zero
# exit) against evaluate_needs(). This proves the decision logic is not a
# no-op: every fixture below was independently proven RED against the
# Red Gate stub (which always returned 0 regardless of input) before the
# real fail-closed logic in evaluate_needs() was written.
# ---------------------------------------------------------------------------
run_self_test() {
    echo "=== check-ci-gate.sh SELF-TEST (S-CIGATE-2) ==="
    echo

    local total=0
    local mismatches=0

    # check_fixture <description> <json> <expected: pass|fail>
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
            actual="fail"
        fi

        if [ "${actual}" = "${expected}" ]; then
            echo "[PASS] ${desc} (expected=${expected}, actual=${actual})"
        else
            echo "[FAIL] ${desc} (expected=${expected}, actual=${actual}, rc=${rc})"
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

    # Fixture 2 — one job failure -> FAIL.
    check_fixture \
        "one-job-failure" \
        '{"fmt":{"result":"failure"},"clippy":{"result":"success"}}' \
        "fail"

    # Fixture 3 — an UNLISTED job reports skipped -> FAIL (only ALLOWED_SKIPS
    # members may tolerate skipped).
    check_fixture \
        "unlisted-job-skipped" \
        '{"fmt":{"result":"skipped"},"clippy":{"result":"success"}}' \
        "fail"

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
        "fail"

    # Fixture 6 — a job reports cancelled -> FAIL.
    check_fixture \
        "job-cancelled" \
        '{"fmt":{"result":"cancelled"},"clippy":{"result":"success"}}' \
        "fail"

    # Fixture 7 — a job reports an invented/unknown result string -> FAIL via
    # the default arm (the structural fix: today's condition allowlists
    # known-bad values, so an unrecognized future value must not pass
    # through unnoticed).
    check_fixture \
        "unrecognized-result-value" \
        '{"fmt":{"result":"action_required"},"clippy":{"result":"success"}}' \
        "fail"

    # Fixture 8 — empty needs context -> FAIL closed (a gate with nothing to
    # check must not vacuously pass).
    check_fixture \
        "empty-needs" \
        '{}' \
        "fail"

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

    local json
    json="$(cat)"
    evaluate_needs "${json}"
    exit $?
}

main "$@"
