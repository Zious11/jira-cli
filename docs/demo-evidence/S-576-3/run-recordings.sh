#!/usr/bin/env bash
# run-recordings.sh — Build debug binary, start mock server, record all 7 VHS tapes
# Usage: cd docs/demo-evidence/S-576-3 && bash run-recordings.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKTREE_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PORT=19878
BIN_LINK_DIR="/tmp/jr-s576-3-bin"
SRC_LINK_DIR="/tmp/jr-s576-3-src"

echo "==> Worktree: $WORKTREE_ROOT"

# 1. Build debug binary
echo "==> Building debug binary..."
cd "$WORKTREE_ROOT"
cargo build 2>&1 | tail -3

# 2. Create symlink in a clean PATH directory
mkdir -p "$BIN_LINK_DIR"
ln -sf "$WORKTREE_ROOT/target/debug/jr" "$BIN_LINK_DIR/jr"
echo "==> jr binary: $(ls -la "$BIN_LINK_DIR/jr")"

# Also expose worktree root for cargo test command
ln -sf "$WORKTREE_ROOT" "$SRC_LINK_DIR" 2>/dev/null || true

# 3. Start mock server
echo "==> Starting mock server on port $PORT..."
python3 "$SCRIPT_DIR/mock-server.py" "$PORT" &
MOCK_PID=$!
echo "    mock server PID: $MOCK_PID"
sleep 1

# Verify server is up
curl -s "http://127.0.0.1:$PORT/rest/api/3/issue/DEMO-1?fields=attachment" | \
  python3 -c "import sys,json; d=json.load(sys.stdin); print(f'  mock OK: {d[\"key\"]} has {len(d[\"fields\"][\"attachment\"])} attachments')"

# 4. Run VHS for each tape
cd "$SCRIPT_DIR"

TAPES=(
  "AC-001-003-004-005-upload-success.tape"
  "AC-006-007-replace-gate.tape"
  "AC-008-dry-run.tape"
  "AC-002-011-error-taxonomy.tape"
  "AC-006-012-delete-ordering.tape"
  "AC-014-016-017-interim-rejection.tape"
  "AC-009-010-013-test-evidence.tape"
)

PASS=0
FAIL=0

for tape in "${TAPES[@]}"; do
  echo "==> Recording: $tape"
  if vhs "$tape"; then
    echo "    OK"
    PASS=$((PASS + 1))
  else
    echo "    FAILED: $tape"
    FAIL=$((FAIL + 1))
  fi
done

# 5. Stop mock server
kill "$MOCK_PID" 2>/dev/null && echo "==> Mock server stopped."

echo ""
echo "==> Results: $PASS passed, $FAIL failed"
echo "==> Artifacts:"
ls -lh *.gif *.webm 2>/dev/null || echo "  (no artifacts yet)"
