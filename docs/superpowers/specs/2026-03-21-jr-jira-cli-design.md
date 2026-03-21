# jr — A Rust CLI for Jira Cloud

## Overview

`jr` is a command-line tool written in Rust for automating Jira Cloud workflows. It provides a fast, scriptable interface for the most common developer interactions with Jira: viewing and transitioning issues, creating tickets, searching with JQL, and logging time.

The tool prioritizes correctness over coverage — it handles scrum and kanban projects properly, queries workflows at runtime instead of guessing, and targets the Jira REST API v3 and Agile REST API.

## Goals

- Replace the Jira web UI for daily developer workflows
- Fast startup, minimal resource usage (Rust single binary)
- Scriptable output for automation (`--output json`)
- Correct handling of project-specific workflows and board types
- Secure credential storage via OS keychain
- Extensible structure for future Atlassian product support (Confluence, JSM, Assets)

## Non-Goals (v1)

- Terminal UI (TUI) mode — on the roadmap, not v1
- Bulk operations (create/update/transition many issues)
- Git integration (auto-branch, PR linking)
- Confluence, JSM, or Assets support
- Offline mode / operation queuing
- Dashboard or filter management

## Command Structure

### Auth & Setup

```
jr init                              # Configure Jira instance + auth
jr auth login                        # Authenticate (OAuth 2.0 or API token)
jr auth login --token                # Authenticate with API token
jr auth status                       # Show current auth state
jr me                                # Show current user info
jr project fields FOO                # List valid issue types, priorities, statuses for a project
```

### Issues

```
jr issue list                        # Smart default: my active issues
jr issue list --jql "..."            # Custom JQL query
jr issue list --project FOO          # Filter by project
jr issue list --status "In Progress" # Filter by status
jr issue list --limit N              # Cap results
jr issue create                      # Create issue (interactive prompts)
jr issue create -p FOO -t Bug -s "Title" -d "Description"
jr issue create -p FOO -t Bug -s "Title" --priority High --label backend
jr issue create -p FOO -t Story -s "Title" --team "Platform"
jr issue create -p FOO -t Bug -s "Title" --description-stdin < desc.md --markdown
jr issue view KEY-123                # View issue details
jr issue view KEY-123 --output json  # Full issue data for AI agents
jr issue move KEY-123 "In Progress"  # Transition issue to status
jr issue move KEY-123                # Transition (prompts for available statuses)
jr issue transitions KEY-123         # List available transitions without moving
jr issue edit KEY-123 --summary "New title"  # Edit issue fields
jr issue edit KEY-123 --type "Bug" --priority "High"
jr issue edit KEY-123 --team "Platform"      # Set team assignment
jr issue edit KEY-123 --label add:backend --label remove:frontend
jr issue assign KEY-123              # Assign to me
jr issue assign KEY-123 --to user    # Assign to someone else
jr issue assign KEY-123 --unassign   # Remove assignee
jr issue comment KEY-123 "message"   # Add comment (plain text)
jr issue comment KEY-123 --markdown "## Heading\n- item"  # Markdown comment
jr issue comment KEY-123 --file notes.md --markdown        # Comment from file
echo "piped text" | jr issue comment KEY-123 --stdin       # Read comment from stdin
jr issue open KEY-123                # Open in browser
jr issue open KEY-123 --url-only     # Print URL instead of opening browser
```

### Boards & Sprints

```
jr board list                        # List boards you have access to
jr board view                        # Show issues on current board
jr sprint list                       # List sprints (scrum only)
jr sprint current                    # Show current sprint issues (scrum only)
```

### Worklogs

```
jr worklog add KEY-123 2h            # Log 2 hours
jr worklog add KEY-123 1h30m -m "note"  # Log with comment
jr worklog add KEY-123 1d            # Log 1 day (= 8h, configurable)
jr worklog add KEY-123 30m           # Log 30 minutes
jr worklog list KEY-123              # View worklogs on issue
```

### Shell & Utilities

```
jr completion bash                   # Generate bash completions
jr completion zsh                    # Generate zsh completions
jr completion fish                   # Generate fish completions
```

Users install completions by redirecting output:
- Bash: `jr completion bash > /usr/share/bash-completion/completions/jr`
- Zsh: `jr completion zsh > ~/.zfunc/_jr`
- Fish: `jr completion fish > ~/.config/fish/completions/jr.fish`

### Global Flags

All commands support:

- `--output json|table` — output format (default: table)
- `--project FOO` — override project key (does not affect board-dependent smart defaults; use `--jql` for cross-project queries)
- `--no-color` — disable colored output (also respects `NO_COLOR` env var)
- `--no-input` — disable all interactive prompts; fail with an error listing required flags instead (auto-enabled when stdin is not a TTY)
- `--verbose` — debug-level detail (full request/response)

## Smart Defaults: Scrum vs Kanban

`jr issue list` (with no flags) behaves differently based on board type:

- **Scrum board:** Shows issues assigned to me in the current active sprint
- **Kanban board:** Shows issues assigned to me that are not in a "Done" category status

Board type is auto-detected during `jr init` and stored in per-project config. Sprint commands (`jr sprint list`, `jr sprint current`) return a clear error on kanban projects.

**API call sequence for smart defaults:**

1. Read `board_id` from `.jr.toml`
2. `GET /rest/agile/1.0/board/{boardId}/configuration` — determine board type
3. **Scrum path:** `GET /rest/agile/1.0/board/{boardId}/sprint?state=active` → get active sprint ID → `POST /rest/api/3/search/jql` with body `{ "jql": "sprint = {sprintId} AND assignee = currentUser()" }` (the Agile sprint issue endpoint does not support JQL filtering)
4. **Kanban path:** `POST /rest/api/3/search/jql` with body `{ "jql": "assignee=currentUser() AND statusCategory != Done AND project = {projectKey}" }` (the Agile board endpoint does not support JQL filtering)

## Transitions

Each Jira project (and even each issue type) can have different workflow transitions. `jr issue move` handles this by querying available transitions at runtime via `GET /rest/api/3/issue/{key}/transitions`.

**Interactive mode** (no target status):
```
$ jr issue move KEY-123
Available transitions for KEY-123 (status: To Do):
  1. In Progress
  2. Blocked
  3. Won't Do
Select transition:
```

**Direct mode** (with target status):
```
$ jr issue move KEY-123 "In Progress"
KEY-123 transitioned to "In Progress"
```

**Partial matching** — case-insensitive substring match against available transitions:
```
$ jr issue move KEY-123 prog
KEY-123 transitioned to "In Progress"
```

**Ambiguous match** — multiple matches prompts for disambiguation:
```
$ jr issue move KEY-123 "In"
Multiple transitions match "In":
  1. In Progress
  2. In Review
Select transition:
```

**Error case** — no match shows available options:
```
$ jr issue move KEY-123 "Deployed"
Error: "Deployed" is not a valid transition for KEY-123
Available transitions: In Progress, Blocked, Won't Do
```

## Team Assignment

Jira's "Team" field is a custom field (not a built-in field like assignee). It is separate from individual assignment — a ticket can be assigned to both a person and a team.

**How it works under the hood:**

1. On first use (or during `jr init`), query `GET /rest/api/3/field` to find the custom field ID for the "Team" field (e.g., `customfield_10001`). Cache this mapping in global config.
2. When `--team "Platform"` is provided, resolve the team name to its ID by querying the field's allowed values.
3. Set the custom field on create/edit via the standard issue fields payload: `{ "customfield_10001": { "id": "team-id" } }`

**Team name matching** uses the same case-insensitive partial matching as transitions — `--team plat` matches "Platform". Ambiguous matches prompt for selection.

**`jr issue view`** displays the team field when present.

**`jr issue list`** supports `--team "Platform"` to filter by team (translates to JQL: `"Team" = "Platform"`).

**Edge case:** If no "Team" custom field exists on the Jira instance, `--team` returns a clear error: `Error: No "Team" field found on this Jira instance`.

## Authentication

### OAuth 2.0 (3LO) — Primary Method

1. `jr auth login` starts a local HTTP server on a random port
2. Opens browser to Atlassian's OAuth consent screen with required scopes:
   - `read:jira-work` — read issues, boards, sprints
   - `write:jira-work` — create/edit issues, transitions, worklogs
   - `read:jira-user` — read user info for assignments
   - `offline_access` — obtain refresh token for persistent sessions
3. User approves; Atlassian redirects to localhost with auth code
4. CLI exchanges code for access token + refresh token
5. CLI calls `GET https://api.atlassian.com/oauth/token/accessible-resources` to resolve the `cloudId` for the Jira instance
6. All subsequent API calls use `https://api.atlassian.com/ex/jira/{cloudId}/rest/api/3/...`
7. Tokens stored in OS keychain (macOS Keychain via `keyring` crate)

**OAuth app credentials:** The CLI ships with an embedded OAuth `client_id` and `client_secret`. Atlassian's OAuth 2.0 (3LO) requires a `client_secret` for the token exchange — there is no public client / PKCE flow. The embedded secret is not truly confidential (it can be extracted from the binary), but this is standard practice for CLI tools (GitHub CLI, Slack CLI, etc.). The secret controls which app is making requests, not user authorization — user consent via the browser is the real security boundary. Users do not need to register their own OAuth app.

### API Token — Fallback Method

1. `jr auth login --token` prompts for email + API token
2. Credentials stored in OS keychain
3. API calls use `https://{instance}.atlassian.net/rest/api/3/...` directly (no cloudId needed)

### Token Lifecycle

- Access token validity checked before each request
- Auto-refresh on 401 response using refresh token
- **Refresh token rotation:** Atlassian returns a new refresh token on each refresh — the new token must be stored, replacing the old one
- If refresh fails, prompt user to re-authenticate
- `jr auth status` shows: auth method, user email, token expiry, connected instance

### Credential Storage

Credentials are **never** stored in config files. All secrets go to OS keychain via the `keyring` crate:

- macOS: Keychain
- Linux: Secret Service (GNOME Keyring / KWallet)
- Windows: Credential Manager

## Configuration

### Global Config

Located at `~/.config/jr/config.toml`:

```toml
[instance]
url = "https://yourorg.atlassian.net"
cloud_id = "abc123-def456"  # Auto-populated during OAuth login
auth_method = "oauth"  # or "api_token"

[defaults]
output = "table"
```

### Per-Project Config

Located at `.jr.toml` in the repository root:

```toml
project = "FOO"
board_id = 42
```

Board type is auto-detected from the Jira API and does not need to be specified.

### Config Resolution

Per-project config overrides global config. The `--project` flag overrides both. Config is loaded via `figment` which supports TOML files + environment variables.

## API Client

### HTTP Client

A single `JiraClient` struct wrapping `reqwest::Client`. Handles:

- Base URL construction from configured instance
- Auth header injection (Bearer token or Basic auth)
- Content-Type headers
- Request/response logging (when `--verbose`)

### Pagination

Most Jira REST API v3 endpoints use offset-based pagination (`startAt` + `maxResults`). Some newer endpoints (notably the JQL search endpoint `POST /rest/api/3/search/jql`) support cursor-based pagination (`nextPageToken`).

The pagination module supports both strategies:

- **Offset-based** (default): Used for issues, comments, worklogs, and most list endpoints. Iterates by incrementing `startAt` by `maxResults` until `total` is reached.
- **Cursor-based**: Used where supported (e.g., JQL search via `POST /rest/api/3/search/jql`). Iterates using `nextPageToken` until the field is absent from the response.

Auto-paginates by default; `--limit N` caps results.

### Rate Limiting

- Reads `X-RateLimit-Remaining` header on all responses for awareness
- On 429 response: reads `Retry-After` header (preferred) or `X-RateLimit-Reset` to determine wait time, retries automatically (up to 3 retries)
- Shows progress indicator during waits

### Error Handling

Errors always suggest what to do next. No stack traces in default output.

```
# Auth errors
Error: Not authenticated. Run "jr auth login" to connect.

# Permission errors
Error: You don't have permission to transition KEY-123

# Network errors
Error: Could not reach yourorg.atlassian.net — check your connection

# Invalid input
Error: "InvalidType" is not a valid issue type for project FOO
Available types: Bug, Story, Task, Epic
```

### Exit Codes

Standardized exit codes for scripting:

| Code | Meaning | When |
|------|---------|------|
| 0 | Success | Operation completed |
| 1 | General error | API errors, permission errors, unknown failures |
| 2 | Auth error | Not authenticated, token expired, refresh failed |
| 64 | Usage error | Invalid arguments, missing required fields |
| 78 | Config error | No instance configured, invalid config file |
| 130 | Interrupted | User pressed Ctrl+C |

### Graceful Shutdown (Ctrl+C)

When the user presses Ctrl+C, `jr` catches SIGINT via `tokio::signal::ctrl_c()` and:

1. Cancels any in-flight HTTP requests (reqwest futures are dropped)
2. Prints a brief cancellation message to stderr
3. Exits with code 130

This prevents partial writes or corrupted state from interrupted API calls.

## AI Agent & Scripting Friendliness

`jr` is designed to be usable by AI coding agents (Claude Code, Cursor, etc.) and shell scripts, not just humans at a terminal.

### Non-Interactive Mode

When stdin is not a TTY, or when `--no-input` is passed, all interactive prompts are disabled. Commands that would normally prompt for input instead fail with a clear error listing the required flags:

```
$ jr issue create --no-input
Error: Missing required flags for non-interactive mode:
  -p/--project   Project key
  -t/--type      Issue type
  -s/--summary   Issue summary
```

This applies to: `jr issue create`, `jr issue move` (when no status given), `jr init`, and `jr auth login`.

### Structured Error Output

When `--output json` is set, errors are also written as JSON to stderr:

```json
{"error": "Not authenticated. Run \"jr auth login\" to connect.", "code": 2}
```

This allows AI agents to parse errors programmatically instead of regex-matching human-readable text.

### No Surprise Side Effects

Commands never open a browser, editor, or pager unless explicitly requested:

- `jr auth login` — prints the OAuth URL to stdout and waits; use `--open` to also open the browser
- `jr issue open KEY-123` — opens browser by default; use `--url-only` to print the URL instead
- All output goes directly to stdout/stderr, never to a pager

### Stdin Support

Commands that accept text content support `--stdin` to read from a pipe:

```bash
echo "Fixed the bug" | jr issue comment KEY-123 --stdin
cat description.md | jr issue create -p FOO -t Bug -s "Title" --description-stdin --markdown
```

### Idempotent Operations

State-changing commands handle already-in-target-state gracefully:

- `jr issue move KEY-123 "Done"` — if already "Done", prints a message and exits 0 (not an error)
- `jr issue assign KEY-123` — if already assigned to me, prints a message and exits 0

This makes it safe for AI agents and scripts to retry commands without error-handling the "already done" case.

### Structured Success Output

When `--output json` is set, write operations return structured JSON to stdout (not human text):

```bash
$ jr issue create -p FOO -t Bug -s "Auth bug" --output json
{"key": "FOO-456"}

$ jr issue move FOO-456 "In Progress" --output json
{"key": "FOO-456", "status": "In Progress"}

$ jr issue comment FOO-456 "Fixed it" --output json
{"key": "FOO-456", "comment_id": "12345"}
```

This allows AI agents to capture created keys and IDs for follow-up commands.

### Rich View for Single-Command Context

`jr issue view KEY-123 --output json` returns all issue data in a single call — summary, description, status, assignee, priority, project, labels, team, comments (with timestamps and authors), and available transitions. An AI agent can understand the full state of an issue from one command without chaining multiple calls.

### Project Field Discovery

`jr project fields FOO --output json` returns valid issue types, priorities, and statuses for a project. AI agents use this to know what values are valid before creating or editing issues, avoiding trial-and-error guessing.

### Querying Transitions Without Moving

`jr issue transitions KEY-123 --output json` returns the list of available transitions for an issue without performing any transition. AI agents use this to check whether a target status is reachable before attempting `jr issue move`.

## Worklog Time Format

Time durations are parsed from a human-friendly format and converted to seconds for the Jira API:

| Input | Meaning | Seconds |
|-------|---------|---------|
| `30m` | 30 minutes | 1800 |
| `2h` | 2 hours | 7200 |
| `1h30m` | 1 hour 30 minutes | 5400 |
| `1d` | 1 day (default: 8 hours) | 28800 |
| `1w` | 1 week (default: 5 days) | 144000 |

The hours-per-day (default 8) and days-per-week (default 5) match Jira's default time tracking settings. These can be overridden in global config if the user's Jira instance uses different values.

Decimal values are not supported — use `1h30m` instead of `1.5h`.

## Rich Text: ADF Handling

Jira Cloud uses Atlassian Document Format (ADF) for all rich text fields.

### Writing Content

- Plain text by default — sent as a simple ADF paragraph
- `--markdown` flag converts Markdown to ADF before sending
- Supported Markdown elements: headings, bold/italic, lists, code blocks, links
- `--file` flag reads content from a file

### Reading Content

- ADF converted to plain text with terminal formatting
- Preserves structure: headings, lists, code blocks
- Complex content (tables, media, embeds) simplified to `[unsupported: table]`
- `--output json` returns raw ADF for scripting

## Project Structure

```
jr/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── cli/
│   │   ├── mod.rs             # Top-level CLI definition
│   │   ├── issue.rs           # Issue subcommands
│   │   ├── board.rs           # Board subcommands
│   │   ├── sprint.rs          # Sprint subcommands
│   │   ├── worklog.rs         # Worklog subcommands
│   │   ├── auth.rs            # Auth subcommands
│   │   └── init.rs            # Init command
│   ├── api/
│   │   ├── mod.rs             # Shared client, auth, pagination, rate limiting
│   │   ├── client.rs          # Base HTTP client + auth header injection
│   │   ├── auth.rs            # OAuth flow, token refresh, keychain
│   │   ├── pagination.rs      # Offset + cursor-based pagination
│   │   ├── rate_limit.rs      # Rate limit handling + retry
│   │   └── jira/              # Jira-specific API calls
│   │       ├── mod.rs
│   │       ├── issues.rs
│   │       ├── boards.rs
│   │       ├── sprints.rs
│   │       ├── worklogs.rs
│   │       ├── transitions.rs
│   │       └── users.rs
│   ├── types/
│   │   └── jira/              # Jira-specific types
│   │       ├── mod.rs
│   │       ├── issue.rs
│   │       ├── board.rs
│   │       ├── project.rs
│   │       └── user.rs
│   ├── config.rs              # Global + per-project config loading
│   ├── output.rs              # Table/JSON formatting
│   └── adf.rs                 # Markdown to ADF, ADF to plain text
```

### Product Extensibility

The `api/` and `types/` directories are namespaced by Atlassian product. Adding Confluence, JSM, or Assets later means adding a new subdirectory:

```
├── api/
│   ├── jira/          # Already exists
│   └── confluence/    # New product
├── types/
│   ├── jira/          # Already exists
│   └── confluence/    # New product
```

Shared infrastructure (`client.rs`, `auth.rs`, `pagination.rs`, `rate_limit.rs`) stays at the `api/` root since all Atlassian Cloud products share the same auth and base URL pattern.

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | 4.x (derive) | CLI argument parsing |
| `clap_complete` | 4.x | Shell completion generation (bash, zsh, fish) |
| `reqwest` | 0.12.x (rustls-tls) | HTTP client (explicit rustls to avoid TLS version issues) |
| `tokio` | 1.x | Async runtime |
| `serde` + `serde_json` | 1.x | JSON serialization |
| `keyring` | 3.x | OS credential storage |
| `figment` | 0.10.x | Layered config (TOML + env vars) |
| `comfy-table` | 7.x | Table output |
| `colored` | 2.x | Terminal colors |
| `dialoguer` | 0.12.x | Interactive prompts |
| `anyhow` | 1.x | Application-level error handling |
| `thiserror` | 2.x | Typed error derivation |
| `base64` | 0.22.x | Base64 encoding for Basic auth |
| `urlencoding` | 2.x | URL encoding for query parameters |
| `toml` | 0.8.x | TOML serialization for saving config |
| `chrono` | 0.4.x | Datetime handling for token expiry and worklogs |
| `open` | 5.x | Open URLs in browser |
| `dirs` | 5.x | XDG config paths |

### Dev Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `wiremock` | 0.6.x | Async HTTP mocking for tests |
| `assert_cmd` | 2.x | CLI binary integration testing |
| `predicates` | 3.x | Test assertion predicates |
| `tempfile` | 3.x | Temporary files/dirs for config tests |
| `insta` | 1.x | Snapshot testing for CLI output (table, JSON, errors) |
| `proptest` | 1.x | Property-based testing for parsers and matching |

## Release Profile

Optimized binary size for distribution:

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

This produces a ~5-10 MB binary with full optimization, stripped symbols, and thin LTO. The `panic = "abort"` setting eliminates unwinding overhead since CLI errors are handled via `Result`, not panics.

## Testing Strategy

### Test Layers

1. **Unit tests** (inline `#[cfg(test)]`): Pure logic — config parsing, duration parsing, ADF conversion, partial matching. Fast, no I/O.
2. **API integration tests** (`tests/`): `JiraClient` against wiremock mock servers. Tests pagination, rate limiting, error handling, auth headers.
3. **CLI integration tests** (`tests/`): Full binary via `assert_cmd`. Tests end-to-end command behavior, output formats, exit codes.
4. **Property tests** (inline): `proptest` for duration parsing edge cases and partial match correctness.
5. **Snapshot tests** (inline + `tests/`): `insta` for table output and error message formatting.

### Test Infrastructure

```
tests/
├── common/
│   ├── mod.rs              # Shared test utilities
│   ├── fixtures.rs         # Reusable mock JSON responses + builders
│   └── mock_server.rs      # Helper to create configured wiremock server
├── api_client.rs           # JiraClient integration tests
├── issue_commands.rs       # Issue command integration tests
├── board_sprint_commands.rs
└── worklog_commands.rs
```

### Config Injection for Tests

The `JR_BASE_URL` environment variable overrides the configured instance URL, allowing integration tests to point at a wiremock server without writing config files. The `JiraClient::new_for_test()` constructor accepts an explicit base URL and auth header for API-level tests.

### What Not to Test

- External crate behavior (reqwest, clap, serde, dialoguer)
- Private implementation details
- Exact log/debug output formatting
- Time-dependent behavior without mocking time

## Distribution

- **Cargo:** `cargo install jr-cli` (the crate name `jr` is taken on crates.io; the binary is still named `jr`)
- **Homebrew:** `brew install zious11/tap/jr` via a custom tap (formula downloads pre-built binaries)
- **GitHub Releases:** Pre-built binaries for macOS (arm64, x86_64), Linux (x86_64, arm64)

## GitHub Environment Setup

### Repository Structure

```
.github/
├── workflows/
│   ├── ci.yml              # Test, clippy, fmt on every PR and push
│   └── release.yml         # Build + publish on version tags (v*)
├── dependabot.yml          # Automated dependency updates
deny.toml                   # cargo-deny license/vulnerability policy
rust-toolchain.toml         # Pin Rust version for consistent builds
```

### CI Workflow (`ci.yml`)

Triggers on every push and PR. Runs in parallel:

1. **Format check:** `cargo fmt --all -- --check`
2. **Lint:** `cargo clippy --all --all-features --tests -- -D warnings`
3. **Test:** `cargo test --all-features`
4. **MSRV check:** Test against the declared minimum Rust version (1.85.0)
5. **Security audit:** `cargo deny check` for license compliance and known vulnerabilities
6. **Code coverage:** `cargo llvm-cov` with results uploaded to Codecov

### Release Workflow (`release.yml`)

Triggers on version tags (`v*`). Uses `cargo-dist` or a custom matrix strategy:

1. **Build matrix:** Cross-compile for all targets:
   - `x86_64-apple-darwin` (macOS Intel)
   - `aarch64-apple-darwin` (macOS Apple Silicon)
   - `x86_64-unknown-linux-gnu` (Linux x86_64)
   - `aarch64-unknown-linux-gnu` (Linux ARM64, via `cross-rs`)
2. **Package:** `.tar.gz` archives with SHA256 checksums
3. **Publish:** Create GitHub Release with all binaries attached
4. **Homebrew:** Auto-update formula in `zious11/homebrew-tap` with new version + checksums

### Cross-Compilation

Linux ARM64 builds use `cross-rs` which provides Docker-based cross-compilation environments. Native compilation for macOS (both architectures) and Linux x86_64 using GitHub-hosted runners.

### reqwest TLS Configuration

reqwest must use the `rustls-tls` feature explicitly (not `default-tls`) to avoid TLS version negotiation issues with Jira Cloud's servers. In `Cargo.toml`:

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

### Dependency Management

- **Dependabot:** Weekly checks for Cargo dependency updates
- **cargo-deny:** Enforces license allowlist (MIT, Apache-2.0, BSD) and blocks known vulnerabilities via RustSec advisory database

### Branch Protection

- Require PR reviews before merging to `main`
- Require all CI status checks to pass
- No force-pushes to `main`

### MSRV Policy

Minimum Supported Rust Version: **1.85.0** (or latest stable minus 3 releases). MSRV bumps increment the minor version. Tested in CI on every PR.

## Roadmap (Post-v1)

- **TUI mode** — interactive terminal UI with keyboard navigation
- **Bulk operations** — create/update/transition many issues from CSV or JQL
- **Git integration** — auto-create branches from tickets, auto-transition on PR events
- **Confluence support** — page CRUD, search
- **JSM support** — service desk queues, request management
- **Assets support** — CMDB queries
- **Offline mode** — queue operations when disconnected, sync when back online
