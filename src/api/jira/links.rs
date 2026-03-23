use crate::api::client::JiraClient;
use crate::types::jira::issue::{IssueLinkType, IssueLinkTypesResponse};
use anyhow::Result;
use serde_json::json;

impl JiraClient {
    /// Create a link between two issues.
    /// outward_key gets the "outward" label (e.g., "blocks"),
    /// inward_key gets the "inward" label (e.g., "is blocked by").
    pub async fn create_issue_link(
        &self,
        outward_key: &str,
        inward_key: &str,
        link_type: &str,
    ) -> Result<()> {
        let body = json!({
            "outwardIssue": {"key": outward_key},
            "inwardIssue": {"key": inward_key},
            "type": {"name": link_type}
        });
        self.post_no_content("/rest/api/3/issueLink", &body).await
    }

    /// Delete an issue link by its ID.
    pub async fn delete_issue_link(&self, link_id: &str) -> Result<()> {
        let path = format!("/rest/api/3/issueLink/{}", link_id);
        self.delete(&path).await
    }

    /// List all available issue link types.
    pub async fn list_link_types(&self) -> Result<Vec<IssueLinkType>> {
        let resp: IssueLinkTypesResponse = self.get("/rest/api/3/issueLinkType").await?;
        Ok(resp.issue_link_types)
    }
}
