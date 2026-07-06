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

    # -----------------------------------------------------------------------
    # CANONICAL_MODE hygiene (F-B2-06): unset before any fixture.
    # Invariant: CANONICAL_MODE MUST NOT be set during Fixtures A–F/I–K.
    # Fixture G sets CANONICAL_MODE=1 inline and unsets it after all G assertions.
    # -----------------------------------------------------------------------
    unset CANONICAL_MODE

    # Preamble check: BC-CITE-001 literal pinned in script header comment.
    grep -Eq '^#.*BC-CITE-001' "${BASH_SOURCE[0]}" \
        || { echo "SELF-TEST FAIL: citation-guard error-code literal missing from script header comment"; exit 1; }

    # Fixture counter and integrity pin.
    readonly EXPECTED_FIXTURES=10
    fixtures_run=0

    # Register cleanup trap for all fixture dirs before creating any tmpdir.
    trap 'rm -rf "${tmp_A:-}" "${tmp_B:-}" "${tmp_C:-}" "${tmp_D:-}" "${tmp_E:-}" "${tmp_F:-}" "${tmp_F_neg:-}" "${tmp_G:-}" "${tmp_G2:-}" "${tmp_I:-}" "${tmp_J:-}" "${tmp_K:-}"' EXIT

    # -------------------------------------------------------------------
    # Fixture A — dead-symbol: file exists, fn NOT defined
    # -------------------------------------------------------------------
    tmp_A=$(mktemp -d)
    mkdir -p "$tmp_A/src"
    printf '**Trace**: `src/adf.rs::nonexistent_fn_selftest`\n' > "$tmp_A/bc-mock.md"
    touch "$tmp_A/src/adf.rs"   # file exists; symbol NOT in it
    set +e; BC_DIR="$tmp_A" SRC_ROOT="$tmp_A" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture A FAIL: expected rc=1, got $rc"; exit 1; }
    grep -qF 'DEAD: nonexistent_fn_selftest not found in src/adf.rs' <<<"$output" \
        || { echo "Fixture A FAIL: expected 'DEAD: nonexistent_fn_selftest not found in src/adf.rs' in output"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture B — dead-file: file NOT created
    # -------------------------------------------------------------------
    tmp_B=$(mktemp -d)
    mkdir -p "$tmp_B/src"
    printf '**Source**: `src/nonexistent_file_selftest.rs::some_fn`\n' > "$tmp_B/bc-mock.md"
    # $tmp_B/src/nonexistent_file_selftest.rs intentionally NOT created
    set +e; BC_DIR="$tmp_B" SRC_ROOT="$tmp_B" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture B FAIL: expected rc=1, got $rc"; exit 1; }
    grep -qF 'DEAD: src/nonexistent_file_selftest.rs not found' <<<"$output" \
        || { echo "Fixture B FAIL: expected 'DEAD: src/nonexistent_file_selftest.rs not found' in output"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture C — import-only false-green: fn appears only in use statement
    # -------------------------------------------------------------------
    tmp_C=$(mktemp -d)
    mkdir -p "$tmp_C/src/cli/issue"
    printf '**Trace**: `src/cli/issue/create.rs::handle_jsm_create`\n' > "$tmp_C/bc-mock.md"
    printf 'use super::jsm_create::{JsmCreateArgs, handle_jsm_create};\n' \
        > "$tmp_C/src/cli/issue/create.rs"
    set +e; BC_DIR="$tmp_C" SRC_ROOT="$tmp_C" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture C FAIL: expected rc=1, got $rc (import-only must be DEAD)"; exit 1; }
    grep -qF 'DEAD: ' <<<"$output" \
        || { echo "Fixture C FAIL: DEAD: prefix missing from output (import-only should be DEAD)"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture D — Source-field extraction: dead citation on **Source** line
    # -------------------------------------------------------------------
    tmp_D=$(mktemp -d)
    mkdir -p "$tmp_D/src"
    printf '**Source**: `src/nonexistent_source_selftest.rs::source_fn`\n' > "$tmp_D/bc-mock.md"
    # $tmp_D/src/nonexistent_source_selftest.rs intentionally NOT created
    set +e; BC_DIR="$tmp_D" SRC_ROOT="$tmp_D" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture D FAIL: expected rc=1, got $rc (Source-field dead citation)"; exit 1; }
    grep -qF 'DEAD: ' <<<"$output" \
        || { echo "Fixture D FAIL: DEAD: prefix missing from output (Source-field dead citation)"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture E — two-pass extraction §-form differential signal (F-B2-02/07)
    # Pass 1 extracts full token `src/mock_e.rs § "some section"` (backtick-only stop);
    # Pass 2 splits at space → `src/mock_e.rs`; file-existence check runs.
    # Assert "1 citations checked" — proves token was NOT silently dropped.
    # -------------------------------------------------------------------
    tmp_E=$(mktemp -d)
    mkdir -p "$tmp_E/src"
    printf '**Trace**: `src/mock_e.rs § "some section"`\n' > "$tmp_E/bc-mock.md"
    touch "$tmp_E/src/mock_e.rs"   # file exists; empty; no symbol check for §-form
    set +e; BC_DIR="$tmp_E" SRC_ROOT="$tmp_E" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 0 ] \
        || { echo "Fixture E FAIL: expected rc=0, got $rc"; exit 1; }
    grep -qF '1 citations checked' <<<"$output" \
        || { echo "Fixture E FAIL: '1 citations checked' missing (§-form token must not be silently dropped)"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture F — success path: fn + bare Source + const + fn-with-space-args
    # Three positive sub-probes + one negative sub-probe; ONE fixtures_run increment.
    # -------------------------------------------------------------------
    tmp_F=$(mktemp -d)
    mkdir -p "$tmp_F/src"
    printf '**Trace**: `src/mock_f.rs::mock_f_fn_selftest`\n**Source**: `src/mock_f.rs`\n' \
        > "$tmp_F/bc-mock.md"
    printf 'fn mock_f_fn_selftest() {}\n' > "$tmp_F/src/mock_f.rs"
    set +e; BC_DIR="$tmp_F" SRC_ROOT="$tmp_F" output=$(run_check 2>&1); rc=$?; set -e
    # Assert main probe BEFORE sub-probes overwrite output/rc.
    [ "$rc" -eq 0 ] \
        || { echo "Fixture F FAIL (main): expected rc=0, got $rc"; exit 1; }
    printf '%s\n' "$output" | grep -qE '^Check passed: [0-9]+ citations checked$' \
        || { echo "Fixture F FAIL (main): output does not match '^Check passed: [0-9]+ citations checked'"; exit 1; }

    # Fixture F sub-probe (1): pub(crate) const MAX_ADF_DEPTH (EC-CITE-051, anchored branch (d))
    # Citation references src/mock_f.rs (the mock file), NOT src/adf.rs (F-B2-01 fix).
    printf '**Trace**: `src/mock_f.rs::MAX_ADF_DEPTH`\n' >> "$tmp_F/bc-mock.md"
    printf 'pub(crate) const MAX_ADF_DEPTH: usize = 256;\n' >> "$tmp_F/src/mock_f.rs"
    set +e; BC_DIR="$tmp_F" SRC_ROOT="$tmp_F" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 0 ] \
        || { echo "Fixture F FAIL (sub-probe 1 EC-CITE-051): expected rc=0, got $rc"; exit 1; }
    printf '%s\n' "$output" | grep -qE '^Check passed: [0-9]+ citations checked$' \
        || { echo "Fixture F FAIL (sub-probe 1): output does not match '^Check passed: [0-9]+ citations checked'"; exit 1; }

    # Fixture F sub-probe (2): fn citation with space-args form (EC-CITE-059, F-B4-H-01)
    # Citation `mock_f_fn_selftest(args: T)`: Pass 2 space-split → `mock_f_fn_selftest(args:`;
    # strip-from-first-( removes `(args:` → `mock_f_fn_selftest`; fn-grep finds definition → ALIVE.
    # Delete-strip mutation: unstripped symbol has unbalanced ( → grep ERE malformed → exits 2 → caught.
    printf '**Trace**: `src/mock_f.rs::mock_f_fn_selftest(args: T)`\n' >> "$tmp_F/bc-mock.md"
    # fn mock_f_fn_selftest() {} defined above; body unchanged
    set +e; BC_DIR="$tmp_F" SRC_ROOT="$tmp_F" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 0 ] \
        || { echo "Fixture F FAIL (sub-probe 2 EC-CITE-059): expected rc=0, got $rc"; exit 1; }
    printf '%s\n' "$output" | grep -qE '^Check passed: [0-9]+ citations checked$' \
        || { echo "Fixture F FAIL (sub-probe 2): output does not match '^Check passed: [0-9]+ citations checked'"; exit 1; }

    # Fixture F negative sub-probe: doc-comment mock MUST classify DEAD under anchored form (F-B3-02)
    # Indented `    // pub const MAX_ADF_DEPTH:` has non-whitespace `//` before `pub const` —
    # ^[[:space:]]* anchor rejects the match → DEAD.
    tmp_F_neg=$(mktemp -d)
    mkdir -p "$tmp_F_neg/src"
    printf '**Trace**: `src/mock_f_neg.rs::MAX_ADF_DEPTH`\n' > "$tmp_F_neg/bc-mock.md"
    printf '    // pub const MAX_ADF_DEPTH: usize = 256;\n' > "$tmp_F_neg/src/mock_f_neg.rs"
    set +e; BC_DIR="$tmp_F_neg" SRC_ROOT="$tmp_F_neg" output_fn=$(run_check 2>&1); rc_fn=$?; set -e
    [ "$rc_fn" -eq 1 ] \
        || { echo "Fixture F FAIL (negative sub-probe): doc-comment mock should be DEAD, got rc=$rc_fn"; exit 1; }

    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture G — coverage-floor probe: TWO probes; ONE fixtures_run increment.
    # CANONICAL_MODE=1 set inline; unset after all G assertions (F-B2-06 + Story A Fixture H).
    # -------------------------------------------------------------------
    tmp_G=$(mktemp -d)
    mkdir -p "$tmp_G/src"
    printf '**Trace**: `src/mock_g.rs::mock_g_fn_selftest`\n' > "$tmp_G/bc-mock.md"
    printf 'fn mock_g_fn_selftest() {}\n' > "$tmp_G/src/mock_g.rs"
    CANONICAL_MODE=1   # toggle floor guard ON (script-scope variable)
    # G main probe: 1 citation, CANONICAL_MODE=1 → floor fires (1 < FLOOR=248)
    set +e; BC_DIR="$tmp_G" SRC_ROOT="$tmp_G" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture G FAIL (main probe): expected rc=1, got $rc"; exit 1; }
    grep -qF 'BC-CITE-COVERAGE-FLOOR:' <<<"$output" \
        || { echo "Fixture G FAIL (main probe): BC-CITE-COVERAGE-FLOOR: missing from output"; exit 1; }
    grep -qF "expected >= ${FLOOR}" <<<"$output" \
        || { echo "Fixture G FAIL (main probe): 'expected >= \${FLOOR}' missing from output"; exit 1; }

    # G second probe: 100 citations (still below FLOOR=248)
    # Kill-trace: mutation -lt "$FLOOR" → -lt "5": 100 > 5 → rc=0 → assertion fails → caught.
    tmp_G2=$(mktemp -d)
    mkdir -p "$tmp_G2/src"
    { for i in $(seq 1 100); do printf '**Trace**: `src/mock_g2.rs::mock_g2_fn`\n'; done; } \
        > "$tmp_G2/bc-mock.md"
    printf 'fn mock_g2_fn() {}\n' > "$tmp_G2/src/mock_g2.rs"
    # CANONICAL_MODE=1 still set from above
    set +e; BC_DIR="$tmp_G2" SRC_ROOT="$tmp_G2" output_g2=$(run_check 2>&1); rc_g2=$?; set -e
    [ "$rc_g2" -eq 1 ] \
        || { echo "Fixture G FAIL (second probe 100 citations): expected rc=1, got $rc_g2"; exit 1; }
    grep -qF 'BC-CITE-COVERAGE-FLOOR:' <<<"$output_g2" \
        || { echo "Fixture G FAIL (second probe): BC-CITE-COVERAGE-FLOOR: missing from output"; exit 1; }
    grep -qF "expected >= ${FLOOR}" <<<"$output_g2" \
        || { echo "Fixture G FAIL (second probe): 'expected >= \${FLOOR}' missing from output"; exit 1; }
    unset CANONICAL_MODE   # Story A Fixture H precedent + F-B2-06: prevent leakage to subsequent fixtures
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture I — ::tests module-path ALIVE (EC-CITE-052, DEC-154 branch (b))
    # -------------------------------------------------------------------
    tmp_I=$(mktemp -d)
    mkdir -p "$tmp_I/src"
    printf '**Trace**: `src/mock_i.rs::tests`\n' > "$tmp_I/bc-mock.md"
    printf 'mod tests {\n}\n' > "$tmp_I/src/mock_i.rs"
    set +e; BC_DIR="$tmp_I" SRC_ROOT="$tmp_I" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 0 ] \
        || { echo "Fixture I FAIL: expected rc=0, got $rc"; exit 1; }
    printf '%s\n' "$output" | grep -qE '^Check passed: [0-9]+ citations checked$' \
        || { echo "Fixture I FAIL: output does not match '^Check passed: [0-9]+ citations checked'"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture J — ::tests module-path negative DEAD (EC-CITE-053)
    # File has bare text "nonexistent_mod" (no mod keyword) so permissive
    # grep -q fallback mutation is killed: bare text matches → rc=0 → caught.
    # -------------------------------------------------------------------
    tmp_J=$(mktemp -d)
    mkdir -p "$tmp_J/src"
    printf '**Trace**: `src/mock_j.rs::nonexistent_mod`\n' > "$tmp_J/bc-mock.md"
    printf 'nonexistent_mod\n' > "$tmp_J/src/mock_j.rs"   # bare text; no mod keyword → DEAD
    set +e; BC_DIR="$tmp_J" SRC_ROOT="$tmp_J" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture J FAIL: expected rc=1, got $rc"; exit 1; }
    grep -qF 'DEAD: ' <<<"$output" \
        || { echo "Fixture J FAIL: DEAD: prefix missing from output"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture K — standalone CamelCase type ALIVE (EC-CITE-054, DEC-154 branch (e))
    # -------------------------------------------------------------------
    tmp_K=$(mktemp -d)
    mkdir -p "$tmp_K/src"
    printf '**Trace**: `src/mock_k.rs::MockKStruct`\n' > "$tmp_K/bc-mock.md"
    printf 'pub struct MockKStruct {\n}\n' > "$tmp_K/src/mock_k.rs"
    set +e; BC_DIR="$tmp_K" SRC_ROOT="$tmp_K" output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 0 ] \
        || { echo "Fixture K FAIL: expected rc=0, got $rc"; exit 1; }
    printf '%s\n' "$output" | grep -qE '^Check passed: [0-9]+ citations checked$' \
        || { echo "Fixture K FAIL: output does not match '^Check passed: [0-9]+ citations checked'"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Post-fixture self-assertions (NOT fixtures; do NOT increment fixtures_run)
    # -------------------------------------------------------------------

    # BC-CITE-001 count pin: header comment + preamble grep + Step-1 echo + own assertion = 4.
    # RED-gate note: with the Step-1 fail-closed echo NOT yet implemented in run_check (stub),
    # the current count is 3 — this assertion fails RED (3 ≠ 4) until the implementer adds
    # the "BC-CITE-001: no bc-*.md files found" echo in Step 1 of run_check.
    [ "$(grep -cF 'BC-CITE-001' "${BASH_SOURCE[0]}")" = "4" ] \
        || { echo "SELF-TEST FAIL: citation-id header exact count mismatch (expected 4)"; exit 1; }

    # Anti-self-match: no FAIL: diagnostic line may contain the literal BC-CITE-001.
    # Fragment composition avoids self-match of this assertion line itself.
    lit1='BC-CITE-''001'
    [ "$(grep -E 'FAIL:' "${BASH_SOURCE[0]}" | grep -cF "$lit1")" = "0" ] \
        || { echo "SELF-TEST FAIL: tracked literal found in a FAIL: diagnostic string"; exit 1; }

    # bash -n count pin: top-of-file syntax check + own assertion = 2.
    [ "$(grep -cF 'bash -n' "${BASH_SOURCE[0]}")" = "2" ] \
        || { echo "SELF-TEST FAIL: bash -n count pin mismatch (expected 2)"; exit 1; }

    # grep -oE count pin: Pass-1 extraction call in run_check + own assertion = 2.
    # RED-gate note: the Pass-1 grep -oE call does not yet exist in run_check (stub).
    # Current count is 1 (only this assertion line) — fails RED until implementer adds it.
    [ "$(grep -cF 'grep -oE' "${BASH_SOURCE[0]}")" = "2" ] \
        || { echo "SELF-TEST FAIL: grep -oE count pin mismatch (expected 2)"; exit 1; }

    # Fixture-count integrity pin (string equality; prevents silent fixture omission).
    [ "$fixtures_run" = "$EXPECTED_FIXTURES" ] \
        || { echo "SELF-TEST-FIXTURE-COUNT: expected ${EXPECTED_FIXTURES} fixtures, got ${fixtures_run}"; exit 1; }

    echo "All self-test fixtures passed (${fixtures_run}/${EXPECTED_FIXTURES})"
    exit 0
else
    run_check
fi
