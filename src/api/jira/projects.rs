use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::api::client::JiraClient;

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueTypeMetadata {
    pub name: String,
    pub description: Option<String>,
    pub subtask: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PriorityMetadata {
    pub name: String,
    pub id: String,
}

impl JiraClient {
    pub async fn get_project_issue_types(
        &self,
        project_key: &str,
    ) -> Result<Vec<IssueTypeMetadata>> {
        let project: serde_json::Value = self
            .get(&format!("/rest/api/3/project/{project_key}"))
            .await?;
        let types = project
            .get("issueTypes")
            .and_then(|v| serde_json::from_value::<Vec<IssueTypeMetadata>>(v.clone()).ok())
            .unwrap_or_default();
        Ok(types)
    }

    pub async fn get_priorities(&self) -> Result<Vec<PriorityMetadata>> {
        self.get("/rest/api/3/priority").await
    }
}
