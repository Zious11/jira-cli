use crate::api::client::JiraClient;
use crate::types::jira::User;
use anyhow::Result;

/// Maximum users requested per page. Atlassian's effective server-side cap
/// for `/user/search` and related endpoints is 100 — requesting more is
/// silently clamped to 100 by the server.
const USER_PAGE_SIZE: u32 = 100;

/// Safety bound on the pagination loop. Atlassian documents a 1000-user hard
/// cap on these endpoints, so at `USER_PAGE_SIZE=100` the loop terminates
/// naturally (empty page) by iteration 11. The 15-iteration bound is purely
/// defensive against pathological server behavior (e.g., Atlassian silently
/// raising the cap). Users in practice see at most ~1000 users; if the loop
/// ever exits via this cap, `search_users_all` emits a stderr warning.
const USER_PAGINATION_SAFETY_CAP: u32 = 15;

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

    /// Single-page variant of `search_users` with explicit `startAt` / `maxResults`.
    /// Private — used only by `search_users_all` to implement the loop.
    async fn search_users_page(
        &self,
        query: &str,
        start_at: u32,
        max_results: u32,
    ) -> Result<Vec<User>> {
        let path = format!(
            "/rest/api/3/user/search?query={}&startAt={}&maxResults={}",
            urlencoding::encode(query),
            start_at,
            max_results,
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

    /// Paginate `/rest/api/3/user/search` until exhausted.
    ///
    /// Jira uses **fixed-window pagination**: the server selects the raw
    /// user range `[startAt, startAt + maxResults)` and *then* applies
    /// permission filtering, returning only visible users — which may be
    /// fewer than `maxResults`. Advancing `startAt` by the returned count
    /// would overlap windows and cause duplicates
    /// (see JRACLOUD-71293). The correct advance is always by
    /// `USER_PAGE_SIZE` (the requested window size).
    ///
    /// A non-empty short page is NOT end-of-data — more visible users may
    /// live in later windows. The only reliable termination signal is an
    /// empty response.
    pub async fn search_users_all(&self, query: &str) -> Result<Vec<User>> {
        let mut all: Vec<User> = Vec::new();
        let mut start_at: u32 = 0;
        let mut reached_end = false;
        for _ in 0..USER_PAGINATION_SAFETY_CAP {
            let page = self
                .search_users_page(query, start_at, USER_PAGE_SIZE)
                .await?;
            if page.is_empty() {
                reached_end = true;
                break;
            }
            all.extend(page);
            start_at = start_at.saturating_add(USER_PAGE_SIZE);
        }
        if !reached_end {
            eprintln!(
                "warning: user search hit pagination safety cap ({} pages, {} users); results may be incomplete",
                USER_PAGINATION_SAFETY_CAP,
                all.len()
            );
        }
        Ok(all)
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

    /// Single-page variant of `search_assignable_users_by_project`.
    /// Private — used only by `search_assignable_users_by_project_all`.
    async fn search_assignable_users_by_project_page(
        &self,
        query: &str,
        project_key: &str,
        start_at: u32,
        max_results: u32,
    ) -> Result<Vec<User>> {
        let path = format!(
            "/rest/api/3/user/assignable/multiProjectSearch?query={}&projectKeys={}&startAt={}&maxResults={}",
            urlencoding::encode(query),
            urlencoding::encode(project_key),
            start_at,
            max_results,
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

    /// Paginate `/rest/api/3/user/assignable/multiProjectSearch` until exhausted.
    ///
    /// Same fixed-window semantics as `search_users_all`: advance `startAt`
    /// by `USER_PAGE_SIZE`, not by returned count, to avoid overlap/duplicate
    /// users. Empty response is the only reliable end-of-data signal; a
    /// non-empty short page is NOT end-of-data.
    pub async fn search_assignable_users_by_project_all(
        &self,
        query: &str,
        project_key: &str,
    ) -> Result<Vec<User>> {
        let mut all: Vec<User> = Vec::new();
        let mut start_at: u32 = 0;
        let mut reached_end = false;
        for _ in 0..USER_PAGINATION_SAFETY_CAP {
            let page = self
                .search_assignable_users_by_project_page(
                    query,
                    project_key,
                    start_at,
                    USER_PAGE_SIZE,
                )
                .await?;
            if page.is_empty() {
                reached_end = true;
                break;
            }
            all.extend(page);
            start_at = start_at.saturating_add(USER_PAGE_SIZE);
        }
        if !reached_end {
            eprintln!(
                "warning: assignable user search hit pagination safety cap ({} pages, {} users); results may be incomplete",
                USER_PAGINATION_SAFETY_CAP,
                all.len()
            );
        }
        Ok(all)
    }

    /// Fetch a single user by accountId.
    ///
    /// Returns a `JrError::ApiError { status: 404 | 400, .. }` when the
    /// accountId is unknown or malformed — Jira is inconsistent which it
    /// returns. Email may be omitted from the response based on the target
    /// user's profile-visibility settings.
    pub async fn get_user(&self, account_id: &str) -> Result<User> {
        let path = format!(
            "/rest/api/3/user?accountId={}",
            urlencoding::encode(account_id)
        );
        self.get(&path).await
    }
}

#[cfg(test)]
mod tests {
    use crate::types::jira::User;

    #[test]
    fn single_user_response_deserializes() {
        let json = r#"{
            "accountId": "5b10ac8d82e05b22cc7d4349",
            "displayName": "Jane Smith",
            "emailAddress": "jane@acme.io",
            "active": true
        }"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.account_id, "5b10ac8d82e05b22cc7d4349");
        assert_eq!(user.display_name, "Jane Smith");
        assert_eq!(user.email_address.as_deref(), Some("jane@acme.io"));
        assert_eq!(user.active, Some(true));
    }

    #[test]
    fn single_user_without_email_deserializes() {
        let json = r#"{
            "accountId": "abc",
            "displayName": "Privacy User",
            "active": true
        }"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.account_id, "abc");
        assert!(user.email_address.is_none());
    }

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
