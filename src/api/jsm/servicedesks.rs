use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::ServiceDeskPage;
use crate::cache::{self, ProjectMeta};
use crate::error::JrError;
use crate::types::jsm::ServiceDesk;
use chrono::Utc;

impl JiraClient {
    /// List all service desks, auto-paginating.
    pub async fn list_service_desks(&self) -> Result<Vec<ServiceDesk>> {
        let mut all = Vec::new();
        let mut start = 0u32;
        let page_size = 50u32;

        loop {
            let path = format!(
                "/rest/servicedeskapi/servicedesk?start={}&limit={}",
                start, page_size
            );
            let page: ServiceDeskPage<ServiceDesk> = self.get_from_instance(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values);
            if !has_more {
                break;
            }
            start = next;
        }
        Ok(all)
    }
}

/// Fetch project metadata, using cache when available.
///
/// 1. Check cache for project_key — return if fresh.
/// 2. GET /rest/api/3/project/{key} — extract projectTypeKey, simplified, id.
/// 3. If service_desk: list service desks, match by projectId to find serviceDeskId.
/// 4. Write to cache and return.
pub async fn get_or_fetch_project_meta(
    client: &JiraClient,
    project_key: &str,
) -> Result<ProjectMeta> {
    // Check cache first.
    // Profile threading lands in Task 7 (JiraClient consumes active profile);
    // until then, use the "default" profile literal as a stopgap.
    if let Some(cached) = cache::read_project_meta("default", project_key)? {
        return Ok(cached);
    }

    // Fetch project details from platform API
    let project: serde_json::Value = client
        .get(&format!(
            "/rest/api/3/project/{}",
            urlencoding::encode(project_key)
        ))
        .await?;

    let project_type = project
        .get("projectTypeKey")
        .and_then(|v| v.as_str())
        .unwrap_or("software")
        .to_string();

    let simplified = project
        .get("simplified")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let project_id = project
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // If it's a service desk, resolve the serviceDeskId
    let service_desk_id = if project_type == "service_desk" {
        let desks = client.list_service_desks().await?;
        desks
            .iter()
            .find(|d| d.project_id == project_id)
            .map(|d| d.id.clone())
    } else {
        None
    };

    let meta = ProjectMeta {
        project_type,
        simplified,
        project_id,
        service_desk_id,
        fetched_at: Utc::now(),
    };

    // Write to cache (best-effort — don't fail the command if cache write fails)
    let _ = cache::write_project_meta("default", project_key, &meta);

    Ok(meta)
}

/// Require the project to be a JSM service desk. Returns the serviceDeskId or errors.
pub async fn require_service_desk(client: &JiraClient, project_key: &str) -> Result<String> {
    let meta = get_or_fetch_project_meta(client, project_key).await?;

    if meta.project_type != "service_desk" {
        let type_label = match meta.project_type.as_str() {
            "software" => "Jira Software",
            "business" => "Jira Work Management",
            _ => "Jira",
        };
        return Err(JrError::UserError(format!(
            "\"{}\" is a {} project. Queue commands require a Jira Service Management project. \
             Run \"jr project fields --project {}\" to see available commands.",
            project_key, type_label, project_key
        ))
        .into());
    }

    meta.service_desk_id.ok_or_else(|| {
        JrError::UserError(format!(
            "No service desk found for project \"{}\". \
             The project may not be configured as a service desk.",
            project_key
        ))
        .into()
    })
}
