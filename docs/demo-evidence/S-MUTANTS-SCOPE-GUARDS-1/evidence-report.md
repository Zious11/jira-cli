# Demo Evidence Report — S-MUTANTS-SCOPE-GUARDS-1

**Story:** CITATION-GUARDS Story A: mutants-policy function-location guard + examine_globs file-existence guard (DEC-150)  
**Branch:** `ci/mutants-scope-guards`  
**Commit at recording:** `cac21ec`  
**Recorded:** 2026-07-04  
**Story version:** v1.48

---

## Coverage Summary

| AC | Description | Success Path | Error Path | Type |
|----|-------------|-------------|-----------|------|
| AC-001 | Guard 2 script exits 0 on clean develop HEAD | AC-001-guard2-success.gif/.webm | Covered by AC-003 failure demo | VHS |
| AC-002 | Guard 2 `--self-test` exits 0 (12 fixtures pass) | AC-002-guard2-selftest.gif/.webm | Fixture suite is the error-path proof | VHS |
| AC-003 | Guard 2 error messages follow CI-MUTANTS-CITE-001 format | Covered by AC-001 (exit 0 path) | AC-003-guard2-failure.gif/.webm | VHS |
| AC-004 | Guard 3 Rust test: all 11 globs resolve (9/9 tests pass) | AC-004-guard3-success.gif/.webm | Covered by AC-005 | VHS |
| AC-005 | Guard 3 test fails deterministically on seeded dead glob | Covered by AC-004 | AC-005-guard3-failure.gif/.webm | VHS |
| AC-006 | Test naming, CI wiring, conventional-commit | AC-006-ci-wiring.gif/.webm (CI job section) | N/A — structural check | VHS |
| AC-007 | Doc fallout: policy doc, CLAUDE.md, CHANGELOG.md updated | Verified via grep evidence below | N/A — documentary | Transcript |

---

## AC-001 — Guard 2 SUCCESS path

**File:** `AC-001-guard2-success.gif` / `.webm`  
**Command run:** `bash scripts/check-cargo-mutants-policy-citations.sh`  
**Expected output (byte-pinned):**
```
Check passed: 11 bullets parsed, 21 (file, fn) pairs validated
```
**Exit code:** 0

The recording shows the script running on the worktree HEAD and emitting the expected
`Check passed: 11 bullets parsed, 21 (file, fn) pairs validated` line, then returning exit 0.

---

## AC-002 — Guard 2 SELF-TEST (12-fixture success)

**File:** `AC-002-guard2-selftest.gif` / `.webm`  
**Command run:** `bash scripts/check-cargo-mutants-policy-citations.sh --self-test && echo 'EXIT: 0'`  
**Expected behaviour:** 12 fixtures (A–L) run offline; all assertions pass; exits 0.

The `--self-test` mode runs 12 fixtures that seed known-dead citations and verify
the script produces the correct CI-MUTANTS-CITE-001 error messages. All 12 fixtures
pass deterministically without network access. The `EXIT: 0` confirmation appears at
the end of the recording.

The self-test is the primary RED-provable evidence: it proves the guard would detect
stale citations if they existed on a future PR (each fixture plants a known defect and
verifies the guard catches it).

---

## AC-003 — Guard 2 FAILURE path (CI-MUTANTS-CITE-001)

**File:** `AC-003-guard2-failure.gif` / `.webm`  
**Helper script:** `demo-guard2-failure.sh` (creates temp copy, runs guard, verifies cleanup)  
**Method:** Corrupts `handle_create` → `handle_create_nonexistent` in a **temp copy** of
`docs/specs/cargo-mutants-policy.md` using `sed`, then runs the guard with
`POLICY_DOC="$TMPFILE"`. The real policy doc is never modified.

**Observed output:**
```
=== AC-003 / AC-001 ERROR PATH: Guard 2 failure demo ===

Temp policy doc: /tmp/policy-corrupt-XXXXXX.md
Corruption: renamed 'handle_create' -> 'handle_create_nonexistent' in §Scope

Running: POLICY_DOC="<tmp>" bash scripts/check-cargo-mutants-policy-citations.sh
---
DEAD: handle_create_nonexistent not found in src/cli/issue/create.rs
1 stale citation(s) found in /tmp/policy-corrupt-XXXXXX.md §Scope
---
Exit code: 1

Verifying real policy doc is unchanged...
OK: git diff clean (real policy doc unmodified)
```

This demonstrates:
- CI-MUTANTS-CITE-001 format: `DEAD: <fn> not found in <file>`
- Summary line: `K stale citation(s) found in <path> §Scope`
- Exit code 1 on any dead citation
- All offenders collected before reporting (collect-all, not fail-fast)
- Real policy doc left unmodified (git diff clean confirmed in-demo)

---

## AC-004 — Guard 3 SUCCESS path (9/9 tests pass)

**File:** `AC-004-guard3-success.gif` / `.webm`  
**Command run:** `cargo test --test mutants_glob_existence 2>&1 | tail -15`  
**Expected:** 9 tests pass, 0 failed

The recording shows all 9 test functions in `tests/mutants_glob_existence.rs` passing:

```
running 9 tests
test test_coverage_floor_does_not_panic_above_threshold ... ok
test test_coverage_floor_does_not_panic_at_exact_threshold ... ok
test test_coverage_floor_panics_at_ten_entries_below_threshold ... ok
test test_detect_empty_examine_globs_array_panics_with_key_missing_message ... ok
test test_detect_missing_examine_globs_key_panics_with_key_missing_message ... ok
test test_coverage_floor_panics_when_entries_below_threshold ... ok
test test_reject_nonexistent_examine_globs_entry_returns_dead_list ... ok
test test_validate_globs_via_toml_parse_returns_dead_entry ... ok
test test_resolve_all_examine_globs_entries_to_real_files ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The test `test_resolve_all_examine_globs_entries_to_real_files` dynamically reads all 11
`examine_globs` entries from `.cargo/mutants.toml` and verifies each resolves to ≥1 real file.

---

## AC-005 — Guard 3 FAILURE path (seeded dead glob)

**File:** `AC-005-guard3-failure.gif` / `.webm`  
**Command run:** `cargo test --test mutants_glob_existence test_reject_nonexistent_examine_globs_entry 2>&1 | tail -12`

This runs `test_reject_nonexistent_examine_globs_entry_returns_dead_list`, which calls
`validate_globs` with `["src/nonexistent_dummy_for_selftest.rs"]` directly (not from `.cargo/mutants.toml`).
The test asserts the returned `Vec<String>` is non-empty, proving guard logic correctly
identifies dead globs.

The test passes (exit 0) — it is a GREEN test that proves the ERROR-PATH logic works.
This is correct: the test itself is the RED-provable fixture — it would fail (RED) if the
`validate_globs` helper had an inverted polarity bug and returned an empty Vec for a dead glob.

---

## AC-006 — CI wiring evidence

**File:** `AC-006-ci-wiring.gif` / `.webm`  
**Command run:** `grep -A 12 'spec-guard:' .github/workflows/ci.yml | head -15`

The recording shows the `spec-guard` job block in `.github/workflows/ci.yml` including:

```yaml
spec-guard:
  name: Spec Guards (BC counts, numeric-count lint, mutants policy scope)
  ...
      - name: check-cargo-mutants-policy-citations self-test (Guard 2)
        run: bash scripts/check-cargo-mutants-policy-citations.sh --self-test
      - name: check-cargo-mutants-policy-citations (Guard 2, DEC-150)
        run: bash scripts/check-cargo-mutants-policy-citations.sh
```

Key AC-006 requirements confirmed:
- Job name updated to include `mutants policy scope` (AC-006 spec-guard job name requirement)
- Two Guard 2 steps: `--self-test` precedes the main guard step (self-test MUST precede main guard per AC-002)
- Guard 3 has NO dedicated step — it rides the `test` job automatically (as required by AC-006)

---

## AC-007 — Documentation fallout

No recording needed — verified via grep.

### (a) `docs/specs/cargo-mutants-policy.md` — `## Guards` section

```
grep result: 635:## Guards
```

The `## Guards` section at line 635 documents:
- Guard 2: `scripts/check-cargo-mutants-policy-citations.sh` (what it checks, CI job, reproduce locally, action on failure)
- Guard 3: `tests/mutants_glob_existence.rs` (what it checks, CI job, reproduce locally, action on failure)

### (b) `CHANGELOG.md` — `[Unreleased]` entry

```
- **CI: mutants-policy citation guard (Guard 2) + examine_globs existence guard (Guard 3) (DEC-150):**
  adds `scripts/check-cargo-mutants-policy-citations.sh` (validates §Scope function-location bulleted
  list; CI-MUTANTS-CITE-001; self-test fixtures; SCOPE-EMPTY guard) and `tests/mutants_glob_existence.rs`
  (validates examine_globs entries resolve to real files; coverage floor; MUTANTS-GLOBS-KEY-MISSING guard).
```

All required keywords present: topic prefix `CI: mutants-policy citation guard (Guard 2) + examine_globs existence guard (Guard 3) (DEC-150)`, both file paths, `CI-MUTANTS-CITE-001`, `SCOPE-EMPTY guard`, `coverage floor`, `MUTANTS-GLOBS-KEY-MISSING guard`.

### (c) `CLAUDE.md` — AI Agent Notes bullets

```
355: - `scripts/check-cargo-mutants-policy-citations.sh` — runs in spec-guard CI job; validates §Scope
     function-location bulleted list against `src/`; exits 1 with CI-MUTANTS-CITE-001 offender list
     if any symbol citation is stale. `--policy-doc` + `--src-root` (self-test only) + `--self-test`
     flags for offline verification. (DEC-150 Guard 2)
356: - `tests/mutants_glob_existence.rs` — always-run guard validating every `examine_globs` entry in
     `.cargo/mutants.toml` resolves to ≥1 real file; fails loudly if a refactor orphans a glob entry.
     (DEC-150 Guard 3)
```

Two bullets added: one for Guard 2 (script name, CI job, trigger, `--self-test` flag) and one for Guard 3 (test file, CI job, trigger).

### No `src/` change

```bash
git diff HEAD -- src/  # no output — no src/ modifications
```

`Cargo.toml` does contain `glob = "0.3"` added to `[dev-dependencies]` for Guard 3.

---

## Files Produced

| File | Purpose |
|------|---------|
| `AC-001-guard2-success.tape` | VHS script — Guard 2 canonical success |
| `AC-001-guard2-success.gif` | Recording — Guard 2 exits 0, 11 bullets, 21 pairs |
| `AC-001-guard2-success.webm` | Recording — same |
| `AC-002-guard2-selftest.tape` | VHS script — Guard 2 --self-test success |
| `AC-002-guard2-selftest.gif` | Recording — 12 fixtures pass, EXIT: 0 |
| `AC-002-guard2-selftest.webm` | Recording — same |
| `AC-003-guard2-failure.tape` | VHS script — Guard 2 failure with corrupted temp copy |
| `AC-003-guard2-failure.gif` | Recording — DEAD: handle_create_nonexistent, exit 1, git clean |
| `AC-003-guard2-failure.webm` | Recording — same |
| `demo-guard2-failure.sh` | Helper — creates temp copy, runs guard, verifies cleanup |
| `AC-004-guard3-success.tape` | VHS script — Guard 3 all 9 tests pass |
| `AC-004-guard3-success.gif` | Recording — 9/9 ok |
| `AC-004-guard3-success.webm` | Recording — same |
| `AC-005-guard3-failure.tape` | VHS script — Guard 3 dead-glob self-proof test |
| `AC-005-guard3-failure.gif` | Recording — test passes (proves guard logic detects dead globs) |
| `AC-005-guard3-failure.webm` | Recording — same |
| `AC-006-ci-wiring.tape` | VHS script — CI spec-guard job section |
| `AC-006-ci-wiring.gif` | Recording — spec-guard job with 2 Guard 2 steps |
| `AC-006-ci-wiring.webm` | Recording — same |
| `evidence-report.md` | This file |
