use anyhow::Result;

use crate::api::client::JiraClient;
use crate::types::jira::{
    GraphqlResponse, TeamEntry, TeamsResponse, TenantContext, TenantContextData,
};

impl JiraClient {
    /// Resolve cloudId and orgId for the Jira instance in a single GraphQL call.
    /// Uses `hostNames` parameter with the instance hostname.
    /// Uses instance_url (not base_url) since this endpoint is on the Jira instance itself.
    pub async fn get_org_metadata(&self, hostname: &str) -> Result<TenantContext> {
        let query = serde_json::json!({
            "query": format!(
                "query getOrgId {{ tenantContexts(hostNames: [\"{hostname}\"]) {{ orgId cloudId }} }}"
            )
        });
        let resp: GraphqlResponse<TenantContextData> = self
            .post_to_instance("/gateway/api/graphql", &query)
            .await?;

        resp.data
            .and_then(|d| d.tenant_contexts.into_iter().next())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Could not resolve organization ID. Check your Jira URL and permissions, or run jr init."
                )
            })
    }

    /// List all teams in the organization, handling cursor-based pagination.
    /// Uses instance_url (not base_url).
    pub async fn list_teams(&self, org_id: &str) -> Result<Vec<TeamEntry>> {
        let mut all_teams: Vec<TeamEntry> = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut path = format!("/gateway/api/public/teams/v1/org/{}/teams", org_id);
            if let Some(ref c) = cursor {
                path.push_str(&format!("?cursor={}", urlencoding::encode(c)));
            }

            let resp: TeamsResponse = self.get_from_instance(&path).await?;

            all_teams.extend(resp.entities);

            match resp.cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }

        Ok(all_teams)
    }
}
