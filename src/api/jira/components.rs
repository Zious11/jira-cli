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
    /// Returns the created `Component`; the ADR-0018 §1 confirming-GET is the
    /// caller's responsibility (BC-8.2.008).
    pub async fn create_component(&self, _body: &Value) -> Result<Component> {
        todo!()
    }

    /// Edit an existing component.
    ///
    /// PUTs `/rest/api/3/component/{component_id}`.
    /// Returns the updated `Component`; the ADR-0018 §1 confirming-GET is the
    /// caller's responsibility (BC-8.3.007).
    pub async fn edit_component(&self, _component_id: &str, _body: &Value) -> Result<Component> {
        todo!()
    }

    /// Fetch a single component by ID.
    ///
    /// GETs `/rest/api/3/component/{component_id}`.
    /// Used as the ADR-0018 §1 confirming-GET after `create_component` /
    /// `edit_component`, and by the `resolve_component` name→id resolver.
    pub async fn get_component(&self, _component_id: &str) -> Result<Component> {
        todo!()
    }
}
