# ADR-0010: list_worklogs Must Use Pagination Loop

## Status
Accepted

## Context

A HIGH-severity data-loss bug was discovered in `list_worklogs` in
`src/api/jira/worklogs.rs`. The function fetched a single `OffsetPage<Worklog>` and
returned `.items().to_vec()` with no pagination loop. Atlassian's default page size for
the worklogs endpoint is 50. Any issue with more than `maxResults` worklogs was silently
truncated to the first page, with no indication to the user that records were omitted.

**Broken implementation:**

```rust
// single page fetch — silently truncates
pub async fn list_worklogs(&self, issue_key: &str) -> Result<Vec<Worklog>> {
    let page: OffsetPage<Worklog> = self.get(&format!("issue/{}/worklog", issue_key)).await?;
    Ok(page.items().to_vec())
}
```

**Correct pattern already in the codebase (`list_comments`):**

```rust
pub async fn list_comments(&self, issue_key: &str) -> Result<Vec<Comment>> {
    paginate_offset(|start| async move {
        self.get(&format!("issue/{}/comment?startAt={}", issue_key, start)).await
    }).await
}
```

Both worklogs and comments are on the same issue and use `OffsetPage<T>`. The comment
endpoint was correctly paginated; the worklog endpoint was not.

## Decision

Refactor `list_worklogs` to use the `paginate_offset` helper, identical in structure to
`list_comments`:

```rust
pub async fn list_worklogs(&self, issue_key: &str) -> Result<Vec<Worklog>> {
    paginate_offset(|start| async move {
        self.get(&format!("issue/{}/worklog?startAt={}", issue_key, start)).await
    }).await
}
```

## Rationale

- `paginate_offset` is the established pattern in `src/api/jira/` for offset-paginated
  endpoints. Using it here is consistent with the rest of the codebase.
- There is no valid reason to want only the first page of worklogs. The previous behavior
  was a bug, not a deliberate design choice.
- Silent data truncation is a HIGH-severity reliability issue — users building reports
  from `jr worklog list` output would get incorrect totals with no warning.

## Consequences

- **Fix scope:** ~10 LOC in `src/api/jira/worklogs.rs::list_worklogs`.
- **Regression risk:** LOW. The behavior change is: users now see all worklogs, not just
  the first page. Tests that assert a specific worklog count must account for pagination.
- **Test requirement:** Add a 2-page worklog integration test. Set up a mock that returns
  two pages (e.g., 50 + 3 items) and verify that `list_worklogs` returns all 53 items.
- **Performance note:** For issues with many hundreds of worklogs, the pagination loop
  may issue multiple HTTP calls. This is correct — users expect complete data, and
  worklogs are not a hot path.
- **BC anchor:** BC-X.5.002.

## See Also

- `src/api/jira/worklogs.rs::list_worklogs` — the paginated implementation
- `src/api/jira/issues.rs::list_comments` — the correct pagination pattern to mirror
- `src/api/pagination.rs::paginate_offset` — the shared offset-pagination helper
