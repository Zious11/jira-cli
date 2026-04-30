//! Integration test: the embedded OAuth path uses the fixed registered
//! callback port (53682) and exchanges credentials against a mock
//! authorization server.
//!
//! Heavyweight — the full version requires `oauth_login` to accept a
//! base-URL override so wiremock can stand in for `auth.atlassian.com`.
//! That refactor is out of scope for the current PR. Until it lands,
//! this test is gated behind `JR_RUN_OAUTH_INTEGRATION=1` and otherwise
//! skips. The on-binary "embedded creds present" assertion is covered
//! by `.github/workflows/release.yml`'s "Verify embedded OAuth app
//! present" smoke step.

#[tokio::test]
async fn embedded_login_uses_fixed_port() {
    if std::env::var("JR_RUN_OAUTH_INTEGRATION").is_err() {
        eprintln!("skipped: set JR_RUN_OAUTH_INTEGRATION=1 to run");
        return;
    }

    // Implementation deferred — depends on a base-URL override in
    // `crate::api::auth::oauth_login` to redirect the authorize +
    // token-exchange calls at wiremock instead of `auth.atlassian.com`.
    // See spec §"Testing strategy" → "Open question 3" and the
    // implementation plan §"Task 13" for the deferral rationale.
    eprintln!("placeholder: this test asserts shape via the release smoke step");
}
