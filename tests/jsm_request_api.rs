//! Integration tests for the JSM request submission + request-type discovery API.
//!
//! Covers AC-001 .. AC-006 from the pr1 story for issue #288.
//! All HTTP tests use the wiremock harness pattern established in `tests/queue.rs`.
//! Serde round-trip tests (AC-005, AC-006) require no HTTP mock server.

use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// AC-001 — `create_jsm_request` POSTs to `/rest/servicedeskapi/request` and
/// returns a `JsmRequestCreated` with the correct `issue_key` and `issue_id`.
///
/// Traces: BC-3.8.001
#[tokio::test]
async fn test_create_jsm_request_posts_to_servicedeskapi_and_returns_issue_key() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/servicedeskapi/request"))
        .and(body_partial_json(json!({
            "serviceDeskId": "10",
            "requestTypeId": "25",
            "requestFieldValues": {
                "summary": "test"
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "issueId": "107001",
            "issueKey": "HELPDESK-1",
            "summary": "Request JSD help via REST",
            "requestTypeId": "25",
            "serviceDeskId": "10",
            "_links": {
                "self": "https://example.atlassian.net/rest/servicedeskapi/request/107001",
                "web": "https://example.atlassian.net/servicedesk/customer/portal/10/HELPDESK-1",
                "agent": "https://example.atlassian.net/browse/HELPDESK-1",
                "jiraRest": "https://example.atlassian.net/rest/api/2/issue/107001"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let body = json!({
        "serviceDeskId": "10",
        "requestTypeId": "25",
        "isAdfRequest": true,
        "requestFieldValues": {
            "summary": "test",
            "description": {"type": "doc", "content": []}
        }
    });

    let result = client
        .create_jsm_request(body)
        .await
        .expect("create_jsm_request should succeed");

    assert_eq!(result.issue_key, "HELPDESK-1");
    assert_eq!(result.issue_id, Some("107001".to_string()));
}

/// AC-002 — `list_request_types` accumulates results across multiple pages,
/// stopping when `isLastPage` is true.
///
/// Traces: BC-X.12.001
#[tokio::test]
async fn test_list_request_types_paginates_is_last_page() {
    let server = MockServer::start().await;

    // Page 1: start=0, isLastPage=false
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/28/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": false,
            "_links": {
                "base": "https://example.atlassian.net/rest/servicedeskapi",
                "next": "https://example.atlassian.net/rest/servicedeskapi/servicedesk/28/requesttype?start=2&limit=50"
            },
            "values": [
                {
                    "id": "11001",
                    "name": "Get IT Help",
                    "description": "Get IT help for hardware, software, or other issues",
                    "helpText": "Please describe the issue in detail",
                    "issueTypeId": "12345",
                    "serviceDeskId": "28",
                    "portalId": "2",
                    "groupIds": ["12"]
                },
                {
                    "id": "11002",
                    "name": "Password Reset",
                    "description": "Reset your password",
                    "helpText": "Provide your username",
                    "issueTypeId": "12346",
                    "serviceDeskId": "28",
                    "portalId": "2",
                    "groupIds": ["12", "13"]
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Page 2: start=2, isLastPage=true
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/28/requesttype"))
        .and(query_param("start", "2"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 2,
            "limit": 50,
            "isLastPage": true,
            "_links": {
                "base": "https://example.atlassian.net/rest/servicedeskapi"
            },
            "values": [
                {
                    "id": "11003",
                    "name": "Hardware Request",
                    "description": "Request new hardware",
                    "helpText": "Specify the hardware you need",
                    "issueTypeId": "12347",
                    "serviceDeskId": "28",
                    "portalId": "2",
                    "groupIds": []
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let result = client
        .list_request_types("28", None)
        .await
        .expect("list_request_types should succeed");

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, "11001");
    assert_eq!(result[1].id, "11002");
    assert_eq!(result[2].id, "11003");
}

/// AC-003 — When `search_query` is `Some("password")`, the HTTP request includes
/// query param `searchQuery=password`. When `None`, the param is absent.
///
/// Traces: BC-X.12.001
#[tokio::test]
async fn test_list_request_types_search_query_forwarded() {
    let server = MockServer::start().await;

    // Mock enforces searchQuery is present AND pagination params are sent correctly.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/28/requesttype"))
        .and(query_param("searchQuery", "password"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": [
                {
                    "id": "11002",
                    "name": "Password Reset",
                    "description": "Reset your password",
                    "helpText": "Provide your username",
                    "issueTypeId": "12346",
                    "serviceDeskId": "28",
                    "portalId": "2",
                    "groupIds": ["12"]
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let result = client
        .list_request_types("28", Some("password"))
        .await
        .expect("list_request_types with search_query should succeed");

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "11002");
    assert_eq!(result[0].name, "Password Reset");
}

/// AC-003 (negative) — When `search_query` is `None`, the `searchQuery` query param
/// MUST NOT be sent. The mock uses `query_param_is_missing("searchQuery")` to enforce
/// this: the mock will not match any request that includes `searchQuery`, so the test
/// fails at the wiremock layer (unmatched request → panic) if the implementation leaks
/// the param.
#[tokio::test]
async fn test_list_request_types_search_query_absent_when_none() {
    let server = MockServer::start().await;

    // query_param_is_missing enforces that searchQuery is truly absent from the request —
    // the mock will NOT match if the implementation sends searchQuery=... as an extra param.
    Mock::given(method("GET"))
        .and(path("/rest/servicedeskapi/servicedesk/28/requesttype"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "50"))
        .and(query_param_is_missing("searchQuery"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1,
            "start": 0,
            "limit": 50,
            "isLastPage": true,
            "_links": {},
            "values": [
                {
                    "id": "11001",
                    "name": "Get IT Help",
                    "description": "Get help",
                    "issueTypeId": "12345",
                    "serviceDeskId": "28",
                    "groupIds": []
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let result = client
        .list_request_types("28", None)
        .await
        .expect("list_request_types with None should succeed");

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "11001");
}

/// AC-004 — `get_request_type_fields` GETs the correct endpoint and returns a
/// `RequestTypeFieldsResponse` with `can_raise_on_behalf_of`, `can_add_request_participants`,
/// and the field list deserialized correctly.
///
/// Traces: BC-X.12.005
#[tokio::test]
async fn test_get_request_type_fields_returns_field_list() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/rest/servicedeskapi/servicedesk/28/requesttype/11001/field",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "canAddRequestParticipants": true,
            "canRaiseOnBehalfOf": true,
            "requestTypeFields": [
                {
                    "fieldId": "summary",
                    "jiraSchema": {"system": "summary", "type": "string"},
                    "name": "What do you need?",
                    "description": null,
                    "required": true,
                    "validValues": [],
                    "visible": true
                },
                {
                    "fieldId": "customfield_10000",
                    "jiraSchema": {
                        "custom": "com.atlassian.jira.plugin.system.customfieldtypes:userpicker",
                        "customId": 10000,
                        "type": "user"
                    },
                    "name": "Nominee",
                    "description": null,
                    "required": true,
                    "validValues": [],
                    "visible": true
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());

    let result = client
        .get_request_type_fields("28", "11001")
        .await
        .expect("get_request_type_fields should succeed");

    assert!(result.can_raise_on_behalf_of);
    assert!(result.can_add_request_participants);
    assert_eq!(result.request_type_fields.len(), 2);
    assert_eq!(result.request_type_fields[0].field_id, "summary");
    assert_eq!(result.request_type_fields[0].name, "What do you need?");
    assert!(result.request_type_fields[0].required);
    assert_eq!(result.request_type_fields[1].field_id, "customfield_10000");
    assert!(result.request_type_fields[1].required);
}

/// AC-005 — `RequestType` struct round-trips through serde correctly.
/// Exercises: `id`, `name`, `description`, `help_text` (← `helpText`),
/// `issue_type_id` (← `issueTypeId`), `group_ids` (← `groupIds`).
///
/// Traces: BC-X.12.008
#[test]
fn test_request_type_struct_round_trip() {
    let raw = r#"{
        "id": "11001",
        "name": "Get IT Help",
        "description": "Get IT help for hardware, software, or other issues",
        "helpText": "Please describe the issue in detail",
        "issueTypeId": "12345",
        "serviceDeskId": "28",
        "portalId": "2",
        "groupIds": ["12", "34"]
    }"#;

    let rt: jr::types::jsm::RequestType = serde_json::from_str(raw)
        .expect("RequestType should deserialize from Atlassian JSON shape");

    assert_eq!(rt.id, "11001");
    assert_eq!(rt.name, "Get IT Help");
    assert_eq!(
        rt.description.as_deref(),
        Some("Get IT help for hardware, software, or other issues")
    );
    assert_eq!(
        rt.help_text.as_deref(),
        Some("Please describe the issue in detail")
    );
    assert_eq!(rt.issue_type_id.as_deref(), Some("12345"));
    assert_eq!(rt.group_ids, vec!["12", "34"]);

    // Re-serialize and parse back — confirms fields captured by the struct survive the round-trip.
    // Fields present in the Atlassian API shape but absent from RequestType (e.g. serviceDeskId,
    // portalId) are intentionally not modelled and are not preserved.
    let re_serialized = serde_json::to_string(&rt).expect("RequestType should serialize");
    let rt2: jr::types::jsm::RequestType = serde_json::from_str(&re_serialized)
        .expect("RequestType should deserialize after re-serialization");

    assert_eq!(rt2.id, rt.id);
    assert_eq!(rt2.name, rt.name);
    assert_eq!(rt2.description, rt.description);
    assert_eq!(rt2.help_text, rt.help_text);
    assert_eq!(rt2.issue_type_id, rt.issue_type_id);
    assert_eq!(rt2.group_ids, rt.group_ids);
}

/// AC-006 — `JsmRequestCreated` deserializes `issueKey` and optional `issueId` correctly.
/// Also verifies that a payload WITHOUT `issueId` does not error (field is `Option`).
///
/// Traces: BC-3.8.001
#[test]
fn test_jsm_request_created_extracts_issue_key() {
    // Full payload — both issueKey and issueId present
    let raw_full = r#"{"issueId":"107001","issueKey":"HELPDESK-1"}"#;
    let result: jr::types::jsm::JsmRequestCreated =
        serde_json::from_str(raw_full).expect("JsmRequestCreated should deserialize with issueId");

    assert_eq!(result.issue_key, "HELPDESK-1");
    assert_eq!(result.issue_id, Some("107001".to_string()));

    // Partial payload — only issueKey, no issueId (optional field must not cause an error)
    let raw_no_id = r#"{"issueKey":"HELPDESK-2","summary":"something","serviceDeskId":"10"}"#;
    let result_no_id: jr::types::jsm::JsmRequestCreated = serde_json::from_str(raw_no_id)
        .expect("JsmRequestCreated should deserialize without issueId (field is Option)");

    assert_eq!(result_no_id.issue_key, "HELPDESK-2");
    assert_eq!(result_no_id.issue_id, None);
}
