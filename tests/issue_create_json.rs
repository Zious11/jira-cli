//! Integration tests for `jr issue create --output json` — issue #253.
//!
//! The JSON output of `issue create` must return the same full Issue payload
//! shape as `issue view --output json`, plus a top-level `url` field. After a
//! successful POST /rest/api/3/issue, the handler does a follow-up GET
//! /rest/api/3/issue/{key} and merges `url` into the result. On GET failure it
//! warns on stderr and falls back to the old `{key, url}` shape (exit 0).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn issue_create_json_returns_full_shape() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // POST /rest/api/3/issue — minimal Atlassian create response.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // GET /rest/api/3/issue/PROJ-123 — full issue payload the handler fetches
    // after a successful create.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-123",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
            "fields": {
                "summary": "test summary",
                "status": {
                    "name": "To Do",
                    "statusCategory": {"name": "To Do", "key": "new"}
                },
                "issuetype": {"name": "Task"},
                "project": {"key": "PROJ"}
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // GET /rest/api/3/field — fetched by `get_or_fetch_cmdb_fields` on a cold
    // cache. An empty array means "no CMDB fields" and keeps the test focused
    // on the shape guarantee.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "test summary",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected success, stderr: {stderr}, stdout: {stdout}"
    );

    let parsed: Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");

    assert_eq!(parsed["key"], "PROJ-123");
    assert!(
        parsed["url"]
            .as_str()
            .expect("url must be a string")
            .ends_with("/browse/PROJ-123"),
        "url must end with /browse/PROJ-123, got: {}",
        parsed["url"]
    );
    assert_eq!(
        parsed["fields"]["summary"], "test summary",
        "fields.summary must be present in the full shape"
    );
    assert_eq!(
        parsed["fields"]["status"]["name"], "To Do",
        "fields.status.name must be present in the full shape"
    );
    assert_eq!(parsed["fields"]["issuetype"]["name"], "Task");
    assert_eq!(parsed["fields"]["project"]["key"], "PROJ");

    assert!(
        !stderr.to_lowercase().contains("warning") && !stderr.to_lowercase().contains("error"),
        "expected clean stderr on happy path, got: {stderr}"
    );

    // The `fetch_error` sentinel must only appear on the fallback path —
    // asserting its absence here pins the happy-path/fallback distinction so
    // a regression that always emits the sentinel is caught.
    assert!(
        parsed.get("fetch_error").is_none(),
        "happy path must not include fetch_error sentinel: {parsed}"
    );
}

/// If the follow-up GET fails, the handler warns on stderr and falls back to
/// the old minimal `{key, url}` shape. The command still exits 0 — the create
/// succeeded, only the enrichment didn't.
#[tokio::test]
async fn issue_create_json_falls_back_on_get_failure() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10002",
            "key": "PROJ-456",
            "self": format!("{}/rest/api/3/issue/10002", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Follow-up GET fails with 500.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-456"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["boom"],
            "errors": {}
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "fallback summary",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(
        output.status.success(),
        "create must still succeed when the follow-up GET fails, stderr: {stderr}"
    );
    assert!(
        stderr.contains("PROJ-456") && stderr.to_lowercase().contains("warn"),
        "expected a stderr warning mentioning the new key, got: {stderr}"
    );
    assert!(
        stderr.contains("jr issue view PROJ-456"),
        "expected recovery hint pointing at `jr issue view`, got: {stderr}"
    );

    let parsed: Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed["key"], "PROJ-456");
    assert!(
        parsed["url"]
            .as_str()
            .expect("url must be a string")
            .ends_with("/browse/PROJ-456")
    );
    // The fallback shape is the old minimal `{key, url}` — no `fields` object.
    assert!(
        parsed.get("fields").is_none(),
        "fallback shape must not contain `fields`, got: {parsed}"
    );
    // Machine-readable sentinel so scripts using `jq '.fields.status.name'`
    // can detect degraded output without parsing stderr. The value is the
    // error message for diagnostics — we only require that it's present and
    // a string.
    assert!(
        parsed["fetch_error"].is_string(),
        "fallback must include fetch_error sentinel: {parsed}"
    );
}

/// The table output path must not trigger a follow-up GET after a successful
/// create. Table formatting only needs the key returned by POST, so fetching
/// the full issue would be a wasted round trip.
#[tokio::test]
async fn issue_create_table_does_not_trigger_follow_up_get() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10001",
            "key": "PROJ-125",
            "self": format!("{}/rest/api/3/issue/10001", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Critical: a GET on the issue MUST NOT be made on the table path.
    // wiremock's .expect(0) will fail the test on drop if the GET was called.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-125"))
        .respond_with(ResponseTemplate::new(500).set_body_string("should not be called"))
        .expect(0)
        .mount(&server)
        .await;

    // Also pin .expect(0) on /rest/api/3/field: the table path currently
    // doesn't call `get_or_fetch_cmdb_fields` at all, and a future refactor
    // that hoists CMDB discovery above the match in `handle_create` would
    // start hitting /field on the table path — a wasted round trip on every
    // table-mode create. This assertion catches that regression.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(Value::Array(vec![])))
        .expect(0)
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "table test",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected success, stderr: {stderr}, stdout: {stdout}"
    );
    // `print_success` writes to stderr, not stdout, so check both to avoid
    // coupling the assertion to output stream details — the key guarantee here
    // is the .expect(0) on the GET mock.
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("Created issue PROJ-125"),
        "output must announce the created key, stdout: {stdout}, stderr: {stderr}"
    );
}

/// The follow-up GET on the JSON path must pass the configured SP + team +
/// CMDB field IDs in `?fields=...`. This pins the `compose_extra_fields`
/// wire-through, so a refactor that accidentally calls `get_issue(&[])`
/// trips this test instead of silently dropping custom fields.
#[tokio::test]
async fn issue_create_json_follow_up_get_passes_configured_extra_fields() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Write a global config at $XDG_CONFIG_HOME/jr/config.toml with SP + team
    // field IDs, matching the shape defined in `src/config.rs` (`FieldsConfig`
    // under `[fields]`).
    let jr_config_dir = config_dir.path().join("jr");
    std::fs::create_dir_all(&jr_config_dir).unwrap();
    std::fs::write(
        jr_config_dir.join("config.toml"),
        "[fields]\nstory_points_field_id = \"customfield_10016\"\nteam_field_id = \"customfield_10001\"\n",
    )
    .unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "10003",
            "key": "PROJ-789",
            "self": format!("{}/rest/api/3/issue/10003", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Happy-path GET so we exercise the success branch. `fields` content is
    // asserted out-of-band via `received_requests()` below.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-789"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10003",
            "key": "PROJ-789",
            "fields": { "summary": "extra-fields test" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // GET /rest/api/3/field — returns one CMDB field so
    // `compose_extra_fields` includes a discovered id in addition to the
    // configured SP + team. Schema string matches
    // `CMDB_SCHEMA_TYPE` in `src/api/jira/fields.rs`.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": "customfield_12345",
                "name": "Affected Services",
                "custom": true,
                "schema": {
                    "type": "any",
                    "custom": "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"
                }
            }
        ])))
        .mount(&server)
        .await;

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "--no-input",
            "issue",
            "create",
            "--project",
            "PROJ",
            "--type",
            "Task",
            "--summary",
            "extra-fields test",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected success, stderr: {stderr}, stdout: {stdout}"
    );

    // Inspect the actual follow-up GET URL. `received_requests` returns all
    // requests the mock server saw; we filter for the GET on the issue and
    // assert `fields=...` contains each configured id. Comma-joined ordering
    // is an implementation detail of `get_issue` — membership is the contract.
    let requests = server.received_requests().await.expect("requests recorded");
    // Filter by path only: the POST goes to `/rest/api/3/issue` (no key
    // suffix), so any hit on `/rest/api/3/issue/PROJ-789` is the follow-up
    // GET by construction. Avoids importing wiremock's Method enum.
    let follow_up = requests
        .iter()
        .find(|r| r.url.path() == "/rest/api/3/issue/PROJ-789")
        .expect("follow-up GET must have been made");

    let fields_query = follow_up
        .url
        .query_pairs()
        .find(|(k, _)| k == "fields")
        .map(|(_, v)| v.into_owned())
        .expect("follow-up GET must carry a `fields` query parameter");

    for expected in [
        "customfield_10016", // SP (configured)
        "customfield_12345", // CMDB (discovered)
        "customfield_10001", // team (configured)
    ] {
        assert!(
            fields_query.split(',').any(|f| f == expected),
            "follow-up GET must request {expected}; got fields={fields_query}"
        );
    }
}
