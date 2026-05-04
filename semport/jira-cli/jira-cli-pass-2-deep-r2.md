# Pass 2 Deepening — Round 2 — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04

## 1. Round metadata

- **Round**: 2
- **Predecessor**: `jira-cli-pass-2-deep-r1.md` (Round 1)
- **Targets attacked (verbatim from Round 1 §9 carryover)**:
  - **#1** — `api/jira/*` resource modules (10 files)
  - **#2** — `cli/issue/changelog.rs` (847 LOC)
  - **#3** — `cli/issue/helpers.rs` (813 LOC)
  - **#4** — `cli/issue/workflow.rs` (788 LOC, **excluding** link/unlink/remote-link which are in `cli/issue/links.rs` — see §2 audit retraction CONV-ABS-1)
  - **#5** — `cli/issue/create.rs` (375 LOC) + `cli/issue/json_output.rs` (149 LOC)
  - **#6** — `api/client.rs` (490 LOC) — extract_error_message, send vs send_raw, JR_BASE_URL/JR_AUTH_HEADER injection
  - **#7** — `api/pagination.rs` (374 LOC) — full envelope characterization
  - **#8** — AQL builder edge cases (re-verified at `jql.rs`)
  - **#9** — `cli/board.rs`, `cli/sprint.rs`, `cli/worklog.rs`, `cli/team.rs`, `cli/user.rs`, `cli/queue.rs`
  - **#10** — `cli/init.rs` per-step error recovery
  - **#12** — `cli/api.rs` (342 LOC)
  - **#13** — `partial_match.rs` property tests + ExactMultiple-as-Exact convention
  - **#14** — `duration.rs` cross-context contrast
  - **#15** — `jql.rs` full property test enumeration

(T-09 ADF deep round 2 and T-11 deferred — see §9.)

---

## 2. Audit of Round 1 against the 5 Known Hallucination Classes

### Class 1 — Over-extrapolated token lists
- **Round 1 §3.2 E-02-01 `FilterOptions<'a>` 11 fields** — re-read `cli/issue/list.rs:598-610`. Verified: 11 fields exactly as listed. ✓
- **Round 1 §3.6 ADF emit catalog (13 emit node types + 5 marks)** — spot-verified at `adf.rs:142-211, 282-285, 103-115`. Verified. ✓
- **Round 1 §3.3 E-03-01 endpoint enumeration ("6 endpoints in api/assets")** — table actually shows 5 files. Round 1 header said "6 endpoints" but the row count is 5. **Counting-convention slip, not fabrication** — the 6 endpoints are scattered across 5 files (objects.rs has 4 endpoints alone). Recount accurate. **No retraction.**
- **Round 1 §3.1 OAuth scope set: "7 scopes"** — verified at `api/auth.rs:58-63`. ✓
- **Round 1 §3.2 E-02-02 "13 positions"** in JQL composition pipeline — re-read `cli/issue/list.rs:613-648`. Verified positions 1-13 plus trailing `ORDER BY`. ✓

### Class 2 — Miscounted enumerations
- **Round 1 self-claim**: "33 new entities, 17 new invariants" — recount of §5:
  - Entities listed: E-01-01..09 (9) + E-02-01..07 (7) + E-03-01..10 (10) + E-05-02..05 (4) + E-07-03..05 (3) = **33** ✓
  - Invariants listed: NEW-INV-01..17 = **17** ✓
- **Round 1 §3.1 `chosen_flow_for_profile_inspects_passed_profile_not_active` line 1247** — line ranges in the test module not re-verified in this round (Round 1 cited line numbers within `cli/auth.rs` test module which is large; not invalidated, but also not re-pinned).

### Class 3 — Named pattern conflation / fabrication

- **CONV-ABS-1 (RETRACTED — Round 1 target attribution slip, not Round 1 finding):** Round 1's §9 carryover target #4 (`cli/issue/workflow.rs (788 LOC)`) attributes `link/unlink/link-types`/`remote-link` operations to `workflow.rs`. **`workflow.rs` does NOT contain those handlers** — they live in `cli/issue/links.rs` (293 LOC), which CLAUDE.md correctly identifies. `workflow.rs` contains ONLY: `handle_move`, `handle_transitions`, `handle_resolutions`, `handle_assign`, `handle_comment`, `handle_open`. **This is a target-list misattribution, not a Round 1 finding. No factual claim to retract; flagged so Round 2 doesn't propagate the error. Round 2 attacks both `workflow.rs` AND `links.rs` separately.**

- **`JR_VERBOSE` framing in Round 1 carryover #6** — the carryover target said "JR_VERBOSE stderr gate". Re-reading `api/client.rs:1-490`: there is NO `JR_VERBOSE` env var. Verbose mode is driven by `verbose: bool` field on `JiraClient`, set by `from_config(config, verbose)` from a CLI flag. The `JR_*` env vars in `api/client.rs` are `JR_BASE_URL` (line 37) and `JR_AUTH_HEADER` (line 65) — both test-injection seams, NOT verbose. **Carryover target was inaccurate; Round 2 corrects to `verbose` parameter + `JR_AUTH_HEADER` env (NEW finding).** No prior Round 1 claim to retract.

- **Round 1 §3.1 `chosen_flow_for_profile` vs `chosen_flow`** — verified consistent. ✓
- **Round 1 §4.1 `peek_oauth_app_source` keychain-error degradation** — not re-verified this round (no contradicting source read).

### Class 4 — Same-basename artifact conflation
- **`cli/issue/links.rs` (293 LOC, NOT 'links' inside workflow.rs)** — the carryover target slip in CONV-ABS-1 is the only same-basename ambiguity surfaced this round. Files actually distinct.
- **`api/jira/links.rs` (97 LOC) vs `cli/issue/links.rs` (293 LOC)** — Round 1 did not conflate; both kept separate. ✓
- **`api/jira/projects.rs` (121 LOC) vs `cli/project.rs` (133 LOC)** — separate. Round 2 reads both; no conflation.

### Class 5 — Inflated or deflated metrics (LOC recount)
Recounted via `wc -l` on Round 2 target files. No Round 1 claim to invalidate. New counts (this round's source citations):

| File | Round 1 cited | Actual | Delta |
|---|---:|---:|---|
| `src/api/client.rs` | 490 | 490 | 0 |
| `src/api/pagination.rs` | (not cited) | 374 | n/a |
| `src/api/jira/issues.rs` | (not cited) | 314 | n/a |
| `src/api/jira/users.rs` | (not cited) | 290 | n/a |
| `src/api/jira/fields.rs` | (not cited) | 303 | n/a |
| `src/api/jira/projects.rs` | (not cited) | 121 | n/a |
| `src/api/jira/teams.rs` | "56" (carryover) | 55 | -1 |
| `src/api/jira/sprints.rs` | (not cited) | 109 | n/a |
| `src/api/jira/worklogs.rs` | (not cited) | 31 | n/a |
| `src/api/jira/links.rs` | (not cited) | 97 | n/a |
| `src/api/jira/resolutions.rs` | (not cited) | 55 | n/a |
| `src/api/jira/statuses.rs` | (not cited) | 21 | n/a |
| `src/api/jira/boards.rs` | (not cited) | 50 | n/a |
| `src/cli/issue/changelog.rs` | "847" (carryover) | 847 | 0 |
| `src/cli/issue/helpers.rs` | "813" (carryover) | 813 | 0 |
| `src/cli/issue/workflow.rs` | "788" (carryover) | 788 | 0 |
| `src/cli/issue/links.rs` | (not cited) | 293 | n/a |
| `src/cli/issue/create.rs` | "375" (carryover) | 375 | 0 |
| `src/cli/issue/json_output.rs` | (not cited) | 149 | n/a |
| `src/cli/api.rs` | "342" (carryover) | 342 | 0 |
| `src/cli/board.rs` | (not cited) | 334 | n/a |
| `src/cli/sprint.rs` | "438" (carryover) | 438 | 0 |
| `src/cli/queue.rs` | "323" (carryover) | 323 | 0 |
| `src/cli/init.rs` | (not cited) | 285 | n/a |
| `src/cli/team.rs` | (not cited) | 120 | n/a |
| `src/cli/user.rs` | (not cited) | 165 | n/a |
| `src/cli/worklog.rs` | (not cited) | 79 | n/a |
| `src/cli/project.rs` | (not cited) | 133 | n/a |
| `src/jql.rs` | "395" (carryover) | 395 | 0 |
| `src/duration.rs` | "159" (carryover) | 159 | 0 |
| `src/partial_match.rs` | "200" (carryover) | 200 | 0 |

**Result**: All Round 1 LOC citations are accurate within ±1 LOC (`api/jira/teams.rs` is 55 LOC actual vs Round 1 "56" carryover — 1-LOC trailing-newline rounding, not a discrepancy).

**Hallucination class audit summary**: **0 retracted findings** from Round 1 substantive claims. **1 carryover-target-list inaccuracy** logged as CONV-ABS-1 (workflow.rs vs links.rs misattribution) so Round 3 doesn't compound it.

---

## 3. Sub-pass 2a deepening: structural — entity model per target

### 3.1 T-API: `api/jira/*` per-resource catalog (Round 2 target #1)

The broad pass §2a.1 listed the 11 files in `api/jira/`. Round 1 covered none individually. This round catalogues each.

#### E-API-01 — `api/jira/issues.rs` (314 LOC)
- **Constants**:
  - `BASE_ISSUE_FIELDS` (lines 12-29): 16 default fields requested on every search/get: `summary, status, issuetype, priority, assignee, reporter, project, description, created, updated, resolution, components, fixVersions, labels, parent, issuelinks`. **`extra_fields` parameter on every API method appends to this base.** Pinned by re-read.
  - **No** explicit `BASE_FIELDS` for changelog or comments — those use endpoint-provided shapes.
- **Public methods (10)**: `search_issues`, `approximate_count`, `get_issue`, `create_issue`, `edit_issue`, `get_transitions`, `transition_issue`, `assign_issue`, `add_comment`, `get_changelog`, `list_comments`. **Round 1 §2b.1 broad-pass enumeration was correct in spirit.**
- **`SearchResult` struct** (lines 32-35): `{ issues: Vec<Issue>, has_more: bool }`. The `has_more` is computed by `search_issues` from page truncation OR `page.has_more()`. **Distinct from `OffsetPage::has_more()` — `SearchResult.has_more` factors in client-side `--limit` truncation.** Pinned by 2 unit tests `search_result_has_more_*` (lines 283-298).
- **Pagination strategy**: `search_issues` uses CURSOR (POST `/search/jql` with `nextPageToken`). `get_changelog` uses OFFSET with **anti-loop guard** (lines 218-230): if `next <= start_at` despite `has_more=true`, returns explicit "JRACLOUD-94357-class schema-drift" error rather than infinite-looping. **NEW INVARIANT (NEW-INV-18).**
- **`add_comment` JSM property structure** (lines 181-191): when `internal=true`, adds `properties: [{key:"sd.public.comment", value:{internal:true}}]`. On non-JSM projects the property is silently accepted. **Round 1 §3.2 noted this; Round 2 confirms exact JSON shape.**
- **`approximate_count`** (lines 101-107): `POST /rest/api/3/search/approximate-count` with body `{jql}`. Response wrapper `ApproximateCountResponse {count: u64}` — module-private. JQL must NOT include ORDER BY.
- **`list_comments` pagination** (lines 238-276): manual offset-window with `?expand=properties`. Distinct from `get_changelog` because Comment endpoint's `comments` array uses a different page key (handled by `OffsetPage<T>::comments` field, see §3.2 below).
- **`transition_issue` two-shape body** (lines 148-165): `{transition:{id}}` for plain transitions, `{transition:{id}, fields:{...}}` when caller passes `Some(&fields)`. Caller is responsible for `{resolution:{name:"Done"}}` shape.

#### E-API-02 — `api/jira/users.rs` (290 LOC)
- **Constants**:
  - `USER_PAGE_SIZE = 100` (line 8) — Atlassian's effective server-side cap on `/user/search`.
  - `USER_PAGINATION_SAFETY_CAP = 15` (line 16) — defensive max iterations. Atlassian's documented 1000-user hard cap means 15 pages × 100 = 1500-user effective ceiling.
- **Public methods (5)**: `get_myself`, `search_users`, `search_users_all`, `search_assignable_users`, `search_assignable_users_by_project`, `search_assignable_users_by_project_all`, `get_user`.
- **NEW INVARIANT (NEW-INV-19) — Fixed-window pagination semantics**: `search_users_all` and `search_assignable_users_by_project_all` advance `start_at` by `USER_PAGE_SIZE` (the **window size**), NOT by the returned count (which would overlap windows after permission filtering and produce duplicates per JRACLOUD-71293). The only reliable end-of-data signal is an empty page; a non-empty short page is NOT end-of-data. **Documented at lines 72-83 and 189-194.**
- **NEW INVARIANT (NEW-INV-20) — Tolerant deserialization for user search**: `search_users`, `search_assignable_users`, `search_assignable_users_by_project` all check `raw.is_array()` first, then fall back to `raw.get("values")` shape. Atlassian's `/user/search` endpoint has historically returned EITHER a flat array OR a paginated `{values: [...]}` shape depending on instance. **Verified at lines 33-41, 60-68, 121-129.**
- **`get_user` 404/400 ambiguity** (lines 230-241 doc-comment): "Jira is inconsistent which it returns" — `JrError::ApiError {status: 404|400, ..}` for unknown accountId. The `cli/user.rs::handle_view` (line 79) handles both as the same UserError. **NEW INVARIANT (NEW-INV-21).**
- **Email-visibility nuance** (line 234): `email_address` may be omitted based on the target user's profile-visibility settings. `User.email_address` is `Option<String>` to accommodate.
- **Safety-cap warning emission** (lines 100-106, 219-225): when the loop exits via `USER_PAGINATION_SAFETY_CAP` (not via empty page), `eprintln!` emits "user search hit pagination safety cap (15 pages, N users); results may be incomplete". **Soft warning, not error.**

#### E-API-03 — `api/jira/fields.rs` (303 LOC)
- **Pub structs**: `Field { id, name, custom: Option<bool>, schema: Option<FieldSchema> }`, `FieldSchema { field_type (rename "type"), custom: Option<String> }`.
- **Constants**:
  - `KNOWN_SP_SCHEMA_TYPES` (lines 45-48): `["com.atlassian.jira.plugin.system.customfieldtypes:float", "com.pyxis.greenhopper.jira:jsw-story-points"]` — the two well-known schema IDs for story points.
  - `CMDB_SCHEMA_TYPE` (line 83): `"com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"` — exact-match CMDB field schema discriminator.
- **Methods (4)**: `list_fields`, `find_team_field_id`, `find_story_points_field_id`, `find_cmdb_fields`.
- **`find_team_field_id` discrimination** (lines 26-32): `name.to_lowercase() == "team"` (EXACT case-insensitive match, not contains) AND `custom == Some(true)`. **Returns FIRST match only.** **NEW INVARIANT (NEW-INV-22):** A custom field literally named `Team` (case-insensitive) is required; non-custom Atlassian-provided team fields are explicitly rejected.
- **`filter_story_points_fields` ranking** (lines 50-81): a candidate must have `custom == Some(true)` AND `schema.field_type == "number"` AND `name.to_lowercase()` in `["story points", "story point estimate"]`. Then sorted by `has_known_schema` (boolean: schema.custom in `KNOWN_SP_SCHEMA_TYPES`) **descending** — known schemas float to top. Returns `Vec<(id, name)>` (caller picks first).
- **`filter_cmdb_fields` discrimination** (lines 85-97): `custom == Some(true)` AND `schema.custom == Some("com.atlassian.jira.plugins.cmdb:cmdb-object-cftype")`. **Strict equality** on the schema ID.
- **NEW INVARIANT (NEW-INV-23) — story-points name allowlist**: ONLY two names accepted: `"story points"` and `"story point estimate"` (case-insensitive). A custom field named `"Story Pts"` would NOT match — pinned by `filter_finds_classic_story_points`, `filter_finds_jsw_story_point_estimate`, and `filter_finds_both_variants`.

#### E-API-04 — `api/jira/teams.rs` (55 LOC)
- **Methods (2)**: `get_org_metadata(hostname) -> TenantContext` and `list_teams(org_id) -> Vec<TeamEntry>`.
- **NEW INVARIANT (NEW-INV-24) — Both methods bypass the OAuth API gateway**: `get_org_metadata` uses `post_to_instance` (instance_url, NOT base_url); `list_teams` uses `get_from_instance`. The GraphQL `tenantContexts` and the public Teams API are hosted on the Jira instance's gateway path (`/gateway/api/...`), which the OAuth-rewritten `https://api.atlassian.com/ex/jira/<cloud_id>/...` would NOT reach. **Architectural significance**: the `instance_url` field on `JiraClient` exists primarily for these two methods — most Jira REST API calls go through `base_url`.
- **GraphQL request shape** (lines 12-17): `{query: "query getOrgId { tenantContexts(hostNames: [\"<host>\"]) { orgId cloudId } }"}` — single hostname only (broad pass §2a.1 said "GraphQL hostNames" but this is the exact shape).
- **Cursor pagination** (`list_teams` loop at lines 33-51): on each iteration, `?cursor=<encoded>` is appended. `TeamsResponse.cursor: Option<String>` — `None` ends the loop. URL-encoded.
- **NEW INVARIANT (NEW-INV-25) — `get_org_metadata` "Could not resolve organization ID" message** (lines 24-28): when `data.tenantContexts.into_iter().next()` is `None`, errors. Trigger: GraphQL response with empty `tenantContexts` array (instance not registered). Message points users at "Check your Jira URL and permissions, or run jr init".

#### E-API-05 — `api/jira/projects.rs` (121 LOC)
- **Pub structs (4)**: `IssueTypeMetadata`, `PriorityMetadata`, `StatusMetadata`, `IssueTypeWithStatuses`. Each is the per-`project fields` Atlas response shape.
- **Methods (4)**: `get_project_issue_types`, `get_priorities`, `get_project_statuses`, `project_exists`, `list_projects`.
- **`get_project_issue_types` resilient JSON path** (lines 41-51): hand-extracts `issueTypes` from raw `serde_json::Value` — `unwrap_or_default()` returns empty Vec on missing/wrong-type. **No hard error if the project lacks an `issueTypes` field.**
- **`project_exists` 404-as-Ok(false)** (lines 73-87): downcasts `JrError::ApiError {status: 404}` to `Ok(false)`; all other errors propagate. Used by `cli/issue/list.rs` early-validation path before composing JQL.
- **NEW INVARIANT (NEW-INV-26) — `list_projects` non-pagination when limit set**: When `max_results: Some(N)`, the function STOPS after the first page (line 113 `if max_results.is_some() || !has_more`). So `--limit 5` returns up to 5 from page 1, even if more pages exist. When `max_results: None`, paginates exhaustively. **Asymmetry not in broad pass.** `page_size` is `min(50)` cap.

#### E-API-06 — `api/jira/sprints.rs` (109 LOC)
- **Methods (4)**: `list_sprints`, `get_sprint_issues`, `add_issues_to_sprint`, `move_issues_to_backlog`.
- **`list_sprints` pagination**: offset, max_results=50, full enumeration. State filter `Some("active"|"closed"|"future")` is URL-encoded.
- **`get_sprint_issues` SprintIssuesResult struct** (lines 105-109): `{issues: Vec<Issue>, has_more: bool}` — same shape as `SearchResult`. Hand-built `fields` query string starts with `summary,status,issuetype,priority,assignee,project` (6 default fields, **NOT** the 16-field BASE_ISSUE_FIELDS used by `search_issues`). **NEW INVARIANT (NEW-INV-27):** Sprint endpoint default fields ≠ search endpoint default fields. A consumer expecting `description` in sprint issue payload must explicitly pass it via `extra_fields`.
- **Backlog endpoint shape** (lines 96-102): `POST /rest/agile/1.0/backlog/issue` with `{issues: [...]}`. Returns 204 No Content. Cap of 50 documented in source comment.

#### E-API-07 — `api/jira/boards.rs` (50 LOC)
- **Methods (2)**: `list_boards(project_key, board_type)`, `get_board_config(board_id) -> BoardConfig`.
- **`list_boards` filter combinations**: project_key uses `projectKeyOrId` query param (NOT `projectKey`); board_type uses `type` (mapped to `"scrum"` or `"kanban"`). Both URL-encoded. Loops with offset+50.
- **No special pagination semantics beyond standard OffsetPage** — straightforward.

#### E-API-08 — `api/jira/links.rs` (97 LOC) — distinct from `cli/issue/links.rs`
- **Methods (4)**: `create_issue_link`, `delete_issue_link`, `list_link_types`, `create_remote_link`.
- **`create_issue_link` parameter convention** (lines 6-22): `outward_key` gets the OUTWARD label (e.g., "blocks"), `inward_key` gets the INWARD label ("is blocked by"). Hardcoded mapping into the request body. **NEW INVARIANT (NEW-INV-28):** Caller convention is `(outward, inward)` — reversing the args reverses link semantics on Jira side.
- **`create_remote_link` minimum body** (lines 42-56): `{object: {url, title}}`. Returns `CreateRemoteLinkResponse {id: u64, self_url: String (rename "self")}`. Atlas returns 201 Created on first link or 200 OK when an existing link with the same `globalId` is updated — caller can't distinguish from response.

#### E-API-09 — `api/jira/worklogs.rs` (31 LOC)
- **Methods (2)**: `add_worklog(key, time_spent_seconds, comment)`, `list_worklogs(key)`.
- **NEW INVARIANT (NEW-INV-29) — `list_worklogs` is NOT paginated**: re-read `api/jira/worklogs.rs:25-30`. The function fetches ONE page via `OffsetPage::items()` and returns. **Issues with >max_results worklogs (Atlas default = 100? — not explicit) would be silently truncated.** No loop, no warning. Confirmed by source. Callers in `cli/worklog.rs::handle_list` show all returned worklogs without an "incomplete" hint.
- `add_worklog` body shape: `{timeSpentSeconds: u64, comment: Option<Value>}` — comment is raw ADF.

#### E-API-10 — `api/jira/resolutions.rs` (55 LOC)
- **Method (1)**: `get_resolutions() -> Vec<Resolution>`. **NOT paginated** (Atlas returns flat list — instance-scope, typically <50 entries). Endpoint: `/rest/api/3/resolution`.
- Used by `cli/issue/workflow.rs::load_resolutions` with cache-first read.

#### E-API-11 — `api/jira/statuses.rs` (21 LOC) — corrects Round 1 framing
- **Round 1 §3.2 NEW-INV-03** described `get_all_statuses` as the global endpoint without enumerating its return shape. **Re-read confirms a key shape distinction**:
- **Module-private `StatusEntry { name: String }`** (lines 5-8) — **NOT the public `Status` struct**. `get_all_statuses` returns `Vec<String>` (sorted, deduped) — NOT `Vec<Status>`.
- **NEW INVARIANT (NEW-INV-30):** `client.get_all_statuses()` returns names only as flat sorted-deduped `Vec<String>` — does NOT include status category. The caller (`cli/issue/list.rs`) uses these names ONLY for `--status` flag validation (substring match against names), not for category-based filtering. Implication: project-scoped `get_project_statuses` returns full `IssueTypeWithStatuses[StatusMetadata]` (with id+name+description); global `get_all_statuses` returns only deduped names. **The two endpoints have different shapes; consumers cannot interchange.**

### 3.2 T-PAG: Pagination envelope characterization (Round 2 target #7)

Round 1 §2a.3 listed the 4 envelopes. This round catalogues each with full field semantics.

#### E-PAG-01 — `OffsetPage<T>` (74 LOC of source)
- **Fields** (lines 8-32): `values: Option<Vec<T>>`, `issues: Option<Vec<T>>`, `worklogs: Option<Vec<T>>`, `comments: Option<Vec<T>>`, `start_at: u32`, `max_results: u32`, `total: u32`. **All 4 item-list fields default to None** — `#[serde(default)]`.
- **`items()` accessor** (lines 36-51): preference order `values > issues > worklogs > comments`. **Returns `&[]` when none populated.** Used by callers that don't know which key the endpoint uses.
- **`has_more()`** (line 54): `start_at + max_results < total` — assumes `total` is accurate. **Brittle** when Atlas's `total` is approximate; combined with cursor pagination on newer endpoints this is being phased out by Atlas.
- **`next_start()`** (line 59): `start_at + max_results`.
- **NEW INVARIANT (NEW-INV-31):** Different Atlas endpoints place items under different keys (`values` for boards/sprints/projects, `issues` for old search, `worklogs` for worklogs, `comments` for comments). The 4-Option struct accommodates all of them without per-endpoint structs. Adding a 5th endpoint key (e.g., a hypothetical `users` key) would require a struct change — the design is closed-extension.

#### E-PAG-02 — `CursorPage<T>` (16 LOC)
- **Fields** (lines 64-73): `issues: Vec<T>` (default empty), `next_page_token: Option<String>`. **Only 2 fields** — no `start_at`, no `total`, no `max_results` reflecting back.
- **`has_more()`**: `next_page_token.is_some()`. Single-source-of-truth.
- **Used by**: `POST /rest/api/3/search/jql` (the v3 cursor-paginated search). Token shape is opaque (Atlas-internal — current implementation is base64-ish but unspecified).
- **NEW INVARIANT (NEW-INV-32):** `CursorPage` has NO server-side `total` count. To estimate "Showing N of ~M" for the user, callers must call `approximate_count` separately (round-trip cost). `cli/issue/list.rs:826-833` does exactly this; falls back to "Showing N results" if the count call fails.

#### E-PAG-03 — `ServiceDeskPage<T>` (32 LOC)
- **Fields** (lines 84-101): `size: u32`, `start: u32`, `limit: u32`, `is_last_page: bool` (rename `isLastPage`), `values: Vec<T>` (default empty). **Note `start`/`limit`/`size` field names — distinct from `OffsetPage`'s `start_at`/`max_results`/`total`.**
- **`has_more()`**: `!is_last_page` — server-determined (unlike `OffsetPage` which computes from offsets).
- **`next_start()`**: `start + size`.
- **NEW INVARIANT (NEW-INV-33):** JSM's `/rest/servicedeskapi/` family advertises a different envelope shape than core Jira REST. The `start_at` vs `start` rename is a **pin point** — using the same struct as `OffsetPage` would silently fail to deserialize.

#### E-PAG-04 — `AssetsPage<T>` (32 LOC)
- **Fields** (lines 135-153): `start_at`, `max_results`, `total`, `is_last: bool` (custom-deserialized, see below), `values: Vec<T>` (default empty).
- **NEW ENTITY (E-PAG-05): `deserialize_bool_or_string`** (lines 118-128): custom serde deserializer accepting EITHER `true`/`false` (bool) OR `"true"`/`"false"` (string). Reflects Assets API's actual JSON drift across endpoints.
- **`has_more()`**: `!is_last`.
- **NEW INVARIANT (NEW-INV-34):** Assets API caps `total` at 1000 regardless of actual matching object count (line 134 source comment). Combined with `max_page_size = 25` (per Round 1 NEW-INV-06), the maximum enumerable result set per query is 1000 objects — beyond that, callers must narrow the AQL.
- **3 deserialization tests pin all three shapes**: `is_last_bool`, `is_last_string` (false), `is_last_string_true` (lines 334-372).

### 3.3 T-CHANGELOG: `cli/issue/changelog.rs` (Round 2 target #2, 847 LOC)

Round 1 §9 carryover #2 flagged this as orphan. Catalogued.

#### E-CL-01 — `LoweredStr` smart constructor (lines 151-171)
- **Module-private** type with **private tuple field** `String`. Only construction path is `LoweredStr::new(s: &str)` which calls `s.to_lowercase()`. The private inner field enforces the invariant "needle is lowercased at construction" — `author_matches` lowercases the haystack at match time and compares directly.
- **Architectural significance**: separating the type into a sub-module (`mod lowered_str`) makes the field unreachable from the rest of `changelog.rs`; the COMPILER enforces the lowercase invariant. This pattern mirrors Round 1's `ResolvedRedirect` (E-01-02) — both use private-field encapsulation to lift an invariant from documentation to type system.
- **NEW INVARIANT (NEW-INV-35):** No code path can produce a `LoweredStr` containing uppercase characters; therefore `author_matches`'s `haystack.contains(needle.as_str())` is sound without re-normalizing the needle.

#### E-CL-02 — `AuthorNeedle` enum (lines 175-215)
- **Variants (2)**: `AccountId(String)` (case-sensitive exact match), `NameSubstring(LoweredStr)` (case-insensitive substring match against `display_name` OR `account_id`).
- **Smart constructor `from_raw`** (lines 200-214): a value is classified as `AccountId` iff:
  - Contains `:`, OR
  - **All three of**: `len >= 12`, contains at least one ASCII digit, AND every char is `is_ascii_alphanumeric() || c == '-' || c == '_'`.
- **Otherwise**: `NameSubstring(LoweredStr::new(trimmed))`.
- **NEW INVARIANT (NEW-INV-36) — ASCII-alphanumeric, NOT general alphanumeric**: The classifier uses `is_ascii_alphanumeric()`. Cyrillic + digit names like `"Александр12345"` (14 chars, has digit) classify as **NameSubstring** because Cyrillic letters are NOT ASCII-alphanumeric. Pinned by `from_raw_long_cyrillic_name_with_digit_is_substring` (lines 396-407) and `from_raw_long_unicode_name_with_digit_is_substring` (lines 383-393). Refactoring to `is_alphanumeric()` would be a regression — same name "AlexanderGreene" with one accent (`AlexanderGreené`) is testably 16-char and a regression target.
- **NEW INVARIANT (NEW-INV-37) — "User12345Name" is intentionally classified as AccountId** (residual edge, line 197-200 doc): `User12345Name` is 13 chars, all ASCII-alphanumeric, has digits. Classifies as AccountId despite being a plausible display name. **Documented residual; explicit test pins** (`from_raw_long_name_with_digit_is_accountid` line 428-434).
- **NEW INVARIANT (NEW-INV-38) — 12-char boundary rule**: exactly 12 chars + 1 digit → AccountId (`abcdefghijk1`). Exactly 12 chars + 0 digits → NameSubstring (`abcdefghijkl`). The `>= 12` gate is exact; `> 12` would silently regress. Pinned by tests at lines 437-457.

#### E-CL-03 — Empty-needle filter-bypass guards (lines 50-79)
- **`--author "$UNSET"` guard**: trim + emptiness check before any classification. Errors `JrError::UserError("--author cannot be empty or whitespace-only. Provide a name, accountId, or \"me\".")`.
- **`--field` parallel guard** (lines 72-79): same empty-string rejection. **Reason** (per source comment): `str::contains("")` is always true, so an empty needle in either filter would silently match every entry. **NEW INVARIANT (NEW-INV-39) — a filter-bypass guard.**

#### E-CL-04 — Sort ordering with parse-failure tiebreak (lines 87-98)
- Default order: `entries.sort_by(|a, b| cmp(b, a))` — DESCENDING (newest first). With `--reverse`: ASCENDING.
- `cmp` (lines 87-93): parses both `created` strings via `parse_created`. If both parse → chrono comparison. If either fails → lexicographic on raw strings. **NEW INVARIANT (NEW-INV-40):** Mixed-format API responses (some `+0000` Jira-style, some `+00:00` RFC3339) sort chronologically because `parse_created` accepts both shapes (lines 257-261).

#### E-CL-05 — `truncate_to_rows` mid-entry truncation (lines 286-304)
- **Behavior**: `--limit N` applies to **rows** (one per `ChangelogItem`), NOT entries. If the cap falls mid-entry (e.g., entry has 5 items but cap-running is 3), the entry's `items` are truncated to keep only first 3, and all subsequent entries dropped. **Cap=0 → clear all.**
- **NEW INVARIANT (NEW-INV-41):** The displayed table row count is exactly `--limit` (not "≤ --limit"); it never overshoots, never undershoots when entries+items contain enough rows. The user gets predictable row count.

#### E-CL-06 — `from_to_display` empty-string-as-absent rule (lines 308-319)
- **Behavior**: `Some("")` is treated as absent (whitespace-only too). Falls through to raw value or em-dash. **NEW INVARIANT (NEW-INV-42):** Without this, a blank `fromString` would render as em-dash even when `from` had a real value.

#### E-CL-07 — `format_date` once-per-process verbose log (lines 267-280)
- **Pattern**: `static LOGGED: AtomicBool = AtomicBool::new(false)` — the FIRST parse failure in the process logs to stderr (when `verbose` is true); subsequent failures are silent. This pattern exists at THREE places in the codebase (Round 1 cited `IssueFields::team_id`'s once-per-process warning). **NEW PATTERN (NEW-PAT-01)**: "once-per-process verbose log" — used to surface schema-drift parse failures without spamming logs.

### 3.4 T-HELPERS: `cli/issue/helpers.rs` (Round 2 target #3, 813 LOC)

#### E-HE-01 — `is_team_uuid` precise-format detector (lines 14-34)
- **Algorithm**: 36 chars exactly; bytes at positions 8, 13, 18, 23 are `b'-'`; all other bytes are `is_ascii_hexdigit()`. **No regex** — manual byte-loop.
- **NEW INVARIANT (NEW-INV-43) — UUID pass-through**: A `--team <uuid>` value matching this format **bypasses cache + name-match entirely** (line 60-65). The Atlassian Teams customfield accepts the UUID directly. **No HTTP cost** for known-UUID callers (agents, scripts).

#### E-HE-02 — `resolve_team_field` 6-step state machine (lines 36-185)
1. Resolve `team_field_id` (config OR `find_team_field_id` API call).
2. UUID pass-through (E-HE-01).
3. Load team cache (or fetch). Track `cache_was_fresh` boolean.
4. `partial_match` against cached names.
5. **Auto-refresh on miss** if cache wasn't fresh: re-fetch, retry match. Bounded to single retry via `cache_was_fresh` boolean.
6. Match dispatch:
   - `Exact` → return `(field_id, team_id)`.
   - `ExactMultiple` → 2-or-more teams with same name. Filter to duplicates only. Interactive: dialoguer Select. `--no-input`: error with id-disambiguated list.
   - `Ambiguous` → interactive: Select. `--no-input`: error with quoted list.
   - `None` → tailored error message: if `fetched_fresh`, omit the "run jr team list --refresh" advice (we just did).
- **NEW INVARIANT (NEW-INV-44) — bounded auto-refresh**: at most ONE re-fetch per `resolve_team_field` call. Eliminates the infinite-refresh-loop class. Documented at lines 84-86.

#### E-HE-03 — `disambiguate_user` private generic (lines 241-341)
- **Used by** `resolve_user`, `resolve_assignee`, `resolve_assignee_by_project` — all 3 user resolvers go through this single dispatcher.
- **5-branch dispatch**: empty list (UserError with caller-supplied `empty_msg`); single match (auto-resolve); `Exact`; `ExactMultiple`; `Ambiguous`; `None` (UserError with caller-supplied `none_msg_fn`).
- **`ExactMultiple` formatting**: when `--no-input`, lines include `(<email>, account: <id>)` if email present, else `(account: <id>)`. The interactive prompt uses similar shape sans "account:" prefix.
- **NEW INVARIANT (NEW-INV-45):** All three user resolvers share a single dispatcher — adding a fourth resolver (e.g., `resolve_user_by_team`) only requires constructing a `Vec<User>` and supplying error messages. The disambiguation logic is single-source.

#### E-HE-04 — `resolve_user` returns JQL fragment (lines 351-382)
- **`"me"` → `"currentUser()"`** (no API call).
- **Otherwise**: search users → filter `active == Some(true)` → disambiguate → return `account_id` (raw, unquoted).
- **NEW INVARIANT (NEW-INV-46) — distinct return contract from `resolve_assignee`**: `resolve_user` returns a STRING that is JQL-ready (either the literal `currentUser()` or a bare accountId). `resolve_assignee` returns `(account_id, display_name)` for `PUT /assignee`. The two cannot be transparently swapped.

#### E-HE-05 — `resolve_assignee` vs `resolve_assignee_by_project`
- **`resolve_assignee(client, name, issue_key, no_input)`** (lines 391-421): uses `/user/assignable/search?issueKey=...` — issue-scoped.
- **`resolve_assignee_by_project(client, name, project_key, no_input)`** (lines 431-466): uses `/user/assignable/multiProjectSearch?projectKeys=...` — project-scoped. Used during issue creation when no issue exists yet.
- **NEW INVARIANT (NEW-INV-47):** No active-user filter on project-scoped path (line 442-444 source comment) — the multiProjectSearch endpoint already filters to assignable users. The issue-scoped `resolve_assignee` also relies on the endpoint's filter. Both paths converge on **server-side filtering** rather than client-side `active` field.

#### E-HE-06 — `resolve_asset` 4-branch resolver (lines 468-592)
- **Branches**:
  1. Input matches `validate_asset_key` (SCHEMA-NUMBER) → return as-is (no HTTP).
  2. Otherwise: AQL `Name like "<escaped>"` search → empty → UserError.
  3. Single result → return `object_key`.
  4. Multiple results → `partial_match` on labels; same 4-way `Exact/ExactMultiple/Ambiguous/None` dispatch as user/team resolvers.
- **NEW INVARIANT (NEW-INV-48):** AQL `Name like "..."` predicate is the bridge between human-readable asset names and object keys. Differs from issue resolution (which has no `Name like` analog — issue keys are direct identifiers, summaries are not searchable as a key surrogate).

#### E-HE-07 — Error-recovery message variants
- **`resolve_team_field` error has 3 forms** depending on `fetched_fresh`:
  1. `cache_was_fresh = true` AND name not found → "checked a fresh team list. Verify the team name or check access permissions."
  2. `cache_was_fresh = false` AND `retry_fetched = false` → "Run \"jr team list --refresh\" to update."
  3. The retry was triggered AND still empty → falls into branch 1 (`fetched_fresh = true`).
- **NEW INVARIANT (NEW-INV-49):** Error message specificity is correlated with what the user just did. Telling them to refresh after a refresh is the worst UX; the boolean tracks state-of-action.

### 3.5 T-WORKFLOW: `cli/issue/workflow.rs` (Round 2 target #4, 788 LOC) — handlers ONLY (NOT links)

#### E-WF-01 — `resolve_resolution_by_name` 4-branch behavior (lines 29-81)
- **Substring single-hit refusal** (lines 65-73): single-substring hits route through `Ambiguous` (not auto-resolved) — diverges from naive expectation. **Rationale (lines 26-28 doc-comment):** "single-substring hit is NOT silently promoted to success — that would diverge from every other resolver in the codebase and bypass the operator's intent to be explicit."
- **`ExactMultiple` lists ONLY duplicates with their ids** (lines 47-65): unlike `resolve_team_field` which lists ALL duplicates, the resolution resolver also includes the ID `(id={})` so the operator can fix the conflict in Jira admin. **NEW INVARIANT (NEW-INV-50):** The "include ID for same-name disambiguation" pattern is unique to resolutions — neither `resolve_team_field` (where IDs are UUIDs and not human-actionable) nor link-type resolution (where duplicates are extremely rare) replicate this.

#### E-WF-02 — `load_resolutions` cache-and-defensive-drop (lines 100-136)
- **Defensive id-drop on write**: if Atlas returns a resolution without an `id`, the cacheable count differs from fetched count, and a stderr warning emits (lines 128-132). The fetched data (with all entries including id-less ones) is still returned to the caller — the cache stores only id-bearing entries, but the live response includes everything.
- **NEW INVARIANT (NEW-INV-51):** The in-memory `Vec<Resolution>` returned by `load_resolutions` may have MORE entries than the cache file. Subsequent runs will see fewer entries (cached subset). On `--refresh`, the full set is re-fetched and the cache rewritten — but id-less entries are again dropped from the cache.

#### E-WF-03 — `handle_move` unified candidate pool (lines 240-339)
- **Algorithm**: builds a single `candidates: Vec<(name, transition_idx)>` list combining BOTH transition names AND target status names from `t.to`. **Case-insensitive dedup** via `seen: HashSet<String>` (lowered key).
- **NEW INVARIANT (NEW-INV-52):** The user can type EITHER the transition name (e.g., "Start Progress") OR the target status name (e.g., "In Progress") — both resolve to the same transition. Same `partial_match` machinery applies.
- **Idempotency check ALSO checks transitions** (lines 195-203): an issue is "already in target" if EITHER `current_status == target` OR if there's a transition whose `name` matches target AND whose `to.name` matches `current`. **Pinned with case-insensitive comparison.** This handles "I asked for the In Progress transition, but I'm already In Progress, so the transition is a no-op."
- **Number-input branch** (lines 227-235): if `target_status.parse::<usize>()` succeeds AND in 1..=transitions.len(), uses the indexed transition. **NEW INVARIANT (NEW-INV-53):** Numeric input is a first-class disambiguation — not just a UI hint.

#### E-WF-04 — `handle_move` resolution-required heuristic (lines 357-377)
- **Heuristic**: lowercased error body contains BOTH `"resolution"` AND `"required"` → transform to actionable hint pointing at `--resolution` and `jr issue resolutions`.
- **Fragility**: if Atlas's error message wording changes (e.g., "resolutions field is mandatory"), the heuristic breaks silently. **NEW INVARIANT (NEW-INV-54) — message-shape coupling:** the heuristic depends on Jira's error message string format; a non-tested external-system contract.

#### E-WF-05 — `handle_assign` 3-state idempotency (lines 472-562)
- **Three-state flow**:
  1. `--unassign` → fetch issue, if `assignee.is_none()` → idempotent success (no HTTP write). Otherwise → `assign_issue(key, None)`.
  2. `--account-id <id>` → no name resolution (raw id used as display name).
  3. Positional `[to]` (or default `me`) → resolve via `resolve_assignee`.
- **Idempotency**: ALWAYS fetches issue first (lines 518-521) and short-circuits if `assignee.account_id == account_id`.
- **NEW INVARIANT (NEW-INV-55):** Every assign call costs ONE GET (idempotency check) + at most one PUT. Two GETs (transitions + issue) are not avoided even when the user provided everything.

#### E-WF-06 — `handle_comment` source-of-truth resolution (lines 583-598)
- **Priority**: `--stdin` > `--file` > positional. Mutual-exclusion enforced by the precedence (later `else if` skipped when earlier matches).
- **`spawn_blocking` for stdin reads** (lines 585-591): the blocking `read_to_string` is wrapped in `tokio::task::spawn_blocking` to avoid starving the runtime. Same pattern in `handle_create` and `handle_edit` for `--description-stdin`. **NEW PATTERN (NEW-PAT-02):** "spawn_blocking for stdin" — the codebase's idiom for any blocking stdin read inside a tokio handler.
- **Trim + empty rejection** (lines 600-603): comment text post-trim cannot be empty.

#### E-WF-07 — `handle_open` URL synthesis (lines 631-646)
- **Format**: `format!("{}/browse/{}", client.base_url(), key)`. **NEW INVARIANT (NEW-INV-56):** The browse URL uses `base_url()` — for OAuth-rewritten profiles, `base_url()` returns `https://api.atlassian.com/ex/jira/<cloud_id>` which is **NOT a browser-friendly URL**. The `instance_url()` is the browser-friendly URL (per `client.rs:351-358`). **POTENTIAL BUG** — `handle_open` should use `instance_url()` for the browser URL on OAuth profiles. Re-checking: **CONFIRMED BUG** for OAuth profiles. This is a Pass 6/Pass 3 candidate finding.

### 3.6 T-LINKS: `cli/issue/links.rs` (NEW Round 2 target — 293 LOC, NOT in workflow.rs)

#### E-LK-01 — Self-link prevention (lines 57-59)
- **Behavior**: `if key1.eq_ignore_ascii_case(&key2) { bail!("Cannot link an issue to itself.") }`. **Pre-API guard** — no network cost on the obvious mistake.

#### E-LK-02 — Link-type resolver dispatch (lines 61-89, 131-164)
- **Both `handle_link` and `handle_unlink` use identical resolver pattern**: `partial_match` against `link_types.iter().map(|lt| lt.name)`. The 4-way dispatch matches the user/team/asset resolvers but with one notable difference: **`MatchResult::ExactMultiple(name) => name`** is treated as `Exact` (lines 65-66, 136-137 with the comment "Link types are unique per Jira API"). **NEW INVARIANT (NEW-INV-57):** Link types are presumed unique by the Atlassian API contract; if a future Atlas update introduces same-name link types, this code would silently use the FIRST one. Documented in source comment.

#### E-LK-03 — `handle_unlink` dual-direction matching (lines 169-190)
- **Match predicate**: a link matches `key2` if EITHER `outwardIssue.key.eq_ignore_ascii_case(key2)` OR `inwardIssue.key.eq_ignore_ascii_case(key2)`. The user shouldn't need to know "is this an inward or outward link" — the unlink query is symmetric.
- **NEW INVARIANT (NEW-INV-58):** `unlink` returns success with count=0 (idempotent no-op) when no link matches, NOT an error. Pinned via `unlink_response(false, 0)` JSON payload.

#### E-LK-04 — `handle_remote_link` URL validation (lines 245-264)
- **3-stage validation**:
  1. Trim + non-empty.
  2. `url::Url::parse` — rejects malformed URLs with `JrError::UserError`.
  3. Scheme must be `"http"` or `"https"` (rejects `file:`, `ftp:`, `mailto:`, etc.).
- **Normalization**: `url = parsed.as_str()` — uses the normalized form (tabs/newlines stripped from path) for the API request, the JSON output, AND the success message — all three agree.
- **NEW INVARIANT (NEW-INV-59):** Atlas's `/remotelink` endpoint accepts ANY string as `object.url` (it doesn't validate). Validation is a CLI-boundary contract, not an API contract. `jr` is stricter than Atlas.
- **Title default**: if `--title` is missing, defaults to URL string (line 268-271).

### 3.7 T-CLIENT: `api/client.rs` (Round 2 target #6, 490 LOC)

#### E-CLI-01 — `JiraClient` 7-field struct (lines 17-28)
- **Fields**: `client: reqwest::Client`, `base_url: String`, `instance_url: String`, `auth_header: String`, `verbose: bool`, `assets_base_url: Option<String>`, `profile_name: String`. **`profile_name` is plumbed through specifically so per-profile cache calls work without `&Config` access** (line 24-26 doc).

#### E-CLI-02 — `from_config` 4-stage build (lines 33-107)
- **Stage 1 — env-override detection** (line 37): `JR_BASE_URL` presence sets `test_override` flag.
- **Stage 2 — profile resolution**: in test-override mode, NO profile is consulted; in real mode, `config.active_profile_or_err()?`. **NEW INVARIANT (NEW-INV-60):** `JR_BASE_URL` short-circuits ALL profile-related URL resolution — including the OAuth `cloud_id` host rewrite that Round 1 §3.5 E-07-04 noted.
- **Stage 3 — auth header**: `JR_AUTH_HEADER` env (lines 65-67) **completely bypasses keychain**. Otherwise: `auth_method == "oauth"` → `Bearer <access>` (loads via `load_oauth_tokens`); else → `Basic <base64(email:token)>`. **NEW INVARIANT (NEW-INV-61):** `JR_AUTH_HEADER` is the test-injection seam for auth that Round 1 carryover #6 incorrectly framed as `JR_VERBOSE`.
- **Stage 4 — assets base URL**: in test mode `<JR_BASE_URL>/jsm/assets`; in OAuth mode `https://api.atlassian.com/ex/jira/<cloud_id>/jsm/assets`. **NEW INVARIANT (NEW-INV-62):** Assets API has its OWN base URL distinct from the main Jira base — the `assets_base_url` field is `Option<String>` because non-OAuth profiles without `cloud_id` cannot reach Assets (returns `JrError::ConfigError("Cloud ID not configured")` at lines 391-395, 414-418).

#### E-CLI-03 — `send` vs `send_raw` divergence
- **`send`** (lines 184-253):
  - On `try_clone()` failure → **panics** via `.expect("request should be cloneable (JSON body)")`. Justification: the codebase only sends JSON bodies, which are cloneable.
  - **Parses 4xx/5xx into `JrError`** via `parse_error` — callers receive typed errors.
  - 429 retry up to `MAX_RETRIES = 3` with `Retry-After` parsing.
  - Final fallback: `unreachable!("retry loop should always return or set last_response")` (line 252).
- **`send_raw`** (lines 265-320):
  - On `try_clone()` failure → **returns Err** (graceful) via `anyhow::anyhow!("request cannot be retried...")`. Reason: `jr api` (the only caller) may be sending streaming bodies the user constructed — non-cloneable is a real possibility.
  - **Does NOT parse 4xx/5xx** — caller (in `cli/api.rs::handle_api`) inspects `response.status()` themselves.
  - Same 429 retry. Final fallback: `unreachable!("loop iterates 0..=MAX_RETRIES; final iteration returns")` (line 319).
- **NEW INVARIANT (NEW-INV-63):** The two unreachable!() calls have DIFFERENT messages — distinguishable by panic output. The divergent error-handling between `send` (parses) and `send_raw` (passes through) is the architectural gate between "structured API access" and "raw passthrough".

#### E-CLI-04 — `extract_error_message` 6-level precedence chain (lines 448-490)
- **Order** (verbatim from doc-comment at lines 440-447):
  1. **Empty body** → `"<empty response body>"`.
  2. **Non-empty `errorMessages` array** (Atlas's standard array shape) → joined with `"; "`.
  3. **Non-empty `errors` object** (field-level validation) → `"<field>: <msg>"` pairs, **SORTED alphabetically** (line 477) before joining.
  4. **`message` string field** (some endpoints).
  5. **`errorMessage` string field** (singular — seen in some JSM endpoints, line 484-486).
  6. **Raw body as string** (fallback when not valid JSON).
- **NEW INVARIANT (NEW-INV-64) — sort-determinism on `errors` object**: The keys are sorted alphabetically (line 477 `pairs.sort()`) so the rendered error message is stable across HashMap iteration order. Critical for snapshot tests.
- **UTF-8 fallback** (lines 453-456): if body isn't UTF-8, uses `String::from_utf8_lossy` (lossy decode). Never panics on binary garbage.

#### E-CLI-05 — Five HTTP method wrappers + 2 instance-bypass + 2 assets-route methods (lines 138-181, 360-428)
- **Standard**: `get`, `post`, `put`, `post_no_content` (returns `()` on 204), `delete`. All use `base_url`.
- **Instance bypass** (lines 360-380): `get_from_instance`, `post_to_instance` — use `instance_url` directly. **Used by `api/jira/teams.rs::get_org_metadata` and `list_teams`** (per E-API-04).
- **Assets routing** (lines 386-428): `get_assets`, `post_assets` — construct URL `{assets_base_url}/workspace/{workspace_id}/v1/{path}`. Both error if `assets_base_url` is None.
- **Generic `request(method, path)`** (lines 431-436): builds a `RequestBuilder` with auth header pre-applied. Used by `cli/api.rs::handle_api` to add custom headers and body before passing to `send_raw`.

### 3.8 T-API-CLI: `cli/api.rs` (Round 2 target #12, 342 LOC)

#### E-API-CLI-01 — `HttpMethod` 5-variant enum (lines 15-34)
- **Variants**: `Get, Post, Put, Patch, Delete`. **NOTE**: HEAD and OPTIONS are NOT exposed — `jr api` is a Jira-API-shaped passthrough, and Atlas REST doesn't typically use HEAD/OPTIONS.
- `From<HttpMethod> for reqwest::Method` impl is exhaustive `match`.

#### E-API-CLI-02 — `normalize_path` 3-rule contract (lines 40-57)
- **Rejection cases**: empty/whitespace, `http://` or `https://` prefix (case-insensitive — `HTTPS://`, `Http://` all rejected).
- **Normalization**: prepends `/` if missing. **NEW INVARIANT (NEW-INV-65):** users can type `rest/api/3/myself` (no leading slash) and `jr api` accepts it. Pre-API contract — the user's intent is "API path" not "URL".

#### E-API-CLI-03 — `parse_header` security fence (lines 61-88)
- **Splits on FIRST colon** (`split_once(':')`). Values can contain colons (`X-Request-Id: abc:def:ghi` — the value is `abc:def:ghi`).
- **NEW INVARIANT (NEW-INV-66) — `Authorization` is forbidden at the parse layer**: case-insensitive match → `JrError::UserError("Cannot override the Authorization header — auth is managed by jr")`. Even attempting to override fails before the request is built. Pinned by 3 tests covering different cases.
- **CRLF injection rejection** (lines 84-86 + test at 286-291): `HeaderValue::from_str` rejects control chars including `\r\n`. Pinned by `test_parse_header_rejects_crlf_injection`.
- **`HeaderName::from_bytes`** is the validator for the key — accepts only HTTP/1.1-conformant tokens.

#### E-API-CLI-04 — `resolve_body` 4-source contract (lines 92-117)
- **Order**:
  1. `None` → `None`.
  2. `Some("@-")` → read from stdin (parameter, for testability).
  3. `Some("@filename")` → read from file.
  4. `Some(inline)` → use string verbatim.
- **JSON validation**: `serde_json::from_str::<Value>(&body)` runs after read — invalid JSON errors with `JrError::UserError("Request body is not valid JSON: <err>")`. **NEW INVARIANT (NEW-INV-67):** ALL request bodies are validated as JSON before sending. The `--data 'not json'` case fails at parse, NOT at the network. Atlas-only endpoint contract: Jira REST is JSON-only, so validation matches the contract.

#### E-API-CLI-05 — `handle_api` build-then-send sequencing (lines 119-176)
- **Order**:
  1. Normalize path.
  2. Resolve body from real stdin.
  3. Parse headers (collect failures).
  4. Build request via `client.request(method, path).build()?` (auth header pre-applied).
  5. Insert body via `body_mut() = Some(...)` AND `Content-Type: application/json`.
  6. Insert custom headers via `headers_mut().insert(...)` — uses **REPLACE semantics** (line 140-142 doc) so user's `-H 'Content-Type: text/plain'` overrides the default.
  7. Send via `send_raw` (no error parsing).
  8. Print response body to stdout (raw bytes — NO trailing newline added, mirrors `gh api`).
  9. Map status: 2xx → OK; 401 → NotAuthenticated; else → ApiError with extracted message.
- **NEW INVARIANT (NEW-INV-68):** `jr api` matches `gh api` exit semantics — status 2xx exits 0, 401 exits 2, other 4xx/5xx exits 1 (via JrError mappings). Body is always printed (even on error) so `jr api ... 2>/dev/null` returns the body for grep'ability.

### 3.9 T-CREATE: `cli/issue/create.rs` (Round 2 target #5, 375 LOC)

#### E-CR-01 — `handle_create` 7-stage field-build pipeline
1. Resolve project key (flag → `.jr.toml` → prompt → error).
2. Resolve issue type (flag → prompt → error).
3. Resolve summary (flag → prompt → error).
4. Resolve description (`--description-stdin` via spawn_blocking, OR `--description`, OR omitted).
5. Build base `fields` JSON: `{project: {key}, issuetype: {name}, summary}`.
6. Merge optional fields: description (markdown→ADF or text→ADF), priority, labels, team (via `resolve_team_field`), points (via `resolve_story_points_field_id`), parent, assignee.
7. `client.create_issue(fields)` → `CreateIssueResponse {key}`.
- **NEW INVARIANT (NEW-INV-69) — `--account-id` short-circuits user-name resolution** (lines 136-143): if `--account-id <id>` is provided, the assignee field is set directly with no API call. If only `--to <name>` → `resolve_assignee_by_project` (project-scoped — NEW-INV-46). If neither → no assignee field set.

#### E-CR-02 — `handle_create` post-create JSON enrichment (lines 153-198)
- **Pattern**: After `create_issue` succeeds, do a follow-up `get_issue` with `compose_extra_fields` (story points + cmdb + team) to return a payload SHAPED LIKE `issue view --output json`. **Plus** a `url` field with the browse URL.
- **Failure mode**: if the follow-up GET fails, prints stderr warning AND emits a fallback JSON `{key, url, fetch_error}` — script consumers can detect the partial-state via `.fetch_error` field. **NEW INVARIANT (NEW-INV-70):** Create never fails post-create — issue exists in Jira; only the JSON output may degrade. The `fetch_error` key is the discriminator.

#### E-CR-03 — `handle_edit` label add/remove syntax (lines 293-331)
- **Label format**:
  - `--label add:foo` → `{"add": "foo"}` in update array.
  - `--label remove:foo` → `{"remove": "foo"}` in update array.
  - `--label foo` (bare) → treated as `{"add": "foo"}` (lines 301-304).
- **Two-shape request**: when ANY label has `add:`/`remove:` prefix (i.e., labels.is_empty() is false), uses `PUT /rest/api/3/issue/<key>` with body `{fields: {...}, update: {labels: [...]}}`. Otherwise, uses standard `client.edit_issue(key, fields)`.
- **NEW INVARIANT (NEW-INV-71):** A bare `--label foo` is equivalent to `--label add:foo`. The `remove:` prefix has no `del:` synonym. Reads the field with `add:`/`remove:` semantics regardless of the issue's current labels — Jira-side adds/removes idempotently.
- **Empty labels array short-circuit** (line 306): only writes `update.labels` when at least one entry is present.

#### E-CR-04 — `handle_edit` no-updates rejection (lines 333-337)
- If the user invoked `jr issue edit FOO-1` with NO field flags AND no labels, **errors** with the full enumeration of acceptable flags. **NEW INVARIANT (NEW-INV-72):** edit cannot be a no-op — every invocation must specify at least one mutation. Distinct from `move`/`assign`/`unlink` which can be idempotent no-ops.

#### E-CR-05 — `--no-points` flag mechanics (lines 282-286)
- **Behavior**: `--no-points` sets the story-points custom field to `null` (`json!(null)`). **Distinct from** absence of `--points` (which omits the field from the update).
- **NEW INVARIANT (NEW-INV-73):** The CLI distinguishes "don't touch points" (no flag) from "clear points" (`--no-points`). `--points 0` is **different again** — sets to numeric 0. Three states.

### 3.10 T-JSON-OUTPUT: `cli/issue/json_output.rs` (149 LOC) — NEW round 2

#### E-JO-01 — 8 response builders (lines 4-82)
- **Functions** (each returning `serde_json::Value`):
  1. `move_response(key, status, changed: bool)` → `{key, status, changed}`.
  2. `assign_changed_response(key, display_name, account_id)` → `{key, assignee, assignee_account_id, changed: true}`.
  3. `assign_unchanged_response(...)` → same but `changed: false`.
  4. `unassign_response(key, changed)` → `{key, assignee: null, changed}`.
  5. `edit_response(key)` → `{key, updated: true}`.
  6. `link_response(key1, key2, link_type)` → `{key1, key2, type, linked: true}`.
  7. `unlink_response(unlinked: bool, count: usize)` → `{unlinked, count}`.
  8. `remote_link_response(key, id, url, title, self_url)` → `{key, id, url, title, self}`.
- **NEW INVARIANT (NEW-INV-74):** All 8 are **insta-snapshot-pinned** (11 unit tests at lines 88-148, each `assert_json_snapshot!`). Any field rename or reordering breaks snapshots. **The snapshots are the contract** — JSON consumers (scripts, AI agents) depend on these exact shapes.

#### E-JO-02 — Distinct field-name conventions
- `key` for issue key (consistent across all builders).
- `assignee_account_id` (snake_case) NOT `accountId` (camelCase) — **diverges from Jira API**; canonical form is the snake-case CLI convention.
- `changed: bool` is the universal "did this command modify state" flag (move, assign).
- `unlinked: bool` + `count: usize` pair instead of `changed` for unlink — caller cares about how many links were removed.
- **NEW INVARIANT (NEW-INV-75):** The CLI's JSON output uses snake_case throughout, distinguishing it from Atlas's camelCase. Any consumer can rely on `assignee_account_id` not being a typo.

### 3.11 T-CMD-CATALOG: Per-command operations (Round 2 target #9 — sprint, board, queue, worklog, team, user, project)

#### E-CMD-01 — `cli/sprint.rs` (438 LOC)
- **5 sub-handlers**: `handle_list`, `handle_current`, `handle_add`, `handle_remove` (`Current` is the active-sprint shortcut). `compute_sprint_summary` is pub-helper.
- **`MAX_SPRINT_ISSUES = 50`** (line 107) — enforced at handler entry for both add/remove. Atlas's documented per-call cap.
- **`resolve_scrum_board` scrum-only enforcement** (lines 67-88): calls `resolve_board_id` then verifies `board_config.board_type.to_lowercase() == "scrum"`. Error message includes the actual board type (e.g., `"Board 23 is a kanban board."`). **NEW INVARIANT (NEW-INV-76):** Sprint commands explicitly reject kanban boards — the error message names the wrong type rather than a generic "scrum required."
- **`compute_sprint_summary` 3-tuple return** (lines 187-214): `(total_points, completed_points, unestimated_count)`. "Done" is determined by `status_category.key == "done"` (lowercase). **NEW INVARIANT (NEW-INV-77):** Story-point completion is gated on category KEY (`"done"`), NOT category NAME (which can vary by language). Pinned at lines 200-202.
- **`handle_current` team-display gating** (lines 287-317): mirrors `cli/issue/list.rs` and `cli/board.rs::handle_view` (per #246 parity). Team column shown only when (Table mode) AND (team_field_id configured) AND (≥1 issue has populated team).

#### E-CMD-02 — `cli/board.rs` (334 LOC)
- **`resolve_board_id` 3-stage** (lines 15-90): CLI override > config > auto-discover via `list_boards(project_key, type_filter)`.
- **`build_kanban_jql` minimal-clause builder** (lines 162-171): `project = "<key>" AND statusCategory != Done ORDER BY rank ASC`. **NEW INVARIANT (NEW-INV-78):** Kanban view's JQL is **always** rank-ordered. Different from `cli/issue/list.rs` which defaults to `updated DESC`. Justification: kanban users expect lane-order display.
- **`handle_view` "Showing N of ~M" approximation** (lines 274-302): for kanban only, calls `approximate_count` for the total-with-approximation hint. Scrum boards don't get approximation (Agile API has no analog). **NEW INVARIANT (NEW-INV-79):** Approximate-count is a kanban-only feature — scrum users see "Showing N results" without total.

#### E-CMD-03 — `cli/queue.rs` (323 LOC)
- **`build_key_in_jql` cap-bypass** (lines 113-117): builds JQL `key IN (KEY1, KEY2, ...)` from queue keys. **No quoting** — issue keys are JQL identifiers, not strings (per source comment). Used to batch-fetch queue issues via the standard search API after first fetching keys via the JSM queue endpoint. Two-step pattern.
- **`reorder_by_queue_position`** (lines 119-137): `HashMap<&str, usize>` lookup; sorts results by queue position. Issues missing from the search result (e.g., permission-denied) are silently dropped (sort places them at `usize::MAX` and they fall off the truncate). **NEW INVARIANT (NEW-INV-80):** A queue's display order is preserved through the search round-trip; permission-denied issues silently disappear without error. **Architectural significance:** this is the pattern for "JSM tells me about issues I'm not allowed to view" — the queue endpoint reveals the keys but the issue endpoint redacts them.
- **`DEFAULT_LIMIT` applied at queue level** (line 84): `effective_limit = limit.or(Some(DEFAULT_LIMIT))` — note `or`, not `unwrap_or` — preserves the type for downstream use.

#### E-CMD-04 — `cli/worklog.rs` (79 LOC)
- **`handle_add` constants** (line 32): `parse_duration(dur, 8, 5)` — **hardcoded** `hours_per_day = 8, days_per_week = 5`. **NEW INVARIANT (NEW-INV-81):** Despite `duration::parse_duration` accepting configurable HPD/DPW (Round 1 §2a.4), the worklog handler ALWAYS uses 8/5. Jira instance settings (which can configure 7-hour days, 4-day weeks) are NOT honored. **POTENTIAL BUG** — if a tenant's Jira settings deviate, `1d` interpreted as 8 hours could differ from the server's reckoning. Round 1 noted that "Jira instance settings drive the live values" — but the live values are NOT used at the call site.
- **`handle_list` non-pagination amplifier**: combined with NEW-INV-29 (list_worklogs not paginated), the visible worklog list is bounded by Atlas's default page size (probably 100). Issues with >100 worklogs lose entries silently in display.

#### E-CMD-05 — `cli/team.rs` (120 LOC)
- **`fetch_and_cache_teams` lazy-org-discovery** (lines 54-72): if `active.org_id` is None, calls `get_org_metadata` to discover BOTH `cloud_id` and `org_id`, persists both to config. **NEW INVARIANT (NEW-INV-82):** First call to `jr team list` may write `cloud_id` and `org_id` to config — the GraphQL `tenantContexts` query is the discovery oracle. Subsequent runs read from config.
- **`resolve_org_id` reload pattern** (lines 99-117): after persisting via mutate, reloads via `Config::load_with(Some(&active_profile_name))` — preserving `--profile` flag through the write. **NEW INVARIANT (NEW-INV-83):** Avoiding the active-profile-name drift from a save-and-rehydrate pattern.

#### E-CMD-06 — `cli/user.rs` (165 LOC)
- **`handle_search` --all mode**: uses `search_users_all` (15-page safety cap from NEW-INV-19) when `--all`; `search_users` (single page) otherwise. Then truncates to `effective_limit` (with `DEFAULT_LIMIT = 30`).
- **`handle_view` 404/400 unification** (lines 75-87): downcast to `JrError::ApiError {status: 404 | 400}` → unified `UserError("User with accountId '<id>' not found")`. Per NEW-INV-21, Jira returns inconsistently between 404 and 400.
- **`format_active` ANSI rendering** (lines 122-127): `Some(true)` → `"✓".green()`, `Some(false)` → `"✗".red()`, `None` → `"—"`. Uses `colored::Colorize` — the global `--no-color` flag (from `cli/mod.rs`) toggles ANSI emission via `colored`'s env var override (`NO_COLOR`).

#### E-CMD-07 — `cli/project.rs` (133 LOC) — covered briefly
- The 2 sub-commands `list` and `fields` are documented in broad pass §2b.1; this round confirmed no LOC-citable additional finding beyond Round 1's broad pass. **NITPICK candidate** for Round 3.

### 3.12 T-INIT: `cli/init.rs` (Round 2 target #10, 285 LOC)

#### E-INIT-01 — 7-step state machine with per-step failure recovery
- **Step 1**: Collision-check existing config (lines 26-48). 3-way discrimination: `Ok(c)` → existing; `JrError::UserError` → recoverable env-var issue (don't delete config); other Err → "fix or remove the file" advice. **NEW INVARIANT (NEW-INV-84):** `init` never overwrites a malformed config — refuses with explicit advice.
- **Step 2**: Multi-profile awareness (lines 49-85). If profiles already exist, prompts "Add another profile?". If `add == false`, **early return Ok(())** — no further action. Profile-name collision validation in a `loop {}`.
- **Step 3**: URL prompt + auth method selector (oauth vs api_token).
- **Step 4**: Save profile entry with URL + auth_method to disk (`save_global` after lenient load).
- **Step 5**: Delegate to `cli::auth::login_oauth` or `cli::auth::login_token` for credential capture.
- **Step 6**: Optional `.jr.toml` generation per-project.
- **Step 7a/b/c**: Discover team field, story points field, org/cloud metadata. Each step is **independently failable** — failure at one step does NOT block subsequent steps. Each writes to config separately.

#### E-INIT-02 — Per-step failure recovery characteristics
- **Step 7a (`find_team_field_id`)**: `if let Ok(Some(team_id))` — silently skips on Err or None.
- **Step 7b (`find_story_points_field_id`)**: dispatches on `matches.len()`:
  - 0 → "skipping. You can set story_points_field_id manually" (warns + continues).
  - 1 → auto-uses; emits "Found story points field: <name> (<id>)".
  - 2+ → interactive Select prompt.
  - Err → eprint + skip.
- **Step 7c (`get_org_metadata` / `list_teams`)**: `if let Ok(metadata)` — silently skips. Inside, `list_teams` fetched in `if let Ok(api_teams)` — silent skip.
- **NEW INVARIANT (NEW-INV-85):** init's prefetch steps are best-effort warmers — failure of any one does NOT fail the overall `jr init` flow. The user can re-run any specific discovery later. **Significant for spec**: a stricter "init must completely succeed" interpretation would be a behavioral regression.

#### E-INIT-03 — `JR_PROFILE_OVERRIDE` retired (per source comment lines 13-16)
- The earlier `JR_PROFILE_OVERRIDE` env-var seam was removed because `unsafe { set_var }` under `#[tokio::main]` is unsound (POSIX `setenv` is not thread-safe; tokio worker threads exist before async-main body runs). The replacement is `new_profile_override: Option<String>` threaded through every `Config::load_*_with` call. **Confirms Round 1 §3.5 `Config::load_with` threading rationale.**

### 3.13 T-PARTIAL: `partial_match.rs` property tests (Round 2 target #13)

#### E-PM-01 — Property tests (4 total at lines 153-198)
1. `exact_match_always_found` — for any of 4 fixed candidates, the exact name returns `Exact`.
2. `never_panics_on_arbitrary_input` — `\\PC{0,50}` (any non-control 0-50 chars) never panics. **The robustness invariant.**
3. `empty_candidates_always_returns_none` — for any non-empty `[a-z]{1,10}` query against empty candidates, returns `None([])`.
4. `duplicate_candidates_yield_exact_multiple` — duplicating any one of 4 candidates and querying for it always returns `ExactMultiple`.
- **NEW INVARIANT (NEW-INV-86) — proptest never-panics**: even on Unicode garbage, `partial_match` is total. Pinned by the `\\PC{0,50}` regex generator.

#### E-PM-02 — `ExactMultiple` preserves first-match casing (line 119-129)
- For candidates `["John Smith", "john smith"]`, query `"john smith"` → `ExactMultiple("John Smith")` — **NOT** `ExactMultiple("john smith")`. The function returns the FIRST matching candidate's casing. **NEW INVARIANT (NEW-INV-87):** Order-of-input determines casing of the returned name. Callers relying on a specific case must filter the candidate list themselves.

#### E-PM-03 — `ExactMultiple-as-Exact` convention at use-sites (carryover #13 from Round 1 §9)
- **Verified at**: `cli/issue/links.rs:65-66, 136-137` (link-types: "treat like Exact if duplicates ever occur"); `cli/issue/workflow.rs:274-285` (move: same dedup pattern); `cli/queue.rs:148-152` (queue resolve). **NEW INVARIANT (NEW-INV-88):** `ExactMultiple` is treated as `Exact` whenever the resolver believes its candidate set ought to be unique by Atlas contract (link types, queue names within a service desk, transitions). It is treated DISTINCTLY (with disambiguation) for resolutions, teams, and users — those CAN have legitimate duplicates. **A convention split, not a property of the enum.**

### 3.14 T-DUR: `duration.rs` cross-context contrast with `jql::validate_duration` (Round 2 target #14)

#### E-DUR-01 — Two duration parsers, two grammars
- **`duration::parse_duration`** (`duration.rs:5-49`):
  - Lowercased input; combined units allowed (`1w2d3h30m` → 1 week + 2 days + 3 hours + 30 minutes).
  - Units: only `w/d/h/m`. `y/M` REJECTED.
  - Configurable `hours_per_day` and `days_per_week` (defaults 8/5 in the CLI but the type signature requires they be passed).
  - **Domain**: worklog time-entry quantities.
- **`jql::validate_duration`** (`jql.rs:16-33`):
  - Case-SENSITIVE; combined units rejected (`4w2d` → Err).
  - Units: `y/M/w/d/h/m` (6 units; `M` is months).
  - **Domain**: JQL relative-date queries (`created >= -2M`).
- **NEW INVARIANT (NEW-INV-89) — same syntax, different rules**: The string `"2M"` is valid JQL (2 months) but INVALID worklog (M is not a worklog unit). The string `"1w2d"` is valid worklog (combined) but INVALID JQL (combined rejected). Spec must distinguish the two grammars or run the wrong validator.

### 3.15 T-JQL: `jql.rs` full property test enumeration (Round 2 target #15)

#### E-JQL-01 — `jql.rs` 1 property test (lines 383-394)
- **`escaped_value_never_has_unescaped_quote`** with input `\\PC{0,100}` (any non-control 0-100 chars). The has_unescaped_quote helper counts trailing backslashes — odd count means escaped, even count means UN-escaped. **NEW INVARIANT (NEW-INV-90):** No matter what the user types in a JQL value, `escape_value` produces output where every `"` has an odd number of preceding backslashes (i.e., is escaped). Pinned by 100-char fuzzing.

#### E-JQL-02 — `proptest-regressions/jql.txt` corpus (single-entry)
- **One regression seed** (read from `proptest-regressions/jql.txt`):
  - `cc c696552d795390c45278b7f3fe08317d68e81494e898dd32ba3a2c97f5dc7df5 # shrinks to s = ""`
- **Translation**: a prior failure was found and minimized to the empty string `""`. The current implementation handles `escape_value("")` correctly (returns `""`). **NEW INVARIANT (NEW-INV-91):** The empty-string corpus entry pins that `escape_value` handles empty input without panic, which an `unwrap_or` or naive `String::insert` could regress.

#### E-JQL-03 — `build_asset_clause` parenthesized OR-join verified (Round 2 target #8)
- **Single-field shape** (`jql.rs:78`): `"<name>" IN aqlFunction("Key = \"<key>\"")` — NO outer parens.
- **Multi-field shape** (`jql.rs:80-81`): `("<name1>" IN aqlFunction("Key = \"<key>\"") OR "<name2>" IN aqlFunction("Key = \"<key>\""))` — wrapped in OUTER parens (single pair). Pinned by tests at lines 277-294.
- **Escape passes both field name AND key**: re-verified at lines 70-73 — `escape_value(name)` AND `escape_value(asset_key)` are applied separately. Pinned by `build_asset_clause_field_name_with_quotes` (lines 297-308) which uses field name `r#"My "Assets""#`.
- **NEW INVARIANT (NEW-INV-92) — single OR-clause-or-many parenthesization**: When there's exactly 1 CMDB field, output is unparenthesized. When there are ≥2, outer parens are added. **Architecturally significant**: a future caller wrapping the output in `... AND (asset_clause)` would over-parenthesize but Jira tolerates it; the current pattern is "let the caller decide outer scoping."

---

## 4. Sub-pass 2b deepening: behavioral — operations and state machines per target

### 4.1 `JiraClient` HTTP method selection state machine

```
Caller request flow:
    │
    ├─► get/post/put/post_no_content/delete (base_url)
    │     │
    │     ▼
    │   send (parses 4xx/5xx → JrError)
    │
    ├─► get_from_instance/post_to_instance (instance_url, OAuth-bypass)
    │     │   used by api/jira/teams.rs (GraphQL + Teams)
    │     ▼
    │   send (same pipeline)
    │
    ├─► get_assets/post_assets ({assets_base_url}/workspace/{ws}/v1/...)
    │     │   used by api/assets/* (Assets/CMDB)
    │     ▼
    │   send (same pipeline)
    │
    └─► request(method, path) → user-built reqwest::Request
          │   used by cli/api.rs::handle_api ONLY
          ▼
        send_raw (preserves all status codes, no error parsing)
```

### 4.2 Issue create JSON output state machine

```
handle_create
    ├── create_issue HTTP POST
    │      ↓
    │   CreateIssueResponse { key }
    │      │
    │      ├─ output = Table → eprint browse_url; print success line
    │      │
    │      └─ output = JSON
    │            │
    │            ├── compose_extra_fields(config, cmdb_fields)
    │            │     (cmdb_fields may be empty if discovery failed; degrades silently)
    │            ↓
    │            client.get_issue(key, &extra)
    │            │
    │            ├─ Ok(issue) → serialize + inject "url" key
    │            │
    │            └─ Err(e) → stderr warning + fallback JSON {key, url, fetch_error}
    ↓
    return Ok(()) 
```

### 4.3 Author needle classification decision tree (`cli/issue/changelog.rs`)

```
input: --author <raw>
    │
    ├─ raw.trim().is_empty() → ERROR (UserError, exit 64)
    │
    ├─ helpers::is_me_keyword(raw) → AccountId(client.get_myself().account_id)
    │     (HTTP call to /myself)
    │
    └─ AuthorNeedle::from_raw(raw)
          │
          ├─ trimmed.contains(':') → AccountId(trimmed)
          │
          ├─ trimmed.len() >= 12
          │   AND trimmed.chars().any(is_ascii_digit)
          │   AND trimmed.chars().all(is_ascii_alphanumeric || '-' || '_')
          │     → AccountId(trimmed)
          │
          └─ otherwise → NameSubstring(LoweredStr::new(trimmed))
                (will substring-match against display_name OR account_id at compare time)
```

### 4.4 `handle_move` decision tree (`cli/issue/workflow.rs`)

```
get_transitions(key)
    ├─ empty → bail "No transitions available"
    │
    ▼
get_issue(key)
    ├─ current_status known
    │
    ▼
target_status:
    ├─ Some(s) → use directly
    │
    └─ None
        ├─ no_input → bail "Target status required"
        └─ interactive → list available + prompt

idempotency check (case-insensitive):
    ├─ current == target → print success, return Ok (no HTTP)
    │
    ├─ any transition has name == target AND target.to.name == current
    │     → same as above (no HTTP)
    │
    ▼
candidate resolution:
    ├─ target_status.parse::<usize>() in 1..=transitions.len() → indexed transition
    │
    ▼ (build candidate_pool: dedup transition names + status names)
    └─ partial_match dispatch:
        ├─ Exact / ExactMultiple → use
        ├─ Ambiguous → no_input: error / interactive: prompt
        └─ None → bail with full transition list

resolution resolution:
    ├─ None → no resolution body
    └─ Some(query) → load_resolutions (cached) → resolve_resolution_by_name (4-branch)

transition_issue(key, id, fields)
    ├─ Err(msg containing "resolution" AND "required")
    │     → transform to actionable hint pointing at --resolution
    │
    ├─ Err(other) → propagate
    │
    └─ Ok → success
```

### 4.5 `auth refresh` 6-step destructive-but-safe flow

```
handle_refresh
    │
    ├─ Config::load_with(--profile)  (strict)
    │
    ├─ validate_profile_name(target)
    │
    ├─ chosen_flow_for_profile(target_profile, --oauth)
    │   (NOT chosen_flow — pinned distinct from active)
    │
    ├─ Pre-flight URL check:
    │     api_token + no_url → bail "use jr auth login --profile X --url ..."
    │
    ├─ Asymmetric clear:
    │     ├─ OAuth → clear_profile_creds(target)
    │     │           (deletes <target>:oauth-* keys; if target=default, also legacy flat keys)
    │     │
    │     └─ Token → clear_all_credentials(&[target])
    │                 (deletes shared email/api-token + all per-profile OAuth tokens)
    │
    ▼
Re-run login flow (token or oauth, identical to fresh login):
    ├─ Ok → emit RefreshArgs::output JSON {status: "refreshed", ...}
    │
    └─ Err → stderr "Credentials were cleared, but the login flow did not complete. Run jr auth login..."
```

(Round 1 §4.1 had this characterization; this round adds the precise `chosen_flow_for_profile` distinction and the 5 destructive sub-states.)

---

## 5. Newly-discovered entities & invariants (NOT in broad Pass 2 or Round 1)

### Entities (E-XX-NN, 39 new this round)

#### Per-resource `api/jira/*` modules (E-API-NN)
- **E-API-01** — `api/jira/issues.rs` 10 methods + `BASE_ISSUE_FIELDS` 16-element array + `SearchResult` + `ApproximateCountResponse`
- **E-API-02** — `api/jira/users.rs` `USER_PAGE_SIZE = 100` + `USER_PAGINATION_SAFETY_CAP = 15` + 6 methods + tolerant deserialization
- **E-API-03** — `api/jira/fields.rs` `Field` + `FieldSchema` + `KNOWN_SP_SCHEMA_TYPES` + `CMDB_SCHEMA_TYPE`
- **E-API-04** — `api/jira/teams.rs` 2 methods + GraphQL request shape + instance-bypass
- **E-API-05** — `api/jira/projects.rs` 5 methods + `IssueTypeMetadata`/`PriorityMetadata`/`StatusMetadata`/`IssueTypeWithStatuses` + `project_exists` 404→Ok(false)
- **E-API-06** — `api/jira/sprints.rs` 4 methods + `SprintIssuesResult` (different default fields than `SearchResult`)
- **E-API-07** — `api/jira/boards.rs` 2 methods
- **E-API-08** — `api/jira/links.rs` 4 methods + outward/inward semantics
- **E-API-09** — `api/jira/worklogs.rs` 2 methods (NOT paginated)
- **E-API-10** — `api/jira/resolutions.rs` 1 method (not paginated)
- **E-API-11** — `api/jira/statuses.rs` private `StatusEntry` + global statuses returned as `Vec<String>` (NOT `Vec<Status>`)

#### Pagination (E-PAG-NN)
- **E-PAG-01** — `OffsetPage<T>` 7 fields, 4 item-list keys, accessor preference order
- **E-PAG-02** — `CursorPage<T>` 2 fields, no `total`
- **E-PAG-03** — `ServiceDeskPage<T>` distinct field names (`start`/`limit`/`size` vs `start_at`/`max_results`/`total`)
- **E-PAG-04** — `AssetsPage<T>` capped `total ≤ 1000`
- **E-PAG-05** — `deserialize_bool_or_string` custom deserializer

#### Changelog (E-CL-NN)
- **E-CL-01** — `LoweredStr` smart constructor (compile-time invariant)
- **E-CL-02** — `AuthorNeedle` 2-variant enum + `from_raw` heuristic
- **E-CL-03** — Empty-needle filter-bypass guards (--author + --field)
- **E-CL-04** — Sort with parse-failure tiebreak
- **E-CL-05** — `truncate_to_rows` mid-entry truncation
- **E-CL-06** — `from_to_display` empty-string-as-absent rule
- **E-CL-07** — `format_date` once-per-process verbose log

#### Helpers (E-HE-NN)
- **E-HE-01** — `is_team_uuid` byte-loop validator
- **E-HE-02** — `resolve_team_field` 6-step state machine
- **E-HE-03** — `disambiguate_user` private generic
- **E-HE-04** — `resolve_user` returns JQL fragment (`currentUser()` or accountId)
- **E-HE-05** — `resolve_assignee` (issue-scoped) vs `resolve_assignee_by_project`
- **E-HE-06** — `resolve_asset` 4-branch resolver
- **E-HE-07** — Tailored error messages tracking `fetched_fresh`

#### Workflow (E-WF-NN)
- **E-WF-01** — `resolve_resolution_by_name` substring single-hit refusal + id-disambiguated duplicates
- **E-WF-02** — `load_resolutions` cache-and-defensive-drop
- **E-WF-03** — `handle_move` unified candidate pool (transitions + statuses)
- **E-WF-04** — `handle_move` resolution-required heuristic
- **E-WF-05** — `handle_assign` 3-state idempotency
- **E-WF-06** — `handle_comment` source-of-truth resolution + spawn_blocking
- **E-WF-07** — `handle_open` URL synthesis (uses `base_url` — POTENTIAL OAUTH BUG)

#### Links (E-LK-NN, NEW corpus)
- **E-LK-01** — Self-link prevention
- **E-LK-02** — Link-type resolver (ExactMultiple-as-Exact convention)
- **E-LK-03** — `handle_unlink` dual-direction matching + idempotent no-op
- **E-LK-04** — `handle_remote_link` 3-stage URL validation + scheme allowlist

#### Client (E-CLI-NN)
- **E-CLI-01** — `JiraClient` 7-field struct + `profile_name` plumbing
- **E-CLI-02** — `from_config` 4-stage build (JR_BASE_URL + JR_AUTH_HEADER + OAuth host rewrite)
- **E-CLI-03** — `send` vs `send_raw` divergence (panic-on-clone-fail vs Err-on-clone-fail)
- **E-CLI-04** — `extract_error_message` 6-level chain with sort-determinism on `errors`
- **E-CLI-05** — 5 standard methods + 2 instance-bypass + 2 assets-route + `request`

#### CLI api passthrough (E-API-CLI-NN)
- **E-API-CLI-01** — `HttpMethod` 5-variant (no HEAD/OPTIONS)
- **E-API-CLI-02** — `normalize_path` 3-rule contract
- **E-API-CLI-03** — `parse_header` security fence (Authorization forbidden, CRLF rejected)
- **E-API-CLI-04** — `resolve_body` 4-source contract (None/@-/@file/inline) + JSON validation
- **E-API-CLI-05** — `handle_api` build-then-send sequencing + replace-semantics for headers

#### Create / edit / json output (E-CR-NN, E-JO-NN)
- **E-CR-01** — `handle_create` 7-stage field-build pipeline + assignee 3-way
- **E-CR-02** — Post-create JSON enrichment with `fetch_error` discriminator
- **E-CR-03** — `handle_edit` label add:/remove: syntax + dual-shape request
- **E-CR-04** — `handle_edit` no-updates rejection
- **E-CR-05** — `--no-points` vs absence vs `--points 0` (3 distinct states)
- **E-JO-01** — 8 insta-snapshot-pinned response builders
- **E-JO-02** — snake_case CLI convention (`assignee_account_id` etc.)

#### CLI per-command (E-CMD-NN)
- **E-CMD-01** — `cli/sprint.rs` `MAX_SPRINT_ISSUES = 50` + scrum-only enforcement + category KEY-not-NAME
- **E-CMD-02** — `cli/board.rs` `build_kanban_jql` rank-ordered + kanban-only approximate-count
- **E-CMD-03** — `cli/queue.rs` `build_key_in_jql` no-quoting + `reorder_by_queue_position`
- **E-CMD-04** — `cli/worklog.rs` hardcoded 8/5 + non-paginated list
- **E-CMD-05** — `cli/team.rs` lazy-org-discovery + reload-after-mutate pattern
- **E-CMD-06** — `cli/user.rs` 404/400 unification + `--no-color` via `colored::Colorize`

#### Init (E-INIT-NN)
- **E-INIT-01** — 7-step state machine
- **E-INIT-02** — Per-step independently-failable best-effort warmers
- **E-INIT-03** — `JR_PROFILE_OVERRIDE` retired (rationale)

### Invariants (NEW-INV-18 .. NEW-INV-92, 75 new this round)

- **NEW-INV-18** — `get_changelog` anti-loop guard (JRACLOUD-94357-class) — `api/jira/issues.rs:218-230`
- **NEW-INV-19** — Fixed-window pagination semantics (advance by USER_PAGE_SIZE, not returned count) — `api/jira/users.rs:72-83`
- **NEW-INV-20** — Tolerant array-or-`{values}` deserialization in user search — `api/jira/users.rs:33-41 (and parallel sites)`
- **NEW-INV-21** — `get_user` 404/400 ambiguity — `api/jira/users.rs:230`
- **NEW-INV-22** — `find_team_field_id` requires custom field literally named "team" — `api/jira/fields.rs:26-32`
- **NEW-INV-23** — Story-points name allowlist (only 2 names) — `api/jira/fields.rs:51`
- **NEW-INV-24** — `get_org_metadata` and `list_teams` bypass OAuth API gateway — `api/jira/teams.rs`
- **NEW-INV-25** — Empty `tenantContexts` → "Could not resolve organization ID" — `api/jira/teams.rs:24-28`
- **NEW-INV-26** — `list_projects` non-pagination when limit set — `api/jira/projects.rs:113`
- **NEW-INV-27** — `get_sprint_issues` default fields ≠ `search_issues` default fields — `api/jira/sprints.rs:53`
- **NEW-INV-28** — `create_issue_link` (outward, inward) parameter semantics — `api/jira/links.rs:6-22`
- **NEW-INV-29** — `list_worklogs` is NOT paginated — `api/jira/worklogs.rs:25-30`
- **NEW-INV-30** — Global `get_all_statuses` returns `Vec<String>` (sorted, deduped, names only) — `api/jira/statuses.rs:14-20`
- **NEW-INV-31** — `OffsetPage` 4-Option item-list field design — `api/pagination.rs:8-32`
- **NEW-INV-32** — `CursorPage` has no server-side `total` (requires separate `approximate_count`) — `api/pagination.rs:64-73`
- **NEW-INV-33** — `ServiceDeskPage` distinct field names from `OffsetPage` — `api/pagination.rs:84-101`
- **NEW-INV-34** — Assets API caps `total` at 1000 — `api/pagination.rs:134`
- **NEW-INV-35** — `LoweredStr` enforces lowercase via private field — `cli/issue/changelog.rs:151-171`
- **NEW-INV-36** — AuthorNeedle ASCII-alphanumeric (NOT general alphanumeric) — `cli/issue/changelog.rs:200-214`
- **NEW-INV-37** — "User12345Name" intentional residual edge → AccountId — `cli/issue/changelog.rs:197-200`
- **NEW-INV-38** — 12-char boundary rule (≥ 12 with digit → AccountId) — `cli/issue/changelog.rs:203-208`
- **NEW-INV-39** — Empty-needle filter-bypass guards on --author and --field — `cli/issue/changelog.rs:50-79`
- **NEW-INV-40** — Sort with parse-failure tiebreak (raw lex compare) — `cli/issue/changelog.rs:87-93`
- **NEW-INV-41** — `truncate_to_rows` exact-row-count guarantee — `cli/issue/changelog.rs:286-304`
- **NEW-INV-42** — `from_to_display` empty-as-absent — `cli/issue/changelog.rs:308-319`
- **NEW-INV-43** — `is_team_uuid` UUID pass-through skips cache + name-match — `cli/issue/helpers.rs:60-65`
- **NEW-INV-44** — Bounded auto-refresh in `resolve_team_field` (single retry) — `cli/issue/helpers.rs:84-86`
- **NEW-INV-45** — All 3 user resolvers share `disambiguate_user` private generic
- **NEW-INV-46** — `resolve_user` returns JQL fragment vs `resolve_assignee` returns tuple — `cli/issue/helpers.rs:351, 391`
- **NEW-INV-47** — Project-scoped assignee resolver relies on server-side filter (no client-side `active` filter) — `cli/issue/helpers.rs:442-444`
- **NEW-INV-48** — `resolve_asset` AQL `Name like "..."` predicate — `cli/issue/helpers.rs:487`
- **NEW-INV-49** — Error message specificity correlates with `fetched_fresh` boolean — `cli/issue/helpers.rs:170-183`
- **NEW-INV-50** — Resolution duplicates listed with id `(id={})` (only resolver doing this) — `cli/issue/workflow.rs:47-65`
- **NEW-INV-51** — `load_resolutions` in-memory result superset of cached — `cli/issue/workflow.rs:117-136`
- **NEW-INV-52** — `handle_move` unified candidate pool (transition NAMES + status NAMES) — `cli/issue/workflow.rs:240-258`
- **NEW-INV-53** — Numeric input first-class disambiguation — `cli/issue/workflow.rs:227-235`
- **NEW-INV-54** — Resolution-required heuristic message-shape coupling — `cli/issue/workflow.rs:357-377`
- **NEW-INV-55** — `handle_assign` always costs 1 GET (idempotency check) + ≤1 PUT — `cli/issue/workflow.rs:472-562`
- **NEW-INV-56** — **POTENTIAL BUG: `handle_open` uses `base_url` — wrong for OAuth profiles** — `cli/issue/workflow.rs:636`. Round 3 + Pass 3 BC candidate.
- **NEW-INV-57** — Link types presumed unique by Atlas contract (`ExactMultiple` → first match) — `cli/issue/links.rs:65-66, 136-137`
- **NEW-INV-58** — `handle_unlink` count=0 idempotent no-op (NOT error) — `cli/issue/links.rs:192-205`
- **NEW-INV-59** — `handle_remote_link` URL validation stricter than Atlas — `cli/issue/links.rs:245-264`
- **NEW-INV-60** — `JR_BASE_URL` short-circuits ALL profile-related URL resolution including OAuth host rewrite — `api/client.rs:42-46, 86-96`
- **NEW-INV-61** — `JR_AUTH_HEADER` env var bypasses keychain (the actual test-injection seam, NOT JR_VERBOSE) — `api/client.rs:65-67`
- **NEW-INV-62** — Assets API has its own base URL (separate `assets_base_url` field) — `api/client.rs:23, 86-96`
- **NEW-INV-63** — `send` panics on `try_clone()` fail; `send_raw` returns Err — `api/client.rs:191-193, 267-272`
- **NEW-INV-64** — `extract_error_message` `errors` object pairs are alphabetically sorted — `api/client.rs:477`
- **NEW-INV-65** — `normalize_path` accepts both `/path` and `path` (auto-prepends `/`) — `cli/api.rs:52-56`
- **NEW-INV-66** — `parse_header` rejects Authorization (case-insensitive) — `cli/api.rs:75-80`
- **NEW-INV-67** — `resolve_body` validates JSON before sending (4xx-budget-saver) — `cli/api.rs:113-114`
- **NEW-INV-68** — `jr api` exit semantics (2xx→0, 401→2, else→1) — `cli/api.rs:164-175`
- **NEW-INV-69** — `--account-id` short-circuits user-name resolution in create — `cli/issue/create.rs:136-143`
- **NEW-INV-70** — Create never fails post-create; `fetch_error` discriminator — `cli/issue/create.rs:176-191`
- **NEW-INV-71** — Bare `--label foo` ≡ `--label add:foo` — `cli/issue/create.rs:301-304`
- **NEW-INV-72** — `handle_edit` no-op rejection (cannot edit with zero mutations) — `cli/issue/create.rs:333-337`
- **NEW-INV-73** — `--points` vs `--no-points` vs absence: 3 distinct states — `cli/issue/create.rs:276-286`
- **NEW-INV-74** — All 8 JSON output builders are insta-snapshot-pinned — `cli/issue/json_output.rs:88-148`
- **NEW-INV-75** — CLI JSON uses snake_case (`assignee_account_id`) — `cli/issue/json_output.rs:13-30`
- **NEW-INV-76** — Sprint commands explicitly reject kanban with named-type error — `cli/sprint.rs:79-86`
- **NEW-INV-77** — Sprint summary uses `category.key == "done"` (NOT category.name) — `cli/sprint.rs:200-202`
- **NEW-INV-78** — Kanban view always rank-ordered — `cli/board.rs:170`
- **NEW-INV-79** — Approximate-count is kanban-only — `cli/board.rs:280-294`
- **NEW-INV-80** — Queue ordering preserved through search; permission-denied issues silently dropped — `cli/queue.rs:121-137`
- **NEW-INV-81** — **POTENTIAL BUG: `cli/worklog.rs::handle_add` hardcodes 8/5 hours-per-day/days-per-week** — `cli/worklog.rs:32`. Jira instance settings ignored.
- **NEW-INV-82** — First `jr team list` discovers + persists `cloud_id` and `org_id` — `cli/team.rs:54-72, 99-117`
- **NEW-INV-83** — `resolve_org_id` reload-after-mutate preserves --profile flag — `cli/team.rs:103-104`
- **NEW-INV-84** — `init` never overwrites malformed config — `cli/init.rs:26-48`
- **NEW-INV-85** — init prefetch steps are best-effort (independently-failable) — `cli/init.rs:193-251`
- **NEW-INV-86** — `partial_match` proptest never-panics on `\\PC{0,50}` — `partial_match.rs:168-170`
- **NEW-INV-87** — `ExactMultiple` preserves first-match casing — `partial_match.rs:27-28`
- **NEW-INV-88** — `ExactMultiple-as-Exact` is a use-site CONVENTION (link types, transitions, queue names) — `cli/issue/links.rs, workflow.rs, queue.rs`
- **NEW-INV-89** — Two duration parsers, two grammars (`1w2d` worklog OK, JQL Err; `2M` JQL OK, worklog Err) — `duration.rs vs jql.rs`
- **NEW-INV-90** — JQL `escape_value` 100-char proptest never produces unescaped quote — `jql.rs:383-394`
- **NEW-INV-91** — `proptest-regressions/jql.txt` pins empty-string corpus
- **NEW-INV-92** — `build_asset_clause` parenthesization rule (≥2 fields → outer parens; 1 field → no parens) — `jql.rs:77-82`

### Patterns (NEW-PAT-NN)
- **NEW-PAT-01** — "Once-per-process verbose log" via static `AtomicBool` (3 sites: `IssueFields::team_id`, `format_date` for changelog, an unverified third site)
- **NEW-PAT-02** — `tokio::task::spawn_blocking` for stdin reads (3 sites: `handle_create`, `handle_edit`, `handle_comment`)

---

## 6. Retracted / corrected

- **CONV-ABS-1**: Round 1 §9 carryover #4 incorrectly attributed `link/unlink/link-types`/`remote-link` operations to `cli/issue/workflow.rs`. Those handlers actually live in `cli/issue/links.rs` (293 LOC). **No prior Round 1 substantive claim used the misattribution; this is logged so Round 3 doesn't propagate the error.** Round 2 catalogues `links.rs` separately as T-LINKS (E-LK-01..04).
- **Round 1 carryover #6 framing of "JR_VERBOSE stderr gate"** — corrected: NO `JR_VERBOSE` env var exists. The actual gate is the `verbose: bool` field on `JiraClient`, set from a CLI flag. The actual env-var test seam is `JR_AUTH_HEADER` (NEW-INV-61).
- **Round 1 §3.3 E-03-01 "6 endpoints in api/assets" header** — actually 5 files; objects.rs has 4 endpoints alone. Counting-convention slip in the header row. Re-verified the file count.

**No prior Round 1 substantive entity or invariant retracted.**

---

## 7. Delta Summary — what's new vs Round 1

| Category | Items added (delta) |
|---|---|
| `api/jira/*` per-resource | **+11 entities** (E-API-01..11) + **+13 invariants** (NEW-INV-18..30) |
| Pagination | **+5 entities** (E-PAG-01..05) + **+4 invariants** (NEW-INV-31..34) |
| Changelog | **+7 entities** (E-CL-01..07) + **+8 invariants** (NEW-INV-35..42) |
| Helpers | **+7 entities** (E-HE-01..07) + **+7 invariants** (NEW-INV-43..49) |
| Workflow | **+7 entities** (E-WF-01..07) + **+7 invariants** (NEW-INV-50..56) |
| Links | **+4 entities** (E-LK-01..04) + **+3 invariants** (NEW-INV-57..59) |
| Client | **+5 entities** (E-CLI-01..05) + **+5 invariants** (NEW-INV-60..64) |
| API passthrough | **+5 entities** (E-API-CLI-01..05) + **+4 invariants** (NEW-INV-65..68) |
| Create/edit/json | **+7 entities** (E-CR-01..05, E-JO-01..02) + **+7 invariants** (NEW-INV-69..75) |
| Sprint/Board/Queue/Worklog/Team/User/Init | **+9 entities** (E-CMD-01..06, E-INIT-01..03) + **+10 invariants** (NEW-INV-76..85) |
| Partial match / Duration / JQL | (no new entities for these targets — confirmed prior Round 1) + **+7 invariants** (NEW-INV-86..92) |
| Patterns | **+2 patterns** (NEW-PAT-01..02) |

**Quantitative delta (Round 2)**:
- New entities: **39** vs Round 1's 33 (+18% Round 2 vs Round 1)
- New invariants: **75** vs Round 1's 17 (+341% Round 2 vs Round 1)
- New patterns: **2**
- Refined existing entities: **1** (E-API-11 corrects Round 1's framing of `get_all_statuses`)
- Retracted findings: **0** (1 carryover-target-list correction logged as CONV-ABS-1)
- LOC recount discrepancies: **0** (all citations within ±1 LOC; one rounding artifact noted)
- Hallucination-class audit findings: **0 retractions**, **1 framing correction** (`JR_VERBOSE` → `verbose:bool` + `JR_AUTH_HEADER`)

**Cumulative (Pass 2 broad + R1 + R2)**:
- Total entities: 51 (broad) + 33 (R1) + 39 (R2) = **123**
- Total invariants: 25 (broad) + 17 (R1) + 75 (R2) = **117**

---

## 8. Novelty Assessment

**Novelty: SUBSTANTIVE**

Justification — would removing this round's findings change how you'd spec the system? **Yes**, in at least 6 model-changing ways:

1. **NEW-INV-19 (fixed-window pagination semantics in user search)** — the JRACLOUD-71293 advance-by-window-not-count rule is non-obvious and load-bearing. A spec rewrite that advanced by returned count would silently produce duplicates after permission filtering. This is a real-world data-correctness finding.

2. **NEW-INV-29 (`list_worklogs` not paginated) AND NEW-INV-81 (worklog hardcoded 8/5)** — TWO worklog-domain bugs/limitations the broad pass and Round 1 missed. The first silently truncates worklogs at Atlas's default page size; the second ignores Jira instance settings. Spec must call these out.

3. **NEW-INV-56 (`handle_open` uses `base_url`, wrong for OAuth)** — a third potential-bug finding. For OAuth profiles, `client.base_url()` returns the API gateway URL, not the user-facing instance URL. Browser-launching the OAuth gateway URL would fail. Pass 3 BC candidate.

4. **NEW-INV-30 (global `get_all_statuses` returns `Vec<String>`, not `Vec<Status>`)** — the asymmetry between project-scoped and global status endpoints is a domain-model boundary. A spec consolidating them would fail to compile.

5. **E-API-04 + NEW-INV-24 (instance-bypass for GraphQL teams)** — the `get_from_instance`/`post_to_instance` methods exist primarily for the `tenantContexts` GraphQL endpoint. A spec without this distinction would route GraphQL through the OAuth gateway, which fails.

6. **E-CL-01 + NEW-INV-35 (`LoweredStr` smart constructor)** — like `ResolvedRedirect` (R1 E-01-02), a private-field-encapsulated invariant that lifts "lowercase needle" from documentation to type system. A spec must preserve the encapsulation pattern.

These are model-changing findings, not refinements. The 75 new invariants this round (vs Round 1's 17) materially expand the spec surface. **SUBSTANTIVE.**

---

## 9. Remaining gaps / next candidate scope (verbatim for Round 3)

### High priority (still under-deepened)

1. **`adf.rs` deep round 2** (1,826 LOC) — Round 1's T-09 was MEDIUM-priority and sampled the node catalog. Round 2 deferred. Round 3 should:
   - Enumerate all 69+ inline unit tests (count via `grep -c '#\[test\]' src/adf.rs`).
   - Characterize table-render edge cases (empty tables, headerless tables, mixed-cell-count rows).
   - Characterize the `wrap_inlines_as_blocks` allowlist (which marks survive `wrap` and which don't).
   - Characterize ADF→text NEWLINE / WHITESPACE rules (does double-newline collapse? trailing newline preserved?).
   - Characterize `mention`/`emoji`/`inlineCard`/`media*` silent-drop semantics — Round 1 NEW-INV-14 noted these but didn't enumerate.

2. **OAuth state machine deep round 2** — Round 1 §3.7 and §4.1 covered the unused `refresh_oauth_token`. Round 3 should:
   - Catalog the FULL XOR-obfuscation pipeline at `build.rs` time.
   - Catalog the `redirect_uri_strategy_strings` test (line 928-937) plus EVERY OAuth-related test.
   - Confirm the `KEYRING_TEST_ENV_MUTEX` poisoned-lock recovery details.

3. **`cli/auth.rs` (1,998 LOC) deep round 2** — Round 1 §4.1 covered the 7 subcommands and §4.2 covered the profile lifecycle. Round 3 should:
   - Catalog `chosen_flow_for_profile` vs `chosen_flow` test pinning.
   - Catalog every JSON output shape (auth list, status, refresh, login).
   - Enumerate the 12 keyring round-trip tests (line numbers + scenarios).
   - Catalog the `prepare_login_target` validator + every error message.

4. **`cli/issue/list.rs` deep round 2 (1,083 LOC)** — Round 1 §3.2 covered query lifecycle. Round 3 should:
   - Enumerate `format_issue_row` + `format_issue_rows_public` exact column ordering.
   - Catalog `compose_extra_fields` interactions with story-points/CMDB/team.
   - Enumerate `handle_view` operation (broad pass listed `view` alongside `list` but not separated out).
   - Walk the unique `--no-color` interactions with `colored::Colorize` global.

5. **`cli/assets.rs` deep round 2 (1,055 LOC)** — Round 1 §3.3 covered the 6 endpoints + filter_tickets + key resolution. Round 3 should:
   - Enumerate ALL `cli/assets.rs` operations (search/view/tickets/schemas/types/schema).
   - Catalog the `--schema` partial-match semantics for schema-by-name.
   - Catalog the `attribute` table-render shape (display_value vs value coalesce).

### Medium priority

6. **`cache.rs` deep round 2 (899 LOC)** — Round 1 §3.4 covered the 6 categories. Round 3 should:
   - Enumerate `with_temp_cache` test scaffolding details.
   - Catalog every cache-write code path's failure mode.
   - Trace the cross-test interaction with `KEYRING_TEST_ENV_MUTEX` and `ENV_LOCK`.

7. **`config.rs` deep round 2 (1,223 LOC)** — Round 1 §3.5 covered profile resolution + migration. Round 3 should:
   - Catalog the migration's exact field-by-field copy logic.
   - Verify NEW-INV-12 (multi-profile fields bug) by tracing config.global.fields.* writes — is there ANY write site that mutates `config.global.fields` after migration drains it?
   - Catalog every `--strict` vs `--lenient` load call site.

8. **`api/auth.rs` (1,397 LOC) line-by-line** — Round 1 §3.1 catalogued entities but not every method:
   - Catalog `clear_profile_creds` vs `clear_all_credentials` exact semantics.
   - Catalog every `read_keyring_optional` error-discrimination branch.
   - Catalog the `KEYRING_TEST_ENV_MUTEX` poisoned-lock recovery details.

9. **`tests/common/fixtures.rs`** + **`tests/*.rs`** integration tests — neither broad pass nor R1/R2 catalogued. Pass 3 will need this for BC enumeration.

### Low priority

10. **`output.rs` (76 LOC)** — single small file; Round 1 carryover deferred. Round 3 candidate or NITPICK.
11. **`error.rs` (137 LOC)** — broad pass §2a.2 catalogued the 11 variants + exit codes. Round 3 should verify every JrError construction site to ensure exit code consistency.
12. **`build.rs`** + `embedded_oauth.rs` codegen — full XOR pipeline not yet line-by-line.

### Pass 4 deepening triggered (cross-pollination — DO NOT write into Pass 2)

13. **`handle_open` URL bug for OAuth (NEW-INV-56)** — should surface as a Pass 3 BC and as a Pass 4 reliability concern. Cross-pollination to Pass 3/Pass 4.
14. **`list_worklogs` non-pagination (NEW-INV-29)** — cross-pollination to Pass 4 (reliability — silent data loss).
15. **`handle_add` hardcoded 8/5 (NEW-INV-81)** — cross-pollination to Pass 4 (reliability + UX).
16. **`get_changelog` anti-loop guard (NEW-INV-18)** — cross-pollination to Pass 4 (defensive programming pattern).
17. **Pass 4 N+1 framing softening (Round 1 carryover #16)** — still pending. Round 2 confirms Round 1's E-02-04 dedup-and-concurrent characterization is correct. Pass 4 round 2 should incorporate.

---

## 10. State Checkpoint

```yaml
pass: 2
round: 2
status: complete
audit_findings_against_hallucination_classes: 1
new_entities: 39
new_invariants: 75
retracted_findings: 0
files_examined: 30
novelty: SUBSTANTIVE
timestamp: 2026-05-04T19:30:00Z
next_round_targets: |-
  1. adf.rs deep round 2 (1,826 LOC) — 69+ unit tests, table-render edge cases, wrap_inlines_as_blocks allowlist, ADF→text whitespace rules, silent-drop node catalog
  2. OAuth state machine deep round 2 — XOR pipeline at build.rs, every OAuth test
  3. cli/auth.rs deep round 2 (1,998 LOC) — chosen_flow tests, JSON output shapes, 12 keyring round-trip tests, prepare_login_target validator
  4. cli/issue/list.rs deep round 2 (1,083 LOC) — format_issue_row column ordering, compose_extra_fields interactions, handle_view operation, --no-color interaction
  5. cli/assets.rs deep round 2 (1,055 LOC) — schema partial-match, attribute display, all sub-commands
  6. cache.rs deep round 2 (899 LOC) — with_temp_cache scaffolding, write-path failures, cross-mutex interactions
  7. config.rs deep round 2 (1,223 LOC) — migration field-by-field, NEW-INV-12 verification, strict/lenient call sites
  8. api/auth.rs line-by-line (1,397 LOC) — clear_profile_creds vs clear_all_credentials, every read_keyring_optional branch
  9. tests/common/fixtures.rs + tests/*.rs integration tests
  10. output.rs (76 LOC) — NITPICK candidate
  11. error.rs JrError construction sites verification
  12. build.rs + embedded_oauth.rs XOR codegen
  13. (cross-pollination — Pass 3) handle_open URL bug for OAuth (NEW-INV-56)
  14. (cross-pollination — Pass 4) list_worklogs non-pagination (NEW-INV-29)
  15. (cross-pollination — Pass 4) handle_add hardcoded 8/5 (NEW-INV-81)
  16. (cross-pollination — Pass 4) Round 1 §9 #16 N+1 framing softening still pending
```
