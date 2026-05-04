---
context: error-taxonomy
title: "Error Taxonomy"
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.X error sections
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §3.1 (JrError variants)
  - Source P8: .factory/semport/jira-cli/jira-cli-pass-8-deep-synthesis.md §6.1 (design patterns)
---

# Error Taxonomy — jira-cli

## Section 1: JrError Variant Catalog

11 variants (corrected from Pass 1 broad's "10" at CONV-ABS §7.3). Source: `src/error.rs`.

| Variant | Exit Code | Category | Severity | When Raised |
|---|---|---|---|---|
| `NotAuthenticated` | 2 | Auth | BROKEN | 401 response from any authenticated endpoint; no token in keychain |
| `InsufficientScope` | 2 | Auth | BROKEN | 401 response with body containing `"scope does not match"` |
| `NetworkError` | 1 | Transport | BROKEN | DNS failure, TLS error, connection refused, timeout |
| `ApiError(status, msg)` | 1 | API | BROKEN | HTTP 4xx (except 401/403/429) or 5xx after retry exhaustion |
| `ConfigError(msg)` | 78 | Config | BROKEN | Malformed TOML, missing required field, migration failure |
| `UserError(msg)` | 64 | User | BROKEN | Bad flag combination, validation failure, ambiguous match |
| `Internal(msg)` | 1 | Internal | BROKEN | Unexpected code path, logic error, unwrap on None in unreachable |
| `Interrupted` | 130 | Signal | — | Ctrl+C received (tokio::select! in main.rs) |
| `Http(#[from] reqwest::Error)` | 1 | Transport | BROKEN | Low-level reqwest transport errors not covered by NetworkError |
| `Io(#[from] std::io::Error)` | 1 | IO | BROKEN | File read/write failures (config, cache) |
| `Json(#[from] serde_json::Error)` | 1 | Parse | BROKEN | Response body deserialization failure |

### Exit Code Semantics

| Exit Code | Meaning | JrError Variants |
|---|---|---|
| 0 | Success | (no error) |
| 1 | Runtime error (API, network, IO, internal) | `NetworkError`, `ApiError`, `Internal`, `Http`, `Io`, `Json` |
| 2 | Authentication error | `NotAuthenticated`, `InsufficientScope` |
| 64 | User error (bad input, validation failure) | `UserError` |
| 78 | Configuration error | `ConfigError` |
| 130 | Interrupted (Ctrl+C) | `Interrupted` |

### JSON Error Shape (--output json)

When `--output json` is active AND a `JrError` is raised, output goes to **stderr** as:

```json
{"error": "<human message>", "code": <exit_code>}
```

- `error`: same message as human-readable stderr would show
- `code`: integer exit code per table above
- Output channel: stderr (NOT stdout; stdout reserved for data)

---

## Section 2: `extract_error_message` 7-Step Precedence Chain

Source: `src/api/client.rs:448-490` (`extract_error_message` function). Corrected from broad pass per CONV-ABS-004; further corrected per ADV-P2-001 (empty body returns literal string, not None; no nested messages[] level; no errorDescription).

| Priority | Condition | Behavior |
|---|---|---|
| 1 (HIGHEST) | Response body byte length == 0 | Return literal string `"<empty response body>"` (early return; no UTF-8 or JSON parsing) |
| 2 | Body bytes are non-UTF-8 | Return `String::from_utf8_lossy(body)` with Unicode replacement chars (early return) |
| 3 | Body is JSON with `errorMessages` array having ≥1 string element | Return elements joined with `"; "` |
| 4 | Body is JSON with non-empty `errors` object | Return `"field: value"` pairs alphabetically sorted, joined with `"; "`; non-string values use `serde_json::Value` display |
| 5 | Body is JSON with top-level `message` string field | Return the string value as-is |
| 6 | Body is JSON with top-level `errorMessage` string field (JSM endpoints) | Return the string value as-is |
| 7 (FALLBACK) | Body is non-JSON OR JSON with no recognized fields matched above | Return raw body string (valid UTF-8 already confirmed at step 2) |

**Key invariants**:
- Step 1 returns a STRING (not None). The string `"<empty response body>"` propagates into `JrError::ApiError { message }`. There is no status-code-derived substitution.
- The function doc comment inside client.rs lists a different order ("1. errorMessages … 5. Empty body") — that comment is STALE. Code execution order above is authoritative.
- `errors.field.messages[]` (nested messages array) is NOT a recognized level. Non-string error values are rendered via `serde_json::Value::to_string()` (curly-brace JSON).
- `errorDescription` is NOT a recognized field. Only `errorMessage` (singular) is supported.

---

## Section 3: Per-Status-Code Error Mapping

### 4xx Responses

| HTTP Status | JrError Variant | Exit Code | Message Pattern |
|---|---|---|---|
| 400 | `ApiError(400, extracted_msg)` | 1 | Extracted message or `"Bad request"` |
| 400 with `resolution` field | `UserError(...)` | 64 | `"Field 'resolution' is required"` → hint: `--resolution`, `jr issue resolutions` |
| 401 (general) | `NotAuthenticated` | 2 | `"Not authenticated. Run: jr auth login"` |
| 401 with scope mismatch | `InsufficientScope` | 2 | `"Insufficient token scope. <details>. Run: jr auth login"` |
| 403 | `ApiError(403, ...)` | 1 | `"Forbidden"` or extracted body message |
| 404 | `ApiError(404, ...)` | 1 | `"Not found: <resource>"` |
| 409 | `ApiError(409, ...)` | 1 | Extracted message |
| 422 | `ApiError(422, ...)` | 1 | Extracted message |
| 429 | Retry (up to MAX_RETRIES=3) | — | Final retry → return 429 response to caller (NOT error for `send_raw`) |

### 5xx Responses

| HTTP Status | JrError Variant | Exit Code | Message Pattern |
|---|---|---|---|
| 500 | `ApiError(500, ...)` | 1 | `"API error (500)"` |
| 502 | `ApiError(502, ...)` | 1 | `"API error (502)"` |
| 503 | `ApiError(503, ...)` | 1 | `"API error (503)"` |
| 5xx (after MAX_RETRIES=3) | `ApiError(status, ...)` | 1 | `"API error (<status>)"` |

---

## Section 4: Remediation Conventions

Every error message must suggest a next action. Conventions by category:

| Category | Suggestion Template |
|---|---|
| NotAuthenticated | `"Run: jr auth login"` or `"Run: jr auth refresh"` |
| InsufficientScope | `"Re-authenticate with required scopes. See: github.com/Zious11/jira-cli/issues/185"` |
| NetworkError | `"Could not reach <host>. Check your network connection."` |
| ApiError (generic) | `"API error (<status>). Check jr auth status."` |
| ConfigError | `"Configuration error: <details>. Check ~/.config/jr/config.toml"` |
| UserError (ambiguous) | `"Ambiguous <thing>: <candidates>"` + list of candidates |
| UserError (validation) | `"Invalid <thing>: <details>"` + valid format hint |

---

## Section 5: `partial_match` Error Semantics

`MatchResult` is a 4-state enum used by status disambiguation, user lookup, and asset status filtering:

| MatchResult variant | When | Error behavior |
|---|---|---|
| `Exact` | Exactly one exact case-insensitive match | No error — use the match |
| `ExactMultiple` | Multiple exact-case matches (same string, different case) | No error — use any (or first) |
| `Ambiguous` | Single substring match overlaps multiple candidates | `UserError` (exit 64) + list all candidates |
| `None` | Zero matches | `UserError` (exit 64) + "not found" |

**Invariant**: Single-substring match is always `Ambiguous` regardless of match count. This is fail-closed design — `partial_match` never silently auto-selects when multiple candidates share a substring.

---

## Section 6: Domain-Specific Error Messages

### Sprint Commands

| Condition | Error | Exit Code |
|---|---|---|
| `sprint list`/`sprint current` on kanban board | `"Sprint commands are only available for scrum boards"` | 1 |
| `sprint add --sprint` + `--current` together | clap error (mutual exclusion) | non-zero |
| `sprint add` with no `--sprint` or `--current` | clap error (required one-of) | non-zero |

### Asset Commands

| Condition | Error | Exit Code |
|---|---|---|
| `validate_asset_key` invalid format | `"Invalid asset key: <key>. Expected: PREFIX-NNN"` | 64 |
| `assets tickets --status <SUBSTR>` ambiguous | `"Ambiguous status: <candidates>"` | 64 |
| `assets schema <TYPE-SUBSTR>` ambiguous | `"Ambiguous type: <candidates>"` | 64 |
| `assets tickets --open` + `--status` together | clap error (mutual exclusion) | non-zero |

### Auth Commands

| Condition | Error | Exit Code |
|---|---|---|
| `auth remove <active-profile>` | `"cannot remove active profile"` | 64 |
| `auth refresh` with unconfigured profile + `--no-input` | `"no URL configured. Run: jr auth login --url <URL>"` | 64 |
| Invalid profile name | `"Invalid profile name: <name>"` | 64 |
| Config TOML parse failure | `"Failed to parse config: <toml error>"` | 78 |

### Config / Profile

| Condition | Error | Exit Code |
|---|---|---|
| Profile not found in config | `"Profile '<name>' not found. Run: jr auth login"` | 64 |
| `JR_PROFILE` set to nonexistent profile | same as above | 64 |
| Multi-profile fields bug (NFR-R-D, MUST-FIX) | After fix: error message must reference `[profiles.<name>]` not deprecated `[fields]` | 64 |

---

## Section 7: `send` vs `send_raw` Error Contract

Two HTTP dispatch paths with different error semantics:

| Path | Auth injection | Error on 4xx/5xx | 429 handling | Used by |
|---|---|---|---|---|
| `send(req)` | Yes — injects `Authorization` header on every retry | Raises `JrError` | Retries up to MAX_RETRIES=3 | `get`, `post`, `put`, `delete`, `post_no_content`, `get_from_instance`, `post_to_instance`, `get_assets`, `post_assets` |
| `send_raw(req)` | Via `request()` (caller calls `client.request()` to build, auth injected there) | Returns `reqwest::Response` to caller — no error | Retries up to MAX_RETRIES=3 THEN returns 429 response | `jr api` raw passthrough |

**Key invariant**: `send_raw` never raises `JrError` for 429 — the raw status code is returned to caller. This is intentional for the `jr api` passthrough command.
