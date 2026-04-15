# 429 Retry Exhaustion Warning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a stderr warning when 429 retry exhaustion occurs in `send` and `send_raw`, so users know jr absorbed backoff time on their behalf.

**Architecture:** Add an unconditional `eprintln!` warning to both retry loops in `src/api/client.rs` at the point where the final 429 falls through. Test via a subprocess handler test using wiremock + assert_cmd stderr capture.

**Tech Stack:** Rust, wiremock, assert_cmd, predicates

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/api/client.rs` | Modify | Add `eprintln!` warning to `send` (line ~202) and `send_raw` (line ~268) |
| `tests/cli_handler.rs` | Modify | Add `test_api_warns_on_429_retry_exhaustion` handler test |

---

### Task 1: Write the failing handler test

**Files:**
- Modify: `tests/cli_handler.rs` (append after line 1337)

- [ ] **Step 1.1: Write the failing test**

Add this test at the end of `tests/cli_handler.rs`:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_api_warns_on_429_retry_exhaustion() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_string(r#"{"errorMessages":["Rate limit exceeded"]}"#),
        )
        .expect(4) // initial + 3 retries (MAX_RETRIES)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/myself"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("warning: rate limited by Jira"))
        .stderr(predicate::str::contains("3 retries"));
}
```

- [ ] **Step 1.2: Run the test to verify it fails**

Run:
```bash
cargo test --test cli_handler test_api_warns_on_429_retry_exhaustion -- --nocapture
```

Expected: FAIL — stderr does not contain "warning: rate limited by Jira" (the warning doesn't exist yet).

---

### Task 2: Add the warning to `send_raw`

**Files:**
- Modify: `src/api/client.rs:268` (just before the final `return Ok(response)`)

- [ ] **Step 2.1: Add the warning before the final return in `send_raw`**

In `src/api/client.rs`, find this block inside `send_raw` (currently around line 268):

```rust
            // Return the response for ANY status (including 4xx/5xx) — no error parsing
            return Ok(response);
```

Replace with:

```rust
            // Warn the user if we exhausted retries on a 429
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                eprintln!(
                    "warning: rate limited by Jira — gave up after {MAX_RETRIES} retries"
                );
            }

            // Return the response for ANY status (including 4xx/5xx) — no error parsing
            return Ok(response);
```

- [ ] **Step 2.2: Run the handler test**

Run:
```bash
cargo test --test cli_handler test_api_warns_on_429_retry_exhaustion -- --nocapture
```

Expected: PASS — the `jr api` subprocess now prints the warning to stderr, and the test asserts it.

- [ ] **Step 2.3: Run the full test suite**

Run:
```bash
cargo test
```

Expected: All tests pass. The existing `test_send_raw_returns_429_after_exhausting_retries` unit test is unaffected (it tests the return value, not stderr).

- [ ] **Step 2.4: Run clippy and fmt**

Run:
```bash
cargo clippy -- -D warnings
cargo fmt --all -- --check
```

Expected: Both clean.

- [ ] **Step 2.5: Commit**

```bash
git add src/api/client.rs tests/cli_handler.rs
git commit -m "fix: warn on stderr when 429 retry exhaustion occurs in send_raw (#172)"
```

---

### Task 3: Add the warning to `send`

**Files:**
- Modify: `src/api/client.rs:202` (just before the `is_client_error()` check)

- [ ] **Step 3.1: Add the warning before the error check in `send`**

In `src/api/client.rs`, find this block inside `send` (currently around line 202):

```rust
            // For non-429 errors, parse and return the error
            if response.status().is_client_error() || response.status().is_server_error() {
                return Err(Self::parse_error(response).await);
            }
```

Replace with:

```rust
            // Warn the user if we exhausted retries on a 429
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                eprintln!(
                    "warning: rate limited by Jira — gave up after {MAX_RETRIES} retries"
                );
            }

            // For non-429 errors, parse and return the error
            if response.status().is_client_error() || response.status().is_server_error() {
                return Err(Self::parse_error(response).await);
            }
```

- [ ] **Step 3.2: Run the full test suite**

Run:
```bash
cargo test
```

Expected: All tests pass. The `send` warning fires the same way as `send_raw` — when `attempt == MAX_RETRIES` and the response is 429, the retry condition `attempt < MAX_RETRIES` is false, so execution falls through to the new warning check.

- [ ] **Step 3.3: Run clippy and fmt**

Run:
```bash
cargo clippy -- -D warnings
cargo fmt --all -- --check
```

Expected: Both clean.

- [ ] **Step 3.4: Commit**

```bash
git add src/api/client.rs
git commit -m "fix: warn on stderr when 429 retry exhaustion occurs in send (#172)"
```

---

## Spec Coverage Checklist

| Spec Requirement | Task |
|------------------|------|
| Warning in `send_raw` | Task 2 |
| Warning in `send` | Task 3 |
| Warning message format: `warning: rate limited by Jira — gave up after 3 retries` | Task 2, 3 (identical message) |
| Not gated on `--verbose` | Task 2, 3 (unconditional `eprintln!`) |
| No change to return types, error messages, exit codes | Verified by existing tests passing |
| Handler subprocess test | Task 1 |
| Uses existing `jr_api_cmd` helper | Task 1 |
| Uses `predicate::str::contains` for stderr | Task 1 |
