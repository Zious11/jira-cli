//! Issue #333: `await_bulk_task` deadline must propagate into `JiraClient::send`
//! so that 429-storm sleeps inside `send` cannot overshoot the caller's deadline.
//!
//! # Problem
//!
//! `await_bulk_task_inner` checks the deadline at the top of each polling loop
//! iteration. Each poll round goes through `self.poll_bulk_task` → `self.get` →
//! `self.send`. `JiraClient::send` retries up to `MAX_RETRIES = 3` on HTTP 429
//! with sleeps capped at `MAX_RETRY_AFTER_SECS = 60`.
//!
//! If a 429-storm hits just before the deadline, a single poll can sleep up to
//! `3 × 60 = 180s` past the deadline before the next deadline check fires. For
//! a 30s deadline, real-world worst case is ~210s elapsed (a ~7× overshoot).
//!
//! # Headline test (AC-001)
//!
//! `test_333_bulk_429_storm_respects_deadline_within_grace`: mounts a wiremock
//! bulk-poll endpoint that returns `HTTP 429` with `Retry-After: 60`
//! indefinitely. Drives `jr issue edit` (subprocess) with
//! `JR_BULK_AWAIT_TIMEOUT_SECS=30` and asserts the command exits non-zero
//! within `WALL_CLOCK_BUDGET_SECS` (~40s — deadline + one in-flight poll RTT +
//! tolerance), not ~210s.
//!
//! # Why 30s, not 5s
//!
//! The clamp must produce a NON-ZERO `actual_sleep` for the first 429 (to
//! exercise the clamp path). With `Retry-After: 60` and a 30s deadline, the
//! first sleep is `min(60, ~30) = ~30s` (slightly less than 30s in practice
//! because the in-flight poll RTT and submit-request RTT have already
//! consumed a few milliseconds from the deadline by the time the clamp
//! computes `remaining`). After that sleep, `remaining < 1ms` and the clamp
//! returns `Expired`. Total wall-clock: ~30s.
//!
//! A shorter deadline (e.g., 5s) would also work but would not exercise the
//! "first sleep is clamped from 60s down to N seconds" code path as clearly.
//!
//! # Subprocess vs in-process
//!
//! This is a subprocess test (via `assert_cmd::Command`) because:
//!   1. The CLI surface — `jr issue edit --jql ... --yes --no-input` — is what
//!      operators run; in-process tests of the API layer alone don't catch
//!      CLI-handler regressions.
//!   2. The `JR_BULK_AWAIT_TIMEOUT_SECS` env-var seam is read by the binary's
//!      CLI plumbing (the resolver in `src/api/jira/bulk.rs`); a subprocess
//!      reads the env naturally without any in-process unsafe `set_var` dance.
//!   3. Per Q5 research-validation (2026-05-12): `tokio::time::pause` is
//!      incompatible with subprocess + wiremock (tokio #4522), so the real-
//!      time wall-clock test is the right tradeoff for this AC.

// ---------------------------------------------------------------------------
// BC ↔ Test index (audit-followup discoverability)
// ---------------------------------------------------------------------------
// BC-3.4.009  →  test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp
//                (S-340 / issue #340: pins task_id literal in [deadline:bulk-outer]
//                stderr message; production site at src/api/jira/bulk.rs:411-415.)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
mod common;

use std::time::{Duration, Instant};

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Wall-clock tolerance for the headline 30s-deadline test. Generous to absorb
/// CI variance and the in-flight poll RTT that exists in addition to the
/// declared deadline.
const WALL_CLOCK_BUDGET_SECS: u64 = 40;

/// Positive lower bound — closes the false-positive risk that the adversary
/// flagged (CONCERN-3 pass-01): if `deadline` were ever computed in the past
/// by a regression, the top-of-loop check fires immediately, the test passes
/// in <1s, and the clamp is NOT exercised. A 25s floor confirms the clamp
/// engaged on AT LEAST the first 429 with a non-trivial sleep (the first
/// `min(60, ~30) = ~30s` sleep), without being so tight that legitimate
/// in-flight RTT shaves the elapsed below the floor.
const WALL_CLOCK_FLOOR_SECS: u64 = 25;

/// Pre-fix runtime worst case (3 retries × 60s sleep = 180s of overshoot
/// inside `send`, plus the 30s deadline = ~210s). If the test ever runs this
/// long, the clamp is not engaging at all.
const PRE_FIX_LOWER_BOUND_SECS: u64 = 120;

/// Build a `jr` command pointing at the mock server, with the timeout test
/// seam set to 30 seconds.
fn jr_cmd_with_30s_deadline(server_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_url)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0") // test:test base64
        .env("JR_BULK_AWAIT_TIMEOUT_SECS", "30");
    cmd
}

/// Bulk task ENQUEUED response — submit endpoint returns this so the polling
/// loop starts.
fn bulk_enqueued(task_id: &str) -> serde_json::Value {
    serde_json::json!({
        "taskId": task_id,
        "status": "ENQUEUED",
        "progressPercent": 0,
        "totalIssueCount": 1,
        "processedAccessibleIssues": [],
        "failedAccessibleIssues": {},
        "invalidOrInaccessibleIssueCount": 0
    })
}

/// Two-issue JQL search response — minimum required for `jr issue edit --jql
/// ... --label` to route through to the bulk POST (BUG-LABEL-400 fix: single-key
/// label edits now use PUT /rest/api/3/issue/{key} directly; 2+ keys still use
/// the bulk path, which is what these deadline tests exercise).
fn jql_search_response_two(key1: &str, key2: &str) -> serde_json::Value {
    fn issue_json(key: &str) -> serde_json::Value {
        serde_json::json!({
            "key": key,
            "fields": {
                "summary": format!("Issue {}", key),
                "status": {"name": "To Do", "statusCategory": {"key": "new", "name": "To Do"}},
                "issuetype": {"name": "Task"},
                "priority": {"name": "Medium"},
                "assignee": null,
                "reporter": null,
                "project": {"key": key.split('-').next().unwrap_or("TEST")},
                "description": null,
                "created": "2024-01-01T00:00:00.000+0000",
                "updated": "2024-01-01T00:00:00.000+0000",
                "resolution": null,
                "components": [],
                "fixVersions": [],
                "labels": [],
                "parent": null,
                "issuelinks": []
            }
        })
    }
    serde_json::json!({
        "issues": [issue_json(key1), issue_json(key2)],
        "nextPageToken": null
    })
}

// ---------------------------------------------------------------------------
// AC-001: 30s deadline + 429-storm on poll → exit within ~40s wall-clock.
// ---------------------------------------------------------------------------

/// Headline AC-001 test. Asserts BC-bulk.poll.deadline-bounded:
/// `await_bulk_task(timeout=30s)` returns within `30s + ε` even when every
/// poll returns 429 with `Retry-After: 60`.
///
/// Pre-fix (before this issue is implemented): the test would run ~210s
/// because the first poll's `send` would sleep 60s × 3 retries = 180s before
/// the next deadline check fires. The wall-clock budget of 40s would be
/// exceeded by >5×.
///
/// Post-fix: `send_inner` clamps the 429 sleep to `min(60, 30 - 0) = 30s`
/// on the first 429; after that sleep, `remaining = 0` (modulo control
/// overhead) and the clamp returns `Err` with a "deadline" message. Total:
/// ~30s wall-clock.
///
/// We assert `< WALL_CLOCK_BUDGET_SECS (40s)` rather than a tight `> 28s`
/// lower bound because:
///   * Faster-than-deadline behavior is acceptable if the clamp engages
///     correctly (e.g., the first sleep might be slightly shorter than 30s
///     due to in-flight poll RTT).
///   * CI variance can stretch the upper bound; the failure mode we want to
///     catch is the 180s+ overshoot, which is far above 40s.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_333_bulk_429_storm_respects_deadline_within_grace() {
    let server = MockServer::start().await;

    // Search returns 2 matched issues (2+ required for label edits to use bulk path;
    // BUG-LABEL-400 fix routes single-key label edits to PUT instead of bulk).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jql_search_response_two("PROJ-1", "PROJ-2")),
        )
        .mount(&server)
        .await;

    // Bulk POST: returns ENQUEUED so the polling loop starts.
    let task_id = "task-333-deadline-test";
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued(task_id)))
        .mount(&server)
        .await;

    // EVERY poll returns HTTP 429 with `Retry-After: 60` — the 429-storm.
    // No expect(_) — we don't know exactly how many polls will fire before
    // the clamp returns Err; what matters is the wall-clock budget.
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "60")
                .set_body_json(serde_json::json!({
                    "errorMessages": ["Rate limited (test wiremock)"]
                })),
        )
        .mount(&server)
        .await;

    let start = Instant::now();

    let output = jr_cmd_with_30s_deadline(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = PROJ",
            "--label",
            "add:foo",
            "--yes",
        ])
        .timeout(Duration::from_secs(WALL_CLOCK_BUDGET_SECS + 30)) // hard kill if clamp fails
        .output()
        .expect("subprocess spawn failed");

    let elapsed = start.elapsed();

    // Primary assertion: wall-clock budget. This is the headline AC-001
    // assertion — any value above 40s means the clamp is not engaging at the
    // 30s deadline, and any value above 120s confirms the pre-fix
    // 3-retries × 60s overshoot is still happening.
    assert!(
        elapsed.as_secs() < WALL_CLOCK_BUDGET_SECS,
        "AC-001 VIOLATION: bulk poll under 30s deadline elapsed {}s, expected < {}s. \
         If elapsed ≥ {}s, the 429-storm clamp is not engaging at all. \
         stderr:\n{}\nstdout:\n{}",
        elapsed.as_secs(),
        WALL_CLOCK_BUDGET_SECS,
        PRE_FIX_LOWER_BOUND_SECS,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    // Lower-bound assertion (CONCERN-3 pass-01): elapsed must be NEAR the
    // 30s deadline, not <1s. If the clamp short-circuits to Err on entry
    // (e.g., due to a deadline-computed-in-the-past regression), the test
    // would otherwise pass in <1s with the clamp un-exercised — false-
    // positive. Floor confirms the clamp engaged on at least the first 429.
    assert!(
        elapsed.as_secs() >= WALL_CLOCK_FLOOR_SECS,
        "AC-001 VIOLATION: bulk poll exited too quickly ({}s, expected ≥ {}s). \
         The clamp should engage on the FIRST 429 with a min(60, ~30) = ~30s \
         sleep before returning Err. Sub-{}s elapsed indicates the deadline \
         short-circuited to Err on function entry instead of after a real \
         clamped sleep — the headline scenario was NOT exercised. \
         stderr:\n{}\nstdout:\n{}",
        elapsed.as_secs(),
        WALL_CLOCK_FLOOR_SECS,
        WALL_CLOCK_FLOOR_SECS,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    // Secondary assertion: command must exit with code 124 (POSIX timeout
    // convention; JrError::DeadlineExceeded). C-3 (F5 pass-03): WITHOUT this
    // exit-code pin a regression from JrError::DeadlineExceeded back to
    // JrError::ApiError(429) (exit 1) would still pass the loose
    // `!output.status.success()` check.
    assert_eq!(
        output.status.code(),
        Some(124),
        "AC-001 VIOLATION: expected exit code 124 (JrError::DeadlineExceeded, \
         POSIX timeout convention). Got code={:?}. stderr:\n{}\nstdout:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    // Tertiary assertion: stderr contains "deadline" (the DeadlineExceeded
    // variant Display produces "Deadline exceeded:" + message with site tag).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("deadline"),
        "AC-001 VIOLATION: expected stderr to contain 'deadline'. \
         Got stderr:\n{stderr}",
    );
}

// ---------------------------------------------------------------------------
// B-1 (F5 pass-02): RUNNING-storm respects deadline via outer-loop clamp.
// ---------------------------------------------------------------------------

/// Companion to AC-001. The 429-storm test exercises the INNER clamp inside
/// `JiraClient::send_inner`. THIS test exercises the OUTER clamp inside
/// `await_bulk_task_inner`'s post-poll exponential-backoff sleep
/// (`src/api/jira/bulk.rs:495-498`).
///
/// Scenario: every poll returns HTTP 200 with `status: "RUNNING"` (a known
/// non-terminal status). No 429s fire, so the inner clamp never engages.
/// Without the outer clamp (pre-B-1), the exponential backoff would sleep
/// up to POLL_MAX_SECS=10s past the 30s deadline before the next top-of-loop
/// check fires. Post-B-1: the backoff sleep is clamped to remaining budget.
///
/// Without the outer clamp the test would elapse 35-40s (10s overshoot per
/// adversarial schedule).  With the clamp it elapses ~31s (deadline + final
/// short clamp + in-flight RTT).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_333_b1_bulk_running_storm_respects_deadline_via_outer_clamp() {
    let server = MockServer::start().await;

    // Search returns 2 matched issues (2+ required for label edits to use bulk path;
    // BUG-LABEL-400 fix routes single-key label edits to PUT instead of bulk).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(jql_search_response_two("PROJ-1", "PROJ-2")),
        )
        .mount(&server)
        .await;

    let task_id = "task-333-b1-running-storm";

    // Bulk POST: returns ENQUEUED so the polling loop starts.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bulk_enqueued(task_id)))
        .mount(&server)
        .await;

    // EVERY poll returns 200 OK with RUNNING — the outer-loop storm.
    // No 429s, so the inner-send clamp never engages.
    let running_response = serde_json::json!({
        "taskId": task_id,
        "status": "RUNNING",
        "progressPercent": 50,
        "totalIssueCount": 1,
        "processedAccessibleIssues": [],
        "failedAccessibleIssues": {},
        "invalidOrInaccessibleIssueCount": 0
    });
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(running_response))
        .mount(&server)
        .await;

    let start = Instant::now();

    let output = jr_cmd_with_30s_deadline(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = PROJ",
            "--label",
            "add:foo",
            "--yes",
        ])
        .timeout(Duration::from_secs(WALL_CLOCK_BUDGET_SECS + 30))
        .output()
        .expect("subprocess spawn failed");

    let elapsed = start.elapsed();

    assert!(
        elapsed.as_secs() < WALL_CLOCK_BUDGET_SECS,
        "B-1 VIOLATION: RUNNING-storm under 30s deadline elapsed {}s, expected < {}s. \
         The outer-loop exponential-backoff clamp at bulk.rs:495 is not engaging — \
         the test elapsed >= {}s indicates POLL_MAX_SECS overshoot is occurring. \
         stderr:\n{}",
        elapsed.as_secs(),
        WALL_CLOCK_BUDGET_SECS,
        WALL_CLOCK_BUDGET_SECS,
        String::from_utf8_lossy(&output.stderr),
    );

    assert!(
        elapsed.as_secs() >= 25,
        "B-1 false-positive guard: elapsed {}s is < 25s; check that the test \
         actually engaged the outer-loop clamp (not the entry-point check). \
         stderr:\n{}",
        elapsed.as_secs(),
        String::from_utf8_lossy(&output.stderr),
    );

    // C-3 (F5 pass-03) — the outer-loop deadline-exceeded site now also
    // returns JrError::DeadlineExceeded (exit 124), matching the inner sites.
    // A regression that reverted bulk.rs:397 back to `anyhow::anyhow!` would
    // exit 1 and break the same-cause-same-exit-code contract.
    assert_eq!(
        output.status.code(),
        Some(124),
        "B-1 VIOLATION: expected exit code 124 (JrError::DeadlineExceeded) \
         from the outer-loop deadline-exceeded site at bulk.rs:397. \
         Got code={:?}. stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("deadline"),
        "B-1 VIOLATION: expected stderr to contain 'deadline'. \
         Got stderr:\n{stderr}",
    );

    // Quaternary assertion (AC #1 of issue #340 / BC-3.4.009): timeout error
    // MUST include the task_id literal in stderr so the user can recover via
    // `jr api /rest/api/3/bulk/queue/<task_id>` without hunting through the
    // Jira UI.
    // Pinning the existing `"[deadline:bulk-outer] Bulk task {task_id} ..."` format
    // at src/api/jira/bulk.rs:411-415 (the message format inside the JrError::DeadlineExceeded Err-return block at lines 408-418).

    // (Loose form — satisfies story S-340 AC #1 literal "stderr.contains(task_id)"
    // and BC-3.4.009 minimum postcondition: the task_id literal MUST appear in stderr.)
    assert!(
        stderr.contains(task_id),
        "BC-3.4.009 VIOLATION (loose): expected stderr to contain task_id literal \
         {task_id:?}. Got stderr:\n{stderr}",
    );

    // (Strict form — guards against substring false-positives, e.g., a future
    // --verbose test printing the GET URL `/rest/api/3/bulk/queue/<task_id>`
    // that would mask a regression in the deadline-message itself.)
    let expected_fragment = format!("[deadline:bulk-outer] Bulk task {task_id} did not");
    assert!(
        stderr
            .to_lowercase()
            .contains(&expected_fragment.to_lowercase()),
        "BC-3.4.009 VIOLATION (strict): expected stderr to contain the deadline-message \
         fragment {expected_fragment:?} (proves task_id is interpolated inside the \
         [deadline:bulk-outer] message, not coincidentally elsewhere). Got stderr:\n{stderr}",
    );
}
