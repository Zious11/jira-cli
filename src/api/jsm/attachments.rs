//! JSM attachment API — two-step servicedeskapi upload flow.
//!
//! **Step 1:** `attach_temporary_file` — POST
//!   `.../servicedesk/{sdId}/attachTemporaryFile` with multipart form data.
//!   Returns the `temporaryAttachmentId` for the file.  Must be called once
//!   per file; collect all IDs before calling step 2.
//!
//! **Step 2:** `post_request_attachment` — POST
//!   `.../request/{issueKey}/attachment` with
//!   `{"temporaryAttachmentIds":[…],"public":<bool>}`.  Returns the curated
//!   `AttachmentObject` list.
//!
//! **SEC-576-006 stale-ID self-heal:** `attach_temporary_file` returns a
//! distinguishable `JrError::ApiError { status: 404 | 403 }` on those HTTP
//! codes so the CLI handler (`handle_attachment_upload_jsm`) can call
//! `invalidate_project_meta_cache`, re-resolve the sdId, and retry ONCE.
//!
//! **ADR-0017 / SEC-576-005:** multipart bodies cannot be cloned; the retry
//! loop rebuilds the `Form` from scratch on every attempt.
//! `X-Atlassian-Token: no-check` is mandatory on every `attachTemporaryFile`
//! POST (mirrors the platform upload constraint).

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::jira::attachments::AttachmentObject;
use crate::api::rate_limit::{MAX_RETRY_AFTER_SECS, RateLimitInfo};
use crate::error::JrError;

/// POST `/rest/servicedeskapi/servicedesk/{sd_id}/attachTemporaryFile`
///
/// Uploads one file as a temporary attachment and returns its
/// `temporaryAttachmentId`.  `X-Atlassian-Token: no-check` is MANDATORY
/// (SEC-576-005 — parallel to the platform upload constraint).
///
/// On 404 or 403 the function returns `JrError::ApiError { status }` so the
/// CLI handler can fire the SEC-576-006 stale-ID self-heal (invalidate cache +
/// retry once with fresh sdId).  Any other error is propagated as-is.
///
/// Multiple files → call this once per file; collect all
/// `temporaryAttachmentId`s, then pass the full slice to
/// `post_request_attachment` (BC-3.9.003 EC-3.9.003-3).
pub async fn attach_temporary_file(
    client: &JiraClient,
    sd_id: &str,
    path: &std::path::Path,
) -> Result<String> {
    use reqwest::StatusCode;
    use tokio_util::io::ReaderStream;

    const MAX_RETRIES: u32 = 3;
    const DEFAULT_RETRY_SECS: u64 = 1;

    let url = format!(
        "{}/rest/servicedeskapi/servicedesk/{}/attachTemporaryFile",
        client.base_url(),
        sd_id
    );

    // Raw filename from path.
    let raw_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    // SEC-576-004 CRLF/NUL guard — prevent header-injection in Content-Disposition.
    let safe_name: String = raw_name
        .chars()
        .map(|c| if matches!(c, '\r' | '\n' | '\0') { '_' } else { c })
        .collect();

    for attempt in 0..=MAX_RETRIES {
        // ADR-0017: multipart bodies cannot be cloned; rebuild Form on every attempt.
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|_| JrError::UserError(format!("file not found: {}", path.display())))?;
        let body = reqwest::Body::wrap_stream(ReaderStream::new(file));
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::stream(body).file_name(safe_name.clone()),
        );

        let response = client
            .reqwest_client()
            .post(&url)
            .header("Authorization", client.authorization_header())
            .header("X-Atlassian-Token", "no-check")
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                let host = e
                    .url()
                    .and_then(|u| u.host_str().map(str::to_string))
                    .unwrap_or_else(|| "Jira".to_string());
                JrError::NetworkError(host)
            })?;

        let status = response.status();

        // 429: rebuild and retry (ADR-0017 — try_clone returns None for multipart).
        if status == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_RETRIES {
            let rate_info = RateLimitInfo::from_headers(response.headers());
            let delay = rate_info.retry_after_secs.unwrap_or(DEFAULT_RETRY_SECS);
            if delay > MAX_RETRY_AFTER_SECS {
                return Err(JrError::ApiError {
                    status: 429,
                    message: format!(
                        "Rate limited; Retry-After {}s exceeds {}s cap. Rerun later.",
                        delay, MAX_RETRY_AFTER_SECS
                    ),
                }
                .into());
            }
            if delay > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            }
            continue;
        }

        // 404 / 403: return a distinguishable error so the caller can fire the
        // SEC-576-006 stale-ID self-heal (invalidate cache + retry once).
        if matches!(status, StatusCode::NOT_FOUND | StatusCode::FORBIDDEN) {
            let status_u16 = status.as_u16();
            let body_bytes = response.bytes().await.unwrap_or_default();
            return Err(JrError::ApiError {
                status: status_u16,
                message: String::from_utf8_lossy(&body_bytes).to_string(),
            }
            .into());
        }

        if !status.is_success() {
            let status_u16 = status.as_u16();
            let body_bytes = response.bytes().await.unwrap_or_default();
            return Err(JrError::ApiError {
                status: status_u16,
                message: String::from_utf8_lossy(&body_bytes).to_string(),
            }
            .into());
        }

        // Parse temporaryAttachmentId from the response.
        let bytes = response.bytes().await?;
        let resp: serde_json::Value = serde_json::from_slice(&bytes)?;
        let tmp_id = resp
            .get("temporaryAttachments")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|obj| obj.get("temporaryAttachmentId"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                JrError::Internal(
                    "attachTemporaryFile response missing temporaryAttachmentId".to_string(),
                )
            })?
            .to_string();

        return Ok(tmp_id);
    }

    // Unreachable: every loop iteration either returns or continues; the
    // 429 path cannot continue past attempt == MAX_RETRIES.
    unreachable!("attach_temporary_file retry loop must return before exhausting iterations");
}

/// POST `/rest/servicedeskapi/request/{issue_key}/attachment`
///
/// Publishes the collected `temporaryAttachmentIds` to the JSM request,
/// controlling customer visibility via `public: bool`.
/// Returns the curated `AttachmentObject` list from the server response.
///
/// **Step-2 failure taxonomy (BC-3.9.006):**
/// - 401 → `JrError::NotAuthenticated` (exit 2) + retry hint
/// - 403 → `JrError::ApiError { status: 403 }` (exit 1) + retry hint
/// - Other 4xx → `JrError::UserError` (exit 64) + retry hint
/// - 5xx / network → `JrError::ApiError` (exit 1) + retry hint
///
/// Retry hint (canonical, BC-3.9.006):
/// `"Temporary attachment IDs may have expired. Try the upload again."`
pub async fn post_request_attachment(
    client: &JiraClient,
    issue_key: &str,
    tmp_ids: &[String],
    public: bool,
) -> Result<Vec<AttachmentObject>> {
    const RETRY_HINT: &str =
        "Temporary attachment IDs may have expired. Try the upload again.";

    let url = format!(
        "{}/rest/servicedeskapi/request/{}/attachment",
        client.base_url(),
        issue_key
    );

    let body = serde_json::json!({
        "temporaryAttachmentIds": tmp_ids,
        "public": public,
    });

    let response = client
        .reqwest_client()
        .post(&url)
        .header("Authorization", client.authorization_header())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let host = e
                .url()
                .and_then(|u| u.host_str().map(str::to_string))
                .unwrap_or_else(|| "Jira".to_string());
            JrError::NetworkError(host)
        })?;

    let status = response.status();

    if status.is_success() {
        let bytes = response.bytes().await?;
        let attachments: Vec<AttachmentObject> = serde_json::from_slice(&bytes)?;
        return Ok(attachments);
    }

    let status_u16 = status.as_u16();
    let body_bytes = response.bytes().await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body_bytes).to_string();

    // BC-3.9.006 step-2 failure taxonomy + append retry hint.
    let err: JrError = match status_u16 {
        401 => JrError::NotAuthenticated {
            hint: format!(
                "{RETRY_HINT}\nRun \"jr auth login\" to reconnect."
            ),
        },
        403 => JrError::ApiError {
            status: 403,
            message: format!("{body_str}\n{RETRY_HINT}"),
        },
        _ if status.is_client_error() => {
            JrError::UserError(format!("{body_str}\n{RETRY_HINT}"))
        }
        _ => JrError::ApiError {
            status: status_u16,
            message: format!("{body_str}\n{RETRY_HINT}"),
        },
    };
    Err(anyhow::anyhow!(err))
}
