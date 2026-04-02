use std::collections::HashMap;

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::AssetsPage;
use crate::cache::{self, CachedObjectTypeAttr};
use crate::error::JrError;
use crate::types::assets::{AssetObject, ObjectAttribute, ObjectTypeAttributeDef};

impl JiraClient {
    /// Search assets via AQL with auto-pagination.
    ///
    /// The `aql` parameter is passed to the API verbatim — callers must ensure
    /// the query is trusted input. For user-supplied object keys interpolated
    /// into AQL, use `resolve_object_key()` which escapes special characters.
    pub async fn search_assets(
        &self,
        workspace_id: &str,
        aql: &str,
        limit: Option<u32>,
        include_attributes: bool,
    ) -> Result<Vec<AssetObject>> {
        let mut all = Vec::new();
        let mut start_at = 0u32;
        let max_page_size = 25u32;

        loop {
            let page_size = match limit {
                Some(cap) => {
                    let remaining = cap.saturating_sub(all.len() as u32);
                    if remaining == 0 {
                        break;
                    }
                    remaining.min(max_page_size)
                }
                None => max_page_size,
            };

            let path = format!(
                "object/aql?startAt={}&maxResults={}&includeAttributes={}",
                start_at, page_size, include_attributes
            );
            let body = serde_json::json!({ "qlQuery": aql });
            let page: AssetsPage<AssetObject> =
                self.post_assets(workspace_id, &path, &body).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values);

            if let Some(cap) = limit {
                if all.len() >= cap as usize {
                    all.truncate(cap as usize);
                    break;
                }
            }
            if !has_more {
                break;
            }
            start_at = next;
        }
        Ok(all)
    }

    /// Get a single asset by its numeric ID.
    pub async fn get_asset(
        &self,
        workspace_id: &str,
        object_id: &str,
        include_attributes: bool,
    ) -> Result<AssetObject> {
        let path = format!(
            "object/{}?includeAttributes={}",
            urlencoding::encode(object_id),
            include_attributes
        );
        self.get_assets(workspace_id, &path).await
    }

    /// Get all attributes for a single object, with full attribute definitions
    /// including human-readable names.
    pub async fn get_object_attributes(
        &self,
        workspace_id: &str,
        object_id: &str,
    ) -> Result<Vec<ObjectAttribute>> {
        let path = format!("object/{}/attributes", urlencoding::encode(object_id));
        self.get_assets(workspace_id, &path).await
    }

    /// Get all attribute definitions for an object type.
    ///
    /// Returns schema-level metadata (name, system, hidden, label, position)
    /// for every attribute defined on the type. Used to enrich search results
    /// where only `objectTypeAttributeId` is present.
    pub async fn get_object_type_attributes(
        &self,
        workspace_id: &str,
        object_type_id: &str,
    ) -> Result<Vec<ObjectTypeAttributeDef>> {
        let path = format!(
            "objecttype/{}/attributes",
            urlencoding::encode(object_type_id)
        );
        self.get_assets(workspace_id, &path).await
    }
}

/// Resolve an object key (e.g., "OBJ-1") to its numeric ID.
/// If the input is purely numeric, returns it as-is.
pub async fn resolve_object_key(
    client: &JiraClient,
    workspace_id: &str,
    key_or_id: &str,
) -> Result<String> {
    if key_or_id.is_empty() {
        return Err(JrError::UserError("Object key or ID cannot be empty.".into()).into());
    }

    if key_or_id.chars().all(|c| c.is_ascii_digit()) {
        return Ok(key_or_id.to_string());
    }

    // Escape quotes and backslashes to prevent AQL injection.
    // AQL uses "Key" (not "objectKey") to match the object key field.
    let escaped = key_or_id.replace('\\', "\\\\").replace('"', "\\\"");

    let results = client
        .search_assets(
            workspace_id,
            &format!("Key = \"{}\"", escaped),
            Some(1),
            false,
        )
        .await?;

    results.into_iter().next().map(|obj| obj.id).ok_or_else(|| {
        JrError::UserError(format!(
            "No asset matching \"{}\" found. Check the object key and try again.",
            key_or_id
        ))
        .into()
    })
}

/// Enrich search results by resolving attribute definitions for each unique object type.
///
/// Returns a HashMap mapping `objectTypeAttributeId` → `CachedObjectTypeAttr` for use
/// in output formatting (filtering system/hidden, sorting by position, displaying names).
///
/// Fetches definitions from cache first, falling back to the API. Results are cached
/// for 7 days per object type.
pub async fn enrich_search_attributes(
    client: &JiraClient,
    workspace_id: &str,
    objects: &[AssetObject],
) -> Result<HashMap<String, CachedObjectTypeAttr>> {
    // Collect unique object type IDs
    let mut type_ids: Vec<String> = objects.iter().map(|o| o.object_type.id.clone()).collect();
    type_ids.sort();
    type_ids.dedup();

    let mut attr_map: HashMap<String, CachedObjectTypeAttr> = HashMap::new();

    for type_id in &type_ids {
        // Try cache first
        let attrs = match cache::read_object_type_attr_cache(type_id) {
            Ok(Some(cached)) => cached,
            _ => {
                // Cache miss — fetch from API
                match client
                    .get_object_type_attributes(workspace_id, type_id)
                    .await
                {
                    Ok(defs) => {
                        let cached: Vec<CachedObjectTypeAttr> = defs
                            .iter()
                            .map(|d| CachedObjectTypeAttr {
                                id: d.id.clone(),
                                name: d.name.clone(),
                                system: d.system,
                                hidden: d.hidden,
                                label: d.label,
                                position: d.position,
                            })
                            .collect();
                        // Best-effort cache write
                        let _ = cache::write_object_type_attr_cache(type_id, &cached);
                        cached
                    }
                    Err(_) => {
                        // Graceful degradation: skip this type, let caller decide on warnings
                        continue;
                    }
                }
            }
        };

        for attr in attrs {
            attr_map.insert(attr.id.clone(), attr);
        }
    }

    Ok(attr_map)
}

#[cfg(test)]
mod tests {
    #[test]
    fn numeric_id_detected() {
        assert!("123".chars().all(|c| c.is_ascii_digit()));
        assert!("0".chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn object_key_not_numeric() {
        assert!(!"OBJ-1".chars().all(|c| c.is_ascii_digit()));
        assert!(!"SCHEMA-88".chars().all(|c| c.is_ascii_digit()));
        assert!(!"abc".chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn empty_string_is_numeric_but_rejected_by_resolve() {
        // Empty string passes chars().all() vacuously, but resolve_object_key
        // has an explicit empty check that rejects it before the numeric check.
        assert!("".chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn aql_escaping() {
        let input = r#"OBJ-1" OR objectType = "Server"#;
        let escaped = input.replace('\\', "\\\\").replace('"', "\\\"");
        let query = format!("objectKey = \"{}\"", escaped);
        assert_eq!(query, r#"objectKey = "OBJ-1\" OR objectType = \"Server""#);
    }
}
