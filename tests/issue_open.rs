/// Integration tests for `handle_open` URL construction (BC-3.4.001, H-046).
///
/// All three tests call `JiraClient::new_for_test_with_instance_url` to set the
/// Red Gate: the stub body is `todo!()`, so every test panics until the implementer
/// fills in the constructor.  Once the stub is filled in:
///
///   - test_bc_3_4_001_oauth_uses_instance_url  verifies that instance_url() diverges
///     from base_url() and that the correct browse URL is formed using instance_url().
///   - test_bc_3_4_001_api_token_regression_guard  runs the `jr issue open --url-only`
///     binary to confirm api-token mode still produces a correct URL (AC-002).
///   - test_bc_3_4_001_no_double_slash  runs the binary with a trailing-slash instance
///     URL to confirm no double-slash in the output (AC-003).
///
/// H-046 alignment:
///   - H-046 MUST-FAIL at dea1664 (baseline): all three tests fail (todo!() panic).
///   - H-046 MUST-PASS after merge: all three tests pass.
#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use predicates::prelude::*;

/// AC-001 (BC-3.4.001 postcondition): handle_open MUST use instance_url() to compose
/// the browse URL.  Given an OAuth profile where base_url() returns the API gateway
/// (https://api.atlassian.com/ex/jira/<cloudId>) and instance_url() returns the
/// real *.atlassian.net URL, the composed URL must contain the instance URL, NOT
/// the API gateway URL.
///
/// Red Gate: panics at new_for_test_with_instance_url (todo!() body).
/// Bug state: after stub is filled, client.base_url() != client.instance_url();
///   verifying instance_url() is used catches the handle_open bug.
/// Green Gate: passes after stub implementation + handle_open fix.
#[test]
fn test_bc_3_4_001_oauth_uses_instance_url() {
    let api_base = "https://api.atlassian.com/ex/jira/my-cloud-123";
    let instance_url = "https://mycompany.atlassian.net";

    // Red Gate: this panics until the implementer fills in the stub.
    let client = jr::api::client::JiraClient::new_for_test_with_instance_url(
        api_base,
        instance_url,
        "Bearer fake-token",
    );

    // After stub is filled in:
    // 1. Verify the two URLs are distinct (test is meaningful).
    assert_ne!(
        client.instance_url(),
        client.base_url(),
        "instance_url() and base_url() must differ for this test to be meaningful"
    );

    // 2. Verify instance_url() returns the real *.atlassian.net URL.
    assert_eq!(
        client.instance_url(),
        instance_url,
        "instance_url() must return the configured instance URL, not the OAuth API gateway"
    );

    // 3. Verify base_url() returns the OAuth API gateway URL.
    assert_eq!(
        client.base_url(),
        api_base,
        "base_url() must return the OAuth API gateway URL"
    );

    // 4. Verify the browse URL composed using instance_url() is correct (BC-3.4.001
    //    postcondition: URL = <instance_url>/browse/<key>).
    let browse_url = format!("{}/browse/PROJ-123", client.instance_url());
    assert!(
        browse_url.contains("mycompany.atlassian.net"),
        "browse URL must contain the *.atlassian.net host, got: {browse_url}"
    );
    assert!(
        !browse_url.contains("api.atlassian.com"),
        "browse URL must NOT contain the API gateway host, got: {browse_url}"
    );
    assert_eq!(
        browse_url, "https://mycompany.atlassian.net/browse/PROJ-123",
        "browse URL must be exactly <instance_url>/browse/<key>"
    );
}

/// AC-002 (BC-3.4.001 postcondition): api-token auth mode is unaffected by the fix.
/// Given an api-token profile, `jr issue open PROJ-123 --url-only` must still emit
/// the configured instance URL.
///
/// Red Gate: panics at new_for_test_with_instance_url (todo!() body).
/// Green Gate: passes — binary output contains mock_instance/browse/PROJ-123.
#[tokio::test]
async fn test_bc_3_4_001_api_token_regression_guard() {
    // Red Gate: call the stub to establish the failing baseline.
    // After the stub is filled in, this no-op call returns a client we don't use
    // further; the real assertion runs via assert_cmd against the binary.
    let _guard_client = jr::api::client::JiraClient::new_for_test_with_instance_url(
        "https://api.atlassian.com/ex/jira/cloud-abc",
        "https://mycompany.atlassian.net",
        "Basic dGVzdDp0ZXN0",
    );

    // With JR_BASE_URL set the binary uses it for both base_url and instance_url,
    // simulating an api-token profile.  `jr issue open --url-only` makes no HTTP
    // calls, so no mock server is needed.
    let instance_url = "https://mycompany.atlassian.net";

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", instance_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "issue", "open", "PROJ-123", "--url-only"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "https://mycompany.atlassian.net/browse/PROJ-123",
        ))
        .stdout(predicate::str::contains("api.atlassian.com").not());
}

/// AC-003 (BC-3.4.001 postcondition): trailing slash on instance_url must not
/// produce a double-slash in the composed browse URL.
/// Expected: https://mycompany.atlassian.net/browse/PROJ-123
/// Not:      https://mycompany.atlassian.net//browse/PROJ-123
///
/// Red Gate: panics at new_for_test_with_instance_url (todo!() body).
/// Green Gate: passes — binary output has single slash before "browse".
#[tokio::test]
async fn test_bc_3_4_001_no_double_slash() {
    // Red Gate: call the stub to establish the failing baseline.
    let _guard_client = jr::api::client::JiraClient::new_for_test_with_instance_url(
        "https://api.atlassian.com/ex/jira/cloud-abc",
        "https://mycompany.atlassian.net/",
        "Basic dGVzdDp0ZXN0",
    );

    // JR_BASE_URL with a trailing slash; from_config trims it.
    let instance_url_with_slash = "https://mycompany.atlassian.net/";

    Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", instance_url_with_slash)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "issue", "open", "PROJ-123", "--url-only"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "https://mycompany.atlassian.net/browse/PROJ-123",
        ))
        .stdout(predicate::str::contains("//browse").not());
}
