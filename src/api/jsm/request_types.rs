//! JSM request-type discovery API client methods.
//!
//! Effectful module (L4) — HTTP calls via `api::client` + `api::pagination`.
//! No business logic; no cache access (cache wiring is pr2's scope).
//!
//! Pagination uses [`ServiceDeskPage`] (`isLastPage` pattern), mirroring
//! `api/jsm/queues.rs::list_queues`. Do NOT use USER_PAGE_SIZE fixed-window
//! advance here — JSM list endpoints do not exhibit JRACLOUD-71293.

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::ServiceDeskPage;
use crate::types::jsm::{RequestType, RequestTypeFieldsResponse};

impl JiraClient {
    /// List all request types for a service desk, auto-paginating.
    ///
    /// Optionally filters by `search_query` (forwarded as `searchQuery=<val>`).
    ///
    /// Traces: BC-X.12.001
    pub async fn list_request_types(
        &self,
        service_desk_id: &str,
        search_query: Option<&str>,
    ) -> Result<Vec<RequestType>> {
        let base = format!(
            "/rest/servicedeskapi/servicedesk/{}/requesttype",
            urlencoding::encode(service_desk_id)
        );
        let mut all = Vec::new();
        let mut start = 0u32;
        let page_size = 50u32;

        loop {
            let path = match search_query {
                Some(q) => format!(
                    "{}?start={}&limit={}&searchQuery={}",
                    base,
                    start,
                    page_size,
                    urlencoding::encode(q)
                ),
                None => format!("{}?start={}&limit={}", base, start, page_size),
            };
            let page: ServiceDeskPage<RequestType> = self.get_from_instance(&path).await?;
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

    /// Fetch the field schema for a specific request type.
    ///
    /// GETs `/rest/servicedeskapi/servicedesk/{id}/requesttype/{rtId}/field`.
    ///
    /// Traces: BC-X.12.005
    pub async fn get_request_type_fields(
        &self,
        service_desk_id: &str,
        request_type_id: &str,
    ) -> Result<RequestTypeFieldsResponse> {
        let path = format!(
            "/rest/servicedeskapi/servicedesk/{}/requesttype/{}/field",
            urlencoding::encode(service_desk_id),
            urlencoding::encode(request_type_id)
        );
        self.get_from_instance(&path).await
    }
}
