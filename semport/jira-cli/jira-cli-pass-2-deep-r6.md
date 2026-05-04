# Pass 2 Deepening — Round 6 — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04

## 1. Round metadata

- **Round**: 6
- **Predecessor**: `jira-cli-pass-2-deep-r5.md`
- **Targets attacked (verbatim from R5 §9 high+medium priority)**:
  - **#1** — `api/client.rs` (490 LOC) — full HTTP method surface, auth header order, 429 retry path, 401 detection, verbose-body redaction
  - **#2** — `cli/issue/create.rs` (375 LOC) — `handle_create` + `handle_edit` field-building, `--description-stdin`, label add:/remove: syntax
  - **#3** — `cli/worklog.rs` (79 LOC) + `duration.rs` (159 LOC) — handle_add pipeline, parser semantics, 8h/5d hardcoding
  - **#4** — `cli/team.rs` (120 LOC) — lazy org_id discovery, refresh-then-cache write, eprintln "No teams"
  - **#5** — `cli/user.rs` (165 LOC) — search/list/view paths, `resolve_effective_limit`, JrError downcast pattern
  - **#6** — `cli/queue.rs` (323 LOC) — require_service_desk gate, two-step issue hydration, queue ordering preservation
  - **#7** — `cli/project.rs` (133 LOC) — only 2 commands (List, Fields); CMDB silent-degrade on discovery failure
  - **#8** — `cli/issue/links.rs` (293 LOC) — link/unlink/remote-link, link-type partial-match, URL normalization
  - **#9** — `cli/issue/helpers.rs` (813 LOC) — UUID pass-through, auto-refresh-on-miss, disambiguate_user, resolve_asset
  - **#10** — `cli/issue/json_output.rs` (149 LOC) — full response shape catalog
  - **#11** — `api/rate_limit.rs` (55 LOC) — Retry-After parsing semantics

(Pass 4 cross-pollination items reserved for Pass 4 — not written into this Pass 2 file.)

---

## 2. Audit of Round 5 against the 5 Known Hallucination Classes

### Class 1 — Over-extrapolated token lists

- **R5 "Cumulative 306 distinct invariants"** — RECOUNT. R5 added NEW-INV-216..NEW-INV-306 = 91 new this round; cumulative = 215 + 91 = **306**. Confirmed. ✓ However, R5 also said "+31 entities" and "(broad 51 + R1 33 + R2 67 + R3 31 + R4 25 + R5 31 = 238)". Recount of R5 §3 sub-entries: list 4 + changelog 4 + workflow 5 + api-issues 4 + cache 3 + adf 3 + sprint 2 + board 3 + teams 2 + init 1 = **31**. ✓ Cumulative entity count is unverifiable without recounting R1..R4 which is out of scope for this round; treating R5's 238 as carry-forward.

- **R5 NEW-INV-229 (asset enrichment HashMap mis-attribution)** — **VERIFIED AT SOURCE**:
  ```rust
  // src/cli/issue/list.rs:398-451
  let mut to_enrich: StdHashMap<(String, String), ()> = StdHashMap::new();   // key = (wid, oid)
  ...
  let mut resolved: StdHashMap<String, (String, String, String)> = StdHashMap::new();   // key = oid alone
  for (oid, result) in results {
      if let Ok(obj) = result {
          resolved.insert(oid, (obj.object_key, obj.label, obj.object_type.name));
      }
  }
  ```
  Pass-1 dedup is `(wid, oid)`; Pass-2 result map is keyed by `oid` only (line 446); insertion at line 449 uses `oid` alone. Pass-3 lookup at line 456 (`resolved.get(oid)`) cannot disambiguate by workspace. **R5's bug claim is correct.** ✓ The fact that Pass-1 dedups by `(wid, oid)` but Pass-2 collapses back to `oid` alone means: in a single-workspace tenant the bug is invisible (oid is unique within a workspace anyway), but in a multi-workspace tenant the second insertion silently overwrites. The Pass-2 callable also closes over a synthesized `wid` (line 432-436) — the per-call workspace-ID is correct, but the result-map key isn't.

- **R5 NEW-INV-178 (no PKCE)** — re-verified in R5 §2; not re-verified again here.

- **R5 NEW-INV-179 (accessible_resources first-wins)** — re-verified in R5 §2; not re-verified again here.

- **R5 NEW-INV-261 (search_issues unbounded fetch)** — RE-VERIFIED at `api/jira/issues.rs:44-95` per R5's evidence. R5 reading is correct: when `limit` is None, the loop runs until `!page_has_more` with no max-pages cap. ✓

- **R5 NEW-INV-295 (GraphQL string-interpolated)** — RE-VERIFIED at `api/jira/teams.rs:14-16` (R5 cited the line range). Confirmed string-interpolation. ✓

### Class 2 — Miscounted enumerations

- **R5 §3.1 "16 invariants" attributed to handle_list deepening** — recount: NEW-INV-216..NEW-INV-231 = 16 invariants ✓
- **R5 §3.2 "12 invariants" for changelog** — NEW-INV-232..NEW-INV-243 = 12 invariants ✓
- **R5 §3.3 "14 invariants" for workflow** — NEW-INV-244..NEW-INV-257 = 14 invariants ✓
- **R5 §3.4 "9 invariants" for api/jira/issues.rs** — NEW-INV-258..NEW-INV-266 = 9 invariants ✓
- **R5 §3.5 "8 invariants" for cache.rs** — NEW-INV-267..NEW-INV-274 = 8 invariants ✓
- **R5 §3.6 "8 invariants" for adf.rs** — NEW-INV-275..NEW-INV-282 = 8 invariants ✓
- **R5 §3.7 "6 invariants" for sprint** — NEW-INV-283..NEW-INV-288 = 6 invariants ✓
- **R5 §3.8 "5 invariants" for board** — NEW-INV-289..NEW-INV-293 = 5 invariants ✓
- **R5 §3.9 "7 invariants" for teams** — NEW-INV-294..NEW-INV-300 = 7 invariants ✓
- **R5 §3.10 "6 invariants" for init** — NEW-INV-301..NEW-INV-306 = 6 invariants ✓

Enumeration sub-totals all reconcile. ✓ The "91 invariants this round" claim is correct (sum: 16+12+14+9+8+8+6+5+7+6 = **91**).

### Class 3 — Named pattern conflation / fabrication

- **R5 NEW-INV-246 (idempotence "JSON `{success: false}`")** — **PARTIAL RETRACTION**. Re-read `cli/issue/json_output.rs` shows the actual JSON field name is `"changed"`, NOT `"success"`. Verified against `move_response`, `assign_changed_response`, `assign_unchanged_response`, `unassign_response`. The semantic claim ("the three idempotent paths emit a flag that distinguishes 'did the work' vs 'was already done'") is correct, but R5 named the field wrong. Logged as **CONV-ABS-10 (CORRECTION)**: NEW-INV-246's field name is `"changed"`, not `"success"`. The architectural claim still holds; only the literal field name was wrong.

- **R5 NEW-INV-219 (silent degrade on no-active-sprint)** — RE-VERIFIED at `cli/issue/list.rs:283-293` per R5's reading. The `Ok([])` arm has no eprintln. ✓ Bug claim stands.

- **R5 NEW-INV-263 (anti-loop guard unique to get_changelog)** — RE-VERIFIED. `search_issues` (cursor-based, structurally can't have offset regression — but COULD have cursor-loop; `cursor` could come back equal to itself), `list_teams` (line 33-54), `search_assignable_users_*` (offset-paginated): none have an anti-loop guard. ✓

- **R5 NEW-INV-285 (MAX_SPRINT_ISSUES = 50)** — Not re-verified in this round; carried as previously verified.

### Class 4 — Same-basename artifact conflation

- **No new same-basename conflations introduced in R5.** The Pass 2 R5 file is clean of this class.
- **NEW (R6 finding)**: CLAUDE.md still says `cli/project.rs # project fields (types, priorities, statuses, CMDB fields)` implying multiple subcommands. **Source has only 2 subcommands** (List, Fields). The "types, priorities, statuses, CMDB fields" enumeration describes WHAT `Fields` shows — not separate commands. This is staleness in CLAUDE.md, similar to CONV-ABS-4/CONV-ABS-7. Logged as **CONV-ABS-11 (CORRECTION)**: `cli/project.rs` exposes only `List` and `Fields` subcommands; the latter shows ALL of {issue_types, priorities, statuses, cmdb_fields} in one combined output.

- **NEW (R6 finding)**: `cli/issue/edit.rs` does NOT exist as a separate file. `handle_edit` lives in `cli/issue/create.rs` (lines 203-354), alongside `handle_create`. R5's gap framing (`cli/issue/create.rs and cli/issue/edit.rs`) was fine because it bundled both under "create.rs and edit.rs" but the second file is a phantom. CLAUDE.md correctly says `create.rs # create + edit (field-building)` — no staleness here, but R5's phrasing implied two files.

### Class 5 — Inflated or deflated metrics (LOC recount)

| File | R5 cited | Actual | Delta |
|---|---:|---:|---|
| `src/api/client.rs` | (not cited explicitly; "broad pass + R3 (?)" framing) | **490** | n/a |
| `src/cli/issue/create.rs` | (not cited explicitly) | **375** | n/a |
| `src/cli/worklog.rs` | (not cited explicitly) | **79** | n/a |
| `src/duration.rs` | (not cited explicitly) | **159** | n/a |
| `src/cli/team.rs` | (not cited explicitly) | **120** | n/a |
| `src/cli/user.rs` | (not cited explicitly) | **165** | n/a |
| `src/cli/queue.rs` | (not cited explicitly) | **323** | n/a |
| `src/cli/project.rs` | (not cited explicitly) | **133** | n/a |
| `src/cli/issue/links.rs` | (not cited explicitly) | **293** | n/a |
| `src/cli/issue/helpers.rs` | (not cited explicitly) | **813** | n/a |
| `src/cli/issue/json_output.rs` | (not cited explicitly) | **149** | n/a |
| `src/api/rate_limit.rs` | (not cited explicitly) | **55** | n/a |

R5 did not publish LOC counts for these files; no comparison possible. Metrics from this round are baseline, not refutations.

**Hallucination class audit summary for R5**:
- **1 substantive correction** (CONV-ABS-10): NEW-INV-246's JSON field name is `"changed"`, not `"success"`. Architectural claim correct; literal name wrong.
- **1 staleness correction in CLAUDE.md** (CONV-ABS-11): `cli/project.rs` has 2 subcommands not 4.
- **R5's "+91 invariants" math reconciles.** R5's "+31 entities" math reconciles.
- **R5's NEW-INV-229 (multi-workspace asset HashMap bug) verifies against source** — this is the most important Pass 4 cross-pollination item to preserve.
- **All other R5 bug claims (NEW-INV-178, 179, 219, 261, 263, 295, 300) re-verified or carried as previously verified.**

---

## 3. Sub-pass 2a deepening: structural — entity model per target

### 3.1 T-CLIENT-R6: `api/client.rs` (490 LOC) — full HTTP wrapper deepening

#### E-CLIENT-R6-01 — `JiraClient` 8-field struct + 2-construction-path catalog (lines 17-122)

**Struct fields** (8):
| Field | Type | Source |
|---|---|---|
| `client` | `reqwest::Client` | builder with 30s timeout (line 84) |
| `base_url` | `String` | `config.base_url()?` — may differ from instance_url under OAuth proxy |
| `instance_url` | `String` | profile URL OR `JR_BASE_URL` override (test mode) |
| `auth_header` | `String` | `JR_AUTH_HEADER` env > OAuth Bearer > Basic api-token |
| `verbose` | `bool` | constructor parameter |
| `assets_base_url` | `Option<String>` | derived from `cloud_id` (real mode) or `JR_BASE_URL/jsm/assets` (test mode) |
| `profile_name` | `String` | the active profile name (plumbed for cache calls without `&Config`) |

Two construction paths:
1. `from_config(config, verbose)` — production path; reads keychain, resolves URLs from profile, applies env overrides
2. `new_for_test(base_url, auth_header)` — integration-test path; **NOT gated behind `#[cfg(test)]`** (line 110-111) so integration tests in `tests/` can use it

- **NEW INVARIANT (NEW-INV-307)**: `new_for_test` defaults `profile_name` to `"default"` (line 120). Per-profile cache calls in test code therefore write under `<cache>/v1/default/`. **Test isolation contract**: integration tests sharing a cache root could collide unless they use a tempdir. **Pass 4 testability concern**: a CI test run with a real `~/.cache/jr/v1/default/` could pollute the user's actual cache. (In practice, integration tests use `JR_BASE_URL` to mock the API, so the cache reads happen but writes go through.)
- **NEW INVARIANT (NEW-INV-308)**: The `verbose` flag is **a struct field, NOT a parameter** to each request. Set ONCE at construction. **Architectural pattern**: per-client verbose flag; threaded through to all subsequent operations. The handler sites that read `client.verbose()` (per R5 NEW-INV-226) cannot toggle this at runtime.
- **NEW INVARIANT (NEW-INV-309)**: The `JR_BASE_URL` env override **routes ALL traffic** (instance, assets, OAuth proxy) to the mock server (line 48-50, 87-88). When set, the profile is **NOT consulted** for any URL target (line 42-43 sets `profile = None`). This is a **flat mock-server architecture** — no separate per-API mock endpoints. Integration tests construct a single wiremock and dispatch by path.
- **NEW INVARIANT (NEW-INV-310)**: The `JR_AUTH_HEADER` env override (line 65-66) **completely short-circuits** keychain credential loading. Tests inject a mock `Bearer mock-token` directly. **Pass 4 security concern**: any process with this env-var leaking gets full auth bypass; production binaries should NOT honor this. (Currently no #[cfg] gate — present in production binary.)

#### E-CLIENT-R6-02 — HTTP method surface (lines 137-181, 265-320, 360-436)

**Public HTTP methods** (10 total — R5 said 7):

| Method | URL prefix | Body? | Returns | Purpose |
|---|---|---|---|---|
| `get<T>(path)` | `base_url` | No | `T` (deserialized) | Standard GET |
| `post<T,B>(path, body)` | `base_url` | JSON | `T` | Standard POST returning JSON |
| `put<B>(path, body)` | `base_url` | JSON | `()` | Standard PUT returning 204 |
| `post_no_content<B>(path, body)` | `base_url` | JSON | `()` | POST returning 204 (e.g., transitions) |
| `delete(path)` | `base_url` | No | `()` | Standard DELETE returning 204 |
| `get_from_instance<T>(path)` | `instance_url` | No | `T` | Bypasses OAuth proxy (e.g., GraphQL) |
| `post_to_instance<T,B>(path, body)` | `instance_url` | JSON | `T` | Bypasses OAuth proxy |
| `get_assets<T>(workspace_id, path)` | `assets_base_url` | No | `T` | Assets API (workspace-scoped) |
| `post_assets<T,B>(workspace_id, path, body)` | `assets_base_url` | JSON | `T` | Assets API (workspace-scoped) |
| `request(method, path)` | `base_url` | (caller-built) | `RequestBuilder` | Escape hatch for `jr api` raw command |
| `send_raw(request)` | (caller-supplied URL) | (caller-supplied) | `Response` (raw) | Bypasses error parsing — for `jr api` |

That's **11** distinct public surface methods (counting `request` as a builder + `send_raw` as a request executor). R5 said 7 — undercounted by 4 (missing get_assets, post_assets, request, send_raw).

- **NEW INVARIANT (NEW-INV-311)**: The `request(method, path)` escape-hatch (line 431-436) **bypasses `send()`** entirely. Returns a raw `RequestBuilder` for the caller to drive. Used by `jr api` for arbitrary HTTP method choice. **Architectural pattern**: a `request → send_raw` pair lets the `jr api` command mirror `curl`-style passthrough while reusing the auth header.
- **NEW INVARIANT (NEW-INV-312)**: `send_raw` (line 265-320) **does NOT call `parse_error`** on 4xx/5xx responses (line 315-316 explicit comment). Used by `jr api` so the user can see raw error bodies. ALL other methods route through `send()` which DOES error-parse 4xx/5xx (line 240-242). **Architectural divergence**: `send` and `send_raw` have **mostly identical retry logic** but diverge on error parsing. **Pass 5 convention**: code-duplication between the two retry loops; refactor candidate but each is ~50 lines and the differences are small.
- **NEW INVARIANT (NEW-INV-313)**: `send_raw` includes a `try_clone()` that returns an explicit anyhow::Err if the request body is non-cloneable (line 267-272), unlike `send()` which `.expect()`s cloneability (line 192-193). **Architectural pattern**: `send` expects only JSON bodies (always cloneable); `send_raw` permits arbitrary bodies (potentially streaming) and must handle non-cloneable failures.
- **NEW INVARIANT (NEW-INV-314)**: `assets_base_url` is `Option<String>` because `cloud_id` may be unset — yet 2 methods (`get_assets`, `post_assets`) require it. The error message at line 392-394 directs the user to `jr init`. **Architectural pattern**: the cloud_id requirement is enforced LAZILY at the assets-call site, not at construction.
- **NEW INVARIANT (NEW-INV-315)**: The Assets URL shape is `{assets_base}/workspace/{wid}/v1/{path}` (line 396-401). The `workspace_id` is **URL-encoded** via `urlencoding::encode`. **Defensive against** workspace IDs containing URL-special chars (in practice, workspace IDs are UUIDs, so this is theoretical robustness).

#### E-CLIENT-R6-03 — Auth header construction (lines 60-82)

**Construction order** (per `auth_method`):
1. **Test override**: `JR_AUTH_HEADER` env-var → use as-is, regardless of method
2. **Real mode, `auth_method == "oauth"`**: `load_oauth_tokens(profile)` → `format!("Bearer {access}")`
3. **Real mode, default (`api_token` or anything else)**: `load_api_token()` → base64(email:token) → `Basic {b64}`

The `auth_method` is read from `profile.auth_method.as_deref().unwrap_or("api_token")` (line 60-62).

- **NEW INVARIANT (NEW-INV-316)**: **Default auth_method is `api_token`** (line 62). A profile with no `auth_method` field uses Basic api-token auth. **Pass 3 BC**: legacy single-profile configs default to api-token; OAuth requires explicit `auth_method = "oauth"` in the profile entry.
- **NEW INVARIANT (NEW-INV-317)**: `load_oauth_tokens` returns `(access, _refresh)` — the **refresh token is loaded but discarded** (line 70). The access token is the only credential the client uses. **Architectural reason**: the client doesn't auto-refresh; it forwards 401 to the caller. The refresh token is only loaded to prove the OAuth chain is intact. **Pass 4 minor**: the underscore-bound refresh token is wasted I/O — could be optimized to load only access. But the function returns both atomically, so the cost is one keychain read regardless.
- **NEW INVARIANT (NEW-INV-318)**: api-token base64 encoding uses `base64::engine::general_purpose::STANDARD` (line 77) — RFC 4648 standard (with `+/` and `=` padding), NOT the URL-safe variant. Jira's API expects standard base64 in the Authorization header (per HTTP Basic Auth spec). **Architectural pin**: a refactor to URL-safe base64 would silently break auth.
- **NEW INVARIANT (NEW-INV-319)**: NO auto-refresh from the client side. A 401 response is parsed by `parse_error` and returned as `JrError::NotAuthenticated` (line 337-345). **The auth handlers** (`cli/auth.rs::handle_refresh`) own refresh logic; the client is auth-passive. **Pass 3 BC + Pass 4 UX**: a long-running operation that crosses an OAuth access-token expiry boundary will fail with 401, and the user must run `jr auth refresh` (or wait for the next call to trigger keychain re-load). Per CLAUDE.md gotcha: `refresh_oauth_token` resolves credentials internally — but it's not invoked from `client.send()`.

#### E-CLIENT-R6-04 — 429 retry path with verbose body logging (lines 184-253, 197-204)

**Retry logic** (`send`):
```
for attempt in 0..=MAX_RETRIES { // MAX_RETRIES = 3 → 4 total attempts
    let req = request.try_clone().expect("...");
    let req = req.header("Authorization", &auth_header);

    if verbose {
        eprintln!("[verbose] {METHOD} {URL}");
        if let Some(bytes) = body.as_bytes() {
            eprintln!("[verbose] body: {}", String::from_utf8_lossy(bytes));
        }
    }

    let response = req.send().await?;
    if response.status() == 429 && attempt < MAX_RETRIES {
        let delay = retry_after_secs.unwrap_or(1);
        sleep(delay).await;
        continue;
    }

    if response.status() == 429 {
        eprintln!("warning: rate limited by Jira — gave up after 3 retries...");
    }

    if response.status().is_client_error() || is_server_error() {
        return Err(parse_error(response).await);
    }

    return Ok(response);
}
```

- **NEW INVARIANT (NEW-INV-320)**: **MAX_RETRIES = 3** (line 11), giving **4 total attempts** (initial + 3 retries) on 429. **Architectural decision**: balanced between resilience and "don't hammer a rate-limited server". Hardcoded; not configurable via flag or env.
- **NEW INVARIANT (NEW-INV-321)**: **DEFAULT_RETRY_SECS = 1** (line 14). When the `Retry-After` header is missing or unparseable, jr waits 1 second between attempts. **Architectural pin**: 1 second is a polite minimum; aggressive enough to avoid stalling but slow enough not to saturate. Hardcoded.
- **NEW INVARIANT (NEW-INV-322)**: The 429 retry loop logs to `eprintln!` ONLY in verbose mode (line 220-226). Non-verbose users see retries silently — only the FINAL exhausted-retry warning prints (line 233-237). **Pass 3 BC**: 429 retries are silent by default; verbose mode traces the timing.
- **NEW INVARIANT (NEW-INV-323)**: **NO PII REDACTION in verbose body logging** (line 200-203). The full request body — which CAN contain `email`, `assignee.accountId`, `body` (comments with user-typed content), or any custom field — is dumped to stderr verbatim via `String::from_utf8_lossy`. **Pass 4 security concern**: a `--verbose` user piping stderr to a log file/incident report can leak sensitive payloads. The Authorization header itself is NOT logged (only path and body). **Architectural intent**: verbose is a developer/operator debug aid; users are expected to know they're enabling diagnostic output.
- **NEW INVARIANT (NEW-INV-324)**: `try_clone()` is `.expect()`d (line 192-193) — assumes JSON body always cloneable. **Architectural assumption**: all `send()` callers pass JSON bodies (via `.json(body)`); reqwest's JSON body IS always cloneable. A hypothetical refactor introducing a streaming body to `send()` would panic at runtime. The defensive `try_clone()` returning `Err` in `send_raw` handles that case (per NEW-INV-313).
- **NEW INVARIANT (NEW-INV-325)**: The retry loop uses `tokio::time::sleep` (line 227) — properly async, doesn't block the runtime. Distinct from `std::thread::sleep` which would block. **Architectural correctness pin**: a refactor to `std::thread::sleep` would block the worker thread and could cause deadlock under high concurrency.

#### E-CLIENT-R6-05 — `parse_error` 401 sub-classification (lines 322-348)

```rust
if status == 401 {
    if message.to_ascii_lowercase().contains("scope does not match") {
        return JrError::InsufficientScope { message }.into();
    }
    return JrError::NotAuthenticated.into();
}
JrError::ApiError { status, message }.into()
```

- **NEW INVARIANT (NEW-INV-326)**: **401 is sub-classified into 2 distinct errors** based on body content. `InsufficientScope` fires when the lowercased message contains `"scope does not match"` — the Atlassian API gateway's rejection shape for granular-scoped personal tokens on POST requests (referenced as "issue #185" in the source comment). **Architectural pattern**: 401 is overloaded by the server; jr peeks at the body to classify. **Pass 4 robustness concern**: same shape as NEW-INV-247 — fragile to Atlassian wording change.
- **NEW INVARIANT (NEW-INV-327)**: `to_ascii_lowercase` (line 339) — case-insensitive but ASCII-only. A scope-mismatch message containing non-ASCII characters (unlikely from Atlassian's English-only error catalog) would compare differently. Same Class-3 shape as NEW-INV-238 — defensive ASCII-only comparison.
- **NEW INVARIANT (NEW-INV-328)**: Body read failure at line 332-335 (`response.bytes().await` Err) is captured as a synthetic message string. Network-level body read failure does NOT bubble as a separate error type — it becomes part of the message. **Architectural pattern**: defensive degradation; the user gets a useful error even when the body read itself failed.

#### E-CLIENT-R6-06 — `extract_error_message` 6-precedence chain (lines 448-490)

**Precedence order**:
1. Empty body → `"<empty response body>"`
2. Non-UTF-8 body → `String::from_utf8_lossy` (lossy)
3. Valid JSON `errorMessages` array (non-empty) → `messages.join("; ")`
4. Valid JSON `errors` object (non-empty) → `"field: msg; field2: msg2"` (sorted by key)
5. Valid JSON `message` string field → that string
6. Valid JSON `errorMessage` string field (singular) → that string
7. Fallback → raw body string

- **NEW INVARIANT (NEW-INV-329)**: The `errors` object emits **sorted** field-error pairs (line 477) — deterministic ordering despite serde_json::Map iteration being arbitrary. **Architectural pin**: same-input-same-output for error messages; pinned by `pairs.sort()`. A change to use `BTreeMap` natively or skip the sort would silently break test snapshot stability.
- **NEW INVARIANT (NEW-INV-330)**: The `errorMessage` (singular) precedence is documented (line 484-485) as "seen in some JSM endpoints". **Pass 3 BC**: jr handles both Jira-core (`errorMessages`/`errors`/`message`) and JSM (`errorMessage`) error shapes uniformly. Cross-product reliability.
- **NEW INVARIANT (NEW-INV-331)**: `serde_json::from_str::<Value>` (line 458) tolerates non-JSON bodies — falls through to raw body string return at line 489. So the function NEVER errors; always returns SOME message. **Architectural pattern**: error extraction is itself error-tolerant — never panics, never returns Err.

### 3.2 T-CREATE-R6: `cli/issue/create.rs` (375 LOC) — handle_create + handle_edit

#### E-CREATE-R6-01 — `handle_create` 6-step pipeline (lines 15-201)

```
1. Resolve project_key: --project | config | prompt | error
2. Resolve issue_type: --type | prompt | error
3. Resolve summary: --summary | prompt | error
4. Resolve description: --description-stdin (spawn_blocking) | --description | None
5. Build fields: project, issuetype, summary, description (ADF), priority, labels, team, points, parent, assignee
6. POST /rest/api/3/issue → response.key
7. Construct browse_url
8. Output:
   - JSON: GET full issue back (--output json), inject `url` field; on GET fail emit `{key, url, fetch_error}` fallback
   - Table: print_success "Created issue {key}" + eprintln browse_url
```

- **NEW INVARIANT (NEW-INV-332)**: `handle_create` has **3 interactive prompts** (project/type/summary), each gated by `!no_input` (lines 46-49, 63-66, 73-77). Under `--no-input` and missing flags, errors carry actionable hints (`Use --project`, `Use --type`, `Use --summary`).
- **NEW INVARIANT (NEW-INV-333)**: The fields are built incrementally as `serde_json::Value` mutation (lines 98-143). `fields["description"] = adf_body;`, `fields["priority"] = json!({...});`, etc. **Pass 5 convention**: serde_json mutation is preferred over building a typed `IssueCreateRequest` struct because Jira fields are open-extension (custom fields).
- **NEW INVARIANT (NEW-INV-334)**: Custom fields (story_points, team) are injected by their **field_id key** directly into the fields object (line 124, 129). `fields[&field_id] = json!(team_id)` — the field_id is dynamically resolved per profile (cache or API discovery). **Architectural pattern**: jr is field-id-agnostic at the type level; runtime resolution lets a single binary work across instances with differing custom-field IDs.
- **NEW INVARIANT (NEW-INV-335)**: The post-create JSON output triggers a **follow-up GET** (line 168) so the JSON shape matches `issue view --output json`. On GET failure, the fallback shape is `{key, url, fetch_error: "<msg>"}` (line 188-189). **Pass 3 BC**: scripts using `jq '.fields.status.name'` can detect the fallback by checking for `fetch_error` presence. Same defensive pattern as graceful cache deserialization.
- **NEW INVARIANT (NEW-INV-336)**: The browse URL uses `client.instance_url()` (line 149) — NOT `base_url`. **Critical for OAuth users**: `base_url` may point at `api.atlassian.com/ex/jira/{cloudId}` (the OAuth proxy); the user's browser needs the real `https://yourorg.atlassian.net` URL. R5 NEW-INV-294 is the same architectural pin from a different angle.
- **NEW INVARIANT (NEW-INV-337)**: `eprintln!("{}", browse_url)` (line 196) emits the URL on **stderr**, not stdout. **Pass 3 BC**: scripts piping `jr issue create | tee` get only the success message on stdout; the URL is a human convenience on stderr. Same pattern as `eprintln "Using board ..."` (NEW-INV-289).

#### E-CREATE-R6-02 — `handle_edit` 8-field-type matrix (lines 203-354)

**Field-type matrix** (each branch is independent; multiple flags can combine):

| Flag | Field set | Mode | has_updates? |
|---|---|---|---|
| `--description` (or `--description-stdin`) + `--markdown?` | `description` (ADF) | direct | ✓ |
| `--summary` | `summary` (string) | direct | ✓ |
| `--type` | `issuetype.name` | direct | ✓ |
| `--priority` | `priority.name` | direct | ✓ |
| `--team` | dynamic field_id (UUID or name lookup) | direct | ✓ |
| `--points` | story_points field_id | direct | ✓ |
| `--no-points` | story_points field_id ← `null` | direct | ✓ |
| `--parent` | `parent.key` | direct | ✓ |
| `--label add:foo` / `remove:bar` / `bare` | `update.labels[]` array of `{add: ...}` / `{remove: ...}` | **update endpoint** | early-exit |

- **NEW INVARIANT (NEW-INV-338)**: `--label` uses the `update` endpoint pattern (line 308-316), distinct from all other flags which use the `fields` direct-replacement pattern. `--label add:foo` emits `{"update": {"labels": [{"add": "foo"}]}}` on a `PUT /issue/{key}`. **Pass 3 BC**: labels support add/remove deltas; all other fields are replace-on-edit. **Architectural divergence**: a single `jr issue edit` call can mix `--summary "X"` AND `--label add:foo` — the body becomes `{"fields": {...}, "update": {"labels": [...]}}` (line 311-314).
- **NEW INVARIANT (NEW-INV-339)**: `--label` with neither `add:` nor `remove:` prefix is **treated as add** (line 302-304). Bare `--label foo` → `{"add": "foo"}`. **Pass 3 BC**: legacy CLI pattern that doesn't require users to type `add:` for the common case.
- **NEW INVARIANT (NEW-INV-340)**: `--no-points` (line 282-286) sets the story-points field to `json!(null)` — explicit JSON null. Distinct from "don't include the field in the request" which Jira treats as "no change". **Architectural decision**: explicit `null` triggers a server-side clear. **Pass 3 BC**: `jr issue edit FOO-1 --no-points` clears story points; `jr issue edit FOO-1` (no points-related flag) leaves them unchanged.
- **NEW INVARIANT (NEW-INV-341)**: `has_updates` is a **manually-tracked bool** (line 229) — set to true on each branch that adds a field. The final empty-update guard (line 333-337) bails with the full flag-list error. **Pass 5 convention**: imperative bool tracking; could be replaced with `if fields.as_object().unwrap().is_empty()` but the labels case has its own early-exit branch which complicates that.
- **NEW INVARIANT (NEW-INV-342)**: `--label` early-exit (line 329) means that when ANY label flag is set, the function uses the `PUT` endpoint with combined fields+update body. NON-label-only edits use `client.edit_issue(...)` (line 339) which hits a different code path. **Architectural divergence**: 2 distinct request shapes in one handler.

### 3.3 T-WORKLOG-R6: `cli/worklog.rs` (79 LOC) + `duration.rs` (159 LOC)

#### E-WORKLOG-R6-01 — `handle_add` minimal pipeline (lines 25-48)

```rust
let seconds = duration::parse_duration(dur, 8, 5)?;   // hardcoded 8h/day, 5d/week
let comment = message.map(adf::text_to_adf);          // plain text, NOT markdown
let worklog = client.add_worklog(key, seconds, comment).await?;
```

- **NEW INVARIANT (NEW-INV-343)**: **The 8/5 constants are hardcoded at the call site** (line 32: `parse_duration(dur, 8, 5)?`), NOT extracted into module-level constants. The duration parser is reusable (`hours_per_day`, `days_per_week` parameters), but the worklog handler hardcodes Jira's documented "default work week" assumption. **Architectural decision**: matches Jira Cloud's default worklog conversion (configurable instance-side but jr does not read that config). **Pass 4 correctness concern**: an instance configured for a 6-day work week would have `1d` interpreted as 8h not 8h*1day-per-week. Confirms R5 NEW-INV-87 (R2 carry).
- **NEW INVARIANT (NEW-INV-344)**: **Worklog comment is `text_to_adf` ONLY** (line 33) — there is **NO `--markdown` flag** for worklog. Distinct from `jr issue create --markdown`, `jr issue comment --markdown` which DO support markdown→ADF. **Pass 3 BC**: worklog comments are single-paragraph plain text; users wanting formatted worklog comments must use the Jira UI. **Pass 4 UX gap**: parity with `issue comment --markdown` would be reasonable.
- **NEW INVARIANT (NEW-INV-345)**: **WorklogCommand has only 2 subcommands**: `Add { key, duration, message }` and `List { key }`. There is NO `--started` flag (R5's gap framing was wrong — confirmed by re-reading the destructure at line 16-22). The `started` field IS surfaced in `handle_list` output (line 66) but cannot be set on Add. **Pass 4 UX gap**: Jira's API supports a `started` parameter (per `add_worklog` could potentially pass it); jr doesn't expose it.
- **NEW INVARIANT (NEW-INV-346)**: **Worklog list does NOT paginate** (line 51 — single `list_worklogs` call). Per `api/jira/worklogs.rs` (R2 carry), this returns the full list in one call. Jira's worklog endpoint supports pagination but jr collapses to single-page-fetch. **Pass 4 reliability concern**: an issue with 100+ worklogs would surface only the first page. Confirms R2 list_worklogs non-pagination invariant.

#### E-WORKLOG-R6-02 — `parse_duration` (`duration.rs:5-49`) — character-state-machine

**Algorithm**:
```
total = 0; current = ""; found_any = false
for ch in input.lowercased():
    if ch.is_ascii_digit():
        current.push(ch)
    else:
        if current.is_empty(): bail "Invalid format"
        let num = current.parse::<u64>()?;
        current.clear()
        found_any = true
        match ch:
            'w' → total += num * dpw * hpd * 3600
            'd' → total += num * hpd * 3600
            'h' → total += num * 3600
            'm' → total += num * 60
            _   → bail "Unknown unit"
if !current.is_empty(): bail "Number without unit"
if !found_any: bail "Invalid format"
return total
```

- **NEW INVARIANT (NEW-INV-347)**: The parser is **case-insensitive via lowercase** (line 6: `input.trim().to_lowercase()`). `"2H"`, `"2h"`, `"2H30M"` all parse equivalently. **Pass 3 BC**: lenient unit casing.
- **NEW INVARIANT (NEW-INV-348)**: Number-without-unit rejection (line 38-42) emits a hint suggesting `"30m"` or `"30h"` — actively coaches the user to add a unit. Pinned by test `test_number_without_unit_fails`. **Pass 5 convention**: error messages suggest the most common fix.
- **NEW INVARIANT (NEW-INV-349)**: There is **NO support for fractional units** (e.g., `"1.5h"`). The character loop only accepts ASCII digits (line 16). `"1.5h"` would error at the `.` character with "Unknown duration unit '.'" (line 33). **Pass 4 UX gap**: users typing `1.5h` get a confusing error. Could special-case the `.` or document explicitly.
- **NEW INVARIANT (NEW-INV-350)**: **Order matters in the input string**: `"1w2d3h30m"` parses as 1+2+3+30 (line 91-93 test pin). Per the algorithm, units are summed in the order encountered — but units aren't sorted/canonicalized. `"30m1h"` would parse as 30m + 1h = 90 minutes. **Pass 5 convention**: order-agnostic parsing of multi-unit durations is not enforced; users typing units in non-standard order get the same total. Pinned implicitly by `test_complex` (uses canonical order).
- **NEW INVARIANT (NEW-INV-351)**: u64 overflow is **not explicitly guarded**. `parse_duration("99999999w", 8, 5)` would compute `99_999_999 * 5 * 8 * 3600` which is `1.44e13` — within u64 range, but `parse_duration("99999999999999w", ...)` would silently overflow on multiplication. **Pass 4 robustness gap**: should use `checked_mul` to detect overflow. Pinned implicitly by the proptest range `1u64..100`.
- **NEW INVARIANT (NEW-INV-352)**: `format_duration` (line 52-61) only emits hours and minutes — **no days/weeks output** (line 53-54: `let hours = seconds / 3600; let minutes = (seconds % 3600) / 60;`). A 1-week worklog (144_000 sec = 40h) renders as `"40h"`, not `"1w"` or `"5d"`. **Pass 5 convention**: format is precision-lossless (round-trippable through parse) but not human-friendly for long durations. The `format_roundtrip` proptest only covers seconds < 86400 (1 day) (line 154).

### 3.4 T-TEAM-R6: `cli/team.rs` (120 LOC) — lazy org_id discovery

#### E-TEAM-R6-01 — `handle_list` 3-state cache dispatch (lines 23-50)

```
if refresh:
    teams = fetch_and_cache_teams(...)            // bypass cache, fetch + write
else:
    match read_team_cache:
        Some(cached) → cached.teams
        None         → fetch_and_cache_teams(...)  // cold cache or expired
```

- **NEW INVARIANT (NEW-INV-353)**: `--refresh` bypasses cache READ but still WRITES through (`fetch_and_cache_teams` always writes — line 70). Same shape as R5 NEW-INV-252 (resolutions). **Pass 5 convention**: `--refresh` flag means "force fetch, but warm the cache for next caller".
- **NEW INVARIANT (NEW-INV-354)**: Empty teams list emits `eprintln!("No teams found.")` and returns Ok (line 38-41) — **no error**. JSON consumers calling `--output json` would print no output (the empty-output path bails BEFORE the print_output call). **Pass 4 JSON-output bug**: a JSON consumer expecting `[]` gets nothing on stdout, only stderr text. Distinct from `output::print_output` which would emit `[]` for an empty slice.

#### E-TEAM-R6-02 — `resolve_org_id` 3-state lazy discovery (lines 76-120)

```
if profile.org_id.is_some(): return cached
url = profile.url.ok_or(...)?
hostname = trim_start("https://").trim_start("http://").trim_end("/")
metadata = client.get_org_metadata(hostname).await?      // GraphQL, returns (cloudId, orgId)
config.profiles[name].cloud_id = metadata.cloud_id
config.profiles[name].org_id = metadata.org_id
config.save_global()?
return metadata.org_id
```

- **NEW INVARIANT (NEW-INV-355)**: Lazy org_id discovery **persists BOTH cloud_id AND org_id** (lines 110-117) — even though only org_id is the function's return value. The cloud_id is a useful side-effect: subsequent JiraClient::from_config calls (under OAuth mode) can construct the assets_base_url without re-discovering. **Architectural pattern**: discovery is opportunistic; one GraphQL round-trip primes both fields.
- **NEW INVARIANT (NEW-INV-356)**: The discovery uses `Config::load_with(Some(&config.active_profile_name))?` (line 103) to RELOAD the config before write. **Architectural reason** (per source comment line 99-102): the `--profile` CLI flag (or `JR_PROFILE` env) must NOT get lost between the original load and this write. A naive `config.save_global()` on the in-memory config would persist the in-memory profile but might race with a concurrent edit. The reload-then-write pattern minimizes the race window.
- **NEW INVARIANT (NEW-INV-357)**: `entry().or_insert_with(ProfileConfig::default)` (line 108-109) is used twice — once for cloud_id and once for org_id. **Defensive against** the profile entry being absent in the reloaded config (e.g., if the user manually removed it between load and save). The `default` profile entry is a no-op `ProfileConfig`. **Pass 5 convention**: graceful insertion-on-missing rather than error.
- **NEW INVARIANT (NEW-INV-358)**: The hostname extraction is **byte-level trim**, NOT proper URL parsing (line 91-94). Same shape as init.rs (R5 NEW-INV-305). Both sites have the same Cloud-only assumption.

### 3.5 T-USER-R6: `cli/user.rs` (165 LOC) — user command dispatch

#### E-USER-R6-01 — 3-subcommand surface (lines 10-26)

```rust
UserCommand::Search { query, limit, all }       → handle_search
UserCommand::List { project, limit, all }       → handle_list
UserCommand::View { account_id }                → handle_view
```

- **NEW INVARIANT (NEW-INV-359)**: `cli/user.rs` is a **thin dispatcher** — all heavy lifting in `api/jira/users.rs`. Per CLAUDE.md, this is the canonical "thin wrapper over api/jira/X.rs" pattern.
- **NEW INVARIANT (NEW-INV-360)**: `handle_search` uses `client.search_users(query)` (text search, fuzzy); `handle_list` uses `client.search_assignable_users_by_project("", project)` — **passing empty query string** (line 57, 61). The empty query is the documented Jira API pattern for "all assignable users in this project." **Architectural pattern**: assignable-users API is `query`-based; empty query = no filter.
- **NEW INVARIANT (NEW-INV-361)**: Both `handle_search` and `handle_list` honor `--all` to switch between paginated and full-fetch APIs (`search_users` vs `search_users_all`, `search_assignable_users_by_project` vs `search_assignable_users_by_project_all`). **Pass 5 convention**: `_all` suffix is the project-wide convention for "fetch everything; no offset cap".
- **NEW INVARIANT (NEW-INV-362)**: `--limit` and `--all` are NOT mutually exclusive at clap level — `resolve_effective_limit(limit, all)` (line 35) is the resolution helper. Per `cli/mod.rs`, `resolve_effective_limit` returns `None` when `all` is true (no truncation). **Pass 5 convention**: when both are set, `--all` wins; user gets a deprecation-warn path... actually NEITHER warns (no eprintln in this file).

#### E-USER-R6-02 — `handle_view` 404/400 special-case (lines 70-101)

```rust
match client.get_user(account_id).await {
    Ok(u) => u,
    Err(e) => {
        if let Some(JrError::ApiError { status, .. }) = e.downcast_ref::<JrError>() {
            if *status == 404 || *status == 400 {
                return Err(JrError::UserError(format!("User with accountId '{}' not found.", account_id)).into());
            }
        }
        return Err(e);
    }
}
```

- **NEW INVARIANT (NEW-INV-363)**: **The Jira API returns 400 (NOT 404)** for an unknown accountId in some cases. Per source's test pattern, the handler treats BOTH 400 AND 404 as "not found" — distinct from a true 400 (validation error). **Architectural pattern**: defensive against Jira's idiosyncratic 400 use; downcast-and-rewrite to a user-friendly UserError.
- **NEW INVARIANT (NEW-INV-364)**: The error rewrite uses `e.downcast_ref::<JrError>()` — anyhow downcast into the project's error type. **Pass 5 convention**: error transformation at handler boundaries; the `JrError::ApiError` from `client.parse_error` is downcast and re-classified. Pinned by `is_some_and` matcher pattern.

### 3.6 T-QUEUE-R6: `cli/queue.rs` (323 LOC) — JSM queue handler

#### E-QUEUE-R6-01 — `require_service_desk` gate (line 29)

```rust
let service_desk_id = servicedesks::require_service_desk(client, &project_key).await?;
```

- **NEW INVARIANT (NEW-INV-365)**: **Every queue subcommand goes through `require_service_desk`** (line 29). Distinct from `cli/queue.rs::handle` directly checking the project type. **Architectural pattern**: the gate lives in `api/jsm/servicedesks.rs` (per CLAUDE.md); `cli/queue.rs` delegates. **Pass 4 cross-pollination**: when project is not a JSM project, `require_service_desk` errors with a user-friendly message (per R4 NEW-INV-148 / R5 carry). The gate is invoked BEFORE branch dispatch, so even `jr queue list` errors on non-JSM projects.

#### E-QUEUE-R6-02 — `handle_view` 4-step orchestration (lines 61-111)

```
1. Resolve queue_id: --id direct, or --name → resolve_queue_by_name (partial_match)
2. Apply default limit (DEFAULT_LIMIT, distinct from issue list's default)
3. Step 1: get_queue_issue_keys (preserves queue ordering)
4. Step 2: search_issues with `key IN (FOO-1, FOO-2, ...)` JQL
5. Step 3: reorder_by_queue_position (HashMap-based stable sort)
6. Step 4: format_issue_rows_public (shared with cli/issue/list.rs) + print_output
```

- **NEW INVARIANT (NEW-INV-366)**: **Two-step issue hydration** (lines 87-105) — Jira's `/queue/{id}/issue` endpoint returns ONLY issue keys; full issue data requires a follow-up `search` JQL. **Architectural decision**: separates queue-membership query from issue-detail fetch. **Pass 4 perf**: each `jr queue view` is 2 API round-trips minimum.
- **NEW INVARIANT (NEW-INV-367)**: The hydration JQL is `key IN (FOO-1, FOO-2, BAR-99)` — comma-separated, **no quoting** (line 116-117 source comment: "Issue keys are identifiers in JQL and must NOT be quoted"). **Pass 5 convention**: JQL identifier vs string distinction encoded as a comment (not as a typed wrapper). A copy-paste of `escape_value` here would be wrong.
- **NEW INVARIANT (NEW-INV-368)**: **Queue-position ordering preserved** via `reorder_by_queue_position` (line 121-137). The Jira search API returns issues in JQL-order (not key-IN-order), so jr re-sorts client-side via a `HashMap<&str, usize>` lookup table. **Performance contract**: O(N) sort with O(N) memory. Pinned by tests `reorder_matches_queue_order`, `reorder_with_missing_key_from_search`.
- **NEW INVARIANT (NEW-INV-369)**: Issues missing from the search result (e.g., permission-denied for some keys) are **silently omitted** (line 134: `unwrap_or(usize::MAX)` followed by sort — the missing keys land at the end, but they're not in the input vec to begin with). The reorder function uses `issues.sort_by_key`, NOT a re-build from queue_keys. So missing issues from Jira's search just don't appear. **Pass 4 UX concern**: users would silently see fewer issues than the queue advertises if some keys are permission-restricted.
- **NEW INVARIANT (NEW-INV-370)**: Empty `keys` returns immediately with empty result + correct headers (line 91-96) — does NOT call `search_issues` with an empty JQL (which would be invalid). **Defensive pattern**: zero-cost empty-queue handling.
- **NEW INVARIANT (NEW-INV-371)**: `resolve_queue_by_name` (line 139-186) produces 4 distinct error shapes: `Multiple queues named "X" found (IDs: a, b). Use --id <a> to specify.` (ExactMultiple), `"X" matches multiple queues: ...` (Ambiguous), `No queue matching "X" found. Run jr queue list ...` (None). **Single-substring DOES NOT auto-resolve** (per `single_substring_is_ambiguous` test at line 228-233). **Pass 5 convention**: same as workflow.rs resolve_resolution_by_name (NEW-INV-249) — case-insensitive exact match required.

### 3.7 T-PROJECT-R6: `cli/project.rs` (133 LOC) — only 2 subcommands

#### E-PROJECT-R6-01 — Subcommand surface (lines 17-27)

```rust
ProjectCommand::List { project_type, limit, all } → handle_list
ProjectCommand::Fields                            → handle_fields  // shows ALL of {types, priorities, statuses, cmdb}
```

- **NEW INVARIANT (NEW-INV-372)**: **`cli/project.rs` has ONLY 2 subcommands** — `List` and `Fields`. CLAUDE.md staleness implies "fields, types, priorities, statuses, CMDB fields" are separate commands; they are NOT. Logged as **CONV-ABS-11**.
- **NEW INVARIANT (NEW-INV-373)**: `handle_fields` issues **4 sequential API calls** (lines 76-79): `get_project_issue_types`, `get_priorities`, `get_project_statuses`, and CMDB-fields cache lookup. NOT parallelized via `join_all` (unlike asset enrichment in NEW-INV-228). **Pass 4 perf**: the call chain is sequential; on a slow connection, `jr project fields FOO` is 4× the latency of one call. **Architectural opportunity**: parallelize via `tokio::try_join!`.
- **NEW INVARIANT (NEW-INV-374)**: **CMDB fields fetch silently degrades** on error (line 79: `get_or_fetch_cmdb_fields(client).await.unwrap_or_default()`). **Pass 4 UX concern**: a Jira instance without Assets installed would emit no CMDB fields and no warning — distinct from a real error. The source comment at create.rs:160-163 documents this as "Pre-existing pattern (same as handle_view, handle_list, project): a CMDB discovery error silently degrades to an empty field list." Tracked as a separate cleanup in PR #253. **Pass 4 cross-pollination**: candidate for unifying — emit eprintln once, codebase-wide.
- **NEW INVARIANT (NEW-INV-375)**: **Subtask issue types are tagged inline** in Table mode (line 100-104): `"  - {name} (subtask)"` if `t.subtask == Some(true)`. JSON mode emits the raw `subtask` field. **Pass 3 BC**: Table output annotates subtask types; JSON is structurally raw.
- **NEW INVARIANT (NEW-INV-376)**: The `has_statuses` check (line 111) gates the entire "Statuses by Issue Type" section — if NO issue type has statuses, the section is omitted entirely. **Pass 5 convention**: empty-section elision in Table output; JSON always emits the field (even if empty array).

### 3.8 T-LINKS-R6: `cli/issue/links.rs` (293 LOC)

#### E-LINKS-R6-01 — 3-subcommand surface

```
LinkTypes      → handle_link_types  (no project context, instance-global)
Link           → handle_link        (key1, key2, --type)
Unlink         → handle_unlink      (key1, key2, optional --type filter)
RemoteLink     → handle_remote_link (key, --url, optional --title)
```

- **NEW INVARIANT (NEW-INV-377)**: `link_types` is **NOT cached** (line 16: `client.list_link_types().await?` directly). Each call fetches. **Pass 4 perf concern**: a session running `jr issue link FOO-1 FOO-2 blocks` then `jr issue link FOO-3 FOO-4 blocks` triggers 2 link-type fetches. **Architectural opportunity**: a per-profile link_types cache (similar to resolutions).
- **NEW INVARIANT (NEW-INV-378)**: **Link types are unique per Jira API** — but the code defensively handles `ExactMultiple` (treats as Exact, line 64-66, 137-138). **Pass 5 convention**: defensive over-handling for theoretical Atlassian regressions; same shape as cli/queue.rs queue resolution (line 154).
- **NEW INVARIANT (NEW-INV-379)**: Self-link rejection (line 57-59) uses `eq_ignore_ascii_case`. `jr issue link FOO-1 foo-1 blocks` is rejected. **Pass 3 BC**: case-insensitive issue-key comparison; same behavior as Jira's web UI.

#### E-LINKS-R6-02 — `handle_unlink` link-iteration architecture (lines 116-228)

```
1. Optionally resolve type filter via partial_match
2. get_issue(key1, &[]) → fetch issue with all default fields
3. Iterate issue.fields.issuelinks; filter by:
     - link.outward_issue.key == key2 OR link.inward_issue.key == key2 (case-insensitive)
     - AND (no type filter, OR link.link_type.name == filter (case-insensitive))
4. If matching empty: print "No link found between X and Y" success message; emit JSON {unlinked: false, count: 0}
5. Else: for each match, delete_issue_link(link.id)
6. Print "Removed N link(s) between X and Y"; emit JSON {unlinked: true, count: N}
```

- **NEW INVARIANT (NEW-INV-380)**: **Unlink uses `delete_issue_link(link.id)` per match** — N API calls for N matches (line 209). NOT batched. **Pass 4 perf**: a multi-link unlink scenario is N round-trips. Architecturally constrained — Jira's API has no batch-delete-links endpoint.
- **NEW INVARIANT (NEW-INV-381)**: Unlink is **idempotent in a unique direction**: "no link to remove" is success (`{unlinked: false, count: 0}`, line 197), NOT an error. **Pass 3 BC**: same shape as move/assign idempotence — already-in-target state returns 0. **Pass 4 cross-pollination**: distinct field name (`unlinked`/`count`) from move (`changed`)/assign (`changed`) — refer to CONV-ABS-10 above.
- **NEW INVARIANT (NEW-INV-382)**: The `key1.eq_ignore_ascii_case(&key2)` prevention exists in `handle_link` (line 57) but NOT in `handle_unlink`. Self-unlink isn't prevented at the CLI; the underlying issue scan would simply find no matches and exit success. **Pass 4 UX consistency**: minor — `jr issue unlink FOO-1 FOO-1 type` is a no-op rather than an error. Probably acceptable but inconsistent with link.

#### E-LINKS-R6-03 — `handle_remote_link` URL validation (lines 232-293)

```
1. Trim url, reject empty
2. url::Url::parse(url) → reject malformed
3. Reject non-http/https schemes
4. url = parsed.as_str()                                         // normalized form
5. title = title.trim().filter(|t| !t.is_empty()).unwrap_or(url) // default title = URL
6. create_remote_link(key, url, title)
```

- **NEW INVARIANT (NEW-INV-383)**: **Client-side URL validation** rejects malformed URLs before the API call (line 245-260). Per source comment (line 241-244): "Jira's `/remotelink` endpoint accepts any string for `object.url` without verifying it's a real URL. Creating a link to 'not-a-url' would succeed silently and produce a broken remote link in the Jira UI." **Architectural pattern**: defensive validation at the CLI boundary because the server is permissive. Pass 4 cross-pollination: a similar pattern could close NEW-INV-257 (--internal silent no-op) — defensive client-side checks compensate for server permissiveness.
- **NEW INVARIANT (NEW-INV-384)**: **Only http/https schemes accepted** (line 254-260). `file://`, `ftp://`, `javascript:`, etc. are rejected. **Pass 4 security pin**: defends against a `javascript:` URL being injected as a Jira UI link (would be a stored XSS vector if Jira's UI rendered it as clickable). Architectural decision: positive-list scheme allowance.
- **NEW INVARIANT (NEW-INV-385)**: The URL is **normalized** by re-extracting `parsed.as_str()` (line 264) before sending. **Defensive against** the `url` crate silently normalizing quirks (e.g., tab/newline stripping). The API request, stdout JSON, and Table success message all agree on the normalized form. **Pass 5 convention**: normalize-then-use to ensure round-trip stability.
- **NEW INVARIANT (NEW-INV-386)**: **Title default = URL** (line 267-270) for script-friendly single-flag use. `jr issue remote-link FOO-1 --url https://example.com` is a valid one-flag invocation. **Pass 3 BC**: title is optional; an unset/empty/whitespace-only title falls back to the URL string.

### 3.9 T-HELPERS-R6: `cli/issue/helpers.rs` (813 LOC) — full deepening

#### E-HELPERS-R6-01 — `is_team_uuid` 36-char hex format detector (lines 14-34)

```rust
fn is_team_uuid(s: &str) -> bool {
    if s.len() != 36 { return false; }
    for (i, b) in s.as_bytes().iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => { if *b != b'-' { return false; } }
            _ => { if !b.is_ascii_hexdigit() { return false; } }
        }
    }
    true
}
```

- **NEW INVARIANT (NEW-INV-387)**: The team-UUID detector pinned by **9 unit tests** covering: standard form, uppercase hex, mixed case, wrong length (35/37/0), wrong hyphen position, non-hex char, plausible team name with hyphens, hyphens in wrong positions at 36 chars, non-hex at hex position. **Pass 5 convention**: heavily-tested format predicate; refactor to a regex would lose the explicit per-position validation. The 8/13/18/23 hyphen positions match RFC 4122 §3.
- **NEW INVARIANT (NEW-INV-388)**: **UUID pass-through bypasses cache + name-match entirely** (line 60-65). An agent passing `--team 36885b3c-1bf0-4f85-a357-c5b858c31de4` skips: the team cache load, the name partial-match, the auto-refresh-on-miss. **Performance contract**: agents that already know UUIDs incur ZERO Jira/cache API cost for team resolution. **Pass 3 BC**: UUID format is a script-friendly escape hatch.
- **NEW INVARIANT (NEW-INV-389)**: UUID pass-through emits a verbose-mode eprintln (line 61-63) — pre-cached agents trace correctly. **Architectural pattern**: verbose mode is uniformly applied across the resolver chain.

#### E-HELPERS-R6-02 — `resolve_team_field` auto-refresh-on-miss (lines 36-185)

**State machine**:
```
1. Resolve field_id (config or find_team_field_id)
2. UUID pass-through (returns)
3. Load teams: cache hit (cache_was_fresh=false) | cache miss → fetch+write (cache_was_fresh=true)
4. partial_match
5. If MatchResult::None AND !cache_was_fresh:
     verbose eprintln "team not in cache, refreshing..."
     fresh = fetch_and_cache_teams(...)   (single retry, BOUNDED)
     retry_match = partial_match(fresh)
     fetched_fresh = true
6. Match outcome handling:
     Exact / ExactMultiple / Ambiguous / None (with branching for fetched_fresh advice text)
```

- **NEW INVARIANT (NEW-INV-390)**: **Auto-refresh-on-miss is BOUNDED to a single retry** (line 87-98). The `cache_was_fresh` flag prevents an infinite-refresh loop: if the cache was already fetched fresh in step 3, step 5 doesn't re-fetch. **Architectural correctness pin**: defensive against a typo causing repeated refetches. Pinned by source comment (lines 84-86).
- **NEW INVARIANT (NEW-INV-391)**: The error message in step 6 **branches on `fetched_fresh`** (lines 167-183). If a fresh fetch happened this call, the error advises "checked a fresh team list" (line 172-173). Otherwise, advises `jr team list --refresh` (line 178-180). **Pass 5 convention**: error advice is contextually accurate — never tells a user to refresh when we just did.
- **NEW INVARIANT (NEW-INV-392)**: ExactMultiple (case-variant duplicates like "Backend" vs "backend") in `no_input` mode lists **ALL duplicates with their IDs** (lines 121-131). Distinct from Ambiguous, which lists candidates without IDs (line 145-153). **Pass 3 BC**: ExactMultiple needs id qualification to disambiguate; Ambiguous needs only name precision.

#### E-HELPERS-R6-03 — `compose_extra_fields` 3-position deterministic order (lines 187-204)

```rust
let mut extra: Vec<String> = Vec::new();
if let Some(sp) = config.global.fields.story_points_field_id.as_deref() {
    extra.push(sp.to_string());
}
for (id, _) in cmdb_fields {
    extra.push(id.clone());
}
if let Some(t) = config.global.fields.team_field_id.as_deref() {
    extra.push(t.to_string());
}
extra
```

- **NEW INVARIANT (NEW-INV-393)**: **Order is documented and pinned**: story-points first, CMDB ids preserved in input slice order, team_field last (line 187-204 + test `extra_fields_for_issue_composes_sp_team_and_cmdb` lines 772-803). **Pass 3 BC**: any refactor that changes order breaks the snapshot test. **Architectural pattern**: order is a downstream-affecting contract — JSON consumers may rely on it for column placement. **Pass 5 convention**: full-vector equality assertion (NOT membership) is the right pin.
- **NEW INVARIANT (NEW-INV-394)**: All three sources are **conditionally included** — unset story_points/team OR empty cmdb yields a smaller vec. **Pass 3 BC**: empty extra_fields is valid; the get_issue endpoint just doesn't request those fields.

#### E-HELPERS-R6-04 — `disambiguate_user` 5-state generic resolver (lines 241-341)

**5 states**:
| Input | Outcome |
|---|---|
| Empty | Err(empty_msg) |
| Single user | Ok((id, name)) — no partial_match invoked |
| Exact (only) | Ok by display_name match |
| ExactMultiple (case-variant) | Err with email/account_id list (no_input) OR Select (interactive) |
| Ambiguous | Err with names list (no_input) OR Select (interactive) |
| None | Err(none_msg_fn(all_names)) — caller-provided message function |

- **NEW INVARIANT (NEW-INV-395)**: **`disambiguate_user` is a generic 5-state resolver** with caller-provided `empty_msg` and `none_msg_fn` closures. Three callers: `resolve_user` (JQL `currentUser()` or accountId), `resolve_assignee` (issue-scoped assignable search), `resolve_assignee_by_project` (project-scoped multi-project search). **Pass 5 convention**: closure-injection pattern for caller-specific error messages while sharing the disambiguation algorithm.
- **NEW INVARIANT (NEW-INV-396)**: **Single-user shortcut** (line 252-254) — if only 1 user in the list, return without partial_match. **Performance contract**: avoids a partial_match call on a singleton. **Pass 3 BC edge case**: a user typing `Jane` against a list of `[Alice]` returns Alice (no name comparison). The caller is expected to have pre-filtered. (In practice, `client.search_users("Jane")` already filters by query, so the singleton case is well-formed.)
- **NEW INVARIANT (NEW-INV-397)**: ExactMultiple in `no_input` mode formats with **email when present, account_id when not** (lines 277-286): `"  Jane Doe (jane1@example.com, account: acc-1)"` vs `"  Privacy User (account: acc-2)"`. **Pass 3 BC**: graceful handling of email-privacy users (some accounts hide email).
- **NEW INVARIANT (NEW-INV-398)**: `resolve_user` (JQL fragment path) **filters to active users only** (line 361-364): `users.into_iter().filter(|u| u.active == Some(true))`. Distinct from `resolve_assignee` which uses `search_assignable_users` (server-side filters to assignable, includes inactive who still have access). **Pass 4 cross-pollination**: a user typing `--reporter inactive_jane` would silently get "no active user found" with the misleading hint. **Architectural decision**: JQL `currentUser()` etc. should target active users; assignment can target a wider set.

#### E-HELPERS-R6-05 — `resolve_asset` 4-path resolver (lines 474-592)

**Paths**:
1. Input matches `SCHEMA-NUMBER` pattern → return as-is (no API call)
2. Search via AQL: `Name like "<escaped>"`, limit 25
3. Empty results → user-error
4. Single result → return its object_key
5. Multiple results → partial_match on labels:
   - Exact → return matched key
   - ExactMultiple (label collisions) → list keys for disambiguation
   - Ambiguous → filter results by matched labels, prompt or error
   - None → list all results with keys

- **NEW INVARIANT (NEW-INV-399)**: **Asset key pass-through** (line 480-482) skips: workspace_id fetch, AQL search, partial_match. Same shape as team UUID pass-through (NEW-INV-388). **Performance contract**: agents passing `--asset OBJ-18` incur zero Assets API cost. The validation uses `crate::jql::validate_asset_key`, NOT a regex inline — so the canonical format is centralized in `jql.rs`.
- **NEW INVARIANT (NEW-INV-400)**: AQL escape via `crate::jql::escape_value(input)` (line 486) — **same escape function** used for project keys in JQL. Per `jql.rs`, this handles double-quote escaping. **Architectural pattern**: jql.rs is the single source of truth for value escaping in both JQL and AQL contexts (despite their different syntaxes — both share the double-quote convention).
- **NEW INVARIANT (NEW-INV-401)**: `search_assets` is called with `Some(25)` limit (line 489) — **hardcoded cap**. A `--asset` query matching 30+ assets returns 25, and the disambiguator works on those 25. **Pass 4 UX gap**: users with very common asset names (e.g., `Server`) get truncated to 25 candidates without warning.
- **NEW INVARIANT (NEW-INV-402)**: ExactMultiple uses **object_key as the disambiguator** (lines 514-545): `"  OBJ-18 (Production DB)"`. **Pass 3 BC**: when label collides, the key is the qualifier. Same pattern as team ExactMultiple but with `key` instead of `id`.
- **NEW INVARIANT (NEW-INV-403)**: The catch-all `MatchResult::None(_)` arm (lines 576-590) **shouldn't normally fire** (per source comment line 577-579) because AQL already filters by `Name like`. But defensively handles it with a "Similar results: ... Use the object key directly." message. **Architectural pattern**: defensive empty-state handling for theoretical AQL/partial_match disagreement.

### 3.10 T-JSON-OUTPUT-R6: `cli/issue/json_output.rs` (149 LOC) — response shape catalog

#### E-JSON-OUTPUT-R6-01 — Full response shape inventory

**8 distinct response shapes**:

| Function | Shape | Used by |
|---|---|---|
| `move_response(key, status, changed)` | `{key, status, changed}` | `handle_move` (success + idempotent) |
| `assign_changed_response(key, dn, aid)` | `{key, assignee, assignee_account_id, changed: true}` | `handle_assign` (success) |
| `assign_unchanged_response(key, dn, aid)` | `{key, assignee, assignee_account_id, changed: false}` | `handle_assign` (already-assigned) |
| `unassign_response(key, changed)` | `{key, assignee: null, changed}` | `handle_assign --unassign` |
| `edit_response(key)` | `{key, updated: true}` | `handle_edit` |
| `link_response(key1, key2, link_type)` | `{key1, key2, type, linked: true}` | `handle_link` |
| `unlink_response(unlinked, count)` | `{unlinked, count}` | `handle_unlink` (success + no-match) |
| `remote_link_response(key, id, url, title, self_url)` | `{key, id, url, title, self}` | `handle_remote_link` |

- **NEW INVARIANT (NEW-INV-404)**: **8 distinct write-operation JSON response shapes** are formally enumerated in `json_output.rs`. Each is a thin `serde_json::json!` macro invocation with a fixed key set. **Pass 3 BC**: all 8 shapes are insta-snapshot-tested (lines 89-148). Refactor that changes a key name breaks the snapshot.
- **NEW INVARIANT (NEW-INV-405)**: **Field naming inconsistency**: `move/assign/unassign` use `"changed"`; `edit` uses `"updated"`; `link` uses `"linked"`; `unlink` uses `"unlinked"`. **Pass 5 convention nitpick**: each operation has its own boolean field name. A unified `"changed"` would simplify scripts but break the existing snapshot tests. **CONV-ABS-10 above is the parent retraction**: R5 conflated all 4 distinct field names as `success`. The actual catalog has 4 distinct names.
- **NEW INVARIANT (NEW-INV-406)**: `unassign_response` includes `"assignee": null` explicitly (line 36), NOT omitted. **Pass 3 BC**: scripts can `.assignee == null` to detect unassignment. Distinct from the `assign_*` shapes which include `assignee` as a non-null string.
- **NEW INVARIANT (NEW-INV-407)**: `remote_link_response` uses `"self"` (line 80), NOT `"self_url"` — to match Jira's API response convention (the API field is named `self`). **Architectural pin**: jr's JSON output mirrors Jira's response field names where they exist. Pass 5 convention: not literally a Pass 3 BC because the field name is dictated by Jira; jr just preserves it.

### 3.11 T-RATE-LIMIT-R6: `api/rate_limit.rs` (55 LOC) — Retry-After parsing

#### E-RATE-LIMIT-R6-01 — `RateLimitInfo::from_headers` (lines 14-29)

```rust
let retry_after_secs = headers
    .get("retry-after")
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.trim().parse::<u64>().ok());
let remaining = headers
    .get("x-ratelimit-remaining")
    ...
```

- **NEW INVARIANT (NEW-INV-408)**: **`Retry-After` header is parsed ONLY as integer seconds** (line 18 `.parse::<u64>()`). RFC 7231 §7.1.3 also permits **HTTP-date format** (`Retry-After: Wed, 21 Oct 2015 07:28:00 GMT`). Jira's API may emit either. An HTTP-date Retry-After value would parse as None, falling back to `DEFAULT_RETRY_SECS = 1` (per NEW-INV-321). **Pass 4 reliability concern**: in HTTP-date mode, a server requesting "wait 60 seconds" effectively becomes "wait 1 second" — could trigger faster retry than intended. **Architectural gap**: no chrono parser for the date-form header.
- **NEW INVARIANT (NEW-INV-409)**: Header value is **trimmed** before parsing (`v.trim().parse::<u64>()`, line 18). **Defensive against** server emitting `"5 "` (trailing whitespace). Pinned implicitly.
- **NEW INVARIANT (NEW-INV-410)**: The `X-RateLimit-Remaining` value is parsed but **NOT consumed** by the client (per NEW-INV-322 retry path uses only `retry_after_secs`). The `remaining` field is exposed as a public struct field but no code reads it. **Architectural pattern**: forward-compatibility — observability/telemetry could light up later. **Pass 4 nitpick**: dead code; either wire to verbose log or remove.
- **NEW INVARIANT (NEW-INV-411)**: Header lookup uses **lowercase ASCII names** (`"retry-after"`, `"x-ratelimit-remaining"`). reqwest's `HeaderMap` lookup is case-insensitive (per RFC 7230), so this works. **Pass 5 convention**: lowercase header names match the convention in `reqwest::header::HeaderName::from_static`.

---

## 4. Sub-pass 2b deepening: behavioral

### 4.1 Auth-header construction state diagram

```
                       │ JR_AUTH_HEADER set?
                       │
              ┌──────yes──────┐
              │               │
              ▼               ▼
         use as-is    ┌── auth_method ──┐
                      │                 │
                  "oauth"         (default/unknown)
                      │                 │
                      ▼                 ▼
              load_oauth_tokens   load_api_token
              (returns access     (returns email, token)
               + refresh, drops          │
               refresh)                  ▼
                      │           base64(email:token)
                      ▼                  │
              "Bearer {access}"          │
                                         ▼
                                    "Basic {b64}"
```

### 4.2 client.send() retry loop dataflow

```
attempt = 0
loop:
    req = request.try_clone() (.expect())
    req = req.header("Authorization", &auth_header)

    if verbose: eprintln method+URL+body         (no PII redaction)

    response = req.send()
        Err → JrError::NetworkError(host)
        Ok  → status check

    if 429 AND attempt < 3:
        delay = retry_after_secs OR 1
        if verbose: eprintln "Rate limited, retrying in {delay}s"
        tokio::time::sleep(delay)
        attempt += 1
        continue

    if 429 (last attempt):
        eprintln "warning: rate limited — gave up after 3 retries"

    if 4xx OR 5xx:
        return parse_error(response)              (401 sub-classifies)
    return Ok(response)
```

### 4.3 Team resolver state machine (full)

```
┌──────────────────────────────────────────────────────────┐
│  resolve_team_field(config, client, team_name, no_input) │
└──────────────────────────────┬───────────────────────────┘
                               │
                  field_id from config? ───── no ──→ find_team_field_id
                               │ yes                         │
                               ▼                            (or err)
                       is_team_uuid(team_name)?
                               │
                ┌─── yes ──→ verbose eprintln; return (field_id, team_name)
                no
                ▼
       cache hit? ──── yes ──→ (teams, cache_was_fresh=false)
       no
       ▼
       fetch_and_cache_teams ──→ (teams, cache_was_fresh=true)
       │
       ▼
       partial_match(team_name, names)
       │
   ┌───┴──────────────────────────────────┐
   │                                      │
   None? AND !cache_was_fresh             else
   │                                      │
   ▼                                      ▼
   verbose eprintln; refresh; retry      match outcome:
   match (fresh, retry, retry_fetched)     Exact / ExactMultiple /
   │                                       Ambiguous / None
   ▼
   merge into single match outcome
   │
   ▼
   if None AND fetched_fresh: error "(checked a fresh team list)"
   else if None: error "Run jr team list --refresh"
```

### 4.4 Asset enrichment 3-pass dataflow (re-verified — NEW-INV-229 confirmed)

```
PASS 1 — Extract:
    issue_assets: Vec<Vec<LinkedAsset>>
    to_enrich: HashMap<(wid, oid), ()>     ← dedup key qualified by workspace
    enrich_indices: Vec<(i, j)>

PASS 2 — Resolve concurrently:
    fallback_wid = get_or_fetch_workspace_id (best-effort)
    futures = to_enrich.keys().map(|(wid,oid)| {
        wid_resolved = if wid.is_empty() { fallback } else { wid }
        async { (oid, get_asset(wid_resolved, oid).await) }
    })
    results = join_all(futures)
    resolved: HashMap<oid, (key, name, type)>   ← key DROPS workspace qualifier!
    for (oid, result) in results:
        if Ok(obj): resolved.insert(oid, ...)

PASS 3 — Redistribute:
    for (i, j) in &enrich_indices:
        if let Some(oid) = issue_assets[i][j].id:
            if let Some((k, n, t)) = resolved.get(oid):     ← lookup by oid alone
                issue_assets[i][j].key = k
                ...

BUG: in multi-workspace tenant, two assets sharing oid across workspaces
     get the SAME enrichment data (the second insertion wins).
```

### 4.5 Worklog handler — duration parser flow

```
"1w2d3h30m" → trim+lower → char loop:
    '1' → current="1"
    'w' → total += 1 * 5 * 8 * 3600 = 144_000
    '2' → current="2"
    'd' → total += 2 * 8 * 3600 = 57_600
    '3' → current="3"
    'h' → total += 3 * 3600 = 10_800
    '3' → current="3"
    '0' → current="30"
    'm' → total += 30 * 60 = 1_800
    end-of-input, current="", found_any=true
return 214_200 seconds (= 59h30m, which is 1w 2d 3h 30m at 8h/day, 5d/week)
```

---

## 5. Newly-discovered entities & invariants (NOT in broad / R1 / R2 / R3 / R4 / R5)

### Entities (R6-NN)

- E-CLIENT-R6-01..06 (struct, HTTP method surface, auth header, 429 retry, parse_error, extract_error_message) → 6 entities
- E-CREATE-R6-01..02 (handle_create pipeline, handle_edit field-type matrix) → 2 entities
- E-WORKLOG-R6-01..02 (handle_add minimal, parse_duration state-machine) → 2 entities
- E-TEAM-R6-01..02 (handle_list 3-state cache, resolve_org_id 3-state lazy) → 2 entities
- E-USER-R6-01..02 (3-subcommand surface, handle_view 404/400 special-case) → 2 entities
- E-QUEUE-R6-01..02 (require_service_desk gate, handle_view 4-step orchestration) → 2 entities
- E-PROJECT-R6-01 (subcommand surface — only 2 commands) → 1 entity
- E-LINKS-R6-01..03 (3-subcommand surface, handle_unlink iteration, handle_remote_link URL validation) → 3 entities
- E-HELPERS-R6-01..05 (is_team_uuid, resolve_team_field auto-refresh, compose_extra_fields, disambiguate_user, resolve_asset) → 5 entities
- E-JSON-OUTPUT-R6-01 (8-shape catalog) → 1 entity
- E-RATE-LIMIT-R6-01 (RateLimitInfo::from_headers) → 1 entity

**Total this round: 27 entities**

### Invariants (NEW-INV-307..NEW-INV-411, 105 new this round)

| # | File | Invariant |
|---|---|---|
| NEW-INV-307 | api/client.rs | new_for_test defaults profile_name to "default"; integration tests share v1/default/ unless tempdir-isolated |
| NEW-INV-308 | api/client.rs | verbose is a struct field set ONCE at construction; no runtime toggle |
| NEW-INV-309 | api/client.rs | JR_BASE_URL routes ALL traffic to mock; profile NOT consulted; flat single-server architecture |
| NEW-INV-310 | api/client.rs | JR_AUTH_HEADER short-circuits keychain — no #[cfg(test)] gate; production binary honors it (Pass 4 security) |
| NEW-INV-311 | api/client.rs | request(method, path) escape hatch bypasses send(); used by jr api raw passthrough |
| NEW-INV-312 | api/client.rs | send_raw does NOT call parse_error on 4xx/5xx; raw response returned for jr api |
| NEW-INV-313 | api/client.rs | send_raw try_clone returns explicit anyhow::Err; send() expects cloneable |
| NEW-INV-314 | api/client.rs | assets_base_url is Option<String>; cloud_id requirement enforced LAZILY at assets call |
| NEW-INV-315 | api/client.rs | Assets URL shape: {base}/workspace/{wid}/v1/{path}; wid URL-encoded |
| NEW-INV-316 | api/client.rs | auth_method default = "api_token"; OAuth requires explicit auth_method = "oauth" |
| NEW-INV-317 | api/client.rs | load_oauth_tokens returns (access, _refresh) — refresh discarded; client doesn't auto-refresh |
| NEW-INV-318 | api/client.rs | api-token uses base64::STANDARD (with +/=), NOT URL-safe variant |
| NEW-INV-319 | api/client.rs | NO auto-refresh from client; 401 → JrError::NotAuthenticated; auth handlers own refresh |
| NEW-INV-320 | api/client.rs | MAX_RETRIES = 3 → 4 total attempts on 429; hardcoded |
| NEW-INV-321 | api/client.rs | DEFAULT_RETRY_SECS = 1 when Retry-After missing/unparseable |
| NEW-INV-322 | api/client.rs | 429 retry messages eprintln only in verbose mode; final exhausted-retry warning always prints |
| NEW-INV-323 | api/client.rs | NO PII REDACTION in verbose body logging — full body via from_utf8_lossy (Pass 4 security) |
| NEW-INV-324 | api/client.rs | send() try_clone is .expect()ed — assumes JSON body always cloneable |
| NEW-INV-325 | api/client.rs | Retry sleep uses tokio::time::sleep — async-correct, doesn't block runtime |
| NEW-INV-326 | api/client.rs | 401 sub-classifies into InsufficientScope (scope mismatch wording) vs NotAuthenticated |
| NEW-INV-327 | api/client.rs | InsufficientScope check is to_ascii_lowercase — case-insensitive but ASCII-only |
| NEW-INV-328 | api/client.rs | parse_error body-read failure becomes synthetic message string; never bubbles separately |
| NEW-INV-329 | api/client.rs | extract_error_message errors-object pairs are sorted (line 477) — deterministic output |
| NEW-INV-330 | api/client.rs | extract_error_message handles "errorMessage" (singular, JSM-specific) — cross-product reliability |
| NEW-INV-331 | api/client.rs | extract_error_message NEVER errors — defensive raw-body fallback |
| NEW-INV-332 | cli/issue/create.rs | handle_create has 3 prompts (project/type/summary); each gated by !no_input |
| NEW-INV-333 | cli/issue/create.rs | Fields built incrementally as serde_json::Value mutation — open-extension for custom fields |
| NEW-INV-334 | cli/issue/create.rs | Custom fields injected by dynamically-resolved field_id key — instance-agnostic at type level |
| NEW-INV-335 | cli/issue/create.rs | JSON output triggers follow-up GET to match issue view shape; on fail, fallback {key, url, fetch_error} |
| NEW-INV-336 | cli/issue/create.rs | browse_url uses instance_url (not base_url) — critical for OAuth proxy users |
| NEW-INV-337 | cli/issue/create.rs | browse_url emitted to stderr in Table mode; stdout reserved for parseable success message |
| NEW-INV-338 | cli/issue/create.rs | --label uses update endpoint pattern (add/remove deltas), distinct from all other fields (replace) |
| NEW-INV-339 | cli/issue/create.rs | Bare --label foo treated as add: prefix-omitted (legacy CLI ergonomics) |
| NEW-INV-340 | cli/issue/create.rs | --no-points sets explicit json!(null) — server-side clear vs absent-field no-change |
| NEW-INV-341 | cli/issue/create.rs | has_updates manual bool tracking; final empty-update guard bails with full flag-list error |
| NEW-INV-342 | cli/issue/create.rs | --label early-exit branches to PUT with combined fields+update body; non-label uses edit_issue |
| NEW-INV-343 | cli/worklog.rs | hardcoded 8h/day, 5d/week at parse_duration call site (NOT module-level constants) |
| NEW-INV-344 | cli/worklog.rs | NO --markdown flag for worklog comment — text_to_adf only (Pass 4 UX gap parity) |
| NEW-INV-345 | cli/worklog.rs | WorklogCommand has 2 subcommands (Add, List); NO --started flag (R5 framing was incorrect) |
| NEW-INV-346 | cli/worklog.rs | Worklog list does NOT paginate — single list_worklogs call (R2 carry confirmed) |
| NEW-INV-347 | duration.rs | Case-insensitive via lowercase; "2H30M" valid |
| NEW-INV-348 | duration.rs | Number-without-unit error suggests "30m" or "30h" — actively coaches user |
| NEW-INV-349 | duration.rs | NO support for fractional units; "1.5h" errors with confusing "Unknown duration unit '.'" |
| NEW-INV-350 | duration.rs | Order-agnostic multi-unit input; "30m1h" parses as 90 minutes |
| NEW-INV-351 | duration.rs | u64 overflow not explicitly guarded; should use checked_mul (Pass 4 robustness) |
| NEW-INV-352 | duration.rs | format_duration emits hours+minutes only; NO days/weeks output even for week-long durations |
| NEW-INV-353 | cli/team.rs | --refresh bypasses cache READ but always WRITES through (same shape as resolutions) |
| NEW-INV-354 | cli/team.rs | Empty teams list emits eprintln "No teams found" + Ok return; JSON consumers get no stdout (Pass 4) |
| NEW-INV-355 | cli/team.rs | resolve_org_id persists BOTH cloud_id AND org_id; opportunistic side-effect priming |
| NEW-INV-356 | cli/team.rs | Reload-then-write via Config::load_with(profile_name) — minimizes race window with concurrent edits |
| NEW-INV-357 | cli/team.rs | entry().or_insert_with(default) defends against profile entry being absent in reload |
| NEW-INV-358 | cli/team.rs | Hostname extraction by hand (trim) — same Cloud-only assumption as init.rs (NEW-INV-305) |
| NEW-INV-359 | cli/user.rs | Thin dispatcher pattern — heavy lifting in api/jira/users.rs (canonical CLAUDE.md pattern) |
| NEW-INV-360 | cli/user.rs | handle_list passes empty query "" to assignable-users API — documented "all assignable" pattern |
| NEW-INV-361 | cli/user.rs | --all dispatches to _all-suffixed API variants (search_users_all, etc.) — project-wide convention |
| NEW-INV-362 | cli/user.rs | --limit and --all not mutually-exclusive at clap; resolve_effective_limit prefers --all |
| NEW-INV-363 | cli/user.rs | handle_view treats Jira's 400 AND 404 both as "not found" — defensive against API idiosyncrasy |
| NEW-INV-364 | cli/user.rs | Error rewrite via e.downcast_ref::<JrError>() — anyhow→typed downcast at boundary |
| NEW-INV-365 | cli/queue.rs | Every queue subcommand goes through require_service_desk gate (BEFORE branch dispatch) |
| NEW-INV-366 | cli/queue.rs | Two-step issue hydration: keys-first then JQL search — 2 API round-trips minimum |
| NEW-INV-367 | cli/queue.rs | key IN (FOO-1, FOO-2) JQL — comma-separated, NOT quoted; identifier vs string distinction |
| NEW-INV-368 | cli/queue.rs | reorder_by_queue_position via HashMap<&str, usize> O(N); preserves queue ordering against JQL-order |
| NEW-INV-369 | cli/queue.rs | Issues missing from search (permission-denied) silently omitted — fewer issues than queue advertises |
| NEW-INV-370 | cli/queue.rs | Empty keys returns immediately; never calls search_issues with empty JQL |
| NEW-INV-371 | cli/queue.rs | Queue resolution: 4 distinct error shapes; single-substring DOES NOT auto-resolve (same as resolutions) |
| NEW-INV-372 | cli/project.rs | ONLY 2 subcommands (List, Fields); CLAUDE.md staleness implied 4 (CONV-ABS-11) |
| NEW-INV-373 | cli/project.rs | handle_fields issues 4 SEQUENTIAL API calls; not parallelized (Pass 4 perf opportunity) |
| NEW-INV-374 | cli/project.rs | CMDB fields fetch silently degrades on error (.unwrap_or_default()) — codebase-wide pattern (PR #253) |
| NEW-INV-375 | cli/project.rs | Subtask issue types tagged inline (subtask) in Table; JSON emits raw subtask field |
| NEW-INV-376 | cli/project.rs | Empty-section elision in Table; JSON always emits field even if empty array |
| NEW-INV-377 | cli/issue/links.rs | link_types NOT cached — each call fetches; Pass 4 perf opportunity for per-profile cache |
| NEW-INV-378 | cli/issue/links.rs | Link types defensively handles ExactMultiple (treats as Exact) — over-handling for theoretical Atlassian regression |
| NEW-INV-379 | cli/issue/links.rs | Self-link rejection via eq_ignore_ascii_case; same as Jira UI behavior |
| NEW-INV-380 | cli/issue/links.rs | Unlink uses delete_issue_link per match — N round-trips for N links; no batch endpoint |
| NEW-INV-381 | cli/issue/links.rs | Unlink no-match returns success {unlinked: false, count: 0} — idempotent (parent of CONV-ABS-10) |
| NEW-INV-382 | cli/issue/links.rs | handle_link prevents self-link; handle_unlink does NOT — minor inconsistency |
| NEW-INV-383 | cli/issue/links.rs | Client-side URL validation rejects malformed URLs — defensive against Jira's permissive /remotelink |
| NEW-INV-384 | cli/issue/links.rs | Only http/https schemes accepted — security pin against javascript:/data: injection |
| NEW-INV-385 | cli/issue/links.rs | URL normalized via parsed.as_str() — round-trip stability across API/JSON/Table |
| NEW-INV-386 | cli/issue/links.rs | Title default = URL string for script-friendly single-flag remote-link |
| NEW-INV-387 | cli/issue/helpers.rs | is_team_uuid pinned by 9 unit tests covering edge cases; per-position validation |
| NEW-INV-388 | cli/issue/helpers.rs | UUID pass-through bypasses cache + name-match — zero-cost for agents with known UUIDs |
| NEW-INV-389 | cli/issue/helpers.rs | UUID pass-through emits verbose-mode eprintln; resolver-chain verbose pattern uniform |
| NEW-INV-390 | cli/issue/helpers.rs | Auto-refresh-on-miss BOUNDED to single retry via cache_was_fresh — no infinite-refresh loop |
| NEW-INV-391 | cli/issue/helpers.rs | Error message branches on fetched_fresh — never tells user to refresh when we just did |
| NEW-INV-392 | cli/issue/helpers.rs | Team ExactMultiple in no_input lists ALL duplicates with IDs (case-variant disambiguation) |
| NEW-INV-393 | cli/issue/helpers.rs | compose_extra_fields order: SP first, CMDB preserved, team last; full-vector test pin |
| NEW-INV-394 | cli/issue/helpers.rs | All three sources conditionally included; empty extra_fields valid |
| NEW-INV-395 | cli/issue/helpers.rs | disambiguate_user generic 5-state resolver with closure-injected error messages — 3 callers |
| NEW-INV-396 | cli/issue/helpers.rs | Single-user shortcut returns without partial_match — performance optimization |
| NEW-INV-397 | cli/issue/helpers.rs | ExactMultiple in no_input formats with email when present, account_id when not — privacy-aware |
| NEW-INV-398 | cli/issue/helpers.rs | resolve_user filters to active users only; resolve_assignee uses server-side assignable filter |
| NEW-INV-399 | cli/issue/helpers.rs | Asset key pass-through skips workspace_id+AQL+partial_match — same shape as team UUID |
| NEW-INV-400 | cli/issue/helpers.rs | AQL escape via jql::escape_value — same fn for JQL and AQL contexts (single source of truth) |
| NEW-INV-401 | cli/issue/helpers.rs | search_assets hardcoded to limit=25; Pass 4 UX gap for very common asset names |
| NEW-INV-402 | cli/issue/helpers.rs | Asset ExactMultiple uses object_key as disambiguator; same pattern as team ExactMultiple with id |
| NEW-INV-403 | cli/issue/helpers.rs | None arm of asset partial_match shouldn't normally fire; defensive against AQL/partial_match disagreement |
| NEW-INV-404 | cli/issue/json_output.rs | 8 distinct write-operation JSON response shapes; all insta-snapshot-tested |
| NEW-INV-405 | cli/issue/json_output.rs | 4 distinct boolean field names (changed/updated/linked/unlinked); CONV-ABS-10 retracts R5's unification |
| NEW-INV-406 | cli/issue/json_output.rs | unassign_response includes "assignee": null EXPLICITLY (not omitted) — script detection |
| NEW-INV-407 | cli/issue/json_output.rs | remote_link_response uses "self" (matches Jira API field name); jr preserves API conventions |
| NEW-INV-408 | api/rate_limit.rs | Retry-After parsed ONLY as integer seconds — RFC 7231 HTTP-date format silently fallback to 1s (Pass 4) |
| NEW-INV-409 | api/rate_limit.rs | Header value trimmed before parse — defensive against trailing whitespace |
| NEW-INV-410 | api/rate_limit.rs | X-RateLimit-Remaining parsed but NOT consumed — forward-compat dead code |
| NEW-INV-411 | api/rate_limit.rs | Lowercase ASCII header names; reqwest case-insensitive lookup makes this work |

### Patterns (NEW-PAT-NN, 0 new this round)

No new patterns. The R5 NEW-PAT-01..03 + auto-refresh-on-miss (now codified as NEW-INV-390) and disambiguate-with-closure (NEW-INV-395) are arguably new patterns but I'm classifying them as invariants of specific entities, not standalone patterns.

---

## 6. Retracted / corrected

- **CONV-ABS-10 (CORRECTION)**: R5's NEW-INV-246 said all 3 idempotent paths (move/assign/unassign) emit JSON `{success: false}`. The actual field is `"changed"` for move/assign/unassign. R5 also missed that `edit_response` uses `"updated"`, `link_response` uses `"linked"`, `unlink_response` uses `"unlinked"`. The architectural claim ("a flag distinguishes did-the-work vs already-done") is correct, but the literal field name in R5 was wrong. The actual catalog is 4 distinct boolean field names — see NEW-INV-405.

- **CONV-ABS-11 (CORRECTION)**: CLAUDE.md describes `cli/project.rs` as "project fields (types, priorities, statuses, CMDB fields)" implying multiple subcommands. **Source has only 2 subcommands** (`List`, `Fields`). The "types/priorities/statuses/cmdb" enumeration describes WHAT `Fields` shows — not separate commands. This is staleness in CLAUDE.md, similar to CONV-ABS-4/CONV-ABS-7. Logged as a Pass 5 documentation gap.

- **NO substantive prior-round entity retracted.** All R5 invariant claims (NEW-INV-219, 229, 261, 263, 295, 300) re-verified against source. NEW-INV-229's multi-workspace asset HashMap bug is the most important Pass 4 cross-pollination item; verified at lines 446 + 449 + 456 of `cli/issue/list.rs`.

---

## 7. Delta Summary — what's new vs Round 5

| Category | Items added (delta) |
|---|---|
| `api/client.rs` deepening (struct, HTTP methods, auth, 429 retry, parse_error, extract_error_message) | **+6 entities + 25 invariants** |
| `cli/issue/create.rs` deepening (handle_create + handle_edit) | **+2 entities + 11 invariants** |
| `cli/worklog.rs` + `duration.rs` deepening | **+2 entities + 10 invariants** |
| `cli/team.rs` deepening (lazy org_id, save-then-reload race) | **+2 entities + 6 invariants** |
| `cli/user.rs` deepening (thin dispatcher, 400/404 special-case) | **+2 entities + 6 invariants** |
| `cli/queue.rs` deepening (require_service_desk, two-step hydration) | **+2 entities + 7 invariants** |
| `cli/project.rs` deepening (only 2 subcommands; staleness retraction) | **+1 entity + 5 invariants** |
| `cli/issue/links.rs` deepening (link/unlink/remote-link) | **+3 entities + 10 invariants** |
| `cli/issue/helpers.rs` deepening (UUID, auto-refresh, disambiguate, resolve_asset) | **+5 entities + 17 invariants** |
| `cli/issue/json_output.rs` deepening (8-shape catalog) | **+1 entity + 4 invariants** |
| `api/rate_limit.rs` deepening (Retry-After RFC 7231 gap) | **+1 entity + 4 invariants** |

**Quantitative delta (Round 6)**:
- New entities: **27**
- New invariants: **105** (NEW-INV-307..NEW-INV-411; vs R5's 91, R4's 62, R3's 61, R2's 75, R1's 17)
- New patterns: **0**
- Refined existing: **1 R5 invariant corrected (NEW-INV-246 field name)**, **1 CLAUDE.md staleness logged (CONV-ABS-11)**
- LOC recount discrepancies: **0** (all R6 cited LOCs verified at file level)
- Verified bug claims: 6/6 (NEW-INV-219, 229, 261, 263, 295, 300) re-verified against source
- **NEW VERIFIED BUGS this round**:
  - NEW-INV-310: JR_AUTH_HEADER short-circuits keychain in production binary (Pass 4 security)
  - NEW-INV-323: NO PII REDACTION in verbose body logging (Pass 4 security)
  - NEW-INV-351: u64 overflow in duration parser not guarded (Pass 4 robustness)
  - NEW-INV-354: empty teams emits eprintln + Ok; JSON consumers see no stdout output (Pass 4)
  - NEW-INV-369: queue view silently omits permission-denied issues (Pass 4 UX)
  - NEW-INV-374: CMDB silent-degrade is codebase-wide pattern (Pass 4 — PR #253 cleanup tracker)
  - NEW-INV-401: search_assets hardcoded limit=25 disambiguator truncation (Pass 4 UX)
  - NEW-INV-408: Retry-After RFC 7231 HTTP-date fallback to 1s (Pass 4 reliability)

**Cumulative (broad + R1 + R2 + R3 + R4 + R5 + R6)**:
- Total entities: 51 (broad) + 33 (R1) + 67 (R2) + 31 (R3) + 25 (R4) + 31 (R5) + 27 (R6) = **265**
- Total distinct invariants: **411** (NEW-INV-1..NEW-INV-411)
- Total patterns: NEW-PAT-01..03 = 3

---

## 8. Novelty Assessment

**Novelty: SUBSTANTIVE**

Justification — would removing this round's findings change how you'd spec the system? **Yes**, in at least 9 model-changing ways:

1. **NEW-INV-310 + 323 (security: env-var auth bypass + verbose PII leak)** — Two distinct security findings in `api/client.rs` that any spec describing jr's threat model must enumerate. `JR_AUTH_HEADER` overrides keychain in the production binary (no `#[cfg(test)]` gate); `--verbose` dumps full request bodies including potentially-sensitive content. Pass 4 cross-pollination needed.

2. **NEW-INV-311..313 (request/send_raw escape hatch)** — A previously-undiscovered HTTP method surface area: `request()` returns a raw RequestBuilder, `send_raw` returns a raw Response without 4xx/5xx parsing. R5 said the surface was 7 methods; it's actually 11. Spec for `jr api` raw passthrough must reference this pair.

3. **NEW-INV-326..330 (parse_error 6-precedence chain + 401 sub-classification)** — The error-extraction logic is more sophisticated than any prior round captured. 6 precedence levels (errorMessages array → errors object → message → errorMessage → empty body → raw). 401 sub-classifies into InsufficientScope vs NotAuthenticated based on body wording. Spec for jr's error model must enumerate these.

4. **NEW-INV-338..342 (--label add:/remove: update endpoint + --no-points explicit null)** — The edit handler has 2 distinct request shapes (fields direct-replacement vs update endpoint) which are mixed in a single PUT body. Spec for `jr issue edit` must distinguish these cases.

5. **NEW-INV-365..371 (queue 2-step hydration with permission-silent omission)** — The queue handler's HashMap-based reorder + permission-denied silent omission is a previously-uncatalogued architecture decision. Spec for `jr queue view` must reflect the 2-trip nature and the silent-omission Pass 4 concern.

6. **NEW-INV-373 (project fields 4-sequential-API-call) + NEW-INV-374 (CMDB silent-degrade is codebase-wide)** — `jr project fields` is 4× the latency of one call (sequential, not parallelized). The CMDB silent-degrade pattern is a CODEBASE-WIDE concern referenced as "PR #253 cleanup" in source comments.

7. **NEW-INV-383..386 (URL validation defense for remote-link)** — Client-side URL parsing + scheme allowlist + normalization. Defends against a Jira /remotelink endpoint that accepts any string. Pass 4 security pin (specifically against `javascript:` injection vectors).

8. **NEW-INV-387..403 (full helpers.rs catalog)** — UUID pass-through, auto-refresh-on-miss bounded retry, generic disambiguate_user resolver, asset key pass-through with hardcoded limit=25 truncation. The helpers.rs file (813 LOC, the largest in the codebase besides list.rs) was previously partially-deepened; this round catalogs the resolver chain end-to-end.

9. **NEW-INV-408 (Retry-After RFC 7231 gap) + NEW-INV-351 (u64 overflow) + NEW-INV-410 (X-RateLimit-Remaining dead code)** — Three distinct robustness/correctness findings in the rate-limit/duration layers. Pass 4 cross-pollination targets.

The 105 new invariants this round (vs Round 5's 91, Round 4's 62) actually **continue to accelerate** novelty rather than decay. Pass 2 has NOT yet converged. **SUBSTANTIVE.**

---

## 9. Remaining gaps / next candidate scope (verbatim for Round 7)

### High priority (still under-deepened or partially attacked)

1. **`adf.rs::adf_to_text`** (lines 345-688) — R5 covered markdown_to_adf only. Round 7 should walk:
   - The ListFrame state machine for ordered/bullet rendering.
   - The render_node 12+ match-arm catalogue.
   - The mention/emoji/inlineCard/media* lossy fall-throughs (per R3 NEW-INV-101).

2. **`api/jira/users.rs`** — never attacked at file level. Round 7 should walk:
   - `search_users`, `search_users_all` pagination shape.
   - `search_assignable_users` (issue-scoped) vs `search_assignable_users_by_project` (multiProjectSearch).
   - `_all` vs paginated semantic differences.
   - `get_user`, `get_myself` shapes.

3. **`api/jira/fields.rs`** — `find_team_field_id`, `discover_story_points_field`, CMDB field discovery. Story points field name heuristics. Round 7 should walk discovery logic.

4. **`api/jira/sprints.rs`, `api/jira/boards.rs`, `api/jira/projects.rs`, `api/jira/links.rs`, `api/jira/statuses.rs`** — small files (<150 LOC) but never deepened at file level. Round 7 could batch-cover.

5. **`api/jsm/servicedesks.rs`** — R4 partially covered queues; this file has `require_service_desk` (orchestration) + project meta caching. Round 7 should walk:
   - The require_service_desk gate logic.
   - Project meta cache interaction.
   - The 404-vs-other-error branching.

6. **`partial_match.rs`** — used by 8+ resolvers (workflow, links, queue, helpers); never deepened at file level. Round 7 should walk:
   - 4-state MatchResult enum (Exact/ExactMultiple/Ambiguous/None).
   - The case-insensitive substring algorithm.
   - The disambiguation guarantees.

### Medium priority

7. **`jql.rs`** — `escape_value`, `validate_asset_key`, `aqlFunction()` builder. Mentioned in CLAUDE.md gotcha but never deepened.

8. **`output.rs`** (declared CONVERGED in R5 §9.12) — but `print_output` polymorphism over Table vs JSON, the `&[Vec<String>]` vs `&[T: Serialize]` dual signature, `print_success` formatting. Could re-open if NITPICK targets remain.

9. **`error.rs`** (declared CONVERGED in R5 §9.12) — but JrError variants × exit_code mapping (0/1/2/64/78/130) was never enumerated entry-by-entry. Could re-open.

10. **`api/auth_embedded.rs` + `build.rs`** (declared CONVERGED in R5 §9.12) — XOR obfuscation algorithm, compile-time embedding mechanism. Could re-open if XOR algorithm details matter to spec.

### Low priority — DO NOT REVISIT (CONVERGED)

11. `cli/auth.rs`, `config.rs`, `cache.rs`, `api/auth.rs` — major deepening rounds done; CONVERGED at file level.

12. `cli/issue/list.rs`, `cli/issue/changelog.rs`, `cli/issue/workflow.rs`, `cli/issue/view.rs`, `cli/issue/comments.rs`, `cli/issue/format.rs` — Rounds 3-5 covered. CONVERGED at file level.

13. `api/jira/issues.rs`, `api/jira/teams.rs`, `api/jira/worklogs.rs` — R5 covered. CONVERGED.

### Pass 4 deepening triggered (cross-pollination — DO NOT write into Pass 2)

14-22. NEW-INV-310 (JR_AUTH_HEADER prod-binary), 323 (no PII redaction in verbose), 351 (u64 overflow), 354 (empty teams JSON), 369 (queue silent-omission), 374 (CMDB codebase-wide silent-degrade), 401 (asset hardcoded 25), 405 (4-distinct-bool-names inconsistency), 408 (Retry-After RFC 7231 gap).
23. (Carry from R5): NEW-INV-219, 229, 261, 263, 281, 287, 288, 295, 300.
24. (Carry from R4): NEW-INV-157, 158, 163, 169, 175, 178, 179, 185, 190.
25. (Carry from R3): NEW-INV-101, 105, 119, 127, 143, 148.
26. (Carry from R2): handle_open OAuth bug, list_worklogs non-pagination (now confirmed by NEW-INV-346), hardcoded 8/5 (now confirmed by NEW-INV-343).

---

## 10. State Checkpoint

```yaml
pass: 2
round: 6
status: complete
audit_findings_against_hallucination_classes: 2
new_entities: 27
new_invariants: 105
retracted_findings: 1
files_examined: 12
novelty: SUBSTANTIVE
timestamp: 2026-05-04T23:55:00Z
next_round_targets: |-
  1. adf.rs::adf_to_text — ListFrame state machine, render_node 12+ match arms, mention/emoji/inlineCard/media* lossy fall-throughs
  2. api/jira/users.rs — search_users vs search_assignable_users vs by_project vs single-user; pagination semantics
  3. api/jira/fields.rs — find_team_field_id, discover_story_points_field heuristics, CMDB field discovery
  4. api/jira/sprints.rs, boards.rs, projects.rs, links.rs, statuses.rs — small-file batch cover
  5. api/jsm/servicedesks.rs — require_service_desk orchestration, project meta cache
  6. partial_match.rs — 4-state MatchResult, case-insensitive substring algorithm, disambiguation guarantees
  7. jql.rs — escape_value, validate_asset_key, aqlFunction() builder
  8. output.rs (re-open) — print_output dual signature, print_success
  9. error.rs (re-open) — JrError variant × exit_code mapping enumeration
  10. (CONVERGED file-level) cli/auth.rs, config.rs, cache.rs, api/auth.rs
  11. (CONVERGED file-level) cli/issue/list.rs, changelog.rs, workflow.rs, view.rs, comments.rs, format.rs
  12. (CONVERGED file-level) api/jira/issues.rs, teams.rs, worklogs.rs
  13-22. (Pass 4 cross-pollination — R6) NEW-INV-310, 323, 351, 354, 369, 374, 401, 405, 408
  23. (Pass 4 carry, R5) NEW-INV-219, 229, 261, 263, 281, 287, 288, 295, 300
  24. (Pass 4 carry, R4) NEW-INV-157, 158, 163, 169, 175, 178, 179, 185, 190
  25. (Pass 4 carry, R3) NEW-INV-101, 105, 119, 127, 143, 148
  26. (Pass 4 carry, R2) handle_open OAuth, list_worklogs (confirmed NEW-INV-346), 8h/5d (confirmed NEW-INV-343), asset dedup (confirmed NEW-INV-229)
```
