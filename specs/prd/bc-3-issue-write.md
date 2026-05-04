---
context: bc-3
title: "Issue Write (create/edit/move/assign/comment/link/open/remote-link)"
total_bcs: 77   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 48   # count of `#### BC-` headings in this file
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/bc-03-issue-write.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.3
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.1
---

# BC-3 — Issue Write

77 behavioral contracts across 7 subdomains: Assign (3.1), Move/Transition (3.2),
Create (3.3), Edit (3.4), Open (3.4-bug-fix), Comment (3.5), Links (3.6), Remote links (3.7).

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
**Source**: `tests/issue_create_json.rs` (29 tests)
**Subject**: Issue write
**Behavior**: Body includes summary, project, issuetype, optional priority, labels, description (ADF), team UUID, story points. Output JSON: `{"key": "FOO-123"}`.
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

#### BC-3.4.006: `issue edit --label add:foo --label remove:bar` interprets prefix and merges with existing

**Confidence**: MEDIUM
**Source**: `tests/issue_create_json.rs`
**Behavior**: `add:` and `remove:` prefixes adjust existing labels; bare label replaces.
**Trace**: Pass 3 BC-213

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

## Total BCs in this file: 40 (representative; BC-INDEX.md carries all 77)
