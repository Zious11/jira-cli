use anyhow::Result;
use serde::Serialize;

use crate::api::client::JiraClient;
use crate::cli::OutputFormat;
use crate::cli::issue::resolve_component;
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::types::jira::component::ComponentLead;

/// Local view type used for `--counts --output json` output.
///
/// Emits `issueCount` (BC-8.1.003) instead of `relatedIssueCount`, and has
/// no `skip_serializing_if` on the optional fields so `null` is always
/// present in the JSON (BC-8.1.002).
#[derive(Serialize)]
struct ComponentCountJson<'a> {
    id: &'a str,
    name: &'a str,
    description: Option<&'a str>,
    lead: Option<&'a ComponentLead>,
    #[serde(rename = "assigneeType")]
    assign_type: Option<&'a str>,
    project: Option<&'a str>,
    /// `null` when enrichment failed for this component (fail-soft per BC-8.1.003).
    #[serde(rename = "issueCount")]
    issue_count: Option<u64>,
}

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
        // Build a JSON-specific view that serializes `issueCount` (BC-8.1.003) and
        // preserves null fields (BC-8.1.002) — distinct from Component which uses
        // `relatedIssueCount` and may have skip_serializing_if on some fields.
        let json_view: Vec<ComponentCountJson<'_>> = components
            .iter()
            .map(|c| ComponentCountJson {
                id: &c.id,
                name: &c.name,
                description: c.description.as_deref(),
                lead: c.lead.as_ref(),
                assign_type: c.assignee_type.as_deref(),
                project: c.project.as_deref(),
                issue_count: c.related_issue_count,
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

    // Suppress the unused-import lint; resolve_component is used on
    // `issue create/edit` paths dispatched by callers of this module.
    let _ = resolve_component;

    Ok(())
}
