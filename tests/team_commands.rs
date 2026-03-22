mod common;

use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_get_org_metadata() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/gateway/api/graphql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::graphql_org_metadata_json()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let metadata = client.get_org_metadata("test.atlassian.net").await.unwrap();
    assert_eq!(metadata.org_id, "test-org-id-456");
    assert_eq!(metadata.cloud_id, "test-cloud-id-123");
}

#[tokio::test]
async fn test_list_teams() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path_regex("/gateway/api/public/teams/v1/org/.*/teams"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::teams_list_json()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let teams = client.list_teams("test-org-id-456").await.unwrap();
    assert_eq!(teams.len(), 3);
    assert_eq!(teams[0].display_name, "Alpha Team");
    assert_eq!(teams[0].team_id, "team-uuid-alpha");
}
