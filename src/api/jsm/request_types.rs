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
        unimplemented!(
            "BC-X.12.001: GET /rest/servicedeskapi/servicedesk/{{id}}/requesttype paginated"
        )
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
        unimplemented!("BC-X.12.005: GET .../requesttype/{{id}}/field")
    }
}
