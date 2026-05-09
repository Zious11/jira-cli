# Demo Evidence Report — S-3.05

**Story:** S-3.05 — Cap `buffer_unordered` concurrency for asset enrichment  
**Branch:** `feat/s-3-05-buffer-unordered-asset-enrichment-cap`  
**Date:** 2026-05-09  
**Recorder:** Demo Recorder agent

---

## Coverage Summary

| AC | Description | Recording | Result |
|----|-------------|-----------|--------|
| AC-001 | Correctness preserved: all 10 id-only assets resolved after enrichment | AC-001-resolves-all-id-only-inputs | PASS |
| AC-002 | Peak concurrency capped at 8: elapsed ≥ 90ms for 20 assets (3 rounds × 50ms) | AC-002-concurrency-capped-at-eight | PASS |
| AC-003 | Retry-through-cap: 429 + Retry-After:1 retried successfully through buffer | AC-003-retry-works-through-cap | PASS |
| AC-004 | `MAX_CONCURRENT_ASSET_FETCHES` constant exists with value 8 | AC-004-constant-is-eight | PASS |

---

## AC-001 — Correctness Preserved

**Test:** `test_enrich_assets_resolves_all_id_only_inputs`  
**File:** `tests/asset_enrichment_concurrency.rs`  
**Tracing:** BC-4.3.002 (enrichment correctness)

**Recording:**
- `AC-001-resolves-all-id-only-inputs.gif`
- `AC-001-resolves-all-id-only-inputs.webm`

Demonstrates that switching from `join_all` to `buffer_unordered(8)` preserves
correctness. 10 id-only assets each hit a distinct mock endpoint; after enrichment
all 10 have non-None `key` and `name` matching expected values. Wiremock `expect(1)`
per endpoint verifies no double-fetching.

---

## AC-002 — Peak Concurrency Capped at 8

**Test:** `test_enrich_assets_concurrency_capped_at_eight`  
**File:** `tests/asset_enrichment_concurrency.rs`  
**Tracing:** BC-4.3.002 (enrichment invariant)

**Recording:**
- `AC-002-concurrency-capped-at-eight.gif`
- `AC-002-concurrency-capped-at-eight.webm`

Demonstrates the timing-based cap assertion. With 20 assets and `set_delay(50ms)`:

- Pre-implementation (`join_all`): all 20 delay phases overlap → elapsed ≈ 50ms → assertion `elapsed >= 90ms` **FAILS**
- Post-implementation (`buffer_unordered(8)`): ceil(20/8) = 3 rounds × 50ms → elapsed ≈ 150ms → assertion **PASSES**

The 90ms threshold gives 40ms margin on both sides. The timing mechanism is robust
because `ResponseTemplate::set_delay` runs outside wiremock's write lock, allowing
true concurrent delay phases.

---

## AC-003 — Retry Works Through Cap

**Test:** `test_enrich_assets_retry_works_through_cap`  
**File:** `tests/asset_enrichment_concurrency.rs`  
**Tracing:** BC-X.1.005 (rate-limit retry)

**Recording:**
- `AC-003-retry-works-through-cap.gif`
- `AC-003-retry-works-through-cap.webm`

Demonstrates that `buffer_unordered` does not bypass per-future retry logic in
`JiraClient::send`. One endpoint (obj-006) returns `429 + Retry-After: 1` on first
call, then `200` on retry. All 6 assets — including the retried one — are resolved
after enrichment.

---

## AC-004 — Constant Exists with Value 8

**Test:** `test_max_concurrent_asset_fetches_constant_is_eight`  
**File:** `tests/asset_enrichment_concurrency_ac004.rs`  
**Tracing:** BC-4.3.002 (postcondition)

**Recording:**
- `AC-004-constant-is-eight.gif`
- `AC-004-constant-is-eight.webm`

Demonstrates that `MAX_CONCURRENT_ASSET_FETCHES` is a public, auditable constant
with value `8usize`. Isolated in its own test file because its pre-implementation
Red Gate is a compile error (symbol did not exist), not an assertion error.

---

## Static Code Evidence

### Constant definition (`src/api/assets/linked.rs`, line 24)

```rust
/// Cap on concurrent asset enrichment HTTP calls.
///
/// jr issues many concurrent GETs to /jsm/assets/workspace/.../v1/object/{id}
/// when enriching id-only CMDB assets in issue list views. Without a cap,
/// the original `join_all` pattern fires K simultaneous requests for K assets.
/// For typical issue lists (5-10 assets) this is fine; for large project
/// views (100+ issues with many CMDB assets), it creates a 100+ request
/// burst that risks 429 rate limiting from Atlassian.
///
/// 8 is a "good neighbor" default — conservative against Atlassian's documented
/// 100 req/s GET burst with no per-app concurrency cap.
pub const MAX_CONCURRENT_ASSET_FETCHES: usize = 8;
```

### `enrich_assets` call site (`src/api/assets/linked.rs`, line 231)

```rust
let results: Vec<_> = stream::iter(futures)
    .buffer_unordered(MAX_CONCURRENT_ASSET_FETCHES)
    .collect()
    .await;
```

### `handle_list` call site (`src/cli/issue/list.rs`, line 449)

```rust
let results: Vec<_> = stream::iter(futures)
    .buffer_unordered(MAX_CONCURRENT_ASSET_FETCHES)
    .collect()
    .await;
```

Both `enrich_assets` (per-issue asset enrichment path) and `handle_list`
(per-field JSON asset enrichment path) apply the same cap, ensuring consistent
rate-limit behavior across all asset enrichment code paths.

---

## Artifact Inventory

| File | Size | AC |
|------|------|----|
| `AC-001-resolves-all-id-only-inputs.tape` | 462 B | AC-001 |
| `AC-001-resolves-all-id-only-inputs.gif` | 89 KB | AC-001 |
| `AC-001-resolves-all-id-only-inputs.webm` | 125 KB | AC-001 |
| `AC-002-concurrency-capped-at-eight.tape` | 462 B | AC-002 |
| `AC-002-concurrency-capped-at-eight.gif` | 90 KB | AC-002 |
| `AC-002-concurrency-capped-at-eight.webm` | 121 KB | AC-002 |
| `AC-003-retry-works-through-cap.tape` | 450 B | AC-003 |
| `AC-003-retry-works-through-cap.gif` | 88 KB | AC-003 |
| `AC-003-retry-works-through-cap.webm` | 128 KB | AC-003 |
| `AC-004-constant-is-eight.tape` | 401 B | AC-004 |
| `AC-004-constant-is-eight.gif` | 79 KB | AC-004 |
| `AC-004-constant-is-eight.webm` | 118 KB | AC-004 |

---

## Coverage: 4/4 ACs demonstrated. No deviations.
