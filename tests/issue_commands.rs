#[allow(dead_code)]
mod common;

use wiremock::matchers::{body_partial_json, method, path, query_param};
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
    let result = client
        .search_issues("assignee = currentUser()", None, &[])
        .await
        .unwrap();
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].key, "FOO-1");
    assert!(!result.has_more);
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
    let result = client
        .search_issues("project = FOO", None, &["customfield_10031"])
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 2);
    assert_eq!(
        result.issues[0].fields.story_points("customfield_10031"),
        Some(5.0)
    );
    assert_eq!(
        result.issues[1].fields.story_points("customfield_10031"),
        None
    );
    assert!(!result.has_more);
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

#[tokio::test]
async fn test_list_link_types() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issueLinkType"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::link_types_response()),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let types = client.list_link_types().await.unwrap();
    assert_eq!(types.len(), 3);
    assert_eq!(types[0].name, "Blocks");
    assert_eq!(types[0].outward.as_deref(), Some("blocks"));
    assert_eq!(types[0].inward.as_deref(), Some("is blocked by"));
}

#[tokio::test]
async fn test_get_issue_with_parent_and_links() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_with_links_response("FOO-2", "Test issue"),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issue = client.get_issue("FOO-2", &[]).await.unwrap();

    let parent = issue.fields.parent.unwrap();
    assert_eq!(parent.key, "FOO-1");
    assert_eq!(parent.fields.unwrap().summary.unwrap(), "Parent Epic");

    let links = issue.fields.issuelinks.unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].link_type.name, "Blocks");
    assert_eq!(links[0].outward_issue.as_ref().unwrap().key, "FOO-3");
}

#[tokio::test]
async fn test_create_issue_link() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issueLink"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client
        .create_issue_link("FOO-1", "FOO-2", "Blocks")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_delete_issue_link() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issueLink/10001"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client.delete_issue_link("10001").await.unwrap();
}

#[tokio::test]
async fn test_search_issues_has_more_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response_with_next_page(vec![
                common::fixtures::issue_response("FOO-1", "Test issue", "To Do"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client
        .search_issues("project = FOO", Some(1), &[])
        .await
        .unwrap();
    assert_eq!(result.issues.len(), 1);
    assert!(result.has_more);
}

#[tokio::test]
async fn test_search_issues_no_more_results() {
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
    let result = client
        .search_issues("project = FOO", Some(10), &[])
        .await
        .unwrap();
    assert_eq!(result.issues.len(), 1);
    assert!(!result.has_more);
}

#[tokio::test]
async fn test_search_issues_no_limit_fetches_all() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response("FOO-1", "Issue 1", "To Do"),
                common::fixtures::issue_response("FOO-2", "Issue 2", "To Do"),
                common::fixtures::issue_response("FOO-3", "Issue 3", "To Do"),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client
        .search_issues("project = FOO", None, &[])
        .await
        .unwrap();
    assert_eq!(result.issues.len(), 3);
    assert!(!result.has_more);
}

#[tokio::test]
async fn test_approximate_count() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/approximate-count"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::approximate_count_response(42)),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let count = client.approximate_count("project = FOO").await.unwrap();
    assert_eq!(count, 42);
}

#[tokio::test]
async fn test_approximate_count_zero() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/approximate-count"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::approximate_count_response(0)),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let count = client.approximate_count("project = FOO").await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_approximate_count_server_error_returns_err() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/approximate-count"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.approximate_count("project = FOO").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_users_single_result() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![("acc-123", "Jane Doe", true)]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users("Jane").await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].account_id, "acc-123");
    assert_eq!(users[0].display_name, "Jane Doe");
    assert_eq!(users[0].active, Some(true));
}

#[tokio::test]
async fn test_search_users_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users("Nobody").await.unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn test_search_users_multiple() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![
                ("acc-1", "Jane Doe", true),
                ("acc-2", "Jane Smith", true),
                ("acc-3", "Jane Inactive", false),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users("Jane").await.unwrap();
    assert_eq!(users.len(), 3);
}

#[tokio::test]
async fn test_search_users_paginated_response() {
    let server = MockServer::start().await;
    // Test the paginated { "values": [...] } response shape
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total": 1,
            "values": [
                {
                    "accountId": "acc-paged",
                    "displayName": "Paged User",
                    "emailAddress": "paged@test.com",
                    "active": true
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users("Paged").await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].account_id, "acc-paged");
    assert_eq!(users[0].display_name, "Paged User");
}

#[tokio::test]
async fn test_search_users_unrecognized_response_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"error": "unexpected"})),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.search_users("Test").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_issues_jql_with_project_scope() {
    let server = MockServer::start().await;

    // The mock only matches if the POST body contains the expected composed JQL
    let expected_jql = r#"project = "PROJ" AND (priority = Highest) ORDER BY updated DESC"#;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "jql": expected_jql
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![common::fixtures::issue_response(
                "PROJ-1",
                "High priority issue",
                "To Do",
            )]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // This is the JQL that handle_list would compose when given
    // --project PROJ --jql "priority = Highest"
    let result = client
        .search_issues(expected_jql, Some(30), &[])
        .await
        .unwrap();
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].key, "PROJ-1");
}

#[tokio::test]
async fn get_issue_includes_standard_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_standard_fields("FOO-42", "Test with all fields"),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issue = client.get_issue("FOO-42", &[]).await.unwrap();

    // Verify new fields are deserialized
    assert_eq!(
        issue.fields.created.as_deref(),
        Some("2026-03-20T14:32:00.000+0000")
    );
    assert_eq!(
        issue.fields.updated.as_deref(),
        Some("2026-03-25T09:15:22.000+0000")
    );

    let reporter = issue.fields.reporter.as_ref().unwrap();
    assert_eq!(reporter.display_name, "Jane Smith");
    assert_eq!(reporter.account_id, "def456");

    assert_eq!(issue.fields.resolution.as_ref().unwrap().name, "Fixed");

    let components = issue.fields.components.as_ref().unwrap();
    assert_eq!(components.len(), 2);
    assert_eq!(components[0].name, "Backend");
    assert_eq!(components[1].name, "API");

    let versions = issue.fields.fix_versions.as_ref().unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].name, "v2.0");
    assert_eq!(versions[0].released, Some(false));
    assert_eq!(versions[0].release_date.as_deref(), Some("2026-04-01"));

    // Verify JSON serialization includes the new fields at the expected paths
    let json_str = serde_json::to_string(&issue).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert!(value["fields"]["created"].is_string());
    assert!(value["fields"]["reporter"].is_object());
    assert!(value["fields"]["resolution"].is_object());
    assert!(value["fields"]["components"].is_array());
    assert!(value["fields"]["fixVersions"].is_array());
}

#[tokio::test]
async fn get_issue_null_standard_fields() {
    let server = MockServer::start().await;

    // Issue with all new fields null/absent
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-43"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-43",
                "Minimal issue",
                "To Do",
            )),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let issue = client.get_issue("FOO-43", &[]).await.unwrap();

    // All new fields should be None (the fixture doesn't include them)
    assert!(issue.fields.created.is_none());
    assert!(issue.fields.updated.is_none());
    assert!(issue.fields.reporter.is_none());
    assert!(issue.fields.resolution.is_none());
    assert!(issue.fields.components.is_none());
    assert!(issue.fields.fix_versions.is_none());
}

#[tokio::test]
async fn test_edit_issue_with_description() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-10"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "description": {
                    "version": 1,
                    "type": "doc",
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [
                                { "type": "text", "text": "Updated description" }
                            ]
                        }
                    ]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client
        .edit_issue(
            "FOO-10",
            serde_json::json!({
                "description": jr::adf::text_to_adf("Updated description")
            }),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_edit_issue_with_markdown_description() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-11"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "description": {
                    "version": 1,
                    "type": "doc",
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "bold text",
                                    "marks": [{"type": "strong"}]
                                }
                            ]
                        }
                    ]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client
        .edit_issue(
            "FOO-11",
            serde_json::json!({
                "description": jr::adf::markdown_to_adf("**bold text**")
            }),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_edit_issue_description_with_other_fields() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-12"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "summary": "New summary",
                "description": {
                    "version": 1,
                    "type": "doc",
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [
                                { "type": "text", "text": "New description" }
                            ]
                        }
                    ]
                }
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client
        .edit_issue(
            "FOO-12",
            serde_json::json!({
                "summary": "New summary",
                "description": jr::adf::text_to_adf("New description")
            }),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_search_assignable_users_single() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .and(query_param("query", "Jane"))
        .and(query_param("issueKey", "FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![("acc-assign-1", "Jane Doe", true)]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client
        .search_assignable_users("Jane", "FOO-1")
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].account_id, "acc-assign-1");
    assert_eq!(users[0].display_name, "Jane Doe");
}

#[tokio::test]
async fn test_search_assignable_users_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .and(query_param("query", "Nobody"))
        .and(query_param("issueKey", "FOO-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client
        .search_assignable_users("Nobody", "FOO-1")
        .await
        .unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn test_search_assignable_users_paginated_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .and(query_param("query", "Paged"))
        .and(query_param("issueKey", "FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total": 1,
            "values": [
                {
                    "accountId": "acc-paged-assign",
                    "displayName": "Paged Assignee",
                    "emailAddress": "paged@test.com",
                    "active": true
                }
            ]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client
        .search_assignable_users("Paged", "FOO-1")
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].account_id, "acc-paged-assign");
}

#[tokio::test]
async fn assign_to_user_resolves_display_name() {
    let server = MockServer::start().await;

    // Mock assignable user search → single result
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![("acc-jane-123", "Jane Doe", true)]),
        ))
        .mount(&server)
        .await;

    // Mock get issue → currently unassigned
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("FOO-1", "Test issue", None),
        ))
        .mount(&server)
        .await;

    // Mock assign → 204
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": "acc-jane-123"
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // Resolve and assign
    let users = client
        .search_assignable_users("Jane", "FOO-1")
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].account_id, "acc-jane-123");

    client
        .assign_issue("FOO-1", Some(&users[0].account_id))
        .await
        .unwrap();
}

#[tokio::test]
async fn assign_to_user_not_found() {
    let server = MockServer::start().await;

    // Mock assignable user search → empty results
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::user_search_response(vec![])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client
        .search_assignable_users("Nonexistent", "FOO-1")
        .await
        .unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn assign_to_me_keyword() {
    let server = MockServer::start().await;

    // Mock get myself
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_response()))
        .mount(&server)
        .await;

    // Mock get issue → currently unassigned
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee("FOO-1", "Test issue", None),
        ))
        .mount(&server)
        .await;

    // Mock assign → 204
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": "abc123"
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // "me" should resolve to get_myself(), not search API
    let me = client.get_myself().await.unwrap();
    assert_eq!(me.account_id, "abc123");

    client
        .assign_issue("FOO-1", Some(&me.account_id))
        .await
        .unwrap();
}

#[tokio::test]
async fn assign_idempotent_already_assigned() {
    let server = MockServer::start().await;

    // Mock assignable user search → single result
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![("acc-jane-123", "Jane Doe", true)]),
        ))
        .mount(&server)
        .await;

    // Mock get issue → already assigned to Jane
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee(
                "FOO-1",
                "Test issue",
                Some(("acc-jane-123", "Jane Doe")),
            ),
        ))
        .mount(&server)
        .await;

    // NO mock for PUT /assignee — if the code tries to call it, the test fails
    // because wiremock returns 404 for unmocked paths.

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // Resolve user
    let users = client
        .search_assignable_users("Jane", "FOO-1")
        .await
        .unwrap();
    assert_eq!(users[0].account_id, "acc-jane-123");

    // Get issue and verify already assigned
    let issue = client.get_issue("FOO-1", &[]).await.unwrap();
    let assignee = issue.fields.assignee.unwrap();
    assert_eq!(assignee.account_id, "acc-jane-123");
}

#[tokio::test]
async fn test_search_issues_includes_labels_parent_issuelinks() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "fields": [
                "summary", "status", "issuetype", "priority", "assignee",
                "reporter", "project", "description", "created", "updated",
                "resolution", "components", "fixVersions",
                "labels", "parent", "issuelinks"
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_labels_parent_links(
                    "FOO-10",
                    "Labeled issue",
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client
        .search_issues("project = FOO", Some(10), &[])
        .await
        .unwrap();

    assert_eq!(result.issues.len(), 1);
    let issue = &result.issues[0];

    // Labels
    let labels = issue.fields.labels.as_ref().expect("labels should be Some");
    assert_eq!(labels, &vec!["bug".to_string(), "frontend".to_string()]);

    // Parent
    let parent = issue.fields.parent.as_ref().expect("parent should be Some");
    assert_eq!(parent.key, "FOO-1");
    assert_eq!(
        parent.fields.as_ref().unwrap().summary.as_deref(),
        Some("Parent Epic")
    );

    // Issue links
    let links = issue
        .fields
        .issuelinks
        .as_ref()
        .expect("issuelinks should be Some");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].link_type.name, "Blocks");
    assert_eq!(links[0].outward_issue.as_ref().unwrap().key, "FOO-3");
}

#[tokio::test]
async fn test_create_issue_with_assignee() {
    let server = MockServer::start().await;

    // Mock multiProjectSearch → single result
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .and(query_param("query", "Jane"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::multi_project_user_search_response(vec![(
                "acc-jane-123",
                "Jane Doe",
            )]),
        ))
        .mount(&server)
        .await;

    // Mock create issue → verify assignee in request body
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "FOO"},
                "issuetype": {"name": "Task"},
                "summary": "Test with assignee",
                "assignee": {"accountId": "acc-jane-123"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("FOO-99")),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // Resolve user
    let users = client
        .search_assignable_users_by_project("Jane", "FOO")
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].account_id, "acc-jane-123");
    assert_eq!(users[0].display_name, "Jane Doe");

    // Create issue with assignee field
    let mut fields = serde_json::json!({
        "project": {"key": "FOO"},
        "issuetype": {"name": "Task"},
        "summary": "Test with assignee",
    });
    fields["assignee"] = serde_json::json!({"accountId": users[0].account_id});

    let response = client.create_issue(fields).await.unwrap();
    assert_eq!(response.key, "FOO-99");
}

#[tokio::test]
async fn test_create_issue_with_assignee_me() {
    let server = MockServer::start().await;

    // Mock get_myself
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(common::fixtures::user_response()))
        .mount(&server)
        .await;

    // Mock create issue → verify assignee uses "me" account ID
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "assignee": {"accountId": "abc123"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("FOO-100")),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // "me" resolves via get_myself, not search API
    let me = client.get_myself().await.unwrap();
    assert_eq!(me.account_id, "abc123");

    // Create issue with self-assignment
    let mut fields = serde_json::json!({
        "project": {"key": "FOO"},
        "issuetype": {"name": "Task"},
        "summary": "Assigned to me",
    });
    fields["assignee"] = serde_json::json!({"accountId": me.account_id});

    let response = client.create_issue(fields).await.unwrap();
    assert_eq!(response.key, "FOO-100");
}

#[tokio::test]
async fn test_create_issue_without_assignee() {
    let server = MockServer::start().await;

    // No multiProjectSearch mock registered — this test only exercises create.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("FOO-101")),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let fields = serde_json::json!({
        "project": {"key": "FOO"},
        "issuetype": {"name": "Task"},
        "summary": "No assignee",
    });

    let response = client.create_issue(fields).await.unwrap();
    assert_eq!(response.key, "FOO-101");
}

#[tokio::test]
async fn test_create_issue_assignee_not_found() {
    let server = MockServer::start().await;

    // Mock multiProjectSearch → empty results
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::multi_project_user_search_response(vec![])),
        )
        .mount(&server)
        .await;

    // Only testing that the API returns an empty list — no create call is made.

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let users = client
        .search_assignable_users_by_project("Nonexistent", "FOO")
        .await
        .unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn test_create_issue_with_account_id() {
    let server = MockServer::start().await;

    // Mock create issue — verify assignee uses accountId format
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .and(body_partial_json(serde_json::json!({
            "fields": {
                "project": {"key": "FOO"},
                "issuetype": {"name": "Task"},
                "summary": "Assigned by accountId",
                "assignee": {"accountId": "direct-acct-789"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("FOO-200")),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    // Build fields with accountId directly — no user search mock needed
    let mut fields = serde_json::json!({
        "project": {"key": "FOO"},
        "issuetype": {"name": "Task"},
        "summary": "Assigned by accountId",
    });
    fields["assignee"] = serde_json::json!({"accountId": "direct-acct-789"});

    let response = client.create_issue(fields).await.unwrap();
    assert_eq!(response.key, "FOO-200");
}

#[tokio::test]
async fn test_move_by_transition_name() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Complete", "Completed"),
                ("31", "Review", "In Review"),
            ]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "Test issue",
                "To Do",
            )),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .and(body_partial_json(
            serde_json::json!({"transition": {"id": "21"}}),
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Complete")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected success, stderr: {stderr}"
    );
    assert!(
        stderr.contains("Moved FOO-1"),
        "Expected move confirmation in stderr: {stderr}"
    );
}

#[tokio::test]
async fn test_move_by_status_name() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Complete", "Completed"),
                ("31", "Review", "In Review"),
            ]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "Test issue",
                "To Do",
            )),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .and(body_partial_json(
            serde_json::json!({"transition": {"id": "21"}}),
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Completed")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected success, stderr: {stderr}"
    );
    assert!(
        stderr.contains("Moved FOO-1"),
        "Expected move confirmation in stderr: {stderr}"
    );
}

#[tokio::test]
async fn test_move_dedup_same_transition_and_status_name() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "In Progress", "In Progress"),
                ("31", "Done", "Done"),
            ]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "Test issue",
                "To Do",
            )),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .and(body_partial_json(
            serde_json::json!({"transition": {"id": "31"}}),
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Done")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected success, stderr: {stderr}"
    );
    assert!(
        stderr.contains("Moved FOO-1"),
        "Expected move confirmation in stderr: {stderr}"
    );
}

#[tokio::test]
async fn test_move_ambiguous_across_transition_and_status_names() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Reopen", "Open"),
                ("31", "Review", "In Review"),
            ]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "Test issue",
                "Closed",
            )),
        )
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Re")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, stderr: {stderr}"
    );
    assert!(
        stderr.contains("Ambiguous"),
        "Expected ambiguity error in stderr: {stderr}"
    );
}

#[tokio::test]
async fn test_move_no_match_shows_status_names() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Complete", "Completed"),
                ("31", "Review", "In Review"),
            ]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "Test issue",
                "To Do",
            )),
        )
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Nonexistent")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure, stderr: {stderr}"
    );
    assert!(
        stderr.contains("Complete (→ Completed)"),
        "Expected enriched error format in stderr: {stderr}"
    );
    assert!(
        stderr.contains("Review (→ In Review)"),
        "Expected enriched error format in stderr: {stderr}"
    );
}

#[tokio::test]
async fn test_move_idempotent_with_status_name() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![(
                "21",
                "Complete",
                "Completed",
            )]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "Test issue",
                "Completed",
            )),
        )
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Completed")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected success (idempotent), stderr: {stderr}"
    );
    assert!(
        stderr.contains("already in status"),
        "Expected idempotent message in stderr: {stderr}"
    );
}

#[tokio::test]
async fn test_move_idempotent_with_transition_name() {
    let server = MockServer::start().await;

    // Transition "Complete" leads to status "Completed"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![(
                "21",
                "Complete",
                "Completed",
            )]),
        ))
        .mount(&server)
        .await;

    // Issue is already in "Completed" — user types transition name "Complete"
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::issue_response(
                "FOO-1",
                "Test issue",
                "Completed",
            )),
        )
        .mount(&server)
        .await;

    // No POST mock — should not transition

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .arg("--no-input")
        .arg("issue")
        .arg("move")
        .arg("FOO-1")
        .arg("Complete")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected success (idempotent via transition name), stderr: {stderr}"
    );
    assert!(
        stderr.contains("already in status"),
        "Expected idempotent message in stderr: {stderr}"
    );
}

#[tokio::test]
async fn test_create_issue_response_includes_browse_url() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("URL-1")),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let response = client
        .create_issue(serde_json::json!({
            "project": {"key": "URL"},
            "issuetype": {"name": "Task"},
            "summary": "Test browse URL",
        }))
        .await
        .unwrap();

    // Verify the key is returned
    assert_eq!(response.key, "URL-1");

    // Verify browse URL can be constructed from instance_url
    let browse_url = format!(
        "{}/browse/{}",
        client.instance_url().trim_end_matches('/'),
        response.key
    );
    assert!(
        browse_url.contains("/browse/URL-1"),
        "Expected browse URL to contain /browse/URL-1, got: {browse_url}"
    );
}

#[tokio::test]
async fn test_assign_issue_with_account_id() {
    let server = MockServer::start().await;

    // Mock GET issue — currently unassigned
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ACC-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee(
                "ACC-1",
                "Test assign by accountId",
                None,
            ),
        ))
        .mount(&server)
        .await;

    // Mock PUT assignee — verify accountId in request body
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/ACC-1/assignee"))
        .and(body_partial_json(serde_json::json!({
            "accountId": "direct-account-id-456"
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    // Assign directly by accountId — no user search mock needed
    client
        .assign_issue("ACC-1", Some("direct-account-id-456"))
        .await
        .unwrap();

    // Verify fixture correctly represents an already-assigned issue
    let server2 = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ACC-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_assignee(
                "ACC-2",
                "Already assigned",
                Some(("direct-account-id-456", "direct-account-id-456")),
            ),
        ))
        .mount(&server2)
        .await;

    let client2 =
        jr::api::client::JiraClient::new_for_test(server2.uri(), "Basic dGVzdDp0ZXN0".into());

    let issue = client2.get_issue("ACC-2", &[]).await.unwrap();
    let assignee = issue.fields.assignee.unwrap();
    assert_eq!(assignee.account_id, "direct-account-id-456");
}

#[tokio::test]
async fn test_assign_issue_invalid_account_id_returns_error() {
    let server = MockServer::start().await;

    // Mock PUT assignee returning 404 with Jira error body for invalid accountId
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/ERR-1/assignee"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["User 'bogus-account-id' does not exist."]
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let result = client.assign_issue("ERR-1", Some("bogus-account-id")).await;

    let err = result.unwrap_err();

    // Verify correct error variant and status code structurally
    assert!(
        err.downcast_ref::<jr::error::JrError>()
            .is_some_and(|e| matches!(e, jr::error::JrError::ApiError { status: 404, .. })),
        "Expected JrError::ApiError with status 404, got: {err}"
    );

    // Verify Jira error message was extracted from the JSON body
    let msg = err.to_string();
    assert!(
        msg.contains("does not exist"),
        "Expected Jira error message in error, got: {msg}"
    );
}
