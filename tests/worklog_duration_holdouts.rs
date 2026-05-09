//! S-2.06 Red Gate — worklog timeSpent passthrough + CMDB cache tuple format pin
//!
//! ## AC-004 Red Gate strategy: compile-error
//!
//! AC-004 tests `jr::duration::parse_duration_validate`, a function that does
//! NOT exist in the current codebase. The test therefore FAILS TO COMPILE
//! until the implementer adds `parse_duration_validate` to `src/duration.rs`.
//!
//! This is intentional: a compile-error Red Gate is the clearest possible
//! signal that the implementation is missing. The implementer's task is to
//! add `parse_duration_validate(input: &str) -> Result<()>` (no hours_per_day /
//! days_per_week parameters) and make AC-004 compile and pass.
//!
//! The compile-failing test is annotated with:
//!   // RED-GATE-COMPILE: replaced by implementer
//! so `rg RED-GATE-COMPILE` finds it.
//!
//! ## Expected Red Gate state per AC
//!
//! | Test | Expected state before implementation |
//! |------|--------------------------------------|
//! | AC-001 | FAIL — assertion error: body contains `timeSpentSeconds`, not `timeSpent` |
//! | AC-002 | FAIL — assertion error: same root cause as AC-001 |
//! | AC-003 | FAIL — assertion error: exit code is not 64 (current code parses "1z" |
//! |        |   as an invalid-unit error but the error message says "Use w, d, h, or m", |
//! |        |   not the new "Nw Nd Nh Nm" hint expected in the test) |
//! | AC-004 | FAIL — compile error: `parse_duration_validate` not found |
//! | AC-005 | PASS — graceful degradation already exists (cache miss on bad format) |
//! | AC-006 | PASS — valid tuple cache already hits without API call |
//!
//! ## Worklog CLI invocation
//!
//! `WorklogCommand::Add` uses a POSITIONAL `duration` argument (not `--time`).
//! CLI: `jr worklog add PROJ-1 "1d" [--message TEXT]`
//! Source: `src/cli/mod.rs::WorklogCommand::Add { key, duration, message }`
//!
//! ## POST body discovery
//!
//! Current production code: `src/api/jira/worklogs.rs::add_worklog`
//!   - Param: `time_spent_seconds: u64`
//!   - POST body: `{"timeSpentSeconds": <u64>}`
//!
//! After implementation:
//!   - Param: `time_spent: &str`
//!   - POST body: `{"timeSpent": "<string>"}`
//!
//! ## CMDB fields endpoint
//!
//! `GET /rest/api/3/field` — returns a JSON array of field objects.
//! CMDB fields are those with `schema.custom == "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"`.
//! Source: `src/api/jira/fields.rs::filter_cmdb_fields`
//!
//! ## Cache struct shape (current)
//!
//! `CmdbFieldsCache { fields: Vec<(String, String)>, fetched_at: DateTime<Utc> }`
//! Stored in: `<XDG_CACHE_HOME>/jr/v1/<profile>/cmdb_fields.json`
//! Legacy (ID-only) format: `["customfield_10191"]` — a bare JSON array of strings.
//! Graceful degradation: `read_cmdb_fields_cache` returns `Ok(None)` on deserialization
//! failure (warns to stderr then continues), confirmed in `src/cache.rs::read_cache`.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use std::sync::Mutex;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a `jr` command with full XDG isolation and mock server wiring.
fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("XDG_CONFIG_HOME", config_dir)
        .arg("--no-input");
    cmd
}

/// Worklog success response (minimal valid shape for the CLI to accept).
fn worklog_success_response() -> Value {
    serde_json::json!({
        "id": "10001",
        "timeSpent": "1d",
        "timeSpentSeconds": 28800,
        "author": {
            "accountId": "acc-test-001",
            "displayName": "Test User"
        },
        "started": "2026-05-07T09:00:00.000+0000"
    })
}

// ---------------------------------------------------------------------------
// Mutex for tests that manipulate XDG_CACHE_HOME
// ---------------------------------------------------------------------------

// Unit tests for AC-005 / AC-006 call `jr::cache` functions that read
// XDG_CACHE_HOME from the environment. We set XDG_CACHE_HOME via env on the
// spawned `jr` process in process-spawn tests, which is safe. For the
// library-level cache tests we follow the same pattern as `src/cache.rs`:
// serialize access with a mutex so parallel test threads do not race.
//
// Note: process-spawn tests (AC-001..AC-003) set XDG_CACHE_HOME only on the
// child process environment, never on the parent process, so they do not need
// the mutex.
static CACHE_ENV_MUTEX: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// AC-001 / BC-X.5.009 — POST body must contain `timeSpent` string, not `timeSpentSeconds`
// ---------------------------------------------------------------------------

/// BC-X.5.009 postcondition: `jr worklog add PROJ-1 "1d"` must POST a body
/// that contains `"timeSpent": "1d"` (string field) and must NOT contain any
/// `timeSpentSeconds` key.
///
/// Red Gate (FAIL before implementation): current production code sends
/// `{"timeSpentSeconds": 28800}`. The body_string_contains matcher on
/// `"timespent"` will pass (that substring appears in `timespentseconds`),
/// but the absence assertion `assert!(!body.contains("timespentseconds"))` will
/// FAIL because the current code does include `timeSpentSeconds`.
///
/// Pins: `src/api/jira/worklogs.rs::add_worklog` POST body field name.
///
/// Design note: We capture the raw request body via `wiremock::Spy` by using a
/// custom body-string matcher. The assertion checks both the presence of the
/// correct field and the absence of the old field.
#[tokio::test]
async fn test_s_2_06_ac_001_bc_x_5_009_worklog_post_body_contains_timespent_string() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Mount a mock that responds 201 to any POST to the worklog endpoint.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(worklog_success_response()))
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["worklog", "add", "PROJ-1", "1d"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The command must succeed first (both old and new code can exit 0 if the
    // mock responds 201; this just confirms we got through to the server).
    assert!(
        output.status.success(),
        "worklog add must exit 0 on a 201 response; stderr: {stderr}, stdout: {stdout}"
    );

    // Inspect the received requests to verify the POST body shape.
    let received = server.received_requests().await.unwrap();
    let post_req = received
        .iter()
        .find(|r| r.method == wiremock::http::Method::POST)
        .expect("POST to worklog endpoint must have been made");

    let body_str = String::from_utf8_lossy(&post_req.body).to_lowercase();

    // AC-001 assertion 1: body MUST contain `"timespent"` as a key
    // (the lowercase form of `"timeSpent"`).
    // The new implementation sends `{"timeSpent": "1d", ...}`.
    // The old implementation sends `{"timeSpentSeconds": 28800}`.
    // We check for `"timespent":` (with colon) to distinguish the field name
    // from an embedded string value that happens to contain "timespent".
    assert!(
        body_str.contains("\"timespent\":"),
        "AC-001 FAIL: POST body must contain `\"timeSpent\":` (string field), \
         but body was: {}",
        body_str
    );

    // AC-001 assertion 2: body MUST NOT contain `timeSpentSeconds`.
    // RED-GATE: current code sends timeSpentSeconds — this assertion FAILS.
    assert!(
        !body_str.contains("timespentseconds"),
        "AC-001 FAIL: POST body must NOT contain `timeSpentSeconds` \
         (old integer field), but body was: {}",
        body_str
    );

    // AC-001 assertion 3: the value of `timeSpent` must be the string "1d".
    assert!(
        body_str.contains("\"timespent\": \"1d\"") || body_str.contains("\"timespent\":\"1d\""),
        "AC-001 FAIL: POST body's `timeSpent` value must be the string \"1d\", \
         but body was: {}",
        body_str
    );
}

// ---------------------------------------------------------------------------
// AC-002 / BC-X.5.009 — complex duration string preserved verbatim in POST body
// ---------------------------------------------------------------------------

/// BC-X.5.009 postcondition: `jr worklog add PROJ-1 "2d 3h 30m"` must POST a
/// body where `timeSpent` is exactly the string `"2d 3h 30m"` — spaces and
/// mixed units preserved verbatim from the user's input.
///
/// Red Gate (FAIL before implementation): current code computes
/// `parse_duration("2d 3h 30m", 8, 5)` which produces an error (space in
/// input is not handled) or a seconds value, then sends `timeSpentSeconds`.
/// Either way, `timeSpentSeconds` is absent from the new expected body.
///
/// Pins: `src/api/jira/worklogs.rs::add_worklog` verbatim string passthrough.
#[tokio::test]
async fn test_s_2_06_ac_002_bc_x_5_009_worklog_post_preserves_complex_string() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10002",
            "timeSpent": "2d 3h 30m",
            "timeSpentSeconds": 0,
            "author": {"accountId": "acc-test-001", "displayName": "Test User"},
            "started": "2026-05-07T09:00:00.000+0000"
        })))
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["worklog", "add", "PROJ-1", "2d 3h 30m"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "worklog add must exit 0 on a 201 response; stderr: {stderr}, stdout: {stdout}"
    );

    let received = server.received_requests().await.unwrap();
    let post_req = received
        .iter()
        .find(|r| r.method == wiremock::http::Method::POST)
        .expect("POST to worklog endpoint must have been made");

    let body_str = String::from_utf8_lossy(&post_req.body).to_lowercase();

    // AC-002 assertion 1: timeSpentSeconds must NOT be present.
    // RED-GATE: current code sends timeSpentSeconds — this assertion FAILS.
    assert!(
        !body_str.contains("timespentseconds"),
        "AC-002 FAIL: POST body must NOT contain `timeSpentSeconds`; body was: {}",
        body_str
    );

    // AC-002 assertion 2: the verbatim string must appear in the body.
    // "2d 3h 30m" lowercased is "2d 3h 30m" (no change).
    assert!(
        body_str.contains("\"timespent\":")
            && (body_str.contains("\"2d 3h 30m\"") || body_str.contains("\"2d 3h 30m\"")),
        "AC-002 FAIL: POST body's `timeSpent` must be the verbatim string \
         \"2d 3h 30m\" with spaces preserved; body was: {}",
        body_str
    );
}

// ---------------------------------------------------------------------------
// AC-003 / BC-X.5.009 — invalid duration rejected before any network call
// ---------------------------------------------------------------------------

/// BC-X.5.009 precondition: when the user passes an invalid duration string
/// (`"1z"` — unknown unit `z`), the command must:
///   (a) exit with code 64 (UserError)
///   (b) emit a stderr message that mentions valid syntax (`Nw Nd Nh Nm`)
///   (c) make ZERO POST requests to the worklog endpoint
///
/// Red Gate (FAIL before implementation): the current code DOES reject "1z" at
/// parse_duration time and DOES exit non-zero, BUT:
///   - The exit code may not be 64 (it depends on how JrError maps the error)
///   - The error message says `"Unknown duration unit 'z'"` and does not
///     mention the new "Nw Nd Nh Nm" hint format
/// The stderr format assertion will fail until the new validator is implemented.
///
/// Pins: `src/duration.rs::parse_duration_validate` error message format.
///       `src/cli/worklog.rs::handle_add` early-exit before network.
#[tokio::test]
async fn test_s_2_06_ac_003_bc_x_5_009_invalid_duration_rejected_before_network() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Mount with expect(0): wiremock panics on drop if this is ever called.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(worklog_success_response()))
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(server.uri().as_str(), cache_dir.path(), config_dir.path())
        .args(["worklog", "add", "PROJ-1", "1z"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // AC-003 assertion (a): must exit with code 64 (UserError).
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-003 FAIL: invalid duration must exit 64 (UserError); \
         got exit code {:?}; stderr: {stderr}",
        output.status.code()
    );

    // AC-003 assertion (b): stderr must mention the valid syntax hint.
    // The new validator's error message must include "Nw Nd Nh Nm" (or similar
    // canonical hint listing all four unit types w/d/h/m).
    // RED-GATE: current error says "Unknown duration unit 'z'" without this hint.
    assert!(
        stderr.contains("Nw")
            || stderr.contains("Nd")
            || stderr.contains("Nh")
            || stderr.contains("Nm")
            || stderr.to_lowercase().contains("nw nd nh nm"),
        "AC-003 FAIL: stderr must contain the syntax hint 'Nw Nd Nh Nm' \
         (or at minimum contain the unit tokens Nw/Nd/Nh/Nm); stderr was: {stderr}"
    );

    // AC-003 assertion (c): zero POSTs must be made (wiremock expect(0) verifies
    // this on server drop — but we also assert explicitly for a clear failure message).
    let received = server.received_requests().await.unwrap();
    let post_count = received
        .iter()
        .filter(|r| r.method == wiremock::http::Method::POST)
        .count();
    assert_eq!(
        post_count, 0,
        "AC-003 FAIL: validation must short-circuit before any POST; \
         got {post_count} POST(s)",
    );
    // wiremock also asserts expect(0) on server drop
}

// ---------------------------------------------------------------------------
// AC-004 / BC-X.5.009 — parse_duration_validate unit contract
// ---------------------------------------------------------------------------

/// BC-X.5.009 validator contract: `parse_duration_validate` is a pure syntactic
/// validator. It must:
///   - Accept any valid Jira duration string (w/d/h/m units, positive integers)
///   - Reject empty strings and strings with invalid units
///   - Have NO `hours_per_day` or `days_per_week` parameters
///   - Return `Ok(())` (or `Ok(something)`) on valid input
///   - Return `Err(_)` on invalid input
///
/// RED-GATE-COMPILE: replaced by implementer
///
/// This test is gated with `#[cfg(any())]` so it is EXCLUDED FROM COMPILATION
/// until the implementer adds `parse_duration_validate`. The `#[cfg(any())]`
/// attribute makes the condition always-false, effectively dead-code-gating
/// the test without removing it from the source.
///
/// **Implementer instructions:**
///   1. Add `pub fn parse_duration_validate(input: &str) -> anyhow::Result<()>`
///      to `src/duration.rs`.
///   2. Remove `hours_per_day` and `days_per_week` parameters entirely.
///   3. Keep only unit validation (w, d, h, m); remove seconds arithmetic.
///   4. Remove the `#[cfg(any())]` line below (or replace with `#[test]`).
///   5. Run `cargo test --test worklog_duration_holdouts` — AC-004 must now PASS.
#[test]
fn test_s_2_06_ac_004_bc_x_5_009_parse_duration_validator_unit() {
    // RED-GATE-COMPILE: `parse_duration_validate` does not exist yet.
    // The compiler error "cannot find function `parse_duration_validate`" is
    // the intended Red Gate signal. The implementer must:
    //   1. Add `pub fn parse_duration_validate(input: &str) -> anyhow::Result<()>`
    //      to `src/duration.rs`.
    //   2. Remove `hours_per_day` and `days_per_week` parameters entirely.
    //   3. Keep only unit validation (w, d, h, m); remove seconds arithmetic.
    //   4. Ensure `jr::duration::parse_duration_validate` is accessible here.

    // Valid single-unit strings — all must succeed.
    assert!(
        jr::duration::parse_duration_validate("1d").is_ok(),
        "AC-004: '1d' is valid — must return Ok"
    );
    assert!(
        jr::duration::parse_duration_validate("2h").is_ok(),
        "AC-004: '2h' is valid — must return Ok"
    );
    assert!(
        jr::duration::parse_duration_validate("30m").is_ok(),
        "AC-004: '30m' is valid — must return Ok"
    );
    assert!(
        jr::duration::parse_duration_validate("1w").is_ok(),
        "AC-004: '1w' is valid — must return Ok"
    );

    // Valid multi-unit strings.
    assert!(
        jr::duration::parse_duration_validate("1w 2d 3h 4m").is_ok(),
        "AC-004: '1w 2d 3h 4m' is valid (spaces between units) — must return Ok"
    );
    assert!(
        jr::duration::parse_duration_validate("1w2d3h30m").is_ok(),
        "AC-004: '1w2d3h30m' is valid (compact form) — must return Ok"
    );
    assert!(
        jr::duration::parse_duration_validate("2d 3h 30m").is_ok(),
        "AC-004: '2d 3h 30m' is valid — must return Ok"
    );

    // Invalid: unknown unit.
    assert!(
        jr::duration::parse_duration_validate("1z").is_err(),
        "AC-004: '1z' has unknown unit 'z' — must return Err"
    );

    // Invalid: empty string.
    assert!(
        jr::duration::parse_duration_validate("").is_err(),
        "AC-004: empty string — must return Err"
    );

    // Invalid: number without unit.
    assert!(
        jr::duration::parse_duration_validate("30").is_err(),
        "AC-004: '30' has no unit — must return Err"
    );

    // Invalid: whitespace only.
    assert!(
        jr::duration::parse_duration_validate("   ").is_err(),
        "AC-004: whitespace-only string — must return Err"
    );
}

// ---------------------------------------------------------------------------
// AC-005 / BC-6.2.013 — legacy ID-only CMDB cache format → graceful miss
// ---------------------------------------------------------------------------

/// BC-6.2.013 postcondition: when `cmdb_fields.json` contains the LEGACY
/// ID-only format `["customfield_10191"]` (a bare JSON array of strings rather
/// than the current `CmdbFieldsCache` struct), `read_cmdb_fields_cache` must:
///   (a) return `Ok(None)` — i.e., treat it as a cache miss
///   (b) NOT panic
///
/// This is a library-level test against `jr::cache::read_cmdb_fields_cache`.
/// It does NOT spawn a process. The legacy format cannot be deserialized into
/// `CmdbFieldsCache { fields: Vec<(String, String)>, fetched_at: DateTime<Utc> }`
/// because a bare JSON array is not an object with those fields.
///
/// Expected: PASS on current develop (the graceful degradation was already
/// implemented in `src/cache.rs::read_cache`). This test pins the invariant
/// so a future "simplification" that replaces the `match serde_json::from_str`
/// with an unwrap or `?` would break this test.
///
/// Pins: `src/cache.rs::read_cache` graceful deserialization failure handling.
#[test]
fn test_s_2_06_ac_005_bc_6_2_013_legacy_id_only_cmdb_cache_graceful_miss() {
    let guard = CACHE_ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp_dir = tempfile::tempdir().unwrap();

    // SAFETY: CACHE_ENV_MUTEX serializes all tests that touch XDG_CACHE_HOME
    // in the parent process. No concurrent env access occurs within this lock.
    unsafe { std::env::set_var("XDG_CACHE_HOME", temp_dir.path()) };

    let result = std::panic::catch_unwind(|| {
        // Write the legacy ID-only format (a bare JSON array of strings).
        // This does NOT match the CmdbFieldsCache struct shape.
        let cache_dir = jr::cache::cache_dir("default");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let legacy_content = r#"["customfield_10191"]"#;
        std::fs::write(cache_dir.join("cmdb_fields.json"), legacy_content).unwrap();

        // AC-005 assertion (a): must return Ok(None) — graceful cache miss.
        let result = jr::cache::read_cmdb_fields_cache("default")
            .expect("read_cmdb_fields_cache must not return Err on format mismatch");
        assert!(
            result.is_none(),
            "AC-005 FAIL: legacy ID-only format must produce a cache miss (None), \
             got Some(…) — this means the old format is being silently mis-parsed. \
             Cache miss expected so the caller re-fetches and overwrites with new format."
        );
        // AC-005 assertion (b): no panic — if we reach here, catch_unwind has nothing to catch.
    });

    unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    drop(guard);

    result.expect("AC-005 FAIL: read_cmdb_fields_cache panicked on legacy ID-only format");
}

// ---------------------------------------------------------------------------
// AC-006 / BC-6.2.013 — valid tuple-format CMDB cache hits without API call
// ---------------------------------------------------------------------------

/// BC-6.2.013 invariant: when `cmdb_fields.json` contains a valid
/// `CmdbFieldsCache` with the current `Vec<(String, String)>` tuple format,
/// `read_cmdb_fields_cache` must return `Ok(Some(cache))` with the correct data.
///
/// This is a library-level test. It writes a valid tuple-format cache entry
/// directly, then calls `read_cmdb_fields_cache` and asserts the data is
/// returned correctly.
///
/// The "no API call" part (expect(0) on the fields endpoint) is verified
/// implicitly: this test does not start a mock server at all, so any attempt
/// to call a real network endpoint would fail with a connection error.
///
/// Expected: PASS on current develop (the valid format was already implemented).
/// This test pins the forward-compatible happy path so the tuple format is not
/// accidentally broken by future refactors.
///
/// Pins: `src/cache.rs::CmdbFieldsCache` serde round-trip for tuple format.
///       `src/cache.rs::write_cmdb_fields_cache` + `read_cmdb_fields_cache`.
#[test]
fn test_s_2_06_ac_006_bc_6_2_013_valid_tuple_cache_hits_no_api_call() {
    let guard = CACHE_ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp_dir = tempfile::tempdir().unwrap();

    unsafe { std::env::set_var("XDG_CACHE_HOME", temp_dir.path()) };

    let result = std::panic::catch_unwind(|| {
        // Write a valid tuple-format cache entry via the production writer.
        jr::cache::write_cmdb_fields_cache(
            "default",
            &[
                ("customfield_10191".to_string(), "Client".to_string()),
                (
                    "customfield_10245".to_string(),
                    "Hardware Asset".to_string(),
                ),
            ],
        )
        .expect("write_cmdb_fields_cache must succeed");

        // AC-006 assertion: must return Ok(Some(cache)) with correct data.
        let cache = jr::cache::read_cmdb_fields_cache("default")
            .expect("read_cmdb_fields_cache must not error")
            .expect("valid tuple-format cache must return Some(cache)");

        assert_eq!(
            cache.fields.len(),
            2,
            "AC-006 FAIL: expected 2 CMDB fields; got {}",
            cache.fields.len()
        );
        assert_eq!(
            cache.fields[0],
            ("customfield_10191".to_string(), "Client".to_string()),
            "AC-006 FAIL: first field tuple mismatch"
        );
        assert_eq!(
            cache.fields[1],
            (
                "customfield_10245".to_string(),
                "Hardware Asset".to_string()
            ),
            "AC-006 FAIL: second field tuple mismatch"
        );

        // Verify the on-disk format can be re-parsed as Vec<(String, String)>.
        // This exercises the serde round-trip directly to catch format drift.
        let cache_dir = jr::cache::cache_dir("default");
        let raw = std::fs::read_to_string(cache_dir.join("cmdb_fields.json")).unwrap();
        let parsed: jr::cache::CmdbFieldsCache =
            serde_json::from_str(&raw).expect("on-disk file must deserialize as CmdbFieldsCache");
        assert!(
            !parsed.fields.is_empty(),
            "AC-006 FAIL: deserialized fields must be non-empty"
        );
        assert!(
            parsed.fields[0].0.starts_with("customfield_"),
            "AC-006 FAIL: first field ID must start with 'customfield_'"
        );
    });

    unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    drop(guard);

    result.expect("AC-006: no panic expected on valid tuple-format cache");
}
