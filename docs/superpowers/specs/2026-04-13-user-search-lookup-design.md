# User Search and Lookup Commands

> **Issue:** #114 — Feature: user search/lookup command

## Problem

There is no CLI path to search for Jira users or resolve an `accountId` to a human name. When `issue assign --to <name>` fails (duplicate names, GDPR-hidden email, user not in expected project), the workaround is to find a ticket assigned to that person, run `jr issue view --output json`, and pull `assignee.accountId` by hand.

AI agents hit the same wall in reverse: JSON output from `issue view`/`issue list` surfaces raw `accountId` strings like `5b10ac8d82e05b22cc7d4349` with no in-CLI way to resolve them to a display name.

The API client already has `search_users`, `search_assignable_users`, and `search_assignable_users_by_project` (used internally by `issue assign`, `issue create --to`, etc.). Only the CLI surface is missing.

## Design

### Commands

| Command | Arg | API | Purpose |
|---|---|---|---|
| `jr user search <query>` | positional `<query>` | `GET /rest/api/3/user/search?query=X` | Fuzzy match displayName + emailAddress |
| `jr user list --project <KEY>` | `--project <KEY>` (required) | `GET /rest/api/3/user/assignable/multiProjectSearch?query=&projectKeys=KEY` | List users assignable to a project |
| `jr user view <accountId>` | positional `<accountId>` | `GET /rest/api/3/user?accountId=X` | Exact lookup — closes the reverse-resolution loop |

`user search` and `user list` reuse existing client methods (`search_users`, `search_assignable_users_by_project`) unchanged. `user view` needs one new client method.

### New API client method

Add to `src/api/jira/users.rs`:

```rust
/// Fetch a single user by accountId.
///
/// Returns `ApiError { status: 404 }` if the user does not exist.
/// Email may be omitted based on the target user's profile-visibility settings.
pub async fn get_user(&self, account_id: &str) -> Result<User> {
    let path = format!(
        "/rest/api/3/user?accountId={}",
        urlencoding::encode(account_id)
    );
    self.get(&path).await
}
```

Pattern matches `get_myself` (same file, line 6).

### Argument names

- `<query>` — matches the Jira API parameter name and reads naturally (`jr user search "jane"` or `jr user search "jane@acme.io"`).
- `<accountId>` — positional; the Atlassian-canonical name for the identifier. Not `--account-id` because this is the primary input, not a filter.
- `--project <KEY>` on `list` — required. Mirrors the convention of `--project` being a global flag with an explicit override (clap will treat the subcommand-local `--project` as the effective value).

### Flags

| Flag | Commands | Behavior |
|---|---|---|
| `--limit <N>` | `search`, `list` | Cap the number of rows shown (default 30 via `DEFAULT_LIMIT`). Applied client-side after the Jira response — does not reduce the API fetch. Conflicts with `--all`. |
| `--all` | `search`, `list` | Disable the default local cap. Jira still returns a single page (default 50, server-capped at 100). True multi-page pagination is a separate follow-up — see the "What stays out of scope" section. |
| global `--output {table,json}` | all three | Inherited from root CLI. |
| global `--no-color`, `--no-input`, `--verbose` | all three | Inherited. |

No `--email` flag — the API `query` parameter already matches both displayName and email substrings (confirmed via Atlassian docs and community reports). Passing `"jane@acme.io"` to `user search` works.

### Output

**Table mode** for `search` and `list`:

```
Display Name        | Email               | Active | Account ID
Jane Smith          | jane@acme.io        | ✓      | 5b10ac8d82e05b22cc7d4349
Ada Lovelace        | —                   | ✓      | 712020:abc123...
John Archive        | —                   | ✗      | 557058:def456...
```

- Column order: `Display Name`, `Email`, `Active`, `Account ID`. Display name first because it's the primary scannable column; accountId last because it's long and mainly for copy-paste.
- Full accountId (not truncated). Matches existing codebase convention (`src/cli/issue/helpers.rs:179,197` shows full IDs in disambiguation output).
- Missing email displays as `—`.
- Active uses `✓` / `✗` with color.

**Table mode** for `view`: labeled rows like `jr issue view` detail view — the four fields surfaced by the `User` type:

```
Account ID:   acc-xyz-1234
Display Name: Jane Smith
Email:        jane@acme.io
Active:       ✓
```

**JSON mode:** raw `User` objects serialized via `src/types/jira/user.rs`. Array for `search`/`list`, single object for `view`. When privacy hides the email, `emailAddress` is emitted as JSON `null` (the field is present, not absent) — `Option<String>` serializes without `skip_serializing_if`.

### Error handling

| Scenario | HTTP | jr behavior |
|---|---|---|
| `view`: unknown accountId | 404 (or 400 — Jira is inconsistent across accountId formats) | `Error: User with accountId 'X' not found.` — exit 1. Match on either status in the handler; rely on wiremock tests to lock in behavior against a real response shape. |
| `search`/`list`: no matches | 200 empty array | `"No results found."` on stdout via `output::print_output` — exit 0 (matches `issue list` convention) |
| `search`/`list`: caller lacks "Browse users and groups" | 200 empty array | Same as "no matches" — API silently returns empty. Help text warns of this. |
| `view`: caller lacks permission | 403 | Propagated through existing error handling with "permission" guidance |
| Network/auth failures | existing | Existing client error handling (401 retry, rate limit, etc.) |

Help text for `search` and `list` includes a note:

> Results depend on the **Browse users and groups** global permission and each user's profile-visibility settings. Empty results may indicate either no matches or missing permission. Email is hidden when the target user's privacy settings opt out.

### CLI module layout

```
src/cli/user.rs       — handler + formatting (NEW)
src/cli/mod.rs        — register `pub mod user;`, add `User { command: UserCommand }` variant and `UserCommand` enum
src/api/jira/users.rs — add `get_user(account_id)` method (~10 lines)
src/main.rs           — dispatch `Command::User { command } => cli::user::handle(cli, command).await`
tests/user_commands.rs — integration tests (NEW)
```

The handler file follows the same shape as `src/cli/team.rs` or `src/cli/project.rs` (small, three-operation module — appropriate since user commands are a thin surface over existing API methods).

### Tests

Integration tests in `tests/user_commands.rs` using wiremock:

1. `search` — query matches two users → table has both rows, JSON is an array of 2
2. `search` — empty result → stdout empty in table mode, "No users found." on stderr, exit 0
3. `search --limit 5` — passes `maxResults=5` to API
4. `search --all` — paginates through multiple pages
5. `list --project FOO` — calls `/user/assignable/multiProjectSearch` with correct projectKeys
6. `list` with no `--project` → clap error, exit 64
7. `view <accountId>` — success, table view shows labeled rows
8. `view <accountId>` — 404 → friendly error, exit 1
9. `view <accountId> --output json` — emits the full user object
10. Privacy case — user with no `emailAddress` field → table shows `—`, JSON shows field absent
11. Verify snapshot tests of table output match (using `insta` consistent with other commands)

Unit tests inline in `src/cli/user.rs`:
- Row-formatting helper handles missing email
- Active indicator maps correctly

## What stays out of scope

| Feature | Why |
|---|---|
| `--email` flag | Redundant with positional query (validated) |
| Multiple projects on `list` | API supports comma-separated; YAGNI for v1. If added later, use repeated `-p X -p Y` per modern CLI convention |
| `/user/bulk` for multi-accountId lookup | No current demand; can add as `user view A B C` later |
| `/user/search/query` structured queries | Powerful (`is assignee of PROJ`, property filters) but a separate feature |
| User list caching | Users join/leave frequently; cache would be stale |
| accountId truncation | Codebase convention is full IDs |
| Auto-disambiguation on `view` by name | Keeps `view` deterministic — `search` is for fuzzy lookup |
| True multi-page pagination for `search`/`list` | Both endpoints support `startAt`/`maxResults` but currently the client calls them once without either. `--all` disables the local cap but cannot exceed Jira's single-page default (50, capped at 100). Filed as a follow-up issue so the client method change can be planned alongside callers like `issue assign --to`. |

## Alignment with project conventions

- **Thin client, no abstraction layer** — reuses existing `search_users` / `search_assignable_users_by_project`, adds one sibling method.
- **Machine-output-first** — all three commands return `--output json`. The empty-result message (`"No results found."`) stays on stdout via `output::print_output`, consistent with the rest of the CLI.
- **Non-interactive by default** — no prompts; every option has a flag equivalent.
- **Idempotent read operations** — all three are pure GETs.
- **Pipe-friendly** — `jr user search jane --output json | jq '.[0].accountId' | xargs jr user view` works cleanly.
