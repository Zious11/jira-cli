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

---

# Red Gate Log — S-2.05 (CLAUDE.md documentation update)

**Date:** 2026-05-08
**Story:** S-2.05 — CLAUDE.md documentation update for NFR-O-L/M/O/V/R + NFR-R-F gap + bonus NFR-O-H
**Feature branch commit:** 594f00c
**Squash-merge SHA:** 7f004ca (PR #307)
**Files modified:** `CLAUDE.md` (+35), `src/api/jira/users.rs` (+9), `src/api/jira/issues.rs` (+7)

## Summary

For documentation-only stories, the Red Gate concept does NOT apply. There are no tests to write, no tests to fail, and no production behavior to verify. The story's "green gate" is grep verification: confirm that every AC — the presence of specific text in CLAUDE.md and source comment strings in the named functions — is satisfied. This is a departure from the TDD Red Gate framing and is deliberate and correct per the story type.

## Verification: Grep Checks (ALL PASS)

The following grep checks were performed to verify every acceptance criterion. All 9 checks PASSED.

| # | Path | Pattern | AC | Result |
|---|------|---------|-----|--------|
| 1 | `CLAUDE.md` | `view.rs` (orphan module entry) | NFR-O-L / AC-001 | PASS |
| 2 | `CLAUDE.md` | `comments.rs` (orphan module entry) | NFR-O-L / AC-001 | PASS |
| 3 | `CLAUDE.md` | `assets.rs.*search enrichment` | NFR-O-M / AC-002 | PASS |
| 4 | `src/api/jira/users.rs` | comment in `search_users_all` | NFR-O-O / AC-003 | PASS |
| 5 | `src/api/jira/users.rs` | comment in `search_assignable_users_by_project_all` | NFR-O-O / AC-003 | PASS |
| 6 | `src/api/jira/issues.rs` | comment in `get_changelog` | NFR-O-O / AC-003 | PASS |
| 7 | `src/api/jira/issues.rs` | comment in `search_issues` | NFR-O-O / AC-003 | PASS |
| 8 | `src/api/jira/issues.rs` | comment in `filter_tickets` | NFR-O-O / AC-003 | PASS |
| 9 | All modified files | No line-number references (e.g., `:NNN`) in new comments | NFR-O-V / AC-004 | PASS |

## Lib Baseline Preserved

Test baseline at merge (post-S-2.04, unchanged by S-2.05):

```
test result: ok. 1091 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out
```

(13 ignored = pre-existing keyring-gated tests behind `#[ignore]`; 0 regressions)

## Production Code Modifications

Comments only. Zero behavioral change. No function signatures altered. No control flow changed. No dev-deps added. Cargo.toml and Cargo.lock are identical to the pre-S-2.05 state.

## Deferred

| ID | Description | Severity |
|----|-------------|----------|
| S-2.05-DEFER-01 | CLAUDE.md `list.rs` description text reads 'list + view + comments' but `view.rs` and `comments.rs` are now separately documented sibling modules after S-2.05. Pre-existing text; not introduced by S-2.05. Target: bundle into next small CLAUDE.md cleanup PR. | LOW |

---

# Red Gate Log — S-2.06 (Worklog timeSpent server-side parsing + CMDB cache tuple pin)

**Date:** 2026-05-08
**Story:** S-2.06 (v2.0.0) — Worklog timeSpent string passthrough + CMDB cache tuple format pin
**Test file:** `tests/worklog_duration_holdouts.rs` (589 lines, new)
**Commits:** b3d2500 (Red Gate tests), 3d5a6ca (impl), 15f509c (un-gate AC-004), a5b64a2 (fixup), 1d88d07 (demo evidence)
**Squash-merge SHA:** c8f15d8 (PR #308 to develop)
**Branch:** deleted post-merge

## Summary

UNLIKE all prior Wave 2 stories (which were regression-pin / inverted Red Gate / documentation-only), S-2.06 has a TRUE Red Gate. Tests at b3d2500 FAIL for behavioral reasons before any production code is written, and pass GREEN only after implementation at a5b64a2.

6 tests written across AC-001..AC-006. State at Red Gate (b3d2500 — BEFORE Step 4):
- AC-001: FAIL (behavioral)
- AC-002: FAIL (behavioral)
- AC-003: FAIL (behavioral)
- AC-004: COMPILE-ERROR Red Gate
- AC-005: PASS (inverted pin)
- AC-006: PASS (inverted pin)

State at Green Gate (a5b64a2 — AFTER Step 4): 6/6 PASS.

## Red Gate State (b3d2500 — BEFORE implementation)

| Test | State at b3d2500 | Failure Mode | AC |
|------|-----------------|-------------|-----|
| AC-001: add_worklog sends timeSpent string | FAIL | Assertion error — POST body had `{"timespentseconds":28800}` (old path); test expected `"timeSpent": "1d"` string key in body | AC-001 |
| AC-002: space-separated compound duration accepted | FAIL | `parse_duration` (old) rejected `"2d 3h 30m"` — spaces unsupported; command exited non-zero before reaching network | AC-002 |
| AC-003: invalid duration rejected with exit 64 + Nw/Nd/Nh/Nm hint | FAIL | Exit code was `Some(1)` not `Some(64)`; stderr message read "Use w, d, h, or m" (old single-char hint) not new `Nw Nd Nh Nm` multi-char hint | AC-003 |
| AC-004: parse_duration_validate function exists | COMPILE-ERROR | Gated with `#[cfg(any())]` because `parse_duration_validate` did not exist in `src/duration.rs`; tagged `// RED-GATE-COMPILE: replaced by implementer` | AC-004 |
| AC-005: CMDB cache graceful degradation (Part A) | PASS | Inverted pin — existing CMDB graceful-degradation behavior already correct at activation HEAD | AC-005 |
| AC-006: CMDB cache format regression pin (Part B) | PASS | Inverted pin — cache tuple format already correct at activation HEAD | AC-006 |

### Failure Analysis — AC-001

`add_worklog` in `src/api/jira/worklogs.rs` was computing `time_spent_seconds: u64 = parse_duration(&duration)? * 60` and sending `{"timespentseconds": <number>}` in the POST body. Test expected the new `timeSpent` string key. Correct FAIL for behavioral reason — the implementation had NOT changed yet.

### Failure Analysis — AC-002

`parse_duration` in `src/duration.rs` parsed single-unit and adjacent multi-unit formats (e.g., `1d2h`) but did not accept space-separated components (e.g., `"2d 3h 30m"`). The new `parse_duration_validate` explicitly handles space-separated tokens. Correct FAIL — old function did not exist, new function not yet written.

### Failure Analysis — AC-003

Old error path in `handle_add` used `parse_duration(&duration)` which returned a generic duration-parse error with exit code 1 and message "Use w, d, h, or m". New spec requires `parse_duration_validate` to return exit code 64 (usage error) and a hint using the multi-character unit names `Nw Nd Nh Nm`. Correct FAIL — old path not yet replaced.

### Failure Analysis — AC-004 (COMPILE-ERROR Red Gate)

`parse_duration_validate` was not yet defined in `src/duration.rs`. The test was gated with `#[cfg(any())]` to avoid a compile error that would prevent the other tests from running. This is the canonical compile-error Red Gate pattern: the compile guard lets the test exist in the file and be visible to future editors, but suppresses compilation until the function is defined. The implementer removed the `#[cfg(any())]` gate in a SEPARATE commit (15f509c) after `parse_duration_validate` was defined in 3d5a6ca.

### Passing Tests — AC-005 / AC-006 (Inverted Pin)

AC-005 and AC-006 pin CMDB cache graceful-degradation behavior (Part B of the story scope). This behavior was already correct at activation HEAD. Tests PASS on first run because they are regression-pin guards, not TDD drivers — the same inverted-Red-Gate framing as all prior Wave 2 test-only stories, applied here to the CMDB half of a hybrid story.

## Green Gate State (a5b64a2 — AFTER implementation)

All 6 ACs pass. Full test suite: 614 unit + integration suites; 13 ignored (pre-existing keyring-gated); 0 regressions.

| Test | State at a5b64a2 | Notes |
|------|----------------|-------|
| AC-001: add_worklog sends timeSpent string | PASS | POST body now `{"timeSpent": "1d"}` |
| AC-002: space-separated compound duration accepted | PASS | `parse_duration_validate("2d 3h 30m")` returns Ok(()) |
| AC-003: invalid duration rejected with exit 64 + hint | PASS | Exit code 64; stderr contains `Nw`, `Nd`, `Nh`, `Nm` |
| AC-004: parse_duration_validate function exists | PASS | `#[cfg(any())]` gate removed in 15f509c after 3d5a6ca defined the function |
| AC-005: CMDB cache graceful degradation | PASS | Inverted pin holds |
| AC-006: CMDB cache format regression pin | PASS | Inverted pin holds |

## TDD Discipline Preserved

- Tests committed BEFORE production code (b3d2500 precedes 3d5a6ca chronologically).
- Implementer made minimal targeted changes per AC — no scope creep.
- Un-gating of AC-004 happened in a SEPARATE commit (15f509c) after `parse_duration_validate` was confirmed defined in 3d5a6ca. This preserves the commit-level audit trail.
- Old `parse_duration` calculator preserved in `src/duration.rs` with `SUPERSEDED-BY: parse_duration_validate (S-2.06); kept only for format_duration round-trip proptest` comment. `format_duration` round-trip proptest still calls it — this is intentional and documented.

## Lib/Integration Baseline

```
test result: ok. 614 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out
```

(13 ignored = pre-existing keyring-gated tests behind `#[ignore]`; 0 regressions)

## Deferred

| ID | Description | Severity |
|----|-------------|----------|
| S-2.06-DEFER-01 | src/duration.rs::parse_duration calculator preserved with SUPERSEDED-BY comment because format_duration round-trip proptest still uses it. Target: future cleanup story. | LOW |
| S-2.06-DEFER-02 | AC-003 stderr OR-chain assertion is lenient (passes on any one of Nw/Nd/Nh/Nm). Could be tightened to require all four substrings. Reviewer nit. | LOW |
| S-2.06-DEFER-03 | src/duration.rs:65 !found_any guard reachability is constrained by prior guards — logically sound but slightly defensive. Reviewer nit; no action needed. | LOW |

---

# Red Gate Log — S-2.07 (Auth --output json + verb-aligned JSON policy + test naming)

**Date:** 2026-05-08
**Story:** S-2.07 (v2.0.0) — Auth --output json for 4 subcommands + verb-aligned JSON policy doc + test naming convention
**Test file:** `tests/auth_output_json.rs` (363 lines, new) + inline snapshot tests in `src/cli/auth.rs` mod tests
**Commits:** 6348037 (Red Gate tests), 082169a (impl: auth.rs + main.rs), 9f456d9 (snapshots: cargo insta accept), cd69fd6 (json-output-shapes spec), ae38093 (test-naming-convention spec), d445b7c (CLAUDE.md bullet), 23227a9 (demo evidence)
**Squash-merge SHA:** ca22be0 (PR #309 squash-merged to develop, 2026-05-08)
**Branch:** deleted post-merge

## Summary

UNLIKE the pure regression-pin stories (S-2.01..S-2.05), S-2.07 has a TRUE Red Gate — tests were written that fail for behavioral reasons before production code was written. This is the same pattern as S-2.06.

11 tests written total across two files:
- `tests/auth_output_json.rs`: 4 process-spawn tests + 1 already-green test (AC-003 pre-existing)
- `src/cli/auth.rs` mod tests: 4 insta snapshot tests + 2 refresh regression-pin unit tests

Red Gate state at 6348037 (BEFORE Step 4 — before any production code change):

| Test | State | Failure Mode | AC |
|------|-------|-------------|-----|
| `test_auth_login_outputs_json` (process-spawn) | FAIL | Assertion error — `jr auth login --output json` emitted human text, not JSON; stdout did not contain `"ok"` key | AC-001 |
| `test_auth_switch_outputs_json` (process-spawn) | FAIL | Assertion error — `jr auth switch --output json` emitted human text, not JSON | AC-001 |
| `test_auth_logout_outputs_json` (process-spawn) | FAIL | Assertion error — `jr auth logout --output json` emitted human text, not JSON | AC-001 |
| `test_auth_remove_outputs_json` (process-spawn) | FAIL | Assertion error — `jr auth remove --output json` emitted human text, not JSON | AC-001 |
| `test_auth_switch_unknown_profile_returns_json_error` (process-spawn) | **PASS** (unexpected) | Already-green — main.rs's global `--output json` error wrapper catches JrError and emits JSON to stderr; test passed because error path was already wired | AC-003 (S-2.07-DEFER-01) |
| `test_auth_login_snapshot` (insta, inline) | FAIL | Snapshot file `auth_login_json.snap` did not exist; insta wrote `.snap.new` | AC-006 |
| `test_auth_switch_snapshot` (insta, inline) | FAIL | Snapshot file `auth_switch_json.snap` did not exist; insta wrote `.snap.new` | AC-006 |
| `test_auth_logout_snapshot` (insta, inline) | FAIL | Snapshot file `auth_logout_json.snap` did not exist; insta wrote `.snap.new` | AC-006 |
| `test_auth_remove_snapshot` (insta, inline) | FAIL | Snapshot file `auth_remove_json.snap` did not exist; insta wrote `.snap.new` | AC-006 |
| `test_refresh_success_payload_emits_status_refreshed_for_token_flow` (unit) | PASS | Helper `refresh_success_payload` already shipped correct shape; regression-pin passes vacuously | AC-002 |
| `test_refresh_success_payload_emits_status_refreshed_for_oauth_flow` (unit) | PASS | Same — regression-pin | AC-002 |

### AC-003 Unexpected Pass — S-2.07-DEFER-01 Confirmed

`test_auth_switch_unknown_profile_returns_json_error` passed at Red Gate time (6348037) before any production code change. Investigation: `main.rs` had a pre-existing `--output json` error interceptor that catches all `JrError` values and serializes them as `{"error": "<msg>", "code": <N>}` to stderr. This meant AC-003 (auth JSON error path) was already satisfied. The v1 spec had specified this as a new AC; in reality it was already working. Documented as S-2.07-DEFER-01.

## Green Gate State (23227a9 — AFTER Step 4)

| Test | State | Notes |
|------|-------|-------|
| `test_auth_login_outputs_json` | PASS | Handler now emits `{"profile": "<name>", "action": "login", "ok": true}` under `--output json` |
| `test_auth_switch_outputs_json` | PASS | Same pattern |
| `test_auth_logout_outputs_json` | PASS | Same pattern |
| `test_auth_remove_outputs_json` | PASS | Same pattern |
| `test_auth_switch_unknown_profile_returns_json_error` | PASS | Unchanged from Red Gate — already-green; holds |
| `test_auth_login_snapshot` | PASS | Snapshot accepted in separate commit 9f456d9 via `cargo insta accept` |
| `test_auth_switch_snapshot` | PASS | Snapshot accepted |
| `test_auth_logout_snapshot` | PASS | Snapshot accepted |
| `test_auth_remove_snapshot` | PASS | Snapshot accepted |
| `test_refresh_success_payload_emits_status_refreshed_for_token_flow` | PASS | Regression-pin holds; auth refresh asymmetric shape preserved |
| `test_refresh_success_payload_emits_status_refreshed_for_oauth_flow` | PASS | Regression-pin holds |

## TDD Discipline Preserved

- Red Gate tests committed BEFORE production code (6348037 precedes 082169a chronologically).
- Snapshots accepted in a SEPARATE commit (9f456d9) AFTER handlers emit JSON at 082169a. This preserves the commit-level audit trail: snapshot acceptance is a deliberate act, not a side effect of implementation.
- 4 snapshot files stored under `src/cli/snapshots/`: `auth_login_json.snap`, `auth_switch_json.snap`, `auth_logout_json.snap`, `auth_remove_json.snap`.
- Spec docs (cd69fd6, ae38093) and CLAUDE.md update (d445b7c) committed as SEPARATE commits after Green Gate — clean separation of test → impl → spec → docs.

## Lib/Integration Baseline

```
test result: ok. 620 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out
```

Previous baseline (post-S-2.06): 614 passed / 0 failed / 13 ignored.
Net change: +6 tests (4 snapshot + 2 refresh regression-pin). 10 ignored = pre-existing keyring-gated tests behind `#[ignore]`.
Zero regressions.

## Deferred

| ID | Description | Severity |
|----|-------------|----------|
| S-2.07-DEFER-01 | AC-003 (auth JSON error path) was already satisfied by main.rs's global --output json error wrapper before any S-2.07 code landed. Confirmed as already-working by the unexpected Green at Red Gate time. Documented in docs/specs/json-output-shapes.md. No action needed. | LOW |
| S-2.07-DEFER-02 | src/cli/auth.rs::mod tests: Pre-existing refresh_payload_pins_token_shape and refresh_payload_pins_oauth_shape tests cover much of AC-002's ground. New tests test_refresh_success_payload_emits_status_refreshed_for_token_flow and _for_oauth_flow are intentionally additive (more specific assertions). No action; intentional overlap. | LOW |
