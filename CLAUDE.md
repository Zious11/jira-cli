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
│   │   └── tickets.rs       # connected tickets
│   └── jira/            # Jira-specific API call implementations (one file per resource)
│       ├── issues.rs    # search, get, create, edit, list comments
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
├── lib.rs               # Crate root (re-exports for integration tests)
├── jql.rs               # JQL utilities: escaping, validation, asset clause builder
├── partial_match.rs     # Case-insensitive substring matching with disambiguation
└── error.rs             # JrError enum with exit codes (0/1/2/64/78/130)
```

Product-namespaced `api/jira/` and `types/jira/` so future Confluence/JSM/Assets support adds sibling directories.

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

## AI Agent Notes

- `JR_BASE_URL` env var overrides the configured Jira instance URL (used by tests to inject wiremock)
- `JR_PROFILE` env var overrides the active profile per-call (combine with direnv to scope a repo to a sandbox site)
- `--profile NAME` flag overrides `JR_PROFILE` for one invocation; precedence is flag > env > config > "default"
- `JiraClient::new_for_test(base_url, auth_header)` constructs a client for integration tests
- Test fixtures live in `tests/common/fixtures.rs`
- Keyring round-trip tests are gated behind `JR_RUN_KEYRING_TESTS=1` + `#[ignore]` because Linux CI may lack secret-service
- All interactive prompts have non-interactive flag equivalents for AI agent usage
- `--output json` on write operations returns structured data (e.g., `{"key": "FOO-123"}`)
