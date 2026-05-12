# S-3.05 Red Gate Log

**Story:** S-3.05 — Cap asset enrichment join_all concurrency with buffer_unordered(8)  
**Date:** 2026-05-08  
**Branch:** feat/s-3-05-buffer-unordered-asset-enrichment-cap  
**Base commit:** 6bce18c  
**Test files:**
- `tests/asset_enrichment_concurrency.rs` (AC-001, AC-002, AC-003)
- `tests/asset_enrichment_concurrency_ac004.rs` (AC-004, compile-error gate — isolated)

---

## Red Gate Results

| AC | Test Name | Expected Pre-impl Result | Actual Result | Notes |
|----|-----------|--------------------------|---------------|-------|
| AC-001 | `test_BC_4_3_002_ac_001_enrich_resolves_all_10_assets` | REGRESSION-PIN (PASS) | PASS | existing join_all resolves all 10 correctly |
| AC-002 | `test_BC_4_3_002_ac_002_concurrent_enrichment_capped_at_8` | ASSERTION ERROR (FAIL) | FAIL: elapsed=56ms < 90ms | join_all overlaps all 20 delays; threshold correctly distinguishes |
| AC-003 | `test_BC_X_1_005_ac_003_cap_does_not_bypass_retry` | REGRESSION-PIN (PASS) | PASS | retry logic transparent to join_all |
| AC-004 | `ac_004_max_concurrent_asset_fetches_constant_defined` | COMPILE ERROR | COMPILE ERROR: E0432 | constant not yet defined in linked.rs |

**Overall: Red Gate VERIFIED. All tests fail for the correct reason pre-implementation.**

---

## Cargo Test Output

### tests/asset_enrichment_concurrency.rs

```
running 3 tests
test test_BC_X_1_005_ac_003_cap_does_not_bypass_retry ... ok
test test_BC_4_3_002_ac_001_enrich_resolves_all_10_assets ... ok
test test_BC_4_3_002_ac_002_concurrent_enrichment_capped_at_8 ... FAILED

failures:
---- test_BC_4_3_002_ac_002_concurrent_enrichment_capped_at_8 stdout ----
AC-002 (BC-4.3.002 invariant): concurrent enrichment must be capped at 8;
expected elapsed >= 90ms (3 rounds × 50ms with cap=8), but got 56ms.
Pre-impl join_all overlaps all 20 delays simultaneously (~50ms).
Post-impl buffer_unordered(8) processes in rounds (~150ms).

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### tests/asset_enrichment_concurrency_ac004.rs

```
error[E0432]: unresolved import `jr::api::assets::linked::MAX_CONCURRENT_ASSET_FETCHES`
  --> tests/asset_enrichment_concurrency_ac004.rs:19:9
   |
19 |     use jr::api::assets::linked::MAX_CONCURRENT_ASSET_FETCHES;
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no `MAX_CONCURRENT_ASSET_FETCHES` in `api::assets::linked`
error: could not compile `jr` (test "asset_enrichment_concurrency_ac004") due to 1 previous error
```

---

## Deviations from Story Sketch

### AC-002: Timing-based approach instead of Arc<AtomicUsize>+fetch_max

**Story sketch** recommended the `Arc<AtomicUsize>` peak-counter approach with `std::thread::sleep` inside the `Respond::respond` method.

**Actual approach**: `ResponseTemplate::set_delay(50ms)` with `Instant::elapsed() >= 90ms` assertion.

**Reason for deviation**: wiremock 0.6.5 serializes ALL calls to `Respond::respond` under a `RwLock::write()` guard (see `src/mock_server/hyper.rs:34-42`). Specifically:

```rust
let (response, delay) = server_state
    .write()
    .await
    .handle_request(wiremock_request)
    .await;
// write lock released here
if let Some(delay) = delay { delay.await; }
```

The `respond()` method runs under the write lock, serializing all calls to peak=1 regardless of whether `join_all` or `buffer_unordered(8)` is used. Using `std::thread::sleep` inside `respond()` blocks a tokio worker while holding the write lock, preventing other requests from being processed at all.

**Timing approach correctness**:
- The `set_delay` delay runs AFTER the write lock is released, so multiple requests can be in their delay phase concurrently.
- With `join_all`: all 20 delay phases overlap → elapsed ≈ 50ms
- With `buffer_unordered(8)`: 3 rounds of 8 × 50ms → elapsed ≈ 150ms
- Threshold of 90ms provides 40ms margin in each direction
- Verified pre-impl: elapsed=56ms (< 90ms, FAILS correctly)
- Expected post-impl: elapsed≈150ms (≥ 90ms, PASSES)

**The story's "flaky timing" concern** applies to 1ms delays with tight thresholds; 50ms delays with a 90ms threshold are robust on loaded CI runners (Atlassian free tier, MacBook Pro M3).

### AC-004: Compile-error isolation follows S-3.07 pattern

Matches the `rate_limit_cap_ac003.rs` precedent: compile-error tests are isolated to prevent blocking assertion-error tests.

---

## Hand-off to Implementer

All tests fail for the correct reason pre-implementation. The implementer should:

1. Add `pub const MAX_CONCURRENT_ASSET_FETCHES: usize = 8;` at the top of `src/api/assets/linked.rs`
2. Add `use futures::stream::{self, StreamExt};` import
3. Replace `futures::future::join_all(futures).await` with `stream::iter(futures).buffer_unordered(MAX_CONCURRENT_ASSET_FETCHES).collect::<Vec<_>>().await`
4. Check `src/cli/issue/list.rs` around line 445 for a second `join_all` call site and apply the same cap

Expected post-implementation outcomes:
- AC-001: still PASSES
- AC-002: PASSES (elapsed ≈ 150ms ≥ 90ms)
- AC-003: still PASSES
- AC-004: PASSES (constant now visible)
- H-038 (asset holdout): still PASSES
