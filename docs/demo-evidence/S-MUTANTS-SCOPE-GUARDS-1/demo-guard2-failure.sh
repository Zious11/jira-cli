#!/usr/bin/env bash
# Demo script: Guard 2 failure path (CI-MUTANTS-CITE-001)
# Corrupts one fn citation in a TEMP COPY only — real policy doc is never modified.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
POLICY_DOC="${REPO_ROOT}/docs/specs/cargo-mutants-policy.md"

echo "=== AC-003 / AC-001 ERROR PATH: Guard 2 failure demo ==="
echo ""

# Create temp copy with handle_create renamed to handle_create_nonexistent
TMPFILE="$(mktemp /tmp/policy-corrupt-XXXXXX.md)"
trap 'rm -f "$TMPFILE"' EXIT

sed 's/`handle_create`/`handle_create_nonexistent`/g' "$POLICY_DOC" > "$TMPFILE"

echo "Temp policy doc: $TMPFILE"
echo "Corruption: renamed 'handle_create' -> 'handle_create_nonexistent' in §Scope"
echo ""
echo "Running: POLICY_DOC=\"<tmp>\" bash scripts/check-cargo-mutants-policy-citations.sh"
echo "---"

set +e
POLICY_DOC="$TMPFILE" bash "${REPO_ROOT}/scripts/check-cargo-mutants-policy-citations.sh"
RC=$?
set -e

echo "---"
echo "Exit code: $RC"
echo ""
echo "Verifying real policy doc is unchanged..."
git -C "$REPO_ROOT" diff --exit-code docs/specs/cargo-mutants-policy.md \
  && echo "OK: git diff clean (real policy doc unmodified)"
