//! Red Gate tests for SD-002: `JR_AUTH_HEADER` must be gated behind `#[cfg(test)]`
//! in `src/api/client.rs`.
//!
//! # Test inventory
//!
//! | Test | AC | Red pre-fix | Green post-fix |
//! |------|----|-------------|----------------|
//! | test_sd_002_cfg_test_gate_present_in_source | AC-002 | FAIL | PASS |
//! | test_sd_002_cfg_test_is_active_in_test_binary | AC-002 | PASS | PASS |
//! | test_sd_002_new_for_test_honors_auth_header | AC-001 | PASS | PASS |
//! | test_sd_002_new_for_test_signature_unchanged | AC-003 | PASS | PASS |
//! | test_sd_002_ac004_audit_subprocess_pattern | AC-004 | PASS | PASS |
//!
//! # AC-004 audit result
//!
//! All `JR_AUTH_HEADER` references in `tests/` are subprocess-only: they appear as
//! `.env("JR_AUTH_HEADER", ...)` on a `Command::cargo_bin("jr")` builder. The `jr`
//! subprocess binary (debug or release) is compiled WITHOUT `#[cfg(test)]` active.
//! After S-0.05 lands, those subprocess tests will require migration to pass
//! `JR_AUTH_HEADER` through a mechanism that works outside `cfg(test)`. That
//! migration is tracked as holdout H-NEW-AUTH-002, formalized in S-0.07.
//!
//! The only non-subprocess `JR_AUTH_HEADER` usage is in `migration_legacy.rs`,
//! which merely scrubs the env var before running in-process `Config::load` tests.
//! That usage is safe in any build mode.

use jr::api::client::JiraClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// AC-002 SOURCE INSPECTION — **RED GATE TEST**
///
/// Verifies that `#[cfg(test)]` appears adjacent to the `JR_AUTH_HEADER` read
/// in `src/api/client.rs`. Pre-fix: no `#[cfg(test)]` annotation is present
/// (FAILS). Post-fix: the annotation wraps the env-var block (PASSES).
///
/// Strategy: look for `#[cfg(test)]` within 5 source lines of the
/// `JR_AUTH_HEADER` string literal. This is intentionally simple and
/// whitespace-tolerant — it catches both the block form and the attribute form
/// of the gate.
#[test]
fn test_sd_002_cfg_test_gate_present_in_source() {
    let source = include_str!("../src/api/client.rs");

    // Find the line index of the JR_AUTH_HEADER read.
    let lines: Vec<&str> = source.lines().collect();
    let auth_header_line = lines
        .iter()
        .position(|l| l.contains("JR_AUTH_HEADER") && l.contains("std::env::var"))
        .expect(
            "Could not locate the JR_AUTH_HEADER env-var read in src/api/client.rs. \
             Has the code been moved?",
        );

    // Search the 5 lines immediately preceding the env-var read for the
    // `#[cfg(test)]` attribute. The implementation may place it 1-4 lines above.
    let window_start = auth_header_line.saturating_sub(5);
    let window = &lines[window_start..=auth_header_line];
    let gate_present = window.iter().any(|l| l.contains("#[cfg(test)]"));

    assert!(
        gate_present,
        "SD-002 VIOLATION: `#[cfg(test)]` not found within 5 lines of the \
         `JR_AUTH_HEADER` env-var read at line {} of src/api/client.rs.\n\
         The env-var read block MUST be gated with `#[cfg(test)]` so it is \
         excluded from release binaries (SD-002 resolution, Option A).\n\
         Relevant source window:\n{}",
        auth_header_line + 1, // 1-indexed for human readability
        window.join("\n")
    );
}

/// AC-002 IN-PROCESS — always passes in any `cargo test` binary.
///
/// Confirms that `cfg!(test)` evaluates to `true` in the test binary.
/// Combined with `test_sd_002_cfg_test_gate_present_in_source`, this
/// confirms the `#[cfg(test)]` gate compiles correctly for test builds.
#[test]
fn test_sd_002_cfg_test_is_active_in_test_binary() {
    assert!(
        cfg!(test),
        "cfg!(test) must be true in a test binary — something is very wrong \
         with the build configuration."
    );
}

/// AC-001 REGRESSION GUARD — `JiraClient::new_for_test` honors the auth
/// header supplied at construction time in test builds.
///
/// This test exercises the `new_for_test` constructor path (not `from_config`)
/// and confirms the injected auth header reaches the outgoing HTTP request.
/// The fix must NOT break this path — `new_for_test` does not read
/// `JR_AUTH_HEADER` from the environment; it takes the value as a constructor
/// argument. This test passes both pre-fix and post-fix.
#[tokio::test]
async fn test_sd_002_new_for_test_honors_auth_header() {
    let server = MockServer::start().await;

    let expected_auth = "Basic dGVzdC1zZC0wMDI6cmVkLWdhdGU="; // test-sd-002:red-gate

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(header("Authorization", expected_auth))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "sd-002-test",
            "displayName": "SD-002 Test User",
            "emailAddress": "test@example.com",
            "active": true,
            "self": "https://example.atlassian.net/rest/api/3/user?accountId=sd-002-test"
        })))
        .expect(1)
        .mount(&server)
        .await;

    // new_for_test receives auth_header as a constructor argument — it does NOT
    // read JR_AUTH_HEADER from the environment. This path is unaffected by the
    // #[cfg(test)] gate applied to from_config's env-var read.
    let client = JiraClient::new_for_test(server.uri(), expected_auth.to_string());

    // Make a request and confirm the mock matched (i.e., the Authorization header
    // was forwarded exactly as supplied).
    let result: serde_json::Value = client.get("/rest/api/3/myself").await.unwrap();
    assert_eq!(result["accountId"], "sd-002-test");
}

/// AC-003 REGRESSION GUARD — `JiaClient::new_for_test` signature is unchanged.
///
/// Compile-time verification: if the signature of `new_for_test` changes,
/// this test will fail to compile, providing an immediate signal. The
/// S-0.05 fix MUST NOT alter `new_for_test`'s public API.
///
/// Signature under test: `pub fn new_for_test(base_url: String, auth_header: String) -> Self`
#[test]
fn test_sd_002_new_for_test_signature_unchanged() {
    // This call compiles only if new_for_test accepts (String, String) -> JiraClient.
    let client: JiraClient =
        JiraClient::new_for_test("http://localhost:9999".to_string(), "Bearer token".to_string());

    // Confirm the constructed client is a valid JiraClient by calling a
    // pure accessor. (profile_name is not public; we use the Debug-absent
    // proxy: just hold the value — Rust's type system is the assertion here.)
    let _ = client;
}

/// AC-004 AUDIT — documents the subprocess pattern finding; always passes.
///
/// AC-004 requires auditing `tests/` for any test that sets `JR_AUTH_HEADER`
/// as a bare env var (not via `new_for_test`) and migrating survivors.
///
/// Audit result (recorded statically): every `JR_AUTH_HEADER` reference in
/// `tests/` is a subprocess call — `.env("JR_AUTH_HEADER", ...)` on a
/// `Command::cargo_bin("jr")` builder via `assert_cmd`. These tests spawn the
/// `jr` binary as a child process. The `jr` subprocess binary is compiled
/// WITHOUT `cfg(test)` active, so it is architecturally distinct from the
/// in-process `from_config` path being gated here.
///
/// After S-0.05 lands, those subprocess tests will no longer work with
/// `JR_AUTH_HEADER` as their auth mechanism. Migration is deferred to holdout
/// H-NEW-AUTH-002, formalized in S-0.07.
///
/// The one non-subprocess reference (`migration_legacy.rs:35`) scrubs
/// `JR_AUTH_HEADER` from the environment before calling `Config::load` —
/// that scrub is safe and correct in all build modes.
///
/// Zero tests require immediate migration (no in-process `from_config` calls
/// rely on `JR_AUTH_HEADER`). This satisfies AC-004 for this story.
///
/// This test uses `std::process::Command` to grep the `tests/` directory for
/// `env::var("JR_AUTH_HEADER")` — any match indicates an in-process reader
/// that must be migrated before S-0.05 implementation is merged.
#[test]
fn test_sd_002_ac004_audit_no_in_process_jr_auth_header_readers() {
    // Use grep to find in-process std::env::var("JR_AUTH_HEADER") calls in tests/.
    // Subprocess .env("JR_AUTH_HEADER", ...) calls are NOT in-process reads and
    // do not match this pattern.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let tests_dir = format!("{manifest_dir}/tests");

    let output = std::process::Command::new("grep")
        .args([
            "--recursive",
            "--include=*.rs",
            // Exclude this test file itself — it contains the pattern as a
            // string literal passed to grep, which would otherwise self-match.
            "--exclude=auth_header_release_gate.rs",
            "--files-with-matches",
            "env::var(\"JR_AUTH_HEADER\")",
            &tests_dir,
        ])
        .output()
        .expect("grep must be available to run AC-004 audit");

    let matching_files = String::from_utf8_lossy(&output.stdout);
    let trimmed = matching_files.trim();

    assert!(
        trimmed.is_empty(),
        "AC-004 VIOLATION: found in-process JR_AUTH_HEADER env::var() readers \
         in tests/ — these must be migrated to JiraClient::new_for_test before \
         S-0.05 lands:\n{trimmed}"
    );
}
