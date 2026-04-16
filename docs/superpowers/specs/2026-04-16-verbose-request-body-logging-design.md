# Verbose Mode Logs Request Body

> **Issue:** #183 — `verbose mode: does not include request body, making silent drops undebuggable`

## Problem

`jr --verbose` currently logs only the HTTP method and URL for outgoing requests:

```
[verbose] PUT https://<site>/rest/api/3/issue/<KEY>
Updated <KEY>
```

When a Jira mutation silently drops a value (e.g. unknown team UUID, unsettable custom field), the user has no way to confirm what payload was actually sent. The only fallback is to hand-trace the source or reach for `curl` with raw credentials — both defeat the purpose of a CLI wrapper.

This is the same diagnostic gap that obscured #181 (silent team mis-resolution) for users in the field. Atlassian's `PUT /rest/api/3/issue/{key}` returns **204 No Content with an empty body and no `warningMessages`** when fields are ignored ("Fields that are not settable will be ignored", per Atlassian REST v3 intro docs), so the response provides no signal either. The request body is the minimum information needed to debug from the client side.

## Design

### Core Change

In `src/api/client.rs`, both `send()` and `send_raw()` already build a clone of the request for inspection (`req.try_clone().and_then(|r| r.build().ok())`). Extend that block to also log the body when present:

```rust
// before (src/api/client.rs:170-174)
if self.verbose {
    if let Some(ref r) = req.try_clone().and_then(|r| r.build().ok()) {
        eprintln!("[verbose] {} {}", r.method(), r.url());
    }
}

// after
if self.verbose {
    if let Some(ref r) = req.try_clone().and_then(|r| r.build().ok()) {
        eprintln!("[verbose] {} {}", r.method(), r.url());
        if let Some(bytes) = r.body().and_then(|b| b.as_bytes()) {
            eprintln!("[verbose] body: {}", String::from_utf8_lossy(bytes));
        }
    }
}
```

`send_raw()` (`client.rs:244-246`) gets the same treatment, except the request is already built (no `try_clone().build()` step).

`reqwest::Body::as_bytes() -> Option<&[u8]>` returns `Some` for in-memory bodies (which `.json()` produces — it serializes via `serde_json::to_vec` and stores the bytes). It returns `None` for streaming bodies. We have no streaming body callers today, so the `None` arm is just defensive — no body line gets printed.

### Output

For a write call:

```
[verbose] PUT https://example.atlassian.net/rest/api/3/issue/HDL-1
[verbose] body: {"fields":{"summary":"new summary","priority":{"name":"Medium"}}}
```

For a GET (no body):

```
[verbose] GET https://example.atlassian.net/rest/api/3/issue/HDL-1
```

The body line is omitted entirely when there is no body — no empty `body: ` line, no `body: null`. This keeps `--verbose` for read commands unchanged.

### Format

- **Compact JSON, not pretty-printed.** `.json(body)` already produces compact output via `serde_json::to_vec`. Don't re-parse and re-format — that adds CPU cost and risks transformation bugs (e.g. key reordering). The output is one line per request, easy to grep.
- **`String::from_utf8_lossy`** rather than `from_utf8`: bodies are always JSON (UTF-8 by definition), but lossy-decode is cheap insurance against a malformed payload crashing the verbose log instead of producing a useful diagnostic.
- **`[verbose] body:` prefix** mirrors the existing `[verbose]` prefix on the URL line, so a single `grep '\[verbose\]'` captures the whole trace.

### What Does NOT Change

| Item | Reason |
|------|--------|
| Default (non-verbose) output | Spec only touches the `if self.verbose` arm |
| Response body | Out of scope — would require restructuring every typed wrapper (`get<T>`, `post<T>`, `put`, `post_no_content`, `delete`) to read bytes-then-parse. Filed as a follow-up |
| `--debug` flag | Not introduced — see "Flag choice" below |
| Header logging | Not requested in #183; `Authorization` would need redaction. Out of scope |
| Body redaction | All `JiraClient::send()` bodies are user content (issue fields, comments, transitions). OAuth `client_secret` and `refresh_token` flow through a separate `reqwest::Client` in `src/api/auth.rs:163,222` that bypasses `send()` entirely, so the verbose path never sees credentials. The `jr api` raw passthrough (`send_raw`) could echo a user-typed `-d '{...}'` payload — caller responsibility, same as `curl -v` |

### Flag Choice

The issue accepts either `--verbose` or a new `--debug` flag. Sticking with `--verbose` because:

- Convention (`curl -v`, `kubectl -v`, `gh -v`, `httpie -v`) keeps bodies out of `-v` to avoid drowning the user in TLS handshakes, redirect chains, and header dumps. Our `--verbose` currently emits **one URL line per call** — there is no noise budget being protected. Adding a body line moves us from "useless verbose" to "minimum useful verbose".
- Issue author explicitly OK with `--verbose`: "verbose currently implies 'show me what you're doing' and the body is the most important part of that".
- Avoids introducing a flag for noise that does not exist. If verbose ever grows headers/timing/response-body, that's the moment to add `--debug` or `-vv`.

## Files Changed

| File | Change |
|------|--------|
| `src/api/client.rs` | `send()` (~line 170): extend verbose block with body line |
| `src/api/client.rs` | `send_raw()` (~line 244): extend verbose block with body line |
| `tests/cli_handler.rs` | Add 3 handler tests: PUT body appears in stderr under `--verbose`; GET body line omitted; `send_raw` (used by `jr api`) emits the literal `-d` payload |

## Testing

**Handler tests** (in `tests/cli_handler.rs` since this is end-to-end CLI behavior, not a unit boundary):

1. **`test_verbose_logs_request_body_for_put`** — wiremock 204 for `PUT /rest/api/3/issue/HDL-1`, run `jr --verbose issue edit HDL-1 -s "new summary"`, assert stderr contains both `[verbose] PUT` and `[verbose] body: {"fields":` (substring match — exact serialized form depends on serde struct field order, so substring is more robust than exact-equality).
2. **`test_verbose_omits_body_line_for_get`** — wiremock 200 for `GET /rest/api/3/issue/HDL-1`, run `jr --verbose issue view HDL-1 --output json`, assert stderr contains `[verbose] GET` and does **not** contain `[verbose] body:`.

No new unit tests in `client.rs`. The verbose-logging branch has no return value or business logic — it's pure I/O. End-to-end tests are the right level.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Body is not buffered (streaming) | `as_bytes()` returns `None` → no body line printed (defensive; no current callers) |
| Body has invalid UTF-8 | `from_utf8_lossy` substitutes replacement chars; no panic |
| `verbose` is false | Block is skipped entirely; zero overhead |

## Alignment with Project Conventions

- **Thin client, no abstraction layer** — change is in the existing `send()`/`send_raw()` methods, no new abstractions
- **Machine-output-first** — verbose output goes to **stderr** (already established by existing `eprintln!`), so `cmd > out.json` keeps stdout clean and `cmd 2> trace.log --verbose` captures the trace
- **Pipe-friendly** — body line is `[verbose] body: {...}` so `grep '\[verbose\] body:'` captures payloads cleanly
- **Non-interactive by default** — verbose is opt-in, not affected by TTY detection
- **Idempotent reads** — verbose adds zero behavioral change to requests, only logging

## Validation

- **Context7 (`/websites/rs_reqwest`):** `reqwest::Body::as_bytes(&self) -> Option<&[u8]>` exists on the async type and returns `Some(&[u8])` for buffered bodies. `.json(body)` produces a buffered body via `serde_json::to_vec`. The existing `try_clone().build()` pattern in `client.rs:171` is the supported way to produce an inspectable `Request` from a `RequestBuilder`.
- **Perplexity (2026-04-16):** Atlassian docs confirm `PUT /rest/api/3/issue/{key}` returns 204 No Content (per JRACLOUD-37536) with no `warningMessages` and no error indicators when fields are silently ignored. The request body is the only client-side signal available.
- **Perplexity (2026-04-16):** `curl -v` does NOT print request bodies (uses `--trace-ascii` for that); `httpie -v` shows headers, `-vv` adds bodies; `kubectl` reserves bodies for `-v=8+`; `gh -v` does not print bodies. Convention is bodies-out-of-verbose for tools whose `-v` already includes TLS/headers/timings. Our `--verbose` is currently empty of that detail, so the convention argument doesn't bind here.
- **Codebase audit:** all OAuth credential exchanges (`src/api/auth.rs:163-186, 222-244`) use a separate `reqwest::Client::new()` and never traverse `JiraClient::send()`. The verbose-logging path will not see `client_secret` or `refresh_token` payloads.
- **Codebase audit:** `src/api/client.rs:170-174, 244-246` are the only `[verbose]` log sites for outgoing requests. No third site to update.

## Out of Scope (Follow-Ups)

| Item | Why deferred |
|------|--------------|
| Response body logging | Touches every typed wrapper (`get<T>`, `post<T>`, `put`, etc.) — bytes-then-parse refactor. File as separate issue to keep this PR focused on the issue-as-filed |
| Response status / headers | Same as above; out of #183 scope |
| `--debug` flag with separate verbosity tier | YAGNI until verbose grows enough to need a noise budget |
| Body redaction policy for `jr api` raw passthrough | User-typed `-d` payloads are caller responsibility; matches `curl -v` semantics |
