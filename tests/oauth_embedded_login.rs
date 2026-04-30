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
    // Opting in to the integration suite without an actual implementation
    // would silently pass and create a false coverage signal. Fail loudly
    // until the deferred wiremock work lands (see spec's "Deferred
    // coverage" section at docs/superpowers/specs/2026-04-30-embedded-
    // oauth-app-design.md).
    unimplemented!(
        "JR_RUN_OAUTH_INTEGRATION=1 enabled but embedded OAuth integration \
         test is not implemented yet; needs the base-URL override in \
         oauth_login before a real wiremock-backed assertion can be written."
    );
}
