---
context: bc-x
title: "Cross-cutting (HTTP client, Runtime, Users, Teams, Worklogs, Projects, Queues, JQL, Partial-match)"
total_bcs: 130   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 64   # count of `#### BC-` headings in this file (corrected at ADV-P7-003)
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/cross-cutting.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.6-2.15
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §3.6-3.8
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.2-3.4
---

# BC-X — Cross-cutting

130 behavioral contracts covering: HTTP client (X.1), Pagination (X.2), Error handling (X.3),
Rate limiting (X.4), Worklogs & duration (X.5), Teams (X.6), Users (X.7), Projects & Queues (X.8),
JQL utilities (X.9), Partial-match (X.10), Build-time (X.11).

---

## Subdomains

### X.1 HTTP Client (JiraClient)

#### BC-X.1.001: Auth header injected on every API call via `req.header("Authorization", &self.auth_header)` at line 195

**Confidence**: HIGH
**Source**: `tests/api_client.rs:14-40`; `src/api/client.rs:195`
**Subject**: HTTP client
**Behavior**: Header value is verbatim auth string (e.g., `Basic dGVzdEBleGFtcGxlLmNvbTpteS1hcGktdG9rZW4=`). Pinned by wiremock `header(...)` matcher. Injected on every retry attempt including the first.
**Trace**: Pass 3 BC-1410-R (R1); BC-1082 (R4)

---

#### BC-X.1.002: `client.send(request)` retries 429 transparently; returns parsed response on 200

**Confidence**: HIGH
**Source**: `tests/api_client.rs:42-70`
**Behavior**: 429-then-200 → caller sees 200 (typed T). Retry is transparent.
**Trace**: Pass 3 BC-1402; BC-1083 (R4)

---

#### BC-X.1.003: `client.send(request)` on exhausted 429 raises `JrError::ApiError{status: 429}` via `parse_error`

**Confidence**: HIGH
**Source**: `src/api/client.rs:184-253`
**Behavior**: After MAX_RETRIES=3 (4 total calls), the last 429 response is parsed via `parse_error` → `JrError::ApiError`. Distinct from `send_raw` behavior (which returns 429, not raises).
**Trace**: Pass 3 BC-1402-R (R1)

---

#### BC-X.1.004: `client.send(request)` requires `RequestBuilder::try_clone()` to succeed; non-cloneable bodies panic

**Confidence**: HIGH
**Source**: `src/api/client.rs:191-194`
**Behavior**: `request.try_clone().expect("request should be cloneable (JSON body)")`. Streaming-body refactor would panic.
**Trace**: Pass 3 BC-1402a (R1)

---

#### BC-X.1.005: `client.send_raw(request)` returns 429 to caller (NOT raises) after MAX_RETRIES=3; `expect(4)` pin

**Confidence**: HIGH
**Source**: `tests/api_client.rs:424-444`
**Subject**: HTTP client
**Behavior**: 4 total calls (initial + 3 retries). FINAL response IS 429. `send_raw` returns it, not raises.
**Trace**: Pass 3 BC-1401; BC-1092 (R4)

---

#### BC-X.1.006: `send_raw` 429-then-200 retries identically to `send`; caller sees 200

**Confidence**: HIGH
**Source**: `tests/api_client.rs:394-422`
**Trace**: Pass 3 BC-1091 (R4)

---

#### BC-X.1.007: `send_raw` preserves 404 as response (NOT converted to Err); used by `jr api` raw passthrough

**Confidence**: HIGH
**Source**: `tests/api_client.rs:367-392`
**Subject**: HTTP client
**Behavior**: 404 response returned to caller with body intact. Error-conversion happens in `get`/`post`/etc., NOT `send_raw`.
**Trace**: Pass 3 BC-1409-R (R1); BC-1090 (R4)

---

#### BC-X.1.008: `send_raw` non-cloneable body returns `anyhow::Error` with explicit message (NOT panic)

**Confidence**: HIGH
**Source**: `src/api/client.rs:267-272`
**Behavior**: `req.try_clone().ok_or_else(|| anyhow::anyhow!("request cannot be retried..."))`. More defensive than `send`.
**Trace**: Pass 3 BC-1402b (R1)

---

#### BC-X.1.009: 429-exhausted warning always emitted to stderr (not verbose-gated)

**Confidence**: HIGH
**Source**: `src/api/client.rs:233-237, 309-313`
**Behavior**: `"warning: rate limited by Jira — gave up after 3 retries. Wait a moment and try again."` — unconditional. Same from both `send` and `send_raw`.
**Trace**: Pass 3 BC-1404; BC-1404-R (R1)

---

#### BC-X.1.010: All HTTP methods (get, post, put, delete, send_raw) inject auth header — no bypass

**Confidence**: HIGH
**Source**: `src/api/client.rs` (R4 §4.1 verification)
**Behavior**: 9 high-level methods use `self.send(request)` (auth at line 195). 2 raw methods use `self.client.execute(req)` after `self.request()` injects header. No method bypasses.
**Trace**: Pass 4 R4 §4.1

---

### X.2 Pagination

#### BC-X.2.001: Offset pagination: `startAt`/`maxResults` + `total` for issue comments, projects, worklogs

**Confidence**: HIGH
**Source**: `src/api/pagination.rs`; 14 unit tests; `tests/comments.rs:104-158`
**Trace**: Pass 3 BC-1406, BC-1407-R (R1)

---

#### BC-X.2.002: Cursor pagination via `nextPageToken` for JQL search

**Confidence**: HIGH
**Source**: `src/api/pagination.rs::CursorPage`; `tests/issue_commands.rs`
**Trace**: Pass 3 BC-1406

---

#### BC-X.2.003: ServiceDeskPage pagination (JSM service desks)

**Confidence**: HIGH
**Source**: `src/api/pagination.rs::ServiceDeskPage`
**Trace**: Pass 3 BC-1406

---

#### BC-X.2.004: `AssetsPage::is_last` accepts bool or string-encoded bool (custom deserializer)

**Confidence**: HIGH
**Source**: `src/api/pagination.rs::AssetsPage`
**Trace**: Pass 3 BC-317 (R1)

---

#### BC-X.2.005: User pagination advances `startAt` by REQUESTED `maxResults` (NOT by returned count)

**Confidence**: HIGH
**Source**: `tests/user_pagination.rs:202-247`; `tests/all_flag_behavior.rs:155-208`
**Subject**: Pagination
**Behavior**: Page 1 returns 35 users; page 2 startAt=100 (advanced by requested 100, NOT by 35). This is a deliberate workaround for JRACLOUD-71293.
**Trace**: Pass 3 BC-702; BC-1119 (R4)

---

#### BC-X.2.006: `USER_PAGINATION_SAFETY_CAP = 1500` (15 pages × 100); emits stderr `"hit pagination safety cap"`; exits 0

**Confidence**: HIGH
**Source**: `tests/user_pagination.rs:459-520`
**Behavior**: Safety cap prevents infinite loops. Warning is observable; exit 0.
**Trace**: Pass 3 BC-1124, BC-1125 (R4)

---

### X.3 Error Handling (universal rules)

#### BC-X.3.001: Network drop → `Could not reach <host>; check your connection` exit 1

**Confidence**: HIGH
**Source**: `tests/issue_list_errors.rs:320-360`; `tests/issue_view_errors.rs:102-134`; `tests/assets_errors.rs:115-153`
**Behavior**: Connect-refused (port 1) → `JrError::NetworkError(host)`.
**Trace**: Pass 3 BC-1206

---

#### BC-X.3.002: 401 → `Not authenticated` + `jr auth login` exit 2 (universal across all subcommands)

**Confidence**: HIGH
**Source**: 6+ test files; `tests/issue_list_errors.rs`, `tests/issue_view_errors.rs`, `tests/comments.rs`, `tests/worklog_commands.rs`, `tests/team_commands.rs`, `tests/assets_errors.rs`
**Trace**: Pass 3 BC-1207

---

#### BC-X.3.003: 5xx → `API error (<status>)` + extract_error_message(body) + exit 1

**Confidence**: HIGH
**Source**: All `*_errors.rs` files; assert `stderr.contains("API error (500)")`
**Trace**: Pass 3 BC-1210

---

#### BC-X.3.004: 400 with field-specific Jira error → stderr formatted as `field: message` (sorted alphabetically)

**Confidence**: HIGH
**Source**: `tests/issue_resolution.rs:124-158`
**Trace**: Pass 3 BC-1211

---

#### BC-X.3.005: 401 + scope-mismatch (case-insensitive) → InsufficientScope with 5 substrings; 403 with substring NOT dispatched

**Confidence**: HIGH
**Source**: `tests/api_client.rs:99-255`
**Trace**: Pass 3 BC-015..018; BC-1085..1088 (R4)

---

#### BC-X.3.006: Ctrl+C exits 130 with `Interrupted` handling

**Confidence**: MEDIUM
**Source**: `src/main.rs:264`
**Trace**: Pass 3 BC-1209

---

#### BC-X.3.007: Error messages must suggest next step (CLAUDE.md convention, universal)

**Confidence**: HIGH
**Source**: Multiple integration tests asserting remediation strings
**Trace**: Pass 3 BC-1212

---

#### BC-X.3.008: stderr must NEVER contain `panic` (universal)

**Confidence**: HIGH
**Source**: 16+ negative assertion tests
**Trace**: Pass 3 BC-1205

---

### X.4 Rate Limiting

#### BC-X.4.001: MAX_RETRIES = 3 (initial + 3 = 4 total calls); `expect(4)` pin

**Confidence**: HIGH
**Source**: `tests/api_client.rs:424-444`; `src/api/client.rs:265-320`
**Trace**: Pass 3 BC-1401-R (R1)

---

#### BC-X.4.002: `Retry-After` header parsed as u64 INTEGER ONLY — HTTP-date format NOT supported

**Confidence**: HIGH
**Source**: `src/api/rate_limit.rs:14-18`; 2 unit tests
**Subject**: Rate limiting
**Behavior**: `header.parse::<u64>()`. HTTP-date format → `None` → falls back to `DEFAULT_RETRY_SECS = 1`. No upper bound — `Retry-After: 86400` is honored as 24h (NFR-R-NEW-1, LOW). CONV-ABS-001 correction.
**Trace**: Pass 3 BC-1403-R (R1)

---

 > [BC-X.4.003..008 are range-collapsed in BC-INDEX.md; not individually bodied]

#### BC-X.4.009: `MAX_RETRY_AFTER_SECS = 60` cap — Retry-After exceeding 60s prints warning and aborts retry

**Confidence**: HIGH (PROPOSED — FIX-IN-PHASE-3)
**Source**: `src/api/rate_limit.rs` (proposed addition)
**Subject**: Rate limiting
**Behavior**: When `Retry-After` header value is a valid u64 AND exceeds `MAX_RETRY_AFTER_SECS = 60`: (1) print to stderr `"warning: Retry-After <NNN>s exceeds 60s; aborting retry, run jr again later"` and (2) exit non-zero (the retry loop does NOT sleep and retry; it returns the 429 response). Values ≤ 60s continue to be honored as before.
**Related**: NFR-R-NEW-1 (cross-link); H-027 (holdout that pins current no-upper-bound behavior — will need updating when this fix lands).
**Note**: This BC describes the PROPOSED fixed behavior, not current behavior. Currently BC-X.4.002 documents no upper bound. This BC is the Phase 3 target state. H-027 documents the current gap.
**Trace**: ADV-P1-029; NFR-R-NEW-1

---

### X.5 Worklogs & Duration

#### BC-X.5.001: `client.add_worklog(key, seconds, message)` POSTs `/issue/<key>/worklog`; returns Worklog; accepts 201

**Confidence**: HIGH
**Source**: `tests/worklog_commands.rs:8-26`
**Trace**: Pass 3 BC-501

---

#### BC-X.5.002: `client.list_worklogs(key)` paginates via `/issue/<key>/worklog` [MUST-FIX: NFR-R-A — HIGH]

**Confidence**: HIGH
**Source**: `src/api/jira/worklogs.rs:25-30` (BUG SITE)

> **MUST-FIX (HIGH — NFR-R-A):** Current code fetches ONE `OffsetPage<Worklog>` and discards
> `total`/`start_at`/`max_results`. Issues with >50 worklogs silently truncate. This contract
> describes the FIXED behavior.

**Spec contract (fixed behavior):**
`list_worklogs` MUST paginate in a loop until `page.total <= page.start_at + page.items().len()`. All pages concatenated and returned to caller. No silent truncation.

**Holdout:** H-045 — `list_worklogs` pagination — all pages returned.
**Trace**: Pass 3 BC-502; NFR-R-A; Pass 4 R4 §1.1

---

#### BC-X.5.003: `worklog list` 5xx → exit 1 + `API error (500)`

**Confidence**: HIGH
**Source**: `tests/worklog_commands.rs:55-93`
**Trace**: Pass 3 BC-503

---

#### BC-X.5.004: `worklog list` 401 → exit 2 + `Not authenticated` + `jr auth login`

**Confidence**: HIGH
**Source**: `tests/worklog_commands.rs:95-120`
**Trace**: Pass 3 BC-504

---

#### BC-X.5.005: `parse_duration("1w2d3h30m", 8, 5)` accepts combined units; returns total seconds

**Confidence**: HIGH
**Source**: `src/duration.rs::tests::test_complex`
**Subject**: Duration
**Behavior**: Distinguished from JQL `validate_duration` which rejects combined units. Used for worklog add.
**Trace**: Pass 3 BC-505

---

#### BC-X.5.006: `parse_duration` is case-insensitive (input lowercased first)

**Confidence**: HIGH
**Source**: `src/duration.rs:6`
**Trace**: Pass 3 BC-506

---

#### BC-X.5.007: `parse_duration("")` errors `Duration cannot be empty`

**Confidence**: HIGH
**Source**: `src/duration.rs:7-9`
**Trace**: Pass 3 BC-507

---

#### BC-X.5.008: `parse_duration("5")` errors `Number without unit`

**Confidence**: HIGH
**Source**: `src/duration.rs:38-42`
**Trace**: Pass 3 BC-508

---

#### BC-X.5.009: `worklog add` hardcodes 8h/day, 5d/week (`parse_duration(dur, 8, 5)` at `cli/worklog.rs:32`)

**Confidence**: HIGH
**Source**: `src/cli/worklog.rs:32`
**Subject**: Duration
**Behavior**: NFR-R-C (MEDIUM): ignores Jira instance time-tracking settings. DOCUMENT-AS-IS for v1.
**Trace**: Pass 3 BC-1014 (R4)

---

#### BC-X.5.010: Duration proptest: `valid_single_units_always_parse`; `combined_units_always_parse`; `garbage_input_never_panics`; `format_roundtrip` (sub-day)

**Confidence**: HIGH
**Source**: `src/duration.rs:128-157`
**Trace**: Pass 3 BC-1099..BC-1102 (R4)

---

### X.6 Teams

#### BC-X.6.001: `client.get_org_metadata(hostname)` POSTs GraphQL `tenantContexts` query to `/gateway/api/graphql`

**Confidence**: HIGH
**Source**: `tests/team_commands.rs:8-26`
**Subject**: Teams
**Behavior**: Returns `TenantContext { org_id, cloud_id }` (ADR-0005).
**Trace**: Pass 3 BC-601

---

#### BC-X.6.002: `client.list_teams(orgId)` GETs `/gateway/api/public/teams/v1/org/<orgId>/teams`

**Confidence**: HIGH
**Source**: `tests/team_commands.rs:28-46`
**Trace**: Pass 3 BC-602

---

#### BC-X.6.003: `team list` 5xx → exit 1; 401 → exit 2; standard error paths

**Confidence**: HIGH
**Source**: `tests/team_commands.rs:62-`
**Trace**: Pass 3 BC-603, BC-604

---

#### BC-X.6.004: `team list` cache-first (7d TTL); `--refresh` forces re-fetch

**Confidence**: MEDIUM
**Source**: `src/cache.rs`
**Trace**: Pass 3 BC-605

---

### X.7 Users

#### BC-X.7.001: `user search Q` GETs `/rest/api/3/user/search?query=Q`

**Confidence**: HIGH
**Source**: `tests/user_commands.rs`; `tests/all_flag_behavior.rs:155-208`
**Trace**: Pass 3 BC-701

---

#### BC-X.7.002: `user list --project P` calls `/rest/api/3/user/assignable/multiProjectSearch?projectKeys=P`

**Confidence**: HIGH
**Source**: `tests/all_flag_behavior.rs:260-`
**Trace**: Pass 3 BC-704

---

#### BC-X.7.003: `user list` (default, no --all) uses single-call legacy path; no startAt/maxResults params

**Confidence**: HIGH
**Source**: `tests/all_flag_behavior.rs:271-275`
**Behavior**: `query_param_is_missing("startAt")` assertion.
**Trace**: Pass 3 BC-705

---

#### BC-X.7.004: Duplicate display names + `--no-input` → exit non-zero; stderr shows emails + accountIds + duplicate name

**Confidence**: HIGH
**Source**: `tests/duplicate_user_disambiguation.rs:21-275`
**Subject**: Users
**Behavior**: Three users "John Smith" x2 + "John Smithson" → disambiguation shows only the two Smiths (not Smithson).
**Trace**: Pass 3 BC-706..BC-708

---

#### BC-X.7.005: `user view <id>` → 404 → friendly `"User with accountId '<id>' not found"` exit 64

**Confidence**: HIGH
**Source**: `tests/user_commands.rs` BC-1132i
**Trace**: Pass 3 BC-1132i (R4)

---

#### BC-X.7.006: `user search --all` advances startAt by REQUESTED maxResults (JRACLOUD-71293 workaround)

**Confidence**: HIGH
**Source**: `tests/user_pagination.rs:202-247`
**Trace**: Pass 3 BC-1119 (R4)

---

### X.8 Projects & Queues

#### BC-X.8.001: `project_exists(key)` → true on 200; false on 404

**Confidence**: HIGH
**Source**: `tests/input_validation.rs:9-42`
**Trace**: Pass 3 BC-801

---

#### BC-X.8.002: `get_project_statuses(key)` → 404 → `JrError::ApiError{status: 404}`

**Confidence**: HIGH
**Source**: `tests/input_validation.rs:233-253`
**Trace**: Pass 3 BC-802

---

#### BC-X.8.003: `get_or_fetch_project_meta(client, key)` caches by project key with 7d TTL

**Confidence**: HIGH
**Source**: `tests/project_meta.rs:24-67`
**Behavior**: Service-desk project → `service_desk_id = Some("15")`. Software project → `None`.
**Trace**: Pass 3 BC-804

---

#### BC-X.8.004: `require_service_desk` errors for software project: "Jira Software project" + "Queue commands require"

**Confidence**: HIGH
**Source**: `tests/project_meta.rs:99-126`
**Trace**: Pass 3 BC-805

---

#### BC-X.8.005: `list_projects` paginates via `startAt`; filter via `typeKey` query param

**Confidence**: HIGH
**Source**: `tests/project_commands.rs:1-323`
**Trace**: Pass 3 BC-1133d, BC-1133e (R4)

---

### X.9 JQL Utilities

#### BC-X.9.001: `escape_value` proptest: for any printable Unicode up to 100 chars, output has NO unescaped quote

**Confidence**: HIGH
**Source**: `src/jql.rs:383-394`; `proptest-regressions/jql.txt` (seed: `s = ""`)
**Subject**: JQL
**Behavior**: `has_unescaped_quote` helper tracks backslash-runs. Regression corpus pinned.
**Trace**: Pass 3 BC-1094 (R4)

---

#### BC-X.9.002: `validate_duration("4w2d")` → Err; single unit `"7d"` → Ok

**Confidence**: HIGH
**Source**: `src/jql.rs:16-34`
**Behavior**: JQL relative-date validator (distinct from worklog parser).
**Trace**: Pass 3 BC-131 (R1)

---

#### BC-X.9.003: `validate_date` → `YYYY-MM-DD` format only; invalid → `JrError::UserError`

**Confidence**: HIGH
**Source**: `src/jql.rs`
**Trace**: Pass 3 BC-132 (R1)

---

#### BC-X.9.004: `strip_order_by` removes ORDER BY clause before count calls and paren-wrapping

**Confidence**: HIGH
**Source**: `src/jql.rs`; `src/cli/issue/list.rs`
**Trace**: Pass 3 BC-102, BC-125 (R1)

---

### X.10 Partial-Match

#### BC-X.10.001: `partial_match` with single-substring → `Ambiguous` (NOT Exact); never auto-resolves

**Confidence**: HIGH
**Source**: `src/partial_match.rs::tests`; 12 unit tests; property tests
**Subject**: Partial-match
**Behavior**: Single-substring match returns `MatchResult::Ambiguous(matches)`. Callers must reject this under `--no-input`. This is the fail-closed invariant.
**Trace**: Pass 3 BC-105 context

---

#### BC-X.10.002: `partial_match(s, &candidates)` proptest: exact match always found; never panics on arbitrary input; empty candidates → None

**Confidence**: HIGH
**Source**: `src/partial_match.rs:153-198`
**Trace**: Pass 3 BC-1095..BC-1097 (R4)

---

#### BC-X.10.003: Duplicate candidates → `MatchResult::ExactMultiple(name)` with `name.to_lowercase() == input.to_lowercase()`

**Confidence**: HIGH
**Source**: `src/partial_match.rs:182-198`
**Trace**: Pass 3 BC-1098 (R4)

---

### X.11 Build-Time

#### BC-X.11.001: `build.rs` reads `JR_BUILD_OAUTH_CLIENT_ID` + `_SECRET` env vars

**Confidence**: HIGH
**Source**: `build.rs` (125 LOC)
**Trace**: Pass 3 BC-1301

---

#### BC-X.11.002: Unix → `/dev/urandom` for 32-byte XOR key; Windows → inline `BCryptGenRandom` FFI

**Confidence**: HIGH
**Source**: `build.rs`
**Trace**: Pass 3 BC-1302

---

#### BC-X.11.003: Non-unix/non-windows → `compile_error!`

**Confidence**: HIGH
**Source**: `build.rs`
**Trace**: Pass 3 BC-1303

---

#### BC-X.11.004: Unset build vars → `EMBEDDED_*` constants are `None`; BYO/prompt path proceeds

**Confidence**: HIGH
**Source**: `build.rs`; `src/api/auth_embedded.rs::tests`
**Trace**: Pass 3 BC-1304

---

#### BC-X.11.005: `proptest-regressions/jql.txt` pinned regression seed for `escape_value("")`

**Confidence**: HIGH
**Source**: `proptest-regressions/jql.txt`
**Trace**: Pass 3 BC-1103 (R4)

---

## Key Invariants

- MAX_RETRIES = 3 (4 total calls); change trips `expect(4)` wiremock assertions
- DEFAULT_RETRY_SECS = 1 (Retry-After fallback)
- No upper bound on Retry-After integer (NFR-R-NEW-1 LOW)
- `partial_match` single-substring → Ambiguous (fail-closed invariant)
- User pagination advances by REQUESTED size (JRACLOUD-71273 workaround)
- Worklog days/hours: 8h/day, 5d/week (hardcoded, NFR-R-C)
- `send` vs `send_raw` bifurcation: typed path vs raw passthrough
