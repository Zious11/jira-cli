# Issue List `--asset` Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--asset <KEY>` filter to `jr issue list` that generates an `aqlFunction()` JQL clause, composable with all existing filters.

**Architecture:** CMDB field discovery is widened from ID-only to (ID, name) pairs so the JQL builder can reference fields by name (required by `aqlFunction()`). A new `build_asset_clause` function produces the AQL JQL fragment. The `--asset` flag auto-enables the existing `--assets` display column.

**Tech Stack:** Rust, clap, reqwest, wiremock (tests), serde

**Spec:** `docs/specs/issue-list-asset-filter.md`

---

### Task 1: Widen `filter_cmdb_fields` to return (id, name) pairs

**Files:**
- Modify: `src/api/jira/fields.rs:85-98` (filter_cmdb_fields)
- Modify: `src/api/jira/fields.rs:39-42` (find_cmdb_field_ids)

- [ ] **Step 1: Update existing unit tests for new return type**

In `src/api/jira/fields.rs`, update the four `filter_cmdb_fields_*` tests to expect `Vec<(String, String)>` tuples:

```rust
#[test]
fn filter_cmdb_fields_finds_assets_type() {
    let fields = vec![make_field(
        "customfield_10191",
        "Client",
        true,
        "any",
        "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
    )];
    let result = filter_cmdb_fields(&fields);
    assert_eq!(
        result,
        vec![("customfield_10191".to_string(), "Client".to_string())]
    );
}

#[test]
fn filter_cmdb_fields_ignores_non_cmdb() {
    let fields = vec![
        make_field(
            "customfield_10031",
            "Story Points",
            true,
            "number",
            "com.atlassian.jira.plugin.system.customfieldtypes:float",
        ),
        make_field(
            "customfield_10191",
            "Client",
            true,
            "any",
            "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
        ),
    ];
    let result = filter_cmdb_fields(&fields);
    assert_eq!(
        result,
        vec![("customfield_10191".to_string(), "Client".to_string())]
    );
}

#[test]
fn filter_cmdb_fields_empty_when_no_cmdb() {
    let fields = vec![make_field(
        "customfield_10031",
        "Story Points",
        true,
        "number",
        "com.atlassian.jira.plugin.system.customfieldtypes:float",
    )];
    let result: Vec<(String, String)> = filter_cmdb_fields(&fields);
    assert!(result.is_empty());
}

#[test]
fn filter_cmdb_fields_multiple() {
    let fields = vec![
        make_field(
            "customfield_10191",
            "Client",
            true,
            "any",
            "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
        ),
        make_field(
            "customfield_10245",
            "Server",
            true,
            "any",
            "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
        ),
    ];
    let result = filter_cmdb_fields(&fields);
    assert_eq!(
        result,
        vec![
            ("customfield_10191".to_string(), "Client".to_string()),
            ("customfield_10245".to_string(), "Server".to_string()),
        ]
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test filter_cmdb_fields -- --nocapture`
Expected: FAIL — return type mismatch.

- [ ] **Step 3: Update `filter_cmdb_fields` to return `(id, name)` tuples**

In `src/api/jira/fields.rs`, change the function:

```rust
pub fn filter_cmdb_fields(fields: &[Field]) -> Vec<(String, String)> {
    fields
        .iter()
        .filter(|f| {
            f.custom == Some(true)
                && f.schema
                    .as_ref()
                    .and_then(|s| s.custom.as_deref())
                    .map(|c| c == CMDB_SCHEMA_TYPE)
                    .unwrap_or(false)
        })
        .map(|f| (f.id.clone(), f.name.clone()))
        .collect()
}
```

Also update `find_cmdb_field_ids` to match:

```rust
pub async fn find_cmdb_field_ids(&self) -> Result<Vec<(String, String)>> {
    let fields = self.list_fields().await?;
    Ok(filter_cmdb_fields(&fields))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test filter_cmdb_fields -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/jira/fields.rs
git commit -m "refactor: widen filter_cmdb_fields to return (id, name) pairs (#88)"
```

---

### Task 2: Update cache to store (id, name) pairs

**Files:**
- Modify: `src/cache.rs:152-187` (CmdbFieldsCache, read/write functions)

- [ ] **Step 1: Update cache unit tests for new type**

In `src/cache.rs`, update the `write_then_read_cmdb_fields_cache` test:

```rust
#[test]
fn write_then_read_cmdb_fields_cache() {
    with_temp_cache(|| {
        write_cmdb_fields_cache(&[
            ("customfield_10191".into(), "Client".into()),
            ("customfield_10245".into(), "Server".into()),
        ])
        .unwrap();

        let cache = read_cmdb_fields_cache().unwrap().expect("should exist");
        assert_eq!(
            cache.fields,
            vec![
                ("customfield_10191".to_string(), "Client".to_string()),
                ("customfield_10245".to_string(), "Server".to_string()),
            ]
        );
    });
}
```

And the `expired_cmdb_fields_cache_returns_none` test:

```rust
#[test]
fn expired_cmdb_fields_cache_returns_none() {
    with_temp_cache(|| {
        let expired = CmdbFieldsCache {
            fields: vec![("customfield_10191".into(), "Client".into())],
            fetched_at: Utc::now() - chrono::Duration::days(8),
        };
        let dir = cache_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let content = serde_json::to_string_pretty(&expired).unwrap();
        std::fs::write(dir.join("cmdb_fields.json"), content).unwrap();

        let result = read_cmdb_fields_cache().unwrap();
        assert!(
            result.is_none(),
            "expired cmdb fields cache should return None"
        );
    });
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test cmdb_fields_cache -- --nocapture`
Expected: FAIL — `field_ids` does not exist, `fields` does not exist.

- [ ] **Step 3: Update `CmdbFieldsCache` and read/write functions**

In `src/cache.rs`:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CmdbFieldsCache {
    pub fields: Vec<(String, String)>,
    pub fetched_at: DateTime<Utc>,
}

pub fn read_cmdb_fields_cache() -> Result<Option<CmdbFieldsCache>> {
    let path = cache_dir().join("cmdb_fields.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: CmdbFieldsCache = serde_json::from_str(&content)?;

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(Some(cache))
}

pub fn write_cmdb_fields_cache(fields: &[(String, String)]) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let cache = CmdbFieldsCache {
        fields: fields.to_vec(),
        fetched_at: Utc::now(),
    };

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(dir.join("cmdb_fields.json"), content)?;
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test cmdb_fields_cache -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/cache.rs
git commit -m "refactor: update CmdbFieldsCache to store (id, name) pairs (#88)"
```

---

### Task 3: Update `get_or_fetch_cmdb_field_ids` and callers

**Files:**
- Modify: `src/api/assets/linked.rs:12-19` (get_or_fetch_cmdb_field_ids)
- Modify: `src/cli/issue/list.rs:261-276` (show_assets block)
- Modify: `src/cli/issue/list.rs:507-509` (handle_view)
- Modify: `src/cli/issue/assets.rs:16` (handle_issue_assets)

- [ ] **Step 1: Update `get_or_fetch_cmdb_field_ids` return type**

In `src/api/assets/linked.rs`, change:

```rust
/// Get CMDB fields (id, name pairs), using cache when available.
pub async fn get_or_fetch_cmdb_fields(client: &JiraClient) -> Result<Vec<(String, String)>> {
    if let Some(cached) = cache::read_cmdb_fields_cache()? {
        return Ok(cached.fields);
    }

    let fields = client.find_cmdb_field_ids().await?;
    let _ = cache::write_cmdb_fields_cache(&fields);
    Ok(fields)
}

/// Convenience: extract just the field IDs from CMDB fields.
pub fn cmdb_field_ids(fields: &[(String, String)]) -> Vec<String> {
    fields.iter().map(|(id, _)| id.clone()).collect()
}
```

- [ ] **Step 2: Update callers in `list.rs` (show_assets block around line 261)**

In `src/cli/issue/list.rs`, update the `cmdb_field_ids` variable block:

```rust
    let cmdb_fields = if show_assets {
        let fields = get_or_fetch_cmdb_fields(client)
            .await
            .unwrap_or_default();
        if fields.is_empty() {
            eprintln!(
                "warning: --assets ignored. No Assets custom fields found on this Jira instance."
            );
        }
        fields
    } else {
        Vec::new()
    };
    let cmdb_field_ids = cmdb_field_ids(&cmdb_fields);
    for f in &cmdb_field_ids {
        extra.push(f.as_str());
    }
```

Also update the import at the top of `list.rs` — change `get_or_fetch_cmdb_field_ids` to `get_or_fetch_cmdb_fields` and add `cmdb_field_ids`:

```rust
use crate::api::assets::linked::{
    cmdb_field_ids, enrich_assets, extract_linked_assets, get_or_fetch_cmdb_fields,
};
```

- [ ] **Step 3: Update `handle_view` in `list.rs` (around line 507)**

Change the `cmdb_field_ids` call in `handle_view`:

```rust
    let cmdb_fields = get_or_fetch_cmdb_fields(client)
        .await
        .unwrap_or_default();
    let cmdb_field_id_list = cmdb_field_ids(&cmdb_fields);
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
    for f in &cmdb_field_id_list {
        extra.push(f.as_str());
    }
```

And update the later reference to `cmdb_field_ids` in `handle_view` (around line 675):

```rust
            if !cmdb_field_id_list.is_empty() {
                let mut linked = extract_linked_assets(&issue.fields.extra, &cmdb_field_id_list);
```

- [ ] **Step 4: Update `handle_issue_assets` in `assets.rs`**

In `src/cli/issue/assets.rs`, update the import and the call:

```rust
use crate::api::assets::linked::{
    cmdb_field_ids as get_cmdb_ids, enrich_assets, extract_linked_assets, get_or_fetch_cmdb_fields,
};
```

Then in the function body:

```rust
    let cmdb_fields = get_or_fetch_cmdb_fields(client).await?;
    let cmdb_field_ids = get_cmdb_ids(&cmdb_fields);

    if cmdb_field_ids.is_empty() {
```

And update the rest of the function to use `cmdb_field_ids` (the local variable, which is already `Vec<String>` of IDs).

- [ ] **Step 5: Verify the project compiles**

Run: `cargo build`
Expected: Compiles without errors.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/api/assets/linked.rs src/cli/issue/list.rs src/cli/issue/assets.rs
git commit -m "refactor: update CMDB field callers for (id, name) pairs (#88)"
```

---

### Task 4: Update integration tests for new CMDB field discovery

**Files:**
- Modify: `tests/cmdb_fields.rs:51-63` (discover_cmdb_field_ids test)

- [ ] **Step 1: Update integration test to expect (id, name) tuples**

In `tests/cmdb_fields.rs`, update `discover_cmdb_field_ids`:

```rust
#[tokio::test]
async fn discover_cmdb_field_ids() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_response_with_cmdb()))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let fields = client.find_cmdb_field_ids().await.unwrap();
    assert_eq!(
        fields,
        vec![("customfield_10191".to_string(), "Client".to_string())]
    );
}
```

And `discover_cmdb_field_ids_empty`:

```rust
#[tokio::test]
async fn discover_cmdb_field_ids_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fields_response_no_cmdb()))
        .mount(&server)
        .await;

    let client =
        jr::api::client::JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".into());
    let fields: Vec<(String, String)> = client.find_cmdb_field_ids().await.unwrap();
    assert!(fields.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --test cmdb_fields -- --nocapture`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tests/cmdb_fields.rs
git commit -m "test: update CMDB integration tests for (id, name) pairs (#88)"
```

---

### Task 5: Add asset key validation and `build_asset_clause`

**Files:**
- Modify: `src/jql.rs` (add `validate_asset_key` and `build_asset_clause`)

- [ ] **Step 1: Write failing tests for `validate_asset_key`**

Add to the `mod tests` block in `src/jql.rs`:

```rust
#[test]
fn validate_asset_key_valid_simple() {
    assert!(validate_asset_key("CUST-5").is_ok());
}

#[test]
fn validate_asset_key_valid_long() {
    assert!(validate_asset_key("SRV-42").is_ok());
}

#[test]
fn validate_asset_key_valid_itsm() {
    assert!(validate_asset_key("ITSM-123").is_ok());
}

#[test]
fn validate_asset_key_invalid_no_number() {
    assert!(validate_asset_key("CUST-").is_err());
}

#[test]
fn validate_asset_key_invalid_no_prefix() {
    assert!(validate_asset_key("-5").is_err());
}

#[test]
fn validate_asset_key_invalid_no_hyphen() {
    assert!(validate_asset_key("foo").is_err());
}

#[test]
fn validate_asset_key_invalid_empty() {
    assert!(validate_asset_key("").is_err());
}

#[test]
fn validate_asset_key_invalid_spaces() {
    assert!(validate_asset_key("CU ST-5").is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test validate_asset_key -- --nocapture`
Expected: FAIL — function not defined.

- [ ] **Step 3: Implement `validate_asset_key`**

Add to `src/jql.rs`:

```rust
/// Validate an asset object key matches the SCHEMA-NUMBER format.
///
/// Asset keys are always `<uppercase-alpha>-<digits>` (e.g., CUST-5, SRV-42, ITSM-123).
pub fn validate_asset_key(key: &str) -> Result<(), String> {
    let Some((prefix, number)) = key.split_once('-') else {
        return Err(format!(
            "Invalid asset key \"{key}\". Expected format: SCHEMA-NUMBER (e.g., CUST-5, SRV-42)."
        ));
    };
    if prefix.is_empty()
        || !prefix.chars().all(|c| c.is_ascii_alphanumeric())
        || number.is_empty()
        || !number.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "Invalid asset key \"{key}\". Expected format: SCHEMA-NUMBER (e.g., CUST-5, SRV-42)."
        ));
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test validate_asset_key -- --nocapture`
Expected: PASS

- [ ] **Step 5: Write failing tests for `build_asset_clause`**

Add to the `mod tests` block in `src/jql.rs`:

```rust
#[test]
fn build_asset_clause_single_field() {
    let fields = vec![("customfield_10191".to_string(), "Client".to_string())];
    let clause = build_asset_clause("CUST-5", &fields);
    assert_eq!(
        clause,
        r#""Client" IN aqlFunction("objectKey = \"CUST-5\"")"#
    );
}

#[test]
fn build_asset_clause_multiple_fields() {
    let fields = vec![
        ("customfield_10191".to_string(), "Client".to_string()),
        ("customfield_10245".to_string(), "Server".to_string()),
    ];
    let clause = build_asset_clause("SRV-42", &fields);
    assert_eq!(
        clause,
        r#"("Client" IN aqlFunction("objectKey = \"SRV-42\"") OR "Server" IN aqlFunction("objectKey = \"SRV-42\""))"#
    );
}

#[test]
fn build_asset_clause_field_name_with_quotes() {
    let fields = vec![("customfield_10191".to_string(), r#"My "Assets""#.to_string())];
    let clause = build_asset_clause("OBJ-1", &fields);
    assert_eq!(
        clause,
        r#""My \"Assets\"" IN aqlFunction("objectKey = \"OBJ-1\"")"#
    );
}
```

- [ ] **Step 6: Run tests to verify they fail**

Run: `cargo test build_asset_clause -- --nocapture`
Expected: FAIL — function not defined.

- [ ] **Step 7: Implement `build_asset_clause`**

Add to `src/jql.rs`:

```rust
/// Build a JQL clause that filters issues by a linked asset object key.
///
/// Uses `aqlFunction()` with the human-readable field name (required by Jira Cloud).
/// When multiple CMDB fields exist, OR them together and wrap in parentheses.
pub fn build_asset_clause(asset_key: &str, cmdb_fields: &[(String, String)]) -> String {
    let clauses: Vec<String> = cmdb_fields
        .iter()
        .map(|(_, name)| {
            format!(
                "\"{}\" IN aqlFunction(\"objectKey = \\\"{}\\\"\")",
                escape_value(name),
                escape_value(asset_key),
            )
        })
        .collect();

    if clauses.len() == 1 {
        clauses.into_iter().next().unwrap()
    } else {
        format!("({})", clauses.join(" OR "))
    }
}
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test build_asset_clause -- --nocapture`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/jql.rs
git commit -m "feat: add validate_asset_key and build_asset_clause (#88)"
```

---

### Task 6: Add `--asset` CLI flag and wire into `handle_list`

**Files:**
- Modify: `src/cli/mod.rs:153-187` (IssueCommand::List)
- Modify: `src/cli/issue/list.rs:56-86` (handle_list destructure)
- Modify: `src/cli/issue/list.rs:180-188` (build_filter_clauses call)
- Modify: `src/cli/issue/list.rs:244-276` (filter guard + cmdb block)

- [ ] **Step 1: Add `--asset` field to `IssueCommand::List`**

In `src/cli/mod.rs`, add after the `assets: bool` field in `IssueCommand::List`:

```rust
        /// Show linked assets column
        #[arg(long)]
        assets: bool,
        /// Filter by linked asset object key (e.g., CUST-5)
        #[arg(long)]
        asset: Option<String>,
```

- [ ] **Step 2: Update the destructure in `handle_list`**

In `src/cli/issue/list.rs`, update the destructure:

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
        open,
        points: show_points,
        assets: show_assets,
        asset: asset_key,
    } = command
    else {
        unreachable!()
    };
```

- [ ] **Step 3: Add asset key validation, clause building, and auto-enable assets column**

In `src/cli/issue/list.rs`, after the `--recent` validation block (around line 86) add:

```rust
    // Validate --asset key format early
    if let Some(ref key) = asset_key {
        crate::jql::validate_asset_key(key).map_err(JrError::UserError)?;
    }
```

Then, after the team clause resolution block (around line 113), add the CMDB field resolution for `--asset`:

```rust
    // Resolve CMDB fields for --asset filter (needs field names for aqlFunction)
    let asset_clause = if let Some(ref key) = asset_key {
        let cmdb_fields = get_or_fetch_cmdb_fields(client).await?;
        if cmdb_fields.is_empty() {
            return Err(JrError::UserError(
                "--asset requires Assets custom fields on this Jira instance. \
                 Assets requires Jira Service Management Premium or Enterprise."
                    .into(),
            )
            .into());
        }
        Some(crate::jql::build_asset_clause(key, &cmdb_fields))
    } else {
        None
    };
```

- [ ] **Step 4: Pass asset clause into filter building**

Update the `build_filter_clauses` call to include the asset clause:

```rust
    let filter_parts = build_filter_clauses(
        assignee_jql.as_deref(),
        reporter_jql.as_deref(),
        resolved_status.as_deref(),
        team_clause.as_deref(),
        recent.as_deref(),
        open,
        asset_clause.as_deref(),
    );
```

Update the `build_filter_clauses` function signature and body:

```rust
fn build_filter_clauses(
    assignee_jql: Option<&str>,
    reporter_jql: Option<&str>,
    status: Option<&str>,
    team_clause: Option<&str>,
    recent: Option<&str>,
    open: bool,
    asset_clause: Option<&str>,
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
    if open {
        parts.push("statusCategory != Done".to_string());
    }
    if let Some(t) = team_clause {
        parts.push(t.to_string());
    }
    if let Some(d) = recent {
        parts.push(format!("created >= -{d}"));
    }
    if let Some(a) = asset_clause {
        parts.push(a.to_string());
    }
    parts
}
```

- [ ] **Step 5: Auto-enable `--assets` display column when `--asset` is set**

In `handle_list`, after the destructure, add:

```rust
    // Auto-enable assets display column when filtering by asset
    let show_assets = show_assets || asset_key.is_some();
```

- [ ] **Step 6: Update the "no scope" guard error message**

Update the guard at line 251 to mention `--asset`:

```rust
        return Err(JrError::UserError(
            "No project or filters specified. Use --project, --assignee, --reporter, --status, --open, --team, --recent, --asset, or --jql. \
             You can also set a default project in .jr.toml or run \"jr init\"."
                .into(),
        )
```

- [ ] **Step 7: Verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

- [ ] **Step 8: Commit**

```bash
git add src/cli/mod.rs src/cli/issue/list.rs
git commit -m "feat: add --asset filter to issue list (#88)"
```

---

### Task 7: Update `build_filter_clauses` unit tests

**Files:**
- Modify: `src/cli/issue/list.rs` (existing tests in `mod tests`)

- [ ] **Step 1: Update all existing `build_filter_clauses` tests**

Every existing call to `build_filter_clauses` needs the new `asset_clause` parameter (pass `None`). Update each test:

```rust
#[test]
fn build_jql_parts_assignee_me() {
    let parts = build_filter_clauses(Some("currentUser()"), None, None, None, None, false, None);
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
        false,
        None,
    );
    assert_eq!(parts, vec!["reporter = 5b10ac8d82e05b22cc7d4ef5"]);
}

#[test]
fn build_jql_parts_recent() {
    let parts = build_filter_clauses(None, None, None, None, Some("7d"), false, None);
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
        false,
        None,
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
    let parts = build_filter_clauses(None, None, None, None, None, false, None);
    assert!(parts.is_empty());
}

#[test]
fn build_jql_parts_jql_plus_status_compose() {
    let filter = build_filter_clauses(None, None, Some("Done"), None, None, false, None);
    let mut all_parts = vec!["type = Bug".to_string()];
    all_parts.extend(filter);
    let jql = all_parts.join(" AND ");
    assert_eq!(jql, r#"type = Bug AND status = "Done""#);
}

#[test]
fn build_jql_parts_status_escaping() {
    let parts =
        build_filter_clauses(None, None, Some(r#"He said "hi" \o/"#), None, None, false, None);
    assert_eq!(parts, vec![r#"status = "He said \"hi\" \\o/""#.to_string()]);
}

#[test]
fn build_jql_parts_open() {
    let parts = build_filter_clauses(None, None, None, None, None, true, None);
    assert_eq!(parts, vec!["statusCategory != Done"]);
}

#[test]
fn build_jql_parts_open_with_assignee() {
    let parts = build_filter_clauses(Some("currentUser()"), None, None, None, None, true, None);
    assert_eq!(parts.len(), 2);
    assert!(parts.contains(&"assignee = currentUser()".to_string()));
    assert!(parts.contains(&"statusCategory != Done".to_string()));
}

#[test]
fn build_jql_parts_all_filters_with_open() {
    let parts = build_filter_clauses(
        Some("currentUser()"),
        Some("currentUser()"),
        None,
        Some(r#"customfield_10001 = "uuid-123""#),
        Some("30d"),
        true,
        None,
    );
    assert_eq!(parts.len(), 5);
    assert!(parts.contains(&"assignee = currentUser()".to_string()));
    assert!(parts.contains(&"reporter = currentUser()".to_string()));
    assert!(parts.contains(&"statusCategory != Done".to_string()));
    assert!(parts.contains(&r#"customfield_10001 = "uuid-123""#.to_string()));
    assert!(parts.contains(&"created >= -30d".to_string()));
}
```

- [ ] **Step 2: Add new test for asset clause**

```rust
#[test]
fn build_jql_parts_asset_clause() {
    let clause = r#""Client" IN aqlFunction("objectKey = \"CUST-5\"")"#;
    let parts = build_filter_clauses(None, None, None, None, None, false, Some(clause));
    assert_eq!(parts, vec![clause.to_string()]);
}

#[test]
fn build_jql_parts_asset_with_assignee() {
    let clause = r#""Client" IN aqlFunction("objectKey = \"CUST-5\"")"#;
    let parts = build_filter_clauses(
        Some("currentUser()"),
        None,
        None,
        None,
        None,
        false,
        Some(clause),
    );
    assert_eq!(parts.len(), 2);
    assert!(parts.contains(&"assignee = currentUser()".to_string()));
    assert!(parts.contains(&clause.to_string()));
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test build_jql_parts -- --nocapture`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "test: update build_filter_clauses tests for --asset parameter (#88)"
```

---

### Task 8: Run full test suite and lint

**Files:** None (verification only)

- [ ] **Step 1: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues.

- [ ] **Step 4: If any issues, fix and commit**

Fix any issues found in steps 1-3 and commit the fixes.
