---
document_type: f1-delta-analysis
phase: phase-f1-delta-analysis
producer: architect
issue: 385
status: draft
created: 2026-05-20
project: jira-cli
mode: BROWNFIELD
intent: enhancement
feature_type: backend
trivial_scope: false
scope: standard
regression_risk: low-medium
severity: N/A
---

# F1 Delta Analysis — Issue #385

## Feature Request

- **Brief:** GitHub issue #385 — "JSM create UX polish: harmonize project-required error (O-08-02), guard empty --request-type (O-08-04), reject --markdown + --field description= conflict (O-08-06), move --type warning post-require_service_desk (O-08-07)"
- **Issue link:** https://github.com/Zious11/jira-cli/issues/385
- **Requested by:** Zious11 (Jared Richards) via adversarial review deferred findings (pass-08 LOW observations)
- **Date:** 2026-05-20
- **Validation source:** `.factory/research/issue-288-pr4-deferred-validation.md` — O-08-02 CONFIRMED (post-pull, lines 652-707), O-08-04 CONFIRMED (lines 303-341), O-08-06 PARTIAL (lines 385-433), O-08-07 CONFIRMED (post-pull, lines 712-796). All four validated against merged PR #381 code on `develop`.

## Problem Summary

Four LOW/UX observations from adversary pass-08 — all validated against the current `develop` branch — are bundled into a single polish issue:

**O-08-02: JSM project-required error is terser than platform sibling.**
`src/cli/issue/create.rs:1891` returns `"project is required for JSM request creation"` (6 words, lowercase, no affordances). The platform sibling at line 146-148 returns `"Project key is required. Use --project or configure .jr.toml. Run \"jr project list\" to see available projects."` (17+ words, mentions `--project`, `.jr.toml`, and a discovery command). The JSM path drops three actionable affordances and has inconsistent capitalization. Verbatim string is pinned by `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (line 1902 of `tests/issue_create_jsm.rs`) — any fix must update that test or be verified against the assertion at line 1943.

**O-08-04: `--request-type ""` (empty string) degrades to "Ambiguous matches all".**
At `src/cli/issue/create.rs:1917`, the numeric bypass checks `!request_type_arg.is_empty() && chars().all(|c| c.is_ascii_digit())` — the `is_empty()` guard makes the condition false for `""`, so empty string falls through to `resolve_jsm_request_type_id`. In `src/partial_match.rs:34-36`, `"<anything>".contains("")` is `true` for every candidate, so every name hits the `Ambiguous` arm, producing "Ambiguous request type — N matches" instead of a clear "request type cannot be empty" message. Confirmed by reading `partial_match.rs:16-43`. No test currently pins or refutes this behavior (grep for `empty`, `O-08-04`, `request_type.*""` in `tests/issue_create_jsm.rs` returns zero results).

**O-08-06: `--field description=<plain text>` + `--markdown` desyncs `isAdfRequest: true`.**
In `src/api/jsm/requests.rs:93-120`, `JsmRequestBuilder::build()` first processes `self.description` → ADF and sets `is_adf_request = true` (lines 94-104). It then iterates `self.extra_fields` and calls `rfv.insert(k.clone(), serde_json::Value::String(v.clone()))` (lines 118-120). If `extra_fields` contains `"description"`, this overwrites the ADF Value with a plain string while `isAdfRequest: true` is still set (line 138-140), producing a desync: `isAdfRequest: true` with a plain-string description. This may produce a Atlassian 400 error OR silently drop ADF formatting — official docs do not confirm the exact behavior. There is no conflict guard in `handle_jsm_create` or `JsmRequestBuilder::build()`. No test covers this scenario. The issue specifies a parse-time rejection at `handle_jsm_create` with exit 64.

**O-08-07: `--type` warning fires pre-`require_service_desk` even on non-JSM projects.**
At `src/cli/issue/create.rs:67-71`, the `--type` warning (and the other 5 platform-only flag warnings at lines 72-96) fires BEFORE `handle_jsm_create` is called at line 98. `require_service_desk` is called at line 1906 (inside `handle_jsm_create`). On a non-JSM project, the user sees BOTH the warning ("warning: --type is ignored when --request-type is set") AND the non-JSM error (exit 64) on stderr. The warning is misleading because the command was never going to dispatch JSM. BC-3.8.010 explicitly permits "need not fire" on early-exit paths — the fix is BC-sanctioned. The existing test `test_jsm_create_type_flag_ignored_with_warning` (line 1242) uses a JSM-mounted project (HELP) and pins warning + success; the non-JSM + `--type` dual-output scenario has no test.

## Classifications

### Intent Classification

**Classified intent:** `enhancement`

**Rationale:** All four findings are UX-polish improvements on already-working code paths. No behavioral regression; no new user capability. O-08-02 improves error message quality. O-08-04 adds an explicit empty-string guard for better diagnostics. O-08-06 adds a parse-time rejection to prevent a latent desync. O-08-07 reorders warning emission to avoid misleading output. These are "improve" operations on existing behavior, not "fix broken" (no user-visible breakage today) and not "add new capability."

### Feature Type Classification

**Classified type:** `backend`

**Rationale:** All changes are in Rust source files (`src/` and `tests/`). No UI screens, no frontend assets, no CI/CD configuration changes. The changes add input-validation guards, reorder code, and update error strings — pure backend.

### Trivial Scope Classification

A change is trivial when ALL of the following are true:

- [ ] Impact boundary: single module, single file, or documentation only — FAILS: at minimum `src/cli/issue/create.rs` + `tests/issue_create_jsm.rs`; O-08-06 may also touch `src/api/jsm/requests.rs`; O-08-02 may extract a shared helper
- [ ] No new BCs needed — FAILS: 2 new BCs estimated (see below); 1 BC modified (BC-3.8.002 error string)
- [x] No architecture change — PASSES: no new modules; additive guards within existing functions
- [x] No new external dependencies — PASSES
- [x] Regression risk: LOW-MEDIUM — BORDERLINE: the O-08-02 fix changes a pinned verbatim error string; requires test update for `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint`

**Classified scope:** `standard`

**Rationale:** Multi-fix bundle touching multiple functions; 2 new BCs needed; at least one existing verbatim-string test must be updated (O-08-02); and BC-3.8.010's warning-position change (O-08-07) requires a regression test for the new intent. Quick dev routing does not apply.

## Impact Assessment

| Dimension | Affected | Details |
|-----------|----------|---------|
| PRD Requirements | BC-3.8.002 modified; 2 new BCs added | BC-3.8.002 errors section: harmonized project-required string must be updated; BC-3.8.016 (empty request-type guard — O-08-04); BC-3.8.017 (`--markdown` + `--field description=` conflict rejection — O-08-06); O-08-07's warning-position change is a refinement to BC-3.8.010 (no new BC needed, behavior is BC-sanctioned) |
| Architecture | 0 new components; optional new helper function | `project_key_required_error(context: &str)` is RECOMMENDED (issue says "recommend extracting" — not mandatory); all changes are guards/reordering within existing functions |
| UX Screens | None | Pure error-message and warning-position improvements |
| Stories | 1 story estimated (see scope recommendation) | All 4 fixes share a single code locus (`handle_jsm_create`, `resolve_jsm_request_type_id`, `requests.rs`); no external dependency between fixes; a single atomic story is appropriate |
| Existing Tests | 1 test MUST be updated; 1 must be monitored | `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (O-08-02: pinned verbatim string will change); `test_jsm_create_type_flag_ignored_with_warning` (O-08-07: warning still fires on JSM path, test must remain green; new companion test needed for non-JSM scenario) |
| Verification Properties | None — no VP directory in use; BC-level anchoring sufficient | No proptest candidates; all fixes are deterministic input-validation guards |

## Affected BC Mapping

### BCs Modified

| BC ID | File | Nature of Change |
|-------|------|-----------------|
| BC-3.8.002 | `bc-3-issue-write.md` | Errors section: update the JSM-path "project is required for JSM request creation" verbatim string to harmonized form matching BC-3.8.016 (the new BC). The existing cross-reference at line 586 ("No project configured → exit 64 with "project is required for JSM request creation" hint") must be updated to match the new harmonized string |
| BC-3.8.010 | `bc-3-issue-write.md` | Postcondition: clarify that warnings fire AFTER `require_service_desk` succeeds (move intent). No new BC needed — BC-3.8.010 already permits "need not fire" on early-exit paths via its existing language; the postcondition section should explicitly state the chosen implementation: warnings fire inside `handle_jsm_create` AFTER `require_service_desk` returns `Ok` |

### New BCs Needed

| Proposed BC ID | File | Description |
|----------------|------|-------------|
| BC-3.8.016 | `bc-3-issue-write.md` | `--request-type ""` (empty string after trim) exits 64 with "request type cannot be empty" (or equivalent clear message). Guard fires BEFORE `resolve_jsm_request_type_id` (equivalently, before the numeric bypass check). Exit code 64. No HTTP calls issued. |
| BC-3.8.017 | `bc-3-issue-write.md` | `--markdown` + `--field description=<any value>` (where the `--field` key is literally `"description"`) rejected at parse-time in `handle_jsm_create` with exit 64 and message indicating the ADF conflict. The conflict is detected by checking `extra_fields.contains_key("description")` after `parse_field_kv`, when `markdown == true`. May produce JSM 400 OR silently drop ADF formatting if not guarded — rejection at parse-time is the correct fix. |

**Note on O-08-02 harmonized string:** The recommended harmonized JSM project-required string is:
`"Project key is required for JSM request creation. Use --project or configure .jr.toml. Run \"jr project list\" to see available JSM projects."`
This adds the `--project`/`.jr.toml`/`jr project list` affordances and the path-context label ("for JSM request creation"), sentence-cases the opening, and preserves the JSM-specific context. The optional shared helper `project_key_required_error(context: &str)` would produce both platform and JSM strings from a common template to prevent future drift.

**Note on O-08-07 warning position:** BC-3.8.010 does NOT need a new BC — moving the warnings inside `handle_jsm_create` (after `require_service_desk` returns `Ok`) is explicitly permitted by BC-3.8.010's "need not fire" language and is a refinement within the current spec. However, the postcondition section of BC-3.8.010 should be updated to record the chosen intent as the canonical implementation.

## Files Changed

### New Files

| File Path | Purpose |
|-----------|---------|
| None | No new source modules needed; all changes are additive guards or reordering within existing files |

### Modified Files

| File Path | Change Type | Risk |
|-----------|-------------|------|
| `src/cli/issue/create.rs` | Logic changes: (1) O-08-02: harmonize JSM project-required error string at line 1891 (optionally extract helper); (2) O-08-04: add explicit empty-string guard before numeric bypass check at line 1916-1930 in `handle_jsm_create`; (3) O-08-06: add conflict guard for `extra_fields.contains_key("description")` when `markdown == true`, after `parse_field_kv` call at line 1962, exit 64; (4) O-08-07: move 6 warning `eprintln!` blocks (lines 67-96 of `handle_create`) INTO `handle_jsm_create` AFTER `require_service_desk` call (line 1906) | LOW-MEDIUM — the O-08-02 change alters a pinned verbatim string (test must be updated); O-08-04/06 are new guards with no existing code to regress; O-08-07 reorders code but preserves all BC-3.8.010/.011 behaviors on the JSM path |
| `tests/issue_create_jsm.rs` | MODIFIED (1 test assertion update) + NEW tests: (1) Update `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (line 1943 assertion) to assert the harmonized error string; (2) NEW test: `test_jsm_create_empty_request_type_exits_64` (O-08-04 pin); (3) NEW test: `test_jsm_create_markdown_field_description_conflict_exits_64` (O-08-06 pin); (4) NEW test: `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project` (O-08-07 pin — asserts warning is ABSENT when project is non-JSM and `--type` is set) | LOW — modifying one assertion (string match); new tests add coverage with no modification to passing tests |

### Optional New File (if helper extracted)

| File Path | Change Type | Risk |
|-----------|-------------|------|
| `src/cli/issue/helpers.rs` | MODIFIED: add `pub(super) fn project_key_required_error(context: &str) -> JrError` helper | LOW — additive; used by both `handle_create` (platform path, line 144-150) and `handle_jsm_create` (JSM path, line 1882-1891) |

### Dependent Files (unchanged but depend on modified files)

| File Path | Depends On | Regression Risk |
|-----------|------------|----------------|
| `src/api/jsm/requests.rs` | May be unchanged (conflict guard is in `handle_jsm_create`, not `build()`) | NONE — if guard is in `handle_jsm_create`, `JsmRequestBuilder::build()` is unmodified |
| `src/api/jsm/servicedesks.rs` | Unchanged — `require_service_desk` interface is unchanged; call site in `handle_jsm_create` moves but doesn't change signature | NONE |
| `src/cli/queue.rs` | `require_service_desk` — unchanged | NONE |
| `src/cli/requesttype.rs` | `require_service_desk` — unchanged | NONE |
| `src/partial_match.rs` | `partial_match` — unchanged; empty-string guard is in the caller (`handle_jsm_create`), not in `partial_match` itself | NONE — note: empty-string behavior of `partial_match` itself is unchanged; the guard is localized per the issue's recommendation |
| `tests/issue_write_holdouts.rs` | H-NEW-JSM-RT-004 (BC-3.8.010) — must remain green; test uses JSM project (HELP); O-08-07 moves warnings post-`require_service_desk` but warnings still fire on the JSM path, so existing holdout is unaffected | LOW |

## Files NOT Changed (Regression Baseline)

These files must not be modified during implementation. All their tests must continue to pass:

- `src/api/client.rs` — `is_oauth_auth()` predicate (added in #384) unchanged; no new predicate needed
- `src/api/auth.rs` — OAuth scopes unchanged
- `src/api/jira/issues.rs` — platform create path entirely unchanged
- `src/api/jsm/requests.rs` — `JsmRequestBuilder::build()` unchanged (guard lives in `handle_jsm_create`)
- `src/api/jsm/servicedesks.rs` — `require_service_desk` interface unchanged; only call-site in `handle_jsm_create` is reordered
- `src/adf.rs`, `src/duration.rs`, `src/output.rs`, `src/jql.rs` — unrelated utilities
- `src/partial_match.rs` — `partial_match` itself is unchanged; empty-string guard is in the caller
- `src/error.rs` — `API_TOKEN_EXPIRY_HINT` and `JrError` unchanged; no new error variant needed (exit 64 uses existing `JrError::UserError`)
- `tests/issue_commands.rs` — platform create coverage; must remain green
- `tests/issue_create_json.rs` — platform create JSON shape; must remain green
- `tests/issue_write_holdouts.rs` — existing holdout suite; H-NEW-JSM-RT-004 (BC-3.8.010 warning) must remain green
- `tests/queue.rs` — adjacent JSM read path; unmodified
- `tests/requesttype_commands.rs` — requesttype list/fields; calls `require_service_desk` for project-type check; unmodified
- `tests/api_client.rs` — `JiraClient` tests; no client-level changes
- `tests/jsm_request_api.rs` — `JsmRequestBuilder` unit/proptest; unchanged if conflict guard is in `handle_jsm_create`
- `src/cli/issue/list.rs`, `workflow.rs`, `links.rs`, `assets.rs` — unrelated issue subcommands
- `src/cli/board.rs`, `src/cli/sprint.rs`, `src/cli/worklog.rs`, `src/cli/team.rs`, `src/cli/user.rs` — unrelated subcommands
- `CLAUDE.md` — no new `JR_*` env vars introduced; no new gotchas requiring documentation

## Risk Assessment

| Risk Type | Level | Rationale |
|-----------|-------|-----------|
| Regression | LOW-MEDIUM | **O-08-02 (MEDIUM):** One existing test (`test_jsm_create_missing_project_exits_64_with_jsm_specific_hint`, line 1943) asserts the verbatim string `"project is required for JSM request creation"`. This test MUST be updated to the harmonized string in the same commit. Until updated, the test will fail — confirming the change is correct. **O-08-04 (LOW):** No existing test covers the empty-string scenario; new guard adds behavior without touching anything passing. **O-08-06 (LOW):** No existing test covers the `--markdown` + `--field description=` scenario; new guard adds rejection without touching passing tests. **O-08-07 (LOW):** Moving 6 warning blocks inside `handle_jsm_create` after `require_service_desk` does NOT affect the JSM path (warnings still fire when project is JSM and flags are set); `test_jsm_create_type_flag_ignored_with_warning` (line 1242) uses a JSM project and must remain green — the change does not break this test. The only behavior change is on the non-JSM path (warnings suppressed), which has no test today. |
| Architecture | NONE | No new modules, no interface changes, no new external dependencies. Optional `project_key_required_error` helper is additive and local to `cli/issue/`. |
| Security | NONE | All changes are on input-validation and error-message surfaces. No authentication, authorization, or token-handling logic is touched. |
| Performance | NONE | New guards are O(n) string comparisons on inputs that have already been parsed. Zero impact on hot paths. |
| Spec Drift (DRIFT-001) | LOW | BC-3.8.002 error-string cross-reference at line 586 of `bc-3-issue-write.md` quotes the old JSM string verbatim; must be updated to the harmonized form in F2. Run `scripts/check-spec-counts.sh` after F2 spec work. |

## Regression Baseline

- **Total tests in `tests/issue_create_jsm.rs`:** 39 test functions (as of `develop` at analysis time)
- **Tests in risk zone (require update or close monitoring):**
  1. `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (O-08-02: assertion at line 1943 asserts `"project is required for JSM request creation"`; MUST be updated to harmonized string in F4) — WILL BE UPDATED
  2. `test_jsm_create_type_flag_ignored_with_warning` (O-08-07: warning fires on JSM project path → test remains green after O-08-07 move; verify in F4) — MUST REMAIN GREEN
  3. `test_jsm_create_ambiguous_request_type_exits_64` (O-08-04 adjacent: tests genuine ambiguity, not empty-string; must remain green) — MUST REMAIN GREEN
- **Tests in risk zone (no update needed, green verification only):**
  - All 37 remaining tests in `tests/issue_create_jsm.rs` (none touch the modified code paths)
  - `tests/issue_write_holdouts.rs` — H-NEW-JSM-RT-004 (BC-3.8.010) must remain green
  - `tests/jsm_request_api.rs` — `JsmRequestBuilder` proptest C.2 (BC-3.8.006 description/ADF presence) must remain green; `build()` is unchanged
- **New tests to be added in F4:**
  1. `test_jsm_create_empty_request_type_exits_64` — O-08-04 pin; exits 64 with explicit empty-string message; no HTTP; no `partial_match` call
  2. `test_jsm_create_markdown_field_description_conflict_exits_64` — O-08-06 pin; `--markdown --field description=plain` exits 64; no HTTP
  3. `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project` — O-08-07 pin; non-JSM project + `--type Task` + `--request-type X`: warning MUST NOT appear; exit 64 from `require_service_desk`; asserts stderr does NOT contain `"warning: --type is ignored"` AND DOES contain the non-JSM-project error

## Scope Recommendation

- **Mode:** Feature Mode — standard single-cycle delivery (F1 through F7)
- **Estimated new BCs:** 2 new (`BC-3.8.016`, `BC-3.8.017`); 2 BCs modified (`BC-3.8.002` error string, `BC-3.8.010` postcondition clarification)
- **Estimated new stories:** 1 story. All four fixes share the same code locus (`handle_jsm_create` and its associated resolver/builder) and have no inter-fix dependencies that require separate worktrees. A single atomic story covering all four fixes is appropriate.
- **Story scope:** (1) Harmonize JSM project-required error string at line 1891 (optionally extract helper) + update `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` assertion. (2) Add empty-string guard in `handle_jsm_create` before numeric bypass + add `test_jsm_create_empty_request_type_exits_64`. (3) Add `--markdown` + `--field description=` conflict guard in `handle_jsm_create` after `parse_field_kv` + add `test_jsm_create_markdown_field_description_conflict_exits_64`. (4) Move 6 platform-only flag warning blocks from `handle_create:67-96` into `handle_jsm_create` after `require_service_desk` + add `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project`.
- **Can parallelize:** No — all four fixes land in the same function (`handle_jsm_create`) and should be delivered atomically to avoid partial-state conflicts.

## Open Questions

1. **O-08-02: Helper extraction decision.** The issue says "recommend extracting a `project_key_required_error(context: &str)` helper." This is optional — the harmonized string can also be inlined at line 1891 without a helper. If extracted, the helper lives in `src/cli/issue/helpers.rs` and is called from both `handle_create` (line 144) and `handle_jsm_create` (line 1882). Recommend F3/F4 make this call based on whether the deduplication benefit outweighs the extraction overhead.

2. **O-08-06: Conflict guard placement.** The issue says "reject the `--markdown` + `--field description=X` combination at parse-time in `handle_jsm_create`, exit 64." This means the guard is in `handle_jsm_create` (after `parse_field_kv` at line 1962), NOT inside `JsmRequestBuilder::build()`. If placed in `build()`, the proptest suite in `tests/jsm_request_api.rs` would need to be extended. Recommend the `handle_jsm_create` placement to keep `JsmRequestBuilder` as a pure builder with no validation responsibility (consistent with its current design).

3. **O-08-06: Exact rejection message wording.** The issue says "reject the `--markdown` + `--field description=X` combination." Recommended wording: `` "`--field description=...` cannot be combined with `--markdown`: it would overwrite the ADF description with plain text, producing a malformed request (may result in a JSM 400 error or silently dropped ADF formatting). Pass `--description` with `--markdown`, or omit `--markdown`." `` — this avoids claiming "Atlassian would 400" (per citation discipline in CLAUDE.md).

4. **O-08-07: Should ALL 6 warnings move or only `--type`?** The issue says "move the BC-3.8.010/011 warnings." BC-3.8.010 covers `--type`; BC-3.8.011 covers the remaining 5 (`--team`, `--points`, `--parent`, `--to`, `--account-id`). For consistency, all 6 should move together. Moving only `--type` would create an asymmetry where 5 of the 6 warnings still fire pre-`require_service_desk`. Recommend moving all 6 in a single pass — this is also what the issue's "option A" implies.

5. **Spec count check:** After F2 spec work, run `scripts/check-spec-counts.sh` and `scripts/check-bc-cumulative-counts.sh`. `total_bcs` in `bc-3-issue-write.md` frontmatter will increase by 2 (from 93 to 95); `definitional_count` will increase by 2 (from 64 to 66). The 3.8 subdomain header count ("15 behavioral contracts") increases to 17.
