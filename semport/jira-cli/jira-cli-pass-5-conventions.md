# Pass 5: Convention & Pattern Catalog — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Builds on: Pass 0 (inventory), Pass 1 (architecture), Pass 2 (domain model), Pass 3 (behavioral contracts), Pass 4 (NFR catalog).

> **Method.** Every convention claim is grounded in a `<file>:<line>` citation, a counted occurrence, or a directory listing. CLAUDE.md is treated as a hypothesis (the project's own claim about itself) and verified against source. Where a convention is universal (e.g., snake_case modules), I quantify the consistency rather than asserting "all". Where the codebase intentionally has a small surface or a single outlier, that's stated as such instead of padded.

---

## 1. Naming conventions

### 1.1 Module names (`*.rs` filenames)

**Rule:** snake_case, **plural for collections of API resources**, **singular for individual concepts / engines / utilities**.

| Path | Plural? | Justification |
|---|---|---|
| `api/jira/issues.rs`, `boards.rs`, `sprints.rs`, `users.rs`, `links.rs`, `worklogs.rs`, `projects.rs`, `statuses.rs`, `teams.rs`, `fields.rs`, `resolutions.rs` | plural | Each file is the API binding for one Jira REST *resource collection* (the resource itself has many instances). |
| `api/jsm/queues.rs`, `servicedesks.rs` | plural | Same rule. |
| `api/assets/objects.rs`, `schemas.rs`, `tickets.rs` | plural | Same rule. |
| `api/assets/linked.rs`, `workspace.rs` | singular | Conceptual operations (linked-asset *extraction*; workspace ID *discovery*) — not REST collections. |
| `api/client.rs`, `auth.rs`, `auth_embedded.rs`, `pagination.rs`, `rate_limit.rs` | singular | Engine / cross-cutting modules. |
| `cli/issue.rs` (mod), `cli/board.rs`, `cli/sprint.rs`, `cli/team.rs`, `cli/user.rs`, `cli/queue.rs`, `cli/worklog.rs`, `cli/project.rs`, `cli/api.rs`, `cli/assets.rs`, `cli/auth.rs`, `cli/init.rs` | singular | Command modules — `jr <noun>` is the user-facing verb-form, so the module is the singular noun. **Confirms** singular for command modules (an outlier from the plural-for-collections rule). |
| `cli/issue/{format,helpers,view,list,workflow,changelog,comments,create,assets,links,json_output}.rs` | mostly singular | All concept modules. `links.rs` is the lone "plural" exception, mirroring the `api/jira/links.rs` resource convention (issue-link operations resource). |
| `types/jira/{issue,board,sprint,user,worklog,team,changelog,project}.rs` | singular | Type definitions — one struct family per file. |
| `types/jsm/{queue,servicedesk}.rs` | singular | Same. |
| `types/assets/{linked,object,schema,ticket}.rs` | singular | Same. |
| `cache.rs`, `config.rs`, `error.rs`, `output.rs`, `adf.rs`, `jql.rs`, `duration.rs`, `partial_match.rs`, `observability.rs`, `main.rs`, `lib.rs`, `build.rs` | singular | All single-concept utility / cross-cutting modules. |

**Cross-context check:** the `api/<product>/` files are plural (resource collections); the `types/<product>/` files are singular (single struct family per file). This is a deliberate split.

**Consistency:** HIGH. All 80 source files conform; no `*_helpers.rs` / `*_utils.rs` / `*_mod.rs` cargo-cult anti-patterns. The `_` is used in `auth_embedded.rs`, `partial_match.rs`, `rate_limit.rs`, `json_output.rs` for compound concepts.

### 1.2 Type names

**Rule:** `PascalCase` (Rust standard).

Spot-checked across all type modules: `Issue`, `IssueFields`, `IssueType`, `Project`, `ProjectSummary`, `ProjectLead`, `User`, `Status`, `StatusCategory`, `Resolution`, `Component`, `Version`, `Comment`, `Transition`, `IssueLink`, `LinkedIssue`, `IssueLinkType`, `Board`, `BoardLocation`, `BoardConfig`, `Sprint`, `Worklog`, `ChangelogEntry`, `ChangelogItem`, `TeamEntry`, `TenantContext`, `ServiceDesk`, `Queue`, `QueueIssueKey`, `AssetObject`, `ObjectType`, `AssetAttribute`, `ObjectAttribute`, `ObjectTypeAttributeDef`, `DefaultType`, `ReferenceType`, `ReferenceObjectType`, `ObjectAttributeValue`, `ObjectSchema`, `ObjectTypeEntry`, `LinkedAsset`, `ConnectedTicket`, `TicketStatus`, `TicketType`, `TicketPriority`, `EmbeddedOAuthApp`, `OAuthAppSource`, `LoginArgs`, `RefreshArgs`, `Config`, `GlobalConfig`, `ProfileConfig`, `ProjectConfig`, `FieldsConfig`, `InstanceConfig`, `DefaultsConfig`, `JrError`, `JiraClient`, `OffsetPage`, `CursorPage`, `ServiceDeskPage`, `AssetsPage`, `RateLimitInfo`, `OutputFormat`, `MatchResult`, `HttpMethod`, `Cli`, `Command`, all `*Command` clap enums.

**Consistency:** HIGH. **Zero deviations** observed across the type/CLI/config/error surface. `OAuthAppSource` and `JrError` do use `OAuth` as a single token (not `Oauth`) — consistent with Rust ecosystem norms.

### 1.3 Function names

**Rule:** `snake_case`.

Verified by sampling all `pub fn` and `pub async fn` declarations in the read files: `from_config`, `new_for_test`, `search_issues`, `get_issue`, `transition_issue`, `add_comment`, `list_comments`, `find_story_points_field_id`, `list_link_types`, `extract_linked_assets_per_field`, `get_or_fetch_workspace_id`, `get_or_fetch_cmdb_fields`, `oauth_login`, `refresh_oauth_token`, `embedded_oauth_app`, `embedded_oauth_app_present`, `peek_oauth_app_source`, `validate_profile_name`, `escape_value`, `validate_duration`, `validate_asset_key`, `validate_date`, `build_asset_clause`, `strip_order_by`, `parse_duration`, `format_duration`, `read_cache`, `write_cache`, `clear_profile_cache`, `cache_dir`, `print_output`, `print_success`, `print_warning`, `print_error`, `render_table`, `render_json`, `log_parse_failure_once`, `resolve_active_profile_name`, `resolve_effective_limit`, `resolve_credential`, `extract_error_message`, `handle_*` per CLI command.

**Consistency:** HIGH. No camelCase function names found.

### 1.4 Constants

**Rule:** `SCREAMING_SNAKE_CASE`.

Verified inventory (from `awk` over all `*.rs` in src/):

| Constant | Type | Location | Purpose |
|---|---|---|---|
| `CACHE_TTL_DAYS` | `i64` | `src/cache.rs:7` | 7-day TTL for all caches. |
| `OAUTH_APP_HINT` | `&str` | `src/cli/auth.rs:258` | User-facing hint string. |
| `REFRESH_HELP_LINE` | `&str` | `src/cli/auth.rs:803` | Keychain ACL hint. |
| `MAX_SPRINT_ISSUES` | `usize` | `src/cli/sprint.rs:107` | Atlassian Agile cap (50). |
| `MAX_RETRIES` | `u32` | `src/api/client.rs:11` | 429-retry cap (3). |
| `DEFAULT_RETRY_SECS` | `u64` | `src/api/client.rs:14` | Fallback 429 backoff (1s). |
| `DEFAULT_SERVICE_NAME` | `&str` | `src/api/auth.rs:8` | Keychain service ("jr-jira-cli"). |
| `KEY_EMAIL` | `&str` | `src/api/auth.rs:19` | Flat keychain key. |
| `KEY_API_TOKEN` | `&str` | `src/api/auth.rs:20` | Flat keychain key. |
| `KEY_OAUTH_ACCESS_LEGACY` | `&str` | `src/api/auth.rs:24` | Legacy flat OAuth key. |
| `KEY_OAUTH_REFRESH_LEGACY` | `&str` | `src/api/auth.rs:25` | Legacy flat OAuth key. |
| `DEFAULT_OAUTH_SCOPES` | `&str` (concat!) | `src/api/auth.rs:58-63` | Pinned 7-scope list. |
| `EMBEDDED_CALLBACK_PORT` | `u16` | `src/api/auth.rs:384` | OAuth callback port (53682). |
| `NULL_GLYPH` | `&str` | `src/cli/issue/changelog.rs:13` | "—" for empty cells. |
| `SYSTEM_AUTHOR` | `&str` | `src/cli/issue/changelog.rs:14` | "(system)" placeholder. |
| `USER_PAGE_SIZE` | `u32` | `src/api/jira/users.rs:8` | 100 per page. |
| `USER_PAGINATION_SAFETY_CAP` | `u32` | `src/api/jira/users.rs:16` | Hard cap (15 pages × 100 = 1,500). |
| `KNOWN_SP_SCHEMA_TYPES` | `&[&str]` | `src/api/jira/fields.rs:45` | Story-points custom-field schema list. |
| `CMDB_SCHEMA_TYPE` | `&str` | `src/api/jira/fields.rs:83` | CMDB custom-field schema discriminator. |
| `BASE_ISSUE_FIELDS` | `&[&str]` | `src/api/jira/issues.rs:12` | Default Jira fields requested per issue. |
| `DEFAULT_LIMIT` | `u32` | `src/cli/mod.rs:740` | Default `--limit` (30). |

**Consistency:** HIGH. 21 named constants, all SCREAMING_SNAKE_CASE. No `lowercase_const` or `kCamelCase` artefacts.

### 1.5 Test function names

**Two patterns coexist** — confirmed by quantification.

Counted across `tests/` (320 named test fns in integration files):
- `test_<verb>_<subject>[_<expected>]` prefix-style: **108** functions (~34%).
- `<subject>_<verb>_<expected>` no-prefix-style: **212** functions (~66%).

Counted across inline unit tests in `src/` (sampled, similar distribution):
- The newer files (changelog.rs, json_output.rs, auth_embedded.rs, sprint.rs) overwhelmingly use the no-prefix style.
- Older files (issue_commands.rs, cli_smoke.rs, api_client.rs) use `test_*` prefix.

Examples (no-prefix style — DOMINANT):
- `assets_search_server_error_surfaces_friendly_message` (`tests/assets_errors.rs`)
- `assets_search_unauthorized_dispatches_reauth_message` (`tests/assets_errors.rs`)
- `issue_resolutions_json_output_lists_all_entries` (`tests/issue_resolution.rs`)
- `issue_move_surfaces_resolution_required_hint` (`tests/issue_resolution.rs`)
- `sprint_current_default_limit_caps_at_30` (`tests/sprint_commands.rs`)
- `sprint_current_unauthorized_dispatches_reauth_message` (`tests/sprint_commands.rs`)
- `assign_idempotent_already_assigned` (`tests/issue_commands.rs`)
- `cross_profile_isolation_team_cache` (`src/cache.rs`)
- `clear_profile_cache_removes_only_that_profile` (`src/cache.rs`)
- `verbose_false_leaves_flag_untouched` (`src/observability.rs`)
- `default_oauth_scopes_pins_the_full_set_with_offline_access` (`src/api/auth.rs` per Pass 2)
- `embedded_oauth_app_debug_redacts_secret` (`src/api/auth_embedded.rs`)

Examples (prefix style — older):
- `test_search_issues`, `test_get_issue`, `test_get_transitions`, `test_create_issue_link` (`tests/issue_commands.rs`)
- `test_help_flag`, `test_version_flag`, `test_assign_to_and_account_id_conflict` (`tests/cli_smoke.rs`)

**Convention rule (inferred):** the no-prefix `<subject>_<verb>_<expected>` form is the project's modern preferred style; older files retain `test_*` prefix because Rust test harness historically required it (it doesn't anymore — `#[test]` / `#[tokio::test]` is sufficient). Migration is opportunistic, not a sweep.

**Consistency:** MIXED (intentional — older files keep their style; new tests prefer no-prefix descriptive form).

### 1.6 CLI subcommand names

**Rule:** lower-case (single token), kebab-case for multi-token, hyphenated for sub-sub-commands when needed.

Verified by reading `cli/mod.rs:54-738`:

| Top-level | Subcommands |
|---|---|
| `init` | (none) |
| `assets` | `search`, `view`, `tickets`, `schemas`, `types`, `schema` |
| `auth` | `login`, `status`, `refresh`, `switch`, `list`, `logout`, `remove` |
| `me` | (none) |
| `project` | `fields`, `list` |
| `issue` | `list`, `view`, `create`, `edit`, `move`, `transitions`, `resolutions`, `assign`, `comment`, `comments`, `changelog`, `open`, `link`, `unlink`, `link-types`, `remote-link`, `assets` |
| `board` | `list`, `view` |
| `sprint` | `list`, `current`, `add`, `remove` |
| `worklog` | `add`, `list` |
| `team` | `list` |
| `user` | `search`, `list`, `view` |
| `queue` | `list`, `view` |
| `api` | (positional path; no subcommand) |
| `completion` | (positional shell) |

**Multi-token examples:** `link-types`, `remote-link` (kebab-case via `#[command(name = "link-types")]` or via Rust enum variant `LinkTypes` clap-rendered to kebab).

**Long-flag names** are kebab-case: `--no-input`, `--no-color`, `--client-id`, `--client-secret`, `--description-stdin`, `--description-file`, `--account-id`, `--url-only`, `--no-points`, `--no-attributes`, `--created-after`, `--created-before`, `--updated-after`, `--updated-before`, `--object-type-id`. Confirmed in `cli/mod.rs:116-585`.

**Consistency:** HIGH. No camelCase or snake_case flag names in the user-facing CLI. Rust source uses snake_case fields (`client_id`) and clap auto-generates kebab-case flags from them, so the convention is enforced by clap derive.

### 1.7 Config keys (TOML)

**Rule:** snake_case in TOML, mapped via `#[serde(rename_all = "snake_case")]` defaults or matching field names.

`~/.config/jr/config.toml` keys (verified in `src/config.rs:7-80`):
- Top-level: `default_profile` (string), `[profiles]` (table), `[defaults]` (table), legacy `[instance]` and `[fields]` (skipped on serialize).
- `[profiles.<name>]`: `url`, `auth_method` (`"oauth"` / `"api_token"`), `cloud_id`, `org_id`, `oauth_scopes`, `team_field_id`, `story_points_field_id`.
- `[defaults]`: `output` (default `"table"`).
- Legacy `[instance]`: `url`, `cloud_id`, `org_id`, `auth_method`, `oauth_scopes`.
- Legacy `[fields]`: `team_field_id`, `story_points_field_id`.

`.jr.toml` (per-project) keys (verified `config.rs:76-80`): `project`, `board_id`.

**Consistency:** HIGH. All keys snake_case. Field names in `Config` Rust structs match TOML keys 1:1 (no rename needed).

### 1.8 Cache file names

**Rule:** snake_case `.json`, fixed names per cache category (no per-key fan-out at the file level — keyed maps live inside one file).

Verified at `src/cache.rs` (cited line numbers):

| Filename | Purpose | Line |
|---|---|---|
| `teams.json` | Team list (whole-file) | `cache.rs:91, 97` |
| `project_meta.json` | Per-project map cache | `cache.rs:119, 153` |
| `workspace.json` | Workspace ID (whole-file) | `cache.rs:188, 194` |
| `resolutions.json` | Resolution catalog (whole-file) | `cache.rs:223, 229` |
| `cmdb_fields.json` | CMDB fields tuples (whole-file) | `cache.rs:250, 256` |
| `object_type_attrs.json` | Object-type attrs map | `cache.rs:293, 326` |

Path layout: `~/.cache/jr/v1/<profile>/<filename>` (verified `cache.rs:64-78`).

**Consistency:** HIGH. 6 cache categories, 6 filenames, all snake_case + `.json`.

### 1.9 Keychain key names

**Rule (verified `src/api/auth.rs:18-32`):**
- **Shared (account-level, flat):** `email`, `api-token`, `oauth_client_id`, `oauth_client_secret`. Two of these use kebab (`api-token`) and two use snake (`oauth_client_id`/`_secret`) — this is **inconsistent** but stable. The kebab-case flat keys (`api-token`) and the legacy `oauth-access-token`/`oauth-refresh-token` are kebab-case; OAuth app credentials (`oauth_client_id`/`oauth_client_secret`) are snake_case.
- **Per-profile (cloudId-scoped, namespaced):** `<profile>:oauth-access-token`, `<profile>:oauth-refresh-token` (kebab-case suffix; `:` separator).
- **Legacy (read-only, "default"-only):** `oauth-access-token`, `oauth-refresh-token` — the unprefixed pre-multi-profile flat keys.

**Service name:** `jr-jira-cli` (constant `DEFAULT_SERVICE_NAME` at `auth.rs:8`); overrideable via `JR_SERVICE_NAME` env (test isolation).

**Consistency:** MIXED — the shared keychain keys mix kebab (`email`, `api-token`, `oauth-access-token`, `oauth-refresh-token`) and snake (`oauth_client_id`, `oauth_client_secret`). This is a **historical wart**: the OAuth app credential keys were added later when multi-profile awareness arrived, and the author chose snake to mirror the Rust source field names rather than the kebab style of the older keys. Not breaking; not pretty.

### 1.10 Branch / commit / ADR / spec naming

- **Branches** (per CLAUDE.md `Conventions`): `type/short-description`, e.g., `feat/issue-commands`, `fix/auth-flow`. Default branch is `develop`. Verified externally — feedback note in user memory references commitizen.
- **Commits**: Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `test:`). Recent commits (gitStatus): `dea1664 chore: bump version`, `2345dca feat: embedded jr OAuth app with XOR obfuscation`, `dc30238 chore: bump version`. Confirms convention.
- **ADR file names**: `NNNN-kebab-case-title.md`. Verified by listing `docs/adr/`: `0001-thin-client-architecture.md`, `0002-oauth-embedded-secret.md`, `0003-reqwest-rustls.md`, `0004-per-feature-specs.md`, `0005-graphql-org-discovery.md`, `0006-embedded-jr-oauth-app.md`. **Consistency:** HIGH. 4-digit zero-padded prefix; kebab-case slug.
- **Per-feature spec file names**: kebab-case `.md` (no date prefix). Verified by listing `docs/specs/`: `multi-profile-auth.md`, `oauth-scopes-configurable.md`, `issue-changelog.md`, `assets-schema-discovery.md`, `team-field-object-shape-tolerance.md`, `list-rs-split.md`, `user-search-pagination.md`, etc. (22 files). **Consistency:** HIGH.
- **Pre-VSDD plans/specs in `docs/superpowers/{specs,plans}/`**: dated `YYYY-MM-DD-kebab-title.md` (e.g., `2026-03-21-jr-jira-cli-design.md`). These are pre-VSDD artefacts (per CLAUDE.md and Pass 0 §8); not the canonical post-v1 spec home — `docs/specs/` is.

---

## 2. Module organization

### 2.1 Product-namespaced API and types directories

Verified by directory listing:

```
src/api/
├── client.rs            # JiraClient HTTP plumbing
├── auth.rs              # OAuth + keychain
├── auth_embedded.rs     # XOR-decoded embedded credentials
├── pagination.rs        # 4 pagination shapes
├── rate_limit.rs        # Retry-After parsing
├── jira/        # Jira Core REST + Agile REST
├── jsm/         # Jira Service Management
└── assets/      # Assets / CMDB

src/types/
├── mod.rs       # 3 LOC (re-exports only)
├── jira/
├── jsm/
└── assets/
```

**Mirrored sibling layout** between `api/` and `types/`. Adding a Confluence client would mean adding `api/confluence/` + `types/confluence/` without touching anything else (Pass 1 §6.1 confirms the placeholder readiness). **Consistency:** HIGH.

### 2.2 Resource-per-file in `api/jira/`

11 files in `api/jira/`, one per Jira REST resource collection (verified by listing): `boards.rs`, `fields.rs`, `issues.rs`, `links.rs`, `projects.rs`, `resolutions.rs`, `sprints.rs`, `statuses.rs`, `teams.rs`, `users.rs`, `worklogs.rs`. Each adds methods to `JiraClient` via `impl JiraClient { ... }` blocks (verified Pass 1 §6.2).

Same convention applies in `api/jsm/` (2 files) and `api/assets/` (5 files: `linked.rs`, `objects.rs`, `schemas.rs`, `tickets.rs`, `workspace.rs`). The Assets directory has fewer "REST resources" because most CMDB operations bundle into one of these conceptual operations (linked-asset extraction, AQL search, schema discovery, ticket cross-reference, workspace ID).

**Consistency:** HIGH.

### 2.3 CLI command split

**Rule:** simple commands → one file `cli/<command>.rs`; complex commands with many subcommands → `cli/<command>/mod.rs` plus topical submodules.

| Pattern | Used by |
|---|---|
| One-file command | `cli/api.rs` (342 LOC), `cli/assets.rs` (1,055 LOC), `cli/auth.rs` (1,998 LOC), `cli/board.rs`, `cli/init.rs`, `cli/project.rs`, `cli/queue.rs`, `cli/sprint.rs`, `cli/team.rs`, `cli/user.rs`, `cli/worklog.rs` |
| Sharded directory | `cli/issue/` only |

`cli/issue/` contents (12 files, 5,078 LOC total): `mod.rs` (dispatch + re-exports), `format.rs` (row formatting), `list.rs` (list/JQL composition), `view.rs`, `create.rs` (create+edit), `workflow.rs` (move/transitions/assign/comment), `links.rs` (link/unlink/link-types), `helpers.rs` (team/points/user resolution), `assets.rs` (linked-asset display), `comments.rs`, `changelog.rs`, `json_output.rs` (JSON write-op response shapes).

**Sharding rule (inferred):** when a single command file would exceed ~1,000 LOC OR when distinct subcommands have orthogonal concerns (read vs write, JSON shape vs orchestration), split into a directory module. The `issue` subsystem is the only one that crosses this threshold; `cli/auth.rs` (1,998 LOC) and `cli/assets.rs` (1,055 LOC) **violate** this implicit rule but remain monolithic — this is a known convention gap (see §9.2).

**Consistency:** MIXED. The pattern is clear but not enforced — `cli/auth.rs` and `cli/assets.rs` are candidates for sharding that haven't been split.

### 2.4 Tests vs source

- **Unit tests (607 fns across 50 modules per Pass 0 §9):** inline in source files via `#[cfg(test)] mod tests { ... }`. Verified by reading `error.rs:65-145`, `cache.rs`, `partial_match.rs:55-160`, etc.
- **Integration tests (324 fns across 36 files):** in `tests/`, one file per topic (e.g., `tests/auth_profiles.rs`, `tests/issue_commands.rs`, `tests/sprint_commands.rs`).
- **Common fixtures:** `tests/common/fixtures.rs` (446 LOC) — fixture builders (`user_response`, `issue_response`, `issue_search_response`, `transitions_response`, `error_response`); `tests/common/mock_server.rs` (13 LOC) — single helper `setup_with_myself()`. Tests opt in via `#[allow(dead_code)] mod common;` followed by `use crate::common::fixtures::*;`. Pattern verified in 28 of 36 integration test files.
- **Snapshot fixtures:** 4 directories — `src/snapshots/`, `src/cli/snapshots/`, `src/cli/issue/snapshots/`, `tests/snapshots/`. 17 `.snap` files total (per Pass 0 §9).

**Consistency:** HIGH. Each layer has a clear home.

### 2.5 Per-feature spec docs in `docs/specs/`

Verified at ADR-0004 (`docs/adr/0004-per-feature-specs.md`): "Use per-feature specs. The v1 design spec becomes the architectural foundation. Each new feature ... gets its own spec in `docs/specs/`." The 22 files in `docs/specs/` confirm this is followed (per `find docs/specs -name '*.md' | wc -l` → 22; matches Pass 0 §8). CLAUDE.md "When adding a new feature" lists "Create a feature spec in `docs/specs/` before implementing" as step 4.

**Pre-VSDD note:** the older `docs/superpowers/specs/` (56 files) and `docs/superpowers/plans/` (75 files) are pre-VSDD artefacts and will be addressed at the Phase 0 → Phase 1 gate per `STATE.md`. These should not be confused with `docs/specs/`.

### 2.6 ADRs in `docs/adr/` with status field

Verified by reading `0004-per-feature-specs.md` head:
- `# ADR-NNNN: Title`
- `## Status` (Accepted / Superseded / Proposed)
- `## Context`
- `## Decision`
- `## Rationale`
- `## Consequences`

All 6 ADRs follow this structure. ADR-0002 (`Superseded` by ADR-0006) and ADR-0006 (`Accepted (re-supersedes ADR-0002)`) demonstrate the supersession chain.

---

## 3. Error handling patterns

### 3.1 `JrError` enum (consolidated, single source of truth)

Verified by reading `src/error.rs` (full file, 137 LOC):

```rust
#[derive(Error, Debug)]
pub enum JrError {
    NotAuthenticated,
    InsufficientScope { message: String },
    NetworkError(String),
    ApiError { status: u16, message: String },
    ConfigError(String),
    UserError(String),
    Internal(String),
    Interrupted,
    Http(#[from] reqwest::Error),
    Io(#[from] std::io::Error),
    Json(#[from] serde_json::Error),
}
```

**11 variants** confirmed (Pass 1 §3a missed `Json` and listed 10; Pass 2 §2a.2 corrects to 11).

`thiserror::Error` derive is in use (Cargo.toml has `thiserror = "2"`). `#[from]` conversions on `Http`, `Io`, `Json` enable `?` propagation from `reqwest::Error`, `std::io::Error`, `serde_json::Error`.

### 3.2 Exit code mapping (`JrError::exit_code()`)

Verified at `error.rs:51-62` (full match):

| Variant | Exit code |
|---|---:|
| `NotAuthenticated` | 2 |
| `InsufficientScope { .. }` | 2 |
| `ConfigError(_)` | 78 |
| `UserError(_)` | 64 |
| `Interrupted` | 130 |
| `NetworkError(_)`, `ApiError{...}`, `Internal(_)`, `Http(_)`, `Io(_)`, `Json(_)` | 1 (catch-all) |

The 64 / 78 / 130 mappings follow `<sysexits.h>` — `EX_USAGE = 64`, `EX_CONFIG = 78`, `SIGINT = 128 + 2 = 130`. Pinned by 4 unit tests in `error.rs:65-115` (`config_error_exit_code`, `user_error_exit_code`, `internal_error_exit_code_is_one`, `insufficient_scope_exit_code`).

**Consistency:** HIGH (exit codes are scriptable detection without parsing stderr).

### 3.3 Error message convention — "always suggest what to do next"

CLAUDE.md `Conventions`: *"Errors: Always suggest what to do next. Map to exit codes via `JrError::exit_code()`"*.

**Exemplary cases (verified):**
1. `NotAuthenticated` (`error.rs:5-6`): `"Not authenticated. Run \"jr auth login\" to connect."` — names the recovery command.
2. `InsufficientScope` (`error.rs:9-15`): multi-line — gateway raw message + workaround "Use a classic token with `write:jira-work` scope" + alternative "Try OAuth 2.0 (run `jr auth login --oauth`)" + issue link `github.com/Zious11/jira-cli/issues/185`. Comprehensive recovery guidance.
3. `JrError::ConfigError("Profile {:?} has no URL configured. Run \"jr auth login --profile {}\".")` (`api/client.rs:54-58` per Pass 4 §3.7) — names the recovery command with the profile name interpolated.
4. `JrError::ConfigError("Cloud ID not configured. Run \"jr init\" to set up your instance.")` (`api/client.rs:391-396` per Pass 4 §3.7).
5. OAuth keychain partial-state error (`api/auth.rs:161-166` per Pass 4 §3.7): `"OAuth keychain entries for profile X are partial ... Run \`jr auth logout\` then \`jr auth login\`"`.
6. Auth-refresh against unconfigured profile (BC-011 in Pass 3): stderr names "no URL configured" + `jr auth login --url` recovery.

**Counter-examples (do any violate?):** spot-checked `JrError::UserError("Invalid selection".into())` (used by interactive disambiguation prompts) — does NOT name a recovery command, but the user is mid-prompt; the next prompt iteration is the recovery. Acceptable. `NetworkError(host)` displays `"Could not reach {host} — check your connection"` — gives diagnostic guidance but no command. Acceptable for transport failures.

**Consistency:** HIGH for state-mutating commands and config errors; MEDIUM for transport / interactive prompts (where "next step" is implicit).

### 3.4 `?` propagation as canonical pattern

Counted call sites: `?` is used in nearly every `pub async fn` in `api/jira/`, `api/jsm/`, `api/assets/`, and every `handle*` in `cli/`. Sample read of `api/jira/issues.rs`, `api/jira/teams.rs`, `api/assets/workspace.rs`, `api/assets/linked.rs`: every fallible call uses `?`.

`#[from]` on `Http`, `Io`, `Json` makes `?` work transparently for the most common error sources. Custom errors (`UserError`, `ConfigError`, `Internal`) are wrapped explicitly via `JrError::UserError(...)` then `.into()` or `?` (when the function returns `anyhow::Result<()>`).

**Consistency:** HIGH.

### 3.5 `anyhow` vs `thiserror` — both, with a clear split

- **`thiserror`** (`Cargo.toml:28`) → defines `JrError` only. The crate's typed error.
- **`anyhow`** (`Cargo.toml:15`) → handler-level `Result<()>` for command handlers + cross-cutting `bail!` / `anyhow!`. Counted: `anyhow::*` appears in 22 source files with hundreds of references.

**Pattern (verified):**
- `cli::*::handle*` returns `anyhow::Result<()>`.
- API client methods return `Result<T, JrError>` or `anyhow::Result<T>` (mixed; `JiraClient::send` returns `Result<T, JrError>`).
- `main.rs:34-49` walks `e.chain()` looking for a `JrError` to extract `exit_code()`; if no `JrError` found, exit 1.

This is a deliberate two-tier strategy: the type-safe layer at the boundary (HTTP / config / file system), `anyhow` for orchestration where a typed enum would be friction. Documented in `error.rs:30-36` (the `Internal` variant docstring).

**Consistency:** HIGH.

### 3.6 `Result<T, JrError>` vs `anyhow::Result<T>` return type

Per file counts (from `awk` over src/):
- Files using `anyhow::*`: 22 (most CLI handlers + client + api impls).
- Files defining specific `Result<T, JrError>`: `api/client.rs`, `api/auth.rs`, `error.rs` itself, scattered helpers.

The convention: when a handler is the *outermost* layer (called by main.rs), it returns `anyhow::Result<()>`. When a function is a *boundary* utility (HTTP send, config load, profile resolve), it returns `Result<T, JrError>` so callers can match precisely.

**Consistency:** HIGH (rule is followed; mixed is intentional).

### 3.7 Panic discipline — no panics in user-visible paths

Pass 3 documented "no panic in stderr" as a universal contract (e.g., BC-011 explicitly asserts `stderr does NOT contain panic`). Verified by:

- **`unwrap()` in src (non-test):** counted via `awk` excluding `#[cfg(test)]` blocks. The handful that exist (`jql.rs:78`, `partial_match.rs:27-28`, `cli/auth.rs:167,196`, `cli/assets.rs:456,699`, `cli/issue/list.rs:405`, `cli/issue/helpers.rs:501`, `api/assets/linked.rs:208`) are all **proven-non-empty** (e.g., `clauses.into_iter().next().unwrap()` after a `clauses.is_empty()` early-return; `flag_id.unwrap()` after `flag_id.is_some() && flag_secret.is_some()` check). These are correctness-by-construction, not blind unwraps.
- **`expect()` in src (non-test):** the most notable is `api/client.rs:191`: `request.try_clone().expect("request should be cloneable (JSON body)")` — Pass 1 §3e and Pass 4 §4.1 both verified this is unreachable in practice (the codebase only sends JSON or no-body, both cloneable).
- **`panic!` in non-test code:** searched via awk over all `*.rs`. **Zero** occurrences in non-test code. All 26 `panic!` occurrences are inside `#[cfg(test)]` modules (assertion failures in `partial_match.rs`, `cli/auth.rs:1965`, `cli/issue/changelog.rs`).
- **`todo!()` / `unimplemented!()`:** **Zero** in src.

**Consistency:** HIGH — verified by a deliberate audit across all of src/.

### 3.8 `unsafe` policy

CLAUDE.md `Conventions`: *"No unsafe code without explicit justification in a comment."*

Pass 4 §2.9 claimed "grep for `unsafe` returns expected places (cache.rs FFI, auth.rs OAuth, build.rs)". My audit confirms — every `unsafe` block in src is for `std::env::set_var` / `remove_var` (which became `unsafe` in Rust edition 2024 because env is shared mutable state). Counted occurrences:

| File | Count | Purpose |
|---|---:|---|
| `src/config.rs` | ~14 | All in `#[cfg(test)] mod tests` setting `JR_*` env per test (`set_var(...)` / `remove_var(...)`). Function-level comments cite "must be inside `unsafe` because env is shared". `config.rs:295` has the canonical justification: setting is unsound under `#[tokio::main]` once worker threads exist. |
| `src/cache.rs` | 2 | `XDG_CACHE_HOME` test setup (lines 372, 374). |
| `src/cli/auth.rs` | 2 | Test setup (lines 1333, 1344). |
| `src/api/auth.rs` | 2 | Test setup (`JR_SERVICE_NAME`, lines 1118, 1122). |
| `src/main.rs:60` | 0 (comment-only) | Actually a comment explaining why `set_var` would be unsound here — main.rs deliberately threads `cli.profile` as a parameter instead. |
| `src/cli/init.rs:15` | 0 (comment-only) | Same — comment explaining the design choice. |

**No `unsafe` in production paths.** All instances are test-only env mutation, and the production code paths explicitly avoid `set_var` (`main.rs:60` comment; `config.rs:200,295` comments). `build.rs` (per Pass 0) uses an FFI shim to `BCryptGenRandom` on Windows but no extra crate; that's a separate file outside `src/`.

**Consistency:** HIGH — every `unsafe` block has either an inline justification comment or is in a test module where the constraint is well-understood.

---

## 4. Test patterns

### 4.1 Test function naming — see §1.5

Two patterns coexist; no-prefix descriptive form is dominant in newer files.

### 4.2 AAA structure (Arrange-Act-Assert)

**Verified pattern (sample from `tests/issue_resolution.rs:11-46`):**

```rust
#[tokio::test]
async fn issue_resolutions_json_output_lists_all_entries() {
    // ARRANGE
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/resolution"))
        .respond_with(ResponseTemplate::new(200).set_body_json(...))
        .mount(&server)
        .await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // ACT
    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "resolutions", "--output", "json"])
        .output()
        .unwrap();

    // ASSERT
    assert!(output.status.success(), "stderr: {}", ...);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().expect("expected JSON array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Done");
}
```

The structure is implicit (no `// arrange`/`// act`/`// assert` comments) but observable: mock setup → command invocation → assertions. **Consistency:** HIGH.

### 4.3 Wiremock setup convention

Verified at `src/api/client.rs:111-122`:

```rust
pub fn new_for_test(base_url: String, auth_header: String) -> Self { ... }
```

This is the explicit test seam. Two integration-test calling conventions exist:

1. **Library-level tests** (e.g., `tests/issue_commands.rs:23`): `MockServer::start().await` → mount mocks → `JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0")` → call client methods directly. Used by 28 of 36 integration test files. Mock count varies (132 mocks in `cli_handler.rs`, 138 in `issue_commands.rs`, 73 in `assets.rs`).
2. **Process-level tests** (e.g., `tests/issue_resolution.rs`, `tests/cli_smoke.rs`): `assert_cmd::Command::cargo_bin("jr")` + `.env("JR_BASE_URL", server.uri())` + `.env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")`. Drives the whole `jr` binary from inside the test. Used for end-to-end exit-code/stderr/stdout validation.

`Mock::expect(N)` is used both positively (count = expected hits) and negatively (`expect(0)` to assert short-circuit, e.g., `tests/issue_list_errors.rs:388` per Pass 3 §1.3).

**Consistency:** HIGH.

### 4.4 Insta snapshot pattern

Verified call sites (`assert_snapshot!` / `assert_json_snapshot!`):
- `src/cli/issue/json_output.rs:91-141` — write-op JSON response shapes (move/assign/edit/link/unlink/remote-link). 11 snapshots in `src/cli/issue/snapshots/`.
- `src/cli/sprint.rs:425, 433` — sprint add/remove JSON.
- `src/cli/auth.rs:1687` — `auth list` table render.
- `src/adf.rs:788, 1106` — markdown→ADF and ADF→text on a complex fixture.
- `tests/issue_changelog.rs:1263` — changelog JSON output.

Snapshot file path convention: `<crate>__<module>__tests__<name>.snap` (auto-generated by insta from module path). Total: **17 `.snap` files** (per Pass 0 §9).

**Coverage:** snapshots are concentrated on JSON-output write-ops (10 of 17), ADF rendering (2), table headers (1), sprint responses (2), changelog (1). NOT covered by snapshots: most read-ops (table renders for issue list/view), comment formatting, error message text, help output. The team uses snapshots tactically where the output shape matters most, not as a blanket coverage tool.

**Consistency:** HIGH (where used).

### 4.5 Proptest pattern

Verified call sites:
- `src/duration.rs:128` — `proptest! { ... }` for `parse_duration` (panic-free on garbage; round-trip for valid inputs).
- `src/jql.rs:383` — `escaped_value_never_has_unescaped_quote` and similar JQL escape invariants. Regression corpus at `proptest-regressions/jql.txt`.
- `src/partial_match.rs:153` — `MatchResult` invariants (single-substring → `Ambiguous`, never `Exact`).

**3 modules use proptest.** Only the modules with non-trivial domain rules need property tests; the rest are covered by example-based tests. **Property tests scope:** never include type-system trivia (no "Vec round-trips through JSON"); they encode domain-meaningful invariants per Pass 2 §2b.5.

### 4.6 `#[ignore]` gating convention

13 `#[ignore]` attrs (Pass 0 §9 confirmed; my audit lists them in 3 files):

| File | Count | Gate env |
|---|---:|---|
| `src/api/auth.rs` | 10 | `JR_RUN_KEYRING_TESTS=1` |
| `tests/auth_profiles.rs` | 2 | `JR_RUN_KEYRING_TESTS=1` |
| `tests/oauth_embedded_login.rs` | 1 | `JR_RUN_OAUTH_INTEGRATION=1` |

**Pattern (verified at `tests/auth_profiles.rs:242-244`):**

```rust
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
#[test]
fn ... {
    if std::env::var("JR_RUN_KEYRING_TESTS").is_err() {
        return;  // belt-and-braces — also a no-op without the env var
    }
    ...
}
```

Both `#[ignore]` (skips by default) AND a runtime env-var early-return guard ("belt and braces"). Reason captured in `src/api/auth.rs:1116`: *"The opt-in `JR_RUN_KEYRING_TESTS` gate further keeps these tests from running by accident on developer machines that DO have a keyring."*

**Consistency:** HIGH.

### 4.7 Test fixtures (`tests/common/fixtures.rs`)

Public API (verified head of file, 446 LOC total): `user_response()`, `issue_response(key, summary, status)`, `issue_search_response(issues)`, `issue_search_response_with_next_page(issues)`, `approximate_count_response(count)`, `transitions_response(...)`, `transitions_response_with_status(...)`, `error_response(messages)`, plus dozens more for boards/sprints/users/projects/teams/comments/changelog/worklog. All return `serde_json::Value` ready for `ResponseTemplate::set_body_json`.

Pattern: each fixture is a small `pub fn` returning a `json!({...})` literal. No state, no setup. **Consistency:** HIGH.

`tests/common/mock_server.rs` (13 LOC) provides one helper: `setup_with_myself()` — pre-mounts `GET /rest/api/3/myself` because every authenticated `JiraClient` flow does this on startup.

### 4.8 `assert_cmd` for end-to-end CLI tests

Used in 36 integration tests where exit code or stderr text matters (verified via `awk`). Pattern: `Command::cargo_bin("jr").env(...).env(...).args([...]).output()`. Then assertions on `output.status.success()`, `output.stdout`, `output.stderr`.

For tests that don't need a process, `JiraClient::new_for_test` is used directly. The choice is per-test based on whether the assertion targets the public CLI contract (use `assert_cmd`) or the library-level API (use `new_for_test`).

### 4.9 `JR_BASE_URL` / `JR_AUTH_HEADER` / `XDG_*HOME` env conventions

Verified at `src/api/client.rs:36-65, 111-122`:
- `JR_BASE_URL`: overrides ALL HTTP targets (used by integration tests to inject wiremock).
- `JR_AUTH_HEADER`: short-circuits credential loading; tests pass `"Basic dGVzdDp0ZXN0"` (`test:test` base64'd).
- `JiraClient::new_for_test(base_url, auth_header)`: explicit constructor that bypasses config + keychain entirely.

For test isolation:
- `XDG_CONFIG_HOME` / `XDG_CACHE_HOME`: pointed at per-test `tempfile::tempdir()` directories so config/cache writes don't leak across tests.
- `JR_SERVICE_NAME`: keychain service name override (test isolation for the `#[ignore]`-gated keyring tests).

**Consistency:** HIGH — every integration test that touches HTTP uses this exact pattern.

### 4.10 Property-test invariants

Per Pass 2 §2b.5 and `src/jql.rs:383-394`, `src/duration.rs:128`, `src/partial_match.rs:153`:

- **JQL:** `escape_value` always produces output where every double-quote is preceded by an odd number of backslashes (no unescaped quote). `validate_duration` doesn't panic on any input. `build_asset_clause` never injects unescaped meta-characters into the AQL function call.
- **Duration:** `parse_duration` doesn't panic on any input. Round-trip property: `format_duration(parse_duration(s)) == s` for canonical-form valid inputs.
- **Partial match:** Single-substring routes to `Ambiguous`, never `Exact`. `Exact` is reserved for case-insensitive exact matches.

---

## 5. Design patterns in use

### 5.1 Thin client over generated SDK (ADR-0001)

ADR-0001 documents the choice to skip a generated OpenAPI client and hand-write the resource bindings. Verified by listing `api/jira/` (11 files) — each is a thin `impl JiraClient { pub async fn ... }` block over `reqwest`. No code generation, no `openapi-generator`, no intermediate abstraction.

### 5.2 Figment-based config layering

Verified at `src/config.rs:189-335` (per Pass 1 §3k). Sources: `GlobalConfig::default()` < `~/.config/jr/config.toml` < `JR_*` env vars (figment toml + env overlay). Per-project `.jr.toml` is a separate `ProjectConfig` field (NOT merged into `GlobalConfig`). Active-profile resolution precedence: **flag > env > config > "default"** (`config.rs:95-110`).

### 5.3 Per-profile cache isolation

Every cache reader/writer takes `profile: &str` first arg (verified in `src/cache.rs` — `read_cache`, `write_cache`, `cache_dir`, `clear_profile_cache`, etc., all gated on profile). `JiraClient` carries `profile_name: String` (`api/client.rs:105`) and exposes `profile_name()` so L4 modules with `&JiraClient` can pass it down.

**Convention enforced by signature**, not phantom-typed wrapper. Pass 1 §7 risk #6 noted this as a known soft fence — a future free-function added to `cache.rs` that forgets `profile` would compile.

### 5.4 Command pattern via clap derive

`#[derive(Parser)]` on `Cli`, `#[derive(Subcommand)]` on `Command` and per-subsystem `*Command` enums. Each variant maps to a handler in `cli/<command>::handle*`. main.rs:248-256 dispatches via `match`. Pass 0 §4 + Pass 1 §1a.

### 5.5 Repository pattern (loose)

Each `api/jira/<resource>.rs` is the repository for its resource — `impl JiraClient` adds the methods. The shared `JiraClient` is the "unit of work" (HTTP transport + auth state). Not a strict DDD repository (no in-memory aggregate caching), but the structural intent is the same.

### 5.6 Cache-aside / lazy-loading

Verified pattern at `api/assets/workspace.rs::get_or_fetch_workspace_id`, `api/assets/linked.rs::get_or_fetch_cmdb_fields`, `api/jira/teams.rs::get_or_fetch_teams` (per Pass 1 §3j and Pass 4 §1.4):

```
fn get_or_fetch_X(client) {
    if let Some(cached) = read_cache(profile)? {
        return Ok(cached);
    }
    let fetched = client.api_fetch_X().await?;
    write_cache(profile, &fetched)?;
    Ok(fetched)
}
```

Cache miss policy: NotFound / corrupt / expired all return `Ok(None)` and trigger refetch (`cache.rs:14-34`). **Consistency:** HIGH across all 6 cache categories.

### 5.7 Lazy migration of legacy keychain keys ("default"-only)

Verified at `src/api/auth.rs:111-169`. On first read of `default:oauth-access-token`:
1. Detect old-format presence (legacy flat `oauth-access-token` set, namespaced absent).
2. Read old.
3. Write new namespaced.
4. Delete old.

**Only for "default" profile.** Non-default profiles never inherit (cross-pollination guard). Pass 2 BC-023.

### 5.8 Build-time codegen (ADR-0006)

`build.rs` (125 LOC) reads `JR_BUILD_OAUTH_CLIENT_ID` / `_SECRET` env vars at compile time. Generates `$OUT_DIR/embedded_oauth.rs` with three module-private constants (`EMBEDDED_ID`, `EMBEDDED_SECRET_XOR`, `EMBEDDED_SECRET_KEY`). `auth_embedded.rs:17` consumes via `include!(concat!(env!("OUT_DIR"), "/embedded_oauth.rs"))`.

When env vars are missing (forks, local builds): emits all three as `None`. `compile_error!` for non-unix/non-windows hosts.

### 5.9 No phantom-type fences

Verified absence: no `PhantomData<Profile>` wrappers, no type-state encoding for OAuth flow stages. Pass 1 §7 noted this as a "convention choice" — the project relies on signature discipline (`profile: &str` first arg) rather than compile-time fences. Trade-off: simpler code; less compile-time safety.

### 5.10 `new_for_test` constructors

`JiraClient::new_for_test(base_url, auth_header)` (`api/client.rs:111-122`). Bypasses config + keychain. Used by all 28 library-level integration tests. **Convention:** when an I/O-bound type needs a test constructor, expose a `new_for_test` rather than mutating the production constructor with test-only branches.

### 5.11 Smart constructor for sensitive types

Per Pass 1 §6.10:
- `EmbeddedOAuthApp::Debug` redacts `client_secret` (`auth_embedded.rs:34-41`).
- `RedirectUriStrategyRequest::bind()` returns `ResolvedRedirect` with private `TcpListener` field — closes a TOCTOU class.

### 5.12 Author needle smart constructor (`docs/specs/author-needle-smart-constructor.md`)

`cli/issue/changelog.rs::AuthorNeedle::classify(input)` (per Pass 2): `:` or 12+ chars with digit → `AccountId`; else `NameSubstring`. Encodes the heuristic for "is this an accountId or a display-name fragment". Pinned by 12+ unit tests (`changelog.rs:330-476` test names like `expected_AccountId_at_12_char_boundary`).

---

## 6. Pre-VSDD project conventions

### 6.1 Conventional Commits

Verified by recent git log (gitStatus): `dea1664 chore: bump version to 0.5.0-dev.7`, `73f61f6 chore: bump version`, `2345dca feat: embedded jr OAuth app with XOR obfuscation`, `dc30238 chore: bump version`. Prefixes used: `feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `test:`. **Consistency:** HIGH.

### 6.2 Branch workflow

Per CLAUDE.md `Conventions`: default branch is `develop`; feature branches → PR to `develop`; release PRs `develop → main`. `main` and `develop` are protected — require CI to pass and code-owner approval; admins can bypass.

### 6.3 Feature spec discipline

Per CLAUDE.md "When adding a new feature":
1. Read CLAUDE.md
2. Read v1 design spec
3. Read relevant ADRs
4. **Create a feature spec in `docs/specs/` before implementing**
5. Follow TDD — write tests first

ADR-0004 captures the rationale (token economy, parallel features, clear ownership). 22 specs in `docs/specs/` confirm the convention is followed.

### 6.4 TDD discipline

CLAUDE.md `Conventions`: *"Tests: TDD. Unit tests inline, integration tests in `tests/`."* The 931 total test functions (607 unit + 324 integration per Pass 0 §9) and the 1.7:1 test-to-source LOC ratio (16,958 vs 23,334) are quantitative evidence of TDD adoption.

### 6.5 No lint suppression without refactoring

CLAUDE.md `Conventions`: *"No lint suppression without refactoring. If clippy warns ... refactor to fix the root cause — don't add `#[allow]`. If refactoring is impractical, ask the user before suppressing and include a justification comment."*

**Audit (verified via awk):**
- `#[allow(dead_code)]` in `tests/common`: appears 29 times across 28 integration test files at the **top of `mod common;`**. Reason: each integration test imports `mod common;` but only uses some helpers — `#[allow(dead_code)]` prevents per-test "unused" warnings for the helpers that test doesn't happen to call. This is a structural necessity of Rust integration tests, not a clippy suppression. Acceptable.
- `#[allow(clippy::*)]` in src: **zero** found in my audit. The convention is upheld.

**Consistency:** HIGH.

### 6.6 Default to fixing code, not tests

CLAUDE.md `Conventions`: *"Default to fixing code, not tests. When a test fails, assume the test is correct and fix the implementation using idiomatic Rust."* Cannot quantitatively verify across the git history without running git log analysis; the convention is documented and the code+test ratio is consistent with it.

### 6.7 Zero clippy warnings policy

`.github/workflows/ci.yml:38-45` (per Pass 0 §5 and Pass 4 §2.8) runs `cargo clippy -- -D warnings`. Combined with the lint-suppression-prohibition (§6.5), this means warnings are real-fixed, not silenced.

---

## 7. Consistency assessment

| Convention | Consistency | Evidence | Outliers |
|---|---|---|---|
| Module naming (snake_case; plural for resource collections, singular for engines/concepts) | HIGH | All 80 source files conform; `_` only for compound concepts. | None. |
| Type names (`PascalCase`) | HIGH | 100+ types verified across all type modules. | None. |
| Function names (`snake_case`) | HIGH | All `pub fn` / `pub async fn` verified. | None. |
| Constants (`SCREAMING_SNAKE_CASE`) | HIGH | 21 named constants enumerated. | None. |
| Test fn naming | MIXED (intentional) | 108 prefix-style + 212 no-prefix-style in tests/. | Newer files prefer no-prefix; migration is opportunistic. |
| CLI subcommand names (kebab-case where multi-token, lowercase otherwise) | HIGH | Verified via `cli/mod.rs:54-738`. | None. |
| Config keys (snake_case TOML) | HIGH | Verified via `config.rs:7-80`. | None. |
| Cache file names (snake_case `.json`) | HIGH | 6 categories; all conform. | None. |
| Keychain key names | MIXED | Shared keys mix kebab (`email`, `api-token`) and snake (`oauth_client_id/_secret`). | OAuth app credential keys use snake; legacy keys use kebab. Historical wart, stable. |
| Branch / commit conventions | HIGH | Recent log shows Conventional Commits. | None observed. |
| ADR / spec file naming | HIGH | `docs/adr/` 4-digit prefix + kebab; `docs/specs/` kebab. | None. |
| Error handling (`JrError` + `?`) | HIGH | Single enum source of truth; `?` propagation universal; `#[from]` for transparent variants. | None. |
| `unsafe` discipline | HIGH | All `unsafe` blocks justified inline; **zero** in production paths (test-only env mutation). | None. |
| `#[allow(clippy::*)]` discipline | HIGH | **Zero** clippy suppressions in src. `#[allow(dead_code)]` in tests/common is a structural necessity. | None. |
| Per-profile cache signature | HIGH | Every reader/writer takes `profile: &str` first. | Convention enforced by signature, not type system — soft fence. |
| Idempotency on state-changing ops | HIGH (move, assign, logout, switch); MEDIUM elsewhere | `move` (Pass 2 INV-1, BCs); `assign` (CLAUDE.md + tests); `logout` (Pass 2 §2b.1); `switch`. | `comment` and `link` are NOT idempotent (each call creates new comment/link); `transitions` is read-only so trivially idempotent; `link-types` ditto. `unlink` is idempotent (re-running on already-unlinked = no-op). Comment-edit is not exposed; remote-link create is not idempotent. |
| Snapshot test coverage | MEDIUM (concentrated, not blanket) | 17 snapshots — heavy on JSON write-op shapes (10) and ADF rendering (2). | Read-op tables and error-message text are NOT snapshotted (pinned via `assert!(stdout.contains(...))` instead). |
| Output format (table vs JSON parity) | HIGH | Every command verified to support `--output json`; write-ops return `{"key", ...}` JSON. Pass 3 §13 BCs assert this is universal. | None observed. |
| Test infrastructure (`JR_BASE_URL` / `JR_AUTH_HEADER` / `XDG_*HOME`) | HIGH | All 36 integration tests use this exact pattern. | None. |
| Insta snapshot path convention (`<crate>__<module>__tests__<name>.snap`) | HIGH | All 17 `.snap` files conform (auto-generated by insta). | None. |
| Clap derive for CLI structure | HIGH | All commands defined via `#[derive(Subcommand)]` enums in `cli/mod.rs`. | None. |
| Resource-per-file in `api/jira/` | HIGH | 11 files, 1 per resource; mirrored in `api/jsm/` and `api/assets/`. | None. |
| Module sharding rule (>1000 LOC + orthogonal subcommands → directory) | MIXED | `cli/issue/` is split (5,078 LOC). | `cli/auth.rs` (1,998 LOC) and `cli/assets.rs` (1,055 LOC) NOT split — known gaps. |

---

## 8. Anti-pattern audit

### 8.1 Findings (with severity)

| Anti-pattern | Severity | Locations / Notes |
|---|---|---|
| **Long files** (>1,000 LOC) | MEDIUM | `cli/auth.rs` 1,998 LOC; `adf.rs` 1,826 LOC; `api/auth.rs` 1,397 LOC; `config.rs` 1,223 LOC; `cli/issue/list.rs` 1,083 LOC; `cli/assets.rs` 1,055 LOC; `cache.rs` 899 LOC; `cli/issue/changelog.rs` 847 LOC; `cli/issue/helpers.rs` 813 LOC; `cli/issue/workflow.rs` 788 LOC. Pass 1 §7 already flagged auth.rs and list.rs. `adf.rs` is a hand-written ADF parser/emitter — naturally large. The CLI handlers (auth, assets, list) are sharding candidates. |
| **`unwrap()` / `expect()` in non-test paths** | LOW | All 10 found `unwrap()` calls in non-test src code are correctness-by-construction (preceded by `is_some()` / non-empty checks). `expect()` calls are mostly proof-of-invariant (e.g., `try_clone().expect(...)` after a JSON-only body). No blind unwraps. |
| **`clone()` in hot paths** | LOW (audit deferred) | Cache lookups return owned `Vec<...>` per call (no `Arc`/`Cow`). For a CLI tool with single-process invocations, this is fine. JQL composition uses `String::from` and `format!` liberally. Not a perf risk for current workload. |
| **String-typed issue keys** (no newtype) | LOW | Issue keys are bare `String` everywhere. `validate_asset_key` exists for asset keys but no equivalent for issue keys (they go through Jira-side validation). Newtype would prevent accidental swap of issue key vs project key vs asset key, but the test coverage and naming conventions catch most issues. |
| **`unsafe` outside justified contexts** | (none) | All `unsafe` blocks have inline rationale or are in test-only env mutation. |
| **`#[allow(clippy::*)]` without justification** | (none) | Zero in src. Test-only `#[allow(dead_code)]` on `mod common;` is structural. |
| **Magic numbers without named constants** | LOW | Pass 4 enumerated 27 named constants. Spot-check found no remaining magic numbers in hot paths. The Atlassian per-page max of 100 is referenced as `USER_PAGE_SIZE` and similar; the legacy `30` default limit is `DEFAULT_LIMIT`. |
| **`panic!` / `todo!()` / `unimplemented!()` in src** | (none) | All 26 `panic!` occurrences are inside `#[cfg(test)] mod tests`. Zero `todo!()` / `unimplemented!()`. |
| **Implicit string conversions losing type safety** | LOW | The `auth_method` field is `Option<String>` (values `"oauth"` / `"api_token"`); could be a typed enum but TOML round-trip and figment overlay are simpler with String. Same for `oauth_scopes` (single space-separated string). Not a bug class observed. |
| **Mixed kebab/snake keychain keys** | LOW | `email`, `api-token`, `oauth-access-token`, `oauth-refresh-token` are kebab; `oauth_client_id`, `oauth_client_secret` are snake. Inconsistent but stable; rename would be a migration burden for existing installs. |
| **Same-profile cache write race** | LOW (Pass 4 §7.5 §21) | `fs::write` is not atomic; two simultaneous `jr` runs on the same profile can race. Self-healing via cache miss policy on read. |

### 8.2 Top 3 anti-patterns by severity

1. **`cli/auth.rs` (1,998 LOC) and `cli/assets.rs` (1,055 LOC) violate the implicit "shard at ~1000 LOC" rule** — the only directory-sharded command is `cli/issue/`. Both files are coherent (auth is profile lifecycle; assets is workspace orchestration), but they're large enough to be hard to evolve safely.
2. **Module-size growth on `cli/issue/list.rs` (now 1,083 LOC, was ~970 per CLAUDE.md)** — already split once via `docs/specs/list-rs-split.md`, but JQL composition + asset clause integration + status auto-inference + date filters + `--open` filtering are still in one function. Continued growth would warrant a second split.
3. **Mixed kebab/snake in keychain key names** — minor cosmetic inconsistency that's stable now but a foot-gun for any future renamer.

---

## 9. Convention strengths and gaps

### 9.1 Strengths (top 5)

1. **Single source of truth for errors with strict exit-code mapping.** `JrError` enum (11 variants), `exit_code()` method (0/1/2/64/78/130 per `<sysexits.h>`), pinned by 4 unit tests. Every command is scriptable via exit code; every error message names a recovery action.
2. **Per-profile cache signature is uniform across all cache call sites — no leakage observed in audit.** Every reader/writer takes `profile: &str` first arg. `JiraClient.profile_name()` is the canonical accessor for L4 modules. Pass 1 §7 risk #6 acknowledged the soft-fence trade-off, but the convention is followed.
3. **Zero `unsafe` in production paths; zero `#[allow(clippy::*)]` in src.** All `unsafe` is test-only `std::env::set_var` (Rust 2024 requirement). All clippy warnings are real-fixed, not silenced. The `cargo clippy -- -D warnings` CI gate enforces this.
4. **Three-axis test-injection seam (`JR_BASE_URL` / `JR_AUTH_HEADER` / `XDG_*HOME`) plus `new_for_test` constructor.** All 36 integration tests use this exact pattern. wiremock + tempfile + per-test env override gives full filesystem and network isolation without touching real keychain.
5. **Product-namespaced API and types directories with mirrored layout.** `api/{jira,jsm,assets}/` ↔ `types/{jira,jsm,assets}/`. Adding Confluence is a sibling-directory addition with zero touches to existing code. ADR-0001 + ADR-0004 codify the architectural and documentation halves of this.

### 9.2 Gaps (top 5)

1. **`cli/auth.rs` 1,998 LOC and `cli/assets.rs` 1,055 LOC violate the implicit module-size convention.** The only directory-sharded command is `cli/issue/`. Both files would benefit from sharding by subcommand topic (auth: login / status / refresh / list / logout / remove; assets: search / view / tickets / schemas). No `docs/specs/auth-rs-split.md` or `docs/specs/assets-rs-split.md` exists — the refactor hasn't been planned.
2. **Test naming style is mixed (108 prefix vs 212 no-prefix) without a written convention.** Project preference (no-prefix descriptive form) is implicit in newer files; older files retain `test_*` prefix. A short note in CLAUDE.md or a contributing guide would close this.
3. **Mixed kebab/snake in keychain key names.** `email`/`api-token`/`oauth-access-token`/`oauth-refresh-token` are kebab; `oauth_client_id`/`oauth_client_secret` are snake. Stable, but inconsistent. Migration would be breaking for existing installs.
4. **Per-profile cache signature is convention, not type-system enforced.** A future free function added to `cache.rs` that forgets to take `profile` would compile. A phantom-typed `Cache<P>` wrapper or a `Profile(String)` newtype would close this. Trade-off acknowledged in Pass 1 §7 risk #6.
5. **Snapshot coverage is concentrated on JSON write-op shapes; read-op tables and error message text are NOT snapshotted.** They're pinned by `assert!(stdout.contains(...))`-style assertions, which catch presence but not formatting regressions. Promoting some of these to snapshots would reduce maintenance burden when output formatting changes intentionally.

---

## 10. State Checkpoint

```yaml
pass: 5
status: complete
naming_conventions: 10            # modules, types, fns, constants, tests, CLI subcmds, config keys, cache files, keychain keys, branch/commit/ADR/spec
design_patterns: 12               # thin client, figment layering, per-profile cache, command (clap), repository (loose), cache-aside, lazy migration, build-time codegen, no phantom-types, new_for_test, smart constructors, author-needle smart constructor
anti_patterns_found: 11           # long files, unwrap (audited safe), clones (deferred), string keys, unsafe (audited safe), #[allow(clippy)] (none), magic numbers (none), panic in src (none), implicit conversions (low), kebab/snake mix, cache write race
files_examined: 32
loc_examined: ~9000
constants_inventoried: 21
test_fns_named_audited: 320       # tests/ count via awk
cache_categories_verified: 6
adrs_verified: 6
docs_specs_count: 22
strengths_top5: confirmed
gaps_top5: confirmed
inputs_consumed:
  - .factory/semport/jira-cli/jira-cli-pass-0-inventory.md
  - .factory/semport/jira-cli/jira-cli-pass-1-architecture.md
  - .factory/semport/jira-cli/jira-cli-pass-2-domain-model.md
  - .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md (head)
  - .factory/semport/jira-cli/jira-cli-pass-4-nfr-catalog.md
  - .reference/jira-cli/CLAUDE.md
  - .reference/jira-cli/Cargo.toml
  - .reference/jira-cli/docs/adr/0004-per-feature-specs.md
  - .reference/jira-cli/docs/adr/ (full listing)
  - .reference/jira-cli/docs/specs/ (full listing)
  - .reference/jira-cli/src/error.rs (full)
  - .reference/jira-cli/src/cache.rs (head + grep)
  - .reference/jira-cli/src/config.rs (grep)
  - .reference/jira-cli/src/cli/mod.rs (head + grep)
  - .reference/jira-cli/src/api/auth.rs (grep)
  - .reference/jira-cli/src/api/client.rs (grep)
  - .reference/jira-cli/tests/common/fixtures.rs (head)
  - .reference/jira-cli/tests/common/mock_server.rs (full)
  - .reference/jira-cli/tests/issue_resolution.rs (head)
verification_methods:
  - awk_for_attributes: 14 invocations
  - awk_for_tests: 8 invocations
  - awk_for_constants: 1 invocation
  - awk_for_unsafe: 1 invocation
  - awk_for_panic: 1 invocation
  - awk_for_unwrap_outside_test: 1 invocation
  - awk_for_allow: 2 invocations
  - awk_for_proptest: 1 invocation
  - awk_for_snapshot: 1 invocation
  - directory_listings: 6
  - file_reads: 6 full + many partial
  - constants_cited: 21 with file:line
  - tests_counted: prefix=108, no-prefix=212 (320 total in tests/)
timestamp: 2026-05-04T00:00:00Z
next_pass: 6
```
