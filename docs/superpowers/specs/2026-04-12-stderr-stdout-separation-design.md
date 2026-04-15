# Separate Human Status Messages (stderr) from Machine Output (stdout)

> **Issue:** #134 — refactor: separate human status messages (stderr) from machine output (stdout)

## Problem

Write commands (create, edit, move, assign, comment, link, unlink, sprint add/remove, worklog add) send confirmation messages to stdout via `output::print_success`. This mixes human-facing status text with machine-parseable data, breaking scriptability. For example, `jr issue create | pbcopy` captures "Created issue FOO-123\nhttps://..." instead of structured data.

The `gh` CLI separates these streams: table mode writes everything to stderr (confirmations, URLs, status), JSON mode writes structured data to stdout. This lets `gh issue create --json url --jq .url | pbcopy` cleanly capture just the URL.

## Design

### Core Change

Change `output::print_success` from `println!` to `eprintln!`. This single-line change moves all ~20 write-command confirmation messages to stderr. The `colored` crate's `.green()` styling works identically with `eprintln!` (validated via Context7 — colored v3 handles stderr TTY detection).

### Standalone Status Messages

Five additional `println!` calls are human status messages, not data:

| File | Current `println!` | Reason for stderr |
|------|---------------------|-------------------|
| `src/cli/auth.rs:21` | `"Credentials stored in keychain."` | Login confirmation |
| `src/cli/auth.rs:28` | `"OAuth 2.0 requires your own Atlassian OAuth app."` | Setup instruction |
| `src/cli/auth.rs:29` | `"Create one at: https://developer.atlassian.com/..."` | Setup instruction |
| `src/cli/init.rs:10` | `"Setting up jr — Jira CLI\n"` | Setup banner |
| `src/cli/init.rs:69` | `"No boards found. You can configure .jr.toml manually."` | Setup guidance |

### Browse URL After Create

`src/cli/issue/create.rs:154` prints the browse URL in Table mode via `println!("{}", browse_url)`. This moves to `eprintln!` to match the `gh` convention: table mode is purely for human consumption. Scripts should use `--output json` to capture URLs. This matches `gh issue create` behavior where the URL goes to stderr in table mode (Perplexity-validated).

### What Stays on stdout

| Output | Reason |
|--------|--------|
| All `--output json` paths | Machine-parseable data — the whole point of JSON mode |
| `print_output` (table/JSON for read commands) | Data output for list, view, board, sprint, etc. |
| `auth status` output (Instance, Auth method, Credentials) | Command's primary data output, not a status message |
| `project fields` output | Data |
| `"No results found."` in `print_output` | Empty data result; scripts expect `cmd \| wc -l` to return 0 |
| `open --url-only` URL | Data output — the command's sole purpose is to emit the URL for piping |

### Stream Model

After this change, the stream model is:

| Mode | stdout | stderr |
|------|--------|--------|
| **Table — read commands** (list, view) | Table data | Warnings, verbose logs |
| **Table — write commands** (create, edit, move) | Nothing | Confirmations, URLs |
| **JSON — all commands** | Structured JSON | Warnings, verbose logs |

### Breaking Change

This is a **behavioral change** for Table-mode write commands. Any script or pipeline that captures stdout from Table-mode write commands (e.g., `jr issue create | grep "Created"`) will stop receiving that output. The migration path is `--output json`, which already exists for all write commands and was always the intended scripting interface. This is a pre-1.0 tool, and this change aligns with the documented project vision (agentic CLI in the spirit of `gh`).

The `open --url-only` command is unaffected — its `println!("{}", url)` is data output and stays on stdout.

## What Does NOT Change

- Return types, error messages, exit codes
- JSON output (all stays on stdout)
- `--no-color` / `NO_COLOR` behavior (colored crate handles stderr identically)
- Read command output (list, view, board list, queue list, etc.)
- `print_output` function behavior
- `print_error` function (already uses `eprintln!`)

## Testing

**Existing tests:** All handler tests use `--output json` and assert on `.stdout()`. Since JSON paths are unaffected, no existing tests need modification.

**1 new handler test** in `tests/cli_handler.rs`:

| Test | Setup | Assertion |
|------|-------|-----------|
| `test_create_table_mode_outputs_to_stderr` | wiremock returns 201 for issue create | Table-mode confirmation + URL on `.stderr()`, stdout is empty |

Uses existing `jr_cmd` helper with no `--output json` flag, matching established handler test patterns.

## Files Changed

| File | Change |
|------|--------|
| `src/output.rs` | `print_success`: `println!` → `eprintln!` |
| `src/cli/auth.rs` | 3 `println!` → `eprintln!` (lines 21, 28, 29) |
| `src/cli/init.rs` | 2 `println!` → `eprintln!` (lines 10, 69) |
| `src/cli/issue/create.rs` | 1 `println!` → `eprintln!` (line 154, browse URL) |
| `tests/cli_handler.rs` | Add `test_create_table_mode_outputs_to_stderr` |

## Validation

- **Perplexity:** Confirmed `gh issue create` writes everything to stderr in table mode; URL is only on stdout via `--json`
- **Perplexity:** Confirmed clig.dev convention — stdout for data, stderr for messaging/logs/status
- **Perplexity:** Confirmed `auth status`-style config display belongs on stdout (primary command output)
- **Context7 (colored):** `eprintln!` with `.green()` works identically to `println!`; no special handling needed
- **Context7 (comfy-table):** `Table::to_string()` returns `String` — no stream interaction; `print_output` controls the stream
- **Perplexity:** Identified edge cases for stdout→stderr migration (breaking scripts, CI log capture); mitigated by pre-1.0 status and existing `--output json` scripting path
- **Local verification:** All `println!` calls in write-command Table-mode paths are either `print_success` (covered by core change) or JSON-mode data (stays stdout); no gaps in change list
