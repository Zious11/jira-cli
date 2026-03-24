use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::AssetsPage;
use crate::error::JrError;
use crate::types::assets::AssetObject;

impl JiraClient {
    /// Search assets via AQL with auto-pagination.
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
}

/// Resolve an object key (e.g., "OBJ-1") to its numeric ID.
/// If the input is purely numeric, returns it as-is.
pub async fn resolve_object_key(
    client: &JiraClient,
    workspace_id: &str,
    key_or_id: &str,
) -> Result<String> {
    if key_or_id.chars().all(|c| c.is_ascii_digit()) {
        return Ok(key_or_id.to_string());
    }

    let results = client
        .search_assets(
            workspace_id,
            &format!("objectKey = \"{}\"", key_or_id),
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
    fn empty_string_is_numeric() {
        assert!("".chars().all(|c| c.is_ascii_digit()));
    }
}
