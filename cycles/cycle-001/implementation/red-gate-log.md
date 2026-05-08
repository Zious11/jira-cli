# Red Gate Log — S-1.03 (NFR-O-A)

**Date:** 2026-05-07
**Story:** S-1.03 — Add tracing crate + wire structured logging to client.rs and auth.rs
**Test file:** `tests/observability.rs`
**Branch:** `feat/S-1.03-tracing-observability`

## Summary

10 tests written. 7 FAIL (source-inspection Red Gate). 3 PASS (vacuous guards
correct pre-fix; structural post-fix guards). Red Gate verified.

## Approach

Option A (source-grep): all tests read source files via `std::fs::read_to_string`.
No `tracing` types are imported into the test file. The test binary compiles today
without the tracing dep. This is the correct approach for infrastructure stories
where the dep is not yet present.

## Test Results (pre-fix)

```
running 10 tests
test test_s_1_03_lib_rs_does_not_init_subscriber ... ok
test test_s_1_03_main_initializes_tracing_subscriber ... FAILED
test test_s_1_03_observability_rs_does_not_init_subscriber ... ok
test test_s_1_03_cargo_toml_has_tracing_dep ... FAILED
test test_s_1_03_cargo_toml_has_tracing_subscriber_dep ... FAILED
test test_s_1_03_main_uses_env_filter ... FAILED
test test_s_1_03_client_uses_tracing_debug ... FAILED
test test_s_1_03_auth_has_tracing_entry_points ... FAILED
test test_s_1_03_client_no_verbose_request_eprintln ... FAILED
test test_s_1_03_auth_no_client_secret_in_tracing_fields ... ok

test result: FAILED. 3 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out
```

## Failure Analysis

| Test | Failure Mode | AC |
|------|-------------|-----|
| `test_s_1_03_cargo_toml_has_tracing_dep` | `tracing = ` not found in Cargo.toml [dependencies] | AC-001 |
| `test_s_1_03_cargo_toml_has_tracing_subscriber_dep` | `tracing-subscriber` not found in Cargo.toml; `env-filter` feature absent | AC-001 |
| `test_s_1_03_main_initializes_tracing_subscriber` | `tracing_subscriber::` not found in src/main.rs | AC-002 |
| `test_s_1_03_main_uses_env_filter` | `EnvFilter`, `with_max_level`, `with_env_filter` all absent from main.rs | AC-002 |
| `test_s_1_03_client_uses_tracing_debug` | `tracing::debug!` and `tracing::trace!` absent from client.rs | AC-003 |
| `test_s_1_03_client_no_verbose_request_eprintln` | 2 `eprintln!("[verbose]` with method/rate-limit content still present in client.rs | AC-003 |
| `test_s_1_03_auth_has_tracing_entry_points` | `tracing::info!` / `tracing::debug!` absent from auth.rs | AC-005 |

## Vacuous Pass Analysis

| Test | Why it passes pre-fix | Structural purpose |
|------|----------------------|--------------------|
| `test_s_1_03_lib_rs_does_not_init_subscriber` | lib.rs has no tracing at all | Guards against double-init if implementer adds subscriber to lib |
| `test_s_1_03_auth_no_client_secret_in_tracing_fields` | auth.rs has no tracing calls to contain secret leaks | Guards against secret-leak post-fix |
| `test_s_1_03_observability_rs_does_not_init_subscriber` | observability.rs has no subscriber init | Guards against subscriber creeping into the helper module |

## AC-004 Delegation

SD-003 regression (AC-004) is fully verified by `tests/verbose_bodies.rs` (6 tests
from S-0.06). No new tests are written here for AC-004 — the existing suite
enforces the contract. The implementer must not break those 6 tests.

## Lib Baseline

```
test result: ok. 600 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out
```

Baseline preserved. Zero regressions.

## Hand-off to Implementer

All 7 behavioral tests fail for the right reason (assertion errors on absent source
patterns, not build errors). To make each test pass:

1. Add to `Cargo.toml` [dependencies]:
   ```toml
   tracing = "0.1"
   tracing-subscriber = { version = "0.3", features = ["env-filter"] }
   ```
2. Add subscriber init to `src/main.rs` after CLI parse, before dispatch:
   ```rust
   let log_level = if cli.verbose_bodies { tracing::Level::TRACE }
                   else if cli.verbose { tracing::Level::DEBUG }
                   else { tracing::Level::WARN };
   tracing_subscriber::fmt().with_max_level(log_level).with_writer(std::io::stderr).init();
   ```
3. Replace `eprintln!("[verbose] {} {}", r.method(), r.url())` and the rate-limit
   eprintln! in `src/api/client.rs` with `tracing::debug!` structured events.
4. Add `tracing::info!` or `tracing::debug!` at `oauth_login`, token exchange,
   and `refresh_oauth_token` entry points in `src/api/auth.rs`.
5. Do NOT add `.init()` to `src/lib.rs` or `src/observability.rs`.
6. Do NOT log `client_secret`, `access_token` values, or `refresh_token` values
   in tracing field lists in `src/api/auth.rs`.

Post-fix, `cargo test --all-features` must pass including the 6 verbose_bodies tests.

---

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

---

# Red Gate Log — S-2.03 (BC-4 assets/CMDB holdout suite)

**Date:** 2026-05-08
**Story:** S-2.03 — BC-4 assets/CMDB regression holdout suite (H-037, H-038, H-039)
**Test file:** `tests/asset_holdouts.rs` (417 lines)
**Commits:** dd5c41f (tests), 212a237 (demo evidence)
**Squash-merge SHA:** e9c2ba8 (PR #305 to develop)
**Branch:** `test/S-2.03-bc-4-asset-enrichment-holdout-suite` (deleted post-merge)

## Inverted Red Gate (Regression-Pin Pattern)

S-2.03 is a regression-pin holdout story. The Red Gate discipline is intentionally
inverted: the 3 tests are written against EXISTING CORRECT behavior at activation
HEAD dea1664. Tests PASS on first run because they pin production behavior that is
already present and correct — not because the tests are vacuous.

This is not a discipline violation. The purpose of the Red Gate is to ensure tests
fail for the right reason before implementation. For regression-pin holdouts, "the
right reason to pass" IS the point: the test is a behavioral contract pin, not a
TDD driver. Recording the explicit framing here so future reviewers understand the
distinction.

Lib baseline at test-write time:

```
test result: ok. 614 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

(~550 integration tests also green; 0 regressions)

## Tests Written

| Test Name | Holdout | BC Anchor | Pass/Fail at Write |
|-----------|---------|-----------|-------------------|
| `test_h037_assets_search_returns_structured_data` | H-037 | BC-4.2.001 (asset search returns structured AQL results) | PASS (pins existing correct behavior) |
| `test_h038_enrich_assets_adds_object_attributes` | H-038 | BC-4.3.002 (enrich_assets join_all concurrency + attribute merge) | PASS (pins existing correct behavior) |
| `test_h039_assets_schema_type_filters_correctly` | H-039 | BC-4.2.006 (--type filter narrows schema objects) | PASS (pins existing correct behavior) |

## H-038 Placement Rationale

H-038 pins `enrich_assets` (BC-4.3.002). `enrich_assets` is declared `pub` in
`src/cli/assets.rs` and is accessible from a library-level integration test because
`src/lib.rs` exports `pub mod api` which re-exports the function via the module
chain. Placement in `tests/asset_holdouts.rs` as a library-level test is the
correct approach — no access workaround was needed.

## Production Code Modifications

NONE. S-2.03 is a test-only delivery. All 3 source files (src/cli/assets.rs,
src/api/assets/linked.rs, src/api/assets/objects.rs) are untouched.

## Deferred

S-2.03-DOC-01 (LOW): Story spec line ~123 names workspace cache file
`workspace_id.json`. Actual filename per `src/cache.rs` and tests is `workspace.json`.
Tests use the correct filename. Story spec text needs a follow-up doc PR — not a
production correctness issue.

---

# Red Gate Log — S-2.04 (BC-5/7 boards, sprints, and ADF rendering holdout suite)

**Date:** 2026-05-08
**Story:** S-2.04 — BC-5/7 boards, sprints, and ADF rendering regression holdout suite (H-040..H-044)
**Test file:** `tests/boards_sprints_holdouts.rs` (770 lines)
**Commits:** e71a61e (tests), 893d45a (demo evidence)
**Squash-merge SHA:** ada9126 (PR #306 to develop)
**Branch:** `test/S-2.04-bc-5-boards-sprints-holdout-suite` (deleted post-merge)

## Inverted Red Gate (Regression-Pin Pattern)

S-2.04 is a regression-pin holdout story. The Red Gate discipline is intentionally
inverted: the 9 tests are written against EXISTING CORRECT behavior at activation
HEAD e9c2ba8/ada9126. Tests PASS on first run because they pin production behavior
that is already present and correct — not because the tests are vacuous.

This follows the same inverted-Red-Gate framing as S-2.03. The purpose of the Red
Gate for regression-pin holdouts is to ensure the test is a behavioral contract pin,
not a TDD driver. Recording the explicit framing here so future reviewers understand
the distinction.

Lib baseline at test-write time:

```
test result: ok. 1091 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out
```

(13 ignored = pre-existing keyring-gated tests behind `#[ignore]`; 0 regressions)

## Tests Written

| Test Name | Holdout | BC Anchor | Pass/Fail at Write |
|-----------|---------|-----------|-------------------|
| `test_h040_board_list_returns_paged_boards` | H-040 (case 1/3) | BC-5.2.001 (board list returns structured paginated results) | PASS (pins existing correct behavior) |
| `test_h040_board_list_json_output` | H-040 (case 2/3) | BC-5.2.001 | PASS (pins existing correct behavior) |
| `test_h040_board_list_name_filter` | H-040 (case 3/3) | BC-5.2.005 (board list --name filter narrows results) | PASS (pins existing correct behavior) |
| `test_h041_board_view_shows_sprint_state` | H-041 | BC-5.2.007 (board view shows active sprint state) | PASS (pins existing correct behavior) |
| `test_h041_board_view_kanban_no_sprint_field` | H-041 | BC-5.2.008 (kanban board view omits sprint field) | PASS (pins existing correct behavior) |
| `test_h042_sprint_list_scrum_board` | H-042 | BC-5.3.001 (sprint list returns sprints for scrum board) | PASS (pins existing correct behavior) |
| `test_h043_sprint_current_shows_team_and_points` | H-043 (case 1/2) | BC-5.3.002 (sprint current shows team + points columns) | PASS (pins existing correct behavior) |
| `test_h043_kanban_board_sprint_error` | H-043 (case 2/2) | BC-5.3.002 / AC-004 (kanban boards reject sprint commands) | PASS (pins existing correct behavior) |
| `test_h044_adf_rendering` | H-044 | BC-7.2.001 (ADF→text rendering produces readable output) | PASS (pins existing correct behavior) |

## Production Code Modifications

NONE. S-2.04 is a test-only delivery. No source files under `src/` were touched.
No dev-deps were added.

## Test Placement Rationale

All 9 tests are integration-level in `tests/boards_sprints_holdouts.rs`. They drive
the `jr` binary via `assert_cmd` process-spawn rather than calling internal functions
directly. This is the correct placement because all tested code paths — board list/view,
sprint list/current, ADF rendering — are reachable through the binary's public CLI
surface. No inline `#[cfg(test)]` mod was added in `src/` because no library-internal
function access was required (contrast with S-2.03 H-038 which needed library-level
access to `enrich_assets`).

## Deferred

| ID | Description | Severity |
|----|-------------|----------|
| S-2.04-DEFER-01 | Story spec AC-004 quotes kanban literal prefix only; production code emits prefix + suffix '. Board {id} is a {type} board.'. Test uses contains(prefix) — correct and robust. Update spec text in follow-up doc PR. | LOW |
| S-2.04-DEFER-02 | Story spec H-043 implementation notes use 'displayName'; CachedTeam struct uses 'name'. Test uses production struct directly. Update spec text in follow-up doc PR. | LOW |
| S-2.04-DOC-01 | Pre-existing: tests/team_column_parity.rs::write_team_cache writes to non-canonical XDG path (missing v1/default/). Not introduced by S-2.04. Target: separate fix story. | LOW |
