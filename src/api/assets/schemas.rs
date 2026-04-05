use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::AssetsPage;
use crate::types::assets::{ObjectSchema, ObjectTypeEntry};

impl JiraClient {
    /// List all object schemas in the workspace with auto-pagination.
    pub async fn list_object_schemas(&self, workspace_id: &str) -> Result<Vec<ObjectSchema>> {
        let mut all = Vec::new();
        let mut start_at = 0u32;
        let page_size = 25u32;

        loop {
            let path = format!(
                "objectschema/list?startAt={}&maxResults={}&includeCounts=true",
                start_at, page_size
            );
            let page: AssetsPage<ObjectSchema> = self.get_assets(workspace_id, &path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values);

            if !has_more {
                break;
            }
            start_at = next;
        }
        Ok(all)
    }

    /// List all object types for a given schema (flat, no pagination).
    pub async fn list_object_types(
        &self,
        workspace_id: &str,
        schema_id: &str,
    ) -> Result<Vec<ObjectTypeEntry>> {
        let path = format!(
            "objectschema/{}/objecttypes/flat?includeObjectCounts=true",
            urlencoding::encode(schema_id)
        );
        self.get_assets(workspace_id, &path).await
    }
}
