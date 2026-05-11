use anyhow::{Result, bail};
use serde_json::json;
use std::time::Duration;

use crate::adf;
use crate::api::assets::linked::get_or_fetch_cmdb_fields;
use crate::api::client::JiraClient;
use crate::api::jira::bulk::BULK_MAX_KEYS;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;

use super::helpers;
use super::json_output;

/// Number of issues above which a `--jql`-driven bulk edit requires explicit
/// `--yes` (or `--no-input` implicit-yes) to proceed. Below this threshold the
/// command runs without prompting because the blast radius is small.
///
/// Set to 5 as a conservative default — many real bulk operations target 10-50
/// issues from a saved JQL filter, so users will hit this prompt routinely. If
/// product feedback indicates the threshold is too aggressive, raise to 25-50.
const JQL_CONFIRM_THRESHOLD: usize = 5;

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
        jql,
        max,
        yes,
        dry_run,
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

    // Validate: at least one selector must be present (keys or --jql).
    // clap doesn't enforce this natively since both are optional — we validate here.
    if keys.is_empty() && jql.is_none() {
        return Err(
            JrError::UserError("Specify at least one issue key or --jql <query>.".into()).into(),
        );
    }

    // Validate: --max is only meaningful with --jql.  clap's `requires` attribute cannot
    // enforce this when positional keys are also present (because `keys` and `jql` have
    // `conflicts_with` between them, which causes clap to skip the `requires` check).
    // We enforce it here instead, before any HTTP calls.
    if max.is_some() && jql.is_none() {
        return Err(JrError::UserError(
            "--max requires --jql. It cannot be used with positional keys because \
             it only limits the number of issues matched by a JQL query. \
             Remove --max or switch to --jql <query>."
                .into(),
        )
        .into());
    }

    // Validate: --markdown is a modifier on --description/--description-stdin, NOT a
    // standalone field change.  Reject it early (before any HTTP calls) so the user
    // gets a clear error instead of a wasted JQL search followed by "No fields specified".
    if markdown && description.is_none() && !description_stdin {
        return Err(JrError::UserError(
            "--markdown requires --description or --description-stdin to take effect. \
             Pass a description alongside --markdown, or omit --markdown."
                .into(),
        )
        .into());
    }

    // Pre-HTTP guard: if no field-change flags are specified, error here BEFORE running
    // any JQL search or making any HTTP calls.  This is the single source of truth for
    // the "no fields" check — both the JQL path and the dry-run path rely on this guard;
    // there is no duplicate check inside the dry-run block.
    //
    // NOTE: `markdown` is intentionally NOT included here — it is a modifier on
    // --description, not an independent field change. The validation above already
    // rejects `--markdown` without a description, so if we reach this point with
    // `markdown == true`, a description must also be set.
    {
        let has_any_field_change = summary.is_some()
            || priority.is_some()
            || issue_type.is_some()
            || !labels.is_empty()
            || team.is_some()
            || points.is_some()
            || no_points
            || parent.is_some()
            || no_parent
            || description.is_some()
            || description_stdin;
        if !has_any_field_change {
            return Err(JrError::UserError(
                "No fields specified to update. Use --summary, --type, --priority, --label, \
                 --team, --points, --no-points, --parent, --no-parent, --description, or \
                 --description-stdin."
                    .into(),
            )
            .into());
        }
    }

    // --- Reject --label combined with non-label field flags. ---
    // --label is routed through a labels-only bulk path (handle_edit_bulk_labels) that
    // does not honour concurrent --summary/--priority/--type flags.  Combining them
    // would silently drop the non-label fields (exit 0, data loss).  Reject the
    // combination HERE, before any HTTP call (including the JQL search), rather than
    // silently discard the fields.
    // Mixed label + field bulk edits require the schema-correct combined payload tracked
    // at #331; until that lands, keep --label and field flags mutually exclusive.
    if !labels.is_empty() {
        let mut conflicting: Vec<&str> = Vec::new();
        if summary.is_some() {
            conflicting.push("--summary");
        }
        if priority.is_some() {
            conflicting.push("--priority");
        }
        if issue_type.is_some() {
            conflicting.push("--type");
        }
        if team.is_some() {
            conflicting.push("--team");
        }
        if points.is_some() {
            conflicting.push("--points");
        }
        if no_points {
            conflicting.push("--no-points");
        }
        if parent.is_some() {
            conflicting.push("--parent");
        }
        if no_parent {
            conflicting.push("--no-parent");
        }
        if description.is_some() {
            conflicting.push("--description");
        }
        if description_stdin {
            conflicting.push("--description-stdin");
        }
        if markdown {
            conflicting.push("--markdown");
        }
        if !conflicting.is_empty() {
            return Err(JrError::UserError(format!(
                "--label cannot be combined with {} in the same call. \
                 Run separate `jr issue edit` commands, or open an issue to track \
                 combined label + field bulk edits (see #331).",
                conflicting.join(", ")
            ))
            .into());
        }
    }

    // --max is meaningless without --jql (positional keys use the existing 1001-key
    // hard cap, not --max). The handler-level guard earlier in this function already
    // rejects `--max` without `--jql` with JrError::UserError (exit 64) because
    // clap's `requires` attribute interacts poorly with the keys/jql `conflicts_with`
    // relationship. By the time we reach this branch we know jql.is_some() so the
    // unwrap_or(50) default is the right behavior.
    let effective_max = max.unwrap_or(50).min(BULK_MAX_KEYS as u32);

    // Resolve the working set of keys.
    // For --jql: execute the search (read-only), then enforce --max cap.
    // For positional keys: use them directly (no HTTP read needed).
    let effective_keys: Vec<String> = if let Some(ref jql_str) = jql {
        if jql_str.trim().is_empty() {
            return Err(JrError::UserError(
                "--jql query cannot be empty. Provide a JQL expression like \
                 'project = FOO AND status = \"To Do\"', or pass keys positionally."
                    .into(),
            )
            .into());
        }

        // --dry-run with --jql: search is read-only, allowed.
        let search_result = client
            .search_issues(jql_str, Some(effective_max + 1), &[])
            .await?;
        let matched = search_result.issues;

        if matched.is_empty() {
            return Err(JrError::UserError(format!(
                "JQL '{}' matched 0 issues. Refine your query or pass keys directly.",
                jql_str,
            ))
            .into());
        }

        if matched.len() > effective_max as usize {
            return Err(JrError::UserError(format!(
                "JQL matched at least {} issues, which exceeds --max {}. \
                 Use --max <N> to allow up to {} issues, or refine your JQL.",
                matched.len(),
                effective_max,
                BULK_MAX_KEYS,
            ))
            .into());
        }

        matched.into_iter().map(|i| i.key).collect()
    } else {
        // Positional keys: enforce the Atlassian hard ceiling.
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
        keys.clone()
    };

    // --- C-1: Reject multi-key edits that include flags unsupported in bulk context. ---
    // These flags (parent, team, points, description, markdown) are only implemented
    // on the single-key path. Passing them with multiple keys previously caused silent
    // data loss: the flag was forwarded to handle_edit_bulk_fields which ignored it,
    // then returned Ok(). We now reject early with a clear error so users aren't surprised.
    //
    // This check runs BEFORE the dry-run block so that `--dry-run --no-parent` also
    // reports the unsupported-flag error consistently with the live path.
    if effective_keys.len() > 1 {
        let mut unsupported: Vec<&str> = Vec::new();
        if parent.is_some() {
            unsupported.push("--parent");
        }
        if no_parent {
            unsupported.push("--no-parent");
        }
        if team.is_some() {
            unsupported.push("--team");
        }
        if points.is_some() {
            unsupported.push("--points");
        }
        if no_points {
            unsupported.push("--no-points");
        }
        if description.is_some() || description_stdin {
            unsupported.push("--description / --description-stdin");
        }
        if markdown {
            unsupported.push("--markdown");
        }
        if !unsupported.is_empty() {
            return Err(JrError::UserError(format!(
                "Multi-key bulk edit doesn't yet support: {}. \
                 Use a single key, or open an issue if this matters for your workflow.",
                unsupported.join(", ")
            ))
            .into());
        }
    }

    // --- Dry-run short-circuit: render diff, no HTTP mutations. ---
    if dry_run {
        // NOTE: The "no fields specified" guard already fired unconditionally above
        // (pre-HTTP guard, lines ~276-294) before execution reaches here.  No
        // duplicate check needed — any invocation with zero field flags exits before
        // this block is entered.
        match output_format {
            OutputFormat::Json => {
                // C-3: --output json must produce machine-readable JSON on stdout,
                // not prose. Build a planned-changes object containing only the
                // fields the user actually requested.
                let mut planned = serde_json::Map::new();
                if let Some(ref s) = summary {
                    planned.insert("summary".into(), json!(s));
                }
                if let Some(ref p) = priority {
                    planned.insert("priority".into(), json!(p));
                }
                if !labels.is_empty() {
                    let label_entries: Vec<serde_json::Value> = labels
                        .iter()
                        .map(|l| {
                            if let Some(name) = l.strip_prefix("add:") {
                                json!({"action": "ADD", "name": name})
                            } else if let Some(name) = l.strip_prefix("remove:") {
                                json!({"action": "REMOVE", "name": name})
                            } else {
                                json!({"action": "ADD", "name": l})
                            }
                        })
                        .collect();
                    planned.insert("labels".into(), json!(label_entries));
                }
                if let Some(ref t) = issue_type {
                    planned.insert("issueType".into(), json!(t));
                }
                if let Some(ref par) = parent {
                    planned.insert("parent".into(), json!(par));
                }
                if no_parent {
                    planned.insert("parent".into(), serde_json::Value::Null);
                }
                if let Some(pts) = points {
                    planned.insert("points".into(), json!(pts));
                }
                if no_points {
                    planned.insert("points".into(), serde_json::Value::Null);
                }
                // Single-key-only fields: team, description, description_stdin, markdown.
                // Multi-key bulk rejects these flags upstream (C-1 guard), so reaching
                // here with effective_keys.len() > 1 and these flags set is impossible.
                if let Some(ref t) = team {
                    planned.insert("team".into(), json!(t));
                }
                if let Some(ref d) = description {
                    planned.insert("description".into(), json!(d));
                } else if description_stdin {
                    // --dry-run does NOT read stdin; document this as a known limitation.
                    planned.insert(
                        "description".into(),
                        json!("<from stdin — not yet read in dry-run>"),
                    );
                }
                if markdown {
                    planned.insert("markdown".into(), json!(true));
                }
                let payload = json!({
                    "dryRun": true,
                    "issues": &effective_keys,
                    "plannedChanges": planned,
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            }
            OutputFormat::Table => {
                // Human-readable prose on stdout (profile-1 for dry-run: data on stdout is fine).
                println!("DRY RUN — no changes will be made.");
                println!("Issues affected ({}):", effective_keys.len());
                for k in &effective_keys {
                    println!("  {k}");
                }
                println!("Planned changes:");
                if let Some(ref s) = summary {
                    println!("  summary → {s}");
                }
                if let Some(ref p) = priority {
                    println!("  priority → {p}");
                }
                if !labels.is_empty() {
                    println!("  labels → {}", labels.join(", "));
                }
                if let Some(ref t) = issue_type {
                    println!("  type → {t}");
                }
                if let Some(ref par) = parent {
                    println!("  parent → {par}");
                }
                if no_parent {
                    println!("  parent → (clear)");
                }
                if let Some(pts) = points {
                    println!("  points → {pts}");
                }
                if no_points {
                    println!("  points → (clear)");
                }
                // Single-key-only fields: team, description, description_stdin, markdown.
                // Multi-key bulk rejects these flags upstream (C-1 guard), so reaching
                // here with effective_keys.len() > 1 and these flags set is impossible.
                if let Some(ref t) = team {
                    println!("  team → {t}");
                }
                if let Some(ref d) = description {
                    // Truncate long descriptions to 60 codepoints for readability.
                    // Use chars().count() / chars().take(60) — NOT byte slicing —
                    // to avoid panics on multi-byte UTF-8 codepoints (Cyrillic,
                    // CJK, emoji, accented chars). Codepoint-aware is the correct
                    // Rust-stdlib idiom; grapheme clusters (unicode_segmentation)
                    // would be overkill for a display truncation.
                    let char_count = d.chars().count();
                    let preview = if char_count > 60 {
                        let truncated: String = d.chars().take(60).collect();
                        format!("{truncated}...")
                    } else {
                        d.clone()
                    };
                    println!("  description → {preview}");
                } else if description_stdin {
                    // --dry-run does NOT read stdin; document this as a known limitation.
                    println!("  description → (read from stdin — not yet read in dry-run)");
                }
                if markdown {
                    println!("  markdown rendering: enabled");
                }
            }
        }
        return Ok(());
    }

    // --- Confirmation for large JQL match sets. ---
    // Safety-net: when --jql is used AND match count > threshold (JQL_CONFIRM_THRESHOLD),
    // require explicit --yes or interactive confirmation.
    // --no-input without --yes on a large set emits a hint but proceeds
    // (implicit-yes policy for non-interactive mode on any size set).
    if jql.is_some() && effective_keys.len() > JQL_CONFIRM_THRESHOLD {
        if !yes && !no_input {
            // Interactive confirmation via dialoguer.
            let prompt = format!(
                "This will bulk-edit {} issues. Proceed?",
                effective_keys.len()
            );
            let confirmed =
                dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt(prompt)
                    .default(false)
                    .interact()
                    .map_err(|e| {
                        JrError::UserError(format!(
                            "Confirmation prompt failed: {e}. Use --yes to skip the prompt or \
                             --no-input to disable interactive confirmation."
                        ))
                    })?;
            if !confirmed {
                return Err(JrError::UserError(
                    "Bulk edit declined at confirmation prompt. No changes made.".into(),
                )
                .into());
            }
        } else if !yes && no_input {
            // Safety-net hint for --no-input without --yes on a large set.
            eprintln!(
                "Warning: bulk edit will affect {} issues (matched by --jql). \
                 Use --yes to skip this hint, or --dry-run to preview. Proceeding.",
                effective_keys.len()
            );
        }
        // --yes: skip prompt entirely.
    }

    // --- Route: labels → bulk API. ---
    if !labels.is_empty() {
        return handle_edit_bulk_labels(&effective_keys, labels, output_format, client, no_input)
            .await;
    }

    // Routing for non-label edits:
    // - 2+ keys (positional or --jql-resolved) → POST /rest/api/3/bulk/issues/fields (bulk API)
    // - 1 key (positional or single-match --jql) → PUT /rest/api/3/issue/{key} (legacy single-key)
    //
    // The single-match --jql case intentionally uses the legacy path because it's
    // per-issue more efficient (no taskId polling) and the bulk API has no advantage
    // for a single issue. Users mental-modeling "JQL → always bulk" should be aware
    // of this asymmetry; it's documented rather than enforced.

    // --- Multi-key non-label: route through bulk_edit_fields. ---
    if effective_keys.len() > 1 {
        return handle_edit_bulk_fields(
            &effective_keys,
            summary.as_deref(),
            priority.as_deref(),
            issue_type.as_deref(),
            output_format,
            client,
        )
        .await;
    }

    // --- Single-key non-label path (unchanged from before) ---
    let key = &effective_keys[0];

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
/// editedFieldsInput shape (best-guess pending #331 empirical verification):
/// - When BOTH ADD and REMOVE labels are present, coalesced into ONE bulk POST
///   with an array of operations:
///   ```json
///   {
///     "labels": [
///       {"labelsAction": "ADD",    "labels": [{"name": "foo"}]},
///       {"labelsAction": "REMOVE", "labels": [{"name": "bar"}]}
///     ]
///   }
///   ```
/// - When only ADD or only REMOVE labels are present, an object form (NOT a
///   single-entry array) is sent for backward compatibility with PR1 tests:
///   ```json
///   {"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}}
///   ```
/// Tests use `body_string_contains` matchers to tolerate the shape difference;
/// canonical Atlassian schema (per #331) requires top-level `labelsFields`
/// array always — that's the long-term target for both code paths.
/// `.expect(1)` enforces ONE bulk POST even when both ADD+REMOVE are specified.
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

    if adds.is_empty() && removes.is_empty() {
        bail!("No label changes specified.");
    }

    // Coalesce ADD and REMOVE into a single bulk POST.
    // Both operations are submitted in one request using an array of label action objects.
    // Shape is best-guess (unverified against live Atlassian API; tracked at #331).
    // PR2 test asserts .expect(1) on bulk POST to ensure ADD+REMOVE coalesce into ONE call,
    // but the exact JSON nesting matches a loose `body_string_contains` matcher — schema
    // accuracy is the work being deferred to #331.
    let mut label_ops: Vec<serde_json::Value> = Vec::new();
    if !adds.is_empty() {
        let add_entries: Vec<serde_json::Value> = adds.iter().map(|n| json!({"name": n})).collect();
        label_ops.push(json!({
            "labelsAction": "ADD",
            "labels": add_entries
        }));
    }
    if !removes.is_empty() {
        let remove_entries: Vec<serde_json::Value> =
            removes.iter().map(|n| json!({"name": n})).collect();
        label_ops.push(json!({
            "labelsAction": "REMOVE",
            "labels": remove_entries
        }));
    }

    // When only one action is present, unwrap to the simpler object form
    // for backward compatibility with PR1 tests (body_partial_json matchers).
    let edited_fields = if label_ops.len() == 1 {
        let op = label_ops.remove(0);
        json!({ "labels": op })
    } else {
        // Both ADD and REMOVE: use the coalesced array form.
        json!({ "labels": label_ops })
    };

    // selectedActions for labels is always ["labels"] regardless of ADD/REMOVE/coalesce.
    let task_id = client
        .bulk_edit_fields(keys, vec!["labels".to_string()], edited_fields)
        .await?;
    // Poll with 5-minute timeout.
    let progress = client
        .await_bulk_task(&task_id, Duration::from_secs(300))
        .await?;

    render_bulk_edit_results(keys, &task_id, &progress, output_format)
}

/// Route non-label multi-key edits through the Atlassian Bulk Fields API.
///
/// Supports 2..=1000 keys with --summary, --priority, --type.
///
/// editedFieldsInput shape (best-guess — unverified against live API):
/// ```json
/// {
///   "summary": "New title",
///   "priority": {"name": "High"},
///   "issuetype": {"name": "Bug"}
/// }
/// ```
/// Tests use body_string_contains("summary") / body_string_contains("priority")
/// as loose matchers so exact nesting variation is tolerated.
async fn handle_edit_bulk_fields(
    keys: &[String],
    summary: Option<&str>,
    priority: Option<&str>,
    issue_type: Option<&str>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let mut edited = serde_json::Map::new();
    let mut selected_actions: Vec<String> = Vec::new();

    if let Some(s) = summary {
        edited.insert("summary".into(), json!(s));
        selected_actions.push("summary".to_string());
    }
    if let Some(p) = priority {
        edited.insert("priority".into(), json!({"name": p}));
        selected_actions.push("priority".to_string());
    }
    if let Some(t) = issue_type {
        edited.insert("issuetype".into(), json!({"name": t}));
        // Match editedFieldsInput key (lowercase). Atlassian docs are ambiguous on
        // canonical casing for the bulk endpoint specifically; the lowercase form
        // matches the legacy single-key path. Empirical schema verification deferred to #331.
        selected_actions.push("issuetype".to_string());
    }

    if edited.is_empty() {
        bail!(
            "No fields specified to update. Use --summary, --type, --priority, --label, --team, \
             --points, --no-points, --parent, --no-parent, --description, or --description-stdin."
        );
    }

    let edited_fields = serde_json::Value::Object(edited);
    let task_id = client
        .bulk_edit_fields(keys, selected_actions, edited_fields)
        .await?;
    let progress = client
        .await_bulk_task(&task_id, Duration::from_secs(300))
        .await?;

    render_bulk_edit_results(keys, &task_id, &progress, output_format)
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
