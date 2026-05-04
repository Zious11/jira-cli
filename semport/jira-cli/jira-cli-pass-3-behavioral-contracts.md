# Pass 3: Behavioral Contracts — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Builds on: Pass 0 (inventory), Pass 1 (architecture), Pass 2 (domain model — 25 draft invariants).

> **Method.** Each BC is grounded in at least one `<file>:<line>` citation. Tests are first-class spec inputs — assertion text and `expect(N)` mock counts pin the contract precisely. Where multiple tests cover one behavior with different inputs, they are merged into a single BC. Confidence: **HIGH** = explicit assertion against an exact value/error variant; **MEDIUM** = assertion is partial (substring, length-only, "non-empty") OR signature-driven with happy-path test only; **LOW** = inferred from one test or source code without a test.

---

## BC stats summary

| Subject area | BCs HIGH | BCs MEDIUM | BCs LOW | Untested invariants from Pass 2 |
|---|---:|---:|---:|---|
| 1. Auth & Identity | 14 | 7 | 2 | INV-11 (partially) |
| 2. Issue read (list/view/comments/changelog) | 17 | 6 | 1 | none |
| 3. Issue write (create/edit/move/assign/comment/link/open/remote-link) | 16 | 5 | 1 | INV-14 (transit-name⇒current-status idempotency) |
| 4. Issue assets / CMDB | 10 | 4 | 1 | none |
| 5. Boards & Sprints | 7 | 3 | 0 | INV-9 partially (`MAX_SPRINT_ISSUES = 50`) |
| 6. Worklogs & duration | 5 | 1 | 0 | none |
| 7. Teams | 4 | 2 | 0 | none |
| 8. Users (search/list/view + disambiguation) | 8 | 1 | 0 | none |
| 9. Projects & Queues | 6 | 2 | 0 | none |
| 10. Configuration (figment, profile precedence, migration) | 9 | 2 | 1 | none |
| 11. Cache (TTL, per-profile, format-change resilience) | 7 | 2 | 1 | INV-10 has unit but no integration |
| 12. Output formatting (table/JSON/ADF/no-color/no-input) | 6 | 4 | 1 | INV-25 partially |
| 13. Error handling (JrError variants, exit codes, messages) | 11 | 3 | 0 | none |
| 14. Build-time (XOR obfuscation, embedded port 53682) | 5 | 1 | 1 | INV-13/15/16 from unit tests; integration deferred |
| 15. Runtime concerns (Ctrl+C, 429 retry, 401 dispatch, pagination) | 9 | 2 | 0 | none |
| **Totals** | **134** | **45** | **9** | **see §3.5** |

(Counts are before convergence — Phase B will refine LOW into HIGH/MEDIUM where possible.)

---

## 1. Test inventory by subject area

### 1.1 Integration test files (36 files, 16,958 LOC, 324 `#[test]`/`#[tokio::test]`)

| File | LOC | Subject area | Notes |
|---|---:|---|---|
| `tests/cli_handler.rs` | 2,134 | Issue write (assign, move, comment, etc.) via process invocation | Largest. Uses `JR_BASE_URL`+`JR_AUTH_HEADER` env override + `--output json`. |
| `tests/issue_commands.rs` | 1,920 | Issue read+write API client surface (`search_issues`, `get_issue`, `transition_issue`, `add_comment`, `find_story_points_field_id`, link types) | Library-level wiremock tests against `JiraClient::new_for_test`. |
| `tests/assets.rs` | 1,799 | Assets/CMDB end-to-end | search/view/tickets/schemas/types/schema. |
| `tests/issue_changelog.rs` | 1,722 | Issue changelog filtering, author needle, date format | Heavy snapshot / structured-output coverage. |
| `tests/all_flag_behavior.rs` | 686 | `--all` vs `--limit` cap (DEFAULT_LIMIT=30) for issue list, user search, user list, board view, sprint current, issue changelog | Pins request body `maxResults` shape. |
| `tests/user_pagination.rs` | 520 | `user list --all` server-side pagination (advances startAt by requested maxResults, NOT by returned count). | Cursor pagination semantics. |
| `tests/sprint_commands.rs` | 515 | Sprint list/current/add/remove, scrum-only check, MAX_SPRINT_ISSUES=50, --all, --limit. | |
| `tests/team_column_parity.rs` | 483 | Team column rendering parity across sprint board / issue list. | |
| `tests/queue.rs` | 478 | JSM queue list/view, AQL via `aqlFunction`. | |
| `tests/api_client.rs` | 444 | HTTP client: 401, 401+scope-mismatch, 429-retry, send_raw, error-message extraction precedence chain. | First-class for cross-cutting BC. |
| `tests/issue_create_json.rs` | 429 | Create/edit JSON output shape (`{"key": "FOO-123"}`), markdown→ADF, label add/remove prefixes. | |
| `tests/issue_list_errors.rs` | 423 | board config 404 (exit 64), 500 (exit 1), sprint error fallback to project JQL, 401 reauth, network drop, ambiguous status (exit 64 + no JQL fired). | |
| `tests/user_commands.rs` | 416 | User search/list/view, single-user GET + disambiguation. | |
| `tests/board_commands.rs` | 411 | Board list (with type filter), board view limit/all, auto-resolve. | |
| `tests/comments.rs` | 402 | Add/list comments, paginated, `--internal` JSM property, 5xx friendly message. | |
| `tests/issue_remote_link.rs` | 348 | Remote-link create + 201 response shape. | |
| `tests/cli_smoke.rs` | 334 | CLI surface smoke (`--help`, `--version`, clap conflicts: assign --to/--account-id/--unassign, --description/--description-stdin, --open/--status, --all/--limit, --created-after/--recent, --points/--no-points). | Process-level, no server. |
| `tests/auth_profiles.rs` | 333 | Multi-profile auth, fresh-install, switch unknown, status fresh, logout unknown, remove active, **flag>env>config>"default" precedence**, global `--profile` propagation. Two `#[ignore]` keyring-gated tests. | |
| `tests/project_commands.rs` | 323 | Project fields/list, statuses, CMDB-field discovery wiring. | |
| `tests/duplicate_user_disambiguation.rs` | 275 | `--no-input` on duplicate display names → exit non-zero with email + accountId in stderr. Covers list/assign/create. | |
| `tests/input_validation.rs` | 253 | `project_exists`, `get_all_statuses`, partial_match integration with project statuses, 404 → ApiError variant. | |
| `tests/team_object_shape.rs` | 243 | `IssueFields::team_id` accepts string-UUID and `{id: "<uuid>"}` object; rejects non-string id. | |
| `tests/issue_view_errors.rs` | 206 | `issue view` 500/401/network-drop/corrupt-team-cache fallback inline (no panic). | |
| `tests/team_commands.rs` | 196 | Org metadata GraphQL, `list_teams`, error-path coverage. | |
| `tests/cmdb_fields.rs` | 189 | `find_cmdb_fields` (filters by `com.atlassian.jira.plugins.cmdb:cmdb-object-cftype` schema), modern CMDB extraction (objectKey + label), null field empty, `enrich_assets` ID→name resolution via workspace + per-object GET. | |
| `tests/migration_legacy.rs` | 172 | `[instance]/[fields]` → `[profiles.default]` migration, idempotent on second load, on-disk file no longer carries legacy sections. | |
| `tests/worklog_commands.rs` | 171 | `add_worklog`, `list_worklogs`, 5xx + 401 friendly messages. | |
| `tests/issue_resolution.rs` | 158 | `issue resolutions` JSON+table, 400 "resolution required" → `--resolution` hint with `jr issue resolutions` discovery pointer. | |
| `tests/assets_errors.rs` | 153 | assets 5xx/401/network-drop friendly errors. | |
| `tests/project_meta.rs` | 126 | `get_or_fetch_project_meta` returns service_desk_id for service-desk project; software project has none; `require_service_desk` errors with "Jira Software project" / "Queue commands require" message. | |
| `tests/auth_refresh.rs` | 106 | `auth refresh --no-input` against unconfigured profile → exit 64, message names "no URL configured" + `jr auth login --url`. Help text. | |
| `tests/auth_login_config_errors.rs` | 97 | Malformed TOML → exit 78 (ConfigError); on-disk file unchanged (no silent overwrite). | |
| `tests/oauth_embedded_login.rs` | 32 | `#[ignore]`-gated stub for full embedded-OAuth wiremock test (deferred). | |
| `tests/common/fixtures.rs` | 446 | Shared fixture builders. | Not test code itself. |
| `tests/common/mock_server.rs` | 13 | wiremock helper. | |
| `tests/common/mod.rs` | 2 | mod glue. | |

### 1.2 Unit test modules in src/ (43 modules, ~607 fns)

| Module | Tests | Subject area |
|---|---:|---|
| `src/adf.rs` | 69 | ADF: text→ADF, markdown→ADF, ADF→text, table render, code blocks, headings, lists. |
| `src/cli/auth.rs` | 44 | CLI handlers + JSON output shapes for login/refresh/list/status/logout/remove. |
| `src/jql.rs` | 43 | escape_value, validate_duration (`4w2d` rejected), validate_asset_key, validate_date, build_asset_clause, strip_order_by + property tests. |
| `src/cli/issue/changelog.rs` | 38 | Author needle smart constructor, classification, format-date, tests on ChangelogItem display, reverse ordering, --field substring filter. |
| `src/config.rs` | 37 | Profile-name validation, figment layering, migration, JR_PROFILE precedence, profile resolution. |
| `src/types/jira/issue.rs` | 36 | story_points (Int/Float/null/string), team_id (string-UUID, object-id, non-string-id), issue field tolerance. |
| `src/cache.rs` | 27 | TTL/expired/corrupt → Ok(None), per-profile namespacing, write/read round-trip. |
| `src/cli/issue/list.rs` | 26 | JQL composition, project scope, --open via `statusCategory != Done`, date-clause synthesis, --asset auto-enables --assets. |
| `src/api/auth.rs` | 22 | Per-profile keychain key naming, scope set pinning, default scopes regression test. |
| `src/cli/api.rs` | 23 | Raw passthrough. |
| `src/cli/issue/helpers.rs` | 21 | Team UUID resolution, story-points assignment, user resolution. |
| `src/cli/assets.rs` | 21 | AQL building, --open via colorName, --status filter. |
| `src/api/assets/linked.rs` | 20 | extract_linked_assets variants, enrichment, per-field iteration. |
| `src/types/assets/linked.rs` | 17 | LinkedAsset display, id-fallback hint. |
| `src/duration.rs` | 16 | Combined units (1w2d3h30m), case-insensitive, error messages. |
| `src/api/pagination.rs` | 14 | OffsetPage, CursorPage, ServiceDeskPage, AssetsPage (bool-or-string is_last). |
| `src/partial_match.rs` | 12 | Single-substring → Ambiguous (not Exact); ExactMultiple for duplicates; property tests. |
| `src/cli/queue.rs` | 11 | Service-desk discovery, queue partial-match. |
| `src/cli/issue/json_output.rs` | 11 | `{"key", "status", "transitioned"}`, `{"key", "assignee", "changed"}`, `{"id", "self"}` shapes. |
| `src/api/jira/fields.rs` | 10 | story-points + CMDB field discovery filters. |
| `src/api/auth_embedded.rs` | 8 | EmbeddedOAuthApp Debug redacts secret, build_embedded_app rejects empty inputs, OnceLock, presence check without decode. |
| `src/cli/issue/format.rs` | 8 | Row formatting, points display, comment-date format. |
| `src/error.rs` | 8 | exit_code mapping (0/1/2/64/78/130). |
| `src/types/assets/object.rs` | 9 | AssetObject + ObjectTypeAttributeDef defaults. |
| `src/cli/issue/workflow.rs` | 6 | Idempotent move (current==target, transition-name maps to current `to`); resolution resolver no auto-promote. |
| `src/cli/sprint.rs` | 6 | Scrum-only error; MAX_SPRINT_ISSUES=50; --board override. |
| `src/types/jira/changelog.rs` | 4 | ChangelogEntry parse with/without author. |
| `src/types/assets/schema.rs` | 4 | Object schema/type defaults for object_count. |
| `src/types/jsm/queue.rs` | 4 | Queue + QueueIssueKey serde. |
| `src/api/jira/issues.rs` | 4 | search/get/edit body shape. |
| `src/api/assets/objects.rs` | 4 | AQL search, get_object body shape, key resolution. |
| `src/api/jira/users.rs` | 3 | search/list/view body shape. |
| `src/cli/board.rs` | 3 | Board list type filter. |
| `src/cli/user.rs` | 3 | User dispatch. |
| `src/cli/mod.rs` | 3 | resolve_effective_limit (`--all` → None; default DEFAULT_LIMIT=30). |
| `src/types/assets/ticket.rs` | 2 | colorName tolerance. |
| `src/output.rs` | 2 | Table render snapshot. |
| `src/api/rate_limit.rs` | 2 | Retry-After int + http-date parse. |
| `src/types/jira/board.rs` | 2 | BoardConfig default board_type. |
| `src/observability.rs` | 1 | log_parse_failure_once gate. |
| `src/api/jira/links.rs` | 1 | Link create body shape. |
| `src/api/jira/resolutions.rs` | 1 | Resolution list parse. |
| `src/cli/issue/create.rs` | 1 | Field-build helper. |

### 1.3 Test infrastructure (cross-file conventions)

- **`JR_BASE_URL`** + **`JR_AUTH_HEADER`** env vars — used together in 30+ test files to inject wiremock URL and bypass keychain.
- **`JR_SERVICE_NAME`** — keychain service-name override. Used in auth tests so subprocesses don't touch the developer's real `jr-jira-cli` keychain.
- **`JR_RUN_KEYRING_TESTS=1`** — gates 8 `#[ignore]` keyring round-trip tests.
- **`JR_RUN_OAUTH_INTEGRATION=1`** — gates the deferred embedded-OAuth integration test.
- **`XDG_CONFIG_HOME`** + **`XDG_CACHE_HOME`** — set per-test to tempdirs to isolate config/cache.
- **`tempfile::tempdir`** + per-test `.jr.toml` — for project-scope tests.
- **`Mock::expect(N)`** — pins exact request count. Tests use it both as positive (must fire ≥1) and negative (`expect(0)` to assert short-circuit, e.g., issue_list_errors.rs:388).

---

## 2. Behavioral Contracts catalog

> **Format**: BC ID → confidence → source citations → behavior → inputs → effects → edges → error variants.

---

### 2.1 Auth & Identity

#### BC-001: `auth list` against fresh-install returns empty array
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:53-60`
**Behavior**: When no `~/.config/jr/config.toml` exists (or no `[profiles.*]` keys), `jr auth list --output json` exits 0 and stdout contains `[]`.
**Effects**: stdout = `[]`, exit 0, no HTTP, no keychain access.
**Edge cases tested**: fresh install with no config file at all.
**Error variant**: none.

#### BC-002: `auth status` against fresh install succeeds with helpful stderr
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:62-75`
**Behavior**: `jr auth status` against an uninitialized config exits 0 (success) and prints `No profiles configured` to stderr. This contract supports first-run probes by setup scripts/CI.
**Edge cases**: no config.toml, no `[profiles]` section.
**Error variant**: none — intentionally success.

#### BC-003: `auth switch <unknown>` exits 64
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:42-50`
**Behavior**: Switching to an unknown profile exits 64 (`UserError`) with no config mutation.
**Error variant**: `JrError::UserError`.

#### BC-004: `auth status --profile <unknown>` exits 64 with "unknown profile"
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:78-96`
**Behavior**: When the explicit `--profile` flag names a profile not present in `[profiles.*]`, `auth status` exits 64 with stderr containing `unknown profile`.
**Error variant**: `JrError::UserError`.

#### BC-005: `auth logout --profile <unknown>` exits 64
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:98-118`
**Behavior**: Logout against unknown profile exits 64 with `unknown profile` in stderr.
**Error variant**: `JrError::UserError`.

#### BC-006: `auth remove <active>` is rejected with exit 64
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:120-140`
**Behavior**: Attempting to remove the currently-active profile exits 64 with stderr containing `cannot remove active`.
**Inputs**: `--no-input`, positional `default` (which is the active profile).
**Effects**: no file changes, no keychain deletion.
**Error variant**: `JrError::UserError`.

#### BC-007: Profile resolution precedence: flag > JR_PROFILE env > config.default_profile > "default"
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:142-186` (test asserts exactly one active profile = `from-flag` when all three layers are populated and disagree); `src/config.rs:95-110` impl.
**Behavior**: `Config::load_with(cli_profile)` resolves the active profile by precedence chain. Tested by populating three profiles (from-config / from-env / from-flag) and confirming flag wins.
**Effects**: `Config.active_profile_name` set; `auth list --output json` returns exactly one element with `"active": true`.

#### BC-008: Global `--profile` flag propagates to `auth status`
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:193-231`
**Behavior**: `jr --profile sandbox auth status` (without subcommand-level `--profile`) targets the sandbox profile. Regression for "main.rs composes effective profile via subcmd.profile.or(cli.profile)".
**Effects**: stderr/stdout reflect sandbox URL/name.

#### BC-009: `auth login --profile <new>` creates a profile with URL even if not yet present
**Confidence**: HIGH (test exists but `#[ignore]`-gated by JR_RUN_KEYRING_TESTS)
**Sources**: `tests/auth_profiles.rs:241-280`
**Behavior**: Login against a not-yet-existing profile uses lenient config load to skip the strict active-profile-existence check, then writes the new `[profiles.NEW]` block with URL + auth_method.
**Effects**: writes config, writes shared `email`/`api-token` keychain keys.

#### BC-010: `auth login --profile X` succeeds even when JR_PROFILE points to an unrelated nonexistent profile
**Confidence**: HIGH (`#[ignore]`-gated)
**Sources**: `tests/auth_profiles.rs:290-332`
**Behavior**: Login uses lenient load throughout (top-level + internal reloads in login_token/login_oauth) so `JR_PROFILE=ghost` doesn't abort an in-flight create of a different profile.

#### BC-011: `auth refresh` against unconfigured profile exits 64 naming "no URL configured"
**Confidence**: HIGH
**Sources**: `tests/auth_refresh.rs:43-106`
**Behavior**: Without `--no-input`, refresh would prompt for credentials; with `--no-input` AND no profile URL set, refresh exits 64 with stderr matching `no URL configured` + `jr auth login` + `--url`. Refuses to clear creds first.
**Edge cases**: empty config + scrubbed JR_INSTANCE_* env.
**Error variant**: `JrError::UserError`. Critically: stderr does NOT contain `panic`.

#### BC-012: Malformed config TOML errors with exit 78 (ConfigError) and does NOT overwrite the file
**Confidence**: HIGH
**Sources**: `tests/auth_login_config_errors.rs:18-97`
**Behavior**: When `~/.config/jr/config.toml` is malformed (e.g., unclosed table header), `auth login --oauth ...` exits 78. Stderr contains `toml` or `parse`. The on-disk file is byte-identical to before. Pre-fix bug: `unwrap_or_default()` swallowed parse errors and `save_global()` overwrote with defaults.
**Error variant**: `JrError::ConfigError`.

#### BC-013: `auth logout` deletes only per-profile OAuth tokens (not shared API-token / OAuth app credentials)
**Confidence**: MEDIUM (per CLAUDE.md gotcha + Pass 2 inv-11; integration via `#[ignore]` keyring tests)
**Sources**: Pass 2 §2b.1 + `src/api/auth.rs:111-169` + `src/cli/auth.rs::handle_logout`
**Behavior**: Deletes `<profile>:oauth-access-token` and `<profile>:oauth-refresh-token`. Profile entry remains in config.toml. Shared keys (`email`, `api-token`, `oauth_client_id`, `oauth_client_secret`) are untouched.

#### BC-014: `auth remove <name>` deletes profile entry, per-profile OAuth tokens, and per-profile cache directory
**Confidence**: MEDIUM (Pass 2 §2b.1 + `cache::clear_profile_cache`)
**Sources**: `src/cli/auth.rs::handle_remove` + `src/cache.rs:82-88`
**Behavior**: Delete sequence: profile entry from config, namespaced OAuth keys, `~/.cache/jr/v1/<name>/` directory. Errors if name == active.
**Error variant**: `JrError::UserError` if active.

#### BC-015: 401 with `scope does not match` body dispatches to InsufficientScope, not NotAuthenticated
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:99-144`
**Behavior**: 401 body containing the literal substring `scope does not match` (case-insensitively) produces `JrError::InsufficientScope` whose Display contains: `Insufficient token scope`, the raw gateway message, the workaround scope `write:jira-work`, the workaround `OAuth 2.0`, and the issue link `github.com/Zious11/jira-cli/issues/185`.
**Error variant**: `JrError::InsufficientScope` (exit 2).

#### BC-016: 401 without scope-mismatch substring falls through to NotAuthenticated
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:147-181`
**Behavior**: 401 with `Session expired` body produces `Not authenticated` and does NOT contain `Insufficient token scope`. Pins the dispatch boundary.

#### BC-017: 401-scope-mismatch matches case-insensitively
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:184-216`
**Behavior**: 401 with body `Unauthorized; Scope Does Not Match` (mixed case) still dispatches to InsufficientScope. Pins `to_ascii_lowercase` step.

#### BC-018: Non-401 status with scope-mismatch substring does NOT dispatch to InsufficientScope
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:219-255`
**Behavior**: 403 body with `scope does not match policy` produces generic `API error (403)`, not InsufficientScope. Status gate prevents broadening.

#### BC-019: Embedded OAuth app `Debug` redacts client_secret
**Confidence**: HIGH
**Sources**: `src/api/auth_embedded.rs::tests::embedded_oauth_app_debug_redacts_secret` (per Pass 2 inv-14, file:34-41)
**Behavior**: `format!("{:?}", EmbeddedOAuthApp{...})` never emits the plaintext secret. Custom Debug impl substitutes a placeholder.

#### BC-020: Build with empty XOR inputs produces `embedded_oauth_app() == None`
**Confidence**: HIGH
**Sources**: `src/api/auth_embedded.rs:100-106` + `build_embedded_app_rejects_empty_inputs` test
**Behavior**: Setting `JR_BUILD_OAUTH_CLIENT_ID=""` (or any zero-length cipher/key) at build time emits a binary whose embedded accessor returns `None`. BYO/prompt fallback proceeds.

#### BC-021: `embedded_oauth_app_present()` reports presence without decoding
**Confidence**: HIGH
**Sources**: `src/api/auth_embedded.rs:132-136` + tests at 244-249
**Behavior**: Presence check inspects only `EMBEDDED_ID.is_some_and(|s| !s.is_empty())` etc. Does not invoke `decode()`. Used by `auth status` so reporting `OAuthAppSource::Embedded` doesn't materialize plaintext in memory.

#### BC-022: `OAuthAppSource` reports correct source
**Confidence**: MEDIUM
**Sources**: `src/api/auth_embedded.rs:46-57` + `src/cli/auth.rs::peek_oauth_app_source` (per Pass 2 §2a.2)
**Behavior**: `auth status` reports `Flag`/`Env`/`Keychain`/`Embedded`/`Prompt`/`None` based on credential resolution chain (highest available wins).

#### BC-023: `default` profile lazy-migrates legacy flat OAuth keys; non-default profiles never inherit
**Confidence**: MEDIUM (CLAUDE.md gotcha + Pass 2 inv-12 + `api/auth.rs:111-169`)
**Sources**: `src/api/auth.rs:24-32, 111-169`
**Behavior**: First read against `default:oauth-access-token` falls back to legacy `oauth-access-token`, copies into namespaced key, deletes legacy. Other profiles (`sandbox` etc.) only read their own namespaced keys — never the legacy keys.

#### BC-024: `refresh_oauth_token` resolves credentials internally; callers pass only `profile`
**Confidence**: LOW (CLAUDE.md gotcha; signature-driven)
**Sources**: CLAUDE.md + `src/api/auth.rs:705-712`
**Behavior**: Function signature is `(profile: &str)` only — internally resolves keychain → embedded. Re-introducing `client_id/_secret` parameters short-circuits the resolver.

---

### 2.2 Issue read (list, view, comments, changelog)

#### BC-101: `issue list` cursor-paginates `POST /rest/api/3/search/jql`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:7-31, 130-166`
**Behavior**: `client.search_issues(jql, limit, fields)` issues `POST /rest/api/3/search/jql`, returns `{issues: Vec<Issue>, has_more: bool}`. Pagination via `nextPageToken` cursor.
**Effects**: HTTP POST.

#### BC-102: `issue list --jql X` wraps user JQL in parens and appends ORDER BY
**Confidence**: HIGH
**Sources**: `tests/all_flag_behavior.rs:54-66, 99-110`
**Behavior**: User-supplied `--jql project = ALL` becomes the literal JQL `(project = ALL) ORDER BY updated DESC`. Preserves original parenthesization at composition time.
**Edge cases**: when `ORDER BY` is already present, it should be stripped (Pass 2 §2b.2 #19).

#### BC-103: `--all` passes maxResults=50 (no limit cap); default passes maxResults=30
**Confidence**: HIGH
**Sources**: `tests/all_flag_behavior.rs:42-145`
**Behavior**: `body_partial_json({maxResults: 50})` matched under `--all`, `maxResults: 30` matched under default. Pinned by request body match.
**Inputs**: `--all` vs no flag (vs `--limit N` set explicitly).
**Effects**: API request body shape differs.

#### BC-104: Default cap is 30; truncation triggers `Showing N results` hint AND `approximate-count` follow-up
**Confidence**: HIGH
**Sources**: `tests/all_flag_behavior.rs:88-145`, `tests/sprint_commands.rs:78-101`
**Behavior**: When default 30-row cap is hit and more results exist, `jr` issues `POST /rest/api/3/search/approximate-count` with the ORDER BY-stripped JQL to compute the count for the "Showing 30 of ~42" hint. With `--all`, no truncation hint and no count call.
**Edge cases**: tested with sprint current as well (`Showing 30 results` in stderr).

#### BC-105: `issue list --status <single-substring>` exits 64 with `Ambiguous status` + candidate list, no JQL search fired
**Confidence**: HIGH
**Sources**: `tests/issue_list_errors.rs:368-422` (uses `expect(0)` on `/search/jql` to prove short-circuit)
**Behavior**: Single-substring match against project statuses routes through `MatchResult::Ambiguous`. Under `--no-input`, errors with stderr `Ambiguous status` + matched candidate (e.g., `In Progress`) and exit 64. Crucially, the JQL search HTTP call is never made.
**Error variant**: `JrError::UserError`.

#### BC-106: `issue list` board_id 404 → exit 64 with actionable message
**Confidence**: HIGH
**Sources**: `tests/issue_list_errors.rs:21-76`
**Behavior**: When `.jr.toml::board_id` points to a deleted/inaccessible board, exits 64 with stderr containing: `Board 42 not found or not accessible`, `board_id` (suggests removing it), `--jql` (suggests alternative).
**Error variant**: `JrError::UserError`.

#### BC-107: `issue list` board config 5xx → exit 1 with hint pointing at `--jql`
**Confidence**: HIGH
**Sources**: `tests/issue_list_errors.rs:78-130`
**Behavior**: 500 on board config exits 1 with stderr `Failed to fetch config for board 42` + `--jql` hint.

#### BC-108: `issue list` sprint list 5xx → exit 1 with hint
**Confidence**: HIGH
**Sources**: `tests/issue_list_errors.rs:132-194`
**Behavior**: 500 on sprint listing exits 1 with `Failed to list sprints for board 42` + `--jql` hint.

#### BC-109: `issue list` no active sprint → falls back to project-scoped JQL
**Confidence**: HIGH
**Sources**: `tests/issue_list_errors.rs:196-263`
**Behavior**: Empty `state=active` sprint list still succeeds the command — falls back to `project = PROJ` JQL and renders results. No error.

#### BC-110: `issue list` 401 → exit 2 + `Not authenticated` + `jr auth login` suggestion + no panic
**Confidence**: HIGH
**Sources**: `tests/issue_list_errors.rs:267-318`
**Behavior**: 401 from any HTTP call in the list flow surfaces `JrError::NotAuthenticated`. stderr contains both message and remediation. Critically: stderr must not contain `panic`.

#### BC-111: `issue list` network drop → exit 1 + `Could not reach` + `check your connection`
**Confidence**: HIGH
**Sources**: `tests/issue_list_errors.rs:320-360`
**Behavior**: connect-refused (privileged port :1) produces `JrError::NetworkError(host)` with friendly message.

#### BC-112: `issue view <key>` issues GET to `/rest/api/3/issue/<key>`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:34-53`, `tests/issue_view_errors.rs`
**Behavior**: Fetches issue. With `--output json`, prints raw JSON. Default: formatted text view.

#### BC-113: `issue view` 5xx → exit 1 + `API error (500)` + no panic
**Confidence**: HIGH
**Sources**: `tests/issue_view_errors.rs:18-56`
**Behavior**: 500 on view exits 1; stderr contains the literal `API error (500)`.
**Error variant**: `JrError::ApiError{ status:500, ...}`.

#### BC-114: `issue view` 401 → exit 2 with `Not authenticated` + `jr auth login`
**Confidence**: HIGH
**Sources**: `tests/issue_view_errors.rs:58-100`

#### BC-115: `issue view` corrupt teams.json cache is non-fatal; UUID + "name not cached" hint shown inline
**Confidence**: HIGH
**Sources**: `tests/issue_view_errors.rs:142-206`
**Behavior**: Truncated `teams.json` (parse error) is treated as cache miss (`Ok(None)` per `cache.rs:23-26`). Issue view succeeds and the Team row shows the raw UUID with `(name not cached — run 'jr team list --refresh')` hint.
**Effects**: stderr does NOT contain panic. Issue #194 — original "stderr warning" proposal was changed to inline hint.

#### BC-116: `issue comments <key>` paginates `/rest/api/3/issue/<key>/comment` with `expand=properties`
**Confidence**: HIGH
**Sources**: `tests/comments.rs:9-46, 73-158`
**Behavior**: Uses 100-per-page (`maxResults=100`). With `--limit N`, passes `maxResults=N`. Without limit, paginates until end.
**Edge cases**: empty list returns empty Vec.

#### BC-117: `issue comments` 5xx → `API error (500)` exit 1
**Confidence**: HIGH
**Sources**: `tests/comments.rs:163-200`

#### BC-118: `issue comments --internal` adds `sd.public.comment` property; pre-existing internal flag preserved on read
**Confidence**: MEDIUM
**Sources**: Pass 2 §2b.2 #24 + `src/api/jira/issues.rs:181-198`; `Comment::properties` shape
**Behavior**: Read shape preserves `EntityProperty[]`. The list_comments query uses `expand=properties`.

#### BC-119: `issue changelog --field <substr>` filters items by case-insensitive field substring
**Confidence**: MEDIUM (38 tests in `src/cli/issue/changelog.rs::tests`)
**Sources**: Pass 0 inventory; `src/cli/issue/changelog.rs:847 LOC, 38 tests`
**Behavior**: Filter applied client-side after `expand=changelog` fetch.

#### BC-120: `issue changelog --author X` smart-constructs author needle
**Confidence**: MEDIUM (Pass 2 §2b)
**Sources**: `src/cli/issue/changelog.rs` author needle
**Behavior**: Input with `:` or 12+ chars containing a digit → exact accountId match. Else displayName-or-accountId substring.

#### BC-121: `issue changelog --reverse` reverses chronological order
**Confidence**: MEDIUM
**Sources**: per `src/cli/issue/changelog.rs::tests` and CLI flag definition

#### BC-122: `client.list_comments(key, None)` lists ALL comments via offset pagination
**Confidence**: HIGH
**Sources**: `tests/comments.rs:104-158`
**Behavior**: With limit=None, advances `startAt` by 100 until total reached.

#### BC-123: `find_story_points_field_id` filters fields by float type + name match
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:168-186`
**Behavior**: Returns Vec of (id, name) tuples for fields with numeric/float schema and name = "Story Points". Discovery for init/dynamic resolution.

#### BC-124: `IssueFields::story_points("customfield_X")` returns `None` for non-numeric values
**Confidence**: HIGH
**Sources**: Pass 2 inv-16; `src/types/jira/issue.rs:83-85` test at 362-366
**Behavior**: Numeric coercion only — string "not a number" → None.

---

### 2.3 Issue write (create / edit / move / assign / comment / link / open / remote-link)

#### BC-201: `issue assign --account-id <id>` PUTs `/issue/<key>/assignee` with `{accountId: <id>}` and emits `{key, assignee, assignee_account_id, changed: true}`
**Confidence**: HIGH
**Sources**: `tests/cli_handler.rs:58-91`
**Behavior**: Body partial-JSON match `{accountId: "direct-id-001"}`. Output JSON includes `"changed": true`, `"key": "HDL-1"`, `"assignee": "direct-id-001"`, `"assignee_account_id": "direct-id-001"`.
**Effects**: HTTP PUT.

#### BC-202: `issue assign --to <name>` resolves via assignable user search and assigns
**Confidence**: HIGH
**Sources**: `tests/cli_handler.rs:93-133`
**Behavior**: GET `/rest/api/3/user/assignable/search?query=<name>&issueKey=<key>`, then PUT with the resolved accountId. Output `"assignee": "Jane Doe"`, `"changed": true`.

#### BC-203: `issue assign --to me` resolves to current user via `/myself`
**Confidence**: HIGH
**Sources**: `tests/cli_handler.rs:135-` (test_handler_assign_self)
**Behavior**: GET `/myself` to discover accountId, then PUT.

#### BC-204: `issue assign` is idempotent — already-assigned-to-target → exit 0 + `"changed": false`
**Confidence**: MEDIUM (Pass 2 §2b.5 inv + cli_handler tests)
**Sources**: Pass 2 inv-1 corollary; `src/cli/issue/workflow.rs::handle_assign`
**Behavior**: Get current assignee, compare, skip PUT if equal.

#### BC-205: `issue assign --unassign` PUTs `{accountId: null}`
**Confidence**: MEDIUM
**Sources**: Pass 2 §2b.1 + `src/cli/issue/workflow.rs`

#### BC-206: clap conflicts: `--to` ⊕ `--account-id` ⊕ `--unassign` (mutually exclusive)
**Confidence**: HIGH
**Sources**: `tests/cli_smoke.rs:170-211`
**Behavior**: Any two of `{--to, --account-id, --unassign}` together → clap exit non-zero with stderr `cannot be used with`.

#### BC-207: `issue move <key> <target>` is idempotent (current==target → no HTTP transition)
**Confidence**: HIGH (Pass 2 inv-1 + `src/cli/issue/workflow.rs:192-224` tests)
**Sources**: `src/cli/issue/workflow.rs::tests` (6 inline tests)
**Behavior**: Fetches current status. If `current == target` (case-insensitive), exits 0 with `"transitioned": false` and message; no POST.

#### BC-208: `issue move` 400 "resolution required" → `--resolution` hint with discovery pointer
**Confidence**: HIGH
**Sources**: `tests/issue_resolution.rs:88-158`
**Behavior**: 400 body `{errors: {resolution: "Field 'resolution' is required"}}` is rewritten to user-facing stderr containing `--resolution` and `jr issue resolutions` (the discovery command).

#### BC-209: `issue move` exits non-zero with helpful hint when resolution required
**Confidence**: HIGH
**Sources**: `tests/issue_resolution.rs:148-157`
**Behavior**: Test asserts non-zero exit + presence of `--resolution` and `jr issue resolutions` in stderr.

#### BC-210: `issue resolutions` reads cache-first (7d TTL); JSON output is array of `{name, ...}` objects
**Confidence**: HIGH
**Sources**: `tests/issue_resolution.rs:11-46, 49-86`
**Behavior**: GET `/rest/api/3/resolution`, cache for 7 days. JSON output `[{name, id, description}, ...]`. Table output shows Name + Description columns. Resolutions without `id` are dropped on cache write (per Pass 2 inv).

#### BC-211: `issue create` POSTs `/rest/api/3/issue` with field-build payload
**Confidence**: HIGH (29 tests in `tests/issue_create_json.rs`)
**Sources**: `tests/issue_create_json.rs`
**Behavior**: Output JSON `{"key": "FOO-123"}`. HTTP body includes summary, project, type, optional priority, labels, description (ADF), team UUID, story points.

#### BC-212: `issue create --markdown -d '...' ` converts markdown to ADF before POST
**Confidence**: MEDIUM (29 tests in issue_create_json + 69 ADF unit tests)
**Sources**: `tests/issue_create_json.rs`, `src/adf.rs::tests`

#### BC-213: `issue edit --label add:foo --label remove:bar` interprets prefix and merges with existing labels
**Confidence**: MEDIUM (issue_create_json tests)
**Sources**: `tests/issue_create_json.rs`
**Behavior**: `add:` and `remove:` prefixes adjust existing labels; bare label replaces.

#### BC-214: `issue edit --description --description-stdin` (clap conflict)
**Confidence**: HIGH
**Sources**: `tests/cli_smoke.rs:34-48`
**Behavior**: Both flags together → exit non-zero with `cannot be used with`.

#### BC-215: `issue edit --points X --no-points` (clap conflict)
**Confidence**: HIGH
**Sources**: `tests/cli_smoke.rs:280-287`

#### BC-216: `issue link <k1> <k2> [--type T]` POSTs `/rest/api/3/issueLink`; default type is "Relates"
**Confidence**: HIGH
**Sources**: `src/api/jira/links.rs::tests`, `src/cli/issue/links.rs`

#### BC-217: `issue unlink` lists, filters by k2 + type, DELETEs each match
**Confidence**: MEDIUM
**Sources**: `src/cli/issue/links.rs`

#### BC-218: `issue link-types` lists `/rest/api/3/issueLinkType`
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:188-`

#### BC-219: `issue comment <key> --internal` adds `properties: [{key:"sd.public.comment", value:{internal:true}}]`
**Confidence**: HIGH (Pass 2 inv-24)
**Sources**: `src/api/jira/issues.rs::add_comment(internal: bool)` + integration tests
**Behavior**: On non-JSM projects, Jira silently ignores the property.

#### BC-220: `issue open <key>` launches browser to `<base>/browse/<key>`
**Confidence**: MEDIUM
**Sources**: Pass 2 §2b.1 + `src/cli/issue/workflow.rs::handle_open`
**Behavior**: Uses `open` crate; no HTTP.

#### BC-221: `issue open --url-only` prints URL to stdout (no browser launch)
**Confidence**: MEDIUM
**Sources**: Pass 2 §2b.1
**Behavior**: For AI-agent / scripted use.

#### BC-222: `issue remote-link <key> --url X` POSTs `/issue/<key>/remotelink`; returns `{"id": <u64>, "self": <url>}` JSON
**Confidence**: HIGH
**Sources**: `tests/issue_remote_link.rs` (348 LOC), Pass 2 §2a.2 `CreateRemoteLinkResponse`

#### BC-223: `issue remote-link` defaults `--title` to URL when omitted
**Confidence**: MEDIUM
**Sources**: Pass 2 §2b.1

#### BC-224: `issue create --to` and `--account-id` are mutually exclusive (clap)
**Confidence**: HIGH
**Sources**: `tests/cli_smoke.rs:215-235`

#### BC-225: `transition_issue(key, id, fields)` includes `fields` only when Some
**Confidence**: HIGH
**Sources**: `tests/issue_commands.rs:80-128`
**Behavior**: `Some(&fields)` → body has `"fields"` key. `None` → body has no `"fields"` key (asserted by string negation on raw body).

---

### 2.4 Issue assets / CMDB

#### BC-301: `find_cmdb_fields()` filters by schema custom string `com.atlassian.jira.plugins.cmdb:cmdb-object-cftype`
**Confidence**: HIGH
**Sources**: `tests/cmdb_fields.rs:50-83`
**Behavior**: Returns `Vec<(String, String)>` of (id, name) tuples matching only CMDB custom fields. Story points and summary are filtered out.
**Edge cases**: empty result when no CMDB fields exist.

#### BC-302: `extract_linked_assets` reads custom field values: array of `{id, key, name, ...}` objects, label, objectKey
**Confidence**: HIGH
**Sources**: `tests/cmdb_fields.rs:86-118`
**Behavior**: Parses `"customfield_10191": [{"label": "Acme Corp", "objectKey": "OBJ-1"}]` into `LinkedAsset { name: Some("Acme Corp"), key: Some("OBJ-1"), ... }`.

#### BC-303: `extract_linked_assets` returns empty for null field
**Confidence**: HIGH
**Sources**: `tests/cmdb_fields.rs:120-146`

#### BC-304: `enrich_assets(client, &mut [LinkedAsset])` resolves `id`-only assets to name/key/type via per-object GET
**Confidence**: HIGH
**Sources**: `tests/cmdb_fields.rs:148-189`
**Behavior**: For each `LinkedAsset` with id but no name, GET `/jsm/assets/workspace/<wid>/v1/object/<id>?includeAttributes=false`. Mutates the asset in place: `key = Some("OBJ-88")`, `name = Some("Acme Corp")`, `asset_type = Some("Client")`.

#### BC-305: `LinkedAsset::display()` falls back to `#<id> (run 'jr init' to resolve asset names)` when only id present
**Confidence**: HIGH
**Sources**: Pass 2 inv-19 + `src/types/assets/linked.rs::tests::display_id_fallback_with_hint`

#### BC-306: AQL `Key` attribute is literal capital-K
**Confidence**: HIGH
**Sources**: `src/jql.rs:70`, Pass 2 inv-4, property tests in `src/jql.rs::tests`
**Behavior**: `build_asset_clause` emits `Key = "<key>"`, not `objectKey`.

#### BC-307: `aqlFunction()` LHS uses field NAME, not customfield_NNNNN
**Confidence**: HIGH
**Sources**: `src/jql.rs:67-74` + Pass 2 inv-4
**Behavior**: Tuples are `(_, name)` — id is destructured-and-ignored. Reversing this would cause Jira to reject AQL at runtime.

#### BC-308: Multiple CMDB fields → parenthesized OR-join
**Confidence**: HIGH
**Sources**: `src/jql.rs:77-81` + tests
**Behavior**: Two CMDB fields produces `("X" IN aqlFunction(...) OR "Y" IN aqlFunction(...))`.

#### BC-309: `validate_asset_key("CUST-5")` → Ok; `"CUST"` → Err; `"5-CUST"` → Err
**Confidence**: HIGH
**Sources**: `src/jql.rs:39-54` + tests
**Behavior**: ASCII alphanumeric prefix + `-` + ASCII digit suffix, both nonempty.

#### BC-310: `assets search Q` discovers workspace ID first (cache or API)
**Confidence**: HIGH
**Sources**: `tests/cmdb_fields.rs:148-189` (workspace mock)
**Behavior**: GET `/rest/servicedeskapi/assets/workspace` → `{values: [{workspaceId: "ws-123"}]}`. Cached as `WorkspaceCache`.

#### BC-311: `assets search` 5xx → exit 1 + `API error (500)` + no panic
**Confidence**: HIGH
**Sources**: `tests/assets_errors.rs:21-64`

#### BC-312: `assets search` 401 → exit 2 + `Not authenticated` + `jr auth login`
**Confidence**: HIGH
**Sources**: `tests/assets_errors.rs:67-113`

#### BC-313: `assets search` network drop → exit 1 + `Could not reach`
**Confidence**: HIGH
**Sources**: `tests/assets_errors.rs:116-153`

#### BC-314: `assets tickets <key> --open` filters `status.colorName != "green"`
**Confidence**: MEDIUM (Pass 2 inv-20 + `src/cli/assets.rs:303-321`)
**Sources**: `src/cli/assets.rs::tests` (21 unit tests)
**Behavior**: Tickets with no status are *included* under `--open`, *excluded* under `--status`.

#### BC-315: `assets tickets --open` and `--status` are clap-conflicting
**Confidence**: HIGH
**Sources**: `tests/cli_smoke.rs:51-58`

---

### 2.5 Boards & Sprints

#### BC-401: `client.list_boards(project, type)` GETs `/rest/agile/1.0/board` with query params
**Confidence**: HIGH
**Sources**: `tests/board_commands.rs:111-`, `tests/sprint_commands.rs:23-39`
**Behavior**: Boards filtered by `projectKeyOrId=PROJ` + `type=scrum|kanban`.

#### BC-402: `sprint list/current` errors on kanban boards with explicit message
**Confidence**: HIGH (Pass 2 inv-23)
**Sources**: `src/cli/sprint.rs:79-86`, inline tests
**Behavior**: `if board_type != "scrum"` → bail with `Sprint commands are only available for scrum boards`.

#### BC-403: `sprint add (--sprint ID | --current)` — flags mutually exclusive
**Confidence**: HIGH
**Sources**: `tests/cli_smoke.rs:116-123`

#### BC-404: `sprint add` requires `--sprint` or `--current`
**Confidence**: HIGH
**Sources**: `tests/cli_smoke.rs:126-133`

#### BC-405: `MAX_SPRINT_ISSUES = 50` cap on `sprint add` and `sprint remove`
**Confidence**: MEDIUM (Pass 2 inv-22; explicit unit tests not yet sampled)
**Sources**: `src/cli/sprint.rs:35-41, 55-61, 107` + 6 inline tests

#### BC-406: `sprint current` truncates to default 30; with `--all` returns full set; under-limit no `Showing` hint
**Confidence**: HIGH
**Sources**: `tests/sprint_commands.rs:63-180`
**Behavior**: 35 issues + default → 30 in stdout + `Showing 30 results`. With `--all` → 35 issues + no hint. With 10 issues → no hint.

#### BC-407: `sprint current --all --limit N` is clap-conflicting
**Confidence**: HIGH
**Sources**: `tests/cli_smoke.rs:310-317`

#### BC-408: `board view --limit --all` clap conflict
**Confidence**: HIGH
**Sources**: `tests/board_commands.rs:96-106`, `tests/cli_smoke.rs:300-307`

#### BC-409: `client.get_sprint_issues(sprintId, jql, limit, fields)` with limit=Some(3) returns 3, has_more=true
**Confidence**: HIGH
**Sources**: `tests/board_commands.rs:23-71`

#### BC-410: Auto-resolve board: list scrum boards for project, pick first
**Confidence**: HIGH
**Sources**: `tests/sprint_commands.rs:23-61`

---

### 2.6 Worklogs & duration

#### BC-501: `client.add_worklog(key, seconds, message)` POSTs `/issue/<key>/worklog`
**Confidence**: HIGH
**Sources**: `tests/worklog_commands.rs:8-26`
**Behavior**: Returns `Worklog { id, time_spent_seconds, ... }`. 201 status.

#### BC-502: `client.list_worklogs(key)` paginates `/issue/<key>/worklog`
**Confidence**: HIGH
**Sources**: `tests/worklog_commands.rs:28-51`

#### BC-503: `worklog list` 5xx → `API error (500)` exit 1 + no panic
**Confidence**: HIGH
**Sources**: `tests/worklog_commands.rs:55-93`

#### BC-504: `worklog list` 401 → exit 2 + `Not authenticated` + `jr auth login`
**Confidence**: HIGH
**Sources**: `tests/worklog_commands.rs:95-120+`

#### BC-505: `parse_duration("1w2d3h30m", 8, 5)` accepts combined units; output is total seconds
**Confidence**: HIGH
**Sources**: `src/duration.rs::tests::test_complex` (per Pass 2 inv-6)
**Behavior**: Distinguished from JQL `validate_duration` which rejects combined units.

#### BC-506: `parse_duration` is case-insensitive (input lowercased first)
**Confidence**: HIGH
**Sources**: `src/duration.rs:6` (`input.to_lowercase()`)

#### BC-507: `parse_duration("")` errors `Duration cannot be empty`
**Confidence**: HIGH
**Sources**: `src/duration.rs:7-9`

#### BC-508: `parse_duration("5")` errors `Number without unit`
**Confidence**: HIGH
**Sources**: `src/duration.rs:38-42`

---

### 2.7 Teams

#### BC-601: `client.get_org_metadata(hostname)` issues GraphQL POST `/gateway/api/graphql` with tenantContexts query
**Confidence**: HIGH
**Sources**: `tests/team_commands.rs:8-26`
**Behavior**: Returns `TenantContext { org_id, cloud_id }` (ADR-0005).

#### BC-602: `client.list_teams(orgId)` GETs `/gateway/api/public/teams/v1/org/<orgId>/teams`
**Confidence**: HIGH
**Sources**: `tests/team_commands.rs:28-46`
**Behavior**: Returns `Vec<TeamEntry { team_id, display_name }>`, cursor-paginated.

#### BC-603: `team list` 5xx → exit 1 + `API error (500)` + no panic
**Confidence**: HIGH
**Sources**: `tests/team_commands.rs:62-106`

#### BC-604: `team list` 401 → exit 2 + reauth message
**Confidence**: HIGH
**Sources**: `tests/team_commands.rs:108-`

#### BC-605: `team list` cache-first (7d TTL); `--refresh` forces re-fetch
**Confidence**: MEDIUM
**Sources**: Pass 2 §2b.1, `src/cache.rs`
**Behavior**: On cache hit, no GraphQL/team-list HTTP. On miss or `--refresh`, GraphQL → teams.json write.

#### BC-606: `IssueFields::team_id` accepts string-UUID; rejects non-string id (object form)
**Confidence**: HIGH (Pass 2 inv-17)
**Sources**: `src/types/jira/issue.rs:101-131`, 9 tests in `src/types/jira/issue.rs::tests` + `tests/team_object_shape.rs` (243 LOC)

---

### 2.8 Users

#### BC-701: `user search Q` GETs `/rest/api/3/user/search?query=Q`
**Confidence**: HIGH
**Sources**: `tests/user_commands.rs`, `tests/all_flag_behavior.rs:155-208`

#### BC-702: `user search --all` server-paginates: advances `startAt` by REQUESTED `maxResults`, NOT by returned count
**Confidence**: HIGH (regression-pinned)
**Sources**: `tests/all_flag_behavior.rs:155-208`
**Behavior**: Page 1 (startAt=0) returns short page of 35 users; page 2 (startAt=100, advanced by requested maxResults=100, not by 35) is empty → terminates.

#### BC-703: `user search` default cap = DEFAULT_LIMIT (30)
**Confidence**: HIGH
**Sources**: `tests/all_flag_behavior.rs:212-251`

#### BC-704: `user list --project P` calls `/rest/api/3/user/assignable/multiProjectSearch?projectKeys=P`
**Confidence**: HIGH
**Sources**: `tests/all_flag_behavior.rs:260-`

#### BC-705: `user list` (default) uses single-call legacy path (no startAt/maxResults params)
**Confidence**: HIGH
**Sources**: `tests/all_flag_behavior.rs:271-275` (asserts `query_param_is_missing("startAt")`)

#### BC-706: Duplicate display names + `--no-input` errors with email + accountId disambiguation in stderr
**Confidence**: HIGH
**Sources**: `tests/duplicate_user_disambiguation.rs:21-69` (issue list), 71-132 (issue assign), 178-232 (issue create)
**Behavior**: Two users with same `displayName` → exit non-zero. stderr contains both emails + both accountIds + the duplicate name.

#### BC-707: Duplicate user without email → falls back to accountId in stderr disambiguation
**Confidence**: HIGH
**Sources**: `tests/duplicate_user_disambiguation.rs:134-176`

#### BC-708: Disambiguation excludes non-duplicate matches
**Confidence**: HIGH
**Sources**: `tests/duplicate_user_disambiguation.rs:235-275`
**Behavior**: Three users incl. "John Smith" x2 + "John Smithson". Disambiguation lists only the two duplicates; "Smithson" is not in stderr.

#### BC-709: `user view <accountId>` GETs `/rest/api/3/user?accountId=<a>`
**Confidence**: HIGH
**Sources**: `tests/user_commands.rs`, `src/api/jira/users.rs`

---

### 2.9 Projects & Queues

#### BC-801: `project_exists(key)` returns true on 200, false on 404
**Confidence**: HIGH
**Sources**: `tests/input_validation.rs:9-42`

#### BC-802: `get_project_statuses(key)` returns 404 → `JrError::ApiError{ status: 404, ... }`
**Confidence**: HIGH
**Sources**: `tests/input_validation.rs:233-253`

#### BC-803: `get_all_statuses()` returns vec of status names from `/rest/api/3/status`
**Confidence**: HIGH
**Sources**: `tests/input_validation.rs:45-64`

#### BC-804: `get_or_fetch_project_meta(client, key)` caches by project key with 7d TTL; on miss fetches project + lists service desks
**Confidence**: HIGH
**Sources**: `tests/project_meta.rs:24-67`
**Behavior**: Service-desk project → meta has `service_desk_id = Some("15")`. Software project → `None`.

#### BC-805: `require_service_desk(client, key)` errors with "Jira Software project" + "Queue commands require" message for non-JSM project
**Confidence**: HIGH
**Sources**: `tests/project_meta.rs:99-126`
**Behavior**: Used by `queue list/view` to fail-fast on software projects.

#### BC-806: `queue list/view` discovers servicedesk via cached project meta; fall back to fresh fetch
**Confidence**: MEDIUM
**Sources**: `src/cli/queue.rs::tests` (11 tests), Pass 2 §2b.1

#### BC-807: `queue view <name>` partial-matches queue name; `--id ID` overrides
**Confidence**: MEDIUM
**Sources**: `src/cli/queue.rs::tests`

#### BC-808: `project list` clap conflict on `--all` + `--limit`
**Confidence**: HIGH
**Sources**: `tests/cli_smoke.rs:290-297`

---

### 2.10 Configuration

#### BC-901: Legacy `[instance]/[fields]` blocks migrate to `[profiles.default]` on first load
**Confidence**: HIGH
**Sources**: `tests/migration_legacy.rs:93-143`
**Behavior**: After load, `config.global.profiles["default"]` carries url, cloud_id, team_field_id, story_points_field_id. On-disk file no longer contains `[instance]` or `[fields]` headers.

#### BC-902: Migration is idempotent (second load doesn't modify file)
**Confidence**: HIGH
**Sources**: `tests/migration_legacy.rs:145-172`
**Behavior**: `after_first == after_second` (byte equality).

#### BC-903: Migration write-back uses file-only baseline (no env overlay)
**Confidence**: MEDIUM (Pass 2 inv-21)
**Sources**: `src/config.rs:240-264`
**Behavior**: Transient `JR_*` env vars don't bleed into `config.toml`.

#### BC-904: `validate_profile_name` accepts `[A-Za-z0-9_-]{1,64}`; rejects `CON`/`NUL`/`AUX`/`PRN`/`COM1-9`/`LPT1-9`/empty/>64/colons/slashes/dots
**Confidence**: HIGH
**Sources**: Pass 2 inv-8, `src/config.rs:113-140` + 37 inline tests
**Behavior**: Rejected names exit 64 at three boundaries: CLI flag, TOML key parse, resolved active name.

#### BC-905: `JR_PROFILE` env overrides config `default_profile`
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:142-186`

#### BC-906: `Config::load_with(Some("X"))` — if X not in `[profiles]` and load is strict → exit 64 with `unknown profile: X`
**Confidence**: HIGH
**Sources**: Pass 2 §2b.6 Flow 2; `src/config.rs:295-310`

#### BC-907: `Config::load_lenient_with(...)` skips active-profile existence check (used by login flow)
**Confidence**: HIGH
**Sources**: `tests/auth_profiles.rs:241-332` (regression docstrings reference round-4 + round-5)

#### BC-908: Default `[defaults] output = "table"`
**Confidence**: HIGH
**Sources**: `src/config.rs:63-74`

#### BC-909: `JR_BASE_URL` env completely overrides profile URL (test/power-user only)
**Confidence**: HIGH (Pass 2 inv-25)
**Sources**: Used by every integration test; `src/config.rs:351-353`, `src/api/client.rs:37-65`

#### BC-910: `JR_DEFAULTS_OUTPUT` env overrides default output format
**Confidence**: MEDIUM
**Sources**: `tests/auth_profiles.rs:25` (scrubbed list); `src/config.rs::DefaultsConfig`

#### BC-911: ProjectConfig walks up cwd looking for `.jr.toml`
**Confidence**: HIGH
**Sources**: Pass 2 §2b.1; tests in `tests/issue_list_errors.rs` set `.jr.toml` in tempdir

---

### 2.11 Cache

#### BC-1001: `read_cache<T>` returns `Ok(None)` for missing file
**Confidence**: HIGH
**Sources**: `src/cache.rs:14-34` + 27 unit tests
**Behavior**: `NotFound` → `Ok(None)`, never `Err`.

#### BC-1002: `read_cache<T>` returns `Ok(None)` + stderr warning for parse-failed file
**Confidence**: HIGH (Pass 2 inv-9 + integration test)
**Sources**: `src/cache.rs:23-26`, `tests/issue_view_errors.rs:142-206` (truncated `teams.json` covers integration)
**Behavior**: Stderr warning text: `warning: cache file <name> unreadable (<err>); will refetch` (per Pass 2 §2b.4).

#### BC-1003: `read_cache<T>` returns `Ok(None)` for expired (>7d) entry
**Confidence**: HIGH
**Sources**: `src/cache.rs:14-34`, unit tests in `cache.rs::tests`
**Behavior**: `Expiring::fetched_at()` + 7-day comparison. No deletion of stale file.

#### BC-1004: Per-profile cache directory: `~/.cache/jr/v1/<profile>/`
**Confidence**: HIGH
**Sources**: `src/cache.rs:7, 30, 76-78`
**Behavior**: Versioned root `v1/` allows future schema-bump cleanup.

#### BC-1005: `clear_profile_cache(name)` is no-op when directory does not exist
**Confidence**: HIGH (Pass 2 inv-10; unit test)
**Sources**: `src/cache.rs:82-88`

#### BC-1006: `cmdb_fields.json` stores `(id, name)` tuples; old ID-only format → cache miss (graceful)
**Confidence**: HIGH
**Sources**: `src/cache.rs:237-247`, CLAUDE.md gotcha, `cache.rs::tests`

#### BC-1007: `ProjectMeta` map cache `project_meta.json` keyed by project key with per-entry TTL
**Confidence**: HIGH
**Sources**: `src/cache.rs:105-143`, `tests/project_meta.rs`

#### BC-1008: `ResolutionsCache` drops resolutions without an `id` on write + stderr warning
**Confidence**: HIGH (Pass 2 §2b.2 #10 / §2b.4)
**Sources**: `src/cli/issue/workflow.rs:117-133`
**Behavior**: stderr line: `warning: N resolution(s) lacked an id and were not cached`.

#### BC-1009: Cache reader/writer signature requires `profile: &str` (multi-profile boundary)
**Confidence**: MEDIUM
**Sources**: CLAUDE.md gotcha, `src/cache.rs::*` per-fn signatures
**Behavior**: Cross-profile leakage is a correctness bug; sandbox vs prod custom-field IDs may differ.

#### BC-1010: `WorkspaceCache` is whole-file (`workspace.json`); `ObjectTypeAttrCache` is map-cache (`object_type_attrs.json` keyed by object-type id)
**Confidence**: HIGH
**Sources**: `src/cache.rs:175-185, 264-282`

---

### 2.12 Output formatting

#### BC-1101: `--output table` uses comfy-table renderer; `--output json` emits structured JSON
**Confidence**: HIGH
**Sources**: `src/output.rs::tests` + many integration tests using `--output json`
**Behavior**: Default is `table`. Test pattern: assert `serde_json::from_str(&stdout)` parses.

#### BC-1102: `--no-color` and `NO_COLOR` env disable ANSI escape sequences
**Confidence**: HIGH (Pass 2 inv #6)
**Sources**: `src/main.rs:13-15` + colored crate

#### BC-1103: `--no-input` auto-enables when stdin is not a TTY
**Confidence**: HIGH (Pass 2 inv-25)
**Sources**: `src/main.rs:18-23`
**Behavior**: `IsTerminal` check; auto-set on pipes / AI agents / scripts.

#### BC-1104: ADF text→ADF emits `{type: "doc", version: 1, content: [{type:"paragraph", content:[{type:"text", text:"..."}]}]}`
**Confidence**: HIGH
**Sources**: `src/adf.rs::tests` (69 unit tests)

#### BC-1105: ADF markdown→ADF round-trip
**Confidence**: HIGH (69 ADF tests cover headings, lists, code, links)
**Sources**: `src/adf.rs::tests`

#### BC-1106: ADF→text rendering preserves structure for tables, code, headings
**Confidence**: HIGH
**Sources**: `src/adf.rs::tests`

#### BC-1107: `format_duration(seconds)` collapses to `30m` / `2h` / `1h30m` (never weeks/days)
**Confidence**: HIGH
**Sources**: `src/duration.rs:52-60`

#### BC-1108: `--output json` on write ops returns `{"key": "FOO-123"}` (or `{key, status, transitioned}`, `{key, assignee, changed}`, `{id, self}`)
**Confidence**: HIGH
**Sources**: `src/cli/issue/json_output.rs::tests` (11 tests), `tests/cli_handler.rs:81-90, 130-132`

#### BC-1109: Comment-date format pinning (verbose-gated parse-failure logging once-per-process)
**Confidence**: MEDIUM
**Sources**: `src/cli/issue/format.rs::tests`, `src/observability.rs::tests`

#### BC-1110: Issue list with truncation prints `Showing N results` to stderr (NOT stdout)
**Confidence**: HIGH
**Sources**: `tests/sprint_commands.rs:97-100`

#### BC-1111: `--all` does NOT print the `Showing` hint
**Confidence**: HIGH
**Sources**: `tests/sprint_commands.rs:175-179`

---

### 2.13 Error handling (JrError variants + exit codes + remediation)

#### BC-1201: `extract_error_message` precedence: `errorMessages[]` → `errors{}` → `message` → `errorMessage` → raw body → `<empty response body>`
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:257-342`
**Behavior**: 6-level chain pinned by 12 unit tests including:
- `errorMessages` array → joined by `; ` (`Issue does not exist; Or you lack permission`)
- `message` field → as-is
- `errorMessages` preferred over `message`
- Empty `errorMessages` falls back to `errors{}` object joined as `field: value` (sorted alphabetically — multiple fields produce stable output)
- Nested `errors.field.messages[]` serialized as JSON
- Singular `errorMessage` field
- Plain text body returned as-is
- Empty body → `<empty response body>` literal

#### BC-1202: `errors` object with mixed value types (string + array) serializes both
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:309-321`
**Behavior**: `{summary: "is required", components: ["a","b"]}` produces stderr containing both `summary: is required` and `components: ["a","b"]`.

#### BC-1203: Empty `errors` object falls through to raw body
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:294-300`

#### BC-1204: `JrError::exit_code()` mapping: NotAuthenticated/InsufficientScope=2, ConfigError=78, UserError=64, Interrupted=130, others=1
**Confidence**: HIGH
**Sources**: `src/error.rs:51-62` + 8 inline tests

#### BC-1205: All API errors emit human-readable message; never leak `panic` to stderr
**Confidence**: HIGH (16+ tests across `tests/*_errors.rs` files assert `!stderr.contains("panic")`)
**Sources**: `tests/issue_list_errors.rs`, `tests/issue_view_errors.rs`, `tests/comments.rs`, `tests/worklog_commands.rs`, `tests/team_commands.rs`, `tests/assets_errors.rs`, `tests/auth_refresh.rs`, `tests/auth_login_config_errors.rs`

#### BC-1206: Network drop → `Could not reach <host>; check your connection` exit 1
**Confidence**: HIGH
**Sources**: `tests/issue_list_errors.rs:320-360`, `tests/issue_view_errors.rs:102-134`, `tests/assets_errors.rs:115-153`
**Behavior**: Privileged port 1 connect-refused. Triggers `JrError::NetworkError(host)`.

#### BC-1207: 401 → `Not authenticated` + `jr auth login` suggestion exit 2 (universal across all subcommands)
**Confidence**: HIGH (replicated in 6+ test files)
**Sources**: `tests/issue_list_errors.rs`, `tests/issue_view_errors.rs`, `tests/comments.rs`, `tests/worklog_commands.rs`, `tests/team_commands.rs`, `tests/assets_errors.rs`

#### BC-1208: `--output json` error shape: `{"error": "<message>", "code": <exit>}` to stderr
**Confidence**: MEDIUM (Pass 2 §2b.4; main.rs:34-49)
**Sources**: `src/main.rs:34-49`

#### BC-1209: Ctrl+C exits 130 with `Interrupted` literal
**Confidence**: MEDIUM (Pass 2 §2b.4)
**Sources**: `src/main.rs:264`

#### BC-1210: 5xx → `API error (<status>)` + raw body message + exit 1
**Confidence**: HIGH
**Sources**: All `*_errors.rs` files; tests assert `stderr.contains("API error (500)")`

#### BC-1211: 400 with field-specific Jira error → stderr formatted as `field: message`
**Confidence**: HIGH (BC-1201/1202 corollaries; `tests/issue_resolution.rs` uses real Atlassian shape)
**Sources**: `tests/issue_resolution.rs:124-158`

#### BC-1212: Error messages must suggest a next step ("Always suggest what to do next" — CLAUDE.md convention)
**Confidence**: HIGH (multiple integration tests assert remediation strings: `jr auth login`, `--jql`, `--resolution`, `jr issue resolutions`, `jr team list --refresh`, `board_id`, `check your connection`)
**Sources**: `tests/issue_list_errors.rs`, `tests/issue_resolution.rs`, `tests/auth_refresh.rs`, `tests/issue_view_errors.rs`

#### BC-1213: Internal "should never happen" errors prefix message with `Internal error:`
**Confidence**: MEDIUM (Pass 2 §2a.2; `error.rs:30-36` doc)
**Sources**: `src/error.rs::tests`

#### BC-1214: 401 + `Insufficient token scope` produces stderr containing GitHub issue link `github.com/Zious11/jira-cli/issues/185`
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:140-143`

---

### 2.14 Build-time concerns

#### BC-1301: `build.rs` reads `JR_BUILD_OAUTH_CLIENT_ID` + `_SECRET` env vars
**Confidence**: HIGH (Pass 0 §4)
**Sources**: `build.rs` 125 LOC

#### BC-1302: Unix builds use `/dev/urandom` for the 32-byte XOR key; Windows uses inline `BCryptGenRandom` FFI shim
**Confidence**: HIGH
**Sources**: `build.rs` (per Pass 0 §4)

#### BC-1303: `compile_error!` for non-unix/non-windows targets (e.g., wasm)
**Confidence**: HIGH
**Sources**: `build.rs`

#### BC-1304: When env vars unset, build emits `EMBEDDED_*` constants as `None`
**Confidence**: HIGH
**Sources**: `build.rs` + `src/api/auth_embedded.rs::tests`
**Behavior**: BYO/prompt path proceeds at runtime.

#### BC-1305: Embedded callback URL is literal `http://127.0.0.1:53682/callback` (port 53682, IPv4 literal)
**Confidence**: HIGH (CLAUDE.md gotcha + ADR-0006)
**Sources**: `src/api/auth.rs:374-477` (per Pass 2 inv-13), CLAUDE.md
**Behavior**: Avoid `localhost`-IPv6 macOS Chrome resolver pitfall.

#### BC-1306: BYO OAuth credentials use dynamic port `:0` (historical behavior)
**Confidence**: MEDIUM
**Sources**: `src/api/auth.rs::login_oauth` per Pass 2 inv-13

#### BC-1307: Integration test for embedded-OAuth flow is `#[ignore]`-gated and stubbed (deferred)
**Confidence**: HIGH
**Sources**: `tests/oauth_embedded_login.rs`

---

### 2.15 Runtime concerns

#### BC-1401: `client.send_raw(request)` retries 429 up to MAX_RETRIES=3, then returns 429 to caller (does NOT raise)
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:344-444`
**Behavior**:
- 200 returns response (200)
- 404 returns response (404) — explicitly NOT converted to error (raw passthrough for `jr api`)
- 429+200 retries → caller sees 200
- 429×4 (initial + 3 retries) → caller sees 429 (no error)
**Edge**: `expect(4)` mock count proves exactly initial+3.

#### BC-1402: Non-`send_raw` requests retry 429 transparently and return parsed response on success
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:42-69`

#### BC-1403: `Retry-After` header parsed (integer seconds; per Pass 2 §2b.4 line)
**Confidence**: MEDIUM
**Sources**: `src/api/rate_limit.rs::tests` (2 tests cover int + http-date forms)

#### BC-1404: 429-exhausted warning to stderr (always, not verbose-gated)
**Confidence**: MEDIUM (Pass 2 §2b.4)
**Sources**: `src/api/client.rs` (after retry loop)
**Behavior**: stderr line `warning: rate limited by Jira — gave up after 3 retries.`

#### BC-1405: Verbose request logging emits `[verbose] METHOD URL` + body
**Confidence**: MEDIUM (Pass 2 §2b.4)
**Sources**: `src/api/client.rs:197-204`

#### BC-1406: Cursor pagination via `nextPageToken` (JQL search)
**Confidence**: HIGH
**Sources**: `src/api/pagination.rs::tests` (14 tests), `tests/issue_commands.rs`

#### BC-1407: Offset pagination via `startAt`/`maxResults` (most other endpoints)
**Confidence**: HIGH
**Sources**: `src/api/pagination.rs::tests`, `tests/comments.rs:104-158`

#### BC-1408: ServiceDeskPage envelope: `{values, isLastPage, size, start, limit}`
**Confidence**: HIGH
**Sources**: `src/api/pagination.rs::tests`, `tests/cmdb_fields.rs:155-160`

#### BC-1409: AssetsPage `is_last` accepts both bool and string via custom deserializer
**Confidence**: HIGH (Pass 2 §2a.3)
**Sources**: `src/api/pagination.rs::tests`

#### BC-1410: Auth header injected on every API call (`Authorization: <auth_header>`)
**Confidence**: HIGH
**Sources**: `tests/api_client.rs:14-40` (asserts header value)

#### BC-1411: Tokio runtime + tokio::select! against ctrl_c (graceful 130 exit)
**Confidence**: MEDIUM (Pass 0 §4)
**Sources**: `src/main.rs`

---

## 3. Cross-reference with Pass 2 invariants

| Pass 2 Invariant | Confidence | Mapped BC(s) | Status |
|---|---|---|---|
| INV-1: idempotent `issue move` | HIGH | BC-207 + tests in `cli/issue/workflow.rs::tests` | COVERED |
| INV-2: partial_match Ambiguous on single substring | HIGH | BC-105 + `partial_match.rs::tests` | COVERED |
| INV-3: jql escape (no unescaped quote) | HIGH | property test in `jql.rs::tests` | COVERED |
| INV-4: build_asset_clause uses NAME + capital `Key` | HIGH | BC-306, BC-307 | COVERED |
| INV-5: validate_duration rejects combined units `4w2d`, reversed `d7` | HIGH | `jql.rs::tests:228-235` | COVERED |
| INV-6: parse_duration accepts combined `1w2d3h30m` | HIGH | BC-505 | COVERED |
| INV-7: profile resolution precedence | HIGH | BC-007 | COVERED |
| INV-8: validate_profile_name rejects invalid | HIGH | BC-904 + 37 config tests | COVERED |
| INV-9: cache reads return Ok(None) for missing/expired/corrupt | HIGH | BC-1001/1002/1003 + BC-115 (integration) | COVERED |
| INV-10: clear_profile_cache no-op for nonexistent dir | HIGH | BC-1005 (unit only — **NO INTEGRATION TEST**) | UNTESTED-INTEGRATION |
| INV-11: per-profile keychain key namespacing | MEDIUM | BC-009/010/013/014/023 (most under `#[ignore]`) | PARTIALLY-TESTED |
| INV-12: non-default profile never inherits legacy | MEDIUM | BC-023 (unit only — by absence of `default` literal) | PARTIALLY-TESTED |
| INV-13: embedded_oauth_app_present without decode | HIGH | BC-021 | COVERED |
| INV-14: EmbeddedOAuthApp Debug redacts | HIGH | BC-019 | COVERED |
| INV-15: empty XOR inputs → None | HIGH | BC-020 | COVERED |
| INV-16: story_points None for non-numeric | HIGH | BC-124 | COVERED |
| INV-17: team_id accepts string + object, rejects non-string id | HIGH | BC-606 + 9 inline tests + `tests/team_object_shape.rs` | COVERED |
| INV-18: Resolution dual-shape | HIGH | `types/jira/issue.rs::tests:600-624` | COVERED (in unit tests, not enumerated as separate BC) |
| INV-19: LinkedAsset display id-fallback | HIGH | BC-305 | COVERED |
| INV-20: --open ticket filter colorName != green | MEDIUM | BC-314 (per `cli/assets.rs:303-321`) | COVERED-UNIT |
| INV-21: --open issue list `statusCategory != Done` | MEDIUM | BC-102, BC-103 (JQL composition) | INDIRECT (no test asserts the literal JQL fragment) |
| INV-22: MAX_SPRINT_ISSUES=50 cap | MEDIUM | BC-405 | UNTESTED-INTEGRATION (unit only; no integration assert) |
| INV-23: scrum-only sprint commands | MEDIUM | BC-402 | COVERED-UNIT |
| INV-24: date validators run before HTTP | MEDIUM | clap conflict tests cover the parse phase | PARTIALLY (no test asserts "no HTTP fired" except via clap-level rejection) |
| INV-25: --no-input auto-set when not TTY | MEDIUM | BC-1103 (impl only) | UNTESTED-DIRECT (asserted indirectly via test invocations) |

### 3.5 Untested invariants — gaps for Phase 0/1 holdout coverage planning

**Critical gaps** (no integration test pins them):
1. **INV-10** — `cache::clear_profile_cache(name)` no-op for nonexistent directory has unit test but no integration test asserting it during `auth remove` flow.
2. **INV-22** — `MAX_SPRINT_ISSUES=50` cap on `sprint add`/`sprint remove` has unit tests but no integration test passing 51+ keys to the CLI.
3. **INV-21** — `--open` JQL fragment literal `statusCategory != Done` is asserted only via composition tests; no integration test issues the wiremock body match for it.
4. **INV-24** — date validators running pre-HTTP is asserted only by clap-level rejection; no test passes a syntactically valid command with a bad date and counts HTTP requests.
5. **INV-25** — `--no-input` TTY autoset behavior is hard to test from `assert_cmd` (always non-TTY). No direct test.
6. **INV-12** — non-default profiles never inheriting legacy keychain keys lacks a positive-side keyring integration test (it's asserted by absence, which is brittle).
7. **INV-11** — most multi-profile keychain BCs are `#[ignore]`-gated by `JR_RUN_KEYRING_TESTS=1` and don't fire by default in CI.

---

## 4. Holdout candidates (TOP 20 for Phase 4 evaluation)

These are user-observable scenarios an evaluator (different model, fresh context) can verify against a binary. Setup uses XDG dirs + wiremock-style fixture servers; expected output is precise.

### Holdout candidate H-001: `auth status` first-run gives helpful guidance, not error
**Setup**: empty `XDG_CONFIG_HOME`. No env vars.
**Action**: `jr auth status`
**Expected**: exit 0; stderr contains `No profiles configured`.
**Why hidden**: Setup scripts probe with this command. A regression that errors here breaks every onboarding flow.

### Holdout candidate H-002: `auth list --output json` returns `[]` for fresh install
**Setup**: empty `XDG_CONFIG_HOME`.
**Action**: `jr auth list --output json`
**Expected**: exit 0; stdout = `[]`.
**Why hidden**: JSON shape is the parsing contract for orchestrators.

### Holdout candidate H-003: profile precedence flag > env > config > "default"
**Setup**: config.toml with three profiles `from-config / from-env / from-flag` + `default_profile = "from-config"`. Set `JR_PROFILE=from-env`.
**Action**: `jr --profile from-flag auth list --output json`
**Expected**: exit 0; exactly one element with `"active": true` and `"name": "from-flag"`.
**Why hidden**: Multi-source precedence is invisible from any single test; tests must vary all three layers.

### Holdout candidate H-004: `auth refresh --no-input` against unconfigured profile fails clearly
**Setup**: empty config. Scrub all `JR_INSTANCE_*`. Set `JR_SERVICE_NAME=jr-jira-cli-test` to isolate keychain.
**Action**: `jr --no-input auth refresh`
**Expected**: exit 64; stderr contains `no URL configured`, `jr auth login`, `--url`. Stderr does NOT contain `panic`.
**Why hidden**: Pre-fix behavior was to clear creds then prompt for email — destructive misleading recovery. Pin against regression.

### Holdout candidate H-005: malformed config TOML errors with exit 78 and does NOT overwrite the file
**Setup**: write malformed TOML (`[unclosed\nbad = \n`) at `XDG_CONFIG_HOME/jr/config.toml`. Capture file bytes.
**Action**: `jr auth login --oauth --client-id X --client-secret Y --no-input`
**Expected**: exit 78; stderr contains `toml` or `parse`; file bytes are unchanged.
**Why hidden**: Pre-fix bug silently overwrote with defaults — destroyed user settings. The non-overwrite invariant is the most important property here.

### Holdout candidate H-006: `issue move FOO-1 "In Progress"` is idempotent when already in target
**Setup**: wiremock returns `GET /issue/FOO-1` with `status.name = "In Progress"`. Mock POST transitions with `expect(0)`.
**Action**: `jr issue move FOO-1 "In Progress" --output json`
**Expected**: exit 0; stdout JSON has `"transitioned": false`. POST mock not invoked.
**Why hidden**: Idempotency is invisible in success-only tests. Verifying by mock count is the only way.

### Holdout candidate H-007: `issue move FOO-1 Done` against state requiring resolution surfaces `--resolution` hint
**Setup**: transitions list has Done; current status In Progress; POST transitions returns 400 `{errors: {resolution: "Field 'resolution' is required"}}`.
**Action**: `jr --no-input issue move FOO-1 Done`
**Expected**: exit non-zero; stderr contains both `--resolution` AND `jr issue resolutions`.
**Why hidden**: Atlassian's raw error wording is unfriendly. The remediation rewrite is the user-value.

### Holdout candidate H-008: `issue list --status prog` (single-substring) errors without firing JQL search
**Setup**: project statuses `[To Do, In Progress, Done]`. Wiremock POST `/search/jql` `expect(0)`.
**Action**: `jr --no-input issue list --status prog` (in `.jr.toml::project="PROJ"` cwd)
**Expected**: exit 64; stderr `Ambiguous status` + `In Progress`. JQL search mock not called.
**Why hidden**: Pin from issue #193 — strict-matching rollout. Behavior boundary is invisible without verifying mock count.

### Holdout candidate H-009: `issue list` with corrupt `teams.json` is non-fatal; UUID + cache hint shown
**Setup**: write `{"teams": [` (truncated) to `~/.cache/jr/v1/default/teams.json`. Mock issue with team UUID `<u>`.
**Action**: `jr issue view PROJ-1`
**Expected**: exit 0; stdout contains `<u>` AND `name not cached` AND `jr team list --refresh`. stderr no panic.
**Why hidden**: Format-change graceful degradation — invisible without manually corrupting the cache file.

### Holdout candidate H-010: `--all` issue list returns more than 30; default truncates with hint
**Setup**: wiremock returns 35 issues in one cursor page. Body match `maxResults=50` for `--all`, `maxResults=30` for default. With default, mock approximate-count returns 35.
**Action**: `jr issue list --jql "project = X" --all --output json` then `jr issue list --jql "project = X" --output json`
**Expected**: first → JSON array length 35. Second → JSON array length 30 (and stderr contains `Showing 30 results` or `~`).
**Why hidden**: Pagination cap is regulated by request body shape; invisible from output count alone (could be coincidence).

### Holdout candidate H-011: legacy `[instance]` config migrates to `[profiles.default]` on first load (idempotent)
**Setup**: write legacy `[instance] / [fields] / [defaults]` config to disk.
**Action**: load config twice (e.g., `jr auth list` twice, or unit-test `Config::load` directly).
**Expected**: After first load, on-disk file has `[profiles.default]`, no `[instance]`/`[fields]`. After second load, file is byte-identical to after first.
**Why hidden**: Migration is one-shot and silent; idempotency is invisible without bytewise comparison.

### Holdout candidate H-012: 401 with `scope does not match` body produces InsufficientScope error with workaround docs
**Setup**: wiremock POST `/rest/api/3/issue` returns 401 body `{message: "Unauthorized; scope does not match"}`.
**Action**: invoke any `client.post(...)` (e.g., `issue create`).
**Expected**: exit 2; stderr contains `Insufficient token scope`, the raw gateway message, `write:jira-work`, `OAuth 2.0`, `github.com/Zious11/jira-cli/issues/185`.
**Why hidden**: A future tightening of the substring match would silently break this. The workaround docs are a key UX contract.

### Holdout candidate H-013: 429 retry — `send_raw` returns 429 to caller after MAX_RETRIES=3
**Setup**: wiremock GET responds 429 with `Retry-After: 0` for 4 calls (`expect(4)`).
**Action**: `client.send_raw(GET /myself)`.
**Expected**: response status = 429 (NOT an error). exactly 4 calls fired.
**Why hidden**: Retry semantics for `jr api` raw passthrough must NOT raise — caller wants the literal status. Easy to break by adding error conversion.

### Holdout candidate H-014: `assign --to <name>` against duplicate display names + `--no-input` errors with email/accountId disambiguation
**Setup**: assignable user search returns two users with same `displayName` `"John Smith"`.
**Action**: `jr issue assign FOO-1 --to "John Smith" --no-input`
**Expected**: exit non-zero; stderr contains both emails (`john1@acme.com`, `john2@other.org`) AND both accountIds (`acc-john-1`, `acc-john-2`).
**Why hidden**: AI-agent ergonomic — needs accountId to retry. Test pin from `tests/duplicate_user_disambiguation.rs`.

### Holdout candidate H-015: clap mutual-exclusion: `--all` and `--limit` together fails fast
**Setup**: none.
**Action**: `jr issue list --all --limit 10`
**Expected**: exit non-zero; stderr contains `cannot be used with`.
**Why hidden**: Many subcommands have similar conflicts; checking one regression-detects refactor mistakes.

### Holdout candidate H-016: `auth remove <active>` is rejected
**Setup**: config with `default_profile = "default"` and `[profiles.default]` set.
**Action**: `jr --no-input auth remove default`
**Expected**: exit 64; stderr contains `cannot remove active`. Config file unchanged.
**Why hidden**: Destructive operation safety; failure here would break invariants others depend on.

### Holdout candidate H-017: AQL clause uses field NAME + capital `Key`
**Setup**: caller passes `cmdb_fields = [("customfield_10191", "Client")]`, asset_key = `CUST-5`.
**Action**: invoke `jql::build_asset_clause("CUST-5", &fields)`.
**Expected**: exact string `"Client" IN aqlFunction("Key = \"CUST-5\"")`. Note: `Client` not `customfield_10191`; `Key` not `objectKey`.
**Why hidden**: Two CLAUDE.md gotchas conflated in one helper. Regression on either is silent (Jira rejects at runtime, not compile time).

### Holdout candidate H-018: `parse_duration("1w2d3h30m", 8, 5)` accepts combined units (vs jql.validate_duration which rejects)
**Setup**: none.
**Action**: invoke both `duration::parse_duration("1w2d3h30m", 8, 5)` and `jql::validate_duration("4w2d")`.
**Expected**: parse_duration → Ok(seconds) where seconds = 1*5*8*3600 + 2*8*3600 + 3*3600 + 30*60. validate_duration → Err.
**Why hidden**: Two parsers with overlapping syntax but DIFFERENT acceptance — easy to confuse.

### Holdout candidate H-019: profile name `foo:bar` is rejected at three boundaries (CLI flag, TOML key, resolved active)
**Setup**: try (a) `--profile foo:bar` flag; (b) write config with `[profiles."foo:bar"]`; (c) set `JR_PROFILE=foo:bar` against existing profile.
**Action**: any non-init `jr` command for each variant.
**Expected**: each → exit 64.
**Why hidden**: Validates the security boundary protecting cache paths and keychain-key namespaces.

### Holdout candidate H-020: `--output json` error shape is structured `{"error", "code"}` to stderr
**Setup**: any command that errors (e.g., `jr --output json auth switch ghost` against config without `[profiles.ghost]`).
**Action**: above.
**Expected**: exit 64; stderr is parseable JSON with keys `error` (string) and `code` (number 64).
**Why hidden**: Programmatic consumers depend on this shape; it's not asserted by most unit tests.

---

## State Checkpoint

```yaml
pass: 3
status: complete
test_files_inventoried: 36          # tests/*.rs (excluding common/)
unit_test_modules_inventoried: 43   # src/**/*.rs with #[cfg(test)] mod tests
bcs_drafted_high: 134
bcs_drafted_medium: 45
bcs_drafted_low: 9
bcs_drafted_total: 188
holdout_candidates: 20
untested_invariants: 7              # INV-10/11/12/21/22/24/25 from Pass 2
files_examined: 24
timestamp: 2026-05-04T02:30:00Z
next_pass: 4
inputs_consumed:
  - .factory/semport/jira-cli/jira-cli-pass-0-inventory.md
  - .factory/semport/jira-cli/jira-cli-pass-2-domain-model.md
  - .reference/jira-cli/CLAUDE.md (treated as hypothesis)
  - .reference/jira-cli/tests/cli_handler.rs (sampled)
  - .reference/jira-cli/tests/issue_commands.rs (sampled)
  - .reference/jira-cli/tests/auth_profiles.rs (full)
  - .reference/jira-cli/tests/api_client.rs (full)
  - .reference/jira-cli/tests/issue_list_errors.rs (full)
  - .reference/jira-cli/tests/issue_view_errors.rs (full)
  - .reference/jira-cli/tests/migration_legacy.rs (full)
  - .reference/jira-cli/tests/sprint_commands.rs (sampled)
  - .reference/jira-cli/tests/auth_refresh.rs (full)
  - .reference/jira-cli/tests/auth_login_config_errors.rs (full)
  - .reference/jira-cli/tests/oauth_embedded_login.rs (full)
  - .reference/jira-cli/tests/all_flag_behavior.rs (sampled)
  - .reference/jira-cli/tests/cmdb_fields.rs (full)
  - .reference/jira-cli/tests/comments.rs (sampled)
  - .reference/jira-cli/tests/issue_resolution.rs (full)
  - .reference/jira-cli/tests/duplicate_user_disambiguation.rs (full)
  - .reference/jira-cli/tests/input_validation.rs (full)
  - .reference/jira-cli/tests/project_meta.rs (full)
  - .reference/jira-cli/tests/assets_errors.rs (full)
  - .reference/jira-cli/tests/cli_smoke.rs (full)
  - .reference/jira-cli/tests/board_commands.rs (sampled)
  - .reference/jira-cli/tests/worklog_commands.rs (sampled)
  - .reference/jira-cli/tests/team_commands.rs (sampled)
  - .reference/jira-cli/src/jql.rs (sampled)
  - .reference/jira-cli/src/duration.rs (sampled)
  - .reference/jira-cli/src/partial_match.rs (full)

deepening_round_required: true       # Phase B should fill in:
                                     # - LOW-confidence BCs (n=9)
                                     # - all 5 ADF unit-tested behaviors enumerated as BCs
                                     # - the 28 untested-by-integration invariants
                                     # - 38 changelog tests not yet enumerated
                                     # - per-changelog-field BCs
```
