use jr::api::client::JiraClient;
use jr::api::client::extract_error_message;
use reqwest::Method;
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

#[test]
fn test_extract_error_message_from_error_messages_array() {
    let body =
        br#"{"errorMessages":["Issue does not exist","Or you lack permission"],"errors":{}}"#;
    let result = extract_error_message(body);
    assert_eq!(result, "Issue does not exist; Or you lack permission");
}

#[test]
fn test_extract_error_message_from_message_field() {
    let body = br#"{"message":"Property with key not found"}"#;
    let result = extract_error_message(body);
    assert_eq!(result, "Property with key not found");
}

#[test]
fn test_extract_error_message_prefers_error_messages_over_message() {
    let body = br#"{"errorMessages":["first"],"message":"second"}"#;
    let result = extract_error_message(body);
    assert_eq!(result, "first");
}

#[test]
fn test_extract_error_message_empty_error_messages_falls_back_to_errors_object() {
    let body = br#"{"errorMessages":[],"errors":{"summary":"You must specify a summary"}}"#;
    let result = extract_error_message(body);
    assert_eq!(result, "summary: You must specify a summary");
}

#[test]
fn test_extract_error_message_errors_object_multiple_fields() {
    let body =
        br#"{"errorMessages":[],"errors":{"summary":"is required","priority":"is required"}}"#;
    let result = extract_error_message(body);
    assert_eq!(result, "priority: is required; summary: is required");
}

#[test]
fn test_extract_error_message_errors_object_empty_falls_through() {
    let body = br#"{"errorMessages":[],"errors":{}}"#;
    let result = extract_error_message(body);
    // Empty errors object, no message field → raw body fallback
    assert_eq!(result, r#"{"errorMessages":[],"errors":{}}"#);
}

#[test]
fn test_extract_error_message_errors_object_nested_value() {
    let body = br#"{"errorMessages":[],"errors":{"customfield_10001":{"messages":["invalid"]}}}"#;
    let result = extract_error_message(body);
    assert_eq!(result, r#"customfield_10001: {"messages":["invalid"]}"#);
}

#[test]
fn test_extract_error_message_errors_object_mixed_values() {
    let body = br#"{"errorMessages":[],"errors":{"summary":"is required","components":["a","b"]}}"#;
    let result = extract_error_message(body);
    assert!(
        result.contains("summary: is required"),
        "expected 'summary: is required' in '{result}'"
    );
    assert!(
        result.contains(r#"components: ["a","b"]"#),
        "expected serialized array in '{result}'"
    );
}

#[test]
fn test_extract_error_message_error_message_singular() {
    let body = br#"{"errorMessage":"Cannot find issue"}"#;
    let result = extract_error_message(body);
    assert_eq!(result, "Cannot find issue");
}

#[test]
fn test_extract_error_message_plain_text_body() {
    let body = b"Internal Server Error";
    let result = extract_error_message(body);
    assert_eq!(result, "Internal Server Error");
}

#[test]
fn test_extract_error_message_empty_body() {
    let body = b"";
    let result = extract_error_message(body);
    assert_eq!(result, "<empty response body>");
}

#[tokio::test]
async fn test_send_raw_returns_response_for_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"accountId":"abc"}"#))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let req = client
        .request(Method::GET, "/rest/api/3/myself")
        .build()
        .unwrap();
    let response = client.send_raw(req).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().await.unwrap();
    assert_eq!(body, r#"{"accountId":"abc"}"#);
}

#[tokio::test]
async fn test_send_raw_returns_response_for_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/MISSING-1"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_string(r#"{"errorMessages":["Issue does not exist"],"errors":{}}"#),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let req = client
        .request(Method::GET, "/rest/api/3/issue/MISSING-1")
        .build()
        .unwrap();
    let response = client.send_raw(req).await.unwrap();

    // Critical: 404 is NOT converted to an error
    assert_eq!(response.status().as_u16(), 404);
    let body = response.text().await.unwrap();
    assert!(body.contains("Issue does not exist"));
}

#[tokio::test]
async fn test_send_raw_retries_429_then_succeeds() {
    let server = MockServer::start().await;
    // First call returns 429
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    // Second call returns 200
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .expect(1)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let req = client
        .request(Method::GET, "/rest/api/3/myself")
        .build()
        .unwrap();
    let response = client.send_raw(req).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn test_send_raw_returns_429_after_exhausting_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .expect(4) // initial + 3 retries (MAX_RETRIES)
        .mount(&server)
        .await;

    let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());

    let req = client
        .request(Method::GET, "/rest/api/3/myself")
        .build()
        .unwrap();
    let response = client.send_raw(req).await.unwrap();

    // Caller receives the 429 response — not an error
    assert_eq!(response.status().as_u16(), 429);
}
