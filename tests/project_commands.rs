#[allow(dead_code)]
mod common;

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_list_projects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::project_search_response(vec![
                common::fixtures::project_response(
                    "FOO",
                    "Project Foo",
                    "software",
                    Some("Jane Doe"),
                ),
                common::fixtures::project_response(
                    "BAR",
                    "Project Bar",
                    "service_desk",
                    Some("John Smith"),
                ),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(None, Some(50)).await.unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].key, "FOO");
    assert_eq!(projects[0].name, "Project Foo");
    assert_eq!(projects[0].project_type_key, "software");
    assert_eq!(projects[0].lead.as_ref().unwrap().display_name, "Jane Doe");
    assert_eq!(projects[1].key, "BAR");
    assert_eq!(projects[1].project_type_key, "service_desk");
}

#[tokio::test]
async fn test_list_projects_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(common::fixtures::project_search_response(vec![])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(None, Some(50)).await.unwrap();
    assert!(projects.is_empty());
}

#[tokio::test]
async fn test_list_projects_lead_missing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::project_search_response(vec![common::fixtures::project_response(
                "FOO",
                "Project Foo",
                "software",
                None,
            )]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client.list_projects(None, Some(50)).await.unwrap();
    assert_eq!(projects.len(), 1);
    assert!(projects[0].lead.is_none());
}

#[tokio::test]
async fn test_list_projects_with_type_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(query_param("typeKey", "software"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::project_search_response(vec![common::fixtures::project_response(
                "FOO",
                "Project Foo",
                "software",
                Some("Jane Doe"),
            )]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let projects = client
        .list_projects(Some("software"), Some(50))
        .await
        .unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].project_type_key, "software");
}

#[tokio::test]
async fn test_list_projects_all_paginates() {
    let server = MockServer::start().await;

    // Page 1: startAt=0, maxResults=50, total=3 (but only 2 returned → has_more=true)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(query_param("startAt", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [
                common::fixtures::project_response("FOO", "Project Foo", "software", Some("Jane")),
                common::fixtures::project_response("BAR", "Project Bar", "software", Some("John")),
            ],
            "startAt": 0,
            "maxResults": 2,
            "total": 3,
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Page 2: startAt=2, maxResults=2, total=3 (1 returned → last page)
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/search"))
        .and(query_param("startAt", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [
                common::fixtures::project_response("BAZ", "Project Baz", "business", None),
            ],
            "startAt": 2,
            "maxResults": 2,
            "total": 3,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    // max_results=None triggers the --all pagination path
    let projects = client.list_projects(None, None).await.unwrap();
    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0].key, "FOO");
    assert_eq!(projects[1].key, "BAR");
    assert_eq!(projects[2].key, "BAZ");
}
