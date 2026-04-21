# User Search Pagination Spec

**Issue:** #189 — true multi-page pagination for `jr user search --all` and `jr user list --all`.

## Problem

`jr user search --all` and `jr user list --all` (added in #188) currently wrap `JiraClient::search_users` and `JiraClient::search_assignable_users_by_project`, which call the Jira API **once** with no `startAt` / `maxResults` query parameters. This caps results at Jira's single-page default (~50 users) even under `--all`. The `--all` flag in these commands means only "disable the local 30-row cap," not "paginate to exhaustion" — a misleading gap documented in `docs/superpowers/specs/2026-04-13-user-search-lookup-design.md` and the current help text.

This spec brings `--all` up to what users expect: paginate through every server page until the result set is exhausted.

## Scope

**In scope:**

- New `JiraClient::search_users_all(query)` method that paginates `/rest/api/3/user/search`.
- New `JiraClient::search_assignable_users_by_project_all(query, project_key)` method that paginates `/rest/api/3/user/assignable/multiProjectSearch`.
- CLI wiring in `src/cli/user.rs`: when `--all` is set, call the `_all` variant; otherwise keep the existing single-call behavior.
- Updated help text on `--all` for `user search` and `user list`.

**Out of scope:**

- `search_assignable_users(query, issue_key)` — the issue-key variant used by `issue assign` / `issue create` disambiguation helpers. No CLI command calls it with `--all` today. If a future command needs paginated assignable-by-issue lookup, a third `_all` method can be added then.
- Name-lookup call sites in `src/cli/issue/helpers.rs` — they stay on the single-call path.
- Any change to the `--limit N` behavior (stays a purely local truncate, no pagination triggered).
- Deduplication of results across pages — Atlassian docs do not document this as a risk; adding dedup preemptively is YAGNI.

## Validated API Facts

From the official Atlassian REST API v3 documentation (`api-group-user-search`):

- Both `/rest/api/3/user/search` and `/rest/api/3/user/assignable/multiProjectSearch` accept `startAt` (integer) and `maxResults` (integer) query parameters.
- Both endpoints return a **flat JSON array** of User objects — no envelope, no `total`, no `isLast`, no `nextPageToken`. The existing helpers in `src/api/pagination.rs` do not apply.
- Both endpoints are subject to a documented **hard cap of 1000 users**: "the operations in this resource only return users found within the first 1000 users."
- The docs warn that responses "usually return fewer users than specified in `maxResults`" because filtering happens *after* the server selects a page from the first 1000. **Short-page-as-end-of-data is NOT a reliable termination signal.** The only reliable termination signal for these endpoints is an empty-array response.
- The Atlassian developer community confirms `maxResults` is effectively capped at 100 server-side, even if you request more.

## Design

### Architecture

Follow the octocrab `all_pages()` idiom: keep the single-call methods untouched for small name-lookup callers, and add separate `_all` wrapper methods that loop on top of a private `_page(start, max)` helper. Rejected alternatives:

- Extending the existing single-call methods with `start_at` / `max_results` parameters. Changes the public API for callers that don't care (helpers.rs) and raises the chance of accidental pagination from a refactor.
- Returning an async `Stream<Item = User>`. No other place in this codebase uses that pattern; YAGNI for one feature.

### API shape

```rust
impl JiraClient {
    // Existing — unchanged public signature:
    pub async fn search_users(&self, query: &str) -> Result<Vec<User>>;
    pub async fn search_assignable_users_by_project(&self, query: &str, project_key: &str) -> Result<Vec<User>>;

    // New public methods:
    pub async fn search_users_all(&self, query: &str) -> Result<Vec<User>>;
    pub async fn search_assignable_users_by_project_all(&self, query: &str, project_key: &str) -> Result<Vec<User>>;

    // New private helpers used only by the `_all` variants:
    async fn search_users_page(&self, query: &str, start_at: u32, max_results: u32) -> Result<Vec<User>>;
    async fn search_assignable_users_by_project_page(
        &self,
        query: &str,
        project_key: &str,
        start_at: u32,
        max_results: u32,
    ) -> Result<Vec<User>>;
}
```

The existing deserialization logic (flat-array-or-object-with-values) is preserved inside the page helpers so the `_all` methods inherit robustness to the response-shape variance the repo already handles.

### Pagination loop

```rust
const USER_PAGE_SIZE: u32 = 100;           // Atlassian effective server cap.
const USER_PAGINATION_SAFETY_CAP: u32 = 15; // Documented cap is 1000 users = 10 iterations; 15 gives 50% headroom.

async fn search_users_all(&self, query: &str) -> Result<Vec<User>> {
    let mut all = Vec::new();
    let mut start_at: u32 = 0;
    for _ in 0..USER_PAGINATION_SAFETY_CAP {
        let page = self.search_users_page(query, start_at, USER_PAGE_SIZE).await?;
        if page.is_empty() {
            break;
        }
        let fetched = page.len() as u32;
        all.extend(page);
        start_at = start_at.saturating_add(fetched);
    }
    Ok(all)
}
```

Key properties:

- **Termination:** empty response only. No `isLast`/`total` to rely on; short pages are filtering artifacts, not end-of-data.
- **Advance by actual page length**, not by `max_results`. This is the correct `startAt` for the next window even when the server returns <100 results.
- **Safety cap: 15 iterations.** Defensive guard against pathological server behavior; generous vs the documented 1000-user hard cap (10 iterations at page 100).
- **Error handling: abort.** Any `?` propagation from a page fetch aborts the loop and returns the error. No retry, no partial-success. Matches `atlassian-python-api` and `jira-node` behavior (validated via Perplexity).
- **Rate limiting** is inherited from `JiraClient::get()` which already handles 429 + `Retry-After` via `src/api/rate_limit.rs`.

### CLI wiring

`src/cli/user.rs::handle_search` and `handle_list`: branch on `all`.

```rust
async fn handle_search(
    query: &str,
    limit: Option<u32>,
    all: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let effective = resolve_effective_limit(limit, all);
    let mut users = if all {
        client.search_users_all(query).await?
    } else {
        client.search_users(query).await?
    };
    if let Some(cap) = effective {
        users.truncate(cap as usize);
    }
    print_user_list(&users, output_format)
}
```

Same shape in `handle_list` for the assignable-by-project path. `resolve_effective_limit(limit, all)` returns `None` when `--all` is set, so the post-fetch truncate is skipped — `--all` continues to mean "don't truncate locally," now combined with "paginate all server pages."

### Help text

Update the `user search --all` and `user list --all` doc comments in `src/cli/mod.rs`:

- Before: "Disable the default 30-row cap. Returns only what the single API call returned."
- After: "Fetch all matching users by paginating through every API page (up to the Jira 1000-user hard cap)."

Follows kubectl/doctl style — stay high-level, avoid leaking page-size internals.

## Tests

### Unit tests (`src/api/jira/users.rs`)

Existing deserialization tests stay. No new unit tests required because the pagination loop logic is exercised end-to-end via integration tests against wiremock (which reflects how the loop behaves against the actual HTTP surface — the thing we care about).

### Integration tests (new file: `tests/user_pagination.rs`)

- `search_users_all_paginates_and_concatenates` — three mocked pages (100, 100, 27) → asserts 227 users returned, in order, and all three mocks were hit once each (`.expect(1)`).
- `search_users_all_stops_on_empty_page` — two full pages + one empty page → asserts 200 users returned and the fourth request is never made.
- `search_users_all_respects_safety_cap` — every mock returns 100 items; asserts exactly 15 requests are made (`.expect(15)`) and the 16th never fires.
- `search_users_all_propagates_error_mid_pagination` — page 2 returns 500; asserts the command exits nonzero and no subsequent page request fires.
- `user_search_all_cli_paginates` — end-to-end `jr user search --all foo` with mocked pagination returns 150+ users in the JSON output.
- `user_list_all_cli_paginates` — end-to-end `jr user list --all --project PROJ` with mocked pagination.
- `user_search_no_all_issues_single_request` — without `--all`, only one request is made regardless of how many users would be available (`.expect(1)` on the single-call path).

Mock tightness: each pagination stub constrains `query_param("startAt", "...")` so a misaligned loop (e.g., advancing by `max_results` instead of actual page length) trips the test instead of silently passing.

### Helper tests (existing, `tests/cli_handler.rs`)

No new tests required — existing name-lookup tests (`issue assign --to`, `issue create --to`, etc.) continue to prove the single-call path stays wired up because the public signature of `search_users` and `search_assignable_users_by_project` doesn't change.

## Backwards Compatibility

No public API or CLI behavior changes for users who don't pass `--all`:

- `search_users(query)` / `search_assignable_users_by_project(query, project_key)` — signature and semantics unchanged.
- `user search QUERY` (no `--all`) — one request, truncate to 30, print. Unchanged.
- `user search QUERY --limit 50` — one request, truncate to 50, print. Unchanged.
- `user search QUERY --all` — was: one request, no truncate, print (up to ~50 results). **Now**: paginate all pages, no truncate, print (up to 1000 results).
- Same three cases for `user list --project PROJ`.

The `--all` behavior change is the feature itself and aligns with what #189 and the help text promise. Not a regression.

## Risks & Mitigations

- **Slow queries:** paginating 10 pages at ~200–500ms each = 2–5s wall time. No progress feedback (matches AWS CLI and gh CLI precedent; validated via Perplexity). Users running interactively see silent wait; this is CLI-industry-standard.
- **Rate limits:** inherited `Retry-After` handling in `JiraClient::get()` covers 429s automatically. No loop-level concern.
- **Server cap evolution:** if Atlassian raises the 1000-user cap, the safety cap of 15 iterations becomes the new ceiling. This is acceptable — users hitting that are edge cases and can bump the const in a follow-up.
- **Misaligned `startAt` in the loop:** guarded by the integration test that constrains `query_param("startAt", ...)` per page.

## Out of Scope / Follow-ups

- Pagination for `/rest/api/3/user/assignable/search` (issue-key variant). File follow-up if a CLI command ever needs it.
- Any UX for users who want "fetch up to N via pagination where N > single-page": explicitly a non-goal. `--all` means all; `--limit N` is a local cap.
