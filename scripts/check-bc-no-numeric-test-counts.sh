#!/usr/bin/env bash
# scripts/check-bc-no-numeric-test-counts.sh
# Enforce PG-365-1 convention: BC `Trace:` and `Source:` fields MUST NOT contain numeric
# test counts. Numeric counts drift as tests are added — qualitative descriptions are stable.
# Sibling to scripts/check-spec-counts.sh.
#
# USAGE:
#   scripts/check-bc-no-numeric-test-counts.sh                  # canonical CI run
#   scripts/check-bc-no-numeric-test-counts.sh --self-test      # offline fixture run
#   scripts/check-bc-no-numeric-test-counts.sh --bc-dir <path>  # alternate BC dir
#
# Exit codes:
#   0 — no violations (or --self-test: all fixtures passed)
#   1 — one or more Trace/Source fields contain numeric test counts (or --self-test failure)
#   2 — BC directory not found or contains no bc-*.md files
#   64 — usage error (unknown argument)
#
# S-627-1: fixed the line-55(-era) PATTERN to stop false-matching digits that are part of a
# structured identifier (CWE-NNN, BC-S.SS.NNN, #NNN issue refs, vN.N.NNN version strings)
# immediately followed by "…tests" within the word-gap window. Added --self-test/--bc-dir
# seam, copied from scripts/check-bc-citation-symbols.sh.

set -euo pipefail

BC_DIR_DEFAULT=".factory/specs/prd"

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
#   (^|[^[:alnum:].#-])           — left boundary: the digit run must sit at the
#                                    start of the (already-`grep -n`-prefixed) line,
#                                    or be preceded by a character that is NOT
#                                    alphanumeric, '.', '#', or '-'. This is what
#                                    excludes digits that are part of a structured
#                                    identifier: CWE-116 ('-' before '116'),
#                                    BC-3.9.001 ('.' before '001'), #576 ('#' before
#                                    '576'), v1.3.94 ('.'/alnum before each digit
#                                    group) all fail this boundary at every possible
#                                    starting offset of their digit run(s), so the
#                                    match never even begins there (S-627-1, AC-1/2).
#   [0-9]+                        — a bare integer
#   [[:space:]]+                  — whitespace separator
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

PATTERN='(^|[^[:alnum:].#-])[0-9]+[[:space:]]+([[:alnum:]_-]+[[:space:]]+){0,3}tests?([^[:alnum:]]|$)'

# ---------------------------------------------------------------------------
# run_check — scans a BC directory for numeric-test-count violations.
# Returns 0 (clean), 1 (violations found), or 2 (bc_dir missing / empty).
# Effectful (filesystem + stdout/stderr); the PATTERN match itself is pure.
# ---------------------------------------------------------------------------
run_check() {
  local bc_dir="$1"

  if [ ! -d "$bc_dir" ]; then
    echo "ERROR: BC directory not found: $bc_dir" >&2
    return 2
  fi

  # Explicitly verify at least one bc-*.md file exists so the guard cannot
  # pass silently when the glob fails to expand (e.g. misconfigured worktree).
  local bc_files
  bc_files=("$bc_dir"/bc-*.md)
  if [ ! -f "${bc_files[0]}" ]; then
    echo "ERROR: no bc-*.md files found in $bc_dir — nothing to scan" >&2
    return 2
  fi

  # Scan using the pre-validated bc_files array (no glob ambiguity).
  # 2>/dev/null is intentionally omitted: I/O errors (unreadable files) must
  # surface as grep exit 2, not be silently absorbed. The compound `|| { ... }`
  # treats grep exit 1 (no matches = clean) as success but re-raises exit 2+
  # so an I/O error causes the check to fail visibly.
  local violations rc
  violations=$(grep -nE '^\*\*(Trace|Source)\*\*:' "${bc_files[@]}" \
    | grep -E "$PATTERN" \
    || { rc=$?; [ "$rc" -eq 1 ] || return "$rc"; })

  if [ -n "$violations" ]; then
    echo "ERROR: BC Trace/Source fields must not contain numeric test counts (PG-365-1 convention)." >&2
    echo "Numeric counts drift as tests are added; use qualitative descriptions instead." >&2
    echo "" >&2
    echo "Offending lines:" >&2
    echo "$violations" >&2
    echo "" >&2
    echo "Fix: replace e.g. '(16 wiremock tests — 15 library tokio + 1 subprocess)'" >&2
    echo "     with '(wiremock suite: library tokio + subprocess)' or similar." >&2
    return 1
  fi

  echo "OK: no numeric test counts in BC Trace/Source fields."
  return 0
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
self_test=0
bc_dir="$BC_DIR_DEFAULT"

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test)
      self_test=1
      shift
      ;;
    --bc-dir)
      bc_dir="${2:?--bc-dir requires a path argument}"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 64
      ;;
  esac
done

# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------
if [ "$self_test" = "1" ]; then

  # Fixture counter and integrity pin (mirrors check-bc-citation-symbols.sh's
  # EXPECTED_FIXTURES pattern — prevents a fixture from being silently deleted
  # while the summary line still claims a passing run).
  readonly EXPECTED_FIXTURES=21
  fixtures_run=0

  tmp_dir=""
  trap 'rm -rf "${tmp_dir:-}"' EXIT

  # run_one_fixture <label> <expect_rc> <fixture_line>
  # Writes a single bc-mock.md containing the fixture line under a **Trace**:
  # field, runs run_check against it in isolation, and asserts the exit code.
  run_one_fixture() {
    local label="$1" expect_rc="$2" line="$3"
    tmp_dir=$(mktemp -d)
    printf '**Trace**: %s\n' "$line" > "$tmp_dir/bc-mock.md"
    local rc=0
    run_check "$tmp_dir" >/dev/null 2>&1 || rc=$?
    if [ "$rc" != "$expect_rc" ]; then
      echo "SELF-TEST FAIL [$label]: expected rc=$expect_rc, got rc=$rc for line: $line" >&2
      exit 1
    fi
    rm -rf "$tmp_dir"
    tmp_dir=""
    fixtures_run=$((fixtures_run + 1))
  }

  # -------------------------------------------------------------------
  # TRUE-POSITIVE fixtures (must trigger — expect rc=1)
  # -------------------------------------------------------------------
  run_one_fixture "POS-1"  1 "16 wiremock tests"
  run_one_fixture "POS-2"  1 "3 tests"
  run_one_fixture "POS-3"  1 "22 test functions"
  run_one_fixture "POS-4"  1 "12 tests"
  run_one_fixture "POS-5"  1 "3 wiremock tests"
  run_one_fixture "POS-6"  1 "added 5 integration tests"
  run_one_fixture "POS-7"  1 "69 unit tests"
  run_one_fixture "POS-8"  1 "1 subprocess test"
  run_one_fixture "POS-9"  1 "(16 wiremock tests — 15 library tokio + 1 subprocess)"

  # -------------------------------------------------------------------
  # TRUE-NEGATIVE fixtures (must NOT trigger — expect rc=0)
  # -------------------------------------------------------------------
  run_one_fixture "NEG-1"  0 "CWE-116"
  run_one_fixture "NEG-2"  0 "CWE-22"
  run_one_fixture "NEG-3"  0 "BC-3.8.012"
  run_one_fixture "NEG-4"  0 "BC-3.9.001"
  run_one_fixture "NEG-5"  0 "#639"
  run_one_fixture "NEG-6"  0 "#576"
  run_one_fixture "NEG-7"  0 "v1.3.94"
  run_one_fixture "NEG-8"  0 "v0.7.0"
  run_one_fixture "NEG-9"  0 "PR-001"
  # Exact real-world false-positive forms from bc-3-issue-write.md (S-627-1
  # Problem Statement) — the un-hyphenated forms Phase 2 will restore. Under
  # the OLD PATTERN, "93" (from CWE-93) and "352" (from CWE-352) were the
  # digit runs that spuriously matched via the 2-word gap allowance.
  run_one_fixture "NEG-10" 0 "SEC-576-004 (CWE-93 multipart encoding test added 2026-07-15)"
  run_one_fixture "NEG-11" 0 "SEC-576-005 (CWE-352 X-Atlassian-Token wiremock test added 2026-07-15)"
  run_one_fixture "NEG-12" 0 "SEC-576-003 (CWE-522 credential-stripping wiremock test requirement added 2026-07-15)"

  # Fixture-count integrity pin (string equality; prevents silent fixture omission).
  [ "$fixtures_run" = "$EXPECTED_FIXTURES" ] \
    || { echo "SELF-TEST-FIXTURE-COUNT: expected ${EXPECTED_FIXTURES} fixtures, got ${fixtures_run}" >&2; exit 1; }

  echo "All self-test fixtures passed (${fixtures_run}/${EXPECTED_FIXTURES})"
  exit 0
else
  set +e
  run_check "$bc_dir"
  rc=$?
  set -e
  exit "$rc"
fi
