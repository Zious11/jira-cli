# Common Filter Flags Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--assignee`, `--reporter`, and `--recent` filter flags to `jr issue list` that compose additively with each other and with `--jql`.

**Architecture:** Three new `Option<String>` flags on `IssueCommand::List`. A `validate_duration()` function in `jql.rs` for client-side `--recent` validation. A `resolve_user()` helper in `helpers.rs` using the user search API + `partial_match` for disambiguation. A `search_users()` method on `JiraClient`. The JQL construction in `handle_list()` is refactored to a unified flow where all flags (including `--jql`) compose via AND. The implicit `assignee = currentUser()` in scrum/kanban paths is removed.

**Tech Stack:** Rust, clap 4 (derive), reqwest, serde, wiremock (tests)

**Spec:** `docs/superpowers/specs/2026-03-24-common-filter-flags-design.md`

---

## File Structure

| File | Responsibility | Change |
|------|---------------|--------|
| `src/jql.rs` | JQL utilities | Add `validate_duration()` |
| `src/api/jira/users.rs` | User API methods | Add `search_users()` |
| `src/cli/mod.rs` | CLI argument definitions | Add `assignee`, `reporter`, `recent` to `IssueCommand::List` |
| `src/cli/issue/helpers.rs` | Issue command helpers | Add `resolve_user()` |
| `src/cli/issue/list.rs` | Issue list handler | Refactor JQL construction, remove implicit `assignee = currentUser()`, compose all flags, integrate new flags |
| `tests/issue_commands.rs` | Integration tests | Add user search + filter composition tests |
| `tests/common/fixtures.rs` | Test fixtures | Add `user_search_response()` |

**Not changed:** `src/cli/issue/assets.rs` (does not call `search_issues`), `src/config.rs`, `src/cache.rs`

---

### Task 1: Add `validate_duration()` to `src/jql.rs`

**Files:**
- Modify: `src/jql.rs`

- [ ] **Step 1: Write unit tests**

Add inside the first `mod tests` block in `src/jql.rs`, after the `strip_order_by_trims_whitespace` test:

```rust
    #[test]
    fn validate_duration_valid_days() {
        assert!(validate_duration("7d").is_ok());
    }

    #[test]
    fn validate_duration_valid_weeks() {
        assert!(validate_duration("4w").is_ok());
    }

    #[test]
    fn validate_duration_valid_months_uppercase() {
        assert!(validate_duration("2M").is_ok());
    }

    #[test]
    fn validate_duration_valid_years() {
        assert!(validate_duration("1y").is_ok());
    }

    #[test]
    fn validate_duration_valid_hours() {
        assert!(validate_duration("5h").is_ok());
    }

    #[test]
    fn validate_duration_valid_minutes() {
        assert!(validate_duration("10m").is_ok());
    }

    #[test]
    fn validate_duration_valid_zero() {
        assert!(validate_duration("0d").is_ok());
    }

    #[test]
    fn validate_duration_invalid_unit() {
        assert!(validate_duration("7x").is_err());
    }

    #[test]
    fn validate_duration_reversed() {
        assert!(validate_duration("d7").is_err());
    }

    #[test]
    fn validate_duration_empty() {
        assert!(validate_duration("").is_err());
    }

    #[test]
    fn validate_duration_combined_units() {
        assert!(validate_duration("4w2d").is_err());
    }

    #[test]
    fn validate_duration_no_digits() {
        assert!(validate_duration("d").is_err());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib validate_duration`
Expected: FAIL — `validate_duration` not found

- [ ] **Step 3: Implement `validate_duration()`**

Add to `src/jql.rs` after the `strip_order_by` function, before the first `#[cfg(test)]`:

```rust
/// Validate a JQL relative date duration string.
///
/// JQL relative dates use the format `<digits><unit>` where unit is one of:
/// `y` (years), `M` (months), `w` (weeks), `d` (days), `h` (hours), `m` (minutes).
/// Units are case-sensitive — `M` is months, `m` is minutes.
/// Combined units like `4w2d` are not supported by Jira.
pub fn validate_duration(s: &str) -> Result<(), String> {
    if s.len() < 2 {
        return Err(format!(
            "Invalid duration '{s}'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M)."
        ));
    }
    let (digits, unit) = s.split_at(s.len() - 1);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "Invalid duration '{s}'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M)."
        ));
    }
    if !matches!(unit, "y" | "M" | "w" | "d" | "h" | "m") {
        return Err(format!(
            "Invalid duration '{s}'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M)."
        ));
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib validate_duration`
Expected: PASS (12 tests)

- [ ] **Step 5: Commit**

```bash
git add src/jql.rs
git commit -m "feat: add validate_duration for JQL relative date validation"
```

---

### Task 2: Add `search_users()` API method

**Files:**
- Modify: `src/api/jira/users.rs`

- [ ] **Step 1: Write the `search_users()` method**

The user search endpoint response format is uncertain (may be flat array `[User, ...]` or paginated `{ values: [...] }`). Add a method that handles both by using `serde_json::Value` and extracting users manually:

```rust
    /// Search for users by name or email prefix.
    ///
    /// Returns active and inactive users — caller should filter by `active` field.
    pub async fn search_users(&self, query: &str) -> Result<Vec<User>> {
        let path = format!(
            "/rest/api/3/user/search?query={}",
            urlencoding::encode(query)
        );
        // The endpoint may return a flat array or a paginated object with "values".
        let raw: serde_json::Value = self.get(&path).await?;
        let users: Vec<User> = if raw.is_array() {
            serde_json::from_value(raw)?
        } else if let Some(values) = raw.get("values") {
            serde_json::from_value(values.clone())?
        } else {
            Vec::new()
        };
        Ok(users)
    }
```

Note: `serde_json::Value` and `serde_json::from_value` are used fully qualified — no additional `use` statement needed. Tests for `search_users()` are deferred to Task 5 because it's a thin HTTP wrapper requiring wiremock.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Commit**

```bash
git add src/api/jira/users.rs
git commit -m "feat: add search_users API method for user name lookup"
```

---

### Task 3: Add `resolve_user()` helper

**Files:**
- Modify: `src/cli/issue/helpers.rs`

- [ ] **Step 1: Write unit test for `me` keyword resolution**

Add at the bottom of `src/cli/issue/helpers.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_me_keyword_lowercase() {
        assert!(is_me_keyword("me"));
    }

    #[test]
    fn is_me_keyword_uppercase() {
        assert!(is_me_keyword("ME"));
    }

    #[test]
    fn is_me_keyword_mixed_case() {
        assert!(is_me_keyword("Me"));
    }

    #[test]
    fn is_me_keyword_not_me() {
        assert!(!is_me_keyword("Jane"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib is_me_keyword`
Expected: FAIL — `is_me_keyword` not found

- [ ] **Step 3: Add `is_me_keyword()` helper and `resolve_user()` function**

Add to `src/cli/issue/helpers.rs`:

```rust
/// Check if a user input string is the "me" keyword (case-insensitive).
fn is_me_keyword(input: &str) -> bool {
    input.eq_ignore_ascii_case("me")
}

/// Resolve a user flag value to a JQL fragment.
///
/// - `"me"` (case-insensitive) → `"currentUser()"` (no API call)
/// - Any other value → search users API, filter active, disambiguate via partial_match
///
/// Returns the JQL value to use (either `"currentUser()"` or an unquoted accountId).
pub(super) async fn resolve_user(
    client: &JiraClient,
    name: &str,
    no_input: bool,
) -> Result<String> {
    if is_me_keyword(name) {
        return Ok("currentUser()".to_string());
    }

    let users = client.search_users(name).await?;
    let active_users: Vec<_> = users
        .into_iter()
        .filter(|u| u.active == Some(true))
        .collect();

    if active_users.is_empty() {
        anyhow::bail!(
            "No active user found matching \"{}\". The user may be deactivated.",
            name
        );
    }

    if active_users.len() == 1 {
        return Ok(active_users[0].account_id.clone());
    }

    // Multiple matches — disambiguate
    let display_names: Vec<String> = active_users.iter().map(|u| u.display_name.clone()).collect();
    match crate::partial_match::partial_match(name, &display_names) {
        crate::partial_match::MatchResult::Exact(matched_name) => {
            let user = active_users
                .iter()
                .find(|u| u.display_name == matched_name)
                .expect("matched name must exist in active_users");
            Ok(user.account_id.clone())
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            if no_input {
                anyhow::bail!(
                    "Multiple users match \"{}\": {}. Use a more specific name.",
                    name,
                    matches.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple users match \"{name}\""))
                .items(&matches)
                .interact()?;
            let selected_name = &matches[selection];
            let user = active_users
                .iter()
                .find(|u| &u.display_name == selected_name)
                .expect("selected name must exist in active_users");
            Ok(user.account_id.clone())
        }
        crate::partial_match::MatchResult::None(_) => {
            anyhow::bail!(
                "No active user found matching \"{}\". The user may be deactivated.",
                name
            );
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib is_me_keyword`
Expected: PASS (4 tests)

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 6: Commit**

```bash
git add src/cli/issue/helpers.rs
git commit -m "feat: add resolve_user helper for --assignee/--reporter name resolution"
```

---

### Task 4: Add CLI flags and refactor JQL construction

This is the core task. It adds the three new flags, refactors JQL construction to a unified flow, removes implicit `assignee = currentUser()`, and makes `--jql` compose with filter flags. All existing tests must be updated atomically.

**Files:**
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/issue/list.rs`

- [ ] **Step 1: Add three new flags to `IssueCommand::List`**

In `src/cli/mod.rs`, inside the `List` variant (after the `all` field at line 168), add:

```rust
        /// Filter by assignee ("me" for current user, or a name to search)
        #[arg(long)]
        assignee: Option<String>,
        /// Filter by reporter ("me" for current user, or a name to search)
        #[arg(long)]
        reporter: Option<String>,
        /// Show issues created within duration (e.g., 7d, 4w, 2M)
        #[arg(long)]
        recent: Option<String>,
```

- [ ] **Step 2: Write unit tests for JQL composition**

Add to the `mod tests` block in `src/cli/issue/list.rs`:

```rust
    #[test]
    fn build_jql_parts_assignee_me() {
        let parts = build_filter_clauses(
            Some("currentUser()"),  // assignee
            None,                    // reporter
            None,                    // status
            None,                    // team
            None,                    // recent
        );
        assert_eq!(parts, vec!["assignee = currentUser()"]);
    }

    #[test]
    fn build_jql_parts_reporter_account_id() {
        let parts = build_filter_clauses(
            None,
            Some("5b10ac8d82e05b22cc7d4ef5"),
            None,
            None,
            None,
        );
        assert_eq!(parts, vec!["reporter = 5b10ac8d82e05b22cc7d4ef5"]);
    }

    #[test]
    fn build_jql_parts_recent() {
        let parts = build_filter_clauses(None, None, None, None, Some("7d"));
        assert_eq!(parts, vec!["created >= -7d"]);
    }

    #[test]
    fn build_jql_parts_all_filters() {
        let parts = build_filter_clauses(
            Some("currentUser()"),
            Some("currentUser()"),
            Some("In Progress"),
            Some(r#"customfield_10001 = "uuid-123""#),
            Some("30d"),
        );
        assert_eq!(parts.len(), 5);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"reporter = currentUser()".to_string()));
        assert!(parts.contains(&"status = \"In Progress\"".to_string()));
        assert!(parts.contains(&r#"customfield_10001 = "uuid-123""#.to_string()));
        assert!(parts.contains(&"created >= -30d".to_string()));
    }

    #[test]
    fn build_jql_parts_empty() {
        let parts = build_filter_clauses(None, None, None, None, None);
        assert!(parts.is_empty());
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib build_jql_parts`
Expected: FAIL — `build_filter_clauses` not found

- [ ] **Step 4: Implement `build_filter_clauses()` and refactor `handle_list()`**

Add `build_filter_clauses()` to `src/cli/issue/list.rs` near the other helper functions:

```rust
/// Build JQL filter clauses from resolved flag values.
fn build_filter_clauses(
    assignee_jql: Option<&str>,
    reporter_jql: Option<&str>,
    status: Option<&str>,
    team_clause: Option<&str>,
    recent: Option<&str>,
) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(a) = assignee_jql {
        parts.push(format!("assignee = {a}"));
    }
    if let Some(r) = reporter_jql {
        parts.push(format!("reporter = {r}"));
    }
    if let Some(s) = status {
        parts.push(format!("status = \"{}\"", crate::jql::escape_value(s)));
    }
    if let Some(t) = team_clause {
        parts.push(t.to_string());
    }
    if let Some(d) = recent {
        parts.push(format!("created >= -{d}"));
    }
    parts
}
```

Now replace the entire JQL construction section of `handle_list()` (lines 28-131). Here is the complete refactored function from destructuring through `effective_jql`:

```rust
    let IssueCommand::List {
        jql,
        status,
        team,
        limit,
        all,
        assignee,
        reporter,
        recent,
        points: show_points,
        assets: show_assets,
    } = command
    else {
        unreachable!()
    };

    let effective_limit = resolve_effective_limit(limit, all);

    // Validate --recent duration format early
    if let Some(ref d) = recent {
        crate::jql::validate_duration(d).map_err(|e| JrError::UserError(e))?;
    }

    // Resolve --assignee and --reporter to JQL values
    let assignee_jql = if let Some(ref name) = assignee {
        Some(helpers::resolve_user(client, name, no_input).await?)
    } else {
        None
    };
    let reporter_jql = if let Some(ref name) = reporter {
        Some(helpers::resolve_user(client, name, no_input).await?)
    } else {
        None
    };

    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();

    // Resolve team name to (field_id, uuid) before building JQL
    let resolved_team = if let Some(ref team_name) = team {
        Some(helpers::resolve_team_field(config, client, team_name, no_input).await?)
    } else {
        None
    };

    // Build pre-formatted team clause for build_filter_clauses
    let team_clause = resolved_team.as_ref().map(|(field_id, team_uuid)| {
        format!(
            "{} = \"{}\"",
            field_id,
            crate::jql::escape_value(team_uuid)
        )
    });

    // Build filter clauses from all flag values
    let filter_parts = build_filter_clauses(
        assignee_jql.as_deref(),
        reporter_jql.as_deref(),
        status.as_deref(),
        team_clause.as_deref(),
        recent.as_deref(),
    );
    let has_filters = !filter_parts.is_empty();

    // Build base JQL + order by
    let (base_parts, order_by): (Vec<String>, &str) = if let Some(raw_jql) = jql {
        // --jql provided: use as base, filter clauses will be appended
        (vec![raw_jql], "updated DESC")
    } else {
        let board_id = config.project.board_id;
        let project_key = config.project_key(project_override);

        if let Some(bid) = board_id {
            match client.get_board_config(bid).await {
                Ok(board_config) => {
                    let board_type = board_config.board_type.to_lowercase();
                    if board_type == "scrum" {
                        match client.list_sprints(bid, Some("active")).await {
                            Ok(sprints) if !sprints.is_empty() => {
                                let sprint = &sprints[0];
                                (vec![format!("sprint = {}", sprint.id)], "rank ASC")
                            }
                            _ => {
                                // No active sprint — fall through to fallback
                                let mut parts = Vec::new();
                                if let Some(ref pk) = project_key {
                                    parts.push(format!(
                                        "project = \"{}\"",
                                        crate::jql::escape_value(pk)
                                    ));
                                }
                                (parts, "updated DESC")
                            }
                        }
                    } else {
                        // Kanban: statusCategory != Done, no implicit assignee
                        let mut parts = Vec::new();
                        if let Some(ref pk) = project_key {
                            parts.push(format!(
                                "project = \"{}\"",
                                crate::jql::escape_value(pk)
                            ));
                        }
                        parts.push("statusCategory != Done".into());
                        (parts, "rank ASC")
                    }
                }
                Err(_) => {
                    let mut parts = Vec::new();
                    if let Some(ref pk) = project_key {
                        parts.push(format!(
                            "project = \"{}\"",
                            crate::jql::escape_value(pk)
                        ));
                    }
                    (parts, "updated DESC")
                }
            }
        } else {
            let mut parts = Vec::new();
            if let Some(ref pk) = project_key {
                parts.push(format!(
                    "project = \"{}\"",
                    crate::jql::escape_value(pk)
                ));
            }
            (parts, "updated DESC")
        }
    };

    // Combine base + filters
    let mut all_parts = base_parts;
    all_parts.extend(filter_parts);

    // Guard against unbounded query
    if all_parts.is_empty() {
        return Err(JrError::UserError(
            "No project or filters specified. Use --project, --assignee, --reporter, --status, --team, or --recent. \
             You can also set a default project in .jr.toml or run \"jr init\"."
                .into(),
        )
        .into());
    }

    let where_clause = all_parts.join(" AND ");
    let effective_jql = format!("{where_clause} ORDER BY {order_by}");
```

The rest of `handle_list()` (from the `cmdb_field_ids` section onward) stays unchanged.

- [ ] **Step 5: Update existing tests and add new guard test**

The `build_fallback_jql` function is now replaced by the inline guard in `handle_list()`. Remove `build_fallback_jql` and all its tests (`fallback_jql_order_by_not_joined_with_and`, `fallback_jql_with_team_has_valid_order_by`, `fallback_jql_with_all_filters`, `fallback_jql_errors_when_no_filters`, `fallback_jql_with_status_only`, `fallback_jql_escapes_special_chars_in_status`).

The `build_filter_clauses` unit tests (Step 2) cover the JQL assembly logic that `build_fallback_jql` previously handled.

Add a test verifying the `--jql` + filter flag composition:

```rust
    #[test]
    fn build_jql_parts_jql_plus_status_compose() {
        // --jql "type = Bug" --status "Done" should AND together
        let filter = build_filter_clauses(
            None,
            None,
            Some("Done"),
            None,
            None,
        );
        // In handle_list, base_parts = ["type = Bug"], filter appended
        let mut all_parts = vec!["type = Bug".to_string()];
        all_parts.extend(filter);
        let jql = all_parts.join(" AND ");
        assert_eq!(jql, r#"type = Bug AND status = "Done""#);
    }
```

- [ ] **Step 6: Run all tests**

Run: `cargo test --all-features`
Expected: ALL PASS

- [ ] **Step 7: Run clippy**

Run: `cargo clippy --all --all-features --tests -- -D warnings`
Expected: No warnings

- [ ] **Step 8: Commit**

```bash
git add src/cli/mod.rs src/cli/issue/list.rs
git commit -m "feat: add --assignee, --reporter, --recent flags with unified JQL composition (#44)"
```

---

### Task 5: Integration tests

**Files:**
- Modify: `tests/common/fixtures.rs`
- Modify: `tests/issue_commands.rs`

- [ ] **Step 1: Add test fixture helper**

In `tests/common/fixtures.rs`, add:

```rust
/// User search response — flat array of User objects.
pub fn user_search_response(users: Vec<(&str, &str, bool)>) -> Value {
    let user_objects: Vec<Value> = users
        .into_iter()
        .map(|(account_id, display_name, active)| {
            json!({
                "accountId": account_id,
                "displayName": display_name,
                "emailAddress": format!("{}@test.com", display_name.to_lowercase().replace(' ', ".")),
                "active": active,
            })
        })
        .collect();
    json!(user_objects)
}
```

- [ ] **Step 2: Write integration tests**

In `tests/issue_commands.rs`, add:

```rust
#[tokio::test]
async fn test_search_users_single_result() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![
                ("acc-123", "Jane Doe", true),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users("Jane").await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].account_id, "acc-123");
    assert_eq!(users[0].display_name, "Jane Doe");
    assert_eq!(users[0].active, Some(true));
}

#[tokio::test]
async fn test_search_users_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                common::fixtures::user_search_response(vec![]),
            ),
        )
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users("Nobody").await.unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn test_search_users_multiple() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![
                ("acc-1", "Jane Doe", true),
                ("acc-2", "Jane Smith", true),
                ("acc-3", "Jane Inactive", false),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let users = client.search_users("Jane").await.unwrap();
    assert_eq!(users.len(), 3);
    // Caller is responsible for filtering active users
}
```

- [ ] **Step 3: Add `resolve_user()` integration tests**

In `tests/issue_commands.rs`, add:

```rust
#[tokio::test]
async fn test_resolve_user_me_keyword() {
    // "me" should return "currentUser()" without making any API call
    // No mock server needed — the function short-circuits
    let server = MockServer::start().await;
    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = jr::cli::issue::helpers::resolve_user(&client, "me", false).await.unwrap();
    assert_eq!(result, "currentUser()");
}

#[tokio::test]
async fn test_resolve_user_single_active_match() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![
                ("acc-123", "Jane Doe", true),
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = jr::cli::issue::helpers::resolve_user(&client, "Jane", true).await.unwrap();
    assert_eq!(result, "acc-123");
}

#[tokio::test]
async fn test_resolve_user_no_active_match() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            common::fixtures::user_search_response(vec![
                ("acc-123", "Jane Doe", false),  // inactive
            ]),
        ))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
    let result = jr::cli::issue::helpers::resolve_user(&client, "Jane", true).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No active user found"));
}
```

Note: `resolve_user()` needs to be `pub` (not just `pub(super)`) for integration tests to access it. Update visibility in `helpers.rs` if needed, or test through the CLI binary instead.

- [ ] **Step 4: Run integration tests**

Run: `cargo test --test issue_commands`
Expected: ALL PASS

- [ ] **Step 5: Run full test suite + clippy**

Run: `cargo test --all-features && cargo clippy --all --all-features --tests -- -D warnings`
Expected: ALL PASS, no warnings

- [ ] **Step 5: Commit**

```bash
git add tests/common/fixtures.rs tests/issue_commands.rs
git commit -m "test: add integration tests for user search and filter flags"
```

---

### Task 6: Final verification and format check

**Files:** None (verification only)

- [ ] **Step 1: Run cargo fmt**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues (if there are, run `cargo fmt --all` and include in commit)

- [ ] **Step 2: Run full CI-equivalent check**

Run: `cargo fmt --all -- --check && cargo clippy --all --all-features --tests -- -D warnings && cargo test --all-features`
Expected: All three pass

- [ ] **Step 3: Fix any issues found, commit if needed**

If `cargo fmt` requires changes:
```bash
cargo fmt --all
git add -u
git commit -m "style: format code"
```
