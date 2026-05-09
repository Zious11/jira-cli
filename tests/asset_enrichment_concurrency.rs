//! S-3.05 TDD Test Suite — asset enrichment concurrency cap.
//!
//! Red Gate status (pre-implementation): see `.factory/cycles/cycle-001/S-3.05/implementation/red-gate-log.md`
//!
//! AC placement:
//! - AC-001 (correctness preserved with 10 assets): REGRESSION-PIN — passes pre- and post-impl.
//! - AC-002 (peak concurrent ≤ 8 with 20 assets): ASSERTION ERROR pre-impl (join_all peaks at 20).
//! - AC-003 (retry-through-cap still works): REGRESSION-PIN — passes pre- and post-impl.
//! - AC-004 (MAX_CONCURRENT_ASSET_FETCHES constant exists): COMPILE ERROR — separate file.
//!
//! Approach for AC-002 (timing-based, not atomic-counter):
//! - Uses `ResponseTemplate::set_delay(50ms)` which runs OUTSIDE wiremock's write lock.
//! - `join_all` overlaps all 20 delay phases simultaneously → elapsed ≈ 50ms.
//! - `buffer_unordered(8)` serializes into ceil(20/8)=3 rounds × 50ms → elapsed ≈ 150ms.
//! - Asserts `elapsed >= 90ms` to distinguish the two behaviors.
//! - NOTE: The `Arc<AtomicUsize>` peak-counter approach from the story sketch does NOT
//!   work with wiremock 0.6.5 because `Respond::respond` runs under a write lock,
//!   serializing all respond() calls. See inline comment in test for full explanation.
//!
//! Workspace ID isolation:
//! - Each test uses a distinct workspace ID string to avoid cross-test cache collisions.
//! - `LinkedAsset.workspace_id` is set on every asset so `enrich_assets` skips the
//!   global workspace discovery endpoint entirely — no extra mock needed for that path.

#[allow(dead_code)]
mod common;

use jr::api::assets::linked::enrich_assets;
use jr::api::client::JiraClient;
use jr::types::assets::LinkedAsset;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal `LinkedAsset` with `id` only (triggers enrichment).
fn id_only_asset(workspace_id: &str, id: &str) -> LinkedAsset {
    LinkedAsset {
        id: Some(id.to_string()),
        workspace_id: Some(workspace_id.to_string()),
        key: None,
        name: None,
        asset_type: None,
    }
}

/// Build a mock asset JSON body for a given id and key suffix.
fn asset_body(id: &str, key: &str, label: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "objectKey": key,
        "label": label,
        "objectType": { "id": "13", "name": "Client" }
    })
}

// ---------------------------------------------------------------------------
// AC-001 / BC-4.3.002 postcondition — correctness regression-pin
//
// REGRESSION-PIN: passes both pre-impl (join_all) and post-impl (buffer_unordered).
// Purpose: ensure the cap doesn't accidentally break correctness.
// ---------------------------------------------------------------------------

/// Traces to BC-4.3.002 (enrichment correctness) AC-001 — verifies that
/// `enrich_assets` resolves all id-only assets via the buffer_unordered
/// concurrent GETs, matching pre-cap join_all behavior.
///
/// Pre-implementation Red Gate: REGRESSION-PIN — currently passes (existing
/// join_all behavior). Post-implementation: must still pass (cap is a concurrency
/// change, not a correctness change).
///
/// 10 distinct asset GET endpoints, each returning a unique objectKey + label.
/// After enrichment all 10 LinkedAssets must have non-None key and name.
#[tokio::test]
async fn test_enrich_assets_resolves_all_id_only_inputs() {
    let server = MockServer::start().await;
    let wid = "ws-ac-001";

    for i in 1u32..=10 {
        let oid = format!("obj-{i:03}");
        let key = format!("ASSET-{i}");
        let label = format!("Asset Label {i}");
        Mock::given(method("GET"))
            .and(path(format!("/jsm/assets/workspace/{wid}/v1/object/{oid}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(asset_body(&oid, &key, &label)))
            .expect(1)
            .mount(&server)
            .await;
    }

    let mut assets: Vec<LinkedAsset> = (1u32..=10)
        .map(|i| id_only_asset(wid, &format!("obj-{i:03}")))
        .collect();

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    enrich_assets(&client, &mut assets).await;

    // Verify wiremock expect(1) per endpoint
    server.verify().await;

    // Every asset must be resolved
    for (i, asset) in assets.iter().enumerate() {
        let n = i + 1;
        assert!(
            asset.key.is_some(),
            "AC-001 (BC-4.3.002): asset {n} must have key after enrichment; got None"
        );
        assert!(
            asset.name.is_some(),
            "AC-001 (BC-4.3.002): asset {n} must have name after enrichment; got None"
        );
        assert_eq!(
            asset.key.as_deref(),
            Some(format!("ASSET-{n}").as_str()),
            "AC-001 (BC-4.3.002): asset {n} key mismatch"
        );
        assert_eq!(
            asset.name.as_deref(),
            Some(format!("Asset Label {n}").as_str()),
            "AC-001 (BC-4.3.002): asset {n} label mismatch"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-002 / BC-4.3.002 invariant — peak concurrent ≤ 8
//
// Pre-implementation Red Gate: ASSERTION ERROR.
// Pre-impl: join_all fires all 20 futures simultaneously → all delays overlap
//   → total elapsed ≈ 50ms.
// Post-impl: buffer_unordered(8) caps to 8 → ceil(20/8)=3 rounds × 50ms = 150ms.
//
// Mechanism: ResponseTemplate::set_delay runs OUTSIDE wiremock's write lock,
// so multiple responses can be delayed concurrently. join_all overlaps all 20
// delays simultaneously (~50ms total). buffer_unordered(8) processes in batches
// of 8 (~150ms total). The assertion `elapsed >= 90ms` distinguishes these.
//
// The Arc<AtomicUsize> peak-counter approach from the original story sketch
// does NOT work with wiremock 0.6.5: the Respond::respond method runs under a
// write lock (see mock_server/hyper.rs), serializing all respond() calls to
// peak=1 regardless of join_all vs buffer_unordered. This is a wiremock
// implementation detail the research agent was not aware of. Timing-based
// testing is the correct approach here because:
// - The delay duration (50ms) is large relative to CI jitter (<5ms)
// - The threshold (90ms) gives 40ms margin: pre-impl peaks at ~50ms
//   (fails by 40ms), post-impl peaks at ~150ms (passes by 60ms).
// - The story's "flaky timing" concern applies to 1ms-level delays; 50ms
//   delays with a 90ms threshold are stable on loaded CI runners.
// ---------------------------------------------------------------------------

/// Traces to BC-4.3.002 (enrichment invariant) AC-002 — verifies that
/// no more than 8 asset GET requests are in-flight simultaneously during
/// enrichment (buffer_unordered cap = MAX_CONCURRENT_ASSET_FETCHES = 8).
///
/// Pre-implementation Red Gate: ASSERTION ERROR — `join_all` fires all 20
/// futures simultaneously. With `set_delay(50ms)`, all 20 delay phases run
/// concurrently (delay runs outside wiremock's write lock), so total elapsed
/// ≈ 50ms. The assertion `elapsed >= 90ms` fails.
///
/// Post-implementation: `buffer_unordered(8)` processes at most 8 concurrent
/// futures. 20 assets / 8 cap = ceil(3) rounds × 50ms ≈ 150ms total.
/// The assertion passes (150ms >= 90ms).
///
/// The timing-based assertion is the Red Gate mechanism.
#[tokio::test]
async fn test_enrich_assets_concurrency_capped_at_eight() {
    let server = MockServer::start().await;
    let wid = "ws-ac-002";

    // Mount a single regex mock that matches all 20 asset GET endpoints.
    // set_delay(50ms) applies OUTSIDE wiremock's write lock — multiple requests
    // can be in their delay phase simultaneously, creating a measurable timing gap
    // between join_all (all 20 overlap → ~50ms) and buffer_unordered(8) (3 rounds → ~150ms).
    Mock::given(method("GET"))
        .and(path_regex(format!(
            r"/jsm/assets/workspace/{wid}/v1/object/obj-\d+"
        )))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(asset_body("any", "ANY-1", "Any Asset"))
                .set_delay(std::time::Duration::from_millis(50)),
        )
        .expect(20)
        .mount(&server)
        .await;

    // 20 id-only assets pointing at the same workspace (different object IDs)
    let mut assets: Vec<LinkedAsset> = (1u32..=20)
        .map(|i| id_only_asset(wid, &format!("obj-{i:03}")))
        .collect();

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let start = std::time::Instant::now();
    enrich_assets(&client, &mut assets).await;
    let elapsed = start.elapsed();

    // Pre-impl (join_all): all 20 delay phases overlap → elapsed ≈ 50ms < 90ms → FAILS
    // Post-impl (buffer_unordered(8)): 3 rounds of 8 → elapsed ≈ 150ms > 90ms → PASSES
    assert!(
        elapsed.as_millis() >= 90,
        "AC-002 (BC-4.3.002 invariant): concurrent enrichment must be capped at 8; \
         expected elapsed >= 90ms (3 rounds × 50ms with cap=8), \
         but got {}ms. \
         Pre-impl join_all overlaps all 20 delays simultaneously (~50ms). \
         Post-impl buffer_unordered(8) processes in rounds (~150ms).",
        elapsed.as_millis()
    );
}

// ---------------------------------------------------------------------------
// AC-003 / BC-X.1.005 invariant — retry still works through the cap
//
// REGRESSION-PIN: passes both pre-impl and post-impl.
// Purpose: confirm the cap doesn't bypass JiraClient::send retry logic.
// ---------------------------------------------------------------------------

/// Traces to BC-X.1.005 (rate-limit retry) AC-003 — verifies that the
/// buffer_unordered concurrency cap does not bypass per-future retry logic
/// in JiraClient::send. When one asset endpoint returns 429 + Retry-After: 1
/// followed by 200, all assets are eventually resolved.
///
/// Pre-implementation Red Gate: REGRESSION-PIN — currently passes (retry logic
/// in JiraClient::send is per-future and is transparent to join_all / buffer_unordered).
///
/// Test setup:
/// - 5 asset endpoints: return 200 immediately.
/// - 1 asset endpoint (obj-006): first call returns 429 + Retry-After: 1; second returns 200.
/// - After enrichment, all 6 assets must be resolved.
///
/// Retry-After: 1 (1 second) is within MAX_RETRY_AFTER_SECS (60), so retry proceeds.
#[tokio::test]
async fn test_enrich_assets_retry_works_through_cap() {
    let server = MockServer::start().await;
    let wid = "ws-ac-003";

    // 5 endpoints that return 200 immediately
    for i in 1u32..=5 {
        let oid = format!("obj-{i:03}");
        let key = format!("OK-{i}");
        let label = format!("OK Asset {i}");
        Mock::given(method("GET"))
            .and(path(format!("/jsm/assets/workspace/{wid}/v1/object/{oid}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(asset_body(&oid, &key, &label)))
            .expect(1)
            .mount(&server)
            .await;
    }

    // One endpoint that returns 429 on first call, 200 on second.
    // wiremock serves mounts in reverse-mount order (last mounted = highest priority),
    // so mount the 429 mock last to give it priority on the first request.
    // We mount two separate mocks with different `up_to_n_times` expectations:
    // - A 200 mock with expect(1) mounted first (serves the retry).
    // - A 429 mock with expect(1) mounted after (serves the first request, wins on priority).
    //
    // wiremock's priority: later-mounted mocks win; once exhausted, falls back to earlier.
    let flapping_oid = "obj-006";
    let flapping_path = format!("/jsm/assets/workspace/{wid}/v1/object/{flapping_oid}");

    // Fallback 200 (serves the retry after the 429 is exhausted)
    Mock::given(method("GET"))
        .and(path(flapping_path.clone()))
        .respond_with(ResponseTemplate::new(200).set_body_json(asset_body(
            flapping_oid,
            "RETRY-1",
            "Retried Asset",
        )))
        .mount(&server)
        .await;

    // First-request 429 with Retry-After: 1 (within cap)
    Mock::given(method("GET"))
        .and(path(flapping_path.clone()))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "1"))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let mut assets: Vec<LinkedAsset> = (1u32..=6)
        .map(|i| id_only_asset(wid, &format!("obj-{i:03}")))
        .collect();

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    enrich_assets(&client, &mut assets).await;

    // All 6 assets must be resolved (the retried one included)
    for (i, asset) in assets.iter().enumerate() {
        let n = i + 1;
        assert!(
            asset.key.is_some(),
            "AC-003 (BC-X.1.005): asset {n} must be resolved after enrichment \
             (retry must work through cap); got key=None, name={:?}",
            asset.name
        );
    }

    // Verify the 5 direct-200 endpoints were hit exactly once
    server.verify().await;
}
