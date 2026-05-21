#!/usr/bin/env bash
# run-demo.sh <scenario> <port> <jr_args...>
# Starts mock server, runs jr, kills server.
set -e

SCENARIO=$1; shift
PORT=$1; shift
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKTREE_ROOT="/Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-388"
JR="$WORKTREE_ROOT/target/debug/jr"

python3 "$SCRIPT_DIR/mock-server.py" "$SCENARIO" "$PORT" &
SERVER_PID=$!
# give server a moment to bind
sleep 0.8

export JR_BASE_URL="http://127.0.0.1:$PORT"
export JR_AUTH_HEADER="Basic dGVzdDp0ZXN0"

"$JR" "$@" 2>&1 || true

kill "$SERVER_PID" 2>/dev/null || true
wait "$SERVER_PID" 2>/dev/null || true
