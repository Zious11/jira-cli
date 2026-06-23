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

**Correct pattern (offset-pagination loop):**

```rust
pub async fn list_worklogs(&self, key: &str) -> Result<Vec<Worklog>> {
    let base_path = format!("/rest/api/3/issue/{}/worklog", key);
    let mut all_items: Vec<Worklog> = Vec::new();
    let mut start_at: u32 = 0;
    loop {
        let path = format!("{}?startAt={}", base_path, start_at);
        let page: OffsetPage<Worklog> = self.get(&path).await?;
        let has_more = page.has_more();
        let next = page.next_start();
        all_items.extend_from_slice(page.items());
        if !has_more {
            break;
        }
        start_at = next;
    }
    Ok(all_items)
}
```

## Decision

Refactor `list_worklogs` to use an inline `loop { … has_more() … next_start() … }` over
`OffsetPage<Worklog>`, collecting all pages before returning. This is the established
offset-pagination pattern in `src/api/jira/` — the same approach used by other
paginated endpoints in the codebase.

## Rationale

- The offset-pagination loop is the codebase's established pattern for
  `OffsetPage<T>`-returning endpoints. Using it here is consistent.
- There is no valid reason to want only the first page of worklogs. The previous behavior
  was a bug, not a deliberate design choice.
- Silent data truncation is a HIGH-severity reliability issue — users building reports
  from `jr worklog list` output would get incorrect totals with no warning.

## Consequences

- **Fix scope:** ~15 LOC in `src/api/jira/worklogs.rs::list_worklogs`.
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
- `src/api/pagination.rs` — `OffsetPage<T>` type providing `has_more()` / `next_start()`
