# Separate Human Status Messages (stderr) from Machine Output (stdout) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move write-command confirmation messages from stdout to stderr, matching the `gh` CLI convention where table mode is for human consumption and `--output json` is for scripting.

**Architecture:** One-line change to `output::print_success` (println→eprintln) moves ~20 call sites. Five additional standalone `println!` calls in auth.rs, init.rs, and create.rs also move to `eprintln!`. One new handler test verifies the stream separation.

**Tech Stack:** Rust, wiremock, assert_cmd, predicates

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/output.rs` | Modify | Change `print_success` from `println!` to `eprintln!` |
| `src/cli/issue/create.rs` | Modify | Change browse URL in Table mode from `println!` to `eprintln!` |
| `src/cli/auth.rs` | Modify | Change 3 status `println!` calls to `eprintln!` |
| `src/cli/init.rs` | Modify | Change 2 status `println!` calls to `eprintln!` |
| `tests/cli_handler.rs` | Modify | Add `test_create_table_mode_outputs_to_stderr` handler test |

---

### Task 1: TDD — test and implement core stream separation

**Files:**
- Modify: `tests/cli_handler.rs` (append new test)
- Modify: `src/output.rs:45-47`
- Modify: `src/cli/issue/create.rs:154`

- [ ] **Step 1.1: Write the failing handler test**

Add this test at the end of `tests/cli_handler.rs`:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_create_table_mode_outputs_to_stderr() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("HDL-300")),
        )
        .mount(&server)
        .await;

    // Use jr_api_cmd (no --output json) to test Table mode
    jr_api_cmd(&server.uri())
        .args([
            "issue",
            "create",
            "-p",
            "HDL",
            "-t",
            "Task",
            "-s",
            "Table mode test",
        ])
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("Created issue HDL-300"))
        .stderr(predicate::str::contains("/browse/HDL-300"));
}
```

This test uses `jr_api_cmd` which sets `JR_BASE_URL` + `JR_AUTH_HEADER` + `--no-input` but does NOT set `--output json`, so the command runs in Table mode. It asserts that:
- stdout is empty (no data in Table-mode write commands)
- stderr contains the confirmation message and browse URL

- [ ] **Step 1.2: Run the test to verify it fails**

Run:
```bash
cargo test --test cli_handler test_create_table_mode_outputs_to_stderr -- --nocapture
```

Expected: FAIL — stdout is NOT empty (it currently contains "Created issue HDL-300" and the URL), and stderr does NOT contain those strings.

- [ ] **Step 1.3: Change `print_success` to use stderr**

In `src/output.rs`, find:

```rust
pub fn print_success(msg: &str) {
    println!("{}", msg.green());
}
```

Replace with:

```rust
pub fn print_success(msg: &str) {
    eprintln!("{}", msg.green());
}
```

- [ ] **Step 1.4: Change browse URL to stderr**

In `src/cli/issue/create.rs`, find:

```rust
        OutputFormat::Table => {
            output::print_success(&format!("Created issue {}", response.key));
            println!("{}", browse_url);
        }
```

Replace with:

```rust
        OutputFormat::Table => {
            output::print_success(&format!("Created issue {}", response.key));
            eprintln!("{}", browse_url);
        }
```

- [ ] **Step 1.5: Run the test to verify it passes**

Run:
```bash
cargo test --test cli_handler test_create_table_mode_outputs_to_stderr -- --nocapture
```

Expected: PASS — confirmation and URL now go to stderr, stdout is empty.

- [ ] **Step 1.6: Run the full test suite**

Run:
```bash
cargo test
```

Expected: All tests pass. Existing handler tests use `--output json` (JSON output stays on stdout), so they are unaffected.

- [ ] **Step 1.7: Run clippy and fmt**

Run:
```bash
cargo clippy -- -D warnings
cargo fmt --all -- --check
```

Expected: Both clean.

- [ ] **Step 1.8: Commit**

```bash
git add src/output.rs src/cli/issue/create.rs tests/cli_handler.rs
git commit -m "refactor: move print_success and browse URL to stderr (#134)"
```

---

### Task 2: Move standalone status messages to stderr

**Files:**
- Modify: `src/cli/auth.rs:21,28,29`
- Modify: `src/cli/init.rs:10,69`

- [ ] **Step 2.1: Change auth.rs status messages**

In `src/cli/auth.rs`, find:

```rust
    auth::store_api_token(&email, &token)?;
    println!("Credentials stored in keychain.");
    Ok(())
```

Replace with:

```rust
    auth::store_api_token(&email, &token)?;
    eprintln!("Credentials stored in keychain.");
    Ok(())
```

In the same file, find:

```rust
pub async fn login_oauth() -> Result<()> {
    println!("OAuth 2.0 requires your own Atlassian OAuth app.");
    println!("Create one at: https://developer.atlassian.com/console/myapps/\n");
```

Replace with:

```rust
pub async fn login_oauth() -> Result<()> {
    eprintln!("OAuth 2.0 requires your own Atlassian OAuth app.");
    eprintln!("Create one at: https://developer.atlassian.com/console/myapps/\n");
```

- [ ] **Step 2.2: Change init.rs status messages**

In `src/cli/init.rs`, find:

```rust
pub async fn handle() -> Result<()> {
    println!("Setting up jr — Jira CLI\n");
```

Replace with:

```rust
pub async fn handle() -> Result<()> {
    eprintln!("Setting up jr — Jira CLI\n");
```

In the same file, find:

```rust
            println!("No boards found. You can configure .jr.toml manually.");
```

Replace with:

```rust
            eprintln!("No boards found. You can configure .jr.toml manually.");
```

- [ ] **Step 2.3: Run the full test suite**

Run:
```bash
cargo test
```

Expected: All tests pass. These `println!` calls are in interactive paths (login, init) with no handler tests — changes are safe.

- [ ] **Step 2.4: Run clippy and fmt**

Run:
```bash
cargo clippy -- -D warnings
cargo fmt --all -- --check
```

Expected: Both clean.

- [ ] **Step 2.5: Commit**

```bash
git add src/cli/auth.rs src/cli/init.rs
git commit -m "refactor: move auth and init status messages to stderr (#134)"
```

---

## Spec Coverage Checklist

| Spec Requirement | Task |
|------------------|------|
| `print_success` → `eprintln!` | Task 1 (Step 1.3) |
| Browse URL after create → stderr | Task 1 (Step 1.4) |
| `auth.rs` 3 `println!` → `eprintln!` | Task 2 (Step 2.1) |
| `init.rs` 2 `println!` → `eprintln!` | Task 2 (Step 2.2) |
| JSON output stays on stdout | Verified by existing tests passing (Task 1 Step 1.6) |
| `auth status` stays on stdout | Not touched — no code change needed |
| `open --url-only` stays on stdout | Not touched — no code change needed |
| `"No results found."` stays on stdout | Not touched — no code change needed |
| Handler test for Table-mode stream separation | Task 1 (Step 1.1) |
