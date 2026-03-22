use crate::api::client::JiraClient;
use crate::api::pagination::OffsetPage;
use crate::types::jira::{Issue, Sprint};
use anyhow::Result;

impl JiraClient {
    /// List sprints for a board, optionally filtering by state (active, closed, future).
    pub async fn list_sprints(&self, board_id: u64, state: Option<&str>) -> Result<Vec<Sprint>> {
        let mut all_sprints: Vec<Sprint> = Vec::new();
        let mut start_at: u32 = 0;
        let max_results: u32 = 50;

        loop {
            let mut path = format!(
                "/rest/agile/1.0/board/{}/sprint?startAt={}&maxResults={}",
                board_id, start_at, max_results
            );
            if let Some(s) = state {
                path.push_str(&format!("&state={}", urlencoding::encode(s)));
            }
            let page: OffsetPage<Sprint> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all_sprints.extend(page.values.unwrap_or_default());

            if !has_more {
                break;
            }
            start_at = next;
        }

        Ok(all_sprints)
    }

    /// Get issues in a specific sprint, with optional JQL filter.
    pub async fn get_sprint_issues(&self, sprint_id: u64, jql: Option<&str>) -> Result<Vec<Issue>> {
        let mut all_issues: Vec<Issue> = Vec::new();
        let mut start_at: u32 = 0;
        let max_results: u32 = 50;

        loop {
            let mut path = format!(
                "/rest/agile/1.0/sprint/{}/issue?startAt={}&maxResults={}",
                sprint_id, start_at, max_results
            );
            if let Some(q) = jql {
                path.push_str(&format!("&jql={}", urlencoding::encode(q)));
            }
            let page: OffsetPage<Issue> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all_issues.extend(page.issues.unwrap_or_default());

            if !has_more {
                break;
            }
            start_at = next;
        }

        Ok(all_issues)
    }
}
