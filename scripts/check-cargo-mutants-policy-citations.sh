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
# Fully implemented (S-MUTANTS-SCOPE-GUARDS-1 / DEC-150). All 12 self-test fixtures pass.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Validate script syntax on every invocation (L-7/L-5 FIX — unconditional, top-of-file).
bash -n "${BASH_SOURCE[0]}"

# ---------------------------------------------------------------------------
# run_check — Guard 2 implementation (DEC-150 / S-MUTANTS-SCOPE-GUARDS-1)
#
# Parses §Scope bulleted list from POLICY_DOC, verifies each (file, fn) pair
# against source definitions via definition-anchored grep, and enforces
# SCOPE-EMPTY and SCOPE-COVERAGE-FLOOR guards.
# ---------------------------------------------------------------------------
run_check() {
    local policy_doc="${POLICY_DOC:-${REPO_ROOT}/docs/specs/cargo-mutants-policy.md}"
    local src_root="${SRC_ROOT:-${REPO_ROOT}}"
    local canonical="${CANONICAL_MODE:-0}"
    local FLOOR=11

    # ------------------------------------------------------------------
    # Step 1: Extract §Scope range (inclusive start, exclusive of next
    # ^## or ^### Sibling Candidates heading).
    # ------------------------------------------------------------------
    local scope_text
    scope_text=$(awk '
        /^## Scope$/ { in_scope=1; next }
        in_scope && /^## / { exit }
        in_scope && /^### Sibling Candidates/ { exit }
        in_scope { print }
    ' "$policy_doc")

    # ------------------------------------------------------------------
    # Step 2: Pre-filter fenced code spans (``` ... ```) from scope_text.
    # Lines inside a triple-backtick fence are removed entirely.
    # ------------------------------------------------------------------
    local filtered_text
    filtered_text=$(printf '%s\n' "$scope_text" | awk '
        /^```/ { in_fence = !in_fence; next }
        !in_fence { print }
    ')

    # ------------------------------------------------------------------
    # Step 3: Group state machine — assemble bullet groups.
    # A group starts at a line matching ^- .
    # Continuation: ^[[:space:]]{2,} (but not ^- )
    # Class-1 terminator: blank line (ends current group)
    # Class-4 terminator: non-blank, not ^- , not ^[[:space:]]{2,} (ends group)
    # Orphan continuation (no open group): ignored.
    # ------------------------------------------------------------------
    local groups=()
    local current_group=""
    local in_group=0

    while IFS= read -r line; do
        if printf '%s' "$line" | grep -qE '^- '; then
            # Start of new bullet — commit previous group if open
            if [ "$in_group" = "1" ] && [ -n "$current_group" ]; then
                groups+=("$current_group")
            fi
            current_group="$line"
            in_group=1
        elif printf '%s' "$line" | grep -qE '^[[:space:]]{2,}' && [ "$in_group" = "1" ]; then
            # Continuation line (indented ≥2 spaces, not a bullet)
            current_group="${current_group}
${line}"
        elif [ -z "$line" ]; then
            # Class-1 terminator: blank line — commit group if open
            if [ "$in_group" = "1" ] && [ -n "$current_group" ]; then
                groups+=("$current_group")
                current_group=""
                in_group=0
            fi
        else
            # Class-4 terminator: non-blank, not bullet, not continuation
            if [ "$in_group" = "1" ] && [ -n "$current_group" ]; then
                groups+=("$current_group")
                current_group=""
                in_group=0
            fi
            # orphan non-continuation line: ignore
        fi
    done <<< "$filtered_text"
    # Commit last open group
    if [ "$in_group" = "1" ] && [ -n "$current_group" ]; then
        groups+=("$current_group")
    fi

    # ------------------------------------------------------------------
    # Step 4 + 5: For each assembled group, extract (file, fn) pairs and
    # validate against source.
    # ------------------------------------------------------------------
    local N=0          # bullet count
    local M=0          # (file, fn) pairs validated
    local offenders=()

    for group in "${groups[@]}"; do
        N=$((N + 1))

        # Extract the first bullet line from the group
        local bullet_line
        bullet_line=$(printf '%s\n' "$group" | grep -m1 '^- ' || true)

        # Shape guard: first backtick token must match ^src/[a-zA-Z0-9_/.-]+\.rs$
        # and must not contain ".."
        local file
        file=$(printf '%s\n' "$bullet_line" | grep -oE '`[^` ]+`' | head -1 | tr -d '`' || true)

        if ! printf '%s' "$file" | grep -qE '^src/[a-zA-Z0-9_/.-]+\.rs$' \
            || printf '%s' "$file" | grep -qF '..'; then
            offenders+=("DEAD: malformed bullet skipped: ${bullet_line}")
            continue
        fi

        # File existence check
        if [ ! -f "${src_root}/${file}" ]; then
            offenders+=("DEAD: ${file} not found")
            continue
        fi

        # Extract function tokens: all backtick tokens after the file token,
        # strip trailing ::anything (::strip transform), filter to ^[a-z_][a-z0-9_]*$
        local all_tokens
        all_tokens=$(printf '%s\n' "$group" | grep -oE '`[^` ]+`' | tr -d '`' || true)

        # Skip the file token (first one matching the src/ pattern)
        local fn_tokens=()
        local file_seen=0
        while IFS= read -r token; do
            if [ "$file_seen" = "0" ] && printf '%s' "$token" | grep -qE '^src/[a-zA-Z0-9_/.-]+\.rs$'; then
                file_seen=1
                continue
            fi
            # Strip everything from last :: onward (::strip transform)
            local stripped
            stripped=$(printf '%s' "$token" | sed 's/.*:://')
            # Filter: must match ^[a-z_][a-z0-9_]*$
            if printf '%s' "$stripped" | grep -qE '^[a-z_][a-z0-9_]*$'; then
                fn_tokens+=("$stripped")
            fi
        done <<< "$all_tokens"

        # Validate each function via definition-anchored grep
        for fn_name in "${fn_tokens[@]}"; do
            M=$((M + 1))
            if ! grep -Eq "^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?((unsafe|const|async|extern[[:space:]]+\"[^\"]*\")[[:space:]]+)*fn[[:space:]]+${fn_name}([^[:alnum:]_]|\$)" \
                    "${src_root}/${file}"; then
                offenders+=("DEAD: ${fn_name} not found in ${file}")
            fi
        done
    done

    # ------------------------------------------------------------------
    # Step 6: SCOPE-EMPTY guard
    # ------------------------------------------------------------------
    if [ "$N" -eq 0 ]; then
        echo "SCOPE-EMPTY: 0 bullets parsed from §Scope — §Scope section missing, empty, or policy doc restructured"
        return 1
    fi

    # ------------------------------------------------------------------
    # Step 7: SCOPE-COVERAGE-FLOOR guard (canonical mode only)
    # ------------------------------------------------------------------
    if [ "$canonical" = "1" ] && [ "$N" -lt "$FLOOR" ]; then
        echo "SCOPE-COVERAGE-FLOOR: expected >= ${FLOOR} §Scope bullets, got ${N}. Update this PIN when bullets are intentionally removed (the floor is a lower bound; additions never fire it)."
        return 1
    fi

    # ------------------------------------------------------------------
    # Step 8: Report offenders or success
    # ------------------------------------------------------------------
    if [ "${#offenders[@]}" -gt 0 ]; then
        for o in "${offenders[@]}"; do
            echo "$o"
        done
        echo "${#offenders[@]} stale citation(s) found in ${policy_doc} §Scope"
        return 1
    fi

    echo "Check passed: ${N} bullets parsed, ${M} (file, fn) pairs validated"
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

    # Preamble check (L-4 FIX — run inside --self-test, BEFORE any fixture)
    grep -Eq '^#.*CI-MUTANTS-CITE-001' "${BASH_SOURCE[0]}" \
        || { echo "SELF-TEST FAIL: citation-guard error-code literal missing from script header comment"; exit 1; }

    # Fixture counter and integrity pin (F-6 FIX / MED-4-P23 FIX / F-VA-28-3 FIX)
    readonly EXPECTED_FIXTURES=12
    fixtures_run=0

    # Register cleanup trap for all twelve fixture dirs before creating any tmpdir.
    trap 'rm -rf "${tmp_A:-}" "${tmp_B:-}" "${tmp_C:-}" "${tmp_D:-}" "${tmp_E:-}" "${tmp_F:-}" "${tmp_G:-}" "${tmp_H:-}" "${tmp_I:-}" "${tmp_J:-}" "${tmp_K:-}" "${tmp_L:-}"' EXIT

    # -------------------------------------------------------------------
    # Fixture A: basic dead-symbol (FIND-VA-36-2: duplicated token; A=3 summary count)
    # -------------------------------------------------------------------
    tmp_A=$(mktemp -d)
    printf '## Scope\n- `src/adf.rs` — `handle_nonexistent_fn_selftest`, `another_missing_fn_selftest`, `handle_nonexistent_fn_selftest`\n\n## Terminator\n' \
        > "$tmp_A/policy.md"
    mkdir -p "$tmp_A/src"
    touch "$tmp_A/src/adf.rs"
    POLICY_DOC="$tmp_A/policy.md"
    SRC_ROOT="$tmp_A"
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture A FAIL: expected rc=1, got $rc"; exit 1; }
    [ "$(grep -c 'DEAD: ' <<<"$output")" = "3" ] \
        || { echo "Fixture A FAIL: expected 3 DEAD lines, got $(grep -c 'DEAD: ' <<<"$output")"; exit 1; }
    grep -qF 'handle_nonexistent_fn_selftest' <<<"$output" \
        || { echo "Fixture A FAIL: handle_nonexistent_fn_selftest missing from output"; exit 1; }
    grep -qF 'another_missing_fn_selftest' <<<"$output" \
        || { echo "Fixture A FAIL: another_missing_fn_selftest missing from output"; exit 1; }
    grep -qE '^3 stale citation\(s\) found in .+ §Scope$' <<<"$output" \
        || { echo "Fixture A FAIL: summary count mismatch (expected 3)"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture B: import-only false-green proof
    # -------------------------------------------------------------------
    tmp_B=$(mktemp -d)
    printf '## Scope\n- `src/cli/issue/create.rs` — `handle_jsm_create`\n\n## Terminator\n' \
        > "$tmp_B/policy.md"
    mkdir -p "$tmp_B/src/cli/issue"
    printf 'use super::jsm_create::{handle_jsm_create};\n' > "$tmp_B/src/cli/issue/create.rs"
    POLICY_DOC="$tmp_B/policy.md"
    SRC_ROOT="$tmp_B"
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture B FAIL: expected rc=1, got $rc"; exit 1; }
    grep -qF 'DEAD: ' <<<"$output" \
        || { echo "Fixture B FAIL: DEAD: prefix missing from output"; exit 1; }
    grep -qF ' not found in ' <<<"$output" \
        || { echo "Fixture B FAIL: 'not found in' missing from output"; exit 1; }
    grep -qF 'handle_jsm_create' <<<"$output" \
        || { echo "Fixture B FAIL: handle_jsm_create missing from output"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture C: empty src-root — all citations dead (FIND-VA-37-2: count pin)
    # -------------------------------------------------------------------
    tmp_C=$(mktemp -d)
    printf '## Scope\n- `src/foo.rs` — `fn_alpha`\n- `src/bar.rs` — `fn_beta`\n\n## Terminator\n' \
        > "$tmp_C/policy.md"
    POLICY_DOC="$tmp_C/policy.md"
    SRC_ROOT="$tmp_C"
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture C FAIL: expected rc=1, got $rc"; exit 1; }
    grep -qF 'DEAD: src/foo.rs not found' <<<"$output" \
        || { echo "Fixture C FAIL: DEAD: src/foo.rs not found missing from output"; exit 1; }
    grep -qF 'DEAD: src/bar.rs not found' <<<"$output" \
        || { echo "Fixture C FAIL: DEAD: src/bar.rs not found missing from output"; exit 1; }
    [ "$(grep -c 'DEAD: [^ ]* not found$' <<<"$output")" = "2" ] \
        || { echo "Fixture C FAIL: not-found offender count mismatch"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture D: scope-empty (FIND-VA-42-2: three message pins; F-VA-32-5: SRC_ROOT default-init)
    # -------------------------------------------------------------------
    unset SRC_ROOT POLICY_DOC
    tmp_D=$(mktemp -d)
    printf '## Scope\n\n## Terminator\n' > "$tmp_D/policy.md"
    POLICY_DOC="$tmp_D/policy.md"
    # SRC_ROOT intentionally NOT set — exercises default-init branch in run_check
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture D FAIL: expected rc=1, got $rc"; exit 1; }
    grep -qF 'SCOPE-EMPTY:' <<<"$output" \
        || { echo "Fixture D FAIL: SCOPE-EMPTY prefix missing from output"; exit 1; }
    grep -qF '0 bullets parsed' <<<"$output" \
        || { echo "Fixture D FAIL: SCOPE-EMPTY message missing expected substring '0 bullets parsed'"; exit 1; }
    grep -qF 'policy doc restructured' <<<"$output" \
        || { echo "Fixture D FAIL: SCOPE-EMPTY message missing expected substring 'policy doc restructured'"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture E: malformed-bullet shape-guard (five bullets, E=5 summary count)
    # -------------------------------------------------------------------
    unset SRC_ROOT POLICY_DOC
    tmp_E=$(mktemp -d)
    printf '## Scope\n- not-a-backtick-path — some_fn\n- `docs/foo.md` — non_src_fn\n- `src/../escape.rs` — `some_fn`\n- `src/foo.py` — `some_fn`\n- `src/foo.rs.bak` — `some_fn`\n\n## Terminator\n' \
        > "$tmp_E/policy.md"
    POLICY_DOC="$tmp_E/policy.md"
    SRC_ROOT="$tmp_E"
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture E FAIL: expected rc=1, got $rc"; exit 1; }
    [ "$(grep -c 'DEAD: malformed bullet skipped:' <<<"$output")" = "5" ] \
        || { echo "Fixture E FAIL: expected 5 malformed-bullet-skipped lines, got $(grep -c 'DEAD: malformed bullet skipped:' <<<"$output")"; exit 1; }
    grep -qF 'DEAD: malformed bullet skipped: - not-a-backtick-path — some_fn' <<<"$output" \
        || { echo "Fixture E FAIL: bullet_line content pin missing from output"; exit 1; }
    grep -qF 'DEAD: malformed bullet skipped: - `src/foo.py` — `some_fn`' <<<"$output" \
        || { echo "Fixture E FAIL: bullet 4 content pin missing from output"; exit 1; }
    grep -qE '^5 stale citation\(s\) found in .+ §Scope$' <<<"$output" \
        || { echo "Fixture E FAIL: summary line not found or wrong count (expected 5)"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture F: two probes, ONE fixtures_run increment
    # -------------------------------------------------------------------
    tmp_F=$(mktemp -d)

    # Probe F-a: success path (TWO bullets; SomeStruct filtered; M=8)
    printf '## Scope\n- `src/mock_mod.rs` — `mock_fn_alpha`, `mock_const_fn`, `mock_unsafe_fn`, `mock_extern_fn`, `mock_scoped_async_fn`, `mock_crate_fn`, `mock_indented_fn`, `SomeStruct`\n- `src/mock_second.rs` — `second_probe_fn`\n\n## Terminator\n' \
        > "$tmp_F/policy.md"
    mkdir -p "$tmp_F/src"
    printf 'pub fn mock_fn_alpha() {}\nconst fn mock_const_fn() -> u32 { 0 }\nunsafe fn mock_unsafe_fn() {}\nextern "C" fn mock_extern_fn() {}\npub(super) async fn mock_scoped_async_fn() {}\npub(crate) fn mock_crate_fn() {}\nimpl MockStruct {\n    pub async fn mock_indented_fn() {}\n}\n' \
        > "$tmp_F/src/mock_mod.rs"
    printf 'fn second_probe_fn() {}\n' > "$tmp_F/src/mock_second.rs"
    POLICY_DOC="$tmp_F/policy.md"
    SRC_ROOT="$tmp_F"
    set +e; output_fa=$(run_check 2>&1); rc_fa=$?; set -e
    [ "$rc_fa" -eq 0 ] \
        || { echo "Fixture F FAIL: Probe F-a expected rc=0, got $rc_fa"; exit 1; }
    grep -qE '^Check passed: 2 bullets parsed, 8 \(file, fn\) pairs validated$' <<<"$output_fa" \
        || { echo "Fixture F FAIL: Probe F-a summary regex mismatch (expected '2 bullets parsed, 8 pairs')"; exit 1; }
    if grep -q 'stale citation' <<<"$output_fa"; then
        echo "Fixture F-a FAIL: negative summary emitted on clean run"; exit 1
    fi

    # Probe F-b: trailing-boundary RED probe (mock_prefix cited, mock_prefix_extended defined)
    printf '## Scope\n- `src/mock_prefix.rs` — `mock_prefix`\n\n## Terminator\n' \
        > "$tmp_F/policy_f_prefix.md"
    printf 'fn mock_prefix_extended() {}\n' > "$tmp_F/src/mock_prefix.rs"
    POLICY_DOC="$tmp_F/policy_f_prefix.md"
    # SRC_ROOT unchanged = $tmp_F
    set +e; output_fb=$(run_check 2>&1); rc_fb=$?; set -e
    [ "$rc_fb" -eq 1 ] \
        || { echo "Fixture F FAIL: Probe F-b expected rc=1 (boundary intact), got $rc_fb"; exit 1; }
    grep -qF 'DEAD: mock_prefix not found in src/mock_prefix.rs' <<<"$output_fb" \
        || { echo "Fixture F FAIL: Probe F-b DEAD line missing from output"; exit 1; }
    grep -qE '^[0-9]+ stale citation\(s\) found in .+ §Scope$' <<<"$output_fb" \
        || { echo "Fixture F FAIL: Probe F-b summary line not found"; exit 1; }

    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture G: file-existence-only + fence-skip (N=2, M=0)
    # -------------------------------------------------------------------
    tmp_G=$(mktemp -d)
    printf '## Scope\n- `src/mock_mod.rs` — serde structs for X\n```\n- `src/fenced_fake.rs` — `fenced_fake_fn`\n```\n- `src/mock_mod2.rs` — more serde structs\n\n## Terminator\n' \
        > "$tmp_G/policy.md"
    mkdir -p "$tmp_G/src"
    touch "$tmp_G/src/mock_mod.rs"
    touch "$tmp_G/src/mock_mod2.rs"
    # src/fenced_fake.rs deliberately NOT created
    POLICY_DOC="$tmp_G/policy.md"
    SRC_ROOT="$tmp_G"
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 0 ] \
        || { echo "Fixture G FAIL: expected rc=0, got $rc"; exit 1; }
    grep -qE '^Check passed: 2 bullets parsed, 0 \(file, fn\) pairs validated$' <<<"$output" \
        || { echo "Fixture G FAIL: expected '2 bullets parsed, 0 pairs validated' in output"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture H: SCOPE-COVERAGE-FLOOR (four probes; ONE fixtures_run increment)
    # -------------------------------------------------------------------
    tmp_H=$(mktemp -d)
    printf '## Scope\n- `src/mock_h.rs` — `mock_fn_one_selftest`\n- `src/mock_h2.rs` — `mock_fn_two_selftest`\n\n## Terminator\n' \
        > "$tmp_H/policy.md"
    mkdir -p "$tmp_H/src"
    printf 'fn mock_fn_one_selftest() {}\n' > "$tmp_H/src/mock_h.rs"
    printf 'fn mock_fn_two_selftest() {}\n' > "$tmp_H/src/mock_h2.rs"
    # Boundary companion (N=11)
    printf '## Scope\n- `src/mock_h_boundary.rs` — `fn_b01`\n- `src/mock_h_boundary.rs` — `fn_b02`\n- `src/mock_h_boundary.rs` — `fn_b03`\n- `src/mock_h_boundary.rs` — `fn_b04`\n- `src/mock_h_boundary.rs` — `fn_b05`\n- `src/mock_h_boundary.rs` — `fn_b06`\n- `src/mock_h_boundary.rs` — `fn_b07`\n- `src/mock_h_boundary.rs` — `fn_b08`\n- `src/mock_h_boundary.rs` — `fn_b09`\n- `src/mock_h_boundary.rs` — `fn_b10`\n- `src/mock_h_boundary.rs` — `fn_b11`\n\n## Terminator\n' \
        > "$tmp_H/policy_h_boundary.md"
    printf 'fn fn_b01() {}\nfn fn_b02() {}\nfn fn_b03() {}\nfn fn_b04() {}\nfn fn_b05() {}\nfn fn_b06() {}\nfn fn_b07() {}\nfn fn_b08() {}\nfn fn_b09() {}\nfn fn_b10() {}\nfn fn_b11() {}\n' \
        > "$tmp_H/src/mock_h_boundary.rs"
    POLICY_DOC="$tmp_H/policy.md"
    SRC_ROOT="$tmp_H"
    CANONICAL_MODE=1

    # RED call (N=2)
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture H FAIL: RED N=2 expected rc=1, got $rc"; exit 1; }
    grep -qF 'SCOPE-COVERAGE-FLOOR:' <<<"$output" \
        || { echo "Fixture H FAIL: SCOPE-COVERAGE-FLOOR: missing from N=2 output"; exit 1; }
    grep -qF 'expected >= 11' <<<"$output" \
        || { echo "Fixture H FAIL: 'expected >= 11' missing from N=2 output"; exit 1; }
    grep -qF 'got 2' <<<"$output" \
        || { echo "Fixture H FAIL: 'got 2' missing from N=2 output"; exit 1; }

    # GREEN companion (N=11 boundary)
    POLICY_DOC="$tmp_H/policy_h_boundary.md"
    set +e; output_h2=$(run_check 2>&1); rc_h2=$?; set -e
    [ "$rc_h2" -eq 0 ] \
        || { echo "Fixture H FAIL: GREEN N=11 expected rc=0, got $rc_h2"; exit 1; }
    if grep -q 'SCOPE-COVERAGE-FLOOR:' <<<"$output_h2"; then echo "Fixture H FAIL: floor fired at N=11 boundary"; exit 1; fi

    # RED probe (N=5 — closes < 11 → < 4 gap)
    printf '## Scope\n- `src/mock_h_red5.rs` — `fn_r01`\n- `src/mock_h_red5.rs` — `fn_r02`\n- `src/mock_h_red5.rs` — `fn_r03`\n- `src/mock_h_red5.rs` — `fn_r04`\n- `src/mock_h_red5.rs` — `fn_r05`\n\n## Terminator\n' \
        > "$tmp_H/policy_h_red5.md"
    printf 'fn fn_r01() {}\nfn fn_r02() {}\nfn fn_r03() {}\nfn fn_r04() {}\nfn fn_r05() {}\n' \
        > "$tmp_H/src/mock_h_red5.rs"
    POLICY_DOC="$tmp_H/policy_h_red5.md"
    set +e; output_h3=$(run_check 2>&1); rc_h3=$?; set -e
    [ "$rc_h3" -eq 1 ] \
        || { echo "Fixture H FAIL: RED N=5 expected rc=1, got $rc_h3"; exit 1; }
    grep -qF 'SCOPE-COVERAGE-FLOOR:' <<<"$output_h3" \
        || { echo "Fixture H FAIL: SCOPE-COVERAGE-FLOOR: missing from N=5 output"; exit 1; }
    grep -qF 'expected >= 11' <<<"$output_h3" \
        || { echo "Fixture H FAIL: 'expected >= 11' missing from N=5 output"; exit 1; }
    grep -qF 'got 5' <<<"$output_h3" \
        || { echo "Fixture H FAIL: 'got 5' missing from N=5 output"; exit 1; }

    # GREEN above-threshold (N=12)
    printf '## Scope\n- `src/mock_h_above.rs` — `fn_a01`\n- `src/mock_h_above.rs` — `fn_a02`\n- `src/mock_h_above.rs` — `fn_a03`\n- `src/mock_h_above.rs` — `fn_a04`\n- `src/mock_h_above.rs` — `fn_a05`\n- `src/mock_h_above.rs` — `fn_a06`\n- `src/mock_h_above.rs` — `fn_a07`\n- `src/mock_h_above.rs` — `fn_a08`\n- `src/mock_h_above.rs` — `fn_a09`\n- `src/mock_h_above.rs` — `fn_a10`\n- `src/mock_h_above.rs` — `fn_a11`\n- `src/mock_h_above.rs` — `fn_a12`\n\n## Terminator\n' \
        > "$tmp_H/policy_h_above.md"
    printf 'fn fn_a01() {}\nfn fn_a02() {}\nfn fn_a03() {}\nfn fn_a04() {}\nfn fn_a05() {}\nfn fn_a06() {}\nfn fn_a07() {}\nfn fn_a08() {}\nfn fn_a09() {}\nfn fn_a10() {}\nfn fn_a11() {}\nfn fn_a12() {}\n' \
        > "$tmp_H/src/mock_h_above.rs"
    POLICY_DOC="$tmp_H/policy_h_above.md"
    set +e; output_h4=$(run_check 2>&1); rc_h4=$?; set -e
    [ "$rc_h4" -eq 0 ] \
        || { echo "Fixture H FAIL: GREEN N=12 expected rc=0, got $rc_h4"; exit 1; }
    if grep -q 'SCOPE-COVERAGE-FLOOR:' <<<"$output_h4"; then echo "Fixture H FAIL: floor fired above threshold"; exit 1; fi

    unset CANONICAL_MODE
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture I: sibling-candidates terminator + general heading stop
    # Two probes, ONE fixtures_run increment.
    # -------------------------------------------------------------------
    tmp_I=$(mktemp -d)

    # Probe I-a: ### Sibling Candidates prefix stop
    printf '## Scope\n- `src/in_scope.rs` — `in_scope_fn`\n### Sibling Candidates Considered and Deferred (MOCK)\n- `src/should_not_be_parsed.rs` — `should_not_be_parsed_fn`\n\n## Terminator\n' \
        > "$tmp_I/policy.md"
    mkdir -p "$tmp_I/src"
    printf 'fn in_scope_fn() {}\n' > "$tmp_I/src/in_scope.rs"
    # src/should_not_be_parsed.rs deliberately NOT created
    POLICY_DOC="$tmp_I/policy.md"
    SRC_ROOT="$tmp_I"
    set +e; output_ia=$(run_check 2>&1); rc_ia=$?; set -e
    [ "$rc_ia" -eq 0 ] \
        || { echo "Fixture I FAIL: Probe I-a expected rc=0, got $rc_ia"; exit 1; }
    grep -qE '^Check passed: 1 bullets parsed, 1 \(file, fn\) pairs validated$' <<<"$output_ia" \
        || { echo "Fixture I FAIL: Probe I-a expected '1 bullets parsed, 1 pairs validated'"; exit 1; }

    # Probe I-b: ^## general heading stop (no ### heading)
    printf '## Scope\n- `src/in_scope.rs` — `in_scope_fn`\n\n## Terminator\n- `src/after_terminator.rs` — `after_fn`\n' \
        > "$tmp_I/policy_i_general.md"
    # src/in_scope.rs already created; src/after_terminator.rs deliberately NOT created
    POLICY_DOC="$tmp_I/policy_i_general.md"
    set +e; output_ib=$(run_check 2>&1); rc_ib=$?; set -e
    [ "$rc_ib" -eq 0 ] \
        || { echo "Fixture I FAIL: Probe I-b expected rc=0, got $rc_ib"; exit 1; }
    grep -qE '^Check passed: 1 bullets parsed, 1 \(file, fn\) pairs validated$' <<<"$output_ib" \
        || { echo "Fixture I FAIL: Probe I-b expected '1 bullets parsed, 1 pairs validated'"; exit 1; }

    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture J: multi-line bullet continuation (TWO groups; N=2, M=3)
    # -------------------------------------------------------------------
    tmp_J=$(mktemp -d)
    printf '## Scope\n- `src/multi_line.rs` — `first_fn`,\n  `second_fn`\n `leaked_one_space_fn`\n- `src/multi_line.rs` — `third_fn`\n\n  `leaked_after_blank_fn`\n`prose_fn_leaked` is documented here.\n\n## Terminator\n' \
        > "$tmp_J/policy.md"
    mkdir -p "$tmp_J/src"
    printf 'fn first_fn() {}\nfn second_fn() {}\nfn third_fn() {}\n' > "$tmp_J/src/multi_line.rs"
    POLICY_DOC="$tmp_J/policy.md"
    SRC_ROOT="$tmp_J"
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 0 ] \
        || { echo "Fixture J FAIL: expected rc=0, got $rc"; exit 1; }
    grep -qE '^Check passed: 2 bullets parsed, 3 \(file, fn\) pairs validated$' <<<"$output" \
        || { echo "Fixture J FAIL: expected '2 bullets parsed, 3 pairs validated'"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture K: file-existence-only entry with missing file
    # -------------------------------------------------------------------
    tmp_K=$(mktemp -d)
    printf '## Scope\n- `src/typesonly.rs` — serde structs\n\n## Terminator\n' \
        > "$tmp_K/policy.md"
    # NO mock source file created — src/typesonly.rs does not exist
    POLICY_DOC="$tmp_K/policy.md"
    SRC_ROOT="$tmp_K"
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 1 ] \
        || { echo "Fixture K FAIL: expected rc=1, got $rc"; exit 1; }
    grep -qF 'DEAD: src/typesonly.rs not found' <<<"$output" \
        || { echo "Fixture K FAIL: expected DEAD: src/typesonly.rs not found in output"; exit 1; }
    grep -qE '^[0-9]+ stale citation\(s\) found in .+ §Scope$' <<<"$output" \
        || { echo "Fixture K FAIL: summary line not found or wrong format"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Fixture L: ::-strip transform (two-:: token; range-start pin)
    # -------------------------------------------------------------------
    tmp_L=$(mktemp -d)
    printf '## Purpose\n- `src/pre_scope.rs` — `pre_scope_fn`\n\n## Scope\n- `src/mock_qq.rs` — `Outer::Mock::build_fn`\n\n## Terminator\n' \
        > "$tmp_L/policy.md"
    mkdir -p "$tmp_L/src"
    printf 'fn build_fn() {}\n' > "$tmp_L/src/mock_qq.rs"
    # src/pre_scope.rs deliberately NOT created
    POLICY_DOC="$tmp_L/policy.md"
    SRC_ROOT="$tmp_L"
    set +e; output=$(run_check 2>&1); rc=$?; set -e
    [ "$rc" -eq 0 ] \
        || { echo "Fixture L FAIL: expected rc=0, got $rc"; exit 1; }
    grep -qE '^Check passed: 1 bullets parsed, 1 \(file, fn\) pairs validated$' <<<"$output" \
        || { echo "Fixture L FAIL: expected 1 parsed, 1 pair validated"; exit 1; }
    fixtures_run=$((fixtures_run + 1))

    # -------------------------------------------------------------------
    # Post-fixture self-assertions (NOT fixtures; MUST NOT increment fixtures_run)
    # F-VA-2 / F-VA-3 / F1-P29 / VA-34-3 FIX
    # -------------------------------------------------------------------
    [ "$(grep -cF 'CI-MUTANTS-CITE-001' "${BASH_SOURCE[0]}")" = "3" ] \
        || { echo "SELF-TEST FAIL: citation-id header/preamble exact count mismatch"; exit 1; }
    [ "$(grep -cF 'bash -n' "${BASH_SOURCE[0]}")" = "2" ] \
        || { echo "SELF-TEST FAIL: syntax-self-check exact count mismatch"; exit 1; }
    [ "$(grep -cF 'grep -Eq' "${BASH_SOURCE[0]}")" = "3" ] \
        || { echo "SELF-TEST FAIL: preamble-grep exact count mismatch"; exit 1; }
    # anti-self-match: literals composed from fragments
    lit1='CI-MUTANTS-''CITE-001'; lit2='bash'' -n'; lit3='grep'' -Eq'
    [ "$(grep -E 'FAIL:' "${BASH_SOURCE[0]}" | grep -cE "$lit1|$lit2|$lit3")" = "0" ] \
        || { echo "SELF-TEST FAIL: tracked literal found in a diagnostic string"; exit 1; }

    # Fixture-count integrity pin (F-6 FIX / MED-4-P23 FIX / F-VA-28-3 FIX)
    [ "$fixtures_run" = "$EXPECTED_FIXTURES" ] \
        || { echo "SELF-TEST-FIXTURE-COUNT: expected ${EXPECTED_FIXTURES} fixtures, got ${fixtures_run}"; exit 1; }

    exit 0
fi

run_check
exit $?
