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

    /// Delete a component, optionally moving its issues to another component.
    ///
    /// DELETEs `/rest/api/3/component/{component_id}`, appending
    /// `?moveIssuesTo=<targetId>` when `move_issues_to` is `Some` (BC-8.2.002
    /// `--move-to` path). When `move_issues_to` is `None` the component's
    /// issues are orphaned (BC-8.2.006 `--orphan` path) — no query param is
    /// sent.
    ///
    /// This function performs ONLY the DELETE call. Callers (`handle_delete`,
    /// S-604-3) are responsible for: the ADR-0018 §1 numeric confirming-GET(s)
    /// on source and/or target, the BC-8.2.007 pre-delete affected-issue
    /// snapshot (which MUST complete successfully before this is called), the
    /// BC-8.2.006 `--orphan` confirmation gate, and the ADR-0018 §2 components
    /// cache invalidation after a successful call.
    ///
    /// A 404 here (source or `moveIssuesTo` target deleted by a concurrent
    /// actor after a successful resolution) propagates as `JrError::ApiError`
    /// — exit 1, per BC-8.2.008 / VP-COMPONENT-024. This is distinct from a
    /// resolver-layer not-found (BC-8.1.008), which is the caller's ordinary
    /// exit-64 `UserError` path and never reaches this function.
    pub async fn delete_component(
        &self,
        component_id: &str,
        move_issues_to: Option<&str>,
    ) -> Result<()> {
        // CWE-116 defense-in-depth: ids are API-sourced numeric strings, but
        // encode regardless (S-604-3 security LOW-1).
        let encoded_id = urlencoding::encode(component_id);
        let path = match move_issues_to {
            Some(target_id) => {
                let encoded_target = urlencoding::encode(target_id);
                format!("/rest/api/3/component/{encoded_id}?moveIssuesTo={encoded_target}")
            }
            None => format!("/rest/api/3/component/{encoded_id}"),
        };
        self.delete(&path).await
    }
}
