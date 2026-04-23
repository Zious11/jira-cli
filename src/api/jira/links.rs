use crate::api::client::JiraClient;
use crate::types::jira::issue::{CreateRemoteLinkResponse, IssueLinkType, IssueLinkTypesResponse};
use anyhow::Result;
use serde_json::json;

impl JiraClient {
    /// Create a link between two issues.
    /// outward_key gets the "outward" label (e.g., "blocks"),
    /// inward_key gets the "inward" label (e.g., "is blocked by").
    pub async fn create_issue_link(
        &self,
        outward_key: &str,
        inward_key: &str,
        link_type: &str,
    ) -> Result<()> {
        let body = json!({
            "outwardIssue": {"key": outward_key},
            "inwardIssue": {"key": inward_key},
            "type": {"name": link_type}
        });
        self.post_no_content("/rest/api/3/issueLink", &body).await
    }

    /// Delete an issue link by its ID.
    pub async fn delete_issue_link(&self, link_id: &str) -> Result<()> {
        let path = format!("/rest/api/3/issueLink/{}", link_id);
        self.delete(&path).await
    }

    /// List all available issue link types.
    pub async fn list_link_types(&self) -> Result<Vec<IssueLinkType>> {
        let resp: IssueLinkTypesResponse = self.get("/rest/api/3/issueLinkType").await?;
        Ok(resp.issue_link_types)
    }

    /// Create a remote link (e.g., web URL, Confluence page) on an issue.
    ///
    /// Endpoint: `POST /rest/api/3/issue/{issueIdOrKey}/remotelink`.
    /// Minimum body is `{"object": {"url": ..., "title": ...}}`. Jira returns
    /// `201 Created` with `{"id": <number>, "self": "<url>"}` on create, or
    /// `200 OK` when an existing link with the same `globalId` is updated.
    pub async fn create_remote_link(
        &self,
        issue_key: &str,
        url: &str,
        title: &str,
    ) -> Result<CreateRemoteLinkResponse> {
        let path = format!(
            "/rest/api/3/issue/{}/remotelink",
            urlencoding::encode(issue_key)
        );
        let body = json!({
            "object": { "url": url, "title": title }
        });
        self.post(&path, &body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, method, path},
    };

    #[tokio::test]
    async fn create_remote_link_posts_object_url_and_title() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/PROJ-1/remotelink"))
            .and(body_partial_json(serde_json::json!({
                "object": {
                    "url": "https://example.com",
                    "title": "Example"
                }
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": 10000,
                "self": "https://example.atlassian.net/rest/api/3/issue/PROJ-1/remotelink/10000"
            })))
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
        let resp = client
            .create_remote_link("PROJ-1", "https://example.com", "Example")
            .await
            .unwrap();

        assert_eq!(resp.id, 10000);
        assert_eq!(
            resp.self_url,
            "https://example.atlassian.net/rest/api/3/issue/PROJ-1/remotelink/10000"
        );
    }
}
