# jr — Jira CLI

A Rust CLI tool for automating Jira Cloud workflows. Binary name: `jr`, package name: `jr`.

## Architecture

Single-crate thin client wrapping Jira REST API v3 and Agile REST API directly with reqwest. No generated client or intermediate abstraction layer.

```
src/
├── main.rs              # Entry point, tokio runtime, clap dispatch, Ctrl+C handling
├── cli/                 # Clap derive definitions + command handlers
│   ├── mod.rs           # CLI enums, global flags (--output, --project, --profile, --no-input, --no-color)
│   ├── issue/           # issue commands (split by operation theme)
│   │   ├── mod.rs       # dispatch + re-exports
│   │   ├── format.rs    # row formatting, headers, points display
│   │   ├── list.rs      # list + view + comments (read operations, unified JQL composition)
│   │   ├── view.rs      # cli/issue/view.rs — issue view handler, detailed single-issue rendering (~287 LOC)
│   │   ├── comments.rs  # cli/issue/comments.rs — comment list formatting and display (~61 LOC)
│   │   ├── create.rs    # create + edit (field-building)
│   │   ├── workflow.rs  # move + transitions + assign + comment + open
│   │   ├── links.rs     # link + unlink + link-types
│   │   ├── helpers.rs   # team/points resolution, user resolution, prompts
│   │   └── assets.rs    # linked assets (issue→asset lookup)
│   ├── assets.rs        # assets search/view/tickets/schemas/types/schema (search enrichment, schema discovery)
│   ├── board.rs         # board list/view
│   ├── sprint.rs        # sprint list/current/add/remove (scrum-only, errors on kanban)
│   ├── worklog.rs       # worklog add/list
│   ├── team.rs          # team list (with cache + lazy org discovery)
│   ├── user.rs          # user search/list/view (thin wrapper over api/jira/users.rs)
│   ├── auth.rs          # auth login/switch/list/status/refresh/logout/remove. Multi-profile aware via --profile.
│   ├── init.rs          # Interactive setup (prefetches org metadata + team cache + story points field)
│   ├── project.rs       # project fields (types, priorities, statuses, CMDB fields)
│   ├── queue.rs         # queue list/view (JSM service desks)
│   └── requesttype.rs   # requesttype list/fields (JSM request-type discovery + 7d cache)
├── api/
│   ├── client.rs        # JiraClient — HTTP methods, auth headers, rate limit retry, 429/401 handling
│   ├── auth.rs          # OAuth 2.0 flow + per-profile keychain layout (shared email/api-token/oauth_client_*; namespaced <profile>:oauth-access-token / <profile>:oauth-refresh-token); lazy migration of legacy flat OAuth keys for the "default" profile
│   ├── pagination.rs    # Offset-based (most endpoints) + cursor-based (JQL search)
│   ├── rate_limit.rs    # Retry-After parsing
│   ├── assets/          # Assets/CMDB API call implementations
│   │   ├── workspace.rs     # workspace ID discovery + cache
│   │   ├── linked.rs        # CMDB field discovery, asset extraction/enrichment (per-field + JSON)
│   │   ├── objects.rs       # AQL search, get object, resolve key
│   │   ├── schemas.rs       # api/assets/schemas.rs — schema discovery + object-type attributes (~44 LOC)
│   │   └── tickets.rs       # connected tickets
│   └── jira/            # Jira-specific API call implementations (one file per resource)
│       ├── issues.rs    # search (full + keys-only), get, create, edit, list comments
│       ├── boards.rs    # list boards, get board config
│       ├── sprints.rs   # list sprints, get sprint issues
│       ├── fields.rs    # list fields, story points + CMDB field discovery
│       ├── statuses.rs  # get all statuses (global, not project-scoped)
│       ├── links.rs     # create/delete issue links, list link types
│       ├── teams.rs     # org metadata (GraphQL), list teams
│       ├── worklogs.rs  # add/list worklogs
│       ├── projects.rs  # project details
│       └── users.rs     # current user, user search, assignable users, single-user lookup
│   ├── jsm/             # JSM-specific API call implementations
│   │   ├── servicedesks.rs  # list service desks, project meta orchestration
│   │   └── queues.rs        # list queues, get queue issues
├── types/assets/        # Serde structs for Assets API responses (AssetObject, ConnectedTicket, LinkedAsset, etc.)
├── types/jira/          # Serde structs for API responses (Issue, Board, Sprint, User, Team, etc.)
├── types/jsm/           # Serde structs for JSM API responses (ServiceDesk, Queue, etc.)
├── cache.rs             # Per-profile XDG cache (~/.cache/jr/v1/<profile>/) — team list, project meta, workspace ID, CMDB fields, object-type attrs, resolutions (all 7-day TTL). Versioned root (`v1/`) lets a future schema bump orphan stale files cleanly.
├── config.rs            # Global (~/.config/jr/config.toml) [profiles.<name>] + default_profile + per-project (.jr.toml), figment layering. Auto-migrates legacy [instance]/[fields] shape on first load. Active profile resolved at load via Config::load_with(cli_profile) (cli flag threaded through as a parameter, NOT an env-var seam) > JR_PROFILE env > default_profile field > "default".
├── output.rs            # Table (comfy-table) and JSON formatting
├── adf.rs               # Atlassian Document Format: text→ADF, markdown→ADF, ADF→text
├── duration.rs          # Worklog duration parser (2h, 1h30m, 1d, 1w)
├── observability.rs     # --verbose / --verbose-bodies flag helpers, eprintln! wrappers (~39 LOC)
├── lib.rs               # Crate root (re-exports for integration tests)
├── jql.rs               # JQL utilities: escaping, validation, asset clause builder
├── partial_match.rs     # Case-insensitive substring matching with disambiguation
└── error.rs             # JrError enum with exit codes (0/1/2/64/78/124/130)
```

Product-namespaced `api/jira/` and `types/jira/` so future Confluence/JSM/Assets support adds sibling directories.

## Known Size Deviations

- `cli/issue/list.rs`: 1,083 LOC post-split (target was ≤750 per `docs/specs/list-rs-split.md`; spec target not achieved but split was partial — `view.rs` and `comments.rs` already extracted). NFR-O-G: DOCUMENT-AS-IS-COMPLETE (S-3.08).

## Build & Test

```bash
cargo build                          # Build debug
cargo build --release                # Build optimized (LTO, strip, panic=abort)
cargo test                           # All tests (unit, integration, proptest, snapshots)
cargo test --lib                     # Unit tests only
cargo test --test '*'                # Integration tests only
cargo clippy -- -D warnings          # Lint (zero warnings policy)
cargo fmt --all -- --check           # Format check
cargo deny check                     # License + vulnerability audit
DIFF_FILE=$(mktemp -t pr.diff.XXXXXX) && trap 'rm -f "$DIFF_FILE"' EXIT && git diff origin/develop...HEAD > "$DIFF_FILE" && cargo mutants --in-diff "$DIFF_FILE" --jobs 4  # Mutation testing on PR diff scope (see docs/specs/cargo-mutants-policy.md)
```

## Conventions

- **Commits:** Conventional Commits format (`feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `test:`)
- **Branches:** `type/short-description` (e.g., `feat/issue-commands`, `fix/auth-flow`). Default branch is `develop`. Feature branches → PR to `develop` → PR to `main` for releases.
- **Protected branches:** `main` and `develop` require CI to pass and code owner approval on PRs. Admins can bypass.
- **Errors:** Always suggest what to do next. Map to exit codes via `JrError::exit_code()`
- **Output:** `--output json` returns structured JSON for both success and errors. Human text is default.
- **Non-interactive:** `--no-input` disables prompts (auto-enabled when stdin is not a TTY). Commands must have fully non-interactive flag equivalents.
- **Idempotent:** State-changing commands (move, assign) exit 0 if already in target state.
- **Tests:** TDD. Unit tests inline, integration tests in `tests/`. Property tests with proptest. Snapshot tests with insta.
- **No unsafe code** without explicit justification in a comment.
- **No lint suppression without refactoring.** If clippy warns (e.g., `too_many_arguments`), refactor to fix the root cause — don't add `#[allow]`. If refactoring is impractical, ask the user before suppressing and include a justification comment.
- **Default to fixing code, not tests.** When a test fails, assume the test is correct and fix the implementation using idiomatic Rust. Only modify a test when requirements have changed — not to accommodate non-idiomatic code or lint workarounds.
- **Test naming:** New tests use `test_<verb>_<subject>_<expected_outcome>` (e.g., `test_auth_switch_returns_json_ok`). Existing tests with no-prefix names are NOT renamed; this convention applies to new tests only. See `docs/specs/test-naming-convention.md`.
- `--dry-run` is implemented on `issue edit` (multi-key positional + `--jql`-resolved sets) with `--output json` support. Pre-PR2 NFR-O-C originally documented this as DOCUMENT-AS-IS-OUT-OF-SCOPE; superseded by issue #110 part 2.
- `jr version --output json` is not implemented (NFR-O-X: deferred to v2; consider for `release-notes` automation).
- `sprint list` table omits start/end dates (NFR-O-U: deferred UX pass v2; available in API response).
- `auth status --output json` covers single-profile JSON; multi-profile listing has no JSON path (NFR-O-N: deferred; planned alongside future `auth list --output json` extension).
- JSON output has no `_meta: {version: N}` envelope (NFR-O-P: deliberate for v0.5; consider for v2 to enable downstream-parser schema-drift detection).

### Output channels

CLI handlers follow one of five output-channel profiles:

1. **Pure** — stdout only (JSON or table data); no stderr output at all.
2. **Read-only** — stdout for data, stderr for hints/warnings (e.g., truncation notices, "showing N of M").
3. **Mixed** — stdout for success data, stderr for errors and hints; applies to most read commands.
4. **Symmetric** — stdout for `--output json`, stderr for human-readable errors in either mode; state-changing commands that also print a result use this profile.
5. **No-log facade** — state-changing commands that emit only a JSON result on stdout (e.g., `{"key": "FOO-123"}`); no progress or logging to stderr.

The distinction matters for scripting: pipe stdout for data, redirect stderr for diagnostics. Never write diagnostic text to stdout in profiles 1/2/3/5.

## Key Decisions

See `docs/adr/` for detailed rationale:
- ADR-0001: Thin client vs generated API client
- ADR-0002: OAuth 2.0 with embedded secret (superseded — see ADR-0006)
- ADR-0003: reqwest with rustls-tls
- ADR-0004: Per-feature specs, not a growing master spec
- ADR-0005: GraphQL hostNames for org discovery (team support)
- ADR-0006: Embedded `jr` OAuth app with compile-time XOR obfuscation (re-supersedes ADR-0002)
- ADR-0015: Proactive resolution enforcement on done-category transitions (`jr issue move` requires `--resolution` or `--no-resolution`)

## Specs & Plans

- **v1 design spec:** `docs/superpowers/specs/2026-03-21-jr-jira-cli-design.md`
- **v1 implementation plan:** `docs/superpowers/plans/2026-03-21-jr-implementation.md`
- **Feature specs (post-v1):** `docs/specs/` — one spec per feature, read before implementing

When adding a new feature:
1. Read this file
2. Read the v1 design spec for architectural context
3. Read relevant ADRs
4. Create a feature spec in `docs/specs/` before implementing
5. Follow TDD — write tests first

## Gotchas

- **Multi-profile boundary:** every cache reader/writer takes `profile: &str` as its first arg. Pass `&config.active_profile_name` from any handler that has `&Config` in scope. Cross-profile cache leakage is a correctness bug, not a UX issue — sandbox vs prod custom-field IDs can differ.
- **Per-profile vs shared OAuth keys:** `email`, `api-token`, `oauth_client_id`, `oauth_client_secret` live under flat keychain keys (account-level, shared across profiles). `oauth-access-token` / `oauth-refresh-token` are namespaced as `<profile>:oauth-*` because they're cloudId-scoped. The `"default"` profile lazy-migrates legacy flat keys on first read; other profiles do not.
- **Cache format changes:** `~/.cache/jr/v1/<profile>/cmdb_fields.json` stores `(id, name)` tuples. Old format (ID-only) causes deserialization failure, handled as cache miss. If you change cache structs, old caches auto-expire (7-day TTL) or fail gracefully. To break compatibility cleanly, bump the cache root from `v1/` to `v2/` — old files orphan harmlessly.
  - **Request-type caches (S-288-pr2):** `request_types_<sid>.json` + `request_type_fields_<sid>_<rtId>.json`; shape from `src/types/jsm/request_type.rs`. Same 7-day TTL / `v1/`-bump rules; deserialize failure → cache miss → refetch (self-heals).
- **`jr requesttype fields <NAME|ID>` numeric-bypass edge case:** all-ASCII-digit input is treated as a numeric RT ID and skips `partial_match`. A request type *named* `"100"` is therefore unreachable by name (no escape exists; look up its ID via `jr requesttype list --output json | jq`). Tracked behavior, not a bug — relevant if you add a `--by-id` flag.
- **Cache-write error handling — two models (S-288-pr2):** most writers in `src/cache.rs` propagate via `?`; the request-type writers (`write_request_type_cache`, `write_request_type_fields_cache`) swallow + `eprintln!("warning:…")` so a failed write never breaks a successful API call. New cache family → pick (a) propagate if correctness-critical, (b) swallow+warn if pure read-acceleration; document the choice in the writer's rustdoc.
- **`aqlFunction()` not `assetsQuery()`:** The Jira Assets JQL function is `aqlFunction()`. It requires the human-readable field **name**, not `cf[ID]` or `customfield_NNNNN`. AQL attribute for object key is `Key` (not `objectKey` — that's the JSON field name).
- **Status category colors are fixed:** `green` = Done, `yellow` = In Progress, `blue-gray` = To Do. These mappings are hardcoded in Jira Cloud across all instances. Used by `--open` filtering.
- **Embedded OAuth app uses fixed callback port 53682.** Callback URL is `http://127.0.0.1:53682/callback` — literal `127.0.0.1` (not `localhost`) to force IPv4 and dodge the macOS/Chrome `localhost`→`::1` pitfall; must match the Atlassian Developer Console registration, so changing the port is a breaking release. Release builds inject `JR_BUILD_OAUTH_CLIENT_ID`/`_SECRET` via `build.rs` → XOR-obfuscated `embedded_oauth.rs`. BYO sources (flag/env/keychain) keep dynamic-port behavior. Detail: ADR-0006, `docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md`.
- **`src/api/auth_embedded.rs` is a thin sibling module** to `auth.rs`. Keep
  obfuscation plumbing there; keep keychain/OAuth flow plumbing in `auth.rs`.
- **`--verbose` is header-only (SD-003 breaking change):** As of v0.6, `--verbose` shows method + URL + status only. It does NOT print request/response bodies. To inspect bodies (e.g., for debugging API calls), use `--verbose-bodies`. This flag emits a 3-line PII warning to stderr because bodies contain accountIds, email addresses, and ADF content. Do not use `--verbose-bodies` in shared terminals, debug log files piped to shared storage, or AI-agent context windows. Migration: `jr ... --verbose` → `jr ... --verbose --verbose-bodies` if body inspection was relied upon.
- **`refresh_oauth_token` resolves credentials internally** (keychain →
  embedded) — callers pass only `profile`. Do not re-introduce
  `client_id`/`client_secret` parameters; they short-circuit the resolver.
- **`--open` filter uses two mechanisms:** Jira issues → `statusCategory != Done` injected into JQL (server-side). Connected CMDB tickets (`cli/assets.rs::filter_tickets`) → client-side `status.colorName != "green"`, because CMDB tickets don't support JQL `statusCategory` filtering.
- **User pagination advances by `USER_PAGE_SIZE`, not returned count:** In
  `src/api/jira/users.rs`, both `search_users_all` and `search_assignable_users_by_project_all`
  increment `start_at` by `USER_PAGE_SIZE` (100) after each page, NOT by the number of
  users returned. This is a workaround for JRACLOUD-71293: Jira uses fixed-window
  pagination (selects range [startAt, startAt+maxResults) then applies permission
  filtering), so a short non-empty page does NOT mean end-of-data. Advancing by returned
  count would overlap windows and produce duplicates. Do not change this behavior.
- **`jr requesttype list/fields` (JSM, `cli/requesttype.rs`):** `list` (Name, Description) + `fields <NAME|ID>` (Field Name, Required, Type). Caches per `(profile, sid[, rtId])`; `--search` results are never cached, only the full list. Cache fns: `{read,write}_request_type[_fields]_cache`.
- **`require_service_desk` call-site label (BC-X.8.004):** `call_site_label: &'static str` is embedded before ` a Jira Service Management project.` (verb dropped for plural/singular agreement), so every caller must pass a full noun-phrase ending in the matching verb (e.g. `…require` / `…requires`). Canonical contract + current caller list: `src/api/jsm/servicedesks.rs::require_service_desk` rustdoc.
- **`jr issue create --request-type` dispatch fork (S-288-pr4):** `--request-type` set → `handle_create` short-circuits to `handle_jsm_create`, POSTing `/rest/servicedeskapi/request` instead of `/rest/api/3/issue` (gated solely on `request_type.is_some()`; absent → platform path byte-for-byte unchanged). On JSM path: `--type` silently ignored (BC-3.8.010), custom fields via `--field`, 401 → `write:servicedesk-request` scope hint. Detail: ADR-0014.
- **When changing `DEFAULT_OAUTH_SCOPES`:** (1) update the embedded `jr` OAuth app's
  permissions in the Atlassian Developer Console at
  https://developer.atlassian.com/console/myapps/ before tagging the release, and
  (2) add a CHANGELOG entry mentioning the re-consent prompt so users aren't surprised.
  Existing access tokens continue working with old scopes until expiry; new logins and
  refresh-token mints trigger re-consent.
- **`board view` truncation hint emits to stderr:** The truncation hint ("Showing N of M
  columns — use --all to see everything") is written to stderr, consistent with the
  convention used by `issue list` and `sprint current`. This is intentional — stderr
  keeps hints out of `--output json` and pipe-friendly stdout.
- **Atlassian's expired-token 401 has no machine-readable signal:** body is `{"errorMessages":[…expired…]}` with NO `code` field and NO RFC-6750 `WWW-Authenticate` header. Auto-refresh (S-3.03) therefore triggers on blanket-401 (`gh` CLI pattern), not substring-match or header inspection. Detail: `.factory/research/S-3.03-wave3-verification.md` (Claim 2, REFUTED).
- **Refresh tokens are strictly single-use on Atlassian.** The "10-minute reuse window" from secondary sources is undocumented and fails in clusters — treat each as one-shot. Concurrent refresh MUST go through `src/api/refresh_coordinator.rs` per-profile single-flight to avoid `invalid_grant` races. Detail: `.factory/research/S-3.03-v2-design-verification.md` (Claim 5, REFUTED).
- **`refresh_coordinator.rs` mutex layering:** outer `std::sync::Mutex<HashMap>` held only briefly for lookup/insert, released BEFORE any `.await`. Inner `tokio::sync::Mutex<RefreshState>` held across the refresh `.await` — MUST be `tokio::sync::Mutex` (no poison-on-panic; a panicked refresh must not permanently break the coordinator). Detail: S-3.03 v2 spec.
- **Bulk transitions are not idempotent.** Single-key `move` exits 0 if already in target status; multi-key `move` (positional or `--to`) transitions every key unconditionally → expect per-key 400s on workflows that reject same-status moves. Pre-filter with `jr issue list --jql "… AND status != \"<target>\""`. Note: `--jql` selection is on `edit` only, not `move`.
  - **Bulk transition wire schema is NESTED, not flat (FIX-BULK-TRANSITION-001):** `POST /rest/api/3/bulk/issues/transition` requires `{"bulkTransitionInputs":[{"selectedIssueIdsOrKeys":[…],"transitionId":"…"}],"sendBulkNotification":false}`. The flat top-level shape (`selectedIssueIdsOrKeys` + `transitionId` at root) documented in the Atlassian OpenAPI JSON is wrong — live Jira rejects it with 400 "bulkTransitionInputs must not be empty". Same asymmetry class as `labelsFields`/`"labels"` and `issueType`/`"issuetype"`. Confirmed: Atlassian community 2026-02-19 + live run 27156639337.
- **`/rest/api/3/search/jql` repeated-`nextPageToken` = JRACLOUD-95368** (live-data drift between page fetches), NOT -94632/-92049/-85546 — those three are misattributed (verified, issue #361/PR #364). `search_issues` / `search_issue_keys` in `src/api/jira/issues.rs` carry an anti-loop guard (sets `has_more=true` on abort) plus an incremental `seen_keys: HashSet` that dedupes all exit paths in first-occurrence order. Two load-bearing string rules: (1) the stderr ORDER BY hint must read "append `, key ASC` to an existing sort, or use `ORDER BY key ASC` if none" — never a bare "add `ORDER BY key ASC`" (JQL allows one ORDER BY → HTTP 400 if user already sorts); (2) the literal `"JRACLOUD-95368"` is pinned by `tests/rate_limit_cap_tests.rs` and `tests/search_issue_keys.rs::test_..._emits_jracloud_95368_literal` — update together. Detail: `.factory/research/issue-361-jra95368-scope.md`, `-jql-orderby.md`.
- **`issue edit` description echo asymmetry (issue #398):** Table/human output echoes
  `description → (updated)` — a marker, never the content. JSON `changed_fields.description`
  carries the **raw user-supplied input string** from `--description` / `--description-stdin`,
  not `"(updated)"` and not an ADF→text round-trip. The two channels intentionally differ:
  the human channel optimizes for scannability; the machine channel must be lossless. Do NOT
  "fix" them to match — this asymmetry is load-bearing. Tested by VP-398-002
  (`test_bc_3_4_012_description_echo_is_updated_marker_not_content` and
  `test_bc_3_4_013_description_echo_is_raw_input_string_not_marker`).
- **`issue edit --field` constraints and JSM behavior (issue #396):**
  (1) single-key only — C-1 guard exits 64 on bulk.
  (2) Changing a JSM issue's **Request Type** is unsupported by any Jira Cloud API — `--field` rejects `sd-customerrequesttype` (JSDCLOUD-4609; PUT 500; out-of-scope).
  (3) JSM Urgency/Impact and other RT select fields CAN be set via `--field NAME=VALUE` **only if** an admin has added the field to the issue's agent Edit screen (portal-only by default).
  (4) `--field` adds one `GET …/editmeta` round-trip (skipped when absent) to validate field + resolve `allowedValues`.
  (5) `write_fields_cache` (in `resolve_edit_fields`) is unconditional on miss/stale (mirrors `write_cmdb_fields_cache`); cache-touching tests use `jr_cmd_with_xdg` + per-test `TempDir` to avoid the real `fields.json`.
  (6) `--field` + `--label` on one key → exit 64 (mutual-exclusion block in `create.rs::handle_edit`); without the guard the label→bulk routing fork would silently drop the `--field` write. Combined label+custom-field bulk tracked at #331. [FIX-F5-001]
- **`issue edit --label` endpoint fork (BUG-LABEL-400):** `handle_edit_bulk_labels` routes on key count. ONE key (incl. a `--jql` set matching one) → `PUT /issue/{key}` with **bare-string** labels via `update_issue_labels` (sync 204). TWO+ keys → `POST /bulk/issues/fields` with `{"name":…}` **objects** via `build_labels_edited_fields` (async poll). The two payload shapes are **asymmetric — do not unify**; both were proven against real Jira (single-key path + #446 bulk-schema fix after live 400s). Broader schema-verification effort: #331.
- **`issue edit --type` multi-key bulk path (S-331, BC-3.4.018/019):**
  1. **camelCase/lowercase asymmetry (load-bearing, do NOT fix):** `selectedActions` uses lowercase `"issuetype"` (canonical field ID); `editedFieldsInput` uses camelCase `"issueType"` (bean name) — verbatim per Atlassian Bulk Ops FAQ, same as `labelsFields`/`"labels"`. Detail: `.factory/research/issue-331-issuetype-bulk-schema.md`.
  2. **Cross-project exit-64 guard (BC-3.4.019):** bulk `--type` requires all keys in ONE project (endpoint takes a single `issueTypeId`); guard fires in `handle_edit_bulk_fields` only when `--type` is present.
  3. **Name→issueTypeId resolution (project-scoped, no cache):** `GET …/createmeta/{proj}/issuetypes`, case-insensitive; unknown name → exit 64 listing valid types. One HTTP call per bulk `--type`. `issues.rs::get_issue_types_for_project`.
- **`jr issue move <KEY> <done-status>` proactive resolution enforcement (BC-3.2.013, ADR-0015):**
  `handle_move` (single-key only; bulk excluded) calls `get_transitions_with_fields` (`?expand=transitions.fields`) instead of `get_transitions`. When the resolved transition targets a done-category status (`to.statusCategory.key == "done"`) AND either `transition.fields` contains a `"resolution"` key OR `transition.is_conditional == Some(true)`, `jr` intercepts BEFORE the POST:
  - **Non-interactive (`--no-input` or stdin not a TTY):** exit 64, stderr: `The transition to "<to_label>" requires a resolution.` followed by a `Try:` hint line and `Run \`jr issue resolutions\`` suggestion (exact wording from `workflow.rs::handle_move` REQUIRED branch — load-bearing substrings: `"requires a resolution"`, `"--resolution"`).
  - **Interactive (TTY):** `dialoguer::Select` prompt listing resolutions from `load_resolutions(client, false)`.
  - **`--resolution <name>` supplied:** resolves via `resolve_resolution_by_name` (exact > prefix > substring), sends atomically in transition body. Ambiguous → exit 64 with candidate list.
  - **`--no-resolution` supplied:** explicit opt-out — proactive enforcement skipped, transition sent without resolution field (identical to pre-ADR-0015 default). Mutually exclusive with `--resolution` (clap → exit 2). Accepted silently on non-done-category transitions (no-op).
  - Bulk `jr issue move` is EXCLUDED — retains BC-3.2.009 reactive backstop only. Scripts doing bulk done-category moves should supply `--resolution` upfront.
  - `jr issue transitions --output json` output is byte-for-byte identical: `Transition.fields` and `Transition.is_conditional` carry `#[serde(skip_serializing)]` so they are never emitted in JSON serialization.
  - The reactive BC-3.2.009 backstop (400 "resolution is required") is preserved alongside BC-3.2.013 as a fallback.
  Detail: BC-3.2.013, ADR-0015, `docs/specs/issue-move-resolution.md`.
- **Markdown footnotes → ADF (`adf.rs`, issue #472):** `markdown_to_adf` enables `Options::ENABLE_FOOTNOTES`. ADF has no native footnote node, so the mapping is **preserve, not drop**: a reference `[^1]` becomes a *plain, unmarked* `[label]` text marker (via `push_footnote_marker` — deliberately does NOT inherit active marks, so a ref inside `**bold**` is not bolded); definitions are collected and flushed at `finish()` into an appended section (one `rule` divider + one `[label] `-prefixed paragraph each). Load-bearing details: (1) pulldown-cmark only emits `FootnoteReference` for *defined* labels — an undefined `[^x]` stays literal text upstream; (2) duplicate `[^1]:` lines are deduped by label (first wins) via `footnote_labels_seen`; (3) definitions render in definition-source order, not reference order (deliberate, deterministic); (4) the `[n]` marker can collide visually with literal `[n]` user text (accepted ADF-mapping limitation). **Empty-container pruning:** pulldown hoists footnote defs out of enclosing blocks, leaving empty shells (`> [^1]: x` → empty `blockquote`); `is_empty_block_container` (a free fn) prunes any `blockquote`/`heading`/`panel`/`listItem`/`bulletList`/`orderedList`/`table`/`tableRow` left with empty `content` (invalid ADF → Jira 400). `paragraph`/`codeBlock`/`tableCell`/`tableHeader` are excluded (empty is valid ADF / a placeholder paragraph keeps cells non-empty / pruning a cell would break column alignment). This also prunes a bare-`#` empty heading. Detail: `src/adf.rs` rustdoc + `adf::tests::test_markdown_footnote_*`.
- **Markdown minor constructs → ADF (`adf.rs`, issue #474):** `markdown_to_adf` also enables `ENABLE_SUPERSCRIPT | ENABLE_SUBSCRIPT | ENABLE_HEADING_ATTRIBUTES`. (1) **subsup:** `^x^` → `subsup` sup, `~x~` → `subsup` sub (rendered back by `adf_to_text` for a lossless round-trip). Enabling `ENABLE_SUBSCRIPT` reassigns *single*-tilde `~x~` from strikethrough to subscript; *double*-tilde `~~x~~` stays `strike` (pinned by `test_markdown_double_tilde_still_strikethrough_not_subscript`). Nested same-type spans (`^a ~b~ c^`) are deduped by `dedup_marks_by_type` so a text node never carries two `subsup` marks (ADF rejects duplicate mark types). **Limitations:** (a) pulldown does not open a superscript when `^` is tight against a preceding word char, so `mc^2^` stays literal — use `mc ^2^`; (b) `code` mark cannot coexist with `subsup`/`em`/`strong`/`strike` on one text node per the ADF schema (`code_inline_node`), so `` ^`x`^ `` would be invalid — not guarded here (pre-existing class: `` **`x`** `` has the same issue; tracked as a follow-up). (2) **Heading attrs:** `## Title {#id}` no longer leaks `{#id}` into text (ADF headings have no id; id/classes/attrs are parsed and dropped). **NOT done:** GFM alerts (`> [!NOTE]`) → ADF `panel` is intentionally deferred — ADF `panel.content` forbids nested `panel`/`table`/`blockquote` and `listItem` forbids `panel`, so it needs the same content-model normalization as #470 (see the comment in `markdown_to_adf`); until then `> [!NOTE]` stays a plain blockquote. Detail: `adf::tests::test_markdown_{superscript,subscript,heading_attributes}*` + `test_render_subsup_mark_reverse_path`.

## AI Agent Notes

- `JR_BASE_URL` env var overrides the configured Jira instance URL (used by tests to inject wiremock). **Debug builds only** — release binaries ignore this env var. The override is gated via `#[cfg(debug_assertions)]` at BOTH read sites — `Config::base_url()` in `src/config.rs` and `JiraClient::from_config()` in `src/api/client.rs` — because either site alone leaves a token-leak vector where `JR_BASE_URL=http://attacker/` would redirect authenticated requests to a non-Atlassian host. Regression-pinned by `tests/base_url_release_gate.rs`. Mirrors the existing `JR_AUTH_HEADER` gate (SD-002).
- `JR_BULK_UNKNOWN_GRACE_SECS` overrides the unknown-bulk-task grace period (default 30s). Debug-only, single-site gate in `bulk.rs::resolve_unknown_status_grace` (not security-critical). Pinned by `tests/bulk_unknown_grace_release_gate.rs`. (#336)
- `JR_BULK_AWAIT_TIMEOUT_SECS` overrides the bulk-poll wall-clock timeout (default 300s). Debug-only, single-site gate in `bulk.rs::resolve_bulk_await_timeout`. Pinned by `tests/bulk_await_timeout_release_gate.rs`. (#333)
- `JR_E2E_ENABLED` — GitHub Actions **repository variable** (`vars.JR_E2E_ENABLED`). Gates the `e2e:` job at scheduling time. NOT a Rust env var; never read by `src/` code. Forks with this variable unset skip cleanly (empty string `!= 'true'`). The canonical repo sets `JR_E2E_ENABLED=true` as a repository variable in GitHub repo settings (not environment-scoped — environment-level variables are not available in `jobs.<id>.if:`). See `docs/specs/e2e-fork-safe-ci-enablement.md §2.3`.
- **When adding a new `JR_*` test-seam env var:** grep `CLAUDE.md` for existing `JR_*` entries and add a parallel line in the SAME commit as the code change. This is the codified doc-fallout pattern from #335/#357; first applied retroactively when `JR_BULK_UNKNOWN_GRACE_SECS` and `JR_BULK_AWAIT_TIMEOUT_SECS` shipped without documentation.
- **Citation discipline for external-tracker IDs in user-facing strings:** before citing a JRACLOUD-*/GitHub/community ID in anything a user sees (stderr, errors, JSON, hints) or in literal rustdoc, Perplexity-validate the source actually documents the symptom — issue #361 had three misattributed JRACLOUD tickets survive multiple PRs. Also ensure the string is valid in the user's env (e.g. JQL allows one ORDER BY) and keep paraphrasing rustdoc in lockstep. Detail: `.factory/research/issue-361-validation.md`, `-followup.md`.
- **Citation form in spec/CLAUDE.md:** prefer symbol-form (`<file>::<fn>` or `… § "<comment>"`) over line numbers, which drift on refactor. Fall back to `<file>:~NN` (`~` = approximate); never a bare `<file>:NN-MM` for new citations. (#408)
- `JR_PROFILE` env var overrides the active profile per-call (combine with direnv to scope a repo to a sandbox site)
- `--profile NAME` flag overrides `JR_PROFILE` for one invocation; precedence is flag > env > config > "default"
- `JiraClient::new_for_test(base_url, auth_header)` constructs a client for integration tests
- Test fixtures live in `tests/common/fixtures.rs`
- Keyring round-trip tests are gated behind `JR_RUN_KEYRING_TESTS=1` + `#[ignore]` (Linux CI may lack secret-service; macOS prompts on novel service names). Coverage in `src/api/auth.rs` (inline), `tests/auth_profiles.rs`, `tests/multi_cloudid_disambiguation.rs`, `tests/oauth_refresh_integration.rs`. Run: `JR_RUN_KEYRING_TESTS=1 cargo test -- --include-ignored`.
- OAuth integration tests in `tests/oauth_embedded_login.rs` are gated behind `JR_RUN_OAUTH_INTEGRATION=1` + `#[ignore]`. The test is currently `unimplemented!()` — it requires a wiremock base-URL override in `oauth_login` before a real assertion can be written. CI does not run it; the embedded-creds smoke test in `release.yml` covers the binary-level check instead.
- **Two-layer E2E gate:** `JR_E2E_ENABLED` (workflow level, repo variable) determines whether the CI job starts; `JR_RUN_E2E` (test-binary level, process env) determines whether individual `#[ignore]` tests run. The workflow sets `JR_RUN_E2E=1` only when the job starts. Each layer independently provides fail-safe behavior. See `docs/specs/e2e-fork-safe-ci-enablement.md §2.1`.
- Live-Jira E2E tests (`tests/e2e_live.rs`) are gated behind `JR_RUN_E2E=1` + `#[ignore]` + a per-test `if !e2e_enabled() { return; }` early-return (S-410). Inert in `cargo test`/`ci.yml` (never `--include-ignored`); gate correctness pinned by `test_e2e_gate_disabled_when_env_unset` + `test_every_ignored_test_has_gate_guard`. Runs only in `.github/workflows/e2e.yml` (non-blocking). Local: `JR_RUN_E2E=1 JR_E2E_BASE_URL=… JR_AUTH_HEADER=… JR_E2E_PROJECT=E2E cargo test --test e2e_live -- --include-ignored --test-threads=1`. **Full env-var table: `docs/specs/e2e-live-jira-testing.md`.** Quick reference (all debug-only seams except `JR_RUN_E2E`):
  - Required: `JR_RUN_E2E`, `JR_E2E_BASE_URL` (→ `JR_BASE_URL`), `JR_AUTH_HEADER` (composed from `JR_E2E_EMAIL`+`JR_E2E_API_TOKEN`), `JR_E2E_PROJECT`.
  - Optional: `JR_E2E_BOARD_ID` (sprint tests), `JR_E2E_JSM_PROJECT` (JSM tests — value is `EJ`; 8 test functions gated on this var; set as environment variable in `jira-e2e` GitHub Environment, NOT a repository variable), `JR_E2E_JSM_RESOLUTION` (resolution name override for `jsm_self_close` teardown and `test_e2e_jsm_resolution_enforcement`; auto-discovered from `jr issue resolutions` when unset — S-JSM-E2E-3), `JR_E2E_STATUS_DONE`/`_IN_PROGRESS` (default Done/In Progress), `JR_E2E_ISSUE_TYPE` (default Task), `JR_E2E_ISSUE_TYPE_ALT` (bulk issueType round-trip; clean-skips if unset), `JR_E2E_POLL_MAX_ATTEMPTS`/`_INITIAL_MS` (test-side poll loop; defaults 5/250ms), `JR_E2E_PARENT_KEY` + `JR_E2E_CHILD_TYPE` (paired; enable `create --parent` test — E2E-HV-2; clean-skip if either unset), `JR_E2E_EDIT_FIELD` (`NAME=VALUE`; enables `edit --field` test — E2E-HV-2; clean-skip if unset). The story-points round-trip test (`create/edit --points`, `--no-points`) needs no env var — it auto-discovers the Story Points field id via `jr api /rest/api/3/field` and writes a temp config, clean-skipping when no such field exists or it is absent from the Create screen.
- **JSM write-test teardown convention (S-JSM-E2E-2 + S-JSM-E2E-3):** Tests that create JSM requests (Scenarios 5+6+8 in `tests/e2e_live.rs`) self-close via `jsm_self_close(key, &h)` in the test body — NOT via the label-based CI sweeper and NOT via the hardcoded `status_done()` name. Root cause for the helper: the EJ JSM workflow has no transition named "Done" (live run 26839267723 confirmed: tests passed green but left ~2 open EJ tickets). `jsm_self_close` dynamically discovers a closing transition via `jr issue transitions <key> --output json`, selects the first transition with `to.statusCategory.key == "done"` (the stable Jira-wide machine constant for a closing/green status — covers Resolved, Closed, Canceled regardless of workflow name), then attempts resolution discovery (S-JSM-E2E-3): it runs `jr issue resolutions --output json` and passes `--resolution <R>` on the move to produce properly-resolved tickets. Resolution name source: `JR_E2E_JSM_RESOLUTION` env override first, else first `name` from the resolutions list. If resolution discovery fails for any reason, falls back to move WITHOUT `--resolution` (S-JSM-E2E-2 behavior). The `status_done()` helper and its use in ES-project tests remain unchanged — ES Scrum correctly has a transition named "Done". Labels do NOT reliably propagate from `JsmRequestBuilder::build()` to the Jira issue `labels` field, so the sweeper cannot catch EJ-created requests. Self-close is best-effort: all failure paths emit `eprintln!("[WARN] jsm_self_close: …")` and return `()`; the test never fails on close failure. Residual orphan risk LOW and accepted (EJ issues are inert; Resolution-screen transition may still fail; see `docs/specs/jsm-e2e-coverage.md §6.3`). Do NOT extend the sweeper for EJ without first fixing label propagation.
- **JSM resolution proactive enforcement (BC-3.2.013, ADR-0015):** `jr issue move` intercepts done-category transitions BEFORE the POST: in non-interactive mode (`--no-input` or stdin not a TTY), exits 64 with `The transition to "<to_label>" requires a resolution.` + `--resolution` hint. Interactive: `dialoguer::Select` prompt. Supply `--resolution <name>` to send the resolution atomically in the transition body (BC-3.2.011); supply `--no-resolution` to opt out of enforcement. The reactive BC-3.2.009 backstop (API 400 "resolution required") is preserved alongside. Run `jr issue resolutions` to discover available resolution names. See `docs/specs/jsm-e2e-coverage.md §5 Scenario 8` for the E2E enforcement test.
- **E2E suite maintenance:** The nightly `e2e.yml` schedule (`0 6 * * *` UTC) is a **data-retention guard**, not just a latency optimization: free Jira Cloud sites deactivate after ~120 days of inactivity and have only a ~15–60 day reactivation window before permanent data deletion. If the nightly job fails with HTTP 401, the `JR_E2E_API_TOKEN` secret has expired — Atlassian caps API tokens at 1 year. Rotate the token in the CI service account before expiry and update the secret in the `jira-e2e` GitHub Environment. Annual rotation is required. Runbook: `docs/specs/e2e-live-jira-testing.md` §9.
- `tests/e2e_cli_surface_guard.rs` — always-run offline guard validating every `jr` subcommand path and flag referenced in `tests/e2e_live.rs` against `jr --help`; catches assumed-CLI-surface defects (nonexistent subcommands/flags) at CI time without requiring `JR_RUN_E2E` or any network access. Does NOT check JSON output shape or exit-code semantics (use serde types or a live run for that). Implemented as E2E-PG-1 / DRIFT-E2E-1; pairs the mechanical `--help` validator with a parser-consistency check (`test_parser_paths_are_subset_of_surface_table`) that flags new e2e_live.rs invocations not yet registered in the guard's SURFACE table.
- All interactive prompts have non-interactive flag equivalents for AI agent usage
- `--output json` on write operations returns structured data (e.g., `{"key": "FOO-123"}`)
- Run `scripts/check-spec-counts.sh` after any edit to .factory/specs/prd/ BC files,
  nfr-catalog.md, or holdout-scenarios.md. Exits 0 if frontmatter counts match body counts.
  Exits 1 with specific mismatch details if drift is detected (DRIFT-001 mitigation).
- Run `scripts/check-bc-cumulative-counts.sh` after any edit to BC-INDEX.md, CANONICAL-COUNTS.md,
  or `total_bcs:` frontmatter values. Exits 0 if all 8 surfaces agree: per-file frontmatter
  (Surface A), BC-INDEX.md Section headers (B), BC-INDEX.md sections: lines (C),
  CANONICAL-COUNTS.md per-file table (D), body preamble prose, BC-INDEX.md frontmatter
  total_bcs (Surface E), CANONICAL-COUNTS.md Sum row (F), and grand-total prose (G).
  Exits 1 with specific mismatch details if drift is detected (DRIFT-002 mitigation).
- **BC Trace and Source fields must not contain numeric test counts.** The `Trace:` and
  `Source:` fields in `.factory/specs/prd/bc-*.md` BC bodies should describe coverage
  qualitatively (file path + test category) rather than enumerate counts. Counts drift as
  tests are added; qualitative descriptions are stable. Enforced by
  `scripts/check-bc-no-numeric-test-counts.sh` in CI. Convention added by PG-365-1.
- `cargo-mutants` is a binary tool installed via `cargo install cargo-mutants --locked` — do NOT add it to `Cargo.toml` as a dev-dependency. Scope and config live in `.cargo/mutants.toml`; policy lives in `docs/specs/cargo-mutants-policy.md`.
