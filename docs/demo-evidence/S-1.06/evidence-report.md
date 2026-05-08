# Demo Evidence Report — S-1.06

**Story:** OAuth Flow Regression Holdout Suite
**Branch:** `test/S-1.06-oauth-flow-holdout-suite`
**Commit at recording:** `da77b76`
**Recorded:** 2026-05-07
**Tool:** VHS 0.11.0

---

## Coverage Summary

| AC | Holdout IDs | Tests | Result | Recording |
|----|-------------|-------|--------|-----------|
| AC-001 | H-001, H-002 | 2 | PASS | [AC-001-no-profiles-paths](#ac-001) |
| AC-002 | H-003 | 1 | PASS | [AC-002-profile-precedence](#ac-002) |
| AC-003 | H-004 | 1 | PASS | [AC-003-auth-refresh-no-url](#ac-003) |
| AC-004 | H-005 | 1 | PASS | [AC-004-malformed-config](#ac-004) |
| AC-005 | H-022 | 3 | PASS | [AC-005-scope-mismatch-dispatch](#ac-005) |
| AC-006 | H-029 | 3 | PASS | [AC-006-redirect-uri-strategies](#ac-006) |
| **Combined** | All | **11** | **11/11 PASS** | [AC-combined](#combined) |

---

## AC-001: No-Profiles Paths (H-001 + H-002) {#ac-001}

**Pins:** `auth status` on fresh install exits 0 with "No profiles configured"; `auth list --output json` on fresh install exits 0 with `[]`.

**Test functions:**
- `test_s_1_06_h_001_auth_status_no_profiles`
- `test_s_1_06_h_002_auth_list_json_no_profiles`

**Command recorded:**
```
cargo test --test oauth_flow_holdouts test_s_1_06_h_00 -- --nocapture --test-threads=1
```

**Result:** 2 passed; 0 failed

**Recordings:**
- `AC-001-no-profiles-paths.gif`
- `AC-001-no-profiles-paths.webm`
- `AC-001-no-profiles-paths.tape`

---

## AC-002: Profile Precedence Chain (H-003) {#ac-002}

**Pins:** Active profile resolved in strict priority order: `--profile` flag > `JR_PROFILE` env > `default_profile` in config > literal "default".

**Test functions:**
- `test_s_1_06_h_003_profile_precedence_chain`

**Command recorded:**
```
cargo test --test oauth_flow_holdouts test_s_1_06_h_003 -- --nocapture --test-threads=1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-002-profile-precedence.gif`
- `AC-002-profile-precedence.webm`
- `AC-002-profile-precedence.tape`

---

## AC-003: Auth Refresh No URL (H-004) {#ac-003}

**Pins:** `auth refresh --no-input` against a profile with no URL exits 64 (UserError) and includes "no URL configured", "jr auth login", and "--url" in stderr. No panic.

**Test functions:**
- `test_s_1_06_h_004_auth_refresh_no_url_configured`

**Command recorded:**
```
cargo test --test oauth_flow_holdouts test_s_1_06_h_004 -- --nocapture --test-threads=1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-003-auth-refresh-no-url.gif`
- `AC-003-auth-refresh-no-url.webm`
- `AC-003-auth-refresh-no-url.tape`

---

## AC-004: Malformed Config Exits 78, File Unchanged (H-005) {#ac-004}

**Pins:** `auth login --oauth` with a malformed `config.toml` exits 78 (ConfigError), surfaces "toml"/"parse" in stderr, leaves file bytes identical, and does not panic.

**Test functions:**
- `test_s_1_06_h_005_malformed_config_exits_78_file_unchanged`

**Command recorded:**
```
cargo test --test oauth_flow_holdouts test_s_1_06_h_005 -- --nocapture --test-threads=1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-004-malformed-config.gif`
- `AC-004-malformed-config.webm`
- `AC-004-malformed-config.tape`

---

## AC-005: Scope-Mismatch Dispatch (H-022) {#ac-005}

**Pins:** 401 + "scope does not match" (any case) → InsufficientScope; 401 without scope phrase → NotAuthenticated (not InsufficientScope); 403 + scope phrase → ApiError (not InsufficientScope).

**Test functions:**
- `test_s_1_06_h_022_scope_mismatch_lowercase_dispatches_insufficient_scope`
- `test_s_1_06_h_022_scope_mismatch_mixed_case_dispatches_insufficient_scope`
- `test_s_1_06_h_022_non_scope_401_and_403_do_not_dispatch_insufficient_scope`

**Command recorded:**
```
cargo test --test oauth_flow_holdouts test_s_1_06_h_022 -- --nocapture --test-threads=1
```

**Result:** 3 passed; 0 failed

**Recordings:**
- `AC-005-scope-mismatch-dispatch.gif`
- `AC-005-scope-mismatch-dispatch.webm`
- `AC-005-scope-mismatch-dispatch.tape`

---

## AC-006: Redirect URI Strategies (H-029) {#ac-006}

**Pins:** Embedded app `redirect_uri` = `http://127.0.0.1:53682/callback` (IPv4, fixed port); BYO DynamicPort uses `http://localhost:<port>/callback` (port != 53682); `EMBEDDED_CALLBACK_PORT` const = 53682 (breaking-release guard).

**Test functions:**
- `test_s_1_06_h_029_embedded_redirect_uri`
- `test_s_1_06_h_029_byo_redirect_uri_dynamic_port`
- `test_s_1_06_h_029_embedded_callback_port_const_is_53682`

**Note:** `test_s_1_06_h_029_embedded_redirect_uri` is skipped in dev builds (no `JR_BUILD_OAUTH_CLIENT_ID` at compile time) — still counts as passing per test design. BYO path and const guard run unconditionally.

**Command recorded:**
```
cargo test --test oauth_flow_holdouts test_s_1_06_h_029 -- --nocapture --test-threads=1
```

**Result:** 3 passed (1 skip-as-pass + 2 full); 0 failed

**Recordings:**
- `AC-006-redirect-uri-strategies.gif`
- `AC-006-redirect-uri-strategies.webm`
- `AC-006-redirect-uri-strategies.tape`

---

## Combined: Full Suite 11/11 Green {#combined}

**Command recorded:**
```
cargo test --test oauth_flow_holdouts -- --nocapture --test-threads=1
```

**Result:** 11 passed; 0 failed; 0 ignored; finished in ~0.84s

**Recordings:**
- `AC-combined-all-s-1-06-pass.gif`
- `AC-combined-all-s-1-06-pass.webm`
- `AC-combined-all-s-1-06-pass.tape`

---

## Quality Gates

| Gate | Status |
|------|--------|
| `cargo build` | CLEAN |
| `cargo test --test oauth_flow_holdouts` | 11/11 PASS |
| `cargo clippy -- -D warnings` | CLEAN (0 warnings) |
| `cargo fmt --all -- --check` | CLEAN |
