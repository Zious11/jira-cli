# Raw API Passthrough Command (`jr api`) — Design Spec

**Issue:** #111
**Status:** Draft
**Date:** 2026-04-08

## Problem

When a high-level `jr` command doesn't cover a use case, there's no way to fall back to the raw Jira REST API using `jr`'s stored credentials. Credentials live in the macOS keychain with no programmatic extraction path, so users can't pipe them to `curl`. The result: an agent or script hits a rough edge and has no escape hatch.

## Solution

Add a top-level `jr api` command that sends an arbitrary HTTP request to the configured Jira instance using the stored auth, modeled on `gh api`. Method, path, request body, and custom headers are all controlled by flags. The response body is printed to stdout as-is; human error messages go to stderr; the exit code conveys success or failure.

## Command Surface

```
jr api <path> [flags]

Arguments:
  <path>    API path (leading slash optional; absolute URLs rejected)

Flags:
  -X, --method <METHOD>       HTTP method [default: GET]
                              Possible values: GET, POST, PUT, PATCH, DELETE
  -d, --data <BODY>           Request body: inline JSON, @file, or @- for stdin
  -H, --header <KEY:VALUE>    Custom header (repeatable)
```

### Examples

```bash
# GET (default method)
jr api /rest/api/3/myself

# Leading slash is optional
jr api rest/api/3/myself

# PUT with inline JSON body
jr api /rest/api/2/issue/PROJ-1/assignee \
  --method PUT \
  --data '{"accountId":"abc123"}'

# POST with body from file
jr api /rest/agile/1.0/sprint/123/issue \
  --method POST \
  --data @payload.json

# POST with body from stdin
echo '{"issues":["PROJ-456"]}' | jr api /rest/agile/1.0/sprint/123/issue -X POST -d @-

# Custom header for experimental JSM API
jr api /rest/servicedeskapi/servicedesk/1/organization \
  -H "X-ExperimentalApi: opt-in"

# Pipe response to jq (raw JSON enables composition)
jr api /rest/api/3/myself | jq .accountId
```

### Design Choices

- **No placeholder magic** (unlike `gh api`'s `{owner}/{repo}`) — `jr` has no equivalent "current repo" notion; users pass literal paths.
- **Stdout is always raw** — `jr api` prints the server's response body to stdout verbatim, regardless of the global `--output` flag. However, stderr error formatting (on non-2xx responses) still follows `--output` — when `--output json` is set, errors appear as `{"error":"...","code":N}` on stderr, consistent with all other `jr` commands.
- **No built-in `--jq`, `--paginate`, `--field`** — users pipe to `jq` or handle pagination via URL query params. More composable, follows Unix philosophy, smaller surface area.
- **Path normalization:** if the path does not start with `/`, prepend one. Absolute URLs (starting with `http://` or `https://`) are rejected with `UserError` — the instance URL comes from config.
- **`@file` / `@-` curl conventions** for body input. A filename literally starting with `@` requires `./` prefix (documented footgun, identical to curl).

## Architecture

### Files Changed

| File | Change | Description |
|------|--------|-------------|
| `src/cli/mod.rs` | Modify | Add `Api { path, method, data, header }` variant to `Command` enum |
| `src/cli/api.rs` | **Create** | New module: `handle_api()`, `HttpMethod` enum, body/header helpers, unit tests |
| `src/api/client.rs` | Modify | Add `send_raw()` method (preserves non-2xx responses); extract `extract_error_message(body: &[u8]) -> String` helper from `parse_error` so both `send()`-error-path and `jr api` can reuse it |
| `src/main.rs` | Modify | Dispatch `Command::Api { ... }` to `cli::api::handle_api()` |
| `tests/cli_handler.rs` | Modify | Add handler tests for the full flow |
| `tests/api_client.rs` | Modify | Add `send_raw()` client-level tests |

### Why a New Top-Level Command

`jr api` is an escape hatch that spans all Jira APIs — issues, boards, sprints, JSM, assets, agile. Placing it under any specific resource module would be misleading. It sits alongside `issue`, `board`, `sprint`, etc. as a peer subcommand.

### Why a New `send_raw()` Method

The existing `send()` at `src/api/client.rs:157` parses non-2xx responses into `JrError::ApiError`, which consumes the response body and destroys the raw JSON. For a raw passthrough, we need to:

1. Keep 429 retry (consistent with every other `jr` command)
2. Skip error parsing — return `reqwest::Response` for any status code
3. Let the caller read the raw body and decide on exit code

The new method reuses the 429 retry loop from `send()` and differs only in the final step: it returns the `Response` directly for 2xx AND 4xx/5xx, with no error parsing. `send_raw()` is ~30 lines.

### Module Layout

`src/cli/api.rs` is estimated at ~200 lines:
- `HttpMethod` enum (`ValueEnum` derive) — 15 lines
- `parse_header(s: &str) -> Result<(HeaderName, HeaderValue)>` — 20 lines
- `normalize_path(s: &str) -> Result<String>` — 15 lines
- `resolve_body<R: Read>(arg: Option<&str>, stdin: R) -> Result<Option<String>>` — 40 lines
- `handle_api(...) -> Result<()>` — 60 lines
- Unit tests — 50 lines

Small enough to stay in one file.

## Request and Response Flow

### Step-by-Step

1. **Parse args** — clap derives method, path, data, header list.
2. **Normalize path** (`normalize_path`):
   - If starts with `http://` or `https://` → `JrError::UserError` ("Use a path like /rest/api/3/... — do not include the instance URL")
   - If starts with `/` → use as-is
   - Otherwise → prepend `/`
3. **Resolve body** (`resolve_body`):
   - `None` → no body
   - `Some("@-")` → read entire stdin into a `String`
   - `Some("@filename")` → read entire file into a `String`
   - `Some(inline)` → use as-is
4. **Validate body is JSON** if present — `serde_json::from_str::<Value>(&body)`. On parse error, `JrError::UserError("Request body is not valid JSON: {err}")`.
5. **Parse headers** (`parse_header`): split each `-H` value on the **first** `:`, trim whitespace on both sides. Empty key or missing `:` → `JrError::UserError("Header must be in 'Key: Value' format")`. Reject any user-supplied `Authorization` header (case-insensitive match) → `JrError::UserError("Cannot override the Authorization header — auth is managed by jr")`. This prevents accidental credential leakage via `--verbose` output and ensures the escape hatch always uses the stored credentials.
6. **Build request:**
   - Start with `client.request(method, &path)` — returns a `RequestBuilder` with URL and auth header set
   - `.build()?` to get a concrete `reqwest::Request`
   - If body present: `req.body_mut().replace(body.into())` and insert `Content-Type: application/json` via `req.headers_mut().insert()`
   - For each parsed custom header (already validated to exclude `Authorization` in step 5), call `req.headers_mut().insert(name, value)` — this **replaces** any existing header of the same name (including auto-set `Content-Type`), giving the user final say
7. **Send** — call `client.send_raw(req)`. Retries 429 automatically. Returns `reqwest::Response` regardless of status code.
8. **Read body** — `response.bytes().await?`.
9. **Print body to stdout** — write bytes as-is (no parsing, no reformatting). Preserves whitespace and key order from the server.
10. **Handle status:**
    - 2xx → exit 0
    - 4xx/5xx → extract a human-readable summary from the body using the same logic as `JiraClient::parse_error` at `src/api/client.rs:219` (tries `errorMessages` array, then `message` string, then falls back to the raw body). Print `Error: {message} (HTTP {status})` to stderr via `output::print_error`. Return `JrError::ApiError { status, message }` (401 → `JrError::NotAuthenticated`). The existing `parse_error` takes a `Response` which consumes the body — since we've already consumed the body bytes, extract the JSON-parsing logic into a shared helper `extract_error_message(body: &[u8]) -> String` and call it from both `parse_error` and `handle_api`.

### Exit Codes

| Scenario | Exit Code | Source |
|----------|-----------|--------|
| 2xx response | 0 | success |
| 4xx/5xx response | 1 | `JrError::ApiError` |
| 401 response | 2 | `JrError::NotAuthenticated` |
| Invalid path, bad JSON body, bad header format | 64 | `JrError::UserError` |
| File read error (`@file`) | 1 | `JrError::Io` (propagated via `?`) |
| Network error | 1 | `JrError::NetworkError` |

Note: exit codes are derived from the `impl JrError::exit_code()` at `src/error.rs:34`. `UserError` → 64; `NotAuthenticated` → 2; all other variants → 1.

### Stdout/Stderr Split

| Stream | Content |
|--------|---------|
| **stdout** | Response body (success or error, raw bytes, no reformatting) |
| **stderr** | Human-readable error summary on HTTP failure (e.g., `Error: Not Found (HTTP 404)`) |

This matches `gh api`'s behavior and enables clean composition:

```bash
jr api /rest/api/3/myself | jq .accountId
```

### Header Precedence

- Auth header is set automatically via `client.request()` — should not be overridable for safety
- `Content-Type: application/json` is auto-set when a body is present
- Custom headers via `-H` apply with **replace** semantics (`HeaderMap::insert`) — the user's value wins over auto-set Content-Type
- Invariant: the outgoing HTTP request has exactly one header for each distinct header name
- **Implementation note:** `RequestBuilder::header()` uses `append()` semantics which produces duplicates. The implementation must build the `Request` via `.build()?`, then manipulate `req.headers_mut()` directly with `insert()`.

### 429 Retry

`send_raw()` retries 429 responses up to `MAX_RETRIES` (3) using the same `Retry-After` logic as `send()`. This is intentional and consistent with every other `jr` command — agents running `jr api` in scripts benefit from automatic backoff, and Jira's documented rate limit behavior expects clients to honor `Retry-After`.

## Type Changes

Add to `src/cli/api.rs`:

```rust
#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}
```

Add to `src/cli/mod.rs` (`Command` enum):

```rust
/// Make a raw authenticated HTTP request to the Jira REST API.
Api {
    /// API path (leading slash optional). Example: /rest/api/3/myself
    path: String,

    /// HTTP method
    #[arg(short = 'X', long, value_enum, default_value_t = HttpMethod::Get)]
    method: HttpMethod,

    /// Request body: inline JSON, @file to read from a file, or @- to read from stdin
    #[arg(short = 'd', long)]
    data: Option<String>,

    /// Custom header in "Key: Value" format (repeatable)
    #[arg(short = 'H', long = "header")]
    header: Vec<String>,
},
```

Add to `src/api/client.rs`:

```rust
/// Send a pre-built request without parsing non-2xx responses into errors.
/// Retries 429 up to MAX_RETRIES times. Returns the raw Response for any status code.
pub async fn send_raw(&self, request: reqwest::Request) -> anyhow::Result<reqwest::Response>
```

## Error Messages

| Scenario | Message | Exit Code |
|----------|---------|-----------|
| Path starts with `http://` or `https://` | `Use a path like /rest/api/3/... — do not include the instance URL` | 64 |
| Body is not valid JSON | `Request body is not valid JSON: <serde error>` | 64 |
| Header missing `:` | `Header must be in 'Key: Value' format (got: <value>)` | 64 |
| Header key is empty | `Header key cannot be empty` | 64 |
| User-supplied `Authorization` header | `Cannot override the Authorization header — auth is managed by jr` | 64 |
| `@file` does not exist | Propagated I/O error from `std::fs::read_to_string` via `JrError::Io` (for example, `No such file or directory`) | 1 |
| 401 response | `Not authenticated. Run "jr auth login" to connect.` | 2 |
| Other HTTP error | `Error: API error (<status>): <errorMessages or message>` on stderr, body on stdout | 1 |

## Testing

### Unit Tests (`src/cli/api.rs`)

| Test | What it verifies |
|------|------------------|
| `test_normalize_path_with_slash` | `/rest/api/3/myself` → unchanged |
| `test_normalize_path_without_slash` | `rest/api/3/myself` → `/rest/api/3/myself` |
| `test_normalize_path_rejects_http_url` | `http://site.atlassian.net/foo` → `UserError` |
| `test_normalize_path_rejects_https_url` | `https://site.atlassian.net/foo` → `UserError` |
| `test_parse_header_valid` | `"X-Foo: bar"` → `("X-Foo", "bar")` |
| `test_parse_header_no_colon` | `"X-Foo bar"` → `UserError` |
| `test_parse_header_empty_key` | `": bar"` → `UserError` |
| `test_parse_header_trims_whitespace` | `"X-Foo:   bar  "` → `("X-Foo", "bar")` |
| `test_parse_header_value_with_colon` | `"X-Request-Id: abc:def"` → `("X-Request-Id", "abc:def")` (first-colon split) |
| `test_parse_header_rejects_authorization` | `"Authorization: Bearer foo"` → `UserError` |
| `test_parse_header_rejects_authorization_case_insensitive` | `"authorization: Bearer foo"` → `UserError` |
| `test_resolve_body_none` | `None` → `Ok(None)` |
| `test_resolve_body_inline_json` | `Some("{\"a\":1}")` → `Ok(Some("{\"a\":1}"))` |
| `test_resolve_body_invalid_json` | `Some("not json")` → `UserError` |
| `test_resolve_body_at_file` | `Some("@/tmp/test.json")` reads the file contents |
| `test_resolve_body_at_file_not_found` | `Some("@/no/such/file")` → `InputError` |
| `test_resolve_body_at_dash_reads_stdin` | `Some("@-")` with injected `Cursor` → body matches Cursor content |

### Handler Tests (`tests/cli_handler.rs`)

Wiremock-backed, call `handle_api()` directly. Each test uses `JiraClient::new_for_test(server.uri(), ...)`.

| Test | What it verifies |
|------|------------------|
| `test_handler_api_get_success` | Mocks GET `/rest/api/3/myself` → 200 JSON. Verifies Authorization header present, response printed to stdout, exit 0 |
| `test_handler_api_post_with_inline_data` | `-d '{"a":1}'` → request body matches, exactly one `Content-Type: application/json` header |
| `test_handler_api_put_with_method_flag` | `--method PUT` → request uses PUT |
| `test_handler_api_custom_header_overrides_content_type` | Body + `-H "Content-Type: text/plain"` → exactly ONE Content-Type header with value `text/plain` |
| `test_handler_api_custom_header_passes_through` | `-H "X-ExperimentalApi: opt-in"` → header present on the request |
| `test_handler_api_error_response_body_to_stdout` | 404 with JSON body → body on stdout, `ApiError` returned |
| `test_handler_api_path_normalization` | `rest/api/3/myself` (no leading slash) → request sent to `/rest/api/3/myself` |

**Exactly-one-header pattern:** Use `server.received_requests().await` to inspect the raw request. Count headers matching a name case-insensitively. This approach works regardless of how reqwest normalizes or merges headers on the wire.

```rust
let requests = server.received_requests().await.unwrap();
let content_type_count = requests[0]
    .headers
    .iter()
    .filter(|(name, _)| name.as_str().eq_ignore_ascii_case("content-type"))
    .count();
assert_eq!(content_type_count, 1);
```

### Client Tests (`tests/api_client.rs`)

| Test | What it verifies |
|------|------------------|
| `test_send_raw_returns_response_for_2xx` | `send_raw` returns `Response` for 200 |
| `test_send_raw_returns_response_for_404` | `send_raw` returns `Response` (NOT an error) for 404 — critical for raw passthrough |
| `test_send_raw_retries_429` | `send_raw` retries 429 with `Retry-After`, then returns 200 response |
| `test_send_raw_returns_response_after_exhausted_429` | After `MAX_RETRIES` 429s, returns the 429 `Response` (caller decides what to do) |
| `test_extract_error_message_from_error_messages` | `{"errorMessages":["foo","bar"]}` → `"foo; bar"` |
| `test_extract_error_message_from_message_field` | `{"message":"foo"}` → `"foo"` |
| `test_extract_error_message_from_plain_text` | `"not json"` → `"not json"` (fallback) |
| `test_extract_error_message_from_empty_body` | `""` → `"<empty response body>"` |

### Stdin Testing Approach

The `resolve_body` function takes `stdin: impl Read` so unit tests can pass a `Cursor` with synthetic content. `handle_api()` calls `std::io::stdin().lock()` internally and passes the result to `resolve_body()` — this matches the existing codebase pattern (`src/cli/issue/workflow.rs:402`, `src/cli/issue/create.rs:84`) where handlers call `stdin()` directly without dependency injection.

This PR uses layered coverage for stdin behavior: unit tests exercise `resolve_body()` directly (including `@-`) with synthetic readers, and `tests/cli_handler.rs` also adds subprocess coverage via `Command::cargo_bin("jr")` to validate the real CLI entrypoint. That subprocess coverage includes an `@-` stdin case, so the end-to-end stdin path is tested as implemented, not just the helper in isolation.

### Test Data

All JSON is synthetic. No real project keys, org IDs, account IDs, or instance URLs. Use placeholders like `PROJ-1`, `HELP-42`, `abc123`.

## Caveats

- **Header append footgun:** `reqwest::RequestBuilder::header()` appends rather than replaces. The implementation must build the `Request` via `.build()` and manipulate `req.headers_mut()` directly with `insert()`. The exactly-one-header test enforces this invariant.
- **Auth header cannot be overridden:** User-supplied `-H Authorization: ...` is rejected with a `UserError` error. Auth is managed by `jr` via `client.request()`, and explicit rejection prevents accidental credential leakage via `--verbose` output.
- **Body size:** The entire body is read into memory before sending. Not suitable for very large payloads (multi-MB uploads) — but Jira's standard API is not typically used for large payloads.
- **Streaming responses:** `jr api` reads the entire response into memory before printing. Fine for JSON payloads; not suitable for streaming endpoints (Jira has none in practice).
- **`@` prefix in filenames:** A filename literally starting with `@` must be passed as `./@file.json` to avoid being interpreted as a nested reference. Matches curl's behavior.
- **Error message extraction:** `extract_error_message` now handles `errorMessages` array, `errors` object (field-level validation), `message` string, `errorMessage` singular, and empty bodies. See #171.

## Out of Scope

- `--jq` / `--template` / `--slurp` response filtering — users pipe to `jq` directly for composition
- `--paginate` — users handle pagination via URL query params (`startAt`, `maxResults`, `nextPageToken`)
- `--field` / `--raw-field` / `-F`/`-f` — users construct JSON bodies themselves or via `jq`
- `--include` / `-i` — response headers not exposed in v1
- Multipart/form-data (file uploads) — deferred; the existing `add_comment` ADF path handles most text-based use cases
- GraphQL endpoint support — Jira's REST API covers the issue's use cases; GraphQL can be added as a future enhancement
- Request/response logging beyond the existing `--verbose` flag
