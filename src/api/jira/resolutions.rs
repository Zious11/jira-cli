use crate::api::client::JiraClient;
use crate::types::jira::Resolution;
use anyhow::Result;

impl JiraClient {
    /// Fetch all resolutions configured on the Jira instance.
    ///
    /// Resolutions are instance-scoped — no per-project endpoint. Returns
    /// the full list for company-managed (classic) projects; team-managed
    /// projects have no resolution concept so the list is irrelevant
    /// (but non-empty — Jira serves the same instance-global list).
    /// Not paginated.
    pub async fn get_resolutions(&self) -> Result<Vec<Resolution>> {
        self.get("/rest/api/3/resolution").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    #[tokio::test]
    async fn get_resolutions_returns_parsed_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/api/3/resolution"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": "10000",
                    "name": "Done",
                    "description": "Work has been completed.",
                    "self": "https://example.atlassian.net/rest/api/3/resolution/10000"
                },
                {
                    "id": "10001",
                    "name": "Won't Do",
                    "description": "This will not be worked on."
                }
            ])))
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
        let resolutions = client.get_resolutions().await.unwrap();

        assert_eq!(resolutions.len(), 2);
        assert_eq!(resolutions[0].name, "Done");
        assert_eq!(resolutions[0].id.as_deref(), Some("10000"));
        assert_eq!(resolutions[1].name, "Won't Do");
    }
}
