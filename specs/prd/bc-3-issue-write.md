---
context: bc-3
title: "Issue Write (create/edit/move/assign/comment/link/open/remote-link)"
total_bcs: 103   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 74   # count of `#### BC-` headings in this file
last_updated: 2026-05-25
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
  - F2 addition (2026-05-20): BC-3.8.016 — --request-type "" (empty) exits 64 before partial_match (issue #385)
  - F2 addition (2026-05-20): BC-3.8.017 — --markdown + --field description= conflict rejected at parse-time exit 64 (issue #385)
  - F2 modified (2026-05-20): BC-3.8.002 — JSM project-required error harmonized with platform affordances (issue #385 O-08-02)
  - F2 modified (2026-05-20): BC-3.8.010 — warning position clarified: fires post-require_service_desk only (issue #385 O-08-07)
  - F2 modified (2026-05-20): BC-3.8.011 — same warning-position constraint applied (issue #385 O-08-07)
  - F2 addition (2026-05-20): BC-3.4.010 — `edit --type` cross-hierarchy 400 → CROSS_HIERARCHY_HINT (JRACLOUD-27893) (issue #388)
  - F2 addition (2026-05-20): BC-3.4.011 — `edit --type` same-hierarchy/indeterminate 400 → typo hint or raw error (issue #388)
  - F2 modified (2026-05-20): BC-3.4.003 — Errors cross-reference added for BC-3.4.010 and BC-3.4.011 (issue #388 annotation only)
  - F2 addition (2026-05-21): BC-3.4.012 — `issue edit` table-mode success echoes one stderr line per changed field (issue #398)
  - F2 addition (2026-05-21): BC-3.4.013 — `issue edit` JSON-mode success includes `changed_fields` object; description carries the RAW user-supplied input string (NOT an adf.rs round-trip); `updated:true` retained (issue #398)
  - F2 addition (2026-05-21): BC-3.4.014 — `issue create` table-mode success echoes resolved team name when `--team` is set (issue #398)
  - F2 modified (2026-05-22, human-gate): BC-3.4.014 — broadened from team-only to ALL set fields, mirroring BC-3.4.012 (human-gate decision 2026-05-22)
  - F2 modified (2026-05-21): BC-3.4.003 — cross-reference to BC-3.4.012 and BC-3.4.013 added (issue #398 annotation only)
  - F2 modified (2026-05-21, adversary round 3): BC-3.4.012 — EC-13 (--description+--summary alphabetical sort pin) and EC-14 (--markdown table-mode still shows (updated)) added (M-1, MED-1, MED-2)
  - F2 modified (2026-05-21, adversary round 3): BC-3.4.013 — EC-11 (--markdown raw Markdown in changed_fields) added; frontmatter trace corrected to raw-input-string model (MED-2, M-1)
  - F2 modified (2026-05-21, adversary round 3): BC-3.4.014 — H1 title KEY token dropped; output channel profile reclassified to profile 4 (Symmetric) (COS-1, MED-4)
  - F2 modified (2026-05-21, adversary round 4): BC-3.4.014 — profile-4 carve-out paragraph added; EC-3.4.014-3 exit code pinned to 64; VP-398-001 fixture constraint added (F-1, O-2, F-3)
  - F2 modified (2026-05-21, adversary round 4): BC-3.4.012 — EC-3.4.012-10 stored-casing clause; VP-398-001 fixture constraint (F-2, F-3)
  - F2 modified (2026-05-21, adversary round 4): BC-3.4.013 — EC-3.4.013-8 stored-casing clause; VP-398-001 fixture constraint (F-2, F-3)
  - F2 modified (2026-05-21, adversary round 5): BC-3.4.012 — VP-398-001 negative case rewritten as direct unit-level is_team_uuid assertion; EC-3.4.012-15 added (MatchResult::None) (F-1, F-3)
  - F2 modified (2026-05-21, adversary round 5): BC-3.4.013 — VP-398-001 negative case rewritten as direct unit-level is_team_uuid assertion; EC-3.4.013-12 added (MatchResult::None) (F-1, F-3)
  - F2 modified (2026-05-21, adversary round 5): BC-3.4.014 — VP-398-001 negative case rewritten as direct unit-level is_team_uuid assertion; EC-3.4.014-5 added (MatchResult::None) (F-1, F-3)
  - F2 modified (2026-05-21, adversary round 7): BC-3.4.012 — VP-398-001 module-private placement sentence added; EC-3.4.012-12 test name pinned; EC-3.4.012-2 clap-conflict wording; VP-398-004 added (F-1, F-2, F-4, F-5)
  - F2 modified (2026-05-21, adversary round 7): BC-3.4.013 — VP-398-001 module-private placement sentence added; EC-3.4.013-10 test name pinned; EC-3.4.013-3 clap-conflict wording; VP-398-002 stdin trailing-newline sub-case inline; VP-398-004 added (F-1, F-2, F-4, F-5, F-6)
  - F2 modified (2026-05-21, adversary round 7): BC-3.4.014 — VP-398-001 module-private placement sentence added (F-1)
  - F2 modified (2026-05-21, adversary round 8): BC-3.4.012 — two-site insertion enumeration for points/parent; f64 .to_string() invariant scoped to --points branch; concrete assertion values for points; EC-3.4.012-12 pinned as integration test (wiremock); EC-3.4.012-16 added (empty-stdin edge case) (MAJOR-1, IMP-3, OBS-2, OBS-4)
  - F2 modified (2026-05-21, adversary round 8): BC-3.4.013 — two-site insertion enumeration for points/parent; f64 .to_string() invariant scoped to --points branch; invariant 4 + VP-398-003 body add test_edit_response_empty_changed_fields; EC-3.4.013-13 added (empty-stdin edge case) (MAJOR-1, MAJOR-2, IMP-3)
  - F2 modified (2026-05-21, adversary round 9): BC-3.4.012 — EC-3.4.012-12 wiremock-only note added (IMPORTANT-1)
  - F2 modified (2026-05-21, adversary round 9): BC-3.4.013 — EC-3.4.013-10 wiremock-only note added (IMPORTANT-1)
  - F2 modified (2026-05-21, adversary round 10): BC-3.4.012 — invariant 6 added (map construction vs emission timing; map discarded on PUT error, emitted only post-204); EC-3.4.012-16 has_any_field_change→has_updates [NOTE: this rename was an over-correction; corrected back in round 12] (IMPORTANT-3, IMPORTANT-2)
  - F2 modified (2026-05-21, adversary round 10): BC-3.4.013 — invariant 4 pinned regenerated snapshot body + top-level key order note; invariant 6 added (map construction vs emission timing); EC-3.4.013-13 has_any_field_change→has_updates [NOTE: this rename was an over-correction; corrected back in round 12]; top-level key order note added to signature paragraph (MAJOR-1, IMPORTANT-1, IMPORTANT-2, IMPORTANT-3)
  - F2 modified (2026-05-21, adversary round 12): BC-3.4.012 — EC-3.4.012-16 reverted to has_any_field_change (pre-HTTP guard at create.rs:341); two-guard clarifying parenthetical added (MAJOR-2)
  - F2 modified (2026-05-21, adversary round 12): BC-3.4.013 — EC-3.4.013-13 reverted to has_any_field_change (pre-HTTP guard at create.rs:341); two-guard clarifying parenthetical added; serde_json top-level key order rationale corrected from insertion-order to alphabetical-by-default (MAJOR-1, MAJOR-2)
  - F2 modified (2026-05-21, adversary round 12): BC-3.4.013 — signature paragraph top-level key order rationale corrected from insertion-order to alphabetical-by-default (MAJOR-1)
  - F2 modified (2026-05-21, adversary round 12): BC-3.4.013 — invariant 4 top-level key order rationale corrected from insertion-order to alphabetical-by-default (MAJOR-1)
  - F2 addition (2026-05-22): BC-3.4.015 — `issue edit --field NAME=VALUE` string/number/date/datetime/user field on single-key path via editmeta (issue #396)
  - F2 addition (2026-05-22): BC-3.4.016 — `issue edit --field NAME=VALUE` single-select option field: value→allowedValues id resolution, wire `{"id":"..."}`, echo shows human label (issue #396)
  - F2 addition (2026-05-22): BC-3.4.017 — `--field` multi-key/--jql multi-issue rejection (C-1 guard) + flag-overlap hard error for summary/description/issuetype/priority (issue #396)
  - F2 amended (2026-05-22, adversary pass 1): BC-3.4.015 — EC-3.4.015-9 empty-NAME behavior corrected; EC-3.4.015-4a number wire format; EC-3.4.015-12a PUT-failure discard; EC-3.4.015-17 case-sensitive bypass deliberate; EC-3.4.015-18 dry-run; resolve_edit_fields canonical signature; VP-396-007..010 added
  - F2 amended (2026-05-22, adversary pass 1): BC-3.4.016 — EC-3.4.016-4 id/label collision note; VP-396-006 added to Verification Properties
  - F2 amended (2026-05-22, adversary pass 1): BC-3.4.017 — invariant 1 Gate B-before-A ordering; EC-3.4.017-2 JQL-multi clarification; EC-3.4.017-10 same-field two-pairs; EC-3.4.017-11 type vs issuetype; EC-3.4.017-12 simultaneous Gate A+B; Gate A postcondition split; LOW-001 EC ref corrected; VP-396-008 added
  - F2 amended (2026-05-22, adversary pass 3): BC-3.4.015 — Step 3b (operations/"set" check + exit 64 hint) added; EC-3.4.015-19 (resolution failure under --dry-run exits 64); EC-3.4.015-20 (operations lacks "set"); EC-3.4.015-18 exit code pinned to 0; VP-396-011 (user/date/datetime wire) and VP-396-012 (operations check) added; VP-396-008 one-liner updated
  - F2 modified (2026-05-25): BC-3.4.017 — EC-3.4.017-14 added (mechanical enforcement meta-test for invariant 2 completeness); invariant 2 cross-reference added (issue #407 F2)
---

# BC-3 — Issue Write

103 behavioral contracts across 8 subdomains: Assign (3.1), Move/Transition (3.2),
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
**Errors**: When `edit --type X` returns HTTP 400, the error path is further classified — see BC-3.4.010 (cross-hierarchy mismatch → `CROSS_HIERARCHY_HINT`) and BC-3.4.011 (same-hierarchy or indeterminate → typo hint or raw error). The primary success path (PUT 204) and ADF description behavior are byte-for-byte unchanged.
**Success output**: On the single-key success path (PUT 204), see BC-3.4.012 (table-mode success: one stderr line per changed field in `field → value` format) and BC-3.4.013 (JSON-mode success: `edit_response` extended with `changed_fields` map). This contract specifies only the PUT wire contract; BC-3.4.012 and BC-3.4.013 govern the confirmation output layer.
**Trace**: Pass 3 BC-1055 (R4)

> **[UPDATED 2026-05-20 issue #388]** Errors cross-reference added for `edit --type` 400 enrichment paths (BC-3.4.010, BC-3.4.011). No behavioral change to this contract.

> **[UPDATED 2026-05-21 issue #398]** Success output cross-reference added for changed-fields echo (BC-3.4.012, BC-3.4.013). No behavioral change to the PUT wire contract.

> **[UPDATED 2026-05-22 issue #396]** `--field NAME=VALUE` extension cross-reference added: BC-3.4.015 (string/number/date/datetime/user field single-key path), BC-3.4.016 (single-select option field), BC-3.4.017 (multi-key/--jql rejection + flag-overlap guard). These BCs extend the `handle_edit` execution path but do not change the PUT wire contract specified here.

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

#### BC-3.4.010: `issue edit KEY --type X` HTTP 400 + cross-hierarchy subtask-flag mismatch → exit 1, `CROSS_HIERARCHY_HINT` on stderr (JRACLOUD-27893)

**Confidence**: HIGH
**Source**: `tests/issue_edit_type_errors.rs` (integration tests — cross-hierarchy direction paths); `src/cli/issue/create.rs::is_cross_hierarchy_type_error` (pure classifier helper); `src/cli/issue/create.rs::CROSS_HIERARCHY_HINT` (shared constant); `src/cli/issue/create.rs` inline `#[cfg(test)] mod is_cross_hierarchy_type_error_proptests` proptest for `is_cross_hierarchy_type_error`
**Subject**: Issue write
**Behavior**: When `edit_issue` returns HTTP 400 AND `is_cross_hierarchy_type_error(src_subtask, tgt_subtask, err)` returns `CrossHierarchy` (i.e., both `src_subtask` and `tgt_subtask` are `Some(a)` and `Some(b)` with `a != b`, covering both standard→sub-task and sub-task→standard directions), the CLI exits 1 and emits `CROSS_HIERARCHY_HINT` on stderr. The hint wording is pinned verbatim:

```
The Jira Cloud REST API does not support changing the standard / sub-task hierarchy level via this endpoint (see JRACLOUD-27893). To convert it, open the issue in the Jira web UI and use the action menu to find the Convert option.
```

This shared constant is also emitted on the `--no-parent` subtask-bound 400 path (gated by `no_parent && is_subtask_parent_error` in `handle_edit`). On the `--no-parent` path, the caller MUST prepend the following verbatim context sentence before the shared constant:

```
Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue.
```

On the `edit --type` path, the constant is emitted directly with no prepended sentence. The neutral framing ("does not support changing the...hierarchy level via this endpoint") accurately describes both call sites — neither requires the word "Converting" which would mis-describe the `--no-parent` case.

**Preconditions**:
- Single-key `jr issue edit KEY --type X` is issued (multi-key bulk path is unaffected by this contract).
- `edit_issue` (PUT `/rest/api/3/issue/<key>`) returns HTTP 400. **HTTP-400 gate**: the caller (`handle_edit`) observes this by downcasting `edit_issue`'s `anyhow::Error` to `JrError::ApiError { status: 400, .. }` (constructed at `src/api/client.rs::parse_error` ~lines 973-997, defined in `src/error.rs`). If `edit_issue` fails with a non-400 error (401, 403, 5xx, network error, etc.), NO enrichment occurs — the raw error is surfaced unchanged and neither BC-3.4.010 nor BC-3.4.011 enrichment applies. The error-enrichment block is entered only on `status == 400`. Note: a non-400 `edit_issue` error (R0b routing row) bypasses both BC-3.4.010 and BC-3.4.011 entirely; see test #10 (`test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment`).
- **Call ordering**: `handle_edit` calls `get_issue` FIRST (it supplies both the source `issuetype.subtask` flag and `fields.project.key`). Only if `get_issue` succeeds is `get_project_issue_types(project_key)` called. Therefore: a `get_issue` failure → Indeterminate immediately (the second call never executes); the unresolvable-name sub-path is reachable only when `get_issue` already succeeded and returned HTTP 200.
- `get_issue` uses the full `BASE_ISSUE_FIELDS` projection (which includes `"issuetype"`). The Atlassian Jira Cloud REST API v3 returns the complete `IssueType` object — including the `subtask` boolean and `hierarchyLevel` — as a nested field within any projected `issuetype` field. The `fields=` query parameter filters top-level issue fields, NOT nested properties of a returned field. Therefore `get_issue` (with `issuetype` in `BASE_ISSUE_FIELDS`) returns the `subtask` sub-field reliably. The `subtask` field is carried in `IssueType` (the struct at `fields.issuetype` in the `Issue` response from `get_issue`); this is the struct that receives the additive `subtask: Option<bool>` field in issue #388 (F4 implementation, not yet in the codebase at F2 spec time).
- **`Option<IssueType>` outer-layer flatten**: `issue.fields.issuetype` in `src/types/jira/issue.rs:62` is `Option<IssueType>` (the whole issuetype object may be absent from the response). `IssueType.subtask` is itself `Option<bool>`. The caller MUST flatten both layers: `issue.fields.issuetype.as_ref().and_then(|t| t.subtask)`. Two distinct sources of `src_subtask: None` exist: (a) the `issuetype` object is wholly absent from the response `fields` — `Option<IssueType>` is `None`; (b) `issuetype` is present but its `subtask` key is omitted from the JSON — `IssueType.subtask` is `None`. Both (a) and (b) collapse to `src_subtask: None` → Indeterminate via the and_then flatten.
- **`get_project_issue_types` deserialization behavior (net-new lookup)**: The type-name lookup against `get_project_issue_types` is **net-new F4 logic** built inside `handle_edit`'s error path — it does not pre-exist. `get_project_issue_types` calls `GET /rest/api/3/project/{key}`, extracts `issueTypes`, and deserializes via `.and_then(|v| from_value::<Vec<IssueTypeMetadata>>(v).ok()).unwrap_or_default()` (live code, `src/api/jira/projects.rs:47-51`). A 200 response with a malformed or missing `issueTypes` key returns `Ok(vec![])` — NOT an `Err`. Therefore deserialization failure is NOT an Indeterminate-trigger; only an HTTP error or network error causes `get_project_issue_types` to return `Err` (→ Indeterminate). A 200 with an unparseable body yields `Ok([])` → the target name is absent from an empty list → the **unresolvable-name sub-path** (typo hint), NOT Indeterminate. This graceful outcome is acceptable: a malformed project-metadata response is rare and the typo hint is not harmful. The client-side name lookup uses **case-insensitive exact match** on the `name` field — this is a deliberate choice for the error-enrichment path and may not perfectly mirror Jira's server-side resolution, but divergence only affects which hint is shown, never edit correctness.
- `is_cross_hierarchy_type_error(src_subtask, tgt_subtask, err)` returns `CrossHierarchy`: both arguments are `Some(_)` and the inner boolean values differ (`src != tgt`).

**Postconditions**:
- Exit code 1.
- Stderr contains the verbatim `CROSS_HIERARCHY_HINT` string:
  ```
  The Jira Cloud REST API does not support changing the standard / sub-task hierarchy level via this endpoint (see JRACLOUD-27893). To convert it, open the issue in the Jira web UI and use the action menu to find the Convert option.
  ```
- Stderr contains the literal `JRACLOUD-27893`.
- Stderr does NOT contain the substring `jr api /rest/api/3/issue` (regression pin unique to the removed fake `PUT /rest/api/3/issue/{key}/convert` hint at `src/cli/issue/create.rs:834`; the exact prior hint text was `jr api /rest/api/3/issue/{key}/convert -X put -d '{"type":{"name":"Task"}}'`; the pin substring `jr api /rest/api/3/issue` uniquely identifies this removed fake-endpoint hint without over-matching the broader `/rest/api/3/issue/` path fragment which may legitimately appear in other diagnostics).
- Stdout is empty (no JSON output for this error path).

**Invariants**:
1. The subtask-flag mismatch via `is_cross_hierarchy_type_error(src_subtask: Option<bool>, tgt_subtask: Option<bool>, err: &str) -> Classification` is the PRIMARY classifier — locale-independent. The pure function returns `CrossHierarchy` only when both arguments are `Some(_)` and differ. The English substring `"issue type selected is invalid"` MUST NOT be used as the sole gate (it fires on plain typos; see research addendum A1).
2. `CROSS_HIERARCHY_HINT` is a shared named constant referenced identically from this path and from the `--no-parent` subtask-bound 400 path (gated by `no_parent && is_subtask_parent_error` in `src/cli/issue/create.rs`, call-site symbol `handle_edit`). Bug fix: replaces the prior fake `PUT /rest/api/3/issue/{key}/convert` hint. On the `--no-parent` path, the caller MUST prepend the following verbatim context sentence before the shared constant:

```
Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue.
```

On the `edit --type` path, the constant is emitted directly with no prepended sentence. The context sentence frames conversion as the means to clear the parent and leads directly into the shared `CROSS_HIERARCHY_HINT`.
3. This contract applies to SINGLE-KEY edit only. The bulk `--type` path (`handle_edit_bulk_fields`) does NOT include this enrichment and must not be modified.

> **Wording note (not a runtime contract):** The word "sub-task" is spelled with a hyphen throughout all hint strings in this BC (not "subtask" without hyphen). This is a spec-drafting convention for the pinned hint strings above; it is not enforced by any test and does not produce observable CLI behavior distinct from a non-hyphenated spelling.

**Deliberate gate asymmetry (m-4)**: The `edit --type` arm enters the enrichment block via a structured downcast: `edit_issue`'s `anyhow::Error` downcasts to `JrError::ApiError { status: 400, .. }` (per `src/error.rs`). The `--no-parent` arm uses the legacy string-based gate `is_subtask_parent_error(&anyhow::Error)` to decide whether to emit the prepended context sentence + `CROSS_HIERARCHY_HINT`. This asymmetry is deliberate: migrating `is_subtask_parent_error` to a structured downcast is explicitly out of #388 scope per KL-3.4.010-1 below — both gates reach the same shared constant, but via distinct mechanisms that were intentionally left unchanged.

**`--no-parent` hint replacement scope (CRITICAL)**: The ENTIRE prior `--no-parent` hint block at `src/cli/issue/create.rs:830-837` is replaced. The multi-line `format!` that composed the prior hint spans lines 830-836; line 837 is the separate `bail!` statement. The prior `format!` contained FOUR sentences: "Tip: subtasks are structurally bound…", "To clear the parent, first convert…", the fake `jr api /rest/api/3/issue/{key}/convert -X put -d '{"type":{"name":"Task"}}'` line, and "(then re-run with --no-parent if needed.)". NONE of these four old sentences are retained. The new block is exactly: the verbatim context sentence below (prepended first), followed immediately by `CROSS_HIERARCHY_HINT` — and nothing else.

**`--no-parent` path postcondition (M-1)**: When `no_parent && is_subtask_parent_error` fires (the `--no-parent` subtask-bound 400 path), stderr MUST contain:
1. The verbatim context sentence `Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue.` (prepended before the shared constant).
2. The verbatim `CROSS_HIERARCHY_HINT` string (containing `JRACLOUD-27893`).
3. The literal `JRACLOUD-27893`.
4. NOT the substring `jr api /rest/api/3/issue` (regression pin on removed fake-endpoint hint; the removed fake hint was `jr api /rest/api/3/issue/{key}/convert -X put -d '{"type":{"name":"Task"}}'` — the pin substring uniquely identifies this removed text).

This postcondition is verified by **T-06 in `tests/issue_edit_no_parent.rs`** (`test_subtask_parent_clear_surfaces_400_with_convert_hint`), NOT by the `issue_edit_type_errors.rs` test set (tests #1/#2/#5 in that file cover the `edit --type` path only).

**`--type` + `--no-parent` dual-gate precedence**: `--type` and `--no-parent` are NOT mutually exclusive in clap — there is NO `conflicts_with` annotation between `issue_type` and `no_parent` on the `IssueCommand::Edit` variant (confirmed in `src/cli/mod.rs` lines 437-459). Both flags can be supplied simultaneously. If both are set and `edit_issue` returns HTTP 400, both the `--type` cross-hierarchy enrichment arm and the `--no-parent` arm could have satisfied preconditions. The deterministic evaluation order in `handle_edit`'s `if let Err(ref e) = edit_result` block MUST be: the `--type` cross-hierarchy enrichment is evaluated FIRST (invoking `get_issue` → `get_project_issue_types` → `is_cross_hierarchy_type_error`); only if it does NOT emit a hint (i.e., the classification is SameCategory or Indeterminate and no hint was shown) does the `--no-parent` arm evaluate. This ordering ensures the more-specific cross-hierarchy diagnosis takes precedence over the legacy string-match gate.

**Known Limitations**:
- KL-3.4.010-1: The `--no-parent` arm's hint emission is gated by `is_subtask_parent_error`, which is a disjunctive English-substring matcher: `msg.contains("subtask") || (msg.contains("parent") && msg.contains("400"))`. The locale-fragility risk differs by disjunct: the first disjunct (`"subtask"`) is an English word and will miss the error on non-English Jira instances; the second disjunct (`"parent"` + `"400"`) is partially locale-robust because `"400"` is a locale-independent HTTP status token, but `"parent"` is still English and may not appear in non-English error messages. Both disjuncts are inherited from the pre-#388 `is_subtask_parent_error` implementation. This is a deliberate scope boundary for issue #388 — modifying `is_subtask_parent_error`'s locale resilience is not part of this delta and is not a regression introduced here.

**Edge Cases**:
- EC-3.4.010-1: standard→sub-task direction (source `subtask: false`, target `subtask: true`) → same hint, same exit code.
- EC-3.4.010-2: sub-task→standard direction (source `subtask: true`, target `subtask: false`) → same hint, same exit code.
- EC-3.4.010-3: The English error substring `"issue type selected is invalid"` is present in the 400 body but the flags DO match (same hierarchy, typo scenario) → hint MUST NOT fire; this is the BC-3.4.011 SameCategory path.

**Trace**: issue #388 F2; `src/cli/issue/create.rs::is_cross_hierarchy_type_error`; `src/cli/issue/create.rs::CROSS_HIERARCHY_HINT`; `src/cli/issue/create.rs` inline `#[cfg(test)] mod is_cross_hierarchy_type_error_proptests` proptest for `is_cross_hierarchy_type_error`; `tests/issue_edit_type_errors.rs` (integration — cross-hierarchy direction paths)

---

#### BC-3.4.011: `issue edit KEY --type X` HTTP 400 + same-hierarchy flags OR indeterminate resolution → exit 1, typo hint or raw error (no JRACLOUD-27893 hint)

**Confidence**: HIGH
**Source**: `tests/issue_edit_type_errors.rs` (integration tests — same-hierarchy typo path, indeterminate paths); `src/cli/issue/create.rs::is_cross_hierarchy_type_error` (pure classifier — `SameCategory` and `Indeterminate` return paths); `src/cli/issue/create.rs` inline `#[cfg(test)] mod is_cross_hierarchy_type_error_proptests` proptest for `is_cross_hierarchy_type_error` (primary verification for classifier properties); `src/cli/issue/create.rs::handle_edit` (caller: unresolvable name → typo hint; fetch-failure → `Indeterminate`)
**Subject**: Issue write
**Behavior**: When `edit_issue` returns HTTP 400 (observed by downcasting to `JrError::ApiError { status: 400, .. }` — constructed at `src/api/client.rs::parse_error` ~lines 973-997, defined in `src/error.rs`) AND `is_cross_hierarchy_type_error(src_subtask, tgt_subtask, err)` does NOT return `CrossHierarchy`, the CLI exits 1 without emitting `CROSS_HIERARCHY_HINT`. If `edit_issue` fails with a non-400 error (401, 403, 5xx, network error, etc.), NO enrichment occurs — the raw error is surfaced unchanged and neither BC-3.4.010 nor BC-3.4.011 enrichment applies; this is the R0b routing row tested by test #10. Three distinct sub-paths apply (all require the HTTP-400 gate to have fired):

**Indeterminate fetch-failure detection — `is_err()` gate, NOT a status downcast**: The `handle_edit` enrichment-fetch failure gate is `Result::is_err()` on the `get_issue` / `get_project_issue_types` call — ANY `Err` variant triggers Indeterminate, regardless of the underlying error variant. This is deliberately distinct from the HTTP-400 gate on `edit_issue`'s error, which IS a structured downcast to `JrError::ApiError { status: 400, .. }` (because `edit_issue`'s 400 does become `ApiError` via `parse_error`). An implementer who detects the Indeterminate fetch-failure by "downcast the enrichment-fetch error to `JrError::ApiError` and check status" would MISS 401s and other non-ApiError variants. Specifically: `get_issue` returning HTTP 401 does NOT produce `JrError::ApiError` — it produces `JrError::NotAuthenticated` or `JrError::InsufficientScope` (per `src/api/client.rs::parse_error` ~lines 973-997 which dispatches 401 to these variants, not `ApiError`). The `is_err()` gate catches all `Err` variants uniformly. The two gate mechanisms are deliberately different and must not be conflated.

**Unresolvable-name sub-path (SameCategory outcome, caller-side)** — `handle_edit` resolves the target type name `X` against the project's issue-type list BEFORE invoking the pure classifier. If `get_project_issue_types` returns HTTP 200 with a non-empty list that simply does not contain the requested name `X` (i.e., a typo'd or wrong type name), `handle_edit` emits the typo hint directly and never calls the classifier:
- Emit the pinned typo hint on stderr:

```
Jira rejected the type change. If the type name is wrong, run `jr project types` to list valid types; the change may also be blocked by workflow or scheme constraints.
```

- Surface the `extract_error_message`-processed 400 message text carried in `JrError::ApiError.message` on stderr (this is the extracted message only — e.g., `issuetype: The issue type selected is invalid.`; the raw JSON envelope such as `{"errors": {...}}` is NOT surfaced because `JiraClient::parse_error` in `src/api/client.rs` runs `extract_error_message()` on the response bytes before constructing `JrError::ApiError.message`; `extract_error_message` is `sanitize_for_stderr(extract_error_message_raw(body))` per `src/api/client.rs:1481` — for plain-ASCII message text, `sanitize_for_stderr` is a no-op, so test substrings from plain-ASCII extracted text are safe; test assertions MUST use plain-ASCII substrings, not control characters or multibyte sequences). When asserting this in tests (#3), choose a substring from the EXTRACTED message (e.g., `The issue type selected is invalid` survives extraction; `{"errors"` or `"issuetype":` as raw JSON keys do not).
- `CROSS_HIERARCHY_HINT` (containing `JRACLOUD-27893`) MUST NOT appear on stderr.
- The pure classifier (`is_cross_hierarchy_type_error`) is NOT invoked on this path.

**SameCategory sub-path (classifier-side)** — `get_project_issue_types` succeeds and the target name IS found; `is_cross_hierarchy_type_error` returns `SameCategory`: both `src_subtask` and `tgt_subtask` are `Some(_)` and the inner boolean values are equal. This covers valid type names rejected by workflow or scheme constraints (a valid type name rejected because the target workflow lacks the issue's current status). The enrichment lookup that determines whether the name IS found uses **case-insensitive exact match on the issue-type `name` field** (so the enrichment verdict agrees with how Jira server-side resolves the type name; partial_match substring matching MUST NOT be used, which could mis-resolve ambiguous type names):
- Emit the same pinned typo hint on stderr (verbatim above).
- Surface the `extract_error_message`-processed 400 message text carried in `JrError::ApiError.message` on stderr (same extraction semantics as the unresolvable-name sub-path above — `sanitize_for_stderr(extract_error_message_raw(body))` is effectively a no-op for plain-ASCII text; assert a plain-ASCII substring from the extracted message in tests (#4), not raw JSON envelope keys).
- `CROSS_HIERARCHY_HINT` (containing `JRACLOUD-27893`) MUST NOT appear on stderr.

**Indeterminate sub-path** — `is_cross_hierarchy_type_error` returns `Indeterminate`. This occurs in two distinct ways:
1. **Either enrichment fetch fails** (Cause-1): `get_issue` OR `get_project_issue_types` returns `Err` — detected by `Result::is_err()` on the call result. ANY `Err` variant triggers Indeterminate: `JrError::NotAuthenticated` (e.g., a `get_issue` 401), `JrError::InsufficientScope` (a `get_issue` 403 scope failure), `JrError::ApiError { status: 5xx, .. }`, network errors, and all other `Err` variants. The `handle_edit` caller does NOT downcast or inspect the error variant — `is_err()` is the gate. NOTE: a 200 response with a malformed `issueTypes` body is NOT a fetch failure — `get_project_issue_types` returns `Ok(vec![])` in that case (due to `.and_then(|v| from_value::<Vec<IssueTypeMetadata>>(v).ok()).unwrap_or_default()` in `src/api/jira/projects.rs:47-51`), which routes to the unresolvable-name sub-path (typo hint), NOT Indeterminate. Indeterminate via Cause-1 requires an actual `Err`, not a 200 with malformed body.
2. **A fetch succeeds but the `subtask` field is absent** (Cause-2): `get_issue` or `get_project_issue_types` returns HTTP 200, but the `issuetype.subtask` field is missing (`None`) after deserialization (field omitted by Jira). The pure classifier `is_cross_hierarchy_type_error(None, _, _)` or `is_cross_hierarchy_type_error(_, None, _)` returns `Indeterminate`. Note: for the source-issue side, Cause-2 also covers the case where the `issuetype` object is wholly absent (`Option<IssueType>` is `None`), because `issue.fields.issuetype.as_ref().and_then(|t| t.subtask)` produces `None` for both a missing issuetype and a present-but-subtask-absent issuetype.

On either Indeterminate cause:
- Surface the `extract_error_message`-processed 400 message text carried in `JrError::ApiError.message` on stderr with NO enrichment hint. When asserting this in tests (#6, #7), choose a substring from the extracted message, not raw JSON envelope keys.
- Neither the cross-hierarchy hint (`CROSS_HIERARCHY_HINT`) nor the typo/workflow hint is emitted.
- Exit code 1.

**Preconditions**:
- Single-key `jr issue edit KEY --type X` is issued.
- `edit_issue` returns HTTP 400. (If `edit_issue` fails with a non-400 error, no enrichment occurs — see R0b routing row / test #10.)
- **Call ordering**: `handle_edit` calls `get_issue` FIRST. Only if `get_issue` succeeds (HTTP 200) is `get_project_issue_types(project_key)` called. A `get_issue` failure — detected by `Result::is_err()` (ANY `Err` variant, not a downcast) → Indeterminate immediately (the second call never executes). The unresolvable-name sub-path is reachable only when `get_issue` already succeeded. This ordering ensures the caller-side routing is provably total with no input matching two branches simultaneously.
- **`Option<IssueType>` outer-layer flatten**: `issue.fields.issuetype` (`src/types/jira/issue.rs:62`) is `Option<IssueType>`; `IssueType.subtask` is `Option<bool>`. The caller MUST read `src_subtask` via `issue.fields.issuetype.as_ref().and_then(|t| t.subtask)`. Two distinct sources of `src_subtask: None` exist: (a) the `issuetype` object is wholly absent from the response — `Option<IssueType>` is `None`; (b) `issuetype` is present but its `subtask` key is omitted from the JSON — `IssueType.subtask` is `None`. Both (a) and (b) collapse to `src_subtask: None` → Indeterminate. Test #6 covers case (b) (source-side subtask key omitted); it also covers case (a) via the same `and_then` flatten path — both produce `src_subtask: None` and route identically.
- ONE OF three routing conditions applies:
  - (Unresolvable-name) `get_project_issue_types` returns HTTP 200 with a non-empty list that does not contain the target name `X` → caller emits typo hint without invoking classifier.
  - (SameCategory) Both `get_issue` and `get_project_issue_types` succeed, the target name IS found, and the deserialized `subtask` values are both `Some(_)` AND equal → classifier returns `SameCategory` → typo hint emitted.
  - (Indeterminate) At least one of `get_issue` or `get_project_issue_types` returns an `Err` (ANY 4xx, 5xx, or network error — NOT a 200 with malformed body, which routes to unresolvable-name instead), OR both fetches return 200 but at least one `subtask` field is `None` → raw error only.

**Postconditions**:
- Exit code 1.
- `CROSS_HIERARCHY_HINT` is absent from stderr on ALL sub-paths (prevents false positives on plain typos and workflow-incompatibility 400s).
- Unresolvable-name and SameCategory: stderr contains the pinned typo hint (verbatim above) plus the `extract_error_message`-processed 400 message text carried in `JrError::ApiError.message`.
- Indeterminate: stderr contains the `extract_error_message`-processed 400 message text carried in `JrError::ApiError.message`; no enrichment hint of any kind.

**Invariants**:
1. `JRACLOUD-27893` MUST NOT appear on stderr on any of the three sub-paths. This prevents the cross-hierarchy hint from misleading users experiencing typos or workflow-incompatibility rejections.
2. Indeterminate degrades gracefully: a fetch failure on the error-enrichment path never supersedes the original 400 error body.
3. The unresolvable-name case routes to the typo hint (not Indeterminate) because the 200 response confirms the API is reachable and the name is definitively wrong — no ambiguity warrants degrading to raw error.

**Edge Cases**:
- EC-3.4.011-1: Both flags are `subtask: false` (two standard issue types, different names — target name found) → SameCategory → typo/workflow hint; no JRACLOUD-27893.
- EC-3.4.011-2: `get_project_issue_types` returns HTTP 5xx → Indeterminate (Cause-1, `is_err()` gate) → `extract_error_message`-processed 400 message only; no hint. Tested by `test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error` (test #4 — covers the R2 routing row: `get_issue` succeeds, project-types call returns 5xx).
- EC-3.4.011-3: `get_project_issue_types` returns HTTP 200 with a non-empty list that does NOT contain the target name `X` (typo'd or wrong type name) → unresolvable-name sub-path → typo hint; NOT Indeterminate. The caller `handle_edit` emits the typo hint directly without invoking the pure classifier (because the name is definitively absent from a successful 200 response, not an API error). Tested by `test_edit_type_unresolved_type_name_surfaces_typo_hint` (test #8).
- EC-3.4.011-4: `get_issue` returns HTTP 401 (auth failure on enrichment fetch — surfaces as `JrError::NotAuthenticated` or `JrError::InsufficientScope`, NOT `JrError::ApiError{401}`, per `src/api/client.rs::parse_error`) → Indeterminate (Cause-1, caught by `is_err()` gate on the `get_issue` call) → `extract_error_message`-processed 400 message only; no hint; `JRACLOUD-27893` absent. This is the R1 routing row (`get_issue` itself fails): `get_issue` returns 5xx or any error → Indeterminate immediately (project-types never called). Tested by `test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error` (test #9 — distinct from test #4 which covers R2 where `get_issue` succeeds but project-types fails).
- EC-3.4.011-5: `get_issue` returns HTTP 200 but Jira omits the `subtask` field from the issuetype object → `subtask: None` after deserialization → `is_cross_hierarchy_type_error(None, _, _)` returns `Indeterminate` → `extract_error_message`-processed 400 message only; no hint. Tested by `test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error` (test #6 — source-side absent subtask flag).
- EC-3.4.011-6: `get_issue` returns HTTP 200 (source `subtask` field present), `get_project_issue_types` returns HTTP 200, but the matched target type's `subtask` key is OMITTED from the response object → `tgt_subtask: None` after deserialization → `is_cross_hierarchy_type_error(_, None, _)` returns `Indeterminate` → `extract_error_message`-processed 400 message only; no enrichment hint; `JRACLOUD-27893` absent. Tested by `test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error` (test #7 — target-side absent subtask flag; symmetric to EC-3.4.011-5).
- EC-3.4.011-7: `get_project_issue_types` returns HTTP 200 with a list that does NOT contain the target name `X` (unresolvable-name path) → typo hint → exit 1; `JRACLOUD-27893` absent; `jr api /rest/api/3/issue` absent. Tested by `test_edit_type_unresolved_type_name_surfaces_typo_hint` (test #8 — the eighth integration test added to cover this previously-untested sub-path).

**Test sub-path mapping** (authoritative — tests in `tests/issue_edit_type_errors.rs`):
- Test #1 (`test_edit_type_cross_hierarchy_std_to_subtask_surfaces_conversion_hint`): CrossHierarchy standard→subtask direction — exercises BC-3.4.010.
- Test #2 (`test_edit_type_cross_hierarchy_subtask_to_std_surfaces_conversion_hint`): CrossHierarchy subtask→standard direction — exercises BC-3.4.010.
- Test #3 (`test_edit_type_same_hierarchy_400_surfaces_typo_hint`): SameCategory classifier-side (both flags `Some(false)`, target name found, hierarchy equal) — exercises BC-3.4.011 SameCategory classifier-side sub-path. `JRACLOUD-27893` MUST NOT appear.
- Test #4 (`test_edit_type_indeterminate_project_types_5xx_surfaces_raw_error`): Indeterminate Cause-1 (GET project types returns 5xx) — exercises BC-3.4.011 Indeterminate sub-path. `JRACLOUD-27893` MUST NOT appear.
- Test #5 (`test_edit_type_cross_hierarchy_hint_no_fake_endpoint_literal`): Regression pin — CrossHierarchy path does NOT emit `jr api /rest/api/3/issue` — exercises BC-3.4.010 postcondition.
- Test #6 (`test_edit_type_indeterminate_absent_subtask_flag_surfaces_raw_error`): Indeterminate Cause-2 source-side (subtask field absent on GET issue) — exercises BC-3.4.011 Indeterminate sub-path EC-3.4.011-5.
- Test #7 (`test_edit_type_indeterminate_absent_target_subtask_flag_surfaces_raw_error`): Indeterminate Cause-2 target-side (subtask field absent on GET project types) — exercises BC-3.4.011 Indeterminate sub-path EC-3.4.011-6.
- Test #8 (`test_edit_type_unresolved_type_name_surfaces_typo_hint`): Unresolvable-name sub-path (200 response, name NOT in list) — exercises BC-3.4.011 unresolvable-name sub-path. `get_project_issue_types` returns 200 with a list that does NOT contain the `--type` value → typo hint, exit 1, `JRACLOUD-27893` absent, `jr api /rest/api/3/issue` absent.
- Test #9 (`test_edit_type_indeterminate_get_issue_fails_surfaces_raw_error`): R1 routing row — `edit_issue` 400, then `get_issue` returns 5xx → Indeterminate (detected by `is_err()` on the `get_issue` call; project-types never called) → exit nonzero, raw error on stderr, no hint, `JRACLOUD-27893` absent, `jr api /rest/api/3/issue` absent. Distinct wiremock topology from test #4 (R2): test #9 has `get_issue` fail; test #4 has `get_issue` succeed then project-types fail. Exercises EC-3.4.011-4.
- Test #10 (`test_edit_type_non_400_edit_error_surfaces_raw_error_no_enrichment`): R0b routing row — `edit_issue` returns e.g. HTTP 403 (a non-400 error) → exit nonzero, raw error on stderr, NEITHER the cross-hierarchy hint NOR the typo hint, `JRACLOUD-27893` absent, `jr api /rest/api/3/issue` absent. No enrichment fetch occurs (`get_issue` and `get_project_issue_types` mocks NOT mounted). Exercises BC-3.4.010 and BC-3.4.011 negative constraint: the enrichment block is entered ONLY on `status == 400`.

**Trace**: issue #388 F2; `src/cli/issue/create.rs::is_cross_hierarchy_type_error` (pure classifier, `SameCategory` and `Indeterminate` variants); `src/cli/issue/create.rs` inline `#[cfg(test)] mod is_cross_hierarchy_type_error_proptests` proptest for `is_cross_hierarchy_type_error`; `src/cli/issue/create.rs::handle_edit` (unresolvable name → typo hint; fetch-failure → `Indeterminate` caller dispatch); `tests/issue_edit_type_errors.rs` (integration — same-hierarchy, indeterminate, absent-subtask-flag, and unresolvable-name paths, tests #3–#8)

---

#### BC-3.4.012: `issue edit KEY` single-key success (table mode) echoes one stderr line per changed field in `field → value` format; resolved team name for `--team`; `(updated)` marker for description

**Confidence**: HIGH
**Source**: issue #398 F2 spec evolution; `src/cli/issue/create.rs::handle_edit` (single-key success path); `output::print_success` (existing stderr channel)
**Subject**: Issue write
**Behavior**: On the single-key `issue edit KEY` success path (PUT 204), AFTER printing `"Updated <key>"` to stderr via `output::print_success`, the handler emits one additional stderr line per field that was changed in this invocation. Format is `  <field> → <value>` (two leading spaces, unicode arrow). Fields and their echo values:

- `summary` → the literal string value passed to `--summary`
- `issue_type` → the literal string value passed to `--type`
- `priority` → the literal string value passed to `--priority`
- `parent`:
  - **`--parent <key>` branch** (`if let Some(parent_key) = parent`): `changed_fields` receives an insertion `"parent" → parent_key_string` at the `if let Some(parent_key) = parent` site.
  - **`--no-parent` branch** (`if no_parent`): `changed_fields` receives an insertion `"parent" → "(cleared)"` at the `if no_parent` site. Key is always `parent` in both cases; no separate `no_parent` key is ever inserted.
- `points`:
  - **`--points <n>` branch** (`if let Some(pts) = points`): `changed_fields` receives an insertion `"points" → pts.to_string()` at the `if let Some(pts) = points` site. The value is Rust's default `f64::to_string()` (e.g., `"5"` for `5.0`, `"2.5"` for `2.5`). This `.to_string()` formatting applies ONLY to this branch.
  - **`--no-points` branch** (`if no_points`): `changed_fields` receives an insertion `"points" → "(cleared)"` at the `if no_points` site. No numeric formatting applies here — the value is the literal string `"(cleared)"`. Key is always `points` in both cases; no separate `no_points` key is ever inserted.
- `team` → the RESOLVED team name (not the user's partial-match query, not the UUID); sourced from the third element of the updated `resolve_team_field` return tuple `(field_id, team_id, team_name)`. When `--team` value was passed as a raw UUID and the UUID-bypass path fires, `team_name` is the UUID itself (echo of the raw value the caller supplied). The UUID-bypass predicate (`is_team_uuid`) checks exactly 36 chars in 8-4-4-4-12 hyphen-separated groups of ASCII hex digits (case-insensitive). A team name that resembles a UUID but fails this predicate still resolves via partial-match.
- `description` → the literal marker `(updated)` — the content is an ADF blob and is NEVER echoed inline. This asymmetry is intentional: the `(updated)` marker tells the user that description changed without flooding the terminal. See the research rationale in `.factory/research/issue-398-field-echo-conventions.md §4` (table/human channel: marker; JSON channel: raw user-supplied input string).

Map keys are always the literal lowercase identifiers in the key table (`summary`, `issue_type`, `priority`, `parent`, `points`, `team`, `description`) — never `customfield_*` IDs. The issue-type key is the literal `issue_type` (matching the Rust field identifier), NOT `type` and NOT `issuetype`.

`--label` edits (single OR multi key) route through `handle_edit_bulk_labels` and are NOT covered by this contract; no `label` key appears in `changed_fields`.

Only fields that were actually changed in the invocation are echoed. The field-echo lines all go to **stderr** (Symmetric profile 4, same channel as the existing confirmation message). Stdout is empty (no JSON in table mode). Exit code 0.

**Scope**: Single-key `handle_edit` path ONLY. The bulk `handle_edit_bulk_fields` and `handle_edit_bulk_labels` paths are unaffected by this contract. Single-key means `effective_keys.len() == 1` after resolution — including a `--jql` query matching exactly one issue. Multi-key (2+ positional, or `--jql` matching 2+) routes to the bulk path and is out of scope.

**Preconditions**:
- `jr issue edit <key> [field flags...]` issued without `--output json` (table mode).
- At least one field flag is supplied. When no field flags are given, `handle_edit` bails with `"No fields specified to update..."` before reaching the PUT — exit 1, no echo fires.
- `--dry-run` is NOT set. `--dry-run` short-circuits before the PUT and emits its own planned-changes preview; the changed-fields echo of this contract does not fire on `--dry-run`.
- Single key (not a bulk path).
- When `--points` or `--no-points` is used, `story_points_field_id` must be configured; otherwise `handle_edit` errors via `resolve_story_points_field_id` (`JrError::ConfigError`, exit 1) before the PUT and the echo does not fire.
- PUT 204 received from Jira API.

**Postconditions**:
- Exit code 0.
- Stderr contains `"Updated <key>"` (via `output::print_success`).
- Stderr contains one `  <field> → <value>` line per changed field, in **alphabetical field-name order**, matching the JSON `changed_fields` BTreeMap key order. Both table-mode echo and JSON-mode `changed_fields` iterate the same `BTreeMap`, guaranteeing identical ordering.
- Stdout is empty.

**Invariants**:
1. The `team` echo value is the RESOLVED name, never a UUID or the user's raw partial-match query (unless the caller supplied a raw UUID, in which case the UUID is echoed). VP-398-001 verifies this invariant.
2. The `description` echo value is always exactly `(updated)`, never the content or a truncated preview. VP-398-002 verifies the asymmetry invariant.
3. The field-echo lines are on stderr, NOT stdout. They are not visible in `--output json` mode (which is governed by BC-3.4.013).
4. Points value uses Rust's default `.to_string()` for `f64` on the **`--points <n>` branch only** (`if let Some(pts) = points`). The `--no-points` branch inserts the literal string `"(cleared)"` — `.to_string()` is not involved. The snapshot test MUST pin both values.
5. All `changed_fields` keys are human-readable field names (never `customfield_*` IDs).
6. **Map construction vs emission timing**: the `changed_fields` BTreeMap MAY be constructed during field resolution (before the PUT), but it is EMITTED (table-mode stderr echo lines) ONLY AFTER `edit_result?` succeeds — i.e., after the PUT returns 204 and passes the BC-3.4.010/011 dual-gate error block. On a 400 or any other error response, the constructed map is discarded and never emitted. The echo lines in this contract are always post-PUT.

**Edge Cases**:
- EC-3.4.012-1: `--team` supplied as a UUID directly (UUID-pass-through path, predicate: 36-char 8-4-4-4-12 ASCII hex groups) → team echo shows the UUID (the raw caller-supplied value, since no name resolution occurred). A team name that resembles a UUID but does not satisfy the exact predicate (e.g., wrong length, non-hex char) still resolves via partial-match.
- EC-3.4.012-2: `--description` and `--description-stdin` are mutually exclusive (BC-3.4.007 clap conflict); whichever one is supplied populates the single `description` key in `changed_fields`. The table-mode echo always shows `  description → (updated)` regardless of which flag was used. The raw string is captured verbatim from the supplied source, including any trailing newline — no normalization is applied before the ADF conversion.
- EC-3.4.012-3: `--no-parent` → map key is `parent`, echo is `  parent → (cleared)`.
- EC-3.4.012-4: `--no-points` → map key is `points`, echo is `  points → (cleared)`.
- EC-3.4.012-5: `--points 5.0` → echo depends on Rust `f64::to_string()` (may produce `"5"` not `"5.0"`); pinned by snapshot test. Concrete assertions (NOT snapshot-only): `--points 5` → stderr contains `  points → 5`; `--points 2.5` → stderr contains `  points → 2.5`. Snapshot pins the full line; assertion pins the exact string to catch a wrong-but-stable snapshot value.
- EC-3.4.012-6: Multiple fields changed simultaneously → one echo line per changed field in **alphabetical field-name order** (BTreeMap iteration order), same ordering as JSON `changed_fields`.
- EC-3.4.012-7: No field flags supplied → `handle_edit` bails with exit 1 before PUT; this contract does not fire.
- EC-3.4.012-8: `--label` flag supplied → routes through `handle_edit_bulk_labels`; this contract does not fire.
- EC-3.4.012-9: `--dry-run` set → `handle_edit` emits planned-changes preview and exits; this contract does not fire.
- EC-3.4.012-10: `--team` triggers interactive disambiguation (`ExactMultiple` or `Ambiguous` match result, `--no-input` absent) → user selects a team from the prompt → the echoed team name is the SELECTED team's display name (not the original query string). The echoed name is the cached team's STORED display-name casing: `duplicates[selection].name` for the `ExactMultiple` path and `teams[idx].name` for the `Ambiguous` path — NOT the user's query-string casing.
- EC-3.4.012-11: `--points/--no-points` used when `story_points_field_id` is not configured → `resolve_story_points_field_id` errors with `JrError::ConfigError` (exit 1) before the PUT; the echo does not fire.
- EC-3.4.012-12: `--summary ""` (empty-string value) → echo is `  summary → ` with nothing after the arrow. This is correct behavior — the empty string is a valid value, not a rendering bug. Pinned by test `test_BC_3_4_012_empty_summary_echoes_empty_value` (integration test (wiremock) — `handle_edit` needs a wiremock PUT 204, so this MUST be an integration test; it cannot be a unit test). Note: this is a wiremock-only test scenario — real Jira rejects an empty `summary` with HTTP 400 (`summary` is a system-required field), so the success-path echo is not reachable against live Jira; the test exercises the echo formatting via a mocked 204 response only.
- EC-3.4.012-13: `jr issue edit KEY --description "x" --summary "y"` → stderr emits, in alphabetical field-name order: `  description → (updated)` first, then `  summary → y` second. This pins that the `description` marker participates in the same BTreeMap alphabetical sort as all other keys — it is NOT moved to the end, and the `(updated)` literal is the value used in the sort position for `description`.
- EC-3.4.012-14: `jr issue edit KEY --markdown --description "**bold**"` → table-mode echo is still `  description → (updated)` regardless of `--markdown`. The Markdown content is never surfaced in table mode; the `(updated)` marker applies uniformly to all description-change paths.
- EC-3.4.012-15: `--team` value matches no team at all (`MatchResult::None(_)`) → `resolve_team_field` errors via `JrError::UserError` before the PUT (exit code per `src/error.rs::exit_code()`, currently 64); no team echo line is emitted and the changed-fields echo does not fire. The error text contains the stable substring `No team matching` (exact wording varies by `fetched_fresh` cache state; assert only the substring). Note: the `None` variant carries a `Vec<String>` of candidate names, unused by this contract.
- EC-3.4.012-16: `jr issue edit KEY --description-stdin < /dev/null` → `desc_text = Some("")`. The edit proceeds — `--description-stdin` is itself a field flag so the no-fields-specified bail (the `has_any_field_change` guard, the pre-HTTP guard at `create.rs:341`) does not fire regardless of stdin content; an empty description is a valid change. (Note: there are two distinct no-fields guards in `handle_edit` — `has_any_field_change` at line 341 bails before any HTTP/JQL, and `has_updates` at line 821 bails inside the field-resolution block. The bail described in this EC is the FORMER — `has_any_field_change` — because `--description-stdin` is an unconditional flag predicate in that `let` binding.) Table-mode echo is `  description → (updated)` (same as any non-empty description). The empty description string is still converted to ADF for the PUT body. Exit code 0.

**Verification Properties**:
- VP-398-001: Resolved team name in `edit` table output is the display name, not a UUID substring. Negative case (DECISION LOCKED — round 5 F-1): write a **direct unit-level assertion on `is_team_uuid`** — call `is_team_uuid("36885b3c-1bf0-4f85-a357-c5b858c31de")` (35 chars, one short of UUID length) and assert the return value is `false`. Reuse or cite the existing `is_team_uuid_rejects_wrong_length` test at `src/cli/issue/helpers.rs` (~line 617). Do NOT write an integration test routing this probe through `partial_match` — that tests `partial_match` fallback behavior, not the `is_team_uuid` predicate boundary. **PLACEMENT (DECISION LOCKED — round 7 F-1): `is_team_uuid` has no `pub` visibility — it is module-private. The `is_team_uuid` negative-case assertion is a UNIT test that MUST be placed in the `#[cfg(test)] mod tests` block inside `src/cli/issue/helpers.rs` (because `is_team_uuid` is module-private and not exported via lib.rs). Do NOT place it in `tests/`. The team-echo positive cases (verifying that a resolved display name, not a UUID, appears in stderr or JSON) remain wiremock integration tests in `tests/`.**
- VP-398-002: Description echo is exactly `(updated)` in table output (not a content preview, not a length, not empty).
- VP-398-004: `--no-parent` produces exactly one `changed_fields` key named `parent` with value `(cleared)` — no `no_parent` key is ever present; identically for `--no-points` → key `points` value `(cleared)`, no `no_points` key. This is verified by asserting the JSON `changed_fields` object (in `--output json` mode) contains exactly the key `parent` (not `no_parent`) with value `"(cleared)"` when `--no-parent` is used, and contains exactly the key `points` (not `no_points`) with value `"(cleared)"` when `--no-points` is used. The table-mode echo uses the same keys (`parent →`, `points →`), verified by asserting stderr does not contain `no_parent` or `no_points` as field labels.

**Trace**: issue #398 F2; `src/cli/issue/create.rs::handle_edit`; `src/cli/issue/helpers.rs::resolve_team_field` (signature change to return 3-tuple; `is_team_uuid` predicate: 36-char, 8-4-4-4-12 ASCII hex groups, case-insensitive); `.factory/research/issue-398-field-echo-conventions.md`; `.factory/phase-f2-spec-evolution/prd-delta-398.md §2`

[NEW 2026-05-21 issue #398 F2]
[UPDATED 2026-05-21 adversarial review round 1: C-2 no-flags is pre-PUT exit-1; M-1 --label exclusion; MED-1 single-key cleared-field model; MED-2 BTreeMap/alphabetical ordering noted; MED-3 --dry-run precondition; MED-4 --jql single-match scope; MIN-2 UUID predicate pinned]
[UPDATED 2026-05-21 adversarial review round 2: F-2 alphabetical ordering pinned in postconditions+EC-6; F-2 stdin verbatim capture clarified in EC-2; F-3 points precondition added EC-11; F-8 interactive disambiguation EC-10; F-9 VP-398-001 negative case rewritten; F-10 key naming clarified; F-13 empty-string EC-12]
[UPDATED 2026-05-21 adversarial review round 3: MED-1 EC-13 added (concrete --description+--summary alphabetical ordering pin with description marker in sort); MED-2 EC-14 added (--markdown table mode still shows (updated) marker); M-1 plain-text reference in description field corrected to raw-user-supplied-input-string]
[UPDATED 2026-05-21 adversarial review round 4: F-2 EC-3.4.012-10 stored-casing clause added (duplicates[selection].name / teams[idx].name, NOT query-string casing); F-3 VP-398-001 fixture constraint + No-team-matching substring assertion]
[UPDATED 2026-05-21 adversarial review round 5: F-1 VP-398-001 negative case rewritten as direct unit-level is_team_uuid assertion (cite is_team_uuid_rejects_wrong_length); F-3 EC-3.4.012-15 added (MatchResult::None → JrError::UserError exit 64, no echo)]
[UPDATED 2026-05-21 adversarial review round 7: F-1 VP-398-001 + explicit module-private placement sentence (UNIT test in helpers.rs #[cfg(test)] block, NOT tests/); F-2 EC-3.4.012-12 test name pinned; F-4 VP-398-004 added (cleared-field single-key model); F-5 EC-3.4.012-2 reworded (clap conflict, not co-occurrence)]
[UPDATED 2026-05-21 adversarial review round 8: MAJOR-1 points/parent bullet split into two-site insertion enumeration; invariant 4 f64 .to_string() scoped to --points branch only; OBS-2 concrete assertion values added to EC-3.4.012-5; OBS-4 EC-3.4.012-12 pinned as integration test (wiremock); IMP-3 EC-3.4.012-16 added (empty-stdin edge case)]
[UPDATED 2026-05-21 adversarial review round 9: IMPORTANT-1 EC-3.4.012-12 wiremock-only note added (real Jira rejects empty summary with HTTP 400)]
[UPDATED 2026-05-21 adversarial review round 10: IMPORTANT-3 invariant 6 added (map construction vs emission timing — map discarded on PUT error, emitted only post-204); IMPORTANT-2 EC-3.4.012-16 has_any_field_change replaced with has_updates]
[UPDATED 2026-05-21 adversarial review round 12: EC-3.4.012-16 reverted to `has_any_field_change` — the round-10 rename to `has_updates` was an over-correction; `has_any_field_change` (create.rs:341) is the pre-HTTP no-fields guard the EC reasons about]

---

#### BC-3.4.013: `issue edit KEY` single-key success (JSON mode) includes `changed_fields` object in `edit_response`; `updated: true` retained; description carries the RAW user-supplied input string

**Confidence**: HIGH
**Source**: issue #398 F2 spec evolution; `src/cli/issue/json_output.rs::edit_response` (signature change); `src/cli/issue/create.rs::handle_edit` (field-resolution block where `desc_text` is captured as the raw user input — `src/adf.rs` ADF→text converter is NOT used for this field)
**Subject**: Issue write
**Behavior**: On the single-key `jr issue edit KEY --output json` success path (PUT 204), the JSON payload on stdout is extended from the prior `{"key": "<key>", "updated": true}` shape to include a `changed_fields` object:

```json
{
  "key": "<key>",
  "updated": true,
  "changed_fields": {
    "<field_name>": "<string_value>"
  }
}
```

`"updated": true` is RETAINED for backward compatibility. Downstream consumers using `.key` or `.updated` in `jq` expressions are unaffected.

`changed_fields` maps literal lowercase field identifiers to JSON string values (never `customfield_*` IDs). JSON key order is deterministic (alphabetical) because `edit_response` uses `BTreeMap<String, String>` internally. All values are JSON strings, including numeric fields (e.g., `"5"` not `5`). The issue-type key is the literal `"issue_type"` (matching the Rust field identifier), NOT `"type"` and NOT `"issuetype"`. Keys and value semantics:

| Key | Value |
|-----|-------|
| `"description"` | The **raw user-supplied input string** from `--description` or `--description-stdin`. NOT the `(updated)` marker. NOT an ADF→text round-trip. The raw string is lossless — it is exactly what the caller sent, before any `markdown_to_adf` conversion. |
| `"issue_type"` | Verbatim string passed to `--type` |
| `"parent"` | **`--parent <key>` branch** (`if let Some(parent_key) = parent`): `changed_fields` receives insertion `"parent" → parent_key_string` at the `if let Some(parent_key) = parent` site. **`--no-parent` branch** (`if no_parent`): `changed_fields` receives insertion `"parent" → "(cleared)"` at the `if no_parent` site. Key is always `"parent"` in both cases; no separate `"no_parent"` key is ever inserted. |
| `"points"` | **`--points <n>` branch** (`if let Some(pts) = points`): `changed_fields` receives insertion `"points" → pts.to_string()` at the `if let Some(pts) = points` site. Value is Rust's default `f64::to_string()` (e.g., `"5"` for `5.0`, `"2.5"` for `2.5`). This `.to_string()` formatting applies ONLY to this branch. **`--no-points` branch** (`if no_points`): `changed_fields` receives insertion `"points" → "(cleared)"` at the `if no_points` site — no numeric formatting. Key is always `"points"` in both cases; no separate `"no_points"` key. |
| `"priority"` | Verbatim string passed to `--priority` |
| `"summary"` | Verbatim string passed to `--summary` |
| `"team"` | RESOLVED team display name (not UUID, not partial-match query); from the `team_name` element of the updated `resolve_team_field` return tuple |

`--label` edits (single OR multi key) route through `handle_edit_bulk_labels` and are NOT covered by this contract; no `"label"` key appears in `changed_fields`.

The deliberate asymmetry between BC-3.4.012 (table: `(updated)` marker for description) and BC-3.4.013 (JSON: raw input string for description) is intentional: the human channel optimizes for scannability; the machine channel must be complete and faithful. This asymmetry MUST NOT be "fixed" to make them match. A CLAUDE.md Gotcha entry should accompany the implementation.

`changed_fields` contains only the fields that were changed in this invocation (same map construction as BC-3.4.012). The JSON output is on stdout. No stderr output in JSON mode (Symmetric profile 4). Exit code 0.

`edit_response` signature changes to: `pub(crate) fn edit_response(key: &str, changed_fields: &BTreeMap<String, String>) -> Value`. The `BTreeMap` is passed from `handle_edit` after it is constructed during field resolution. Alphabetical key order within `changed_fields` is guaranteed by `BTreeMap`. The top-level object key order (the relative position of `"key"`, `"updated"`, and `"changed_fields"`) is determined by `serde_json::Map`'s default alphabetical key ordering (`preserve_order` feature is NOT enabled in this crate — confirmed in Cargo.toml). The top-level keys `changed_fields`, `key`, `updated` are already in alphabetical order, so the pinned snapshot body is `{"changed_fields": {...}, "key": "TEST-1", "updated": true}` regardless of the order they are written in the `json!{}` literal. The top-level key order is NOT contractually pinned beyond whatever the regenerated insta snapshot records; only the INNER `changed_fields` key order is contractually alphabetical.

**Preconditions**:
- `jr issue edit <key> [field flags...] --output json` issued.
- At least one field flag is supplied. When no field flags are given, `handle_edit` bails with `"No fields specified to update..."` before reaching the PUT — exit 1, no JSON emitted.
- `--dry-run` is NOT set. `--dry-run` short-circuits before the PUT and emits its own planned-changes preview; the changed-fields echo of this contract does not fire on `--dry-run`.
- Single key (not a bulk path). Single-key means `effective_keys.len() == 1` after resolution — including a `--jql` query matching exactly one issue. Multi-key (2+ positional, or `--jql` matching 2+) routes to the bulk path and is out of scope.
- When `--points` or `--no-points` is used, `story_points_field_id` must be configured; otherwise `handle_edit` errors via `resolve_story_points_field_id` (`JrError::ConfigError`, exit 1) before the PUT and no JSON is emitted.
- PUT 204 received from Jira API.

**Postconditions**:
- Exit code 0.
- Stdout is valid JSON with keys: `"key"` (string), `"updated"` (boolean `true`), `"changed_fields"` (object with string values in alphabetical key order).
- `"updated": true` is present (backward-compat invariant).
- `changed_fields["team"]` is the resolved display name, never a UUID (unless the caller supplied a raw UUID directly).
- `changed_fields["description"]` is the raw user-supplied input string, never `"(updated)"`.
- Stderr is empty.

**Invariants**:
1. `"updated": true` MUST remain in the payload. Its removal is a breaking change. VP-398-003 verifies this invariant.
2. `changed_fields["description"]` MUST be the raw user input string (lossless; no ADF→text round-trip). VP-398-002 verifies the asymmetry holds (JSON gets raw string; table gets `(updated)` marker).
3. `changed_fields["team"]` MUST be the resolved display name. VP-398-001 verifies.
4. `changed_fields` JSON key order is alphabetical (guaranteed by `BTreeMap`). The insta snapshot `jr__cli__issue__json_output__tests__edit.snap` MUST be updated to reflect the new shape. The `test_edit` unit test in `src/cli/issue/json_output.rs` MUST be updated to pass a non-empty `BTreeMap` for `changed_fields` — specifically `BTreeMap` with `"summary" → "New title"`. **Pinned expected regenerated snapshot body (DECISION LOCKED — round 10 MAJOR-1)**: the regenerated snapshot content MUST be exactly `{"changed_fields": {"summary": "New title"}, "key": "TEST-1", "updated": true}` (with `changed_fields` before `key` before `updated`). The top-level key order is alphabetical because `serde_json::Map` serializes keys in alphabetical order by default — the `preserve_order` feature is NOT enabled in this crate (confirmed in Cargo.toml). The top-level keys `changed_fields`, `key`, `updated` are already in alphabetical order, so the pinned snapshot body is correct regardless of the order they are written in the `json!{}` literal. Additionally, a new test `test_edit_response_empty_changed_fields` MUST be added (applying the new-test `test_<verb>_<subject>_<expected_outcome>` naming convention): this test calls `edit_response` with an empty `BTreeMap<String, String>` and asserts the resulting JSON has `"updated": true` and `"changed_fields": {}`. It does NOT use an insta snapshot (see VP-398-003 snapshot test split). **Top-level key order note**: the top-level `edit_response` object key order follows `serde_json::Map`'s default alphabetical key ordering (`preserve_order` NOT enabled) and is NOT contractually pinned beyond whatever the regenerated snapshot records. Only the INNER `changed_fields` key order is contractually alphabetical.
5. All `changed_fields` keys are the literal lowercase identifiers (`summary`, `issue_type`, `priority`, `parent`, `points`, `team`, `description`) — never `customfield_*` IDs. The issue-type key is the literal `issue_type` (matching the Rust field identifier), NOT `type` and NOT `issuetype`.
6. **Map construction vs emission timing**: the `changed_fields` BTreeMap MAY be constructed during field resolution (before the PUT), but it is EMITTED (included in the JSON payload on stdout) ONLY AFTER `edit_result?` succeeds — i.e., after the PUT returns 204 and passes the BC-3.4.010/011 dual-gate error block. On a 400 or any other error response, the constructed map is discarded and the JSON payload of this contract is never written to stdout.

**Edge Cases**:
- EC-3.4.013-1: No field flags supplied → `handle_edit` bails with exit 1 before PUT; no JSON emitted.
- EC-3.4.013-2: `--team` value was a raw UUID (UUID-bypass path) → `changed_fields["team"]` is the UUID (the raw value supplied, since no name lookup occurred).
- EC-3.4.013-3: `--description` and `--description-stdin` are mutually exclusive (BC-3.4.007 clap conflict); whichever one is supplied populates the single `description` key. When `--description-stdin` is used, `changed_fields["description"]` is the raw piped content string (same lossless path as `--description`). The raw string is captured verbatim as read from stdin, including any trailing newline — no trailing-newline normalization is applied.
- EC-3.4.013-4: `--no-parent` set → `changed_fields["parent"] = "(cleared)"`. No separate `"no_parent"` key.
- EC-3.4.013-5: `--no-points` set → `changed_fields["points"] = "(cleared)"`. No separate `"no_points"` key.
- EC-3.4.013-6: `--label` flag supplied → routes through `handle_edit_bulk_labels`; this contract does not fire.
- EC-3.4.013-7: `--dry-run` set → `handle_edit` emits planned-changes preview and exits; this contract does not fire.
- EC-3.4.013-8: `--team` triggers interactive disambiguation (`ExactMultiple` or `Ambiguous` match result, `--no-input` absent) → user selects a team from the prompt → `changed_fields["team"]` is the SELECTED team's display name (not the original query string). The echoed name is the cached team's STORED display-name casing: `duplicates[selection].name` for the `ExactMultiple` path and `teams[idx].name` for the `Ambiguous` path — NOT the user's query-string casing.
- EC-3.4.013-9: `--points/--no-points` used when `story_points_field_id` is not configured → `resolve_story_points_field_id` errors with `JrError::ConfigError` (exit 1) before the PUT; no JSON is emitted.
- EC-3.4.013-10: `--summary ""` (empty-string value) → `changed_fields["summary"] = ""`. The empty string is a valid value; the key is present in the output. Pinned by test `test_BC_3_4_013_empty_summary_in_changed_fields` (asserting the JSON `changed_fields` object contains `"summary": ""` — the key is present with an empty string value, not absent). Note: this is a wiremock-only test scenario — real Jira rejects an empty `summary` with HTTP 400 (`summary` is a system-required field), so the success-path echo is not reachable against live Jira; the test exercises the echo formatting via a mocked 204 response only.
- EC-3.4.013-11: `jr issue edit KEY --markdown --description "**bold**"` → `changed_fields["description"]` is the literal raw string `**bold**` (raw Markdown), NOT ADF JSON and NOT plain-text-rendered. The `--markdown` flag causes `markdown_to_adf("**bold**")` to be invoked for the PUT body sent to Jira, but the raw input string `"**bold**"` is captured BEFORE that conversion and stored in `changed_fields`. The `src/adf.rs` converter is not involved in populating `changed_fields["description"]` in any way.
- EC-3.4.013-12: `--team` value matches no team at all (`MatchResult::None(_)`) → `resolve_team_field` errors via `JrError::UserError` before the PUT (exit code per `src/error.rs::exit_code()`, currently 64); no JSON is emitted and the changed-fields echo does not fire. The error text contains the stable substring `No team matching` (exact wording varies by `fetched_fresh` cache state; assert only the substring). Note: the `None` variant carries a `Vec<String>` of candidate names, unused by this contract.
- EC-3.4.013-13: `jr issue edit KEY --description-stdin < /dev/null` → `desc_text = Some("")`. The edit proceeds — `--description-stdin` is itself a field flag so the no-fields-specified bail (the `has_any_field_change` guard, the pre-HTTP guard at `create.rs:341`) does not fire regardless of stdin content; an empty description is a valid change. (Note: there are two distinct no-fields guards in `handle_edit` — `has_any_field_change` at line 341 bails before any HTTP/JQL, and `has_updates` at line 821 bails inside the field-resolution block. The bail described in this EC is the FORMER — `has_any_field_change` — because `--description-stdin` is an unconditional flag predicate in that `let` binding.) JSON output: `changed_fields["description"]` is `""` (empty string). The `"description"` key IS present in `changed_fields`. Exit code 0.

**Verification Properties**:
- VP-398-001: Resolved team name in `edit` JSON `changed_fields.team` is the display name, not a UUID substring. Negative case (DECISION LOCKED — round 5 F-1): write a **direct unit-level assertion on `is_team_uuid`** — call `is_team_uuid("36885b3c-1bf0-4f85-a357-c5b858c31de")` (35 chars, one short of UUID length) and assert the return value is `false`. Reuse or cite the existing `is_team_uuid_rejects_wrong_length` test at `src/cli/issue/helpers.rs` (~line 617). Do NOT write an integration test routing this probe through `partial_match` — that tests `partial_match` fallback behavior, not the `is_team_uuid` predicate boundary. **PLACEMENT (DECISION LOCKED — round 7 F-1): `is_team_uuid` has no `pub` visibility — it is module-private. The `is_team_uuid` negative-case assertion is a UNIT test that MUST be placed in the `#[cfg(test)] mod tests` block inside `src/cli/issue/helpers.rs` (because `is_team_uuid` is module-private and not exported via lib.rs). Do NOT place it in `tests/`. The team-echo positive cases (verifying that a resolved display name, not a UUID, appears in JSON `changed_fields.team`) remain wiremock integration tests in `tests/`.**
- VP-398-002: `changed_fields.description` in JSON output is NOT `"(updated)"` (it is the raw user input string). In table output, description echo IS `(updated)` (asymmetry pinned by two separate assertions). **Sub-case — stdin trailing-newline not normalized**: When `--description-stdin` is used and the piped content ends with a trailing newline, `changed_fields["description"]` MUST be exactly `"My description\n"` — the trailing `\n` must be present and must not be silently stripped. Test: `printf 'My description\n' | jr issue edit KEY --description-stdin --output json`; parse JSON; assert `changed_fields.description == "My description\n"` (not `"My description"`). Suggested test name: `test_BC_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields`. Applies to BC-3.4.013 (JSON mode); table mode always shows `(updated)` regardless of content.
- VP-398-003: `"updated": true` is present in `edit_response` JSON payload (backward-compat invariant). Test strategy: pass a single-field edit (e.g., `--summary "New title"`) in `--output json` mode; parse JSON; assert `output["updated"] == true` and `output["changed_fields"]` is non-empty. Also assert `"updated": true` in the updated insta snapshot. **Snapshot test split (DECISION LOCKED — round 7 F-3; see also invariant 4 above)**: the existing `test_edit` MUST be updated to pass a non-empty `BTreeMap`; the NEW `test_edit_response_empty_changed_fields` test covers the empty-map case and asserts `"updated": true` AND `"changed_fields": {}` directly (no snapshot). Both tests together verify that `"updated": true` is always present regardless of whether `changed_fields` is empty or non-empty.
- VP-398-004: `--no-parent` produces exactly one `changed_fields` key named `parent` with value `(cleared)` — no `no_parent` key is ever present; identically for `--no-points` → key `points` value `(cleared)`, no `no_points` key. Assert: `changed_fields` in JSON output contains `"parent": "(cleared)"` (not `"no_parent"`) when `--no-parent` is used; and `"points": "(cleared)"` (not `"no_points"`) when `--no-points` is used.

**Trace**: issue #398 F2; `src/cli/issue/json_output.rs::edit_response`; `.factory/research/issue-398-field-echo-conventions.md §4`; `.factory/phase-f2-spec-evolution/prd-delta-398.md §2`

[NEW 2026-05-21 issue #398 F2]
[UPDATED 2026-05-21 adversarial review round 1: C-2 no-flags is pre-PUT exit-1; M-1 --label exclusion; M-2 description is raw input string not ADF→text; MED-1 single-key cleared-field model (parent/points); MED-2 BTreeMap alphabetical ordering; MED-3 --dry-run precondition; MED-4 --jql single-match scope]
[UPDATED 2026-05-21 adversarial review round 2: F-2 stdin verbatim capture clarified in EC-3; F-3 points precondition added EC-9; F-8 interactive disambiguation EC-8; F-9 VP-398-001 negative case rewritten; F-10 key naming clarified; F-13 empty-string EC-10]
[UPDATED 2026-05-21 adversarial review round 3: MED-2 EC-11 added (--markdown --description raw Markdown string in changed_fields; src/adf.rs not used for changed_fields population)]
[UPDATED 2026-05-21 adversarial review round 4: F-2 EC-3.4.013-8 stored-casing clause added (duplicates[selection].name / teams[idx].name, NOT query-string casing); F-3 VP-398-001 fixture constraint + No-team-matching substring assertion]
[UPDATED 2026-05-21 adversarial review round 5: F-1 VP-398-001 negative case rewritten as direct unit-level is_team_uuid assertion (cite is_team_uuid_rejects_wrong_length); F-3 EC-3.4.013-12 added (MatchResult::None → JrError::UserError exit 64, no JSON emitted)]
[UPDATED 2026-05-21 adversarial review round 7: F-1 VP-398-001 + explicit module-private placement sentence (UNIT test in helpers.rs #[cfg(test)] block, NOT tests/); F-2 EC-3.4.013-10 test name pinned; F-4 VP-398-004 added (cleared-field single-key model); F-5 EC-3.4.013-3 reworded (clap conflict, not co-occurrence); F-6 VP-398-002 stdin trailing-newline sub-case added inline]
[UPDATED 2026-05-21 adversarial review round 8: MAJOR-1 parent/points table rows split into two-site insertion enumeration; f64 .to_string() scoped to --points branch only (not --no-points); MAJOR-2 invariant 4 + VP-398-003 body add test_edit_response_empty_changed_fields; IMP-3 EC-3.4.013-13 added (empty-stdin edge case, changed_fields["description"]=="")]
[UPDATED 2026-05-21 adversarial review round 9: IMPORTANT-1 EC-3.4.013-10 wiremock-only note added (real Jira rejects empty summary with HTTP 400)]
[UPDATED 2026-05-21 adversarial review round 10: MAJOR-1 invariant 4 pinned regenerated snapshot body ({"changed_fields": {"summary": "New title"}, "key": "TEST-1", "updated": true}); IMPORTANT-1 top-level key order note added to invariant 4 and signature paragraph; IMPORTANT-2 EC-3.4.013-13 has_any_field_change replaced with has_updates; IMPORTANT-3 invariant 6 added (map construction vs emission timing — map discarded on PUT error, emitted only post-204)]
[UPDATED 2026-05-21 adversarial review round 12: EC-3.4.013-13 reverted to `has_any_field_change` — the round-10 rename to `has_updates` was an over-correction; `has_any_field_change` (create.rs:341) is the pre-HTTP no-fields guard the EC reasons about]

---

#### BC-3.4.014: `issue create` table-mode success echoes ALL set fields to stderr (mirroring BC-3.4.012)

**Confidence**: HIGH
**Source**: issue #398 F2 spec evolution; `src/cli/issue/create.rs::handle_create` (table-mode success path); `src/cli/issue/helpers.rs::resolve_team_field` (signature change to return 3-tuple)
**Subject**: Issue write

> **[REVISED 2026-05-22 human-gate]** BC-3.4.014 broadened from team-only to all-set-fields echo to match BC-3.4.012. The sentence "Unlike `issue edit`, `issue create` echoes ONLY the resolved team name" is superseded and removed.

**Behavior**: On the `jr issue create` success path (table mode, no `--output json`), the existing two-line output:

```
Created issue FOO-123
https://example.atlassian.net/browse/FOO-123
```

gains one `  <field> → <value>` stderr line per field the create command set, appearing between the `"Created issue <key>"` confirmation and the browse URL:

```
Created issue FOO-123
  assignee → Jane Doe
  description → (updated)
  issue_type → Task
  label → bug, urgent
  parent → PROJ-5
  points → 5
  priority → High
  summary → Fix the login bug
  team → Platform Core
https://example.atlassian.net/browse/FOO-123
```

Field echo lines are sorted in **alphabetical field-name order** (matching BC-3.4.012). Only fields actually set by the caller appear — unset optional fields emit no line. Format is `  <field> → <value>` with two leading spaces and a unicode right arrow, identical to BC-3.4.012.

**Fields echoed and their table-mode values (create-path enumeration)**:

- `summary` → literal `--summary` value. Required field; always present on the platform path (post-resolve).
- `issue_type` → literal `--type` value. Required field; always present on the platform path.
- `description` → literal `(updated)` marker. Content is never echoed in table mode. Same asymmetry as BC-3.4.012. (`--description` or `--description-stdin`; either source shows the marker.)
- `priority` → literal `--priority` value.
- `label` → comma-separated list of label values (e.g., `bug, urgent` for `--label bug --label urgent`). If a single label is supplied, no trailing comma. If `--label` is absent, no echo line.
- `team` → RESOLVED display name (not UUID, not partial-match query). UUID-bypass: when the caller passes a raw UUID, the UUID is echoed as-is (no lookup occurred). Uses the third element from `resolve_team_field`'s `(field_id, team_id, team_name)` return tuple.
- `points` → `f64::to_string()` result (e.g., `"5"` for 5.0, `"2.5"` for 2.5).
- `parent` → issue key string from `--parent` (e.g., `PROJ-5`).
- `assignee` → display name of the resolved assignee. Sourced from `resolve_assignee_by_project`'s second return element `_display_name` (currently unused — must be bound and used for echo). When `--account-id` is used instead of `--to`, the account ID is echoed as the value (no display name lookup occurs on the `--account-id` path).

**Fields NOT echoed**:
- `project` — implicit/required; not echoed (same decision as BC-3.4.012 which does not echo the issue key).
- `--request-type` path fields — the JSM path is governed by BC-3.8.011; this contract applies to the platform path only.
- `--label` on create is the platform single-POST path (NOT the bulk path used by `edit --label`). Because all labels are present in the create POST body, echoing them as a comma-joined list is feasible and IS implemented. There is no `label` key exclusion on create (contrast with BC-3.4.012 which explicitly excludes `label` because `edit --label` routes through `handle_edit_bulk_labels`).

**JSON mode is UNCHANGED**: `issue create --output json` already performs a follow-up GET returning the full created issue object — a superset of the edit `changed_fields`. No `changed_fields` key is added to create JSON output; the JSON path is byte-for-byte identical to pre-#398 behavior. The full issue object is richer than `changed_fields` would be, making a `changed_fields` addition redundant.

**Output channel profile**: All output lines (`Created issue <key>`, field echo lines, browse URL) are emitted to **stderr**. Stdout is empty in table mode. The browse URL was already on stderr pre-#398 (via `eprintln!`). This is **output channel profile 4 (Symmetric)**: stdout is empty in table mode; in `--output json` mode stdout carries the full JSON payload while stderr is empty. Profile-4 carve-out: success confirmation lines on stderr is pre-existing behavior, not an error path. #398 only inserts field-echo lines into the same pre-existing stderr stream.

**Preconditions**:
- `jr issue create [flags...]` issued without `--output json`.
- The `--request-type` flag is absent (platform create path; JSM path is governed by BC-3.8.011).
- All field resolution succeeds (team, assignee, story-points field ID).
- POST 201 received; `issueKey` extracted.

**Postconditions**:
- Exit code 0.
- Stderr contains `"Created issue <key>"` (via `output::print_success`).
- Stderr contains one `  <field> → <value>` line per field set, in alphabetical field-name order, between the "Created issue" line and the browse URL.
- Stderr contains the browse URL.
- Stdout is empty.

**Invariants**:
1. The `team` echo value is the RESOLVED display name, never a UUID (unless the caller supplied a UUID directly). VP-398-001 covers `edit` and `create` table-mode team echo.
2. The `description` echo value is always `(updated)` — never the content, never truncated. Same asymmetry as BC-3.4.012.
3. Field echo lines appear between the "Created issue" confirmation and the browse URL — never after the browse URL.
4. When no optional flags are set (only required `--summary` and `--type` supplied), the minimal echo contains only `issue_type` and `summary` lines.
5. The `label` echo is a comma-separated join of the labels Vec, in the order they appear on the command line (no re-sorting of the labels themselves; only the field-key `label` is alphabetically sorted relative to other field keys).
6. The echo map is constructed alongside field-building; it is discarded if the POST fails. Field echo lines are emitted only post-201.

**Edge Cases**:
- EC-3.4.014-1: `--team` supplied as a UUID directly → team echo shows the UUID (UUID-bypass path; no name resolution occurred).
- EC-3.4.014-2: `--team` triggers disambiguation prompt (interactive, `--no-input` absent) → user selects a team → resolved name is echoed.
- EC-3.4.014-3: `--no-input` with an ambiguous team name → `resolve_team_field` errors via `JrError::UserError` before POST (exit code per `src/error.rs::exit_code()`, currently 64); no echo emitted.
- EC-3.4.014-4: JSM create path (`--request-type` set) → this BC does NOT apply; the team warning is governed by BC-3.8.011 (`--team` is ignored on JSM path). None of the create field echo lines fire on the JSM path.
- EC-3.4.014-5: `--team` value matches no team at all (`MatchResult::None(_)`) → `resolve_team_field` errors via `JrError::UserError` before POST (exit code per `src/error.rs::exit_code()`, currently 64); no echo emitted and the create does not proceed. The error text contains the stable substring `No team matching` (exact wording varies by `fetched_fresh` cache state; assert only the substring). Note: the `None` variant carries a `Vec<String>` of candidate names, unused by this contract.
- EC-3.4.014-6: `--label` absent → no `label` echo line emitted.
- EC-3.4.014-7: `--to me` → assignee resolves via `get_myself()`; display name from the myself response is echoed as `assignee → <display_name>`.
- EC-3.4.014-8: `--account-id <id>` used instead of `--to` → `assignee → <account_id>` (the account ID is echoed; no display-name lookup is performed on the `--account-id` path, consistent with existing `jr issue assign --account-id` behavior).
- EC-3.4.014-9: `--label bug --label urgent` → `label → bug, urgent` (comma-space separated).
- EC-3.4.014-10: Only `--summary` and `--type` set → echo contains `issue_type` and `summary` lines only; output byte-for-byte identical to `BC-3.4.012` equivalent when only those two fields are set.
- EC-3.4.014-11: `--points 5.0` → echo depends on Rust `f64::to_string()` (may produce `"5"` not `"5.0"`); pinned by snapshot test. Concrete assertions (NOT snapshot-only): `jr issue create ... --points 5` → stderr contains `  points → 5`; `jr issue create ... --points 2.5` → stderr contains `  points → 2.5`. Snapshot pins the full line; assertion pins the exact string to catch a wrong-but-stable snapshot value. (Mirrors EC-3.4.012-5.)
- EC-3.4.014-12: `jr issue create ... --summary ""` (empty-string value) → echo line is `  summary → ` with nothing after the arrow. This is correct rendering — the empty string is a valid value, not a rendering bug. Note: this is a wiremock-only test scenario — real Jira rejects an empty `summary` with HTTP 400 (`summary` is a system-required field), so the success-path echo is not reachable against live Jira; the test exercises the echo formatting via a mocked 201 response only. (Mirrors EC-3.4.012-12; clap accepts `--summary ""` even though the field is required by the API.)
- EC-3.4.014-13: `--points` used when `story_points_field_id` is not configured → `handle_create` errors via `resolve_story_points_field_id` with `JrError::ConfigError` (exit 1) before the POST; no echo fires. (Mirrors EC-3.4.012-11.)

**Verification Properties**:
- VP-398-001: Resolved team name in `create` table output is the display name, not a UUID substring (shared VP with BC-3.4.012 and BC-3.4.013). Negative case (DECISION LOCKED — round 5 F-1): write a **direct unit-level assertion on `is_team_uuid`** — call `is_team_uuid("36885b3c-1bf0-4f85-a357-c5b858c31de")` (35 chars, one short of UUID length) and assert the return value is `false`. Reuse or cite the existing `is_team_uuid_rejects_wrong_length` test at `src/cli/issue/helpers.rs` (~line 617). Do NOT write an integration test routing this probe through `partial_match`. **PLACEMENT (DECISION LOCKED — round 7 F-1): `is_team_uuid` has no `pub` visibility — it is module-private. The `is_team_uuid` negative-case assertion is a UNIT test that MUST be placed in the `#[cfg(test)] mod tests` block inside `src/cli/issue/helpers.rs`. Do NOT place it in `tests/`.** The team-echo positive cases remain wiremock integration tests in `tests/`.
- VP-398-005: Broadened to cover all-fields create echo. Integration test (wiremock) verifies: (a) `jr issue create --team <unresolvable_name> --no-input` exits 64, no POST issued; (b) `jr issue create --summary X --type Task --priority High --team "Platform Core"` in table mode emits `  priority → High` and `  team → Platform Core` on stderr (alphabetical order) between "Created issue" and browse URL. Suggested test names: `test_BC_3_4_014_create_unresolvable_team_no_input_exits_64`, `test_BC_3_4_014_create_all_fields_echo_alphabetical_order`. See verification-delta-398.md §VP-398-005 for full test strategy.
- VP-398-006 (NEW): Create `description` echo is `(updated)` marker (table mode) — never the content. Integration test: `jr issue create --summary X --type Task --description "Some content"` in table mode emits `  description → (updated)` on stderr, does NOT contain `"Some content"`. Suggested test name: `test_BC_3_4_014_create_description_echo_is_updated_marker`.

**Trace**: issue #398 F2; `src/cli/issue/create.rs::handle_create`; `src/cli/issue/helpers.rs::resolve_team_field`; `.factory/phase-f2-spec-evolution/prd-delta-398.md §2`; human-gate decision 2026-05-22

[NEW 2026-05-21 issue #398 F2]
[UPDATED 2026-05-21 adversarial review round 1: MIN-3 Trace repointed to prd-delta-398.md §2 (locked decisions)]
[UPDATED 2026-05-21 adversarial review round 2: F-7 output channel profile explicit (all three lines to stderr; stdout empty)]
[UPDATED 2026-05-21 adversarial review round 3: COS-1 H1 title drops erroneous KEY token; MED-4 output channel profile reclassified from profile 5 (No-log facade) to profile 4 (Symmetric)]
[UPDATED 2026-05-21 adversarial review round 4: F-1 profile-4 carve-out paragraph added; F-3 VP-398-001 fixture constraint + No-team-matching substring assertion; O-2 EC-3.4.014-3 exit code pinned to 64]
[UPDATED 2026-05-21 adversarial review round 5: F-1 VP-398-001 negative case rewritten as direct unit-level is_team_uuid assertion; F-3 EC-3.4.014-5 added]
[UPDATED 2026-05-21 adversarial review round 7: F-1 VP-398-001 + explicit module-private placement sentence]
[UPDATED 2026-05-21 adversarial review round 8: IMP-5 EC-3.4.014-3/5 wording softened; VP-398-005 added]
[REVISED 2026-05-22 human-gate: BC-3.4.014 broadened from team-only echo to ALL set fields echo, mirroring BC-3.4.012; label/assignee decisions documented; EC-3.4.014-6..10 added; VP-398-006 added; JSON-mode note added; obsolete "ONLY --team" scope sentence removed]
[UPDATED 2026-05-22 re-convergence pass 1-3: EC-3.4.014-11 added (--points f64::to_string() format assertions, mirrors EC-3.4.012-5); EC-3.4.014-12 added (empty-string --summary echo, mirrors EC-3.4.012-12); EC-3.4.014-13 added (--points without story_points_field_id configured → ConfigError exit 1, mirrors EC-3.4.012-11)]

---

#### BC-3.4.015: `issue edit KEY --field NAME=VALUE` (string/number/date/datetime/user field, single-key path) — resolves field name, validates against editmeta, serializes per type, PUTs; success echoes field in `changed_fields`

**Confidence**: HIGH
**Source**: issue #396 F2 spec evolution; `src/cli/issue/create.rs::handle_edit` (single-key success path, extended); `src/api/jira/issues.rs::get_editmeta` (new); `src/cli/issue/helpers.rs::resolve_edit_fields` (new, owns field-lookup and ambiguity handling); `.factory/research/issue-396-jsm-fields-validation.md`
**Subject**: Issue write

**Description**: On the single-key `issue edit KEY --field NAME=VALUE` path, for fields
whose `editmeta` schema type is `string`, `number`, `date`, `datetime`, or `user`:
the handler resolves the field name to its `customfield_NNNNN` id, confirms the field
is on the Edit screen via `editmeta`, serializes `VALUE` per the schema type, and PUTs
it alongside any other changed fields. Successful resolution inserts the field into the
`changed_fields` BTreeMap (key: human field name or `customfield_NNNNN` literal; value:
the raw `VALUE` string), so it appears in the BC-3.4.012 table-mode echo and the
BC-3.4.013 JSON-mode `changed_fields` object.

**`resolve_edit_fields` canonical signature** (as of F2 amendment, P2-006 corrected, F-1 reconciled):
`resolve_edit_fields(client: &JiraClient, profile: &str, key: &str, field_pairs: &HashMap<String, String>, fields: &mut Value, changed_fields: &mut BTreeMap<String, String>) -> Result<()>`

The `field_pairs` parameter is `&HashMap<String, String>` (NOT `&[(String, String)]`) because `parse_field_kv` (the upstream parser at `src/cli/issue/create.rs:1982-1997`) returns `HashMap<String, String>`. `parse_field_kv` uses `map.insert(key, value)` with last-wins semantics — duplicate `--field` keys are collapsed AT PARSE TIME, before `resolve_edit_fields` ever runs. An ordered slice would be structurally incompatible with this upstream output. `HashMap` is the correct type at this boundary.

The `profile: &str` parameter (second arg, after `client`) is REQUIRED because `read_fields_cache(profile)` and `write_fields_cache(profile, ...)` are called inside this function. Per the CLAUDE.md hard rule: every cache reader/writer takes `profile: &str`; cross-profile leakage is a correctness bug (sandbox vs prod custom-field IDs can differ). The caller passes `&config.active_profile_name`.

The function mutates the caller's `fields` JSON object and `changed_fields` map in place; returns `Ok(())` on full success or `Err` on any resolution failure. The divergent F1 line-141 form `-> Result<(Value, Vec<(String,String)>)>` (which also lacked `profile` and used `Vec`) is **superseded** by this signature; the `&mut` + `HashMap` form avoids allocations and is structurally consistent with the upstream parser output. Any implementation that uses the F1 form must be updated before merge.

**Field-name resolution algorithm** (per `resolve_edit_fields`):

1. If `NAME` matches `customfield_\d+` (case-sensitive): bypass Steps 2–2b; use `NAME`
   as the field ID. This is the same bypass used by `parse_field_kv` on the JSM
   create path (BC-3.8.008).
2. **Cache-first field-list fetch** (new per F2 amendment): read
   `~/.cache/jr/v1/<profile>/fields.json` (`read_fields_cache(profile)`).
   - **Cache hit (non-stale, ≤7 days old)**: use the cached `Vec<(id, name)>` directly.
     No `GET /rest/api/3/field` HTTP call is made.
   - **Cache miss or stale**: call `list_fields()` (→ `GET /rest/api/3/field`). On
     success, write the result to `fields.json` via `write_fields_cache(profile, &fields)`
     using the **best-effort writer pattern** (see invariant 6). The fetched result is
     used for this invocation regardless of whether the cache write succeeds.
   - The field list (from cache or API) is shared across all `--field` pairs in the same
     invocation — at most one cache read and at most one API call per `issue edit`
     invocation, regardless of how many `--field` pairs are supplied.
2b. Perform case-insensitive exact match first against the field list; if no exact match,
   perform case-insensitive substring match.
   - Zero matches → `JrError::UserError` with hint to use `jr project fields` or
     supply `customfield_NNNNN` directly. Exit 64.
   - Multiple substring matches → `JrError::UserError` naming the ambiguous candidates.
     Exit 64.
   - Single match → use its `id`.
3. Call `get_editmeta(key)` (→ `GET /rest/api/3/issue/{key}/editmeta`). If the
   resolved field ID is absent from `editmeta.fields` → `JrError::UserError` with
   Edit-screen actionable hint ("ask a project admin to add this field to the Edit
   screen"). Exit 64. This applies to BOTH the name-resolved path AND the
   `customfield_NNNNN` literal bypass path. The `editmeta` response is NOT cached
   (see non-goal note below).
3b. **Operations check** (new, P3-LOW-002): inspect `editmeta.fields[id].operations`.
   If `"set"` is NOT present in the list → `JrError::UserError`: "field '<NAME>'
   does not support direct `set` via the edit API (operations: [<actual_ops>]). Use
   the Jira web UI or check with your project admin." Exit 64. No PUT attempted.
   This guards against fields that are present on the Edit screen but are read-only
   (e.g., system-managed computed fields) — a PUT for such a field would be rejected
   by the server anyway; catching it early gives a more actionable error. Standard
   editable custom fields always include `"set"` in their `operations` array.
4. Read `editmeta.fields[id].schema.type` and serialize `VALUE`. Full type dispatch
   matrix (F-4: `option` explicitly anchored here so this step covers all types):
   - `string` or `text`: bare JSON string.
   - `number`: parse `VALUE` as `f64` (error → exit 64 if non-numeric or non-finite).
     Wire: JSON number. See EC-3.4.015-4 and EC-3.4.015-4a.
   - `date` / `datetime`: bare JSON string (no client-side ISO 8601 validation; server
     validates). See VP-396-011.
   - `user`: `{"accountId": VALUE}`. Caller supplies raw `accountId`. See VP-396-011.
   - **`option`**: → dispatch to BC-3.4.016 Step 4a. Resolve `VALUE` against
     `editmeta.fields[id].allowedValues` (human label → option `id`); wire payload is
     `{"id": "<optionId>"}`. `resolve_edit_fields` delegates the option-value resolution
     step to the same code path as BC-3.4.016. This arm must be handled BEFORE the
     unknown→exit-64 arm — `option` is a known, supported type.
   - `array` / `any` / unknown: `JrError::UserError` naming the unsupported type with
     a hint. Exit 64.
5. Merge the resolved `(field_id, serialized_value)` pair into the shared `fields`
   JSON object (same object used by all other `issue edit` flags).
6. After successful resolution: insert `(human_name_or_field_id, VALUE)` into
   `changed_fields`. For the `customfield_NNNNN` literal bypass path, the key is the
   literal `customfield_NNNNN` string. For name-resolved fields, the key is the human
   name as it was supplied in `--field NAME=VALUE` (not the resolved `customfield_*` id).

**Non-goal — `editmeta` is NOT cached**: The `GET /rest/api/3/issue/{key}/editmeta`
response is issue-specific and mutable (an admin can change the Edit screen at any
time). Caching it would risk stale `allowedValues` producing wrong option IDs on the
wire. No `editmeta` cache is planned for v1. This is a deliberate non-goal and must
not be flagged as a gap by reviewers.

**Preconditions**:
- `jr issue edit <key> --field NAME=VALUE [--field ...]` issued on the single-key path.
- No flag-overlap (BC-3.4.017 Gate B passes).
- No multi-key context (BC-3.4.017 Gate A passes).
- At least one other field flag OR `--field` alone satisfies `has_any_field_change`.
- PUT 204 received from Jira API.

**Postconditions**:
- Exit code 0.
- The field is updated on the Jira issue.
- `changed_fields` contains the `--field` key/value entries alongside any other changed
  fields, in BTreeMap alphabetical order.
- Table-mode stderr: `  <NAME> → <VALUE>` echo line (consistent with BC-3.4.012).
- JSON-mode stdout: `changed_fields["<NAME>"] == "<VALUE>"` (consistent with BC-3.4.013).
- `GET /rest/api/3/field` is NOT called when a warm (non-stale) `fields.json` cache
  exists for the active profile. At most one `GET /rest/api/3/field` call per invocation
  regardless of how many `--field` pairs are supplied.
- `fields.json` cache is populated on a cache miss; the populated file persists for
  subsequent invocations (7-day TTL, same as all other jr caches).
- `get_editmeta(key)` is called AT MOST ONCE per invocation (the response is shared
  across all `--field` pairs).
- `get_editmeta` is NOT called when `--field` is absent (no latency added to existing
  `issue edit` invocations).

**Invariants**:
1. `--field` pairs are resolved AFTER all existing flag resolutions (description,
   summary, type, priority, team, points, no_points, parent, no_parent). The
   `resolve_edit_fields` call is the last step before `client.edit_issue`.
2. The `changed_fields` map key for a `--field` entry is the human-supplied `NAME`
   (or the `customfield_NNNNN` literal for bypass calls) — never the internal
   `customfield_NNNNN` ID when a name was resolved.
3. The `fields` JSON object is the same object used by all other flags. The
   `--field` entries are merged into it, not a separate object.
4. On PUT failure (non-204 response), the constructed `changed_fields` entries for
   `--field` are discarded (same invariant as BC-3.4.012 invariant 6 — map emitted
   only post-204).
5. The `number` type serialization reuses `f64` parsing. If `VALUE` parses successfully
   as `f64`, the wire value is the JSON number. If not, exit 64 before the PUT.
6. **Field-list cache contract** (mirrors `CmdbFieldsCache` / `cmdb_fields.json` pattern
   in `src/cache.rs`): the `fields.json` cache stores `Vec<(String, String)>` — `(id, name)`
   tuples — under `~/.cache/jr/v1/<profile>/fields.json`, 7-day TTL, per-profile. The
   struct is `FieldsCache { fields: Vec<(String, String)>, fetched_at: DateTime<Utc> }`
   implementing `Expiring`. Read function: `read_fields_cache(profile: &str) -> Result<Option<FieldsCache>>`.
   Write function: `write_fields_cache(profile: &str, fields: &[(String, String)]) -> Result<()>`.
7. **Best-effort writer** (`write_fields_cache`): cache write failures are swallowed via
   `eprintln!("warning: failed to write fields cache: {e}")` and the function returns
   `Ok(())`. This follows the request-type cache writer pattern (`write_request_type_cache`
   in `src/cache.rs`): a missed cache write costs at most one extra HTTP call on the
   next invocation — it must NEVER fail a successful field resolution. The writer's
   rustdoc MUST document this choice with: "Best-effort: disk-write errors are logged to
   stderr and swallowed; callers always proceed with the fetched result."
8. **Cache is a read-acceleration shortcut only** — not correctness-critical. The global
   field list changes only when Jira admins add/remove custom fields (infrequent). A
   7-day stale cache in the worst case causes a name-resolution failure against a newly
   added field (user can clear via cache path or supply `customfield_NNNNN` directly).
9. The `editmeta` response is NEVER cached. See non-goal note above the algorithm.
10. **`resolve_edit_fields` MUST be called INSIDE the `--dry-run` block** (before the
    `return Ok(())` short-circuit), NOT after it. The existing `--dry-run` block in
    `src/cli/issue/create.rs:551-708` is self-contained and short-circuits with
    `return Ok(())` at line 707. Any code placed AFTER the dry-run block never executes
    under `--dry-run`. Therefore: `resolve_edit_fields` (Steps 1–6) must be invoked
    within the dry-run path so that (a) the resolved `--field` entries appear in the
    planned-changes preview table/JSON, and (b) resolution failures (zero-match, bad type,
    absent from `editmeta`, `"set"` absent from `operations`) still propagate as `Err`
    and exit 64 even under `--dry-run`. The PUT (Step 6 `client.edit_issue`) must NOT be
    called inside the dry-run path. Concrete placement: the dry-run path runs parse →
    Gate B → Gate A → existing-flag resolutions → `resolve_edit_fields` →
    render-preview → `return Ok(())`. The live path runs the same steps but replaces
    render-preview with `client.edit_issue` → success-echo.

**Edge Cases**:
- EC-3.4.015-1: `--field "Unknown Field=Value"` — zero matches in `list_fields()` →
  exit 64 with actionable hint naming `jr project fields` as a discovery tool.
- EC-3.4.015-2: `--field "Sum=Value"` — multiple substring matches (e.g., "Summary",
  "Sum Total") → exit 64 naming the ambiguous candidates with their `customfield_NNNNN`
  IDs to help the caller use the literal bypass.
- EC-3.4.015-3: Field found in `list_fields()` but absent from `editmeta` (not on Edit
  screen) → exit 64 with "ask a project admin to add this field to the Edit screen for
  this issue's project/issue type."
- EC-3.4.015-4: Number field (`schema.type: "number"`) with a non-numeric or non-finite
  `VALUE` → exit 64 with parse error message. No PUT attempted. Two distinct failure
  modes: (a) `"abc".parse::<f64>()` fails at parse → exit 64 immediately; (b) `"inf"` or
  `"nan"` parse successfully as `f64` but `serde_json::Number::from_f64(v)` returns
  `None` for non-finite values (NaN, +Inf, -Inf) → exit 64 at the JSON-number
  construction step. Both paths produce the same user-facing exit 64; see EC-3.4.015-4a
  for the integer-representation invariant on success.
- EC-3.4.015-4a: Number field with `VALUE = "5"` (integer input) → parses to `f64(5.0)`
  → wire value is the JSON number `5` (NOT `5.0`). The `serde_json` `Number` type
  preserves the integer representation when `f64` has no fractional part (i.e., `5.0_f64`
  serializes as `5`, not `5.0`). Implementation: use `serde_json::Number::from_f64(v)`
  (returns `Option`; error if NaN/Inf → exit 64). VP-396-010 pins this invariant.
  `5e3` round-trips as `5000` (serde_json normalizes scientific notation to integer form
  when the value is a whole number). `5.5` serializes as `5.5`.
- EC-3.4.015-5: Field has `schema.type: "array"` or `schema.type: "any"` → exit 64
  with message naming the unsupported type and suggesting the Jira UI or a future
  `--field` v2 for multi-value support.
- EC-3.4.015-6: `list_fields()` API failure (401/403/5xx) → propagated via `?`. The
  error surfaces as a standard auth/API error using the existing error-hint infrastructure
  (`API_TOKEN_EXPIRY_HINT` on 401, raw message on other statuses). No PUT attempted.
- EC-3.4.015-7: `get_editmeta` API failure (including 404 = unknown issue key) →
  propagated via `?`. Same error surface as EC-3.4.015-6.
- EC-3.4.015-8: `customfield_NNNNN` literal bypass — field absent from `editmeta` →
  exit 64 with Edit-screen hint using the literal `customfield_NNNNN` as the field
  name in the message. Same error as EC-3.4.015-3 but triggered without a `list_fields()`
  round-trip.
- EC-3.4.015-9: `--field =VALUE` (empty `NAME`) → `parse_field_kv` splits on the first
  `=` and returns `Ok(("", "VALUE"))` (no error — the string contains `=`). The empty key
  falls through to Step 2b name resolution and exits 64 via the zero-match path (same as
  EC-3.4.015-1: zero matches → exit 64 with actionable hint). There is no dedicated
  empty-NAME guard in `parse_field_kv`; the zero-match exit path in `resolve_edit_fields`
  is the sole error handler for empty NAME.
- EC-3.4.015-10: `--field NAME` (no `=` in the argument) → parse error at
  `parse_field_kv` → exit 64.
- EC-3.4.015-11: `--field NAME=` (empty `VALUE`, name present) → allowed. Empty string
  is a legal value for string fields and is passed to Jira. Jira validates required
  fields server-side; optional string fields may be cleared with an empty value.
- EC-3.4.015-12: Multiple `--field` pairs in one invocation — all share the same
  field list (from cache or single API fetch) and the same `editmeta` result. If any
  pair fails resolution (e.g., `--field A=ok --field B=bad` where `B` is absent from
  `list_fields()`), `resolve_edit_fields` returns `Err` on the first failing pair; the
  entire call fails with exit 64 and zero PUT is attempted. `changed_fields` is discarded
  (never emitted). VP-396-009 pins this all-or-nothing invariant.
- EC-3.4.015-12a: Valid `--field` with a PUT mock returning 400 → the resolution
  succeeds (exit 64 is NOT triggered at the resolution stage); the PUT is attempted; the
  400 surfaces as a `JrError` with the server's error body; exit code reflects failure
  (exit 1 or as mapped by `JrError`). `changed_fields` is discarded (invariant 4:
  emitted only post-204). VP-396-009 pins this path. No `  NAME → VALUE` echo is
  emitted on table mode; no `changed_fields` key appears in JSON mode.
- EC-3.4.015-13: `--field` and other flags (`--summary`, `--priority`, etc.) in the
  same invocation — the `fields` JSON object contains entries from both sources; the
  single PUT carries all changes simultaneously. The `changed_fields` map contains
  entries from both sources in alphabetical key order.
- EC-3.4.015-14: **Cache hit** — `~/.cache/jr/v1/<profile>/fields.json` exists and is
  ≤7 days old → field list is loaded from cache; `GET /rest/api/3/field` is NOT called.
  The resolution and PUT proceed normally. VP-396-006 verifies this invariant.
- EC-3.4.015-15: **Cache miss or stale** — `fields.json` absent or >7 days old → `GET
  /rest/api/3/field` is called; result is written to `fields.json` via the best-effort
  writer; resolution proceeds with the fetched list. Subsequent invocations within 7
  days skip the HTTP call.
- EC-3.4.015-16: **Cache-write failure** — disk full, permissions error, or other I/O
  failure during `write_fields_cache` → `eprintln!("warning: failed to write fields
  cache: ...")` is emitted to stderr; the function returns `Ok(())`. The current
  invocation proceeds with the fetched field list and resolves normally; exit code is
  NOT affected by the cache-write failure. The next invocation will encounter a cache
  miss (and attempt another fetch + write).

- EC-3.4.015-17: `--field CUSTOMFIELD_10001=Value` (mixed/upper-case `customfield_`
  prefix) → the bypass regex `customfield_\d+` is case-sensitive (Rust `Regex::is_match`
  on a lowercase-only pattern). `CUSTOMFIELD_10001` does NOT match the bypass. It falls
  through to Step 2b name resolution. If no field named `CUSTOMFIELD_10001` exists in
  the cached/fetched field list, exit 64 via the zero-match path with the standard
  actionable hint ("use `jr project fields` or supply the lowercase `customfield_NNNNN`
  literal directly"). This is a deliberate design choice: the Jira Cloud REST API uses
  lowercase `customfield_` prefix in all API responses; accepting uppercase would mask
  typos and create a second bypass surface. Users must supply the exact lowercase literal
  to activate the bypass.
- EC-3.4.015-18: `--field NAME=VALUE --dry-run` → Gate A and Gate B still fire (the
  guards are evaluated before any HTTP, including under `--dry-run`). If the gates pass,
  `resolve_edit_fields` is called INSIDE the `--dry-run` block (before the `return Ok(())`
  short-circuit) — see invariant 10 for the mandatory control-flow placement. The
  read-only HTTP calls (`GET /rest/api/3/field` / cache read, `GET /rest/api/3/issue/
  {key}/editmeta`) execute within `resolve_edit_fields` as they would on the live path.
  The PUT is NOT issued. The planned-changes preview (same as BC-3.4.012 EC-3.4.012-9
  behavior) reflects the resolved `--field` entries in the preview table.
  **Exit code: 0** (the dry-run block returns `Ok(())` — confirmed from source at
  `src/cli/issue/create.rs:707`: `return Ok(());` at the end of the dry-run block).
  Mirrors EC-3.4.012-9. Implementers MUST NOT place `resolve_edit_fields` after the
  dry-run `return Ok(())` — it would silently skip `--field` preview and never surface
  resolution failures under `--dry-run`.
- EC-3.4.015-19: **Resolution failure under `--dry-run`** — if field resolution fails
  (zero-match, ambiguous name, unsupported type, field absent from `editmeta`, or
  `"set"` absent from `operations`) while `--dry-run` is set, the resolution error is
  still surfaced with **exit 64**. The dry-run preview is NOT rendered when resolution
  fails: the read-only HTTP calls (`list_fields()`, `editmeta`) run as normal, but if
  they produce an error before the preview is rendered, `resolve_edit_fields` returns
  `Err` and the error propagates through `handle_edit` as a standard `JrError`. The
  `--dry-run` flag does not suppress or defer resolution errors — it only suppresses
  the PUT and redirects the success path to a preview. VP-396-008 covers the
  resolution-failure-under-dry-run sub-case.
- EC-3.4.015-20: **`operations` lacks `"set"`** — field is present in `editmeta` (Step 3
  passes), but `editmeta.fields[id].operations` does not contain `"set"` → Step 3b fires
  → exit 64 with hint naming the field and its actual operations list. No PUT attempted.
  This covers computed/read-only fields that appear on the Edit screen but cannot be set
  via the API. VP-396-012 verifies this path.

**Verification Properties**:
- VP-396-001: String/number `--field` value appears in `changed_fields` echo (table and
  JSON); human name as key; `customfield_NNNNN` literal bypass skips field-list fetch
  entirely.
- VP-396-003: Field absent from `editmeta` → exit 64 with Edit-screen actionable hint;
  no PUT issued.
- VP-396-004: Unsupported field types (`array`, `any`) → exit 64 with hint; no PUT issued.
- VP-396-006: Warm `fields.json` cache (non-stale) → no `GET /rest/api/3/field` HTTP
  call; field resolution and PUT still succeed.
- VP-396-007: Cache-write failure (`write_fields_cache` I/O error) → `warning:` line on
  stderr, exit 0, resolution and PUT succeed (best-effort swallow positively tested).
- VP-396-008: `--field` + `--dry-run` → success path exits 0; read-only HTTP (cache,
  `editmeta`) fires; PUT NOT issued; resolution failure under `--dry-run` still exits 64.
- VP-396-009: Multi-`--field` partial-failure and PUT-failure discard `changed_fields`.
- VP-396-010: Number field `f64` wire serialization — integer inputs produce exact integer
  JSON output (`5` → `5`, NOT `5.0`).
- VP-396-011: `user`-type wire shape `{"accountId": VALUE}` and `date`/`datetime`
  bare-string pass-through are present on wire; claimed in BC-3.4.015 Step 4.
- VP-396-012 (P3-LOW-002): field present in `editmeta` but `"set"` absent from
  `operations` → exit 64 with actionable hint; no PUT.

**Trace**: issue #396 F2; `src/cli/issue/create.rs::handle_edit` (resolution integration);
`src/api/jira/issues.rs::get_editmeta` (new); `src/cli/issue/helpers.rs::resolve_edit_fields`
(new, orchestrates resolution pipeline — owns exact-match-then-substring logic and all
exit-64 ambiguity handling; any field-lookup helper it calls is an implementation detail
not spec-anchored here);
`src/types/jira/editmeta.rs` (new — `EditMeta`, `EditMetaField`, `EditMetaFieldSchema`,
`AllowedValue`); `src/cache.rs::FieldsCache` / `read_fields_cache` / `write_fields_cache`
(new, mirrors `CmdbFieldsCache` / `cmdb_fields.json` pattern; best-effort writer);
`.factory/research/issue-396-jsm-fields-validation.md`;
`.factory/phase-f2-spec-evolution/prd-delta-396.md §3 and §5`

[NEW 2026-05-22 issue #396 F2]
[AMENDED 2026-05-22 F2 cache gap: field-list cache (fields.json, 7-day TTL, best-effort writer) specified; editmeta non-goal stated; EC-3.4.015-14..16 added; invariants 6-9 added; VP-396-006 cited]
[AMENDED 2026-05-22 adversary pass 3: Step 3b (operations/"set" check) added; EC-3.4.015-19 (resolution failure under --dry-run, exit 64) added; EC-3.4.015-18 exit code pinned to 0; VP-396-011 (user/date/datetime wire) and VP-396-012 (operations check) added]

---

#### BC-3.4.016: `issue edit KEY --field NAME=VALUE` (single-select `option` field) — resolves human option value to `allowedValues[].id`, sends `{"id":"<id>"}` on wire; `changed_fields` echo shows human label

**Confidence**: HIGH
**Source**: issue #396 F2 spec evolution; `src/cli/issue/create.rs::handle_edit`; `src/api/jira/issues.rs::get_editmeta`; `.factory/research/issue-396-jsm-fields-validation.md §Q2`
**Subject**: Issue write

**Description**: When `editmeta` reports `schema.type == "option"` for the resolved
field, the handler additionally resolves the human-readable `VALUE` to the numeric
option `id` from `editmeta.fields[id].allowedValues`. The wire payload uses the
`{"id": "<optionId>"}` shape required by the Jira Cloud REST API for single-select
custom fields. The `changed_fields` echo shows the human option label (not the id),
keeping the output readable for both table and JSON consumers.

This BC builds on BC-3.4.015 (same field-name resolution, `editmeta` fetch, and
merge steps apply). Only Step 4 differs: instead of bare-string serialization, the
option value is resolved to its `id` before building the wire fragment. **The
cache-first field-list fetch from BC-3.4.015 invariants 6–8 applies here equally** —
field-name resolution reads from `fields.json` before falling back to `GET
/rest/api/3/field`; the `editmeta` response remains uncached.

**Option value resolution** (Step 4a, applied after `schema.type == "option"` is
detected):

1. If `VALUE` matches an `allowedValues[].id` exactly (numeric string comparison) →
   use that `id` as-is (id-bypass path). The `changed_fields` echo value is `VALUE`
   (the raw literal, not a reverse-looked-up label — no label resolution occurs on
   the id-bypass path).
2. Otherwise: perform case-insensitive exact match on `allowedValues[].value`.
   If no exact match, perform case-insensitive substring match.
   - Zero matches → `JrError::UserError` listing allowed values (e.g., "Allowed values:
     High, Medium, Low"). Exit 64.
   - Multiple substring matches → `JrError::UserError` listing ambiguous candidates with
     their ids (e.g., "value 'H' is ambiguous — found: High (id=10286), Unknown (id=10299).
     Specify the exact value."). Exit 64.
   - `allowedValues` is empty or absent → `JrError::UserError` ("field 'NAME' has no
     configured option values. Confirm the field is set up correctly in your Jira
     project admin."). Exit 64.
   - Single match → use its `id`. `changed_fields` echo value is the matched
     `allowedValues[].value` (the stored label, not the user's query casing).

Wire payload: `{"fields": {"customfield_NNNNN": {"id": "<optionId>"}}}`.

`changed_fields` key: human field name (or `customfield_NNNNN` literal for bypass).
`changed_fields` value: matched `allowedValues[].value` (stored label) — NOT the
option `id`. Exception: when the id-bypass path fires, `changed_fields` value is
`VALUE` (the id literal).

**Preconditions**:
- Same as BC-3.4.015 (single-key path, no flag-overlap, no multi-key context, PUT 204).
- `editmeta.fields[id].schema.type == "option"`.
- `allowedValues` is populated (non-empty) for single-match case.

**Postconditions**:
- Exit code 0.
- PUT body contains `{"customfield_NNNNN": {"id": "<resolvedOptionId>"}}`.
- `changed_fields["<NAME>"]` == matched option label (stored casing from `allowedValues[].value`),
  NOT the option `id`, NOT the user's query casing.
- Table-mode stderr: `  <NAME> → <matched_label>` echo (consistent with BC-3.4.012).
- JSON-mode `changed_fields["<NAME>"]` == `"<matched_label>"` (consistent with BC-3.4.013).

**Invariants**:
1. The wire payload for `option`-type fields MUST use `{"id": "<optionId>"}`. Sending
   `{"value": "..."}` is rejected by the Jira Cloud REST API (confirmed in research Q2).
2. The `changed_fields` value is the STORED label (casing from `allowedValues[].value`),
   not the user's query string. Case-insensitive matching but stored-casing echo.
3. The option `id` is never exposed in the `changed_fields` echo (for the name-match
   path). The id appears only on the wire and in the server's response.
4. The id-bypass path (when `VALUE` is an exact numeric match to an `allowedValues[].id`)
   does not perform a reverse lookup — the echo value is the raw id.

**Edge Cases**:
- EC-3.4.016-1: `allowedValues` is empty or absent for the `option`-type field → exit
  64 with "field has no configured option values" message. This is unusual but possible
  for misconfigured fields.
- EC-3.4.016-2: `VALUE` matches no `allowedValues[].value` → exit 64 listing the allowed
  values. The error message enumerates all `allowedValues[].value` strings to aid the caller.
- EC-3.4.016-3: `VALUE` is a substring match against multiple `allowedValues[].value`
  entries (e.g., `--field Urgency=h` matches "High" and "High Priority") → exit 64
  listing ambiguous candidates with their ids.
- EC-3.4.016-4: `VALUE` is a valid option `id` (numeric, e.g., `"10286"`) → id-bypass:
  used directly without `allowedValues[].value` lookup. `changed_fields` echo is `"10286"`.
  No reverse label lookup. This mirrors the `customfield_NNNNN` bypass for field names.
  Note: if an option `id` and an option `value` happen to be the same numeric string
  (e.g., id=`"42"` and another option value=`"42"`), the id-bypass wins — the numeric
  check is applied first. This is a deliberate disambiguation rule: id-bypass takes
  priority over label matching when the value string is purely numeric and matches an id.
- EC-3.4.016-5: Case-insensitive matching: `--field Urgency=high` (all lowercase) →
  matches `"High"` in `allowedValues` → `changed_fields` shows `"High"` (stored casing),
  not `"high"`.
- EC-3.4.016-6: `--field Urgency=HIGH` (all uppercase) → matches `"High"` →
  `changed_fields` shows `"High"` (stored casing).
- EC-3.4.016-7: Exact match takes precedence over substring: `"High"` with `VALUE="High"`
  (exact) → uses exact-match result, even if "High" is also a substring of "High Priority".
  Ambiguity is evaluated only when there is no exact match.

**Verification Properties**:
- VP-396-002: Option field resolves to `{"id": ...}` on wire; `changed_fields` echo
  shows human label (not id); case-insensitive matching; option-id bypass.
- VP-396-006: Warm `fields.json` cache (non-stale) → no `GET /rest/api/3/field` HTTP
  call; field-name resolution for option fields proceeds from cache; `editmeta` fetch
  and PUT still execute normally. (BC-3.4.016 inherits the cache-first behavior from
  BC-3.4.015 invariants 6–8 — the same `resolve_edit_fields` step 2/2b path is
  followed regardless of whether the field schema type is `string` or `option`.)

**Trace**: issue #396 F2; `src/cli/issue/create.rs::handle_edit`;
`src/api/jira/issues.rs::get_editmeta`; `.factory/research/issue-396-jsm-fields-validation.md §Q2`
(wire format confirmed: `{"customfield_NNNNN": {"id": "..."}}` is the working shape);
`.factory/phase-f2-spec-evolution/prd-delta-396.md §3`

[NEW 2026-05-22 issue #396 F2]

---

#### BC-3.4.017: `--field` multi-key/`--jql` multi-issue rejection (C-1 guard) + flag-overlap hard error for `summary`/`description`/`issuetype`/`priority`

**Confidence**: HIGH
**Source**: issue #396 F2 spec evolution; `src/cli/issue/create.rs::handle_edit` (C-1 guard, `REJECTED_IN_BULK` set); `.factory/phase-f2-spec-evolution/prd-delta-396.md §3`
**Subject**: Issue write

**Description**: Two enforcement gates ensure `--field` is not misused in contexts
where its behavior is either undefined (bulk edit) or would silently overwrite an
explicitly-set flag value (flag overlap). Both gates fire BEFORE any HTTP call.

**Gate A — multi-key/`--jql` multi-issue rejection (C-1 guard):**

`--field` is added to the `REJECTED_IN_BULK` set in `handle_edit`. When the handler
detects 2+ positional keys, or when `--jql` resolves to 2+ issues, the C-1 block
fires with the same error pattern used by other bulk-rejected flags (`--parent`,
`--team`, `--description`): "Multi-key bulk edit doesn't yet support: `--field`. Use
a single key, or open an issue if this matters for your workflow." Exit 64.

`--jql` resolving to exactly ONE issue routes through the existing single-match fast
path and proceeds normally on the single-key path (consistent with BC-3.4.003 and
all other bulk-rejected flags).

**Gate B — flag-overlap hard error:**

If a dedicated flag and `--field` both target the same system field in the same
invocation:
- `--summary X --field summary=Y` (or `--field Summary=Y` — case-insensitive on the
  `--field NAME` side against the known system field keys)
- `--description X --field description=Y`
- `--type X --field issuetype=Y` (note: `--type` maps to the Jira system field key
  `issuetype`, not `type`)
- `--priority X --field priority=Y`

→ `JrError::UserError`: "<Field> is set by both --<flag> and --field; use only one."
Exit 64. NO HTTP call (no `list_fields()`, no `editmeta`, no PUT).

Gate B is evaluated at the top of `handle_edit`, after clap parsing (so both flag
values are in scope), but before any field resolution or HTTP calls. This ensures the
guard is O(1) and never causes a latency penalty.

**Scope of Gate B**: Exactly four first-party system fields (`summary`, `description`,
`issuetype`, `priority`). Team (`--team`) and points (`--points`/`--no-points`) use
dynamically-resolved custom field IDs; overlap detection for those would require an
API call, violating the "no HTTP before the guard" invariant. These are deferred to v2.

**Scope of Gate A**: `--field` is REJECTED_IN_BULK (not BULK_SUPPORTED). This is
intentional: the Jira Cloud Bulk API does not support arbitrary custom field writes;
adding bulk `--field` support would require a separate design pass.

**Preconditions for Gate A error**:
- 2+ positional keys supplied, OR `--jql` resolves to 2+ issues.
- `--field` is present.

**Preconditions for Gate B error**:
- At least one of the four dedicated flags (`--summary`, `--description`, `--type`,
  `--priority`) is present AND the corresponding system field key is targeted by a
  `--field NAME=VALUE` pair (case-insensitive key comparison).

**Postconditions (Gate A)**:
- Exit code 64.
- Stderr contains a message referencing `--field` and the bulk-rejection pattern.
- **Positional multi-key sub-case**: No HTTP calls are made (no JQL execution, no
  `list_fields()`, no `editmeta`, no PUT). The gate fires purely from argument count.
- **`--jql` multi-issue sub-case**: The JQL search IS executed to determine the matched
  issue count (you cannot know the count without running the query). Once 2+ results are
  detected, the gate fires. No `list_fields()`, no `editmeta`, no PUT is issued.
  The JQL call is the only HTTP call that occurs before the gate fires.

**Postconditions (Gate B)**:
- Exit code 64.
- Stderr contains the overlap error message naming the conflicting flag and field.
- No HTTP calls are made.

**Invariants**:
1. **Gate B is evaluated before Gate A.** When an invocation is BOTH multi-key AND flag-
   overlap (both conditions are simultaneously true), Gate B fires first: the flag-overlap
   error is emitted to stderr, Gate A is NOT evaluated, and exactly ONE error message
   reaches stderr. This ordering is intentional: a flag-overlap error is a programmer
   mistake that is equally invalid on any key count, and surfacing it directly is more
   actionable than a bulk-rejection that obscures the root cause.
2. The `REJECTED_IN_BULK` set partition test (the compile-time assertion in
   `test_343_every_edit_field_is_categorized` that partitions flags into `SELECTORS`,
   `BULK_SUPPORTED`, and `REJECTED_IN_BULK`) must be updated to include `--field`. This
   ensures the partition is exhaustive: `--field` appears in exactly ONE of the three
   sets. The `--label` conflict block's completeness against that partition is
   mechanically enforced by `test_label_conflict_block_lists_every_relevant_flag`
   (see EC-3.4.017-14).
3. `--jql` matching exactly ONE issue routes to the single-key path — this is NOT an
   error. Gate A only fires when `--jql` matches 2+ issues.
4. The flag-overlap comparison on the `--field NAME` side is case-insensitive against
   the canonical system field keys (`summary`, `description`, `issuetype`, `priority`).
   A `--field SUMMARY=X` or `--field Summary=X` is detected as an overlap for
   `--summary Y`.

**Edge Cases**:
- EC-3.4.017-1: `jr issue edit KEY1 KEY2 --field Urgency=High` → Gate A fires → exit
  64, bulk-rejection message.
- EC-3.4.017-2: `jr issue edit --jql "project = FOO" --field Urgency=High` when JQL
  matches 2+ issues → JQL search executes (required to determine match count) → Gate A
  fires → exit 64. No `list_fields()`, no `editmeta`, no PUT.
- EC-3.4.017-3: `jr issue edit --jql "key = FOO-1" --field Urgency=High` when JQL
  matches exactly 1 issue → Gate A does NOT fire → single-key path proceeds normally.
- EC-3.4.017-4: `jr issue edit KEY --summary "New title" --field summary=Other` →
  Gate B fires for `summary` → exit 64, overlap error, no HTTP.
- EC-3.4.017-5: `jr issue edit KEY --description "text" --field description=other` →
  Gate B fires for `description` → exit 64.
- EC-3.4.017-6: `jr issue edit KEY --type Bug --field issuetype=Task` → Gate B fires
  for `issuetype` (note: `--type` maps to the `issuetype` system field key, not `type`)
  → exit 64.
- EC-3.4.017-7: `jr issue edit KEY --priority High --field priority=Low` → Gate B
  fires for `priority` → exit 64.
- EC-3.4.017-8: `jr issue edit KEY --team "Platform Core" --field team=Other` → Gate B
  does NOT fire (team uses a dynamically-resolved custom field ID; deferred to v2) →
  both `--team` and `--field team=Other` are processed; last-write-wins in the `fields`
  JSON object. This is a known limitation documented in the CLAUDE.md Gotcha entry.
- EC-3.4.017-9: `jr issue edit KEY --field NAME=` (empty value) → Gate B does NOT fire
  (field overlap check requires matching a dedicated flag, not just any `--field` pair);
  empty value is allowed by BC-3.4.015 EC-3.4.015-11.
- EC-3.4.017-10: `jr issue edit KEY --field summary=A --field summary=B` (two `--field`
  pairs targeting the same system field, WITHOUT the dedicated `--summary` flag) → Gate B
  does NOT fire (Gate B requires the dedicated flag AND a `--field` pair for the same
  key; two `--field` pairs for the same key without the dedicated flag is not a Gate B
  condition). `parse_field_kv` (at `src/cli/issue/create.rs:1982-1997`) collapses the
  duplicate key AT PARSE TIME via `map.insert(key, value)` — the HashMap retains only
  the LAST value (`"B"`). `resolve_edit_fields` never sees both entries; it receives
  `{"summary": "B"}` as a single-entry `HashMap<String, String>`. No "second write"
  occurs inside `resolve_edit_fields` — the collapse happens before it is called.
  End state: `summary` is set to `"B"` on the wire. No error is produced.
  This is last-wins behavior, implemented entirely within `parse_field_kv` (BC-3.8.008).
- EC-3.4.017-11: `jr issue edit KEY --field type=Bug` (using `type` as the field name,
  not `issuetype`) → Gate B does NOT fire. The Gate B comparison checks whether the
  `--field NAME` key, lowercased, matches the canonical system field keys `summary`,
  `description`, `issuetype`, `priority`. The key `type` does NOT match `issuetype`.
  `--field type=Bug` is treated as an ordinary name lookup in `resolve_edit_fields` and
  proceeds to field-name resolution (Step 2b). Note: `--type` maps to the `issuetype`
  system field key in Jira; a `--field` pair targeting `issuetype` directly WOULD trigger
  Gate B when `--type` is also present. Using `type` (without `issue`) as a field name
  is a user error that surfaces as a resolution error (EC-3.4.015-1: zero matches or
  wrong field), not a Gate B conflict.
- EC-3.4.017-12: `jr issue edit KEY1 KEY2 --summary "New" --field summary=Other` →
  both multi-key (Gate A) AND flag-overlap (Gate B) conditions are true. Gate B fires
  first (evaluated before Gate A per invariant 1): the flag-overlap error is emitted to
  stderr, Gate A is NOT evaluated, and exit code is 64. Exactly one error message
  reaches stderr. The multi-key detection is not reached.
- EC-3.4.017-13: `jr issue edit KEY --label add:foo --field Severity=Critical` on a single
  key → exit 64 with `--label` conflict-block error. The `--label` short-circuit at
  `src/cli/issue/create.rs:~835` routes to `handle_edit_bulk_labels` which does not accept
  `field_pairs`; without rejection before the routing decision the `--field` write silently
  drops (exit 0, data loss). The `--label` mutual-exclusion block at lines 445-489 rejects
  this combination before any HTTP call. Error: `"--label cannot be combined with --field in
  the same call. Run separate \`jr issue edit\` commands, or open an issue to track combined
  label + field bulk edits (see #331)."` Combined label + custom-field bulk edits tracked at
  #331. [FIX-F5-001]
- EC-3.4.017-14: The `--label` conflict block at
  `src/cli/issue/create.rs::handle_edit::if !labels.is_empty()` is mechanically enforced
  complete by `test_label_conflict_block_lists_every_relevant_flag` (in `create.rs::tests`).
  **Extraction strategy**: the meta-test parses the conflict-block source via
  `include_str!("create.rs")` and extracts every `conflicting.push("--<flag>")` literal
  from the ENTIRE file (global extraction). This is safe because the local variable name
  `conflicting` is used exclusively within the `if !labels.is_empty() { ... }` block in
  `handle_edit`; if a future cycle introduces a second `conflicting` variable anywhere in
  `create.rs`, the meta-test must be re-scoped to brace-matched extraction. A guard comment
  MUST be added in `create.rs` at the conflict-block declaration site: `// NOTE: the variable
  name 'conflicting' is reserved for this block — test_label_conflict_block_lists_every_relevant_flag
  uses a global scan of conflicting.push("--...") in create.rs`.
  **Expected set construction**: build a `BTreeSet<String>` (NOT `HashSet` — deterministic
  failure diffs across runs, mirrors `test_343_every_edit_field_is_categorized`) from
  `(BULK_SUPPORTED \ {"label"}) ∪ REJECTED_IN_BULK`. For each field, the kebab-case CLI
  flag name is the explicit `long = "<literal>"` value when present, otherwise the field
  name with underscores replaced by hyphens (clap's implicit default). Of the 12 fields
  currently in scope: `issue_type` carries `#[arg(long = "type")]` and maps to `--type`
  (NOT `--issue-type`); the other 11 (`summary`, `priority`, `team`, `points`,
  `no_points`, `parent`, `no_parent`, `description`, `description_stdin`, `markdown`,
  `field`) use the implicit snake→kebab transform. Any future field added to
  `BULK_SUPPORTED`/`REJECTED_IN_BULK` with a non-mechanical `long = "..."` rename will
  be caught by the R2 pin's 12-flag enumeration — the extractor side and the expected
  side must be reconciled together.
  **Assertion**: assert extracted `BTreeSet<String>` equals expected `BTreeSet<String>`.
  A regression that drops any `conflicting.push` line OR adds a new Edit field to
  `BULK_SUPPORTED`/`REJECTED_IN_BULK` without extending the conflict block fails this
  meta-test at `cargo test` time.
  **R2 pin**: include at least one pin test asserting the extractor correctly parses a
  known-good input string (e.g., assert extracted set has exactly 12 members for the
  current block: `--field`, `--summary`, `--priority`, `--type`, `--team`, `--points`,
  `--no-points`, `--parent`, `--no-parent`, `--description`, `--description-stdin`,
  `--markdown`. `--label` itself is the guard condition on the outer `if`, not a pushed
  entry).
  **Co-author**: 10 positive regression tests in `tests/issue_edit_field.rs`
  (`test_label_plus_<flag>_rejected_with_exit_64_no_http` for each of: `priority`, `type`,
  `team`, `points`, `no-points`, `parent`, `no-parent`, `description`, `description-stdin`,
  `markdown`). Test names use snake_case substitution for kebab-case flags
  (e.g., `--no-points` → `test_label_plus_no_points_...`; Rust identifiers cannot contain
  hyphens). Each test asserts exit 64, stderr contains `"--label cannot be combined with"`,
  and stderr contains the specific flag name as a SEPARATE assertion — not as one
  concatenated substring (the conflict block joins all conflicting flags into a single
  comma-separated message). For the `--markdown` test specifically: the invocation uses
  `--label add:x --markdown --description "text"`, which causes BOTH `--description` and
  `--markdown` to appear in the conflict output (`"--label cannot be combined with
  --description, --markdown in the same call. ..."`). Assert `stderr.contains("--markdown")`
  AND `stderr.contains("--label cannot be combined with")` as two separate checks, NOT
  `stderr.contains("--label cannot be combined with --markdown")` (that concatenation does
  not appear verbatim when `--description` precedes `--markdown` in the joined output). Note:
  the `--markdown` test uses `--label add:x --markdown --description "text"` because
  `--markdown` alone triggers an earlier guard at `create.rs:357-363` before the conflict
  block; pairing with `--description` bypasses the early guard and reaches the conflict block,
  verifying the `--markdown` row. [Issue #407]

**Verification Properties**:
- VP-396-005: Multi-key/`--jql`-multi-issue rejection exits 64; flag-overlap hard error
  for `summary`, `description`, `issuetype`, `priority` exits 64 before any HTTP call.
- VP-396-008: `--field` + `--dry-run` → success path exits 0; Gate A/B still fire;
  read-only HTTP executes for preview; PUT NOT issued; resolution failure still exits 64.

**Trace**: issue #396 F2; `src/cli/issue/create.rs::handle_edit` (`REJECTED_IN_BULK`
set update; Gate B overlap check; `has_any_field_change` update to include `--field`);
`.factory/phase-f2-spec-evolution/prd-delta-396.md §3`

[NEW 2026-05-22 issue #396 F2]

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

17 behavioral contracts covering: (a) `jr issue create --request-type` dispatch to the JSM service desk API
(BC-3.8.001..009), (b) forward-direction cross-flag warnings when platform-only flags are passed alongside
`--request-type` (BC-3.8.010..011), (c) inverse-direction cross-flag warnings when JSM-only flags are
passed on the platform path (BC-3.8.012..013), (d) auth-conditional 401 error hints on the JSM POST
path: Basic-auth API-token-expiry hint (BC-3.8.014) and OAuth write-scope hint (BC-3.8.015), gated solely
by `JiraClient::is_oauth_auth()`, and (e) JSM-path input guards: empty `--request-type` early-exit
(BC-3.8.016) and `--markdown` + `--field description=` conflict rejection (BC-3.8.017).
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
**Errors**: Non-JSM project → `require_service_desk` returns `JrError::UserError`; exit 64; no HTTP to servicedeskapi. Error message MUST be call-site-specific: 'Project "<KEY>" is a <type> project. `--request-type` requires a Jira Service Management project. Run "jr project list" to find a JSM project.' (NOT the legacy "Queue commands require…" string from BC-X.8.004 — that string is reserved for queue commands only; see BC-3.8.002 and BC-X.8.004 [UPDATED 2026-05-18 issue #288].) No project resolvable AND (`no_input` is effective OR `prompt_input` itself errors) → exit 64 with the harmonized message: "Project key is required for JSM request creation. Use --project or configure .jr.toml. Run \"jr project list\" to see available JSM projects." — carries the same `--project` / `.jr.toml` / `jr project list` affordances as the platform path (see BC-3.3.001) while preserving the "for JSM request creation" context. Note: `no_input` is effective when set explicitly via `--no-input` OR when stdin is not a TTY (`--no-input` is auto-enabled on non-TTY stdin per CLAUDE.md). The code site (`create.rs:1882-1891`) checks `no_input` only — the non-TTY case is already covered by that single flag. When `no_input` is NOT effective, the handler attempts `helpers::prompt_input("Project key")` first; the harmonized error surfaces only if the prompt itself errors.
**Trace**: `tests/issue_create_jsm.rs` (service desk ID resolution, non-JSM project error path, missing-project error string); `src/api/jsm/servicedesks.rs::require_service_desk`
**Source**: API-verified: `serviceDeskId` is a required string in request body
**Confidence**: HIGH

> **[UPDATED 2026-05-20 issue #385 O-08-02]** The "no project configured" error string harmonized. Previous verbatim: `"project is required for JSM request creation"` (terse, lowercase, no affordances). New verbatim: `"Project key is required for JSM request creation. Use --project or configure .jr.toml. Run \"jr project list\" to see available JSM projects."` — adds `--project`/`.jr.toml`/`jr project list` affordances, sentence-cases the opening, and preserves the JSM-specific context label. The implementing story MUST update `test_jsm_create_missing_project_exits_64_with_jsm_specific_hint` (in `tests/issue_create_jsm.rs`) to assert the new string. The previous error string was: `"project is required for JSM request creation"`.

> **[UPDATED 2026-05-20 issue #385 adversary pass-8 M-01]** Precondition for the harmonized error qualified: the error fires only when no project is resolvable AND `no_input` is effective (OR `prompt_input` itself errors). `no_input` is effective when set explicitly via `--no-input` OR when stdin is not a TTY (auto-enabled per CLAUDE.md) — the code site checks `no_input` only; the non-TTY path is not a separate trigger. When `no_input` is not effective, the handler attempts `helpers::prompt_input` first. "No project configured" alone (without the `no_input`-effective qualifier) is an incomplete precondition.

> **[UPDATED 2026-05-20 issue #385 adversary pass-13 H-1]** Reframed from three independent triggers (`--no-input` / non-TTY / `prompt_input` failure) to TWO conditions: (1) `no_input` is effective (covering both explicit `--no-input` and auto-enabled non-TTY as a single flag check), (2) `prompt_input` itself errors. Resolves the apparent contradiction between "three triggers" in the BC and "one check (`no_input`)" in the code.

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
**Behavior**: When `--request-type` is present, the `--type` flag (if also supplied) is silently ignored at the JSM-dispatch site EXCEPT for emitting a single stderr line: "warning: --type is ignored when --request-type is set; request type encodes the issue type". Exit code unchanged (still 0 on success, or 64/1/2 on applicable error paths). JSON output shape is unchanged from BC-3.8.001. **Warning position (O-08-07):** the warning is emitted at step 5 of the Canonical Guard Ordering (see BC-3.8.016) — INSIDE `handle_jsm_create` AFTER `require_service_desk` returns `Ok`, and BEFORE request-type resolution (step 6: numeric-bypass check, `resolve_jsm_request_type_id`, `parse_field_kv`, POST). NOT before `handle_jsm_create` is called and NOT before `require_service_desk` is called. Consequence: on a non-JSM project (assuming `--request-type` is non-empty — an empty/whitespace-only `--request-type` exits at step 1 per BC-3.8.016 regardless of project type), the user sees ONLY the non-JSM project error (from `require_service_desk`), NOT both the warning and the error. The warning is suppressed on early-exit paths where `require_service_desk` fails. Because the warning fires at step 5 — before request-type resolution at step 6 — on a JSM project with an unresolvable `--request-type` name, the `--type` warning WILL have fired (step 5) and the partial-match error from BC-3.8.003 follows at step 6; both appear on stderr. This is acceptable because the project IS a valid service desk so the "type ignored" warning is genuinely informative. On the success path, the warning fires regardless of `--no-input` or `--output json` settings.
**Inputs**: `--request-type <X>` AND `--type <Y>` (both set simultaneously)
**Outputs/Effects**: Same JSM POST behavior as BC-3.8.001 with the `--type` value unused. One stderr line emitted: "warning: --type is ignored when --request-type is set; request type encodes the issue type". No change to stdout JSON shape. No change to exit code.
**Errors**: None — this is a warning path, not an error path. The presence of `--type` alongside `--request-type` is not an error.
**Trace**: `tests/issue_create_jsm.rs` (warning_on_type_with_request_type integration test; non-JSM project warning-suppression test)
**Source**: ADR-0014 §"Dispatch fork: --type interaction" — `--type` is meaningless in the JSM path because `requestTypeId` encodes the issue type server-side; emitting a warning rather than erroring preserves backward compatibility for scripts that habitually pass `--type`.
**Confidence**: HIGH

> **[UPDATED 2026-05-20 issue #385 O-08-07]** Warning position clarified: the `--type` warning MUST fire inside `handle_jsm_create` AFTER `require_service_desk` returns `Ok`, not before `handle_jsm_create` is entered. Previous behavior (warning firing pre-`require_service_desk` in `handle_create`) produced spurious dual output on non-JSM projects. The implementing story MUST add `test_jsm_create_type_flag_warning_suppressed_on_non_jsm_project` asserting that when `--request-type` is set (non-empty) + project is non-JSM, the `--type` warning is ABSENT from stderr and only the non-JSM project error is emitted. The existing test `test_jsm_create_type_flag_ignored_with_warning` (JSM path) MUST remain green — warnings still fire on the JSM success path.

> **[UPDATED 2026-05-20 issue #385 adversary pass-7 M-01]** Step placement made explicit: warning fires at step 5 (Canonical Guard Ordering), BEFORE request-type resolution at step 6 — not after. Removed stale "after flag parsing and request-type resolution succeed" wording. Removed stale "need not fire" clause for partial-match failure (BC-3.8.003): because the warning fires at step 5 BEFORE step-6 resolution, the warning WILL have appeared by the time partial-match failure surfaces — both messages appear on stderr on a JSM project with an unresolvable request type.

> **[UPDATED 2026-05-20 issue #385 adversary pass-8 H-03]** Threading note: achieving step-5 placement requires the `--type` (`issue_type`) flag value to be in scope inside `handle_jsm_create` at the warning site. Pre-#385, `JsmCreateArgs` does not carry `issue_type`. The implementer MUST thread it in — by extending `JsmCreateArgs`, passing it as an additional parameter, or an equivalent mechanism. The BC constrains WHEN the warning fires (step 5), not HOW the value is threaded. See prd-delta-385.md §O-08-07 Implementation Note for the full threading discussion covering all six flags.

> **[UPDATED 2026-05-20 issue #385 adversary pass-12 F-02]** Single-site requirement: the existing pre-dispatch warning emission block in `handle_create` (which currently fires these warnings before `handle_jsm_create` is called) MUST be REMOVED as part of implementing O-08-07. The `--type` warning must exist at exactly ONE site — canonical step 5 inside `handle_jsm_create`. Double-emission from two code sites is a defect. The new `test_jsm_create_platform_flag_warnings_emit_once_on_success` (Required Test Deliverable item 7) pins this constraint. This is distinct from BC-3.8.011's idempotency contract (one warning per repeated logical flag) — that covers duplicate flag occurrences, not duplicate code sites.

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
the same flag twice still emits ONE warning per logical flag. **Warning position (O-08-07):** all six warnings (the `--type` warning of BC-3.8.010 plus the five platform-only flag warnings of BC-3.8.011) are emitted INSIDE `handle_jsm_create` AFTER `require_service_desk` returns `Ok` — mirroring the BC-3.8.010 position constraint. On a non-JSM project, NONE of these warnings fire; only the non-JSM project error is emitted.

**Inputs**: any combination of `--team`, `--points`, `--parent`, `--to`, `--account-id`
with `--request-type`
**Outputs/Effects**: One stderr warning line per dropped flag; JSM dispatch continues
normally; exit 0 on success.
**Errors**: None — these are warnings, not errors. Dispatch proceeds.
**Related BCs**: BC-3.4.014 — on the JSM path, the `--team` flag is ignored (this contract applies instead); BC-3.4.014's team echo does NOT fire on the JSM path. BC-3.4.014 EC-3.4.014-4 records this exclusion reciprocally.
**Trace**: `tests/issue_create_jsm.rs` (per-flag warning-emission integration tests, one assertion per platform-only flag)
**Source**: Adversary pass-01 C-02 codification; mirrors BC-3.8.010 pattern
**Confidence**: HIGH

[NEW 2026-05-19 issue #288 pr4 adversary-pass-01 C-02] Added to codify the cross-flag
warning policy after adversary pass-01 found silent-drop of 5 platform-only flags on
the JSM dispatch path.

> **[UPDATED 2026-05-20 issue #385 O-08-07]** Warning position constraint applied: all six warnings (the `--type` warning of BC-3.8.010 plus the five platform-only flag warnings of BC-3.8.011) move inside `handle_jsm_create` AFTER `require_service_desk` succeeds — co-located so that on a non-JSM project, NONE of these warnings fire; only the non-JSM project error is emitted. All existing per-flag integration tests MUST remain green — warnings still fire on the JSM success path.

> **[UPDATED 2026-05-20 issue #385 adversary pass-8 H-03]** Threading note: achieving step-5 placement for the five platform-only flag warnings (`--team`, `--points`, `--parent`, `--to`, `--account-id`) requires those flag values to be in scope inside `handle_jsm_create` at the warning site. Pre-#385, `JsmCreateArgs` does not carry these fields. The implementer MUST thread them in — by extending `JsmCreateArgs`, passing them as additional parameters, or an equivalent mechanism. This BC constrains WHEN the warnings fire (step 5), not HOW the values are threaded. See prd-delta-385.md §O-08-07 Implementation Note for the full threading discussion.

> **[UPDATED 2026-05-20 issue #385 adversary pass-12 F-02]** Single-site requirement: the existing pre-dispatch warning emission block in `handle_create` (which currently fires these warnings before `handle_jsm_create` is called) MUST be REMOVED as part of implementing O-08-07. All five platform-only flag warnings must exist at exactly ONE site — canonical step 5 inside `handle_jsm_create`. Double-emission from two code sites is a defect. The new `test_jsm_create_platform_flag_warnings_emit_once_on_success` (Required Test Deliverable item 7) pins this. Note: this is distinct from the existing idempotency contract ("one warning per logical flag regardless of how many times that flag is repeated by the caller") — idempotency concerns duplicate flag occurrences, not duplicate code sites emitting warnings.

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
**Trace**: `tests/issue_create_jsm.rs` (integration tests for the HTTP-401 Basic-auth path): (a) `test_jsm_create_basic_auth_scope_mismatch_401_rewrites_to_api_token_hint` (NEW) — pins the `InsufficientScope`→`NotAuthenticated` rewrite path with a "scope does not match" body fixture; (b) `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (REPURPOSED in place by F4 — fixture stays Basic `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0`, generic-expiry 401 body; assertions flipped from `write:servicedesk-request` to API-token-expiry hint; negative assertion that `write:servicedesk-request` is ABSENT; per adversary-pass-9 C-01 correction — this test is a BC-3.8.014 pin, NOT a BC-3.8.015 pin). The Basic-auth generic-expiry path is pinned by test (b); test (a) covers the scope-mismatch rewrite path. Both AC-3 and AC-5 describe the same observable behavior (API-token-expiry hint for Basic-auth 401) and share test (b) as the generic-expiry pin.
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

`test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` (repurposed in place by F4; `JR_AUTH_HEADER=Basic dGVzdDp0ZXN0`, generic 401 body; assertions assert API-token-expiry hint and assert `write:servicedesk-request` is ABSENT) is the **BC-3.8.014 pin** — NOT a BC-3.8.015 pin. Basic + generic-401 produces the API-token-expiry hint.

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

[REVISED 2026-05-19 issue #384 adversary-pass-9 C-01 CRITICAL design correction] Complete rewrite of testable contract. The F2 passes 1-8 plan ("migrate the pre-#384 Basic-auth 401 test to Bearer + generic-expiry body") was unworkable: a Bearer + generic-expiry 401 routes through the refresh coordinator (client.rs:727+), which deterministically fails with a raw anyhow error (not a `JrError`) via the `JR_AUTH_HEADER` seam, so the `write:servicedesk-request` hint is never injected. The ONLY deterministic Bearer→`JrError`→`write:servicedesk-request` path is the scope-mismatch short-circuit (client.rs:696-704). BC-3.8.015 is now re-specified to its true testable contract: the scope-mismatch path, pinned by the EXISTING `async fn test_jsm_create_oauth_scope_mismatch_401_surfaces_write_servicedesk_request_hint` (already green on `develop`, unmodified). H-NEW-JSM-RT-003 re-bound to this test. `test_jsm_create_basic_auth_generic_401_surfaces_api_token_hint` stays Basic and becomes a BC-3.8.014 pin with flipped assertions. BC-X.8.007 Setup corrected to scope-mismatch body.

---

#### Canonical Guard Ordering — `handle_jsm_create`

**SINGLE SOURCE OF TRUTH** for the complete guard/HTTP ordering in `handle_jsm_create`. BC-3.8.016 (step 1) and BC-3.8.017 (step 2) reference this block rather than embedding copies. `prd-delta-385.md §Canonical Guard Ordering` is a pointer to this block. When changing any step, update ONLY this block.

The following is the complete, implementer-authoritative ordering of input guards, warnings, and HTTP calls in `handle_jsm_create`. Every BC and holdout in this delta is specified against this ordering:

0. Project-key resolution (BC-3.8.002; `create.rs:1882-1891`) — may exit 64 when no project is resolvable AND `no_input` is effective (set explicitly via `--no-input` or auto-enabled on non-TTY stdin) OR `prompt_input` errors. NO HTTP. (O-08-02/BC-3.8.002 harmonizes the error string emitted by this block; see BC-3.8.002)
1. **BC-3.8.016** — Empty/whitespace-only `--request-type` guard — exit 64, NO HTTP. Guard evaluates `request_type_arg.trim().is_empty()`; the inline numeric-bypass check and `partial_match` (both inside step 6) occur much later.
2. **BC-3.8.017** — `--markdown` + `--field description=<value>` conflict guard — exit 64, NO HTTP. Fires when any raw `--field` token's key (substring before first `=`, NO trim, NO case-fold) is exactly `"description"` — case-SENSITIVE exact match mirroring `parse_field_kv`.
3. Existing `--markdown`-requires-`--description` guard — exit 64, NO HTTP.
4. `require_service_desk` — FIRST HTTP call in `handle_jsm_create`.
5. BC-3.8.010/BC-3.8.011 platform-only flag warnings — all six warnings (the `--type` warning of BC-3.8.010 plus the five platform-only flag warnings of BC-3.8.011) fire only AFTER `require_service_desk` returns `Ok`. The existing pre-dispatch warning block in `handle_create` MUST be removed — warnings exist at exactly ONE site (this step).
6. Numeric-bypass check → `resolve_jsm_request_type_id` (non-numeric input) → summary resolution, then description resolution (both in `handle_jsm_create`, after request-type resolution) → `parse_field_kv` → POST.

Guards 1 and 2 fire after project-key resolution (step 0) and before `require_service_desk` (step 4) — zero HTTP when either fires.

---

#### BC-3.8.016: `--request-type ""` or whitespace-only value exits 64 before `require_service_desk` with explicit message

**Confidence**: HIGH
**Subject**: Issue write (JSM path — input guard)
**Behavior**: When `--request-type` is set to the empty string or a whitespace-only string (i.e., the user passes `--request-type ""` or `--request-type "   "`), `handle_jsm_create` MUST detect the empty-or-whitespace-only input AFTER project-key resolution (step 0) but BEFORE `require_service_desk` (step 4). Guard ordering: see the Canonical Guard Ordering for subdomain 3.8 above (this guard is step 1).

Exit code: 64. Stderr contains: `"request type cannot be empty"` (**CANONICAL SOURCE — all duplicate occurrences in prd-delta-385.md, holdout-scenarios.md, and spec-changelog.md MUST be updated together with this copy; cf. JR_* doc-fallout pattern in CLAUDE.md**) (assert via `contains`). No HTTP calls are issued. The guard evaluates `request_type_arg.trim().is_empty()` — it rejects empty-or-whitespace-only values. The un-trimmed value is passed downstream UNCHANGED if the guard does NOT fire; this BC does NOT normalize or trim the value for downstream use. Consequently, non-empty whitespace-padded values (e.g. `--request-type " 5 "`) are OUT OF SCOPE for this BC and are EXPLICITLY DEFERRED out of #385 scope — they pass this guard and the un-trimmed value proceeds to step 6, where `" 5 "` fails the numeric-bypass check (not all-digits) and falls into `partial_match`. The current outcome is a potentially confusing "request type not found" error (because `" 5 "` is unlikely to substring-match any request type name), not a clean exit. This is a KNOWN RESIDUAL edge case — deferred, not benign.
**Inputs**: `--request-type ""` or `--request-type "   "` (empty or whitespace-only after trim); whitespace-padded non-empty values are out of scope for this BC.
**Outputs/Effects**: exit 64; stderr contains "request type cannot be empty" (substring match via `contains` — duplicated from the CANONICAL copy above; update together); stdout empty; no HTTP.
**Errors**: This BC IS the error contract. No downstream resolution attempted.
**Trace**: `tests/issue_create_jsm.rs::test_jsm_create_empty_request_type_exits_64` (integration test — H-NEW-JSM-RT-006 realized_by binding); `src/cli/issue/create.rs::handle_jsm_create` (guard at very top, before `require_service_desk`)
**Source**: O-08-04 CONFIRMED in `.factory/research/issue-288-pr4-deferred-validation.md`. Without this guard, `--request-type ""` falls through to `resolve_jsm_request_type_id` → `partial_match("", &names)` → returns `Ambiguous` for any NON-EMPTY candidate list (and `None` for an empty one) — either outcome produces a misleading message. See `src/partial_match.rs::partial_match` (substring-match branch): `"<anything>".contains("")` is `true` for all candidates, so every name in a non-empty list matches the empty string.
**Confidence**: HIGH

[NEW 2026-05-20 issue #385 F2] Closes O-08-04: empty `--request-type` guard. Guard fires at top of `handle_jsm_create` before `require_service_desk` — no HTTP can be issued.

[UPDATED 2026-05-20 issue #385 adversary pass-1 F-01/F-03/F-08] Placement strengthened from "before `resolve_jsm_request_type_id`" to "at the VERY TOP of `handle_jsm_create`, before `require_service_desk`" — ensuring zero HTTP on this path. Canonical guard ordering list added. Assertion mode made explicit: stderr asserted via `contains` of substring "request type cannot be empty".

[UPDATED 2026-05-20 issue #385 adversary pass-2 F-01] Scope clarified: guard tests `trim().is_empty()` only; it does NOT normalize the value for downstream use. Non-empty whitespace-padded values (e.g. `" 5 "`) are OUT OF SCOPE — they pass the guard and follow existing pre-#385 resolution behavior.

[UPDATED 2026-05-20 issue #385 adversary pass-3 H-01/H-05] Wording corrected: guard fires at step 1, before `require_service_desk` (step 4); numeric-bypass check and `partial_match` occur at step 6, not near the handler top — removed any phrasing implying otherwise. CANONICAL SOURCE designation added to the "request type cannot be empty" message string.

---

#### BC-3.8.017: `--markdown` + `--field description=<value>` combination rejected at the top of `handle_jsm_create`; exit 64

**Confidence**: HIGH
**Subject**: Issue write (JSM path — input guard)
**Behavior**: When `handle_jsm_create` detects both (a) `--markdown` is set AND (b) the raw `--field` arg list contains an entry whose key (first `=`-delimited token) is `"description"`, the handler MUST reject the combination AFTER project-key resolution (step 0) but BEFORE `require_service_desk`. Guard ordering: see the Canonical Guard Ordering for subdomain 3.8 above (this guard is step 2).

Guard 2 (this BC) uses a RAW first-`=`-split on each `--field` token — full `parse_field_kv` is not required for the conflict check. The key check is: any `--field` token where the raw substring before the first `=` (NO trimming, NO case-folding) is EXACTLY `"description"` — case-SENSITIVE, no-trim match, identical to how `parse_field_kv` extracts the key. This check is performed BEFORE `require_service_desk` so that NO HTTP is issued when the conflict is present. The guard fires if and only if the raw key equals `"description"` exactly — so `--field Description=X` (key `Description`) and `--field " description"=X` (key `" description"`) do NOT trigger the guard and are not a desync (HashMap key `Description` does not overwrite `requestFieldValues["description"]`).

The guard fires whenever `--markdown` is set AND a `--field description=…` is present — regardless of whether `--description` is also present. (The guard sits at step 2 above, BEFORE the existing `--markdown`-requires-`--description` guard at step 3. So `--markdown --field description=X` with NO `--description` flag correctly triggers THIS guard's conflict message, not the "requires --description" message.)

Exit code: 64. Stderr message (verbatim — **CANONICAL SOURCE; all duplicate occurrences in prd-delta-385.md, holdout-scenarios.md, and spec-changelog.md MUST be updated together with this copy; cf. JR_* doc-fallout pattern in CLAUDE.md**):
"`--field description=...` cannot be combined with `--markdown`: it would overwrite the ADF description with plain text, desyncing `isAdfRequest: true` with a plain-string description value (may result in a JSM 400 error or silently dropped ADF formatting). Pass `--description` with `--markdown`, or omit `--markdown`."
No HTTP calls are issued on this path.

When `--markdown` is absent, the guard does NOT fire — `--field description=value` without `--markdown` is permitted (it populates `requestFieldValues["description"]` as a plain string with `isAdfRequest: false` or omitted, which is coherent). When no `--field` token has a raw key exactly equal to `"description"`, the guard does NOT fire — `--markdown` alone (with `--description` or `--description-stdin`) is the normal ADF path. The guard does not inspect the description source (`--description` vs `--description-stdin`): if `--markdown` is set and a `--field` token has the raw key exactly `"description"`, the guard fires regardless of which description-source flag was used (EC-3.8.017-4). `--field Description=X` (capital D) + `--markdown` does NOT trigger the guard — raw key `Description` does not equal `"description"`; no desync occurs because HashMap key `Description` does not overwrite `requestFieldValues["description"]` (EC-3.8.017-3). A `--field` token with NO `=` character at all (e.g. `--field description`) does NOT trigger this guard — the raw first-`=`-split check requires a `=`-present form to extract a key; a no-`=` token has no extractable key and therefore never satisfies the conflict condition (EC-3.8.017-5). The downstream outcome depends on other flags: if a description source (`--description` or `--description-stdin`) is also present (e.g. `--markdown --description "X" --field description`), step 3 is satisfied and the no-`=` token reaches `parse_field_kv` at step 6, which surfaces the existing malformed-pair error; if NO description source is present alongside `--markdown`, the step-3 `--markdown`-requires-`--description` guard fires first. In both cases, BC-3.8.017's step-2 guard does not fire.

**Rationale**: `JsmRequestBuilder::build()` populates `requestFieldValues["description"]` with the ADF object during description handling and computes `is_adf_request = true`; it then iterates `extra_fields`, and an `extra_fields` entry keyed exactly `"description"` overwrites the ADF value with a plain string; `isAdfRequest: true` is still emitted in the final body — producing the desync. This desync may produce a JSM 400 error OR silently drop ADF formatting — the exact Atlassian behavior is not documented and must not be asserted. Parse-time rejection is the correct fix.
**Inputs**: `--markdown` flag set AND `--field <key>=<any value>` where the raw `<key>` (substring before first `=`, NO trimming, NO case-folding) is exactly `"description"` — case-SENSITIVE, no-trim match. `--field Description=X` (key `Description`) does NOT trigger this guard.
**Outputs/Effects**: exit 64; stderr contains the conflict message (assert via `contains`); stdout empty; no HTTP.
**Errors**: This BC IS the error contract. The rejection happens at the top of `handle_jsm_create` before `require_service_desk`.
**Trace**: `tests/issue_create_jsm.rs::test_jsm_create_markdown_field_description_conflict_exits_64` (integration test — H-NEW-JSM-RT-007 realized_by binding); `src/cli/issue/create.rs::handle_jsm_create` (guard at top, before `require_service_desk`)
**Source**: O-08-06 PARTIAL in `.factory/research/issue-288-pr4-deferred-validation.md`. The "may produce a JSM 400 OR silently drop ADF" phrasing is intentional per CLAUDE.md citation discipline — this spec MUST NOT assert "Atlassian returns 400" because the exact server behavior is undocumented. The guard rationale is the desync, not a confirmed 400.
**Confidence**: HIGH

[NEW 2026-05-20 issue #385 F2] Closes O-08-06: `--markdown` + `--field description=` conflict guard. Guard is in `handle_jsm_create` (not in `JsmRequestBuilder::build()`), preserving `JsmRequestBuilder` as a pure builder with no validation responsibility. Conflict guard in `build()` would require extending `tests/jsm_request_api.rs` proptest suite — caller-side placement keeps that suite unchanged.

[UPDATED 2026-05-20 issue #385 adversary pass-1 F-01/F-03/F-04] Placement strengthened: guard sits at the VERY TOP of `handle_jsm_create` before `require_service_desk` (no HTTP). Guard ordering listed explicitly. Guard fires whenever `--markdown` + `--field description=…` is present regardless of whether `--description` is also set (guard precedes the `--markdown`-requires-`--description` guard). Raw first-`=`-split is sufficient — full `parse_field_kv` not required for the conflict check. EC-3.8.017-1 updated accordingly.

[UPDATED 2026-05-20 issue #385 adversary pass-3 H-02/H-03] Key matching changed from case-SENSITIVE literal `"description"` to case-INSENSITIVE (`key.trim().to_ascii_lowercase() == "description"`). Removed the uncited claim that JSM field names are case-sensitive. EC-3.8.017-3 updated: `--field Description=X` now DOES trigger the guard. EC-3.8.017-4 added: `--markdown --description-stdin --field description=X` → guard fires; guard does not inspect `--description`/`--description-stdin` source.

[UPDATED 2026-05-20 issue #385 adversary pass-5 M-03] EC-3.8.017-5 added: a `--field` token with NO `=` character does NOT trigger this guard — no extractable key means the conflict condition is never satisfied. Non-triggering-cases paragraph updated to reference EC-3.8.017-5 and describe two possible downstream outcomes (step-6 malformed-pair error when a description source is present; step-3 markdown-requires-description guard when no description source is present).

[UPDATED 2026-05-20 issue #385 adversary pass-11 H-1] Key matching REVERSED from case-INSENSITIVE (pass-3 H-02) to case-SENSITIVE, no-trim — the guard MUST mirror `parse_field_kv`'s raw key extraction (`pair[..eq_pos]`, no `.trim()`, no case-folding) and HashMap exact-overwrite semantics. The desync (`extra_fields["description"]` overwrites `requestFieldValues["description"]`) occurs ONLY when the raw key is exactly `"description"`. `--field Description=X` produces HashMap key `Description`, which does NOT overwrite `requestFieldValues["description"]` — no desync, guard does NOT fire. The pass-3 H-02 case-insensitive framing was based on the incorrect premise that a differently-cased key could produce the desync. EC-3.8.017-3 updated: `--field Description=X` does NOT trigger the guard. Inputs field, non-triggering-cases paragraph, and all guard-match descriptions updated to remove "case-insensitive"/"trim" wording.

---

## JSON Output Shape Contracts (all confirmed by insta snapshots)

| Operation | JSON shape | Key field note |
|-----------|-----------|---------------|
| `move` (changed) | `{"changed": true, "key": "TEST-1", "status": "In Progress"}` | 3 keys alphabetical |
| `move` (unchanged) | `{"changed": false, "key": "TEST-1", "status": "Done"}` | idempotent form |
| `assign` (changed) | `{"assignee": "Jane Doe", "assignee_account_id": "abc123", "changed": true, "key": "TEST-1"}` | `assignee_account_id` snake_case |
| `assign` (unchanged) | identical with `changed: false` | |
| `unassign` | `{"assignee": null, "changed": true, "key": "TEST-1"}` | `assignee` is EXPLICIT null |
| `edit` | `{"changed_fields": {...}, "key": "TEST-1", "updated": true}` | 3 keys; `changed_fields` is a BTreeMap-ordered object |
| `link` | `{"key1": "TEST-1", "key2": "TEST-2", "linked": true, "type": "Blocks"}` | symmetric key1/key2 |
| `unlink` | `{"count": 2, "unlinked": true}` | `count: 0` when no match |
| `remote-link` | `{"id": 10000, "key": "TEST-1", "self": <url>, "title": <title>, "url": <url>}` | 5 keys |
| `create` | `{"key": "FOO-123"}` | minimal |

Sources: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__*.snap`; BC-1104..BC-1112 (R4)

## Total BCs in this file: 74 individually-bodied (cumulative 103 incl. range-collapsed; see BC-INDEX.md)

_Last updated 2026-05-25 (issue #407 F2): +EC-3.4.017-14 — mechanical enforcement meta-test for BC-3.4.017 invariant 2 (conflict block completeness via `test_label_conflict_block_lists_every_relevant_flag`); BC-3.4.017 invariant 2 cross-reference added; no BC count changes (103/74 unchanged). Previous update (2026-05-22 issue #396 F2): +3 BCs (BC-3.4.015..017) — BC-3.4.015 (`issue edit --field` string/number/date/datetime/user field single-key path, with editmeta validation, fields.json cache, and dry-run invariants), BC-3.4.016 (`issue edit --field` single-select `option` field), BC-3.4.017 (`--field` multi-key/`--jql` rejection Gate A and flag-overlap Gate B); Section 3.4 header updated to 17 contracts. Previous update (2026-05-21 issue #398 F2): +3 BCs (BC-3.4.012..014) — BC-3.4.012 (issue edit table-mode success echo), BC-3.4.013 (issue edit JSON-mode success echo with changed_fields), BC-3.4.014 (issue create table-mode all-fields echo (broadened from team-only at the 2026-05-22 human-gate to mirror BC-3.4.012)); BC-3.4.003 Success output cross-reference added; Section 3.4 header updated to 14 contracts. Previous update (2026-05-20 issue #388): +2 BCs (BC-3.4.010..011): BC-3.4.010 (cross-hierarchy `edit --type` 400 → CROSS_HIERARCHY_HINT citing JRACLOUD-27893) and BC-3.4.011 (same-hierarchy/indeterminate `edit --type` 400 → typo hint or raw error, no JRACLOUD-27893 hint) added in F2 delta (issue #388). BC-3.4.003 Errors cross-reference updated (annotation only, no behavioral change). Section 3.4 header updated to 11 contracts. Previous update (2026-05-20 issue #385): +2 BCs (BC-3.8.016..017); BC-3.8.002/010/011 modified._
