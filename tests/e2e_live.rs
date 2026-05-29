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
//! JR_AUTH_HEADER="Basic $(printf '%s:%s' "$EMAIL" "$TOKEN" | base64 -w0)" \
//! JR_E2E_PROJECT=E2E \
//! cargo test --test e2e_live -- --include-ignored --test-threads=1
//! ```
//!
//! `--test-threads=1` is required: the tests share a single live Jira project and
//! parallel execution causes rate-limit pressure and non-deterministic write-flow ordering.
//!
//! # Required environment variables (for gated tests)
//!
//! | Variable            | Required | Notes                                              |
//! |---------------------|----------|----------------------------------------------------|
//! | `JR_RUN_E2E`        | yes      | Must be `"1"` to run gated tests                  |
//! | `JR_E2E_BASE_URL`   | yes      | Real Jira Cloud site URL                           |
//! | `JR_AUTH_HEADER`    | yes      | Pre-composed `Basic <base64(email:token)>` header  |
//! | `JR_E2E_PROJECT`    | yes      | Scrum project key (e.g. `E2E`)                     |
//! | `JR_E2E_BOARD_ID`   | no       | Board ID; enables sprint list/current tests        |
//! | `JR_E2E_JSM_PROJECT`| no       | JSM project key; enables queue/requesttype tests   |
//! | `JR_E2E_EMAIL`      | no       | Service account email; used by user-search test    |
//! | `JR_E2E_STATUS_DONE`| no       | Status name for "closed"; default `"Done"`         |
//! | `JR_E2E_STATUS_IN_PROGRESS` | no | Status name for "in progress"; default `"In Progress"` |

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
/// Panics if the var is unset — every live test that calls this should be
/// guarded by `if !e2e_enabled() { return; }` at the top.
fn project() -> String {
    env::var("JR_E2E_PROJECT").expect("JR_E2E_PROJECT must be set when JR_RUN_E2E=1")
}

/// Returns the configured "Done" status name (default: `"Done"`).
fn status_done() -> String {
    env::var("JR_E2E_STATUS_DONE").unwrap_or_else(|_| "Done".to_string())
}

/// Returns the configured "In Progress" status name (default: `"In Progress"`).
fn status_in_progress() -> String {
    env::var("JR_E2E_STATUS_IN_PROGRESS").unwrap_or_else(|_| "In Progress".to_string())
}

/// Poll `jr issue view <key> --output json` with bounded retry.
///
/// Attempts at most `MAX_ATTEMPTS` iterations with a fixed backoff between
/// each attempt. Returns the parsed `serde_json::Value` on the first
/// successful attempt (exit 0 + valid JSON).
///
/// Rationale: GET-by-key is *assumed* read-after-write consistent (unlike JQL
/// search which is documented eventually consistent), but the bounded retry is
/// the real guarantee per AC-005.
///
/// # Panics
///
/// Panics with a descriptive message after exhausting all attempts.
fn poll_view(key: &str, harness: &E2eHarness) -> Value {
    const MAX_ATTEMPTS: u32 = 5;
    const BACKOFF: Duration = Duration::from_millis(500);

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
            std::thread::sleep(BACKOFF);
        }
    }

    panic!(
        "poll_view({key}): timed out after {MAX_ATTEMPTS} attempts — \
         GET-by-key not consistent"
    );
}

// ---------------------------------------------------------------------------
// AC-001 — Non-gated gate-invariant test (ALWAYS runs in normal `cargo test`)
// ---------------------------------------------------------------------------

/// Pins the invariant that `JR_RUN_E2E` is NOT set in a normal `cargo test` run.
///
/// This test always runs (no `#[ignore]`, no early-return guard). It protects
/// against accidental live calls by failing loudly if `ci.yml` somehow sets
/// `JR_RUN_E2E=1`, which would cause all `#[ignore]`+early-return-gated tests
/// to reach the live Jira site without the `--include-ignored` flag being
/// intentional.
///
/// Traces to: AC-001, NFR-T-E2E-1, design spec §4 Gating.
#[test]
fn test_suite_is_noop_without_jr_run_e2e() {
    // This test runs in ci.yml's plain `cargo test` and asserts that
    // JR_RUN_E2E is NOT set (which would mean ci.yml is unintentionally
    // setting it, causing live tests to run without the --include-ignored flag).
    assert_ne!(
        env::var("JR_RUN_E2E").as_deref(),
        Ok("1"),
        "JR_RUN_E2E=1 must not be set in a normal cargo test run"
    );
}

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
/// Uses a simple state machine that tracks whether the scanner is inside a
/// double-quoted string literal (`"..."`) or a char literal (`'.'`),
/// honoring backslash escapes in both.  Only braces that occur OUTSIDE
/// of any literal are counted toward depth, so `{` / `}` characters in
/// string arguments (e.g. `format!("{{ }}")` or assertion messages) do not
/// confuse the depth counter.
fn extract_fn_body(lines: &[&str], fn_start: usize) -> String {
    #[derive(PartialEq)]
    enum Scan {
        Code,
        InString,
        InChar,
    }

    let mut body = String::new();
    let mut depth = 0usize;
    let mut found_open = false;
    let mut state = Scan::Code;

    'outer: for line in lines.iter().skip(fn_start) {
        body.push_str(line);
        body.push('\n');

        let mut chars = line.chars();
        while let Some(ch) = chars.next() {
            match state {
                Scan::Code => match ch {
                    '"' => state = Scan::InString,
                    '\'' => state = Scan::InChar,
                    '{' => {
                        depth += 1;
                        found_open = true;
                    }
                    '}' => {
                        depth = depth.saturating_sub(1);
                    }
                    _ => {}
                },
                Scan::InString => match ch {
                    '\\' => {
                        // Skip the next character (escape sequence).
                        chars.next();
                    }
                    '"' => state = Scan::Code,
                    _ => {}
                },
                Scan::InChar => match ch {
                    '\\' => {
                        // Skip the escaped char.
                        chars.next();
                    }
                    '\'' => state = Scan::Code,
                    _ => {}
                },
            }
        }

        if found_open && depth == 0 {
            break 'outer;
        }
    }

    body
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
/// May return an empty array on a freshly provisioned project — the assertion
/// is shape-only.
///
/// Traces to: AC-004, NFR-T-E2E-1.
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
}

/// E2E: `jr issue list --jql "project=<E2E> AND summary ~ e2e" --output json`
/// applies the JQL filter correctly and returns a JSON array.
///
/// Traces to: AC-004, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_list_with_summary_filter_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let jql = format!("project={} AND summary ~ e2e", project());
    let output = h
        .cmd()
        .args(["issue", "list", "--jql", &jql, "--output", "json"])
        .output()
        .expect("failed to spawn jr");

    assert!(
        output.status.success(),
        "issue list with summary filter failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("issue list output must be valid JSON");
    assert!(
        v.is_array(),
        "issue list output must be a JSON array; got: {v}"
    );
}

/// E2E: `jr board list --output json` returns a JSON array.
///
/// Shape-only assertion — the board count is site-specific and not guaranteed
/// to be non-empty on all valid E2E sites.
///
/// Traces to: AC-004, NFR-T-E2E-1.
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
}

/// E2E: `jr sprint list --board <BOARD_ID> --output json` returns a JSON array.
///
/// Skipped cleanly when `JR_E2E_BOARD_ID` is not set.
///
/// Traces to: AC-004, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_sprint_list_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let board_id = match env::var("JR_E2E_BOARD_ID") {
        Ok(id) if !id.is_empty() => id,
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

    assert!(
        output.status.success(),
        "sprint list failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("sprint list output must be valid JSON");
    assert!(
        v.is_array(),
        "sprint list output must be a JSON array; got: {v}"
    );
}

/// E2E: `jr sprint current --board <BOARD_ID> --output json` returns valid JSON.
///
/// Skipped cleanly when `JR_E2E_BOARD_ID` is not set.
///
/// Traces to: AC-004, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_sprint_current_returns_json() {
    if !e2e_enabled() {
        return;
    }
    let board_id = match env::var("JR_E2E_BOARD_ID") {
        Ok(id) if !id.is_empty() => id,
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

    assert!(
        output.status.success(),
        "sprint current failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // The output is an object or array depending on whether a sprint is active.
    let _v: Value =
        serde_json::from_slice(&output.stdout).expect("sprint current output must be valid JSON");
}

/// E2E: `jr user search <query> --output json` returns a JSON array.
///
/// Shape-only assertion — Browse Users permission availability varies across
/// sites and is not guaranteed non-empty for all valid E2E deployments
/// (lesson from S-398 over-fitting). The search term is derived from
/// `JR_E2E_EMAIL` if set (local-part only), otherwise falls back to `"e2e"`.
///
/// Traces to: AC-004, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_user_search_returns_array() {
    if !e2e_enabled() {
        return;
    }
    // Use the email's local-part as a search query if available, otherwise "e2e".
    let query = env::var("JR_E2E_EMAIL")
        .ok()
        .and_then(|e| e.split('@').next().map(|s| s.to_string()))
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
}

/// E2E: `jr project fields --project <E2E> --output json` returns a JSON object
/// with the expected top-level keys.
///
/// `project fields --output json` returns an object (not an array) with keys:
/// `project`, `issue_types`, `priorities`, `statuses_by_issue_type`, `asset_fields`.
/// Asserts object shape and presence of `issue_types` and `statuses_by_issue_type`.
///
/// Traces to: AC-004, NFR-T-E2E-1.
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
    assert!(
        v.get("issue_types").is_some(),
        "project fields JSON must contain 'issue_types' key; got: {v}"
    );
    assert!(
        v.get("statuses_by_issue_type").is_some(),
        "project fields JSON must contain 'statuses_by_issue_type' key; got: {v}"
    );
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
        Ok(p) if !p.is_empty() => p,
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
        Ok(p) if !p.is_empty() => p,
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
/// Exercises all 7 sub-steps of the write flow against the live site:
///
/// 1. `issue create` → captures the new issue key.
/// 2. `poll_view(key)` → bounded retry; confirms the issue is GET-consistent.
/// 3. `issue edit` → updates the summary.
/// 4. `issue comment` → posts a comment.
/// 5. `worklog add 5m` → logs 5 minutes of work.
/// 6. `issue move <key> <status_in_progress()>` → single-key idempotent move.
/// 7. `issue move <key> <status_done()>` → closes the issue.
///
/// Assumption: the project uses a workflow with reachable In Progress → Done
/// transitions under the configured status names (`JR_E2E_STATUS_IN_PROGRESS`
/// and `JR_E2E_STATUS_DONE`). If a transition assert fails, the `if: always()`
/// teardown step in `e2e.yml` is the safety net for closing any leaked issues
/// (they carry the `e2e-<run_label>` label for that purpose).
///
/// The label `e2e-<run_label>` is used on the created issue so the CI
/// teardown step (e2e.yml `if: always()`) can close any leftover issues.
///
/// Traces to: AC-007, NFR-T-E2E-1, design spec §4 Write flow.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_write_flow_create_edit_comment_worklog_close() {
    if !e2e_enabled() {
        return;
    }

    let label = run_label();
    let summary_create = format!("[e2e {label}] smoke test");
    let summary_edit = format!("[e2e {label}] smoke test (edited)");
    let proj = project();

    let h = e2e_harness();

    // --- Step 1: create issue ---
    let create_output = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            "Task",
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
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // --- Step 2: poll_view ---
    let view_json = poll_view(&key, &h);
    assert_eq!(
        view_json.get("key").and_then(Value::as_str),
        Some(key.as_str()),
        "poll_view response must contain the created issue key"
    );

    // --- Step 3: edit summary ---
    let edit_output = h
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
        .expect("failed to spawn jr for issue edit");

    assert!(
        edit_output.status.success(),
        "issue edit failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&edit_output.stdout),
        String::from_utf8_lossy(&edit_output.stderr)
    );

    // --- Step 4: add comment ---
    // The `issue comment` subcommand takes the message as a positional argument,
    // not via `--body`. See `IssueCommand::Comment { message: Option<String>, .. }`
    // in src/cli/mod.rs.
    let comment_output = h
        .cmd()
        .args([
            "issue",
            "comment",
            &key,
            "E2E smoke comment",
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

    // --- Step 5: log 5 minutes of work ---
    let worklog_output = h
        .cmd()
        .args(["worklog", "add", &key, "5m", "--output", "json"])
        .output()
        .expect("failed to spawn jr for worklog add");

    assert!(
        worklog_output.status.success(),
        "worklog add failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&worklog_output.stdout),
        String::from_utf8_lossy(&worklog_output.stderr)
    );

    // --- Step 6: move to In Progress (idempotent single-key move) ---
    let move_wip_output = h
        .cmd()
        .args(["issue", "move", &key, &status_in_progress()])
        .output()
        .expect("failed to spawn jr for issue move to in-progress");

    assert!(
        move_wip_output.status.success(),
        "issue move to '{}' failed for {key}:\nstdout: {}\nstderr: {}",
        status_in_progress(),
        String::from_utf8_lossy(&move_wip_output.stdout),
        String::from_utf8_lossy(&move_wip_output.stderr)
    );

    // --- Step 7: move to Done ---
    let move_done_output = h
        .cmd()
        .args(["issue", "move", &key, &status_done()])
        .output()
        .expect("failed to spawn jr for issue move to done");

    assert!(
        move_done_output.status.success(),
        "issue move to '{}' failed for {key}:\nstdout: {}\nstderr: {}",
        status_done(),
        String::from_utf8_lossy(&move_done_output.stdout),
        String::from_utf8_lossy(&move_done_output.stderr)
    );
}

// ---------------------------------------------------------------------------
// AC-004 — worklog list (requires a key; uses a project-scoped list first)
// ---------------------------------------------------------------------------

/// E2E: `jr worklog list <KEY> --output json` exits 0 and returns a JSON array.
///
/// Picks the most recent issue from the project to check worklogs on. If no
/// issues exist in the project yet, this test is skipped (the write-flow test
/// should be run first to seed the project).
///
/// Traces to: AC-004, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_worklog_list_returns_array() {
    if !e2e_enabled() {
        return;
    }

    let h = e2e_harness();

    // Find any existing issue to run worklog list against.
    let jql = format!("project={} ORDER BY created DESC", project());
    let list_output = h
        .cmd()
        .args([
            "issue", "list", "--jql", &jql, "--limit", "1", "--output", "json",
        ])
        .output()
        .expect("failed to spawn jr for issue list");

    assert!(
        list_output.status.success(),
        "issue list for worklog test failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&list_output.stdout),
        String::from_utf8_lossy(&list_output.stderr)
    );

    let issues: Value =
        serde_json::from_slice(&list_output.stdout).expect("issue list output must be valid JSON");

    let key = match issues.as_array().and_then(|arr| arr.first()) {
        Some(issue) => issue
            .get("key")
            .and_then(Value::as_str)
            .expect("issue JSON must have 'key' field")
            .to_string(),
        None => {
            // No issues in the project yet; skip this test.
            return;
        }
    };

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
}

// ---------------------------------------------------------------------------
// AC-004 — issue view (requires a key)
// ---------------------------------------------------------------------------

/// E2E: `jr issue view <KEY> --output json` exits 0 and contains a `"key"` field.
///
/// Uses the most recent issue from the project. Skipped if the project has
/// no issues yet.
///
/// Traces to: AC-004, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_view_returns_key_field() {
    if !e2e_enabled() {
        return;
    }

    let h = e2e_harness();

    let jql = format!("project={} ORDER BY created DESC", project());
    let list_output = h
        .cmd()
        .args([
            "issue", "list", "--jql", &jql, "--limit", "1", "--output", "json",
        ])
        .output()
        .expect("failed to spawn jr for issue list");

    assert!(list_output.status.success(), "issue list failed");

    let issues: Value =
        serde_json::from_slice(&list_output.stdout).expect("issue list output must be valid JSON");

    let key = match issues.as_array().and_then(|arr| arr.first()) {
        Some(issue) => issue
            .get("key")
            .and_then(Value::as_str)
            .expect("issue JSON must have 'key' field")
            .to_string(),
        None => {
            // No issues in the project yet; skip.
            return;
        }
    };

    let view_output = h
        .cmd()
        .args(["issue", "view", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue view");

    assert!(
        view_output.status.success(),
        "issue view failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&view_output.stdout),
        String::from_utf8_lossy(&view_output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&view_output.stdout).expect("issue view output must be valid JSON");
    assert!(
        v.get("key").is_some(),
        "issue view JSON must contain a 'key' field; got: {v}"
    );
}
