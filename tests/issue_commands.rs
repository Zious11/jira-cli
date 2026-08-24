#[allow(dead_code)]
mod common;

use wiremock::matchers::{any, body_partial_json, method, path, query_param};
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

// ═══════════════════════════════════════════════════════════════════════
// S-606-1: `jr issue list --component` filter (BC-2.1.018..022)
// ═══════════════════════════════════════════════════════════════════════
//
// 17 acceptance-criteria tests (AC-001..017) covering:
//   - bare `--component <NAME>` OR-composition (AC-001, AC-002)
//   - `not:<NAME>` OR-EMPTY exclusion form (AC-003, AC-004)
//   - bare + `not:` coexistence (AC-005)
//   - `none` reserved keyword (AC-006, AC-007, AC-008)
//   - `all:<N1>,<N2>` AND-composition (AC-009, AC-010, AC-011, AC-012)
//   - resolver failure zero-search guarantee (AC-013, AC-014, AC-015)
//   - BC-2.1.007 clause ordering (AC-016)
//   - reserved-syntax collision documentation (AC-017)
//
// All tests exercise the full CLI (`assert_cmd`) rather than the private
// `build_filter_clauses`/`resolve_component_clauses` functions directly —
// the composed JQL is observed via the actual outbound
// `POST /rest/api/3/search/jql` request body captured through
// `MockServer::received_requests()`, mirroring the pattern established in
// `tests/component_commands.rs` (AC-017 snapshot-JQL assertion).
//
// Implementation status: `resolve_component_clauses` and `build_filter_clauses`'s
// component branch are implemented; all 18 integration tests below are green,
// exercising the shipped `--component` filter behavior (BC-2.1.018..022).

/// Shared harness: `jr` CLI invocation pre-wired with an isolated cache and
/// config directory (per-test tempdirs) so the ADR-0018 components-cache
/// layer never leaks across tests or picks up a developer's real
/// `~/.cache/jr` / `~/.config/jr` state.
fn s606_1_cmd(
    server_uri: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache_dir)
        .env("JR_CONFIG_DIR", config_dir);
    cmd
}

/// Mounts `GET /rest/api/3/project/{key}` (the `project_exists` pre-flight
/// check `handle_list` runs whenever `--project`/a configured project is
/// present and `--status` is absent) so tests using `--project FOO` don't
/// fail on an unrelated 404 before ever reaching `--component` logic.
async fn s606_1_mock_project_exists(server: &MockServer, key: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/project/{key}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::project_response(
                key,
                "Test Project",
                "software",
                None,
            )),
        )
        .mount(server)
        .await;
}

/// Mounts `GET /rest/api/3/project/{key}/components` (the §8.4 resolver's
/// candidate-list fetch, BC-8.4.001) with the given component fixtures.
async fn s606_1_mock_components(
    server: &MockServer,
    key: &str,
    components: Vec<serde_json::Value>,
) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/project/{key}/components")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::component_list_response(components)),
        )
        .mount(server)
        .await;
}

/// Mounts a normal (unrestricted call-count) `POST /rest/api/3/search/jql`
/// response so the happy-path tests can complete and their composed JQL can
/// be inspected afterward via `s606_1_composed_jql`.
async fn s606_1_mock_search_empty(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(vec![])),
        )
        .mount(server)
        .await;
}

/// Extracts the `jql` string field from the single captured
/// `POST /rest/api/3/search/jql` request body.
async fn s606_1_composed_jql(server: &MockServer) -> String {
    let received = server.received_requests().await.unwrap();
    let search_req = received
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("S-606-1: search/jql request must have fired");
    let body: serde_json::Value =
        serde_json::from_slice(&search_req.body).expect("S-606-1: body must be valid JSON");
    body["jql"]
        .as_str()
        .expect("S-606-1: jql field must be a string")
        .to_string()
}

/// VP-COMPONENT-013 zero-search guarantee: mounts a catch-all
/// `.expect(0)` on `POST /rest/api/3/search/jql` — any component
/// resolution failure or precondition rejection MUST short-circuit before
/// the issue search ever fires.
async fn s606_1_expect_zero_search(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_search_response(vec![])),
        )
        .expect(0)
        .mount(server)
        .await;
}

/// Strongest form of the zero-HTTP guarantee: registers catch-all
/// `.expect(0)` mocks for ANY GET and ANY POST request. Used for the
/// pre-flight precondition guards (`none` combination, repeated `all:`,
/// `all:`+bare mixing, no-project-scope) which BC-2.1.020/021/022 document
/// as firing with literally ZERO HTTP calls — not merely zero resolver/
/// search calls — because the guard is evaluated purely from the CLI-
/// supplied `--component` values, before project validation or resolution.
async fn s606_1_expect_zero_http(server: &MockServer) {
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(server)
        .await;
}

// ── AC-001 (BC-2.1.018 postcondition 1 — OR composition) ─────────────────

/// `--component Backend --component Frontend` composes ONE clause
/// `component in (10001, 10002)` (input order preserved), not two separate
/// clauses.
#[tokio::test]
async fn test_bc_2_1_018_issue_list_component_repeated_or_composed_single_clause() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "Backend",
            "--component",
            "Frontend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-001: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("component in (10001, 10002)"),
        "AC-001: expected single OR-composed clause 'component in (10001, 10002)' in jql: {jql}"
    );
    assert!(
        !jql.contains("component in (10001) AND component in (10002)"),
        "AC-001: must NOT emit two separate clauses, got jql: {jql}"
    );
}

/// Step-4.5 LOW-1: `--component Frontend --component Backend` (input order
/// INVERTED relative to id-ascending order) must compose `component in
/// (10002, 10001)` — INPUT order preserved, not sorted to id-ascending
/// `(10001, 10002)`. Every other multi-value test in this suite supplies
/// Backend before Frontend, where input order happens to coincide with
/// id-ascending order, so a wrong impl that sorted ids (e.g. via a
/// `BTreeSet`/`.sort()`) would still pass those tests undetected. This test
/// inverts the order to catch exactly that regression.
#[tokio::test]
async fn test_bc_2_1_018_issue_list_component_or_preserves_input_order_not_sorted() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "Frontend",
            "--component",
            "Backend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "LOW-1: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("component in (10002, 10001)"),
        "LOW-1: expected input-order-preserved clause 'component in (10002, 10001)' in jql: {jql}"
    );
    assert!(
        !jql.contains("component in (10001, 10002)"),
        "LOW-1: must NOT sort ids to id-ascending order, got jql: {jql}"
    );
}

// ── AC-002 (BC-2.1.018 EC-2.1.018-1 — single value) ───────────────────────

/// `--component Backend` alone → `component in (10001)`, NOT rewritten to
/// `component = 10001`.
#[tokio::test]
async fn test_bc_2_1_018_issue_list_component_single_value_stays_in_clause() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "Backend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-002: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("component in (10001)"),
        "AC-002: expected 'component in (10001)' in jql: {jql}"
    );
    assert!(
        !jql.contains("component = 10001"),
        "AC-002: single value must NOT be rewritten to 'component = 10001', got jql: {jql}"
    );
}

// ── AC-003 (BC-2.1.019 Postcondition 1 — OR-EMPTY form) ──────────────────

/// `--component not:Frontend` → the FULL parenthesized `(component not in
/// (10002) OR component is EMPTY)` form, never a bare `not in`.
#[tokio::test]
async fn test_bc_2_1_019_issue_list_component_not_composes_or_empty_form() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10002", "Frontend", None, None, None,
        )],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "not:Frontend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-003: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("(component not in (10002) OR component is EMPTY)"),
        "AC-003: expected full OR-EMPTY form in jql: {jql}"
    );
    // Step-4.5 AC-003 tidy: the two negatives this replaced (a stray-space
    // variant and a double-paren variant) could never match regardless of
    // correctness, so they added no protection. This meaningful negative
    // instead proves the OR-EMPTY wrapper is actually present: every
    // occurrence of the bare inner clause `component not in (10002)` must
    // be part of the full wrapped form — a regression that emitted the bare
    // `not in` clause without the OR-EMPTY wrapper would desync these two
    // counts.
    let bare_not_count = jql.matches("component not in (10002)").count();
    let wrapped_count = jql
        .matches("(component not in (10002) OR component is EMPTY)")
        .count();
    assert_eq!(
        bare_not_count, wrapped_count,
        "AC-003: every 'component not in (10002)' occurrence must be wrapped in the OR-EMPTY form, got jql: {jql}"
    );
}

// ── AC-004 (BC-2.1.019 EC-2.1.019-1 — multiple not: in one group) ────────

/// `--component not:Backend --component not:Frontend` → ONE clause
/// `(component not in (10001, 10002) OR component is EMPTY)`, not two.
#[tokio::test]
async fn test_bc_2_1_019_issue_list_component_multiple_not_single_group() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "not:Backend",
            "--component",
            "not:Frontend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-004: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("(component not in (10001, 10002) OR component is EMPTY)"),
        "AC-004: expected single grouped OR-EMPTY clause in jql: {jql}"
    );
    assert!(
        jql.matches("OR component is EMPTY").count() == 1,
        "AC-004: expected exactly ONE OR-EMPTY group, got jql: {jql}"
    );
}

// ── AC-005 (BC-2.1.018 Precondition 3 / BC-2.1.019 Postcondition 2 —
//    bare+not: coexist) ────────────────────────────────────────────────

/// `--component Backend --component not:Frontend` → BOTH clauses compose,
/// AND-joined, bare FIRST: `component in (10001) AND (component not in
/// (10002) OR component is EMPTY)`.
#[tokio::test]
async fn test_bc_2_1_018_issue_list_component_bare_and_not_coexist_bare_first() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "Backend",
            "--component",
            "not:Frontend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-005: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("component in (10001) AND (component not in (10002) OR component is EMPTY)"),
        "AC-005: expected bare-then-not: AND-joined composition in jql: {jql}"
    );
}

// ── AC-006 (BC-2.1.020 Postcondition 1 — reserved keyword, zero resolver
//    HTTP) ─────────────────────────────────────────────────────────────

/// `--component none` → `component is EMPTY`, ZERO §8.4 resolver HTTP (no
/// candidate-list GET fires for `none` specifically).
#[tokio::test]
async fn test_bc_2_1_020_issue_list_component_none_zero_resolver_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    // The resolver GET must NEVER fire for `none` — VP-COMPONENT-015.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::component_list_response(vec![common::fixtures::component_response(
                "10001", "Backend", None, None, None,
            )]),
        ))
        .expect(0)
        .mount(&server)
        .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "none",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-006: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("component is EMPTY"),
        "AC-006: expected 'component is EMPTY' in jql: {jql}"
    );
}

// INFO-1 (Step-4.5 adversarial finding, cheap coverage add): BC-2.1.020's
// `none` keyword match is documented case-insensitive, but AC-006 only
// exercises the exact lowercase spelling `"none"`. Pins `--component NONE`
// (uppercase) and `--component None` (mixed case) against the same
// zero-resolver-HTTP / `component is EMPTY` behavior as AC-006.

/// `--component NONE` (uppercase) and `--component None` (mixed case) both
/// compose `component is EMPTY` with ZERO §8.4 resolver HTTP, identically to
/// `--component none` (AC-006) — BC-2.1.020 case-insensitivity.
#[tokio::test]
async fn test_bc_2_1_020_issue_list_component_none_case_insensitive() {
    for spelling in ["NONE", "None"] {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        s606_1_mock_project_exists(&server, "FOO").await;
        // The resolver GET must NEVER fire for any case-variant of `none` —
        // VP-COMPONENT-015 / BC-2.1.020 case-insensitivity.
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project/FOO/components"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                common::fixtures::component_list_response(vec![
                    common::fixtures::component_response("10001", "Backend", None, None, None),
                ]),
            ))
            .expect(0)
            .mount(&server)
            .await;
        s606_1_mock_search_empty(&server).await;

        let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "list",
                "--project",
                "FOO",
                "--component",
                spelling,
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "INFO-1 ({spelling}): expected exit 0, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let jql = s606_1_composed_jql(&server).await;
        assert!(
            jql.contains("component is EMPTY"),
            "INFO-1 ({spelling}): expected 'component is EMPTY' in jql: {jql}"
        );
    }
}

// ── AC-007 (BC-2.1.020 Behavior — combination rejection) ─────────────────

/// `--component none --component Backend` → exit 64 pre-flight, ZERO HTTP —
/// `none` rejects ANY other `--component` occurrence.
#[tokio::test]
async fn test_bc_2_1_020_issue_list_component_none_combination_rejected() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_expect_zero_http(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "none",
            "--component",
            "Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-007: expected exit 64, got {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("--component none cannot be combined with other --component values."),
        "AC-007: expected exact BC-2.1.020 message, got stderr: {stderr}"
    );
}

// ── AC-008 (BC-2.1.020 Precondition 2 / EC-2.1.020-3 — project-scope
//    requirement) ─────────────────────────────────────────────────────

/// `--component none` with no `--project` and no configured project → exit
/// 64 pre-flight, ZERO HTTP — `none` is NOT exempt from project-scoping.
#[tokio::test]
async fn test_bc_2_1_020_issue_list_component_none_requires_project_scope() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    // No .jr.toml written — cwd carries no default project.
    let project_dir = tempfile::tempdir().unwrap();

    s606_1_expect_zero_http(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .current_dir(project_dir.path())
        .args(["--no-input", "issue", "list", "--component", "none"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-008: expected exit 64, got {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "--component none requires --project (or a configured default project) to avoid an unrestricted org-wide search."
        ),
        "AC-008: expected exact BC-2.1.022 EC-2.1.022-2 message, got stderr: {stderr}"
    );
}

// ── AC-009 (BC-2.1.021 Postcondition 1 — AND composition) ────────────────

/// `--component all:Backend,Frontend` → `component = 10001 AND component =
/// 10002` (repeated equality, NOT `IN`).
#[tokio::test]
async fn test_bc_2_1_021_issue_list_component_all_and_composed_repeated_equality() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "all:Backend,Frontend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-009: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("component = 10001 AND component = 10002"),
        "AC-009: expected repeated-equality AND clause in jql: {jql}"
    );
    assert!(
        !jql.contains("component in ("),
        "AC-009: must NOT use IN for all:, got jql: {jql}"
    );
}

// ── AC-010 (BC-2.1.021 Precondition 1 — repeated all: rejected) ──────────

/// `--component all:X --component all:Y` (two `all:` occurrences) → exit
/// 64, exact BC-2.1.021 message.
#[tokio::test]
async fn test_bc_2_1_021_issue_list_component_repeated_all_prefix_rejected() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_expect_zero_http(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "all:X",
            "--component",
            "all:Y",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-010: expected exit 64, got {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "--component all: may only be specified once; comma-separate multiple names within one all: value."
        ),
        "AC-010: expected exact BC-2.1.021 Precondition 1 message, got stderr: {stderr}"
    );
}

// ── AC-011 (BC-2.1.021 Precondition 2 / EC-2.1.021-2 — all:+bare
//    rejected) ─────────────────────────────────────────────────────────

/// `--component all:Backend --component Frontend` (mixing `all:` with a
/// bare value) → exit 64 pre-flight, zero HTTP.
#[tokio::test]
async fn test_bc_2_1_021_issue_list_component_all_mixed_with_bare_rejected() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_expect_zero_http(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "all:Backend",
            "--component",
            "Frontend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-011: expected exit 64, got {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        !stderr.is_empty(),
        "AC-011: expected a non-empty rejection message"
    );
    assert!(
        stderr.contains("--component all: cannot be combined with other --component values."),
        "AC-011: expected exact all:+bare-combination rejection message, got stderr: {stderr}"
    );
}

// ── AC-012 (BC-2.1.021 EC-2.1.021-1 — single-name all: degenerates) ──────

/// `--component all:Backend` (single name, no comma) → `component = 10001`
/// (one-term AND, a DIFFERENT code path from `--component Backend`).
#[tokio::test]
async fn test_bc_2_1_021_issue_list_component_all_single_name_degenerates() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "all:Backend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-012: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("component = 10001"),
        "AC-012: expected 'component = 10001' in jql: {jql}"
    );
    assert!(
        !jql.contains("component in (10001)"),
        "AC-012: all:'s single-name degenerate form must NOT go through the bare IN path, got jql: {jql}"
    );
}

// ── AC-013 (BC-2.1.022 Behavior — zero-match resolver failure) ───────────

/// `--component BadName` (zero matches) → exit 64, exact BC-8.4.002
/// message; `POST /rest/api/3/search/jql` NEVER called (VP-COMPONENT-013).
#[tokio::test]
async fn test_bc_2_1_022_issue_list_component_unknown_name_zero_search() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    // Deliberately unsorted fixture order to prove the "Available:" list is
    // sorted alphabetically by the implementation, not passed through as-is.
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10002", "Frontend", None, None, None),
            common::fixtures::component_response("10001", "Backend", None, None, None),
        ],
    )
    .await;
    s606_1_expect_zero_search(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "BadName",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-013: expected exit 64, got {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "Component 'BadName' not found in project FOO. Available: Backend, Frontend."
        ),
        "AC-013: expected exact BC-8.4.002 message with alphabetically-sorted \
         Available list, got stderr: {stderr}"
    );
}

// ── AC-014 (BC-2.1.022 Behavior — ambiguous resolver failure) ────────────

/// `--component Amb` (2+ matches) → exit 64, exact BC-8.4.003 message; zero
/// JQL search calls.
#[tokio::test]
async fn test_bc_2_1_022_issue_list_component_ambiguous_name_zero_search() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    // Deliberately reverse-alphabetical fixture order (mirrors AC-013's
    // "Available:" test above) so the "Matches:" assertion below can only
    // pass if the implementation actually sorts the candidates — with the
    // fixture already alphabetical, deleting `candidates.sort_by_key` at
    // `list.rs`'s ambiguous-match branch would go undetected.
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("20002", "Ambition", None, None, None),
            common::fixtures::component_response("20001", "Amber", None, None, None),
        ],
    )
    .await;
    s606_1_expect_zero_search(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "Amb",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-014: expected exit 64, got {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Ambiguous component 'Amb'. Matches: Amber, Ambition."),
        "AC-014: expected exact BC-8.4.003 message, got stderr: {stderr}"
    );
}

// ── AC-015 (BC-2.1.022 EC-2.1.022-1 — no project scope, bare/not:/all:) ──

/// `--component <NAME>` (bare) with no `--project` and no configured
/// project → exit 64 pre-flight BEFORE attempting the resolver GET, naming
/// `--project`.
#[tokio::test]
async fn test_bc_2_1_022_issue_list_component_no_project_scope_exits_64_before_get() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();

    s606_1_expect_zero_http(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .current_dir(project_dir.path())
        .args(["--no-input", "issue", "list", "--component", "Backend"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-015: expected exit 64, got {:?}, stderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "--component requires --project (or a configured default project) to resolve component names."
        ),
        "AC-015: expected exact BC-2.1.022 message, got stderr: {stderr}"
    );
}

// ── AC-016 (BC-2.1.007 amendment — clause ordering) ───────────────────────

/// `--assignee me --component Backend --created-after 2026-01-01` composes
/// clauses in the stable order with `component` positioned AFTER `asset`
/// (absent here) and BEFORE `created-after`/`updated-after`.
#[tokio::test]
async fn test_bc_2_1_007_issue_list_component_clause_ordering_after_asset_before_dates() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--assignee",
            "me",
            "--component",
            "Backend",
            "--created-after",
            "2026-01-01",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-016: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    let assignee_idx = jql
        .find("assignee = currentUser()")
        .expect("AC-016: assignee clause must be present");
    let component_idx = jql
        .find("component in (10001)")
        .expect("AC-016: component clause must be present");
    let created_idx = jql
        .find("created >= \"2026-01-01\"")
        .expect("AC-016: created-after clause must be present");

    assert!(
        assignee_idx < component_idx,
        "AC-016: component must come AFTER assignee, got jql: {jql}"
    );
    assert!(
        component_idx < created_idx,
        "AC-016: component must come BEFORE created-after, got jql: {jql}"
    );
}

// ── AC-017 (BC-2.1.019/020/021 reserved-syntax collision documentation) ──

/// A component literally named `"none"`, `"not:Deprecated"`, or
/// `"all:Backend"` is unreachable via the corresponding `--component`
/// form — the reserved prefix/keyword always short-circuits before the
/// literal name could ever be selected. Verified structurally across all
/// three reserved forms in one test.
#[tokio::test]
async fn test_bc_2_1_019_020_021_reserved_syntax_collisions_short_circuit_documented() {
    // ── (a) `none` — literal component named "none" is unreachable; zero
    //    resolver HTTP proves the keyword short-circuits before any name
    //    lookup could even see the literal "none" component. ──
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        s606_1_mock_project_exists(&server, "FOO").await;
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project/FOO/components"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                common::fixtures::component_list_response(vec![
                    common::fixtures::component_response("40001", "none", None, None, None),
                ]),
            ))
            .expect(0)
            .mount(&server)
            .await;
        s606_1_mock_search_empty(&server).await;

        let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "list",
                "--project",
                "FOO",
                "--component",
                "none",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "AC-017a: expected exit 0, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let jql = s606_1_composed_jql(&server).await;
        assert!(
            jql.contains("component is EMPTY"),
            "AC-017a: 'none' keyword must short-circuit to 'component is EMPTY' \
             even when a literal component named \"none\" exists, got jql: {jql}"
        );
    }

    // ── (b) `not:Deprecated` — literal component named "not:Deprecated"
    //    is unreachable; the prefix strips and resolves "Deprecated"
    //    (id 30001), never the literal "not:Deprecated" (id 30002). ──
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        s606_1_mock_project_exists(&server, "FOO").await;
        s606_1_mock_components(
            &server,
            "FOO",
            vec![
                common::fixtures::component_response("30001", "Deprecated", None, None, None),
                common::fixtures::component_response("30002", "not:Deprecated", None, None, None),
            ],
        )
        .await;
        s606_1_mock_search_empty(&server).await;

        let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "list",
                "--project",
                "FOO",
                "--component",
                "not:Deprecated",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "AC-017b: expected exit 0, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let jql = s606_1_composed_jql(&server).await;
        assert!(
            jql.contains("(component not in (30001) OR component is EMPTY)"),
            "AC-017b: 'not:Deprecated' must resolve to the id of the component \
             named \"Deprecated\" (30001), never the literal \
             \"not:Deprecated\" component (30002), got jql: {jql}"
        );
        assert!(
            !jql.contains("30002"),
            "AC-017b: literal 'not:Deprecated' component (30002) must never \
             appear in the composed clause, got jql: {jql}"
        );
    }

    // ── (c) `all:Backend` — literal component named "all:Backend" is
    //    unreachable; the prefix strips and resolves "Backend" (id 50001),
    //    never the literal "all:Backend" (id 50002). ──
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        s606_1_mock_project_exists(&server, "FOO").await;
        s606_1_mock_components(
            &server,
            "FOO",
            vec![
                common::fixtures::component_response("50001", "Backend", None, None, None),
                common::fixtures::component_response("50002", "all:Backend", None, None, None),
            ],
        )
        .await;
        s606_1_mock_search_empty(&server).await;

        let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "list",
                "--project",
                "FOO",
                "--component",
                "all:Backend",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "AC-017c: expected exit 0, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let jql = s606_1_composed_jql(&server).await;
        assert!(
            jql.contains("component = 50001"),
            "AC-017c: 'all:Backend' must resolve to the id of the component \
             named \"Backend\" (50001), never the literal \"all:Backend\" \
             component (50002), got jql: {jql}"
        );
        assert!(
            !jql.contains("50002"),
            "AC-017c: literal 'all:Backend' component (50002) must never \
             appear in the composed clause, got jql: {jql}"
        );
    }
}

// ── F5-A-M1/F5-C-001 (2026-08-17, human-adjudicated: UNION) — ExactMultiple
//    read-path disposition (BC-2.1.018 Postcondition 3, BC-2.1.019
//    Postcondition 3, BC-2.1.021 Postcondition 2, BC-2.1.022
//    EC-2.1.022-3 / "ExactMultiple read-path disposition") ────────────────
//
// Fixture shared across all four tests below: project FOO has a case-only
// duplicate component pair — `Backend` (id 10001) and `backend` (id 10005)
// — plus a benign, non-duplicated `Frontend` (id 10002). `partial_match`
// resolves any of `Backend`/`backend` to `MatchResult::ExactMultiple`
// ("Backend" — the first-encountered spelling); the current (pre-fix)
// implementation silently first-picks a single id from that result, which
// is the exact silent-incomplete-filter defect this amendment closes. Each
// test below MUST fail red against that first-pick implementation.

/// BC-2.1.018 Postcondition 3 / EC-2.1.018-3: a single bare `--component
/// Backend` value that resolves to `MatchResult::ExactMultiple` (case-only
/// duplicate `Backend`/10001 + `backend`/10005) must UNION both ids into the
/// `in (...)` clause — `component in (10001, 10005)` — never silently
/// first-pick a single id.
#[tokio::test]
async fn test_bc_2_1_018_issue_list_component_exactmultiple_unions_all_ids() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10005", "backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "Backend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "EC-2.1.018-3: expected exit 0 (UNION, not an ambiguity error), stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("component in (10001, 10005)"),
        "EC-2.1.018-3: expected UNIONed clause 'component in (10001, 10005)' \
         (ascending numeric order within the ExactMultiple value's own slot), \
         got jql: {jql}"
    );
    assert!(
        !jql.contains("component in (10001)"),
        "EC-2.1.018-3: must NOT silently first-pick a single-id clause \
         'component in (10001)' — that is the exact silent-incomplete-filter \
         defect this amendment closes, got jql: {jql}"
    );
}

/// BC-2.1.019 Postcondition 3 / EC-2.1.019-4: a single `--component
/// not:Backend` value that resolves to `MatchResult::ExactMultiple` must
/// UNION both duplicate ids into the exclusion set — `(component not in
/// (10001, 10005) OR component is EMPTY)` — never exclude only one
/// duplicate.
#[tokio::test]
async fn test_bc_2_1_019_issue_list_component_not_exactmultiple_unions_all_ids() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10005", "backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "not:backend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "EC-2.1.019-4: expected exit 0 (UNION, not an ambiguity error), stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("(component not in (10001, 10005) OR component is EMPTY)"),
        "EC-2.1.019-4: expected UNIONed exclusion clause '(component not in \
         (10001, 10005) OR component is EMPTY)', got jql: {jql}"
    );
    assert!(
        !jql.contains("(component not in (10001) OR component is EMPTY)"),
        "EC-2.1.019-4: must NOT silently first-pick a single-id exclusion — \
         an issue carrying only the other duplicate would incorrectly match \
         under that defect, got jql: {jql}"
    );
}

/// BC-2.1.021 Postcondition 2 / EC-2.1.021-4: a name within an `all:` list
/// that resolves to `MatchResult::ExactMultiple` becomes a parenthesized
/// OR-of-equalities term — `(component = 10001 OR component = 10005)` —
/// standing in for that one name's position in the AND-chain, never a bare
/// single-id equality.
#[tokio::test]
async fn test_bc_2_1_021_issue_list_component_all_exactmultiple_or_of_equalities() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10005", "backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "all:backend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "EC-2.1.021-4: expected exit 0 (UNION, not an ambiguity error), stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("(component = 10001 OR component = 10005)"),
        "EC-2.1.021-4: expected parenthesized OR-of-equalities term \
         '(component = 10001 OR component = 10005)', got jql: {jql}"
    );
    assert!(
        jql.contains("OR component = 10005"),
        "EC-2.1.021-4: must NOT silently first-pick a bare 'component = \
         10001' equality with no OR-group — that is the exact \
         silent-incomplete-filter defect this amendment closes, got jql: {jql}"
    );
}

/// BC-2.1.021 Postcondition 2 mixed case: `all:backend,Frontend` — the first
/// comma-separated name (`backend`) resolves ExactMultiple and becomes a
/// parenthesized OR-of-equalities term; the second (`Frontend`) resolves to
/// a single id and stays a bare equality. Both AND-join in comma-supplied
/// order: `(component = 10001 OR component = 10005) AND component = 10002`.
#[tokio::test]
async fn test_bc_2_1_021_issue_list_component_all_mixed_exactmultiple_and_ordinary() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10005", "backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "all:backend,Frontend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "EC-2.1.021-4 (mixed): expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("(component = 10001 OR component = 10005) AND component = 10002"),
        "EC-2.1.021-4 (mixed): expected \
         '(component = 10001 OR component = 10005) AND component = 10002' \
         (ExactMultiple OR-group first, in comma-supplied order, AND-joined \
         with the ordinary single-id term), got jql: {jql}"
    );
}

// ── SEC-707-1 (PR #707 fast-follow, LOW — injection-safety regression pin) ─

/// Pins the injection-safety guarantee noted in the PR #707 review: only the
/// resolved NUMERIC `Component.id` ever reaches the composed JQL — never a
/// raw, potentially attacker-influenced component NAME — even when that name
/// is itself crafted with JQL metacharacters. A component whose NAME embeds
/// a JQL-breakout attempt (`Back"end) OR 1=1--`) must still resolve to
/// nothing but its numeric id (10001) in the final clause; none of the raw
/// metachar substrings from the name may leak into the composed JQL string.
/// This is injection-safe by construction today (`resolve_one_component_id`
/// only ever pushes the resolved `id` into the clause, never `name`) — this
/// test pins that guarantee as a regression check.
#[tokio::test]
async fn test_bc_2_1_018_issue_list_component_name_with_jql_metachars_uses_numeric_id_only() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let metachar_name = "Back\"end) OR 1=1--";

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", metachar_name, None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            metachar_name,
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "SEC-707-1: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("component in (10001)"),
        "SEC-707-1: expected the resolved numeric-id clause \
         'component in (10001)' in jql: {jql}"
    );
    assert!(
        !jql.contains("OR 1=1"),
        "SEC-707-1: raw JQL-breakout substring 'OR 1=1' from the component \
         NAME must never reach the composed JQL, got jql: {jql}"
    );
    assert!(
        !jql.contains("Back\"end"),
        "SEC-707-1: raw component NAME substring 'Back\"end' must never \
         reach the composed JQL, got jql: {jql}"
    );
    assert!(
        !jql.contains("Back\"end)"),
        "SEC-707-1: raw component NAME substring 'Back\"end)' (including the \
         literal ')' embedded in the attacker-controlled name) must never \
         reach the composed JQL verbatim, got jql: {jql}"
    );
}

// ── --component + --jql interaction (PR #707 fast-follow, LOW) ───────────

/// Pins the CURRENT `--component` + `--jql` interaction (pr-reviewer LOW
/// finding — previously untested): both flags compose via a plain AND —
/// `--jql` does NOT silently override or ignore `--component`, nor does
/// `--component` ever suppress `--jql`. `handle_list` (`src/cli/issue/
/// list.rs`) builds the JQL base from `--jql` unconditionally of
/// `--component`'s presence (`build_jql_base_parts`: project-scope-prefixed,
/// then the user's raw `--jql` expression parenthesized), and separately
/// resolves `--component` into `component_clauses` before ever branching on
/// whether `--jql` was supplied; `all_parts = base_parts + filter_parts`
/// (which includes `component_clauses`) is then AND-joined unconditionally.
/// There is no `if jql.is_some() { skip component }`-shaped special case —
/// `--component` composes with `--jql` exactly the same way every other
/// filter flag already does (`--assignee`, `--status`, `--created-after`,
/// ...; see the BC-2.1.007 clause-ordering test above and
/// `tests/issue_read_holdouts.rs::test_s_2_01_h_035_...`).
#[tokio::test]
async fn test_bc_2_1_018_issue_list_component_with_jql_flag_current_behavior() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--component",
            "Backend",
            "--jql",
            "status = \"Open\"",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert_eq!(
        jql,
        "project = \"FOO\" AND (status = \"Open\") AND component in (10001) ORDER BY updated DESC",
        "current behavior: --component and --jql compose via AND, neither \
         overrides the other; got jql: {jql}"
    );
}

// =============================================================================
// S-605-1: `issue create --component` / `issue edit --component` (single-key)
// =============================================================================
//
// 17 acceptance criteria (AC-001..AC-017) covering:
//   - BC-3.4.022: single-key `edit --component add:/remove:` native
//     `update`-verb wire shape + editmeta-gated read-modify-write fallback
//     (AC-001..006)
//   - BC-3.4.024: `create --component` bare additive body composition +
//     the `--request-type` pre-flight guard (AC-007..009)
//   - BC-3.4.025: resolution-mechanism invariant — ONE project component-
//     list GET, never duplicated with editmeta (AC-010)
//   - BC-3.4.012/013 amendments: table/JSON changed-fields echo, prefixed
//     and bare-normalized (AC-011..013)
//   - BC-3.4.017 amendment: Gate B's flag-overlap set gains `components`
//     as its 5th member (AC-014)
//   - BC-3.4.020 amendment: `--label` + `--component` mutual exclusion
//     (AC-015)
//   - BC-3.4.021 amendment: `--dry-run` `plannedChanges.components` +
//     table preview, bare-normalization parity with the live echo
//     (AC-016..017)
//
// These tests were authored red against `todo!()` stubs in
// `format::normalize_component_changes`, `format::format_component_changes_echo`,
// `format::component_changes_dry_run_json`, `format::ComponentAction::as_wire_str`,
// `edit_issue_components`, `resolve_create_components`, and three inline guard
// sites in `edit.rs`/`create.rs` — every cited call site is now implemented.
// See story `.factory/stories/S-605-1-issue-component-single-key.md`.
//
// Step-4.5 Round 1 review (F1/F2) added three further tests below AC-017:
// dry-run --component now resolves names (exits 64 on an unresolvable name,
// same as the live path) instead of skipping resolution, and the components
// echo/dry-run preview render in CLI input order rather than the wire's
// ADD-before-REMOVE reordering (labels precedent, EC-3.4.020-8).

/// Shared harness: plain `jr` CLI invocation, no XDG isolation needed —
/// component resolution has no cache wired into this read/resolve path yet
/// (CLAUDE.md `cache.rs` note: the ADR-0018 §2 components-cache functions
/// exist but are "not yet wired into any read/resolve path this cycle").
fn s605_1_cmd(server_uri: &str) -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0");
    cmd
}

/// Mounts `GET /rest/api/3/project/{key}/components` (the §8.4 resolver's
/// candidate-list fetch, BC-8.4.001/BC-3.4.025) with the given component
/// fixtures. Mirrors `s606_1_mock_components`.
async fn s605_1_mock_components(
    server: &MockServer,
    key: &str,
    components: Vec<serde_json::Value>,
) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/project/{key}/components")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::component_list_response(components)),
        )
        .mount(server)
        .await;
}

/// Mounts `GET /rest/api/3/issue/{key}/editmeta`, scoped to the
/// `components` field only, with the given `operations` list (BC-3.4.022
/// Postcondition 3's wire-shape-decision gate — `&["add","remove"]` selects
/// the native path, anything else selects the read-modify-write fallback).
async fn s605_1_mock_editmeta(server: &MockServer, key: &str, operations: &[&str]) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/issue/{key}/editmeta")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::editmeta_components_response(operations)),
        )
        .mount(server)
        .await;
}

/// Mounts a decoy `GET /rest/api/3/project/{key}/components` for a
/// DIFFERENT project than the one under test, `.expect(0)` — the
/// cross-project non-collision fixture the story's Edge Cases section
/// calls for ("`--component` name resolution reuses the SAME resolver
/// contract as `component edit`/`delete` … a cross-project non-collision
/// fixture … scoped to the issue-write call sites"). Proves
/// `resolve_component`'s project scoping is honored end-to-end: a
/// same-named component in another project must never be queried, let
/// alone matched, by an issue-write `--component` resolution.
async fn s605_1_mock_decoy_project_components(server: &MockServer, decoy_key: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/project/{decoy_key}/components")))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::component_list_response(vec![common::fixtures::component_response(
                "99001", "Backend", None, None, None,
            )]),
        ))
        .expect(0)
        .mount(server)
        .await;
}

/// Captures every request received at `PUT /rest/api/3/issue/{key}`,
/// parsed as JSON, in arrival order. Deliberately does NOT filter by body
/// shape — a stray extra PUT (e.g. `handle_edit`'s generic post-component
/// `client.edit_issue(key, fields)` call firing with an empty `fields`
/// object when `--component` is the only flag supplied) must be visible
/// here, not silently swallowed by a shape filter.
async fn s605_1_captured_puts(server: &MockServer, key: &str) -> Vec<serde_json::Value> {
    let received = server.received_requests().await.unwrap();
    let want_path = format!("/rest/api/3/issue/{key}");
    received
        .iter()
        .filter(|r| r.method == wiremock::http::Method::PUT && r.url.path() == want_path)
        .map(|r| {
            serde_json::from_slice(&r.body)
                .unwrap_or_else(|e| panic!("PUT body must be valid JSON: {e}"))
        })
        .collect()
}

/// Captures every request received at `POST /rest/api/3/issue`, parsed as
/// JSON, in arrival order.
async fn s605_1_captured_posts(server: &MockServer) -> Vec<serde_json::Value> {
    let received = server.received_requests().await.unwrap();
    received
        .iter()
        .filter(|r| r.method == wiremock::http::Method::POST && r.url.path() == "/rest/api/3/issue")
        .map(|r| {
            serde_json::from_slice(&r.body)
                .unwrap_or_else(|e| panic!("POST body must be valid JSON: {e}"))
        })
        .collect()
}

// ── AC-001 (BC-3.4.022 Postcondition 1 — native wire shape) ────────────────

/// `jr issue edit FOO-1 --component add:Backend --component remove:Frontend`
/// (single key) → exactly ONE `PUT /rest/api/3/issue/FOO-1` with body
/// `{"update":{"components":[{"add":{"name":"Backend"}},
/// {"remove":{"name":"Frontend"}}]}}` (VP-COMPONENT-011). The bulk endpoint
/// is never hit. Also proves cross-project non-collision: a decoy project
/// "BAR" carrying a same-named "Backend" component is never queried.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_add_remove_native_wire_shape() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s605_1_mock_decoy_project_components(&server, "BAR").await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // VP-COMPONENT-011: the bulk endpoint is never hit by the single-key path.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
            "--component",
            "remove:Frontend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-001: expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "AC-001: expected exactly 1 PUT to FOO-1; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "Backend"}},
                    {"remove": {"name": "Frontend"}}
                ]
            }
        }),
        "AC-001: PUT body must be the exact native update-verb shape"
    );
}

// ── AC-002 (BC-3.4.022 Edge Case EC-3.4.022-2 — bare treated as ADD) ───────

/// `--component Backend` (bare, no prefix) → `{"add":{"name":"Backend"}}`.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_bare_component_treated_as_add() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-002: expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "AC-002: expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "Backend"}}
                ]
            }
        }),
        "AC-002: bare --component must normalize to an ADD element, never a bare string"
    );
}

// ── AC-003 (BC-3.4.022 Postcondition 2 — ADD-before-REMOVE ordering) ──────

/// `--component remove:Gadget --component add:Widget` (remove specified
/// FIRST on the CLI) → the wire `components` array still emits the ADD
/// element (Widget) before the REMOVE element (Gadget), regardless of CLI
/// input order.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_add_precedes_remove_regardless_of_cli_order() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Widget", None, None, None),
            common::fixtures::component_response("10002", "Gadget", None, None, None),
        ],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Gadget",
            "--component",
            "add:Widget",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-003: expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "AC-003: expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "Widget"}},
                    {"remove": {"name": "Gadget"}}
                ]
            }
        }),
        "AC-003: ADD element must precede REMOVE element regardless of CLI order"
    );
}

// ── AC-004 (BC-3.4.022 Postcondition 3 — editmeta native path) ────────────

/// editmeta advertises `fields.components.operations` containing
/// `add`/`remove` → the native `update`-verb PUT fires directly, ZERO
/// extra `GET /rest/api/3/issue/{key}` for current components.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_editmeta_native_path() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // AC-004's core assertion: zero extra full-issue GET for the fallback's
    // current-components read — the native path never needs it.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::issue_response("FOO-1", "Test", "To Do")),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-004: expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "AC-004: expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "Backend"}}
                ]
            }
        })
    );
}

// ── AC-005 (BC-3.4.022 Postcondition 3 — read-modify-write fallback) ──────

/// editmeta does NOT advertise `add`/`remove` for `components` (only
/// `set`) → `jr` GETs current `fields.components`, computes the new full
/// array client-side (existing components + newly-resolved adds, existing
/// order preserved, new adds appended), and `PUT`s via the `set` verb
/// `{"fields":{"components":[...]}}`. MED-1 fix (Step-4.5 Round 4): a
/// RETAINED existing component is re-emitted by IDENTITY -- `{"id":...}`
/// when it has one (the normal case for a real Jira component) -- not by
/// bare name, since Jira allows multiple same-named components and a bare
/// name is ambiguous on the wire. The newly-added component ("Backend",
/// resolved from a NAME input) is unaffected and still wires as
/// `{"name":...}`.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_editmeta_fallback_read_modify_write() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    // editmeta advertises ONLY "set" — no add/remove — so the fallback path
    // must fire.
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    // MED-1: the existing retained component carries a concrete id
    // ("30001") so the assertion below can pin the exact identity-based
    // re-emission -- {"id":"30001"}, not the ambiguous {"name":"Existing"}.
    let mut issue_with_existing = common::fixtures::issue_response("FOO-1", "Test", "To Do");
    issue_with_existing["fields"]["components"] = serde_json::json!([
        {"name": "Existing", "id": "30001"}
    ]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_existing))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-005: expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "AC-005: expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"id": "30001"},
                    {"name": "Backend"}
                ]
            }
        }),
        "AC-005 (MED-1): fallback PUT must use the set-verb fields.components \
         shape, with the retained existing component re-emitted by IDENTITY \
         ({{\"id\":\"30001\"}}, not bare name) and the newly-resolved add \
         appended by its own resolved shape"
    );
}

// ── AC-006 (BC-3.4.022 Edge Case EC-3.4.022-3 — unknown name, zero PUT) ───

/// Unknown component name → exit 64 via §8.4 (BC-8.4.002), ZERO `PUT`
/// calls (the editmeta/list-components GET used for resolution is the
/// only HTTP that fires).
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_unknown_component_zero_put() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    // Mounted defensively — the resolution-vs-editmeta call ordering is not
    // pinned by the BC, so this must not become a spurious failure mode
    // regardless of which the implementation checks first.
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Nonexistent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-006: expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains(
            "Component 'Nonexistent' not found in project FOO. Available: Backend, Frontend."
        ),
        "AC-006: expected BC-8.4.002 canonical not-found message; stderr={stderr}"
    );
}

// ── AC-007 (BC-3.4.024 Postcondition 1 — create body composition) ─────────

/// `jr issue create --project FOO --component Backend --component Frontend`
/// → `fields.components = [{"name":"Backend"},{"name":"Frontend"}]`, CLI
/// input order. Also proves cross-project non-collision via a decoy "BAR"
/// project.
#[tokio::test]
async fn test_bc_3_4_024_issue_create_component_body_composition() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s605_1_mock_decoy_project_components(&server, "BAR").await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("FOO-1")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "FOO",
            "--type",
            "Task",
            "--summary",
            "Test create components",
            "--component",
            "Backend",
            "--component",
            "Frontend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-007: expected exit 0; stderr={stderr}"
    );

    let posts = s605_1_captured_posts(&server).await;
    assert_eq!(
        posts.len(),
        1,
        "AC-007: expected exactly 1 POST; got {posts:?}"
    );
    assert_eq!(
        posts[0]["fields"]["components"],
        serde_json::json!([{"name": "Backend"}, {"name": "Frontend"}]),
        "AC-007: fields.components must be object-with-name form, CLI input order"
    );
}

// ── AC-008 (BC-3.4.024 Edge Case EC-3.4.024-2 — no prefix interpretation) ─

/// `jr issue create --project FOO --component add:Backend` → the resolver
/// attempts to match a component LITERALLY named `"add:Backend"` →
/// unknown-name exit 64 (prefix grammar is `edit`-only).
#[tokio::test]
async fn test_bc_3_4_024_issue_create_component_no_prefix_interpretation() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "FOO",
            "--type",
            "Task",
            "--summary",
            "Test no prefix",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-008: expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains(
            "Component 'add:Backend' not found in project FOO. Available: Backend, Frontend."
        ),
        "AC-008: 'add:Backend' must be resolved LITERALLY (no prefix stripping on create); stderr={stderr}"
    );
}

// ── AC-009 (BC-3.4.024 Postcondition 3 / EC-3.4.024-3 — request-type guard) ─

/// `jr issue create --request-type "IT Request" --component Backend` →
/// exit 64, stderr names both `--component` and `--request-type`, ZERO
/// HTTP calls (no service-desk lookup, no RT-id resolution, no component
/// resolution).
#[tokio::test]
async fn test_bc_3_4_024_issue_create_component_request_type_guard_zero_http() {
    let server = MockServer::start().await;

    // Strongest zero-HTTP guarantee: catch-all .expect(0) for ANY request.
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "create",
            "--request-type",
            "IT Request",
            "--component",
            "Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-009: expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--component"),
        "AC-009: stderr must name --component; stderr={stderr}"
    );
    assert!(
        stderr.contains("--request-type"),
        "AC-009: stderr must name --request-type; stderr={stderr}"
    );
    // Step-4.5 Round 6 (INFO, Lens C): BC-3.4.024's Behavior Summary
    // requires the guard to "suggest a follow-up `jr issue edit
    // --component`" (SOUL: "always suggest what to do next") -- pin the
    // exact suggestion substring the code emits, not just that the two
    // flags are named.
    assert!(
        stderr.contains("`jr issue edit --component`"),
        "AC-009: stderr must suggest the follow-up `jr issue edit \
         --component` path per BC-3.4.024's Behavior Summary; stderr={stderr}"
    );
}

// ── AC-010 (BC-3.4.025 Invariant 1 — one GET, not duplicated) ─────────────

/// Within one `issue create --component X --component Y` invocation, the
/// project component-list GET fires EXACTLY once — never duplicated (not
/// once per `--component` value, and never duplicated with an editmeta
/// GET — `create` never consults editmeta at all).
#[tokio::test]
async fn test_bc_3_4_025_issue_create_component_resolution_one_get_not_duplicated() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::component_list_response(vec![
                common::fixtures::component_response("10001", "Backend", None, None, None),
                common::fixtures::component_response("10002", "Frontend", None, None, None),
            ]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    // Step-4.5 Round 2 LOW-1: the prior decoy here mounted
    // `GET /rest/api/3/issue/createmeta/FOO/issuetypes/components` with
    // `.expect(0)` to "prove create never consults editmeta" -- but that path
    // is not the real editmeta shape (`GET /rest/api/3/issue/{key}/editmeta`)
    // and no create code path could ever construct or call it (create has no
    // issue key before the POST succeeds), so the guard passed vacuously
    // under every implementation. There is no real editmeta-shaped path to
    // repoint the decoy at (no key exists pre-POST), so it is deleted rather
    // than fabricated at a fake path -- the load-bearing `.expect(1)` on the
    // component-list GET above is what actually proves the resolution
    // mechanism (BC-3.4.025), and `.expect(1)` on the POST below is the only
    // other HTTP call this invocation may make.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("FOO-1")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "FOO",
            "--type",
            "Task",
            "--summary",
            "Test one GET",
            "--component",
            "Backend",
            "--component",
            "Frontend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-010: expected exit 0; stderr={stderr}"
    );
    // `.expect(1)` on the component-list GET mock verifies the exactly-once
    // invariant on server drop.
}

// ── AC-011 (BC-3.4.012 amendment — table echo, prefixed) ──────────────────

/// Single-key success (table mode) with `--component add:X --component
/// remove:Y` → stderr contains `  components → add:X, remove:Y`.
#[tokio::test]
async fn test_bc_3_4_012_issue_edit_component_table_echo_prefixed() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
            "--component",
            "remove:Frontend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-011: expected exit 0; stderr={stderr}"
    );
    assert!(
        stderr.contains("  components \u{2192} add:Backend, remove:Frontend"),
        "AC-011: expected the components echo line verbatim; stderr={stderr}"
    );
}

// ── AC-012 (BC-3.4.012 EC-3.4.012-17 — bare normalization) ────────────────

/// `--component Backend` (bare) → stderr echo is `  components →
/// add:Backend`, NOT `  components → Backend`.
#[tokio::test]
async fn test_bc_3_4_012_issue_edit_component_table_echo_bare_normalized_to_add() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-012: expected exit 0; stderr={stderr}"
    );
    assert!(
        stderr.contains("  components \u{2192} add:Backend"),
        "AC-012: bare input must normalize to 'add:Backend' in the echo; stderr={stderr}"
    );
    assert!(
        !stderr.contains("  components \u{2192} Backend\n")
            && !stderr.trim_end().ends_with("components \u{2192} Backend"),
        "AC-012: echo must never render the bare, unprefixed name; stderr={stderr}"
    );
}

// ── AC-013 (BC-3.4.013 amendment / EC-3.4.013-14 — JSON echo) ─────────────

/// `--component Backend --output json` (bare) →
/// `changed_fields["components"] == "add:Backend"` — a STRING, not a JSON
/// array.
#[tokio::test]
async fn test_bc_3_4_013_issue_edit_component_json_echo_normalized_string() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "Backend",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-013: expected exit 0; stderr={stderr}"
    );

    let body: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("AC-013: stdout is not valid JSON: {e}; stdout={stdout}"));
    assert_eq!(
        body["changed_fields"]["components"],
        serde_json::json!("add:Backend"),
        "AC-013: changed_fields.components must be the normalized string \
         'add:Backend', not an array; body={body}"
    );
    assert!(
        body["changed_fields"]["components"].is_string(),
        "AC-013: changed_fields.components must be a JSON string; body={body}"
    );
}

// ── AC-014 (BC-3.4.017 amendment / EC-3.4.017-15 — Gate B fifth field) ────

/// `jr issue edit KEY1 KEY2 --component add:X --field components=Y` →
/// exit 64 (Gate B fires for `components`, the fifth field), no HTTP.
/// `--field Components=Y` (capitalized) triggers the same guard.
///
/// C-MED-1 fix (Step-4.5 Round 5): this 2-KEY input ALSO trips the C-1
/// multi-key rejection (edit.rs, "Multi-key bulk edit doesn't yet
/// support: --component") with the identical observable (exit 64, zero
/// HTTP) -- so exit-code-only assertions here cannot distinguish Gate B
/// from C-1; deleting Gate B's `components` clause would leave this test
/// green (C-1 alone would still catch this 2-key input). Asserting the
/// VERBATIM Gate B message closes that gap for the 2-key case; the
/// genuinely Gate-B-only scenario (single key, where C-1 never fires) is
/// covered separately by
/// `test_bc_3_4_017_issue_edit_single_key_component_field_overlap_gate_b`
/// below.
#[tokio::test]
async fn test_bc_3_4_017_issue_edit_bulk_component_field_overlap_gate_b() {
    let server = MockServer::start().await;

    // Strongest zero-HTTP guarantee, shared across both invocations below.
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let lower = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "KEY-1",
            "KEY-2",
            "--component",
            "add:X",
            "--field",
            "components=Y",
        ])
        .output()
        .unwrap();
    let lower_stderr = String::from_utf8_lossy(&lower.stderr);
    assert_eq!(
        lower.status.code(),
        Some(64),
        "AC-014 (lowercase 'components'): expected exit 64; stderr={lower_stderr}"
    );
    assert!(
        lower_stderr.contains("components is set by both --component and --field; use only one."),
        "C-MED-1: expected the verbatim Gate B message (distinguishing it \
         from the C-1 multi-key rejection, which would also exit 64 on \
         this same 2-key input); stderr={lower_stderr}"
    );

    let capitalized = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "KEY-1",
            "KEY-2",
            "--component",
            "add:X",
            "--field",
            "Components=Y",
        ])
        .output()
        .unwrap();
    let cap_stderr = String::from_utf8_lossy(&capitalized.stderr);
    assert_eq!(
        capitalized.status.code(),
        Some(64),
        "AC-014 (capitalized 'Components'): expected exit 64; stderr={cap_stderr}"
    );
    assert!(
        cap_stderr.contains("components is set by both --component and --field; use only one."),
        "C-MED-1: expected the verbatim Gate B message for the capitalized \
         'Components' variant too; stderr={cap_stderr}"
    );
    // `.expect(0)` on the catch-all mock verifies zero HTTP across BOTH
    // invocations cumulatively on server drop.
}

// ── C-MED-1 — Gate B's genuinely-untested scenario: SINGLE-KEY overlap ───

/// `jr issue edit FOO-1 --component add:X --field components=Y` (SINGLE
/// key) → exit 64, the verbatim Gate B message, ZERO HTTP. Single-key
/// input never reaches the C-1 multi-key rejection block, so Gate B is
/// the ONLY guard preventing a --component/--field-components double-write
/// here -- this is the scenario AC-014 (2-key input) cannot exercise.
#[tokio::test]
async fn test_bc_3_4_017_issue_edit_single_key_component_field_overlap_gate_b() {
    let server = MockServer::start().await;

    // Strongest zero-HTTP guarantee.
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:X",
            "--field",
            "components=Y",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "C-MED-1 (single-key): expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("components is set by both --component and --field; use only one."),
        "C-MED-1 (single-key): expected the verbatim Gate B message; stderr={stderr}"
    );
    // `.expect(0)` on the catch-all mock verifies zero HTTP.
}

// ── AC-015 (BC-3.4.020 amendment — label/component mutual exclusion) ──────

/// `jr issue edit KEY --label add:foo --component add:bar` (single key) →
/// exit 64, stderr contains both `"--label cannot be combined with"` and
/// `"--component"` as separate substrings; NEITHER the label-bulk path
/// nor the component wire path fires — zero HTTP (VP-COMPONENT-027).
#[tokio::test]
async fn test_bc_3_4_020_issue_edit_label_component_mutual_exclusion_zero_http() {
    let server = MockServer::start().await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--label",
            "add:foo",
            "--component",
            "add:bar",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-015: expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("--label cannot be combined with"),
        "AC-015: expected the canonical conflict-block message; stderr={stderr}"
    );
    assert!(
        stderr.contains("--component"),
        "AC-015: expected --component named in the conflict message; stderr={stderr}"
    );
}

// ── AC-016 (BC-3.4.021 amendment — dry-run JSON + table) ───────────────────

/// `--dry-run --output json FOO-1 --component add:X --component remove:Y`
/// → `plannedChanges.components == [{"action":"ADD","name":"X"},
/// {"action":"REMOVE","name":"Y"}]`; table mode →
/// `"  components → add:X, remove:Y"`; ZERO `PUT`/editmeta-fallback `GET`
/// calls (VP-COMPONENT-028).
///
/// Step-4.5 Round 1 F1 fix: component NAME resolution (BC-8.4) DOES fire
/// during dry-run (EC-3.4.021-20 -- it's a read-only GET) -- each of the two
/// invocations below uses its OWN `MockServer` so the resolution GET can be
/// pinned to EXACTLY `.expect(1)` per invocation, rather than tolerating an
/// unbounded/absent call as the pre-F1 version of this test did.
#[tokio::test]
async fn test_bc_3_4_021_issue_edit_component_dry_run_json_and_table_zero_mutation() {
    async fn mount_resolution_and_zero_mutation_guards(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project/FOO/components"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                common::fixtures::component_list_response(vec![
                    common::fixtures::component_response("10001", "X", None, None, None),
                    common::fixtures::component_response("10002", "Y", None, None, None),
                ]),
            ))
            .expect(1)
            .mount(server)
            .await;

        // Zero-mutation guarantee: no PUT of any kind may occur under --dry-run.
        Mock::given(method("PUT"))
            .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
            .expect(0)
            .mount(server)
            .await;
    }

    let json_server = MockServer::start().await;
    mount_resolution_and_zero_mutation_guards(&json_server).await;

    let json_out = s605_1_cmd(&json_server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:X",
            "--component",
            "remove:Y",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    let json_stderr = String::from_utf8_lossy(&json_out.stderr);
    let json_stdout = String::from_utf8_lossy(&json_out.stdout);
    assert!(
        json_out.status.success(),
        "AC-016 (json): expected exit 0; stderr={json_stderr}"
    );
    let body: serde_json::Value = serde_json::from_str(&json_stdout).unwrap_or_else(|e| {
        panic!("AC-016 (json): stdout is not valid JSON: {e}; stdout={json_stdout}")
    });
    assert_eq!(
        body["plannedChanges"]["components"],
        serde_json::json!([
            {"action": "ADD", "name": "X"},
            {"action": "REMOVE", "name": "Y"}
        ]),
        "AC-016 (json): plannedChanges.components must be the structured \
         ADD/REMOVE array; body={body}"
    );

    let table_server = MockServer::start().await;
    mount_resolution_and_zero_mutation_guards(&table_server).await;

    let table_out = s605_1_cmd(&table_server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:X",
            "--component",
            "remove:Y",
            "--dry-run",
        ])
        .output()
        .unwrap();
    let table_stderr = String::from_utf8_lossy(&table_out.stderr);
    let table_stdout = String::from_utf8_lossy(&table_out.stdout);
    assert!(
        table_out.status.success(),
        "AC-016 (table): expected exit 0; stderr={table_stderr}"
    );
    assert!(
        table_stdout.contains("  components \u{2192} add:X, remove:Y"),
        "AC-016 (table): expected the dry-run preview line verbatim; stdout={table_stdout}"
    );
    // `.expect(1)` on each server's resolution GET and `.expect(0)` on each
    // server's PUT catch-all verify both invariants per-invocation.
}

// ── AC-017 (BC-3.4.021 amendment — dry-run bare normalization parity) ─────

/// `jr issue edit FOO-1 --component X --dry-run` (bare) → table preview
/// renders `"  components → add:X"` — IDENTICAL normalization to the
/// live-edit echo (AC-012), not `"  components → X"`.
#[tokio::test]
async fn test_bc_3_4_021_issue_edit_component_dry_run_bare_normalization_matches_live() {
    let server = MockServer::start().await;

    // Step-4.5 Round 1 F1 fix: resolution now fires during dry-run --
    // pinned to exactly 1 call (one invocation in this test).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::component_list_response(vec![common::fixtures::component_response(
                "10001", "X", None, None, None,
            )]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "X",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "AC-017: expected exit 0; stderr={stderr}"
    );
    assert!(
        stdout.contains("  components \u{2192} add:X"),
        "AC-017: bare --component must normalize to 'add:X' in the dry-run \
         table preview, matching the live-edit echo; stdout={stdout}"
    );
    assert!(
        !stdout.contains("  components \u{2192} X\n")
            && !stdout.trim_end().ends_with("components \u{2192} X"),
        "AC-017: dry-run preview must never render the bare, unprefixed name; stdout={stdout}"
    );
}

// =============================================================================
// Step-4.5 Round 1 (F1/F2) — additional tests, adjudicated against verbatim
// BC text
// =============================================================================

// ── F1 (BC-3.4.021 EC-3.4.021-20) — dry-run unresolvable name exits 64 ────

/// `jr issue edit FOO-1 --component add:Nonexistent --dry-run` → exit 64
/// via the same BC-8.4.002 canonical not-found message the live path emits
/// (AC-006), BEFORE any `plannedChanges` output. The resolution GET still
/// fires (it's read-only); the mutating PUT never does.
#[tokio::test]
async fn test_bc_3_4_021_issue_edit_component_dry_run_unknown_name_exits_64() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::component_list_response(vec![common::fixtures::component_response(
                "10001", "Backend", None, None, None,
            )]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Nonexistent",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F1: dry-run with an unresolvable component name must exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("Component 'Nonexistent' not found in project FOO. Available: Backend."),
        "F1: expected the canonical BC-8.4.002 not-found message; stderr={stderr}"
    );
    assert!(
        stdout.is_empty(),
        "F1: stdout must be EMPTY on this exit-64 -- zero plannedChanges output \
         (BC-3.4.021 EC-3.4.021-20); stdout={stdout}"
    );
}

// ── F2 (BC-3.4.012/013/021 amendments) — echo/preview in CLI input order ──

/// `--component remove:Y --component add:X` (remove specified FIRST on the
/// CLI) → the table echo renders `remove:Y, add:X` -- CLI input order, NOT
/// reordered to `add:X, remove:Y` -- while the live PUT wire body still
/// emits ADD-before-REMOVE regardless of CLI order (AC-003 precedent).
/// Mirrors labels' EC-3.4.020-8: only the wire reorders, never the echo.
#[tokio::test]
async fn test_bc_3_4_012_issue_edit_component_echo_preserves_cli_input_order() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "X", None, None, None),
            common::fixtures::component_response("10002", "Y", None, None, None),
        ],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Y",
            "--component",
            "add:X",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "F2: expected exit 0; stderr={stderr}"
    );
    assert!(
        stderr.contains("  components \u{2192} remove:Y, add:X"),
        "F2: table echo must preserve CLI input order (remove specified \
         first) -- NOT reordered to add:X, remove:Y; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(puts.len(), 1, "F2: expected exactly 1 PUT; got {puts:?}");
    assert_eq!(
        puts[0],
        serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "X"}},
                    {"remove": {"name": "Y"}}
                ]
            }
        }),
        "F2: the live PUT wire body must stay ADD-before-REMOVE regardless \
         of CLI input order -- only the echo/preview render in CLI order"
    );
}

/// `--component remove:Y --component add:X --output json` (remove specified
/// FIRST on the CLI) → `changed_fields["components"] == "remove:Y, add:X"`
/// -- CLI input order, same as the table echo (BC-3.4.013 amendment:
/// "identical format and CLI-input-order semantics to the table-mode echo").
#[tokio::test]
async fn test_bc_3_4_013_issue_edit_component_json_echo_preserves_cli_input_order() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "X", None, None, None),
            common::fixtures::component_response("10002", "Y", None, None, None),
        ],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Y",
            "--component",
            "add:X",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "F2: expected exit 0; stderr={stderr}"
    );
    let body: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("F2: stdout is not valid JSON: {e}; stdout={stdout}"));
    assert_eq!(
        body["changed_fields"]["components"],
        serde_json::json!("remove:Y, add:X"),
        "F2: changed_fields.components must preserve CLI input order \
         (remove specified first) -- NOT reordered to add:X, remove:Y; body={body}"
    );
}

/// `--dry-run --component remove:Y --component add:X` (remove specified
/// FIRST on the CLI) → `plannedChanges.components ==
/// [{"action":"REMOVE","name":"Y"},{"action":"ADD","name":"X"}]` and the
/// table preview renders `remove:Y, add:X` -- both in CLI input order,
/// identical to the live echo (F2), not reordered ADD-before-REMOVE.
#[tokio::test]
async fn test_bc_3_4_021_issue_edit_component_dry_run_preserves_cli_input_order() {
    async fn mount_resolution_and_zero_mutation_guards(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/rest/api/3/project/FOO/components"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                common::fixtures::component_list_response(vec![
                    common::fixtures::component_response("10001", "X", None, None, None),
                    common::fixtures::component_response("10002", "Y", None, None, None),
                ]),
            ))
            .expect(1)
            .mount(server)
            .await;
        Mock::given(method("PUT"))
            .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
            .expect(0)
            .mount(server)
            .await;
    }

    let json_server = MockServer::start().await;
    mount_resolution_and_zero_mutation_guards(&json_server).await;

    let json_out = s605_1_cmd(&json_server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Y",
            "--component",
            "add:X",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    let json_stderr = String::from_utf8_lossy(&json_out.stderr);
    let json_stdout = String::from_utf8_lossy(&json_out.stdout);
    assert!(
        json_out.status.success(),
        "F2 (json): expected exit 0; stderr={json_stderr}"
    );
    let body: serde_json::Value = serde_json::from_str(&json_stdout).unwrap_or_else(|e| {
        panic!("F2 (json): stdout is not valid JSON: {e}; stdout={json_stdout}")
    });
    assert_eq!(
        body["plannedChanges"]["components"],
        serde_json::json!([
            {"action": "REMOVE", "name": "Y"},
            {"action": "ADD", "name": "X"}
        ]),
        "F2 (json): plannedChanges.components must preserve CLI input order \
         (remove specified first); body={body}"
    );

    let table_server = MockServer::start().await;
    mount_resolution_and_zero_mutation_guards(&table_server).await;

    let table_out = s605_1_cmd(&table_server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Y",
            "--component",
            "add:X",
            "--dry-run",
        ])
        .output()
        .unwrap();
    let table_stderr = String::from_utf8_lossy(&table_out.stderr);
    let table_stdout = String::from_utf8_lossy(&table_out.stdout);
    assert!(
        table_out.status.success(),
        "F2 (table): expected exit 0; stderr={table_stderr}"
    );
    assert!(
        table_stdout.contains("  components \u{2192} remove:Y, add:X"),
        "F2 (table): expected the dry-run preview line in CLI input order; \
         stdout={table_stdout}"
    );
}

// =============================================================================
// Step-4.5 Round 2 — additional coverage: RMW fallback REMOVE branch (MEDIUM-1)
// and resolver ExactMultiple/Ambiguous error branches on the edit AND create
// paths (MEDIUM-2)
// =============================================================================

// ── MEDIUM-1 (BC-3.4.022 Postcondition 3) — RMW fallback REMOVE branch ────

/// editmeta advertises ONLY `set` (no add/remove) → the read-modify-write
/// fallback fires. `--component add:X --component remove:Y`, where the
/// issue's CURRENT `fields.components` already contains `Y` and an
/// untouched `Z`: the computed `set`-verb array must retain `Z`, drop `Y`,
/// and append `X` -- `Z` first (existing, order-preserved), `X` second
/// (newly resolved add). Prior to this test, the `current.retain(|name|
/// !removes.contains(name))` removal computation (edit.rs) was exercised
/// ONLY by AC-005, which passes solely `--component add:Backend` -- zero
/// coverage on the removal path itself. A mutation inverting or deleting
/// that `retain` predicate would have stayed green (a real data-loss risk
/// on a fallback PUT that silently keeps a component the user asked to
/// remove).
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_fallback_remove_computes_correct_set_array() {
    let server = MockServer::start().await;

    // Project component list: X and Y must resolve (they're the CLI
    // --component inputs); Z is pre-existing on the issue and is never
    // resolved -- it must not need to appear in this list.
    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "X", None, None, None),
            common::fixtures::component_response("10002", "Y", None, None, None),
        ],
    )
    .await;
    // editmeta advertises ONLY "set" -- no add/remove -- so the fallback
    // path must fire (same gate condition as AC-005).
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_components("FOO-1", "Test", &["Z", "Y"]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:X",
            "--component",
            "remove:Y",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "MEDIUM-1: expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "MEDIUM-1: expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"name": "Z"},
                    {"name": "X"}
                ]
            }
        }),
        "MEDIUM-1: fallback set-verb PUT must retain the untouched \
         pre-existing component (Z), drop the removed one (Y), and append \
         the newly-resolved add (X) -- in that order"
    );
}

// ── MEDIUM-2 (BC-8.4.003) — ExactMultiple / Ambiguous on edit AND create ──

/// `issue edit`: a component-list GET returning two components whose names
/// are identical case-insensitively ("Backend" id 10001, "backend" id
/// 10002) → `MatchResult::ExactMultiple` → exit 64, zero PUT, exact
/// BC-8.4.003 message (pinned verbatim, including the LIST-ORDER id list --
/// mirrors `component.rs`'s `test_bc_x_10_003_component_edit_exact_multiple_fails_closed`
/// precedent, applied to the issue-edit copy of the resolver call).
#[tokio::test]
async fn test_bc_8_4_003_issue_edit_component_exact_multiple_exits_64() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "backend", None, None, None),
        ],
    )
    .await;
    // Mounted defensively -- the resolution-vs-editmeta call ordering is
    // not pinned by the BC (mirrors AC-006's precedent).
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "MEDIUM-2 (edit ExactMultiple): expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains(
            "Multiple components named \"Backend\" found (IDs: 10001, 10002). \
             Pass the numeric ID directly."
        ),
        "MEDIUM-2 (edit ExactMultiple): expected exact BC-8.4.003 message; \
         stderr={stderr}"
    );
}

/// `issue edit`: `--component add:Amb` where the project component list
/// has two partial matches ("Ambition" id 20002, "Amber" id 20001, mounted
/// in reverse-alphabetical fixture order so the assertion can only pass if
/// the implementation actually sorts the candidates) → `MatchResult::Ambiguous`
/// → exit 64, zero PUT, exact BC-8.4.003 message (mirrors the
/// `issue list --component` precedent, `test_bc_2_1_022_issue_list_component_ambiguous_name_zero_search`,
/// applied to the issue-edit copy of the resolver call).
#[tokio::test]
async fn test_bc_8_4_003_issue_edit_component_ambiguous_exits_64() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("20002", "Ambition", None, None, None),
            common::fixtures::component_response("20001", "Amber", None, None, None),
        ],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Amb",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "MEDIUM-2 (edit Ambiguous): expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("Ambiguous component 'Amb'. Matches: Amber, Ambition."),
        "MEDIUM-2 (edit Ambiguous): expected exact BC-8.4.003 message \
         (alphabetically-sorted Matches list); stderr={stderr}"
    );
}

/// `issue create`: same case-only-duplicate fixture as the edit test above,
/// via `resolve_create_components` (create.rs's own copy of the resolver
/// call) → exit 64, zero POST, exact BC-8.4.003 message.
#[tokio::test]
async fn test_bc_8_4_003_issue_create_component_exact_multiple_exits_64() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "backend", None, None, None),
        ],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "FOO",
            "--type",
            "Task",
            "--summary",
            "Test exact multiple",
            "--component",
            "Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "MEDIUM-2 (create ExactMultiple): expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains(
            "Multiple components named \"Backend\" found (IDs: 10001, 10002). \
             Pass the numeric ID directly."
        ),
        "MEDIUM-2 (create ExactMultiple): expected exact BC-8.4.003 message; \
         stderr={stderr}"
    );
}

/// `issue create`: same reverse-alphabetical-fixture partial-match setup as
/// the edit Ambiguous test above, via `resolve_create_components` → exit
/// 64, zero POST, exact BC-8.4.003 message.
#[tokio::test]
async fn test_bc_8_4_003_issue_create_component_ambiguous_exits_64() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("20002", "Ambition", None, None, None),
            common::fixtures::component_response("20001", "Amber", None, None, None),
        ],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "FOO",
            "--type",
            "Task",
            "--summary",
            "Test ambiguous",
            "--component",
            "Amb",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "MEDIUM-2 (create Ambiguous): expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("Ambiguous component 'Amb'. Matches: Amber, Ambition."),
        "MEDIUM-2 (create Ambiguous): expected exact BC-8.4.003 message \
         (alphabetically-sorted Matches list); stderr={stderr}"
    );
}

// =============================================================================
// Step-4.5 Round 3 — F1: numeric --component input wires as {"id":...}, not
// {"name":...} (BC-8.4.001 numeric bypass / BC-8.1.008); F2: --field
// validation must run before the --component mutation (partial-write fix)
// =============================================================================

// ── F1 — numeric --component on `issue create` wires as {"id":...} ───────

/// `jr issue create --project FOO --component 10001` (numeric, no prefix
/// grammar on create) → `fields.components == [{"id":"10001"}]` -- NOT
/// `{"name":"10001"}`. BC-8.4.001's numeric bypass means all-ASCII-digit
/// input is always a component id (BC-8.1.008), and Jira's issue
/// components field accepts `{"id":...}`.
#[tokio::test]
async fn test_bc_3_4_024_issue_create_component_numeric_wires_as_id() {
    let server = MockServer::start().await;

    // The project component-list GET still fires (BC-3.4.025's resolution
    // mechanism runs unconditionally), but its contents are irrelevant to a
    // numeric value -- BC-8.4.001's bypass short-circuits before any
    // candidate-list matching.
    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "99999",
            "Unrelated",
            None,
            None,
            None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(common::fixtures::create_issue_response("FOO-1")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "FOO",
            "--type",
            "Task",
            "--summary",
            "Test numeric component id",
            "--component",
            "10001",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "F1 (create numeric): expected exit 0; stderr={stderr}"
    );

    let posts = s605_1_captured_posts(&server).await;
    assert_eq!(
        posts.len(),
        1,
        "F1 (create numeric): expected exactly 1 POST; got {posts:?}"
    );
    assert_eq!(
        posts[0]["fields"]["components"],
        serde_json::json!([{"id": "10001"}]),
        "F1 (create numeric): a numeric --component value must wire as \
         {{\"id\":...}}, never {{\"name\":...}}"
    );
}

// ── F1 — numeric --component on `issue edit` (single-key, native path) ───

/// `jr issue edit FOO-1 --component add:10001 --component remove:20002`
/// (both numeric) → `PUT /rest/api/3/issue/FOO-1` body
/// `{"update":{"components":[{"add":{"id":"10001"}},{"remove":{"id":"20002"}}]}}`
/// -- id-keyed objects, not name-keyed. The wire's ADD-before-REMOVE
/// ordering (AC-003) is unaffected by the id-vs-name discriminator.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_numeric_wires_as_id_native_path() {
    let server = MockServer::start().await;

    // As above: the component-list GET fires but its contents are
    // irrelevant to numeric inputs.
    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "99999",
            "Unrelated",
            None,
            None,
            None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:10001",
            "--component",
            "remove:20002",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "F1 (edit numeric native): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "F1 (edit numeric native): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "update": {
                "components": [
                    {"add": {"id": "10001"}},
                    {"remove": {"id": "20002"}}
                ]
            }
        }),
        "F1 (edit numeric native): numeric add/remove targets must wire as \
         {{\"id\":...}}, never {{\"name\":...}}"
    );
}

// ── F1 — numeric --component remove on the RMW fallback path ─────────────

/// editmeta advertises ONLY `set` (fallback fires). The issue's CURRENT
/// `fields.components` has an id-bearing component (id `"20002"`) and an
/// untouched other component. `--component remove:20002` (numeric) → the
/// computed set-verb array drops the id-MATCHED component (not a name
/// match) and keeps the untouched one, re-emitted by its own id (MED-1
/// fix, Step-4.5 Round 4) rather than an ambiguous bare name.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_numeric_remove_fallback_matches_by_id() {
    let server = MockServer::start().await;

    // Unused for a numeric input's resolution, but resolve_component_change_names
    // fetches it unconditionally.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::component_list_response(vec![])),
        )
        .mount(&server)
        .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    let mut issue_with_ids = common::fixtures::issue_response("FOO-1", "Test", "To Do");
    issue_with_ids["fields"]["components"] = serde_json::json!([
        {"name": "Untouched", "id": "30001"},
        {"name": "ToRemove", "id": "20002"}
    ]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_ids))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:20002",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "F1 (fallback numeric remove): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "F1 (fallback numeric remove): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"id": "30001"}
                ]
            }
        }),
        "F1/MED-1 (fallback numeric remove): the id-matched component (id \
         \"20002\") must be dropped by ID match, not name match; the \
         untouched component must be retained and re-emitted by IDENTITY \
         ({{\"id\":\"30001\"}}, MED-1 fix -- not the ambiguous bare name)"
    );
}

// ── F2 — --field validation must run before the --component mutation ────

/// `jr issue edit FOO-1 --component add:X --field bogusfield=Y` (single
/// key, unresolvable --field name) → exit 64, ZERO component mutation.
/// Before the F2 fix, `edit_issue_components` fired its PUT BEFORE
/// `resolve_edit_fields` validated `--field`, so an invalid `--field`
/// landed the component change, THEN exited 64 -- a partial write.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_field_validated_before_component_put() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // --field resolution's fields-list GET returns no field matching
    // "bogusfield" -- resolution fails BEFORE editmeta, BEFORE any
    // --component HTTP call (mirrors tests/issue_edit_field.rs's
    // test_bc_3_4_017_ec_11_field_type_key_not_rejected_by_gate_b pattern:
    // "No editmeta, no PUT — resolution fails before reaching editmeta").
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "customfield_10001", "name": "Severity", "custom": true }
        ])))
        .mount(&server)
        .await;

    // F2's core assertion: edit_issue_components must never even be
    // entered -- its first HTTP call would be this component-list GET.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    // Zero mutation guarantee: no PUT of any kind (component's native/
    // fallback PUT, or the generic client.edit_issue PUT) may occur.
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:X",
            "--field",
            "bogusfield=Y",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F2: expected exit 64 for an unresolvable --field; stderr={stderr}"
    );
    // `.expect(0)` on both the component-list GET and the PUT catch-all
    // verify the component mutation never started -- the invalid --field
    // is caught first, so there is no partial write.
}

// =============================================================================
// Step-4.5 Round 4 — MED-1: RMW fallback re-emits RETAINED components by
// IDENTITY (id when present), not by ambiguous bare name, on an issue
// carrying multiple same-named components
// =============================================================================

/// editmeta advertises ONLY `set` (fallback fires). The issue's CURRENT
/// `fields.components` has TWO same-named components: "Backend" id
/// `"10001"` and "Backend" id `"10002"` (Jira allows this -- the entire
/// reason F1 added numeric-id targeting).
///
/// (a) `--component remove:10001` (numeric) → the set-verb array retains
///     ONLY id `"10002"`, re-emitted by its own id -- the survivor is
///     unambiguous, the removed one is gone.
/// (b) `--component add:Frontend` (no remove) → BOTH Backends survive and
///     must be re-emitted by id, never by the ambiguous bare name
///     "Backend" twice (which Jira would silently dedupe to one --
///     component "10002" silently dropped from the issue, exit 0 -- the
///     data-loss bug MED-1 closes), plus the newly-resolved Frontend.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_retains_duplicate_named_by_id() {
    let mut issue_dup = common::fixtures::issue_response("FOO-1", "Test", "To Do");
    issue_dup["fields"]["components"] = serde_json::json!([
        {"name": "Backend", "id": "10001"},
        {"name": "Backend", "id": "10002"}
    ]);

    // --- (a) remove:10001 -> only id 10002 survives, emitted by id. ---
    let remove_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::component_list_response(vec![])),
        )
        .mount(&remove_server)
        .await;
    s605_1_mock_editmeta(&remove_server, "FOO-1", &["set"]).await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_dup.clone()))
        .expect(1)
        .mount(&remove_server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&remove_server)
        .await;

    let remove_output = s605_1_cmd(&remove_server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:10001",
        ])
        .output()
        .unwrap();
    let remove_stderr = String::from_utf8_lossy(&remove_output.stderr);
    assert!(
        remove_output.status.success(),
        "MED-1 (remove): expected exit 0; stderr={remove_stderr}"
    );
    let remove_puts = s605_1_captured_puts(&remove_server, "FOO-1").await;
    assert_eq!(
        remove_puts.len(),
        1,
        "MED-1 (remove): expected exactly 1 PUT; got {remove_puts:?}"
    );
    assert_eq!(
        remove_puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"id": "10002"}
                ]
            }
        }),
        "MED-1 (remove): only id 10002 must survive, re-emitted by id -- \
         a bare-name re-emission would be ambiguous with the removed \
         same-named component"
    );

    // --- (b) add:Frontend (no remove) -> both Backends survive, each
    // emitted by id (never by the ambiguous bare name "Backend" twice). ---
    let add_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/FOO/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::component_list_response(vec![common::fixtures::component_response(
                "40001", "Frontend", None, None, None,
            )]),
        ))
        .mount(&add_server)
        .await;
    s605_1_mock_editmeta(&add_server, "FOO-1", &["set"]).await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_dup))
        .expect(1)
        .mount(&add_server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&add_server)
        .await;

    let add_output = s605_1_cmd(&add_server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Frontend",
        ])
        .output()
        .unwrap();
    let add_stderr = String::from_utf8_lossy(&add_output.stderr);
    assert!(
        add_output.status.success(),
        "MED-1 (add): expected exit 0; stderr={add_stderr}"
    );
    let add_puts = s605_1_captured_puts(&add_server, "FOO-1").await;
    assert_eq!(
        add_puts.len(),
        1,
        "MED-1 (add): expected exactly 1 PUT; got {add_puts:?}"
    );
    assert_eq!(
        add_puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"id": "10001"},
                    {"id": "10002"},
                    {"name": "Frontend"}
                ]
            }
        }),
        "MED-1 (add): BOTH same-named Backends must survive, each \
         re-emitted by its own id -- two bare {{\"name\":\"Backend\"}} \
         entries would silently dedupe to one on Jira's side (data loss)"
    );
}

// =============================================================================
// Step-4.5 Round 5 — B-LOW-1: RMW fallback add-before-remove parity with the
// native path
// =============================================================================

/// RMW fallback (editmeta lacks add/remove) with `--component add:Backend
/// --component remove:Backend` (the SAME component both added and
/// removed) → the set-verb body must NOT contain Backend, matching the
/// native update-verb path's add-before-remove semantics (BC-3.4.022 Post
/// 2: Jira applies `[{"add":X},{"remove":X}]` in order, leaving X absent).
/// An untouched pre-existing component survives unaffected.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_add_remove_same_matches_native() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    let mut issue_with_other = common::fixtures::issue_response("FOO-1", "Test", "To Do");
    issue_with_other["fields"]["components"] = serde_json::json!([
        {"name": "Other", "id": "50001"}
    ]);
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_other))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
            "--component",
            "remove:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "B-LOW-1: expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "B-LOW-1: expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"id": "50001"}
                ]
            }
        }),
        "B-LOW-1: add:Backend + remove:Backend (same component) must leave \
         Backend ABSENT from the set-verb body, matching the native \
         add-before-remove wire semantics; the untouched 'Other' component \
         must survive"
    );
}

// =============================================================================
// Step-4.5 Round 5 — C-LOW-1: editmeta native-vs-fallback gate is
// mutation-survivable at the "add" ONLY (no "remove") boundary
// =============================================================================

/// editmeta advertises `add` ONLY (no `remove`) for `components` → the
/// RMW fallback fires (`GET` current components, `set`-verb PUT), NOT the
/// native update-verb path. Only the two extremes -- `["add","remove"]`
/// (AC-004, native) and `["set"]` (AC-005, fallback) -- were previously
/// tested; a `native_supported = ops.any(=="add") && ops.any(=="remove")`
/// -> `||` mutation would misclassify this `["add"]`-only case as
/// native-supported and survive both of those tests untouched. This test
/// pins the `&&`.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_editmeta_add_only_uses_fallback() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    // editmeta advertises "add" ONLY -- no "remove" -- must select the
    // fallback, not the native path.
    s605_1_mock_editmeta(&server, "FOO-1", &["add"]).await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_components("FOO-1", "Test", &[]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "C-LOW-1: expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "C-LOW-1: expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"name": "Backend"}
                ]
            }
        }),
        "C-LOW-1: editmeta advertising 'add' only (no 'remove') must use \
         the set-verb RMW fallback shape (fields.components), NOT the \
         native update-verb PUT shape (update.components) -- pins the && \
         (not ||) in the native_supported gate. The GET current-issue \
         call's .expect(1) above independently proves the fallback path \
         (not the native path, which never calls get_issue) actually ran."
    );
}

/// editmeta advertises `remove` ONLY (no `add`) for `components` → the RMW
/// fallback fires, NOT the native update-verb path. Mirror of the add-only
/// test above (Step-4.5 Round 6, LOW/Lens C): without this, a mutation
/// replacing the first conjunct of `native_supported = ops.any(=="add") &&
/// ops.any(=="remove")` with `true` would survive every other editmeta
/// test (add-only→fallback still holds via the second conjunct, set→
/// fallback still holds, add+remove→native still holds).
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_editmeta_remove_only_uses_fallback() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    // editmeta advertises "remove" ONLY -- no "add" -- must select the
    // fallback, not the native path.
    s605_1_mock_editmeta(&server, "FOO-1", &["remove"]).await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_components("FOO-1", "Test", &["Backend"]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "LOW (Lens C): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "LOW (Lens C): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": []
            }
        }),
        "LOW (Lens C): editmeta advertising 'remove' only (no 'add') must \
         use the set-verb RMW fallback shape (fields.components), NOT the \
         native update-verb PUT shape (update.components) -- pins the && \
         (not a first-conjunct-elided variant) in the native_supported \
         gate. The GET current-issue call's .expect(1) above independently \
         proves the fallback path actually ran."
    );
}

// =============================================================================
// Step-4.5 Round 6 — HIGH-1 (F-A-001): RMW fallback silently failed to
// remove a NAME-specified component against a live (id-bearing) issue
// component. A COMPLETE test matrix (a)-(f) below closes this definitively.
// =============================================================================

/// (a) THE BUG, RED-then-GREEN proof. RMW fallback (editmeta lacks
/// add/remove) on an issue whose current "Backend" component HAS an id
/// (`"10050"` -- the LIVE-Jira-correct shape) with `--component
/// remove:Backend` (a NAME remove target) → the set-verb body must NOT
/// contain Backend.
///
/// THE BUG (pre-fix): the RMW fallback mapped each existing component to a
/// SINGLE `ComponentRef` (`Id` when it has one, else `Name`), so an
/// id-bearing Backend became `ComponentRef::Id("10050")` — a NAME remove
/// target (`ComponentRef::Name("Backend")`) never matches an `Id`-variant
/// candidate under `ComponentRef`'s derived, variant-sensitive `PartialEq`,
/// so `retain` kept it: Backend was silently NOT removed, exit 0, a false
/// `components → remove:Backend` success echo — silent data loss on the
/// COMMON case (name remove + live Jira + RMW fallback).
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_name_remove_matches_id_bearing_existing() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10050", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    let issue_with_id_bearing = common::fixtures::issue_response_with_components_and_ids(
        "FOO-1",
        "Test",
        &[("Backend", "10050")],
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_id_bearing))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "HIGH-1 (a): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "HIGH-1 (a): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": []
            }
        }),
        "HIGH-1 (a): a NAME remove target must drop an id-bearing existing \
         component by matching NAME, not just id -- the set-verb body must \
         NOT contain Backend"
    );
}

/// (b) existing id-bearing "Backend"(10050) + `remove:10050` (numeric
/// remove target) → drops Backend. Regression guard: this must KEEP
/// passing under the HIGH-1 fix (it already passed under the buggy Round-5
/// code, since an id-remove against an id-bearing component happened to
/// still match).
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_numeric_remove_matches_id_bearing_existing() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10050", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    let issue_with_id_bearing = common::fixtures::issue_response_with_components_and_ids(
        "FOO-1",
        "Test",
        &[("Backend", "10050")],
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_id_bearing))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:10050",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "HIGH-1 (b): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "HIGH-1 (b): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": []
            }
        }),
        "HIGH-1 (b): a numeric remove target must drop the id-matched \
         existing component"
    );
}

/// (c) existing id=None "Legacy" (the `issue_response_with_components`
/// minimal-fixture shape) + `remove:Legacy` (name remove) → drops Legacy.
/// Regression guard preserving the MED-1/Round-4 id=None coverage: adding
/// `issue_response_with_components_and_ids` for HIGH-1 must NOT retire the
/// id=None code path.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_name_remove_matches_id_absent_existing() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10070", "Legacy", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_components("FOO-1", "Test", &["Legacy"]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Legacy",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "HIGH-1 (c): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "HIGH-1 (c): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": []
            }
        }),
        "HIGH-1 (c): a name remove target must drop an id=None existing \
         component matching by name"
    );
}

/// (d) existing id-bearing "Backend"(10050) + `add:Backend --component
/// remove:Backend` (name add==remove, targeting a component that ALSO
/// already exists on the issue with an id) → Backend ABSENT (net), matching
/// the native path's add-before-remove semantics (B-LOW-1 parity). Distinct
/// from the Round-5 B-LOW-1 test, whose existing-components fixture did NOT
/// contain a pre-existing Backend at all -- this proves the pre-existing,
/// id-bearing instance is ALSO removed, not just the would-be add
/// short-circuited.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_add_remove_same_id_bearing_existing() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10050", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    let issue_with_id_bearing = common::fixtures::issue_response_with_components_and_ids(
        "FOO-1",
        "Test",
        &[("Backend", "10050")],
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_id_bearing))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
            "--component",
            "remove:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "HIGH-1 (d): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "HIGH-1 (d): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": []
            }
        }),
        "HIGH-1 (d): add:Backend + remove:Backend against a pre-existing, \
         id-bearing Backend must leave the set NET ABSENT -- matching the \
         native path's add-before-remove semantics"
    );
}

/// (e) existing id-bearing Backend(10050) AND Frontend(10060) + BOTH
/// `remove:Backend` and `remove:Frontend` (two disjoint name removes) →
/// both dropped, matched by name against their own id-bearing entries.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_disjoint_name_removes_id_bearing_existing() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10050", "Backend", None, None, None),
            common::fixtures::component_response("10060", "Frontend", None, None, None),
        ],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    let issue_with_id_bearing = common::fixtures::issue_response_with_components_and_ids(
        "FOO-1",
        "Test",
        &[("Backend", "10050"), ("Frontend", "10060")],
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_id_bearing))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Backend",
            "--component",
            "remove:Frontend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "HIGH-1 (e): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "HIGH-1 (e): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": []
            }
        }),
        "HIGH-1 (e): two disjoint name-remove targets must each drop their \
         own id-bearing existing component"
    );
}

/// (f) untouched components (not targeted by any remove) always survive,
/// re-emitted by identity (`{"id":...}`) -- an id-bearing "Other" component
/// survives a `remove:Backend` that targets a DIFFERENT, disjoint
/// component.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_untouched_survives_by_identity() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10050", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    let issue_with_id_bearing = common::fixtures::issue_response_with_components_and_ids(
        "FOO-1",
        "Test",
        &[("Backend", "10050"), ("Other", "30001")],
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_id_bearing))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "HIGH-1 (f): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "HIGH-1 (f): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"id": "30001"}
                ]
            }
        }),
        "HIGH-1 (f): the untouched 'Other' component must survive, \
         re-emitted by its own identity"
    );
}

// =============================================================================
// Step-4.5 Round 7 — MEDIUM-1: merge the single-key --component edit into
// ONE PUT with other field changes (close the two-PUT partial-write window)
// =============================================================================

/// THE KEY RED-then-GREEN TEST. `--component add:Backend --priority Bogus`
/// on the NATIVE update-verb path (editmeta advertises add+remove), where
/// Jira rejects the invalid priority with 400: exactly ONE PUT must fire,
/// carrying BOTH `update.components` and `fields.priority` in the SAME
/// request body -- so the whole edit (including the component change) is
/// rejected together, never landing a partial write.
///
/// This test was run against the PRE-FIX (two-PUT) code first and
/// confirmed RED: the component-only PUT (`{"update":{"components":[...]}}`)
/// succeeded (204) -- the component change LANDED -- and only the
/// SEPARATE, SECOND field-only PUT (`{"fields":{"priority":...}}`) 400'd,
/// producing a false-negative partial write (component change applied,
/// user told the edit failed). After the fix, only ONE PUT fires, with the
/// combined body, and it alone determines success/failure -- no separate
/// component-only PUT can ever land.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_and_invalid_field_one_put_rejects_whole_edit() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    // Decoy (Round-7 pre-fix shape): a PUT whose body is EXACTLY the
    // component-only native update -- this is what the OLD two-PUT code's
    // FIRST PUT looked like. Mounted so a pre-fix regression is caught by a
    // count assertion below (any real hit here means a second, separate PUT
    // fired) -- NOT mounted with .expect(1)/.expect(0) here because its
    // count is asserted indirectly via the total-PUT-count check.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "Backend"}}
                ]
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    // Decoy (Round-7 pre-fix shape): a PUT whose body is EXACTLY the
    // field-only set -- the OLD two-PUT code's SECOND PUT.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "fields": {
                "priority": {"name": "Bogus"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(serde_json::json!({"errorMessages": ["Invalid priority"]})),
        )
        .mount(&server)
        .await;

    // THE FIXED shape: one PUT, combined body.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "Backend"}}
                ]
            },
            "fields": {
                "priority": {"name": "Bogus"}
            }
        })))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(serde_json::json!({"errorMessages": ["Invalid priority"]})),
        )
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
            "--priority",
            "Bogus",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "MEDIUM-1: expected exit 1 (raw ApiError, no --type enrichment path); \
         stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "MEDIUM-1: expected EXACTLY ONE PUT to the issue -- a second, \
         separate component-only PUT landing before the field-validation \
         error is exactly the partial-write bug this fix closes; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "Backend"}}
                ]
            },
            "fields": {
                "priority": {"name": "Bogus"}
            }
        }),
        "MEDIUM-1: the single PUT must carry BOTH update.components and \
         fields.priority in one body"
    );
}

/// Happy-path native combined PUT: `--component add:Backend --summary
/// "New"` → ONE PUT with both `update.components` and `fields.summary`,
/// 204, exit 0.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_and_summary_one_put_native() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
            "--summary",
            "New",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "MEDIUM-1 (happy path native): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "MEDIUM-1 (happy path native): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "Backend"}}
                ]
            },
            "fields": {
                "summary": "New"
            }
        }),
        "MEDIUM-1 (happy path native): the single PUT must carry BOTH \
         update.components and fields.summary"
    );
}

/// Happy-path RMW-fallback combined PUT: editmeta lacks add/remove +
/// `--component add:Backend --summary "New"` → ONE PUT with
/// `fields.components` folded into the SAME `fields` object as the other
/// field change.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_and_summary_one_put_fallback() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_components("FOO-1", "Test", &[]),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
            "--summary",
            "New",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "MEDIUM-1 (happy path fallback): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "MEDIUM-1 (happy path fallback): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"name": "Backend"}
                ],
                "summary": "New"
            }
        }),
        "MEDIUM-1 (happy path fallback): fields.components must be folded \
         into the SAME fields object as the other field change, in one PUT"
    );
}

// ── LOW-2 — dedup an add already present in the existing set (RMW) ───────

/// RMW fallback (editmeta lacks add/remove): `--component add:Backend`
/// where "Backend" is ALREADY present on the issue (with an id) → the
/// set-verb array must NOT contain Backend twice -- the add is deduped
/// against the already-surviving existing entry.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_add_already_present_deduped() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10050", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    let issue_with_id_bearing = common::fixtures::issue_response_with_components_and_ids(
        "FOO-1",
        "Test",
        &[("Backend", "10050")],
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_id_bearing))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "LOW-2: expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(puts.len(), 1, "LOW-2: expected exactly 1 PUT; got {puts:?}");
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"id": "10050"}
                ]
            }
        }),
        "LOW-2: Backend must appear exactly ONCE -- the add must be deduped \
         against the already-surviving existing entry, not appended a \
         second time"
    );
}

// =============================================================================
// Step-4.5 Round 8 — F-LOW-001: accepted, documented divergence for
// cross-identifier contradictory add/remove of the SAME component (docs +
// pinning tests only -- NO behavioral change)
// =============================================================================

/// RMW fallback (editmeta lacks add/remove): the issue's current Backend
/// component has id `"100"`. `--component remove:100 --component
/// add:Backend` (cross-identifier: id `100` IS the component named
/// Backend) → the resulting `fields.components` array CONTAINS Backend --
/// net PRESENT. This is the ACCEPTED-DIVERGENCE contract documented in
/// edit.rs's RMW fallback comment block (point 4, F-LOW-001): this
/// contradictory cross-identifier input is NOT reconciled between the
/// native and RMW-fallback wire paths -- native nets Backend ABSENT (see
/// `test_bc_3_4_022_issue_edit_component_native_cross_identifier_add_remove_nets_absent`
/// below) via Jira's fixed add-before-remove ops ordering, while this RMW
/// path nets Backend PRESENT because its add-survivor filter only excludes
/// a same-`ComponentRef` remove or a surviving-existing id-OR-name match --
/// neither catches a cross-identifier collision (id `100` is never
/// resolved to the name `Backend` here). Accepted because the input is
/// self-contradictory, native's ordering is Jira-fixed, reconciling would
/// require fragile cross-identifier resolution in a path that has already
/// regressed three times, and no UNRELATED component is ever lost on
/// either path.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_rmw_cross_identifier_add_remove_accepted_divergence()
{
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "100", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["set"]).await;

    let issue_with_id_bearing = common::fixtures::issue_response_with_components_and_ids(
        "FOO-1",
        "Test",
        &[("Backend", "100")],
    );
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_with_id_bearing))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:100",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "F-LOW-001 (RMW): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "F-LOW-001 (RMW): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "fields": {
                "components": [
                    {"name": "Backend"}
                ]
            }
        }),
        "F-LOW-001 (RMW): ACCEPTED DIVERGENCE -- cross-identifier \
         remove:100 + add:Backend (same real component) must net PRESENT \
         on the RMW fallback path, the opposite of the native path's net \
         ABSENT. This is documented, intentional, contradictory-input \
         behavior (edit.rs RMW fallback comment, point 4) -- not a bug."
    );
}

/// Native path (editmeta advertises add+remove) counterpart to the RMW
/// test above: the SAME cross-identifier input, `--component remove:100
/// --component add:Backend`, on the native path → the ops array is
/// `[{"add":{"name":"Backend"}},{"remove":{"id":"100"}}]` (add-before-
/// remove, BC-3.4.022 Post 2, unaffected by the id-vs-name discriminator)
/// -- Jira applies this in order, so Backend ends net ABSENT. Pins the
/// OTHER side of the F-LOW-001 accepted divergence.
#[tokio::test]
async fn test_bc_3_4_022_issue_edit_component_native_cross_identifier_add_remove_nets_absent() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "100", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "--component",
            "remove:100",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "F-LOW-001 (native): expected exit 0; stderr={stderr}"
    );

    let puts = s605_1_captured_puts(&server, "FOO-1").await;
    assert_eq!(
        puts.len(),
        1,
        "F-LOW-001 (native): expected exactly 1 PUT; got {puts:?}"
    );
    assert_eq!(
        puts[0],
        serde_json::json!({
            "update": {
                "components": [
                    {"add": {"name": "Backend"}},
                    {"remove": {"id": "100"}}
                ]
            }
        }),
        "F-LOW-001 (native): cross-identifier remove:100 + add:Backend \
         (same real component) must emit add-before-remove per BC-3.4.022 \
         Post 2 -- Jira applies this in order, netting Backend ABSENT, the \
         opposite of the RMW fallback's net PRESENT (documented \
         accepted divergence, edit.rs point 4)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// S-605-2: `issue edit KEY1 KEY2 ... --component add:X` (multi-key/`--jql`
// bulk path) — BC-3.4.023. Coverage for `handle_edit_bulk_components`
// (src/cli/issue/edit.rs) and `build_component_edited_fields`
// (src/api/jira/bulk.rs), both fully implemented.
//
// AC-010 (the live-Jira smoke-test release gate, DEC-280) is NOT here — it
// lives in tests/e2e_live.rs, gated behind JR_RUN_E2E=1 + #[ignore], per that
// file's existing conventions.
// ═══════════════════════════════════════════════════════════════════════════

/// Generate `n` sequential positional issue keys `"{prefix}-1".."{prefix}-n"`.
fn s605_2_keys(prefix: &str, n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("{prefix}-{i}")).collect()
}

/// Mounts `GET /rest/api/3/bulk/queue/{task_id}` -> terminal `COMPLETE`,
/// with every key in `processed_keys` marked successful.
async fn s605_2_mount_poll_complete(server: &MockServer, task_id: &str, processed_keys: &[String]) {
    let keys_ref: Vec<&str> = processed_keys.iter().map(String::as_str).collect();
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_complete(task_id, &keys_ref)),
        )
        .mount(server)
        .await;
}

/// Mounts `GET /rest/api/3/bulk/queue/{task_id}` -> terminal `FAILED`
/// (EC-3.4.023-4 chunk-abort scenario).
async fn s605_2_mount_poll_failed(server: &MockServer, task_id: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(common::fixtures::bulk_task_failed(
                task_id,
                "Simulated chunk failure",
            )),
        )
        .mount(server)
        .await;
}

/// Captures every `POST /rest/api/3/bulk/issues/fields` request body, parsed
/// as JSON, in arrival order.
async fn s605_2_captured_bulk_posts(server: &MockServer) -> Vec<serde_json::Value> {
    let received = server.received_requests().await.unwrap();
    received
        .iter()
        .filter(|r| {
            r.method == wiremock::http::Method::POST
                && r.url.path() == "/rest/api/3/bulk/issues/fields"
        })
        .map(|r| {
            serde_json::from_slice(&r.body)
                .unwrap_or_else(|e| panic!("bulk POST body must be valid JSON: {e}"))
        })
        .collect()
}

// ── AC-001 (BC-3.4.023 Postcondition 1/2 -- wire shape) ────────────────────

/// `jr issue edit FOO-1 FOO-2 --component add:Backend` -> exactly ONE
/// `POST /rest/api/3/bulk/issues/fields` with `selectedActions ==
/// ["components"]` and `editedFieldsInput.multiselectComponents` as a
/// SINGLE OBJECT (not an array) -- `{"fieldId":"components",
/// "components":[{"componentId":10001}],"bulkEditMultiSelectFieldOption":
/// "ADD"}`. Also asserts the POST body's top-level key SET is exactly
/// `{selectedIssueIdsOrKeys, selectedActions, editedFieldsInput}` -- i.e. NO
/// `sendBulkNotification` key (BC-3.4.023 Postcondition 2's 2026-08-19
/// clarification).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_add_wire_shape() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-comp-add-001")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let keys = vec!["FOO-1".to_string(), "FOO-2".to_string()];
    s605_2_mount_poll_complete(&server, "task-comp-add-001", &keys).await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-001: expected exit 0; stderr={stderr}"
    );

    let posts = s605_2_captured_bulk_posts(&server).await;
    assert_eq!(
        posts.len(),
        1,
        "AC-001: expected exactly 1 bulk POST; got {posts:?}"
    );

    assert_eq!(
        posts[0],
        serde_json::json!({
            "selectedIssueIdsOrKeys": ["FOO-1", "FOO-2"],
            "selectedActions": ["components"],
            "editedFieldsInput": {
                "multiselectComponents": {
                    "fieldId": "components",
                    "components": [{"componentId": 10001}],
                    "bulkEditMultiSelectFieldOption": "ADD"
                }
            }
        }),
        "AC-001: bulk POST body must be the exact multiselectComponents wire shape"
    );

    // Clarifying note (2026-08-19): the body's top-level key set MUST be
    // exactly {selectedIssueIdsOrKeys, selectedActions, editedFieldsInput} --
    // no sendBulkNotification key. Asserted independently of the exact-body
    // match above so a future body-shape refactor can't accidentally
    // reintroduce sendBulkNotification while still matching a looser
    // equality check elsewhere.
    let top_level_keys: std::collections::BTreeSet<&str> = posts[0]
        .as_object()
        .expect("AC-001: bulk POST body must be a JSON object")
        .keys()
        .map(String::as_str)
        .collect();
    let expected_keys: std::collections::BTreeSet<&str> = [
        "selectedIssueIdsOrKeys",
        "selectedActions",
        "editedFieldsInput",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        top_level_keys, expected_keys,
        "AC-001: bulk POST body top-level key set must be exactly \
         {{selectedIssueIdsOrKeys, selectedActions, editedFieldsInput}} -- \
         sendBulkNotification MUST be absent (BC-3.4.023 Postcondition 2, \
         2026-08-19 clarification)"
    );
}

/// Step-4.5 Round-3 F3: `--component add:Backend --component add:Frontend`
/// on 2 keys are the SAME action (ADD), so `resolve_bulk_component_ids`
/// buckets both resolved ids into one `add_ids` vec and
/// `build_component_edited_fields` must emit BOTH as a 2-element
/// `components` array in a SINGLE POST -- never one POST per component, and
/// never silently truncated to the first id. Kills a `.take(1)`-shaped
/// mutation on the component id list, which no prior test (all single-id)
/// could catch.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_two_adds_one_action_emits_two_element_array() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-comp-two-adds")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let keys = vec!["FOO-1".to_string(), "FOO-2".to_string()];
    s605_2_mount_poll_complete(&server, "task-comp-two-adds", &keys).await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
            "--component",
            "add:Frontend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "F3: expected exit 0; stderr={stderr}"
    );

    let posts = s605_2_captured_bulk_posts(&server).await;
    assert_eq!(
        posts.len(),
        1,
        "F3: two adds in one action must be ONE bulk POST, not one per component; got {posts:?}"
    );

    assert_eq!(
        posts[0],
        serde_json::json!({
            "selectedIssueIdsOrKeys": ["FOO-1", "FOO-2"],
            "selectedActions": ["components"],
            "editedFieldsInput": {
                "multiselectComponents": {
                    "fieldId": "components",
                    "components": [{"componentId": 10001}, {"componentId": 10002}],
                    "bulkEditMultiSelectFieldOption": "ADD"
                }
            }
        }),
        "F3: expected an exact, ordered 2-element components array [Backend, Frontend]"
    );
}

// ── AC-002 (BC-3.4.023 Postcondition 3 -- two sequential POSTs) ────────────

/// `jr issue edit FOO-1 FOO-2 --component add:Backend --component
/// remove:Frontend` -> EXACTLY two sequential `POST /bulk/issues/fields`
/// calls, ADD first (fully polled to completion) then REMOVE -- NOT one
/// coalesced POST (the label bulk path's shape, which this path
/// deliberately diverges from per the BC-3.4.023 pass-14 correction).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_mixed_add_remove_two_sequential_posts() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;

    let keys = vec!["FOO-1".to_string(), "FOO-2".to_string()];

    // Distinguish the ADD POST from the REMOVE POST via body content -- each
    // must be a SEPARATE POST (never coalesced into one body carrying both
    // actions).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "editedFieldsInput": {
                "multiselectComponents": {
                    "bulkEditMultiSelectFieldOption": "ADD"
                }
            }
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-comp-add")),
        )
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_complete(&server, "task-comp-add", &keys).await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "editedFieldsInput": {
                "multiselectComponents": {
                    "bulkEditMultiSelectFieldOption": "REMOVE"
                }
            }
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-comp-remove")),
        )
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_complete(&server, "task-comp-remove", &keys).await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
            "--component",
            "remove:Frontend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-002: expected exit 0; stderr={stderr}"
    );

    let posts = s605_2_captured_bulk_posts(&server).await;
    assert_eq!(
        posts.len(),
        2,
        "AC-002: expected exactly 2 sequential bulk POSTs (ADD then REMOVE, \
         never coalesced); got {posts:?}"
    );

    let first_option =
        posts[0]["editedFieldsInput"]["multiselectComponents"]["bulkEditMultiSelectFieldOption"]
            .as_str();
    let second_option =
        posts[1]["editedFieldsInput"]["multiselectComponents"]["bulkEditMultiSelectFieldOption"]
            .as_str();
    assert_eq!(
        first_option,
        Some("ADD"),
        "AC-002: first bulk POST must be the ADD action; posts={posts:?}"
    );
    assert_eq!(
        second_option,
        Some("REMOVE"),
        "AC-002: second bulk POST must be the REMOVE action; posts={posts:?}"
    );

    // Step-4.5 Round-1 F2 fix: `render_bulk_edit_results` used to be called
    // once PER (chunk, action) pair inside the loop, so a mixed add:/remove:
    // edit printed TWO concatenated `println!("{}", ...)` JSON documents on
    // stdout -- not valid as a single JSON parse. Results must now be
    // accumulated across the whole invocation and rendered ONCE: exactly one
    // top-level JSON value parses from the full stdout.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(
        parsed.is_ok(),
        "AC-002/F2: expected stdout to be exactly ONE parseable top-level JSON \
         document, got parse error {:?}; stdout={stdout}",
        parsed.err()
    );
}

// ── AC-003 (BC-3.4.023 Invariant 2 -- integer componentId, parse step) ─────

/// The resolved `String` component id is explicitly parsed to a numeric
/// type before body assembly: `components[].componentId` in the POST body
/// is a JSON integer, never a string and never a `{"name":...}` object.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_id_is_json_integer_not_string() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "20007", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-comp-intid")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let keys = vec!["FOO-1".to_string(), "FOO-2".to_string()];
    s605_2_mount_poll_complete(&server, "task-comp-intid", &keys).await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-003: expected exit 0; stderr={stderr}"
    );

    let posts = s605_2_captured_bulk_posts(&server).await;
    assert_eq!(posts.len(), 1, "AC-003: expected exactly 1 bulk POST");

    let components = posts[0]["editedFieldsInput"]["multiselectComponents"]["components"]
        .as_array()
        .expect("AC-003: components must be a JSON array");
    assert_eq!(
        components.len(),
        1,
        "AC-003: expected exactly 1 component entry"
    );

    let component_id_value = &components[0]["componentId"];
    assert!(
        component_id_value.is_u64(),
        "AC-003: componentId must be a JSON integer (u64-representable); \
         got {component_id_value:?}"
    );
    assert_eq!(
        component_id_value.as_u64(),
        Some(20007),
        "AC-003: componentId must equal the resolved numeric id 20007"
    );
    assert!(
        !component_id_value.is_string(),
        "AC-003: componentId MUST NOT be a JSON string; got {component_id_value:?}"
    );
    assert!(
        components[0].get("name").is_none(),
        "AC-003: components[] entries MUST NOT be {{\"name\":...}} objects \
         (that shape belongs to BC-3.4.022's single-key path, not this bulk \
         path); got {:?}",
        components[0]
    );
}

// ── AC-004 (BC-3.4.023 Postcondition 4 -- resolution before POST) ──────────

/// Component NAMES resolve via §8.4 to numeric ids BEFORE the bulk POST is
/// built; an unknown/ambiguous name -> exit 64, ZERO bulk POST calls.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_unknown_name_zero_post() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Nonexistent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-004: expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains(
            "Component 'Nonexistent' not found in project FOO. Available: Backend, Frontend."
        ),
        "AC-004: expected BC-8.4.002 canonical not-found message; stderr={stderr}"
    );
}

// ── AC-005 (BC-3.4.023 Edge Case EC-3.4.023-1 -- cross-project guard) ──────

/// Keys spanning 2+ projects with `--component` -> exit 64 BEFORE any HTTP
/// (mirrors BC-3.4.019's `--type` cross-project guard exactly -- component
/// ids are project-scoped).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_cross_project_guard() {
    let server = MockServer::start().await;
    // Mount NO mocks -- any HTTP call is a test failure; verified via
    // received_requests().len() == 0 below.

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "BAR-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-005: expected exit 64 for cross-project --component keys; \
         stderr={stderr} stdout={stdout}"
    );
    assert!(
        stderr.contains("--component"),
        "AC-005: expected '--component' in error message; stderr={stderr}"
    );
    assert!(
        stderr.contains("FOO"),
        "AC-005: expected project key 'FOO' in error message; stderr={stderr}"
    );
    assert!(
        stderr.contains("BAR"),
        "AC-005: expected project key 'BAR' in error message; stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    assert_eq!(
        received.len(),
        0,
        "AC-005: expected zero HTTP calls before the cross-project guard \
         fires; got {} requests: {:?}",
        received.len(),
        received
            .iter()
            .map(|r| format!("{} {}", r.method, r.url))
            .collect::<Vec<_>>()
    );
}

// ── Step-4.5 Round-2 fix F1 (dry-run/live cross-project divergence) ────────
//
// The cross-project guard above (AC-005) only ever fired inside the LIVE
// path (`handle_edit_bulk_components`), which the pre-fix `--dry-run` block
// never reached (it returns before that function is called). A multi-key
// `--component --dry-run` spanning 2+ projects therefore resolved against
// ONLY `effective_keys[0]`'s project and previewed success for an input the
// live run refuses with exit 64. `--dry-run` must exercise the SAME guard
// the live path enforces (mirrors the pre-existing `--type` guard, which is
// hoisted above the dry-run block specifically for this reason).

/// `jr issue edit FOO-1 BAR-2 --component add:Backend --dry-run` (keys
/// spanning 2 projects) must exit 64 with ZERO HTTP calls -- the SAME
/// cross-project error the live (non-dry-run) run produces (AC-005) --
/// instead of previewing a plan resolved against only FOO-1's project.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_dry_run_cross_project_exits_64_zero_http() {
    let server = MockServer::start().await;
    // Mount NO mocks -- the hoisted guard is purely client-side (no HTTP
    // needed to detect a cross-project key set), so any HTTP call at all is
    // a test failure; verified via received_requests().len() == 0 below.

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "BAR-2",
            "--component",
            "add:Backend",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(64),
        "F1 (dry-run): expected exit 64 for cross-project --component keys, \
         matching the live run's guard; stderr={stderr} stdout={stdout}"
    );
    assert!(
        stderr.contains("--component"),
        "F1 (dry-run): expected '--component' in error message; stderr={stderr}"
    );
    assert!(
        stderr.contains("FOO") && stderr.contains("BAR"),
        "F1 (dry-run): expected both project keys in error message; stderr={stderr}"
    );
    // Zero stdout: a preview must never be emitted alongside an exit-64
    // error (Invariant 2/3, same postcondition the pre-existing dry-run
    // resolution-error tests in this file already pin).
    assert!(
        stdout.is_empty(),
        "F1 (dry-run): expected empty stdout on error; stdout={stdout}"
    );

    let received = server.received_requests().await.unwrap();
    assert_eq!(
        received.len(),
        0,
        "F1 (dry-run): expected zero HTTP calls before the cross-project \
         guard fires; got {} requests: {:?}",
        received.len(),
        received
            .iter()
            .map(|r| format!("{} {}", r.method, r.url))
            .collect::<Vec<_>>()
    );
}

// ── Step-4.5 Round-2 fix F2 (dry-run oversized-numeric-id divergence) ──────
//
// The multi-key `--component` LIVE path resolves each change to a numeric
// `componentId` via `resolve_bulk_component_ids`, which performs an
// explicit `String -> u64` parse (Invariant 2) that can fail on an
// oversized numeric-bypass id even when NAME resolution (the only step
// dry-run previously performed) succeeds. Dry-run must exercise the SAME
// parse so it never previews success for an input the live run rejects.

/// `jr issue edit FOO-1 FOO-2 --component add:99999999999999999999999999
/// --dry-run` -> exit 64 (`JrError::UserError`, same as the live run --
/// see `test_bc_3_4_023_issue_edit_bulk_component_oversized_numeric_id_is_user_error`),
/// with zero POSTs -- not a preview of success.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_dry_run_oversized_numeric_id_exits_64() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:99999999999999999999999999",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F2 (dry-run): an oversized numeric --component id must exit 64 in \
         dry-run, matching the live run's JrError::UserError; stderr={stderr}"
    );
    assert!(
        stderr.contains("99999999999999999999999999"),
        "F2 (dry-run): expected the offending value echoed in the error message; \
         stderr={stderr}"
    );
    assert!(
        stdout.is_empty(),
        "F2 (dry-run): expected empty stdout on error; stdout={stdout}"
    );

    let received = server.received_requests().await.unwrap();
    let posts: Vec<_> = received
        .iter()
        .filter(|r| r.method == wiremock::http::Method::POST)
        .collect();
    assert!(
        posts.is_empty(),
        "F2 (dry-run): the oversized componentId must never reach a POST; got {posts:?}"
    );
}

/// Happy-path regression pin (Step-4.5 Round-2, NOTE): a VALID multi-key
/// `--component --dry-run` (same project, resolvable names) must still
/// preview successfully after F1/F2 hoist the cross-project guard and add
/// the numeric-id resolution check -- neither change may regress the
/// normal case. Mirrors the single-key AC-016 json-mode assertion shape.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_dry_run_multi_key_happy_path_previews() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
            "--dry-run",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "happy-path regression: expected exit 0; stderr={stderr}"
    );
    let body: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("happy-path regression: stdout is not valid JSON: {e}; stdout={stdout}")
    });
    assert_eq!(
        body["plannedChanges"]["components"],
        serde_json::json!([{"action": "ADD", "name": "Backend"}]),
        "happy-path regression: plannedChanges.components must still preview \
         correctly for a valid multi-key --component --dry-run; body={body}"
    );

    // Step-4.5 Round-3 F2: the dry-run path previously fetched the
    // project's component candidate list TWICE -- once via
    // `resolve_component_change_names` (for the preview) and once via
    // `resolve_bulk_component_ids` (for id/parse validation) -- each
    // independently GETting `/rest/api/3/project/{key}/components` for the
    // SAME project. The consolidated path fetches it exactly ONCE and
    // reuses the result for both.
    let received = server.received_requests().await.unwrap();
    let component_gets = received
        .iter()
        .filter(|r| {
            r.method == wiremock::http::Method::GET
                && r.url.path() == "/rest/api/3/project/FOO/components"
        })
        .count();
    assert_eq!(
        component_gets, 1,
        "F2: expected exactly 1 GET .../project/FOO/components call for a \
         multi-key --component --dry-run (name-resolution and id-validation \
         must reuse ONE fetched list, not fetch it twice); got {component_gets}"
    );
}

// ── AC-006 (BC-3.4.023 Edge Case EC-3.4.023-3 -- single-issue fallthrough) ─

/// `--jql` matching exactly 1 issue -> routes to S-605-1's single-key path
/// (BC-3.4.022, native `update`-verb PUT), NOT this bulk path.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_component_jql_single_match_uses_single_key_path() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::component_delete_snapshot_page(&["FOO-1"], None),
        ))
        .expect(1)
        .mount(&server)
        .await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s605_1_mock_editmeta(&server, "FOO-1", &["add", "remove"]).await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // The bulk endpoint MUST NOT be called for a single-match --jql.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "key = FOO-1",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-006: expected exit 0 for single-match --jql routed to the \
         single-key path; stderr={stderr}"
    );
    // wiremock .expect(0/1) assertions verify routing on Drop.
}

// ── AC-007 (BC-3.4.023 Postcondition 6 -- 1000-issue chunking) ─────────────

/// 1500 issues in one project, `--component add:Backend` -> TWO sequential
/// bulk POSTs: the first carrying 1000 issues' worth of
/// `selectedIssueIdsOrKeys`, the second the remaining 500 -- each polled to
/// completion before the next chunk's POST fires. Exit 0 iff BOTH chunks
/// succeed.
///
/// The `keys` positional's clap definition (`src/cli/mod.rs`, `Edit { keys,
/// ... }`) carries `#[arg(num_args = 0..=10000, ...)]`, widened for exactly
/// this scenario (BC-3.4.023 Postcondition 6) -- a >1000-key `--component`
/// invocation reaches `handle_edit_bulk_components`'s chunking loop below.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_1000_issue_chunking() {
    let server = MockServer::start().await;

    let keys = s605_2_keys("FOO", 1500);
    let chunk1: Vec<String> = keys[..1000].to_vec();
    let chunk2: Vec<String> = keys[1000..].to_vec();

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": chunk1
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-chunk-1")),
        )
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_complete(&server, "task-chunk-1", &chunk1).await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": chunk2
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-chunk-2")),
        )
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_complete(&server, "task-chunk-2", &chunk2).await;

    let mut args: Vec<String> = vec!["--no-input".into(), "issue".into(), "edit".into()];
    args.extend(keys.iter().cloned());
    args.push("--component".into());
    args.push("add:Backend".into());

    let output = s605_1_cmd(&server.uri()).args(&args).output().unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-007: expected exit 0 when both chunks succeed; stderr={stderr}"
    );

    let posts = s605_2_captured_bulk_posts(&server).await;
    assert_eq!(
        posts.len(),
        2,
        "AC-007: expected exactly 2 sequential chunked bulk POSTs (1000 \
         then 500); got {} posts",
        posts.len()
    );
    assert_eq!(
        posts[0]["selectedIssueIdsOrKeys"],
        serde_json::json!(chunk1),
        "AC-007: first POST must carry the first 1000 keys, in order"
    );
    assert_eq!(
        posts[1]["selectedIssueIdsOrKeys"],
        serde_json::json!(chunk2),
        "AC-007: second POST must carry the remaining 500 keys, in order"
    );
}

// ── AC-008 (BC-3.4.023 Postcondition 6 -- chunk-major/action-minor) ────────

/// 1500 issues, `--component add:X --component remove:Y` (both >1000
/// chunking AND mixed add:/remove:) -> `2 * ceil(1500/1000) == 4` sequential
/// POSTs total, chunk-major then action-minor ordering: chunk1-ADD,
/// chunk1-REMOVE, chunk2-ADD, chunk2-REMOVE.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_chunking_and_mixed_ops_four_posts() {
    let server = MockServer::start().await;

    let keys = s605_2_keys("FOO", 1500);
    let chunk1: Vec<String> = keys[..1000].to_vec();
    let chunk2: Vec<String> = keys[1000..].to_vec();

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;

    // Four distinct (chunk, action) combinations, each its own POST + poll.
    let combos: [(&str, &Vec<String>, &str); 4] = [
        ("task-c1-add", &chunk1, "ADD"),
        ("task-c1-remove", &chunk1, "REMOVE"),
        ("task-c2-add", &chunk2, "ADD"),
        ("task-c2-remove", &chunk2, "REMOVE"),
    ];
    for (task_id, chunk_keys, option) in &combos {
        Mock::given(method("POST"))
            .and(path("/rest/api/3/bulk/issues/fields"))
            .and(body_partial_json(serde_json::json!({
                "selectedIssueIdsOrKeys": chunk_keys,
                "editedFieldsInput": {
                    "multiselectComponents": {
                        "bulkEditMultiSelectFieldOption": option
                    }
                }
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(common::fixtures::bulk_task_enqueued(task_id)),
            )
            .expect(1)
            .mount(&server)
            .await;
        s605_2_mount_poll_complete(&server, task_id, chunk_keys).await;
    }

    let mut args: Vec<String> = vec!["--no-input".into(), "issue".into(), "edit".into()];
    args.extend(keys.iter().cloned());
    args.push("--component".into());
    args.push("add:Backend".into());
    args.push("--component".into());
    args.push("remove:Frontend".into());

    let output = s605_1_cmd(&server.uri()).args(&args).output().unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC-008: expected exit 0; stderr={stderr}"
    );

    let posts = s605_2_captured_bulk_posts(&server).await;
    assert_eq!(
        posts.len(),
        4,
        "AC-008: expected exactly 2*ceil(1500/1000)==4 sequential POSTs; \
         got {} posts",
        posts.len()
    );

    // Chunk-major, action-minor ordering: chunk1-ADD, chunk1-REMOVE,
    // chunk2-ADD, chunk2-REMOVE.
    let expected_order = [
        (&chunk1, "ADD"),
        (&chunk1, "REMOVE"),
        (&chunk2, "ADD"),
        (&chunk2, "REMOVE"),
    ];
    for (i, (expected_keys, expected_option)) in expected_order.iter().enumerate() {
        assert_eq!(
            posts[i]["selectedIssueIdsOrKeys"],
            serde_json::json!(expected_keys),
            "AC-008: POST #{} must carry the expected chunk's keys; posts={posts:?}",
            i + 1
        );
        assert_eq!(
            posts[i]["editedFieldsInput"]["multiselectComponents"]
                ["bulkEditMultiSelectFieldOption"]
                .as_str(),
            Some(*expected_option),
            "AC-008: POST #{} must carry the expected action (chunk-major, \
             action-minor ordering); posts={posts:?}",
            i + 1
        );
    }
}

// ── AC-009 (BC-3.4.023 Edge Case EC-3.4.023-4 -- chunk-failure abort) ──────

/// 2500 issues (3 chunks: 1000, 1000, 500), `--component add:Backend` --
/// chunk 1 succeeds, chunk 2 FAILS -> chunk 3 is NEVER attempted. Chunk 1's
/// already-committed change is NOT rolled back (non-transactional across
/// chunks); the error output surfaces only chunk 2's `await_bulk_task`
/// failure, not a per-chunk `renamed[]`/`failed[]` report shape (unlike
/// `component rename --all-projects`).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_chunk_failure_aborts_remaining() {
    let server = MockServer::start().await;

    let keys = s605_2_keys("FOO", 2500);
    let chunk1: Vec<String> = keys[..1000].to_vec();
    let chunk2: Vec<String> = keys[1000..2000].to_vec();
    let chunk3: Vec<String> = keys[2000..].to_vec();

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    // Chunk 1: succeeds.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": chunk1
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-c1-ok")),
        )
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_complete(&server, "task-c1-ok", &chunk1).await;

    // Chunk 2: FAILS.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": chunk2
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-c2-fail")),
        )
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_failed(&server, "task-c2-fail").await;

    // Chunk 3: MUST NEVER be attempted.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": chunk3
        })))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let mut args: Vec<String> = vec!["--no-input".into(), "issue".into(), "edit".into()];
    args.extend(keys.iter().cloned());
    args.push("--component".into());
    args.push("add:Backend".into());

    let output = s605_1_cmd(&server.uri()).args(&args).output().unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "AC-009: expected exit 1 (generic bulk-task failure, not a \
         JrError::UserError) when chunk 2 fails; stderr={stderr}"
    );
    assert!(
        stderr.contains("FAILED"),
        "AC-009: expected the await_bulk_task FAILED-status error to \
         surface in stderr; stderr={stderr}"
    );

    let posts = s605_2_captured_bulk_posts(&server).await;
    assert_eq!(
        posts.len(),
        2,
        "AC-009: expected exactly 2 bulk POSTs (chunk 1 + chunk 2) -- chunk \
         3 must never be attempted after chunk 2 fails; got {} posts",
        posts.len()
    );
    assert_eq!(
        posts[0]["selectedIssueIdsOrKeys"],
        serde_json::json!(chunk1),
        "AC-009: first POST must be chunk 1"
    );
    assert_eq!(
        posts[1]["selectedIssueIdsOrKeys"],
        serde_json::json!(chunk2),
        "AC-009: second POST must be chunk 2 (the one that fails)"
    );
}

// ── Step-4.5 Round-1 F3 fix (BC-3.4.023 Postcondition 6 -- --jql chunking) ─

/// `--jql` matching 1500 issues + `--component add:Backend` -> TWO
/// sequential chunked bulk POSTs (1000 then 500), mirroring AC-007's
/// positional-key scenario but exercised via the `--jql` path.
///
/// Before this fix, `effective_max` was unconditionally clamped to
/// `BULK_MAX_KEYS` (1000) regardless of `--component`, and the `--max`
/// clap flag itself was capped at `1..=1000` -- so a >1000-issue `--jql`
/// match could never reach the chunking loop this test exercises (only the
/// positional-keys path could, since `keys`'s clap arg was separately
/// widened to `0..=10000` for S-605-2). This test proves the `--jql` path
/// now reaches the same chunking behavior.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_jql_1500_issue_chunking() {
    let server = MockServer::start().await;

    let keys = s605_2_keys("FOO", 1500);
    let chunk1: Vec<String> = keys[..1000].to_vec();
    let chunk2: Vec<String> = keys[1000..].to_vec();

    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::component_delete_snapshot_page(&key_refs, None),
        ))
        .expect(1)
        .mount(&server)
        .await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": chunk1
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-jql-chunk-1")),
        )
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_complete(&server, "task-jql-chunk-1", &chunk1).await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "selectedIssueIdsOrKeys": chunk2
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-jql-chunk-2")),
        )
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_complete(&server, "task-jql-chunk-2", &chunk2).await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = FOO",
            "--max",
            "1500",
            "--yes",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "F3: expected exit 0 for --jql 1500-issue --component chunking; stderr={stderr}"
    );

    let posts = s605_2_captured_bulk_posts(&server).await;
    assert_eq!(
        posts.len(),
        2,
        "F3: expected exactly 2 sequential chunked bulk POSTs (1000 then \
         500) via --jql; got {} posts",
        posts.len()
    );
    assert_eq!(
        posts[0]["selectedIssueIdsOrKeys"],
        serde_json::json!(chunk1),
        "F3: first POST must carry the first 1000 keys, in order"
    );
    assert_eq!(
        posts[1]["selectedIssueIdsOrKeys"],
        serde_json::json!(chunk2),
        "F3: second POST must carry the remaining 500 keys, in order"
    );
}

/// `--max` above 1000 WITHOUT `--component` (e.g. combined with `--label`)
/// must still be rejected -- the F3 widening only applies to the
/// `--component` bulk path, which chunks internally; every other bulk
/// field path issues a single un-chunked POST and stays hard-capped at
/// `BULK_MAX_KEYS` (1000).
#[tokio::test]
async fn test_max_above_1000_without_component_still_rejected() {
    let server = MockServer::start().await;
    // No bulk POST mock -- the ceiling must fire before any mutation call.

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "--jql",
            "project = FOO",
            "--max",
            "1500",
            "--yes",
            "--label",
            "add:urgent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F3: expected exit 64 for --max 1500 without --component; stderr={stderr}"
    );
    assert!(
        stderr.contains("1000"),
        "F3: expected the 1000-issue ceiling named in the error; stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    assert_eq!(
        received.len(),
        0,
        "F3: expected zero HTTP calls before the ceiling guard fires; got {} \
         requests: {:?}",
        received.len(),
        received
            .iter()
            .map(|r| format!("{} {}", r.method, r.url))
            .collect::<Vec<_>>()
    );
}

// ── Defensive test: componentId String->u64 parse-failure internal-invariant

/// If the §8.4 resolver ever returns a non-numeric component id (an
/// internal-invariant violation that should never happen in practice --
/// every resolver-returned component id is itself a digit-only string on
/// the wire), the bulk `--component` path MUST NOT silently truncate or
/// coerce the value, and MUST NOT surface it as a `JrError::UserError`
/// (exit 64) -- that exit code is reserved for user-input errors, and a
/// malformed resolver id is not user input. It also MUST NOT crash as an
/// unhandled Rust panic (`panic!`/`unwrap()`/`expect()` bubbling all the
/// way to the process) -- BC-3.4.023 Invariant 2 calls this "an unexpected
/// internal error", which this codebase's convention (`error.rs`'s
/// `JrError` taxonomy) treats as a gracefully-propagated `Err`, not an
/// uncaught panic. This test asserts the negative space: exit code is NOT
/// 64 (not treated as a user error), NOT 0 (not silently swallowed), stderr
/// does NOT contain Rust's default panic banner, and -- most importantly --
/// zero bulk POST calls are made (the malformed id must never reach the
/// wire).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_id_parse_failure_surfaces_as_internal_error() {
    let server = MockServer::start().await;

    // A component resource with a non-numeric id -- the resolver returns
    // this id verbatim as a String; the bulk path's explicit
    // `id.parse::<u64>()` step must fail on it.
    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "not-a-number",
            "Backend",
            None,
            None,
            None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code();
    assert_ne!(
        exit_code,
        Some(64),
        "componentId parse failure is an internal-invariant violation, not \
         a JrError::UserError -- MUST NOT exit 64; stderr={stderr}"
    );
    assert_ne!(
        exit_code,
        Some(0),
        "componentId parse failure MUST NOT be silently swallowed as \
         success; stderr={stderr}"
    );
    assert!(
        !stderr.contains("panicked at"),
        "componentId parse failure must be handled as a graceful internal \
         error (a propagated Err), not an unhandled Rust panic; stderr={stderr}"
    );
    // Step-4.5 Round-2 fix (F4): positively confirm the `JrError::Internal`
    // branch actually fired, not merely that some non-64/non-0/non-panic
    // exit occurred. Every `JrError::Internal` call site is required (per
    // `error.rs`'s doc comment) to self-describe with the literal
    // "Internal error:" prefix, which `main.rs` then echoes verbatim via
    // `eprintln!("Error: {e}")`.
    assert!(
        stderr.contains("Internal error:"),
        "componentId parse failure must surface via the JrError::Internal \
         branch, whose message carries the literal \"Internal error:\" \
         marker; stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    let bulk_posts: Vec<_> = received
        .iter()
        .filter(|r| {
            r.method == wiremock::http::Method::POST
                && r.url.path() == "/rest/api/3/bulk/issues/fields"
        })
        .collect();
    assert!(
        bulk_posts.is_empty(),
        "the malformed componentId must never reach the wire; got {bulk_posts:?}"
    );
}

// ── Step-4.5 Round-1 F4 fix (BC-3.4.023 Invariant 2 -- numeric-bypass
//    oversized id is user input, not an internal invariant violation) ──────

/// `--component add:<oversized-all-digit-string>` (a value that IS
/// all-ASCII-digit, so it takes the §8.4 numeric-id bypass and skips
/// `partial_match` entirely, but overflows `u64::MAX`) is USER input, not a
/// resolver-returned name -- the parse failure must surface as
/// `JrError::UserError` (exit 64), never `JrError::Internal`. Before this
/// fix, `resolve_bulk_component_ids` mapped every `id.parse::<u64>()`
/// failure to `JrError::Internal` regardless of source, incorrectly
/// asserting (in both the code comment and the error text) that the value
/// "did not come from user input" even on this bypass path.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_oversized_numeric_id_is_user_error() {
    let server = MockServer::start().await;

    // The numeric bypass (helpers::resolve_component) never calls
    // partial_match for an all-digit input, so the component-list GET is
    // still made (candidate list fetch happens unconditionally before the
    // per-change resolve loop) but its contents are irrelevant here.
    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:99999999999999999999999999",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code();
    assert_eq!(
        exit_code,
        Some(64),
        "F4: an oversized numeric --component id is USER input (via the \
         §8.4 numeric-id bypass) -- the parse failure must be a \
         JrError::UserError (exit 64), not JrError::Internal; stderr={stderr}"
    );
    assert!(
        stderr.contains("99999999999999999999999999"),
        "F4: expected the offending value echoed in the error message; \
         stderr={stderr}"
    );
    assert!(
        !stderr.contains("panicked at"),
        "F4: must not surface as an unhandled Rust panic; stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    let bulk_posts: Vec<_> = received
        .iter()
        .filter(|r| {
            r.method == wiremock::http::Method::POST
                && r.url.path() == "/rest/api/3/bulk/issues/fields"
        })
        .collect();
    assert!(
        bulk_posts.is_empty(),
        "F4: the oversized componentId must never reach the wire; got {bulk_posts:?}"
    );
}

// ── Step-4.5 Round-1 F5 fix (BC-3.4.023 -- ExactMultiple/Ambiguous coverage
//    on the bulk path) ──────────────────────────────────────────────────────
//
// AC-004 (`test_bc_3_4_023_issue_edit_bulk_component_unknown_name_zero_post`)
// only exercises `MatchResult::None`. The `ExactMultiple` and `Ambiguous`
// branches of `resolve_bulk_component_ids` (mirrors the single-key
// MEDIUM-2 tests above, `test_bc_8_4_003_issue_edit_component_*`) had ZERO
// coverage on the bulk (2+ key) path -- a mutation dropping either branch's
// exit-64/zero-POST guard would have stayed green.

/// Bulk path: a component-list GET returning two components whose names
/// are identical case-insensitively ("Backend" id 10001, "backend" id
/// 10002) -> `MatchResult::ExactMultiple` -> exit 64, ZERO bulk POST, exact
/// BC-8.4.003 message (mirrors `test_bc_8_4_003_issue_edit_component_exact_multiple_exits_64`,
/// applied to the 2+-key bulk resolver call).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_exact_multiple_zero_post() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "backend", None, None, None),
        ],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F5 (ExactMultiple): expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains(
            "Multiple components named \"Backend\" found (IDs: 10001, 10002). \
             Pass the numeric ID directly."
        ),
        "F5 (ExactMultiple): expected exact BC-8.4.003 message; stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    let bulk_posts: Vec<_> = received
        .iter()
        .filter(|r| {
            r.method == wiremock::http::Method::POST
                && r.url.path() == "/rest/api/3/bulk/issues/fields"
        })
        .collect();
    assert!(
        bulk_posts.is_empty(),
        "F5 (ExactMultiple): zero bulk POSTs expected; got {bulk_posts:?}"
    );
}

/// Bulk path: `--component add:Amb` where the project component list has
/// two partial matches ("Ambition" id 20002, "Amber" id 20001, mounted in
/// reverse-alphabetical fixture order) -> `MatchResult::Ambiguous` -> exit
/// 64, ZERO bulk POST, exact BC-8.4.003 message (mirrors
/// `test_bc_8_4_003_issue_edit_component_ambiguous_exits_64`, applied to
/// the 2+-key bulk resolver call).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_ambiguous_zero_post() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("20002", "Ambition", None, None, None),
            common::fixtures::component_response("20001", "Amber", None, None, None),
        ],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(501).set_body_string("must not be called"))
        .expect(0)
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Amb",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F5 (Ambiguous): expected exit 64; stderr={stderr}"
    );
    assert!(
        stderr.contains("Ambiguous component 'Amb'. Matches: Amber, Ambition."),
        "F5 (Ambiguous): expected exact BC-8.4.003 message (alphabetically- \
         sorted Matches list); stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    let bulk_posts: Vec<_> = received
        .iter()
        .filter(|r| {
            r.method == wiremock::http::Method::POST
                && r.url.path() == "/rest/api/3/bulk/issues/fields"
        })
        .collect();
    assert!(
        bulk_posts.is_empty(),
        "F5 (Ambiguous): zero bulk POSTs expected; got {bulk_posts:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Step-4.5 Round-1 F1 fix (BC-3.4.023): multi-key `--component` mutual
// exclusion with --summary/--priority/--type/--label. Before this fix,
// `handle_edit`'s routing dispatched straight to `handle_edit_bulk_components`
// whenever `!components.is_empty() && effective_keys.len() > 1`, BEFORE the
// generic bulk-fields routing a few lines below that would have carried
// --summary/--priority/--type — so those flags were silently discarded
// (exit 0, no error, no PUT/POST reflecting them). --label was already
// guarded (BC-3.4.020 amendment), so that combination is a regression test
// confirming the pre-existing guard, not new behavior.
// ═══════════════════════════════════════════════════════════════════════════

/// `--component` + `--summary` on 2+ keys -> exit 64, ZERO HTTP calls (no
/// silent drop of --summary).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_plus_summary_exits_64_zero_http() {
    let server = MockServer::start().await;
    // No mocks mounted -- the mutual-exclusion guard must fire before any
    // HTTP call (verified via received_requests().len() == 0 below).

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
            "--summary",
            "New title",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F1: expected exit 64 for --component + --summary on a multi-key \
         edit; stderr={stderr} stdout={stdout}"
    );
    assert!(
        stderr.contains("--component"),
        "F1: expected '--component' named in the guard message; stderr={stderr}"
    );
    assert!(
        stderr.contains("--summary"),
        "F1: expected '--summary' named in the guard message; stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    assert_eq!(
        received.len(),
        0,
        "F1: expected zero HTTP calls before the guard fires; got {} requests: {:?}",
        received.len(),
        received
            .iter()
            .map(|r| format!("{} {}", r.method, r.url))
            .collect::<Vec<_>>()
    );
}

/// `--component` + `--priority` on 2+ keys -> exit 64, ZERO HTTP calls.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_plus_priority_exits_64_zero_http() {
    let server = MockServer::start().await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
            "--priority",
            "High",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F1: expected exit 64 for --component + --priority on a multi-key \
         edit; stderr={stderr}"
    );
    assert!(
        stderr.contains("--component"),
        "F1: expected '--component' named in the guard message; stderr={stderr}"
    );
    assert!(
        stderr.contains("--priority"),
        "F1: expected '--priority' named in the guard message; stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    assert_eq!(
        received.len(),
        0,
        "F1: expected zero HTTP calls before the guard fires; got {} requests: {:?}",
        received.len(),
        received
            .iter()
            .map(|r| format!("{} {}", r.method, r.url))
            .collect::<Vec<_>>()
    );
}

/// `--component` + `--type` on 2+ keys -> exit 64, ZERO HTTP calls.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_plus_type_exits_64_zero_http() {
    let server = MockServer::start().await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
            "--type",
            "Bug",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F1: expected exit 64 for --component + --type on a multi-key edit; \
         stderr={stderr}"
    );
    assert!(
        stderr.contains("--component"),
        "F1: expected '--component' named in the guard message; stderr={stderr}"
    );
    assert!(
        stderr.contains("--type"),
        "F1: expected '--type' named in the guard message; stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    assert_eq!(
        received.len(),
        0,
        "F1: expected zero HTTP calls before the guard fires; got {} requests: {:?}",
        received.len(),
        received
            .iter()
            .map(|r| format!("{} {}", r.method, r.url))
            .collect::<Vec<_>>()
    );
}

/// `--component` + `--label` on 2+ keys -> exit 64, ZERO HTTP calls.
/// Regression test: this combination is already rejected by the
/// pre-existing BC-3.4.020 amendment `--label` conflict guard (which fires
/// earlier, unconditionally on any key count) -- this test pins that the
/// combination still fails closed after the F1 fix, rather than exercising
/// new code.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_plus_label_exits_64_zero_http() {
    let server = MockServer::start().await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
            "--label",
            "add:urgent",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(64),
        "F1: expected exit 64 for --component + --label on a multi-key \
         edit; stderr={stderr}"
    );
    assert!(
        stderr.contains("--component"),
        "F1: expected '--component' named in the guard message; stderr={stderr}"
    );

    let received = server.received_requests().await.unwrap();
    assert_eq!(
        received.len(),
        0,
        "F1: expected zero HTTP calls before the guard fires; got {} requests: {:?}",
        received.len(),
        received
            .iter()
            .map(|r| format!("{} {}", r.method, r.url))
            .collect::<Vec<_>>()
    );
}

// ── Step-4.5 Round-3 F1 (render_bulk_component_results partial-failure
//    coverage) ──────────────────────────────────────────────────────────────
//
// `render_bulk_component_results` (src/cli/issue/edit.rs) has an error-row
// branch (`if let Some(err) = op.progress.failed_accessible_issues.get(...)`)
// and an `any_failed` flag that -- when true -- triggers a `bail!` AFTER
// rendering. `await_bulk_task` returns `Err` for FAILED/CANCELLED/DEAD, but
// for terminal `PARTIAL_FAILURE`/`PROCESSED_WITH_ERRORS` it returns
// `Ok(progress)` with `failed_accessible_issues` populated -- flowing INTO
// the render function's error-row branch. Prior to this fix, no test ever
// exercised a PARTIAL_FAILURE poll response, so a mutation deleting
// `any_failed = true` (silent success on a partial failure) survived green.

/// A bulk `--component add:Backend` on 2 keys whose poll terminates
/// `PARTIAL_FAILURE` (FOO-1 succeeds, FOO-2 fails) must: (1) exit non-zero,
/// (2) print the `any_failed` bail message on stderr, and (3) in
/// `--output json` mode, emit a per-key `"status":"error"` row for the
/// failed key alongside a `"status":"success"` row for the succeeded key.
/// This kills the "delete `any_failed = true`" mutation (silent-success on
/// partial failure).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_partial_failure_exits_nonzero_and_renders_error_row()
 {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-comp-partial")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-comp-partial"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::bulk_task_partial_failure(
                "task-comp-partial",
                &["FOO-1"],
                &[("FOO-2", "Permission denied for FOO-2")],
            ),
        ))
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "F1: a PARTIAL_FAILURE poll must exit non-zero (any_failed=true), not 0; stderr={stderr}"
    );
    assert!(
        stderr.contains("One or more issues failed during bulk edit"),
        "F1: expected the any_failed bail message on stderr; stderr={stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("F1: stdout must be valid JSON even on partial failure: {e}; stdout={stdout}")
    });

    let operations = parsed["operations"]
        .as_array()
        .expect("F1: expected top-level 'operations' array");
    assert_eq!(
        operations.len(),
        1,
        "F1: expected exactly one operation (single ADD action); got {operations:?}"
    );
    let results = operations[0]["results"]
        .as_array()
        .expect("F1: expected 'results' array on the operation");

    // Step-4.5 Round-8 coverage: pin the exact row count for this 2-key
    // scenario. `render_bulk_component_results`'s secondary loop renders any
    // `failed_accessible_issues` entry NOT already covered by the primary
    // per-submitted-key loop -- guarded by
    // `!op.keys.iter().any(|k| k == failed_key)`. FOO-2 IS in `op.keys` (the
    // submitted chunk), so it must be rendered exactly ONCE by the primary
    // loop. Relaxing/removing that guard (e.g. mutating `!` away, or the
    // whole condition to `true`) would additionally re-render FOO-2 via the
    // secondary loop, producing a duplicate row -- invisible to a `.find()`
    // lookup (which only ever sees the first match) but caught here.
    assert_eq!(
        results.len(),
        2,
        "F1: expected exactly 2 result rows (FOO-1, FOO-2) with no duplicates; results={results:?}"
    );
    let foo2_count = results.iter().filter(|r| r["key"] == "FOO-2").count();
    assert_eq!(
        foo2_count, 1,
        "F1: FOO-2 is in the submitted chunk and failed -- it must be rendered exactly once by \
         the primary loop, not duplicated by the secondary loop; results={results:?}"
    );

    let foo1 = results
        .iter()
        .find(|r| r["key"] == "FOO-1")
        .unwrap_or_else(|| panic!("F1: expected a result row for FOO-1; results={results:?}"));
    assert_eq!(
        foo1["status"], "success",
        "F1: FOO-1 was in processedAccessibleIssues and must render status=success; row={foo1:?}"
    );

    let foo2 = results
        .iter()
        .find(|r| r["key"] == "FOO-2")
        .unwrap_or_else(|| panic!("F1: expected a result row for FOO-2; results={results:?}"));
    assert_eq!(
        foo2["status"], "error",
        "F1: FOO-2 was in failedAccessibleIssues and must render status=error; row={foo2:?}"
    );
    assert_eq!(
        foo2["error"], "Permission denied for FOO-2",
        "F1: the error row must carry the BulkActionError summary; row={foo2:?}"
    );
}

/// A bulk `--component add:Backend` on 2 keys whose poll response includes a
/// `failedAccessibleIssues` entry for a key that was NOT among the submitted
/// `selectedIssueIdsOrKeys` (FOO-99, while only FOO-1/FOO-2 were submitted)
/// exercises `render_bulk_component_results`'s SECONDARY loop -- `for
/// (failed_key, err) in &op.progress.failed_accessible_issues { if
/// !op.keys.iter().any(|k| k == failed_key) { ... any_failed = true; } }` --
/// distinct from the primary per-submitted-key loop the tests above
/// exercise. No fixture ever placed a failed key outside the submitted set
/// before this test, so the secondary loop's `any_failed = true` was
/// production-reachable but never hit: a mutation deleting it (or replacing
/// the `!op.keys.iter().any(...)` guard with `false`, or deleting the loop
/// entirely) silently exits 0 on an out-of-chunk failure. This is a
/// Step-4.5 Round-7 coverage-only pin: no production behavior is asserted to
/// be correct or desirable here, only pinned so the silent-success mutation
/// is caught.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_out_of_chunk_failed_key_sets_failure() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-comp-oochunk")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-comp-oochunk"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::bulk_task_partial_failure(
                "task-comp-oochunk",
                &["FOO-1", "FOO-2"],
                &[("FOO-99", "Out-of-chunk failure for FOO-99")],
            ),
        ))
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "an out-of-chunk failedAccessibleIssues entry must set any_failed and exit non-zero; \
         status={:?} stderr={stderr}",
        output.status
    );
    assert!(
        stderr.contains("One or more issues failed during bulk edit"),
        "expected the any_failed bail message on stderr; stderr={stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("stdout must be valid JSON even on an out-of-chunk failure: {e}; stdout={stdout}")
    });

    let operations = parsed["operations"]
        .as_array()
        .expect("expected top-level 'operations' array");
    assert_eq!(
        operations.len(),
        1,
        "expected exactly one operation (single ADD action); got {operations:?}"
    );
    let results = operations[0]["results"]
        .as_array()
        .expect("expected 'results' array on the operation");

    let foo1 = results
        .iter()
        .find(|r| r["key"] == "FOO-1")
        .unwrap_or_else(|| panic!("expected a result row for FOO-1; results={results:?}"));
    assert_eq!(
        foo1["status"], "success",
        "FOO-1 was submitted and processed; row={foo1:?}"
    );

    let foo2 = results
        .iter()
        .find(|r| r["key"] == "FOO-2")
        .unwrap_or_else(|| panic!("expected a result row for FOO-2; results={results:?}"));
    assert_eq!(
        foo2["status"], "success",
        "FOO-2 was submitted and processed; row={foo2:?}"
    );

    let foo99 = results
        .iter()
        .find(|r| r["key"] == "FOO-99")
        .unwrap_or_else(|| {
            panic!(
                "expected a result row for FOO-99 -- the out-of-chunk failed key rendered by \
                 the SECONDARY loop; results={results:?}"
            )
        });
    assert_eq!(
        foo99["status"], "error",
        "FOO-99 was in failedAccessibleIssues but not among the submitted chunk keys, so it \
         must render via the secondary loop as status=error; row={foo99:?}"
    );
    assert_eq!(
        foo99["error"], "Out-of-chunk failure for FOO-99",
        "the secondary loop's error row must carry the BulkActionError summary; row={foo99:?}"
    );
}

// ── Step-4.5 Round-8 (render_bulk_component_results row-count coverage) ────
//
// The tests above each exercise the primary loop (submitted key, failed IN
// chunk) or the secondary loop (failed key NOT in the submitted chunk) in
// isolation. Neither one, alone, can distinguish "the secondary loop's guard
// correctly SKIPPED an in-chunk failed key" from "the guard fired anyway and
// produced a duplicate row" -- a `.find()` lookup only ever sees the first
// matching row, so a silently-duplicated FOO-2 entry is invisible to the
// existing per-key assertions. This test submits FOO-1 + FOO-2 and has the
// poll response fail BOTH an in-chunk key (FOO-2, primary loop) AND an
// out-of-chunk key (FOO-99, secondary loop) in the SAME response, then pins
// the total row count. Relaxing the secondary loop's
// `!op.keys.iter().any(|k| k == failed_key)` guard (e.g. dropping the `!`,
// or replacing the whole condition with `true`) would re-render FOO-2 a
// second time via the secondary loop, growing `results.len()` from 3 to 4 --
// caught here, not by any single-key `.find()` assertion.

/// `jr issue edit FOO-1 FOO-2 --component add:Backend` where the poll
/// response reports FOO-1 processed, FOO-2 failed (submitted, so rendered by
/// the PRIMARY loop), and FOO-99 failed (not submitted, so rendered by the
/// SECONDARY loop) in one response must produce exactly 3 result rows --
/// FOO-1 (success), FOO-2 (error, exactly once), FOO-99 (error) -- with a
/// non-zero exit. This is a Step-4.5 Round-8 coverage-only pin: no
/// production behavior is asserted to be correct or desirable here, only
/// pinned so a duplicate-row mutation on the primary/secondary split is
/// caught.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_mixed_in_and_out_of_chunk_failures_no_duplicate_rows()
 {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-comp-mixed")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-comp-mixed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::bulk_task_partial_failure(
                "task-comp-mixed",
                &["FOO-1"],
                &[
                    ("FOO-2", "Permission denied for FOO-2"),
                    ("FOO-99", "Out-of-chunk failure for FOO-99"),
                ],
            ),
        ))
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a mixed in-chunk + out-of-chunk failure must exit non-zero; status={:?} stderr={stderr}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("stdout must be valid JSON on a mixed failure: {e}; stdout={stdout}")
    });

    let operations = parsed["operations"]
        .as_array()
        .expect("expected top-level 'operations' array");
    assert_eq!(operations.len(), 1, "expected exactly one operation");
    let results = operations[0]["results"]
        .as_array()
        .expect("expected 'results' array on the operation");

    assert_eq!(
        results.len(),
        3,
        "expected exactly 3 result rows (FOO-1 success, FOO-2 error, FOO-99 error) with no \
         duplicates; results={results:?}"
    );

    for key in ["FOO-1", "FOO-2", "FOO-99"] {
        let count = results.iter().filter(|r| r["key"] == key).count();
        assert_eq!(
            count, 1,
            "{key} must appear exactly once in results, not duplicated across the primary and \
             secondary render loops; results={results:?}"
        );
    }

    let foo1 = results.iter().find(|r| r["key"] == "FOO-1").unwrap();
    assert_eq!(foo1["status"], "success", "row={foo1:?}");

    let foo2 = results.iter().find(|r| r["key"] == "FOO-2").unwrap();
    assert_eq!(foo2["status"], "error", "row={foo2:?}");
    assert_eq!(foo2["error"], "Permission denied for FOO-2", "row={foo2:?}");

    let foo99 = results.iter().find(|r| r["key"] == "FOO-99").unwrap();
    assert_eq!(foo99["status"], "error", "row={foo99:?}");
    assert_eq!(
        foo99["error"], "Out-of-chunk failure for FOO-99",
        "row={foo99:?}"
    );
}

/// A bulk `--component add:Backend` on 2 keys where the poll response omits
/// FOO-2 from BOTH `processedAccessibleIssues` and `failedAccessibleIssues`
/// (counted only via `invalidOrInaccessibleIssueCount`) must render FOO-2's
/// row as `"status":"inaccessible"` -- not `"success"` and not `"error"` --
/// while FOO-1 (present in `processedAccessibleIssues`) still renders
/// `"status":"success"`. Per BC-3.4.023's current, documented behavior
/// (shared with the pre-existing `render_bulk_edit_results` convention for
/// the `--type`/`--label` paths), an inaccessible key does NOT set
/// `any_failed` -- the overall command still exits 0, with the row itself
/// the only signal. This is a Step-4.5 Round-6 coverage-only pin: no
/// production behavior is asserted to be correct or desirable here, only
/// pinned so a mutation collapsing the "inaccessible" literal to "success"
/// (or deleting the `else if processed.contains(...)` guard) is caught.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_inaccessible_key_renders_inaccessible_status() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::bulk_task_enqueued("task-comp-inaccessible"),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-comp-inaccessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::bulk_task_inaccessible("task-comp-inaccessible", &["FOO-1"], 1),
        ))
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "an inaccessible key must NOT set any_failed -- overall exit must stay 0 (current, \
         documented behavior shared with render_bulk_edit_results); status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("stdout must be valid JSON on an inaccessible-key result: {e}; stdout={stdout}")
    });

    let operations = parsed["operations"]
        .as_array()
        .expect("expected top-level 'operations' array");
    assert_eq!(
        operations.len(),
        1,
        "expected exactly one operation (single ADD action); got {operations:?}"
    );
    let results = operations[0]["results"]
        .as_array()
        .expect("expected 'results' array on the operation");

    let foo1 = results
        .iter()
        .find(|r| r["key"] == "FOO-1")
        .unwrap_or_else(|| panic!("expected a result row for FOO-1; results={results:?}"));
    assert_eq!(
        foo1["status"], "success",
        "FOO-1 was in processedAccessibleIssues and must render status=success; row={foo1:?}"
    );

    let foo2 = results
        .iter()
        .find(|r| r["key"] == "FOO-2")
        .unwrap_or_else(|| panic!("expected a result row for FOO-2; results={results:?}"));
    assert_eq!(
        foo2["status"], "inaccessible",
        "FOO-2 was in neither processedAccessibleIssues nor failedAccessibleIssues and must \
         render status=inaccessible (not success, not error); row={foo2:?}"
    );
}

/// Table-mode companion to the JSON test above: the `else` arm of the table
/// renderer (`status => eprintln!("warning: {key}: {status}")`) is the only
/// place an inaccessible key is surfaced to a human in table mode -- it is
/// neither a `print_success` row nor an `error:` line. Cheap-to-add per
/// Step-4.5 Round-6 (table assertion optional, JSON alone would suffice).
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_inaccessible_key_table_mode_warns() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::bulk_task_enqueued("task-comp-inaccessible-table"),
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/bulk/queue/task-comp-inaccessible-table"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::bulk_task_inaccessible("task-comp-inaccessible-table", &["FOO-1"], 1),
        ))
        .mount(&server)
        .await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "an inaccessible key must NOT set any_failed -- overall exit must stay 0; \
         status={:?} stderr={stderr}",
        output.status
    );
    assert!(
        stderr.contains("warning: FOO-2: inaccessible"),
        "expected a stderr warning naming the inaccessible key in table mode; stderr={stderr}"
    );
}

/// A successful mixed `add:X remove:Y` bulk `--component` edit on 2 keys, in
/// `--output json` mode, must emit the FULL expected structure: an
/// `operations` array with exactly 2 entries (one per action), each with the
/// correct `action` (ADD/REMOVE, in that order), a non-empty `taskId`, and
/// per-key `results[].status == "success"` for every key. This kills
/// mutations that drop an operation from `operations_json` or change the
/// `"status":"success"` literal.
#[tokio::test]
async fn test_bc_3_4_023_issue_edit_bulk_component_success_renders_full_json_structure() {
    let server = MockServer::start().await;

    s605_1_mock_components(
        &server,
        "FOO",
        vec![
            common::fixtures::component_response("10001", "Backend", None, None, None),
            common::fixtures::component_response("10002", "Frontend", None, None, None),
        ],
    )
    .await;

    let keys = vec!["FOO-1".to_string(), "FOO-2".to_string()];

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "editedFieldsInput": {
                "multiselectComponents": {
                    "bulkEditMultiSelectFieldOption": "ADD"
                }
            }
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::bulk_task_enqueued("task-comp-full-add")),
        )
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_complete(&server, "task-comp-full-add", &keys).await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/bulk/issues/fields"))
        .and(body_partial_json(serde_json::json!({
            "editedFieldsInput": {
                "multiselectComponents": {
                    "bulkEditMultiSelectFieldOption": "REMOVE"
                }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::bulk_task_enqueued("task-comp-full-remove"),
        ))
        .expect(1)
        .mount(&server)
        .await;
    s605_2_mount_poll_complete(&server, "task-comp-full-remove", &keys).await;

    let output = s605_1_cmd(&server.uri())
        .args([
            "--no-input",
            "--output",
            "json",
            "issue",
            "edit",
            "FOO-1",
            "FOO-2",
            "--component",
            "add:Backend",
            "--component",
            "remove:Frontend",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "F1: expected exit 0 on a fully-successful mixed edit; stderr={stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("F1: stdout must be one parseable JSON document: {e}; stdout={stdout}")
    });

    let operations = parsed["operations"]
        .as_array()
        .expect("F1: expected top-level 'operations' array");
    assert_eq!(
        operations.len(),
        2,
        "F1: expected exactly 2 operations (ADD then REMOVE); got {operations:?}"
    );

    assert_eq!(
        operations[0]["action"], "ADD",
        "F1: first operation's action must be ADD; op={:?}",
        operations[0]
    );
    assert_eq!(
        operations[1]["action"], "REMOVE",
        "F1: second operation's action must be REMOVE; op={:?}",
        operations[1]
    );

    for (idx, op) in operations.iter().enumerate() {
        let task_id = op["taskId"]
            .as_str()
            .unwrap_or_else(|| panic!("F1: operation {idx} must have a string taskId; op={op:?}"));
        assert!(
            !task_id.is_empty(),
            "F1: operation {idx}'s taskId must be non-empty; op={op:?}"
        );

        let results = op["results"].as_array().unwrap_or_else(|| {
            panic!("F1: operation {idx} must have a 'results' array; op={op:?}")
        });
        assert_eq!(
            results.len(),
            2,
            "F1: operation {idx} must have exactly 2 per-key result rows (FOO-1, FOO-2); op={op:?}"
        );
        for key in ["FOO-1", "FOO-2"] {
            let row = results.iter().find(|r| r["key"] == key).unwrap_or_else(|| {
                panic!("F1: operation {idx} missing a result row for {key}; op={op:?}")
            });
            assert_eq!(
                row["status"], "success",
                "F1: operation {idx}'s {key} row must be status=success; row={row:?}"
            );
        }
    }
}

// ─── S-575-1: `--fields <CSV>` on `issue list` / `issue view` (Red Gate) ───
//
// BC-2.2.033 (list), BC-2.3.041 (view), BC-2.6.052 (additive client
// methods). Covers the REPLACE-semantics request composition (not UNION),
// the typed-output null/extra-flatten postconditions, `key`-always-present,
// CSV whitespace-trimming, and the new client methods' verbatim
// pass-through. Table-mode rejection and empty-CSV pre-HTTP rejection are
// covered separately in tests/issue_list_errors.rs and
// tests/issue_view_errors.rs; the `--points` silent-no-op interaction is
// covered in tests/all_flag_behavior.rs. AC-009 (the 10 existing
// get_issue/search_issues call sites are unaffected) is explicitly
// "no new test — verified via full regression suite" per the story, so no
// dedicated test is added here for it.

/// AC-001 / BC-2.2.033 Postcondition 1: `--fields` on `issue list` REPLACES
/// `BASE_ISSUE_FIELDS` entirely — the `fields` array sent to
/// `POST /rest/api/3/search/jql` must be EXACTLY the requested, trimmed,
/// comma-joined CSV, in supplied order — no union, no config-driven extras.
#[tokio::test]
async fn test_bc_2_2_033_issue_list_fields_replaces_requested_field_set() {
    let server = MockServer::start().await;

    // F2 (adversary review, S-575-1): match on method+path only. The old
    // `body_partial_json` matcher was INCLUSIVE (assert-json-diff) — it
    // ignores trailing actual elements, so a future append-union regression
    // (e.g. re-injecting a config-driven extra field onto the end of the
    // array) would still satisfy it. The EXACT full-array assertion — the
    // one that actually guards BC-2.2.033 Postcondition 1's REPLACE-not-UNION
    // invariant — happens below via `server.received_requests()`.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![serde_json::json!({
                "key": "PROJ-1",
                "fields": {
                    "summary": "Narrow fields issue",
                    "status": {"name": "To Do"},
                    "comment": {"comments": []}
                }
            })]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--jql",
            "project = PROJ",
            "--fields",
            "summary,status,comment",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = output.stdout.clone();
    let parsed: serde_json::Value = serde_json::from_slice(&stdout).unwrap_or_else(|e| {
        panic!(
            "stdout not valid JSON: {e}\n{}",
            String::from_utf8_lossy(&stdout)
        )
    });
    assert_eq!(parsed[0]["key"], "PROJ-1");
    assert_eq!(parsed[0]["fields"]["summary"], "Narrow fields issue");

    let requests = server.received_requests().await.expect("requests recorded");
    let search_request = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("search POST must have been made");
    let body: serde_json::Value =
        serde_json::from_slice(&search_request.body).expect("request body must be valid JSON");
    assert_eq!(
        body["fields"],
        serde_json::json!(["summary", "status", "comment"]),
        "fields array must be EXACTLY [\"summary\", \"status\", \"comment\"] \
         (REPLACE semantics, no union) — got: {}",
        body["fields"]
    );
}

/// AC-002 / BC-2.3.041 Postcondition 1: `--fields` on `issue view` mirrors
/// the list twin exactly — the `fields=` query param sent to
/// `GET /rest/api/3/issue/<KEY>` (via the new `get_issue_with_fields`
/// client method, BC-2.6.052) must be EXACTLY the requested CSV.
#[tokio::test]
async fn test_bc_2_3_041_issue_view_fields_replaces_requested_field_set() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-5"))
        .and(query_param("fields", "summary,comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ-5",
            "fields": {
                "summary": "View narrow fields",
                "comment": {"comments": []}
            }
        })))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "view",
            "PROJ-5",
            "--fields",
            "summary,comment",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed["key"], "PROJ-5");
    assert_eq!(parsed["fields"]["summary"], "View narrow fields");
}

/// AC-003 / BC-2.2.033 Postcondition 2: named `IssueFields` struct fields
/// NOT covered by the `--fields` request serialize as JSON `null`
/// (missing-key -> `None`, standard serde `Option<T>` behavior); unnamed
/// requested fields (e.g. a `customfield_NNNNN`) flow through
/// `IssueFields.extra` (`#[serde(flatten)]`) verbatim.
#[tokio::test]
async fn test_bc_2_2_033_issue_list_fields_unrequested_named_fields_are_null() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(serde_json::json!({
            "fields": ["summary", "status", "customfield_10084"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![serde_json::json!({
                "key": "PROJ-1",
                "fields": {
                    "summary": "Narrow fields issue",
                    "status": {"name": "To Do"},
                    "customfield_10084": "custom-value-xyz"
                }
            })]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--jql",
            "project = PROJ",
            "--fields",
            "summary,status,customfield_10084",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    let issue = &parsed[0];
    assert_eq!(issue["fields"]["summary"], "Narrow fields issue");
    assert!(
        issue["fields"]["priority"].is_null(),
        "unrequested named field 'priority' must serialize as null, got: {issue}"
    );
    assert!(
        issue["fields"]["assignee"].is_null(),
        "unrequested named field 'assignee' must serialize as null, got: {issue}"
    );
    assert!(
        issue["fields"]["duedate"].is_null(),
        "unrequested named field 'duedate' must serialize as null, got: {issue}"
    );
    assert_eq!(
        issue["fields"]["customfield_10084"], "custom-value-xyz",
        "unnamed requested field must round-trip verbatim through the extra flatten, got: {issue}"
    );
}

/// AC-007 / BC-2.2.033 Postcondition 3: `key` is present in `issue list`
/// JSON output regardless of whether `key` appears in the `--fields` CSV —
/// Jira always returns it top-level. (The typed `Issue` struct has no
/// top-level `id` field — only `key` is part of this contract; BC-2.2.033
/// Postcondition 3 names `key` exclusively.)
#[tokio::test]
async fn test_bc_2_2_033_issue_list_fields_key_always_present_regardless_of_csv() {
    let server = MockServer::start().await;

    // F2 (adversary review, S-575-1): method+path only — see the exact
    // full-array assertion on `server.received_requests()` below, which is
    // what actually guards REPLACE-not-UNION here (a `body_partial_json`
    // match against `["summary"]` would not catch a trailing append).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![serde_json::json!({
                "key": "PROJ-7",
                "fields": { "summary": "Key-only fields request" }
            })]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--jql",
            "project = PROJ",
            "--fields",
            "summary",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert_eq!(
        parsed[0]["key"], "PROJ-7",
        "'key' must be present at issue top level even though 'key' does not \
         appear in the --fields CSV"
    );

    let requests = server.received_requests().await.expect("requests recorded");
    let search_request = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("search POST must have been made");
    let body: serde_json::Value =
        serde_json::from_slice(&search_request.body).expect("request body must be valid JSON");
    assert_eq!(
        body["fields"],
        serde_json::json!(["summary"]),
        "fields array must be EXACTLY [\"summary\"] (REPLACE semantics, no \
         union) — got: {}",
        body["fields"]
    );
}

/// AC-012 / BC-2.3.041 Postcondition 3: `key` is present in `issue view`
/// JSON output regardless of `--fields` CSV contents — same Jira guarantee
/// as AC-007, verified independently on the view path.
#[tokio::test]
async fn test_bc_2_3_041_issue_view_fields_key_always_present() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-8"))
        .and(query_param("fields", "summary"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ-8",
            "fields": { "summary": "View key-only fields request" }
        })))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue", "view", "PROJ-8", "--fields", "summary", "--output", "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert_eq!(
        parsed["key"], "PROJ-8",
        "'key' must be present at issue top level even though 'key' does not \
         appear in the --fields CSV"
    );
}

/// AC-008 / BC-2.2.033 Edge Case EC-2.2.033-2: `--fields "summary, status"`
/// (embedded whitespace around the comma) behaves identically to
/// `--fields "summary,status"` — each CSV segment is trimmed before use.
#[tokio::test]
async fn test_bc_2_2_033_issue_list_fields_csv_segments_are_trimmed() {
    let server = MockServer::start().await;

    // F2 (adversary review, S-575-1): method+path only — the exact
    // full-array assertion below on `server.received_requests()` is what
    // actually proves BOTH trimming ("summary, status" -> "summary"/"status"
    // with no embedded whitespace) AND REPLACE-not-UNION (no trailing
    // append); a `body_partial_json` inclusive match can't distinguish
    // either from a superset.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![serde_json::json!({
                "key": "PROJ-9",
                "fields": {
                    "summary": "Trimmed CSV",
                    "status": {"name": "To Do"}
                }
            })]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--jql",
            "project = PROJ",
            "--fields",
            "summary, status",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "embedded whitespace in --fields CSV must be trimmed before use, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed[0]["key"], "PROJ-9");

    let requests = server.received_requests().await.expect("requests recorded");
    let search_request = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("search POST must have been made");
    let body: serde_json::Value =
        serde_json::from_slice(&search_request.body).expect("request body must be valid JSON");
    assert_eq!(
        body["fields"],
        serde_json::json!(["summary", "status"]),
        "fields array must be EXACTLY [\"summary\", \"status\"] — trimmed, \
         REPLACE semantics, no union — got: {}",
        body["fields"]
    );
}

/// AC-010 / BC-2.6.052 Postcondition 2: the new field-override client
/// methods (`get_issue_with_fields`, `search_issues_with_fields`) send the
/// caller-supplied field list EXACTLY — comma-joined for the GET query
/// param, as a JSON array for the POST body — with no `BASE_ISSUE_FIELDS`
/// union. Exercised at the client layer directly (not via the CLI) so this
/// pins the method contract independent of the CLI-layer wiring in
/// list.rs/view.rs.
#[tokio::test]
async fn test_bc_2_6_052_field_override_methods_send_verbatim_field_list() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .and(query_param("fields", "summary,comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ-1",
            "fields": { "summary": "Narrow", "comment": {"comments": []} }
        })))
        .mount(&server)
        .await;

    // F2 (adversary review, S-575-1): method+path only — the old
    // `body_partial_json` matcher was inclusive and could not detect a
    // trailing append-union regression. The exact full-array assertion
    // below on `server.received_requests()` is the actual guard for
    // BC-2.6.052 Postcondition 2's verbatim-pass-through contract.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![serde_json::json!({
                "key": "PROJ-2",
                "fields": { "summary": "Narrow2", "comment": {"comments": []} }
            })]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let issue = client
        .get_issue_with_fields("PROJ-1", &["summary", "comment"])
        .await
        .expect("get_issue_with_fields must send exactly the caller-supplied fields");
    assert_eq!(issue.key, "PROJ-1");

    let result = client
        .search_issues_with_fields("project = PROJ", Some(10), &["summary", "comment"])
        .await
        .expect("search_issues_with_fields must send exactly the caller-supplied fields");
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].key, "PROJ-2");

    let requests = server.received_requests().await.expect("requests recorded");
    let search_request = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("search POST must have been made");
    let body: serde_json::Value =
        serde_json::from_slice(&search_request.body).expect("request body must be valid JSON");
    assert_eq!(
        body["fields"],
        serde_json::json!(["summary", "comment"]),
        "search_issues_with_fields must send fields EXACTLY [\"summary\", \"comment\"] \
         with no BASE_ISSUE_FIELDS union — got: {}",
        body["fields"]
    );
}

/// AC-010 / BC-2.6.052 Edge Case EC-2.6.052-1: an empty field slice reaching
/// the new client method(s) is NOT a client-layer error — the method is a
/// thin, unvalidated pass-through; CLI-layer pre-HTTP validation
/// (BC-2.2.033/BC-2.3.041 Precondition 3) is the sole enforcement point for
/// a non-empty field list.
#[tokio::test]
async fn test_bc_2_6_052_field_override_methods_empty_slice_is_not_a_client_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .and(query_param("fields", ""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ-1",
            "fields": { "summary": "Empty fields slice" }
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let issue = client
        .get_issue_with_fields("PROJ-1", &[])
        .await
        .expect("an empty field slice must not be rejected at the client layer");
    assert_eq!(issue.key, "PROJ-1");
}

/// F1 (adversary review, S-575-1): a user-supplied field name containing a
/// URL-significant character (`&`, `#`, or a literal space) must be
/// percent-encoded PER SEGMENT before being joined into the `fields` query
/// param on `get_issue_with_fields`'s GET request — otherwise the raw
/// character corrupts the query string (e.g. an unescaped `&` starts a new
/// query param, silently dropping every field after it). This asserts the
/// RAW (still-encoded) query string on the wire, not just the
/// wiremock-decoded `query_param` matcher, so a regression that stops
/// encoding would fail this test even though a naive decoded-value
/// comparison might still appear to match.
#[tokio::test]
async fn test_get_issue_with_fields_url_encodes_special_characters_in_field_names() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-1"))
        .and(query_param("fields", "summary,field&name,with space"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ-1",
            "fields": { "summary": "Encoded fields" }
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let issue = client
        .get_issue_with_fields("PROJ-1", &["summary", "field&name", "with space"])
        .await
        .expect("special characters in field names must not break the request");
    assert_eq!(issue.key, "PROJ-1");

    // Inspect the RAW (still percent-encoded) query string actually sent on
    // the wire — this is the assertion that would catch a regression back to
    // unencoded `fields.join(",")`, since an unencoded `&`/space would still
    // happen to round-trip through wiremock's decoded `query_param` matcher
    // above in some cases but would corrupt a real Jira request.
    let requests = server.received_requests().await.expect("requests recorded");
    let request = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/issue/PROJ-1")
        .expect("GET request must have been made");
    let raw_query = request
        .url
        .query()
        .expect("request must carry a query string");

    assert!(
        raw_query.contains("field%26name"),
        "literal '&' in a field name must be percent-encoded as %26 on the wire; got: {raw_query}"
    );
    assert!(
        !raw_query.contains("field&name"),
        "an unencoded '&' would start a new query param and corrupt the fields list; got: {raw_query}"
    );
    assert!(
        raw_query.contains("with%20space") || raw_query.contains("with+space"),
        "literal space in a field name must be percent-encoded on the wire; got: {raw_query}"
    );

    // Decoded round-trip: the three original field names, and only those
    // three, must survive encode -> Jira-side decode.
    let fields_query = request
        .url
        .query_pairs()
        .find(|(k, _)| k == "fields")
        .map(|(_, v)| v.into_owned())
        .expect("request must carry a `fields` query parameter");
    assert_eq!(fields_query, "summary,field&name,with space");
}

/// Adversary Pass 5 regression: `IssueFields.summary` was the only
/// non-`Option` typed field on `IssueFields` — every sibling is
/// `Option<...>`. Under REPLACE semantics a `--fields` CSV that omits
/// `summary` (e.g. `--fields status`) makes Jira's response `.fields` omit
/// the `summary` key entirely, which previously failed deserialization with
/// `missing field \`summary\`` and errored the command instead of returning
/// the requested projection. `summary` is now `Option<String>` like every
/// other named field; this pins `jr issue list --fields status` succeeding
/// with `summary` absent/null in the mocked response.
#[tokio::test]
async fn test_issue_list_fields_omitting_summary_does_not_error() {
    let server = MockServer::start().await;

    // Reproduces live Jira: the `fields` array requested is `["status"]`
    // only, so the mocked `.fields` response object OMITS `summary`
    // entirely — this is the exact shape that previously broke
    // deserialization.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![serde_json::json!({
                "key": "PROJ-11",
                "fields": {
                    "status": {"name": "To Do"}
                }
            })]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--jql",
            "project = PROJ",
            "--fields",
            "status",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "issue list --fields status (summary omitted from the CSV and from \
         Jira's response) must not error, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed[0]["key"], "PROJ-11");
    assert_eq!(parsed[0]["fields"]["status"]["name"], "To Do");
    assert!(
        parsed[0]["fields"]["summary"].is_null(),
        "summary must serialize as null when omitted from the --fields CSV, got: {}",
        parsed[0]
    );
}

/// View-path twin of `test_issue_list_fields_omitting_summary_does_not_error`
/// — `jr issue view <KEY> --fields status` must not error when Jira's
/// response `.fields` omits `summary`.
#[tokio::test]
async fn test_issue_view_fields_omitting_summary_does_not_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-12"))
        .and(query_param("fields", "status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ-12",
            "fields": {
                "status": {"name": "In Progress"}
            }
        })))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue", "view", "PROJ-12", "--fields", "status", "--output", "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "issue view --fields status (summary omitted from the CSV and from \
         Jira's response) must not error, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed["key"], "PROJ-12");
    assert_eq!(parsed["fields"]["status"]["name"], "In Progress");
    assert!(
        parsed["fields"]["summary"].is_null(),
        "summary must serialize as null when omitted from the --fields CSV, got: {parsed}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// S-584-1: Preserve raw ADF for `--fields comment`
// (BC-2.2.034, BC-2.3.042)
// ═══════════════════════════════════════════════════════════════════════
//
// CONFIRMATORY story -- the raw-ADF-passthrough behavior already exists via
// `IssueFields.extra`'s `#[serde(flatten)]` catch-all (S-575-1's `--fields`
// REPLACE-semantics mechanism, BC-2.2.033/BC-2.3.041). These 4 tests are
// expected to pass GREEN immediately with zero src/ changes -- there is no
// fail-first Red Gate for this story, unlike most stories in this file. If
// any of these 4 tests were to go RED, that would mean a real behavioral
// gap exists and the story is no longer purely confirmatory.
//
// AC-001/AC-002 assert raw-ADF deep-equality on `issue list`/`issue view
// --fields comment --output json`. AC-003/AC-004 are negative regression
// tests confirming the two *other* pre-existing `adf_to_text` call sites
// (`issue comments`, and `issue view`'s table-mode description row) remain
// fully independent and unaffected by the new `--fields comment` path.

/// A non-trivial ADF comment body -- a paragraph containing a `strong`-marked
/// text run, followed by a two-item bullet list -- used as the canonical
/// fixture across AC-001/AC-002/AC-003 so the raw-ADF deep-equality
/// assertions (AC-001/AC-002) and the flattened-plain-text assertion
/// (AC-003) are checking the SAME underlying content through the two
/// independent code paths (`extra` passthrough vs. `adf_to_text`).
fn s584_1_fixture_comment_adf() -> serde_json::Value {
    serde_json::json!({
        "type": "doc",
        "version": 1,
        "content": [
            {
                "type": "paragraph",
                "content": [
                    {"type": "text", "text": "Testing "},
                    {"type": "text", "text": "STRONGWORD", "marks": [{"type": "strong"}]},
                    {"type": "text", "text": " marker."}
                ]
            },
            {
                "type": "bulletList",
                "content": [
                    {
                        "type": "listItem",
                        "content": [
                            {"type": "paragraph", "content": [{"type": "text", "text": "AlphaItem"}]}
                        ]
                    },
                    {
                        "type": "listItem",
                        "content": [
                            {"type": "paragraph", "content": [{"type": "text", "text": "BetaItem"}]}
                        ]
                    }
                ]
            }
        ]
    })
}

/// AC-001 / BC-2.2.034 Postcondition 1: `jr issue list --fields comment
/// --output json` returns `.fields.comment.comments[].body` as the raw ADF
/// object Jira returned -- deep-equal to the fixture's non-trivial ADF,
/// never a flattened plain-text string.
#[tokio::test]
async fn test_bc_2_2_034_issue_list_fields_comment_returns_raw_adf() {
    let server = MockServer::start().await;
    let adf = s584_1_fixture_comment_adf();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![serde_json::json!({
                "key": "PROJ-584",
                "fields": {
                    "summary": "Raw ADF comment fixture",
                    "comment": {
                        "comments": [
                            {
                                "id": "20001",
                                "author": {
                                    "accountId": "abc",
                                    "displayName": "Alice",
                                    "active": true
                                },
                                "body": adf,
                                "created": "2026-08-21T10:00:00.000+0000"
                            }
                        ]
                    }
                }
            })]),
        ))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "list",
            "--jql",
            "project = PROJ",
            "--fields",
            "summary,comment",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    let body = parsed[0]["fields"]["comment"]["comments"][0]["body"].clone();
    assert_eq!(
        body,
        s584_1_fixture_comment_adf(),
        "AC-001: .fields.comment.comments[].body must deep-equal the raw ADF \
         fixture, never a flattened string -- got: {body}"
    );
    assert_eq!(
        body["type"], "doc",
        "AC-001: body must be a raw ADF object with type=doc, got: {body}"
    );
}

/// AC-002 / BC-2.3.042 Postcondition 1: `jr issue view <KEY> --fields
/// comment --output json` returns `.fields.comment.comments[].body` as the
/// same raw ADF object, via the same fixture -- the `issue view` twin of
/// AC-001.
#[tokio::test]
async fn test_bc_2_3_042_issue_view_fields_comment_returns_raw_adf() {
    let server = MockServer::start().await;
    let adf = s584_1_fixture_comment_adf();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-584"))
        .and(query_param("fields", "summary,comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ-584",
            "fields": {
                "summary": "Raw ADF comment fixture",
                "comment": {
                    "comments": [
                        {
                            "id": "20001",
                            "author": {
                                "accountId": "abc",
                                "displayName": "Alice",
                                "active": true
                            },
                            "body": adf,
                            "created": "2026-08-21T10:00:00.000+0000"
                        }
                    ]
                }
            }
        })))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "issue",
            "view",
            "PROJ-584",
            "--fields",
            "summary,comment",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    let body = parsed["fields"]["comment"]["comments"][0]["body"].clone();
    assert_eq!(
        body,
        s584_1_fixture_comment_adf(),
        "AC-002: .fields.comment.comments[].body must deep-equal the raw ADF \
         fixture, never a flattened string -- got: {body}"
    );
    assert_eq!(
        body["type"], "doc",
        "AC-002: body must be a raw ADF object with type=doc, got: {body}"
    );
}

/// AC-003 / BC-2.2.034 Postcondition 2: `jr issue comments <KEY>` -- the
/// PRE-EXISTING, unrelated command -- run against the SAME fixture issue
/// still renders plain text via `adf::adf_to_text`, confirming the new
/// `--fields comment` raw-ADF path (AC-001) and the pre-existing `issue
/// comments` flattening path do not regress each other. Negative
/// regression test: if `issue comments` output ever contained raw ADF
/// (e.g. because a future maintainer wired `extra`-passthrough into it, or
/// vice versa), this test would fail.
#[tokio::test]
async fn test_bc_2_2_034_issue_comments_command_unaffected_by_fields_comment_path() {
    let server = MockServer::start().await;
    let adf = s584_1_fixture_comment_adf();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-584/comment"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "100"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [
                {
                    "id": "20001",
                    "author": {
                        "accountId": "abc",
                        "displayName": "Alice",
                        "active": true
                    },
                    "body": adf,
                    "created": "2026-08-21T10:00:00.000+0000"
                }
            ],
            "startAt": 0,
            "maxResults": 100,
            "total": 1
        })))
        .mount(&server)
        .await;

    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "issue", "comments", "PROJ-584"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("STRONGWORD")
            && stdout.contains("AlphaItem")
            && stdout.contains("BetaItem"),
        "AC-003: issue comments must still flatten the ADF body to plain \
         text via adf_to_text, unaffected by the --fields comment raw-ADF \
         path (AC-001) -- stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("\"type\":") && !stdout.contains("\"type\" :"),
        "AC-003: issue comments output must never contain raw ADF JSON -- \
         stdout:\n{stdout}"
    );
}

/// AC-004 / BC-2.3.042 Edge Case EC-2.3.042-2: `issue
/// view`'s table-mode description row (the one existing `adf_to_text` call
/// site inside `view.rs::handle_view`) is unaffected by this story.
/// `--fields` is JSON-only (BC-2.3.041 Precondition 2), so combining
/// `--fields comment` with default (table) output never reaches that code
/// path at all -- it exits 64 pre-HTTP instead. Separately, a plain `issue
/// view` (no `--fields`) confirms the table-mode description-row
/// `adf_to_text` call site itself is still intact and functioning.
#[tokio::test]
async fn test_bc_2_3_042_view_table_mode_description_render_unaffected() {
    let server = MockServer::start().await;

    // Part 1: `--fields comment` without `--output json` (default table
    // mode) must exit 64 PRE-HTTP -- the table-mode description-row code
    // path is never reached in this combination.
    let output = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args([
            "--no-input",
            "issue",
            "view",
            "PROJ-584",
            "--fields",
            "summary,comment",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-004 part 1: --fields without --output json must exit 64, \
         stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        server
            .received_requests()
            .await
            .expect("requests recorded")
            .is_empty(),
        "AC-004 part 1: --fields + table-mode rejection must be pre-HTTP -- \
         zero requests expected"
    );

    // Part 2: the table-mode description row's `adf_to_text` call site is
    // untouched -- a plain `issue view` (no `--fields`) still flattens a
    // description ADF to plain text.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-585"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PROJ-585",
            "fields": {
                "summary": "Table mode description check",
                "status": {"name": "To Do"},
                "description": {
                    "type": "doc",
                    "version": 1,
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [{"type": "text", "text": "PLAINDESC"}]
                        }
                    ]
                }
            }
        })))
        .mount(&server)
        .await;

    let output2 = assert_cmd::Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["--no-input", "issue", "view", "PROJ-585"])
        .output()
        .unwrap();
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        output2.status.success(),
        "AC-004 part 2: stderr: {}",
        String::from_utf8_lossy(&output2.stderr)
    );
    assert!(
        stdout2.contains("PLAINDESC"),
        "AC-004 part 2: table-mode description row must still flatten the \
         description ADF via adf_to_text -- stdout:\n{stdout2}"
    );
}

/// ADV-LRE-F5-C-LOW-002 (Lens C cross-story combined-flag integration test):
/// `jr issue list --jql "<scope>" --fields "summary,comment" --sort
/// "updated:desc" --output json` proves `--fields` (S-575-1/BC-2.2.033) and
/// `--sort` (S-588-1/BC-2.1.025) compose end-to-end in a single invocation
/// -- the outgoing search request carries BOTH the `--fields` projection
/// (`summary`, `comment`) AND the `--jql`-derived base clause with the
/// `--sort`-composed `ORDER BY updated DESC, key ASC` fragment appended.
#[tokio::test]
async fn test_issue_list_fields_and_sort_compose_end_to_end() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_search_response(vec![serde_json::json!({
                "key": "PROJ-585",
                "fields": {
                    "summary": "Combined --fields + --sort fixture",
                    "comment": { "comments": [] }
                }
            })]),
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
            "--fields",
            "summary,comment",
            "--sort",
            "updated:desc",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let received = server.received_requests().await.unwrap();
    let search_req = received
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/search/jql")
        .expect("search/jql request must have fired");
    let body: serde_json::Value =
        serde_json::from_slice(&search_req.body).expect("body must be valid JSON");

    let fields = body["fields"]
        .as_array()
        .expect("fields must be an array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        vec!["summary", "comment"],
        "the outgoing request's fields projection must be exactly the \
         --fields list, got: {fields:?}"
    );

    let jql = body["jql"].as_str().expect("jql field must be a string");
    assert!(
        jql.contains("ORDER BY updated DESC, key ASC"),
        "the outgoing request's JQL must carry the --sort-composed \
         ORDER BY fragment, got: {jql}"
    );
    assert!(
        jql.contains("project = PROJ"),
        "the outgoing request's JQL must still carry the --jql-supplied \
         base clause, got: {jql}"
    );

    // Confirm the JSON output itself carries the projected `comment` field
    // (proving --fields actually reached the field-projection path, not
    // just the request body).
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert!(
        parsed[0]["fields"]["comment"].is_object(),
        "expected the projected 'comment' field in JSON output, got: {parsed}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// S-579-1: `jr issue list --updated-recent <duration>` filter
// (BC-2.1.023, amended BC-2.1.006/BC-2.1.007)
// ═══════════════════════════════════════════════════════════════════════
//
// Red Gate history: this block was authored RED-first during S-579-1 --
// `--updated-recent` started as a compilable stub in
// `src/cli/issue/list.rs::handle_list` (`if updated_recent.is_some() {
// todo!("S-579-1: --updated-recent") }`), so every test below failed
// (panic/exit 101, or an assertion mismatch against the pre-amendment
// 14-source stderr enumeration) until the real clause-composition logic
// landed. The stub has since been implemented, and, per DEC-306/FIX-F5-LRE-1,
// the alone-case now proceeds rather than exiting 64 (see AC-007 below) --
// every test in this block passes today. Reuses the S-606-1 harness helpers (`s606_1_cmd`,
// `s606_1_mock_project_exists`, `s606_1_mock_search_empty`,
// `s606_1_composed_jql`, `s606_1_expect_zero_http`) defined above in this
// same file — same isolated-cache/config pattern, same
// `MockServer::received_requests()` JQL-capture technique.
//
// 8 acceptance-criteria tests (AC-001..008), one per story AC:
//   - AC-001: clause composition (`updated >= -{d}`)
//   - AC-002: pre-HTTP combined-units rejection via shared `jql::validate_duration`
//   - AC-003: asymmetric `conflicts_with` (--updated-after only, not --updated-before)
//   - AC-004: free composition with --recent (recent's clause precedes updated-recent's)
//   - AC-005: BC-2.1.007 stable-order position (after recent, before asset)
//   - AC-006: BC-2.1.006 15-source "no filters" stderr enumeration
//   - AC-007: --updated-recent alone (FIX-F5-LRE-1, ADV-LRE-F5-A-MED-001):
//     proceeds to a query exactly like `--recent` alone — see AC-007's
//     current test for the amended assertion; the dedicated
//     `--updated-recent`-alone exit-64 guard this AC originally exercised
//     has been removed by human adjudication.
//   - AC-008: field-swap fidelity (`updated`, not `created`)

/// AC-001 / BC-2.1.023 Postcondition 1: `--updated-recent 60d` composes the
/// clause `updated >= -60d` — the direct field-swapped analogue of
/// `--recent`'s `created >= -{d}` template.
#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_clause() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--updated-recent",
            "60d",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-001: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("updated >= -60d"),
        "AC-001: expected clause 'updated >= -60d' in composed JQL, got: {jql}"
    );
}

/// AC-002 / BC-2.1.023 Precondition 2: `--updated-recent` is validated via
/// the SAME `jql::validate_duration` validator `--recent` uses — combined
/// units (`4w2d`) are rejected pre-HTTP with the identical error shape
/// BC-2.1.008 pins for `--recent`, and zero HTTP calls are issued.
#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_rejects_combined_units_pre_http() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_expect_zero_http(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--updated-recent",
            "4w2d",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "AC-002: expected failure on combined-unit duration, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-002: invalid --updated-recent duration should exit 64 (UserError), \
         got: {:?} (stderr: {stderr})",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "Invalid duration '4w2d'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M)."
        ),
        "AC-002: expected the shared jql::validate_duration error shape in stderr, got: {stderr}"
    );
}

/// AC-003 / BC-2.1.023 Edge Case EC-2.1.023-2 (human-locked DEC-298):
/// `--updated-recent` + `--updated-after` is a clap `conflicts_with`
/// rejection (exit 2). `--updated-recent` + `--updated-before` does NOT
/// conflict — deliberate asymmetry mirroring the pre-existing
/// `--recent` x `--created-after` pattern, not a bug to "fix".
#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_conflicts_with_updated_after_only() {
    // (a) --updated-recent + --updated-after -> clap conflict, exit 2,
    // native clap-level rejection (no HTTP, no handler code reached). Given
    // its OWN MockServer (not shared with part (b)'s mounted mocks) so the
    // "no HTTP" claim is actually asserted, not just stated in prose
    // (pr-review cycle 1 Finding 4).
    let server_conflict = MockServer::start().await;
    let cache_dir1 = tempfile::tempdir().unwrap();
    let config_dir1 = tempfile::tempdir().unwrap();
    s606_1_expect_zero_http(&server_conflict).await;

    let output_conflict = s606_1_cmd(
        &server_conflict.uri(),
        cache_dir1.path(),
        config_dir1.path(),
    )
    .args([
        "--no-input",
        "issue",
        "list",
        "--project",
        "FOO",
        "--updated-recent",
        "60d",
        "--updated-after",
        "2026-01-01",
    ])
    .output()
    .unwrap();
    assert_eq!(
        output_conflict.status.code(),
        Some(2),
        "AC-003a: --updated-recent + --updated-after must be a clap conflict \
         (exit 2), got: {:?} (stderr: {})",
        output_conflict.status.code(),
        String::from_utf8_lossy(&output_conflict.stderr)
    );

    // (b) --updated-recent + --updated-before -> NO conflict, composes both
    // clauses successfully (exit 0). This is the half that exercises the
    // clause-composition logic (Red Gate-authored during S-579-1, since
    // implemented). Its own MockServer, distinct from (a)'s.
    let server_no_conflict = MockServer::start().await;
    let cache_dir2 = tempfile::tempdir().unwrap();
    let config_dir2 = tempfile::tempdir().unwrap();
    s606_1_mock_project_exists(&server_no_conflict, "FOO").await;
    s606_1_mock_search_empty(&server_no_conflict).await;

    let output_no_conflict = s606_1_cmd(
        &server_no_conflict.uri(),
        cache_dir2.path(),
        config_dir2.path(),
    )
    .args([
        "--no-input",
        "issue",
        "list",
        "--project",
        "FOO",
        "--updated-recent",
        "60d",
        "--updated-before",
        "2026-01-01",
    ])
    .output()
    .unwrap();
    assert!(
        output_no_conflict.status.success(),
        "AC-003b: --updated-recent + --updated-before must NOT conflict \
         (asymmetric per DEC-298), expected exit 0, got: {:?} (stderr: {})",
        output_no_conflict.status.code(),
        String::from_utf8_lossy(&output_no_conflict.stderr)
    );
}

/// AC-004 / BC-2.1.023 Postcondition 3 / Edge Case EC-2.1.023-3:
/// `--updated-recent 30d --recent 30d` composes BOTH clauses, AND-joined,
/// with `recent`'s clause emitted before `updated-recent`'s (per
/// BC-2.1.007's stable order) — no error.
#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_freely_with_recent() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--updated-recent",
            "30d",
            "--recent",
            "30d",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-004: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    let recent_idx = jql
        .find("created >= -30d")
        .expect("AC-004: --recent clause 'created >= -30d' must be present");
    let updated_recent_idx = jql
        .find("updated >= -30d")
        .expect("AC-004: --updated-recent clause 'updated >= -30d' must be present");
    assert!(
        recent_idx < updated_recent_idx,
        "AC-004: 'recent' clause must precede 'updated-recent' clause \
         per BC-2.1.007's stable order, got jql: {jql}"
    );
}

/// AC-005 / BC-2.1.007 amendment (stable-order position): `--recent 7d
/// --updated-recent 60d --asset CUST-5` composes clauses with
/// `updated-recent` positioned immediately AFTER `recent` and BEFORE
/// `asset`. This end-to-end integration test verifies only RELATIVE order,
/// via `jql.find()` substring-index comparison — a smoke check that the
/// composed JQL string contains the clauses in the right relative order,
/// not a positional guarantee. The authoritative `Vec<String>`
/// positional-equality check (AC-005's mandated discipline) lives in the
/// unit test
/// `test_bc_2_1_007_build_filter_clauses_updated_recent_immediately_after_recent_before_asset`
/// in `src/cli/issue/list.rs`, which asserts exact `Vec<String>` equality
/// on `build_filter_clauses`'s output.
#[tokio::test]
async fn test_bc_2_1_007_issue_list_updated_recent_clause_ordering_after_recent_before_asset() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;

    // CMDB fields discovery, required by build_asset_clause for a direct
    // asset-key passthrough (mirrors tests/cli_handler.rs's
    // test_handler_list_asset_key_passthrough_skips_assets_api pattern).
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": "customfield_10191",
                "name": "Client",
                "custom": true,
                "schema": {
                    "type": "any",
                    "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
                    "customId": 10191
                }
            }
        ])))
        .mount(&server)
        .await;

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--recent",
            "7d",
            "--updated-recent",
            "60d",
            "--asset",
            "CUST-5",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-005: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    let recent_idx = jql
        .find("created >= -7d")
        .expect("AC-005: --recent clause must be present");
    let updated_recent_idx = jql
        .find("updated >= -60d")
        .expect("AC-005: --updated-recent clause must be present");
    let asset_idx = jql
        .find("aqlFunction")
        .expect("AC-005: --asset clause must be present");

    assert!(
        recent_idx < updated_recent_idx,
        "AC-005: updated-recent must come AFTER recent, got jql: {jql}"
    );
    assert!(
        updated_recent_idx < asset_idx,
        "AC-005: updated-recent must come BEFORE asset, got jql: {jql}"
    );
}

/// AC-006 / BC-2.1.006 amendment: with no project, no filters, and no
/// `--jql`, exit 64 with stderr enumerating all 15 filter sources —
/// `--updated-recent` appended as source #15, immediately before `or --jql`.
#[tokio::test]
async fn test_bc_2_1_006_issue_list_no_filters_stderr_enumerates_15_sources() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_expect_zero_http(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--no-input", "issue", "list"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "AC-006: expected failure with no project/filters, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-006: no-filters guard should exit 64 (UserError), got: {:?} (stderr: {stderr})",
        output.status.code()
    );
    assert!(
        stderr.contains(
            "No project or filters specified. Use --project, --assignee, --reporter, --status, \
             --open, --team, --recent, --created-after, --created-before, --updated-after, \
             --updated-before, --asset, --component, --updated-recent, or --jql. \
             You can also set a default project in .jr.toml or run \"jr init\"."
        ),
        "AC-006: expected the amended 15-source stderr enumeration (with \
         --updated-recent inserted immediately before 'or --jql'), got: {stderr}"
    );
}

/// AC-007 / BC-2.1.023 Edge Case EC-2.1.023-4 (AMENDED, FIX-F5-LRE-1,
/// ADV-LRE-F5-A-MED-001, human-adjudicated): `--updated-recent` alone (no
/// `--project`/configured project, no other filter) now PROCEEDS to a
/// cross-project query exactly like `--recent` alone and every other filter
/// source — `build_filter_clauses` composes the `updated >= -{d}` clause
/// into `filter_parts` regardless of `base_parts`, so the end-of-function
/// BC-2.1.006 guard sees a non-empty `all_parts` and does not fire. This
/// supersedes the original AC-007 (dedicated `--updated-recent`-alone
/// exit-64 guard), which was the only 1 of 15 filter sources with that
/// special case — it has been removed so `--updated-recent` composes
/// uniformly with the other fourteen.
#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_alone_proceeds_like_recent() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--no-input", "issue", "list", "--updated-recent", "60d"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-007 (amended): --updated-recent alone with no project must now \
         proceed exactly like --recent alone, got exit: {:?} (stderr: {})",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("updated >= -60d"),
        "AC-007 (amended): expected clause 'updated >= -60d' in composed \
         JQL, got: {jql}"
    );
    assert!(
        !jql.contains("project ="),
        "AC-007 (amended): --updated-recent alone with no configured \
         project must not spuriously scope to a project, got: {jql}"
    );

    // ADV-FIXF5-P1-LOW-001 / VP-UPDATED-RECENT-002: the alone path must
    // fire exactly ONE search/jql request -- no retry, no duplicate
    // composition, no accidental second call.
    let received = server.received_requests().await.unwrap();
    let search_call_count = received
        .iter()
        .filter(|r| r.url.path() == "/rest/api/3/search/jql")
        .count();
    assert_eq!(
        search_call_count, 1,
        "VP-UPDATED-RECENT-002: --updated-recent alone must fire exactly \
         one POST /rest/api/3/search/jql request, got: {search_call_count}"
    );
}

/// Pass 2 MEDIUM regression guard (retained post FIX-F5-LRE-1): a
/// `.jr.toml` configuring only `board_id` (no `project` key -- a valid
/// state per `Config::board_id`/`test_board_id_cli_override` in
/// `src/config.rs`) is a genuinely scoped configuration.
/// `jr issue list --updated-recent 60d` must fall through to the same
/// active-sprint board resolution that a bare `jr issue list` and
/// `jr issue list --recent 60d` both already succeed under, and the final
/// composed JQL must contain both the sprint scope and the
/// `updated >= -60d` clause. (No no-filters guard is in play here at all
/// any more -- the dedicated `--updated-recent`-alone guard this test used
/// to regression-guard against has been removed; this test now exercises
/// pure clause composition in a board-scoped configuration.)
#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_with_configured_board_falls_through_to_sprint_scope()
 {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();

    // .jr.toml with ONLY board_id set -- no `project` key.
    std::fs::write(project_dir.path().join(".jr.toml"), "board_id = 42\n").unwrap();

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

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .current_dir(project_dir.path())
        .args(["--no-input", "issue", "list", "--updated-recent", "60d"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "board-scoped --updated-recent must NOT exit 64, got: {:?} (stderr: {stderr})",
        output.status.code()
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("sprint = 100"),
        "expected the active-sprint scope clause in the composed JQL, got: {jql}"
    );
    assert!(
        jql.contains("updated >= -60d"),
        "expected the --updated-recent clause in the composed JQL, got: {jql}"
    );
}

/// AC-008 / BC-2.1.023 Postcondition 1 (field-swap fidelity): `--updated-recent
/// 7d` produces `updated >= -7d` — NOT `created >= -7d` — confirming the
/// field name is correctly swapped from `--recent`'s template rather than
/// copy-pasted verbatim.
#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_uses_updated_field_not_created() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--updated-recent",
            "7d",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "AC-008: expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("updated >= -7d"),
        "AC-008: expected 'updated >= -7d' in composed JQL, got: {jql}"
    );
    assert!(
        !jql.contains("created >= -7d"),
        "AC-008: must NOT emit 'created >= -7d' (field-swap fidelity — \
         --updated-recent must not be copy-pasted from --recent's template \
         verbatim), got: {jql}"
    );
}

/// pr-review cycle 1 Finding 1 regression guard (AMENDED, FIX-F5-LRE-1,
/// ADV-LRE-F5-A-MED-001, human-adjudicated): a `.jr.toml` configuring only
/// `board_id` (no `project` key), a scrum board, with NO active sprint
/// resolves `base_parts` to an EMPTY Vec (the scrum "no active sprint"
/// fallback seeds `parts` from `project_key`, which is `None` here). The
/// `base_parts.is_empty()` backstop guard this test used to regression-guard
/// has been removed: `--updated-recent`'s own clause now makes the final
/// `all_parts` non-empty and the query PROCEEDS, exactly like every other
/// filter source composed with an otherwise-unscoped board configuration in
/// this same shape (e.g. `--recent` alone would compose identically) — an
/// unbounded, cross-project query is the deliberate, human-adjudicated new
/// behavior, not a residual gap.
#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_board_scrum_no_active_sprint_no_project_proceeds()
 {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();

    // .jr.toml with ONLY board_id set -- no `project` key.
    std::fs::write(project_dir.path().join(".jr.toml"), "board_id = 42\n").unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("scrum")),
        )
        .mount(&server)
        .await;

    // NO active sprint -- empty `values` array.
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/sprint"))
        .and(query_param("state", "active"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::sprint_list_response(vec![])),
        )
        .mount(&server)
        .await;

    // The query now proceeds -- the search endpoint IS expected to fire.
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .current_dir(project_dir.path())
        .args(["--no-input", "issue", "list", "--updated-recent", "60d"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "board-scrum-no-active-sprint --updated-recent must now proceed \
         (amended AC-007 behavior), got: {:?} (stderr: {})",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert!(
        jql.contains("updated >= -60d"),
        "expected the --updated-recent clause in the composed JQL, got: {jql}"
    );
    assert!(
        !jql.contains("sprint ="),
        "no active sprint exists in this configuration, so no sprint scope \
         should be present, got: {jql}"
    );
    assert!(
        !jql.contains("project ="),
        "no project is configured, so no project scope should be present, \
         got: {jql}"
    );
}

// ── pr-review cycle 1 Finding 2 (AMENDED, FIX-F5-LRE-1, ADV-LRE-F5-A-MED-001):
// composition coverage for the 9 filter flags not already exercised by an
// existing --updated-recent AC test (project/recent/asset/updated_before/
// board_id are already covered by AC-001/AC-004/AC-005/AC-003b/the board
// regression tests above). These tests originally regression-guarded a
// dedicated `--updated-recent`-alone no-filters guard's conjunction; that
// guard has been removed (human-adjudicated: `--updated-recent` alone now
// proceeds like every other filter source, see the amended AC-007 test
// above), so these 9 tests are reframed to their remaining value: proving
// `--updated-recent` COMPOSES correctly (produces a valid combined JQL
// containing BOTH the `updated >= -60d` clause and the paired flag's own
// effect) when paired with each of the 9 flags, rather than merely
// asserting a since-removed guard was not tripped.

fn pr1_write_team_config(config_dir: &std::path::Path, team_field_id: &str) {
    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            "default_profile = \"default\"\n\n[profiles.default]\nurl = \"https://acme.atlassian.net\"\nteam_field_id = \"{team_field_id}\"\n"
        ),
    )
    .unwrap();
}

/// Asserts `--updated-recent` composed successfully alongside another
/// filter flag: exit 0, and the composed JQL (captured from the mounted
/// mock server) contains the `updated >= -60d` clause. Does not assert the
/// paired flag's own clause shape -- that is covered by each flag's own
/// dedicated test elsewhere; this only proves the PAIRING doesn't drop or
/// corrupt `--updated-recent`'s contribution.
async fn pr1_assert_composes_successfully(
    output: &std::process::Output,
    server: &MockServer,
    flag_desc: &str,
) {
    assert!(
        output.status.success(),
        "--updated-recent + {flag_desc} must compose successfully, \
         expected exit 0, got: {:?} (stderr: {})",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let jql = s606_1_composed_jql(server).await;
    assert!(
        jql.contains("updated >= -60d"),
        "--updated-recent + {flag_desc} must still contribute the \
         'updated >= -60d' clause to the composed JQL, got: {jql}"
    );
}

#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_with_assignee() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--updated-recent",
            "60d",
            "--assignee",
            "me",
        ])
        .output()
        .unwrap();

    pr1_assert_composes_successfully(&output, &server, "--assignee me").await;
}

#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_with_reporter() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--updated-recent",
            "60d",
            "--reporter",
            "me",
        ])
        .output()
        .unwrap();

    pr1_assert_composes_successfully(&output, &server, "--reporter me").await;
}

#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_with_status() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": "1", "name": "To Do", "statusCategory": {"key": "new"}},
            {"id": "2", "name": "In Progress", "statusCategory": {"key": "indeterminate"}},
            {"id": "3", "name": "Done", "statusCategory": {"key": "done"}}
        ])))
        .mount(&server)
        .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--updated-recent",
            "60d",
            "--status",
            "To Do",
        ])
        .output()
        .unwrap();

    pr1_assert_composes_successfully(&output, &server, "--status \"To Do\"").await;
}

#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_with_team() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // team_field_id configured + a UUID team value: both skip every HTTP
    // resolution step (find_team_field_id, cache load, GraphQL discovery)
    // via the UUID pass-through in `resolve_team_field`/`is_team_uuid`.
    pr1_write_team_config(config_dir.path(), "customfield_10100");
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--updated-recent",
            "60d",
            "--team",
            "12345678-1234-1234-1234-123456789012",
        ])
        .output()
        .unwrap();

    pr1_assert_composes_successfully(&output, &server, "--team <uuid>").await;
}

#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_with_open() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--updated-recent",
            "60d",
            "--open",
        ])
        .output()
        .unwrap();

    pr1_assert_composes_successfully(&output, &server, "--open").await;
}

#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_with_component() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // --component requires --project (BC-2.1.022) regardless of
    // --updated-recent, so --project is included here.
    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_components(
        &server,
        "FOO",
        vec![common::fixtures::component_response(
            "10001", "Backend", None, None, None,
        )],
    )
    .await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--updated-recent",
            "60d",
            "--component",
            "Backend",
        ])
        .output()
        .unwrap();

    pr1_assert_composes_successfully(&output, &server, "--project FOO --component Backend").await;
}

#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_with_created_after() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--updated-recent",
            "60d",
            "--created-after",
            "2026-01-01",
        ])
        .output()
        .unwrap();

    pr1_assert_composes_successfully(&output, &server, "--created-after 2026-01-01").await;
}

#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_with_created_before() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--updated-recent",
            "60d",
            "--created-before",
            "2026-01-01",
        ])
        .output()
        .unwrap();

    pr1_assert_composes_successfully(&output, &server, "--created-before 2026-01-01").await;
}

#[tokio::test]
async fn test_bc_2_1_023_issue_list_updated_recent_composes_with_jql() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--updated-recent",
            "60d",
            "--jql",
            "project = FOO",
        ])
        .output()
        .unwrap();

    pr1_assert_composes_successfully(&output, &server, "--jql \"project = FOO\"").await;
}

// ═══════════════════════════════════════════════════════════════════════
// S-588-1: `jr issue list --sort <field>:asc|desc` (BC-2.1.024/BC-2.1.025)
// ═══════════════════════════════════════════════════════════════════════
//
// Red Gate for S-588-1. `parse_sort`/`compose_order_by_with_sort`
// (`src/cli/issue/list.rs`) are `todo!()` stubs at the time these tests are
// written -- every `--sort`-bearing test below currently panics rather than
// asserting a real mismatch. Reuses the `s606_1_*` harness helpers (mock
// server + isolated cache/config dirs + composed-JQL capture) established by
// S-606-1's `--component` test suite above, and the `.jr.toml board_id`
// board-scoped pattern established by S-579-1's tests
// (`test_bc_2_1_023_issue_list_updated_recent_with_configured_board_falls_through_to_sprint_scope`).

/// AC-001 / BC-2.1.025 Edge Case EC-2.1.025-1: `--sort updated:desc` on the
/// default-project branch composes `order_by = "updated DESC, key ASC"` --
/// exact-string equality on the full composed JQL, not substring containment.
#[tokio::test]
async fn test_bc_2_1_025_issue_list_sort_composes_secondary_key_asc() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--sort",
            "updated:desc",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert_eq!(
        jql, "project = \"FOO\" ORDER BY updated DESC, key ASC",
        "AC-001: --sort updated:desc must compose order_by = \"updated DESC, \
         key ASC\", got: {jql}"
    );
}

/// AC-002 / BC-2.1.025 Postcondition 2 / EC-2.1.025-2: `--sort key:asc`
/// composes `order_by = "key ASC"` with NO redundant secondary `key ASC`
/// clause appended.
#[tokio::test]
async fn test_bc_2_1_025_issue_list_sort_key_field_omits_secondary_clause() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--sort",
            "key:asc",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert_eq!(
        jql, "project = \"FOO\" ORDER BY key ASC",
        "AC-002: --sort key:asc must compose order_by = \"key ASC\" with no \
         redundant secondary clause, got: {jql}"
    );
}

/// AC-003 / BC-2.1.024 postcondition 1: `--sort key:ASC` and `--sort
/// key:AsC` parse identically to `--sort key:asc` -- case-insensitive
/// direction matching.
#[tokio::test]
async fn test_bc_2_1_024_issue_list_sort_direction_case_insensitive() {
    for value in ["key:ASC", "key:AsC", "key:asc"] {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        s606_1_mock_project_exists(&server, "FOO").await;
        s606_1_mock_search_empty(&server).await;

        let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "--no-input",
                "issue",
                "list",
                "--project",
                "FOO",
                "--sort",
                value,
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "--sort {value:?}: stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let jql = s606_1_composed_jql(&server).await;
        assert_eq!(
            jql, "project = \"FOO\" ORDER BY key ASC",
            "AC-003: --sort {value:?} must parse identically to --sort \
             key:asc, got: {jql}"
        );
    }
}

/// AC-005 / BC-2.1.025 Postcondition 3: `--sort` overrides the `--jql`
/// branch's own default too -- both the hardcoded `"updated DESC"`
/// replacement AND the user's own embedded `--jql` `ORDER BY` (already
/// unconditionally stripped/replaced per BC-2.1.002 regardless of `--sort`).
#[tokio::test]
async fn test_bc_2_1_025_issue_list_sort_overrides_jql_branch_default() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--jql",
            "project = FOO ORDER BY rank ASC",
            "--sort",
            "updated:desc",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert_eq!(
        jql, "(project = FOO) ORDER BY updated DESC, key ASC",
        "AC-005: --sort must override both the --jql branch's hardcoded \
         \"updated DESC\" default and the user's own embedded --jql ORDER BY \
         (independently stripped per BC-2.1.002), got: {jql}"
    );
}

/// AC-006 / BC-2.1.025 Postcondition 4 / EC-2.1.025-4: `--sort rank:asc` on
/// a kanban board (whose default `order_by` is `"rank ASC"` when `--sort` is
/// absent, BC-2.1.004) overrides to `order_by = "rank ASC, key ASC"` -- NOT
/// collapsed to the bare board default, since the `key`-omission rule is
/// scoped to the literal field name `key`, not to a branch's own default
/// sort field.
#[tokio::test]
async fn test_bc_2_1_025_issue_list_sort_overrides_kanban_board_rank_default() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();

    // .jr.toml with both a default project and a kanban board_id.
    std::fs::write(
        project_dir.path().join(".jr.toml"),
        "project = \"FOO\"\nboard_id = 42\n",
    )
    .unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;

    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/42/configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::board_config_response("kanban")),
        )
        .mount(&server)
        .await;

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .current_dir(project_dir.path())
        .args(["--no-input", "issue", "list", "--sort", "rank:asc"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert_eq!(
        jql, "project = \"FOO\" AND statusCategory != Done ORDER BY rank ASC, key ASC",
        "AC-006: --sort rank:asc on a kanban board must override the board \
         rank ASC default AND still append the key ASC secondary sort, got: {jql}"
    );
}

/// BC-2.1.025 Postcondition 4 (scrum leg): `--sort` overrides the
/// scrum-active-sprint branch's board-driven `"rank ASC"` default too --
/// applied UNIFORMLY across both board branches (DEC-298 "always wins"), not
/// merely the kanban leg covered by AC-006.
#[tokio::test]
async fn test_bc_2_1_025_issue_list_sort_overrides_scrum_active_sprint_rank_default() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let project_dir = tempfile::tempdir().unwrap();

    // .jr.toml with ONLY board_id set -- no `project` key (mirrors S-579-1's
    // scrum-board pattern).
    std::fs::write(project_dir.path().join(".jr.toml"), "board_id = 42\n").unwrap();

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

    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .current_dir(project_dir.path())
        .args(["--no-input", "issue", "list", "--sort", "updated:desc"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert_eq!(
        jql, "sprint = 100 ORDER BY updated DESC, key ASC",
        "BC-2.1.025 Postcondition 4: --sort must override the \
         scrum-active-sprint branch's board-driven rank ASC default too, \
         got: {jql}"
    );
}

/// Composes with filters: `--sort` combined with `--project`/`--assignee`
/// puts the filter clause in the WHERE and the sort + key ASC secondary in
/// the ORDER BY, without disturbing `build_filter_clauses`' output
/// (BC-2.1.025 Postcondition 5 -- `--sort` does not push its own filter
/// clause).
#[tokio::test]
async fn test_bc_2_1_025_issue_list_sort_composes_with_filters() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--assignee",
            "me",
            "--sort",
            "updated:desc",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert_eq!(
        jql, "project = \"FOO\" AND assignee = currentUser() ORDER BY updated DESC, key ASC",
        "--sort must compose with existing filter clauses: WHERE carries the \
         filter, ORDER BY carries the sort + key ASC secondary, got: {jql}"
    );
}

/// AC-009 / BC-2.1.025 Postcondition 5 / BC-2.1.006 Note: `--sort` is NOT a
/// member of BC-2.1.006's "no filters specified" filter-source enumeration
/// -- `jr issue list --sort updated:desc` with no project/filters/`--jql`
/// still exits 64 via the SAME guard a completely bare invocation hits.
#[tokio::test]
async fn test_bc_2_1_006_issue_list_sort_alone_does_not_satisfy_filter_requirement() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_expect_zero_http(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["--no-input", "issue", "list", "--sort", "updated:desc"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "AC-009: --sort alone with no project/filters must still fail the \
         no-filters guard, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "AC-009: expected exit 64 (UserError) from BC-2.1.006's no-filters \
         guard, got: {:?} (stderr: {stderr})",
        output.status.code()
    );
    assert!(
        stderr.contains("No project or filters specified"),
        "AC-009: --sort must NOT be treated as a filter source -- expected \
         the canonical no-filters-specified message, got: {stderr}"
    );
}

/// AC-010 / BC-2.1.025 Edge Case EC-2.1.025-3: `--sort KEY:desc` (a
/// case-variant field name matching `key`) still triggers the secondary-sort
/// omission (case-insensitive match on the FIELD name for that check only),
/// but the field's OWN casing is passed through verbatim -- `order_by =
/// "KEY DESC"`, not lowercased.
#[tokio::test]
async fn test_bc_2_1_025_issue_list_sort_key_omission_case_insensitive_field_casing_preserved() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    s606_1_mock_project_exists(&server, "FOO").await;
    s606_1_mock_search_empty(&server).await;

    let output = s606_1_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "--no-input",
            "issue",
            "list",
            "--project",
            "FOO",
            "--sort",
            "KEY:desc",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let jql = s606_1_composed_jql(&server).await;
    assert_eq!(
        jql, "project = \"FOO\" ORDER BY KEY DESC",
        "AC-010: the omission check must match \"KEY\" against \"key\" \
         case-insensitively (no secondary clause), but the field's OWN \
         casing must be preserved verbatim (not lowercased), got: {jql}"
    );
}
