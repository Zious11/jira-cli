---
context: bc-3
title: "Issue Write (create/edit/move/assign/comment/link/open/remote-link)"
total_bcs: 88   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 59   # count of `#### BC-` headings in this file
last_updated: 2026-05-18
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/bc-03-issue-write.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.3
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.1
  - F2 addition (2026-05-15): BC-3.4.009 — bulk-poll timeout task_id contract (issue #340)
  - F2 addition (2026-05-18): BC-3.8.001..010 — JSM request submission (issue #288 F2 added 001..009; F1d pass-01 added BC-3.8.010 to close --type interaction)
  - F1d addition (2026-05-18): BC-3.8.010 — --type ignored with warning when --request-type is set (issue #288 adversary pass-01)
---

# BC-3 — Issue Write

88 behavioral contracts across 8 subdomains: Assign (3.1), Move/Transition (3.2),
Create (3.3), Edit+Open (3.4), Comment (3.5), Links (3.6), Remote links (3.7),
JSM Request Create (3.8).

---

## Subdomains

### 3.1 Assign

#### BC-3.1.001: `issue assign --account-id <id>` PUTs `/issue/<key>/assignee` with `{accountId: <id>}`

**Confidence**: HIGH
**Source**: `tests/cli_handler.rs:58-91`; `tests/issue_commands.rs:1646-1703`
**Subject**: Issue write
**Behavior**: Body partial-JSON match `{accountId: "direct-id-001"}`. Output JSON: `{"changed": true, "key": "HDL-1", "assignee": "direct-id-001", "assignee_account_id": "direct-id-001"}`.
**Effects**: HTTP PUT to `/rest/api/3/issue/<key>/assignee`.
**Trace**: Pass 3 BC-201; BC-1077 (R4)

---

#### BC-3.1.002: `issue assign --to <name>` resolves via assignable user search then assigns

**Confidence**: HIGH
**Source**: `tests/cli_handler.rs:93-133`; `tests/issue_commands.rs:807-854`
**Subject**: Issue write
**Behavior**: GET `/rest/api/3/user/assignable/search?query=<name>&issueKey=<key>` → PUT with resolved accountId. Output `"assignee": "Jane Doe"`, `"changed": true`.
**Trace**: Pass 3 BC-202; BC-1059 (R4)

---

#### BC-3.1.003: `issue assign --to me` resolves current user via `/myself`

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:879-920`
**Subject**: Issue write
**Behavior**: `get_myself()` → `assign_issue(key, Some(&me.account_id))`. ZERO search HTTP.
**Trace**: Pass 3 BC-203; BC-1061 (R4)

---

#### BC-3.1.004: `issue assign` is idempotent — already-assigned-to-target → exit 0 + `"changed": false`

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:922-965`
**Subject**: Issue write
**Behavior**: `search_assignable_users` returns the user; `get_issue` shows already-assigned matching account_id; NO PUT mock mounted. Wiremock returns 404 for unmounted paths — test passes proving CLI short-circuits before PUT.
**Trace**: Pass 3 BC-204; BC-1062 (R4)

---

#### BC-3.1.005: `issue assign --unassign` PUTs `{accountId: null}`

**Confidence**: MEDIUM
**Source**: `src/cli/issue/workflow.rs::handle_assign`
**Trace**: Pass 3 BC-205

---

#### BC-3.1.006: `--to` ⊕ `--account-id` ⊕ `--unassign` clap conflict (mutually exclusive)

**Confidence**: HIGH
**Source**: `tests/cli_smoke.rs:170-211`
**Trace**: Pass 3 BC-206

---

#### BC-3.1.007: `search_assignable_users` returning empty Vec → `Ok(Vec::new())` (NOT Err); handler decides UX

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:856-877`
**Behavior**: Empty result is a caller-level UX error, not a client error.
**Trace**: Pass 3 BC-1060 (R4)

---

#### BC-3.1.008: `assign_issue("ERR-1", Some("bogus-id"))` against 404 → Err + `"does not exist"` message

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1705-1738`
**Behavior**: 404 body `{errorMessages: ["User '...' does not exist."]}` → `JrError::ApiError{status: 404, ..}`; extracted via `extract_error_message`.
**Trace**: Pass 3 BC-1078 (R4)

---

#### BC-3.1.009: `search_assignable_users_by_project(query, projectKey)` GETs `/rest/api/3/user/assignable/multiProjectSearch` (NOT `/user/search`)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1024-1082`
**Behavior**: Uses `projectKeys` AND `query` params. Accepts same FOUR response shapes as `search_users`.
**Trace**: Pass 3 BC-1064 (R4)

---

### 3.2 Move / Transition

#### BC-3.2.001: `issue move <key> <target>` is idempotent when current == target (by status name)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1500-1549`
**Subject**: Issue write
**Behavior**: `get_issue` shows current status == target → exit 0; stderr `"already in status"`; ZERO `POST /transitions` mock fires.
**Trace**: Pass 3 BC-207; BC-1074 (R4); Top-30 BC rank #12

---

#### BC-3.2.002: `issue move <key>` is idempotent via transition-name→status-name resolution too

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1551-1604`
**Subject**: Issue write
**Behavior**: Transition name `"Complete"` → destination status `"Completed"` → already there → short-circuit. stderr `"already in status"`.
**Trace**: Pass 3 BC-1075 (R4)

---

#### BC-3.2.003: `issue move` resolves transition by NAME match (e.g., `"Complete"`)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1219-1276`
**Behavior**: Fetches transitions, resolves `transition.name == "Complete"`, POSTs with `{transition: {id: "21"}}`. stderr: `"Moved FOO-1"`.
**Trace**: Pass 3 BC-1069 (R4)

---

#### BC-3.2.004: `issue move` resolves by STATUS NAME match (e.g., `transition.to.name == "Completed"`)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1278-1335`
**Behavior**: Status NAME match path (distinct from transition-name match). Same POST.
**Trace**: Pass 3 BC-1070 (R4)

---

#### BC-3.2.005: Duplicate candidates (same transition + status name) are de-duplicated; only ONE candidate presented

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1337-1394`
**Behavior**: `transition.name == "Done"` AND `transition.to.name == "Done"` → dedup → one candidate → succeeds.
**Trace**: Pass 3 BC-1071 (R4)

---

#### BC-3.2.006: Ambiguous move → exit non-zero + stderr `"Ambiguous"` + NO POST

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1396-1444`
**Trace**: Pass 3 BC-1072 (R4)

---

#### BC-3.2.007: No-match move → enriched candidate list in stderr: `"Complete (→ Completed)"` format

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1446-1498`
**Behavior**: Transition NAME → status NAME format in error candidates.
**Trace**: Pass 3 BC-1073 (R4)

---

#### BC-3.2.008: `--no-input` single-substring move → exit 64 + `"Ambiguous transition"` + ZERO POST

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1748-1810`
**Behavior**: `mock.expect(0)` on `POST /transitions`. stderr contains `"Ambiguous transition"` AND `"In Progress"`. Exit EXACTLY 64.
**Trace**: Pass 3 BC-1079 (R4)

---

#### BC-3.2.009: `issue move` 400 "resolution required" → `--resolution` hint + `jr issue resolutions` discovery pointer

**Confidence**: HIGH
**Source**: `tests/issue_resolution.rs:88-158`
**Behavior**: 400 body `{errors: {resolution: "Field 'resolution' is required"}}` → stderr contains `--resolution` AND `jr issue resolutions`.
**Trace**: Pass 3 BC-208, BC-209

---

#### BC-3.2.010: `issue resolutions` reads cache-first (7d TTL); JSON: `[{name, id, description}]`

**Confidence**: HIGH
**Source**: `tests/issue_resolution.rs:11-46, 49-86`
**Behavior**: GET `/rest/api/3/resolution`, cached 7 days. Table shows Name + Description. Resolutions without `id` dropped on cache write (+ stderr warning).
**Trace**: Pass 3 BC-210

---

#### BC-3.2.011: `transition_issue(key, id, Some(&fields))` body contains `{transition: {id}, fields: {resolution: {name: "Done"}}}`

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:79-103`
**Behavior**: Fields merged alongside transition in body. `expect(1)`.
**Trace**: Pass 3 BC-1039 (R4)

---

#### BC-3.2.012: `transition_issue(key, id, None)` body MUST NOT contain `"fields"` key

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:105-128`
**Behavior**: Negative-serialization pin. `body.contains("\"fields\"") == false`. Atlassian rejects `fields: null`.
**Trace**: Pass 3 BC-1040 (R4)

---

### 3.3 Create

#### BC-3.3.001: `issue create` POSTs `/rest/api/3/issue` returning `{"key": "FOO-123"}`

**Confidence**: HIGH
**Source**: `tests/issue_create_json.rs` (integration tests covering create body shape, field combinations, and JSON output)
**Subject**: Issue write
**Behavior**: Body includes summary, project, issuetype, optional priority, labels, description (ADF), team UUID, story points. Output JSON: `{"key": "FOO-123"}`.

> **[UPDATED 2026-05-18 issue #288]** The platform endpoint behavior described above applies ONLY when `--request-type` is absent. When `--request-type` is present, dispatch is to `POST /rest/servicedeskapi/request` instead (see BC-3.8.001). The core platform-create contract is otherwise unchanged — the existing platform path is unmodified in behavior.
> **Previous (pre-#288):** This BC stated unconditionally that `issue create` always POSTs to `/rest/api/3/issue`. After #288 that invariant becomes conditional: platform endpoint when `--request-type` absent; JSM endpoint when `--request-type` present.

**Trace**: Pass 3 BC-211

---

#### BC-3.3.002: `issue create` with assignee — uses `search_assignable_users_by_project` (multiProjectSearch)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1024-1082`
**Behavior**: Full body partial-match: `{project: {key}, issuetype: {name}, summary, assignee: {accountId}}`. Response 201 with `key: "FOO-99"`.
**Trace**: Pass 3 BC-1064 (R4)

---

#### BC-3.3.003: `issue create --to me` uses `get_myself()` (no search HTTP)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1084-1127`
**Trace**: Pass 3 BC-1065 (R4)

---

#### BC-3.3.004: `issue create` WITHOUT assignee — body has `{project, issuetype, summary}` ONLY (no assignee key)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1129-1154`
**Trace**: Pass 3 BC-1066 (R4)

---

#### BC-3.3.005: `issue create` assignee-not-found → stops short of create (NO POST mock)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1156-1180`
**Trace**: Pass 3 BC-1067 (R4)

---

#### BC-3.3.006: `issue create --account-id <id>` skips user search entirely

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1182-1217`
**Behavior**: Body has `assignee: {accountId: "direct-acct-789"}` directly.
**Trace**: Pass 3 BC-1068 (R4)

---

#### BC-3.3.007: `--to` and `--account-id` clap conflict on `issue create`

**Confidence**: HIGH
**Source**: `tests/cli_smoke.rs:215-235`
**Trace**: Pass 3 BC-224

---

#### BC-3.3.008: `issue create --markdown -d '...'` converts markdown to ADF before POST

**Confidence**: MEDIUM
**Source**: `tests/issue_create_json.rs`
**Trace**: Pass 3 BC-212

---

#### BC-3.3.009: `create_issue` browse URL uses `client.instance_url()` (NOT `client.base_url()`)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1606-1644`
**Behavior**: Integration test constructs URL via `client.instance_url()`. Cross-references BC-3.4.001 (NFR-R-B bug).
**Trace**: Pass 3 BC-1076 (R4)

---

### 3.4 Edit and Open

#### BC-3.4.001: `handle_open` MUST compose URL as `<instance_url>/browse/<key>` using `client.instance_url()` [MUST-FIX: NFR-R-B]

**Confidence**: HIGH
**Source**: `src/cli/issue/workflow.rs:636` (BUG SITE: currently uses `client.base_url()`)

> **MUST-FIX (HIGH — NFR-R-B):** Current code at line 636 uses `client.base_url()` which
> returns `api.atlassian.com/ex/jira/<cloudId>` for OAuth profiles — not a valid browse URL.
> This contract describes the FIXED behavior.

**Spec contract (fixed behavior):**
URL is composed as `format!("{}/browse/{}", client.instance_url(), key)`. `client.instance_url()` returns the real `*.atlassian.net` URL even for OAuth profiles. Fix is one line.

**Effects**: `issue open` and `issue open --url-only` produce correct browse URLs for OAuth users.
**Holdout:** H-046 — `jr issue open FOO-1` uses instance URL, not API gateway URL.
**Trace**: Pass 3 BC-220; NFR-R-B; BC-1010 (R4)

---

#### BC-3.4.002: `issue open --url-only` prints URL to stdout (no browser launch)

**Confidence**: MEDIUM
**Source**: Pass 2 §2b.1
**Trace**: Pass 3 BC-221

---

#### BC-3.4.003: `issue edit` PUTs `/rest/api/3/issue/<key>` with ADF description; accepts 204

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:609-645`
**Behavior**: Body partial-match pins full ADF doc shape: `{fields: {description: {version:1, type:"doc", content[0]: {type:"paragraph", ...}}}}`.
**Trace**: Pass 3 BC-1055 (R4)

---

#### BC-3.4.004: `issue edit` with `markdown_to_adf("**bold text**")` → ADF marks `[{type: "strong"}]` on wire

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:647-687`
**Trace**: Pass 3 BC-1056 (R4)

---

#### BC-3.4.005: `issue edit` with multiple fields sends both in body simultaneously

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:689-727`
**Trace**: Pass 3 BC-1057 (R4)

---

#### BC-3.4.006: `issue edit --label add:foo --label remove:bar` interprets prefix and emits correct JSON wire shape

**Confidence**: HIGH
**Source**: `tests/issue_bulk.rs`; `tests/issue_bulk_pr2.rs`; `src/cli/issue/create.rs::build_labels_edited_fields`; `src/cli/issue/create.rs` inline `#[cfg(test)] mod build_labels_proptests`
**Behavior**: `add:` and `remove:` prefixes adjust existing labels; bare label replaces.
The label JSON builder (`build_labels_edited_fields`) produces one of two wire shapes
depending on whether adds, removes, or both are present:

- **Object-form** (single-action — only ADD, or only REMOVE):
  ```json
  {"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}}
  ```
  or
  ```json
  {"labels": {"labelsAction": "REMOVE", "labels": [{"name": "bar"}]}}
  ```

- **Array-form** (both ADD and REMOVE present — coalesced into a single bulk POST):
  ```json
  {"labels": [
    {"labelsAction": "ADD",    "labels": [{"name": "foo"}]},
    {"labelsAction": "REMOVE", "labels": [{"name": "bar"}]}
  ]}
  ```

**Invariants**:
1. The ADD entry appears in the output if and only if `adds` is non-empty.
2. The REMOVE entry appears in the output if and only if `removes` is non-empty.
3. The caller bails on empty inputs — at least one of ADD or REMOVE is always present when `build_labels_edited_fields` is invoked.
4. When both ADD and REMOVE entries are present, array-form is used and the ADD entry precedes the REMOVE entry.
5. When exactly one action is present, object-form (not array-form) is used.

**Schema note**: This BC pins the wire shape as currently emitted by the code. Whether
the array-form (`[{labelsAction: "ADD", ...}, {labelsAction: "REMOVE", ...}]`) is what
Atlassian's bulk API formally accepts is a separate correctness concern tracked in issue
#331. Future schema-validation work (issue #331) may require updating this BC if the
canonical shape differs.

**Confidence rationale**: Confidence bumped MEDIUM → HIGH by issue #345 (S-345), which
extracts `build_labels_edited_fields` as a named pure function and adds an inline proptest
(`#[cfg(test)] mod build_labels_proptests` in `src/cli/issue/create.rs`) covering both shapes and all
five invariants. The proptest follows the pattern established by `src/jql.rs`,
`src/duration.rs`, and `src/partial_match.rs`.

**Trace**: Pass 3 BC-213; issue #345; S-345

---

#### BC-3.4.007: `--description` and `--description-stdin` clap conflict

**Confidence**: HIGH
**Source**: `tests/cli_smoke.rs:34-48`
**Trace**: Pass 3 BC-214

---

#### BC-3.4.008: `--points X` and `--no-points` clap conflict

**Confidence**: HIGH
**Source**: `tests/cli_smoke.rs:280-287`
**Trace**: Pass 3 BC-215

---

#### BC-3.4.009: outer-loop deadline check MUST include `task_id` literal in stderr message

**Confidence**: HIGH
**Source**: issue #340 + PR #360; `src/api/jira/bulk.rs:408-418` (`[deadline:bulk-outer]` site); `tests/bulk_deadline_propagation.rs`
**Subject**: Issue write (bulk edit path)
**Behavior**: When `await_bulk_task_inner`'s top-of-loop deadline check fires (i.e., the
bulk task remained non-terminal until the caller-supplied wall-clock deadline expired),
the `JrError::DeadlineExceeded` error message emitted to stderr MUST contain the literal
value of `task_id` AND the site tag `[deadline:bulk-outer]`. The message format is:
`"[deadline:bulk-outer] Bulk task <task_id> did not complete within <N>s timeout. Check Jira for task status."`
This allows the user to recover manually by inspecting the task directly at
`jr api /rest/api/3/bulk/queue/<task_id>`.

**Scope**: This contract applies exclusively to the outer-loop deadline site
(`[deadline:bulk-outer]` tag at `src/api/jira/bulk.rs:408-418`). It does NOT extend to
inner-loop deadline exits (`[deadline:429-retry]` in `JiraClient::send_inner`,
`src/api/client.rs:585-600`), because `task_id` is not in scope at those sites and
plumbing it through `send_inner` would require a non-trivial cross-module signature
change. Out-of-scope deferral noted; if a future enhancement adds `task_id` to the
client layer, a sibling BC SHOULD be created to cover that site.
**Effects**: Exit code 124 (`JrError::DeadlineExceeded`). Stderr contains the `task_id` value.
**Invariants**: The `task_id` value in the message MUST match the `taskId` returned by the
initial bulk POST response. It MUST pass `validate_task_id` before insertion (CWE-117
log-injection guard — audited in PR #355).
**VP Extension**: Extends `BC-bulk.poll.deadline-bounded` (issue-333 working label) —
adds the requirement that `task_id` appears in the stderr output in addition to the
existing wall-clock bound and `"deadline"` substring assertions.
**Trace**: issue #340 AC #1; `src/api/jira/bulk.rs::await_bulk_task_inner` (`[deadline:bulk-outer]` site)

---

### 3.5 Comments

#### BC-3.5.001: `issue comment <key> --internal` adds `sd.public.comment` property

**Confidence**: HIGH
**Source**: `src/api/jira/issues.rs::add_comment(internal: bool)`
**Behavior**: `properties: [{key:"sd.public.comment", value:{internal:true}}]`. Non-JSM: silently ignored.
**Trace**: Pass 3 BC-219

---

### 3.6 Links

#### BC-3.6.001: `issue link <k1> <k2> [--type T]` POSTs `/rest/api/3/issueLink`; default type "Relates"

**Confidence**: HIGH
**Source**: `src/api/jira/links.rs::tests`; `tests/issue_commands.rs:233-248`
**Trace**: Pass 3 BC-216; BC-1045 (R4)

---

#### BC-3.6.002: `issue link FOO-1 FOO-2 --type block` single-substring → exit 64 + `"Ambiguous link type"` + ZERO POST

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1812-1867`
**Trace**: Pass 3 BC-1080 (R4)

---

#### BC-3.6.003: `issue unlink FOO-1 FOO-2 --type block` single-substring → exit 64 + ZERO DELETE

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:1869-1920`
**Trace**: Pass 3 BC-1081 (R4)

---

#### BC-3.6.004: `client.delete_issue_link("10001")` DELETEs `/rest/api/3/issueLink/10001`; accepts 204

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:250-262`
**Trace**: Pass 3 BC-1046 (R4)

---

#### BC-3.6.005: `client.list_link_types()` returns 3 link types from `/rest/api/3/issueLinkType`

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:188-206`
**Trace**: Pass 3 BC-218; BC-1043 (R4)

---

### 3.7 Remote Links

#### BC-3.7.001: `issue remote-link <key> --url X` POSTs `/issue/<key>/remotelink`; URL gains trailing slash from `url::Url::parse` normalization

**Confidence**: HIGH
**Source**: `tests/issue_remote_link.rs:19-84`
**Behavior**: Body partial-JSON: `{object: {url: "https://example.com/", title: "Example"}}`. Trailing slash on URL. Output JSON: `{key, id, url, title, self}` (5 keys, normalized URL).
**Trace**: Pass 3 BC-222; BC-1126 (R4)

---

#### BC-3.7.002: `issue remote-link` defaults `--title` to URL when omitted

**Confidence**: HIGH
**Source**: `tests/issue_remote_link.rs:87-147`
**Trace**: Pass 3 BC-223; BC-1127 (R4)

---

#### BC-3.7.003: `issue remote-link --url not-a-url` → exit 64 + `"--url"` + `"not a valid url"`; ZERO HTTP

**Confidence**: HIGH
**Source**: `tests/issue_remote_link.rs:259-301`
**Behavior**: Pre-HTTP URL validation.
**Trace**: Pass 3 BC-1130 (R4)

---

#### BC-3.7.004: `issue remote-link --url ftp://example.com` → exit 64 + `"http or https"` + `"ftp"`

**Confidence**: HIGH
**Source**: `tests/issue_remote_link.rs:309-348`
**Behavior**: Scheme allowlist: only `http` and `https` accepted; all other schemes (e.g., `ftp`) rejected. Any URL whose scheme is not `http` or `https` triggers exit 64 with stderr containing `"http or https"` and the rejected scheme name.
**Trace**: Pass 3 BC-1131 (R4)

---

## BC-3.8: JSM Request Submission

10 behavioral contracts covering `jr issue create --request-type` dispatch to the JSM service desk API.
All BCs in this section require the `--request-type` flag to be set; the platform path (BC-3.3.001) is
entirely unchanged when `--request-type` is absent.

---

#### BC-3.8.001: `issue create --request-type <NAME|ID>` dispatches to `POST /rest/servicedeskapi/request`; platform path unchanged when flag absent

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: When `--request-type` is present, `handle_create` dispatches to `JiraClient::create_jsm_request` which POSTs to `/rest/servicedeskapi/request`. Body: `{serviceDeskId (string), requestTypeId (string), requestFieldValues (map), isAdfRequest (bool)}`. Response 201 includes `issueKey`. Output JSON (both table and `--output json`): `{"key": "<issueKey>"}` — identical shape to platform create. When `--request-type` is absent, the `POST /rest/servicedeskapi/request` endpoint is not called (validated by `expect(0)` mock pattern).
**Inputs**: `--request-type <NAME|ID>`, `--project <KEY>` (or active profile), `--summary <text>`
**Outputs/Effects**: HTTP POST to `/rest/servicedeskapi/request`; stdout `{"key": "HELP-42"}` on success; exit 0.
**Errors**: Non-JSM project (via `require_service_desk`) → exit 64 before any HTTP; see BC-3.8.002. 401 scope error → BC-3.8.009.
**Trace**: `tests/issue_create_jsm.rs` (integration tests — dispatch path, routing guard); `src/cli/issue/create.rs` (conditional dispatch branch)
**Source**: API-verified: `POST /rest/servicedeskapi/request` returns 201 with `{issueId, issueKey, currentStatus, _links}`
**Confidence**: HIGH

---

#### BC-3.8.002: JSM body uses `requestFieldValues` map; `serviceDeskId` resolved via `require_service_desk` from `--project`

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: Before POSTing, `handle_create` calls `require_service_desk(client, project_key)` to resolve the numeric `serviceDeskId` string. The JSM request body uses `requestFieldValues` (a `Map<String, serde_json::Value>`) for all field values, NOT the platform `fields` map. `serviceDeskId` is a required top-level field (string, NOT integer). If `--project` is absent and no active-profile project is configured, exits 64 with actionable message before any HTTP.
**Inputs**: `--project <KEY>` (or config active project); resolved `serviceDeskId`
**Outputs/Effects**: Body shape: `{serviceDeskId: "3", requestTypeId: "5", requestFieldValues: {...}}`. `serviceDeskId` is the string representation of the integer ID returned by the service desk list API.
**Errors**: Non-JSM project → `require_service_desk` returns `JrError::UserError`; exit 64; no HTTP to servicedeskapi. Error message MUST be call-site-specific: 'Project "<KEY>" is a <type> project. `--request-type` requires a Jira Service Management project. Run "jr project list" to find a JSM project.' (NOT the legacy "Queue commands require…" string from BC-X.8.004 — that string is reserved for queue commands only; see BC-3.8.002 and BC-X.8.004 [UPDATED 2026-05-18 issue #288].) No project configured → exit 64 with "project is required for JSM request creation" hint.
**Trace**: `tests/issue_create_jsm.rs` (service desk ID resolution, non-JSM project error path); `src/api/jsm/servicedesks.rs::require_service_desk`
**Source**: API-verified: `serviceDeskId` is a required string in request body
**Confidence**: HIGH

---

#### BC-3.8.003: `--request-type <NAME>` resolved via partial-match (case-insensitive); errors clean on Ambiguous, ExactMultiple, None with `jr requesttype list` hint

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: When `--request-type` is a non-numeric string, the handler fetches (or cache-hits) the service desk's request type list, then calls `partial_match(input, &names)`. `MatchResult::Exact(id)` → proceeds. `MatchResult::Ambiguous` or `MatchResult::ExactMultiple` → exits 64 with "Ambiguous request type" + candidate names + hint "Use `jr requesttype list --project <KEY>` to see all request types". `MatchResult::None` → exits 64 with "Request type not found" + hint. In `--no-input` mode, ambiguous partial match exits 64 cleanly (does NOT prompt).
**Inputs**: `--request-type <NAME>` (string, non-numeric); service desk request type list (API or cache)
**Outputs/Effects**: Resolved `requestTypeId` string passed into JSM request body.
**Errors**: Ambiguous → exit 64; None → exit 64; both with actionable hint. Zero HTTP to `POST /rest/servicedeskapi/request` on error paths.
**Trace**: `tests/issue_create_jsm.rs` (name-not-found path, ambiguous-match path); `src/partial_match.rs`; `src/cli/requesttype.rs`
**Source**: Follows `partial_match` pattern established by `jr issue move` and `jr queue`
**Confidence**: HIGH

---

#### BC-3.8.004: `--request-type <ID>` (numeric string) bypasses name resolution

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: When `--request-type` value is parseable as a non-negative integer (e.g., `"5"`, `"12"`), the value is used directly as `requestTypeId` without fetching or querying the request type list. No partial-match is performed. No cache read for this path. The numeric string is passed verbatim as `requestTypeId` in the JSM request body.
**Inputs**: `--request-type <ID>` where ID parses as `u64`
**Outputs/Effects**: Body includes `requestTypeId: "<numeric-string>"`; no GET to request type list endpoint.
**Errors**: If the API rejects the ID (e.g., 400 "invalid request type"), standard API error path applies (exit 1 + message).
**Trace**: `tests/issue_create_jsm.rs` (numeric-ID bypass path)
**Source**: Consistent with `jr queue view <ID>` numeric-bypass pattern
**Confidence**: HIGH

---

#### BC-3.8.005: `--summary` → `requestFieldValues.summary` (required by JSM API)

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: The `--summary` flag value is placed in `requestFieldValues["summary"]` as a JSON string. The JSM API requires `summary` in `requestFieldValues` (not as a top-level field). If `--summary` is absent and `--no-input` is set, exits 64 with "summary is required" — mirrors existing platform required-summary behavior. Interactive mode (TTY, no `--no-input`) may prompt for summary.
**Inputs**: `--summary <text>`
**Outputs/Effects**: `requestFieldValues["summary"] = "<text>"` in body.
**Errors**: Missing `--summary` + `--no-input` → exit 64 "summary is required for JSM request submission".
**Trace**: `tests/issue_create_jsm.rs` (summary field mapping); body shape assertions
**Source**: API-verified: `summary` is a required field in `requestFieldValues` for most request types
**Confidence**: HIGH

---

#### BC-3.8.006: `--description` → `requestFieldValues.description`; `--markdown` triggers ADF; plain text uses `text_to_adf` + `isAdfRequest: true`

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: When description is provided, `isAdfRequest: true` is always set in the request body (both plain-text and markdown paths use ADF). Plain-text description (`--description "text"` without `--markdown`) is converted via `text_to_adf("text")` and placed in `requestFieldValues["description"]`. Markdown description (`--description "**bold**" --markdown`) is converted via `markdown_to_adf("**bold**")` and placed in `requestFieldValues["description"]`. When description is absent, `requestFieldValues["description"]` is omitted (NOT null) and `isAdfRequest` may be omitted or set to false. The ADF utilities are the same `src/adf.rs` functions used by the platform create path.
**Inputs**: `--description <text>` (optional), `--markdown` (flag)
**Outputs/Effects**: `requestFieldValues["description"] = <ADF-doc-object>` when description present; `isAdfRequest: true` in body when description present.
**Errors**: `--description` and `--description-stdin` clap conflict (inherits from platform create).
**Trace**: `tests/issue_create_jsm.rs` (description ADF conversion); `src/adf.rs` unit tests
**Source**: API-verified: `isAdfRequest: true` enables ADF for rich-text fields
**Confidence**: HIGH

---

#### BC-3.8.007: `--priority <NAME>`, `--label <X>` (repeatable) → `requestFieldValues.priority` / `requestFieldValues.labels`

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: `--priority <NAME>` maps to `requestFieldValues["priority"] = {"name": "<NAME>"}` (same object shape as platform priority; consistent with existing `jr issue create` platform behavior). `--label <X>` (repeatable) maps to `requestFieldValues["labels"] = ["<X1>", "<X2>", ...]` as a JSON array of plain strings — NOT `[{"name": "foo"}]`. These are system-field name mappings (using the field's logical name, not `customfield_NNNNN`). If the request type does not include these fields, the JSM API ignores or rejects them; no client-side validation of which fields are valid for a given request type is performed (validation is server-side).
**Inputs**: `--priority <NAME>` (optional), `--label <X>` (optional, repeatable)
**Outputs/Effects**: Corresponding entries in `requestFieldValues` map when flags are set.
**Errors**: Unsupported field for request type → API 400; handled as standard API error (exit 1 + message).
**Trace**: `tests/issue_create_jsm.rs` (priority and label mapping); body shape assertions
**Source**: Atlassian docs confirm `labels` wire shape is a plain string array `["alpha","beta"]` for both `POST /rest/api/3/issue` and `POST /rest/servicedeskapi/request` `requestFieldValues` (https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-labels/). Priority wire shape `{"name": "<NAME>"}` is consistent with current `jr` platform-create code. Caveat: JSDSERVER-4564 documents that JSM may silently ignore `requestFieldValues.priority` if the request type schema does not include priority — implementation MUST NOT assume the field surfaces in the response.
**Confidence**: HIGH

---

#### BC-3.8.008: `--field NAME=VALUE` (repeatable) maps NAME → `requestFieldValues`; `customfield_NNNNN` literal bypasses lookup; only first `=` splits key; empty value allowed; duplicate NAME → last wins

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: Each `--field NAME=VALUE` pair is parsed by splitting on the FIRST `=` only (value may contain `=`). The resulting `(name, value)` is inserted into `requestFieldValues` with `name` as the JSON key and `value` as a JSON string. If `NAME` begins with `customfield_` followed by digits (e.g., `customfield_10200`), it is used as-is as the key (no lookup). Otherwise, `NAME` is used as-is as the key (logical field name). Empty value (`--field "fieldname="`) is valid and inserts an empty string. Duplicate `NAME` entries → last occurrence wins (map semantics). `--field` entries are merged with `--summary`, `--description`, `--priority`, `--label` entries in `requestFieldValues`; `--field summary=X` overrides `--summary X` (last-wins on the map key).
**Inputs**: `--field NAME=VALUE` (optional, repeatable)
**Outputs/Effects**: Each pair inserted into `requestFieldValues`; merged with other field sources.
**Errors**: Missing `=` in `--field` value → exit 64 "invalid field format: expected NAME=VALUE".
**Trace**: `tests/issue_create_jsm.rs` (field mapping, first-equals split, duplicate-key, empty-value); body shape assertions
**Source**: Consistent with `--field` conventions; split-on-first-equals is standard CLI convention
**Confidence**: HIGH

---

#### BC-3.8.009: `--on-behalf-of <accountId>` → `raiseOnBehalfOf`; value passed through as-is; invalid accountIds rejected server-side

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: When `--on-behalf-of <accountId>` is set, the value is placed as `raiseOnBehalfOf: "<accountId>"` in the JSM request body top level (NOT inside `requestFieldValues`). When absent, `raiseOnBehalfOf` is omitted from the body entirely (NOT null). `--on-behalf-of` accepts the raw value as-is and passes through to JSM API as `raiseOnBehalfOf` field. No client-side regex format validation is performed — this matches `--account-id` pass-through behavior (see BC-3.1.001); client-side format validation would false-negative legacy accountIds (Atlassian accountIds are not documented as a fixed format; migrated accountIds may use colon-separated forms like `557058:abc...`). Invalid accountIds are rejected server-side by JSM with a 400 — surface that error with a hint to use `jr user search <query>` to look up accountIds. No email-to-accountId lookup is performed (consistent with `--account-id` convention elsewhere in `jr`).
**Inputs**: `--on-behalf-of <accountId>` (optional)
**Outputs/Effects**: `raiseOnBehalfOf: "<accountId>"` in body when set; omitted when absent.
**Errors**: JSM 400 on invalid accountId → exit 1 with API error message + hint "Use `jr user search <query>` to look up accountIds". Scope error for `write:servicedesk-request` → BC-X.3.005 (InsufficientScope dispatch) + BC-1.6.042 (401 substring match) + H-NEW-JSM-RT-003 (regression pin).
**Trace**: `tests/issue_create_jsm.rs` (raiseOnBehalfOf injection, absence omission); `src/cli/issue/create.rs`
**Source**: BC-3.1.001 (`issue assign --account-id` pass-through precedent); BC-X.3.005 (server-rejected accountId error path). Pass-through behavior is the documented Atlassian recommendation; client-side format validation would false-negative legacy accountIds.
**Confidence**: HIGH

---

#### BC-3.8.010: `--type` is IGNORED with stderr warning when `--request-type` is set

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: When `--request-type` is present, the `--type` flag (if also supplied) is silently ignored at the JSM-dispatch site EXCEPT for emitting a single stderr line: "warning: --type is ignored when --request-type is set; request type encodes the issue type". Exit code unchanged (still 0 on success, or 64/1/2 on applicable error paths). JSON output shape is unchanged from BC-3.8.001. The warning is emitted in the success path before the POST is issued (after flag parsing and request-type resolution succeed). If the command early-exits on `--summary`-missing (BC-3.8.005) or partial-match failure (BC-3.8.003), the warning need not fire — the user's `--type` was not silently consumed because the command did not proceed. On the success path, the warning fires regardless of `--no-input` or `--output json` settings.
**Inputs**: `--request-type <X>` AND `--type <Y>` (both set simultaneously)
**Outputs/Effects**: Same JSM POST behavior as BC-3.8.001 with the `--type` value unused. One stderr line emitted: "warning: --type is ignored when --request-type is set; request type encodes the issue type". No change to stdout JSON shape. No change to exit code.
**Errors**: None — this is a warning path, not an error path. The presence of `--type` alongside `--request-type` is not an error.
**Trace**: `tests/issue_create_jsm.rs` (warning_on_type_with_request_type integration test)
**Source**: ADR-0014 §"Dispatch fork: --type interaction" — `--type` is meaningless in the JSM path because `requestTypeId` encodes the issue type server-side; emitting a warning rather than erroring preserves backward compatibility for scripts that habitually pass `--type`.
**Confidence**: HIGH

---

## JSON Output Shape Contracts (all confirmed by insta snapshots)

| Operation | JSON shape | Key field note |
|-----------|-----------|---------------|
| `move` (changed) | `{"changed": true, "key": "TEST-1", "status": "In Progress"}` | 3 keys alphabetical |
| `move` (unchanged) | `{"changed": false, "key": "TEST-1", "status": "Done"}` | idempotent form |
| `assign` (changed) | `{"assignee": "Jane Doe", "assignee_account_id": "abc123", "changed": true, "key": "TEST-1"}` | `assignee_account_id` snake_case |
| `assign` (unchanged) | identical with `changed: false` | |
| `unassign` | `{"assignee": null, "changed": true, "key": "TEST-1"}` | `assignee` is EXPLICIT null |
| `edit` | `{"key": "TEST-1", "updated": true}` | 2 keys |
| `link` | `{"key1": "TEST-1", "key2": "TEST-2", "linked": true, "type": "Blocks"}` | symmetric key1/key2 |
| `unlink` | `{"count": 2, "unlinked": true}` | `count: 0` when no match |
| `remote-link` | `{"id": 10000, "key": "TEST-1", "self": <url>, "title": <title>, "url": <url>}` | 5 keys |
| `create` | `{"key": "FOO-123"}` | minimal |

Sources: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__*.snap`; BC-1104..BC-1112 (R4)

## Total BCs in this file: 59 individually-bodied (cumulative 88 incl. range-collapsed; see BC-INDEX.md)

_Last updated 2026-05-18: +10 BCs total (BC-3.8.001..010, issue #288): 9 added in F2 delta; 1 added in F1d adversary pass-01 to close the `--type` interaction risk. BC-3.3.001 modified to add conditional routing clause. BC-3.8.002 Errors updated (call-site-specific message); BC-3.8.007 Confidence HIGH + labels wire shape hardened + priority JSDSERVER-4564 caveat; BC-3.8.009 regex removed (pass-through behavior); BC-3.8 section header range updated to 001..010._
