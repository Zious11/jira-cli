#!/usr/bin/env bash
# Demo helper: run jr issue edit with 1001 keys to trigger the cap (exit 64)
# Uses debug build (cargo run) + JR_AUTH_HEADER override to avoid keychain.

WORKTREE="/Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-110-pr1"
export JR_BASE_URL="http://127.0.0.1:9"
export JR_AUTH_HEADER="Basic dGVzdA=="

keys=()
for i in $(seq 1 1001); do
    keys+=("FOO-$i")
done

cd "$WORKTREE"
output=$(cargo run --quiet -- --no-input issue edit "${keys[@]}" --label add:foo 2>&1)
code=$?
echo "$output" | tail -3
echo "exit code: $code"
