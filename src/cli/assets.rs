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
    fn filter_status_partial_match() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "Done", "green"),
        ];
        let result = filter_tickets(tickets, false, Some("prog")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "A-1");
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
}
