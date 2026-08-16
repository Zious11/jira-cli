use anyhow::Result;

use crate::api::client::JiraClient;
use crate::types::jira::component::{Component, RelatedIssueCounts};

impl JiraClient {
    /// Fetch all components for a project.
    ///
    /// GETs `/rest/api/3/project/{project_key}/components`.
    /// Assumed non-paginated per BC-8.1.001 — pending F4 live verification.
    pub async fn list_components(&self, project_key: &str) -> Result<Vec<Component>> {
        let path = format!("/rest/api/3/project/{}/components", project_key);
        self.get(&path).await
    }

    /// Fetch the related issue count for a single component.
    ///
    /// GETs `/rest/api/3/component/{component_id}/relatedIssueCounts`.
    /// Called per-component by `jr component list --counts` (BC-8.1.003).
    pub async fn get_related_issue_counts(&self, component_id: &str) -> Result<RelatedIssueCounts> {
        let path = format!("/rest/api/3/component/{}/relatedIssueCounts", component_id);
        self.get(&path).await
    }
}
