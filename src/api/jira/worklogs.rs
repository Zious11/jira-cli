use crate::api::client::JiraClient;
use crate::api::pagination::OffsetPage;
use crate::types::jira::Worklog;
use anyhow::Result;
use serde_json::Value;

impl JiraClient {
    /// Add a worklog entry to an issue.
    ///
    /// `time_spent` is passed verbatim to the Jira API's `timeSpent` field.
    /// Jira Cloud parses the string server-side (e.g. `"1d"`, `"2d 3h 30m"`).
    /// The caller is responsible for validating `time_spent` before calling this
    /// function (use `duration::parse_duration_validate`).
    pub async fn add_worklog(
        &self,
        key: &str,
        time_spent: &str,
        comment: Option<Value>,
    ) -> Result<Worklog> {
        let path = format!("/rest/api/3/issue/{}/worklog", urlencoding::encode(key));
        let mut body = serde_json::json!({
            "timeSpent": time_spent,
        });
        if let Some(c) = comment {
            body["comment"] = c;
        }
        self.post(&path, &body).await
    }

    /// List all worklogs on an issue, paginating until all pages are fetched.
    ///
    /// BC-X.5.002: iterates with offset-based pagination via `OffsetPage::has_more` /
    /// `OffsetPage::next_start` until all pages are consumed.
    // NFR-O-T: The worklog endpoint returns a PageBean<Worklog> envelope with
    // offset-based startAt/maxResults pagination. Atlassian does not contractually
    // document a default page size (per JRACLOUD-67570: "default and maximum sizes
    // of paged data are not considered part of the API"). Observed behaviour as of
    // 2026-05 returns up to ~5000 worklogs per page; treat this as best-effort and
    // always loop on pagination, never assume a single call returns everything.
    pub async fn list_worklogs(&self, key: &str) -> Result<Vec<Worklog>> {
        let base_path = format!("/rest/api/3/issue/{}/worklog", urlencoding::encode(key));
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
}
