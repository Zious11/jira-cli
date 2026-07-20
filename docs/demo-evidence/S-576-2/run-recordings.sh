#!/usr/bin/env bash
# run-recordings.sh — produce all S-576-2 demo recordings
# Run from anywhere; uses absolute paths throughout.
set -euo pipefail

WORKTREE="/Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-576-2"
EVID_DIR="$WORKTREE/docs/demo-evidence/S-576-2"
JR_BIN="$WORKTREE/target/debug/jr"
MOCK_PORT=19877

export PATH="/opt/homebrew/bin:$PATH"

echo "=== S-576-2 Demo Recorder ==="

# 1. Create the jr wrapper script
echo "[1/5] Creating jr wrapper at /tmp/jr-s576-2-bin/jr..."
mkdir -p /tmp/jr-s576-2-bin /tmp/jr-s576-2-cfg /tmp/jr-s576-2-cac
cat > /tmp/jr-s576-2-bin/jr << WRAPPER
#!/usr/bin/env bash
exec "$JR_BIN" "\$@"
WRAPPER
chmod +x /tmp/jr-s576-2-bin/jr

# 2. Kill any prior mock server on the port
echo "[2/5] Checking for prior mock server on port $MOCK_PORT..."
lsof -ti:$MOCK_PORT | xargs kill -9 2>/dev/null || true
sleep 0.3

# 3. Start mock server
echo "[3/5] Starting mock server on port $MOCK_PORT..."
python3 "$EVID_DIR/mock-server.py" "$MOCK_PORT" &
MOCK_PID=$!
trap "kill $MOCK_PID 2>/dev/null || true; echo 'Mock server stopped.'" EXIT
sleep 0.5

curl -sf "http://127.0.0.1:$MOCK_PORT/rest/api/3/issue/DEMO-10?fields=attachment" >/dev/null \
  || { echo "ERROR: mock server did not start"; exit 1; }
echo "    Mock server up (PID $MOCK_PID)"

# 4. Record all tapes from the evidence directory
echo "[4/5] Recording tapes..."
cd "$EVID_DIR"

TAPES=(
  "AC-001-002-018-single-download.tape"
  "AC-004-005-006-010-batch-all.tape"
  "AC-007-008-019-newest-filter.tape"
  "AC-011-012-fail-soft.tape"
  "AC-014-015-016-cwe22-sanitization.tape"
  "AC-003-009-013-error-taxonomy.tape"
  "AC-017-019-structural-and-tests.tape"
)

for tape in "${TAPES[@]}"; do
  echo "  vhs $tape"
  vhs "$tape"
done

# 5. Verify
echo "[5/5] Verifying output files..."
EXPECTED=(
  "AC-001-002-018-single-download.gif"
  "AC-001-002-018-single-download.webm"
  "AC-004-005-006-010-batch-all.gif"
  "AC-004-005-006-010-batch-all.webm"
  "AC-007-008-019-newest-filter.gif"
  "AC-007-008-019-newest-filter.webm"
  "AC-011-012-fail-soft.gif"
  "AC-011-012-fail-soft.webm"
  "AC-014-015-016-cwe22-sanitization.gif"
  "AC-014-015-016-cwe22-sanitization.webm"
  "AC-003-009-013-error-taxonomy.gif"
  "AC-003-009-013-error-taxonomy.webm"
  "AC-017-019-structural-and-tests.gif"
  "AC-017-019-structural-and-tests.webm"
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
