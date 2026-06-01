# Research: E2E gating for priority / issuetype bulk edit, single-issue priority, priority discovery, worklog, unassign

Date: 2026-06-01
Scope: Gate new live-Jira E2E tests + the #331 bulk payload bug fix for `jr`.
Method: Perplexity PRIMARY (search/reason) + direct fetch of the authoritative Atlassian
"Bulk Operation APIs: Additional Examples and FAQs" page (the same source that verified the
LABELS schema). Perplexity's free-text *reasoning* on the bulk schema was UNRELIABLE (it
hallucinated `priorityFields`/`issueTypeFields` arrays and a `selectedFieldOption` shape — both
WRONG); the verbatim FAQ JSON is the ground truth and is what this report relies on.

---

## Test-design + fix implications (READ THIS FIRST)

**The bulk priority/issuetype bug is confirmed and the corrected wire shapes are known.** jr's
`handle_edit_bulk_fields` (`src/cli/issue/create.rs::handle_edit_bulk_fields`) currently sends
`editedFieldsInput = {"priority": {"name": p}, "issuetype": {"name": t}}`. Both are wrong.
The verified-correct `editedFieldsInput` for `POST /rest/api/3/bulk/issues/fields` is:

```json
{
  "selectedActions": ["priority", "issuetype"],
  "selectedIssueIdsOrKeys": ["K1", "K2"],
  "editedFieldsInput": {
    "priority":  { "priorityId":  "3"     },
    "issueType": { "issueTypeId": "10001" }
  },
  "sendBulkNotification": false
}
```

Two structural corrections vs. today's code, plus a value-form correction:
1. **Priority** key stays `"priority"` but the inner shape is `{"priorityId": "<id>"}` — an `id`
   string, NOT `{"name": "High"}`. (jr's CLAUDE.md note "we currently ship `{"name": ...}`" is the
   bug.) Note: jr's older single-key/dry-run path sent a bare `{"priorityId": <int>}` (integer, no
   wrapper key) — that is ALSO wrong: the value must be a **string** and must live under the
   top-level `"priority"` object inside `editedFieldsInput`.
2. **Issue type** inner object key is `issueType` (camelCase) carrying `{"issueTypeId": "<id>"}`
   — an `id` string, NOT `{"issuetype": {"name": "Bug"}}` (lowercase + name). The
   `selectedActions` value, however, is the lowercase `"issuetype"` (the FAQ example lists
   `selectedActions: ["labels","issuetype","priority"]` while `editedFieldsInput` uses the
   camelCase `issueType` object key — these intentionally differ).
3. Both fields are **id-by-string**, so jr must RESOLVE name→id before building the bulk body.
   The bulk endpoint does not accept names. (Contrast: the single-issue `PUT` path DOES accept
   names — see Q3.)

**Portable E2E decisions (no hardcoded "High"):**
- To pick a priority for a test, do NOT hardcode `"High"`. Resolve the project's valid
  priorities at runtime and pick one whose `id` differs from the issue's current `priority.id`.
  Preferred discovery: `GET /rest/api/3/priority/search?projectId=<id>` (project-scheme-aware,
  returns `values[]` with stable `id`/`name`/`isDefault`). Fallback for older behavior:
  `fields.priority.allowedValues` from `GET /rest/api/3/issue/createmeta`. Do NOT rely on the
  global `GET /rest/api/3/priority` for "valid for this project" — with priority schemes a global
  priority can 400 on issue edit if it is not in the project's scheme.
- For the bulk E2E test specifically: read two issues' current priority via
  `GET /rest/api/3/issue/{key}?fields=priority`, pick a target `priorityId` from
  `priority/search` that is `!=` the current one for both, fire the bulk edit, then poll-read
  back. Same pattern for issue type, but issue-type changes are workflow/screen-sensitive — gate
  the issuetype bulk E2E behind `JR_E2E_ISSUE_TYPE` (already documented) and an alternate target
  type that shares the project's workflow, or skip cleanly if only one type exists.
- **Single-issue priority** (jr routes single-key edits to `PUT /rest/api/3/issue/{key}`): the
  PUT path accepts `{"fields":{"priority":{"name":"High"}}}` OR `{"id":"3"}`. `id` (string) is
  the Atlassian-recommended canonical form (name is display-data, localizable/editable). For a
  portable E2E assertion, resolve to `id` and assert on `id` after read-back.
- **Worklog** add→list is read-after-write consistent in Cloud v3 (the "missing last minute"
  caveat is Data-Center `/worklog/updated`-only). Minimal add body: `{"timeSpentSeconds": 3600}`.
  No tier gate beyond the "Work on issues" permission; a clean add→list→assert E2E is safe.
- **Unassign** via `PUT /rest/api/3/issue/{key}/assignee` `{"accountId": null}` is read-after-
  write consistent, BUT a project with "Allow unassigned issues" disabled (or a forced default
  assignee / assign post-function) will silently re-apply an assignee, so `GET` shows a user, not
  `null`. The unassign E2E MUST detect this and skip rather than fail: after unassigning, if
  `fields.assignee != null`, treat as "project disallows unassigned" and skip (do not assert).

Net code-fix recommendation for #331: change `handle_edit_bulk_fields` to resolve
priority-name→id and issuetype-name→id, then emit `{"priority":{"priorityId":"<id>"}}` and
`{"issueType":{"issueTypeId":"<id>"}}`; keep `selectedActions` values lowercase
(`"priority"`, `"issuetype"`). Mirror the labels fix that already landed (`labelsFields` array).

---

## Q1. Bulk edit PRIORITY — exact editedFieldsInput schema

**Verdict: jr's current `{"priority":{"name": p}}` is WRONG. Correct shape is
`{"priority":{"priorityId":"<id-string>"}}`.**

- There is NO `priorityFields` / `prioritiesFields` array. Perplexity's reasoning model invented
  one; the verbatim Atlassian FAQ JSON does not have it. Priority is a single-valued object
  directly under `editedFieldsInput`, keyed `"priority"`, with one property `"priorityId"`.
- `priorityId` value is a **string** (the FAQ example shows `"1"`, quoted). Not an integer, not a
  name. jr's older bare-integer `{"priorityId": <int>}` is doubly wrong (no wrapper + int type).
- Required top-level keys: `selectedActions` (string[]), `selectedIssueIdsOrKeys` (string[]),
  `editedFieldsInput` (object), `sendBulkNotification` (bool).

Complete correct body for setting priority on `["K1","K2"]`:

```json
{
  "selectedActions": ["priority"],
  "selectedIssueIdsOrKeys": ["K1", "K2"],
  "editedFieldsInput": {
    "priority": { "priorityId": "3" }
  },
  "sendBulkNotification": false
}
```

Source (verbatim JSON, all three fields in one example):
https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/
Endpoint reference:
https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/

---

## Q2. Bulk edit ISSUE TYPE — exact schema

**Verdict: jr's current `{"issuetype":{"name": t}}` is WRONG. Correct shape is
`{"issueType":{"issueTypeId":"<id-string>"}}`.**

- NO `issueTypeFields` array (again, Perplexity reasoning hallucinated it). The object is keyed
  `"issueType"` (camelCase) under `editedFieldsInput`, with one property `"issueTypeId"`.
- `issueTypeId` value is a **string** (FAQ shows `"10013"`). An id, not a name.
- Casing asymmetry to preserve: `selectedActions` lists the field as lowercase `"issuetype"`,
  while the `editedFieldsInput` object key is camelCase `"issueType"`. Both appear in the same
  authoritative example.

Complete correct body for setting issue type on `["K1","K2"]`:

```json
{
  "selectedActions": ["issuetype"],
  "selectedIssueIdsOrKeys": ["K1", "K2"],
  "editedFieldsInput": {
    "issueType": { "issueTypeId": "10001" }
  },
  "sendBulkNotification": false
}
```

Caveat: changing issue type is workflow/screen-sensitive; incompatible target types or missing
required fields cause per-issue failures in the async bulk task result (not a 400 on submit).
E2E must read back and tolerate per-key failure, or pick a workflow-compatible target type.

Source: same FAQ page (verbatim `"issueType": {"issueTypeId": "10013"}`):
https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/

---

## Q3. Single-issue priority via PUT

**Verdict: `PUT /rest/api/3/issue/{key}` with `{"fields":{"priority":{"id":"3"}}}` is canonical.
`{"name":"High"}` ALSO works but `id` (string) is the recommended, portable form.**

- `fields.priority` is a Priority resource (id/name pair). Either `{"id":"<string>"}` or
  `{"name":"<string>"}` is accepted on the issue edit/create endpoints.
- Atlassian recommends `id` because names are display data (localizable, admin-editable) while
  `id` is stable. For an E2E assertion that survives instance renames, resolve to `id` and assert
  on `id`.
- Name-based setting is reliable as long as the name is unique and unchanged in the instance —
  acceptable for a controlled E2E project but `id` is safer.

```json
{
  "fields": { "priority": { "id": "3" } }
}
```

Source:
https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/
(REST v3 intro confirms entity-by-string-id convention):
https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/

---

## Q4. Discovering valid priorities (portability)

**Recommended portable approach: `GET /rest/api/3/priority/search?projectId=<id>`, then select
by `id` (and prefer `isDefault` if you just need "any valid one").**

- `GET /rest/api/3/priority` (global) is NOT deprecated, but with priority schemes it returns
  priorities that may be INVALID for a given project — using one can 400 on edit. Do not use it
  to decide "valid for this project".
- `GET /rest/api/3/priority/search?projectId=<projectId>` returns a paginated `values[]` of
  priorities valid under the project's scheme, each with stable `id`, `name`, and an `isDefault`
  flag. This is the project-scheme-aware, scheme-respecting source.
- Fallback (older behavior / DC parity): `GET /rest/api/3/issue/createmeta?projectIds=...&
  issuetypeIds=...&expand=projects.issuetypes.fields` → `fields.priority.allowedValues[]`.

"Pick a valid priority different from the issue's current one" recipe for E2E:
1. `GET /rest/api/3/issue/{key}?fields=priority` → current `priority.id`.
2. `GET /rest/api/3/priority/search?projectId=<id>` → `values[]`.
3. Choose the first `values[i].id != current.id`. If only one priority exists in the scheme,
   skip the test (cannot make a distinguishable change).

Source:
https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-priorities/
https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/ (createmeta)

---

## Q5. Worklog add + read-back

**Verdict: minimal add body is `{"timeSpentSeconds": 3600}`; add→GET is read-after-write
consistent in Cloud v3. Safe for a clean add-then-list E2E assertion.**

- Add: `POST /rest/api/3/issue/{key}/worklog` with at minimum `timeSpentSeconds` (integer
  seconds). For 1h: `{"timeSpentSeconds": 3600}`. Optional: `comment` (ADF), `started`,
  `visibility`. The POST response returns the created worklog incl. its `id`.
- Read-back: `GET /rest/api/3/issue/{key}/worklog` (list) or `GET .../worklog/{id}` (by id) are
  strongly consistent in Cloud v3 — the created worklog appears immediately.
- The "will not return worklogs updated during last minute" caveat is DATA CENTER
  `/rest/api/.../worklog/updated` ONLY — it does NOT apply to Cloud v3 issue-scoped worklog
  reads. No special tier gate; requires the "Work on issues" permission. Note: if jr sends
  `timeSpent` ("1h") instead of `timeSpentSeconds`, both are accepted by the API, but
  `timeSpentSeconds` is unambiguous and recommended for the E2E body.

```json
{ "timeSpentSeconds": 3600 }
```

Source:
https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-worklogs/
DC-only caveat contrast:
https://developer.atlassian.com/server/jira/platform/rest/v10003/api-group-worklog/

---

## Q6. Unassign semantics (reconfirm)

**Verdict: `PUT /rest/api/3/issue/{key}/assignee` `{"accountId": null}` unassigns and is
read-after-write consistent — BUT project config can silently re-assign, so the E2E must
skip-not-fail when `fields.assignee` comes back non-null.**

- Endpoint: `PUT /rest/api/3/issue/{key}/assignee`, body `{"accountId": null}` → docs:
  "if accountId is set to null, the issue is set to unassigned." (Use PUT, not POST.)
- Read-back: `GET /rest/api/3/issue/{key}?fields=assignee` reflects the change immediately
  (strong consistency for issue edits). A non-null result after a 2xx PUT is a CONFIG artifact,
  not eventual consistency.
- Caveats that force a skip:
  1. Project "Allow unassigned issues" DISABLED → Jira re-applies a default assignee (project
     lead / "Automatic"); GET shows a user, not null.
  2. Project default assignee forces a user.
  3. Automation rules / workflow post-functions re-assign after edit.
- E2E pattern: unassign → GET assignee; if `null` assert pass; if non-null, SKIP with a log line
  ("project disallows unassigned or has a default-assignee/automation; skipping unassign check").
  Consider an opt-in env gate if the provisioned E2E project's unassigned policy is unknown.

```json
{ "accountId": null }
```

Source:
https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/
Config caveat (community-confirmed, behavior-only):
https://community.developer.atlassian.com/t/set-issue-to-unassigned-via-api/82622

---

## Confidence / uncertainty flags

- Q1, Q2 (bulk shapes): HIGH confidence — verbatim from the same authoritative Atlassian FAQ page
  that verified the labels schema; the `priorityId`/`issueTypeId` string forms are quoted in the
  page's single combined example. The `selectedActions` lowercase vs `editedFieldsInput` camelCase
  asymmetry for issue type is taken directly from that example and should be double-checked once
  against a live sandbox before the #331 fix lands (cheap to verify with `--verbose-bodies`).
- Q3, Q4, Q5, Q6: HIGH confidence on the documented endpoints/shapes; the unassign and
  priority-scheme CAVEATS are config-dependent runtime behaviors (community-sourced but
  consistent and matching Atlassian's own field docs) — hence the skip-not-fail E2E guidance.
- Perplexity's *reasoning* tool was explicitly UNRELIABLE for the bulk schema (hallucinated array
  keys). All bulk-schema claims here rest on the fetched FAQ JSON, not on model free-text.

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity perplexity_reason | 2 | Bulk priority + issuetype schema (UNRELIABLE — hallucinated; superseded by FAQ fetch) |
| Perplexity perplexity_search | 3 | priority discovery/scheme; single-issue PUT priority + worklog; unassign caveats |
| WebSearch | 2 | locate authoritative bulk FAQ page + surface verbatim editedFieldsInput example |
| WebFetch | 4 | fetch bulk FAQ page (verbatim JSON — ground truth); dev-docs pages (2 truncated) |
| Grep | 1 | confirm jr's current wrong bulk payload in src/cli/issue/create.rs |
| Read | 1 | read handle_edit_bulk_fields to pin the exact fix site |
| Training data | 0 areas | none relied upon for claims; all sourced |

**Total MCP tool calls:** 5 Perplexity + (Context7 not used)
**Total tool calls (incl. web/local):** 13
**Training data reliance:** low — every claim is tied to an Atlassian URL or the fetched FAQ JSON;
the bulk schema specifically was taken from verbatim docs after Perplexity reasoning proved wrong.
