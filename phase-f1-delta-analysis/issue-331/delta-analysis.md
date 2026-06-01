---
document_type: delta-analysis-report
feature_name: "Fix issueType bulk-edit wire schema (camelCase key + issueTypeId value + project-scoped name resolution)"
issue: 331
created: 2026-06-01
spec_version_at_analysis: "post-S-452-labels"
status: draft
intent: "bug-fix"
feature_type: "backend"
severity: "MEDIUM"
trivial_scope: false
predecessor_cycles: "PR #452 (priority fix), PR #448/#446 (labels fix)"
research_source: ".factory/research/issue-331-issuetype-bulk-schema.md"
---

# F1 Delta Analysis: Issue #331 — `issueType` Bulk-Edit Wire Schema Fix

## Feature Request

- **Brief:** Issue #331 (remaining scope) — fix the `editedFieldsInput["issuetype"]` bulk shape:
  wrong key casing (`issuetype` → `issueType`), wrong value (`{"name":"..."}` → `{"issueTypeId":"<id-string>"}`),
  and missing name→id resolution (which is project-scoped, unlike priority's global lookup).
- **Requested by:** #331 tracking issue; wire schema verified 2026-06-01 in `.factory/research/issue-331-issuetype-bulk-schema.md`.
- **Date:** 2026-06-01

---

## Classifications

### Intent Classification

**Classified intent:** `bug-fix`

**Rationale:** The current implementation builds a payload shape that almost certainly produces
HTTP 400 or silent no-op on real Jira Cloud. Two bugs are present:
1. `editedFieldsInput` key uses lowercase `"issuetype"` — should be camelCase `"issueType"`.
2. Value uses `{"name": t}` (name-based) — should be `{"issueTypeId": "<id-string>"}` (id-based).

No new user-facing capability is added. The existing `--type` flag on multi-key edit is already
present; this fix makes it actually work against Atlassian's API. This is the same classification
that applied to the priority fix (PR #452) and the labels fix (PR #446/#448).

### Feature Type Classification

**Classified type:** `backend`

**Rationale:** Wire format fix and name→id resolution inside `handle_edit_bulk_fields`. No new CLI
flags, no UI, no protocol change. Adds one new API call (`GET /rest/api/3/issue/createmeta/{proj}/issuetypes`)
analogous to the priority resolver's `GET /rest/api/3/priority`.

### Trivial Scope Classification

**Classified scope:** `standard` (not trivial)

**Rationale:** Although the code change is moderate (~50–80 LOC including resolver), the fix has
a genuinely novel constraint — project-scoped issue-type IDs — that has no precedent in the
codebase. The cross-project guard (detect and error early when keys span >1 project) is new
logic requiring careful implementation and test coverage. The trivial-scope bar requires no new
BCs; this fix requires at least one new BC and possibly two.

---

## Wire Schema Verification Summary

From `.factory/research/issue-331-issuetype-bulk-schema.md` (authoritative — verbatim Atlassian
Bulk Operations FAQ page, fetched twice for consistency):

| Aspect | Current jr code | VERIFIED canonical | Status |
|--------|-----------------|-------------------|--------|
| `editedFieldsInput` key | `"issuetype"` (lowercase) | `"issueType"` (camelCase) | REFUTED |
| value object | `{"name": "<type name>"}` | `{"issueTypeId": "<id-string>"}` | REFUTED |
| `selectedActions` value | `"issuetype"` (lowercase) | `"issuetype"` (lowercase) | CONFIRMED |
| container form | direct object | direct object (like priority, NOT like labels) | CONFIRMED |

**The `selectedActions` string stays lowercase `"issuetype"`. The `editedFieldsInput` key becomes
camelCase `"issueType"`. These intentionally differ.** This asymmetry is identical to the
priority pattern (action string `"priority"`, container key `"priority"` — both lowercase, same
there; for issueType the action string lowercase `"issuetype"` diverges from the camelCase
container key `"issueType"`).

The bulk endpoint is ID-only. No `{"name": "Bug"}` form exists for `issueType` on this endpoint.

### Existing Audit Test Is Now WRONG

`tests/issue_bulk_pr2.rs::test_multi_key_type_update_uses_consistent_issuetype_casing` (line ~1395)
was written to pin the **currently incorrect** casing. It asserts:
- `body_str.contains("issuetype")` — checks for the OLD lowercase key.
- `!body_str.contains("\"issueType\"")` — actively asserts camelCase is ABSENT.

After the fix, the correct body will have `"issueType"` (camelCase) in `editedFieldsInput` and
`"issuetype"` (lowercase) in `selectedActions`. The test's second assertion
(`!body_str.contains("\"issueType\"")`) will **FAIL** after the fix — this test must be
rewritten as part of this story.

---

## Impact Boundary

### Production Files

| File | Function | Change Type | Description |
|------|----------|-------------|-------------|
| `src/cli/issue/create.rs` | `handle_edit_bulk_fields` | MODIFY | Fix key casing (`issuetype` → `issueType`), fix value (`{"name":t}` → `{"issueTypeId": resolved_id}`), add resolver call + cross-project guard |
| `src/api/jira/issues.rs` | new fn `get_issue_types_for_project` | ADD | `GET /rest/api/3/issue/createmeta/{proj}/issuetypes`, analogous to `get_priorities` in `src/api/jira/fields.rs` |
| `src/types/jira/issues.rs` (or fields.rs) | new struct `IssueTypeMetadata` | ADD | Serde struct for `{id: String, name: String}` entries in the createmeta issuetypes response. NOTE: `IssueTypeMetadata` already exists in `src/api/jira/projects.rs` for the `edit --type` 400 enrichment path (BC-3.4.010/011) — reuse or pub(crate)-expose that struct rather than duplicating. Verify location before implementing. |
| `src/types/jira/bulk.rs` | SCHEMA NOTES comment block (~lines 243-252) | MODIFY | Remove "best-guess unverified" caveat for `issueType`; update to the now-verified camelCase-key + issueTypeId shape. |
| `src/cli/issue/create.rs` | dry-run builder block (~line 663-673) | MODIFY | Update comment: "best-guess pending #331" is now resolved. The dry-run `issueType` preview can remain a bare string (still intentionally simplified, same as priority); update the comment to remove the "unverified" qualifier. No behavioral change to the dry-run output format itself. |

### Test Files

| File | Test | Change Type | Description |
|------|------|-------------|-------------|
| `tests/issue_bulk_pr2.rs` | `test_multi_key_type_update_uses_consistent_issuetype_casing` | REWRITE | Currently pins incorrect shape. Must be replaced with a test asserting: (a) `body_str.contains("\"issueType\"")` as an `editedFieldsInput` key, (b) `body_str.contains("\"issueTypeId\"")` in the value, (c) `body_str.contains("\"issuetype\"")` still appears (in `selectedActions`), (d) `body_str` does NOT contain `"\"name\":"` in the issueType context (name-based shape eliminated). Rename test to `test_multi_key_type_update_body_uses_issue_type_id`. |
| `tests/issue_bulk_pr2.rs` | new test: `test_bulk_issuetype_body_uses_issuetype_id_not_name` | ADD | Mirrors `test_bulk_priority_body_uses_priority_id_not_name`. Mounts a mock for `GET /rest/api/3/issue/createmeta/FOO/issuetypes` returning `{values:[{id:"10001",name:"Bug"}]}`, mounts the bulk POST mock requiring `body_string_contains("issueTypeId")`, runs `jr issue edit FOO-1 FOO-2 --type Bug --no-input`, asserts body contains `"issueTypeId"` and does NOT contain `"\"name\":"` in the value position. |
| `tests/issue_bulk_pr2.rs` | new test: `test_bulk_issuetype_cross_project_keys_exits_64` | ADD | New behavior. Runs `jr issue edit FOO-1 BAR-2 --type Bug --no-input` (keys span FOO and BAR), no bulk POST should be issued; asserts exit code 64 and stderr contains an actionable message about cross-project keys and `--type`. |
| `tests/issue_bulk_pr2.rs` | new test: `test_bulk_issuetype_unknown_type_name_exits_non_zero` | ADD | Mounts createmeta issuetypes returning `{values:[{id:"10001",name:"Bug"}]}`, runs `--type Nonexistent`, expects UserError / exit 64 with hint listing valid types (mirrors priority's unknown-name path). |
| `tests/e2e_live.rs` | new gated E2E test | ADD (conditional) | See Open Question Q3 below. If approved: gates behind `JR_RUN_E2E`; creates two same-project issues, bulk-changes `--type` to a type available in the E2E project, verifies via `issue view`. |

### Documentation / Spec Files

| File | Change Type | Description |
|------|-------------|-------------|
| `CLAUDE.md` | MODIFY | Add/update gotcha entry for `issue edit --type` bulk path (multi-key). Specifically: (a) `selectedActions` is lowercase `"issuetype"`, `editedFieldsInput` key is camelCase `"issueType"` — they intentionally differ; (b) cross-project guard exits 64 — `--type` on multi-key bulk requires all keys in the same project; (c) name→issueTypeId resolution uses `GET /rest/api/3/issue/createmeta/{proj}/issuetypes`, project-scoped (not global like priority). |
| `.factory/specs/prd/bc-3-issue-write.md` | MODIFY | See BC Delta section below. |
| `src/types/jira/bulk.rs` | MODIFY | SCHEMA NOTES block — remove unverified caveats, document confirmed shape. |
| `src/cli/issue/create.rs` rustdoc | MODIFY | `handle_edit_bulk_fields` rustdoc (~lines 1280-1296) — update "Issue type: the current implementation sends..." caveat to document the fixed shape and note the project-scoped ID semantics. |

---

## BC Delta

### Existing BC Coverage of Bulk `--type`

There is **no existing BC** explicitly governing the multi-key `--type` bulk-edit wire format.
The only existing test touching the bulk `--type` path is
`test_multi_key_type_update_uses_consistent_issuetype_casing` — which currently pins the
**wrong shape** and uses a loose `body_string_contains` matcher that does not verify the
value object contents.

BCs BC-3.4.010 and BC-3.4.011 explicitly note:
> "This contract applies to SINGLE-KEY edit only. The bulk `--type` path
> (`handle_edit_bulk_fields`) does NOT include this enrichment and must not be modified."

These BCs are unaffected by this fix. The single-key `--type` path (`PUT /rest/api/3/issue/{key}`
with `{"fields":{"issuetype":{"id":"..."}}}`) is byte-for-byte unchanged.

### New BCs Required

**BC-3.4.018 (proposed):** `issue edit KEY1 KEY2 --type <NAME>` multi-key bulk path uses
`editedFieldsInput["issueType"] = {"issueTypeId": "<id-string>"}` with `selectedActions: ["issuetype"]`.
Name is resolved via `GET /rest/api/3/issue/createmeta/{proj}/issuetypes` (case-insensitive exact
match on `name`). On no-match, exit 64 with a UserError listing valid type names. The `issueType`
key in `editedFieldsInput` is camelCase; the `selectedActions` element is lowercase `"issuetype"`.

**BC-3.4.019 (proposed, new behavior):** `issue edit KEY1 KEY2 --type <NAME>` where the supplied
keys span multiple Jira projects → exit 64 with a clear error message indicating that multi-key
`--type` edits require all keys to belong to the same project (issue-type IDs are project-scoped).
No HTTP call to the bulk endpoint is issued. This is the v1 cross-project guard.

**EC additions to existing BCs:**
- BC-3.4.005 (`issue edit` with multiple fields sends both in body simultaneously): add EC for
  `--type + --summary` multi-key where both fields appear in the bulk `editedFieldsInput`.
  Low priority — the fix does not change the multi-field composition logic.

### Revised Count Impact

Section 3.4 currently has 17 contracts (BC-3.4.001..017). This change adds 2 BCs (018, 019),
bringing the count to 19. The `total_bcs` frontmatter in `bc-3-issue-write.md` must be updated;
run `scripts/check-bc-cumulative-counts.sh` after editing.

---

## Regression Risk

**Rating: MEDIUM**

### Risk 1: Single-Key `--type` Path Contamination (HIGH probability if guard is missing)

The fix modifies `handle_edit_bulk_fields`, which is only called for 2+ keys. The single-key
`--type` path goes through `handle_edit` → `edit_issue` (PUT `/rest/api/3/issue/{key}`). These
are independent code paths. **The fix must not touch `handle_edit` or any single-key logic.**

Mitigation: confirm the call-site gate at the top of `handle_edit` that forks to
`handle_edit_bulk_fields` only when `effective_keys.len() >= 2` is unchanged. Existing
single-key `--type` integration tests (BC-3.4.003 suite, BC-3.4.010/011 suite) provide
regression coverage.

### Risk 2: Dry-Run Builder Comment-Only / No Behavioral Change

The dry-run block at lines ~650-690 in `create.rs` emits a bare string for `issueType` in the
`plannedChanges` JSON preview. The comment explicitly states this is "intentionally simplified,
NOT a byte-for-byte snapshot of the wire request." This is the same model used for priority.
**The dry-run output format (bare string) should NOT change** — only the surrounding comment
updates to remove the "unverified" qualifier. Risk: an implementer who reads the old comment
and decides to "fix" the dry-run to also use `issueTypeId` would break BC-3.X (dry-run
simplified preview invariant). Mitigation: spec the dry-run contract explicitly as "bare string
continues to be emitted; only comments updated."

### Risk 3: `IssueTypeMetadata` Struct Duplication

`IssueTypeMetadata` (or equivalent `{id, name}` struct) already exists for the BC-3.4.010/011
error-enrichment path in `src/api/jira/projects.rs`. If the implementer defines a new
duplicate struct instead of reusing the existing one, there will be a lint warning or two
structs with identical layouts that can diverge. Mitigation: spec the story to explicitly check
for and reuse the existing struct.

### Risk 4: Audit Test Negation Logic Reversal

`test_multi_key_type_update_uses_consistent_issuetype_casing` currently ASSERTS that camelCase
`"issueType"` is ABSENT. After the fix this must be PRESENT. If the test is not updated, it
will pass on the OLD (wrong) code and fail on the NEW (correct) code — an inverted safety net.
The story AC must require this test to be rewritten and its assertions validated against the
new body.

### Risk 5: Cross-Project Guard Scope

The guard that detects cross-project key sets must inspect the key prefix (the alpha part before
`-`) to derive the project. This is heuristic — Jira project keys are uppercase letters followed
by `-`. A key like `ALPHA-1` has project `ALPHA`. Edge case: project key with a trailing numeric
component (e.g., `PROJ2-1` → project `PROJ2`). The guard must correctly split on the LAST hyphen
to extract the issue number, keeping all preceding characters as the project key. Mitigation:
spec the split rule explicitly in the story AC and add a unit test covering `PROJ2-1`.

---

## Story Count Recommendation

**Recommended: 1 story (S-331)**

The fix is contained to a single execution path (`handle_edit_bulk_fields`). All five change
types (production fix, new API function, struct reuse, test rewrite, 2 new tests, CLAUDE.md
gotcha, BC additions) are tightly coupled and should ship together. Splitting into sub-stories
(e.g., "schema fix" + "cross-project guard") would leave the feature in a broken intermediate
state between stories.

### Effort Estimate

| Task | LOC estimate |
|------|-------------|
| `handle_edit_bulk_fields` fix (key, value, resolver call, cross-project guard) | ~40–60 LOC |
| `get_issue_types_for_project` API function + response struct (or reuse) | ~25–35 LOC |
| `bulk.rs` SCHEMA NOTES comment update | ~5–8 LOC |
| Dry-run rustdoc comment update | ~3–5 LOC |
| `handle_edit_bulk_fields` rustdoc update | ~5–8 LOC |
| Rewrite `test_multi_key_type_update_uses_consistent_issuetype_casing` | ~30–40 LOC |
| New test: `test_bulk_issuetype_body_uses_issuetype_id_not_name` | ~50–70 LOC |
| New test: `test_bulk_issuetype_cross_project_keys_exits_64` | ~20–30 LOC |
| New test: `test_bulk_issuetype_unknown_type_name_exits_non_zero` | ~20–30 LOC |
| BC-3.4.018 + BC-3.4.019 + CLAUDE.md gotcha | ~60–90 LOC (spec prose) |
| E2E test (if Q3 approved) | ~40–60 LOC |

**Total production code:** ~75–110 LOC.
**Total test code:** ~120–170 LOC (without E2E) or ~160–230 LOC (with E2E).
**Effort:** small-medium; similar to the priority fix (PR #452). Estimated 1–2 dev sessions.

### Acceptance Criteria Sketch (AC for S-331)

1. `jr issue edit FOO-1 FOO-2 --type Bug --no-input` submits a bulk POST whose body contains:
   - `"issueType"` (camelCase) as an `editedFieldsInput` key.
   - `"issueTypeId"` with a string id value resolved from the project's createmeta issuetypes.
   - `"issuetype"` (lowercase) in `selectedActions`.
   - Does NOT contain `"name"` in the issueType value position.
2. `jr issue edit FOO-1 BAR-2 --type Bug --no-input` (cross-project keys) exits 64 with an
   actionable message; no bulk POST is issued.
3. `jr issue edit FOO-1 FOO-2 --type Nonexistent --no-input` exits 64 with a message listing
   valid types for the project (analogous to priority's unknown-name error message).
4. `jr issue edit FOO-1 --type Bug` (single key) behavior is BYTE-FOR-BYTE unchanged. No
   additional HTTP call to createmeta issuetypes. BC-3.4.003/010/011 tests still pass.
5. `test_multi_key_type_update_uses_consistent_issuetype_casing` is rewritten; renamed; now
   asserts camelCase `"issueType"` IS present in body and `"issueTypeId"` IS present.
6. `bulk.rs` SCHEMA NOTES no longer contains "best-guess" or "unverified" qualifiers for issueType.
7. BC-3.4.018 and BC-3.4.019 are added to `bc-3-issue-write.md`; BC count updated to 19;
   `scripts/check-bc-cumulative-counts.sh` exits 0.
8. CLAUDE.md has a gotcha entry for the `--type` multi-key bulk path covering the three
   constraints: camelCase vs lowercase asymmetry, cross-project guard, createmeta resolution.

---

## Open Questions for Human Gate

**Q1 — Cross-project behavior (v1 error-early vs per-project grouping):**

The research document recommends error-early (exit 64) for cross-project multi-key `--type` edits
in v1, because a single `issueTypeId` in the bulk endpoint cannot be valid for all projects
simultaneously. This analysis recommends the same. However, per-project grouping (send one bulk
POST per unique project, each with the project-specific id) is a viable v2 path.

Confirm: **should v1 error-early (exit 64) with a clear cross-project message, rather than
attempt per-project grouping?** If yes, BC-3.4.019 is written to document the exit-64 behavior.
If no, per-project grouping becomes part of this story's scope (significantly increases effort
and test complexity).

**Q2 — Project derivation strategy for multi-key name→id resolution:**

For the cross-project guard to work, the code must derive each key's project from the key string
(e.g., `FOO-1` → project `FOO`, `PROJ2-100` → project `PROJ2`). The proposed approach: split
each key on the last hyphen and take everything before it as the project key. If all keys share
the same prefix, proceed with one createmeta issuetypes call for that project. If they differ,
error early.

Confirm: **is this heuristic (split on last hyphen) the correct project-extraction strategy?**
Is there a scenario where a valid Jira issue key would have a project key containing a hyphen?
(Standard Jira project keys are uppercase ASCII letters plus optional digits — no hyphens — so
this should be safe, but human confirmation is preferred before coding the guard.)

**Q3 — Live E2E test for `--type` bulk edit:**

Priority was validated via a live E2E run (E2E 26735034015 for PR #452). Labels were validated
similarly. Should the `--type` bulk fix also include a gated live E2E test in `tests/e2e_live.rs`
(behind `JR_RUN_E2E=1 + #[ignore]`) that:
- Creates two same-project issues via `jr issue create`.
- Bulk-changes their type via `jr issue edit KEY1 KEY2 --type <type_from_E2E_project>`.
- Asserts via `jr issue view` that both issues now show the new type.

This adds ~40–60 LOC to the story scope. Given that the prior two #331 sub-fixes were validated
live, this is advisable but requires a known available issue type in the `JR_E2E_PROJECT`.

Confirm: **add gated E2E test, and if so, is there a stable second issue type in the E2E project
to use as the target (beyond the default issue type for created issues)?** A new env var
`JR_E2E_ISSUE_TYPE_ALT` (parallel to `JR_E2E_ISSUE_TYPE`) might be needed to specify the
target type for the bulk change.

**Q4 — Cache for createmeta issuetypes lookups:**

The priority resolver calls `GET /rest/api/3/priority` without caching (one HTTP call per bulk
`--priority` invocation). The same model could apply to issueType: no cache, one HTTP call per
bulk `--type` invocation. Alternatively, a per-`(profile, projectKey)` issue-type cache (7-day
TTL, stored as `~/.cache/jr/v1/<profile>/issue_types_<projectKey>.json`) could save round-trips
for repeated use.

Confirm: **ship without a cache (matching priority's model) for v1, or add a 7-day cache in
this story?** If caching is added in this story, it adds ~30–40 LOC to `src/cache.rs` plus
corresponding tests, and requires a CLAUDE.md cache-format gotcha entry alongside the
existing cache schema entries. Recommendation is no-cache for v1 (simplest, matches priority
precedent, cache can be added later if round-trip latency is observed).

---

## Structured Human-Review Questions

### Scope Completeness

1. Are there additional multi-key `--type` scenarios not covered by the three new tests
   (happy path, cross-project, unknown name)?
2. Should `--type + --summary` multi-key bulk (both fields in one editedFieldsInput) get its
   own explicit test, or is the single-field `--type` test sufficient?
3. Is BC-3.4.019 (cross-project guard) classified as a new user-facing behavior that requires
   its own BC, or is it correctly an EC on BC-3.4.018?

### Anchor Correctness

4. The existing `IssueTypeMetadata` struct in `src/api/jira/projects.rs` (used by BC-3.4.010/011
   error enrichment) — confirm it has the right shape (`id: String, name: String`) for reuse
   in the createmeta issuetypes resolver, or if the createmeta response uses different field
   names that require a distinct struct.
5. Confirm the createmeta issuetypes endpoint response uses `values[].id` (string) and
   `values[].name` — not `issueTypes[].id` or a different container field name. The research
   document asserts this but recommends a Context7/live verification before wiring the Serde
   struct (flagged as "inconclusive" in the research doc §Inconclusive).

### Coverage Gaps

6. Is the CLAUDE.md gotcha for `JR_E2E_ISSUE_TYPE_ALT` (or equivalent) needed if Q3 is
   approved? Per CLAUDE.md: "When adding a new `JR_*` test-seam env var, add a parallel line
   in the SAME commit as the code change."
7. Should `test_multi_key_type_update_uses_consistent_issuetype_casing` be renamed or replaced
   in full? It is not referenced anywhere else in the codebase; deletion + replacement is clean.

### Convention Consistency

8. Confirm: the name-resolution error message for unknown `--type` should follow the same
   format as the priority error message:
   `"Issue type '{name}' not found. Valid issue types: {list}. Run \`jr project fields --project {key}\` to see types for your project."`
   Should the `jr project fields` hint be included (currently `jr project fields` lists priority
   and status — does it list issue types?)?
9. Is `test_<verb>_<subject>_<expected_outcome>` the required test naming scheme for all new
   tests in this story, per the CLAUDE.md test-naming convention?

---

## Pre-Implementation Checklist

Before writing any code, the implementer MUST:

- [ ] Read `src/api/jira/projects.rs` and confirm the shape and accessibility of
  `IssueTypeMetadata` (or whatever the existing struct is named). Reuse it rather than
  creating a new duplicate.
- [ ] Verify the createmeta issuetypes response shape by either querying Context7 for the
  Jira REST API v3 docs or running a live curl against a real Jira instance. Confirm the
  container field name (`values` vs `issueTypes` vs something else) before writing Serde
  structs.
- [ ] Read `src/cli/issue/create.rs::handle_edit` carefully to confirm the exact fork point
  where `handle_edit_bulk_fields` is called (the `effective_keys.len() >= 2` gate) and
  verify the single-key path is not touched.
- [ ] Run all existing bulk tests before making any changes: `cargo test --test issue_bulk_pr2`
  — verify they pass on the current branch before the fix.
- [ ] After the fix, verify `test_multi_key_type_update_uses_consistent_issuetype_casing` fails
  before the rewrite (confirming it was pinning the wrong shape) and passes after the rewrite.

---

_Created 2026-06-01. Research source: `.factory/research/issue-331-issuetype-bulk-schema.md`._
_Awaiting human gate Q1–Q4 before proceeding to F2 spec / F3 story._
