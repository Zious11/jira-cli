#!/usr/bin/env bash
set -euo pipefail

# run-tests.sh — fixture self-test for check-bc-cumulative-counts.sh (DRIFT-002)
#
# Invokes scripts/check-bc-cumulative-counts.sh against each fixture mini-tree
# and asserts the expected exit code. Run from the repo root:
#
#   bash tests/spec-count-fixtures/run-tests.sh
#
# Expected output (after guard script is implemented):
#   PASS: known-good exits 0
#   PASS: total_bcs drift exits 1
#   PASS: prose count drift exits 1
#   PASS: grand-total drift exits 1
#   PASS: Surface-C sections: line drift exits 1
#   PASS: Surface-D canonical table row drift exits 1
#   PASS: body decoy prose ignored; guard reads preamble only exits 0
#   Results: 7 passed, 0 failed
#
# Red Gate state (before guard script exists):
#   All 7 fixtures will show FAIL or ERROR.
#   Results: 0 passed, 7 failed
#   (This is the correct state for TDD Red Gate.)

# Repo-root guard: must be run from the directory that contains scripts/ and tests/
[ -d "scripts" ] || { echo "ERROR: Run from repo root (no scripts/ directory found here)"; exit 1; }
[ -d "tests/spec-count-fixtures" ] || { echo "ERROR: Run from repo root (no tests/spec-count-fixtures/ found)"; exit 1; }

SCRIPT="scripts/check-bc-cumulative-counts.sh"
PASS=0
FAIL=0

# Total expected fixtures — updated here whenever a fixture directory is added/removed
# (mirrors the run() call count below; kept as a single source of truth for the guard-absent message)
TOTAL=7

# Guard-absent check: fail clearly rather than with a cryptic bash error
if [ ! -f "$SCRIPT" ]; then
  echo "ERROR: guard script not found at $SCRIPT"
  echo "       This is the expected Red Gate state — the guard has not been implemented yet."
  echo "       Run the fixture tests again after scripts/check-bc-cumulative-counts.sh is created."
  echo ""
  echo "Results: 0 passed, $TOTAL failed (guard absent)"
  exit 1
fi

# run <fixture-dir> <expected-exit-code> <label>
run() {
  local fixture=$1
  local expect=$2
  local label=$3

  if [ ! -d "$fixture" ]; then
    FAIL=$((FAIL+1))
    echo "FAIL: $label (fixture directory not found: $fixture)"
    return
  fi

  # Run the guard from within the fixture directory so its .factory/ path resolves.
  # Capture both stdout+stderr and the exit code without triggering set -e.
  local output
  local exit_code
  output=$(cd "$fixture" && bash "$(pwd)/../../../$SCRIPT" 2>&1) && exit_code=0 || exit_code=$?

  if [ "$exit_code" = "$expect" ]; then
    PASS=$((PASS+1))
    echo "PASS: $label"
  else
    FAIL=$((FAIL+1))
    echo "FAIL: $label (expected exit $expect, got $exit_code)"
    # Print guard output indented for diagnosis
    echo "$output" | sed 's/^/       /'
  fi
}

run "tests/spec-count-fixtures/known-good"       0 "known-good exits 0"
run "tests/spec-count-fixtures/bc-drift-total"   1 "total_bcs drift exits 1"
run "tests/spec-count-fixtures/bc-drift-prose"   1 "prose count drift exits 1"
run "tests/spec-count-fixtures/bc-drift-grandtotal" 1 "grand-total drift exits 1"
run "tests/spec-count-fixtures/bc-drift-sections-c" 1 "Surface-C sections: line drift exits 1"
run "tests/spec-count-fixtures/bc-drift-canonical-d" 1 "Surface-D canonical table row drift exits 1"
run "tests/spec-count-fixtures/bc-drift-decoy-prose-ok" 0 "body decoy prose ignored; guard reads preamble only exits 0"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
