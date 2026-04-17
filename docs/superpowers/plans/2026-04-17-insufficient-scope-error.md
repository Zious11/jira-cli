# Insufficient-Scope 401 Error Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a distinct `JrError::InsufficientScope` variant and restructure `parse_error` so 401 responses carrying `"scope does not match"` surface actionable workaround guidance instead of the generic "Not authenticated" message.

**Architecture:** One new `thiserror` variant in `src/error.rs` (exit code `2`). `parse_error` in `src/api/client.rs` restructured to read the body first, then branch on 401 + case-insensitive substring `"scope does not match"`. Graceful fallback to existing `NotAuthenticated` on any mismatch — no regression surface.

**Tech Stack:** Rust 2024, thiserror 2, reqwest 0.13, wiremock 0.6, tokio.

**Spec:** `docs/superpowers/specs/2026-04-17-insufficient-scope-error-design.md`

**Local CI gate (run after every task before commit):**
```bash
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test
```

---

## File Structure

| File | Change | Purpose |
|---|---|---|
| `src/error.rs` | Modify (+~15 lines, +1 unit test) | New `JrError::InsufficientScope` variant + exit-code arm + test |
| `src/api/client.rs` | Modify `parse_error` (lines 295-309, replace 14 lines with ~18) | Read body first, branch on 401 + substring match |
| `tests/api_client.rs` | Modify (+2 integration tests, no imports needed) | Positive scope-mismatch test + explicit negative fall-through test |

No new files. No new dependencies. No changes to `Cargo.toml`.

---

## Task 1: Add `JrError::InsufficientScope` variant

**Files:**
- Modify: `src/error.rs:1-43`

- [ ] **Step 1: Write the failing unit test**

Edit `src/error.rs`, add inside the `#[cfg(test)] mod tests` block (after the existing `user_error_display_passthrough` test at line 73):

```rust
    #[test]
    fn insufficient_scope_exit_code() {
        assert_eq!(
            JrError::InsufficientScope {
                message: "Unauthorized; scope does not match".into()
            }
            .exit_code(),
            2
        );
    }

    #[test]
    fn insufficient_scope_display_includes_workarounds() {
        let err = JrError::InsufficientScope {
            message: "Unauthorized; scope does not match".into(),
        };
        let s = err.to_string();
        assert!(s.contains("Insufficient token scope"), "got: {s}");
        assert!(
            s.contains("Unauthorized; scope does not match"),
            "raw message should be included: {s}"
        );
        assert!(s.contains("write:jira-work"), "workaround missing: {s}");
        assert!(s.contains("OAuth 2.0"), "workaround missing: {s}");
        assert!(
            s.contains("github.com/Zious11/jira-cli/issues/185"),
            "issue link missing: {s}"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib error::`

Expected: FAIL with `error[E0599]: no variant or associated item named "InsufficientScope" found for enum "JrError"` (the type doesn't exist yet).

- [ ] **Step 3: Add the variant**

Edit `src/error.rs` — add the new variant between `NotAuthenticated` (line 5-6) and `NetworkError` (line 8-9) so it stays grouped with the other auth/network errors:

```rust
    #[error("Not authenticated. Run \"jr auth login\" to connect.")]
    NotAuthenticated,

    #[error(
        "Insufficient token scope: {message}\n\n\
         The Atlassian API gateway rejects granular-scoped personal tokens on POST \
         requests (while PUT/GET succeed). Workarounds:\n  \
         • Use a classic token with \"write:jira-work\" scope instead of granular scopes, or\n  \
         • Try OAuth 2.0 (run \"jr auth login --oauth\") — may avoid this bug, not verified\n\
         See https://github.com/Zious11/jira-cli/issues/185 for details."
    )]
    InsufficientScope { message: String },

    #[error("Could not reach {0} — check your connection")]
    NetworkError(String),
```

- [ ] **Step 4: Add the exit-code arm**

Edit `src/error.rs`, inside the `impl JrError { pub fn exit_code ... }` match block (around line 34-42). Add the `InsufficientScope` arm grouped with `NotAuthenticated`:

```rust
    pub fn exit_code(&self) -> i32 {
        match self {
            JrError::NotAuthenticated => 2,
            JrError::InsufficientScope { .. } => 2,
            JrError::ConfigError(_) => 78,
            JrError::UserError(_) => 64,
            JrError::Interrupted => 130,
            _ => 1,
        }
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib error::`

Expected: PASS (all existing tests + 2 new ones).

- [ ] **Step 6: Run full CI gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`

Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add src/error.rs
git commit -m "feat(error): add JrError::InsufficientScope variant (#185)"
```

---

## Task 2: Restructure `parse_error` to dispatch on body content

**Files:**
- Modify: `src/api/client.rs:295-309`

- [ ] **Step 1: Read the existing `parse_error` to confirm line range**

Run: `sed -n '295,309p' src/api/client.rs`

Expected output:
```rust
    /// Parse an error response into a `JrError`.
    async fn parse_error(response: Response) -> anyhow::Error {
        let status = response.status().as_u16();

        if status == 401 {
            return JrError::NotAuthenticated.into();
        }

        let message = match response.bytes().await {
            Ok(body) => extract_error_message(&body),
            Err(e) => format!("Could not read error response: {e}"),
        };

        JrError::ApiError { status, message }.into()
    }
```

If the line range differs, adjust the edit below accordingly. Do not proceed if the function body has diverged from the spec's expected shape — flag it as a NEEDS_CONTEXT escalation.

- [ ] **Step 2: Replace the function body**

Edit `src/api/client.rs` — replace the entire `parse_error` function (lines 295-309) with:

```rust
    /// Parse an error response into a `JrError`.
    ///
    /// Always reads the response body first, then branches on status. On 401, if
    /// the body's message contains `"scope does not match"` (case-insensitive,
    /// ASCII), returns `JrError::InsufficientScope` with the raw gateway message
    /// — matches the Atlassian API gateway's rejection shape for granular-scoped
    /// personal tokens on POST requests (see issue #185). Any other 401 falls
    /// through to `NotAuthenticated`; non-401 4xx/5xx returns `ApiError`.
    async fn parse_error(response: Response) -> anyhow::Error {
        let status = response.status().as_u16();
        let message = match response.bytes().await {
            Ok(body) => extract_error_message(&body),
            Err(e) => format!("Could not read error response: {e}"),
        };

        if status == 401 {
            if message.to_ascii_lowercase().contains("scope does not match") {
                return JrError::InsufficientScope { message }.into();
            }
            return JrError::NotAuthenticated.into();
        }

        JrError::ApiError { status, message }.into()
    }
```

- [ ] **Step 3: Run existing 401 test to verify non-scope fall-through still works**

Run: `cargo test --test api_client test_401_returns_not_authenticated`

Expected: PASS. The existing test uses body message `"Client must be authenticated to access this resource."` — no "scope" substring, so it still falls through to `NotAuthenticated`.

- [ ] **Step 4: Run full CI gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`

Expected: all green (667 existing tests pass + 2 from Task 1).

- [ ] **Step 5: Commit**

```bash
git add src/api/client.rs
git commit -m "feat(client): dispatch 401 scope-mismatch to InsufficientScope (#185)"
```

---

## Task 3: Add integration test for scope-mismatch 401 path

**Files:**
- Modify: `tests/api_client.rs` (add test after the existing `test_401_returns_not_authenticated` at line 72)

- [ ] **Step 1: Verify existing test structure**

Run: `sed -n '1,30p' tests/api_client.rs`

Confirm the imports include `wiremock::{MockServer, Mock, ResponseTemplate}`, `wiremock::matchers::{method, path}`, `serde_json::json`, and `jr::api::client::JiraClient`. Existing 401 test at line 72 is the template to mirror.

- [ ] **Step 2: Write the failing test**

Edit `tests/api_client.rs` — add this test immediately after the existing `test_401_returns_not_authenticated` function (ends around line 97):

```rust
#[tokio::test]
async fn test_401_scope_mismatch_returns_insufficient_scope() {
    // Atlassian API gateway rejects granular-scoped personal tokens on POST
    // requests with this exact body shape. The error must surface actionable
    // workaround guidance instead of the generic "Not authenticated" message.
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "code": 401,
            "message": "Unauthorized; scope does not match"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        JiraClient::new_for_test(server.uri(), "Basic granular-token".to_string());

    let err = client
        .post::<serde_json::Value, _>(
            "/rest/api/3/issue",
            &serde_json::json!({"fields": {"summary": "test"}}),
        )
        .await
        .unwrap_err();

    let s = err.to_string();
    assert!(
        s.contains("Insufficient token scope"),
        "expected distinct scope error, got: {s}"
    );
    assert!(
        s.contains("Unauthorized; scope does not match"),
        "raw gateway message should be preserved: {s}"
    );
    assert!(
        s.contains("write:jira-work"),
        "classic-scope workaround missing: {s}"
    );
    assert!(s.contains("OAuth 2.0"), "OAuth workaround missing: {s}");
    assert!(
        s.contains("github.com/Zious11/jira-cli/issues/185"),
        "issue link missing: {s}"
    );
}
```

- [ ] **Step 3: Run the new test to verify it fails**

Run: `cargo test --test api_client test_401_scope_mismatch_returns_insufficient_scope`

Expected: **Actually passes** if Tasks 1 and 2 are complete — this integration test exercises the already-implemented logic. The "failing test" convention is inverted here because the test is written after the implementation rather than before. If Task 2 was skipped or is incomplete, the test FAILS because the error will still be generic `"Not authenticated"`.

If it fails: return to Task 2 and verify the dispatch logic landed correctly.

- [ ] **Step 4: Verify `JiraClient::post` signature**

Run: `grep -n "pub async fn post" src/api/client.rs`

Expected: a method signature matching `pub async fn post<T: DeserializeOwned, B: Serialize>(...)`. If the signature differs, adjust the turbofish in the test to match. If `post` doesn't exist at all, flag as NEEDS_CONTEXT.

- [ ] **Step 5: Run full CI gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`

Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add tests/api_client.rs
git commit -m "test(client): cover 401 scope-mismatch → InsufficientScope (#185)"
```

---

## Task 4: Add integration test for 401 fall-through boundary

**Files:**
- Modify: `tests/api_client.rs` (add test after the Task 3 test)

- [ ] **Step 1: Write the failing test**

Edit `tests/api_client.rs` — add this test immediately after `test_401_scope_mismatch_returns_insufficient_scope`:

```rust
#[tokio::test]
async fn test_401_without_scope_mismatch_falls_through_to_not_authenticated() {
    // 401 responses whose body does NOT contain "scope does not match" (e.g.,
    // expired session, bad credentials) must continue to return the generic
    // NotAuthenticated error. Pins the dispatch boundary intentionally so a
    // future tightening of the substring match surfaces as a test failure
    // instead of a silent behavior change.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "code": 401,
            "message": "Session expired"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        JiraClient::new_for_test(server.uri(), "Basic expired-session".to_string());

    let err = client
        .get::<serde_json::Value>("/rest/api/3/myself")
        .await
        .unwrap_err();

    let s = err.to_string();
    assert!(
        s.contains("Not authenticated"),
        "expected generic 401 fall-through, got: {s}"
    );
    assert!(
        !s.contains("Insufficient token scope"),
        "must NOT dispatch to InsufficientScope without the substring: {s}"
    );
}
```

- [ ] **Step 2: Run the new test**

Run: `cargo test --test api_client test_401_without_scope_mismatch_falls_through_to_not_authenticated`

Expected: PASS — Task 2's fall-through branch returns `NotAuthenticated` for any 401 body without the substring.

- [ ] **Step 3: Run full CI gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`

Expected: all green. Total new tests from this PR: 4 (2 unit in `src/error.rs` from Task 1, 2 integration in `tests/api_client.rs` from Tasks 3+4).

- [ ] **Step 4: Commit**

```bash
git add tests/api_client.rs
git commit -m "test(client): pin 401-without-scope fall-through boundary (#185)"
```

---

## Task 5: Cross-verify the complete behavior

**Files:** None modified — verification only.

- [ ] **Step 1: Run the full test suite one more time**

Run: `cargo test`

Expected: all tests pass (prior baseline + 4 new tests from Tasks 1/3/4).

- [ ] **Step 2: Confirm clippy and format are clean**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings`

Expected: zero output from fmt, zero warnings from clippy.

- [ ] **Step 3: Skim the diff for anything surprising**

Run: `git log --oneline develop..HEAD` then `git diff develop..HEAD --stat`

Expected: 4 commits, ~3 files changed, roughly +110/-5 lines. If the diff touches any other file, investigate before proceeding.

- [ ] **Step 4: Smoke-test the Display output manually (optional but recommended)**

Run:
```bash
cat > /tmp/jr_display_test.rs <<'EOF'
use jr::error::JrError;
fn main() {
    let err = JrError::InsufficientScope {
        message: "Unauthorized; scope does not match".into(),
    };
    println!("{err}");
}
EOF
```

This won't compile standalone since `JrError` is non-public in some crate setups — skip if it fails. The unit test `insufficient_scope_display_includes_workarounds` already covers the Display output content, so this manual check is redundant but can be useful for visual inspection of the multi-line formatting.

---

## Self-Review

**1. Spec coverage:**

| Spec section | Task |
|---|---|
| Error variant shape (display text, exit code 2) | Task 1 |
| `parse_error` body-first restructure | Task 2 |
| 401 + substring dispatch to `InsufficientScope` | Task 2 |
| Graceful fallback to `NotAuthenticated` | Task 2 (fall-through), Task 4 (test) |
| Positive wiremock test | Task 3 |
| Negative wiremock test | Task 4 |
| Exit-code unit test | Task 1 |
| Display-text unit test (workaround strings present) | Task 1 |

Spec-required files all covered: `src/error.rs` (Task 1), `src/api/client.rs` (Task 2), `tests/api_client.rs` (Tasks 3+4).

**2. Placeholder scan:** None — every step has exact code, exact commands, exact expected output.

**3. Type consistency:**
- `JrError::InsufficientScope { message: String }` — struct variant with named field `message`. Used consistently across Tasks 1 (definition, tests) and 2 (construction).
- `parse_error` return type unchanged (`anyhow::Error`) — caller-compatible.
- `post::<serde_json::Value, _>` turbofish in Task 3 matches the existing `post` signature pattern (confirmed by the `get::<serde_json::Value>` usage in the existing test at line 87).
- Substring match string `"scope does not match"` (lowercase, no punctuation) used identically in Task 2 and Task 3's test fixture.

---

## Execution Handoff

Plan complete. Size: 4 code tasks + 1 verification. Bite-sized: each task has 4-7 steps, each step is 2-5 minutes.
