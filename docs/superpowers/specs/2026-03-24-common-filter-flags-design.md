# Common Filter Flags for Issue List — Design Spec

**Goal:** Add `--assignee`, `--reporter`, and `--recent` shorthand flags to `jr issue list` so common queries don't require JQL knowledge.

**Problem:** The most common queries — "show my tickets," "show what I reported," "show recent tickets" — require `--jql 'assignee = currentUser() AND ...'` syntax. This is a barrier for users unfamiliar with JQL.

**Addresses:** [GitHub Issue #44](https://github.com/Zious11/jira-cli/issues/44)

---

## Architecture

Three new flags on `jr issue list` that generate JQL clauses under the hood. All flags compose additively with each other, with `--jql`, and with existing flags (`--status`, `--team`). The `me` keyword resolves to `currentUser()` server-side (no API call). Non-`me` values resolve via the Jira user search API with partial-match disambiguation.

**Precedent:** GitHub CLI (`gh issue list`) composes `--search` with shorthand flags (`--assignee`, `--label`, `--state`) additively — they AND together. We follow the same pattern.

---

## 1. New CLI Flags

```
--assignee <name|me>   Filter by assignee. "me" resolves to currentUser()
--reporter <name|me>   Filter by reporter. "me" resolves to currentUser()
--recent <duration>    Show issues created within duration (e.g., 7d, 4w, 2M)
```

### Flag Definitions

In `IssueCommand::List`:

```rust
/// Filter by assignee ("me" for current user, or a name to search)
#[arg(long)]
assignee: Option<String>,

/// Filter by reporter ("me" for current user, or a name to search)
#[arg(long)]
reporter: Option<String>,

/// Show issues created within duration (e.g., 7d, 4w, 2M)
#[arg(long)]
recent: Option<String>,
```

### Composition Rules

- All flags AND together: `--assignee me --status "In Progress" --recent 7d` produces `assignee = currentUser() AND status = "In Progress" AND created >= -7d`
- Flags compose with `--jql`: `--jql "type = Bug" --assignee me` produces `type = Bug AND assignee = currentUser()`
- Flags compose with auto-detected board JQL (scrum sprint, kanban)
- Each flag can only be specified once (clap default for `Option<String>`)

### Duration Format

JQL relative dates use the format `(+/-)nn(unit)` where units are case-sensitive:

| Unit | Meaning |
|------|---------|
| `y` | years |
| `M` | months (uppercase) |
| `w` | weeks |
| `d` | days |
| `h` | hours |
| `m` | minutes (lowercase) |

Combined units like `4w2d` are not supported by Jira. Client-side validation regex: `^\d+[yMwdhm]$`.

---

## 2. User Resolution

### The `me` Keyword

`--assignee me` and `--reporter me` resolve to `currentUser()` in JQL — a server-side function requiring no API call. Case-insensitive match (`Me`, `ME`, `me` all work).

### Name Resolution (non-`me` values)

For non-`me` values like `--assignee "Jane"`:

1. Call `GET /rest/api/3/user/search?query=<name>` — prefix-matches displayName and email
2. Filter results to only active users (`active == true`) — the endpoint returns both active and inactive users
3. If exactly 1 active user matches → use their `accountId` in JQL: `assignee = <accountId>`
4. If multiple active users match → use `partial_match` module for disambiguation (same pattern as `--team`)
   - Interactive mode: prompt user to pick
   - `--no-input` mode: error listing the matches
5. If 0 active matches → error: `"No active user found matching '<name>'. The user may be deactivated."`

**Note:** accountId is used without quotes in JQL: `assignee = 5b10ac8d82e05b22cc7d4ef5`

### API Endpoint Details (validated via Perplexity)

- **Endpoint:** `GET /rest/api/3/user/search?query=<name>`
- **Response:** Conflicting documentation — Perplexity sources disagree on whether this is a flat array `[User, ...]` or paginated `{ "values": [...] }`. The implementation should try deserializing as `Vec<User>` first (flat array) and fall back to extracting from a paginated wrapper if needed. Verify empirically during implementation. Each User object has `accountId`, `displayName`, `active`, etc.
- **Permission:** Requires "Browse users and groups" global permission (standard)
- **Behavior without permission:** May return empty results (200 OK) rather than 403 — indistinguishable from "no matches"
- **Prefix matching:** Matches start of displayName or emailAddress words

---

## 3. JQL Construction Changes

### Current Behavior

- `--jql` bypasses all auto-detection; `--status` and `--team` are silently ignored
- Scrum path hardcodes `assignee = currentUser()`
- Kanban path hardcodes `assignee = currentUser()`

### New Behavior

All paths use a unified JQL assembly flow:

1. **Start with base JQL parts** (`Vec<String>`):
   - If `--jql` provided → push as first part
   - If no `--jql` → auto-detect board context:
     - Scrum: `sprint = {id}` (no implicit `assignee = currentUser()`)
     - Kanban: `project = "KEY" AND statusCategory != Done` (no implicit `assignee = currentUser()`)
     - Fallback: `project = "KEY"` (if available)
2. **Append filter flag clauses:**
   - `--assignee me` → push `assignee = currentUser()`
   - `--assignee "Jane"` → resolve → push `assignee = <accountId>`
   - `--reporter me` → push `reporter = currentUser()`
   - `--reporter "Jane"` → resolve → push `reporter = <accountId>`
   - `--status "In Progress"` → push `status = "In Progress"` (unchanged)
   - `--team "Alpha"` → push `<field_id> = "<uuid>"` (unchanged)
   - `--recent 7d` → push `created >= -7d`
3. **Join** all parts with ` AND `
4. **Append** `ORDER BY` clause (rank for board queries, updated DESC for fallback)

**Unbounded query guard:** The current `build_fallback_jql` errors when no filters are provided (`project_key`, `status`, `resolved_team` all `None`). This guard must be updated to consider the new flags as well — a query like `--assignee me` with no project is valid JQL. The error should only fire when *all* filter sources (project, status, team, assignee, reporter, recent, jql) are empty.

### Breaking Changes

**1. Implicit `assignee = currentUser()` removed.** The scrum and kanban auto-detection paths currently hardcode `assignee = currentUser()`. This is removed. Users who want their own tickets use `--assignee me` explicitly. This makes the behavior consistent and predictable — no hidden filters.

**Migration:** Users running `jr issue list` (no flags, board configured) will now see all sprint/board tickets instead of just their own. The `--assignee me` flag restores the previous behavior.

**2. `--jql` now composes with filter flags.** Previously, `--jql` silently ignored `--status` and `--team`. Now all flags AND together with `--jql`. Users who relied on `--status` being silently dropped when `--jql` was present will get different (more specific) results. This is the correct behavior — the old silent-ignore was a bug, not a feature.

---

## 4. Duration Validation

Client-side validation gives better errors than Jira's generic 400:

```rust
pub fn validate_duration(s: &str) -> Result<(), String> {
    let re = regex or manual check: digits followed by one of [yMwdhm]
    // Valid: "7d", "30d", "4w", "2M", "1y", "5h", "10m"
    // Invalid: "7x", "d7", "", "4w2d"
}
```

No regex crate needed — a simple manual check (all chars except last are digits, last char is one of `yMwdhm`, at least 2 chars total) is cleaner.

**Why `jql.rs` and not `duration.rs`:** The existing `src/duration.rs` handles worklog durations (`1h30m`, `2d`) which support combined units and a different format. JQL relative date durations (`7d`, `2M`) are a distinct format — single unit only, case-sensitive `M` for months. They belong in `jql.rs` alongside other JQL utilities (`escape_value`, `strip_order_by`).

Error message: `"Invalid duration '7x'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M)."`

---

## 5. Error Handling

| Scenario | Behavior |
|----------|----------|
| `--assignee "nonexistent"` → 0 matches | Error: `"No user found matching 'nonexistent'. Check the name and try again."` |
| `--assignee "J"` → multiple, interactive | Prompt to pick (same UX as `--team` disambiguation) |
| `--assignee "J"` → multiple, `--no-input` | Error: `"Multiple users match 'J': Jane Doe, John Smith. Use a more specific name."` |
| `--recent "7x"` → invalid duration | Error: `"Invalid duration '7x'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M)."` |
| User search API returns empty (no permission) | Same as "no matches" — `"No user found matching 'X'."` |
| User search API fails (network/500) | Propagate error with context |

---

## 6. File Structure

### Modified Files

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add `assignee`, `reporter`, `recent` flags to `IssueCommand::List` |
| `src/cli/issue/list.rs` | Refactor JQL construction to compose all flags additively; remove implicit `assignee = currentUser()` from scrum/kanban paths; add duration validation; call `resolve_user()` for assignee/reporter |
| `src/cli/issue/helpers.rs` | Add `resolve_user()` helper (user search + partial match disambiguation) |
| `src/api/jira/users.rs` | Add `search_users()` method |
| `src/jql.rs` | Add `validate_duration()` function |

### Not Changed

- `src/api/client.rs` — no new HTTP methods needed
- `src/config.rs` — no new config entries
- `src/cache.rs` — no caching for user search results (names change, no TTL benefit)

---

## 7. Testing Strategy

### Unit Tests

- `validate_duration()` — valid formats (`7d`, `30d`, `4w`, `2M`, `1y`, `5h`, `10m`, `0d`), invalid formats (`7x`, `d7`, ``, `4w2d`)
- `resolve_user()` with `me`/`Me`/`ME` → returns `"currentUser()"` without API call
- JQL composition — all flag combinations produce correct JQL strings
- JQL composition with `--jql` base + filter flags

### Integration Tests (wiremock)

- User search returns 1 result → accountId used in JQL
- User search returns 0 results → error message
- User search returns multiple → disambiguation (may be hard to test interactively; test the non-interactive error path)
- `--recent 7d` → `created >= -7d` in JQL
- `--jql "type = Bug" --assignee me` → `type = Bug AND assignee = currentUser()`
- `--assignee me --status "Done" --recent 30d` → three AND clauses

### Manual Testing

- `jr issue list --project KEY --assignee me`
- `jr issue list --project KEY --reporter me --recent 7d`
- `jr issue list --project KEY --assignee "Jane" --status "In Progress"`
- `jr issue list --jql "type = Bug" --assignee me --recent 30d`
- `jr issue list` (board configured) — should show all tickets, not just own

---

## Validation Sources

| Decision | Validated by |
|----------|-------------|
| `gh issue list` composes `--search` with shorthand flags additively | Perplexity (GitHub CLI docs) |
| `assignee = currentUser()` and `reporter = currentUser()` are valid JQL | Perplexity (JQL reference) |
| `created >= -7d` is valid JQL for relative dates | Perplexity (Atlassian JQL docs) |
| Duration units: `y`, `M` (months), `w`, `d`, `h`, `m` (minutes) — case-sensitive | Perplexity (Atlassian JQL functions reference) |
| Combined units like `4w2d` are not supported | Perplexity |
| Display names don't work directly in JQL assignee/reporter fields | Perplexity (Atlassian community) |
| `~` (CONTAINS) operator doesn't work on assignee/reporter fields | Perplexity (Atlassian JQL operators) |
| `GET /rest/api/3/user/search?query=<name>` for prefix-matching user lookup | Perplexity (Atlassian API docs) |
| User search response format conflicted — may be flat array or paginated; verify during implementation | Perplexity (conflicting Atlassian API docs) |
| accountId used without quotes in JQL: `assignee = accountId` | Perplexity (Atlassian community) |
| User search requires "Browse users and groups" permission; may return empty instead of 403 | Perplexity (Atlassian developer community) |
| `/user/search` is better than `/user/assignable/search` for reporter resolution | Perplexity (Atlassian API docs) |
