# jr

[![CI](https://github.com/Zious11/jira-cli/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/Zious11/jira-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Zious11/jira-cli?label=release)](https://github.com/Zious11/jira-cli/releases/latest)
[![Pre-release](https://img.shields.io/github/v/release/Zious11/jira-cli?include_prereleases&label=dev)](https://github.com/Zious11/jira-cli/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-orange.svg)](https://blog.rust-lang.org/)
[![codecov](https://codecov.io/gh/Zious11/jira-cli/branch/develop/graph/badge.svg)](https://codecov.io/gh/Zious11/jira-cli)

A fast, agent-friendly CLI for Jira Cloud, written in Rust. Built for both humans and AI agents — commands support structured JSON output, actionable error messages with suggested next steps, and `--no-input` mode for fully non-interactive automation.

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

# My assigned tickets
jr issue list --assignee me

# Tickets I reported in the last 7 days
jr issue list --reporter me --recent 7d

# Open issues assigned to me (excludes Done)
jr issue list --assignee me --open

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
| `jr issue list` | List issues (`--assignee`, `--reporter`, `--recent`, `--status`, `--open`, `--team`, `--jql`, `--limit`/`--all`, `--points`, `--assets`) |
| `jr issue view KEY` | View issue details (includes story points, linked assets) |
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
| `jr board list` | List boards |
| `jr board view` | Show current board issues |
| `jr sprint list` | List sprints (scrum only) |
| `jr sprint current` | Show current sprint issues (with points summary) |
| `jr worklog add KEY 2h` | Log time (`1h30m`, `1d`, `1w`) |
| `jr worklog list KEY` | List worklogs |
| `jr queue list`                  | List JSM queues for the project's service desk |
| `jr queue view <name>`           | View issues in a queue (partial name match)    |
| `jr assets search <AQL>`        | Search assets via AQL query                    |
| `jr assets view <key>`          | View asset details (key or numeric ID)         |
| `jr assets tickets <key>`       | Show Jira issues connected to an asset         |
| `jr team list` | List available teams (`--refresh` to force update) |
| `jr project fields FOO` | Show valid issue types and priorities |
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
- Exit codes: 0=success, 1=error, 2=auth, 64=usage, 78=config, 130=interrupted

```bash
# AI agent workflow example
jr issue view KEY-123 --output json          # Get full context
jr issue move KEY-123 "In Progress"          # Start work
echo "Fixed the bug" | jr issue comment KEY-123 --stdin  # Add comment
jr issue move KEY-123 "Done"                 # Complete
```

## License

MIT
