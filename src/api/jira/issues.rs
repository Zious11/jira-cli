use crate::api::client::JiraClient;
use crate::api::pagination::{CursorPage, OffsetPage};
use crate::types::jira::{Comment, CreateIssueResponse, Issue, TransitionsResponse};
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

/// Default fields requested when fetching issues (search and get).
///
/// Both `search_issues` and `get_issue` use this list so they stay in sync.
/// Callers can request additional fields via `extra_fields` parameters.
const BASE_ISSUE_FIELDS: &[&str] = &[
    "summary",
    "status",
    "issuetype",
    "priority",
    "assignee",
    "reporter",
    "project",
    "description",
    "created",
    "updated",
    "resolution",
    "components",
    "fixVersions",
    "labels",
    "parent",
    "issuelinks",
];

/// Result of a paginated issue search, including whether more results exist.
pub struct SearchResult {
    pub issues: Vec<Issue>,
    pub has_more: bool,
}

/// Result of a keys-only paginated issue search.
///
/// Parallel to [`SearchResult`]. The field name `keys` mirrors the `issues`
/// field name on `SearchResult` (domain-named, not generic `items`) per the
/// Rust SDK precedent surveyed in
/// `.factory/research/issue-350-search-issue-keys-design.md`.
///
/// `has_more` is set to `true` in two cases:
///
/// 1. **Caller limit hit:** the caller supplied a `limit` and upstream still
///    had rows available beyond that limit.
/// 2. **JRACLOUD-94632 guard abort:** the anti-loop guard fired (upstream
///    returned the same `nextPageToken` twice), pagination was aborted with
///    a stderr warning, and results may be incomplete due to a server bug.
///
/// Pure cursor exhaustion (no limit set, upstream returns `isLast: true`)
/// always returns `has_more = false`. Callers that need completeness
/// guarantees should treat `has_more = true` as "results may be truncated"
/// regardless of whether a `limit` was supplied.
///
/// Traces to BC-2.6.050.
#[derive(Debug, Clone, PartialEq)]
pub struct KeySearchResult {
    pub keys: Vec<String>,
    pub has_more: bool,
}

#[derive(Deserialize)]
struct ApproximateCountResponse {
    count: u64,
}

/// Minimal deserialization target for `search_issue_keys`.
///
/// Reads ONLY the top-level `key` field per issue. Atlassian guarantees
/// `key` is at the top level regardless of the request body's `fields`
/// value; the `fields` map itself is intentionally not deserialized here
/// to keep the payload contract narrow.
///
/// Does NOT use `#[serde(deny_unknown_fields)]` — Atlassian routinely
/// adds top-level response fields and the SDK convention is to ignore
/// unknowns silently.
///
/// `Default` is required here, but NOT for the fundamental reason one might
/// assume: `Vec<T>::default()` returns `vec![]` for ANY `T`, so the runtime
/// semantics of `#[serde(default)]` on `CursorPage::issues: Vec<T>` do NOT
/// actually require `T: Default`.
///
/// The requirement comes from serde-derive's CONSERVATIVE bound-inference
/// algorithm: when `#[serde(default)]` appears on a field of type `Vec<T>`,
/// the generated `Deserialize` impl adds `T: Default` to its where-clause
/// regardless of whether the bound is logically necessary. Removing `Default`
/// from `IssueKeyRow` produces:
///   error[E0277]: the trait bound `IssueKeyRow: Default` is not satisfied
///   required for `CursorPage<IssueKeyRow>` to implement `DeserializeOwned`
/// This is a known serde-derive limitation (conservative macro bounds), not a
/// fundamental requirement of `Vec` or serde's runtime logic. The `Default`
/// value is never used at runtime.
#[derive(Deserialize, Default)]
struct IssueKeyRow {
    key: String,
}

impl JiraClient {
    /// Search issues using JQL with cursor-based pagination.
    pub async fn search_issues(
        &self,
        jql: &str,
        limit: Option<u32>,
        extra_fields: &[&str],
    ) -> Result<SearchResult> {
        let max_per_page = limit.unwrap_or(50).min(100);
        let mut all_issues: Vec<Issue> = Vec::new();
        let mut next_page_token: Option<String> = None;

        let mut fields = BASE_ISSUE_FIELDS.to_vec();
        fields.extend_from_slice(extra_fields);

        let mut more_available = false;

        // Anti-loop guard: Jira Cloud /rest/api/3/search/jql intermittently returns
        // the same nextPageToken twice, causing infinite pagination loops. This is a
        // confirmed upstream bug — JRACLOUD-94632, JRACLOUD-92049, JRACLOUD-85546
        // (also reported in atlassian/atlassian-mcp-server#118 and
        // ankitpokhrel/jira-cli#898). Mirrors anti-loop pattern from get_changelog
        // (lines 222-230). When the guard fires, we emit a stderr warning citing
        // JRACLOUD-94632 so users have a search term.
        let mut prev_cursor: Option<String> = None;

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": max_per_page,
                "fields": fields
            });

            if let Some(ref token) = next_page_token {
                body["nextPageToken"] = serde_json::json!(token);
            }

            let page: CursorPage<Issue> = self.post("/rest/api/3/search/jql", &body).await?;

            let page_has_more = page.has_more();
            let next_cursor = page.next_page_token.clone();
            all_issues.extend(page.issues);

            if let Some(max) = limit {
                if all_issues.len() >= max as usize {
                    more_available = all_issues.len() > max as usize || page_has_more;
                    all_issues.truncate(max as usize);
                    break;
                }
            }

            if !page_has_more {
                break;
            }

            // GUARD: detect repeated cursor token (next == prev) → abort + warn.
            // NFR-R-F: prevents infinite loop when server returns the same
            // nextPageToken twice (confirmed upstream bug JRACLOUD-94632).
            // Stderr-literal pin: tests/rate_limit_cap_tests.rs::ac_008_and_ac_new_d_search_jql_cursor_loop_terminates_with_jracloud_warning
            // Do NOT change the literal "JRACLOUD-94632" without updating that test.
            if next_cursor.is_some() && next_cursor == prev_cursor {
                eprintln!(
                    "[jr] WARNING: Atlassian /rest/api/3/search/jql returned the same \
                     nextPageToken twice — aborting pagination loop. Some results may \
                     be missing. Upstream bug: JRACLOUD-94632."
                );
                break;
            }

            prev_cursor = next_cursor.clone();
            next_page_token = next_cursor;
        }

        Ok(SearchResult {
            issues: all_issues,
            has_more: more_available,
        })
    }

    /// Search issues using JQL and return ONLY the matching issue keys.
    ///
    /// Lightweight variant of [`Self::search_issues`] — requests
    /// `fields: ["key"]` in the POST body and deserializes only the
    /// top-level `key`, avoiding the ~10 KB-per-issue payload that
    /// `BASE_ISSUE_FIELDS` incurs.
    ///
    /// Use this when the caller only needs keys (e.g., JQL-driven
    /// bulk-edit selection at `cli/issue/create.rs::handle_edit`). For
    /// body-bearing reads use [`Self::search_issues`].
    ///
    /// `has_more` is set to `true` in two cases: (1) the caller's `limit`
    /// was hit AND upstream still had rows available, or (2) the
    /// JRACLOUD-94632 anti-loop guard fired (upstream returned the same
    /// `nextPageToken` twice), aborting pagination early with a stderr
    /// warning. Pure cursor exhaustion returns `has_more = false`. See
    /// [`KeySearchResult`] for the full contract.
    ///
    /// The per-page clamp `.min(100)` is a conservative client-side choice
    /// for parity with `search_issues`; Atlassian docs note that id/key-only
    /// requests can return more rows per page than full-body requests.
    ///
    /// Traces to BC-2.6.050.
    pub async fn search_issue_keys(
        &self,
        jql: &str,
        limit: Option<u32>,
    ) -> Result<KeySearchResult> {
        let max_per_page = limit.unwrap_or(50).min(100);
        let mut all_keys: Vec<String> = Vec::new();
        let mut next_page_token: Option<String> = None;

        let mut more_available = false;

        // Anti-loop guard: Jira Cloud /rest/api/3/search/jql intermittently returns
        // the same nextPageToken twice, causing infinite pagination loops. This is a
        // confirmed upstream bug — JRACLOUD-94632, JRACLOUD-92049, JRACLOUD-85546
        // (also reported in atlassian/atlassian-mcp-server#118 and
        // ankitpokhrel/jira-cli#898). Mirrors the guard in `search_issues`.
        let mut prev_cursor: Option<String> = None;

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": max_per_page,
                "fields": ["key"]
            });

            if let Some(ref token) = next_page_token {
                body["nextPageToken"] = serde_json::json!(token);
            }

            let page: CursorPage<IssueKeyRow> = self.post("/rest/api/3/search/jql", &body).await?;

            let page_has_more = page.has_more();
            let next_cursor = page.next_page_token.clone();
            all_keys.extend(page.issues.into_iter().map(|r| r.key));

            if let Some(max) = limit {
                if all_keys.len() >= max as usize {
                    // `all_keys.len() > max` handles the Apr 2025 regression
                    // (JRACLOUD-95368) where the server overshoots maxResults AND
                    // sets isLast:true — the overshoot proves more data existed.
                    // `page_has_more` handles the normal "server said more pages" case.
                    // Do NOT simplify to `page_has_more` alone — that would miss the
                    // regression scenario.
                    more_available = all_keys.len() > max as usize || page_has_more;
                    all_keys.truncate(max as usize);
                    break;
                }
            }

            if !page_has_more {
                break;
            }

            // GUARD: detect repeated cursor token (next == prev) → abort + warn.
            // Mirrors the JRACLOUD-94632 guard block in search_issues above — same prev/next cursor check, same stderr warning text.
            // Stderr-literal pin: tests/search_issue_keys.rs::test_search_issue_keys_stderr_emits_jracloud_94632_literal
            // Do NOT change the literal "JRACLOUD-94632" without updating that test.
            if next_cursor.is_some() && next_cursor == prev_cursor {
                eprintln!(
                    "[jr] WARNING: Atlassian /rest/api/3/search/jql returned the same \
                     nextPageToken twice — aborting pagination loop. Some results may \
                     be missing. Upstream bug: JRACLOUD-94632."
                );
                // Guard-aborted: signal incomplete results via has_more=true so callers
                // can distinguish "clean exhaustion" from "server-bug abort". Atlassian's
                // API guarantees non-overlapping pages even when the cursor token repeats
                // (JRACLOUD-94632 is a cursor metadata bug, not a duplicate-data bug), so
                // all_keys collected so far are unique — but the result set is incomplete.
                more_available = true;
                break;
            }

            prev_cursor = next_cursor.clone();
            next_page_token = next_cursor;
        }

        Ok(KeySearchResult {
            keys: all_keys,
            has_more: more_available,
        })
    }

    /// Get an approximate count of issues matching a JQL query.
    ///
    /// Uses the dedicated count endpoint which is lightweight (no issue data fetched).
    /// The JQL should not include ORDER BY — use `jql::strip_order_by()` before calling.
    pub async fn approximate_count(&self, jql: &str) -> Result<u64> {
        let body = serde_json::json!({ "jql": jql });
        let resp: ApproximateCountResponse = self
            .post("/rest/api/3/search/approximate-count", &body)
            .await?;
        Ok(resp.count)
    }

    /// Get a single issue by key.
    pub async fn get_issue(&self, key: &str, extra_fields: &[&str]) -> Result<Issue> {
        let mut fields: Vec<&str> = BASE_ISSUE_FIELDS.to_vec();
        fields.extend_from_slice(extra_fields);
        let path = format!(
            "/rest/api/3/issue/{}?fields={}",
            urlencoding::encode(key),
            fields.join(",")
        );
        self.get(&path).await
    }

    /// Create a new issue.
    pub async fn create_issue(&self, fields: Value) -> Result<CreateIssueResponse> {
        let body = serde_json::json!({ "fields": fields });
        self.post("/rest/api/3/issue", &body).await
    }

    /// Edit an existing issue's fields.
    pub async fn edit_issue(&self, key: &str, fields: Value) -> Result<()> {
        let path = format!("/rest/api/3/issue/{}", urlencoding::encode(key));
        let body = serde_json::json!({ "fields": fields });
        self.put(&path, &body).await
    }

    /// Get available transitions for an issue.
    pub async fn get_transitions(&self, key: &str) -> Result<TransitionsResponse> {
        let path = format!("/rest/api/3/issue/{}/transitions", urlencoding::encode(key));
        self.get(&path).await
    }

    /// Transition an issue to a new status, optionally setting extra fields
    /// in the same request (e.g. `resolution`). Passing `fields = None`
    /// preserves the pre-existing behaviour of sending only the transition id.
    ///
    /// When `fields` is `Some(&json)`, the value is merged as-is under the
    /// `fields` key of the request body — callers are responsible for shaping
    /// it correctly (Atlassian expects `{"resolution": {"name": "Done"}}` or
    /// `{"resolution": {"id": "10000"}}`).
    pub async fn transition_issue(
        &self,
        key: &str,
        transition_id: &str,
        fields: Option<&serde_json::Value>,
    ) -> Result<()> {
        let path = format!("/rest/api/3/issue/{}/transitions", urlencoding::encode(key));
        let body = match fields {
            Some(f) => serde_json::json!({
                "transition": { "id": transition_id },
                "fields": f,
            }),
            None => serde_json::json!({
                "transition": { "id": transition_id }
            }),
        };
        self.post_no_content(&path, &body).await
    }

    /// Assign an issue. Pass `None` for account_id to unassign.
    pub async fn assign_issue(&self, key: &str, account_id: Option<&str>) -> Result<()> {
        let path = format!("/rest/api/3/issue/{}/assignee", urlencoding::encode(key));
        let body = serde_json::json!({
            "accountId": account_id
        });
        self.put(&path, &body).await
    }

    /// Add a comment to an issue.
    ///
    /// When `internal` is true, sets the `sd.public.comment` entity property
    /// to mark the comment as internal (agent-only) on JSM projects.
    /// On non-JSM projects, the property is silently accepted with no effect.
    pub async fn add_comment(&self, key: &str, body: Value, internal: bool) -> Result<Comment> {
        let path = format!("/rest/api/3/issue/{}/comment", urlencoding::encode(key));
        let mut payload = serde_json::json!({ "body": body });
        if internal {
            payload["properties"] = serde_json::json!([{
                "key": "sd.public.comment",
                "value": { "internal": true }
            }]);
        }
        self.post(&path, &payload).await
    }

    /// Fetch the full audit changelog for an issue.
    ///
    /// Offset-paginated under `values[]`. Always fetches every page;
    /// sort/filter/truncate are the caller's responsibility — the Jira
    /// changelog endpoint supports no server-side filters and does not
    /// guarantee sort order.
    pub async fn get_changelog(
        &self,
        key: &str,
    ) -> Result<Vec<crate::types::jira::ChangelogEntry>> {
        let base = format!("/rest/api/3/issue/{}/changelog", urlencoding::encode(key));
        let mut all = Vec::new();
        let mut start_at = 0u32;
        let max_page_size: u32 = 100;

        loop {
            let path = format!("{}?startAt={}&maxResults={}", base, start_at, max_page_size);
            let page: OffsetPage<crate::types::jira::ChangelogEntry> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values.unwrap_or_default());

            if !has_more {
                break;
            }
            // Guard against an API response that advertises more pages but
            // returns a page that wouldn't advance `startAt` — otherwise we'd
            // infinite-loop on a malformed/empty page (JRACLOUD-94357-class
            // schema-drift scenarios). Surface as an explicit error instead.
            if next <= start_at {
                return Err(anyhow::anyhow!(
                    "Jira changelog pagination did not advance (startAt {} → {}) \
                     despite has_more=true. The server returned a malformed page; \
                     retry later or report to Jira support.",
                    start_at,
                    next
                ));
            }
            start_at = next;
        }

        Ok(all)
    }

    /// List comments on an issue with auto-pagination.
    pub async fn list_comments(&self, key: &str, limit: Option<u32>) -> Result<Vec<Comment>> {
        let base = format!("/rest/api/3/issue/{}/comment", urlencoding::encode(key));
        let mut all = Vec::new();
        let mut start_at = 0u32;
        let max_page_size: u32 = 100;

        loop {
            let page_size = match limit {
                Some(cap) => {
                    let remaining = cap.saturating_sub(all.len() as u32);
                    if remaining == 0 {
                        break;
                    }
                    remaining.min(max_page_size)
                }
                None => max_page_size,
            };
            let path = format!(
                "{}?startAt={}&maxResults={}&expand=properties",
                base, start_at, page_size
            );
            let page: OffsetPage<Comment> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.append(&mut page.comments.unwrap_or_default());

            if let Some(cap) = limit {
                if all.len() >= cap as usize {
                    all.truncate(cap as usize);
                    break;
                }
            }
            if !has_more {
                break;
            }
            start_at = next;
        }
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_result_has_more_false_when_no_truncation() {
        let result = SearchResult {
            issues: vec![],
            has_more: false,
        };
        assert!(!result.has_more);
    }

    #[test]
    fn search_result_has_more_true_when_truncated() {
        let result = SearchResult {
            issues: vec![],
            has_more: true,
        };
        assert!(result.has_more);
    }

    #[test]
    fn approximate_count_response_deserializes() {
        let json = r#"{"count": 1234}"#;
        let resp: ApproximateCountResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.count, 1234);
    }

    #[test]
    fn approximate_count_response_zero() {
        let json = r#"{"count": 0}"#;
        let resp: ApproximateCountResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.count, 0);
    }
}
