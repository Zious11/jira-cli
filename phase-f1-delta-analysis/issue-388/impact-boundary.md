# Impact Boundary Report — Issue #388
# Accurate cross-hierarchy type-change error + fix fake-endpoint hint (Option A)

**Issue**: #388
**Date**: 2026-05-20
**Analyst**: architect (Phase F1 Step 3)
**Feature type**: backend
**Intent**: bug-fix + enhancement (error-message accuracy)
**Trivial scope**: NO (requires two new async API calls on the error path; new detection
                       logic; shared hint constant; regression-pinned literal string)

---

## 1. Validated Design (re-statement for traceability)

Scope is single-key `jr issue edit KEY --type X` and the `--no-parent` subtask path only.
No multi-key bulk path. No actual cross-hierarchy conversion feature.

### On `edit_issue` HTTP 400 failure (error path only — happy path untouched):

1. Fetch the source issue (`GET /rest/api/3/issue/{key}`) to read
   `fields.issuetype.subtask` (bool) and `fields.project.key` (String).
2. Call `get_project_issue_types(project_key)` — already exists in
   `src/api/jira/projects.rs` — to resolve the requested `--type` target name
   against the project's `IssueTypeMetadata` list and read its `subtask: Option<bool>`.
3. Compare source and target `subtask` booleans:
   - `source.subtask != target.subtask` → CrossHierarchy → emit cross-hierarchy hint
   - `source.subtask == target.subtask` → SameCategory → emit typo hint ("run `jr project types`")
   - Fetch/resolve fails (API error, type name not found) → Indeterminate → raw error, no hint
4. The subtask-flag mismatch is the primary signal. The locale-fragile English error
   substring `"issue type selected is invalid"` (A1 addendum) is NOT used as a
   classifier — the detection gate is structural (downcast to `JrError::ApiError { status: 400 }`)
   combined with source-vs-target `subtask` flag comparison.

### Fix the `--no-parent` hint bug (`src/cli/issue/create.rs` ~line 834):

Replace the bogus `jr api /rest/api/3/issue/{key}/convert -X put -d ...` hint (which
references a non-existent endpoint per research Q2) with the shared cross-hierarchy hint
constant. No new logic required — same hint, different insertion point.

---

## 2. Component Classification

| Component | Classification | Rationale |
|-----------|---------------|-----------|
| `src/cli/issue/create.rs` — `handle_edit` error block (~lines 827-839) | MODIFIED | Replace `--no-parent` hint string with shared constant; add `--type` 400 detector block |
| `src/cli/issue/create.rs` — `is_subtask_parent_error` (~line 1159) | DEPENDENT | Read-only reference; not structurally changed |
| `src/types/jira/issue.rs` — `IssueType` struct | MODIFIED | Add `subtask: Option<bool>` field; currently only carries `name: String` |
| `src/api/jira/projects.rs` — `get_project_issue_types` | DEPENDENT | Already exists; accepts `&str`, returns `Vec<IssueTypeMetadata>` with `subtask: Option<bool>`; no signature change needed |
| `src/api/jira/issues.rs` — `get_issue` | DEPENDENT | Already exists; fetches `issuetype` field via `BASE_ISSUE_FIELDS`; once `IssueType.subtask` is added, it will be deserialized automatically |
| `src/error.rs` | DEPENDENT | `JrError::ApiError { status, message }` is the downcast target; no variant change needed |
| `tests/issue_edit_no_parent.rs` | MODIFIED | Fix T-06 assertion: `convert hint` check must no longer match the fake endpoint string; pin that the bogus `/rest/api/3/issue/{key}/convert` literal is absent from stderr |
| `tests/issue_commands.rs` (or new file) | NEW | Wiremock integration tests for `--type` cross-hierarchy 400 (std→subtask, subtask→std directions); SameCategory typo-hint path; Indeterminate path |
| `.factory/specs/prd/bc-3-issue-write.md` | MODIFIED | Add new BC(s) for cross-hierarchy hint on `--type` 400 and for corrected `--no-parent` hint |
| `.factory/specs/prd/BC-INDEX.md` | MODIFIED | Register new BC(s) |
| `.factory/specs/prd/CANONICAL-COUNTS.md` | MODIFIED | Increment BC count |
| `CLAUDE.md` "Gotchas" section | MODIFIED | Add note about the corrected `--no-parent` hint and the `--type` 400 detector; note that `is_subtask_parent_error` is NOT replaced, only supplemented |

---

## 3. API Method Availability Audit

### `get_issue` — EXISTS, signature sufficient

`src/api/jira/issues.rs:426`:
```rust
pub async fn get_issue(&self, key: &str, extra_fields: &[&str]) -> Result<Issue>
```
`issuetype` is already in `BASE_ISSUE_FIELDS` (line 16). Once `IssueType` gains the
`subtask: Option<bool>` field (see section 4 below), this call returns the flag
without any API method change.

### `get_project_issue_types` — EXISTS, signature sufficient

`src/api/jira/projects.rs:37`:
```rust
pub async fn get_project_issue_types(&self, project_key: &str) -> Result<Vec<IssueTypeMetadata>>
```
`IssueTypeMetadata` already has `subtask: Option<bool>` (line 12 of projects.rs).
The method calls `GET /rest/api/3/project/{key}` and plucks `issueTypes` from the
response. No new API method or struct needed for this side.

**Conclusion: NO new API method needs to be created.** Both required HTTP calls are
already implemented. The only new code is the detection logic in `handle_edit` and the
struct field addition in `IssueType`.

---

## 4. Required Struct Change — `IssueType` in `src/types/jira/issue.rs`

Current (line 158-161):
```rust
pub struct IssueType {
    pub name: String,
}
```

Required:
```rust
pub struct IssueType {
    pub name: String,
    #[serde(default)]
    pub subtask: Option<bool>,
}
```

The `#[serde(default)]` attribute ensures backward compatibility — existing API
responses that omit the `subtask` field (e.g., old cached values, truncated fields
queries) deserialize to `None` rather than erroring. The `Option<bool>` type aligns
with `IssueTypeMetadata.subtask` in projects.rs (same field semantics).

**Impact of this struct change:** `IssueType` is used in `IssueFields` (issue.rs:62),
which is used wherever issues are fetched. The `subtask` field is additive and
`#[serde(default)]`-gated, so all existing deserialization paths remain valid. All
existing tests that construct `IssueType { name: "...".into() }` must use a struct-
update syntax or add `subtask: None` — verify existing test fixtures in `tests/common/`.

---

## 5. Architecture Verdict

**Structural change required: NO new module, NO new trait, NO new interface.**

The change is entirely internal to the single-key error path of `handle_edit`. It adds
two sequential async calls (`get_issue`, `get_project_issue_types`) on the **error path
only** — the happy path is byte-for-byte unchanged. The purity boundary, module
decomposition, and dependency graph are unaffected. `handle_edit` is already in the
effectful shell (I/O-bound); these additional HTTP calls remain in the same layer.

The new detection function (`is_cross_hierarchy_type_error` or equivalent) is a
pure function (takes `&anyhow::Error`, source `Option<bool>`, target `Option<bool>`,
returns a classification enum) — eligible for unit testing without wiremock.

The shared cross-hierarchy hint string should be a `const &str` in `src/cli/issue/create.rs`
(or in `src/error.rs` following the `API_TOKEN_EXPIRY_HINT` precedent) so both the
`--type` error block and the `--no-parent` hint block reference the same literal. This
enables the regression-literal pinning test to check one constant rather than two
independent strings.

---

## 6. Regression Risk Assessment

| Module | Risk | Rationale |
|--------|------|-----------|
| `handle_edit` happy path (single-key `--type` succeeds) | LOW | Detection code is inside `if let Err(ref e) = edit_result { ... }`. Error-path-only — the success branch at line 840 (`edit_result?`) is never reached through the new code. No behavioral change on 204 response. |
| `handle_edit` `--no-parent` happy path | LOW | Only the hint string inside the error block changes. The `is_subtask_parent_error` detection function itself is unchanged. |
| `handle_edit` multi-key bulk path | NONE | Routed via `handle_edit_bulk_fields` at line 736, which returns before reaching the single-key error block. Zero code overlap. |
| `IssueType` struct deserialization | LOW | `subtask: Option<bool>` with `#[serde(default)]` is additive. All existing deserializations succeed; only struct-literal constructions in test code must add `subtask: None`. |
| `get_issue` behavior | NONE | No signature change; no new HTTP call on the happy path. The additional field is silently deserialized. |
| `get_project_issue_types` behavior | NONE | No change whatsoever; already used by `cli/project.rs`. |
| `tests/issue_edit_no_parent.rs` T-06 | LOW | The hint assertion will change from `contains("convert")` to a more precise check, and must pin that the fake endpoint string is absent. Green test becomes more specific — not a regression risk, an intentional tightening. |
| JSM create path | NONE | `handle_jsm_create` is a separate code path entered only when `--request-type` is set. No overlap with `handle_edit`. |

**Overall regression risk: LOW.** The happy path is untouched. All additional logic
executes only inside an error branch.

---

## 7. Locale-Fragility Mitigation (Research Addendum A1/A2)

The research addendum confirms that `"The issue type selected is invalid."` is NOT a
boundary-cross-specific message — it fires for any invalid type name (typo, wrong
project, wrong id). The detection gate must NOT rely on this substring alone.

Implementation constraint: the `is_cross_hierarchy_type_error` classifier MUST:
1. First gate on `JrError::ApiError { status: 400, .. }` (structural, locale-independent).
2. Then attempt to fetch source issue and resolve target type.
3. Use the `subtask` boolean comparison as the definitive signal.
4. Treat fetch/resolve failure as Indeterminate — emit raw error without a hint.
5. NOT attempt English-substring matching on the API body as a primary classifier.

This is locale-safe and avoids false positives on workflow-incompatibility 400s
(research addendum A4), which produce a different server message and will not produce
a `subtask`-flag mismatch.

---

## 8. Files Likely Changed

| File | Change Type | Nature of Change |
|------|------------|-----------------|
| `src/cli/issue/create.rs` | MODIFIED | (a) Add `CROSS_HIERARCHY_HINT` const. (b) Replace fake-endpoint hint in `--no-parent` error block (~line 834) with const ref. (c) Add `--type` 400 detector block after `edit_result?` (or inline in `if let Err`). (d) Add `is_cross_hierarchy_type_error` helper function near `is_subtask_parent_error`. |
| `src/types/jira/issue.rs` | MODIFIED | Add `subtask: Option<bool>` with `#[serde(default)]` to `IssueType` struct. |
| `tests/issue_edit_no_parent.rs` | MODIFIED | Tighten T-06 hint assertions: pin absence of `/rest/api/3/issue/{key}/convert` literal; update `contains("convert")` checks to match new shared hint wording (JRACLOUD-27893, web UI action). |
| `tests/issue_commands.rs` or new `tests/issue_edit_type_change.rs` | NEW | Integration tests: CrossHierarchy std→subtask (200 GET + 200 project types + 400 edit → exit 1 + hint on stderr); CrossHierarchy subtask→std (mirror); SameCategory (400 edit + flag mismatch = false → typo hint); Indeterminate (GET fails → raw error); regression: literal `JRACLOUD-27893` in stderr on CrossHierarchy path. |
| `.factory/specs/prd/bc-3-issue-write.md` | MODIFIED | Add BC-3.4.0NN for cross-hierarchy hint on `edit --type` 400; amend or replace existing `--no-parent` hint BC if one exists. Update `total_bcs` and `definitional_count`. |
| `.factory/specs/prd/BC-INDEX.md` | MODIFIED | Register new BC(s). |
| `.factory/specs/prd/CANONICAL-COUNTS.md` | MODIFIED | Bump BC count. |
| `CLAUDE.md` | MODIFIED | Gotchas: note corrected `--no-parent` hint (cite #388); note `--type` 400 cross-hierarchy detection (structural gate, not substring); note `is_subtask_parent_error` is NOT replaced. |

---

## 9. Files Explicitly NOT Changed (Regression Baseline)

| File | Why unchanged |
|------|--------------|
| `src/api/jira/issues.rs` | `get_issue` and `edit_issue` signatures unchanged; no behavior change |
| `src/api/jira/projects.rs` | `get_project_issue_types` unchanged; already works |
| `src/api/client.rs` | HTTP layer unchanged |
| `src/cli/issue/create.rs` lines 267–736 (field-building, bulk routing, JQL resolution) | All unaffected by error-path-only change |
| `src/cli/issue/create.rs` `handle_create` / `handle_jsm_create` | Different command; no overlap |
| `src/cli/mod.rs` | No new CLI flags; no clap definition changes |
| `src/error.rs` | `JrError` enum unchanged; `ApiError { status: 400 }` is the existing downcast target |
| `src/cli/issue/workflow.rs` | `move` command; unrelated |
| `src/cli/issue/list.rs` | Read-only command; unrelated |
| `tests/issue_create_jsm.rs` | JSM create path; no overlap with `edit --type` error path |
| `tests/issue_bulk_pr2.rs` | Multi-key bulk path; no overlap |
| `tests/issue_bulk.rs` | Same |
| `tests/bulk_deadline_propagation.rs` | Bulk timeout path; no overlap |
| All other test files | No shared code paths touched |
| `src/cache.rs` | No cache involved in this feature |
| `src/config.rs` | No config change |
| `src/adf.rs`, `src/jql.rs`, `src/duration.rs` | Pure utilities; untouched |
| `.factory/research/issue-284-no-parent-flag.md` | Historical document; not edited (PR description should note the Claim-2 misattribution per research recommendation) |

---

## 10. Downstream BC Traceability

Existing BCs that intersect with this change:

| BC | Intersection | Action |
|----|-------------|--------|
| BC-3.4.003 (`issue edit` PUTs `/rest/api/3/issue/{key}`) | Happy path; untouched | None |
| BC-3.4.005 (`issue edit` with multiple fields sends both in body) | Happy path; untouched | None |
| Existing `--no-parent` subtask 400 BC (if any in bc-3-issue-write.md) | Hint text changes | Update or supersede the BC with corrected hint wording |

New BCs required (IDs assigned by product-owner in F2):
- `BC-3.4.0NN`: `edit --type X` returns 400 and source-issue `issuetype.subtask` differs from target-type `subtask` → exit 1 + cross-hierarchy hint on stderr containing JRACLOUD-27893
- `BC-3.4.0NN+1`: `edit --type X` returns 400 and source/target `subtask` flags match → exit 1 + typo hint ("run `jr project types`")
- `BC-3.4.0NN+2`: `edit --no-parent` returns 400 → exit 1 + cross-hierarchy hint (same wording; no longer contains `/rest/api/3/issue/{key}/convert`)

---

## 11. Summary

**What exists and works:**
- `get_issue` (issues.rs): returns `IssueType` already in `BASE_ISSUE_FIELDS`
- `get_project_issue_types` (projects.rs): returns `IssueTypeMetadata` with `subtask: Option<bool>`
- `JrError::ApiError { status: 400 }` downcast pattern: used elsewhere in the codebase
- `is_subtask_parent_error`: existing pure predicate (not replaced)

**What is new:**
- `IssueType.subtask: Option<bool>` field (1 struct field addition)
- `CROSS_HIERARCHY_HINT` const (shared hint string, eliminates fake endpoint)
- `is_cross_hierarchy_type_error` classifier (pure function, unit-testable)
- Two async calls on the `edit_issue` 400 error path only
- 3-path hint dispatch: CrossHierarchy / SameCategory / Indeterminate

**What is NOT new:**
- No new API module
- No new CLI flag
- No new clap definition
- No change to the happy path
- No change to the multi-key bulk path
- No new public `JiraClient` method
