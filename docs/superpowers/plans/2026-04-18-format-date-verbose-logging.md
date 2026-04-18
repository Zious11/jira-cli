# Verbose Parse-Failure Logging for Date Formatters — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When `--verbose` is set, emit a one-shot `eprintln!("[verbose] ...")` per call-site the first time `format_date` or `format_comment_date` fails to parse a timestamp, so a future Jira format regression surfaces instead of silently corrupting the rendered column.

**Architecture:** Add a tiny `src/observability.rs` module with a `log_parse_failure_once(&AtomicBool, site, iso, verbose)` helper. Expose `JiraClient::verbose()` so handlers can thread the flag down to the two formatters. Each formatter declares a function-local `static LOGGED: AtomicBool` and calls the helper on parse failure. Four new integration tests (two per formatter) cover the "logs exactly once" and "silent without `--verbose`" contract via `assert_cmd` + `wiremock`.

**Tech Stack:** Rust 2024 edition, `std::sync::atomic` only (no new dep), `assert_cmd` + `wiremock` for integration tests, clap derive (already wired, `--verbose` is `global = true`).

**Spec:** `docs/specs/format-date-verbose-parse-failure-logging.md`

**Task ordering rationale:** RED tests precede their GREEN implementation, and each GREEN task lands *all* foundation pieces referenced by its caller within the same commit. This avoids any window where `cargo clippy --all-targets -- -D warnings` would flag the helper or accessor as dead code, and keeps CLAUDE.md's "no lint suppression" rule unviolated.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/observability.rs` | **new** — one helper `log_parse_failure_once` |
| `src/lib.rs` | register `mod observability;` |
| `src/api/client.rs` | add `pub fn verbose(&self) -> bool` accessor |
| `src/cli/issue/changelog.rs` | thread `verbose: bool` through `handle → build_rows → format_date`; call helper on parse fail |
| `src/cli/issue/list.rs` | thread `verbose: bool` through comments callers → `format_comment_row → format_comment_date`; call helper on parse fail |
| `tests/issue_changelog.rs` | 2 new integration tests (verbose logs once + silent without verbose) |
| `tests/comments.rs` | 2 new integration tests (mirror for comments path) |

No other modules touched. No schema/serde changes.

---

## Task 1: RED — two failing changelog integration tests

Add the two integration tests that pin the `format_date` contract. Against the current unchanged implementation:

- `changelog_verbose_logs_parse_failure_once` will **FAIL** (stderr empty, expected `"timestamp failed to parse"` × 1).
- `changelog_parse_failure_silent_without_verbose` will **PASS vacuously** (asserts the *absence* of a message, which is trivially true pre-implementation). That's fine — we still write it now so Task 2's implementation can't regress the "silent without verbose" half.

**Files:**
- Modify: `tests/issue_changelog.rs` (append two new `#[tokio::test]` functions at the end of the file)

- [ ] **Step 1.1: Append the two tests**

Open `tests/issue_changelog.rs`. Add these tests at the end of the file. Match the style of neighboring tests (`#[tokio::test] async fn`, use of `wiremock` + `assert_cmd`, same imports at the top of the file — no new imports needed).

```rust
#[tokio::test]
async fn changelog_verbose_logs_parse_failure_once() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/BAD-1/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [
                {
                    "id": "1",
                    "author": {
                        "accountId": "u1",
                        "displayName": "Alice",
                        "emailAddress": null,
                        "active": true
                    },
                    "created": "not-a-date",
                    "items": [{
                        "field": "status",
                        "fieldtype": "jira",
                        "from": null, "fromString": "To Do",
                        "to": null, "toString": "In Progress"
                    }]
                },
                {
                    "id": "2",
                    "author": {
                        "accountId": "u1",
                        "displayName": "Alice",
                        "emailAddress": null,
                        "active": true
                    },
                    "created": "still-not-a-date",
                    "items": [{
                        "field": "status",
                        "fieldtype": "jira",
                        "from": null, "fromString": "In Progress",
                        "to": null, "toString": "Done"
                    }]
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 2,
            "isLast": true
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "BAD-1", "--verbose"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let count = stderr.matches("timestamp failed to parse").count();
    assert_eq!(
        count, 1,
        "expected exactly one parse-failure log across 2 bad entries, got {count}. stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("[verbose] changelog"),
        "expected [verbose] changelog prefix in stderr, got:\n{stderr}"
    );
}

#[tokio::test]
async fn changelog_parse_failure_silent_without_verbose() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/BAD-2/changelog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [{
                "id": "1",
                "author": {
                    "accountId": "u1",
                    "displayName": "Alice",
                    "emailAddress": null,
                    "active": true
                },
                "created": "not-a-date",
                "items": [{
                    "field": "status",
                    "fieldtype": "jira",
                    "from": null, "fromString": "A",
                    "to": null, "toString": "B"
                }]
            }],
            "startAt": 0,
            "maxResults": 100,
            "total": 1,
            "isLast": true
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "changelog", "BAD-2"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("failed to parse"),
        "expected no verbose parse-failure output without --verbose, got:\n{stderr}"
    );
}
```

Fixture notes:
- `values[]` is the offset-paginated key used by Jira's changelog endpoint (matches existing `get_changelog` in `src/api/jira/issues.rs`).
- `created` is a plain `String` in `ChangelogEntry` (see `src/types/jira/changelog.rs:13`), so `"not-a-date"` deserializes and reaches `format_date`.
- `author` is included so the row renders fully; the parse-failure path triggers on `created` alone.
- `--verbose` is `global = true` in `src/cli/mod.rs:39`, so the positional order `["issue", "changelog", "BAD-1", "--verbose"]` is valid.

- [ ] **Step 1.2: Run the two new tests, confirm the RED split**

```bash
cargo test --test issue_changelog changelog_verbose_logs_parse_failure_once changelog_parse_failure_silent_without_verbose
```

Expected:
- `changelog_verbose_logs_parse_failure_once` — **FAIL** with `expected exactly one parse-failure log across 2 bad entries, got 0`.
- `changelog_parse_failure_silent_without_verbose` — **PASS** (vacuously).

If `changelog_verbose_logs_parse_failure_once` passes too, STOP — the implementation already emits something, and the plan needs revisiting.

- [ ] **Step 1.3: Commit**

```bash
git add tests/issue_changelog.rs
git commit -m "test(changelog): pin verbose parse-failure logging contract for format_date (#214)"
```

---

## Task 2: GREEN — observability foundation + thread `verbose` through changelog

One commit that lands the observability module, the `JiraClient::verbose()` accessor, and the changelog wiring that references both. The helper has a caller the instant it exists, so no dead-code suppression is ever needed.

**Files:**
- Create: `src/observability.rs`
- Modify: `src/lib.rs`
- Modify: `src/api/client.rs`
- Modify: `src/cli/issue/changelog.rs`

- [ ] **Step 2.1: Create `src/observability.rs`**

```rust
//! Lightweight observability primitives shared across commands.
//!
//! Intentionally tiny: the project has no tracing/log crate, and a
//! single `--verbose`-gated `eprintln!` is the established pattern
//! (see `src/api/client.rs` for HTTP-request logging). Expand to a
//! real tracing layer when there is cross-subsystem need.

use std::sync::atomic::{AtomicBool, Ordering};

/// Log a parse-failure once per `flag` per process, gated on `verbose`.
///
/// `flag` is typically a function-local `static AtomicBool`, one per
/// call-site, so each formatter fires at most one line per run. The
/// `site` argument is a short human label (e.g. `"changelog"`,
/// `"comment"`) included in the message for disambiguation.
pub(crate) fn log_parse_failure_once(
    flag: &AtomicBool,
    site: &str,
    iso: &str,
    verbose: bool,
) {
    if verbose && !flag.swap(true, Ordering::Relaxed) {
        eprintln!("[verbose] {site} timestamp failed to parse: {iso}");
    }
}
```

- [ ] **Step 2.2: Register the module in `src/lib.rs`**

Add, alphabetically among the existing `pub mod ...;` lines:

```rust
pub(crate) mod observability;
```

- [ ] **Step 2.3: Add `verbose()` accessor to `JiraClient`**

Inside the existing `impl JiraClient { ... }` in `src/api/client.rs` (begins at line 26), a good spot is right after `new_for_test` (around line 108):

```rust
    /// Whether the client was constructed with `--verbose` enabled.
    /// Handlers use this to gate optional diagnostic output.
    pub fn verbose(&self) -> bool {
        self.verbose
    }
```

- [ ] **Step 2.4: Change `format_date` signature and body in `src/cli/issue/changelog.rs`**

Locate `format_date` (around line 197). Replace the whole function with:

```rust
/// Render a Jira ISO-8601 timestamp as `YYYY-MM-DD HH:MM` in the user's
/// local time zone. Falls back to the raw string on parse failure; when
/// `verbose` is true, emits a one-shot `[verbose]` stderr note the first
/// time parsing fails in this process.
fn format_date(iso: &str, verbose: bool) -> String {
    use std::sync::atomic::AtomicBool;
    static LOGGED: AtomicBool = AtomicBool::new(false);
    match parse_created(iso) {
        Some(dt) => dt
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string(),
        None => {
            crate::observability::log_parse_failure_once(
                &LOGGED, "changelog", iso, verbose,
            );
            iso.to_string()
        }
    }
}
```

- [ ] **Step 2.5: Change `build_rows` signature and forward the flag**

Locate `build_rows` (directly below `format_date`). Its current signature is:

```rust
fn build_rows(entries: &[ChangelogEntry]) -> Vec<Vec<String>> {
```

Change to:

```rust
fn build_rows(entries: &[ChangelogEntry], verbose: bool) -> Vec<Vec<String>> {
```

Inside the function body, find each call to `format_date(&entry.created)` and change it to `format_date(&entry.created, verbose)`.

- [ ] **Step 2.6: Update the caller in `handle`**

`handle` (begins around line 23) calls `build_rows(&entries)` in the `OutputFormat::Table` arm. Change it to `build_rows(&entries, client.verbose())`.

- [ ] **Step 2.7: Update existing inline unit tests that call `format_date` directly**

In `src/cli/issue/changelog.rs`, `format_date_converts_rfc3339_to_local` (around line 456) and `format_date_falls_back_to_raw_on_parse_failure` (around line 465) call `format_date(iso)`. Pass `false` as the second arg in each:

```rust
// format_date_converts_rfc3339_to_local
let out = format_date("2026-04-16T14:02:00.000+0000", false);

// format_date_falls_back_to_raw_on_parse_failure
let out = format_date("not-a-date", false);
```

These unit tests do not exercise the verbose path (which would risk cross-test static pollution per the spec's maintainer caveat).

- [ ] **Step 2.8: Run the integration tests from Task 1 — expect GREEN + STILL-GREEN**

```bash
cargo test --test issue_changelog changelog_verbose_logs_parse_failure_once changelog_parse_failure_silent_without_verbose
```

Expected: both PASS. `changelog_verbose_logs_parse_failure_once` moves FAIL → PASS; `changelog_parse_failure_silent_without_verbose` stays PASS.

- [ ] **Step 2.9: Full check set**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: fmt clean, zero clippy warnings (the helper is now referenced by `format_date`, so `dead_code` will NOT trigger), all tests pass.

- [ ] **Step 2.10: Commit**

```bash
git add src/observability.rs src/lib.rs src/api/client.rs src/cli/issue/changelog.rs
git commit -m "feat(changelog): log format_date parse failures once when --verbose (#214)"
```

---

## Task 3: RED — two failing comments integration tests

Mirror the changelog pair for `format_comment_date` in `tests/comments.rs`. Same RED split (first test fails, second passes vacuously).

**Files:**
- Modify: `tests/comments.rs` (append two new `#[tokio::test]` functions at the end of the file)

- [ ] **Step 3.1: Append the two tests**

Open `tests/comments.rs`. Add at the end:

```rust
#[tokio::test]
async fn comments_verbose_logs_parse_failure_once() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/BAD-1/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": {
                        "accountId": "u1", "displayName": "Alice",
                        "emailAddress": "a@test.com", "active": true
                    },
                    "body": { "type": "doc", "version": 1, "content": [
                        { "type": "paragraph", "content": [
                            { "type": "text", "text": "first" }
                        ]}
                    ]},
                    "created": "not-a-date"
                },
                {
                    "id": "10002",
                    "author": {
                        "accountId": "u1", "displayName": "Alice",
                        "emailAddress": "a@test.com", "active": true
                    },
                    "body": { "type": "doc", "version": 1, "content": [
                        { "type": "paragraph", "content": [
                            { "type": "text", "text": "second" }
                        ]}
                    ]},
                    "created": "still-not-a-date"
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 2
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "BAD-1", "--verbose"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let count = stderr.matches("timestamp failed to parse").count();
    assert_eq!(
        count, 1,
        "expected exactly one parse-failure log across 2 bad comments, got {count}. stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("[verbose] comment"),
        "expected [verbose] comment prefix in stderr, got:\n{stderr}"
    );
}

#[tokio::test]
async fn comments_parse_failure_silent_without_verbose() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/BAD-2/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "comments": [
                {
                    "id": "10001",
                    "author": {
                        "accountId": "u1", "displayName": "Alice",
                        "emailAddress": "a@test.com", "active": true
                    },
                    "body": { "type": "doc", "version": 1, "content": [
                        { "type": "paragraph", "content": [
                            { "type": "text", "text": "first" }
                        ]}
                    ]},
                    "created": "not-a-date"
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 1
        })))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["issue", "comments", "BAD-2"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("failed to parse"),
        "expected no verbose parse-failure output without --verbose, got:\n{stderr}"
    );
}
```

- [ ] **Step 3.2: Run the two new tests, confirm the RED split**

```bash
cargo test --test comments comments_verbose_logs_parse_failure_once comments_parse_failure_silent_without_verbose
```

Expected:
- `comments_verbose_logs_parse_failure_once` — **FAIL** (stderr empty).
- `comments_parse_failure_silent_without_verbose` — **PASS** (vacuously).

- [ ] **Step 3.3: Commit**

```bash
git add tests/comments.rs
git commit -m "test(comments): pin verbose parse-failure logging contract for format_comment_date (#214)"
```

---

## Task 4: GREEN — thread `verbose` through comments path

Make `comments_verbose_logs_parse_failure_once` pass by threading `client.verbose()` through the comments rendering path.

**Files:**
- Modify: `src/cli/issue/list.rs`

- [ ] **Step 4.1: Change `format_comment_date` signature and body**

Locate `format_comment_date` (around line 592). Current body:

```rust
fn format_comment_date(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .or_else(|_| chrono::DateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.3f%z"))
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| iso.to_string())
}
```

Replace with:

```rust
fn format_comment_date(iso: &str, verbose: bool) -> String {
    use std::sync::atomic::AtomicBool;
    static LOGGED: AtomicBool = AtomicBool::new(false);
    match chrono::DateTime::parse_from_rfc3339(iso)
        .or_else(|_| chrono::DateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.3f%z"))
    {
        Ok(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => {
            crate::observability::log_parse_failure_once(
                &LOGGED, "comment", iso, verbose,
            );
            iso.to_string()
        }
    }
}
```

- [ ] **Step 4.2: Change `format_comment_row` signature and forward the flag**

Locate `format_comment_row` (directly below `format_comment_date`, around line 599). Current signature:

```rust
fn format_comment_row(
    author_name: Option<&str>,
    created: Option<&str>,
    body_text: Option<&str>,
) -> Vec<String> {
```

Change to:

```rust
fn format_comment_row(
    author_name: Option<&str>,
    created: Option<&str>,
    body_text: Option<&str>,
    verbose: bool,
) -> Vec<String> {
```

Inside the body, the `created.map(format_comment_date)` line becomes:

```rust
created
    .map(|c| format_comment_date(c, verbose))
    .unwrap_or_else(|| "-".into()),
```

- [ ] **Step 4.3: Update `format_comment_row` callers to pass `client.verbose()`**

Find every call site in `src/cli/issue/list.rs`:

```bash
grep -n "format_comment_row(" /Users/zious/Documents/GITHUB/jira-cli/.worktrees/format-date-verbose-logging/src/cli/issue/list.rs
```

Each caller should have `&client` in scope (comment-rendering happens inside handler functions that receive the client). Pass `client.verbose()` as the new last arg to every call.

- [ ] **Step 4.4: Run the integration tests from Task 3 — expect GREEN + STILL-GREEN**

```bash
cargo test --test comments comments_verbose_logs_parse_failure_once comments_parse_failure_silent_without_verbose
```

Expected: both PASS.

- [ ] **Step 4.5: Update existing inline unit tests that call `format_comment_date` directly**

Three unit tests exist in `src/cli/issue/list.rs` (around `format_comment_date_rfc3339` at line 982, `format_comment_date_jira_offset_no_colon` at line 990, `format_comment_date_malformed_returns_raw` at line 998). Each needs the second arg:

```rust
// format_comment_date_rfc3339
let out = format_comment_date("2026-04-16T14:02:00.000+00:00", false);

// format_comment_date_jira_offset_no_colon
let out = format_comment_date("2026-04-16T14:02:00.000+0000", false);

// format_comment_date_malformed_returns_raw
let out = format_comment_date("not-a-date", false);
```

- [ ] **Step 4.6: Full check set**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: fmt clean, zero clippy warnings, all tests pass.

- [ ] **Step 4.7: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "feat(comments): log format_comment_date parse failures once when --verbose (#214)"
```

---

## Self-Review Results

**Spec coverage:**
- §"Approach" (verbose-gated eprintln with once-per-run dedup via `AtomicBool::swap(Relaxed)`) → Task 2 Step 2.1 ✓
- §"Plumbing" (`JiraClient::verbose()` + threading through formatters) → Task 2 Steps 2.3, 2.4-2.6; Task 4 Steps 4.1-4.3 ✓
- §"Algorithm" (function-local `static LOGGED: AtomicBool` at each call-site) → Task 2 Step 2.4 + Task 4 Step 4.1 ✓
- §"Tests" (4 integration tests: verbose-logs-once + silent-without-verbose × 2 files) → Tasks 1 + 3 ✓
- §"Out of scope" (no tracing crate, no per-string dedup, no help-text edit) → no task touches these ✓
- §"Maintainer caveat" (cross-test pollution) — honored by not introducing any unit test that invokes `verbose=true` across tests ✓

**Placeholder scan:** No TBD/TODO/implement-later. Every step shows exact code and commands.

**Type consistency:**
- `log_parse_failure_once(flag: &AtomicBool, site: &str, iso: &str, verbose: bool)` — used identically in Task 2.4 and Task 4.1.
- `format_date(iso: &str, verbose: bool) -> String` — defined once, consumed once.
- `format_comment_date(iso: &str, verbose: bool) -> String` — defined once, consumed once.
- `build_rows(entries: &[ChangelogEntry], verbose: bool) -> Vec<Vec<String>>` — defined and consumed consistently.
- `format_comment_row(..., verbose: bool) -> Vec<String>` — defined and consumed consistently.
- `JiraClient::verbose(&self) -> bool` — introduced in Task 2.3, used in Task 2.6 and Task 4.3.

**Dead-code check:** Every new symbol has a caller within its introducing commit — no `#[allow(dead_code)]` or `#[expect(dead_code)]` anywhere.
