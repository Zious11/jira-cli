# Demo Evidence Report — S-0.01

**Story:** Fix `handle_open` to use `instance_url()` for OAuth profiles  
**Story ID:** S-0.01  
**Branch:** fix/handle-open-oauth-instance-url  
**Date recorded:** 2026-05-07  
**Recording medium:** VHS (terminal recordings — CLI product)  
**Font:** Menlo (system default on macOS)

---

## Coverage Summary

| AC | Description | Test | Recording | Result |
|----|-------------|------|-----------|--------|
| AC-001 | OAuth URL correctness — `instance_url()` used, not `base_url()` | `test_bc_3_4_001_oauth_uses_instance_url` | AC-001-oauth-url-correctness.gif | PASS |
| AC-002 | api-token regression guard — instance URL unchanged by fix | `test_bc_3_4_001_api_token_regression_guard` | AC-002-api-token-regression-guard.gif | PASS |
| AC-003 | No double-slash in composed browse URL | `test_bc_3_4_001_no_double_slash` | AC-003-no-double-slash.gif | PASS |
| Combined | All three ACs pass in a single `cargo test --test issue_open` run | all 3 | AC-combined-all-three-pass.gif | 3/3 PASS |

---

## AC-001: OAuth URL Correctness

**Acceptance Criterion:** Given an OAuth profile where `client.base_url()` returns
`https://api.atlassian.com/ex/jira/my-cloud-123` and `client.instance_url()` returns
`https://mycompany.atlassian.net`, when `jr issue open FOO-1 --url-only` is executed,
stdout contains `https://mycompany.atlassian.net/browse/FOO-1` and does NOT contain
`api.atlassian.com`.

**Test:** `test_bc_3_4_001_oauth_uses_instance_url` in `tests/issue_open.rs`

**Recordings:**
- `AC-001-oauth-url-correctness.gif` (122 KB)
- `AC-001-oauth-url-correctness.webm` (329 KB)
- `AC-001-oauth-url-correctness.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_3_4_001_oauth_uses_instance_url ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
```

---

## AC-002: api-token Regression Guard

**Acceptance Criterion:** Given an api-token profile where `client.instance_url()`
returns `https://mycompany.atlassian.net`, when `jr issue open PROJ-123 --url-only`
is executed, stdout contains `https://mycompany.atlassian.net/browse/PROJ-123`.
(Regression guard: api-token mode is unaffected by this fix.)

**Test:** `test_bc_3_4_001_api_token_regression_guard` in `tests/issue_open.rs`

**Recordings:**
- `AC-002-api-token-regression-guard.gif` (126 KB)
- `AC-002-api-token-regression-guard.webm` (337 KB)
- `AC-002-api-token-regression-guard.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_3_4_001_api_token_regression_guard ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.70s
```

---

## AC-003: No Double-Slash

**Acceptance Criterion:** The URL does not gain a double slash: result is
`https://mycompany.atlassian.net/browse/PROJ-123`, not
`https://mycompany.atlassian.net//browse/PROJ-123`.

**Test:** `test_bc_3_4_001_no_double_slash` in `tests/issue_open.rs`

**Recordings:**
- `AC-003-no-double-slash.gif` (116 KB)
- `AC-003-no-double-slash.webm` (283 KB)
- `AC-003-no-double-slash.tape` (VHS script source)

**Evidence (captured test output):**
```
running 1 test
test test_bc_3_4_001_no_double_slash ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.70s
```

---

## Combined: All Three ACs Pass

**Recordings:**
- `AC-combined-all-three-pass.gif` (104 KB)
- `AC-combined-all-three-pass.webm` (313 KB)
- `AC-combined-all-three-pass.tape` (VHS script source)

**Evidence (captured test output):**
```
running 3 tests
test test_bc_3_4_001_api_token_regression_guard ... ok
test test_bc_3_4_001_no_double_slash ... ok
test test_bc_3_4_001_oauth_uses_instance_url ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s
```

---

## Quality Gates

All gates verified clean after demo recordings (no source changes):

| Gate | Result |
|------|--------|
| `cargo build` | clean |
| `cargo test` | green (3 passed, 0 failed) |
| `cargo clippy -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |

---

## Traceability

| Recording | AC | BC Anchor | H-046 Outcome |
|-----------|----|-----------|---------------|
| AC-001 | AC-001 | BC-3.4.001 | MUST-PASS (was MUST-FAIL at dea1664) |
| AC-002 | AC-002 | BC-3.4.001 | MUST-PASS |
| AC-003 | AC-003 | BC-3.4.001 | MUST-PASS |
