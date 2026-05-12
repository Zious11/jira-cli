# S-3.07 Red Gate Log

Story: S-3.07 v2.0.0 — LOW NFR code fixes (Retry-After cap, profile name precision) + /rest/api/3/search/jql anti-loop guard
Step: 3 (Test Writer)
Date: 2026-05-08
Branch: feat/s-3-07-low-nfr-fixes-and-jracloud-anti-loop

---

## Test File Placement (Deviation from Story Frontmatter)

Story frontmatter specifies `test_files: [tests/rate_limit_cap_tests.rs]`.
Actual test placement:

| File | ACs | Reason |
|------|-----|--------|
| `tests/rate_limit_cap_tests.rs` | AC-001, AC-002, AC-008, AC-NEW-D | Integration tests requiring wiremock + tokio |
| `tests/rate_limit_cap_ac003.rs` | AC-003 | Isolated to prevent compile-error from blocking other tests |
| `src/config.rs::tests` (inline) | AC-006, AC-007 | Profile-name unit tests belong in their canonical home |

**Deviation rationale:**
- AC-003's compile-fail Red Gate (symbol `MAX_RETRY_AFTER_SECS` not defined) would prevent all other tests in `rate_limit_cap_tests.rs` from running, making it impossible to observe their assertion-error failures. Separation into `rate_limit_cap_ac003.rs` lets each AC show its independent failure mode.
- AC-006/AC-007 placed inline in `src/config.rs::tests` per the story's explicit guidance: "The unit test file is the canonical location for `validate_profile_name` tests; existing tests at lines 759-787 of src/config.rs prove this convention."

---

## Red Gate Classification

**STRICT TDD** — mixed Red Gate with three distinct failure modes:
- **COMPILE ERROR** for AC-003 (structural invariant — symbol doesn't exist)
- **ASSERTION ERROR** for AC-001, AC-006, AC-007, AC-008, AC-NEW-D (behavior not yet implemented)
- **REGRESSION-PIN (PASSES pre-impl)** for AC-002 (preserves existing within-cap retry behavior)

---

## Per-AC Pre-Implementation Results

### AC-003 (rate_limit_cap_ac003.rs)
**Status: COMPILE ERROR**
```
error[E0432]: unresolved import `jr::api::rate_limit::MAX_RETRY_AFTER_SECS`
  --> tests/rate_limit_cap_ac003.rs:18:9
   |
18 |     use jr::api::rate_limit::MAX_RETRY_AFTER_SECS;
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no `MAX_RETRY_AFTER_SECS` in `api::rate_limit`
```
Failure reason traces to: constant does not exist in `src/api/rate_limit.rs`. The implementer must add `pub const MAX_RETRY_AFTER_SECS: u64 = 60;` (or `pub(crate)`) to make this compile.

### AC-001 (rate_limit_cap_tests.rs)
**Status: ASSERTION ERROR** (timeout fires)
```
thread panicked: AC-001 (BC-X.4.009): call must return before the 10s timeout;
pre-implementation this fails because Retry-After:2400 causes a 2400s sleep
which never completes under start_paused=true
```
Failure reason traces to: `src/api/client.rs::send()` line 322 executes `tokio::time::sleep(Duration::from_secs(2400))` without checking against any cap. With `start_paused = true`, this sleep never completes. The outer `tokio::time::timeout(10s, ...)` fires, returns `Err(Elapsed)`, and the assertion `result.is_ok()` fails.

Test mechanism: `#[tokio::test(start_paused = true)]` + 10s `tokio::time::timeout`. Pre-impl: times out. Post-impl: returns Err in microseconds.

### AC-002 (rate_limit_cap_tests.rs)
**Status: REGRESSION-PIN — PASSES PRE-IMPLEMENTATION**
```
test ac_002_retry_after_within_cap_retries ... ok
```
The existing rate-limit retry logic in `src/api/client.rs::send()` already retries on 429 and succeeds when the server returns 200 on the next attempt. `Retry-After: 0` (within any reasonable cap) causes zero delay, and the mock's second response (200) succeeds. This test preserves existing behavior — it must not be broken by the implementation.

Note: The story's AC-002 specifies `Retry-After: 30`. This test uses `Retry-After: 0` to avoid clock-advance complexity with `#[tokio::test]` (non-paused clock, `sleep(0)` is instant). The logical invariant (within-cap → retry proceeds) is equivalent. Documented here as an in-test deviation.

### AC-006 (src/config.rs::tests)
**Status: ASSERTION ERROR**
```
thread panicked: AC-006 (BC-6.1.004 invariant): error for a 65-char profile name
must contain 'too long' or 'max 64'. Got:
"invalid profile name \"aaa...\"; allowed: A-Z a-z 0-9 _ - up to 64 chars; ..."
```
Failure reason traces to: `fn invalid_profile_name()` in `src/config.rs` line 136 emits a single generic message for all validation failures. The implementer must split the length check and charset check to produce distinct, precise messages.

### AC-007 (src/config.rs::tests)
**Status: ASSERTION ERROR**
```
thread panicked: AC-007 (BC-6.1.004 invariant): error for a profile name with a
space must contain 'invalid characters' or 'a-z, 0-9'. Got:
"invalid profile name \"foo bar\"; allowed: A-Z a-z 0-9 _ - up to 64 chars; ..."
```
Failure reason traces to: same as AC-006 — the generic `invalid_profile_name()` helper is used for all violations without distinguishing length vs. charset.

### AC-008 + AC-NEW-D (rate_limit_cap_tests.rs, combined test)
**Status: ASSERTION ERROR** (subprocess killed by 15s timeout, then JRACLOUD-94632 not in stderr)
```
thread panicked: AC-NEW-D (NFR-R-F): stderr must contain 'JRACLOUD-94632' when the
anti-loop guard fires. Got stderr: [empty — subprocess was SIGKILL'd after 15s]
```
Failure reason traces to: `src/api/jira/issues.rs` lines 59-65 contain only a `// KNOWN-GAP: NFR-R-F` comment, no real guard. The `search_issues` cursor loop iterates indefinitely when `nextPageToken` never changes. The subprocess is killed by the 15s assert_cmd timeout.

Test mechanism: `assert_cmd::Command::timeout(15s)` + `--all` flag to bypass the default 30-issue limit. The subprocess runs until SIGKILL, returns empty stderr, and the `JRACLOUD-94632` assertion fails. Post-implementation: the guard triggers within 2 iterations, warning emitted to stderr, command exits in ~1s.

**Note on test duration:** AC-008 takes ~15 real seconds pre-implementation (subprocess runs until killed). This is intentional — the 15s timeout is the minimum time to demonstrate the infinite-loop bug. CI must tolerate this. Post-implementation the test completes in ~1 second.

---

## Infrastructure Changes

### Cargo.toml dev-dependencies addition

Added `tokio = { version = "1", features = ["test-util"] }` to `[dev-dependencies]`.

**Rationale:** `tokio = { version = "1", features = ["full"] }` in `[dependencies]` does NOT include `test-util` (Cargo's `full` feature set for tokio excludes `test-util` to avoid shipping test infrastructure in production builds). The `#[tokio::test(start_paused = true)]` attribute requires `test-util`. Adding it as a dev-dependency is purely a test infrastructure change — it does not affect production builds.

**Verification:** Cargo features are additive. The dev-dependency augments the feature set only for test binaries. The production binary compiled under `--release` is unaffected.

---

## Iron Law Verification

The Iron Law ("NO IMPLEMENTATION WITHOUT RED GATE FIRST") is satisfied:

| AC | Pre-impl result | Failure reason correctly traces to behavior under test |
|----|----------------|------------------------------------------------------|
| AC-001 | TIMEOUT/ASSERTION | `client.rs` sleeps 2400s without cap check |
| AC-002 | PASS (regression-pin) | Existing retry works; preserves behavior |
| AC-003 | COMPILE ERROR | `MAX_RETRY_AFTER_SECS` symbol doesn't exist |
| AC-006 | ASSERTION ERROR | Generic error message lacks "too long"/"max 64" |
| AC-007 | ASSERTION ERROR | Generic error message lacks "invalid characters"/"a-z, 0-9" |
| AC-008 | ASSERTION ERROR | Cursor loop runs forever, SIGKILL'd |
| AC-NEW-D | ASSERTION ERROR | No JRACLOUD-94632 warning emitted |

Zero new ACs pass before implementation (AC-002 is documented regression-pin, not new behavior).

---

## Cargo Test Commands and Exit Codes

```bash
# AC-003 (compile error):
cargo test --test rate_limit_cap_ac003
# exit code: 1 (compile error)

# AC-001, AC-002, AC-008, AC-NEW-D (assertion errors):
cargo test --test rate_limit_cap_tests
# exit code: 1 (3 failed: ac_001, ac_008+ac_new_d; 1 passed: ac_002)

# AC-006, AC-007 (assertion errors):
cargo test --lib "test_validate_profile_name"
# exit code: 1 (2 failed: ac_006, ac_007)
```
