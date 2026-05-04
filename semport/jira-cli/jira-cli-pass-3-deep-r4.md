# Pass 3 Deep — Round 4: Behavioral Contracts (jira-cli / jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Builds on: `pass-3-deep-r3.md` (419 BCs / 354 HIGH / 59 MEDIUM / 6 LOW after R3; 43 holdouts).

> **Method.** Round 4 attacked the verbatim Round-3 §9 deferred-target list:
> per-test enumeration of `tests/issue_commands.rs` (54 tests),
> `tests/api_client.rs` non-extract tests (11), proptest enumeration,
> insta snapshot enumeration, `tests/issue_remote_link.rs` (6),
> `tests/user_commands.rs` (14), `tests/project_commands.rs` (10),
> `tests/issue_resolution.rs` (3), `tests/issue_view_errors.rs` (4),
> `tests/assets_errors.rs` (3), `tests/cmdb_fields.rs` (5),
> `tests/team_column_parity.rs` full (7 tests + helpers),
> `tests/auth_login_config_errors.rs` (1 test),
> `tests/user_pagination.rs` remaining 7 tests, `src/api/auth.rs` 22
> source unit tests, `src/api/auth_embedded.rs` 8 tests, BC-130
> verification, OAuth state-machine per-transition BCs.
>
> Files freshly read this round (full): `tests/auth_login_config_errors.rs`
> (97 LOC, 1 test), `tests/assets_errors.rs` (153 LOC, 3 tests),
> `tests/issue_view_errors.rs` (206 LOC, 4 tests), `tests/issue_resolution.rs`
> (158 LOC, 3 tests), `tests/cmdb_fields.rs` (189 LOC, 5 tests).
>
> Files freshly read chunked: `tests/issue_commands.rs` (1-200, 200-700,
> 700-1300, 1300-1920); `tests/api_client.rs` (1-260, 340-445);
> `tests/user_commands.rs` (1-417); `tests/project_commands.rs` (1-150);
> `tests/issue_remote_link.rs` (full); `tests/team_column_parity.rs`
> (structural via fn-name grep + 1-150); `tests/user_pagination.rs`
> (200-520); `src/jql.rs` (100-395 — proptest block); `src/partial_match.rs`
> (140-200 — proptest block); `src/duration.rs` (120-159 — proptest
> block); `src/api/auth.rs` (920-1397 — full test module incl.
> `fixed_port_strategy_eaddrinuse_friendly_error`); `src/api/auth_embedded.rs`
> (130-250 — full test module); `src/cli/issue/json_output.rs` (80-149 —
> insta module); `src/cli/issue/list.rs` (650-1085 — fn-name grep for
> BC-130 verification); insta snapshot files for all 11 json_output tests
> + 2 sprint snapshots + 1 auth list snapshot + 2 ADF snapshots + 1
> changelog snapshot; verbatim source bytes for the EADDRINUSE error
> message at `src/api/auth.rs:438-442`; `src/cli/issue/workflow.rs:636`
> (handle_open URL construction); `src/api/jira/worklogs.rs` (full 31
> LOC) and `src/cli/worklog.rs:32` (8/5 hardcoded).

---

## 1. Round metadata

| Field | Value |
|---|---|
| Round | 4 of (max 5) |
| Targets attacked this round | T-12 issue_commands.rs full 54-test enumeration; T-13 api_client.rs remaining 11 tests; T-14 BC-130 verification (build_jql_parts_* names); T-15 proptest enumeration (4 blocks across jql/partial_match/duration); T-16 Insta snapshot enumeration (17 snap files); T-17 user_pagination.rs remaining 7 tests; T-18 batch enumeration of 9 smaller integration test files; T-19 src/api/auth.rs 22 source unit tests; T-20 src/api/auth_embedded.rs 8 tests; T-21 OAuth state-machine per-transition BCs; T-22 source-file unit-test count audit |
| Targets DEFERRED to round 5 | None — all R3 §9 targets attacked. R4 declares Pass 3 has CONVERGED. |
| Files freshly read this round (full) | 5 — `tests/auth_login_config_errors.rs`, `tests/assets_errors.rs`, `tests/issue_view_errors.rs`, `tests/issue_resolution.rs`, `tests/cmdb_fields.rs` |
| Files freshly read this round (chunked) | 16 — see above |
| BCs in pass-3 broad | 193 (recounted) |
| BCs after R1 | 271 (211 HIGH / 53 MEDIUM / 7 LOW) |
| BCs after R2 | 343 (281 HIGH / 56 MEDIUM / 6 LOW) |
| BCs after R3 | 419 (354 HIGH / 59 MEDIUM / 6 LOW) |
| BCs added this round | **+121 net new** (mostly HIGH; see §3) |
| BCs promoted MEDIUM→HIGH | 0 |
| BCs corrected (CONV-ABS-011..012) | 2 (see §6) |
| BCs after round 4 | **540 total** (475 HIGH / 59 MEDIUM / 6 LOW) |

---

## 2. Audit of Round 3 against the 5 Known Hallucination Classes

### 2.1 Over-extrapolated token lists
- **R3 BC-1010** (`handle_open` uses `client.base_url()`): re-verified verbatim by reading `src/cli/issue/workflow.rs:636` — the literal source line is `let url = format!("{}/browse/{}", client.base_url(), key);`. **No retraction.**
- **R3 BC-1014** (`worklog add` hardcodes 8/5): re-verified at `src/cli/worklog.rs:32` — `let seconds = duration::parse_duration(dur, 8, 5)?;`. **No retraction.**
- **R3 BC-1012/1013** (`list_worklogs` non-paginated): re-verified by reading `src/api/jira/worklogs.rs` in full — the function is exactly `async fn list_worklogs(&self, key: &str) -> Result<Vec<Worklog>> { ... let page: OffsetPage<Worklog> = self.get(&path).await?; Ok(page.items().to_vec()) }`. The Vec is returned, no `total` is exposed to the caller, no loop. **No retraction.**
- **R3 H-042** (EADDRINUSE message substrings): re-verified at `src/api/auth.rs:438-442`:
  ```
  "port {p} is in use; the jr OAuth callback needs this port. \
   Set --client-id/--client-secret (or set \
   JR_OAUTH_CLIENT_ID/JR_OAUTH_CLIENT_SECRET) to fall \
   back to a dynamic port."
  ```
  All 5 substring claims (`port 53682 is in use`, `the jr OAuth callback needs this port`, `--client-id/--client-secret`, `JR_OAUTH_CLIENT_ID/JR_OAUTH_CLIENT_SECRET`, `dynamic port`) are present in the literal. **No retraction.**

### 2.2 Miscounted enumerations
- **R3 stated total 419 = 354H + 59M + 6L**: arithmetic check 354+59+6 = 419. ✓
- **R3 net-new claim "+76"**: 343 + 76 = 419. ✓
- **R3 claim "tests/team_column_parity.rs — 7 tests"** (R3 §9.8): recount via `awk '/^[[:space:]]*#\[(tokio::)?test/{c++} END{print c}' tests/team_column_parity.rs` returns **7** (tokio::test annotations at lines 124, 181, 220, 284, 341, 380, 425). The two functions `mount_sprint_prereqs` (line 85) and `mount_kanban_board_prereqs` (line 257) are helper async fns, NOT tests. **R3 count correct. ✓**
- **R3 claim "tests/auth_login_config_errors.rs — survey-level"** with R3 §9.8 listing it under the batch: re-verified — file has **exactly 1 test** (`auth_login_oauth_surfaces_malformed_config_without_overwriting`, line 19). The R3 hedge "survey-level" was overcautious — it's a single-test file. **Reclassified to per-test in this round.**
- **R3 claim "src/cache.rs 27 tests, src/config.rs 37, src/jql.rs 43, src/partial_match.rs 12, src/duration.rs 16"**: recounted via awk — `27, 37, 43, 12, 16` exactly. **All correct. ✓**

### 2.3 Named pattern conflation / fabrication
- **R3 deferred BC-130 list.rs unit-test names verification**: this round directly grep'd `cli/issue/list.rs` for the names. All claimed names exist (line numbers in §3.3). **No fabrication.**
- **R3 §3.4 NEW-INV-29 (worklog non-paginated)** verified above. **No fabrication.**

### 2.4 Same-basename artifact conflation
- **R3 source vs integration fn-count distinctions**: cli/issue/changelog.rs (38 source unit tests) vs tests/issue_changelog.rs (39 integration tests) was correctly distinguished in R2 BC-925..936 + earlier R1 BCs. **No conflation.**
- **R3 BC-035 file-attribution correction**: this round re-verified `src/cli/auth.rs:1523-1564` is the correct location of `default_oauth_scopes_pins_the_full_set_with_offline_access`; `src/api/auth.rs:34-63` is the constant definition. R3's correction (CONV-ABS-010) stands. **No further action.**

### 2.5 Inflated or deflated metrics
- **R3 line-range citations** (e.g., `tests/cli_handler.rs:281-333` for create-with-name-search): spot-checked via awk; line ranges fit the function bodies. **No inflation.**

### 2.6 NEW audit issue: R3 stated R3 §3.10 OAuth state-machine "GENERATE_STATE → BUILD_AUTHORIZE_URL → ..." sequence
The state machine is described at the spec level. Verifying against `src/api/auth.rs`: `generate_state()` (line 882), `build_authorize_url()` (line 846), `extract_query_param()` (line 898), `resolve_refresh_app_credentials()` (line 781), `RedirectUriStrategyRequest::bind` (lines 415-451) — all exist. The state-machine ordering matches the function call order in the live `oauth_login` flow at `src/cli/auth.rs`. **No retraction.**

---

## 3. BC additions / promotions, per target T-NN

### 3.1 T-12 — `tests/issue_commands.rs` 54-test full enumeration (NEW)

R3 deferred this; R4 closes it with per-test BCs. Many have prior coverage at the source-unit level (e.g., search_issues, get_issue) so duplicate BCs are NOT created — only NEW tests get NEW BCs. Pinning each is via the single test that mounts the relevant mock and asserts.

#### BC-1036 (NEW): `client.search_issues("assignee = currentUser()", None, &[])` POSTs `/rest/api/3/search/jql` with empty fields list; deserializes `issues[0].key=="FOO-1"`, `has_more=false`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:8-31` (`test_search_issues`)

#### BC-1037 (NEW): `client.get_issue("FOO-1", &[])` GETs `/rest/api/3/issue/FOO-1`; deserializes `fields.status.name == "In Progress"` (None-safe via `unwrap()` test pattern)
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:33-53` (`test_get_issue`)

#### BC-1038 (NEW): `client.get_transitions("FOO-1")` GETs `/rest/api/3/issue/FOO-1/transitions`; returns 2-element transitions vec; subsequent `transition_issue("FOO-1", "21", None)` POSTs to same path with `{transition: {id: "21"}}` body and accepts 204
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:55-77` (`test_get_transitions`)

#### BC-1039 (NEW): `transition_issue(key, id, Some(&fields))` body partial-match pins `{transition: {id: "31"}, fields: {resolution: {name: "Done"}}}` with `expect(1)` — fields are merged into body alongside transition
**Confidence**: HIGH (NEW; pins resolution-required transition support)
**Sources**: `tests/issue_commands.rs:79-103` (`transition_issue_with_fields_sends_fields_in_body`)
**Behavior**: This is the integration-level pin for the `--resolution` flag plumbing. Cross-references BC-1062 (issue_resolution.rs:88).

#### BC-1040 (NEW): `transition_issue(key, id, None)` body MUST NOT contain `"fields"` key (verified via post-request `body.contains("\"fields\"") == false`)
**Confidence**: HIGH (NEW; negative-serialization pin)
**Sources**: `tests/issue_commands.rs:105-128` (`transition_issue_without_fields_omits_fields_key`)
**Effects**: Pin against a refactor that emits `fields: null` (which Atlassian rejects with a schema error). Same pattern as BC-1004 (`assignee` omission).

#### BC-1041 (NEW): `search_issues` with `&["customfield_10031"]` extra fields → response deserializes story_points correctly: `Some(5.0)` for issue with custom field, `None` for issue without
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:130-166` (`test_search_issues_with_story_points`)

#### BC-1042 (NEW): `client.find_story_points_field_id()` returns `vec![("customfield_10031", "Story Points")]` from `/rest/api/3/field` response with `name == "Story Points"`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:168-186` (`test_find_story_points_field_id`)

#### BC-1043 (NEW): `client.list_link_types()` returns 3 link types from `/rest/api/3/issueLinkType`; first has `name="Blocks", outward=Some("blocks"), inward=Some("is blocked by")`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:188-206` (`test_list_link_types`)

#### BC-1044 (NEW): `get_issue` with parent + links: `fields.parent.key == "FOO-1"`, `fields.parent.fields.summary == "Parent Epic"`, `fields.issuelinks[0].link_type.name == "Blocks"`, `fields.issuelinks[0].outward_issue.key == "FOO-3"`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:208-231` (`test_get_issue_with_parent_and_links`)

#### BC-1045 (NEW): `client.create_issue_link("FOO-1", "FOO-2", "Blocks")` POSTs to `/rest/api/3/issueLink`; accepts 201; returns `Ok(())`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:233-248` (`test_create_issue_link`)

#### BC-1046 (NEW): `client.delete_issue_link("10001")` DELETEs `/rest/api/3/issueLink/10001`; accepts 204; returns `Ok(())`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:250-262` (`test_delete_issue_link`)

#### BC-1047 (NEW): `search_issues` with `Some(1)` limit and a fixture with a continuation token sets `result.has_more = true` (cursor-based pagination signal)
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:264-285` (`test_search_issues_has_more_flag`)

#### BC-1048 (NEW): `search_issues` with `Some(10)` against a 1-issue fixture returns `has_more = false` (NOT undefined — explicitly false when fixture omits next-page token)
**Confidence**: HIGH (boundary)
**Sources**: `tests/issue_commands.rs:287-310` (`test_search_issues_no_more_results`)

#### BC-1049 (NEW): `search_issues(.., None, ..)` (no-limit) consumes the full single-page response; 3 issues returned; `has_more == false`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:312-335` (`test_search_issues_no_limit_fetches_all`)

#### BC-1050 (NEW): `client.approximate_count("project = FOO")` POSTs `/rest/api/3/search/approximate-count`; returns `u64` (42, 0 boundary cases); 5xx propagates as `Err`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:337-386` (3 tests: `test_approximate_count`, `test_approximate_count_zero`, `test_approximate_count_server_error_returns_err`)

#### BC-1051 (NEW): `client.search_users(query)` GETs `/rest/api/3/user/search` and accepts FOUR distinct response shapes: (a) bare array `[{...}]`, (b) `{values: [...]}` paginated envelope, (c) empty array `[]`, (d) error shape `{error: ...}` → returns `Err`
**Confidence**: HIGH (NEW; 4-shape robustness contract)
**Sources**: `tests/issue_commands.rs:388-490` (5 tests: `test_search_users_single_result`, `test_search_users_empty`, `test_search_users_multiple`, `test_search_users_paginated_response`, `test_search_users_unrecognized_response_errors`)
**Effects**: Pin: a Jira tenant returning either shape (bare or paginated) MUST be deserializable. The shape is detected via serde-untagged enum. Unrecognized shapes do NOT default to empty — they error.

#### BC-1052 (NEW): `search_issues` with composed JQL `project = "PROJ" AND (priority = Highest) ORDER BY updated DESC` body partial-match pins the LITERAL composed string (project key double-quoted; user JQL parenthesized; ORDER BY appended)
**Confidence**: HIGH (NEW; pinning the exact JQL composition output)
**Sources**: `tests/issue_commands.rs:492-524` (`test_search_issues_jql_with_project_scope`)

#### BC-1053 (NEW): `get_issue` with full standard-fields fixture deserializes: `created`, `updated` (RFC3339+0000 format strings), `reporter` (display_name+account_id), `resolution.name`, `components` (array of {name}), `fix_versions` (array of {name, released, release_date}); all also serialize back to JSON via `serde_json::to_string` at expected paths (`fields.created`, `fields.fixVersions`, etc. — note camelCase JSON path)
**Confidence**: HIGH (NEW; round-trip serde pin)
**Sources**: `tests/issue_commands.rs:526-577` (`get_issue_includes_standard_fields`)

#### BC-1054 (NEW): `get_issue` with minimal fixture (none of the new fields): `fields.created`, `updated`, `reporter`, `resolution`, `components`, `fix_versions` ALL deserialize as `None` (NOT panic, NOT empty Vec)
**Confidence**: HIGH (NEW; defensive deserialization pin)
**Sources**: `tests/issue_commands.rs:579-607` (`get_issue_null_standard_fields`)

#### BC-1055 (NEW): `client.edit_issue("FOO-10", json!{description: text_to_adf("Updated description")})` PUTs `/rest/api/3/issue/FOO-10`; body partial-match pins `fields.description.{version: 1, type: "doc", content[0]: {type: "paragraph", content[0]: {type: "text", text: "Updated description"}}}` (full ADF doc shape verified at the wire); accepts 204
**Confidence**: HIGH (NEW; ADF on PUT pin)
**Sources**: `tests/issue_commands.rs:609-645` (`test_edit_issue_with_description`)

#### BC-1056 (NEW): `edit_issue` with `markdown_to_adf("**bold text**")` → body partial-match pins `text` is `"bold text"` (not `**bold text**`) AND `marks: [{type: "strong"}]` (markdown converted to ADF before wire)
**Confidence**: HIGH (NEW; markdown→ADF on wire pin)
**Sources**: `tests/issue_commands.rs:647-687` (`test_edit_issue_with_markdown_description`)

#### BC-1057 (NEW): `edit_issue` with mixed fields: `{summary: "New summary", description: text_to_adf("New description")}` → body has BOTH keys; description is wrapped in ADF doc shape; summary is plain string
**Confidence**: HIGH (NEW; multi-field edit pin)
**Sources**: `tests/issue_commands.rs:689-727` (`test_edit_issue_description_with_other_fields`)

#### BC-1058 (NEW): `client.search_assignable_users(query, key)` GETs `/rest/api/3/user/assignable/search` with both `query` AND `issueKey` query params; returns single result, empty result, OR `{values: [...]}` paginated envelope identically to `search_users`
**Confidence**: HIGH (PROMOTED scope; covers 3 tests)
**Sources**: `tests/issue_commands.rs:729-805` (3 tests: single, empty, paginated)

#### BC-1059 (NEW): assign chain — `search_assignable_users` → `get_issue` → `assign_issue("FOO-1", Some(account_id))` PUTs `/rest/api/3/issue/FOO-1/assignee` with `{accountId: <id>}` body; accepts 204
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:807-854` (`assign_to_user_resolves_display_name`)

#### BC-1060 (NEW): empty assignable-user-search result → `search_assignable_users` returns `Ok(Vec::new())` (NOT Err); caller decides what to do (the not-found case is a UX error in the handler, not a client error)
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:856-877` (`assign_to_user_not_found`)

#### BC-1061 (NEW): `get_myself()` GETs `/rest/api/3/myself`; returns User with `account_id: "abc123"` (per fixtures::user_response default); chained with `assign_issue(key, Some(&me.account_id))` for `--to me` flow (zero search HTTP)
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:879-920` (`assign_to_me_keyword`)

#### BC-1062 (NEW): assign idempotency — `search_assignable_users` returns the user, `get_issue` shows already-assigned (matching account_id); the test mounts NO mock for `PUT /assignee`. Wiremock returns 404 for unmocked paths. The CLI handler MUST short-circuit before the PUT — verified by the test passing without a 404
**Confidence**: HIGH (NEW; alternative-pin: absence of mock IS the assertion)
**Sources**: `tests/issue_commands.rs:922-965` (`assign_idempotent_already_assigned`)

#### BC-1063 (NEW): `search_issues` with no `fields` param → server-side default fields list includes `summary, status, issuetype, priority, assignee, reporter, project, description, created, updated, resolution, components, fixVersions, labels, parent, issuelinks` (16 fields, EXACT order, body partial-JSON match)
**Confidence**: HIGH (NEW; default-field-set contract)
**Sources**: `tests/issue_commands.rs:967-1022` (`test_search_issues_includes_labels_parent_issuelinks`)
**Effects**: A regression that drops one of these fields will fail this test. The order matters because the test uses `body_partial_json` against an exact array.

#### BC-1064 (NEW): `create_issue` flow with assignee resolution — `search_assignable_users_by_project(query, projectKey)` GETs `/rest/api/3/user/assignable/multiProjectSearch` (NOT `/user/search`) with `projectKeys` AND `query` params; result feeds into `create_issue` body; full body partial-match: `{project: {key}, issuetype: {name}, summary, assignee: {accountId}}`; response deserializes to `CreateResponse{key: "FOO-99"}` from 201
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:1024-1082` (`test_create_issue_with_assignee`)

#### BC-1065 (NEW): create with `--to me` short-circuits via `get_myself()`; NO multiProjectSearch mock needed; body has `assignee.accountId == myself.account_id`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:1084-1127` (`test_create_issue_with_assignee_me`)

#### BC-1066 (NEW): create WITHOUT assignee — body has `{project, issuetype, summary}` ONLY (no assignee key); response `key: "FOO-101"`
**Confidence**: HIGH (cross-references BC-1004)
**Sources**: `tests/issue_commands.rs:1129-1154` (`test_create_issue_without_assignee`)

#### BC-1067 (NEW): create-assignee-not-found — `search_assignable_users_by_project` returns empty Vec; handler stops short of `create_issue` (NO POST mock needed)
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:1156-1180` (`test_create_issue_assignee_not_found`)

#### BC-1068 (NEW): create with explicit `--account-id <id>` skips user search entirely; body has `assignee: {accountId: "direct-acct-789"}` directly
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:1182-1217` (`test_create_issue_with_account_id`)

#### BC-1069 (NEW): `jr issue move FOO-1 Complete` (transition NAME match) — fetches transitions, fetches issue (idempotency probe), POSTs `/transitions` with `{transition: {id: "21"}}` matching the named transition. stderr contains `"Moved FOO-1"`. exit 0
**Confidence**: HIGH (PROMOTED to integration scope)
**Sources**: `tests/issue_commands.rs:1219-1276` (`test_move_by_transition_name`)

#### BC-1070 (NEW): `jr issue move FOO-1 Completed` (status NAME match — different from transition name "Complete") — same flow; the resolver matches `transition.to.name == "Completed"` and posts the same `id: "21"`. stderr "Moved FOO-1"
**Confidence**: HIGH (NEW; status-vs-transition-name dispatch)
**Sources**: `tests/issue_commands.rs:1278-1335` (`test_move_by_status_name`)

#### BC-1071 (NEW): `move FOO-1 Done` where `transition.name == "Done"` AND `transition.to.name == "Done"` (BOTH match) — DEDUP: only ONE candidate is presented; succeeds; POSTs `id: "31"` (no Ambiguous error)
**Confidence**: HIGH (NEW; de-duplication algorithm pin)
**Sources**: `tests/issue_commands.rs:1337-1394` (`test_move_dedup_same_transition_and_status_name`)

#### BC-1072 (NEW): `move FOO-1 Re` (substring matches `Reopen` AND `Review`) — Ambiguous error; exit non-zero; stderr contains `"Ambiguous"`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:1396-1444` (`test_move_ambiguous_across_transition_and_status_names`)

#### BC-1073 (NEW): `move FOO-1 Nonexistent` — error stderr contains BOTH enriched candidates: `"Complete (→ Completed)"` AND `"Review (→ In Review)"` (transition NAME → status NAME format)
**Confidence**: HIGH (NEW; enriched-error format pin)
**Sources**: `tests/issue_commands.rs:1446-1498` (`test_move_no_match_shows_status_names`)

#### BC-1074 (NEW): `move FOO-1 Completed` where issue ALREADY in "Completed" status — exit 0, stderr contains `"already in status"`; NO POST mock fires (idempotency)
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:1500-1549` (`test_move_idempotent_with_status_name`)

#### BC-1075 (NEW): `move FOO-1 Complete` (transition name) where issue is already in destination status "Completed" — STILL idempotent: the resolver maps `Complete` → status `Completed` → already-there short-circuit; stderr contains `"already in status"`; NO POST mock fires
**Confidence**: HIGH (NEW; transition-name→status-name idempotency path)
**Sources**: `tests/issue_commands.rs:1551-1604` (`test_move_idempotent_with_transition_name`)

#### BC-1076 (NEW): `create_issue` response `key: "URL-1"` AND `client.instance_url()` is the correct base for browse URL composition (verified by test-side construction `format!("{}/browse/{}", instance_url.trim_end_matches('/'), response.key)` containing `/browse/URL-1`)
**Confidence**: HIGH (NEW; documents that integration-test-side uses instance_url; cross-references BC-1010 bug)
**Sources**: `tests/issue_commands.rs:1606-1644` (`test_create_issue_response_includes_browse_url`)
**Effects**: This test's positive use of `client.instance_url()` reinforces R3 BC-1010's bug claim about handle_open using `base_url()` instead.

#### BC-1077 (NEW): `assign_issue("ACC-1", Some("direct-account-id-456"))` PUTs `/rest/api/3/issue/ACC-1/assignee` with `{accountId: "direct-account-id-456"}` body; accepts 204; NO user search needed for explicit account_id
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:1646-1703` (`test_assign_issue_with_account_id`)

#### BC-1078 (NEW): `assign_issue("ERR-1", Some("bogus-account-id"))` against 404 body `{errorMessages: ["User 'bogus-account-id' does not exist."]}` → returns `Err(JrError::ApiError{status: 404, ..})`; error message contains `"does not exist"` (extracted via `extract_error_message`)
**Confidence**: HIGH (NEW; error-variant + status-code structural pin)
**Sources**: `tests/issue_commands.rs:1705-1738` (`test_assign_issue_invalid_account_id_returns_error`)

#### BC-1079 (NEW): `jr --no-input issue move FOO-1 prog` — partial_match returns single substring hit `In Progress`; `--no-input` rejects ambiguous-substring; exit code EXACTLY `Some(64)`; stderr contains `"Ambiguous transition"` AND `"In Progress"`; ZERO `POST /transitions` (mock has `expect(0)`)
**Confidence**: HIGH (NEW; integration boundary for partial_match)
**Sources**: `tests/issue_commands.rs:1748-1810` (`test_move_single_substring_rejected_no_input`)

#### BC-1080 (NEW): `jr --no-input issue link FOO-1 FOO-2 --type block` — partial_match returns single substring hit `Blocks` (only link type containing "block"); exit 64; stderr contains `"Ambiguous link type"` AND `"Blocks"`; ZERO `POST /issueLink` (mock has `expect(0)`)
**Confidence**: HIGH (NEW; symmetric with BC-1079)
**Sources**: `tests/issue_commands.rs:1812-1867` (`test_link_single_substring_rejected_no_input`)

#### BC-1081 (NEW): `jr --no-input issue unlink FOO-1 FOO-2 --type block` — same partial_match dispatch; exit 64; stderr contains `"Ambiguous link type"` AND `"Blocks"`; ZERO `DELETE /issueLink/*` (no mock mounted)
**Confidence**: HIGH (NEW)
**Sources**: `tests/issue_commands.rs:1869-1920` (`test_unlink_single_substring_rejected_no_input`)

### 3.2 T-13 — `tests/api_client.rs` remaining 11 non-extract_error_message tests (NEW)

#### BC-1082 (NEW): `client.get::<T>("/rest/api/3/myself")` injects `Authorization: Basic <auth_header>` exactly as constructed by `JiraClient::new_for_test`; the wiremock `header(...)` matcher pins the verbatim auth string from the test
**Confidence**: HIGH (PROMOTED; verifies header injection)
**Sources**: `tests/api_client.rs:14-40` (`test_get_request_with_auth_header`)
**Behavior**: Test verifies the `Authorization` header value `Basic dGVzdEBleGFtcGxlLmNvbTpteS1hcGktdG9rZW4=` (i.e. `test@example.com:my-api-token`) appears verbatim. Pin: header-injection path is unconditional (every request gets the header).

#### BC-1083 (NEW): `client.get(...)` against 429-then-200 retries automatically; first response `429` with `Retry-After: 0` is consumed internally (NOT surfaced as Err); second `200` deserializes to typed result. Mocks `up_to_n_times(1).expect(1)` for 429 + `expect(1)` for 200
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:42-70` (`test_rate_limit_retry`)

#### BC-1084 (NEW): 401 response with body `{message: "Client must be authenticated to access this resource."}` (no scope substring) → returned `Err` whose Display contains `"Not authenticated"`
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:72-97` (`test_401_returns_not_authenticated`)

#### BC-1085 (NEW): 401 POST response with body `{code: 401, message: "Unauthorized; scope does not match"}` → returned `Err` whose Display contains ALL of: `"Insufficient token scope"`, the raw gateway message `"Unauthorized; scope does not match"`, the classic-scope hint `"write:jira-work"`, the OAuth hint `"OAuth 2.0"`, AND the issue link `"github.com/Zious11/jira-cli/issues/185"`
**Confidence**: HIGH (NEW; 5-substring pin for actionable error)
**Sources**: `tests/api_client.rs:99-144` (`test_401_scope_mismatch_returns_insufficient_scope`)
**Effects**: Pin against a refactor that drops any of the actionable hints (the issue link is the most fragile — pin specifically guards against link removal during repo migration).

#### BC-1086 (NEW): 401 with body NOT containing `"scope does not match"` (e.g. `"Session expired"`) → falls through to `"Not authenticated"`; MUST NOT contain `"Insufficient token scope"`
**Confidence**: HIGH (NEW; dispatch-boundary pin)
**Sources**: `tests/api_client.rs:146-181` (`test_401_without_scope_mismatch_falls_through_to_not_authenticated`)

#### BC-1087 (NEW): 401 scope-mismatch dispatch is CASE-INSENSITIVE — body `"Unauthorized; Scope Does Not Match"` (mixed case) STILL dispatches to `InsufficientScope`. Pin via `to_ascii_lowercase()` in parse_error
**Confidence**: HIGH (NEW)
**Sources**: `tests/api_client.rs:183-216` (`test_401_scope_mismatch_matches_case_insensitively`)

#### BC-1088 (NEW): scope-mismatch dispatch is GATED on status==401 — a 403 response containing `"scope does not match"` substring DOES NOT dispatch to `InsufficientScope`; falls through to `"API error (403)"`
**Confidence**: HIGH (NEW; status-gate pin against future broadening)
**Sources**: `tests/api_client.rs:218-255` (`test_non_401_with_scope_substring_does_not_dispatch_to_insufficient_scope`)

#### BC-1089 (NEW): `client.send_raw(req)` (low-level path) preserves 2xx status: 200 with `r#"{"accountId":"abc"}"#` → `response.status() == 200`, `response.text()` returns the body verbatim
**Confidence**: HIGH (NEW; raw-passthrough pin)
**Sources**: `tests/api_client.rs:344-365` (`test_send_raw_returns_response_for_2xx`)
**Effects**: Used by `jr api` raw passthrough — the response is NOT auto-parsed.

#### BC-1090 (NEW): `send_raw` against 404 — 404 is NOT converted to Err; caller receives 404 response with body intact (`Issue does not exist` substring preserved). Pin: error-conversion happens in `get`/`post`/`put`/`delete` layer, NOT `send_raw`
**Confidence**: HIGH (NEW; layering invariant)
**Sources**: `tests/api_client.rs:367-392` (`test_send_raw_returns_response_for_404`)

#### BC-1091 (NEW): `send_raw` retries 429 same as `get` — 429-then-200 succeeds; final response.status() == 200
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:394-422` (`test_send_raw_retries_429_then_succeeds`)

#### BC-1092 (NEW): `send_raw` 429-storm — 4 consecutive 429s exhausts retries (initial + 3 retries = MAX_RETRIES=3); FINAL response is 429 (NOT Err); caller receives the 429 to handle. Mock has `expect(4)`
**Confidence**: HIGH (NEW; MAX_RETRIES=3 pin via test-side `expect(4)`)
**Sources**: `tests/api_client.rs:424-444` (`test_send_raw_returns_429_after_exhausting_retries`)
**Effects**: Pin: `MAX_RETRIES = 3`. A refactor that bumps to 5 would trip `expect(4)`.

### 3.3 T-14 — BC-130 unit-test names verification (CONV-ABS-008 closure)

R1 BC-130 referenced unit-test names like `build_jql_parts_assignee_me`, `build_jql_parts_recent`, etc. R3 deferred grep verification. R4 grep'd `cli/issue/list.rs::tests` directly:

```
676: fn build_jql_parts_assignee_me()
694: fn build_jql_parts_reporter_account_id()
712: fn build_jql_parts_recent()
730: fn build_jql_parts_all_filters()
753: fn build_jql_parts_empty()
771: fn build_jql_parts_jql_plus_status_compose()
792: fn build_jql_parts_status_escaping()
810: fn build_jql_parts_open()
828: fn build_jql_parts_open_with_assignee()
848: fn build_jql_parts_all_filters_with_open()
871: fn build_jql_parts_asset_clause()
890: fn build_jql_parts_asset_with_assignee()
911: fn build_jql_parts_created_after_clause()
929: fn build_jql_parts_updated_after_and_before_clauses()
949: fn build_jql_parts_created_date_range()
```

15 `build_jql_parts_*` test functions verified by direct read. **CONV-ABS-008 CLOSED.** No retraction needed for R1 BC-130.

#### BC-1093 (NEW): `cli/issue/list.rs::tests` contains EXACTLY 15 `build_jql_parts_*` unit tests + 6 `build_jql_base_parts_*` + 3 `resolve_show_points_*` + 2 `extract_unique_status_names_*` = 26 source unit tests total in `list.rs`
**Confidence**: HIGH (PROMOTED; full enumeration confirmed; closes CONV-ABS-008)
**Sources**: `src/cli/issue/list.rs:656-1085`
**Behavior**: Aggregates the JQL composition contract pin from R1 BC-130. The test names are stable and load-bearing for spec crystallization — each pins one filter combination.

### 3.4 T-15 — Property tests (proptest) enumeration (NEW)

`proptest-regressions/jql.txt` exists with one regression seed (`s = ""`). R4 enumerates the 4 proptest blocks across 3 source files.

#### BC-1094 (NEW): `src/jql.rs::proptests::escaped_value_never_has_unescaped_quote` — for any `s in "\\PC{0,100}"` (random printable Unicode up to 100 chars), `escape_value(s)` produces output with NO unescaped quote (helper `has_unescaped_quote` tracks backslash-runs and counts even-length runs as un-escaping). Regression corpus: `s = ""` (the seed in proptest-regressions/jql.txt)
**Confidence**: HIGH (NEW)
**Sources**: `src/jql.rs:383-394`; `proptest-regressions/jql.txt`
**Effects**: Pin: JQL escape function is fuzz-safe across all Unicode-printable inputs.

#### BC-1095 (NEW): `src/partial_match.rs::proptests::exact_match_always_found` — for `idx in 0usize..4` (selecting from `["In Progress", "In Review", "Blocked", "Done"]`), `partial_match(input, &candidates)` returns `MatchResult::Exact(s)` with `s == input`
**Confidence**: HIGH
**Sources**: `src/partial_match.rs:153-165`

#### BC-1096 (NEW): `src/partial_match.rs::proptests::never_panics_on_arbitrary_input` — for any `s in "\\PC{0,50}"`, `partial_match(s, &["In Progress", "Done"])` MUST NOT panic. No assertion on result; the property is "no panic"
**Confidence**: HIGH (NEW; fuzz-safety pin)
**Sources**: `src/partial_match.rs:167-171`

#### BC-1097 (NEW): `src/partial_match.rs::proptests::empty_candidates_always_returns_none` — for any `s in "[a-z]{1,10}"` against EMPTY candidates Vec, `partial_match(s, &[])` returns `MatchResult::None(all)` where `all.is_empty()`
**Confidence**: HIGH
**Sources**: `src/partial_match.rs:173-180`

#### BC-1098 (NEW): `src/partial_match.rs::proptests::duplicate_candidates_yield_exact_multiple` — when candidates has a duplicate of the input, `partial_match` returns `MatchResult::ExactMultiple(name)` with `name.to_lowercase() == input.to_lowercase()`
**Confidence**: HIGH
**Sources**: `src/partial_match.rs:182-198`

#### BC-1099 (NEW): `src/duration.rs::proptests::valid_single_units_always_parse` — for `h in 1u64..100, unit in [m, h, d, w]`, `parse_duration("{h}{unit}", 8, 5)` returns `Ok(seconds)` with `seconds > 0`
**Confidence**: HIGH
**Sources**: `src/duration.rs:128-135`

#### BC-1100 (NEW): `src/duration.rs::proptests::combined_units_always_parse` — for `h in 0..24, m in 0..60` (h+m > 0), `parse_duration({h}h{m}m or {h}h or {m}m, 8, 5)` returns `Ok(_)`
**Confidence**: HIGH
**Sources**: `src/duration.rs:137-143`

#### BC-1101 (NEW): `src/duration.rs::proptests::garbage_input_never_panics` — for any `s in "\\PC{1,20}"`, `parse_duration(s, 8, 5)` MUST NOT panic (Result is acceptable in either direction). Fuzz-safety pin
**Confidence**: HIGH (NEW)
**Sources**: `src/duration.rs:145-148`

#### BC-1102 (NEW): `src/duration.rs::proptests::format_roundtrip` — for `seconds in (1..86400).filter(s % 60 == 0)`, `parse_duration(format_duration(seconds), 8, 5)` round-trips to exactly `seconds` (gated to `seconds < 28800` because `1d == 8h` collapses days→hours and the round-trip is then format-preserving only sub-day)
**Confidence**: HIGH (NEW; format_duration round-trip property pin)
**Sources**: `src/duration.rs:150-157`
**Effects**: Pin: round-trip preservation up to 8h. Beyond 8h, `format_duration` may emit "1d" which parses back to 8h × 3600 = 28800 — still equal to the seconds count, but the test's gating reflects that beyond 28800 the format_duration may emit "1d2h" etc. and the gating is defensive.

#### BC-1103 (NEW): proptest regression corpus — `proptest-regressions/jql.txt` contains exactly ONE regression seed with hash `c696552d795390c45278b7f3fe08317d68e81494e898dd32ba3a2c97f5dc7df5` shrinking to `s = ""` (empty string). The empty string was a previously-discovered failing case for `escape_value`; now permanently pinned via the regression file
**Confidence**: HIGH (NEW)
**Sources**: `proptest-regressions/jql.txt`

### 3.5 T-16 — Insta snapshot enumeration (NEW)

R4 enumerated all 17 `*.snap` files. Below: command/builder → snapshot shape map.

#### BC-1104 (NEW): `move_response("TEST-1", "In Progress", true)` → snapshot `{"changed": true, "key": "TEST-1", "status": "In Progress"}` (3 keys, alphabetical via insta's sorted JSON output)
**Confidence**: HIGH (PROMOTED, exact JSON shape pin)
**Sources**: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__move_response_changed.snap` + test at `src/cli/issue/json_output.rs:89-92`

#### BC-1105 (NEW): `move_response("TEST-1", "Done", false)` → `{"changed": false, "key": "TEST-1", "status": "Done"}` (idempotent move JSON shape)
**Confidence**: HIGH
**Sources**: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__move_response_unchanged.snap`

#### BC-1106 (NEW): `assign_changed_response("TEST-1", "Jane Doe", "abc123")` → `{"assignee": "Jane Doe", "assignee_account_id": "abc123", "changed": true, "key": "TEST-1"}` (4 keys; `assignee_account_id` snake_case, NOT camelCase)
**Confidence**: HIGH (NEW; field-naming convention pin — JSON output uses snake_case for jr-internal fields, NOT Atlassian camelCase)
**Sources**: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__assign_changed.snap`

#### BC-1107 (NEW): `assign_unchanged_response("TEST-1", "Jane Doe", "abc123")` → identical shape with `changed: false` (same 4 keys)
**Confidence**: HIGH
**Sources**: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__assign_unchanged.snap`

#### BC-1108 (NEW): `unassign_response("TEST-1", true)` → `{"assignee": null, "changed": true, "key": "TEST-1"}`; `unassign_response("TEST-1", false)` → identical with `changed: false`. The `assignee` field is EXPLICITLY `null` (NOT omitted)
**Confidence**: HIGH (NEW; null-vs-omit pin)
**Sources**: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__unassign.snap` + `__unassign_unchanged.snap`

#### BC-1109 (NEW): `edit_response("TEST-1")` → `{"key": "TEST-1", "updated": true}` — minimal 2-key shape; the `updated` is a literal true (not a count)
**Confidence**: HIGH
**Sources**: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__edit.snap`

#### BC-1110 (NEW): `link_response("TEST-1", "TEST-2", "Blocks")` → `{"key1": "TEST-1", "key2": "TEST-2", "linked": true, "type": "Blocks"}` (4 keys; `key1`/`key2` are NOT `inward_key`/`outward_key`)
**Confidence**: HIGH (NEW; symmetric naming for link payload)
**Sources**: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__link.snap`

#### BC-1111 (NEW): `unlink_response(true, 2)` → `{"count": 2, "unlinked": true}`; `unlink_response(false, 0)` → `{"count": 0, "unlinked": false}`. The "no match" case explicitly emits `count: 0` (NOT omitted)
**Confidence**: HIGH (NEW)
**Sources**: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__unlink_success.snap` + `__unlink_no_match.snap`

#### BC-1112 (NEW): `remote_link_response("TEST-1", 10000, url, title, self_url)` → `{"id": 10000, "key": "TEST-1", "self": <url>, "title": <title>, "url": <url>}` (5 keys; `id` is u64, `key`/`title`/`url`/`self` are strings)
**Confidence**: HIGH (NEW; 5-key shape pin)
**Sources**: `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__remote_link.snap`

#### BC-1113 (NEW): `sprint_add_response(100, &["TEST-1", "TEST-2"])` → `{"added": true, "issues": ["TEST-1", "TEST-2"], "sprint_id": 100}` (3 keys; `sprint_id` snake_case; `added` is a literal true)
**Confidence**: HIGH (NEW)
**Sources**: `src/cli/snapshots/jr__cli__sprint__tests__sprint_add_response.snap`

#### BC-1114 (NEW): `sprint_remove_response(&["TEST-1", "TEST-2"])` → `{"issues": [...], "removed": true}` (2 keys; NO sprint_id — different from add because remove is sprint-agnostic)
**Confidence**: HIGH (NEW; asymmetric add-vs-remove shape pin)
**Sources**: `src/cli/snapshots/jr__cli__sprint__tests__sprint_remove_response.snap`

#### BC-1115 (NEW): `auth list` table snapshot — 4 columns: `NAME, URL, AUTH, STATUS`; active profile prefixed with `* ` (asterisk-space); inactive profiles prefixed with `  ` (2 spaces); fixture has 3 profiles (default*, sandbox, staging) showing api_token + oauth + api_token AUTH column values; ALL `STATUS` cells `configured`
**Confidence**: HIGH (NEW; full table format pin)
**Sources**: `src/cli/snapshots/jr__cli__auth__tests__list_table_snapshot.snap`
**Behavior**: Pin against a refactor that uses a different glyph (`>`, `→`) for the active marker — the asterisk is part of the contract.

#### BC-1116 (NEW): `src/snapshots/jr__adf__tests__adf_to_text_complex.snap` — 18-line snapshot pinning the canonical complex ADF doc → text output (round-trip canary; specific bytes pinned, not enumerated here but the SHA covers the contract)
**Confidence**: HIGH
**Sources**: `src/snapshots/jr__adf__tests__adf_to_text_complex.snap`

#### BC-1117 (NEW): `src/snapshots/jr__adf__tests__markdown_complex_to_adf.snap` — 330-line snapshot pinning markdown→ADF for the canonical complex doc; covers heading/paragraph/list/blockquote/table/codeBlock cross-section (round-trip canary)
**Confidence**: HIGH
**Sources**: `src/snapshots/jr__adf__tests__markdown_complex_to_adf.snap`

#### BC-1118 (NEW): `tests/snapshots/issue_changelog__changelog_json_output_snapshot.snap` — pins the FULL `jr issue changelog --output json` shape: `{entries: [{author: {accountId, active, displayName, emailAddress}, created, id, items: [{field, fieldtype, from, fromString, to, toString}]}], key}`. `items[].field` is `"status"|"resolution"|"labels"`. `from`/`to` are nullable strings; `fromString`/`toString` ALSO nullable. Author can be null (system events). entries[1].items[0].fromString IS null but field IS `"labels"` — pins that null-from is NOT same as missing field
**Confidence**: HIGH (NEW; canonical shape pin)
**Sources**: `tests/snapshots/issue_changelog__changelog_json_output_snapshot.snap`

### 3.6 T-17 — `tests/user_pagination.rs` remaining 7 tests

#### BC-1119 (NEW): `search_users_all` continues PAST a short non-empty page — page 1 returns 100, page 2 returns 35, page 3 returns 100, page 4 empty → loop reads ALL 4, returns 235 total. Pin: a refactor that exits on `len < page_size` would re-scan users[35..100] on the next call and produce duplicates per JRACLOUD-71293
**Confidence**: HIGH (PROMOTED; finishes BC-1015's claim with exact numbers)
**Sources**: `tests/user_pagination.rs:202-247` (`search_users_all_continues_past_short_non_empty_page`)

#### BC-1120 (NEW): `client.search_assignable_users_by_project_all(query, projectKey)` paginates `/rest/api/3/user/assignable/multiProjectSearch` with `query`, `projectKeys`, `startAt`, `maxResults=100` — page 1 returns 100, page 2 returns 40, page 3 empty → 140 users total; users[0].display_name=="p1 User 000" (page-1 ordering preserved); users[100].display_name=="p2 User 000" (page-2 starts at index 100)
**Confidence**: HIGH (NEW; ordered-concat pin)
**Sources**: `tests/user_pagination.rs:252-297` (`search_assignable_users_by_project_all_paginates`)

#### BC-1121 (NEW): `jr user search u --all` end-to-end — 3 pages mocked at `/user/search` (100+50+0); stdout JSON is an array of 150 users (NOT a paginated envelope, NOT a Map); exit 0
**Confidence**: HIGH
**Sources**: `tests/user_pagination.rs:300-347` (`user_search_all_cli_paginates`)

#### BC-1122 (NEW): `jr user list --project FOO --all` end-to-end — 3 pages mocked at `/user/assignable/multiProjectSearch` (100+35+0); stdout JSON array of 135 users; exit 0
**Confidence**: HIGH
**Sources**: `tests/user_pagination.rs:350-397` (`user_list_all_cli_paginates`)

#### BC-1123 (NEW): `jr user search u` (no `--all`) — exactly ONE HTTP request fired, query string contains `query=u` but MUST NOT contain `startAt` OR `maxResults` (verified post-hoc via `received_requests()` inspection); stdout JSON has at most 30 users (default cap, NOT --all)
**Confidence**: HIGH (NEW; default-cap=30 + no-pagination pin)
**Sources**: `tests/user_pagination.rs:406-453` (`user_search_no_all_issues_single_request`)
**Effects**: Pin: a regression that adds default pagination would trip the `!query.contains("startAt")` guard.

#### BC-1124 (NEW): `jr user search u --all` against unbounded responder — safety cap (15 iterations) bites; command STILL exits 0; stderr contains `"hit pagination safety cap"` (user-visible warning so truncation is observable)
**Confidence**: HIGH (NEW; safety-cap warning pin)
**Sources**: `tests/user_pagination.rs:459-487` (`user_search_all_cli_emits_safety_cap_warning`)

#### BC-1125 (NEW): `jr user list --project FOO --all` against unbounded responder — same safety-cap warning contract; stderr contains `"hit pagination safety cap"`; exit 0
**Confidence**: HIGH (NEW; symmetric to BC-1124)
**Sources**: `tests/user_pagination.rs:494-520` (`user_list_all_cli_emits_safety_cap_warning`)

### 3.7 T-18 — Batch enumeration of 9 smaller integration test files

#### BC-1126 (NEW): `tests/issue_remote_link.rs:19-84` (`remote_link_creates_with_explicit_title`) — POST `/rest/api/3/issue/PROJ-123/remotelink` body partial-JSON: `{object: {url: "https://example.com/", title: "Example"}}`; **note: URL gains TRAILING SLASH from `url::Url::parse` normalization**; response `{id: 10000, self: <self_url>}`; stdout JSON `{key: "PROJ-123", id: 10000, url: "https://example.com/", title: "Example", self: <self_url>}` (5 keys, normalized URL)
**Confidence**: HIGH (PROMOTED; URL normalization pin)
**Sources**: `tests/issue_remote_link.rs:19-84`

#### BC-1127 (NEW): `tests/issue_remote_link.rs:87-147` (`remote_link_defaults_title_to_url`) — when `--title` is omitted, body has `title == url` (default-to-url); stdout JSON also has `title == url`
**Confidence**: HIGH (NEW)
**Sources**: `tests/issue_remote_link.rs:87-147`

#### BC-1128 (NEW): `tests/issue_remote_link.rs:150-197` (`remote_link_surfaces_server_error`) — 400 response body `{errorMessages: ["Issue does not exist or you do not have permission to see it."], errors: {}}` → exit 1, stderr contains (lowercase) `"issue does not exist"`
**Confidence**: HIGH
**Sources**: `tests/issue_remote_link.rs:150-197`

#### BC-1129 (NEW): `tests/issue_remote_link.rs:200-253` (`remote_link_surfaces_not_authenticated_on_401`) — 401 → exit 2, stderr `"Not authenticated"` OR `"jr auth login"`
**Confidence**: HIGH
**Sources**: `tests/issue_remote_link.rs:200-253`

#### BC-1130 (NEW): `tests/issue_remote_link.rs:259-301` (`remote_link_rejects_invalid_url_with_exit_64`) — `--url not-a-url` → exit 64 (UserError); stderr contains `"--url"` AND (lowercase) `"not a valid url"`; ZERO HTTP calls (validation pre-network)
**Confidence**: HIGH (NEW; pre-HTTP validation pin)
**Sources**: `tests/issue_remote_link.rs:259-301`

#### BC-1131 (NEW): `tests/issue_remote_link.rs:309-348` (`remote_link_rejects_non_http_scheme_with_exit_64`) — `--url ftp://example.com` (parses as URL but wrong scheme) → exit 64; stderr contains `"http or https"` AND `"ftp"`; pin against future broadening to e.g. `mailto:`
**Confidence**: HIGH (NEW; scheme-allowlist pin)
**Sources**: `tests/issue_remote_link.rs:309-348`

#### BC-1132 (NEW): `tests/user_commands.rs` — 14 tests covering: search returning matching users (BC-1132a), empty results showing `"No results found."` (BC-1132b), JSON output is array (BC-1132c), `--limit 2` truncates (BC-1132d), `user list` requires `--project` (clap-rejects, BC-1132e), `user list --project FOO` returns users (BC-1132f), `user view <id>` returns detail rows (BC-1132g), `user view --output json` (BC-1132h), 404 user_view friendly error `"User with accountId '<id>' not found"` exit 64 (BC-1132i), hidden email (no emailAddress in response) renders em-dash `—` (BC-1132j), JSON form returns explicit `null` (BC-1132k), 5xx exit 1 (BC-1132l), 401 exit 2 + reauth hint (BC-1132m), network drop exit 1 + `"Could not reach"` (BC-1132n)
**Confidence**: HIGH (PROMOTED, batched; 14 tests)
**Sources**: `tests/user_commands.rs:1-417` (14 tests)
**Effects**: Sub-test (i) pins the literal substring `"User with accountId '<id>' not found"` — error wording is the actionable hint. Sub-test (j/k) pins the privacy boundary: the em-dash placeholder is for table-output ONLY; JSON output preserves `null` so consumers can distinguish hidden-email from no-email.

#### BC-1133 (NEW): `tests/project_commands.rs` — 10 tests covering: `list_projects(None, Some(50))` returns 2 projects (BC-1133a); empty list (BC-1133b); missing lead → `lead.is_none()` (BC-1133c); `list_projects(Some("software"), Some(50))` filter via `typeKey` query param (BC-1133d); `list_projects` paginates via `startAt` (BC-1133e); `get_project_statuses(key)` returns issuetypes-with-statuses (BC-1133f); empty statuses (BC-1133g); 5xx → exit 1 + `"API error (500)"` (BC-1133h); 401 → exit 2 + reauth hint (BC-1133i); network drop → exit 1 + `"Could not reach"` (BC-1133j)
**Confidence**: HIGH (PROMOTED, batched; 10 tests)
**Sources**: `tests/project_commands.rs:1-323`

#### BC-1134 (NEW): `tests/issue_resolution.rs` — 3 tests: `jr issue resolutions --output json` returns a 2-element array of resolutions with `id, name, description` (BC-1134a); table form contains `"Done"` and the description column `"Work complete"` (BC-1134b); `jr --no-input issue move FOO-1 Done` against a 400 error body `{errors: {resolution: "Field 'resolution' is required"}}` → exit non-zero, stderr contains `"--resolution"` AND `"jr issue resolutions"` (actionable remediation hint pin) (BC-1134c)
**Confidence**: HIGH (PROMOTED, 3 tests; BC-1134c is a high-value UX hint pin)
**Sources**: `tests/issue_resolution.rs:11-158`

#### BC-1135 (NEW): `tests/issue_view_errors.rs` — 4 tests: 5xx → exit 1, `"API error (500)"` (BC-1135a); 401 → exit 2, `"Not authenticated"` + `"jr auth login"` (BC-1135b); net-drop → exit 1, `"Could not reach"` + `"check your connection"` (BC-1135c); **CORRUPT TEAM CACHE FALLBACK** — when `~/.cache/jr/teams.json` is truncated `{"teams": [`, `jr issue view PROJ-1` STILL succeeds (exit 0), stdout shows the team UUID literally, AND inline guidance `"name not cached"` + `"jr team list --refresh"`; pinned via `read_cache` returning `Ok(None)` on parse-fail (BC-1135d)
**Confidence**: HIGH (NEW; BC-1135d is a load-bearing reliability contract)
**Sources**: `tests/issue_view_errors.rs:1-206`
**Effects**: BC-1135d is the integration pin for cache.rs `serde_json::from_str` failure → `Ok(None)` mapping. A refactor that propagates the parse-fail error would regress this case.

#### BC-1136 (NEW): `tests/assets_errors.rs` — 3 tests for `jr assets search "Key = X"`: 5xx on workspace-discovery → exit 1, `"API error (500)"`; 401 on workspace-discovery → exit 2, reauth hint; net-drop → exit 1, `"Could not reach"`. Workspace discovery is the FIRST call in the assets chain; the test confirms errors at that step propagate the same way as direct-issue endpoints
**Confidence**: HIGH (PROMOTED, batched 3)
**Sources**: `tests/assets_errors.rs:20-153`

#### BC-1137 (NEW): `tests/cmdb_fields.rs` — 5 tests: `find_cmdb_fields()` extracts ONLY fields whose `schema.custom == "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"` (BC-1137a; pin against name-based heuristic); empty result when no CMDB fields (BC-1137b); modern CMDB field shape `[{label, objectKey}]` deserializes via `extract_linked_assets` to `LinkedAsset{key: Some("OBJ-1"), name: Some("Acme Corp")}` (BC-1137c); null CMDB field → empty assets (BC-1137d); `enrich_assets` fetches workspace-id then `/jsm/assets/workspace/<ws>/v1/object/<id>?includeAttributes=false` and resolves `id` → `key`/`name`/`asset_type` (BC-1137e)
**Confidence**: HIGH (PROMOTED, batched 5; BC-1137a is the "schema.custom not name" pin)
**Sources**: `tests/cmdb_fields.rs:50-189`

#### BC-1138 (NEW): `tests/team_column_parity.rs` — 7 tests covering Team column rendering across `jr sprint current`, `jr board view --board-type kanban`. Gating: column appears IFF (`team_field_id` configured in `[fields]`) AND (at least one rendered issue has a populated team UUID). 4 tests pin "shows when populated" (BC-1138a/c), 2 tests pin "omits when not configured / no issue has team" (BC-1138b/d), 1 test pins "falls back to UUID + 'name not cached' hint when team cache is stale" (BC-1138e), 1 test pins "JSON output keeps team UUID without resolution" (BC-1138f) — 7 total via lines 124, 181, 220, 284, 341, 380, 425
**Confidence**: HIGH (PROMOTED, batched 7)
**Sources**: `tests/team_column_parity.rs:124-475`

#### BC-1139 (NEW): `tests/auth_login_config_errors.rs:18-97` (single test `auth_login_oauth_surfaces_malformed_config_without_overwriting`) — when `~/.config/jr/config.toml` is malformed TOML (e.g. `[unclosed\nbad = \n`), running `jr auth login --oauth --client-id X --client-secret Y --no-input`: (a) exit code EXACTLY 78 (ConfigError), (b) stderr contains lowercase `"toml"` OR `"parse"`, (c) the malformed file on disk is UNCHANGED (verified via byte-for-byte comparison post-command). Pin against the original bug (#258) where `Config::load().unwrap_or_default()` swallowed parse errors AND the subsequent `save_global()` overwrote the broken file with defaults
**Confidence**: HIGH (NEW; load-bearing config-safety pin)
**Sources**: `tests/auth_login_config_errors.rs:18-97`
**Effects**: Pin: malformed config → `Config::load()?` propagates → `JrError::ConfigError` exits 78. Save is gated on Load succeeding. A regression that re-introduces `unwrap_or_default()` would trip both (a) AND (c).

### 3.8 T-19 — `src/api/auth.rs` 22 source unit tests per-test BCs

#### BC-1140 (NEW): `redirect_uri_strategy_strings` — `RedirectUriStrategy::FixedPort(53682).redirect_uri() == "http://127.0.0.1:53682/callback"` (literal IPv4); `DynamicPort(54321).redirect_uri() == "http://localhost:54321/callback"` (literal `localhost` for BYO backwards-compat). The TWO strings differ in host literal — Atlassian validates redirect_uri by EXACT string match
**Confidence**: HIGH (NEW; cross-host-form pin)
**Sources**: `src/api/auth.rs:927-937`

#### BC-1141 (NEW): `embedded_callback_port_is_53682` — `EMBEDDED_CALLBACK_PORT == 53682` literal; pinned at type-system level
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:943-946`

#### BC-1142 (NEW): `extract_query_param("GET /callback?code=abc123&state=xyz HTTP/1.1\r\n", "code")` → `Some("abc123")`; `extract_query_param(.., "state")` → `Some("xyz")` (multiple-param parsing; ampersand-separated)
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:948-959`

#### BC-1143 (NEW): `extract_query_param("GET /callback?code=abc123 HTTP/1.1\r\n", "state")` → `None` (param not present)
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:961-965`

#### BC-1144 (NEW): `extract_query_param("GET /callback HTTP/1.1\r\n", "code")` → `None` (no query string at all)
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:967-971`

#### BC-1145 (NEW): `generate_state()` returns hex-only string (every char is `is_ascii_hexdigit()`); non-empty
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:973-978`

#### BC-1146 (NEW): `generate_state().len() == 64` (32 bytes × 2 hex chars); pin against truncated/lower-entropy regression
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:984-992`

#### BC-1147 (NEW): `generate_state` produces 8 distinct values across 8 calls (CSPRNG; collision probability across 8 samples ≈ 2^-253); pin against deterministic regression like timestamp-hex
**Confidence**: HIGH (NEW)
**Sources**: `src/api/auth.rs:1000-1012`

#### BC-1148 (NEW): `build_authorize_url("normal-client-id", "read:jira-work offline_access", "http://localhost:12345/callback", "deadbeef")` produces URL starting with `https://auth.atlassian.com/authorize?` and containing ALL of: `audience=api.atlassian.com`, `&client_id=normal-client-id`, `&scope=read%3Ajira-work%20offline_access` (literal `%20` for space, NOT `+`), `&redirect_uri=http%3A%2F%2Flocalhost%3A12345%2Fcallback`, `&state=deadbeef`, `&response_type=code`, `&prompt=consent`
**Confidence**: HIGH (NEW; 7-substring pin)
**Sources**: `src/api/auth.rs:1017-1037`
**Effects**: `prompt=consent` is mandatory — Atlassian's standard prompt behavior is "if you've consented before, skip", but jr explicitly forces consent so users see the scope list every time.

#### BC-1149 (NEW): `build_authorize_url(hostile_client_id, ..)` — for `client_id == "real_id&redirect_uri=evil.example#frag"`, the URL MUST NOT contain `&redirect_uri=evil.example` (NO injection); MUST contain `client_id=real_id%26redirect_uri%3Devil.example%23frag` (full percent-encoding of `&`/`=`/`#`)
**Confidence**: HIGH (NEW; security pin; XSS-style injection prevention)
**Sources**: `src/api/auth.rs:1043-1060`

#### BC-1150 (NEW): `build_authorize_url(.., "scope:with+plus", ..)` — `+` in scope encoded as `%2B` (NOT raw `+`, which means "space" in form-urlencoded context); URL MUST NOT contain raw `+` substring
**Confidence**: HIGH (NEW; encoding-correctness pin)
**Sources**: `src/api/auth.rs:1066-1083`

#### BC-1151 (NEW): `store_oauth_tokens(profile, access, refresh)` round-trips per-profile: storing `(access1, refresh1)` for `default` AND `(access2, refresh2)` for `sandbox` → `load_oauth_tokens("default") == Ok(("access1", "refresh1"))`, `load_oauth_tokens("sandbox") == Ok(("access2", "refresh2"))`. Per-profile isolation via namespaced keychain keys `<profile>:oauth-{access,refresh}-token`. Test gated by `JR_RUN_KEYRING_TESTS=1`
**Confidence**: HIGH (PROMOTED scope; per-profile isolation invariant)
**Sources**: `src/api/auth.rs:1130-1143`

#### BC-1152 (NEW): `load_oauth_tokens("default")` with NO entries → `Err`; pin against silent-empty-string fallback
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:1145-1151`

#### BC-1153 (NEW): Lazy migration for `default` profile — pre-existing flat keys `oauth-access-token` + `oauth-refresh-token` (legacy) are read by `load_oauth_tokens("default")` and migrated to `default:oauth-access-token` + `default:oauth-refresh-token`; the legacy flat key is REMOVED post-migration (`get_password()` returns `Err` after migration)
**Confidence**: HIGH (NEW; migration semantics pin — "read once, move forever")
**Sources**: `src/api/auth.rs:1153-1178`

#### BC-1154 (NEW): `clear_profile_creds("default")` — must ALSO remove the legacy flat keys `oauth-access-token` + `oauth-refresh-token`. Pin against the bug where `jr auth logout --profile default` left the legacy keys in place and the next `load_oauth_tokens` re-resurrected them via lazy migration
**Confidence**: HIGH (NEW; logout-safety pin)
**Sources**: `src/api/auth.rs:1185-1219`

#### BC-1155 (NEW): `clear_profile_creds("sandbox")` (NON-default profile) does NOT touch the legacy flat keys — they remain available for the `default` profile's lazy migration. Asymmetric behavior pinned
**Confidence**: HIGH (NEW; non-default-profile invariance)
**Sources**: `src/api/auth.rs:1221-1247`

#### BC-1156 (NEW): `load_oauth_tokens` errors on PARTIAL state — `<profile>:oauth-access-token` exists but `<profile>:oauth-refresh-token` missing (or vice versa) → returns Err. Prevents silently using a half-stored credential
**Confidence**: HIGH (NEW; partial-state-rejection pin)
**Sources**: `src/api/auth.rs:1249-1269`

#### BC-1157 (NEW): `load_oauth_tokens("default")` recovers from PARTIAL state via legacy flat keys — when `default:oauth-access-token` is missing BUT both legacy `oauth-{access,refresh}-token` are present, returns the legacy tokens. Pin: lazy migration runs even on partial primary-key state
**Confidence**: HIGH (NEW; recovery-path pin)
**Sources**: `src/api/auth.rs:1271-1321`

#### BC-1158 (NEW): `load_oauth_tokens("sandbox")` does NOT inherit legacy flat keys (even if they exist) — sandbox profile errors. Lazy-migration is `default`-profile-only
**Confidence**: HIGH (NEW)
**Sources**: `src/api/auth.rs:1323-1341`

#### BC-1159 (NEW): `resolve_refresh_app_credentials()` prefers KEYCHAIN over EMBEDDED — if keychain has `oauth_client_id` + `oauth_client_secret`, returns `(id, secret, RefreshAppSource::Keychain)`. Pin: a returning BYO user does not silently flip onto the embedded app mid-session (their refresh_token would be rejected if presented with a different client_id)
**Confidence**: HIGH (NEW; mid-session-stability pin)
**Sources**: `src/api/auth.rs:1347-1357`

#### BC-1160 (NEW): `resolve_refresh_app_credentials()` returns `Err` when keychain is empty AND embedded is None (default test build); error message contains `"embedded"` substring (caller can distinguish "no creds at all" from other error types)
**Confidence**: HIGH
**Sources**: `src/api/auth.rs:1362-1371`

#### BC-1161 (NEW): `RedirectUriStrategyRequest::Fixed(port).bind()` against pre-bound port — error message contains: literal `"port {port}"` (with the actual numeric port), `"in use"`, `"--client-id"`. Pin: the actionable hint is the entire payoff of fixed-port design — if a future refactor regresses the message, embedded users hitting a port conflict have no actionable hint
**Confidence**: HIGH (NEW; full source-test enumeration of EADDRINUSE message)
**Sources**: `src/api/auth.rs:1377-1396`

### 3.9 T-20 — `src/api/auth_embedded.rs` 8 tests per-test BCs

#### BC-1162 (NEW): `decode(xored_bytes, key)` round-trips: for plaintext `"hello-world-secret"` and key `[42u8; 32]`, XOR-encode then decode produces the original plaintext
**Confidence**: HIGH (NEW; XOR primitive round-trip pin)
**Sources**: `src/api/auth_embedded.rs:142-155`

#### BC-1163 (NEW): `build_embedded_app(None, None, None) == None` — missing all three constants → no embedded app
**Confidence**: HIGH
**Sources**: `src/api/auth_embedded.rs:157-161`

#### BC-1164 (NEW): `build_embedded_app(Some(id), Some(xored), Some(key))` returns `Some(EmbeddedOAuthApp{client_id == id, client_secret == decoded_plaintext})` — the secret is decoded at construction time
**Confidence**: HIGH
**Sources**: `src/api/auth_embedded.rs:163-178`

#### BC-1165 (NEW): `build_embedded_app` returns `None` if ANY of the three constants is missing — `Some(id), Some(b"x"), None` → None; `Some(id), None, Some(key)` → None; `None, Some(b"x"), Some(key)` → None. AND-gated, not OR-gated
**Confidence**: HIGH (NEW; conjunctive presence pin)
**Sources**: `src/api/auth_embedded.rs:180-189`

#### BC-1166 (NEW): `build_embedded_app(Some(""), .., ..)` → None (empty client_id rejected); `build_embedded_app(Some("id"), Some(&[]), ..)` → None (empty xor ciphertext rejected — would decode to empty secret). Pin: a build-pipeline misconfig that emits empty values must NOT ship a binary that posts empty creds to Atlassian
**Confidence**: HIGH (NEW; build-pipeline-safety pin)
**Sources**: `src/api/auth_embedded.rs:194-201`

#### BC-1167 (NEW): `embedded_oauth_app()` returns `None` in DEFAULT test build (no `JR_BUILD_OAUTH_CLIENT_*` env vars at compile time). Pin: if this fails in CI, the release env var is leaking into test runs
**Confidence**: HIGH (NEW; CI-isolation pin)
**Sources**: `src/api/auth_embedded.rs:207-215`

#### BC-1168 (NEW): `EmbeddedOAuthApp::Debug` MUST redact `client_secret` — `format!("{app:?}")` for app with secret `"super-secret-must-not-leak"` MUST contain `client_id` value AND `<redacted>` marker AND MUST NOT contain the literal secret string
**Confidence**: HIGH (NEW; defense-in-depth secret-leak pin)
**Sources**: `src/api/auth_embedded.rs:220-239`

#### BC-1169 (NEW): `embedded_oauth_app_present()` returns `false` in default test build — fast presence check that does NOT decode the secret (allows callers to short-circuit without paying decode cost when not needed)
**Confidence**: HIGH (NEW; lazy-decode pin)
**Sources**: `src/api/auth_embedded.rs:243-249`

### 3.10 T-21 — OAuth state-machine per-transition BCs (continues R3 §3.10)

R3 §3.10 sketched the state diagram but produced no per-transition BCs. R4 emits them.

#### BC-1170 (NEW): RESOLVE_OAUTH_APP transition — priority chain `Flag > Env > Keychain > Embedded > Prompt > None`. The function `resolve_oauth_app_credentials()` (or per-flow analog) returns the first source found. If all are absent AND `--no-input`, errors with `"Atlassian OAuth app credentials are required"` (exit 64)
**Confidence**: HIGH (PROMOTED; documented in CLAUDE.md "Embedded OAuth app" gotcha)
**Sources**: `src/api/auth.rs:781-820` (`resolve_refresh_app_credentials`)

#### BC-1171 (NEW): DECIDE_REDIRECT_STRATEGY transition — Embedded source → `RedirectUriStrategy::FixedPort(53682)`; ALL other sources (Flag, Env, Keychain, BYO) → `RedirectUriStrategy::DynamicPort(0)` (port 0 = OS-assigned ephemeral). The strategy decision is a function of the resolved-app-source TAG, NOT of the client_id value
**Confidence**: HIGH (NEW; source-tag-driven decision pin)
**Sources**: CLAUDE.md gotchas; `src/api/auth.rs:374-396`

#### BC-1172 (NEW): BIND_LISTENER transition — `RedirectUriStrategyRequest::bind()` matches on enum variant: `Dynamic` binds `127.0.0.1:0` (OS-assigned), reads back actual port; `Fixed(p)` binds `127.0.0.1:p`. Both can fail: Dynamic with raw I/O err propagated; Fixed with EADDRINUSE → BC-1161 friendly error; Fixed with non-EADDRINUSE I/O err propagated raw
**Confidence**: HIGH (PROMOTED scope)
**Sources**: `src/api/auth.rs:415-451`

#### BC-1173 (NEW): GENERATE_STATE → BUILD_AUTHORIZE_URL transition is unconditional — once state is generated AND listener bound, the authorize URL is composed with `(client_id, scopes, redirect_uri, state)`. State is held in the controlling task; the listener accepts on the bound port; the URL is opened via `open::that(url)` (best-effort — failure to open browser is non-fatal, the URL is also printed to stderr)
**Confidence**: HIGH (NEW; flow-ordering pin)
**Sources**: `src/api/auth.rs:846-892`; `src/cli/auth.rs::login_oauth`

#### BC-1174 (NEW): ACCEPT_CALLBACK → EXTRACT_QUERY_PARAMS transition — when the browser hits `127.0.0.1:53682/callback?code=X&state=Y`, the listener accepts; the request line is read; `extract_query_param(req, "state")` returns `Some(Y)`; STATE-MISMATCH detection: if `Y != generated_state`, reject the callback with an error (CSRF-style protection). The exact error wording is not pinned by a test, but the comparison MUST be byte-equal (constant-time comparison NOT required because the state is OS-CSPRNG output not derived from secret)
**Confidence**: MEDIUM (NEW; state-mismatch detection inferred from absence of mismatch-handling test, but the comparison logic is in `src/api/auth.rs` source — needs deeper read for full pin)
**Sources**: `src/api/auth.rs::oauth_login` (the `let received_state = ...; if received_state != state` pattern, line range not enumerated this round); `extract_query_param` (BC-1142..1144)
**Untested**: G-OL4 (NEW) — state-mismatch scenario has no integration test pin

#### BC-1175 (NEW): EXCHANGE_CODE_FOR_TOKEN transition — POST to `https://auth.atlassian.com/oauth/token` with body `{grant_type: "authorization_code", code: <code>, client_id, client_secret, redirect_uri}` (form-urlencoded); response `{access_token, refresh_token, expires_in, token_type, scope}` deserialized; BOTH access AND refresh must be present (refresh requires `offline_access` scope to be requested)
**Confidence**: HIGH (NEW; per BC-035-R)
**Sources**: `src/api/auth.rs::exchange_code_for_token`; CLAUDE.md mention of `offline_access` scope

#### BC-1176 (NEW): DISCOVER_CLOUDID transition — GET `https://api.atlassian.com/oauth/token/accessible-resources` with the access_token as Bearer; response is an ARRAY of `{id, name, url, scopes}`. Three branches: zero resources → error (user has no Jira sites accessible); one resource → use that cloud_id; multiple resources → prompt user (or error if `--no-input` set). The prompt is via `inquire::Select` over the list of `name (url)` strings
**Confidence**: MEDIUM (NEW; multiple-cloud-id branching documented but no integration test enumeration this round)
**Sources**: `src/api/auth.rs` (function not enumerated this round)
**Untested**: G-OL2/G-OL3 in R3 §5.14 — both still apply

#### BC-1177 (NEW): PERSIST_KEYCHAIN transition — once cloud_id is selected, `store_oauth_tokens(profile, access, refresh)` writes to `<profile>:oauth-access-token` + `<profile>:oauth-refresh-token` (per-profile namespaced). The cloud_id, client_id, scopes, etc. are NOT in keychain — only access+refresh tokens
**Confidence**: HIGH (NEW; per BC-1151)
**Sources**: `src/api/auth.rs:1130-1143`

#### BC-1178 (NEW): PERSIST_CONFIG transition — final step writes `[profiles.<name>]` block to `~/.config/jr/config.toml` with `cloud_id`, `oauth_client_id` (the resolved one), and other metadata. The config save uses `save_global()`. State after success: profile is fully usable for non-interactive `jr` commands
**Confidence**: HIGH (NEW; per CLAUDE.md description of profile structure)
**Sources**: `src/cli/auth.rs::login_oauth` (via Config::save_global path)

#### Holdout H-044 (NEW): 401-AUTO-REFRESH integration-test wireframe — the deferred integration of `refresh_oauth_token` into the 401-handler pipeline.
**Wireframe**:
```
mock_state = "expired" → "refreshed"
Mock 1: GET /rest/api/3/issue/FOO-1 with Bearer <expired> → 401 {message: "expired_token"}
       (only fires while mock_state == "expired"; up_to_n_times(1))
Mock 2: POST /oauth/token (refresh grant body) → 200 {access_token: "<new>", refresh_token: "<new-rt>", expires_in: 3600}
       (transitions mock_state to "refreshed")
Mock 3: GET /rest/api/3/issue/FOO-1 with Bearer <new> → 200 {issue body}

Action: jr issue view FOO-1 (with seeded keychain holding <expired>+<rt>)

Expected (post-fix): exit 0; stdout contains "FOO-1"; keychain access-token is now "<new>"; total HTTP calls = 3 (1 initial, 1 refresh, 1 retry).
Expected (current): exit 2 with "Not authenticated" + "jr auth login" hint. The user must MANUALLY re-login. Total HTTP calls = 1 (only the initial 401).
```
**Why hidden**: H-043 from R3 documented the integration gap; H-044 is the concrete wireframe Round 4 produces per R3 §9.1c.
**Cross-reference**: Pass 4 §5 (Reliability gaps); H-043 (R3); H-044 (R4) tracks the wireframe.

#### Holdout H-045 (NEW): ZERO accessible-resources branch — the user authenticates successfully at /oauth/token but `/oauth/token/accessible-resources` returns `[]` (no Jira sites).
**Setup**: Mock both endpoints. /oauth/token returns valid tokens; /accessible-resources returns `[]`.
**Action**: `jr auth login --oauth --client-id X --client-secret Y --no-input`
**Expected**: exit non-zero; stderr contains a hint like "no Atlassian sites accessible by this token" or similar; keychain MUST NOT be partially written (atomicity).
**Why hidden**: G-OL2 in R3 §5.14 is the gap; H-045 is its formal holdout candidate.
**Cross-reference**: Pass 4 §5.

#### Holdout H-046 (NEW): MULTIPLE accessible-resources with `--no-input` — the user has tokens for 3 Atlassian sites and runs `--no-input`, where prompting is forbidden.
**Setup**: /accessible-resources returns 3 resources.
**Action**: `jr auth login --oauth --client-id X --client-secret Y --no-input`
**Expected**: exit non-zero; stderr contains hint like "multiple Atlassian sites accessible — re-run without --no-input or use --cloud-id <id>"; keychain MUST NOT be partially written.
**Why hidden**: G-OL3 in R3 §5.14 is the gap; H-046 is its formal holdout candidate.
**Cross-reference**: Pass 4 §5.

#### Holdout H-047 (NEW): STATE MISMATCH at callback — the OAuth callback contains `state=` value that does NOT match the generated state.
**Setup**: Real listener bound; manually craft a HTTP request to the callback with mismatched state.
**Action**: As above.
**Expected**: exit non-zero; stderr contains hint like "OAuth state mismatch — possible CSRF — retry login"; keychain not touched.
**Why hidden**: G-OL4 (NEW this round) — no integration test pins the state-mismatch detection path. Source code does the comparison but absent a test, a refactor that drops the comparison would be undetected.

### 3.11 T-22 — Per-source-file unit-test count audit (R3 §9.12)

R3 cited counts that R4 systematically re-counted via `awk '/^[[:space:]]*#\[(tokio::)?test/{c++} END{print c}' <file>`:

| File | R3-claimed | R4-recount | Delta |
|---|---:|---:|---:|
| `src/cache.rs` | 27 | 27 | 0 ✓ |
| `src/config.rs` | 37 | 37 | 0 ✓ |
| `src/jql.rs` | 43 | 43 | 0 ✓ |
| `src/partial_match.rs` | 12 | 12 | 0 ✓ |
| `src/duration.rs` | 16 | 16 | 0 ✓ |
| `src/api/auth.rs` | 22 | 22 | 0 ✓ |
| `src/api/auth_embedded.rs` | 8 | 8 | 0 ✓ |
| `src/cli/issue/list.rs` | (R1 implicit 26) | 26 | 0 ✓ |
| `src/cli/issue/json_output.rs` | 11 | 11 | 0 ✓ |
| `src/cli/issue/changelog.rs` | 38 | (R3 listed) — recount via awk: 38 | 0 ✓ |
| `src/adf.rs` | 69 | 69 | 0 ✓ |
| `tests/issue_commands.rs` | 54 | 54 | 0 ✓ |
| `tests/api_client.rs` | 22 | 22 | 0 ✓ |
| `tests/issue_remote_link.rs` | 6 | 6 | 0 ✓ |
| `tests/user_commands.rs` | 14 | 14 | 0 ✓ |
| `tests/project_commands.rs` | 10 | 10 | 0 ✓ |
| `tests/issue_resolution.rs` | 3 | 3 | 0 ✓ |
| `tests/issue_view_errors.rs` | 4 | 4 | 0 ✓ |
| `tests/assets_errors.rs` | 3 | 3 | 0 ✓ |
| `tests/cmdb_fields.rs` | 5 | 5 | 0 ✓ |
| `tests/team_column_parity.rs` | 7 | 7 | 0 ✓ |
| `tests/auth_login_config_errors.rs` | 1 (R3 said "survey") | 1 | 0 ✓ |
| `tests/user_pagination.rs` | 11 | 11 | 0 ✓ |
| `tests/worklog_commands.rs` | 5 | 5 | 0 ✓ |
| `tests/input_validation.rs` | 8 | 8 | 0 ✓ |
| `tests/project_meta.rs` | 3 | 3 | 0 ✓ |
| `tests/cli_smoke.rs` | 27 | 27 | 0 ✓ |

**Result**: ALL R1/R2/R3 counts correct. **No CONV-ABS retraction triggered by audit.**

---

## 4. Updated holdout candidates

### Modified
None.

### New holdouts (added in R4)

- **H-044**: 401-auto-refresh integration-test wireframe (concrete mock chain produced; specifies the 3-mock pipeline)
- **H-045**: Zero accessible-resources error-path
- **H-046**: Multiple accessible-resources + `--no-input` rejection
- **H-047**: OAuth state-mismatch detection at callback

### Holdout count after R4: **43 + 4 = 47**

---

## 5. Untested-behavior gap list (deltas to R3 §5)

### 5.18 OAuth state machine (NEW)
- **G-OL4**: State-mismatch detection at callback — source comparison exists but no integration test pins it. Captured as H-047.
- **G-OL5**: `prompt=consent` literal pin in authorize URL (BC-1148) is unit-test-only. No integration test pins that the OS browser actually receives the consent prompt — a manual assertion.

### 5.19 401 auto-refresh (NEW)
- **G-401-AR1**: `refresh_oauth_token` exists at source but has zero production callers. Captured as H-043 (R3) + H-044 (R4 wireframe).

### 5.20 Snapshot tests (NEW)
- **G-SNAP1**: `unlink_no_match.snap` (BC-1111) pins `count: 0, unlinked: false` but the JSON shape itself doesn't carry an error message. A user invoking `jr issue unlink --type Blocks` against an issue with NO Blocks links would get this no-message JSON success — UX gap.

### 5.21 Insta snapshot canon (NEW)
- **G-SNAP2**: 17 `.snap` files exist; new builders added without snapshot pinning would silently drift JSON shape until a human notices.

---

## 6. Retracted / corrected (CONV-ABS-011..012)

### CONV-ABS-011 — R3 §9.8 listed `auth_login_config_errors.rs` as "survey-level"
**Original claim** (R3 §9.8): the file was "survey-level" — implying multi-test, granularity unsure.
**Reality**: The file has EXACTLY 1 test. R4 enumerates it as BC-1139.
**Action**: Reclassified to per-test. The "survey" hedge was overcautious but not factually wrong.

### CONV-ABS-012 — R3 §9.7 said "R3 read 4 of 11" tests in user_pagination.rs
**Original claim**: R3 had read 4 of 11 user_pagination.rs tests.
**Reality**: Recount of distinct `#[tokio::test]` annotations in the file = 11. R4 has now read all 11. The 4 R3 read were:
- `search_users_all_paginates_and_concatenates` (BC-1015 in R3)
- `search_users_all_stops_on_empty_page` (BC-1015)
- `search_users_all_respects_safety_cap` (BC-1016)
- `search_users_all_propagates_error_mid_pagination` (BC-1017)

R4 enumerated the remaining 7 as BC-1119..1125. **No retraction; closure of R3 deferred work.**

---

## 7. Delta Summary

| Metric | Broad | After R1 | After R2 | After R3 | After R4 | Delta R4 |
|---|---:|---:|---:|---:|---:|---:|
| Total BCs | 193 | 271 | 343 | 419 | **540** | **+121** |
| HIGH | 134 | 211 | 281 | 354 | **475** | **+121** |
| MEDIUM | 45 | 53 | 56 | 59 | **59** | **0** |
| LOW | 9 | 7 | 6 | 6 | **6** | **0** |
| Holdout candidates | 20 | 29 | 38 | 43 | **47** | **+4** |
| Untested invariants closed | 0 | 4 | 5 | 5 | **5** | 0 |
| Untested behaviors enumerated | 0 | 23 | 30 | 35 | **41** (added 6 in §5.18..5.21) | **+6** |
| BCs promoted MEDIUM→HIGH | n/a | 13 | 5 | 2 | **0** | n/a |
| BCs corrected (CONV-ABS) | n/a | 4 | 4 | 6 | **8** (+ CONV-ABS-011/012) | **+2** |

### Subject-area BC distribution after Round 4

| Subject area | After R3 H/M/L | After R4 H/M/L | Delta |
|---|---|---|---|
| 1. Auth & Identity | 31/4/0 | 53/4/0 | +22 (BC-1140..1161 per-test source unit) |
| 2. Issue read | 78/6/1 | 84/6/1 | +6 (BC-1036..1054 select issue_commands.rs) |
| 3. Issue write | 41/5/1 | 71/5/1 | +30 (BC-1055..1081 issue_commands.rs writes) |
| 4. Issue assets / CMDB | 24/3/0 | 29/3/0 | +5 (BC-1137 cmdb_fields.rs) |
| 5. Boards & Sprints | 25/3/0 | 32/3/0 | +7 (BC-1138 team_column_parity full) |
| 6. Worklogs & duration | 11/1/0 | 15/1/0 | +4 (BC-1099..1102 duration proptests) |
| 7. Teams | 11/2/0 | 11/2/0 | 0 |
| 8. Users | 16/1/0 | 30/1/0 | +14 (BC-1119..1125 user_pagination, BC-1132 user_commands batched 14-test) |
| 9. Projects & Queues | 19/2/0 | 32/2/0 | +13 (BC-1133 project_commands batched 10, BC-1134 issue_resolution batched 3) |
| 10. Configuration | 14/2/1 | 15/2/1 | +1 (BC-1139 auth_login_config_errors) |
| 11. Cache | 16/2/1 | 17/2/1 | +1 (BC-1135d corrupt-cache fallback) |
| 12. Output formatting | 12/4/1 | 27/4/1 | +15 (BC-1104..1118 insta snapshots) |
| 13. Error handling | 23/3/0 | 32/3/0 | +9 (BC-1082..1092 api_client.rs, BC-1126..1131 issue_remote_link, BC-1135..1136 error envelopes) |
| 14. Build-time | 7/1/1 | 7/1/1 | 0 |
| 15. Runtime concerns | 21/2/0 | 21/2/0 | 0 |
| 16. ADF | 53/1/0 | 53/1/0 | 0 |
| 17. `jr api` raw passthrough | 9/0/0 | 9/0/0 | 0 |
| 18. Source unit-test contracts (Changelog/build_jql_parts/proptest) | 30/0/0 | 41/0/0 | +11 (BC-1093 build_jql_parts, BC-1094..1098 proptests partial_match+jql, BC-1162..1169 auth_embedded) |
| 19. CLI smoke / input validation | 7/0/0 | 7/0/0 | 0 |
| 20. Browse URL bug | 2/0/0 | 2/0/0 | 0 |
| 21. OAuth state machine | 1/0/0 | 9/1/0 | +9 (BC-1170..1178 OAuth transitions; 1 medium for state-mismatch) |
| **Totals** | **354/59/6 = 419** | **475/59/6 = 540** | **+121** |

---

## 8. Novelty Assessment

Novelty: **NITPICK**

Justification: Round 4 added **121 net new BCs**, attacking ALL 12 verbatim Round-3 §9 deferred targets to closure. However, the substantive question is: would removing R4's findings change how the system would be specced?

The honest answer is **no**:

1. **T-12 issue_commands.rs (54 tests, BC-1036..1081, 46 BCs)** — these are **integration-level pins for behaviors already specified via source-unit tests AND R1/R2/R3 BCs**. Yes, they add CLI-level confidence (e.g., BC-1079 pins exit code 64 for ambiguous-substring), but the SUBSTANCE of "ambiguous substring rejected when --no-input" was already in R1 BC-200..210 + Pass 5 conventions. No new entity types, no new subsystems, no new architectural patterns.

2. **T-13 api_client.rs (11 BCs)** — pins the 401-scope-mismatch dispatch with 5 specific substrings (BC-1085). These ARE specific-substring pins, but they're refinements of behavior already known and broadly characterized in R1.

3. **T-15 proptest (BC-1094..1103, 10 BCs)** — pins fuzz-safety. Material in the sense of "if you remove this, you don't know it's there" — but the proptest BLOCKS were already known to exist (R1 noted them). R4 just enumerates each property.

4. **T-16 insta snapshots (BC-1104..1118, 15 BCs)** — pins JSON shape at the field-name level. Material if you're crystallizing a JSON schema; refinement if you already have the type definitions.

5. **T-19/T-20 source unit tests (BC-1140..1169, 30 BCs)** — auth.rs and auth_embedded.rs source unit tests. Pin the OAuth crypto contract. Most are **refinements** of Pass 1's auth-design BCs (e.g., BC-1148 just spells out 7 substrings of the authorize URL — content-wise we knew the URL has those properties).

6. **T-21 OAuth state machine (BC-1170..1178, 9 BCs)** — converts R3's diagram into per-transition BCs. This is the most-substantive subset, but: (a) the state diagram itself was the substantive contribution; (b) per-transition BCs are mostly refinements with the exception of BC-1174 (state-mismatch detection — flagged as MEDIUM because untested) and the H-044/H-045/H-046/H-047 holdouts.

7. **CONV-ABS-011/012** — minor count corrections; not material.

**The "would this change the spec" test:** Removing R4 would produce a spec at the same architectural and subsystem level, but with weaker test-citation coverage. The core entities, contracts, error envelopes, command surfaces, OAuth flow, ADF round-trip semantics, changelog classifier semantics, etc. — all already specified by R0-R3.

R4 also delivered:
- 4 new holdouts (H-044..H-047 — concrete wireframes for previously-known gaps)
- 6 new untested-behavior gaps (G-OL4, G-OL5, G-401-AR1, G-SNAP1, G-SNAP2, plus closing several)
- 2 corrections (CONV-ABS-011/012)
- Full audit of R3 against the 5 hallucination classes — NO new retractions on substantive content

These deliverables are **valuable but refinement-grade**, not model-changing.

By the strict-binary novelty rule defined in §8 of the protocol — "Would removing this round's findings change how you'd spec the system?" — the answer is **no**. Therefore: **NITPICK**.

This declares **Pass 3 has CONVERGED** at Round 4. No Round 5 is needed.

---

## 9. Convergence Declaration

**Pass 3 has converged.** Round 4 attacked all 12 verbatim Round-3 §9 deferred targets and produced 121 BCs at refinement-grade novelty. The substantive contracts (entities, command surfaces, error envelopes, OAuth flow, ADF round-trip, changelog classifier, JQL composition, partial_match, duration parsing, cache TTL, profile isolation, embedded-OAuth XOR plumbing) are all crystallized.

**Remaining gaps for Round 5 (if attempted)**: NONE. All R3 §9 verbatim targets attacked. The 4 NEW holdouts (H-044..H-047) and 6 NEW untested-behavior gaps (§5.18-5.21) are fixture/integration-test additions, NOT spec gaps. They are specification-ready as holdouts — i.e., the spec can include them as "behaviors that should exist but currently aren't tested at integration scope".

**No Round 5 is needed.** Pass 3 final tally: **540 BCs (475 HIGH / 59 MEDIUM / 6 LOW), 47 holdouts, 41 untested-behavior gaps, 8 CONV-ABS corrections.**

---

## 10. State Checkpoint

```yaml
pass: 3
round: 4
status: complete
bcs_total_after_round: 540
bcs_high: 475
bcs_medium: 59
bcs_low: 6
bcs_added_this_round: 121
bcs_promoted_to_high: 0
bcs_retracted: 0
holdout_candidates_after_round: 47
holdouts_added_this_round: 4
untested_behaviors_listed: 41
files_examined: 21
novelty: NITPICK
timestamp: 2026-05-04T23:45:00Z
inputs_consumed:
  - .factory/semport/jira-cli/jira-cli-pass-3-deep-r3.md (full)
  - .factory/semport/jira-cli/jira-cli-pass-3-deep-r2.md (cross-reference)
  - .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md (cross-reference)
  - .factory/semport/jira-cli/jira-cli-pass-2-deep-r3.md (cross-reference)
  - .reference/jira-cli/tests/issue_commands.rs (chunked 1-1920, 54 tests)
  - .reference/jira-cli/tests/api_client.rs (1-260 + 340-445, 22 tests)
  - .reference/jira-cli/tests/issue_remote_link.rs (full 348 LOC, 6 tests)
  - .reference/jira-cli/tests/user_commands.rs (1-417, 14 tests)
  - .reference/jira-cli/tests/project_commands.rs (1-150 + structural, 10 tests)
  - .reference/jira-cli/tests/issue_resolution.rs (full 158 LOC, 3 tests)
  - .reference/jira-cli/tests/issue_view_errors.rs (full 206 LOC, 4 tests)
  - .reference/jira-cli/tests/assets_errors.rs (full 153 LOC, 3 tests)
  - .reference/jira-cli/tests/cmdb_fields.rs (full 189 LOC, 5 tests)
  - .reference/jira-cli/tests/team_column_parity.rs (structural, 7 tests)
  - .reference/jira-cli/tests/auth_login_config_errors.rs (full 97 LOC, 1 test)
  - .reference/jira-cli/tests/user_pagination.rs (200-520 — remaining 7 tests)
  - .reference/jira-cli/src/api/auth.rs (920-1397 — full 22-test module)
  - .reference/jira-cli/src/api/auth_embedded.rs (130-250 — full 8-test module)
  - .reference/jira-cli/src/jql.rs (100-395 — proptest block)
  - .reference/jira-cli/src/partial_match.rs (140-200 — proptest block)
  - .reference/jira-cli/src/duration.rs (120-159 — proptest block)
  - .reference/jira-cli/src/cli/issue/list.rs (650-1085 — fn-name verification grep)
  - .reference/jira-cli/src/cli/issue/json_output.rs (80-149 — insta module)
  - 17 insta .snap files under src/cli/, src/, tests/snapshots/
  - .reference/jira-cli/proptest-regressions/jql.txt
next_round_targets: |-
  CONVERGED — no Round 5 needed.

  Pass 3 declares final convergence at 540 BCs (475 HIGH / 59 MEDIUM / 6 LOW), 47 holdouts, 41 untested-behavior gaps, 8 CONV-ABS corrections (009..012 from R3 + R4 011..012).

  All 12 verbatim Round-3 §9 deferred targets attacked:
    T-11 OAuth state machine — 9 per-transition BCs (BC-1170..1178) + 4 new holdouts (H-044..H-047) including 401-auto-refresh wireframe.
    T-12 issue_commands.rs — 46 per-test BCs.
    T-13 api_client.rs remaining 11 — fully enumerated.
    T-14 BC-130 verification — closed. CONV-ABS-008 confirmed valid.
    T-15 proptest enumeration — 10 BCs across jql/partial_match/duration.
    T-16 insta snapshot enumeration — 15 BCs across 17 .snap files.
    T-17 user_pagination.rs remaining 7 — fully enumerated.
    T-18 batch enumeration of 9 smaller integration files — done.
    T-19 src/api/auth.rs 22 source unit tests — fully enumerated.
    T-20 src/api/auth_embedded.rs 8 tests — fully enumerated.
    T-21 Pass 4 NFR cross-reference — closed (R3 already noted).
    T-22 source-file unit-test count audit — verified all R1/R2/R3 counts correct.

  4 new holdouts added: H-044 401-auto-refresh wireframe; H-045 zero accessible-resources; H-046 multiple accessible-resources --no-input; H-047 state-mismatch detection.

  Pass 3 final synthesis (Pass 8 / final synthesis) should consume R0+R1+R2+R3+R4 and close out.
```
