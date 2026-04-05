use crate::api::client::JiraClient;
use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct StatusEntry {
    name: String,
}

impl JiraClient {
    /// Fetch all statuses from active workflows (global, not project-scoped).
    ///
    /// Returns a flat list of unique status names. The endpoint is not paginated.
    pub async fn get_all_statuses(&self) -> Result<Vec<String>> {
        let entries: Vec<StatusEntry> = self.get("/rest/api/3/status").await?;
        let mut names: Vec<String> = entries.into_iter().map(|e| e.name).collect();
        names.sort();
        names.dedup();
        Ok(names)
    }
}
