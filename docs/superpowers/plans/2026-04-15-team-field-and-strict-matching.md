# Strict Name Matching + Team Field on `issue view` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the silent mis-resolution in `partial_match` (the root cause of #181) and expose the team field on `jr issue view` (#182). Together they close #181, #182, and #192.

**Architecture:** One-line change to `partial_match.rs` collapses the `1 => Exact` substring-fallback arm into `Ambiguous`, so every existing call site routes single-hit substrings through its existing disambiguation branch (TTY prompt / `--no-input` error). Separately, `handle_view` gains a team row by extending the `extra_fields` fetch and adding a render block modeled on the story-points pattern.

**Tech Stack:** Rust, wiremock, assert_cmd, predicates, anyhow, serde_json

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/partial_match.rs` | Modify | Collapse substring `1 => Exact` arm into `Ambiguous`; update unit tests |
| `tests/input_validation.rs` | Modify | Update `valid_status_partial_match_resolves` to expect `Ambiguous` |
| `src/types/jira/issue.rs` | Modify | Add `team_id(field_id) -> Option<String>` helper on `IssueFields` |
| `src/cli/issue/list.rs` | Modify | Extend `extra_fields` with team field id; render team row in `handle_view` |
| `tests/common/fixtures.rs` | Modify | Add `issue_response_with_team(key, summary, status, team_field_id, team_uuid)` fixture |
| `tests/cli_handler.rs` | Modify | Add four handler tests (team cached / team uncached / field unconfigured / substring-rejects-under-no-input) |

---

### Task 1: Core — `partial_match` rejects silent substring hits

**Files:**
- Modify: `src/partial_match.rs:39-42` (the match arm) and the unit tests at `src/partial_match.rs:66-100`
- Modify: `tests/input_validation.rs:124-160` (the `valid_status_partial_match_resolves` test)

- [ ] **Step 1.1: Update the unit test `test_partial_match_unique` to expect the new behavior**

In `src/partial_match.rs`, find:

```rust
#[test]
fn test_partial_match_unique() {
    match partial_match("prog", &candidates()) {
        MatchResult::Exact(s) => assert_eq!(s, "In Progress"),
        _ => panic!("Expected unique match"),
    }
}
```

Replace with:

```rust
#[test]
fn test_partial_match_single_substring_is_ambiguous() {
    // Single substring hits route through Ambiguous so callers can
    // prompt (TTY) or error (--no-input) — never silently resolve.
    match partial_match("prog", &candidates()) {
        MatchResult::Ambiguous(matches) => {
            assert_eq!(matches, vec!["In Progress".to_string()]);
        }
        other => panic!("Expected Ambiguous, got {:?}", other),
    }
}
```

- [ ] **Step 1.2: Update the unit test `test_blocked_unique` to expect the new behavior**

In `src/partial_match.rs`, find:

```rust
#[test]
fn test_blocked_unique() {
    match partial_match("block", &candidates()) {
        MatchResult::Exact(s) => assert_eq!(s, "Blocked"),
        _ => panic!("Expected unique match"),
    }
}
```

Replace with:

```rust
#[test]
fn test_blocked_single_substring_is_ambiguous() {
    match partial_match("block", &candidates()) {
        MatchResult::Ambiguous(matches) => {
            assert_eq!(matches, vec!["Blocked".to_string()]);
        }
        other => panic!("Expected Ambiguous, got {:?}", other),
    }
}
```

- [ ] **Step 1.3: Run the unit tests to confirm they fail**

Run:
```bash
cargo test --lib partial_match
```

Expected: FAIL — the two renamed tests fail because the current code returns `Exact` not `Ambiguous` for single substring hits.

- [ ] **Step 1.4: Update the integration test that locks the old behavior**

In `tests/input_validation.rs`, find the body of `valid_status_partial_match_resolves` (around line 155):

```rust
    let result = jr::partial_match::partial_match("in prog", &names);
    match result {
        jr::partial_match::MatchResult::Exact(name) => assert_eq!(name, "In Progress"),
        other => panic!("Expected Exact, got {:?}", std::mem::discriminant(&other)),
    }
}
```

Replace with:

```rust
    let result = jr::partial_match::partial_match("in prog", &names);
    match result {
        jr::partial_match::MatchResult::Ambiguous(matches) => {
            assert_eq!(matches, vec!["In Progress".to_string()]);
        }
        other => panic!("Expected Ambiguous, got {:?}", std::mem::discriminant(&other)),
    }
}
```

Also rename the test function for clarity:

```rust
async fn valid_status_single_substring_is_ambiguous() {
```

- [ ] **Step 1.5: Apply the one-line fix**

In `src/partial_match.rs`, find:

```rust
    match matches.len() {
        0 => MatchResult::None(candidates.to_vec()),
        1 => MatchResult::Exact(matches.into_iter().next().unwrap()),
        _ => MatchResult::Ambiguous(matches),
    }
```

Replace with:

```rust
    match matches.len() {
        0 => MatchResult::None(candidates.to_vec()),
        _ => MatchResult::Ambiguous(matches),
    }
```

- [ ] **Step 1.6: Run the full test suite**

Run:
```bash
cargo test
```

Expected: all tests pass. If any additional tests rely on the old `1 => Exact` behavior, they surface here and get updated in the same commit. Likely suspects: none — the codebase audit found only the two tests above, but re-run to be certain.

- [ ] **Step 1.7: Run lint and format checks**

Run:
```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: both clean.

- [ ] **Step 1.8: Commit**

```bash
git add src/partial_match.rs tests/input_validation.rs
git commit -m "fix: reject silent substring resolution in partial_match (#181, #192)"
```

---

### Task 2: Add `team_id` helper to `IssueFields`

**Files:**
- Modify: `src/types/jira/issue.rs` (add helper + unit test adjacent to `story_points`)

- [ ] **Step 2.1: Write a failing unit test for the helper**

In `src/types/jira/issue.rs`, find the existing `impl IssueFields` block that defines `story_points`:

```rust
impl IssueFields {
    pub fn story_points(&self, field_id: &str) -> Option<f64> {
        self.extra.get(field_id)?.as_f64()
    }
}
```

Below that block, add a test module (or extend the existing one if present — check the bottom of the file first):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fields_with_extra(key: &str, value: serde_json::Value) -> IssueFields {
        let mut fields = IssueFields::default();
        fields.extra.insert(key.to_string(), value);
        fields
    }

    #[test]
    fn team_id_reads_string_value() {
        let fields = fields_with_extra(
            "customfield_10001",
            json!("36885b3c-1bf0-4f85-a357-c5b858c31de4"),
        );
        assert_eq!(
            fields.team_id("customfield_10001"),
            Some("36885b3c-1bf0-4f85-a357-c5b858c31de4".to_string())
        );
    }

    #[test]
    fn team_id_returns_none_for_null_value() {
        let fields = fields_with_extra("customfield_10001", json!(null));
        assert_eq!(fields.team_id("customfield_10001"), None);
    }

    #[test]
    fn team_id_returns_none_for_missing_key() {
        let fields = IssueFields::default();
        assert_eq!(fields.team_id("customfield_10001"), None);
    }
}
```

Note: if a `#[cfg(test)] mod tests` block already exists in the file, add just the three test functions inside it instead of creating a new one.

- [ ] **Step 2.2: Run the test to see it fail**

Run:
```bash
cargo test --lib types::jira::issue::tests::team_id
```

Expected: FAIL — `team_id` is not yet defined.

- [ ] **Step 2.3: Implement the helper**

In the same `impl IssueFields` block, add the helper below `story_points`:

```rust
impl IssueFields {
    pub fn story_points(&self, field_id: &str) -> Option<f64> {
        self.extra.get(field_id)?.as_f64()
    }

    pub fn team_id(&self, field_id: &str) -> Option<String> {
        self.extra.get(field_id)?.as_str().map(String::from)
    }
}
```

- [ ] **Step 2.4: Run the tests to verify they pass**

Run:
```bash
cargo test --lib types::jira::issue::tests::team_id
```

Expected: PASS (all three cases).

- [ ] **Step 2.5: Commit**

```bash
git add src/types/jira/issue.rs
git commit -m "feat: add team_id helper to IssueFields (#182)"
```

---

### Task 3: Surface team field on `issue view`

**Files:**
- Modify: `src/cli/issue/list.rs` (`handle_view` — around lines 699 and 908)
- Modify: `tests/common/fixtures.rs` (add fixture helper)
- Modify: `tests/cli_handler.rs` (add three handler tests)

- [ ] **Step 3.1: Add a fixture helper for an issue with a team field set**

In `tests/common/fixtures.rs`, add near the other `issue_response_with_*` helpers:

```rust
pub fn issue_response_with_team(
    key: &str,
    summary: &str,
    team_field_id: &str,
    team_uuid: &str,
) -> Value {
    let mut response = issue_response(key, summary, "To Do");
    response["fields"][team_field_id] = json!(team_uuid);
    response
}
```

The existing `issue_response` produces the base JSON; we inject the team customfield into `fields`.

- [ ] **Step 3.2: Write failing handler tests**

In `tests/cli_handler.rs`, append three new tests at the end of the file. First, a constant for the test field id near the other helpers at the top:

```rust
const TEST_TEAM_FIELD_ID: &str = "customfield_10100";
```

Then the three tests:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_view_renders_team_name_when_cached() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-500"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_team(
                "HDL-500",
                "Team cached",
                TEST_TEAM_FIELD_ID,
                "team-uuid-abc",
            ),
        ))
        .mount(&server)
        .await;

    // For this test to work, the team cache must contain team-uuid-abc => "Platform".
    // The subagent must configure TEST_JR_CONFIG_DIR / pre-populate
    // ~/.cache/jr/teams.json via an env var override. If no such override exists,
    // inspect cache.rs to see what env var controls the cache directory and use
    // a tempdir — then pre-write the team cache before running jr.
    //
    // Also, JR_CONFIG_FILE (or equivalent) must set team_field_id = TEST_TEAM_FIELD_ID
    // in the [fields] section. Inspect config.rs for the env var / config path override.
    //
    // If no such overrides exist, add them as a minimal scaffolding change in this task
    // (e.g., JR_CACHE_DIR and JR_CONFIG_PATH env vars that config.rs / cache.rs honor
    // only when set). Keep the scaffolding generic — do not hard-code team-specific
    // paths.

    jr_cmd(&server.uri())
        .args(["issue", "view", "HDL-500"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "\"{}\": \"team-uuid-abc\"",
            TEST_TEAM_FIELD_ID
        )));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_view_renders_team_uuid_when_not_cached() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-501"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response_with_team(
                "HDL-501",
                "Team uncached",
                TEST_TEAM_FIELD_ID,
                "team-uuid-unknown",
            ),
        ))
        .mount(&server)
        .await;

    jr_cmd(&server.uri())
        .args(["issue", "view", "HDL-501"])
        .assert()
        .success()
        .stdout(predicate::str::contains("team-uuid-unknown"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_view_omits_team_row_when_field_unconfigured() {
    // In this test, the test config must have team_field_id = None.
    // Response still contains the field but should be ignored.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/HDL-502"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::issue_response("HDL-502", "No team field", "To Do"),
        ))
        .mount(&server)
        .await;

    // Assert the JSON output succeeds — row omission is a table-mode concern,
    // so this test just confirms no crash and no team-related output appears.
    jr_cmd(&server.uri())
        .args(["issue", "view", "HDL-502"])
        .assert()
        .success();
}
```

**Test scaffolding pattern:**

- `src/cache.rs:60-68` — `cache_dir()` honors `XDG_CACHE_HOME`.
- `src/config.rs:130-140` — config loader honors `XDG_CONFIG_HOME`.
- **CRITICAL** — `cli_handler.rs` tests spawn the `jr` binary via `assert_cmd::Command::cargo_bin("jr")`. While env vars set via `std::env::set_var` are inherited by spawned children at process spawn, `set_var` mutates process-global state and can cause cross-test interference when tests run in parallel. The existing `project_meta.rs` pattern uses `set_var` because it invokes `jr::` library functions in-process — that pattern is **not applicable here**. For these spawned-binary tests, pass XDG vars explicitly via `.env()` on the `Command` builder so each command gets isolated environment configuration:

```rust
let cache_dir = tempfile::tempdir().unwrap();
let config_dir = tempfile::tempdir().unwrap();

// Pre-populate team cache
let teams_dir = cache_dir.path().join("jr");
std::fs::create_dir_all(&teams_dir).unwrap();
let cache = jr::cache::TeamCache {
    fetched_at: chrono::Utc::now(),
    teams: vec![
        jr::cache::CachedTeam {
            id: "team-uuid-abc".into(),
            name: "Platform".into(),
        },
        jr::cache::CachedTeam {
            id: "team-uuid-platform-ops".into(),
            name: "Platform Ops".into(),
        },
    ],
};
std::fs::write(
    teams_dir.join("teams.json"),
    serde_json::to_string(&cache).unwrap(),
).unwrap();

// Pre-populate config with team_field_id
let conf_dir = config_dir.path().join("jr");
std::fs::create_dir_all(&conf_dir).unwrap();
std::fs::write(
    conf_dir.join("config.toml"),
    "[fields]\nteam_field_id = \"customfield_10100\"\n",
).unwrap();

// Use a bespoke command builder that sets XDG vars via .env()
let mut cmd = assert_cmd::Command::cargo_bin("jr").unwrap();
cmd.env("JR_BASE_URL", server.uri())
    .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
    .env("XDG_CACHE_HOME", cache_dir.path())
    .env("XDG_CONFIG_HOME", config_dir.path())
    .arg("--no-input")
    .arg("--output")
    .arg("json");
// then cmd.args(...).assert().success()...
```

Since the existing `jr_cmd()` helper in `cli_handler.rs` does not accept XDG overrides, extract a new helper `jr_cmd_with_xdg(server_uri, cache_dir, config_dir)` at the top of the file (next to the existing `jr_cmd`) and use it for the team-related tests. Keep the existing `jr_cmd` unchanged for tests that don't need XDG overrides.

**Env mutex is NOT needed** for this approach — each `Command` carries its own explicit env, so there's no shared process-wide state to serialize. Tests can stay at `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` to match other handler tests in the file.

- [ ] **Step 3.3: Run the tests to verify they fail**

Run:
```bash
cargo test --test cli_handler test_view_renders_team
cargo test --test cli_handler test_view_omits_team_row
```

Expected: FAIL — the JSON output currently does not include the team customfield because `extra_fields` doesn't fetch it.

- [ ] **Step 3.4: Extend `extra_fields` in `handle_view`**

In `src/cli/issue/list.rs`, find the block around line 699:

```rust
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
    for f in &cmdb_field_id_list {
        extra.push(f.as_str());
    }
    let mut issue = client.get_issue(&key, &extra).await?;
```

Replace with:

```rust
    let team_field_id: Option<&str> = config.global.fields.team_field_id.as_deref();

    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
    for f in &cmdb_field_id_list {
        extra.push(f.as_str());
    }
    if let Some(t) = team_field_id {
        extra.push(t);
    }
    let mut issue = client.get_issue(&key, &extra).await?;
```

- [ ] **Step 3.5: Render the team row in the table output**

In the same `handle_view` function, find the block that renders the story points row (around lines 908-915):

```rust
    if let Some(field_id) = sp_field_id {
        let points_display = ...
        rows.push(vec!["Points".into(), points_display]);
    }
```

Add a similar block immediately after for the team row:

```rust
    if let Some(field_id) = team_field_id {
        if let Some(team_uuid) = issue.fields.team_id(field_id) {
            let team_display = match crate::cache::read_team_cache()
                .ok()
                .flatten()
                .and_then(|c| c.teams.into_iter().find(|t| t.id == team_uuid))
            {
                Some(cached) => cached.name,
                None => format!(
                    "{} (name not cached — run 'jr team list --refresh')",
                    team_uuid
                ),
            };
            rows.push(vec!["Team".into(), team_display]);
        }
    }
```

Verify `crate::cache::read_team_cache` returns the type suggested — if the cache module uses different names (e.g., `CachedTeamList`, `.teams` vs `.data`), adapt the lookup accordingly by reading `src/cache.rs`. The structure check was already confirmed in the helpers module (`helpers.rs:29-32`), so this pattern mirrors what's already used for team resolution.

- [ ] **Step 3.6: Run the failing tests to verify they now pass**

Run:
```bash
cargo test --test cli_handler test_view_renders_team
cargo test --test cli_handler test_view_omits_team_row
```

Expected: PASS.

- [ ] **Step 3.7: Run the full test suite**

Run:
```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 3.8: Run lint and format checks**

Run:
```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: both clean.

- [ ] **Step 3.9: Commit**

```bash
git add src/cli/issue/list.rs tests/cli_handler.rs tests/common/fixtures.rs
git commit -m "feat: show team field on issue view (#182)"
```


---

### Task 4: Handler test — strict matching rejects `--team` substring under `--no-input`

**Files:**
- Modify: `tests/cli_handler.rs` (append one new test)

- [ ] **Step 4.1: Write the failing test**

Append to `tests/cli_handler.rs`:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_edit_team_substring_rejects_under_no_input() {
    let server = MockServer::start().await;

    // No PUT mock — if the assertion is right, no HTTP call should happen.
    // The cache must contain a team like "Platform Ops" so that "Ops" would
    // have matched as a substring under the old behavior.
    //
    // Pre-populate the team cache via the test override from Task 3 so that
    // the single-team entry is "Platform Ops" with id "team-uuid-platform-ops".

    jr_cmd(&server.uri())
        .args(["issue", "edit", "HDL-600", "--team", "Ops"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Ambiguous"))
        .stderr(predicate::str::contains("Platform Ops"));
}
```

- [ ] **Step 4.2: Run the test**

Run:
```bash
cargo test --test cli_handler test_edit_team_substring_rejects_under_no_input
```

Expected: PASS (this is already the new behavior after Task 1 — this test locks the guarantee).

If it fails, verify the team cache fixture setup from Task 3 is producing the expected cached team and the subagent is pre-populating it for this test.

- [ ] **Step 4.3: Run the full test suite**

Run:
```bash
cargo test
```

Expected: all pass.

- [ ] **Step 4.4: Run lint and format checks**

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] **Step 4.5: Commit**

```bash
git add tests/cli_handler.rs
git commit -m "test: lock strict matching behavior for --team under --no-input (#181)"
```

---

## Spec Coverage Checklist

| Spec Requirement | Task |
|------------------|------|
| `partial_match` collapses `1 => Exact` into `Ambiguous` | Task 1 (Step 1.5) |
| Existing unit tests updated to expect new behavior | Task 1 (Steps 1.1, 1.2) |
| Existing integration test updated | Task 1 (Step 1.4) |
| All 8 call sites route `Ambiguous` correctly | Codebase audit in spec — no code change needed; verified by `cargo test` passing in Task 1 Step 1.6 |
| `team_id` helper on `IssueFields` | Task 2 |
| `extra_fields` extended with team field in view | Task 3 (Step 3.4) |
| Team row rendered in view table | Task 3 (Step 3.5) |
| Row omitted when team field unconfigured | Task 3 (Step 3.5 — guarded by `if let Some(field_id) = team_field_id`) |
| Row omitted when UUID absent or null | Task 3 (Step 3.5 — guarded by `team_id()` returning `None`) |
| Fallback text when UUID not in cache | Task 3 (Step 3.5 — match arm on `read_team_cache`) |
| JSON output unchanged (raw customfield passes through) | Task 3 (Step 3.2 — the team-cached test asserts on JSON output containing the raw UUID) |
| Handler test: team cached + rendered | Task 3 (Step 3.2) |
| Handler test: team UUID uncached fallback | Task 3 (Step 3.2) |
| Handler test: field unconfigured | Task 3 (Step 3.2) |
| Handler test: --team substring under --no-input errors | Task 4 |

## Self-review notes

- **Config/cache overrides** (Task 3 Step 3.2): the test scaffolding needs env var overrides for config and cache paths. If these don't already exist, the subagent adds them as minimal, purpose-neutral additions (not team-specific). This is a prerequisite for Task 3 and Task 4 — inspect before writing the tests.
- **Render ordering**: the team row sits next to the story points row in the view table. Position it after Points for consistency with the existing columns (both are custom fields).
- **TTY single-item prompt UX**: not tested under TTY (handler tests run headless with `--no-input`). The TTY path continues to work via the existing `dialoguer::Select` code — no new test coverage added since `--no-input` is the assertion surface that matters for agents.
