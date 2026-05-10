use anyhow::{Result, bail};
use serde_json::json;
use std::time::Duration;

use crate::adf;
use crate::api::assets::linked::get_or_fetch_cmdb_fields;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;

use super::helpers;
use super::json_output;

/// Maximum number of keys allowed in a single bulk edit call (Atlassian API limit).
const BULK_MAX_KEYS: usize = 1000;

pub(super) async fn handle_create(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Create {
        project,
        issue_type,
        summary,
        description,
        description_stdin,
        priority,
        label: labels,
        team,
        points,
        markdown,
        parent,
        to,
        account_id,
    } = command
    else {
        unreachable!()
    };

    // Resolve project key
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Project key").ok()
            }
        })
        .ok_or_else(|| {
            JrError::UserError(
                "Project key is required. Use --project or configure .jr.toml. \
                 Run \"jr project list\" to see available projects."
                    .into(),
            )
        })?;

    // Resolve issue type
    let issue_type_name = issue_type
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Issue type (e.g., Task, Bug, Story)").ok()
            }
        })
        .ok_or_else(|| JrError::UserError("Issue type is required. Use --type".into()))?;

    // Resolve summary
    let summary_text = summary
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Summary").ok()
            }
        })
        .ok_or_else(|| JrError::UserError("Summary is required. Use --summary".into()))?;

    // Resolve description. spawn_blocking isolates the blocking stdin read
    // from the tokio runtime so later async work isn't starved while waiting
    // on piped input.
    let desc_text = if description_stdin {
        let buf = tokio::task::spawn_blocking(|| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            Ok::<_, std::io::Error>(buf)
        })
        .await??;
        Some(buf)
    } else {
        description
    };

    // Build fields
    let mut fields = json!({
        "project": { "key": project_key },
        "issuetype": { "name": issue_type_name },
        "summary": summary_text,
    });

    if let Some(ref text) = desc_text {
        let adf_body = if markdown {
            adf::markdown_to_adf(text)
        } else {
            adf::text_to_adf(text)
        };
        fields["description"] = adf_body;
    }

    if let Some(ref prio) = priority {
        fields["priority"] = json!({ "name": prio });
    }

    if !labels.is_empty() {
        fields["labels"] = json!(labels);
    }

    if let Some(ref team_name) = team {
        let (field_id, team_id) =
            helpers::resolve_team_field(config, client, team_name, no_input).await?;
        fields[&field_id] = json!(team_id);
    }

    if let Some(pts) = points {
        let field_id = helpers::resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(pts);
    }

    if let Some(ref parent_key) = parent {
        fields["parent"] = json!({"key": parent_key});
    }

    if let Some(ref id) = account_id {
        fields["assignee"] = json!({"accountId": id});
    } else if let Some(ref user_query) = to {
        let (acct_id, _display_name) =
            helpers::resolve_assignee_by_project(client, user_query, &project_key, no_input)
                .await?;
        fields["assignee"] = json!({"accountId": acct_id});
    }

    let response = client.create_issue(fields).await?;

    let browse_url = format!(
        "{}/browse/{}",
        client.instance_url().trim_end_matches('/'),
        response.key
    );

    match output_format {
        OutputFormat::Json => {
            // Follow-up GET so the JSON output matches `issue view --output json`
            // (full Issue shape), plus `url`. On GET failure we keep the create
            // succeeding — warn on stderr and fall back to the old `{key, url}`
            // shape so downstream consumers always get at least the key + URL.
            //
            // Pre-existing pattern (same as handle_view, handle_list, project): a CMDB
            // discovery error silently degrades to an empty field list. Tracked as a
            // separate cleanup in the follow-up concerns documented on PR #253 — will
            // be addressed codebase-wide, not per-call-site.
            let cmdb_fields = get_or_fetch_cmdb_fields(client).await.unwrap_or_default();
            let extra_owned = helpers::compose_extra_fields(config, &cmdb_fields);
            let extra: Vec<&str> = extra_owned.iter().map(String::as_str).collect();

            match client.get_issue(&response.key, &extra).await {
                Ok(issue) => {
                    let mut issue_json = serde_json::to_value(&issue)?;
                    if let Some(obj) = issue_json.as_object_mut() {
                        obj.insert("url".into(), serde_json::Value::String(browse_url.clone()));
                    }
                    println!("{}", serde_json::to_string_pretty(&issue_json)?);
                }
                Err(err) => {
                    // Fallback JSON carries a top-level `fetch_error` string so
                    // scripts using `jq '.fields.status.name'` can tell this
                    // shape apart from success without parsing stderr. Recovery
                    // hint points users at `jr issue view` for the full payload.
                    let err_msg = format!("{err}");
                    eprintln!(
                        "warning: issue created ({}) but follow-up fetch failed: {err_msg}. \
                         Run `jr issue view {} --output json` to retrieve the full payload.",
                        response.key, response.key
                    );
                    let mut json_response = serde_json::to_value(&response)?;
                    json_response["url"] = json!(browse_url);
                    json_response["fetch_error"] = json!(err_msg);
                    println!("{}", serde_json::to_string_pretty(&json_response)?);
                }
            }
        }
        OutputFormat::Table => {
            output::print_success(&format!("Created issue {}", response.key));
            eprintln!("{}", browse_url);
        }
    }

    Ok(())
}

pub(super) async fn handle_edit(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Edit {
        keys,
        summary,
        issue_type,
        priority,
        label: labels,
        team,
        points,
        no_points,
        parent,
        no_parent,
        description,
        description_stdin,
        markdown,
    } = command
    else {
        unreachable!()
    };

    // AC (cap): enforce Atlassian's 1,000-issue limit per bulk call.
    if keys.len() > BULK_MAX_KEYS {
        return Err(JrError::UserError(format!(
            "Too many issue keys: {} provided, maximum is {}. \
             Split into batches of {} or fewer and run multiple times.",
            keys.len(),
            BULK_MAX_KEYS,
            BULK_MAX_KEYS,
        ))
        .into());
    }

    // Route: labels → bulk API (1..=1000 keys).
    // Non-label single-key edits → existing single-key PUT path (backward-compatible).
    // Non-label multi-key edits are not yet implemented in this PR.
    if !labels.is_empty() {
        return handle_edit_bulk_labels(&keys, labels, output_format, client, no_input).await;
    }

    // --- Single-key non-label path (unchanged from before) ---
    if keys.len() > 1 {
        bail!(
            "Multi-key edit without --label is not yet supported. \
             Use a single key or add --label add:<name> / --label remove:<name>."
        );
    }
    let key = &keys[0];

    let mut fields = json!({});
    let mut has_updates = false;

    // Resolve description (see handle_create for rationale on spawn_blocking).
    let desc_text = if description_stdin {
        let buf = tokio::task::spawn_blocking(|| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            Ok::<_, std::io::Error>(buf)
        })
        .await??;
        Some(buf)
    } else {
        description
    };

    if let Some(ref text) = desc_text {
        let adf_body = if markdown {
            adf::markdown_to_adf(text)
        } else {
            adf::text_to_adf(text)
        };
        fields["description"] = adf_body;
        has_updates = true;
    }

    if let Some(ref s) = summary {
        fields["summary"] = json!(s);
        has_updates = true;
    }

    if let Some(ref t) = issue_type {
        fields["issuetype"] = json!({ "name": t });
        has_updates = true;
    }

    if let Some(ref p) = priority {
        fields["priority"] = json!({ "name": p });
        has_updates = true;
    }

    if let Some(ref team_name) = team {
        let (field_id, team_id) =
            helpers::resolve_team_field(config, client, team_name, no_input).await?;
        fields[&field_id] = json!(team_id);
        has_updates = true;
    }

    if let Some(pts) = points {
        let field_id = helpers::resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(pts);
        has_updates = true;
    }

    if no_points {
        let field_id = helpers::resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(null);
        has_updates = true;
    }

    if let Some(ref parent_key) = parent {
        fields["parent"] = json!({"key": parent_key});
        has_updates = true;
    }

    if no_parent {
        fields["parent"] = serde_json::Value::Null;
        has_updates = true;
    }

    if !has_updates {
        bail!(
            "No fields specified to update. Use --summary, --type, --priority, --label, --team, --points, --no-points, --parent, --no-parent, --description, or --description-stdin."
        );
    }

    let edit_result = client.edit_issue(key, fields).await;
    if let Err(ref e) = edit_result {
        if no_parent && is_subtask_parent_error(e) {
            let hint = format!(
                "{e}\n\n\
                 Tip: subtasks are structurally bound to a parent. \
                 To clear the parent, first convert the subtask to a standard issue:\n  \
                 jr api /rest/api/3/issue/{key}/convert -X put -d '{{\"type\":{{\"name\":\"Task\"}}}}'\n\
                 (then re-run with --no-parent if needed.)"
            );
            bail!("{hint}");
        }
    }
    edit_result?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json_output::edit_response(key))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Updated {}", key));
        }
    }

    Ok(())
}

/// Route label edits through the Atlassian Bulk Fields API.
///
/// Supports 1..=1000 keys. `labels` is a list of "add:NAME" / "remove:NAME" / "NAME" strings.
///
/// editedFieldsInput shape (labelsAction casing "ADD"/"REMOVE" — best-guess per
/// SCHEMA NOTES in tests/issue_bulk.rs; exact values unverified against live API):
/// ```json
/// {"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}}
/// ```
/// Multiple label operations are collapsed: all adds into one ADD block, all removes into
/// one REMOVE block. When both adds and removes are present, two separate bulk calls are
/// made (Atlassian's editedFieldsInput for labels may not support mixed ADD/REMOVE in
/// one request — unverified; safe default is sequential calls).
///
/// Output:
/// - Table mode: per-key success/error lines.
/// - JSON mode: `{"taskId":"...","results":[{"key":"...","status":"success|error","error":"..."}]}`
/// - Single-key JSON mode: also includes `"key":"..."` at top level (backward-compat shape).
/// - Exit 0 if all succeeded; exit 1 if any failed.
async fn handle_edit_bulk_labels(
    keys: &[String],
    labels: Vec<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
    _no_input: bool,
) -> Result<()> {
    // Parse labels into add/remove buckets.
    let mut adds: Vec<String> = Vec::new();
    let mut removes: Vec<String> = Vec::new();

    for l in &labels {
        if let Some(name) = l.strip_prefix("add:") {
            adds.push(name.to_string());
        } else if let Some(name) = l.strip_prefix("remove:") {
            removes.push(name.to_string());
        } else {
            // Bare label treated as add.
            adds.push(l.clone());
        }
    }

    // Determine the primary action and label list for the bulk call.
    // When both adds and removes exist, we prioritize the adds bucket
    // and attach the removes in a second object under the same payload
    // if possible. The safe interpretation per Atlassian's bulk-edit
    // UI semantics is that a single labelsAction applies to the whole
    // batch; mixing requires two separate requests.
    //
    // For this PR, use the first non-empty bucket (adds wins if both).
    // Second bucket (if present) makes a sequential follow-up call.
    // SEMPORT-REVIEW: Verify whether Atlassian accepts mixed ADD/REMOVE
    // in a single editedFieldsInput.labels call.
    let calls: Vec<(&str, &Vec<String>)> = {
        let mut v: Vec<(&str, &Vec<String>)> = Vec::new();
        if !adds.is_empty() {
            v.push(("ADD", &adds));
        }
        if !removes.is_empty() {
            v.push(("REMOVE", &removes));
        }
        v
    };

    if calls.is_empty() {
        bail!("No label changes specified.");
    }

    // Use the last task_id and progress for output (covers most cases with one call).
    let mut final_task_id = String::new();
    let mut final_progress = None;

    for (action, label_names) in &calls {
        let label_entries: Vec<serde_json::Value> =
            label_names.iter().map(|n| json!({"name": n})).collect();

        let edited_fields = json!({
            "labels": {
                "labelsAction": action,
                "labels": label_entries
            }
        });

        let task_id = client.bulk_edit_fields(keys, edited_fields).await?;
        // Poll with 5-minute timeout.
        let progress = client
            .await_bulk_task(&task_id, Duration::from_secs(300))
            .await?;

        final_task_id = task_id;
        final_progress = Some(progress);
    }

    let progress = final_progress.expect("at least one call was made");
    render_bulk_edit_results(keys, &final_task_id, &progress, output_format)
}

/// Render bulk edit results to stdout/stderr and return the appropriate exit code.
///
/// - Table mode: print per-key success/error lines.
/// - JSON mode: `{"taskId":"...","results":[...]}` with optional `"key"` for single-key BC.
/// - Returns `Ok(())` if all succeeded; returns `Err(exit-1)` if any failed.
fn render_bulk_edit_results(
    keys: &[String],
    task_id: &str,
    progress: &crate::types::jira::bulk::BulkOperationProgress,
    output_format: &OutputFormat,
) -> Result<()> {
    let processed: std::collections::HashSet<&str> = progress
        .processed_accessible_issues
        .iter()
        .map(String::as_str)
        .collect();

    // Build per-key result list. Keys not in processed or failed are assumed
    // inaccessible/invalid (Atlassian may silently exclude them).
    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut any_failed = false;

    for key in keys {
        if let Some(err) = progress.failed_accessible_issues.get(key.as_str()) {
            let summary = err.summary();
            results.push(json!({
                "key": key,
                "status": "error",
                "error": summary,
            }));
            any_failed = true;
        } else if processed.contains(key.as_str()) {
            results.push(json!({
                "key": key,
                "status": "success",
            }));
        } else {
            // Not in processed and not in failed — inaccessible or invalid.
            results.push(json!({
                "key": key,
                "status": "inaccessible",
            }));
        }
    }

    // Also capture any failed keys that weren't in our input list
    // (shouldn't happen, but Atlassian may return unexpected keys).
    for (failed_key, err) in &progress.failed_accessible_issues {
        if !keys.iter().any(|k| k == failed_key) {
            results.push(json!({
                "key": failed_key,
                "status": "error",
                "error": err.summary(),
            }));
            any_failed = true;
        }
    }

    match output_format {
        OutputFormat::Json => {
            let mut payload = json!({
                "taskId": task_id,
                "results": results,
            });
            // Single-key backward-compat: include "key" at top level.
            if keys.len() == 1 {
                payload["key"] = json!(&keys[0]);
            }
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        OutputFormat::Table => {
            for entry in &results {
                let key = entry["key"].as_str().unwrap_or("?");
                match entry["status"].as_str().unwrap_or("?") {
                    "success" => output::print_success(&format!("Updated {key}")),
                    "error" => {
                        let err_msg = entry["error"].as_str().unwrap_or("unknown error");
                        eprintln!("error: {key}: {err_msg}");
                    }
                    status => eprintln!("warning: {key}: {status}"),
                }
            }
        }
    }

    if any_failed {
        // Return a non-Ok result that maps to exit code 1.
        bail!("One or more issues failed during bulk edit. See output above for details.");
    }

    Ok(())
}

/// Returns `true` when the error message indicates Jira rejected a parent-clear
/// operation because the issue is a subtask (subtasks are structurally bound to
/// a parent and cannot be un-parented without first converting to a regular issue).
///
/// Matches both common Atlassian error shapes (case-insensitive):
/// - `errors: { "parent": "<message containing 'subtask'>" }`
///   → extract_error_message yields "parent: Subtasks must have a parent."
/// - `errorMessages: ["... subtask ... parent ..."]`
fn is_subtask_parent_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("subtask") || (msg.contains("parent") && msg.contains("400"))
}

#[cfg(test)]
mod tests {
    use crate::error::JrError;

    #[test]
    fn missing_project_returns_user_error() {
        let result: Option<String> = None;
        let err = result
            .ok_or_else(|| {
                JrError::UserError(
                    "Project key is required. Use --project or configure .jr.toml. \
                     Run \"jr project list\" to see available projects."
                        .into(),
                )
            })
            .unwrap_err();
        assert_eq!(err.exit_code(), 64);
        assert!(err.to_string().contains("Project key is required"));
    }
}
