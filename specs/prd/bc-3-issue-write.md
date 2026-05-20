---
context: bc-3
title: "Issue Write (create/edit/move/assign/comment/link/open/remote-link)"
total_bcs: 93   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 64   # count of `#### BC-` headings in this file
last_updated: 2026-05-19
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/bc-03-issue-write.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.3
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.1
  - F2 addition (2026-05-15): BC-3.4.009 — bulk-poll timeout task_id contract (issue #340)
  - F2 addition (2026-05-18): BC-3.8.001..010 — JSM request submission (issue #288 F2 added 001..009; F1d pass-01 added BC-3.8.010 to close --type interaction)
  - F1d addition (2026-05-18): BC-3.8.010 — --type ignored with warning when --request-type is set (issue #288 adversary pass-01)
  - F1d addition (2026-05-19): BC-3.8.011 — platform-only flags emit stderr warnings on JSM path (issue #288 adversary-pass-01 C-02); H-01 BC-3.8.003 verb aligned "Use"→"Run"
  - F2 addition (2026-05-19): BC-3.8.012..013 — inverse warning symmetry: --field and --on-behalf-of silent-drop on platform path (issue #383)
  - F2 addition (2026-05-19): BC-3.8.014..015 — JSM 401 auth-conditional hints on handle_jsm_create: Basic-auth (is_oauth_auth==false) → API-token hint with InsufficientScope rewrite; OAuth (is_oauth_auth==true) → existing behavior preserved (issue #384; corrected model: gate is is_oauth_auth() alone)
---

# BC-3 — Issue Write

93 behavioral contracts across 8 subdomains: Assign (3.1), Move/Transition (3.2),
Create (3.3), Edit+Open (3.4), Comment (3.5), Links (3.6), Remote links (3.7),
JSM Request Create + Platform-Path Inverse Warnings + Auth-Conditional 401 Hints (3.8).

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

> **[UPDATED 2026-05-18 issue #288; amended 2026-05-19 issue #383]** The platform endpoint behavior described above applies ONLY when `--request-type` is absent. When `--request-type` is present, dispatch is to `POST /rest/servicedeskapi/request` instead (see BC-3.8.001). The POST body, JSON response, and exit code on the platform path are unchanged by these additions; however, when `--field` or `--on-behalf-of` are supplied without `--request-type`, the platform path now emits stderr warnings (see BC-3.8.012, BC-3.8.013) — so the platform path is not fully unmodified in observable behavior post-#383.
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

### 3.8 JSM Request Create + Platform-Path Inverse Warnings + Auth-Conditional 401 Hints

15 behavioral contracts covering: (a) `jr issue create --request-type` dispatch to the JSM service desk API
(BC-3.8.001..009), (b) forward-direction cross-flag warnings when platform-only flags are passed alongside
`--request-type` (BC-3.8.010..011), (c) inverse-direction cross-flag warnings when JSM-only flags are
passed on the platform path (BC-3.8.012..013), and (d) auth-conditional 401 error hints on the JSM POST
path: Basic-auth API-token-expiry hint (BC-3.8.014) and OAuth write-scope hint (BC-3.8.015), gated solely
by `JiraClient::is_oauth_auth()`.
BCs 001..011 require `--request-type` to be set. The platform path (BC-3.3.001) — its POST body,
JSON response, and exit code — is unchanged when `--request-type` is absent. BCs 012..013 add
inverse-direction stderr warnings on the platform path (when `--field` / `--on-behalf-of` are
passed without `--request-type`) without altering POST behavior, response, or exit code.

---

#### BC-3.8.001: `issue create --request-type <NAME|ID>` dispatches to `POST /rest/servicedeskapi/request`; platform POST body, JSON response, and exit code unchanged when `--request-type` absent

**Confidence**: HIGH
**Subject**: Issue write (JSM path)
**Behavior**: When `--request-type` is present, `handle_create` dispatches to `JiraClient::create_jsm_request` which POSTs to `/rest/servicedeskapi/request`. Body: `{serviceDeskId (string), requestTypeId (string), requestFieldValues (map), isAdfRequest (bool)}`. Response 201 includes `issueKey`. Output JSON (both table and `--output json`): `{"key": "<issueKey>"}` — identical shape to platform create. When `--request-type` is absent, the `POST /rest/servicedeskapi/request` endpoint is not called (validated by `expect(0)` mock pattern).
**Inputs**: `--request-type <NAME|ID>`, `--project <KEY>` (or active profile), `--summary <text>`
**Outputs/Effects**: HTTP POST to `/rest/servicedeskapi/request`; stdout `{"key": "HELP-42"}` on success; exit 0.
**Errors**: Non-JSM project (via `require_service_desk`) → exit 64 before any HTTP; see BC-3.8.002. 401 → BC-3.8.009 (auth-conditional: Basic-auth API-token hint → BC-3.8.014; OAuth → BC-3.8.015).
**Trace**: `tests/issue_create_jsm.rs` (integration tests — dispatch path, routing guard); `src/cli/issue/create.rs` (conditional dispatch branch)
**Source**: API-verified: `POST /rest/servicedeskapi/request` returns 201 with `{issueId, issueKey, currentStatus, _links}`
**Confidence**: HIGH

> **[UPDATED 2026-05-19 issue #384]** Errors cross-reference updated: 401 on the JSM POST is auth-conditional; see BC-3.8.009 (auth-conditional gate), which cross-references BC-3.8.014 (Basic-auth: API-token-expiry hint) and BC-3.8.015 (OAuth: existing write-scope hint behavior). No behavioral change — cross-reference refresh only.

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
**Behavior**: When `--request-type` is a non-numeric string, the handler fetches (or cache-hits) the service desk's request type list, then calls `partial_match(input, &names)`. `MatchResult::Exact(id)` → proceeds. `MatchResult::Ambiguous` or `MatchResult::ExactMultiple` → exits 64 with "Ambiguous request type" + candidate names + hint "Run `jr requesttype list --project <KEY>` to see all request types". `MatchResult::None` → exits 64 with "Request type not found" + hint. In `--no-input` mode, ambiguous partial match exits 64 cleanly (does NOT prompt).

[UPDATED 2026-05-19 issue #288 pr4 adversary-pass-01 H-01] Hint verb aligned from
"Use" to "Run" to match Wave 2 cli/requesttype.rs sibling (line 227) and the
Wave 3 cli/issue/create.rs dispatch fork (lines 2005, 2017). Imperative active
verb fits jr CLI ergonomics. Wave 2 pass-02 M-2 precedent applied.
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
**Errors**: JSM 400 on invalid accountId → exit 1 with API error message + hint "Use `jr user search <query>` to look up accountIds". 401 on the JSM POST is auth-conditional — see BC-3.8.014 (Basic-auth: API-token-expiry hint) and BC-3.8.015 (OAuth: `write:servicedesk-request` hint). See also BC-X.3.005 (InsufficientScope dispatch) + BC-1.6.042 (401 substring match) + H-NEW-JSM-RT-003 (OAuth scope-mismatch regression pin).
**Trace**: `tests/issue_create_jsm.rs` (raiseOnBehalfOf injection, absence omission); `src/cli/issue/create.rs`
**Source**: BC-3.1.001 (`issue assign --account-id` pass-through precedent); BC-X.3.005 (server-rejected accountId error path). Pass-through behavior is the documented Atlassian recommendation; client-side format validation would false-negative legacy accountIds.
**Confidence**: HIGH

> **[UPDATED 2026-05-19 issue #384]** Errors section revised: the monolithic "Scope error for `write:servicedesk-request`" wording replaced with auth-conditional phrasing. The gate is `client.is_oauth_auth()` alone — not error variant. Basic-auth 401s (any body shape, including "scope does not match") route to BC-3.8.014 (API-token-expiry hint; any `InsufficientScope` is rewritten to `NotAuthenticated`). OAuth 401s route to BC-3.8.015 (existing behavior, now explicitly gated: for OAuth, BOTH the `InsufficientScope` arm AND the `NotAuthenticated` arm produce the `write:servicedesk-request` hint — the pre-#384 `map_err` at `src/cli/issue/create.rs:1988-1995` already rewrites `NotAuthenticated` to inject this hint for all auth schemes). The prior single-hint behavior is superseded by the auth-gate introduced in BC-3.8.014/015.
>
> **[REVISED 2026-05-19 issue #384 adversary-pass-2 H-05/H-06]** Corrected false claim: previous text stated OAuth `NotAuthenticated` gives "generic `jr auth login` hint" — this is FALSE. The existing pre-#384 `map_err` (`src/cli/issue/create.rs:1988-1995`) already rewrites the `NotAuthenticated` arm to inject `write:servicedesk-request` for all auth schemes. Post-#384, that rewrite is preserved unchanged for OAuth. Both arms produce `write:servicedesk-request` for OAuth.

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

#### BC-3.8.011: Platform-only flags ignored on JSM path emit stderr warnings

**Confidence**: HIGH
**Subject**: JSM request submission cross-flag interaction
**Behavior**: When `--request-type <NAME|ID>` is set on `jr issue create`, the following
platform-only flags are NOT supported by the JSM `/rest/servicedeskapi/request` endpoint
and are silently ignored if passed. For EACH such flag set, the handler MUST emit ONE
warning line to stderr (NOT stdout, NOT in --output json data), then continue with the
JSM dispatch normally. Flags covered:

- `--team <id>`: warning `"warning: --team is ignored when --request-type is set; teams are managed by the request type's workflow"`
- `--points <n>`: warning `"warning: --points is ignored when --request-type is set; story points are not part of JSM request schema"`
- `--parent <key>`: warning `"warning: --parent is ignored when --request-type is set; JSM requests cannot be sub-tasks"`
- `--to <accountId>`: warning `"warning: --to is ignored when --request-type is set; use --on-behalf-of to set the requester"`
- `--account-id <id>`: warning `"warning: --account-id is ignored when --request-type is set; use --on-behalf-of to set the requester"`

Generalizes the existing `--type` warning pattern from BC-3.8.010. Idempotent — passing
the same flag twice still emits ONE warning per logical flag.

**Inputs**: any combination of `--team`, `--points`, `--parent`, `--to`, `--account-id`
with `--request-type`
**Outputs/Effects**: One stderr warning line per dropped flag; JSM dispatch continues
normally; exit 0 on success.
**Errors**: None — these are warnings, not errors. Dispatch proceeds.
**Trace**: `tests/issue_create_jsm.rs` (per-flag warning-emission integration tests, one assertion per platform-only flag)
**Source**: Adversary pass-01 C-02 codification; mirrors BC-3.8.010 pattern
**Confidence**: HIGH

[NEW 2026-05-19 issue #288 pr4 adversary-pass-01 C-02] Added to codify the cross-flag
warning policy after adversary pass-01 found silent-drop of 5 platform-only flags on
the JSM dispatch path.

---

#### BC-3.8.012: `--field` on platform path emits stderr warning (idempotent per flag NAME)

**Confidence**: HIGH
**Subject**: Issue write (platform path cross-flag interaction)
**Behavior**: When `jr issue create` is invoked WITHOUT `--request-type` but WITH one or
more `--field NAME=VALUE` flags, the handler MUST emit ONE warning line to stderr
(NOT stdout, NOT in `--output json` data) BEFORE the platform POST is issued. The
warning fires ONCE per logical flag NAME — mirroring BC-3.8.011's idempotent semantic.
Passing `--field` multiple times (e.g., `--field A=1 --field A=2`, or
`--field A=1 --field B=2`) emits exactly one warning total; the warning is per-flag-NAME
(`--field`), not per-NAME-VALUE pair. The platform path then runs to completion as if
`--field` was not supplied. Exit code is unchanged (0 on success). Stdout output
(e.g., `{"key": "FOO-123"}`) is unchanged.

Verbatim warning string (emitted once, regardless of how many `--field` occurrences):
`"warning: --field is ignored on the platform create path; it only applies with --request-type (JSM service-desk requests). To pass custom fields to a JSM request type, also supply --request-type."`

Inverse symmetry to BC-3.8.008: `--field` is accepted and meaningful on the JSM path
(maps to `requestFieldValues`); on the platform path it has no effect and MUST warn.
The warning fires regardless of `--no-input` or `--output json` settings. If the command
early-exits before the POST (e.g., missing required field), the warning need not fire.

When `--field` is absent (clap default: empty Vec), NO warning is emitted — i.e., the
stderr stream from a plain platform-path invocation is byte-identical to pre-issue-#383
behavior.

Platform path does NOT parse `--field NAME=VALUE` strings (only detects presence of
the flag). A malformed `--field` (e.g., `--field bare-name-no-equals`) on the platform
path still triggers the one warning and is then discarded; no exit-64 error fires.
Format validation per BC-3.8.008 applies only on the JSM path.

Cross-reference: BC-3.8.012 and BC-3.8.013 fire independently when both `--field` and
`--on-behalf-of` are present without `--request-type`; both warnings appear on stderr
(each collapsed per its own idempotency rule).

**Inputs**: `--field NAME=VALUE` (one or more) WITHOUT `--request-type`
**Outputs/Effects**: ONE stderr warning line (regardless of how many `--field` flags);
platform POST proceeds normally with the `--field` values discarded; stdout and exit
code unchanged.
**Errors**: None — this is a warning path, not an error path.
**Trace**: `tests/issue_create_jsm.rs` (integration tests covering platform-path inverse-warning
for `--field`). Test placement: current Trace cites the existing JSM test file; F3
story-writer may choose to (a) keep tests in `issue_create_jsm.rs` (extending the file's
scope) or (b) split into a new test file for cleaner perimeter. That decision is deferred
to F3.
**Source**: Issue #383 F1 delta analysis; structurally mirrors BC-3.8.010 (Inputs/Outputs
sub-fields), semantically mirrors BC-3.8.011 (warn-and-continue pattern, idempotent per
logical flag NAME); inverse symmetry to BC-3.8.008 / BC-3.8.009. Note: wording expanded
from F1 proposal (`"warning: --field is ignored without --request-type; use --request-type
to submit a JSM request with custom fields"`) to clarify the "platform create path" vs JSM
dispatch distinction explicitly, per F2 review.
**Confidence**: HIGH

[NEW 2026-05-19 issue #383 F2] Added to close the platform-path inverse-warning symmetry
gap identified in F1 delta analysis: `--field` is silently dropped on platform path with
no user feedback.

---

#### BC-3.8.013: `--on-behalf-of` on platform path emits stderr warning

**Confidence**: HIGH
**Subject**: Issue write (platform path cross-flag interaction)
**Behavior**: When `jr issue create` is invoked WITHOUT `--request-type` but WITH
`--on-behalf-of <ACCOUNT_ID>`, the handler MUST emit ONE warning line to stderr
(NOT stdout, NOT in `--output json` data) BEFORE the platform POST is issued. The
platform path then runs to completion as if `--on-behalf-of` was not supplied. Exit
code is unchanged (0 on success). Stdout output (e.g., `{"key": "FOO-123"}`) is
unchanged. Because `--on-behalf-of` is `Option<String>` and can only appear once on
the command line, idempotency does not alter the observable behavior — one occurrence
emits one warning, matching BC-3.8.011's per-logical-flag-NAME rule.

Verbatim warning string:
`"warning: --on-behalf-of is ignored on the platform create path; it only applies with --request-type (JSM service-desk requests). To raise a request on behalf of another user, also supply --request-type."`

Inverse symmetry to BC-3.8.009: `--on-behalf-of` is accepted and meaningful on the
JSM path (maps to `raiseOnBehalfOf`); on the platform path it has no effect and MUST
warn. The warning fires regardless of `--no-input` or `--output json` settings. If the
command early-exits before the POST (e.g., missing required field), the warning need
not fire.

When `--on-behalf-of` is absent (clap default: None), NO warning is emitted — i.e.,
the stderr stream from a plain platform-path invocation is byte-identical to
pre-issue-#383 behavior.

Cross-reference: BC-3.8.012 and BC-3.8.013 fire independently when both `--field` and
`--on-behalf-of` are present without `--request-type`; both warnings appear on stderr
(each collapsed per its own idempotency rule).

**Inputs**: `--on-behalf-of <ACCOUNT_ID>` WITHOUT `--request-type`
**Outputs/Effects**: ONE stderr warning line; platform POST proceeds normally with
`--on-behalf-of` discarded; stdout and exit code unchanged.
**Errors**: None — this is a warning path, not an error path.
**Trace**: `tests/issue_create_jsm.rs` (integration tests covering platform-path inverse-warning
for `--on-behalf-of`). Test placement: current Trace cites the existing JSM test file;
F3 story-writer may choose to (a) keep tests in `issue_create_jsm.rs` (extending the
file's scope) or (b) split into a new test file for cleaner perimeter. That decision is
deferred to F3.
**Source**: Issue #383 F1 delta analysis; structurally mirrors BC-3.8.010 (Inputs/Outputs
sub-fields), semantically mirrors BC-3.8.011 (warn-and-continue pattern, idempotent per
logical flag NAME); inverse symmetry to BC-3.8.008 / BC-3.8.009. Note: wording expanded
from F1 proposal (`"warning: --on-behalf-of is ignored without --request-type; use
--request-type to submit a JSM request on behalf of another user"`) to clarify the
"platform create path" vs JSM dispatch distinction explicitly, per F2 review.
**Confidence**: HIGH

[NEW 2026-05-19 issue #383 F2] Added to close the platform-path inverse-warning symmetry
gap identified in F1 delta analysis: `--on-behalf-of` is silently dropped on platform
path with no user feedback.

---

#### BC-3.8.014: Basic-auth 401 on JSM POST (`handle_jsm_create`) → API-token-expiry hint; no OAuth-scope language

**Confidence**: HIGH
**Subject**: Issue write (JSM path — auth-conditional error hint)
**Behavior**: When `POST /rest/servicedeskapi/request` returns 401 AND the active auth scheme is Basic (i.e., `JiraClient::is_oauth_auth()` returns `false`), the `handle_jsm_create` `map_err` MUST surface an API-token-expiry hint and exit 2. The gate is `is_oauth_auth() == false` ALONE — the incoming error variant is irrelevant.

Implementation: the `map_err` must inspect `client.is_oauth_auth()`. If `false`, REWRITE any incoming error (whether `JrError::NotAuthenticated` or `JrError::InsufficientScope`) to `JrError::NotAuthenticated { hint: <API_TOKEN_HINT> }`. This rewrite is mandatory: a Basic-auth 401 whose response body contains "scope does not match" would otherwise propagate as `InsufficientScope` (per `src/api/client.rs:696-704`), causing the user to see OAuth scope language that is actionably wrong for Basic-auth users. The rewrite suppresses that path.

The `hint` field value (stored in `JrError::NotAuthenticated { hint }`) MUST be the shared constant `API_TOKEN_EXPIRY_HINT` (defined once in **`src/error.rs`** — NOT in `src/api/client.rs` or any new module — referenced identically by the `handle_jsm_create` site and the `require_service_desk` site — see BC-X.8.006). `src/error.rs` is imported by both the `api` and `cli` layers with no layering inversion, and it keeps "no new modules / no architecture delta" true. This shared constant prevents hint-text divergence between the two call sites.

The rendered stderr line prepends `"Not authenticated. "` (from `src/error.rs:5`); the `hint` field contains only the body text after that prefix. Tests MUST assert via `contains`, not `==`, to tolerate the rendered prefix. The hint field value is:

<!-- This block is duplicated from the CANONICAL copy in prd-delta-384.md §BC-3.8.014 — all copies MUST be updated together; cf. the JR_* doc-fallout pattern in CLAUDE.md (adversary-pass-4 F-04). -->
```
Your API token may be expired or revoked. Regenerate it at
https://id.atlassian.com/manage-profile/security/api-tokens
then run `jr auth login` to re-store the credentials.
```

The hint MUST NOT contain any OAuth-scope language (e.g., `write:servicedesk-request`, `OAuth`, `scope`). Basic-auth users have API tokens with implicit permissions, not OAuth granular scopes; surfacing a scope hint is misleading and actionably wrong. The hint MUST NOT say `jr auth refresh` (meaningless for Basic auth — no OAuth refresh token).

Gate: `client.is_oauth_auth() == false` — predicate is `self.auth_header.starts_with("Bearer ")`. **Value-space precision**: `JiraClient::load_auth_from_keychain` produces exactly `"Bearer {access_token}"` for OAuth or `"Basic {base64_encoded}"` for Basic/API-token. The `JR_AUTH_HEADER` debug-only test seam (CLAUDE.md SD-002, `#[cfg(debug_assertions)]`) can inject either form in tests. `auth_header` is never empty at call time — the constructor errors via `?` if the keychain yields nothing. `is_oauth_auth()` is `self.auth_header.starts_with("Bearer ")` — the SAME discriminant the production code already trusts at `src/api/client.rs:718` and `:802`. No other predicate or ad-hoc string check should be introduced. This is 100% reliable for the value-space produced by `load_auth_from_keychain`.

**Inputs**: Active auth = Basic; JSM POST returns HTTP 401 (any body shape — including generic expiry and "scope does not match" bodies)
**Outputs/Effects**: exit 2; stderr contains the API-token-expiry hint (assert via `contains`); stdout empty; any `InsufficientScope` from the 401 is rewritten to `NotAuthenticated` before surfacing.
**Errors**: None beyond the 401 itself — this BC IS the error-handling contract.
**Trace**: `tests/issue_create_jsm.rs` (Basic-auth 401 integration tests): (a) `test_jsm_create_basic_auth_401_surfaces_api_token_hint` (NEW) — asserts stderr `contains` "expired or revoked" and `contains` `id.atlassian.com/manage-profile/security/api-tokens` and `contains` `jr auth login`; asserts stderr does NOT contain `write:servicedesk-request`; (b) `test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint` (NEW) — pins the `InsufficientScope`→`NotAuthenticated` rewrite path with a "scope does not match" body fixture; (c) `test_jsm_create_401_hint_contains_write_servicedesk_request` (REPURPOSED in place by F4 — fixture stays Basic `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0`, generic-expiry 401 body; F4 MUST flip assertions from `write:servicedesk-request` to API-token-expiry hint and add negative assertion that `write:servicedesk-request` is ABSENT; F4 may rename at discretion; per adversary-pass-9 C-01 correction — this test is a BC-3.8.014 pin, NOT a BC-3.8.015 pin).
**Source**: Issue #384 F2 corrected model; O-08-01 CONFIRMED in `.factory/research/issue-288-pr4-deferred-validation.md`; `src/api/client.rs:696-704` (scope-mismatch body check fires before Bearer guard at line 718 — body content, not auth scheme, decides variant before map_err); CLAUDE.md gotcha "Atlassian's expired-access-token 401 response shape".
**Confidence**: HIGH

[NEW 2026-05-19 issue #384 F2] Closes O-08-01: Basic-auth API-token-expiry 401 was incorrectly surfacing the OAuth `write:servicedesk-request` scope hint. The gate is `is_oauth_auth() == false` alone; the map_err must REWRITE any incoming 401-derived error variant to `NotAuthenticated` with the API-token hint, because a Basic-auth 401 with a "scope does not match" body arrives as `InsufficientScope` (body check at client.rs:696 fires before Bearer guard at line 718).

[REVISED 2026-05-19 issue #384 F2 adversary correction] Previous version incorrectly stated "Basic-auth 401s land in `JrError::NotAuthenticated`, not `InsufficientScope`." This is FALSE. The 401 handler in `src/api/client.rs` checks the response BODY for "scope does not match" at line 696 BEFORE checking the `Bearer` guard at line 718. So a Basic-auth 401 with a scope-mismatch-flavored body lands in `InsufficientScope`. The corrected model: gate is `is_oauth_auth() == false` alone; `map_err` must rewrite both `NotAuthenticated` and `InsufficientScope` arms to the API-token hint.

---

#### BC-3.8.015: OAuth 401 on JSM POST (`handle_jsm_create`) → `write:servicedesk-request` hint via `InsufficientScope` scope-mismatch path (deterministic); `NotAuthenticated` post-refresh path is pre-existing, out of #384 test scope

**Confidence**: HIGH
**Subject**: Issue write (JSM path — auth-conditional error hint)
**Behavior**: When `POST /rest/servicedeskapi/request` returns 401 AND the active auth scheme is OAuth/Bearer (i.e., `JiraClient::is_oauth_auth()` returns `true`), the observable behavior depends on the 401 response body:

- **`JrError::InsufficientScope` (body contains "scope does not match" — client.rs:696-704 short-circuit, DETERMINISTIC):** The scope-mismatch body check at `src/api/client.rs:696-704` fires BEFORE the Bearer guard at `src/api/client.rs:718` AND before the refresh coordinator. This means for a Bearer client, a scope-mismatch 401 short-circuits directly to `InsufficientScope` and lands in `handle_jsm_create`'s `map_err` as a genuine `JrError`. The `map_err` on the `is_oauth_auth() == true` branch preserves `InsufficientScope` and its hint names `write:servicedesk-request` + `required_scope: Some("write:servicedesk-request")`; exit 2. **This is the ONLY deterministically testable OAuth→`JrError`→`write:servicedesk-request` path via the `JR_AUTH_HEADER` test seam.** The EXISTING test `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (under the `// ─── C-01: OAuth InsufficientScope 401 surfaces write:servicedesk-request ────` section banner in `tests/issue_create_jsm.rs`) is the BC-3.8.015 regression pin. It uses `JR_AUTH_HEADER=Bearer test-oauth-token` + body `{"errorMessages": ["Unauthorized; scope does not match"]}` and asserts `write:servicedesk-request`, `jr auth refresh`, `jr auth login`. This test is GREEN on `develop` UNMODIFIED — it is the BC-3.8.015 pin. It MUST remain green unmodified.

- **`JrError::NotAuthenticated` (non-scope-mismatch Bearer 401, post-refresh path — NOT deterministically testable via `JR_AUTH_HEADER` seam):** A Bearer client with a generic-expiry 401 body (no "scope does not match") does NOT short-circuit at client.rs:696-704. Instead, it enters the auto-refresh coordinator at line 727+. In any test using the `JR_AUTH_HEADER=Bearer ...` seam (no keychain OAuth tokens, no `JR_OAUTH_TOKEN_URL` mock), the refresh call deterministically fails with a raw `anyhow::bail!` error from `refresh_oauth_token_with_url` — NOT a `JrError`. That raw anyhow error propagates to `handle_jsm_create`'s `map_err`, where `e.downcast::<JrError>()` hits the `Err(other) => other` arm — no `JrError` branch fires, and the `write:servicedesk-request` hint is NEVER injected. **Consequence:** BC-3.8.015 must NOT claim a Bearer + generic-expiry 401 surfaces `write:servicedesk-request`. The pre-existing `NotAuthenticated` arm rewrite at `src/cli/issue/create.rs:1988-1995` injects `write:servicedesk-request` for OAuth only after a SUCCESSFUL token refresh followed by a 401 retry — this path is real and pre-existing but is NOT reliably reachable via the `JR_AUTH_HEADER` test seam. It is pre-existing behavior, unchanged by #384, and is out of #384's deterministic-test scope. No test for this path is mandated by this delta.

The gate is `is_oauth_auth() == true` ALONE for the `map_err` branch decision. This BC documents what was previously implicit and makes it explicitly gated by the `is_oauth_auth()` check.

Gate: `client.is_oauth_auth() == true` (predicate returns true when `Authorization` header starts with `Bearer `).

**Test instruction (adversary-pass-9 C-01 corrected design):**

`test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` is the BC-3.8.015 regression pin. It is already green on `develop` and MUST remain green unmodified. F4 must NOT alter this test. Confirmed by reading `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` in `tests/issue_create_jsm.rs`: Bearer fixture (`JR_AUTH_HEADER=Bearer test-oauth-token`), scope-mismatch body (`{"errorMessages": ["Unauthorized; scope does not match"]}`), asserts `write:servicedesk-request` + `jr auth refresh` + `jr auth login`. Uses `mount_project_meta_help`, `mount_service_desk_list`, `mount_request_types_password_reset` helpers, project `HELP`, `--request-type "Password Reset"`, `--summary "Reset my password"`.

H-NEW-JSM-RT-003 is re-bound to `test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` — see the Revised Holdout Scenarios section in `prd-delta-384.md`.

`test_jsm_create_401_hint_contains_write_servicedesk_request` (currently `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0`, generic 401 body, asserting `write:servicedesk-request`) is a **BC-3.8.014 pin** post-F4 — NOT a BC-3.8.015 pin. After BC-3.8.014 lands, Basic + generic-401 produces the API-token-expiry hint. F4 MUST repurpose this test in place: keep the Basic fixture, flip the assertions from `write:servicedesk-request` to the BC-3.8.014 API-token-expiry hint, and add a negative assertion that `write:servicedesk-request` is ABSENT. F4 may rename the test at its discretion. The fixture MUST remain Basic — do NOT migrate to Bearer.

**Inputs**: Active auth = Bearer/OAuth; JSM POST returns HTTP 401 with scope-mismatch body (`{"errorMessages": ["Unauthorized; scope does not match"]}`)
**Outputs/Effects**: exit 2; stderr contains `write:servicedesk-request`; stdout empty.
**Errors**: None beyond the 401 itself — this BC IS the error-handling contract.
**Trace**: `tests/issue_create_jsm.rs` — `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (under the `// ─── C-01: OAuth InsufficientScope 401 surfaces write:servicedesk-request ────` section banner; existing test, green on `develop`; logic/fixture/assertions MUST remain unmodified; F4 SHOULD add `// H-NEW-JSM-RT-003 + BC-3.8.015 anchor` to its rustdoc comment — comment-only, no behavior impact; this IS the BC-3.8.015 pin and IS H-NEW-JSM-RT-003 per re-bind in adversary-pass-9 C-01).
**Source**: Issue #384 F2 adversary-pass-9 C-01 corrected design; BC-1.3.023; H-NEW-JSM-RT-003; `src/api/client.rs:696-704` (scope-mismatch short-circuit fires BEFORE refresh coordinator — the ONLY deterministic Bearer→`JrError` path); `src/api/client.rs:718` (Bearer guard — NOT reached for scope-mismatch bodies); `src/api/client.rs:727+` (refresh coordinator — entered by generic-expiry Bearer 401; deterministically fails with raw anyhow error via `JR_AUTH_HEADER` seam, not a `JrError`).
**Confidence**: HIGH

[NEW 2026-05-19 issue #384 F2] Formally pins the OAuth path as the surviving branch after the Basic/OAuth split. Pre-#384, both Basic and OAuth 401s shared the same hint logic; post-#384, the Basic-auth arm is intercepted by BC-3.8.014 before it reaches the OAuth behavior.

[REVISED 2026-05-19 issue #384 F2 adversary-pass-2 C-02/H-05/H-06] (C-02) Renderer prefix corrected: `"Insufficient token scope: "` (colon) not `"Insufficient token scope. "` (period) — per `src/error.rs:8-16`. (H-05/H-06) Corrected false claim about pre-#384 map_err behavior; both arms produce `write:servicedesk-request` for OAuth — exactly as pre-#384.

[REVISED 2026-05-19 issue #384 adversary-pass-5 F-01/F-02/F-03] (F-01) Clarified H-NEW-JSM-RT-003 artifact identity. (F-02) Added explicit warning about mandatory Bearer fixture migration. (F-03) Confirmed test function by reading its body; symbol-relative anchor used.

[REVISED 2026-05-19 issue #384 adversary-pass-8 F-02] Replaced hardcoded line citations with symbol-relative anchors per CLAUDE.md anti-drift convention.

[REVISED 2026-05-19 issue #384 adversary-pass-9 C-01 CRITICAL design correction] Complete rewrite of testable contract. The F2 passes 1-8 plan ("migrate `test_jsm_create_401_hint_contains_write_servicedesk_request` to Bearer + generic-expiry body") was unworkable: a Bearer + generic-expiry 401 routes through the refresh coordinator (client.rs:727+), which deterministically fails with a raw anyhow error (not a `JrError`) via the `JR_AUTH_HEADER` seam, so the `write:servicedesk-request` hint is never injected. The ONLY deterministic Bearer→`JrError`→`write:servicedesk-request` path is the scope-mismatch short-circuit (client.rs:696-704). BC-3.8.015 is now re-specified to its true testable contract: the scope-mismatch path, pinned by the EXISTING `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (already green on `develop`, unmodified). H-NEW-JSM-RT-003 re-bound to this test. `test_jsm_create_401_hint_contains_write_servicedesk_request` stays Basic and becomes a BC-3.8.014 pin with flipped assertions. BC-X.8.007 Setup corrected to scope-mismatch body.

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

## Total BCs in this file: 64 individually-bodied (cumulative 93 incl. range-collapsed; see BC-INDEX.md)

_Last updated 2026-05-19: +2 BCs (BC-3.8.014..015): BC-3.8.014 (Basic-auth 401 API-token hint, with InsufficientScope→NotAuthenticated rewrite) and BC-3.8.015 (OAuth 401 existing behavior now explicitly gated) added in F2 delta (issue #384) to close JSM 401 auth-conditional hint gap (O-08-01). Corrected model: gate is `is_oauth_auth()` alone, not error variant. BC-3.8.001 errors cross-reference refreshed (auth-conditional 401 → BC-3.8.009/014/015; no behavioral change). BC-3.8.009 errors section updated (auth-conditional). BC-3.8 section header updated to 15 contracts, clause (d) added._
