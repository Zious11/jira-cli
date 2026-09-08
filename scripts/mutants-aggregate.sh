#!/usr/bin/env bash
# scripts/mutants-aggregate.sh — sharded mutation-test gate aggregator
# (cycle-006 mutants-ci-sharding). Invoked by ci.yml's `mutants-aggregate`
# job (the `ci-gate.needs` member that replaces the pre-sharding single
# `mutants` job) as the sole pass/fail arbiter for the sharded mutation
# gate. Implements Steps -1..6 (INV-AGG / INV-COMPLETE / INV-ESCALATE) per
# the authoritative design below; every branch was authored via strict
# TDD's RED->GREEN cycle against this file's own `--self-test` fixture
# harness (all 20 fixtures GREEN — S-cycle6-mutants-ci-sharding.md Tasks
# 10-15).
#
# See:
#   - .factory/phase-f2-spec-evolution/cycle-006/ci-yml-design.md §3
#     (the authoritative Step -1..6 pseudo-bash this file mirrors)
#   - .factory/phase-f2-spec-evolution/cycle-006/mutants-sharding-invariants.md
#     (INV-AGG / INV-COMPLETE / INV-ESCALATE statements)
#   - .factory/phase-f2-spec-evolution/cycle-006/architecture-delta.md §6.2a
#     (extraction rationale, --self-test harness shape, dispatcher shape)
#   - .factory/cycles/cycle-006/phase-f3-stories/S-cycle6-mutants-ci-sharding.md
#     (the full story this file implements)
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
# 20 fixtures are wired below in run_mutants_aggregate_self_test(), well
# above AC-031's floor of 12 — every FATAL/warning-only arm named in the
# story's "INV-AGG/INV-COMPLETE Sub-Invariant -> Named RED Fixture"
# mini-table has its own dedicated fixture (AC-001/002x2/003/006/007/008/
# 013/014/022/023/036/037/038/039x2, plus the three previously-unnamed
# per-shard sub-invariant-3/5/6 fixtures, plus a Step-0 push-event no-op
# regression fixture). All 20 fixtures are GREEN against the real
# evaluate_mutants_aggregate() implementation below (S-cycle6-mutants-ci-
# sharding.md Tasks 10-15's RED->GREEN cycle, verified via
# `bash scripts/mutants-aggregate.sh --self-test`). A silently deleted or
# loosened fixture reopens the exact false-green class each one was
# written to catch — do not shrink this count without confirming no
# coverage was lost.
readonly EXPECTED_MUTANTS_AGG_FIXTURES=20

# evaluate_mutants_aggregate — the sole pass/fail arbiter `mutants-aggregate`
# (ci.yml) invokes. Implements Steps -1..6 (INV-AGG / INV-COMPLETE /
# INV-ESCALATE) per ci-yml-design.md §3 and mutants-sharding-invariants.md.
# Reads its inputs from the seven eval-step env vars (EVENT_NAME, ESCALATED,
# MUTANT_COUNT, OVERALL_DIFF_LINES, PLAN_RESULT, STATUS_DIR, SHARD_DIR) —
# see ci.yml's `mutants-aggregate` job's "Evaluate sharded mutation gate"
# step's `env:` mapping for the wiring, byte-pinned in
# tests/ci_gate_completeness.rs.
evaluate_mutants_aggregate() {
  # --- Step -1 (runtime-hardening, architecture-delta.md §6.11): resolve
  #     the trusted jq binary ONCE, before Step 0, and reuse it for every
  #     jq invocation below — mirrors check-ci-gate.sh::evaluate_needs's
  #     own resolve-once-reuse-everywhere discipline (a single TOCTOU-style
  #     PATH mutation mid-function cannot then make different invocations
  #     within this same decision see different binaries). Resolved BEFORE
  #     Step 0's push-event no-op, not lazily at first use in Step 3 — a
  #     compromised jq on PATH is a red flag regardless of which internal
  #     branch would otherwise run; check-ci-gate.sh applies the identical
  #     ordering for the same reason. ---
  local jq_bin
  if ! jq_bin=$(resolve_trusted_jq); then
    return 2
  fi

  # --- Step 0: push-event no-op (this job's only "skipped"-equivalent
  #     path — resolved as an ordinary success, NEVER a GHA `skipped`
  #     conclusion; see the job-level if: always() comment in ci.yml).
  #
  #     Fail-CLOSED allowlist case statement: only a KNOWN, explicitly-
  #     recognized non-PR event exits 0 early; anything else — including
  #     an empty string, and including any FUTURE ci.yml trigger event
  #     nobody has added to this case statement yet — falls through to a
  #     hard FAIL naming the unrecognized value. ---
  case "${EVENT_NAME}" in
    pull_request)
      : # fall through — this is the one event this gate exists for
      ;;
    push|schedule|workflow_dispatch)
      echo "OK: not a pull_request event (${EVENT_NAME}) — mutation gate not applicable, same as the pre-sharding single-job design's push-event skip."
      return 0
      ;;
    *)
      echo "FAIL: EVENT_NAME ('${EVENT_NAME}') is neither 'pull_request' nor a recognized non-PR event (push, schedule, workflow_dispatch). Treating an empty or unrecognized value as a FAIL, not a pass-through — a malformed EVENT_NAME must never be silently interpreted as 'nothing to do here.' If this is a legitimate new ci.yml trigger event, add it to this case statement's non-PR branch explicitly, in the SAME change that adds the trigger."
      return 1
      ;;
  esac

  # --- Step 0.5 (diagnostics improvement, INV-ESCALATE Residual Risk 5):
  #     distinguish "mutants-plan itself crashed" from the generic
  #     missing-shards message, before falling through. ---
  if [ "${PLAN_RESULT}" != "success" ]; then
    echo "FAIL: mutants-plan (diff computation + escalation pre-count) reported '${PLAN_RESULT}', not 'success'. The shard matrix could not have received a valid diff file; treat this as a harness failure, not a kill-rate failure. Check the mutants-plan job's own logs."
    return 1
  fi

  # --- Step 1: INV-ESCALATE — escalation short-circuits before any
  #     shard-artifact inspection. ---
  if [ "${ESCALATED}" = "true" ]; then
    echo "FAIL: PR generates ${MUTANT_COUNT} in-diff mutants, over the 120-mutant threshold for the sharded per-PR gate."
    echo ""
    echo "Two ways forward:"
    echo "  1. PREFERRED: split this PR into smaller, more focused changes."
    echo "  2. If genuinely large and reviewed: a repo admin can merge via"
    echo "     GitHub's branch-protection 'Require approvals' bypass,"
    echo "     explicitly acknowledging the unverified mutation coverage in"
    echo "     the PR description (same audited mechanism already used for a"
    echo "     budget-exceeded cancelled mutants run — see"
    echo "     docs/specs/cargo-mutants-policy.md §F-2)."
    echo ""
    echo "A full, non-diff-scoped run will also occur in the next scheduled"
    echo "nightly full-mutation run regardless of how this PR is merged."
    return 1
  fi

  # ============================================================
  # Steps 2-5 below replace the pre-sharding single-job design's
  # count-based "0 artifacts -> non-empty-diff -> exit 0" branch entirely.
  # See mutants-sharding-invariants.md §INV-COMPLETE for the full
  # rationale. Ordering is load-bearing: presence (Step 2) ->
  # interpretation (Step 3) -> reconciliation (Step 4) ->
  # base-ref-drift/zero-mutant check (Step 5) -> kill-rate (Step 6).
  # ============================================================

  # --- Step 2: INV-COMPLETE Part B — sentinel presence check.
  #     This is the SOLE fail-closed completeness gate; it does NOT look
  #     at outcomes.json at all. STATUS_DIR/SHARD_DIR are read from the
  #     environment (set by ci.yml's `env:` block). Fail loudly (`:?`)
  #     rather than silently operating on an empty path if either is
  #     somehow unset. ---
  STATUS_DIR="${STATUS_DIR:?STATUS_DIR must be set (see ci.yml's env: block)}"
  SHARD_DIR="${SHARD_DIR:?SHARD_DIR must be set (see ci.yml's env: block)}"
  EXPECTED_SHARDS=8  # MUST match ci.yml `mutants` job's
                      # strategy.matrix.shard list length — update
                      # both in the SAME commit
                      # (test_mutants_aggregate_expected_shards_matches_matrix_shard_count
                      # cross-checks this structurally).

  shopt -s nullglob
  sentinel_files=("${STATUS_DIR}"/mutants-shard-status-*/shard-status-*.json)

  missing=()
  for i in $(seq 0 $((EXPECTED_SHARDS - 1))); do
    f="${STATUS_DIR}/mutants-shard-status-${i}/shard-status-${i}.json"
    if [ ! -f "${f}" ]; then
      missing+=("${i}")
    fi
  done
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "FAIL: missing shard status sentinel for shard index/indices: ${missing[*]}"
    echo "      Expected exactly ${EXPECTED_SHARDS} status sentinels; found $(( EXPECTED_SHARDS - ${#missing[@]} ))."
    echo "      A crashed, cancelled, or never-scheduled shard job means its"
    echo "      mutants were never verified — this gate FAILS CLOSED rather"
    echo "      than silently excluding them from the denominator."
    return 1
  fi
  actual_sentinel_count="${#sentinel_files[@]}"
  if [ "${actual_sentinel_count}" -ne "${EXPECTED_SHARDS}" ]; then
    echo "FAIL: ${actual_sentinel_count} shard status sentinel(s) found, expected exactly ${EXPECTED_SHARDS} (duplicate or stray artifact — investigate a re-run/name collision)."
    return 1
  fi

  # --- Step 3: INV-COMPLETE Part C — per-shard interpretation +
  #     per-shard guards (malformed-JSON, integer-validation, H-1
  #     schema-drift, M-2 reconciliation) THEN summation
  #     (INV-AGG). ---
  caught_total=0; missed_total=0; timeout_total=0; unviable_total=0
  for i in $(seq 0 $((EXPECTED_SHARDS - 1))); do
    sentinel="${STATUS_DIR}/mutants-shard-status-${i}/shard-status-${i}.json"
    run_outcome=$("${jq_bin}" -r '.run_outcome' "${sentinel}")
    has_outcomes=$("${jq_bin}" -r '.has_outcomes' "${sentinel}")
    f="${SHARD_DIR}/mutants-shard-outcomes-${i}/outcomes.json"

    if [ "${has_outcomes}" != "true" ]; then
      if [ "${run_outcome}" = "success" ]; then
        echo "OK: shard ${i} legitimately produced 0 mutants (run_outcome=success, has_outcomes=false) — contributes 0."
        continue
      fi
      echo "FAIL: shard ${i}'s run-mutants step did not complete successfully (run_outcome=${run_outcome}) and produced no outcomes.json. Treating as a harness crash, not a legitimate 0-mutant shard."
      return 1
    fi

    # has_outcomes == true: trust the data regardless of
    # run_outcome (a non-zero cargo-mutants exit code under
    # --baseline skip is routine, not evidence of untrustworthy
    # output) — but first defend against a sentinel/data desync.
    if [ ! -f "${f}" ]; then
      echo "FAIL: shard ${i}'s status sentinel claims has_outcomes=true but mutants-shard-outcomes-${i}/outcomes.json was not found in the download. Sentinel/data desync — treating as a failure, not silently skipping."
      return 1
    fi
    if ! "${jq_bin}" empty "${f}" 2>/dev/null; then
      echo "FAIL: shard ${i}'s outcomes.json exists but is malformed JSON."
      return 1
    fi

    caught=$("${jq_bin}" '.caught // 0' "${f}")
    missed=$("${jq_bin}" '.missed // 0' "${f}")
    timeout=$("${jq_bin}" '.timeout // 0' "${f}")
    unviable=$("${jq_bin}" '.unviable // 0' "${f}")
    total_mutants=$("${jq_bin}" '.total_mutants // 0' "${f}")

    [[ "${caught}"        =~ ^[0-9]+$ ]] || caught=0
    [[ "${missed}"        =~ ^[0-9]+$ ]] || missed=0
    [[ "${timeout}"       =~ ^[0-9]+$ ]] || timeout=0
    [[ "${unviable}"      =~ ^[0-9]+$ ]] || unviable=0
    [[ "${total_mutants}" =~ ^[0-9]+$ ]] || total_mutants=0

    _outcomes_len=$("${jq_bin}" '(.outcomes // []) | length' "${f}" 2>/dev/null || echo 0)
    [[ "${_outcomes_len}" =~ ^[0-9]+$ ]] || _outcomes_len=0
    _sum_check=$((caught + missed + timeout + unviable))
    if [ "${_sum_check}" -eq 0 ] && { [ "${_outcomes_len}" -gt 0 ] || [ "${total_mutants}" -ne 0 ]; }; then
      echo "FAIL: shard ${i}'s outcomes.json schema drift detected (non-empty outcomes/total_mutants but all summary keys sum to 0). Pin: cargo-mutants@27.1.0"
      return 1
    fi
    if [ "${total_mutants}" -ne 0 ] && [ "${_sum_check}" -ne "${total_mutants}" ]; then
      echo "::warning::Schema mismatch on shard ${i}: total_mutants=${total_mutants} but sum of known categories=${_sum_check}."
    fi

    caught_total=$((caught_total + caught))
    missed_total=$((missed_total + missed))
    timeout_total=$((timeout_total + timeout))
    unviable_total=$((unviable_total + unviable))
  done

  total_scored=$((caught_total + missed_total + timeout_total + unviable_total))
  echo "Pooled summary: ${caught_total} caught / ${missed_total} missed / ${timeout_total} timeout / ${unviable_total} unviable (across ${EXPECTED_SHARDS} shards)"

  # --- Step 4: INV-AGG sub-invariant 8 — pooled-total <-> pre-count
  #     reconciliation. Exact equality, both directions
  #     (`total_scored != MUTANT_COUNT`, not a directional
  #     `total_scored >= MUTANT_COUNT`) — an over-count is also treated
  #     as dangerous, not merely anomalous. This is a HARD FAIL, not a
  #     `::warning::`. ---
  [[ "${MUTANT_COUNT}" =~ ^[0-9]+$ ]] || { echo "FAIL: MUTANT_COUNT ('${MUTANT_COUNT}') from mutants-plan is not a valid non-negative integer — cannot reconcile."; return 1; }
  if [ "${total_scored}" -ne "${MUTANT_COUNT}" ]; then
    echo "FAIL: Pooled scored-mutant count (${total_scored}) does not reconcile with mutants-plan's pre-count (MUTANT_COUNT=${MUTANT_COUNT})."
    echo "      This means the mutant set actually examined by the shard matrix differs from the set mutants-plan counted as in-diff-scope — either mutants went missing between planning and shard execution (a dropped, possibly-surviving mutant would silently pass otherwise), or the shard matrix examined more than was planned. Failing closed rather than trusting a partial or over-scoped pooled total. See mutants-sharding-invariants.md §INV-AGG sub-invariant 8's round-5 callout if this fires on a legitimate PR — root-cause before assuming this check is wrong."
    return 1
  fi

  # --- Step 5: base-ref-drift guard (moved here per Path B item 5).
  #     `total_scored` (the shards' OWN pooled total, folded in Step 3
  #     from their own outcomes.json data — NOT MUTANT_COUNT) is the
  #     discriminator for "legitimately nothing to gate on" —
  #     OVERALL_DIFF_LINES is consulted only to explain WHY it is zero,
  #     never as the primary completeness signal. With Step 4's hard
  #     fail above, this branch is reachable ONLY when
  #     `total_scored == MUTANT_COUNT == 0` — Step 4 already returned 1
  #     for ANY mismatch, including a `MUTANT_COUNT > 0` that reconciled
  #     down to a `total_scored` of 0 through a dropped-mutant defect. ---
  [[ "${OVERALL_DIFF_LINES:-0}" =~ ^[0-9]+$ ]] || { echo "FAIL: OVERALL_DIFF_LINES ('${OVERALL_DIFF_LINES:-}') from mutants-plan is not a valid non-negative integer — cannot evaluate the base-ref-drift guard."; return 1; }
  if [ "${total_scored}" -eq 0 ]; then
    if [ "${OVERALL_DIFF_LINES:-0}" -eq 0 ]; then
      echo "FAIL: 0 mutants scored (MUTANT_COUNT=0, already reconciled at Step 4) AND overall diff is EMPTY."
      echo "      Possible base-ref drift — same F-3 signature as the"
      echo "      pre-sharding single-job design."
      return 1
    fi
    echo "OK: 0 mutants scored — MUTANT_COUNT=0 (reconciled at Step 4; this is the only way to reach 0 scored mutants under the restored hard fail) — non-empty diff produced no mutable lines in examine_globs files (comment-only, whitespace, docs-only, or non-scoped-file PR)."
    return 0
  fi

  # --- Step 6: kill-rate computation (INV-AGG). This step reads only
  #     caught_total/missed_total/timeout_total/unviable_total, folded in
  #     Step 3 from the shards' own outcomes.json — it never reads
  #     MUTANT_COUNT. Step 6 is UNREACHABLE whenever Step 4 finds a
  #     mismatch (it already `return`ed 1 above): an incomplete or
  #     unreconciled mutant set is never allowed to reach the kill-rate
  #     decision at all. ---
  killable=$((caught_total + missed_total + timeout_total))
  if [ "${killable}" -eq 0 ]; then
    echo "OK: ${total_scored} mutant(s) generated, all unviable."
    return 0
  fi

  kill_rate=$(( (caught_total * 100) / killable ))
  echo "Pooled kill rate: ${kill_rate}% (target >= 90%)"

  if [ "${kill_rate}" -lt 90 ]; then
    echo "FAIL: pooled kill rate ${kill_rate}% is below the 90% target."
    return 1
  fi

  echo "OK: sharded cargo-mutants gate passed (pooled kill rate ${kill_rate}% >= 90%)."
  return 0
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

# run_mutants_aggregate_self_test — the fixture harness proving
# evaluate_mutants_aggregate()'s decision logic. Mirrors
# scripts/check-ci-gate.sh::run_self_test's check_fixture harness shape
# (architecture-delta.md §6.2a): every fixture below is a genuine,
# same-process call into evaluate_mutants_aggregate() (no subprocess, exactly
# like check-ci-gate.sh's evaluate_needs) against a synthetic STATUS_DIR/
# SHARD_DIR filesystem tree plus the seven eval-step env vars. Every
# fixture was authored RED-before-GREEN (S-cycle6-mutants-ci-sharding.md
# Tasks 10-15) and is GREEN today against the real
# evaluate_mutants_aggregate() body above — this file's own fixtures must
# not be loosened to accommodate a future implementation change; fix the
# implementation instead.
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
