# Issue #331 — `issueType` Bulk-Edit Wire-Schema Verification

**Date:** 2026-06-01
**Endpoint:** `POST /rest/api/3/bulk/issues/fields`
**Scope:** Verify the canonical `editedFieldsInput` / `selectedActions` shape to change issue type on the bulk endpoint, before implementing the `jr issue edit KEY1 KEY2 ... --type <NAME>` (multi-key) fix.
**Status:** CONFIRMED (high confidence — verbatim official-docs JSON + repo empirical priority precedent + Perplexity cross-check).

---

## TL;DR Verdict

The current best-guess shape jr ships is **REFUTED on two of three points**:

| Aspect | Current jr code (best-guess) | VERIFIED canonical | Verdict |
|---|---|---|---|
| `editedFieldsInput` key | `"issuetype"` (lowercase) | `"issueType"` (**camelCase**) | REFUTED |
| value object | `{"name": "<type name>"}` | `{"issueTypeId": "<id-string>"}` | REFUTED |
| `selectedActions` value | `"issuetype"` (lowercase) | `"issuetype"` (**lowercase**) | CONFIRMED |
| container form | direct map object (`editedFieldsInput["issuetype"] = {...}`) | direct map object (`editedFieldsInput["issueType"] = {...}`) | CONFIRMED (form right, key casing wrong) |

So: the **container form is correct** (issueType is a direct map object inside `editedFieldsInput`, like `priority` — NOT a `labelsFields`-style array), but the **key casing is wrong** (`issueType` not `issuetype`) and the **value is wrong** (must be `issueTypeId` string, not `name`). The current code will almost certainly 400 / no-op on real Jira.

**The single most important nuance — easy to get wrong:** the `selectedActions` string is lowercase `"issuetype"` while the `editedFieldsInput` key is camelCase `"issueType"`. They intentionally differ. (Same asymmetry the FAQ shows for labels: action string `"labels"`, but no asymmetry there since the container is also `labelsFields`. For issueType/priority the action string is the lowercase system field id while the container key is the camelCase bean name.)

---

## Resolution of source conflicts (why I trust this)

During research, sources DISAGREED on the issueType container form. I resolved the conflict empirically:

- **Perplexity `perplexity_reason` (first call)** claimed `selectedActions: [{"fieldId":"issuetype","type":"edit"}]` (array of OBJECTS) and a `BulkEditActionInputBean` schema. **DISCREDITED** — its inline citations were unrelated JQL-syntax pages; it was reasoning from training, not retrieval. The repo's own CONFIRMED labels schema (#446) uses `selectedActions: ["labels"]` (array of STRINGS), which directly contradicts the object form. Refuted.
- **Perplexity `perplexity_search` (one call)** claimed issueType uses an `issueTypeFields: [{...}]` ARRAY container (like labels) and `priorityFields`. **DISCREDITED** — the repo shipped priority as `editedFieldsInput["priority"] = {"priorityId": "<id>"}` (direct object, NOT `priorityFields` array) in #452 and it **passed live Jira on the first try**. If the `priorityFields` array form were correct, #452 would have 400'd. Refuted.
- **Verbatim fetch of the official Atlassian Bulk Operations FAQ page (two independent WebFetch calls)** returned the literal JSON example below, showing `issueType` and `priority` as DIRECT map objects and `labelsFields` as an array — all three coexisting in one `editedFieldsInput`. This is consistent with the repo's empirically-proven priority and labels shapes. **This is the source of truth.**

The decisive tiebreaker is the repo's existing empirical evidence: priority's `{"priorityId": "<id>"}` direct-object form already passed live Jira, and issueType sits right next to it in the same verbatim FAQ example with the identical direct-object pattern. issueType therefore follows the priority precedent exactly (id-based, camelCase container, direct object) — NOT the labels precedent (array container).

---

## Verified canonical payloads (copy-pasteable)

### Verbatim official FAQ example (the source of truth)

From the Atlassian "Bulk operations: additional examples and FAQs" page, the literal `editedFieldsInput` example shows all three fields together:

```json
{
    "selectedActions": [
        "labels",
        "issuetype",
        "priority"
    ],
    "selectedIssueIdsOrKeys": [
        "10001",
        "10002",
        "10003"
    ],
    "editedFieldsInput": {
        "labelsFields": [
            {
                "fieldId": "labels",
                "labels": [
                    { "name": "Hello" }
                ],
                "bulkEditMultiSelectFieldOption": "ADD"
            }
        ],
        "issueType": {
            "issueTypeId": "10013"
        },
        "priority": {
            "priorityId": "1"
        }
    }
}
```

Source: https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/ (fetched verbatim 2026-06-01).

### Single-key issueType bulk edit (what jr should build)

Note: jr's multi-key bulk path always uses the bulk endpoint for 2+ keys. (A single key takes the `PUT /rest/api/3/issue/{key}` path for labels today; issueType single-key is the legacy `editmeta`/PUT path — see Implementation implications. The shape below is the bulk-endpoint shape, which jr's `handle_edit_bulk_fields` builds for 2+ keys.)

```json
{
    "selectedActions": ["issuetype"],
    "selectedIssueIdsOrKeys": ["PROJ-1"],
    "editedFieldsInput": {
        "issueType": { "issueTypeId": "10013" }
    }
}
```

### Multi-key issueType bulk edit

```json
{
    "selectedActions": ["issuetype"],
    "selectedIssueIdsOrKeys": ["PROJ-1", "PROJ-2", "PROJ-3"],
    "editedFieldsInput": {
        "issueType": { "issueTypeId": "10013" }
    }
}
```

Exact-shape facts (all verbatim-confirmed):
- `selectedActions` element for issue type: literal lowercase string `"issuetype"`.
- `editedFieldsInput` key: literal camelCase `"issueType"`.
- value object: `{ "issueTypeId": "<id>" }` where `<id>` is a **STRING** (e.g. `"10013"`, NOT integer `10013`).
- top-level issue list property: `"selectedIssueIdsOrKeys"` (this matches jr's existing `BulkEditRequest.selected_issue_ids_or_keys`).

---

## Name → issueTypeId resolution

The bulk endpoint is **ID-only**. There is no `{"name": "Bug"}` form for issueType on this endpoint (the FAQ value object only ever shows `issueTypeId`). jr must resolve the user-supplied `--type <NAME>` to a numeric issue-type id string before building the payload. This mirrors what #452 did for priority (`GET /rest/api/3/priority` → resolve name → `priorityId`).

### Resolution endpoint(s)

Issue-type IDs are **project-scoped** (the same issue-type NAME can have different IDs in different projects, and a project's issue-type scheme determines which types are valid). The correct resolution path is the create-meta issue-types endpoint, scoped to the target project:

- **Preferred (paginated, project + issuetype scoped):**
  `GET /rest/api/3/issue/createmeta/{projectIdOrKey}/issuetypes`
  Returns the issue types available for that project, each with `id` (string) and `name`. Resolve `--type` case-insensitively against `name`, take `id`.
  Docs: https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/#api-rest-api-3-issue-createmeta-projectidorkey-issuetypes-get

- **Alternative (global, NOT project-scoped — use with caution):**
  `GET /rest/api/3/issuetype` returns ALL issue types across the instance. This does NOT tell you which types are valid for a given project's scheme, and can return duplicate names with different ids. Only safe if you also validate against the project's scheme. **Not recommended** as the primary resolver for a multi-project bulk edit.

- **Per-project-scoped variant:**
  `GET /rest/api/3/issuetype/project?projectId={id}` returns issue types for a single project. Usable but requires a numeric projectId (extra lookup from key); the createmeta issuetypes endpoint is the more direct fit since jr already deals in project keys.

### CRITICAL per-project caveat for multi-key bulk

A multi-key bulk edit (`jr issue edit FOO-1 BAR-2 --type Bug`) **may span multiple projects**, and:

1. Issue-type IDs differ per project. `"Bug"` might be id `10001` in project FOO and `10103` in project BAR.
2. The bulk endpoint takes ONE `issueTypeId` for the entire batch. There is no per-issue id in the `issueType` container.

Consequences jr must handle:
- If all keys are in the **same project**, resolve `--type` once against that project's createmeta issuetypes and use the single id. Safe.
- If keys span **multiple projects**, a single `issueTypeId` is almost certainly invalid for at least some issues (the bulk task will report per-issue failures, or the type id won't exist in the other project's scheme). jr should either:
  - (a) detect cross-project key sets and **error early** with a clear message ("`--type` on a multi-key edit requires all issues to be in the same project; resolve issue-type ids differ per project"), or
  - (b) group keys by project, resolve the id per project, and issue one bulk POST per project group.
  Recommendation: ship (a) first (simplest, safe, matches the de-risking goal); track (b) as a follow-up. This is a genuinely new constraint that priority did NOT have (priority ids are global, not project-scoped).

---

## Documented constraints & error shapes

- **Same issue-type scheme constraint:** the target issue type must be in the issue's project's issue-type scheme. Changing to a type not in the scheme fails per-issue.
- **Standard ↔ sub-task constraint:** you generally cannot bulk-edit a standard issue into a sub-task type (or vice versa) via this endpoint — that is a *move* operation. Atlassian routes type changes that alter the hierarchy level through `POST /rest/api/3/bulk/issues/move`, not `/bulk/issues/fields`. If `--type` targets a sub-task type for standard issues, expect per-issue failures.
- **Required-field validation on the new type:** if the new issue type has required fields not present on the issues, the bulk task may report per-issue validation failures (the new type's required fields must be satisfiable).
- **Async task model (unchanged):** the endpoint returns `200` with a `taskId`; success/failure is reported per-issue via the poll endpoint `GET /rest/api/3/bulk/queue/{taskId}` (jr's existing `await_bulk_task` / `BulkOperationProgress`). Invalid issue-type ids surface as FAILED tasks or per-issue failures in the progress body's failure list, NOT as a synchronous 400 on submit (submit-time 400s are reserved for malformed payloads, e.g. wrong key casing or missing `selectedActions`).
- **Batch limits:** up to 1000 issues / 200 fields per request (matches jr's existing `max 1000` note). Source: bulk operations group docs.
- **Submit-time 400 triggers (the ones jr's shape bug would hit):** wrong field key in `editedFieldsInput` (e.g. lowercase `issuetype`), `editedFieldsInput` present without matching `selectedActions` entry, or an integer instead of a string id. These are payload-shape errors caught at submit.

---

## Cross-check: priority precedent (#452)

`#452` fixed priority to `editedFieldsInput["priority"] = {"priorityId": "<id-string>"}` (camelCase container key, id-based value) and passed live Jira first try. issueType follows the **identical pattern**:

| | priority (#452, proven) | issueType (this report) |
|---|---|---|
| `selectedActions` string | `"priority"` | `"issuetype"` |
| `editedFieldsInput` key | `"priority"` (camelCase) | `"issueType"` (camelCase) |
| value object | `{"priorityId": "<id>"}` | `{"issueTypeId": "<id>"}` |
| id JSON type | **string** | **string** |
| id scope | global (instance-wide) | **project-scoped** (key divergence) |
| resolution endpoint | `GET /rest/api/3/priority` | `GET /rest/api/3/issue/createmeta/{proj}/issuetypes` |

**Consistency:** confirmed — both are id-based, camelCase-container, string-id, direct-object (NOT array). **One divergence:** issueTypeId is **project-scoped** whereas priorityId is global. This is the only place issueType is harder than priority, and it is the single thing implementation must get right for multi-project key sets.

Note on id JSON type: the FAQ shows BOTH `priorityId` and `issueTypeId` as **strings** (`"1"`, `"10013"`). jr's priority code already clones the priority `id` as a `String` and ships it as a JSON string — consistent. issueType should do the same (id as JSON string, not int). (The CLAUDE.md task brief's phrasing "priorityId is an int" is inaccurate vs the verbatim FAQ — both are strings; treat issueTypeId as a string.)

---

## Cross-check: labels precedent (#448/#446) — DIFFERENT container

Labels use a top-level-of-`editedFieldsInput` `labelsFields` **array** (`editedFieldsInput["labelsFields"] = [{...}]`), because labels are multi-value add/remove operations. issueType is single-value "set", so it does **NOT** use an array container. issueType goes in `editedFieldsInput["issueType"]` as a **direct object**, exactly like priority. Confirmed by the verbatim FAQ example where all three (`labelsFields` array, `issueType` object, `priority` object) coexist in one `editedFieldsInput`.

So to answer RQ5 directly: **issueType goes in `editedFieldsInput` as a direct map object (the priority pattern), NOT its own top-level array (the labels pattern).**

---

## Implementation implications

**1. New name→id resolution code: YES.** Add an issue-type resolver analogous to the priority resolver in `handle_edit_bulk_fields`. Call `GET /rest/api/3/issue/createmeta/{projectIdOrKey}/issuetypes`, match `--type` case-insensitively against `name`, take the string `id`. On no-match, return a `UserError` listing valid type names (mirror the priority error message + suggest `jr project fields`).

**2. Per-project handling: YES — this is the new hard part.** Unlike priority (global ids), issue-type ids are project-scoped. The multi-key bulk endpoint takes ONE `issueTypeId`. Recommended v1: derive the project prefix from the resolved keys; if they span >1 project, error early with a clear message; if single-project, resolve once and proceed. (Optional v2: per-project grouping + one POST per group.)

**3. New cache: OPTIONAL, not required for correctness.** createmeta issuetypes is one extra HTTP per `--type` bulk call (same cost model as priority's `GET /rest/api/3/priority`). A per-`(profile, projectKey)` issue-type cache (7-day TTL, sibling to existing caches) would save round-trips but is a nice-to-have. Recommendation: ship without a cache first (matches priority's no-cache approach), add later if needed. If added, follow the "read-acceleration shortcut → swallow+warn on write error" model per CLAUDE.md.

**4. Single-key vs multi-key fork: NO new fork needed for the bulk path itself.** The `editedFieldsInput["issueType"] = {"issueTypeId": ...}` shape is what `handle_edit_bulk_fields` builds for the 2+ key path. Unlike labels (which needed a single-key `PUT /rest/api/3/issue/{key}` divergence because the bulk endpoint 400'd for single keys), there is no evidence the issueType bulk shape 400s for a single key — but jr's current single-key `--type` likely goes through the platform `editmeta`/PUT path already (`{"fields":{"issuetype":{"id":"..."}}}`), which is independently correct. Verify the single-key `--type` path is unaffected by this change; the fix is scoped to the multi-key bulk builder in `handle_edit_bulk_fields`.

**5. Concrete code change (src/cli/issue/create.rs `handle_edit_bulk_fields`, ~lines 1334-1340):**
   - Change `edited.insert("issuetype".into(), json!({"name": t}));`
     to `edited.insert("issueType".into(), json!({"issueTypeId": resolved_id}));` (camelCase key, id value).
   - Keep `selected_actions.push("issuetype".to_string());` (lowercase — already correct).
   - Add the createmeta resolution + cross-project guard before the insert.
   - Update the `BulkEditRequest`/`bulk.rs` SCHEMA NOTES (lines ~243-252) and the dry-run builder comments (~651-689, ~1273-1339) to reflect the now-verified shape, and unify the dry-run preview if desired.

**6. Recommended verification before merge:** add a wiremock test pinning `body_string_contains("issueType")` AND `body_string_contains("issueTypeId")` (mirror `test_bulk_priority_body_uses_priority_id_not_name`), plus a live-E2E probe (`JR_RUN_E2E`) that creates two same-project issues and bulk-changes their type, then asserts via `issue view`. The schema is high-confidence from verbatim docs + the priority precedent, but a live probe is the cheap final tiebreaker — and it directly validates the project-scoped id resolution, which has no precedent in the codebase.

---

## Inconclusive / flagged items

- **Cross-project bulk behavior (exact error shape):** I did not find a verbatim doc example of what the bulk task reports when one `issueTypeId` is invalid for some issues in a multi-project batch. Strongly inferred (per-issue FAILED entries in the progress body) but not verbatim-confirmed. The recommended early cross-project guard sidesteps this entirely for v1. If implementing per-project grouping (v2), run a live probe to capture the exact failure shape first.
- **createmeta vs `issuetype/project` choice:** both endpoints resolve name→id project-scoped; I recommend createmeta issuetypes because jr deals in project keys (no numeric projectId lookup needed). Not a correctness issue, just ergonomics — confirm the createmeta issuetypes response shape (`values[].id`, `values[].name`, paginated) against Context7/live before wiring.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity perplexity_reason | 1 | Initial bulk issueType schema (DISCREDITED — hallucinated `selectedActions` object form + `BulkEditActionInputBean`; citations unrelated) |
| Perplexity perplexity_search | 3 | Cross-check container form (caught + resolved the `issueTypeFields` array vs direct-object conflict), selectedActions casing |
| WebFetch | 5 | Atlassian OpenAPI JSON (partial), and verbatim JSON extraction from the official Bulk Operations FAQ page (the decisive source of truth) |
| Read (repo) | 3 | Current jr shape in create.rs + bulk.rs SCHEMA NOTES; verified priority precedent shape |
| Grep (repo) | 3 | labelsFields container form, priority test pins, dry-run builder |
| Training data | 1 area | issue-type scheme / sub-task move constraints (general Jira knowledge — flagged; the move-vs-edit distinction corroborated by Perplexity source [1] of the issueTypeFields query) |

**Total MCP/tool calls:** ~11 external (4 Perplexity, 5 WebFetch) + 6 repo (Read/Grep).
**Training data reliance:** low — every wire-shape claim is grounded in either verbatim official-docs JSON (fetched twice for consistency) or the repo's empirically-proven priority/labels precedents. The two source conflicts encountered were resolved against the repo's live-Jira-passing precedents, not training. Only the general issue-type-scheme/sub-task constraints lean partly on model knowledge, and those are corroborated by Atlassian support docs.

### Sources
- https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/ (verbatim editedFieldsInput JSON — source of truth)
- https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/ (endpoint group, batch limits, move-vs-edit)
- https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/#api-rest-api-3-issue-createmeta-projectidorkey-issuetypes-get (name→issueTypeId resolution)
- Repo precedent: `tests/issue_bulk_pr2.rs` (priority `priorityId` string, #452 live-Jira-proven), `src/cli/issue/create.rs::build_labels_edited_fields` (labels `labelsFields` array, #446 live-Jira-proven)
