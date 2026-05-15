#!/usr/bin/env bash
# scripts/check-bc-no-numeric-test-counts.sh
# Enforce PG-365-1 convention: BC `Trace:` and `Source:` fields MUST NOT contain numeric
# test counts. Numeric counts drift as tests are added — qualitative descriptions are stable.
# Sibling to scripts/check-spec-counts.sh.
#
# Exit codes:
#   0 — no violations
#   1 — one or more Trace/Source fields contain numeric test counts
#   2 — BC directory not found or contains no bc-*.md files

set -euo pipefail

BC_DIR=".factory/specs/prd"

if [ ! -d "$BC_DIR" ]; then
  echo "ERROR: BC directory not found: $BC_DIR" >&2
  exit 2
fi

# Explicitly verify at least one bc-*.md file exists so the guard cannot
# pass silently when the glob fails to expand (e.g. misconfigured worktree).
bc_files=("$BC_DIR"/bc-*.md)
if [ ! -f "${bc_files[0]}" ]; then
  echo "ERROR: no bc-*.md files found in $BC_DIR — nothing to scan" >&2
  exit 2
fi

# Match patterns like:
#   "16 wiremock tests"
#   "15 library tests"
#   "1 subprocess test"
#   "3 tests"
#   "69 unit tests"
#   "4 new dedupe tests"
# in lines starting with "**Trace**:" or "**Source**:" (both BC traceability fields).
# Be specific to these field markers to avoid false positives from spec body prose
# that may mention test counts in passing (e.g., "the existing 26 unit tests
# for this BC are in tests/foo.rs").
#
# Pattern breakdown (POSIX ERE — portable across GNU grep and BSD grep):
#   [0-9]+                         — a bare integer
#   [[:space:]]+                   — whitespace separator
#   ([[:alnum:]_-]+[[:space:]]+){0,3}  — up to 3 optional adjective words;
#                                      [[:alnum:]_-] includes hyphen so patterns
#                                      like "end-to-end" or "wiremock-based" match
#   tests?                         — "test" or "tests"
#   ([^[:alnum:]]|$)               — not followed by an alphanumeric (avoids
#                                    matching "tester", "testing")
#
# Note: \b, \w, \s are PCRE/GNU extensions not available in POSIX ERE or
# BSD grep (macOS). Use [[:space:]], [[:alnum:]], and bracket-expression
# boundaries for portability.

PATTERN='[0-9]+[[:space:]]+([[:alnum:]_-]+[[:space:]]+){0,3}tests?([^[:alnum:]]|$)'

# Scan using the pre-validated bc_files array (no glob ambiguity).
# 2>/dev/null is intentionally omitted: I/O errors (unreadable files) must
# surface as grep exit 2, not be silently absorbed. The compound `|| { ... }`
# treats grep exit 1 (no matches = clean) as success but re-raises exit 2+
# so an I/O error causes the script to fail visibly.
violations=$(grep -nE '^\*\*(Trace|Source)\*\*:' "${bc_files[@]}" \
  | grep -E "$PATTERN" \
  || { rc=$?; [ "$rc" -eq 1 ] || exit "$rc"; })

if [ -n "$violations" ]; then
  echo "ERROR: BC Trace/Source fields must not contain numeric test counts (PG-365-1 convention)." >&2
  echo "Numeric counts drift as tests are added; use qualitative descriptions instead." >&2
  echo "" >&2
  echo "Offending lines:" >&2
  echo "$violations" >&2
  echo "" >&2
  echo "Fix: replace e.g. '(16 wiremock tests — 15 library tokio + 1 subprocess)'" >&2
  echo "     with '(wiremock suite: library tokio + subprocess)' or similar." >&2
  exit 1
fi

echo "OK: no numeric test counts in BC Trace/Source fields."
