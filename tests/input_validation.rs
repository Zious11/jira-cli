#[allow(dead_code)]
mod common;

use std::collections::HashSet;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn project_exists_returns_true_for_valid_project() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10000",
            "key": "PROJ",
            "name": "My Project"
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    assert!(client.project_exists("PROJ").await.unwrap());
}

#[tokio::test]
async fn project_exists_returns_false_for_invalid_project() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/NOPE"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["No project could be found with key 'NOPE'."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    assert!(!client.project_exists("NOPE").await.unwrap());
}

#[tokio::test]
async fn get_all_statuses_returns_status_names() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": "1", "name": "To Do", "statusCategory": {"key": "new"}},
            {"id": "2", "name": "In Progress", "statusCategory": {"key": "indeterminate"}},
            {"id": "3", "name": "Done", "statusCategory": {"key": "done"}}
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses = client.get_all_statuses().await.unwrap();
    assert_eq!(statuses.len(), 3);
    assert!(statuses.contains(&"To Do".to_string()));
    assert!(statuses.contains(&"In Progress".to_string()));
    assert!(statuses.contains(&"Done".to_string()));
}

fn project_statuses_response(statuses: Vec<&str>) -> serde_json::Value {
    let status_objects: Vec<serde_json::Value> = statuses
        .iter()
        .enumerate()
        .map(|(i, name)| {
            serde_json::json!({
                "id": format!("{}", i + 1),
                "name": name,
                "description": null
            })
        })
        .collect();
    serde_json::json!([{
        "id": "1",
        "name": "Task",
        "subtask": false,
        "statuses": status_objects
    }])
}

#[tokio::test]
async fn invalid_status_with_project_returns_no_match() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ/statuses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_statuses_response(vec![
                "To Do",
                "In Progress",
                "Done",
            ])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses_response = client.get_project_statuses("PROJ").await.unwrap();

    let names: Vec<String> = {
        let mut seen = HashSet::new();
        let mut n = Vec::new();
        for it in &statuses_response {
            for s in &it.statuses {
                if seen.insert(s.name.clone()) {
                    n.push(s.name.clone());
                }
            }
        }
        n.sort();
        n
    };

    let result = jr::partial_match::partial_match("Nonexistant", &names);
    assert!(matches!(result, jr::partial_match::MatchResult::None(_)));
}

#[tokio::test]
async fn valid_status_partial_match_resolves() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ/statuses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_statuses_response(vec![
                "To Do",
                "In Progress",
                "Done",
            ])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses_response = client.get_project_statuses("PROJ").await.unwrap();
    let names: Vec<String> = {
        let mut seen = HashSet::new();
        let mut n = Vec::new();
        for it in &statuses_response {
            for s in &it.statuses {
                if seen.insert(s.name.clone()) {
                    n.push(s.name.clone());
                }
            }
        }
        n.sort();
        n
    };

    let result = jr::partial_match::partial_match("in prog", &names);
    match result {
        jr::partial_match::MatchResult::Exact(name) => assert_eq!(name, "In Progress"),
        other => panic!("Expected Exact, got {:?}", std::mem::discriminant(&other)),
    }
}

#[tokio::test]
async fn ambiguous_status_returns_multiple_matches() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/PROJ/statuses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_statuses_response(vec![
                "In Progress",
                "In Review",
                "Done",
            ])),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses_response = client.get_project_statuses("PROJ").await.unwrap();
    let names: Vec<String> = {
        let mut seen = HashSet::new();
        let mut n = Vec::new();
        for it in &statuses_response {
            for s in &it.statuses {
                if seen.insert(s.name.clone()) {
                    n.push(s.name.clone());
                }
            }
        }
        n.sort();
        n
    };

    let result = jr::partial_match::partial_match("in", &names);
    match result {
        jr::partial_match::MatchResult::Ambiguous(matches) => {
            assert!(matches.contains(&"In Progress".to_string()));
            assert!(matches.contains(&"In Review".to_string()));
        }
        other => panic!(
            "Expected Ambiguous, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[tokio::test]
async fn status_validation_with_global_statuses() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": "1", "name": "Open", "statusCategory": {"key": "new"}},
            {"id": "2", "name": "Closed", "statusCategory": {"key": "done"}}
        ])))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let statuses = client.get_all_statuses().await.unwrap();

    let result = jr::partial_match::partial_match("Nonexistant", &statuses);
    assert!(matches!(result, jr::partial_match::MatchResult::None(_)));
}

#[tokio::test]
async fn project_statuses_404_means_project_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/project/NOPE/statuses"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": ["No project could be found with key 'NOPE'."],
            "errors": {}
        })))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = client.get_project_statuses("NOPE").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.downcast_ref::<jr::error::JrError>()
            .is_some_and(|e| matches!(e, jr::error::JrError::ApiError { status: 404, .. }))
    );
}
