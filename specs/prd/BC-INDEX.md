---
context: bc-index
title: "BC Master Index"
total_bcs: 542  # cumulative claim (incl. range-collapsed) — see preamble below
last_updated: 2026-05-04
source_pass: 3
sections:
  - bc-1-auth-identity.md (57 BCs cumulative; 46 individually-bodied)
  - bc-2-issue-read.md (91 BCs cumulative; 49 individually-bodied)
  - bc-3-issue-write.md (77 BCs cumulative; 48 individually-bodied)
  - bc-4-assets-cmdb.md (32 BCs cumulative; 22 individually-bodied)
  - bc-5-boards-sprints.md (35 BCs cumulative; 17 individually-bodied)
  - bc-6-config-cache.md (39 BCs cumulative; 29 individually-bodied)
  - bc-7-output-render.md (80 BCs cumulative; 34 individually-bodied)
  - cross-cutting.md (130 BCs cumulative; 64 individually-bodied)
  - nfr-catalog.md (45 NFR items, not counted in BC total)
---

# BC Master Index — jira-cli L3 PRD

Master traceability: L3 BC ID → L2 entity → Pass 3 BC ID → Source code → Confidence → Subject

---

## Preamble: Ranged vs. Anchored BCs

**Two kinds of BC entries exist in this index:**

1. **Individually-anchored** — has a `#### BC-S.SS.NNN:` heading in the corresponding body file. Can be directly linked. Test names should be `test_BC_S_SS_NNN_<description>`.
2. **Range-collapsed** — a single index row covers multiple BCs that were clustered in Pass 3 but not individually expanded to body headings. Marked with `[range-collapsed]`. They are counted in `total_bcs` (cumulative claim) but do not have individually-bodied `#### BC-` headings.

**Source of truth**: The body files (`bc-*.md`, `cross-cutting.md`) are canonical. This index is derived from them. When a body file and this index disagree on a BC ID or title, the body file wins.

**Counting**: `total_bcs` in each file's frontmatter = cumulative claim (individually-bodied + range-collapsed). `definitional_count` = count of `#### BC-` headings in that file only.

---

## Index Format

```
| L3 BC ID | Summary | Pass 3 BC ID | Source code | Confidence | Subject |
```

Pass 3 BC ID refers to the originating BC number in the semport pass files.
R1/R4 prefix = deepening round that introduced it.
`[range-collapsed]` = BC exists in cumulative count but not individually-bodied in the file.

---

## Section 1: Auth & Identity (bc-1-auth-identity.md) — 57 BCs cumulative; 46 individually-bodied

### 1.1 OAuth Flow & Profile Resolution (12 BCs: BC-1.1.001..012)

| L3 BC ID | Summary | Pass 3 BC ID | Source code | Confidence | Subject |
|---|---|---|---|---|---|
| BC-1.1.001 | `auth list` against fresh-install returns empty JSON array | BC-001 | tests/auth_profiles.rs:53-60 | HIGH | Auth & Identity |
| BC-1.1.002 | `auth status` against fresh install exits 0 with helpful stderr | BC-002 | tests/auth_profiles.rs:62-75 | HIGH | Auth & Identity |
| BC-1.1.003 | `auth switch <unknown>` exits 64 | BC-003 | tests/auth_profiles.rs:42-50 | HIGH | Auth & Identity |
| BC-1.1.004 | `auth status --profile <unknown>` exits 64 with "unknown profile" | BC-004 | tests/auth_profiles.rs:78-96 | HIGH | Auth & Identity |
| BC-1.1.005 | `auth logout --profile <unknown>` exits 64 | BC-005 | tests/auth_profiles.rs:98-118 | HIGH | Auth & Identity |
| BC-1.1.006 | `auth remove <active>` is rejected with exit 64 | BC-006 | tests/auth_profiles.rs:120-140 | HIGH | Auth & Identity |
| BC-1.1.007 | Profile resolution precedence: flag > JR_PROFILE env > config.default_profile > "default" | BC-007 | tests/auth_profiles.rs:142-186; src/config.rs:95-110 | HIGH | Auth & Identity |
| BC-1.1.008 | Global `--profile` flag propagates to `auth status` via main.rs composition | BC-008 | tests/auth_profiles.rs:193-231 | HIGH | Auth & Identity |
| BC-1.1.009 | `auth login --profile <new>` creates profile even when profile doesn't yet exist | BC-009 | tests/auth_profiles.rs:241-280 | HIGH | Auth & Identity |
| BC-1.1.010 | `auth login --profile X` succeeds even when JR_PROFILE points to absent profile | BC-010 | tests/auth_profiles.rs:290-332 | HIGH | Auth & Identity |
| BC-1.1.011 | `auth refresh --no-input` against unconfigured profile exits 64 naming "no URL configured" | BC-011 | tests/auth_refresh.rs:43-106 | HIGH | Auth & Identity |
| BC-1.1.012 | Malformed config TOML errors exit 78 and does NOT overwrite the file | BC-012; BC-1139 (R4) | tests/auth_login_config_errors.rs:18-97 | HIGH | Auth & Identity |

### 1.2 Profile Lifecycle Management (6 BCs: BC-1.2.013..018)

| L3 BC ID | Summary | Pass 3 BC ID | Source code | Confidence | Subject |
|---|---|---|---|---|---|
| BC-1.2.013 | `auth logout` deletes only `<profile>:oauth-access-token` and `<profile>:oauth-refresh-token` | BC-013-R | src/api/auth.rs:24-32, 88-97 | HIGH | Auth & Identity |
| BC-1.2.014 | `auth remove <name>` performs three-step delete: config entry, OAuth tokens, cache directory | BC-014-R | src/cli/auth.rs; src/cache.rs:82-88 | HIGH | Auth & Identity |
| BC-1.2.015 | `auth refresh --help` includes the `--oauth` flag | BC-026 (R1) | tests/auth_refresh.rs:7-24 | HIGH | Auth & Identity |
| BC-1.2.016 | `auth refresh --oauth --help` is accepted in either flag order | BC-027 (R1) | tests/auth_refresh.rs:26-40 | HIGH | Auth & Identity |
| BC-1.2.017 | `auth login --profile X` against `JR_PROFILE=ghost` succeeds creating profile X | BC-029 (R1) | tests/auth_profiles.rs:282-333 | HIGH | Auth & Identity |
| BC-1.2.018 | Global `--profile` propagates to all auth subcommands via subcmd.profile.or(cli.profile) | BC-030 (R1) | tests/auth_profiles.rs:188-231 | HIGH | Auth & Identity |

### 1.3 Embedded OAuth App (6 BCs: BC-1.3.019..024)

| L3 BC ID | Summary | Pass 3 BC ID | Source code | Confidence | Subject |
|---|---|---|---|---|---|
| BC-1.3.019 | Embedded OAuth app `Debug` redacts client_secret | BC-019; BC-1168 (R4) | src/api/auth_embedded.rs:34, 220-239 | HIGH | Auth & Identity |
| BC-1.3.020 | Build with empty XOR inputs → `embedded_oauth_app()` returns None | BC-020 | src/api/auth_embedded.rs:100-106 | HIGH | Auth & Identity |
| BC-1.3.021 | `embedded_oauth_app_present()` checks presence without decoding | BC-021; BC-022-R (R1) | src/api/auth_embedded.rs:132-136 | HIGH | Auth & Identity |
| BC-1.3.022 | `OAuthAppSource` resolution chain: Flag > Env > Keychain > Embedded > Prompt > None | BC-022-R | src/api/auth_embedded.rs:46-57 | HIGH | Auth & Identity |
| BC-1.3.023 | DEFAULT_OAUTH_SCOPES includes `offline_access`, CMDB scopes, and `write:jira-work` | BC-035 (R1) | src/api/auth.rs:34-63 | HIGH | Auth & Identity |
| BC-1.3.024 | Embedded OAuth integration test is `#[ignore]`-gated and stubs `unimplemented!()` | BC-028 (R1) | tests/oauth_embedded_login.rs:13-32 | HIGH | Auth & Identity |

### 1.4 Token Keychain Layout (6 BCs: BC-1.4.025..030)

| L3 BC ID | Summary | Pass 3 BC ID | Source code | Confidence | Subject |
|---|---|---|---|---|---|
| BC-1.4.025 | `default` profile lazy-migrates legacy flat OAuth keys; non-default profiles never inherit | BC-023-R | src/api/auth.rs:111-169 | HIGH | Auth & Identity |
| BC-1.4.026 | `refresh_oauth_token` signature is `(profile: &str)` only — resolves credentials internally | BC-024-R | src/api/auth.rs:700-770; CLAUDE.md | HIGH | Auth & Identity |
| BC-1.4.027 | Per-profile keychain keys: `<profile>:oauth-access-token` / `<profile>:oauth-refresh-token` | BC-1153 (R4) | src/api/auth.rs:24-32 | HIGH | Auth & Identity |
| BC-1.4.028 | `load_oauth_tokens` errors on PARTIAL state (one token present, other missing) | BC-1156 (R4) | src/api/auth.rs:1249-1269 | HIGH | Auth & Identity |
| BC-1.4.029 | `load_oauth_tokens("sandbox")` does NOT inherit legacy flat keys | BC-1158 (R4) | src/api/auth.rs:1323-1341 | HIGH | Auth & Identity |
| BC-1.4.030 | `resolve_refresh_app_credentials` prefers KEYCHAIN over EMBEDDED | BC-1159 (R4) | src/api/auth.rs:1347-1357 | HIGH | Auth & Identity |

### 1.5 OAuth State Machine (11 BCs: BC-1.5.031..041)

| L3 BC ID | Summary | Pass 3 BC ID | Source code | Confidence | Subject |
|---|---|---|---|---|---|
| BC-1.5.031 | Embedded OAuth callback URL is exactly `http://127.0.0.1:53682/callback` | BC-031 (R1); BC-1140/1141 (R4) | src/api/auth.rs:374-477; ADR-0006 | HIGH | Auth & Identity |
| BC-1.5.032 | `RedirectUriStrategyRequest::Fixed(p)` produces EADDRINUSE friendly error | BC-032 (R1); BC-1161 (R4) | src/api/auth.rs:427-447 | HIGH | Auth & Identity |
| BC-1.5.033 | `ResolvedRedirect` private fields prevent listener detachment from strategy | BC-033 (R1) | src/api/auth.rs:455-477 | HIGH | Auth & Identity |
| BC-1.5.034 | BYO OAuth uses `DynamicPort` (dynamic `:0`); embedded uses `FixedPort(53682)` | BC-1140 (R4) | src/api/auth.rs:927-937 | HIGH | Auth & Identity |
| BC-1.5.035 | `generate_state()` produces 32 bytes from OsRng encoded as 64 hex chars | BC-1146 (R4) | src/api/auth.rs:882 | HIGH | Auth & Identity |
| BC-1.5.036 | OAuth flow has NO PKCE (`code_challenge`/`code_verifier` absent) | BC-1148, BC-1149 (R4) | src/api/auth.rs:608-616 | HIGH | Auth & Identity |
| BC-1.5.037 | `build_authorize_url` percent-encodes hostile `client_id` containing injection chars | BC-1149 (R4) | src/api/auth.rs:1043-1060 | HIGH | Auth & Identity |
| BC-1.5.038 | `accessible_resources` first-wins for cloud_id discovery (silent first-only) | BC-1176 (R4) | src/api/auth.rs | HIGH | Auth & Identity |
| BC-1.5.039 | OAuth token stored as `<profile>:oauth-access-token` and `<profile>:oauth-refresh-token` post-login | BC-1151 (R4) | src/api/auth.rs | HIGH | Auth & Identity |
| BC-1.5.040 | OAuth callback validates state (CSRF check) before token exchange | H-047 (holdout) | src/api/auth.rs:898 | HIGH | Auth & Identity |
| BC-1.5.041 | `extract_query_param` parses `code` and `state` from HTTP GET request line | BC-1142, BC-1143, BC-1144 (R4) | src/api/auth.rs:948-965 | HIGH | Auth & Identity |

### 1.6 Auth Error Handling & 401 Dispatch (5 BCs: BC-1.6.042..046)

| L3 BC ID | Summary | Pass 3 BC ID | Source code | Confidence | Subject |
|---|---|---|---|---|---|
| BC-1.6.042 | 401 + `scope does not match` body → InsufficientScope with 5 required substrings | BC-015; BC-1085 (R4) | tests/api_client.rs:99-144 | HIGH | Auth & Identity |
| BC-1.6.043 | 401 without scope-mismatch substring → NotAuthenticated, NOT InsufficientScope | BC-016; BC-1086 (R4) | tests/api_client.rs:146-181 | HIGH | Auth & Identity |
| BC-1.6.044 | 401 scope-mismatch match is case-insensitive (`to_ascii_lowercase`) | BC-017; BC-1087 (R4) | tests/api_client.rs:183-216 | HIGH | Auth & Identity |
| BC-1.6.045 | Non-401 status with scope-mismatch substring does NOT dispatch to InsufficientScope | BC-018; BC-1088 (R4) | tests/api_client.rs:219-255 | HIGH | Auth & Identity |
| BC-1.6.046 | `auth list` table snapshot: 4 columns, active profile with `* ` prefix | BC-1115 (R4) | src/cli/snapshots/jr__cli__auth__tests__list_table_snapshot.snap | HIGH | Auth & Identity |

---

## Section 2: Issue Read (bc-2-issue-read.md) — 91 BCs cumulative; 49 individually-bodied

### 2.1 JQL Composition (17 BCs: BC-2.1.001..017)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-2.1.001 | `issue list` cursor-paginates via `POST /rest/api/3/search/jql` | BC-101 | src/cli/issue/list.rs | HIGH |
| BC-2.1.002 | `--jql X` wraps in parens, strips ORDER BY, re-appends `ORDER BY updated DESC` | BC-102, BC-125 (R1) | src/cli/issue/list.rs:36-52 | HIGH |
| BC-2.1.003 | Scrum board with active sprint → JQL `sprint = <id> ORDER BY rank ASC` | BC-126 (R1) | src/cli/issue/list.rs:278-282 | HIGH |
| BC-2.1.004 | Kanban board → `project = "X" AND statusCategory != Done ORDER BY rank ASC` | BC-127 (R1) | src/cli/issue/list.rs:302-310 | HIGH |
| BC-2.1.005 | No board_id → `project = "X" ORDER BY updated DESC` | BC-128 (R1) | src/cli/issue/list.rs:331-338 | HIGH |
| BC-2.1.006 | No project AND no filters AND no `--jql` → exit 64 listing all 12 filter sources | BC-129 (R1) | src/cli/issue/list.rs:344-351 | HIGH |
| BC-2.1.007 | `build_filter_clauses` emits in stable order: assignee, reporter, status, open, team, recent, asset, date filters | BC-130 (R1); BC-1093 (R4) | src/cli/issue/list.rs:613-649 | HIGH |
| BC-2.1.008 | `--recent <duration>` validated by `jql::validate_duration`; combined units rejected | BC-131 (R1) | src/cli/issue/list.rs:90-92 | HIGH |
| BC-2.1.009 | `--created-after/before` and `--updated-after/before` validated via `jql::validate_date` BEFORE any HTTP | BC-132 (R1) | src/cli/issue/list.rs:95-114 | HIGH |
| BC-2.1.010 | `--created-before` and `--updated-before` use `date + Days::new(1)` for end-day-inclusive semantics | BC-133 (R1) | src/cli/issue/list.rs:118-126 | HIGH |
| BC-2.1.011 | `--asset KEY` resolves via CMDB fields; if NO CMDB fields → exit 64 with JSM plan message | BC-134 (R1) | src/cli/issue/list.rs:168-183 | HIGH |
| BC-2.1.012 | `--asset KEY` ambiguous AQL result → exit 64 `Multiple assets match`; NO issue search fired | BC-135 (R1) | tests/assets.rs:1480-1573 | HIGH |
| BC-2.1.013 | `--status <single-substring>` → exit 64 `Ambiguous status`; NO JQL search fired | BC-105, BC-136 (R1) | tests/issue_list_errors.rs:368-422 | HIGH |
| BC-2.1.014 | `--status NOMATCH` → `JrError::UserError` listing available statuses alphabetically | BC-138 (R1) | src/cli/issue/list.rs:234-246 | HIGH |
| BC-2.1.015 | `--status <ExactMultiple>` treated as Exact (case-variant duplicates) | BC-137 (R1) | src/cli/issue/list.rs:223-226 | HIGH |
| BC-2.1.016 | `--assets` column auto-enabled when `--asset KEY` filter is set | BC-145 (R1) | src/cli/issue/list.rs:86-87 | HIGH |
| BC-2.1.017 | `--assets` with no CMDB fields → stderr warning, no asset column | BC-146 (R1) | src/cli/issue/list.rs:357-371 | HIGH |

### 2.2 Issue List Behavior (14 BCs: BC-2.2.018..031)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-2.2.018 | `--all` passes `maxResults=50`; default passes `maxResults=30` | BC-103, BC-141 (R1) | tests/all_flag_behavior.rs:42-145 | HIGH |
| BC-2.2.019 | Truncation triggers second HTTP `POST /rest/api/3/search/approximate-count` | BC-104, BC-140 (R1) | tests/all_flag_behavior.rs:88-145 | HIGH |
| BC-2.2.020 | `--all` + `--limit N` clap conflict: `cannot be used with` | BC-142 (R1) | tests/cli_smoke.rs:300-307 | HIGH |
| BC-2.2.021 | `--points` with no story_points_field_id → silently ignored, stderr warning | BC-143 (R1) | src/cli/issue/list.rs:756-770 | HIGH |
| BC-2.2.022 | `--points` with configured field → pushes `customfield_NNNNN` onto request `extra` fields list | BC-144 (R1) | src/cli/issue/list.rs:147-149, 656-668 | HIGH |
| BC-2.2.023 | Asset enrichment deduplicates by `(workspace_id, object_id)` before per-asset GETs | BC-147 (R1) | src/cli/issue/list.rs:397-411 | HIGH |
| BC-2.2.024 | board_id 404 → exit 64 with `Board 42 not found or not accessible` + board_id hint + `--jql` hint | BC-106 | tests/issue_list_errors.rs:21-76 | HIGH |
| BC-2.2.025 | board config 5xx → exit 1 with `Failed to fetch config for board 42` + `--jql` hint | BC-107 | tests/issue_list_errors.rs:78-130 | HIGH |
| BC-2.2.026 | Sprint list 5xx → exit 1 with `Failed to list sprints for board 42` + `--jql` hint | BC-108 | tests/issue_list_errors.rs:132-194 | HIGH |
| BC-2.2.027 | No active sprint → falls back to project-scoped JQL without error | BC-109 | tests/issue_list_errors.rs:196-263 | HIGH |
| BC-2.2.028 | `search_issues` default fields list: 16 fields in EXACT order | BC-1063 (R4) | tests/issue_commands.rs:967-1022 | HIGH |
| BC-2.2.029 | `search_issues` with cursor continuation token sets `has_more = true` | BC-1047, BC-1048 (R4) | tests/issue_commands.rs:264-310 | HIGH |
| BC-2.2.030 | `search_issues` JQL body includes literal composed string with double-quoted project key | BC-1052 (R4) | tests/issue_commands.rs:492-524 | HIGH |
| BC-2.2.031 | `client.approximate_count(jql)` POSTs to `/rest/api/3/search/approximate-count`; 5xx propagates as Err | BC-1050 (R4) | tests/issue_commands.rs:337-386 | HIGH |

### 2.3 Issue View (7 BCs: BC-2.3.032..038)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-2.3.032 | `issue view <key>` GETs `/rest/api/3/issue/<key>` with `--output json` returning raw JSON | BC-112 | tests/issue_commands.rs:33-53 | HIGH |
| BC-2.3.033 | `issue view` 5xx → exit 1 + `API error (500)` + no panic | BC-113; BC-1135a (R4) | tests/issue_view_errors.rs:18-56 | HIGH |
| BC-2.3.034 | `issue view` 401 → exit 2 + `Not authenticated` + `jr auth login` | BC-114; BC-1135b (R4) | tests/issue_view_errors.rs:58-100 | HIGH |
| BC-2.3.035 | Corrupt `teams.json` cache is non-fatal; UUID + "name not cached" hint shown inline | BC-115; BC-1135d (R4) | tests/issue_view_errors.rs:142-206 | HIGH |
| BC-2.3.036 | `get_issue` deserializes: created, updated, reporter, resolution, components, fix_versions (all nullable) | BC-1053, BC-1054 (R4) | tests/issue_commands.rs:526-577, 579-607 | HIGH |
| BC-2.3.037 | `get_issue` with parent + links deserializes `fields.parent.key`, `fields.issuelinks[0].link_type.name` | BC-1044 (R4) | tests/issue_commands.rs:208-231 | HIGH |
| BC-2.3.038 | `IssueFields::story_points("customfield_X")` returns None for non-numeric values | BC-124 | src/types/jira/issue.rs:83-85 | HIGH |

### 2.4 Comments (4 BCs: BC-2.4.039..042)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-2.4.039 | `issue comments <key>` paginates at 100/page with `expand=properties` | BC-116 | tests/comments.rs:9-46, 73-158 | HIGH |
| BC-2.4.040 | `issue comments` 5xx → exit 1 + `API error (500)` | BC-117 | tests/comments.rs:163-200 | HIGH |
| BC-2.4.041 | `issue comments --internal` adds `sd.public.comment` property (JSM-aware) | BC-118 | src/api/jira/issues.rs:181-198 | MEDIUM |
| BC-2.4.042 | `client.list_comments(key, None)` lists ALL comments via offset pagination | BC-122 | tests/comments.rs:104-158 | HIGH |

### 2.5 Changelog (4 BCs: BC-2.5.043..046)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-2.5.043 | `issue changelog --field <substr>` filters items by case-insensitive field substring (client-side) | BC-119 | src/cli/issue/changelog.rs | MEDIUM |
| BC-2.5.044 | `issue changelog --author X` smart-constructs author needle | BC-120 | src/cli/issue/changelog.rs | MEDIUM |
| BC-2.5.045 | `issue changelog --reverse` reverses chronological order | BC-121 | src/cli/issue/changelog.rs | MEDIUM |
| BC-2.5.046 | Changelog JSON output snapshot pins full shape including nullable `fromString`/`toString` | BC-1118 (R4) | tests/snapshots/issue_changelog | HIGH |

### 2.6 API Layer (3 BCs: BC-2.6.047..049)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-2.6.047 | `client.search_issues` with story-points extra field: deserializes `Some(5.0)` for issue with field, `None` without | BC-1041 (R4) | tests/issue_commands.rs:130-166 | HIGH |
| BC-2.6.048 | `client.find_story_points_field_id()` returns fields with name == "Story Points" from `/rest/api/3/field` | BC-1042 (R4) | tests/issue_commands.rs:168-186 | HIGH |
| BC-2.6.049 | `search_users` accepts FOUR distinct response shapes (bare array, paginated, empty, error) | BC-1051 (R4) | tests/issue_commands.rs:388-490 | HIGH |

---

## Section 3: Issue Write (bc-3-issue-write.md) — 77 BCs cumulative; 48 individually-bodied

### 3.1 Assign (9 BCs: BC-3.1.001..009)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-3.1.001 | `issue assign --account-id <id>` PUTs `/issue/<key>/assignee` with `{accountId: <id>}` | BC-201; BC-1077 (R4) | tests/cli_handler.rs:58-91 | HIGH |
| BC-3.1.002 | `issue assign --to <name>` resolves via assignable user search then assigns | BC-202; BC-1059 (R4) | tests/cli_handler.rs:93-133 | HIGH |
| BC-3.1.003 | `issue assign --to me` resolves current user via `/myself` | BC-203; BC-1061 (R4) | tests/issue_commands.rs:879-920 | HIGH |
| BC-3.1.004 | `issue assign` is idempotent — already-assigned-to-target → exit 0 + `"changed": false` | BC-204; BC-1062 (R4) | tests/issue_commands.rs:922-965 | HIGH |
| BC-3.1.005 | `issue assign --unassign` PUTs `{accountId: null}` | BC-205 | src/cli/issue/workflow.rs | MEDIUM |
| BC-3.1.006 | `--to` ⊕ `--account-id` ⊕ `--unassign` clap conflict (mutually exclusive) | BC-206 | tests/cli_smoke.rs:170-211 | HIGH |
| BC-3.1.007 | `search_assignable_users` returning empty Vec → `Ok(Vec::new())` (NOT Err); handler decides UX | BC-1060 (R4) | tests/issue_commands.rs:856-877 | HIGH |
| BC-3.1.008 | `assign_issue("ERR-1", Some("bogus-id"))` against 404 → Err + `"does not exist"` message | BC-1078 (R4) | tests/issue_commands.rs:1705-1738 | HIGH |
| BC-3.1.009 | `search_assignable_users_by_project(query, projectKey)` GETs `/rest/api/3/user/assignable/multiProjectSearch` | BC-1064 (R4) | tests/issue_commands.rs:1024-1082 | HIGH |

### 3.2 Move / Transition (12 BCs: BC-3.2.001..012)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-3.2.001 | `issue move <key> <target>` is idempotent when current == target (by status name) | BC-207; BC-1074 (R4) | tests/issue_commands.rs:1500-1549 | HIGH |
| BC-3.2.002 | `issue move <key>` is idempotent via transition-name→status-name resolution too | BC-1075 (R4) | tests/issue_commands.rs:1551-1604 | HIGH |
| BC-3.2.003 | `issue move` resolves transition by NAME match | BC-1069 (R4) | tests/issue_commands.rs:1219-1276 | HIGH |
| BC-3.2.004 | `issue move` resolves by STATUS NAME match | BC-1070 (R4) | tests/issue_commands.rs:1278-1335 | HIGH |
| BC-3.2.005 | Duplicate candidates (same transition + status name) are de-duplicated; only ONE candidate presented | BC-1071 (R4) | tests/issue_commands.rs:1337-1394 | HIGH |
| BC-3.2.006 | Ambiguous move → exit non-zero + stderr `"Ambiguous"` + NO POST | BC-1072 (R4) | tests/issue_commands.rs:1396-1444 | HIGH |
| BC-3.2.007 | No-match move → enriched candidate list in stderr: `"Complete (→ Completed)"` format | BC-1073 (R4) | tests/issue_commands.rs:1446-1498 | HIGH |
| BC-3.2.008 | `--no-input` single-substring move → exit 64 + `"Ambiguous transition"` + ZERO POST | BC-1079 (R4) | tests/issue_commands.rs:1748-1810 | HIGH |
| BC-3.2.009 | `issue move` 400 "resolution required" → `--resolution` hint + `jr issue resolutions` discovery pointer | BC-208, BC-209 | tests/issue_resolution.rs:88-158 | HIGH |
| BC-3.2.010 | `issue resolutions` reads cache-first (7d TTL); JSON: `[{name, id, description}]` | BC-210 | tests/issue_resolution.rs:11-46, 49-86 | HIGH |
| BC-3.2.011 | `transition_issue(key, id, Some(&fields))` body contains `{transition: {id}, fields: {resolution: {name: "Done"}}}` | BC-1039 (R4) | tests/issue_commands.rs:79-103 | HIGH |
| BC-3.2.012 | `transition_issue(key, id, None)` body MUST NOT contain `"fields"` key | BC-1040 (R4) | tests/issue_commands.rs:105-128 | HIGH |

### 3.3 Create (9 BCs: BC-3.3.001..009)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-3.3.001 | `issue create` POSTs `/rest/api/3/issue` returning `{"key": "FOO-123"}` | BC-211 | tests/issue_create_json.rs | HIGH |
| BC-3.3.002 | `issue create` with assignee — uses `search_assignable_users_by_project` (multiProjectSearch) | BC-1064 (R4) | tests/issue_commands.rs:1024-1082 | HIGH |
| BC-3.3.003 | `issue create --to me` uses `get_myself()` (no search HTTP) | BC-1065 (R4) | tests/issue_commands.rs:1084-1127 | HIGH |
| BC-3.3.004 | `issue create` WITHOUT assignee — body has `{project, issuetype, summary}` ONLY (no assignee key) | BC-1066 (R4) | tests/issue_commands.rs:1129-1154 | HIGH |
| BC-3.3.005 | `issue create` assignee-not-found → stops short of create (NO POST mock) | BC-1067 (R4) | tests/issue_commands.rs:1156-1180 | HIGH |
| BC-3.3.006 | `issue create --account-id <id>` skips user search entirely | BC-1068 (R4) | tests/issue_commands.rs:1182-1217 | HIGH |
| BC-3.3.007 | `--to` and `--account-id` clap conflict on `issue create` | BC-224 | tests/cli_smoke.rs:215-235 | HIGH |
| BC-3.3.008 | `issue create --markdown -d '...'` converts markdown to ADF before POST | BC-212 | tests/issue_create_json.rs | MEDIUM |
| BC-3.3.009 | `create_issue` browse URL uses `client.instance_url()` (NOT `client.base_url()`) | BC-1076 (R4) | tests/issue_commands.rs:1606-1644 | HIGH |

### 3.4 Edit and Open (8 BCs: BC-3.4.001..008)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-3.4.001 | `handle_open` MUST compose URL as `<instance_url>/browse/<key>` using `client.instance_url()` [MUST-FIX: NFR-R-B] | BC-220; NFR-R-B; BC-1010 (R4) | src/cli/issue/workflow.rs:636 | HIGH |
| BC-3.4.002 | `issue open --url-only` prints URL to stdout (no browser launch) | BC-221 | Pass 2 §2b.1 | MEDIUM |
| BC-3.4.003 | `issue edit` PUTs `/rest/api/3/issue/<key>` with ADF description; accepts 204 | BC-1055 (R4) | tests/issue_commands.rs:609-645 | HIGH |
| BC-3.4.004 | `issue edit` with `markdown_to_adf("**bold text**")` → ADF marks `[{type: "strong"}]` on wire | BC-1056 (R4) | tests/issue_commands.rs:647-687 | HIGH |
| BC-3.4.005 | `issue edit` with multiple fields sends both in body simultaneously | BC-1057 (R4) | tests/issue_commands.rs:689-727 | HIGH |
| BC-3.4.006 | `issue edit --label add:foo --label remove:bar` interprets prefix and merges with existing | BC-213 | tests/issue_create_json.rs | MEDIUM |
| BC-3.4.007 | `--description` and `--description-stdin` clap conflict | BC-214 | tests/cli_smoke.rs:34-48 | HIGH |
| BC-3.4.008 | `--points X` and `--no-points` clap conflict | BC-215 | tests/cli_smoke.rs:280-287 | HIGH |

### 3.5 Comments (1 BC: BC-3.5.001)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-3.5.001 | `issue comment <key> --internal` adds `sd.public.comment` property | BC-219 | src/api/jira/issues.rs | HIGH |

### 3.6 Links (5 BCs: BC-3.6.001..005)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-3.6.001 | `issue link <k1> <k2> [--type T]` POSTs `/rest/api/3/issueLink`; default type "Relates" | BC-216; BC-1045 (R4) | src/api/jira/links.rs | HIGH |
| BC-3.6.002 | `issue link FOO-1 FOO-2 --type block` single-substring → exit 64 + `"Ambiguous link type"` + ZERO POST | BC-1080 (R4) | tests/issue_commands.rs:1812-1867 | HIGH |
| BC-3.6.003 | `issue unlink FOO-1 FOO-2 --type block` single-substring → exit 64 + ZERO DELETE | BC-1081 (R4) | tests/issue_commands.rs:1869-1920 | HIGH |
| BC-3.6.004 | `client.delete_issue_link("10001")` DELETEs `/rest/api/3/issueLink/10001`; accepts 204 | BC-1046 (R4) | tests/issue_commands.rs:250-262 | HIGH |
| BC-3.6.005 | `client.list_link_types()` returns 3 link types from `/rest/api/3/issueLinkType` | BC-218; BC-1043 (R4) | tests/issue_commands.rs:188-206 | HIGH |

### 3.7 Remote Links (4 BCs: BC-3.7.001..004)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-3.7.001 | `issue remote-link <key> --url X` POSTs `/issue/<key>/remotelink`; URL gains trailing slash from normalization | BC-222; BC-1126 (R4) | tests/issue_remote_link.rs:19-84 | HIGH |
| BC-3.7.002 | `issue remote-link` defaults `--title` to URL when omitted | BC-223; BC-1127 (R4) | tests/issue_remote_link.rs:87-147 | HIGH |
| BC-3.7.003 | `issue remote-link --url not-a-url` → exit 64 + `"--url"` + `"not a valid url"`; ZERO HTTP | BC-1130 (R4) | tests/issue_remote_link.rs:259-301 | HIGH |
| BC-3.7.004 | `issue remote-link --url ftp://example.com` → exit 64 + `"http or https"` + `"ftp"` | BC-1131 (R4) | tests/issue_remote_link.rs:309-348 | HIGH |

---

## Section 4: Assets & CMDB (bc-4-assets-cmdb.md) — 32 BCs cumulative; 22 individually-bodied

### 4.1 AQL / CMDB Field Resolution (7 BCs: BC-4.1.001..007)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-4.1.001 | `find_cmdb_fields()` filters by `schema.custom == "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"` | BC-301; BC-1137a (R4) | tests/cmdb_fields.rs:50-83 | HIGH |
| BC-4.1.002 | `build_asset_clause` for single CMDB field emits `"<NAME>" IN aqlFunction("Key = \"<KEY>\"")` (NO outer parens) | BC-306, BC-306-R (R1) | src/jql.rs:61-82 | HIGH |
| BC-4.1.003 | `build_asset_clause` uses `escape_value` for BOTH field name AND asset key | BC-307, BC-307-R (R1) | src/jql.rs:67-74 | HIGH |
| BC-4.1.004 | Two CMDB fields → parenthesized OR-join: `("X" IN aqlFunction(...) OR "Y" IN aqlFunction(...))` | BC-308, BC-308-R (R1) | src/jql.rs:77-81 | HIGH |
| BC-4.1.005 | `validate_asset_key("CUST-5")` → Ok; `"CUST"` → Err; `"5-CUST"` → Err | BC-309 | src/jql.rs:39-54 | HIGH |
| BC-4.1.006 | `extract_linked_assets` reads `[{label, objectKey}]` shape → `LinkedAsset{key, name}` | BC-302; BC-1137c (R4) | tests/cmdb_fields.rs:86-118 | HIGH |
| BC-4.1.007 | `extract_linked_assets` returns empty Vec for null custom field value | BC-303; BC-1137d (R4); BC-324 (R1) | tests/cmdb_fields.rs:120-146 | HIGH |

### 4.2 Asset Search & View (9 BCs: BC-4.2.001..009)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-4.2.001 | `assets search` discovers workspace ID first (cache or API) | BC-310; BC-322 (R1) | tests/assets.rs | HIGH |
| BC-4.2.002 | `client.search_assets(workspace_id, aql, limit, include_attrs)` POSTs to `/jsm/assets/workspace/<id>/v1/object/aql` | BC-316 (R1) | tests/assets.rs:39-80, 238-295 | HIGH |
| BC-4.2.003 | `AssetsPage::is_last` accepts both bool and string-encoded bool `"true"` | BC-317 (R1) | tests/assets.rs:140-170 | HIGH |
| BC-4.2.004 | `client.get_asset(workspace_id, id, include_attrs=true)` GETs `/jsm/assets/workspace/<id>/v1/object/<oid>?includeAttributes=true` | BC-318 (R1) | tests/assets.rs:172-203 | HIGH |
| BC-4.2.005 | `client.get_connected_tickets(workspace_id, oid)` GETs `/jsm/assets/workspace/<id>/v1/objectconnectedtickets/<oid>/tickets` | BC-319 (R1) | tests/assets.rs:205-236 | HIGH |
| BC-4.2.006 | `assets tickets <KEY> --status PROG` ambiguous → exit 64 `Ambiguous status` + both candidates | BC-320 (R1) | tests/assets.rs:1579-1684 | HIGH |
| BC-4.2.007 | `assets schema <TYPE-SUBSTR>` ambiguous → exit 64 `Ambiguous type` + NO per-type attribute fetch | BC-321 (R1) | tests/assets.rs:1695-1799 | HIGH |
| BC-4.2.008 | `assets tickets --open` filters `status.colorName != "green"` (client-side) | BC-314 | src/cli/assets.rs:303-321 | MEDIUM |
| BC-4.2.009 | `assets tickets --open` and `--status` clap conflict | BC-315 | tests/cli_smoke.rs:51-58 | HIGH |

### 4.3 Asset Enrichment (3 BCs: BC-4.3.001..003)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| **BC-4.3.001** | **Asset enrichment `resolved` HashMap MUST be keyed by `(workspace_id, oid)` not `oid` alone [MUST-FIX: NFR-R-E]** | BC-147 (R1); NFR-R-E | src/cli/issue/list.rs:440,446,449,456 | HIGH |
| BC-4.3.002 | `enrich_assets(client, &mut [LinkedAsset])` resolves ONLY assets with `id.is_some() && key.is_none() && name.is_none()` | BC-304; BC-323 (R1); BC-1137e (R4) | tests/cmdb_fields.rs:148-189 | HIGH |
| BC-4.3.003 | `LinkedAsset::display()` falls back to `#<id> (run 'jr init' to resolve asset names)` when only id present | BC-305 | src/types/assets/linked.rs | HIGH |

### 4.4 Asset Error Handling (3 BCs: BC-4.4.001..003)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-4.4.001 | `assets search` 5xx → exit 1 + `API error (500)` + no panic | BC-311; BC-1136 (R4) | tests/assets_errors.rs:21-64 | HIGH |
| BC-4.4.002 | `assets search` 401 → exit 2 + `Not authenticated` + `jr auth login` | BC-312 | tests/assets_errors.rs:67-113 | HIGH |
| BC-4.4.003 | `assets search` network drop → exit 1 + `Could not reach` | BC-313 | tests/assets_errors.rs:116-153 | HIGH |

---

## Section 5: Boards & Sprints (bc-5-boards-sprints.md) — 35 BCs cumulative; 17 individually-bodied

### 5.1 Board Commands (4 BCs: BC-5.1.001..004)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-5.1.001 | `client.list_boards(project, type)` GETs `/rest/agile/1.0/board` with query params | BC-401 | tests/board_commands.rs | HIGH |
| BC-5.1.002 | `board view --limit --all` clap conflict | BC-408 | tests/board_commands.rs:96-106 | HIGH |
| BC-5.1.003 | Auto-resolve board: list scrum boards for project, pick first | BC-410 | tests/sprint_commands.rs:23-61 | HIGH |
| BC-5.1.004 | `client.get_sprint_issues(sprintId, jql, limit, fields)` with `limit=Some(3)` returns 3 issues, `has_more=true` | BC-409 | tests/board_commands.rs:23-71 | HIGH |

### 5.2 Sprint Commands (8 BCs: BC-5.2.001..008)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-5.2.001 | `sprint list/current` errors on kanban boards with `"Sprint commands are only available for scrum boards"` | BC-402 | src/cli/sprint.rs:79-86 | HIGH |
| BC-5.2.002 | `sprint add --sprint ID` and `sprint add --current` are mutually exclusive (clap) | BC-403 | tests/cli_smoke.rs:116-123 | HIGH |
| BC-5.2.003 | `sprint add` requires `--sprint` or `--current` | BC-404 | tests/cli_smoke.rs:126-133 | HIGH |
| BC-5.2.004 | `MAX_SPRINT_ISSUES = 50` caps `sprint add` and `sprint remove` | BC-405 | src/cli/sprint.rs:35-61, 107 | MEDIUM |
| BC-5.2.005 | `sprint current` truncates to 30 by default; with `--all` returns full set; under-limit no hint | BC-406 | tests/sprint_commands.rs:63-180 | HIGH |
| BC-5.2.006 | `sprint current --all --limit N` clap conflict | BC-407 | tests/cli_smoke.rs:310-317 | HIGH |
| BC-5.2.007 | Sprint JSON output snapshot: sprint_add_response → `{"added": true, "issues": [...], "sprint_id": 100}` | BC-1113 (R4) | src/cli/snapshots/ | HIGH |
| BC-5.2.008 | Sprint JSON output: sprint_remove_response → `{"issues": [...], "removed": true}` (NO sprint_id) | BC-1114 (R4) | src/cli/snapshots/ | HIGH |

### 5.3 Team Column Parity (4 BCs: BC-5.3.001..004)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-5.3.001 | Team column appears IFF `team_field_id` configured AND at least one issue has populated team UUID | BC-1138a (R4) | tests/team_column_parity.rs:124, 181 | HIGH |
| BC-5.3.002 | Team column omitted when `team_field_id` not configured OR no issue has team UUID | BC-1138b (R4) | tests/team_column_parity.rs:220, 284 | HIGH |
| BC-5.3.003 | Team column shows `"UUID (name not cached — run 'jr team list --refresh')"` when cache is stale | BC-1138e (R4) | tests/team_column_parity.rs:341 | HIGH |
| BC-5.3.004 | `--output json` preserves team UUID without resolution (no cache lookup) | BC-1138f (R4) | tests/team_column_parity.rs:380 | HIGH |

### 5.4 API Layer (1 BC: BC-5.4.001)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-5.4.001 | `IssueFields::team_id` accepts string-UUID + object `{id}` form | BC-606 | src/types/jira/issue.rs:101-131 | HIGH |

---

## Section 6: Config & Cache (bc-6-config-cache.md) — 39 BCs cumulative; 29 individually-bodied

### 6.1 Configuration (13 BCs: BC-6.1.001..013)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-6.1.001 | Legacy `[instance]/[fields]` blocks migrate to `[profiles.default]` on first load | BC-901 | tests/migration_legacy.rs:93-143 | HIGH |
| BC-6.1.002 | Migration is idempotent: second load produces byte-identical file | BC-902 | tests/migration_legacy.rs:145-172 | HIGH |
| BC-6.1.003 | Migration write-back uses file-only baseline (no env overlay bleeds to disk) | BC-903; BC-153 (R1) | src/config.rs:240-264 | HIGH |
| BC-6.1.004 | `validate_profile_name` rejects: empty, >64 chars, non-`[A-Za-z0-9_-]`, reserved Windows names (case-insensitive) | BC-904; BC-904-R (R1) | src/config.rs:113-140 | HIGH |
| BC-6.1.005 | Profile-name validation runs at THREE boundaries: TOML key iteration, resolved active name, CLI flag | BC-152 (R1) | src/config.rs:269-282, 308-310 | HIGH |
| BC-6.1.006 | `resolve_active_profile_name` precedence: cli_flag → env_var → global.default_profile → "default" | BC-905; BC-905-R (R1) | src/config.rs | HIGH |
| BC-6.1.007 | `Config::load_with(cli_profile)` strict — errors with `"unknown profile: <X>; known: <list>"` | BC-906; BC-906-R (R1) | src/config.rs:319-328 | HIGH |
| BC-6.1.008 | `Config::load_lenient_with` skips active-profile existence check (used ONLY by `jr auth login`) | BC-907; BC-907-R (R1) | src/config.rs:285-289 | HIGH |
| BC-6.1.009 | Default `[defaults] output = "table"` | BC-908 | src/config.rs:63-74 | HIGH |
| BC-6.1.010 | `JR_BASE_URL` env completely overrides profile URL (test/power-user) | BC-909 | src/config.rs:351-353 | HIGH |
| BC-6.1.011 | `find_project_config()` walks up cwd to filesystem root looking for `.jr.toml`; returns first match | BC-911; BC-911-R (R1) | src/config.rs:340-353 | HIGH |
| BC-6.1.012 | User-facing migration message emitted to stderr exactly once per process | BC-151 (R1) | src/config.rs:262-265 | HIGH |
| BC-6.1.013 | `JR_PROFILE` env override for active profile; scrubbed by tests to prevent direnv pollution | BC-154 (R1) | tests/auth_profiles.rs:9-32 | HIGH |

### 6.2 Cache (15 BCs: BC-6.2.001..015)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-6.2.001 | `read_cache<T>` returns `Ok(None)` for NotFound; propagates other I/O errors | BC-1001; BC-1001-R (R1) | src/cache.rs:14-34 | HIGH |
| BC-6.2.002 | `read_cache<T>` returns `Ok(None)` AND stderr warning for parse failure | BC-1002; BC-1002-R (R1) | src/cache.rs:23-26 | HIGH |
| BC-6.2.003 | TTL check: `(Utc::now() - fetched_at).num_days() >= CACHE_TTL_DAYS (7)`; exactly 7 days is expired | BC-1003; BC-1003-R (R1) | src/cache.rs:7, 30-32 | HIGH |
| BC-6.2.004 | Per-profile cache directory: `~/.cache/jr/v1/<profile>/` | BC-1004 | src/cache.rs:7, 30, 76-78 | HIGH |
| BC-6.2.005 | `clear_profile_cache(name)` is no-op when directory doesn't exist (does NOT error) | BC-1005; BC-1005-R (R1) | src/cache.rs:82-88 | HIGH |
| BC-6.2.006 | `cmdb_fields.json` stores (id, name) tuples; old ID-only format → cache miss (graceful) | BC-1006 | src/cache.rs:237-247 | HIGH |
| BC-6.2.007 | `ProjectMeta` map cache `project_meta.json` keyed by project key; per-entry TTL | BC-1007 | src/cache.rs:105-143 | HIGH |
| BC-6.2.008 | `ResolutionsCache` drops resolutions without `id` on write + stderr warning | BC-1008 | src/cli/issue/workflow.rs:117-133 | HIGH |
| BC-6.2.009 | Cross-profile isolation: writing `prod` cache does NOT make `sandbox` cache visible | BC-1011 (R1) | src/cache.rs:389-406 | HIGH |
| BC-6.2.010 | `clear_profile_cache("prod")` does NOT delete `sandbox` data | BC-1012 (R1) | src/cache.rs:408-439 | HIGH |
| BC-6.2.011 | Corrupt cache files (garbage data + valid-JSON-wrong-shape) both return `Ok(None)` | BC-1013 (R1) | src/cache.rs:808-861 | HIGH |
| BC-6.2.012 | `write_project_meta` MERGES into existing map; corruption recovery → fresh start + stderr warning | BC-1014 (R1) | src/cache.rs:146-173 | HIGH |
| BC-6.2.013 | `write_object_type_attr_cache` MERGES into existing per-type map; same corruption recovery pattern | BC-1015 (R1) | src/cache.rs:318-354 | HIGH |
| BC-6.2.014 | Cache write is non-atomic (`std::fs::write`); crash mid-write leaves truncated file; read-side resilient | BC-1016 (R1) | src/cache.rs:42, 171, 351 | HIGH |
| BC-6.2.015 | Every cache reader/writer takes `profile: &str` as its first parameter (soft-fence convention) | NFR-SCA-2; ADV-P1-019 | src/cache.rs (all public functions) | HIGH |

### 6.3 Multi-Profile Fields — MUST-FIX (1 BC: BC-6.3.001)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| **BC-6.3.001** | **Per-profile `story_points_field_id` and `team_field_id` survive `Config::save_global()` and are read by ALL hot-path read sites [MUST-FIX: NFR-R-D — CRITICAL]** | NFR-R-D; NEW-INV-12; NEW-INV-143 | 14 sites in src/ | HIGH |

---

## Section 7: Output Rendering (bc-7-output-render.md) — 80 BCs cumulative; 34 individually-bodied

### 7.1 Table / JSON Output (5 BCs: BC-7.1.001..005)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-7.1.001 | `--output table` uses comfy-table renderer; `--output json` emits structured JSON | BC-1101 | src/output.rs | HIGH |
| BC-7.1.002 | `--no-color` and `NO_COLOR` env disable ANSI escape sequences | BC-1102 | src/main.rs:13-15 | HIGH |
| BC-7.1.003 | `--no-input` auto-enables when stdin is not a TTY (`IsTerminal` check) | BC-1103 | src/main.rs:18-23 | HIGH |
| BC-7.1.004 | Truncation hint emitted to stderr (NOT stdout); `--all` suppresses hint | BC-1110, BC-1111 | tests/sprint_commands.rs:97-100, 175-179 | HIGH |
| BC-7.1.005 | `--output json` error shape: `{"error": "<message>", "code": <exit>}` to stderr | BC-1208 | src/main.rs:34-49 | MEDIUM |

### 7.2 ADF Rendering (5 individually-bodied BCs: BC-7.2.001..005; 54 BCs cumulative including range-collapsed)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-7.2.001 | `text_to_adf("hello")` emits standard ADF doc shape | BC-1104 | src/adf.rs::tests | HIGH |
| BC-7.2.002 | `markdown_to_adf("**bold**")` emits marks `[{type:"strong"}]` on the text node | BC-1105 | src/adf.rs::tests | HIGH |
| BC-7.2.003 | ADF markdown round-trip covers: headings, lists, code blocks, blockquotes, tables, links | BC-1117 (R4) | src/snapshots/ | HIGH |
| BC-7.2.004 | ADF→text rendering: table render, code, headings preserved; lossy nodes silently dropped | BC-1106; BC-1116 (R4) | src/adf.rs::tests | HIGH |
| BC-7.2.005 | `markdown_to_adf("**bold text**")` body on wire: `marks: [{type: "strong"}]`; `text` is `"bold text"` NOT `"**bold text**"` | BC-1056 (R4) | tests/issue_commands.rs:647-687 | HIGH |
| BC-7.2.006..054 | Additional ADF contracts (range-collapsed from bc-7 body) [range-collapsed; not individually-bodied] | BC-1106..1117 | src/adf.rs::tests (69 tests) | HIGH |

### 7.3 Error Display (9 BCs: BC-7.3.001..009)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-7.3.001 | `extract_error_message` 7-step precedence chain (empty body → literal string FIRST; no None/status-derived path) | BC-1201-R (R1); ADV-P2-001 | src/api/client.rs:448-490 | HIGH |
| BC-7.3.002 | `errors{}` string values: `field: <value>`; non-string: `field: <serde_json::Value debug>` | BC-1201a (R1) | src/api/client.rs:469-475 | HIGH |
| BC-7.3.003 | `errors{}` iteration is alphabetically sorted (deterministic) | BC-1201b (R1) | src/api/client.rs:477 | HIGH |
| BC-7.3.004 | Empty `errorMessages[]` and empty `errors{}` fall through to raw body (no early exit) | BC-1201c (R1) | src/api/client.rs:459-466 | HIGH |
| BC-7.3.005 | `--output json` + empty 4xx body → stderr JSON `{"error": "<empty response body>", "code": <exit>}` (literal string, not status-derived) | BC-1208; ADV-P1-026; ADV-P2-001 | src/main.rs:34-49 | HIGH |
| BC-7.3.006 | `JrError::exit_code()` mapping | BC-1204 | src/error.rs:51-62 | HIGH |
| BC-7.3.007 | All API errors must suggest a next step (CLAUDE.md convention) | BC-1212 | tests/*_errors.rs | HIGH |
| BC-7.3.008 | stderr must NEVER contain `panic` | BC-1205 | 16+ tests | HIGH |
| BC-7.3.009 | Internal errors prefix with `Internal error:` | BC-1213 | src/error.rs:30-36 | MEDIUM |

### 7.4 JSON Output Shapes (12 BCs: BC-7.4.001..012)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-7.4.001 | move changed → `{"changed": true, "key": "TEST-1", "status": "In Progress"}` | BC-1104 (R4) | src/cli/issue/snapshots/ | HIGH |
| BC-7.4.002 | move unchanged → `{"changed": false, "key": "TEST-1", "status": "Done"}` | BC-1105 (R4) | src/cli/issue/snapshots/ | HIGH |
| BC-7.4.003 | assign changed → `{...assignee_account_id...}` — `assignee_account_id` is snake_case | BC-1106 (R4) | src/cli/issue/snapshots/ | HIGH |
| BC-7.4.004 | unassign → `{"assignee": null, "changed": true, "key": "TEST-1"}` — `assignee` is EXPLICIT null | BC-1108 (R4) | src/cli/issue/snapshots/ | HIGH |
| BC-7.4.005 | edit → `{"key": "TEST-1", "updated": true}` — minimal 2-key shape | BC-1109 (R4) | src/cli/issue/snapshots/ | HIGH |
| BC-7.4.006 | link → `{"key1": "TEST-1", "key2": "TEST-2", "linked": true, "type": "Blocks"}` | BC-1110 (R4) | src/cli/issue/snapshots/ | HIGH |
| BC-7.4.007 | unlink → `{"count": 2, "unlinked": true}`; no-match → `{"count": 0, "unlinked": false}` | BC-1111 (R4) | src/cli/issue/snapshots/ | HIGH |
| BC-7.4.008 | remote-link → `{"id": 10000, "key": "TEST-1", "self": <url>, "title": <title>, "url": <url>}` | BC-1112 (R4) | src/cli/issue/snapshots/ | HIGH |
| BC-7.4.009 | sprint add → `{"added": true, "issues": [...], "sprint_id": 100}` — sprint_id snake_case | BC-1113 (R4) | src/cli/snapshots/ | HIGH |
| BC-7.4.010 | sprint remove → `{"issues": [...], "removed": true}` — NO sprint_id | BC-1114 (R4) | src/cli/snapshots/ | HIGH |
| BC-7.4.011 | auth list table → 4 cols: NAME, URL, AUTH, STATUS; active prefix `* ` (asterisk-space) | BC-1115 (R4) | src/cli/snapshots/ | HIGH |
| BC-7.4.012 | `user view` hidden email → table shows em-dash `—`; JSON output shows explicit `null` | BC-1132j, BC-1132k (R4) | tests/user_commands.rs | HIGH |

### 7.5 Observability (3 BCs: BC-7.5.001..003)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-7.5.001 | Verbose request logging emits `[verbose] METHOD URL` + `[verbose] body: <utf8>` | BC-1405; BC-1405-R (R1) | src/api/client.rs:197-204, 274-279 | HIGH |
| BC-7.5.002 | `log_parse_failure_once` gate — parse failure logged at most once per (process, key) | BC-1109 | src/observability.rs | MEDIUM |
| BC-7.5.003 | `format_duration(seconds)` collapses to `30m` / `2h` / `1h30m` (hours+minutes only) | BC-1107 | src/duration.rs:52-60 | HIGH |

---

## Section X: Cross-Cutting Utilities (cross-cutting.md) — 130 BCs cumulative; 64 individually-bodied

### X.1 HTTP Client (10 BCs: BC-X.1.001..010)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.1.001 | Auth header injected on every API call via `req.header("Authorization", &self.auth_header)` at line 195 | BC-1410-R (R1); BC-1082 (R4) | tests/api_client.rs:14-40 | HIGH |
| BC-X.1.002 | `client.send(request)` retries 429 transparently; returns parsed response on 200 | BC-1402; BC-1083 (R4) | tests/api_client.rs:42-70 | HIGH |
| BC-X.1.003 | `client.send(request)` on exhausted 429 raises `JrError::ApiError{status: 429}` via `parse_error` | BC-1402-R (R1) | src/api/client.rs:184-253 | HIGH |
| BC-X.1.004 | `client.send(request)` requires `RequestBuilder::try_clone()` to succeed; non-cloneable bodies panic | BC-1402a (R1) | src/api/client.rs:191-194 | HIGH |
| BC-X.1.005 | `client.send_raw(request)` returns 429 to caller (NOT raises) after MAX_RETRIES=3; `expect(4)` pin | BC-1401; BC-1092 (R4) | tests/api_client.rs:424-444 | HIGH |
| BC-X.1.006 | `send_raw` 429-then-200 retries identically to `send`; caller sees 200 | BC-1091 (R4) | tests/api_client.rs:394-422 | HIGH |
| BC-X.1.007 | `send_raw` preserves 404 as response (NOT converted to Err); used by `jr api` raw passthrough | BC-1409-R (R1); BC-1090 (R4) | tests/api_client.rs:367-392 | HIGH |
| BC-X.1.008 | `send_raw` non-cloneable body returns `anyhow::Error` with explicit message (NOT panic) | BC-1402b (R1) | src/api/client.rs:267-272 | HIGH |
| BC-X.1.009 | 429-exhausted warning always emitted to stderr (not verbose-gated) | BC-1404; BC-1404-R (R1) | src/api/client.rs:233-237, 309-313 | HIGH |
| BC-X.1.010 | All HTTP methods inject auth header — no bypass | Pass 4 R4 §4.1 | src/api/client.rs | HIGH |

### X.2 Pagination (6 BCs: BC-X.2.001..006)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.2.001 | Offset pagination: `startAt`/`maxResults` + `total` for issue comments, projects, worklogs | BC-1406, BC-1407-R (R1) | src/api/pagination.rs | HIGH |
| BC-X.2.002 | Cursor pagination via `nextPageToken` for JQL search | BC-1406 | src/api/pagination.rs | HIGH |
| BC-X.2.003 | ServiceDeskPage pagination (JSM service desks) | BC-1406 | src/api/pagination.rs | HIGH |
| BC-X.2.004 | `AssetsPage::is_last` accepts bool or string-encoded bool (custom deserializer) | BC-317 (R1) | src/api/pagination.rs | HIGH |
| BC-X.2.005 | User pagination advances `startAt` by REQUESTED `maxResults` (NOT by returned count) | BC-702; BC-1119 (R4) | tests/user_pagination.rs:202-247 | HIGH |
| BC-X.2.006 | `USER_PAGINATION_SAFETY_CAP = 1500` (15 pages × 100); emits stderr `"hit pagination safety cap"`; exits 0 | BC-1124, BC-1125 (R4) | tests/user_pagination.rs:459-520 | HIGH |

### X.3 Error Handling (8 BCs: BC-X.3.001..008)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.3.001 | Network drop → `Could not reach <host>; check your connection` exit 1 | BC-1206 | tests/issue_list_errors.rs:320-360 | HIGH |
| BC-X.3.002 | 401 → `Not authenticated` + `jr auth login` exit 2 (universal across all subcommands) | BC-1207 | 6+ test files | HIGH |
| BC-X.3.003 | 5xx → `API error (<status>)` + extract_error_message(body) + exit 1 | BC-1210 | All *_errors.rs files | HIGH |
| BC-X.3.004 | 400 with field-specific Jira error → stderr formatted as `field: message` (sorted alphabetically) | BC-1211 | tests/issue_resolution.rs:124-158 | HIGH |
| BC-X.3.005 | 401 + scope-mismatch (case-insensitive) → InsufficientScope; 403 with substring NOT dispatched | BC-015..018; BC-1085..1088 (R4) | tests/api_client.rs:99-255 | HIGH |
| BC-X.3.006 | Ctrl+C exits 130 with `Interrupted` handling | BC-1209 | src/main.rs:264 | MEDIUM |
| BC-X.3.007 | Error messages must suggest next step (CLAUDE.md convention, universal) | BC-1212 | Multiple integration tests | HIGH |
| BC-X.3.008 | stderr must NEVER contain `panic` (universal) | BC-1205 | 16+ negative assertion tests | HIGH |

### X.4 Rate Limiting (9 BCs: BC-X.4.001..009)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.4.001 | MAX_RETRIES = 3 (initial + 3 = 4 total calls); `expect(4)` pin | BC-1401-R (R1) | tests/api_client.rs:424-444 | HIGH |
| BC-X.4.002 | `Retry-After` header parsed as u64 INTEGER ONLY — HTTP-date format NOT supported | BC-1403-R (R1) | src/api/rate_limit.rs:14-18 | HIGH |
| BC-X.4.003..008 | Additional rate-limiting BCs [range-collapsed; not individually-bodied] | BC-701..708 | src/api/rate_limit.rs | HIGH |
| BC-X.4.009 | `MAX_RETRY_AFTER_SECS = 60` cap — Retry-After exceeding 60s prints warning and aborts retry [PROPOSED FIX-IN-PHASE-3] | ADV-P1-029; NFR-R-NEW-1 | src/api/rate_limit.rs (proposed) | HIGH |

### X.5 Worklogs & Duration (10 BCs: BC-X.5.001..010)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.5.001 | `client.add_worklog(key, seconds, message)` POSTs `/issue/<key>/worklog`; returns Worklog; accepts 201 | BC-501 | tests/worklog_commands.rs:8-26 | HIGH |
| **BC-X.5.002** | **`client.list_worklogs(key)` paginates via `/issue/<key>/worklog` [MUST-FIX: NFR-R-A — HIGH]** | BC-502; NFR-R-A | src/api/jira/worklogs.rs:25-30 | HIGH |
| BC-X.5.003 | `worklog list` 5xx → exit 1 + `API error (500)` | BC-503 | tests/worklog_commands.rs:55-93 | HIGH |
| BC-X.5.004 | `worklog list` 401 → exit 2 + `Not authenticated` + `jr auth login` | BC-504 | tests/worklog_commands.rs:95-120 | HIGH |
| BC-X.5.005 | `parse_duration("1w2d3h30m", 8, 5)` accepts combined units; returns total seconds | BC-505 | src/duration.rs::tests | HIGH |
| BC-X.5.006 | `parse_duration` is case-insensitive (input lowercased first) | BC-506 | src/duration.rs:6 | HIGH |
| BC-X.5.007 | `parse_duration("")` errors `Duration cannot be empty` | BC-507 | src/duration.rs:7-9 | HIGH |
| BC-X.5.008 | `parse_duration("5")` errors `Number without unit` | BC-508 | src/duration.rs:38-42 | HIGH |
| BC-X.5.009 | `worklog add` hardcodes 8h/day, 5d/week (`parse_duration(dur, 8, 5)` at `cli/worklog.rs:32`) | BC-1014 (R4) | src/cli/worklog.rs:32 | HIGH |
| BC-X.5.010 | Duration proptest: `valid_single_units_always_parse`; `combined_units_always_parse`; `garbage_input_never_panics`; `format_roundtrip` | BC-1099..BC-1102 (R4) | src/duration.rs:128-157 | HIGH |

### X.6 Teams (4 BCs: BC-X.6.001..004)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.6.001 | `client.get_org_metadata(hostname)` POSTs GraphQL `tenantContexts` query to `/gateway/api/graphql` | BC-601 | tests/team_commands.rs:8-26 | HIGH |
| BC-X.6.002 | `client.list_teams(orgId)` GETs `/gateway/api/public/teams/v1/org/<orgId>/teams` | BC-602 | tests/team_commands.rs:28-46 | HIGH |
| BC-X.6.003 | `team list` 5xx → exit 1; 401 → exit 2; standard error paths | BC-603, BC-604 | tests/team_commands.rs:62- | HIGH |
| BC-X.6.004 | `team list` cache-first (7d TTL); `--refresh` forces re-fetch | BC-605 | src/cache.rs | MEDIUM |

### X.7 Users (6 BCs: BC-X.7.001..006)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.7.001 | `user search Q` GETs `/rest/api/3/user/search?query=Q` | BC-701 | tests/user_commands.rs | HIGH |
| BC-X.7.002 | `user list --project P` calls `/rest/api/3/user/assignable/multiProjectSearch?projectKeys=P` | BC-704 | tests/all_flag_behavior.rs:260- | HIGH |
| BC-X.7.003 | `user list` (default, no --all) uses single-call legacy path; no startAt/maxResults params | BC-705 | tests/all_flag_behavior.rs:271-275 | HIGH |
| BC-X.7.004 | Duplicate display names + `--no-input` → exit non-zero; stderr shows emails + accountIds + duplicate name | BC-706..BC-708 | tests/duplicate_user_disambiguation.rs | HIGH |
| BC-X.7.005 | `user view <id>` → 404 → friendly `"User with accountId '<id>' not found"` exit 64 | BC-1132i (R4) | tests/user_commands.rs | HIGH |
| BC-X.7.006 | `user search --all` advances startAt by REQUESTED maxResults (JRACLOUD-71293 workaround) | BC-1119 (R4) | tests/user_pagination.rs:202-247 | HIGH |

### X.8 Projects & Queues (5 BCs: BC-X.8.001..005)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.8.001 | `project_exists(key)` → true on 200; false on 404 | BC-801 | tests/input_validation.rs:9-42 | HIGH |
| BC-X.8.002 | `get_project_statuses(key)` → 404 → `JrError::ApiError{status: 404}` | BC-802 | tests/input_validation.rs:233-253 | HIGH |
| BC-X.8.003 | `get_or_fetch_project_meta(client, key)` caches by project key with 7d TTL | BC-804 | tests/project_meta.rs:24-67 | HIGH |
| BC-X.8.004 | `require_service_desk` errors for software project: "Jira Software project" + "Queue commands require" | BC-805 | tests/project_meta.rs:99-126 | HIGH |
| BC-X.8.005 | `list_projects` paginates via `startAt`; filter via `typeKey` query param | BC-1133d, BC-1133e (R4) | tests/project_commands.rs:1-323 | HIGH |

### X.9 JQL Utilities (4 BCs: BC-X.9.001..004)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.9.001 | `escape_value` proptest: for any printable Unicode up to 100 chars, output has NO unescaped quote | BC-1094 (R4) | src/jql.rs:383-394 | HIGH |
| BC-X.9.002 | `validate_duration("4w2d")` → Err; single unit `"7d"` → Ok | BC-131 (R1) | src/jql.rs:16-34 | HIGH |
| BC-X.9.003 | `validate_date` → `YYYY-MM-DD` format only; invalid → `JrError::UserError` | BC-132 (R1) | src/jql.rs | HIGH |
| BC-X.9.004 | `strip_order_by` removes ORDER BY clause before count calls and paren-wrapping | BC-102, BC-125 (R1) | src/jql.rs | HIGH |

### X.10 Partial-Match (3 BCs: BC-X.10.001..003)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.10.001 | `partial_match` with single-substring → `Ambiguous` (NOT Exact); never auto-resolves | BC-105 context | src/partial_match.rs | HIGH |
| BC-X.10.002 | `partial_match(s, &candidates)` proptest: exact match always found; never panics; empty candidates → None | BC-1095..BC-1097 (R4) | src/partial_match.rs:153-198 | HIGH |
| BC-X.10.003 | Duplicate candidates → `MatchResult::ExactMultiple(name)` with `name.to_lowercase() == input.to_lowercase()` | BC-1098 (R4) | src/partial_match.rs:182-198 | HIGH |

### X.11 Build-Time (5 BCs: BC-X.11.001..005)

| L3 BC ID | Summary | Pass 3 BC ID | Source | Confidence |
|---|---|---|---|---|
| BC-X.11.001 | `build.rs` reads `JR_BUILD_OAUTH_CLIENT_ID` + `_SECRET` env vars | BC-1301 | build.rs | HIGH |
| BC-X.11.002 | Unix → `/dev/urandom` for 32-byte XOR key; Windows → inline `BCryptGenRandom` FFI | BC-1302 | build.rs | HIGH |
| BC-X.11.003 | Non-unix/non-windows → `compile_error!` | BC-1303 | build.rs | HIGH |
| BC-X.11.004 | Unset build vars → `EMBEDDED_*` constants are `None`; BYO/prompt path proceeds | BC-1304 | build.rs; src/api/auth_embedded.rs::tests | HIGH |
| BC-X.11.005 | `proptest-regressions/jql.txt` pinned regression seed for `escape_value("")` | BC-1103 (R4) | proptest-regressions/jql.txt | HIGH |

---

## MUST-FIX Register (4 items)

| L3 BC ID | NFR Source | Severity | Site | Phase 3 Routing |
|---|---|---|---|---|
| **BC-6.3.001** | NFR-R-D | CRITICAL | 14 sites `config.global.fields.*` | FIX-IN-PHASE-3 |
| **BC-X.5.002** | NFR-R-A | HIGH | `src/api/jira/worklogs.rs:25-30` | FIX-IN-PHASE-3 |
| **BC-3.4.001** | NFR-R-B | HIGH | `src/cli/issue/workflow.rs:636` | FIX-IN-PHASE-3 |
| **BC-4.3.001** | NFR-R-E | HIGH | `src/cli/issue/list.rs:446,449,456` | FIX-IN-PHASE-3 |

---

## Coverage Statistics

| Section | BC Count (cumulative) | Individually-bodied |
|---|---|---|
| 1: Auth & Identity | 57 | 46 |
| 2: Issue Read | 91 | 49 |
| 3: Issue Write | 77 | 48 |
| 4: Assets & CMDB | 32 | 22 |
| 5: Boards & Sprints | 35 | 17 |
| 6: Config & Cache | 39 | 29 |
| 7: Output Rendering | 80 | 34 |
| X: Cross-Cutting | 130 | 64 |
| **Total** | **541** | **309** |

Plus 1 NEW BC (BC-X.4.009 / ADV-P1-029) = **542 cumulative total**.

**Note**: Cumulative total (542) ≠ individually-bodied count (309). The difference (233) comprises range-collapsed BCs that exist in the cumulative claim but are not individually headlined in body files. This is by design — range-collapsed BCs trace to Pass 3 source material but were not individually expanded. The 4 MUST-FIX BCs are included in the individually-bodied count.

---

## Pass 3 BC ID Mapping Table (key entries)

| Pass 3 BC ID | L3 BC ID | Notes |
|---|---|---|
| BC-001..012 | BC-1.1.001..012 | Auth core (body 1.1) |
| BC-013-R..014-R | BC-1.2.013..014 | Profile lifecycle |
| BC-019..022-R | BC-1.3.019..022 | Embedded OAuth app |
| BC-101..BC-109 | BC-2.1.001..BC-2.2.027 | Issue read broad |
| BC-201..225 | BC-3.1.001..BC-3.7.004 | Issue write |
| BC-301..315 | BC-4.1.001..BC-4.4.003 | Assets broad |
| BC-316..324 | BC-4.2.001..BC-4.4.003 | Assets R1 |
| BC-401..410 | BC-5.1.001..BC-5.2.008 | Boards/sprints broad |
| BC-501..508 | BC-X.5.001..008 | Worklogs |
| BC-601..606 | BC-X.6.001..004, BC-5.4.001 | Teams, team_id deserialization |
| BC-701..708 | BC-X.7.001..006, BC-X.4.001..002 | Users, rate limiting |
| BC-801..805 | BC-X.8.001..005 | Projects/queues |
| BC-901..909 | BC-6.1.001..010 | Config |
| BC-1001..1016 | BC-6.2.001..014 | Cache |
| R1 BC-1201-R..d | BC-7.3.001..004 | extract_error_message |
| R4 BC-1104..1117 | BC-7.2..BC-7.4 | JSON output shapes, ADF |
| R4 BC-1119..1125 | BC-X.7.006, BC-X.2.005..006 | User pagination |
| R4 BC-1126..1132 | BC-3.7.001..004, BC-X.7 | Remote links, user commands |
| R4 BC-1133..1139 | BC-X.8.005, BC-6.1, BC-1.1.012 | Projects, config errors |
| R4 BC-1140..1178 | BC-1.3..1.5 | Auth OAuth state machine |
| R4 BC-1138a..f | BC-5.3.001..004 | Team column parity |
| R4 NFR-R-D | BC-6.3.001 | MUST-FIX CRITICAL |
| R4 NFR-R-A | BC-X.5.002 | MUST-FIX HIGH |
| R4 NFR-R-B | BC-3.4.001 | MUST-FIX HIGH |
| R4 NFR-R-E | BC-4.3.001 | MUST-FIX HIGH |

---

## Traceability Gaps

| Pass 3 BC ID | Disposition |
|---|---|
| BC-105 (partial_match single-substring) | Absorbed into BC-X.10.001 |
| BC-314 (--open assets color filter) | Absorbed into BC-4.2.008 |
| BC-505 (parse_duration combined units) | Absorbed into BC-X.5.005 |
| BC-1099..1103 (duration proptests) | Absorbed into BC-X.5.010 |
| BC-1103 (proptest regression seed) | Absorbed into BC-X.11.005 |
| BC-152..154 (config validation points) | Absorbed into BC-6.1.004..006 |
| BC-1201-R variants (4 sub-BCs) | Absorbed into BC-7.3.001..004 |
| R4 BC-1402a,1402b (try_clone semantics) | Absorbed into BC-X.1.004, BC-X.1.008 |

**Unresolved gaps**: 0 — all Pass 3 BCs are either directly mapped or absorbed into a parent L3 contract.
