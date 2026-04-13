# 429 Retry Exhaustion Warning

> **Issue:** #172 — jr api: 429 retry exhaustion is invisible to users

## Problem

When `JiraClient::send_raw` or `JiraClient::send` exhaust `MAX_RETRIES` (3) for HTTP 429 responses, the user gets no indication that retries were attempted. They see a delayed error (or delayed 429 response from `jr api`) with no explanation for the hang.

## Design

### Approach

Add a non-verbose `eprintln!` warning to both `send` and `send_raw` when the final retry attempt returns 429. The warning fires unconditionally (not gated on `--verbose`) because retry exhaustion is operationally significant — users need to know jr absorbed backoff time on their behalf.

### Warning Message

```
warning: rate limited by Jira — gave up after 3 retries
```

- Lowercase `warning:` prefix matches Rust ecosystem convention (`cargo`, `rustc`) and existing jr patterns (`src/cli/issue/list.rs:362`, `src/cli/board.rs:205`)
- Includes retry count for debuggability (Perplexity-validated convention)
- Does not include total delay — per-retry delays are already visible via `--verbose`

### Placement

**`send_raw`** (`src/api/client.rs`): The retry loop iterates `0..=MAX_RETRIES`. On the final attempt (`attempt == MAX_RETRIES`), if the response is 429, the retry condition `attempt < MAX_RETRIES` is false, so execution falls through to `return Ok(response)`. The warning is inserted just before this return, guarded by a 429 status check:

```rust
if response.status() == StatusCode::TOO_MANY_REQUESTS {
    eprintln!("warning: rate limited by Jira — gave up after {MAX_RETRIES} retries");
}
return Ok(response);
```

**`send`** (`src/api/client.rs`): Same loop structure. On the final attempt, a 429 falls through the retry condition and hits the `is_client_error()` check, which converts it to `JrError::ApiError`. The warning is inserted before this check:

```rust
if response.status() == StatusCode::TOO_MANY_REQUESTS {
    eprintln!("warning: rate limited by Jira — gave up after {MAX_RETRIES} retries");
}
if response.status().is_client_error() || response.status().is_server_error() {
    return Err(Self::parse_error(response).await);
}
```

### What Does NOT Change

- Return types of `send` or `send_raw`
- Error message format (`JrError::ApiError` Display)
- Exit codes
- `--output json` behavior
- Stdout output (warning is stderr only)

## Testing

**1 new handler subprocess test** in `tests/cli_handler.rs`:

| Test | Setup | Assertion |
|------|-------|-----------|
| `test_api_warns_on_429_retry_exhaustion` | wiremock returns 429 for all requests (persistent mock) | stderr contains `"warning: rate limited by Jira"` and `"3 retries"` |

Uses the existing `jr_api_cmd` helper + `assert_cmd` stderr assertions (`predicate::str::contains`), matching the project's established pattern (22 existing stderr assertions across handler and smoke tests).

No unit test needed — `eprintln!` is not capturable in-process without external crates, and the subprocess test reliably captures stderr.

## Files Changed

| File | Change |
|------|--------|
| `src/api/client.rs` | Add `eprintln!` warning to both `send` and `send_raw` retry loops |
| `tests/cli_handler.rs` | Add `test_api_warns_on_429_retry_exhaustion` handler test |

## Validation

- **Perplexity:** Confirmed lowercase `warning:` is Rust CLI convention; recommended including retry count
- **Context7 (reqwest):** No built-in retry mechanism — retry logic is entirely custom in jr
- **Context7 (gh CLI):** gh does not retry 429s at all — no precedent to follow; jr's retry is already more user-friendly
- **Codebase:** Existing `warning:` pattern in `list.rs` and `board.rs` confirms format choice
