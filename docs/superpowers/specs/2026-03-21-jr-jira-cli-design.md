# jr — A Rust CLI for Jira Cloud

## Overview

`jr` is a command-line tool written in Rust for automating Jira Cloud workflows. It provides a fast, scriptable interface for the most common developer interactions with Jira: viewing and transitioning issues, creating tickets, searching with JQL, and logging time.

The tool prioritizes correctness over coverage — it handles scrum and kanban projects properly, queries workflows at runtime instead of guessing, and uses Jira's current API (v3 with cursor-based pagination) rather than deprecated endpoints.

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
jr issue view KEY-123                # View issue details
jr issue move KEY-123 "In Progress"  # Transition issue to status
jr issue move KEY-123                # Transition (prompts for available statuses)
jr issue assign KEY-123              # Assign to me
jr issue assign KEY-123 --to user    # Assign to someone else
jr issue comment KEY-123 "message"   # Add comment (plain text)
jr issue comment KEY-123 --markdown "## Heading\n- item"  # Markdown comment
jr issue comment KEY-123 --file notes.md --markdown        # Comment from file
jr issue open KEY-123                # Open in browser
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
jr worklog list KEY-123              # View worklogs on issue
```

### Global Flags

All commands support:

- `--output json|table` — output format (default: table)
- `--project FOO` — override per-project config
- `--no-color` — disable colored output (also respects `NO_COLOR` env var)
- `--verbose` — debug-level detail (full request/response)

## Smart Defaults: Scrum vs Kanban

`jr issue list` (with no flags) behaves differently based on board type:

- **Scrum board:** Shows issues assigned to me in the current active sprint
- **Kanban board:** Shows issues assigned to me that are not in a "Done" category status

Board type is auto-detected during `jr init` and stored in per-project config. Sprint commands (`jr sprint list`, `jr sprint current`) return a clear error on kanban projects.

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

**Fuzzy matching** — partial input matches against available transitions:
```
$ jr issue move KEY-123 prog
KEY-123 transitioned to "In Progress"
```

**Error case** — no match shows available options:
```
$ jr issue move KEY-123 "Deployed"
Error: "Deployed" is not a valid transition for KEY-123
Available transitions: In Progress, Blocked, Won't Do
```

## Authentication

### OAuth 2.0 (3LO) — Primary Method

1. `jr auth login` starts a local HTTP server on a random port
2. Opens browser to Atlassian's OAuth consent screen
3. User approves; Atlassian redirects to localhost with auth code
4. CLI exchanges code for access token + refresh token
5. Tokens stored in OS keychain (macOS Keychain via `keyring` crate)
6. Refresh token used automatically when access token expires

### API Token — Fallback Method

1. `jr auth login --token` prompts for email + API token
2. Credentials stored in OS keychain

### Token Lifecycle

- Access token validity checked before each request
- Auto-refresh on 401 response using refresh token
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

Uses Jira's current cursor-based pagination (`nextPageToken` + `isLast`), not the deprecated `startAt` approach. Auto-paginates by default; `--limit N` caps results.

### Rate Limiting

- Reads `X-RateLimit-Remaining` and `X-RateLimit-Reset` response headers
- On 429 response: waits for reset time, retries automatically (up to 3 retries)
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
│   │   ├── pagination.rs      # Cursor-based pagination
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
| `reqwest` | 0.12.x | HTTP client |
| `tokio` | 1.x | Async runtime |
| `serde` + `serde_json` | 1.x | JSON serialization |
| `keyring` | 2.x | OS credential storage |
| `figment` | 0.10.x | Layered config (TOML + env vars) |
| `comfy-table` | 7.x | Table output |
| `colored` | 3.x | Terminal colors |
| `dialoguer` | 0.12.x | Interactive prompts |
| `anyhow` + `thiserror` | 1.x | Error handling |
| `open` | latest | Open URLs in browser |
| `dirs` | 5.x | XDG config paths |

## Distribution

- **Cargo:** `cargo install jr` (if name is available on crates.io)
- **Homebrew:** `brew install zious11/tap/jr` via a custom tap
- **GitHub Releases:** Pre-built binaries for macOS (arm64, x86_64) and Linux (x86_64)

## Roadmap (Post-v1)

- **TUI mode** — interactive terminal UI with keyboard navigation
- **Bulk operations** — create/update/transition many issues from CSV or JQL
- **Git integration** — auto-create branches from tickets, auto-transition on PR events
- **Confluence support** — page CRUD, search
- **JSM support** — service desk queues, request management
- **Assets support** — CMDB queries
- **Offline mode** — queue operations when disconnected, sync when back online
