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

For CI or headless boxes, pass credentials via env vars or flags
(e.g. `JR_EMAIL="..." JR_API_TOKEN="$TOKEN" jr --no-input auth refresh`)
so the flow completes without a TTY.

Tracked in [#207](https://github.com/Zious11/jira-cli/issues/207). A
longer-term fix (Developer ID signing) is tracked as a separate issue.

## Quick Start

```bash
# Set up your Jira instance and authenticate
jr init

# Authenticate with API token (default)
jr auth login

# Non-interactive API token (CI / agents): flags or env vars, no TTY required.
# Prefer env vars for secrets — bare CLI args can leak via process lists.
JR_EMAIL="you@example.com" JR_API_TOKEN="$TOKEN" jr --no-input auth login
```

### OAuth 2.0 (recommended on official binaries)

Official `jr` releases ship with a built-in `jr` Atlassian OAuth app, so
authentication is one command:

```bash
jr auth login --oauth --profile my-site --url https://my-site.atlassian.net
```

Your browser opens, you click "Allow" on the `jr` consent screen, done.

Scopes default to Atlassian's recommended classic set; override via
`[instance].oauth_scopes` in `config.toml` — see Configuration below.

#### Bring your own OAuth app

If you're on a fork, source build, or enterprise tenant that requires its
own OAuth app, register one at
[Atlassian Developer Console](https://developer.atlassian.com/console/myapps/),
then pass `--client-id`/`--client-secret` or set
`JR_OAUTH_CLIENT_ID`/`JR_OAUTH_CLIENT_SECRET`:

```bash
JR_OAUTH_CLIENT_ID="$ID" JR_OAUTH_CLIENT_SECRET="$SECRET" \
    jr --no-input auth login --oauth
```

### Everyday commands

```bash
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

# Close a JSM ticket atomically with status + resolution (so SLAs + reports stay accurate)
jr issue move JSM-42 Done --resolution Fixed

# Log time
jr worklog add KEY-123 2h -m "Fixed the auth bug"

# Add a comment (public reply by default)
jr issue comment KEY-123 "Deployed to staging"

# Add an internal-only comment on a JSM issue (agents see it, customers don't)
jr issue comment JSM-42 "customer is on the paid plan — prioritizing" --internal
```

## Commands

| Command | Description |
|---------|-------------|
| `jr init` | Configure Jira instance and authenticate (prompts to add another profile if any are already configured) |
| `jr auth login` | Authenticate with API token (default) or `--oauth` for OAuth 2.0. `--profile NAME` targets a specific profile (creates if absent); `--url URL` sets the Jira instance URL when creating. Non-interactive: `--email`/`--token` or `JR_EMAIL`/`JR_API_TOKEN`; `--client-id`/`--client-secret` or `JR_OAUTH_CLIENT_ID`/`JR_OAUTH_CLIENT_SECRET` for OAuth |
| `jr auth switch <NAME>` | Set the default profile in `config.toml`. Errors if `NAME` doesn't exist |
| `jr auth list` | List configured profiles (table or JSON via `--output`); active profile marked with `*` |
| `jr auth status` | Show authentication status for the active profile, or `--profile NAME` for another |
| `jr auth refresh` | Refresh credentials for the active profile (or `--profile NAME`); same flags/env vars as `auth login` |
| `jr auth logout` | Clear OAuth tokens for the active profile (or `--profile NAME`); shared API token NOT touched |
| `jr auth remove <NAME>` | Permanently delete a profile (config entry + cache + per-profile OAuth tokens). Cannot remove the active profile |
| `jr me` | Show current user info |
| `jr issue list` | List issues (`--assignee`, `--reporter`, `--recent`, `--status`, `--open`, `--team`, `--asset KEY`, `--jql`, `--limit`/`--all`, `--points`, `--assets`) |
| `jr issue view KEY` | View issue details (per-field asset rows, enriched JSON, story points) |
| `jr issue create` | Create an issue (`--team`, `--points`) |
| `jr issue edit KEY` | Edit issue fields (`--team`, `--points`, `--no-points`) |
| `jr issue move KEY [STATUS]` | Transition issue (partial match on status name). `--resolution <name>` atomically sets resolution on the transition for JSM/resolution-required workflows. |
| `jr issue transitions KEY` | List available transitions |
| `jr issue resolutions` | List instance-scoped resolution values (cached 7 days; `--refresh` to bust). Discover what to pass to `--resolution` on `jr issue move`. |
| `jr issue assign KEY` | Assign to self (or `--to USER`, `--unassign`) |
| `jr issue comment KEY "msg"` | Add a comment (`--stdin`, `--file`, `--markdown`, `--internal` for JSM agent-only notes) |
| `jr issue comments KEY` | List comments (`--limit N`; JSM issues show a Visibility column: External / Internal) |
| `jr issue open KEY` | Open in browser (`--url-only` for scripts) |
| `jr issue link KEY1 KEY2` | Link two issues (`--type blocks`, defaults to Relates) |
| `jr issue unlink KEY1 KEY2` | Remove link(s) between issues (`--type` to filter) |
| `jr issue link-types` | List available link types |
| `jr issue remote-link KEY --url URL` | Attach a Confluence page or web URL (`--title` optional, defaults to URL) |
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
| `--profile NAME` | Override the active profile for this invocation (precedence: this flag > `JR_PROFILE` env > `default_profile` in config > `"default"`) |
| `--no-color` | Disable colored output (also respects `NO_COLOR` env) |
| `--no-input` | Disable interactive prompts (auto-enabled in pipes/scripts) |
| `--verbose` | Show HTTP request/response details |

## Configuration

```bash
# Global config
~/.config/jr/config.toml

# Per-project config (in your repo root)
.jr.toml

# Per-profile cache (disposable, 7-day TTL)
~/.cache/jr/v1/<profile>/teams.json
```

**Global config (multi-profile shape):**
```toml
default_profile = "default"

[profiles.default]
url = "https://yourorg.atlassian.net"
auth_method = "api_token"  # or "oauth"
# cloud_id, org_id, oauth_scopes, team_field_id, story_points_field_id
# are auto-discovered during `jr init` / `jr auth login --oauth` and
# populated here per profile.
# oauth_scopes = "read:issue:jira write:issue:jira ... offline_access"

[profiles.sandbox]
url = "https://yourorg-sandbox.atlassian.net"
auth_method = "api_token"
# Sandbox sites usually mirror production custom-field IDs, but `jr` stores
# them per profile so divergence doesn't silently corrupt cached lookups.

[defaults]
output = "table"
```

Switching between profiles:

```bash
jr auth switch sandbox          # persistent — writes default_profile in config.toml
jr --profile sandbox issue list # one-shot — overrides for this call only
JR_PROFILE=sandbox jr issue list # session-scoped (works well with direnv)
```

A single classic Atlassian API token authenticates the same user against
any Atlassian Cloud site, so `email` + `api-token` are stored once in the
OS keychain and shared by all `api_token` profiles. OAuth tokens are
cloudId-scoped and stored per profile.

**Per-project config:**
```toml
project = "FOO"
board_id = 42
```

**Migrating from single-instance configs:** the first run after upgrading
auto-migrates a legacy `[instance]`+`[fields]` config to the new
`[profiles.default]` shape (one stderr notice; idempotent). OAuth tokens
in the OS keychain lazy-migrate from flat keys (`oauth-access-token`) to
namespaced keys (`default:oauth-access-token`) on first authenticated
read. Old cache files at `~/.cache/jr/*.json` orphan harmlessly when the
new layout starts using `~/.cache/jr/v1/<profile>/`.

## Scripting & AI Agents

`jr` is designed to be used by scripts and AI coding agents:

- `--output json` returns structured JSON for both success and errors
- `--no-input` disables all interactive prompts (auto-enabled when stdin is not a TTY)
- `--stdin` flag on comment/create reads content from pipes
- `--url-only` prints URLs instead of opening a browser
- State-changing commands are idempotent (exit 0 if already in target state)
- Structured exit codes (see [Exit Codes](#exit-codes) table)
- `auth login` / `auth refresh` accept credentials via flags (`--email`, `--token`, `--client-id`, `--client-secret`) or env vars (`JR_EMAIL`, `JR_API_TOKEN`, `JR_OAUTH_CLIENT_ID`, `JR_OAUTH_CLIENT_SECRET`) — no TTY required. Prefer env vars for secrets.
- `--profile NAME` flag and `JR_PROFILE` env var let agents target a specific profile per-call without mutating the user's `default_profile`. Combined with direnv (`echo 'export JR_PROFILE=sandbox' >> .envrc`), a repo can scope all `jr` calls to a sandbox site automatically.

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
