/// API-layer Red Gate tests for delete_comment, update_comment, get_comment.
///
/// These are direct JiraClient method calls backed by wiremock. They verify
/// URL construction, wire shapes, and response handling at the API layer —
/// independent of CLI handlers (handler integration tests live in S-577-4/5).
///
/// BC anchors: BC-3.5.002, BC-3.5.005, BC-3.5.006, BC-3.5.007, BC-3.5.010
/// Story: S-577-2, GitHub issue #577
use serde_json::json;
use std::collections::BTreeSet;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_AUTH: &str = "Basic dGVzdDp0ZXN0";

fn minimal_adf() -> serde_json::Value {
    json!({
        "type": "doc",
        "version": 1,
        "content": [{"type": "paragraph", "content": [{"type": "text", "text": "hello"}]}]
    })
}

// ---------------------------------------------------------------------------
// AC-001: delete_comment — DELETE 204 → Ok(())
// BC-3.5.002, EC-3.5.002-2
// ---------------------------------------------------------------------------

/// Verify that `delete_comment` sends DELETE to the correct path and
/// returns `Ok(())` on a 204 response. The mock path assertion confirms
/// `urlencoding::encode(key)` is applied — standard keys are a no-op,
/// so the path is `/rest/api/3/issue/FOO-1/comment/10001` verbatim.
#[tokio::test]
async fn test_delete_comment_204_returns_ok() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(server.uri(), TEST_AUTH.to_string());
    client.delete_comment("FOO-1", "10001").await.unwrap();
}

// ---------------------------------------------------------------------------
// AC-002: update_comment body-only — "properties" key MUST NOT be present
// BC-3.5.005, VP-577-001
// ---------------------------------------------------------------------------

/// Verify that `update_comment` with `visibility_flag: None` sends a PUT body
/// whose top-level key set is exactly `{"body"}`. The `"properties"` key must
/// not be present — not as an empty array, not as null.
#[tokio::test]
async fn test_update_comment_body_only_no_properties_key() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(server.uri(), TEST_AUTH.to_string());
    client
        .update_comment("FOO-1", "10001", minimal_adf(), None)
        .await
        .unwrap();

    let captured = server.received_requests().await.unwrap();
    assert_eq!(captured.len(), 1, "expected exactly 1 PUT request");
    let body: serde_json::Value =
        serde_json::from_slice(&captured[0].body).expect("PUT body must be valid JSON");
    let keys: BTreeSet<&str> = body
        .as_object()
        .expect("PUT body must be an object")
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["body"]),
        "body-only PUT must have exactly {{\"body\"}} as top-level keys; \
         got {keys:?} — \"properties\" must not be present"
    );
}

// ---------------------------------------------------------------------------
// AC-003: update_comment --internal — properties wire shape with internal:true
// BC-3.5.006, VP-577-002
// ---------------------------------------------------------------------------

/// Verify that `update_comment` with `visibility_flag: Some(true)` sends the
/// correct `properties` array in the PUT body:
/// - Top-level key set == `{"body", "properties"}` (exact)
/// - `properties[0].key == "sd.public.comment"` (dot, NOT underscore)
/// - `properties[0].value.internal == true` (boolean, NOT string "true")
/// - `properties` array length == 1
/// - `"visibility"` key is absent
#[tokio::test]
async fn test_update_comment_internal_properties_wire_shape() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(server.uri(), TEST_AUTH.to_string());
    client
        .update_comment("FOO-1", "10001", minimal_adf(), Some(true))
        .await
        .unwrap();

    let captured = server.received_requests().await.unwrap();
    assert_eq!(captured.len(), 1, "expected exactly 1 PUT request");
    let body: serde_json::Value =
        serde_json::from_slice(&captured[0].body).expect("PUT body must be valid JSON");

    let keys: BTreeSet<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["body", "properties"]),
        "internal PUT must have exactly {{\"body\",\"properties\"}} as top-level keys"
    );

    let props = body["properties"]
        .as_array()
        .expect("properties must be an array");
    assert_eq!(
        props.len(),
        1,
        "properties array must have exactly 1 element"
    );
    assert_eq!(
        props[0]["key"], "sd.public.comment",
        "property key must be \"sd.public.comment\" (dot-separated, NOT underscore)"
    );
    assert_eq!(
        props[0]["value"]["internal"],
        serde_json::Value::Bool(true),
        "internal must be boolean true, NOT string \"true\""
    );
    assert!(
        body.get("visibility").is_none(),
        "\"visibility\" key must not be present in internal PUT body"
    );
}

// ---------------------------------------------------------------------------
// AC-004: update_comment --public — properties wire shape with internal:false
// BC-3.5.007, VP-577-003
// ---------------------------------------------------------------------------

/// Verify that `update_comment` with `visibility_flag: Some(false)` sends the
/// correct `properties` array in the PUT body:
/// - Top-level key set == `{"body", "properties"}` (exact)
/// - `properties[0].value.internal == false` (boolean, NOT string "false")
/// - `properties[0].key == "sd.public.comment"`
/// - Array length == 1
/// - `"visibility"` key is absent
#[tokio::test]
async fn test_update_comment_public_properties_wire_shape() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(server.uri(), TEST_AUTH.to_string());
    client
        .update_comment("FOO-1", "10001", minimal_adf(), Some(false))
        .await
        .unwrap();

    let captured = server.received_requests().await.unwrap();
    assert_eq!(captured.len(), 1, "expected exactly 1 PUT request");
    let body: serde_json::Value =
        serde_json::from_slice(&captured[0].body).expect("PUT body must be valid JSON");

    let keys: BTreeSet<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["body", "properties"]),
        "public PUT must have exactly {{\"body\",\"properties\"}} as top-level keys"
    );

    let props = body["properties"]
        .as_array()
        .expect("properties must be an array");
    assert_eq!(
        props.len(),
        1,
        "properties array must have exactly 1 element"
    );
    assert_eq!(
        props[0]["value"]["internal"],
        serde_json::Value::Bool(false),
        "internal must be boolean false, NOT string \"false\""
    );
    assert_eq!(
        props[0]["key"], "sd.public.comment",
        "property key must be \"sd.public.comment\""
    );
    assert!(
        body.get("visibility").is_none(),
        "\"visibility\" key must not be present in public PUT body"
    );
}

// ---------------------------------------------------------------------------
// AC-005: get_comment — GET includes ?expand=properties query parameter
// BC-3.5.010
// ---------------------------------------------------------------------------

/// Verify that `get_comment` sends a GET request with `?expand=properties`
/// as a query parameter. Without this parameter Jira silently omits the
/// `properties` array from the response.
///
/// The wiremock mock requires the `expand=properties` query param to match —
/// if it is absent the mock returns 404 and the call returns Err.
#[tokio::test]
async fn test_get_comment_sends_expand_properties_query_param() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/comment/10001"))
        .and(query_param("expand", "properties"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "10001",
            "body": {
                "type": "doc",
                "version": 1,
                "content": []
            }
        })))
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(server.uri(), TEST_AUTH.to_string());
    let result = client.get_comment("FOO-1", "10001").await.unwrap();
    assert_eq!(result["id"], "10001");
}

// ---------------------------------------------------------------------------
// Whole-body mutant kill — delete_comment → Ok(()) survivor
// The load-bearing assertion is reqs.len()==1; the %20 assertions document
// encoding behavior but do NOT distinguish urlencoding::encode from bare key.
// ---------------------------------------------------------------------------

/// Kill the cargo-mutants whole-body mutant `delete_comment → Ok(())` via the
/// `reqs.len() == 1` assertion — `test_delete_comment_204_returns_ok` never
/// asserts that a request was sent, so a stub returning `Ok(())` immediately
/// would pass that test but fail here.
///
/// The `%20` assertions document percent-encoding of the key but do NOT
/// distinguish `urlencoding::encode(key)` from bare `key` for a space: the
/// `url` crate re-encodes a raw space to `%20` at parse time, so both paths
/// produce `%20` in the received URL. A genuine encode-vs-bare distinction
/// would need a character the URL path-encode set leaves alone but
/// `urlencoding` escapes (e.g. `+` → `%2B`). Formal VP-577-027 CLI-level
/// encoding ownership remains S-577-3.
#[tokio::test]
async fn test_delete_comment_encodes_key_with_space_in_url() {
    let server = MockServer::start().await;

    // Loose method-only matcher — the assertion is on the received URL, not the
    // path matcher, so we don't need to pre-encode the expected path here.
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(server.uri(), TEST_AUTH.to_string());
    client.delete_comment("MY KEY-1", "10001").await.unwrap();

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1, "expected exactly 1 DELETE request");
    let url_str = reqs[0].url.as_str();
    assert!(
        url_str.contains("MY%20KEY-1"),
        "space in key must be percent-encoded as %20; got: {url_str}"
    );
    assert!(
        !url_str.contains("MY KEY-1"),
        "raw space must not appear in URL; got: {url_str}"
    );
}
