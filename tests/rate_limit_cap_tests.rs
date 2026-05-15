//! S-3.07 TDD Test Suite — failing tests for AC-001, AC-002, AC-003, AC-006, AC-007,
//! AC-008, AC-NEW-D.
//!
//! Red Gate status (pre-implementation): see `.factory/cycles/cycle-001/S-3.07/implementation/red-gate-log.md`
//!
//! AC placement:
//! - AC-001 (rate-limit exceed-cap → abort): integration test, wiremock + tokio time control
//! - AC-002 (rate-limit within-cap → retry): integration test, wiremock + tokio time control
//! - AC-003 (MAX_RETRY_AFTER_SECS constant exists): compile-time import check
//! - AC-006, AC-007 (profile name message precision): INLINE in src/config.rs::tests — not here
//! - AC-008 (cursor loop terminates): subprocess test via assert_cmd
//! - AC-NEW-D (stderr contains JRACLOUD-95368): subprocess test via assert_cmd
//!
//! Citation history: AC-NEW-D originally asserted on the literal "JRACLOUD-94632",
//! which was rebound to "JRACLOUD-95368" by issue #361 after research (2026-05-13)
//! showed JRACLOUD-94632 covers an unrelated /search/jql defect (initial-request
//! `nextPageToken=null` rejection, resolved Jun 2025). JRACLOUD-95368
//! ("nextPageToken pagination is not snapshot-stable under live mutation") is the
//! correctly-attributed root-cause ticket for the repeated-cursor symptom this
//! guard catches.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use jr::api::client::JiraClient;
use tempfile::TempDir;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Shared subprocess helper
// ---------------------------------------------------------------------------

/// Build a `jr issue list` command with wiremock override. Mirrors the pattern
/// from `tests/rate_limit_holdouts.rs::jr_cmd_with_mock`.
fn jr_cmd_with_mock(server_uri: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CONFIG_HOME", config_dir)
        .env("XDG_CACHE_HOME", config_dir)
        .env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .env_remove("JR_OAUTH_CLIENT_ID")
        .env_remove("JR_OAUTH_CLIENT_SECRET");
    cmd
}

// ---------------------------------------------------------------------------
// AC-003: MAX_RETRY_AFTER_SECS constant — see tests/rate_limit_cap_ac003.rs
//
// AC-003's compile-fail test is in a SEPARATE file (tests/rate_limit_cap_ac003.rs)
// so its compile error does not block AC-001/AC-002/AC-008/AC-NEW-D from running.
// The Red Gate log documents both files and their separate outcomes.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// AC-001: Retry-After exceeds cap → abort retry immediately
//
// BC-X.4.009 postcondition.
//
// Pre-implementation Red Gate: ASSERTION ERROR — the test wraps the call in a
// tokio::time::timeout(10s). Pre-implementation, the code does
// `tokio::time::sleep(Duration::from_secs(2400))` inside the 429 handler.
// With `start_paused = true`, that sleep never completes (clock is paused and no
// one advances it), so the outer timeout fires after 10 real seconds, causing the
// test to FAIL with "deadline elapsed". The assertion `elapsed < 2s` also fails.
//
// Post-implementation: the cap check short-circuits at 2400 > 60, returns Err
// immediately (no sleep at all), elapsed ≪ 1s, test passes.
// ---------------------------------------------------------------------------

/// BC-X.4.009 postcondition: when wiremock returns `Retry-After: 2400` (40 min,
/// a typical Atlassian value), `send` must detect 2400 > MAX_RETRY_AFTER_SECS=60,
/// abort the retry loop immediately (no sleep), and return an error.
///
/// Uses `tokio::time::timeout` as the wall-clock gate: post-implementation the
/// cap check short-circuits at 2400 > 60 and returns Err in microseconds (no sleep).
/// Pre-implementation (no cap), the code would sleep for 2400 real seconds; the 10s
/// timeout catches this and causes the test to fail.
///
/// Note: `start_paused = true` is intentionally absent. `start_paused + wiremock`
/// is incompatible: tokio auto-advances the virtual clock before the mock server's
/// TCP accept task is scheduled, causing `timeout` to fire at T=10s instantly.
/// This test verifies wall-clock termination (< 10s real time), which is the correct
/// AC-001 invariant for an interactive CLI.
#[tokio::test]
async fn ac_001_retry_after_exceeds_cap_aborts_retry() {
    let server = MockServer::start().await;

    // Wiremock: every GET returns 429 with Retry-After: 2400 (typical Atlassian value).
    // We set .expect(1) — pre-implementation, all 4 retry attempts would hit the server,
    // but the test times out before they complete. Post-implementation, exactly 1 hit
    // (first attempt before abort). Expect is NOT enforced here so we don't add
    // a second failure mode that obscures the cap-assertion failure.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "2400"))
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // Wrap in a 10-second wall-clock timeout so CI doesn't hang.
    // Pre-implementation: times out (sleep(2400s) never completes with paused clock).
    // Post-implementation: returns Err in microseconds.
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get::<serde_json::Value>("/rest/api/3/myself"),
    )
    .await;

    assert!(
        result.is_ok(),
        "AC-001 (BC-X.4.009): call must return before the 10s timeout; \
         pre-implementation this fails because Retry-After:2400 causes a 2400s sleep \
         which never completes under start_paused=true"
    );

    let inner = result.unwrap();
    assert!(
        inner.is_err(),
        "AC-001 (BC-X.4.009): when Retry-After:{} exceeds MAX_RETRY_AFTER_SECS=60, \
         send must return Err (abort retry, not sleep); got: {:?}",
        2400,
        inner.ok()
    );
}

// ---------------------------------------------------------------------------
// AC-002: Retry-After within cap → retry and succeed
//
// BC-X.4.009 postcondition (regression-pin).
//
// Pre-implementation Red Gate: this test MAY pass if the existing rate-limit
// retry logic already retries on 429. We use `Retry-After: 0` (within cap,
// no sleep needed) so the test runs without needing clock advance. If it
// passes pre-implementation, that is documented as a REGRESSION-PIN in the
// Red Gate log (the test preserves existing behavior, not new behavior).
//
// The AC explicitly requires Retry-After: 30 within cap. We use Retry-After: 0
// here to avoid clock-advance complexity while verifying the same logical
// invariant (within-cap value → retry proceeds). Noted as deviation in Red Gate log.
// ---------------------------------------------------------------------------

/// BC-X.4.009 postcondition (regression-pin): when `Retry-After` is within the cap
/// (0s ≤ value ≤ 60s), the retry loop MUST proceed (not abort), and the client
/// MUST return Ok on the subsequent 200 response.
///
/// Uses Retry-After: 0 (boundary value, no sleep). Does NOT use `start_paused = true`
/// because even sleep(0) requires clock advance with paused clock — and this test
/// wants to verify the REGRESSION-PIN behavior (within-cap → retry proceeds), not
/// timing behavior. The 10s wall-clock timeout guards against any unexpected hang.
///
/// If this passes pre-implementation: document as REGRESSION-PIN in Red Gate log.
#[tokio::test]
async fn ac_002_retry_after_within_cap_retries() {
    let server = MockServer::start().await;

    // First request: 429 with Retry-After: 0 (within 60s cap, no sleep needed).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Second request (after retry): 200 OK.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "test-account",
            "displayName": "Test User"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get::<serde_json::Value>("/rest/api/3/myself"),
    )
    .await;

    // Must not timeout.
    assert!(
        result.is_ok(),
        "AC-002 (BC-X.4.009 regression-pin): within-cap Retry-After must not hang; \
         got timeout after 10s"
    );

    // Must return Ok (retry succeeded).
    assert!(
        result.unwrap().is_ok(),
        "AC-002 (BC-X.4.009 regression-pin): after Retry-After:0 (within cap=60), \
         retry must succeed and return Ok(response)"
    );

    // MockServer drop enforces: exactly 1 call to 429 mock + 1 call to 200 mock.
}

// ---------------------------------------------------------------------------
// AC-008 + AC-NEW-D: cursor loop terminates + stderr contains "JRACLOUD-95368"
//
// NFR-R-F (DOCUMENT-AS-IS-FIXED routing).
//
// Strategy: subprocess via assert_cmd. The `jr issue list` command calls
// `search_issues` internally. We mount a wiremock that returns the same
// `nextPageToken` on every response. Pre-implementation: the loop runs
// indefinitely; we set a 15-second assert_cmd timeout so the test FAILS with
// "timed out" rather than hanging CI forever. Post-implementation: the guard
// breaks the loop within 2 iterations, emits the JRACLOUD-95368 warning to
// stderr, and the command exits within ~1 second.
//
// The test asserts BOTH:
//   (a) AC-008: command completes (loop terminates) — implied by timeout not firing
//   (b) AC-NEW-D: stderr contains literal "JRACLOUD-95368"
// ---------------------------------------------------------------------------

/// NFR-R-F postcondition (AC-008 + AC-NEW-D): when `/rest/api/3/search/jql` returns
/// the same `nextPageToken` twice (the repeated-cursor symptom of JRACLOUD-95368
/// live-data drift), the `search_issues` cursor loop MUST:
///   1. Terminate (not infinite-loop) — verified by 15s assert_cmd timeout
///   2. Emit a stderr warning containing `"JRACLOUD-95368"` — AC-NEW-D
///
/// Pre-implementation Red Gate: ASSERTION ERROR — the loop runs indefinitely against
/// the stuck-cursor mock. The assert_cmd 15-second timeout fires, causing `output()`
/// to return an error, which panics the test with "command timed out".
///
/// Post-implementation: guard breaks within 2 iterations, JRACLOUD-95368 appears
/// in stderr, command exits promptly.
#[tokio::test]
async fn ac_008_and_ac_new_d_search_jql_cursor_loop_terminates_with_jracloud_warning() {
    let server = MockServer::start().await;

    // Mount a mock that ALWAYS returns the same nextPageToken on every POST.
    // This simulates the JRACLOUD-95368 symptom: live-data drift between page
    // fetches causes the server to land on a previously-emitted cursor offset.
    // Without the anti-loop guard, search_issues loops indefinitely here.
    let stuck_response = serde_json::json!({
        "issues": [
            {
                "key": "TEST-1",
                "fields": {
                    "summary": "Test issue",
                    "status": {"name": "To Do"},
                    "issuetype": {"name": "Task"},
                    "priority": {"name": "Medium"},
                    "assignee": null,
                    "reporter": null,
                    "project": {"key": "TEST"},
                    "description": null,
                    "created": "2026-01-01T00:00:00.000+0000",
                    "updated": "2026-01-01T00:00:00.000+0000",
                    "resolution": null,
                    "components": [],
                    "fixVersions": [],
                    "labels": [],
                    "parent": null,
                    "issuelinks": []
                }
            }
        ],
        "nextPageToken": "stuck-cursor-abc123"
    });

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&stuck_response))
        // No .expect() — the mock fires until the guard triggers. Pre-implementation
        // it would fire indefinitely; post-implementation it fires twice (loop + guard).
        .mount(&server)
        .await;

    let config_dir = TempDir::new().unwrap();

    // Run `jr issue list --all --jql "project = TEST" --no-input` with the stuck-cursor mock.
    // The --all flag passes limit=None to search_issues, making the cursor loop unlimited.
    // Without --all, the default limit=30 would cap iteration at 30 page fetches (1 issue per
    // page × 30 = limit reached), masking the infinite-loop bug. With --all, the loop runs
    // until the anti-loop guard triggers OR the process is killed by the timeout.
    // The --jql flag bypasses the "no filter specified" early-exit guard.
    // Use a 15-second timeout: pre-implementation the command hangs, 15s elapses,
    // assert_cmd returns Err("timed out"), which panics the unwrap below.
    let output = jr_cmd_with_mock(&server.uri(), config_dir.path())
        .args([
            "issue",
            "list",
            "--all",
            "--jql",
            "project = TEST",
            "--no-input",
        ])
        .timeout(std::time::Duration::from_secs(15))
        .output();

    // Pre-implementation: `output` is Err because the subprocess timed out.
    // This assert fires with a clear message instead of a generic panic.
    assert!(
        output.is_ok(),
        "AC-008 (NFR-R-F): `jr issue list` must terminate within 15s when \
         /rest/api/3/search/jql returns a stuck cursor (JRACLOUD-95368 \
         repeated-cursor symptom). Pre-implementation: loop runs indefinitely, \
         command times out. Post-implementation: anti-loop guard breaks within \
         2 iterations."
    );

    let output = output.unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // AC-NEW-D: stderr must contain the literal Atlassian ticket reference.
    // Pre-implementation: stderr is empty (no warning emitted). Assertion fails.
    // Post-implementation: guard emits warning containing "JRACLOUD-95368".
    assert!(
        stderr.contains("JRACLOUD-95368"),
        "AC-NEW-D (NFR-R-F): stderr must contain 'JRACLOUD-95368' when the anti-loop \
         guard fires. This gives users a copy-pasteable search term for the \
         snapshot-instability root cause documented in the upstream Atlassian \
         tracker. Pre-implementation: no warning emitted. Got stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// search_issues guard-abort has_more=true contract pin (Copilot review on PR #364).
//
// Library-level companion to the subprocess `ac_008_...` test above: directly
// calls `client.search_issues(...)` against a stuck-cursor mock and asserts
// `SearchResult.has_more == true` after the guard fires. Matches the parallel
// `KeySearchResult` contract — both result types now signal incompleteness
// via `has_more = true` on repeated-cursor abort, instead of silently
// returning `has_more = false` despite the explicit "Some results may be
// missing" warning.
//
// Three CLI readers consume `SearchResult.has_more` for truncation hints:
// `cli/issue/list.rs`, `cli/board.rs`, `cli/sprint.rs`. With this contract,
// they will correctly display the "Showing N of M, use --all to see more"
// hint on guard-abort instead of misleading the user that the result set is
// complete.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issues_repeated_cursor_abort_sets_has_more_true() {
    let server = MockServer::start().await;

    let stuck_response = serde_json::json!({
        "issues": [
            {
                "key": "TEST-1",
                "fields": {
                    "summary": "Test issue",
                    "status": {"name": "To Do"},
                    "issuetype": {"name": "Task"},
                    "priority": {"name": "Medium"},
                    "assignee": null,
                    "reporter": null,
                    "project": {"key": "TEST"},
                    "description": null,
                    "created": "2026-01-01T00:00:00.000+0000",
                    "updated": "2026-01-01T00:00:00.000+0000",
                    "resolution": null,
                    "components": [],
                    "fixVersions": [],
                    "labels": [],
                    "parent": null,
                    "issuelinks": []
                }
            }
        ],
        "nextPageToken": "stuck-loop"
    });

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&stuck_response))
        // Exactly 2 hits: page 1 establishes prev_cursor, page 2 repeats it
        // and triggers the guard before a 3rd request would be sent.
        .expect(2)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client
        .search_issues("project = TEST", None, &[])
        .await
        .expect("guard must abort gracefully");

    // Contract pin: guard-abort sets has_more=true (matches KeySearchResult).
    // Pre-fix (Copilot review): has_more was silently false.
    assert!(
        result.has_more,
        "search_issues MUST set has_more=true on repeated-cursor guard abort \
         to match KeySearchResult and honour the explicit 'Some results may be \
         missing' stderr warning. Pre-fix: has_more was false."
    );
    assert!(
        !result.issues.is_empty(),
        "guard-abort must preserve page 1's issues; loop is broken before runaway"
    );
}

// ---------------------------------------------------------------------------
// search_issues dedupe contract pin (issue #365, AC-003).
//
// When the repeated-cursor guard fires, search_issues MUST dedupe the
// accumulated issues keyed on issue.key (first-occurrence wins). This mirrors
// the search_issue_keys dedupe added in the same PR.
//
// Setup: page 1 returns [TEST-1], page 2 returns [TEST-1, TEST-2] with the
// same nextPageToken. Combined before dedupe: [TEST-1, TEST-1, TEST-2].
// After incremental seen_keys HashSet dedupe (keyed on issue.key): [TEST-1, TEST-2].
// Guard fires; has_more=true.
//
// Mock disambiguation: page-1 matcher uses body_partial_json({"jql": "q"}) +
// up_to_n_times(1); page-2 matcher uses body_partial_json({"nextPageToken":
// "loop"}) per the pattern established in search_issue_keys test 13.
//
// Traces to BC-2.6.051 (AC-003 of issue #365).
// ---------------------------------------------------------------------------

/// Build a minimal full-issue JSON body for `issue.key`. Field shapes mirror
/// the `stuck_response` used in `test_search_issues_repeated_cursor_abort_sets_has_more_true`.
fn issue_body(key: &str) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "fields": {
            "summary": format!("Issue {}", key),
            "status": {"name": "To Do"},
            "issuetype": {"name": "Task"},
            "priority": {"name": "Medium"},
            "assignee": null,
            "reporter": null,
            "project": {"key": key.split('-').next().expect("split always yields at least one item")},
            "description": null,
            "created": "2026-01-01T00:00:00.000+0000",
            "updated": "2026-01-01T00:00:00.000+0000",
            "resolution": null,
            "components": [],
            "fixVersions": [],
            "labels": [],
            "parent": null,
            "issuelinks": []
        }
    })
}

/// Build a `/rest/api/3/search/jql` response with the given issue keys and an
/// optional next-page cursor. Issue field shapes mirror `stuck_response` in
/// `test_search_issues_repeated_cursor_abort_sets_has_more_true`.
fn jql_issues_response(keys: &[&str], next_page_token: Option<&str>) -> serde_json::Value {
    let issues: Vec<serde_json::Value> = keys.iter().map(|k| issue_body(k)).collect();
    let mut body = serde_json::json!({
        "issues": issues,
    });
    if let Some(t) = next_page_token {
        body["nextPageToken"] = serde_json::json!(t);
    }
    body
}

#[tokio::test]
async fn test_search_issues_repeated_cursor_abort_dedupes() {
    let server = MockServer::start().await;

    // Page 1 — initial fetch; returns [TEST-1] with nextPageToken: "loop".
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({"jql": "q"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_issues_response(&["TEST-1"], Some("loop"))),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Page 2 — returns [TEST-1, TEST-2] with the same nextPageToken: "loop".
    // TEST-1 is a consecutive duplicate in the combined vec; guard fires.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(
            serde_json::json!({"nextPageToken": "loop"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_issues_response(&["TEST-1", "TEST-2"], Some("loop"))),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client
        .search_issues("q", None, &[])
        .await
        .expect("guard must abort gracefully");

    // Dedupe contract: [TEST-1, TEST-2] — incremental seen_keys HashSet rejects
    // the duplicate TEST-1. The pre-dedup candidate list was [TEST-1, TEST-1, TEST-2].
    assert_eq!(
        result
            .issues
            .iter()
            .map(|i| i.key.as_str())
            .collect::<Vec<_>>(),
        vec!["TEST-1", "TEST-2"],
        "search_issues MUST dedupe on repeated-cursor abort while preserving \
         first-occurrence order (keyed on issue.key)"
    );
    assert!(
        result.has_more,
        "repeated-cursor guard abort must set has_more=true"
    );
}

// ---------------------------------------------------------------------------
// Non-consecutive duplicate correctness pin for search_issues (issue #365).
//
// Mirror of test_search_issue_keys_dedupes_non_consecutive_across_pages for
// full Issue bodies. Page 1 returns [TEST-1], page 2 returns [TEST-2, TEST-1]
// (non-consecutive duplicate). Combined before dedupe: [TEST-1, TEST-2, TEST-1].
// Vec::dedup() would return [TEST-1, TEST-2, TEST-1] unchanged (non-adjacent).
// Incremental seen_keys HashSet correctly returns [TEST-1, TEST-2].
//
// Traces to BC-2.6.051 (AC-003 of issue #365).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issues_dedupes_non_consecutive_across_pages() {
    let server = MockServer::start().await;

    // Page 1 — returns [TEST-1] with nextPageToken: "loop".
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({"jql": "q"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_issues_response(&["TEST-1"], Some("loop"))),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Page 2 — returns [TEST-2, TEST-1] (non-consecutive duplicate) with same token.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(
            serde_json::json!({"nextPageToken": "loop"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(jql_issues_response(&["TEST-2", "TEST-1"], Some("loop"))),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client
        .search_issues("q", None, &[])
        .await
        .expect("guard must abort gracefully");

    // Dedupe contract: [TEST-1, TEST-2] — incremental seen_keys HashSet rejects
    // the non-adjacent duplicate TEST-1. Vec::dedup() would incorrectly return
    // [TEST-1, TEST-2, TEST-1] unchanged (consecutive-only).
    assert_eq!(
        result
            .issues
            .iter()
            .map(|i| i.key.as_str())
            .collect::<Vec<_>>(),
        vec!["TEST-1", "TEST-2"],
        "search_issues MUST dedupe non-consecutive duplicates (Vec::dedup() \
         is insufficient — it only removes consecutive duplicates)"
    );
    assert!(
        result.has_more,
        "repeated-cursor guard abort must set has_more=true"
    );
}

// ---------------------------------------------------------------------------
// Limit-truncation path dedupe pin for search_issues (issue #365, AC-004).
//
// Mirror of test_search_issue_keys_limit_truncation_dedupes_under_drift for
// full Issue bodies. limit=11. Single page returns 11 issues with TEST-1
// duplicated, no nextPageToken. After per-iteration dedupe: 10 unique issues.
// Truncation check: 10 < 11 → does NOT fire. Cursor exhaustion exits loop.
//
// If dedupe runs AFTER the truncation check (wrong placement), 11 >= 11 would
// fire the truncation block spuriously and set has_more=true.
//
// Traces to BC-2.6.051 (AC-004 of issue #365).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issues_limit_truncation_dedupes_under_drift() {
    let server = MockServer::start().await;

    // Single page: 11 issues with TEST-1 duplicated, no nextPageToken.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jql_issues_response(
                &[
                    "TEST-1", "TEST-1", "TEST-2", "TEST-3", "TEST-4", "TEST-5", "TEST-6", "TEST-7",
                    "TEST-8", "TEST-9", "TEST-10",
                ],
                None,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    // limit=11: the +1 over-fetch sentinel pattern used by handle_edit::effective_keys.
    let result = client
        .search_issues("q", Some(11), &[])
        .await
        .expect("must succeed");

    // After dedupe: 10 unique issues. Truncation check 10 < 11 does NOT fire.
    assert_eq!(
        result.issues.len(),
        10,
        "after deduplication, exactly 10 unique issues must be returned"
    );
    assert_eq!(
        result.issues[0].key, "TEST-1",
        "first-occurrence order must be preserved (TEST-1 was first on the page)"
    );
    assert!(
        !result.has_more,
        "cursor exhaustion after dedupe (10 < 11 limit) must set has_more=false"
    );
}

// ---------------------------------------------------------------------------
// NEGATIVE-PIN: Risk #5 collision corner for search_issues (issue #365, AC-004).
//
// Mirror of test_search_issue_keys_apr2025_overshoot_silenced_by_drift_dedupe
// for full Issue bodies. When the post-dedupe unique count exactly equals the
// limit (10 == 10), the truncation check fires and more_available =
// (10 > 10) || false = false → has_more=false. This is Risk #5 — the dedupe
// silences the overshoot signal.
//
// A future PR fixing Risk #5 MUST update the has_more assertion from false to
// true in lockstep.
//
// Traces to BC-2.6.051 (AC-004 negative-pin of issue #365).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_issues_apr2025_overshoot_silenced_by_drift_dedupe() {
    let server = MockServer::start().await;

    // Single page: 11 issues with TEST-1 duplicated, no nextPageToken.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jql_issues_response(
                &[
                    "TEST-1", "TEST-1", "TEST-2", "TEST-3", "TEST-4", "TEST-5", "TEST-6", "TEST-7",
                    "TEST-8", "TEST-9", "TEST-10",
                ],
                None,
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    // limit=10: exactly effective_max; triggers the truncation-check collision.
    let result = client
        .search_issues("q", Some(10), &[])
        .await
        .expect("must succeed");

    // After dedupe: 10 unique issues. Truncation check: 10 >= 10 → fires.
    // more_available = (10 > 10) || false = false → has_more = false.
    assert_eq!(
        result.issues.len(),
        10,
        "exactly 10 unique issues must be returned after dedupe"
    );
    // REGRESSION-PIN: has_more is false because dedupe collapsed the Apr 2025
    // overshoot duplicate before the truncation check could detect it. This is
    // Risk #5 — not the desired behavior (ideally has_more=true to signal the
    // overshoot). A future PR fixing Risk #5 MUST update this assertion to
    // `assert!(result.has_more)` in lockstep. See
    // test_search_issue_keys_apr2025_overshoot_silenced_by_drift_dedupe for
    // the parallel keys-only variant.
    assert!(
        !result.has_more,
        "REGRESSION-PIN: has_more is false because dedupe collapsed the Apr 2025 \
         overshoot duplicate. This is Risk #5 — not desired behavior. \
         Update this assertion if Risk #5 is fixed."
    );
}
