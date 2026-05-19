#!/usr/bin/env bash
set -euo pipefail

# check-bc-cumulative-counts.sh — DRIFT-002 mitigation
#
# Validates that cumulative total_bcs count claims agree across 7 surfaces
# (plus body-preamble prose) for each bounded context file:
#
#   Surface A: bc-N.md frontmatter total_bcs:
#   Surface B: BC-INDEX.md ## Section N: header cumulative count
#   Surface C: BC-INDEX.md frontmatter sections: line for each file
#   Surface D: CANONICAL-COUNTS.md per-file total_bcs table row
#   Body prose: "N behavioral contracts" preamble line in each bc-N.md
#
# Grand-total surfaces (must equal sum of per-file Surface A values):
#   Surface E: BC-INDEX.md frontmatter total_bcs:
#   Surface F: CANONICAL-COUNTS.md **Sum** row
#   Surface G: CANONICAL-COUNTS.md grand-total prose line
#
# Exit codes:
#   0 — all counts agree
#   1 — one or more mismatches detected (details printed to stdout)

# Repo-root guard
[ -d ".factory" ] || { echo "ERROR: Run from repo root (no .factory/ directory found here)"; exit 1; }

FACTORY=".factory/specs/prd"
BC_INDEX="$FACTORY/BC-INDEX.md"
CANONICAL="$FACTORY/CANONICAL-COUNTS.md"
ERRORS=0
FILE_COUNT=0
TOTAL_SUM=0

# ── Per-file surface checks ──────────────────────────────────────────────────

for f in "$FACTORY"/bc-*.md "$FACTORY"/cross-cutting.md; do
  [ -f "$f" ] || continue
  basename_f=$(basename "$f")
  FILE_COUNT=$((FILE_COUNT+1))

  # Surface A: frontmatter total_bcs:
  surface_a=$(grep '^total_bcs:' "$f" | awk '{print $2}')
  if [ -z "$surface_a" ]; then
    echo "ERROR: $basename_f: total_bcs frontmatter not found"
    ERRORS=$((ERRORS+1))
    continue
  fi
  TOTAL_SUM=$((TOTAL_SUM + surface_a))

  # Surface B: BC-INDEX.md ## Section N: header cumulative count
  # Match section headers that reference this file by filename
  surface_b=$(grep "^## Section.*($basename_f)" "$BC_INDEX" \
    | sed 's/.*— \([0-9]*\) BCs cumulative.*/\1/' || true)
  if [ -z "$surface_b" ]; then
    echo "ERROR: $basename_f: no ## Section header found in BC-INDEX.md for this file"
    ERRORS=$((ERRORS+1))
  elif [ "$surface_b" != "$surface_a" ]; then
    echo "ERROR: $basename_f: total_bcs frontmatter=$surface_a but BC-INDEX.md Section header=$surface_b"
    ERRORS=$((ERRORS+1))
  fi

  # Surface C: BC-INDEX.md frontmatter sections: line for this file
  # The frontmatter sections: lines have format "  - FILENAME (N BCs cumulative; ...)"
  # Use the leading "  - " to avoid matching table rows that also reference the filename.
  surface_c=$(grep "^  - $basename_f " "$BC_INDEX" \
    | sed 's/.* (\([0-9]*\) BCs cumulative.*/\1/' || true)
  if [ -z "$surface_c" ]; then
    echo "ERROR: $basename_f: no sections: entry found in BC-INDEX.md frontmatter for this file"
    ERRORS=$((ERRORS+1))
  elif [ "$surface_c" != "$surface_a" ]; then
    echo "ERROR: $basename_f: total_bcs frontmatter=$surface_a but BC-INDEX.md sections: line=$surface_c"
    ERRORS=$((ERRORS+1))
  fi

  # Surface D: CANONICAL-COUNTS.md per-file total_bcs table row
  # Extract only the "Per-file total_bcs" section (skip L2 alignment table)
  surface_d=$(awk '/^### Per-file total_bcs/,/^### Grand total/' "$CANONICAL" \
    | grep "^| $basename_f |" \
    | awk -F'|' '{gsub(/ /,"",$3); print $3}' || true)
  if [ -z "$surface_d" ]; then
    echo "ERROR: $basename_f: no row found in CANONICAL-COUNTS.md per-file total_bcs table"
    ERRORS=$((ERRORS+1))
  elif [ "$surface_d" != "$surface_a" ]; then
    echo "ERROR: $basename_f: total_bcs frontmatter=$surface_a but CANONICAL-COUNTS.md table row=$surface_d"
    ERRORS=$((ERRORS+1))
  fi

  # Body-preamble prose: "N behavioral contracts" line in the bc-N.md body
  prose_count=$(grep -m1 'behavioral contracts' "$f" | sed 's/^\([0-9]*\) behavioral.*/\1/' || true)
  if [ -z "$prose_count" ]; then
    echo "ERROR: $basename_f: no \"N behavioral contracts\" preamble line found in body"
    ERRORS=$((ERRORS+1))
  elif [ "$prose_count" != "$surface_a" ]; then
    echo "ERROR: $basename_f: total_bcs frontmatter=$surface_a but body prose=\"$prose_count behavioral contracts\""
    ERRORS=$((ERRORS+1))
  fi

done

# ── Grand-total surface checks ───────────────────────────────────────────────

# Surface E: BC-INDEX.md frontmatter total_bcs:
surface_e=$(grep '^total_bcs:' "$BC_INDEX" | awk '{print $2}')
if [ -z "$surface_e" ]; then
  echo "ERROR: BC-INDEX.md: total_bcs frontmatter not found"
  ERRORS=$((ERRORS+1))
elif [ "$surface_e" != "$TOTAL_SUM" ]; then
  echo "ERROR: BC-INDEX.md frontmatter total_bcs=$surface_e but computed sum of per-file total_bcs=$TOTAL_SUM"
  ERRORS=$((ERRORS+1))
fi

# Surface F: CANONICAL-COUNTS.md **Sum** row
surface_f=$(grep '^| \*\*Sum\*\*' "$CANONICAL" | sed 's/.*\*\*\([0-9]*\)\*\*.*/\1/' || true)
if [ -z "$surface_f" ]; then
  echo "ERROR: CANONICAL-COUNTS.md: **Sum** row not found"
  ERRORS=$((ERRORS+1))
elif [ "$surface_f" != "$TOTAL_SUM" ]; then
  echo "ERROR: CANONICAL-COUNTS.md **Sum** row=$surface_f but computed sum of per-file total_bcs=$TOTAL_SUM"
  ERRORS=$((ERRORS+1))
fi

# Surface G: CANONICAL-COUNTS.md grand-total prose line
surface_g=$(grep '^\*\*Canonical grand total:' "$CANONICAL" | sed 's/.*: \([0-9]*\)\*\*.*/\1/' || true)
if [ -z "$surface_g" ]; then
  echo "ERROR: CANONICAL-COUNTS.md: **Canonical grand total:** prose line not found"
  ERRORS=$((ERRORS+1))
elif [ "$surface_g" != "$TOTAL_SUM" ]; then
  echo "ERROR: CANONICAL-COUNTS.md grand-total prose=$surface_g but computed sum of per-file total_bcs=$TOTAL_SUM"
  ERRORS=$((ERRORS+1))
fi

# ── Summary ──────────────────────────────────────────────────────────────────

if [ "$ERRORS" -gt 0 ]; then
  echo "FAIL: $ERRORS spec cumulative count mismatch(es). Fix frontmatter, BC-INDEX.md, or"
  echo "      CANONICAL-COUNTS.md before merging."
  exit 1
fi

echo "OK: all cumulative BC counts verified ($TOTAL_SUM total across $FILE_COUNT files)."
