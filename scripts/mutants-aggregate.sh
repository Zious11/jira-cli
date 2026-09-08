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
# EXPECTED_FIXTURES fixed-denominator pin (ADV-P61-INFO-006 pattern):
# once the Tasks 10-15 RED->GREEN cycle lands real fixtures inside
# run_mutants_aggregate_self_test(), this constant must be MECHANICALLY
# recounted against the actual fixture set (AC-031: floor of 12, but the
# implementer must re-derive the true F4 value, not copy a number from a
# doc) rather than assumed. Zero fixtures exist in this scaffold, so this
# constant is declared but not yet load-bearing — do not wire a
# total-vs-EXPECTED comparison into run_mutants_aggregate_self_test()
# until real fixtures exist (Task 10/12/14 onward); doing so prematurely
# would either be vacuously 0==0 (no real proof) or require guessing a
# number ahead of the fixtures that justify it.
readonly EXPECTED_MUTANTS_AGG_FIXTURES=12

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

# run_mutants_aggregate_self_test — SCAFFOLD SKELETON ONLY. Mirrors
# scripts/check-ci-gate.sh::run_self_test's check_fixture harness shape
# (architecture-delta.md §6.2a), but carries zero real fixtures yet — every
# fixture in the "INV-AGG/INV-COMPLETE Sub-Invariant -> Named RED Fixture"
# mini-table (S-cycle6-mutants-ci-sharding.md) is added by Tasks 10-15, one
# RED->GREEN pair at a time. This function deliberately returns non-zero
# unconditionally so `--self-test` does NOT pass yet (Red Gate,
# BC-5.38.001) — a passing self-test with zero real fixtures would be a
# false-green self-test, not a scaffold.
run_mutants_aggregate_self_test() {
    echo "=== mutants-aggregate.sh SELF-TEST (cycle-006 mutants-ci-sharding) ==="
    echo
    echo "TODO(F4-GREEN): fixture harness not yet implemented. The"
    echo "INV-AGG/INV-COMPLETE/INV-ESCALATE decision logic in"
    echo "evaluate_mutants_aggregate() and its RED fixtures land together via"
    echo "the Tasks 10-15 RED->GREEN cycle (S-cycle6-mutants-ci-sharding.md;"
    echo "architecture-delta.md §6.2a). This scaffold intentionally fails so"
    echo "the Red Gate holds until those fixtures exist (target floor:"
    echo "EXPECTED_MUTANTS_AGG_FIXTURES=${EXPECTED_MUTANTS_AGG_FIXTURES}, to be"
    echo "mechanically re-verified against the shipped fixture set at F4)."
    return 1
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
