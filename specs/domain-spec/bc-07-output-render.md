---
title: "BC-07: Output Rendering & Error Handling"
version: "1.0.0"
snapshot_sha: "dea166471e22eff55974d7675593469b37048c5f"
traces_to: "README.md"
source_passes: "Pass 2 broad §2a.2 Errors + §2a.3 Value objects + R1 §3.6 T-09 ADF + Pass 8 §2.2 BC#13,14"
entity_count: 16
invariant_count: 18
bc_count: 80
risk_level: HIGH
---

# BC-07: Output Rendering & Error Handling

Covers all output formatting (table and JSON), ADF rendering, the `JrError` type, and the runtime concerns of `main.rs`. The ADF renderer alone is 1,826 LOC and accounts for 51 BCs in the pass-3 contract index.

---

## §1 Ubiquitous Language

| Term | Definition |
|------|-----------|
| **ADF** | Atlassian Document Format. JSON-based structured text format used by Jira for issue descriptions, comment bodies, and worklog comments. |
| **`adf_to_text`** | Converts ADF JSON to plain terminal text. Lossy: mention/emoji/inlineCard/media nodes silently dropped. |
| **`markdown_to_adf`** | Converts CommonMark markdown to ADF. Used by `--markdown` flag on create/edit/comment. |
| **`text_to_adf`** | Converts plain text to ADF (minimal wrapping). Default for description/comment input. |
| **Table mode** | Default output format. Uses `comfy-table` with Unicode box-drawing. |
| **JSON mode** | `--output json`. Structured JSON to stdout. Error envelopes to stderr as `{"error":..., "code":...}`. |
| **stdout** | Machine output: table rows, JSON payloads. |
| **stderr** | Human status text: warnings, progress, verbose logs, error messages. |
| **`NULL_GLYPH`** | Em-dash `—` (U+2014). Used in table cells for missing/null values. |
| **Write-op JSON shape** | The JSON structure returned by state-changing commands. Varies by operation (see BC-03). |
| **`JrError`** | The crate's error enum. 11 variants with distinct exit codes. `Display` is user-facing. |
| **exit code** | `0` success, `1` generic error, `2` auth error, `64` user input error, `78` config error, `130` interrupted. |
| **`extract_error_message`** | 7-level precedence chain for extracting human-readable messages from Jira API responses: (1) empty body literal "Empty error response from Jira API"; (2) non-UTF-8 lossy decode; (3) `errorMessages[]`; (4) `errors{field:msg}`; (5) top-level `message`; (6) `errorMessage` (singular, JSM); (7) raw body text. |
| **ADF lossy nodes** | `mention`, `emoji`, `inlineCard`, `media` — dropped silently in `adf_to_text`. Documented at source as "per #202 spec". NFR-O-A (MEDIUM). |

---

## §2 Entities

| Entity | Module | Key Fields | Notes |
|--------|--------|-----------|-------|
| `JrError` (enum) | `error.rs:3-49` | 11 variants (see §3) | The crate-wide error type. All API, config, and user errors funnel here. |
| `OutputFormat` (enum) | `cli/mod.rs:47-51` | `Table`, `Json` | `derive(Clone, Copy, ValueEnum)`. Controls `--output` contract globally. |
| ADF node tree | `adf.rs` | JSON `Value` nodes + mark DSL | Not a typed struct; uses raw `serde_json::Value` with a DSL for emitting common shapes. |
| `format_issue_rows_public` | `cli/issue/format.rs` | Row-building for issue table | Handles conditional Points/Assets/Team columns. |
| `format_comment_date` | `cli/issue/format.rs` | ADF date parser for comment rows | Once-per-process parse failure logging via `observability::log_parse_failure_once`. |
| JSON write-op shapes | `cli/issue/json_output.rs` | `{key}` (create), `{key,status,transitioned}` (move), `{changed}` (assign), `{updated}` (edit), `{linked}`/`{unlinked}` (link) | 4 distinct boolean field names — anti-pattern P5R1-AP-05. |
| `OffsetPage<T>` | `api/pagination.rs` | `max_results`, `start_at`, `total`, `values: Vec<T>` | Used by most Jira endpoints. |
| `CursorPage<T>` | `api/pagination.rs` | `values: Vec<T>`, `nextPageToken: Option<String>` | Used by JQL search (`POST /search/jql`). |
| `ServiceDeskPage<T>` | `api/pagination.rs` | `_links`, `values: Vec<T>` | JSM-specific envelope. |
| `AssetsPage<T>` | `api/pagination.rs` | `values: Vec<T>`, `isLast: bool_or_string` | Custom serde deserializer tolerates `isLast: true` (bool) or `"true"` (string). |
| `JiraClient` | `api/client.rs` | `base_url`, `instance_url`, `auth_header`, HTTP methods | Single struct with 11 public HTTP methods. L3 HTTP-path bifurcation (validated vs raw). |
| Rate-limit state | `api/rate_limit.rs` + `api/client.rs` | `MAX_RETRIES = 3`, `DEFAULT_RETRY_SECS = 1` | Retry-After integer-only parser (HTTP-date format → falls back to `DEFAULT_RETRY_SECS`). |
| `observability` | `observability.rs:1-39` | `log_parse_failure_once(flag, site, iso, verbose)` | Single helper. Once-per-process gate via caller-supplied `&AtomicBool`. No tracing crate. |
| `HttpMethod` (enum) | `cli/api.rs` | `GET`, `POST`, `PUT`, `DELETE`, `PATCH` | Value-enum for `jr api -X METHOD`. |
| Error envelope (JSON) | `main.rs:34-49` | `{"error": ..., "code": ...}` | Emitted to stderr when `--output json` AND error occurs. Plain text `"Error: <msg>"` otherwise. |
| `Cli` struct + subcommand enums | `cli/mod.rs:18-738` | 14 top-level commands, sub-enums per command | Domain-bearing: each clap variant's flag set encodes a feature. |

---

## §3 JrError Variants

11 variants, confirmed by Pass 2 R1 (CONV-ABS counting).

| Variant | Exit code | Trigger |
|---------|----------:|---------|
| `NotAuthenticated` | 2 | 401 with no auth, or no stored credentials |
| `InsufficientScope { message }` | 2 | 401 + body containing `"scope does not match"` (case-insensitive). Display includes 5 required substrings (BC-1085). |
| `NetworkError(String)` | 1 | reqwest reachability failure (DNS, connect) |
| `ApiError { status, message }` | 1 | Any 4xx/5xx not specialised above; `message` from 7-level `extract_error_message` chain |
| `ConfigError(String)` | 78 | Missing config, unconfigured profile |
| `UserError(String)` | 64 | Bad CLI input: invalid profile name, ambiguous match, empty selection |
| `Internal(String)` | 1 | "Should never happen" violations; must be prefixed `"Internal error:"` by callers |
| `Interrupted` | 130 | Reserved; actual Ctrl+C exits via `process::exit(130)` in `main.rs:264` (does NOT construct this variant) |
| `Http(reqwest::Error)` | 1 | `#[from]` transparent |
| `Io(std::io::Error)` | 1 | `#[from]` transparent |
| `Json(serde_json::Error)` | 1 | `#[from]` transparent |

---

## §4 Operations

| Surface | Behavior |
|---------|---------|
| `output::print_output(format, rows, headers)` | Routes to `comfy-table` (Table) or `serde_json::to_string_pretty` (JSON). |
| `adf::adf_to_text(value)` | Recursive ADF → terminal text. Lossy: mention/emoji/inlineCard/media silently dropped. |
| `adf::markdown_to_adf(md)` | CommonMark → ADF via `pulldown-cmark`. |
| `adf::text_to_adf(text)` | Plain text → ADF (minimal paragraph wrapping). |
| `JiraClient::send(...)` | Validated path: issues HTTP, parses error, produces typed `JrError`. Includes rate-limit retry (max 3). |
| `JiraClient::send_raw(...)` | Passthrough: returns `reqwest::Response` unchanged. 4xx/5xx NOT converted to error. Used ONLY by `jr api`. |
| `api/rate_limit.rs` | Parses `Retry-After` as integer seconds only. HTTP-date format → silently falls back to `DEFAULT_RETRY_SECS = 1`. |
| `main.rs` error walker | Top-level `match err { JrError => exit_code }`. JSON mode: `{"error":msg,"code":N}` to stderr. Text mode: `"Error: msg"` to stderr. |

---

## §5 Business Rules & Invariants

| ID | Invariant | Source |
|----|----------|--------|
| INV-OUT-001 | Table = stdout. JSON (machine) = stdout. Human status text, warnings, verbose logs = stderr. Error envelopes = stderr (regardless of `--output`). | CLAUDE.md, Pass 2 §2b.4 |
| INV-OUT-002 | `--no-color` flag AND `NO_COLOR` env (IEEE Std NO_COLOR) both disable ANSI escape codes via `colored::control::set_override(false)`. | `main.rs:13-15` |
| INV-OUT-003 | `--no-input` is auto-set when `stdin` is not a TTY (pipe, script, AI agent, non-interactive). Guards all interactive prompts. | `main.rs:18-23` |
| INV-OUT-004 | `InsufficientScope` Display MUST contain all 5 substrings: `"Insufficient token scope"`, raw message, `"write:jira-work"`, `"OAuth 2.0"`, `github.com/Zious11/jira-cli/issues/185` link. | `error.rs`, BC-1085 |
| INV-OUT-005 | 401 + `"scope does not match"` (case-insensitive) → `InsufficientScope`. 401 NOT containing that substring → `NotAuthenticated`. 403 + "scope does not match" → NOT `InsufficientScope` (status gate). | BC-1085,1086,1087,1088 |
| INV-OUT-006 | `send_raw` delivers 429 responses to the CALLER (after MAX_RETRIES=3). After 4 consecutive 429s, the 4th response (still 429) is returned — NOT converted to `Err`. | BC-1092 |
| INV-OUT-007 | `send` (validated path) retries on 429 up to `MAX_RETRIES = 3` times with `Retry-After` sleep (or `DEFAULT_RETRY_SECS = 1` fallback). After exhaustion: always-stderr warning `"warning: rate limited by Jira — gave up after 3 retries."` | `api/client.rs:220-300` |
| INV-OUT-008 | `Retry-After` integer-only parser. HTTP-date format (RFC 7231 §7.1.3) silently falls back to `DEFAULT_RETRY_SECS`. NFR-R-NEW-1 (LOW). | `api/rate_limit.rs` |
| INV-OUT-009 | ADF mention/emoji/inlineCard/media nodes silently dropped in `adf_to_text`. Documented in source as "per #202 spec". NFR-O-A (MEDIUM). | `adf.rs:531-540` |
| INV-OUT-010 | ADF `orderedList` `attrs.order` falsy values default to 1 (treated as first-order list). | `adf.rs:407-416`, NEW-INV-15 |
| INV-OUT-011 | ADF `listItem` wraps children to satisfy ADF schema. Widened allowlist — spec + extra children tolerated. | `adf.rs:163-188`, NEW-INV-16 |
| INV-OUT-012 | ADF `tableCell` content always wrapped in a block. | `adf.rs:201-211`, NEW-INV-17 |
| INV-OUT-013 | ADF roundtrip (text→ADF→text or markdown→ADF→text) is lossy in both directions. No round-trip fidelity guarantee. | NEW-INV-14 |
| INV-OUT-014 | `extract_error_message` 7-level precedence: (1) empty body literal; (2) non-UTF-8 lossy; (3) `errorMessages[]`; (4) `errors{field:msg}`; (5) top-level `message`; (6) `errorMessage` (singular, JSM); (7) raw body text. | `api/client.rs`, Pass 1 §3 |
| INV-OUT-015 | `--verbose` logs request METHOD + URL to stderr. Authorization header is NOT logged. Request body IS logged (full, via `String::from_utf8_lossy`). Body PII not redacted (NFR-S-C MEDIUM). | `api/client.rs:197-278` |
| INV-OUT-016 | `observability::log_parse_failure_once` fires at most one line per call-site per process (via caller-supplied `&AtomicBool` gate). Not verbose-gated — always fires when triggered. | `observability.rs:1-39` |
| INV-OUT-017 | Cache-corruption warning is ALWAYS emitted to stderr (not verbose-gated): `"warning: cache file <name> unreadable (<err>); will refetch"`. | `cache.rs:26,128,159` |
| INV-OUT-018 | Config-migration notice always emitted: `"Migrated config to multi-profile layout ..."`. | `config.rs:260-263` |

---

## §6 Aggregate Boundaries

- **`JrError`** is the single error type aggregating all domain errors. Exit codes are the public contract (`exit_code()` method).
- **`JiraClient`** is the single HTTP-layer aggregate. The L3 bifurcation (validated vs raw) is an internal design choice, not a public boundary.
- **ADF renderer** (`adf.rs`) is a self-contained subdomain (1,826 LOC, 51 BCs). No external dependencies beyond `pulldown-cmark` and `serde_json`.

---

## §7 Cross-Context Dependencies

| Depends on | Reason |
|-----------|--------|
| **All contexts** | Every context produces `JrError` and consumes `OutputFormat`. |
| **Cross-cutting** | Pagination shapes live in `api/pagination.rs` (cross-cutting). |

---

## Harmonization Notes

- Auth subcommands (login/switch/logout/remove/refresh) lack `--output json` support (5 commands). Gap noted in Pass 5 R2-T2.
- `observability.rs` (39 LOC) is the project's deliberate stance against a logging framework: one function, `--verbose`-gated `eprintln!` is the established pattern.
