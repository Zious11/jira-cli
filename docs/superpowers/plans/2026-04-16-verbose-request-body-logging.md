# Verbose Request Body Logging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Print the JSON request body alongside the existing `[verbose] METHOD URL` line whenever `--verbose` is set, so users can debug Jira's silent field drops without reaching for `curl`.

**Architecture:** Two surgical edits to existing `if self.verbose { ... }` blocks in `src/api/client.rs` (`send` and `send_raw`). Use `reqwest::Body::as_bytes()` on the buffered JSON body produced by `RequestBuilder::json()`, format as `[verbose] body: {...}` on stderr. Three new handler tests in `tests/cli_handler.rs` lock the behavior end-to-end (PUT body present, GET body absent, `send_raw` body via `jr api`).

**Tech Stack:** Rust, reqwest 0.13 (async), assert_cmd + predicates, wiremock

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/api/client.rs` | Modify | Extend the existing `if self.verbose { ... }` block at line 170 (`send`) to also `eprintln!` the body bytes when present. Same change at line 244 (`send_raw`). |
| `tests/cli_handler.rs` | Append | Three handler tests asserting that `--verbose` prints the body for PUT (via `send`), omits the body line for GET, and prints the body for POST through `send_raw` (via `jr api`). |

No new source code files or modules. No fixture changes — the existing `issue_response("HDL-1", "old summary", "To Do")` fixture is enough for the GET test.

---

### Task 1: TDD — verbose mode logs request body

**Files:**
- Modify: `src/api/client.rs:170-174` (`send`) and `src/api/client.rs:244-246` (`send_raw`)
- Modify: `tests/cli_handler.rs` (append three tests: PUT body, GET body absent, `send_raw` body via `jr api`)

- [ ] **Step 1.1: Write the failing test for PUT body**

Append to `tests/cli_handler.rs`:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_verbose_logs_request_body_for_put() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/HDL-1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .arg("--verbose")
        .args(["issue", "edit", "HDL-1", "--summary", "new summary"])
        .assert()
        .success()
        .stderr(predicate::str::contains("[verbose] PUT"))
        .stderr(predicate::str::contains("[verbose] body:"))
        .stderr(predicate::str::contains("\"summary\":\"new summary\""));
}
```

Uses the `jr_cmd(&server.uri())` helper and appends `.arg("--verbose")` — clap global flags can appear before the subcommand in any order, so there is no need to reconstruct the command from scratch.

The third `stderr` assertion (`"\"summary\":\"new summary\""`) is the strict check: it confirms the body line really contains the JSON the CLI sent, not just the prefix. Substring match (no leading `{`) tolerates whatever wrapping serde produces around it (e.g. `{"fields":{...}}`).

- [ ] **Step 1.2: Write the failing test for GET (body line omitted)**

Append immediately after the previous test:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_verbose_omits_body_line_for_get() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_response("HDL-1", "old summary", "To Do")),
        )
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .arg("--verbose")
        .args(["issue", "view", "HDL-1"])
        .assert()
        .success()
        .stderr(predicate::str::contains("[verbose] GET"))
        .stderr(predicate::str::contains("[verbose] body:").not());
}
```

`predicate::str::contains(...).not()` is the established negation pattern (predicates 3.x; already in use in this codebase via `predicates::prelude::*`).

- [ ] **Step 1.2b: Write the failing test for `send_raw` body (jr api)**

`send_raw()` is a separate code path used by `jr api` and is not covered by the PUT test (which goes through `send()`). Append:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_verbose_logs_request_body_for_send_raw() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/HDL-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .arg("--verbose")
        .args([
            "api",
            "/rest/api/3/issue/HDL-1/transitions",
            "-X",
            "post",
            "-d",
            r#"{"transition":{"id":"31"}}"#,
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("[verbose] POST"))
        .stderr(predicate::str::contains(
            "[verbose] body: {\"transition\":{\"id\":\"31\"}}",
        ));
}
```

Note `jr_api_cmd` (not `jr_cmd`) — `jr api` does not accept `--output json` so the helper without that flag is appropriate. The `-d` payload is asserted byte-exact in stderr because `send_raw` does not re-serialize the body.

- [ ] **Step 1.3: Run all three tests to verify they fail**

Run:
```bash
cargo test --test cli_handler test_verbose -- --nocapture
```

Expected for `test_verbose_logs_request_body_for_put`: **FAIL** at the `"[verbose] body:"` assertion — current code only prints `[verbose] PUT <url>`, no body line.

Expected for `test_verbose_logs_request_body_for_send_raw`: **FAIL** at the `"[verbose] body:"` assertion for the same reason.

Expected for `test_verbose_omits_body_line_for_get`: **PASS** unexpectedly (the body line is already absent because no body is logged today). This is OK — keep the test as a regression guard for after the change.

If the GET test happens to fail because of stderr noise we don't expect, capture the actual output via `--nocapture` and adjust the assertion before moving on. Do not weaken the assertion just to make it pass.

- [ ] **Step 1.4: Implement body logging in `send`**

Open `src/api/client.rs` and find this block (around line 170):

```rust
            if self.verbose {
                if let Some(ref r) = req.try_clone().and_then(|r| r.build().ok()) {
                    eprintln!("[verbose] {} {}", r.method(), r.url());
                }
            }
```

Replace with:

```rust
            if self.verbose {
                if let Some(ref r) = req.try_clone().and_then(|r| r.build().ok()) {
                    eprintln!("[verbose] {} {}", r.method(), r.url());
                    if let Some(bytes) = r.body().and_then(|b| b.as_bytes()) {
                        eprintln!("[verbose] body: {}", String::from_utf8_lossy(bytes));
                    }
                }
            }
```

Why this works:
- `RequestBuilder::json(&value)` calls `serde_json::to_vec(&value)` and stores the result via `*req.body_mut() = Some(body.into())`. The body is buffered in memory, not streamed.
- `reqwest::Body::as_bytes() -> Option<&[u8]>` returns `Some` for buffered bodies. (`None` only happens for streaming bodies — which this codebase has zero of: a grep for `wrap_stream`, `body_stream`, and `reqwest::Body::from(` in `src/` returned no matches.)
- `String::from_utf8_lossy` is the right call: JSON is UTF-8 by definition, but lossy-decode means a malformed body cannot panic the verbose log path — it just substitutes replacement characters, which is the right failure mode for a diagnostic.

- [ ] **Step 1.5: Implement body logging in `send_raw`**

In the same file, find this block (around line 244):

```rust
            if self.verbose {
                eprintln!("[verbose] {} {}", req.method(), req.url());
            }
```

Replace with:

```rust
            if self.verbose {
                eprintln!("[verbose] {} {}", req.method(), req.url());
                if let Some(bytes) = req.body().and_then(|b| b.as_bytes()) {
                    eprintln!("[verbose] body: {}", String::from_utf8_lossy(bytes));
                }
            }
```

Note the difference: `send_raw` already has a built `reqwest::Request` (not a `RequestBuilder`), so `req.body()` works directly without the `try_clone().build()` dance.

- [ ] **Step 1.6: Run all three tests to verify they pass**

Run:
```bash
cargo test --test cli_handler test_verbose -- --nocapture
```

Expected: all three **PASS**.

If `test_verbose_logs_request_body_for_put` still fails on the substring `"\"summary\":\"new summary\""`, dump the captured stderr (visible with `--nocapture`) and check whether serde produced spaces around the colon (e.g. `"summary": "new summary"`). If so, change the assertion to use a less brittle substring like `predicate::str::contains("new summary")` rather than weakening the structural check — but only after confirming the actual format. (`serde_json` defaults to compact, no spaces, so this should not be needed.)

- [ ] **Step 1.7: Run the full test suite**

Run:
```bash
cargo test
```

Expected: all tests pass. The change is additive (only adds an `eprintln!` inside an `if self.verbose` block that defaults false), so no existing test should observe new behavior. Existing handler tests do not pass `--verbose`, so their stderr expectations remain untouched.

If anything fails: read the failure carefully. If a test asserts on stderr and a prior `[verbose]` line snuck in (shouldn't happen — we only log when `verbose: true`), trace the source.

- [ ] **Step 1.8: Run clippy and fmt**

Run:
```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: both clean. The new code is two `if let Some(...)` chains and an `eprintln!` — nothing clippy should object to.

If `cargo fmt --check` fails, run `cargo fmt --all` and re-stage. Do not skip this — CI runs `--check` and an unformatted PR will go red.

- [ ] **Step 1.9: Commit**

```bash
git add src/api/client.rs tests/cli_handler.rs
git commit -m "feat: log request body in verbose mode (#183)"
```

---

## Spec Coverage Checklist

| Spec Requirement | Task |
|------------------|------|
| Body logged after URL line in `send()` | Task 1, Step 1.4 |
| Same change in `send_raw()` | Task 1, Step 1.5 |
| Body line omitted entirely when no body | Task 1, Step 1.4 (`if let Some(bytes) = ...` skips); Task 1, Step 1.2 (test) |
| Compact JSON, not pretty-printed | Task 1, Step 1.4 — the spec relies on `.json()` already producing compact bytes; we don't re-format |
| `String::from_utf8_lossy` for invalid UTF-8 safety | Task 1, Step 1.4 |
| `[verbose] body:` prefix mirrors `[verbose]` URL prefix | Task 1, Step 1.4 |
| Output goes to stderr | Task 1, Step 1.4 (`eprintln!`); Task 1, Steps 1.1–1.2 (tests assert via `.stderr(...)`) |
| Default (non-verbose) output unchanged | Task 1, Step 1.7 (full test suite) |
| Handler test for PUT body present | Task 1, Step 1.1 |
| Handler test for GET body absent | Task 1, Step 1.2 |
| No header logging, no response body, no `--debug` flag | Spec out-of-scope items — no task needed |
| OAuth credentials never traverse this path | Verified in spec; no task needed (separate `reqwest::Client` in `src/api/auth.rs`) |
