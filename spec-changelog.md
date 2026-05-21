---
document_type: spec-changelog
project: "jr (jira-cli)"
---

# Spec Changelog

Track all spec version changes. Most recent version first.

## [1.3.0] - 2026-05-20

### Type: MINOR

### Summary

Issue #388: Accurate cross-hierarchy type-change error + fix fake-endpoint hint (Option A). Adds 2 new BCs (BC-3.4.010, BC-3.4.011) defining the enriched error behaviour for `jr issue edit KEY --type X` HTTP 400 responses. Annotates BC-3.4.003 with an Errors cross-reference (no behavioral change). The `CROSS_HIERARCHY_HINT` constant (citing JRACLOUD-27893, no fake endpoint) also replaces the misleading `PUT /rest/api/3/issue/{key}/convert` hint at `src/cli/issue/create.rs:834`. Grand total: 575 → 577.

### New Requirements

| ID | Description |
|----|-------------|
| BC-3.4.010 | `jr issue edit KEY --type X` HTTP 400 + source `issuetype.subtask` differs from target type's `subtask` (cross-hierarchy mismatch) → exit 1, `CROSS_HIERARCHY_HINT` on stderr. Hint wording pinned verbatim: cites JRACLOUD-27893, directs user to Jira web UI action menu (`...`), avoids exact UI label (locale-resilient). Subtask-flag mismatch is the primary classifier; English substring `"issue type selected is invalid"` is NOT the sole gate. `CROSS_HIERARCHY_HINT` constant also replaces the fake `/rest/api/3/issue/{key}/convert` hint at `create.rs:834` (`--no-parent` subtask-bound 400 path). |
| BC-3.4.011 | `jr issue edit KEY --type X` HTTP 400 + same-hierarchy flags (`src_subtask == tgt_subtask`) → exit 1, typo hint referencing `jr project types` + raw Atlassian error body. OR indeterminate (source-issue fetch or project-types fetch fails) → exit 1, raw error body only; NO enrichment hint. `CROSS_HIERARCHY_HINT` (JRACLOUD-27893) MUST NOT appear on either sub-path (prevents false positives on typos and workflow-incompatibility 400s). |

### Modified Requirements

| ID | Nature |
|----|--------|
| BC-3.4.003 | Errors cross-reference added: when `edit --type X` returns HTTP 400, see BC-3.4.010 (cross-hierarchy → CROSS_HIERARCHY_HINT) and BC-3.4.011 (same-hierarchy/indeterminate → typo hint or raw error). Primary success path (PUT 204) and ADF description behavior are byte-for-byte unchanged. |

### Impact Assessment

| Dimension | Before | After | Delta |
|-----------|--------|-------|-------|
| bc-3-issue-write.md individually-bodied | 66 | 68 | +2 |
| bc-3-issue-write.md total_bcs | 95 | 97 | +2 |
| BC-INDEX.md total_bcs (grand total) | 575 | 577 | +2 |
| CANONICAL-COUNTS.md Sum | 575 | 577 | +2 |
| BCs modified (no count change) | — | BC-3.4.003 (annotation only) | — |

### New Holdout Scenarios

None. The ten (10) integration tests in `tests/issue_edit_type_errors.rs` provide complete regression coverage for the new BC paths. No holdout-level coverage is required for this delta (error-path enrichment only; no new user-visible flows or success paths).

### Required Test Deliverables

Required test deliverables (to be mandated by the implementing story in F3). Authoritative count: **TEN (10)** integration tests in `tests/issue_edit_type_errors.rs` (the delta-analysis.md figure of five is superseded by this F2 spec delta):

1. `test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint` — GET issue (`subtask: Some(false)`), GET project types (target `subtask: Some(true)`), PUT returns 400 → exit 1, stderr contains `JRACLOUD-27893`, stderr does NOT contain `jr api /rest/api/3/issue` (regression pin)
2. `test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint` — reverse direction (`subtask: Some(true)` → `Some(false)`), same assertions
3. `test_edit_type_same_hierarchy_400_surfaces_typo_hint` — both flags `subtask: Some(false)` → exit 1, stderr contains `` `jr project types` ``, stderr does NOT contain `JRACLOUD-27893` (negative pin), stderr does NOT contain `jr api /rest/api/3/issue` (fake-endpoint regression pin)
4. `test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error` — GET issue succeeds (`subtask: Some(false)`), GET project types returns 5xx → exit 1, extracted 400 message on stderr, no hint, stderr does NOT contain `JRACLOUD-27893`, stderr does NOT contain `jr api /rest/api/3/issue`
5. `test_edit_type_cross_hierarchy_hint_no_fake_endpoint_literal` — regression pin: CrossHierarchy 400 path → stderr does NOT contain `jr api /rest/api/3/issue`
6. `test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error` — `get_issue` returns HTTP 200 with `subtask` key OMITTED from issuetype object → `src_subtask: None` → `Indeterminate` → exit 1, extracted 400 message on stderr, no hint, `JRACLOUD-27893` absent, `jr api /rest/api/3/issue` absent (tests Indeterminate Cause-2, source-side)
7. `test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error` — source `subtask: Some(false)` present; `get_project_issue_types` returns HTTP 200 with target type's `subtask` key OMITTED → `tgt_subtask: None` → `Indeterminate` → exit 1, same negative assertions (tests Indeterminate Cause-2, target-side)
8. `test_edit_type_unresolved_type_name_surfaces_typo_hint` — `get_issue` returns HTTP 200 with source `subtask: Some(false)`; `get_project_issue_types` returns HTTP 200 with a list that does NOT contain the `--type` value → unresolvable-name sub-path → typo hint, stderr contains `` `jr project types` ``, `JRACLOUD-27893` absent, `jr api /rest/api/3/issue` absent
9. `test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error` — `edit_issue` returns HTTP 400; `get_issue` returns 5xx → `Indeterminate` immediately (R1 routing row; `get_project_issue_types` never called); exit nonzero, raw error on stderr, no hint, `JRACLOUD-27893` absent, `jr api /rest/api/3/issue` absent
10. `test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment` — `edit_issue` returns HTTP 403 (non-400, R0b routing row) → enrichment block bypassed entirely; exit nonzero, raw error on stderr, no hint, `JRACLOUD-27893` absent, `jr api /rest/api/3/issue` absent; no wiremock stubs for `get_issue` or `get_project_issue_types`

Additionally: strengthen T-06 in `tests/issue_edit_no_parent.rs` (`test_subtask_parent_clear_surfaces_400_with_convert_hint`): add `assert!(stderr.contains("JRACLOUD-27893"))` literal-pin, `assert!(!stderr.contains("jr api /rest/api/3/issue"))` negative regression guard, and `assert!(stderr.contains("Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue."))` (pins the verbatim normative context sentence). The regression-pin substring `jr api /rest/api/3/issue` supersedes the broader form `/rest/api/3/issue/` from the F1 delta-analysis — the broader form is over-broad and false-positive-prone against legitimate diagnostic output.

### Feature Request Link

- https://github.com/Zious11/jira-cli/issues/388

---

## [1.2.0] - 2026-05-20

### Type: MINOR

### Summary

Issue #385: JSM create UX polish — harmonize project-required error (O-08-02), guard empty `--request-type` (O-08-04), reject `--markdown` + `--field description=` conflict (O-08-06), clarify warning position post-`require_service_desk` (O-08-07). Adds 2 new BCs (BC-3.8.016, BC-3.8.017) and modifies 3 BCs (BC-3.8.002, BC-3.8.010, BC-3.8.011). Grand total: 573 → 575.

### New Requirements

| ID | Description |
|----|-------------|
| BC-3.8.016 | `--request-type ""` (empty string or whitespace-only after trim) exits 64 with "request type cannot be empty" before `partial_match` or numeric bypass; no HTTP issued |
| BC-3.8.017 | `--markdown` + `--field description=<value>` combination rejected at parse-time in `handle_jsm_create`; exit 64; rationale: desync of `isAdfRequest: true` with plain-string description "may result in a JSM 400 error or silently dropped ADF formatting" (NOT asserted as certain) |

### Modified Requirements

| ID | Nature |
|----|--------|
| BC-3.8.002 | "No project resolvable AND `no_input` effective (explicit `--no-input` OR auto-enabled on non-TTY stdin) OR `prompt_input` errors" error string harmonized: `"Project key is required for JSM request creation. Use --project or configure .jr.toml. Run \"jr project list\" to see available JSM projects."` — adds `--project`/`.jr.toml`/`jr project list` affordances matching platform path, preserves "for JSM request creation" context. The code checks `no_input` only; non-TTY auto-enables it (CLAUDE.md). Previous string: `"project is required for JSM request creation"`. |
| BC-3.8.010 | Warning position clarified: `--type` warning fires INSIDE `handle_jsm_create` AFTER `require_service_desk` returns `Ok`, NOT pre-`handle_jsm_create`. Non-JSM project: ONLY the non-JSM error is emitted (no spurious warning). New companion test required: `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project`. |
| BC-3.8.011 | Same warning-position constraint applied: all six warnings (the `--type` warning of BC-3.8.010 plus the five platform-only flag warnings of BC-3.8.011: --team, --points, --parent, --to, --account-id) move to post-`require_service_desk` position in `handle_jsm_create`. |

### New Holdout Scenarios

| ID | Description |
|----|-------------|
| H-NEW-JSM-RT-006 | `--request-type ""` exits 64 with explicit empty-string message; no HTTP (pins BC-3.8.016) |
| H-NEW-JSM-RT-007 | `--markdown` + `--field description=plain` exits 64 at parse-time; no HTTP (pins BC-3.8.017) |

**O-08-02 holdout-exempt note**: BC-3.8.002 (O-08-02: project-required error string) is DELIBERATELY holdout-exempt. Unlike O-08-04 (→H-NEW-JSM-RT-006) and O-08-06 (→H-NEW-JSM-RT-007), this is a string-only error-message change with no control-flow impact. The existing unit test `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (updated to assert the new verbatim string) provides complete regression coverage. See prd-delta-385.md §BC-3.8.002 for the canonical statement.

### Impact Assessment

| Dimension | Before | After | Delta |
|-----------|--------|-------|-------|
| bc-3-issue-write.md individually-bodied | 64 | 66 | +2 |
| bc-3-issue-write.md total_bcs | 93 | 95 | +2 |
| BC-INDEX.md total_bcs (grand total) | 573 | 575 | +2 |
| CANONICAL-COUNTS.md Sum | 573 | 575 | +2 |
| holdout-scenarios.md total_holdouts | 55 | 57 | +2 |
| BCs modified (no count change) | — | BC-3.8.002, BC-3.8.010, BC-3.8.011 | — |

### Required Test Deliverables

Required test deliverables: see `.factory/phase-f2-spec-evolution/prd-delta-385.md §Required Test Deliverables` (canonical copy — do not duplicate here).

---

## [1.1.0] - 2026-05-19

### Type: MINOR

### Summary

Issue #384: JSM 401 hint surface refinement — distinguish Basic-auth (API-token-expiry hint) vs OAuth (preserve existing hint behavior) on both the `handle_jsm_create` dispatch path and the `require_service_desk` project-GET path. Adds `JiraClient::is_oauth_auth()` predicate contract and four new behavioral contracts covering the auth-conditional error hint branches.

**Corrected design model (adversary C-01/C-02):** The gate is `is_oauth_auth()` ALONE — not error variant. A Basic-auth 401 with a "scope does not match" body arrives as `InsufficientScope` (body check at `src/api/client.rs:696` fires before Bearer guard at line 718). The Basic-auth `map_err` must REWRITE any incoming variant to `NotAuthenticated` with the API-token hint. A shared constant `API_TOKEN_EXPIRY_HINT` is required at both call sites (BC-3.8.014 and BC-X.8.006).

### New Requirements

| ID | Description |
|----|-------------|
| BC-3.8.014 | Basic-auth 401 on JSM POST (`handle_jsm_create`) → any variant rewritten to `NotAuthenticated` with API-token-expiry hint (no OAuth-scope language); gated by `client.is_oauth_auth() == false`; shared constant `API_TOKEN_EXPIRY_HINT` |
| BC-3.8.015 | OAuth 401 on JSM POST (`handle_jsm_create`) → existing behavior preserved unchanged; for OAuth (`is_oauth_auth() == true`), BOTH arms (InsufficientScope AND NotAuthenticated) produce `write:servicedesk-request` hint — the pre-#384 map_err at `src/cli/issue/create.rs:1988-1995` already rewrites NotAuthenticated to `write:servicedesk-request` for all auth; now explicitly gated on `client.is_oauth_auth() == true` |
| BC-X.8.006 | Basic-auth 401 from `require_service_desk` project GET (cache miss) → any variant rewritten to `NotAuthenticated` with API-token-expiry hint; gated by `client.is_oauth_auth() == false`; shared constant `API_TOKEN_EXPIRY_HINT`; benefits all JSM callers |
| BC-X.8.007 | OAuth 401 from `require_service_desk` project GET (cache miss) → both sub-case arms rewrite to `JrError::NotAuthenticated { hint }` (NOT InsufficientScope — that Display is purpose-built for the POST scenario) with read-side scope hint (`read:jira-work` + `read:servicedesk-request`); gated by `client.is_oauth_auth() == true`; both scopes in DEFAULT_OAUTH_SCOPES; same new map_err as BC-X.8.006 |

### Modified Requirements

| ID | Previous | Updated | Rationale |
|----|----------|---------|-----------|
| BC-3.8.001 | Errors cross-reference: no auth-conditional 401 reference | Errors cross-reference updated to point at BC-3.8.009 (auth-conditional: Basic-auth → BC-3.8.014; OAuth → BC-3.8.015) | Cross-reference refresh — no behavioral change |
| BC-3.8.009 | Errors section: monolithic "Scope error for `write:servicedesk-request`" | Auth-conditional: `is_oauth_auth() == false` → BC-3.8.014 (API-token hint; any variant rewritten); `is_oauth_auth() == true` → BC-3.8.015 (existing behavior) | Gate is `is_oauth_auth()` alone; Basic-auth users must never see OAuth scope language |
| BC-X.3.002 | Universal 401 baseline (no JSM footnote) | Added JSM auth-conditional footnote: gate is `is_oauth_auth()` alone; Basic-auth any variant → API-token hint; OAuth → existing variant behavior; base contract unchanged for non-JSM paths | Cross-reference for implementers |

### Revised Holdouts

| ID | Previous | Updated | Rationale |
|----|----------|---------|-----------|
| H-NEW-JSM-RT-003 | Auth fixture: `JR_AUTH_HEADER=Basic ...`; asserted `write:servicedesk-request`; project mock missing `"id"` field; servicedesk mock missing `"projectId"` field | Auth fixture: `JR_AUTH_HEADER=Bearer test-oauth-token` (OAuth); asserts `write:servicedesk-request` (BC-3.8.015 pin). Setup uses helper abstraction: `mount_project_meta_help` (project `HELP`, id `"99"`) + `mount_service_desk_list` (servicedesk list, `projectId "99"`) + `mount_request_types_password_reset` (single-element list: `"Password Reset"` only). BC-X.8.006/X.8.007 removed from BC list (those BCs fire on 401 from `require_service_desk` GETs — this holdout's GETs return 200). Clarifying note added. [Fixture re-bound Pass-9 to real bound test `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` — see `holdout-scenarios.md §H-NEW-JSM-RT-003`.] | Prior rationale was incorrect (Basic-auth + scope-mismatch body still routes to InsufficientScope, not NotAuthenticated). Mock bodies were missing required `id`/`projectId` fields that would cause exit 64 before the JSM POST (the holdout's target). |

### Test Instructions (canonical — adversary-pass-9 C-01 corrected; use THESE, not any earlier draft)

> **[adversary-pass-9 C-01 CRITICAL correction]** Prior instructions (items 1 and 4 below in the old draft) said "switch the Basic-auth 401 test to Bearer" — this was UNWORKABLE. See adversary-pass-9 §Corrections below. Corrected instructions (item 3 reflects actual F4 outcome — test repurposed in place and renamed to `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`):

1. `test_jsm_create_basic_auth_401_surfaces_api_token_hint` — NEW (BC-3.8.014); Basic-auth fixture, generic 401 body → assert API-token hint.
2. `test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint` — NEW (BC-3.8.014); Basic-auth fixture, "scope does not match" body → assert API-token hint (pins InsufficientScope→NotAuthenticated rewrite; highest regression risk).
3. `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` — REPURPOSED IN PLACE (BC-3.8.014 pin; renamed by F4 from the pre-#384 name): fixture STAYS `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0`; assertions flipped from `write:servicedesk-request` to API-token-expiry hint; negative assertion that `write:servicedesk-request` is ABSENT. Do NOT switch to Bearer.
4. `test_require_service_desk_basic_auth_401_surfaces_api_token_hint` — NEW (BC-X.8.006); Basic-auth fixture, cache miss forced, project-GET returns generic-expiry 401 → assert API-token-expiry hint.
5. `test_require_service_desk_oauth_401_surfaces_read_scope_hint` — NEW (BC-X.8.007); Bearer fixture, cache miss forced, project-GET returns scope-mismatch 401 (`{"errorMessages": ["Unauthorized; scope does not match"]}`) → assert `read:jira-work` + `read:servicedesk-request`; does NOT contain `write:servicedesk-request`. Scope-mismatch body required — generic-expiry body routes through refresh coordinator (raw anyhow, not a JrError).
6. `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` — EXISTING (BC-3.8.015 pin, H-NEW-JSM-RT-003); already green on `develop`; MUST remain green unmodified. Bearer fixture, scope-mismatch body → asserts `write:servicedesk-request`.

All hint assertions use `contains`, not `==` (renderer prepends `"Not authenticated. "` or `"Insufficient token scope: "` — NOTE colon, not period, per `src/error.rs:8-16`).

### Removed Requirements

None.

### New Verification Properties

None (all 4 AC paths are boolean dispatch gates; proptest not applicable; BC-level integration test coverage sufficient per F1 delta analysis).

### Architecture Changes

- `JiraClient::is_oauth_auth() -> bool`: new public predicate method on `src/api/client.rs` — additive, no structural change. Reads existing `self.auth_header` field; no new dependencies.
- `API_TOKEN_EXPIRY_HINT: &str`: new shared constant in **`src/error.rs`** (NOT `src/api/client.rs` or any new module — `src/error.rs` is imported by both `api` and `cli` layers with no layering inversion; "no new modules" constraint honored). Referenced identically by `handle_jsm_create` and `require_service_desk` map_err sites.
- Architecture delta: none required.

### Impact Assessment

- **Affected stories:** 1 new story to implement (`is_oauth_auth` predicate + `API_TOKEN_EXPIRY_HINT` constant + gate both hint sites with rewrite logic + repurpose the pre-#384 Basic-auth 401 test → `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` as BC-3.8.014 pin with flipped assertions [fixture stays Basic, no Bearer migration] + add 3 new integration tests + `test_require_service_desk_oauth_401_surfaces_read_scope_hint` must use scope-mismatch body)
- **Affected tests:** `tests/issue_create_jsm.rs` — `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` stayed Basic, was repurposed with assertions flipped to API-token-expiry hint (not `write:servicedesk-request`); 2 new Basic-auth tests added; 1 new `require_service_desk` Basic test; 1 new `require_service_desk` OAuth test with scope-mismatch body. `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` is the BC-3.8.015 pin AND H-NEW-JSM-RT-003 binding — remains green unmodified. (Adversary-pass-9 C-01 correction: prior statement that holdout was realized as the pre-#384 test name is SUPERSEDED.)
- **Migration needed:** NO (no API change; error hint text changes only)
- **Migration notes:** N/A

### Adversary Pass-2 Corrections (2026-05-19)

Applied after second fresh-context adversary pass found 3 CRITICAL + 6 HIGH + 4 MEDIUM findings:

| Finding | Resolution |
|---------|-----------|
| C-01: `require_service_desk` has NO existing `map_err` at line 117 | BC-X.8.006/007 now explicitly state "MUST introduce a NEW `map_err`" — not "modify" |
| C-02: `InsufficientScope` renderer uses colon, not period | All `"Insufficient token scope. "` citations corrected to `"Insufficient token scope: "` throughout |
| C-03: BC-X.8.007 must NOT use `InsufficientScope` | Both OAuth sub-case arms in BC-X.8.007 now rewrite to `NotAuthenticated { hint }` — the `InsufficientScope` Display is POST-specific noise on a read GET |
| H-01: Dual exit codes (64 / 2) on `require_service_desk` | Added to BC-X.8.006/007: exit 64 (UserError, non-JSM) vs exit 2 (NotAuthenticated, 401) |
| H-02: H-NEW-JSM-RT-003 missing `GET /rest/api/3/project/{KEY}` mock | Added step 2 to holdout setup in holdout-scenarios.md and prd-delta |
| H-03: Count evidence missing from prd-delta | Added verbatim guard output + CANONICAL-COUNTS.md authority citation |
| H-04: `is_oauth_auth()` value-space imprecise | Full value-space documented in BC-3.8.014 and prd-delta; constructor error-on-empty noted |
| H-05/H-06: BC-3.8.015 false claim about OAuth NotAuthenticated → "generic jr auth login" | Corrected: pre-#384 map_err at create.rs:1988-1995 ALREADY rewrites NotAuthenticated to `write:servicedesk-request` for all auth; OAuth BOTH arms produce `write:servicedesk-request` |
| M-01: Trace file paths valid | Confirmed: `tests/issue_create_jsm.rs` exists. NOTE (adversary-pass-5 F-01 correction): H-NEW-JSM-RT-003 was at this point realized in `tests/issue_create_jsm.rs` — there is no separate `tests/issue_write_holdouts.rs` holding this holdout. At pass-5, the holdout was realized AS `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`. Subsequently, adversary-pass-9 C-01 re-bound H-NEW-JSM-RT-003 to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`. |
| M-02: Cache-warm suppression as user-facing boundary | Added to BC-X.8.006/007 as explicit behavioral boundary (not just test-setup note) |
| M-03: `API_TOKEN_EXPIRY_HINT` location | Pinned to `src/error.rs` in BC-3.8.014, BC-X.8.006, prd-delta, changelog |
| M-04: Count evidence recorded | Covered by H-03 fix |

### Adversary Pass-3 Corrections (2026-05-19)

Applied after third fresh-context adversary pass found 2 CRITICAL + 6 HIGH findings:

| Finding | Resolution |
|---------|-----------|
| C-01: BC-X.8.006/007 described trigger as only `GET /rest/api/3/project/{key}`; `get_or_fetch_project_meta` issues TWO live GETs for service_desk-type projects | BC-X.8.006/007 Behavior sections broadened: trigger is "any 401 from `get_or_fetch_project_meta`'s live calls — the project GET OR the service-desk list GET (the latter fires only for service_desk-type projects)"; trigger description heading updated from "project GET" to "cache miss" |
| C-02: H-NEW-JSM-RT-003 project mock missing `"id"` field → `project_id` defaults to `""` → desk match fails → exit 64 before JSM POST | **[SUPERSEDED by Pass-6 — see `holdout-scenarios.md §H-NEW-JSM-RT-003`]** Pass-3 corrected mock bodies to `{"key":"HELPDESK","id":"10001","projectTypeKey":"service_desk"}` and `"projectId":"10001"`. Pass-6 subsequently regrounded the holdout to the real bound test fixture (project `HELP`, id `"99"`, `mount_project_meta_help`/`mount_service_desk_list` helpers). The `HELPDESK`/`10001` bodies here are historical and no longer authoritative. |
| H-03: BC-X.8.007 hint text leads with BYO-scope sentence before session-expiry recovery | Hint rewritten: LEADS with session-expiry recovery (`jr auth refresh` / `jr auth login`), BYO-OAuth scope sentence is SECONDARY |
| H-04: BC-X.8.007 verbatim hint block labeled "InsufficientScope-arm rewrite" as if sub-case-specific; both arms emit identical hint | ONE canonical verbatim hint block documented and labeled "both arms of the require_service_desk OAuth 401 map_err emit this identical hint"; single pinnable string for the acceptance test |
| H-05: BC-X.8.006/007 acceptance tests unnamed ("New integration test") | Named test functions added following project convention: `test_require_service_desk_basic_auth_401_surfaces_api_token_hint` (BC-X.8.006) and `test_require_service_desk_oauth_401_surfaces_read_scope_hint` (BC-X.8.007); cross-caller coverage clarified (map_err is in require_service_desk; tests pin create path; queue/requesttype existing tests cover regression) |
| H-06: H-NEW-JSM-RT-003 `BC:` list included BC-X.8.006/X.8.007 even though this holdout's GETs return 200 | BC-X.8.006/X.8.007 removed from BC list; clarifying note added to holdout body explaining why (those BCs fire on 401 from the GETs; this holdout's GETs return 200; those BCs are pinned by dedicated integration tests) |
| H-07: Changelog "Modified Requirements" table listed H-NEW-JSM-RT-003 (a holdout) alongside BCs | H-NEW-JSM-RT-003 moved to separate "Revised Holdouts" subsection in changelog |
| H-08: BC-3.8.001 missing from "Modified BCs" section and changelog "Modified Requirements" table | BC-3.8.001 added to both with annotation "cross-reference refresh — no behavioral change" |

### Adversary Pass-4 Corrections (2026-05-19)

Applied after fourth fresh-context adversary pass found 0 CRITICAL + 1 HIGH + 3 MEDIUM findings. Design model confirmed converged. All findings are pinning/consistency defects:

| Finding | Severity | Resolution |
|---------|----------|-----------|
| F-01: H-NEW-JSM-RT-003 step 4 request-type mock has bare-object body `{id: "5", name: "Get IT Help"}` that does NOT deserialize into the request-type page struct (paginated envelope with `isLastPage` + `values`); name resolution fails before the holdout reaches the JSM POST | HIGH | Step 4 body corrected to `{"isLastPage": true, "values": [{"id": "5", "name": "Get IT Help", "description": "IT support"}]}` — verbatim match to H-NEW-JSM-RT-004's step 3 mock (same endpoint, same struct, same deserializer); revision note added to holdout body |
| F-02: BC-3.8.014 acceptance-test list in prd-delta included `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` described at the time as a Bearer-fixture test under BC-3.8.014 (Basic-auth) — a test-ownership contradiction; BC body already correctly scoped it | MEDIUM | Removed from BC-3.8.014 acceptance-test list; added explicit F-02 note (subsequently superseded by adversary-pass-9 C-01 which confirmed the test stays Basic and is the BC-3.8.014 pin) |
| F-03: Required test deliverables not explicitly enumerated as mandatory acceptance-gate deliverables; scope-mismatch-rewrite test (`test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint`) — the highest-regression-risk pin — not flagged with ordering-dependency note | MEDIUM | Added "Required Test Deliverables" section to prd-delta-384.md listing all 5 named test functions as MANDATORY ACs; scope-mismatch-rewrite test explicitly flagged as highest-regression-risk pin with `client.rs:696-718` ordering dependency; corresponding entry added to this changelog Impact Assessment |
| F-04: `API_TOKEN_EXPIRY_HINT` Basic-auth hint text and BC-X.8.007 OAuth read-scope hint text each inlined verbatim in multiple spec files without a canonical-source designation; no doc-fallout protection on future edits | MEDIUM | Designated prd-delta-384.md copy of each hint as the CANONICAL verbatim block (labeled); duplicate locations in bc-3-issue-write.md, cross-cutting.md annotated with "duplicated from prd-delta-384.md §<BC> CANONICAL block — all copies MUST be updated together; cf. JR_* doc-fallout pattern in CLAUDE.md" |

### Required Test Deliverables (adversary-pass-4 F-03; adversary-pass-9 C-01 corrected — Impact Assessment entry)

> **[adversary-pass-5 LOW]** This list is duplicated near-verbatim from `prd-delta-384.md §Required Test Deliverables`. The `prd-delta-384.md` copy is canonical. Update both copies together when either changes.

> **[adversary-pass-9 C-01 CRITICAL correction]** Item 3 corrected — see adversary-pass-9 §Corrections below. Item 6 added.

The following named test functions are MANDATORY acceptance-gate deliverables of the implementing story. The implementing story's ACs MUST include each as a discrete AC:

1. `test_jsm_create_basic_auth_401_surfaces_api_token_hint` (NEW — BC-3.8.014)
2. `test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint` (NEW — BC-3.8.014; **highest regression risk** — pins `client.rs:696-718` ordering; must not be skipped)
3. `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (REPURPOSED IN PLACE by F4 — BC-3.8.014 pin; fixture STAYS Basic; assertions flipped to API-token-expiry hint; negative assertion that `write:servicedesk-request` is ABSENT. Bearer not used — Bearer + generic-expiry routes through refresh coordinator and is not a valid pin.)
4. `test_require_service_desk_basic_auth_401_surfaces_api_token_hint` (NEW — BC-X.8.006; generic-expiry 401 body for Basic is fine — Basic never enters refresh path)
5. `test_require_service_desk_oauth_401_surfaces_read_scope_hint` (NEW — BC-X.8.007; scope-mismatch 401 body REQUIRED — generic-expiry Bearer 401 routes through refresh coordinator and is not a valid pin)
6. `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (EXISTING — BC-3.8.015 pin; H-NEW-JSM-RT-003 re-bound here; already green on `develop`; MUST remain unmodified)

BC-3.8.015 has a holdout pin (H-NEW-JSM-RT-003 — re-bound to item 6 above). BC-3.8.014, BC-X.8.006, BC-X.8.007 rely solely on the named integration tests.

### Adversary Pass-5 Corrections (2026-05-19)

Applied after fifth fresh-context adversary pass found 0 CRITICAL + 3 HIGH + 4 MEDIUM findings. Design model and source-code anchors confirmed sound. All findings are test-symbol-accuracy and doc-consistency defects:

| Finding | Severity | Resolution |
|---------|----------|-----------|
| F-01: H-NEW-JSM-RT-003 test-file location contradiction — holdout body + changelog Impact Assessment cite `tests/issue_write_holdouts.rs`; ground truth is `tests/issue_create_jsm.rs` (the string `H-NEW-JSM-RT-003` appears ONLY there); the holdout and the pre-#384 Basic-auth 401 test were the SAME artifact at pass-5 | HIGH | Changelog Impact Assessment corrected: holdout was in `tests/issue_create_jsm.rs`, realized AS `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (renamed by F4). M-01 pass-2 table entry corrected. holdout-scenarios.md §H-NEW-JSM-RT-003 clarified. Subsequently superseded by adversary-pass-9 C-01 which re-bound H-NEW-JSM-RT-003 to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint`. |
| F-02: BC-3.8.015 "UNCHANGED" framing misleading — the pre-#384 Basic-auth 401 test (`test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`) used a Basic-auth fixture with generic 401 body (pre-#384 bug scenario); post-#384, Basic+generic-401 MUST produce the API-token hint per BC-3.8.014 → this test WOULD FAIL after BC-3.8.014 lands | HIGH | BC-3.8.015 section in prd-delta-384.md reworded. Subsequently superseded by adversary-pass-9 C-01 which found Bearer migration was unworkable — test was repurposed in place as a BC-3.8.014 pin with flipped assertions. |
| F-03: BC-3.8.015 Trace/prd-delta cite `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` as a test that "must remain green unmodified" without confirming the exact `async fn` symbol; test was unverified | HIGH | Verified by reading `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` in `tests/issue_create_jsm.rs` (under the `// ─── C-01: OAuth InsufficientScope 401 surfaces write:servicedesk-request ────` section banner): confirmed `async fn` name, uses `JR_AUTH_HEADER=Bearer test-oauth-token` and a 401 body `{"errorMessages": ["Unauthorized; scope does not match"]}`. BC-3.8.015 Trace in bc-3-issue-write.md and prd-delta updated to use symbol-relative anchor (section banner + `async fn` name); hardcoded line numbers removed per adversary-pass-8 F-02. |
| F-04: H-NEW-JSM-RT-003 missing cache-miss precondition — BC-X.8.006/007 explicitly mandate "MUST force a cache miss"; the holdout's request-type GET mock is only reached on a cold cache but this precondition is implicit | MEDIUM | One-line cache-miss precondition added to H-NEW-JSM-RT-003 Setup in holdout-scenarios.md: "Cache dir is empty (isolated `tempfile::tempdir()` for `XDG_CACHE_HOME`) — all GET mocks are reached on a cold cache." |
| F-05: prd-delta Count Impact table omits "Before total" column — the +2 per file cannot be verified end-to-end | MEDIUM | "Before total" column added: bc-3-issue-write.md was 91 definitional / cross-cutting was 138 definitional; grand total before was 569; grand total after is 573 (+4 new BCs). Guard-script output in prd-delta relabeled "expected post-edit output; authoritative verification is `check-bc-cumulative-counts.sh`, not this document." |
| F-06: prd-delta §BC-3.8.001 summary understates change — says only "point at BC-3.8.009" but the BC body also names BC-3.8.014/015 inline | MEDIUM | prd-delta §Modified Behavioral Contracts §BC-3.8.001 summary aligned with BC body: "Errors cross-reference routes 401 via BC-3.8.009 and additionally names BC-3.8.014/015 inline." |
| F-07: `is_oauth_auth()` Interface Contract section missing `JR_AUTH_HEADER` seam value-space note — `is_oauth_auth()` is case- and space-sensitive; a malformed seam value silently misclassifies as Basic | MEDIUM | Added to prd-delta-384.md §Interface Contract: test fixtures using the debug-only `JR_AUTH_HEADER` seam MUST supply `"Bearer <token>"` (capital B, single trailing space) for OAuth branch and `"Basic <b64>"` for Basic branch; malformed values silently misclassify as Basic. |
| LOW: "Required Test Deliverables" list duplicated near-verbatim in prd-delta and changelog | LOW | prd-delta copy designated canonical; changelog copy annotated "duplicated from prd-delta-384.md §Required Test Deliverables — update together." |

### Adversary Pass-8 Corrections (2026-05-19)

Applied after eighth fresh-context adversary pass found 0 CRITICAL + 3 MEDIUM completeness/coherence defects (F-01, F-02, F-03) plus 1 LOW (URL-encoding note):

| Finding | Severity | Resolution |
|---------|----------|-----------|
| F-01: H-NEW-JSM-RT-003 Setup step 4 under-describes the request-type fixture — says "returns request types including Password Reset" without mentioning the two-element list or the `partial_match` resolution mechanism | MEDIUM | step 4 rewritten in holdout-scenarios.md §H-NEW-JSM-RT-003: `mount_request_type_list` returns a TWO-element list (`Get IT Help` id 11001 + `Password Reset` id 11002) via `two_request_types_body()`; `--request-type "Password Reset"` resolves via unique-substring `partial_match` (no ambiguity); note added that the sibling test deliberately uses the distinct `mount_request_types_password_reset` helper — do NOT consolidate the two helpers. |
| F-02: Hardcoded `tests/issue_create_jsm.rs:NNNN` line citations in F2 delta artifacts drift on every test insertion — conflicts with CLAUDE.md anti-drift convention | MEDIUM | Replaced EVERY `tests/issue_create_jsm.rs:NNNN` citation across `prd-delta-384.md`, `bc-3-issue-write.md`, `holdout-scenarios.md`, `spec-changelog.md` with symbol-relative anchors (`async fn` names and `// ─── section banner ───` references). `src/` line references retained (stable design anchors). De-pinned: `issue_create_jsm.rs:1523` → `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` under `// ─── C-01 ───` banner; `issue_create_jsm.rs:1548` → `JR_AUTH_HEADER` env line inside that same `async fn`; `issue_create_jsm.rs:1335` → `JR_AUTH_HEADER` env line inside the pre-#384 Basic-auth 401 test (subsequently renamed by F4 to `async fn test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`); `issue_create_jsm.rs:1309` → `async fn test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (same rename). |
| F-03: H-NEW-JSM-RT-003 `BC:` list omits BC-3.8.014 asymmetrically — the Expected section asserts BC-3.8.014's negative boundary but the `**Note**` only justifies BC-X.8.006/007's absence, leaving BC-3.8.014's absence unjustified | MEDIUM | Extended the `**Note**` in holdout-scenarios.md §H-NEW-JSM-RT-003 to address BC-3.8.014: the holdout asserts only BC-3.8.014's *negative* boundary (OAuth path must not leak the Basic-auth hint); BC-3.8.014's *positive* path is pinned by dedicated `test_jsm_create_basic_auth_401_surfaces_api_token_hint`; BC-3.8.014 intentionally absent from `BC:` list (consistent with how BC-X.8.006/007 are handled). |
| LOW: BC-X.8.006/007 Setup blocks mount `GET /rest/api/3/project/{KEY}` without noting URL-encoding — a key with special chars would need an encoded mock path | LOW | Added one-line note to cross-cutting.md §BC-X.8.006 and §BC-X.8.007 Setup blocks: the project key is URL-encoded by `get_or_fetch_project_meta` (`urlencoding::encode`), so a wiremock `path()` matcher is exact for plain-alphanumeric keys (the named tests use `HELP`); a key with special characters would require an encoded mock path. |

### Adversary Pass-9 Corrections (2026-05-19) — CRITICAL Control-Flow Trace

Applied after ninth fresh-context adversary pass traced the actual control flow in `src/api/client.rs` and found the OAuth test-pinning design from passes 1-8 was structurally impossible. This is a CRITICAL design correction.

**Root cause (traced control flow):**
- `client.rs:696-705`: scope-mismatch body (`"scope does not match"`) → `JrError::InsufficientScope` IMMEDIATELY, before Bearer guard AND before refresh coordinator. This fires for ANY auth scheme.
- `client.rs:718`: `if !auth_header.starts_with("Bearer ")` → `JrError::NotAuthenticated`. Fires ONLY for Basic auth. A Bearer client does NOT take this return.
- `client.rs:727+`: Bearer client with non-scope-mismatch 401 enters the auto-refresh coordinator. In `JR_AUTH_HEADER` seam tests (no keychain tokens), `refresh_oauth_token_with_url` returns raw `anyhow::bail!` (not a `JrError`). The `map_err`'s `e.downcast::<JrError>()` hits `Err(other) => other` — hint never injected.

| Finding | Severity | Resolution |
|---------|----------|-----------|
| C-01: BC-3.8.015 plan "migrate the pre-#384 Basic-auth 401 test to Bearer + generic-expiry body" was IMPOSSIBLE — Bearer + generic-expiry routes through refresh coordinator, fails with raw anyhow (not a JrError), `write:servicedesk-request` hint never injected. | CRITICAL | BC-3.8.015 re-specified: testable contract is scope-mismatch path ONLY (client.rs:696-704 short-circuit → deterministic `JrError::InsufficientScope`). Existing `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (green on `develop`, unmodified) is the BC-3.8.015 pin. Generic-OAuth-401 refresh path is pre-existing, unchanged by #384, out of #384 test scope — stated explicitly in BC-3.8.015. |
| C-02: The pre-#384 Basic-auth 401 test — Basic + generic-401 under #384 produces BC-3.8.014 API-token-expiry hint; old assertion (`write:servicedesk-request`) would fail. Plan to switch to Bearer was impossible (C-01). | CRITICAL | Test REPURPOSED IN PLACE and RENAMED by F4 to `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`: fixture stays `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0`; assertions flipped from `write:servicedesk-request` to BC-3.8.014 API-token-expiry hint; negative assertion that `write:servicedesk-request` is ABSENT added. BC-3.8.014 pin. Required Test Deliverables item 3 updated. |
| C-03: H-NEW-JSM-RT-003 was bound to the pre-#384 Basic-auth 401 test (now renamed `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint`; Bearer + generic-expiry was impossible per C-01). | CRITICAL | H-NEW-JSM-RT-003 RE-BOUND to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (Bearer + scope-mismatch body — deterministic). Holdout rewritten in holdout-scenarios.md. Title updated. Holdout count unchanged (55 — re-bind not add/remove). |
| C-04: BC-X.8.007 Setup specified generic-expiry 401 body for project-GET mock — same defect: Bearer + generic-expiry routes through refresh coordinator, read-scope hint never injected. | CRITICAL | BC-X.8.007 Setup in cross-cutting.md corrected to scope-mismatch body (`{"errorMessages": ["Unauthorized; scope does not match"]}`). WHY explanation added inline. BC-X.8.006 (Basic) UNAFFECTED — Basic never enters refresh path. |
| C-05 (F1 decision reversal): F1 delta analysis §Decision #2 recorded "revise H-NEW-JSM-RT-003 to a Bearer + generic-body fixture." Decision unworkable. | HIGH | Formally reversed in adversary-pass-9: H-NEW-JSM-RT-003 is now the scope-mismatch Bearer test (existing, green, unmodified). The Basic generic-401 test is a BC-3.8.014 pin with flipped assertions. |

### Feature Request Link

- https://github.com/Zious11/jira-cli/issues/384

---

## [1.0.0] - 2026-05-04

### Type: MAJOR

### Summary

Initial L3 PRD release. Brownfield Phase 1 Burst 2 — 540 behavioral contracts imported from Pass 3, sharded into 7 bounded contexts plus cross-cutting. Baseline for all subsequent versions.

### New Requirements

All initial requirements (BC-1.*.* through BC-7.*.*, BC-X.*.*). See README.md Document Map.

### Impact Assessment

- **Affected stories:** None (initial release)
- **Migration needed:** NO
