# jr — Jira CLI

A Rust CLI tool for automating Jira Cloud workflows. Binary name: `jr`, crate name: `jr-cli`.

## Architecture

Single-crate thin client wrapping Jira REST API v3 and Agile REST API directly with reqwest. No generated client or intermediate abstraction layer.

```
src/
├── main.rs              # Entry point, tokio runtime, clap dispatch, Ctrl+C handling
├── cli/                 # Clap derive definitions + command handlers
│   ├── mod.rs           # CLI enums, global flags (--output, --project, --no-input, --no-color)
│   ├── issue/           # issue commands (split by operation theme)
│   │   ├── mod.rs       # dispatch + re-exports
│   │   ├── format.rs    # row formatting, headers, points display
│   │   ├── list.rs      # list + view + comments (read operations, unified JQL composition)
│   │   ├── create.rs    # create + edit (field-building)
│   │   ├── workflow.rs  # move + transitions + assign + comment + open
│   │   ├── links.rs     # link + unlink + link-types
│   │   ├── helpers.rs   # team/points resolution, user resolution, prompts
│   │   └── assets.rs    # linked assets (issue→asset lookup)
│   ├── assets.rs        # assets search/view/tickets
│   ├── board.rs         # board list/view
│   ├── sprint.rs        # sprint list/current (scrum-only, errors on kanban)
│   ├── worklog.rs       # worklog add/list
│   ├── team.rs          # team list (with cache + lazy org discovery)
│   ├── auth.rs          # auth login (API token default, --oauth for OAuth 2.0), auth status
│   ├── init.rs          # Interactive setup (prefetches org metadata + team cache + story points field)
│   ├── project.rs       # project fields (issue types, priorities for a project)
│   └── queue.rs         # queue list/view (JSM service desks)
├── api/
│   ├── client.rs        # JiraClient — HTTP methods, auth headers, rate limit retry, 429/401 handling
│   ├── auth.rs          # OAuth 2.0 flow, API token storage, keychain read/write, token refresh
│   ├── pagination.rs    # Offset-based (most endpoints) + cursor-based (JQL search)
│   ├── rate_limit.rs    # Retry-After parsing
│   ├── assets/          # Assets/CMDB API call implementations
│   │   ├── workspace.rs     # workspace ID discovery + cache
│   │   ├── linked.rs        # CMDB field discovery cache, adaptive parsing, enrichment
│   │   ├── objects.rs       # AQL search, get object, resolve key
│   │   └── tickets.rs       # connected tickets
│   └── jira/            # Jira-specific API call implementations (one file per resource)
│       ├── issues.rs    # search, get, create, edit, list comments
│       ├── boards.rs    # list boards, get board config
│       ├── sprints.rs   # list sprints, get sprint issues
│       ├── fields.rs    # list fields, story points field discovery
│       ├── links.rs     # create/delete issue links, list link types
│       ├── teams.rs     # org metadata (GraphQL), list teams
│       ├── worklogs.rs  # add/list worklogs
│       ├── projects.rs  # project details
│       └── users.rs     # current user, user search, assignable users
│   ├── jsm/             # JSM-specific API call implementations
│   │   ├── servicedesks.rs  # list service desks, project meta orchestration
│   │   └── queues.rs        # list queues, get queue issues
├── types/assets/        # Serde structs for Assets API responses (AssetObject, ConnectedTicket, LinkedAsset, etc.)
├── types/jira/          # Serde structs for API responses (Issue, Board, Sprint, User, Team, etc.)
├── types/jsm/           # Serde structs for JSM API responses (ServiceDesk, Queue, etc.)
├── cache.rs             # XDG cache (~/.cache/jr/) — team list, project meta, workspace ID (all 7-day TTL)
├── config.rs            # Global (~/.config/jr/config.toml) + per-project (.jr.toml), figment layering
├── output.rs            # Table (comfy-table) and JSON formatting
├── adf.rs               # Atlassian Document Format: text→ADF, markdown→ADF, ADF→text
├── duration.rs          # Worklog duration parser (2h, 1h30m, 1d, 1w)
├── jql.rs               # JQL utilities: escape_value, strip_order_by, validate_duration
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
- ADR-0002: OAuth 2.0 auth approach (superseded — no embedded secrets, user-provided OAuth credentials)
- ADR-0003: reqwest with rustls-tls
- ADR-0004: Per-feature specs, not a growing master spec
- ADR-0005: GraphQL hostNames for org discovery (team support)

## Specs & Plans

- **v1 design spec:** `docs/superpowers/specs/2026-03-21-jr-jira-cli-design.md`
- **v1 implementation plan:** `docs/superpowers/plans/2026-03-21-jr-implementation.md`
- **Feature specs (post-v1):** `docs/specs/{feature-name}.md`
- **Team assignment spec:** `docs/specs/team-assignment.md`

When adding a new feature:
1. Read this file
2. Read the v1 design spec for architectural context
3. Read relevant ADRs
4. Create a feature spec in `docs/specs/` before implementing
5. Follow TDD — write tests first

## AI Agent Notes

- `JR_BASE_URL` env var overrides the configured Jira instance URL (used by tests to inject wiremock)
- `JiraClient::new_for_test(base_url, auth_header)` constructs a client for integration tests
- Test fixtures live in `tests/common/fixtures.rs`
- All interactive prompts have non-interactive flag equivalents for AI agent usage
- `--output json` on write operations returns structured data (e.g., `{"key": "FOO-123"}`)
