#!/usr/bin/env bash
# check-cargo-mutants-policy-citations.sh — CI-MUTANTS-CITE-001
#
# Guard 2 (DEC-150): validates docs/specs/cargo-mutants-policy.md §Scope
# function-location bulleted list against src/ definitions via
# definition-anchored grep. Exits 1 if any (file, fn) pair is dead.
#
# USAGE:
#   scripts/check-cargo-mutants-policy-citations.sh                # canonical CI run
#   scripts/check-cargo-mutants-policy-citations.sh --self-test    # offline fixture run
#   scripts/check-cargo-mutants-policy-citations.sh --policy-doc <path>  # alternate doc
#
# TODO(stub): implement §Scope parsing, definition-anchored grep, and twelve
# self-test fixtures. See S-MUTANTS-SCOPE-GUARDS-1 / DEC-150 / BC-5.38.001.
# RED-gate stub form: run_check() { return 0; } — NO output.
# Under this stub every --self-test fixture fails RED:
#   rc=1-expecting probes (A, B, C, D, E, K, F-b, H N=2/N=5) fail rc assertions;
#   rc=0/content-asserting probes (F-a, G, I-a, I-b, J, L) fail on empty output.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Validate script syntax on every invocation (L-7/L-5 FIX — unconditional, top-of-file).
bash -n "${BASH_SOURCE[0]}"

# ---------------------------------------------------------------------------
# STUB: run_check — RED-gate mandated form (BC-5.38.001 / story RED-gate para)
# Returns 0 with NO output. All twelve --self-test fixtures fail RED against
# this stub. Replaced by full implementation in S-MUTANTS-SCOPE-GUARDS-1.
# ---------------------------------------------------------------------------
run_check() {
    return 0
}

# ---------------------------------------------------------------------------
# Argument parsing — minimal skeleton; full implementation in story Task 2.
# ---------------------------------------------------------------------------
self_test=0
CANONICAL_MODE=0

while [ $# -gt 0 ]; do
    case "$1" in
        --self-test)
            self_test=1
            shift
            ;;
        --policy-doc)
            POLICY_DOC="${2:?--policy-doc requires a path argument}"
            shift 2
            ;;
        --src-root)
            if [ "$self_test" = "0" ]; then
                echo "Error: --src-root is only valid with --self-test" >&2
                exit 64
            fi
            SRC_ROOT="${2:?--src-root requires a directory argument}"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

if [ "$self_test" = "0" ] && [ -z "${POLICY_DOC+x}" ]; then CANONICAL_MODE=1; fi

# ---------------------------------------------------------------------------
# Main dispatch
# ---------------------------------------------------------------------------
if [ "$self_test" = "1" ]; then
    # TODO(stub): implement twelve self-test fixtures (A–L) and four
    # post-fixture self-assertions. See S-MUTANTS-SCOPE-GUARDS-1 Task 2.
    exit 0
fi

run_check
exit $?
