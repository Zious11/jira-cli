---
story_id: S-383
cycle: cycle-001
red_gate_date: 2026-05-19
verified_by: orchestrator (independent re-run of tests in worktree after test-writer commit `5dc53a5`)
verdict: PASSED
---

# S-383 Red Gate Log

## Context

S-383 implements BC-3.8.012 + BC-3.8.013 — inverse-direction stderr warnings on `jr issue create` platform path when `--field` / `--on-behalf-of` are passed without `--request-type`. Quick-dev route (no F2/F3 architecture work; just additive BC delta), F2 converged 11 passes (last 2 with ZERO findings, ZERO novelty).

## Step 2 (stubs) skipped — pure-addition story

Unlike most stories, S-383 introduces no new symbols / types / modules / functions. The change is 2 additional `eprintln!` calls inside an existing `handle_create` function. The existing `src/cli/issue/create.rs` already compiles cleanly, so the "compilable stubs" intermediate state from the deliver-story workflow does not apply.

Decision: SKIP Step 2 explicitly. Document in this log. Proceed directly to Step 3.

This parallels how S-382 handled the analogous case (the stub state for S-382 was "variant signature widened but Display template unchanged" — a stub-equivalent that maintained correct compilation but produced wrong observable behavior).

## Tests added in Step 3 (commit `5dc53a5`)

Test file: `tests/issue_create_jsm.rs` (extending existing file; awkwardness acknowledged in story's Test File Decision section).

7 tests added covering 7 ACs:

| Test | AC | BC |
|------|-----|-----|
| `test_platform_create_field_flag_emits_warning_without_request_type` | AC-1 | BC-3.8.012 |
| `test_platform_create_on_behalf_of_flag_emits_warning_without_request_type` | AC-2 | BC-3.8.013 |
| `test_platform_create_both_inverse_flags_emit_independent_warnings` | AC-3 | both |
| `test_platform_create_without_inverse_flags_emits_no_new_warnings` | AC-4 | negative guard |
| `test_platform_create_field_idempotent_one_warning_per_logical_flag` | AC-5 | BC-3.8.012 idempotency |
| `test_jsm_create_with_field_and_request_type_does_not_fire_bc_3_8_012` | AC-6 | forward-path regression |
| `test_platform_create_malformed_field_one_warning_no_exit_64` | AC-7 | BC-3.8.012 malformed-field edge |

Shared helper: `mount_platform_create_stubs` (async wiremock setup for AC-1/2/3/4/5/7).

## Red Gate verification results

Command: `cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-383 && cargo test --test issue_create_jsm test_platform_create` (then a second run for the JSM regression test).

| Test | Result | Notes |
|------|--------|-------|
| AC-1 `test_platform_create_field_flag_emits_warning_without_request_type` | **FAIL** | Assertion error at `tests/issue_create_jsm.rs:2365` — "BC-3.8.012 / AC-1: verbatim warning must appear on stderr; got: [no warning emitted]" |
| AC-2 `test_platform_create_on_behalf_of_flag_emits_warning_without_request_type` | **FAIL** | Assertion at `tests/issue_create_jsm.rs:2435` — same pattern, BC-3.8.013 verbatim missing |
| AC-3 `test_platform_create_both_inverse_flags_emit_independent_warnings` | **FAIL** | Assertion at `tests/issue_create_jsm.rs:2505` — BC-3.8.012 warning missing |
| AC-4 `test_platform_create_without_inverse_flags_emits_no_new_warnings` | PASS | Negative guard — asserts warnings ABSENT; correct both pre and post implementation |
| AC-5 `test_platform_create_field_idempotent_one_warning_per_logical_flag` | **FAIL** | Count assertion at `tests/issue_create_jsm.rs:2619` — `left: 0` `right: 1` (warning count 0, expected 1) |
| AC-6 `test_jsm_create_with_field_and_request_type_does_not_fire_bc_3_8_012` | PASS | Negative regression gate — asserts BC-3.8.012 absent on JSM path; correct both sides |
| AC-7 `test_platform_create_malformed_field_one_warning_no_exit_64` | **FAIL** | Count assertion at `tests/issue_create_jsm.rs:2741` — same `0 vs 1` pattern |

Final cargo test summary: `test result: FAILED. 2 passed; 5 failed; 0 ignored; 0 measured; 29 filtered out`.

## Red Gate Outcome: **PASSED**

- 5 NEW tests FAIL with assertion errors (NOT panic, NOT build error)
- Failure messages reference behavior under test ("verbatim warning must appear on stderr" / "warning must appear EXACTLY ONCE")
- 2 NEW tests PASS — both correctly designed as negative guards (AC-4: absent flag → no warning; AC-6: JSM path → no platform warning)
- Existing 29 tests in `issue_create_jsm.rs` filtered out and unaffected
- All tests compile (no build errors)

Step 4 (TDD implementation) authorized: implementer must add 2 `eprintln!` warnings in `src/cli/issue/create.rs::handle_create` (around line 119, BEFORE the existing `if request_type.is_some()` block) to satisfy AC-1/2/3/5/7. AC-4/6 must remain passing.

## Compliance Notes

- IRON LAW satisfied: "NO IMPLEMENTATION WITHOUT RED GATE VERIFICATION FIRST" — verified by orchestrator before authorizing Step 4.
- Step 2 explicitly skipped per pure-addition-story rationale (documented above).
- Test naming convention applied (`test_<verb>_<subject>_<expected_outcome>`).
- Two-part assertions where appropriate (positive + negative for AC-3 / AC-6).
- No `||` / `.or_else()` / accept-either patterns (L-288-pr2-02 compliant).
