//! JSM request-submission API client methods.
//!
//! Effectful module (L4) — HTTP calls via `api::client`. No business logic.
//! The single method wraps `POST /rest/servicedeskapi/request`.

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::types::jsm::JsmRequestCreated;

impl JiraClient {
    /// Submit a new JSM customer request.
    ///
    /// POSTs `body` to `/rest/servicedeskapi/request` and deserializes the
    /// HTTP 201 response into [`JsmRequestCreated`].
    ///
    /// Traces: BC-3.8.001
    pub async fn create_jsm_request(&self, body: serde_json::Value) -> Result<JsmRequestCreated> {
        self.post_to_instance("/rest/servicedeskapi/request", &body)
            .await
    }
}
