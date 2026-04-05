# Resolve Asset-Typed Custom Fields Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enrich CMDB custom fields in JSON output, show per-field asset rows in `issue view`, and add CMDB fields to `project fields`.

**Architecture:** All three features build on the existing CMDB field discovery (`get_or_fetch_cmdb_fields` returning `(id, name)` pairs) and the `extract_linked_assets` + `enrich_assets` pipeline. JSON enrichment mutates the `fields.extra` HashMap entries in-place before serialization. Per-field rows iterate CMDB fields individually instead of lumping them into one "Assets" row. `project fields` appends a section using the cached field metadata.

**Tech Stack:** Rust, serde_json, wiremock (tests)

**Spec:** `docs/specs/resolve-asset-custom-fields.md`

**Dependency:** This plan assumes PR #88 (`feat/issue-list-asset-filter`) is merged to `develop` first — it provides `get_or_fetch_cmdb_fields` returning `Vec<(String, String)>` and the `cmdb_field_ids` helper.

---

### Task 1: Add `extract_linked_assets_per_field` to `linked.rs`

**Files:**
- Modify: `src/api/assets/linked.rs`

- [ ] **Step 1: Write failing tests**

Add to the `mod tests` block in `src/api/assets/linked.rs`:

```rust
#[test]
fn extract_per_field_single_field() {
    let mut extra = HashMap::new();
    extra.insert(
        "customfield_10191".into(),
        json!([{"label": "Acme Corp", "objectKey": "OBJ-1"}]),
    );
    let cmdb_fields = vec![
        ("customfield_10191".to_string(), "Client".to_string()),
    ];
    let result = extract_linked_assets_per_field(&extra, &cmdb_fields);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "Client");
    assert_eq!(result[0].1.len(), 1);
    assert_eq!(result[0].1[0].key.as_deref(), Some("OBJ-1"));
}

#[test]
fn extract_per_field_multiple_fields() {
    let mut extra = HashMap::new();
    extra.insert(
        "customfield_10191".into(),
        json!([{"label": "Acme Corp", "objectKey": "OBJ-1"}]),
    );
    extra.insert(
        "customfield_10245".into(),
        json!([{"label": "Email Server", "objectKey": "SRV-42"}]),
    );
    let cmdb_fields = vec![
        ("customfield_10191".to_string(), "Client".to_string()),
        ("customfield_10245".to_string(), "Affected Service".to_string()),
    ];
    let result = extract_linked_assets_per_field(&extra, &cmdb_fields);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, "Client");
    assert_eq!(result[1].0, "Affected Service");
}

#[test]
fn extract_per_field_skips_empty() {
    let mut extra = HashMap::new();
    extra.insert("customfield_10191".into(), json!(null));
    extra.insert(
        "customfield_10245".into(),
        json!([{"label": "Email Server", "objectKey": "SRV-42"}]),
    );
    let cmdb_fields = vec![
        ("customfield_10191".to_string(), "Client".to_string()),
        ("customfield_10245".to_string(), "Affected Service".to_string()),
    ];
    let result = extract_linked_assets_per_field(&extra, &cmdb_fields);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "Affected Service");
}

#[test]
fn extract_per_field_missing_field() {
    let extra = HashMap::new();
    let cmdb_fields = vec![
        ("customfield_10191".to_string(), "Client".to_string()),
    ];
    let result = extract_linked_assets_per_field(&extra, &cmdb_fields);
    assert!(result.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test extract_per_field -- --nocapture`
Expected: FAIL — function not defined.

- [ ] **Step 3: Implement `extract_linked_assets_per_field`**

Add to `src/api/assets/linked.rs`:

```rust
/// Extract linked assets grouped by CMDB field, returning (field_name, assets) pairs.
/// Skips fields that have no linked assets on the issue.
pub fn extract_linked_assets_per_field(
    extra: &HashMap<String, Value>,
    cmdb_fields: &[(String, String)],
) -> Vec<(String, Vec<LinkedAsset>)> {
    let mut result = Vec::new();
    for (field_id, field_name) in cmdb_fields {
        let assets = extract_linked_assets(extra, &[field_id.clone()]);
        if !assets.is_empty() {
            result.push((field_name.clone(), assets));
        }
    }
    result
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test extract_per_field -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/assets/linked.rs
git commit -m "feat: add extract_linked_assets_per_field for per-field asset display (#90)"
```

---

### Task 2: Add `enrich_json_assets` to `linked.rs`

**Files:**
- Modify: `src/api/assets/linked.rs`

- [ ] **Step 1: Write failing tests**

Add to the `mod tests` block in `src/api/assets/linked.rs`:

```rust
#[test]
fn enrich_json_injects_resolved_fields() {
    let mut extra = HashMap::new();
    extra.insert(
        "customfield_10191".to_string(),
        json!([{"objectId": "88", "workspaceId": "ws-1"}]),
    );

    let per_field = vec![(
        "customfield_10191".to_string(),
        vec![LinkedAsset {
            id: Some("88".into()),
            workspace_id: Some("ws-1".into()),
            key: Some("OBJ-88".into()),
            name: Some("Acme Corp".into()),
            asset_type: Some("Client".into()),
        }],
    )];

    enrich_json_assets(&mut extra, &per_field);

    let enriched = &extra["customfield_10191"];
    let arr = enriched.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    // Original fields preserved
    assert_eq!(arr[0]["objectId"], "88");
    assert_eq!(arr[0]["workspaceId"], "ws-1");
    // Enriched fields injected
    assert_eq!(arr[0]["objectKey"], "OBJ-88");
    assert_eq!(arr[0]["label"], "Acme Corp");
    assert_eq!(arr[0]["objectType"], "Client");
}

#[test]
fn enrich_json_preserves_already_enriched() {
    let mut extra = HashMap::new();
    extra.insert(
        "customfield_10191".to_string(),
        json!([{"objectKey": "OBJ-1", "label": "Already There"}]),
    );

    let per_field = vec![(
        "customfield_10191".to_string(),
        vec![LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Already There".into()),
            ..Default::default()
        }],
    )];

    enrich_json_assets(&mut extra, &per_field);

    let enriched = &extra["customfield_10191"];
    let arr = enriched.as_array().unwrap();
    assert_eq!(arr[0]["objectKey"], "OBJ-1");
    assert_eq!(arr[0]["label"], "Already There");
}

#[test]
fn enrich_json_partial_enrichment() {
    let mut extra = HashMap::new();
    extra.insert(
        "customfield_10191".to_string(),
        json!([
            {"objectId": "88", "workspaceId": "ws-1"},
            {"objectId": "99", "workspaceId": "ws-1"}
        ]),
    );

    // Only first asset was resolved
    let per_field = vec![(
        "customfield_10191".to_string(),
        vec![
            LinkedAsset {
                id: Some("88".into()),
                workspace_id: Some("ws-1".into()),
                key: Some("OBJ-88".into()),
                name: Some("Acme".into()),
                asset_type: Some("Client".into()),
            },
            LinkedAsset {
                id: Some("99".into()),
                workspace_id: Some("ws-1".into()),
                key: None,
                name: None,
                asset_type: None,
            },
        ],
    )];

    enrich_json_assets(&mut extra, &per_field);

    let arr = extra["customfield_10191"].as_array().unwrap();
    // First asset enriched
    assert_eq!(arr[0]["objectKey"], "OBJ-88");
    // Second asset: no enrichment injected (key/name were None)
    assert!(arr[1].get("objectKey").is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test enrich_json -- --nocapture`
Expected: FAIL — function not defined.

- [ ] **Step 3: Implement `enrich_json_assets`**

Add to `src/api/assets/linked.rs`:

```rust
/// Inject enriched asset data back into the issue's `fields.extra` HashMap.
///
/// For each CMDB field, matches enriched `LinkedAsset` entries by position to the
/// original JSON array elements and injects `objectKey`, `label`, and `objectType`
/// as additional fields (additive, does not remove existing fields).
pub fn enrich_json_assets(
    extra: &mut HashMap<String, Value>,
    per_field: &[(String, Vec<LinkedAsset>)],
) {
    for (field_id, assets) in per_field {
        let Some(value) = extra.get_mut(field_id) else {
            continue;
        };
        let Some(arr) = value.as_array_mut() else {
            continue;
        };

        for (i, asset) in assets.iter().enumerate() {
            if i >= arr.len() {
                break;
            }
            let Some(obj) = arr[i].as_object_mut() else {
                continue;
            };
            if let Some(ref key) = asset.key {
                obj.insert("objectKey".to_string(), Value::String(key.clone()));
            }
            if let Some(ref name) = asset.name {
                obj.insert("label".to_string(), Value::String(name.clone()));
            }
            if let Some(ref asset_type) = asset.asset_type {
                obj.insert("objectType".to_string(), Value::String(asset_type.clone()));
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test enrich_json -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/assets/linked.rs
git commit -m "feat: add enrich_json_assets for JSON output enrichment (#90)"
```

---

### Task 3: Update `handle_view` — per-field rows + JSON enrichment

**Files:**
- Modify: `src/cli/issue/list.rs` (`handle_view` function, around lines 496-702)

This task depends on PR #88 being merged. The code references `get_or_fetch_cmdb_fields` (returning `Vec<(String, String)>`), `cmdb_field_ids`, and other changes from that PR.

- [ ] **Step 1: Update imports**

In `src/cli/issue/list.rs`, update the `linked` import to include the new functions:

```rust
use crate::api::assets::linked::{
    cmdb_field_ids, enrich_assets, enrich_json_assets, extract_linked_assets_per_field,
    get_or_fetch_cmdb_fields,
};
```

Remove `extract_linked_assets` and `format_linked_assets` from the import if they are no longer used in this file (check `handle_list` — it still uses `extract_linked_assets` for the list table; keep it if so).

- [ ] **Step 2: Rewrite the `handle_view` function**

Replace the body of `handle_view` with:

```rust
pub(super) async fn handle_view(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::View { key } = command else {
        unreachable!()
    };

    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let cmdb_fields = get_or_fetch_cmdb_fields(client)
        .await
        .unwrap_or_default();
    let cmdb_field_id_list = cmdb_field_ids(&cmdb_fields);
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
    for f in &cmdb_field_id_list {
        extra.push(f.as_str());
    }
    let mut issue = client.get_issue(&key, &extra).await?;

    // Extract and enrich assets per-field (shared by both JSON and table paths)
    let mut per_field_assets = if !cmdb_fields.is_empty() {
        let per_field = extract_linked_assets_per_field(&issue.fields.extra, &cmdb_fields);
        // Collect all assets for batch enrichment
        let mut all_assets: Vec<LinkedAsset> = per_field
            .iter()
            .flat_map(|(_, assets)| assets.clone())
            .collect();
        enrich_assets(client, &mut all_assets).await;

        // Redistribute enriched assets back to per-field structure
        let mut enriched_per_field = Vec::new();
        let mut offset = 0;
        for (field_name, assets) in &per_field {
            let enriched = all_assets[offset..offset + assets.len()].to_vec();
            offset += assets.len();
            enriched_per_field.push((field_name.clone(), enriched));
        }
        enriched_per_field
    } else {
        Vec::new()
    };

    match output_format {
        OutputFormat::Json => {
            // Build (field_id, enriched_assets) for JSON injection
            if !per_field_assets.is_empty() {
                let per_field_by_id: Vec<(String, Vec<LinkedAsset>)> = cmdb_fields
                    .iter()
                    .filter_map(|(id, name)| {
                        per_field_assets
                            .iter()
                            .find(|(n, _)| n == name)
                            .map(|(_, assets)| (id.clone(), assets.clone()))
                    })
                    .collect();
                enrich_json_assets(&mut issue.fields.extra, &per_field_by_id);
            }
            println!("{}", output::render_json(&issue)?);
        }
        OutputFormat::Table => {
            let desc_text = issue
                .fields
                .description
                .as_ref()
                .map(adf::adf_to_text)
                .unwrap_or_else(|| "(no description)".into());

            let mut rows = vec![
                vec!["Key".into(), issue.key.clone()],
                vec!["Summary".into(), issue.fields.summary.clone()],
                vec![
                    "Type".into(),
                    issue
                        .fields
                        .issue_type
                        .as_ref()
                        .map(|t| t.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Status".into(),
                    issue
                        .fields
                        .status
                        .as_ref()
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Priority".into(),
                    issue
                        .fields
                        .priority
                        .as_ref()
                        .map(|p| p.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Assignee".into(),
                    issue
                        .fields
                        .assignee
                        .as_ref()
                        .map(|a| a.display_name.clone())
                        .unwrap_or_else(|| "Unassigned".into()),
                ],
                vec![
                    "Reporter".into(),
                    issue
                        .fields
                        .reporter
                        .as_ref()
                        .map(|r| r.display_name.clone())
                        .unwrap_or_else(|| "(none)".into()),
                ],
                vec![
                    "Created".into(),
                    issue
                        .fields
                        .created
                        .as_deref()
                        .map(format_comment_date)
                        .unwrap_or_else(|| "-".into()),
                ],
                vec![
                    "Updated".into(),
                    issue
                        .fields
                        .updated
                        .as_deref()
                        .map(format_comment_date)
                        .unwrap_or_else(|| "-".into()),
                ],
                vec![
                    "Project".into(),
                    issue
                        .fields
                        .project
                        .as_ref()
                        .map(|p| format!("{} ({})", p.name.as_deref().unwrap_or(""), p.key))
                        .unwrap_or_default(),
                ],
                vec![
                    "Labels".into(),
                    issue
                        .fields
                        .labels
                        .as_ref()
                        .filter(|l| !l.is_empty())
                        .map(|l| l.join(", "))
                        .unwrap_or_else(|| "(none)".into()),
                ],
            ];

            rows.push(vec![
                "Parent".into(),
                issue
                    .fields
                    .parent
                    .as_ref()
                    .map(|p| {
                        let summary = p
                            .fields
                            .as_ref()
                            .and_then(|f| f.summary.as_deref())
                            .unwrap_or("");
                        format!("{} ({})", p.key, summary)
                    })
                    .unwrap_or_else(|| "(none)".into()),
            ]);

            let links_display = issue
                .fields
                .issuelinks
                .as_ref()
                .filter(|links| !links.is_empty())
                .map(|links| {
                    links
                        .iter()
                        .map(|link| {
                            if let Some(ref outward) = link.outward_issue {
                                let desc = link
                                    .link_type
                                    .outward
                                    .as_deref()
                                    .unwrap_or(&link.link_type.name);
                                let summary = outward
                                    .fields
                                    .as_ref()
                                    .and_then(|f| f.summary.as_deref())
                                    .unwrap_or("");
                                format!("{} {} ({})", desc, outward.key, summary)
                            } else if let Some(ref inward) = link.inward_issue {
                                let desc = link
                                    .link_type
                                    .inward
                                    .as_deref()
                                    .unwrap_or(&link.link_type.name);
                                let summary = inward
                                    .fields
                                    .as_ref()
                                    .and_then(|f| f.summary.as_deref())
                                    .unwrap_or("");
                                format!("{} {} ({})", desc, inward.key, summary)
                            } else {
                                link.link_type.name.clone()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| "(none)".into());
            rows.push(vec!["Links".into(), links_display]);

            // Per-field asset rows (replaces the old single "Assets" row)
            for (field_name, assets) in &per_field_assets {
                let display = format_linked_assets(assets);
                rows.push(vec![field_name.clone(), display]);
            }

            if let Some(field_id) = sp_field_id {
                let points_display = issue
                    .fields
                    .story_points(field_id)
                    .map(format::format_points)
                    .unwrap_or_else(|| "(none)".into());
                rows.push(vec!["Points".into(), points_display]);
            }

            rows.push(vec!["Description".into(), desc_text]);

            println!("{}", output::render_table(&["Field", "Value"], &rows));
        }
    }

    Ok(())
}
```

Note: the `issue` binding must become `let mut issue = ...` because we mutate `issue.fields.extra` for JSON enrichment.

- [ ] **Step 3: Verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "feat: per-field asset rows and JSON enrichment in issue view (#90)"
```

---

### Task 4: Update `handle_list` — JSON enrichment when `--assets` active

**Files:**
- Modify: `src/cli/issue/list.rs` (`handle_list` function, around lines 280-366)

Currently the `issue list` enrichment block resolves assets for table mode but doesn't inject them into JSON. The change: after enrichment, if output is JSON and `--assets` is active, inject the enriched data into each issue's `fields.extra`.

The `issue_assets` vec is a flat list per issue, not grouped by field. Rather than complex offset tracking, use a simple approach: for each issue, build a per-field-ID mapping using the already-enriched `issue_assets` data.

- [ ] **Step 1: Make `issues` mutable**

Change `let issues = search_result.issues;` to `let mut issues = search_result.issues;`.

- [ ] **Step 2: Add JSON enrichment after the existing enrichment block**

In `handle_list`, after the closing `}` of the `if show_assets_col { ... }` block and before the `let rows: Vec<Vec<String>> = ...` line, add:

```rust
    // For JSON output with --assets, inject enriched data back into issue JSON
    if show_assets_col && matches!(output_format, OutputFormat::Json) {
        for (i, issue) in issues.iter_mut().enumerate() {
            // Re-extract per field to get field_id grouping, then match by position
            // to the enriched issue_assets[i] which has the same total ordering
            let mut per_field_by_id: Vec<(String, Vec<LinkedAsset>)> = Vec::new();
            let mut offset = 0;
            for field_id in &cmdb_field_id_list {
                let count = extract_linked_assets(
                    &issue.fields.extra,
                    &[field_id.clone()],
                )
                .len();
                if count > 0 && offset + count <= issue_assets[i].len() {
                    let enriched = issue_assets[i][offset..offset + count].to_vec();
                    per_field_by_id.push((field_id.clone(), enriched));
                }
                offset += count;
            }
            enrich_json_assets(&mut issue.fields.extra, &per_field_by_id);
        }
    }
```

Add `enrich_json_assets` to the imports at the top of the file.

- [ ] **Step 2: Verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/cli/issue/list.rs
git commit -m "feat: enrich CMDB fields in issue list JSON output (#90)"
```

---

### Task 5: Add CMDB fields to `project fields`

**Files:**
- Modify: `src/cli/project.rs` (`handle_fields` function)

- [ ] **Step 1: Add import**

Add to the top of `src/cli/project.rs`:

```rust
use crate::api::assets::linked::get_or_fetch_cmdb_fields;
```

- [ ] **Step 2: Fetch CMDB fields in `handle_fields`**

After the existing `let statuses = ...` line, add:

```rust
    let cmdb_fields = get_or_fetch_cmdb_fields(client)
        .await
        .unwrap_or_default();
```

- [ ] **Step 3: Add CMDB fields to JSON output**

In the `OutputFormat::Json` branch, add `asset_fields` to the JSON object:

```rust
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "project": project_key,
                    "issue_types": issue_types,
                    "priorities": priorities,
                    "statuses_by_issue_type": statuses,
                    "asset_fields": cmdb_fields.iter().map(|(id, name)| {
                        serde_json::json!({"id": id, "name": name})
                    }).collect::<Vec<_>>(),
                })
            );
        }
```

- [ ] **Step 4: Add CMDB fields to table output**

In the `OutputFormat::Table` branch, after the statuses block (after the closing `}` of `if has_statuses { ... }`), add:

```rust
            if !cmdb_fields.is_empty() {
                println!("\nCustom Fields (Assets) \u{2014} instance-wide:");
                for (id, name) in &cmdb_fields {
                    println!("  - {} ({})", name, id);
                }
            }
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

- [ ] **Step 6: Commit**

```bash
git add src/cli/project.rs
git commit -m "feat: show CMDB custom fields in project fields output (#90)"
```

---

### Task 6: Run full test suite and lint

**Files:** None (verification only)

- [ ] **Step 1: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues. If any, run `cargo fmt --all` to fix.

- [ ] **Step 4: If any issues, fix and commit**

Fix any issues found and commit the fixes.
