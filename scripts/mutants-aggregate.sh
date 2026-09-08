#!/usr/bin/env bash
# scripts/mutants-aggregate.sh — sharded mutation-test gate aggregator
# (cycle-006 mutants-ci-sharding).
#
# STATUS: SCAFFOLD ONLY (F4 Blocking Precondition 2b,
# S-cycle6-mutants-ci-sharding.md Task 2). This file exists, parses, is
# executable, sources the shared trusted-jq resolver, and exposes the
# `evaluate_mutants_aggregate()` entry point + a `--self-test` dispatch —
# but its actual Step -1 through Step 6 decision-logic BODY
# (INV-AGG / INV-COMPLETE / INV-ESCALATE) is deliberately NOT implemented
# here. Every line of that logic is authored ONLY via the Tasks 10-15
# RED->GREEN cycle: write the failing `--self-test` fixture first, then
# the minimum code to pass it. Writing the decision-logic bodies in this
# scaffold task would make every later fixture GREEN on creation, violating
# strict TDD's Red Gate (BC-5.38.001/BC-5.38.005) — a fixture must fail
# before the code that satisfies it exists.
#
# See:
#   - .factory/phase-f2-spec-evolution/cycle-006/ci-yml-design.md §3
#     (the authoritative Step -1..6 pseudo-bash this file will mirror)
#   - .factory/phase-f2-spec-evolution/cycle-006/mutants-sharding-invariants.md
#     (INV-AGG / INV-COMPLETE / INV-ESCALATE statements)
#   - .factory/phase-f2-spec-evolution/cycle-006/architecture-delta.md §6.2a
#     (extraction rationale, --self-test harness shape, dispatcher shape)
#   - .factory/cycles/cycle-006/phase-f3-stories/S-cycle6-mutants-ci-sharding.md
#     Task 2 (this scaffold), Tasks 10-15 (the RED->GREEN cycle that fills
#     this file in)
#
# TODO(F4-GREEN): decision logic per ci-yml-design.md §3 + invariants.
set -euo pipefail

# Explicit syntax self-check — same repo convention scripts/check-ci-gate.sh
# documents and relies on (see that file's own header comment for the full
# rationale); kept here for consistency even though, as there, every
# function below is fully defined before `main "$@"` ever runs.
bash -n "${BASH_SOURCE[0]}"

# Source the shared, PATH-shim-resistant jq resolver (architecture-delta.md
# §6.11, round-7) — pure-bash directory computation, no external `dirname`
# call, so there is nothing on PATH left to shim for the SOURCE path itself
# (ADV-P61-HIGH-001's fix, applied here identically to
# scripts/check-ci-gate.sh's own preamble).
_mutants_agg_self="${BASH_SOURCE[0]}"
if [ "${_mutants_agg_self}" = "${_mutants_agg_self#*/}" ]; then
    _mutants_agg_dir="."          # no slash at all -> invoked from cwd
else
    _mutants_agg_dir="${_mutants_agg_self%/*}"
    [ -z "${_mutants_agg_dir}" ] && _mutants_agg_dir="/"
fi
# shellcheck source=lib/trusted-jq.sh
source "${_mutants_agg_dir}/lib/trusted-jq.sh"
unset _mutants_agg_self _mutants_agg_dir

# EXPECTED_MUTANTS_AGG_FIXTURES — sibling to check-ci-gate.sh's
# EXPECTED_FIXTURES fixed-denominator pin (ADV-P61-INFO-006 pattern).
# Test-writer RED phase (cycle-006 mutants-ci-sharding, Tasks 10/12/14):
# 20 fixtures are wired below in run_mutants_aggregate_self_test(), well
# above AC-031's floor of 12 — every FATAL/warning-only arm named in the
# story's "INV-AGG/INV-COMPLETE Sub-Invariant -> Named RED Fixture"
# mini-table has its own dedicated fixture (AC-001/002x2/003/006/007/008/
# 013/014/022/023/036/037/038/039x2, plus the three previously-unnamed
# per-shard sub-invariant-3/5/6 fixtures, plus a Step-0 push-event no-op
# regression fixture). Every fixture below is a genuine RED proof today:
# evaluate_mutants_aggregate() is still the Task-2b scaffold stub (always
# returns 1 with a generic TODO message), so a "pass"-expecting fixture
# fails on exit code and a "fail"-expecting fixture fails on its
# diagnostic-substring assertion (the TODO text never contains the real
# diagnostic). The implementer re-verifies this count mechanically at F4
# per AC-031 before relying on it as final.
readonly EXPECTED_MUTANTS_AGG_FIXTURES=20

# evaluate_mutants_aggregate — the sole pass/fail arbiter `mutants-aggregate`
# (ci.yml) invokes. SCAFFOLD STUB ONLY — see the file header above. Every
# Step -1..6 branch (trusted-jq resolution, the push-event no-op allowlist,
# the escalation short-circuit, sentinel presence/interpretation, pooled
# summation, the MUTANT_COUNT reconciliation, the base-ref-drift guard, and
# the kill-rate computation) is DEFERRED to Tasks 10-15's RED->GREEN cycle.
# This stub is intentionally a no-op that reports "not yet implemented" —
# it must never be mistaken for a legitimate PASS or FAIL decision.
evaluate_mutants_aggregate() {
  # TODO(F4-GREEN): decision logic per ci-yml-design.md §3 + invariants
  # (INV-AGG / INV-COMPLETE / INV-ESCALATE). See Tasks 10-15 in
  # S-cycle6-mutants-ci-sharding.md — every branch below this comment is
  # authored ONLY once its own failing --self-test fixture exists first.
  echo "TODO(F4-GREEN): evaluate_mutants_aggregate() is a scaffold stub —" >&2
  echo "decision logic (INV-AGG/INV-COMPLETE/INV-ESCALATE) is not yet" >&2
  echo "implemented. See S-cycle6-mutants-ci-sharding.md Task 2b and" >&2
  echo "Tasks 10-15 for the RED->GREEN cycle that fills this function in." >&2
  return 1
}

# ---------------------------------------------------------------------------
# --self-test fixture-tree helpers (Tasks 10/12/14 RED phase). Build a
# synthetic STATUS_DIR/SHARD_DIR tree matching the exact layout
# evaluate_mutants_aggregate()'s Step 2/3 will read once implemented
# (ci-yml-design.md §3: STATUS_DIR/mutants-shard-status-<i>/shard-status-<i>.json,
# SHARD_DIR/mutants-shard-outcomes-<i>/outcomes.json).
# ---------------------------------------------------------------------------

_agg_tmp_dirs=()

_agg_mktemp_dir() {
    local d
    d=$(mktemp -d)
    _agg_tmp_dirs+=("${d}")
    printf '%s' "${d}"
}

_agg_cleanup_tmp_dirs() {
    local d
    for d in "${_agg_tmp_dirs[@]:-}"; do
        [ -n "${d}" ] && [ -d "${d}" ] && rm -rf "${d}"
    done
    _agg_tmp_dirs=()
}

# _agg_write_sentinel <status_dir> <idx> <run_outcome> <has_outcomes: true|false>
_agg_write_sentinel() {
    local dir="$1" idx="$2" run_outcome="$3" has_outcomes="$4"
    mkdir -p "${dir}/mutants-shard-status-${idx}"
    printf '{"shard_index": %s, "run_outcome": "%s", "has_outcomes": %s}\n' \
        "${idx}" "${run_outcome}" "${has_outcomes}" \
        > "${dir}/mutants-shard-status-${idx}/shard-status-${idx}.json"
}

# _agg_write_outcomes <shard_dir> <idx> <caught> <missed> <timeout> <unviable> [total_mutants] [outcomes_array_literal]
_agg_write_outcomes() {
    local dir="$1" idx="$2" caught="$3" missed="$4" timeout="$5" unviable="$6"
    local total="${7:-}" outcomes_literal="${8:-[]}"
    mkdir -p "${dir}/mutants-shard-outcomes-${idx}"
    if [ -z "${total}" ]; then
        total=$((caught + missed + timeout + unviable))
    fi
    printf '{"caught": %s, "missed": %s, "timeout": %s, "unviable": %s, "total_mutants": %s, "outcomes": %s}\n' \
        "${caught}" "${missed}" "${timeout}" "${unviable}" "${total}" "${outcomes_literal}" \
        > "${dir}/mutants-shard-outcomes-${idx}/outcomes.json"
}

# _agg_write_all_legit_empty <status_dir> <first_idx> <last_idx> — each
# shard gets a "legitimately produced 0 mutants" sentinel (run_outcome=
# success, has_outcomes=false, INV-COMPLETE Part C legit-empty arm), no
# outcomes.json at all.
_agg_write_all_legit_empty() {
    local dir="$1" first="$2" last="$3" i
    for ((i = first; i <= last; i++)); do
        _agg_write_sentinel "${dir}" "${i}" "success" "false"
    done
}

# run_mutants_aggregate_self_test — Tasks 10/12/14 RED-phase fixture harness.
# Mirrors scripts/check-ci-gate.sh::run_self_test's check_fixture harness
# shape (architecture-delta.md §6.2a): every fixture below is a genuine,
# same-process call into evaluate_mutants_aggregate() (no subprocess, exactly
# like check-ci-gate.sh's evaluate_needs) against a synthetic STATUS_DIR/
# SHARD_DIR filesystem tree plus the seven eval-step env vars. Every fixture
# is RED today: evaluate_mutants_aggregate() is still the Task-2b scaffold
# stub (unconditionally returns 1 with a generic TODO message on stderr), so
# a "pass"-expecting fixture fails on exit code and a "fail"-expecting
# fixture fails on its diagnostic-substring assertion (the stub's TODO text
# never contains the real diagnostic). Tasks 11/13/15 fill in
# evaluate_mutants_aggregate()'s real body to turn these GREEN, one
# RED->GREEN pair at a time — this file's own fixtures must not be
# loosened to make that easier.
run_mutants_aggregate_self_test() {
    echo "=== mutants-aggregate.sh SELF-TEST (cycle-006 mutants-ci-sharding) ==="
    echo

    trap _agg_cleanup_tmp_dirs RETURN

    local total=0
    local mismatches=0

    # agg_check_fixture <desc> <expected: "pass"|"fail:<rc>"> [expected_substring] [forbidden_substring]
    # Reads its inputs from the AGG_* variables the caller sets immediately
    # before invoking this function (bash dynamic scoping — mirrors
    # check-ci-gate.sh::check_fixture's direct read/write of its enclosing
    # function's `total`/`mismatches` locals, the same mechanism this file
    # relies on here).
    agg_check_fixture() {
        local desc="$1" expected="$2" expected_substring="${3:-}" forbidden_substring="${4:-}"
        total=$((total + 1))

        local output rc=0
        output=$(
            EVENT_NAME="${AGG_EVENT_NAME}" \
            ESCALATED="${AGG_ESCALATED}" \
            MUTANT_COUNT="${AGG_MUTANT_COUNT}" \
            OVERALL_DIFF_LINES="${AGG_OVERALL_DIFF_LINES}" \
            PLAN_RESULT="${AGG_PLAN_RESULT}" \
            STATUS_DIR="${AGG_STATUS_DIR}" \
            SHARD_DIR="${AGG_SHARD_DIR}" \
            evaluate_mutants_aggregate 2>&1
        ) || rc=$?

        local actual
        if [ "${rc}" -eq 0 ]; then
            actual="pass"
        else
            actual="fail:${rc}"
        fi

        local ok=true
        [ "${actual}" = "${expected}" ] || ok=false
        if [ -n "${expected_substring}" ] && ! grep -qF -- "${expected_substring}" <<<"${output}"; then
            ok=false
        fi
        if [ -n "${forbidden_substring}" ] && grep -qF -- "${forbidden_substring}" <<<"${output}"; then
            ok=false
        fi

        if [ "${ok}" = true ]; then
            echo "[PASS] ${desc} (expected=${expected}, actual=${actual})"
        else
            echo "[FAIL] ${desc} (expected=${expected}, actual=${actual})"
            [ -n "${expected_substring}" ] && echo "       expected output to contain: \"${expected_substring}\""
            [ -n "${forbidden_substring}" ] && echo "       expected output to NOT contain: \"${forbidden_substring}\""
            echo "       --- evaluate_mutants_aggregate output ---"
            while IFS= read -r line; do
                echo "       ${line}"
            done <<<"${output}"
            mismatches=$((mismatches + 1))
        fi
    }

    local AGG_EVENT_NAME AGG_ESCALATED AGG_MUTANT_COUNT AGG_OVERALL_DIFF_LINES AGG_PLAN_RESULT
    local AGG_STATUS_DIR AGG_SHARD_DIR
    local i

    # ==== Fixture 1 (AC-001) — pooled sum-not-average kill rate, healthy 90% ====
    AGG_EVENT_NAME="pull_request"; AGG_ESCALATED="false"; AGG_PLAN_RESULT="success"
    AGG_OVERALL_DIFF_LINES="250"
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    for i in 0 1 2 3 4 5 6 7; do _agg_write_sentinel "${AGG_STATUS_DIR}" "${i}" "success" "true"; done
    _agg_write_outcomes "${AGG_SHARD_DIR}" 0 90 8 2 0 100
    for i in 1 2 3 4 5 6 7; do _agg_write_outcomes "${AGG_SHARD_DIR}" "${i}" 0 0 0 0 0; done
    AGG_MUTANT_COUNT="100"
    agg_check_fixture \
        "AC-001: pooled sum-not-average kill rate — healthy 90% passes" \
        "pass" "gate passed"

    # ==== Fixture 2 (AC-002, Part B arm 1) — missing shard sentinel fails closed ====
    AGG_EVENT_NAME="pull_request"; AGG_ESCALATED="false"; AGG_PLAN_RESULT="success"
    AGG_MUTANT_COUNT="0"; AGG_OVERALL_DIFF_LINES="10"
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    for i in 0 1 2 3 4 5 6; do _agg_write_sentinel "${AGG_STATUS_DIR}" "${i}" "success" "false"; done
    agg_check_fixture \
        "AC-002 Part B: missing shard 7 sentinel fails closed" \
        "fail:1" "missing shard status sentinel"

    # ==== Fixture 3 (AC-002, Part B arm 2) — duplicate shard sentinel fails closed ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    for i in 0 1 2 3 4 5 6 7; do _agg_write_sentinel "${AGG_STATUS_DIR}" "${i}" "success" "false"; done
    mkdir -p "${AGG_STATUS_DIR}/mutants-shard-status-0-dup"
    printf '{"shard_index": 0, "run_outcome": "success", "has_outcomes": false}\n' \
        > "${AGG_STATUS_DIR}/mutants-shard-status-0-dup/shard-status-0.json"
    agg_check_fixture \
        "AC-002 Part B: duplicate/stray shard sentinel artifact fails closed" \
        "fail:1" "duplicate or stray artifact"

    # ==== Fixture 4 (AC-006, CRIT-1) — all 8 shards crash, fails closed ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    for i in 0 1 2 3 4 5 6 7; do _agg_write_sentinel "${AGG_STATUS_DIR}" "${i}" "failure" "false"; done
    agg_check_fixture \
        "AC-006 (CRIT-1 regression guard): all 8 shards crash fails closed" \
        "fail:1" "Treating as a harness crash"

    # ==== Fixture 5 (AC-007, HIGH-1) — legitimately-empty shards pass ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    _agg_write_sentinel "${AGG_STATUS_DIR}" 0 "success" "true"
    _agg_write_outcomes "${AGG_SHARD_DIR}" 0 45 5 0 0 50
    _agg_write_all_legit_empty "${AGG_STATUS_DIR}" 1 7
    AGG_MUTANT_COUNT="50"
    agg_check_fixture \
        "AC-007 (HIGH-1 regression guard): legitimately-empty shards contribute 0 and pass" \
        "pass" "legitimately produced 0 mutants"

    # ==== Fixture 6 (AC-036) — fold-wins arm: has_outcomes=true folds despite run_outcome=failure ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    _agg_write_sentinel "${AGG_STATUS_DIR}" 0 "failure" "true"
    _agg_write_outcomes "${AGG_SHARD_DIR}" 0 90 8 2 0 100
    _agg_write_all_legit_empty "${AGG_STATUS_DIR}" 1 7
    AGG_MUTANT_COUNT="100"
    agg_check_fixture \
        "AC-036: has_outcomes=true always wins over run_outcome (fold, not exclude)" \
        "pass" "Pooled summary: 90 caught"

    # ==== Fixture 7 (AC-037, EC-018) — sentinel/data desync fails closed ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    _agg_write_sentinel "${AGG_STATUS_DIR}" 0 "success" "true"
    # deliberately no outcomes.json written for shard 0 — the desync.
    _agg_write_all_legit_empty "${AGG_STATUS_DIR}" 1 7
    agg_check_fixture \
        "AC-037: sentinel claims has_outcomes=true but outcomes.json is absent (desync) fails closed" \
        "fail:1" "Sentinel/data desync"

    # ==== Fixture 8 (INV-AGG sub-invariant 3) — per-shard malformed JSON fails closed ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    _agg_write_sentinel "${AGG_STATUS_DIR}" 0 "success" "true"
    mkdir -p "${AGG_SHARD_DIR}/mutants-shard-outcomes-0"
    printf '{not valid json' > "${AGG_SHARD_DIR}/mutants-shard-outcomes-0/outcomes.json"
    _agg_write_all_legit_empty "${AGG_STATUS_DIR}" 1 7
    agg_check_fixture \
        "INV-AGG sub-invariant 3: per-shard malformed outcomes.json fails the whole aggregation closed" \
        "fail:1" "malformed JSON"

    # ==== Fixture 9 (INV-AGG sub-invariant 5 / H-1) — per-shard schema drift fails closed ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    _agg_write_sentinel "${AGG_STATUS_DIR}" 0 "success" "true"
    _agg_write_outcomes "${AGG_SHARD_DIR}" 0 0 0 0 0 0 '["m1","m2","m3"]'
    _agg_write_all_legit_empty "${AGG_STATUS_DIR}" 1 7
    agg_check_fixture \
        "INV-AGG sub-invariant 5 (H-1): non-empty outcomes but all summary keys sum to 0 is schema drift, fails closed" \
        "fail:1" "schema drift detected"

    # ==== Fixture 10 (INV-AGG sub-invariant 6 / M-2) — total_mutants mismatch is warning-only, non-fatal ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    for i in 0 1 2 3 4 5 6 7; do _agg_write_sentinel "${AGG_STATUS_DIR}" "${i}" "success" "true"; done
    _agg_write_outcomes "${AGG_SHARD_DIR}" 0 90 8 2 0 105  # total_mutants=105 != sum=100
    for i in 1 2 3 4 5 6 7; do _agg_write_outcomes "${AGG_SHARD_DIR}" "${i}" 0 0 0 0 0; done
    AGG_MUTANT_COUNT="100"
    agg_check_fixture \
        "INV-AGG sub-invariant 6 (M-2): per-shard total_mutants mismatch warns but does not fail the job" \
        "pass" "::warning::Schema mismatch on shard 0"

    # ==== Fixture 11 (AC-008) — pooled-total < MUTANT_COUNT (undercount) hard fails ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    _agg_write_sentinel "${AGG_STATUS_DIR}" 0 "success" "true"
    _agg_write_outcomes "${AGG_SHARD_DIR}" 0 90 8 2 0 100
    _agg_write_all_legit_empty "${AGG_STATUS_DIR}" 1 7
    AGG_MUTANT_COUNT="101"
    agg_check_fixture \
        "AC-008: total_scored (100) < MUTANT_COUNT (101) — under-count hard-fails, exact equality" \
        "fail:1" "does not reconcile"

    # ==== Fixture 12 (AC-022) — pooled-total > MUTANT_COUNT (overcount) hard fails, symmetric ====
    AGG_MUTANT_COUNT="99"
    agg_check_fixture \
        "AC-022: total_scored (100) > MUTANT_COUNT (99) — over-count hard-fails, symmetric with AC-008" \
        "fail:1" "does not reconcile"

    # ==== Fixture 13 (AC-023) — a healthy kill rate never rescues a reconciliation mismatch ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    _agg_write_sentinel "${AGG_STATUS_DIR}" 0 "success" "true"
    _agg_write_outcomes "${AGG_SHARD_DIR}" 0 95 3 2 0 100  # kill_rate=95%, deliberately healthy-looking
    _agg_write_all_legit_empty "${AGG_STATUS_DIR}" 1 7
    AGG_MUTANT_COUNT="101"
    agg_check_fixture \
        "AC-023: healthy 95% kill rate does not rescue a reconciliation mismatch — completeness over quality" \
        "fail:1" "does not reconcile" "gate passed"

    # ==== Fixture 14 (AC-013) — malformed-but-set OVERALL_DIFF_LINES fails closed ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    for i in 0 1 2 3 4 5 6 7; do _agg_write_sentinel "${AGG_STATUS_DIR}" "${i}" "success" "true"; done
    _agg_write_outcomes "${AGG_SHARD_DIR}" 0 90 8 2 0 100
    for i in 1 2 3 4 5 6 7; do _agg_write_outcomes "${AGG_SHARD_DIR}" "${i}" 0 0 0 0 0; done
    AGG_MUTANT_COUNT="100"; AGG_OVERALL_DIFF_LINES="not-a-number"
    agg_check_fixture \
        "AC-013: malformed-but-set OVERALL_DIFF_LINES fails closed (symmetric with MUTANT_COUNT's Step-4 guard)" \
        "fail:1" "not a valid non-negative integer"

    # ==== Fixture 15 (AC-039, arm a) — base-ref-drift FAILs when 0 scored and 0 diff lines ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    _agg_write_all_legit_empty "${AGG_STATUS_DIR}" 0 7
    AGG_MUTANT_COUNT="0"; AGG_OVERALL_DIFF_LINES="0"
    agg_check_fixture \
        "AC-039 arm a: total_scored=0 and OVERALL_DIFF_LINES=0 — base-ref-drift FAIL" \
        "fail:1" "Possible base-ref drift"

    # ==== Fixture 16 (AC-039, arm b) — base-ref-drift OKs when 0 scored but diff lines > 0 ====
    AGG_OVERALL_DIFF_LINES="42"
    agg_check_fixture \
        "AC-039 arm b: total_scored=0 but OVERALL_DIFF_LINES=42 — '0 mutants scored' OK, mirror of arm a" \
        "pass" "0 mutants scored"

    # ==== Fixture 17 (AC-003) — escalation short-circuits BEFORE any shard-artifact inspection ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    # deliberately zero sentinels present — proves Step 1 (escalation) is
    # reached and returns before Step 2's sentinel-presence check would.
    AGG_ESCALATED="true"; AGG_MUTANT_COUNT="281"; AGG_OVERALL_DIFF_LINES="500"
    agg_check_fixture \
        "AC-003: ESCALATED=true short-circuits before shard-sentinel inspection, ordinary failure not skip" \
        "fail:1" "over the 120-mutant threshold" "missing shard status sentinel"
    AGG_ESCALATED="false"

    # ==== Fixture 18 (AC-038) — Step 0.5 PLAN_RESULT != success short-circuits BEFORE Step 2 ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    # deliberately zero sentinels present — proves the mutants-plan-specific
    # diagnostic is reached and returned before Step 2's generic
    # missing-sentinel message would fire for the identical root cause.
    AGG_PLAN_RESULT="failure"
    agg_check_fixture \
        "AC-038: PLAN_RESULT != 'success' fails closed with a mutants-plan-specific diagnostic before Step 2" \
        "fail:1" "mutants-plan (diff computation" "missing shard status sentinel"
    AGG_PLAN_RESULT="success"

    # ==== Fixture 19 (AC-014) — Step 0 fail-closed allowlist rejects an unrecognized/empty EVENT_NAME ====
    AGG_STATUS_DIR=$(_agg_mktemp_dir); AGG_SHARD_DIR=$(_agg_mktemp_dir)
    AGG_EVENT_NAME=""
    agg_check_fixture \
        "AC-014: empty/unrecognized EVENT_NAME fails closed via the allowlist case, never a silent pass-through" \
        "fail:1" "neither 'pull_request' nor a recognized non-PR event"

    # ==== Fixture 20 (Step 0 regression) — a genuinely non-PR event (push) is a legitimate no-op pass ====
    AGG_EVENT_NAME="push"
    agg_check_fixture \
        "Step 0 regression: EVENT_NAME=push is a legitimate no-op pass, not a failure" \
        "pass" "not a pull_request event"
    AGG_EVENT_NAME="pull_request"

    echo
    if [ "${total}" != "${EXPECTED_MUTANTS_AGG_FIXTURES}" ]; then
        echo "SELF-TEST-FIXTURE-COUNT: expected ${EXPECTED_MUTANTS_AGG_FIXTURES} fixtures," \
             "found ${total}. A silently deleted or added fixture changes" \
             "coverage of the INV-AGG/INV-COMPLETE/INV-ESCALATE arms this" \
             "file's own mini-table names — every fixture above is load-bearing;" \
             "update EXPECTED_MUTANTS_AGG_FIXTURES ONLY after confirming no" \
             "coverage was lost."
        mismatches=$((mismatches + 1))
    fi

    echo
    if [ "${mismatches}" -eq 0 ]; then
        echo "PASS: all ${total} fixtures matched."
        return 0
    else
        echo "FAIL: ${mismatches} of ${total} fixture(s) mismatched."
        return 1
    fi
}

# main <args...> — real invocation runs the evaluator once and exits with
# its return code; `--self-test` instead runs the (currently stubbed)
# fixture harness. Mirrors scripts/check-ci-gate.sh::main's dispatch shape.
main() {
    if [ "${1:-}" = "--self-test" ]; then
        run_mutants_aggregate_self_test
        exit $?
    fi
    evaluate_mutants_aggregate
    exit $?
}

main "$@"
