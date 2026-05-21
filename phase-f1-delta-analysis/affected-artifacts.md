---
document_type: f1-delta-analysis
phase: phase-f1-delta-analysis
producer: business-analyst
issue: 388
status: draft
created: 2026-05-20
project: jira-cli
mode: BROWNFIELD
intent: enhancement
feature_type: backend
trivial_scope: false
scope: standard
regression_risk: medium
severity: N/A
---

# F1 Delta Analysis — Affected-Artifact Mapping — Issue #388

## Feature Summary (Reshaped)

Issue #388 originally asked for sub-task ↔ standard issue-type conversion
via `jr issue edit --type`. Research (`.factory/research/issue-388-subtask-conversion.md`)
confirmed NO public Jira Cloud REST API supports cross-hierarchy conversion.
The feature is reshaped to **Option A**:

1. **New behavior — cross-hierarchy 400 on `edit --type`:** When
   `jr issue edit KEY --type X` returns HTTP 400, fetch the source issue's
   `issuetype.subtask` flag and resolve the target type's `subtask` flag
   from project metadata. If the flags differ (cross-hierarchy mismatch),
   emit a `CrossHierarchy` hint: explain why the API rejects it, cite
   JRACLOUD-27893, and point users to the Jira web UI ellipsis-menu
   conversion action. If flags match or resolution is indeterminate, emit
   a typo hint (pointing to `jr project types`). If neither subtask-flag
   comparison nor the best-effort English substring is conclusive, surface
   Atlassian's raw error body.

2. **Bug fix — `--no-parent` subtask hint at `create.rs:834`:** The
   existing hint references a non-existent endpoint
   `PUT /rest/api/3/issue/{key}/convert`. Replace it with the shared,
   corrected web-UI workaround hint (no fake endpoint, no `jr api` command).

Scope is **single-key `jr issue edit KEY --type X`** only. The bulk edit
path and multi-key path are NOT affected; no conversion feature is
introduced.

---

## Intent Classification

**Classified intent:** `enhancement` (with a bundled bug fix)

**Rationale:** The primary deliverable is improved error surfacing for an
existing command path that already works for same-category type changes.
The bug fix at `create.rs:834` is a correctness repair (a fake endpoint
reference) bundled into the same PR because the corrected hint text is
shared. Neither change adds new user capabilities or removes existing
ones. The classification matches the precedent set by issue #384 (JSM 401
auth-aware hints), which is the closest analogue: same file, same
"improve error hint" intent, classified `enhancement`.

**Feature type:** `backend` — all changes are in Rust source files
(`src/cli/issue/create.rs`) and new integration tests
(`tests/issue_edit_no_parent.rs`, new test file for edit-type error paths).

---

## Trivial Scope Check

A change is trivial when ALL of the following are true:

- [ ] Single module, single file, or documentation only — FAILS: at least
      `src/cli/issue/create.rs` (logic change in `handle_edit` + bug fix
      in hint text) plus new/modified test files; may require a project
      metadata fetch added to `src/api/jira/projects.rs` or
      `src/api/jira/issues.rs`.
- [ ] No new BCs needed — FAILS: 3 new BCs (cross-hierarchy hint,
      typo hint, corrected `--no-parent` hint).
- [ ] No architecture change — PASSES: additive error-path logic, no new
      modules required (reuses existing `get_issue` and project-meta
      infrastructure).
- [ ] No new external dependencies — PASSES.
- [ ] Regression risk LOW — FAILS: MEDIUM — `tests/issue_edit_no_parent.rs`
      T-06 asserts the OLD fake-endpoint hint text and must be updated;
      the `is_subtask_parent_error` helper must remain non-regressed.

**Classified scope:** `standard`

**Rationale:** Multi-path logic change, 3 new BCs, test update to an
existing regression pin (T-06 in `issue_edit_no_parent.rs`), and one
additional HTTP call (source-issue `issuetype` fetch) on the edit-error
path. Does not meet trivial threshold.

---

## Affected BC Mapping

### BCs Modified (UNCHANGED behavior, annotation/cross-reference update only)

| BC ID | File | Nature of Change |
|-------|------|-----------------|
| BC-3.4.003 | `bc-3-issue-write.md` | Errors section: add cross-reference to new BC-3.4.010 (cross-hierarchy 400 → hint) and BC-3.4.011 (typo 400 → hint). No behavioral change to the primary success path or ADF description behavior. |
| BC-3.4.006 | `bc-3-issue-write.md` | No change needed — label-coalesce path is orthogonal. UNCHANGED. |

**Justification:** BC-3.4.003 covers `issue edit` PUTs. Its Errors section
currently does not describe behavior for 400 responses on `--type`. Adding
cross-references to the new BCs keeps the error-handling surface fully
documented without changing any existing asserted behavior.

### BCs UNCHANGED (in regression risk zone, no modification needed)

| BC ID | File | Reason UNCHANGED |
|-------|------|-----------------|
| BC-3.4.001 | `bc-3-issue-write.md` | `handle_open` URL composition — entirely unrelated to edit errors. |
| BC-3.4.004 | `bc-3-issue-write.md` | ADF bold-text on wire — success path, not error path. |
| BC-3.4.005 | `bc-3-issue-write.md` | Multiple fields in single body — success path, unchanged. |
| BC-3.4.006 | `bc-3-issue-write.md` | Label coalesce — different code branch from `--type`. |
| BC-3.4.007 | `bc-3-issue-write.md` | `--description` / `--description-stdin` conflict — clap layer, unchanged. |
| BC-3.4.008 | `bc-3-issue-write.md` | `--points` / `--no-points` conflict — clap layer, unchanged. |
| BC-3.4.009 | `bc-3-issue-write.md` | Bulk-poll timeout task_id pin — bulk path, unrelated. |
| BC-X.3.004 | `cross-cutting.md` | 400 with field-specific error → `field: message` (sorted). This BC covers the generic 400 formatting path; the new behavior is a *subsequent enrichment* that fires only when the generic 400 is detected AND the subtask-flag mismatch confirms a cross-hierarchy error. The base BC is unchanged. |
| BC-X.3.007 | `cross-cutting.md` | Error messages must suggest next step — the new hints satisfy this invariant; BC text is unchanged. |

---

### New BCs Required

**Numbering rationale:** The last allocated BC in Section 3.4 is
BC-3.4.009 (issue #340). Issue #388 adds BCs in `bc-3-issue-write.md` as
siblings. Following the sequential-within-subsection convention used by
issues #384 (BC-3.8.014, BC-3.8.015) and #385 (BC-3.8.016, BC-3.8.017),
the next available IDs are BC-3.4.010 and BC-3.4.011. The corrected
`--no-parent` hint is a modification to an EXISTING behavior (T-06 in
`issue_edit_no_parent.rs`), not a new observable behavior, so it does NOT
require a new BC — it requires updating the implementation assertion in
the existing test. However, because the hint text changes in a
regression-pinned test, the BC-3.4.010 body should note the `--no-parent`
path shares the same hint wording.

| Proposed BC ID | File | Description | Anchor Justification |
|----------------|------|-------------|----------------------|
| BC-3.4.010 | `bc-3-issue-write.md` | `jr issue edit KEY --type X` HTTP 400 + source-vs-target subtask-flag mismatch (cross-hierarchy) → exit 1 with conversion hint: explains API limitation, cites JRACLOUD-27893, points to Jira web UI ellipsis-menu "Convert to issue / Convert to sub-task" action. Hint wording avoids quoting exact UI label (locale/version instability per research addendum A3). The subtask-flag mismatch is the PRIMARY signal; the English "issue type selected is invalid" substring is secondary/best-effort (locale-fragile per addendum A2). Same corrected web-UI hint emitted for `--no-parent` subtask-bound 400 (bug fix at `create.rs:834` — replaces fake `PUT /rest/api/3/issue/{key}/convert` endpoint reference). | BC-3.4.010 covers the cross-hierarchy edit-type 400 error path because the product brief (research file `issue-388-subtask-conversion.md` §Option A) specifies: "Detect the boundary-crossing 400 ... Emit a hint that (a) explains why it failed, (b) points to the web-UI wizard, (c) cites JRACLOUD-27893." This is grounded in the research scope, not invented. |
| BC-3.4.011 | `bc-3-issue-write.md` | `jr issue edit KEY --type X` HTTP 400 + source-vs-target subtask-flag MATCH (same hierarchy level, not a cross-hierarchy error) or subtask-flag resolution indeterminate → exit 1 with typo/workflow hint: suggests verifying the type name with `jr project types`, and surfaces raw Atlassian error text. This prevents the cross-hierarchy hint from firing falsely on plain typos (research addendum A1 — same "issue type selected is invalid" message is NOT a reliable boundary classifier). | BC-3.4.011 covers the same-hierarchy 400 fallback because research addendum A1 explicitly flags that the bare error substring over-claims; a typo path must emit a distinct, less specific hint. Grounded in `issue-388-subtask-conversion.md` §Addendum A1 (REFINE verdict). |

**Note on BC-3.4.010 and the `--no-parent` bug fix:** The `--no-parent`
hint at `create.rs:834` and the `edit --type` cross-hierarchy hint (BC-3.4.010)
share the same corrected web-UI wording. The `--no-parent` path is not a
new BC because BC-3.4.003 and T-06 in `tests/issue_edit_no_parent.rs`
already anchor the subtask-parent-clear error behavior. T-06's
`stderr.contains("convert")` assertion will REMAIN true (the corrected
hint still contains a reference to the conversion action); only the fake
endpoint text (`jr api /rest/api/3/issue/{key}/convert ...`) will be
removed, and a JRACLOUD-27893 citation + web-UI pointer will be added.
T-06 does not pin the fake endpoint string, so it does not fail mechanically
— but the implementation assertion should be strengthened to a
literal-pin on `JRACLOUD-27893` (see Existing Tests section below).

---

## Story Identification

### Regression-Risk Zone Stories

Stories whose implementation directly touched `src/cli/issue/create.rs` or
the edit handler, making their associated test files most likely to contain
tests that interact with the changed code:

| Story ID | Title | Risk Reason |
|----------|-------|-------------|
| S-2.02 | BC-3 issue-write holdout suite | Touched `handle_edit` error paths; `tests/issue_write_holdouts.rs` in risk zone |
| S-345 | Label-coalesce JSON builder + proptest | Touched `create.rs` bulk-labels paths; `tests/issue_bulk_pr2.rs` in risk zone |
| S-340 | Pin task_id in bulk-poll timeout | Touched bulk path in `create.rs`; `tests/bulk_deadline_propagation.rs` in risk zone |
| issue-288-pr4-dispatch | JSM --request-type dispatch fork | Major `create.rs` refactor — `tests/issue_create_jsm.rs` in risk zone |
| S-383 | Platform-inverse warnings (--field, --on-behalf-of) | Touched `create.rs` warning path; `tests/issue_create_jsm.rs` in risk zone |
| S-385 | JSM input validation UX polish | Touched `create.rs` guard ordering; `tests/issue_create_jsm.rs` in risk zone |

### Stories NOT in Regression Risk Zone (but referencing Section 3.4)

| Story ID | Reason Out of Scope |
|----------|---------------------|
| S-0.01 | `handle_open` URL fix — different function, `workflow.rs` |
| S-2.01 | BC-2 issue-read — no write path touched |
| S-3.01 | auth.rs shard split — no create.rs |

---

## Existing Tests — Enumeration and Impact

### Tests Directly Impacted (MUST UPDATE)

| Test File | Test Name | Current State | Required Change |
|-----------|-----------|---------------|-----------------|
| `tests/issue_edit_no_parent.rs` | `test_subtask_parent_clear_surfaces_400_with_convert_hint` (T-06) | Asserts `stderr.contains("convert") \|\| stderr.contains("subtask") \|\| stderr.contains("standard issue")`. Currently PASSES because `create.rs:834` emits text containing "convert", "subtask", and "standard issue". | After fix, hint text changes (fake endpoint removed, web-UI + JRACLOUD-27893 added). The existing `contains("convert")` check will still pass since "Convert" appears in the new web-UI reference wording. However, the test should be STRENGTHENED to a regression-literal pin: add assertion that `stderr.contains("JRACLOUD-27893")` and that `stderr` does NOT contain `/rest/api/3/issue/` (which would indicate the fake endpoint text survived). Mirror of the `JRACLOUD-95368` literal-pin pattern documented in CLAUDE.md. |

### Tests Confirmed UNCHANGED (regression baseline)

| Test File | Scope | Why Unchanged |
|-----------|-------|---------------|
| `tests/issue_edit_no_parent.rs` T-01, T-02, T-03, T-04, T-05, T-07, T-08 | `--no-parent` flag behavior | All test success paths, clap conflicts, and PUT body shapes — no interaction with error hint text |
| `tests/issue_bulk_pr2.rs` | All bulk-edit paths (multi-key, --jql, --dry-run, --type bulk) | Bulk path is NOT affected by #388; single-key `--type` error handling is in `handle_edit`, not bulk dispatch |
| `tests/issue_commands.rs` BC-3.3.x, BC-3.4.x | Platform create, issue edit ADF/multiple-fields success paths | Success paths; `edit_issue` 204 path unchanged |
| `tests/issue_create_jsm.rs` | All JSM request create paths | JSM dispatch path does not include the `is_cross_hierarchy_type_error` logic; JSM uses `requestTypeId` not `--type` |
| `tests/issue_create_json.rs` | Platform create JSON shape | Create path, not edit |
| `tests/issue_write_holdouts.rs` | H-006, H-007, H-008, H-014 — move/transition and assign holdouts | Different subcommands |

### New Test File Required

| New Test File | Purpose |
|---------------|---------|
| `tests/issue_edit_type_errors.rs` | Integration tests for the new `--type` 400 error-enrichment paths. Required test functions: (1) `test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint` — mocks GET issue (standard, `issuetype.subtask: false`), GET project issue-types (target is subtask), PUT returns 400 "issue type selected is invalid" → asserts exit 1 + `JRACLOUD-27893` in stderr + no fake endpoint. (2) `test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint` — reverse direction. (3) `test_edit_type_typo_same_hierarchy_surfaces_typo_hint` — mocks GET issue (standard), GET project types, PUT returns 400 → both flags are `subtask: false` → asserts exit 1 + `jr project types` or raw error + NO `JRACLOUD-27893` in stderr. (4) `test_edit_type_subtask_flag_indeterminate_surfaces_raw_error` — GET issue succeeds but project types returns 5xx → asserts exit 1 + raw Atlassian error body surfaced. (5) `test_edit_type_cross_hierarchy_hint_does_not_contain_fake_endpoint` — a regression-literal pin asserting `/rest/api/3/issue/` is ABSENT from the hint (mirrors the `JRACLOUD-95368` stderr-literal pin pattern). |

---

## Verification Properties (VP-NNN)

**No VP directory exists** in `.factory/specs/`. This was confirmed in
`.factory/phase-f2-spec-evolution/verification-delta.md` §1 for issue
#288, and the structure has not changed. Property-level correctness on
this project is anchored at BC level via integration tests and inline
proptest blocks.

**Proptest assessment for #388:**

- The new `is_cross_hierarchy_type_error()` detector (if extracted as a
  pure function) is a strong proptest candidate: it takes a source
  `issuetype.subtask: bool`, a target `issuetype.subtask: bool`, and an
  error message string, and returns a classification. A proptest property
  `for any (src_subtask: bool, tgt_subtask: bool, msg: String), if
  src_subtask != tgt_subtask then result is CrossHierarchy` would pin the
  discriminant invariant.
- `is_subtask_parent_error()` already exists as a pure helper at
  `create.rs:1159`. The new companion detector should follow the same
  pattern (a `fn is_cross_hierarchy_type_error(err, src_is_subtask,
  tgt_is_subtask) -> Classification` pure function).
- **Recommendation:** Add an inline proptest in `src/cli/issue/create.rs`
  in the existing `mod proptests` (or `mod tests`) block, mirroring the
  established project pattern.

**No new VP-NNN artifacts are required.** If the F2 (spec-evolution) pass
opts to codify the proptest property as a VP body, it should use the
pattern established by issue #288's `verification-delta.md`.

---

## Risk Assessment

| Risk Type | Level | Rationale |
|-----------|-------|-----------|
| Regression: T-06 fake-endpoint pin | MEDIUM | T-06 in `issue_edit_no_parent.rs` does NOT pin the fake endpoint string (it checks `stderr.contains("convert")`), so it will not mechanically break. But an implementer could overlook strengthening it to a literal pin. The recommended new assertion (`!stderr.contains("/rest/api/3/issue/")`) must be added explicitly. |
| False-positive cross-hierarchy hint | MEDIUM | Research addendum A1: the "issue type selected is invalid" error is NOT boundary-specific. The subtask-flag comparison is the mandatory primary gate. If the project-types fetch fails (5xx, 401, cache miss), the detector must degrade gracefully to the raw error surface (path 4 in new test file), not to the conversion hint. |
| Additional HTTP on edit-error path | LOW | The new behavior adds a `GET /rest/api/3/issue/{key}?fields=issuetype` and a project-types fetch ONLY on the 400 error path — zero impact on the success path. Rate-limit and latency risk is negligible. |
| i18n fragility | LOW | Research addendum A2 flags locale-fragile string matching. The subtask-flag comparison is locale-independent; the English substring is secondary. Implementation must follow this discipline. |
| Bulk path regression | NONE | The bulk path (`handle_edit_bulk_fields`, `issue_bulk_pr2.rs`) is NOT affected; it uses `--type` but does not have the cross-hierarchy enrichment logic. |
| Security | NONE | Error hint quality improvement only; no auth path changes, no new credentials handling. |

---

## Files Changed

### Source Files Modified

| File Path | Change Type | Risk |
|-----------|-------------|------|
| `src/cli/issue/create.rs` | Logic change in `handle_edit`: add cross-hierarchy 400 detector after `edit_result` error check; fix hint text at line 834 (`--no-parent` subtask path) | MEDIUM — touches the edit error path; success path byte-for-byte unchanged |

### Source Files Possibly Modified (TBD in F2/F4)

| File Path | Possible Change | Decision Point |
|-----------|-----------------|---------------|
| `src/api/jira/issues.rs` | May need a targeted `get_issue_type(key)` helper that fetches only `issuetype` fields (reuses or extends existing `get_issue`) to avoid fetching the full issue payload on an error path | F4 architect decision: reuse `get_issue` with `fields=issuetype` vs. add a thin helper |
| `src/api/jira/projects.rs` | May need `get_project_issue_types(project_key)` to resolve target type's subtask flag from project metadata; OR reuse `get_or_fetch_project_meta` from `src/cache.rs` | F4 architect decision: cache-first fetch is preferred to avoid extra HTTP on error path |

### Test Files Modified

| File Path | Change Type |
|-----------|-------------|
| `tests/issue_edit_no_parent.rs` | MODIFIED: strengthen T-06 assertions — add `JRACLOUD-27893` literal-pin + `!stderr.contains("/rest/api/3/issue/")` regression guard |

### New Test Files

| File Path | Purpose |
|-----------|---------|
| `tests/issue_edit_type_errors.rs` | New integration tests for `--type` cross-hierarchy 400 enrichment (5 test functions enumerated above) |

### Files NOT Changed (Regression Baseline)

- `src/cli/issue/workflow.rs` — move/transition/assign/open; unrelated
- `src/cli/issue/list.rs` — read path; unrelated
- `src/cli/issue/links.rs` — link/unlink; unrelated
- `src/api/jsm/servicedesks.rs` — JSM service-desk layer; unrelated
- `src/api/jsm/queues.rs` — JSM queue path; unrelated
- `src/api/client.rs` — no new methods needed (no auth-predicate gate unlike #384)
- `tests/issue_create_jsm.rs` — JSM create path; unrelated
- `tests/issue_bulk_pr2.rs` — bulk edit; unrelated
- `tests/issue_commands.rs` — BC-3.3.x, BC-3.4.x success paths; unrelated
- `tests/issue_create_json.rs` — platform create JSON shape; unrelated
- `CLAUDE.md` — no new gotcha entry required UNLESS the
  `is_cross_hierarchy_type_error` detector introduces a new test-seam
  env var (not anticipated; error path logic, no env-var seam needed)

---

## BC ID Registry (New Anchors Proposed)

| Proposed BC ID | Section File | Status |
|----------------|-------------|--------|
| BC-3.4.010 | `bc-3-issue-write.md` | NEW — cross-hierarchy 400 → conversion hint |
| BC-3.4.011 | `bc-3-issue-write.md` | NEW — same-hierarchy 400 → typo hint + raw error |

**Total new BCs: 2**

The current `definitional_count` for Section 3 is 66 (as of `bc-3-issue-write.md`
frontmatter). After this issue: 68. The `total_bcs` for Section 3 is 95.
After this issue: 97. `BC-INDEX.md` `total_bcs` would move from 575 → 577.

**Note:** These counts must be updated atomically by the product-owner in
F2 using `scripts/check-bc-cumulative-counts.sh` to prevent DRIFT-002.

---

## Precedent Reference: Issue #384

Issue #384 ("JSM 401 auth-aware error hints") is the closest precedent:

- Same file: `src/cli/issue/create.rs`
- Same pattern: error-path enrichment gated on a detectable condition
  (auth type for #384; subtask-flag comparison for #388)
- Same F1 output format and BC numbering approach
- Issue #384 created 4 new BCs; issue #388 creates 2 new BCs (smaller
  scope: single command path, single error kind, no cross-module predicate)
- Issue #384 had 1 story; issue #388 is expected to have 1 story

The primary difference: #384 added a new method (`is_oauth_auth`) on
`JiraClient`; #388 adds only a pure function in `create.rs` (no new
public API surface on `JiraClient`).

---

## Scope Recommendation

- **Mode:** Feature Mode — standard single-cycle delivery
- **Estimated new BCs:** 2 new (`BC-3.4.010`, `BC-3.4.011`); 1 BC annotation update (`BC-3.4.003`)
- **Estimated new stories:** 1 story covering: implement `is_cross_hierarchy_type_error` detector, add 2 new HTTP calls on edit-400 error path (issue-type fetch + project-types fetch), fix `create.rs:834` hint, update T-06 assertions in `issue_edit_no_parent.rs`, and add `tests/issue_edit_type_errors.rs` (5 new test functions)
- **Can parallelize:** No — single substrate (the detector logic) drives both fix sites; implement atomically

## Open Questions for F2/F4

1. **Fetch strategy for source issuetype:** Should the implementation
   call `get_issue(key, &["issuetype"])` (add `fields` param override to
   limit payload) or reuse the full `get_issue` response if one is already
   in scope? Currently `handle_edit` does NOT fetch the issue before PUT —
   a new GET is required on the 400 error path only.

2. **Fetch strategy for target type subtask flag:** The cheapest path is
   `GET /rest/api/3/project/{projectKey}/issuetypes` or the project meta
   cache. The target type name is available from the `--type` argument;
   the subtask flag must be fetched from project metadata. F4 architect
   should confirm which API call is used and whether the project meta cache
   (7-day TTL, keyed by project key) covers this case.

3. **`--no-parent` hint strengthening:** Should T-06 be updated in the
   same PR as the `edit --type` change, or in a separate preceding PR?
   Bundling is cleaner (single source of truth for the shared hint text)
   but adds test churn. Recommend bundling since both changes are in the
   same `handle_edit` function.

4. **Hint wording finalization:** Research addendum A3 flags that "More →"
   is stale UI wording. The recommended locale-resilient wording is: "To
   convert, open the issue in the Jira web UI and use the action menu
   (ellipsis `...`) to find 'Convert to issue' or 'Convert to sub-task'."
   F2 should pin the exact hint wording in BC-3.4.010 before F4 implements.
