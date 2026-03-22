use jr::api::client::JiraClient;
use serde::Deserialize;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug, Deserialize)]
struct MyselfResponse {
    account_id: String,
    display_name: String,
}

#[tokio::test]
async fn test_get_request_with_auth_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(header(
            "Authorization",
            "Basic dGVzdEBleGFtcGxlLmNvbTpteS1hcGktdG9rZW4=",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "account_id": "123abc",
            "display_name": "Test User"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(
        server.uri(),
        "Basic dGVzdEBleGFtcGxlLmNvbTpteS1hcGktdG9rZW4=".to_string(),
    );

    let resp: MyselfResponse = client.get("/rest/api/3/myself").await.unwrap();
    assert_eq!(resp.account_id, "123abc");
    assert_eq!(resp.display_name, "Test User");
}

#[tokio::test]
async fn test_rate_limit_retry() {
    let server = MockServer::start().await;

    // First request returns 429 with Retry-After
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Second request returns 200
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "account_id": "123abc",
            "display_name": "Test User"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic fake-auth".to_string());

    let resp: MyselfResponse = client.get("/rest/api/3/myself").await.unwrap();
    assert_eq!(resp.account_id, "123abc");
}

#[tokio::test]
async fn test_401_returns_not_authenticated() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "message": "Client must be authenticated to access this resource."
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic bad-token".to_string());

    let err = client
        .get::<serde_json::Value>("/rest/api/3/myself")
        .await
        .unwrap_err();

    let err_string = err.to_string();
    assert!(
        err_string.contains("Not authenticated"),
        "Expected 'Not authenticated' in error, got: {err_string}"
    );
}
