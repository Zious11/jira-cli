//! S-3.05 AC-004 — MAX_CONCURRENT_ASSET_FETCHES constant existence check.
//!
//! Isolated in its own file so its compile-error Red Gate does not block
//! the assertion-error Red Gates in `tests/asset_enrichment_concurrency.rs`.
//!
//! Pre-implementation Red Gate: COMPILE ERROR — `MAX_CONCURRENT_ASSET_FETCHES`
//! does not exist in `jr::api::assets::linked`. The constant must be `pub` or
//! `pub(crate)` and visible from integration tests via `jr::`.
//!
//! Post-implementation: compiles and the value assertion holds.

/// BC-4.3.002 postcondition: `MAX_CONCURRENT_ASSET_FETCHES` is defined in
/// `jr::api::assets::linked` with a value of exactly 8.
///
/// Pre-implementation Red Gate: COMPILE ERROR — symbol does not exist.
/// Post-implementation: compiles and assertion holds.
#[test]
fn ac_004_max_concurrent_asset_fetches_constant_defined() {
    use jr::api::assets::linked::MAX_CONCURRENT_ASSET_FETCHES;
    assert_eq!(
        MAX_CONCURRENT_ASSET_FETCHES, 8usize,
        "AC-004 (BC-4.3.002 postcondition): MAX_CONCURRENT_ASSET_FETCHES must equal 8; \
         cap rationale: conservative good-neighbor default against Atlassian's 100 req/s \
         GET burst limit (verified 2026-05-08)."
    );
}
