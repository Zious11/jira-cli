//! Attachment API: list and download Jira issue attachments.
//!
//! S-576-1: `list_attachments` — `GET /rest/api/3/issue/{key}?fields=attachment`.
//!
//! S-576-2: `get_attachment_metadata` + `get_attachment_content` — two-step
//! streaming download (BC-2.7.007).
//! - Step 1: `GET /rest/api/3/attachment/{id}` (metadata; partial-struct-tolerant).
//! - Step 2: `GET /rest/api/3/attachment/content/{id}` (content; redirect-following;
//!   MUST NOT use `?redirect=false` — JRACLOUD-97046; Authorization stripped on
//!   cross-host redirect — GHSA-9857-6MW7-FQ2M; this is CORRECT CDN behavior).

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::api::client::JiraClient;
use crate::error::JrError;

/// A Jira attachment object as returned in `fields.attachment[]`.
///
/// All fields are `pub` — `tests/attachment_upload.rs::test_vp_576_004_*`
/// constructs fixtures directly for VP-576-004 cross-path shape verification (P74-001).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentObject {
    /// REST API self-link for this attachment.
    #[serde(rename = "self")]
    pub self_url: String,

    /// Attachment ID.
    pub id: String,

    /// Original filename as stored by Jira.
    pub filename: String,

    /// Author of the upload. May be `null` when the account no longer exists.
    pub author: Option<serde_json::Value>,

    /// ISO 8601 timestamp when the attachment was uploaded.
    pub created: String,

    /// File size in bytes.
    pub size: u64,

    /// MIME type reported by Jira.
    /// `None` when the field is absent in the API response (sparse object tolerance).
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,

    /// Download URL for the attachment content (`content` in the Jira API;
    /// renamed to `contentUrl` in the curated JSON shape per BC-2.7.002 / #585).
    #[serde(rename = "content")]
    pub content: String,
}

/// Wire-format response for `GET /rest/api/3/issue/{key}?fields=attachment`.
#[derive(Deserialize)]
struct IssueAttachmentResponse {
    fields: IssueAttachmentFields,
}

#[derive(Deserialize)]
struct IssueAttachmentFields {
    #[serde(default)]
    attachment: Vec<AttachmentObject>,
}

// ---------------------------------------------------------------------------
// S-576-2: metadata response type
// ---------------------------------------------------------------------------

/// Wire-format response for `GET /rest/api/3/attachment/{id}`.
///
/// All content fields are `Option` for partial-struct tolerance (P26-003):
/// the Jira attachment metadata endpoint may omit fields for deleted or
/// restricted attachments; missing fields MUST NOT abort the download.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AttachmentMetadata {
    /// Attachment ID (always present in practice).
    pub id: String,

    /// Original filename as stored by Jira.
    pub filename: Option<String>,

    /// File size in bytes as reported by Jira metadata.
    /// NOT authoritative for the download manifest — use bytes-written (P31-002).
    pub size: Option<u64>,

    /// MIME type reported by Jira.
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,

    /// Download URL (the CDN redirect target that reqwest follows automatically).
    pub content: Option<String>,
}

// ---------------------------------------------------------------------------
// S-576-2: JiraClient impl extensions
// ---------------------------------------------------------------------------

impl JiraClient {
    /// List attachments on a Jira issue.
    ///
    /// Issues `GET /rest/api/3/issue/{key}?fields=attachment`.
    ///
    /// Error mapping (BC-2.7.006):
    /// - 404 → `JrError::UserError` (exit 64): issue not found.
    /// - 401 → `JrError::NotAuthenticated` (exit 2): handled by client.
    /// - 403 → `JrError::ApiError { status: 403 }` (exit 1): permission denied.
    /// - 5xx → `JrError::ApiError` (exit 1): handled by client.
    /// - network error → `JrError::NetworkError` (exit 1): handled by client.
    pub async fn list_attachments(&self, key: &str) -> Result<Vec<AttachmentObject>> {
        let path = format!("/rest/api/3/issue/{}?fields=attachment", key);
        let result = self.get::<IssueAttachmentResponse>(&path).await;
        match result {
            Ok(resp) => Ok(resp.fields.attachment),
            Err(e) => match e.downcast_ref::<JrError>() {
                Some(JrError::ApiError { status, .. }) if *status == 404 => Err(
                    JrError::UserError(format!("Issue {key} not found or not accessible.")).into(),
                ),
                Some(JrError::ApiError { status, .. }) if *status == 403 => {
                    Err(JrError::ApiError {
                        status: 403,
                        message: format!("Permission denied: cannot access issue {key}."),
                    }
                    .into())
                }
                _ => Err(e),
            },
        }
    }

    /// Fetch metadata for a single attachment (BC-2.7.007 step 1).
    ///
    /// Issues `GET /rest/api/3/attachment/{id}`.
    ///
    /// Partial-struct tolerance per P26-003: `AttachmentMetadata` fields are
    /// all `Option` — missing fields (e.g., for deleted attachments) do NOT abort.
    ///
    /// Error mapping (BC-2.7.012):
    /// - 404 → `JrError::UserError` (exit 64): attachment not found.
    /// - 401 → `JrError::NotAuthenticated` (exit 2): handled by client.
    /// - 403 → `JrError::ApiError` (exit 1): `"Permission denied: cannot access attachment <id>."`.
    /// - 5xx / network → `JrError::ApiError` / `JrError::NetworkError` (exit 1).
    pub async fn get_attachment_metadata(&self, id: &str) -> Result<AttachmentMetadata> {
        let path = format!("/rest/api/3/attachment/{}", id);
        let result = self.get::<AttachmentMetadata>(&path).await;
        match result {
            Ok(meta) => Ok(meta),
            Err(e) => match e.downcast_ref::<JrError>() {
                Some(JrError::ApiError { status, .. }) if *status == 404 => Err(
                    JrError::UserError(format!("Attachment {id} not found or not accessible."))
                        .into(),
                ),
                Some(JrError::ApiError { status, .. }) if *status == 403 => {
                    Err(JrError::ApiError {
                        status: 403,
                        message: format!("Permission denied: cannot access attachment {id}."),
                    }
                    .into())
                }
                _ => Err(e),
            },
        }
    }

    /// Stream attachment binary content (BC-2.7.007 step 2).
    ///
    /// Issues `GET /rest/api/3/attachment/content/{id}`.
    ///
    /// **MUST NOT** append `?redirect=false` — JRACLOUD-97046 (breaks some file formats).
    /// Follows 302/303 redirects via reqwest's default redirect policy.
    /// `Authorization` and `Cookie` headers are stripped by reqwest on cross-host
    /// redirect (GHSA-9857-6MW7-FQ2M) — this is CORRECT CDN behavior; do NOT fight it.
    ///
    /// The caller is responsible for streaming `response.bytes_stream()` to disk
    /// (ADR-0017 reqwest `stream` feature).
    ///
    /// Error mapping (BC-2.7.012):
    /// - 401 → `JrError::NotAuthenticated` (exit 2).
    /// - 403 → `JrError::ApiError` (exit 1): permission denied.
    /// - 404 → `JrError::ApiError` (exit 1): attachment not found (AC-009: this
    ///   endpoint does NOT remap 404→UserError; raw API status propagates).
    /// - 5xx / network → `JrError::ApiError` / `JrError::NetworkError` (exit 1).
    pub async fn get_attachment_content(&self, id: &str) -> Result<reqwest::Response> {
        // ALWAYS use the platform URL (EC-2.7.007-2). MUST NOT use metadata.content —
        // that field may be a servicedeskapi URL (JSDCLOUD-10841).
        // MUST NOT append ?redirect=false (JRACLOUD-97046).
        let path = format!("/rest/api/3/attachment/content/{}", id);
        self.get_raw_response(&path).await
    }
}
