use anyhow::Result;

use crate::api::assets::{objects, workspace};
use crate::api::client::JiraClient;
use crate::cli::{AssetsCommand, OutputFormat};
use crate::output;

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
        AssetsCommand::Tickets { key, limit } => {
            handle_tickets(&workspace_id, &key, limit, output_format, client).await
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
    let object = client
        .get_asset(workspace_id, &object_id, attributes)
        .await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&object)?);
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

            if attributes && !object.attributes.is_empty() {
                println!();
                let attr_rows: Vec<Vec<String>> = object
                    .attributes
                    .iter()
                    .flat_map(|attr| {
                        attr.values.iter().map(move |v| {
                            vec![
                                attr.object_type_attribute_id.clone(),
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
                    output::render_table(&["Attribute ID", "Value"], &attr_rows)
                );
            }
        }
    }
    Ok(())
}

async fn handle_tickets(
    workspace_id: &str,
    key: &str,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let object_id = objects::resolve_object_key(client, workspace_id, key).await?;
    let resp = client
        .get_connected_tickets(workspace_id, &object_id)
        .await?;

    match output_format {
        OutputFormat::Json => {
            // JSON: return full response including allTicketsQuery metadata
            println!("{}", output::render_json(&resp)?);
        }
        OutputFormat::Table => {
            let tickets: Vec<_> = match limit {
                Some(n) => resp.tickets.into_iter().take(n as usize).collect(),
                None => resp.tickets,
            };

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
