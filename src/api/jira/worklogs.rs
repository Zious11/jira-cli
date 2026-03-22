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

    /// List all worklogs on an issue.
    pub async fn list_worklogs(&self, key: &str) -> Result<Vec<Worklog>> {
        let path = format!("/rest/api/3/issue/{}/worklog", urlencoding::encode(key));
        let page: OffsetPage<Worklog> = self.get(&path).await?;
        Ok(page.items().to_vec())
    }
}
