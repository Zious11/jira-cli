use std::collections::HashMap;

use anyhow::Result;

use crate::api::assets::{objects, workspace};
use crate::api::client::JiraClient;
use crate::cache::CachedObjectTypeAttr;
use crate::cli::{AssetsCommand, OutputFormat};
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};
use crate::types::assets::{AssetAttribute, ConnectedTicket};

pub async fn handle(
    command: AssetsCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let workspace_id = workspace::get_or_fetch_workspace_id(client).await?;

    match command {
        AssetsCommand::Search {
            query,
            limit,
            attributes,
        } => {
            handle_search(
                &workspace_id,
                &query,
                limit,
                attributes,
                output_format,
                client,
            )
            .await
        }
        AssetsCommand::View { key, no_attributes } => {
            handle_view(&workspace_id, &key, no_attributes, output_format, client).await
        }
        AssetsCommand::Tickets {
            key,
            limit,
            open,
            status,
        } => {
            handle_tickets(
                &workspace_id,
                &key,
                limit,
                open,
                status,
                output_format,
                client,
            )
            .await
        }
        AssetsCommand::Schemas => handle_schemas(&workspace_id, output_format, client).await,
        AssetsCommand::Types { schema } => {
            handle_types(&workspace_id, schema, output_format, client).await
        }
        AssetsCommand::Schema { name, schema } => {
            handle_schema(&workspace_id, &name, schema, output_format, client).await
        }
    }
}

async fn handle_search(
    workspace_id: &str,
    query: &str,
    limit: Option<u32>,
    attributes: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let objects = client
        .search_assets(workspace_id, query, limit, attributes)
        .await?;

    if attributes {
        let attr_map =
            crate::api::assets::objects::enrich_search_attributes(client, workspace_id, &objects)
                .await?;

        match output_format {
            OutputFormat::Json => {
                // Serialize to Value, inject objectTypeAttribute, filter system/hidden
                let mut json_objects: Vec<serde_json::Value> = Vec::new();
                for obj in &objects {
                    let mut obj_value = serde_json::to_value(obj)?;
                    if let Some(attrs_array) = obj_value
                        .get_mut("attributes")
                        .and_then(|a| a.as_array_mut())
                    {
                        // Inject objectTypeAttribute into each attribute
                        for attr_value in attrs_array.iter_mut() {
                            if let Some(attr_id) = attr_value
                                .get("objectTypeAttributeId")
                                .and_then(|v| v.as_str())
                            {
                                if let Some(def) = attr_map.get(attr_id) {
                                    if let Some(map) = attr_value.as_object_mut() {
                                        map.insert(
                                            "objectTypeAttribute".to_string(),
                                            serde_json::json!({
                                                "name": def.name,
                                                "position": def.position,
                                            }),
                                        );
                                    }
                                }
                            }
                        }
                        // Filter out system and hidden attributes
                        attrs_array.retain(|attr| {
                            let attr_id = attr
                                .get("objectTypeAttributeId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            match attr_map.get(attr_id) {
                                Some(def) => !def.system && !def.hidden,
                                None => true, // keep unknown attributes
                            }
                        });
                        // Sort by position
                        attrs_array.sort_by_key(|attr| {
                            let attr_id = attr
                                .get("objectTypeAttributeId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            attr_map
                                .get(attr_id)
                                .map(|d| d.position)
                                .unwrap_or(i32::MAX)
                        });
                    }
                    json_objects.push(obj_value);
                }
                println!("{}", output::render_json(&json_objects)?);
            }
            OutputFormat::Table => {
                let rows: Vec<Vec<String>> = objects
                    .iter()
                    .map(|o| {
                        let attr_str = format_inline_attributes(&o.attributes, &attr_map);
                        vec![
                            o.object_key.clone(),
                            o.object_type.name.clone(),
                            o.label.clone(),
                            attr_str,
                        ]
                    })
                    .collect();
                output::print_output(
                    output_format,
                    &["Key", "Type", "Name", "Attributes"],
                    &rows,
                    &objects,
                )?;
            }
        }
        Ok(())
    } else {
        let rows: Vec<Vec<String>> = objects
            .iter()
            .map(|o| {
                vec![
                    o.object_key.clone(),
                    o.object_type.name.clone(),
                    o.label.clone(),
                ]
            })
            .collect();
        output::print_output(output_format, &["Key", "Type", "Name"], &rows, &objects)
    }
}

/// Format attributes as inline `Name: Value` pairs for table display.
///
/// Filters out system, hidden, and label attributes. Sorts by position.
/// Attributes without a matching definition fall back to showing the raw ID.
/// Multi-value attributes use the first displayValue (or value as fallback).
fn format_inline_attributes(
    attributes: &[AssetAttribute],
    attr_map: &HashMap<String, CachedObjectTypeAttr>,
) -> String {
    // Pair each attribute with its definition (or None for unknown)
    let mut pairs: Vec<(&AssetAttribute, Option<&CachedObjectTypeAttr>)> = attributes
        .iter()
        .filter(|a| {
            match attr_map.get(&a.object_type_attribute_id) {
                Some(def) => !def.system && !def.hidden && !def.label,
                None => true, // keep unknown attributes (graceful degradation)
            }
        })
        .map(|a| (a, attr_map.get(&a.object_type_attribute_id)))
        .collect();
    // Known attributes sorted by position; unknown appended at end
    pairs.sort_by_key(|(_, def)| def.map(|d| d.position).unwrap_or(i32::MAX));

    pairs
        .iter()
        .filter_map(|(attr, def)| {
            let value = attr
                .values
                .first()
                .and_then(|v| v.display_value.as_deref().or(v.value.as_deref()));
            let name = def
                .map(|d| d.name.as_str())
                .unwrap_or(&attr.object_type_attribute_id);
            value.map(|v| format!("{}: {}", name, v))
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

async fn handle_view(
    workspace_id: &str,
    key: &str,
    no_attributes: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let object_id = objects::resolve_object_key(client, workspace_id, key).await?;
    let object = client.get_asset(workspace_id, &object_id, false).await?;

    match output_format {
        OutputFormat::Json => {
            if !no_attributes {
                let mut attrs = client
                    .get_object_attributes(workspace_id, &object_id)
                    .await?;
                // JSON: filter system and hidden only (keep label for programmatic consumers)
                attrs
                    .retain(|a| !a.object_type_attribute.system && !a.object_type_attribute.hidden);
                attrs.sort_by_key(|a| a.object_type_attribute.position);
                // Inject richer attributes into the existing object JSON to preserve
                // the root-level schema (additive change, not a wrapper envelope).
                let mut object_value = serde_json::to_value(&object)?;
                if let serde_json::Value::Object(ref mut map) = object_value {
                    map.insert("attributes".to_string(), serde_json::to_value(&attrs)?);
                }
                println!("{}", output::render_json(&object_value)?);
            } else {
                println!("{}", output::render_json(&object)?);
            }
        }
        OutputFormat::Table => {
            let mut rows = vec![
                vec!["Key".into(), object.object_key.clone()],
                vec!["Type".into(), object.object_type.name.clone()],
                vec!["Name".into(), object.label.clone()],
            ];

            if let Some(ref created) = object.created {
                rows.push(vec!["Created".into(), created.clone()]);
            }
            if let Some(ref updated) = object.updated {
                rows.push(vec!["Updated".into(), updated.clone()]);
            }

            println!("{}", output::render_table(&["Field", "Value"], &rows));

            if !no_attributes {
                let mut attrs = client
                    .get_object_attributes(workspace_id, &object_id)
                    .await?;
                attrs.retain(|a| {
                    !a.object_type_attribute.system
                        && !a.object_type_attribute.hidden
                        && !a.object_type_attribute.label
                });
                attrs.sort_by_key(|a| a.object_type_attribute.position);

                if !attrs.is_empty() {
                    println!();
                    let attr_rows: Vec<Vec<String>> = attrs
                        .iter()
                        .flat_map(|attr| {
                            attr.values.iter().map(move |v| {
                                vec![
                                    attr.object_type_attribute.name.clone(),
                                    v.display_value
                                        .clone()
                                        .or_else(|| v.value.clone())
                                        .unwrap_or_default(),
                                ]
                            })
                        })
                        .collect();
                    println!(
                        "{}",
                        output::render_table(&["Attribute", "Value"], &attr_rows)
                    );
                }
            }
        }
    }
    Ok(())
}

/// Filter connected tickets by status. Returns the filtered list.
///
/// `--open`: exclude tickets where status.colorName == "green" (Done category).
/// `--status`: partial match on status.name.
/// Tickets with no status are included by --open, excluded by --status.
fn filter_tickets(
    tickets: Vec<ConnectedTicket>,
    open: bool,
    status: Option<&str>,
) -> Result<Vec<ConnectedTicket>> {
    if open {
        return Ok(tickets
            .into_iter()
            .filter(|t| {
                t.status
                    .as_ref()
                    .and_then(|s| s.color_name.as_deref())
                    .map(|c| c != "green")
                    .unwrap_or(true)
            })
            .collect());
    }

    if let Some(status_input) = status {
        let mut seen = std::collections::HashSet::new();
        let status_names: Vec<String> = tickets
            .iter()
            .filter_map(|t| t.status.as_ref().map(|s| s.name.clone()))
            .filter(|name| seen.insert(name.clone()))
            .collect();

        let matched = match partial_match::partial_match(status_input, &status_names) {
            MatchResult::Exact(name) => name,
            // Case-sensitive dedup upstream; treat like Exact if case-variant duplicates slip through
            MatchResult::ExactMultiple(name) => name,
            MatchResult::Ambiguous(matches) => {
                return Err(JrError::UserError(format!(
                    "Ambiguous status \"{}\". Matches: {}",
                    status_input,
                    matches.join(", ")
                ))
                .into());
            }
            MatchResult::None(all) => {
                let available = if all.is_empty() {
                    "none".to_string()
                } else {
                    all.join(", ")
                };
                return Err(JrError::UserError(format!(
                    "No status matching \"{}\". Available: {}",
                    status_input, available
                ))
                .into());
            }
        };

        return Ok(tickets
            .into_iter()
            .filter(|t| {
                t.status
                    .as_ref()
                    .map(|s| s.name == matched)
                    .unwrap_or(false)
            })
            .collect());
    }

    Ok(tickets)
}

async fn handle_tickets(
    workspace_id: &str,
    key: &str,
    limit: Option<u32>,
    open: bool,
    status: Option<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let object_id = objects::resolve_object_key(client, workspace_id, key).await?;
    let resp = client
        .get_connected_tickets(workspace_id, &object_id)
        .await?;

    let has_filter = open || status.is_some();
    let filtered = filter_tickets(resp.tickets, open, status.as_deref())?;

    let tickets: Vec<_> = match limit {
        Some(n) => filtered.into_iter().take(n as usize).collect(),
        None => filtered,
    };

    match output_format {
        OutputFormat::Json => {
            if has_filter {
                // Filtered: return bare array (allTicketsQuery no longer represents what's shown)
                println!("{}", output::render_json(&tickets)?);
            } else {
                // Unfiltered: preserve full response envelope for backward compatibility
                println!(
                    "{}",
                    output::render_json(&crate::types::assets::ConnectedTicketsResponse {
                        tickets,
                        all_tickets_query: resp.all_tickets_query,
                    })?
                );
            }
        }
        OutputFormat::Table => {
            let rows: Vec<Vec<String>> = tickets
                .iter()
                .map(|t| {
                    vec![
                        t.key.clone(),
                        t.issue_type
                            .as_ref()
                            .map(|it| it.name.clone())
                            .unwrap_or_else(|| "\u{2014}".into()),
                        t.title.clone(),
                        t.status
                            .as_ref()
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| "\u{2014}".into()),
                        t.priority
                            .as_ref()
                            .map(|p| p.name.clone())
                            .unwrap_or_else(|| "\u{2014}".into()),
                    ]
                })
                .collect();

            output::print_output(
                output_format,
                &["Key", "Type", "Title", "Status", "Priority"],
                &rows,
                &tickets,
            )?;
        }
    }
    Ok(())
}

/// Resolve a --schema flag to a single schema, matching by ID (exact) or name (partial).
fn resolve_schema<'a>(
    input: &str,
    schemas: &'a [crate::types::assets::ObjectSchema],
) -> Result<&'a crate::types::assets::ObjectSchema> {
    // Try exact ID match first
    if let Some(s) = schemas.iter().find(|s| s.id == input) {
        return Ok(s);
    }
    // Partial match on name
    let names: Vec<String> = schemas.iter().map(|s| s.name.clone()).collect();
    match partial_match::partial_match(input, &names) {
        MatchResult::Exact(name) => Ok(schemas.iter().find(|s| s.name == name).unwrap()),
        MatchResult::ExactMultiple(_) => {
            let input_lower = input.to_lowercase();
            let duplicates: Vec<String> = schemas
                .iter()
                .filter(|s| s.name.to_lowercase() == input_lower)
                .map(|s| format!("{} (id: {})", s.name, s.id))
                .collect();
            Err(JrError::UserError(format!(
                "Multiple schemas named \"{}\": {}. Use the schema ID instead.",
                input,
                duplicates.join(", ")
            ))
            .into())
        }
        MatchResult::Ambiguous(matches) => Err(JrError::UserError(format!(
            "Ambiguous schema \"{}\". Matches: {}",
            input,
            matches.join(", ")
        ))
        .into()),
        MatchResult::None(all) => {
            let available = if all.is_empty() {
                "none".to_string()
            } else {
                all.join(", ")
            };
            Err(JrError::UserError(format!(
                "No schema matching \"{}\". Available: {}",
                input, available
            ))
            .into())
        }
    }
}

async fn handle_schemas(
    workspace_id: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let schemas = client.list_object_schemas(workspace_id).await?;
    if schemas.is_empty() {
        return Err(JrError::UserError("No asset schemas found in this workspace.".into()).into());
    }

    let rows: Vec<Vec<String>> = schemas
        .iter()
        .map(|s| {
            vec![
                s.id.clone(),
                s.object_schema_key.clone(),
                s.name.clone(),
                s.description.clone().unwrap_or_else(|| "\u{2014}".into()),
                s.object_type_count.to_string(),
                s.object_count.to_string(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "Key", "Name", "Description", "Types", "Objects"],
        &rows,
        &schemas,
    )
}

async fn handle_types(
    workspace_id: &str,
    schema_filter: Option<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let schemas = client.list_object_schemas(workspace_id).await?;
    if schemas.is_empty() {
        return Err(JrError::UserError("No asset schemas found in this workspace.".into()).into());
    }

    let target_schemas: Vec<&crate::types::assets::ObjectSchema> = match &schema_filter {
        Some(input) => vec![resolve_schema(input, &schemas)?],
        None => schemas.iter().collect(),
    };

    // Build a map of schema_id → schema_name for injection
    let schema_names: HashMap<&str, &str> = schemas
        .iter()
        .map(|s| (s.id.as_str(), s.name.as_str()))
        .collect();

    let mut all_types = Vec::new();
    for schema in &target_schemas {
        let types = client.list_object_types(workspace_id, &schema.id).await?;
        all_types.extend(types);
    }

    match output_format {
        OutputFormat::Json => {
            // Inject schemaName into each entry
            let mut json_types: Vec<serde_json::Value> = Vec::new();
            for t in &all_types {
                let mut val = serde_json::to_value(t)?;
                if let Some(map) = val.as_object_mut() {
                    let schema_name = schema_names.get(t.object_schema_id.as_str()).unwrap_or(&"");
                    map.insert(
                        "schemaName".to_string(),
                        serde_json::Value::String(schema_name.to_string()),
                    );
                }
                json_types.push(val);
            }
            println!("{}", output::render_json(&json_types)?);
        }
        OutputFormat::Table => {
            let rows: Vec<Vec<String>> = all_types
                .iter()
                .map(|t| {
                    let schema_name = schema_names
                        .get(t.object_schema_id.as_str())
                        .unwrap_or(&"\u{2014}");
                    vec![
                        t.id.clone(),
                        t.name.clone(),
                        schema_name.to_string(),
                        t.description.clone().unwrap_or_else(|| "\u{2014}".into()),
                        t.object_count.to_string(),
                    ]
                })
                .collect();

            output::print_output(
                output_format,
                &["ID", "Name", "Schema", "Description", "Objects"],
                &rows,
                &all_types,
            )?;
        }
    }
    Ok(())
}

/// Build an ambiguous type error with schema-labeled matches.
fn ambiguous_type_error(
    input: &str,
    matches: &[String],
    candidates: &[(crate::types::assets::ObjectTypeEntry, String)],
) -> JrError {
    let labeled: Vec<String> = candidates
        .iter()
        .filter(|(t, _)| matches.contains(&t.name))
        .map(|(t, s)| format!("{} ({})", t.name, s))
        .collect();
    JrError::UserError(format!(
        "Ambiguous type \"{}\". Matches: {}. Use --schema to narrow results.",
        input,
        labeled.join(", ")
    ))
}

/// Format the Type column for an attribute definition.
fn format_attribute_type(attr: &crate::types::assets::ObjectTypeAttributeDef) -> String {
    if let Some(ref dt) = attr.default_type {
        return dt.name.clone();
    }
    if let Some(ref rot) = attr.reference_object_type {
        return format!("Reference \u{2192} {}", rot.name);
    }
    "Unknown".to_string()
}

async fn handle_schema(
    workspace_id: &str,
    type_name: &str,
    schema_filter: Option<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let schemas = client.list_object_schemas(workspace_id).await?;
    if schemas.is_empty() {
        return Err(JrError::UserError("No asset schemas found in this workspace.".into()).into());
    }

    let target_schemas: Vec<&crate::types::assets::ObjectSchema> = match &schema_filter {
        Some(input) => vec![resolve_schema(input, &schemas)?],
        None => schemas.iter().collect(),
    };

    // Collect all object types with their schema name
    let mut candidates: Vec<(crate::types::assets::ObjectTypeEntry, String)> = Vec::new();
    for schema in &target_schemas {
        let types = client.list_object_types(workspace_id, &schema.id).await?;
        for t in types {
            candidates.push((t, schema.name.clone()));
        }
    }

    if candidates.is_empty() {
        return Err(JrError::UserError(
            "No object types found. Run \"jr assets schemas\" to verify your workspace has schemas."
                .into(),
        )
        .into());
    }

    // Partial match on type name — deduplicated for partial_match, then
    // check for cross-schema duplicates on the resolved name.
    let mut deduped_names: Vec<String> = candidates.iter().map(|(t, _)| t.name.clone()).collect();
    deduped_names.sort();
    deduped_names.dedup();
    let matched_name = match partial_match::partial_match(type_name, &deduped_names) {
        MatchResult::Exact(name) => name,
        // Case-sensitive dedup upstream; treat like Exact if case-variant duplicates slip through
        MatchResult::ExactMultiple(name) => name,
        MatchResult::Ambiguous(matches) => {
            return Err(ambiguous_type_error(type_name, &matches, &candidates).into());
        }
        MatchResult::None(_) => {
            return Err(JrError::UserError(format!(
                "No object type matching \"{}\". Run \"jr assets types\" to see available types.",
                type_name
            ))
            .into());
        }
    };

    // Check for cross-schema duplicates: same name in multiple schemas
    let same_name: Vec<&(crate::types::assets::ObjectTypeEntry, String)> = candidates
        .iter()
        .filter(|(t, _)| t.name == matched_name)
        .collect();
    if same_name.len() > 1 {
        let labeled: Vec<String> = same_name
            .iter()
            .map(|(t, s)| format!("{} ({})", t.name, s))
            .collect();
        return Err(JrError::UserError(format!(
            "Ambiguous type \"{}\". Matches: {}. Use --schema to narrow results.",
            type_name,
            labeled.join(", ")
        ))
        .into());
    }

    let (matched_type, schema_name) = same_name.first().unwrap();

    // Fetch attributes
    let attrs = client
        .get_object_type_attributes(workspace_id, &matched_type.id)
        .await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&attrs)?);
        }
        OutputFormat::Table => {
            println!(
                "Object Type: {} (Schema: {})\n",
                matched_type.name, schema_name
            );

            let mut visible: Vec<&crate::types::assets::ObjectTypeAttributeDef> =
                attrs.iter().filter(|a| !a.system && !a.hidden).collect();
            visible.sort_by_key(|a| a.position);

            let rows: Vec<Vec<String>> = visible
                .iter()
                .map(|a| {
                    vec![
                        a.position.to_string(),
                        a.name.clone(),
                        format_attribute_type(a),
                        if a.minimum_cardinality >= 1 {
                            "Yes".into()
                        } else {
                            "No".into()
                        },
                        if a.editable {
                            "Yes".into()
                        } else {
                            "No".into()
                        },
                    ]
                })
                .collect();

            if rows.is_empty() {
                println!("No user-defined attributes.");
            } else {
                println!(
                    "{}",
                    output::render_table(&["Pos", "Name", "Type", "Required", "Editable"], &rows)
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::assets::{ConnectedTicket, TicketPriority, TicketStatus, TicketType};

    fn make_ticket(key: &str, status_name: &str, color: &str) -> ConnectedTicket {
        ConnectedTicket {
            key: key.to_string(),
            id: "1".to_string(),
            title: format!("Ticket {}", key),
            reporter: None,
            created: None,
            updated: None,
            status: Some(TicketStatus {
                name: status_name.to_string(),
                color_name: Some(color.to_string()),
            }),
            issue_type: Some(TicketType {
                name: "Task".to_string(),
            }),
            priority: Some(TicketPriority {
                name: "Medium".to_string(),
            }),
        }
    }

    fn make_ticket_no_status(key: &str) -> ConnectedTicket {
        ConnectedTicket {
            key: key.to_string(),
            id: "1".to_string(),
            title: format!("Ticket {}", key),
            reporter: None,
            created: None,
            updated: None,
            status: None,
            issue_type: None,
            priority: None,
        }
    }

    #[test]
    fn filter_open_excludes_done() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "Done", "green"),
            make_ticket("A-3", "To Do", "blue-gray"),
        ];
        let result = filter_tickets(tickets, true, None).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].key, "A-1");
        assert_eq!(result[1].key, "A-3");
    }

    #[test]
    fn filter_open_includes_no_status() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket_no_status("A-2"),
        ];
        let result = filter_tickets(tickets, true, None).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_status_exact_match() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "Done", "green"),
            make_ticket("A-3", "To Do", "blue-gray"),
        ];
        let result = filter_tickets(tickets, false, Some("Done")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "A-2");
    }

    #[test]
    fn filter_status_single_substring_is_ambiguous() {
        // Single substring hits are now Ambiguous — callers must use the exact name.
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "Done", "green"),
        ];
        let result = filter_tickets(tickets, false, Some("prog"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"), "got: {err}");
        assert!(err.contains("In Progress"), "got: {err}");
    }

    #[test]
    fn filter_status_no_match() {
        let tickets = vec![make_ticket("A-1", "In Progress", "yellow")];
        let result = filter_tickets(tickets, false, Some("Blocked"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No status matching"));
        assert!(err.contains("In Progress"));
    }

    #[test]
    fn filter_status_ambiguous() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "In Review", "yellow"),
        ];
        let result = filter_tickets(tickets, false, Some("In"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"));
    }

    #[test]
    fn filter_status_excludes_no_status() {
        let tickets = vec![
            make_ticket("A-1", "Done", "green"),
            make_ticket_no_status("A-2"),
        ];
        let result = filter_tickets(tickets, false, Some("Done")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "A-1");
    }

    #[test]
    fn no_filter_returns_all() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "Done", "green"),
        ];
        let result = filter_tickets(tickets, false, None).unwrap();
        assert_eq!(result.len(), 2);
    }

    use crate::types::assets::{DefaultType, ObjectTypeAttributeDef, ReferenceObjectType};

    fn make_attr_def(
        default_type: Option<DefaultType>,
        reference_object_type: Option<ReferenceObjectType>,
    ) -> ObjectTypeAttributeDef {
        ObjectTypeAttributeDef {
            id: "1".into(),
            name: "test".into(),
            system: false,
            hidden: false,
            label: false,
            position: 0,
            default_type,
            reference_type: None,
            reference_object_type,
            minimum_cardinality: 0,
            maximum_cardinality: 1,
            editable: true,
            description: None,
            options: None,
        }
    }

    #[test]
    fn format_attr_type_default_type() {
        let attr = make_attr_def(
            Some(DefaultType {
                id: 0,
                name: "Text".into(),
            }),
            None,
        );
        assert_eq!(super::format_attribute_type(&attr), "Text");
    }

    #[test]
    fn format_attr_type_reference() {
        let attr = make_attr_def(
            None,
            Some(ReferenceObjectType {
                id: "122".into(),
                name: "Service".into(),
            }),
        );
        assert_eq!(
            super::format_attribute_type(&attr),
            "Reference \u{2192} Service"
        );
    }

    #[test]
    fn format_attr_type_unknown() {
        let attr = make_attr_def(None, None);
        assert_eq!(super::format_attribute_type(&attr), "Unknown");
    }

    #[test]
    fn format_attr_type_default_takes_precedence() {
        let attr = make_attr_def(
            Some(DefaultType {
                id: 0,
                name: "Text".into(),
            }),
            Some(ReferenceObjectType {
                id: "1".into(),
                name: "Svc".into(),
            }),
        );
        assert_eq!(super::format_attribute_type(&attr), "Text");
    }

    // ── resolve_schema tests ─────────────────────────────────────

    fn make_schema(id: &str, name: &str) -> crate::types::assets::ObjectSchema {
        crate::types::assets::ObjectSchema {
            id: id.into(),
            name: name.into(),
            object_schema_key: format!("KEY{}", id),
            description: None,
            object_count: 0,
            object_type_count: 0,
        }
    }

    #[test]
    fn resolve_schema_exact_id_match() {
        let schemas = vec![make_schema("10", "ITSM"), make_schema("20", "HR")];
        let result = super::resolve_schema("10", &schemas).unwrap();
        assert_eq!(result.id, "10");
        assert_eq!(result.name, "ITSM");
    }

    #[test]
    fn resolve_schema_exact_name_match() {
        let schemas = vec![make_schema("10", "ITSM"), make_schema("20", "HR")];
        let result = super::resolve_schema("ITSM", &schemas).unwrap();
        assert_eq!(result.id, "10");
    }

    #[test]
    fn resolve_schema_case_insensitive_name_match() {
        let schemas = vec![make_schema("10", "ITSM"), make_schema("20", "HR")];
        let result = super::resolve_schema("itsm", &schemas).unwrap();
        assert_eq!(result.id, "10");
    }

    #[test]
    fn resolve_schema_single_substring_is_ambiguous() {
        // Single substring hits are now Ambiguous — callers must use the exact name.
        let schemas = vec![make_schema("10", "ITSM Assets"), make_schema("20", "HR")];
        let err = super::resolve_schema("itsm", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Ambiguous"), "got: {msg}");
        assert!(msg.contains("ITSM Assets"), "got: {msg}");
    }

    #[test]
    fn resolve_schema_no_match() {
        let schemas = vec![make_schema("10", "ITSM"), make_schema("20", "HR")];
        let err = super::resolve_schema("Finance", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("No schema matching"), "got: {msg}");
        assert!(msg.contains("Finance"), "got: {msg}");
    }

    #[test]
    fn resolve_schema_ambiguous_match() {
        let schemas = vec![
            make_schema("10", "IT Assets"),
            make_schema("20", "IT Services"),
        ];
        let err = super::resolve_schema("IT", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Ambiguous"), "got: {msg}");
    }

    #[test]
    fn resolve_schema_duplicate_names_returns_error_with_ids() {
        let schemas = vec![make_schema("10", "Assets"), make_schema("20", "Assets")];
        let err = super::resolve_schema("Assets", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Multiple schemas"), "got: {msg}");
        assert!(msg.contains("id: 10"), "should list first ID, got: {msg}");
        assert!(msg.contains("id: 20"), "should list second ID, got: {msg}");
        assert!(
            msg.contains("Use the schema ID instead"),
            "should suggest using ID, got: {msg}"
        );
    }

    #[test]
    fn resolve_schema_duplicate_names_case_insensitive() {
        let schemas = vec![make_schema("10", "Assets"), make_schema("20", "assets")];
        let err = super::resolve_schema("assets", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Multiple schemas"), "got: {msg}");
        assert!(msg.contains("id: 10"), "should list first ID, got: {msg}");
        assert!(msg.contains("id: 20"), "should list second ID, got: {msg}");
    }

    #[test]
    fn resolve_schema_id_takes_priority_over_name() {
        // Schema ID "HR" matches exactly, even though name "ITSM" doesn't
        let schemas = vec![make_schema("HR", "ITSM"), make_schema("20", "HR")];
        let result = super::resolve_schema("HR", &schemas).unwrap();
        assert_eq!(result.id, "HR");
        assert_eq!(result.name, "ITSM");
    }
}
