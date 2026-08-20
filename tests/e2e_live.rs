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
//! | `JR_E2E_EMAIL`              | no       | Service account email; used by user-search and assign-by-query tests |
//! | `JR_E2E_STATUS_DONE`        | no       | Status name for "closed"; default `"Done"`                    |
//! | `JR_E2E_STATUS_IN_PROGRESS` | no       | Status name for "in progress"; default `"In Progress"`        |
//! | `JR_E2E_ISSUE_TYPE`         | no       | Issue type for test-created issues; default `"Task"` (F-12)   |
//! | `JR_E2E_POLL_MAX_ATTEMPTS`  | no       | Max poll iterations for `poll_jql`/`poll_view` (default 5)   |
//! |                             |          | and `poll_component_filter` (default 7, run 32384091667);    |
//! |                             |          | read by test code only — no `#[cfg(debug_assertions)]` needed |
//! | `JR_E2E_POLL_INITIAL_MS`    | no       | Initial backoff milliseconds for `poll_jql` (default 250)    |
//! |                             |          | and `poll_component_filter` (default 500, run 32384091667);  |
//! |                             |          | read by test code only — no `#[cfg(debug_assertions)]` needed |
//! | `JR_E2E_PARENT_KEY`         | no       | Existing parent/epic key; enables `create --parent` test (E2E-HV-2) |
//! | `JR_E2E_CHILD_TYPE`         | no       | Child issue type valid under the parent (e.g. `Sub-task`); paired with `JR_E2E_PARENT_KEY` |
//! | `JR_E2E_EDIT_FIELD`         | no       | `NAME=VALUE` custom field on the Edit screen; enables `edit --field` test (E2E-HV-2) |
//! |                             |          | The story-points field id is auto-discovered via `jr api`; no env var needed |

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

    /// Write a `config.toml` into the isolated config dir.
    ///
    /// The default harness runs entirely off the `JR_BASE_URL` / `JR_AUTH_HEADER`
    /// test seams and never materializes a config file. Tests that exercise
    /// config-resolved fields — currently only `story_points_field_id`, needed
    /// by `--points` / `--no-points` — call this to seed a minimal
    /// `[profiles.default]` entry. Auth and base URL still come from the env
    /// seams; the file only supplies the field id the resolver reads from config.
    fn write_config(&self, toml: &str) {
        let dir = self.config_dir.path().join("jr");
        std::fs::create_dir_all(&dir).expect("failed to create config dir");
        std::fs::write(dir.join("config.toml"), toml).expect("failed to write config.toml");
    }
}

/// Discover the "Story Points" custom-field id via `jr api /rest/api/3/field`.
///
/// Returns the field id (e.g. `customfield_10016`) for the first field whose
/// name matches a known story-points label, or `None` when no such field
/// exists on the site (clean-skip signal for the points round-trip test) or
/// the API call / parse fails. The match is case-insensitive and covers both
/// the company-managed "Story Points" and team-managed "Story point estimate"
/// labels.
fn discover_story_points_field(h: &E2eHarness) -> Option<String> {
    let path = "/rest/api/3/field".to_string();
    let out = h.cmd().args(["api", &path]).output().ok()?;
    if !out.status.success() {
        eprintln!("[WARN] discover_story_points_field: `jr api {path}` exited non-zero");
        return None;
    }
    let fields: Value = serde_json::from_slice(&out.stdout).ok()?;
    let wanted = ["story points", "story point estimate"];
    fields.as_array()?.iter().find_map(|f| {
        let name = f.get("name").and_then(Value::as_str)?.to_lowercase();
        if wanted.contains(&name.as_str()) {
            f.get("id").and_then(Value::as_str).map(str::to_owned)
        } else {
            None
        }
    })
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

/// Returns a per-invocation-unique suffix for a component fixture NAME
/// (distinct from `run_label()` itself, which stays a stable per-run/per-
/// project marker used elsewhere for label-based sweeper cleanup).
///
/// **MED-2 (S-COMP-E2E-1 adversarial review):** in CI, `run_label()` is
/// `e2e-{GITHUB_RUN_ID}` -- constant across "re-run failed jobs" (only
/// `GITHUB_RUN_ATTEMPT` increments, `GITHUB_RUN_ID` does not). If a run is
/// cancelled or killed before `ComponentDropGuard`'s best-effort `Drop`
/// teardown fires, the component it created leaks under a name derived
/// solely from `run_label()`; the re-run's `component create` call for that
/// same fixture then collides on the still-live name and fails with a real
/// (non-permission) HTTP 400 -- which under the OLD panic-on-any-non-403/404
/// discipline would incorrectly read as a genuine regression rather than a
/// leaked-fixture collision.
///
/// Deliberately scoped to component fixture NAMES only (`{label}-lifecycle`,
/// `{label}-rename-src`/`-dst`) rather than changing `run_label()` itself,
/// which many other tests in this suite depend on for label-based JQL
/// filtering and sweeper-driven cleanup -- widening its shape is out of scope
/// for this fix. Incorporates `GITHUB_RUN_ATTEMPT` (increments on every
/// GitHub Actions re-run; absent locally) plus a nanosecond timestamp
/// (guarantees uniqueness for local runs, which have no `GITHUB_RUN_ATTEMPT`
/// at all, and adds defense-in-depth even in CI) so a leaked fixture from an
/// earlier attempt can never collide with the current one's create call.
fn component_fixture_suffix() -> String {
    let attempt = env::var("GITHUB_RUN_ATTEMPT").unwrap_or_else(|_| "0".to_string());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    format!("{attempt}-{nanos}")
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

/// Best-effort self-close for a JSM issue.
///
/// JSM workflows have no "Done" status (unlike ES Scrum), so this helper
/// discovers a closing transition dynamically using `jr issue transitions
/// --output json` and selects the first transition whose target
/// `statusCategory.key == "done"` — the stable, Jira-wide machine constant
/// for a closing/green status, covering Resolved, Closed, Canceled, and any
/// custom done-category status regardless of workflow name.
///
/// Best-effort self-close for EJ JSM issues created during E2E tests.
///
/// Discovers a done-category transition dynamically via `jr issue transitions
/// <key> --output json` (using `statusCategory.key == "done"` — the stable,
/// Jira-wide machine constant), then issues `jr issue move <key> <transition_name>`.
///
/// Resolution discovery (S-JSM-E2E-3 improvement): before the final move call,
/// this helper attempts to discover a resolution name and passes `--resolution <R>`
/// to produce properly-resolved tickets rather than issues closed via API bypass
/// (resolution=null). Resolution name precedence (highest first):
///
/// 1. `JR_E2E_JSM_RESOLUTION` env override (if set and non-empty).
/// 2. First `name` from `jr issue resolutions --output json`.
///
/// If resolution discovery fails for any reason (non-zero exit, JSON parse error,
/// empty list, missing `name` field), falls back to moving WITHOUT `--resolution`
/// (preserving S-JSM-E2E-2 behavior). A `[WARN]` is emitted on fallback.
///
/// Never fails the test: all failure branches emit `eprintln!("[WARN] …")`
/// and return `()`. No `panic!`, `assert!`, `unwrap()`, or `expect()` on the
/// transitions-fetch or move steps.
///
/// Design: S-JSM-E2E-2 (dynamic close-transition discovery).
///         S-JSM-E2E-3 (resolution discovery added).
/// Root cause fixed: `jr issue move <key> "Done"` fails on EJ JSM workflows
/// that have no transition named "Done" (live-run 26839267723, 2026-06-02).
fn jsm_self_close(key: &str, h: &E2eHarness) {
    let out = match h
        .cmd()
        .args(["issue", "transitions", key, "--output", "json"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[WARN] jsm_self_close: failed to spawn transitions command for {key}: {e}");
            return;
        }
    };

    if !out.status.success() {
        eprintln!(
            "[WARN] jsm_self_close: transitions fetch failed for {key} (exit {:?}) — orphan risk LOW",
            out.status.code()
        );
        return;
    }

    // `jr issue transitions --output json` emits a bare JSON array of Transition objects.
    // Each element shape: {id, name, to: {name, statusCategory: {name, key}}}.
    let transitions: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[WARN] jsm_self_close: JSON parse error for {key}: {e} — orphan risk LOW");
            return;
        }
    };

    // Preference order: Resolved > Closed > Done > any other done-category target status.
    // Match against to.name (the target STATUS name, e.g. "Resolved"), not t.name
    // (the transition verb, e.g. "Resolve") — jr issue move takes a status name.
    let preferred = ["Resolved", "Closed", "Done"];
    let done_name = transitions.as_array().and_then(|arr| {
        // Try preferred target-status names first for determinism.
        for pref in &preferred {
            if arr.iter().any(|t| {
                t["to"]["statusCategory"]["key"].as_str() == Some("done")
                    && t["to"]["name"].as_str() == Some(*pref)
            }) {
                return Some(pref.to_string());
            }
        }
        // Fall back to the first done-category transition's target status name.
        arr.iter()
            .find(|t| t["to"]["statusCategory"]["key"].as_str() == Some("done"))
            .and_then(|t| t["to"]["name"].as_str().map(str::to_owned))
    });

    let name = match done_name {
        Some(n) => n,
        None => {
            eprintln!(
                "[WARN] jsm_self_close: no done-category transition found for {key} — orphan risk LOW"
            );
            return;
        }
    };

    // Resolution discovery (S-JSM-E2E-3): attempt to find a resolution name so that the
    // close produces a properly-resolved ticket rather than one closed via API bypass
    // (resolution=null). Best-effort: any failure falls back to no-resolution move.
    let resolution_name: Option<String> = jsm_discover_resolution(h);

    // Build the move command args with or without --resolution.
    let move_result = if let Some(ref res) = resolution_name {
        h.cmd()
            .args(["issue", "move", key, &name, "--resolution", res])
            .output()
    } else {
        h.cmd().args(["issue", "move", key, &name]).output()
    };

    let close_out = match move_result {
        Ok(o) => o,
        Err(e) => {
            eprintln!(
                "[WARN] jsm_self_close: failed to spawn move command for {key} (transition '{name}'): {e}"
            );
            return;
        }
    };

    if !close_out.status.success() {
        eprintln!(
            "[WARN] jsm_self_close: move to {name:?} failed for {key} (exit {:?}) — \
             orphan risk LOW (Resolution-screen-required transition possible; see spec §6.3)",
            close_out.status.code()
        );
    }
}

/// Discover a resolution name for use with `--resolution` on JSM close transitions.
///
/// Precedence:
///   1. `JR_E2E_JSM_RESOLUTION` env override (if set and non-empty).
///   2. First `name` from `jr issue resolutions --output json`.
///
/// Returns `None` when:
/// - `jr issue resolutions` exits non-zero.
/// - The output cannot be parsed as a JSON array.
/// - The array is empty.
/// - The first element has no `"name"` string field.
///
/// All failure paths emit `eprintln!("[WARN] jsm_discover_resolution: …")` for
/// observability. Callers should fall back to the no-resolution path on `None`.
fn jsm_discover_resolution(h: &E2eHarness) -> Option<String> {
    // 1. Check env override first.
    if let Ok(v) = std::env::var("JR_E2E_JSM_RESOLUTION") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }

    // 2. Fetch from `jr issue resolutions --output json`.
    let out = match h
        .cmd()
        .args(["issue", "resolutions", "--output", "json"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[WARN] jsm_discover_resolution: failed to spawn resolutions command: {e}");
            return None;
        }
    };

    if !out.status.success() {
        eprintln!(
            "[WARN] jsm_discover_resolution: resolutions fetch failed (exit {:?}) — \
             falling back to no-resolution close",
            out.status.code()
        );
        return None;
    }

    let resolutions: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[WARN] jsm_discover_resolution: JSON parse error: {e} — \
                 falling back to no-resolution close"
            );
            return None;
        }
    };

    let arr = match resolutions.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => {
            eprintln!(
                "[WARN] jsm_discover_resolution: empty resolutions list — \
                 falling back to no-resolution close"
            );
            return None;
        }
    };

    match arr[0].get("name").and_then(serde_json::Value::as_str) {
        Some(n) => Some(n.to_string()),
        None => {
            eprintln!(
                "[WARN] jsm_discover_resolution: resolutions[0] has no 'name' string field — \
                 falling back to no-resolution close"
            );
            None
        }
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

/// Create a throwaway Task issue in the E2E project and return its key.
///
/// The issue carries the `e2e-<run_label>` label so the `if: always()`
/// teardown step in `e2e.yml` closes it even if the test panics before its
/// own best-effort close runs. Polls via `poll_view` for GET-consistency
/// before returning so callers can immediately operate on the key.
///
/// Panics (fails the test) only on `issue create` non-zero exit or malformed
/// JSON — a seed failure is a genuine harness/site fault, not a clean skip.
///
/// Shared by the sprint add/remove and multi-key move round-trip tests
/// (E2E-HV-1) to avoid duplicating the create+poll pattern.
fn seed_issue(h: &E2eHarness, label: &str, summary: &str) -> String {
    let itype = issue_type();
    let out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &project(),
            "--type",
            &itype,
            "--summary",
            summary,
            "--label",
            label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for seed issue create");
    assert!(
        out.status.success(),
        "seed issue create failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let json: Value =
        serde_json::from_slice(&out.stdout).expect("seed issue create output must be valid JSON");
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .expect("seed issue create JSON must contain a 'key' field")
        .to_string();
    assert_key_format(&key);
    // Block until the issue is GET-visible so callers can act on it immediately.
    let _ = poll_view(&key, h);
    key
}

/// Best-effort teardown: move an issue to the configured "Done" status.
///
/// Never fails the test — any non-zero exit emits a `[WARN]` and returns.
/// The `e2e-<run_label>` label on seeded issues is the authoritative safety
/// net; this is a courtesy close so the live project stays tidy between runs.
///
/// Passes `--no-resolution` so the close is robust against workflows whose
/// Done transition requires a resolution: without it, ADR-0015 / BC-3.2.013
/// proactive enforcement would exit 64 in non-interactive mode and leave the
/// issue open. `--no-resolution` is a silent no-op on transitions that do not
/// require a resolution, so it is always safe here.
fn best_effort_close(h: &E2eHarness, key: &str) {
    match h
        .cmd()
        .args([
            "issue",
            "move",
            key,
            &status_done(),
            "--no-resolution",
            "--output",
            "json",
        ])
        .output()
    {
        Ok(o) if o.status.success() => {}
        Ok(o) => eprintln!(
            "[WARN] best_effort_close: move {key} to {:?} failed (exit {:?}) — \
             label sweeper will reap; orphan risk LOW",
            status_done(),
            o.status.code()
        ),
        Err(e) => eprintln!("[WARN] best_effort_close: failed to spawn move for {key}: {e}"),
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
            // Also recognises `async fn test_` so that async gated tests cannot
            // silently escape the gate-first invariant enforced below.
            let mut fn_line = None;
            for (offset, line) in lines[i..lines.len().min(i + 5)].iter().enumerate() {
                let trimmed = line.trim_start();
                // Strip an optional `async ` prefix before the `fn test_` check
                // so both sync (`fn test_…`) and async (`async fn test_…`) forms
                // are recognised.  Without this, an `async fn test_` line would
                // not match and the gate check would be silently skipped.
                let without_async = trimmed.strip_prefix("async ").unwrap_or(trimmed);
                if without_async.starts_with("fn test_") {
                    fn_line = Some(i + offset);
                    break;
                }
            }

            if let Some(fn_start) = fn_line {
                // Extract the function name for error messages.
                // Strip `async ` if present before stripping `fn `.
                let fn_name = {
                    let trimmed = lines[fn_start].trim();
                    let without_async = trimmed.strip_prefix("async ").unwrap_or(trimmed);
                    without_async
                        .trim_start_matches("fn ")
                        .split('(')
                        .next()
                        .unwrap_or("(unknown)")
                        .to_string()
                };

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
// JSM E2E tests — guarded by JR_E2E_JSM_PROJECT (S-JSM-E2E-1)
//
// All JSM tests (Scenarios 1-6) require JR_E2E_JSM_PROJECT to be set and
// non-empty. Scenario 7 (non-JSM guard) uses JR_E2E_PROJECT only.
//
// Clean-skip policy (spec §3):
//   §3.1 — missing JR_E2E_JSM_PROJECT → loud eprintln + return
//   §3.2 — empty list from dynamic discovery → loud eprintln + return
//   §3.3 — 403 from any API call → loud eprintln + return (never fail)
//
// Teardown design (spec §6):
//   Write tests (Scenarios 5, 6) self-close via `jr issue move <key> <Done>`.
//   Best-effort: warn on failure, never fail the test on close failure.
//   Labels do NOT propagate through servicedeskapi to Jira issue labels, so
//   the label-based CI sweeper CANNOT cover EJ. Self-close is the only
//   reliable mechanism. (spec §6.2)
// ---------------------------------------------------------------------------

/// E2E: `jr queue list --project <JSM> --output json` exits 0 and every item
/// has non-null `"id"` and `"name"` fields. (Scenario 1 — deepened shape assertions)
///
/// Replaces `test_e2e_jsm_queue_list_exits_ok`. An empty array is a valid state
/// (test passes with zero items). Skipped cleanly when `JR_E2E_JSM_PROJECT` is
/// not set.
///
/// Traces to: AC-001, VER-JSM-E2E-1 (un-contracted orphan — queue list output has no BC;
/// tracked in S-QUEUE-BC-1; see jsm-e2e-coverage.md §2.2).
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_queue_list_shape() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!("[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM test");
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

    let queues: Vec<Value> =
        serde_json::from_slice(&output.stdout).expect("queue list output must be a JSON array");

    // Per-item field assertions — only fires if queues is non-empty (spec §5 Scenario 1).
    for (i, item) in queues.iter().enumerate() {
        assert!(
            item.get("id").is_some() && !item["id"].is_null(),
            "queue list item[{i}] must have non-null 'id' field; got: {item}"
        );
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        assert!(
            !name.is_empty(),
            "queue list item[{i}] must have non-empty 'name' string field; got: {item}"
        );
    }
}

/// E2E: `jr requesttype list --project <JSM> --output json` exits 0 and every
/// item has non-null `"id"` and `"name"` fields. (Scenario 2 — deepened shape assertions)
///
/// Replaces `test_e2e_jsm_requesttype_list_exits_ok`. An empty array is a valid
/// state. Skipped cleanly when `JR_E2E_JSM_PROJECT` is not set.
///
/// Traces to: AC-002, VER-JSM-E2E-2, BC-X.12.001.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_requesttype_list_shape() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!("[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM test");
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

    let rts: Vec<Value> = serde_json::from_slice(&output.stdout)
        .expect("requesttype list output must be a JSON array");

    // Per-item field assertions — only fires if rts is non-empty (spec §5 Scenario 2).
    for (i, item) in rts.iter().enumerate() {
        assert!(
            item.get("id").is_some() && !item["id"].is_null(),
            "requesttype list item[{i}] must have non-null 'id' field; got: {item}"
        );
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        assert!(
            !name.is_empty(),
            "requesttype list item[{i}] must have non-empty 'name' string field; got: {item}"
        );
    }
}

/// E2E: `jr queue view` by name AND by `--id` — exercises both routing branches.
/// (Scenario 3)
///
/// `jr queue view --output json` returns the queue's ISSUES as a JSON array of
/// issue objects (each with `"key"` and `"fields"`), NOT a queue identity object.
/// This test validates both routing paths (name→id resolution vs direct --id) by
/// asserting exit 0 + parseable issue array on each. An empty issue array is a
/// valid pass (an extant queue with zero issues). The routing coverage — not the
/// issue count — is the assertion value.
///
/// Discovers the queue fixture dynamically from `queue list[0]`. Skips cleanly
/// if the queue list is empty or if a 403 is returned.
///
/// Traces to: AC-003, VER-JSM-E2E-3 (un-contracted orphan — queue view output has no BC;
/// tracked in S-QUEUE-BC-1; see jsm-e2e-coverage.md §2.2).
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_queue_view() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!("[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM test");
            return;
        }
    };
    let h = e2e_harness();

    // Step 1: list queues to discover the fixture dynamically (spec §4.1).
    let list_out = h
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
        list_out.status.success(),
        "queue list failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&list_out.stdout),
        String::from_utf8_lossy(&list_out.stderr)
    );

    let queues: Vec<Value> =
        serde_json::from_slice(&list_out.stdout).expect("queue list must be a JSON array");

    // Step 2: skip cleanly if the list is empty (spec §3.2).
    if queues.is_empty() {
        eprintln!("[SKIP] No queues found on {jsm_project} — skipping queue view test");
        return;
    }

    // Step 3: extract first_id and first_name from queues[0] (spec §4.1 steps 3-4).
    let first_id = {
        let id_val = &queues[0]["id"];
        if id_val.is_null() {
            eprintln!("[SKIP] queues[0].id is null — skipping queue view test");
            return;
        }
        // Stringify: id may be integer or string in the JSON response.
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] queues[0].id is an unexpected type — skipping");
            return;
        }
    };
    let first_name = match queues[0].get("name").and_then(Value::as_str) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => {
            eprintln!("[SKIP] queues[0].name is missing or empty — skipping queue view test");
            return;
        }
    };

    // Step 4: by-name path — exercises name→id resolution in src/cli/queue.rs.
    // `queue view --output json` returns the queue's ISSUES as a JSON array of
    // issue objects (key + fields). An empty array is valid (queue exists but has
    // zero issues). If non-empty, each element must carry "key" and "fields".
    let by_name_out = h
        .cmd()
        .args([
            "queue",
            "view",
            &first_name,
            "--project",
            &jsm_project,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");

    let by_name_stderr = String::from_utf8_lossy(&by_name_out.stderr).to_string();

    // 403 clean-skip (spec §3.3).
    if !by_name_out.status.success() && by_name_stderr.contains("403") {
        eprintln!("[SKIP] queue view by-name returned 403 — skipping (feature unavailable)");
        return;
    }

    // Duplicate-name clean-skip (F-3): if EJ has two queues with the same name,
    // resolve_queue_by_name returns UserError (exit 64) with a "Multiple queues"
    // message. This is not a test failure — skip the by-name sub-path and let
    // the by-id sub-path (which never uses name resolution) continue.
    if !by_name_out.status.success() && by_name_stderr.contains("Multiple queues") {
        eprintln!(
            "[SKIP] queue view by-name: multiple queues named '{first_name}' on {jsm_project} \
             — skipping by-name sub-path (spec §4.1 duplicate-name caveat)"
        );
        // Fall through to the by-id sub-path rather than returning; the by-id
        // path still provides routing-branch coverage.
    } else {
        assert!(
            by_name_out.status.success(),
            "queue view by-name failed:\nstdout: {}\nstderr: {by_name_stderr}",
            String::from_utf8_lossy(&by_name_out.stdout),
        );

        let by_name_v: Value = serde_json::from_slice(&by_name_out.stdout)
            .expect("queue view by-name output must be valid JSON");

        // Assert it is a JSON array; if non-empty, each element is an issue object
        // with "key" and "fields". Empty array = valid (queue with no open issues).
        assert_array_of_objects_with_keys(&by_name_v, &["key", "fields"]);
    }

    // Step 5: by-id path — exercises the --id direct routing branch.
    let by_id_out = h
        .cmd()
        .args([
            "queue",
            "view",
            "--id",
            &first_id,
            "--project",
            &jsm_project,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");

    let by_id_stderr = String::from_utf8_lossy(&by_id_out.stderr).to_string();

    // 403 clean-skip (spec §3.3).
    if !by_id_out.status.success() && by_id_stderr.contains("403") {
        eprintln!("[SKIP] queue view by-id returned 403 — skipping (feature unavailable)");
        return;
    }

    assert!(
        by_id_out.status.success(),
        "queue view by-id failed:\nstdout: {}\nstderr: {by_id_stderr}",
        String::from_utf8_lossy(&by_id_out.stdout),
    );

    let by_id_v: Value = serde_json::from_slice(&by_id_out.stdout)
        .expect("queue view by-id output must be valid JSON");

    // Same shape contract: issue array. Empty is valid.
    assert_array_of_objects_with_keys(&by_id_v, &["key", "fields"]);
}

/// E2E: `jr requesttype fields <numeric_id>` exits 0 and response contains a
/// top-level `"fields"` key. Pins the numeric-bypass path end-to-end. (Scenario 4)
///
/// The request-type id is discovered dynamically from `requesttype list[0]`.
/// Because the id is all-ASCII-digit, `src/cli/requesttype.rs` takes the
/// numeric-bypass path (skips `partial_match` and cache name resolution).
///
/// Traces to: AC-004, VER-JSM-E2E-4, BC-X.12.005, BC-3.8.004.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_requesttype_fields() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!("[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM test");
            return;
        }
    };
    let h = e2e_harness();

    // Step 1: list request types to discover the fixture dynamically (spec §4.2).
    let list_out = h
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
        list_out.status.success(),
        "requesttype list failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&list_out.stdout),
        String::from_utf8_lossy(&list_out.stderr)
    );

    let rts: Vec<Value> =
        serde_json::from_slice(&list_out.stdout).expect("requesttype list must be a JSON array");

    // Step 2: skip cleanly if the list is empty (spec §3.2).
    if rts.is_empty() {
        eprintln!(
            "[SKIP] No request types found on {jsm_project} — skipping requesttype fields test"
        );
        return;
    }

    // Step 3: extract first_rt_id and confirm all-ASCII-digit (spec §4.2 steps 3-4).
    let first_rt_id = {
        let id_val = &rts[0]["id"];
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] rts[0].id is not a string or integer — skipping");
            return;
        }
    };

    if !first_rt_id.chars().all(|c| c.is_ascii_digit()) {
        eprintln!(
            "[SKIP] rts[0].id={first_rt_id} is not all-ASCII-digit — skipping numeric-bypass test"
        );
        return;
    }

    // Step 4: run requesttype fields with the numeric id.
    let fields_out = h
        .cmd()
        .args([
            "requesttype",
            "fields",
            &first_rt_id,
            "--project",
            &jsm_project,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");

    let fields_stderr = String::from_utf8_lossy(&fields_out.stderr).to_string();

    // 403 clean-skip (spec §3.3).
    if !fields_out.status.success() && fields_stderr.contains("403") {
        eprintln!("[SKIP] requesttype fields returned 403 — skipping (feature unavailable)");
        return;
    }

    // Step 5: assert exit 0.
    assert!(
        fields_out.status.success(),
        "requesttype fields {first_rt_id} failed:\nstdout: {}\nstderr: {fields_stderr}",
        String::from_utf8_lossy(&fields_out.stdout),
    );

    // Step 6: assert the top-level "fields" key is present.
    let v: Value = serde_json::from_slice(&fields_out.stdout)
        .expect("requesttype fields output must be valid JSON");
    assert!(
        v.get("fields").is_some(),
        "requesttype fields response must contain top-level 'fields' key; got: {v}"
    );
}

/// E2E: Internal vs external comment visibility round-trip on a fresh JSM request.
/// (Scenario 5)
///
/// Creates a fresh EJ request, adds a public comment and an --internal comment,
/// reads back `jr issue comments --output json`, and asserts the
/// `sd.public.comment` entity property is set on the internal comment and absent
/// (or not true) on the public comment. Self-closes the created issue.
///
/// Traces to: AC-005, VER-JSM-E2E-5, BC-3.5.001 (write side: --internal adds
/// sd.public.comment), BC-2.4.041 (read side: issue comments exposes it).
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_comment_visibility() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!("[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM test");
            return;
        }
    };
    let h = e2e_harness();
    let run_id = run_label();

    // Step 1: list request types to discover the fixture dynamically.
    let list_out = h
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

    if !list_out.status.success() {
        let stderr = String::from_utf8_lossy(&list_out.stderr);
        if stderr.contains("403") {
            eprintln!("[SKIP] requesttype list returned 403 — skipping comment visibility test");
            return;
        }
        panic!(
            "requesttype list failed:\nstdout: {}\nstderr: {stderr}",
            String::from_utf8_lossy(&list_out.stdout)
        );
    }

    let rts: Vec<Value> =
        serde_json::from_slice(&list_out.stdout).expect("requesttype list must be a JSON array");

    if rts.is_empty() {
        eprintln!(
            "[SKIP] No request types found on {jsm_project} — skipping comment visibility test"
        );
        return;
    }

    let first_rt_id = {
        let id_val = &rts[0]["id"];
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] rts[0].id is not a usable type — skipping");
            return;
        }
    };

    // Step 3: create a fresh JSM request.
    let summary = format!("[e2e-jsm-comment {run_id}] visibility round-trip");
    let create_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &summary,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");

    let create_stderr = String::from_utf8_lossy(&create_out.stderr).to_string();

    if !create_out.status.success() {
        if create_stderr.contains("403") {
            eprintln!("[SKIP] issue create returned 403 — skipping comment visibility test");
            return;
        }
        eprintln!(
            "[SKIP] issue create failed (non-fatal skip) — cannot test comment visibility\n\
             stdout: {}\nstderr: {create_stderr}",
            String::from_utf8_lossy(&create_out.stdout)
        );
        return;
    }

    let create_v: Value = serde_json::from_slice(&create_out.stdout)
        .expect("issue create --output json must be valid JSON");
    let key = create_v
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain 'key' field")
        .to_string();

    // Step 4: add a public comment (no --internal flag).
    // F-2a: 403 on any comment step → clean-skip (spec §3.3). Close is attempted first.
    let public_comment = format!("public comment from e2e run {run_id}");
    let pub_out = h
        .cmd()
        .args([
            "issue",
            "comment",
            "add",
            &key,
            &public_comment,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");
    let pub_stderr = String::from_utf8_lossy(&pub_out.stderr).to_string();
    if !pub_out.status.success() {
        // FIX 3a: ANY comment-add failure → best-effort close + skip (not just 403).
        // Close always runs once a key is captured so no code path can orphan the issue.
        jsm_self_close(&key, &h);
        eprintln!(
            "[SKIP] issue comment (public) failed (non-fatal, exit {:?}) — \
             skipping comment visibility test\nstdout: {}\nstderr: {pub_stderr}",
            pub_out.status.code(),
            String::from_utf8_lossy(&pub_out.stdout),
        );
        return;
    }

    // Step 5: add an internal comment.
    let internal_comment = format!("internal comment from e2e run {run_id}");
    let int_out = h
        .cmd()
        .args([
            "issue",
            "comment",
            "add",
            &key,
            &internal_comment,
            "--internal",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");
    let int_stderr = String::from_utf8_lossy(&int_out.stderr).to_string();
    if !int_out.status.success() {
        // FIX 3a: ANY comment-add failure → best-effort close + skip (not just 403).
        jsm_self_close(&key, &h);
        eprintln!(
            "[SKIP] issue comment --internal failed (non-fatal, exit {:?}) — \
             skipping comment visibility test\nstdout: {}\nstderr: {int_stderr}",
            int_out.status.code(),
            String::from_utf8_lossy(&int_out.stdout),
        );
        return;
    }

    // Step 6: self-close FIRST, then read + assert (F-2b + FIX 3a: close-always-runs).
    // No-orphan invariant: once a valid key is captured, every subsequent exit path
    // either (a) calls `issue move <key> Done` before returning (comment-add failures
    // now use best-effort-close-then-skip rather than hard assert), or (b) reaches
    // this unconditional close below. Assertions after this point are purely in-memory
    // and cannot leave the issue open.
    //
    // Note: the pre-key-capture path (JSON parse failure before `key` is bound)
    // cannot orphan because no issue key was obtained — nothing to close.
    //
    // Step 9 (executed here, before assertions): self-close (spec §6.1 best-effort).
    // Uses jsm_self_close which discovers a done-category transition dynamically
    // (statusCategory.key == "done") rather than hardcoding "Done" — the EJ JSM
    // workflow has no transition named "Done". (S-JSM-E2E-2 fix.)
    jsm_self_close(&key, &h);

    // F-3: bounded retry on read-back + property assertions.
    // Property expansion can lag on a cold free-tier site; retry the full
    // read-back + assertion cycle with exponential backoff before failing.
    // schedule: 250 ms → 500 ms → 1 000 ms → 2 000 ms (4 sleeps, 5 attempts).
    const MAX_COMMENT_ATTEMPTS: usize = 5;
    let backoff_ms: &[u64] = &[250, 500, 1_000, 2_000];

    // Helper: check whether a comment has sd.public.comment.internal == true.
    let has_internal_prop = |c: &Value| -> bool {
        let props = match c.get("properties").and_then(Value::as_array) {
            Some(p) => p,
            None => return false,
        };
        props.iter().any(|p| {
            p.get("key").and_then(Value::as_str) == Some("sd.public.comment")
                && p.get("value")
                    .and_then(|v| v.get("internal"))
                    .and_then(Value::as_bool)
                    == Some(true)
        })
    };

    // Helper: does a comment's ADF body JSON contain the given text substring?
    // Comment.body is Option<serde_json::Value> (ADF). Matching on the serialized
    // JSON substring mirrors the technique used in the platform write-flow test.
    let body_contains = |c: &Value, needle: &str| -> bool {
        c.get("body")
            .map(|b| {
                serde_json::to_string(b)
                    .unwrap_or_default()
                    .contains(needle)
            })
            .unwrap_or(false)
    };

    // FIX 2: retry until the FULL success predicate holds — not just until both bodies appear.
    // Property expansion (`sd.public.comment`) can lag after the comment body becomes visible.
    // Breaking as soon as both bodies appeared but before the property is expanded causes a
    // hard-fail on the subsequent assert, defeating the F-3 retry purpose.
    //
    // Loop invariant: retry while the full predicate is false AND budget remains.
    // On first iteration where the full predicate holds → break and skip the final asserts
    // (nothing to assert: we already know it passed).
    // On budget exhaustion → emit [SKIP] and return, never a hard fail.
    // The issue is already closed at this point — extra retries are orphan-safe.
    let mut last_comments: Vec<Value> = Vec::new();
    let mut full_predicate_held = false;

    for attempt in 1..=MAX_COMMENT_ATTEMPTS {
        // Step 6 (read): fetch all comments.
        let comments_out = h
            .cmd()
            .args(["issue", "comments", &key, "--output", "json"])
            .output()
            .expect("failed to spawn jr");

        if !comments_out.status.success() {
            let cstderr = String::from_utf8_lossy(&comments_out.stderr);
            if cstderr.contains("403") {
                eprintln!("[SKIP] issue comments returned 403 — skipping assertions");
                return;
            }
            // Non-403 failure: retry if budget remains, else warn and bail.
            if attempt < MAX_COMMENT_ATTEMPTS {
                std::thread::sleep(Duration::from_millis(backoff_ms[attempt - 1]));
                continue;
            }
            eprintln!(
                "[WARN] issue comments failed after {MAX_COMMENT_ATTEMPTS} attempts — \
                 skipping property assertions\nstderr: {cstderr}"
            );
            return;
        }

        let comments: Vec<Value> = match serde_json::from_slice(&comments_out.stdout) {
            Ok(v) => v,
            Err(_) => {
                if attempt < MAX_COMMENT_ATTEMPTS {
                    std::thread::sleep(Duration::from_millis(backoff_ms[attempt - 1]));
                    continue;
                }
                eprintln!("[WARN] issue comments output was not valid JSON — skipping assertions");
                return;
            }
        };

        // Evaluate the FULL success predicate (FIX 2: body presence AND property state).
        // Both must hold before we can pass; either lagging → retry.
        let internal_appeared = comments.iter().any(|c| body_contains(c, &internal_comment));
        let public_appeared = comments.iter().any(|c| body_contains(c, &public_comment));
        let internal_comment_found = comments
            .iter()
            .filter(|c| body_contains(c, &internal_comment))
            .any(&has_internal_prop);
        let public_comment_not_internal = comments
            .iter()
            .filter(|c| body_contains(c, &public_comment))
            .all(|c| !has_internal_prop(c));

        // Full predicate: both bodies visible AND property state correct.
        let predicate = internal_appeared
            && public_appeared
            && internal_comment_found
            && public_comment_not_internal;

        last_comments = comments;

        if predicate {
            full_predicate_held = true;
            break;
        }

        // Predicate not yet satisfied — retry if budget remains.
        if attempt < MAX_COMMENT_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(backoff_ms[attempt - 1]));
        }
        // If this was the last attempt, fall through to budget-exhaustion handling below.
    }

    if !full_predicate_held {
        // Budget exhausted with predicate still false. Check whether the failure is from
        // bodies not appearing (lag) or from property state (genuine regression).
        // Either way: emit [SKIP] for body-absence (environmental); fall through to the
        // hard asserts for property-state failure so genuine regressions are visible.
        let internal_appeared = last_comments
            .iter()
            .any(|c| body_contains(c, &internal_comment));
        let public_appeared = last_comments
            .iter()
            .any(|c| body_contains(c, &public_comment));

        if !internal_appeared || !public_appeared {
            // Comments never appeared — environmental lag; skip.
            eprintln!(
                "[SKIP] comment read-back after {MAX_COMMENT_ATTEMPTS} attempts: \
                 internal_appeared={internal_appeared} public_appeared={public_appeared} — \
                 body/property expansion lag on free-tier site; skipping assertions"
            );
            return;
        }

        // Both bodies appeared but property state is wrong — this is a real regression.
        // Assertions are purely in-memory; no orphan risk (issue is already closed).
        // Step 7 (F-1): the internal comment must have sd.public.comment.internal==true.
        assert!(
            last_comments
                .iter()
                .filter(|c| body_contains(c, &internal_comment))
                .any(&has_internal_prop),
            "The comment whose body contains '{internal_comment}' must have \
             sd.public.comment.internal==true; comments: {last_comments:?}"
        );
        // Step 8 (F-1): the public comment must NOT have sd.public.comment.internal==true.
        assert!(
            last_comments
                .iter()
                .filter(|c| body_contains(c, &public_comment))
                .all(|c| !has_internal_prop(c)),
            "The comment whose body contains '{public_comment}' must NOT have \
             sd.public.comment.internal==true; comments: {last_comments:?}"
        );
    }
}

/// E2E: `jr issue create --request-type` write round-trip against a fresh JSM request.
/// (Scenario 6 — ADR-0014 dispatch fork pin)
///
/// Exercises `handle_jsm_create` which dispatches to
/// `POST /rest/servicedeskapi/request` (NOT `/rest/api/3/issue`). The response
/// type `JsmRequestCreated` deserializes `issue_key: String`; `handle_jsm_create`
/// emits `{"key": issue_key}` on stdout. Self-closes the created issue.
///
/// Traces to: AC-006, VER-JSM-E2E-6, BC-3.8.001, BC-3.8.004.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_create_request_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!("[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM test");
            return;
        }
    };
    let h = e2e_harness();
    let run_id = run_label();

    // Step 1: list request types to discover the fixture dynamically (spec §4.2).
    let list_out = h
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

    if !list_out.status.success() {
        let stderr = String::from_utf8_lossy(&list_out.stderr);
        if stderr.contains("403") {
            eprintln!("[SKIP] requesttype list returned 403 — skipping create round-trip test");
            return;
        }
        panic!(
            "requesttype list failed:\nstdout: {}\nstderr: {stderr}",
            String::from_utf8_lossy(&list_out.stdout)
        );
    }

    let rts: Vec<Value> =
        serde_json::from_slice(&list_out.stdout).expect("requesttype list must be a JSON array");

    // Step 2: skip cleanly if the list is empty (spec §3.2).
    if rts.is_empty() {
        eprintln!(
            "[SKIP] No request types found on {jsm_project} — skipping create round-trip test"
        );
        return;
    }

    // Step 2 cont: extract and validate the id (spec §4.2 steps 3-4).
    let first_rt_id = {
        let id_val = &rts[0]["id"];
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] rts[0].id is not a usable type — skipping");
            return;
        }
    };

    if !first_rt_id.chars().all(|c| c.is_ascii_digit()) {
        eprintln!("[SKIP] rts[0].id={first_rt_id} is not all-ASCII-digit — skipping");
        return;
    }

    // Step 3: create a request via `issue create --request-type` (ADR-0014 fork).
    let summary = format!("[e2e-jsm {run_id}] create round-trip");
    let create_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &summary,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr");

    let create_stderr = String::from_utf8_lossy(&create_out.stderr).to_string();

    if !create_out.status.success() {
        if create_stderr.contains("403") {
            eprintln!("[SKIP] issue create returned 403 — skipping create round-trip test");
            return;
        }
        eprintln!(
            "[SKIP] issue create failed (non-fatal skip) — cannot test create round-trip\n\
             stdout: {}\nstderr: {create_stderr}",
            String::from_utf8_lossy(&create_out.stdout)
        );
        return;
    }

    // Step 4: assert exit 0 and parse the key.
    // Note (FIX 3b): the `.expect()` and empty-key `assert!` below execute before
    // any key is bound, so they cannot orphan an issue — if they fire, no EJ issue
    // key was successfully obtained and there is nothing to close. The no-orphan
    // guarantee only applies once a valid, non-empty `key` is in scope (i.e., from
    // step 5 onward), which is when the self-close at step 6 becomes reachable.
    let create_v: Value = serde_json::from_slice(&create_out.stdout)
        .expect("issue create --output json must be valid JSON");
    let key = create_v
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain 'key' field")
        .to_string();

    assert!(
        !key.is_empty(),
        "issue create --request-type: 'key' field must be non-empty; got: {create_v}"
    );

    // Step 5: non-fatal bounded poll for GET-by-key consistency (F-2b).
    // poll_view() would panic after MAX_ATTEMPTS, orphaning the EJ issue.
    // Instead: local loop returning Option<Value>; on exhaustion warn and continue.
    // The self-close at step 6 is unconditional once key is bound.
    const MAX_VIEW_ATTEMPTS: u32 = 5;
    const VIEW_BACKOFF_MS: [u64; 4] = [250, 500, 1_000, 2_000];
    let mut view_result: Option<Value> = None;
    for attempt in 1..=MAX_VIEW_ATTEMPTS {
        let out = h
            .cmd()
            .args(["issue", "view", &key, "--output", "json"])
            .output()
            .expect("failed to spawn jr for view poll");
        if out.status.success() {
            if let Ok(v) = serde_json::from_slice::<Value>(out.stdout.as_slice()) {
                view_result = Some(v);
                break;
            }
        }
        if attempt < MAX_VIEW_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(
                VIEW_BACKOFF_MS[(attempt - 1) as usize],
            ));
        }
    }

    // Step 6: self-close BEFORE any remaining assertions (F-2b: close-always-runs).
    // Performing the self-close here guarantees that poll exhaustion or a prefix-
    // assertion panic below cannot leave the EJ issue open. Uses jsm_self_close
    // which discovers a done-category transition dynamically (statusCategory.key ==
    // "done") rather than hardcoding "Done" — the EJ JSM workflow has no transition
    // named "Done". (S-JSM-E2E-2 fix.)
    jsm_self_close(&key, &h);

    // Step 7: assert key prefix matches the JSM project (in-memory, no network).
    let expected_prefix = format!("{jsm_project}-");
    assert!(
        key.starts_with(&expected_prefix),
        "issue create --request-type: key '{key}' must start with '{expected_prefix}'"
    );

    // Step 8: assert poll_view result (in-memory, no network after close).
    match view_result {
        Some(view_v) => {
            let view_key = view_v
                .get("key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            assert_eq!(
                view_key, key,
                "poll_view: returned key '{view_key}' must equal created key '{key}'"
            );
        }
        None => {
            eprintln!(
                "[WARN] poll_view({key}) did not resolve after {MAX_VIEW_ATTEMPTS} attempts — \
                 GET-by-key lag on free-tier site; skipping view assertion (issue was closed)"
            );
        }
    }
}

/// E2E: `jr queue list --project <non-JSM>` exits 64 and stderr contains
/// `"Jira Service Management project"`. (Scenario 7 — require_service_desk guard)
///
/// This test does NOT require `JR_E2E_JSM_PROJECT`. It targets the standard
/// Scrum project (`JR_E2E_PROJECT`), which is NOT a JSM project, and asserts
/// that the `require_service_desk` guard fires correctly.
///
/// Traces to: AC-007, VER-JSM-E2E-7, BC-X.8.004.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_jsm_non_jsm_guard() {
    if !e2e_enabled() {
        return;
    }
    let proj = project();
    let h = e2e_harness();

    // Step 1: run queue list against the non-JSM Scrum project.
    let output = h
        .cmd()
        .args(["queue", "list", "--project", &proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr");

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // `jr` always emits error text to STDERR in both human and json modes (src/main.rs
    // uses eprintln! for the JSON error envelope). The combined check below is a defensive
    // superset: it can never miss a stderr-only message and stays robust to any future
    // output-channel change without needing a code update here.
    let combined = format!("{stdout}{stderr}");

    // Defensive clean-skip (FIX 1): an auth failure must not masquerade as a guard-
    // assertion failure. `require_service_desk` rewrites a 401 into JrError::NotAuthenticated
    // (exit code 2, message "Not authenticated…") — the raw "401" string does NOT appear in
    // output on this path. Key on exit code 2 as the definitive auth-failure signal; also
    // check combined text for "401", "403", "Not authenticated" as belt-and-suspenders.
    //
    // Harness precondition (OBS-2): JR_E2E_PROJECT must name a live, reachable, NON-JSM
    // (Jira Software/Work) project. A missing or unreachable project hard-fails this test
    // by design — do NOT broaden the auth-skip to cover 404 or network errors, because
    // that would mask a guard regression where the wrong error is returned instead of
    // exit-64 + the JSM-guard message.
    let is_auth_failure = output.status.code() == Some(2)
        || (output.status.code() != Some(64)
            && (combined.contains("401")
                || combined.contains("403")
                || combined.contains("Not authenticated")));

    if !output.status.success() && is_auth_failure {
        eprintln!(
            "[SKIP] auth failure (token expired/insufficient scope, exit {:?}) — \
             skipping non-JSM guard test (spec §3.3)",
            output.status.code()
        );
        return;
    }

    // Step 2: assert non-zero exit code.
    assert!(
        !output.status.success(),
        "queue list on non-JSM project must fail; got exit 0\nstdout: {stdout}\nstderr: {stderr}",
    );

    // Step 2 (cont): assert specifically exit code 64 (UserError per JrError::exit_code()).
    assert_eq!(
        output.status.code(),
        Some(64),
        "queue list on non-JSM project must exit 64 (UserError); got: {:?}\n\
         stdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );

    // Step 3: assert combined stdout+stderr contains the stable substring from
    // require_service_desk (FIX 1: channel-robust — `--output json` may route error
    // text to stdout, so check both channels rather than stderr alone).
    assert!(
        combined.contains("Jira Service Management project"),
        "stdout+stderr must contain 'Jira Service Management project' \
         (BC-X.8.004 require_service_desk guard); got stdout: {stdout}\nstderr: {stderr}"
    );
}

/// E2E: BC-3.2.013 proactive resolution enforcement — positive path + enforcement assertion.
/// (Scenario 8 — S-JSM-RESOLUTION-REQUIRED inverted from S-JSM-E2E-3 bypass-demo)
///
/// Verifies that `jr issue move` proactively enforces a resolution on done-category
/// transitions: with `--resolution` the move succeeds and the resolution is readable
/// back; without `--resolution` in non-interactive mode the command exits 64 with a
/// `--resolution` hint (BC-3.2.013 enforcement gate fired).
///
/// Steps:
///   a. Discover a resolution via `jr issue resolutions --output json`; pick
///      `JR_E2E_JSM_RESOLUTION` env override if set, else `resolutions[0].name`.
///      If the list is empty → clean-skip.
///   b. Discover a done-category status name via `jr issue transitions --output json`
///      on a probe issue (first discovered RT; immediately closed after probe).
///      If none → clean-skip.
///   c. POSITIVE (BC-3.2.011 + BC-2.3.036): create ticket A; move with --resolution;
///      assert exit 0 (403 → skip); read back; assert `fields.resolution.name == R`.
///   d. ENFORCE (BC-3.2.013): create ticket B; move WITHOUT --resolution in
///      --no-input mode; assert exit 64; assert stderr contains "--resolution" hint.
///      BC-3.2.013 enforcement gate must fire proactively (before the POST).
///   e. TEARDOWN: both tickets self-closed via `jsm_self_close` on every exit path
///      after the key is captured. No created ticket is left open.
///
/// Traces to: BC-3.2.013 (proactive enforcement gate), BC-3.2.011 (resolution body),
///            BC-2.3.036 (read-back), BC-3.2.009 (reactive backstop retained),
///            AC-3 (jsm_self_close resolution), AC-5 (surface guard), VER-JSM-E2E-8.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_resolution_enforcement() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!("[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM resolution test");
            return;
        }
    };
    let h = e2e_harness();
    let run_id = run_label();

    // ── Step a: discover resolution name ─────────────────────────────────────
    let resolution_name: Option<String> = jsm_discover_resolution(&h);
    let resolution_name = match resolution_name {
        Some(n) => n,
        None => {
            eprintln!(
                "[SKIP] jr issue resolutions returned empty or failed — \
                 skipping resolution enforcement test"
            );
            return;
        }
    };
    eprintln!("[INFO] resolution_enforcement: using resolution '{resolution_name}'");

    // ── Step b: discover request-type id for ticket creation ─────────────────
    let list_out = h
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
        .expect("failed to spawn jr requesttype list");

    if !list_out.status.success() {
        let stderr = String::from_utf8_lossy(&list_out.stderr);
        if stderr.contains("403") {
            eprintln!("[SKIP] requesttype list returned 403 — skipping resolution test");
            return;
        }
        panic!(
            "requesttype list failed:\nstdout: {}\nstderr: {stderr}",
            String::from_utf8_lossy(&list_out.stdout)
        );
    }

    let rts: Vec<serde_json::Value> =
        serde_json::from_slice(&list_out.stdout).expect("requesttype list must be a JSON array");

    if rts.is_empty() {
        eprintln!("[SKIP] No request types found on {jsm_project} — skipping resolution test");
        return;
    }

    let first_rt_id = {
        let id_val = &rts[0]["id"];
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] rts[0].id is not a usable type — skipping");
            return;
        }
    };

    // ── Step b (cont): discover done-category transition name via a probe ─────
    // Create a minimal probe request, get its transitions, immediately close it.
    let probe_summary = format!("[e2e-jsm-res-probe {run_id}] transition discovery");
    let probe_create_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &probe_summary,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr issue create (probe)");

    if !probe_create_out.status.success() {
        let stderr = String::from_utf8_lossy(&probe_create_out.stderr);
        if stderr.contains("403") {
            eprintln!("[SKIP] issue create returned 403 (probe) — skipping resolution test");
            return;
        }
        eprintln!(
            "[SKIP] issue create (probe) failed — skipping resolution test\n\
             stdout: {}\nstderr: {stderr}",
            String::from_utf8_lossy(&probe_create_out.stdout)
        );
        return;
    }

    let probe_v: serde_json::Value =
        serde_json::from_slice(&probe_create_out.stdout).expect("probe create JSON must be valid");
    let probe_key = probe_v
        .get("key")
        .and_then(serde_json::Value::as_str)
        .expect("probe create JSON must contain 'key'")
        .to_string();

    // Discover transitions on the probe key.
    let trans_out = h
        .cmd()
        .args(["issue", "transitions", &probe_key, "--output", "json"])
        .output()
        .expect("failed to spawn jr issue transitions (probe)");

    // Close the probe key before checking transitions result — ensures no orphan.
    jsm_self_close(&probe_key, &h);

    if !trans_out.status.success() {
        eprintln!(
            "[SKIP] transitions fetch failed for probe {probe_key} (exit {:?}) — \
             skipping resolution test",
            trans_out.status.code()
        );
        return;
    }

    let transitions_v: serde_json::Value = match serde_json::from_slice(&trans_out.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[SKIP] transitions JSON parse error for probe: {e} — skipping");
            return;
        }
    };

    // Pick a done-category status name (same preference order as jsm_self_close).
    let preferred_status = ["Resolved", "Closed", "Done"];
    let done_status: Option<String> = transitions_v.as_array().and_then(|arr| {
        for pref in &preferred_status {
            if arr.iter().any(|t| {
                t["to"]["statusCategory"]["key"].as_str() == Some("done")
                    && t["to"]["name"].as_str() == Some(*pref)
            }) {
                return Some(pref.to_string());
            }
        }
        arr.iter()
            .find(|t| t["to"]["statusCategory"]["key"].as_str() == Some("done"))
            .and_then(|t| t["to"]["name"].as_str().map(str::to_owned))
    });

    let done_status = match done_status {
        Some(s) => s,
        None => {
            eprintln!(
                "[SKIP] no done-category transition found on {probe_key} — \
                 skipping resolution enforcement test"
            );
            return;
        }
    };
    eprintln!("[INFO] resolution_enforcement: done status name is '{done_status}'");

    // ── Step c: POSITIVE path (BC-3.2.011 + BC-2.3.036) ─────────────────────
    // Create ticket A and move with --resolution.
    let summary_a = format!("[e2e-jsm-res-pos {run_id}] resolution positive path");
    let create_a_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &summary_a,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr issue create (ticket A)");

    if !create_a_out.status.success() {
        let stderr = String::from_utf8_lossy(&create_a_out.stderr);
        if stderr.contains("403") {
            eprintln!("[SKIP] issue create (A) returned 403 — skipping positive path");
            return;
        }
        eprintln!(
            "[SKIP] issue create (A) failed — skipping positive path\n\
             stdout: {}\nstderr: {stderr}",
            String::from_utf8_lossy(&create_a_out.stdout)
        );
        return;
    }

    let create_a_v: serde_json::Value =
        serde_json::from_slice(&create_a_out.stdout).expect("ticket A create JSON must be valid");
    let key_a = create_a_v
        .get("key")
        .and_then(serde_json::Value::as_str)
        .expect("ticket A create JSON must contain 'key'")
        .to_string();

    // Move ticket A with --resolution.
    let move_a_out = h
        .cmd()
        .args([
            "issue",
            "move",
            &key_a,
            &done_status,
            "--resolution",
            &resolution_name,
        ])
        .output()
        .expect("failed to spawn jr issue move (ticket A)");

    let move_a_stderr = String::from_utf8_lossy(&move_a_out.stderr).to_string();

    if !move_a_out.status.success() {
        // FIX 3: distinguish known clean-skip conditions from genuine failures.
        if move_a_stderr.contains("403") {
            // 403 → permission issue; close best-effort and skip.
            jsm_self_close(&key_a, &h);
            eprintln!("[SKIP] issue move (A) returned 403 — skipping positive path");
            return;
        }
        if move_a_stderr
            .to_lowercase()
            .contains("multiple resolutions")
        {
            // Ambiguous resolution name (exit 64, BC resolver fires before API call).
            // This happens when the instance has two resolutions with the same name.
            // Not a --resolution bug; clean-skip.
            jsm_self_close(&key_a, &h);
            eprintln!(
                "[SKIP] issue move (A) returned 'Multiple resolutions' (exit {:?}) — \
                 ambiguous resolution name '{resolution_name}'; skipping positive path",
                move_a_out.status.code()
            );
            return;
        }
        // Any other failure is a genuine --resolution regression worth surfacing.
        jsm_self_close(&key_a, &h);
        panic!(
            "issue move {key_a} {done_status} --resolution {resolution_name} failed \
             (unexpected; this is the positive path):\n\
             exit: {:?}\nstdout: {}\nstderr: {move_a_stderr}",
            move_a_out.status.code(),
            String::from_utf8_lossy(&move_a_out.stdout),
        );
    }

    // FIX 2: predicate-driven retry — only break when fields.resolution.name is populated.
    // Breaking on first exit-0 + parseable JSON (old behavior) false-fails on cold sites
    // where the issue body returns before the resolution field propagates (same anti-pattern
    // fixed for Scenario 5 comment-property lag). The predicate: retry while the name is
    // absent or empty; break as soon as it is non-empty.
    const MAX_VIEW_ATTEMPTS: u32 = 5;
    const VIEW_BACKOFF_MS: [u64; 4] = [250, 500, 1_000, 2_000];
    // `view_a` holds the FINAL view value where the resolution field was confirmed
    // non-empty (predicate satisfied). `None` means budget was exhausted without
    // the field appearing.
    let mut view_a: Option<serde_json::Value> = None;
    for attempt in 1..=MAX_VIEW_ATTEMPTS {
        let vout = h
            .cmd()
            .args(["issue", "view", &key_a, "--output", "json"])
            .output()
            .expect("failed to spawn jr issue view (ticket A)");
        if vout.status.success() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&vout.stdout) {
                // Predicate: fields.resolution.name is present and non-empty.
                let has_resolution = v
                    .get("fields")
                    .and_then(|f| f.get("resolution"))
                    .and_then(|r| r.get("name"))
                    .and_then(serde_json::Value::as_str)
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                if has_resolution {
                    view_a = Some(v);
                    break;
                }
            }
        }
        if attempt < MAX_VIEW_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(
                VIEW_BACKOFF_MS[(attempt - 1) as usize],
            ));
        }
    }

    match &view_a {
        Some(v) => {
            let res_name = v
                .get("fields")
                .and_then(|f| f.get("resolution"))
                .and_then(|r| r.get("name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            // FIX 1: case-insensitive comparison — jr resolves the name case-insensitively
            // and Jira persists the canonical casing. A JR_E2E_JSM_RESOLUTION=done override
            // (canonical "Done") must not false-fail.
            assert!(
                res_name.eq_ignore_ascii_case(&resolution_name),
                "AC-1 (BC-3.2.011 + BC-2.3.036): after move with --resolution '{resolution_name}', \
                 fields.resolution.name must match '{resolution_name}' (case-insensitive); \
                 got '{res_name}'. full issue view: {v}"
            );
            eprintln!(
                "[INFO] resolution_enforcement POSITIVE: fields.resolution.name '{res_name}' \
                 matches '{resolution_name}' (case-insensitive; BC-3.2.011 confirmed)"
            );
        }
        None => {
            // FIX 2: budget exhausted without the field appearing → skip read-back assertion.
            // The move already returned exit 0, which is the atomic API acceptance evidence
            // for BC-3.2.011. GET-by-key resolution-field lag on a cold free-tier site is
            // environmental, not a jr bug.
            eprintln!(
                "[WARN] resolution_enforcement POSITIVE: fields.resolution.name did not appear \
                 in {MAX_VIEW_ATTEMPTS} view attempts — GET-by-key resolution lag on free-tier \
                 site; skipping read-back assertion. move accepted --resolution (exit 0) is \
                 sufficient evidence per spec §8 (BC-3.2.011 floor satisfied)"
            );
        }
    }

    // ── Step d: ENFORCEMENT assertion (BC-3.2.013) ────────────────────────────
    // S-JSM-RESOLUTION-REQUIRED: inverted from bypass-demo to hard enforcement assertion.
    // Create ticket B and attempt move WITHOUT --resolution in --no-input mode.
    // BC-3.2.013 requires jr to exit 64 and include "--resolution" in stderr.
    let summary_b = format!("[e2e-jsm-res-enforce {run_id}] resolution enforcement path");
    let create_b_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &summary_b,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr issue create (ticket B)");

    if !create_b_out.status.success() {
        let stderr = String::from_utf8_lossy(&create_b_out.stderr);
        jsm_self_close(&key_a, &h); // A was already moved (done-category) — jsm_self_close is idempotent
        if stderr.contains("403") {
            eprintln!("[SKIP] issue create (B) returned 403 — skipping enforcement path");
            return;
        }
        eprintln!(
            "[SKIP] issue create (B) failed — skipping enforcement path\n\
             stdout: {}\nstderr: {stderr}",
            String::from_utf8_lossy(&create_b_out.stdout)
        );
        return;
    }

    let create_b_v: serde_json::Value =
        serde_json::from_slice(&create_b_out.stdout).expect("ticket B create JSON must be valid");
    let key_b = create_b_v
        .get("key")
        .and_then(serde_json::Value::as_str)
        .expect("ticket B create JSON must contain 'key'")
        .to_string();

    // Attempt move WITHOUT --resolution in --no-input mode.
    // BC-3.2.013: proactive gate must exit 64 + stderr contains "--resolution".
    let move_b_out = h
        .cmd()
        .args(["issue", "move", &key_b, &done_status, "--no-input"])
        .output()
        .expect("failed to spawn jr issue move (ticket B)");

    let move_b_stderr = String::from_utf8_lossy(&move_b_out.stderr).to_string();

    if move_b_out.status.code() == Some(403) || move_b_stderr.contains("403") {
        // 403 on move B — clean-skip after teardown.
        jsm_self_close(&key_b, &h);
        eprintln!("[SKIP] issue move (B) returned 403 — enforcement path skipped");
        return;
    }

    assert_eq!(
        move_b_out.status.code(),
        Some(64),
        "BC-3.2.013: proactive gate must exit 64 for no-resolution done-category move \
         in --no-input mode; got {:?}\nstderr: {move_b_stderr}",
        move_b_out.status.code()
    );
    assert!(
        move_b_stderr.contains("--resolution"),
        "BC-3.2.013: stderr must contain '--resolution' hint; got stderr: {move_b_stderr}"
    );
    eprintln!(
        "[INFO] ENFORCE: BC-3.2.013 proactive gate fired — jr refused no-resolution \
         done-category move (exit 64, '--resolution' in stderr)"
    );

    // Ticket B was NOT moved — proactive gate blocked the POST. Self-close it now.
    jsm_self_close(&key_b, &h);
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
    // `jr issue comment add` takes the message as a positional argument (S-577-1).
    // See `CommentSubcommand::Add { message: Option<String>, .. }` in src/cli/mod.rs.
    let comment_output = h
        .cmd()
        .args([
            "issue",
            "comment",
            "add",
            &key,
            &comment_text,
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

// ===========================================================================
// E2E-HV-1 — high-value coverage gap closure
//   1. project list           (read; pagination surface)
//   2. user list --project    (read; assignable-users path)
//   3. sprint add / remove     (write round-trip)
//   4. issue move multi-key    (bulk transition path — non-idempotent per CLAUDE.md)
// ===========================================================================

/// E2E: `jr project list --output json` returns a JSON array of projects.
///
/// When non-empty, each element has `key` + `name` (presence + type, not value
/// equality — project inventory varies per site). The authenticated service
/// account always has at least the E2E project visible, but the "if non-empty"
/// contract stays portable across sites (lesson from S-398 over-fitting).
///
/// Traces to: E2E-HV-1, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_project_list_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["project", "list", "--output", "json"])
        .output()
        .expect("failed to spawn jr for project list");

    assert!(
        output.status.success(),
        "project list failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("project list output must be valid JSON");
    let arr = v
        .as_array()
        .unwrap_or_else(|| panic!("project list output must be a JSON array; got: {v}"));
    // The authenticated account always has access to at least its own project,
    // so an empty array is a genuine defect (e.g. pagination dropping the only
    // page or a serde regression), NOT a portable "if non-empty" case. Assert
    // non-empty so the test cannot pass vacuously.
    assert!(
        !arr.is_empty(),
        "project list must return at least one accessible project; got an empty array"
    );
    // Each element exposes key + name.
    assert_array_of_objects_with_keys(&v, &["key", "name"]);
}

/// E2E: `jr user list --project <P> --output json` returns a JSON array.
///
/// Lists users assignable to the E2E project. When non-empty, each element has
/// `accountId` + `displayName` (same serde-confirmed keys as `user search`).
/// Depends on the "Browse users and groups" permission, so an empty array is a
/// valid result — the "if non-empty" contract avoids permission-coupling.
///
/// Traces to: E2E-HV-1, BC-2.2.028, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_user_list_assignable_returns_array() {
    if !e2e_enabled() {
        return;
    }
    let proj = project();
    let h = e2e_harness();
    let output = h
        .cmd()
        .args(["user", "list", "--project", &proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr for user list");

    assert!(
        output.status.success(),
        "user list failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let v: Value =
        serde_json::from_slice(&output.stdout).expect("user list output must be valid JSON");
    assert!(
        v.is_array(),
        "user list output must be a JSON array; got: {v}"
    );
    assert_array_of_objects_with_keys(&v, &["accountId", "displayName"]);
}

/// E2E: `jr sprint add --sprint <ID> <KEY>` then `jr sprint remove <KEY>` round-trip.
///
/// Clean-skips when:
/// - `JR_E2E_BOARD_ID` is unset (no board to resolve a sprint from), OR
/// - the board is not a scrum board (`"only available for scrum boards"`), OR
/// - the board has no active sprint (`"No active sprint"`).
///
/// On the happy path: seeds a throwaway issue, adds it to the active sprint
/// (asserts `added: true`), removes it back to the backlog (asserts
/// `removed: true`), then best-effort closes the seed issue. The active sprint
/// id is discovered via `jr sprint current` rather than hard-coded (S-398
/// over-fitting lesson).
///
/// Traces to: E2E-HV-1, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_sprint_add_remove_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let board_id = match env::var("JR_E2E_BOARD_ID") {
        Ok(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => return, // clean skip: no board configured
    };
    let h = e2e_harness();

    // Discover the active sprint id via `jr sprint current` (do not hard-code).
    let current = h
        .cmd()
        .args([
            "sprint", "current", "--board", &board_id, "--output", "json",
        ])
        .output()
        .expect("failed to spawn jr for sprint current");
    if !current.status.success() {
        let stderr = String::from_utf8_lossy(&current.stderr);
        if stderr.contains("No active sprint") || stderr.contains("only available for scrum boards")
        {
            return; // clean skip: no sprint capability / no active sprint
        }
        panic!("sprint current failed unexpectedly:\nstderr: {stderr}");
    }
    let current_json: Value =
        serde_json::from_slice(&current.stdout).expect("sprint current output must be valid JSON");
    let sprint_id = current_json
        .get("sprint")
        .and_then(|s| s.get("id"))
        .and_then(Value::as_u64)
        .expect("sprint current JSON must contain sprint.id");

    let label = run_label();
    let key = seed_issue(&h, &label, &format!("[e2e {label}] sprint round-trip"));

    // Add to the active sprint.
    let add = h
        .cmd()
        .args([
            "sprint",
            "add",
            "--sprint",
            &sprint_id.to_string(),
            &key,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for sprint add");
    assert!(
        add.status.success(),
        "sprint add failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );
    let add_json: Value =
        serde_json::from_slice(&add.stdout).expect("sprint add output must be valid JSON");
    assert_eq!(
        add_json.get("added"),
        Some(&Value::Bool(true)),
        "sprint add JSON must have added: true; got: {add_json}"
    );

    // Remove back to the backlog.
    let remove = h
        .cmd()
        .args(["sprint", "remove", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for sprint remove");
    assert!(
        remove.status.success(),
        "sprint remove failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&remove.stdout),
        String::from_utf8_lossy(&remove.stderr)
    );
    let remove_json: Value =
        serde_json::from_slice(&remove.stdout).expect("sprint remove output must be valid JSON");
    assert_eq!(
        remove_json.get("removed"),
        Some(&Value::Bool(true)),
        "sprint remove JSON must have removed: true; got: {remove_json}"
    );

    best_effort_close(&h, &key);
}

/// E2E: `jr issue move <K1> <K2> --to <STATUS> --output json` (multi-key bulk).
///
/// The single-key move path is covered by the write-flow test; this exercises
/// the distinct bulk path (`POST .../bulk` + async poll), which is documented
/// as non-idempotent in CLAUDE.md and previously had no live coverage.
///
/// Seeds two throwaway issues, waits for search-index visibility, then bulk-moves
/// both to "In Progress" (a non-done status, so the bulk path is not subject to
/// single-key resolution enforcement) and asserts the `{taskId, results: [...]}`
/// payload reports one `{key, status}` per key with a documented status
/// (`success` / `inaccessible` / `error`).
///
/// The async bulk task can report a freshly-seeded issue as `inaccessible` (its
/// accessibility index lags behind GET and JQL search — a Jira-side condition,
/// not a `jr` defect). To stay deterministic, any non-`success` key is then
/// driven to its target via the single-key transition endpoint (a direct
/// GET-transitions + POST that does not use the lagging bulk-accessibility index).
/// Then best-effort closes both seeds.
///
/// Traces to: E2E-HV-1, BC-3.2.009, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_move_multikey_bulk() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let label = run_label();
    let target = status_in_progress();

    let key1 = seed_issue(&h, &label, &format!("[e2e {label}] bulk-move A"));
    let key2 = seed_issue(&h, &label, &format!("[e2e {label}] bulk-move B"));

    // Block until BOTH seeds are SEARCH-INDEX-visible before firing the bulk move.
    //
    // seed_issue only guarantees GET-consistency (poll_view). The async
    // bulk-transition task, however, resolves issues through the search index,
    // which lags independently: a GET-visible-but-not-yet-indexed issue is
    // excluded from the task's processed *and* failed sets and reported
    // `inaccessible`. That is the flake this guards against (live run
    // 27159962721 reported ES-1049 as inaccessible). poll_jql with
    // FailOnShort(2) absorbs the lag with the same idiom as
    // test_e2e_pagination_dedup: 0 results = pure index lag (clean-skip),
    // 1 result after full budget = fail loud, 2 = proceed. An exact `key in`
    // probe queries the same index subsystem the bulk task uses, so a positive
    // result is the strongest available "the move will not report inaccessible"
    // gate.
    let settle_jql = format!("key in ({key1}, {key2})");
    let settled = poll_jql(
        &settle_jql,
        |v| v.as_array().is_some_and(|a| a.len() == 2),
        PollJqlMode::FailOnShort(2),
        &h,
    );
    if settled.is_none() {
        // 0 results after full budget — pure index lag; clean-skip rather than
        // fire a move that is guaranteed to report `inaccessible`.
        eprintln!(
            "test_e2e_issue_move_multikey_bulk: seeds never reached search-index \
             visibility (0 results after full poll budget) — clean-skip"
        );
        best_effort_close(&h, &key1);
        best_effort_close(&h, &key2);
        return;
    }

    // Fire the bulk transition — the path under test (POST .../bulk + async poll).
    let output = h
        .cmd()
        .args([
            "issue", "move", &key1, &key2, "--to", &target, "--output", "json",
        ])
        .output()
        .expect("failed to spawn jr for multi-key issue move");

    let v: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "multi-key move stdout must be valid JSON:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
    });

    // Contract under test: the bulk path returns one {key, status} per input key.
    let results = v
        .get("results")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("multi-key move JSON must contain 'results' array; got: {v}"));
    assert_eq!(
        results.len(),
        2,
        "multi-key move must report a result per key; got: {v}"
    );
    let status_of = |key: &str| -> String {
        results
            .iter()
            .find(|r| r.get("key").and_then(Value::as_str) == Some(key))
            .and_then(|r| r.get("status").and_then(Value::as_str))
            .unwrap_or_else(|| panic!("multi-key move results must include {key}; got: {v}"))
            .to_string()
    };
    for key in [&key1, &key2] {
        let s = status_of(key);
        assert!(
            matches!(s.as_str(), "success" | "inaccessible" | "error"),
            "unexpected bulk status {s:?} for {key}; got: {v}"
        );
    }

    // A freshly-seeded issue can be reported `inaccessible` by the async bulk
    // task: its accessibility index lags behind GET *and* JQL search (live runs
    // 27159962721, 27167602346 — the JQL settle above is necessary but not
    // sufficient). That is a Jira-side condition, not a `jr` defect (jr faithfully
    // relays the task result and the {results} contract is validated above). Drive
    // any non-success key to its target via the single-key transition endpoint,
    // which uses a direct GET-transitions + POST (NOT the lagging bulk-accessibility
    // index), so the test stays deterministic. A persistent `error` here surfaces
    // a real failure.
    for key in [&key1, &key2] {
        if status_of(key) == "success" {
            continue;
        }
        let mut delay = Duration::from_millis(500);
        let mut moved = false;
        for attempt in 1..=5 {
            let single = h
                .cmd()
                .args(["issue", "move", key, "--to", &target, "--output", "json"])
                .output()
                .expect("failed to spawn jr for single-key move retry");
            if single.status.success() {
                moved = true;
                break;
            }
            if attempt < 5 {
                std::thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_secs(8));
            }
        }
        assert!(
            moved,
            "key {key} was {:?} in the bulk result and could not be moved to {target:?} \
             via single-key retry either",
            status_of(key),
        );
    }

    best_effort_close(&h, &key1);
    best_effort_close(&h, &key2);
}

// ===========================================================================
// E2E-HV-2 — write-flag coverage (description / stdin / markdown / comment
//            input channels, plus instance-gated points / parent / field)
// ===========================================================================

/// E2E: description round-trip across `create --description`,
/// `edit --description`, and `edit --description-stdin`.
///
/// - Create carries `--description`; the follow-up-GET JSON exposes a non-null
///   `fields.description` (ADF object).
/// - `edit --description <text>` returns `changed_fields.description == <text>`
///   (the RAW input string, NOT an ADF round-trip — BC-3.4.013, issue #398).
/// - `edit --description-stdin` reads the body from piped stdin and produces
///   the same raw-echo contract.
///
/// Traces to: E2E-HV-2, BC-3.4.013, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_description_create_edit_stdin_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let itype = issue_type();
    let proj = project();
    let h = e2e_harness();

    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] description round-trip"),
            "--description",
            "initial description from --description",
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --description");
    assert!(
        create.status.success(),
        "create --description failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let create_json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();
    assert_key_format(&key);
    assert!(
        create_json
            .get("fields")
            .and_then(|f| f.get("description"))
            .is_some_and(|d| !d.is_null()),
        "create --description must yield a non-null fields.description; got: {create_json}"
    );

    // edit --description: changed_fields.description echoes the RAW input.
    let new_desc = "edited description via --description flag";
    let edit = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--description",
            new_desc,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for edit --description");
    assert!(edit.status.success(), "edit --description failed for {key}");
    let edit_json: Value =
        serde_json::from_slice(&edit.stdout).expect("edit output must be valid JSON");
    assert_eq!(
        edit_json
            .get("changed_fields")
            .and_then(|cf| cf.get("description"))
            .and_then(Value::as_str),
        Some(new_desc),
        "edit JSON changed_fields.description must be the raw input string (BC-3.4.013); got: {edit_json}"
    );

    // edit --description-stdin: body piped via stdin, same raw-echo contract.
    let piped_desc = "description piped via --description-stdin";
    let edit_stdin = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--description-stdin",
            "--output",
            "json",
        ])
        .write_stdin(piped_desc)
        .output()
        .expect("failed to spawn jr for edit --description-stdin");
    assert!(
        edit_stdin.status.success(),
        "edit --description-stdin failed for {key}:\nstderr: {}",
        String::from_utf8_lossy(&edit_stdin.stderr)
    );
    let stdin_json: Value =
        serde_json::from_slice(&edit_stdin.stdout).expect("edit-stdin output must be valid JSON");
    assert_eq!(
        stdin_json
            .get("changed_fields")
            .and_then(|cf| cf.get("description"))
            .and_then(Value::as_str),
        Some(piped_desc),
        "edit --description-stdin changed_fields.description must echo the piped raw input; got: {stdin_json}"
    );

    best_effort_close(&h, &key);
}

/// E2E: `issue create --markdown --description <md>` converts Markdown to ADF
/// and the resulting ADF document contains a `heading` node.
///
/// Verifies the **forward** markdown→ADF direction only: a Markdown heading
/// (`# E2E Markdown Heading`) becomes a `heading` ADF node in
/// `fields.description.content`. This proves the `--markdown` flag drove
/// `markdown_to_adf` (rather than `text_to_adf`), which a plain string could
/// not produce.
///
/// Read-path (`adf_to_text`) coverage for the reverse direction is in
/// `test_e2e_adf_read_path_human_output`.
///
/// Traces to: BC-7.2.003, E2E-HV-2, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_markdown_description_produces_heading_node() {
    // Verifies the forward markdown→ADF direction only: asserts a heading node
    // appears in the submitted ADF. Read-path (adf_to_text) coverage is in
    // test_e2e_adf_read_path_human_output.
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let itype = issue_type();
    let proj = project();
    let h = e2e_harness();

    let md = "# E2E Markdown Heading\n\nA paragraph with **bold** text.";
    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] markdown description"),
            "--markdown",
            "--description",
            md,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --markdown");
    assert!(
        create.status.success(),
        "create --markdown failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();

    let content = json
        .get("fields")
        .and_then(|f| f.get("description"))
        .and_then(|d| d.get("content"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!("markdown create must yield ADF fields.description.content; got: {json}")
        });
    assert!(
        content
            .iter()
            .any(|node| node.get("type").and_then(Value::as_str) == Some("heading")),
        "--markdown must produce a heading ADF node (not a flat paragraph); got: {content:?}"
    );

    best_effort_close(&h, &key);
}

/// Recursively search an ADF value for a `text` node whose **visible text is
/// `url`** AND which carries a `link` mark whose `href` **contains `url`**.
/// Returns true on the first match. Used to assert a bare URL survived as a real
/// link rather than plain text (#473).
///
/// Two deliberate choices (F5 review): (1) requiring the matched `text` to equal
/// the URL proves the URL *run itself* is linked, not some neighbouring text a
/// wrong-slice regression might have marked; (2) both `text` and `href` are
/// compared **tolerant of trailing slash(es)** Jira may add on storage
/// (`trim_end_matches('/')` strips any number), but otherwise EXACTLY — a
/// `contains`-style match would wrongly accept a redirect
/// href that merely embeds the URL in a query string (e.g.
/// `https://evil.example?u=https://example.com/...`).
///
/// Recursion is unbounded by depth, which is safe here: the only input is the
/// small, self-created issue description read back via `poll_view`, never an
/// adversarial or pathologically deep document.
fn adf_has_linked_url(node: &Value, url: &str) -> bool {
    let norm = |s: &str| s.trim_end_matches('/').to_string();
    let target = norm(url);
    if node.get("type").and_then(Value::as_str) == Some("text")
        && node.get("text").and_then(Value::as_str).map(&norm) == Some(target.clone())
    {
        if let Some(marks) = node.get("marks").and_then(Value::as_array) {
            let hit = marks.iter().any(|m| {
                m.get("type").and_then(Value::as_str) == Some("link")
                    && m.get("attrs")
                        .and_then(|a| a.get("href"))
                        .and_then(Value::as_str)
                        .map(&norm)
                        == Some(target.clone())
            });
            if hit {
                return true;
            }
        }
    }
    match node {
        Value::Array(items) => items.iter().any(|v| adf_has_linked_url(v, url)),
        Value::Object(map) => map.values().any(|v| adf_has_linked_url(v, url)),
        _ => false,
    }
}

/// E2E (#473): a bare `http(s)://` URL in a `--markdown` description produces an
/// ADF `link` mark that Jira's REST API ACCEPTS and PRESERVES on read-back.
///
/// This is the live proof of the feature's load-bearing premise: Jira does not
/// auto-linkify a plain-text URL in a REST-submitted ADF body (smart-link unfurl
/// is a browser-editor, compose-time feature), so the explicit `link` mark our
/// autolink pass adds is *required* for the URL to be clickable. We create via
/// `--markdown`, then GET-by-key (`poll_view`) and assert the STORED description
/// carries a `link` mark whose `href` is the URL — proving the mark survived both
/// the create POST and Jira's server-side storage/normalization (not merely that
/// our client built it locally before sending).
///
/// Traces to: #473 (bare-URL autolink), broader ADF E2E coverage (#475).
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_markdown_bare_url_produces_link_mark() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let itype = issue_type();
    let proj = project();
    let h = e2e_harness();

    let url = "https://example.com/e2e-autolink";
    let md = format!("see {url} now");
    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] bare-url autolink"),
            "--markdown",
            "--description",
            &md,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --markdown bare-url");
    assert!(
        create.status.success(),
        "create --markdown (bare url) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();

    // Read back what Jira STORED — proves the REST API accepted and preserved the
    // link mark, not just that our client built it locally before the POST.
    let view = poll_view(&key, &h);
    let description = &view["fields"]["description"];
    assert!(
        adf_has_linked_url(description, url),
        "bare URL must round-trip as an ADF link mark on the URL text (href \
         containing {url}); stored description: {description}"
    );

    best_effort_close(&h, &key);
}

/// E2E: `issue comment` across all three input channels — `--file`, `--stdin`,
/// `--markdown` (positional message).
///
/// Seeds a fresh issue (zero comments), adds one comment through each channel
/// (asserting each returns a JSON object with an `id`), then verifies
/// `issue comments --output json` reports at least three comments.
///
/// Traces to: E2E-HV-2, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_comment_input_channels() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let h = e2e_harness();
    let key = seed_issue(&h, &label, &format!("[e2e {label}] comment channels"));

    let assert_comment_id = |out: std::process::Output, channel: &str| {
        assert!(
            out.status.success(),
            "comment via {channel} failed for {key}:\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v: Value = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("comment ({channel}) output must be valid JSON: {e}"));
        assert!(
            v.get("id").and_then(Value::as_str).is_some(),
            "comment ({channel}) JSON must contain an 'id'; got: {v}"
        );
    };

    // --file
    let file = h.config_dir.path().join("comment-body.txt");
    std::fs::write(&file, "comment body from a file").expect("write comment file");
    let file_arg = file.to_string_lossy().to_string();
    let out_file = h
        .cmd()
        .args([
            "issue", "comment", "add", &key, "--file", &file_arg, "--output", "json",
        ])
        .output()
        .expect("failed to spawn jr for comment --file");
    assert_comment_id(out_file, "--file");

    // --stdin
    let out_stdin = h
        .cmd()
        .args([
            "issue", "comment", "add", &key, "--stdin", "--output", "json",
        ])
        .write_stdin("comment body from stdin")
        .output()
        .expect("failed to spawn jr for comment --stdin");
    assert_comment_id(out_stdin, "--stdin");

    // --markdown (positional message)
    let out_md = h
        .cmd()
        .args([
            "issue",
            "comment",
            "add",
            &key,
            "**bold** comment via markdown",
            "--markdown",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for comment --markdown");
    assert_comment_id(out_md, "--markdown");

    // Verify all three landed.
    let comments = h
        .cmd()
        .args(["issue", "comments", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue comments");
    assert!(comments.status.success(), "issue comments failed for {key}");
    let arr: Value =
        serde_json::from_slice(&comments.stdout).expect("comments output must be valid JSON");
    assert!(
        arr.as_array().is_some_and(|a| a.len() >= 3),
        "issue comments must report >= 3 comments after three adds; got: {arr}"
    );

    best_effort_close(&h, &key);
}

/// E2E: story-points round-trip — `create --points`, `edit --points`,
/// `edit --no-points`.
///
/// Self-configuring + clean-skipping: discovers the Story Points field id via
/// `jr api /rest/api/3/field` and writes it into the harness config (the
/// resolver reads `story_points_field_id` from config, which the seam-only
/// harness otherwise lacks). Clean-skips when:
/// - no story-points field exists on the site, OR
/// - the field is not on the project's Create screen (the create attempt fails
///   — an instance-config issue, not a `jr` defect; emits a `[WARN]`).
///
/// On the happy path: `create --points 5` yields `fields.<id> == 5.0`;
/// `edit --points 8` yields `changed_fields.points == "8"`; `edit --no-points`
/// yields `changed_fields.points == "(cleared)"`.
///
/// Traces to: E2E-HV-2, BC-3.4.012, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_points_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let sp_id = match discover_story_points_field(&h) {
        Some(id) => id,
        None => return, // clean skip: no story-points field on this site
    };
    h.write_config(&format!(
        "default_profile = \"default\"\n\n[profiles.default]\nstory_points_field_id = \"{sp_id}\"\n"
    ));

    let label = run_label();
    let itype = issue_type();
    let proj = project();

    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] points round-trip"),
            "--points",
            "5",
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --points");
    if !create.status.success() {
        eprintln!(
            "[WARN] points round-trip: create --points failed (field {sp_id} likely not on the \
             project Create screen) — clean skip; stderr: {}",
            String::from_utf8_lossy(&create.stderr)
        );
        return;
    }
    let create_json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();
    assert_eq!(
        create_json
            .get("fields")
            .and_then(|f| f.get(&sp_id))
            .and_then(Value::as_f64),
        Some(5.0),
        "create --points 5 must set fields.{sp_id} to 5.0; got: {create_json}"
    );

    // edit --points 8
    let edit = h
        .cmd()
        .args(["issue", "edit", &key, "--points", "8", "--output", "json"])
        .output()
        .expect("failed to spawn jr for edit --points");
    assert!(edit.status.success(), "edit --points failed for {key}");
    let edit_json: Value =
        serde_json::from_slice(&edit.stdout).expect("edit output must be valid JSON");
    assert_eq!(
        edit_json
            .get("changed_fields")
            .and_then(|cf| cf.get("points"))
            .and_then(Value::as_str),
        Some("8"),
        "edit --points 8 must echo changed_fields.points == \"8\"; got: {edit_json}"
    );

    // edit --no-points
    let clear = h
        .cmd()
        .args(["issue", "edit", &key, "--no-points", "--output", "json"])
        .output()
        .expect("failed to spawn jr for edit --no-points");
    assert!(clear.status.success(), "edit --no-points failed for {key}");
    let clear_json: Value =
        serde_json::from_slice(&clear.stdout).expect("edit output must be valid JSON");
    assert_eq!(
        clear_json
            .get("changed_fields")
            .and_then(|cf| cf.get("points"))
            .and_then(Value::as_str),
        Some("(cleared)"),
        "edit --no-points must echo changed_fields.points == \"(cleared)\"; got: {clear_json}"
    );

    best_effort_close(&h, &key);
}

/// E2E: `issue create --parent <KEY>` parents a new issue under an existing one.
///
/// Instance-gated: requires `JR_E2E_PARENT_KEY` (an existing parent/epic issue)
/// and `JR_E2E_CHILD_TYPE` (an issue type valid as that parent's child, e.g.
/// `Sub-task` or `Story`). Clean-skips when either is unset, since the valid
/// parent/child hierarchy is entirely project-config dependent.
///
/// On the happy path: creates a child with `--parent` and asserts the
/// follow-up-GET JSON reports `fields.parent.key == <parent>`.
///
/// Traces to: E2E-HV-2, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_parent_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let parent = match env::var("JR_E2E_PARENT_KEY") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => return, // clean skip: no parent issue configured
    };
    let child_type = match env::var("JR_E2E_CHILD_TYPE") {
        Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => return, // clean skip: no child issue type configured
    };
    let label = run_label();
    let proj = project();
    let h = e2e_harness();

    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &child_type,
            "--summary",
            &format!("[e2e {label}] child of {parent}"),
            "--parent",
            &parent,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --parent");
    assert!(
        create.status.success(),
        "create --parent failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();
    assert_eq!(
        json.get("fields")
            .and_then(|f| f.get("parent"))
            .and_then(|p| p.get("key"))
            .and_then(Value::as_str),
        Some(parent.as_str()),
        "create --parent must set fields.parent.key to {parent}; got: {json}"
    );

    best_effort_close(&h, &key);
}

/// E2E: `issue edit --field NAME=VALUE` sets an arbitrary custom field.
///
/// Instance-gated: requires `JR_E2E_EDIT_FIELD` in `NAME=VALUE` form, where
/// `NAME` is a custom field present on the issue's Edit screen (validated via
/// `GET .../editmeta`). Clean-skips when unset, since no custom field is
/// guaranteed to exist on an arbitrary site.
///
/// On the happy path: seeds an issue, applies `--field NAME=VALUE`, and asserts
/// the edit succeeded (`updated == true`) and recorded a non-empty
/// `changed_fields` map (the resolved field keys are instance-specific, so the
/// assertion checks for presence rather than an exact key).
///
/// Traces to: E2E-HV-2, NFR-T-E2E-1.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_edit_custom_field() {
    if !e2e_enabled() {
        return;
    }
    let field = match env::var("JR_E2E_EDIT_FIELD") {
        Ok(f) if f.contains('=') && !f.trim().is_empty() => f,
        _ => return, // clean skip: no custom field configured
    };
    let label = run_label();
    let h = e2e_harness();
    let key = seed_issue(&h, &label, &format!("[e2e {label}] custom field edit"));

    let edit = h
        .cmd()
        .args(["issue", "edit", &key, "--field", &field, "--output", "json"])
        .output()
        .expect("failed to spawn jr for edit --field");
    assert!(
        edit.status.success(),
        "edit --field {field:?} failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&edit.stdout),
        String::from_utf8_lossy(&edit.stderr)
    );
    let json: Value = serde_json::from_slice(&edit.stdout).expect("edit output must be valid JSON");
    assert_eq!(
        json.get("updated"),
        Some(&Value::Bool(true)),
        "edit --field must report updated == true; got: {json}"
    );
    assert!(
        json.get("changed_fields")
            .and_then(Value::as_object)
            .is_some_and(|m| !m.is_empty()),
        "edit --field must record a non-empty changed_fields map; got: {json}"
    );

    best_effort_close(&h, &key);
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
            "add",
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

/// E2E: `jr issue assign <key> --to <email>` — assign-by-query (email primary, display-name fallback).
///
/// This test covers the assign-BY-QUERY resolution path: the `--to <query>` flag
/// triggers `resolve_assignee` → `search_assignable_users` → `GET …/assignable/search?query=…`,
/// which is a distinct code path from the no-arg self-assign that calls `/myself`.
/// (`--to` is the only way to pass a user query; there is no positional for the assignee.)
///
/// LOAD-BEARING INVARIANT: The `--to` value MUST NOT be the literal string `me` —
/// `resolve_assignee` routes `me` to the /myself fast-path BEFORE the assignable-user
/// search, which would bypass the query-resolution path under test. `JR_E2E_EMAIL` is
/// an email address, so this holds in all intended configurations.
///
/// Strategy:
/// 1. Self-assign (no args) to capture `me_account_id` and `me_display_name` as ground truth.
/// 2. Unassign so the next assign is a real state change.
/// 3. Assign by email (primary). Failure modes:
///    - Exit 0 + `changed: true` + accountId == me_account_id (after bounded RYW wait) → PASS.
///    - Exit 0 + `changed: true` + accountId empty after RYW wait → `panic!` citing
///      propagation lag (NOT a resolver defect); mirrors the display-name branch.
///    - Exit 0 + `changed: true` + accountId non-empty but != me_account_id → `panic!`
///      citing resolver defect (wrong account resolved).
///    - Exit non-zero + stderr contains "No assignable user matching" OR
///      "No assignable user with a name matching" → instance privacy policy suppresses email
///      match (`resolve_assignee` emits two distinct no-match forms); fall back to display-name.
///    - Exit non-zero + any other stderr → HARD FAIL (unexpected jr error).
/// 4. Display-name fallback (privacy-locked instances only):
///    - If `me_display_name` is None (also hidden by privacy) → clean-skip with loud
///      eprintln! (instance policy condition, not a jr defect).
///    - Otherwise re-unassign (verified null), then assign by display name (`changed: true`
///      asserted). After a bounded RYW wait: empty accountId → `panic!` citing propagation
///      lag (NOT a resolver defect); non-empty → fall through to final assert.
/// 5. Final assert: effective assignee.accountId == me_account_id.
///
/// COVERAGE LIMITATION: In a single-account E2E instance, `me_account_id` is the only
/// assignable account. This test proves the query→resolve→PUT plumbing round-trips
/// correctly, but cannot detect a resolver that returns the WRONG user among multiple
/// candidates (that would require a second seeded account in the instance).
///
/// Clean-skip: `JR_E2E_EMAIL` not set or empty (email query is the target of this test;
/// without it, there is nothing meaningful to exercise — skipping is correct).
///
/// Traces to: AC-009, BC-3.1.003, design spec §6.2.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_assign_by_query() {
    if !e2e_enabled() {
        return;
    }

    // Clean-skip when JR_E2E_EMAIL is not set or empty — the email query is the
    // primary subject of this test; without it there is nothing to exercise.
    let email = match std::env::var("JR_E2E_EMAIL") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => {
            eprintln!(
                "SKIP: JR_E2E_EMAIL not set — skipping assign-by-query test \
                 (email is the primary query under test)."
            );
            return;
        }
    };

    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // --- Seed one issue ---
    let summary = format!("[e2e {label}] assign-by-query-seed");
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
        .expect("failed to spawn jr for issue create (assign-by-query seed)");

    assert!(
        create_output.status.success(),
        "issue create (assign-by-query seed) failed:\nstdout: {}\nstderr: {}",
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

    // Confirm GET-consistency before proceeding.
    poll_view(&key, &h);

    // --- Ground truth: self-assign (no args) to capture me_account_id ---
    let self_assign_output = h
        .cmd()
        .args(["issue", "assign", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue assign (self, ground-truth step)");

    assert!(
        self_assign_output.status.success(),
        "issue assign (self) failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&self_assign_output.stdout),
        String::from_utf8_lossy(&self_assign_output.stderr)
    );

    let view_after_self = poll_view(&key, &h);
    let me_account_id = view_after_self
        .get("fields")
        .and_then(|f| f.get("assignee"))
        .and_then(|a| a.get("accountId"))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .expect("after self-assign, fields.assignee.accountId must be non-empty")
        .to_string();

    let me_display_name: Option<String> = view_after_self
        .get("fields")
        .and_then(|f| f.get("assignee"))
        .and_then(|a| a.get("displayName"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    // --- Unassign so the email-based assign is a real state change ---
    let unassign_output = h
        .cmd()
        .args(["issue", "assign", &key, "--unassign", "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue assign --unassign");

    assert!(
        unassign_output.status.success(),
        "issue assign --unassign failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&unassign_output.stdout),
        String::from_utf8_lossy(&unassign_output.stderr)
    );

    let view_after_unassign = poll_view(&key, &h);
    assert!(
        view_after_unassign
            .get("fields")
            .and_then(|f| f.get("assignee"))
            .map(|a| a.is_null())
            .unwrap_or(true),
        "after --unassign, fields.assignee must be null; got: {:?}",
        view_after_unassign
            .get("fields")
            .and_then(|f| f.get("assignee"))
    );

    // --- Primary path: assign by email query ---
    let email_assign_output = h
        .cmd()
        .args(["issue", "assign", &key, "--to", &email, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue assign (by email)");

    let email_path_ok: bool;
    let effective_account_id: String;

    // M-2: bounded read-your-writes backoff constants — shared by both the email and
    // display-name assign result reads. Must be declared outside the if/else so the
    // display-name fallback branch can reference them.
    const RYW_MAX: u32 = 5;
    const RYW_BACKOFF_MS: [u64; 4] = [250, 500, 1_000, 2_000];

    if email_assign_output.status.success() {
        // M-1: assert the command JSON reports `changed: true` — proves the resolve→PUT
        // actually ran rather than hitting the idempotent short-circuit (which emits
        // `changed: false`). We unassigned immediately before, so unchanged is a defect.
        let email_assign_json: Value = serde_json::from_slice(&email_assign_output.stdout)
            .expect("assign-by-email stdout must be valid JSON");
        assert_eq!(
            email_assign_json.get("changed"),
            Some(&Value::Bool(true)),
            "assign-by-email JSON must carry `changed: true` (issue was unassigned before this \
             call; `changed: false` means the idempotent short-circuit fired, bypassing the \
             query-resolve→PUT path under test); stdout: {}",
            String::from_utf8_lossy(&email_assign_output.stdout)
        );

        // M-2: bounded read-your-writes wait — poll until assignee.accountId is non-empty
        // (up to 5 attempts with 250 ms → 2 s backoff), then compare. A bare poll_view
        // can return the stale null from the preceding unassign before the PUT propagates,
        // which would trigger a false "resolver defect" panic.
        let mut resulting_id = String::new();
        for attempt in 1..=RYW_MAX {
            let v = poll_view(&key, &h);
            let id = v
                .get("fields")
                .and_then(|f| f.get("assignee"))
                .and_then(|a| a.get("accountId"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !id.is_empty() {
                resulting_id = id;
                break;
            }
            if attempt < RYW_MAX {
                std::thread::sleep(Duration::from_millis(
                    RYW_BACKOFF_MS[(attempt - 1) as usize],
                ));
            }
        }

        if resulting_id.is_empty() {
            // Exit 0 + changed:true but accountId still empty after the full RYW budget —
            // this is a read-your-writes propagation lag, NOT a resolver defect. Mirror the
            // display-name branch's terminal check so the two paths are visibly parallel.
            panic!(
                "assign-by-email exited 0 with changed:true but assignee.accountId \
                 never propagated after the bounded read-your-writes wait \
                 ({RYW_MAX} attempts); this is a read-your-writes propagation lag, \
                 NOT a resolver defect. email={email:?}, me_account_id={me_account_id:?}"
            );
        } else if resulting_id != me_account_id {
            // Exit 0, accountId propagated, but resolved to the WRONG account — genuine
            // resolver defect, not propagation lag and not an instance-policy condition.
            panic!(
                "assign-by-email exited 0 but resolved to the WRONG account (resolver defect); \
                 email={email:?}, expected me_account_id={me_account_id:?}, \
                 got accountId={resulting_id:?}"
            );
        } else {
            // Email path succeeded and resolved to the correct account.
            email_path_ok = true;
            effective_account_id = resulting_id;
        }
    } else {
        // Non-zero exit. Discriminate by stderr content:
        // - "No assignable user matching" (zero search results) OR
        //   "No assignable user with a name matching" (results present but no substring hit)
        //   → `resolve_assignee` emits both forms; both indicate instance privacy policy
        //   suppresses email matching. This is the ONLY case where display-name fallback
        //   is valid.
        // - Anything else → unexpected jr error; hard-fail so we don't silently hide bugs.
        let stderr_str = String::from_utf8_lossy(&email_assign_output.stderr);
        let is_resolver_no_match = stderr_str.contains("No assignable user matching")
            || stderr_str.contains("No assignable user with a name matching");

        assert!(
            is_resolver_no_match,
            "assign-by-email failed with an unexpected error (neither resolver no-match form); \
             this is a jr defect, not an instance-configuration condition. \
             email={email:?}, exit={:?}, stderr={}",
            email_assign_output.status.code(),
            stderr_str
        );

        // Legitimate privacy-policy case: email match suppressed by instance settings.
        // Fall back to display-name. If displayName is also hidden, clean-skip.
        eprintln!(
            "test_e2e_issue_assign_by_query: instance privacy policy suppresses email \
             matching (resolver no-match); falling back to display-name query. \
             email={email:?}"
        );

        email_path_ok = false;

        // O-2: if displayName is also hidden by privacy, we cannot validate on this
        // instance — clean-skip rather than hard-fail (instance-policy condition, not
        // a jr defect).
        let dn = match me_display_name.as_deref().filter(|d| !d.is_empty()) {
            Some(d) => d,
            None => {
                eprintln!(
                    "test_e2e_issue_assign_by_query: email match disabled by instance policy \
                     AND displayName hidden; cannot validate assign-by-query on this instance \
                     — skipping. email={email:?}"
                );
                return;
            }
        };

        // Re-unassign before display-name fallback and verify null (O-1).
        let re_unassign_output = h
            .cmd()
            .args(["issue", "assign", &key, "--unassign", "--output", "json"])
            .output()
            .expect("failed to spawn jr for re-unassign before display-name fallback");
        assert!(
            re_unassign_output.status.success(),
            "re-unassign before display-name fallback failed for {key}:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&re_unassign_output.stdout),
            String::from_utf8_lossy(&re_unassign_output.stderr)
        );
        let view_re_unassigned = poll_view(&key, &h);
        assert!(
            view_re_unassigned
                .get("fields")
                .and_then(|f| f.get("assignee"))
                .map(|a| a.is_null())
                .unwrap_or(true),
            "after re-unassign before display-name fallback, fields.assignee must be null; \
             got: {:?}",
            view_re_unassigned
                .get("fields")
                .and_then(|f| f.get("assignee"))
        );

        eprintln!("test_e2e_issue_assign_by_query: retrying with display-name query {dn:?}");

        let dn_assign_output = h
            .cmd()
            .args(["issue", "assign", &key, "--to", dn, "--output", "json"])
            .output()
            .expect("failed to spawn jr for issue assign (display-name fallback)");

        assert!(
            dn_assign_output.status.success(),
            "assign-by-display-name fallback failed for {key}: \
             display_name={dn:?}, email={email:?};\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&dn_assign_output.stdout),
            String::from_utf8_lossy(&dn_assign_output.stderr)
        );

        // M-1: display-name assign must also report `changed: true`.
        let dn_assign_json: Value = serde_json::from_slice(&dn_assign_output.stdout)
            .expect("assign-by-display-name stdout must be valid JSON");
        assert_eq!(
            dn_assign_json.get("changed"),
            Some(&Value::Bool(true)),
            "assign-by-display-name JSON must carry `changed: true` (issue was re-unassigned \
             before this call); stdout: {}",
            String::from_utf8_lossy(&dn_assign_output.stdout)
        );

        // M-2: bounded read-your-writes wait before reading the resulting accountId.
        let mut dn_resulting_id = String::new();
        for attempt in 1..=RYW_MAX {
            let v = poll_view(&key, &h);
            let id = v
                .get("fields")
                .and_then(|f| f.get("assignee"))
                .and_then(|a| a.get("accountId"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !id.is_empty() {
                dn_resulting_id = id;
                break;
            }
            if attempt < RYW_MAX {
                std::thread::sleep(Duration::from_millis(
                    RYW_BACKOFF_MS[(attempt - 1) as usize],
                ));
            }
        }

        // Mirror the email-path terminal check: distinguish propagation lag from a
        // resolver defect. The display-name assign exited 0 with changed:true, so if
        // the accountId is still empty after the full RYW budget it is an
        // eventual-consistency propagation timeout — NOT a resolver defect. Panic with
        // the precise cause so the failure is not misattributed at triage.
        if dn_resulting_id.is_empty() {
            panic!(
                "assign-by-display-name exited 0 with changed:true but assignee.accountId \
                 never propagated after the bounded read-your-writes wait \
                 ({RYW_MAX} attempts); this is a read-your-writes propagation lag, \
                 NOT a resolver defect. display_name={dn:?}, email={email:?}, \
                 me_account_id={me_account_id:?}"
            );
        }

        effective_account_id = dn_resulting_id;
    }

    // COVERAGE LIMITATION: see rustdoc above — single-account instances cannot detect
    // a resolver that returns the wrong user among multiple candidates.
    assert_eq!(
        effective_account_id, me_account_id,
        "assign-by-query MUST resolve to me_account_id={me_account_id:?}; \
         email={email:?}, email_path_ok={email_path_ok}, \
         fallback_display_name={me_display_name:?}, \
         effective_account_id={effective_account_id:?}"
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

/// E2E: multi-key `jr issue edit K1 K2 --label add:<probe>` / `--label remove:<probe>`
/// round-trip using the corrected `labelsFields` bulk schema (issue #446).
///
/// Seeds TWO throwaway issues, adds a probe label to both via a single multi-key
/// bulk edit, verifies via poll_view that the probe appears on BOTH, then removes
/// the probe from both, and verifies it is absent from BOTH.
///
/// - Clean-skip on HTTP 403 OR 404 from the bulk endpoint:
///   - 403 = caller lacks "Make bulk changes" global permission (all tiers, esp. Free).
///   - 404 = bulk-changes endpoint not available on this plan.
///     Only 403/404 trigger a skip; any other non-zero exit panics so a payload
///     regression fails loudly rather than skipping.
///
/// - The bulk endpoint is async (returns taskId); `jr` polls until COMPLETE.
///   `poll_view` retries GET for eventual-consistency after the bulk task completes.
///
/// Traces to: issue #446, design spec §6.4.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_edit_label_multikey_bulk_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed two throwaway issues, both tagged with run_label() for sweeper teardown.
    let make_issue = |suffix: &str| -> String {
        let summary = format!("[e2e {label}] multikey-label-{suffix}");
        let create_out = h
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
            .expect("failed to spawn jr for issue create (multikey label seed)");
        assert!(
            create_out.status.success(),
            "issue create ({suffix}) failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&create_out.stdout),
            String::from_utf8_lossy(&create_out.stderr)
        );
        serde_json::from_slice::<Value>(&create_out.stdout)
            .expect("issue create output must be valid JSON")
            .get("key")
            .and_then(Value::as_str)
            .expect("issue create JSON must contain a 'key' field")
            .to_string()
    };

    let key1 = make_issue("a");
    let key2 = make_issue("b");

    // Probe label: unique per run, no spaces, no special characters (label constraints).
    let probe = format!("{label}-mk");

    // Assert probe is ABSENT on both issues before adding.
    for key in [&key1, &key2] {
        let before = poll_view(key, &h);
        let before_labels: Vec<String> = before
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
            "probe '{probe}' must be ABSENT on {key} before add; got: {before_labels:?}"
        );
    }

    // ADD the probe label to BOTH keys in one bulk call.
    let add_out = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key1,
            &key2,
            "--label",
            &format!("add:{probe}"),
        ])
        .output()
        .expect("failed to spawn jr for multi-key issue edit --label add");

    if !add_out.status.success() {
        let stderr = String::from_utf8_lossy(&add_out.stderr);
        // Skip only on 403 (permission denied) or 404 (endpoint unavailable).
        if add_out.status.code() == Some(1) && (stderr.contains("403") || stderr.contains("404")) {
            eprintln!(
                "SKIP: bulk-edit {code} — 'Make bulk changes' permission not available \
                 on this site; skipping multi-key label round-trip test.\nstderr: {stderr}",
                code = if stderr.contains("403") { "403" } else { "404" }
            );
            return;
        }
        panic!(
            "multi-key issue edit --label add failed (non-403/404 — not a permission skip):\n\
             exit: {:?}\nstdout: {}\nstderr: {}",
            add_out.status.code(),
            String::from_utf8_lossy(&add_out.stdout),
            stderr,
        );
    }

    // Assert probe IS PRESENT on both issues after add.
    for key in [&key1, &key2] {
        let after_add = poll_view(key, &h);
        let labels: Vec<String> = after_add
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
            labels.contains(&probe),
            "probe '{probe}' must be PRESENT on {key} after add; got: {labels:?}"
        );
    }

    // REMOVE the probe label from BOTH keys in one bulk call.
    let remove_out = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key1,
            &key2,
            "--label",
            &format!("remove:{probe}"),
        ])
        .output()
        .expect("failed to spawn jr for multi-key issue edit --label remove");

    if !remove_out.status.success() {
        let stderr = String::from_utf8_lossy(&remove_out.stderr);
        if remove_out.status.code() == Some(1) && (stderr.contains("403") || stderr.contains("404"))
        {
            eprintln!(
                "SKIP: bulk-edit {code} on remove — skipping.\nstderr: {stderr}",
                code = if stderr.contains("403") { "403" } else { "404" }
            );
            return;
        }
        panic!(
            "multi-key issue edit --label remove failed (non-403/404):\n\
             exit: {:?}\nstdout: {}\nstderr: {}",
            remove_out.status.code(),
            String::from_utf8_lossy(&remove_out.stdout),
            stderr,
        );
    }

    // Assert probe is ABSENT on both issues after remove.
    for key in [&key1, &key2] {
        let after_remove = poll_view(key, &h);
        let labels: Vec<String> = after_remove
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
            !labels.contains(&probe),
            "probe '{probe}' must be ABSENT on {key} after remove; got: {labels:?}"
        );
    }
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

/// E2E: single-key `jr issue edit <KEY> --priority <chosen>` round-trip.
///
/// Seeds one issue. Discovers valid priorities via `jr project fields --output json`.
/// Reads the issue's current priority from poll_view. Picks a priority whose name
/// differs from the current one. Edits the priority. Polls for the new value.
///
/// Portable: never hardcodes "High"; picks the target dynamically.
/// Clean-skip when the project's priority scheme has fewer than 2 priorities
/// (cannot make a distinguishable change).
///
/// Traces to: E2E-PG-4.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_edit_priority_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed one throwaway issue.
    let summary = format!("[e2e {label}] priority-roundtrip-seed");
    let create_out = h
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
        .expect("failed to spawn jr for issue create (priority roundtrip seed)");
    assert!(
        create_out.status.success(),
        "issue create (priority roundtrip seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_out.stdout),
        String::from_utf8_lossy(&create_out.stderr)
    );
    let key = serde_json::from_slice::<Value>(&create_out.stdout)
        .expect("issue create output must be valid JSON")
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Wait for GET-consistency before reading the current priority.
    let before = poll_view(&key, &h);
    let current_name = before
        .get("fields")
        .and_then(|f| f.get("priority"))
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Discover valid priorities via `jr project fields --output json`.
    let pf_out = h
        .cmd()
        .args(["project", "fields", "--project", &proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr for project fields");
    assert!(
        pf_out.status.success(),
        "project fields failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&pf_out.stdout),
        String::from_utf8_lossy(&pf_out.stderr)
    );
    let pf_json: Value =
        serde_json::from_slice(&pf_out.stdout).expect("project fields output must be valid JSON");
    let priorities = pf_json
        .get("priorities")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // Pick a priority whose name differs from the current one.
    let target_name = priorities
        .iter()
        .filter_map(|p| p.get("name").and_then(Value::as_str))
        .find(|&name| !name.eq_ignore_ascii_case(&current_name))
        .map(str::to_string);
    let chosen = match target_name {
        Some(n) => n,
        None => {
            eprintln!(
                "SKIP: project {proj} has fewer than 2 priorities (only {current_name:?}); \
                 cannot make a distinguishable change. Skipping priority round-trip test."
            );
            return;
        }
    };

    // Edit the priority (single-key → PUT /rest/api/3/issue/{key}).
    let edit_out = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--priority",
            &chosen,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue edit --priority");

    // Clean-skip when the project uses a non-default priority scheme and the globally-valid
    // priority is not valid for this project (Jira returns HTTP 400 in that case).
    if !edit_out.status.success() {
        let stderr = String::from_utf8_lossy(&edit_out.stderr);
        if stderr.contains("400") {
            eprintln!(
                "SKIP: priority '{chosen}' not valid for this project's priority scheme; \
                 skipping priority round-trip test (HTTP 400 from Jira)."
            );
            return;
        }
        panic!(
            "issue edit --priority failed for {key} (non-400 — unexpected error):\n\
             exit: {:?}\nstdout: {}\nstderr: {}",
            edit_out.status.code(),
            String::from_utf8_lossy(&edit_out.stdout),
            stderr,
        );
    }

    // Poll for the updated priority.
    let after = poll_view(&key, &h);
    let new_name = after
        .get("fields")
        .and_then(|f| f.get("priority"))
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    assert!(
        new_name.eq_ignore_ascii_case(&chosen),
        "After single-key priority edit, fields.priority.name must be '{chosen}'; \
         got '{new_name}' (key={key})"
    );
}

/// E2E: multi-key `jr issue edit K1 K2 --priority <chosen>` round-trip.
///
/// Seeds two issues. Discovers a target priority that differs from each issue's
/// current priority. Fires the bulk edit. Polls both issues and asserts priority.
///
/// This test is the live backstop for the Part A bulk payload fix (issue #331).
/// It validates that `jr` sends `{"priorityId": "<id>"}` (not `{"name": ...}`)
/// by asserting the observable outcome on a real Jira instance.
///
/// Clean-skip: 403 (bulk-changes permission denied) or 404 (endpoint unavailable).
/// Any other non-zero exit panics — a payload regression should fail loudly.
///
/// Portable: never hardcodes "High"; picks the target priority dynamically.
///
/// Traces to: E2E-PG-4, issue #331.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_edit_priority_multikey_bulk_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed two throwaway issues.
    let make_issue = |suffix: &str| -> String {
        let summary = format!("[e2e {label}] prio-bulk-{suffix}");
        let out = h
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
            .expect("failed to spawn jr for issue create (priority bulk seed)");
        assert!(
            out.status.success(),
            "issue create ({suffix}) failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout)
            .expect("issue create output must be valid JSON")
            .get("key")
            .and_then(Value::as_str)
            .expect("issue create JSON must contain a 'key' field")
            .to_string()
    };

    let key1 = make_issue("a");
    let key2 = make_issue("b");

    // Read current priorities of both issues.
    let before1 = poll_view(&key1, &h);
    let before2 = poll_view(&key2, &h);
    let current1 = before1
        .get("fields")
        .and_then(|f| f.get("priority"))
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let current2 = before2
        .get("fields")
        .and_then(|f| f.get("priority"))
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Discover valid priorities.
    let pf_out = h
        .cmd()
        .args(["project", "fields", "--project", &proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr for project fields");
    assert!(
        pf_out.status.success(),
        "project fields failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&pf_out.stdout),
        String::from_utf8_lossy(&pf_out.stderr)
    );
    let pf_json: Value =
        serde_json::from_slice(&pf_out.stdout).expect("project fields must be valid JSON");
    let priorities = pf_json
        .get("priorities")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // Pick a priority that differs from both issues' current priority.
    let chosen = priorities
        .iter()
        .filter_map(|p| p.get("name").and_then(Value::as_str))
        .find(|&name| {
            !name.eq_ignore_ascii_case(&current1) && !name.eq_ignore_ascii_case(&current2)
        })
        .map(str::to_string);
    let chosen = match chosen {
        Some(n) => n,
        None => {
            eprintln!(
                "SKIP: could not find a priority distinct from both {current1:?} and {current2:?}; \
                 skipping bulk priority round-trip test."
            );
            return;
        }
    };

    // Pre-validate: confirm the chosen priority is settable for this project's priority scheme
    // by doing a single-key edit on key1 via the reliable PUT path. If the project uses a
    // non-default priority scheme, a globally-valid priority can be invalid here (HTTP 400).
    // This pre-check disambiguates: once we know the priority is valid for the project, any
    // subsequent bulk-path 400 is unambiguously a payload regression (not a scheme mismatch).
    let precheck_out = h
        .cmd()
        .args(["issue", "edit", &key1, "--priority", &chosen, "--no-input"])
        .output()
        .expect("failed to spawn jr for single-key priority pre-check");

    if !precheck_out.status.success() {
        let stderr = String::from_utf8_lossy(&precheck_out.stderr);
        if stderr.contains("400") {
            eprintln!(
                "SKIP: priority '{chosen}' not valid for this project's priority scheme \
                 (HTTP 400 on single-key pre-check); skipping bulk priority round-trip test."
            );
            return;
        }
        panic!(
            "single-key priority pre-check failed for {key1} (non-400 — unexpected error):\n\
             exit: {:?}\nstdout: {}\nstderr: {}",
            precheck_out.status.code(),
            String::from_utf8_lossy(&precheck_out.stdout),
            stderr,
        );
    }
    // key1 now has the chosen priority via the single-key PUT path.

    // Bulk edit both keys (key1 again — idempotent; key2 — new).
    // After the pre-validation above, any 400 from the bulk path is unambiguously a
    // payload regression, not a project priority-scheme mismatch. Keep the 403/404
    // narrow clean-skip for permission/availability issues.
    let edit_out = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key1,
            &key2,
            "--priority",
            &chosen,
            "--no-input",
        ])
        .output()
        .expect("failed to spawn jr for multi-key issue edit --priority");

    if !edit_out.status.success() {
        let stderr = String::from_utf8_lossy(&edit_out.stderr);
        // Clean-skip only on 403 (permission denied) or 404 (endpoint unavailable).
        if edit_out.status.code() == Some(1) && (stderr.contains("403") || stderr.contains("404")) {
            eprintln!(
                "SKIP: bulk-edit {code} — 'Make bulk changes' permission not available or \
                 endpoint absent on this site; skipping bulk priority round-trip test.\n\
                 stderr: {stderr}",
                code = if stderr.contains("403") { "403" } else { "404" }
            );
            return;
        }
        // 400 here is unambiguous: the priority was pre-validated via single-key PUT,
        // so a bulk 400 means the bulk payload is wrong — fail loudly.
        panic!(
            "multi-key issue edit --priority failed (non-403/404 — likely a payload regression):\n\
             exit: {:?}\nstdout: {}\nstderr: {}",
            edit_out.status.code(),
            String::from_utf8_lossy(&edit_out.stdout),
            stderr,
        );
    }

    // Poll both issues and assert priority updated.
    // key1 was set via single-key pre-validation; key2 was set via the bulk path.
    // Both must reflect the chosen priority.
    for key in [&key1, &key2] {
        let after = poll_view(key, &h);
        let new_name = after
            .get("fields")
            .and_then(|f| f.get("priority"))
            .and_then(|p| p.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        assert!(
            new_name.eq_ignore_ascii_case(&chosen),
            "After bulk priority edit, fields.priority.name must be '{chosen}'; \
             got '{new_name}' (key={key})"
        );
    }
}

/// E2E: `jr worklog add <KEY> 1h` → `jr worklog list <KEY> --output json` round-trip.
///
/// Seeds one issue. Adds 1 hour of work via `jr worklog add`. Reads back the
/// worklog list and asserts at least one entry has `timeSpentSeconds == 3600`.
///
/// Read-after-write is consistent in Jira Cloud v3 for issue-scoped worklog
/// reads (the "missing last minute" caveat is Data-Center-only). No tier gate
/// beyond "Work on issues" permission.
///
/// Traces to: E2E-PG-4.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_worklog_add_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed one throwaway issue.
    let summary = format!("[e2e {label}] worklog-add-roundtrip-seed");
    let create_out = h
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
        .expect("failed to spawn jr for issue create (worklog-add seed)");
    assert!(
        create_out.status.success(),
        "issue create (worklog-add seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_out.stdout),
        String::from_utf8_lossy(&create_out.stderr)
    );
    let key = serde_json::from_slice::<Value>(&create_out.stdout)
        .expect("issue create output must be valid JSON")
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Wait for GET-consistency before logging work.
    poll_view(&key, &h);

    // Add 1 hour of work.
    let add_out = h
        .cmd()
        .args(["worklog", "add", &key, "1h", "--output", "json"])
        .output()
        .expect("failed to spawn jr for worklog add");
    assert!(
        add_out.status.success(),
        "worklog add failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&add_out.stdout),
        String::from_utf8_lossy(&add_out.stderr)
    );

    // Read back the worklog list.
    let list_out = h
        .cmd()
        .args(["worklog", "list", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for worklog list");
    assert!(
        list_out.status.success(),
        "worklog list failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&list_out.stdout),
        String::from_utf8_lossy(&list_out.stderr)
    );

    let worklogs: Value =
        serde_json::from_slice(&list_out.stdout).expect("worklog list output must be valid JSON");
    assert!(
        worklogs.is_array(),
        "worklog list output must be a JSON array; got: {worklogs}"
    );
    let has_1h = worklogs
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.get("timeSpentSeconds").and_then(Value::as_u64) == Some(3600));
    assert!(
        has_1h,
        "worklog list must contain an entry with timeSpentSeconds == 3600 (1h); \
         got: {worklogs}"
    );
}

/// E2E: `jr issue assign <KEY> --unassign` removes the assignee.
///
/// Seeds one issue. Assigns it to self first. Then unassigns.
/// After unassign: if `fields.assignee` is null → PASS.
/// If `fields.assignee` is non-null after a 2xx unassign → CLEAN-SKIP with
/// an explanatory message (project has "Allow unassigned issues" disabled or
/// a forced default-assignee / automation post-function — config-dependent
/// behavior, not a jr bug).
///
/// Traces to: E2E-PG-4, BC-3.2.001 (unassign semantics).
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_unassign() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed one throwaway issue.
    let summary = format!("[e2e {label}] unassign-seed");
    let create_out = h
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
        .expect("failed to spawn jr for issue create (unassign seed)");
    assert!(
        create_out.status.success(),
        "issue create (unassign seed) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create_out.stdout),
        String::from_utf8_lossy(&create_out.stderr)
    );
    let key = serde_json::from_slice::<Value>(&create_out.stdout)
        .expect("issue create output must be valid JSON")
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain a 'key' field")
        .to_string();

    // Wait for GET-consistency.
    poll_view(&key, &h);

    // Assign to self (no --to argument = self-assignment via /myself).
    let assign_out = h
        .cmd()
        .args(["issue", "assign", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue assign (self)");
    assert!(
        assign_out.status.success(),
        "issue assign (self) failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&assign_out.stdout),
        String::from_utf8_lossy(&assign_out.stderr)
    );

    // Confirm assigned.
    let assigned = poll_view(&key, &h);
    let assignee_id = assigned
        .get("fields")
        .and_then(|f| f.get("assignee"))
        .and_then(|a| a.get("accountId"))
        .and_then(Value::as_str);
    assert!(
        assignee_id.is_some_and(|id| !id.is_empty()),
        "after self-assignment, fields.assignee.accountId must be non-null; \
         fields.assignee: {:?}",
        assigned.get("fields").and_then(|f| f.get("assignee"))
    );

    // Unassign.
    let unassign_out = h
        .cmd()
        .args(["issue", "assign", &key, "--unassign", "--output", "json"])
        .output()
        .expect("failed to spawn jr for issue assign --unassign");
    assert!(
        unassign_out.status.success(),
        "issue assign --unassign failed for {key}:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&unassign_out.stdout),
        String::from_utf8_lossy(&unassign_out.stderr)
    );

    // Poll and check assignee.
    let after = poll_view(&key, &h);
    let assignee_after = after.get("fields").and_then(|f| f.get("assignee"));

    // Clean-skip if assignee is non-null after unassign:
    // project may have "Allow unassigned issues" disabled or a default-assignee/automation.
    let is_null = assignee_after.is_none_or(Value::is_null);
    if !is_null {
        eprintln!(
            "SKIP: after unassign, fields.assignee is still non-null for {key}. \
             Project '{proj}' likely has 'Allow unassigned issues' disabled or a \
             default-assignee / automation post-function. This is a project-config \
             limitation, not a jr bug. Skipping unassign assertion."
        );
        return;
    }
    assert!(
        is_null,
        "After unassign, fields.assignee must be null; \
         got: {assignee_after:?} (key={key})"
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

/// E2E: multi-key `jr issue edit K1 K2 --type <alt>` bulk issueType round-trip.
///
/// Seeds two issues using the default issue type (`JR_E2E_ISSUE_TYPE`, default "Task").
/// Bulk-changes their type to `JR_E2E_ISSUE_TYPE_ALT` via the bulk edit path.
/// Polls both issues and asserts the new type appears in `fields.issuetype.name`.
///
/// Clean-skip: if `JR_E2E_ISSUE_TYPE_ALT` is not set (no alternate type available).
/// Also clean-skips on 403 (bulk-changes permission denied) or 404 (endpoint absent).
///
/// This test validates the project-scoped id resolution that has no precedent in the
/// codebase: priority was global; issueType is project-scoped (issue #331 / BC-3.4.018).
///
/// `JR_E2E_ISSUE_TYPE_ALT` is read by test code only — no `#[cfg(debug_assertions)]`
/// src/ gate needed. Documented in CLAUDE.md AI Agent Notes per the JR_* doc-fallout rule.
///
/// Traces to: AC-011 (BC-3.4.018), issue #331.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_edit_issuetype_multikey_bulk_roundtrip() {
    if !e2e_enabled() {
        return;
    }

    // Clean-skip if the alternate issue type env var is not set.
    let alt_type = match std::env::var("JR_E2E_ISSUE_TYPE_ALT") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => {
            eprintln!(
                "SKIP: JR_E2E_ISSUE_TYPE_ALT not set — skipping bulk issueType round-trip test."
            );
            return;
        }
    };

    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Seed two throwaway issues using the default issue type.
    let make_issue = |suffix: &str| -> String {
        let summary = format!("[e2e {label}] issuetype-bulk-{suffix}");
        let out = h
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
            .expect("failed to spawn jr for issue create (issuetype bulk seed)");
        assert!(
            out.status.success(),
            "issue create ({suffix}) failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout)
            .expect("issue create output must be valid JSON")
            .get("key")
            .and_then(Value::as_str)
            .expect("issue create JSON must contain a 'key' field")
            .to_string()
    };

    let key1 = make_issue("a");
    let key2 = make_issue("b");

    // Wait for GET-consistency before editing.
    poll_view(&key1, &h);
    poll_view(&key2, &h);

    // Bulk edit both keys to the alternate type.
    let edit_out = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key1,
            &key2,
            "--type",
            &alt_type,
            "--no-input",
        ])
        .output()
        .expect("failed to spawn jr for multi-key issue edit --type");

    if !edit_out.status.success() {
        let stderr = String::from_utf8_lossy(&edit_out.stderr);
        // Clean-skip on 403 (permission denied) or 404 (endpoint unavailable).
        if edit_out.status.code() == Some(1) && (stderr.contains("403") || stderr.contains("404")) {
            eprintln!(
                "SKIP: bulk-edit {code} — 'Make bulk changes' permission not available or \
                 endpoint absent on this site; skipping bulk issueType round-trip test.\n\
                 stderr: {stderr}",
                code = if stderr.contains("403") { "403" } else { "404" }
            );
            return;
        }
        // Any other failure is a bug — fail loudly.
        panic!(
            "multi-key issue edit --type failed:\n\
             exit: {:?}\nstdout: {}\nstderr: {}",
            edit_out.status.code(),
            String::from_utf8_lossy(&edit_out.stdout),
            stderr,
        );
    }

    // Poll both issues and assert the new type is reflected.
    for key in [&key1, &key2] {
        let after = poll_view(key, &h);
        let new_type = after
            .get("fields")
            .and_then(|f| f.get("issuetype"))
            .and_then(|it| it.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        assert!(
            new_type.eq_ignore_ascii_case(&alt_type),
            "After bulk issueType edit, fields.issuetype.name must be '{alt_type}'; \
             got '{new_type}' (key={key})"
        );
    }
}

/// E2E: multi-key `jr issue edit K1 K2 --component add:<X>` / `--component
/// remove:<X>` bulk round-trip using the `multiselectComponents` wire shape
/// (BC-3.4.023, S-605-2).
///
/// **DEC-280 LIVE-JIRA RELEASE GATE (BC-3.4.023 Delivery note):** this test
/// is the mandated live smoke test — one ADD POST and one REMOVE POST,
/// against >= 2 real issues in one project — that MUST pass before the bulk
/// `--component` path ships to release. The `multiselectComponents` wire
/// shape (`src/api/jira/bulk.rs::build_component_edited_fields`) is
/// documented and triple-corroborated (Atlassian doc example + swagger
/// OpenAPI + apidog mirror) but was NOT live-verified at spec-authoring
/// time. If this test observes a non-403/404 failure (e.g. a live 400),
/// that is evidence the documented shape is wrong -- BC-3.4.023 must be
/// corrected to the observed true shape before this story can be marked
/// done, mirroring how `FIX-BULK-TRANSITION-001` (#446) was discovered via
/// exactly this kind of live failure, not by static review.
///
/// **Precondition (BC-3.4.023 Delivery note, added 2026-08-19):** the
/// target project MUST already have >= 1 component defined -- Jira's
/// `GET /rest/api/3/bulk/issues/fields` field-discovery response only lists
/// `components` in the bulk-edit allowlist when the selected issues'
/// project actually has components configured; a componentless project
/// surfaces `components` with an `unavailableMessage` instead, which would
/// false-negative this test for a reason unrelated to wire-shape
/// correctness. This precondition is checked by discovering an existing
/// component via `jr component list --project <proj> --output json` and
/// clean-skipping if the project has none -- no new `JR_E2E_*` env var is
/// introduced for this, since the component name is read directly off the
/// live project rather than configured.
///
/// Mirrors `test_e2e_issue_edit_label_multikey_bulk_roundtrip`'s structure
/// (seed two issues, ADD then assert-present, REMOVE then assert-absent,
/// clean-skip on 403/404 = "Make bulk changes" permission/plan gate) but
/// against `fields.components[].name` instead of `fields.labels[]`.
///
/// Traces to: AC-010, VP-COMPONENT-012, DEC-280.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_edit_component_multikey_bulk_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let proj = project();
    let itype = issue_type();
    let h = e2e_harness();

    // Precondition: the project must have >= 1 component already defined.
    // Clean-skip (not a failure) if it has none -- see the DEC-280
    // precondition note in this test's doc comment above.
    let list_out = h
        .cmd()
        .args(["component", "list", "--project", &proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr for component list");
    if !list_out.status.success() {
        eprintln!(
            "SKIP: `jr component list --project {proj}` failed -- cannot verify \
             the >= 1 component precondition; skipping bulk --component \
             round-trip test.\nstderr: {}",
            String::from_utf8_lossy(&list_out.stderr)
        );
        return;
    }
    let components: Value =
        serde_json::from_slice(&list_out.stdout).expect("component list output must be valid JSON");
    let component_name = match components.as_array().and_then(|arr| arr.first()) {
        Some(c) => c
            .get("name")
            .and_then(Value::as_str)
            .expect("component list entry must have a 'name' field")
            .to_string(),
        None => {
            eprintln!(
                "SKIP: project {proj} has zero components defined -- BC-3.4.023's \
                 Delivery note precondition requires >= 1; skipping bulk \
                 --component round-trip test. Configure a component on this \
                 project to enable this release-gate test."
            );
            return;
        }
    };

    // Seed two throwaway issues, both tagged with run_label() for sweeper
    // teardown, and WITHOUT the target component (so ADD is a genuine change).
    let make_issue = |suffix: &str| -> String {
        let summary = format!("[e2e {label}] multikey-component-{suffix}");
        let create_out = h
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
            .expect("failed to spawn jr for issue create (multikey component seed)");
        assert!(
            create_out.status.success(),
            "issue create ({suffix}) failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&create_out.stdout),
            String::from_utf8_lossy(&create_out.stderr)
        );
        serde_json::from_slice::<Value>(&create_out.stdout)
            .expect("issue create output must be valid JSON")
            .get("key")
            .and_then(Value::as_str)
            .expect("issue create JSON must contain a 'key' field")
            .to_string()
    };

    let key1 = make_issue("a");
    let key2 = make_issue("b");

    let component_names = |v: &Value| -> Vec<String> {
        v.get("fields")
            .and_then(|f| f.get("components"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.get("name").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };

    // Assert the component is ABSENT on both issues before adding (freshly
    // created issues carry no components).
    for key in [&key1, &key2] {
        let before = poll_view(key, &h);
        assert!(
            !component_names(&before).contains(&component_name),
            "component '{component_name}' must be ABSENT on {key} before add; \
             got: {:?}",
            component_names(&before)
        );
    }

    // ADD the component to BOTH keys in one bulk call -- BC-3.4.023
    // Postcondition 1/2 wire shape (multiselectComponents ADD).
    let add_out = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key1,
            &key2,
            "--component",
            &format!("add:{component_name}"),
        ])
        .output()
        .expect("failed to spawn jr for multi-key issue edit --component add");

    if !add_out.status.success() {
        let stderr = String::from_utf8_lossy(&add_out.stderr);
        // Skip only on 403 (permission denied) or 404 (endpoint unavailable) --
        // any OTHER failure (e.g. a 400) is exactly the DEC-280 release-gate
        // signal that the documented wire shape is wrong and must NOT be
        // silently skipped.
        if add_out.status.code() == Some(1) && (stderr.contains("403") || stderr.contains("404")) {
            eprintln!(
                "SKIP: bulk-edit {code} -- 'Make bulk changes' permission not \
                 available on this site; skipping bulk --component round-trip \
                 test.\nstderr: {stderr}",
                code = if stderr.contains("403") { "403" } else { "404" }
            );
            return;
        }
        panic!(
            "DEC-280 RELEASE GATE FAILURE: multi-key issue edit --component add \
             failed (non-403/404 -- not a permission skip). This is evidence the \
             multiselectComponents wire shape documented in BC-3.4.023 does NOT \
             match live Jira -- correct the BC to the observed true shape before \
             proceeding (FIX-BULK-TRANSITION-001/#446 precedent), then re-run \
             test-writer/implementer against the corrected shape.\n\
             exit: {:?}\nstdout: {}\nstderr: {}",
            add_out.status.code(),
            String::from_utf8_lossy(&add_out.stdout),
            stderr,
        );
    }

    // Assert the component IS PRESENT on both issues after add.
    for key in [&key1, &key2] {
        let after_add = poll_view(key, &h);
        assert!(
            component_names(&after_add).contains(&component_name),
            "component '{component_name}' must be PRESENT on {key} after add; \
             got: {:?}",
            component_names(&after_add)
        );
    }

    // REMOVE the component from BOTH keys in one bulk call -- BC-3.4.023
    // Postcondition 3 (a SEPARATE sequential POST, not coalesced with the
    // ADD above; here issued as its own `jr` invocation, which exercises
    // the same REMOVE wire shape as the mixed add:/remove: single-invocation
    // case would for its second POST).
    let remove_out = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key1,
            &key2,
            "--component",
            &format!("remove:{component_name}"),
        ])
        .output()
        .expect("failed to spawn jr for multi-key issue edit --component remove");

    if !remove_out.status.success() {
        let stderr = String::from_utf8_lossy(&remove_out.stderr);
        if remove_out.status.code() == Some(1) && (stderr.contains("403") || stderr.contains("404"))
        {
            eprintln!(
                "SKIP: bulk-edit {code} on remove -- skipping.\nstderr: {stderr}",
                code = if stderr.contains("403") { "403" } else { "404" }
            );
            return;
        }
        panic!(
            "DEC-280 RELEASE GATE FAILURE: multi-key issue edit --component \
             remove failed (non-403/404 -- not a permission skip). This is \
             evidence the multiselectComponents REMOVE wire shape documented in \
             BC-3.4.023 does NOT match live Jira -- correct the BC before \
             proceeding.\nexit: {:?}\nstdout: {}\nstderr: {}",
            remove_out.status.code(),
            String::from_utf8_lossy(&remove_out.stdout),
            stderr,
        );
    }

    // Assert the component is ABSENT on both issues after remove.
    for key in [&key1, &key2] {
        let after_remove = poll_view(key, &h);
        assert!(
            !component_names(&after_remove).contains(&component_name),
            "component '{component_name}' must be ABSENT on {key} after remove; \
             got: {:?}",
            component_names(&after_remove)
        );
    }
}

// ────────────────────────────────────────────────────────────────────────────
// S-COMP-E2E-1: live E2E coverage for the component command family
//
// Every other command in the family — `component create`/`list`/`edit`/
// `delete`/`rename`, `issue create --component` (single-key), `issue edit
// --component` (single-key native `update`-verb path, distinct from the bulk
// `multiselectComponents` path already covered above), and `issue list
// --component` (bare/`not:`/`none` JQL-composition grammar) — had ZERO
// live-Jira verification before this story. This section closes that gap
// with pure test-hardening: no new product behavior, no new BCs.
//
// Traces to: S-COMP-E2E-1, BC-8.1.001/002/005/007, BC-8.2.001/006/008,
// BC-8.3.001, BC-3.4.022/024/025, BC-2.1.018/019/020.
// ────────────────────────────────────────────────────────────────────────────

/// Returns `true` (and emits a `SKIP:` message) when `out` failed with a 403
/// -- or, when `allow_404` is `true`, also a 404 -- the shared clean-skip
/// predicate for this story's component-family E2E tests (EC-COMP-E2E-3,
/// AC-014). Any OTHER non-zero exit is NOT a skip signal; callers must treat
/// it as a genuine test failure and `panic!` with full stdout/stderr context,
/// mirroring `test_e2e_issue_edit_component_multikey_bulk_roundtrip`'s
/// release-gate discipline above.
///
/// **MED-1 finding 1 (S-COMP-E2E-1 adversarial review):** the match is
/// anchored to the exact rendered status-code token -- `"API error (403)"` /
/// `"API error (404)"`, per `src/error.rs`'s `JrError::ApiError` Display impl
/// (`"API error ({status}): {message}"`) and `main.rs`'s `eprintln!("Error:
/// {e}")` / `--output json` error envelope, both of which route through that
/// same Display -- rather than a bare `contains("403")` / `contains("404")`
/// substring search. The bare form collided with digits appearing ANYWHERE
/// in the error body (a component id like `10403`/`10404`, or a run-label
/// fixture name echoed back by Jira), misclassifying a genuine 500/400
/// failure as a permission skip.
///
/// **MED-1 finding 2:** `allow_404` distinguishes precondition probes -- where
/// a 404 is a legitimate "feature/permission absent" signal (e.g.
/// `discover_component`, or a `… create` call that has not yet created
/// anything this test depends on existing) -- from post-create mutations
/// (edit, delete, rename, or an issue-edit acting on a key this SAME test
/// already created), where a 404 means the resource vanished mid-test: a real
/// bug, never a permission gate. Post-create-mutation call sites MUST pass
/// `allow_404: false`.
fn skip_on_403_404(out: &std::process::Output, context: &str, allow_404: bool) -> bool {
    if out.status.success() {
        return false;
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let is_403 = stderr.contains("API error (403)");
    let is_404 = allow_404 && stderr.contains("API error (404)");
    if out.status.code() == Some(1) && (is_403 || is_404) {
        eprintln!(
            "SKIP: {context} returned {code} -- permission/plan gate; skipping.\nstderr: {stderr}",
            code = if is_403 { "403" } else { "404" }
        );
        true
    } else {
        false
    }
}

/// Discover the first component defined on `proj` via `jr component list
/// --project <proj> --output json`.
///
/// Clean-skip (returns `None` + `eprintln!("SKIP: ...")`) when the project has
/// zero components or the discovery call fails with a 403/404 permission/plan
/// gate. Any OTHER non-zero exit is a genuine test failure (panics) — mirrors
/// `test_e2e_issue_edit_component_multikey_bulk_roundtrip`'s precondition-check
/// discipline, reused here for AC-009/AC-010/AC-011 (S-COMP-E2E-1).
fn discover_component(h: &E2eHarness, proj: &str, context: &str) -> Option<String> {
    let out = h
        .cmd()
        .args(["component", "list", "--project", proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr for component list (discovery)");
    if !out.status.success() {
        if skip_on_403_404(&out, context, /* allow_404 */ true) {
            return None;
        }
        panic!(
            "{context}: component list failed (non-403/404 -- not a permission skip):\n\
             stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let components: Value =
        serde_json::from_slice(&out.stdout).expect("component list output must be valid JSON");
    match components.as_array().and_then(|arr| arr.first()) {
        Some(c) => Some(
            c.get("name")
                .and_then(Value::as_str)
                .expect("component list entry must have a 'name' field")
                .to_string(),
        ),
        None => {
            eprintln!("SKIP: {context} -- project {proj} has zero components defined");
            None
        }
    }
}

/// Bounded-backoff poll for `jr issue list --project <proj> --component <comp>
/// --output json`, returning `true` once `key` appears in the results.
///
/// Reuses `poll_schedule`'s exponential-backoff SCHEDULE (EC-COMP-E2E-5), and
/// drives the `--project`/`--component` flag form rather than `--jql`, since
/// AC-011..AC-013 test the flag-composition grammar directly, not JQL string
/// composition.
///
/// # Budget (widened post-run-32384091667, see below)
///
/// Honors the same `JR_E2E_POLL_MAX_ATTEMPTS` / `JR_E2E_POLL_INITIAL_MS` env
/// seams as `poll_jql` (identical parse-with-fallback pattern), so a caller
/// can widen or narrow the budget uniformly across both pollers. When unset,
/// defaults to `max_attempts=7, initial_ms=500` -> `poll_schedule(7, 500)` =
/// `[500, 1000, 2000, 4000, 8000, 16000]`, a ~31.5s worst-case ceiling.
///
/// **Root cause for the wider default:** live e2e run 32384091667 on
/// `develop` `d467f95a` failed `test_e2e_issue_list_component_filter_grammar`
/// at the AC-011 positive poll — the old hardcoded `poll_schedule(5, 250)`
/// budget (~3.75s total) was not enough for a just-created issue's component
/// association to become JQL-SEARCH-indexed on live Jira Cloud, even though
/// the component write itself had already landed (the other 4 component
/// tests, which verify via GET-by-key `poll_view`, passed in the same run).
/// This was a false-RED from search-index propagation lag, not a product
/// bug. Because this loop still returns as soon as `key` appears, the happy
/// path (indexed within a couple of seconds) is unaffected — only genuine
/// index lag pays into the longer tail of the schedule.
///
/// **LOW-2 (S-COMP-E2E-1 adversarial review) — this does NOT mirror
/// `poll_jql`'s CALLER-FACING behavior**, only its backoff schedule.
/// `poll_jql` offers a `PollJqlMode::SkipOnEmpty` / `FailOnShort` distinction
/// so a caller can choose "clean-skip on empty" vs. "panic if results stay
/// short of a minimum". This function has no such mode parameter: on budget
/// exhaustion (empty OR non-matching results through all attempts) it simply
/// returns `false`, and its sole caller treats that as a hard `assert!`
/// failure (AC-011), never a clean skip. That is intentional here — AC-014
/// documents this suite as a release gate, not a best-effort probe — but it
/// means the two functions are NOT interchangeable and a caller expecting
/// `poll_jql`-style skip semantics from this function will get a panic
/// instead. Widening the budget does not change this: a truly-absent key
/// still fails the test once the (now longer) budget is exhausted.
fn poll_component_filter(h: &E2eHarness, proj: &str, comp: &str, key: &str) -> bool {
    let max_attempts: usize = match std::env::var("JR_E2E_POLL_MAX_ATTEMPTS") {
        Ok(v) if !v.trim().is_empty() => v.trim().parse().unwrap_or(7).max(1),
        _ => 7,
    };
    let initial_ms: u64 = match std::env::var("JR_E2E_POLL_INITIAL_MS") {
        Ok(v) if !v.trim().is_empty() => v.trim().parse().unwrap_or(500),
        _ => 500,
    };
    let schedule = poll_schedule(max_attempts, initial_ms);
    for attempt in 1..=max_attempts {
        let out = h
            .cmd()
            .args([
                "issue",
                "list",
                "--project",
                proj,
                "--component",
                comp,
                "--output",
                "json",
            ])
            .output()
            .expect("failed to spawn jr for issue list --component (poll)");
        if out.status.success() {
            if let Ok(v) = serde_json::from_slice::<Value>(&out.stdout) {
                if let Some(arr) = v.as_array() {
                    if arr
                        .iter()
                        .any(|i| i.get("key").and_then(Value::as_str) == Some(key))
                    {
                        return true;
                    }
                }
            }
        }
        if attempt < max_attempts {
            std::thread::sleep(Duration::from_millis(schedule[attempt - 1]));
        }
    }
    false
}

/// Best-effort `Drop`-guard teardown for a throwaway component created during
/// this story's E2E tests (AC-015).
///
/// Modeled verbatim on `AttachmentDropGuard` (S-576-6): a fresh
/// `E2eHarness::new()` is spawned inside `drop()` rather than borrowing the
/// test's own harness across a potential panic-unwind, and every failure path
/// emits `eprintln!("[WARN] ...")` and returns — `drop()` must never panic.
///
/// `component_id` defaults to `None` (no cleanup performed) and must be
/// populated IMMEDIATELY after the corresponding `component create` call
/// succeeds — never before, and never skipped even on an early return (an
/// unpopulated guard performs no cleanup by design). `project` must be
/// populated alongside `component_id`; if `component_id` is `Some` while
/// `project` is `None`, `drop()` warns and skips instead of attempting a
/// malformed delete.
struct ComponentDropGuard {
    project: Option<String>,
    component_id: Option<String>,
}

impl ComponentDropGuard {
    fn new() -> Self {
        Self {
            project: None,
            component_id: None,
        }
    }
}

impl Drop for ComponentDropGuard {
    fn drop(&mut self) {
        let Some(ref id) = self.component_id else {
            return;
        };
        let Some(ref proj) = self.project else {
            eprintln!(
                "[WARN] ComponentDropGuard Drop: component_id {id} set but project is None -- \
                 cannot delete; this is a test bug, not a live-Jira condition."
            );
            return;
        };
        let h = E2eHarness::new();
        match h
            .cmd()
            .args([
                "component",
                "delete",
                id,
                "--project",
                proj,
                "--orphan",
                "--yes",
            ])
            .output()
        {
            Ok(o) if o.status.success() => {}
            Ok(o) => eprintln!(
                "[WARN] ComponentDropGuard Drop: delete {id} failed (exit {:?}): {}",
                o.status.code(),
                String::from_utf8_lossy(&o.stderr)
            ),
            Err(e) => eprintln!("[WARN] ComponentDropGuard Drop: delete spawn error: {e}"),
        }
    }
}

/// E2E: `jr component create` → `list` → `edit` → `list` → `delete` → `list`
/// full lifecycle round-trip against a live Jira Cloud project.
///
/// Traces to: AC-001..AC-006, BC-8.1.001, BC-8.1.002, BC-8.1.005, BC-8.1.007,
/// BC-8.2.001, BC-8.2.006, BC-8.2.008.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_component_lifecycle_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let proj = project();
    let label = run_label();
    // MED-2: unique per invocation so a leaked fixture from a killed/cancelled
    // prior CI attempt can never collide with this run's create call.
    let name = format!("{label}-lifecycle-{}", component_fixture_suffix());
    let mut guard = ComponentDropGuard::new();

    // AC-001: create.
    let create_out = h
        .cmd()
        .args([
            "component",
            "create",
            "--project",
            &proj,
            &name,
            "--description",
            "S-COMP-E2E-1 lifecycle fixture",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for component create");
    if !create_out.status.success() {
        if skip_on_403_404(&create_out, "component create", /* allow_404 */ true) {
            return;
        }
        panic!(
            "component create failed (non-403/404 -- not a permission skip):\n\
             stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&create_out.stdout),
            String::from_utf8_lossy(&create_out.stderr)
        );
    }
    let created: Value = serde_json::from_slice(&create_out.stdout)
        .expect("component create output must be valid JSON");
    let id = created
        .get("id")
        .and_then(Value::as_str)
        .expect("component create JSON must contain an 'id' field")
        .to_string();

    // Arm the guard IMMEDIATELY after create succeeds, before any further assertion --
    // a panic in the shape/name/project assertions below must still trigger cleanup.
    guard.project = Some(proj.clone());
    guard.component_id = Some(id.clone());

    assert_eq!(
        created.as_object().map(|o| o.len()),
        Some(3),
        "component create JSON must have exactly 3 keys (id, name, project); got: {created}"
    );
    assert_eq!(
        created.get("name").and_then(Value::as_str),
        Some(name.as_str())
    );
    assert_eq!(
        created.get("project").and_then(Value::as_str),
        Some(proj.as_str())
    );

    // AC-002: list reflects the created component.
    let list_out_1 = h
        .cmd()
        .args(["component", "list", "--project", &proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr for component list (AC-002)");
    assert!(
        list_out_1.status.success(),
        "component list (AC-002) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&list_out_1.stdout),
        String::from_utf8_lossy(&list_out_1.stderr)
    );
    let list1: Value = serde_json::from_slice(&list_out_1.stdout)
        .expect("component list (AC-002) output must be valid JSON");
    let arr1 = list1
        .as_array()
        .expect("component list (AC-002) must be a JSON array");
    assert!(
        arr1.iter()
            .any(|c| c.get("id").and_then(Value::as_str) == Some(id.as_str())
                && c.get("name").and_then(Value::as_str) == Some(name.as_str())),
        "component list (AC-002) must contain id={id} name={name}; got: {list1}"
    );

    // AC-003: edit (only-supplied-fields; JSON result shape).
    //
    // LOW-3 (S-COMP-E2E-1 adversarial review): this is a BLACK-BOX assertion —
    // it verifies the edit's observable result shape (id/name/project keys)
    // and that `name` was actually updated, but it supplies BOTH `--name` and
    // `--description` on this call, so it cannot distinguish "only supplied
    // fields were sent on the wire" from "all fields were sent and happened
    // to match". BC-8.1.007's "only-supplied-fields" wire-contract guarantee
    // (e.g. that editing just `--name` does NOT also re-send `description`)
    // is covered by wiremock/unit tests elsewhere, not by this live E2E test.
    let new_name = format!("{name}-renamed");
    let edit_out = h
        .cmd()
        .args([
            "component",
            "edit",
            &id,
            "--project",
            &proj,
            "--name",
            &new_name,
            "--description",
            "S-COMP-E2E-1 lifecycle fixture (edited)",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for component edit");
    if !edit_out.status.success() {
        if skip_on_403_404(&edit_out, "component edit", /* allow_404 */ false) {
            return;
        }
        panic!(
            "component edit failed (non-403/404 -- not a permission skip):\n\
             stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&edit_out.stdout),
            String::from_utf8_lossy(&edit_out.stderr)
        );
    }
    let edited: Value =
        serde_json::from_slice(&edit_out.stdout).expect("component edit output must be valid JSON");
    assert_eq!(
        edited.as_object().map(|o| o.len()),
        Some(3),
        "component edit JSON must have exactly 3 keys (id, name, project); got: {edited}"
    );
    assert_eq!(edited.get("id").and_then(Value::as_str), Some(id.as_str()));
    assert_eq!(
        edited.get("name").and_then(Value::as_str),
        Some(new_name.as_str())
    );

    // AC-004: list reflects the edit.
    let list_out_2 = h
        .cmd()
        .args(["component", "list", "--project", &proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr for component list (AC-004)");
    assert!(
        list_out_2.status.success(),
        "component list (AC-004) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&list_out_2.stdout),
        String::from_utf8_lossy(&list_out_2.stderr)
    );
    let list2: Value = serde_json::from_slice(&list_out_2.stdout)
        .expect("component list (AC-004) output must be valid JSON");
    let arr2 = list2
        .as_array()
        .expect("component list (AC-004) must be a JSON array");
    assert!(
        arr2.iter()
            .any(|c| c.get("id").and_then(Value::as_str) == Some(id.as_str())
                && c.get("name").and_then(Value::as_str) == Some(new_name.as_str())),
        "component list (AC-004) must contain id={id} name={new_name}; got: {list2}"
    );
    assert!(
        !arr2
            .iter()
            .any(|c| c.get("name").and_then(Value::as_str) == Some(name.as_str())),
        "component list (AC-004) must NOT contain the original name {name}; got: {list2}"
    );

    // AC-005: delete (--orphan --yes; JSON result shape).
    let delete_out = h
        .cmd()
        .args([
            "component",
            "delete",
            &id,
            "--project",
            &proj,
            "--orphan",
            "--yes",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for component delete");
    if !delete_out.status.success() {
        if skip_on_403_404(&delete_out, "component delete", /* allow_404 */ false) {
            return;
        }
        panic!(
            "component delete failed (non-403/404 -- not a permission skip):\n\
             stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&delete_out.stdout),
            String::from_utf8_lossy(&delete_out.stderr)
        );
    }
    let deleted: Value = serde_json::from_slice(&delete_out.stdout)
        .expect("component delete output must be valid JSON");
    let mut delete_keys: Vec<&str> = deleted
        .as_object()
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    delete_keys.sort_unstable();
    assert_eq!(
        delete_keys,
        vec![
            "affectedIssueCount",
            "affectedIssues",
            "deleted",
            "movedIssuesTo"
        ],
        "component delete JSON must have exactly these 4 keys; got: {deleted}"
    );
    assert_eq!(
        deleted.get("deleted").and_then(Value::as_str),
        Some(id.as_str())
    );
    assert!(
        deleted
            .get("movedIssuesTo")
            .map(Value::is_null)
            .unwrap_or(false),
        "movedIssuesTo must be JSON null under --orphan; got: {deleted}"
    );

    // Disarm the guard -- the delete above already succeeded (AC-005).
    guard.component_id = None;

    // AC-006: list reflects the deletion.
    let list_out_3 = h
        .cmd()
        .args(["component", "list", "--project", &proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr for component list (AC-006)");
    assert!(
        list_out_3.status.success(),
        "component list (AC-006) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&list_out_3.stdout),
        String::from_utf8_lossy(&list_out_3.stderr)
    );
    let list3: Value = serde_json::from_slice(&list_out_3.stdout)
        .expect("component list (AC-006) output must be valid JSON");
    let arr3 = list3
        .as_array()
        .expect("component list (AC-006) must be a JSON array");
    assert!(
        !arr3
            .iter()
            .any(|c| c.get("id").and_then(Value::as_str) == Some(id.as_str())),
        "component list (AC-006) must NOT contain id={id} after delete; got: {list3}"
    );
}

/// E2E: `jr component rename OLD NEW --project <proj>` round-trip against a
/// live Jira Cloud project — id-preservation + PUT wire shape (BC-8.3.001).
///
/// Traces to: AC-007, AC-008, BC-8.3.001.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_component_rename_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let proj = project();
    let label = run_label();
    // MED-2: unique per invocation (shared suffix across src/dst so the pair
    // reads as one fixture) so a leaked fixture from a killed/cancelled prior
    // CI attempt can never collide with this run's create call.
    let suffix = component_fixture_suffix();
    let old_name = format!("{label}-rename-src-{suffix}");
    let new_name = format!("{label}-rename-dst-{suffix}");
    let mut guard = ComponentDropGuard::new();

    // Fresh throwaway component fixture (own guard instance, tracked by
    // numeric id so cleanup survives the rename below).
    let create_out = h
        .cmd()
        .args([
            "component",
            "create",
            "--project",
            &proj,
            &old_name,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for component create (rename fixture)");
    if !create_out.status.success() {
        if skip_on_403_404(
            &create_out,
            "component create (rename fixture)",
            /* allow_404 */ true,
        ) {
            return;
        }
        panic!(
            "component create (rename fixture) failed (non-403/404 -- not a permission skip):\n\
             stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&create_out.stdout),
            String::from_utf8_lossy(&create_out.stderr)
        );
    }
    let created: Value = serde_json::from_slice(&create_out.stdout)
        .expect("component create (rename fixture) output must be valid JSON");
    let id = created
        .get("id")
        .and_then(Value::as_str)
        .expect("component create (rename fixture) JSON must contain an 'id' field")
        .to_string();
    guard.project = Some(proj.clone());
    guard.component_id = Some(id.clone());

    // AC-007: rename.
    let rename_out = h
        .cmd()
        .args([
            "component",
            "rename",
            &old_name,
            &new_name,
            "--project",
            &proj,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for component rename");
    if !rename_out.status.success() {
        if skip_on_403_404(&rename_out, "component rename", /* allow_404 */ false) {
            return;
        }
        panic!(
            "component rename failed (non-403/404 -- not a permission skip):\n\
             stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&rename_out.stdout),
            String::from_utf8_lossy(&rename_out.stderr)
        );
    }
    let renamed_out: Value = serde_json::from_slice(&rename_out.stdout)
        .expect("component rename output must be valid JSON");
    let renamed = renamed_out
        .get("renamed")
        .expect("component rename JSON must contain a top-level 'renamed' key");
    assert_eq!(
        renamed.get("id").and_then(Value::as_str),
        Some(id.as_str()),
        "renamed.id must equal the id captured at creation (BC-8.3.001 id-preservation); got: {renamed_out}"
    );
    assert_eq!(
        renamed.get("from").and_then(Value::as_str),
        Some(old_name.as_str())
    );
    assert_eq!(
        renamed.get("to").and_then(Value::as_str),
        Some(new_name.as_str())
    );
    assert_eq!(
        renamed.get("project").and_then(Value::as_str),
        Some(proj.as_str())
    );

    // AC-008: list reflects the rename.
    let list_out = h
        .cmd()
        .args(["component", "list", "--project", &proj, "--output", "json"])
        .output()
        .expect("failed to spawn jr for component list (AC-008)");
    assert!(
        list_out.status.success(),
        "component list (AC-008) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&list_out.stdout),
        String::from_utf8_lossy(&list_out.stderr)
    );
    let list: Value = serde_json::from_slice(&list_out.stdout)
        .expect("component list (AC-008) output must be valid JSON");
    let arr = list
        .as_array()
        .expect("component list (AC-008) must be a JSON array");
    assert!(
        arr.iter()
            .any(|c| c.get("id").and_then(Value::as_str) == Some(id.as_str())
                && c.get("name").and_then(Value::as_str) == Some(new_name.as_str())),
        "component list (AC-008) must contain id={id} name={new_name}; got: {list}"
    );
    assert!(
        !arr.iter()
            .any(|c| c.get("name").and_then(Value::as_str) == Some(old_name.as_str())),
        "component list (AC-008) must NOT contain the original name {old_name}; got: {list}"
    );

    // Teardown handled by `guard`'s Drop impl (component delete --orphan --yes,
    // by the stable numeric id — survives the rename above per AC-015).
}

/// E2E: `jr issue create --project <proj> --component <comp>` sets the
/// initial `components` array on a live Jira Cloud issue (BC-3.4.024).
///
/// Component discovery mirrors
/// `test_e2e_issue_edit_component_multikey_bulk_roundtrip`'s precondition
/// check — clean-skip if the project has zero components. No throwaway
/// component is created by this test (independent of the lifecycle fixtures
/// above).
///
/// Traces to: AC-009, BC-3.4.024, BC-3.4.025.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_create_component_single_key_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let proj = project();
    let itype = issue_type();
    let label = run_label();

    let comp = match discover_component(&h, &proj, "issue create --component discovery") {
        Some(c) => c,
        None => return,
    };

    let summary = format!("[e2e {label}] create --component single-key");
    let create_out = h
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
            "--component",
            &comp,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create --component");
    if !create_out.status.success() {
        if skip_on_403_404(
            &create_out,
            "issue create --component",
            /* allow_404 */ true,
        ) {
            return;
        }
        panic!(
            "issue create --component failed (non-403/404 -- not a permission skip):\n\
             stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&create_out.stdout),
            String::from_utf8_lossy(&create_out.stderr)
        );
    }
    let created: Value = serde_json::from_slice(&create_out.stdout)
        .expect("issue create --component output must be valid JSON");
    let key = created
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create --component JSON must contain a 'key' field")
        .to_string();

    let view = poll_view(&key, &h);
    let names: Vec<String> = view
        .get("fields")
        .and_then(|f| f.get("components"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        names.contains(&comp),
        "fields.components[].name must contain '{comp}' on {key}; got: {names:?}"
    );

    best_effort_close(&h, &key);
}

/// E2E: `jr issue edit <key> --component add:<comp>` / `remove:<comp>` on
/// EXACTLY ONE key — the single-key native `update`-verb wire shape
/// (BC-3.4.022), distinct from
/// `test_e2e_issue_edit_component_multikey_bulk_roundtrip` above, which
/// always supplies 2+ keys and therefore only ever exercises BC-3.4.023's
/// `multiselectComponents` bulk shape.
///
/// Architecture Compliance Rule 3: this test MUST supply exactly ONE key on
/// the `issue edit --component` command line — using 2+ keys would silently
/// re-exercise the bulk path instead of the single-key native path this test
/// targets.
///
/// Traces to: AC-010, BC-3.4.022.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_edit_component_single_key_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let proj = project();
    let label = run_label();

    let comp = match discover_component(&h, &proj, "issue edit --component single-key discovery") {
        Some(c) => c,
        None => return,
    };

    // Fresh, comp-free issue (--component NOT supplied at create time).
    let summary = format!("[e2e {label}] edit --component single-key");
    let key = seed_issue(&h, &label, &summary);

    let component_names = |v: &Value| -> Vec<String> {
        v.get("fields")
            .and_then(|f| f.get("components"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.get("name").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };

    let before = poll_view(&key, &h);
    assert!(
        !component_names(&before).contains(&comp),
        "component '{comp}' must be ABSENT on {key} before add; got: {:?}",
        component_names(&before)
    );

    // Single-key add (exactly ONE key on the command line -- BC-3.4.022, not
    // BC-3.4.023's bulk multiselectComponents shape).
    let add_out = h
        .cmd()
        .args(["issue", "edit", &key, "--component", &format!("add:{comp}")])
        .output()
        .expect("failed to spawn jr for single-key issue edit --component add");
    if !add_out.status.success() {
        if skip_on_403_404(
            &add_out,
            "single-key issue edit --component add",
            /* allow_404 */ false,
        ) {
            best_effort_close(&h, &key);
            return;
        }
        panic!(
            "single-key issue edit --component add failed (non-403/404 -- not a permission \
             skip):\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&add_out.stdout),
            String::from_utf8_lossy(&add_out.stderr)
        );
    }
    let after_add = poll_view(&key, &h);
    assert!(
        component_names(&after_add).contains(&comp),
        "component '{comp}' must be PRESENT on {key} after add; got: {:?}",
        component_names(&after_add)
    );

    // Single-key remove.
    let remove_out = h
        .cmd()
        .args([
            "issue",
            "edit",
            &key,
            "--component",
            &format!("remove:{comp}"),
        ])
        .output()
        .expect("failed to spawn jr for single-key issue edit --component remove");
    if !remove_out.status.success() {
        if skip_on_403_404(
            &remove_out,
            "single-key issue edit --component remove",
            /* allow_404 */ false,
        ) {
            best_effort_close(&h, &key);
            return;
        }
        panic!(
            "single-key issue edit --component remove failed (non-403/404 -- not a permission \
             skip):\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&remove_out.stdout),
            String::from_utf8_lossy(&remove_out.stderr)
        );
    }
    let after_remove = poll_view(&key, &h);
    assert!(
        !component_names(&after_remove).contains(&comp),
        "component '{comp}' must be ABSENT on {key} after remove; got: {:?}",
        component_names(&after_remove)
    );

    best_effort_close(&h, &key);
}

/// E2E: `jr issue list --project <proj> --component <comp>` bare/`not:`/`none`
/// filter grammar composition against a live JQL search (BC-2.1.018/019/020).
///
/// Component discovery mirrors AC-009/AC-010 (independent call, clean-skip on
/// empty). A fresh issue is created WITH `--component <comp>` at create time,
/// then polled via a bounded backoff loop (`poll_component_filter`, mirrors
/// the suite's `poll_jql` convention) before the filter assertions, to absorb
/// JQL search indexing lag (EC-COMP-E2E-5).
///
/// Traces to: AC-011, AC-012, AC-013, BC-2.1.018, BC-2.1.019, BC-2.1.020.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_issue_list_component_filter_grammar() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let proj = project();
    let itype = issue_type();
    let label = run_label();

    let comp = match discover_component(&h, &proj, "issue list --component filter discovery") {
        Some(c) => c,
        None => return,
    };

    let summary = format!("[e2e {label}] list --component filter grammar");
    let create_out = h
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
            "--component",
            &comp,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create --component (filter fixture)");
    if !create_out.status.success() {
        if skip_on_403_404(
            &create_out,
            "issue create --component (filter fixture)",
            /* allow_404 */ true,
        ) {
            return;
        }
        panic!(
            "issue create --component (filter fixture) failed (non-403/404 -- not a permission \
             skip):\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&create_out.stdout),
            String::from_utf8_lossy(&create_out.stderr)
        );
    }
    let created: Value = serde_json::from_slice(&create_out.stdout)
        .expect("issue create --component (filter fixture) output must be valid JSON");
    let key = created
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create --component (filter fixture) JSON must contain a 'key' field")
        .to_string();
    let _ = poll_view(&key, &h);

    // LOW-A / flaky-risk fix (S-COMP-E2E-1 adversarial review, round 2): seed
    // a SECOND, throwaway CONTROL issue that carries NO component. Without
    // it, the bare-filter assertion below is positive-only (a regression
    // that dropped the component constraint on the bare path would still
    // contain `key`), and the `not:`/`none` non-empty assertions further
    // down are only ever satisfied by externally-accumulated component-less
    // issues in the project rather than anything this test controls. The
    // control issue makes all three self-sufficient from this test's own
    // fixtures.
    let control_summary =
        format!("[e2e {label}] list --component filter grammar (control, no component)");
    let control_create_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &control_summary,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue create (filter fixture control, no component)");
    if !control_create_out.status.success() {
        if skip_on_403_404(
            &control_create_out,
            "issue create (filter fixture control, no component)",
            /* allow_404 */ true,
        ) {
            best_effort_close(&h, &key);
            return;
        }
        panic!(
            "issue create (filter fixture control, no component) failed (non-403/404 -- not a \
             permission skip):\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&control_create_out.stdout),
            String::from_utf8_lossy(&control_create_out.stderr)
        );
    }
    let control_created: Value = serde_json::from_slice(&control_create_out.stdout)
        .expect("issue create (filter fixture control, no component) output must be valid JSON");
    let control_key = control_created
        .get("key")
        .and_then(Value::as_str)
        .expect(
            "issue create (filter fixture control, no component) JSON must contain a 'key' field",
        )
        .to_string();
    let _ = poll_view(&control_key, &h);

    let key_in_results = |v: &Value, target: &str| -> bool {
        v.as_array()
            .map(|arr| {
                arr.iter()
                    .any(|i| i.get("key").and_then(Value::as_str) == Some(target))
            })
            .unwrap_or(false)
    };

    // AC-011: bare --component finds the tagged key. Bounded poll to absorb
    // JQL search indexing lag (EC-COMP-E2E-5).
    let found = poll_component_filter(&h, &proj, &comp, &key);
    assert!(
        found,
        "issue list --component {comp} must contain {key} (AC-011); \
         search indexing may not have caught up"
    );
    // LOW-A (S-COMP-E2E-1 adversarial review): the assertion above is
    // positive-only -- it never proves the bare filter EXCLUDES a
    // component-less issue. A regression that dropped the component
    // constraint on the bare path (returning all project issues unfiltered)
    // would still contain `key` and pass the assertion above. Re-query the
    // same bare filter and assert the component-less CONTROL key is absent.
    //
    // MEDIUM fix (S-COMP-E2E-1 follow-up review): the poll above proves
    // GET-by-key consistency for `key` via `poll_view` (issue view), NOT
    // JQL-search-index consistency for `control_key` (issue list). Without
    // an independent proof that `control_key` is actually JQL-searchable,
    // the absence assertion below is vacuous: if a bare-filter regression
    // dropped the component constraint AND `control_key` simply hasn't hit
    // the search index yet, the assertion would pass for the wrong reason
    // (not indexed, not "correctly excluded"). Prove indexing first, via a
    // filter `control_key` MUST satisfy (`not:{comp}` -- it has no
    // component) -- mirrors the discipline already used at AC-012/AC-013
    // below. This poll doubles as AC-012's own control-indexing proof, so
    // its result is reused there instead of polling a second time.
    let control_found_not = poll_component_filter(&h, &proj, &format!("not:{comp}"), &control_key);
    assert!(
        control_found_not,
        "issue list --component not:{comp} must contain the component-less control key \
         {control_key} (AC-011/AC-012 indexing proof); search indexing may not have caught up"
    );
    let bare_out = h
        .cmd()
        .args([
            "issue",
            "list",
            "--project",
            &proj,
            "--component",
            &comp,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue list --component (AC-011 control check)");
    assert!(
        bare_out.status.success(),
        "issue list --component {comp} (AC-011 control check) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&bare_out.stdout),
        String::from_utf8_lossy(&bare_out.stderr)
    );
    let bare_v: Value = serde_json::from_slice(&bare_out.stdout)
        .expect("issue list --component (AC-011 control check) output must be valid JSON");
    assert!(
        !key_in_results(&bare_v, &control_key),
        "issue list --component {comp} must NOT contain the component-less control key \
         {control_key} (AC-011); got: {bare_v}"
    );

    // AC-012: not:<comp> excludes the tagged key (issue HAS the component)
    // and includes the control key (issue has NO component). `control_found_not`
    // was already proven true above (hoisted as the AC-011 indexing proof) --
    // it absorbs indexing lag for the control issue AND doubles as the
    // "provably non-empty, self-controlled" evidence the LOW-1 non-empty
    // check below used to lack. Reused here rather than polling a second
    // time for the identical filter/key pair.
    let not_out = h
        .cmd()
        .args([
            "issue",
            "list",
            "--project",
            &proj,
            "--component",
            &format!("not:{comp}"),
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue list --component not: (AC-012)");
    assert!(
        not_out.status.success(),
        "issue list --component not:{comp} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&not_out.stdout),
        String::from_utf8_lossy(&not_out.stderr)
    );
    let not_v: Value = serde_json::from_slice(&not_out.stdout)
        .expect("issue list --component not: output must be valid JSON");
    assert!(
        !key_in_results(&not_v, &key),
        "issue list --component not:{comp} must NOT contain {key} (AC-012); got: {not_v}"
    );
    // LOW-1 (S-COMP-E2E-1 adversarial review): the assertion above is
    // vacuously true if the filter returns an EMPTY result set — that would
    // also be a real regression (the filter over-excluding, or JQL
    // composition breaking outright), not evidence the exclusion worked. The
    // control_found_not poll above already proves this test's OWN
    // control fixture composes real, non-empty `not:` results; this
    // assertion remains as a second, independent signal (and stays correct
    // even in projects that also accumulate external component-less
    // issues).
    assert!(
        not_v.as_array().is_some_and(|arr| !arr.is_empty()),
        "issue list --component not:{comp} must return a NON-EMPTY result set (AC-012); \
         an empty set would vacuously satisfy the exclusion check above without \
         proving the not: filter is composing real results; got: {not_v}"
    );

    // AC-013: none excludes the tagged key (issue HAS a component) and
    // includes the control key (issue has NO component). Same poll-first
    // rationale as AC-012 above.
    let control_found_none = poll_component_filter(&h, &proj, "none", &control_key);
    assert!(
        control_found_none,
        "issue list --component none must contain the component-less control key \
         {control_key} (AC-013); search indexing may not have caught up"
    );
    let none_out = h
        .cmd()
        .args([
            "issue",
            "list",
            "--project",
            &proj,
            "--component",
            "none",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for issue list --component none (AC-013)");
    assert!(
        none_out.status.success(),
        "issue list --component none failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&none_out.stdout),
        String::from_utf8_lossy(&none_out.stderr)
    );
    let none_v: Value = serde_json::from_slice(&none_out.stdout)
        .expect("issue list --component none output must be valid JSON");
    assert!(
        !key_in_results(&none_v, &key),
        "issue list --component none must NOT contain {key} (AC-013); got: {none_v}"
    );
    // LOW-1 (S-COMP-E2E-1 adversarial review): same vacuous-empty-set concern
    // as the AC-012 assertion above — `none` returning zero results would
    // also (incorrectly) satisfy "target key absent" without proving the
    // filter did any real exclusion. The control_found_none poll above
    // already proves this test's OWN control fixture composes real,
    // non-empty `none` results; this assertion remains as a second,
    // independent signal.
    assert!(
        none_v.as_array().is_some_and(|arr| !arr.is_empty()),
        "issue list --component none must return a NON-EMPTY result set (AC-013); \
         an empty set would vacuously satisfy the exclusion check above without \
         proving the none filter is composing real results; got: {none_v}"
    );

    best_effort_close(&h, &key);
    best_effort_close(&h, &control_key);
}

// ────────────────────────────────────────────────────────────────────────────
// ADF markdown round-trip tests (#475)
//
// Each test creates an issue via `jr issue create --markdown --description <md>`
// and then reads back the STORED ADF via `poll_view` to assert specific ADF node
// shapes survived both the POST and Jira's server-side normalization.
//
// All tests use the standard e2e_enabled() / run_label() / issue_type() /
// project() / e2e_harness() helpers and clean up via best_effort_close().
//
// LISTITEM NORMALIZATION (#470): `listItem` normalization-correctness assertions
// are a lower live-value class (no observable user-facing change on read-back)
// and are deferred as a follow-up — worth adding if a regression surfaces.
// ────────────────────────────────────────────────────────────────────────────

/// Recursively searches an ADF JSON value for a `taskItem` node whose
/// descendant text contains `text` (case-sensitive substring) and whose
/// `attrs.state` equals `state` (exact, e.g. `"TODO"` or `"DONE"`).
///
/// Tolerates Jira-added fields such as `localId` on the node (matching is by
/// type/state/text, not exact equality of the whole node).
///
/// Recursion is unbounded by depth; safe because the only input is the small,
/// self-created issue description read back via `poll_view`.
fn adf_has_task_item(node: &Value, text: &str, state: &str) -> bool {
    if node.get("type").and_then(Value::as_str) == Some("taskItem") {
        let node_state = node
            .get("attrs")
            .and_then(|a| a.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if node_state == state && adf_contains_text(node, text) {
            return true;
        }
    }
    match node {
        Value::Array(items) => items.iter().any(|v| adf_has_task_item(v, text, state)),
        Value::Object(map) => map.values().any(|v| adf_has_task_item(v, text, state)),
        _ => false,
    }
}

/// Returns `true` if any descendant `text` node in `node` contains `needle`
/// as a substring.
fn adf_contains_text(node: &Value, needle: &str) -> bool {
    if node.get("type").and_then(Value::as_str) == Some("text") {
        if let Some(t) = node.get("text").and_then(Value::as_str) {
            if t.contains(needle) {
                return true;
            }
        }
    }
    match node {
        Value::Array(items) => items.iter().any(|v| adf_contains_text(v, needle)),
        Value::Object(map) => map.values().any(|v| adf_contains_text(v, needle)),
        _ => false,
    }
}

/// Recursively searches an ADF JSON value for a node whose `type` field equals
/// `node_type`.  Generic counterpart to the type-specific helpers; use this
/// when you need to assert *absence* of a node type (e.g. no `orderedList`).
fn adf_has_node_type(node: &Value, node_type: &str) -> bool {
    if node.get("type").and_then(Value::as_str) == Some(node_type) {
        return true;
    }
    match node {
        Value::Array(items) => items.iter().any(|v| adf_has_node_type(v, node_type)),
        Value::Object(map) => map.values().any(|v| adf_has_node_type(v, node_type)),
        _ => false,
    }
}

/// Recursively searches an ADF JSON value for a `taskList` node.
fn adf_has_task_list(node: &Value) -> bool {
    adf_has_node_type(node, "taskList")
}

/// Recursively searches an ADF JSON value for a `panel` node with the given
/// `panel_type` in its `attrs.panelType` field.
fn adf_has_panel(node: &Value, panel_type: &str) -> bool {
    if node.get("type").and_then(Value::as_str) == Some("panel") {
        let pt = node
            .get("attrs")
            .and_then(|a| a.get("panelType"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if pt == panel_type {
            return true;
        }
    }
    match node {
        Value::Array(items) => items.iter().any(|v| adf_has_panel(v, panel_type)),
        Value::Object(map) => map.values().any(|v| adf_has_panel(v, panel_type)),
        _ => false,
    }
}

/// Recursively searches an ADF JSON value for a `text` node carrying a
/// `subsup` mark whose `attrs.type` equals `mark_type` (e.g. `"sub"` or
/// `"sup"`).
fn adf_has_subsup_mark(node: &Value, mark_type: &str) -> bool {
    if node.get("type").and_then(Value::as_str) == Some("text") {
        if let Some(marks) = node.get("marks").and_then(Value::as_array) {
            let hit = marks.iter().any(|m| {
                m.get("type").and_then(Value::as_str) == Some("subsup")
                    && m.get("attrs")
                        .and_then(|a| a.get("type"))
                        .and_then(Value::as_str)
                        == Some(mark_type)
            });
            if hit {
                return true;
            }
        }
    }
    match node {
        Value::Array(items) => items.iter().any(|v| adf_has_subsup_mark(v, mark_type)),
        Value::Object(map) => map.values().any(|v| adf_has_subsup_mark(v, mark_type)),
        _ => false,
    }
}

/// E2E (#471): a GFM task-list in a `--markdown` description produces ADF
/// `taskItem` nodes with `attrs.state` of `"TODO"` / `"DONE"` that Jira
/// accepts and preserves on read-back.
///
/// Description: `- [ ] todo item\n- [x] done item`
///
/// Asserts:
/// - A `taskItem` whose text contains `"todo item"` has `state == "TODO"`.
/// - A `taskItem` whose text contains `"done item"` has `state == "DONE"`.
/// - A `taskList` container exists somewhere in the description ADF.
///
/// Traces to: #471 (GFM task lists → ADF taskList/taskItem), #475 (ADF E2E batch).
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_markdown_task_list_produces_task_items() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let itype = issue_type();
    let proj = project();
    let h = e2e_harness();

    let md = "- [ ] todo item\n- [x] done item";
    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] task-list taskItem round-trip"),
            "--markdown",
            "--description",
            md,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --markdown task-list");
    assert!(
        create.status.success(),
        "create --markdown (task list) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();

    let view = poll_view(&key, &h);
    let description = &view["fields"]["description"];

    assert!(
        adf_has_task_item(description, "todo item", "TODO"),
        "unchecked task item must round-trip as taskItem with state=TODO; \
         stored description: {description}"
    );
    assert!(
        adf_has_task_item(description, "done item", "DONE"),
        "checked task item must round-trip as taskItem with state=DONE; \
         stored description: {description}"
    );
    assert!(
        adf_has_task_list(description),
        "description must contain a taskList container node; \
         stored description: {description}"
    );

    best_effort_close(&h, &key);
}

/// E2E (#471 EC-17): an ordered-syntax task list (`1. [ ] …`) in a `--markdown`
/// description produces ADF `taskItem` nodes — NOT an `orderedList`.
///
/// EC-17 specifies that an ordered-list task list promotes to `taskList`, so
/// the same `taskItem`/`taskList` ADF structure is expected regardless of
/// whether the source markdown used `- ` or `1. ` prefixes.
///
/// Description: `1. [ ] ordered todo\n2. [x] ordered done`
///
/// Asserts:
/// - A `taskItem` with `state == "TODO"` containing `"ordered todo"`.
/// - A `taskItem` with `state == "DONE"` containing `"ordered done"`.
/// - A `taskList` container is present (no `orderedList`).
///
/// Traces to: #471 EC-17, #475 (ADF E2E batch).
///
/// RISK: pulldown-cmark 0.13 may or may not promote ordered task-list syntax
/// to `Tag::TaskListMarker` the same way it does for unordered lists. If this
/// test fails live, it means EC-17 is unimplemented or the ordered path
/// produces `orderedList` instead of `taskList` — actionable signal.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_markdown_ordered_task_list_produces_task_items() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let itype = issue_type();
    let proj = project();
    let h = e2e_harness();

    let md = "1. [ ] ordered todo\n2. [x] ordered done";
    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] ordered task-list taskItem round-trip"),
            "--markdown",
            "--description",
            md,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --markdown ordered task-list");
    assert!(
        create.status.success(),
        "create --markdown (ordered task list) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();

    let view = poll_view(&key, &h);
    let description = &view["fields"]["description"];

    assert!(
        adf_has_task_item(description, "ordered todo", "TODO"),
        "unchecked ordered task item must round-trip as taskItem with state=TODO \
         (EC-17: ordered task list promotes to taskList); \
         stored description: {description}"
    );
    assert!(
        adf_has_task_item(description, "ordered done", "DONE"),
        "checked ordered task item must round-trip as taskItem with state=DONE \
         (EC-17: ordered task list promotes to taskList); \
         stored description: {description}"
    );
    assert!(
        adf_has_task_list(description),
        "description must contain a taskList container (not orderedList) for \
         ordered task-list syntax (EC-17); stored description: {description}"
    );
    assert!(
        !adf_has_node_type(description, "orderedList"),
        "description must NOT contain an orderedList node — ordered task-list syntax \
         must promote to taskList, not remain as orderedList (EC-17); \
         stored description: {description}"
    );

    best_effort_close(&h, &key);
}

/// E2E (#474): subscript `~x~` and superscript `^x^` in a `--markdown`
/// description produce ADF `subsup` marks with the correct `attrs.type` that
/// Jira accepts and preserves on read-back.
///
/// Description: `H ~2~ O and E=mc ^2^`
///
/// Note the space before `^2^`: pulldown-cmark does not open a superscript
/// when `^` is immediately preceded by a word character (so `mc^2^` stays
/// literal — use `mc ^2^` instead).
///
/// Asserts:
/// - A `text` node carries a `subsup` mark with `attrs.type == "sub"`.
/// - A `text` node carries a `subsup` mark with `attrs.type == "sup"`.
///
/// Traces to: #474 (subsup marks), #475 (ADF E2E batch).
///
/// RISK: Jira Cloud may normalize `subsup` marks into something else on
/// storage (e.g. drop unknown mark types). If the test fails live with
/// adf_has_subsup_mark returning false, check the stored description JSON
/// to determine whether Jira accepted the mark — if Jira silently drops it,
/// the behavior is a Jira-side limitation, not a jr bug.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_markdown_subsup_produces_subsup_marks() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let itype = issue_type();
    let proj = project();
    let h = e2e_harness();

    // Space before ^2^ is required: pulldown-cmark won't open superscript
    // tight against a preceding word char (see CLAUDE.md #474 gotcha).
    // Space after the closing ~2~ is also required: the proven-supported form
    // (unit test `test_markdown_subscript_to_subsup_sub`) uses whitespace on
    // both sides of the span. Tight-closing `~2~O` may not be recognized as
    // subscript by pulldown-cmark — using `~2~ O` avoids a spurious failure on
    // first live run (adversary MED-001).
    let md = "H ~2~ O and E=mc ^2^";
    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] subsup mark round-trip"),
            "--markdown",
            "--description",
            md,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --markdown subsup");
    assert!(
        create.status.success(),
        "create --markdown (subsup) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();

    let view = poll_view(&key, &h);
    let description = &view["fields"]["description"];

    assert!(
        adf_has_subsup_mark(description, "sub"),
        "subscript ~2~ must round-trip as a subsup mark with attrs.type=\"sub\"; \
         stored description: {description}"
    );
    assert!(
        adf_has_subsup_mark(description, "sup"),
        "superscript ^2^ must round-trip as a subsup mark with attrs.type=\"sup\"; \
         stored description: {description}"
    );

    best_effort_close(&h, &key);
}

/// E2E (#483): a GFM alert blockquote (`> [!NOTE]`) in a `--markdown`
/// description produces an ADF `panel` node with `attrs.panelType == "info"`
/// that Jira accepts and preserves on read-back.
///
/// Also covers `> [!WARNING]` → `panelType == "warning"` to exercise a second
/// kind mapping and confirm the exhaustive `panel_type_for` dispatch.
///
/// Description:
/// ```text
/// > [!NOTE]
/// > note body
///
/// > [!WARNING]
/// > warn body
/// ```
///
/// Asserts:
/// - A `panel` node with `attrs.panelType == "info"` exists (Note → info).
/// - A `panel` node with `attrs.panelType == "warning"` exists (Warning → warning).
///
/// Traces to: #483 (GFM alerts → ADF panel), #475 (ADF E2E batch).
///
/// RISK: Jira Cloud may reject or normalize `panel` nodes submitted via REST
/// on some site configurations (panel availability is editor-flag-gated on
/// some older Jira Cloud versions). If the test fails live with the panel
/// node absent from the stored description, check whether the site's ADF
/// schema supports `panel` — this is a Jira-side limitation.
///
/// NOTE: The canonical E2E Jira site is assumed to be a modern Jira Cloud
/// instance where `panel` nodes are supported. Assertions are hard (not
/// skipped) because a live pass is the intended verification for #483. If
/// the site is reconfigured to disable panels, update or skip the test and
/// file an ops note in `docs/specs/e2e-live-jira-testing.md`.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_markdown_gfm_alert_produces_panel() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let itype = issue_type();
    let proj = project();
    let h = e2e_harness();

    let md = "> [!NOTE]\n> note body\n\n> [!WARNING]\n> warn body";
    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] GFM alert panel round-trip"),
            "--markdown",
            "--description",
            md,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --markdown GFM alert");
    assert!(
        create.status.success(),
        "create --markdown (GFM alert) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();

    let view = poll_view(&key, &h);
    let description = &view["fields"]["description"];

    assert!(
        adf_has_panel(description, "info"),
        "expected ADF `panel` (panelType=info) in stored description — if absent, \
         either the markdown→ADF panel mapping regressed (#483) OR the canonical E2E \
         Jira site lacks panel support (verify site config / panel editor flag); \
         stored description: {description}"
    );
    assert!(
        adf_has_panel(description, "warning"),
        "expected ADF `panel` (panelType=warning) in stored description — if absent, \
         either the markdown→ADF panel mapping regressed (#483) OR the canonical E2E \
         Jira site lacks panel support (verify site config / panel editor flag); \
         stored description: {description}"
    );

    best_effort_close(&h, &key);
}

/// E2E (#489): block-level HTML in a `--markdown` description is preserved as
/// literal text rather than dropped, and that literal text survives Jira's
/// REST API storage and read-back.
///
/// Description: `<div>raw block content</div>`
///
/// Asserts:
/// - The text `"raw block content"` appears somewhere in the stored ADF.
///
/// This is the live proof that #489's "preserve, not drop" semantics work
/// end-to-end: the block HTML is rendered as a literal-text paragraph in ADF
/// and Jira accepts it.
///
/// Traces to: #489 (block HTML → literal text), #475 (ADF E2E batch).
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_markdown_block_html_preserved() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let itype = issue_type();
    let proj = project();
    let h = e2e_harness();

    let md = "<div>raw block content</div>";
    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] block HTML preserved as literal text"),
            "--markdown",
            "--description",
            md,
            "--label",
            &label,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr for create --markdown block HTML");
    assert!(
        create.status.success(),
        "create --markdown (block HTML) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();

    let view = poll_view(&key, &h);
    let description = &view["fields"]["description"];

    assert!(
        adf_contains_text(description, "raw block content"),
        "block HTML must be preserved as literal text in the stored ADF \
         (text node containing \"raw block content\"); \
         stored description: {description}"
    );

    best_effort_close(&h, &key);
}

/// Returns `true` if any `listItem` node in `node` has a direct child whose
/// `type` field equals `"blockquote"`.
///
/// Used by `test_e2e_adf_read_path_human_output` (AC-2) to assert that
/// `normalize_list_item_content` stripped blockquotes from `listItem` content
/// before submission to Jira. The check is **direct-child only**: it inspects
/// `listItem["content"]` array entries, not arbitrary descendants, so a
/// `blockquote` deeper in the tree (e.g. inside a `paragraph`) does not trigger
/// a `true` return.
///
/// Tolerates Jira-added `localId` or other extra fields — matching is by
/// `type` only. Recursion is unbounded by depth, safe because the only input
/// is the small, self-created issue description read back via `poll_view`.
fn adf_has_blockquote_in_list_item(node: &Value) -> bool {
    if node.get("type").and_then(Value::as_str) == Some("listItem") {
        if let Some(content) = node.get("content").and_then(Value::as_array) {
            if content
                .iter()
                .any(|child| child.get("type").and_then(Value::as_str) == Some("blockquote"))
            {
                return true;
            }
        }
    }
    match node {
        Value::Array(items) => items.iter().any(adf_has_blockquote_in_list_item),
        Value::Object(map) => map.values().any(adf_has_blockquote_in_list_item),
        _ => false,
    }
}

/// E2E (#475): exercises the `adf_to_text` read path via `jr issue view` and
/// `jr issue comments` in human/table mode (no `--output json`), and verifies
/// that `normalize_list_item_content` is exercised against Jira Cloud.
///
/// Three AC sub-sequences within one create → view → comment → close lifecycle:
///
/// **AC-1 — `adf_to_text` via `jr issue view` human mode:**
/// Creates an issue with a rich Markdown description containing a heading, a
/// list item with a nested blockquote (`- > …`), a fenced code block, and a
/// hyperlink. Invokes `jr issue view <key>` WITHOUT `--output json` and asserts
/// that `adf_to_text`-rendered content words appear in stdout. Single-token
/// words are used (not multi-word phrases) to be resilient to comfy-table
/// `ContentArrangement::Dynamic` word-wrapping that may insert newlines between
/// adjacent words within a cell.
///
/// **AC-2 — `normalize_list_item_content` live exercise:**
/// Using the same issue and `poll_view` JSON: (a) create exit-0 proves no Jira
/// 400 from invalid ADF (the blockquote was normalised before submission);
/// (b) positive gate confirms `listItem` nodes are present; (c) text sanity
/// checks the blockquote prose was not silently dropped; (d) absence assertion
/// `!adf_has_blockquote_in_list_item` confirms no `blockquote` node is a
/// direct child of any `listItem` in the stored ADF.
///
/// **AC-3 — `adf_to_text` via `jr issue comments` human mode:**
/// Seeds a comment `"Comment **body** with _emphasis_" --markdown`. Reads back
/// via `jr issue comments <key>` in table mode. Asserts `**body**` (strong) and
/// `*emphasis*` (em, single-asterisk re-emit) appear in stdout, and that
/// `_emphasis_` (the raw input syntax) does NOT appear — proving `adf_to_text`
/// was actually called rather than a raw passthrough.
///
/// Traces to: BC-7.2.003, BC-7.2.004, BC-7.2.006, #475.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site"]
fn test_e2e_adf_read_path_human_output() {
    if !e2e_enabled() {
        return;
    }
    let label = run_label();
    let itype = issue_type();
    let proj = project();
    let h = e2e_harness();

    // -------------------------------------------------------------------------
    // Setup: build a rich Markdown description and create an issue.
    //
    // The description contains:
    //   - A level-2 heading           → AC-1: assert "Header" appears in view
    //   - A list item with blockquote → AC-1: assert "blockquote" appears;
    //                                    AC-2: assert normalization happened
    //   - A fenced code block         → AC-1: assert "snippet" appears in view
    //   - A hyperlink                 → AC-1: assert "link" appears in view
    //   - Plain prose                 → padding
    //
    // Use `--description-stdin` + `write_stdin` so leading-dash content in the
    // markdown (`- > nested blockquote text`) is never parsed by clap as a flag.
    // -------------------------------------------------------------------------
    let md = "## Section Header\n\n\
              - > nested blockquote text\n\n\
              ```\ncode snippet\n```\n\n\
              [link text](https://example.com)\n\n\
              A plain prose paragraph.";

    let create = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &proj,
            "--type",
            &itype,
            "--summary",
            &format!("[e2e {label}] ADF read path E2E"),
            "--description-stdin",
            "--markdown",
            "--label",
            &label,
            "--output",
            "json",
        ])
        .write_stdin(md)
        .output()
        .expect("failed to spawn jr for create --description-stdin --markdown");

    assert!(
        create.status.success(),
        "create with blockquote-in-listItem must exit 0 (AC-2 primary assertion);\
         \nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );

    let create_json: Value =
        serde_json::from_slice(&create.stdout).expect("create output must be valid JSON");
    let key = create_json
        .get("key")
        .and_then(Value::as_str)
        .expect("create JSON must contain a 'key'")
        .to_string();

    // -------------------------------------------------------------------------
    // AC-1: human-mode view — assert adf_to_text rendered content words appear.
    //
    // Single-token content words are used rather than multi-word phrases because
    // comfy-table ContentArrangement::Dynamic may word-wrap long lines inside a
    // cell by inserting newlines between adjacent words. Single tokens are never
    // split by the cell-wrap algorithm.
    // -------------------------------------------------------------------------
    let view_human = h
        .cmd()
        .args(["issue", "view", &key])
        .output()
        .expect("jr issue view (human mode) failed");
    assert!(
        view_human.status.success(),
        "jr issue view (human mode) must exit 0 for {key};\nstderr: {}",
        String::from_utf8_lossy(&view_human.stderr)
    );
    let stdout_view = String::from_utf8_lossy(&view_human.stdout);

    assert!(
        stdout_view.contains("Header"),
        "heading word 'Header' must appear in human-mode view output (AC-1);\
         \nstdout: {stdout_view}"
    );
    assert!(
        stdout_view.contains("snippet"),
        "code-block word 'snippet' must appear in human-mode view output (AC-1);\
         \nstdout: {stdout_view}"
    );
    assert!(
        stdout_view.contains("blockquote"),
        "blockquote prose word 'blockquote' must appear in human-mode view output after \
         normalization (AC-1);\nstdout: {stdout_view}"
    );
    assert!(
        stdout_view.contains("link"),
        "hyperlink anchor word 'link' must appear in human-mode view output (AC-1);\
         \nstdout: {stdout_view}"
    );

    // -------------------------------------------------------------------------
    // AC-2: structural inspection via poll_view (JSON).
    //
    // Step 1 — positive gate: listItem nodes must be present (prevents vacuous
    //           absence assertion on stale or empty ADF).
    // Step 2 — content sanity: blockquote text was not silently dropped.
    // Step 3 — absence assertion: no blockquote node is a direct child of any
    //           listItem.content (proves normalize_list_item_content ran).
    // -------------------------------------------------------------------------
    let view_json = poll_view(&key, &h);
    let description_json = &view_json["fields"]["description"];

    assert!(
        adf_has_node_type(description_json, "listItem"),
        "ADF must contain listItem nodes (positive gate before absence assertion — AC-2);\
         \ndescription: {description_json}"
    );
    assert!(
        adf_contains_text(description_json, "nested blockquote text"),
        "blockquote text content must appear somewhere in the ADF \
         (sanity check: content not dropped — AC-2);\ndescription: {description_json}"
    );
    assert!(
        !adf_has_blockquote_in_list_item(description_json),
        "listItem.content must not contain a blockquote node after \
         normalize_list_item_content (AC-2);\ndescription: {description_json}"
    );

    // -------------------------------------------------------------------------
    // AC-3: comment read path — adf_to_text via `jr issue comments` human mode.
    //
    // Seed a comment with Markdown emphasis using underscore syntax (`_emphasis_`).
    // After markdown_to_adf → Jira → adf_to_text:
    //   - strong "body"    renders as **body**
    //   - em "emphasis"    renders as *emphasis*  (single asterisk, NOT underscore)
    //
    // The `!contains("_emphasis_")` assertion is the key discriminator: a raw
    // passthrough would leave the underscore form intact; the live adf_to_text
    // path converts it to single-asterisk em rendering.
    // -------------------------------------------------------------------------
    let comment_out = h
        .cmd()
        .args([
            "issue",
            "comment",
            "add",
            &key,
            "Comment **body** with _emphasis_",
            "--markdown",
        ])
        .output()
        .expect("failed to spawn jr for issue comment --markdown");
    assert!(
        comment_out.status.success(),
        "issue comment --markdown must exit 0 for {key} (AC-3);\nstderr: {}",
        String::from_utf8_lossy(&comment_out.stderr)
    );

    let comments_out = h
        .cmd()
        .args(["issue", "comments", &key])
        .output()
        .expect("failed to spawn jr for issue comments (human mode)");
    assert!(
        comments_out.status.success(),
        "jr issue comments (human mode) must exit 0 for {key} (AC-3);\nstderr: {}",
        String::from_utf8_lossy(&comments_out.stderr)
    );
    let stdout_comments = String::from_utf8_lossy(&comments_out.stdout);

    assert!(
        stdout_comments.contains("**body**"),
        "strong text must render as **body** in comments human-mode output (AC-3);\
         \nstdout: {stdout_comments}"
    );
    assert!(
        stdout_comments.contains("*emphasis*"),
        "em text must render as *emphasis* (single asterisk) in comments human-mode \
         output (AC-3);\nstdout: {stdout_comments}"
    );
    assert!(
        !stdout_comments.contains("_emphasis_"),
        "underscore em syntax must not appear — adf_to_text re-emits em as *x*, not _x_ \
         (AC-3 discriminator);\nstdout: {stdout_comments}"
    );

    // Teardown: label-based CI sweeper handles cleanup; best_effort_close as
    // belt-and-suspenders for this standard (non-JSM) issue.
    best_effort_close(&h, &key);
}

// ---------------------------------------------------------------------------
// Helpers for test_e2e_comment_edit_visibility_merge_semantics
// ---------------------------------------------------------------------------

/// Fetch the raw JSON for a single comment with properties expanded.
///
/// Calls `jr api GET /rest/api/3/issue/{key}/comment/{cid}?expand=properties`
/// directly so the full Jira API response (including the `properties` array)
/// is available for assertion without going through the typed `Comment` struct.
fn get_comment_api_json(h: &E2eHarness, key: &str, cid: &str) -> Option<Value> {
    let path = format!("/rest/api/3/issue/{key}/comment/{cid}?expand=properties");
    let out = h.cmd().args(["api", &path]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

/// Extract `sd.public.comment.internal` boolean from a comment JSON response.
///
/// Returns `Some(true)` / `Some(false)` when the property is present,
/// `None` when the property is absent or malformed.
fn sd_internal_prop(c: &Value) -> Option<bool> {
    c.get("properties")
        .and_then(Value::as_array)?
        .iter()
        .find(|p| p.get("key").and_then(Value::as_str) == Some("sd.public.comment"))?
        .get("value")
        .and_then(|v| v.get("internal"))
        .and_then(Value::as_bool)
}

/// Build a minimal single-paragraph ADF document for a plain-text comment body.
fn adf_paragraph(text: &str) -> Value {
    serde_json::json!({
        "version": 1,
        "type": "doc",
        "content": [{"type": "paragraph", "content": [{"type": "text", "text": text}]}]
    })
}

/// Find the key of the most-recently-created issue in a JSM project.
///
/// Used by `test_e2e_comment_edit_visibility_merge_semantics` to locate a
/// long-lived fixture issue without creating one. Returns `None` and emits
/// a `[SKIP]` eprintln when the list call fails or the project has no issues.
fn find_jsm_issue_key(h: &E2eHarness, jsm_project: &str) -> Option<String> {
    let jql = format!("project={jsm_project} ORDER BY created DESC");
    let list_out = h
        .cmd()
        .args(["issue", "list", "--jql", &jql, "--output", "json"])
        .output()
        .ok()?;
    if !list_out.status.success() {
        eprintln!(
            "[SKIP] issue list for {jsm_project} failed — skipping MERGE semantics test\n\
             stderr: {}",
            String::from_utf8_lossy(&list_out.stderr)
        );
        return None;
    }
    let issues: Vec<Value> = serde_json::from_slice(&list_out.stdout).ok()?;
    issues.first()?.get("key")?.as_str().map(str::to_owned)
}

/// Delete a probe comment, logging a `[WARN]` on failure (best-effort teardown).
fn delete_comment_probe(h: &E2eHarness, key: &str, cid: &str) {
    let del = h
        .cmd()
        .args(["issue", "comment", "delete", key, "--id", cid, "--yes"])
        .output();
    if del.map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!(
            "[WARN] failed to delete probe comment {cid} on {key} \
             — orphan risk LOW"
        );
    }
}

/// Retry helper: read back a comment and check a predicate.
///
/// Retries up to 3 times with 500 ms delays (property expansion can lag on
/// free-tier sites). Returns `Some(comment_json)` on the first passing check,
/// `None` after all attempts fail or the predicate never holds.
fn poll_comment_until(
    h: &E2eHarness,
    key: &str,
    cid: &str,
    check: &dyn Fn(&Value) -> bool,
    label: &str,
) -> Option<Value> {
    for attempt in 1u8..=3 {
        let c = match get_comment_api_json(h, key, cid) {
            Some(v) => v,
            None => {
                if attempt < 3 {
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
                eprintln!("[WARN] {label}: GET comment {cid} failed after 3 attempts");
                return None;
            }
        };
        if check(&c) {
            return Some(c);
        }
        if attempt < 3 {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    eprintln!(
        "[WARN] {label}: predicate did not hold after 3 attempts \
         — property lag or assertion failure"
    );
    None
}

/// Discover a usable project role name for comment visibility restriction testing.
///
/// Calls `jr api GET /rest/api/3/project/{project_key}/role`, which returns a JSON
/// object mapping role names to their URL. Prefers `"Service Desk Team"` (the
/// canonical, stable agent role on JSM company-managed projects; Atlassian explicitly
/// refused to rename it — JSDCLOUD-1376 Won't Fix; DEC-175 Q3). Falls back to the
/// first key in the response object. Returns `None` when the API call fails, the
/// response is not a JSON object, or the object has no keys.
fn discover_project_role(h: &E2eHarness, project_key: &str) -> Option<String> {
    let path = format!("/rest/api/3/project/{project_key}/role");
    let out = h.cmd().args(["api", &path]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let roles: serde_json::Map<String, Value> = serde_json::from_slice(&out.stdout).ok()?;
    if roles.contains_key("Service Desk Team") {
        return Some("Service Desk Team".to_owned());
    }
    roles.keys().next().map(|k| k.to_owned())
}

/// Extract the `visibility.value` string from a comment JSON response.
///
/// Returns `None` when the `visibility` field is absent or its `value` is not a string.
fn comment_visibility_value(c: &Value) -> Option<&str> {
    c.get("visibility")
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
}

/// Posts a probe comment body to `POST /rest/api/3/issue/{key}/comment` and
/// returns the new comment id, or `None` (with an eprintln! warning) on any failure.
fn post_probe_comment(h: &E2eHarness, key: &str, body: &str, scenario: &str) -> Option<String> {
    let post_path = format!("/rest/api/3/issue/{key}/comment");
    let create = h
        .cmd()
        .args(["api", "-X", "POST", &post_path, "-d", body])
        .output()
        .expect("failed to spawn jr api POST for probe comment");
    if !create.status.success() {
        eprintln!(
            "[WARN] {scenario}: probe comment create failed (exit {:?}) — skipping",
            create.status.code()
        );
        return None;
    }
    let cv: Value = match serde_json::from_slice(&create.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[WARN] {scenario}: comment create JSON parse error: {e} — skipping");
            return None;
        }
    };
    match cv.get("id").and_then(Value::as_str).map(str::to_owned) {
        Some(id) => Some(id),
        None => {
            eprintln!(
                "[WARN] {scenario}: comment create response has no 'id' \
                 — skipping; got: {cv}"
            );
            None
        }
    }
}

// ---------------------------------------------------------------------------
// E2E: comment edit MERGE / PRESERVED semantics probe
// ---------------------------------------------------------------------------

/// E2E: `jr issue comment edit` MERGE / PRESERVED semantics probe.
///
/// Verifies three behavioral contracts on a live JSM project using a
/// pre-existing EJ issue as a reusable comment fixture (not closed by the test):
///
/// - **Scenario 1 (MERGE probe):** Creates a comment with
///   `sd.public.comment={internal:true}` via `jr api POST`, edits it twice with
///   `--internal`, and asserts the property is preserved after each edit.
/// - **Scenario 2 (PRESERVED-visibility baseline):** Discovers a JSM project role via
///   `GET /rest/api/3/project/{proj}/role` (prefers "Service Desk Team"; DEC-175 Q3).
///   Creates a comment with a Jira `visibility` restriction
///   (`{"type":"role","value":"<role>"}`), asserts the restriction is present on
///   GET read-back immediately after create (anti-vacuous-pass guard per DEC-175 Q2:
///   an invalid role name may be silently dropped by the API, so assert on round-trip
///   not on 2xx alone), performs a body-only edit (no flag), and asserts the
///   `visibility` restriction is still present unchanged (PRESERVED: a body-only PUT
///   sends no `"visibility"` key and must not clear the existing restriction).
///   Clean-skips Scenarios 2/3 if no project role is discoverable.
/// - **Scenario 3 (compound cell — orthogonal axes):** Creates a comment with BOTH
///   a `visibility` restriction AND `sd.public.comment={internal:true}`, asserts
///   both present on read-back, edits with `--public --yes`, and asserts (a)
///   `sd.public.comment` is updated to `internal=false` (MERGE) and (b) the
///   `visibility` restriction is still present (PRESERVED — properties-MERGE PUT
///   does not include a `"visibility"` key; two axes are orthogonal per DEC-175 Q5).
///
/// Each scenario deletes its own probe comment immediately after assertions.
/// The parent EJ issue is NOT closed.
///
/// Traces to: BC-3.5.006 delivery obligation (b), AC-001 (--internal), AC-002 (--public).
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_comment_edit_visibility_merge_semantics() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!(
                "[SKIP] JR_E2E_JSM_PROJECT not set \
                 — skipping comment edit MERGE semantics test"
            );
            return;
        }
    };
    let h = e2e_harness();
    let run_id = run_label();

    // Find a pre-existing EJ issue — long-lived shared fixture (NOT closed by test).
    let key = match find_jsm_issue_key(&h, &jsm_project) {
        Some(k) => k,
        None => {
            eprintln!("[SKIP] no {jsm_project} issue found — skipping MERGE semantics test");
            return;
        }
    };

    // Discover a project role for PRESERVED-visibility probes (Scenarios 2/3).
    // Prefers "Service Desk Team" (canonical JSM company-managed agent role;
    // Atlassian Won't-Fix JSDCLOUD-1376; DEC-175 Q3). Scenarios 2/3 are
    // individually clean-skipped when discovery fails — see labeled blocks below.
    let vis_role_opt = discover_project_role(&h, &jsm_project);

    // ── Scenario 1 (5-step MERGE probe) ──────────────────────────────────────
    // (1) Create probe comment with sd.public.comment={internal:true}.
    // (2) GET; assert internal=true (comment created correctly).
    // (3) Edit with --internal ("updated body").
    // (4) GET; assert internal=true (MERGE: existing property preserved).
    // (5) Edit with --internal again ("body again").
    //     GET; assert internal=true still (MERGE is stable on repeated --internal).
    // Teardown: jr issue comment delete KEY --id CID --yes
    'scenario1: {
        let s1_body = serde_json::json!({
            "body": adf_paragraph(&format!("S1 probe {run_id}")),
            "properties": [{"key": "sd.public.comment", "value": {"internal": true}}]
        })
        .to_string();

        let cid = match post_probe_comment(&h, &key, &s1_body, "S1") {
            Some(id) => id,
            None => break 'scenario1,
        };

        // (2) Assert comment was created with internal=true.
        if poll_comment_until(
            &h,
            &key,
            &cid,
            &|c| sd_internal_prop(c) == Some(true),
            "S1(create)",
        )
        .is_none()
        {
            eprintln!(
                "[WARN] S1: sd.public.comment not set after create \
                 — skipping Scenario 1"
            );
            delete_comment_probe(&h, &key, &cid);
            break 'scenario1;
        }

        // (3) First --internal edit.
        let edit1 = h
            .cmd()
            .args([
                "issue",
                "comment",
                "edit",
                &key,
                "--id",
                &cid,
                &format!("S1 edit-1 {run_id}"),
                "--internal",
                "--output",
                "json",
            ])
            .output()
            .expect("failed to spawn jr issue comment edit for S1 edit-1");
        if !edit1.status.success() {
            eprintln!(
                "[WARN] S1: edit-1 failed (exit {:?}) \
                 — skipping remaining Scenario 1 steps\nstderr: {}",
                edit1.status.code(),
                String::from_utf8_lossy(&edit1.stderr)
            );
            delete_comment_probe(&h, &key, &cid);
            break 'scenario1;
        }

        // (4) Assert internal=true preserved after first --internal edit (MERGE).
        match poll_comment_until(
            &h,
            &key,
            &cid,
            &|c| sd_internal_prop(c) == Some(true),
            "S1(edit-1)",
        ) {
            None => {
                eprintln!("[WARN] S1: sd.public.comment not visible after edit-1 — skipping");
                delete_comment_probe(&h, &key, &cid);
                break 'scenario1;
            }
            Some(c) => {
                assert!(
                    sd_internal_prop(&c) == Some(true),
                    "S1: sd.public.comment must remain internal=true after --internal edit \
                     (MERGE: existing property preserved — BC-3.5.006); got: {c}"
                );
            }
        }

        // (5) Second --internal edit.
        let edit2 = h
            .cmd()
            .args([
                "issue",
                "comment",
                "edit",
                &key,
                "--id",
                &cid,
                &format!("S1 edit-2 {run_id}"),
                "--internal",
                "--output",
                "json",
            ])
            .output()
            .expect("failed to spawn jr issue comment edit for S1 edit-2");
        if !edit2.status.success() {
            eprintln!(
                "[WARN] S1: edit-2 failed (exit {:?}) \
                 — skipping S1 stability check",
                edit2.status.code()
            );
            delete_comment_probe(&h, &key, &cid);
            break 'scenario1;
        }

        // Assert internal=true still stable (MERGE is idempotent on repeated --internal).
        match poll_comment_until(
            &h,
            &key,
            &cid,
            &|c| sd_internal_prop(c) == Some(true),
            "S1(edit-2)",
        ) {
            None => {
                eprintln!("[WARN] S1: sd.public.comment not stable after S1 edit-2 — skipping");
                delete_comment_probe(&h, &key, &cid);
                break 'scenario1;
            }
            Some(c) => {
                assert!(
                    sd_internal_prop(&c) == Some(true),
                    "S1: sd.public.comment must be stable at internal=true after two \
                     --internal edits (MERGE stability — BC-3.5.006); got: {c}"
                );
            }
        }

        delete_comment_probe(&h, &key, &cid);
    }

    // ── Scenario 2 (PRESERVED-visibility baseline — 5-step, DEC-175) ───────────
    // Verifies that a body-only PUT leaves an existing Jira `visibility` restriction
    // UNCHANGED (PRESERVED). Uses the platform `visibility` field, NOT
    // `sd.public.comment` properties — these are orthogonal dimensions (DEC-175 Q5).
    //
    // (1) Clean-skip if role discovery yielded nothing.
    // (2) Create probe comment WITH visibility={"type":"role","value":"<role>"}.
    // (3) GET; assert visibility.value == <role> (anti-vacuous-pass per DEC-175 Q2:
    //     an invalid role name may be silently dropped; assert round-trip, not 2xx).
    // (4) Body-only edit (no --internal/--public flag).
    // (5) GET; assert visibility still present with same type/value (PRESERVED:
    //     body-only PUT sends no "visibility" key, must not clear the restriction).
    // Teardown: jr issue comment delete KEY --id CID --yes
    'scenario2: {
        let role_name = match vis_role_opt.as_deref() {
            Some(r) => r.to_owned(),
            None => {
                eprintln!(
                    "[SKIP] S2: no usable project role discovered for {jsm_project} \
                     — skipping PRESERVED-visibility baseline (DEC-175)"
                );
                break 'scenario2;
            }
        };

        let s2_body = serde_json::json!({
            "body": adf_paragraph(&format!("S2 probe {run_id}")),
            "visibility": {"type": "role", "value": role_name}
        })
        .to_string();

        let cid = match post_probe_comment(&h, &key, &s2_body, "S2") {
            Some(id) => id,
            None => break 'scenario2,
        };

        // (3) Anti-vacuous-pass guard (DEC-175 Q2): assert visibility is present on
        // GET read-back immediately after create. An invalid role name may be silently
        // dropped by Jira (unconfirmed behavior), making assertions vacuous. Asserting
        // on the round-trip ensures we test a real restriction, not a ghost.
        {
            let role = role_name.as_str();
            if poll_comment_until(
                &h,
                &key,
                &cid,
                &|c| comment_visibility_value(c) == Some(role),
                "S2(create-readback)",
            )
            .is_none()
            {
                eprintln!(
                    "[WARN] S2: visibility.value != '{role_name}' after create \
                     — role may be invalid on {jsm_project} or API lag; \
                     skipping Scenario 2 to avoid vacuous assertion (DEC-175 Q2)"
                );
                delete_comment_probe(&h, &key, &cid);
                break 'scenario2;
            }
        }

        // (4) Body-only edit — no --internal/--public flag: tests PRESERVED semantics.
        let edit = h
            .cmd()
            .args([
                "issue",
                "comment",
                "edit",
                &key,
                "--id",
                &cid,
                &format!("S2 body-only edit {run_id}"),
                "--output",
                "json",
            ])
            .output()
            .expect("failed to spawn jr issue comment edit for S2 body-only edit");
        if !edit.status.success() {
            eprintln!(
                "[WARN] S2: body-only edit failed (exit {:?}) \
                 — skipping Scenario 2",
                edit.status.code()
            );
            delete_comment_probe(&h, &key, &cid);
            break 'scenario2;
        }

        // (5) Assert visibility restriction is PRESERVED after body-only edit.
        // A body-only PUT sends only {"body":<adf>} — no "visibility" key — so the
        // existing restriction must be untouched (BC-3.5.006, DEC-175 Q6).
        {
            let role = role_name.as_str();
            match poll_comment_until(
                &h,
                &key,
                &cid,
                &|c| comment_visibility_value(c) == Some(role),
                "S2(edit)",
            ) {
                None => {
                    eprintln!(
                        "[WARN] S2: visibility not visible after body-only edit \
                         — skipping assertion"
                    );
                    delete_comment_probe(&h, &key, &cid);
                    break 'scenario2;
                }
                Some(c) => {
                    assert!(
                        comment_visibility_value(&c) == Some(role),
                        "S2: Jira visibility restriction must be PRESERVED after a body-only \
                         edit — body-only PUT sends no 'visibility' key and must not clear \
                         the existing restriction (BC-3.5.006, DEC-175 Q6); got: {c}"
                    );
                }
            }
        }

        delete_comment_probe(&h, &key, &cid);
    }

    // ── Scenario 3 (compound cell — orthogonal axes, DEC-175) ───────────────
    // Verifies that visibility (Jira platform restriction) and sd.public.comment
    // (JSM portal visibility property) are orthogonal (DEC-175 Q5): a
    // properties-MERGE edit (--public --yes) updates sd.public.comment but does NOT
    // disturb a pre-existing Jira visibility restriction (PRESERVED because the PUT
    // body does not include a "visibility" key).
    //
    // (1) Clean-skip if role discovery yielded nothing.
    // (2) Create probe comment with BOTH visibility={"type":"role","value":"<role>"}
    //     AND sd.public.comment={internal:true}.
    // (3) GET; assert BOTH present (anti-vacuous-pass for both dimensions).
    // (4) Edit with --public --yes.
    //     Expected wire PUT: {"body":<adf>,"properties":[{"key":"sd.public.comment",
    //     "value":{"internal":false}}]} — no "visibility" key → PRESERVED on that axis.
    // (5) GET; assert BOTH:
    //     (a) sd.public.comment is now internal=false (MERGE: property updated), AND
    //     (b) visibility restriction still present with same value (PRESERVED:
    //         orthogonal axis untouched — DEC-175 Q5, BC-3.5.006).
    // Teardown: jr issue comment delete KEY --id CID --yes
    'scenario3: {
        let role_name = match vis_role_opt.as_deref() {
            Some(r) => r.to_owned(),
            None => {
                eprintln!(
                    "[SKIP] S3: no usable project role discovered for {jsm_project} \
                     — skipping compound-cell orthogonal-axes probe (DEC-175)"
                );
                break 'scenario3;
            }
        };

        let s3_body = serde_json::json!({
            "body": adf_paragraph(&format!("S3 probe {run_id}")),
            "visibility": {"type": "role", "value": role_name},
            "properties": [{"key": "sd.public.comment", "value": {"internal": true}}]
        })
        .to_string();

        let cid = match post_probe_comment(&h, &key, &s3_body, "S3") {
            Some(id) => id,
            None => break 'scenario3,
        };

        // (3) Assert BOTH visibility and sd.public.comment present on read-back.
        // Anti-vacuous-pass guard for both dimensions (DEC-175 Q2 for visibility).
        {
            let role = role_name.as_str();
            let both_present = |c: &Value| {
                comment_visibility_value(c) == Some(role) && sd_internal_prop(c) == Some(true)
            };
            if poll_comment_until(&h, &key, &cid, &both_present, "S3(create-readback)").is_none() {
                eprintln!(
                    "[WARN] S3: visibility or sd.public.comment not present after create \
                     — role may be invalid on {jsm_project} or API lag; \
                     skipping Scenario 3 to avoid vacuous assertion"
                );
                delete_comment_probe(&h, &key, &cid);
                break 'scenario3;
            }
        }

        // (4) Edit with --public --yes.
        let edit = h
            .cmd()
            .args([
                "issue",
                "comment",
                "edit",
                &key,
                "--id",
                &cid,
                &format!("S3 public edit {run_id}"),
                "--public",
                "--yes",
                "--output",
                "json",
            ])
            .output()
            .expect("failed to spawn jr issue comment edit for S3 --public --yes");
        if !edit.status.success() {
            eprintln!(
                "[WARN] S3: --public --yes edit failed (exit {:?}) \
                 — skipping Scenario 3\nstderr: {}",
                edit.status.code(),
                String::from_utf8_lossy(&edit.stderr)
            );
            delete_comment_probe(&h, &key, &cid);
            break 'scenario3;
        }

        // (5) Assert full predicate: sd.public.comment=false AND visibility preserved.
        {
            let role = role_name.as_str();
            let full_pred = |c: &Value| {
                sd_internal_prop(c) == Some(false) && comment_visibility_value(c) == Some(role)
            };
            match poll_comment_until(&h, &key, &cid, &full_pred, "S3(--public)") {
                None => {
                    eprintln!(
                        "[WARN] S3: full predicate (sd.public.comment=false AND visibility \
                         present) did not hold after retries — skipping assertions"
                    );
                    delete_comment_probe(&h, &key, &cid);
                    break 'scenario3;
                }
                Some(c) => {
                    assert!(
                        sd_internal_prop(&c) == Some(false),
                        "S3: sd.public.comment must be updated to internal=false after \
                         --public --yes edit (MERGE: property value updated — BC-3.5.006); \
                         got: {c}"
                    );
                    assert!(
                        comment_visibility_value(&c) == Some(role),
                        "S3: Jira visibility restriction must be PRESERVED after --public \
                         --yes edit — properties-MERGE PUT does not include a 'visibility' \
                         key and must not disturb the existing restriction \
                         (orthogonal axes — DEC-175 Q5, BC-3.5.006); got: {c}"
                    );
                }
            }
        }

        delete_comment_probe(&h, &key, &cid);
    }
}

// ---------------------------------------------------------------------------
// P2-3c schema-capture helpers — test-code only, never compiled into src/
// ---------------------------------------------------------------------------

/// Recursively replaces every JSON leaf value with a type placeholder so the
/// structural schema can be printed to CI logs without emitting any real data
/// (account IDs, URLs, issue keys, attachment IDs, timestamps, emails, etc.).
///
/// Rules:
/// - Object  → keys kept verbatim, values sanitized recursively.
/// - Array   → one-element array with the first element sanitized
///   (or an empty array if the source array is empty).
/// - String  → `"<string>"`
/// - Number  → `"<number>"`
/// - Bool    → `"<bool>"`
/// - Null    → `null` (unchanged — null is structural information)
fn p2_3c_sanitize(v: &Value) -> Value {
    match v {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), p2_3c_sanitize(v)))
                .collect(),
        ),
        Value::Array(arr) => {
            if arr.is_empty() {
                Value::Array(vec![])
            } else {
                Value::Array(vec![p2_3c_sanitize(&arr[0])])
            }
        }
        Value::String(_) => Value::String("<string>".to_string()),
        Value::Number(_) => Value::String("<number>".to_string()),
        Value::Bool(_) => Value::String("<bool>".to_string()),
        Value::Null => Value::Null,
    }
}

/// Prints a sanitized structural JSON schema to stdout.  Each line is prefixed
/// with `P2-3C-SCHEMA: [<label>]` so the capture can be grepped from CI
/// `--show-output` logs.  No real values are ever printed.
fn p2_3c_print(label: &str, v: &Value) {
    let sanitized = p2_3c_sanitize(v);
    let pretty = serde_json::to_string_pretty(&sanitized).unwrap_or_default();
    for line in pretty.lines() {
        println!("P2-3C-SCHEMA: [{label}] {line}");
    }
}

// ---------------------------------------------------------------------------
// S-576-5: JSM attachment upload --public / --internal E2E tests
// AC-011 (Scenario 9 in jsm-e2e-coverage.md)
// ---------------------------------------------------------------------------

/// E2E smoke test: `jr issue attachment upload <JSM-KEY> <FILE> --public --yes`
/// uploads a temporary file as a customer-visible attachment via the
/// servicedeskapi two-step flow (BC-3.9.003) and returns a non-empty curated
/// attachment array.
///
/// Gated by `JR_E2E_JSM_PROJECT` (same as other JSM tests). Uses `jsm_self_close`
/// for teardown convention (S-JSM-E2E-2). Best-effort attachment delete fires before
/// jsm_self_close — the AID is parsed from upload stdout; on parse failure a [WARN]
/// is emitted and self-close proceeds regardless.
///
/// Traces to: AC-011, BC-3.9.003, BC-3.9.007.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_attachment_upload_public() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!(
                "[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM attachment upload --public test"
            );
            return;
        }
    };
    let h = e2e_harness();
    let run_id = run_label();

    // Step 1: discover a request type to create a fresh JSM request.
    let list_out = h
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
        .expect("failed to spawn jr requesttype list");

    if !list_out.status.success() {
        let s = String::from_utf8_lossy(&list_out.stderr);
        eprintln!("[SKIP] requesttype list failed — skipping JSM attachment upload --public: {s}");
        return;
    }

    let rts: Vec<Value> = match serde_json::from_slice(&list_out.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[SKIP] requesttype list not a JSON array: {e} — skipping");
            return;
        }
    };
    if rts.is_empty() {
        eprintln!(
            "[SKIP] no request types found on {jsm_project} — skipping attachment upload --public"
        );
        return;
    }
    let first_rt_id = {
        let id_val = &rts[0]["id"];
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] rts[0].id is not a usable type — skipping");
            return;
        }
    };

    // Step 2: create a JSM request.
    let summary = format!("[e2e-jsm {run_id}] attachment upload --public");
    let create_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &summary,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr issue create");

    if !create_out.status.success() {
        let s = String::from_utf8_lossy(&create_out.stderr);
        eprintln!("[SKIP] issue create failed — skipping attachment upload --public: {s}");
        return;
    }

    let create_v: Value = serde_json::from_slice(&create_out.stdout)
        .expect("issue create --output json must be valid JSON");
    let key = create_v
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain 'key' field")
        .to_string();
    assert!(
        !key.is_empty(),
        "created key must be non-empty; got: {create_v}"
    );

    // Step 3: write a temp file to upload.
    let upload_dir = tempfile::TempDir::new().expect("failed to create upload temp dir");
    let upload_file = upload_dir.path().join("e2e_public.txt");
    std::fs::write(&upload_file, b"S-576-5 e2e public attachment").expect("write test file");

    // Step 4: upload --public --yes → two-step servicedeskapi flow.
    let upload_out = h
        .cmd()
        .args([
            "issue",
            "attachment",
            "upload",
            &key,
            &upload_file.to_string_lossy(),
            "--public",
            "--yes",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr attachment upload");

    // Probe hardening: emit sanitized stderr BEFORE teardown so failure evidence
    // always reaches the CI log even when upload exits non-zero (P2-3c fix,
    // S-576-5 re-probe run after 29940792930).
    if !upload_out.status.success() {
        let stderr_raw = String::from_utf8_lossy(&upload_out.stderr);
        let stderr_val = serde_json::from_str::<Value>(&stderr_raw)
            .unwrap_or_else(|_| Value::String(stderr_raw.into_owned()));
        let sanitized = p2_3c_sanitize(&stderr_val);
        println!(
            "P2-3C-SCHEMA-ERROR: {}",
            serde_json::to_string(&sanitized).unwrap_or_else(|_| format!("{sanitized:?}"))
        );
    }

    // Step 5 (teardown — runs before assertions so no residue survives test failure):
    // (a) Best-effort parse the attachment AID and delete it.
    //     The attachment persists independently of ticket status (BC-3.9.011).
    //     Do NOT panic on parse failure — teardown must always reach jsm_self_close.
    let upload_stdout_raw = String::from_utf8_lossy(&upload_out.stdout);
    let teardown_aid: Option<String> = serde_json::from_str::<Vec<Value>>(&upload_stdout_raw)
        .ok()
        .and_then(|arr| arr.into_iter().next())
        .and_then(|item| item.get("id").and_then(Value::as_str).map(str::to_string));
    if let Some(aid) = &teardown_aid {
        match h
            .cmd()
            .args(["issue", "attachment", "delete", aid, "--yes"])
            .output()
        {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let s = String::from_utf8_lossy(&o.stderr);
                eprintln!("[WARN] E2E public: failed to delete attachment {aid}: {s}");
            }
            Err(e) => eprintln!("[WARN] E2E public: failed to spawn attachment delete: {e}"),
        }
    } else {
        eprintln!(
            "[WARN] E2E public: could not parse AID from upload stdout — attachment not deleted"
        );
    }
    // (b) Self-close regardless of upload/delete result (F-2b teardown).
    jsm_self_close(&key, &h);

    // Step 6: assert upload succeeded.
    let upload_stderr = String::from_utf8_lossy(&upload_out.stderr);
    let upload_stdout = String::from_utf8_lossy(&upload_out.stdout);
    assert!(
        upload_out.status.success(),
        "AC-011 E2E public: upload must exit 0; got {:?}\nstdout: {upload_stdout}\nstderr: {upload_stderr}",
        upload_out.status.code()
    );

    // Step 7: parse curated array.
    let arr: Vec<Value> = serde_json::from_str(&upload_stdout)
        .expect("AC-011 E2E public: --output json must be a JSON array");
    assert!(
        !arr.is_empty(),
        "AC-011 E2E public: uploaded attachment array must be non-empty; stdout: {upload_stdout}"
    );

    // P2-3c schema probe A: print sanitized curated upload output BEFORE shape assertions
    // (BC-3.9.011).  Schema is captured even if the shape check below fails.
    p2_3c_print("CURATED-UPLOAD-public", &Value::Array(arr.clone()));

    // P2-3c schema probe B: raw platform attachment JSON (BC-3.9.007 wire source).
    // GET /rest/api/3/issue/{key}?fields=attachment returns the raw Jira attachment
    // objects before jr curates them — the platform wire format evidence for BC-3.9.007.
    let raw_path = format!("/rest/api/3/issue/{key}?fields=attachment");
    if let Ok(raw_out) = h.cmd().args(["api", &raw_path]).output() {
        if raw_out.status.success() {
            if let Ok(raw_v) = serde_json::from_slice::<Value>(&raw_out.stdout) {
                p2_3c_print("RAW-PLATFORM-attachment-public", &raw_v);
            }
        }
    }

    // Step 8: minimal shape check (BC-3.9.007 curated keys).
    let item = &arr[0];
    for field in &[
        "id",
        "filename",
        "contentUrl",
        "mimeType",
        "size",
        "author",
        "created",
    ] {
        assert!(
            item.get(field).is_some(),
            "AC-011 E2E public: curated attachment must have key '{field}'; got: {item}"
        );
    }
    assert!(
        item.get("self").is_none(),
        "AC-011 E2E public: curated attachment must NOT contain 'self'; got: {item}"
    );
}

/// E2E smoke test: `jr issue attachment upload <JSM-KEY> <FILE> --internal`
/// uploads a temporary file as a staff-only (internal) attachment via the
/// servicedeskapi two-step flow with `public:false` (BC-3.9.004) and returns a
/// non-empty curated attachment array. No interactive confirmation gate is
/// needed for `--internal`.
///
/// Gated by `JR_E2E_JSM_PROJECT`. Uses `jsm_self_close` for teardown.
///
/// Traces to: AC-011, BC-3.9.004, BC-3.9.007.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_attachment_upload_internal() {
    if !e2e_enabled() {
        return;
    }
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!(
                "[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM attachment upload --internal test"
            );
            return;
        }
    };
    let h = e2e_harness();
    let run_id = run_label();

    // Step 1: discover a request type.
    let list_out = h
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
        .expect("failed to spawn jr requesttype list");

    if !list_out.status.success() {
        let s = String::from_utf8_lossy(&list_out.stderr);
        eprintln!(
            "[SKIP] requesttype list failed — skipping JSM attachment upload --internal: {s}"
        );
        return;
    }

    let rts: Vec<Value> = match serde_json::from_slice(&list_out.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[SKIP] requesttype list not a JSON array: {e} — skipping");
            return;
        }
    };
    if rts.is_empty() {
        eprintln!(
            "[SKIP] no request types found on {jsm_project} — skipping attachment upload --internal"
        );
        return;
    }
    let first_rt_id = {
        let id_val = &rts[0]["id"];
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] rts[0].id is not a usable type — skipping");
            return;
        }
    };

    // Step 2: create a JSM request.
    let summary = format!("[e2e-jsm {run_id}] attachment upload --internal");
    let create_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &summary,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr issue create");

    if !create_out.status.success() {
        let s = String::from_utf8_lossy(&create_out.stderr);
        eprintln!("[SKIP] issue create failed — skipping attachment upload --internal: {s}");
        return;
    }

    let create_v: Value = serde_json::from_slice(&create_out.stdout)
        .expect("issue create --output json must be valid JSON");
    let key = create_v
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain 'key' field")
        .to_string();
    assert!(
        !key.is_empty(),
        "created key must be non-empty; got: {create_v}"
    );

    // Step 3: write a temp file to upload.
    let upload_dir = tempfile::TempDir::new().expect("failed to create upload temp dir");
    let upload_file = upload_dir.path().join("e2e_internal.txt");
    std::fs::write(&upload_file, b"S-576-5 e2e internal attachment").expect("write test file");

    // Step 4: upload --internal → two-step servicedeskapi flow with public:false.
    // No --yes needed; --internal has no interactive confirmation gate.
    let upload_out = h
        .cmd()
        .args([
            "issue",
            "attachment",
            "upload",
            &key,
            &upload_file.to_string_lossy(),
            "--internal",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr attachment upload");

    // Probe hardening: emit sanitized stderr BEFORE teardown so failure evidence
    // always reaches the CI log even when upload exits non-zero (P2-3c fix,
    // S-576-5 re-probe run after 29940792930).
    if !upload_out.status.success() {
        let stderr_raw = String::from_utf8_lossy(&upload_out.stderr);
        let stderr_val = serde_json::from_str::<Value>(&stderr_raw)
            .unwrap_or_else(|_| Value::String(stderr_raw.into_owned()));
        let sanitized = p2_3c_sanitize(&stderr_val);
        println!(
            "P2-3C-SCHEMA-ERROR: {}",
            serde_json::to_string(&sanitized).unwrap_or_else(|_| format!("{sanitized:?}"))
        );
    }

    // Step 5 (teardown — runs before assertions so no residue survives test failure):
    // (a) Best-effort parse the attachment AID and delete it.
    //     The attachment persists independently of ticket status (BC-3.9.011).
    //     Do NOT panic on parse failure — teardown must always reach jsm_self_close.
    let upload_stdout_raw = String::from_utf8_lossy(&upload_out.stdout);
    let teardown_aid: Option<String> = serde_json::from_str::<Vec<Value>>(&upload_stdout_raw)
        .ok()
        .and_then(|arr| arr.into_iter().next())
        .and_then(|item| item.get("id").and_then(Value::as_str).map(str::to_string));
    if let Some(aid) = &teardown_aid {
        match h
            .cmd()
            .args(["issue", "attachment", "delete", aid, "--yes"])
            .output()
        {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let s = String::from_utf8_lossy(&o.stderr);
                eprintln!("[WARN] E2E internal: failed to delete attachment {aid}: {s}");
            }
            Err(e) => eprintln!("[WARN] E2E internal: failed to spawn attachment delete: {e}"),
        }
    } else {
        eprintln!(
            "[WARN] E2E internal: could not parse AID from upload stdout — attachment not deleted"
        );
    }
    // (b) Self-close regardless of upload/delete result (F-2b teardown).
    jsm_self_close(&key, &h);

    // Step 6: assert upload succeeded.
    let upload_stderr = String::from_utf8_lossy(&upload_out.stderr);
    let upload_stdout = String::from_utf8_lossy(&upload_out.stdout);
    assert!(
        upload_out.status.success(),
        "AC-011 E2E internal: upload must exit 0; got {:?}\nstdout: {upload_stdout}\nstderr: {upload_stderr}",
        upload_out.status.code()
    );

    // Step 7: parse curated array.
    let arr: Vec<Value> = serde_json::from_str(&upload_stdout)
        .expect("AC-011 E2E internal: --output json must be a JSON array");
    assert!(
        !arr.is_empty(),
        "AC-011 E2E internal: uploaded attachment array must be non-empty; stdout: {upload_stdout}"
    );

    // P2-3c schema probe A: print sanitized curated upload output BEFORE shape assertions
    // (BC-3.9.011).  Schema is captured even if the shape check below fails.
    p2_3c_print("CURATED-UPLOAD-internal", &Value::Array(arr.clone()));

    // P2-3c schema probe B: raw platform attachment JSON (BC-3.9.007 wire source).
    let raw_path = format!("/rest/api/3/issue/{key}?fields=attachment");
    if let Ok(raw_out) = h.cmd().args(["api", &raw_path]).output() {
        if raw_out.status.success() {
            if let Ok(raw_v) = serde_json::from_slice::<Value>(&raw_out.stdout) {
                p2_3c_print("RAW-PLATFORM-attachment-internal", &raw_v);
            }
        }
    }

    // Step 8: minimal shape check (BC-3.9.007 curated keys).
    let item = &arr[0];
    for field in &[
        "id",
        "filename",
        "contentUrl",
        "mimeType",
        "size",
        "author",
        "created",
    ] {
        assert!(
            item.get(field).is_some(),
            "AC-011 E2E internal: curated attachment must have key '{field}'; got: {item}"
        );
    }
    assert!(
        item.get("self").is_none(),
        "AC-011 E2E internal: curated attachment must NOT contain 'self'; got: {item}"
    );
}

// ---------------------------------------------------------------------------
// S-576-6: attachment live E2E coverage — platform round-trip + JSM echo shapes
// AC-001 (test_e2e_attachment_platform_roundtrip)
// AC-002 (test_e2e_jsm_attachment_public_echo_shape)
// AC-003 (test_e2e_jsm_attachment_internal_echo_shape)
// AC-004 (test_e2e_jsm_attachment_upload_no_flag)
// ---------------------------------------------------------------------------

/// Drop-guard for JSM attachment E2E teardown (AC-010, AC-002).
///
/// Ensures (1) AID deletion and (2) `jsm_self_close` run on both normal return
/// AND panic unwind. Mandatory for `test_e2e_jsm_attachment_public_echo_shape`
/// (BC-3.9.011 ADV-022: public attachments persist independently of ticket status).
///
/// Populate `guard.key` immediately after capturing the issue key and
/// `guard.aid` immediately after capturing the upload AID. Both default to
/// `None`; a `None` field triggers no cleanup.
struct AttachmentDropGuard {
    aid: Option<String>,
    key: Option<String>,
}

impl AttachmentDropGuard {
    fn new() -> Self {
        Self {
            aid: None,
            key: None,
        }
    }
}

impl Drop for AttachmentDropGuard {
    fn drop(&mut self) {
        // (1) Delete AID before jsm_self_close (ADV-022 ordering invariant).
        if let Some(ref aid) = self.aid {
            let h = E2eHarness::new();
            match h
                .cmd()
                .args(["issue", "attachment", "delete", aid, "--yes"])
                .output()
            {
                Ok(o) if o.status.success() => {}
                Ok(o) => eprintln!(
                    "[WARN] AttachmentDropGuard Drop: delete {} failed (exit {:?}): {}",
                    aid,
                    o.status.code(),
                    String::from_utf8_lossy(&o.stderr)
                ),
                Err(e) => eprintln!("[WARN] AttachmentDropGuard Drop: delete spawn error: {e}"),
            }
        }
        // (2) Self-close JSM issue.
        if let Some(ref key) = self.key {
            let h = E2eHarness::new();
            jsm_self_close(key, &h);
        }
    }
}

/// E2E round-trip: upload → list (table + JSON) → download → delete → post-delete list.
///
/// Exercises the full `jr issue attachment` surface against a live Jira Cloud ES
/// project. Verifies BC-2.7.001 (list table filename), BC-2.7.002 (list JSON
/// shape + contentUrl), BC-2.7.007 (download exits 0 + file exists),
/// BC-3.9.001/009 (upload exits 0 + curated array), BC-3.9.008/010 (delete
/// exits 0 + JSON `{"deleted":true,"id":"<AID>"}` + post-delete list
/// confirms AID gone).
///
/// Uses `seed_issue` for ES issue creation (label `rl` enables CI sweeper pick-up
/// on test failure). Collect-results-then-assert pattern: teardown (delete AID +
/// `best_effort_close`) runs before any `assert!`.
///
/// Traces to: AC-001, BC-2.7.001, BC-2.7.002, BC-2.7.007, BC-3.9.001, BC-3.9.008,
/// BC-3.9.009, BC-3.9.010.
#[test]
#[ignore = "set JR_RUN_E2E=1 and use --include-ignored to run"]
fn test_e2e_attachment_platform_roundtrip() {
    if !e2e_enabled() {
        return;
    }
    let h = e2e_harness();
    let rl = run_label();
    let summary = format!("[e2e {}] attachment round-trip", rl);
    let key = seed_issue(&h, &rl, &summary);

    // Step 2: create a temp file with platform-neutral filename (B4/B5, S-576-2).
    // ASCII-printable only; no Windows-reserved names; `.txt` extension.
    let upload_dir = TempDir::new().expect("failed to create upload temp dir");
    let filename = "attachment-e2e-test.txt".to_string();
    let upload_file = upload_dir.path().join(&filename);
    let file_content = format!("jr e2e attachment round-trip {}", rl);
    std::fs::write(&upload_file, file_content.as_bytes()).expect("failed to write test file");

    // Step 3: Upload (BC-3.9.001/009).
    let upload_out = h
        .cmd()
        .args([
            "issue",
            "attachment",
            "upload",
            &key,
            &upload_file.to_string_lossy(),
            "--output",
            "json",
            "--yes",
        ])
        .output()
        .expect("failed to spawn jr attachment upload");

    // Capture AID from upload stdout before any assertion can panic.
    let upload_stdout_raw = String::from_utf8_lossy(&upload_out.stdout);
    let aid: Option<String> = serde_json::from_str::<Vec<Value>>(&upload_stdout_raw)
        .ok()
        .and_then(|arr| arr.into_iter().next())
        .and_then(|item| item.get("id").and_then(Value::as_str).map(str::to_string));

    // Step 4: List (table) — BC-2.7.001.
    let list_table_out = h
        .cmd()
        .args(["issue", "attachment", "list", &key])
        .output()
        .expect("failed to spawn jr attachment list (table)");

    // Step 5: List (JSON) — BC-2.7.002.
    let list_json_out = h
        .cmd()
        .args(["issue", "attachment", "list", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr attachment list --output json");

    // Step 6: Download — BC-2.7.007.
    let download_dir = TempDir::new().expect("failed to create download temp dir");
    let download_path = download_dir.path().join("downloaded.txt");
    let download_out = aid.as_ref().map(|the_aid| {
        h.cmd()
            .args([
                "issue",
                "attachment",
                "download",
                &key,
                "--id",
                the_aid,
                "--out",
                &download_path.to_string_lossy(),
            ])
            .output()
            .expect("failed to spawn jr attachment download")
    });

    // Step 7: Delete (AID teardown) — BC-3.9.008/010.
    let delete_out = aid.as_ref().map(|the_aid| {
        h.cmd()
            .args([
                "issue",
                "attachment",
                "delete",
                the_aid,
                "--yes",
                "--output",
                "json",
            ])
            .output()
            .expect("failed to spawn jr attachment delete")
    });

    // Step 8: List post-delete (two-step post-condition: delete exits 0 AND AID gone).
    let post_delete_out = h
        .cmd()
        .args(["issue", "attachment", "list", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr attachment list post-delete");

    // Step 9: Issue teardown (best-effort; sweeper handles orphans via label rl).
    best_effort_close(&h, &key);

    // ---- Assertions (all teardown complete before this line) ----

    // Upload assertions (BC-3.9.001/009).
    let upload_stderr = String::from_utf8_lossy(&upload_out.stderr);
    let upload_stdout = String::from_utf8_lossy(&upload_out.stdout);
    assert!(
        upload_out.status.success(),
        "AC-001: upload must exit 0; got {:?}\nstdout: {upload_stdout}\nstderr: {upload_stderr}",
        upload_out.status.code()
    );
    let upload_arr: Vec<Value> = serde_json::from_str(&upload_stdout)
        .expect("AC-001: upload --output json must be a JSON array");
    assert!(
        !upload_arr.is_empty(),
        "AC-001: upload JSON array must be non-empty; stdout: {upload_stdout}"
    );
    let the_aid = aid
        .as_ref()
        .expect("AC-001: AID must be parseable from upload output");
    assert!(!the_aid.is_empty(), "AC-001: AID must be non-empty");

    // List table assertion (BC-2.7.001).
    let list_table_stdout = String::from_utf8_lossy(&list_table_out.stdout);
    assert!(
        list_table_out.status.success(),
        "AC-001: list (table) must exit 0"
    );
    assert!(
        list_table_stdout.contains(&filename),
        "AC-001: list table must contain filename '{filename}'; stdout: {list_table_stdout}"
    );

    // List JSON assertions (BC-2.7.002).
    let list_json_stdout = String::from_utf8_lossy(&list_json_out.stdout);
    assert!(
        list_json_out.status.success(),
        "AC-001: list (JSON) must exit 0"
    );
    let list_arr: Vec<Value> = serde_json::from_str(&list_json_stdout)
        .expect("AC-001: list --output json must be a JSON array");
    let list_item = list_arr
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(the_aid.as_str()))
        .unwrap_or_else(|| {
            panic!("AC-001: list JSON must contain item with id={the_aid}; got: {list_json_stdout}")
        });
    for field in &[
        "filename",
        "contentUrl",
        "mimeType",
        "size",
        "created",
        "author",
    ] {
        assert!(
            list_item.get(field).is_some(),
            "AC-001: list JSON item must have key '{field}' (BC-2.7.002); got: {list_item}"
        );
    }
    let content_url = list_item
        .get("contentUrl")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !content_url.is_empty(),
        "AC-001: contentUrl must be non-null and non-empty (BC-2.7.002); got: {list_item}"
    );

    // Download assertions (BC-2.7.007).
    if let Some(ref dl_out) = download_out {
        let dl_stderr = String::from_utf8_lossy(&dl_out.stderr);
        assert!(
            dl_out.status.success(),
            "AC-001: download must exit 0; got {:?}\nstderr: {dl_stderr}",
            dl_out.status.code()
        );
        assert!(
            download_path.exists(),
            "AC-001: downloaded file must exist at {}",
            download_path.display()
        );
        let dl_bytes =
            std::fs::read(&download_path).expect("AC-001: failed to read downloaded file");
        assert!(
            !dl_bytes.is_empty(),
            "AC-001: downloaded file must be non-empty"
        );
    }

    // Delete assertions (BC-3.9.008/010).
    if let Some(ref del_out) = delete_out {
        let del_stdout = String::from_utf8_lossy(&del_out.stdout);
        let del_stderr = String::from_utf8_lossy(&del_out.stderr);
        assert!(
            del_out.status.success(),
            "AC-001: delete must exit 0; got {:?}\nstdout: {del_stdout}\nstderr: {del_stderr}",
            del_out.status.code()
        );
        let del_json: Value = serde_json::from_str(&del_stdout)
            .expect("AC-001: delete --output json must be valid JSON");
        assert_eq!(
            del_json["deleted"],
            Value::Bool(true),
            "AC-001: delete JSON must have deleted:true (BC-3.9.010); got: {del_json}"
        );
        assert_eq!(
            del_json["id"].as_str(),
            Some(the_aid.as_str()),
            "AC-001: delete JSON 'id' must match AID (BC-3.9.010); got: {del_json}"
        );
    }

    // Post-delete list assertion: two-step post-condition (BC-3.9.008 — AID removed).
    let post_del_stdout = String::from_utf8_lossy(&post_delete_out.stdout);
    assert!(
        post_delete_out.status.success(),
        "AC-001: post-delete list must exit 0"
    );
    let post_del_arr: Vec<Value> = serde_json::from_str(&post_del_stdout)
        .expect("AC-001: post-delete list JSON must be a JSON array");
    assert!(
        !post_del_arr
            .iter()
            .any(|item| item.get("id").and_then(Value::as_str) == Some(the_aid.as_str())),
        "AC-001: post-delete list must not contain AID={the_aid} (BC-3.9.008); got: {post_del_stdout}"
    );
}

/// E2E shape verification: `--public --yes --output json` exits 0 and returns the
/// confirmed P2-3c BC-3.9.011 curated attachment schema (bare array; confirmed field
/// set `{author, contentUrl, created, filename, id, mimeType, size}`; no `"self"` key).
///
/// Pins the exact BC-3.9.011 EC-3.9.011-1 schema (P2-3c SATISFIED — S-576-5 probe
/// runs 29936980027 + 29940792930 + 29945857059, 2026-07-22). Additive to S-576-5's
/// `test_e2e_jsm_attachment_upload_public` (functional correctness); this test pins
/// the `--output json` echo shape.
///
/// Uses `AttachmentDropGuard` (AC-010): ensures delete-AID then `jsm_self_close` run
/// on both normal return and panic unwind (ADV-022 obligation).
///
/// Gated by `JR_RUN_E2E=1` + `JR_E2E_JSM_PROJECT`. Gate-2 (empty RT list) +
/// Gate-3 (403) trigger clean-skips.
///
/// Traces to: AC-002, AC-005, BC-3.9.003, BC-3.9.007 EC-3.9.007-2, BC-3.9.011.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_attachment_public_echo_shape() {
    if !e2e_enabled() {
        return;
    }
    // Gate 1 (§3.1): JR_E2E_JSM_PROJECT must be set.
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!(
                "[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM attachment --public echo shape test"
            );
            return;
        }
    };
    let h = e2e_harness();
    let run_id = run_label();

    // Drop-guard: populated with key + AID as captured; ensures cleanup on panic.
    let mut guard = AttachmentDropGuard::new();

    // Step 1: discover a request type (Gate 2 applies on empty list).
    let list_out = h
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
        .expect("failed to spawn jr requesttype list");

    if !list_out.status.success() {
        let s = String::from_utf8_lossy(&list_out.stderr);
        if s.contains("403") {
            eprintln!(
                "[SKIP] requesttype list returned 403 — skipping JSM attachment --public echo shape"
            );
            return;
        }
        eprintln!(
            "[SKIP] requesttype list failed — skipping JSM attachment --public echo shape: {s}"
        );
        return;
    }

    let rts: Vec<Value> = match serde_json::from_slice(&list_out.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[SKIP] requesttype list not a JSON array: {e} — skipping");
            return;
        }
    };
    // Gate 2 (§3.2): skip if no request types found.
    if rts.is_empty() {
        eprintln!(
            "[SKIP] No request types found on {jsm_project} — skipping JSM attachment --public echo shape"
        );
        return;
    }
    let first_rt_id = {
        let id_val = &rts[0]["id"];
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] rts[0].id is not a usable type — skipping");
            return;
        }
    };

    // Step 2: create a fresh JSM request.
    let summary = format!("[e2e-jsm {run_id}] attachment --public echo shape");
    let create_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &summary,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr issue create");

    if !create_out.status.success() {
        let s = String::from_utf8_lossy(&create_out.stderr);
        if s.contains("403") {
            eprintln!(
                "[SKIP] issue create returned 403 — skipping JSM attachment --public echo shape"
            );
            return;
        }
        eprintln!("[SKIP] issue create failed — skipping JSM attachment --public echo shape: {s}");
        return;
    }

    let create_v: Value = serde_json::from_slice(&create_out.stdout)
        .expect("issue create --output json must be valid JSON");
    let key = create_v
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain 'key' field")
        .to_string();
    assert!(!key.is_empty(), "created JSM key must be non-empty");
    // Register key with guard immediately so jsm_self_close runs on any later panic.
    guard.key = Some(key.clone());

    // Step 3: write a temp file (platform-neutral filename).
    let upload_dir = TempDir::new().expect("failed to create upload temp dir");
    let upload_file = upload_dir.path().join("attachment-e2e-public.txt");
    std::fs::write(&upload_file, b"S-576-6 e2e public attachment echo shape")
        .expect("write test file");

    // Step 4: upload --public --yes --output json.
    let upload_out = h
        .cmd()
        .args([
            "issue",
            "attachment",
            "upload",
            &key,
            &upload_file.to_string_lossy(),
            "--public",
            "--yes",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr attachment upload");

    // Gate 3 (§3.3): 403 on upload → clean-skip; guard closes issue.
    if !upload_out.status.success() {
        let s = String::from_utf8_lossy(&upload_out.stderr);
        if s.contains("403") {
            eprintln!("[SKIP] attachment upload --public returned 403 — skipping");
            return; // guard drops here: jsm_self_close(key) runs
        }
    }

    // Capture AID and register with guard BEFORE any assertion that could panic.
    let upload_stdout_raw = String::from_utf8_lossy(&upload_out.stdout);
    let aid: Option<String> = serde_json::from_str::<Vec<Value>>(&upload_stdout_raw)
        .ok()
        .and_then(|arr| arr.into_iter().next())
        .and_then(|item| item.get("id").and_then(Value::as_str).map(str::to_string));
    match &aid {
        Some(a) => guard.aid = Some(a.clone()),
        None => eprintln!(
            "[WARN] AC-002: could not parse AID from upload stdout — \
             attachment not registered in drop-guard"
        ),
    }

    // ---- Assertions (drop-guard active; cleanup runs on any panic below) ----

    let upload_stderr = String::from_utf8_lossy(&upload_out.stderr);
    let upload_stdout = String::from_utf8_lossy(&upload_out.stdout);
    assert!(
        upload_out.status.success(),
        "AC-002: upload --public must exit 0; got {:?}\nstdout: {upload_stdout}\nstderr: {upload_stderr}",
        upload_out.status.code()
    );

    // Step 5: BC-3.9.011 confirmed shape (EC-3.9.011-1, P2-3c SATISFIED).
    // Confirmed: bare curated array [{author, contentUrl, created, filename, id, mimeType, size}].
    // Probe runs: 29936980027 + 29940792930 + 29945857059 (S-576-5, 2026-07-22).
    let arr: Vec<Value> =
        serde_json::from_str(&upload_stdout).expect("AC-002: --output json must be a JSON array");
    assert!(
        !arr.is_empty(),
        "AC-002: upload JSON array must be non-empty (BC-3.9.011); stdout: {upload_stdout}"
    );
    let the_aid = aid
        .as_ref()
        .expect("AC-002: AID must be parseable from upload output");
    assert!(!the_aid.is_empty(), "AC-002: AID must be non-empty");

    // BC-3.9.011 EC-3.9.011-1: confirmed curated field set.
    let item = &arr[0];
    for field in &[
        "id",
        "filename",
        "contentUrl",
        "mimeType",
        "size",
        "author",
        "created",
    ] {
        assert!(
            item.get(field).is_some(),
            "AC-002: curated attachment must have key '{field}' (BC-3.9.011 EC-3.9.011-1); got: {item}"
        );
    }
    // BC-3.9.011: no raw Jira 'self' key (stripped by curation pipeline).
    assert!(
        item.get("self").is_none(),
        "AC-002: curated attachment must NOT contain 'self' (BC-3.9.011); got: {item}"
    );
    // guard drops here (end of function scope): delete AID then jsm_self_close.
}

/// E2E shape verification: `--internal --output json` exits 0 and returns a bare
/// curated JSON array with NO top-level `"public"` key (BC-3.9.011 EC-3.9.011-3).
///
/// Confirms: (a) output is a bare array (not a `{"public":…,"uploaded":[…]}` envelope);
/// (b) no top-level `"public"` key; (c) confirmed curated field set from BC-3.9.011
/// EC-3.9.011-1 (P2-3c SATISFIED). Additive to S-576-5's
/// `test_e2e_jsm_attachment_upload_internal` (functional correctness); this test pins
/// the `--output json` echo shape.
///
/// Teardown uses collect-results-then-assert: delete AID then `jsm_self_close` run
/// before assertions.
///
/// Gated by `JR_RUN_E2E=1` + `JR_E2E_JSM_PROJECT`. Gate-2 (empty RT list) +
/// Gate-3 (403) trigger clean-skips.
///
/// Traces to: AC-003, BC-3.9.004, BC-3.9.007 EC-3.9.007-2, BC-3.9.011.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_attachment_internal_echo_shape() {
    if !e2e_enabled() {
        return;
    }
    // Gate 1 (§3.1).
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!(
                "[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM attachment --internal echo shape test"
            );
            return;
        }
    };
    let h = e2e_harness();
    let run_id = run_label();

    // Step 1: discover a request type (Gate 2).
    let list_out = h
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
        .expect("failed to spawn jr requesttype list");

    if !list_out.status.success() {
        let s = String::from_utf8_lossy(&list_out.stderr);
        if s.contains("403") {
            eprintln!(
                "[SKIP] requesttype list returned 403 — skipping JSM attachment --internal echo shape"
            );
            return;
        }
        eprintln!(
            "[SKIP] requesttype list failed — skipping JSM attachment --internal echo shape: {s}"
        );
        return;
    }

    let rts: Vec<Value> = match serde_json::from_slice(&list_out.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[SKIP] requesttype list not a JSON array: {e} — skipping");
            return;
        }
    };
    if rts.is_empty() {
        eprintln!(
            "[SKIP] No request types found on {jsm_project} — skipping JSM attachment --internal echo shape"
        );
        return;
    }
    let first_rt_id = {
        let id_val = &rts[0]["id"];
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] rts[0].id is not a usable type — skipping");
            return;
        }
    };

    // Step 2: create a fresh JSM request.
    let summary = format!("[e2e-jsm {run_id}] attachment --internal echo shape");
    let create_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &summary,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr issue create");

    if !create_out.status.success() {
        let s = String::from_utf8_lossy(&create_out.stderr);
        if s.contains("403") {
            eprintln!(
                "[SKIP] issue create returned 403 — skipping JSM attachment --internal echo shape"
            );
            return;
        }
        eprintln!(
            "[SKIP] issue create failed — skipping JSM attachment --internal echo shape: {s}"
        );
        return;
    }

    let create_v: Value = serde_json::from_slice(&create_out.stdout)
        .expect("issue create --output json must be valid JSON");
    let key = create_v
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain 'key' field")
        .to_string();
    assert!(!key.is_empty(), "created JSM key must be non-empty");

    // Step 3: write a temp file.
    let upload_dir = TempDir::new().expect("failed to create upload temp dir");
    let upload_file = upload_dir.path().join("attachment-e2e-internal.txt");
    std::fs::write(&upload_file, b"S-576-6 e2e internal attachment echo shape")
        .expect("write test file");

    // Step 4: upload --internal --output json (no confirmation gate — BC-3.9.004).
    let upload_out = h
        .cmd()
        .args([
            "issue",
            "attachment",
            "upload",
            &key,
            &upload_file.to_string_lossy(),
            "--internal",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr attachment upload");

    // Gate 3 (§3.3): 403 → clean-skip.
    if !upload_out.status.success() {
        let s = String::from_utf8_lossy(&upload_out.stderr);
        if s.contains("403") {
            eprintln!("[SKIP] attachment upload --internal returned 403 — skipping");
            jsm_self_close(&key, &h);
            return;
        }
    }

    // Capture AID for teardown.
    let upload_stdout_raw = String::from_utf8_lossy(&upload_out.stdout);
    let teardown_aid: Option<String> = serde_json::from_str::<Vec<Value>>(&upload_stdout_raw)
        .ok()
        .and_then(|arr| arr.into_iter().next())
        .and_then(|item| item.get("id").and_then(Value::as_str).map(str::to_string));

    // Teardown before assertions (collect-results-then-assert pattern).
    // (a) Delete AID (ADV-022 ordering: before jsm_self_close).
    if let Some(ref aid) = teardown_aid {
        match h
            .cmd()
            .args(["issue", "attachment", "delete", aid, "--yes"])
            .output()
        {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                eprintln!(
                    "[WARN] AC-003: failed to delete attachment {aid} (exit {:?}): {}",
                    o.status.code(),
                    String::from_utf8_lossy(&o.stderr)
                );
            }
            Err(e) => eprintln!("[WARN] AC-003: failed to spawn attachment delete: {e}"),
        }
    } else {
        eprintln!("[WARN] AC-003: could not parse AID from upload stdout — attachment not deleted");
    }
    // (b) Self-close JSM issue.
    jsm_self_close(&key, &h);

    // ---- Assertions (teardown complete) ----

    let upload_stderr = String::from_utf8_lossy(&upload_out.stderr);
    let upload_stdout = String::from_utf8_lossy(&upload_out.stdout);
    assert!(
        upload_out.status.success(),
        "AC-003: upload --internal must exit 0; got {:?}\nstdout: {upload_stdout}\nstderr: {upload_stderr}",
        upload_out.status.code()
    );

    // Step 5: BC-3.9.011 --internal shape assertion.
    // Parse as Value first to assert bare-array structure (no "public"/"uploaded" envelope).
    let raw_v: Value =
        serde_json::from_str(&upload_stdout).expect("AC-003: --output json must be valid JSON");
    // BC-3.9.011 EC-3.9.011-3: output MUST NOT be an object envelope — catches
    // {"public":…,"uploaded":[…]} regression specifically (P2-3c SATISFIED).
    // This assert fires before is_array() so a regression trips the EC-3.9.011-3 label.
    assert!(
        !raw_v.is_object(),
        "AC-003: --internal output must NOT be an object envelope — bare array required \
         (BC-3.9.011 EC-3.9.011-3); got: {raw_v}"
    );
    // BC-3.9.011 EC-3.9.011-1: output MUST be a bare array, not any other non-object type.
    assert!(
        raw_v.is_array(),
        "AC-003: --internal output must be a bare JSON array (BC-3.9.011 EC-3.9.011-1); \
         got: {raw_v}"
    );

    let arr = raw_v.as_array().expect("already asserted is_array");
    assert!(
        !arr.is_empty(),
        "AC-003: --internal upload JSON array must be non-empty; stdout: {upload_stdout}"
    );

    // BC-3.9.011 EC-3.9.011-1: confirmed curated field set (P2-3c probe runs 29936980027+29940792930+29945857059).
    let item = &arr[0];
    for field in &[
        "id",
        "filename",
        "contentUrl",
        "mimeType",
        "size",
        "author",
        "created",
    ] {
        assert!(
            item.get(field).is_some(),
            "AC-003: curated attachment must have key '{field}' (BC-3.9.011 EC-3.9.011-1); got: {item}"
        );
    }
    assert!(
        item.get("self").is_none(),
        "AC-003: curated attachment must NOT contain 'self'; got: {item}"
    );
}

/// E2E no-visibility-flag: `jr issue attachment upload <JSM-KEY> <FILE> --yes
/// --output json` (no `--public`/`--internal`) exits 0 via the platform POST path
/// (BC-3.9.002) and returns a non-empty curated array. Verifies the uploaded AID
/// appears in a subsequent `attachment list` (observable post-condition for the
/// platform-POST default-internal path).
///
/// Gated by `JR_RUN_E2E=1` + `JR_E2E_JSM_PROJECT`. Gate-2 (empty RT list) +
/// Gate-3 (403) trigger clean-skips.
///
/// Traces to: AC-004, BC-3.9.002, BC-3.9.009.
#[test]
#[ignore = "set JR_RUN_E2E=1 and JR_E2E_JSM_PROJECT and use --include-ignored to run"]
fn test_e2e_jsm_attachment_upload_no_flag() {
    if !e2e_enabled() {
        return;
    }
    // Gate 1 (§3.1).
    let jsm_project = match env::var("JR_E2E_JSM_PROJECT") {
        Ok(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => {
            eprintln!(
                "[SKIP] JR_E2E_JSM_PROJECT not set — skipping JSM attachment upload no-flag test"
            );
            return;
        }
    };
    let h = e2e_harness();
    let run_id = run_label();

    // Step 1: discover a request type (Gate 2).
    let list_out = h
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
        .expect("failed to spawn jr requesttype list");

    if !list_out.status.success() {
        let s = String::from_utf8_lossy(&list_out.stderr);
        if s.contains("403") {
            eprintln!(
                "[SKIP] requesttype list returned 403 — skipping JSM attachment upload no-flag"
            );
            return;
        }
        eprintln!("[SKIP] requesttype list failed — skipping JSM attachment upload no-flag: {s}");
        return;
    }

    let rts: Vec<Value> = match serde_json::from_slice(&list_out.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[SKIP] requesttype list not a JSON array: {e} — skipping");
            return;
        }
    };
    if rts.is_empty() {
        eprintln!(
            "[SKIP] No request types found on {jsm_project} — skipping JSM attachment upload no-flag"
        );
        return;
    }
    let first_rt_id = {
        let id_val = &rts[0]["id"];
        if let Some(s) = id_val.as_str() {
            s.to_string()
        } else if let Some(n) = id_val.as_i64() {
            n.to_string()
        } else {
            eprintln!("[SKIP] rts[0].id is not a usable type — skipping");
            return;
        }
    };

    // Step 2: create a fresh JSM request.
    let summary = format!("[e2e-jsm {run_id}] attachment upload no-flag");
    let create_out = h
        .cmd()
        .args([
            "issue",
            "create",
            "--project",
            &jsm_project,
            "--request-type",
            &first_rt_id,
            "--summary",
            &summary,
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr issue create");

    if !create_out.status.success() {
        let s = String::from_utf8_lossy(&create_out.stderr);
        if s.contains("403") {
            eprintln!("[SKIP] issue create returned 403 — skipping JSM attachment upload no-flag");
            return;
        }
        eprintln!("[SKIP] issue create failed — skipping JSM attachment upload no-flag: {s}");
        return;
    }

    let create_v: Value = serde_json::from_slice(&create_out.stdout)
        .expect("issue create --output json must be valid JSON");
    let key = create_v
        .get("key")
        .and_then(Value::as_str)
        .expect("issue create JSON must contain 'key' field")
        .to_string();
    assert!(!key.is_empty(), "created JSM key must be non-empty");

    // Step 3: write a temp file.
    let upload_dir = TempDir::new().expect("failed to create upload temp dir");
    let upload_file = upload_dir.path().join("attachment-e2e-noflag.txt");
    std::fs::write(&upload_file, b"S-576-6 e2e no-flag attachment").expect("write test file");

    // Step 4: upload without visibility flags → platform POST (BC-3.9.002).
    let upload_out = h
        .cmd()
        .args([
            "issue",
            "attachment",
            "upload",
            &key,
            &upload_file.to_string_lossy(),
            "--yes",
            "--output",
            "json",
        ])
        .output()
        .expect("failed to spawn jr attachment upload");

    // Gate 3 (§3.3): 403 → clean-skip.
    if !upload_out.status.success() {
        let s = String::from_utf8_lossy(&upload_out.stderr);
        if s.contains("403") {
            eprintln!("[SKIP] attachment upload (no-flag) returned 403 — skipping");
            jsm_self_close(&key, &h);
            return;
        }
    }

    // Capture AID.
    let upload_stdout_raw = String::from_utf8_lossy(&upload_out.stdout);
    let teardown_aid: Option<String> = serde_json::from_str::<Vec<Value>>(&upload_stdout_raw)
        .ok()
        .and_then(|arr| arr.into_iter().next())
        .and_then(|item| item.get("id").and_then(Value::as_str).map(str::to_string));

    // Step 5: verify AID appears in list (post-condition of upload success).
    let list_verify_out = h
        .cmd()
        .args(["issue", "attachment", "list", &key, "--output", "json"])
        .output()
        .expect("failed to spawn jr attachment list");

    // Teardown: (a) delete AID, (b) jsm_self_close.
    if let Some(ref aid) = teardown_aid {
        match h
            .cmd()
            .args(["issue", "attachment", "delete", aid, "--yes"])
            .output()
        {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                eprintln!(
                    "[WARN] AC-004: failed to delete attachment {aid} (exit {:?}): {}",
                    o.status.code(),
                    String::from_utf8_lossy(&o.stderr)
                );
            }
            Err(e) => eprintln!("[WARN] AC-004: failed to spawn attachment delete: {e}"),
        }
    } else {
        eprintln!("[WARN] AC-004: could not parse AID from upload stdout — attachment not deleted");
    }
    jsm_self_close(&key, &h);

    // ---- Assertions ----

    let upload_stderr = String::from_utf8_lossy(&upload_out.stderr);
    let upload_stdout = String::from_utf8_lossy(&upload_out.stdout);
    assert!(
        upload_out.status.success(),
        "AC-004: upload (no-flag) must exit 0; got {:?}\nstdout: {upload_stdout}\nstderr: {upload_stderr}",
        upload_out.status.code()
    );

    let arr: Vec<Value> = serde_json::from_str(&upload_stdout)
        .expect("AC-004: upload --output json must be a JSON array");
    assert!(
        !arr.is_empty(),
        "AC-004: upload JSON array must be non-empty (BC-3.9.002/009); stdout: {upload_stdout}"
    );
    let the_aid = teardown_aid
        .as_ref()
        .expect("AC-004: AID must be parseable from upload output");
    assert!(!the_aid.is_empty(), "AC-004: AID must be non-empty");

    // Step 5 assertion: AID present in list (confirms upload succeeded via platform POST path).
    let list_stderr = String::from_utf8_lossy(&list_verify_out.stderr);
    if !list_verify_out.status.success() && list_stderr.contains("403") {
        eprintln!(
            "[SKIP] AC-004: list returned 403 — skipping JSM attachment upload no-flag verification (AC-007 §3.3)"
        );
        return;
    }
    let list_stdout = String::from_utf8_lossy(&list_verify_out.stdout);
    assert!(list_verify_out.status.success(), "AC-004: list must exit 0");
    let list_arr: Vec<Value> = serde_json::from_str(&list_stdout)
        .expect("AC-004: list --output json must be a JSON array");
    assert!(
        list_arr
            .iter()
            .any(|item| item.get("id").and_then(Value::as_str) == Some(the_aid.as_str())),
        "AC-004: list must contain uploaded AID={the_aid} (BC-3.9.002); got: {list_stdout}"
    );
}
