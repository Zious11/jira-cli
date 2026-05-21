---
document_type: f1-delta-analysis
phase: phase-f1-delta-analysis
producer: architect
issue: 388
status: draft
created: 2026-05-20
project: jira-cli
mode: BROWNFIELD
intent: enhancement
bundled_fix: true
feature_type: backend
trivial_scope: false
scope: standard
regression_risk: medium
severity: N/A
inputs:
  - ".factory/phase-f1-delta-analysis/issue-388/impact-boundary.md"
  - ".factory/phase-f1-delta-analysis/affected-artifacts.md"
  - ".factory/research/issue-388-subtask-conversion.md"
---

# F1 Delta Analysis — Issue #388

## Feature

- **Name:** Accurate cross-hierarchy type-change error + fix fake-endpoint hint (Option A)
- **Issue link:** https://github.com/Zious11/jira-cli/issues/388
- **Research source:** `.factory/research/issue-388-subtask-conversion.md` (all six
  research questions answered at HIGH confidence; Option A validated by addendum
  sections A1–A5)
- **Closest precedent:** Issue #384 (JSM 401 auth-aware error hints) — same file, same
  "improve error surfacing" pattern, classified `enhancement`

## Problem Summary

`jr issue edit KEY --type X` that crosses the standard ↔ sub-task hierarchy boundary
returns HTTP 400 with no actionable guidance. The Jira Cloud REST API does not support
cross-hierarchy type conversion (JRACLOUD-27893, open since 2012, unresolved). The
current error gives the user nothing useful — they cannot distinguish a cross-hierarchy
rejection from a typo.

A bundled bug is present at `src/cli/issue/create.rs:834`: the `--no-parent` subtask
error hint tells users to run `jr api /rest/api/3/issue/{key}/convert -X put -d ...`,
referencing a non-existent Jira Cloud endpoint (confirmed REFUTED by research Q2). This
hint was introduced by issue #284; the prior research file `issue-284-no-parent-flag.md`
Claim 2 contained a misattribution in the same family as the JRACLOUD misattributions
documented in CLAUDE.md under issue #361. The hint must be corrected regardless of
which implementation option is chosen for #388.

## Validated Design (Option A — reshaped)

Scope is **single-key `jr issue edit KEY --type X`** error path only. The multi-key
bulk path and all happy paths are byte-for-byte unchanged.

**On `edit_issue` HTTP 400 only:**

1. Fetch the source issue via `get_issue` (already in `api/jira/issues.rs:426`) to read
   `fields.issuetype.subtask: Option<bool>` and `fields.project.key`.
2. Resolve the `--type` target name against project issue-type metadata via
   `get_project_issue_types` (already in `api/jira/projects.rs:37`) to read the target
   type's `subtask: Option<bool>`.
3. Compare the two booleans:
   - `src != tgt` → **CrossHierarchy** → emit `CROSS_HIERARCHY_HINT` (JRACLOUD-27893,
     web-UI ellipsis-menu Convert action, no fake endpoint, no exact UI label)
   - `src == tgt` → **SameCategory** → emit typo hint ("run `jr project types`")
   - Either fetch fails → **Indeterminate** → surface raw Atlassian error, no hint
4. The subtask-flag comparison is the **primary signal** (locale-independent). The
   English substring `"issue type selected is invalid"` is NOT used as a primary
   classifier (research addendum A1: same message fires on plain typos; A2: locale-
   fragile).
5. Fix `create.rs:834`: replace the fake-endpoint hint in the `--no-parent` subtask
   path with the shared `CROSS_HIERARCHY_HINT` constant (same corrected wording).

---

## Intent and Classification

| Dimension | Value |
|-----------|-------|
| Intent | `enhancement` (with bundled bug fix — the fake-endpoint correction at `create.rs:834`) |
| Feature type | `backend` |
| Trivial scope | `false` |
| Scope | `standard` |
| Regression risk | `MEDIUM` |

**Intent rationale:** The primary deliverable is improved error surfacing for an existing
command path. The fake-endpoint bug fix is bundled because both changes share the same
`CROSS_HIERARCHY_HINT` constant and both live inside `handle_edit`. Matches the #384
precedent ("JSM 401 hint surface refinement" → `enhancement`).

**Scope rationale:** Multi-path logic change (3 output branches), 1 struct field
addition, 2 new BCs + 1 BC cross-reference update, a new test file (5 functions), an
existing regression-pinned test that must be strengthened, and two new async calls on
the error path. Does not meet the trivial threshold (single module, no new BCs, LOW
regression risk — fails all three gates).

**Regression risk rationale:** T-06 in `tests/issue_edit_no_parent.rs` asserts
`stderr.contains("convert")` and will NOT mechanically break after the fix (the new hint
still references the Convert action), but an implementer who does not actively strengthen
the assertion leaves the literal-pin incomplete. The medium classification is a
deliberate forcing function to ensure T-06 is upgraded to a full literal-pin and the
`is_cross_hierarchy_type_error` classifier is unit-tested.

---

## Impact Assessment

| Artifact Class | Impact | Detail |
|---------------|--------|--------|
| PRD / BCs | 2 new + 1 annotation-only update | BC-3.4.010 (NEW — CrossHierarchy path), BC-3.4.011 (NEW — SameCategory / Indeterminate path), BC-3.4.003 (MODIFIED — Errors cross-ref annotation only, no behavioral change). BC counts: bc-3 `total_bcs` 95 → 97; `definitional_count` 66 → 68; BC-INDEX `total_bcs` 575 → 577. |
| Architecture | No structural change | No new module, no new public `JiraClient` method, no new trait. Additive: 1 struct field (`IssueType.subtask`), 1 const (`CROSS_HIERARCHY_HINT`), 1 pure helper function (`is_cross_hierarchy_type_error`). Both required API calls (`get_issue`, `get_project_issue_types`) already exist. |
| UX / Screens | None | Pure error-message improvement; no new user flows, no new flags. |
| Stories | 1 new story | Single story: implement `is_cross_hierarchy_type_error` classifier, add two error-path fetches to `handle_edit`, fix `create.rs:834` hint, add `IssueType.subtask` field, update T-06 literal-pin, add `tests/issue_edit_type_errors.rs` (5 functions), add inline proptest. |
| Existing tests | 1 test strengthened (T-06) | `test_subtask_parent_clear_surfaces_400_with_convert_hint` in `tests/issue_edit_no_parent.rs` — current `contains("convert")` check survives mechanically but must be upgraded to a literal-pin on `JRACLOUD-27893` and a negative-pin on `/rest/api/3/issue/`. |
| New tests | New file — 5 integration tests | `tests/issue_edit_type_errors.rs` covering all 3 output branches plus 2 regression-literal pins. |
| Verification Properties | 1 proptest candidate | `is_cross_hierarchy_type_error(src_subtask: bool, tgt_subtask: bool, err: &str) -> Classification` is a pure function. An inline proptest property — `for any (src: bool, tgt: bool, msg: String), if src != tgt then result is CrossHierarchy` — should be co-located with the function in `src/cli/issue/create.rs mod tests`. No VP-NNN artifacts required (no VP directory in use; BC-level anchoring is the project standard). |

---

## Affected BC Mapping

### New BCs Required

**Numbering:** The last allocated BC in Section 3.4 is BC-3.4.009 (issue #340, F2
2026-05-15). The next available sequential IDs are BC-3.4.010 and BC-3.4.011, following
the convention established by issues #384 (BC-3.8.014–015) and #385 (BC-3.8.016–017).

| Proposed BC ID | File | Description |
|----------------|------|-------------|
| BC-3.4.010 | `bc-3-issue-write.md` | `jr issue edit KEY --type X` returns HTTP 400 AND source `issuetype.subtask` differs from target type's `subtask` (cross-hierarchy mismatch) → exit 1, `CROSS_HIERARCHY_HINT` on stderr. Hint content: explains the Jira Cloud REST API does not support cross-hierarchy type conversion, cites JRACLOUD-27893, and directs the user to the Jira web UI ellipsis-menu `...` action ("Convert to issue" / "Convert to sub-task"). Hint wording avoids quoting an exact UI label (locale and version instability per research addendum A3). The subtask-flag mismatch is the PRIMARY gate; the English "issue type selected is invalid" substring is locale-fragile and MUST NOT be the sole classifier (addendum A1). The same `CROSS_HIERARCHY_HINT` constant is emitted for the `--no-parent` subtask-bound 400 path (bug fix at `create.rs:834` — replacing the fake `PUT /rest/api/3/issue/{key}/convert` endpoint reference). |
| BC-3.4.011 | `bc-3-issue-write.md` | `jr issue edit KEY --type X` returns HTTP 400 AND source and target `subtask` flags MATCH (same hierarchy level, not a cross-hierarchy error) OR flag resolution is indeterminate (fetch of source issue or project types fails) → exit 1. SameCategory path: emit typo hint referencing `jr project types`, surface raw Atlassian error body. Indeterminate path: surface raw Atlassian error body without any enrichment hint. `CROSS_HIERARCHY_HINT` (JRACLOUD-27893 citation) MUST NOT appear on either sub-path, preventing false positives on plain type-name typos or workflow-incompatibility 400s (research addendum A4). |

### BCs Modified (Annotation Only — No Behavioral Change)

| BC ID | File | Nature of Change |
|-------|------|-----------------|
| BC-3.4.003 | `bc-3-issue-write.md` | Errors section: add cross-reference to BC-3.4.010 and BC-3.4.011 for the 400 `--type` paths. Primary success path (PUT 204) and ADF description behavior are byte-for-byte unchanged. |

### BCs in Regression Risk Zone (Confirmed Unchanged)

| BC ID | File | Reason Unchanged |
|-------|------|-----------------|
| BC-3.4.004 | `bc-3-issue-write.md` | ADF on wire — success path; not touched |
| BC-3.4.005 | `bc-3-issue-write.md` | Multiple fields in single body — success path; not touched |
| BC-3.4.006 | `bc-3-issue-write.md` | Label coalesce — different branch from `--type` single-key path |
| BC-3.4.007 | `bc-3-issue-write.md` | `--description` / `--description-stdin` clap conflict — unchanged |
| BC-3.4.008 | `bc-3-issue-write.md` | `--points` / `--no-points` clap conflict — unchanged |
| BC-3.4.009 | `bc-3-issue-write.md` | Bulk-poll timeout task_id pin — bulk path entirely unrelated |
| BC-X.3.004 | `cross-cutting.md` | Generic 400 field-error formatting — base BC unchanged; new behavior is subsequent enrichment on the CrossHierarchy sub-path only |
| BC-X.3.007 | `cross-cutting.md` | Error messages must suggest next step — new hints satisfy this invariant; BC text unchanged |

---

## Reconciliation: Architect vs. Business Analyst

The two inputs agree on all material decisions. Three points of explicit reconciliation:

**1. `src/api/jira/projects.rs` and `src/api/jira/issues.rs` — no new method needed.**
The business-analyst input lists both files as "possibly modified (TBD in F4)" with a
question about whether thin helper wrappers should be added. The architect input
confirms both methods already exist with sufficient signatures: `get_issue` includes
`issuetype` in `BASE_ISSUE_FIELDS`, and `get_project_issue_types` already returns
`Vec<IssueTypeMetadata>` with `subtask: Option<bool>`. Neither file requires
modification. The F4 implementer should reuse these methods directly.

**2. `IssueType` struct must gain `subtask: Option<bool>` — one struct change required.**
The business-analyst input does not explicitly mention the struct change. The architect
input identifies it as the only necessary change outside `create.rs`: `src/types/jira/
issue.rs` `IssueType` currently carries only `name: String`; the `subtask` flag must be
added with `#[serde(default)]` for backward-compatible deserialization. This is the
mechanism that makes `get_issue` return the source issue's subtask flag without any
additional HTTP call or method change.

**3. Regression risk level — MEDIUM confirmed.**
Both inputs converge on MEDIUM. The architect analysis identifies the primary risk
driver as the T-06 literal-pin gap (not a mechanical break but an intentional
strengthening obligation). The business-analyst analysis identifies the same risk
driver plus the false-positive classification risk. Both are mitigated by: (a) the
subtask-flag mismatch being the primary gate, not the English substring, and (b) the
Indeterminate path degrading gracefully to raw error rather than emitting the
conversion hint.

---

## Files Changed

### Modified Source Files

| File | Change Type | Risk |
|------|------------|------|
| `src/cli/issue/create.rs` | MODIFIED — add `CROSS_HIERARCHY_HINT` const; add `is_cross_hierarchy_type_error` pure helper near `is_subtask_parent_error` (~line 1159); add CrossHierarchy / SameCategory / Indeterminate dispatch block inside `if let Err(ref e) = edit_result { ... }` in `handle_edit`; replace fake-endpoint hint at ~line 834 (`--no-parent` path) with `CROSS_HIERARCHY_HINT` const reference; add inline proptest in `mod tests` for `is_cross_hierarchy_type_error` | MEDIUM — error-path-only; happy path bytes unchanged |
| `src/types/jira/issue.rs` | MODIFIED — add `subtask: Option<bool>` with `#[serde(default)]` to `IssueType` struct | LOW — additive field; backward-compatible deserialization; existing struct-literal constructions in test fixtures need `subtask: None` (verify `tests/common/fixtures.rs`) |

### New Files

| File | Purpose |
|------|---------|
| `tests/issue_edit_type_errors.rs` | 5 integration tests covering the new `--type` 400 enrichment paths |

The five required test functions:
1. `test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint` — GET issue (standard, `subtask: false`), GET project types (target is subtask, `subtask: true`), PUT returns 400 → exit 1 + `JRACLOUD-27893` in stderr + `/rest/api/3/issue/` absent from stderr
2. `test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint` — reverse direction
3. `test_edit_type_same_hierarchy_400_surfaces_typo_hint` — both flags `subtask: false` → exit 1 + `jr project types` in stderr, `JRACLOUD-27893` ABSENT from stderr
4. `test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error` — GET issue succeeds, GET project types returns 5xx → exit 1 + raw Atlassian error body, no hint
5. `test_edit_type_cross_hierarchy_hint_no_fake_endpoint_literal` — regression pin: asserts `/rest/api/3/issue/` is absent from stderr on a CrossHierarchy 400 path (mirrors the `JRACLOUD-95368` literal-pin pattern documented in CLAUDE.md)

### Modified Test Files

| File | Change Type |
|------|------------|
| `tests/issue_edit_no_parent.rs` | MODIFIED — strengthen T-06 (`test_subtask_parent_clear_surfaces_400_with_convert_hint`): retain existing `contains("convert")` checks; add `assert!(stderr.contains("JRACLOUD-27893"))` literal-pin; add `assert!(!stderr.contains("/rest/api/3/issue/"))` negative regression guard |

### Spec Files

| File | Change Type |
|------|------------|
| `.factory/specs/prd/bc-3-issue-write.md` | MODIFIED — append BC-3.4.010 and BC-3.4.011 bodies under Section 3.4; add cross-reference annotation to BC-3.4.003 Errors section; update `total_bcs` 95 → 97; update `definitional_count` 66 → 68 |
| `.factory/specs/prd/BC-INDEX.md` | MODIFIED — register BC-3.4.010 and BC-3.4.011; update `total_bcs` 575 → 577 and running-total annotation |
| `.factory/specs/prd/CANONICAL-COUNTS.md` | MODIFIED — bump bc-3 count and grand total by 2 |

---

## Files NOT Changed (Regression Baseline)

These files must not be modified during implementation. All their tests must continue
to pass unchanged.

| File | Why Unchanged |
|------|--------------|
| `src/api/jira/issues.rs` | `get_issue` and `edit_issue` signatures unchanged; `issuetype` already in `BASE_ISSUE_FIELDS`; no behavior change on any call path |
| `src/api/jira/projects.rs` | `get_project_issue_types` already exists with correct return type; no signature change |
| `src/api/client.rs` | No new predicate method needed (unlike #384); no HTTP layer change |
| `src/error.rs` | `JrError::ApiError { status, message }` is the existing downcast target; no variant change |
| `src/cli/mod.rs` | No new CLI flags; no clap definition change |
| `src/cli/issue/create.rs` lines 267–736 | Field-building, bulk routing, JQL resolution — all outside the error block; byte-for-byte unchanged |
| `src/cli/issue/create.rs` `handle_create` / `handle_jsm_create` | Different command path; zero code overlap with `handle_edit` |
| `src/cli/issue/workflow.rs` | `move` / `transition` / `assign` / `open`; unrelated |
| `src/cli/issue/list.rs` | Read path; unrelated |
| `src/cli/issue/links.rs` | Link/unlink; unrelated |
| `src/api/jsm/servicedesks.rs` | JSM service-desk layer; not called by `handle_edit` |
| `src/api/jsm/queues.rs` | JSM queue path; unrelated |
| `src/cache.rs` | No cache involved in this feature |
| `src/config.rs` | No config change |
| `src/adf.rs`, `src/jql.rs`, `src/duration.rs` | Pure utilities; untouched |
| `tests/issue_create_jsm.rs` | JSM create path; `is_cross_hierarchy_type_error` logic does not exist on the JSM dispatch |
| `tests/issue_bulk_pr2.rs` | Bulk edit path — does NOT include CrossHierarchy enrichment; `--type` on the bulk path uses `handle_edit_bulk_fields`, not the single-key error block |
| `tests/issue_bulk.rs` | Bulk path; unrelated |
| `tests/bulk_deadline_propagation.rs` | Bulk timeout path; unrelated |
| `tests/issue_commands.rs` | BC-3.3.x, BC-3.4.x success paths; unrelated |
| `tests/issue_create_json.rs` | Platform create JSON shape; unrelated |
| `tests/issue_write_holdouts.rs` | Holdout suite; `handle_edit` success paths unchanged |
| `tests/issue_edit_no_parent.rs` T-01 through T-05, T-07, T-08 | All `--no-parent` success-path, clap-conflict, and PUT-body tests; no interaction with hint text |
| `CLAUDE.md` | No new test-seam env var introduced; no new env-var-seam gotcha needed. The corrected `--no-parent` hint and the `--type` 400 detector are implementation details, not agent-facing seams. (If F4 determines a gotcha entry is warranted for the `is_cross_hierarchy_type_error` classification logic, the implementer should add one at that stage.) |

---

## Risk Assessment

| Risk Type | Level | Rationale |
|-----------|-------|-----------|
| Regression — T-06 literal-pin gap | MEDIUM | T-06 in `tests/issue_edit_no_parent.rs` asserts `stderr.contains("convert")` which will still pass after the fix (the corrected hint still references the Convert action). The risk is that an implementer treats the mechanical green as sufficient. The test MUST be actively strengthened with the `JRACLOUD-27893` literal-pin and the `/rest/api/3/issue/` negative-pin; these are not automatic. |
| Regression — false-positive CrossHierarchy hint | MEDIUM | Research addendum A1: the English error substring `"issue type selected is invalid"` fires on both boundary-cross errors AND plain typos. If the implementation relies on the substring as the primary gate (rather than the subtask-flag comparison), the CrossHierarchy hint fires on typos, misleading the user. The Indeterminate path (fetch failure) must degrade to raw error only, not to the CrossHierarchy hint. Mitigated by: mandatory subtask-flag comparison as the primary gate; test function 3 (same-hierarchy typo path) and test function 4 (indeterminate) pinning the negative constraint. |
| Additional HTTP calls on edit-error path | LOW | Two sequential GET calls on the 400 error path only (`get_issue` → `get_project_issue_types`). Zero impact on the success path. Rate-limit and latency risk is negligible on an error path. |
| i18n / locale fragility | LOW | Subtask-flag comparison is locale-independent. The English substring is relegated to secondary/best-effort role. Mitigated by the architectural constraint in the BC. |
| `IssueType` struct deserialization | LOW | `subtask: Option<bool>` with `#[serde(default)]` is backward-compatible. All existing API responses deserializing `IssueType` receive `subtask: None` if the field is absent. All existing struct-literal constructions in test fixtures must add `subtask: None` — verify `tests/common/fixtures.rs`. |
| Bulk path | NONE | `handle_edit_bulk_fields` at `create.rs:736` returns before the single-key error block. The CrossHierarchy enrichment does not exist on the bulk path and must not be added to it in this cycle. |
| Architecture | NONE | No new module, no new public interface, no new external dependency. Both required API calls already exist. |
| Security | NONE | Error-hint quality improvement only. No auth path, no credential handling, no scope change. |

---

## Recommended Scope for F2–F7

### F2 — Spec Evolution

- Append BC-3.4.010 and BC-3.4.011 bodies to `bc-3-issue-write.md` under Section 3.4.
- Add Errors cross-reference annotation to BC-3.4.003.
- Update `total_bcs`, `definitional_count`, BC-INDEX, and CANONICAL-COUNTS atomically.
- Run `scripts/check-bc-cumulative-counts.sh` after all spec edits (DRIFT-002).
- Run `scripts/check-spec-counts.sh` after all spec edits (DRIFT-001).
- Pin the verbatim `CROSS_HIERARCHY_HINT` wording in BC-3.4.010 (locale-resilient
  ellipsis-menu pointer, JRACLOUD-27893 citation, no exact UI label — per research
  addendum A3 and recommendation from affected-artifacts.md §Open Questions 4).
- Pin the verbatim typo-hint wording for BC-3.4.011 (references `jr project types`).
- **No new stories, no implementation work in F2.**

### F3 — Story Decomposition

Single story, no parallelism. The `is_cross_hierarchy_type_error` classifier is the
substrate for all fix sites; both sites must ship atomically. Suggested story scope:

> Implement `is_cross_hierarchy_type_error` classifier (pure function, proptest-covered)
> + add CrossHierarchy/SameCategory/Indeterminate dispatch block in `handle_edit`'s
> 400 error branch + fix `create.rs:834` `--no-parent` hint + add `IssueType.subtask`
> field + strengthen T-06 in `tests/issue_edit_no_parent.rs` + add
> `tests/issue_edit_type_errors.rs` (5 tests).

### F4 — TDD Implementation

**Implementation decisions (open questions from affected-artifacts.md resolved):**

1. **Source issue fetch:** Call `get_issue(key, &[])` on the 400 error path (extra_fields
   empty — `issuetype` is already in `BASE_ISSUE_FIELDS`). Do not add a thin helper.

2. **Target type fetch:** Call `get_project_issue_types(project_key)` directly (no cache
   intermediary in this cycle — the error path is infrequent enough that an extra HTTP
   call is acceptable; caching is a follow-up if profiling shows latency pressure).

3. **`--no-parent` hint update:** Bundle in the same PR as the `edit --type` change.
   Both changes reference `CROSS_HIERARCHY_HINT`; atomic delivery prevents the const
   from being added without the bug fix, or vice versa.

4. **`IssueType.subtask` struct change:** Verify `tests/common/fixtures.rs` for any
   struct-literal `IssueType { name: "..." }` constructions and add `subtask: None`.

5. **Test file location:** New test functions go in a new file
   `tests/issue_edit_type_errors.rs` (not appended to `tests/issue_commands.rs`) to
   keep per-feature grouping consistent with the existing `tests/issue_edit_no_parent.rs`
   naming pattern.

### F5 — Adversarial Review

Focus areas:
- Verify the CrossHierarchy hint does NOT fire on the test-3 same-hierarchy path (the
  most likely adversarial finding on this feature class given addendum A1).
- Verify the Indeterminate path (project-types fetch 5xx) degrades to raw error only.
- Verify the `JRACLOUD-27893` literal appears in the CrossHierarchy hint and is absent
  from the SameCategory hint.
- Verify the bulk `--type` path (`handle_edit_bulk_fields`) is completely unaffected.
- Verify no duplicate emit: `CROSS_HIERARCHY_HINT` must appear exactly once in the
  `--no-parent` error block and exactly once in the `--type` error block — not in both
  when both flags happen to be set (which is not a valid combination, but confirm clap
  enforces this or the logic is otherwise safe).

### F6 / F7 — Formal Hardening / Convergence

No formal verification properties are required. The inline proptest on
`is_cross_hierarchy_type_error` (added in F4) is the verification-architecture artifact
for this feature. No VP-NNN document is needed.

---

## Open Questions (Deferred to F2 / F4)

1. **Hint wording finalization (F2):** Research addendum A3 confirms that "More →" is
   stale Jira Cloud UI wording and exact labels vary by locale/version. The F2
   product-owner should pin the exact verbatim hint in BC-3.4.010. Recommended
   locale-resilient wording (from affected-artifacts.md §Open Questions 4):
   *"To convert, open the issue in the Jira web UI and use the action menu (`...`) to
   find 'Convert to issue' or 'Convert to sub-task'."*

2. **`scripts/check-bc-no-numeric-test-counts.sh` enforcement (F2):** New BC bodies
   for BC-3.4.010 and BC-3.4.011 must use qualitative Source fields (file path +
   test category), not numeric test counts. Enforced by CI.

3. **`is_subtask_parent_error` relationship (F4):** The new `is_cross_hierarchy_type_error`
   helper is a sibling to `is_subtask_parent_error` at `create.rs:1159`. The existing
   function is NOT replaced or modified — it remains the gate for the `--no-parent`
   parent-clear 400 (a subtask refusing to lose its parent). The two functions
   address distinct errors; the implementer must not merge them.
