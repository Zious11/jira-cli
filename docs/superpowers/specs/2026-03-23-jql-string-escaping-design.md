# JQL String Escaping

## Overview

Escape user-supplied values before interpolation into JQL double-quoted string literals to prevent JQL injection. Add a `jql::escape_value` helper and apply it to all 9 string interpolation sites.

Closes: GitHub issue #28.

## Problem

JQL queries are built using `format!("project = \"{}\"", pk)` without escaping. A value like `--status 'In Progress" OR assignee != currentUser() OR status = "Done'` breaks out of the JQL string literal and injects arbitrary JQL clauses.

Risk is low in direct CLI usage (users authenticate with their own credentials and can already pass raw JQL via `--jql`). The concern is automation pipelines where `--status` or `--project` values come from untrusted external sources (CI, chatbots, webhook-driven scripts).

## Design

### New module: `src/jql.rs`

A single public function following the crate's pattern of focused utility modules (`partial_match.rs`, `duration.rs`, `adf.rs`):

```rust
/// Escape a value for interpolation into a JQL double-quoted string literal.
///
/// Backslashes are escaped first, then double quotes. Order matters: escaping
/// quotes first would introduce backslashes that the second pass re-escapes,
/// leaving the quote exposed (escape neutralization attack).
pub fn escape_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
```

Register in `src/lib.rs` with `pub mod jql;`.

### Affected call sites

9 string interpolation sites across 2 files. Each `format!("... = \"{}\"", value)` becomes `format!("... = \"{}\"", crate::jql::escape_value(value))`:

| File | Location | Value escaped |
|------|----------|--------------|
| `src/cli/issue/list.rs` | scrum path | `status` |
| `src/cli/issue/list.rs` | scrum path | `team_uuid` |
| `src/cli/issue/list.rs` | kanban path | `project_key` |
| `src/cli/issue/list.rs` | kanban path | `status` |
| `src/cli/issue/list.rs` | kanban path | `team_uuid` |
| `src/cli/issue/list.rs` | `build_fallback_jql` | `project_key` |
| `src/cli/issue/list.rs` | `build_fallback_jql` | `status` |
| `src/cli/issue/list.rs` | `build_fallback_jql` | `team_uuid` |
| `src/cli/board.rs` | kanban path | `project_key` |

### What doesn't change

- **`--jql` raw JQL** — user-supplied, passed through verbatim. This is intentional: the user explicitly owns the query semantics.
- **Sprint IDs** (`sprint = {}` at list.rs:61) — integer from Jira API response, not string-interpolated into quotes.
- **Field identifiers** (`field_id` at list.rs:68, 91, 170) — from config or API field discovery, always `customfield_NNNNN` format, interpolated unquoted as JQL field names. These are trusted: `team_field_id` comes from Jira's field API, and `story_points_field_id` is set by `jr init` or manually by the user in their own config file.
- **URL encoding** in `api/jira/sprints.rs` — HTTP-level encoding, not JQL escaping.
- **`board.rs` `ORDER BY AND` bug** (line 67-68) — pre-existing invalid JQL construction tracked separately as issue #31. Out of scope for this change.

### Control characters (newline, tab, carriage return)

JQL's parser recognizes `\n`, `\t`, and `\r` as escape sequences inside double-quoted strings. However, these are not an injection vector — they cannot break out of a string literal or alter query semantics. A literal newline in a CLI flag value is already unlikely (shell argument parsing strips them), and in automation pipelines, values come from structured sources (JSON, YAML) where newlines would be explicit.

The `escape_value` function does not escape control characters. If a value contains a literal newline, it passes through as-is — Jira's JQL parser will either treat it as whitespace within the string or reject the query with a parse error, both of which are safe outcomes. No silent semantic change occurs.

### Why escape order matters

Backslash must be escaped before double quote. Proof by counterexample with input `foo\"bar`:

**Correct (backslash first):**
1. `\` → `\\`: `foo\\"bar`
2. `"` → `\"`: `foo\\\"bar`
3. JQL parser: `\\` (literal `\`) + `\"` (literal `"`) + `bar` — string intact.

**Wrong (quote first):**
1. `"` → `\"`: `foo\\"bar`
2. `\` → `\\`: `foo\\\\"bar`
3. JQL parser: `\\` (literal `\`) + `\\` (literal `\`) + `"` closes the string — injection!

## Testing

5 unit tests + 1 proptest on `escape_value`:

1. **No special characters** — input unchanged
2. **Double quotes escaped** — `He said "hello"` → `He said \"hello\"`
3. **Backslashes escaped** — `path\to\file` → `path\\to\\file`
4. **Escape neutralization prevented** — `foo\"bar` → `foo\\\"bar` (both `\` and `"` escaped)
5. **Trailing backslash** — `foo\` → `foo\\` (closing `"` of JQL string not consumed)
6. **Proptest: no unescaped quotes** — for arbitrary strings, verify the escaped output contains no `"` that isn't preceded by an odd number of `\` characters. This catches edge cases the enumerated tests might miss.

Existing `build_fallback_jql` tests are unaffected — they use clean string values that don't contain special characters.

## Files touched

| File | Change |
|------|--------|
| `src/jql.rs` | New: `escape_value` function + unit tests |
| `src/lib.rs` | Add `pub mod jql;` |
| `src/cli/issue/list.rs` | Apply `crate::jql::escape_value()` at 8 interpolation sites |
| `src/cli/board.rs` | Apply `crate::jql::escape_value()` at 1 interpolation site |
