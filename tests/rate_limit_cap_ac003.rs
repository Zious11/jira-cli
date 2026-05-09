//! S-3.07 AC-003 — MAX_RETRY_AFTER_SECS constant existence check.
//!
//! Isolated in its own file so its compile-error Red Gate does not block
//! the assertion-error Red Gates in `tests/rate_limit_cap_tests.rs`.
//!
//! Pre-implementation Red Gate: COMPILE ERROR — `MAX_RETRY_AFTER_SECS` does not
//! exist in `jr::api::rate_limit`. The constant must be `pub` or `pub(crate)`.
//!
//! Post-implementation: compiles and the value assertion holds.

/// BC-X.4.009 invariant: `MAX_RETRY_AFTER_SECS` is defined in `jr::api::rate_limit`
/// with a value of exactly 60 (interactive-CLI fail-fast trade-off per RFC 9110 §10.2.3).
///
/// Pre-implementation Red Gate: COMPILE ERROR — symbol does not exist.
/// Post-implementation: compiles and assertion holds.
#[test]
fn ac_003_max_retry_after_secs_constant_defined() {
    use jr::api::rate_limit::MAX_RETRY_AFTER_SECS;
    assert_eq!(
        MAX_RETRY_AFTER_SECS, 60u64,
        "AC-003 (BC-X.4.009 invariant): MAX_RETRY_AFTER_SECS must equal 60; \
         cap rationale: interactive-CLI fail-fast trade-off per RFC 9110 §10.2.3"
    );
}
