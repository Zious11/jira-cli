//! Attachment API: list attachments on a Jira issue.
//!
//! S-576-1: first-landing story of SOH-ATTACHMENTS-1.
//! `GET /rest/api/3/issue/{key}?fields=attachment` — no separate attachments endpoint exists.

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
    #[serde(rename = "mimeType")]
    pub mime_type: String,

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
}
