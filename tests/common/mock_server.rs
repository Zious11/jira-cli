use super::fixtures;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub async fn setup_with_myself() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixtures::user_response()))
        .mount(&server)
        .await;
    server
}
