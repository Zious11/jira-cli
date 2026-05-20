---
context: bc-x
title: "Cross-cutting (HTTP client, Runtime, Users, Teams, Worklogs, Projects, Queues, JQL, Partial-match, JSM Request Types)"
total_bcs: 140   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 74   # count of `#### BC-` headings in this file
last_updated: 2026-05-19
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/cross-cutting.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.6-2.15
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §3.6-3.8
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.2-3.4
  - F2 addition (2026-05-18): BC-X.12.001..008 — JSM request type discovery (issue #288)
  - F2 addition (2026-05-19): BC-X.8.006..007 — auth-conditional 401 hints on require_service_desk path (cache miss only): Basic-auth (is_oauth_auth==false) → API-token hint with InsufficientScope rewrite; OAuth (is_oauth_auth==true) → read:jira-work + read:servicedesk-request hint (issue #384; corrected model: gate is is_oauth_auth() alone)
---

# BC-X — Cross-cutting

140 behavioral contracts covering: HTTP client (X.1), Pagination (X.2), Error handling (X.3),
Rate limiting (X.4), Worklogs & duration (X.5), Teams (X.6), Users (X.7), Projects & Queues (X.8),
JQL utilities (X.9), Partial-match (X.10), Build-time (X.11), JSM Request Types (X.12).

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
**Source**: `src/api/pagination.rs`; unit test suite (pagination module); `tests/comments.rs:104-158`
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

> **[UPDATED 2026-05-19 issue #384]** JSM auth-conditional footnote: For JSM dispatch paths (both `handle_jsm_create` and `require_service_desk`), 401 behavior is auth-conditional — see BC-3.8.014 / BC-X.8.006 (Basic-auth: `is_oauth_auth() == false` → API-token-expiry hint; any `InsufficientScope` is REWRITTEN to `NotAuthenticated` before surfacing) and BC-3.8.015 / BC-X.8.007 (OAuth: `is_oauth_auth() == true` → existing error-variant behavior preserved). The gate is `is_oauth_auth()` alone, not error variant. The Base contract BC-X.3.002 applies to all non-JSM paths and to any JSM path that does not trigger the auth-conditional map_err.

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
**Source**: `src/api/rate_limit.rs:14-18`; unit test suite (rate_limit module)
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

#### BC-X.5.005: `parse_duration_validate("1w2d3h30m")` accepts combined units (validator — production path only)

**Confidence**: HIGH
**Source**: `src/duration.rs::tests::test_complex`
**Subject**: Duration
**Behavior**: Distinguished from JQL `validate_duration` which rejects combined units. Used for worklog add. `parse_duration_validate("1w2d3h30m")` is the sole production path. Note: the 3-arg `parse_duration(s, hours_per_day, days_per_week)` calculator was deleted in S-3.10 — it had no production caller after S-2.06 v2.0.0 and was retained only for the `format_duration` round-trip proptest, which has been rewritten to not depend on it.
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

#### BC-X.5.009: `worklog add` forwards the user-supplied duration string to Jira as `timeSpent`

**Confidence**: HIGH
**Source**: `src/cli/worklog.rs::handle_add` + `src/api/jira/worklogs.rs::add_worklog` + `src/duration.rs::parse_duration_validate`
**Subject**: Duration
**Behavior**: `worklog add` forwards the user-supplied duration string to Jira as `timeSpent`. Jira's server applies its configured `workingHoursPerDay`/`workingDaysPerWeek`. `parse_duration_validate` is a client-side syntax validator only (no arithmetic). Resolves NFR-R-C silent-wrong-answer on customized instances. RESOLVED via S-2.06 v2.0.0 (PR #308 / c8f15d8 / DEC-010 / Option 1 pivot).
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

#### BC-X.8.004: `require_service_desk` errors for software project: "Jira Software project" + queue-command-specific error message

**Confidence**: HIGH
**Source**: `tests/project_meta.rs:99-126`
**Trace**: Pass 3 BC-805

> **[UPDATED 2026-05-18 issue #288]** The literal "Queue commands require…" error string is removed from `src/api/jsm/servicedesks.rs::require_service_desk` and replaced by a caller-supplied context label. BC-X.8.004 now defines the queue-command-specific message only: 'Project "<KEY>" is a <type> project. Queue commands (`jr queue`) require a Jira Service Management project. Run "jr project list" to find a JSM project.' For the `jr issue create --request-type` call site, the error message is: 'Project "<KEY>" is a <type> project. `--request-type` requires a Jira Service Management project. Run "jr project list" to find a JSM project.' (see BC-3.8.002). For `jr requesttype list/fields` call sites: 'Project "<KEY>" is a <type> project. `jr requesttype` commands require a Jira Service Management project. Run "jr project list" to find a JSM project.' (see BC-X.12.003). Previous version of this BC required only the common prefix "Jira Software project" — the call-site-specific suffix is now part of the contract.
>
> **Implementation contract**: The call-site label is passed to `require_service_desk(client, project_key, call_site_label)` as a `&'static str` parameter. The function MUST NOT hard-code per-call-site branches; the message is formatted with the supplied label. Acceptable `call_site_label` values: `"queue commands"`, `"--request-type"`, `"jr requesttype commands"` (or equivalent constants in the calling modules). The implementer may use an enum if it strengthens type safety, but the boundary contract at the function signature is `&'static str`.

---

#### BC-X.8.005: `list_projects` paginates via `startAt`; filter via `typeKey` query param

**Confidence**: HIGH
**Source**: `tests/project_commands.rs:1-323`
**Trace**: Pass 3 BC-1133d, BC-1133e (R4)

---

#### BC-X.8.006: Basic-auth 401 from `require_service_desk` (cache miss) → API-token-expiry hint; no OAuth-scope language

**Confidence**: HIGH
**Subject**: X.8 Projects & Queues (JSM auth-conditional error hint — require_service_desk path)
**Behavior**: `require_service_desk` in `src/api/jsm/servicedesks.rs` calls `get_or_fetch_project_meta` which is cache-first (7-day TTL). The 401 hint described here fires ONLY on a cache MISS — when live HTTP calls are actually issued. **Trigger clarification (C-01):** `get_or_fetch_project_meta` issues TWO live GETs on a cache miss for a `service_desk`-type project: (1) `GET /rest/api/3/project/{key}` to fetch project details, and (2) `GET /rest/servicedeskapi/servicedesk` (via `client.list_service_desks()`) to match service desk by `projectId`. The new `map_err` wraps the entire `get_or_fetch_project_meta(...)` future, so it catches a 401 from EITHER GET. Both are JSM-read operations; the API-token-expiry hint applies uniformly to both. **User-facing behavioral boundary**: a warm `(profile, project_key)` project-meta cache entry suppresses this hint at the `require_service_desk` step; the 401 then surfaces at the next live HTTP call (e.g., the JSM POST → BC-3.8.014/015). Any test exercising this BC MUST force a cache miss (e.g., by not pre-populating the project-meta cache).

When a live GET inside `get_or_fetch_project_meta` returns 401 AND the active auth scheme is Basic (i.e., `JiraClient::is_oauth_auth()` returns `false`), the implementation MUST **introduce a NEW `map_err`** on the `get_or_fetch_project_meta(...)` call inside `require_service_desk` (line 117 of `src/api/jsm/servicedesks.rs`). The current code at line 117 is `let meta = get_or_fetch_project_meta(client, project_key).await?;` — the `?` propagates raw with no hint. The new `map_err` must surface an API-token-expiry hint. The gate is `is_oauth_auth() == false` ALONE — the incoming error variant is irrelevant.

**Dual exit codes on `require_service_desk`:** After this BC is implemented, `require_service_desk` has TWO failure exit codes: exit 64 (`JrError::UserError`, the existing non-JSM-project path, BC-X.8.004) and exit 2 (`JrError::NotAuthenticated`, the new 401 path). The implementer MUST NOT normalize them — they are distinct error categories.

Implementation: the new `map_err` must REWRITE any incoming error (whether `JrError::NotAuthenticated` or `JrError::InsufficientScope`) to `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`. The shared constant `API_TOKEN_EXPIRY_HINT` (defined once in **`src/error.rs`** — NOT in `src/api/client.rs` or any new module) is referenced identically by both the `handle_jsm_create` site (BC-3.8.014) and this `require_service_desk` site. `src/error.rs` is imported by both the `api` and `cli` layers with no layering inversion. This prevents hint-text divergence between the two call sites and adds no new modules.

The `hint` field value (stored in `JrError::NotAuthenticated { hint }`) MUST be identical to BC-3.8.014's hint (shared constant). The rendered stderr line prepends `"Not authenticated. "`; the `hint` field contains only the body text. Tests MUST assert via `contains`, not `==`. The hint field value is:

<!-- This block is duplicated from the CANONICAL copy in prd-delta-384.md §BC-3.8.014 — all copies MUST be updated together; cf. the JR_* doc-fallout pattern in CLAUDE.md (adversary-pass-4 F-04). -->
```
Your API token may be expired or revoked. Regenerate it at
https://id.atlassian.com/manage-profile/security/api-tokens
then run `jr auth login` to re-store the credentials.
```

This hint MUST NOT contain any OAuth-scope language. The hint MUST NOT say `jr auth refresh` (meaningless for Basic auth). The `require_service_desk` function is shared across all JSM callers: `handle_jsm_create` (`jr issue create --request-type`), `jr queue list/view`, and `jr requesttype list/fields`. All callers benefit from this contract.

Gate: `client.is_oauth_auth() == false` at the new `map_err` site on the `get_or_fetch_project_meta` call inside `require_service_desk` (per orchestrator decision 1: the contract is on `require_service_desk` itself, not on individual callers).

**Inputs**: Active auth = Basic; `GET /rest/api/3/project/{key}` returns HTTP 401 (any body shape); project-meta cache is empty (cache miss — the live GET is issued).
**Outputs/Effects**: exit 2; stderr contains the API-token-expiry hint (assert via `contains`); stdout empty; any `InsufficientScope` from the 401 is rewritten to `NotAuthenticated` before surfacing.
**Errors**: None beyond the 401 itself — this BC IS the error-handling contract.
**Setup** (for `test_require_service_desk_basic_auth_401_surfaces_api_token_hint`):
0. **Isolated `XDG_CACHE_HOME` tempdir** (e.g., `tempfile::tempdir()`) — forces a project-meta cache miss so the live GET inside `get_or_fetch_project_meta` actually fires. A warm cache would bypass the HTTP call and the 401 would never be seen.
1. Auth fixture: `JR_AUTH_HEADER=Basic <b64>` (capital B, single space — any valid Base64 value; e.g., `Basic dGVzdDp0ZXN0`).
2. Mount `GET /rest/api/3/project/{KEY}` to return HTTP 401 with a verbatim generic-expiry body: `{"errorMessages": ["The access token provided is expired, revoked, malformed, or invalid for other reasons."], "errors": {}}`. This is the **canonical pinned 401 path** for this named test — the project GET is triggered first by `get_or_fetch_project_meta` on a cache miss. **URL-encoding note (adversary-pass-8 LOW):** the project key is URL-encoded by `get_or_fetch_project_meta` via `urlencoding::encode`, so a wiremock `path()` matcher is exact for plain-alphanumeric keys (the named test uses `HELP`); a project key containing special characters would require an encoded mock path.
3. The second GET arm (`list_service_desks()` → `GET /rest/servicedeskapi/servicedesk`) is covered **structurally** because the `map_err` wraps the entire `get_or_fetch_project_meta` future — it is NOT separately pinned by this test. A dedicated test for the service-desk-list 401 arm is not required; the `map_err` wraps both GETs uniformly. The canonical-vs-structural distinction is explicit: this test pins the project-GET arm; the service-desk-list arm is covered by the shared `map_err` on `get_or_fetch_project_meta`.
4. Drive via: `jr issue create --project <KEY> --request-type <NAME> --summary "..." --no-input` (which calls `require_service_desk` first, triggering the 401 before reaching the JSM POST).
**Trace**: `tests/issue_create_jsm.rs` (integration test `test_require_service_desk_basic_auth_401_surfaces_api_token_hint` — NEW; Basic-auth fixture, cache miss forced; asserts stderr `contains` "expired or revoked" and `contains` `id.atlassian.com/manage-profile/security/api-tokens` and `contains` `jr auth login`; asserts stderr does NOT contain `write:servicedesk-request`). The new `map_err` is placed inside `require_service_desk` (shared by `jr issue create`, `jr queue`, `jr requesttype`), so all three callers structurally benefit; this test pins the `create` caller path; existing `queue`/`requesttype` integration tests cover regression for those callers.
**Source**: Issue #384 F2; O-08-05 CONFIRMED in `.factory/research/issue-288-pr4-deferred-validation.md` (lines 342-381); `src/api/client.rs:696-704` (body check before Bearer guard — same issue as BC-3.8.014); `src/api/jsm/servicedesks.rs:52-85` (get_or_fetch_project_meta issues TWO live GETs on a service_desk-type cache miss: GET /rest/api/3/project/{key} AND GET /rest/servicedeskapi/servicedesk; the new map_err must wrap the entire future, catching 401 from either GET); `src/api/jsm/servicedesks.rs:117` (raw `?` propagation — no existing map_err on get_or_fetch_project_meta call; the new map_err MUST be introduced here).
**Confidence**: HIGH

[NEW 2026-05-19 issue #384 F2] Closes O-08-05: `require_service_desk` 401 on the project-GET/service-desk-list path had no JSM-specific hint. The auth-conditional `map_err` is placed inside `require_service_desk` itself (not at call sites), so all three JSM caller paths benefit. Gate is `is_oauth_auth() == false` alone; map_err must rewrite both `NotAuthenticated` and `InsufficientScope` to the API-token hint (shared constant with BC-3.8.014 site).

[REVISED 2026-05-19 issue #384 F2 adversary correction] Previous version incorrectly stated `src/api/client.rs:711-722 (Basic-auth 401 → NotAuthenticated)` as the explanation. This is incomplete: a Basic-auth 401 with a "scope does not match" body lands in `InsufficientScope` (body check at line 696 fires before Bearer guard at line 718). The corrected model: gate is `is_oauth_auth() == false` alone; `map_err` rewrites any incoming variant.

[REVISED 2026-05-19 issue #384 F2 adversary-pass-2 C-01/H-01/H-04/M-02/M-03] (C-01) Changed "map_err inside require_service_desk" to "MUST introduce a NEW map_err" — no existing map_err exists at line 117; the implementation must add one. (H-01) Dual exit codes documented explicitly: exit 64 (UserError, non-JSM) vs exit 2 (NotAuthenticated, 401). (M-02) Cache-warm suppression stated as user-facing behavioral boundary, not just a test-setup note. (M-03) API_TOKEN_EXPIRY_HINT constant location pinned to src/error.rs.

[REVISED 2026-05-19 issue #384 F2 adversary-pass-3 C-01/H-05] (C-01) Trigger broadened: `get_or_fetch_project_meta` issues TWO live GETs for service_desk-type projects — the project GET AND the service-desk list GET. The new map_err catches 401 from either. (H-05) Named acceptance test function added: `test_require_service_desk_basic_auth_401_surfaces_api_token_hint`; cross-caller coverage clarified (map_err is in require_service_desk; test pins create path; queue/requesttype existing tests cover regression).

---

#### BC-X.8.007: OAuth 401 from `require_service_desk` (cache miss) → read-side scope hint (`read:jira-work` + `read:servicedesk-request`)

**Confidence**: HIGH
**Subject**: X.8 Projects & Queues (JSM auth-conditional error hint — require_service_desk path)
**Behavior**: `require_service_desk` calls `get_or_fetch_project_meta` which is cache-first (7-day TTL). This BC fires ONLY on a cache MISS — when live HTTP calls are actually issued. **Trigger clarification (C-01):** `get_or_fetch_project_meta` issues TWO live GETs on a cache miss for a `service_desk`-type project: (1) `GET /rest/api/3/project/{key}` to fetch project details, and (2) `GET /rest/servicedeskapi/servicedesk` (via `client.list_service_desks()`) to match service desk by `projectId`. The new `map_err` wraps the entire `get_or_fetch_project_meta(...)` future, so it catches a 401 from EITHER GET. Both are JSM-read operations; the read-side scope hint applies uniformly to both. **User-facing behavioral boundary**: a warm `(profile, project_key)` project-meta cache entry suppresses this hint at the `require_service_desk` step; the 401 then surfaces at the next live HTTP call (e.g., the JSM POST → BC-3.8.014/015). Any test exercising this BC MUST force a cache miss.

When a live GET inside `get_or_fetch_project_meta` returns 401 AND the active auth scheme is OAuth/Bearer (i.e., `JiraClient::is_oauth_auth()` returns `true`), the NEW `map_err` introduced inside `require_service_desk` (see BC-X.8.006 — same new `map_err` on line 117) MUST surface a read-side scope hint for BOTH sub-cases, via `JrError::NotAuthenticated { hint }`. The gate is `is_oauth_auth() == true` ALONE.

**Dual exit codes on `require_service_desk`:** After BC-X.8.006/007 are implemented, `require_service_desk` has TWO failure exit codes: exit 64 (`JrError::UserError`, the existing non-JSM-project path, BC-X.8.004) and exit 2 (`JrError::NotAuthenticated`, the new 401 path from this BC and BC-X.8.006). The implementer MUST NOT normalize them — they are distinct error categories.

For both sub-cases of OAuth 401, the implementation rewrites to `JrError::NotAuthenticated { hint }` (NOT `InsufficientScope` — the `InsufficientScope` Display is a fixed template purpose-built for the issue-#185 POST scenario; for a read GET it produces irrelevant POST-specific noise). Both arms of the `map_err` emit the SAME single canonical hint string — there is ONE pinnable hint text, not two. This makes the acceptance test unambiguous: both the `InsufficientScope` arm and the `NotAuthenticated` arm produce identical output.

Rationale for hint content: `GET /rest/api/3/project/{key}` is a platform endpoint requiring `read:jira-work`; JSM service-desk context discovery additionally requires `read:servicedesk-request`. Both scopes are in `DEFAULT_OAUTH_SCOPES` (verified: `src/api/auth.rs:60-61`), so re-consent via `jr auth login` genuinely obtains them — the hint IS actionable. Because jr's default OAuth app already grants these scopes, expiry is the more common cause for default-scoped users. The hint therefore LEADS with session-expiry recovery (`jr auth refresh` / `jr auth login`) and SECOND mentions, for BYO-OAuth users, that `jr auth login` must be used to re-consent with `read:jira-work` and `read:servicedesk-request` — `jr auth refresh` alone cannot add missing scopes (it re-mints with the same granted scope set) (H-03: expiry-recovery leads; BYO-scope sentence is secondary and explicitly connects `jr auth login` to scope acquisition).

NOTE: this does NOT change BC-3.8.015 — the JSM POST OAuth `InsufficientScope` arm is genuinely the #185 POST scenario, so keeping `InsufficientScope` there is correct and unchanged. Scopes are `read:jira-work` + `read:servicedesk-request` (NOT `write:servicedesk-request` — that applies to the subsequent POST, which `require_service_desk` never reaches).

Gate: `client.is_oauth_auth() == true` at the new `map_err` site inside `require_service_desk` (same `map_err` as BC-X.8.006, branching on the predicate result).

The `hint` field value (body text after the `"Not authenticated. "` renderer prefix from `src/error.rs`). Tests MUST assert via `contains`, not `==`. Both arms of the `require_service_desk` OAuth 401 `map_err` emit this identical hint:

<!-- This block is duplicated from the CANONICAL copy in prd-delta-384.md §BC-X.8.007 — all copies MUST be updated together; cf. the JR_* doc-fallout pattern in CLAUDE.md (adversary-pass-4 F-04). -->
```
Your OAuth token may be expired. Run `jr auth refresh` to renew the token, or
`jr auth login` to re-authorize. If using a custom OAuth app, run `jr auth login`
to re-consent with read:jira-work and read:servicedesk-request — `jr auth refresh`
alone cannot add missing scopes (it re-mints with the same granted scope set).
```

This is the canonical pinnable string for `test_require_service_desk_oauth_401_surfaces_read_scope_hint`. Acceptance tests assert `contains` `read:jira-work` AND `contains` `read:servicedesk-request`; assert does NOT contain `write:servicedesk-request`.

**Inputs**: Active auth = Bearer/OAuth; a live GET inside `get_or_fetch_project_meta` returns HTTP 401 (any body — project GET or service-desk list GET); project-meta cache is empty (cache miss — the live GETs are issued).
**Outputs/Effects**: exit 2; stderr contains `"Not authenticated. "` prefix and read-scope hint (assert `contains` `read:jira-work` AND `contains` `read:servicedesk-request`; assert does NOT contain `write:servicedesk-request`); stdout empty.
**Errors**: None beyond the 401 itself — this BC IS the error-handling contract.
**Setup** (for `test_require_service_desk_oauth_401_surfaces_read_scope_hint`):
0. **Isolated `XDG_CACHE_HOME` tempdir** (e.g., `tempfile::tempdir()`) — forces a project-meta cache miss so the live GET inside `get_or_fetch_project_meta` actually fires. A warm cache would bypass the HTTP call and the 401 would never be seen.
1. Auth fixture: `JR_AUTH_HEADER=Bearer test-oauth-token` (capital B, single space — the established OAuth/Bearer fixture string used throughout `tests/issue_create_jsm.rs`).
2. Mount `GET /rest/api/3/project/{KEY}` to return HTTP 401 with a **scope-mismatch body**: `{"errorMessages": ["Unauthorized; scope does not match"]}`. **WHY scope-mismatch body is required:** A Bearer client receiving a generic-expiry 401 body on this GET does NOT short-circuit to `JrError::InsufficientScope` — it enters the auto-refresh coordinator (client.rs:727+), which deterministically fails with a raw `anyhow::bail!` error (not a `JrError`) via the `JR_AUTH_HEADER` seam (no keychain tokens). That raw error propagates without entering the `map_err`'s `JrError` match arms, so the read-scope hint is never injected. The scope-mismatch body (`"scope does not match"` substring) triggers the short-circuit at client.rs:696-704 BEFORE the refresh coordinator, landing as `JrError::InsufficientScope` in the `map_err`, which then rewrites to `JrError::NotAuthenticated { hint }` with the read-scope hint. A generic-expiry body would produce a non-deterministic, non-`JrError` failure path — not a valid pin for this BC. **BC-X.8.006 (Basic) is NOT affected** by this constraint: a Basic 401 never enters the refresh path (gated on `Bearer` at client.rs:718), so any body deterministically yields a `JrError`; BC-X.8.006's Setup may use a generic-expiry body (as specified). This is the **canonical pinned 401 path** for this named test — the project GET is triggered first by `get_or_fetch_project_meta` on a cache miss. **URL-encoding note (adversary-pass-8 LOW):** the project key is URL-encoded by `get_or_fetch_project_meta` via `urlencoding::encode`, so a wiremock `path()` matcher is exact for plain-alphanumeric keys (the named test uses `HELP`); a project key containing special characters would require an encoded mock path.
3. The second GET arm (`list_service_desks()` → `GET /rest/servicedeskapi/servicedesk`) is covered **structurally** because the `map_err` wraps the entire `get_or_fetch_project_meta` future — it is NOT separately pinned by this test. The canonical-vs-structural distinction is explicit: this test pins the project-GET arm; the service-desk-list arm is covered by the shared `map_err` on `get_or_fetch_project_meta`. No dedicated test for the service-desk-list 401 arm is required; both arms emit the identical hint (as established in BC-X.8.007 body above).
4. Drive via: `jr issue create --project <KEY> --request-type <NAME> --summary "..." --no-input` (which calls `require_service_desk` first, triggering the 401 before reaching the JSM POST). The test mounts only the 401 project-GET mock; no request-type resolution mock is needed because the command exits at the `require_service_desk` step.
**Trace**: `tests/issue_create_jsm.rs` (integration test `test_require_service_desk_oauth_401_surfaces_read_scope_hint` — NEW; OAuth/Bearer fixture, cache miss forced; asserts stderr `contains` `read:jira-work` AND `contains` `read:servicedesk-request`; asserts stderr does NOT contain `write:servicedesk-request`). The new `map_err` is placed inside `require_service_desk` (shared by `jr issue create`, `jr queue`, `jr requesttype`), so all three callers structurally benefit; this test pins the `create` caller path; existing `queue`/`requesttype` integration tests cover regression for those callers.
**Source**: Issue #384 F2; O-08-05 CONFIRMED; `src/api/auth.rs:60-61` (both `read:jira-work` and `read:servicedesk-request` in DEFAULT_OAUTH_SCOPES — hint IS actionable for default-scoped users); `src/api/client.rs:696-704` (scope-mismatch body detection → InsufficientScope); `src/api/jsm/servicedesks.rs:52-85` (get_or_fetch_project_meta issues TWO live GETs on a service_desk-type cache miss: GET /rest/api/3/project/{key} AND GET /rest/servicedeskapi/servicedesk; the new map_err must wrap the entire future); orchestrator decision: read-side scopes for this path, NOT write-scope; `src/api/jsm/servicedesks.rs:117` (new map_err must be introduced here — see BC-X.8.006).
**Confidence**: HIGH

[NEW 2026-05-19 issue #384 F2] Pins the OAuth read-scope hint for the require_service_desk 401 path. Prior to issue #384, no hint existed for this path. The read-side scope names differ from BC-3.8.015's write-scope name — a user whose token has `write:servicedesk-request` but not `read:jira-work` would fail at require_service_desk before ever reaching the POST. Both scopes are in DEFAULT_OAUTH_SCOPES, making `jr auth login` genuinely actionable for session-expiry cases.

[REVISED 2026-05-19 issue #384 F2 adversary-pass-2 C-02/C-03/H-01/M-02] (C-02) Removed incorrect "Insufficient token scope. " (period) renderer-prefix citation — the actual `InsufficientScope` Display renders with a colon: "Insufficient token scope: {message}". (C-03) Both sub-case arms of the OAuth 401 now rewrite to `JrError::NotAuthenticated { hint }` — NOT `InsufficientScope`. The `InsufficientScope` Display is purpose-built for the issue-#185 POST scenario and always appends irrelevant POST-specific guidance when applied to a read GET. (H-01) Dual exit codes documented explicitly. (M-02) Cache-warm suppression stated as user-facing behavioral boundary.

[REVISED 2026-05-19 issue #384 F2 adversary-pass-3 C-01/H-03/H-04/H-05] (C-01) Trigger broadened: get_or_fetch_project_meta issues TWO live GETs for service_desk-type projects; map_err wraps the entire future and catches 401 from either. (H-03) Hint ordering corrected: leads with session-expiry recovery (jr auth refresh / jr auth login), BYO-scope sentence is SECONDARY. (H-04) Both arms of the map_err emit ONE canonical verbatim hint — no sub-case difference; single pinnable string documented; hint block relabeled "both arms emit this identical hint". (H-05) Named acceptance test function added: `test_require_service_desk_oauth_401_surfaces_read_scope_hint`; cross-caller coverage clarified.

[REVISED 2026-05-19 issue #384 adversary-pass-6 F-07] BYO-OAuth sentence in hint reworded: for a BYO-OAuth user with genuinely missing scopes, `jr auth refresh` re-mints a token with the SAME deficient scope set — it cannot add scopes. Only `jr auth login` re-consents and can acquire `read:jira-work` + `read:servicedesk-request`. Hint text updated to connect `jr auth login` explicitly to scope acquisition; `jr auth refresh` positioned as expiry-recovery only. Rationale paragraph in BC body aligned.

[REVISED 2026-05-19 issue #384 adversary-pass-9 C-01 CRITICAL design correction] Setup block corrected: the project-GET 401 mock body changed from generic-expiry to **scope-mismatch** (`{"errorMessages": ["Unauthorized; scope does not match"]}`). A Bearer client receiving a generic-expiry 401 on this GET routes through the refresh coordinator (client.rs:727+), which fails with a raw anyhow error (not a `JrError`) via the `JR_AUTH_HEADER` seam — the read-scope hint is never injected, making the test non-deterministic. The scope-mismatch body short-circuits to `JrError::InsufficientScope` at client.rs:696-704 BEFORE the refresh coordinator, deterministically reaching the `map_err`. BC-X.8.006 (Basic) is UNAFFECTED — Basic 401s never enter the refresh path and any body yields a `JrError` deterministically; BC-X.8.006's generic-expiry Setup remains as-is.

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
**Source**: `src/partial_match.rs::tests`; unit test suite (partial_match module); property tests
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

## BC-X.12: JSM Request Type Discovery

8 behavioral contracts covering `jr requesttype list` and `jr requesttype fields` subcommands,
backed by the service desk requesttype API. These are discovery commands used before
`jr issue create --request-type` to identify valid request types and their required fields.

---

#### BC-X.12.001: `jr requesttype list` lists request types for the active project's service desk

**Confidence**: HIGH
**Subject**: JSM request type discovery
**Behavior**: `jr requesttype list --project <KEY>` calls `GET /rest/servicedeskapi/servicedesk/<id>/requesttype` (paginated via `isLastPage`). Default table output shows columns: Name, Description. ID is available in `--output json` only. Returns all request types for the resolved service desk. Uses `require_service_desk(client, key)` to resolve the `serviceDeskId` before calling the list endpoint.
**Inputs**: `--project <KEY>` (required; uses active-profile project if absent and profile has one configured)
**Outputs/Effects**: stdout table (Name + Description columns by default); exit 0 on success.
**Errors**: No project configured and no `--project` flag → exit 64 "project is required". Non-JSM project → exit 64 via `require_service_desk` (BC-X.8.004).
**Trace**: `tests/requesttype_commands.rs` (list command, table output); `src/cli/requesttype.rs`; `src/api/jsm/request_types.rs`
**Source**: API-verified: `GET /rest/servicedeskapi/servicedesk/{id}/requesttype` returns `{start, limit, isLastPage, values}`
**Confidence**: HIGH

---

#### BC-X.12.002: `--search <QUERY>` filters via JSM `searchQuery` parameter (name or description partial match)

**Confidence**: HIGH
**Subject**: JSM request type discovery
**Behavior**: When `--search <QUERY>` is set, the `searchQuery` query parameter is appended to `GET /rest/servicedeskapi/servicedesk/<id>/requesttype?searchQuery=<QUERY>`. Filtering is server-side (Atlassian API). No client-side secondary filtering is applied. If `--search` returns an empty `values` array, the command exits 0 with an empty table (NOT an error). The `searchQuery` parameter supports name and description substring matching as defined by the Atlassian API.
**Inputs**: `--search <QUERY>` (optional)
**Outputs/Effects**: Filtered request type list; may be empty table on no match.
**Errors**: API error (5xx) → exit 1 + "API error (N)". 401 → exit 2 + `jr auth login`.
**Trace**: `tests/requesttype_commands.rs` (search parameter propagation, empty-result path)
**Source**: API-verified: `searchQuery` is a supported query param on the list endpoint
**Confidence**: HIGH

---

#### BC-X.12.003: `--project <KEY>` overrides active profile; `require_service_desk` errors clean on non-JSM project with call-site-specific message

**Confidence**: HIGH
**Subject**: JSM request type discovery
**Behavior**: `--project <KEY>` takes precedence over any project configured in the active profile (same precedence rule as all other project-flag uses). The flag is the non-interactive mechanism for specifying the target project. `require_service_desk` returns a typed error for non-JSM (software) projects — the command exits 64 with a call-site-specific error message (NOT the legacy "Queue commands require…" string). Error message MUST be: 'Project "<KEY>" is a <type> project. `jr requesttype` commands require a Jira Service Management project. Run "jr project list" to find a JSM project.' Zero HTTP calls to the requesttype endpoint are made.
**Inputs**: `--project <KEY>` (overrides profile-level project config)
**Outputs/Effects**: Project-scoped service desk ID resolved before any requesttype API call.
**Errors**: Non-JSM project → exit 64 + call-site-specific message (see above); NO requesttype HTTP. Software project check fires before the list request.
**Trace**: `tests/requesttype_commands.rs` (non-JSM project exit-64 path); `src/api/jsm/servicedesks.rs::require_service_desk`
**Source**: Reuses `require_service_desk` established for `jr queue`; caller-supplied context label per BC-X.8.004 [UPDATED 2026-05-18 issue #288]
**Confidence**: HIGH

---

#### BC-X.12.004: `--output json` returns structured JSON array; default table shows Name + Description columns

**Confidence**: HIGH
**Subject**: JSM request type discovery
**Behavior**: `jr requesttype list --output json` returns a JSON array to stdout: `[{id: "<str>", name: "<str>", description: "<str>", helpText: "<str>"|null, issueTypeId: "<str>"|null, groupIds: ["<str>", ...]}, ...]`. Each element uses the fields returned by the Atlassian API; `null` for absent optional fields. Table output (default) shows Name + Description columns only; ID is not shown in table mode. Truncation hint ("Showing N of M") goes to stderr when applicable.
**Inputs**: `--output json` (optional flag)
**Outputs/Effects**: stdout JSON array on `--output json`; stdout table on default.
**Errors**: Empty list returns `[]` (JSON) or empty table; NOT an error condition.
**Trace**: `tests/requesttype_commands.rs` (JSON output shape, table output shape); body deserialization tests
**Source**: API-verified: response values include `id`, `name`, `description`, `helpText`, `issueTypeId`, `groupIds`
**Confidence**: HIGH

---

#### BC-X.12.005: `jr requesttype fields <NAME|ID>` lists fields for a request type

**Confidence**: HIGH
**Subject**: JSM request type discovery
**Behavior**: `jr requesttype fields <NAME|ID> --project <KEY>` resolves the request type (by name or numeric ID, same logic as BC-X.12.006 below), then calls `GET /rest/servicedeskapi/servicedesk/<id>/requesttype/<rtId>/field`. Returns metadata about each field: `fieldId`, `name`, `required` (bool), `jiraSchema` (system/custom type info), and optionally `defaultValues` and `validValues`. Default table output shows columns: Field Name, Required (YES/NO), Type.
**Inputs**: `<NAME|ID>` positional argument (required); `--project <KEY>` (required or from profile)
**Outputs/Effects**: stdout table with field metadata; exit 0 on success.
**Errors**: Request type not found → exit 64 via `partial_match` (BC-X.12.006). Non-JSM project → exit 64 via `require_service_desk`.
**Caching**: Fields for a request type are cached per `(profile, serviceDeskId, requestTypeId)` with 7-day TTL at cache key `~/.cache/jr/v1/<profile>/request_type_fields_<service_desk_id>_<request_type_id>.json`. Cache miss → HTTP fetch + write. Corrupt or expired cache is treated as a miss (self-heals). Recovery path: manual deletion of the cache file (same convention as BC-X.12.008 for the request-type list cache). No `--refresh` flag is provided in this delta.
**Trace**: `tests/requesttype_commands.rs` (fields command, required/optional field rendering, cache hit: second call fires no HTTP); `src/cli/requesttype.rs`; `src/api/jsm/request_types.rs`; `src/cache.rs` (request_type_fields cache read/write functions)
**Source**: API-verified: `GET .../requesttype/{rtId}/field` returns `{canRaiseOnBehalfOf, canAddRequestParticipants, requestTypeFields[{fieldId, name, description?, required, defaultValues?, validValues?, jiraSchema{system|custom|customId|type}, visible}]}`. See also architecture-delta.md §"Cache Key Prefix".
**Confidence**: HIGH

---

#### BC-X.12.006: Partial-name resolution for `<NAME|ID>` uses `partial_match`; ambiguity errors with disambiguation hint

**Confidence**: HIGH
**Subject**: JSM request type discovery
**Behavior**: When `<NAME|ID>` is a non-numeric string, the handler fetches (or cache-hits) the request type list, extracts names, and calls `partial_match(input, &names)`. `MatchResult::Exact(id)` → proceeds. `MatchResult::Ambiguous` → exits 64 with "Ambiguous request type" + all candidate names listed in stderr + hint "Run `jr requesttype list --project <KEY>` to see all request types". `MatchResult::None` → exits 64 with "Request type not found: <input>" + same hint. `MatchResult::ExactMultiple(name)` (case-variant duplicates, e.g., "Password Reset" and "password reset") → exits 64 with `'Multiple request types named "<name>" found (IDs: <id1>, <id2>, ...). Pass the numeric ID directly.'` in stderr. Rationale: Atlassian REST does not guarantee a stable ordering for case-variant duplicates within the same service desk, so deterministic resolution requires the numeric ID. This matches the `cli/queue.rs` precedent for duplicate queue names. In `--no-input` mode, ambiguous result exits 64 cleanly without prompting.
[UPDATED 2026-05-18 issue #288 adversary-pass-01 H-3]: ExactMultiple was previously documented as "treated as Exact, proceeds" — hardened to exits 64 after impl review confirmed Atlassian REST does not guarantee stable ordering for case-variant duplicates, making "pick first" non-deterministic and unsafe. Conservative resolution (require numeric ID) matches cli/queue.rs precedent.
[UPDATED 2026-05-18 issue #288 adversary-pass-01 M-2]: Hint verb changed from "Use" to "Run" to match imperative active voice used throughout jr's CLI ergonomics and the impl's actual emission.
**Inputs**: `<NAME|ID>` positional (non-numeric → name resolution; numeric → bypass as in BC-3.8.004)
**Outputs/Effects**: Resolved `requestTypeId` integer used for the field fetch call.
**Errors**: Ambiguous → exit 64; None → exit 64; both without firing the field GET.
**Trace**: `tests/requesttype_commands.rs` (partial-match disambiguation, not-found, numeric bypass); `src/partial_match.rs`
**Source**: Follows `partial_match` pattern established by `jr queue` and `jr issue move`
**Confidence**: HIGH

---

#### BC-X.12.007: `--output json` for `jr requesttype fields` returns structured JSON with `required` bool per field; default table shows Field, Required, Type

**Confidence**: HIGH
**Subject**: JSM request type discovery
**Behavior**: `jr requesttype fields <NAME|ID> --output json --project <KEY>` returns a JSON object to stdout: `{canRaiseOnBehalfOf: bool, canAddRequestParticipants: bool, fields: [{fieldId: "<str>", name: "<str>", required: bool, jiraSchema: {type: "<str>", ...}, defaultValues?: [...], validValues?: [...]}]}`. The `required` field is a boolean (true = must be provided by submitter). Default table output shows: Field (name column), Required (YES/NO), Type (from `jiraSchema.type`).
**Inputs**: `--output json` (optional flag)
**Outputs/Effects**: stdout JSON object on `--output json`; stdout table on default.
**Errors**: API error (5xx) → exit 1. 401 → exit 2.
**Trace**: `tests/requesttype_commands.rs` (JSON output shape, required flag rendering)
**Source**: API-verified: `requestTypeFields[].required` is a boolean field in the API response
**Confidence**: HIGH

---

#### BC-X.12.008: Request types cached per `(profile, serviceDeskId)` with 7-day TTL; cache miss self-heals; cache key: `v1/<profile>/request_types_<service_desk_id>.json`

**Confidence**: HIGH
**Subject**: JSM request type discovery
**Behavior**: On `requesttype list` or name-resolution calls, the handler first checks `read_request_type_cache(profile, service_desk_id)`. Cache hit (valid, within 7-day TTL) → returns cached `Vec<RequestType>` without HTTP. Cache miss (absent, expired, or corrupt JSON) → fetches from API, writes to `write_request_type_cache(profile, service_desk_id, types)`, then proceeds. Cache file path: `~/.cache/jr/v1/<profile>/request_types_<service_desk_id>.json`. The `<service_desk_id>` in the filename is the numeric service desk ID as a string. Cache is keyed per `(profile, serviceDeskId)` to respect multi-profile isolation invariant (different profiles may have different service desks). Corrupt cache file is treated as a miss (self-heals).
**Inputs**: profile name (active profile), serviceDeskId (resolved by `require_service_desk`)
**Outputs/Effects**: Cache write on miss; cache read on hit (no HTTP). Cache TTL = 7 days (matching all other `jr` caches).
**Errors**: Cache write failure is non-fatal (logged to stderr as hint; does not abort the command). Cache corruption is non-fatal (treated as miss).
**Stale-cache window**: Up to 7 days. If a Jira admin renames a request type or modifies its required fields, users will see stale data for up to 7 days. No `--refresh` or `--no-cache` flag is provided in this delta (deferred). Recovery path: users may force a refresh by deleting `~/.cache/jr/v1/<profile>/request_types_<service_desk_id>.json` manually. Cache miss on `partial_match::None` does NOT auto-retry with cache-bypass; the error message MUST hint at manual cache deletion: 'Request type "<NAME>" not found. Run `jr requesttype list --project <KEY>` to see all request types, or delete the cache file at ~/.cache/jr/v1/<profile>/request_types_<service_desk_id>.json if a recent admin change is suspected.'

[UPDATED 2026-05-18 issue #288 adversary-pass-04 M-1 + M-4] Aligned hint phrasing
to BC-X.12.006 ("see all request types") and added the `--project <KEY>` flag for
actionability when no profile project is configured. Prior wording ("current types"
without `--project`) is superseded; impl + tests already match the aligned form.
**Fields cache**: See BC-X.12.005 §Caching for the per-request-type fields cache (sibling cache, same 7-day TTL and recovery semantics).
**Trace**: `tests/requesttype_commands.rs` (cache hit: second call fires no HTTP); `src/cache.rs` (RequestTypeCache struct); `src/api/jsm/request_types.rs`
**Source**: Follows `teams.json` cache pattern; 7-day TTL matches all other caches in `src/cache.rs`
**Confidence**: HIGH

---

## Key Invariants

- MAX_RETRIES = 3 (4 total calls); change trips `expect(4)` wiremock assertions
- DEFAULT_RETRY_SECS = 1 (Retry-After fallback)
- No upper bound on Retry-After integer (NFR-R-NEW-1 LOW)
- `partial_match` single-substring → Ambiguous (fail-closed invariant)
- User pagination advances by REQUESTED size (JRACLOUD-71273 workaround)
- Worklog days/hours: 8h/day, 5d/week (hardcoded, NFR-R-C)
- `send` vs `send_raw` bifurcation: typed path vs raw passthrough
