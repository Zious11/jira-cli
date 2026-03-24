use crate::api::client::JiraClient;
use crate::api::pagination::{CursorPage, OffsetPage};
use crate::types::jira::{Comment, CreateIssueResponse, Issue, TransitionsResponse};
use anyhow::Result;
use serde_json::Value;

impl JiraClient {
    /// Search issues using JQL with cursor-based pagination.
    pub async fn search_issues(
        &self,
        jql: &str,
        limit: Option<u32>,
        extra_fields: &[&str],
    ) -> Result<Vec<Issue>> {
        let max_per_page = limit.unwrap_or(50).min(100);
        let mut all_issues: Vec<Issue> = Vec::new();
        let mut next_page_token: Option<String> = None;

        let mut fields = vec![
            "summary",
            "status",
            "issuetype",
            "priority",
            "assignee",
            "project",
            "description",
        ];
        fields.extend_from_slice(extra_fields);

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": max_per_page,
                "fields": fields
            });

            if let Some(ref token) = next_page_token {
                body["nextPageToken"] = serde_json::json!(token);
            }

            let page: CursorPage<Issue> = self.post("/rest/api/3/search/jql", &body).await?;

            let has_more = page.has_more();
            let token = page.next_page_token.clone();
            all_issues.extend(page.issues);

            if let Some(max) = limit {
                if all_issues.len() >= max as usize {
                    all_issues.truncate(max as usize);
                    break;
                }
            }

            if !has_more {
                break;
            }

            next_page_token = token;
        }

        Ok(all_issues)
    }

    /// Get a single issue by key.
    pub async fn get_issue(&self, key: &str, extra_fields: &[&str]) -> Result<Issue> {
        let mut fields =
            "summary,status,issuetype,priority,assignee,project,description,labels,parent,issuelinks".to_string();
        for f in extra_fields {
            fields.push(',');
            fields.push_str(f);
        }
        let path = format!(
            "/rest/api/3/issue/{}?fields={}",
            urlencoding::encode(key),
            fields
        );
        self.get(&path).await
    }

    /// Create a new issue.
    pub async fn create_issue(&self, fields: Value) -> Result<CreateIssueResponse> {
        let body = serde_json::json!({ "fields": fields });
        self.post("/rest/api/3/issue", &body).await
    }

    /// Edit an existing issue's fields.
    pub async fn edit_issue(&self, key: &str, fields: Value) -> Result<()> {
        let path = format!("/rest/api/3/issue/{}", urlencoding::encode(key));
        let body = serde_json::json!({ "fields": fields });
        self.put(&path, &body).await
    }

    /// Get available transitions for an issue.
    pub async fn get_transitions(&self, key: &str) -> Result<TransitionsResponse> {
        let path = format!("/rest/api/3/issue/{}/transitions", urlencoding::encode(key));
        self.get(&path).await
    }

    /// Transition an issue to a new status.
    pub async fn transition_issue(&self, key: &str, transition_id: &str) -> Result<()> {
        let path = format!("/rest/api/3/issue/{}/transitions", urlencoding::encode(key));
        let body = serde_json::json!({
            "transition": { "id": transition_id }
        });
        self.post_no_content(&path, &body).await
    }

    /// Assign an issue. Pass `None` for account_id to unassign.
    pub async fn assign_issue(&self, key: &str, account_id: Option<&str>) -> Result<()> {
        let path = format!("/rest/api/3/issue/{}/assignee", urlencoding::encode(key));
        let body = serde_json::json!({
            "accountId": account_id
        });
        self.put(&path, &body).await
    }

    /// Add a comment to an issue.
    pub async fn add_comment(&self, key: &str, body: Value) -> Result<Comment> {
        let path = format!("/rest/api/3/issue/{}/comment", urlencoding::encode(key));
        let payload = serde_json::json!({ "body": body });
        self.post(&path, &payload).await
    }

    /// List comments on an issue with auto-pagination.
    pub async fn list_comments(&self, key: &str, limit: Option<u32>) -> Result<Vec<Comment>> {
        let base = format!("/rest/api/3/issue/{}/comment", urlencoding::encode(key));
        let mut all = Vec::new();
        let mut start_at = 0u32;
        let max_page_size: u32 = 100;

        loop {
            let page_size = match limit {
                Some(cap) => {
                    let remaining = cap.saturating_sub(all.len() as u32);
                    if remaining == 0 {
                        break;
                    }
                    remaining.min(max_page_size)
                }
                None => max_page_size,
            };
            let path = format!("{}?startAt={}&maxResults={}", base, start_at, page_size);
            let page: OffsetPage<Comment> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.append(&mut page.comments.unwrap_or_default());

            if let Some(cap) = limit {
                if all.len() >= cap as usize {
                    all.truncate(cap as usize);
                    break;
                }
            }
            if !has_more {
                break;
            }
            start_at = next;
        }
        Ok(all)
    }
}
