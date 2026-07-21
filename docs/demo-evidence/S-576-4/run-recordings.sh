#!/usr/bin/env bash
# Run all S-576-4 VHS recordings.
# Must be run from the story worktree root (.worktrees/S-576-4/).
# Requires: vhs, jr debug binary on PATH, Python 3

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKTREE_ROOT="$(cd "$SCRIPT_DIR/../../../" && pwd)"
EVIDENCE_DIR="$SCRIPT_DIR"
PORT=19879
BIN_DIR="/tmp/jr-s576-4-bin"

echo "=== S-576-4 demo recorder ==="
echo "Worktree: $WORKTREE_ROOT"
echo "Evidence dir: $EVIDENCE_DIR"
echo ""

# Build debug binary
echo "[1/4] Building debug binary..."
cd "$WORKTREE_ROOT"
cargo build 2>&1 | tail -3

mkdir -p "$BIN_DIR"
cp "$WORKTREE_ROOT/target/debug/jr" "$BIN_DIR/jr"
echo "      Binary: $BIN_DIR/jr"

# Start mock server
echo ""
echo "[2/4] Starting mock server (port $PORT)..."
python3 "$EVIDENCE_DIR/mock-server.py" &
MOCK_PID=$!
echo "      PID: $MOCK_PID"
sleep 1

cleanup() {
  echo ""
  echo "[cleanup] Stopping mock server (PID $MOCK_PID)..."
  kill "$MOCK_PID" 2>/dev/null || true
}
trap cleanup EXIT

# Check vhs
if ! command -v vhs &>/dev/null; then
  echo "ERROR: vhs not found. Install with: brew install vhs"
  exit 1
fi

echo ""
echo "[3/4] Recording tapes..."
cd "$EVIDENCE_DIR"

TAPES=(
  "AC-001-002-003-010-single-gate.tape"
  "AC-001-013-dec168-targeted-404.tape"
  "AC-004-005-bulk-failsoft.tape"
  "AC-006-007-011-issue-older-than.tape"
  "AC-008-009-dry-run.tape"
  "AC-007-016-duration-errors.tape"
  "AC-012-014-015-clap-tests.tape"
)

for tape in "${TAPES[@]}"; do
  echo "  vhs $tape ..."
  PATH="$BIN_DIR:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin" vhs "$tape"
  echo "      done"
done

echo ""
echo "[4/4] Artifacts:"
ls -lh "$EVIDENCE_DIR"/*.gif "$EVIDENCE_DIR"/*.webm 2>/dev/null | awk '{print "  " $5, $9}'

echo ""
echo "=== Done. All tapes recorded. ==="
