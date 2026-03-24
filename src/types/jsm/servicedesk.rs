use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ServiceDesk {
    pub id: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(rename = "projectName")]
    pub project_name: String,
}
