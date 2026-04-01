use anyhow::Result;

use crate::api::assets::{objects, workspace};
use crate::api::client::JiraClient;
use crate::cli::{AssetsCommand, OutputFormat};
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};
use crate::types::assets::ConnectedTicket;

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
        AssetsCommand::View { key, attributes } => {
            handle_view(&workspace_id, &key, attributes, output_format, client).await
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
        let rows: Vec<Vec<String>> = objects
            .iter()
            .map(|o| {
                vec![
                    o.object_key.clone(),
                    o.object_type.name.clone(),
                    o.label.clone(),
                    o.created.clone().unwrap_or_default(),
                    o.updated.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::print_output(
            output_format,
            &["Key", "Type", "Name", "Created", "Updated"],
            &rows,
            &objects,
        )
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

async fn handle_view(
    workspace_id: &str,
    key: &str,
    attributes: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let object_id = objects::resolve_object_key(client, workspace_id, key).await?;
    let object = client.get_asset(workspace_id, &object_id, false).await?;

    match output_format {
        OutputFormat::Json => {
            if attributes {
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

            if attributes {
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

    let filtered = filter_tickets(resp.tickets, open, status.as_deref())?;

    let tickets: Vec<_> = match limit {
        Some(n) => filtered.into_iter().take(n as usize).collect(),
        None => filtered,
    };

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&tickets)?);
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
