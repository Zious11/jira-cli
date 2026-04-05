use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Project {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProjectSummary {
    pub key: String,
    pub name: String,
    #[serde(rename = "projectTypeKey")]
    pub project_type_key: String,
    pub lead: Option<ProjectLead>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProjectLead {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
}
