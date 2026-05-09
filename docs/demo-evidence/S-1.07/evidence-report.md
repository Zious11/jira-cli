# Demo Evidence Report — S-1.07

**Story:** Rate-Limit Regression Holdout Suite
**Branch:** `test/S-1.07-rate-limit-holdout-suite`
**Commit at recording:** `1e78e5f`
**Recorded:** 2026-05-07
**Tool:** VHS 0.11.0

---

## Coverage Summary

| AC | Holdout IDs | Tests | Result | Recording |
|----|-------------|-------|--------|-----------|
| AC-001 | H-013 (x2) | 2 | PASS | [AC-001-send-raw-persistent-429](#ac-001) |
| AC-002 | H-027 | 1 | PASS (KNOWN-GAP) | [AC-002-retry-after-86400-no-cap](#ac-002) |
| AC-003 | H-013 (recovery) | 1 | PASS | [AC-003-send-recovers-after-2-retries](#ac-003) |
| AC-004 | — | 1 | PASS | [AC-004-retry-after-0-boundary](#ac-004) |
| AC-005 | — | 1 | PASS | [AC-005-retry-after-unparseable](#ac-005) |
| **Combined** | All | **6** | **6/6 PASS** | [COMBINED-all-6-tests-green](#combined) |

---

## AC-001: send_raw Persistent 429 — Retry Exhaustion (H-013) {#ac-001}

**Pins:** `send_raw` on a server that returns 429 for ALL requests returns `Ok(Response)` with
status 429 after exhausting `MAX_RETRIES=3` (4 total HTTP calls). Wiremock `.expect(4)` enforces
the exact call count. Process-level test verifies stderr "rate limited by Jira — gave up after 3
retries" warning.

**Test functions:**
- `test_s_1_07_h_013_send_raw_persistent_429_returns_429_after_max_retries`
- `test_s_1_07_h_013_send_raw_gave_up_warning_in_stderr`

**Command recorded:**
```
cargo test --test rate_limit_holdouts test_s_1_07_h_013_send_raw -- --nocapture --test-threads=1 2>&1
```

**Result:** 2 passed; 0 failed

**Recordings:**
- `AC-001-send-raw-persistent-429.gif`
- `AC-001-send-raw-persistent-429.webm`
- `AC-001-send-raw-persistent-429.tape`

---

## AC-002: Retry-After 86400 — No Cap (H-027, KNOWN-GAP) {#ac-002}

**Pins:** `RateLimitInfo::from_headers` with `Retry-After: 86400` returns
`retry_after_secs == Some(86400)`. The value is NOT clamped to any maximum.

**KNOWN-GAP:** H-027. When NFR-R-NEW-1 (`MAX_RETRY_AFTER_SECS=60` cap) is implemented in
S-3.07, the assertion changes to `Some(60)` and holdout status becomes MUST-PASS-AFTER-FIX.

**Test functions:**
- `test_s_1_07_h_027_retry_after_86400_no_cap`

**Command recorded:**
```
cargo test --test rate_limit_holdouts test_s_1_07_h_027 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-002-retry-after-86400-no-cap.gif`
- `AC-002-retry-after-86400-no-cap.webm`
- `AC-002-retry-after-86400-no-cap.tape`

---

## AC-003: send Recovers After 2 Retries (Recovery Regression Guard) {#ac-003}

**Pins:** The standard `send` path (via `get`) retries on 429 and succeeds when the server
eventually returns 200. Sequence: 429 → 429 → 200. Wiremock enforces exactly 3 HTTP calls
via `.up_to_n_times(2)` + `.expect(2)` for the 429 mock and `.expect(1)` for the 200 mock.

**Test functions:**
- `test_s_1_07_h_013_send_recovers_after_2_retries`

**Command recorded:**
```
cargo test --test rate_limit_holdouts test_s_1_07_h_013_send_recovers -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-003-send-recovers-after-2-retries.gif`
- `AC-003-send-recovers-after-2-retries.webm`
- `AC-003-send-recovers-after-2-retries.tape`

---

## AC-004: Retry-After 0 — Boundary Value Preserved {#ac-004}

**Pins:** `RateLimitInfo::from_headers` with `Retry-After: 0` returns
`retry_after_secs == Some(0)`. Zero is a valid value (no delay before retry) and must not be
coerced to `None` or `DEFAULT_RETRY_SECS`.

**Test functions:**
- `test_s_1_07_retry_after_0_boundary`

**Command recorded:**
```
cargo test --test rate_limit_holdouts test_s_1_07_retry_after_0 -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-004-retry-after-0-boundary.gif`
- `AC-004-retry-after-0-boundary.webm`
- `AC-004-retry-after-0-boundary.tape`

---

## AC-005: Retry-After Unparseable — Returns None {#ac-005}

**Pins:** `RateLimitInfo::from_headers` with a non-integer `Retry-After: abc` header returns
`retry_after_secs == None`. The fallback to `DEFAULT_RETRY_SECS` (1s) is applied at the call
site via `.unwrap_or(DEFAULT_RETRY_SECS)`, NOT inside `from_headers` itself. Per NFR-SCA-1:
HTTP-date format is treated the same way — any non-integer value falls through to `None`.

**Test functions:**
- `test_s_1_07_retry_after_unparseable_returns_none`

**Command recorded:**
```
cargo test --test rate_limit_holdouts test_s_1_07_retry_after_unparseable -- --nocapture --test-threads=1 2>&1
```

**Result:** 1 passed; 0 failed

**Recordings:**
- `AC-005-retry-after-unparseable.gif`
- `AC-005-retry-after-unparseable.webm`
- `AC-005-retry-after-unparseable.tape`

---

## Combined: Full Suite 6/6 Green {#combined}

**Command recorded:**
```
cargo test --test rate_limit_holdouts -- --nocapture --test-threads=1 2>&1
```

**Result:** 6 passed; 0 failed; 0 ignored; finished in ~0.77s

**Recordings:**
- `COMBINED-all-6-tests-green.gif`
- `COMBINED-all-6-tests-green.webm`
- `COMBINED-all-6-tests-green.tape`

---

## Quality Gates

| Gate | Status |
|------|--------|
| `cargo build` | CLEAN |
| `cargo test --test rate_limit_holdouts` | 6/6 PASS |
| `cargo clippy -- -D warnings` | CLEAN (0 warnings) |
| `cargo fmt --all -- --check` | CLEAN |
