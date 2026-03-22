use crate::api::client::JiraClient;
use crate::types::jira::User;
use anyhow::Result;

impl JiraClient {
    pub async fn get_myself(&self) -> Result<User> {
        self.get("/rest/api/3/myself").await
    }
}
