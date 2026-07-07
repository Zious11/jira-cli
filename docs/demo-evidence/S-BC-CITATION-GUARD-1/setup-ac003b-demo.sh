#!/usr/bin/env bash
# Setup helper for AC-003b demo — creates temp bc file with missing .snap citation
# Called from VHS Hidden section to avoid backtick escaping issues in tape syntax
set -euo pipefail

TMP=$(mktemp -d)
echo "$TMP" > /tmp/ac003b-tmpdir.txt

# Create bc stub with a citation to a .snap file that does not exist on disk.
# Tier-ii non-.rs tokens: file-existence check only (no symbol grep).
printf '**Trace**: `src/cli/auth/tests/snapshots/jr__demo_nonexistent.snap`\n' \
    > "$TMP/bc-snap.md"

echo "AC-003b temp dir: $TMP"
echo "bc-snap.md contents:"
cat "$TMP/bc-snap.md"
