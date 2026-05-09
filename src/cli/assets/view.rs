use anyhow::Result;

use crate::api::assets::objects;
use crate::api::client::JiraClient;
use crate::cli::OutputFormat;
use crate::output;

pub async fn handle_view(
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
