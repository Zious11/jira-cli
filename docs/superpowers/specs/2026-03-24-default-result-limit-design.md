# Default Result Limit for Issue List — Design Spec

**Goal:** Add a sensible default result limit to `jr issue list` so large projects don't dump thousands of rows into the terminal.

**Problem:** `jr issue list --project <KEY>` returns all issues with no default limit. On projects with thousands of issues, this produces megabytes of unusable output. The only way to control it is `--limit`, which is optional and defaults to unlimited.

**Addresses:** [GitHub Issue #43](https://github.com/Zious11/jira-cli/issues/43)

---

## Architecture

The feature touches three layers: CLI argument parsing, the API search method's return type, and a new lightweight API call for approximate counts. No new code files or modules are introduced — all runtime changes fit within existing modules.

**Precedent:** GitHub CLI (`gh issue list`) defaults to 30 results with `-L/--limit` to override. It has no `--all` flag and no truncation message. We improve on this by adding both `--all` and a truncation hint with approximate total count.

---

## 1. CLI Changes

### New Flag: `--all`

Add `--all` boolean flag to `IssueCommand::List`, mutually exclusive with `--limit` via clap's `conflicts_with`:

```rust
/// Fetch all results (no default limit)
#[arg(long, conflicts_with = "limit")]
all: bool,
```

Clap automatically rejects `--all --limit 50` with a clear error message.

### Default Limit Constant

```rust
const DEFAULT_LIMIT: u32 = 30;
```

### Effective Limit Resolution

```rust
let effective_limit = if all {
    None              // unlimited — current behavior
} else {
    Some(limit.unwrap_or(DEFAULT_LIMIT))
};
```

| User input | `effective_limit` | Behavior |
|------------|-------------------|----------|
| (nothing) | `Some(30)` | Default 30 results |
| `--limit 50` | `Some(50)` | Explicit 50 results |
| `--all` | `None` | Unlimited (current behavior) |
| `--all --limit 50` | N/A | Clap rejects with conflict error |

### Scope

Only `jr issue list` gets the default limit. Other list-like commands (`queue view`, `assets search`, `sprint current`) are unaffected — they can be standardized in a follow-up issue.

---

## 2. API Layer Changes

### New Return Type: `SearchResult`

`search_issues()` currently returns `Result<Vec<Issue>>`. The `has_more` signal from the last page's `next_page_token` is computed but discarded. Change the return type to preserve it:

```rust
pub struct SearchResult {
    pub issues: Vec<Issue>,
    pub has_more: bool,
}
```

The `has_more` field is `true` when either: (a) the API's `next_page_token` is present when we stop fetching, OR (b) we fetched more issues than the effective limit and truncated (i.e., `all_issues.len() > limit` before truncation). This covers two distinct break paths: the limit was reached mid-page (where the API may say no more pages, but we still truncated) and the API indicates more pages exist.

All callers of `search_issues()` must be updated to destructure `SearchResult` instead of `Vec<Issue>`.

### New Method: `approximate_count()`

```rust
pub async fn approximate_count(&self, jql: &str) -> Result<u64> {
    let body = serde_json::json!({ "jql": jql });
    let resp: ApproximateCountResponse = self
        .post("/rest/api/3/search/approximate-count", &body)
        .await?;
    Ok(resp.count)
}
```

Response struct (file-private):

```rust
#[derive(Deserialize)]
struct ApproximateCountResponse {
    count: u64,
}
```

**JQL preparation:** Strip `ORDER BY` clauses from the JQL before passing to `approximate_count()` — ordering is meaningless for a count query and the endpoint requires bounded JQL. A simple `jql.split(" ORDER BY").next()` or regex suffices.

**Endpoint details (validated via Perplexity):**
- `POST /rest/api/3/search/approximate-count`
- Request: `{"jql": "project = PROJ"}`
- Response: `{"count": 36}`
- Requires only `Browse projects` permission (standard, no special scopes)
- Returns `{"count": 0}` for zero matches (200 OK)
- Returns 400 for invalid JQL (won't happen — we use the same JQL that just succeeded)
- Count is approximate — recent updates may lag slightly
- Available on all Jira Cloud plans

---

## 3. Truncation Message

### When It Fires

Only when **all three conditions** are met:
1. `has_more == true` (results were truncated)
2. `--all` was not passed
3. The search returned at least one issue

### Flow

1. `search_issues(jql, effective_limit)` → `SearchResult { issues, has_more }`
2. Render table/JSON to stdout
3. If `has_more` → call `approximate_count(jql)`
4. Print hint to **stderr**

### Message Format

```
Showing 30 of ~1234 results. Use --limit or --all to see more.
```

The tilde (`~`) indicates the count is approximate.

### Graceful Degradation

If `approximate_count()` fails (network error, unexpected 403, etc.), fall back to a message without the total:

```
Showing 30 results. Use --limit or --all to see more.
```

### Why stderr

The message goes to stderr so it doesn't pollute piped output (`jr issue list | grep`) or JSON output (`--output json`). This follows the CLI composability principle: stdout is for data, stderr is for humans.

### JSON Output

`--output json` continues to emit the issues array to stdout unchanged. Truncation metadata (e.g., `"truncated": true, "approximate_total": 1234`) in the JSON body is out of scope for this change — the stderr hint is sufficient for now. A structured JSON envelope can be added in a follow-up if scripting users need programmatic truncation detection.

---

## 4. Error Handling & Edge Cases

### approximate_count Fails

Degrade gracefully — show hint without total. Never fail the command because of a count call.

### Zero Results

`search_issues` returns 0 issues with `has_more: false`. No truncation message shown. No approximate count call.

### --all on Small Result Set

`search_issues` returns all issues with `has_more: false`. No truncation message. No extra API call.

### Race Condition: Issues Deleted Between Search and Count

`approximate_count` returns a lower number than `issues.len()`. Harmless — the tilde already signals approximation. If count is 0, skip the message.

### JQL Matches Exactly the Limit

e.g., 30 issues exist, limit is 30. The API may return `has_more: false` if all fit in one page, or `has_more: true` if pagination boundaries don't align perfectly. Either way the behavior is correct: no false truncation messages (if `has_more: false`), or a harmless hint (if `has_more: true` and count returns ~30).

---

## 5. File Structure

### Modified Files

| File | Change |
|------|--------|
| `src/cli/mod.rs` | Add `all: bool` flag to `IssueCommand::List` with `conflicts_with = "limit"` |
| `src/cli/issue/list.rs` | Default limit logic (`DEFAULT_LIMIT`), pass `effective_limit` to search, truncation message to stderr after output |
| `src/api/jira/issues.rs` | New `SearchResult` struct, modify `search_issues()` return type to `Result<SearchResult>`, add `approximate_count()` method + `ApproximateCountResponse` |
| `src/cli/board.rs` | Update caller (line 69) to destructure `SearchResult` |
| `tests/issue_commands.rs` | Update `test_search_issues` and `test_search_issues_with_story_points` to destructure `SearchResult` |

### Potentially Modified (if they call `search_issues`)

| File | Change |
|------|--------|
| `src/cli/issue/assets.rs` | Update caller to destructure `SearchResult` (if it calls `search_issues`) |

### Not Changed

- `src/api/pagination.rs` — no changes to pagination structs
- `src/api/client.rs` — no new HTTP methods needed
- `src/config.rs` — no new config entries
- `src/cache.rs` — no caching for this feature

---

## 6. Testing Strategy

### Unit Tests

- `ApproximateCountResponse` deserializes `{"count": 36}` and `{"count": 0}` correctly
- Default limit logic: no flags → `Some(30)`, `--limit 50` → `Some(50)`, `--all` → `None`
- `SearchResult` propagates `has_more` correctly

### Integration Tests (wiremock)

- Search returns 30 results + `has_more: true` → approximate count endpoint called → stderr contains truncation message with `~` total
- Search returns 10 results + `has_more: false` → approximate count endpoint **not** called → stderr is empty
- `--all` flag → search called with no limit → no truncation message
- `--limit 50` → respects explicit limit → truncation message if more exist
- Approximate count endpoint returns 500 → graceful degradation → stderr contains hint without total
- Zero results → no truncation message

### Not Tested

- Clap `conflicts_with` behavior — this is clap's responsibility, well-tested upstream

---

## Validation Sources

| Decision | Validated by |
|----------|-------------|
| `gh issue list` defaults to 30, no `--all` flag | Perplexity (GitHub CLI docs) |
| kubectl defaults to all results, `--limit` to restrict | Perplexity |
| UX research: silent truncation causes users to mistake partial for complete | Perplexity (Baymard Institute research) |
| `POST /rest/api/3/search/approximate-count` exists, returns `{"count": N}` | Perplexity (Atlassian support KB + API docs) |
| Approximate count requires only `Browse projects` permission | Perplexity (Atlassian API docs) |
| Cursor-based JQL endpoint does NOT return `total` field | Perplexity (Atlassian developer docs) |
| Truncation messages should go to stderr for composability | Perplexity (CLI best practices) |
| Clap `conflicts_with` is bi-directional, auto-generates error | Perplexity (clap docs + Rust users forum) |
| `approximate-count` requires bounded JQL, ORDER BY may be unnecessary | Perplexity (Atlassian API docs) |
| Default 30 matches `gh` precedent for developer CLI tools | Perplexity (GitHub CLI docs + CLI conventions) |
| `nextPageToken` is null/absent when total equals maxResults (no false has_more) | Perplexity (Atlassian developer community) |
