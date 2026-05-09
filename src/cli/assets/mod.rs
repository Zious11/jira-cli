mod schemas;
mod search;
mod tickets;
mod view;

use anyhow::Result;

use crate::api::assets::workspace;
use crate::api::client::JiraClient;
use crate::cli::{AssetsCommand, OutputFormat};

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
            search::handle_search(
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
            view::handle_view(&workspace_id, &key, no_attributes, output_format, client).await
        }
        AssetsCommand::Tickets {
            key,
            limit,
            open,
            status,
        } => {
            tickets::handle_tickets(
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
        AssetsCommand::Schemas => {
            schemas::handle_schemas(&workspace_id, output_format, client).await
        }
        AssetsCommand::Types { schema } => {
            schemas::handle_types(&workspace_id, schema, output_format, client).await
        }
        AssetsCommand::Schema { name, schema } => {
            schemas::handle_schema(&workspace_id, &name, schema, output_format, client).await
        }
    }
}
