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
