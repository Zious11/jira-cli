use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::api::client::JiraClient;
use crate::api::pagination::OffsetPage;
use crate::types::jira::ProjectSummary;

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

    pub async fn list_projects(
        &self,
        type_key: Option<&str>,
        max_results: Option<u32>,
    ) -> Result<Vec<ProjectSummary>> {
        let page_size = max_results.map(|m| m.min(50)).unwrap_or(50);
        let mut all_projects: Vec<ProjectSummary> = Vec::new();
        let mut start_at: u32 = 0;

        loop {
            let mut path = format!(
                "/rest/api/3/project/search?orderBy=key&startAt={}&maxResults={}",
                start_at, page_size
            );
            if let Some(tk) = type_key {
                path.push_str(&format!("&typeKey={}", urlencoding::encode(tk)));
            }

            let page: OffsetPage<ProjectSummary> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all_projects.extend(page.values.unwrap_or_default());

            // If caller specified a limit, stop after one page
            if max_results.is_some() || !has_more {
                break;
            }
            start_at = next;
        }

        Ok(all_projects)
    }
}
