use anyhow::Result;
use serde::Deserialize;

use crate::api::client::JiraClient;
use crate::api::pagination::ServiceDeskPage;
use crate::cache;
use crate::error::JrError;

#[derive(Debug, Default, Deserialize)]
struct WorkspaceEntry {
    #[serde(rename = "workspaceId")]
    workspace_id: String,
}

/// Get the Assets workspace ID, using cache when available.
///
/// The discovery endpoint returns a paginated response with workspace entries.
/// In practice there's only one workspace per site.
pub async fn get_or_fetch_workspace_id(client: &JiraClient) -> Result<String> {
    if let Some(cached) = cache::read_workspace_cache()? {
        return Ok(cached.workspace_id);
    }

    let page: ServiceDeskPage<WorkspaceEntry> = client
        .get_from_instance("/rest/servicedeskapi/assets/workspace")
        .await
        .map_err(|e| {
            if let Some(JrError::ApiError { status, .. }) = e.downcast_ref::<JrError>() {
                if *status == 404 || *status == 403 {
                    return JrError::UserError(
                        "Assets is not available on this Jira site. \
                         Assets requires Jira Service Management Premium or Enterprise."
                            .into(),
                    )
                    .into();
                }
            }
            e
        })?;

    let workspace_id = page
        .values
        .into_iter()
        .next()
        .map(|w| w.workspace_id)
        .ok_or_else(|| {
            JrError::UserError(
                "No Assets workspace found on this Jira site. \
                 Assets requires Jira Service Management Premium or Enterprise."
                    .into(),
            )
        })?;

    let _ = cache::write_workspace_cache(&workspace_id);

    Ok(workspace_id)
}
