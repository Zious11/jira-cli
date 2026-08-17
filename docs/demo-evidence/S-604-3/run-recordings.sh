#!/usr/bin/env bash
# Run all S-604-3 VHS recordings.
# Must be run from the story worktree root (.worktrees/S-604-3/).
# Requires: vhs, jr debug binary, Python 3
#
# SAFETY: this script only ever talks to the LOCAL mock server started below
# (127.0.0.1:19880). JR_BASE_URL is a debug-only seam (CLAUDE.md) that
# redirects jr's HTTP client to that local mock. No real Jira instance is
# ever contacted, and `jr component delete` (irreversible on a real
# instance) never runs against anything but the throwaway in-memory fixtures
# in mock-server.py.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKTREE_ROOT="$(cd "$SCRIPT_DIR/../../../" && pwd)"
EVIDENCE_DIR="$SCRIPT_DIR"
PORT=19880
BIN_DIR="/tmp/jr-s604-3-bin"
CFG_DIR="/tmp/jr-s604-3-cfg"
CAC_DIR="/tmp/jr-s604-3-cac"

echo "=== S-604-3 demo recorder ==="
echo "Worktree: $WORKTREE_ROOT"
echo "Evidence dir: $EVIDENCE_DIR"
echo ""

echo "[1/4] Building debug binary..."
cd "$WORKTREE_ROOT"
cargo build 2>&1 | tail -3

mkdir -p "$BIN_DIR" "$CFG_DIR/jr" "$CAC_DIR"
cp "$WORKTREE_ROOT/target/debug/jr" "$BIN_DIR/jr"
cat > "$CFG_DIR/jr/config.toml" <<EOF
default_profile = "default"
[profiles.default]
url = "http://127.0.0.1:$PORT"
auth_method = "api_token"
EOF
echo "      Binary: $BIN_DIR/jr"

echo ""
echo "[2/4] Starting mock server (port $PORT)..."
python3 "$EVIDENCE_DIR/mock-server.py" "$PORT" &
MOCK_PID=$!
echo "      PID: $MOCK_PID"
sleep 1

cleanup() {
  echo ""
  echo "[cleanup] Stopping mock server (PID $MOCK_PID)..."
  kill "$MOCK_PID" 2>/dev/null || true
}
trap cleanup EXIT

if ! command -v vhs &>/dev/null; then
  echo "ERROR: vhs not found. Install with: brew install vhs"
  exit 1
fi

echo ""
echo "[3/4] Recording tapes..."
cd "$EVIDENCE_DIR"

TAPES=(
  "AC-001-004-disposition-guard.tape"
  "AC-005-020-move-to-success.tape"
  "AC-012-013-orphan-gate.tape"
  "AC-019-snapshot-drift-fail-closed.tape"
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
