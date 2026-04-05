# Date Filter Flags for `jr issue list` — Design Spec

> **Issue:** #113 — `issue list: add --created-after and --created-before date filters`

## Problem

`jr issue list` supports `--recent 7d` for relative date filtering but not absolute date ranges. Users who need issues created or updated within a specific date range must use raw JQL:

```bash
jr issue list --jql 'project = PROJ AND created >= "2026-03-18"'
```

This requires knowing JQL syntax, which is a knowledge barrier for common queries.

## Solution

Add four convenience flags that generate JQL date clauses:

| Flag | JQL generated | Meaning |
|------|--------------|---------|
| `--created-after YYYY-MM-DD` | `created >= "YYYY-MM-DD"` | Issues created on or after this date |
| `--created-before YYYY-MM-DD` | `created < "YYYY-MM-DD+1"` | Issues created on or before this date |
| `--updated-after YYYY-MM-DD` | `updated >= "YYYY-MM-DD"` | Issues updated on or after this date |
| `--updated-before YYYY-MM-DD` | `updated < "YYYY-MM-DD+1"` | Issues updated on or before this date |

### Operator semantics — the midnight problem

JQL interprets date-only values as midnight (00:00:00). This creates a subtle trap:

- `created >= "2026-03-18"` means "from midnight March 18 onwards" — **includes** all of March 18 ✅
- `created <= "2026-03-18"` means "up to midnight March 18" — **excludes** issues created during March 18 ❌

To give users intuitive "on or before this date" behavior, the `--before` flags add one day and use `<`:

- `--created-before 2026-03-18` generates `created < "2026-03-19"` — includes all of March 18 ✅

The `--after` flags use `>=` directly since midnight-of-date is the correct lower bound.

### Date format

Accept `YYYY-MM-DD` only (ISO 8601 calendar date). Jira JQL also accepts `YYYY/MM/DD` and optional `HH:MM` time, but we accept only the canonical format for simplicity and consistency. Users who need time precision or alternate formats can use `--jql`.

### Validation

Parse dates with `chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d")` before sending to JQL. This catches:

- Invalid format (e.g., `03-18-2026`, `2026/03/18`)
- Impossible dates (e.g., `2026-02-30`, `2026-13-01`)

Add `validate_date(s: &str) -> Result<NaiveDate, String>` to `jql.rs`. Returns the parsed `NaiveDate` (needed by `--before` flags to compute +1 day). Validation happens early in `handle_list`, same pattern as `--recent`.

### Flag conflicts

| Flag | Conflicts with |
|------|---------------|
| `--created-after` | `--recent` (both set a lower bound on `created`) |
| `--created-before` | (none) |
| `--updated-after` | (none) |
| `--updated-before` | (none) |

`--created-after` and `--created-before` do NOT conflict with each other — using both creates a date range. Same for the `--updated-*` pair.

None of the date flags conflict with `--jql`. When combined, date clauses are AND'd with the user's JQL, same as all other filter flags.

### JQL generation

Each flag adds a clause via `build_filter_clauses` in `list.rs`. For `--after` flags, the clause is a simple string interpolation. For `--before` flags, the date is incremented by one day using `chrono::Days::new(1)` and formatted back to `YYYY-MM-DD`.

### Composability

All four flags combine freely with each other and with existing flags:

```bash
# Date range
jr issue list --created-after 2026-03-01 --created-before 2026-03-31

# With other filters
jr issue list --created-after 2026-03-18 --assignee me --open

# Updated date range
jr issue list --updated-after 2026-03-01 --updated-before 2026-04-01 --status "In Progress"
```

### Error messages

**Invalid date format:**
```
Invalid date "03-18-2026". Expected format: YYYY-MM-DD (e.g., 2026-03-18).
```

**Impossible date:**
```
Invalid date "2026-02-30". Expected format: YYYY-MM-DD (e.g., 2026-03-18).
```

**Conflict with `--recent`:**
```
error: the argument '--created-after <DATE>' cannot be used with '--recent <DURATION>'
```
(Clap's automatic conflict error message.)

### Non-interactive / JSON output

No interactive behavior. The flags are fully non-interactive — they take a value and generate JQL. No special JSON output handling needed; the flags only affect which issues are returned.

## Files changed

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add 4 new args to `IssueCommand::List` with `conflicts_with` on `created_after` |
| `src/jql.rs` | Add `validate_date(s: &str) -> Result<NaiveDate, String>` |
| `src/cli/issue/list.rs` | Validate dates early, pass to `build_filter_clauses`, add 4 JQL clauses |
| `tests/cli_smoke.rs` | Smoke test for `--created-after`/`--recent` conflict |
| `tests/cli_handler.rs` | Handler test for date flags generating correct JQL |

## Out of scope

- Time-of-day precision (`--created-after "2026-03-18 14:30"`) — use `--jql`
- `YYYY/MM/DD` format — use `--jql`
- Relative date expressions in these flags (e.g., `--created-after "2 weeks ago"`) — use `--recent`
- `startOfDay()` / `endOfDay()` JQL functions — the +1 day approach is simpler and equivalent
