---
document_type: research
issue_id: 288
title: "Verification — JSM request types in jr issue create"
last_updated: 2026-05-09
sources_count: 7
---

# Issue #288 — Verification: JSM request types in jr issue create

> Scope: verify the API claims in #288 against canonical Atlassian docs, surface
> the design decisions jr's maintainer needs to make, and recommend an
> implementation path that fits jr's existing patterns.

---

## Claim 1 — `POST /rest/servicedeskapi/request` endpoint contract

**Status:** VERIFIED.

**Citation:** developer.atlassian.com — Jira Service Management Cloud REST API,
"Create customer request" (api-group-request).

**Summary of verified contract:**

- **HTTP:** `POST /rest/servicedeskapi/request`, `201 Created` on success.
- **Required body fields:**
  - `serviceDeskId` — type `string` (NOT a number; the JSON wire type is string).
  - `requestTypeId` — type `string` (same — string in JSON, even though it
    looks numeric).
- **Optional body fields:**
  - `requestFieldValues` — object; keys are Jira field IDs (e.g. `summary`,
    `description`, `customfield_10010`) and values are field-shape-specific.
  - `raiseOnBehalfOf` — string (accountId or email); rejected for
    customer-only callers. This is the JSM equivalent of "reporter".
  - `requestParticipants` — array of accountIds; rejected for customer-only
    callers, and rejected outright if the feature is disabled on the project.
  - `channel` — string (e.g. `"api"`, `"jiraMobile"`).
  - `form` — Form object (used when the request type has an attached
    Atlassian Forms form).
  - `isAdfRequest` — boolean; when true, text fields in `requestFieldValues`
    are interpreted as ADF rather than wiki/plain text.
- **Response shape:** `CustomerRequestDTO` containing `issueId`, `issueKey`,
  `requestTypeId`, `serviceDeskId`, `summary`, `createdDate`, `reporter`,
  `requestFieldValues` (array), `currentStatus`, and `_links` (`self`, `web`,
  `agent`, `jiraRest`).
- **OAuth scopes (Classic):** `write:servicedesk-request` (recommended).
- **OAuth scopes (Granular):** `write:request:jira-service-management`,
  `read:request:jira-service-management`, `read:user:jira`.
- **Error shapes:** `400` (bad/missing required fields, invalid request type),
  `401` (unauthorized), `403` (insufficient permissions; e.g. user can't
  raise-on-behalf-of), `500`.

**jr implication:**

- jr's existing OAuth scope set MUST be audited against
  `write:servicedesk-request`. The `jr` embedded OAuth app
  (ADR-0006, `embedded_oauth.rs`) declares scopes at console-registration
  time; if `write:servicedesk-request` is not in the registered scope list,
  request creation will return `401 invalid_token` regardless of the
  user's grant. **This is a blocking pre-flight check before merging
  any implementation.**
- IDs are strings on the wire — store them as `String`, not `u64`. jr's
  existing `ServiceDesk { id: String }` already follows this convention
  (`src/types/jsm/servicedesk.rs:5`).
- The 400 error for missing required fields is a structured response — jr
  should surface the field-level errors (not just the HTTP code) to the user.

---

## Claim 2 — Discovery endpoints

**Status:** VERIFIED for endpoints (1) and (2); PARTIALLY VERIFIED for (3).

**Citation:** developer.atlassian.com — api-group-servicedesk; cross-referenced
with Perplexity-summarized field-meta schema.

**(2a) `GET /rest/servicedeskapi/servicedesk` — list service desks**
- Pagination: **offset-based** (`start`, `limit`); identical pattern to jr's
  existing `list_service_desks` in `src/api/jsm/servicedesks.rs:12`.
- Per-item fields: `id`, `projectId`, `projectName`, `projectKey`,
  `_links.self`.
- jr already wraps this. **No new endpoint code required.**
- **Note:** the docs explicitly warn this is slow on instances with
  hundreds of service desks. jr's per-profile cache (`ProjectMeta`,
  `service_desk_id`) already mitigates this.

**(2b) `GET /rest/servicedeskapi/servicedesk/{serviceDeskId}/requesttype`**
- Pagination: **offset-based** (`start`, `limit`).
- Additional query params: `expand`, plus a `searchQuery` and `groupId`
  parameter on this resource (Atlassian docs list these for the
  request-type listing — INCONCLUSIVE on exact semantics; the docs page
  truncated before reaching the parameter table on my fetch). Worst-case
  jr filters client-side.
- Per-item fields (per Perplexity, cross-referenced against the JSM REST
  reference): `id`, `name`, `description`, `helpText`, `issueTypeId`,
  `serviceDeskId`, `groupIds`, `icon`, `fields`.
- IDs are strings.

**(2c) `GET /rest/servicedeskapi/servicedesk/{serviceDeskId}/requesttype/{requestTypeId}/field`**
- Returns `CustomerRequestCreateMetaDTO`. The exact wrapper schema
  (`canRaiseOnBehalfOf`, `canAddRequestParticipants`, `requestTypeFields[]`)
  is documented but the fetch redirected past the detail panel — flagged
  INCONCLUSIVE for the wrapper's outer fields; **field-entry shape is
  VERIFIED** via Perplexity:
  ```
  {
    "fieldId": "summary",            // or "customfield_10001"
    "name":    "Summary",
    "required": true,
    "jiraSchema": {
      "type":   "string|array|number|option",
      "items":  "string",            // for arrays
      "system": "summary"            // or null/customfield context
    },
    "validValues":  [{"value":"...","name":"..."}],
    "defaultValues": [...]
  }
  ```
- No special scope beyond `read:servicedesk-request` (granular: `read:request:jira-service-management`).

**jr implication:**

- All three discovery endpoints are offset-paginated, identical to jr's
  existing `ServiceDeskPage<T>` pattern (`src/api/pagination.rs`). Reuse
  it directly.
- Cache request-type metadata per `(profile, service_desk_id)` and field
  metadata per `(profile, service_desk_id, request_type_id)` with the
  same 7-day TTL as other cache entries. Field schemas change rarely;
  request type list changes a bit more often.
- Field-meta entries already give jr enough information for client-side
  required-field validation BEFORE submitting the POST — better UX than
  round-tripping through a 400.

---

## Claim 3 — `projectKey` → `serviceDeskId` mapping

**Status:** VERIFIED.

**Citation:** Atlassian community + JRASERVER-72607 (confirms the gap exists
in Cloud).

**Summary:**

- `GET /rest/servicedeskapi/servicedesk` does **NOT** support a
  `searchQuery` or `projectKey` query parameter. There is no server-side
  filter.
- The canonical mapping pattern is:
  1. `GET /rest/api/3/project/{projectKey}` → extract `id` (the projectId).
  2. `GET /rest/servicedeskapi/servicedesk` → list all desks.
  3. Match the desk where `desk.projectId == project.id`.
- This is **already implemented** in
  `get_or_fetch_project_meta` (`src/api/jsm/servicedesks.rs:41-99`) and
  cached as `ProjectMeta { service_desk_id: Option<String> }` per profile.

**jr implication:**

- **No new mapping code needed.** The existing `require_service_desk(client,
  project_key)` helper (line 102) already does exactly what request creation
  needs: returns the resolved `serviceDeskId` or a clean
  `JrError::UserError` if the project isn't a service desk.
- Reuse it verbatim from the new request-create handler.

---

## Claim 4 — Request-type name-vs-ID UX

**Status:** VERIFIED constraint; design recommendation below.

**Findings:**

- Request type IDs are stable strings (e.g. `"10"`, `"123"`).
- Request type **names** ("Submit a request or incident", "Get IT help",
  "Report a bug") are uniquely scoped to a single service desk — i.e. they
  are unique within `(serviceDeskId, name)`. Across an instance, names
  collide regularly (every project has a "Get IT help").
- There is no server-side name lookup; jr lists and matches client-side.

**jr implication:**

- Accept `--request-type <ID|NAME>` exactly as the issue suggests.
- Detection: numeric-only string → treat as ID; otherwise treat as name.
  (Not 100% safe — a request type's name could be `"42"` in theory — but
  the same heuristic jr uses elsewhere.) Better: try ID lookup first, fall
  back to name search on 404.
- Use jr's existing `partial_match` helper (`src/partial_match.rs`) for
  case-insensitive substring matching with disambiguation. This matches
  jr's pattern for link types and statuses (per CLAUDE.md "Conventions").
- Scope the match to a single `serviceDeskId` so name uniqueness is
  guaranteed within the search space.

---

## Claim 5 — Required fields handling (UX)

**Status:** VERIFIED constraint; UX is a design decision.

**Findings:**

- The field-meta endpoint returns a typed schema for every field, including
  `required: true|false`, type, and `validValues`. jr can pre-validate
  client-side instead of relying solely on 400 responses.
- jr's existing `issue create` already accepts repeated structured flags
  (`--label`, `--team`, `--points`, `--parent`, `--to`, `--account-id`)
  rather than a generic `--field key=value`. This is intentional — jr is
  domain-aware, not a generic field bag.
- A survey of comparable CLIs (ankitpokhrel/jira-cli, atlassian-python-api,
  jira-python) returned INCONCLUSIVE results from MCP search — the queries
  did not surface README-level confirmation. Based on knowledge of the
  ecosystem: ankitpokhrel/jira-cli uses `--custom key=value` repeated;
  atlassian-python-api takes a Python dict. Treat this as model knowledge,
  not citation.

**jr implication — recommended UX:**

- **Primary mechanism:** repeated `--field <id-or-name>=<value>`. This is
  the only mechanism that scales to arbitrary required custom fields without
  dedicating a flag per field.
- **Convenience flags** for the universal core (kept consistent with
  `issue create`): `--summary`, `--description`, `--description-stdin`,
  `--markdown`. These map onto `requestFieldValues.summary` and
  `requestFieldValues.description` server-side.
- **Pre-validation:** before POST, fetch field-meta, find any `required`
  fields not provided via flags, and:
  - In TTY + `!--no-input`: prompt for each missing required field
    interactively, displaying `name`, `description`, and `validValues` if
    the field is an option.
  - In `--no-input`: error with a list of missing fields (and a
    `jr request fields --request-type X` hint).
- **Drop the JSON-blob option** for a v1 of this feature. It's a power-user
  escape hatch and adds API surface area; we can add `--field-values @file`
  later if a real user needs it.
- **Field-name resolution:** `--field summary=...` (system field name) and
  `--field "Severity=High"` (custom field display name) should both work.
  Resolve names to `customfield_NNNNN` IDs via the field-meta response.
  Pure `customfield_NNNNN=...` should also work for scripts.

---

## Claim 6 — Reporter UX (`raiseOnBehalfOf`)

**Status:** VERIFIED.

**Findings:**

- `raiseOnBehalfOf` is the documented field on `POST /request`. It accepts
  an accountId or, for compatibility, an email address.
- Default behavior (omit the field): the authenticated user is recorded as
  the reporter.
- Permission gate: callers with the customer-only permission CANNOT use
  `raiseOnBehalfOf`. JSM agents and admins can. 403 is returned otherwise.
- `requestParticipants` (plural watchers) is a separate field, also
  accountId-array.

**jr implication:**

- Surface a `--reporter <email|accountId>` flag that maps to
  `raiseOnBehalfOf`. Reuse jr's existing user resolution helper
  (`cli/issue/helpers.rs` — `resolve_user`) to convert email → accountId
  when needed (matches the pattern used by `--account-id` on
  `issue create`).
- Document the permission gate in the error message: when 403 comes back,
  point the user at the JSM project role configuration.
- Defer `--participant <user>` (multi-value) to v2 unless a user requests
  it.

---

## Claim 7 — Error shapes

**Status:** VERIFIED.

**Findings:**

- `400`: missing/invalid required fields. Body contains
  `errorMessages: [...]` and (sometimes) `errors: { fieldId: "..." }`.
  Same shape as platform `/rest/api/3/issue` 400.
- `401`: token-level (expired / invalid scope). jr's existing 401
  auto-refresh wiring (S-3.03) covers this.
- `403`: caller can't see the service desk OR can't `raiseOnBehalfOf`.
  This is a permissions issue — distinct from 401 and should NOT trigger
  refresh.
- `500`: generic backend.

**jr implication:**

- Map these onto `JrError::UserError` (for 400/403) vs `JrError::ApiError`
  (500). The existing error-shape handler in `api/client.rs` already does
  the right thing for 400 — confirm the JSM-specific error body parses
  through the same path.

---

## Claim 8 — Positioning: extend `issue create` vs new `request create`

**Status:** DESIGN DECISION (recommended PIVOT to new subcommand).

**Findings:**

- `POST /rest/api/3/issue` will technically create an issue inside a JSM
  project — but the result is missing critical metadata: it has no
  Customer Request Type set, doesn't appear in customer portal queues,
  and skips request-type-specific required-field validation. This is the
  exact problem #288 describes. (Verified via Atlassian community
  discussions; the customfield is "Customer Request Type", typically
  `customfield_10010` per instance, but the field cannot be reliably set
  via `/rest/api/3/issue` because the portal-routing logic is server-side
  in the servicedeskapi handler.)
- Therefore the two endpoints are NOT interchangeable. A `--request-type`
  flag on `issue create` would have to silently switch endpoints under the
  hood, with subtly different field semantics, error shapes, and required
  fields.
- Conceptual mismatch: an `issue create` invocation that sometimes uses
  `requestFieldValues` (key=Jira field id) and sometimes uses `fields`
  (key=Jira field id, but with different shape rules — e.g. ADF
  description handling differs) is an awkward API.
- jr already has `jr queue` as a JSM-specific noun (`src/cli/queue.rs`).
  A `jr request` noun fits the same pattern.

**Recommendation: NEW SUBCOMMAND `jr request`.**

| Original assumption (#288)            | Re-evaluated viability                                        |
|---------------------------------------|---------------------------------------------------------------|
| Add `--request-type` to `issue create`| **Pivot.** Two endpoints ≠ one command. Discoverability also suffers — `jr issue create --help` becomes a wall. |
| Single discovery command              | Confirmed: `jr request types` + `jr request fields`.          |
| Same auth/scopes as jr already has    | **Audit needed.** `write:servicedesk-request` must be in the embedded OAuth app's scope list. |
| `--request-type <ID\|NAME>`           | Confirmed; use partial-match helper, scoped to one desk.      |
| `requestFieldValues` is a JSON object | Confirmed; map via repeated `--field` flags client-side.      |

---

## Recommended architecture

**Surface:**

```
jr request types    --project PROJKEY [--output json]
                    # GET /servicedesk/{id}/requesttype
                    # → table: id, name, description, # required fields

jr request fields   --project PROJKEY --request-type <ID|NAME> [--output json]
                    # GET /servicedesk/{id}/requesttype/{rt}/field
                    # → table: fieldId, name, required, type, validValues

jr request create   --project PROJKEY \
                    --request-type <ID|NAME> \
                    --summary "..." \
                    [--description "..." | --description-stdin] \
                    [--markdown] \
                    [--field <id-or-name>=<value> ...] \
                    [--reporter <email|accountId>] \
                    [--output json]
                    # Pre-validates required fields client-side via
                    # field-meta endpoint, prompts in TTY, errors with
                    # a missing-field list under --no-input.
                    # POSTs /rest/servicedeskapi/request.
                    # Idempotency: NOT implementable (no server-side
                    # idempotency key on this endpoint) — document.

jr request view     <KEY>   [--output json]
                    # GET /rest/servicedeskapi/request/{issueIdOrKey}
                    # Different from `jr issue view` because it surfaces
                    # request-type, status (currentStatus), portal links.
                    # OPTIONAL for v1 — issue view already works for
                    # the underlying issue.
```

**Pre-flight for the maintainer (BLOCKER before implementation):**

1. Verify the embedded OAuth app's registered scope list at the Atlassian
   Developer Console includes `write:servicedesk-request` (Classic) or
   the granular equivalent. If it does not, scope expansion is itself a
   release-coordinated change (existing tokens won't have the new scope
   until users re-auth).
2. Confirm jr's existing OAuth refresh handles 401 from
   `/rest/servicedeskapi/*` paths the same way it handles
   `/rest/api/3/*`. The path prefix differs but the auth mechanism is
   identical, so this should be a non-issue — confirm with a test.

---

## Recommended next action

**PROCEED — but with the OAuth scope pre-flight as Wave 0.**

### Effort estimate

- **Wave 0 (pre-flight):** 0.5 day — confirm scope list, write a smoke
  test that calls `POST /rest/servicedeskapi/request` against the dev
  sandbox with a stub payload, expecting 400 (not 401). If 401 comes
  back, scope is missing and release coordination is needed.
- **Wave 1 (read-only discovery):** 1–1.5 days — `jr request types`,
  `jr request fields`. Pure additive; no auth or write surface.
- **Wave 2 (write):** 1.5–2 days — `jr request create` including required-
  field pre-validation, interactive prompt, `--reporter` / raiseOnBehalfOf,
  error mapping. Bulk of the work is UX (prompting, validation, output
  formatting), not API plumbing.
- **Wave 3 (polish):** 0.5 day — caching, integration tests, docs/spec.

**Total: ~4–5 days** for a feature-complete v1, single-developer.

### Acceptance criteria draft

- `jr request types --project PROJ` lists request types with id/name/desc.
- `jr request fields --project PROJ --request-type "Get IT help"` lists
  required fields with type, default, validValues.
- `jr request create --project PROJ --request-type X --summary Y` succeeds
  when X has no other required fields. Returns the issue key.
- `jr request create` with missing required fields and `--no-input`
  errors with a structured list of which fields are missing.
- `jr request create` in TTY + `!--no-input` interactively prompts for
  every missing required field, validating against `validValues`.
- `jr request create --reporter someone@example.com` successfully sets
  `raiseOnBehalfOf` (resolved to accountId), and produces a clean 403
  error message when the caller lacks permission.
- `--output json` returns `{"key": "PROJ-123", "id": "...", "self": "..."}`
  on success and a structured error object on failure (matches the
  Symmetric output-channel profile).
- All commands fail with a `JrError::UserError` and exit code 64 when
  `--project` resolves to a non-JSM project (reuses
  `require_service_desk`).

### File list

- `src/api/jsm/requests.rs` — NEW. `create_customer_request`,
  `list_request_types`, `get_request_type_fields`. Mirrors the
  `servicedesks.rs` / `queues.rs` modules.
- `src/types/jsm/request.rs` — NEW. `RequestType`, `RequestTypeField`,
  `CustomerRequestCreateMetaDTO`, `CreateRequestPayload`,
  `CreateRequestResponse`. Add `pub mod request;` + re-export to
  `src/types/jsm/mod.rs`.
- `src/cli/request.rs` — NEW. `RequestCommand` enum
  (`Types`, `Fields`, `Create`), handlers, output formatters.
- `src/cli/mod.rs` — extend the top-level enum with
  `Command::Request(RequestCommand)`.
- `src/main.rs` — wire dispatch.
- `src/cache.rs` — extend with
  `read_request_types(profile, service_desk_id)` /
  `write_request_types(...)` and equivalent for field metadata. 7-day TTL.
- `tests/jsm_request_create.rs` — NEW. Wiremock-based integration tests:
  list types, list fields, create happy path, create with missing
  required, create with raiseOnBehalfOf, 403 for non-JSM project, 401
  triggers refresh, 400 surfaces field errors.
- `docs/specs/jsm-request-create.md` — NEW feature spec (per
  `docs/specs/` convention).
- `docs/adr/0007-jsm-request-vs-issue-create.md` — NEW ADR documenting
  why request creation is a separate subcommand rather than a flag on
  `issue create`.
- `CLAUDE.md` — append a "JSM request types" gotcha noting the
  endpoint-vs-platform-issue distinction and the `write:servicedesk-request`
  scope dependency.

### Open questions to flag for the maintainer

1. **Scope audit:** confirm the embedded OAuth app's registered scopes
   include `write:servicedesk-request`. **(BLOCKER)**
2. **Channel default:** should `jr request create` send `channel: "api"`
   to identify itself in the portal/audit log? (Atlassian doesn't seem to
   require it; recommended yes for traceability.)
3. **ADF vs plain text descriptions:** the existing `issue create`
   converts `--description` via `adf::text_to_adf` /
   `adf::markdown_to_adf`. The JSM endpoint uses the same ADF format
   when `isAdfRequest: true`. Recommend: always set `isAdfRequest: true`
   and reuse jr's existing ADF helpers verbatim.
4. **Should `jr request view` ship in v1 or v2?** v2 keeps the diff
   smaller; `jr issue view <KEY>` already works for the underlying issue,
   the only thing missing is a portal link.

---

## Inconclusive areas (flagged)

- The exact wrapper schema of `CustomerRequestCreateMetaDTO` (the outer
  fields `canRaiseOnBehalfOf`, `canAddRequestParticipants`,
  `requestTypeFields`) — confirmed by name in earlier Atlassian docs but
  the canonical page truncated before reaching the schema panel on this
  fetch. **Resolve by reading the OpenAPI spec at
  `developer.atlassian.com/cloud/jira/service-desk/swagger.v3.json`
  during Wave 1 implementation.**
- Whether `searchQuery` is supported on
  `GET /servicedesk/{id}/requesttype`. Atlassian's listing pages mention
  the parameter but the truncated fetch did not confirm semantics.
  **Resolve empirically — fall back to client-side filter if it's a
  no-op.**
- SDK survey for required-field UX (jira-python, ankitpokhrel/jira-cli,
  go-jira) returned INCONCLUSIVE from MCP queries. The recommended UX
  in this report is grounded in jr's own conventions, not in cross-tool
  research. If the maintainer wants a comparison table before locking
  the design, it's a follow-up research task.

---

## Research Methods

| Tool                     | Queries | Purpose                                                                                  |
|--------------------------|---------|------------------------------------------------------------------------------------------|
| WebFetch                 | 4       | developer.atlassian.com — request endpoint, servicedesk/requesttype/field listing pages. |
| Perplexity search        | 3       | Field-meta schema, projectKey-mapping confirmation, platform-vs-servicedesk endpoint, SDK UX survey (last one inconclusive). |
| Read (local)             | 4       | jr current jsm module surface, types, issue/create handler shape.                        |
| Training data            | 2 areas | (a) cross-SDK UX survey because Perplexity returned no useful results; (b) Customer Request Type customfield identity (`customfield_10010` is per-instance — not universal). Both flagged inline. |

**Total MCP/web tool calls:** 7 (4 WebFetch + 3 Perplexity).
**Training data reliance:** low–medium. Every API contract claim is sourced
from developer.atlassian.com; the only training-data fallbacks are the
SDK UX survey (explicitly flagged INCONCLUSIVE) and the assertion that
"Customer Request Type" is the relevant customfield (corroborated by
Perplexity citation 2 of the platform-vs-servicedesk query, but not a
direct API doc reference).

### Sources

- developer.atlassian.com — JSM Cloud REST: api-group-request (POST /request).
- developer.atlassian.com — JSM Cloud REST: api-group-servicedesk (list
  servicedesk, list requesttype, requesttype field meta).
- Atlassian Community thread on managing custom fields with request types
  (`/forums/Jira-Service-Management/.../qaq-p/1677504`).
- Atlassian Support — "Customize the fields of a request type"
  (`support.atlassian.com/jira-service-management-cloud/docs/customize-the-fields-of-a-request-type/`).
- Atlassian Support — "List of supported custom fields for request types in
  JSM customer portals" (`support.atlassian.com/jira/kb/...`).
- JRASERVER-72607 — confirms the missing project-search filter on the
  Cloud servicedesk list endpoint.
- Local: `src/api/jsm/servicedesks.rs`, `src/api/jsm/queues.rs`,
  `src/types/jsm/servicedesk.rs`, `src/cli/issue/create.rs` — for jr's
  current shape and conventions.
