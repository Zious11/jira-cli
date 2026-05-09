# Demo Evidence Report — S-3.03

## Story Metadata

| Field | Value |
|-------|-------|
| Story ID | S-3.03 |
| Title | Auto-refresh OAuth on 401 with Single-Flight Coordination |
| Version | 2.0.0 (v2 spec) |
| Mode | strict |
| Branch | `feat/S-3.03-auto-refresh-single-flight` |
| Base SHA | `b6ab77c` (develop before feature) |
| Product type | CLI (Rust) |
| Recording tool | VHS 0.11.0 |
| Recorded | 2026-05-09 |

## Commit SHAs

| Role | SHA | Subject |
|------|-----|---------|
| test-writer (red gate) | `fdd2cc7` | test(S-3.03): red gate — auto-refresh OAuth on 401 with single-flight coordination |
| implementer | `1d96a2a` | feat(S-3.03): auto-refresh OAuth on 401 with per-profile single-flight coordination |
| parallel-safety fix | `d80f5cb` | test(S-3.03): serialize env-var-using tests with file-local Mutex (CI parallel-safe) |

## Per-AC Coverage

| AC | Claim | Artifact | Confirmation |
|----|-------|----------|--------------|
| AC-001 | `JiaClient::send()` auto-refreshes on 401, retries once, returns 200 to caller | `AC-001-auto-refresh-on-401.{gif,webm}` | `test_send_retries_once_after_refresh_on_401` passes: wiremock 401→refresh→retry→200 |
| AC-002 | Refresh tokens persisted via `store_oauth_tokens`, not raw keyring calls | (keyring-gated; see AC-009-011 row) | `test_refresh_persists_rotated_tokens_via_store_oauth_tokens` — `#[ignore]` gated, passes with `JR_RUN_KEYRING_TESTS=1` |
| AC-003 | `invalid_grant` surfaces `NotAuthenticated` with hint `"run 'jr auth refresh' to re-authenticate"` | `AC-003-invalid-grant-not-authenticated.{gif,webm}` | `test_invalid_grant_surfaces_not_authenticated_with_refresh_hint` passes |
| AC-004 | Auto-refresh capped at exactly 1 attempt (both variants: retry-401 and refresh-fail) | `AC-004-one-attempt-cap.{gif,webm}` | Two sub-tests pass: `_when_retry_also_401` and `_when_refresh_fails` |
| AC-005 | N=10 concurrent sends → exactly 1 refresh HTTP call; `Mock::expect(1)` verified on drop | `AC-005-concurrent-single-refresh.{gif,webm}` | `test_concurrent_sends_single_refresh_via_coordinator` passes |
| AC-006 | N=10 concurrent invalid_grant → 1 refresh call, all 10 surface `NotAuthenticated` | `AC-006-no-thundering-herd.{gif,webm}` | `test_concurrent_invalid_grant_no_thundering_herd` passes |
| AC-007 | Manual `jr auth refresh` CLI command unchanged; auth holdout suite has no regressions | `AC-007-manual-refresh-unchanged.{gif,webm}` | `test_manual_jr_auth_refresh_unchanged` passes (regression pin) |
| AC-008 | Contract test pins URL, `grant_type`, token rotation, and `invalid_grant` handling | `AC-008-refresh-contract-pinning.{gif,webm}` | `test_refresh_contract_pins_url_grant_type_rotation_invalid_grant` passes |
| AC-009 | Waiters use in-memory `RefreshState.last_access_token`, not keychain re-read | `AC-009-011-keyring-gated-ignored.{gif,webm}` | `test_waiters_use_in_memory_token_not_keychain` — `#[ignore]` gated |
| AC-010 | Inter-process reconcile: on `invalid_grant`, re-read keychain; if rotated by peer, retry original call | `AC-009-011-keyring-gated-ignored.{gif,webm}` | `test_inter_process_reconcile_after_invalid_grant` — `#[ignore]` gated |
| AC-011 | Persist-before-publish: `RefreshState` not updated if `store_oauth_tokens` fails | `AC-009-011-keyring-gated-ignored.{gif,webm}` | `test_persist_before_publish_fault_injection` — `#[ignore]` gated |
| Bonus-1 | Mutex layering rule documented in `refresh_coordinator.rs` module preamble | `AC-Bonus-1-mutex-layering-rule.{gif,webm}` | `head -30 src/api/refresh_coordinator.rs` shows `//! # Mutex layering rule` comment |
| Bonus-2 | 8/8 always-run integration tests + 612/612 lib tests — no regression | `AC-Bonus-2-all-tests-and-no-regression.{gif,webm}` | Both suites complete with `test result: ok` |

## Artifact List

All files reside under `docs/demo-evidence/S-3.03/`.

| Artifact | Type | Size |
|----------|------|------|
| `AC-001-auto-refresh-on-401.tape` | VHS script | 689B |
| `AC-001-auto-refresh-on-401.gif` | GIF recording | 74K |
| `AC-001-auto-refresh-on-401.webm` | WEBM recording | 114K |
| `AC-003-invalid-grant-not-authenticated.tape` | VHS script | 768B |
| `AC-003-invalid-grant-not-authenticated.gif` | GIF recording | 82K |
| `AC-003-invalid-grant-not-authenticated.webm` | WEBM recording | 123K |
| `AC-004-one-attempt-cap.tape` | VHS script | 675B |
| `AC-004-one-attempt-cap.gif` | GIF recording | 81K |
| `AC-004-one-attempt-cap.webm` | WEBM recording | 125K |
| `AC-005-concurrent-single-refresh.tape` | VHS script | 715B |
| `AC-005-concurrent-single-refresh.gif` | GIF recording | 78K |
| `AC-005-concurrent-single-refresh.webm` | WEBM recording | 116K |
| `AC-006-no-thundering-herd.tape` | VHS script | 719B |
| `AC-006-no-thundering-herd.gif` | GIF recording | 77K |
| `AC-006-no-thundering-herd.webm` | WEBM recording | 116K |
| `AC-007-manual-refresh-unchanged.tape` | VHS script | 702B |
| `AC-007-manual-refresh-unchanged.gif` | GIF recording | 71K |
| `AC-007-manual-refresh-unchanged.webm` | WEBM recording | 108K |
| `AC-008-refresh-contract-pinning.tape` | VHS script | 788B |
| `AC-008-refresh-contract-pinning.gif` | GIF recording | 82K |
| `AC-008-refresh-contract-pinning.webm` | WEBM recording | 124K |
| `AC-009-011-keyring-gated-ignored.tape` | VHS script | 959B |
| `AC-009-011-keyring-gated-ignored.gif` | GIF recording | 136K |
| `AC-009-011-keyring-gated-ignored.webm` | WEBM recording | 242K |
| `AC-Bonus-1-mutex-layering-rule.tape` | VHS script | 696B |
| `AC-Bonus-1-mutex-layering-rule.gif` | GIF recording | 132K |
| `AC-Bonus-1-mutex-layering-rule.webm` | WEBM recording | 68K |
| `AC-Bonus-2-all-tests-and-no-regression.tape` | VHS script | 706B |
| `AC-Bonus-2-all-tests-and-no-regression.gif` | GIF recording | 85K |
| `AC-Bonus-2-all-tests-and-no-regression.webm` | WEBM recording | 193K |

## Reproduction Commands

```bash
# Pre-build test binary (avoids compilation in each recording)
cargo test --test oauth_refresh_integration --no-run

# Run always-on tests (8/8)
cargo test --test oauth_refresh_integration 2>&1 | tail -5

# Run keyring-gated tests (Linux CI only)
JR_RUN_KEYRING_TESTS=1 JR_SERVICE_NAME=jr-s303-test \
  cargo test --test oauth_refresh_integration -- --ignored --test-threads=1

# Run lib tests for regression check (612/612)
cargo test --lib 2>&1 | tail -5

# Replay a specific recording
vhs docs/demo-evidence/S-3.03/AC-001-auto-refresh-on-401.tape
```

## Mutex Layering Rule (CLAUDE.md excerpt)

From `src/api/refresh_coordinator.rs` preamble and CLAUDE.md gotcha section:

> Outer `std::sync::Mutex<HashMap<...>>` is held ONLY BRIEFLY for HashMap lookup/insert;
> it is released BEFORE any `.await`. Inner `tokio::sync::Mutex<RefreshState>` is held
> across the refresh `.await`. NEVER use `std::sync::Mutex` for the inner mutex —
> `tokio::sync::Mutex` is mandatory because it does NOT poison on panic, which is the
> correct semantic for refresh (a panicked refresh should not permanently break the
> coordinator).

## Deviations

| # | Deviation | Rationale |
|---|-----------|-----------|
| a | The test-writer commit (`fdd2cc7`) and implementer commit (`1d96a2a`) are separate commits, not squashed | Factory convention: test-writer and implementer are distinct phases; squash happens at PR merge |
| b | `tokio::sync::Mutex<()>` used as env-var lock in test harness (`d80f5cb`) | Standard Rust async locking for env isolation in `tokio::test`; `std::sync::Mutex` would deadlock across `.await` in the test harness |
| c | Test names use `test_<verb>_<subject>_<outcome>` convention per CLAUDE.md | New naming convention applied to all new tests; existing tests in other files not renamed |
| d | AC-002, AC-009, AC-010, AC-011 have `#[ignore]` gate (`JR_RUN_KEYRING_TESTS=1`) | macOS CI may not have secure-storage access; keychain tests require real keyring; Linux CI runs these gated tests |

## Disclaimer

AC-005 and AC-006 wall-clock timing varies based on system parallelism. Tests use
`Mock::expect(1)` for the concurrency contract assertion — the assertion holds regardless
of wall-clock duration. The `tokio::sync::Mutex<()>` env-lock in the test harness
serializes env-var-touching tests within a single test binary run to prevent
`JR_OAUTH_TOKEN_URL` contamination between tests. This lock is test-only and does not
affect production code paths.
