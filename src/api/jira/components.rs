use anyhow::Result;
use serde_json::Value;

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

    /// Create a new component.
    ///
    /// POSTs `/rest/api/3/component`.
    /// Returns the created `Component`; the ADR-0018 §2 cache invalidation is
    /// the caller's responsibility (BC-8.2.008).
    pub async fn create_component(&self, body: &Value) -> Result<Component> {
        self.post("/rest/api/3/component", body).await
    }

    /// Edit an existing component.
    ///
    /// PUTs `/rest/api/3/component/{component_id}`.
    /// Returns the updated `Component` (BC-8.1.007: `--output json` returns
    /// `{"id","name","project"}` — same shape as create).
    /// ADR-0018 §2 cache invalidation is the caller's responsibility.
    pub async fn edit_component(&self, component_id: &str, body: &Value) -> Result<Component> {
        let path = format!("/rest/api/3/component/{}", component_id);
        self.put_json(&path, body).await
    }

    /// Fetch a single component by ID.
    ///
    /// GETs `/rest/api/3/component/{component_id}`.
    /// Used as the ADR-0018 §1 numeric confirming-GET for the `jr component edit` numeric
    /// path (`handle_edit`). Future delete/rename numeric paths (S-604-3/S-608-1) reuse it.
    /// NOT called by `resolve_component`, which is a pure candidate-list resolver that
    /// performs no HTTP.
    pub async fn get_component(&self, component_id: &str) -> Result<Component> {
        let path = format!("/rest/api/3/component/{}", component_id);
        self.get(&path).await
    }
}
