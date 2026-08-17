use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cache;
use crate::cli::AssigneeType;
use crate::cli::OutputFormat;
use crate::cli::issue::resolve_component;
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::partial_match::MatchResult;

use super::ComponentSubcommand;

/// Top-level dispatch for `jr component` subcommands.
pub async fn handle(
    command: ComponentSubcommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_flag: Option<&str>,
) -> Result<()> {
    match command {
        ComponentSubcommand::List { project, counts } => {
            handle_list(
                project.as_deref().or(project_flag),
                counts,
                output_format,
                config,
                client,
            )
            .await
        }
        ComponentSubcommand::Create {
            project,
            name,
            description,
            lead,
            assignee_type,
        } => {
            handle_create(
                CreateComponentArgs {
                    project,
                    name,
                    description,
                    lead,
                    assignee_type,
                },
                output_format,
                config,
                client,
            )
            .await
        }
        ComponentSubcommand::Edit {
            name_or_id,
            project,
            name,
            description,
            lead,
        } => {
            handle_edit(
                EditComponentArgs {
                    name_or_id,
                    project,
                    new_name: name,
                    description,
                    lead,
                },
                output_format,
                config,
                client,
            )
            .await
        }
    }
}

/// Handle `jr component list [--project KEY] [--output json] [--counts]`.
///
/// BC-8.1.001 — BC-8.1.004.
/// Uses `resolve_component` (BC-8.4.001 — BC-8.4.005) for --component flag
/// resolution on future `issue create/edit` paths as well as this list command.
async fn handle_list(
    project: Option<&str>,
    counts: bool,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    // BC-8.1.004: no project → exit 64 before any HTTP call.
    let project_key = config.project_key(project).ok_or_else(|| {
        JrError::UserError(
            "No project configured. Pass --project KEY or set \
             project = \"...\" in .jr.toml. Run \"jr project list\" to see available projects."
                .into(),
        )
    })?;

    // Fetch the component list (assumed non-paginated per BC-8.1.001 / ADR-0018).
    let mut components = client.list_components(&project_key).await?;

    // BC-8.1.003: N+1 enrichment when --counts is requested.
    // Fail-soft: a 5xx on one component's count renders '?' (table) / null (JSON),
    // emits a stderr warning naming the component, and does NOT fail the command.
    if counts {
        for c in &mut components {
            match client.get_related_issue_counts(&c.id).await {
                Ok(rc) => {
                    c.related_issue_count = Some(rc.issue_count);
                }
                Err(e) => {
                    eprintln!(
                        "warning: failed to fetch issue count for component {} ({}): {e}",
                        c.name, c.id
                    );
                    c.related_issue_count = None;
                }
            }
        }
    }

    // Build table rows — columns: ID, Name, Description, Lead, Assignee Type [, Issues]
    let dash = "-".to_string();
    let question = "?".to_string();

    if counts {
        let rows: Vec<Vec<String>> = components
            .iter()
            .map(|c| {
                vec![
                    c.id.clone(),
                    c.name.clone(),
                    c.description.clone().unwrap_or_else(|| dash.clone()),
                    c.lead
                        .as_ref()
                        .and_then(|l| l.display_name.clone())
                        .unwrap_or_else(|| dash.clone()),
                    c.assignee_type.clone().unwrap_or_else(|| dash.clone()),
                    c.related_issue_count
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| question.clone()),
                ]
            })
            .collect();
        // Build JSON objects that are a strict superset of the plain JSON (BC-8.1.003):
        // serialize the full Component (which includes isAssigneeTypeValid when Some,
        // and all other fields from BC-8.1.002), then:
        //   - remove `relatedIssueCount` (internal name; replaced by `issueCount`)
        //   - insert `issueCount` (integer on success, JSON null on fail-soft — key
        //     ALWAYS present per BC-8.1.003).
        let json_view: Vec<serde_json::Value> = components
            .iter()
            .map(|c| {
                let mut v =
                    serde_json::to_value(c).expect("Component serializes to JSON infallibly");
                if let Some(obj) = v.as_object_mut() {
                    obj.remove("relatedIssueCount");
                    let count_val = match c.related_issue_count {
                        Some(n) => serde_json::Value::from(n),
                        None => serde_json::Value::Null,
                    };
                    obj.insert("issueCount".to_string(), count_val);
                }
                v
            })
            .collect();
        output::print_output(
            output_format,
            &[
                "ID",
                "Name",
                "Description",
                "Lead",
                "Assignee Type",
                "Issues",
            ],
            &rows,
            &json_view,
        )?;
    } else {
        let rows: Vec<Vec<String>> = components
            .iter()
            .map(|c| {
                vec![
                    c.id.clone(),
                    c.name.clone(),
                    c.description.clone().unwrap_or_else(|| dash.clone()),
                    c.lead
                        .as_ref()
                        .and_then(|l| l.display_name.clone())
                        .unwrap_or_else(|| dash.clone()),
                    c.assignee_type.clone().unwrap_or_else(|| dash.clone()),
                ]
            })
            .collect();
        output::print_output(
            output_format,
            &["ID", "Name", "Description", "Lead", "Assignee Type"],
            &rows,
            &components,
        )?;
    }

    Ok(())
}

/// Caller-supplied arguments for `handle_create`.
///
/// Bundles the five command-specific parameters so `handle_create` stays
/// within clippy's 7-argument limit (same pattern as `JsmUploadOpts`).
struct CreateComponentArgs {
    project: String,
    name: String,
    description: Option<String>,
    lead: Option<String>,
    assignee_type: Option<AssigneeType>,
}

/// Caller-supplied arguments for `handle_edit`.
///
/// Bundles the five command-specific parameters so `handle_edit` stays
/// within clippy's 7-argument limit.
struct EditComponentArgs {
    name_or_id: String,
    project: Option<String>,
    new_name: Option<String>,
    description: Option<String>,
    lead: Option<String>,
}

/// Maps a clap `AssigneeType` variant to the Jira API string value.
fn assignee_type_to_api_str(at: &AssigneeType) -> &'static str {
    match at {
        AssigneeType::ComponentLead => "COMPONENT_LEAD",
        AssigneeType::ProjectLead => "PROJECT_LEAD",
        AssigneeType::Unassigned => "UNASSIGNED",
        AssigneeType::ProjectDefault => "PROJECT_DEFAULT",
    }
}

/// Handle `jr component create --project KEY NAME [options]`.
///
/// BC-8.1.005 (create POST) + BC-8.1.006 (--lead resolution) (S-604-2).
/// POSTs `/rest/api/3/component`, resolves lead via
/// `search_assignable_users_by_project`, then invalidates the project's
/// component cache entry (ADR-0018 §2).
async fn handle_create(
    args: CreateComponentArgs,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let CreateComponentArgs {
        project,
        name,
        description,
        lead,
        assignee_type,
    } = args;

    // BC-8.1.006: `--lead ""` has no effect on create — exit 64 before any HTTP.
    if let Some(ref lead_val) = lead {
        if lead_val.is_empty() {
            return Err(JrError::UserError(
                "--lead \"\" has no effect on create \u{2014} there is no existing lead to clear. \
                 Omit --lead, or supply a name."
                    .into(),
            )
            .into());
        }
    }

    // BC-8.1.006: resolve lead via assignable-user search when --lead is supplied.
    let lead_account_id: Option<String> = if let Some(ref lead_query) = lead {
        let users = client
            .search_assignable_users_by_project(lead_query, &project)
            .await?;
        match users.len() {
            0 => {
                return Err(
                    JrError::UserError(format!("No user matching '{}'", lead_query)).into(),
                );
            }
            1 => Some(users.into_iter().next().unwrap().account_id),
            _ => {
                // BC-8.1.006 EC-8.1.006-1 / BC-X.7.004: list each candidate's
                // email + accountId (mirrors `issue assign --to` ambiguous path).
                let mut lines = format!("Ambiguous lead '{}'. Candidates:", lead_query);
                for u in &users {
                    let email = u.email_address.as_deref().unwrap_or("(no email)");
                    lines.push_str(&format!(
                        "\n  {} <{}> ({})",
                        u.display_name, email, u.account_id
                    ));
                }
                return Err(JrError::UserError(lines).into());
            }
        }
    } else {
        None
    };

    // BC-8.1.005 / VP-COMPONENT-022: build body omitting absent optional keys.
    let mut body = serde_json::Map::new();
    body.insert("name".to_string(), serde_json::Value::String(name.clone()));
    body.insert(
        "project".to_string(),
        serde_json::Value::String(project.clone()),
    );
    if let Some(desc) = description {
        body.insert("description".to_string(), serde_json::Value::String(desc));
    }
    if let Some(account_id) = lead_account_id {
        body.insert(
            "leadAccountId".to_string(),
            serde_json::Value::String(account_id),
        );
    }
    if let Some(ref at) = assignee_type {
        body.insert(
            "assigneeType".to_string(),
            serde_json::Value::String(assignee_type_to_api_str(at).to_string()),
        );
    }

    let body_value = serde_json::Value::Object(body);
    let component = client.create_component(&body_value).await?;

    // ADR-0018 §2: invalidate components cache after successful mutation.
    cache::invalidate_components_cache(&config.active_profile_name, &project);

    // Symmetric output channel (profile 4): JSON → stdout, human → stderr.
    // BC-8.1.005: the confirmed project key — prefer the API response field,
    // fall back to the --project argument used in the POST body.
    let project_key_display = component.project.as_deref().unwrap_or(&project).to_string();
    match output_format {
        OutputFormat::Json => {
            // F-04 / BC-8.1.005: emit exactly {"id","name","project"}.
            let json_out = serde_json::json!({
                "id": component.id,
                "name": component.name,
                "project": project_key_display,
            });
            println!("{}", output::render_json(&json_out)?);
        }
        OutputFormat::Table => {
            // F-05 / BC-8.1.005: canonical confirmation string.
            eprintln!(
                "Created component \"{}\" (id {}) in project {}.",
                component.name, component.id, project_key_display
            );
        }
    }

    Ok(())
}

/// Returns `true` when `s` is a non-empty all-ASCII-digit string (numeric
/// component ID path — BC-8.1.004 numeric-ID exemption).
fn is_numeric_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Handle `jr component edit NAME_OR_ID [options]`.
///
/// BC-8.1.007 (edit PUT), BC-8.1.008 (not-found), BC-8.4.001 (name resolver), BC-8.1.004 (numeric-id exemption) (S-604-2).
/// Resolves the component by name (via project component list + partial_match)
/// or by numeric ID (via confirming GET), PUTs `/rest/api/3/component/{id}`,
/// then invalidates the project's component cache entry (ADR-0018 §2).
async fn handle_edit(
    args: EditComponentArgs,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let EditComponentArgs {
        name_or_id,
        project,
        new_name,
        description,
        lead,
    } = args;

    // BC-8.1.007 P16: no-fields guard fires BEFORE any HTTP (for both name
    // and numeric paths).
    let has_fields = new_name.is_some() || description.is_some() || lead.is_some();
    if !has_fields {
        return Err(JrError::UserError(
            "no fields specified to update. Supply --name, --description, or --lead.".into(),
        )
        .into());
    }

    // Determine whether input is a numeric ID or a component name.
    let (component_id, project_key) = if is_numeric_id(&name_or_id) {
        // BC-8.1.004: numeric ID exempts from the no-project guard.
        // ADR-0018 §1: ONE confirming GET derives both existence and project.
        let comp = match client.get_component(&name_or_id).await {
            Ok(c) => c,
            Err(e) => {
                let is_404 = e
                    .downcast_ref::<JrError>()
                    .map(|je| matches!(je, JrError::ApiError { status: 404, .. }))
                    .unwrap_or(false);
                if is_404 {
                    // F-06 / BC-8.1.008: check effective project (flag > config)
                    // for variant selection — NOT just the --project flag.
                    let effective_project = config.project_key(project.as_deref());
                    let msg = match effective_project {
                        Some(p) => format!(
                            "Component '{}' not found in project {}. Run: jr component list",
                            name_or_id, p
                        ),
                        None => format!(
                            "Component '{}' not found. \
                             Run: jr component list --project <KEY> to see valid components.",
                            name_or_id
                        ),
                    };
                    return Err(JrError::UserError(msg).into());
                }
                return Err(e);
            }
        };

        // F-07 / BC-8.1.007: fail-closed if the confirming GET returned no project
        // field, regardless of whether --project was supplied.
        // FIX 4 (PR#704 Finding C): adopting a user-supplied --project value
        // when the confirming GET returned no project field is unsafe — we
        // cannot verify the component's actual project scope. Exit 64 in both
        // cases (--project supplied or not).
        let derived_project = comp.project.clone().unwrap_or_default();
        let final_project_key: String = if !derived_project.is_empty() {
            derived_project.clone()
        } else if project.is_some() {
            return Err(JrError::UserError(format!(
                "Component {} returned no project field; cannot verify --project \
                 or scope the update. The component's project could not be determined.",
                name_or_id
            ))
            .into());
        } else {
            return Err(JrError::UserError(format!(
                "Component {} exists but Jira returned no project field. \
                 Pass --project KEY to disambiguate.",
                name_or_id
            ))
            .into());
        };
        // If --project was supplied, verify it matches the derived project.
        // (Only reached when derived_project is non-empty per the guard above.)
        if let Some(ref user_project) = project {
            if !user_project.eq_ignore_ascii_case(&derived_project) {
                return Err(JrError::UserError(format!(
                    "Component {} belongs to project {}, not {}.",
                    name_or_id, derived_project, user_project
                ))
                .into());
            }
        }

        (comp.id.clone(), final_project_key)
    } else {
        // Name-based: project is required (BC-8.1.004 — no exemption for names).
        let pk = config.project_key(project.as_deref()).ok_or_else(|| {
            JrError::UserError(
                "No project configured. Pass --project KEY or set \
                 project = \"...\" in .jr.toml."
                    .into(),
            )
        })?;

        let components = client.list_components(&pk).await?;
        let candidate_names: Vec<String> = components.iter().map(|c| c.name.clone()).collect();

        let matched_name = match resolve_component(&name_or_id, &pk, &candidate_names) {
            MatchResult::Exact(n) => n,
            MatchResult::ExactMultiple(matched_name) => {
                // BC-X.10.003 / hardened issue #288 H-3: picking the first of
                // duplicate-named components is non-deterministic and unsafe.
                // Fail closed with all matching IDs so the caller can use a
                // numeric ID to disambiguate.
                let ids: Vec<String> = components
                    .iter()
                    .filter(|c| c.name.to_lowercase() == matched_name.to_lowercase())
                    .map(|c| c.id.clone())
                    .collect();
                return Err(JrError::UserError(format!(
                    "Multiple components named \"{}\" found (IDs: {}). \
                     Pass the numeric ID directly.",
                    matched_name,
                    ids.join(", ")
                ))
                .into());
            }
            MatchResult::Ambiguous(mut candidates) => {
                // BC-8.4.003: Matches list must be alphabetically sorted (case-insensitive)
                // and terminated with a period.
                candidates.sort_by_key(|s| s.to_lowercase());
                return Err(JrError::UserError(format!(
                    "Ambiguous component '{}'. Matches: {}.",
                    name_or_id,
                    candidates.join(", ")
                ))
                .into());
            }
            MatchResult::None(mut available) => {
                // BC-8.4.002: Available list must be alphabetically sorted (case-insensitive)
                // and terminated with a period.
                available.sort_by_key(|s| s.to_lowercase());
                return Err(JrError::UserError(format!(
                    "Component '{}' not found in project {}. Available: {}.",
                    name_or_id,
                    pk,
                    available.join(", ")
                ))
                .into());
            }
        };

        // Find the matching component to get its ID.
        let comp = components
            .into_iter()
            .find(|c| c.name == matched_name)
            .ok_or_else(|| {
                JrError::Internal(format!(
                    "Internal error: resolved component name '{}' not found in list.",
                    matched_name
                ))
            })?;

        (comp.id, pk)
    };

    // Save field values for BC-3.4.012 field echo before they are moved/consumed
    // into the PUT body map below (F-05 edit).
    let echo_name = new_name.clone();
    let echo_desc = description.clone();
    let echo_lead = lead.clone();

    // Build partial PUT body — only supplied fields (VP-COMPONENT-023).
    let mut body = serde_json::Map::new();
    if let Some(n) = new_name {
        body.insert("name".to_string(), serde_json::Value::String(n));
    }
    if let Some(desc) = description {
        body.insert("description".to_string(), serde_json::Value::String(desc));
    }
    if let Some(ref lead_val) = lead {
        if lead_val.is_empty() {
            // BC-8.1.007: --lead "" → explicit clear → null
            body.insert("leadAccountId".to_string(), serde_json::Value::Null);
        } else {
            // BC-8.1.006: resolve lead via assignable-user search.
            let users = client
                .search_assignable_users_by_project(lead_val, &project_key)
                .await?;
            match users.len() {
                0 => {
                    return Err(
                        JrError::UserError(format!("No user matching '{}'", lead_val)).into(),
                    );
                }
                1 => {
                    body.insert(
                        "leadAccountId".to_string(),
                        serde_json::Value::String(users.into_iter().next().unwrap().account_id),
                    );
                }
                _ => {
                    // BC-8.1.006 / BC-X.7.004: list each candidate's email + accountId.
                    let mut lines = format!("Ambiguous lead '{}'. Candidates:", lead_val);
                    for u in &users {
                        let email = u.email_address.as_deref().unwrap_or("(no email)");
                        lines.push_str(&format!(
                            "\n  {} <{}> ({})",
                            u.display_name, email, u.account_id
                        ));
                    }
                    return Err(JrError::UserError(lines).into());
                }
            }
        }
    }

    let body_value = serde_json::Value::Object(body);

    // PUT — BC-8.1.007 AC-016: 404 on PUT (race condition) is ApiError (exit 1),
    // not UserError (exit 64).  Let the error propagate as-is.
    // BC-8.1.007: Jira's PUT /rest/api/3/component/{id} returns the updated
    // component body — capture it so the JSON output path can use it (F-01).
    let updated = client.edit_component(&component_id, &body_value).await?;

    // ADR-0018 §2: invalidate components cache after successful mutation.
    cache::invalidate_components_cache(&config.active_profile_name, &project_key);

    // Symmetric output channel (profile 4): JSON → stdout, human → stderr.
    match output_format {
        OutputFormat::Json => {
            // F-01 / BC-8.1.007: emit exactly {"id","name","project"} — same
            // shape as create (BC-8.1.005).
            let proj = updated.project.as_deref().unwrap_or(&project_key);
            let json_out = serde_json::json!({
                "id": updated.id,
                "name": updated.name,
                "project": proj,
            });
            println!("{}", output::render_json(&json_out)?);
        }
        OutputFormat::Table => {
            // F-05 / BC-8.1.007: confirmation header matching `create` profile (symmetric).
            eprintln!(
                "Updated component \"{}\" (id {}) in project {}.",
                updated.name,
                updated.id,
                updated.project.as_deref().unwrap_or(&project_key)
            );
            // BC-3.4.012: one "  field → value" line per changed field.
            if let Some(n) = echo_name {
                eprintln!("  name \u{2192} {}", n);
            }
            if let Some(d) = echo_desc {
                eprintln!("  description \u{2192} {}", d);
            }
            if let Some(l) = echo_lead {
                if l.is_empty() {
                    eprintln!("  lead \u{2192} (cleared)");
                } else {
                    eprintln!("  lead \u{2192} {}", l);
                }
            }
        }
    }

    Ok(())
}
