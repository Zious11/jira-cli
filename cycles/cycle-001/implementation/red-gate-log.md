# Red Gate Log — S-0.05 (SD-002)

**Date:** 2026-05-07
**Story:** S-0.05 — Gate `JR_AUTH_HEADER` env var read behind `#[cfg(test)]`
**Test file:** `tests/auth_header_release_gate.rs`
**Branch:** `feat/jr-auth-header-cfg-test-gate`

## Summary

5 tests written. 1 FAILS (source-inspection Red Gate). 4 PASS (regression guards
and audit). Red Gate verified.

## Test Results (pre-fix)

```
running 5 tests
test test_sd_002_cfg_test_is_active_in_test_binary ... ok
test test_sd_002_cfg_test_gate_present_in_source ... FAILED
test test_sd_002_new_for_test_signature_unchanged ... ok
test test_sd_002_new_for_test_honors_auth_header ... ok
test test_sd_002_ac004_audit_no_in_process_jr_auth_header_readers ... ok

test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

## Failure Analysis

| Test | Failure Mode | AC |
|------|-------------|-----|
| `test_sd_002_cfg_test_gate_present_in_source` | `#[cfg(test)]` not found within 5 lines of `JR_AUTH_HEADER` env-var read at line 65 of `src/api/client.rs`. The gate annotation is absent — correct Red state. | AC-002 |

## Passing Tests

| Test | Why it passes pre-fix | AC |
|------|----------------------|----|
| `test_sd_002_cfg_test_is_active_in_test_binary` | `cfg!(test)` is always true in any test binary | AC-002 in-process |
| `test_sd_002_new_for_test_honors_auth_header` | `new_for_test` takes `auth_header` as constructor arg; unaffected by env-var gate | AC-001 |
| `test_sd_002_new_for_test_signature_unchanged` | `new_for_test` signature is currently correct | AC-003 |
| `test_sd_002_ac004_audit_no_in_process_jr_auth_header_readers` | All `JR_AUTH_HEADER` references in `tests/` are subprocess `.env()` calls, not in-process `env::var()` reads | AC-004 |

## AC-004 Audit Finding

All `JR_AUTH_HEADER` references in `tests/` (100+ occurrences across ~20 files)
are subprocess invocations via `Command::cargo_bin("jr").env("JR_AUTH_HEADER", ...)`.
The `jr` subprocess binary is compiled WITHOUT `cfg(test)` active. After the
`#[cfg(test)]` gate lands in `from_config`, those subprocess tests will no longer
be able to use `JR_AUTH_HEADER` for auth bypass. Migration is deferred to holdout
H-NEW-AUTH-002, formalized in S-0.07.

Zero in-process `env::var("JR_AUTH_HEADER")` calls exist in `tests/` — no
immediate migration is required for S-0.05.

The one non-subprocess reference (`migration_legacy.rs:35`) scrubs `JR_AUTH_HEADER`
from the environment before calling `Config::load` in-process. That scrub is safe
and correct in all build modes.

## Lib Baseline

```
test result: ok. 600 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out
```

Baseline preserved. Zero regressions.

## Hand-off to Implementer

One test fails for the right reason (assertion error: source file lacks the
`#[cfg(test)]` gate). Wrap `src/api/client.rs:64-66` env-var read block in
`#[cfg(test)]` per SD-002 Option A resolution. Post-fix, all 5 tests must pass.

Fix pattern: wrap the `if let Ok(header) = std::env::var("JR_AUTH_HEADER")` block
at line 64-66 with `#[cfg(test)]` so it is excluded from release builds. Extract
the keychain lookup into a helper to avoid duplication (optional but recommended
per story notes).

**Important:** the subprocess integration tests in `tests/` that pass
`JR_AUTH_HEADER` via `.env()` to the `jr` binary will break post-fix.
That is expected — it is the S-0.07 holdout. Do not "fix" those tests as part
of S-0.05.

---

# Red Gate Log — S-0.04 (BC-6.3.001)

**Date:** 2026-05-07
**Story:** S-0.04 — Migrate 14 field-read sites to `config.active_profile()`
**Test file:** `tests/multi_profile_fields.rs`
**Branch:** `fix/multi-profile-fields-active`

## Summary

8 tests written. 6 FAIL (behavioral — correct Red state). 2 PASS (unit contract
tests that verify `Config::active_profile()` itself is correct). Red Gate verified.

## Test Results (pre-fix)

```
running 8 tests
test test_bc_6_3_001_active_profile_returns_per_profile_field_ids ... ok
test test_bc_6_3_001_field_ids_survive_toml_save_round_trip ... ok
test test_bc_6_3_001_sandbox_profile_uses_sandbox_story_points_field_id ... FAILED
test test_bc_6_3_001_error_message_references_profiles_section_not_fields ... FAILED
test test_bc_6_3_001_points_column_present_after_save_round_trip ... FAILED
test test_bc_6_3_001_list_points_warning_references_profiles_section ... FAILED
test test_bc_6_3_001_board_view_shows_team_after_save_round_trip ... FAILED
test test_bc_6_3_001_sprint_current_shows_team_and_points_after_save_round_trip ... FAILED

test result: FAILED. 2 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
```

## Failure Analysis

| Test | Failure Mode | BC Clause |
|------|-------------|-----------|
| `test_bc_6_3_001_sandbox_profile_uses_sandbox_story_points_field_id` | Process exits non-zero: "Story points field not configured" — code reads `config.global.fields` (empty post-round-trip) instead of sandbox profile's `customfield_10099` | BC-6.3.001 postcondition / H-NEW-MP-001 |
| `test_bc_6_3_001_points_column_present_after_save_round_trip` | Points column absent from table output because `config.global.fields.story_points_field_id` is `None` | BC-6.3.001 postcondition (AC-002) |
| `test_bc_6_3_001_sprint_current_shows_team_and_points_after_save_round_trip` | Neither Points nor Team column present — both field IDs read from `config.global.fields` (None) | BC-6.3.001 postcondition (AC-003) |
| `test_bc_6_3_001_board_view_shows_team_after_save_round_trip` | Team column absent — `config.global.fields.team_field_id` is `None` | BC-6.3.001 postcondition (AC-004) |
| `test_bc_6_3_001_error_message_references_profiles_section_not_fields` | Error says `[fields]` not `[profiles.<name>]` | BC-6.3.001 postcondition (AC-005) |
| `test_bc_6_3_001_list_points_warning_references_profiles_section` | Warning says `[fields].story_points_field_id` not `[profiles.<name>]` | BC-6.3.001 postcondition (AC-006) |

## Passing Tests

| Test | Why it passes | Notes |
|------|--------------|-------|
| `test_bc_6_3_001_active_profile_returns_per_profile_field_ids` | `Config::active_profile()` is already correct | Documents the contract; confirms `active_profile()` returns per-profile data |
| `test_bc_6_3_001_field_ids_survive_toml_save_round_trip` | TOML serialization/deserialization of `GlobalConfig` preserves `[profiles.*]` field IDs | Confirms the data survives round-trip at the struct level |

## Lib Baseline

```
test result: ok. 600 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out
```

Baseline preserved. Zero regressions.

## Hand-off to Implementer

All 6 behavioral tests fail for correct reasons (assertion errors, not build
errors). Fix the 14 call sites listed in S-0.04 and update the 2 error message
strings to make each test pass. The two unit tests must continue to pass.

Fix pattern: `config.global.fields.X` → `config.active_profile().X`
