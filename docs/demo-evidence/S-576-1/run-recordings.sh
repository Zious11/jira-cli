#!/usr/bin/env bash
# run-recordings.sh — produce all S-576-1 demo recordings
# Run from anywhere; uses absolute paths throughout.
set -euo pipefail

WORKTREE="/Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-576-1"
EVID_DIR="$WORKTREE/docs/demo-evidence/S-576-1"
JR_BIN="$WORKTREE/target/debug/jr"
MOCK_PORT=19876

export PATH="/opt/homebrew/bin:$PATH"

echo "=== S-576-1 Demo Recorder ==="

# 1. Create the jr wrapper script
echo "[1/5] Creating jr wrapper at /tmp/jr-s576-bin/jr..."
mkdir -p /tmp/jr-s576-bin /tmp/jr-s576-demo-cfg /tmp/jr-s576-demo-cac
cat > /tmp/jr-s576-bin/jr << WRAPPER
#!/usr/bin/env bash
exec "$JR_BIN" "\$@"
WRAPPER
chmod +x /tmp/jr-s576-bin/jr

# 2. Create a symlink so AC-010/011 tapes can cd to the worktree
echo "[2/5] Linking worktree as /tmp/jr-s576-worktree..."
rm -f /tmp/jr-s576-worktree
ln -sf "$WORKTREE" /tmp/jr-s576-worktree

# 3. Start mock server
echo "[3/5] Starting mock server on port $MOCK_PORT..."
lsof -ti:$MOCK_PORT | xargs kill -9 2>/dev/null || true
sleep 0.3
python3 "$EVID_DIR/mock-server.py" "$MOCK_PORT" &
MOCK_PID=$!
trap "kill $MOCK_PID 2>/dev/null || true; echo 'Mock server stopped.'" EXIT
sleep 0.5

curl -sf "http://127.0.0.1:$MOCK_PORT/rest/api/3/issue/DEMO-1?fields=attachment" >/dev/null \
  || { echo "ERROR: mock server did not start"; exit 1; }
echo "    Mock server up (PID $MOCK_PID)"

# 4. Record all tapes from the evidence directory
echo "[4/5] Recording tapes..."
cd "$EVID_DIR"

TAPES=(
  "AC-001-002-table-zero-attachments.tape"
  "AC-003-004-json-filter-hint.tape"
  "AC-005-006-007-filters.tape"
  "AC-008-invalid-filter.tape"
  "AC-009-error-taxonomy.tape"
  "AC-010-surface-guard.tape"
  "AC-011-docs-obligations.tape"
)

for tape in "${TAPES[@]}"; do
  echo "  vhs $tape"
  vhs "$tape"
done

# 5. Verify
echo "[5/5] Verifying output files..."
EXPECTED=(
  "AC-001-002-table-zero-attachments.gif"
  "AC-001-002-table-zero-attachments.webm"
  "AC-003-004-json-filter-hint.gif"
  "AC-003-004-json-filter-hint.webm"
  "AC-005-006-007-filters.gif"
  "AC-005-006-007-filters.webm"
  "AC-008-invalid-filter.gif"
  "AC-008-invalid-filter.webm"
  "AC-009-error-taxonomy.gif"
  "AC-009-error-taxonomy.webm"
  "AC-010-surface-guard.gif"
  "AC-010-surface-guard.webm"
  "AC-011-docs-obligations.gif"
  "AC-011-docs-obligations.webm"
)

all_ok=true
for f in "${EXPECTED[@]}"; do
  if [[ -f "$EVID_DIR/$f" && -s "$EVID_DIR/$f" ]]; then
    size=$(du -h "$EVID_DIR/$f" | cut -f1)
    echo "  OK  $f ($size)"
  else
    echo "  MISSING: $f"
    all_ok=false
  fi
done

if $all_ok; then
  echo ""
  echo "All recordings produced successfully."
else
  echo ""
  echo "WARNING: Some recordings missing."
  exit 1
fi
