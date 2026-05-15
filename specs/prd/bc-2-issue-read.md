---
context: bc-2
title: "Issue Read (list/view/comments/changelog)"
total_bcs: 93   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 51   # count of `#### BC-` headings in this file
last_updated: 2026-05-13
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/bc-02-issue-read.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.2
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §3.2
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.1
---

# BC-2 — Issue Read (list / view / comments / changelog)

92 behavioral contracts across 6 subdomains: JQL composition (2.1), Issue list
behavior (2.2), Issue view (2.3), Comments (2.4), Changelog (2.5), API layer (2.6).

---

## Subdomains

### 2.1 JQL Composition (the canonical build pipeline)

#### BC-2.1.001: `issue list` cursor-paginates via `POST /rest/api/3/search/jql`

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:7-31, 130-166`
**Subject**: Issue read
**Behavior**: `client.search_issues(jql, limit, fields)` posts to `/rest/api/3/search/jql`; returns `{issues: Vec<Issue>, has_more: bool}`. Pagination via `nextPageToken` cursor.
**Trace**: Pass 3 BC-101

---

#### BC-2.1.002: `--jql X` wraps in parens, strips ORDER BY, re-appends `ORDER BY updated DESC`

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:36-52`; `tests/all_flag_behavior.rs:54-66`; 26 unit tests
**Subject**: Issue read
**Behavior**: `build_jql_base_parts(jql, project_key)` calls `jql::strip_order_by(jql)`, wraps in parens. Order-by slot is ALWAYS `"updated DESC"` — user's `ORDER BY rank ASC` is silently replaced. `--jql "priority = Highest ORDER BY created DESC" --project PROJ` → `(project = "PROJ") AND (priority = Highest) ORDER BY updated DESC`.
**Edge cases**: user ORDER BY is stripped, never preserved.
**Trace**: Pass 3 BC-102, BC-125 (R1)

---

#### BC-2.1.003: Scrum board with active sprint → JQL `sprint = <id> ORDER BY rank ASC`

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:278-282`; `tests/all_flag_behavior.rs:347-352`
**Subject**: Issue read
**Behavior**: When no `--jql` AND board_id+scrum+active-sprint: `sprint = {sprint.id}` + order by `rank ASC`. Sprint ID from `client.list_sprints(bid, Some("active"))`.
**Trace**: Pass 3 BC-126 (R1)

---

#### BC-2.1.004: Kanban board → `project = "X" AND statusCategory != Done ORDER BY rank ASC`

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:302-310`; `tests/all_flag_behavior.rs:497-516, 542-562`
**Subject**: Issue read
**Behavior**: Body-match pins literal composed JQL. The `statusCategory != Done` is server-side (not `--open` flag).
**Trace**: Pass 3 BC-127 (R1)

---

#### BC-2.1.005: No board_id → `project = "X" ORDER BY updated DESC`

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:331-338`; `tests/all_flag_behavior.rs:42-86`
**Trace**: Pass 3 BC-128 (R1)

---

#### BC-2.1.006: No project AND no filters AND no `--jql` → exit 64 listing all 13 filter sources

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:344-351`
**Subject**: Issue read
**Behavior**: stderr contains literal `"No project or filters specified. Use --project, --assignee, --reporter, --status, --open, --team, --recent, --created-after, --created-before, --updated-after, --updated-before, --asset, or --jql. You can also set a default project in .jr.toml or run \"jr init\"."`.
**Error taxonomy**: `JrError::UserError` (exit 64).
**Trace**: Pass 3 BC-129 (R1)

---

#### BC-2.1.007: `build_filter_clauses` emits in stable order: assignee, reporter, status, open, team, recent, asset, created-after/before, updated-after/before

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:613-649`; 17 unit tests including `build_jql_parts_*`
**Subject**: Issue read
**Behavior**: Each `Some` flag pushes clause in listed order. Final JQL: `parts.join(" AND ")`. Order stable across invocations. Key clause shapes:
- `assignee = currentUser()` (for `--assignee me`)
- `reporter = <accountId>` (raw, not quoted)
- `created >= -7d` (for `--recent 7d`)
- `statusCategory != Done` (for `--open`)
- `status = "He said \"hi\" \\o/"` (JQL-escaped)
**Trace**: Pass 3 BC-130 (R1); BC-1093 (R4 enumeration)

---

#### BC-2.1.008: `--recent <duration>` validated by `jql::validate_duration` (NOT `duration::parse_duration`); combined units rejected

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:90-92`; `src/jql.rs:16-34`
**Subject**: Issue read
**Behavior**: `validate_duration("4w2d")` → Err. `--recent 4w2d` → `JrError::UserError("Invalid duration '4w2d'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M).")`. Pre-HTTP validation.
**Trace**: Pass 3 BC-131 (R1)

---

#### BC-2.1.009: `--created-after/before` and `--updated-after/before` validated via `jql::validate_date` BEFORE any HTTP

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:95-114`
**Subject**: Issue read
**Behavior**: Format: `YYYY-MM-DD`. On invalid: `Invalid date "<X>". Expected format: YYYY-MM-DD (e.g., 2026-03-18).` All four validators run before HTTP.
**Trace**: Pass 3 BC-132 (R1)

---

#### BC-2.1.010: `--created-before` and `--updated-before` use `date + Days::new(1)` for end-day-inclusive semantics

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:118-126`
**Subject**: Issue read
**Behavior**: User passes `--created-before 2026-03-31`; emitted clause is `created < "2026-04-01"`. Pinned by unit test `build_jql_parts_created_date_range`.
**Trace**: Pass 3 BC-133 (R1)

---

#### BC-2.1.011: `--asset KEY` resolves via CMDB fields; if NO CMDB fields → exit 64 with JSM plan message

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:168-183`
**Subject**: Issue read
**Behavior**: On `cmdb_fields.is_empty()`: `JrError::UserError("--asset requires Assets custom fields on this Jira instance. Assets requires a paid Jira Service Management plan.")`.
**Trace**: Pass 3 BC-134 (R1)

---

#### BC-2.1.012: `--asset KEY` ambiguous AQL result → exit 64 `Multiple assets match`; NO issue search fired

**Confidence**: HIGH
**Source**: `tests/assets.rs:1480-1573`; `src/cli/issue/list.rs:128-133`
**Subject**: Issue read
**Behavior**: Test asserts `stderr.contains("Multiple assets match")` + both candidate labels + `expect(0)` on `/rest/api/3/search/jql`. Exit 64.
**Trace**: Pass 3 BC-135 (R1)

---

#### BC-2.1.013: `--status <single-substring>` → exit 64 `Ambiguous status`; NO JQL search fired

**Confidence**: HIGH
**Source**: `tests/issue_list_errors.rs:368-422`; `src/cli/issue/list.rs:222-247`
**Subject**: Issue read
**Behavior**: `Mock::expect(0)` on `POST /rest/api/3/search/jql`. stderr `Ambiguous status "prog". Matches: In Progress`. Exit 64.
**Trace**: Pass 3 BC-105, BC-136 (R1)

---

#### BC-2.1.014: `--status NOMATCH` → `JrError::UserError` listing available statuses alphabetically

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:234-246`
**Subject**: Issue read
**Behavior**: `MatchResult::None(all)` constructs full error: `"No status matching \"X\" for project Y. Available: <comma-joined alphabetical list>"`. List always sorted.
**Trace**: Pass 3 BC-138 (R1)

---

#### BC-2.1.015: `--status <ExactMultiple>` treated as Exact (case-variant duplicates)

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:223-226`
**Trace**: Pass 3 BC-137 (R1)

---

#### BC-2.1.016: `--assets` column auto-enabled when `--asset KEY` filter is set

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:86-87`
**Subject**: Issue read
**Behavior**: `let show_assets = show_assets || asset_key.is_some();`
**Trace**: Pass 3 BC-145 (R1)

---

#### BC-2.1.017: `--assets` with no CMDB fields → stderr warning, no asset column

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:357-371`
**Behavior**: stderr: `"warning: --assets ignored. No Assets custom fields found on this Jira instance."`.
**Trace**: Pass 3 BC-146 (R1)

---

### 2.2 Issue List Behavior

#### BC-2.2.018: `--all` passes `maxResults=50`; default passes `maxResults=30`

**Confidence**: HIGH
**Source**: `tests/all_flag_behavior.rs:42-145`
**Subject**: Issue read
**Behavior**: `maxResults=50` for `--all`; `maxResults=30` for default. Pinned by request body match. `src/api/jira/issues.rs:50`: `max_per_page = limit.unwrap_or(50).min(100)`.
**Trace**: Pass 3 BC-103, BC-141 (R1)

---

#### BC-2.2.019: Truncation triggers second HTTP `POST /rest/api/3/search/approximate-count`

**Confidence**: HIGH
**Source**: `tests/all_flag_behavior.rs:88-145`; body-match pins `"jql": "(project = CAP)"`
**Subject**: Issue read
**Behavior**: When `--all` NOT set AND results > limit: issues `POST /search/approximate-count` with ORDER BY-stripped JQL. Stderr: `Showing 30 of ~42`. With `--all`: no truncation hint AND no count call.
**Trace**: Pass 3 BC-104, BC-140 (R1)

---

#### BC-2.2.020: `--all` + `--limit N` clap conflict: `cannot be used with`

**Confidence**: HIGH
**Source**: `tests/cli_smoke.rs:300-307`
**Trace**: Pass 3 BC-142 (R1)

---

#### BC-2.2.021: `--points` with no story_points_field_id → silently ignored, stderr warning

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:756-770`
**Subject**: Issue read
**Behavior**: stderr: `"warning: --points ignored. Story points field not configured. Run "jr init" or set story_points_field_id under [profiles.<name>] in ~/.config/jr/config.toml"`. Non-fatal; list proceeds without points column. Note: message must reference `[profiles.<name>]` not the deprecated `[fields]` section.
**Related**: BC-6.3.001 (multi-profile fields MUST-FIX); the error message text updated here is one of the pinned-text changes required by that fix.
**Trace**: Pass 3 BC-143 (R1)

---

#### BC-2.2.022: `--points` with configured field → pushes `customfield_NNNNN` onto request `extra` fields list

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:147-149, 656-668`
**Trace**: Pass 3 BC-144 (R1)

---

#### BC-2.2.023: Asset enrichment deduplicates by `(workspace_id, object_id)` before per-asset GETs

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:397-411`
**Subject**: Issue read
**Behavior**: `to_enrich: HashMap<(String, String), ()>` collects unique workspace/object pairs. Per-asset GETs issued once per unique key via `join_all` (concurrent). Mitigates partial N+1.
**Trace**: Pass 3 BC-147 (R1)

---

#### BC-2.2.024: board_id 404 → exit 64 with `Board 42 not found or not accessible` + board_id hint + `--jql` hint

**Confidence**: HIGH
**Source**: `tests/issue_list_errors.rs:21-76`
**Error taxonomy**: `JrError::UserError`.
**Trace**: Pass 3 BC-106

---

#### BC-2.2.025: board config 5xx → exit 1 with `Failed to fetch config for board 42` + `--jql` hint

**Confidence**: HIGH
**Source**: `tests/issue_list_errors.rs:78-130`
**Trace**: Pass 3 BC-107

---

#### BC-2.2.026: Sprint list 5xx → exit 1 with `Failed to list sprints for board 42` + `--jql` hint

**Confidence**: HIGH
**Source**: `tests/issue_list_errors.rs:132-194`
**Trace**: Pass 3 BC-108

---

#### BC-2.2.027: No active sprint → falls back to project-scoped JQL without error

**Confidence**: HIGH
**Source**: `tests/issue_list_errors.rs:196-263`
**Subject**: Issue read
**Behavior**: Empty `state=active` sprint list → falls back to `project = PROJ` JQL. No error, no warning (silent degrade per state machine §2.5 of Pass 8 synthesis).
**Trace**: Pass 3 BC-109

---

#### BC-2.2.028: `search_issues` default fields list: 16 fields in EXACT order

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:967-1022`
**Subject**: Issue read
**Behavior**: `summary, status, issuetype, priority, assignee, reporter, project, description, created, updated, resolution, components, fixVersions, labels, parent, issuelinks`. Body partial-JSON match asserts EXACT array.
**Trace**: Pass 3 BC-1063 (R4)

---

#### BC-2.2.029: `search_issues` with cursor continuation token sets `has_more = true`

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:264-310`
**Trace**: Pass 3 BC-1047, BC-1048 (R4)

---

#### BC-2.2.030: `search_issues` JQL body includes literal composed string with double-quoted project key

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:492-524`
**Behavior**: `project = "PROJ" AND (priority = Highest) ORDER BY updated DESC` pinned by body partial-match.
**Trace**: Pass 3 BC-1052 (R4)

---

#### BC-2.2.031: `client.approximate_count(jql)` POSTs to `/rest/api/3/search/approximate-count`; 5xx propagates as Err

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:337-386`
**Behavior**: Returns `u64`. Zero and 42 boundary cases tested. Server error → Err.
**Trace**: Pass 3 BC-1050 (R4)

---

### 2.3 Issue View

#### BC-2.3.032: `issue view <key>` GETs `/rest/api/3/issue/<key>` with `--output json` returning raw JSON

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:33-53`
**Trace**: Pass 3 BC-112

---

#### BC-2.3.033: `issue view` 5xx → exit 1 + `API error (500)` + no panic

**Confidence**: HIGH
**Source**: `tests/issue_view_errors.rs:18-56`
**Trace**: Pass 3 BC-113; BC-1135a (R4)

---

#### BC-2.3.034: `issue view` 401 → exit 2 + `Not authenticated` + `jr auth login`

**Confidence**: HIGH
**Source**: `tests/issue_view_errors.rs:58-100`
**Trace**: Pass 3 BC-114; BC-1135b (R4)

---

#### BC-2.3.035: Corrupt `teams.json` cache is non-fatal; UUID + "name not cached" hint shown inline

**Confidence**: HIGH
**Source**: `tests/issue_view_errors.rs:142-206`
**Subject**: Issue read
**Behavior**: Truncated `teams.json` (`{"teams": [`) → `read_cache` returns `Ok(None)` (parse-fail = cache miss). Issue view exits 0. Team row shows raw UUID + `(name not cached — run 'jr team list --refresh')`. stderr NOT contain `panic`.
**Trace**: Pass 3 BC-115; BC-1135d (R4); Top-30 BC rank #26

---

#### BC-2.3.036: `get_issue` deserializes: created, updated, reporter, resolution, components, fix_versions (all nullable)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:526-577, 579-607`
**Behavior**: Full fixture: all fields present. Minimal fixture: all return `None` (NOT panic). RFC3339+0000 timestamps, camelCase JSON paths.
**Trace**: Pass 3 BC-1053, BC-1054 (R4)

---

#### BC-2.3.037: `get_issue` with parent + links deserializes `fields.parent.key`, `fields.issuelinks[0].link_type.name`

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:208-231`
**Trace**: Pass 3 BC-1044 (R4)

---

#### BC-2.3.038: `IssueFields::story_points("customfield_X")` returns None for non-numeric values

**Confidence**: HIGH
**Source**: `src/types/jira/issue.rs:83-85`
**Trace**: Pass 3 BC-124

---

### 2.4 Comments

#### BC-2.4.039: `issue comments <key>` paginates at 100/page with `expand=properties`

**Confidence**: HIGH
**Source**: `tests/comments.rs:9-46, 73-158`
**Subject**: Issue read
**Behavior**: `maxResults=100`. `--limit N` → `maxResults=N`. Paginates via startAt until total reached.
**Trace**: Pass 3 BC-116

---

#### BC-2.4.040: `issue comments` 5xx → exit 1 + `API error (500)`

**Confidence**: HIGH
**Source**: `tests/comments.rs:163-200`
**Trace**: Pass 3 BC-117

---

#### BC-2.4.041: `issue comments --internal` adds `sd.public.comment` property (JSM-aware)

**Confidence**: MEDIUM
**Source**: `src/api/jira/issues.rs:181-198`
**Behavior**: `properties: [{key:"sd.public.comment", value:{internal:true}}]` on write. Read shape preserves `EntityProperty[]`. Non-JSM: Jira silently ignores.
**Trace**: Pass 3 BC-118

---

#### BC-2.4.042: `client.list_comments(key, None)` lists ALL comments via offset pagination

**Confidence**: HIGH
**Source**: `tests/comments.rs:104-158`
**Behavior**: Advances `startAt` by 100 until total reached.
**Trace**: Pass 3 BC-122

---

### 2.5 Changelog

#### BC-2.5.043: `issue changelog --field <substr>` filters items by case-insensitive field substring (client-side)

**Confidence**: MEDIUM
**Source**: `src/cli/issue/changelog.rs`; 38 unit tests
**Trace**: Pass 3 BC-119

---

#### BC-2.5.044: `issue changelog --author X` smart-constructs author needle (`:` or 12+ chars with digit → exact accountId)

**Confidence**: MEDIUM
**Source**: `src/cli/issue/changelog.rs` author needle
**Trace**: Pass 3 BC-120

---

#### BC-2.5.045: `issue changelog --reverse` reverses chronological order

**Confidence**: MEDIUM
**Source**: `src/cli/issue/changelog.rs`
**Trace**: Pass 3 BC-121

---

#### BC-2.5.046: Changelog JSON output snapshot pins full shape including nullable `fromString`/`toString`

**Confidence**: HIGH
**Source**: `tests/snapshots/issue_changelog__changelog_json_output_snapshot.snap`
**Subject**: Issue read
**Behavior**: `{entries: [{author: {accountId, active, displayName, emailAddress}, created, id, items: [{field, fieldtype, from, fromString, to, toString}]}], key}`. `author` can be `null` (system events). `fromString`/`toString` ARE nullable (null != missing).
**Trace**: Pass 3 BC-1118 (R4)

---

### 2.6 API Layer (Search / Find)

#### BC-2.6.047: `client.search_issues` with story-points extra field: deserializes `Some(5.0)` for issue with field, `None` without

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:130-166`
**Trace**: Pass 3 BC-1041 (R4)

---

#### BC-2.6.048: `client.find_story_points_field_id()` returns fields with name == "Story Points" from `/rest/api/3/field`

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:168-186`
**Trace**: Pass 3 BC-1042 (R4)

---

#### BC-2.6.049: `search_users` accepts FOUR distinct response shapes (bare array, paginated, empty, error)

**Confidence**: HIGH
**Source**: `tests/issue_commands.rs:388-490`
**Subject**: Issue read
**Behavior**: Bare array `[{...}]`; `{values: [...]}` paginated envelope; `[]`; error shape → Err. Via serde-untagged enum. Unrecognized shapes do NOT default to empty — they error.
**Trace**: Pass 3 BC-1051 (R4); Top-30 BC rank #20

---

#### BC-2.6.050: `client.search_issue_keys(jql, limit)` posts `/rest/api/3/search/jql` with body `fields: ["key"]` and returns `KeySearchResult { keys, has_more }`

**Confidence**: HIGH
**Source**: issue #350 (audit-followup from PR #348 / issue #110 PR2 Copilot review round 7); spec at `docs/specs/2026-05-13-search-issue-keys.md`; research at `.factory/research/issue-350-search-issue-keys-design.md`
**Subject**: Issue read (API layer — keys-only JQL search)
**Behavior**: POST `/rest/api/3/search/jql` sends body `fields: ["key"]` exclusively (never `BASE_ISSUE_FIELDS`). Deserializes only the top-level `key` per issue; ignores `fields {}` and unknown top-level fields. Paginates via `nextPageToken` cursor identically to `search_issues`, including the JRACLOUD-95368 repeated-cursor anti-loop guard (same stderr warning text). Returns `KeySearchResult { keys: Vec<String>, has_more: bool }`; `has_more = true` under TWO conditions: (a) the caller's limit was hit while the API still had rows (caller-side truncation), OR (b) the JRACLOUD-95368 repeated-cursor anti-loop guard fired (results may be incomplete; data loss is signaled to callers via this bit). Pure cursor exhaustion (page_has_more = false on a non-truncated path) always returns `has_more = false`. Refinement from PR #362 Copilot R1. Clamps `maxResults` per page to `.min(100)` for parity with `search_issues`. On every page-fetch iteration, after extending `all_keys` and before any break-decision check, `search_issue_keys` deduplicates `all_keys` in-place using order-preserving, first-occurrence-wins deduplication (HashSet retain, keyed on the key string). All exit paths (guard-abort, limit-truncation, cursor-exhaustion) therefore return a duplicate-free `keys` vec. Introduced in #365.
**Trace**: `src/api/jira/issues.rs::search_issue_keys` (impl); `src/cli/issue/create.rs::handle_edit::effective_keys` (caller); `tests/search_issue_keys.rs` (16 wiremock tests — 15 library tokio + 1 subprocess) + `tests/issue_bulk_pr2.rs::test_handle_edit_jql_truncation_error_still_triggers_after_migration` (caller-level regression)

---

#### BC-2.6.051: `client.search_issues(jql, limit, fields)` deduplicates results in-place on all exit paths (JRACLOUD-95368 mitigation)

**Confidence**: HIGH
**Source**: issue #365 (dedupe follow-up from PR #362); spec at `docs/specs/2026-05-14-search-issue-keys-dedupe.md`; research at `.factory/research/issue-365-design-validation.md`
**Subject**: Issue read (API layer — full-body JQL search)
**Behavior**: On every page-fetch iteration, after extending `all_issues` and before any break-decision check, `search_issues` deduplicates `all_issues` in-place using order-preserving, first-occurrence-wins deduplication keyed on `issue.key` (HashSet<String> of cloned keys, because `Issue` does not impl `Hash`). All exit paths (guard-abort, limit-truncation, cursor-exhaustion) therefore return a duplicate-free `issues` vec. `SearchResult.has_more` semantics are unchanged. As of issue #365, `has_more = true` on the guard-abort path no longer implies that `issues` contains duplicates. Symmetric to BC-2.6.050.
**Trace**: `src/api/jira/issues.rs::search_issues` (impl); `tests/rate_limit_cap_tests.rs` (4 new dedupe tests: `test_search_issues_repeated_cursor_abort_dedupes`, `test_search_issues_dedupes_non_consecutive_across_pages`, `test_search_issues_limit_truncation_dedupes_under_drift`, `test_search_issues_apr2025_overshoot_silenced_by_drift_dedupe`)

---

## Error Path Summary

All issue-read errors follow the universal pattern (BC-X.3.012):
- Network drop → exit 1 + `"Could not reach <host>; check your connection"`
- 401 → exit 2 + `Not authenticated` + `jr auth login`
- 5xx → exit 1 + `API error (5xx)` + friendly message
- Never: `panic` in stderr

Pass 3 sources: `tests/issue_list_errors.rs`, `tests/issue_view_errors.rs`, `tests/comments.rs`

## Total BCs in this file: 51 (representative set; BC-INDEX.md carries all 93)
