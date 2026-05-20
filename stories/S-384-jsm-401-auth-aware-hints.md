---
document_type: story
story_id: "S-384"
title: "JSM 401 auth-aware error hints: gate handle_jsm_create + require_service_desk on is_oauth_auth() (closes #384)"
wave: feature-followup
status: ready
intent: enhancement
feature_type: backend
scope: standard
issue: 384
points: 5
priority: medium
tdd_mode: strict
estimated_effort: medium
depends_on: []
bc_anchors:
  - BC-3.8.014
  - BC-3.8.015
  - BC-X.8.006
  - BC-X.8.007
  - BC-3.8.001
  - BC-3.8.009
  - BC-X.3.002
holdout_anchors:
  - H-NEW-JSM-RT-003
nfr_anchors: []
adr_refs:
  - ADR-0014
sd_refs:
  - SD-002
parent_phase: F3-story-decomposition
spec_source: ".factory/phase-f2-spec-evolution/prd-delta-384.md"
implementation_strategy: tdd
module_criticality: HIGH  # src/api/client.rs + src/cli/issue/create.rs + src/api/jsm/servicedesks.rs — auth-dispatch and JSM error paths
files_modified:
  - src/api/client.rs        # MODIFIED — new pub fn is_oauth_auth(&self) -> bool
  - src/error.rs             # MODIFIED — new pub const API_TOKEN_EXPIRY_HINT: &str
  - src/cli/issue/create.rs  # MODIFIED — gate handle_jsm_create map_err on is_oauth_auth()
  - src/api/jsm/servicedesks.rs  # MODIFIED — introduce new map_err on get_or_fetch_project_meta call
test_files:
  - tests/issue_create_jsm.rs  # MODIFIED — 2 new tests + repurpose existing + anchor comment
breaking_change: false
# BC status: BC-3.8.014/015/X.8.006/X.8.007 produced in F2 (2026-05-19, 9 passes CONVERGED).
# Modified BCs: BC-3.8.001 (cross-ref refresh), BC-3.8.009 (errors section update), BC-X.3.002 (footnote).
# F3 story produced after F2 convergence confirmed complete.
---

# S-384 — JSM 401 Auth-Aware Error Hints

## Source of Truth

F1 delta analysis: `.factory/phase-f1-delta-analysis/delta-analysis-384.md` (approved).
F2 PRD delta: `.factory/phase-f2-spec-evolution/prd-delta-384.md` (CONVERGED, 9 passes, 2026-05-19).

Authoritative hint text for `API_TOKEN_EXPIRY_HINT` and the OAuth read-scope hint is in the F2 PRD delta
CANONICAL verbatim blocks. Do NOT paraphrase — copy verbatim into the constant bodies.

## Problem Statement

Two 401-hint surfaces in the JSM path produce misleading guidance for Basic-auth (API-token) users:

**Problem 1 (O-08-01 CONFIRMED):** `handle_jsm_create` in `src/cli/issue/create.rs` maps BOTH
`JrError::NotAuthenticated` AND `JrError::InsufficientScope` to the `write:servicedesk-request`
OAuth scope hint, regardless of auth scheme. Basic-auth users do not have OAuth scopes — the hint
is irrelevant and misleading. The pre-fix test (now renamed `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`,
formerly `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`) used `JR_AUTH_HEADER=Basic ...`,
proving the bug was live on `develop`; the test has been repurposed in place with flipped assertions.

**Problem 2 (O-08-05 CONFIRMED):** `require_service_desk` in `src/api/jsm/servicedesks.rs`
propagates 401 from `get_or_fetch_project_meta` (via `?`) with no JSM-specific scope guidance.
A Basic-auth user gets a generic "Run jr auth login" hint; an OAuth user gets nothing that
names the required read-side scopes (`read:jira-work` + `read:servicedesk-request`).

**Shared fix substrate:** Add `pub fn is_oauth_auth(&self) -> bool` to `JiraClient` (reads
`self.auth_header.starts_with("Bearer ")`). Gate error-hint dispatch in two call sites on this
predicate. Add a shared constant `API_TOKEN_EXPIRY_HINT: &str` in `src/error.rs`.

### Key Design Constraint (pass-9 C-01 correction)

A Bearer + generic-expiry 401 body routes through the auto-refresh coordinator
(`src/api/client.rs:727+`), which fails with raw `anyhow::bail!` (not a `JrError`) via the
`JR_AUTH_HEADER` test seam — the `map_err` hint is never injected on that path. The ONLY
deterministic Bearer path that reaches a `map_err` as a `JrError` is the scope-mismatch body
short-circuit at `client.rs:696-704` (`InsufficientScope`). This determines which 401 bodies the
acceptance tests must use for OAuth fixtures (see AC-5 and AC-6).

## Behavioral Contracts

| BC ID | File | Title | Clause(s) |
|-------|------|-------|-----------|
| BC-3.8.014 | `bc-3-issue-write.md` | Basic-auth 401 on JSM POST → API-token-expiry hint; InsufficientScope rewritten | postconditions 1–3 |
| BC-3.8.015 | `bc-3-issue-write.md` | OAuth 401 on JSM POST → existing write:servicedesk-request behavior, now gated on is_oauth_auth()==true | postconditions 1–2 |
| BC-X.8.006 | `cross-cutting.md` | Basic-auth 401 from require_service_desk → API-token-expiry hint; InsufficientScope rewritten | postconditions 1–3 |
| BC-X.8.007 | `cross-cutting.md` | OAuth 401 from require_service_desk → read-side scope hint (read:jira-work + read:servicedesk-request) | postconditions 1–2 |
| BC-3.8.001 | `bc-3-issue-write.md` | issue create --request-type dispatch | Errors field cross-ref (refresh only — no behavioral change) |
| BC-3.8.009 | `bc-3-issue-write.md` | --on-behalf-of errors section | Errors section updated with auth-conditional phrasing |
| BC-X.3.002 | `cross-cutting.md` | Universal 401 baseline | JSM auth-conditional footnote added |

## Acceptance Criteria

### AC-1 — `is_oauth_auth()` predicate: Basic returns false, Bearer returns true
(traces to BC-3.8.014 precondition 1 / BC-3.8.015 precondition 1 / BC-X.8.006 precondition 1 / BC-X.8.007 precondition 1)

`JiraClient::is_oauth_auth()` returns `false` when `self.auth_header` starts with `"Basic "` and
returns `true` when it starts with `"Bearer "`. This is the single predicate gating all hint
dispatch. Implementation: `self.auth_header.starts_with("Bearer ")` — case-sensitive, single space
after `Bearer`. No other predicate or ad-hoc check should be introduced at either call site.

### AC-2 — `API_TOKEN_EXPIRY_HINT` shared constant in `src/error.rs`
(traces to BC-3.8.014 postcondition 1 / BC-X.8.006 postcondition 1)

A constant `pub const API_TOKEN_EXPIRY_HINT: &str` exists in `src/error.rs` with the EXACT
verbatim value (canonical from F2 PRD delta):

```
Your API token may be expired or revoked. Regenerate it at
https://id.atlassian.com/manage-profile/security/api-tokens
then run `jr auth login` to re-store the credentials.
```

Both the `handle_jsm_create` map_err site and the `require_service_desk` map_err site reference
this SAME constant — never a duplicated string literal. The hint must NOT contain `write:servicedesk-request`,
`jr auth refresh`, or any OAuth-scope language.

### AC-3 — `handle_jsm_create` Basic-auth 401 (generic body) → API-token-expiry hint
(traces to BC-3.8.014 postcondition 1 — test: `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`)

When `POST /rest/servicedeskapi/request` returns HTTP 401 with a generic-expiry body and the active
auth is Basic (`JR_AUTH_HEADER=Basic <b64>`), the `handle_jsm_create` map_err MUST rewrite any incoming
variant to `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`. Stderr MUST:
- `contains` "expired or revoked"
- `contains` "id.atlassian.com/manage-profile/security/api-tokens"
- `contains` "jr auth login"
- NOT contain "write:servicedesk-request"

Exit code: 2.

**Test:** `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (repurposed in place by F4 from pre-#384 name; duplicate `test_jsm_create_basic_auth_401_surfaces_api_token_hint` deleted by Copilot dedup — both AC-3 and AC-5 share this test as the generic-expiry pin).
File: `tests/issue_create_jsm.rs`.

### AC-4 — `handle_jsm_create` Basic-auth 401 (scope-mismatch body) → API-token-expiry hint (InsufficientScope rewritten)
(traces to BC-3.8.014 postcondition 2 — test: `test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint`)

**HIGHEST regression risk.** When `POST /rest/servicedeskapi/request` returns HTTP 401 with a
scope-mismatch body (`{"errorMessages": ["Unauthorized; scope does not match"]}`) and the active auth
is Basic, the `handle_jsm_create` map_err MUST REWRITE the incoming `InsufficientScope` variant to
`JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`.

This pins the non-obvious ordering at `client.rs:696-718`: the body check fires BEFORE the Bearer
guard, so a Basic-auth 401 with a scope-mismatch body lands as `InsufficientScope` in the
`map_err` WITHOUT the rewrite, exposing misleading OAuth language to Basic-auth users. The rewrite
suppresses this.

Assertions: same as AC-3 — API-token hint present, `write:servicedesk-request` absent.

**New test required:** `test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint`
File: `tests/issue_create_jsm.rs`. This test MUST NOT be skipped.

### AC-5 — `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` repurposed in place as BC-3.8.014 pin
(traces to BC-3.8.014 postcondition 3)

The EXISTING pre-#384 Basic-auth 401 test in
`tests/issue_create_jsm.rs` has been repurposed in place and renamed to
`test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` with flipped assertions:

- Fixture STAYS: `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0` (do NOT switch to Bearer)
- New assertions: (a) `contains` "expired or revoked", (b) `contains` "id.atlassian.com/manage-profile/security/api-tokens", (c) `contains` "jr auth login", (d) does NOT contain "write:servicedesk-request"
- The test function may be renamed at implementer discretion

Additionally, the stale "scope-mismatch" section banner and rustdoc above this test function MUST be
updated to reflect the new BC-3.8.014 Basic-auth API-token-expiry framing (per F2 PRD delta O-01 note).

**WHY the fixture stays Basic:** A Bearer + generic-expiry 401 routes through the refresh coordinator
(`client.rs:727+`), which fails with raw `anyhow` (not a `JrError`) via `JR_AUTH_HEADER` seam —
the hint is never injected, making the Bearer test non-deterministic.

### AC-6 — OAuth scope-mismatch 401 on JSM POST → write:servicedesk-request hint unchanged
(traces to BC-3.8.015 postconditions 1–2 — test: `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`)

The EXISTING test `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`
in `tests/issue_create_jsm.rs` MUST remain GREEN AND UNMODIFIED. Logic, fixture, and assertions
must not change.

This test IS H-NEW-JSM-RT-003 (re-bound per F2 adversary-pass-9). F4 SHOULD add a comment-only
anchor to the test's rustdoc: `// H-NEW-JSM-RT-003 + BC-3.8.015 anchor` (or equivalent).
A comment-only change has no behavior impact.

Fixture: `JR_AUTH_HEADER=Bearer test-oauth-token`, body `{"errorMessages": ["Unauthorized; scope does not match"]}`.
Assertions: `write:servicedesk-request`, `jr auth refresh`, `jr auth login`.

### AC-7 — `require_service_desk` Basic-auth 401 (cache miss) → API-token-expiry hint
(traces to BC-X.8.006 postconditions 1–3 — test: `test_require_service_desk_basic_auth_401_surfaces_api_token_hint`)

A NEW `map_err` must be INTRODUCED on the `get_or_fetch_project_meta(...)` call inside
`require_service_desk` (`src/api/jsm/servicedesks.rs:117`). "Introduce", not "modify" — there is
currently no `map_err` there; the `?` propagates raw.

When any live GET inside `get_or_fetch_project_meta` returns 401 and `client.is_oauth_auth() == false`:
the map_err MUST rewrite any incoming variant to `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`.

Test setup: isolated `XDG_CACHE_HOME` tempdir (forces cache miss so the live project GET fires);
`JR_AUTH_HEADER=Basic <b64>`; mock `GET /rest/api/3/project/{KEY}` returns HTTP 401 with body
`{"errorMessages": ["The access token provided is expired, revoked, malformed, or invalid for other reasons."], "errors": {}}`.

Assertions: stderr `contains` "expired or revoked", `contains` "id.atlassian.com/manage-profile/security/api-tokens", `contains` "jr auth login"; does NOT contain "write:servicedesk-request".

**New test required:** `test_require_service_desk_basic_auth_401_surfaces_api_token_hint`
File: `tests/issue_create_jsm.rs`.
Note: All three callers (`handle_jsm_create`, `jr queue`, `jr requesttype`) benefit from the
`map_err` in `require_service_desk`. This test pins the `create` caller path; existing
`queue`/`requesttype` integration tests cover regression.

### AC-8 — `require_service_desk` OAuth 401 (cache miss, scope-mismatch body) → read-scope hint
(traces to BC-X.8.007 postconditions 1–2 — test: `test_require_service_desk_oauth_401_surfaces_read_scope_hint`)

When any live GET inside `get_or_fetch_project_meta` returns 401 and `client.is_oauth_auth() == true`,
BOTH sub-case arms (InsufficientScope AND NotAuthenticated) MUST rewrite to
`JrError::NotAuthenticated { hint }` with the read-side scope hint (canonical verbatim from F2 PRD delta):

```
Your OAuth token may be expired. Run `jr auth refresh` to renew the token, or
`jr auth login` to re-authorize. If using a custom OAuth app, run `jr auth login`
to re-consent with read:jira-work and read:servicedesk-request — `jr auth refresh`
alone cannot add missing scopes (it re-mints with the same granted scope set).
```

Test setup: isolated `XDG_CACHE_HOME` tempdir (forces cache miss); `JR_AUTH_HEADER=Bearer test-oauth-token`;
mock `GET /rest/api/3/project/{KEY}` returns HTTP 401 with **scope-mismatch body**:
`{"errorMessages": ["Unauthorized; scope does not match"]}`.

**WHY scope-mismatch body required:** A Bearer client with generic-expiry 401 enters the refresh
coordinator (`client.rs:727+`), fails with raw `anyhow`, and the map_err never fires. Scope-mismatch
body short-circuits to `InsufficientScope` at `client.rs:696-704` BEFORE the refresh coordinator —
deterministically reaching the `map_err`.

Assertions: stderr `contains` "read:jira-work" AND `contains` "read:servicedesk-request"; does NOT contain "write:servicedesk-request".

**New test required:** `test_require_service_desk_oauth_401_surfaces_read_scope_hint`
File: `tests/issue_create_jsm.rs`.

## Required Test Deliverables Summary

| # | Test Function | Type | BC Pin | Status |
|---|--------------|------|--------|--------|
| 1 | ~~`test_jsm_create_basic_auth_401_surfaces_api_token_hint`~~ DELETED (duplicate of row 3; Basic-auth generic-expiry path pinned by `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`) | ~~NEW~~ | BC-3.8.014 | AC-3 (shares test with AC-5) |
| 2 | `test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint` | NEW | BC-3.8.014 (highest-risk) | AC-4 |
| 3 | `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (repurposed and renamed by F4 from pre-#384 name) | REPURPOSED in place | BC-3.8.014 (assertions flipped, asserts API-token hint, `write:servicedesk-request` ABSENT) | AC-5 |
| 4 | `test_require_service_desk_basic_auth_401_surfaces_api_token_hint` | NEW | BC-X.8.006 | AC-7 |
| 5 | `test_require_service_desk_oauth_401_surfaces_read_scope_hint` | NEW | BC-X.8.007 | AC-8 |
| 6 | `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` | EXISTING — must stay GREEN unmodified | BC-3.8.015 / H-NEW-JSM-RT-003 | AC-6 |

All 6 are MANDATORY acceptance-gate deliverables. Missing any one leaves a BC without a test pin.

## Files to Touch

| File | Change | Risk |
|------|--------|------|
| `src/api/client.rs` | Add `pub fn is_oauth_auth(&self) -> bool` | LOW — additive public method on existing struct; reads existing field |
| `src/error.rs` | Add `pub const API_TOKEN_EXPIRY_HINT: &str` | LOW — additive constant; accessible from both api and cli layers with no layering inversion |
| `src/cli/issue/create.rs` | Gate `handle_jsm_create` map_err on `client.is_oauth_auth()` (Basic branch: rewrite any variant to `NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`; OAuth branch: preserve existing behavior) | MEDIUM — modifies error-path dispatch in central JSM create handler |
| `src/api/jsm/servicedesks.rs` | Introduce new `map_err` on `get_or_fetch_project_meta(...)` call in `require_service_desk` (Basic: rewrite to API-token hint; OAuth: rewrite both arms to NotAuthenticated with read-scope hint) | MEDIUM — modifies shared function called by handle_jsm_create, queue, requesttype |
| `tests/issue_create_jsm.rs` | (1) Repurpose existing test AC-5; (2) Add 4 new test functions AC-3/4/7/8; (3) Add H-NEW-JSM-RT-003 anchor comment to AC-6 test rustdoc | MEDIUM — modifies existing regression pin; new tests cover all 4 new-BC paths |

Do NOT touch BC files (sealed in F2). Do NOT modify `src/api/auth.rs`, `src/api/refresh_coordinator.rs`,
`DEFAULT_OAUTH_SCOPES`, or any test outside `tests/issue_create_jsm.rs` except for regression verification.

## Regression Baseline

Tests that MUST remain green unmodified after this delivery:

- `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (AC-6 — H-NEW-JSM-RT-003; Bearer + scope-mismatch → write:servicedesk-request)
- `test_platform_create_401_no_jsm_scope_hint` (platform Basic-auth 401 must not contain JSM scope; unaffected by this change)
- All 42 other tests in `tests/issue_create_jsm.rs`
- `tests/queue.rs` — adjacent JSM read path; `require_service_desk` shared function; must remain green
- `tests/requesttype_commands.rs` — project-type check path; must remain green
- `tests/api_client.rs` — BC-1.6.042..045 scope-mismatch dispatch tests; unaffected
- `tests/issue_create_json.rs` — platform create JSON shape; unaffected
- `tests/issue_commands.rs` — BC-3.3.x platform create coverage; unaffected

## Token Budget Estimate

| Item | Tokens (approx) |
|------|----------------|
| This story file | ~4 k |
| F2 PRD delta (prd-delta-384.md — main body only, skip Appendix) | ~12 k |
| BC files (4 new BCs: bc-3-issue-write.md §BC-3.8.014/015 + cross-cutting.md §BC-X.8.006/007) | ~6 k |
| `src/api/client.rs` (read is_oauth_auth insertion site + existing Bearer guard at :718/:802) | ~4 k |
| `src/error.rs` (read constant declaration site) | ~2 k |
| `src/cli/issue/create.rs` (read handle_jsm_create map_err block ~lines 1983-2009) | ~4 k |
| `src/api/jsm/servicedesks.rs` (read require_service_desk function ~lines 112-141) | ~2 k |
| `tests/issue_create_jsm.rs` (read existing test structure + AC-6 test for anchor comment site) | ~15 k |
| Tool outputs + cargo test output | ~5 k |
| **Total** | **~54 k** |

Well within single-agent context. No split required.
LOC delta estimate: ~5 lines in `client.rs`, ~4 lines in `error.rs`, ~15 lines in `create.rs`,
~20 lines in `servicedesks.rs`, ~160-200 lines in `tests/issue_create_jsm.rs` (4 new tests + repurpose).

## Tasks

- [ ] Read F2 PRD delta main body (`.factory/phase-f2-spec-evolution/prd-delta-384.md`) — capture CANONICAL verbatim blocks for `API_TOKEN_EXPIRY_HINT` and OAuth read-scope hint
- [ ] Read `src/api/client.rs` around line 696-730 — understand body-check ordering (InsufficientScope before Bearer guard) + identify `is_oauth_auth()` insertion site
- [ ] Read `src/error.rs` — identify `API_TOKEN_EXPIRY_HINT` constant insertion site (before or after existing enums/constants)
- [ ] Read `src/cli/issue/create.rs` lines ~1983-2009 — identify exact `handle_jsm_create` map_err block to gate
- [ ] Read `src/api/jsm/servicedesks.rs` lines 112-141 — identify `get_or_fetch_project_meta(...)` call to add `map_err` to
- [ ] Add `pub fn is_oauth_auth(&self) -> bool` to `JiraClient` in `src/api/client.rs`
- [ ] Add `pub const API_TOKEN_EXPIRY_HINT: &str` to `src/error.rs` with EXACT verbatim text from CANONICAL block
- [ ] Gate `handle_jsm_create` map_err: Basic (`is_oauth_auth()==false`) → rewrite any variant to `NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`; OAuth (`is_oauth_auth()==true`) → preserve existing behavior unchanged
- [ ] Introduce new `map_err` on `get_or_fetch_project_meta(...)` in `require_service_desk`: Basic → `NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT }`; OAuth → `NotAuthenticated { hint: <read-scope-hint verbatim> }`
- [x] Repurpose the pre-#384 Basic-auth 401 test in place → renamed to `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`: fixture stays Basic, assertions flipped to API-token-expiry hint, `write:servicedesk-request` absent, stale section banner/rustdoc updated (AC-5)
- [ ] Add anchor comment `// H-NEW-JSM-RT-003 + BC-3.8.015 anchor` to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` rustdoc (AC-6 comment-only)
- [x] ~~Add `test_jsm_create_basic_auth_401_surfaces_api_token_hint` (AC-3)~~ DELETED (Copilot dedup — duplicate of `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`; AC-3 + AC-5 share the repurposed test)
- [ ] Add `test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint` (AC-4)
- [ ] Add `test_require_service_desk_basic_auth_401_surfaces_api_token_hint` with isolated `XDG_CACHE_HOME` tempdir + Basic fixture (AC-7)
- [ ] Add `test_require_service_desk_oauth_401_surfaces_read_scope_hint` with isolated `XDG_CACHE_HOME` tempdir + Bearer fixture + scope-mismatch 401 body (AC-8)
- [ ] Verify `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` is UNCHANGED and GREEN (AC-6)
- [ ] Run `cargo test` — 6 mandatory tests pass; all pre-existing tests remain green; no regressions in queue/requesttype
- [ ] Run `cargo clippy -- -D warnings` — zero warnings; no `#[allow]` suppressions
- [ ] Run `cargo build --release` — succeeds
- [ ] Run `bash scripts/check-spec-counts.sh && bash scripts/check-bc-cumulative-counts.sh` — both exit 0 (BC count delta: +4 definitional; total 569→573)
- [ ] Per-story adversary 3/3 CLEAN before push

## Previous Story Intelligence

This story is adjacent to S-383 (platform-inverse warnings, merged PR #390) and
issue-288-pr4-dispatch (JSM dispatch fork, PR #381). Key lessons applied here:

- **From issue-288-pr4-dispatch and S-383:** The `handle_jsm_create` map_err block (lines ~1983-2009)
  is already well-tested in `tests/issue_create_jsm.rs`. Use existing BC-3.8.009-related test
  patterns as the structural template for new AC-3/4 tests.

- **From S-382 (JrError::InsufficientScope refactor):** `JrError::NotAuthenticated` is now a struct
  variant `{ hint: String }`. Use `JrError::NotAuthenticated { hint: API_TOKEN_EXPIRY_HINT.to_string() }`
  in the map_err rewrite (or `.into()` if a From impl exists). Verify the exact variant syntax from
  `src/error.rs` before writing the map_err.

- **Verbatim string discipline (from multiple prior stories):** Copy the CANONICAL verbatim blocks
  byte-for-byte from the F2 PRD delta. Any character deviation causes adversarial failures. The test
  assertions use `.contains()` not `==`, but the constant must be byte-identical to the CANONICAL block.

- **`JR_AUTH_HEADER` seam value-space (from SD-002 / S-0.05):** The seam is `#[cfg(debug_assertions)]`
  (CLAUDE.md SD-002). Test fixtures MUST supply `"Bearer test-oauth-token"` (capital B, single space)
  for the OAuth branch and `"Basic <b64>"` (capital B, single space) for the Basic branch. A malformed
  seam value (e.g., lowercase `"bearer"`) silently misclassifies as Basic.

- **Cache-miss precondition for require_service_desk tests:** AC-7 and AC-8 tests must force a cache
  miss. Use an isolated `XDG_CACHE_HOME` tempdir (same pattern as other cache-miss integration tests).
  Without this, the project-GET mock may never fire (cache hit skips the live GET).

- **Refresh coordinator interaction:** Do NOT attempt to test a Bearer + generic-expiry 401 on the
  JSM POST path. That path enters the refresh coordinator and fails with raw `anyhow` — the map_err
  never fires. Scope-mismatch body is the ONLY deterministic Bearer path to a `JrError`.

## Architecture Compliance Rules

- `is_oauth_auth()` is a PURE predicate on `JiraClient` reading the existing `self.auth_header`
  field. It is case-sensitive: `starts_with("Bearer ")` (capital B, single trailing space). This is
  the SAME discriminant production code already trusts at `client.rs:718` and `:802`.

- `API_TOKEN_EXPIRY_HINT` MUST live in `src/error.rs` — NOT `src/api/client.rs` or a new module.
  `src/error.rs` is imported by both the `api` and `cli` layers with no layering inversion.

- The `require_service_desk` `map_err` wraps the ENTIRE `get_or_fetch_project_meta(...)` future —
  not just the project GET arm. This is correct: `get_or_fetch_project_meta` issues two live GETs
  on a cache miss (project GET + service-desk list GET), and the API-token/read-scope hint applies
  uniformly to both.

- No new modules, no new crate dependencies, no `Cargo.toml` changes. This is confirmed by the
  F2 PRD delta §Architecture Delta section: "No architecture delta."

- The BC-3.8.015 OAuth branch (`handle_jsm_create` map_err) is genuinely UNCHANGED from pre-#384
  behavior. BOTH arms (InsufficientScope and NotAuthenticated) produce the `write:servicedesk-request`
  hint for OAuth — exactly as pre-#384. The only change is that the Basic branch now takes a
  different path.

- `require_service_desk` map_err OAuth arm MUST use `JrError::NotAuthenticated { hint }` — NOT
  `JrError::InsufficientScope`. The `InsufficientScope` Display is a fixed template purpose-built
  for the POST scenario (hardcoded POST-specific guidance). Read-side scope guidance does not fit
  that template and would produce misleading output.

- No `#[allow]` suppressions. If clippy warns, refactor to fix root cause per CLAUDE.md convention.

- Output channel: error hints reach the user via the existing `JrError` Display machinery.
  The hint text does NOT use `eprintln!` directly — it is embedded in the `hint` field of
  `JrError::NotAuthenticated { hint }`, which the error renderer prints to stderr.

## Forbidden Dependencies

- The `handle_jsm_create` and `require_service_desk` call sites MUST NOT duplicate the hint string
  literals. Both must reference `API_TOKEN_EXPIRY_HINT` from `src/error.rs`. Any implementation
  that inlines the hint text at one site is a correctness violation — single source of truth.

- `is_oauth_auth()` is the ONLY predicate for auth-scheme detection in error-hint dispatch.
  Do NOT introduce ad-hoc `auth_header.starts_with("Bearer ")` checks at the call sites.

## Library & Framework Requirements

- No new dependencies. All changes use stdlib + existing project types (`JrError`, `JiraClient`).
- `wiremock` in `tests/issue_create_jsm.rs` is already a dev-dependency — use the same version
  and import pattern as the existing tests in that file.
- `assert_cmd` + `predicates` are already present — use them for binary-level assertions.
- Isolated `XDG_CACHE_HOME` tempdir: use `tempfile::TempDir` (already a dev-dependency; pattern
  established in existing cache-miss tests).

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/api/client.rs` | Modify | Add `pub fn is_oauth_auth(&self) -> bool` as a new method on `JiraClient`; ~5 LOC net |
| `src/error.rs` | Modify | Add `pub const API_TOKEN_EXPIRY_HINT: &str`; ~6 LOC net |
| `src/cli/issue/create.rs` | Modify | Gate map_err in `handle_jsm_create`; ~15 LOC net change in existing map_err block |
| `src/api/jsm/servicedesks.rs` | Modify | Introduce new `.map_err(...)` on `get_or_fetch_project_meta(...)` call inside `require_service_desk`; ~20 LOC net |
| `tests/issue_create_jsm.rs` | Modify | Repurpose 1 existing test (AC-5); add 4 new test functions (AC-3/4/7/8); add anchor comment to AC-6 test; ~160-200 LOC net |
| `.factory/stories/STORY-INDEX.md` | Modify | Append S-384 row to Story Manifest + Feature Followup table; update total_stories and last_updated |
| `.factory/sprint-state.yaml` | Modify | Append S-384 entry under `feature_followup_standalone` block |

## Branch / PR Plan

- Branch: `feat/issue-384-jsm-401-auth-aware-hints`
- Target: `develop`
- Commit style: `feat(jsm): auth-aware 401 hints — gate is_oauth_auth() in handle_jsm_create + require_service_desk (#384)`
- PR closes #384
- CHANGELOG entry recommended: new `is_oauth_auth()` predicate and auth-conditional hint behavior
  is user-visible (error message content change for Basic-auth users)

## Per-Story Delivery Notes

- Demos (Step 5) are LOCAL-ONLY per `docs/demo-evidence/` gitignore convention.
- Per-story adversary 3/3 CLEAN required before push.
- F2 is CONVERGED (9 passes, 2026-05-19) — BC files are sealed. Do NOT re-edit BC files unless
  the adversary finds a discrepancy between the BC body and the implementation. If a discrepancy
  is found, escalate rather than self-amending.
- The `check-bc-cumulative-counts.sh` guard must exit 0 post-edit. Expected BC count after F2
  edits: 573 total (+4: BC-3.8.014/015 in bc-3; BC-X.8.006/007 in cross-cutting). If the BC
  files were already updated in F2, both guard scripts should exit 0 without additional edits.
- Test 5 (`test_require_service_desk_oauth_401_surfaces_read_scope_hint`) requires extra care:
  the 401 body MUST be a scope-mismatch body, not a generic-expiry body. A generic-expiry body
  with a Bearer token enters the refresh coordinator and never reaches the map_err.
