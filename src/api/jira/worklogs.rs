use crate::api::client::JiraClient;
use crate::api::pagination::OffsetPage;
use crate::types::jira::Worklog;
use anyhow::Result;
use serde_json::Value;

impl JiraClient {
    /// Add a worklog entry to an issue.
    pub async fn add_worklog(
        &self,
        key: &str,
        time_spent_seconds: u64,
        comment: Option<Value>,
    ) -> Result<Worklog> {
        let path = format!("/rest/api/3/issue/{}/worklog", urlencoding::encode(key));
        let mut body = serde_json::json!({
            "timeSpentSeconds": time_spent_seconds,
        });
        if let Some(c) = comment {
            body["comment"] = c;
        }
        self.post(&path, &body).await
    }

    /// List all worklogs on an issue, paginating until all pages are fetched.
    ///
    /// BC-X.5.002: iterates with offset-based pagination until `total <= start_at + count`.
    pub async fn list_worklogs(&self, key: &str) -> Result<Vec<Worklog>> {
        let base_path = format!("/rest/api/3/issue/{}/worklog", urlencoding::encode(key));
        let mut all_items: Vec<Worklog> = Vec::new();
        let mut start_at: usize = 0;

        loop {
            let path = format!("{}?startAt={}", base_path, start_at);
            let page: OffsetPage<Worklog> = self.get(&path).await?;
            let count = page.items().len();
            all_items.extend_from_slice(page.items());

            let fetched_up_to = start_at + count;
            if (page.total as usize) <= fetched_up_to || count == 0 {
                break;
            }
            start_at = fetched_up_to;
        }

        Ok(all_items)
    }
}
