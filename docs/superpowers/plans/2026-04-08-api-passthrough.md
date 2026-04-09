# Raw API Passthrough Command (`jr api`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `jr api` command — an escape hatch for making arbitrary authenticated HTTP requests to the Jira REST API using stored credentials.

**Architecture:** Thin wrapper that reuses existing `JiraClient::request()` for URL/auth construction. A new `JiraClient::send_raw()` method preserves non-2xx responses (unlike `send()` which converts them to errors). A new `src/cli/api.rs` module houses the CLI helpers and handler. Body input follows curl's `@file`/`@-` conventions. Custom headers use `HeaderMap::insert` semantics (replace, not append).

**Tech Stack:** Rust, clap (derive API), reqwest, tokio, wiremock (tests), assert_cmd (subprocess handler tests), serde_json.

**Spec:** `docs/superpowers/specs/2026-04-08-api-passthrough-design.md`

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src/cli/api.rs` | **Create** | `HttpMethod` enum, `handle_api()` function, `parse_header`/`normalize_path`/`resolve_body` helpers, unit tests |
| `src/cli/mod.rs` | Modify | Register `pub mod api;` and add `Api { path, method, data, header }` variant to `Command` enum |
| `src/api/client.rs` | Modify | Add `send_raw()` method; extract `extract_error_message(body: &[u8]) -> String` helper (refactor `parse_error` to use it) |
| `src/main.rs` | Modify | Dispatch `Command::Api { .. }` to `cli::api::handle_api()` |
| `tests/api_client.rs` | Modify | Add tests for `send_raw` (2xx, 4xx, 429 retry) and `extract_error_message` (errorMessages, message, fallback) |
| `tests/cli_handler.rs` | Modify | Add subprocess handler tests for `jr api` (GET, POST body, PUT method, custom headers, error passthrough, path normalization, stdin body) |

---

## Task 1: Extract `extract_error_message` helper

**Files:**
- Modify: `src/api/client.rs` (lines ~218–251 currently contain `parse_error`)
- Test: `tests/api_client.rs`

**Context:** The existing `parse_error` method at `src/api/client.rs:219` takes a `reqwest::Response`, reads its body, and extracts an error summary from JSON (`errorMessages` array or `message` string) or falls back to the raw body. For `jr api`, we've already consumed the body as bytes in the handler, so we need the same extraction logic exposed as a standalone helper that takes `&[u8]`.

**Refactor:** Add `extract_error_message(body: &[u8]) -> String` as a free function (or `impl JiraClient` associated function). Update `parse_error` to call it.

- [ ] **Step 1.1: Write failing tests for `extract_error_message`**

Add to `tests/api_client.rs`:

```rust
use jr::api::client::extract_error_message;

#[test]
fn test_extract_error_message_from_error_messages_array() {
    let body = br#"{"errorMessages":["Issue does not exist","Or you lack permission"],"errors":{}}"#;
    let result = extract_error_message(body);
    assert_eq!(result, "Issue does not exist; Or you lack permission");
}

#[test]
fn test_extract_error_message_from_message_field() {
    let body = br#"{"message":"Property with key not found"}"#;
    let result = extract_error_message(body);
    assert_eq!(result, "Property with key not found");
}

#[test]
fn test_extract_error_message_prefers_error_messages_over_message() {
    let body = br#"{"errorMessages":["first"],"message":"second"}"#;
    let result = extract_error_message(body);
    assert_eq!(result, "first");
}

#[test]
fn test_extract_error_message_empty_error_messages_falls_back_to_body() {
    let body = br#"{"errorMessages":[]}"#;
    let result = extract_error_message(body);
    assert_eq!(result, r#"{"errorMessages":[]}"#);
}

#[test]
fn test_extract_error_message_plain_text_body() {
    let body = b"Internal Server Error";
    let result = extract_error_message(body);
    assert_eq!(result, "Internal Server Error");
}

#[test]
fn test_extract_error_message_empty_body() {
    let body = b"";
    let result = extract_error_message(body);
    assert_eq!(result, "");
}
```

- [ ] **Step 1.2: Run tests to verify they fail**

```bash
cargo test --test api_client extract_error_message
```

Expected: FAIL with `unresolved import jr::api::client::extract_error_message` or similar.

- [ ] **Step 1.3: Add `extract_error_message` helper to client.rs**

Add as a `pub` free function (or `impl` associated function) near `parse_error` in `src/api/client.rs`:

```rust
/// Extract a human-readable error message from a Jira error response body.
/// Matches the behavior of the old `parse_error` body handling:
/// 1. Try `errorMessages` array → join with "; "
/// 2. Try `message` string
/// 3. Fall back to the raw body string
pub fn extract_error_message(body: &[u8]) -> String {
    let body_str = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => return String::from_utf8_lossy(body).into_owned(),
    };

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
        if let Some(msgs) = json.get("errorMessages").and_then(|v| v.as_array()) {
            let messages: Vec<&str> = msgs.iter().filter_map(|m| m.as_str()).collect();
            if !messages.is_empty() {
                return messages.join("; ");
            }
        }
        if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
            return msg.to_string();
        }
    }

    body_str.to_string()
}
```

- [ ] **Step 1.4: Refactor `parse_error` to use `extract_error_message`**

Replace the body-parsing block inside `parse_error` (currently lines ~226-248 in `src/api/client.rs`):

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

- [ ] **Step 1.5: Run tests to verify they pass**

```bash
cargo test --test api_client extract_error_message
cargo test --test api_client  # full file — ensures parse_error still works
```

Expected: all tests PASS.

- [ ] **Step 1.6: Run full test suite to ensure no regressions**

```bash
cargo test
```

Expected: all tests PASS (existing tests depending on `parse_error` behavior should still pass since the logic is preserved).

- [ ] **Step 1.7: Commit**

```bash
git add src/api/client.rs tests/api_client.rs
git commit -m "refactor: extract_error_message helper from parse_error (#111)"
```

---

## Task 2: Add `send_raw()` to JiraClient

**Files:**
- Modify: `src/api/client.rs`
- Test: `tests/api_client.rs`

**Context:** The existing `send()` method converts non-2xx responses to errors via `parse_error`, destroying the raw body. For `jr api`, we need to return the `reqwest::Response` regardless of status so the handler can print the body as-is. We still want 429 retry (consistent with other `jr` commands) so we reuse the retry loop logic.

`send_raw` takes a pre-built `reqwest::Request` (not a `RequestBuilder`) because the handler builds the request manually to control header insert semantics.

- [ ] **Step 2.1: Write failing tests for `send_raw`**

Add to `tests/api_client.rs`:

```rust
use reqwest::Method;

#[tokio::test]
async fn test_send_raw_returns_response_for_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"accountId":"abc"}"#))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(
        server.uri(),
        "Basic dGVzdDp0ZXN0".to_string(),
    );

    let req = client
        .request(Method::GET, "/rest/api/3/myself")
        .build()
        .unwrap();
    let response = client.send_raw(req).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    assert_eq!(body, r#"{"accountId":"abc"}"#);
}

#[tokio::test]
async fn test_send_raw_returns_response_for_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/MISSING-1"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_string(r#"{"errorMessages":["Issue does not exist"],"errors":{}}"#),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(
        server.uri(),
        "Basic dGVzdDp0ZXN0".to_string(),
    );

    let req = client
        .request(Method::GET, "/rest/api/3/issue/MISSING-1")
        .build()
        .unwrap();
    let response = client.send_raw(req).await.unwrap();

    // Critical: 404 is NOT converted to an error
    assert_eq!(response.status().as_u16(), 404);
    let body = response.text().await.unwrap();
    assert!(body.contains("Issue does not exist"));
}

#[tokio::test]
async fn test_send_raw_retries_429_then_succeeds() {
    let server = MockServer::start().await;
    // First call returns 429
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    // Second call returns 200
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(
        server.uri(),
        "Basic dGVzdDp0ZXN0".to_string(),
    );

    let req = client
        .request(Method::GET, "/rest/api/3/myself")
        .build()
        .unwrap();
    let response = client.send_raw(req).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn test_send_raw_returns_429_after_exhausting_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .expect(4) // initial + 3 retries (MAX_RETRIES)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(
        server.uri(),
        "Basic dGVzdDp0ZXN0".to_string(),
    );

    let req = client
        .request(Method::GET, "/rest/api/3/myself")
        .build()
        .unwrap();
    let response = client.send_raw(req).await.unwrap();

    // Caller receives the 429 response — not an error
    assert_eq!(response.status().as_u16(), 429);
}
```

- [ ] **Step 2.2: Run tests to verify they fail**

```bash
cargo test --test api_client send_raw
```

Expected: FAIL with `no method named send_raw found for struct JiraClient`.

- [ ] **Step 2.3: Implement `send_raw`**

Add to `src/api/client.rs` inside `impl JiraClient` (near `send()`):

```rust
/// Send a pre-built request without parsing non-2xx responses into errors.
/// Retries 429 up to MAX_RETRIES times. Returns the raw Response for ANY status.
///
/// Used by `jr api` (the raw passthrough command) where the caller needs
/// the full response body regardless of HTTP status. Auth header is already
/// set on the request by `client.request()`.
pub async fn send_raw(&self, request: reqwest::Request) -> anyhow::Result<Response> {
    let mut last_response: Option<Response> = None;

    for attempt in 0..=MAX_RETRIES {
        let req = request
            .try_clone()
            .expect("request should be cloneable (no streaming body)");

        if self.verbose {
            eprintln!("[verbose] {} {}", req.method(), req.url());
        }

        let response = match self.client.execute(req).await {
            Ok(r) => r,
            Err(e) => {
                let url = e
                    .url()
                    .map(|u| u.host_str().unwrap_or("unknown").to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                return Err(JrError::NetworkError(url).into());
            }
        };

        if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_RETRIES {
            let rate_info = RateLimitInfo::from_headers(response.headers());
            let delay = rate_info.retry_after_secs.unwrap_or(DEFAULT_RETRY_SECS);
            if self.verbose {
                eprintln!(
                    "[verbose] Rate limited (429). Retrying in {delay}s (attempt {}/{})",
                    attempt + 1,
                    MAX_RETRIES
                );
            }
            tokio::time::sleep(Duration::from_secs(delay)).await;
            last_response = Some(response);
            continue;
        }

        // Return the response for ANY status (including 4xx/5xx) — no error parsing
        return Ok(response);
    }

    // Exhausted retries — return the last 429 response to the caller
    Ok(last_response.expect("retry loop always sets last_response on 429"))
}
```

- [ ] **Step 2.4: Run tests to verify they pass**

```bash
cargo test --test api_client send_raw
```

Expected: all 4 `send_raw` tests PASS.

- [ ] **Step 2.5: Run full test suite**

```bash
cargo test
```

Expected: all tests PASS.

- [ ] **Step 2.6: Commit**

```bash
git add src/api/client.rs tests/api_client.rs
git commit -m "feat: add send_raw method to JiraClient (#111)"
```

---

## Task 3: Create `src/cli/api.rs` module with `HttpMethod` enum and `normalize_path`

**Files:**
- Create: `src/cli/api.rs`
- Modify: `src/cli/mod.rs` (add `pub mod api;`)

**Context:** This task lays the foundation for the new module. We create an empty `api.rs` with a `HttpMethod` enum (clap `ValueEnum`) and a `normalize_path` helper. No handler logic yet — just the scaffolding and the first helper with its tests.

- [ ] **Step 3.1: Create `src/cli/api.rs` with `HttpMethod` enum**

```rust
//! `jr api` — raw API passthrough command.
//!
//! Provides an escape hatch for calling the Jira REST API directly with
//! stored credentials, modeled on `gh api`. Supports method override,
//! request body (inline / file / stdin), and custom headers.

use crate::error::JrError;
use anyhow::Result;
use clap::ValueEnum;
use reqwest::Method;

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl From<HttpMethod> for Method {
    fn from(method: HttpMethod) -> Self {
        match method {
            HttpMethod::Get => Method::GET,
            HttpMethod::Post => Method::POST,
            HttpMethod::Put => Method::PUT,
            HttpMethod::Patch => Method::PATCH,
            HttpMethod::Delete => Method::DELETE,
        }
    }
}

/// Normalize a user-provided API path:
/// - Accept absolute paths like `/rest/api/3/myself`
/// - Prepend `/` if missing (e.g. `rest/api/3/myself` → `/rest/api/3/myself`)
/// - Reject absolute URLs (starting with `http://` or `https://`)
pub(crate) fn normalize_path(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Err(JrError::UserError(
            "Use a path like /rest/api/3/... — do not include the instance URL".into(),
        )
        .into());
    }
    if trimmed.starts_with('/') {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("/{trimmed}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_with_slash() {
        let result = normalize_path("/rest/api/3/myself").unwrap();
        assert_eq!(result, "/rest/api/3/myself");
    }

    #[test]
    fn test_normalize_path_without_slash() {
        let result = normalize_path("rest/api/3/myself").unwrap();
        assert_eq!(result, "/rest/api/3/myself");
    }

    #[test]
    fn test_normalize_path_trims_whitespace() {
        let result = normalize_path("  /rest/api/3/myself  ").unwrap();
        assert_eq!(result, "/rest/api/3/myself");
    }

    #[test]
    fn test_normalize_path_rejects_http_url() {
        let err = normalize_path("http://site.atlassian.net/rest/api/3/myself").unwrap_err();
        assert!(err.to_string().contains("do not include the instance URL"));
    }

    #[test]
    fn test_normalize_path_rejects_https_url() {
        let err = normalize_path("https://site.atlassian.net/rest/api/3/myself").unwrap_err();
        assert!(err.to_string().contains("do not include the instance URL"));
    }
}
```

- [ ] **Step 3.2: Register the module in `src/cli/mod.rs`**

Add to the top of `src/cli/mod.rs`:

```rust
pub mod api;
```

Place it in alphabetical order with the existing `pub mod` declarations (after `pub mod assets;`, before `pub mod auth;` — actually `api` comes first alphabetically, so before `assets`).

The module list should become:

```rust
pub mod api;
pub mod assets;
pub mod auth;
pub mod board;
pub mod init;
pub mod issue;
pub mod project;
pub mod queue;
pub mod sprint;
pub mod team;
pub mod worklog;
```

- [ ] **Step 3.3: Run unit tests**

```bash
cargo test --lib cli::api::tests::test_normalize_path
```

Expected: all 5 `normalize_path` tests PASS.

- [ ] **Step 3.4: Run full build to verify clippy is clean**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 3.5: Commit**

```bash
git add src/cli/api.rs src/cli/mod.rs
git commit -m "feat: add HttpMethod enum and normalize_path helper (#111)"
```

---

## Task 4: Add `parse_header` helper

**Files:**
- Modify: `src/cli/api.rs`

**Context:** Parse user-supplied `-H "Key: Value"` strings into `(HeaderName, HeaderValue)` pairs. Reject the `Authorization` header to prevent credential override. Split on the FIRST colon only (so values like `Bearer abc:def` work correctly).

- [ ] **Step 4.1: Add failing tests for `parse_header`**

Add to the existing `#[cfg(test)] mod tests { ... }` block in `src/cli/api.rs`:

```rust
#[test]
fn test_parse_header_valid() {
    let (name, value) = parse_header("X-Foo: bar").unwrap();
    assert_eq!(name.as_str(), "x-foo");
    assert_eq!(value.to_str().unwrap(), "bar");
}

#[test]
fn test_parse_header_no_colon() {
    let err = parse_header("X-Foo bar").unwrap_err();
    assert!(err.to_string().contains("Key: Value"));
}

#[test]
fn test_parse_header_empty_key() {
    let err = parse_header(": bar").unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn test_parse_header_trims_whitespace() {
    let (name, value) = parse_header("  X-Foo  :   bar  ").unwrap();
    assert_eq!(name.as_str(), "x-foo");
    assert_eq!(value.to_str().unwrap(), "bar");
}

#[test]
fn test_parse_header_value_with_colon() {
    // Value contains a colon — should split on FIRST colon only
    let (name, value) = parse_header("X-Request-Id: abc:def:ghi").unwrap();
    assert_eq!(name.as_str(), "x-request-id");
    assert_eq!(value.to_str().unwrap(), "abc:def:ghi");
}

#[test]
fn test_parse_header_rejects_authorization() {
    let err = parse_header("Authorization: Bearer foo").unwrap_err();
    assert!(err.to_string().contains("Authorization"));
}

#[test]
fn test_parse_header_rejects_authorization_case_insensitive() {
    let err = parse_header("authorization: Bearer foo").unwrap_err();
    assert!(err.to_string().contains("Authorization"));
    let err = parse_header("AUTHORIZATION: Bearer foo").unwrap_err();
    assert!(err.to_string().contains("Authorization"));
}
```

- [ ] **Step 4.2: Run tests to verify they fail**

```bash
cargo test --lib cli::api::tests::test_parse_header
```

Expected: FAIL with `cannot find function parse_header`.

- [ ] **Step 4.3: Implement `parse_header`**

Add to `src/cli/api.rs` (above the `#[cfg(test)] mod tests` block):

```rust
use reqwest::header::{HeaderName, HeaderValue};

/// Parse a user-supplied header string in `Key: Value` format.
/// Rejects `Authorization` (case-insensitive) to prevent credential override.
pub(crate) fn parse_header(raw: &str) -> Result<(HeaderName, HeaderValue)> {
    let (key, value) = raw
        .split_once(':')
        .ok_or_else(|| {
            JrError::UserError(format!(
                "Header must be in 'Key: Value' format (got: {raw})"
            ))
        })?;

    let key = key.trim();
    let value = value.trim();

    if key.is_empty() {
        return Err(JrError::UserError("Header key cannot be empty".into()).into());
    }

    if key.eq_ignore_ascii_case("authorization") {
        return Err(JrError::UserError(
            "Cannot override the Authorization header — auth is managed by jr".into(),
        )
        .into());
    }

    let name = HeaderName::from_bytes(key.as_bytes())
        .map_err(|e| JrError::UserError(format!("Invalid header name '{key}': {e}")))?;
    let value = HeaderValue::from_str(value)
        .map_err(|e| JrError::UserError(format!("Invalid header value '{value}': {e}")))?;

    Ok((name, value))
}
```

- [ ] **Step 4.4: Run tests to verify they pass**

```bash
cargo test --lib cli::api::tests::test_parse_header
```

Expected: all 7 `parse_header` tests PASS.

- [ ] **Step 4.5: Clippy clean**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 4.6: Commit**

```bash
git add src/cli/api.rs
git commit -m "feat: add parse_header helper with Authorization rejection (#111)"
```

---

## Task 5: Add `resolve_body` helper

**Files:**
- Modify: `src/cli/api.rs`

**Context:** Resolve the `--data` argument into the actual body string. Supports inline JSON, `@file` (read from file), and `@-` (read from stdin). Takes a `Read` trait parameter for stdin so unit tests can inject a `Cursor`. Validates that the final body is valid JSON.

- [ ] **Step 5.1: Add failing tests for `resolve_body`**

Add to the `#[cfg(test)] mod tests` block in `src/cli/api.rs`:

```rust
use std::io::Cursor;

#[test]
fn test_resolve_body_none() {
    let stdin: Cursor<&[u8]> = Cursor::new(b"");
    let result = resolve_body(None, stdin).unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_resolve_body_inline_json() {
    let stdin: Cursor<&[u8]> = Cursor::new(b"");
    let result = resolve_body(Some(r#"{"a":1}"#), stdin).unwrap();
    assert_eq!(result, Some(r#"{"a":1}"#.to_string()));
}

#[test]
fn test_resolve_body_invalid_json_errors() {
    let stdin: Cursor<&[u8]> = Cursor::new(b"");
    let err = resolve_body(Some("not json"), stdin).unwrap_err();
    assert!(err.to_string().contains("Request body is not valid JSON"));
}

#[test]
fn test_resolve_body_at_file_reads_contents() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), r#"{"from":"file"}"#).unwrap();
    let arg = format!("@{}", tmp.path().display());

    let stdin: Cursor<&[u8]> = Cursor::new(b"");
    let result = resolve_body(Some(&arg), stdin).unwrap();
    assert_eq!(result, Some(r#"{"from":"file"}"#.to_string()));
}

#[test]
fn test_resolve_body_at_file_not_found() {
    let stdin: Cursor<&[u8]> = Cursor::new(b"");
    let err = resolve_body(Some("@/nonexistent/path/to/file.json"), stdin).unwrap_err();
    // Propagated std::io::Error
    assert!(err.to_string().to_lowercase().contains("no such file"));
}

#[test]
fn test_resolve_body_at_dash_reads_stdin() {
    let stdin_content = br#"{"from":"stdin"}"#;
    let stdin = Cursor::new(&stdin_content[..]);
    let result = resolve_body(Some("@-"), stdin).unwrap();
    assert_eq!(result, Some(r#"{"from":"stdin"}"#.to_string()));
}
```

- [ ] **Step 5.2: Verify `tempfile` is available as a dev-dependency**

```bash
grep tempfile Cargo.toml
```

Expected: `tempfile = "3"` under `[dev-dependencies]` (already present as of 2026-04-08). If missing for some reason, add it under `[dev-dependencies]` in `Cargo.toml`.

- [ ] **Step 5.3: Run tests to verify they fail**

```bash
cargo test --lib cli::api::tests::test_resolve_body
```

Expected: FAIL with `cannot find function resolve_body`.

- [ ] **Step 5.4: Implement `resolve_body`**

Add to `src/cli/api.rs` (above the `#[cfg(test)] mod tests` block):

```rust
use std::io::Read;

/// Resolve the `--data` argument into an actual request body.
/// - `None` → `None`
/// - `Some("@-")` → read from `stdin` parameter
/// - `Some("@filename")` → read from file
/// - `Some(inline)` → use as-is
///
/// Validates that the resulting body is valid JSON.
pub(crate) fn resolve_body<R: Read>(arg: Option<&str>, mut stdin: R) -> Result<Option<String>> {
    let body = match arg {
        None => return Ok(None),
        Some("@-") => {
            let mut buf = String::new();
            stdin.read_to_string(&mut buf)?;
            buf
        }
        Some(s) if s.starts_with('@') => {
            let path = &s[1..];
            std::fs::read_to_string(path)?
        }
        Some(s) => s.to_string(),
    };

    // Validate JSON — Jira REST API always uses JSON, catch typos before network
    serde_json::from_str::<serde_json::Value>(&body).map_err(|e| {
        JrError::UserError(format!("Request body is not valid JSON: {e}"))
    })?;

    Ok(Some(body))
}
```

- [ ] **Step 5.5: Run tests to verify they pass**

```bash
cargo test --lib cli::api::tests::test_resolve_body
```

Expected: all 6 `resolve_body` tests PASS.

- [ ] **Step 5.6: Clippy clean**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 5.7: Commit**

```bash
git add src/cli/api.rs Cargo.toml Cargo.lock
git commit -m "feat: add resolve_body helper with stdin/file/inline support (#111)"
```

---

## Task 6: Add `Api` variant to `Command` enum and wire up dispatch

**Files:**
- Modify: `src/cli/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/cli/api.rs` (add stub `handle_api`)

**Context:** Wire up the CLI plumbing: add the `Api { .. }` variant to the `Command` enum, dispatch it from `main.rs`, and add a placeholder `handle_api` that just returns `Ok(())`. We'll implement the real handler in Task 7.

- [ ] **Step 6.1: Add `Api` variant to `Command` enum**

Edit `src/cli/mod.rs`. Add a new variant to the `Command` enum (place it after `Queue { ... }` and before `Completion { ... }`):

```rust
    /// Make a raw authenticated HTTP request to the Jira REST API.
    Api {
        /// API path (leading slash optional). Example: /rest/api/3/myself
        path: String,

        /// HTTP method
        #[arg(short = 'X', long, value_enum, default_value_t = api::HttpMethod::Get)]
        method: api::HttpMethod,

        /// Request body: inline JSON, @file to read from a file, or @- to read from stdin
        #[arg(short = 'd', long)]
        data: Option<String>,

        /// Custom header in "Key: Value" format (repeatable)
        #[arg(short = 'H', long = "header")]
        header: Vec<String>,
    },
```

- [ ] **Step 6.2: Add stub `handle_api` to `src/cli/api.rs`**

Add to `src/cli/api.rs` (above the `#[cfg(test)]` block):

```rust
use crate::api::client::JiraClient;

/// Main entry point for `jr api`.
///
/// Takes the parsed CLI arguments, performs validation, builds an HTTP request,
/// sends it via `JiraClient::send_raw`, and prints the response body to stdout.
pub async fn handle_api(
    _path: String,
    _method: HttpMethod,
    _data: Option<String>,
    _header: Vec<String>,
    _client: &JiraClient,
) -> Result<()> {
    // Implemented in Task 7
    Ok(())
}
```

- [ ] **Step 6.3: Dispatch `Command::Api` in `src/main.rs`**

Edit `src/main.rs`. Add a new match arm inside the `match cli.command { ... }` block (place it alongside the other command arms, after `cli::Command::Queue { command } => ...`):

```rust
            cli::Command::Api {
                path,
                method,
                data,
                header,
            } => {
                let config = config::Config::load()?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::api::handle_api(path, method, data, header, &client).await
            }
```

- [ ] **Step 6.4: Verify build succeeds**

```bash
cargo build
```

Expected: SUCCESS. No warnings.

- [ ] **Step 6.5: Verify help output shows the new command**

```bash
cargo run -- api --help
```

Expected: Output showing `Make a raw authenticated HTTP request to the Jira REST API.` and the `-X`, `-d`, `-H` flags.

- [ ] **Step 6.6: Run full test suite**

```bash
cargo test
```

Expected: all tests PASS.

- [ ] **Step 6.7: Clippy clean**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 6.8: Commit**

```bash
git add src/cli/mod.rs src/main.rs src/cli/api.rs
git commit -m "feat: add Api variant to Command enum with stub handler (#111)"
```

---

## Task 7: Implement `handle_api`

**Files:**
- Modify: `src/cli/api.rs`

**Context:** This is the core handler. It:
1. Normalizes the path
2. Resolves the body (inline / file / stdin)
3. Parses custom headers (filtering Authorization)
4. Builds a `reqwest::Request` and applies headers with `insert()` semantics (not `append()`) to avoid duplicates
5. Sends via `client.send_raw()`
6. Writes the response body to stdout
7. Returns success or an `ApiError` based on HTTP status

- [ ] **Step 7.1: Implement `handle_api`**

Replace the stub `handle_api` in `src/cli/api.rs` with the real implementation.

First, update the imports at the top of `src/cli/api.rs`. After Tasks 3–6 the imports should be:

```rust
use crate::api::client::{JiraClient, extract_error_message};
use crate::error::JrError;
use anyhow::Result;
use clap::ValueEnum;
use reqwest::Method;
use reqwest::header::{CONTENT_TYPE, HeaderName, HeaderValue};
use std::io::{Read, Write};
```

(Add `extract_error_message`, `CONTENT_TYPE`, and `Write` to existing imports — merge them into the existing `use` statements rather than adding duplicates.)

Then replace the stub `handle_api` body:

```rust
pub async fn handle_api(
    path: String,
    method: HttpMethod,
    data: Option<String>,
    header: Vec<String>,
    client: &JiraClient,
) -> Result<()> {
    // 1. Normalize the path
    let normalized_path = normalize_path(&path)?;

    // 2. Resolve the body (reads real stdin in production)
    let body = resolve_body(data.as_deref(), std::io::stdin().lock())?;

    // 3. Parse custom headers (rejects Authorization)
    let custom_headers: Vec<(HeaderName, HeaderValue)> = header
        .iter()
        .map(|h| parse_header(h))
        .collect::<Result<Vec<_>>>()?;

    // 4. Build the request using the shared client helper, then .build() to get
    //    a concrete Request we can modify via headers_mut().insert().
    //    This avoids RequestBuilder::header()'s append semantics which would
    //    duplicate Content-Type when the user supplies their own.
    let mut req = client
        .request(method.into(), &normalized_path)
        .build()?;

    if let Some(ref body_str) = body {
        *req.body_mut() = Some(body_str.clone().into());
        req.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    }

    for (name, value) in custom_headers {
        req.headers_mut().insert(name, value);
    }

    // 5. Send via send_raw — preserves non-2xx responses
    let response = client.send_raw(req).await?;
    let status = response.status();
    let body_bytes = response.bytes().await?;

    // 6. Print response body to stdout (raw bytes, no reformatting).
    //    Matches gh api behavior: no trailing newline added — preserves
    //    exact server bytes for file redirection.
    std::io::stdout().write_all(&body_bytes)?;

    // 7. Handle status code
    if status.is_success() {
        Ok(())
    } else if status.as_u16() == 401 {
        Err(JrError::NotAuthenticated.into())
    } else {
        let message = extract_error_message(&body_bytes);
        // Print a human error summary to stderr
        crate::output::print_error(&format!("{message} (HTTP {})", status.as_u16()));
        Err(JrError::ApiError {
            status: status.as_u16(),
            message,
        }
        .into())
    }
}
```

- [ ] **Step 7.2: Run existing unit tests to ensure they still pass**

```bash
cargo test --lib cli::api
```

Expected: all unit tests from Tasks 3, 4, 5 PASS (18 tests: 5 normalize_path + 7 parse_header + 6 resolve_body).

- [ ] **Step 7.3: Build the binary**

```bash
cargo build
```

Expected: SUCCESS, no warnings.

- [ ] **Step 7.4: Clippy clean**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 7.5: Run full test suite**

```bash
cargo test
```

Expected: all tests PASS (no regressions).

- [ ] **Step 7.6: Commit**

```bash
git add src/cli/api.rs
git commit -m "feat: implement jr api handler with raw response passthrough (#111)"
```

---

## Task 8: Add handler integration tests

**Files:**
- Modify: `tests/cli_handler.rs`

**Context:** Handler tests in this codebase are subprocess tests using `assert_cmd::Command::cargo_bin("jr")` with `JR_BASE_URL` and `JR_AUTH_HEADER` env vars to route traffic to a wiremock server and bypass keychain auth. The `jr_cmd(server_uri)` helper at `tests/cli_handler.rs:13` wraps this pattern.

Important: the existing helper sets `--output json`. For `jr api` tests, we do NOT want that because `jr api` ignores `--output`. Build a separate command without that flag.

- [ ] **Step 8.1: Add a helper function for `jr api` commands**

Add to `tests/cli_handler.rs` (near the existing `jr_cmd` helper at line 13):

```rust
/// Build a `jr` command pre-configured for handler-level testing of `jr api`.
/// Unlike `jr_cmd`, does not set `--output json` since `jr api` ignores it.
fn jr_api_cmd(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input");
    cmd
}
```

- [ ] **Step 8.2: Add handler test for `GET` success**

Append to `tests/cli_handler.rs`:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_get_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"accountId":"abc-123","displayName":"Test User"}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/myself"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"accountId\":\"abc-123\""))
        .stdout(predicate::str::contains("\"displayName\":\"Test User\""));
}
```

- [ ] **Step 8.3: Add handler test for POST with inline body — verify single Content-Type**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_post_with_inline_data() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({"fields": {"summary": "Test"}})))
        .respond_with(ResponseTemplate::new(201).set_body_string(r#"{"key":"PROJ-1"}"#))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/api/3/issue",
            "--method",
            "post",
            "--data",
            r#"{"fields":{"summary":"Test"}}"#,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"key\":\"PROJ-1\""));

    // Verify exactly one Content-Type header on the received request
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let content_type_count = requests[0]
        .headers
        .iter()
        .filter(|(name, _)| name.as_str().eq_ignore_ascii_case("content-type"))
        .count();
    assert_eq!(
        content_type_count, 1,
        "expected exactly one Content-Type header, got {content_type_count}"
    );
}
```

- [ ] **Step 8.4: Add handler test for PUT method**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_put_with_method_flag() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/assignee"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/api/3/issue/PROJ-1/assignee",
            "-X",
            "put",
            "-d",
            r#"{"accountId":"abc-123"}"#,
        ])
        .assert()
        .success();
}
```

- [ ] **Step 8.5: Add handler test for custom header passthrough**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_custom_header_passes_through() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/1/organization"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"values":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/servicedeskapi/servicedesk/1/organization",
            "-H",
            "X-ExperimentalApi: opt-in",
        ])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let has_experimental_header = requests[0]
        .headers
        .iter()
        .any(|(name, value)| {
            name.as_str().eq_ignore_ascii_case("x-experimentalapi")
                && value.as_bytes() == b"opt-in"
        });
    assert!(has_experimental_header, "X-ExperimentalApi header missing");
}
```

- [ ] **Step 8.6: Add handler test for custom Content-Type overrides auto-set**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_custom_content_type_overrides_default() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/thing"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    // Note: body must still be valid JSON (we validate at resolve_body stage).
    // The Content-Type override is tested separately from the JSON validation.
    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/api/3/thing",
            "-X",
            "post",
            "-d",
            r#"{"ok":true}"#,
            "-H",
            "Content-Type: application/vnd.atlassian.custom+json",
        ])
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let content_type_values: Vec<String> = requests[0]
        .headers
        .iter()
        .filter(|(name, _)| name.as_str().eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| String::from_utf8_lossy(value.as_bytes()).to_string())
        .collect();
    assert_eq!(
        content_type_values.len(),
        1,
        "expected exactly one Content-Type, got {content_type_values:?}"
    );
    assert_eq!(
        content_type_values[0], "application/vnd.atlassian.custom+json",
        "user-supplied Content-Type must override the default"
    );
}
```

- [ ] **Step 8.7: Add handler test for error response — body to stdout, exit non-zero**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_error_response_body_to_stdout() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/MISSING-1"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_string(r#"{"errorMessages":["Issue does not exist"],"errors":{}}"#),
        )
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/issue/MISSING-1"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("Issue does not exist"))
        // main.rs prints "Error: {e}" where e is JrError::ApiError with Display
        // "API error ({status}): {message}" — stderr contains "(404)" and the extracted message
        .stderr(predicate::str::contains("(404)"))
        .stderr(predicate::str::contains("Issue does not exist"));
}
```

- [ ] **Step 8.8: Add handler test for path normalization**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_path_normalization_missing_slash() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .expect(1)
        .mount(&server)
        .await;

    // No leading slash — should still work
    jr_api_cmd(&server.uri())
        .args(["api", "rest/api/3/myself"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ok\":true"));
}
```

- [ ] **Step 8.9: Add handler test for rejecting absolute URLs**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_rejects_absolute_url() {
    let server = MockServer::start().await;
    // No mock defined — if the handler tries to hit the network, it will fail

    jr_api_cmd(&server.uri())
        .args(["api", "https://example.atlassian.net/rest/api/3/myself"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("do not include the instance URL"));
}
```

- [ ] **Step 8.10: Add handler test for Authorization header rejection**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_rejects_authorization_header() {
    let server = MockServer::start().await;

    jr_api_cmd(&server.uri())
        .args([
            "api",
            "/rest/api/3/myself",
            "-H",
            "Authorization: Bearer pwned",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cannot override the Authorization header",
        ));
}
```

- [ ] **Step 8.11: Add handler test for stdin body (via `write_stdin`)**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_handler_api_stdin_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/thing"))
        .and(body_partial_json(serde_json::json!({"from":"stdin"})))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    jr_api_cmd(&server.uri())
        .args(["api", "/rest/api/3/thing", "-X", "post", "-d", "@-"])
        .write_stdin(r#"{"from":"stdin"}"#)
        .assert()
        .success();
}
```

- [ ] **Step 8.12: Run all handler tests**

```bash
cargo test --test cli_handler test_handler_api
```

Expected: all 10 new `test_handler_api_*` tests PASS.

- [ ] **Step 8.13: Run full test suite**

```bash
cargo test
```

Expected: all tests PASS.

- [ ] **Step 8.14: Clippy clean**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 8.15: Format check**

```bash
cargo fmt --all -- --check
```

Expected: no output (clean). If output appears, run `cargo fmt --all` and include the formatting changes in the commit.

- [ ] **Step 8.16: Commit**

```bash
git add tests/cli_handler.rs
git commit -m "test: add handler integration tests for jr api (#111)"
```

---

## Task 9: Manual smoke test against real Jira

**Context:** Before finalizing, do a lightweight manual verification against a real Jira instance to confirm the end-to-end flow works with real auth and real responses. This catches any gaps the mock-based tests missed.

**Note:** This is optional if the user doesn't have a configured Jira instance handy, but recommended when available.

- [ ] **Step 9.1: Build release binary**

```bash
cargo build --release
```

- [ ] **Step 9.2: Test GET against the live instance**

```bash
./target/release/jr api /rest/api/3/myself
```

Expected: JSON response with the authenticated user's details. Exit code 0.

- [ ] **Step 9.3: Test path without leading slash**

```bash
./target/release/jr api rest/api/3/myself
```

Expected: Same response as Step 9.2.

- [ ] **Step 9.4: Test 404 error response**

```bash
./target/release/jr api /rest/api/3/issue/NOPE-99999
```

Expected: Error body on stdout, `Error: ... (HTTP 404)` on stderr, exit code 1.

- [ ] **Step 9.5: Test piping to jq**

```bash
./target/release/jr api /rest/api/3/myself | jq .accountId
```

Expected: Just the accountId string.

- [ ] **Step 9.6: Test rejection of absolute URL**

```bash
./target/release/jr api https://example.atlassian.net/rest/api/3/myself
```

Expected: Error on stderr: "Use a path like /rest/api/3/... — do not include the instance URL". Exit code 64.

- [ ] **Step 9.7: Test Authorization rejection**

```bash
./target/release/jr api /rest/api/3/myself -H "Authorization: Bearer nope"
```

Expected: Error on stderr: "Cannot override the Authorization header". Exit code 64.

No commit needed — this task is verification only.

---

## Task 10: Verify final state and prepare for PR

- [ ] **Step 10.1: Run the full test suite one more time**

```bash
cargo test
```

Expected: all tests PASS.

- [ ] **Step 10.2: Run clippy**

```bash
cargo clippy -- -D warnings
```

Expected: no warnings.

- [ ] **Step 10.3: Run format check**

```bash
cargo fmt --all -- --check
```

Expected: no output.

- [ ] **Step 10.4: Verify git log looks clean**

```bash
git log --oneline origin/develop..HEAD
```

Expected: ~8 commits, each with a conventional commit prefix referencing `#111`.

- [ ] **Step 10.5: Check for any leftover TODO markers**

```bash
grep -rn "TODO\|FIXME\|XXX" src/cli/api.rs src/api/client.rs
```

Expected: no matches (other than pre-existing ones not introduced by this PR).

---

## Spec Coverage Checklist

- [x] `--method` flag with GET/POST/PUT/PATCH/DELETE → Task 3 (`HttpMethod` enum), Task 6 (wire-up)
- [x] `--data` with inline/`@file`/`@-` → Task 5 (`resolve_body`)
- [x] `--header` repeatable flag → Task 4 (`parse_header`), Task 7 (`handle_api` application)
- [x] Path normalization (leading slash, reject absolute URLs) → Task 3 (`normalize_path`), Task 8.8, Task 8.9
- [x] `send_raw()` preserving non-2xx responses → Task 2
- [x] 429 retry consistent with `send()` → Task 2 Step 2.3
- [x] Body → stdout, errors → stderr → Task 7 Step 7.1
- [x] Exit codes: 0 success, 64 UserError, 2 NotAuthenticated, 1 ApiError/Io/Network → Task 7 Step 7.1
- [x] Authorization header rejection → Task 4 Step 4.3, Task 8.10
- [x] Exactly one Content-Type header when body + custom Content-Type → Task 7 (insert semantics), Task 8.3, Task 8.6
- [x] `extract_error_message` shared helper → Task 1
- [x] Test data: synthetic only (no real project keys/IDs/URLs) → all test tasks use `PROJ-1`, `abc-123`, etc.

---

## Notes for the Implementer

- **Error variant mapping (important):** The codebase uses `JrError::UserError(String)` for bad-input errors (exit 64), NOT `JrError::BadInput`. File read errors propagate via `?` as `JrError::Io` (exit 1). `NotAuthenticated` exits with code 2, not 1. See `src/error.rs:34`.

- **Clippy `too_many_arguments`:** `handle_api` takes 5 arguments which is under the default clippy threshold of 7. If a future refactor bumps it past 7, refactor — do NOT add `#[allow(clippy::too_many_arguments)]` (per `CLAUDE.md`).

- **Header append footgun:** `reqwest::RequestBuilder::header()` uses `HeaderMap::append()` which produces duplicates. The handler must `.build()?` the request first and then manipulate `req.headers_mut()` with `insert()`. Task 7 does this correctly.

- **`request()` method already sets auth:** The existing `JiraClient::request()` at `src/api/client.rs:334` already adds the `Authorization` header. The handler does not need to set it.

- **stdout flushing:** Writing raw bytes to `std::io::stdout()` may buffer — not a concern here because the process exits after the handler returns, flushing everything. If the flow changes to do more work after printing, consider `stdout().flush()`.

- **Test data:** Use placeholders `PROJ-1`, `HELP-42`, `MISSING-1`, `abc-123`. Never use real Jira keys, org IDs, or instance URLs.
