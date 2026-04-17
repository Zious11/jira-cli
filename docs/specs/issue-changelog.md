# Issue Changelog Command

**Issue:** #200
**Date:** 2026-04-17
**Status:** Design

## Problem

`jr issue view` shows only the current state of an issue. There's no way to
inspect its audit history — who transitioned status, who reassigned, when
labels or points changed. Recovering that timeline requires opening the issue
in the browser and scrolling the History tab.

For agents and scripts, the missing access is a real gap: time-in-status
audits, "why did this land in Done?" debugging, and workflows that need to
reason about prior state all require the changelog.

## Solution

Add a new subcommand:

```
jr issue changelog <KEY> [flags]
```

Backed by the Jira Cloud REST API endpoint
`GET /rest/api/3/issue/{issueIdOrKey}/changelog` (offset-paginated, returns
`values[]` with `id`, `author`, `created`, and `items[]`).

Table output is flat (one row per `ChangelogItem`) for pipe-friendly scanning;
JSON output preserves the nested API structure for agents that need to
correlate co-occurring changes.

## CLI Surface

```
jr issue changelog <KEY>                      # default, newest-first, limit 30
jr issue changelog FOO-1 --all                # no limit
jr issue changelog FOO-1 --limit 5
jr issue changelog FOO-1 --field status       # status changes only
jr issue changelog FOO-1 --field status --field assignee
jr issue changelog FOO-1 --author me
jr issue changelog FOO-1 --reverse            # oldest-first
jr issue changelog FOO-1 --output json
```

### Flags

| Flag | Type | Default | Behavior |
|------|------|---------|----------|
| `--limit N` | `u32` | 30 | Cap post-filter rows. Conflicts with `--all`. `0` returns empty. |
| `--all` | flag | off | No output truncation (still always fetches every page). Conflicts with `--limit`. |
| `--field <NAME>` | repeatable | — | Client-side filter by field name (case-insensitive substring). |
| `--author <ME\|NAME\|ACCOUNTID>` | `String` | — | Client-side filter. `me` resolves via `/myself`. Names match `displayName` substring. Bare accountId matched literally. |
| `--reverse` | flag | off | Render oldest-first instead of default newest-first. |

Global `--output`, `--no-color`, `--no-input` already in scope.

### Fetch / filter / sort / limit semantics

The Jira changelog API supports no server-side filter parameters, and its
return order is not officially documented (observed oldest-first but not
guaranteed — confirmed via Perplexity against Atlassian docs). We therefore
sort client-side unconditionally rather than rely on server order.

The default display is newest-first with `--limit 30`, so we cannot
early-exit the fetch loop based on `--limit` alone (we'd risk returning
whichever 30 entries the server happened to stream first).

v1 algorithm is deliberately simple and correct:

1. Fetch **all** changelog pages.
2. Sort in memory: DESC by default, ASC with `--reverse`.
3. Apply client-side `--field` / `--author` filters.
4. Truncate to `--limit` (no cap if `--all`).

Cost: fetches the entire changelog even for small `--limit`. For typical
Jira issues (dozens of entries), this is fine. For issues with hundreds of
entries, we still make a bounded number of offset-paginated calls. If that
becomes a measurable problem, the optimization is to walk pages backwards
using `total` from the first page — tracked in Future Extensions.

## Architecture

### File layout

```
src/
├── cli/issue/
│   └── changelog.rs      # NEW — handler, formatting, filter logic
├── api/jira/
│   └── issues.rs         # add `get_changelog` method to existing impl block
├── types/jira/
│   └── changelog.rs      # NEW — ChangelogEntry, ChangelogItem
```

`list.rs` (1422 lines) is left untouched; CLAUDE.md already flags it as too
large. Adding to `list.rs` would deepen the problem. Creating
`cli/issue/changelog.rs` matches the split already underway
(`workflow.rs`, `links.rs`, `assets.rs`).

### Types

```rust
// src/types/jira/changelog.rs
use crate::types::jira::User;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangelogEntry {
    pub id: String,
    pub author: Option<User>,   // null for automation/system events
    pub created: String,        // ISO-8601; rendered client-side
    pub items: Vec<ChangelogItem>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogItem {
    pub field: String,
    pub fieldtype: String,
    pub from: Option<String>,
    pub from_string: Option<String>,
    pub to: Option<String>,
    pub to_string: Option<String>,
}
```

Register in `src/types/jira/mod.rs`:

```rust
pub mod changelog;
pub use changelog::*;
```

### API method

Added to the existing `impl JiraClient` in `src/api/jira/issues.rs`
(same resource; clippy warns against scattering inherent impls):

```rust
pub async fn get_changelog(&self, key: &str) -> Result<Vec<ChangelogEntry>> {
    // Offset-paginated loop using OffsetPage<ChangelogEntry>.
    // Response items are under `values`; OffsetPage.items() already prefers
    // this key. Fetches every page and returns the full list in whatever
    // order the API chose. Sorting, filtering, and limit application are
    // the caller's responsibility (handled in the handler).
}
```

No filter or limit parameter: the fetch is always exhaustive. Sort, filter,
and truncate happen in the handler after fetch (see semantics above).

### Handler flow (`src/cli/issue/changelog.rs`)

1. Parse args from `IssueCommand::Changelog`.
2. Resolve `--author me` → accountId via `client.get_current_user()`
   (only if the flag is set).
3. Call `client.get_changelog(key)` — fetches all pages.
4. Sort entries by `created`: DESC by default, ASC with `--reverse`.
5. Apply client-side filters: drop entries whose author doesn't match
   `--author`, and drop items whose field doesn't match any `--field`
   (field filter operates at the item level, not the entry level — an
   entry with status + resolution changes survives `--field status` but
   only its status item is rendered).
6. Flatten `(entry, item)` pairs in sorted order.
7. Truncate to `--limit` post-filter rows (no cap if `--all`).
8. Render: table or JSON.

Field filtering operating at the item level is deliberate: when a single
transition updates status + resolution + timeestimate, users asking
`--field status` want the status change alone, not the whole cluster.

### Output formats

**Table (default):**

```
DATE              AUTHOR          FIELD        FROM              TO
2026-04-16 14:02  Alice Smith     status       To Do             In Progress
2026-04-16 14:02  Alice Smith     resolution   —                 Done
2026-04-15 09:31  Bob Jones       labels       —                 backend
2026-04-14 11:10  (system)        assignee     —                 Alice Smith
```

- **DATE**: `YYYY-MM-DD HH:MM` in the user's local time zone.
- **AUTHOR**: `displayName`, or `(system)` for null. Matches project
  convention of readable labels (`src/cli/issue/format.rs:62` uses
  `"Unassigned"` for null assignees).
- **FIELD**: raw from API (`status`, custom field display name, etc.).
- **FROM / TO**: prefer `fromString` / `toString`; fall back to raw
  `from` / `to`. Null / empty renders as `—` (em dash).

**JSON (`--output json`)**: preserves nested API structure.

```json
{
  "key": "FOO-123",
  "entries": [
    {
      "id": "10000",
      "author": { "accountId": "...", "displayName": "Alice Smith" },
      "created": "2026-04-16T14:02:11.000Z",
      "items": [
        {
          "field": "status",
          "fieldtype": "jira",
          "from": "1",
          "fromString": "To Do",
          "to": "3",
          "toString": "In Progress"
        }
      ]
    }
  ]
}
```

**Empty result:** table prints header + `No changelog entries.`; JSON emits
`{ "key": "FOO-123", "entries": [] }`. Both exit 0.

## Error Handling

| Case | Exit | Message | Hint |
|------|------|---------|------|
| 404 (issue not found) | 2 | `Issue FOO-123 not found` | `Check the key and your project access` |
| 403 (no permission) | 2 | `No access to FOO-123 changelog` | `Browse Projects permission is required` |
| 401 (auth) | handled centrally by `JiraClient` | — | — |
| `--author me` unauthenticated | bubbles up auth error | — | `Run jr auth login` |
| No entries after filter | 0 | empty render | — |
| `--limit 0` | 0 | empty render | — (post-fetch truncation to 0 rows) |

All mapped to `JrError` variants already in use.

## Changes by File

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add `IssueCommand::Changelog { key, limit, all, field (Vec), author, reverse }` |
| `src/cli/issue/mod.rs` | Dispatch `IssueCommand::Changelog` → `changelog::handle` |
| `src/cli/issue/changelog.rs` | NEW — handler, formatting, filter closure |
| `src/api/jira/issues.rs` | Add `get_changelog` to `impl JiraClient` |
| `src/types/jira/changelog.rs` | NEW — `ChangelogEntry`, `ChangelogItem` |
| `src/types/jira/mod.rs` | `pub mod changelog; pub use changelog::*;` |
| `tests/issue_changelog.rs` | NEW — integration tests |
| `tests/common/fixtures.rs` | Add a `changelog_response_page` fixture builder (new helper for this endpoint's response shape) |

## Testing Strategy

TDD: write tests first. Use wiremock for integration, `insta` for snapshots.

### Unit tests (inline)

- Deserialization of the API response into `ChangelogEntry`, including:
  - `author: null` (automation / system events)
  - Missing `fromString` / `toString` (raw `from` / `to` only)
  - Multiple `items[]` in one entry
- Field filter predicate: case-insensitive substring, any-item match.
- Author filter predicate: `me`, name substring, raw accountId.
- Sort direction: default DESC vs `--reverse` ASC.
- Flattening: entry with 3 items → 3 rows preserving entry metadata.

### Integration tests (`tests/issue_changelog.rs`)

- Happy path: single-page response → expected table rows + order.
- `--output json` → nested structure matches API.
- `--limit 2` on multi-page response → fetches all pages, renders 2 newest.
- `--all` → renders every row (no truncation).
- `--field status` with mixed-field response across pages → only status
  items survive; unrelated items (resolution, labels) are dropped even
  when they share an entry.
- `--author me` → mock `/myself` stub + changelog stub; confirms filter.
- `--reverse` → entries rendered oldest-first.
- 404 / 403 → correct exit code + message.
- Empty response → exit 0 with empty-state render.
- Non-TTY: same JSON output as TTY (`--output json` forced).

### Snapshot tests (`insta`)

- Table output for representative multi-entry, multi-item response.
- JSON output structure.

No proptest: data shape is small and exhaustively enumerable.

### Manual smoke checks (post-merge)

- Against an issue with automation-driven transitions → `(system)` renders.
- `--field "Story Points"` (custom field display name) → matches correctly.

## API Constraints (Validated)

- `GET /rest/api/3/issue/{key}/changelog` supports **only** `startAt` and
  `maxResults` query params. No server-side field / author / date filtering.
  All filtering must be client-side. (Perplexity)
- API sort order is **not officially documented**. Observed oldest-first,
  but this is not guaranteed, so we sort client-side unconditionally by
  `created`. (Perplexity)
- Response shape is offset-paginated: `{ startAt, maxResults, total,
  isLast, values[] }`. Each entry has `id`, `author` (optional), `created`,
  `items[]`. Each item has `field`, `fieldtype`, `from`, `fromString`, `to`,
  `toString`. (Perplexity)
- `author` is **optional / nullable** for automation, workflow post-functions,
  anonymous activity, and migrated / imported data. `Option<User>` required
  on the struct. (Perplexity)
- No bulk endpoint (`POST /rest/api/3/changelog/bulkfetch` does not exist).
  (Perplexity)
- Error responses use the standard Jira envelope:
  `{ errorMessages: [...], errors: {...}, status: int }`. (Perplexity)
- Developer-CLI output conventions: nested JSON (no flattening), local-time
  in tables, ISO-8601 UTC in JSON. (Perplexity, Context7 on `gh`)
- Repeatable `--flag value --flag value` is idiomatic in clap and already
  used in `jr` (`--label`, `--header`). (Context7, grep of codebase)

## Future Extensions (Out of Scope)

- **Backward-walk fetch optimization.** Use `total` from the first page to
  compute the last-page `startAt` and walk pages backwards, exiting as
  soon as `--limit` post-filter rows accumulate. Skip if the whole
  changelog is small. Only worth doing if a user reports slowness on
  large-history issues.
- `jr issue timeline <KEY>` — merged view of changelog + comments + worklogs.
  A distinct feature, not part of this spec.
- `--since <duration>` / `--from <date>` — would need client-side date
  filtering on the full response; revisit if a real use case emerges.
- Color-coded FIELD column (status changes highlighted) — YAGNI for v1.
