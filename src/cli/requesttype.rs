//! `jr requesttype list/fields` discovery commands for JSM request types.
//!
//! Anchors BC-X.12.001..008. Uses `require_service_desk` from
//! `src/api/jsm/servicedesks.rs` (extended in this PR to take a call-site label
//! per BC-X.8.004), `partial_match` for name resolution, and `cache::*` for the
//! per-(profile, serviceDeskId) request-type cache (7d TTL).

use anyhow::Result;
use serde_json;

use crate::api::client::JiraClient;
use crate::api::jsm::servicedesks;
use crate::cache;
use crate::cli::{OutputFormat, RequestTypeCommand};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};
use crate::types::jsm::RequestType;

pub async fn handle(
    command: RequestTypeCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
) -> Result<()> {
    let project_key = config.project_key(project_override).ok_or_else(|| {
        JrError::UserError(
            "No project configured. Run \"jr init\" or pass --project. \
             Run \"jr project list\" to see available projects."
                .into(),
        )
    })?;

    let service_desk_id = servicedesks::require_service_desk(
        client,
        &project_key,
        "`jr requesttype` commands require",
    )
    .await?;

    let profile = &config.active_profile_name;

    match command {
        RequestTypeCommand::List { search } => {
            handle_list(
                &service_desk_id,
                search.as_deref(),
                profile,
                output_format,
                client,
            )
            .await
        }
        RequestTypeCommand::Fields { name_or_id } => {
            handle_fields(
                &service_desk_id,
                &name_or_id,
                &project_key,
                profile,
                output_format,
                client,
            )
            .await
        }
    }
}

async fn handle_list(
    service_desk_id: &str,
    search: Option<&str>,
    profile: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    // Normalize Some("") to None — empty-string search would forward
    // `?searchQuery=` with no value to the API and bypass cache for no reason.
    let search = search.filter(|s| !s.is_empty());

    let types = if search.is_some() {
        // Always fetch when searching — search results are not cached.
        client.list_request_types(service_desk_id, search).await?
    } else {
        // Try cache first for the full list.
        match cache::read_request_type_cache(profile, service_desk_id)? {
            Some(cached) => cached,
            None => {
                let fetched = client.list_request_types(service_desk_id, None).await?;
                cache::write_request_type_cache(profile, service_desk_id, &fetched)?;
                fetched
            }
        }
    };

    let rows: Vec<Vec<String>> = types
        .iter()
        .map(|t| vec![t.name.clone(), t.description.clone().unwrap_or_default()])
        .collect();

    output::print_output(output_format, &["Name", "Description"], &rows, &types)
}

async fn handle_fields(
    service_desk_id: &str,
    name_or_id: &str,
    project_key: &str,
    profile: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    // Determine the request type ID.
    let request_type_id = if !name_or_id.is_empty()
        && name_or_id.chars().all(|c| c.is_ascii_digit())
    {
        // All digits — treat as a numeric ID directly.
        name_or_id.to_string()
    } else {
        // Name — resolve via partial_match.
        resolve_request_type_id(name_or_id, service_desk_id, project_key, profile, client).await?
    };

    // Try fields cache.
    let fields_response =
        match cache::read_request_type_fields_cache(profile, service_desk_id, &request_type_id)? {
            Some(cached) => cached,
            None => {
                let fetched = client
                    .get_request_type_fields(service_desk_id, &request_type_id)
                    .await?;
                cache::write_request_type_fields_cache(
                    profile,
                    service_desk_id,
                    &request_type_id,
                    &fetched,
                )?;
                fetched
            }
        };

    let rows: Vec<Vec<String>> = fields_response
        .request_type_fields
        .iter()
        .map(|f| {
            let type_label = f
                .jira_schema
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("—")
                .to_string();
            vec![
                f.name.clone(),
                if f.required { "YES" } else { "NO" }.to_string(),
                type_label,
            ]
        })
        .collect();

    // H-1: BC-X.12.007 mandates JSON keys { canRaiseOnBehalfOf, canAddRequestParticipants, fields }.
    // The `RequestTypeFieldsResponse` type serializes the fields array as `requestTypeFields`
    // (camelCase from the Atlassian API) — intentional at the type layer for API round-trip.
    // We shape the output here at the handler layer to match the BC-mandated public key names.
    match output_format {
        OutputFormat::Json => {
            let shaped = serde_json::json!({
                "canRaiseOnBehalfOf": fields_response.can_raise_on_behalf_of,
                "canAddRequestParticipants": fields_response.can_add_request_participants,
                "fields": fields_response.request_type_fields,
            });
            output::print_output(
                output_format,
                &["Field Name", "Required", "Type"],
                &rows,
                &shaped,
            )
        }
        OutputFormat::Table => output::print_output(
            output_format,
            &["Field Name", "Required", "Type"],
            &rows,
            &fields_response,
        ),
    }
}

/// Resolve a request type name to its ID using partial_match.
///
/// Loads the request type list (from cache if available, otherwise fetches).
/// Returns the ID on unambiguous match, or a UserError on ambiguity/no-match.
async fn resolve_request_type_id(
    name: &str,
    service_desk_id: &str,
    project_key: &str,
    profile: &str,
    client: &JiraClient,
) -> Result<String> {
    // Load request types (cache → fetch fallback).
    let types = match cache::read_request_type_cache(profile, service_desk_id)? {
        Some(cached) => cached,
        None => {
            let fetched = client.list_request_types(service_desk_id, None).await?;
            cache::write_request_type_cache(profile, service_desk_id, &fetched)?;
            fetched
        }
    };

    let names: Vec<String> = types.iter().map(|t| t.name.clone()).collect();

    match partial_match::partial_match(name, &names) {
        MatchResult::Exact(matched_name) => Ok(find_id_by_name(&matched_name, &types)),
        MatchResult::ExactMultiple(matched_name) => {
            // Multiple types with the exact same name — list IDs and suggest disambiguation.
            // Use case-insensitive comparison to mirror cli/queue.rs ExactMultiple handling.
            let matched_lower = matched_name.to_lowercase();
            let ids: Vec<String> = types
                .iter()
                .filter(|t| t.name.to_lowercase() == matched_lower)
                .map(|t| t.id.clone())
                .collect();
            Err(JrError::UserError(format!(
                "Multiple request types named \"{matched_name}\" found (IDs: {}). Pass the numeric ID directly.",
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
        MatchResult::None(_) => Err(JrError::UserError(format!(
            "Request type \"{name}\" not found. \
             Run `jr requesttype list --project {project_key}` to see all request types, \
             or delete the cache file at ~/.cache/jr/v1/{profile}/request_types_{service_desk_id}.json \
             if a recent admin change is suspected."
        ))
        .into()),
    }
}

fn find_id_by_name(name: &str, types: &[RequestType]) -> String {
    types
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.id.clone())
        .expect("matched name from partial_match::Exact must exist in types — caller invariant")
}
