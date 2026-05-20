---
document_type: f1-delta-analysis
phase: phase-f1-delta-analysis
producer: architect
issue: 384
status: draft
created: 2026-05-19
project: jira-cli
mode: BROWNFIELD
intent: enhancement
feature_type: backend
trivial_scope: false
scope: standard
regression_risk: medium
severity: N/A
---

# F1 Delta Analysis — Issue #384

## Feature Request

- **Brief:** GitHub issue #384 — "JSM 401 hint surface refinement: distinguish Basic vs OAuth auth, cover require_service_desk path (O-08-01 + O-08-05 from #381)"
- **Issue link:** https://github.com/Zious11/jira-cli/issues/384
- **Requested by:** Zious11 (Jared Richards) via adversarial review deferred findings
- **Date:** 2026-05-19
- **Validation source:** `.factory/research/issue-288-pr4-deferred-validation.md` — FINDING 3: O-08-01 (lines 170-234) CONFIRMED; O-08-05 (lines 342-384) CONFIRMED. Premises accepted as validated; no re-validation performed.

## Problem Summary

Two distinct 401-hint surfaces in the JSM path produce misleading or missing guidance:

**Problem 1 (O-08-01):** `handle_jsm_create` in `src/cli/issue/create.rs` (lines ~1983-2009) maps BOTH `JrError::NotAuthenticated` (Basic-auth API-token expiry 401) AND `JrError::InsufficientScope` (OAuth scope-mismatch 401) to the same `write:servicedesk-request` OAuth-scope hint. Basic-auth users do not have OAuth scopes; the hint misleads them. The existing test `test_jsm_create_401_hint_contains_write_servicedesk_request` (line 1309, BC-1.3.023) uses `JR_AUTH_HEADER=Basic ...`, which hits the `NotAuthenticated` branch — proving the bug is live in the currently merged code. The separate test `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (line 1523, C-01) uses `Bearer` auth to cover the `InsufficientScope` branch.

**Problem 2 (O-08-05):** `require_service_desk` in `src/api/jsm/servicedesks.rs` (lines 112-141) calls `get_or_fetch_project_meta` which performs `GET /rest/api/3/project/{key}`. A 401 there propagates through `parse_error` to `NotAuthenticated { hint: "Run jr auth login to connect." }` with no JSM-specific scope guidance. The needed read-side scopes are `read:jira-work` (platform project GET) + `read:servicedesk-request` (JSM context), NOT `write:servicedesk-request`.

**Shared fix substrate:** Add `pub fn is_oauth_auth(&self) -> bool` on `JiraClient` (checks `self.auth_header.starts_with("Bearer ")`). Gate the existing `NotAuthenticated` arm in `handle_jsm_create` behind this predicate, and add a `map_err` to `require_service_desk` (or its call site inside `handle_jsm_create`) with auth-aware read-side scope hints.

## Classifications

### Intent Classification

**Classified intent:** `enhancement`

**Rationale:** The issue title says "refinement"; the two problems are error-hint quality improvements on already-working code paths, not new user capabilities or regression fixes on broken behavior. The JSM create POST succeeds; only the 401-error messaging diverges from user intent. This falls squarely in the "improve error surfacing" enhancement category.

### Feature Type Classification

**Classified type:** `backend`

**Rationale:** All changes are in Rust source files. No UI screens, no frontend assets, no CI/CD config. The change adds a predicate method on `JiraClient`, gates existing hint strings behind that predicate, and adds a new `map_err` branch in `require_service_desk`. Zero impact on any non-backend artifact.

### Trivial Scope Classification

A change is trivial when ALL of the following are true:

- [x] Impact boundary: single module, single file, or documentation only — FAILS: 3 source files (`src/api/client.rs`, `src/cli/issue/create.rs`, `src/api/jsm/servicedesks.rs`) plus new tests
- [ ] No new BCs needed — FAILS: 4 new BCs needed (2 per problem × 2 auth variants)
- [x] No architecture change — PASSES: new predicate is additive public method, not structural
- [x] No new external dependencies — PASSES
- [ ] Regression risk: LOW — MEDIUM: touches auth-adjacent error dispatch; existing holdout H-NEW-JSM-RT-003 must remain green

**Classified scope:** `standard`

**Rationale:** Multi-file change, 4 new BCs, new integration test coverage for 4 acceptance paths, and existing test `test_jsm_create_401_hint_contains_write_servicedesk_request` (BC-1.3.023 pin) must be updated to assert the new Basic-auth-specific hint text. Quick dev routing does not apply.

## Impact Assessment

| Dimension | Affected | Details |
|-----------|----------|---------|
| PRD Requirements | 0 new BCs in existing files (modified); 4 new BCs added as siblings | BC-3.8.009 modified (errors section clarification); BC-X.3.002 modified (footnote); 4 new BCs: BC-3.8.014 (Basic-auth 401 on JSM dispatch → API-token hint), BC-3.8.015 (OAuth 401 on JSM dispatch → write:servicedesk-request hint unchanged), BC-X.8.006 (Basic-auth 401 on require_service_desk → API-token hint), BC-X.8.007 (OAuth 401 on require_service_desk → read-side scope hint) |
| Architecture | 0 new components; 1 method added (MODIFIED) | `JiraClient::is_oauth_auth() -> bool` added to `src/api/client.rs` |
| UX Screens | None | Pure error-message quality improvement; no new user flows |
| Stories | 1 new story estimated | Single story: implement `is_oauth_auth` predicate + gate both hint sites + add 4 integration test paths |
| Existing Tests | 3 tests in regression risk zone | `test_jsm_create_401_hint_contains_write_servicedesk_request` (BC-1.3.023 pin — must be UPDATED to assert Basic-specific hint); `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (C-01 — must remain green, asserts Bearer + InsufficientScope path); H-NEW-JSM-RT-003 holdout (must remain green) |
| Verification Properties | None — no VP directory in use; BC-level anchoring sufficient | No proptest candidates; all 4 paths are boolean dispatch gates |

## Affected BC Mapping

### BCs Modified

| BC ID | File | Nature of Change |
|-------|------|-----------------|
| BC-3.8.009 | `bc-3-issue-write.md` | Errors section: replace monolithic "Scope error for `write:servicedesk-request`" with auth-conditional phrasing referencing BC-3.8.014 (Basic) and BC-3.8.015 (OAuth) |
| BC-X.3.002 | `cross-cutting.md` | Add footnote: "For JSM dispatch paths, 401 behavior is auth-conditional — see BC-3.8.014 / BC-X.8.006 (Basic) and BC-3.8.015 / BC-X.8.007 (OAuth)" |

### New BCs Needed

| Proposed BC ID | File | Description |
|----------------|------|-------------|
| BC-3.8.014 | `bc-3-issue-write.md` | Basic-auth 401 on JSM POST (`handle_jsm_create`) → API-token-expiry hint; no OAuth-scope language; references `jr auth login` with token-rotation URL |
| BC-3.8.015 | `bc-3-issue-write.md` | OAuth 401 on JSM POST (`handle_jsm_create`) → `write:servicedesk-request` hint unchanged from current behavior; `JrError::InsufficientScope` branch only |
| BC-X.8.006 | `cross-cutting.md` | Basic-auth 401 on `require_service_desk` path → API-token-expiry hint (same wording pattern as BC-3.8.014); no OAuth-scope language |
| BC-X.8.007 | `cross-cutting.md` | OAuth 401 on `require_service_desk` path → read-side scope hint: `read:jira-work` + `read:servicedesk-request`; `JrError::InsufficientScope` with `required_scope` set |

**Note on holdout:** H-NEW-JSM-RT-003 currently asserts `stderr.contains("write:servicedesk-request")` using `JR_AUTH_HEADER=Basic ...`. After this fix, the Basic-auth 401 path will NOT emit the OAuth scope string. The holdout scenario definition must be revised in F2 to use `Bearer` auth (the InsufficientScope path) for the scope-mismatch assertion, and a companion scenario added for the Basic-auth path.

## Files Changed

### New Files

| File Path | Purpose |
|-----------|---------|
| None | No new source modules needed; all changes are additive to existing files |

### Modified Files

| File Path | Change Type | Risk |
|-----------|-------------|------|
| `src/api/client.rs` | Additive: new `pub fn is_oauth_auth(&self) -> bool` method | LOW — additive public method; checks existing field `self.auth_header`; no behavior change to any existing code path |
| `src/cli/issue/create.rs` | Logic change: gate `NotAuthenticated` arm in `handle_jsm_create` map_err (lines ~1988-1994) behind `client.is_oauth_auth()` | MEDIUM — modifies error-path dispatch; existing Basic-auth 401 test (`BC-1.3.023` pin) currently asserts the OAuth hint string and will fail after the fix, confirming the change is correct |
| `src/api/jsm/servicedesks.rs` | Additive: add `map_err` to `get_or_fetch_project_meta` call inside `require_service_desk` (or at call site in `handle_jsm_create`) with auth-conditional scope hints | MEDIUM — modifies the error path of a shared function called by 4+ call sites: `handle_jsm_create`, `cli/queue.rs`, `cli/requesttype.rs`; must not change non-401 error paths |
| `tests/issue_create_jsm.rs` | MODIFIED + new tests: (1) update `test_jsm_create_401_hint_contains_write_servicedesk_request` to assert Basic-auth-specific hint; (2) add 3 new integration test functions pinning: Basic-auth 401 on require_service_desk, OAuth 401 on require_service_desk, OAuth-scope 401 on JSM dispatch (verify unchanged) | MEDIUM — modifies an existing regression pin; new tests cover all 4 AC paths |

### Dependent Files (unchanged but depend on modified files)

| File Path | Depends On | Regression Risk |
|-----------|------------|----------------|
| `src/cli/queue.rs` | `require_service_desk` in `servicedesks.rs` | LOW — queue commands call `require_service_desk` for project-type validation, not for 401 dispatch; existing queue 401 error path unchanged |
| `src/cli/requesttype.rs` | `require_service_desk` in `servicedesks.rs` | LOW — same as queue: project-type check; not a 401 surface in existing tests |
| `tests/jsm_request_api.rs` | `JiaClient` in `client.rs` | LOW — unit-level API tests; `is_oauth_auth` is additive and not exercised in these tests |
| `tests/api_client.rs` | `src/api/client.rs` | LOW — BC-1.6.042..045 scope-mismatch dispatch tests; these test `parse_error` / `send_inner` which is unchanged |
| `src/api/jsm/queues.rs` | Adjacent JSM module | NONE — no code dependency on changed files |
| `.factory/specs/prd/holdout-scenarios.md` | H-NEW-JSM-RT-003 | MEDIUM — must be revised in F2 spec work: scenario must switch to Bearer auth for the write-scope-mismatch path and add a Basic-auth companion scenario |

## Files NOT Changed (Regression Baseline)

These files must not be modified during implementation. All their tests must continue to pass:

- `src/api/jira/issues.rs` — platform create path entirely unchanged; no dispatch interaction
- `src/api/jsm/requests.rs` — `create_jsm_request` implementation unchanged; HTTP layer is not modified
- `src/api/auth.rs` — `DEFAULT_OAUTH_SCOPES` unchanged; this feature does not add scopes
- `src/cli/auth/tests/mod.rs` — scope-pin test unchanged
- `src/api/pagination.rs`, `src/api/rate_limit.rs` — unrelated infrastructure
- `src/adf.rs`, `src/duration.rs`, `src/output.rs`, `src/jql.rs` — unrelated utilities
- `tests/issue_create_json.rs` — platform create JSON shape; must remain green
- `tests/issue_commands.rs` — BC-3.3.x platform create coverage; must remain green
- `tests/issue_write_holdouts.rs` — existing holdout suite; H-NEW-JSM-RT-003 must remain green (or be intentionally revised per F2 spec)
- `tests/queue.rs` — adjacent JSM read path; unmodified
- `tests/auth_profiles.rs` — multi-profile isolation; unmodified
- `tests/requesttype_commands.rs` — requesttype list/fields tests; call `require_service_desk` but test project-type error paths only, not 401 paths
- `src/cli/issue/list.rs`, `workflow.rs`, `links.rs`, `helpers.rs`, `assets.rs` — unrelated issue subcommands
- `src/cli/board.rs`, `src/cli/sprint.rs`, `src/cli/worklog.rs`, `src/cli/team.rs`, `src/cli/user.rs` — unrelated subcommands
- `CLAUDE.md` — no documentation change required (new predicate is internal; hint text changes are user-visible but the CLAUDE.md gotchas section covers the blanket-401 and scope-mismatch architecture, which is unchanged)

## Risk Assessment

| Risk Type | Level | Rationale |
|-----------|-------|-----------|
| Regression | MEDIUM | The existing BC-1.3.023 pin (`test_jsm_create_401_hint_contains_write_servicedesk_request`) currently asserts `stderr.contains("write:servicedesk-request")` using Basic auth. After the fix, this assertion will fail intentionally — the test must be updated. Any implementation that passes the old test without updating it is broken. The C-01 test (`test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`) covers the Bearer-auth InsufficientScope path and must remain green unmodified. Risk is manageable because both tests exist and clearly delineate the two paths. |
| Architecture | LOW | `is_oauth_auth()` is a pure predicate reading an already-existing private field. No trait change, no interface breaking, no new crate dependency. Adding a `map_err` to `require_service_desk` is purely additive to the error path. |
| Security | LOW | The change is on the error-hint surface, not on authentication itself. `is_oauth_auth()` is read-only. The guard cannot be used to bypass authentication. The fix improves hint accuracy; it does not change which error variant is returned or what exit code fires. |
| Performance | NONE | `auth_header.starts_with("Bearer ")` is O(7) string comparison at error-path time. Zero impact on hot paths. |

## Regression Baseline

- **Total tests in `tests/issue_create_jsm.rs`:** 45 test functions (2,750 LOC)
- **Tests in risk zone (require update or close monitoring):**
  1. `test_jsm_create_401_hint_contains_write_servicedesk_request` — MUST BE UPDATED (currently asserts `write:servicedesk-request` on Basic-auth 401; after fix, Basic-auth 401 should NOT contain this string)
  2. `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` — MUST REMAIN GREEN (Bearer auth + InsufficientScope; hint unchanged)
  3. `test_platform_create_401_no_jsm_scope_hint` — MUST REMAIN GREEN (platform Basic-auth 401 must not contain JSM scope; unaffected)
  4. H-NEW-JSM-RT-003 in `tests/issue_write_holdouts.rs` — MUST BE REVISED: scenario uses Basic auth to assert `write:servicedesk-request`; must switch to Bearer auth to remain a valid holdout; or create two companion holdouts
- **Risk zone test files:** `tests/issue_create_jsm.rs`, `tests/issue_write_holdouts.rs`
- **Unchanged test files that must stay green:** all 42 remaining tests in `tests/issue_create_jsm.rs`, plus `tests/queue.rs`, `tests/requesttype_commands.rs`, `tests/api_client.rs`, `tests/issue_create_json.rs`, `tests/issue_commands.rs`

## Scope Recommendation

- **Mode:** Feature Mode — standard single-cycle delivery
- **Estimated new BCs:** 4 new (`BC-3.8.014`, `BC-3.8.015`, `BC-X.8.006`, `BC-X.8.007`); 2 BCs modified (`BC-3.8.009`, `BC-X.3.002`); H-NEW-JSM-RT-003 holdout definition revised
- **Estimated new stories:** 1 story (all 4 acceptance paths share a single fix substrate — the `is_oauth_auth` predicate — and can be delivered in a single atomic commit group)
- **Story scope:** Implement `JiaClient::is_oauth_auth()` → gate `handle_jsm_create` `NotAuthenticated` arm → add `map_err` to `require_service_desk` call → update `test_jsm_create_401_hint_contains_write_servicedesk_request` → add 3 new integration tests for the remaining AC paths → revise H-NEW-JSM-RT-003 holdout (or add companion)
- **Can parallelize:** No — the single predicate method is the substrate for both fix sites; implement it first, then both hint sites can be coded in the same pass

## Open Questions

1. **`require_service_desk` map_err placement:** The fix can be applied either (a) inside `require_service_desk` itself (adding `client.is_oauth_auth()` check before returning the error from `get_or_fetch_project_meta`), or (b) at the call site in `handle_jsm_create` (wrapping the `require_service_desk(...)` call with a second `map_err` alongside the existing `create_jsm_request` map_err). Option (a) benefits all callers (queue, requesttype) but means `require_service_desk` now carries hint responsibility. Option (b) is more localized but leaves queue/requesttype 401 paths unchanged. The issue text says "add map_err to `require_service_desk`" suggesting (a). Recommend option (a) for consistency with the issue; human should confirm if they prefer option (b).

2. **H-NEW-JSM-RT-003 holdout:** The holdout currently uses Basic auth to trigger the 401 and asserts `write:servicedesk-request`. After the fix this will fail. F2 spec work must either: (a) revise the holdout to use Bearer auth + "scope does not match" body (the correct trigger for the write-scope-mismatch path), or (b) split into two holdouts — one for OAuth InsufficientScope (must contain `write:servicedesk-request`) and one for Basic NotAuthenticated (must contain API-token-expiry hint). Recommend option (b) for complete coverage; human should confirm holdout revision strategy before F2 commits.

3. **API-token-expiry hint wording:** The issue proposes "Generate a new API token at https://id.atlassian.com/manage-profile/security/api-tokens" as the Basic-auth-specific hint. The existing `NotAuthenticated` hint in `client.rs` says "Run jr auth login to connect." Confirm the preferred wording for the new Basic-auth hint text to avoid inconsistency with other 401 surfaces.
