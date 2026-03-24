use anyhow::Result;

use crate::api::client::JiraClient;
use crate::types::assets::ConnectedTicketsResponse;

impl JiraClient {
    /// Get Jira issues connected to an asset object.
    pub async fn get_connected_tickets(
        &self,
        workspace_id: &str,
        object_id: &str,
    ) -> Result<ConnectedTicketsResponse> {
        let path = format!(
            "objectconnectedtickets/{}/tickets",
            urlencoding::encode(object_id)
        );
        self.get_assets(workspace_id, &path).await
    }
}
