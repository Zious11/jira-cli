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
        // CWE-116 defense-in-depth: consistent with `delete_component`'s
        // encoding of every interpolated path segment (F5-B-LOW-1) — a
        // project key is caller-sourced (from `--project`/config), so encode
        // regardless of the current alphanumeric-only convention.
        let encoded_key = urlencoding::encode(project_key);
        let path = format!("/rest/api/3/project/{encoded_key}/components");
        self.get(&path).await
    }

    /// Fetch the related issue count for a single component.
    ///
    /// GETs `/rest/api/3/component/{component_id}/relatedIssueCounts`.
    /// Called per-component by `jr component list --counts` (BC-8.1.003).
    pub async fn get_related_issue_counts(&self, component_id: &str) -> Result<RelatedIssueCounts> {
        // CWE-116 defense-in-depth, consistent with `delete_component`
        // (F5-B-LOW-1). Encoding a numeric id string is a no-op.
        let encoded_id = urlencoding::encode(component_id);
        let path = format!("/rest/api/3/component/{encoded_id}/relatedIssueCounts");
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
        // CWE-116 defense-in-depth, consistent with `delete_component`
        // (F5-B-LOW-1). Encoding a numeric id string is a no-op.
        let encoded_id = urlencoding::encode(component_id);
        let path = format!("/rest/api/3/component/{encoded_id}");
        self.put_json(&path, body).await
    }

    /// Fetch a single component by ID.
    ///
    /// GETs `/rest/api/3/component/{component_id}`.
    /// Reused as the ADR-0018 §1 numeric confirming-GET by `handle_edit`,
    /// `handle_delete` (S-604-3), and `resolve_rename_source` (S-608-1).
    /// NOT called by `resolve_component`, which is a pure candidate-list resolver that
    /// performs no HTTP.
    pub async fn get_component(&self, component_id: &str) -> Result<Component> {
        // CWE-116 defense-in-depth, consistent with `delete_component`
        // (F5-B-LOW-1). Encoding a numeric id string is a no-op.
        let encoded_id = urlencoding::encode(component_id);
        let path = format!("/rest/api/3/component/{encoded_id}");
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

    /// Rename a component (S-608-1, BC-8.3.001 Postcondition 1).
    ///
    /// PUTs `/rest/api/3/component/{component_id}` with body EXACTLY
    /// `{"name": new_name}` — no other fields. This is a pure rename; reuses
    /// `edit_component`'s PUT mechanics at the implementation layer (see this
    /// story's Architecture Mapping table) but is kept as its own function
    /// because the caller-facing contract (single-project resolution
    /// unconditionally requiring `--project`, plus the `--all-projects`
    /// fan-out calling this once per matched project — BC-8.3.002/BC-8.3.003)
    /// is distinct from `edit_component`'s multi-field partial-PUT contract.
    ///
    /// Returns the updated `Component`. Component `id` is unchanged by the
    /// rename (BC-8.3.001). A 404 here (component deleted by a concurrent
    /// actor after a successful resolution — either the single-project
    /// resolver or the `--all-projects` per-project discovery loop)
    /// propagates as `JrError::ApiError` — exit 1, distinct from a
    /// resolver-layer not-found (BC-8.1.008 / AC-017). A 400 name collision
    /// (BC-8.3.007) is surfaced verbatim and is NOT pre-validated here or by
    /// any caller.
    ///
    /// Callers (`handle_rename`, S-608-1) are responsible for: the ADR-0018
    /// §1 numeric confirming-GET on `OLD` (single-project form only —
    /// BC-8.3.001 M1), the `--all-projects` per-project exact-equality
    /// discovery loop (BC-8.3.002 — NOT `resolve_component`/`partial_match`),
    /// per-project continue-on-error accumulation (BC-8.3.003), the
    /// `--dry-run` short-circuit that must issue ZERO calls to this function
    /// (BC-8.3.004), and the ADR-0018 §2 components cache invalidation after
    /// each successful call.
    pub async fn rename_component(&self, component_id: &str, new_name: &str) -> Result<Component> {
        let body = serde_json::json!({ "name": new_name });
        // Reuses `edit_component`'s PUT mechanics (URL-encoding included) —
        // see this function's own rustdoc for why it stays a separate
        // caller-facing function despite the implementation-layer reuse.
        self.edit_component(component_id, &body).await
    }
}
