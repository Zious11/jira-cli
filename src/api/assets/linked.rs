use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;

use crate::api::assets::workspace::get_or_fetch_workspace_id;
use crate::api::client::JiraClient;
use crate::cache;
use crate::types::assets::LinkedAsset;

/// Get CMDB field IDs, using cache when available.
pub async fn get_or_fetch_cmdb_field_ids(client: &JiraClient) -> Result<Vec<String>> {
    if let Some(cached) = cache::read_cmdb_fields_cache()? {
        return Ok(cached.field_ids);
    }

    let field_ids = client.find_cmdb_field_ids().await?;
    let _ = cache::write_cmdb_fields_cache(&field_ids);
    Ok(field_ids)
}

/// Extract linked assets from issue extra fields using discovered CMDB field IDs.
pub fn extract_linked_assets(
    extra: &HashMap<String, Value>,
    cmdb_field_ids: &[String],
) -> Vec<LinkedAsset> {
    let mut assets = Vec::new();

    for field_id in cmdb_field_ids {
        let Some(value) = extra.get(field_id) else {
            continue;
        };
        if value.is_null() {
            continue;
        }

        match value {
            Value::Array(arr) => {
                for item in arr {
                    if let Some(asset) = parse_cmdb_value(item) {
                        assets.push(asset);
                    }
                }
            }
            Value::Object(_) => {
                if let Some(asset) = parse_cmdb_value(value) {
                    assets.push(asset);
                }
            }
            Value::String(s) => {
                assets.push(LinkedAsset {
                    name: Some(s.clone()),
                    ..Default::default()
                });
            }
            _ => {}
        }
    }

    assets
}

fn parse_cmdb_value(value: &Value) -> Option<LinkedAsset> {
    let obj = value.as_object()?;

    let label = obj.get("label").and_then(|v| v.as_str()).map(String::from);
    let object_key = obj
        .get("objectKey")
        .and_then(|v| v.as_str())
        .map(String::from);
    let object_id = obj.get("objectId").and_then(|v| {
        v.as_str()
            .map(String::from)
            .or_else(|| v.as_u64().map(|n| n.to_string()))
    });
    let workspace_id = obj
        .get("workspaceId")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Only create an asset if we got at least something useful.
    if label.is_none() && object_key.is_none() && object_id.is_none() {
        return None;
    }

    Some(LinkedAsset {
        key: object_key,
        name: label,
        asset_type: None,
        id: object_id,
        workspace_id,
    })
}

/// Enrich assets that only have IDs by fetching from the Assets API.
pub async fn enrich_assets(client: &JiraClient, assets: &mut [LinkedAsset]) {
    // Only enrich assets that have an ID but are missing key/name.
    let needs_enrichment: Vec<usize> = assets
        .iter()
        .enumerate()
        .filter(|(_, a)| a.id.is_some() && a.key.is_none() && a.name.is_none())
        .map(|(i, _)| i)
        .collect();

    if needs_enrichment.is_empty() {
        return;
    }

    // Check whether all assets that need enrichment carry their own workspace_id.
    // If any are missing it, we fall back to fetching the global workspace ID.
    let all_have_workspace = needs_enrichment
        .iter()
        .all(|&idx| assets[idx].workspace_id.is_some());

    let fallback_workspace_id: Option<String> = if all_have_workspace {
        None
    } else {
        // Get workspace ID — required for Assets API calls.
        match get_or_fetch_workspace_id(client).await {
            Ok(wid) => Some(wid),
            Err(_) => return, // Degrade gracefully
        }
    };

    let futures: Vec<_> = needs_enrichment
        .iter()
        .map(|&idx| {
            // Prefer the per-asset workspace_id; fall back to the global one.
            let wid = assets[idx]
                .workspace_id
                .clone()
                .or_else(|| fallback_workspace_id.clone())
                .expect("workspace_id must be available (checked above)");
            let oid = assets[idx].id.clone().unwrap();
            async move {
                let result = client.get_asset(&wid, &oid, false).await;
                (idx, result)
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    for (idx, result) in results {
        if let Ok(obj) = result {
            assets[idx].key = Some(obj.object_key);
            assets[idx].name = Some(obj.label);
            assets[idx].asset_type = Some(obj.object_type.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_extra(field_id: &str, value: Value) -> HashMap<String, Value> {
        let mut map = HashMap::new();
        map.insert(field_id.to_string(), value);
        map
    }

    #[test]
    fn parse_modern_label_and_key() {
        let extra = make_extra(
            "customfield_10191",
            json!([{"label": "Acme Corp", "objectKey": "OBJ-1"}]),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].key.as_deref(), Some("OBJ-1"));
        assert_eq!(assets[0].name.as_deref(), Some("Acme Corp"));
        assert!(assets[0].id.is_none());
    }

    #[test]
    fn parse_legacy_ids_only() {
        let extra = make_extra(
            "customfield_10191",
            json!([{"workspaceId": "ws-1", "objectId": "88", "id": "ws-1:88"}]),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].id.as_deref(), Some("88"));
        assert_eq!(assets[0].workspace_id.as_deref(), Some("ws-1"));
        assert!(assets[0].key.is_none());
        assert!(assets[0].name.is_none());
    }

    #[test]
    fn parse_mixed_fields() {
        let extra = make_extra(
            "customfield_10191",
            json!([{
                "label": "Acme Corp",
                "objectKey": "OBJ-1",
                "workspaceId": "ws-1",
                "objectId": "88"
            }]),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].key.as_deref(), Some("OBJ-1"));
        assert_eq!(assets[0].name.as_deref(), Some("Acme Corp"));
        assert_eq!(assets[0].id.as_deref(), Some("88"));
    }

    #[test]
    fn parse_null_field_skipped() {
        let extra = make_extra("customfield_10191", Value::Null);
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert!(assets.is_empty());
    }

    #[test]
    fn parse_empty_array() {
        let extra = make_extra("customfield_10191", json!([]));
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert!(assets.is_empty());
    }

    #[test]
    fn parse_missing_field_skipped() {
        let extra = HashMap::new();
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert!(assets.is_empty());
    }

    #[test]
    fn parse_string_value_as_name() {
        let extra = make_extra("customfield_10191", json!("Some Asset"));
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].name.as_deref(), Some("Some Asset"));
    }

    #[test]
    fn parse_multiple_cmdb_fields() {
        let mut extra = HashMap::new();
        extra.insert(
            "customfield_10191".into(),
            json!([{"label": "Acme", "objectKey": "OBJ-1"}]),
        );
        extra.insert(
            "customfield_10245".into(),
            json!([{"label": "Server-1", "objectKey": "SRV-1"}]),
        );
        let field_ids = vec!["customfield_10191".into(), "customfield_10245".into()];
        let assets = extract_linked_assets(&extra, &field_ids);
        assert_eq!(assets.len(), 2);
    }

    #[test]
    fn parse_multiple_objects_in_array() {
        let extra = make_extra(
            "customfield_10191",
            json!([
                {"label": "Acme", "objectKey": "OBJ-1"},
                {"label": "Globex", "objectKey": "OBJ-2"}
            ]),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].name.as_deref(), Some("Acme"));
        assert_eq!(assets[1].name.as_deref(), Some("Globex"));
    }

    #[test]
    fn parse_single_object_not_array() {
        let extra = make_extra(
            "customfield_10191",
            json!({"label": "Acme", "objectKey": "OBJ-1"}),
        );
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].key.as_deref(), Some("OBJ-1"));
    }

    #[test]
    fn parse_empty_object_skipped() {
        let extra = make_extra("customfield_10191", json!([{}]));
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert!(assets.is_empty());
    }

    #[test]
    fn parse_numeric_object_id() {
        let extra = make_extra("customfield_10191", json!([{"objectId": 88}]));
        let assets = extract_linked_assets(&extra, &["customfield_10191".into()]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].id.as_deref(), Some("88"));
    }
}
