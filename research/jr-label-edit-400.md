# jr label-edit HTTP 400 "Invalid request payload" — root cause + fix

**Date:** 2026-05-31
**Researcher:** research agent (Perplexity-primary, Atlassian-docs-verified)
**Status:** complete — definitive

---

## Fix recommendation

Route **single-key** label add/remove through **`PUT /rest/api/3/issue/{issueIdOrKey}`** using the `update` verb with **bare-string** label values (NOT `{"name":"foo"}` objects, NOT the bulk endpoint). The exact body `jr` should send for `jr issue edit KEY --label add:foo --label remove:bar` is:

```json
{
  "update": {
    "labels": [
      { "add": "foo" },
      { "remove": "bar" }
    ]
  }
}
```

This is a single HTTP round-trip, returns `204 No Content` on success, and avoids every defect in the current bulk path. The current 400 is caused by `jr` sending a **fabricated, malformed bulk payload** (`editedFieldsInput.labels.labelsAction` with `{"name":...}` items and no `selectedActions`) to `POST /rest/api/3/bulk/issues/fields` — none of those property names match the real schema (see Q2). Switch single-key edits to the `PUT` above; reserve the bulk endpoint for 2+ keys and fix its payload to the real `labelsFields` schema.

---

## Q1 — Canonical single-issue label add/remove (PUT issue)

**Endpoint:** `PUT /rest/api/3/issue/{issueIdOrKey}` → `204 No Content` on success.

The `update` verb takes `labels` as an **array of operation objects**, each with a single key (`add` or `remove`) whose value is a **bare label string**. Confirmed — your proposed syntax is correct:

```json
{
  "update": {
    "labels": [
      { "add": "foo" },
      { "remove": "bar" }
    ]
  }
}
```

- Label values are **bare strings** (`"add": "foo"`), **NOT** `{"name":"foo"}` objects. The `{"name":...}` object form is a bulk-endpoint-only construct (Q2) and is invalid here.
- You can mix multiple `add` and `remove` operations in the same array in one request.
- A pure add of one label: `{"update":{"labels":[{"add":"foo"}]}}`.
- Add-only via `fields` (full replace) is also possible — `{"fields":{"labels":["foo","bar"]}}` — but that **overwrites** all labels, so use `update` for incremental add/remove.

**Citations:**
- Atlassian "Advanced field editing using JSON" — labels example `{"update":{"labels":[{"add":"test-label"}]}}` (bare string): https://confluence.atlassian.com/spaces/automation112/pages/1688902281/Advanced+field+editing+using+JSON
- Jira Cloud REST v3 Edit issue endpoint: https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/
- Jira REST API examples (same `fields`/`update` add/remove structure): https://developer.atlassian.com/server/jira/platform/jira-rest-api-examples/

---

## Q2 — Bulk fields endpoint payload (the real schema)

**Endpoint:** `POST /rest/api/3/bulk/issues/fields`

The current jr "best-guess" payload is wrong on **every** count. The real, documented schema:

```json
{
  "selectedActions": ["labels"],
  "selectedIssueIdsOrKeys": ["10001", "10002"],
  "editedFieldsInput": {
    "labelsFields": [
      {
        "fieldId": "labels",
        "labels": [ { "name": "Hello" } ],
        "bulkEditMultiSelectFieldOption": "ADD"
      }
    ]
  }
}
```

Answering each sub-question:

- **(a) Shape of the labels value:** Labels live under `editedFieldsInput.labelsFields`, which is an **ARRAY** of label-field objects (NOT `editedFieldsInput.labels`, NOT a single object). Each item is keyed by `fieldId` (`"labels"` for the system labels field). So it is a `BulkEditGetFields`-style `fieldId`-keyed item inside the `labelsFields` array — exactly the "something else entirely" branch.
- **(b) Action property + casing:** The action property is **`bulkEditMultiSelectFieldOption`** — NOT `labelsAction`. Allowed enum values are **`ADD`**, **`REPLACE`**, **`REMOVE`**, **`REMOVE_ALL`** (all upper-case). `"ADD"`/`"REMOVE"` exist but the property name jr uses (`labelsAction`) does not.
- **(c) Label items:** In the **bulk** endpoint, label items ARE `{"name":"..."}` objects (unlike the `PUT issue` endpoint, where they are bare strings). This asymmetry is the trap jr fell into — it borrowed the object form but put it under the wrong property names.
- **(d) Required top-level keys:** `selectedActions` (array of action identifiers — MUST contain `"labels"` for a label edit), `selectedIssueIdsOrKeys` (array), and `editedFieldsInput` (object). `sendBulkNotification` is optional. The current jr payload **omits `selectedActions` entirely**, which alone guarantees a 400. The endpoint is NOT a free-for-all: it edits only an allowlist of fields, discoverable via `GET /rest/api/3/bulk/issues/editable/fields`; you must declare each field group in `selectedActions`.

**Citations:**
- Atlassian official bulk-operation examples/FAQs — exact `labelsFields` payload quoted above: https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/
- Jira Cloud REST v3 Issue bulk operations API group (`POST /rest/api/3/bulk/issues/fields`, `GET /rest/api/3/bulk/issues/editable/fields`): https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/

**Why jr's current bulk payload 400s — concrete diff:**

| jr sends (wrong) | Real schema (correct) |
|---|---|
| `editedFieldsInput.labels` (object) | `editedFieldsInput.labelsFields` (array) |
| `labelsAction: "ADD"` | `bulkEditMultiSelectFieldOption: "ADD"` |
| no `fieldId` | `fieldId: "labels"` |
| no `selectedActions` | `selectedActions: ["labels"]` (required) |
| `labels: [{"name":"foo"}]` | `labels: [{"name":"foo"}]` (this part is OK for bulk) |

---

## Q3 — Which approach is correct for jr

**Recommendation: single-key → `PUT issue` (Q1); 2+ keys → bulk endpoint (Q2, corrected).**

- For a **single key** (`jr issue edit KEY --label add:x`), the `PUT /rest/api/3/issue/{key}` path is simpler, synchronous (`204` immediately, no async task to poll), requires no `selectedActions` plumbing, and uses the lighter bare-string `update` form. There is no reason to involve the bulk endpoint for one issue.
- For **2+ keys**, keep `POST /rest/api/3/bulk/issues/fields`, but rewrite the payload to the real `labelsFields` schema (Q2) and add the required `selectedActions: ["labels"]`. The bulk endpoint is asynchronous (returns a task to monitor), so it is only worth the extra complexity when batching.
- **Why the single PUT avoids the bulk 400:** the PUT endpoint has no `selectedActions` gate and accepts the simpler `update` verb that jr already builds correctly elsewhere (e.g. for other field edits). The bulk 400 is purely a payload-shape mismatch (wrong property names + missing required `selectedActions`), not a permissions or field-not-editable issue — labels ARE bulk-editable.

**Citations:** same as Q1 and Q2 above.

---

## Q4 — Label value constraints (independent 400 risks)

- **All-leading-digits / all-digit labels are LEGAL** on Jira Cloud. A value like `26730687481-probe` or `26730687481` is valid — nothing in the label constraints forbids numeric-only or digit-leading values. So the probe label is **not** the cause of the 400.
- **Spaces are NOT allowed** — labels are single tokens; a label containing a space is rejected with `400 Bad Request`. (Not relevant to the probe label, but the most common legitimate label-value 400.)
- **Length:** must be under 255 characters.
- **Disallowed characters:** special characters such as `:`, `?`, `@` are rejected.

So the 400 in this case is **entirely** the malformed bulk payload (Q2), not the label value.

**Citations:**
- Jira label constraints (no spaces, <255 chars, no special chars, digits allowed): https://atly.io/blog/labels-jira/
- Atlassian tracker on label character handling: https://jira.atlassian.com/browse/JRASERVER-65019
- Atlassian community — label length/character limits: https://community.atlassian.com/forums/Jira-questions/What-is-the-limit-on-the-number-of-labels-that-can-be-stored-in/qaq-p/2607716

---

## Confidence

- **Q1 (PUT update bare-string syntax):** HIGH — corroborated by Atlassian JSON-editing docs + REST examples; matches the well-established `update`/`add`/`remove` pattern.
- **Q2 (bulk `labelsFields` schema):** HIGH — exact payload quoted directly from the official Atlassian developer "bulk-operation-additional-examples-and-faqs" page, cross-confirmed by two independent Perplexity searches.
- **Q3 (routing recommendation):** HIGH — follows directly from Q1/Q2 mechanics.
- **Q4 (all-digit labels legal):** MEDIUM-HIGH — confirmed across label-constraint sources; Atlassian has no single canonical "label grammar" doc page, so this rests on community + tracker sources rather than a formal spec, but the consensus is unambiguous.

Note: the live Atlassian OpenAPI/Swagger JSON and the rendered API-group pages are JS-hydrated and truncate under WebFetch; the bulk schema was therefore verified via the official Atlassian *prose* examples page (a first-party `developer.atlassian.com` source), not the raw swagger. This is first-party and definitive for the payload shape.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity perplexity_search | 4 | PUT update labels syntax; bulk editedFieldsInput schema; label value constraints; bulk labelsFields confirmation |
| Perplexity perplexity_reason | 1 | Attempt to verify exact OpenAPI property names (inconclusive — no schema in index; flagged) |
| WebFetch | 4 | Atlassian issues/bulk API-group pages (truncated); swagger JSON (truncated); **bulk-operation-examples FAQ page (SUCCESS — canonical payload)** |
| Training data | 1 area | General `update` vs `fields` verb semantics — cross-checked against cited Atlassian docs, not relied upon for property names or version specifics |

**Total MCP/web tool calls:** 9 (5 Perplexity + 4 WebFetch)
**Training data reliance:** low — every load-bearing claim (PUT syntax, bulk `labelsFields` schema, required keys, label legality) is backed by a cited first-party or community URL; the canonical bulk payload is a direct quote from an official `developer.atlassian.com` page.

---

## Multi-key bulk labels — exact schema (issue #446)

**Date:** 2026-05-31
**Researcher:** research agent (Perplexity-primary + first-party Atlassian docs)
**Status:** complete — HIGH confidence on the full payload; one resolved conflict on labels item-shape (see Q2).

### Exact payload to implement

For `jr issue edit K1 K2 --label add:foo --label remove:bar`, `jr` must `POST /rest/api/3/bulk/issues/fields` with:

```json
{
  "selectedActions": ["labels"],
  "selectedIssueIdsOrKeys": ["K1", "K2"],
  "editedFieldsInput": {
    "labelsFields": [
      {
        "fieldId": "labels",
        "bulkEditMultiSelectFieldOption": "ADD",
        "labels": [{ "name": "foo" }]
      },
      {
        "fieldId": "labels",
        "bulkEditMultiSelectFieldOption": "REMOVE",
        "labels": [{ "name": "bar" }]
      }
    ]
  },
  "sendBulkNotification": false
}
```

Add+remove in one call = **TWO `labelsFields` array elements** (one `ADD`, one `REMOVE`), each with its own `bulkEditMultiSelectFieldOption`. NOT one element with two actions.

---

### Q1 — Complete top-level request body

`POST /rest/api/3/bulk/issues/fields` top-level fields (camelCase exact):

| Field | Required? | Type | Notes |
|---|---|---|---|
| `selectedActions` | **REQUIRED** | `array<string>` | Allowlist of field groups being edited. For labels = `["labels"]`. Omitting it 400s — the endpoint gates every edit on a declared action. |
| `selectedIssueIdsOrKeys` | **REQUIRED** | `array<string>` | Issue IDs or keys (e.g. `["10001","K1"]`). This is the issue selector — confirmed name, NOT `issueIdsOrKeys`. |
| `editedFieldsInput` | **REQUIRED** | `object` | Container; holds per-type sub-arrays (`labelsFields`, etc.). |
| `sendBulkNotification` | optional | `boolean` | Notification flag. Defaults true server-side; set `false` to suppress. |

Full example editing labels on 2 issues: see "Exact payload to implement" block above (verbatim shape from the Atlassian bulk-ops FAQ example, transcribed twice via WebFetch).

**Citations:**
- Atlassian bulk-ops examples/FAQ (verbatim `selectedActions`/`selectedIssueIdsOrKeys`/`editedFieldsInput`/`labelsFields` payload): https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/
- Issue bulk operations API group: https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/

---

### Q2 — `labels` field object — exact shape (the critical part)

Inside `editedFieldsInput`, labels live under **`labelsFields`** — an **ARRAY**, NOT a single object, NOT `labels`. Each element:

- `fieldId`: `"labels"` (string, exact).
- `bulkEditMultiSelectFieldOption`: action enum, exact casing — **`ADD`** / **`REPLACE`** / **`REMOVE`** / **`REMOVE_ALL`** (all upper-case). NOT `labelsAction`.
- `labels`: array of **OBJECTS each with a `name` property** → `[{"name":"foo"}]`. **NOT bare strings** on the bulk endpoint.

**Add label foo AND remove label bar** = TWO separate `labelsFields` elements (one `ADD`, one `REMOVE`) — see exact payload block above.

**CONFLICT RESOLVED — labels item shape (`{"name":...}` object vs bare string):**
Prior research (Q2) said object form; one Perplexity query in this round claimed bare strings. The bare-string claim is **WRONG for the bulk endpoint** — it was inferred from the *platform* `PUT /rest/api/3/issue` field model (`labels: string[]`), and that query explicitly admitted it did not have the bulk FAQ page in its results. The authoritative source is the **first-party Atlassian bulk-ops FAQ**, whose own bulk-endpoint example shows `labels: [{"name":"Hello"}]` — transcribed verbatim twice. The bulk endpoint's `labelsFields[].labels` DTO is the **object form**. The bare-string form belongs ONLY to the platform `update.labels` path, not bulk. Confidence: HIGH (the only source that actually retrieved the bulk-endpoint example shows objects; the dissent generalized the wrong endpoint).

`bulkEditMultiSelectFieldOption` semantics (for labels): `ADD` appends; `REMOVE` removes listed; `REPLACE` overwrites the whole set with the listed labels; `REMOVE_ALL` clears all (label list may be empty/ignored). Only `ADD` is shown verbatim on the FAQ page ("there are other options as well"); the four-value enum is corroborated by prior research + Perplexity multi-select semantics — MEDIUM-HIGH on the three non-`ADD` literals (Atlassian doesn't enumerate them on a fetchable prose page; the API-group reference is JS-hydrated and truncates under WebFetch).

**Citations:** same as Q1 (FAQ page is the verbatim source).

---

### Q3 — `selectedActions` value for labels

`selectedActions` for a labels edit = **`["labels"]`** (the field-group identifier string, which equals the `fieldId` `"labels"`). It MUST be present and MUST include `"labels"` for any `labelsFields` edit; mismatch/omission → 400. Confirmed verbatim in the FAQ example (`"selectedActions": ["labels"]`). Confidence: HIGH.

**Citation:** https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/

---

### Q4 — Response + async model

`POST /rest/api/3/bulk/issues/fields` is **asynchronous** and returns a small JSON body containing a **`taskId`** (string, e.g. `"10001"`). Poll **`GET /rest/api/3/bulk/queue/{taskId}`** for status (running/complete/failed + processed/error counts). The existing jr poll model (taskId → poll queue) is **CORRECT** and unchanged. Confidence: HIGH — `taskId` shown verbatim in the FAQ's progress-response example; queue-poll pattern confirmed by Perplexity against the API-group docs.

**Citations:**
- FAQ progress response (`taskId`): https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/
- API group (queue endpoint): https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/

---

### Q5 — Permission + tier (E2E gating)

- **Global permission:** caller needs **"Make bulk changes"** (Cloud) / "Bulk change" (Server) global permission, PLUS normal project-level Edit-issue permission on the affected issues.
- **OAuth 2.0 granular scopes** (3LO/Forge): **`write:issue:jira`** + **`read:issue:jira`** — NOT a bulk-specific scope like `write:issue-bulk:jira`. Connect apps cannot access this resource.
- **Free tier:** "Make bulk changes" is **NOT configurable/available on Jira Cloud Free** — Free sites lock down global permissions. So a live multi-key label E2E test will fail (403/permission) on a Free service account.
  - **E2E guidance:** the multi-key bulk-label E2E test MUST **clean-skip** when the CI service account is on Free tier (or lacks "Make bulk changes"). Detect via a 403 on the first bulk POST and skip rather than fail, OR gate behind an explicit env var (e.g. `JR_E2E_BULK=1`) only set when the account is known-paid. Do NOT assume the E2E account can bulk-edit.

Confidence: HIGH on permission name + scopes (API-group docs); HIGH on Free-tier unavailability (Atlassian global-permission model + multiple community confirmations).

**Citations:**
- OAuth scopes (`write:issue:jira` / `read:issue:jira`) for bulk-ops group: https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/
- "Make bulk changes" global permission: https://confluence.atlassian.com/adminjiraserver/managing-global-permissions
- Bulk-change permission + Free-tier limits (Appfire/community): https://appfire.com/resources/blog/everything-to-know-about-jira-bulk-edits ; https://community.atlassian.com/forums/Jira-Service-Management/Where-can-you-edit-the-permissions-to-bulk-edit-issues/qaq-p/2110699

---

### Confidence summary (issue #446)

| Q | Confidence | Notes |
|---|---|---|
| Q1 top-level body | HIGH | All four field names verbatim from FAQ; `selectedActions` required-ness confirmed. |
| Q2 labelsFields + `{"name":...}` objects | HIGH | Object form is the bulk-endpoint DTO; bare-string dissent was wrong-endpoint inference. Enum: `ADD` verbatim; `REPLACE`/`REMOVE`/`REMOVE_ALL` MEDIUM-HIGH (not on a fetchable prose page). |
| Q2 add+remove = 2 elements | HIGH | Two `labelsFields` entries, one per action. |
| Q3 selectedActions=`["labels"]` | HIGH | Verbatim. |
| Q4 taskId async + queue poll | HIGH | `taskId` verbatim; existing poll model correct. |
| Q5 perms/scopes/Free-tier | HIGH | Scopes + "Make bulk changes" + Free-tier lockout all corroborated. |

**Unresolved ambiguity:** the exact string literals `REPLACE` / `REMOVE` / `REMOVE_ALL` are not individually quoted on any WebFetch-able prose page (only `ADD` is). They rest on prior research + Perplexity multi-select semantics, not a verbatim Atlassian enum dump (the API-group reference and swagger JSON both truncate/hydrate under WebFetch). Before relying on `REMOVE`/`REMOVE_ALL` in code, validate against a live API call or the rendered swagger UI. For the #446 fix (`add:`/`remove:`), only `ADD` and `REMOVE` are needed — `ADD` is verbatim-confirmed; `REMOVE` is the obvious paired literal and should be live-validated in the E2E or a manual smoke test.

### Research Methods (issue #446 addendum)

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity perplexity_reason | 1 | Full-schema attempt (no web context in that call — discarded) |
| Perplexity perplexity_search | 4 | top-level schema; async/taskId + perms/tier; add+remove element count; labels item-shape conflict; OAuth scopes |
| WebFetch | 4 | bulk-ops FAQ page x2 (SUCCESS — verbatim payload); API-group reference (truncated/hydrated); swagger JSON x2 (truncated past window) |
| Training data | 1 area | `update` vs bulk DTO distinction — used only to adjudicate the cross-endpoint conflict, cross-checked against cited FAQ |

**Total tool calls (this addendum):** 10 (5 Perplexity + 5 WebFetch/WebSearch; 1 WebSearch failed on classifier outage)
**Training data reliance:** low — every load-bearing field name + the `{"name":...}` object shape is from the verbatim first-party FAQ transcription; scopes/perms/taskId are first-party-cited.
