#!/usr/bin/env bash
# check-bc-citation-symbols.sh — BC-CITE-001
#
# Guard 1 (DEC-148): validates src/ file and symbol citations in **Trace**: /
# **Source**: fields of bc-*.md bodies against the develop src/ tree via
# definition-anchored grep. Exits 1 if any citation is stale.
#
# USAGE:
#   scripts/check-bc-citation-symbols.sh                  # canonical CI run
#   scripts/check-bc-citation-symbols.sh --self-test      # offline fixture run
#   scripts/check-bc-citation-symbols.sh --bc-dir <path>  # alternate BC dir
#   scripts/check-bc-citation-symbols.sh --src-root <dir> --self-test  # self-test only
#
# STUB (S-BC-CITATION-GUARD-1 RED-gate): run_check() emits no output and
# returns 0. All --self-test fixtures will be RED until implemented by
# the test-writer (Task 3). No-output stub mandates all ten fixtures (A–K)
# to be RED before implementation begins (BC-5.38.001).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Validate script syntax on every invocation (unconditional, top-of-file).
bash -n "${BASH_SOURCE[0]}"

# ---------------------------------------------------------------------------
# Script-scope variables (BC-X.13.004 invariant, F-B1-01):
#   FLOOR MUST be script-scope, NOT local inside run_check — single
#   recalibration touchpoint; Fixture G assertion resolves the SAME FLOOR
#   the guard comparison uses, so a mutation weakening only the comparison
#   value is still caught.
#   CANONICAL_MODE MUST ALSO be script-scope — Fixture G toggle mechanism
#   (CANONICAL_MODE=1 set in shell scope before invoking run_check) requires
#   this; a local CANONICAL_MODE would make the toggle a no-op.
# ---------------------------------------------------------------------------
FLOOR=248       # floor(0.75 × N); N ≈ 331 (post-Task-0-hygiene census; F-B3-03).
                # Pre-hygiene DEC-154 census: N=326, FLOOR=244.
                # Script-scope (NOT local) — single recalibration touchpoint.
                # Implementer MUST run canonical mode on develop HEAD, record N,
                # set FLOOR=floor(0.75*N).
CANONICAL_MODE=0

# ---------------------------------------------------------------------------
# run_check — STUB (BC-5.38.001 Red Gate; S-BC-CITATION-GUARD-1)
#
# No-output stub: returns 0 immediately, emits no output.
# RED-gate accounting: under this stub ALL ten fixtures (A–K) fail RED:
#   - Fixtures A, B, C, D, G, J (rc=1-expecting): fail [ "$rc" -eq 1 ].
#   - Fixtures E, F, I, K (rc=0 + content-asserting): pass rc check but
#     fail their content assertion (empty output has no "citations checked"
#     / "^Check passed:" match).
#   - Fixture G CANONICAL_MODE probe: stub rc=0, assertion expects rc=1 → RED.
# An output-emitting stub is NOT sanctioned: it could incidentally satisfy
# Fixture F's content assertion while leaving others RED, corrupting the
# RED-gate observation.
# ---------------------------------------------------------------------------
run_check() {
    return 0
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
self_test=0
# CANONICAL_MODE initialized above at script scope; re-set here to satisfy
# the prior-art pattern (check-cargo-mutants-policy-citations.sh:202-203).
CANONICAL_MODE=0

while [ $# -gt 0 ]; do
    case "$1" in
        --self-test)
            self_test=1
            shift
            ;;
        --bc-dir)
            BC_DIR="${2:?--bc-dir requires a path argument}"
            shift 2
            ;;
        --src-root)
            SRC_ROOT="${2:?--src-root requires a path argument}"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 64
            ;;
    esac
done

# --src-root is only valid with --self-test (prevents accidental redirect of
# a real guard run to a temp directory; F-B2-09 usage-error message pin).
if [ -n "${SRC_ROOT+x}" ] && [ "$self_test" = "0" ]; then
    echo "Error: --src-root is only valid with --self-test" >&2
    exit 64
fi

# CANONICAL_MODE: active when no --self-test and no --bc-dir CLI flag.
# (Note: standalone --bc-dir without a BC_DIR env var leaves CANONICAL_MODE=1
# because the check tests the env var, not the CLI flag — documented behavior.)
if [ "$self_test" = "0" ] && [ -z "${BC_DIR+x}" ]; then CANONICAL_MODE=1; fi

# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------
if [ "$self_test" = "1" ]; then
    # --self-test fixture suite is not yet implemented (test-writer's job,
    # Task 3). Exits 0 trivially — no fixtures exist yet.
    exit 0
else
    run_check
fi
