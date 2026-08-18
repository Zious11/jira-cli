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
    no_input: bool,
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
            // F5-A-L2: `--project` supplied locally (subcommand position)
            // takes precedence; falls back to the global `--project` flag.
            // NO `.jr.toml` config fallback (BC-8.1.004/BC-8.1.005) —
            // `handle_create` enforces presence itself and exits 64 if
            // neither position supplied a value.
            let project = project.or_else(|| project_flag.map(str::to_string));
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
        ComponentSubcommand::Delete {
            name_or_id,
            project,
            move_to,
            orphan,
            yes,
        } => {
            handle_delete(
                DeleteComponentArgs {
                    name_or_id,
                    project,
                    move_to,
                    orphan,
                    yes,
                },
                output_format,
                config,
                client,
                no_input,
            )
            .await
        }
        ComponentSubcommand::Rename {
            old,
            new,
            project,
            all_projects,
            dry_run,
        } => {
            handle_rename(
                RenameComponentArgs {
                    old,
                    new,
                    project,
                    all_projects,
                    dry_run,
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
    project: Option<String>,
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

    // F5-A-L2 / BC-8.1.004 + BC-8.1.005: --project is required for create with
    // NO `.jr.toml` config fallback — it must come from either the local
    // `--project` flag or the global `--project` flag (merged by the caller).
    // Exit 64 before any HTTP call, mirroring BC-8.1.004's other no-project
    // guards.
    let project = project.ok_or_else(|| {
        JrError::UserError(
            "No project configured. Pass --project KEY (component create has no \
             .jr.toml config fallback)."
                .into(),
        )
    })?;

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

/// Caller-supplied arguments for `handle_delete`.
///
/// Bundles the command-specific parameters so `handle_delete` stays within
/// clippy's 7-argument limit (same pattern as `CreateComponentArgs` /
/// `EditComponentArgs`).
struct DeleteComponentArgs {
    name_or_id: String,
    project: Option<String>,
    move_to: Option<String>,
    orphan: bool,
    yes: bool,
}

/// Returns `true` when `e` downcasts to `JrError::ApiError { status: 404, .. }`.
///
/// Shared by the source and `--move-to` target numeric confirming-GET paths
/// (ADR-0018 §1) — both treat a 404 on that GET as an ordinary resolver-layer
/// not-found, never a race.
fn is_404_error(e: &anyhow::Error) -> bool {
    e.downcast_ref::<JrError>()
        .map(|je| matches!(je, JrError::ApiError { status: 404, .. }))
        .unwrap_or(false)
}

/// BC-8.2.001 Postcondition 3 / DEC-188: the neither-`--move-to`-nor-`--orphan`
/// exit-64 guard. Application-level check (never a clap `ArgGroup::required`,
/// which would wrongly produce exit 2) — names BOTH flags, no affected-issue
/// count (the snapshot never fires in this path per BC-8.2.007 Postcondition 1).
fn disposition_guard_error() -> anyhow::Error {
    JrError::UserError(
        "Refusing to delete: no disposition supplied for this component's issues. \
         Supply --move-to <NAME|ID> to move them to another component, or --orphan \
         to remove the component with no replacement."
            .into(),
    )
    .into()
}

/// BC-8.2.002/BC-8.2.003/BC-8.2.004: a numeric `--move-to` target that either
/// 404s on its confirming GET or resolves to a different project than the
/// source — treated identically to "no match in scope" (zero `DELETE`).
fn move_to_not_found_in_project(target_input: &str, project_key: &str) -> anyhow::Error {
    JrError::UserError(format!(
        "--move-to target '{}' not found in project {}.",
        target_input, project_key
    ))
    .into()
}

/// Handle `jr component delete NAME_OR_ID [--project KEY] (--move-to
/// NAME_OR_ID | --orphan) [--yes]` (S-604-3).
///
/// BC-8.2.001 — BC-8.2.008: disposition-required guard, `--move-to`
/// resolution (source-project-scoped, numeric-target confirming GET,
/// self-move guard), numeric-source project confirmation (both
/// dispositions), the `--orphan` confirmation gate, the BC-8.2.007
/// pre-delete JQL snapshot (fully paginated, fail-closed on drift/error —
/// reuses the `search_issue_keys`-style pagination loop from
/// `api/jira/issues.rs`, NOT reimplemented here), and the BC-8.2.008
/// `--output json` result shape / not-found-vs-race idempotency taxonomy.
///
/// Ordering (Forbidden Dependencies, story S-604-3): the snapshot search
/// MUST complete successfully before `delete_component`'s DELETE fires.
///
/// `clap`'s `conflicts_with` on `ComponentSubcommand::Delete` already
/// enforces the both-flags-supplied case (exit 2) before this handler ever
/// runs; the neither-flag case (BC-8.2.001 Postcondition 3, DEC-188) is this
/// handler's own application-level `JrError::UserError` guard (exit 64).
async fn handle_delete(
    args: DeleteComponentArgs,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let DeleteComponentArgs {
        name_or_id,
        project,
        move_to,
        orphan,
        yes,
    } = args;

    let has_disposition = move_to.is_some() || orphan;

    // Resolve SOURCE. Invariant 1 (BC-8.2.001): for a NAME source, resolution
    // fires BEFORE the disposition guard — an unresolvable name reports
    // "not found", never the disposition-guard message. For a NUMERIC
    // source, no HTTP call is available before a disposition is chosen (the
    // numeric-source confirming GET only fires once a disposition is
    // supplied), so the disposition guard fires FIRST — the documented
    // asymmetry (EC-8.2.001-4).
    let (component_id, project_key, component_name): (String, String, String) =
        if !is_numeric_id(&name_or_id) {
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
                    candidates.sort_by_key(|s| s.to_lowercase());
                    return Err(JrError::UserError(format!(
                        "Ambiguous component '{}'. Matches: {}.",
                        name_or_id,
                        candidates.join(", ")
                    ))
                    .into());
                }
                MatchResult::None(mut available) => {
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

            let comp = components
                .into_iter()
                .find(|c| c.name == matched_name)
                .ok_or_else(|| {
                    JrError::Internal(format!(
                        "Internal error: resolved component name '{}' not found in list.",
                        matched_name
                    ))
                })?;

            // Invariant 1: resolution succeeded — the disposition guard is
            // checked only now, AFTER the not-found/ambiguous checks above.
            if !has_disposition {
                return Err(disposition_guard_error());
            }

            (comp.id, pk, comp.name)
        } else {
            // Numeric source: disposition guard fires FIRST — zero HTTP is
            // reachable in this path to discover non-existence otherwise.
            if !has_disposition {
                return Err(disposition_guard_error());
            }

            // BC-8.2.002 M1 (P4-broadened to both dispositions): the SAME
            // single-object confirming GET BC-8.1.008's numeric bypass
            // already requires for existence, now also read for `project`.
            let comp = match client.get_component(&name_or_id).await {
                Ok(c) => c,
                Err(e) => {
                    if is_404_error(&e) {
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

            let derived_project = comp.project.clone().unwrap_or_default();
            let final_project_key: String = if !derived_project.is_empty() {
                derived_project.clone()
            } else if project.is_some() {
                return Err(JrError::UserError(format!(
                    "Component {} returned no project field; cannot verify --project \
                     or scope the delete. The component's project could not be determined.",
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

            if let Some(ref user_project) = project {
                if !user_project.eq_ignore_ascii_case(&derived_project) {
                    return Err(JrError::UserError(format!(
                        "Component {} belongs to project {}, not {}.",
                        name_or_id, derived_project, user_project
                    ))
                    .into());
                }
            }

            (comp.id, final_project_key, comp.name)
        };

    // Resolve TARGET (--move-to) — scoped EXCLUSIVELY to the source's project
    // (BC-8.2.003), completing BEFORE any DELETE call (BC-8.2.002).
    let target_id: Option<String> = if let Some(ref mv) = move_to {
        if is_numeric_id(mv) {
            // Numeric target: confirming GET validates its project matches
            // the source's (BC-8.2.002 M2) — a mismatch or 404 is treated
            // identically to "no match in scope" (BC-8.2.004).
            let target = match client.get_component(mv).await {
                Ok(c) => c,
                Err(e) => {
                    if is_404_error(&e) {
                        return Err(move_to_not_found_in_project(mv, &project_key));
                    }
                    return Err(e);
                }
            };
            let target_project = target.project.clone().unwrap_or_default();
            if !target_project.eq_ignore_ascii_case(&project_key) {
                return Err(move_to_not_found_in_project(mv, &project_key));
            }
            Some(target.id)
        } else {
            let components = client.list_components(&project_key).await?;
            let candidate_names: Vec<String> = components.iter().map(|c| c.name.clone()).collect();

            let matched_name = match resolve_component(mv, &project_key, &candidate_names) {
                MatchResult::Exact(n) => n,
                MatchResult::ExactMultiple(matched_name) => {
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
                    candidates.sort_by_key(|s| s.to_lowercase());
                    return Err(JrError::UserError(format!(
                        "Ambiguous component '{}'. Matches: {}.",
                        mv,
                        candidates.join(", ")
                    ))
                    .into());
                }
                MatchResult::None(mut available) => {
                    available.sort_by_key(|s| s.to_lowercase());
                    return Err(JrError::UserError(format!(
                        "Component '{}' not found in project {}. Available: {}.",
                        mv,
                        project_key,
                        available.join(", ")
                    ))
                    .into());
                }
            };

            let comp = components
                .into_iter()
                .find(|c| c.name == matched_name)
                .ok_or_else(|| {
                    JrError::Internal(format!(
                        "Internal error: resolved component name '{}' not found in list.",
                        matched_name
                    ))
                })?;
            Some(comp.id)
        }
    } else {
        None
    };

    // BC-8.2.005: self-move guard — ID equality (not name-string equality),
    // fires BEFORE the snapshot and the DELETE.
    if let Some(ref tid) = target_id {
        if *tid == component_id {
            return Err(JrError::UserError(
                "--move-to target is the same component being deleted. \
                 Choose a different component, or use --orphan."
                    .into(),
            )
            .into());
        }
    }

    // BC-8.2.007: pre-delete JQL snapshot — the resolved NUMERIC id, never
    // the name, fully paginated via the reused `search_issue_keys` loop.
    // Fail-closed on ANY non-normal-completion outcome, including the
    // JRACLOUD-95368 anti-loop guard's successful `has_more=true` partial
    // return (a genuine 5xx/network fetch error already fails closed via
    // `?` below).
    let jql = format!("component = {} ORDER BY key ASC", component_id);
    let snapshot = client.search_issue_keys(&jql, None).await?;
    if snapshot.has_more {
        return Err(JrError::SnapshotIncomplete(
            "could not reliably enumerate affected issues — aborting delete".into(),
        )
        .into());
    }
    let affected_issues = snapshot.keys;
    let affected_count = affected_issues.len();

    // BC-8.2.006: --orphan confirmation gate. --move-to NEVER prompts or
    // requires --yes (Invariant 1) — this block is skipped entirely then.
    if orphan && !yes {
        if no_input {
            return Err(JrError::UserError(format!(
                "--orphan requires --yes when running non-interactively. This permanently \
                 removes the component from {} issue(s) with no replacement.",
                affected_count
            ))
            .into());
        }

        // Direct stdin read (not `dialoguer::Confirm::interact_on`) — mirrors
        // `handle_comment_delete`'s DEC-174 rationale: console's `is_term()`
        // gate returns `NotConnected` on piped stderr, as in every subprocess
        // test here. Prompt → stderr; y/N response from stdin; EOF → Interrupted.
        use std::io::BufRead;
        eprint!(
            "Delete component '{}' and remove it from {} issue(s)? This cannot be undone. [y/N] ",
            component_name, affected_count
        );
        let _ = std::io::Write::flush(&mut std::io::stderr());

        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => return Err(JrError::Interrupted.into()),
            Ok(_) => {
                let answer = line.trim().to_ascii_lowercase();
                if answer != "y" && answer != "yes" {
                    // Declined (or Enter for the default N) — not itself an
                    // error, exit 0, zero DELETE.
                    return Ok(());
                }
            }
        }
    }

    // BC-8.2.002/BC-8.2.006: the DELETE — `moveIssuesTo` when --move-to was
    // chosen, absent under --orphan. A 404 here (source or target deleted by
    // a concurrent actor AFTER successful resolution) propagates as
    // `JrError::ApiError` — exit 1 — DISTINCT from the resolver-layer
    // not-found paths above (BC-8.2.008 / VP-COMPONENT-024).
    client
        .delete_component(&component_id, target_id.as_deref())
        .await?;

    // ADR-0018 §2: invalidate components cache after successful mutation.
    cache::invalidate_components_cache(&config.active_profile_name, &project_key);

    // Symmetric output channel (profile 4): JSON → stdout, human → stderr.
    match output_format {
        OutputFormat::Json => {
            // BC-8.2.008: exactly {"deleted","movedIssuesTo","affectedIssueCount","affectedIssues"}.
            let json_out = serde_json::json!({
                "deleted": component_id,
                "movedIssuesTo": target_id,
                "affectedIssueCount": affected_count,
                "affectedIssues": affected_issues,
            });
            println!("{}", output::render_json(&json_out)?);
        }
        OutputFormat::Table => {
            let disposition_desc = match &target_id {
                Some(tid) => format!("moved to component {}", tid),
                None => "orphaned (no replacement)".to_string(),
            };
            eprintln!(
                "Deleted component \"{}\" (id {}) \u{2014} {} affected issue(s), {}.",
                component_name, component_id, affected_count, disposition_desc
            );
        }
    }

    Ok(())
}

/// Caller-supplied arguments for `handle_rename`.
///
/// Bundles the command-specific parameters so `handle_rename` stays within
/// clippy's 7-argument limit (same pattern as `CreateComponentArgs` /
/// `EditComponentArgs` / `DeleteComponentArgs`).
struct RenameComponentArgs {
    /// Current component name (single-project form: partial-matched via
    /// §8.4; `--all-projects` form: exact case-insensitive equality per
    /// BC-8.3.002) or numeric ID.
    old: String,
    /// New component name.
    new: String,
    /// Single-project scope selector. `--project` and `--all-projects` are
    /// clap `conflicts_with`-paired (BC-8.3.005) — at most one of
    /// `project`/`all_projects` is meaningfully populated at a time, but
    /// both are threaded through so `handle_rename` can apply the
    /// application-level neither-supplied exit-64 guard itself.
    project: Option<String>,
    /// `--all-projects` fan-out scope selector (BC-8.3.002).
    all_projects: bool,
    /// `--dry-run` preview mode — zero mutating HTTP (BC-8.3.004).
    dry_run: bool,
}

/// Handle `jr component rename OLD NEW (--project KEY | --all-projects)
/// [--dry-run]` (S-608-1).
///
/// BC-8.3.001 (single-project PUT + numeric-`OLD` project confirmation M1),
/// BC-8.3.002 (`--all-projects` exact-equality fan-out discovery + numeric-
/// `OLD` pre-flight rejection), BC-8.3.003 (per-project continue-on-error
/// atomicity, exit code reflects any failure), BC-8.3.004 (`--dry-run`
/// zero-mutation preview with identical discovery scope to the live run),
/// BC-8.3.005 (scope-selection guard — clap `conflicts_with` for
/// both-supplied, application-level `JrError::UserError` for
/// neither-supplied), BC-8.3.006 (case-only rename is never short-circuited
/// in either scope), BC-8.3.007 (`NEW` name-collision surfaced verbatim, not
/// pre-validated).
///
/// Single-project form resolves `OLD` scoped to `--project` (name via §8.4
/// `resolve_component`/`partial_match`, numeric via the ADR-0018 §1
/// confirming-GET reused from `handle_edit`/`handle_delete`) then calls
/// `JiraClient::rename_component` once. `--all-projects` iterates
/// `JiraClient::list_projects` (paginated, reused as-is — no pagination
/// logic is reimplemented here) and, per accessible project, lists that
/// project's components looking for an EXACT case-insensitive name match
/// (deliberately NOT `resolve_component`'s substring semantics — BC-8.3.002),
/// calling `rename_component` once per match with continue-on-error
/// semantics (BC-8.3.003). `--dry-run` performs the identical discovery
/// (single target resolution, or the full per-project fan-out loop) but
/// issues zero calls to `rename_component`. On success, invalidates the
/// affected project(s)' components cache entries (ADR-0018 §2) — for
/// `--all-projects`, only for projects that actually renamed.
async fn handle_rename(
    args: RenameComponentArgs,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let RenameComponentArgs {
        old,
        new,
        project,
        all_projects,
        dry_run,
    } = args;

    // BC-8.3.005 Postcondition: both-supplied is already a clap
    // `conflicts_with` exit-2 rejection (never reaches here). Neither
    // supplied is this application-level exit-64 guard — DEC-188, NOT a
    // clap `ArgGroup::required(true)` (which would wrongly exit 2).
    if project.is_none() && !all_projects {
        return Err(JrError::UserError(
            "rename requires either --project <KEY> or --all-projects — supply exactly one.".into(),
        )
        .into());
    }

    if all_projects {
        handle_rename_all_projects(&old, &new, dry_run, output_format, config, client).await
    } else {
        // Precondition 1 guarantees `project` is `Some` here (the neither-
        // guard above already excluded the `None` case for this branch).
        let project_key = project.expect("project.is_some() checked above");
        handle_rename_single_project(
            &old,
            &new,
            &project_key,
            dry_run,
            output_format,
            config,
            client,
        )
        .await
    }
}

/// Resolve `old` to a component id scoped to `project_key` for the
/// single-project form of `rename` (BC-8.3.001).
///
/// Numeric `old`: fires the ADR-0018 §1 confirming `GET` (BC-8.3.001 M1),
/// comparing the confirmed `project` field against the REQUIRED
/// `project_key` — a mismatch exits 64 pre-flight (EC-8.3.001-1), ZERO `PUT`
/// calls. A 404 on the confirming `GET` is the ordinary not-found path,
/// ALWAYS the project-qualified message variant (EC-8.3.001-2) — `--project`
/// is Precondition 1's unconditional requirement, so it is always known here.
///
/// Name `old`: uses the §8.4 resolver (`resolve_component`/`partial_match`)
/// scoped to `project_key` — structurally identical to `handle_edit`'s
/// name-based branch (S-604-2 precedent, reused not duplicated).
async fn resolve_rename_source(
    old: &str,
    project_key: &str,
    client: &JiraClient,
) -> Result<String> {
    if is_numeric_id(old) {
        let comp = match client.get_component(old).await {
            Ok(c) => c,
            Err(e) => {
                if is_404_error(&e) {
                    return Err(JrError::UserError(format!(
                        "Component '{}' not found in project {}. Run: jr component list",
                        old, project_key
                    ))
                    .into());
                }
                return Err(e);
            }
        };
        let derived_project = comp.project.clone().unwrap_or_default();
        if !derived_project.eq_ignore_ascii_case(project_key) {
            return Err(JrError::UserError(format!(
                "Component {} belongs to project {}, not {}.",
                old, derived_project, project_key
            ))
            .into());
        }
        Ok(comp.id)
    } else {
        let components = client.list_components(project_key).await?;
        let candidate_names: Vec<String> = components.iter().map(|c| c.name.clone()).collect();

        let matched_name = match resolve_component(old, project_key, &candidate_names) {
            MatchResult::Exact(n) => n,
            MatchResult::ExactMultiple(matched_name) => {
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
                candidates.sort_by_key(|s| s.to_lowercase());
                return Err(JrError::UserError(format!(
                    "Ambiguous component '{}'. Matches: {}.",
                    old,
                    candidates.join(", ")
                ))
                .into());
            }
            MatchResult::None(mut available) => {
                available.sort_by_key(|s| s.to_lowercase());
                return Err(JrError::UserError(format!(
                    "Component '{}' not found in project {}. Available: {}.",
                    old,
                    project_key,
                    available.join(", ")
                ))
                .into());
            }
        };

        let comp = components
            .into_iter()
            .find(|c| c.name == matched_name)
            .ok_or_else(|| {
                JrError::Internal(format!(
                    "Internal error: resolved component name '{}' not found in list.",
                    matched_name
                ))
            })?;

        Ok(comp.id)
    }
}

/// Single-project form of `rename` (BC-8.3.001 — including M1's numeric
/// confirming-GET; BC-8.3.004's `--dry-run` short-circuit; BC-8.3.006's
/// case-only-rename non-short-circuit — the PUT always fires once resolved,
/// regardless of `old.eq_ignore_ascii_case(new)`; BC-8.3.007's verbatim
/// collision passthrough — no client-side pre-check for `new`).
async fn handle_rename_single_project(
    old: &str,
    new: &str,
    project_key: &str,
    dry_run: bool,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let component_id = resolve_rename_source(old, project_key, client).await?;

    if dry_run {
        match output_format {
            OutputFormat::Json => {
                let json_out = serde_json::json!({
                    "dryRun": true,
                    "targets": [
                        {"project": project_key, "id": component_id, "from": old, "to": new}
                    ]
                });
                println!("{}", output::render_json(&json_out)?);
            }
            OutputFormat::Table => {
                eprintln!("DRY RUN — no changes will be made.");
                eprintln!(
                    "  {}: {} \u{2192} {} (id {})",
                    project_key, old, new, component_id
                );
            }
        }
        return Ok(());
    }

    // BC-8.3.007: no pre-flight existence check for `new` — the server
    // validates authoritatively; a 400 collision propagates verbatim.
    // BC-8.3.001 Idempotency: a 404 here (component deleted concurrently
    // after a successful resolution) propagates as `JrError::ApiError`
    // (exit 1), distinct from `resolve_rename_source`'s exit-64 not-found.
    let updated = client.rename_component(&component_id, new).await?;

    // ADR-0018 §2: invalidate the project's components cache after a
    // successful mutation.
    cache::invalidate_components_cache(&config.active_profile_name, project_key);

    match output_format {
        OutputFormat::Json => {
            // BC-8.3.001 Postcondition 2: exact JSON shape.
            let json_out = serde_json::json!({
                "renamed": {
                    "id": updated.id,
                    "from": old,
                    "to": new,
                    "project": project_key,
                }
            });
            println!("{}", output::render_json(&json_out)?);
        }
        OutputFormat::Table => {
            eprintln!(
                "Renamed component \"{}\" to \"{}\" (id {}) in project {}.",
                old, new, updated.id, project_key
            );
        }
    }

    Ok(())
}

/// Verbatim rejection message for a numeric `OLD` under `--all-projects`
/// (BC-8.3.002 Precondition 2) — a numeric component id is inherently
/// single-project-scoped and cannot select across multiple projects.
const ALL_PROJECTS_NUMERIC_OLD_REJECTED_MSG: &str = "rename --all-projects requires OLD to be a component NAME, not a numeric id (component ids are project-scoped and cannot be used to select across multiple projects). Use rename OLD NEW --project KEY to target a single project by id.";

/// One project's discovered rename target under `--all-projects`
/// (BC-8.3.002 Postcondition 3).
struct RenameTarget {
    project: String,
    id: String,
}

/// `--all-projects` fan-out discovery (BC-8.3.002): iterates
/// `list_projects` (paginated, reused as-is) and, per accessible project,
/// looks for a component whose name EXACTLY case-insensitively equals `old`
/// — deliberately NOT `resolve_component`/`partial_match`'s substring
/// semantics (BC-8.3.002 Matching-semantics divergence). Shared by both the
/// live and `--dry-run` paths so discovery scope is IDENTICAL between them
/// (BC-8.3.004 Invariant 1).
async fn discover_rename_targets(old: &str, client: &JiraClient) -> Result<Vec<RenameTarget>> {
    let projects = client.list_projects(None, None).await?;
    let mut targets = Vec::new();
    for p in &projects {
        let components = client.list_components(&p.key).await?;
        if let Some(comp) = components
            .iter()
            .find(|c| c.name.to_lowercase() == old.to_lowercase())
        {
            targets.push(RenameTarget {
                project: p.key.clone(),
                id: comp.id.clone(),
            });
        }
    }
    Ok(targets)
}

/// `--all-projects` form of `rename` (BC-8.3.002 discovery + numeric-`OLD`
/// pre-flight rejection, BC-8.3.003 per-project continue-on-error atomicity,
/// BC-8.3.004 `--dry-run` zero-mutation preview, BC-8.3.006 case-only-rename
/// non-short-circuit — every discovered target's PUT always fires).
async fn handle_rename_all_projects(
    old: &str,
    new: &str,
    dry_run: bool,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    // BC-8.3.002 Precondition 2 / EC-8.3.002-2 / EC-8.3.004-2: a numeric OLD
    // is rejected pre-flight, BEFORE list_projects — zero HTTP of any kind,
    // in both the live and --dry-run forms (mutually exclusive with the
    // dry-run preview for this input).
    if is_numeric_id(old) {
        return Err(JrError::UserError(ALL_PROJECTS_NUMERIC_OLD_REJECTED_MSG.into()).into());
    }

    let targets = discover_rename_targets(old, client).await?;

    if dry_run {
        match output_format {
            OutputFormat::Json => {
                let targets_json: Vec<serde_json::Value> = targets
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "project": t.project,
                            "id": t.id,
                            "from": old,
                            "to": new,
                        })
                    })
                    .collect();
                let json_out = serde_json::json!({"dryRun": true, "targets": targets_json});
                println!("{}", output::render_json(&json_out)?);
            }
            OutputFormat::Table => {
                eprintln!("DRY RUN — no changes will be made.");
                if targets.is_empty() {
                    eprintln!("0 components would be renamed.");
                } else {
                    for t in &targets {
                        eprintln!("  {}: {} \u{2192} {} (id {})", t.project, old, new, t.id);
                    }
                }
            }
        }
        return Ok(());
    }

    // BC-8.3.003: per-project atomic, continue-on-error, no rollback.
    let mut renamed: Vec<serde_json::Value> = Vec::new();
    let mut failed: Vec<serde_json::Value> = Vec::new();
    let mut table_lines: Vec<String> = Vec::new();

    for t in &targets {
        match client.rename_component(&t.id, new).await {
            Ok(updated) => {
                cache::invalidate_components_cache(&config.active_profile_name, &t.project);
                renamed.push(serde_json::json!({
                    "project": t.project,
                    "id": updated.id,
                    "status": "ok",
                }));
                table_lines.push(format!("{}: renamed", t.project));
            }
            Err(e) => {
                let error_text = e.to_string();
                failed.push(serde_json::json!({
                    "project": t.project,
                    "error": error_text,
                }));
                table_lines.push(format!("{}: FAILED — {}", t.project, error_text));
            }
        }
    }

    let any_failed = !failed.is_empty();
    let renamed_count = renamed.len();

    match output_format {
        OutputFormat::Json => {
            let json_out = serde_json::json!({"renamed": renamed, "failed": failed});
            println!("{}", output::render_json(&json_out)?);
        }
        OutputFormat::Table => {
            for line in &table_lines {
                eprintln!("{line}");
            }
            eprintln!("{renamed_count} renamed");
        }
    }

    // BC-8.3.003 Postcondition 2: exit 0 iff every attempted project
    // succeeded; exit 1 if ≥1 failed (partial success must not look
    // identical to full success to an automated caller).
    if any_failed {
        return Err(anyhow::anyhow!(
            "{} of {} project rename(s) failed",
            failed.len(),
            targets.len()
        ));
    }

    Ok(())
}
