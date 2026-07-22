//! JSM attachment API — S-576-5 stubs.
//!
//! Two-step servicedeskapi upload flow (BC-3.9.003):
//!   Step 1: `attach_temporary_file` — POST .../attachTemporaryFile → temporaryAttachmentId
//!   Step 2: `post_request_attachment` — POST .../request/{key}/attachment → uploaded objects
//!
//! SEC-576-006 stale-ID self-heal is implemented in the CLI handler
//! (`handle_attachment_upload_jsm`) by catching 404/403 from `attach_temporary_file`,
//! calling `invalidate_project_meta_cache`, re-resolving the sdId via
//! `resolve_service_desk_id`, and retrying ONCE.
//!
//! **RED-phase stubs (S-576-5):** bodies are `todo!()` — all tests in
//! `tests/attachment_jsm.rs` fail until Task 2 implementation replaces these.

#![allow(dead_code)] // RED-phase stub — remove when implementer wires these in Task 5.

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::jira::attachments::AttachmentObject;

/// POST `/rest/servicedeskapi/servicedesk/{sd_id}/attachTemporaryFile`
///
/// Uploads one file as a temporary attachment and returns its `temporaryAttachmentId`.
/// `X-Atlassian-Token: no-check` is MANDATORY (SEC-576-005 parallel — same as platform POST).
///
/// On 404 or 403: returns a distinguishable error so the CLI handler can fire
/// SEC-576-006 stale-ID self-heal (invalidate cache + retry once with fresh sdId).
/// On any other error: propagates as-is.
///
/// Multiple files → call this once per file; collect all `temporaryAttachmentId`s,
/// then pass the full list to `post_request_attachment` (BC-3.9.003 EC-3.9.003-3).
///
/// **RED-phase stub — implement in Task 2.**
pub async fn attach_temporary_file(
    _client: &JiraClient,
    _sd_id: &str,
    _path: &std::path::Path,
) -> Result<String> {
    todo!(
        "S-576-5 RED stub: attach_temporary_file — implement in Task 2 (step 1 POST attachTemporaryFile)"
    )
}

/// POST `/rest/servicedeskapi/request/{issue_key}/attachment`
///
/// Publishes temporary attachment IDs to the JSM request, controlling customer
/// visibility via the `"public"` boolean.  Returns the curated attachment objects
/// from the server response (P2-3c deferred probe: final shape confirmed by
/// S5 implementer against EJ instance before PR close — BC-3.9.007).
///
/// Step-2 failure taxonomy (BC-3.9.006):
/// - 4xx (excl. 401/403) → exit 64 + retry hint
/// - 401 → exit 2 + auth hint + retry hint
/// - 403 → exit 1 + Jira error body + retry hint
/// - 5xx / network → exit 1 + retry hint
///
/// Retry hint text (canonical, BC-3.9.006):
/// `"Temporary attachment IDs may have expired. Try the upload again."`
///
/// **RED-phase stub — implement in Task 2.**
pub async fn post_request_attachment(
    _client: &JiraClient,
    _issue_key: &str,
    _tmp_ids: &[String],
    _public: bool,
) -> Result<Vec<AttachmentObject>> {
    todo!(
        "S-576-5 RED stub: post_request_attachment — implement in Task 2 (step 2 POST .../request/ISSUEKEY/attachment)"
    )
}
