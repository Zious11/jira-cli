---
document_type: prd-delta
phase: phase-f2-spec-evolution
issue: 384
producer: product-owner
date: 2026-05-19
spec_version_bump: "1.0.0 → 1.1.0"
bump_type: MINOR
status: complete
architecture_delta: none
---

# PRD Delta — Issue #384

## Summary

Issue #384 ("JSM 401 hint surface refinement: distinguish Basic vs OAuth auth, cover require_service_desk path") adds four new behavioral contracts and modifies two existing ones. The fix substrate is a new `JiraClient::is_oauth_auth() -> bool` predicate that gates error-hint dispatch in two call sites.

**No architecture delta is needed.** The predicate is an additive public method on `JiraClient` with no structural change. It reads the existing `self.auth_header` field. No new modules, no new crate dependencies, no dependency-graph changes.

---

## Corrected Design Model (adversary C-01/C-02, pass-2 C-01/C-02/C-03/H-05/H-06)

**The gate is `client.is_oauth_auth()` ALONE. Error variant is irrelevant to the gate.**

The original delta draft assumed: "Basic-auth 401 → `JrError::NotAuthenticated`; OAuth 401 → `JrError::InsufficientScope`." This is FALSE. The 401 handler in `src/api/client.rs` checks the response BODY for "scope does not match" at line 696 BEFORE checking the `Bearer` guard at line 718. A Basic-auth 401 with a scope-mismatch-flavored body lands in `InsufficientScope`; a Basic-auth 401 without it lands in `NotAuthenticated`. So error variant is decided by body content, not auth scheme.

**Pass-2 C-01 — `require_service_desk` has NO existing `map_err`.** `src/api/jsm/servicedesks.rs:117` is `let meta = get_or_fetch_project_meta(client, project_key).await?;` — the `?` propagates raw. BC-X.8.006/007 now explicitly state: the implementation MUST **introduce a NEW `map_err`** on the `get_or_fetch_project_meta(...)` call inside `require_service_desk`. "Introduce", not "modify".

**Pass-2 C-02 — renderer prefix is a colon, not a period.** The `InsufficientScope` Display (`src/error.rs:8-16`) renders `"Insufficient token scope: {message}"` — a COLON. Any spec text or changelog text that cited `"Insufficient token scope. "` (period) has been corrected throughout.

**Pass-2 C-03 — BC-X.8.007 must NOT use `InsufficientScope`.** The `InsufficientScope` Display is a fixed template purpose-built for the issue-#185 POST scenario (hardcoded POST-specific guidance). For an OAuth user failing a `GET /rest/api/3/project/{key}` (a read), all of that guidance is irrelevant noise. Therefore: BC-X.8.007's OAuth `require_service_desk` 401 for BOTH sub-case arms (InsufficientScope arm AND NotAuthenticated arm) rewrites to `JrError::NotAuthenticated { hint }` — fully controllable hint text. The read-side scope guidance (`read:jira-work` + `read:servicedesk-request`) and session-expiry recovery text all go into the `hint` field. NOTE: BC-3.8.015 is unchanged — the JSM POST OAuth `InsufficientScope` arm is genuinely the #185 POST scenario.

**Pass-2 H-05/H-06 — BC-3.8.015 misdescribed pre-#384 OAuth NotAuthenticated arm behavior.** The pre-#384 `handle_jsm_create` `map_err` (`src/cli/issue/create.rs:1988-1995`) ALREADY rewrites the `NotAuthenticated` arm to inject the `write:servicedesk-request` hint for all auth schemes. So for OAuth (`is_oauth_auth() == true`), BOTH the `NotAuthenticated` arm AND the `InsufficientScope` arm produce the `write:servicedesk-request` hint — exactly as pre-#384. There is no auth-scheme-conditional sub-case within the OAuth branch. BC-3.8.015 has been corrected to state this TRUE unchanged behavior.

Consequence: the `map_err` at both sites (`handle_jsm_create`, `require_service_desk`) must:
- On `is_oauth_auth() == false` (Basic): REWRITE any incoming error (whether `NotAuthenticated` or `InsufficientScope`) to `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }` — the rewrite suppresses the misleading `InsufficientScope` path for Basic-auth users.
- On `is_oauth_auth() == true` (OAuth): For the JSM POST (`handle_jsm_create`), preserve the existing pre-#384 rewrite behavior unchanged — both arms produce `write:servicedesk-request`. For `require_service_desk` (BC-X.8.007), rewrite both arms to `NotAuthenticated { hint }` with read-side scope guidance (NOT `InsufficientScope`).

**`is_oauth_auth()` value-space precision:** `JiraClient::load_auth_from_keychain` produces exactly `"Bearer {access_token}"` for OAuth or `"Basic {base64_encoded}"` for Basic/API-token. The `JR_AUTH_HEADER` debug-only test seam (CLAUDE.md SD-002, `#[cfg(debug_assertions)]`) can inject either form. `auth_header` is never empty at call time — the constructor errors via `?` if the keychain yields nothing. `is_oauth_auth()` = `self.auth_header.starts_with("Bearer ")` — the SAME discriminant production code already trusts at `src/api/client.rs:718` and `:802`. This is 100% reliable for the value-space produced by `load_auth_from_keychain`.

---

## New Behavioral Contracts

### BC-3.8.014 — Basic-auth 401 on JSM POST (`handle_jsm_create`) → API-token-expiry hint; `InsufficientScope` rewritten

**File**: `.factory/specs/prd/bc-3-issue-write.md`
**Location**: After BC-3.8.013, before JSON Output Shape Contracts section

Closes finding O-08-01 (CONFIRMED). When `POST /rest/servicedeskapi/request` returns 401 and the active auth is Basic (`client.is_oauth_auth() == false`), the `handle_jsm_create` `map_err` MUST REWRITE any incoming error variant to `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`. The hint field value (body after the `"Not authenticated. "` renderer prefix) is the shared constant `API_TOKEN_EXPIRY_HINT`:

<!-- CANONICAL verbatim block for API_TOKEN_EXPIRY_HINT (adversary-pass-4 F-04). This is the single source of truth for the Basic-auth hint text. All copies in bc-3-issue-write.md §BC-3.8.014 and cross-cutting.md §BC-X.8.006 are duplicates of this block — all copies MUST be updated together; cf. the JR_* doc-fallout pattern in CLAUDE.md. -->
```
Your API token may be expired or revoked. Regenerate it at
https://id.atlassian.com/manage-profile/security/api-tokens
then run `jr auth login` to re-store the credentials.
```

The hint must NOT contain OAuth-scope language. Must NOT say `jr auth refresh`. Tests assert via `contains`, not `==` (the renderer prepends `"Not authenticated. "`).

`API_TOKEN_EXPIRY_HINT` is a shared constant used identically by both the `handle_jsm_create` site (BC-3.8.014) and the `require_service_desk` site (BC-X.8.006) — single source of truth, prevents divergence.

**Acceptance tests**:
- `test_jsm_create_basic_auth_401_surfaces_api_token_hint` (NEW): Basic-auth fixture, generic 401 body → assert stderr `contains` "expired or revoked", `contains` `id.atlassian.com/manage-profile/security/api-tokens`, does NOT contain `write:servicedesk-request`.
- `test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint` (NEW): Basic-auth fixture, "scope does not match" body → assert same API-token hint (pins the `InsufficientScope`→`NotAuthenticated` rewrite path).

> **[adversary-pass-4 F-02 note — SUPERSEDED by adversary-pass-9 C-01]** ~~`test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` is NOT an acceptance test for BC-3.8.014. This test is being switched to a Bearer fixture and becomes a BC-3.8.015 (OAuth-path) pin; a Bearer-fixture test cannot be an acceptance test for BC-3.8.014 (Basic-auth). It is listed under BC-3.8.015 acceptance tests below. Cross-cutting update: the implementing story must update this test's fixture AND its ownership in the test file is implicitly BC-3.8.015 post-update.~~ The fixture switch from Basic to Bearer was unworkable: a Bearer + generic-expiry 401 routes through the refresh coordinator, which deterministically fails with raw anyhow (not a `JrError`) via the `JR_AUTH_HEADER` seam — the `write:servicedesk-request` hint is never injected. **`test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` stayed on its Basic fixture and was repurposed in place as a BC-3.8.014 acceptance pin** (assertions flipped to API-token-expiry hint; renamed by F4). See Required Test Deliverables item #3 for the corrected plan.

---

### BC-3.8.015 — OAuth 401 on JSM POST (`handle_jsm_create`) → existing behavior, now explicitly gated on `is_oauth_auth() == true`

**File**: `.factory/specs/prd/bc-3-issue-write.md`
**Location**: After BC-3.8.014

When the JSM POST returns 401 and `client.is_oauth_auth() == true`, the `map_err` MUST preserve the existing pre-#384 behavior UNCHANGED for both sub-cases:
- `InsufficientScope` (body contains "scope does not match"): hint names `write:servicedesk-request` + `required_scope: Some(...)`.
- `NotAuthenticated` (non-scope-mismatch Bearer 401 post-refresh-failure): the pre-#384 `handle_jsm_create` `map_err` at `src/cli/issue/create.rs:1988-1995` ALREADY rewrites this arm to inject the `write:servicedesk-request` hint for all auth. So for OAuth, BOTH arms produce the `write:servicedesk-request` hint — exactly as pre-#384.

The claim "unchanged" is true: **no OAuth sub-case output is modified by issue #384**. This BC documents what was previously implicit and makes it explicitly gated on `is_oauth_auth() == true`.

> **[adversary-pass-5 F-02 — SUPERSEDED by adversary-pass-9 C-01]** IMPORTANT: The OAuth *output* is genuinely unchanged — but the **pre-#384 regression pin for the Basic-auth 401 scenario FAILED after BC-3.8.014 landed** as expected. That test used a Basic-auth fixture (`JR_AUTH_HEADER=Basic dGVzdDp0ZXN0`) with a generic 401 body — precisely the pre-#384 bug scenario. After BC-3.8.014 landed, Basic+generic-401 produced the API-token hint (not `write:servicedesk-request`), causing the old assertion to fail. F4 repurposed the test in place (fixture stays Basic, assertions flipped to API-token-expiry hint, `write:servicedesk-request` ABSENT) and renamed it to `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`. The original plan of migrating to Bearer was unworkable (see adversary-pass-9 C-01).

**Acceptance tests** (post-adversary-pass-9 C-01 correction — fixture switch to Bearer was unworkable; see adversary-pass-9 §Corrections below):
- `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (REPURPOSED IN PLACE by F4 — fixture stays `Basic dGVzdDp0ZXN0`; assertions assert API-token-expiry hint and `write:servicedesk-request` ABSENT). This is a BC-3.8.014 pin — NOT H-NEW-JSM-RT-003 (see adversary-pass-9 C-01 re-binding below).
- `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (must remain green unmodified — confirmed by reading `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` in `tests/issue_create_jsm.rs`, located under the `// ─── C-01: OAuth InsufficientScope 401 surfaces write:servicedesk-request ────` section banner): uses `JR_AUTH_HEADER=Bearer test-oauth-token` + body `{"errorMessages": ["Unauthorized; scope does not match"]}`.
- H-NEW-JSM-RT-003 is realized AS `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (re-bound by adversary-pass-9 C-01 — see §Revised Holdout Scenario below).

---

### BC-X.8.006 — Basic-auth 401 from `require_service_desk` (cache miss) → API-token-expiry hint; `InsufficientScope` rewritten

**File**: `.factory/specs/prd/cross-cutting.md`
**Location**: After BC-X.8.005, before BC-X.8.007

Closes finding O-08-05 (CONFIRMED). The contract is placed on `require_service_desk` itself (per orchestrator decision 1), so all three JSM callers (`handle_jsm_create`, `jr queue`, `jr requesttype`) benefit without per-caller changes.

This BC fires ONLY on a cache MISS in `get_or_fetch_project_meta` (7-day TTL). Tests MUST force a cache miss.

**Pass-3 C-01 — trigger is ANY 401 from `get_or_fetch_project_meta`, not just the project GET.** `get_or_fetch_project_meta` issues TWO live GETs on a cache miss for a `service_desk`-type project: (1) `GET /rest/api/3/project/{key}` and (2) `GET /rest/servicedeskapi/servicedesk` (via `client.list_service_desks()`). The new `map_err` wraps the entire `get_or_fetch_project_meta(...)` future, so it catches a 401 from EITHER GET. Both are JSM-read operations; the API-token-expiry hint applies uniformly to both.

When any live GET inside `get_or_fetch_project_meta` returns 401 and `client.is_oauth_auth() == false`, the `map_err` inside `require_service_desk` MUST REWRITE any incoming variant to `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }` (same shared constant as BC-3.8.014 — identical hint text, single source of truth).

**Acceptance tests** (`test_require_service_desk_basic_auth_401_surfaces_api_token_hint` — NEW): Basic-auth fixture, cache miss forced, project GET returns 401 → assert stderr `contains` "expired or revoked", `contains` `id.atlassian.com/manage-profile/security/api-tokens`, `contains` `jr auth login`; does NOT contain `write:servicedesk-request`. All three callers benefit from the map_err in require_service_desk; this test pins the `create` caller path; existing `queue`/`requesttype` integration tests cover regression.

**Setup** (for `test_require_service_desk_basic_auth_401_surfaces_api_token_hint`): Isolated `XDG_CACHE_HOME` tempdir (forces cache miss so the live project GET fires); `JR_AUTH_HEADER=Basic <b64>`; mount `GET /rest/api/3/project/{KEY}` returning HTTP 401 with body `{"errorMessages": ["The access token provided is expired, revoked, malformed, or invalid for other reasons."], "errors": {}}`. The project GET is the **canonical pinned 401 path** for this named test. The second GET arm (`GET /rest/servicedeskapi/servicedesk`) is covered **structurally** by the shared `map_err` wrapping the entire `get_or_fetch_project_meta` future — not by a dedicated test. See `cross-cutting.md §BC-X.8.006` for the authoritative Setup block.

---

### BC-X.8.007 — OAuth 401 from `require_service_desk` (cache miss) → read-side scope hint (`read:jira-work` + `read:servicedesk-request`)

**File**: `.factory/specs/prd/cross-cutting.md`
**Location**: After BC-X.8.006

This BC fires ONLY on a cache MISS. **Pass-3 C-01 — trigger is ANY 401 from `get_or_fetch_project_meta`, not just the project GET.** Same dual-GET trigger as BC-X.8.006; the `map_err` wraps the entire future and catches 401 from the project GET OR the service-desk list GET.

When any live GET inside `get_or_fetch_project_meta` returns 401 and `client.is_oauth_auth() == true`, BOTH sub-case arms (InsufficientScope and NotAuthenticated) emit ONE canonical hint via `JrError::NotAuthenticated { hint }`. There is no sub-case difference — a single pinnable hint string for the acceptance test.

**Pass-3 H-03 — hint ordering corrected.** The hint LEADS with session-expiry recovery (`jr auth refresh` / `jr auth login`), then SECOND mentions BYO-OAuth scope guidance (`read:jira-work` + `read:servicedesk-request`). Because jr's default OAuth app already grants these scopes, expiry is the more common cause; the ordering reflects that.

**Pass-3 H-04 — one canonical verbatim hint, not two.** Both arms emit this identical hint (canonical pinnable string):

<!-- CANONICAL verbatim block for BC-X.8.007 OAuth read-scope hint (adversary-pass-4 F-04). This is the single source of truth for the OAuth require_service_desk 401 hint text. All copies in cross-cutting.md §BC-X.8.007 are duplicates of this block — all copies MUST be updated together; cf. the JR_* doc-fallout pattern in CLAUDE.md. -->
```
Your OAuth token may be expired. Run `jr auth refresh` to renew the token, or
`jr auth login` to re-authorize. If using a custom OAuth app, run `jr auth login`
to re-consent with read:jira-work and read:servicedesk-request — `jr auth refresh`
alone cannot add missing scopes (it re-mints with the same granted scope set).
```

Scopes are `read:jira-work` + `read:servicedesk-request` (NOT `write:servicedesk-request` — the write scope applies to the subsequent POST, which `require_service_desk` never reaches).

**Acceptance tests** (`test_require_service_desk_oauth_401_surfaces_read_scope_hint` — NEW): OAuth/Bearer fixture, cache miss forced, project GET returns 401 → assert stderr `contains` `read:jira-work` AND `contains` `read:servicedesk-request`; does NOT contain `write:servicedesk-request`. All three callers benefit; test pins the `create` path; existing queue/requesttype tests cover regression.

**Setup** (for `test_require_service_desk_oauth_401_surfaces_read_scope_hint`): Isolated `XDG_CACHE_HOME` tempdir (forces cache miss so the live project GET fires); `JR_AUTH_HEADER=Bearer test-oauth-token`; mount `GET /rest/api/3/project/{KEY}` returning HTTP 401 with body `{"errorMessages": ["The access token provided is expired, revoked, malformed, or invalid for other reasons."], "errors": {}}`. The project GET is the **canonical pinned 401 path** for this named test. The second GET arm (`GET /rest/servicedeskapi/servicedesk`) is covered **structurally** by the shared `map_err` — not by a dedicated test. The test mounts only the 401 project-GET mock; no request-type resolution mock is needed because the command exits at `require_service_desk`. See `cross-cutting.md §BC-X.8.007` for the authoritative Setup block.

---

## Required Test Deliverables (adversary-pass-4 F-03; adversary-pass-9 C-01 corrected)

> **[adversary-pass-5 LOW]** This list is the CANONICAL copy. The spec-changelog.md §Required Test Deliverables section is a duplicate — update both copies together when either changes.

> **[adversary-pass-9 C-01 CRITICAL design correction]** Item #3 below has been corrected. The prior plan (switch the Basic-auth 401 test from Basic to Bearer + keep `write:servicedesk-request` assertion) was unworkable: a Bearer + generic-expiry 401 routes through the refresh coordinator, which fails with raw anyhow (not a `JrError`) via the `JR_AUTH_HEADER` seam, so the `write:servicedesk-request` hint is never injected. Item #3 is now a **repurpose-in-place** to BC-3.8.014 (fixture stays Basic; assertions flipped to API-token-expiry hint; F4 renamed to `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`). Item #6 is added: the EXISTING `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` test (already green, must remain unmodified) is the BC-3.8.015 pin and the H-NEW-JSM-RT-003 re-binding.

The following named test functions are MANDATORY acceptance-gate deliverables of the implementing story. The implementing story's acceptance criteria MUST include each of the following as a discrete AC — none may be skipped or deferred:

1. **`test_jsm_create_basic_auth_401_surfaces_api_token_hint`** (NEW — BC-3.8.014): Basic-auth fixture, generic 401 body → assert API-token-expiry hint surfaced.
2. **`test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint`** (NEW — BC-3.8.014): Basic-auth fixture, "scope does not match" 401 body → assert API-token-expiry hint surfaced. **HIGHEST regression risk:** this test pins the non-obvious `InsufficientScope`→`NotAuthenticated` rewrite path. It depends on the ordering in `client.rs:696-718` where the body check fires BEFORE the Bearer guard — the Basic-auth "scope does not match" body lands in `InsufficientScope` without the rewrite, exposing misleading OAuth language to Basic-auth users. This test MUST NOT be skipped.
3. **`test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`** (REPURPOSED IN PLACE by F4 — BC-3.8.014 ownership): fixture stays `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0` and generic-expiry 401 body (NOT migrated to Bearer). F4 flipped assertions: (a) asserts `contains` "expired or revoked", (b) asserts `contains` `id.atlassian.com/manage-profile/security/api-tokens`, (c) asserts `contains` `jr auth login`, (d) asserts does NOT contain `write:servicedesk-request`. **WHY:** a Bearer + generic-expiry 401 routes through the refresh coordinator (client.rs:727+), which fails with raw anyhow (not a `JrError`) via the `JR_AUTH_HEADER` seam — the hint is never injected and the Bearer test would be non-deterministic. This is a BC-3.8.014 pin, NOT a BC-3.8.015 pin.
4. **`test_require_service_desk_basic_auth_401_surfaces_api_token_hint`** (NEW — BC-X.8.006): Basic-auth fixture, cache miss forced, project-GET returns generic-expiry 401 → assert API-token-expiry hint from `require_service_desk`. Basic client never enters the refresh path, so any 401 body deterministically yields a `JrError`.
5. **`test_require_service_desk_oauth_401_surfaces_read_scope_hint`** (NEW — BC-X.8.007): OAuth/Bearer fixture, cache miss forced, project-GET returns **scope-mismatch 401** (`{"errorMessages": ["Unauthorized; scope does not match"]}`) → assert `read:jira-work` + `read:servicedesk-request` in hint; does NOT contain `write:servicedesk-request`. **WHY scope-mismatch body required:** a Bearer client with generic-expiry 401 enters the refresh coordinator, fails with raw anyhow (not a `JrError`), and the hint is never injected. Scope-mismatch body short-circuits to `InsufficientScope` at client.rs:696-704 BEFORE the refresh coordinator, deterministically reaching the `map_err`.
6. **`test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`** (EXISTING — BC-3.8.015 pin, H-NEW-JSM-RT-003 re-binding): Bearer fixture, scope-mismatch body → asserts `write:servicedesk-request` + `jr auth refresh` + `jr auth login`. Already green on `develop`. The test's LOGIC, FIXTURE, and ASSERTIONS MUST remain unmodified (they are already correct). F4 SHOULD add an anchor reference to the test's rustdoc comment so the holdout↔test binding is reflected in code: add `// H-NEW-JSM-RT-003 + BC-3.8.015 anchor` (or equivalent) to the existing rustdoc above `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`. A comment-only change has no behavior impact and is explicitly permitted. This is symmetric to the O-01 rustdoc update required for `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (BC-3.8.014 framing).

**Note on scope-mismatch-rewrite ordering:** The `InsufficientScope`→`NotAuthenticated` rewrite (test #2 above) is the highest-regression-risk pin in this delta. It depends on the fact that the body check at `client.rs:696-718` fires BEFORE the Bearer guard — so a Basic-auth 401 with "scope does not match" body arrives as `InsufficientScope` in the `map_err`. Without the rewrite, the Basic-auth path incorrectly surfaces OAuth-scope language. Any future refactor of `client.rs:696-718` ordering must account for this.

> **[adversary-pass-7 O-01, updated by adversary-pass-9 C-01]** The prior O-01 note (F4 must drop "scope-mismatch" wording from section banner when switching to Bearer) no longer applies — the test stayed Basic (no fixture switch) and was renamed to `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`. F4 updated the section banner and rustdoc to reflect the new BC-3.8.014 API-token-expiry framing (since the test is now a Basic-auth API-token pin, not an OAuth scope pin).

This section corresponds to the `check-spec-counts.sh` / `check-bc-cumulative-counts.sh` policy (CLAUDE.md): these tests are the verification anchors for the 4 new BCs, and missing any one of them leaves a BC without a test pin.

---

## Modified Behavioral Contracts

### BC-3.8.001 — `issue create --request-type` dispatch [CROSS-REFERENCE REFRESH ONLY]

Errors cross-reference routes 401 via BC-3.8.009 and additionally names BC-3.8.014/015 inline. Specifically: the Errors field now reads "401 → BC-3.8.009 (auth-conditional: Basic-auth API-token hint → BC-3.8.014; OAuth → BC-3.8.015)". No behavioral change — cross-reference refresh only.

> **[adversary-pass-5 F-06]** Previous summary only said "point at BC-3.8.009"; the actual BC body also names BC-3.8.014/015 directly in the Errors field. Summary aligned with the real BC body.

### BC-3.8.009 — `--on-behalf-of` errors section [UPDATED]

Errors section revised: "401 on the JSM POST is auth-conditional — see BC-3.8.014 (Basic-auth: `is_oauth_auth() == false` → API-token-expiry hint; any `InsufficientScope` rewritten) and BC-3.8.015 (OAuth: `is_oauth_auth() == true` → existing behavior)." The gate is `is_oauth_auth()` alone, not error variant.

### BC-X.3.002 — Universal 401 baseline [UPDATED]

Added JSM auth-conditional footnote: "For JSM dispatch paths, 401 behavior is auth-conditional (gate: `is_oauth_auth()`). Basic-auth: BC-3.8.014 / BC-X.8.006 (any variant → API-token hint). OAuth: BC-3.8.015 / BC-X.8.007 (existing variant behavior). Base contract semantics unchanged for all non-JSM paths."

---

## Revised Holdout Scenario

### H-NEW-JSM-RT-003 [REVISED — per orchestrator decision 2; pass-2 H-02 mock fix; pass-3 C-02/H-06 mock body fix; pass-5 F-01 file-location fix; pass-9 C-01 CRITICAL re-binding]

> **[adversary-pass-9 C-01 CRITICAL correction]** The prior F2 passes 1-8 had H-NEW-JSM-RT-003 realized as `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` with a planned fixture switch from Basic to Bearer + generic-expiry body. This plan was unworkable: a Bearer + generic-expiry 401 routes through the refresh coordinator (client.rs:727+), which deterministically fails with a raw anyhow error (not a `JrError`) via the `JR_AUTH_HEADER` seam — the `write:servicedesk-request` hint is never injected. H-NEW-JSM-RT-003 is NOW RE-BOUND to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (Bearer + scope-mismatch body — the ONLY deterministic OAuth→`JrError`→`write:servicedesk-request` path). `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` stayed Basic and was repurposed to BC-3.8.014 (fixture stayed, assertions flipped to API-token-expiry hint; renamed by F4). Holdout count unchanged (55 total — re-bind, not add/remove).

> **[adversary-pass-5 F-01 — superseded by pass-9 C-01]** The prior F-01 note ("H-NEW-JSM-RT-003 is realized AS `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`") is SUPERSEDED by the pass-9 re-binding above.

H-NEW-JSM-RT-003 is NOW realized AS `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` in `tests/issue_create_jsm.rs`. This test is already GREEN on `develop` UNMODIFIED and MUST remain unmodified. Auth: `JR_AUTH_HEADER=Bearer test-oauth-token`. JSM POST 401 body: scope-mismatch (`{"errorMessages": ["Unauthorized; scope does not match"]}`). Assertions: `write:servicedesk-request`, `jr auth refresh`, `jr auth login`. Uses `mount_project_meta_help`, `mount_service_desk_list`, `mount_request_types_password_reset` helpers, project `HELP`, `--request-type "Password Reset"`, `--summary "Reset my password"`.

The holdout count is UNCHANGED (55 total — per `total_holdouts: 55` frontmatter in `holdout-scenarios.md`, which `scripts/check-spec-counts.sh` validates on every edit).

**Pass-2 H-02 — mock setup was incomplete.** `handle_jsm_create` calls `require_service_desk` FIRST, which issues `GET /rest/api/3/project/{key}` (cache-first; a fresh test = cache miss = live GET). The previous holdout setup mocked only the `servicedesk/requesttype/POST` endpoints and never mocked `GET /rest/api/3/project/{key}`. Without this mock, `require_service_desk` would fail on an unmocked GET before the holdout could reach the JSM POST it was designed to pin. The mock setup in `holdout-scenarios.md` has been updated: step 2 is now `GET /rest/api/3/project/HELPDESK` returning a service-desk-type project; the subsequent steps are renumbered 3-5. The holdout is in `tests/issue_create_jsm.rs` (confirmed extant at line 1309).

**Pass-3 C-02 — project mock body must include `"id"` field AND servicedesk mock must include `"projectId"` field.** `get_or_fetch_project_meta` reads `project.get("id")` at `src/api/jsm/servicedesks.rs:70-74` and then matches the service desk via `.find(|d| d.project_id == project_id)` at line 81. The pass-2 project mock body `{"key":"HELPDESK","projectTypeKey":"service_desk"}` had NO `id` field → `project_id` defaults to `""` → desk match fails → `service_desk_id` is `None` → `require_service_desk` returns `JrError::UserError` "No service desk found" (exit 64) → the holdout never reaches the JSM POST. Corrected mock bodies (verbatim):
- Step 2 (project GET): `{"key": "HELPDESK", "id": "10001", "projectTypeKey": "service_desk"}`
- Step 3 (servicedesk list GET): `{"values": [{"id": "3", "projectId": "10001", "projectName": "Help Desk"}]}` — `"projectId"` is the exact JSON key per `#[serde(rename = "projectId")]` in `src/types/jsm/servicedesk.rs:6`.

> **[SUPERSEDED by Pass-6 F-01/F-02/F-03]** The holdout was regrounded to the real `HELP`/`id:99` fixture in Pass-6. The `HELPDESK`/`10001` bodies above are **historical and no longer authoritative** — they describe an intermediate fictional fixture that was corrected when the holdout was rewritten. After Pass-9 C-01, H-NEW-JSM-RT-003 was further re-bound from `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`. The authoritative mock setup is in `holdout-scenarios.md` §H-NEW-JSM-RT-003.

**Pass-3 H-06 — BC-X.8.006/X.8.007 removed from `BC:` frontmatter list.** This holdout's `require_service_desk` GETs (steps 2 and 3) return HTTP 200. BC-X.8.006/X.8.007 fire only on a 401 from those GETs and are pinned by dedicated integration tests (`test_require_service_desk_basic_auth_401_surfaces_api_token_hint` and `test_require_service_desk_oauth_401_surfaces_read_scope_hint`). A clarifying note has been added to the holdout body in `holdout-scenarios.md`.

The holdout count is UNCHANGED (55 total — per `total_holdouts: 55` frontmatter in `holdout-scenarios.md`, which `scripts/check-spec-counts.sh` validates on every edit).

---

## Interface Contract: `JiraClient::is_oauth_auth()`

The new predicate is a public method on `JiraClient` in `src/api/client.rs`:

```rust
pub fn is_oauth_auth(&self) -> bool {
    self.auth_header.starts_with("Bearer ")
}
```

Returns `true` for OAuth/Bearer auth; `false` for Basic/API-token auth. This is the SINGLE source of truth for auth-scheme detection in error-hint dispatch. No other predicate or ad-hoc check should be introduced.

> **[adversary-pass-5 F-07] `JR_AUTH_HEADER` seam value-space:** `is_oauth_auth()` is **case- and space-sensitive** by design — it uses `starts_with("Bearer ")` (capital B, single space after). Test fixtures using the debug-only `JR_AUTH_HEADER` seam (CLAUDE.md SD-002, `#[cfg(debug_assertions)]`) MUST supply:
> - `"Bearer <token>"` (capital B, single trailing space before token) for the OAuth/Bearer branch
> - `"Basic <b64>"` (capital B, single trailing space before b64 value) for the Basic/API-token branch
>
> A malformed seam value (e.g., `"bearer foo"` in lowercase, or `"Bearer  foo"` with two spaces) silently misclassifies as Basic (`starts_with("Bearer ")` returns `false`), causing the BC-3.8.014 Basic-auth path to fire instead of the BC-3.8.015 OAuth path. This would make `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (a Bearer fixture) assert the wrong hint branch with no obvious failure. All new BC-3.8.015 acceptance tests MUST use exactly `"Bearer test-oauth-token"` (as already established in the `JR_AUTH_HEADER` env line inside `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`).

The implementation also requires a shared constant `API_TOKEN_EXPIRY_HINT: &str` accessible to both the `handle_jsm_create` map_err site and the `require_service_desk` map_err site. **Location: `src/error.rs`** — NOT `src/api/client.rs` or any new module. `src/error.rs` is imported by both the `api` and `cli` layers with no layering inversion, and it keeps "no new modules / no architecture delta" true. Both sites must reference this constant — never duplicate the string.

---

## Count Impact

> **[adversary-pass-5 F-05]** "Before total" column added for end-to-end verifiability. Guard-script output below is the expected post-edit output; authoritative verification is the CI gate `check-bc-cumulative-counts.sh`, not this document.

| File | Before definitional | Before total | After definitional | After total |
|------|--------------------|--------------|--------------------|-------------|
| bc-1-auth-identity.md | 46 | 57 | 46 | 57 |
| bc-2-issue-read.md | 51 | 93 | 51 | 93 |
| bc-3-issue-write.md | 62 | 91 | 64 | 93 |
| bc-4-assets-cmdb.md | 22 | 32 | 22 | 32 |
| bc-5-boards-sprints.md | 17 | 35 | 17 | 35 |
| bc-6-config-cache.md | 29 | 39 | 29 | 39 |
| bc-7-output-render.md | 38 | 84 | 38 | 84 |
| cross-cutting.md | 72 | 138 | 74 | 140 |
| **Grand total** | — | **569** | — | **573** |

Per-file `total_bcs` sum: 57 + 93 + 93 + 32 + 35 + 39 + 84 + 140 = **573** (arithmetically verifiable). Source: `CANONICAL-COUNTS.md` §Per-file total_bcs table.

+4 BCs added (BC-3.8.014, BC-3.8.015, BC-X.8.006, BC-X.8.007). Grand total before: 569. Grand total after: 573.

**Count evidence (H-03):** Both guards exit 0 post-edit. Expected post-edit guard outputs (authoritative verification is `check-bc-cumulative-counts.sh` in CI — not this document):

```
$ bash scripts/check-spec-counts.sh
OK: all spec counts verified.

$ bash scripts/check-bc-cumulative-counts.sh
OK: all cumulative BC counts verified (573 total across 8 files).
```

Authority: `CANONICAL-COUNTS.md` is the source of truth for the 573 grand total. All 8 per-file `total_bcs` values and their sum are listed in the table above and cross-verified against `CANONICAL-COUNTS.md §Sum row`. The two changed files (+2 definitional in bc-3, +2 definitional in cross-cutting) are reflected in those files' frontmatter (`total_bcs: 93` and `total_bcs: 140` respectively) and in BC-INDEX.md. No count drift introduced by adversary-pass-2 corrections (editorial changes only; no new BC headings added).

---

## Architecture Delta

**No architecture delta.** `JiraClient::is_oauth_auth()` is an additive public method on an existing struct. No new modules, no new files, no dependency-graph changes. Architecture documents (`ARCH-INDEX.md`, subsystem definitions) require no updates. The orchestrator's pre-decision stated "no structural change — confirm this in the delta doc" — confirmed.

---

## Spec Version Bump

| Dimension | Value |
|-----------|-------|
| Previous version | 1.0.0 |
| New version | 1.1.0 |
| Bump type | MINOR |
| Rationale | New feature requirements (4 new BCs, 3 modified BCs, 1 revised holdout). No breaking changes, no removed requirements. Corrected design model: gate is `is_oauth_auth()` alone, not error variant; Basic-auth map_err rewrites `InsufficientScope` to `NotAuthenticated` with API-token hint. Adversary-pass-9: corrected OAuth test-pin design — scope-mismatch path is the ONLY deterministic Bearer→`JrError` route via `JR_AUTH_HEADER` seam. |

Changelog entry written to `.factory/spec-changelog.md`.

---

## Appendix: Correction History (Superseded — audit trail only)

> All blocks below are superseded. The live design is fully stated above. This appendix preserves the pass-by-pass correction record for audit traceability — F3/F4 consumers should read the main body only.

---

### Adversary Pass-3 Corrections (2026-05-19)

Applied after third fresh-context adversary pass found 2 CRITICAL + 6 HIGH findings. Design model confirmed SOUND. All findings are completeness/pinning/holdout-correctness defects:

| Finding | Severity | Resolution |
|---------|----------|-----------|
| C-01: BC-X.8.006/007 trigger described as only `GET /rest/api/3/project/{key}`; `get_or_fetch_project_meta` issues a SECOND live GET (`GET /rest/servicedeskapi/servicedesk`) for service_desk-type projects | CRITICAL | Trigger broadened in BC-X.8.006/007 Behavior sections; heading updated from "project GET, cache miss" to "cache miss"; Source fields updated to cite servicedesks.rs:52-85 explicitly |
| C-02: H-NEW-JSM-RT-003 project mock missing `"id"` field → `project_id` defaults to `""` → desk match fails → exit 64 before JSM POST | CRITICAL | Mock bodies corrected verbatim: step 2 includes `"id": "10001"`; step 3 includes `"projectId": "10001"` (exact JSON key per `#[serde(rename = "projectId")]`); justification added inline |
| H-03: BC-X.8.007 hint text leads with BYO-scope sentence; expiry-recovery is buried | HIGH | Hint rewritten to LEAD with `jr auth refresh`/`jr auth login`; BYO-scope sentence is SECONDARY |
| H-04: BC-X.8.007 verbatim hint labeled "InsufficientScope-arm" as if sub-case-specific; both arms must emit identical hint | HIGH | ONE canonical verbatim hint documented; labeled "both arms emit this identical hint"; single pinnable string ensures testability |
| H-05: BC-X.8.006/007 acceptance tests unnamed | HIGH | Named test functions added: `test_require_service_desk_basic_auth_401_surfaces_api_token_hint` (BC-X.8.006) and `test_require_service_desk_oauth_401_surfaces_read_scope_hint` (BC-X.8.007); cross-caller claim clarified honestly (tests pin create path; queue/requesttype covered by existing tests) |
| H-06: H-NEW-JSM-RT-003 `BC:` list included BC-X.8.006/X.8.007 even though GETs return 200 | HIGH | BC-X.8.006/X.8.007 removed from BC list; clarifying note added to holdout body |
| H-07: Changelog "Modified Requirements" table listed H-NEW-JSM-RT-003 alongside BCs | HIGH | H-NEW-JSM-RT-003 moved to new "Revised Holdouts" subsection in changelog |
| H-08: BC-3.8.001 missing from prd-delta "Modified Behavioral Contracts" and changelog "Modified Requirements" | HIGH | BC-3.8.001 added to both, annotated "cross-reference refresh — no behavioral change" |

---

### Adversary Pass-4 Corrections (2026-05-19)

Applied after fourth fresh-context adversary pass found 0 CRITICAL + 1 HIGH + 3 MEDIUM findings. Design model confirmed converged. All findings are pinning/consistency defects:

| Finding | Severity | Resolution |
|---------|----------|-----------|
| F-01: H-NEW-JSM-RT-003 step 4 request-type mock has bare-object body `{id: "5", name: "Get IT Help"}` that does NOT deserialize into the request-type page struct (paginated envelope with `isLastPage` + `values`); name resolution fails before the holdout reaches the JSM POST | HIGH | Step 4 body corrected to `{"isLastPage": true, "values": [{"id": "5", "name": "Get IT Help", "description": "IT support"}]}`; revision note added to holdout body |
| F-02: BC-3.8.014 acceptance-test list in prd-delta included `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` under BC-3.8.014 (Basic-auth) — at the time it was described as a Bearer-fixture test, creating a test-ownership contradiction | MEDIUM | Removed from BC-3.8.014 list; F-02 note added (subsequently superseded by adversary-pass-9 C-01 which confirmed the test stays Basic and is the BC-3.8.014 pin) |
| F-03: Required test deliverables not explicitly enumerated as mandatory ACs; scope-mismatch-rewrite test not flagged as highest-regression-risk | MEDIUM | "Required Test Deliverables" section added to prd-delta; all 5 test functions listed as MANDATORY ACs; scope-mismatch-rewrite test flagged as highest-regression-risk with `client.rs:696-718` ordering dependency |
| F-04: `API_TOKEN_EXPIRY_HINT` and BC-X.8.007 hint text inlined in multiple spec files without canonical-source designation | MEDIUM | prd-delta copies designated CANONICAL; duplicate locations annotated "update together" per JR_* doc-fallout pattern |

---

### Adversary Pass-5 Corrections (2026-05-19)

Applied after fifth fresh-context adversary pass found 0 CRITICAL + 3 HIGH + 4 MEDIUM findings. Design model and all source-code anchors confirmed sound. All findings are test-symbol-accuracy and doc-consistency defects:

| Finding | Severity | Resolution |
|---------|----------|-----------|
| F-01: H-NEW-JSM-RT-003 test-file location contradiction — holdout body says `tests/issue_create_jsm.rs`; spec-changelog §Impact Assessment says `tests/issue_write_holdouts.rs`; ground truth is `tests/issue_create_jsm.rs` (H-NEW-JSM-RT-003 appeared there at the time as annotations on the pre-#384 Basic-auth 401 test); the holdout and the test were the SAME artifact at that point | HIGH | Spec-changelog Impact Assessment corrected; holdout section in prd-delta updated; BC-3.8.015 Trace note added; holdout-scenarios.md §H-NEW-JSM-RT-003 clarified. Subsequently superseded by adversary-pass-9 C-01 which re-bound H-NEW-JSM-RT-003 to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`. |
| F-02: BC-3.8.015 "UNCHANGED" framing misleading — the pre-#384 Basic-auth 401 test (`test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`) used Basic-auth fixture with generic 401 body; post-#384, Basic+generic-401 MUST produce API-token hint per BC-3.8.014 → test WOULD FAIL after BC-3.8.014 lands; Bearer migration described as hard prerequisite | HIGH | BC-3.8.015 section in prd-delta reworded; bc-3-issue-write.md §BC-3.8.015 updated. Subsequently superseded by adversary-pass-9 C-01 which found Bearer migration was unworkable — test was repurposed in place as a BC-3.8.014 pin with assertions flipped. |
| F-03: BC-3.8.015 cites `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` as "must remain green unmodified" without confirming exact `async fn` symbol | HIGH | Verified by reading `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` in `tests/issue_create_jsm.rs` (under the `// ─── C-01: OAuth InsufficientScope 401 surfaces write:servicedesk-request ────` section banner): exact `async fn` name confirmed; uses `Bearer test-oauth-token` + body `{"errorMessages": ["Unauthorized; scope does not match"]}`. (Pass-5 initially annotated with a hardcoded line number; pass-8 F-02 replaced that citation with the symbol-relative anchor form — `async fn` name + section banner — now used in bc-3-issue-write.md and this delta.) |
| F-04: H-NEW-JSM-RT-003 missing cache-miss precondition — BC-X.8.006/007 require forced cache miss; holdout relies on same precondition but leaves it implicit | MEDIUM | Cache-miss precondition added to holdout-scenarios.md §H-NEW-JSM-RT-003 Setup step 0: isolated `XDG_CACHE_HOME` tempdir required |
| F-05: prd-delta Count Impact table omits "Before total" column — +4 BCs cannot be verified end-to-end | MEDIUM | "Before total" column added (bc-3: 91; cross-cutting: 138; grand total before: 569); guard output relabeled "expected post-edit output; authoritative verification is `check-bc-cumulative-counts.sh` in CI" |
| F-06: prd-delta §BC-3.8.001 summary says only "point at BC-3.8.009"; actual BC body also names BC-3.8.014/015 inline | MEDIUM | BC-3.8.001 summary in prd-delta aligned: "Errors cross-reference routes 401 via BC-3.8.009 and additionally names BC-3.8.014/015 inline" |
| F-07: `is_oauth_auth()` Interface Contract missing `JR_AUTH_HEADER` seam value-space — predicate is case- and space-sensitive; malformed seam values silently misclassify as Basic | MEDIUM | Added to Interface Contract section: seam MUST supply `"Bearer <token>"` (capital B, single space) for OAuth branch; `"Basic <b64>"` for Basic branch; malformed values silently misclassify |
| LOW: "Required Test Deliverables" list duplicated near-verbatim in prd-delta and changelog | LOW | prd-delta copy designated canonical; changelog copy annotated "duplicated from prd-delta-384.md §Required Test Deliverables — update together" |

---

### Adversary Pass-6 Corrections (2026-05-19)

Applied after sixth fresh-context adversary pass found 0 CRITICAL + 3 HIGH + 4 MEDIUM findings. All findings relate to H-NEW-JSM-RT-003 describing a fictional fixture and not matching the real bound test, plus supporting clarifications.

| Finding | Severity | Resolution |
|---------|----------|-----------|
| F-01/F-02/F-03 (HIGH): H-NEW-JSM-RT-003 in holdout-scenarios.md accumulated fictional fixture detail across passes 2-4 (project `HELPDESK`/`id:10001`/`"Get IT Help"`/scope-mismatch body) that did not match the real bound test `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`. Title said "scope-mismatch" but the bound test used a generic-expiry body routing via `NotAuthenticated`. Setup prescribed verbatim wiremock JSON for a non-existent fixture. Expected section included `stderr contains "Insufficient token scope"` which is FALSE for a generic-expiry body. Why-hidden text falsely stated the `InsufficientScope` branch is exercised. | HIGH | H-NEW-JSM-RT-003 fully rewritten to faithfully describe the real bound test (at pass-6). Subsequently, adversary-pass-9 C-01 re-bound H-NEW-JSM-RT-003 to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (Bearer + scope-mismatch; the only deterministic OAuth path). The pass-6 rewrite is superseded by the pass-9 re-binding. |
| F-04 (MEDIUM): BC-3.8.015 did not explicitly state which renderer prefix corresponds to which arm, nor that "green for both body variants" refers only to the `write:servicedesk-request` substring not to the renderer prefix. | MEDIUM | BC-3.8.015 in bc-3-issue-write.md updated: explicit renderer prefix per arm (`"Insufficient token scope: "` for InsufficientScope; `"Not authenticated. "` for NotAuthenticated); "green for both body variants" scoped to the `write:servicedesk-request` substring only. |
| F-05 (MEDIUM): Count Impact table cited grand total 573 without per-file breakdown for all 8 BC files — grand total was not arithmetically verifiable. | MEDIUM | Count Impact table expanded: all 8 BC files listed with Before/After definitional and total columns; per-file `total_bcs` sum (57+93+93+32+35+39+84+140=573) stated explicitly as arithmetically verifiable. |
| F-06 (MEDIUM): "55 holdouts" claim was an unanchored magic number with no citation of `total_holdouts:` frontmatter or guard-script validation. | MEDIUM | Holdout count now cites `total_holdouts: 55` frontmatter in holdout-scenarios.md; notes that `scripts/check-spec-counts.sh` validates this on every edit. |
| F-07 (MEDIUM): BC-X.8.007 BYO-OAuth hint sentence said "ensure the app is granted read:jira-work + read:servicedesk-request permissions" but did not state that `jr auth refresh` is insufficient for missing scopes — a BYO user who runs `jr auth refresh` re-mints with the same deficient scope set and gets no benefit. | MEDIUM | BC-X.8.007 hint text and rationale in cross-cutting.md and prd-delta-384.md (CANONICAL copy) updated: BYO sentence now explicitly states `jr auth login` for re-consent/scope acquisition and notes that `jr auth refresh` alone cannot add missing scopes. |

---

### Adversary Pass-7 Corrections (2026-05-19)

Applied after seventh fresh-context adversary pass found 1 MEDIUM + 3 LOW findings. Design model confirmed converged and verified. All findings are fixture-construction completeness and stale-text defects:

| Finding | Severity | Resolution |
|---------|----------|-----------|
| F-01 (MEDIUM): BC-X.8.006 and BC-X.8.007 in cross-cutting.md had named acceptance tests but no fixture-construction Setup block — implementers could not build the test without re-deriving setup from the BC body text. | MEDIUM | Setup blocks added to both BC-X.8.006 and BC-X.8.007 in `cross-cutting.md` (and mirrored summaries in this prd-delta). Each Setup specifies: isolated `XDG_CACHE_HOME` tempdir (cache miss), auth fixture, 401 mock body (verbatim generic-expiry shape), canonical-vs-structural distinction (project GET is the pinned arm; service-desk-list GET is covered structurally by the shared `map_err`). |
| O-01 (LOW): `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` section banner and rustdoc still said "scope-mismatch" but the test used a generic-expiry body routing via `NotAuthenticated`. | LOW | Note added to Required Test Deliverables: F4 implementation MUST update the section banner and rustdoc in `tests/issue_create_jsm.rs` to drop stale "scope-mismatch" wording. (The pass-7 note said "when switching fixture to Bearer" — this fixture switch was subsequently found unworkable by adversary-pass-9 C-01; F4 instead updated the banner/rustdoc to reflect BC-3.8.014 API-token-expiry framing while keeping the Basic fixture.) |
| O-03 (LOW): H-NEW-JSM-RT-003 Setup step 2 in holdout-scenarios.md quoted a partial mock body `{"id": "99", "key": "HELP", "projectTypeKey": "service_desk"}` omitting `"simplified": false` that the real `mount_project_meta_help` helper includes. | LOW | Verbatim JSON dropped from step 2; replaced with behavioral abstraction "via the `mount_project_meta_help` helper (project `HELP`, id `99`, service-desk type)" — the helper is authoritative for the exact body. |
| O-04 (LOW): Pass-3 C-02 block in prd-delta still prescribes `HELPDESK`/`id:10001` mock bodies as if current — superseded by Pass-6 regrounding to `HELP`/`id:99`. | LOW | Pass-3 C-02 block annotated "[SUPERSEDED by Pass-6 F-01/F-02/F-03]" with explicit note that `HELPDESK`/`10001` bodies are historical and no longer authoritative. |

---

### Adversary Pass-9 Corrections (2026-05-19) — CRITICAL Control-Flow Trace

Applied after ninth fresh-context adversary pass traced the actual control flow in `src/api/client.rs` and found the OAuth test-pinning design from passes 1-8 was incorrect. This is a CRITICAL design correction — it changes which tests pin BC-3.8.015 and H-NEW-JSM-RT-003, and corrects BC-X.8.007's Setup body.

**Root cause:** Passes 1-8 assumed a Bearer + generic-expiry 401 body on the JSM POST (and project GET) would reach `handle_jsm_create`'s `map_err` as a `JrError::NotAuthenticated`, enabling the `write:servicedesk-request` hint injection. This assumption was false. Traced actual control flow:
- Line 696-705: scope-mismatch body (`"scope does not match"`) → `JrError::InsufficientScope` IMMEDIATELY, BEFORE Bearer guard and BEFORE refresh coordinator.
- Line 718: `if !auth_header.starts_with("Bearer ")` → `JrError::NotAuthenticated`. Fires ONLY for Basic auth. Bearer client does NOT take this return.
- Line 727+: Bearer client with non-scope-mismatch 401 enters the auto-refresh coordinator. In any `JR_AUTH_HEADER` seam test (no keychain tokens), `refresh_oauth_token_with_url` returns raw `anyhow::bail!` — NOT a `JrError`. The `map_err`'s `e.downcast::<JrError>()` hits `Err(other) => other` arm — hint is never injected.

| Finding | Severity | Resolution |
|---------|----------|-----------|
| C-01: BC-3.8.015 plan to migrate `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` to Bearer + generic-expiry body was IMPOSSIBLE — that fixture routes through refresh coordinator, fails with raw anyhow, `write:servicedesk-request` hint never injected. | CRITICAL | BC-3.8.015 re-specified: testable contract is the scope-mismatch path ONLY (client.rs:696-704 short-circuit → deterministic). Existing `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (green on `develop`, unmodified) is the BC-3.8.015 pin. The generic-OAuth-401 refresh path is pre-existing, unchanged by #384, and OUT of #384's deterministic-test scope — noted explicitly in BC-3.8.015. |
| C-02: The pre-#384 Basic-auth 401 test (now `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`) was planned to be migrated to Bearer. Under #384, Basic + generic-401 produces the BC-3.8.014 API-token-expiry hint — the EXISTING assertions (`write:servicedesk-request`) would break. The plan said "switch to Bearer and keep asserting" but Bearer generic-401 routes through refresh coordinator (IMPOSSIBLE as shown in C-01). | CRITICAL | Test REPURPOSED IN PLACE: fixture stays Basic, assertions flipped from `write:servicedesk-request` to BC-3.8.014 API-token-expiry hint; negative assertion that `write:servicedesk-request` is ABSENT added. F4 renamed the test to `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`. Required Test Deliverables item #3 updated. This test is now a BC-3.8.014 pin. |
| C-03: H-NEW-JSM-RT-003 was bound to the pre-#384 Basic-auth 401 test (now renamed `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`; Bearer + generic-expiry body was impossible — see C-01). | CRITICAL | H-NEW-JSM-RT-003 RE-BOUND to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (Bearer + scope-mismatch body — deterministic). Holdout rewritten to faithfully describe THIS test (project `HELP`, `mount_request_types_password_reset`, `--request-type "Password Reset"`, `--summary "Reset my password"`, scope-mismatch body, asserts `write:servicedesk-request`/`jr auth refresh`/`jr auth login`). Title updated to "scope-mismatch" framing. Holdout count unchanged (55 — re-bind, not add/remove). |
| C-04: BC-X.8.007 Setup specified generic-expiry 401 body for the project-GET mock — same control-flow defect as C-01: Bearer + generic-expiry routes through refresh coordinator, raw anyhow propagates, read-scope hint never injected. | CRITICAL | BC-X.8.007 Setup corrected to **scope-mismatch 401 body** (`{"errorMessages": ["Unauthorized; scope does not match"]}`). WHY explanation added inline: scope-mismatch short-circuits to `InsufficientScope` at client.rs:696-704 BEFORE refresh coordinator → deterministically reaches `map_err`. BC-X.8.006 (Basic) is unaffected — Basic never enters refresh path, any body is deterministic. |

**Correction 5 (F1 decision reversal record):** The original F1 delta analysis §Decision #2 recorded "revise H-NEW-JSM-RT-003 to a Bearer + generic-body fixture." This decision was unworkable due to the refresh-coordinator control-flow issue. Formally reversed in adversary-pass-9 C-01: H-NEW-JSM-RT-003 is now the scope-mismatch Bearer test (existing, green, unmodified). The Basic generic-401 test is a BC-3.8.014 pin with flipped assertions.
