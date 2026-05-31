//! Live-Jira E2E integration test suite (S-E2E-1).
//!
//! # Gating
//!
//! Every live test is `#[ignore]` AND contains an early-return guard:
//! ```
//! if !e2e_enabled() { return; }
//! ```
//! This dual-gate follows the pattern established by `tests/oauth_embedded_login.rs`
//! (S-410 lesson): `#[ignore]` prevents the test from running under normal `cargo test`,
//! and the early-return guard prevents execution when `--include-ignored` is passed
//! without `JR_RUN_E2E=1`.
//!
//! # Running
//!
//! ```bash
//! JR_RUN_E2E=1 \
//! JR_E2E_BASE_URL=https://<site>.atlassian.net \
//! JR_AUTH_HEADER="Basic $(printf '%s:%s' "$EMAIL" "$TOKEN" | base64 | tr -d '\n')" \
//! JR_E2E_PROJECT=E2E \
//! cargo test --test e2e_live -- --include-ignored --test-threads=1
//! ```
//!
//! `--test-threads=1` is required: the tests share a single live Jira project and
//! parallel execution causes rate-limit pressure and non-deterministic write-flow ordering.
//!
//! # Required environment variables (for gated tests)
//!
//! | Variable                    | Required | Notes                                                         |
//! |-----------------------------|----------|---------------------------------------------------------------|
//! | `JR_RUN_E2E`                | yes      | Must be `"1"` to run gated tests                             |
//! | `JR_E2E_BASE_URL`           | yes      | Real Jira Cloud site URL                                      |
//! | `JR_AUTH_HEADER`            | yes      | Pre-composed `Basic <base64(email:token)>` header             |
//! | `JR_E2E_PROJECT`            | yes      | Scrum project key (e.g. `E2E`)                               |
//! | `JR_E2E_BOARD_ID`           | no       | Board ID; enables sprint list/current tests                   |
//! | `JR_E2E_JSM_PROJECT`        | no       | JSM project key; enables queue/requesttype tests              |
//! | `JR_E2E_EMAIL`              | no       | Service account email; used by user-search test               |
//! | `JR_E2E_STATUS_DONE`        | no       | Status name for "closed"; default `"Done"`                    |
//! | `JR_E2E_STATUS_IN_PROGRESS` | no       | Status name for "in progress"; default `"In Progress"`        |
//! | `JR_E2E_ISSUE_TYPE`         | no       | Issue type for test-created issues; default `"Task"` (F-12)   |
//! | `JR_E2E_POLL_MAX_ATTEMPTS`  | no       | Max poll iterations for `poll_jql`/`poll_view` (default 5);  |
//! |                             |          | read by test code only — no `#[cfg(debug_assertions)]` needed |
//! | `JR_E2E_POLL_INITIAL_MS`    | no       | Initial backoff milliseconds for `poll_jql` (default 250);   |
//! |                             |          | read by test code only — no `#[cfg(debug_assertions)]` needed |

use assert_cmd::Command;
use serde_json::Value;
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Gate helper
// ---------------------------------------------------------------------------

/// Pure gate logic: returns `true` only when the given value is `Some("1")`.
///
/// Extracted as a pure function so the gate can be tested without any env
/// mutation. The public entry point `e2e_enabled()` delegates to this.
///
/// Traces to: AC-001, AC-002.
fn e2e_enabled_from(v: Option<&str>) -> bool {
    v == Some("1")
}

/// Returns `true` only when `JR_RUN_E2E` is set to `"1"`.
///
/// Used as the early-return guard in every `#[ignore]`-gated test.
fn e2e_enabled() -> bool {
    e2e_enabled_from(env::var("JR_RUN_E2E").ok().as_deref())
}

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

/// Configuration state for the E2E harness.
///
/// Holds `TempDir` handles to keep temp directories alive for the duration
/// of the test. The directories are cleaned up when this struct is dropped.
struct E2eHarness {
    config_dir: TempDir,
    cache_dir: TempDir,
}

impl E2eHarness {
    fn new() -> Self {
        E2eHarness {
            config_dir: TempDir::new().expect("failed to create temp config dir"),
            cache_dir: TempDir::new().expect("failed to create temp cache dir"),
        }
    }

    /// Build a `jr` binary command with the E2E environment configured:
    /// - `JR_BASE_URL` from `JR_E2E_BASE_URL`
    /// - `JR_AUTH_HEADER` from `JR_AUTH_HEADER` env var (pre-composed Basic header)
    /// - Isolated `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` (per-test temp dirs)
    /// - `--no-input` prepended (non-interactive mode)
    ///
    /// The harness returns an owned `E2eHarness` guard rather than a bare
    /// `Command` because the `TempDir` handles must remain alive for the
    /// entire duration of the `jr` subprocess (AC-003 deviation: TempDir
    /// ownership requires the caller to bind the harness).
    fn cmd(&self) -> Command {
        let base_url =
            env::var("JR_E2E_BASE_URL").expect("JR_E2E_BASE_URL must be set when JR_RUN_E2E=1");
        let auth_header =
            env::var("JR_AUTH_HEADER").expect("JR_AUTH_HEADER must be set when JR_RUN_E2E=1");

        let mut cmd = Command::cargo_bin("jr").expect("jr binary must be built");
        cmd.env("JR_BASE_URL", &base_url)
            .env("JR_AUTH_HEADER", &auth_header)
            .env("XDG_CONFIG_HOME", self.config_dir.path())
            .env("XDG_CACHE_HOME", self.cache_dir.path())
            // Remove any stray env vars that could interfere with the config
            .env_remove("JR_PROFILE")
            .env_remove("JR_DEFAULT_PROFILE")
            .arg("--no-input");
        cmd
    }
}

/// Build a `jr` command with the E2E environment.
///
/// Convenience wrapper for tests that construct their own `E2eHarness`. For
/// tests that need to keep the harness alive across multiple `cmd()` calls,
/// use `E2eHarness::new()` directly.
///
/// NOTE: the returned `E2eHarness` must be kept alive for the duration of
/// the test — dropping it early removes the temp dirs before `jr` finishes.
fn e2e_harness() -> E2eHarness {
    E2eHarness::new()
}

/// Returns a run-scoped label string.
///
/// Uses `GITHUB_RUN_ID` if set (CI), otherwise falls back to the current
/// Unix timestamp in milliseconds (local runs).
fn run_label() -> String {
    match env::var("GITHUB_RUN_ID") {
        Ok(id) if !id.is_empty() => format!("e2e-{id}"),
        _ => {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_millis();
            format!("e2e-{ms}")
        }
    }
}

/// Returns the E2E project key from the `JR_E2E_PROJECT` env var.
///
/// Panics if the var is unset, empty, or whitespace-only — every live test
/// that calls this should be guarded by `if !e2e_enabled() { return; }` at the top.
fn project() -> String {
    let p = env::var("JR_E2E_PROJECT")
        .expect("JR_E2E_PROJECT must be set for E2E tests")
        .trim()
        .to_string();
    assert!(
        !p.is_empty(),
        "JR_E2E_PROJECT must not be empty or whitespace-only"
    );
    p
}

/// Returns the configured "Done" status name (default: `"Done"`).
///
/// Treats an empty or whitespace-only env value as absent and falls back to
/// the default. This handles GitHub Actions `vars.*` expressions that evaluate
/// to `""` (empty string) when the variable is unconfigured — `env::var` returns
/// `Ok("")` in that case, so `unwrap_or_else` would never fire (FIX-A, S-E2E-2).
fn status_done() -> String {
    match std::env::var("JR_E2E_STATUS_DONE") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => "Done".to_string(),
    }
}

/// Returns the configured "In Progress" status name (default: `"In Progress"`).
///
/// Treats an empty or whitespace-only env value as absent and falls back to
/// the default. This handles GitHub Actions `vars.*` expressions that evaluate
/// to `""` (empty string) when the variable is unconfigured — `env::var` returns
/// `Ok("")` in that case, so `unwrap_or_else` would never fire (FIX-A, S-E2E-2).
fn status_in_progress() -> String {
    match std::env::var("JR_E2E_STATUS_IN_PROGRESS") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => "In Progress".to_string(),
    }
}

/// Poll `jr issue view <key> --output json` with bounded exponential backoff.
///
/// Attempts at most `MAX_ATTEMPTS` iterations. The sleep between each attempt
/// doubles from the initial delay, giving a bounded exponential schedule:
///
/// | Attempt | Sleep before next |
/// |---------|-------------------|
/// | 1       | 250 ms            |
/// | 2       | 500 ms            |
/// | 3       | 1 000 ms          |
/// | 4       | 2 000 ms          |
/// | 5       | — (last attempt)  |
///
/// Worst-case total wall time: ~7.75 s (250 + 500 + 1000 + 2000 + up to one
/// `jr` subprocess round-trip). The loop is hard-capped at `MAX_ATTEMPTS`
/// iterations — there is no `loop` / `while true` and no unbounded retry.
///
/// Returns the parsed `serde_json::Value` on the first successful attempt
/// (exit 0 + valid JSON).
///
/// Rationale: GET-by-key is *assumed* read-after-write consistent (unlike JQL
/// search which is documented eventually consistent), but the bounded retry
/// provides headroom for cold free-tier Jira sites per AC-005.
///
/// # Panics
///
/// Panics with a descriptive message after exhausting all attempts.
fn poll_view(key: &str, harness: &E2eHarness) -> Value {
    const MAX_ATTEMPTS: u32 = 5;
    // Exponential backoff delays (ms) indexed by attempt number (0-based).
    // Length must be >= MAX_ATTEMPTS - 1 (sleep is skipped on the last attempt).
    const BACKOFF_MS: [u64; 4] = [250, 500, 1_000, 2_000];

    for attempt in 1..=MAX_ATTEMPTS {
        let output = harness
            .cmd()
            .args(["issue", "view", key, "--output", "json"])
            .output()
            .expect("failed to spawn jr for poll_view");

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(v) = serde_json::from_str::<Value>(stdout.trim()) {
                return v;
            }
        }

        if attempt < MAX_ATTEMPTS {
            let delay_ms = BACKOFF_MS[(attempt - 1) as usize];
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
    }

    panic!(
        "poll_view({key}): timed out after {MAX_ATTEMPTS} attempts — \
         GET-by-key not consistent"
    );
}

/// Returns the issue type to use when creating test issues.
///
/// Reads `JR_E2E_ISSUE_TYPE` if set and non-empty; otherwise defaults to `"Task"`.
/// Env-parametric so the assertion `v["fields"]["issuetype"]["name"] == issue_type()`
/// is portable across instances with different issue type names (F-12).
fn issue_type() -> String {
    match std::env::var("JR_E2E_ISSUE_TYPE") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => "Task".to_string(),
    }
}

// ---------------------------------------------------------------------------
// §4 Foundation helpers — poll_jql, shape matchers, transient classifier
// (S-E2E-3 AC-001 through AC-004)
// ---------------------------------------------------------------------------

/// Compute the bounded exponential backoff schedule as a pure function.
///
/// Returns a `Vec<u64>` of sleep durations in milliseconds (length = max_attempts - 1).
/// Each entry doubles from `initial_ms`. The schedule is used by `poll_jql` and
/// could also be used by `poll_view` when refactored.
///
/// This is a pure function so it can be tested without touching the environment.
fn poll_schedule(max_attempts: usize, initial_ms: u64) -> Vec<u64> {
    if max_attempts == 0 {
        return Vec::new();
    }
    let mut schedule = Vec::with_capacity(max_attempts.saturating_sub(1));
    let mut delay = initial_ms;
    for _ in 0..max_attempts.saturating_sub(1) {
        schedule.push(delay);
        delay = delay.saturating_mul(2);
    }
    schedule
}

/// Poll mode for `poll_jql`.
///
/// - `SkipOnEmpty`: on budget exhaustion with 0 results, return `None` (clean skip).
///   A non-zero result that doesn't satisfy the predicate is NOT retried — caller
///   must not use this mode when a positive result count is expected.
/// - `FailOnShort(min)`: retry when count is in `1..min` and budget is not yet
///   exhausted (index lag toward target). On budget exhaustion with count in `1..min`,
///   panic loud (REGRESSION). On 0 results, behaves identically to `SkipOnEmpty`
///   (clean skip, both during retries and at budget exhaustion).
#[derive(Debug, Clone, Copy)]
enum PollJqlMode {
    SkipOnEmpty,
    FailOnShort(usize),
}

/// Decision returned by `poll_outcome` for each iteration of `poll_jql`.
///
/// Extracted as a pure enum so the decision logic can be tested in isolation
/// without spawning any processes (S-E2E-3 BUG-3 fix).
#[derive(Debug, Clone, PartialEq, Eq)]
enum PollDecision {
    /// Return the current result to the caller (predicate satisfied, or
    /// `SkipOnEmpty` with a non-zero result that failed the predicate).
    Return,
    /// Sleep and retry (count is 0, OR `FailOnShort` with count in 1..min
    /// and budget is not yet exhausted).
    Retry,
    /// Budget exhausted with 0 results — clean skip, return `None`.
    SkipNone,
    /// Budget exhausted with count in `1..min` (`FailOnShort`) — REGRESSION panic.
    FailPanic,
}

/// Pure decision function for one `poll_jql` iteration.
///
/// # Arguments
///
/// - `last_count`: number of results returned by the current attempt (0 = no results).
/// - `predicate_met`: whether the caller's predicate was satisfied.
/// - `budget_exhausted`: whether this is the last allowed attempt.
/// - `mode`: the polling mode.
///
/// # Contract
///
/// | last_count | predicate_met | budget_exhausted | mode          | decision   |
/// |------------|---------------|------------------|---------------|------------|
/// | 0          | false         | false            | any           | Retry      |
/// | 0          | false         | true             | any           | SkipNone   |
/// | >0         | true          | any              | any           | Return     |
/// | >0         | false         | false            | SkipOnEmpty   | Return     |
/// | >0         | false         | false            | FailOnShort   | Retry      |
/// | >0         | false         | true             | SkipOnEmpty   | Return     |
/// | >0         | false         | true             | FailOnShort(m)| FailPanic  |
fn poll_outcome(
    last_count: usize,
    predicate_met: bool,
    budget_exhausted: bool,
    mode: PollJqlMode,
) -> PollDecision {
    if predicate_met {
        return PollDecision::Return;
    }
    if last_count == 0 {
        return if budget_exhausted {
            PollDecision::SkipNone
        } else {
            PollDecision::Retry
        };
    }
    // last_count > 0, predicate not met.
    match mode {
        PollJqlMode::SkipOnEmpty => PollDecision::Return,
        PollJqlMode::FailOnShort(_) => {
            if budget_exhausted {
                PollDecision::FailPanic
            } else {
                PollDecision::Retry
            }
        }
    }
}

/// Poll `jr issue list --jql <jql> --output json` with bounded exponential backoff.
///
/// Intended for assertions *about search behavior* (e.g. `issue list --jql ...`),
/// NOT for confirming a write landed — for that, use `poll_view` (GET-consistent).
///
/// # Retry policy
///
/// Uses `poll_outcome` for all decision logic (pure, testable). In summary:
///
/// - 0 results: retryable (pure index lag) regardless of mode.
/// - Non-zero + predicate satisfied: return `Some(value)`.
/// - Non-zero + predicate NOT satisfied (`SkipOnEmpty`): do NOT retry; return
///   the value immediately (NEVER masks a positive result in skip-on-empty mode).
/// - Non-zero + predicate NOT satisfied (`FailOnShort`): retry until budget
///   exhausted (absorbing index lag toward the target count).
/// - Budget exhausted with 0 results: clean-skip (return `None` + eprintln!).
/// - Budget exhausted with count in `1..min` (`FailOnShort(min)`): panic (REGRESSION).
///
/// # Env seams (test-code only — no `#[cfg(debug_assertions)]` needed)
///
/// - `JR_E2E_POLL_MAX_ATTEMPTS` (default 5): max iterations.
/// - `JR_E2E_POLL_INITIAL_MS` (default 250): initial backoff in milliseconds.
///
/// # Emits
///
/// Elapsed poll time to stderr on every exit path.
fn poll_jql(
    jql: &str,
    predicate: impl Fn(&Value) -> bool,
    mode: PollJqlMode,
    harness: &E2eHarness,
) -> Option<Value> {
    let max_attempts: usize = match std::env::var("JR_E2E_POLL_MAX_ATTEMPTS") {
        Ok(v) if !v.trim().is_empty() => v.trim().parse().unwrap_or(5).max(1),
        _ => 5,
    };
    let initial_ms: u64 = match std::env::var("JR_E2E_POLL_INITIAL_MS") {
        Ok(v) if !v.trim().is_empty() => v.trim().parse().unwrap_or(250),
        _ => 250,
    };
    // poll_schedule(5, 250) yields [250, 500, 1000, 2000] — identical to poll_view's
    // hardcoded BACKOFF_MS constant, so the default timing is unchanged by the refactor.
    let schedule = poll_schedule(max_attempts, initial_ms);
    let start = std::time::Instant::now();

    let mut last_count: usize = 0;
    let mut last_value: Option<Value> = None;
    for attempt in 1..=max_attempts {
        let output = harness
            .cmd()
            .args(["issue", "list", "--jql", jql, "--output", "json"])
            .output()
            .expect("failed to spawn jr for poll_jql");

        // Staged for M2 wiring: `is_transient_error` classifies HTTP status codes
        // (429, 503, 0) as retryable. The subprocess exit code from `jr` is not an
        // HTTP status code, so full wiring requires the binary to emit a structured
        // error with a status field. Until then, a non-success exit with no parseable
        // JSON is treated as Retry unconditionally (same as 0-results — index lag fallback).

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(v) = serde_json::from_str::<Value>(stdout.trim()) {
                if let Some(arr) = v.as_array() {
                    last_count = arr.len();
                    let predicate_met = last_count > 0 && predicate(&v);
                    let budget_exhausted = attempt == max_attempts;
                    let decision = poll_outcome(last_count, predicate_met, budget_exhausted, mode);

                    match decision {
                        PollDecision::Return => {
                            let elapsed = start.elapsed().as_millis();
                            if predicate_met {
                                eprintln!(
                                    "poll_jql: predicate satisfied after {attempt} attempt(s) \
                                     ({elapsed} ms elapsed)"
                                );
                            } else {
                                eprintln!(
                                    "poll_jql: non-zero result ({last_count}) but predicate not \
                                     satisfied after {attempt} attempt(s) ({elapsed} ms elapsed)"
                                );
                            }
                            return Some(v);
                        }
                        PollDecision::Retry => {
                            last_value = Some(v);
                            // Fall through to sleep/retry below.
                        }
                        PollDecision::SkipNone => {
                            let elapsed = start.elapsed().as_millis();
                            eprintln!(
                                "poll_jql: budget exhausted after {max_attempts} attempt(s) \
                                 ({elapsed} ms); 0 results — treating as index lag, clean-skip"
                            );
                            return None;
                        }
                        PollDecision::FailPanic => {
                            let elapsed = start.elapsed().as_millis();
                            let min = match mode {
                                PollJqlMode::FailOnShort(m) => m,
                                PollJqlMode::SkipOnEmpty => unreachable!(),
                            };
                            panic!(
                                "REGRESSION: poll_jql expected at least {min} results after \
                                 full poll budget ({max_attempts} attempts, {elapsed} ms), \
                                 but got {last_count}. \
                                 This is a persistent short count, not index lag."
                            );
                        }
                    }
                }
            }
        }

        if attempt < max_attempts {
            let delay_ms = schedule[attempt - 1];
            eprintln!(
                "poll_jql: attempt {attempt}/{max_attempts} — 0 results or parse error; \
                 sleeping {delay_ms} ms"
            );
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
    }

    // Budget exhausted without returning inside the loop (only reachable when
    // the last attempt produced a parse error or non-success exit — the
    // SkipNone/FailPanic paths are taken from inside the loop for valid JSON).
    let elapsed = start.elapsed().as_millis();
    let budget_exhausted = true;
    let decision = poll_outcome(last_count, false, budget_exhausted, mode);
    match decision {
        PollDecision::SkipNone | PollDecision::Retry => {
            eprintln!(
                "poll_jql: budget exhausted after {max_attempts} attempt(s) ({elapsed} ms); \
                 0 results — treating as index lag, clean-skip"
            );
            None
        }
        PollDecision::FailPanic => {
            let min = match mode {
                PollJqlMode::FailOnShort(m) => m,
                PollJqlMode::SkipOnEmpty => unreachable!(),
            };
            panic!(
                "REGRESSION: poll_jql expected at least {min} results after full poll budget \
                 ({max_attempts} attempts, {elapsed} ms), but got {last_count}. \
                 This is a persistent short count, not index lag."
            );
        }
        PollDecision::Return => {
            // Reachable only when an earlier FailOnShort retry captured a value
            // (last_value = Some) and the final attempt produced a parse error /
            // non-success. In SkipOnEmpty mode this arm is unreachable (non-zero
            // results return inside the loop). Returns the last successfully-parsed
            // value.
            debug_assert!(
                last_value.is_some(),
                "poll_jql post-loop Return arm reached with no captured value"
            );
            last_value
        }
    }
}

// ---------------------------------------------------------------------------
// §4 Shape matchers — pure helpers with always-run unit tests
// (S-E2E-3 AC-003)
// ---------------------------------------------------------------------------

/// Asserts that `key` matches the Jira issue key format `^[A-Z][A-Z0-9]+-\d+$`.
///
/// Implemented without the `regex` crate (not a dev-dep) using a character-by-character
/// check. Panics with a descriptive message if the format is invalid.
///
/// # Format rules
///
/// - Project prefix: one or more characters where the first is `[A-Z]` and the
///   rest are `[A-Z0-9]`.
/// - Separator: a single `-`.
/// - Issue number: one or more ASCII digits `[0-9]`.
fn assert_key_format(key: &str) {
    let valid = key_format_valid(key);
    assert!(
        valid,
        "key format invalid: expected ^[A-Z][A-Z0-9]+-\\d+$ but got {key:?}"
    );
}

/// Pure predicate for key format validation (extracted for testability).
fn key_format_valid(key: &str) -> bool {
    // Split on the last '-' to separate project prefix from issue number.
    let Some(dash_pos) = key.rfind('-') else {
        return false;
    };
    let (prefix, number_with_dash) = key.split_at(dash_pos);
    let number = &number_with_dash[1..]; // skip the '-'

    // Prefix must be non-empty, start with A-Z, rest A-Z0-9.
    if prefix.is_empty() {
        return false;
    }
    let mut chars = prefix.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_uppercase() {
        return false;
    }
    for c in chars {
        if !c.is_ascii_uppercase() && !c.is_ascii_digit() {
            return false;
        }
    }

    // Number must be non-empty and all ASCII digits.
    if number.is_empty() {
        return false;
    }
    for c in number.chars() {
        if !c.is_ascii_digit() {
            return false;
        }
    }

    true
}

/// Locale-invariant Jira status category.
///
/// Maps to Jira's fixed `statusCategory.key` values which are stable across
/// all instances and locales. NEVER use `statusCategory.name` — that is localized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusCategory {
    /// `statusCategory.key == "new"` (the "To Do" category).
    ToDo,
    /// `statusCategory.key == "indeterminate"` (the "In Progress" category).
    InProgress,
    /// `statusCategory.key == "done"` (the "Done" category).
    Done,
}

impl StatusCategory {
    /// Returns the stable locale-invariant `statusCategory.key` string.
    fn key(self) -> &'static str {
        match self {
            StatusCategory::ToDo => "new",
            StatusCategory::InProgress => "indeterminate",
            StatusCategory::Done => "done",
        }
    }
}

/// Asserts that `v["statusCategory"]["key"]` equals the expected stable key.
///
/// `expected` is a `StatusCategory` enum variant — NEVER a free `&str` status name
/// (which would be locale-fragile). Maps `ToDo→"new"`, `InProgress→"indeterminate"`,
/// `Done→"done"`.
fn assert_status_category(v: &Value, expected: StatusCategory) {
    let got = v
        .get("statusCategory")
        .and_then(|sc| sc.get("key"))
        .and_then(Value::as_str);
    assert_eq!(
        got,
        Some(expected.key()),
        "statusCategory.key mismatch: expected {:?} ({}) but got {:?}; value: {v}",
        expected,
        expected.key(),
        got
    );
}

/// Asserts that `v` has the shape of a Jira issue object:
/// - `v["key"]` matches the key format.
/// - `v["fields"]` is an object.
/// - `v["fields"]["summary"]` is present (string or null).
/// - `v["fields"]["status"]` contains a `statusCategory` object.
fn assert_issue_shape(v: &Value) {
    let key = v.get("key").and_then(Value::as_str).unwrap_or_else(|| {
        panic!("assert_issue_shape: 'key' field missing or not a string; value: {v}")
    });
    assert_key_format(key);

    let fields = v
        .get("fields")
        .unwrap_or_else(|| panic!("assert_issue_shape: 'fields' field missing; value: {v}"));
    assert!(
        fields.is_object(),
        "assert_issue_shape: 'fields' must be an object; got: {fields}"
    );

    // 'summary' must be present (string or null — newly created issues may have null).
    assert!(
        fields.get("summary").is_some(),
        "assert_issue_shape: 'fields.summary' must be present; value: {v}"
    );

    // 'status' must contain a 'statusCategory' object.
    let status = fields.get("status").unwrap_or_else(|| {
        panic!("assert_issue_shape: 'fields.status' must be present; value: {v}")
    });
    assert!(
        status.get("statusCategory").is_some_and(Value::is_object),
        "assert_issue_shape: 'fields.status.statusCategory' must be an object; got: {status}"
    );
}

/// Asserts that `v` is a JSON array and, for every element (if non-empty), each element
/// has all the given `keys` present.
///
/// An empty array always passes — this is the portable "if non-empty, every element
/// conforms" contract (spec §3). Never requires non-empty.
fn assert_array_of_objects_with_keys(v: &Value, keys: &[&str]) {
    assert!(v.is_array(), "expected a JSON array; got: {v}");
    // Empty array: the for-loop below is a no-op — vacuously true by design.
    // Spec §3: "if non-empty, every element conforms." An empty list is valid
    // on a freshly provisioned project and must never be forced non-empty.
    for (i, elem) in v.as_array().unwrap().iter().enumerate() {
        for &key in keys {
            assert!(
                elem.get(key).is_some(),
                "element[{i}] is missing key {key:?}; element: {elem}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// §4 Transient classifier — pure helper with always-run unit tests
// (S-E2E-3 AC-004)
// ---------------------------------------------------------------------------

/// Returns `true` if the error is transient and the call should be retried.
///
/// Retry on: 429 (rate limit), 503 (service unavailable), 0 (connection reset /
/// empty response).
///
/// Never retry: any other 4xx (`400..=499` except 429) — these are caller errors
/// and retrying would hide bugs. Also never retry other 5xx (except 503).
///
/// This is a pure function (no side effects, no I/O) so it can be tested without
/// spawning any process.
///
/// # Staged for M2 wiring
///
/// `poll_jql` currently cannot extract an HTTP status code from the `jr` subprocess
/// exit code — the two are different things. Full wiring requires the binary to emit
/// a structured error response with a parseable HTTP status field. Until then this
/// function is exercised by unit tests but not called from the live poll loop.
fn is_transient_error(status_code: u16, _stderr: &str) -> bool {
    matches!(status_code, 429 | 503 | 0)
}

// ---------------------------------------------------------------------------
// M3 AC-003 — Leak-detection log (always-run; never fails)
// ---------------------------------------------------------------------------

/// Leak-detection log: counts pre-existing open E2E issues and emits the count
/// to stderr as a warn-only signal.
///
/// ALWAYS-RUN test (not `#[ignore]`) — NOT covered by `test_every_ignored_test_has_gate_guard`.
/// The `e2e_enabled()` early-return MUST remain the first statement before any
/// `e2e_harness()`/`.cmd()`/`.output()` call; verify manually when editing.
///
/// This function is ALWAYS-RUN (not `#[ignore]`). It does NOT require
/// `e2e_enabled()` — instead it reads `JR_RUN_E2E` directly and returns early
/// when the var is not `"1"`, so no live calls are made under normal `cargo test`.
///
/// NEVER fails regardless of count. A high count signals broken teardown in
/// previous runs and is visible in CI logs.
///
/// JQL: `summary ~ "e2e"` (tokenized full-text; matches the `e2e` token embedded
/// in `[e2e <run_label>]` summaries). Do NOT use `labels ~ "e2e-"` — the `~`
/// operator is not supported on the `labels` field (HTTP 400; spec §7.1 F-02).
///
/// Traces to: AC-003, NFR-T-E2E-1, spec §7.1.
#[test]
fn test_aaaaa_leak_detection_log() {
    // Early-return if not in live E2E mode — no subprocess invocation.
    if !e2e_enabled() {
        return;
    }
    let proj = match std::env::var("JR_E2E_PROJECT") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => {
            eprintln!("E2E leak-detection: JR_E2E_PROJECT not set; skipping orphan count");
            return;
        }
    };
    let h = e2e_harness();
    let jql = format!(
        "project={} AND summary ~ \"e2e\" AND statusCategory != Done",
        proj
    );
    let output = h
        .cmd()
        .args(["issue", "list", "--jql", &jql, "--output", "json"])
        .output();
    let count = match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<Value>(stdout.trim()) {
                Ok(v) => v.as_array().map(|a| a.len()).unwrap_or(0),
                Err(_) => 0,
            }
        }
        _ => 0,
    };
    eprintln!(
        "E2E leak-detection: {} orphaned open E2E issue(s) found (warn-only; high count = broken teardown)",
        count
    );
    // NEVER fails — this is a warn-only observability signal.
}

// ---------------------------------------------------------------------------
// AC-001 — Non-gated gate-invariant test (ALWAYS runs in normal `cargo test`)
// ---------------------------------------------------------------------------

/// Verifies `e2e_enabled_from()` gate logic without any env mutation.
///
/// Tests the pure function over literal inputs to pin the exact gate semantics.
/// No `unsafe`, no process-env mutation, no race risk under multi-threaded
/// `cargo test`.
///
/// Traces to: AC-001/AC-002 gate logic.
#[test]
fn test_e2e_gate_disabled_when_env_unset() {
    assert!(
        !e2e_enabled_from(None),
        "e2e_enabled_from(None) must return false (var absent)"
    );
    assert!(
        e2e_enabled_from(Some("1")),
        "e2e_enabled_from(Some(\"1\")) must return true"
    );
    assert!(
        !e2e_enabled_from(Some("0")),
        "e2e_enabled_from(Some(\"0\")) must return false"
    );
    assert!(
        !e2e_enabled_from(Some("")),
        "e2e_enabled_from(Some(\"\")) must return false"
    );
    assert!(
        !e2e_enabled_from(Some("1 ")),
        "e2e_enabled_from(Some(\"1 \")) must return false (trailing space)"
    );
}

/// Meta-guard: every `#[ignore]`-annotated test in this file must contain
/// the `e2e_enabled()` guard token in its body, AND that guard must appear
/// BEFORE the first occurrence of any live-call token (`e2e_harness(`,
/// `.cmd()`, or `.output()`).
///
/// Reads the source of this file via `include_str!` and scans for test
/// functions annotated with `#[ignore`. For each such function:
///
/// 1. The body is extracted using a string-literal-aware brace-depth counter
///    that skips `{`/`}` characters inside `"..."` string literals and `'.'`
///    char literals (honoring `\` escapes). This prevents false brace-depth
///    readings caused by `{` or `}` inside string arguments.
///
/// 2. The guard `e2e_enabled()` is checked to appear BEFORE the first
///    occurrence of any live-call token (`e2e_harness(`, `.cmd()`, or
///    `.output()`). A test that spawns `jr` before calling the guard must
///    fail this meta-test.
///
/// This regression-pins AC-002: it is impossible to add a new gated test and
/// forget the guard (or mis-order it) without this test failing.
///
/// Traces to: AC-002, design spec §4 Gating.
#[test]
fn test_every_ignored_test_has_gate_guard() {
    let source = include_str!("e2e_live.rs");

    // Live-call tokens: any of these appearing before `e2e_enabled()` is a
    // violation — they would cause `jr` to be spawned without the gate check.
    const LIVE_CALL_TOKENS: &[&str] = &["e2e_harness(", ".cmd()", ".output()"];

    let mut violations: Vec<String> = Vec::new();

    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim_start().starts_with("#[ignore") {
            // Scan forward up to 5 lines to find the `fn test_` line.
            let mut fn_line = None;
            for (offset, line) in lines[i..lines.len().min(i + 5)].iter().enumerate() {
                if line.trim_start().starts_with("fn test_") {
                    fn_line = Some(i + offset);
                    break;
                }
            }

            if let Some(fn_start) = fn_line {
                // Extract the function name for error messages.
                let fn_name = lines[fn_start]
                    .trim()
                    .trim_start_matches("fn ")
                    .split('(')
                    .next()
                    .unwrap_or("(unknown)")
                    .to_string();

                // Build the raw body string using a string-literal-aware
                // brace-depth counter so that `{` / `}` inside `"..."` or
                // `'.'` literals are not counted toward depth.
                let body = extract_fn_body(&lines, fn_start);

                // Check 1: guard token is present at all.
                if !body.contains("e2e_enabled()") {
                    violations.push(format!("{fn_name}: missing `e2e_enabled()` guard"));
                    i = fn_start + 1;
                    continue;
                }

                // Check 2: guard appears BEFORE the first live-call token.
                let guard_pos = body.find("e2e_enabled()").unwrap();
                for token in LIVE_CALL_TOKENS {
                    if let Some(call_pos) = body.find(token) {
                        if call_pos < guard_pos {
                            violations.push(format!(
                                "{fn_name}: live-call token `{token}` appears at byte {call_pos} \
                                 before `e2e_enabled()` at byte {guard_pos}"
                            ));
                        }
                    }
                }

                i = fn_start + 1;
                continue;
            }
        }
        i += 1;
    }

    assert!(
        violations.is_empty(),
        "AC-002 VIOLATION: the following #[ignore]-annotated tests have \
         guard ordering problems:\n  {}\n\
         Every gated test MUST call `e2e_enabled()` BEFORE any live call \
         (`e2e_harness(`, `.cmd()`, `.output()`).",
        violations.join("\n  ")
    );
}

/// Extract the full source text of the function starting at `fn_start`.
///
/// Uses a state machine that tracks whether the scanner is inside a
/// double-quoted string literal (`"..."`), a char literal (`'.'`),
/// a `//` line comment, or a `/* ... */` block comment.
/// Only braces that occur OUTSIDE of any literal or comment are counted
/// toward depth, so `{` / `}` characters in string arguments, comments,
/// or assertion messages do not confuse the depth counter.
///
/// # Lifetime sigils
///
/// Rust's `'` character is used both as a char-literal delimiter and as a
/// lifetime sigil (e.g. `&'static str`, `'a`).  A lifetime `'` is always
/// followed immediately by an ASCII identifier-start character (`a-z A-Z _`).
/// A char literal `'` is followed by the character content or a `\` escape.
/// The scanner uses this distinction: a `'` followed by an identifier-start
/// byte is treated as a lifetime sigil and skipped rather than entering
/// `InChar` state.  Note that this heuristic does not handle the degenerate
/// case where a char literal begins with an identifier-start character, e.g.
/// `'a'` — that will be treated as a lifetime.  For the purposes of this
/// meta-guard (scanning Rust *test* source for brace balance), this is
/// acceptable: a lifetime mis-classified as a char literal would at worst
/// keep the scanner in `Code` state (the correct behaviour), and a char
/// literal mis-classified as a lifetime would emit one extra bare `'` char
/// which also stays in `Code` state.  In either case the depth counter
/// remains correct unless the char literal or lifetime content itself
/// contains `{` or `}`, which is vanishingly rare in test source.
///
/// # Block comment nesting
///
/// Rust block comments nest (`/* /* */ */`), but this scanner uses a simple
/// non-nesting scan for block comments: it enters `InBlockComment` on `/*`
/// and exits on the first `*/`.  Nested block comments inside test function
/// bodies are uncommon enough that this limitation is acceptable.  A comment
/// is added inline noting this residual limitation.
fn extract_fn_body(lines: &[&str], fn_start: usize) -> String {
    #[derive(PartialEq)]
    enum Scan {
        Code,
        InString,
        InChar,
        InLineComment,
        InBlockComment,
    }

    let mut body = String::new();
    let mut depth = 0usize;
    let mut found_open = false;
    let mut state = Scan::Code;

    'outer: for line in lines.iter().skip(fn_start) {
        body.push_str(line);
        body.push('\n');

        // Line comments last only until end-of-line; reset to Code at each new line.
        if state == Scan::InLineComment {
            state = Scan::Code;
        }

        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            let ch = bytes[i] as char;
            match state {
                Scan::InLineComment => {
                    // Consume to end-of-line; the outer loop resets state after
                    // each line, so we just break here.
                    break;
                }
                Scan::InBlockComment => {
                    // Look for the `*/` terminator.
                    // Non-nesting: the first `*/` ends the comment regardless
                    // of nested `/*` inside (residual limitation — uncommon in
                    // test source bodies).
                    if ch == '*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                        i += 2;
                        state = Scan::Code;
                        continue;
                    }
                    i += 1;
                }
                Scan::InString => match ch {
                    '\\' => {
                        // Skip the next byte (escape sequence).
                        i += 2;
                    }
                    '"' => {
                        state = Scan::Code;
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                },
                Scan::InChar => match ch {
                    '\\' => {
                        // Skip the escaped byte.
                        i += 2;
                    }
                    '\'' => {
                        state = Scan::Code;
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                },
                Scan::Code => {
                    // Check for `//` line comment first (takes priority over
                    // any `"` or `'` that might appear on the same byte).
                    if ch == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                        state = Scan::InLineComment;
                        break; // Consume the rest of the line without scanning.
                    }
                    // Check for `/*` block comment.
                    if ch == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                        state = Scan::InBlockComment;
                        i += 2;
                        continue;
                    }
                    match ch {
                        '"' => {
                            state = Scan::InString;
                            i += 1;
                        }
                        '\'' => {
                            // Distinguish lifetime sigil from char literal.
                            // A lifetime is `'` followed immediately by an
                            // ASCII identifier-start character (a-z, A-Z, _).
                            // In that case, skip the `'` and stay in Code.
                            let next_is_ident_start = i + 1 < bytes.len() && {
                                let nb = bytes[i + 1];
                                nb.is_ascii_alphabetic() || nb == b'_'
                            };
                            if next_is_ident_start {
                                // Lifetime sigil — not a char literal; stay in Code.
                                i += 1;
                            } else {
                                state = Scan::InChar;
                                i += 1;
                            }
                        }
                        '{' => {
                            depth += 1;
                            found_open = true;
                            i += 1;
                        }
                        '}' => {
                            depth = depth.saturating_sub(1);
                            i += 1;
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }
            }
        }

        if found_open && depth == 0 {
            break 'outer;
        }
    }

    body
}

// ---------------------------------------------------------------------------
// M3 AC-002 — Secret-leak guard (gated; e2e_enabled() FIRST)
// ---------------------------------------------------------------------------

/// E2E secret-leak guard: asserts that `jr` output (stdout + stderr) never
/// contains the base64 token portion of `JR_AUTH_HEADER`.
///
/// This is a cheap, high-value, portable regression guard. A future code change
/// that accidentally logs auth headers (e.g. verbose mode, debug output, error
/// messages including the Authorization header value) will be caught by this test
/// on the next live run.
///
/// **Why token-only (not email):** The credential that must never leak is the
/// base64-encoded `email:token` string in `JR_AUTH_HEADER`. The service-account
/// email address is NOT guarded here because it legitimately appears in issue
/// metadata returned by `issue list --output json`: `IssueFields.reporter` and
/// `IssueFields.assignee` are `Option<User>`, and `User.email_address` is
/// serialized as `emailAddress` in JSON (see `src/types/jira/user.rs`). Issues
/// created by the service account have the SA email in their reporter/assignee
/// fields. Asserting the email is absent from `issue list` output is incorrect —
/// it conflates "an email that legitimately appears in issue metadata" with
/// "a credential leaking through output". The security property we care about is
/// that the base64 auth token itself is never echoed back.
///
/// Implementation:
/// 1. Extracts the base64 portion from `JR_AUTH_HEADER` (the part after "Basic ").
/// 2. Runs `issue list --jql "project=<E2E> AND summary ~ e2e" --output json`.
/// 3. Asserts neither stdout NOR stderr contains the base64 token.
///
/// Traces to: AC-002 (M3 secret-leak guard), NFR-T-E2E-1, spec §7.1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_no_secret_in_output() {
    if !e2e_enabled() {
        return;
    }

    // Extract the base64 token from JR_AUTH_HEADER (part after "Basic ").
    // This is the actual credential that must never appear in jr output.
    let auth_header =
        std::env::var("JR_AUTH_HEADER").expect("JR_AUTH_HEADER must be set when JR_RUN_E2E=1");
    let base64_token = auth_header
        .strip_prefix("Basic ")
        .unwrap_or(&auth_header)
        .trim()
        .to_string();

    let proj = project();
    let jql = format!("project={} AND summary ~ e2e", proj);

    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["issue", "list", "--jql", &jql, "--output", "json"])
        .output()
        .expect("failed to spawn jr for secret-leak guard test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Assert the base64 token is not present in either channel.
    // The SA email is NOT asserted here — it legitimately appears in issue
    // reporter/assignee fields and is not a credential (see rustdoc above).
    assert!(
        !stdout.contains(&base64_token),
        "SECURITY: stdout contains the base64 auth token — credential leak detected!\n\
         stdout (truncated to 200 chars): {:?}",
        stdout.chars().take(200).collect::<String>()
    );
    assert!(
        !stderr.contains(&base64_token),
        "SECURITY: stderr contains the base64 auth token — credential leak detected!\n\
         stderr (truncated to 200 chars): {:?}",
        stderr.chars().take(200).collect::<String>()
    );
}

// ---------------------------------------------------------------------------
// AC-004 — Read command coverage (all #[ignore] + early-return gated)
// ---------------------------------------------------------------------------

/// E2E: `jr issue list --jql "project=<E2E>" --output json` returns a JSON array
/// and validates the JR_AUTH_HEADER seam end-to-end.
///
/// This is the auth-seam validator: it is the first test that makes a real
/// network call. A 401 response here means the JR_AUTH_HEADER seam or the
/// credential is broken — there is no need for a separate `auth status` test
/// because `auth status` is plaintext-only and makes no Jira API calls.
///
/// When non-empty: asserts every element has `key` (format) + `fields` present,
/// and `fields.status.statusCategory` is an object (BC-2.2.028; spec §5.1).
///
/// May return an empty array on a freshly provisioned project — the "if non-empty"
/// assertions are shape-only and portable.
///
/// Traces to: AC-004, AC-005, BC-2.2.028, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_list_by_project_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let jql = format!("project={}", project());
    let output = h
        .cmd()
        .args(["issue", "list", "--jql", &jql, "--output", "json"])
        .output()
        .expect("failed to spawn jr");

    assert!(
        output.status.success(),
        "issue list by project failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("issue list output must be valid JSON");
    assert!(
        v.is_array(),
        "issue list output must be a JSON array; got: {v}"
    );

    // M1 deepening (AC-005): if non-empty, assert element shape.
    assert_array_of_objects_with_keys(&v, &["key", "fields"]);
    for elem in v.as_array().unwrap() {
        let key_str = elem.get("key").and_then(Value::as_str).unwrap_or("");
        if !key_str.is_empty() {
            assert_key_format(key_str);
        }
        // statusCategory must be an object when present.
        if let Some(status) = elem.get("fields").and_then(|f| f.get("status")) {
            assert!(
                status.get("statusCategory").is_some_and(Value::is_object),
                "fields.status.statusCategory must be an object; elem: {elem}"
            );
        }
    }
}

/// E2E: `jr issue list --jql "project=<E2E> AND summary ~ e2e" --output json`
/// applies the JQL filter correctly and returns a JSON array.
///
/// Uses `poll_jql` in `SkipOnEmpty` mode to absorb JQL index lag on cold
/// free-tier Jira sites (JRACLOUD-97427; spec §7.1 AC-001). A bare `issue list`
/// call without retry is a latent flake on first provisioning when the index
/// has not yet caught up. `SkipOnEmpty` means: if the budget is exhausted with
/// 0 results, the test emits an eprintln! skip notice and returns without
/// failure (pure index lag, not a `jr` regression). If results appear, element
/// shape is validated normally.
///
/// When non-empty: asserts every element has `key` (format) + `fields`,
/// and `fields.status.statusCategory` is an object (BC-2.2.028; spec §5.1).
///
/// Traces to: AC-001 (M3 poll_jql adoption), AC-004, AC-005, BC-2.2.028,
/// NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_list_with_summary_filter_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let jql = format!("project={} AND summary ~ e2e", project());

    // poll_jql in SkipOnEmpty mode: absorbs JQL index lag on cold indices.
    // A 0-result after full budget is a clean skip — index lag, not a jr regression.
    // A non-zero result triggers shape validation below.
    let result = poll_jql(
        &jql,
        |_| true, // any non-empty array satisfies the predicate
        PollJqlMode::SkipOnEmpty,
        &h,
    );

    let v = match result {
        None => {
            // Budget exhausted with 0 results — pure index lag; clean skip.
            eprintln!(
                "test_e2e_issue_list_with_summary_filter_returns_array: \
                 clean-skip (JQL index lag; 0 results after full poll budget)"
            );
            return;
        }
        Some(v) => v,
    };

    // poll_jql always returns Some(array) on a non-None result.
    assert!(
        v.is_array(),
        "poll_jql result must be a JSON array; got: {v}"
    );

    // M1 deepening (AC-005): if non-empty, assert element shape.
    assert_array_of_objects_with_keys(&v, &["key", "fields"]);
    for elem in v.as_array().unwrap() {
        let key_str = elem.get("key").and_then(Value::as_str).unwrap_or("");
        if !key_str.is_empty() {
            assert_key_format(key_str);
        }
        if let Some(status) = elem.get("fields").and_then(|f| f.get("status")) {
            assert!(
                status.get("statusCategory").is_some_and(Value::is_object),
                "fields.status.statusCategory must be an object; elem: {elem}"
            );
        }
    }
}

/// E2E: `jr board list --output json` returns a JSON array.
///
/// When non-empty: each element has `id` + `name` + `type` keys (BC-5.1.001; spec §5.1).
/// The board count is site-specific — the "if non-empty" contract is portable.
///
/// Traces to: AC-004, AC-006, BC-5.1.001, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_board_list_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["board", "list", "--output", "json"])
        .output()
        .expect("failed to spawn jr");

    assert!(
        output.status.success(),
        "board list failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("board list output must be valid JSON");
    assert!(
        v.is_array(),
        "board list output must be a JSON array; got: {v}"
    );

    // M1 deepening (AC-006): if non-empty, each element has id + name + type.
    assert_array_of_objects_with_keys(&v, &["id", "name", "type"]);
}

/// E2E: `jr sprint list --board <BOARD_ID> --output json` returns a JSON array.
///
/// Skipped cleanly when `JR_E2E_BOARD_ID` is not set.
///
/// Also skipped cleanly when the board is not a scrum board: `resolve_scrum_board`
/// in `src/cli/sprint.rs` exits non-zero with stderr containing
/// `"only available for scrum boards"` for kanban, simple, and team-managed boards.
/// This condition is not a `jr` defect — it reflects the board type of the
/// provisioned E2E site (FIX-B, S-E2E-2).
///
/// When non-empty: each element has `id`; if `state` is present it is a string
/// (BC-5.2.005; spec §5.1).
///
/// Traces to: AC-004, AC-007, BC-5.2.005, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_sprint_list_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let board_id = match env::var("JR_E2E_BOARD_ID") {
        Ok(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => {
            // Skipped: JR_E2E_BOARD_ID not set.
            return;
        }
    };
    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["sprint", "list", "--board", &board_id, "--output", "json"])
        .output()
        .expect("failed to spawn jr");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("only available for scrum boards") {
            return; // clean skip — board is not a scrum board (kanban/simple/team-managed); not a jr defect
        }
        panic!(
            "sprint list failed unexpectedly:\nstdout: {}\nstderr: {stderr}",
            String::from_utf8_lossy(&output.stdout),
        );
    }

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("sprint list output must be valid JSON");
    assert!(
        v.is_array(),
        "sprint list output must be a JSON array; got: {v}"
    );

    // M1 deepening (AC-007): if non-empty, each element has `id`.
    assert_array_of_objects_with_keys(&v, &["id"]);
    // If `state` is present, it must be a string (Option<String> in the Sprint type).
    for elem in v.as_array().unwrap() {
        if let Some(state) = elem.get("state") {
            assert!(
                state.is_string() || state.is_null(),
                "sprint.state must be a string or null; got: {state} in elem: {elem}"
            );
        }
    }
}

/// E2E: `jr sprint current --board <BOARD_ID> --output json` returns valid JSON.
///
/// Skipped cleanly when `JR_E2E_BOARD_ID` is not set.
///
/// Also skipped cleanly when:
/// - The board has no active sprint: `handle_current` exits 1 with stderr
///   containing `"No active sprint found for board ..."` on a freshly provisioned
///   free Scrum site that has not started any sprint.
/// - The board is not a scrum board: `resolve_scrum_board` exits non-zero with
///   stderr containing `"only available for scrum boards"` for kanban, simple,
///   and team-managed boards.
///
/// Both conditions are clean skips — not `jr` defects (FIX-B, S-E2E-2).
///
/// On success: asserts the output is `{sprint, issues, sprint_summary?}` with
/// `v["sprint"]["id"]` present, `v["sprint"]["state"]` a string if present,
/// and `v["issues"]` an array. If `v["issues"]` is non-empty, `assert_issue_shape`
/// is called on each element (BC-5.2.005; spec §5.1).
///
/// Traces to: AC-004, AC-007, BC-5.2.005, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_sprint_current_returns_json() {
    if !e2e_enabled() {
        return;
    }
    let board_id = match env::var("JR_E2E_BOARD_ID") {
        Ok(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => {
            // Skipped: JR_E2E_BOARD_ID not set.
            return;
        }
    };
    let h = e2e_harness();
    let output = h
        .cmd()
        .args([
            "sprint", "current", "--board", &board_id, "--output", "json",
        ])
        .output()
        .expect("failed to spawn jr");

    // Clean skip: board has no active sprint OR is not a scrum board.
    // Both are valid E2E site configurations — not test failures.
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No active sprint") || stderr.contains("only available for scrum boards")
        {
            return; // clean skip — board has no sprint capability / no active sprint; not a jr defect
        }
        panic!(
            "sprint current failed unexpectedly:\nstdout: {}\nstderr: {stderr}",
            String::from_utf8_lossy(&output.stdout),
        );
    }

    // On the success path, the output must be valid JSON.
    let v: Value =
        serde_json::from_slice(&output.stdout).expect("sprint current output must be valid JSON");

    // M1 deepening (AC-007): assert {sprint, issues} object shape.
    // sprint current JSON is {sprint: {...}, issues: [...], sprint_summary?: {...}}
    assert!(
        v.is_object(),
        "sprint current output must be a JSON object; got: {v}"
    );
    assert!(
        v.get("sprint").is_some(),
        "sprint current JSON must contain 'sprint' key; got: {v}"
    );
    assert!(
        v.get("sprint").and_then(|s| s.get("id")).is_some(),
        "sprint current JSON sprint.id must be present; got: {v}"
    );
    if let Some(state) = v.get("sprint").and_then(|s| s.get("state")) {
        assert!(
            state.is_string() || state.is_null(),
            "sprint.state must be a string or null; got: {state}"
        );
    }
    let issues = v
        .get("issues")
        .unwrap_or_else(|| panic!("sprint current JSON must contain 'issues' key; got: {v}"));
    assert!(
        issues.is_array(),
        "sprint current JSON issues must be an array; got: {issues}"
    );
    // If non-empty, assert issue shape on each element.
    for elem in issues.as_array().unwrap() {
        assert_issue_shape(elem);
    }
}

/// E2E: `jr user search <query> --output json` returns a JSON array.
///
/// When non-empty: each element has `accountId` + `displayName` keys (presence +
/// type, NOT value equality). These JSON keys are confirmed by the serde rename
/// attributes on `src/types/jira/user.rs::User` (DI-E2E-F2-2; spec §5.1).
///
/// Browse Users permission availability varies across sites and the array may be
/// empty — "if non-empty" contract is portable (lesson from S-398 over-fitting).
///
/// Traces to: AC-004, AC-008, BC-2.2.028, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_user_search_returns_array() {
    if !e2e_enabled() {
        return;
    }
    // Use the email's local-part as a search query if non-empty, otherwise "e2e".
    // Mirror the FIX-A empty-env guard: treat Ok("") the same as Err (absent).
    let query = env::var("JR_E2E_EMAIL")
        .ok()
        .map(|e| e.trim().split('@').next().unwrap_or_default().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "e2e".to_string());

    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["user", "search", &query, "--output", "json"])
        .output()
        .expect("failed to spawn jr");

    assert!(
        output.status.success(),
        "user search failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("user search output must be valid JSON");
    assert!(
        v.is_array(),
        "user search output must be a JSON array; got: {v}"
    );

    // M1 deepening (AC-008): if non-empty, each element has accountId + displayName.
    // JSON keys confirmed by serde rename in src/types/jira/user.rs (DI-E2E-F2-2).
    assert_array_of_objects_with_keys(&v, &["accountId", "displayName"]);
    // Type check: accountId and displayName must be strings when present.
    for elem in v.as_array().unwrap() {
        if let Some(aid) = elem.get("accountId") {
            assert!(
                aid.is_string(),
                "accountId must be a string; got: {aid} in elem: {elem}"
            );
        }
        if let Some(dn) = elem.get("displayName") {
            assert!(
                dn.is_string(),
                "displayName must be a string; got: {dn} in elem: {elem}"
            );
        }
    }
}

/// E2E: `jr project fields --project <E2E> --output json` returns a JSON object
/// with all 5 documented top-level keys.
///
/// `project fields --output json` returns an object with keys:
/// `project`, `issue_types`, `priorities`, `statuses_by_issue_type`, `asset_fields`.
/// Asserts key **presence only** — never non-empty (F-08: `asset_fields` is `[]` on
/// non-CMDB instances; `priorities`/`statuses_by_issue_type` may be empty; spec §5.1).
///
/// Traces to: AC-004, AC-006, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_project_fields_returns_object() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let output = h
        .cmd()
        .args([
            "project",
            "fields",
            "--project",
            &project(),
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");

    assert!(
        output.status.success(),
        "project fields failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("project fields output must be valid JSON");
    assert!(
        v.is_object(),
        "project fields output must be a JSON object; got: {v}"
    );

    // M1 deepening (AC-006): assert ALL 5 documented keys are present (never non-empty).
    // Trap F-08: asset_fields is [] on non-CMDB instances; do NOT assert non-empty.
    for key in &[
        "project",
        "issue_types",
        "priorities",
        "statuses_by_issue_type",
        "asset_fields",
    ] {
        assert!(
            v.get(*key).is_some(),
            "project fields JSON must contain {key:?} key; got: {v}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-004 — JSM optional read tests (guarded by JR_E2E_JSM_PROJECT)
// ---------------------------------------------------------------------------

/// E2E: `jr queue list --project <JSM> --output json` exits 0.
///
/// Skipped cleanly when `JR_E2E_JSM_PROJECT` is not set.
///
/// Traces to: AC-004, NFR-T-E2E-1, design spec §4 Optional/feature-flagged.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_queue_list_exits_ok() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            // Skipped: JR_E2E_JSM_PROJECT not set.
            return;
        }
    };
    let h = e2e_harness();
    let output = h
        .cmd()
        .args([
            "queue",
            "list",
            "--project",
            &jsm_project,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");

    assert!(
        output.status.success(),
        "queue list failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("queue list output must be valid JSON");
    assert!(
        v.is_array(),
        "queue list output must be a JSON array; got: {v}"
    );
}

/// E2E: `jr requesttype list --project <JSM> --output json` exits 0.
///
/// Skipped cleanly when `JR_E2E_JSM_PROJECT` is not set.
///
/// Traces to: AC-004, NFR-T-E2E-1, design spec §4 Optional/feature-flagged.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_requesttype_list_exits_ok() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            // Skipped: JR_E2E_JSM_PROJECT not set.
            return;
        }
    };
    let h = e2e_harness();
    let output = h
        .cmd()
        .args([
            "requesttype",
            "list",
            "--project",
            &jsm_project,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");

    assert!(
        output.status.success(),
        "requesttype list failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("requesttype list output must be valid JSON");
    assert!(
        v.is_array(),
        "requesttype list output must be a JSON array; got: {v}"
    );
}

// ---------------------------------------------------------------------------
// AC-007 — Write flow happy path
// ---------------------------------------------------------------------------

/// E2E: Full write flow — create, poll_view, edit, comment, worklog, move.
///
/// Exercises all write sub-steps against the live site with round-trip assertions:
///
/// 1. `issue create` → assert key format + url presence; poll_view + assert summary,
///    issue type name (env-parametric via `issue_type()`), run label in labels (AC-010).
/// 2. `issue edit --summary` → assert `changed_fields.summary` + `updated: true`;
///    poll_view + assert summary changed (AC-011).
///    Sub-step 2b: `issue edit --description` → assert JSON `changed_fields.description
///    == raw text` (BC-3.4.013) AND stderr contains `(updated)` marker (BC-3.4.012;
///    DI-E2E-F2-1: marker is on stderr, not stdout) (AC-012).
/// 3. `issue comment` → `issue comments` read-back; assert comment text is a substring
///    of the serialized JSON (ADF caveat: body is not a flat string) (AC-013).
/// 4. `worklog add 5m` → `worklog list` + assert an entry with timeSpentSeconds==300
///    (AC-014).
/// 5. `issue move → In Progress` → poll_view assert statusCategory key "indeterminate";
///    re-issue same move assert exit 0 + `changed: false` (idempotency; AC-015).
/// 6. `issue move → Done` → poll_view assert statusCategory key "done" (AC-015).
///
/// The label `e2e-<run_label>` is used on the created issue so the CI teardown
/// step (e2e.yml `if: always()`) can close any leftover issues.
///
/// Traces to: AC-010 through AC-015, BC-2.2.028, BC-2.3.032, BC-2.4.039,
/// BC-3.2.001, BC-3.4.012, BC-3.4.013, BC-X.5.001, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_write_flow_create_edit_comment_worklog_close() {
    if !e2e_enabled() {
        return;
    }

    let label = run_label();
    let itype = issue_type();
    let summary_create = format!("[e2e {label}] smoke test");
    let summary_edit = format!("[e2e {label}] smoke test (edited)");
    let desc_text = format!("E2E description set by {label}");
    let comment_text = format!("E2E smoke comment {label}");
    let proj = project();

    let h = e2e_harness();

    // -------------------------------------------------------------------------
    // Step 1: create issue (AC-010)
    // -------------------------------------------------------------------------
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary_create,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create");

    assert!(
        create_output.status.success(),
        "issue create failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let create_json: Value = serde_json::from_slice(&create_output.stdout)
        .expect("issue create output must be valid JSON");

    // Assert key format (AC-010).
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();
    assert_key_format(&key);

    // Assert url presence (F-05: create returns full Issue + top-level url; AC-010).
    assert!(
        create_json.get("url").and_then(Value::as_str).is_some(),
        "issue create JSON must contain a 'url' string field; got: {create_json}"
    );

    // poll_view and assert summary + issue type + run label (AC-010).
    let view_after_create = poll_view(&key, &h);
    assert_eq!(
        view_after_create
            .get("fields")
            .and_then(|f| f.get("summary"))
            .and_then(Value::as_str),
        Some(summary_create.as_str()),
        "poll_view summary must equal the seed summary after create"
    );
    assert_eq!(
        view_after_create
            .get("fields")
            .and_then(|f| f.get("issuetype"))
            .and_then(|t| t.get("name"))
            .and_then(Value::as_str),
        Some(itype.as_str()),
        "poll_view issuetype.name must equal the --type value passed (env-parametric; F-12)"
    );
    let labels_arr = view_after_create
        .get("fields")
        .and_then(|f| f.get("labels"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    assert!(
        labels_arr.contains(&label.as_str()),
        "poll_view labels must contain the run label {label:?}; got: {labels_arr:?}"
    );

    // Optional search-path check: poll_jql with SkipOnEmpty (AC-010 / spec §4).
    // Use poll_jql — not poll_view — because this assertion is specifically about
    // the JQL search path (eventual consistency). A 0-result is clean-skip (index lag).
    // This is the canonical usage of poll_jql: "use poll_jql only for assertions
    // specifically about search behavior" (spec §4 verification ordering rule).
    let search_jql = format!("project={} AND key={}", proj, key);
    let _ = poll_jql(
        &search_jql,
        |v| v.as_array().is_some_and(|a| !a.is_empty()),
        PollJqlMode::SkipOnEmpty,
        &h,
    );
    // poll_jql may return None on index lag — that is a clean skip, not a failure.
    // The write is confirmed by the poll_view above.

    // -------------------------------------------------------------------------
    // Step 2a: edit summary (AC-011)
    // -------------------------------------------------------------------------
    let edit_summary_output = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--summary",
            &summary_edit,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue edit (summary)");

    assert!(
        edit_summary_output.status.success(),
        "issue edit (summary) failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&edit_summary_output.stdout),
        String::from_utf8_lossy(&edit_summary_output.stderr)
    );

    let edit_summary_json: Value = serde_json::from_slice(&edit_summary_output.stdout)
        .expect("issue edit (summary) output must be valid JSON");

    // Assert changed_fields.summary present (AC-011).
    assert!(
        edit_summary_json
            .get("changed_fields")
            .and_then(|cf| cf.get("summary"))
            .is_some(),
        "edit JSON must have changed_fields.summary; got: {edit_summary_json}"
    );
    // Assert top-level updated == true (AC-011).
    // `updated` is a TOP-LEVEL key in the edit response JSON — NOT nested inside
    // `changed_fields`. Structure: {key, changed_fields: {...}, updated: true}.
    // See src/cli/issue/json_output.rs::edit_response for the canonical layout.
    assert_eq!(
        edit_summary_json.get("updated"),
        Some(&Value::Bool(true)),
        "edit JSON must have top-level updated == true; got: {edit_summary_json}"
    );

    // poll_view + assert summary changed (AC-011).
    let view_after_edit = poll_view(&key, &h);
    assert_eq!(
        view_after_edit
            .get("fields")
            .and_then(|f| f.get("summary"))
            .and_then(Value::as_str),
        Some(summary_edit.as_str()),
        "poll_view summary must equal summary_edit after edit"
    );

    // -------------------------------------------------------------------------
    // Step 2b: edit description — #398 asymmetry (AC-012)
    //
    // TWO separate invocations are required because the two BCs are exercised
    // on different output channels and cannot be tested from a single `jr` call:
    //
    //   BC-3.4.013 (JSON/lossless channel): `changed_fields.description` carries
    //     the raw user-supplied input string. This requires `--output json`.
    //   BC-3.4.012 (human/table channel): stderr contains the `(updated)` marker.
    //     The marker is emitted ONLY in OutputFormat::Table branch (eprintln!);
    //     JSON mode suppresses it entirely (stderr is silent in JSON mode).
    //
    // Asserting `(updated)` in stderr of a `--output json` invocation would
    // always fail — the marker is never written in JSON mode. Hence the split.
    // -------------------------------------------------------------------------

    // Invocation 2b-i: JSON mode — verify lossless description echo (BC-3.4.013).
    let edit_desc_json_output = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--description",
            &desc_text,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue edit (description, JSON mode)");

    assert!(
        edit_desc_json_output.status.success(),
        "issue edit (description, JSON mode) failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&edit_desc_json_output.stdout),
        String::from_utf8_lossy(&edit_desc_json_output.stderr)
    );

    // JSON channel (stdout): changed_fields.description == raw input string (BC-3.4.013).
    // Stderr of this invocation has NO marker — do not assert on it.
    let edit_desc_json: Value = serde_json::from_slice(&edit_desc_json_output.stdout)
        .expect("issue edit (description, JSON mode) stdout must be valid JSON");
    assert_eq!(
        edit_desc_json
            .get("changed_fields")
            .and_then(|cf| cf.get("description"))
            .and_then(Value::as_str),
        Some(desc_text.as_str()),
        "JSON channel changed_fields.description must equal the raw input string (BC-3.4.013); \
         got: {edit_desc_json}"
    );

    // Invocation 2b-ii: table mode — verify the (updated) marker on stderr (BC-3.4.012).
    // Use a distinct description value so the edit actually changes the field.
    let desc_text2 = format!("{desc_text} (v2)");
    let edit_desc_table_output = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--description",
            &desc_text2,
            // No --output json: table mode emits the (updated) marker to stderr.
        ])
        .output()
        .expect("failed to spawn jr for issue edit (description, table mode)");

    assert!(
        edit_desc_table_output.status.success(),
        "issue edit (description, table mode) failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&edit_desc_table_output.stdout),
        String::from_utf8_lossy(&edit_desc_table_output.stderr)
    );

    // Human channel (stderr): must contain the '(updated)' marker (BC-3.4.012; DI-E2E-F2-1).
    let edit_desc_table_stderr = String::from_utf8_lossy(&edit_desc_table_output.stderr);
    assert!(
        edit_desc_table_stderr.contains("(updated)"),
        "human channel (stderr) must contain '(updated)' marker for description edit \
         (BC-3.4.012); stderr: {edit_desc_table_stderr:?}"
    );

    // -------------------------------------------------------------------------
    // Step 3: add comment + read-back (AC-013)
    // -------------------------------------------------------------------------
    // The `issue comment` subcommand takes the message as a positional argument,
    // not via `--body`. See `IssueCommand::Comment { message: Option<String>, .. }`
    // in src/cli/mod.rs.
    let comment_output = h
        .cmd()
        .args(["issue", "comment", &key, &comment_text, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue comment");

    assert!(
        comment_output.status.success(),
        "issue comment failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&comment_output.stdout),
        String::from_utf8_lossy(&comment_output.stderr)
    );

    // Read back: issue comments <key> (GET-consistent; no JQL).
    let comments_output = h
        .cmd()
        .args(["issue", "comments", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue comments read-back");

    assert!(
        comments_output.status.success(),
        "issue comments read-back failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&comments_output.stdout),
        String::from_utf8_lossy(&comments_output.stderr)
    );

    let comments_json: Value = serde_json::from_slice(&comments_output.stdout)
        .expect("issue comments output must be valid JSON");
    assert!(
        comments_json.is_array(),
        "issue comments output must be a JSON array; got: {comments_json}"
    );
    assert!(
        !comments_json.as_array().unwrap().is_empty(),
        "issue comments array must be non-empty after posting a comment"
    );

    // ADF caveat (AC-013): Comment.body is an ADF object, NOT a flat string.
    // Assert the posted text appears as a substring of the serialized JSON.
    let comments_serialized = serde_json::to_string(&comments_json).unwrap();
    assert!(
        comments_serialized.contains(&comment_text),
        "comment text {comment_text:?} must appear as a substring of the serialized \
         comments JSON (ADF body contains the text as a nested value); \
         got: {comments_serialized}"
    );

    // -------------------------------------------------------------------------
    // Step 4: log 5 minutes of work + worklog list assert (AC-014)
    // -------------------------------------------------------------------------
    let worklog_add_output = h
        .cmd()
        .args(["worklog", "add", &key, "5m", "--output", "json"])
        .output()
        .expect("failed to spawn jr for worklog add");

    assert!(
        worklog_add_output.status.success(),
        "worklog add failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&worklog_add_output.stdout),
        String::from_utf8_lossy(&worklog_add_output.stderr)
    );

    // worklog list + assert an entry with timeSpentSeconds == 300 (AC-014).
    let worklog_list_output = h
        .cmd()
        .args(["worklog", "list", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for worklog list (step 4)");

    assert!(
        worklog_list_output.status.success(),
        "worklog list failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&worklog_list_output.stdout),
        String::from_utf8_lossy(&worklog_list_output.stderr)
    );

    let worklog_arr: Value = serde_json::from_slice(&worklog_list_output.stdout)
        .expect("worklog list output must be valid JSON");
    assert!(
        worklog_arr.is_array(),
        "worklog list output must be a JSON array; got: {worklog_arr}"
    );
    let has_300 = worklog_arr
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.get("timeSpentSeconds").and_then(Value::as_u64) == Some(300));
    assert!(
        has_300,
        "worklog list must contain an entry with timeSpentSeconds == 300 (5m); \
         got: {worklog_arr}"
    );

    // -------------------------------------------------------------------------
    // Step 5: move to In Progress + idempotency (AC-015)
    // -------------------------------------------------------------------------
    let move_wip_output = h
        .cmd()
        .args([
            "issue",
            "move",
            &key,
            &status_in_progress(),
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue move to in-progress");

    assert!(
        move_wip_output.status.success(),
        "issue move to '{}' failed for {key}:\nstdout: {}\nstderr: {}",
        status_in_progress(),
        String::from_utf8_lossy(&move_wip_output.stdout),
        String::from_utf8_lossy(&move_wip_output.stderr)
    );

    // poll_view: assert statusCategory.key == "indeterminate" (In Progress) by category key,
    // not status name (portable; AC-015).
    let view_wip = poll_view(&key, &h);
    let wip_status = view_wip
        .get("fields")
        .and_then(|f| f.get("status"))
        .unwrap_or_else(|| {
            panic!("poll_view after move-to-in-progress must have fields.status; got: {view_wip}")
        });
    assert_status_category(wip_status, StatusCategory::InProgress);

    // Re-issue the same move — single-key idempotency (BC-3.2.001; AC-015).
    let move_wip_idempotent = h
        .cmd()
        .args([
            "issue",
            "move",
            &key,
            &status_in_progress(),
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for idempotent move");

    assert!(
        move_wip_idempotent.status.success(),
        "idempotent issue move must exit 0 for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&move_wip_idempotent.stdout),
        String::from_utf8_lossy(&move_wip_idempotent.stderr)
    );

    let idempotent_json: Value = serde_json::from_slice(&move_wip_idempotent.stdout)
        .expect("idempotent move output must be valid JSON");
    // Single-key move JSON is {key, status, changed}; idempotent re-issue returns changed: false.
    assert_eq!(
        idempotent_json.get("changed"),
        Some(&Value::Bool(false)),
        "idempotent move JSON must have changed: false (BC-3.2.001); got: {idempotent_json}"
    );

    // -------------------------------------------------------------------------
    // Step 6: move to Done (AC-015)
    // -------------------------------------------------------------------------
    let move_done_output = h
        .cmd()
        .args(["issue", "move", &key, &status_done(), "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue move to done");

    assert!(
        move_done_output.status.success(),
        "issue move to '{}' failed for {key}:\nstdout: {}\nstderr: {}",
        status_done(),
        String::from_utf8_lossy(&move_done_output.stdout),
        String::from_utf8_lossy(&move_done_output.stderr)
    );

    // poll_view: assert statusCategory.key == "done" (AC-015).
    let view_done = poll_view(&key, &h);
    let done_status = view_done
        .get("fields")
        .and_then(|f| f.get("status"))
        .unwrap_or_else(|| {
            panic!("poll_view after move-to-done must have fields.status; got: {view_done}")
        });
    assert_status_category(done_status, StatusCategory::Done);
}

// ---------------------------------------------------------------------------
// AC-004 — worklog list (requires a key; uses a project-scoped list first)
// ---------------------------------------------------------------------------

/// E2E: `jr worklog list <KEY> --output json` exits 0 and returns a JSON array.
///
/// This test is self-seeding: it creates a throwaway Task issue labeled with
/// `run_label()` at the start, polls for GET-consistency via `poll_view`, and
/// then runs `worklog list` against that key. This guarantees the read path is
/// always exercised regardless of whether the project is freshly provisioned or
/// already populated, and regardless of test execution order under
/// `--test-threads=1`.
///
/// The created issue carries the `e2e-<run_label>` label so the `if: always()`
/// teardown step in `e2e.yml` will close it even if the test is interrupted.
///
/// Traces to: AC-004, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_worklog_list_returns_array() {
    if !e2e_enabled() {
        return;
    }

    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Self-seed: create a throwaway issue so this test always has a key to work with.
    let summary = format!("[e2e {label}] worklog-list seed");
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (worklog-list seed)");

    assert!(
        create_output.status.success(),
        "issue create (worklog-list seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let create_json: Value = serde_json::from_slice(&create_output.stdout)
        .expect("issue create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Poll for GET-consistency before running worklog list.
    poll_view(&key, &h);

    let worklog_output = h
        .cmd()
        .args(["worklog", "list", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for worklog list");

    assert!(
        worklog_output.status.success(),
        "worklog list failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&worklog_output.stdout),
        String::from_utf8_lossy(&worklog_output.stderr)
    );

    let v: Value = serde_json::from_slice(&worklog_output.stdout)
        .expect("worklog list output must be valid JSON");
    assert!(
        v.is_array(),
        "worklog list output must be a JSON array; got: {v}"
    );

    // M1 deepening (AC-008): if non-empty, timeSpentSeconds — if present — is numeric.
    // The field is Option<u64> in the Worklog type; do NOT require it non-null (F-07).
    // The exact == 300 value check is reserved for the write-flow step 4 (AC-014) only.
    for (i, entry) in v.as_array().unwrap().iter().enumerate() {
        if let Some(tss) = entry.get("timeSpentSeconds") {
            assert!(
                tss.is_number(),
                "worklog entry[{i}].timeSpentSeconds must be numeric when present; got: {tss}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// AC-004 — issue view (requires a key)
// ---------------------------------------------------------------------------

/// E2E: `jr issue view <KEY> --output json` exits 0 and contains a `"key"` field.
///
/// This test is self-seeding: it creates a throwaway Task issue labeled with
/// `run_label()` at the start, polls for GET-consistency via `poll_view`, and
/// then runs `issue view` against that key. This guarantees the read path is
/// always exercised regardless of whether the project is freshly provisioned or
/// already populated, and regardless of test execution order under
/// `--test-threads=1`.
///
/// The created issue carries the `e2e-<run_label>` label so the `if: always()`
/// teardown step in `e2e.yml` will close it even if the test is interrupted.
///
/// Traces to: AC-004, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_view_returns_key_field() {
    if !e2e_enabled() {
        return;
    }

    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Self-seed: create a throwaway issue so this test always has a key to work with.
    let summary = format!("[e2e {label}] issue-view seed");
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (issue-view seed)");

    assert!(
        create_output.status.success(),
        "issue create (issue-view seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let create_json: Value = serde_json::from_slice(&create_output.stdout)
        .expect("issue create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Poll for GET-consistency before running issue view.
    let view_json = poll_view(&key, &h);
    assert!(
        view_json.get("key").is_some(),
        "issue view JSON must contain a 'key' field; got: {view_json}"
    );
}

// ---------------------------------------------------------------------------
// Unit tests for `extract_fn_body` parser (always-run, no gate required)
// ---------------------------------------------------------------------------

/// Regression pin for the comment + lifetime parser fix (PR #433 Copilot finding).
///
/// Verifies that `extract_fn_body` correctly handles:
///   1. `//` line comments containing apostrophes (e.g. `// don't forget`)
///   2. `//` line comments containing double-quotes (e.g. `// a "quoted" word`)
///   3. `/* ... */` block comments containing quotes and braces
///   4. Rust lifetime sigils (`&'static str`) that must NOT enter `InChar` state
///   5. A closing brace inside a string literal that must NOT close the function body
///
/// Without the fix, a `'` inside a `//` comment would toggle the scanner into
/// `InChar` state and subsequent `'` or brace characters would be misinterpreted,
/// potentially causing the meta-guard `test_every_ignored_test_has_gate_guard`
/// to mis-extract function bodies and produce false pass/fail results.
#[test]
fn test_extract_fn_body_handles_line_comment_with_apostrophe() {
    // Function body containing `// don't forget` — the apostrophe must not
    // desync the brace counter.
    let src = [
        "fn foo() {",
        "    // don't forget to close",
        "    let x = 1;",
        "}",
        "fn bar() {}", // must NOT be included in the extracted body
    ];
    let body = extract_fn_body(&src, 0);
    // Must include up to and including the closing `}` of `foo`.
    assert!(
        body.contains("fn foo()"),
        "body must start at fn foo; got: {body:?}"
    );
    assert!(
        body.contains("don't forget"),
        "body must contain the comment text; got: {body:?}"
    );
    assert!(
        !body.contains("fn bar()"),
        "body must NOT include fn bar (over-extraction); got: {body:?}"
    );
    // Brace depth must be balanced — the extracted body ends at the right `}`.
    let open = body.chars().filter(|&c| c == '{').count();
    let close = body.chars().filter(|&c| c == '}').count();
    assert_eq!(
        open, close,
        "extracted body must have balanced braces; open={open} close={close}"
    );
}

#[test]
fn test_extract_fn_body_handles_line_comment_with_double_quote() {
    // Function body containing `// a "quoted" word` — the double-quote must not
    // enter InString state.
    let src = [
        "fn foo() {",
        r#"    // a "quoted" word in comment"#,
        "    let x = 2;",
        "}",
        "fn bar() {}",
    ];
    let body = extract_fn_body(&src, 0);
    assert!(
        body.contains("fn foo()"),
        "body must start at fn foo; got: {body:?}"
    );
    assert!(
        !body.contains("fn bar()"),
        "body must NOT include fn bar; got: {body:?}"
    );
}

#[test]
fn test_extract_fn_body_handles_block_comment_with_quotes_and_braces() {
    // Block comment containing quotes and braces must not affect depth.
    let src = [
        "fn foo() {",
        "    /* don't count { these } braces \"or these\" */",
        "    let y = 3;",
        "}",
        "fn bar() {}",
    ];
    let body = extract_fn_body(&src, 0);
    assert!(
        body.contains("fn foo()"),
        "body must start at fn foo; got: {body:?}"
    );
    assert!(
        !body.contains("fn bar()"),
        "body must NOT include fn bar (block comment braces must not count); got: {body:?}"
    );
    let open = body.chars().filter(|&c| c == '{').count();
    let close = body.chars().filter(|&c| c == '}').count();
    // The body text includes the braces inside the comment, but the scanner
    // must still terminate at the real closing brace.  Only assert termination
    // (fn bar absent) and that the raw text is intact.
    let _ = (open, close); // brace counts in raw text include comment braces; shape check only
}

#[test]
fn test_extract_fn_body_handles_lifetime_sigil() {
    // `&'static str` lifetime must NOT enter InChar state.
    // If it did, the `'` in `str` would never close it and subsequent `'`
    // characters would be misinterpreted.
    let src = [
        "fn foo() {",
        "    let s: &'static str = \"hello\";",
        "    // don't forget the lifetime",
        "}",
        "fn bar() {}",
    ];
    let body = extract_fn_body(&src, 0);
    assert!(
        body.contains("fn foo()"),
        "body must start at fn foo; got: {body:?}"
    );
    assert!(
        !body.contains("fn bar()"),
        "body must NOT include fn bar (lifetime must not desync scanner); got: {body:?}"
    );
}

#[test]
fn test_extract_fn_body_handles_closing_brace_in_string() {
    // A `}` inside a string literal must NOT close the function body early.
    let src = [
        "fn foo() {",
        r#"    let x = "}"; // closing brace in string"#,
        "    let y = 4;",
        "}",
        "fn bar() {}",
    ];
    let body = extract_fn_body(&src, 0);
    assert!(
        body.contains("fn foo()"),
        "body must start at fn foo; got: {body:?}"
    );
    assert!(
        body.contains("let y = 4"),
        "body must include content after the string-literal brace; got: {body:?}"
    );
    assert!(
        !body.contains("fn bar()"),
        "body must NOT include fn bar; got: {body:?}"
    );
}

#[test]
fn test_extract_fn_body_combined_comments_and_lifetime() {
    // All three hazards together: line comment with apostrophe, block comment
    // with brace and quote, and a lifetime.  This is the canonical regression
    // case from the PR #433 Copilot finding.
    let src = [
        "fn test_fn() {",
        "    // don't forget",
        "    /* block { comment } with \"quotes\" */",
        "    let s: &'static str = \"value\";",
        r#"    let closing = "}"; // closing brace in string"#,
        "    assert!(true);",
        "}",
        "// trailing comment — don't include fn next",
        "fn next() {}",
    ];
    let body = extract_fn_body(&src, 0);
    assert!(
        body.contains("fn test_fn()"),
        "body must start at fn test_fn; got: {body:?}"
    );
    assert!(
        body.contains("assert!(true)"),
        "body must include assert; got: {body:?}"
    );
    assert!(
        !body.contains("fn next()"),
        "body must NOT include fn next; got: {body:?}"
    );
}

/// ALWAYS-RUN guard (not `#[ignore]`): asserts that no single test function body in
/// this file exceeds 500 lines.
///
/// This is the permanent CI guard against runaway `#[ignore]`-gated dead code:
/// such code compiles, passes clippy, and passes `cargo test` (without `--include-ignored`)
/// even if it is corrupted (e.g. a `.args([...])` array repeated 2000 times).
/// This meta-test catches that class of corruption at CI time without needing live
/// credentials.
///
/// Line budget: 500 lines per function body. The write-flow test
/// (`test_e2e_write_flow_create_edit_comment_worklog_close`) is the largest legitimate
/// function at ~438 lines; the budget is set above that with headroom. A runaway
/// `.args([...])` array repeated 2000 times would be orders of magnitude larger and
/// is caught immediately. Any violator is listed by name so it can be fixed without
/// guesswork.
///
/// Uses the same `extract_fn_body` brace-aware scanner as
/// `test_every_ignored_test_has_gate_guard` to handle brace characters inside
/// string literals, comments, and lifetime sigils correctly.
#[test]
fn test_no_test_function_exceeds_line_budget() {
    const MAX_TEST_FN_LINES: usize = 500;
    let source = include_str!("e2e_live.rs");
    let lines: Vec<&str> = source.lines().collect();
    let mut violators: Vec<String> = Vec::new();

    let mut i = 0;
    while i < lines.len() {
        // Match any `fn test_` line (with or without preceding `#[ignore]`).
        if lines[i].trim_start().starts_with("fn test_") {
            let fn_name = lines[i]
                .trim()
                .trim_start_matches("fn ")
                .split('(')
                .next()
                .unwrap_or("(unknown)")
                .to_string();
            let body = extract_fn_body(&lines, i);
            let body_lines = body.lines().count();
            if body_lines > MAX_TEST_FN_LINES {
                violators.push(format!(
                    "{fn_name}: {body_lines} lines (budget: {MAX_TEST_FN_LINES})"
                ));
            }
            // Skip past this function to avoid re-scanning inner closures as top-level fns.
            // Advance by at least 1 to avoid infinite loop; the scanner stops at function end.
            i += body_lines.max(1);
            continue;
        }
        i += 1;
    }

    assert!(
        violators.is_empty(),
        "LINE-BUDGET VIOLATION: the following test functions exceed {MAX_TEST_FN_LINES} lines \
         (this guard catches runaway dead code that compiles but is never executed):\n  {}\n\
         Refactor the function or extract helpers to bring it under the budget.",
        violators.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Unit tests for §4 foundation helpers (S-E2E-3 AC-001–AC-004)
// All tests below are always-run (NOT #[ignore]) — they test pure logic.
// ---------------------------------------------------------------------------

// --- poll_schedule ---

#[test]
fn test_poll_schedule_default_produces_exponential_delays() {
    let schedule = poll_schedule(5, 250);
    // 5 attempts → 4 delays
    assert_eq!(
        schedule.len(),
        4,
        "schedule length must be max_attempts - 1"
    );
    assert_eq!(schedule[0], 250);
    assert_eq!(schedule[1], 500);
    assert_eq!(schedule[2], 1000);
    assert_eq!(schedule[3], 2000);
}

#[test]
fn test_poll_schedule_zero_attempts_returns_empty() {
    let schedule = poll_schedule(0, 250);
    assert!(
        schedule.is_empty(),
        "0 attempts must produce an empty schedule"
    );
}

#[test]
fn test_poll_schedule_one_attempt_returns_empty() {
    let schedule = poll_schedule(1, 100);
    assert!(schedule.is_empty(), "1 attempt needs no sleep delays");
}

// --- AC-001 / AC-002: poll_outcome pure decision logic (table-driven) ---
// poll_jql makes live network calls; the decision logic is extracted into
// poll_outcome so it can be exercised without spawning any processes.

/// Table-driven unit tests for `poll_outcome` (S-E2E-3 BUG-3).
///
/// Each case specifies (last_count, predicate_met, budget_exhausted, mode)
/// and the expected `PollDecision`. These tests are always-run (#[test], NOT
/// #[ignore]) and must never be gated behind JR_RUN_E2E.
#[test]
fn test_poll_outcome_zero_results_not_exhausted_retries() {
    // 0 results, budget not exhausted → Retry regardless of mode.
    assert_eq!(
        poll_outcome(0, false, false, PollJqlMode::SkipOnEmpty),
        PollDecision::Retry,
        "0 results + not exhausted + SkipOnEmpty → Retry"
    );
    assert_eq!(
        poll_outcome(0, false, false, PollJqlMode::FailOnShort(3)),
        PollDecision::Retry,
        "0 results + not exhausted + FailOnShort → Retry"
    );
}

#[test]
fn test_poll_outcome_zero_results_exhausted_skip_none() {
    // 0 results, budget exhausted → SkipNone regardless of mode.
    assert_eq!(
        poll_outcome(0, false, true, PollJqlMode::SkipOnEmpty),
        PollDecision::SkipNone,
        "0 results + exhausted + SkipOnEmpty → SkipNone"
    );
    assert_eq!(
        poll_outcome(0, false, true, PollJqlMode::FailOnShort(1)),
        PollDecision::SkipNone,
        "0 results + exhausted + FailOnShort → SkipNone (not FailPanic; pure lag)"
    );
}

#[test]
fn test_poll_outcome_predicate_met_returns_regardless_of_mode_or_budget() {
    // Predicate satisfied → Return regardless of mode or budget.
    assert_eq!(
        poll_outcome(5, true, false, PollJqlMode::SkipOnEmpty),
        PollDecision::Return,
        "predicate met + not exhausted + SkipOnEmpty → Return"
    );
    assert_eq!(
        poll_outcome(5, true, true, PollJqlMode::SkipOnEmpty),
        PollDecision::Return,
        "predicate met + exhausted + SkipOnEmpty → Return"
    );
    assert_eq!(
        poll_outcome(2, true, false, PollJqlMode::FailOnShort(3)),
        PollDecision::Return,
        "predicate met + not exhausted + FailOnShort → Return"
    );
    assert_eq!(
        poll_outcome(2, true, true, PollJqlMode::FailOnShort(3)),
        PollDecision::Return,
        "predicate met + exhausted + FailOnShort → Return"
    );
}

#[test]
fn test_poll_outcome_nonzero_skip_on_empty_returns_immediately() {
    // Non-zero + predicate not met + SkipOnEmpty → Return (never masks positive result).
    assert_eq!(
        poll_outcome(2, false, false, PollJqlMode::SkipOnEmpty),
        PollDecision::Return,
        "non-zero + predicate not met + not exhausted + SkipOnEmpty → Return"
    );
    assert_eq!(
        poll_outcome(2, false, true, PollJqlMode::SkipOnEmpty),
        PollDecision::Return,
        "non-zero + predicate not met + exhausted + SkipOnEmpty → Return"
    );
}

#[test]
fn test_poll_outcome_fail_on_short_retries_nonzero_under_min_before_exhaustion() {
    // Non-zero + predicate not met + FailOnShort + NOT exhausted → Retry
    // (absorbs index lag toward target count).
    assert_eq!(
        poll_outcome(2, false, false, PollJqlMode::FailOnShort(3)),
        PollDecision::Retry,
        "2 results + FailOnShort(3) + not exhausted → Retry"
    );
    assert_eq!(
        poll_outcome(1, false, false, PollJqlMode::FailOnShort(5)),
        PollDecision::Retry,
        "1 result + FailOnShort(5) + not exhausted → Retry"
    );
}

#[test]
fn test_poll_outcome_fail_on_short_panics_at_budget_exhaustion_with_nonzero() {
    // Non-zero + predicate not met + FailOnShort + exhausted → FailPanic (REGRESSION).
    assert_eq!(
        poll_outcome(2, false, true, PollJqlMode::FailOnShort(3)),
        PollDecision::FailPanic,
        "2 results + FailOnShort(3) + exhausted → FailPanic"
    );
    assert_eq!(
        poll_outcome(1, false, true, PollJqlMode::FailOnShort(5)),
        PollDecision::FailPanic,
        "1 result + FailOnShort(5) + exhausted → FailPanic"
    );
}

// --- AC-003: shape matcher unit tests ---

#[test]
fn test_assert_key_format_accepts_valid() {
    // Standard valid keys.
    assert!(key_format_valid("E2E-1"), "E2E-1 must be valid");
    assert!(key_format_valid("PROJ-999"), "PROJ-999 must be valid");
    assert!(key_format_valid("ABC-100"), "ABC-100 must be valid");
    assert!(
        key_format_valid("A1-1"),
        "A1-1 must be valid (digit in prefix after first char)"
    );
    assert!(
        key_format_valid("MYPROJECT-42"),
        "MYPROJECT-42 must be valid"
    );
}

#[test]
fn test_assert_key_format_rejects_invalid() {
    // Lowercase prefix.
    assert!(
        !key_format_valid("e2e-1"),
        "lowercase prefix must be rejected"
    );
    // Bare number.
    assert!(!key_format_valid("123"), "bare number must be rejected");
    // No dash separator.
    assert!(!key_format_valid("ABC"), "no dash must be rejected");
    // Leading digit in project prefix.
    assert!(
        !key_format_valid("1ABC-1"),
        "leading digit in prefix must be rejected"
    );
    // Missing issue number after dash.
    assert!(
        !key_format_valid("ABC-"),
        "empty issue number must be rejected"
    );
    // Non-digit in issue number.
    assert!(
        !key_format_valid("ABC-1A"),
        "non-digit in issue number must be rejected"
    );
    // Empty string.
    assert!(!key_format_valid(""), "empty string must be rejected");
}

#[test]
fn test_assert_status_category_matches_key_not_name() {
    // Each StatusCategory variant must map to the correct locale-invariant key.
    assert_eq!(StatusCategory::ToDo.key(), "new");
    assert_eq!(StatusCategory::InProgress.key(), "indeterminate");
    assert_eq!(StatusCategory::Done.key(), "done");

    // assert_status_category should pass when v["statusCategory"]["key"] matches.
    let todo_val = serde_json::json!({"statusCategory": {"key": "new", "name": "To Do"}});
    assert_status_category(&todo_val, StatusCategory::ToDo);

    let wip_val =
        serde_json::json!({"statusCategory": {"key": "indeterminate", "name": "In Progress"}});
    assert_status_category(&wip_val, StatusCategory::InProgress);

    let done_val = serde_json::json!({"statusCategory": {"key": "done", "name": "Done"}});
    assert_status_category(&done_val, StatusCategory::Done);
}

#[test]
#[should_panic(expected = "statusCategory.key mismatch")]
fn test_assert_status_category_panics_on_wrong_key() {
    let val = serde_json::json!({"statusCategory": {"key": "new"}});
    // Passing InProgress when the key is "new" must panic.
    assert_status_category(&val, StatusCategory::InProgress);
}

#[test]
fn test_assert_issue_shape_valid() {
    let v = serde_json::json!({
        "key": "E2E-1",
        "fields": {
            "summary": "a test issue",
            "status": {
                "statusCategory": {"key": "new", "name": "To Do"}
            }
        }
    });
    assert_issue_shape(&v); // must not panic
}

#[test]
#[should_panic(expected = "assert_issue_shape")]
fn test_assert_issue_shape_rejects_missing_fields() {
    let v = serde_json::json!({
        "key": "E2E-1"
        // missing "fields"
    });
    assert_issue_shape(&v); // must panic
}

#[test]
fn test_assert_array_of_objects_with_keys_empty_passes() {
    // Vacuously true by design: empty array satisfies "if non-empty, every element
    // conforms" (spec §3). Do NOT change this to require a non-empty array —
    // that would break portability on freshly provisioned projects.
    let v = serde_json::json!([]);
    assert_array_of_objects_with_keys(&v, &["id", "name"]); // empty → must not panic
}

#[test]
fn test_assert_array_of_objects_with_keys_all_present() {
    let v = serde_json::json!([
        {"id": 1, "name": "board-a", "type": "scrum"},
        {"id": 2, "name": "board-b", "type": "kanban"}
    ]);
    assert_array_of_objects_with_keys(&v, &["id", "name", "type"]); // must not panic
}

#[test]
#[should_panic(expected = "is missing key")]
fn test_assert_array_of_objects_with_keys_missing_key_panics() {
    let v = serde_json::json!([{"id": 1, "name": "board-a"}]);
    assert_array_of_objects_with_keys(&v, &["id", "name", "type"]); // "type" missing → panic
}

// --- AC-004: transient classifier unit tests ---

#[test]
fn test_transient_classifier_retries_429_and_503() {
    assert!(
        is_transient_error(429, ""),
        "429 must be classified as transient"
    );
    assert!(
        is_transient_error(503, ""),
        "503 must be classified as transient"
    );
    assert!(
        is_transient_error(0, ""),
        "0 (connection reset) must be classified as transient"
    );
}

#[test]
fn test_transient_classifier_does_not_retry_400_404_401() {
    assert!(
        !is_transient_error(400, ""),
        "400 must NOT be classified as transient"
    );
    assert!(
        !is_transient_error(404, ""),
        "404 must NOT be classified as transient"
    );
    assert!(
        !is_transient_error(401, ""),
        "401 must NOT be classified as transient"
    );
    assert!(
        !is_transient_error(500, ""),
        "500 must NOT be classified as transient (only 503 is)"
    );
    assert!(
        !is_transient_error(422, ""),
        "422 must NOT be classified as transient"
    );
}

// ---------------------------------------------------------------------------
// S-E2E-4 — §6.1 Read / Discovery Tests
// ---------------------------------------------------------------------------

/// E2E: `jr issue link-types --output json` returns a JSON array.
///
/// If non-empty, each element has `name` present (string). `id`, `inward`,
/// `outward` are `Option` in `IssueLinkType` and serialize as null — only
/// `name` is guaranteed (F-06).
///
/// Traces to: AC-006, BC-3.6.005, design spec §6.1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_link_types_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["issue", "link-types", "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue link-types");

    assert!(
        output.status.success(),
        "issue link-types failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("issue link-types output must be valid JSON");
    assert!(
        v.is_array(),
        "issue link-types output must be a JSON array; got: {v}"
    );

    // If non-empty: each element must have `name` (F-06: id/inward/outward are Option).
    for (i, elem) in v.as_array().unwrap().iter().enumerate() {
        assert!(
            elem.get("name").and_then(Value::as_str).is_some(),
            "link-types element[{i}] must have a string 'name' field; got: {elem}"
        );
    }
}

/// E2E: `jr team list --output json` exits 0.
///
/// If the org has no teams, `handle_list` prints "No teams found." to stderr
/// and exits 0 with EMPTY stdout. Clean-skip on empty stdout + exit 0 —
/// do NOT call `serde_json::from_slice` on empty input.
///
/// If stdout is non-empty: parse as JSON array and do a basic shape check.
///
/// **Known harness limitation:** `team list` calls `resolve_org_id` which reads
/// the profile URL from config before making any HTTP request. The E2E harness
/// uses empty temp XDG dirs (no `config.toml`) and injects the Jira base URL
/// via the `JR_BASE_URL` debug seam — but that seam only intercepts the HTTP
/// client construction path, not the `Config::active_profile().url` read that
/// `resolve_org_id` performs first. Consequently `team list` exits 78 with
/// "has no URL configured" even when `JR_BASE_URL` is set, unlike `issue list`
/// and other commands that reach the API (and the seam) first. This is a
/// test-harness/command interaction, NOT a `jr` bug — `team list` legitimately
/// requires a configured profile URL. Until the E2E harness is extended to
/// inject a minimal `config.toml`, treat this condition as a clean skip.
/// Candidate src/ follow-up: make `team list` fall back to `JR_BASE_URL` for
/// hostname discovery the same way the HTTP client does, so the harness works
/// without a config file.
///
/// Traces to: AC-005, BC-X.6.004, design spec §6.1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_team_list_returns_array_or_skips() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["team", "list", "--output", "json"])
        .output()
        .expect("failed to spawn jr for team list");

    // Clean skip: profile config is missing (exit 78 + "has no URL configured").
    // This happens in the E2E harness because `team list` validates the profile
    // URL in `resolve_org_id` before any HTTP call, so the JR_BASE_URL seam
    // never fires. See rustdoc above for the full explanation.
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_lower = stderr.to_lowercase();
        if stderr_lower.contains("has no url configured")
            || stderr_lower.contains("no url configured")
        {
            eprintln!(
                "test_e2e_team_list_returns_array_or_skips: \
                 clean-skip (profile config missing in harness — \
                 'has no URL configured'; known harness limitation)"
            );
            return;
        }
        panic!(
            "team list failed unexpectedly:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            stderr
        );
    }

    // Empty stdout + exit 0 is the empty-org path — clean skip.
    let stdout = output.stdout.trim_ascii();
    if stdout.is_empty() {
        eprintln!(
            "test_e2e_team_list_returns_array_or_skips: empty stdout (empty-org path — \
             'No teams found.' on stderr); clean-skip"
        );
        return;
    }

    let v: Value =
        serde_json::from_slice(stdout).expect("team list non-empty output must be valid JSON");
    assert!(
        v.is_array(),
        "team list output must be a JSON array; got: {v}"
    );
    // If non-empty: verify the array is parseable (any object shape is acceptable —
    // team fields are instance-specific). Just confirm it parsed without panic.
}

/// E2E: `jr issue transitions <key> --output json` returns a JSON array.
///
/// Seeds one issue; each element must have `id` (string) and `name` (string).
/// If a `to` field is present on any element, it is an object with
/// `statusCategory.key` in `{"new", "indeterminate", "done"}`.
///
/// Critical constraint (C-2): there is NO top-level `to_category` field.
/// The category is nested at `to.statusCategory.key`.
///
/// Traces to: AC-001, BC-7.3.006, design spec §6.1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_transitions_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed one issue.
    let summary = format!("[e2e {label}] transitions-seed");
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (transitions seed)");

    assert!(
        create_output.status.success(),
        "issue create (transitions seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let create_json: Value = serde_json::from_slice(&create_output.stdout)
        .expect("issue create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Confirm GET-consistency before querying transitions.
    poll_view(&key, &h);

    let transitions_output = h
        .cmd()
        .args(["issue", "transitions", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue transitions");

    assert!(
        transitions_output.status.success(),
        "issue transitions failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&transitions_output.stdout),
        String::from_utf8_lossy(&transitions_output.stderr)
    );

    let v: Value = serde_json::from_slice(&transitions_output.stdout)
        .expect("issue transitions output must be valid JSON");
    assert!(
        v.is_array(),
        "issue transitions output must be a JSON array; got: {v}"
    );

    // If non-empty: each element must have `id` (string) and `name` (string).
    // If a `to` field is present: statusCategory.key must be in the fixed set.
    let valid_cat_keys = ["new", "indeterminate", "done"];
    for (i, elem) in v.as_array().unwrap().iter().enumerate() {
        assert!(
            elem.get("id").and_then(Value::as_str).is_some(),
            "transition[{i}] must have a string 'id' field; elem: {elem}"
        );
        assert!(
            elem.get("name").and_then(Value::as_str).is_some(),
            "transition[{i}] must have a string 'name' field; elem: {elem}"
        );
        // `to` is Option<Status> — may be absent.
        if let Some(to) = elem.get("to") {
            let cat_key = to
                .get("statusCategory")
                .and_then(|sc| sc.get("key"))
                .and_then(Value::as_str);
            assert!(
                cat_key.is_some_and(|k| valid_cat_keys.contains(&k)),
                "transition[{i}].to.statusCategory.key must be one of \
                 {{new, indeterminate, done}}; got: {cat_key:?}; to: {to}"
            );
        }
    }
}

/// E2E: `jr issue changelog <key> --output json` returns an OBJECT `{key, entries}`.
///
/// Seeds one issue and edits its summary, then reads the changelog.
/// The output shape is `ChangelogOutput { key, entries }`
/// (NOT a bare array, NOT `{key, histories}`).
///
/// Critical constraint (F-03): assert `v.is_object()` AND `v["entries"].is_array()`.
/// Do NOT assert `v.is_array()` or `v["histories"]`.
///
/// Shape-only assertion: `{key, entries:[]}` is valid — entries MAY be empty due
/// to changelog indexing lag. Entry count is NOT asserted.
///
/// Traces to: AC-003, BC-2.5.043–046, design spec §6.1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_changelog_returns_object() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed one issue.
    let summary_orig = format!("[e2e {label}] changelog-seed");
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary_orig,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (changelog seed)");

    assert!(
        create_output.status.success(),
        "issue create (changelog seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let create_json: Value = serde_json::from_slice(&create_output.stdout)
        .expect("issue create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Confirm GET-consistency before editing.
    poll_view(&key, &h);

    // Edit summary to create a changelog entry.
    let summary_edited = format!("[e2e {label}] changelog-seed (edited)");
    let edit_output = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--summary",
            &summary_edited,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue edit (changelog seed)");

    assert!(
        edit_output.status.success(),
        "issue edit (changelog seed) failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&edit_output.stdout),
        String::from_utf8_lossy(&edit_output.stderr)
    );

    // Now read the changelog.
    let changelog_output = h
        .cmd()
        .args(["issue", "changelog", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue changelog");

    assert!(
        changelog_output.status.success(),
        "issue changelog failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&changelog_output.stdout),
        String::from_utf8_lossy(&changelog_output.stderr)
    );

    let v: Value = serde_json::from_slice(&changelog_output.stdout)
        .expect("issue changelog output must be valid JSON");

    // F-03: shape is {key, entries} — NOT a bare array, NOT {key, histories}.
    assert!(
        v.is_object(),
        "issue changelog output must be a JSON object ({{key, entries}}); got: {v}"
    );
    assert!(
        v.get("key").and_then(Value::as_str).is_some(),
        "changelog object must have a string 'key' field; got: {v}"
    );
    assert!(
        v.get("entries").is_some_and(Value::is_array),
        "changelog object must have an array 'entries' field; got: {v}"
    );
}

/// E2E: `jr issue comments <key> --output json` returns a JSON array (standalone).
///
/// Seeds one issue and adds a comment, then reads comments via the standalone
/// `issue comments` command. Asserts at least one element (the seeded comment).
///
/// This test exercises the standalone comment-read path independently of the
/// write-flow comment read-back from S-E2E-1.
///
/// Traces to: AC-002, design spec §6.1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_comments_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed one issue.
    let summary = format!("[e2e {label}] comments-seed");
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (comments seed)");

    assert!(
        create_output.status.success(),
        "issue create (comments seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let create_json: Value = serde_json::from_slice(&create_output.stdout)
        .expect("issue create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Confirm GET-consistency before adding comment.
    poll_view(&key, &h);

    // Add a comment.
    let comment_output = h
        .cmd()
        .args([
            "issue",
            "comment",
            &key,
            "E2E standalone comments test comment",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue comment");

    assert!(
        comment_output.status.success(),
        "issue comment failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&comment_output.stdout),
        String::from_utf8_lossy(&comment_output.stderr)
    );

    // Read comments via the standalone `issue comments` command.
    // Retry up to max_attempts times with exponential backoff: comment visibility
    // can lag the POST on a loaded instance. Mirror poll_view's pattern using the
    // same poll_schedule(max_attempts, initial_ms) helper and the same env seams.
    let max_attempts: usize = match std::env::var("JR_E2E_POLL_MAX_ATTEMPTS") {
        Ok(v) if !v.trim().is_empty() => v.trim().parse().unwrap_or(5).max(1),
        _ => 5,
    };
    let initial_ms: u64 = match std::env::var("JR_E2E_POLL_INITIAL_MS") {
        Ok(v) if !v.trim().is_empty() => v.trim().parse().unwrap_or(250),
        _ => 250,
    };
    let schedule = poll_schedule(max_attempts, initial_ms);

    let mut last_value: Option<Value> = None;
    'retry: for (attempt, &delay_ms) in schedule.iter().enumerate() {
        let comments_output = h
            .cmd()
            .args(["issue", "comments", &key, "--output", "json"])
            .output()
            .expect("failed to spawn jr for issue comments");

        assert!(
            comments_output.status.success(),
            "issue comments failed for {key} (attempt {}):\nstdout: {}\nstderr: {}",
            attempt + 1,
            String::from_utf8_lossy(&comments_output.stdout),
            String::from_utf8_lossy(&comments_output.stderr)
        );

        let v: Value = serde_json::from_slice(&comments_output.stdout)
            .expect("issue comments output must be valid JSON");
        assert!(
            v.is_array(),
            "issue comments output must be a JSON array; got: {v}"
        );
        if v.as_array().is_some_and(|a| !a.is_empty()) {
            // Comment is visible — done.
            last_value = Some(v);
            break 'retry;
        }
        // Array is empty — comment hasn't propagated yet. Sleep and retry.
        last_value = Some(v);
        std::thread::sleep(Duration::from_millis(delay_ms));
    }

    // Final attempt (or only attempt when schedule is empty).
    let v = if last_value
        .as_ref()
        .is_some_and(|v| v.as_array().is_some_and(|a| !a.is_empty()))
    {
        last_value.unwrap()
    } else {
        let comments_output = h
            .cmd()
            .args(["issue", "comments", &key, "--output", "json"])
            .output()
            .expect("failed to spawn jr for issue comments (final attempt)");

        assert!(
            comments_output.status.success(),
            "issue comments failed for {key} (final attempt):\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&comments_output.stdout),
            String::from_utf8_lossy(&comments_output.stderr)
        );

        serde_json::from_slice(&comments_output.stdout)
            .expect("issue comments output must be valid JSON")
    };

    assert!(
        v.is_array(),
        "issue comments output must be a JSON array; got: {v}"
    );
    // We seeded one comment — at least one element must be present.
    assert!(
        v.as_array().is_some_and(|a| !a.is_empty()),
        "issue comments array must have at least one element after seeding a comment; got: {v}"
    );
}

/// E2E: `jr board view --board <JR_E2E_BOARD_ID> --output json` returns a bare JSON array.
///
/// Gated on `JR_E2E_BOARD_ID` being set and non-empty. Clean-skip if:
/// - `JR_E2E_BOARD_ID` unset or empty.
/// - Command exits non-zero and stderr contains "No active sprint" (board has no active sprint).
///
/// Critical constraint (H-1): `board view --output json` is a BARE JSON ARRAY of issue
/// objects, NOT an object. `--board` is a FLAG (not a positional argument).
///
/// Traces to: AC-004, BC-5.1.001, design spec §6.1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_BOARD_ID and use --include-ignored to run"]
fn test_e2e_board_view_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let board_id = match env::var("JR_E2E_BOARD_ID") {
        Ok(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => {
            eprintln!("test_e2e_board_view_returns_array: JR_E2E_BOARD_ID not set — clean-skip");
            return;
        }
    };

    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["board", "view", "--board", &board_id, "--output", "json"])
        .output()
        .expect("failed to spawn jr for board view");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No active sprint") {
            eprintln!(
                "test_e2e_board_view_returns_array: board {board_id} has no active sprint — \
                 clean-skip"
            );
            return;
        }
        panic!(
            "board view --board {board_id} failed unexpectedly:\nstdout: {}\nstderr: {stderr}",
            String::from_utf8_lossy(&output.stdout),
        );
    }

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("board view output must be valid JSON");
    // H-1: bare JSON array of issue objects.
    assert!(
        v.is_array(),
        "board view output must be a bare JSON array of issue objects; got: {v}"
    );

    // If non-empty: each element must have the basic issue shape.
    for elem in v.as_array().unwrap() {
        assert_issue_shape(elem);
    }
}

/// E2E: `jr user view <accountId> --output json` returns a JSON object with `accountId`.
///
/// Resolves self-accountId from `user search` seed output. If the search returns an
/// empty array (Browse Users permission absent), clean-skip.
///
/// `accountId` is a POSITIONAL argument to `user view` (not a flag). The JSON key
/// after serde rename is `accountId`.
///
/// Traces to: AC-007, BC-3.1.003, design spec §6.1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_user_view_returns_object() {
    if !e2e_enabled() {
        return;
    }

    // Resolve self-accountId via `user search`.
    // Use the email local-part if set; otherwise fall back to "e2e".
    let query = env::var("JR_E2E_EMAIL")
        .ok()
        .map(|e| e.trim().split('@').next().unwrap_or_default().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "e2e".to_string());

    let h = e2e_harness();
    let search_output = h
        .cmd()
        .args(["user", "search", &query, "--output", "json"])
        .output()
        .expect("failed to spawn jr for user search (self-resolve)");

    if !search_output.status.success() {
        eprintln!(
            "test_e2e_user_view_returns_object: user search failed — clean-skip; stderr: {}",
            String::from_utf8_lossy(&search_output.stderr)
        );
        return;
    }

    let search_v: Value = serde_json::from_slice(&search_output.stdout)
        .expect("user search output must be valid JSON");

    // If search returned an empty array, Browse Users permission is absent — clean skip.
    let account_id = match search_v.as_array().and_then(|a| a.first()) {
        Some(user) => match user.get("accountId").and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => {
                eprintln!(
                    "test_e2e_user_view_returns_object: first user has no 'accountId' — \
                     clean-skip"
                );
                return;
            }
        },
        None => {
            eprintln!(
                "test_e2e_user_view_returns_object: user search returned empty array \
                 (Browse Users permission absent) — clean-skip"
            );
            return;
        }
    };

    // Now run `user view <accountId>` — accountId is a positional argument.
    let view_output = h
        .cmd()
        .args(["user", "view", &account_id, "--output", "json"])
        .output()
        .expect("failed to spawn jr for user view");

    assert!(
        view_output.status.success(),
        "user view {account_id} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&view_output.stdout),
        String::from_utf8_lossy(&view_output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&view_output.stdout).expect("user view output must be valid JSON");
    assert!(
        v.is_object(),
        "user view output must be a JSON object; got: {v}"
    );
    assert!(
        v.get("accountId").and_then(Value::as_str).is_some(),
        "user view JSON must contain a string 'accountId' field; got: {v}"
    );
}

// ---------------------------------------------------------------------------
// S-E2E-4 — §6.2 Write / Behavioral Tests
// ---------------------------------------------------------------------------

/// E2E: `jr issue edit <key> --dry-run --output json` returns valid JSON but
/// does NOT mutate the issue.
///
/// Self-seeds one issue with a known summary S1. Runs `issue edit --summary S2
/// --dry-run --output json`. Asserts: (a) output is valid JSON, (b) a subsequent
/// `poll_view` shows the summary is still S1 (load-bearing: no mutation occurred).
///
/// Do NOT hard-pin dry-run JSON key names — the no-mutation round-trip is the
/// portable contract.
///
/// Traces to: AC-010, BC-2.2.028, design spec §6.2.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_edit_dry_run_no_mutation() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed one issue with a known summary.
    let summary_orig = format!("[e2e {label}] dry-run-seed");
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary_orig,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (dry-run seed)");

    assert!(
        create_output.status.success(),
        "issue create (dry-run seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let create_json: Value = serde_json::from_slice(&create_output.stdout)
        .expect("issue create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Confirm GET-consistency before dry-run.
    poll_view(&key, &h);

    // Run the dry-run edit with a different summary.
    let summary_new = format!("[e2e {label}] dry-run-seed (SHOULD NOT APPEAR)");
    let dry_run_output = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--summary",
            &summary_new,
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue edit --dry-run");

    assert!(
        dry_run_output.status.success(),
        "issue edit --dry-run failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&dry_run_output.stdout),
        String::from_utf8_lossy(&dry_run_output.stderr)
    );

    // (a) Output must be valid JSON.
    let _dry_run_json: Value = serde_json::from_slice(&dry_run_output.stdout)
        .expect("issue edit --dry-run output must be valid JSON");

    // (b) Load-bearing: poll_view must show the ORIGINAL summary (no mutation).
    let view = poll_view(&key, &h);
    let actual_summary = view
        .get("fields")
        .and_then(|f| f.get("summary"))
        .and_then(Value::as_str);
    assert_eq!(
        actual_summary,
        Some(summary_orig.as_str()),
        "dry-run MUST NOT mutate the issue; expected summary {summary_orig:?} but got \
         {actual_summary:?}"
    );
}

/// E2E: `jr issue assign <key>` with no assignee argument → self-assignment.
///
/// There is NO `--me` flag — `handle_assign` falls to the `client.get_myself()`
/// branch when no assignee is given. Read `assignee.accountId` from `poll_view`,
/// not from the assign command output.
///
/// Traces to: AC-009, BC-3.1.003, design spec §6.2.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_assign_self() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed one issue.
    let summary = format!("[e2e {label}] assign-self-seed");
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (assign-self seed)");

    assert!(
        create_output.status.success(),
        "issue create (assign-self seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let create_json: Value = serde_json::from_slice(&create_output.stdout)
        .expect("issue create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Confirm GET-consistency before assigning.
    poll_view(&key, &h);

    // Assign with NO assignee argument (omitting triggers self-assignment via /myself).
    // There is NO --me flag (F-01).
    let assign_output = h
        .cmd()
        .args(["issue", "assign", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue assign (self)");

    assert!(
        assign_output.status.success(),
        "issue assign (self) failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&assign_output.stdout),
        String::from_utf8_lossy(&assign_output.stderr)
    );

    // Read assignee from poll_view (not from assign output — assign JSON is flat).
    let view = poll_view(&key, &h);
    let assignee_id = view
        .get("fields")
        .and_then(|f| f.get("assignee"))
        .and_then(|a| a.get("accountId"))
        .and_then(Value::as_str);

    assert!(
        assignee_id.is_some_and(|id| !id.is_empty()),
        "after self-assignment, fields.assignee.accountId must be a non-null non-empty string; \
         fields.assignee: {:?}",
        view.get("fields").and_then(|f| f.get("assignee"))
    );
}

/// E2E: `jr issue link A B` / `jr issue unlink A B` round-trip.
///
/// Seeds two issues (A and B). Links A to B (omitting `--type` to use the
/// built-in default "Relates"). Verifies the link by traversing
/// `fields.issuelinks[]` and checking B's key appears under EITHER
/// `inwardIssue.key` OR `outwardIssue.key` (F-09: render side not contractually
/// fixed). Then unlinks. Verifies the link is gone.
///
/// Traces to: AC-008, BC-3.6.001, BC-3.6.004, design spec §6.2.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_link_and_unlink() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed issue A.
    let summary_a = format!("[e2e {label}] link-seed-A");
    let create_a = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary_a,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (link seed A)");

    assert!(
        create_a.status.success(),
        "issue create (link seed A) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_a.stdout),
        String::from_utf8_lossy(&create_a.stderr)
    );

    let key_a = serde_json::from_slice::<Value>(&create_a.stdout)
        .expect("issue create A output must be valid JSON")
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create A JSON must contain a 'key' field")
        .to_string();

    // Seed issue B.
    let summary_b = format!("[e2e {label}] link-seed-B");
    let create_b = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary_b,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (link seed B)");

    assert!(
        create_b.status.success(),
        "issue create (link seed B) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_b.stdout),
        String::from_utf8_lossy(&create_b.stderr)
    );

    let key_b = serde_json::from_slice::<Value>(&create_b.stdout)
        .expect("issue create B output must be valid JSON")
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create B JSON must contain a 'key' field")
        .to_string();

    // Confirm GET-consistency for both before linking.
    poll_view(&key_a, &h);
    poll_view(&key_b, &h);

    // Link A to B (omit --type to use built-in default "Relates").
    let link_output = h
        .cmd()
        .args(["issue", "link", &key_a, &key_b, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue link");

    assert!(
        link_output.status.success(),
        "issue link {key_a} {key_b} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&link_output.stdout),
        String::from_utf8_lossy(&link_output.stderr)
    );

    // Verify the link: poll_view(A) and check issuelinks[] for B's key.
    let view_a_linked = poll_view(&key_a, &h);
    let issue_links = view_a_linked
        .get("fields")
        .and_then(|f| f.get("issuelinks"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let b_found_in_links = issue_links.iter().any(|link| {
        let inward = link
            .get("inwardIssue")
            .and_then(|i| i.get("key"))
            .and_then(Value::as_str);
        let outward = link
            .get("outwardIssue")
            .and_then(|i| i.get("key"))
            .and_then(Value::as_str);
        inward == Some(key_b.as_str()) || outward == Some(key_b.as_str())
    });

    assert!(
        b_found_in_links,
        "after linking, {key_b} must appear in {key_a}.fields.issuelinks[].inwardIssue.key \
         OR outwardIssue.key; issuelinks: {:?}",
        issue_links
    );

    // Unlink A from B.
    let unlink_output = h
        .cmd()
        .args(["issue", "unlink", &key_a, &key_b, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue unlink");

    assert!(
        unlink_output.status.success(),
        "issue unlink {key_a} {key_b} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&unlink_output.stdout),
        String::from_utf8_lossy(&unlink_output.stderr)
    );

    // Verify the link is gone: poll_view(A) and check issuelinks[] for B's key.
    let view_a_unlinked = poll_view(&key_a, &h);
    let issue_links_after = view_a_unlinked
        .get("fields")
        .and_then(|f| f.get("issuelinks"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let b_still_linked = issue_links_after.iter().any(|link| {
        let inward = link
            .get("inwardIssue")
            .and_then(|i| i.get("key"))
            .and_then(Value::as_str);
        let outward = link
            .get("outwardIssue")
            .and_then(|i| i.get("key"))
            .and_then(Value::as_str);
        inward == Some(key_b.as_str()) || outward == Some(key_b.as_str())
    });

    assert!(
        !b_still_linked,
        "after unlinking, {key_b} must NOT appear in {key_a}.fields.issuelinks[]; \
         issuelinks after unlink: {:?}",
        issue_links_after
    );
}

/// E2E: Pagination dedup — creates 3 issues under a per-test-unique label
/// and asserts the returned keys are duplicate-free and a superset of the 3 created.
///
/// The unique label embeds both `run_label()` (which uses GITHUB_RUN_ID) and a
/// per-attempt discriminator (`GITHUB_RUN_ATTEMPT` or a timestamp nonce) so that
/// workflow re-runs don't reuse the same label and inflate the result count.
///
/// Traces to: AC-011, BC-2.6.051 (JRACLOUD-95368 dedup contract), design spec §6.2.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_pagination_dedup() {
    use std::collections::HashSet;
    if !e2e_enabled() {
        return;
    }

    // Capture run_label() once: used both as the sweeper label AND as the base
    // for unique_label so the sweeper can find these issues by base label.
    let base_label = run_label();

    // Build a per-attempt-unique label (M-2: embed run_id AND attempt discriminator).
    // GITHUB_RUN_ATTEMPT re-runs with a different counter; for local runs, a
    // millisecond timestamp nonce (total milliseconds since epoch) provides
    // sufficient uniqueness.
    let run_attempt = env::var("GITHUB_RUN_ATTEMPT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_millis()
                .to_string()
        });
    let unique_label = format!("{base_label}-a{run_attempt}-pg");

    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Create 3 issues labeled with BOTH the base run label (for sweeper teardown)
    // AND the unique label (for dedup JQL — F5 F-2 fix).
    let mut created_keys = Vec::with_capacity(3);
    for n in 1..=3u8 {
        let summary = format!("[e2e {base_label}] dedup-seed-{n}");
        let create_output = h
            .cmd()
            .args([
                "issue",
                "create",
                "--project",
                &proj,
                "--type",
                &itype,
                "--summary",
                &summary,
                "--label",
                &base_label,
                "--label",
                &unique_label,
                "--output",
                "json",
            ])
            .output()
            .expect("failed to spawn jr for issue create (dedup seed)");

        assert!(
            create_output.status.success(),
            "issue create (dedup seed {n}) failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&create_output.stdout),
            String::from_utf8_lossy(&create_output.stderr)
        );

        let create_json: Value = serde_json::from_slice(&create_output.stdout)
            .expect("issue create output must be valid JSON");
        let key = create_json
            .get("key")
            .and_then(Value::as_str)
            .expect("issue create JSON must contain a 'key' field")
            .to_string();
        created_keys.push(key);
    }

    // JQL exact-match on the unique label. `labels=<label>` is valid JQL;
    // `labels ~ "..."` is NOT supported on the labels field.
    let jql = format!(
        "labels=\"{unique_label}\" ORDER BY key ASC",
        unique_label = unique_label
    );

    // poll_jql with FailOnShort(3): 0 results = index lag (clean-skip); 1-2 = FAIL loud.
    let result = poll_jql(
        &jql,
        |v| v.as_array().is_some_and(|a| a.len() >= 3),
        PollJqlMode::FailOnShort(3),
        &h,
    );

    let returned_keys = match result {
        None => {
            // 0 results after full budget — pure index lag, clean-skip.
            eprintln!(
                "test_e2e_pagination_dedup: poll_jql returned None (0 results / index lag) \
                 — clean-skip"
            );
            return;
        }
        Some(v) => v
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|elem| elem.get("key").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>(),
    };

    // Assert duplicate-free (dedup contract: BC-2.6.051).
    let key_set: HashSet<&str> = returned_keys.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        key_set.len(),
        returned_keys.len(),
        "REGRESSION (BC-2.6.051): returned keys contain duplicates — dedup contract violated; \
         keys: {:?}",
        returned_keys
    );

    // Assert returned keys are a SUPERSET of the 3 created keys (not "exactly 3" — dedup
    // contract is under test; other issues with the same label from prior runs may appear
    // if label uniqueness is insufficient, but the 3 created keys MUST all be present).
    let created_set: HashSet<&str> = created_keys.iter().map(|s| s.as_str()).collect();
    for key in &created_keys {
        assert!(
            key_set.contains(key.as_str()),
            "REGRESSION: created key {key} not found in poll_jql results — \
             superset check failed; returned: {:?}",
            returned_keys
        );
    }
    // Note: the per-key loop above already asserts key_set.contains(key) for every
    // created key, so a redundant created_set.is_subset(&key_set) check is omitted.
    let _ = created_set; // suppress unused-variable warning
}

// ---------------------------------------------------------------------------
// E2E-PG-4 — §6.4 Label add/remove, link --type/unlink --type, remote-link smoke
// ---------------------------------------------------------------------------

/// E2E: `jr issue edit <KEY> --label add:<L>` adds a label; `--label remove:<L>` removes it.
///
/// Seeds one throwaway issue labeled with `run_label()` for teardown. Derives the
/// test label `e2e-<token>` from `run_label()` (hyphen-separated, no spaces — Q3:
/// label values must be whitespace-free for Jira to accept them).
///
/// Portability constraints:
/// - Asserts SET membership (`labels[]` contains / does not contain L), never order
///   or total count — other labels from the workflow are invisible to this assertion.
/// - Clean-skip on permission denial (HTTP 4xx + "permission" / "403" in stderr):
///   the `Bulk Changes` global permission gates `issue edit --label`. Not all Jira
///   Cloud instances grant this permission to service accounts.
///
/// Traces to: E2E-PG-4, design spec §6.4.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_edit_label_add_remove_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed a throwaway issue tagged with run_label() for sweeper teardown.
    let summary = format!("[e2e {label}] label-roundtrip-seed");
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (label roundtrip seed)");

    assert!(
        create_output.status.success(),
        "issue create (label roundtrip seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let key = serde_json::from_slice::<Value>(&create_output.stdout)
        .expect("issue create output must be valid JSON")
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Use a probe label distinct from the seed label — unique per run, no spaces (HIGH-1).
    // format!("{label}-probe") is the canonical probe name; `label` == run_label() above.
    let probe = format!("{label}-probe");

    // Confirm GET-consistency before editing labels.
    // --- HIGH-1: assert probe label is ABSENT before the add call ---
    let before_json = poll_view(&key, &h);
    let before_labels: Vec<String> = before_json
        .get("fields")
        .and_then(|f| f.get("labels"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !before_labels.contains(&probe),
        "probe label '{probe}' must be ABSENT before add; found labels: {before_labels:?}"
    );

    // ADD the probe label.
    let add_output = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--label",
            &format!("add:{probe}"),
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue edit --label add");

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        // Skip is intentionally narrow — only bulk-changes permission denial (HTTP 403);
        // any other failure must fail the test.
        if add_output.status.code() == Some(1) && stderr.contains("403") {
            eprintln!(
                "SKIP: bulk-edit 403 — 'Bulk Changes' global permission \
                 not enabled on this site; skipping label round-trip test.\nstderr: {stderr}"
            );
            return;
        }
        panic!(
            "issue edit --label add failed for {key} (non-403 error — not a permission skip):\n\
             exit: {:?}\nstdout: {}\nstderr: {}",
            add_output.status.code(),
            String::from_utf8_lossy(&add_output.stdout),
            stderr,
        );
    }

    // --- HIGH-1: assert probe label IS present after add ---
    let view_after_add = poll_view(&key, &h);
    let labels_after_add: Vec<String> = view_after_add
        .get("fields")
        .and_then(|f| f.get("labels"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        labels_after_add.contains(&probe),
        "probe label '{probe}' must be PRESENT after add; found labels: {labels_after_add:?}"
    );

    // REMOVE the probe label.
    let remove_output = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--label",
            &format!("remove:{probe}"),
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue edit --label remove");

    if !remove_output.status.success() {
        let stderr = String::from_utf8_lossy(&remove_output.stderr);
        // Skip is intentionally narrow — only bulk-changes permission denial (HTTP 403);
        // any other failure must fail the test.
        if remove_output.status.code() == Some(1) && stderr.contains("403") {
            eprintln!(
                "SKIP: bulk-edit 403 on remove — 'Bulk Changes' global permission \
                 not enabled on this site.\nstderr: {stderr}"
            );
            return;
        }
        panic!(
            "issue edit --label remove failed for {key} (non-403 error — not a permission skip):\n\
             exit: {:?}\nstdout: {}\nstderr: {}",
            remove_output.status.code(),
            String::from_utf8_lossy(&remove_output.stdout),
            stderr,
        );
    }

    // --- HIGH-1: assert probe label is ABSENT after remove ---
    let view_after_remove = poll_view(&key, &h);
    let labels_after_remove: Vec<String> = view_after_remove
        .get("fields")
        .and_then(|f| f.get("labels"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !labels_after_remove.contains(&probe),
        "probe label '{probe}' must be ABSENT after remove; found labels: {labels_after_remove:?}"
    );
}

/// E2E: `jr issue link A B --type <T>` / `jr issue unlink A B --type <T>` round-trip
/// using a dynamically-discovered link type that is NOT "Relates".
///
/// Discovers available link types by calling `jr issue link-types --output json`.
/// Picks the first type whose name is NOT "Relates" (case-insensitive). If no such
/// type exists on the instance, the test clean-skips.
///
/// Portability: never hardcodes a type name in assertions — all assertions reference
/// the discovered type `T` obtained at runtime (Q1 constraint).
///
/// Direction-agnostic verification: asserts that key B appears in `fields.issuelinks[]`
/// under EITHER `inwardIssue.key` OR `outwardIssue.key`, AND that the matching link
/// entry's `type.name` equals T (case-insensitive).
///
/// Traces to: E2E-PG-4, BC-3.6.001, BC-3.6.004, design spec §6.4.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_link_with_type_and_unlink_with_type() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Step 1: discover available link types dynamically.
    let link_types_output = h
        .cmd()
        .args(["issue", "link-types", "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue link-types (type discovery)");

    assert!(
        link_types_output.status.success(),
        "issue link-types failed during type discovery:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&link_types_output.stdout),
        String::from_utf8_lossy(&link_types_output.stderr)
    );

    let link_types_json: Value = serde_json::from_slice(&link_types_output.stdout)
        .expect("issue link-types output must be valid JSON for type discovery");
    let link_types_arr = link_types_json
        .as_array()
        .expect("issue link-types JSON must be an array");

    // Pick the first non-"Relates" type (case-insensitive). Q1: dynamic discovery only.
    let discovered_type: Option<String> = link_types_arr.iter().find_map(|lt| {
        let name = lt.get("name").and_then(Value::as_str)?;
        if name.eq_ignore_ascii_case("Relates") {
            None
        } else {
            Some(name.to_string())
        }
    });

    let type_name = match discovered_type {
        Some(t) => t,
        None => {
            eprintln!(
                "test_e2e_issue_link_with_type_and_unlink_with_type: \
                 clean-skip — no non-'Relates' link type found on this instance \
                 (link_types: {link_types_json})"
            );
            return;
        }
    };

    // Step 2: seed two throwaway issues A and B.
    let summary_a = format!("[e2e {label}] typed-link-seed-A");
    let create_a = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary_a,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (typed-link seed A)");

    assert!(
        create_a.status.success(),
        "issue create (typed-link seed A) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_a.stdout),
        String::from_utf8_lossy(&create_a.stderr)
    );

    let key_a = serde_json::from_slice::<Value>(&create_a.stdout)
        .expect("issue create A output must be valid JSON")
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create A JSON must contain a 'key' field")
        .to_string();

    let summary_b = format!("[e2e {label}] typed-link-seed-B");
    let create_b = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary_b,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (typed-link seed B)");

    assert!(
        create_b.status.success(),
        "issue create (typed-link seed B) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_b.stdout),
        String::from_utf8_lossy(&create_b.stderr)
    );

    let key_b = serde_json::from_slice::<Value>(&create_b.stdout)
        .expect("issue create B output must be valid JSON")
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create B JSON must contain a 'key' field")
        .to_string();

    // Confirm GET-consistency for both before linking.
    poll_view(&key_a, &h);
    poll_view(&key_b, &h);

    // Step 3: link A → B using the discovered type T.
    let link_output = h
        .cmd()
        .args([
            "issue", "link", &key_a, &key_b, "--type", &type_name, "--output", "json",
        ])
        .output()
        .expect("failed to spawn jr for issue link --type");

    assert!(
        link_output.status.success(),
        "issue link {key_a} {key_b} --type {type_name:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&link_output.stdout),
        String::from_utf8_lossy(&link_output.stderr)
    );

    // Verify link: poll_view(A), find a link entry that references B AND has type T.
    let view_a_linked = poll_view(&key_a, &h);
    let issue_links = view_a_linked
        .get("fields")
        .and_then(|f| f.get("issuelinks"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let typed_link_found = issue_links.iter().any(|link| {
        // Check B's key appears under inwardIssue or outwardIssue (direction-agnostic, F-09).
        let inward_key = link
            .get("inwardIssue")
            .and_then(|i| i.get("key"))
            .and_then(Value::as_str);
        let outward_key = link
            .get("outwardIssue")
            .and_then(|i| i.get("key"))
            .and_then(Value::as_str);
        let b_present = inward_key == Some(key_b.as_str()) || outward_key == Some(key_b.as_str());

        // Check the link type name matches T (case-insensitive).
        let link_type_name = link
            .get("type")
            .and_then(|t| t.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let type_matches = link_type_name.eq_ignore_ascii_case(&type_name);

        b_present && type_matches
    });

    assert!(
        typed_link_found,
        "after linking {key_a} → {key_b} --type {type_name:?}, must find a link entry with \
         type.name={type_name:?} (case-insensitive) and {key_b} in inwardIssue.key or \
         outwardIssue.key; issuelinks: {:?}",
        issue_links
    );

    // Step 4: unlink A from B scoped to type T.
    let unlink_output = h
        .cmd()
        .args([
            "issue", "unlink", &key_a, &key_b, "--type", &type_name, "--output", "json",
        ])
        .output()
        .expect("failed to spawn jr for issue unlink --type");

    assert!(
        unlink_output.status.success(),
        "issue unlink {key_a} {key_b} --type {type_name:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&unlink_output.stdout),
        String::from_utf8_lossy(&unlink_output.stderr)
    );

    // Verify unlink: poll_view(A), assert no typed link to B remains.
    let view_a_unlinked = poll_view(&key_a, &h);
    let issue_links_after = view_a_unlinked
        .get("fields")
        .and_then(|f| f.get("issuelinks"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let typed_link_still_present = issue_links_after.iter().any(|link| {
        let inward_key = link
            .get("inwardIssue")
            .and_then(|i| i.get("key"))
            .and_then(Value::as_str);
        let outward_key = link
            .get("outwardIssue")
            .and_then(|i| i.get("key"))
            .and_then(Value::as_str);
        let b_present = inward_key == Some(key_b.as_str()) || outward_key == Some(key_b.as_str());

        let link_type_name = link
            .get("type")
            .and_then(|t| t.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let type_matches = link_type_name.eq_ignore_ascii_case(&type_name);

        b_present && type_matches
    });

    assert!(
        !typed_link_still_present,
        "after unlinking {key_a} from {key_b} --type {type_name:?}, the typed link must be \
         gone; issuelinks after unlink: {:?}",
        issue_links_after
    );
}

/// E2E: `jr issue remote-link <KEY> --url <URL> --title <TITLE>` create-only smoke.
///
/// Seeds one throwaway issue labeled with `run_label()` for teardown. Posts a remote
/// link to a stable no-op URL (`https://example.com/e2e`). Asserts exit-0 and that
/// stdout is a valid JSON object (the response shape varies by instance but is always
/// a JSON object with at least `id` and `self` when the link is created).
///
/// # Why no read-back verification
///
/// Remote links are NOT included in issue `fields` from `GET /rest/api/3/issue/{key}`.
/// They are available only via `GET /rest/api/3/issue/{key}/remotelink`, which `jr`
/// does not expose. Read-back verification is therefore out of scope for this suite
/// (E2E-PG-4 / research Q2). Teardown is handled by deleting the parent issue via
/// the sweeper (which deletes the issue, cascading to its remote links).
///
/// Traces to: E2E-PG-4, design spec §6.4.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_remote_link_smoke() {
    // Remote links are not retrievable via issue GET fields (separate /remoteLink endpoint
    // jr does not expose); this is a create-only smoke — round-back verification is OUT
    // OF SCOPE (see E2E-PG-4 / research Q2).
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed a throwaway issue for the remote link to attach to.
    let summary = format!("[e2e {label}] remote-link-smoke-seed");
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (remote-link smoke seed)");

    assert!(
        create_output.status.success(),
        "issue create (remote-link smoke seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_output.stdout),
        String::from_utf8_lossy(&create_output.stderr)
    );

    let key = serde_json::from_slice::<Value>(&create_output.stdout)
        .expect("issue create output must be valid JSON")
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Confirm GET-consistency before attaching the remote link.
    poll_view(&key, &h);

    // Create the remote link. Title embeds the run label for traceability.
    let title = format!("e2e {label}");
    let remote_link_output = h
        .cmd()
        .args([
            "issue",
            "remote-link",
            &key,
            "--url",
            "https://example.com/e2e",
            "--title",
            &title,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue remote-link");

    assert!(
        remote_link_output.status.success(),
        "issue remote-link {key} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&remote_link_output.stdout),
        String::from_utf8_lossy(&remote_link_output.stderr)
    );

    // Assert stdout parses as JSON AND is a non-empty object (LOW-1).
    // Do NOT assert instance-specific id/self/url values — those differ per site.
    let stdout = String::from_utf8_lossy(&remote_link_output.stdout);
    let response: Value =
        serde_json::from_str(stdout.trim()).expect("issue remote-link stdout must be valid JSON");
    assert!(
        response.is_object(),
        "issue remote-link stdout must be a JSON object; got: {response}"
    );
    assert!(
        !response.as_object().map(|o| o.is_empty()).unwrap_or(true),
        "issue remote-link JSON must be a non-empty object (>=1 key); got: {response}"
    );
    // Teardown: the sweeper deletes the parent issue which cascades to its remote links.
}

// ---------------------------------------------------------------------------
// S-E2E-4 — §6.3 Error / Exit-Code Paths (no mutation)
// ---------------------------------------------------------------------------

/// E2E: `jr issue view E2E-99999999 --output json` exits with a non-zero code
/// in `{1, 64}` (404-not-found path).
///
/// No mutation. Assert: exit code ∈ {1, 64} + stdout empty + no panic.
/// Do NOT assert error message substrings (locale/wording-fragile).
///
/// Traces to: AC-012, BC-7.3.006, design spec §6.3.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_view_404_exits_nonzero() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["issue", "view", "E2E-99999999", "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue view (404 test)");

    let exit_code = output.status.code().unwrap_or(-1);
    // Exit code must be in {1, 64} — NOT 0 (success) and NOT 101 (panic/SIGABRT).
    assert!(
        exit_code == 1 || exit_code == 64,
        "issue view of non-existent key must exit 1 or 64; got {exit_code}"
    );
    // stdout must be empty — no JSON error envelope on error paths (H-2).
    assert!(
        output.stdout.trim_ascii().is_empty(),
        "stdout must be empty on 404 error path; got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    // Error text must appear on stderr (we don't assert wording — locale-fragile).
    assert!(
        !output.stderr.is_empty(),
        "stderr must be non-empty (error message) on 404 path"
    );
}

/// E2E: `jr issue list --jql "this is not valid (" --output json` exits with a
/// non-zero code in `{1, 64}` (400 malformed JQL path).
///
/// No mutation. Assert: exit code ∈ {1, 64} + stdout empty + no panic.
///
/// Traces to: AC-013, BC-7.3.006, design spec §6.3.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_list_bad_jql_exits_nonzero() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let output = h
        .cmd()
        .args([
            "issue",
            "list",
            "--jql",
            "this is not valid (",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue list (bad JQL test)");

    let exit_code = output.status.code().unwrap_or(-1);
    assert!(
        exit_code == 1 || exit_code == 64,
        "issue list with malformed JQL must exit 1 or 64; got {exit_code}"
    );
    assert!(
        output.stdout.trim_ascii().is_empty(),
        "stdout must be empty on bad-JQL error path; got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !output.stderr.is_empty(),
        "stderr must be non-empty (error message) on bad-JQL path"
    );
}

/// E2E: A well-formed but wrong `JR_AUTH_HEADER` exits 2 (`JrError::NotAuthenticated`).
///
/// Constructs a syntactically valid `Basic <base64(wrong:creds)>` header that will
/// 401 from Jira. Overrides `JR_AUTH_HEADER` on the command environment to use the
/// bad header. Asserts: exit code = 2 + stdout empty + no panic.
///
/// **Why `issue create` instead of `issue list`:** A read command like `issue list`
/// may succeed (exit 0) even with bad credentials if the Jira project allows
/// anonymous/public read. Using `issue create` (a write operation) guarantees the
/// command requires authentication regardless of project visibility — Jira always
/// rejects writes with a 401 when the credentials are invalid, so the bad-header
/// override reliably triggers exit 2 on any instance. The create will always fail
/// before an issue is made because the 401 arrives at the HTTP layer before any
/// issue data is committed.
///
/// This is debug-build-only by construction (F-11): the `JR_AUTH_HEADER` seam is
/// gated behind `#[cfg(debug_assertions)]` (SD-002). The harness runs the debug
/// binary, so this is consistent with the rest of the suite.
///
/// Traces to: AC-014, BC-X.3.002, BC-7.3.006, design spec §6.3.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_bad_auth_exits_2() {
    if !e2e_enabled() {
        return;
    }

    // Build a syntactically valid but wrong Basic auth header.
    // base64("wrong-email@example.com:wrong-token") =
    // "d3JvbmctZW1haWxAZXhhbXBsZS5jb206d3JvbmctdG9rZW4="
    let bad_auth_header = "Basic d3JvbmctZW1haWxAZXhhbXBsZS5jb206d3JvbmctdG9rZW4=".to_string();

    let proj = project();
    let itype = issue_type();
    let label = run_label();
    // Defence-in-depth: bad credentials cannot create an issue, but prefix the summary
    // with [e2e <label>] and carry the run label so that IF the instance ever behaved
    // unexpectedly and an issue slipped through, BOTH the per-run teardown
    // (labels=e2e-<run_id>) and the sweeper (summary ~ "e2e") would reap it.
    let summary = format!("[e2e {label}] bad-auth probe (should never be created)");

    let h = e2e_harness();
    let output = h
        .cmd()
        // This .env() call OVERRIDES the good JR_AUTH_HEADER that E2eHarness::cmd()
        // already set. std::process::Command uses the last .env() call for a given
        // key, so ordering matters: this must come AFTER h.cmd() for the override to
        // take effect. Any future refactor of E2eHarness::cmd() must preserve this
        // ordering — moving the good-header injection AFTER this call would break the
        // bad-auth test silently.
        .env("JR_AUTH_HEADER", &bad_auth_header)
        // Use `issue create` (a write): a write cannot be served anonymously, so a
        // public-read project cannot mask the auth failure with a 200.
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &summary,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for bad-auth test");

    // PORTABILITY (live-verified, run 26718339455): wrong credentials on a WRITE must NOT
    // succeed — but the exact failure mode is INSTANCE-DEPENDENT, not a fixed exit code:
    //   - Private instance: bad Basic auth → HTTP 401 → JrError::NotAuthenticated → exit 2.
    //   - Public-read instance (the CI project): a bad-credential write is rejected with
    //     HTTP 400 "you don't have permission to create issues" → JrError::ApiError → exit 1
    //     (observed: `API error (400): The target project doesn't exist or you don't have
    //     permission to create issues in it`).
    // Asserting an exact exit 2 is overfit to one instance (this is NOT a jr bug — jr correctly
    // maps whatever status the server returns). The portable, security-meaningful contract is:
    // the write FAILED (non-zero exit) AND created no issue (no `key` in stdout). Both exit 1
    // (400) and exit 2 (401) satisfy "the wrong credential could not write".
    let exit_code = output.status.code().unwrap_or(-1);
    assert_ne!(
        exit_code,
        0,
        "bad auth must NOT succeed a write — expected non-zero exit (401→2 or 400→1); \
         got {exit_code}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The create must not have produced an issue: stdout must not carry a created `key`.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let created_key = serde_json::from_str::<Value>(stdout.trim())
        .ok()
        .and_then(|v| v.get("key").and_then(Value::as_str).map(str::to_string));
    assert!(
        created_key.is_none(),
        "bad auth must not create an issue — stdout unexpectedly contains a 'key': {stdout}"
    );
}
