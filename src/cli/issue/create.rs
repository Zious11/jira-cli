use std::collections::HashMap;

use anyhow::{Result, bail};
use serde_json::json;

use crate::adf;
use crate::api::assets::linked::get_or_fetch_cmdb_fields;
use crate::api::client::JiraClient;
use crate::api::jira::bulk::{BULK_MAX_KEYS, resolve_bulk_await_timeout};
use crate::api::jsm::requests::JsmRequestBuilder;
use crate::api::jsm::servicedesks;
use crate::cache;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};

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
        request_type,
        field: field_pairs,
        on_behalf_of,
    } = command
    else {
        unreachable!()
    };

    // Dispatch fork: when --request-type is set, route to JSM path.
    // Platform path (when flag absent) is structurally unchanged. (BC-3.8.001, BC-3.3.001)
    if request_type.is_some() {
        // Emit stderr warnings for platform-only flags that are silently ignored on the
        // JSM path (BC-3.8.010, BC-3.8.011). Warnings fire BEFORE dispatch so they appear
        // even if dispatch errors out later (e.g., missing summary, bad request type).
        if issue_type.is_some() {
            eprintln!(
                "warning: --type is ignored when --request-type is set; request type encodes the issue type"
            );
        }
        if team.is_some() {
            eprintln!(
                "warning: --team is ignored when --request-type is set; teams are managed by the request type's workflow"
            );
        }
        if points.is_some() {
            eprintln!(
                "warning: --points is ignored when --request-type is set; story points are not part of JSM request schema"
            );
        }
        if parent.is_some() {
            eprintln!(
                "warning: --parent is ignored when --request-type is set; JSM requests cannot be sub-tasks"
            );
        }
        if to.is_some() {
            eprintln!(
                "warning: --to is ignored when --request-type is set; use --on-behalf-of to set the requester"
            );
        }
        if account_id.is_some() {
            eprintln!(
                "warning: --account-id is ignored when --request-type is set; use --on-behalf-of to set the requester"
            );
        }

        return handle_jsm_create(
            client,
            config,
            output_format,
            project_override,
            no_input,
            JsmCreateArgs {
                project,
                request_type,
                summary,
                description,
                description_stdin,
                priority,
                labels,
                markdown,
                on_behalf_of,
                field_pairs,
            },
        )
        .await;
    }

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
            .search_issue_keys(jql_str, Some(effective_max + 1))
            .await?;
        let matched_keys = search_result.keys;

        if matched_keys.is_empty() {
            return Err(JrError::UserError(format!(
                "JQL '{}' matched 0 issues. Refine your query or pass keys directly.",
                jql_str,
            ))
            .into());
        }

        if matched_keys.len() > effective_max as usize {
            return Err(JrError::UserError(format!(
                "JQL matched at least {} issues, which exceeds --max {}. \
                 Use --max <N> to allow up to {} issues, or refine your JQL.",
                matched_keys.len(),
                effective_max,
                BULK_MAX_KEYS,
            ))
            .into());
        }

        matched_keys
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
                    // NOTE: This entire dry-run preview block (labels here, plus
                    // `priority` and `issueType` below) emits INTENTIONALLY simplified
                    // shapes that DO NOT match the POST body shapes sent to Atlassian:
                    //   - `labels`: dry-run emits `[{"action": "ADD", "name": "foo"}]`
                    //     (flat array). POST body emits
                    //     `{"labelsAction": "ADD", "labels": [{"name": "foo"}]}` (nested,
                    //     or an array of those objects when ADD+REMOVE coalesce).
                    //   - `priority`: dry-run emits a bare string. POST body wraps as
                    //     `{"name": "..."}` (best-guess; Atlassian docs document
                    //     `{"priorityId": <int>}`).
                    //   - `issueType`: dry-run emits a bare string. POST body wraps as
                    //     `{"issuetype": {"name": "..."}}` (best-guess; Atlassian docs
                    //     document `{"issueTypeId": "..."}`).
                    // The dry-run JSON is a human-and-tool-friendly preview, NOT a
                    // byte-for-byte snapshot of the wire request. Rationale: all three
                    // POST shapes are best-guesses pending #331 empirical verification.
                    // Locking dry-run consumers to unverified canonical Atlassian
                    // shapes now would force a second breaking change once #331
                    // confirms the true shapes. Once #331 verifies the wire shapes,
                    // this dry-run builder can be unified with
                    // `handle_edit_bulk_labels` / `handle_edit_bulk_fields` to
                    // emit byte-identical JSON.
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

/// Build the edited-fields JSON object for a bulk-labels edit.
///
/// Returns the complete `editedFieldsInput` object (with a top-level `"labels"` key)
/// to be passed directly to `bulk_edit_fields`. JSON shapes per BC-3.4.006:
/// - Single-action ADD: object-form
///   `{"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}}`
/// - Single-action REMOVE: object-form
///   `{"labels": {"labelsAction": "REMOVE", "labels": [{"name": "bar"}]}}`
/// - Both-action (ADD + REMOVE): array-form with ADD at index 0, REMOVE at index 1
///   `{"labels": [{"labelsAction": "ADD", "labels": [{"name": "foo"}]}, {"labelsAction": "REMOVE", "labels": [{"name": "bar"}]}]}`
///
/// Caller MUST bail BEFORE calling this if both inputs are empty.
///
/// Pure function — no I/O, no async, no client refs. Enables proptest coverage
/// of the JSON shape invariants without wiremock.
///
/// Shape is best-guess (unverified against live Atlassian API; tracked at #331).
/// PR2 test asserts .expect(1) on bulk POST to ensure ADD+REMOVE coalesce into ONE call,
/// but the exact JSON nesting matches a loose `body_string_contains` matcher — schema
/// accuracy is the work being deferred to #331.
fn build_labels_edited_fields(adds: &[String], removes: &[String]) -> serde_json::Value {
    debug_assert!(
        !adds.is_empty() || !removes.is_empty(),
        "build_labels_edited_fields: caller MUST bail when both inputs are empty (BC-3.4.006)",
    );
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

    // Single-action: object-form (backward-compat with PR1 tests).
    // Both-action: coalesced array-form (single bulk POST for ADD+REMOVE).
    if label_ops.len() == 1 {
        let op = label_ops.remove(0);
        json!({ "labels": op })
    } else {
        json!({ "labels": label_ops })
    }
}

/// Route label edits through the Atlassian Bulk Fields API.
///
/// Supports 1..=1000 keys. `labels` is a list of "add:NAME" / "remove:NAME" / "NAME" strings.
///
/// NOTE: The `--dry-run --output json` `plannedChanges.labels` shape (built in the
/// dry-run block of `handle_edit` above) is a SIMPLIFIED preview using `{action, name}`
/// pairs in a flat array, NOT a byte-for-byte snapshot of the POST body built here.
/// Dry-run is a human-and-tool-friendly diff; the POST body is constructed by
/// `build_labels_edited_fields` (BC-3.4.006) and represents the current best-guess
/// Atlassian shape (still unverified, pending #331). Once #331 confirms the
/// canonical wire shape, both paths can converge.
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
/// per #331's Perplexity research, the canonical Atlassian schema is documented
/// to use a top-level `labelsFields` array — that's the long-term target for
/// both code paths once #331's empirical sandbox verification confirms it.
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

    // Coalesce ADD and REMOVE into a single bulk POST when both are present.
    // Both operations are submitted in one request as an array of label-action objects.
    // See build_labels_edited_fields doc-comment for the verbatim #331 schema caveat.
    let edited_fields = build_labels_edited_fields(&adds, &removes);

    // selectedActions for labels is always ["labels"] regardless of ADD/REMOVE/coalesce.
    let task_id = client
        .bulk_edit_fields(keys, vec!["labels".to_string()], edited_fields)
        .await?;
    // Poll with 5-minute timeout.
    let progress = client
        .await_bulk_task(&task_id, resolve_bulk_await_timeout())
        .await?;

    render_bulk_edit_results(keys, &task_id, &progress, output_format)
}

/// Route non-label multi-key edits through the Atlassian Bulk Fields API.
///
/// Supports 2..=1000 keys with --summary, --priority, --type.
///
/// NOTE: The `--dry-run --output json` `plannedChanges` block emits SIMPLIFIED
/// previews for these same fields (bare strings for `priority` and `issueType`,
/// see the dry-run builder in `handle_edit` above) that do NOT match the POST
/// body shapes built here. Dry-run is a human-and-tool-friendly diff; the POST
/// body shapes here are the current best-guess (still unverified, pending #331).
/// Once #331 confirms the canonical wire shapes and the bulk builders are
/// extracted into pure functions, the two paths can converge.
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
        .await_bulk_task(&task_id, resolve_bulk_await_timeout())
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
    use std::collections::BTreeSet;

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

    /// Categorization meta-test for `IssueCommand::Edit` fields (issue #343).
    ///
    /// # Why this test exists
    ///
    /// The C-1 fix in issue #110 part 2 added a hand-rolled rejection list at
    /// `handle_edit` (`if effective_keys.len() > 1 { ... }`) that returns an
    /// error when multi-key bulk edit is invoked with flags that only the
    /// single-key path implements. The original silent-drop bug was: a user
    /// passes `--parent X` with multiple keys, the flag is silently ignored,
    /// no error fires, and the user thinks the edit succeeded.
    ///
    /// The C-1 list is hand-rolled and depends on the developer remembering
    /// to update it whenever they add a new field to `IssueCommand::Edit`. If
    /// they don't, the silent-drop bug returns. This test catches that drift
    /// at compile-and-test time.
    ///
    /// # Strategy
    ///
    /// Source-text inspection: read `src/cli/mod.rs` at compile time via
    /// `include_str!`, locate the `IssueCommand::Edit {` block, and extract
    /// every field name declared inside it. Compare the extracted set against
    /// three hand-maintained categorization sets:
    ///
    /// - **SELECTORS** — flags that select which issues to edit, not what
    ///   to change: `keys`, `jql`, `max`, `yes`, `dry_run`.
    /// - **BULK_SUPPORTED** — field flags that work on multi-key bulk path:
    ///   `summary`, `issue_type`, `priority`, `label`.
    /// - **REJECTED_IN_BULK** — field flags that only work on single-key
    ///   path; multi-key invocation must error: `parent`, `no_parent`,
    ///   `team`, `points`, `no_points`, `description`, `description_stdin`,
    ///   `markdown`.
    ///
    /// The test asserts:
    /// 1. The union of the three sets equals the extracted field set.
    /// 2. The three sets are pairwise disjoint (no field in two categories).
    /// 3. Every category contains at least one field (sanity check).
    ///
    /// # Failure modes this catches
    ///
    /// - A new flag is added to `Edit` but not categorized: union mismatch.
    /// - A flag is moved between categories without updating both lists:
    ///   intersection violation OR union mismatch.
    /// - A flag is renamed in `Edit` but not in the routing code: extracted
    ///   set differs from category sets.
    ///
    /// # Maintenance protocol
    ///
    /// When a future PR adds a flag to `IssueCommand::Edit`:
    /// 1. This test fails with a diff between expected and actual sets.
    /// 2. The PR author decides which category the new flag belongs in:
    ///    - Selector? Add to `SELECTORS` here.
    ///    - Bulk-safe field? Add to `BULK_SUPPORTED` AND wire the bulk path
    ///      in `handle_edit_bulk_fields` (or similar) to honor it.
    ///    - Single-key-only field? Add to `REJECTED_IN_BULK` AND extend the
    ///      C-1 rejection block in `handle_edit` to surface a clear error.
    /// 3. The test passes only when both the test list and the routing code
    ///    agree on the new flag's category.
    ///
    /// Closes audit-followup #343.
    #[test]
    fn test_343_every_edit_field_is_categorized() {
        let cli_source = include_str!("../mod.rs");

        let edit_fields = extract_edit_field_names(cli_source);

        // SELECTORS — flags that pick which issues to edit, not what changes.
        let selectors: BTreeSet<&str> = [
            "keys",    // positional issue keys (single or multi-key)
            "jql",     // JQL match set for bulk edit
            "max",     // upper bound on JQL match count
            "yes",     // skip interactive confirmation for large match sets
            "dry_run", // preview only, no HTTP mutations
        ]
        .into_iter()
        .collect();

        // BULK_SUPPORTED — field flags that work in multi-key bulk context.
        // These must be honored by both the single-key path AND the bulk path
        // (handle_edit_bulk_fields / handle_edit_bulk_labels).
        let bulk_supported: BTreeSet<&str> = [
            "summary",    // text summary update
            "issue_type", // issue type change (clap flag: --type)
            "priority",   // priority change
            "label",      // add/remove labels via labels coalesce
        ]
        .into_iter()
        .collect();

        // REJECTED_IN_BULK — field flags that ONLY the single-key path implements.
        // Multi-key invocation with any of these MUST return an error from the
        // C-1 rejection block in handle_edit (see lines ~426-465 of this file).
        // Adding to this set without extending the rejection block reintroduces
        // the silent-drop bug C-1 was meant to fix.
        let rejected_in_bulk: BTreeSet<&str> = [
            "parent",
            "no_parent",
            "team",
            "points",
            "no_points",
            "description",
            "description_stdin",
            "markdown",
        ]
        .into_iter()
        .collect();

        // --- ASSERTIONS ---

        // 1. Each category has at least one field (sanity check; protects
        //    against an empty hardcoded list slipping through unnoticed).
        assert!(!selectors.is_empty(), "SELECTORS must not be empty");
        assert!(
            !bulk_supported.is_empty(),
            "BULK_SUPPORTED must not be empty"
        );
        assert!(
            !rejected_in_bulk.is_empty(),
            "REJECTED_IN_BULK must not be empty"
        );

        // 2. Pairwise disjoint — no field categorized in more than one set.
        let s_b: BTreeSet<&&str> = selectors.intersection(&bulk_supported).collect();
        assert!(
            s_b.is_empty(),
            "SELECTORS and BULK_SUPPORTED overlap: {s_b:?} — every field belongs to exactly one category"
        );
        let s_r: BTreeSet<&&str> = selectors.intersection(&rejected_in_bulk).collect();
        assert!(
            s_r.is_empty(),
            "SELECTORS and REJECTED_IN_BULK overlap: {s_r:?} — every field belongs to exactly one category"
        );
        let b_r: BTreeSet<&&str> = bulk_supported.intersection(&rejected_in_bulk).collect();
        assert!(
            b_r.is_empty(),
            "BULK_SUPPORTED and REJECTED_IN_BULK overlap: {b_r:?} — every field belongs to exactly one category"
        );

        // 3. Union equals the extracted set — every Edit field is categorized
        //    AND no category lists a field that doesn't exist in Edit.
        let categorized: BTreeSet<String> = selectors
            .iter()
            .chain(bulk_supported.iter())
            .chain(rejected_in_bulk.iter())
            .map(|s| (*s).to_string())
            .collect();

        let missing_from_categories: Vec<&String> = edit_fields
            .iter()
            .filter(|f| !categorized.contains(*f))
            .collect();
        let missing_from_edit: Vec<&String> = categorized
            .iter()
            .filter(|c| !edit_fields.contains(*c))
            .collect();

        assert!(
            missing_from_categories.is_empty(),
            "Issue #343 VIOLATION: `IssueCommand::Edit` fields not categorized: {missing_from_categories:?}.\n\
             A new flag was added to src/cli/mod.rs::IssueCommand::Edit without being placed in one of\n\
             SELECTORS, BULK_SUPPORTED, or REJECTED_IN_BULK in this test.\n\
             Decide which category applies and update both this test AND the matching routing code\n\
             in handle_edit (see the maintenance protocol in this test's doc comment).\n\
             Extracted Edit fields: {edit_fields:?}\n\
             Currently categorized: {categorized:?}"
        );
        assert!(
            missing_from_edit.is_empty(),
            "Issue #343 VIOLATION: category sets reference fields that no longer exist on `IssueCommand::Edit`: {missing_from_edit:?}.\n\
             A field was renamed or removed in src/cli/mod.rs without updating this test.\n\
             Extracted Edit fields: {edit_fields:?}\n\
             Currently categorized: {categorized:?}"
        );
    }

    // R2 pins for the formatting-tolerant closing-brace matcher
    // (extract_edit_field_names). These feed synthetic source text through the
    // extractor and confirm it copes with rustfmt-produced variants of the
    // closing `}` line.

    #[test]
    fn test_343_extractor_tolerates_no_trailing_comma() {
        // If `Edit` is the LAST variant in the enum, rustfmt may emit `}`
        // with no trailing comma. The matcher must still find it.
        let synthetic = "\
pub enum IssueCommand {
    Edit {
        keys: Vec<String>,
        summary: Option<String>,
    }
}
";
        let fields = extract_edit_field_names(synthetic);
        assert_eq!(
            fields,
            BTreeSet::from(["keys".to_string(), "summary".to_string()])
        );
    }

    #[test]
    fn test_343_extractor_tolerates_trailing_comment_on_closing() {
        // `},  // last variant` should still match.
        let synthetic = "\
pub enum IssueCommand {
    Edit {
        keys: Vec<String>,
        jql: Option<String>,
    },  // closing comment
}
";
        let fields = extract_edit_field_names(synthetic);
        assert_eq!(
            fields,
            BTreeSet::from(["keys".to_string(), "jql".to_string()])
        );
    }

    #[test]
    fn test_343_extractor_tolerates_trailing_whitespace_on_closing() {
        // `},   ` (closing with stray trailing spaces) — rustfmt usually strips
        // these but some editors may produce them; matcher must still cope.
        let synthetic =
            "pub enum IssueCommand {\n    Edit {\n        keys: Vec<String>,\n    },   \n}\n";
        let fields = extract_edit_field_names(synthetic);
        assert_eq!(fields, BTreeSet::from(["keys".to_string()]));
    }

    /// Helper: extract all field names declared inside the `IssueCommand::Edit {`
    /// variant in `src/cli/mod.rs`. Operates on the source text so it does not
    /// require any compile-time reflection or third-party derive macro.
    ///
    /// Strategy:
    /// 1. Locate the `Edit {` line (matched by `trim_start().starts_with("Edit {")`,
    ///    so the variant's own indent is irrelevant).
    /// 2. Walk forward until the matching closing brace via
    ///    `is_matching_closing_brace` — tolerant of rustfmt-equivalent shapes:
    ///    `}` followed by optional `,`, optional whitespace, and optional
    ///    line-comment, all at the same indent prefix as the opening line.
    ///    See the closure's inline comment for the exact rules.
    /// 3. Inside that range, treat any trimmed line of the form `<name>: <type>...`
    ///    (any indent — fields are detected by the `name:` shape, not by
    ///    column position) as a field declaration. Skip lines that start with
    ///    `#[` (attributes), `//` (line/doc comments), or are blank.
    ///
    /// Returns the extracted field names as a `BTreeSet<String>` so the
    /// iteration/`Debug` output order is deterministic — assertion failure
    /// messages produce stable, reviewable diffs across runs and machines.
    /// (`HashSet` would not satisfy this: its iteration order depends on the
    /// hash seed, which varies per process.)
    fn extract_edit_field_names(source: &str) -> BTreeSet<String> {
        let lines: Vec<&str> = source.lines().collect();

        let edit_start = lines
            .iter()
            .position(|l| l.trim_start().starts_with("Edit {"))
            .expect(
                "Could not locate `Edit {` in src/cli/mod.rs — has the variant been renamed?\n\
                 Update the extractor to match the new variant name.",
            );

        // The opening line is `    Edit {` (4-space indent for a clap subcommand
        // variant). The closing line begins with `}` at the SAME indent as the
        // opening line. Match tolerantly so the meta-test fails only on
        // semantic drift (the variant being renamed/removed/restructured), not
        // on benign rustfmt-produced formatting changes such as:
        //   - `}` followed by `,` and a comment: `    }, // comment`
        //   - last-variant `}` with no trailing comma (Rust allows this when
        //     `Edit` is the final variant in the enum)
        //   - trailing whitespace after the brace/comma
        //
        // Logic:
        //   1. Line must start with exactly `opening_indent_width` spaces
        //      followed by `}`. Field-internal braces sit at a deeper indent
        //      (more spaces than `opening_indent_width`), so the `}` is no
        //      longer at byte `closing_indent.len()` and `strip_prefix('}')`
        //      below rejects them. The opener's own indent isn't hard-coded
        //      — `opening_indent_width` is captured from the actual line.
        //   2. After the `}`, only allow: end-of-line, `,`, whitespace, or a
        //      line-comment (`//...`). Anything else means we hit a different
        //      construct and must keep scanning.
        let opening_indent_width = lines[edit_start].len() - lines[edit_start].trim_start().len();
        let closing_indent: String = " ".repeat(opening_indent_width);

        let is_matching_closing_brace = |line: &str| -> bool {
            // 1. Line must start with EXACTLY `closing_indent` spaces, and the
            //    next char must be `}`. A deeper-indented `}` (e.g., the closer
            //    of a nested struct inside a field) has more spaces after the
            //    prefix, so `strip_prefix('}')` fails and we reject below.
            if !line.starts_with(&closing_indent) {
                return false;
            }
            let rest = &line[closing_indent.len()..];
            let Some(after_brace) = rest.strip_prefix('}') else {
                return false;
            };
            // 2. After `}`, accept (in order): optional `,`, optional
            //    whitespace, optional `//`-comment, or end-of-line.
            let after_optional_comma = after_brace.strip_prefix(',').unwrap_or(after_brace);
            let trailing = after_optional_comma.trim_start();
            trailing.is_empty() || trailing.starts_with("//")
        };

        let edit_end = lines
            .iter()
            .enumerate()
            .skip(edit_start + 1)
            .find(|(_, l)| is_matching_closing_brace(l))
            .map(|(i, _)| i)
            .expect(
                "Could not locate matching closing brace for `Edit {{` block in src/cli/mod.rs.\n\
                 Expected a line starting with the same indent as `Edit {{`, containing `}}` \
                 optionally followed by `,` and optional whitespace/comment.\n\
                 The variant may have been renamed, removed, or significantly restructured \
                 — update the extractor to match the new shape.",
            );

        let mut fields = BTreeSet::new();

        for line in &lines[edit_start + 1..edit_end] {
            let trimmed = line.trim_start();
            // Skip attributes, doc comments, blank lines, and inline comments.
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#[") {
                continue;
            }
            // Match patterns like `name: Type,` or `name: Type<...>,`.
            // A field declaration line starts with an identifier followed by `:`.
            // We extract everything up to the first `:` and validate it as an
            // identifier.
            if let Some((ident, _rest)) = trimmed.split_once(':') {
                let ident = ident.trim();
                let is_valid_ident = !ident.is_empty()
                    && ident
                        .chars()
                        .next()
                        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
                    && ident.chars().all(|c| c == '_' || c.is_ascii_alphanumeric());
                if is_valid_ident {
                    fields.insert(ident.to_string());
                }
            }
        }

        assert!(
            !fields.is_empty(),
            "Field extraction returned an empty set for `IssueCommand::Edit` — \
             the extractor regex/parser likely no longer matches the variant's \
             formatting. Update extract_edit_field_names() to match the current source."
        );

        fields
    }
}

#[cfg(test)]
mod build_labels_proptests {
    use super::build_labels_edited_fields;
    use proptest::prelude::*;
    use serde_json::Value;

    proptest! {
        /// BC-3.4.006 invariants: top-level "labels" key always present; ADD/REMOVE
        /// entries appear iff respective input non-empty; single-action uses object-form;
        /// both-action uses array-form with ADD-at-index-0, REMOVE-at-index-1.
        ///
        /// `prop_assume!` filters out the empty/empty case because the caller
        /// (`handle_edit_bulk_labels`) bails on `adds.is_empty() && removes.is_empty()`.
        #[test]
        fn build_labels_edited_fields_invariants(
            adds in proptest::collection::vec("[a-z]{1,10}", 0..5),
            removes in proptest::collection::vec("[a-z]{1,10}", 0..5),
        ) {
            prop_assume!(!adds.is_empty() || !removes.is_empty());

            let result = build_labels_edited_fields(&adds, &removes);

            // Invariant 0: top-level value MUST be a JSON object with EXACTLY one key ("labels").
            let obj = result.as_object().expect("BC-3.4.006: top-level value MUST be a JSON object");
            prop_assert_eq!(obj.len(), 1, "BC-3.4.006: top-level object MUST have exactly one key ('labels')");

            // Invariant 1: top-level "labels" key is always present.
            let labels = result.get("labels").expect("BC-3.4.006: top-level 'labels' key MUST be present");

            // Strict shape-pinning helper — panics with BC-3.4.006 message on any
            // key-set deviation so a regression that emits {"WRONG_KEY": "foo"} instead
            // of {"name": "foo"} fails loudly rather than being silently dropped.
            let extract_action_and_names = |action_obj: &Value| -> (String, Vec<String>) {
                let obj = action_obj.as_object().expect(
                    "BC-3.4.006: each action entry MUST be a JSON object",
                );
                assert_eq!(
                    obj.len(), 2,
                    "BC-3.4.006: each action entry MUST have EXACTLY 2 keys (labelsAction + labels), got: {:?}",
                    obj.keys().collect::<Vec<_>>()
                );
                let action = obj
                    .get("labelsAction")
                    .and_then(|v| v.as_str())
                    .expect("BC-3.4.006: action entry MUST have labelsAction: String")
                    .to_string();
                let inner_labels = obj
                    .get("labels")
                    .and_then(|v| v.as_array())
                    .expect("BC-3.4.006: action entry MUST have labels: Array");
                let names: Vec<String> = inner_labels
                    .iter()
                    .map(|item| {
                        let item_obj = item.as_object().expect(
                            "BC-3.4.006: each label entry MUST be a JSON object",
                        );
                        assert_eq!(
                            item_obj.len(), 1,
                            "BC-3.4.006: each label entry MUST have EXACTLY 1 key (name), got: {:?}",
                            item_obj.keys().collect::<Vec<_>>()
                        );
                        item_obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .expect("BC-3.4.006: each label entry MUST have name: String")
                            .to_string()
                    })
                    .collect();
                (action, names)
            };

            match (adds.is_empty(), removes.is_empty()) {
                // Both-action: array-form with ADD then REMOVE.
                (false, false) => {
                    let arr = labels.as_array().expect(
                        "BC-3.4.006: both-action MUST produce array-form (not object-form)",
                    );
                    prop_assert_eq!(arr.len(), 2, "BC-3.4.006: array-form MUST have exactly 2 entries (ADD + REMOVE)");
                    let (a0_action, a0_names) = extract_action_and_names(&arr[0]);
                    let (a1_action, a1_names) = extract_action_and_names(&arr[1]);
                    prop_assert_eq!(a0_action, "ADD",    "BC-3.4.006: array index 0 MUST be ADD");
                    prop_assert_eq!(a1_action, "REMOVE", "BC-3.4.006: array index 1 MUST be REMOVE");
                    prop_assert_eq!(a0_names, adds.clone(),    "BC-3.4.006: ADD names MUST match input");
                    prop_assert_eq!(a1_names, removes.clone(), "BC-3.4.006: REMOVE names MUST match input");
                }
                // Single-action ADD: object-form, labelsAction=ADD.
                (false, true) => {
                    let obj = labels.as_object().expect(
                        "BC-3.4.006: single-action ADD MUST produce object-form (not array-form)",
                    );
                    prop_assert_eq!(
                        obj.len(), 2,
                        "BC-3.4.006: single-ADD labels object MUST have EXACTLY 2 keys (labelsAction + labels), got: {:?}",
                        obj.keys().collect::<Vec<_>>()
                    );
                    let (action, names) = extract_action_and_names(&Value::Object(obj.clone()));
                    prop_assert_eq!(action, "ADD", "BC-3.4.006: single-ADD MUST set labelsAction=ADD");
                    prop_assert_eq!(names, adds.clone(), "BC-3.4.006: ADD names MUST match input");
                }
                // Single-action REMOVE: object-form, labelsAction=REMOVE.
                (true, false) => {
                    let obj = labels.as_object().expect(
                        "BC-3.4.006: single-action REMOVE MUST produce object-form (not array-form)",
                    );
                    prop_assert_eq!(
                        obj.len(), 2,
                        "BC-3.4.006: single-REMOVE labels object MUST have EXACTLY 2 keys (labelsAction + labels), got: {:?}",
                        obj.keys().collect::<Vec<_>>()
                    );
                    let (action, names) = extract_action_and_names(&Value::Object(obj.clone()));
                    prop_assert_eq!(action, "REMOVE", "BC-3.4.006: single-REMOVE MUST set labelsAction=REMOVE");
                    prop_assert_eq!(names, removes.clone(), "BC-3.4.006: REMOVE names MUST match input");
                }
                // Both empty: filtered by prop_assume!; unreachable.
                (true, true) => unreachable!("filtered by prop_assume! above"),
            }
        }
    }
}

/// Proptest properties for `parse_field_kv` (AC-013, BC-3.8.008).
///
/// Properties A.1–A.4 cover the four invariants stated in the verification delta.
#[cfg(test)]
mod parse_field_kv_proptests {
    use super::parse_field_kv;
    use proptest::prelude::*;

    proptest! {
        /// A.1 (BC-3.8.008): first `=` is the delimiter; subsequent `=` chars
        /// are part of the value. For any valid NAME and VALUE (which may contain
        /// `=` chars), round-tripping through parse_field_kv preserves the value.
        #[test]
        fn prop_parse_field_kv_first_equals_split(
            name in "[a-z][a-z0-9_]{0,19}",
            value_prefix in "[a-z]{1,10}",
            value_suffix in "[=a-z0-9]{0,10}",
        ) {
            let pair = format!("{name}={value_prefix}={value_suffix}");
            let pairs = vec![pair];
            let result = parse_field_kv(&pairs)
                .unwrap_or_else(|e| panic!("A.1: parse_field_kv must succeed for valid pair; got error: {e:?}"));
            let expected_value = format!("{value_prefix}={value_suffix}");
            prop_assert_eq!(
                result.get(&name).map(String::as_str),
                Some(expected_value.as_str()),
                "A.1: BC-3.8.008 first-equals split must yield full value after first '='"
            );
        }

        /// A.2 (BC-3.8.008): empty value is allowed. `key=` produces `{"key": ""}`.
        #[test]
        fn prop_parse_field_kv_empty_value_allowed(
            name in "[a-z][a-z0-9_]{0,19}",
        ) {
            let pair = format!("{name}=");
            let pairs = vec![pair];
            let result = parse_field_kv(&pairs)
                .unwrap_or_else(|e| panic!("A.2: parse_field_kv must accept 'name=' (empty value); got error: {e:?}"));
            prop_assert_eq!(
                result.get(&name).map(String::as_str),
                Some(""),
                "A.2: BC-3.8.008 empty value after '=' must be accepted and preserved"
            );
        }

        /// A.3 (BC-3.8.008): duplicate key — last value wins.
        /// Two pairs with the same key must result in only the second value.
        #[test]
        fn prop_parse_field_kv_last_value_wins_on_duplicates(
            name in "[a-z][a-z0-9_]{0,19}",
            first_val in "[a-z]{1,10}",
            last_val in "[a-z]{1,10}",
        ) {
            let pairs = vec![
                format!("{name}={first_val}"),
                format!("{name}={last_val}"),
            ];
            let result = parse_field_kv(&pairs)
                .unwrap_or_else(|e| panic!("A.3: parse_field_kv must succeed for duplicate key pairs; got error: {e:?}"));
            prop_assert_eq!(
                result.get(&name).map(String::as_str),
                Some(last_val.as_str()),
                "A.3: BC-3.8.008 duplicate key: last value must win"
            );
            prop_assert_eq!(
                result.len(),
                1,
                "A.3: BC-3.8.008 duplicate keys must collapse to one entry"
            );
        }

        /// A.4 (BC-3.8.008): no panic on arbitrary input — any string that
        /// contains at least one `=` must parse without panic (may return Ok or Err).
        #[test]
        fn prop_parse_field_kv_no_panic_on_arbitrary_input(
            raw in ".{0,80}",
        ) {
            // The function contract: no panic for any input.
            // Ok or Err is both acceptable; only panics are forbidden.
            let pairs = vec![raw];
            let _ = parse_field_kv(&pairs); // must not panic
        }
    }
}

/// Parse `--field NAME=VALUE` pairs into a `HashMap<String, String>`.
///
/// Splitting rule (BC-3.8.008): the FIRST `=` in each pair separates name from
/// value. Any subsequent `=` characters are part of the value. Duplicate keys
/// use the last value provided (last-wins). A pair without `=` is a user error
/// (exit 64 via [`JrError::UserError`]).
///
/// # Errors
///
/// Returns `JrError::UserError` if any pair is missing `=`.
pub(crate) fn parse_field_kv(pairs: &[String]) -> Result<HashMap<String, String>, JrError> {
    let mut map = HashMap::new();
    for pair in pairs {
        let Some(eq_pos) = pair.find('=') else {
            return Err(JrError::UserError(format!(
                "--field \"{pair}\" is not a valid NAME=VALUE pair: missing '='. \
                 Use --field NAME=VALUE (e.g., --field customfield_10200=foo)."
            )));
        };
        let key = pair[..eq_pos].to_string();
        let value = pair[eq_pos + 1..].to_string();
        // Last-wins for duplicate keys (BC-3.8.008).
        map.insert(key, value);
    }
    Ok(map)
}

/// Argument bundle for `handle_jsm_create`.
///
/// Reduces argument count on `handle_jsm_create` to satisfy `clippy::too_many_arguments`
/// (CLAUDE.md policy: refactor rather than `#[allow]`).
///
/// # Field policy
///
/// `IssueCommand::Create` carries 16+ flags. The JSM dispatch path uses a subset.
/// Each `Create` flag falls into one of three categories:
///
/// **Pass-through to JSM (included in this struct):**
/// - `project`, `request_type`, `summary`, `description`, `description_stdin`,
///   `priority`, `labels`, `markdown`, `on_behalf_of`, `field_pairs`
///
/// **Ignored with stderr warning (handled BEFORE dispatch in `handle_create`,
/// per BC-3.8.010 + BC-3.8.011):**
/// - `issue_type` (`--type`): JSM request types replace it
/// - `team` (`--team`): not in JSM request schema
/// - `points` (`--points`): not in JSM request schema
/// - `parent` (`--parent`): JSM requests cannot be sub-tasks
/// - `to` (`--to`): superseded by `--on-behalf-of` (raiseOnBehalfOf)
/// - `account_id` (`--account-id`): superseded by `--on-behalf-of`
///
/// **No-op on JSM (silently dropped):**
/// - (none currently — every Create flag is either passed or warned)
///
/// When adding a new `Create` flag, decide which category it belongs to and add it
/// to this list to keep future maintainers from re-discovering the matrix.
struct JsmCreateArgs {
    project: Option<String>,
    request_type: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    description_stdin: bool,
    priority: Option<String>,
    labels: Vec<String>,
    markdown: bool,
    on_behalf_of: Option<String>,
    field_pairs: Vec<String>,
}

/// Orchestrate a JSM customer-request creation.
///
/// Called by [`handle_create`] when `--request-type` is present. Never called
/// when `--request-type` is absent (platform path is the fall-through).
///
/// Steps (BC-3.8.001..010):
/// 1. Resolve the service desk ID via [`servicedesks::require_service_desk`]
///    with label `` "`jr issue create --request-type` requires" ``.
/// 2. Resolve `request_type_arg`: if all-digits → use as-is (numeric bypass,
///    BC-3.8.004); else → read cache / fetch via `list_request_types` /
///    `partial_match`. Ambiguous or missing → exit 64.
/// 3. Build `requestFieldValues` from `--summary`, `--description` (ADF),
///    `--priority`, `--label`, `--field` via [`parse_field_kv`].
/// 4. If `--type` is also set → emit stderr warning (BC-3.8.010). Do NOT error.
/// 5. Build body via [`JsmRequestBuilder`].
/// 6. POST via [`JiraClient::create_jsm_request`].
/// 7. Emit `{"key": "<issue_key>"}` on stdout (`--output json` shape per AC-015).
async fn handle_jsm_create(
    client: &JiraClient,
    config: &Config,
    output_format: &OutputFormat,
    project_override: Option<&str>,
    no_input: bool,
    args: JsmCreateArgs,
) -> Result<()> {
    let JsmCreateArgs {
        project,
        request_type,
        summary,
        description,
        description_stdin,
        priority,
        labels,
        markdown,
        on_behalf_of,
        field_pairs,
    } = args;

    // Resolve the request_type arg — we know it's Some because this function is only
    // called when request_type.is_some().
    let request_type_arg = request_type.expect("handle_jsm_create called without --request-type");

    // Resolve project key (BC-3.8.001).
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Project key").ok()
            }
        })
        .ok_or_else(|| JrError::UserError("project is required for JSM request creation".into()))?;

    // M-01 (adversary pass-02-retry) + platform-path parity: --markdown requires
    // a description source on the JSM path, just like the platform path.
    if markdown && description.is_none() && !description_stdin {
        return Err(JrError::UserError(
            "--markdown requires --description or --description-stdin to take effect. \
             Pass a description alongside --markdown, or omit --markdown."
                .into(),
        )
        .into());
    }

    // Resolve service desk ID — errors with BC-X.8.004 message for non-JSM projects
    // (BC-3.8.002). Call-site label "`jr issue create --request-type` requires".
    let service_desk_id = servicedesks::require_service_desk(
        client,
        &project_key,
        "`jr issue create --request-type` requires",
    )
    .await?;

    let profile = &config.active_profile_name;

    // Resolve request type ID (BC-3.8.003, BC-3.8.004).
    let request_type_id =
        if !request_type_arg.is_empty() && request_type_arg.chars().all(|c| c.is_ascii_digit()) {
            // Numeric bypass — use directly without list endpoint call (BC-3.8.004).
            request_type_arg.clone()
        } else {
            // Name resolution: cache → API → partial_match (BC-3.8.003).
            resolve_jsm_request_type_id(
                &request_type_arg,
                &service_desk_id,
                &project_key,
                profile,
                client,
            )
            .await?
        };

    // Resolve summary (BC-3.8.005).
    let summary_text = summary
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Summary").ok()
            }
        })
        .ok_or_else(|| {
            JrError::UserError(
                "summary is required for JSM request submission. Use --summary.".into(),
            )
        })?;

    // Resolve description. spawn_blocking isolates the blocking stdin read from the
    // tokio runtime so later async work isn't starved while waiting on piped input.
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

    // Parse --field NAME=VALUE pairs (BC-3.8.008).
    let extra_fields = parse_field_kv(&field_pairs)?;

    // Build the POST body (BC-3.8.005..009).
    let body = JsmRequestBuilder {
        service_desk_id: &service_desk_id,
        request_type_id: &request_type_id,
        summary: &summary_text,
        description: desc_text.as_deref(),
        markdown,
        priority: priority.as_deref(),
        labels: &labels,
        on_behalf_of: on_behalf_of.as_deref(),
        extra_fields: &extra_fields,
    }
    .build();

    // POST to /rest/servicedeskapi/request (BC-3.8.001).
    // On 401: surface a scope-mismatch hint pointing at write:servicedesk-request
    // (BC-1.3.023, BC-X.3.005, H-NEW-JSM-RT-003).
    // Both Basic-auth 401 (`NotAuthenticated`) and OAuth scope-mismatch 401
    // (`InsufficientScope`) must produce the write:servicedesk-request hint (C-01).
    let created =
        client
            .create_jsm_request(body)
            .await
            .map_err(|e| match e.downcast::<JrError>() {
                Ok(JrError::NotAuthenticated { .. }) => {
                    anyhow::anyhow!(JrError::NotAuthenticated {
                        hint: "The `write:servicedesk-request` OAuth scope may be missing. \
                       Run `jr auth refresh` or `jr auth login` to re-consent with \
                       the updated scope."
                            .to_string(),
                    })
                }
                Ok(JrError::InsufficientScope { message, .. }) => {
                    anyhow::anyhow!(JrError::InsufficientScope {
                        message: format!(
                            "{message} (`jr issue create --request-type` requires the \
                         `write:servicedesk-request` OAuth scope. \
                         Run `jr auth refresh` to refresh, or `jr auth login` to re-authorize \
                         with updated scopes.)"
                        ),
                        required_scope: Some("write:servicedesk-request".to_string()),
                    })
                }
                Ok(other) => anyhow::anyhow!(other),
                Err(other) => other,
            })?;

    // Emit output (AC-015, BC-3.8.001).
    let issue_key = &created.issue_key;
    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::json!({"key": issue_key}));
        }
        OutputFormat::Table => {
            output::print_success(&format!("Created request {issue_key}"));
        }
    }

    Ok(())
}

/// Resolve a request type name to its ID for the JSM create path.
///
/// Mirrors `cli/requesttype.rs::resolve_request_type_id` — cache → fetch → `partial_match`.
async fn resolve_jsm_request_type_id(
    name: &str,
    service_desk_id: &str,
    project_key: &str,
    profile: &str,
    client: &JiraClient,
) -> Result<String> {
    let types = match cache::read_request_type_cache(profile, service_desk_id)? {
        Some(cached) => cached,
        None => {
            let fetched = client.list_request_types(service_desk_id, None).await?;
            // `write_request_type_cache` is a best-effort writer per CLAUDE.md gotcha —
            // it swallows IO errors via eprintln and returns Ok(()). Use `let _` to make
            // the no-propagation intent explicit (the `?` would be dead code).
            let _ = cache::write_request_type_cache(profile, service_desk_id, &fetched);
            fetched
        }
    };

    let names: Vec<String> = types.iter().map(|t| t.name.clone()).collect();

    match partial_match::partial_match(name, &names) {
        MatchResult::Exact(matched_name) => {
            let id = types
                .iter()
                .find(|t| t.name == matched_name)
                .map(|t| t.id.clone())
                .expect("partial_match::Exact match must exist in types");
            Ok(id)
        }
        MatchResult::ExactMultiple(matched_name) => {
            let matched_lower = matched_name.to_lowercase();
            let ids: Vec<String> = types
                .iter()
                .filter(|t| t.name.to_lowercase() == matched_lower)
                .map(|t| t.id.clone())
                .collect();
            Err(JrError::UserError(format!(
                "Multiple request types named \"{matched_name}\" found (IDs: {}). \
                 Pass the numeric ID directly.",
                ids.join(", ")
            ))
            .into())
        }
        MatchResult::Ambiguous(matches) => Err(JrError::UserError(format!(
            "Ambiguous request type \"{name}\" matches: {}. \
             Run `jr requesttype list --project {project_key}` to see all request types.",
            matches
                .iter()
                .map(|m| format!("\"{m}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into()),
        MatchResult::None(_) => {
            let cache_path =
                cache::cache_dir(profile).join(format!("request_types_{service_desk_id}.json"));
            Err(JrError::UserError(format!(
                "Request type \"{name}\" not found. \
                 Run `jr requesttype list --project {project_key}` to see all request types, \
                 or delete the cache file at {} \
                 if a recent admin change is suspected.",
                cache_path.display()
            ))
            .into())
        }
    }
}
