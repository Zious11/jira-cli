# jr

[![CI](https://github.com/Zious11/jira-cli/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/Zious11/jira-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Zious11/jira-cli?label=release)](https://github.com/Zious11/jira-cli/releases/latest)
[![Pre-release](https://img.shields.io/github/v/release/Zious11/jira-cli?include_prereleases&label=dev)](https://github.com/Zious11/jira-cli/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-orange.svg)](https://blog.rust-lang.org/)
[![codecov](https://codecov.io/gh/Zious11/jira-cli/branch/develop/graph/badge.svg)](https://codecov.io/gh/Zious11/jira-cli)

A fast, agent-friendly CLI for Jira Cloud, written in Rust. Built for both humans and AI agents — commands support structured JSON output, actionable error messages with suggested next steps, and `--no-input` mode for fully non-interactive automation.

## Why jr?

- **Fast** — native Rust binary, no JVM or Node runtime
- **Agent-friendly** — structured JSON output, non-interactive mode, idempotent operations, actionable error messages with exit codes
- **Smart defaults** — auto-discovers scrum boards, story points fields, and CMDB asset fields during `jr init`
- **Composable filters** — chain `--assignee`, `--status`, `--team`, `--asset`, `--open`, `--recent` on `issue list`
- **Assets/CMDB support** — search assets, view linked tickets, filter by asset on issues, enriched JSON output
- **Partial matching** — type `jr issue move KEY "prog"` and it matches "In Progress"
- **JSM queues** — list and view JSM service desk queues
- **Shell completions** — bash, zsh, fish

## Install

### One-liner (macOS, Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/Zious11/jira-cli/main/install.sh | sh
```

To install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/Zious11/jira-cli/main/install.sh | sh -s -- v0.3.0
```

### From source

```bash
brew install rust   # if you don't have Rust installed
git clone https://github.com/Zious11/jira-cli.git
cd jira-cli
cargo install --path .
```

### Coming soon

```bash
# Homebrew tap (planned)
brew install zious11/tap/jr

# Crates.io (planned)
cargo install jr-cli
```

## macOS: after upgrading the binary

When `jr` is replaced at its installed path — via `brew upgrade`, manual
`cp`, or `curl | tar` — macOS's legacy Keychain Services treats the new
binary as a different application and can prompt up to 4 times per
command indefinitely.

Fix:

```bash
jr auth refresh
```

This clears the stored credentials and re-runs the login flow so the
new binary becomes the creator of fresh Keychain entries. **Click
"Always Allow"** on the two prompts macOS shows during re-store —
otherwise future commands will prompt again.

Tracked in [#207](https://github.com/Zious11/jira-cli/issues/207). A
longer-term fix (Developer ID signing) is tracked as a separate issue.

## Quick Start

```bash
# Set up your Jira instance and authenticate
jr init

# Authenticate with API token (default)
jr auth login

# Or authenticate with OAuth 2.0 (requires your own OAuth app)
jr auth login --oauth

# View your current sprint/board issues
jr issue list --project FOO

# Sprint list (auto-discovers scrum board for project)
jr sprint list --project FOO

# My assigned tickets
jr issue list --assignee me

# Tickets I reported in the last 7 days
jr issue list --reporter me --recent 7d

# Open issues assigned to me (excludes Done)
jr issue list --assignee me --open

# Issues in a specific status
jr issue list --project FOO --status "In Progress"

# Issues linked to a specific asset
jr issue list --project FOO --asset CUST-5 --open

# Open tickets for an asset (quick lookup)
jr assets tickets CUST-5 --open

# Discover available projects
jr project list

# View a specific issue
jr issue view KEY-123

# Create an issue
jr issue create --project FOO --type Bug --summary "Auth token not refreshing" --priority High --points 5

# Transition an issue
jr issue move KEY-123 "In Progress"

# Log time
jr worklog add KEY-123 2h -m "Fixed the auth bug"

# Add a comment
jr issue comment KEY-123 "Deployed to staging"
```

## Commands

| Command | Description |
|---------|-------------|
| `jr init` | Configure Jira instance and authenticate |
| `jr auth login` | Authenticate with API token (default) or `--oauth` for OAuth 2.0 |
| `jr auth status` | Show authentication status |
| `jr me` | Show current user info |
| `jr issue list` | List issues (`--assignee`, `--reporter`, `--recent`, `--status`, `--open`, `--team`, `--asset KEY`, `--jql`, `--limit`/`--all`, `--points`, `--assets`) |
| `jr issue view KEY` | View issue details (per-field asset rows, enriched JSON, story points) |
| `jr issue create` | Create an issue (`--team`, `--points`) |
| `jr issue edit KEY` | Edit issue fields (`--team`, `--points`, `--no-points`) |
| `jr issue move KEY [STATUS]` | Transition issue (partial match on status name) |
| `jr issue transitions KEY` | List available transitions |
| `jr issue assign KEY` | Assign to self (or `--to USER`, `--unassign`) |
| `jr issue comment KEY "msg"` | Add a comment (`--stdin`, `--file`, `--markdown`) |
| `jr issue comments KEY` | List comments (`--limit N`) |
| `jr issue open KEY` | Open in browser (`--url-only` for scripts) |
| `jr issue link KEY1 KEY2` | Link two issues (`--type blocks`, defaults to Relates) |
| `jr issue unlink KEY1 KEY2` | Remove link(s) between issues (`--type` to filter) |
| `jr issue link-types` | List available link types |
| `jr issue assets KEY`          | Show assets linked to an issue                |
| `jr board list` | List boards (`--project`, `--type scrum\|kanban`) |
| `jr board view --board 42` | Show current board issues (`--board` or config, `--limit`/`--all`) |
| `jr sprint list --board 42` | List sprints (`--board` or config or auto-discover, scrum only) |
| `jr sprint current --board 42` | Show current sprint issues (with points summary) |
| `jr sprint add --sprint 100 KEY...` | Add issues to a sprint (`--current` for active sprint) |
| `jr sprint remove KEY...` | Move issues to backlog (removes from all sprints) |
| `jr worklog add KEY 2h` | Log time (`1h30m`, `1d`, `1w`) |
| `jr worklog list KEY` | List worklogs |
| `jr queue list`                  | List JSM queues for the project's service desk |
| `jr queue view <name>`           | View issues in a queue (partial name match)    |
| `jr assets search <AQL>`        | Search assets via AQL query (`--attributes` resolves names) |
| `jr assets view <key>`          | View asset details (key or numeric ID)         |
| `jr assets tickets <key>`       | Show Jira issues connected to an asset (`--open`, `--status`, `--limit`) |
| `jr assets schemas`             | List object schemas in the workspace           |
| `jr assets types [--schema]`    | List object types (all or filtered by schema)  |
| `jr assets schema <TYPE>`       | Show attributes for an object type (partial match) |
| `jr team list` | List available teams (`--refresh` to force update) |
| `jr user search <query>` | Search users by display name or email (`--limit`/`--all`) |
| `jr user list --project FOO` | List users assignable to a project (`--limit`/`--all`) |
| `jr user view <accountId>` | Look up a single user by accountId |
| `jr project list` | List accessible projects (`--type`, `--limit`/`--all`) |
| `jr project fields --project FOO` | Show valid issue types, priorities, statuses, and asset custom fields |
| `jr completion bash\|zsh\|fish` | Generate shell completions |

## Global Flags

| Flag | Description |
|------|-------------|
| `--output json\|table` | Output format (default: table) |
| `--project FOO` | Override project key |
| `--no-color` | Disable colored output (also respects `NO_COLOR` env) |
| `--no-input` | Disable interactive prompts (auto-enabled in pipes/scripts) |
| `--verbose` | Show HTTP request/response details |

## Configuration

```bash
# Global config
~/.config/jr/config.toml

# Per-project config (in your repo root)
.jr.toml

# Team cache (disposable, 7-day TTL)
~/.cache/jr/teams.json
```

**Global config:**
```toml
[instance]
url = "https://yourorg.atlassian.net"
auth_method = "api_token"  # or "oauth"

[defaults]
output = "table"

[fields]
story_points_field_id = "customfield_XXXXX"  # auto-discovered during init
```

**Per-project config:**
```toml
project = "FOO"
board_id = 42
```

## Scripting & AI Agents

`jr` is designed to be used by scripts and AI coding agents:

- `--output json` returns structured JSON for both success and errors
- `--no-input` disables all interactive prompts (auto-enabled when stdin is not a TTY)
- `--stdin` flag on comment/create reads content from pipes
- `--url-only` prints URLs instead of opening a browser
- State-changing commands are idempotent (exit 0 if already in target state)
- Structured exit codes (see [Exit Codes](#exit-codes) table)

```bash
# AI agent workflow example
jr issue view KEY-123 --output json          # Get full context
jr issue move KEY-123 "In Progress"          # Start work
echo "Fixed the bug" | jr issue comment KEY-123 --stdin  # Add comment
jr issue move KEY-123 "Done"                 # Complete
```

## Shell Completions

```bash
# Bash (add to ~/.bashrc)
eval "$(jr completion bash)"

# Zsh (add to ~/.zshrc)
eval "$(jr completion zsh)"

# Fish (add to ~/.config/fish/config.fish)
jr completion fish | source
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Authentication error |
| 64 | Usage error (bad arguments) |
| 78 | Configuration error |
| 130 | Interrupted (Ctrl+C) |

## License

MIT
