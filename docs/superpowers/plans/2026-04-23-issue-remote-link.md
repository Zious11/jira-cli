# `jr issue remote-link` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add a `jr issue remote-link <KEY> --url <URL> [--title <TITLE>]` subcommand that attaches a remote link (URL+title) to a Jira issue via `POST /rest/api/3/issue/{key}/remotelink`. MVP — create only, no list/delete/update.

**Architecture:** Mirrors the existing `jr issue link` pattern: clap enum variant in `cli/mod.rs`, dispatch in `cli/issue/mod.rs`, handler in `cli/issue/links.rs`, API wrapper in `api/jira/links.rs`, serde types in `types/jira/links.rs` (or extend existing).

**Tech Stack:** reqwest (existing JiraClient), serde_json, wiremock for integration tests, clap derive.

**Spec:** `docs/specs/issue-remote-link.md`

---

## Task 1: Add `CreateRemoteLinkResponse` type + `JiraClient::create_remote_link` wrapper

**Files:**
- Modify or add: `src/types/jira/links.rs` OR `src/types/jira/issue.rs` — whichever already exports `CreateIssueLinkResponse` / `IssueLink`. Grep first.
- Modify: `src/api/jira/links.rs` — add `create_remote_link`.
- Unit test: inside one of the above via `#[cfg(test)]`.

- [ ] **Step 1: Write the failing unit test**

In `src/api/jira/links.rs` (or wherever the module tests live), add a failing test that:
- Uses `JiraClient::new_for_test` against a `wiremock::MockServer`.
- Mounts `POST /rest/api/3/issue/PROJ-1/remotelink` returning status 201 with body `{"id": 10000, "self": "https://tenant.atlassian.net/rest/api/2/issue/PROJ-1/remotelink/10000"}`.
- Asserts the request body sent was `{"object": {"url": "https://example.com", "title": "Example"}}`.
- Asserts the deserialized `CreateRemoteLinkResponse` has `id == 10000` and `self_url == "https://..."`.

Run: `cargo test --lib api::jira::links::tests` → FAIL (type + method not defined).

- [ ] **Step 2: Add the type**

```rust
// in src/types/jira/issue.rs (alongside CreateIssueResponse):

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateRemoteLinkResponse {
    pub id: u64,
    #[serde(rename = "self")]
    pub self_url: String,
}
```

Export from `src/types/jira/mod.rs` if that file re-exports types.

- [ ] **Step 3: Add the API wrapper**

In `src/api/jira/links.rs`:

```rust
pub async fn create_remote_link(
    &self,
    issue_key: &str,
    url: &str,
    title: &str,
) -> Result<CreateRemoteLinkResponse> {
    let path = format!(
        "/rest/api/3/issue/{}/remotelink",
        urlencoding::encode(issue_key)
    );
    let body = serde_json::json!({
        "object": { "url": url, "title": title }
    });
    self.post(&path, &body).await
}
```

Match the signature style of the sibling `create_issue_link` above it.

- [ ] **Step 4: Run the test, confirm green**

`cargo test --lib api::jira::links::tests` → PASS.

- [ ] **Step 5: Full CI set**

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All green.

- [ ] **Step 6: Commit**

```bash
git add src/types/jira/issue.rs src/api/jira/links.rs
git commit -m "feat(api): add create_remote_link wrapper for POST /issue/{key}/remotelink"
```

---

## Task 2: CLI enum variant + handler + dispatch

**Files:**
- Modify: `src/cli/mod.rs` — add `IssueCommand::RemoteLink { key, url, title }` variant.
- Modify: `src/cli/issue/mod.rs` — dispatch `RemoteLink` to handler.
- Modify: `src/cli/issue/links.rs` — new `handle_remote_link`.
- Modify: `src/cli/issue/json_output.rs` — add `remote_link_response(...)` helper.

- [ ] **Step 1: Read the existing `IssueCommand::Link` variant** in `src/cli/mod.rs` to match the clap attributes, long-help style, and visibility.

- [ ] **Step 2: Add the variant**

```rust
/// Link a Confluence page or arbitrary web URL to an issue as a remote link.
/// Renders under the issue's "Web links" (or "Confluence pages") panel.
RemoteLink {
    /// Issue key (e.g. PROJ-123).
    key: String,

    /// URL to link to.
    #[arg(long)]
    url: String,

    /// Label shown in the Jira UI. Defaults to the URL when omitted.
    #[arg(long)]
    title: Option<String>,
},
```

- [ ] **Step 3: Add dispatch in `src/cli/issue/mod.rs`**

Mirror the existing `IssueCommand::Link` dispatch arm:

```rust
IssueCommand::RemoteLink { .. } => {
    links::handle_remote_link(command, output_format, client).await
}
```

Note: no `no_input` param — this command has no prompts.

- [ ] **Step 4: Add `handle_remote_link` in `src/cli/issue/links.rs`**

```rust
pub(super) async fn handle_remote_link(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::RemoteLink { key, url, title } = command else {
        unreachable!()
    };

    // Default the title to the URL for script-friendly single-flag invocation.
    let title = title.unwrap_or_else(|| url.clone());

    let response = client.create_remote_link(&key, &url, &title).await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json_output::remote_link_response(
                    &key,
                    response.id,
                    &url,
                    &title,
                    &response.self_url,
                ))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Linked {} → {} (id: {})",
                url, key, response.id
            ));
        }
    }

    Ok(())
}
```

- [ ] **Step 5: Add `remote_link_response` helper in `src/cli/issue/json_output.rs`**

```rust
pub(super) fn remote_link_response(
    key: &str,
    id: u64,
    url: &str,
    title: &str,
    self_url: &str,
) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "id": id,
        "url": url,
        "title": title,
        "self": self_url,
    })
}
```

- [ ] **Step 6: Full CI set**

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 7: Commit**

```bash
git add src/cli/mod.rs src/cli/issue/mod.rs src/cli/issue/links.rs src/cli/issue/json_output.rs
git commit -m "feat(issue): jr issue remote-link subcommand (#199)"
```

---

## Task 3: Integration tests

**Files:**
- Create: `tests/issue_remote_link.rs` — new wiremock integration test file.

- [ ] **Step 1: Read existing conventions**

Inspect `tests/issue_commands.rs` or `tests/issue_create_json.rs` for fixture patterns — env vars, tempdir XDG isolation, `assert_cmd::Command::cargo_bin("jr")` invocation style.

- [ ] **Step 2: Write 3 tests**

```rust
mod common;

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn remote_link_creates_with_explicit_title() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-123/remotelink"))
        .and(body_partial_json(serde_json::json!({
            "object": { "url": "https://example.com", "title": "Example" }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 10000,
            "self": format!("{}/rest/api/2/issue/PROJ-123/remotelink/10000", server.uri())
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let cwd_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .current_dir(cwd_dir.path())
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args([
            "issue", "remote-link", "PROJ-123",
            "--url", "https://example.com",
            "--title", "Example",
            "--output", "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["key"], "PROJ-123");
    assert_eq!(parsed["id"], 10000);
    assert_eq!(parsed["url"], "https://example.com");
    assert_eq!(parsed["title"], "Example");
    assert!(parsed["self"].as_str().unwrap().ends_with("/remotelink/10000"));
}

#[tokio::test]
async fn remote_link_defaults_title_to_url() {
    let server = MockServer::start().await;

    // Key assertion: the POST body should use the URL as the title when --title is omitted.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-124/remotelink"))
        .and(body_partial_json(serde_json::json!({
            "object": { "url": "https://example.com/page", "title": "https://example.com/page" }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 10001,
            "self": format!("{}/rest/api/2/issue/PROJ-124/remotelink/10001", server.uri())
        })))
        .expect(1)
        .mount(&server)
        .await;

    // ...env setup identical...

    let output = Command::cargo_bin("jr")
        .unwrap()
        // env vars ...
        .args([
            "issue", "remote-link", "PROJ-124",
            "--url", "https://example.com/page",
            "--output", "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let parsed: Value = serde_json::from_str(&String::from_utf8(output.stdout).unwrap()).unwrap();
    assert_eq!(parsed["title"], "https://example.com/page");
}

#[tokio::test]
async fn remote_link_surfaces_server_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-999/remotelink"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": ["Issue does not exist or you do not have permission to see it."],
            "errors": {}
        })))
        .expect(1)
        .mount(&server)
        .await;

    // ...env setup identical...

    let output = Command::cargo_bin("jr")
        .unwrap()
        // env vars ...
        .args([
            "issue", "remote-link", "PROJ-999",
            "--url", "https://example.com",
            "--output", "json",
            "--no-input",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "expected failure on 400");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Issue does not exist") || stderr.contains("400"),
        "server error should be surfaced in stderr: {stderr}"
    );
}
```

- [ ] **Step 3: Run + iterate**

`cargo test --test issue_remote_link` → all 3 pass.

- [ ] **Step 4: Full CI set**

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 5: Commit**

```bash
git add tests/issue_remote_link.rs
git commit -m "test(issue): wiremock coverage for jr issue remote-link"
```

---

## Task 4: README + help-text polish

**Files:**
- Modify: `README.md` — add one-line entry under the commands table.

- [ ] **Step 1: Grep README for the commands table**

`grep -n "jr issue link" README.md`. Insert `jr issue remote-link` row immediately after.

- [ ] **Step 2: Verify `jr issue remote-link --help`** renders nicely.

```
cargo run -- issue remote-link --help
```

- [ ] **Step 3: Commit (only if README was touched)**

```bash
git add README.md
git commit -m "docs: mention jr issue remote-link in commands table"
```

---

## Task 5: Final checks

- [ ] **Step 1: Full CI-equivalent set**
```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 2: Declare done.** Branch ready for local review + PR.
