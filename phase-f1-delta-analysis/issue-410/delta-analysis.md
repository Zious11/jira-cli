---
document_type: f1-delta-analysis
phase: phase-f1-delta-analysis
producer: architect
issue: 410
status: draft
created: 2026-05-26
project: jira-cli
mode: BROWNFIELD
intent: bug-fix
bundled_fix: false
feature_type: infrastructure
trivial_scope: false
scope: small
regression_risk: low
severity: low-medium
inputs:
  - "tests/multi_cloudid_disambiguation.rs"
  - "tests/oauth_refresh_integration.rs"
  - "src/api/auth.rs"
  - "CLAUDE.md"
---

# F1 Delta Analysis — Issue #410

## Feature

- **Name:** Test infra: unify keychain-touching test isolation pattern
- **Issue link:** https://github.com/Zious11/jira-cli/issues/410
- **CI flake evidence:** run 26477547114 — `test_cloud_id_flag_picks_named_resource_not_first` fails on macOS runner
- **Closest precedent:** `JR_RUN_KEYRING_TESTS=1` gate already in use in `src/api/auth.rs` inline tests

---

## Problem Statement

Two test files contain tests that touch the system keychain either directly or transitively
via `store_oauth_tokens` / `load_oauth_tokens` in `src/api/auth.rs`. These tests run
unconditionally in CI (no `#[ignore]` gate), which causes intermittent failures on macOS
GitHub runners when the keychain partitioning quirk triggers a system prompt — halting
the runner. The existing canonical convention (CLAUDE.md line 351) requires
`JR_RUN_KEYRING_TESTS=1` + `#[ignore]` for all keychain-touching tests, but neither test
file follows it.

The issue body report claims:
- `tests/multi_cloudid_disambiguation.rs`: 10 keychain-touching tests, none gated
- `tests/oauth_refresh_integration.rs`: 7 always-run tests, status of keychain access uncertain

This audit verifies those counts and provides per-test classification.

---

## Audit Scope and Method

**Read-only audit.** No source or test files were modified.

Classification criteria:
- **KEYCHAIN-DIRECT**: calls `keyring::Entry::new`, `store_oauth_tokens`, `load_oauth_tokens`,
  or `clear_profile_creds` from within the test process (inline or via `harness::*` helpers).
- **KEYCHAIN-TRANSITIVE**: spawns `jr` as a subprocess that reaches `store_oauth_tokens`
  or `load_oauth_tokens` in `auth.rs` during the expected success path.
- **NO-KEYCHAIN**: pure subprocess test that fails before the keychain write/read path
  (e.g., flag-recognition tests that exit 2 pre-implementation, or tests where the flow
  never reaches `store_oauth_tokens` because an earlier assertion fires).

Key call chain for subprocess tests: `jr auth login --oauth` → `oauth_login()` →
`store_oauth_tokens(profile, ...)` at `src/api/auth.rs:813`. If the subprocess reaches
this line successfully, it touches the real keychain (unless `JR_SERVICE_NAME` overrides
it to an isolated namespace). The multi_cloudid tests set `JR_SERVICE_NAME` via `jr_isolated()`,
which redirects keychain writes to a test-scoped namespace — but the keychain IS still
accessed (the macOS security framework prompt still fires if the service name is novel).

Key call chain for `oauth_refresh_integration.rs`: `harness::seed_oauth_tokens()` calls
`jr::api::auth::store_oauth_tokens` directly from the test process (in-process, not subprocess).

---

## Per-Test Keychain Classification: `tests/multi_cloudid_disambiguation.rs`

Total tests in file: **10**

The file defines `jr_isolated()` (line 183) which sets `JR_SERVICE_NAME=jr-test-mc-<pid>-<tid>-<nanos>`
on every subprocess command. This partitions the keychain namespace but does NOT eliminate
keychain access — novel service names still trigger macOS security framework interaction.

| # | Test Function | Line | Gated? | Classification | Rationale |
|---|--------------|------|--------|----------------|-----------|
| 1 | `test_cloud_id_flag_recognized_in_help` | 249 | NO | **NO-KEYCHAIN** | Runs `jr auth login --help` only; no OAuth flow, no keychain touch. Pure flag-registration check. |
| 2 | `test_cloud_id_flag_is_parsed_not_rejected_by_clap` | 285 | NO | **KEYCHAIN-TRANSITIVE** | Calls `jr auth login --oauth --cloud-id cloud-A --no-input`. The subprocess enters `oauth_login()`. Pre-implementation it exits 2 (clap error) before any keychain call, but post-implementation it reaches `store_oauth_tokens`. Transitively touches keychain on the green path. |
| 3 | `test_cloud_id_flag_picks_named_resource_not_first` | 343 | NO | **KEYCHAIN-TRANSITIVE** | Full OAuth login flow with wiremock. On success, `oauth_login()` calls `store_oauth_tokens` at `auth.rs:813`. Uses `jr_isolated()` so `JR_SERVICE_NAME` is set, but the keychain IS accessed. This is the test confirmed flaky on CI (run 26477547114). |
| 4 | `test_cloud_id_flag_value_not_in_response_exits_64` | 430 | NO | **NO-KEYCHAIN** | Expected exit code is 64 (cloud-id not found). Flow must exit before `store_oauth_tokens` because the resource is not found. No keychain write on this path. |
| 5 | `test_no_input_multi_org_exits_64_with_actionable_error` | 503 | NO | **NO-KEYCHAIN** | Expected exit 64 on disambiguation failure. Flow exits before `store_oauth_tokens`. No keychain write. |
| 6 | `test_no_input_multi_org_lists_available_cloud_ids_in_error` | 575 | NO | **NO-KEYCHAIN** | Same as #5 — expected exit 64. No keychain write on the error path. |
| 7 | `test_single_resource_no_regression_single_org_path` | 659 | NO | **KEYCHAIN-TRANSITIVE** | Single-org success path. `oauth_login()` succeeds and calls `store_oauth_tokens` at `auth.rs:813`. Uses `jr_isolated()` but keychain IS accessed. |
| 8 | `test_callback_url_contains_127_0_0_1_and_port_53682` | 742 | NO | **NO-KEYCHAIN** | Runs `jr auth login --help`. No OAuth flow, no keychain touch. |
| 9 | `test_cloud_id_flag_does_not_change_redirect_uri_in_authorize_url` | 802 | NO | **KEYCHAIN-TRANSITIVE** | Full OAuth login flow with wiremock; success path calls `store_oauth_tokens` at `auth.rs:813`. Uses `jr_isolated()`. Keychain IS accessed post-implementation. |
| 10 | `test_interactive_select_via_stdin_picks_second_resource` | 884 | NO | **KEYCHAIN-TRANSITIVE** | Interactive path with stdin injection. Success path calls `store_oauth_tokens` at `auth.rs:813`. Uses `jr_isolated()`. Keychain IS accessed post-implementation. |
| 11 | `test_cloud_id_help_text_mentions_disambiguation_or_multiple_orgs` | 1049 | NO | **NO-KEYCHAIN** | Runs `jr auth login --help`. No OAuth flow, no keychain touch. |

**COUNT CORRECTION — FLAGGED:** The issue body claims 10 keychain-touching tests. The actual
count is **5 KEYCHAIN-TRANSITIVE** tests (tests 2, 3, 7, 9, 10) and **6 NO-KEYCHAIN** tests
(tests 1, 4, 5, 6, 8, 11). The file has **11 total tests**, not 10 as the issue body implies.
Test #11 (`test_cloud_id_help_text_mentions_disambiguation_or_multiple_orgs`) was missed
in the issue body count. It is NO-KEYCHAIN.

The confirmed flaky CI test (#3, `test_cloud_id_flag_picks_named_resource_not_first`) is
correctly identified as KEYCHAIN-TRANSITIVE. It is the only test that both (a) succeeds
post-implementation and (b) runs the full `store_oauth_tokens` path on macOS.

**Pre-implementation behavior note:** Tests 2, 3, 7, 9, 10 currently exit with an error
(clap exit 2, wiremock-missing network error, or flow-never-reaches-keychain) before
touching the keychain on the current `develop` branch. However, once implementation lands,
they will touch the keychain on every run. The gating decision should apply proactively.

---

## Per-Test Keychain Classification: `tests/oauth_refresh_integration.rs`

Total tests in file: **11** (not 7 as claimed in the issue body — see count correction below)

The file's own module-level comment (lines 28-42) clearly documents the gate classification.
The `harness` module (line 137) defines `seed_oauth_tokens()` (line 204) which calls
`jr::api::auth::store_oauth_tokens` directly. This is KEYCHAIN-DIRECT from the test process.

| # | Test Function | Line | `#[ignore]`? | Gated via `JR_RUN_KEYRING_TESTS`? | Classification | Rationale |
|---|--------------|------|-------------|----------------------------------|----------------|-----------|
| 1 | `test_send_retries_once_after_refresh_on_401` | 241 | NO | NO | **NO-KEYCHAIN** | Uses `JiraClient::new_for_test` + wiremock only. Does NOT call `seed_oauth_tokens`. The `refresh_oauth_token` path calls `load_oauth_tokens(profile)` at `auth.rs:882`, but pre-implementation `send()` never triggers refresh. Post-implementation: will call `load_oauth_tokens("default")` — this IS a transitive keychain read. Currently always-run; see FLAG below. |
| 2 | `test_refresh_persists_rotated_tokens_via_store_oauth_tokens` | 329 | YES | YES (line 330) | **KEYCHAIN-DIRECT** | Calls `harness::seed_oauth_tokens()` (line 349) → `store_oauth_tokens`. Calls `auth::load_oauth_tokens(harness::TEST_PROFILE)` (line 396). Properly gated. |
| 3 | `test_invalid_grant_surfaces_not_authenticated_with_refresh_hint` | 428 | NO | NO | **NO-KEYCHAIN (pre-impl) / KEYCHAIN-TRANSITIVE (post-impl)** | Same pattern as #1. No `seed_oauth_tokens` call. Post-implementation, `refresh_oauth_token` calls `load_oauth_tokens("default")` at `auth.rs:882`. Currently always-run. |
| 4 | `test_send_caps_refresh_at_one_attempt_when_retry_also_401` | 504 | NO | NO | **NO-KEYCHAIN (pre-impl) / KEYCHAIN-TRANSITIVE (post-impl)** | Same pattern as #1. No `seed_oauth_tokens`. Post-implementation calls `load_oauth_tokens`. Currently always-run. |
| 5 | `test_send_caps_refresh_at_one_attempt_when_refresh_fails` | 562 | NO | NO | **NO-KEYCHAIN (pre-impl) / KEYCHAIN-TRANSITIVE (post-impl)** | Same pattern as #1. No `seed_oauth_tokens`. Post-implementation calls `load_oauth_tokens`. Currently always-run. |
| 6 | `test_concurrent_sends_single_refresh_via_coordinator` | 627 | NO | NO | **NO-KEYCHAIN (pre-impl) / KEYCHAIN-TRANSITIVE (post-impl)** | Same pattern as #1. N=10 concurrent `new_for_test` clients sharing `profile_name="default"`. Post-implementation, each refresh attempt reads `load_oauth_tokens("default")`. Currently always-run. |
| 7 | `test_concurrent_invalid_grant_no_thundering_herd` | 720 | NO | NO | **NO-KEYCHAIN (pre-impl) / KEYCHAIN-TRANSITIVE (post-impl)** | Same pattern as #1. N=10 concurrent. Post-implementation calls `load_oauth_tokens`. Currently always-run. |
| 8 | `test_manual_jr_auth_refresh_unchanged` | 814 | NO | NO | **NO-KEYCHAIN** | Runs `jr auth refresh --help` only. No OAuth flow, no keychain touch. Regression guard — expected to pass both pre- and post-implementation. Does not need gating. |
| 9 | `test_refresh_contract_pins_url_grant_type_rotation_invalid_grant` | 860 | NO | NO | **NO-KEYCHAIN (pre-impl) / KEYCHAIN-TRANSITIVE (post-impl)** | Same pattern as #1. Two sub-sections. Post-implementation calls `load_oauth_tokens`. Currently always-run. |
| 10 | `test_waiters_use_in_memory_token_not_keychain` | 995 | YES | YES (line 996) | **KEYCHAIN-DIRECT** | Calls `harness::seed_oauth_tokens()` (line 1017). Calls `harness::cleanup_oauth_tokens()` (line 1074). Properly gated. |
| 11 | `test_inter_process_reconcile_after_invalid_grant` | 1117 | YES | YES (line 1119) | **KEYCHAIN-DIRECT** | Calls `harness::seed_oauth_tokens()` (line 1131). Calls `auth::load_oauth_tokens` (line 1195). Calls `harness::cleanup_oauth_tokens()` (line 1274). Properly gated. |
| 12 | `test_persist_before_publish_fault_injection` | 1303 | YES | YES (line 1307) + `JR_S303_PERSIST_FAIL=1` | **KEYCHAIN-DIRECT** | Calls `harness::seed_oauth_tokens()` (line 1331). Calls `auth::load_oauth_tokens` (line 1370). Properly gated behind two env vars. |

**COUNT CORRECTION — FLAGGED:** The issue body claims 7 always-run tests in this file.
The actual count is **7 always-run tests** (tests 1, 3, 4, 5, 6, 7, 8, 9) — wait, that
is actually **8 always-run tests** (tests 1, 3, 4, 5, 6, 7, 8, 9). Of these 8:
- 1 (`test_manual_jr_auth_refresh_unchanged`, test #8) is definitively NO-KEYCHAIN and
  should remain always-run — it is a help-text regression guard.
- 7 (tests 1, 3, 4, 5, 6, 7, 9) are NO-KEYCHAIN pre-implementation but will become
  KEYCHAIN-TRANSITIVE post-implementation via `load_oauth_tokens("default")` at
  `auth.rs:882` inside `refresh_oauth_token`.

The issue body's claim of "7 always-run tests with uncertain keychain status" maps
approximately to tests 1, 3, 4, 5, 6, 7, 9 — the 7 that will become transitive
post-implementation. This count is correct. The test numbering missed `test_manual_jr_auth_refresh_unchanged`
(test #8) as a distinct always-run/no-keychain test.

**Total in file: 12 tests (not 11).** The module-level comment says "All 11 tests" (line 3)
but counts AC-004 as one test when there are two test functions (v1 and v2). Actual function
count is 12. The module-level comment is off by one.

---

## Severity Assessment

**Severity: LOW-MEDIUM** (not pure LOW)

Rationale:
- LOW component: the bug is test-infrastructure only. No production code paths are affected.
  No user-visible behavior changes. No BC regressions possible.
- MEDIUM component: the CI flake is confirmed (run 26477547114) and blocks the S-3.04
  development cycle. A test that intermittently prompts for keychain access on a headless
  macOS runner is not merely cosmetic — it stalls CI jobs and requires manual re-run.
- The fix is mechanical (add `#[ignore]` + guard) but requires judgment on which env var
  to use (see Convention Recommendation below).

---

## Trivial Scope Assessment

**Classification: STANDARD (not trivial)**

Rationale for standard classification:
1. The fix spans two files and requires reasoning about pre-implementation vs post-implementation
   keychain behavior for each test.
2. Tests 1, 3, 4, 5, 6, 7, 9 in `oauth_refresh_integration.rs` are currently NO-KEYCHAIN
   but will become KEYCHAIN-TRANSITIVE post-implementation. The implementer must gate them
   proactively, not reactively — this is a forward-looking judgment call, not a mechanical
   annotation.
3. Adding a new `JR_RUN_CLOUDID_INTEGRATION` env var (Option B) requires updating CLAUDE.md
   with the new pattern, which is a documentation discipline obligation.
4. The `multi_cloudid_disambiguation.rs` always-run tests that ARE currently NO-KEYCHAIN
   (tests 1, 4, 5, 6, 8, 11) must NOT be gated — gating them would suppress useful red-gate
   failures that run in standard CI. Correct classification of each test matters.

---

## Impact: Affected Files

Test files requiring changes:
- `/Users/zious/Documents/GITHUB/jira-cli/tests/multi_cloudid_disambiguation.rs` — add gate to 5 KEYCHAIN-TRANSITIVE tests (tests 2, 3, 7, 9, 10)
- `/Users/zious/Documents/GITHUB/jira-cli/tests/oauth_refresh_integration.rs` — add gate to 7 always-run KEYCHAIN-TRANSITIVE tests (tests 1, 3, 4, 5, 6, 7, 9)

Documentation requiring changes:
- `/Users/zious/Documents/GITHUB/jira-cli/CLAUDE.md` — document new env var (if Option B) or extend existing coverage description (if Option A)

Production code: **NO CHANGES REQUIRED**.

---

## Convention Recommendation

**Recommendation: Option A — Extend `JR_RUN_KEYRING_TESTS=1` to cover all keychain-touching tests.**

### Rationale

**Developer ergonomics:**
- A single gate (`JR_RUN_KEYRING_TESTS=1`) is already the canonical convention per CLAUDE.md
  line 351. Developers who know to set it for one test know it for all tests.
- Per-suite env vars (`JR_RUN_CLOUDID_INTEGRATION=1`) require developers to discover each
  new variable by reading test file headers. This creates a knowledge fragmentation problem
  that grows as new OAuth-related test files are added.
- Option B creates a precedent where every new test suite involving auth can invent a new
  `JR_RUN_<X>_INTEGRATION` gate. This is already happening: `JR_RUN_OAUTH_INTEGRATION`
  exists for `tests/oauth_embedded_login.rs` (CLAUDE.md line 352). A third gating
  variable compounds the problem.

**CI behavior:**
- `JR_RUN_KEYRING_TESTS=1` is already plumbed into CI knowledge (documented in CLAUDE.md).
  Extending its coverage to additional test files requires no CI workflow changes — developers
  already understand they must opt-in to run keychain tests.
- Option B would require CI-adjacent documentation to list all three env vars when setting
  up a developer environment with full keychain test coverage.

**Precedent:**
- The inline unit tests in `src/api/auth.rs` already use `JR_RUN_KEYRING_TESTS=1` (e.g.,
  the `with_test_keyring` wrapper at line 1308). Extending this to integration test files
  keeps all keychain gates under one umbrella.
- `JR_RUN_OAUTH_INTEGRATION=1` exists for `oauth_embedded_login.rs` because that suite is
  conceptually distinct (it tests the OAuth browser flow, not keychain read/write). The
  `multi_cloudid_disambiguation.rs` and `oauth_refresh_integration.rs` tests touch the
  keychain as a side-effect of OAuth token storage — that is exactly the `JR_RUN_KEYRING_TESTS`
  semantic.

**Caveat:** Tests 1, 4, 5, 6, 8, 11 in `multi_cloudid_disambiguation.rs` (the NO-KEYCHAIN
tests) and test #8 in `oauth_refresh_integration.rs` MUST NOT be gated. They are valid
always-run tests that provide red-gate signal in standard CI. The implementer must apply
the gate selectively per the classification tables above.

---

## Risk Assessment

**Risk: LOW**

- Production code paths: unchanged.
- Existing gated tests: unchanged (tests already behind `#[ignore]`/`JR_RUN_KEYRING_TESTS`
  remain unchanged).
- The only behavioral change is that 12 tests move from always-run to `#[ignore]` +
  `JR_RUN_KEYRING_TESTS=1` gated.
- Regression baseline: the 6 always-run NO-KEYCHAIN tests in `multi_cloudid_disambiguation.rs`
  continue to run in CI and provide red-gate coverage for flag-registration and error-path ACs.

---

## Regression Baseline

The following tests are classified NO-KEYCHAIN and MUST remain always-run after this fix:

**`multi_cloudid_disambiguation.rs` (always-run after fix):**
- `test_cloud_id_flag_recognized_in_help` — flag registration red gate
- `test_cloud_id_flag_value_not_in_response_exits_64` — exit-64 error path
- `test_no_input_multi_org_exits_64_with_actionable_error` — exit-64 error path
- `test_no_input_multi_org_lists_available_cloud_ids_in_error` — exit-64 error path + message content
- `test_callback_url_contains_127_0_0_1_and_port_53682` — port regression pin
- `test_cloud_id_help_text_mentions_disambiguation_or_multiple_orgs` — help text quality

**`oauth_refresh_integration.rs` (always-run after fix):**
- `test_manual_jr_auth_refresh_unchanged` — manual refresh command regression guard

---

## Implementation Notes for Implementer

1. For each KEYCHAIN-TRANSITIVE test in `multi_cloudid_disambiguation.rs`: add `#[ignore]`
   attribute and add an early-return guard:
   ```rust
   if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
       return; // requires JR_RUN_KEYRING_TESTS=1 and system keychain access
   }
   ```
   These are async tests (`#[tokio::test]`), so the guard goes inside the test body.

2. For each KEYCHAIN-TRANSITIVE test in `oauth_refresh_integration.rs`: same pattern.
   The env lock (`harness::env_lock().lock().await`) must be acquired AFTER the guard
   check to avoid holding the lock for skipped tests.

3. Update CLAUDE.md to extend the `JR_RUN_KEYRING_TESTS=1` documentation (line 351)
   to explicitly call out that it covers `tests/multi_cloudid_disambiguation.rs` (KEYCHAIN-TRANSITIVE
   tests) and `tests/oauth_refresh_integration.rs` (KEYCHAIN-TRANSITIVE always-run tests).

4. The `JR_SERVICE_NAME` isolation in `jr_isolated()` (multi_cloudid, line 183) is correct
   and should be preserved. It redirects the keychain service name to a test-scoped namespace.
   This does not eliminate the macOS security framework prompt for novel service names, but it
   does prevent cross-test keychain contamination.

5. DO NOT gate `test_manual_jr_auth_refresh_unchanged` — it is NO-KEYCHAIN and provides
   useful always-run regression coverage.
