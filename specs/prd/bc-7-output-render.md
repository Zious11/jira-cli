---
context: bc-7
title: "Output Rendering & Error"
total_bcs: 80   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 34   # count of `#### BC-` headings in this file
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/bc-07-output-render.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.12-2.13
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.5-3.6
---

# BC-7 — Output Rendering & Error

80 behavioral contracts across 5 subdomains: Table/JSON output (7.1), ADF rendering (7.2),
Error display (7.3), JSON output shapes (7.4), Observability (7.5).

---

## Subdomains

### 7.1 Table / JSON Output

#### BC-7.1.001: `--output table` uses comfy-table renderer; `--output json` emits structured JSON

**Confidence**: HIGH
**Source**: `src/output.rs::tests`; integration tests using `--output json`
**Subject**: Output rendering
**Behavior**: Default is `table`. Integration test pattern: assert `serde_json::from_str(&stdout)` parses. Table uses comfy-table crate.
**Trace**: Pass 3 BC-1101

---

#### BC-7.1.002: `--no-color` and `NO_COLOR` env disable ANSI escape sequences

**Confidence**: HIGH
**Source**: `src/main.rs:13-15`; colored crate
**Trace**: Pass 3 BC-1102

---

#### BC-7.1.003: `--no-input` auto-enables when stdin is not a TTY (`IsTerminal` check)

**Confidence**: HIGH
**Source**: `src/main.rs:18-23`
**Subject**: Output rendering
**Behavior**: Auto-set on pipes / AI agents / scripts. Every command must have fully non-interactive flag equivalents.
**Trace**: Pass 3 BC-1103

---

#### BC-7.1.004: Truncation hint emitted to stderr (NOT stdout); `--all` suppresses hint

**Confidence**: HIGH
**Source**: `tests/sprint_commands.rs:97-100, 175-179`
**Subject**: Output rendering
**Behavior**: stderr line: `"Showing N results"`. With `--all`: NO hint. Used by issue list, sprint current, board view.
**Trace**: Pass 3 BC-1110, BC-1111

---

#### BC-7.1.005: `--output json` error shape: `{"error": "<message>", "code": <exit>}` to stderr

**Confidence**: MEDIUM
**Source**: `src/main.rs:34-49`
**Subject**: Output rendering
**Behavior**: When `--output json` is active AND an error occurs, stderr gets JSON error envelope.
**Trace**: Pass 3 BC-1208 (error handling context)

---

### 7.2 ADF Rendering (51 contracts)

#### BC-7.2.001: `text_to_adf("hello")` emits `{type:"doc", version:1, content:[{type:"paragraph", content:[{type:"text", text:"hello"}]}]}`

**Confidence**: HIGH
**Source**: `src/adf.rs::tests` (69 unit tests)
**Subject**: Output rendering
**Behavior**: Standard ADF doc shape. Version is always 1.
**Trace**: Pass 3 BC-1104

---

#### BC-7.2.002: `markdown_to_adf("**bold**")` emits marks `[{type:"strong"}]` on the text node

**Confidence**: HIGH
**Source**: `src/adf.rs::tests`
**Trace**: Pass 3 BC-1105

---

#### BC-7.2.003: ADF markdown round-trip covers: headings, lists, code blocks, blockquotes, tables, links

**Confidence**: HIGH
**Source**: `src/snapshots/jr__adf__tests__markdown_complex_to_adf.snap` (330-line snapshot)
**Subject**: Output rendering
**Behavior**: Canonical complex doc → ADF snapshot. Round-trip canary; specific bytes pinned.
**Trace**: Pass 3 BC-1117 (R4)

---

#### BC-7.2.004: ADF→text rendering: table render, code, headings preserved; lossy nodes (mention/emoji/inlineCard/media) silently dropped

**Confidence**: HIGH
**Source**: `src/adf.rs::tests`; `src/snapshots/jr__adf__tests__adf_to_text_complex.snap` (18-line snapshot)
**Subject**: Output rendering
**Behavior**: `_` fall-through arm at `adf.rs:531-540` silently drops unsupported nodes (documented per #202 spec). NFR-O-A (MEDIUM): ADF lossy nodes in text mode.
**Trace**: Pass 3 BC-1106; BC-1116 (R4)

---

#### BC-7.2.005: `markdown_to_adf("**bold text**")` body on wire: `marks: [{type: "strong"}]`; `text` is `"bold text"` NOT `"**bold text**"`

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:647-687`
**Behavior**: Wire-level pin; markdown fully converted before HTTP.
**Trace**: Pass 3 BC-1056 (R4)

---

### 7.3 Error Display

#### BC-7.3.001: `extract_error_message` 7-step precedence chain (canonical from source)

**Confidence**: HIGH
**Source**: `src/api/client.rs:448-490`; `tests/api_client.rs:257-342`
**Subject**: Output rendering
**Behavior**: Precedence (first match wins, returning `String`):
1. Empty body (len == 0) → literal string `"<empty response body>"` (early return before UTF-8 check)
2. Non-UTF-8 bytes → `String::from_utf8_lossy` with replacement chars (early return)
3. `errorMessages[]` non-empty (JSON array with at least one string element) → elements joined with `"; "`
4. `errors{}` non-empty (JSON object) → `"field: value"` pairs, alphabetically sorted, joined with `"; "`; non-string values use `serde_json::Value` display
5. `message` string field → as-is
6. `errorMessage` string field (singular; seen in JSM endpoints) → as-is
7. Raw body string fallback (non-JSON or no recognized field matches)

**Key invariant**: Empty body check is step 1 — the literal `"<empty response body>"` string IS the return value. There is no None/caller-derives path; the string propagates into `ApiError { message }`.

Note: The function doc comment inside client.rs lists precedence as "1. errorMessages … 5. Empty body … 6. Raw body" — this comment is STALE and does NOT reflect code execution order. Steps 1–2 are early returns before JSON parsing begins. Source code is authoritative; doc comment will be fixed in Phase 3. Corrected by R1 CONV-ABS-004; further corrected by ADV-P2-001.
**Trace**: Pass 3 BC-1201-R (R1); ADV-P2-001

---

#### BC-7.3.002: `errors{}` string values: `field: <value>`; non-string: `field: <serde_json::Value debug>`

**Confidence**: HIGH
**Source**: `src/api/client.rs:469-475`; `tests/api_client.rs:303-307`
**Behavior**: Mixed types: `{summary: "is req", customfield_10001: {messages:["invalid"]}}` → `customfield_10001: {"messages":["invalid"]}`.
**Trace**: Pass 3 BC-1201a (R1)

---

#### BC-7.3.003: `errors{}` iteration is alphabetically sorted (deterministic)

**Confidence**: HIGH
**Source**: `src/api/client.rs:477`; `tests/api_client.rs:286-292`
**Behavior**: `pairs.sort()` before join. `{summary: "req", priority: "req"}` → `priority: req; summary: req` (priority first).
**Trace**: Pass 3 BC-1201b (R1)

---

#### BC-7.3.004: Empty `errorMessages[]` and empty `errors{}` fall through to raw body (no early exit)

**Confidence**: HIGH
**Source**: `src/api/client.rs:459-466`; `tests/api_client.rs:294-300`
**Trace**: Pass 3 BC-1201c (R1)

---

#### BC-7.3.005: `--output json` + empty 4xx body → stderr JSON `{"error": "<empty response body>", "code": <exit>}`

**Confidence**: HIGH
**Source**: `src/main.rs:34-49`; `src/api/client.rs:448-490`
**Subject**: Output rendering
**Behavior**: When `--output json` is active AND the response has a zero-length body (4xx), `extract_error_message` returns the literal string `"<empty response body>"` (step 1 of BC-7.3.001). This string propagates into `JrError::ApiError { message }` and then into the JSON error envelope: `{"error": "<empty response body>", "code": <exit-code>}` to stderr. `code` is the integer exit code matching `JrError::exit_code()`. There is no status-code-derived substitution; the literal string IS the message.
**Edge case**: If body is `{}` (empty JSON object, NOT zero-length bytes), `extract_error_message` falls to step 7 (raw body `{}`), not the empty-body path. The `"<empty response body>"` literal only appears when `body.is_empty()` is true (byte length == 0).
**Trace**: Pass 3 BC-1208; BC-7.3.001 (extract_error_message); ADV-P1-026; ADV-P2-001

---

#### BC-7.3.006: `JrError::exit_code()` mapping

**Confidence**: HIGH
**Source**: `src/error.rs:51-62`; 8 inline tests
**Subject**: Output rendering
**Behavior**: See error-taxonomy.md for full table. Key codes: NotAuthenticated=2, InsufficientScope=2, ConfigError=78, UserError=64, Interrupted=130, NetworkError=1, ApiError=1, Json=1, Http=1, Other=1, Success=0.
**Trace**: Pass 3 BC-1204

---

#### BC-7.3.007: All API errors must suggest a next step (CLAUDE.md convention)

**Confidence**: HIGH
**Source**: `tests/issue_list_errors.rs`, `tests/issue_resolution.rs`, `tests/auth_refresh.rs`, `tests/issue_view_errors.rs`
**Subject**: Output rendering
**Behavior**: At least one of: `jr auth login`, `--jql`, `--resolution`, `jr issue resolutions`, `jr team list --refresh`, `board_id`, `check your connection`, `jr init` must appear in stderr.
**Trace**: Pass 3 BC-1212

---

#### BC-7.3.008: stderr must NEVER contain `panic`

**Confidence**: HIGH
**Source**: 16+ tests across `tests/*_errors.rs` files asserting `!stderr.contains("panic")`
**Subject**: Output rendering
**Behavior**: Universal constraint. All error paths produce friendly messages.
**Trace**: Pass 3 BC-1205

---

#### BC-7.3.009: Internal errors prefix with `Internal error:`

**Confidence**: MEDIUM
**Source**: `src/error.rs:30-36`
**Trace**: Pass 3 BC-1213

---

### 7.4 JSON Output Shapes (insta snapshot contracts)

All snapshots from `src/cli/issue/snapshots/` and `src/cli/snapshots/`. Keys are sorted alphabetically in insta output.

#### BC-7.4.001: move changed → `{"changed": true, "key": "TEST-1", "status": "In Progress"}`
**Source**: `jr__cli__issue__json_output__tests__move_response_changed.snap`
**Trace**: Pass 3 BC-1104 (R4)

#### BC-7.4.002: move unchanged → `{"changed": false, "key": "TEST-1", "status": "Done"}`
**Source**: `jr__cli__issue__json_output__tests__move_response_unchanged.snap`
**Trace**: Pass 3 BC-1105 (R4)

#### BC-7.4.003: assign changed → `{"assignee": "Jane Doe", "assignee_account_id": "abc123", "changed": true, "key": "TEST-1"}` — `assignee_account_id` is snake_case (NOT camelCase)
**Source**: `jr__cli__issue__json_output__tests__assign_changed.snap`
**Trace**: Pass 3 BC-1106 (R4)

#### BC-7.4.004: unassign → `{"assignee": null, "changed": true, "key": "TEST-1"}` — `assignee` is EXPLICIT null (NOT omitted)
**Source**: `jr__cli__issue__json_output__tests__unassign.snap`
**Trace**: Pass 3 BC-1108 (R4)

#### BC-7.4.005: edit → `{"key": "TEST-1", "updated": true}` — minimal 2-key shape
**Source**: `jr__cli__issue__json_output__tests__edit.snap`
**Trace**: Pass 3 BC-1109 (R4)

#### BC-7.4.006: link → `{"key1": "TEST-1", "key2": "TEST-2", "linked": true, "type": "Blocks"}` — symmetric key1/key2
**Source**: `jr__cli__issue__json_output__tests__link.snap`
**Trace**: Pass 3 BC-1110 (R4)

#### BC-7.4.007: unlink → `{"count": 2, "unlinked": true}`; no-match → `{"count": 0, "unlinked": false}` (count: 0 NOT omitted)
**Source**: `jr__cli__issue__json_output__tests__unlink_success.snap`
**Trace**: Pass 3 BC-1111 (R4)

#### BC-7.4.008: remote-link → `{"id": 10000, "key": "TEST-1", "self": <url>, "title": <title>, "url": <url>}` — id is u64
**Source**: `jr__cli__issue__json_output__tests__remote_link.snap`
**Trace**: Pass 3 BC-1112 (R4)

#### BC-7.4.009: sprint add → `{"added": true, "issues": [...], "sprint_id": 100}` — sprint_id snake_case
**Source**: `jr__cli__sprint__tests__sprint_add_response.snap`
**Trace**: Pass 3 BC-1113 (R4)

#### BC-7.4.010: sprint remove → `{"issues": [...], "removed": true}` — NO sprint_id (remove is sprint-agnostic)
**Source**: `jr__cli__sprint__tests__sprint_remove_response.snap`
**Trace**: Pass 3 BC-1114 (R4)

#### BC-7.4.011: auth list table → 4 cols: NAME, URL, AUTH, STATUS; active prefix `* ` (asterisk-space); inactive `  ` (2 spaces)
**Source**: `jr__cli__auth__tests__list_table_snapshot.snap`
**Trace**: Pass 3 BC-1115 (R4)

#### BC-7.4.012: `user view` hidden email → table shows em-dash `—`; JSON output shows explicit `null` (privacy boundary)
**Source**: `tests/user_commands.rs` BC-1132j/k
**Trace**: Pass 3 BC-1132j, BC-1132k (R4)

### 7.5 Observability

#### BC-7.5.001: Verbose request logging emits `[verbose] METHOD URL` + `[verbose] body: <utf8>` (when body present)

**Confidence**: HIGH
**Source**: `src/api/client.rs:197-204, 274-279`
**Subject**: Output rendering
**Behavior**: Two lines per request. Body is utf-8 lossy. Retry logging: `[verbose] Rate limited (429). Retrying in {delay}s (attempt N/M)`. Authorization header NOT logged (NFR-S-C flag — body IS logged, auth NOT).
**Trace**: Pass 3 BC-1405; BC-1405-R (R1)

---

#### BC-7.5.002: `log_parse_failure_once` gate — parse failure logged at most once per (process, key)

**Confidence**: MEDIUM
**Source**: `src/observability.rs::tests`
**Trace**: Pass 3 BC-1109 (format.rs context)

---

#### BC-7.5.003: `format_duration(seconds)` collapses to `30m` / `2h` / `1h30m` (hours+minutes only; never weeks/days)

**Confidence**: HIGH
**Source**: `src/duration.rs:52-60`
**Trace**: Pass 3 BC-1107

---

## Key Invariants

- stdout = data; stderr = errors, warnings, hints (universal discipline)
- ADF lossy for mention/emoji/inlineCard/media — documented, not a bug
- JSON output uses snake_case for jr-internal fields (NOT Atlassian camelCase)
- Insta snapshots pin exact bytes — any glyph or key change breaks snapshot test
- `extract_error_message` empty-body check is FIRST (not last)
