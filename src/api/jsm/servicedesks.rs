use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::ServiceDeskPage;
use crate::cache::{self, ProjectMeta};
use crate::error::{API_TOKEN_EXPIRY_HINT, JrError};
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
    let profile = client.profile_name();
    if let Some(cached) = cache::read_project_meta(profile, project_key)? {
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
    let _ = cache::write_project_meta(profile, project_key, &meta);

    Ok(meta)
}

/// Require the project to be a JSM service desk. Returns the serviceDeskId or errors.
///
/// `call_site_label` is embedded in the error message to produce caller-specific
/// error text per BC-X.8.004 + BC-X.12.003. The label MUST be a full noun-phrase
/// ending in the matching verb (e.g., "Queue commands (`jr queue`) require" or
/// "`jr requesttype` commands require"), because the template drops the verb to
/// allow correct number agreement for plural vs singular subjects.
///
/// Error message template (verbatim per BC-X.8.004 + BC-X.12.003):
/// `Project "<KEY>" is a <TYPE> project. <LABEL> a Jira Service Management project.
///  Run "jr project list" to find a JSM project.`
pub async fn require_service_desk(
    client: &JiraClient,
    project_key: &str,
    call_site_label: &'static str,
) -> Result<String> {
    // Introduce a new map_err on get_or_fetch_project_meta to provide auth-aware
    // 401 hints (BC-X.8.006 / BC-X.8.007). The map_err wraps the ENTIRE future so
    // it catches 401 from either live GET (project GET or service-desk list GET)
    // issued on a cache miss.
    //
    // Only rewrite 401-derived errors (NotAuthenticated / InsufficientScope);
    // all other error variants pass through unchanged.
    let meta = get_or_fetch_project_meta(client, project_key)
        .await
        .map_err(|e| match e.downcast::<JrError>() {
            Ok(JrError::NotAuthenticated { .. }) | Ok(JrError::InsufficientScope { .. }) => {
                // Auth-conditional hint dispatch (BC-X.8.006 / BC-X.8.007).
                let hint = if client.is_oauth_auth() {
                    // OAuth: read-side scope hint naming read:jira-work and
                    // read:servicedesk-request. Both arms produce the same hint
                    // (BC-X.8.007 postcondition 1 / pass-3 H-04 one canonical hint).
                    // NOT InsufficientScope — that template is POST-specific (#185).
                    "Your OAuth token may be expired. Run `jr auth refresh` to renew the token, or\n\
                    `jr auth login` to re-authorize. If using a custom OAuth app, run `jr auth login`\n\
                    to re-consent with read:jira-work and read:servicedesk-request — `jr auth refresh`\n\
                    alone cannot add missing scopes (it re-mints with the same granted scope set)."
                        .to_string()
                } else {
                    // Basic: API-token-expiry hint (BC-X.8.006 postcondition 1).
                    API_TOKEN_EXPIRY_HINT.to_string()
                };
                anyhow::anyhow!(JrError::NotAuthenticated { hint })
            }
            Ok(other) => anyhow::anyhow!(other),
            Err(other) => other,
        })?;

    if meta.project_type != "service_desk" {
        let type_label = match meta.project_type.as_str() {
            "software" => "Jira Software",
            "business" => "Jira Work Management",
            _ => "Jira",
        };
        return Err(JrError::UserError(format!(
            "Project \"{}\" is a {} project. {} a Jira Service Management project. \
             Run \"jr project list\" to find a JSM project.",
            project_key, type_label, call_site_label
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
