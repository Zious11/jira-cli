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

    /// Upload one or more files as attachments to a Jira issue (S-576-3; BC-3.9.001).
    ///
    /// Issues `POST /rest/api/3/issue/{key}/attachments` with `Content-Type:
    /// multipart/form-data`. All files are included as separate `"file"`-named parts
    /// in a single request (EC-3.9.001-2 — one POST regardless of file count).
    ///
    /// **MANDATORY:** `X-Atlassian-Token: no-check` header on every request (BC-3.9.001
    /// invariant — Jira XSRF protection returns an XSRF-related rejection without it,
    /// regardless of authentication method; ADR-0017).
    ///
    /// **ADR-0017 retry constraint:** `Request::try_clone()` returns `None` for multipart
    /// bodies. Retry MUST rebuild a fresh `tokio::fs::File::open` and a new
    /// `reqwest::multipart::Form`; do NOT attempt to clone the original request.
    ///
    /// Error mapping (BC-3.9.012):
    /// - 404 → `JrError::UserError` (exit 64): issue not found.
    /// - 413 → `JrError::ApiError { status: 413 }` (exit 1): file exceeds server-configured limit.
    /// - 401 (scope mismatch) → `JrError::InsufficientScope` (exit 2): body contains "scope does
    ///   not match"; handled inline — NOT delegated to the client retry layer.
    /// - 401 (other) → `JrError::NotAuthenticated` (exit 2): handled inline with login hint.
    /// - 403 → `JrError::ApiError { status: 403 }` (exit 1): permission denied.
    /// - 429 (Retry-After ≤ cap) → retry; 429 (Retry-After > cap) → `JrError::ApiError
    ///   { status: 429 }` (exit 1): rate-limit cap exceeded.
    /// - 5xx / network → `JrError::ApiError` / `JrError::NetworkError` (exit 1).
    pub async fn upload_attachments(
        &self,
        key: &str,
        file_paths: &[std::path::PathBuf],
    ) -> Result<Vec<AttachmentObject>> {
        use crate::api::rate_limit::{MAX_RETRY_AFTER_SECS, RateLimitInfo};
        use reqwest::StatusCode;
        use tokio_util::io::ReaderStream;

        const MAX_RETRIES: u32 = 3;
        const DEFAULT_RETRY_SECS: u64 = 1;
        let url = format!("{}/rest/api/3/issue/{}/attachments", self.base_url(), key);

        for attempt in 0..=MAX_RETRIES {
            // ADR-0017: multipart bodies cannot be cloned; rebuild from fresh file
            // handles on every retry attempt (Request::try_clone() returns None).
            let mut form = reqwest::multipart::Form::new();
            for path in file_paths {
                let raw_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());
                // SEC-576-004: strip CR, LF, and NUL from the filename before
                // embedding in a Content-Disposition header value to prevent
                // header-injection (CWE-93). These three chars are the only ones
                // that can break MIME boundary framing or inject a new header line.
                let safe_name: String = raw_name
                    .chars()
                    .map(|c| {
                        if matches!(c, '\r' | '\n' | '\0') {
                            '_'
                        } else {
                            c
                        }
                    })
                    .collect();
                let file = tokio::fs::File::open(path).await.map_err(|_| {
                    JrError::UserError(format!("file not found: {}", path.display()))
                })?;
                let body = reqwest::Body::wrap_stream(ReaderStream::new(file));
                form = form.part(
                    "file",
                    reqwest::multipart::Part::stream(body).file_name(safe_name),
                );
            }

            let response = self
                .reqwest_client()
                .post(&url)
                .header("Authorization", self.authorization_header())
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

            // 429: rebuild request and retry (ADR-0017 — try_clone returns None for multipart).
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

            if status == StatusCode::PAYLOAD_TOO_LARGE {
                return Err(JrError::ApiError {
                    status: 413,
                    message: "Attachment too large: the file exceeds the server-configured limit."
                        .to_string(),
                }
                .into());
            }
            if status == StatusCode::NOT_FOUND {
                return Err(JrError::UserError(format!(
                    "Issue {key} not found or not accessible."
                ))
                .into());
            }
            if status == StatusCode::UNAUTHORIZED {
                let body = response.bytes().await.unwrap_or_default();
                let msg = String::from_utf8_lossy(&body).to_string();
                if msg.to_ascii_lowercase().contains("scope does not match") {
                    return Err(JrError::InsufficientScope {
                        message: msg,
                        required_scope: None,
                    }
                    .into());
                }
                return Err(JrError::NotAuthenticated {
                    hint: "Run \"jr auth login\" to connect.".to_string(),
                }
                .into());
            }
            if !status.is_success() {
                let status_u16 = status.as_u16();
                let body = response.bytes().await.unwrap_or_default();
                return Err(JrError::ApiError {
                    status: status_u16,
                    message: String::from_utf8_lossy(&body).to_string(),
                }
                .into());
            }

            let bytes = response.bytes().await?;
            return Ok(serde_json::from_slice(&bytes)?);
        }

        // Unreachable: every loop iteration either returns or continues; the
        // 429 path cannot continue past attempt == MAX_RETRIES because
        // `attempt < MAX_RETRIES` is false and the code falls through to the
        // `!status.is_success()` return above.
        unreachable!("upload retry loop must return before exhausting iterations");
    }

    /// Delete a single attachment by ID (S-576-3; BC-3.9.017 / VP-576-003).
    ///
    /// Issues `DELETE /rest/api/3/attachment/{id}`.
    ///
    /// Used by `--replace-existing` to remove all same-filename attachments before
    /// re-uploading (JRACLOUD-96384: multiple same-filename attachments may coexist;
    /// ALL are deleted). VP-576-003: all DELETEs MUST complete before any POST upload
    /// begins — ordering is enforced by the caller (`replace_existing_attachments`).
    ///
    /// Error mapping:
    /// - 204 → success (`Ok(())`).
    /// - 403 → `JrError::ApiError { status: 403 }` (exit 1): permission denied.
    /// - 404 → `JrError::UserError` (exit 64): attachment not found or already deleted.
    /// - 5xx / network → `JrError::ApiError` / `JrError::NetworkError` (exit 1).
    pub async fn delete_attachment(&self, attachment_id: &str) -> Result<()> {
        let path = format!("/rest/api/3/attachment/{}", attachment_id);
        let result = self.delete(&path).await;
        match result {
            Ok(()) => Ok(()),
            Err(e) => match e.downcast_ref::<JrError>() {
                Some(JrError::ApiError { status, .. }) if *status == 404 => {
                    Err(JrError::UserError(format!(
                        "Attachment {attachment_id} not found or already deleted."
                    ))
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

#[cfg(test)]
mod tests {
    // SEC-576-004 / CWE-93 unit pins for the safe_name transformation guard.
    //
    // The guard lives inline in `upload_attachments` (~line 211):
    //   raw_name.chars().map(|c| if matches!(c, '\r' | '\n' | '\0') { '_' } else { c }).collect()
    //
    // Mirrored here as a free function so the transformation is testable without
    // spawning a subprocess or needing a real file on disk.
    fn safe_name(raw: &str) -> String {
        raw.chars()
            .map(|c| {
                if matches!(c, '\r' | '\n' | '\0') {
                    '_'
                } else {
                    c
                }
            })
            .collect()
    }

    /// CR (\r) and LF (\n) are each independently mapped to '_'.
    ///
    /// This prevents header-line injection via Content-Disposition (CWE-93).
    /// A filename like "file\r\nX-Injected: hdr" would otherwise split the MIME
    /// header into two lines, injecting an arbitrary header field.
    #[test]
    fn test_sec_576_004_safe_name_crlf_mapped_to_underscore() {
        // Both \r and \n in one filename → two underscores
        assert_eq!(
            safe_name("file\r\nX-Injected: hdr"),
            "file__X-Injected: hdr"
        );
        // Lone \r → single underscore
        assert_eq!(safe_name("only\r"), "only_");
        // Lone \n → single underscore
        assert_eq!(safe_name("only\n"), "only_");
        // Multiple consecutive newlines
        assert_eq!(safe_name("a\n\nb"), "a__b");
    }

    /// NUL byte (\0) is mapped to '_'.
    ///
    /// NUL in a Content-Disposition filename can truncate the value in
    /// C-string-based parsers, silently dropping the rest of the name.
    #[test]
    fn test_sec_576_004_safe_name_nul_mapped_to_underscore() {
        assert_eq!(safe_name("fi\0le"), "fi_le");
        assert_eq!(safe_name("\0"), "_");
        assert_eq!(safe_name("a\0b\0c"), "a_b_c");
    }

    /// Double-quote ('"') is NOT sanitized by safe_name — it passes through unchanged.
    ///
    /// SPEC vs IMPL NOTE:
    /// AC-018 lists double-quote as a potential injection concern for the
    /// Content-Disposition `filename=` quoted-string parameter. The current guard
    /// covers only \r, \n, \0 (SEC-576-004 comment: "only chars that can break MIME
    /// boundary framing or inject a new header line"). A raw '"' in the filename
    /// affects only the quoted-string boundary, which reqwest's
    /// `Part::file_name()` encoding is responsible for handling (percent-encoding
    /// or `filename*` RFC 5987 form). If reqwest emits a raw '"' into
    /// Content-Disposition without escaping, a filename like `a"b.txt` could
    /// break the quoted-string parameter; this is a residual risk tracked at
    /// AC-018 / SEC-576-004.
    ///
    /// This test pins the CURRENT behavior (pass-through) as a regression
    /// anchor. If safe_name is ever extended to cover '"', update this test
    /// and the integration test in tests/attachment_upload.rs.
    #[test]
    fn test_sec_576_004_safe_name_double_quote_not_in_guard() {
        // '"' is NOT mapped; passes through unchanged.
        assert_eq!(safe_name("file\"name.txt"), "file\"name.txt");
        assert_eq!(safe_name("\"leading"), "\"leading");
        assert_eq!(safe_name("trailing\""), "trailing\"");
        assert_eq!(safe_name("mid\"dle"), "mid\"dle");
    }

    /// Benign filenames are unmodified (regression guard — safe_name must not
    /// corrupt valid filenames).
    #[test]
    fn test_sec_576_004_safe_name_normal_filenames_unchanged() {
        assert_eq!(safe_name("report.pdf"), "report.pdf");
        assert_eq!(safe_name("file;name.txt"), "file;name.txt");
        assert_eq!(
            safe_name("file name with spaces.doc"),
            "file name with spaces.doc"
        );
        assert_eq!(safe_name(""), "");
        assert_eq!(safe_name("ascii_only-123.tar.gz"), "ascii_only-123.tar.gz");
    }
}
