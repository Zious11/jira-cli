# JrError Exit Codes — Design Spec

## Problem

Several commands guard against missing config or missing user input with `anyhow::anyhow!()` instead of `JrError` variants. Since `main.rs` uses `downcast_ref::<JrError>()` to map exit codes, these fall through to exit code 1 (generic error) instead of the semantically correct exit code.

This makes it impossible for scripts and AI agents to distinguish "missing config" (fixable by running `jr init`) from "runtime error" (transient failure).

**Issue:** #30

## Solution

Replace `anyhow::anyhow!(...)` with the appropriate `JrError` variant at 13 locations. Also change the `ConfigError` display format from `"Configuration error: {0}"` to `"{0}"` so error messages remain identical. No new variants, no new types.

### ConfigError Display Format Change

The current `ConfigError` variant has `#[error("Configuration error: {0}")]`, which prepends "Configuration error: " to the message. To preserve existing error message text exactly, change the format to `#[error("{0}")]` — matching `UserError`'s format. The error is already semantically identified by its variant and exit code; the prefix is redundant.

### Exit Code Mapping (existing, unchanged)

| Variant | Exit Code | sysexits.h | Meaning |
|---------|-----------|------------|---------|
| `JrError::ConfigError` | 78 | EX_CONFIG | Missing config file values |
| `JrError::UserError` | 64 | EX_USAGE | Missing CLI input / bad usage |
| `JrError::NotAuthenticated` | 2 | — | Auth required |
| `JrError::Interrupted` | 130 | — | Ctrl+C |
| All others | 1 | — | Generic / runtime |

### Group 1 — ConfigError (exit 78)

Missing values in config files that the user needs to set up via `jr init` or manual config editing.

| File | Line | Current | Message |
|------|------|---------|---------|
| `src/cli/board.rs` | 50 | `anyhow::anyhow!(...)` | "No board configured. Use --board <ID> or set board_id in .jr.toml..." |
| `src/cli/sprint.rs` | 20 | `anyhow::anyhow!(...)` | "No board configured. Use --board <ID> or set board_id in .jr.toml..." |
| `src/api/client.rs` | 36 | `anyhow::anyhow!(...)` | "No Jira instance configured. Run \"jr init\" first." |
| `src/cli/team.rs` | 86 | `anyhow::anyhow!(...)` | "No Jira instance configured. Run \"jr init\" first." |
| `src/config.rs` | 98 | `anyhow::anyhow!(...)` | "No Jira instance configured. Run \"jr init\" first." |
| `src/cli/issue/helpers.rs` | 20 | `anyhow::anyhow!(...)` | "No \"Team\" field found on this Jira instance..." |
| `src/cli/issue/helpers.rs` | 78 | `anyhow::anyhow!(...)` | "Story points field not configured..." |

Each becomes `JrError::ConfigError("...".into())`.

### Group 2 — UserError (exit 64)

Missing required CLI input that the user should provide via flags.

| File | Line | Current | Message |
|------|------|---------|---------|
| `src/cli/issue/create.rs` | 48 | `anyhow::anyhow!(...)` | "Project key is required. Use --project or configure .jr.toml..." |
| `src/cli/issue/create.rs` | 63 | `anyhow::anyhow!(...)` | "Issue type is required. Use --type" |
| `src/cli/issue/create.rs` | 74 | `anyhow::anyhow!(...)` | "Summary is required. Use --summary" |
| `src/cli/project.rs` | 70 | `anyhow::anyhow!(...)` | "No project specified. Run \"jr project list\"..." |
| `src/cli/issue/workflow.rs` | 121 | `anyhow::anyhow!(...)` | "Invalid selection" |
| `src/cli/issue/workflow.rs` | 123 | `bail!(...)` | "Selection out of range" |

Each becomes `JrError::UserError("...".into())`. Note: line 123 uses `bail!()` (which is `anyhow::bail!`), same issue — the error is an untyped anyhow string that fails the downcast.

### Not Touched

These `anyhow::anyhow!` calls stay as-is because exit code 1 is appropriate:

- `src/api/auth.rs:145,147` — OAuth callback errors (no auth code, no state parameter). These are transient runtime errors during the OAuth flow.
- `src/api/auth.rs:205` — "No accessible Jira sites found." Arguably a config issue (wrong account), but fires during the OAuth flow itself, before config exists. Leaving as exit 1 is consistent with the other OAuth errors in the same function.
- `src/duration.rs:24` — Duration parse error (invalid number in a worklog duration string). This is a value-level parse failure, not a missing-config or missing-input error.
- `src/api/jira/teams.rs:26` — "Could not resolve organization ID." This can indicate a permissions problem or a network/API issue; the ambiguity makes exit 1 (generic) the safest choice.

### Conversion Mechanics

`JrError` derives `thiserror::Error`, which implements `std::error::Error`. `anyhow` has a blanket `From<E: Error>` impl, so returning `JrError::ConfigError("...".into())` from an `ok_or_else` closure in a function returning `anyhow::Result<T>` compiles without explicit `.into()` on the variant.

The `?` operator handles the `JrError` → `anyhow::Error` conversion. `downcast_ref::<JrError>()` in `main.rs` then successfully finds the variant and maps the exit code.

### What Changes

- `ConfigError` display format: `"Configuration error: {0}"` → `"{0}"` (removes redundant prefix, preserves message text)
- Exit codes for 13 error paths (from 1 to 78 or 64)

### What Doesn't Change

- Error message text (identical strings after the format change above)
- The `JrError` enum variants (no new variants added)
- Auth, API, or runtime error paths
- Any command's success path

## Testing

### Unit Tests

Verify `JrError::exit_code()` mapping for `ConfigError` and `UserError` (may already exist — add if missing).

### Integration Tests

For each affected command, trigger the error condition and assert:
1. The error message text is preserved
2. The process exits with the correct code (78 or 64)

Priority integration tests (representative of all 13 sites — both groups use the same mechanical pattern):
- `board view` without `board_id` configured → exit 78 (representative of all 7 ConfigError sites)
- `issue create` without `--project` in non-interactive mode → exit 64 (representative of all 6 UserError sites)

### Existing Tests

No existing tests should break — error messages are identical (after the ConfigError format change), and no test currently asserts exit codes for these paths.

Any test that previously asserted the `"Configuration error: "` prefix in `ConfigError` display output would need updating — but grep confirms no such tests exist.
