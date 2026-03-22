#[allow(dead_code)]
mod common;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_search_issues() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "FOO-1",
                "Test issue",
                "To Do",
            )]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issues = client
        .search_issues("assignee = currentUser()", None, &[])
        .await
        .unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].key, "FOO-1");
}

#[tokio::test]
async fn test_get_issue() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "Test issue",
                "In Progress",
            )),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issue = client.get_issue("FOO-1", &[]).await.unwrap();
    assert_eq!(issue.key, "FOO-1");
    assert_eq!(issue.fields.status.unwrap().name, "In Progress");
}

#[tokio::test]
async fn test_get_transitions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response(vec![("21", "In Progress"), ("31", "Done")]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let transitions = client.get_transitions("FOO-1").await.unwrap();
    assert_eq!(transitions.transitions.len(), 2);
    client.transition_issue("FOO-1", "21").await.unwrap();
}

#[tokio::test]
async fn test_search_issues_with_story_points() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_points(
                    "FOO-1",
                    "Story A",
                    "To Do",
                    Some(5.0),
                ),
                common::fixtures::issue_response_with_points("FOO-2", "Story B", "Done", None),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issues = client
        .search_issues("project = FOO", None, &["customfield_10031"])
        .await
        .unwrap();

    assert_eq!(issues.len(), 2);
    assert_eq!(
        issues[0].fields.story_points("customfield_10031"),
        Some(5.0)
    );
    assert_eq!(issues[1].fields.story_points("customfield_10031"), None);
}

#[tokio::test]
async fn test_find_story_points_field_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::fields_response_with_story_points()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let matches = client.find_story_points_field_id().await.unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0, "customfield_10031");
    assert_eq!(matches[0].1, "Story Points");
}
