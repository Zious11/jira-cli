#!/usr/bin/env bash
set -euo pipefail

# check-spec-counts.sh — DRIFT-001 mitigation
#
# Verifies that numeric count claims in spec frontmatter match the actual
# counts in each file's body. Run from the repo root before merging any
# edit to .factory/specs/prd/ BC files, nfr-catalog.md, or holdout-scenarios.md.
#
# Exit codes:
#   0 — all counts match
#   1 — one or more mismatches detected (details printed to stdout)

# Repo-root guard
[ -d ".factory" ] || { echo "ERROR: Run from repo root (no .factory/ directory found here)"; exit 1; }

FACTORY=".factory/specs/prd"
ERRORS=0

# Check each bc-N-*.md file
for f in "$FACTORY"/bc-*.md; do
  [ -f "$f" ] || continue
  actual=$(grep -c '^#### BC-' "$f" || true)
  declared=$(grep '^definitional_count:' "$f" | awk '{print $2}')
  if [ "$actual" != "$declared" ]; then
    echo "ERROR: $f: actual #### BC- count=$actual, frontmatter definitional_count=$declared"
    ERRORS=$((ERRORS+1))
  fi
done

# Check nfr-catalog.md
nfr_file="$FACTORY/nfr-catalog.md"
if [ -f "$nfr_file" ]; then
  nfr_actual=$(grep -c '^| \*\*NFR-' "$nfr_file" || true)
  nfr_declared=$(grep '^total_nfrs:' "$nfr_file" | awk '{print $2}')
  if [ "$nfr_actual" != "$nfr_declared" ]; then
    echo "ERROR: nfr-catalog.md: actual NFR count=$nfr_actual, frontmatter total_nfrs=$nfr_declared"
    ERRORS=$((ERRORS+1))
  fi
else
  echo "WARNING: $nfr_file not found — skipping NFR count check"
fi

# Check holdout-scenarios.md
h_file="$FACTORY/holdout-scenarios.md"
if [ -f "$h_file" ]; then
  h_actual=$(grep -c '^### H-' "$h_file" || true)
  h_declared=$(grep '^total_holdouts:' "$h_file" | awk '{print $2}')
  if [ "$h_actual" != "$h_declared" ]; then
    echo "ERROR: holdout-scenarios.md: actual H- count=$h_actual, frontmatter total_holdouts=$h_declared"
    ERRORS=$((ERRORS+1))
  fi
else
  echo "WARNING: $h_file not found — skipping holdout count check"
fi

if [ "$ERRORS" -gt 0 ]; then
  echo "FAIL: $ERRORS spec count mismatch(es). Fix frontmatter or body before merging."
  exit 1
fi
echo "OK: all spec counts verified."
