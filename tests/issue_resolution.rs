use assert_cmd::Command;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[allow(dead_code)]
mod common;

#[tokio::test]
async fn issue_resolutions_json_output_lists_all_entries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/resolution"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "10000", "name": "Done", "description": "Work complete." },
            { "id": "10001", "name": "Won't Do" }
        ])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "resolutions", "--output", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().expect("expected JSON array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Done");
    assert_eq!(arr[1]["name"], "Won't Do");
}

#[tokio::test]
async fn issue_resolutions_table_output_prints_names() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/resolution"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "10000", "name": "Done", "description": "Work complete." }
        ])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "resolutions"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Done"),
        "expected table to show Done: {stdout}"
    );
    assert!(
        stdout.contains("Work complete"),
        "expected description column: {stdout}"
    );
}

#[tokio::test]
async fn issue_move_surfaces_resolution_required_hint() {
    let server = MockServer::start().await;

    // 1. transitions list — one terminal transition
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [
                {
                    "id": "31",
                    "name": "Done",
                    "to": { "name": "Done" }
                }
            ]
        })))
        .mount(&server)
        .await;

    // 2. issue GET — handle_move's current-status probe (for idempotency).
    //    Not in the plan's listed mocks, but the flow requires it to reach
    //    the transition POST; status "In Progress" is different from "Done"
    //    so the idempotent shortcut doesn't fire.
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

    // 3. transition POST — reject with Atlassian's real-world shape
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": [],
            "errors": {
                "resolution": "Field 'resolution' is required"
            }
        })))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["--no-input", "issue", "move", "FOO-1", "Done"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--resolution"),
        "error should mention --resolution flag: {stderr}"
    );
    assert!(
        stderr.contains("jr issue resolutions"),
        "error should point at `jr issue resolutions` for discovery: {stderr}"
    );
}
