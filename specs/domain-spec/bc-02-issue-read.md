---
title: "BC-02: Issue Read (list/view/comments/changelog)"
version: "1.0.0"
snapshot_sha: "dea166471e22eff55974d7675593469b37048c5f"
traces_to: "README.md"
source_passes: "Pass 2 broad §2a.2 Jira + §2b.1 + R5 §3.2 (NEW-INV-216..244) + Pass 8 §2.2 BC#2"
entity_count: 32
invariant_count: 28
bc_count: 92
risk_level: HIGH
---

# BC-02: Issue Read

Covers the read side of the Issue domain: `jr issue list`, `jr issue view`, `jr issue comments`, and `jr issue changelog`. Largest BC by behavioral contract count (91 BCs, ~2,500 LOC).

---

## §1 Ubiquitous Language

| Term | Definition |
|------|-----------|
| **JQL composition** | The multi-stage process of building a JQL string from user flags (project scope, `--open`, date filters, `--asset` clause, `--status`, `--assignee`, `--reporter`, `--team`, `--recent`, ORDER BY). |
| **`--open` predicate** | For Jira issues: JQL `statusCategory != Done`. NOT colour-based. |
| **Cursor pagination** | Used for JQL search (`POST /search/jql` → `CursorPage<Issue>`). Distinct from offset pagination. |
| **Offset pagination** | Used for comments and changelog (`GET /comment`, `GET ?expand=changelog`). |
| **N+1 closed** | Asset enrichment in `issue list` uses `join_all` (concurrent), NOT N sequential calls. Verified at R5. |
| **Team column gating** | Team-name resolution only runs in Table mode AND when `team_field_id` is set. JSON mode skips cache reads. |
| **`AuthorNeedle`** | Smart constructor for `--author` on changelog: 12+ chars with a digit → `AccountId`; else `NameSubstring`. |
| **`LoweredStr`** | Module-private newtype in `changelog.rs`. Any `LoweredStr` is guaranteed lowercased at construction. |
| **sprint-aware dispatch** | `issue list` branches based on board_id + board_type + active sprint to compose the JQL base. |
| **changelog row** | One `ChangelogItem` in the table (a single field change). One `ChangelogEntry` can produce multiple rows. |
| **`me` keyword** | In `--assignee me`, `--reporter me`, `--author me`: resolves to `client.get_myself()` before query. |
| **CMDB enrichment** | Per-row asset lookup during `issue list`: `extract_linked_assets_per_field` → concurrent `get_asset` calls. |

---

## §2 Entities

Source: Pass 2 broad §2a.2 Jira context rows.

| Entity | Module | Key Fields | Notes |
|--------|--------|-----------|-------|
| `Issue` | `types/jira/issue.rs:8-12` | `key: String`, `fields: IssueFields` | Aggregate root. Read-mostly. Never persisted locally. Created via `CreateIssueResponse` (key only). |
| `IssueFields` | `types/jira/issue.rs:56-80` | `summary`, `description: Option<Value>` (ADF), `status`, `issue_type`, `priority`, `assignee`, `reporter`, `project`, `created`, `updated`, `resolution`, `components`, `fix_versions`, `labels`, `parent`, `issuelinks`, `extra: HashMap<String, Value>` (flatten) | The open container `extra` carries all custom fields: story points, team UUID, CMDB asset arrays. |
| `IssueType` | `types/jira/issue.rs:158-161` | `name: String` | Read-only enrichment of `IssueFields`. |
| `IssueProject` | `types/jira/issue.rs:168-172` | `key: String`, `name: Option<String>` | Inline projection from `issue.fields.project`. Narrower than `Project`. |
| `Status` | `types/jira/issue.rs:145-150` | `name: String`, `status_category: Option<StatusCategory>` | Read-only on `IssueFields`. |
| `StatusCategory` | `types/jira/issue.rs:152-156` | `name: String`, `key: String` | `key ∈ {"new","indeterminate","done"}`. Used by `--open` via JQL; colour is NOT first-class here. |
| `Priority` | `types/jira/issue.rs:163-166` | `name: String` | — |
| `Resolution` | `types/jira/issue.rs:174-185` | `id: Option<String>`, `name: String`, `description: Option<String>` | Tolerates both `{name}` (from issue.fields) and `{id,name,description}` (from `/resolution`). |
| `Component` | `types/jira/issue.rs:187-190` | `name: String` | Intentionally narrow. |
| `Version` | `types/jira/issue.rs:192-198` | `name: String`, `released: Option<bool>`, `release_date: Option<String>` | Renamed `fixVersions`. |
| `Comment` | `types/jira/issue.rs:218-226` | `id: Option<String>`, `body: Option<Value>` (ADF), `author: Option<User>`, `created: Option<String>`, `properties: Vec<EntityProperty>` | Separate root. ADF rendered via `adf.rs`. |
| `EntityProperty` | `types/jira/issue.rs:212-216` | `key: String`, `value: Value` | Currently only `sd.public.comment` key consumed (JSM internal flag). |
| `ChangelogEntry` | `types/jira/changelog.rs:6-16` | `id: String`, `author: Option<User>`, `created: String`, `items: Vec<ChangelogItem>` | Separate root. `author` is `None` for automation/post-functions. |
| `ChangelogItem` | `types/jira/changelog.rs:18-31` | `field: String`, `fieldtype: String`, `from: Option<String>`, `from_string: Option<String>`, `to: Option<String>`, `to_string: Option<String>` | One row in changelog table. Nullable `from_string`/`to_string` distinct from absent fields (snapshot-pinned). |
| `User` | `types/jira/user.rs:3-12` | `account_id: String`, `display_name: String`, `email_address: Option<String>`, `active: Option<bool>` | Universal foreign-key target. Assignee, reporter, comment author, changelog author, worklog author. |
| `ParentIssue` | `types/jira/issue.rs:14-18` | `key: String`, `fields: Option<LinkedIssueFields>` | — |
| `LinkedIssueFields` | `types/jira/issue.rs:20-23` | `summary: Option<String>` | Shared by `ParentIssue.fields` and `LinkedIssue.fields`. |
| `IssueLink` | `types/jira/issue.rs:25-34` | `id: String`, `link_type: IssueLinkType`, `inward_issue: Option<LinkedIssue>`, `outward_issue: Option<LinkedIssue>` | — |
| `LinkedIssue` | `types/jira/issue.rs:36-40` | `key: String`, `fields: Option<LinkedIssueFields>` | — |
| `IssueLinkType` | `types/jira/issue.rs:42-48` | `id: Option<String>`, `name: String`, `inward: Option<String>`, `outward: Option<String>` | — |
| `IssueLinkTypesResponse` | `types/jira/issue.rs:50-54` | `issue_link_types: Vec<IssueLinkType>` | — |
| `Project` | `types/jira/project.rs:3-7` | `key: String`, `name: String` | Lightweight read-only projection. |
| `ProjectSummary` | `types/jira/project.rs:9-16` | `key: String`, `name: String`, `project_type_key: String`, `lead: Option<ProjectLead>` | Discovery shape. Drives `cache::ProjectMeta`. `project_type_key == "service_desk"` → JSM. |
| `ProjectLead` | `types/jira/project.rs:18-24` | `display_name: String`, `account_id: String` | — |
| `ServiceDesk` (JSM) | `types/jsm/servicedesk.rs:3-10` | `id: String`, `project_id: String`, `project_name: String` | Read-only. `service_desk_id` cached in `ProjectMeta`. |
| `Queue` (JSM) | `types/jsm/queue.rs:3-11` | `id: String`, `name: String`, `jql: Option<String>`, `fields: Option<Vec<String>>`, `issue_count: Option<u64>` | Queue's `jql` field is re-issued as a search. |
| `QueueIssueKey` (JSM) | `types/jsm/queue.rs:17-20` | `key: String` | Thin-projection bridge: JSM queue → full issue via standard search. |
| `TenantContext` | `types/jira/team.rs:17-23` | `org_id: String`, `cloud_id: String` | Returned by GraphQL `tenantContexts` (ADR-0005). |
| `TeamEntry` | `types/jira/team.rs:26-32` | `team_id: String`, `display_name: String` | Cached as `CachedTeam`. |
| `TeamsResponse` | `types/jira/team.rs:34-39` | `entities: Vec<TeamEntry>`, `cursor: Option<String>` | Cursor-paginated. |
| `GraphqlResponse<T>` | `types/jira/team.rs:5-8` | `data: Option<T>` | Generic GraphQL envelope. |
| `AuthorNeedle` (smart constructor) | `cli/issue/changelog.rs` | Variants: `AccountId(String)`, `NameSubstring(String)` | Classifies `--author` value: 12+ chars with ASCII digit + all ASCII alphanumeric → AccountId; else NameSubstring. |

---

## §3 Value Objects & Enums

- **`DEFAULT_LIMIT = 30`** (`cli/mod.rs:740`): default page size for all list operations.
- **`IssueFields::story_points(field_id)`**: returns `Option<f64>`. Numeric coercion only; non-numeric present value → `None`.
- **`IssueFields::team_id(field_id, verbose)`**: accepts string-UUID or `{"id": "<string>"}` Atlas Teams object. Object with non-string `id` → `None` + once-per-process verbose warning.
- **`LoweredStr`**: private newtype in `changelog.rs`. Only constructable via `LoweredStr::new(s)` which lowercases. Guards `author_matches` from re-normalization.
- **`NULL_GLYPH = "—"`** (em-dash U+2014): per-file const in `changelog.rs:13`, also in `cli/assets.rs`. Em-dash is the table null sentinel.
- **`DEFAULT_FIELDS`**: 16-field list in exact order for `search_issues` (`summary, status, issuetype, priority, assignee, reporter, project, description, created, updated, resolution, components, fixVersions, labels, parent, issuelinks`). Order is load-bearing (pinned by test BC-1063).

---

## §4 Operations

| Command | HTTP | Notes |
|---------|------|-------|
| `issue list` | `POST /rest/api/3/search/jql` (cursor-paginated) | Validates dates/durations BEFORE first HTTP call. Optionally enriches CMDB fields (cache or API). Composes full JQL pipeline. |
| `issue view <key>` | `GET /rest/api/3/issue/<key>` | Optional CMDB enrichment when asset fields present. ADF body → text via `adf::adf_to_text`. |
| `issue comments <key>` | `GET /rest/api/3/issue/<key>/comment` (offset-paginated) | Renders body via `adf::adf_to_text`. |
| `issue changelog <key>` | `GET /rest/api/3/issue/<key>?expand=changelog` (offset-paginated) | ALL filters (field, author, date) applied CLIENT-SIDE — no server-side filter support. |
| `issue transitions <key>` | `GET /rest/api/3/issue/<key>/transitions` | Read-only; drives `issue move`. |
| `issue resolutions` | Cache-first; on miss: `GET /rest/api/3/resolution` | 7-day TTL. |
| `issue link-types` | `GET /rest/api/3/issueLinkType` | Read-only. |
| `issue open <key>` | No HTTP; opens browser | Currently uses `client.base_url()` — broken for OAuth profiles (NFR-R-B). Should use `client.instance_url()`. |
| `board list` / `board view` | `GET /rest/agile/1.0/board[/<id>/issue]` | — |
| `project list` | `GET /rest/api/3/project/search` | — |
| `queue list` / `queue view` | `GET /servicedeskapi/servicedesk/<id>/queue[/<qid>/issue]` → followup search | Two-step thin-projection pattern. |
| `me` | `GET /rest/api/3/myself` | 3-row table. |

---

## §5 Business Rules & Invariants

| ID | Invariant | Source |
|----|----------|--------|
| INV-READ-001 | `--open` for Jira issues uses JQL `statusCategory != Done` (category key, NOT colour, NOT status name). Instance-agnostic. | `cli/issue/list.rs:303,308,625` |
| INV-READ-002 | Date/duration validators run before any HTTP call. A bad `--created-after` does not cost a network request. | `cli/issue/list.rs:90-114` |
| INV-READ-003 | `--asset KEY` auto-enables `--assets` display column: `let show_assets = show_assets || asset_key.is_some()`. | `cli/issue/list.rs:87` |
| INV-READ-004 | Empty JQL guard: `issue list` with no project scope and no `--jql` and no board produces a user error. | `cli/issue/list.rs:344-352`, NEW-INV-04 |
| INV-READ-005 | `--asset` requires CMDB fields to exist on the Jira instance. If CMDB field discovery returns empty, the asset clause cannot be built → user error. | `cli/issue/list.rs:170-178`, NEW-INV-01 |
| INV-READ-006 | Sprint-aware dispatch: scrum board with active sprint → `sprint = N ORDER BY rank ASC`; scrum with no active sprint → **silent degradation** to `project = X ORDER BY updated DESC` (NO warning). Kanban → `statusCategory != Done ORDER BY rank ASC`. | `cli/issue/list.rs:283-310`, NEW-INV-219,220 |
| INV-READ-007 | Project-key JQL injection is uniformly prevented: ALL three `project =` clause emit sites in `handle_list` call `jql::escape_value(pk)`. | `cli/issue/list.rs:302,309,333`, NEW-INV-221 |
| INV-READ-008 | Board type other than `"scrum"` falls to the kanban arm (including any future Jira board types). String compare is case-insensitive. | `cli/issue/list.rs`, NEW-INV-222 |
| INV-READ-009 | Team column gating: `matches!(output_format, OutputFormat::Table) && let Some(field_id) = team_field_id`. JSON mode ALWAYS skips team-cache read (zero filesystem I/O). | `cli/issue/list.rs:501`, NEW-INV-223 |
| INV-READ-010 | Team cache is READ-ONLY from `issue list`. Populating the team cache requires `jr team list`. | `cli/issue/list.rs:514-517`, NEW-INV-224 |
| INV-READ-011 | `build team_map once, query per-row` — O(1) per-row resolution against HashMap. Source comment documents this as a deliberate optimization. | `cli/issue/list.rs:493-499`, NEW-INV-225 |
| INV-READ-012 | Asset enrichment dedup key is `(workspace_id, object_id)` — workspace-qualified. But the CURRENT `resolved` HashMap drops the workspace qualifier, keyed only by `oid` (NFR-R-E). | `cli/issue/list.rs:406,446`, NEW-INV-227,229 |
| INV-READ-013 | Asset enrichment uses `futures::future::join_all` — M concurrent HTTP calls (NOT N×K sequential). Performance contract: completes in ≈max(individual latency). 429-storm risk when M is large. | `cli/issue/list.rs:445`, NEW-INV-228 |
| INV-READ-014 | Workspace ID is fetched LAZILY: only if `to_enrich` is non-empty. Zero-cost for issues without asset fields. | `cli/issue/list.rs:415`, NEW-INV-230 |
| INV-READ-015 | JSON enrichment back-injection depends on `extract_linked_assets` being deterministic and idempotent. Positional index matching between table and JSON paths is an architectural fragility. | `cli/issue/list.rs:472-487`, NEW-INV-231 |
| INV-READ-016 | Changelog filters (field, author, date) are ALL client-side. No server-side support. For heavily-logged issues, 100+ API calls + full materialization before filtering. | `api/jira/issues.rs`, NEW-INV-232 |
| INV-READ-017 | `--author me` issues `client.get_myself()` BEFORE field validation. A user with an empty `--field ""` makes a `/myself` call before the early-exit. | `cli/issue/changelog.rs:50-79`, NEW-INV-233 |
| INV-READ-018 | Empty `--author` / `--field` values are explicitly rejected. `str::contains("")` would always match; empty-needle bypass prevention is intentional. | `cli/issue/changelog.rs:44-49,65-79`, NEW-INV-234 |
| INV-READ-019 | `changelog --limit N` means N TABLE ROWS, not N changelog entries. A single entry with 10 field changes = 10 rows. Last entry may be partially trimmed if truncation lands mid-entry. | `cli/issue/changelog.rs:286-304`, NEW-INV-235 |
| INV-READ-020 | `AuthorNeedle` 12-char boundary: ≥12 chars with ASCII digit + all ASCII alphanumeric → AccountId. Non-ASCII letters force NameSubstring path. | `cli/issue/changelog.rs`, NEW-INV-237,238 |
| INV-READ-021 | `LoweredStr` private newtype is the ONLY compile-time-guaranteed lowercased string in the codebase. Refactoring to `String` silently drops the invariant. | `cli/issue/changelog.rs:145-171`, NEW-INV-239 |
| INV-READ-022 | Changelog sort is chronological (parsed `DateTime`), not lexicographic. Mixed UTC-offset formats are handled; unparseable → lexicographic fallback. | `cli/issue/changelog.rs`, NEW-INV-240,241 |
| INV-READ-023 | `IssueFields::story_points(field_id)` returns `None` for any non-numeric value (no coercion). Pinned by unit test. | `types/jira/issue.rs:83-85` |
| INV-READ-024 | `IssueFields::team_id` accepts string-UUID and `{"id":"<string>"}` shapes. Rejects `{"id":<number>}` → `None` + verbose warning. | `types/jira/issue.rs:101-131`, Pass 2 INV-17 |
| INV-READ-025 | Status validation path decision: project-scoped validation uses `extract_unique_status_names` (flattens from issue-types-grouped response); global uses `get_all_statuses` directly (already flat). | `cli/issue/list.rs`, NEW-INV-216,217 |
| INV-READ-026 | `issue open <key>` URL is currently composed via `client.base_url()` — broken for OAuth profiles (returns `api.atlassian.com/ex/jira/<cloudId>`). Fix: use `client.instance_url()`. This is NFR-R-B (HIGH). | `cli/issue/workflow.rs:636` |
| INV-READ-027 | `DEFAULT_FIELDS` list (16 fields in exact order) governs what `search_issues` requests from Jira. Order matters. Pinned by BC-1063. | `tests/issue_commands.rs:967-1022` |
| INV-READ-028 | `Resolution` struct tolerates both `{name}` only (from `issue.fields.resolution`) and `{id,name,description}` (from `/resolution` endpoint). Single struct handles both shapes via optional fields. | `types/jira/issue.rs:174-185` |

---

## §6 Aggregate Boundaries

- **`Issue`** is the aggregate root. Owns `IssueFields` (1:1). Comments, transitions, changelog, and worklogs are associated (own endpoint, own struct), not aggregated.
- **`Comment`** is its own root (separate endpoint, `EntityProperty[]` for JSM flag).
- **`ChangelogEntry`** is its own root (one entry → multiple `ChangelogItem` rows).
- **`ProjectSummary → ProjectMeta`**: discovery path; `ProjectMeta` is the cached projection (drives JSM detection).

---

## §7 Cross-Context Dependencies

| Depends on | Reason |
|-----------|--------|
| **Assets/CMDB (BC-04)** | `issue list` and `issue view` enrich with linked-asset data via `extract_linked_assets_per_field` + `client.get_asset`. |
| **Configuration (BC-06)** | Reads `story_points_field_id`, `team_field_id` (currently via legacy `config.global.fields.*` — NFR-R-D), `board_id`, `project`. |
| **Cache (BC-06)** | Reads `cmdb_fields`, `workspace`, `teams`, `project_meta`, `resolutions`. |
| **Auth (BC-01)** | `JiraClient` built from config + keychain. |
| **Output (BC-07)** | Table/JSON rendering via `output.rs` + `format.rs` + `json_output.rs`. ADF rendering via `adf.rs`. |
| **Cross-cutting** | JQL composition via `jql.rs`. Partial match via `partial_match.rs`. Duration validation via `duration.rs`. |
