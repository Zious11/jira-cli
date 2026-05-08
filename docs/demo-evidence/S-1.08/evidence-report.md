# Demo Evidence Report — S-1.08

**Story:** Keychain per-profile layout holdout suite (H-016, BC-1.4.025..BC-1.4.030)
**Branch:** `test/S-1.08-keychain-roundtrip-holdout`
**Commit at recording:** `0262801`
**Recorded:** 2026-05-07
**Tool:** VHS 0.11.0

---

## Coverage Summary

| AC | BC / Holdout | Tests | Result | Recording |
|----|-------------|-------|--------|-----------|
| AC-001 | BC-1.4.027 | 7 | PASS | [AC-001-key-naming-per-profile](#ac-001) |
| AC-002 | BC-1.4.025 | 1 | PASS | [AC-002-lazy-migration-guard](#ac-002) |
| AC-003 | BC-1.4.028 | 1 | PASS | [AC-003-partial-state-error](#ac-003) |
| AC-004 | BC-1.4.029 | 1 | PASS | [AC-004-profile-boundary](#ac-004) |
| AC-005 | BC-1.4.030 | 4 | PASS | [AC-005-resolver-precedence](#ac-005) |
| AC-006 | H-016 | 3 | PASS | [AC-006-h016-active-profile-guard](#ac-006) |
| **Combined** | All | **17** | **17/17 PASS** | [COMBINED-all-17-tests-green](#combined) |

---

## AC-001: OAuth token keys follow namespaced pattern (BC-1.4.027) {#ac-001}

**Pins:** All OAuth token storage/retrieval uses keys of the form `<profile>:oauth-access-token`
and `<profile>:oauth-refresh-token`. Shared keys (`email`, `api-token`, `oauth_client_id`,
`oauth_client_secret`) are NOT namespaced. Tests inspect the key-construction logic in
`src/api/auth.rs` directly, without requiring a live keychain service.

**Test functions:**
- `test_s_1_08_ac001_oauth_access_key_default_profile`
- `test_s_1_08_ac001_oauth_access_key_sandbox_profile`
- `test_s_1_08_ac001_oauth_refresh_key_default_profile`
- `test_s_1_08_ac001_oauth_refresh_key_sandbox_profile`
- `test_s_1_08_ac001_profile_keys_are_distinct_across_profiles`
- `test_s_1_08_ac001_shared_keys_are_not_namespaced`
- `test_s_1_08_ac001_key_format_structure`

**Command recorded:**
```
cargo test --lib test_s_1_08_ac001 -- --nocapture --test-threads=1 2>&1
```

**Result:** 7 passed; 0 failed

**Recordings:**
- `AC-001-key-naming-per-profile.gif`
- `AC-001-key-naming-per-profile.webm`
- `AC-001-key-naming-per-profile.tape`

---

## AC-002: Lazy migration guard is default-only (BC-1.4.025) {#ac-002}

**Pins:** `load_oauth_tokens(profile)` lazy-migrates legacy flat keys ONLY when
`profile == "default"`. For non-default profiles (`"sandbox"`, etc.), the migration
is skipped. Two `if profile == "default"` guards in `src/api/auth.rs` enforce this.

**Test functions:**
- `test_s_1_08_ac002_lazy_migration_guard_sentinel_is_default`

**Command recorded:**
```
cargo test --lib test_s_1_08_ac002 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-002-lazy-migration-guard.gif`
- `AC-002-lazy-migration-guard.webm`
- `AC-002-lazy-migration-guard.tape`

---

## AC-003: Partial credential state returns error (BC-1.4.028) {#ac-003}

**Pins:** Partial OAuth token state (access token present, refresh token absent — or vice versa)
is never silently accepted. `load_oauth_tokens` returns an `Err` rather than using the partial
credential. This prevents subtle authentication failures from incomplete token writes.

**Test functions:**
- `test_s_1_08_ac003_partial_state_error_message_contains_partial`

**Command recorded:**
```
cargo test --lib test_s_1_08_ac003 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-003-partial-state-error.gif`
- `AC-003-partial-state-error.webm`
- `AC-003-partial-state-error.tape`

---

## AC-004: Per-profile namespaced keys never alias legacy flat keys (BC-1.4.029) {#ac-004}

**Pins:** `load_oauth_tokens("sandbox")` reads only `sandbox:oauth-access-token` and
`sandbox:oauth-refresh-token`. It does NOT read the legacy flat `oauth-access-token`
key (the default profile's legacy key). This prevents cross-profile credential leakage
where a sandbox profile would inherit production OAuth tokens.

**Test functions:**
- `test_s_1_08_ac004_namespaced_key_never_aliases_legacy_key`

**Command recorded:**
```
cargo test --lib test_s_1_08_ac004 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-004-profile-boundary.gif`
- `AC-004-profile-boundary.webm`
- `AC-004-profile-boundary.tape`

---

## AC-005: Keychain credential resolver precedence (BC-1.4.030) {#ac-005}

**Pins:** `resolve_refresh_app_credentials` checks keychain FIRST. BYO credentials
(`oauth_client_id` / `oauth_client_secret` stored in keychain) take priority over the
embedded OAuth app. The embedded app is only used when no BYO credentials are present.
Four tests cover all resolver paths: keychain-wins-over-embedded, keychain-only,
embedded-fallback, and no-source-resolved.

**Test functions:**
- `test_s_1_08_ac005_keychain_wins_over_embedded_when_both_present`
- `test_s_1_08_ac005_keychain_wins_when_only_keychain_present`
- `test_s_1_08_ac005_embedded_fallback_when_no_keychain`
- `test_s_1_08_ac005_none_when_no_source_resolved`

**Command recorded:**
```
cargo test --lib test_s_1_08_ac005 -- --nocapture --test-threads=1 2>&1
```

**Result:** 4 passed; 0 failed

**Recordings:**
- `AC-005-resolver-precedence.gif`
- `AC-005-resolver-precedence.webm`
- `AC-005-resolver-precedence.tape`

---

## AC-006: Active profile removal rejected — H-016 (BC-1.1.006) {#ac-006}

**Pins:** `jr --no-input auth remove <active-profile>` must exit 64 with stderr containing
`cannot remove active`, and the config file on disk must be byte-identical before and after
the command. This is the H-016 holdout: three process-spawn variants cover the basic case,
the no-`--no-input`-flag case (non-TTY detection), and the multi-profile case (second profile
present does not weaken the guard).

**Test functions:**
- `test_s_1_08_ac006_h016_remove_active_profile_rejected_and_config_unchanged`
- `test_s_1_08_ac006_h016_remove_active_profile_rejected_without_no_input_flag`
- `test_s_1_08_ac006_h016_remove_active_profile_rejected_with_second_profile_present`

**Command recorded:**
```
cargo test --test keychain_layout_holdouts -- --nocapture --test-threads=1 2>&1
```

**Result:** 3 passed; 0 failed

**Recordings:**
- `AC-006-h016-active-profile-guard.gif`
- `AC-006-h016-active-profile-guard.webm`
- `AC-006-h016-active-profile-guard.tape`

---

## Combined: All 17 tests green {#combined}

**All AC-001 through AC-006 tests run in a single invocation showing 17/17 green.**

**Command recorded:**
```
cargo test test_s_1_08_ -- --nocapture --test-threads=1 2>&1
```

**Result:** 17 passed; 0 failed

**Breakdown:**
- AC-001 (BC-1.4.027): 7 key-naming tests
- AC-002 (BC-1.4.025): 1 lazy-migration guard test
- AC-003 (BC-1.4.028): 1 partial-state error test
- AC-004 (BC-1.4.029): 1 profile boundary isolation test
- AC-005 (BC-1.4.030): 4 resolver precedence tests
- AC-006 (H-016): 3 process-spawn active-profile guard tests

**Recordings:**
- `COMBINED-all-17-tests-green.gif`
- `COMBINED-all-17-tests-green.webm`
- `COMBINED-all-17-tests-green.tape`

---

## Quality Gates

| Gate | Status |
|------|--------|
| `cargo build` | PASS (clean, 0 errors) |
| `cargo test test_s_1_08_` | PASS (17/17) |
| `cargo clippy -- -D warnings` | PASS (0 warnings) |
| `cargo fmt --all -- --check` | PASS (no formatting issues) |
