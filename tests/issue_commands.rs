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
    client.transition_issue("FOO-1", "21", None).await.unwrap();
}

#[tokio::test]
async fn transition_issue_with_fields_sends_fields_in_body() {
    use wiremock::matchers::{body_partial_json, method, path};

    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .and(body_partial_json(serde_json::json!({
            "transition": { "id": "31" },
            "fields": { "resolution": { "name": "Done" } }
        })))
        .respond_with(wiremock::ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let fields = serde_json::json!({ "resolution": { "name": "Done" } });
    client
        .transition_issue("FOO-1", "31", Some(&fields))
        .await
        .unwrap();
    // wiremock .expect(1) verifies the matcher was hit exactly once
}

#[tokio::test]
async fn transition_issue_without_fields_omits_fields_key() {
    use wiremock::matchers::{method, path};

    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(wiremock::ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    client.transition_issue("FOO-1", "31", None).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let body = String::from_utf8_lossy(&requests[0].body);
    assert!(
        !body.contains("\"fields\""),
        "fields key must be absent when None is passed, got body: {body}"
    );
    assert!(body.contains("\"transition\""));
    assert!(body.contains("\"31\""));
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
    // AC-12 (BC-2.3.036 [AMENDED]): `duedate` deserializes as a NAMED field
    // when present in the fixture — date-only `YYYY-MM-DD`, no time
    // component, no `#[serde(rename)]` needed (wire name already lowercase).
    assert_eq!(issue.fields.duedate.as_deref(), Some("2027-07-30"));

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
    // AC-1 / AC-14 (BC-2.3.036 [AMENDED]): `duedate` is a NAMED field, so it
    // serializes at `fields.duedate` directly — NOT inside the `extra`
    // flatten map (which would put it at an entirely different, unnamed
    // location with no dedicated key). Note: the runtime JSON assertion
    // below would also pass if `duedate` were flattened under a key of the
    // same name — the actual named-field-vs-flatten guarantee rests on the
    // compile-time `issue.fields.duedate` field access (AC-12 / view.rs),
    // which only compiles because `duedate` is a real struct field.
    assert_eq!(value["fields"]["duedate"], "2027-07-30");
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
    // AC-13 (BC-2.3.036 [AMENDED]): `duedate` absent from the wire response
    // deserializes to `None`, not a panic.
    assert!(issue.fields.duedate.is_none());
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
                "description": jr::adf::markdown_to_adf("**bold text**").unwrap()
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
                "duedate", "resolution", "components", "fixVersions",
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

// ── partial_match single-substring rejection (issue #193) ────────────
//
// These lock the guarantee that a single substring-only hit under
// `--no-input` routes through `Ambiguous` and errors before any
// state-changing HTTP call is made. The unit tests in partial_match.rs
// cover the matcher itself; these cover the handler wiring at each
// call site.

#[tokio::test]
async fn test_move_single_substring_rejected_no_input() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::transitions_response_with_status(vec![
                ("21", "Start", "In Progress"),
                ("31", "Close", "Closed"),
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

    // Assert no transition POST occurs — the substring must short-circuit.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "issue", "move", "FOO-1", "prog"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure on ambiguous substring, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Ambiguous transition should exit 64 (UserError), got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous transition"),
        "Expected 'Ambiguous transition' in stderr: {stderr}"
    );
    assert!(
        stderr.contains("In Progress"),
        "Expected matched candidate 'In Progress' in stderr: {stderr}"
    );
}

#[tokio::test]
async fn test_link_single_substring_rejected_no_input() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issueLinkType"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::link_types_response()),
        )
        .mount(&server)
        .await;

    // Assert no link POST occurs — the substring must short-circuit.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issueLink"))
        .respond_with(ResponseTemplate::new(201))
        .expect(0)
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--no-input",
            "issue",
            "link",
            "FOO-1",
            "FOO-2",
            "--type",
            "block",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure on ambiguous substring, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Ambiguous link type should exit 64 (UserError), got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous link type"),
        "Expected 'Ambiguous link type' in stderr: {stderr}"
    );
    assert!(
        stderr.contains("Blocks"),
        "Expected matched candidate 'Blocks' in stderr: {stderr}"
    );
}

#[tokio::test]
async fn test_unlink_single_substring_rejected_no_input() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issueLinkType"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::link_types_response()),
        )
        .mount(&server)
        .await;

    // Assert no DELETE /issueLink/* occurs — the substring must short-circuit.
    // Unlink also fetches candidate links (GET /rest/api/3/issue/FOO-1?fields=issuelinks)
    // before any delete, but that's irrelevant here since we error out before
    // reaching that call. No DELETE mock is mounted at all.
    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--no-input",
            "issue",
            "unlink",
            "FOO-1",
            "FOO-2",
            "--type",
            "block",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure on ambiguous substring, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "Ambiguous link type should exit 64 (UserError), got: {:?}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous link type"),
        "Expected 'Ambiguous link type' in stderr: {stderr}"
    );
    assert!(
        stderr.contains("Blocks"),
        "Expected matched candidate 'Blocks' in stderr: {stderr}"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// S-668-1 — `duedate` in `issue view`/`issue list` (issue #668)
//
// BC-2.2.028 [AMENDED], BC-2.2.032 [NEW], BC-2.3.036 [AMENDED], BC-2.3.039 [NEW]
//
// BC-5.38.001: the tests below exercise the IMPLEMENTED
// `src/cli/issue/format.rs::render_due_date` helper and the `"Due Date"`
// header literal emitted by `::issue_table_headers` — `render_due_date`
// returns the due-date value verbatim when present and `"-"` for
// `None`/empty; `issue_table_headers` pushes the literal header string
// `"Due Date"` when `show_duedate` is true. See the direct unit tests on
// `render_due_date` in `src/cli/issue/format.rs::tests` for the tightest
// anchor on that behavior.
// ─────────────────────────────────────────────────────────────────────────

/// Split a comfy-table row/header line into trimmed cell strings, one per
/// column, preserving positional (index) alignment with the header row.
///
/// `output.rs` renders with comfy-table's `UTF8_FULL_CONDENSED` preset, whose
/// preset string (`comfy_table::style::presets::UTF8_FULL_CONDENSED`) documents
/// itself with the example `│ Hello ┆ there │`: `│` (U+2502) is the OUTER
/// border character (appears once on each end of the line); the INTERNAL
/// column separator is `┆` (U+2506, "BOX DRAWINGS LIGHT TRIPLE DASH
/// VERTICAL"). Split on `┆` for exactly one chunk per column, then trim both
/// whitespace and any leftover outer `│` border from the first/last chunk.
///
/// Deliberately does NOT filter out empty cells: `line.split('┆')` already
/// yields exactly `column_count` chunks, so dropping an empty chunk would
/// silently shift every later chunk's index — a genuinely empty cell (e.g.
/// an unset field rendered as `""`) would then desynchronize any
/// index-based `header[i]` <-> `cells[i]` comparison a caller relies on.
/// None of this crate's fixtures currently render a genuinely empty cell,
/// but callers must not depend on that continuing to be true.
fn table_cells(line: &str) -> Vec<&str> {
    line.split('┆')
        .map(|s| s.trim().trim_matches('│').trim())
        .collect()
}

/// AC-1 (BC-2.3.036 [AMENDED] / BC-2.2.028): `issue view --output json`
/// includes `.fields.duedate` verbatim when set. Does NOT reach
/// `render_due_date` — the JSON branch in `handle_view` serializes the
/// typed struct directly, before the Table arm's row-building code.
#[tokio::test]
async fn test_issue_view_json_includes_duedate_when_set() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_duedate(
                "PROJ-1",
                "Ship the widget",
                "To Do",
                Some("2027-07-30"),
            ),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "--output", "json", "issue", "view", "PROJ-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["fields"]["duedate"], "2027-07-30",
        "AC-1: .fields.duedate must equal the fixture value verbatim, got: {json}"
    );
}

/// AC-2 (BC-2.3.036 [AMENDED]): `issue view --output json` shows
/// `.fields.duedate` as JSON `null` (present, not omitted) when unset.
/// Same JSON-branch reasoning as AC-1 — does not reach `render_due_date`.
#[tokio::test]
async fn test_issue_view_json_duedate_null_when_unset() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_duedate(
                "PROJ-2",
                "No due date yet",
                "To Do",
                None,
            ),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "--output", "json", "issue", "view", "PROJ-2"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        json["fields"]
            .as_object()
            .expect("fields is an object")
            .contains_key("duedate"),
        "AC-2: duedate key must be PRESENT (as null), not omitted, got: {json}"
    );
    assert!(
        json["fields"]["duedate"].is_null(),
        "AC-2: .fields.duedate must be JSON null when unset, got: {:?}",
        json["fields"]["duedate"]
    );
}

/// AC-3 (BC-2.2.028 / BC-2.2.032 JSON-mode clause): `issue list --output
/// json` includes `duedate` per row, unconditionally — no `--duedate` flag
/// is passed in this invocation. `handle_list` still computes rows/headers
/// via `format::format_issue_row`/`issue_table_headers` unconditionally of
/// output format, but with `show_duedate == false` neither
/// `render_due_date` nor the `"Due Date"` header push is reached.
#[tokio::test]
async fn test_issue_list_json_includes_duedate_unconditional_of_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_duedate(
                    "PROJ-1",
                    "Has a due date",
                    "To Do",
                    Some("2027-07-30"),
                ),
                common::fixtures::issue_response_with_duedate(
                    "PROJ-2",
                    "No due date",
                    "To Do",
                    None,
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "list",
            "--jql",
            "project = PROJ",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let arr = json.as_array().expect("issue list JSON is an array");
    assert_eq!(arr.len(), 2);
    assert_eq!(
        arr[0]["fields"]["duedate"], "2027-07-30",
        "AC-3: set duedate must round-trip verbatim, got: {arr:?}"
    );
    assert!(
        arr[1]["fields"]["duedate"].is_null(),
        "AC-3: unset duedate must serialize as JSON null, got: {arr:?}"
    );
}

/// AC-4 (BC-2.3.039): `issue view` human output ALWAYS shows a "Due Date"
/// row (unconditional, no flag), positioned between `Updated` and
/// `Project`, rendering the fixture value verbatim. This reaches
/// `format::render_due_date` unconditionally in `view.rs`'s Table arm.
#[tokio::test]
async fn test_issue_view_human_shows_due_date_row_when_set() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_duedate(
                "PROJ-1",
                "Ship the widget",
                "To Do",
                Some("2027-07-30"),
            ),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "issue", "view", "PROJ-1"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    let lines: Vec<&str> = stdout.lines().collect();
    let due_date_line = lines
        .iter()
        .position(|l| l.contains("Due Date"))
        .unwrap_or_else(|| panic!("AC-4: no 'Due Date' row found in stdout:\n{stdout}"));
    assert!(
        lines[due_date_line].contains("2027-07-30"),
        "AC-4: Due Date row must render the value verbatim, got line: {}",
        lines[due_date_line]
    );

    let updated_line = lines
        .iter()
        .position(|l| l.contains("Updated"))
        .expect("Updated row should exist");
    let project_line = lines
        .iter()
        .position(|l| l.contains("Project"))
        .expect("Project row should exist");
    assert!(
        updated_line < due_date_line && due_date_line < project_line,
        "AC-4: Due Date row must sit between Updated ({updated_line}) and \
         Project ({project_line}), got Due Date at {due_date_line}"
    );
}

/// AC-5 (BC-2.3.039 empty-rendering clause): `issue view` human output
/// shows `-` (not `(none)`) for an unset Due Date. Reaches
/// `render_due_date`.
#[tokio::test]
async fn test_issue_view_human_shows_dash_when_duedate_unset() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_duedate(
                "PROJ-2",
                "No due date yet",
                "To Do",
                None,
            ),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "issue", "view", "PROJ-2"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    let due_date_line = stdout
        .lines()
        .find(|l| l.contains("Due Date"))
        .unwrap_or_else(|| panic!("AC-5: no 'Due Date' row found in stdout:\n{stdout}"));
    let cells = table_cells(due_date_line);
    assert_eq!(
        cells.get(1).copied(),
        Some("-"),
        "AC-5: Due Date row must render '-' (not '(none)') when unset, got row: {cells:?}"
    );
}

/// AC-6 (BC-2.2.032 column-position clause): `issue list --duedate` shows
/// the column at `Key, Type, Status, Priority, Due Date, Assignee, Summary`
/// (no --points/--assets/--team flags), rendering the fixture value
/// verbatim. Reaches both the `"Due Date"` header push and `render_due_date`.
#[tokio::test]
async fn test_issue_list_duedate_flag_shows_column_correct_position() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_duedate(
                    "PROJ-1",
                    "Ship the widget",
                    "To Do",
                    Some("2027-07-30"),
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--no-input",
            "issue",
            "list",
            "--jql",
            "project = PROJ",
            "--duedate",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    let header_line = stdout
        .lines()
        .find(|l| l.contains("Key") && l.contains("Summary"))
        .unwrap_or_else(|| panic!("AC-6: no header row found in stdout:\n{stdout}"));
    let headers = table_cells(header_line);
    assert_eq!(
        headers,
        vec![
            "Key", "Type", "Status", "Priority", "Due Date", "Assignee", "Summary"
        ],
        "AC-6: column order must be Key, Type, Status, Priority, Due Date, \
         Assignee, Summary (no --points/--assets/--team passed) — got: {headers:?}"
    );

    let data_line = stdout
        .lines()
        .find(|l| l.contains("PROJ-1"))
        .expect("data row for PROJ-1 present");
    let cells = table_cells(data_line);
    let due_date_idx = headers
        .iter()
        .position(|h| *h == "Due Date")
        .expect("Due Date header present");
    assert_eq!(
        cells.get(due_date_idx).copied(),
        Some("2027-07-30"),
        "AC-6: Due Date cell must render the fixture value verbatim, got row: {cells:?}"
    );
}

/// AC-7 (BC-2.2.032 empty-rendering clause): `issue list --duedate` shows
/// `-` for an unset Due Date. Reaches both the `"Due Date"` header push and
/// `render_due_date`.
#[tokio::test]
async fn test_issue_list_duedate_flag_shows_dash_when_unset() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_duedate(
                    "PROJ-2",
                    "No due date",
                    "To Do",
                    None,
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--no-input",
            "issue",
            "list",
            "--jql",
            "project = PROJ",
            "--duedate",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    let header_line = stdout
        .lines()
        .find(|l| l.contains("Key") && l.contains("Summary"))
        .expect("header row present");
    let headers = table_cells(header_line);
    let due_date_idx = headers
        .iter()
        .position(|h| *h == "Due Date")
        .expect("Due Date header present");

    let data_line = stdout
        .lines()
        .find(|l| l.contains("PROJ-2"))
        .expect("data row for PROJ-2 present");
    let cells = table_cells(data_line);
    assert_eq!(
        cells.get(due_date_idx).copied(),
        Some("-"),
        "AC-7: Due Date cell must render '-' when unset, got row: {cells:?}"
    );
}

/// AC-8 (BC-2.2.032 opt-in clause): `issue list` WITHOUT `--duedate` omits
/// the column entirely — column absent, not merely hidden. Does NOT reach
/// `render_due_date`/the `"Due Date"` header push (`show_duedate == false`).
#[tokio::test]
async fn test_issue_list_without_duedate_flag_omits_column() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_duedate(
                    "PROJ-1",
                    "Ship the widget",
                    "To Do",
                    Some("2027-07-30"),
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "issue", "list", "--jql", "project = PROJ"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    // Positive anchors first: the table actually rendered a row and a
    // stable neighbor header, so the negative assertions below are not
    // vacuously true on an empty/errored table.
    assert!(
        stdout.contains("PROJ-1"),
        "AC-8: expected issue row PROJ-1 to be present, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Priority"),
        "AC-8: expected neighbor header 'Priority' to be present, got:\n{stdout}"
    );

    assert!(
        !stdout.contains("Due Date"),
        "AC-8: header must NOT contain 'Due Date' without --duedate, got:\n{stdout}"
    );
    let has_duedate_cell = stdout
        .lines()
        .any(|l| table_cells(l).contains(&"2027-07-30"));
    assert!(
        !has_duedate_cell,
        "AC-8: fixture due-date string must not appear as a distinct table \
         cell (column absent, not merely hidden), got:\n{stdout}"
    );
}

/// AC-9 (BC-2.2.032 JSON-mode clause): `--duedate --output json` is a
/// silent no-op — identical JSON shape to `--output json` alone, no
/// stderr warning. `handle_list` computes headers unconditionally of
/// output format, so with `--duedate` set this reaches the `"Due Date"`
/// header push even in JSON mode; the header must simply have no effect on
/// the JSON shape.
#[tokio::test]
async fn test_issue_list_duedate_flag_json_output_is_noop() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_duedate(
                    "PROJ-1",
                    "Ship the widget",
                    "To Do",
                    Some("2027-07-30"),
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let base_args = [
        "--no-input",
        "--output",
        "json",
        "issue",
        "list",
        "--jql",
        "project = PROJ",
    ];

    let with_flag = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(base_args)
        .arg("--duedate")
        .output()
        .unwrap();

    let without_flag = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(base_args)
        .output()
        .unwrap();

    let with_flag_stderr = String::from_utf8_lossy(&with_flag.stderr);
    let without_flag_stderr = String::from_utf8_lossy(&without_flag.stderr);
    assert!(with_flag.status.success(), "stderr: {with_flag_stderr}");
    assert!(
        without_flag.status.success(),
        "stderr: {without_flag_stderr}"
    );

    let with_flag_json: serde_json::Value = serde_json::from_slice(&with_flag.stdout).unwrap();
    let without_flag_json: serde_json::Value =
        serde_json::from_slice(&without_flag.stdout).unwrap();
    assert_eq!(
        with_flag_json, without_flag_json,
        "AC-9: --duedate must have zero effect on --output json shape"
    );

    assert!(
        !with_flag_stderr.contains("--duedate"),
        "AC-9: --duedate combined with --output json must not emit a \
         warning, got: {with_flag_stderr}"
    );
}

/// EC-5 (adversarial pass, S-668-1 LOW finding): `issue list --duedate
/// --points` combined — the full column order (Priority, Due Date, Points,
/// Assignee, …) is otherwise pinned only at the unit level
/// (`src/cli/issue/format.rs::tests::test_issue_table_headers_full_order_with_all_optional_columns`).
/// This is the CLI-level (wiremock) counterpart: it exercises the real
/// config → `resolve_show_points` → header-building path with both flags
/// set simultaneously, mirroring the config scaffolding used by
/// `tests/multi_profile_fields.rs::test_bc_6_3_001_points_column_present_after_save_round_trip`.
#[tokio::test]
async fn test_issue_list_duedate_and_points_columns_ordered_correctly() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Config with the story-points field id under [profiles.default], no
    // legacy [fields] block — mirrors write_single_profile_config_no_legacy_fields
    // in tests/multi_profile_fields.rs.
    let conf_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&conf_dir).unwrap();
    std::fs::write(
        conf_dir.join("config.toml"),
        r#"
default_profile = "default"

[profiles.default]
url = "https://acme.atlassian.net"
story_points_field_id = "customfield_10031"
"#,
    )
    .unwrap();

    // One issue carrying both a duedate and a story-points value.
    let issue = {
        let mut base = common::fixtures::issue_response_with_duedate(
            "PROJ-1",
            "Ship the widget",
            "To Do",
            Some("2027-07-30"),
        );
        base["fields"]["customfield_10031"] = serde_json::json!(5.0);
        base
    };

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(vec![issue])),
        )
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", &conf_dir)
        .args([
            "--no-input",
            "issue",
            "list",
            "--jql",
            "project = PROJ",
            "--duedate",
            "--points",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    let header_line = stdout
        .lines()
        .find(|l| l.contains("Key") && l.contains("Summary"))
        .unwrap_or_else(|| panic!("EC-5: no header row found in stdout:\n{stdout}"));
    let headers = table_cells(header_line);
    assert_eq!(
        headers,
        vec![
            "Key", "Type", "Status", "Priority", "Due Date", "Points", "Assignee", "Summary"
        ],
        "EC-5: combined --duedate --points column order must be Key, Type, \
         Status, Priority, Due Date, Points, Assignee, Summary — got: {headers:?}"
    );

    let data_line = stdout
        .lines()
        .find(|l| l.contains("PROJ-1"))
        .expect("data row for PROJ-1 present");
    let cells = table_cells(data_line);

    let due_date_idx = headers
        .iter()
        .position(|h| *h == "Due Date")
        .expect("Due Date header present");
    let points_idx = headers
        .iter()
        .position(|h| *h == "Points")
        .expect("Points header present");

    assert_eq!(due_date_idx, 4, "EC-5: Due Date must be at column index 4");
    assert_eq!(points_idx, 5, "EC-5: Points must be at column index 5");
    assert!(
        due_date_idx < points_idx,
        "EC-5: Due Date must appear before Points in the header row"
    );

    assert_eq!(
        cells.get(due_date_idx).copied(),
        Some("2027-07-30"),
        "EC-5: Due Date cell must render the fixture value, got row: {cells:?}"
    );
    assert_eq!(
        cells.get(points_idx).copied(),
        Some("5"),
        "EC-5: Points cell must render the fixture value, got row: {cells:?}"
    );
}

/// AC-16 (BC-2.2.032 Scope clause): `board view` does not gain a Due Date
/// column even when the underlying issue has a set `duedate` — the call
/// site passes `None` for the new parameter unconditionally. Does not
/// reach `render_due_date`/the `"Due Date"` header push (regression guard).
#[tokio::test]
async fn test_board_view_no_due_date_column_regardless_of_duedate_value() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("kanban")),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_duedate(
                    "PROJ-1",
                    "Ship the widget",
                    "To Do",
                    Some("2027-07-30"),
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--no-input",
            "--project",
            "PROJ",
            "board",
            "view",
            "--board",
            "42",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );
    // Positive anchors first: the table actually rendered a row and a
    // stable neighbor header, so the negative assertion below is not
    // vacuously true on an empty/errored table.
    assert!(
        stdout.contains("PROJ-1"),
        "AC-16: expected issue row PROJ-1 to be present, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Priority"),
        "AC-16: expected neighbor header 'Priority' to be present, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Due Date"),
        "AC-16: board view must not show a Due Date column, got:\n{stdout}"
    );
}

/// AC-16 (BC-2.2.032 Scope clause): `sprint current` does not gain a Due
/// Date column. Does not reach `render_due_date`/the `"Due Date"` header push
/// (regression guard).
#[tokio::test]
async fn test_sprint_current_no_due_date_column_regardless_of_duedate_value() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board"))
        .and(query_param("projectKeyOrId", "PROJ"))
        .and(query_param("type", "scrum"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::board_list_response(vec![common::fixtures::board_response(
                42,
                "PROJ Scrum Board",
                "scrum",
                "PROJ",
            )]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .and(query_param("state", "active"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::sprint_list_response(vec![common::fixtures::sprint(
                100, "Sprint 1", "active",
            )]),
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/sprint/100/issue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::sprint_issues_response(
                vec![common::fixtures::issue_response_with_duedate(
                    "PROJ-1",
                    "Ship the widget",
                    "To Do",
                    Some("2027-07-30"),
                )],
                1,
            ),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "--project", "PROJ", "sprint", "current"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );
    // Positive anchors first: the table actually rendered a row and a
    // stable neighbor header, so the negative assertion below is not
    // vacuously true on an empty/errored table.
    assert!(
        stdout.contains("PROJ-1"),
        "AC-16: expected issue row PROJ-1 to be present, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Priority"),
        "AC-16: expected neighbor header 'Priority' to be present, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Due Date"),
        "AC-16: sprint current must not show a Due Date column, got:\n{stdout}"
    );
}

/// AC-16 (BC-2.2.032 Scope clause): `queue view` does not gain a Due Date
/// column. Does not reach `render_due_date`/the `"Due Date"` header push
/// (regression guard).
#[tokio::test]
async fn test_queue_view_no_due_date_column_regardless_of_duedate_value() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ",
            "projectTypeKey": "service_desk",
            "simplified": false
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{"id": "15", "projectId": "10001", "projectName": "Test Project"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/15/queue/10/issue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "values": [{"key": "PROJ-1", "fields": {"summary": "Ship the widget"}}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![
                common::fixtures::issue_response_with_duedate(
                    "PROJ-1",
                    "Ship the widget",
                    "To Do",
                    Some("2027-07-30"),
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .args([
            "--no-input",
            "--project",
            "PROJ",
            "queue",
            "view",
            "--id",
            "10",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );
    // Positive anchors first: the table actually rendered a row and a
    // stable neighbor header, so the negative assertion below is not
    // vacuously true on an empty/errored table.
    assert!(
        stdout.contains("PROJ-1"),
        "AC-16: expected issue row PROJ-1 to be present, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Priority"),
        "AC-16: expected neighbor header 'Priority' to be present, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Due Date"),
        "AC-16: queue view must not show a Due Date column, got:\n{stdout}"
    );
}
