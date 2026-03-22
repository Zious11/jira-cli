use serde::{Deserialize, Serialize};

/// Generic wrapper for Atlassian GraphQL responses
#[derive(Debug, Deserialize)]
pub struct GraphqlResponse<T> {
    pub data: Option<T>,
}

#[derive(Debug, Deserialize)]
pub struct TenantContextData {
    #[serde(rename = "tenantContexts")]
    pub tenant_contexts: Vec<TenantContext>,
}

/// A tenant context returned by the GraphQL `tenantContexts` query.
/// Contains both orgId and cloudId.
#[derive(Debug, Deserialize)]
pub struct TenantContext {
    #[serde(rename = "orgId")]
    pub org_id: String,
    #[serde(rename = "cloudId")]
    pub cloud_id: String,
}

/// A single team entry from the Teams REST API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TeamEntry {
    #[serde(rename = "teamId")]
    pub team_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

/// Response from `GET /gateway/api/public/teams/v1/org/{orgId}/teams`
#[derive(Debug, Deserialize)]
pub struct TeamsResponse {
    pub entities: Vec<TeamEntry>,
    pub cursor: Option<String>,
}
