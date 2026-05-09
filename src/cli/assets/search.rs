use std::collections::HashMap;

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cache::CachedObjectTypeAttr;
use crate::cli::OutputFormat;
use crate::output;
use crate::types::assets::AssetAttribute;

pub async fn handle_search(
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
