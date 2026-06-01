---
document_type: story
story_id: "S-331"
title: "Fix issueType bulk-edit wire schema: camelCase key, issueTypeId value, project-scoped resolution"
wave: feature-followup
status: draft
priority: high
estimated_effort: small-medium
tdd_mode: strict
regression_risk: MEDIUM
bc_anchors:
  - BC-3.4.018
  - BC-3.4.019
holdout_anchors: []
nfr_anchors: []
adr_refs: []
sd_refs: []
files_modified:
  - src/cli/issue/create.rs (handle_edit_bulk_fields: fix key casing, value shape, add resolver + cross-project guard; dry-run builder comment update)
  - src/api/jira/issues.rs (new fn get_issue_types_for_project)
  - src/types/jira/bulk.rs (SCHEMA NOTES comment update — remove unverified caveats)
  - CLAUDE.md (add gotcha entry for --type multi-key bulk path)
test_files:
  - tests/issue_bulk_pr2.rs (rewrite test_multi_key_type_update_uses_consistent_issuetype_casing → test_multi_key_type_update_body_uses_issue_type_id; add test_bulk_issuetype_body_uses_issuetype_id_not_name; test_bulk_issuetype_cross_project_keys_exits_64; test_bulk_issuetype_unknown_type_name_exits_non_zero)
  - tests/e2e_live.rs (new gated test behind JR_RUN_E2E; clean-skip if JR_E2E_ISSUE_TYPE_ALT unset)
breaking_change: false
producer: story-writer
version: "1.0"
last_updated: 2026-06-01
depends_on: []
blocks: []
issue: 331
---

# S-331: Fix `issueType` Bulk-Edit Wire Schema

## Context

`jr issue edit KEY1 KEY2 --type <NAME>` (multi-key path) routes through
`handle_edit_bulk_fields` in `src/cli/issue/create.rs` and POSTs to
`POST /rest/api/3/bulk/issues/fields`. The current implementation builds the
payload with two bugs:

1. **Wrong key casing:** `editedFieldsInput["issuetype"]` (lowercase) — Atlassian's
   API requires camelCase `"issueType"` for this field's container key. The
   `selectedActions` element correctly stays lowercase `"issuetype"` (these
   intentionally differ per the verified Atlassian Bulk Operations FAQ).
2. **Wrong value shape:** `{"name": "<type name>"}` — the bulk endpoint is ID-only
   and requires `{"issueTypeId": "<id-string>"}`. There is no `{"name": ...}` form for
   `issueType` on this endpoint.

These bugs almost certainly produce HTTP 400 or a silent no-op on real Jira Cloud.
This fix is the third in the `#331` schema-correction series, following the priority
fix (PR #452) and the labels fix (PR #448/#446).

A complication specific to `issueType` (not present in the priority fix): issue-type
IDs are **project-scoped**. The bulk endpoint takes one `issueTypeId` for the entire
batch, so a multi-key set spanning multiple projects cannot be safely resolved to a
single id. This story implements an error-early cross-project guard (exit 64 before
any HTTP call) as the v1 policy; per-project grouping is explicitly deferred.

One existing test in `tests/issue_bulk_pr2.rs` —
`test_multi_key_type_update_uses_consistent_issuetype_casing` — actively asserts that
the camelCase `"issueType"` key is ABSENT from the body. That assertion inverts after
the fix. This test MUST be rewritten and renamed as part of this story; it MUST fail
on the pre-fix code (Red Gate verification) and pass after the fix.

## Behavioral Contracts

| BC | Title |
|----|-------|
| BC-3.4.018 | Multi-key `--type` bulk wire shape: `editedFieldsInput["issueType"] = {"issueTypeId": "<id>"}` with `selectedActions: ["issuetype"]`; name resolved via `GET /rest/api/3/issue/createmeta/{proj}/issuetypes` |
| BC-3.4.019 | Cross-project guard: keys spanning >1 distinct project exit 64 before any API call |

Source: `.factory/specs/prd/bc-3-issue-write.md` §BC-3.4.018 and §BC-3.4.019 (added
2026-06-01, issue #331 F2).

## Acceptance Criteria

**AC-001** (traces to BC-3.4.018, postcondition 2 — wire shape)
`jr issue edit FOO-1 FOO-2 --type Bug --no-input` submits a bulk POST whose body:
- Contains `"issueType"` (camelCase) as the `editedFieldsInput` key.
- Contains `"issueTypeId"` with a string id value (e.g. `"10001"`) resolved from the
  project's createmeta issuetypes response.
- Contains `"issuetype"` (lowercase) in the `selectedActions` array.
- Does NOT contain a `"name"` key in the issueType value position (the old name-based
  shape is eliminated).
The id is resolved via `GET /rest/api/3/issue/createmeta/FOO/issuetypes` before the
bulk POST. The two strings (`"issuetype"` in selectedActions vs `"issueType"` in
editedFieldsInput) intentionally differ — this asymmetry MUST NOT be "fixed."
Verified by `test_bulk_issuetype_body_uses_issuetype_id_not_name`.

**AC-002** (traces to BC-3.4.018, invariant 2 — unknown type name)
`jr issue edit FOO-1 FOO-2 --type Nonexistent --no-input` where the createmeta
issuetypes response lists only `[{id: "10001", name: "Bug"}]`:
- Exits with code 64.
- Writes to stderr a message containing `"Nonexistent"` and listing valid type names
  for the project (format: `"Issue type 'Nonexistent' not found for project FOO.
  Valid types: Bug."`).
- NO `POST /rest/api/3/bulk/issues/fields` is issued.
Verified by `test_bulk_issuetype_unknown_type_name_exits_non_zero`.

**AC-003** (traces to BC-3.4.019, postcondition — cross-project guard)
`jr issue edit FOO-1 BAR-2 --type Bug --no-input` (keys span projects FOO and BAR):
- Exits with code 64.
- Writes to stderr a message that contains `"--type"`, references the cross-project
  constraint (e.g. `"requires all issues to be in the same project"`), and names both
  distinct projects `FOO` and `BAR`.
- NO `GET /rest/api/3/issue/createmeta/...` call is issued.
- NO `POST /rest/api/3/bulk/issues/fields` call is issued.
- The guard fires for ONLY the `--type` flag; a cross-project `--summary`-only bulk
  edit is unaffected (BC-3.4.019 invariant 2).
Verified by `test_bulk_issuetype_cross_project_keys_exits_64`.

**AC-004** (traces to BC-3.4.018, invariant 3 — single-key regression)
`jr issue edit FOO-1 --type Bug` (single key) behavior is BYTE-FOR-BYTE unchanged:
- Routes to the existing `handle_edit` → `PUT /rest/api/3/issue/FOO-1` path.
- `GET /rest/api/3/issue/createmeta/.../issuetypes` is NOT called.
- Existing integration tests for BC-3.4.003, BC-3.4.010, and BC-3.4.011 pass without
  modification.
Verified implicitly by the full test suite passing after the fix (no changes to
single-key test code).

**AC-005** (traces to BC-3.4.018, invariant 1 — case-insensitive resolution)
`jr issue edit FOO-1 FOO-2 --type bug --no-input` (all-lowercase name) resolves
case-insensitively to `{id: "10001", name: "Bug"}` and proceeds to the bulk POST
with `"issueTypeId": "10001"`. The input casing does not affect resolution.
Verified by the dedicated test `test_bulk_issuetype_resolves_type_name_case_insensitively`
in `tests/issue_bulk_pr2.rs`, which passes `--type bug` (lowercase) against a createmeta
mock returning `{name:"Bug"}` and asserts exit 0 with `issueTypeId` `"10001"`.

**AC-006** (traces to BC-3.4.018, invariant 4 — project key extraction)
Project key extraction splits each issue key on the LAST hyphen and takes all
characters before it as the project key. Examples:
- `FOO-1` → project `FOO`
- `PROJ2-100` → project `PROJ2`
- `FOO-1` and `FOO-2` → both project `FOO`; cross-project guard does NOT fire.
Verified by a unit test covering the extraction helper for keys including `PROJ2-100`.

**AC-007** (traces to BC-3.4.018, postcondition 4 + invariant 5 — dry-run builder consistency)
`jr issue edit FOO-1 FOO-2 --type Bug --dry-run --output json` dry-run output:
- `plannedChanges` contains key `"issueType"` (camelCase — matching the live POST key
  per invariant 5; the pre-fix dry-run used lowercase `"issuetype"` which was a
  secondary spec violation).
- The value in `plannedChanges.issueType` is a bare string (the type name `"Bug"`) —
  intentionally simplified, NOT `{"issueTypeId": "..."}`.
- The surrounding comment in the dry-run builder MUST NOT contain `"best-guess"` or
  `"unverified"` qualifiers for issueType after the fix ships.
- No `GET /rest/api/3/issue/createmeta/.../issuetypes` call is issued during dry-run.

**AC-008** (traces to BC-3.4.018, postcondition — test rewrite Red Gate)
The existing test `test_multi_key_type_update_uses_consistent_issuetype_casing` is
rewritten and renamed to `test_multi_key_type_update_body_uses_issue_type_id`. The
rewritten test asserts:
- `body_str.contains("\"issueType\"")` — camelCase key IS present in `editedFieldsInput`.
- `body_str.contains("\"issueTypeId\"")` — id-based value IS present.
- `body_str.contains("\"issuetype\"")` — lowercase string IS present in `selectedActions`.
- `body_str` does NOT contain `"\"name\":"` in the issueType value position.
The implementer MUST verify that the OLD test (before rewrite) FAILS on pre-fix code,
confirming the old test was pinning the wrong shape (Red Gate check).

**AC-009** (traces to BC-3.4.018, postcondition — SCHEMA NOTES documentation)
`src/types/jira/bulk.rs` SCHEMA NOTES comment block no longer contains `"best-guess"`,
`"unverified"`, or `"pending #331"` qualifiers for the issueType field. The comment
documents the verified canonical shape: camelCase `"issueType"` key, `"issueTypeId"`
string value, and the intentional casing asymmetry with `selectedActions`.

**AC-010** (traces to BC-3.4.018 + BC-3.4.019 — CLAUDE.md documentation)
`CLAUDE.md` contains a gotcha entry for the `--type` multi-key bulk path that covers:
- The camelCase/lowercase asymmetry: `selectedActions` element is `"issuetype"` (lowercase);
  `editedFieldsInput` key is `"issueType"` (camelCase) — they intentionally differ.
- The cross-project guard: `--type` on multi-key bulk requires all keys in the same project;
  cross-project sets exit 64 before any API call.
- The name→issueTypeId resolution endpoint: `GET /rest/api/3/issue/createmeta/{proj}/issuetypes`,
  project-scoped (not global like priority), no cache.
Per the `JR_*` doc-fallout rule in CLAUDE.md AI Agent Notes, the entry for
`JR_E2E_ISSUE_TYPE_ALT` (see AC-011) must be added in the same commit.

**AC-011** (traces to BC-3.4.018 — gated live E2E test)
A gated integration test is added to `tests/e2e_live.rs` behind `JR_RUN_E2E=1` and
`#[ignore]`. The test:
- Has an early-return guard (`if !e2e_enabled() { return; }`) as the first statement,
  per the E2E gate pattern established in S-E2E-3 (every gated test must guard against
  `--include-ignored` without the env var).
- Reads `JR_E2E_ISSUE_TYPE_ALT` env var. If unset, the test clean-skips (returns
  immediately after the `e2e_enabled()` guard with a log line like
  `"skipping: JR_E2E_ISSUE_TYPE_ALT not set"`).
- If `JR_E2E_ISSUE_TYPE_ALT` is set: creates two issues in `JR_E2E_PROJECT` using
  `jr issue create`, bulk-changes their type via
  `jr issue edit KEY1 KEY2 --type <value_of_JR_E2E_ISSUE_TYPE_ALT> --no-input`,
  verifies both issues show the new type via `jr issue view --output json`, then
  cleans up (closes/deletes the test issues).
- `JR_E2E_ISSUE_TYPE_ALT` is a debug-only seam (`#[cfg(debug_assertions)]` read-site
  is optional for this env var since it is read only in test code, but must be
  documented in CLAUDE.md per the `JR_*` doc-fallout rule).
`tests/e2e_cli_surface_guard.rs` SURFACE table must be updated if any new `jr` CLI
invocations are introduced in the E2E test that are not already registered there.

**AC-012** (traces to BC-3.4.018 + BC-3.4.019 — release gate)
`cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` all pass with
zero new failures or warnings after the changes.

## Tasks (TDD order — strict Red → Green)

### Phase 1: Red Gate (failing tests before any production changes)

**Task 1 — Verify existing inverted test (pre-work check)**
Before touching any source file, run `cargo test --test issue_bulk_pr2
test_multi_key_type_update_uses_consistent_issuetype_casing` and confirm it PASSES
on the current (pre-fix) code. This establishes it was pinning the wrong shape and
will need to be inverted. Document the observation as a comment.

**Task 2 — Rewrite the inverted test (Red Gate for AC-008)**
Rewrite `test_multi_key_type_update_uses_consistent_issuetype_casing` in
`tests/issue_bulk_pr2.rs` to `test_multi_key_type_update_body_uses_issue_type_id`.
The rewritten test should:
- Mount a wiremock stub for `GET /rest/api/3/issue/createmeta/FOO/issuetypes`
  returning `{"values": [{"id": "10001", "name": "Bug"}]}`.
- Assert `body_str.contains("\"issueType\"")` (camelCase key present).
- Assert `body_str.contains("\"issueTypeId\"")` (id-based value present).
- Assert `body_str.contains("\"issuetype\"")` (lowercase in selectedActions present).
- Assert the body does NOT contain `"\"name\":"` in the issueType value position.
Run `cargo test --test issue_bulk_pr2 test_multi_key_type_update_body_uses_issue_type_id`
and confirm it FAILS on the pre-fix code. This is the Red Gate.

**Task 3 — Write new test: body shape pin (Red Gate for AC-001)**
In `tests/issue_bulk_pr2.rs`, add
`test_bulk_issuetype_body_uses_issuetype_id_not_name` mirroring
`test_bulk_priority_body_uses_priority_id_not_name`. Mount:
- `GET /rest/api/3/issue/createmeta/FOO/issuetypes` returning
  `{"values": [{"id": "10001", "name": "Bug"}]}`.
- Bulk POST mock requiring `body_string_contains("issueTypeId")`.
Run `jr issue edit FOO-1 FOO-2 --type Bug --no-input` against wiremock.
Assert: body contains `"issueTypeId"`, does NOT contain `"\"name\":"` in the
issueType context. Confirm this FAILS on pre-fix code.

**Task 4 — Write new test: cross-project guard exits 64 (Red Gate for AC-003)**
In `tests/issue_bulk_pr2.rs`, add
`test_bulk_issuetype_cross_project_keys_exits_64`.
Run `jr issue edit FOO-1 BAR-2 --type Bug --no-input` with NO mocks mounted.
Assert: exit code 64, stderr contains `"--type"` and project keys `FOO` and `BAR`,
no HTTP calls to wiremock's createmeta or bulk POST endpoints.
Confirm this FAILS on pre-fix code (pre-fix either succeeds or errors for other
reasons — it does NOT exit 64 with the correct message).

**Task 5 — Write new test: unknown type name exits 64 (Red Gate for AC-002)**
In `tests/issue_bulk_pr2.rs`, add
`test_bulk_issuetype_unknown_type_name_exits_non_zero`.
Mount `GET /rest/api/3/issue/createmeta/FOO/issuetypes` returning
`{"values": [{"id": "10001", "name": "Bug"}]}`.
Run `jr issue edit FOO-1 FOO-2 --type Nonexistent --no-input`.
Assert: exit code 64, stderr contains `"Nonexistent"` and `"Bug"`, no bulk POST.
Confirm this FAILS on pre-fix code.

**Task 6 — Write project key extraction unit test (Red Gate for AC-006)**
Add a unit test (inline `#[cfg(test)]` in `src/cli/issue/create.rs` or in
`tests/issue_bulk_pr2.rs`) covering the project-key extraction helper for
`PROJ2-100` → `PROJ2`, `FOO-1` → `FOO`, `FOO-2` → `FOO` (same project).
Confirm the helper function does not yet exist (so the test cannot even compile
without the Green step).

### Phase 2: Green (production changes to make failing tests pass)

**Task 7 — Pre-implementation read (prerequisite check)**
Read `src/api/jira/projects.rs` and locate any existing `IssueTypeMetadata` or
equivalent `{id: String, name: String}` struct used by the BC-3.4.010/011 enrichment
path. If it exists and is accessible (`pub(crate)` or `pub`), plan to reuse it. If
not accessible, plan to either widen visibility or define a parallel struct in
`src/api/jira/issues.rs` with the same shape. Document the decision in a code comment
(`// Reuse IssueTypeMetadata from projects.rs` or `// Separate struct: projects.rs
struct is not reachable from issues.rs without restructuring`).

**Task 8 — Add `get_issue_types_for_project` to `src/api/jira/issues.rs`**
Implement `pub(crate) async fn get_issue_types_for_project(client, project_key) ->
Result<Vec<IssueTypeEntry>>` (where `IssueTypeEntry` is the reused or new
`{id: String, name: String}` struct). Issues a `GET /rest/api/3/issue/createmeta/
{projectKey}/issuetypes` call, deserializes the `values` array. No caching — one HTTP
call per invocation, matching the priority resolver model. Rustdoc on the function
MUST state: no cache, one-shot call per invocation, project-scoped, call site in
`handle_edit_bulk_fields` only.

**Task 9 — Implement project key extraction helper + cross-project guard in `handle_edit_bulk_fields`**
Add a private helper `fn project_key_from_issue_key(key: &str) -> &str` that splits
on the last hyphen and returns everything before it. Add the cross-project guard:
before any HTTP call, compute the set of distinct project keys across `effective_keys`;
if count > 1, return a `JrError::UserError` with the required message (contains
`"--type"`, cross-project constraint language, and the distinct project key list).

**Task 10 — Fix `editedFieldsInput` key and value in `handle_edit_bulk_fields`**
Replace:
```rust
edited.insert("issuetype".into(), json!({"name": t}));
```
with:
```rust
let resolved_id = /* call get_issue_types_for_project, case-insensitive match */;
edited.insert("issueType".into(), json!({"issueTypeId": resolved_id}));
```
Keep `selected_actions.push("issuetype".to_string());` (lowercase — already correct).
Add the unknown-name error path: if no match found, return `JrError::UserError` with
the required message format from BC-3.4.018 invariant 2.

**Task 11 — Fix dry-run builder key casing + update comment**
In `src/cli/issue/create.rs` dry-run builder block (~line 663-673), update the
`editedFieldsInput` key from `"issuetype"` to `"issueType"` (camelCase, per BC-3.4.018
invariant 5: dry-run and live POST MUST use the same key). The VALUE in the dry-run
preview remains a bare string (the type name) — this is intentionally simplified, same
model as priority. Remove any `"best-guess"`, `"unverified"`, or `"pending #331"`
qualifier comments near this code.

**Task 12 — Update `src/types/jira/bulk.rs` SCHEMA NOTES**
In the SCHEMA NOTES comment block (~lines 243-252), remove `"best-guess"` /
`"unverified"` / `"pending #331"` qualifiers for the issueType field. Document the
confirmed shape: camelCase `"issueType"` container key, `"issueTypeId"` string value,
and the intentional asymmetry with the `selectedActions` lowercase `"issuetype"` string.
Update `handle_edit_bulk_fields` rustdoc to describe the now-fixed shape and the
project-scoped id semantics.

### Phase 3: Documentation + Release Gate

**Task 13 — Add gated E2E test (AC-011)**
In `tests/e2e_live.rs`, add the `JR_RUN_E2E`-gated + `#[ignore]` test per AC-011.
Add `JR_E2E_ISSUE_TYPE_ALT` to the E2E env-var list in CLAUDE.md (AI Agent Notes
section, parallel to `JR_E2E_ISSUE_TYPE`). Update `tests/e2e_cli_surface_guard.rs`
SURFACE table if new `jr` invocations are introduced.

**Task 14 — Update CLAUDE.md gotcha entry (AC-010)**
Add the three-part `--type` multi-key bulk gotcha entry to CLAUDE.md (Gotchas section),
covering: the camelCase/lowercase asymmetry, the cross-project exit-64 guard, and the
createmeta resolution endpoint. Include the `JR_E2E_ISSUE_TYPE_ALT` debug-only env var
documentation in the AI Agent Notes section.

**Task 15 — Release gate (AC-012)**
Run `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check`. All must pass
with zero new failures or warnings.

## Test Plan

### Wiremock Integration Tests (all in `tests/issue_bulk_pr2.rs`)

| Test name | What is pinned | Expected result |
|-----------|---------------|-----------------|
| `test_multi_key_type_update_body_uses_issue_type_id` (REWRITE) | Body has camelCase `"issueType"` key AND `"issueTypeId"` value; no `"name"` in value | Pass on fixed code, FAIL on pre-fix code |
| `test_bulk_issuetype_body_uses_issuetype_id_not_name` (NEW) | Full body-shape pin: `"issueType"` camelCase, `"issueTypeId"` string, `"issuetype"` in selectedActions, no `"name"` | Pass on fixed code, FAIL on pre-fix code |
| `test_bulk_issuetype_cross_project_keys_exits_64` (NEW) | Cross-project FOO-1 + BAR-2 with `--type` → exit 64, required stderr substrings, zero HTTP calls | Pass on fixed code, FAIL on pre-fix code |
| `test_bulk_issuetype_unknown_type_name_exits_non_zero` (NEW) | Unknown `--type Nonexistent` → exit 64, lists valid types, no bulk POST | Pass on fixed code, FAIL on pre-fix code |

Each wiremock test mounts `GET /rest/api/3/issue/createmeta/FOO/issuetypes` returning
`{"values": [{"id": "10001", "name": "Bug"}]}` (except the cross-project test which
must mount NO HTTP mocks and assert zero requests are received).

### Unit Test

| Test | Location | What is verified |
|------|----------|-----------------|
| `test_project_key_extraction_*` | Inline `#[cfg(test)]` in `src/cli/issue/create.rs` | `project_key_from_issue_key` returns `"FOO"` for `"FOO-1"`, `"PROJ2"` for `"PROJ2-100"`, etc. |

### Regression Verification

The following existing tests MUST pass unchanged (no modifications to test code):
- BC-3.4.003 suite (single-key `--type` PUT path)
- BC-3.4.010/011 suite (single-key `edit --type` 400 error enrichment)
- `test_multi_key_type_update_uses_consistent_issuetype_casing` predecessor — this test
  is DELETED and replaced by `test_multi_key_type_update_body_uses_issue_type_id`; no
  other tests reference the old name.

### Gated Live E2E Test (in `tests/e2e_live.rs`)

Gated behind `JR_RUN_E2E=1` + `#[ignore]` + early-return guard per project convention:
- Create two issues in `JR_E2E_PROJECT` with `JR_E2E_ISSUE_TYPE` (default type).
- Run `jr issue edit KEY1 KEY2 --type <JR_E2E_ISSUE_TYPE_ALT> --no-input`.
- Verify both issues have the new type via `jr issue view KEY --output json`.
- Clean up (close/delete test issues).
- Clean-skip if `JR_E2E_ISSUE_TYPE_ALT` is not set.
This validates the project-scoped id resolution against real Jira Cloud, the aspect
with no existing codebase precedent (priority was global; this is project-scoped).

## Out of Scope

- **Per-project grouping (v2):** Sending one bulk POST per distinct project group,
  each with the project-correct `issueTypeId`, is explicitly NOT in this story. BC-3.4.019
  invariant 3 documents this deferral. Any code that implements per-project grouping
  would require updating BC-3.4.019 first.
- **Cache for createmeta issuetypes lookups:** No cache is added in this story. One HTTP
  call per `--type` bulk invocation matches the priority resolver model. Cache can be
  added as a follow-up if repeated-call latency is observed in practice.
- **`--type + --summary` multi-key combination test:** The fix does not change the
  multi-field composition logic; the `--type + --summary` combined bulk case is not
  explicitly tested in this story (the composition invariant is unchanged).
- **Single-key `--type` path (handle_edit → PUT):** byte-for-byte unchanged. BC-3.4.003,
  BC-3.4.010, BC-3.4.011 govern this path and are unmodified.

## Risk / Notes

### Risk 1: Single-key path contamination (HIGH probability if guard is missing)
The fix modifies only `handle_edit_bulk_fields`, called exclusively for 2+ keys.
The single-key `--type` path goes through `handle_edit` → `edit_issue`. These are
independent. The fix MUST NOT touch `handle_edit` or any single-key logic.
Mitigation: confirm the call-site gate (`effective_keys.len() >= 2`) is unchanged;
rely on existing BC-3.4.003/010/011 tests for regression coverage.

### Risk 2: Dry-run builder key fix (secondary spec violation)
The dry-run builder currently also uses lowercase `"issuetype"` for the
`editedFieldsInput` key. BC-3.4.018 invariant 5 requires the key to be camelCase in
both dry-run and live POST. Only the KEY changes (the VALUE stays as a bare string).
Do NOT change the value shape in the dry-run — the "intentionally simplified" model
is preserved.

### Risk 3: IssueTypeMetadata struct duplication
Check `src/api/jira/projects.rs` for an existing `{id: String, name: String}` struct
before defining a new one. Duplicate structs with identical layouts produce lint
warnings and can diverge. See Task 7.

### Risk 4: Inverted test Red Gate
`test_multi_key_type_update_uses_consistent_issuetype_casing` currently PASSES on
wrong code. After rewrite, it must FAIL on pre-fix code and PASS on fixed code.
The implementer must verify this explicitly (Task 1 + Task 2).

### Risk 5: Cross-project guard scope leakage
The guard fires ONLY when `--type` is present (BC-3.4.019 invariant 4). `--summary`,
`--priority`, and other flags on multi-key sets are not affected. Do not make the
guard unconditional.

## Token Budget Estimate

| Component | Estimated tokens |
|-----------|-----------------|
| Story spec (this file) | ~5,500 |
| `bc-3-issue-write.md` §BC-3.4.018 + §BC-3.4.019 (read for exact AC contract language) | ~1,200 |
| `src/cli/issue/create.rs` (read handle_edit_bulk_fields + dry-run builder) | ~2,000 |
| `src/api/jira/issues.rs` (read existing structure; add get_issue_types_for_project) | ~1,000 |
| `src/api/jira/projects.rs` (read IssueTypeMetadata check) | ~600 |
| `src/types/jira/bulk.rs` (read SCHEMA NOTES; update comment) | ~400 |
| `tests/issue_bulk_pr2.rs` (read existing tests; rewrite + add 3 new tests) | ~3,000 |
| `tests/e2e_live.rs` (read existing gate pattern; add new gated test) | ~1,500 |
| `tests/e2e_cli_surface_guard.rs` (read SURFACE table; update if needed) | ~500 |
| `CLAUDE.md` (read Gotchas + AI Agent Notes; add entries) | ~800 |
| `cargo test` + `cargo clippy` output verification | ~400 |
| **Total** | **~17,000** |

Within the 20-30% agent context window budget for a small-medium story. The bulk of
context is the existing test file (~970 LOC) and `create.rs` (large file). The implementer
should read only the specific function bodies they modify, not the entire files.

## Previous Story Intelligence

**S-452 (priority bulk fix, PR #452, merged 2026-05-28)** is the direct predecessor.
The `editedFieldsInput["priority"] = {"priorityId": "<id-string>"}` pattern (camelCase
container key, id-based value, direct object) is the model this fix follows exactly.
The priority fix passed live Jira on the first try, providing high confidence the
same pattern is correct for issueType (confirmed by the verbatim Atlassian Bulk
Operations FAQ source). Key difference: priority IDs are global (one `GET
/rest/api/3/priority` call, no project scope); issueType IDs are project-scoped
(one `GET /rest/api/3/issue/createmeta/{proj}/issuetypes` call, with cross-project
guard). Reuse the priority resolver's error-message format and `UserError` exit-64
pattern.

**S-448/S-446 (labels bulk fix)** established the `labelsFields` ARRAY container
pattern — this is the OPPOSITE of what issueType uses. issueType is a DIRECT OBJECT
like priority, NOT an array like labels. Do not confuse the two patterns.

## Architecture Compliance Rules

1. **Single-key path untouched:** `handle_edit` and `edit_issue` (PUT single-key path)
   MUST NOT be modified. The fork to `handle_edit_bulk_fields` is gated on
   `effective_keys.len() >= 2`; this gate MUST remain unchanged.

2. **No caching for createmeta issuetypes:** The resolver calls
   `GET /rest/api/3/issue/createmeta/{proj}/issuetypes` once per invocation without
   caching. This matches the priority precedent. Adding a cache is out of scope.

3. **No new `JrError` variant:** Use `JrError::UserError` (exit 64) for both the
   unknown-type-name error and the cross-project guard error, matching the priority
   error pattern. Do not add a new variant.

4. **Zero lint suppression:** No `#[allow(...)]` annotations. Refactor if clippy
   warns (e.g., for function argument count on `handle_edit_bulk_fields`). Per project
   zero-suppression policy.

5. **`IssueTypeMetadata` reuse:** Check for an existing `{id: String, name: String}`
   struct in `src/api/jira/projects.rs` before defining a new one. Do not create a
   duplicate struct with an identical layout.

6. **`selectedActions` stays lowercase:** The `selected_actions.push("issuetype"
   .to_string())` line is CORRECT and must NOT be changed to `"issueType"`. The
   asymmetry between `selectedActions` and `editedFieldsInput` key casing is
   intentional and confirmed by verbatim Atlassian docs.

7. **E2E gate pattern:** The new E2E test MUST have `#[ignore]`, a `JR_RUN_E2E`
   early-return guard as the FIRST statement, and a clean-skip for missing
   `JR_E2E_ISSUE_TYPE_ALT`. Per `tests/e2e_live.rs` convention established by S-E2E-3.

## Library & Framework Requirements

No new Cargo dependencies. All required library capabilities are already in scope:

| Dependency | Already present | Usage in this story |
|------------|-----------------|---------------------|
| `reqwest` / `JiraClient` | Yes | `get_issue_types_for_project` HTTP call |
| `serde_json` / `json!` macro | Yes | Payload construction |
| `wiremock` | Yes (dev) | New integration test mocks |
| `serde` / `Deserialize` | Yes | `IssueTypeEntry` / `IssueTypeMetadata` struct |
| `tokio` | Yes | Async fn |

Do NOT add any new `[dependencies]` or `[dev-dependencies]` entries to `Cargo.toml`.

## File Structure Requirements

| File | Action | Notes |
|------|--------|-------|
| `src/cli/issue/create.rs` | MODIFY | `handle_edit_bulk_fields`: fix key (`issuetype` → `issueType`), fix value (`{"name":t}` → `{"issueTypeId": resolved_id}`), add `project_key_from_issue_key` helper, add cross-project guard, add resolver call. Dry-run builder: fix key casing; remove unverified comment qualifiers. |
| `src/api/jira/issues.rs` | MODIFY | Add `pub(crate) async fn get_issue_types_for_project(client, project_key) -> Result<Vec<IssueTypeEntry>>`. |
| `src/types/jira/bulk.rs` | MODIFY | SCHEMA NOTES block: remove `"best-guess"` / `"unverified"` qualifiers for issueType; document confirmed shape. |
| `tests/issue_bulk_pr2.rs` | MODIFY | Rewrite + rename `test_multi_key_type_update_uses_consistent_issuetype_casing`; add three new tests. |
| `tests/e2e_live.rs` | MODIFY | Add gated E2E test (behind `JR_RUN_E2E` + `#[ignore]` + early-return guard + `JR_E2E_ISSUE_TYPE_ALT` clean-skip). |
| `tests/e2e_cli_surface_guard.rs` | MODIFY IF NEEDED | Update SURFACE table if new `jr` CLI paths are introduced in the E2E test. |
| `CLAUDE.md` | MODIFY | Add `--type` multi-key bulk gotcha to Gotchas section; add `JR_E2E_ISSUE_TYPE_ALT` to AI Agent Notes E2E env-var list. |
| `src/api/jira/projects.rs` | READ ONLY (Task 7 check) | Do not modify; check for reusable `IssueTypeMetadata` struct only. |
| `src/cli/issue/handle_edit` | DO NOT TOUCH | Single-key path is out of scope. |
| `Cargo.toml` | DO NOT TOUCH | No new dependencies. |

## References

- Issue #331: `fix(bulk): issueType bulk-edit wire schema`
- F1 delta analysis: `.factory/phase-f1-delta-analysis/issue-331/delta-analysis.md`
- Wire schema research: `.factory/research/issue-331-issuetype-bulk-schema.md`
- BC definitions: `.factory/specs/prd/bc-3-issue-write.md` §BC-3.4.018 + §BC-3.4.019
- Priority fix precedent: PR #452 (S-452)
- Labels fix precedent: PR #448 / #446
