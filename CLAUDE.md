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
│   └── queue.rs         # queue list/view (JSM service desks)
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
- **`list.rs` is large (~970 lines):** Contains both `handle_list` and `handle_view` plus all JQL composition logic. If modifying, read the full function you're changing — context matters.
- **`aqlFunction()` not `assetsQuery()`:** The Jira Assets JQL function is `aqlFunction()`. It requires the human-readable field **name**, not `cf[ID]` or `customfield_NNNNN`. AQL attribute for object key is `Key` (not `objectKey` — that's the JSON field name).
- **Status category colors are fixed:** `green` = Done, `yellow` = In Progress, `blue-gray` = To Do. These mappings are hardcoded in Jira Cloud across all instances. Used by `--open` filtering.
- **Embedded OAuth app uses fixed callback port 53682.** The release build
  workflow injects `JR_BUILD_OAUTH_CLIENT_ID`/`_SECRET` (CI-only env vars)
  via `build.rs`, which generates an XOR-obfuscated `embedded_oauth.rs` in
  `$OUT_DIR`. The bound callback URL is `http://127.0.0.1:53682/callback`
  (literal `127.0.0.1`, not `localhost` — forces IPv4 to match the listener
  bind and avoids the macOS/Chrome `localhost`→`::1` resolver pitfall),
  registered exactly in Atlassian Developer Console. Changing the port is a
  breaking release. BYO sources (flag, env, keychain) keep the historical
  dynamic-port behavior. See ADR-0006 and
  `docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md`.
- **`src/api/auth_embedded.rs` is a thin sibling module** to `auth.rs`. Keep
  obfuscation plumbing there; keep keychain/OAuth flow plumbing in `auth.rs`.
- **`--verbose` is header-only (SD-003 breaking change):** As of v0.6, `--verbose` shows method + URL + status only. It does NOT print request/response bodies. To inspect bodies (e.g., for debugging API calls), use `--verbose-bodies`. This flag emits a 3-line PII warning to stderr because bodies contain accountIds, email addresses, and ADF content. Do not use `--verbose-bodies` in shared terminals, debug log files piped to shared storage, or AI-agent context windows. Migration: `jr ... --verbose` → `jr ... --verbose --verbose-bodies` if body inspection was relied upon.
- **`refresh_oauth_token` resolves credentials internally** (keychain →
  embedded) — callers pass only `profile`. Do not re-introduce
  `client_id`/`client_secret` parameters; they short-circuit the resolver.
- **`--open` filter uses two mechanisms:** For Jira issues, `statusCategory != Done` is
  injected into the JQL query (server-side filter). For connected CMDB tickets in
  `cli/assets.rs` (`filter_tickets` function), filtering is client-side via
  `status.colorName != "green"`. Both implement the same user intent ("show only open
  items") but at different layers — CMDB connected tickets do not support JQL
  `statusCategory` filtering. Status category colors are hardcoded in Jira Cloud:
  `green` = Done, `yellow` = In Progress, `blue-gray` = To Do.
- **User pagination advances by `USER_PAGE_SIZE`, not returned count:** In
  `src/api/jira/users.rs`, both `search_users_all` and `search_assignable_users_by_project_all`
  increment `start_at` by `USER_PAGE_SIZE` (100) after each page, NOT by the number of
  users returned. This is a workaround for JRACLOUD-71293: Jira uses fixed-window
  pagination (selects range [startAt, startAt+maxResults) then applies permission
  filtering), so a short non-empty page does NOT mean end-of-data. Advancing by returned
  count would overlap windows and produce duplicates. Do not change this behavior.
- **`board view` truncation hint emits to stderr:** The truncation hint ("Showing N of M
  columns — use --all to see everything") is written to stderr, consistent with the
  convention used by `issue list` and `sprint current`. This is intentional — stderr
  keeps hints out of `--output json` and pipe-friendly stdout.
- **Atlassian's expired-access-token 401 response shape:** The Jira REST API v3 returns
  `HTTP 401` with body `{"errorMessages": ["The access token provided is expired, revoked,
  malformed, or invalid for other reasons."]}` — there is NO machine-readable `code` field
  and NO RFC-6750-compliant `WWW-Authenticate: Bearer error="invalid_token"` header.
  Auto-refresh wiring (S-3.03) uses blanket-401 trigger (matches `gh` CLI pattern), not
  substring-match (locale-fragile) and not RFC-6750 header inspection (Atlassian doesn't
  follow the spec). Source: `.factory/research/S-3.03-wave3-verification.md` (Claim 2, REFUTED).
- **Refresh-token rotation is strictly single-use on Atlassian.** The "10-minute reuse
  window" mentioned in some secondary sources (Nango.dev, etc.) is NOT documented by
  Atlassian and is known to fail in clusters. Treat each refresh token as one-shot.
  Concurrent refresh attempts MUST go through `src/api/refresh_coordinator.rs`
  per-profile single-flight to avoid `invalid_grant` races. Source:
  `.factory/research/S-3.03-v2-design-verification.md` (Claim 5, REFUTED).
- **`refresh_coordinator.rs` mutex layering rule:** outer `std::sync::Mutex<HashMap<...>>`
  is held ONLY BRIEFLY for HashMap lookup/insert; it is released BEFORE any `.await`.
  Inner `tokio::sync::Mutex<RefreshState>` is held across the refresh `.await`. NEVER
  use `std::sync::Mutex` for the inner mutex — `tokio::sync::Mutex` is mandatory because
  it does NOT poison on panic, which is the correct semantic for refresh (a panicked
  refresh should not permanently break the coordinator). Source: S-3.03 v2 spec.
- **Bulk transitions are not idempotent.** Single-key `move` exits 0 if the issue is
  already in the target status. Multi-key `move KEY1 KEY2 ... STATUS` (legacy positional
  form) and `move KEY1 KEY2 ... --to STATUS` issue the transition unconditionally for
  every key. For workflows that reject same-status transitions, expect per-key 400
  errors in the bulk response. To pre-filter, list candidate keys first with
  `jr issue list --jql "<query> AND status != \"<target>\"" --output json` and pass
  them as positional args. The `--jql` selection form is on `edit` only — `move` does
  not accept `--jql`.
- **`/rest/api/3/search/jql` repeated-`nextPageToken` symptom = JRACLOUD-95368, NOT
  JRACLOUD-94632 / -92049 / -85546.** When Jira Cloud's enhanced JQL search returns the
  same `nextPageToken` on consecutive pages (which would cause an infinite client loop
  without the anti-loop guard in `search_issues` / `search_issue_keys` in
  `src/api/jira/issues.rs`), the documented root-cause ticket is
  [JRACLOUD-95368](https://jira.atlassian.com/browse/JRACLOUD-95368) — *"nextPageToken
  pagination is not snapshot-stable under live mutation"* — i.e. live-data drift between
  page fetches. Earlier code/spec versions cited JRACLOUD-94632 / -92049 / -85546 as
  "confirmed upstream bugs"; **all three are misattributed** (94632 = initial
  `nextPageToken=null` rejection, fixed Jun 2025; 92049 = `startAt` offset behavior,
  resolved Invalid; 85546 = `/field/search` `nextPage` field, different endpoint). The
  rebind happened in issue #361 / PR #364 (2026-05-14). Both `SearchResult.has_more`
  and `KeySearchResult.has_more` set `true` on guard abort. Both `search_issue_keys`
  and `search_issues` use an incremental `seen_keys: HashSet<String>` (maintained
  outside the pagination loop) to deduplicate on all exit paths — unique keys/issues
  are appended once in first-occurrence order; the accumulated Vec is never rescanned
  (implemented in #365 — closed). Duplicate keys/issues from live-data drift are
  eliminated client-side before any break-decision check. Callers receive a
  duplicate-free result. The user-facing stderr warning includes an
  actionable ORDER BY mitigation; do NOT revert to a single-`ORDER BY` shorthand like
  "add `ORDER BY key ASC` to your JQL" — JQL allows only one ORDER BY clause, so users
  with an existing sort would receive HTTP 400. The precise wording is "append
  `, key ASC` to an existing sort, or use `ORDER BY key ASC` if none". Stderr-literal
  pin: any change to the literal `"JRACLOUD-95368"` in the warning must be paired with
  updates to `tests/rate_limit_cap_tests.rs::ac_008_and_ac_new_d_…` and
  `tests/search_issue_keys.rs::test_search_issue_keys_stderr_emits_jracloud_95368_literal`.
  Source: `.factory/research/issue-361-jra95368-scope.md`,
  `.factory/research/issue-361-jql-orderby.md`.

## AI Agent Notes

- `JR_BASE_URL` env var overrides the configured Jira instance URL (used by tests to inject wiremock). **Debug builds only** — release binaries ignore this env var. The override is gated via `#[cfg(debug_assertions)]` at BOTH read sites — `Config::base_url()` in `src/config.rs` and `JiraClient::from_config()` in `src/api/client.rs` — because either site alone leaves a token-leak vector where `JR_BASE_URL=http://attacker/` would redirect authenticated requests to a non-Atlassian host. Regression-pinned by `tests/base_url_release_gate.rs`. Mirrors the existing `JR_AUTH_HEADER` gate (SD-002).
- `JR_BULK_UNKNOWN_GRACE_SECS` env var overrides the unknown-bulk-task-status grace period (default 30s). **Debug builds only** — gated via `#[cfg(debug_assertions)]` in `src/api/jira/bulk.rs::resolve_unknown_status_grace`. Used by CLI integration tests to drive the warn+escalate path in ~1s. Not security-critical (no token-leak vector); single-site gate. Regression-pinned by `tests/bulk_unknown_grace_release_gate.rs`. Closes audit-followup #336.
- `JR_BULK_AWAIT_TIMEOUT_SECS` env var overrides the bulk-poll wall-clock timeout (default 300s / 5min). **Debug builds only** — gated via `#[cfg(debug_assertions)]` in `src/api/jira/bulk.rs::resolve_bulk_await_timeout`. Used by `tests/bulk_deadline_propagation.rs` to drive the 429-storm clamp through the binary in ~30s instead of ~300s. Not security-critical (no token-leak vector); single-site gate. Regression-pinned by `tests/bulk_await_timeout_release_gate.rs`. Closes audit-followup #333.
- **When adding a new `JR_*` test-seam env var:** grep `CLAUDE.md` for existing `JR_*` entries and add a parallel line in the SAME commit as the code change. This is the codified doc-fallout pattern from #335/#357; first applied retroactively when `JR_BULK_UNKNOWN_GRACE_SECS` and `JR_BULK_AWAIT_TIMEOUT_SECS` shipped without documentation.
- **Citation discipline for external-tracker IDs in user-facing strings:** when adding a JRACLOUD-* / GitHub-issue / community-thread citation to anything a user will see (stderr warnings, error messages, JSON output, runtime hints) OR to a rustdoc / code comment that future maintainers will read literally, validate the cited source actually documents the symptom you're attributing to it — Perplexity is the cheapest validator. The PR #364 cycle (issue #361) showed the existing codebase had three misattributed JRACLOUD tickets that survived multiple PRs because no one had read them; nine rounds of Copilot review then surfaced five separate sites where rustdoc shorthand "(`ORDER BY key ASC`)" would have prompted invalid JQL when copy-pasted. Default to: (1) Perplexity-validate any external claim, (2) ensure user-facing strings are syntactically valid in the user's likely environment (e.g., JQL only allows one ORDER BY clause), (3) if rustdoc/comment paraphrases the user-facing text, keep them in lockstep. Source: `.factory/research/issue-361-validation.md` and `-followup.md`.
- `JR_PROFILE` env var overrides the active profile per-call (combine with direnv to scope a repo to a sandbox site)
- `--profile NAME` flag overrides `JR_PROFILE` for one invocation; precedence is flag > env > config > "default"
- `JiraClient::new_for_test(base_url, auth_header)` constructs a client for integration tests
- Test fixtures live in `tests/common/fixtures.rs`
- Keyring round-trip tests are gated behind `JR_RUN_KEYRING_TESTS=1` + `#[ignore]` because Linux CI may lack secret-service
- OAuth integration tests in `tests/oauth_embedded_login.rs` are gated behind `JR_RUN_OAUTH_INTEGRATION=1` + `#[ignore]`. The test is currently `unimplemented!()` — it requires a wiremock base-URL override in `oauth_login` before a real assertion can be written. CI does not run it; the embedded-creds smoke test in `release.yml` covers the binary-level check instead.
- All interactive prompts have non-interactive flag equivalents for AI agent usage
- `--output json` on write operations returns structured data (e.g., `{"key": "FOO-123"}`)
- Run `scripts/check-spec-counts.sh` after any edit to .factory/specs/prd/ BC files,
  nfr-catalog.md, or holdout-scenarios.md. Exits 0 if frontmatter counts match body counts.
  Exits 1 with specific mismatch details if drift is detected (DRIFT-001 mitigation).
