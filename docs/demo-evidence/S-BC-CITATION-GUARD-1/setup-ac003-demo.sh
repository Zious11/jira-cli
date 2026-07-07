#!/usr/bin/env bash
# Setup helper for AC-003 demo — creates temp bc file with dead symbol citation
# Called from VHS Hidden section to avoid backtick escaping issues in tape syntax
set -euo pipefail

TMP=$(mktemp -d)
echo "$TMP" > /tmp/ac003-tmpdir.txt

# Create bc stub with a citation to a nonexistent function in a real src file.
# The backtick-quoted citation form mirrors actual bc-*.md Trace field format.
printf '**Trace**: `src/cli/issue/edit.rs::handle_edit_nonexistent_fn_demo`\n' \
    > "$TMP/bc-dead.md"

echo "AC-003 temp dir: $TMP"
echo "bc-dead.md contents:"
cat "$TMP/bc-dead.md"
