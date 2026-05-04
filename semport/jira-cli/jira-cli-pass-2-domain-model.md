# Pass 2: Domain Model — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Builds on: Pass 0 (inventory) + Pass 1 (architecture).

> Pass 2 has two halves. **Sub-pass 2a (structural)** catalogues the entities, value objects, enums, and aggregate boundaries — every field cited is read directly from a struct definition. **Sub-pass 2b (behavioral)** catalogues the operations, business rules, state machines, events, and a draft set of testable invariants for Pass 3. CLAUDE.md and the Pass 1 architecture are inputs but not authority — claims that disagree with code are flagged.
>
> Where a subsection has fewer than 3 substantive items, that's stated explicitly rather than padded.

---

## Sub-pass 2a: Structural model

### 2a.1 Bounded contexts

The crate has a single binary, but the source tree carries seven internally distinct bounded contexts. Each owns a directory (or single module) and has clear inbound/outbound dependencies — verified by reading the `mod.rs` files plus the `use` lines cited in Pass 1.

| Context | Owns | Depends on (downstream) | Notes |
|---|---|---|---|
| **Jira (Core + Agile)** | `src/api/jira/`, `src/types/jira/`, all `cli/issue/*`, `cli/board.rs`, `cli/sprint.rs`, `cli/worklog.rs`, `cli/team.rs`, `cli/user.rs`, `cli/project.rs` | API client, Cache, Auth, Output | The dominant context (1,457 LOC of API impls + 934 LOC of types + the entire `cli/issue/` subtree at 5,078 LOC). Resource-per-file: `boards.rs`, `fields.rs`, `issues.rs`, `links.rs`, `projects.rs`, `resolutions.rs`, `sprints.rs`, `statuses.rs`, `teams.rs`, `users.rs`, `worklogs.rs`. |
| **JSM (Service Desk)** | `src/api/jsm/`, `src/types/jsm/`, `cli/queue.rs` + part of `cli/assets.rs` (workspace discovery is JSM REST) | API client, Cache, Auth | Small (214 + 98 LOC). Two files: `servicedesks.rs`, `queues.rs`. JSM also lives quietly inside `cli/issue/workflow.rs::handle_comment` — the `--internal` flag adds a JSM-specific `sd.public.comment` property. |
| **Assets / CMDB** | `src/api/assets/`, `src/types/assets/`, `cli/assets.rs`, `cli/issue/assets.rs` | API client (separate `get_assets`/`post_assets` HTTP path), Cache, Workspace discovery (cross-cuts JSM context) | 920 + 779 LOC. Five API files: `linked.rs` (CMDB-field discovery + asset extraction/enrichment, the largest at ~557 LOC), `objects.rs` (AQL search + key resolution), `schemas.rs` (object schema + flat object-type listing), `tickets.rs` (connected tickets), `workspace.rs` (workspace ID discovery). |
| **Authentication / Identity** | `src/api/auth.rs` (1,397 LOC), `src/api/auth_embedded.rs` (250 LOC), `src/cli/auth.rs` (1,998 LOC), keychain layer (`keyring` crate) | Config (profile resolution), build.rs (XOR codegen), browser launcher (`open` crate) | Two-layer split: `api::auth` is keychain + OAuth flow, `cli::auth` is per-subcommand orchestration + JSON output shapes. `auth_embedded.rs` is a thin sibling for XOR-obfuscated build-time secrets. |
| **Configuration** | `src/config.rs` (1,223 LOC) | figment, JR_* env vars, file system (`~/.config/jr/config.toml`, walked-up `.jr.toml`) | Owns: `Config`, `GlobalConfig`, `ProfileConfig`, `ProjectConfig`, `FieldsConfig`, `InstanceConfig` (legacy), `DefaultsConfig`, profile-name validation, legacy migration. The active profile name is resolved here, not at use-sites. |
| **Cache** | `src/cache.rs` (899 LOC) | File system (`~/.cache/jr/v1/<profile>/`), chrono | Strict per-profile boundary: every reader/writer takes `profile: &str`. 7-day TTL on every entry. Hosts six cache categories (teams, project_meta, workspace, cmdb_fields, object_type_attrs, resolutions). |
| **Output formatting** | `src/output.rs` (76 LOC), `src/adf.rs` (1,826 LOC), `cli/issue/format.rs`, `cli/issue/json_output.rs` | comfy-table, serde_json, pulldown-cmark | Two output sinks (table, JSON). ADF (Atlassian Document Format) renderer is its own sub-domain — bidirectional text↔ADF + markdown→ADF. JSON write-op response shapes (e.g., `{"key": "FOO-123"}`) live in `cli/issue/json_output.rs`. |

**Cross-context relationships** (verified):

- **Jira → Assets:** `cli/issue/list.rs` and `cli/issue/view.rs` enrich issues with linked-asset data. The boundary is `api::assets::linked::extract_linked_assets*` (uses the `extra: HashMap<String, Value>` flatten field on `IssueFields` to harvest CMDB-field values). This is the only cross-context coupling at the type level.
- **JSM → Assets:** Workspace discovery uses the JSM REST endpoint (`/rest/servicedeskapi/assets/workspace`), not an Assets-native endpoint — see `api::assets::workspace::get_or_fetch_workspace_id`. Cross-context by necessity (Atlassian published it under JSM).
- **Auth → Config:** `api::auth::resolve_refresh_app_credentials` reads from keychain only; `cli::auth::login_oauth` uses the broader resolver (flag → env → keychain → embedded → prompt). Both touch `Config` only to read/write the active profile entry.
- **Cache, Auth, Config all share the active profile name** — but only Config knows how to *resolve* it. The other two consume a `&str`.

The directory layout is namespaced for sibling expansion. Adding a Confluence context would mean `api/confluence/`, `types/confluence/`, and a top-level `cli/confluence.rs` without disturbing existing code.

### 2a.2 Entity catalog

Every field below comes from a struct read with the Read tool. File path + struct name cited; line numbers are stable for the snapshot SHA.

#### Jira context

| Entity | Module | Fields (rust type → JSON) | Lifecycle | Relationships |
|---|---|---|---|---|
| `Issue` | `types/jira/issue.rs:8-12` | `key: String`, `fields: IssueFields` | Read-mostly. Created by `client.create_issue(fields: Value)` (`api/jira/issues.rs:122`) which returns `CreateIssueResponse { key }` only — never the full `Issue`. Mutated via `client.edit_issue` and `client.transition_issue`. Never persisted locally. | Aggregates `IssueFields`; cross-references `Project` via `IssueFields::project`. |
| `IssueFields` | `types/jira/issue.rs:56-80` | `summary: String`, `description: Option<Value>` (raw ADF), `status: Option<Status>`, `issue_type: Option<IssueType>` (`#[serde(rename = "issuetype")]`), `priority: Option<Priority>`, `assignee: Option<User>`, `reporter: Option<User>`, `project: Option<IssueProject>`, `created: Option<String>` (ISO-8601), `updated: Option<String>`, `resolution: Option<Resolution>`, `components: Option<Vec<Component>>`, `fix_versions: Option<Vec<Version>>` (rename `fixVersions`), `labels: Option<Vec<String>>`, `parent: Option<ParentIssue>`, `issuelinks: Option<Vec<IssueLink>>`, `extra: HashMap<String, Value>` (`#[serde(flatten)]`) | Owned by `Issue`. Has two domain accessors: `story_points(field_id)` returns `Option<f64>` (numeric coercion only); `team_id(field_id, verbose)` accepts both scalar UUID and `{"id": "<uuid>"}` Atlas Teams object shape, with once-per-process verbose warning on unexpected shapes. | Holds the `extra` flatten — the open container for custom fields (story points, team UUID, CMDB asset arrays). All custom-field access goes through it. |
| `IssueType` | `types/jira/issue.rs:158-161` | `name: String` | Read-only enrichment of `IssueFields`. | — |
| `Project` | `types/jira/project.rs:3-7` | `key: String`, `name: String` | Lightweight projection used by `client.list_projects` and `client.get_project`. | Distinct from `IssueProject` (the inline project object on issue.fields) and `ProjectSummary`. |
| `ProjectSummary` | `types/jira/project.rs:9-16` | `key: String`, `name: String`, `project_type_key: String` (rename `projectTypeKey`), `lead: Option<ProjectLead>` | Returned by `client.list_projects_summary` and used to fill the cache entry that drives JSM detection (`project_type_key == "service_desk"`). | Aggregates `ProjectLead`. Drives `cache::ProjectMeta`. |
| `ProjectLead` | `types/jira/project.rs:18-24` | `display_name: String` (rename `displayName`), `account_id: String` (rename `accountId`) | — | — |
| `IssueProject` | `types/jira/issue.rs:168-172` | `key: String`, `name: Option<String>` | Inline projection from `issue.fields.project`. | Distinct from `Project` — narrower; used as a foreign-key reference. |
| `User` | `types/jira/user.rs:3-12` | `account_id: String` (rename `accountId`), `display_name: String` (rename `displayName`), `email_address: Option<String>` (rename `emailAddress`), `active: Option<bool>` | The single `User` struct — re-exported from `types::jira` (`mod.rs:16` does `pub use user::User`). Used as assignee, reporter, comment author, changelog author, worklog author. | Universal foreign-key target across Jira context. |
| `Status` | `types/jira/issue.rs:145-150` | `name: String`, `status_category: Option<StatusCategory>` (rename `statusCategory`) | Read-only on `IssueFields`. Also fetched standalone via `client.get_statuses` (`api/jira/statuses.rs`). | Aggregates `StatusCategory`. Status colour driven externally by Jira API; see §2b.2. |
| `StatusCategory` | `types/jira/issue.rs:152-156` | `name: String`, `key: String` | — | Encodes the three-bucket category vocabulary used by `--open` filtering: `key ∈ {"new","indeterminate","done"}` mapped to colour tokens (Pass 1 §3a says `green`/`yellow`/`blue-gray`; this struct carries `key`, not the colour — the colour lives on the parent `Status` struct only when fetched from the standalone `/status` endpoint, not from `issue.fields`). |
| `Priority` | `types/jira/issue.rs:163-166` | `name: String` | — | — |
| `Resolution` | `types/jira/issue.rs:174-185` | `id: Option<String>` (skip if none on serialize), `name: String`, `description: Option<String>` (skip if none) | Two distinct shapes: `GET /rest/api/3/resolution` returns `{id,name,description}`; `issue.fields.resolution` returns `{name}` only. Single struct tolerates both. Cached for 7 days as `cache::ResolutionsCache`. | Persisted in `cache::CachedResolution { id, name, description }` — note `id` is **non-optional** in the cache (entries without `id` are dropped on write with a stderr warning). |
| `Component` | `types/jira/issue.rs:187-190` | `name: String` | — | Component is otherwise structurally `{name}`-only; the full Atlassian `Component` object is intentionally not modeled. |
| `Version` (a.k.a. fix-version) | `types/jira/issue.rs:192-198` | `name: String`, `released: Option<bool>`, `release_date: Option<String>` (rename `releaseDate`) | — | Used only via `IssueFields::fix_versions`. |
| `Comment` | `types/jira/issue.rs:218-226` | `id: Option<String>`, `body: Option<Value>` (raw ADF), `author: Option<User>`, `created: Option<String>`, `properties: Vec<EntityProperty>` (default empty) | Read via `client.list_comments`, written via `client.add_comment`. Body is ADF JSON; rendering is delegated to `src/adf.rs`. The `properties` array carries the JSM-specific `sd.public.comment` flag for the `--internal` toggle. | Aggregates `EntityProperty` and `User`. |
| `EntityProperty` | `types/jira/issue.rs:212-216` | `key: String`, `value: Value` | Comment-level metadata, currently only `sd.public.comment` is consumed. | Degenerate — just key/value. |
| `Transition` | `types/jira/issue.rs:200-205` | `id: String`, `name: String`, `to: Option<Status>` | Read-only — Jira computes available transitions per-issue. | References `Status`. Drives the `jr issue move` resolver. |
| `TransitionsResponse` | `types/jira/issue.rs:207-210` | `transitions: Vec<Transition>` | Wrapper for the `GET /transitions` payload. | — |
| `IssueLink` | `types/jira/issue.rs:25-34` | `id: String`, `link_type: IssueLinkType` (rename `type`), `inward_issue: Option<LinkedIssue>` (rename `inwardIssue`), `outward_issue: Option<LinkedIssue>` (rename `outwardIssue`) | Created via `client.create_issue_link`, deleted via `client.delete_issue_link`. | Aggregates `LinkedIssue` and `IssueLinkType`. |
| `LinkedIssue` | `types/jira/issue.rs:36-40` | `key: String`, `fields: Option<LinkedIssueFields>` | — | — |
| `LinkedIssueFields` | `types/jira/issue.rs:20-23` | `summary: Option<String>` | Used both as `ParentIssue.fields` and `LinkedIssue.fields`. | — |
| `ParentIssue` | `types/jira/issue.rs:14-18` | `key: String`, `fields: Option<LinkedIssueFields>` | — | — |
| `IssueLinkType` | `types/jira/issue.rs:42-48` | `id: Option<String>`, `name: String`, `inward: Option<String>`, `outward: Option<String>` | Read via `client.list_link_types`. | — |
| `IssueLinkTypesResponse` | `types/jira/issue.rs:50-54` | `issue_link_types: Vec<IssueLinkType>` (rename `issueLinkTypes`) | — | — |
| `CreateIssueResponse` | `types/jira/issue.rs:228-231` | `key: String` | Returned by `client.create_issue`. The whole response is intentionally narrow — no `id`, no `self` URL. | — |
| `CreateRemoteLinkResponse` | `types/jira/issue.rs:233-238` | `id: u64`, `self_url: String` (rename `self`) | Returned by `client.create_remote_link` (the `RemoteLink` issue subcommand). | — |
| `Board` | `types/jira/board.rs:3-11` | `id: u64`, `name: String`, `board_type: String` (rename `type`), `location: Option<BoardLocation>` | Read-only. Fetched via `client.list_boards` and `client.get_board_config`. The `board_type` string drives scrum-vs-kanban behaviour in `cli/sprint.rs::resolve_scrum_board`. | Aggregates `BoardLocation`. References `ProjectConfig.board_id` (the per-project default). |
| `BoardLocation` | `types/jira/board.rs:13-19` | `project_key: Option<String>` (rename `projectKey`), `project_name: Option<String>` (rename `projectName`) | — | — |
| `BoardConfig` | `types/jira/board.rs:21-27` | `id: u64`, `name: String`, `board_type: String` (rename `type`, default `""`) | Used to resolve the kanban-vs-scrum check (`cli/sprint.rs:79`). | — |
| `Sprint` | `types/jira/sprint.rs:3-12` | `id: u64`, `name: String`, `state: Option<String>` (`"active"`/`"closed"`/`"future"`), `start_date: Option<String>` (rename `startDate`), `end_date: Option<String>` (rename `endDate`) | Read via `client.list_sprints`, `client.get_sprint`. Mutated via `client.add_issues_to_sprint` / `client.move_issues_to_backlog` (both at sprint-collection level, not on the sprint struct itself). | Aggregates issues at the API level (board → sprints → issues), but the struct itself doesn't carry an issue list — issues are fetched separately. |
| `Worklog` | `types/jira/worklog.rs:6-16` | `id: Option<String>`, `author: Option<User>`, `time_spent_seconds: Option<u64>` (rename `timeSpentSeconds`), `time_spent: Option<String>` (rename `timeSpent`), `comment: Option<Value>` (ADF), `started: Option<String>` | Created via `client.add_worklog`, listed via `client.list_worklogs`. | Aggregates `User`. Comment is raw ADF rendered through `adf.rs`. |
| `ChangelogEntry` | `types/jira/changelog.rs:6-16` | `id: String`, `author: Option<User>` (default), `created: String`, `items: Vec<ChangelogItem>` (default empty) | Read-only via `client.get_changelog`. `author` is `None` for automation/post-functions/migrations. | Aggregates `ChangelogItem`. Multiple items per entry = multiple field-level changes in one transition. |
| `ChangelogItem` | `types/jira/changelog.rs:18-31` | `field: String`, `fieldtype: String`, `from: Option<String>`, `from_string: Option<String>` (rename `fromString`), `to: Option<String>`, `to_string: Option<String>` (rename `toString`) | — | — |
| `TenantContext` | `types/jira/team.rs:17-23` | `org_id: String` (rename `orgId`), `cloud_id: String` (rename `cloudId`) | Returned by the GraphQL `tenantContexts` query (ADR-0005). Used for org discovery in `init` and the team subsystem. | — |
| `TenantContextData` | `types/jira/team.rs:9-13` | `tenant_contexts: Vec<TenantContext>` | GraphQL response wrapper. | — |
| `GraphqlResponse<T>` | `types/jira/team.rs:5-8` | `data: Option<T>` | Generic GraphQL envelope. | — |
| `TeamEntry` | `types/jira/team.rs:26-32` | `team_id: String` (rename `teamId`), `display_name: String` (rename `displayName`) | Returned by `GET /gateway/api/public/teams/v1/org/{orgId}/teams`. Cached for 7 days as `CachedTeam`. | Persisted in `cache::CachedTeam`. |
| `TeamsResponse` | `types/jira/team.rs:34-39` | `entities: Vec<TeamEntry>`, `cursor: Option<String>` | Cursor-paginated. | — |

#### JSM context

| Entity | Module | Fields | Lifecycle | Relationships |
|---|---|---|---|---|
| `ServiceDesk` | `types/jsm/servicedesk.rs:3-10` | `id: String`, `project_id: String` (rename `projectId`), `project_name: String` (rename `projectName`) | Read-only, fetched via `api::jsm::servicedesks`. The service-desk ID is part of `cache::ProjectMeta::service_desk_id`. | — |
| `Queue` | `types/jsm/queue.rs:3-11` | `id: String`, `name: String`, `jql: Option<String>`, `fields: Option<Vec<String>>`, `issue_count: Option<u64>` (rename `issueCount`) | Read-only — listed and viewed via `api::jsm::queues`. | The `jql` field is the queue's own filter; consumers re-issue search with it. |
| `QueueIssueKey` | `types/jsm/queue.rs:17-20` | `key: String` | Lightweight key-only struct used in the two-step JSM queue flow: list issue keys from queue endpoint, then fetch full issues via standard search. Demonstrates the explicit thin-projection convention. | Bridges JSM → Jira contexts (key lookup). |

(JSM context has 3 entity types — fewer than 3 substantive items above the threshold; this is a small intentional surface.)

#### Assets / CMDB context

| Entity | Module | Fields | Lifecycle | Relationships |
|---|---|---|---|---|
| `AssetObject` | `types/assets/object.rs:3-15` | `id: String`, `label: String`, `object_key: String` (rename `objectKey`), `object_type: ObjectType` (rename `objectType`), `created: Option<String>`, `updated: Option<String>`, `attributes: Vec<AssetAttribute>` (default empty) | Read-only — fetched via AQL search or single-object GET. | Aggregates `ObjectType` and `AssetAttribute`. |
| `ObjectType` | `types/assets/object.rs:17-22` | `id: String`, `name: String`, `description: Option<String>` | Inline reference inside `AssetObject`; also fetched standalone (see `ObjectTypeEntry`). | — |
| `AssetAttribute` | `types/assets/object.rs:24-31` | `id: String`, `object_type_attribute_id: String` (rename `objectTypeAttributeId`), `values: Vec<ObjectAttributeValue>` (rename `objectAttributeValues`, default empty) | Inline on AQL search results. | Has only the numeric `objectTypeAttributeId`, not the human name — must be enriched via `get_object_type_attributes`. |
| `ObjectAttribute` | `types/assets/object.rs:43-52` | `id: String`, `object_type_attribute_id: String` (rename `objectTypeAttributeId`), `object_type_attribute: ObjectTypeAttributeDef` (rename `objectTypeAttribute`), `values: Vec<ObjectAttributeValue>` | Returned by `GET /object/{id}/attributes` — the "fat" attribute shape with name resolution baked in. | Aggregates `ObjectTypeAttributeDef`. |
| `ObjectTypeAttributeDef` | `types/assets/object.rs:55-81` | `id: String`, `name: String`, `system: bool` (default), `hidden: bool` (default), `label: bool` (default), `position: i32` (default), `default_type: Option<DefaultType>` (rename `defaultType`), `reference_type: Option<ReferenceType>` (rename `referenceType`), `reference_object_type: Option<ReferenceObjectType>` (rename `referenceObjectType`), `minimum_cardinality: i32` (rename `minimumCardinality`, default), `maximum_cardinality: i32` (rename `maximumCardinality`, default), `editable: bool` (default), `description: Option<String>`, `options: Option<String>` | Schema metadata for an attribute. Used by `jr assets schema`. Backward-compatibility: most fields default, so old JSON without them deserializes cleanly. | Reference attributes carry both `reference_type` (e.g., "Depends on") and `reference_object_type` (e.g., "Service") — together they encode CMDB relationships. |
| `DefaultType` | `types/assets/object.rs:84-88` | `id: i32`, `name: String` | E.g., `{0, "Text"}`, `{10, "Select"}`. | — |
| `ReferenceType` | `types/assets/object.rs:91-95` | `id: String`, `name: String` | E.g., "Depends on", "References". | — |
| `ReferenceObjectType` | `types/assets/object.rs:98-102` | `id: String`, `name: String` | The target type a reference attribute points at. | — |
| `ObjectAttributeValue` | `types/assets/object.rs:33-38` | `value: Option<String>`, `display_value: Option<String>` (rename `displayValue`) | A single attribute cell. | — |
| `ObjectSchema` | `types/assets/schema.rs:5-15` | `id: String`, `name: String`, `object_schema_key: String` (rename `objectSchemaKey`), `description: Option<String>`, `object_count: i64` (rename `objectCount`, default), `object_type_count: i64` (rename `objectTypeCount`, default) | Read-only via `jr assets schemas`. | Top-level container. |
| `ObjectTypeEntry` | `types/assets/schema.rs:18-33` | `id: String`, `name: String`, `description: Option<String>`, `position: i32` (default), `object_count: i64` (rename `objectCount`, default), `object_schema_id: String` (rename `objectSchemaId`), `inherited: bool` (default), `abstract_object_type: bool` (rename `abstractObjectType`, default) | Returned by `/objectschema/{id}/objecttypes/flat`. | Distinct from `ObjectType` (which is the inline reference inside `AssetObject`). |
| `LinkedAsset` | `types/assets/linked.rs:4-17` | `key: Option<String>`, `name: Option<String>`, `asset_type: Option<String>` (rename `type`), `id: Option<String>`, `workspace_id: Option<String>` | The display-side struct. **Serialize-only** — there is no `Deserialize` derive. Created by extraction from `IssueFields::extra` then enriched via per-field GET. All fields are `Option` because partial extraction (e.g., id-only when CMDB field discovery hasn't run yet) is normal. | Foreign-key into the Assets context — the bridging type between Jira context and Assets context. Has `display()` and `display_name_only()` formatters with a graceful "run jr init to resolve asset names" fallback when only `id` is present. |
| `ConnectedTicketsResponse` | `types/assets/ticket.rs:3-9` | `tickets: Vec<ConnectedTicket>` (default empty), `all_tickets_query: Option<String>` (rename `allTicketsQuery`) | Returned by the connected-tickets endpoint. | The `all_tickets_query` carries an opaque `issueFunction in assetsObject(...)` JQL clause for follow-up searches. |
| `ConnectedTicket` | `types/assets/ticket.rs:11-23` | `key: String`, `id: String`, `title: String`, `reporter: Option<String>`, `created: Option<String>`, `updated: Option<String>`, `status: Option<TicketStatus>`, `issue_type: Option<TicketType>` (rename `type`), `priority: Option<TicketPriority>` | Read-only, returned alongside an `AssetObject`. | Distinct from `Issue` even though both represent Jira tickets — connected-ticket payload includes a `colorName` on status that the Jira Issue API doesn't, and the title field is `title` not `summary`. |
| `TicketStatus` | `types/assets/ticket.rs:25-30` | `name: String`, `color_name: Option<String>` (rename `colorName`) | The `color_name` carries the Jira status-category colour token (`green`/`yellow`/`blue-gray`), used for the assets-context `--open` filter (`cli/assets.rs:303-321`). | This is the **only place** colour names appear as first-class data in the codebase; in the Jira context the colour is implicit in `StatusCategory.key`. |
| `TicketType` | `types/assets/ticket.rs:32-35` | `name: String` | — | — |
| `TicketPriority` | `types/assets/ticket.rs:37-40` | `name: String` | — | — |

#### Authentication / Identity context

| Entity | Module | Fields | Lifecycle | Relationships |
|---|---|---|---|---|
| `EmbeddedOAuthApp` | `api/auth_embedded.rs:22-26` | `client_id: String`, `client_secret: String` | Decoded once per process via `OnceLock` from XOR-obfuscated build constants. Plaintext held in process memory for the binary's lifetime. Custom `Debug` impl redacts `client_secret`. | Defense-in-depth: presence-only check (`embedded_oauth_app_present()`) is available so `jr auth status` doesn't need to materialize the plaintext. |
| `OAuthAppSource` (enum) | `api/auth_embedded.rs:46-57` | Variants: `Flag`, `Env`, `Keychain`, `Embedded`, `Prompt`, `None` (sentinel) | Reported by `jr auth status` so users can tell which credentials drove the live session. | Has `label() -> &'static str`. |
| `LoginArgs` | `cli/auth.rs:543` (struct passed to `handle_login`) | Per main.rs:91-110: `profile: Option<String>`, `url: Option<String>`, `oauth: bool`, `email: Option<String>`, `token: Option<String>`, `client_id: Option<String>`, `client_secret: Option<String>`, `no_input: bool` | Builds on the way down through `handle_login → login_token / login_oauth`. | Encodes all login inputs in a single struct (no scattered parameters). |
| `RefreshArgs<'_>` | `cli/auth.rs:845` | `profile: Option<&str>`, `oauth: bool`, `email: Option<String>`, `token: Option<String>`, `client_id: Option<String>`, `client_secret: Option<String>`, `no_input: bool`, `output: &OutputFormat` | Same shape as `LoginArgs` but borrowed; refresh = clear + relogin (per Pass 1 §8 deviation). | — |
| Keychain key namespacing | `api/auth.rs:18-32` (constants + `oauth_access_key`/`oauth_refresh_key` fns) | `KEY_EMAIL = "email"`, `KEY_API_TOKEN = "api-token"`, `oauth_client_id` (flat), `oauth_client_secret` (flat), `KEY_OAUTH_ACCESS_LEGACY = "oauth-access-token"` (flat, read-only after migration), `KEY_OAUTH_REFRESH_LEGACY = "oauth-refresh-token"` (flat, read-only), namespaced: `format!("{profile}:oauth-access-token")` and `format!("{profile}:oauth-refresh-token")` | Shared keys are written by the API-token login flow and the OAuth-app credential flow. Per-profile namespaced keys are written on every successful OAuth flow. The `"default"` profile lazy-migrates legacy flat keys on first read; non-default profiles never inherit. | Confirms CLAUDE.md's "shared vs namespaced" boundary at the source. |

(Auth context has 5 first-class entities/types — at threshold; documented as-is.)

#### Configuration context

| Entity | Module | Fields | Lifecycle | Relationships |
|---|---|---|---|---|
| `Config` | `config.rs:82-88` | `global: GlobalConfig`, `project: ProjectConfig`, `active_profile_name: String` | Built by `Config::load` / `Config::load_with(cli_profile)` / `Config::load_lenient_with`. The active profile name is resolved at construction (precedence flag > env > config > "default"). | The runtime config object — every handler that hits the network has one. |
| `GlobalConfig` | `config.rs:27-52` | `default_profile: Option<String>` (default), `profiles: BTreeMap<String, ProfileConfig>` (default), `instance: InstanceConfig` (legacy, skip on serialize), `fields: FieldsConfig` (legacy, skip on serialize), `defaults: DefaultsConfig` (default) | Loaded from `~/.config/jr/config.toml` via figment + `JR_*` env overlay. Migration drops legacy `instance`/`fields` from disk on next save. | `BTreeMap` (not HashMap) for stable ordering in `jr auth list` and serialized config. |
| `ProfileConfig` | `config.rs:16-25` | `url: Option<String>`, `auth_method: Option<String>` (`"oauth"`/`"api_token"`), `cloud_id: Option<String>`, `org_id: Option<String>`, `oauth_scopes: Option<String>`, `team_field_id: Option<String>`, `story_points_field_id: Option<String>` | Per-profile entry under `[profiles.<name>]` in TOML. The custom-field IDs are duplicated here (vs. the legacy `[fields]` section) so each profile can have distinct sandbox/prod IDs. | The fundamental unit of multi-profile isolation. |
| `ProjectConfig` | `config.rs:76-80` | `project: Option<String>`, `board_id: Option<u64>` | Loaded from a `.jr.toml` file walked up from cwd. Not merged into `GlobalConfig` — kept as a separate field on `Config`. | The "what does this repository talk to" layer. |
| `FieldsConfig` (legacy) | `config.rs:10-14` | `team_field_id: Option<String>`, `story_points_field_id: Option<String>` | Read-only, drained into `ProfileConfig` during migration. Skipped on serialize after migration. | Confirmed legacy. |
| `InstanceConfig` (legacy) | `config.rs:54-61` | `url: Option<String>`, `cloud_id: Option<String>`, `org_id: Option<String>`, `auth_method: Option<String>`, `oauth_scopes: Option<String>` | Same migration semantics as `FieldsConfig`. | — |
| `DefaultsConfig` | `config.rs:63-74` | `output: String` (default `"table"`) | Currently single-field — exists as an extension point. | — |

#### Cache context

Entities below are all whole-file cached (read by `read_cache<T: Expiring + DeserializeOwned>`) except `ProjectMeta` and `CachedObjectTypeAttr`, which use keyed-map caches with per-entry TTL.

| Entity | Module | Fields | Notes |
|---|---|---|---|
| `Expiring` (trait) | `cache.rs:10-12` | `fn fetched_at(&self) -> DateTime<Utc>` | The TTL contract. |
| `CachedTeam` | `cache.rs:45-49` | `id: String`, `name: String` | Persisted shape. |
| `TeamCache` | `cache.rs:51-61` | `fetched_at: DateTime<Utc>`, `teams: Vec<CachedTeam>` | Whole-file `teams.json`. |
| `ProjectMeta` | `cache.rs:105-112` | `project_type: String`, `simplified: bool`, `project_id: String`, `service_desk_id: Option<String>`, `fetched_at: DateTime<Utc>` | Map cache `project_meta.json` (key: project key). Per-entry TTL. |
| `WorkspaceCache` | `cache.rs:175-185` | `workspace_id: String`, `fetched_at: DateTime<Utc>` | Whole-file `workspace.json`. |
| `CachedResolution` | `cache.rs:202-208` | `id: String` (non-optional), `name: String`, `description: Option<String>` | Note: id is non-optional; resolutions without ids are dropped on write (defensive — see `cli/issue/workflow.rs:117-133`). |
| `ResolutionsCache` | `cache.rs:210-220` | `resolutions: Vec<CachedResolution>`, `fetched_at: DateTime<Utc>` | Whole-file `resolutions.json`. |
| `CmdbFieldsCache` | `cache.rs:237-247` | `fields: Vec<(String, String)>` (id, name tuples), `fetched_at: DateTime<Utc>` | Whole-file `cmdb_fields.json`. The `(id, name)` tuple shape is the format-change point flagged in CLAUDE.md. |
| `CachedObjectTypeAttr` | `cache.rs:264-276` | `id: String`, `name: String`, `system: bool` (default), `hidden: bool` (default), `label: bool` (default), `position: i32` (default) | Persisted projection of `ObjectTypeAttributeDef` — a thin projection (no defaultType / referenceType / cardinality), enough to drive `jr assets schema` display. |
| `ObjectTypeAttrCache` | `cache.rs:278-282` | `fetched_at: DateTime<Utc>`, `types: HashMap<String, Vec<CachedObjectTypeAttr>>` | Map cache `object_type_attrs.json` (key: object-type id), per-file TTL. |

#### Errors

| Variant | Module | Fields | Exit code | Trigger |
|---|---|---|---:|---|
| `NotAuthenticated` | `error.rs:5-6` | — | 2 | 401 with no auth, or no stored credentials. |
| `InsufficientScope { message }` | `error.rs:9-16` | `message: String` | 2 | 401 + body matching "scope does not match" (issue #185 — granular tokens reject POST). Display is multi-line and includes workaround instructions and the GitHub issue link. |
| `NetworkError(String)` | `error.rs:18-19` | host string | 1 | reqwest reachability failure (DNS, connect). |
| `ApiError { status, message }` | `error.rs:21-22` | `status: u16`, `message: String` | 1 | Any 4xx/5xx not specialised above. Message comes from `extract_error_message` (Pass 1 §8 — 6-level precedence chain). |
| `ConfigError(String)` | `error.rs:24-25` | reason string | 78 | Missing config / profile unconfigured. |
| `UserError(String)` | `error.rs:27-28` | reason string | 64 | Bad CLI input — invalid profile name, ambiguous match, empty selection, validation failure. |
| `Internal(String)` | `error.rs:30-36` | reason string (must be prefixed `"Internal error:"` by callers) | 1 | "Should never happen" invariant violations. Distinguished from `UserError` (64) and `ConfigError` (78) so callers can match on "is this a bug?". |
| `Interrupted` | `error.rs:38-39` | — | 130 | Reserved variant; the actual Ctrl+C path in `main.rs:264` exits 130 directly without constructing this variant. |
| `Http(reqwest::Error)` | `error.rs:41-42` | `#[from]` transparent | 1 | reqwest internal errors. |
| `Io(std::io::Error)` | `error.rs:44-45` | `#[from]` transparent | 1 | Filesystem failures. |
| `Json(serde_json::Error)` | `error.rs:47-48` | `#[from]` transparent | 1 | Serde failures. |

Verified at `error.rs:1-49` plus the `exit_code()` `match` at `error.rs:51-62`. Pass 1 listed 10 variants; the actual count is **11** (Pass 1 missed `Json`).

### 2a.3 Value objects & enums

Pure-data types and small domain enums.

- **`OutputFormat`** (`cli/mod.rs:47-51`): `enum { Table, Json }`. `derive(Clone, Copy, ValueEnum)`. Implements the `--output` contract globally.
- **`HttpMethod`** (referenced from `cli/mod.rs:117` as `cli::api::HttpMethod`): the value-enum the `jr api` raw passthrough takes via `-X`. Defined in `cli/api.rs` (which I did not read in full, but Pass 0 lists at 342 LOC and main.rs:248 dispatches into it).
- **`MatchResult`** (`partial_match.rs:3-13`): `enum { Exact(String), ExactMultiple(String), Ambiguous(Vec<String>), None(Vec<String>) }`. Domain-meaningful — encodes the resolution outcome for partial matching used by every "find by name" resolver in the codebase. Single-substring hits route through `Ambiguous` (not `Exact`) by design — the resolvers must prompt or error rather than silently resolve. Property-tested.
- **`OAuthAppSource`** (`api/auth_embedded.rs:46-57`): see §2a.2. Six-variant enum with `label()`.
- **`JrError`** (`error.rs:3-49`): see §2a.2. Eleven variants, exit-code mapping.
- **`Cli`** + Subcommand enums (`cli/mod.rs:18-738`): `Cli`, `Command`, `AssetsCommand`, `AuthCommand`, `IssueCommand`, `ProjectCommand`, `BoardCommand`, `SprintCommand`, `TeamCommand`, `UserCommand`, `WorklogCommand`, `QueueCommand`. Each clap Subcommand enum encodes the argument shape — they're domain-bearing because each variant's flag set encodes a feature.
- **Pagination shapes** (`api/pagination.rs`, per Pass 1 §3f): `OffsetPage<T>`, `CursorPage<T>`, `ServiceDeskPage<T>`, `AssetsPage<T>`. Four distinct cursor/offset envelopes corresponding to the four API style families (Jira REST core, JQL search, JSM, Assets). `AssetsPage::is_last` accepts both bool and string via custom deserializer.
- **`DEFAULT_OAUTH_SCOPES`** (`api/auth.rs:58-63`): a `&'static str` listing the seven default OAuth scopes. Built via `concat!` so double-spaces are visually obvious; pinned by a regression test (cited in source: `default_oauth_scopes_pins_the_full_set_with_offline_access`).
- **`DEFAULT_LIMIT`** (`cli/mod.rs:740`): `pub(crate) const DEFAULT_LIMIT: u32 = 30`. Drives `resolve_effective_limit`.
- **`MAX_SPRINT_ISSUES`** (`cli/sprint.rs:107`): `const MAX_SPRINT_ISSUES: usize = 50`. Per-operation cap on `sprint add` / `sprint remove`.
- **`MAX_RETRIES`/`DEFAULT_RETRY_SECS`** (`api/client.rs:11-14`, per Pass 1 §3e): `MAX_RETRIES = 3`, `DEFAULT_RETRY_SECS = 1`.
- **`EMBEDDED_CALLBACK_PORT`** (`api/auth.rs:384`, per Pass 1 §3d): the literal `53682`. Owned by `api::auth`, not `api::auth_embedded` (Pass 1 §8 deviation #3).
- **Issue keys are bare `String`s** — no wrapper type. The format is `[A-Za-z0-9]+-\d+` (Atlassian rule), but the codebase doesn't enforce it on `Issue` types. There IS a stricter validator for the analogous *asset* key in `jql.rs::validate_asset_key`. Issue-key validation is implicit (whatever Jira accepts).
- **Profile names are `String`s** with `validate_profile_name` (`config.rs:113-140`) — see §2a.4. The validator is the gate.
- **JQL strings are bare `String`s** — escaped at the call site via `jql::escape_value`, validated piecewise (`validate_duration`, `validate_asset_key`, `validate_date`).
- **AQL clauses** are bare `String`s built by `jql::build_asset_clause` (`jql.rs:61-82`). The clause is `"<field_name>" IN aqlFunction("Key = \"<asset_key>\"")` (single field) or parenthesized OR-join of multiples. The function name is `aqlFunction`, the AQL attribute is `Key` (capitalized), confirming the CLAUDE.md gotcha.
- **Worklog duration parser output** (`duration.rs:5-49`): `parse_duration(input, hours_per_day, days_per_week) -> Result<u64>` returns total seconds as `u64`. Inverse `format_duration(seconds) -> String` returns `"30m"`, `"2h"`, or `"1h30m"` (never with weeks/days — format collapses to hours+minutes). Property-tested for never-panics on garbage input and round-trip.
- **ADF nodes** (`adf.rs`, 1,826 LOC, not read in full): a hand-written ADF parser/emitter; nodes are JSON `Value`s plus a small DSL for emitting common shapes. The public surface (per Pass 1) includes `text→ADF`, `markdown→ADF`, `ADF→text`.
- **`Expiring` trait** (`cache.rs:10-12`): a single-method trait `fn fetched_at(&self) -> DateTime<Utc>`. The TTL contract.

There is **no enum** for status-category colour. CLAUDE.md says "green/yellow/blue-gray are hardcoded" — those tokens appear only as raw `&str` literals at filter call-sites (`cli/assets.rs:303-321` for connected-tickets) or as `statusCategory != Done` JQL fragments (`cli/issue/list.rs:308, 625`). No `enum StatusColor` exists. This is a deliberate "stay close to JSON" choice and consistent with the rest of the types module.

### 2a.4 Schema-level invariants

- **Profile names** (`config.rs:113-140`): `[A-Za-z0-9_-]{1,64}`, with reserved Windows names blocked: `CON`, `NUL`, `AUX`, `PRN`, `COM1-9`, `LPT1-9`. Validation is enforced at three boundary points: the `--profile` CLI flag (`main.rs:62`), every key in `[profiles.*]` in TOML (`config.rs:274-282`), and the resolved active-profile name (`config.rs:304`). **Reason:** the resolved name flows into cache paths AND keychain key prefixes; a value like `foo:bar` or `../etc/passwd` would corrupt those namespaces.
- **Asset object keys** (`jql.rs:39-54`): `<alphanumeric>-<digits>` — prefix must be ASCII alphanumeric and non-empty, number must be ASCII digits and non-empty. E.g., `CUST-5`, `SRV-42`, `ITSM-123`. Case is not lowercased.
- **JQL-relative-date durations** (`jql.rs:16-33`): `<digits><single-unit>` where unit is `y`, `M`, `w`, `d`, `h`, or `m`. Case-sensitive (`M` = months, `m` = minutes). Combined units like `4w2d` are rejected. Empty rejected. Reversed order (`d7`) rejected.
- **Worklog durations** (`duration.rs:5-49`): permits the *combined* form (`1w2d3h30m`) and is case-insensitive (input is lowercased first). Only `w`/`d`/`h`/`m` units. **Note: the JQL duration validator and the worklog duration parser have different syntaxes** — same-looking inputs can pass one and fail the other. Worklog uses configurable `hours_per_day`/`days_per_week` (defaults 8/5 in tests; Jira instance settings drive the live values).
- **Absolute dates** (`jql.rs:88-92`): ISO-8601 `YYYY-MM-DD` only. Leap-day handling delegated to `chrono::NaiveDate::parse_from_str` — feb-30 and feb-29 in non-leap years are rejected.
- **JQL escaping order** (`jql.rs:6-8`): backslashes first, then double-quotes. Order is load-bearing — reversing them allows escape-neutralization (escaping quotes first introduces backslashes that the second pass re-escapes, leaving the quote exposed). Property-tested with `escaped_value_never_has_unescaped_quote` (`jql.rs:383-394`).
- **AQL `Key` attribute capitalization** (`jql.rs:70`): the AQL attribute name is the literal string `Key` (capital K), not the JSON-field-name `objectKey`. Confirmed at the source as `format!("Key = \\\"{}\\\"", ...)`. This pins the CLAUDE.md gotcha.
- **`aqlFunction` field-name requirement** (`jql.rs:61-82`): the JQL function `aqlFunction()` requires the human-readable field **name** as the wrapped expression's left-hand identifier — `cf[ID]` and `customfield_NNNNN` are both rejected by Jira at runtime. The escape pipeline runs `escape_value` on both the field name AND the asset key.
- **Cache file naming and TTL** (`cache.rs:7, 30, 76-78, 119-143`): TTL is a hard 7 days for every cached item. Per-profile dir: `<root>/v1/<profile>/`. Whole-file cache filenames are constants with `.json` extension: `teams.json`, `workspace.json`, `resolutions.json`, `cmdb_fields.json`. Map caches: `project_meta.json` (per-project-key map), `object_type_attrs.json` (per-object-type-id map). The versioned root `v1/` allows future-proof orphaning of stale schemas.
- **Cache miss policy** (`cache.rs:14-34`): `NotFound` → `Ok(None)`; deserialization failure → stderr warning + `Ok(None)`; expired → `Ok(None)`. The "corrupt = miss" rule means cache-format changes auto-recover (corruption is treated as cache miss, not error).
- **Per-profile keychain key namespacing** (`api/auth.rs:24-32, 111-169`): shared (account-level) flat keys: `email`, `api-token`, `oauth_client_id`, `oauth_client_secret`. Per-profile namespaced keys: `<profile>:oauth-access-token`, `<profile>:oauth-refresh-token`. Legacy flat keys (`oauth-access-token`, `oauth-refresh-token`) are read-only after migration; only the `"default"` profile inherits them, and the migration deletes them after copying. **Non-default profiles never inherit legacy keys** — preventing cross-pollination of credentials across distinct Jira sites.
- **OAuth scope set is pinned** (`api/auth.rs:58-63`): `read:jira-work write:jira-work read:jira-user read:servicedesk-request read:cmdb-object:jira read:cmdb-schema:jira offline_access`. The embedded `jr` Atlassian app must be registered with this exact set or the authorize call rejects with `invalid_scope`.
- **`IssueFields::extra` is a `HashMap<String, Value>` flatten container** (`types/jira/issue.rs:78-79`): all custom fields land here. Story points and team UUID are accessed by ID via `IssueFields::story_points(field_id)` and `IssueFields::team_id(field_id, verbose)`. The flatten field also feeds CMDB asset extraction.
- **`IssueFields::team_id` shape tolerance** (`issue.rs:101-131`): accepts both string-UUID (legacy/some tenants) and `{"id":"<uuid>", ...}` Atlas Teams object shape. Object with non-string `id` → `None` + once-per-process verbose warning. Pinned by 9 unit tests including a non-string-id regression test (`team_id_returns_none_for_object_with_non_string_id`).
- **Resolutions without an id are dropped on cache write** (`cli/issue/workflow.rs:117-133`): a defensive fallback. The `cache::CachedResolution.id` field is non-optional, so an id-less resolution can't be persisted; the count delta is logged to stderr.

### 2a.5 Aggregate boundaries

- **`Issue` is the obvious aggregate root.** It owns `IssueFields` (1:1), which in turn owns/references many leaves: `Status` → `StatusCategory`, `IssueType`, `Priority`, `Resolution`, `User` (assignee, reporter), `IssueProject`, `Component[]`, `Version[]`, `ParentIssue` → `LinkedIssueFields`, `IssueLink[]` → `LinkedIssue` + `IssueLinkType`. Subordinates accessed via `extra: HashMap<String, Value>`: story points (numeric), team UUID (string or object), CMDB linked-asset arrays (raw JSON). The aggregate boundary stops at the issue level — comments, transitions, changelog, and worklogs are *associated* (own endpoint, own struct) but not aggregated.
- **`Comment` is its own root** (separate endpoint, separate aggregate boundary). Aggregates `User` (author), `EntityProperty[]` (JSM internal-comment flag).
- **`ChangelogEntry` is its own root.** Aggregates `User` (author) and `ChangelogItem[]` (one entry can describe many simultaneous field changes).
- **`Worklog` is its own root.** Aggregates `User` (author).
- **`Board` is a thin root** that doesn't directly aggregate sprints or issues — those are queried separately. `Board → BoardLocation` is the only owned relationship. Board-level state machine (scrum vs kanban) lives in `BoardConfig.board_type`.
- **`Sprint` is its own root.** Doesn't own issues — sprint-issue relationships are 1:N via separate API calls (`client.list_sprints`, `client.add_issues_to_sprint`, `client.move_issues_to_backlog`).
- **`Project` and `ProjectSummary` are siblings, not parent-child.** `Project` is the lightweight `{key, name}` shape; `ProjectSummary` is the discovery shape with `lead`. `ProjectMeta` (the cached `{project_type, simplified, project_id, service_desk_id}` projection) is yet a third shape, persisted in `cache::ProjectMeta`.
- **`AssetObject` is the Assets-context root.** Owns `ObjectType` (1:1) and `AssetAttribute[]` (each with `ObjectAttributeValue[]`). For the "fat" attribute fetch (single-object detail), each `ObjectAttribute` additionally aggregates `ObjectTypeAttributeDef` (which itself can aggregate `DefaultType`, `ReferenceType`, `ReferenceObjectType`).
- **`LinkedAsset` is a leaf-only display projection** — has no aggregate behaviour, just `display()` formatters.
- **`ObjectSchema → ObjectTypeEntry`** is the discovery aggregate (one schema, many object types). Discovered via `jr assets schemas` → `jr assets types --schema X`.
- **`ServiceDesk → Queue → Issue`** is a three-level lookup (queue lists return `QueueIssueKey`, then the keys feed `client.search_issues`). The two-step query is a deliberate thin-projection pattern (avoid pulling expensive issue payloads via JSM endpoint when only keys are needed).
- **Configuration aggregate:** `Config → GlobalConfig + ProjectConfig`. `GlobalConfig → BTreeMap<String, ProfileConfig>` is the multi-profile heart. `Config::active_profile_name` is a *resolved* projection (not a stored field on disk).
- **Cache aggregate:** there is no in-memory cache aggregate; each cached entity is its own self-contained whole-file or keyed-map artifact. The implicit aggregate root is the per-profile cache directory.
- **Auth aggregate:** the `keyring` service `jr-jira-cli` (or `JR_SERVICE_NAME` override) is the implicit aggregate root. Inside it, two key-shape namespaces co-exist: shared (flat) and per-profile (`<profile>:` prefix).

---

## Sub-pass 2b: Behavioral model

### 2b.1 Operations catalog

Catalogued from `cli/mod.rs` (CLI surface) + each `cli/<cmd>::handle` dispatcher + `api/jira/*.rs` HTTP methods. Inputs are flag/positional; effects describe HTTP, file system, keychain, cache, and idempotency.

**Top-level command surface** (per `Cli::Command` in `cli/mod.rs:54-133`): `Init`, `Assets`, `Auth`, `Me`, `Project`, `Issue`, `Board`, `Sprint`, `Worklog`, `Team`, `User`, `Queue`, `Api`, `Completion`. Confirms Pass 0 / Pass 1 (CLAUDE.md is missing `Api` and `Completion`).

#### Auth subsystem

| Command | Inputs | Effects | Output | Idempotent? |
|---|---|---|---|---|
| `auth login` | `--profile`, `--url`, `--oauth`, `--email`, `--token`, `--client-id`, `--client-secret` (all optional; flags can be replaced by `JR_*` env vars) | Resolves credentials (flag → env → keychain → embedded → prompt for OAuth; flag → env → prompt for API token). Writes to keychain (shared `email`/`api-token` for token flow; `<profile>:oauth-*` for OAuth flow). Writes profile entry to `config.toml` with `url`, `auth_method`, optional `cloud_id` / `org_id` / `oauth_scopes`. For OAuth: opens browser, binds local listener (`127.0.0.1:53682` for embedded, `:0` for BYO), exchanges code for tokens, queries `accessible-resources` for cloudId discovery. | Text or JSON (`{...profile-summary...}`). | Yes-ish — re-running overwrites the profile and refreshes tokens. Old tokens are clobbered. |
| `auth status` | `--profile` (defaults to active) | Reads profile entry from `config.toml`. Reads keychain to test credential presence. For OAuth, reports `OAuthAppSource` via `peek_oauth_app_source` (does NOT decode embedded plaintext — defense in depth). | Text or JSON. | Yes (read-only). |
| `auth refresh` | `--profile`, `--oauth`, `--email`, `--token`, `--client-id`, `--client-secret` | **Clear-and-relogin** (per Pass 1 §8). Deletes existing keychain entries for the target profile + clears that profile's cache (per the macOS Keychain ACL workaround for issue #207). Then re-runs the same flow as `auth login`. | Text or JSON. | Yes — destructive but idempotent. |
| `auth switch <name>` | positional `name` | Reads `config.toml`, validates `<name>` exists in `[profiles]`, sets `default_profile = <name>`, saves config. | Text. | Yes. |
| `auth list` | none | Reads `config.toml`. Renders rows of `(profile_name, url, auth_method, cloudId, active?)`. | Text or JSON (via `render_list_table` / `render_list_json` at `cli/auth.rs:1125, 1152`). | Yes (read-only). |
| `auth logout` | `--profile` | Deletes the per-profile OAuth token entries (`<profile>:oauth-*`). Shared API-token / OAuth app credentials are NEVER touched. Profile entry stays in `config.toml`. | Text. | Yes — second call is no-op. |
| `auth remove <name>` | positional `name` (cannot be active) | Deletes the `[profiles.<name>]` entry from `config.toml`, the per-profile OAuth tokens, and the per-profile cache directory (`cache::clear_profile_cache(name)`). Shared credentials NEVER touched. Errors if `name == active_profile`. | Text. | Yes — second call would error "unknown profile". |

#### Init

| Command | Inputs | Effects | Output | Idempotent? |
|---|---|---|---|---|
| `init` | none (interactive) | Creates / updates default profile entry in `config.toml`, prefetches: org metadata via GraphQL `tenantContexts` (sets `org_id` / `cloud_id` on profile), team list (writes `cache::TeamCache`), story-points custom-field id discovery (writes `profile.story_points_field_id`). `cli/init.rs::handle` skips auth at top of `main.rs:77`. | Interactive prompts; final summary. | Yes — re-running rediscovers and overwrites. |

#### Issue subsystem

12 + 1 = 13 issue subcommands per `cli/mod.rs:278-561`:

| Command | Inputs | Effects | Output | Idempotent? |
|---|---|---|---|---|
| `issue list` | `--jql`, `--status`, `--team`, `--limit`/`--all`, `--assignee`, `--reporter`, `--recent`, `--open`, `--points`, `--assets`, `--asset`, `--created-after`, `--created-before`, `--updated-after`, `--updated-before` | Validates date/duration formats early (no network on bad input). Optionally fetches CMDB fields (cache or API) when `--asset`/`--assets` is set. Composes JQL (project scope, `--open` → `statusCategory != Done`, asset clause, etc.). Cursor-paginated `POST /rest/api/3/search/jql`. Optional per-row asset enrichment. | Table (with conditional Points/Assets/Team columns) or JSON. | Yes (read-only). |
| `issue view <key>` | positional `key` | Fetches issue via `GET /rest/api/3/issue/<key>`. Optional CMDB enrichment if linked-asset display is on. | Detail view (formatted) or JSON (raw issue). | Yes. |
| `issue create` | `--project`/`-p`, `--type`/`-t`, `--summary`/`-s`, `--description`/`-d` or `--description-stdin`, `--priority`, `--label` (repeatable), `--team`, `--points`, `--markdown`, `--parent`, `--to`/`--account-id` (mutually exclusive) | Optional team/user resolution (cache or API). ADF translation of description (markdown if `--markdown`). `POST /rest/api/3/issue`. | `{"key": "FOO-123"}` JSON (`cli/issue/json_output.rs::move_response` and friends). | No — every call creates a new issue. |
| `issue edit <key>` | `--summary`, `--type`, `--priority`, `--label` (with `add:`/`remove:` prefixes), `--team`, `--points` or `--no-points`, `--parent`, `--description`/`--description-stdin`, `--markdown` | `PUT /rest/api/3/issue/<key>` with computed field deltas. | Success message or JSON. | No — replaces or modifies state. |
| `issue move <key> [status] [--resolution X]` | positional `key`, optional `status`, `--resolution` | **Idempotent state machine** (see §2b.3). Fetches transitions + current status; if `current == target`, prints success and exits 0 without an HTTP write. Otherwise resolves status name via `partial_match`, optionally resolves resolution name (no auto-promote on substring), `POST /rest/api/3/issue/<key>/transitions`. Transforms 400 "resolution required" errors into `--resolution` hint. | `{"key", "status", "transitioned": bool}` JSON, or text success/skip message. | **Yes** (per CLAUDE.md and source at `workflow.rs:192-224`). |
| `issue transitions <key>` | positional `key` | `GET /rest/api/3/issue/<key>/transitions`. | Table `[ID, Name, To Status]` or JSON. | Yes (read-only). |
| `issue resolutions [--refresh]` | `--refresh` | Cache-first read (`cache::ResolutionsCache`, 7-day TTL). On miss or `--refresh`, `GET /rest/api/3/resolution`. Cache-write on success. | Table `[Name, Description]` or JSON. | Yes (read-only). |
| `issue assign <key> [--to/--account-id/--unassign]` | positional `key`, mutually-exclusive flags | If unassign: `PUT issue/{key}/assignee {accountId: null}`. Else resolves user via search or accountId. **Idempotent** when target = current. | Success or JSON. | **Yes** (per CLAUDE.md, by checking current assignee before write). |
| `issue comment <key> [message]` | positional `key`, optional positional `message`, `--markdown`, `--file`, `--stdin`, `--internal` | Reads message from positional/file/stdin. ADF-converts (markdown if flagged). `POST /rest/api/3/issue/<key>/comment` with `internal=true` flag adding `properties: [{key:"sd.public.comment", value:{internal:true}}]`. | Comment ID or JSON. | No — every call creates a new comment. |
| `issue comments <key> [--limit N]` | positional `key`, `--limit` | Paginated `GET /rest/api/3/issue/<key>/comment`. | Table or JSON. | Yes (read-only). |
| `issue changelog <key> [--limit/--all] [--field X (repeatable)] [--author Y] [--reverse]` | positional `key`, multi-flag filters | `GET /rest/api/3/issue/<key>?expand=changelog`. Post-filter by field-substring + author-needle (smart constructor: `:` or 12+ chars with digit → exact accountId; else displayName/accountId substring). | Table or JSON. | Yes (read-only). |
| `issue open <key> [--url-only]` | positional `key`, `--url-only` | Computes URL `<base>/browse/<key>`. Without `--url-only`: launches via `open` crate. With: prints URL only (script/AI-friendly). No HTTP. | URL or browser launch. | Yes (no state change). |
| `issue link <k1> <k2> [--type T (default Relates)]` | positional, type defaults to "Relates" | Resolves link-type name via partial_match. `POST /rest/api/3/issueLink`. | Success or JSON. | No — creates a new link each time. |
| `issue unlink <k1> <k2> [--type T]` | positional, optional type filter | Lists links on k1, filters by k2 + optional type, `DELETE /rest/api/3/issueLink/<id>` for each match. | Count of removed or JSON. | Yes (re-running on already-unlinked = no-op since the filter empties out). |
| `issue link-types` | none | `GET /rest/api/3/issueLinkType`. | Table or JSON. | Yes (read-only). |
| `issue remote-link <key> --url X [--title Y]` | positional, `--url`, optional `--title` | `POST /rest/api/3/issue/<key>/remotelink`. Title defaults to URL. | `{"id": <u64>, "self": <url>}` JSON or text. | No — creates a new remote link each time. |
| `issue assets <key>` | positional | Fetches issue + CMDB fields. Extracts and enriches per-field linked assets (per-field GET `object/<key>` on the Assets API, paginated). | Table or JSON. | Yes (read-only). |

#### Project subsystem

| Command | Inputs | Effects | Output | Idempotent? |
|---|---|---|---|---|
| `project list` | `--type`, `--limit`, `--all` | `GET /rest/api/3/project/search` paginated. Optional type filter. | Table or JSON. | Yes. |
| `project fields` | none (uses `--project`/`.jr.toml`) | `GET /rest/api/3/project/<key>/statuses` + auxiliary calls. Discovers issue types, priorities, statuses, CMDB fields. | Multi-section table or JSON. | Yes. |

#### Board / Sprint / Worklog / Team / User / Queue

| Command | Inputs | Effects | Output | Idempotent? |
|---|---|---|---|---|
| `board list [--type T]` | type filter (`scrum`/`kanban`) | `GET /rest/agile/1.0/board`. | Table or JSON. | Yes. |
| `board view [--board ID] [--limit/--all]` | board override, limit | Resolves board (from `--board` or `.jr.toml::board_id`). `GET /rest/agile/1.0/board/<id>/issue`. | Table or JSON. | Yes. |
| `sprint list [--board ID]` | optional board | Resolves board, **errors out** if `board_type != "scrum"`. `GET /rest/agile/1.0/board/<id>/sprint`. | Table or JSON. | Yes. |
| `sprint current [--board ID] [--limit/--all]` | optional board | Same scrum check. Lists active sprints; uses first; lists its issues. | Table or JSON. | Yes. |
| `sprint add (--sprint ID \| --current) <issue...> [--board ID]` | positional issues, sprint or current, optional board | Caps at `MAX_SPRINT_ISSUES = 50`. `POST /rest/agile/1.0/sprint/<sprintId>/issue` with the keys. | `{sprint_id, issues, added: true}` JSON. | No (each call adds; idempotency comes from the API's behaviour, not the CLI). |
| `sprint remove <issue...>` | positional issues | Caps at 50. `move_issues_to_backlog` (POST to backlog endpoint). | `{issues, removed: true}` JSON. | Idempotent at API layer. |
| `worklog add <key> <duration> [-m message]` | positional, message | Parses `duration` via `duration::parse_duration` (using profile's hours-per-day/days-per-week — defaults 8/5). `POST /rest/api/3/issue/<key>/worklog`. | Worklog ID or JSON. | No (creates new worklog each call). |
| `worklog list <key>` | positional | `GET /rest/api/3/issue/<key>/worklog`. | Table or JSON. | Yes. |
| `team list [--refresh]` | refresh | Cache-first read (`teams.json`, 7-day TTL). On miss or refresh: GraphQL `tenantContexts` for orgId (lazy), then `GET /gateway/api/public/teams/v1/org/<orgId>/teams` (cursor-paginated). Cache-write. | Table or JSON. | Yes. |
| `user search <query> [--limit/--all]` | positional query | `GET /rest/api/3/user/search?query=<q>` paginated up to 1000-user hard cap. | Table or JSON. | Yes. |
| `user list -p <project> [--limit/--all]` | required project | `GET /rest/api/3/user/assignable/multiProjectSearch?projectKeys=<p>` paginated. | Table or JSON. | Yes. |
| `user view <accountId>` | positional | `GET /rest/api/3/user?accountId=<a>`. | Table or JSON. | Yes. |
| `queue list` | none (uses project) | Discovers service desk for the project (`cache::ProjectMeta::service_desk_id` or fetch). `GET /rest/servicedeskapi/servicedesk/<id>/queue` (`ServiceDeskPage`). | Table or JSON. | Yes. |
| `queue view [name] [--id ID] [--limit N]` | optional positional or `--id`, limit | Resolves queue (partial_match by name, or use `--id`). `GET /rest/servicedeskapi/servicedesk/<id>/queue/<qid>/issue` returns `QueueIssueKey[]`; followup `search_issues` for full payloads. | Table or JSON. | Yes. |

#### Assets subsystem

| Command | Inputs | Effects | Output | Idempotent? |
|---|---|---|---|---|
| `assets search <query> [--limit] [--attributes]` | positional AQL query, limit, include attributes | Resolves workspace ID (cache or API). `POST /jsm/assets/workspace/<wid>/v1/object/aql`. Auto-pagination, max-page=25, `AssetsPage::is_last` (bool-or-string tolerant). | Table or JSON. | Yes. |
| `assets view <key>` | positional, `--no-attributes` | Resolves workspace. If input is fully numeric → use as object-id; else → AQL key→id resolution. `GET object/<id>?includeAttributes=...`. Optionally enriches with `get_object_attributes` for human-readable attribute names. | Table (key/value attrs) or JSON. | Yes. |
| `assets tickets <key> [--limit] [--open] [--status]` | positional, mutually-exclusive `--open`/`--status` | Resolves workspace + object id. Connected-tickets endpoint. `--open` filters `status.colorName != "green"`. `--status` filters via partial_match on status names. | Table or JSON. | Yes. |
| `assets schemas` | none | `GET /jsm/assets/workspace/<wid>/v1/objectschema/list`. | Table or JSON. | Yes. |
| `assets types [--schema X]` | optional schema filter | Lists schemas (cache/API), filters, then for each: `GET /objectschema/<id>/objecttypes/flat`. | Table or JSON. | Yes. |
| `assets schema <name> [--schema X]` | required object-type name, optional schema scope | Discovers object type (partial_match by name). `GET /objecttype/<id>/attributes`. Cached as `cache::ObjectTypeAttrCache`. | Table (attributes definition) or JSON. | Yes. |

#### Top-level utility commands

| Command | Inputs | Effects | Output | Idempotent? |
|---|---|---|---|---|
| `me` | none | `GET /rest/api/3/myself`. | 3-row table or JSON. | Yes. |
| `api <path> [-X METHOD] [-d data] [-H header...]` | path, method, body, headers | Raw passthrough through `JiraClient::send_raw`. Accepts inline JSON / `@file` / `@-`. Status not parsed — caller sees raw response. Used by tests, debugging, and AI agents that need an escape hatch. | Raw response body to stdout. | Depends on the call. |
| `completion <shell>` | required positional | Generates shell completion script via `clap_complete`. No config, no auth (early-out at `main.rs:67`). | Script to stdout. | Yes. |

### 2b.2 Business rules

These are the constraints embedded in the implementation, with file:line citations.

1. **Sprint commands fail on kanban boards.** `cli/sprint.rs:79-86`: `if board_type != "scrum" { bail!("Sprint commands are only available for scrum boards. Board {} is a {} board.", ...) }`. Confirmed.
2. **Asset clause uses field NAME, not customfield ID** (`jql.rs:67-74`): clause emits `"<name>" IN aqlFunction("Key = \"<key>\"")`. The `(id, _name)` tuple's id is destructured-and-ignored (`|(_, name)|`).
3. **AQL `Key` attribute capitalization** (`jql.rs:70`): hardcoded `Key` (capital K). Not `objectKey`.
4. **`--open` filtering** uses two distinct mechanisms depending on context:
   - For Jira issues (`cli/issue/list.rs:303, 308, 625`): JQL clause `statusCategory != Done`. Status-category-key based; doesn't depend on colour.
   - For connected tickets (`cli/assets.rs:303-321`): client-side filter on `status.color_name != "green"`. Tickets with no status are *included* by `--open` (`.unwrap_or(true)`) but *excluded* by `--status`.
5. **`--no-input` auto-enables when stdin is not a TTY** (`main.rs:18-23`): `if !cli.no_input { use std::io::IsTerminal; if !std::io::stdin().is_terminal() { cli.no_input = true; } }`.
6. **`--no-color` and `NO_COLOR` env disable ANSI** (`main.rs:13-15`): `if cli.no_color || std::env::var("NO_COLOR").is_ok() { colored::control::set_override(false); }`.
7. **Per-profile cache reader/writer signature** (`cache.rs:16, 37, 90, 94, 187, 191, 222, 226, 249, 253, 289, ...`): every read/write fn takes `profile: &str` first. Convention enforced by signature, no compile-time fence — a free function added later that forgets to pass profile would compile.
8. **State-changing `issue move` is idempotent** (`workflow.rs:192-224`): if `current_status == target` (case-insensitive) OR if the input matches a transition name whose `to` equals current status, return `transitioned: false` and exit 0 with a "X is already in status Y" message. No HTTP write.
9. **`MAX_SPRINT_ISSUES = 50`** (`cli/sprint.rs:107, 35-41, 55-61`): per-call cap on `sprint add` and `sprint remove`.
10. **Resolution resolver does not auto-promote substring hits** (`workflow.rs:65-79`): `MatchResult::Ambiguous` always errors; only `Exact` (case-insensitive) auto-resolves. Project convention. Pinned by code comment: *"single-substring hits are not silently promoted to success — that would diverge from every other resolver in the codebase."*
11. **`partial_match` returns `Ambiguous` for single substring hits** (`partial_match.rs:39-42`, tests at 67-77, 99-106): single-substring is not `Exact`. Caller decides whether to prompt (TTY) or error (`--no-input`). Property-tested.
12. **`refresh_oauth_token` resolves credentials internally** (`api/auth.rs:705-712`, callers pass only `profile`). Explicit gotcha in CLAUDE.md; verified at source.
13. **Embedded OAuth uses fixed callback port 53682; BYO uses dynamic port 0** (`api/auth.rs:374-477`, `cli/auth.rs::login_oauth`). Documented in CLAUDE.md and ADR-0006.
14. **Embedded OAuth presence check does not decode** (`api/auth_embedded.rs:132-136`, defense-in-depth): `embedded_oauth_app_present()` reads `EMBEDDED_ID.is_some_and(|s| !s.is_empty())` etc. without ever invoking `decode()`. Used by `jr auth status` and `peek_oauth_app_source`.
15. **`embedded_oauth_app` rejects empty strings/zero-length ciphertext at build time** (`api/auth_embedded.rs:100-106`): a build pipeline that sets `JR_BUILD_OAUTH_CLIENT_ID=""` produces a `None` (BYO fallback), not an empty-credential ship.
16. **`Debug` for `EmbeddedOAuthApp` redacts `client_secret`** (`api/auth_embedded.rs:34-41`): pinned by `embedded_oauth_app_debug_redacts_secret` test.
17. **OAuth scopes are pinned** (`api/auth.rs:58-63`): the seven default scopes, joined via `concat!` so double-spaces are visually obvious. Pinned by regression test.
18. **Auto-enable `--assets` when `--asset` is set** (`cli/issue/list.rs:87`): `let show_assets = show_assets || asset_key.is_some();` — filtering implies displaying.
19. **Date validators run before any HTTP** (`cli/issue/list.rs:90-114`): early-return on bad input, so a typo doesn't cost a network call. Same pattern for `--recent`/duration.
20. **Profile-name validation gates three boundaries** (`config.rs:113-140`, `main.rs:62`, `config.rs:274-282, 304`): CLI flag → TOML keys → resolved active name. All three reject `[A-Za-z0-9_-]{1,64}` + Windows reserved.
21. **Migration write-back uses file-only data, not env-overlaid** (`config.rs:240-264`): so transient `JR_*` vars don't bleed into `config.toml`. Comment explicit at `config.rs:241-247`.
22. **`Config::active_profile_or_err` returns `&ProfileConfig` (borrowed, hot path); `active_profile()` returns owned `ProfileConfig` (cold path)** — Pass 1 §8 deviation #8.
23. **`statusCategory != Done` is the canonical "open" predicate** (`cli/issue/list.rs`): not `status != Done`, because Jira instances customise status names but not category keys.
24. **Comment `--internal` adds `sd.public.comment` property** (`api/jira/issues.rs:181-198` add_comment signature `internal: bool`): no-op on non-JSM projects (Jira ignores the property silently). Documented in CLAUDE.md.
25. **JR_BASE_URL completely overrides the active profile's URL** (`config.rs:351-353`, `client.rs:37-65`): intended only for tests/power users. Bypasses URL field on the profile.
26. **Refresh of `auth refresh` is destructive**: clears keychain entries + cache. Clear-and-relogin (per Pass 1 §8 deviation #4); not "use refresh token grant".

### 2b.3 State machines

#### OAuth token lifecycle

```
                           re-login
                ┌───────────────────────────────┐
                │                                │
                ▼                                │
       ┌────────────────┐  jr auth login    ┌────┴────────┐
       │ needs-login    ├──────────────────▶│ authenticated│
       │ (no keychain   │  (write tokens to │ (access ok,  │
       │  entries)      │   <profile>:oauth-*)│  refresh ok)│
       └────────────────┘                    └────┬────────┘
                ▲                                 │
                │                                 │ access expires
                │                                 ▼
                │                          ┌──────────────┐
                │                          │ access-expired,│
                │                          │ refresh ok   │
                │                          └────┬─────────┘
                │                               │
                │                               │ refresh_oauth_token (deferred)
                │                               │ OR re-login (current cli flow)
                │                               ▼
                │                          ┌──────────────┐
                │  jr auth refresh         │ refreshed    │
                │  (clear + relogin)       │ (new tokens) │
                │                          └──────────────┘
                │                                 ▲
                │                                 │
                │         partial-state           │
                │     (one of namespaced pair      │
                │      missing) → user error       │
                │      OR legacy→namespaced         │
                │      auto-migrate for "default"   │
                │                                  │
                └──────────────────────────────────┘
```

Verified against `api/auth.rs:111-169`. Per-profile namespacing is `<profile>:oauth-*-token`. Legacy lazy-migrate runs only for `"default"` profile and only on read of an absent or partially-present namespaced pair. The "refreshed" state is reached via a cleared-and-relogin path because `refresh_oauth_token` has no production callers (Pass 1 §8 deviation #4).

#### Cache entry lifecycle

```
   ┌────────┐  read_cache returns Ok(None) / NotFound
   │ miss   │◀───────────────────────────────┐
   └────┬───┘                                │
        │ network fetch                      │
        ▼                                    │
   ┌──────────┐  write_cache (atomic JSON)   │
   │ writing  │                              │
   └────┬─────┘                              │
        ▼                                    │
   ┌──────────┐                              │
   │ hit-fresh│  read_cache returns Ok(Some) │
   │ (<7d)    │                              │
   └────┬─────┘                              │
        │ time elapses                       │
        ▼                                    │
   ┌──────────┐  read_cache returns Ok(None) │
   │ stale    ├──────────────────────────────┤
   │ (≥7d)    │                              │
   └──────────┘                              │
                                             │
   ┌──────────┐  read_cache returns Ok(None) │
   │ corrupt  │  + stderr warning            │
   │ (parse-  ├──────────────────────────────┘
   │  fail)   │
   └──────────┘
```

Verified at `cache.rs:14-34`. Stale and corrupt are indistinguishable to callers (both return `Ok(None)`); the difference is only the stderr line.

#### Issue transition flow (per `issue move`)

```
   ┌──────────────────┐
   │ command parsed   │
   └────┬─────────────┘
        ▼
   ┌──────────────────┐  GET /transitions, GET /issue
   │ fetch state      │
   └────┬─────────────┘
        │
        ▼
   ┌─────────────────────────┐
   │ current == target?      │  yes → exit 0, transitioned: false
   └────┬────────────────────┘  (no HTTP write)
        │ no
        ▼
   ┌──────────────────┐
   │ resolve target   │  partial_match(target,
   │ (transition name │   transitions ∪ to-status names)
   │  + to-status     │
   │  unified pool)   │
   └────┬─────────────┘
        │
        │ Exact / ExactMultiple → use; Ambiguous → prompt or error;
        │ None → bail with candidate list
        ▼
   ┌──────────────────┐  optional load_resolutions for --resolution
   │ resolve resoln   │
   │ (no auto-promote │
   │  on substring)   │
   └────┬─────────────┘
        ▼
   ┌──────────────────┐  POST /transitions
   │ transition       │  with optional fields.resolution
   └────┬─────────────┘
        │
        │ 400 with "resolution" + "required" → user-facing hint
        │ rewriting (workflow.rs:357-377)
        ▼
   ┌──────────────────┐
   │ exit 0, success  │
   └──────────────────┘
```

Verified at `cli/issue/workflow.rs:140-398`.

#### Profile lifecycle

```
   nonexistent
       │ jr auth login --profile NEW (or jr init for "default")
       │   • creates [profiles.NEW] in config.toml
       │   • writes <NEW>:oauth-* to keychain (or shared api-token)
       ▼
   exists, inactive
       │ jr auth switch NEW
       │   • sets default_profile = NEW
       ▼
   active
       │ jr auth switch OTHER             jr auth logout --profile NEW
       │   • default_profile = OTHER       • deletes <NEW>:oauth-* from keychain
       │                                    • config entry stays
       ▼                                  ▼
   inactive again                       inactive, no tokens
       │ jr auth remove NEW                    │ jr auth login --profile NEW
       │   • config entry removed              │   • re-acquire tokens
       │   • <NEW>:oauth-* deleted             ▼
       │   • cache/v1/<NEW>/ deleted        active again
       ▼   • errors if NEW == active
   nonexistent (back to start)
```

Verified at `cli/auth.rs:540-1180` (login/refresh/switch/list/logout/remove handlers).

#### Build-time embedded OAuth state machine

```
   build start
       │
       ▼
   ┌────────────────────────────────┐
   │ JR_BUILD_OAUTH_CLIENT_ID set?  │
   │ JR_BUILD_OAUTH_CLIENT_SECRET   │
   │ set?                           │
   └────┬───────────────────────────┘
        │
        │ both set:                          either missing:
        │  • generate 32-byte random key      • emit OUT_DIR/embedded_oauth.rs
        │    (Unix: /dev/urandom;              with EMBEDDED_ID = None,
        │     Windows: BCryptGenRandom         EMBEDDED_SECRET_XOR = None,
        │     FFI shim, no extra crates)       EMBEDDED_SECRET_KEY = None
        │  • XOR-obfuscate secret             • binary uses BYO/prompt path
        │  • emit OUT_DIR/embedded_oauth.rs
        ▼
   binary
       │ embedded_oauth_app() called
       ▼
   ┌────────────────────────────────┐
   │ build_embedded_app(id, xor, key)│
   │  • all None → None             │
   │  • any empty → None            │
   │  • else: XOR-decode → Some     │
   │  • cached in OnceLock          │
   └────────────────────────────────┘
```

Verified at `build.rs` (per Pass 0 §4 + §5) and `auth_embedded.rs:1-136`.

### 2b.4 Events

There is no domain-event bus. Observable signals are stderr lines, mostly verbose-gated. Catalogued:

- **HTTP request log line** (`api/client.rs:197-204`, per Pass 1 §3c): `[verbose] METHOD URL` plus body when `--verbose`. Stderr.
- **Rate-limit retry log line** (`api/client.rs:220-225, 294-300`): `[verbose] Rate limited (429). Retrying in Ns (attempt M/3)`. Stderr.
- **Rate-limit-exhausted warning** (`api/client.rs`, after retries): `warning: rate limited by Jira — gave up after 3 retries.` Stderr (always, not verbose-gated).
- **Cache-corruption warning** (`cache.rs:26, 128, 159`): `warning: cache file <name> unreadable (<err>); will refetch`. Stderr (always — not verbose-gated, because cache corruption is a real anomaly).
- **Resolutions-without-id warning** (`cli/issue/workflow.rs:128-132`): `warning: N resolution(s) lacked an id and were not cached`. Stderr.
- **Team-field shape warning** (`types/jira/issue.rs:101-131`): `[verbose] team field "<id>" has unexpected shape (got <kind>). Expected string UUID or object with string "id".` Stderr; once-per-process gate.
- **Date/changelog parse-failure** (via `observability::log_parse_failure_once`): `[verbose] <site> timestamp failed to parse: <iso>`. One per call-site per process. Used in `cli/issue/changelog.rs` and `cli/issue/format.rs::format_comment_date`.
- **Config-migration notice** (`config.rs:260-263`): `Migrated config to multi-profile layout (single profile "default"). Run 'jr auth list' to view profiles.` Always emitted on first migration.
- **Ctrl+C interrupt notice** (`main.rs:264`): `\nInterrupted` then `process::exit(130)`.
- **Fatal error log** (`main.rs:34-49`): either structured `{"error": ..., "code": ...}` JSON on stderr (when `--output json`) or `Error: <message>` plain text.

`observability.rs` (39 LOC) is the only "module" exposing an event helper, and it's `pub(crate)`. The codebase's stance (per `observability.rs:1-6`): *"Intentionally tiny: the project has no tracing/log crate, and a single `--verbose`-gated `eprintln!` is the established pattern."* The function `log_parse_failure_once(flag, site, iso, verbose)` embeds the once-per-process gate via a caller-supplied `&AtomicBool` so each parser fires at most one line per run. There are NO domain events emitted on profile switch, cache invalidation, OAuth refresh, or rate-limit retry beyond the verbose stderr lines above. There is NO HTTP-response log line — only requests are logged.

### 2b.5 Critical invariants (testable claims, draft for Pass 3)

These are domain-meaningful claims grounded in code (not type-system trivia). Each carries a confidence level and the source location.

1. **(HIGH)** `jr issue move FOO-123 "In Progress"` exits 0 with `transitioned: false` if FOO-123 is already in "In Progress" — no HTTP transition is issued. Source: `cli/issue/workflow.rs:192-224`. Pinned by integration tests (need to identify).
2. **(HIGH)** `partial_match` never returns `Exact` for a single substring hit — single-substring routes through `Ambiguous`. Source: `partial_match.rs:39-42, 67-77`. Property-tested.
3. **(HIGH)** `jql::escape_value` always produces output where every double-quote is preceded by an odd number of backslashes (no unescaped quote). Source: `jql.rs:6-8`, property test at `jql.rs:383-394`.
4. **(HIGH)** `jql::build_asset_clause` uses the human-readable field NAME (not `customfield_NNNNN`) in the `aqlFunction` LHS, and uses the literal `Key` (capital K) as the AQL attribute. Source: `jql.rs:67-74`, unit tests `build_asset_clause_*`.
5. **(HIGH)** `jql::validate_duration("4w2d")` is an error; combined units are rejected. `jql::validate_duration("d7")` is also an error; reversed order rejected. Source: `jql.rs:16-33`, tests at 228-235.
6. **(HIGH)** `duration::parse_duration` accepts combined units `1w2d3h30m` (unlike the JQL validator). Source: `duration.rs:5-49`, test `test_complex` at 90-95.
7. **(HIGH)** `Config::load_with(Some(name))` precedence is `name` > `JR_PROFILE` env > config `default_profile` > `"default"`. Source: `config.rs:95-110, 220-300`.
8. **(HIGH)** `validate_profile_name` rejects `foo:bar`, `../etc/passwd`, `CON`, `NUL`, empty, >64 chars, and any non-`[A-Za-z0-9_-]` character. Source: `config.rs:113-140`.
9. **(HIGH)** Cache reads return `Ok(None)` on missing / expired (>7d) / corrupt files; never `Err` for those cases. Source: `cache.rs:14-34, 119-143, 287-329`.
10. **(HIGH)** `cache::clear_profile_cache(name)` is no-op when `<root>/v1/<name>/` does not exist. Source: `cache.rs:82-88`.
11. **(HIGH)** `auth login --profile <NEW>` writes per-profile `<NEW>:oauth-access-token` and `<NEW>:oauth-refresh-token` keys (not flat). Source: `api/auth.rs:88-97`.
12. **(HIGH)** OAuth load for non-`"default"` profile NEVER inherits legacy flat keys. Source: `api/auth.rs:111-169`, tested by absence (no `default` literal in the legacy fallback branches when `profile != "default"`).
13. **(HIGH)** `embedded_oauth_app_present()` returns false in any build that did NOT have `JR_BUILD_OAUTH_CLIENT_ID` and `JR_BUILD_OAUTH_CLIENT_SECRET` set at compile time, AND does NOT decode the secret. Source: `api/auth_embedded.rs:132-136`, tests at 244-249.
14. **(HIGH)** `EmbeddedOAuthApp::Debug` never includes `client_secret`. Source: `api/auth_embedded.rs:34-41`, test `embedded_oauth_app_debug_redacts_secret`.
15. **(HIGH)** Build with `JR_BUILD_OAUTH_CLIENT_ID=""` produces a binary with `embedded_oauth_app() == None`. Source: `api/auth_embedded.rs:100-106`, test `build_embedded_app_rejects_empty_inputs`.
16. **(HIGH)** `IssueFields::story_points("customfield_X")` returns `None` for a present-but-non-numeric value (e.g., `"not a number"`). Source: `types/jira/issue.rs:83-85`, test at 362-366.
17. **(HIGH)** `IssueFields::team_id` accepts both string-UUID and `{"id": <string>}` shapes; rejects `{"id": <number>}` (no coercion). Source: `types/jira/issue.rs:101-131`, tests at 318-327.
18. **(HIGH)** `Resolution` deserializes both the `{name}` shape (from `issue.fields.resolution`) and the `{id, name, description}` shape (from `/resolution`). Source: `types/jira/issue.rs:174-185`, tests at 600-624.
19. **(HIGH)** `LinkedAsset::display()` falls back to `#<id> (run "jr init" to resolve asset names)` when only `id` is present. Source: `types/assets/linked.rs:19-44`, test `display_id_fallback_with_hint`.
20. **(MEDIUM)** Connected-tickets `--open` filter uses `colorName != "green"` (string compare on the raw API token), not status-category-key. Source: `cli/assets.rs:303-321`.
21. **(MEDIUM)** Issue-list `--open` filter uses JQL `statusCategory != Done`, not colour-based. Source: `cli/issue/list.rs:303, 308, 625`.
22. **(MEDIUM)** `sprint add` and `sprint remove` cap at `MAX_SPRINT_ISSUES = 50` per call; exceeding errors with explicit message. Source: `cli/sprint.rs:35-41, 55-61, 107`.
23. **(MEDIUM)** `sprint list/current` against a kanban board errors with explicit "Sprint commands are only available for scrum boards" message. Source: `cli/sprint.rs:79-86`.
24. **(MEDIUM)** Date filter validation runs before any HTTP call. A bad `--created-after` value never costs a network request. Source: `cli/issue/list.rs:90-114`.
25. **(MEDIUM)** `--no-input` is auto-set when stdin is not a TTY (pipe, AI agent, scripts). Source: `main.rs:18-23`.

### 2b.6 Cross-cutting flows (sequence narratives)

**Flow 1 — First-time user runs `jr init`, configuring the default profile end-to-end.**

A user runs `jr init`. Because `init` is special-cased in `main.rs:77` (`cli::Command::Init => cli::init::handle().await`), no `Config::load_with` is called and no `JiraClient` is built before the handler runs. `init::handle()` enters interactive mode: it prompts for the Jira instance URL and the auth method (API token or OAuth). For OAuth, it routes through `cli::auth::login_oauth` which resolves the credential source via the chain flag → env → keychain → embedded → prompt. With the embedded `jr` app present (release builds), `OAuthAppSource::Embedded` is selected and the fixed-port `127.0.0.1:53682` strategy is used. The flow opens the browser, the user consents on Atlassian, the local listener accepts a single redirect, validates the `state` against CSRF, exchanges the code for `(access_token, refresh_token)` at `auth.atlassian.com/oauth/token`, then queries `api.atlassian.com/oauth/token/accessible-resources` to discover the `cloud_id`. Tokens are written to `keyring` as `default:oauth-access-token` and `default:oauth-refresh-token`. `init` then runs the org-discovery GraphQL `tenantContexts` query (ADR-0005) to resolve `org_id`, fetches teams via `GET /gateway/api/public/teams/v1/org/<orgId>/teams` (cursor-paginated) and writes `cache::TeamCache` to `~/.cache/jr/v1/default/teams.json`. Story-points field discovery runs `client.find_story_points_field()` (queries `/rest/api/3/field`) and writes the resulting `customfield_NNNNN` to `[profiles.default].story_points_field_id`. Final `config.toml` save uses file-only baseline (no env overlay) so transient `JR_*` env vars don't leak. The user sees a summary and the binary exits 0.

**Flow 2 — Power user runs `jr --profile sandbox issue list ENG --sprint "current" --points --asset CUST-5`.**

`main.rs::run` validates `cli.profile = Some("sandbox")` via `validate_profile_name`. Loads `Config::load_with(Some("sandbox"))`: figment merges defaults + `~/.config/jr/config.toml` + `JR_*` env, runs migration if needed, validates every `[profiles.*]` key, walks up cwd looking for `.jr.toml`, resolves active profile name (`"sandbox"` because the flag wins precedence). The strict load asserts `"sandbox"` is present in `[profiles]` or errors `UserError("unknown profile: sandbox")` (exit 64). Builds `JiraClient::from_config(&config, verbose=false)`: reads `[profiles.sandbox].auth_method`; if `oauth`, calls `api::auth::load_oauth_tokens("sandbox")` which reads `sandbox:oauth-access-token` from keychain (no legacy fallback for non-default profile) and stamps the `Authorization: Bearer ...` header. Dispatches into `cli::issue::handle(Box::new(IssueCommand::List{...}), ...)`. The list handler:

1. Runs `validate_duration("...")` if `--recent` was set; runs `validate_date(...)` for each `--created-/--updated-after/-before` flag. Early-out on bad input.
2. Sees `--asset CUST-5` is set, so auto-enables `--assets` (display column).
3. Calls `api::assets::linked::get_or_fetch_cmdb_fields(client)`. This in turn calls `cache::read_cmdb_fields_cache("sandbox")`. Cache hit: returns `Vec<(id, name)>`. Cache miss: `client.list_fields()` → filter for CMDB types → `cache::write_cmdb_fields_cache("sandbox", ...)`.
4. `jql::build_asset_clause("CUST-5", &cmdb_fields)` produces `"<field_name>" IN aqlFunction("Key = \"CUST-5\"")` (or parenthesized OR-join if multiple CMDB fields exist).
5. Composes the final JQL: project scope (`project = "ENG"`), `--jql` content stripped of `ORDER BY`, `--asset` clause, all four date clauses, `--open` → `statusCategory != Done`, `--status` partial-match, `--assignee`/`--reporter` (with `me` resolution via `client.get_myself` if applicable), `--team` resolution, `--recent`, finally `ORDER BY updated DESC`.
6. Cursor-paginates `POST /rest/api/3/search/jql` (`CursorPage<Issue>`) up to `limit` (default 30, or all if `--all`). On each page: 429 retries via Retry-After (max 3 retries, default 1s). On 401 + body "scope does not match" → `JrError::InsufficientScope` (exit 2 with workaround instructions).
7. Per row: extracts story points via `IssueFields::story_points(field_id)`; extracts and enriches linked assets via `extract_linked_assets_per_field(&issue.fields.extra, &cmdb_fields)` then per-field `client.get_asset(workspace_id, ...)`. Workspace ID comes from `cache::read_workspace_cache("sandbox")` or fresh fetch.
8. `cli::issue::format::format_issue_rows_public` builds rows; `output::print_output` renders table or JSON.

**Flow 3 — `jr issue view PROJ-123 --linked-assets` (issue view with full asset enrichment).**

(Note: there's no separate `--linked-assets` flag on `issue view` — assets enrichment in view is opportunistic when the issue carries CMDB-field values. The flow holds for `jr issue assets PROJ-123` too.)

Loads config, builds client (same as Flow 2). Dispatches to `cli::issue::view::handle_view`. Handler:

1. Calls `client.get_issue("PROJ-123", &[])` (`GET /rest/api/3/issue/PROJ-123`) — returns `Issue` with full `IssueFields::extra` populated.
2. Calls `api::assets::linked::get_or_fetch_cmdb_fields(client)` — cache or API fetch (same path as Flow 2 step 3).
3. Calls `extract_linked_assets_per_field(&issue.fields.extra, &cmdb_fields)` — for each `(field_id, field_name)` tuple, looks up `extra[field_id]`. The value can be a JSON array of `{id, key, name, ...}` objects, a single object, or null. The extractor produces `Vec<(String, Vec<LinkedAsset>)>` (per-field linked assets).
4. For each `LinkedAsset`, calls `cache::read_workspace_cache("sandbox")` once (lazy) to get `workspace_id`. On miss, `GET /rest/servicedeskapi/assets/workspace` (NOT the api.atlassian.com proxy — this lives on the Jira instance). Per Pass 1 §3.4f, may surface `JrError::UserError` with "Assets not available on this Jira site (requires JSM Premium/Enterprise)" on 403/404.
5. For each `LinkedAsset` with a numeric `id`, calls `client.get_asset(workspace_id, &id, true)` (`GET /jsm/assets/workspace/<wid>/v1/object/<id>?includeAttributes=true`). Returns `AssetObject` with full attribute payload.
6. Renders: text view by section, or JSON via `output::render_json`. The display uses `LinkedAsset::display()` formatters for tables, `AssetObject` direct for detail. ADF body of the issue is rendered through `adf::adf_to_text`.

The flow demonstrates the full cross-context coupling: Jira context → cache → Assets context → cache → Assets API. The cache hits dominate — first-time runs incur 3+ extra HTTP calls (CMDB field discovery, workspace discovery, per-asset enrichment), warm runs incur only the per-asset enrichment.

---

## State Checkpoint

```yaml
pass: 2
status: complete
sub_pass_2a_entities: 51        # Jira(28) + JSM(3) + Assets(15) + Auth(5)
sub_pass_2a_value_objects: 19   # incl. enums, pagination shapes, constants
sub_pass_2b_operations: 39      # CLI subcommands cataloged
sub_pass_2b_invariants_drafted: 25
files_examined: 21
timestamp: 2026-05-04T01:30:00Z
next_pass: 3
inputs_consumed:
  - .factory/semport/jira-cli/jira-cli-pass-0-inventory.md
  - .factory/semport/jira-cli/jira-cli-pass-1-architecture.md
  - .reference/jira-cli/CLAUDE.md (treated as hypothesis, deviations noted)
  - .reference/jira-cli/src/types/jira/issue.rs (full)
  - .reference/jira-cli/src/types/jira/board.rs (full)
  - .reference/jira-cli/src/types/jira/sprint.rs (full)
  - .reference/jira-cli/src/types/jira/user.rs (full)
  - .reference/jira-cli/src/types/jira/team.rs (full)
  - .reference/jira-cli/src/types/jira/project.rs (full)
  - .reference/jira-cli/src/types/jira/worklog.rs (full)
  - .reference/jira-cli/src/types/jira/changelog.rs (full)
  - .reference/jira-cli/src/types/jira/mod.rs (full)
  - .reference/jira-cli/src/types/jsm/queue.rs (full)
  - .reference/jira-cli/src/types/jsm/servicedesk.rs (full)
  - .reference/jira-cli/src/types/jsm/mod.rs (full)
  - .reference/jira-cli/src/types/assets/object.rs (full)
  - .reference/jira-cli/src/types/assets/linked.rs (full)
  - .reference/jira-cli/src/types/assets/schema.rs (full)
  - .reference/jira-cli/src/types/assets/ticket.rs (full)
  - .reference/jira-cli/src/types/assets/mod.rs (full)
  - .reference/jira-cli/src/error.rs (full)
  - .reference/jira-cli/src/config.rs (head, ~350 LOC of 1223)
  - .reference/jira-cli/src/cache.rs (head, ~300 LOC of 899)
  - .reference/jira-cli/src/jql.rs (full)
  - .reference/jira-cli/src/duration.rs (full)
  - .reference/jira-cli/src/cli/mod.rs (full)
  - .reference/jira-cli/src/cli/sprint.rs (head, ~150 LOC)
  - .reference/jira-cli/src/cli/issue/workflow.rs (head, ~450 LOC of 788)
  - .reference/jira-cli/src/cli/issue/list.rs (head, ~120 LOC of 1083)
  - .reference/jira-cli/src/cli/issue/format.rs (head, ~120 LOC)
  - .reference/jira-cli/src/cli/assets.rs (lines 295-360)
  - .reference/jira-cli/src/observability.rs (full)
  - .reference/jira-cli/src/partial_match.rs (full)
  - .reference/jira-cli/src/api/auth.rs (head, ~200 LOC of 1397)
  - .reference/jira-cli/src/api/auth_embedded.rs (full)
  - .reference/jira-cli/src/api/assets/objects.rs (head, ~120 LOC)
  - .reference/jira-cli/src/main.rs (full)
```
