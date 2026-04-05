use crate::api::client::JiraClient;
use crate::types::jira::User;
use anyhow::Result;

impl JiraClient {
    pub async fn get_myself(&self) -> Result<User> {
        self.get("/rest/api/3/myself").await
    }

    /// Search for users by name or email prefix.
    ///
    /// Returns active and inactive users — caller should filter by `active` field.
    /// The response format may vary (flat array or paginated object), so both are handled.
    pub async fn search_users(&self, query: &str) -> Result<Vec<User>> {
        let path = format!(
            "/rest/api/3/user/search?query={}",
            urlencoding::encode(query)
        );
        let raw: serde_json::Value = self.get(&path).await?;
        let users: Vec<User> = if raw.is_array() {
            serde_json::from_value(raw)?
        } else if let Some(values) = raw.get("values") {
            serde_json::from_value(values.clone())?
        } else {
            anyhow::bail!(
                "Unexpected response from user search API. Expected a JSON array or object with \"values\" key."
            );
        };
        Ok(users)
    }

    /// Search for users assignable to a specific issue.
    ///
    /// Uses the `/user/assignable/search` endpoint which returns users
    /// eligible for assignment on the issue's project.
    pub async fn search_assignable_users(&self, query: &str, issue_key: &str) -> Result<Vec<User>> {
        let path = format!(
            "/rest/api/3/user/assignable/search?query={}&issueKey={}",
            urlencoding::encode(query),
            urlencoding::encode(issue_key),
        );
        let raw: serde_json::Value = self.get(&path).await?;
        let users: Vec<User> = if raw.is_array() {
            serde_json::from_value(raw)?
        } else if let Some(values) = raw.get("values") {
            serde_json::from_value(values.clone())?
        } else {
            anyhow::bail!(
                "Unexpected response from assignable user search API. Expected a JSON array or object with \"values\" key."
            );
        };
        Ok(users)
    }

    /// Search for users assignable to issues in a project.
    ///
    /// Uses the `/user/assignable/multiProjectSearch` endpoint with a single project key.
    /// Useful when no specific issue key is available (e.g., during issue creation).
    pub async fn search_assignable_users_by_project(
        &self,
        query: &str,
        project_key: &str,
    ) -> Result<Vec<User>> {
        let path = format!(
            "/rest/api/3/user/assignable/multiProjectSearch?query={}&projectKeys={}",
            urlencoding::encode(query),
            urlencoding::encode(project_key),
        );
        let raw: serde_json::Value = self.get(&path).await?;
        let users: Vec<User> = if raw.is_array() {
            serde_json::from_value(raw)?
        } else if let Some(values) = raw.get("values") {
            serde_json::from_value(values.clone())?
        } else {
            anyhow::bail!(
                "Unexpected response from assignable user search API. Expected a JSON array or object with \"values\" key."
            );
        };
        Ok(users)
    }
}

#[cfg(test)]
mod tests {
    use crate::types::jira::User;

    #[test]
    fn multi_project_search_response_deserializes() {
        let json = r#"[
            {"accountId": "abc123", "displayName": "Alice", "active": true},
            {"accountId": "def456", "displayName": "Bob", "emailAddress": "bob@test.com"}
        ]"#;
        let users: Vec<User> = serde_json::from_str(json).unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].account_id, "abc123");
        assert_eq!(users[0].display_name, "Alice");
        assert_eq!(users[0].active, Some(true));
        assert_eq!(users[1].account_id, "def456");
        assert_eq!(users[1].email_address.as_deref(), Some("bob@test.com"));
        assert_eq!(users[1].active, None);
    }
}
