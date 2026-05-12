# S-3.01 Review Findings

Story: Shard-split cli/auth.rs into auth/ module (9 files)
PR: #319

## Convergence Table

| Cycle | Findings | Blocking | Fixed | Remaining | Verdict |
|-------|----------|----------|-------|-----------|---------|
| 1 | 0 | 0 | 0 | 0 | APPROVE |

## Cycle 1 — 2026-05-09

**Reviewer verdict:** APPROVE

**Scope reviewed:**
- `src/cli/auth/mod.rs` (121 LOC) — dispatch, AuthFlow enum, re-exports
- `src/cli/auth/login.rs` (366 LOC) — handle_login, login_token, login_oauth, prepare_login_target
- `src/cli/auth/keychain.rs` (256 LOC) — resolve_credential, resolve_oauth_app_credentials
- `src/cli/auth/refresh.rs` (144 LOC) — refresh_credentials, RefreshArgs
- `src/cli/auth/status.rs` (140 LOC) — status, peek_oauth_app_source
- `src/cli/auth/remove.rs` (129 LOC) — handle_remove, handle_remove_in_memory
- `src/cli/auth/list.rs` (70 LOC) — handle_list, render_list_table, render_list_json
- `src/cli/auth/switch.rs` (51 LOC) — handle_switch, handle_switch_in_memory
- `src/cli/auth/logout.rs` (50 LOC) — handle_logout, resolve_logout_target
- `src/cli/auth/tests/mod.rs` (997 LOC) — consolidated test module

**Findings:** None

### AC Verification

| AC | Claim | Verified |
|----|-------|---------|
| AC-001 | All tests pass | YES — test module present and complete; demo evidence shows 612 pass |
| AC-002 | No keyring::Entry in cli/auth/*.rs | YES — keychain.rs delegates entirely to api::auth; no keyring::Entry import |
| AC-003 | Release build green | YES — demo evidence AC-003 |
| AC-004 | No shard >800 LOC (max 366 login.rs) | YES — all shards confirmed under 800 LOC |
| AC-005 | 7 subcommands surface unchanged | YES — all 7 re-exported from mod.rs |
| AC-006 | BC-7.4.013-016 JSON shapes preserved | YES — list/logout/status/refresh all emit same JSON shape |

### Structural Decisions Assessment

1. **AuthFlow pub(crate)** — Correct. The enum is used in `pub fn refresh_credentials` signature
   which crosses from mod.rs into refresh.rs. The `private-interfaces` lint requires minimum
   `pub(crate)`. Not in public API. Justified.

2. **Tests consolidated to auth/tests/mod.rs** — Correct. The test module uses `use super::*`
   to access items across all shards (chosen_flow, chosen_flow_for_profile, AuthFlow, etc.).
   Spreading tests across 9 shards would require duplicating fixture helpers. Consolidated
   approach is idiomatic and matches the project's pattern for complex test fixtures.

3. **`pub(crate)` re-exports gated behind `#[cfg(test)]`** — Correct Rust pattern. Items like
   `prepare_login_target`, `resolve_oauth_scopes`, `refresh_success_payload`, etc. are only
   needed in tests and gated properly.

4. **Shard ordering (extract smallest first)** — 10 incremental commits in dependency order,
   each compiling cleanly. Correct implementation strategy for risk-bounded refactoring.

### Security Notes

- `keychain.rs` delegates entirely to `crate::api::auth::*` functions — zero direct `keyring::Entry` calls
- `unsafe` blocks in test module are guarded by `ENV_LOCK` mutex — canonical Rust test pattern
- No new injection surface, no auth bypass, no new dependencies

## Status: CONVERGED (1 cycle)
