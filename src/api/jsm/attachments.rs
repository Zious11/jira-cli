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

    // SEC-576-004 CRLF/NUL/double-quote/backslash guard — prevent header-injection
    // in Content-Disposition. CR/LF/NUL can inject new header lines; '"' can break
    // the RFC 2616 quoted-string parameter boundary; '\' is the RFC 2616
    // quoted-string escape character — a stray '\' lets a parser misread the next
    // character as an escaped sequence (F5-R1-006, F5-R2-002, CWE-93).
    let safe_name: String = raw_name
        .chars()
        .map(|c| {
            if matches!(c, '\r' | '\n' | '\0' | '"' | '\\') {
                '_'
            } else {
                c
            }
        })
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
            let body_bytes = response.bytes().await.unwrap_or_default();
            // BC-3.9.012: 401 on first occurrence maps to NotAuthenticated (exit 2),
            // consistent with step-2 401 and the platform upload path. No stale-heal
            // retry is applied — stale-heal fires only on 404/403 (checked above).
            if status == StatusCode::UNAUTHORIZED {
                return Err(JrError::NotAuthenticated {
                    hint: "Run `jr auth login` to re-authenticate.".to_string(),
                }
                .into());
            }
            let status_u16 = status.as_u16();
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

/// Defensively curate one entry from the servicedeskapi
/// `AttachmentCreateResultDTO.attachments.values[]` array into an
/// `AttachmentObject`.
///
/// The servicedeskapi `AttachmentDTO` shape differs from the platform
/// `AttachmentObject` (confirmed by P2-3c schema probe run 29940792930):
/// - `created` is an OBJECT `{"iso8601":"…","jira":"…","friendly":"…","epochMillis":N}`,
///   NOT a bare string.
/// - There is NO top-level `id` field — the attachment ID is the last path
///   segment of `_links.jiraRest`.
/// - The download URL lives at `_links.content`, NOT a top-level `content` field.
/// - `author` is a full `UserDTO` object — downstream `serialize_attachment_curated`
///   extracts `accountId` and `displayName` from whatever value is present here.
///
/// All fields have graceful fallbacks (empty string / `None` / 0) so schema
/// drift or a missing field NEVER errors the upload command — the upload
/// succeeded server-side; the echo MUST NOT fail it (BC-3.9.007 intent).
fn curate_jsm_attachment_entry(v: &serde_json::Value) -> AttachmentObject {
    // id: last path segment of _links.jiraRest; fallback top-level "id"; else "".
    let id = v
        .get("_links")
        .and_then(|l| l.get("jiraRest"))
        .and_then(|u| u.as_str())
        .and_then(|url| url.rsplit('/').next())
        .map(str::to_string)
        .or_else(|| v.get("id").and_then(|i| i.as_str()).map(str::to_string))
        .unwrap_or_default();

    // content (→ contentUrl in curated JSON): _links.content; fallback top-level
    // "content" or "contentUrl"; else "".
    let content = v
        .get("_links")
        .and_then(|l| l.get("content"))
        .and_then(|u| u.as_str())
        .or_else(|| v.get("content").and_then(|c| c.as_str()))
        .or_else(|| v.get("contentUrl").and_then(|c| c.as_str()))
        .unwrap_or_default()
        .to_string();

    // created: object → .iso8601; string → direct; else "".
    let created = match v.get("created") {
        Some(serde_json::Value::Object(obj)) => obj
            .get("iso8601")
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => String::new(),
    };

    // filename: direct string; fallback "".
    let filename = v
        .get("filename")
        .and_then(|f| f.as_str())
        .unwrap_or_default()
        .to_string();

    // mimeType: optional.
    let mime_type = v
        .get("mimeType")
        .and_then(|m| m.as_str())
        .map(str::to_string);

    // size: fallback 0.
    let size = v.get("size").and_then(|s| s.as_u64()).unwrap_or(0);

    // author: preserve full UserDTO Value — downstream serialize_attachment_curated
    // extracts accountId + displayName from whatever object is present here
    // (BC-2.7.002 curation layer).  null/absent → None.
    let author = v.get("author").cloned().filter(|a| !a.is_null());

    // self_url: _links.self; fallback "".
    let self_url = v
        .get("_links")
        .and_then(|l| l.get("self"))
        .and_then(|u| u.as_str())
        .unwrap_or_default()
        .to_string();

    AttachmentObject {
        self_url,
        id,
        filename,
        author,
        created,
        size,
        mime_type,
        content,
    }
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
/// - 5xx → `JrError::ApiError` (exit 1) + retry hint
/// - transport/network → `JrError::NetworkError` (exit 1, no retry hint)
///
/// Retry hint (canonical, BC-3.9.006 — HTTP branches only):
/// `"Temporary attachment IDs may have expired. Try the upload again."`
pub async fn post_request_attachment(
    client: &JiraClient,
    issue_key: &str,
    tmp_ids: &[String],
    public: bool,
) -> Result<Vec<AttachmentObject>> {
    const RETRY_HINT: &str = "Temporary attachment IDs may have expired. Try the upload again.";

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
            // F5-R1-007: use canonical NetworkError variant (exit 1), not ApiError{status:0}
            // (status=0 is not a real HTTP code and leaks implementation noise).
            // NetworkError format: "Could not reach {host} — check your connection".
            JrError::NetworkError(host)
        })?;

    let status = response.status();

    if status.is_success() {
        // The real Jira API returns AttachmentCreateResultDTO — an object, not a bare
        // array.  Confirmed by P2-3c schema probe run 29936980027 (S-576-5):
        //   {"comment": {...} | null,
        //    "attachments": {"size": N, "start": N, "limit": N, "isLastPage": bool,
        //                    "values": [...AttachmentDTO...]}}
        //
        // The servicedeskapi AttachmentDTO shape differs from the platform
        // AttachmentObject — confirmed by P2-3c schema probe run 29940792930:
        //   - `created` is an OBJECT {"iso8601":"…","jira":"…","friendly":"…","epochMillis":N}
        //   - NO top-level `id` — extract from `_links.jiraRest` URL tail
        //   - Content URL lives at `_links.content`, not top-level `content`
        //
        // BC-3.9.007 (enforced): the upload succeeded server-side; the echo MUST
        // NOT fail the command.  Parse best-effort: non-JSON body or missing
        // attachments.values → return an empty Vec with a stderr warning.
        let bytes = response.bytes().await.unwrap_or_default();
        let attachments = match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(resp) => match resp
                .get("attachments")
                .and_then(|a| a.get("values"))
                .and_then(|v| v.as_array())
            {
                Some(values) => values.iter().map(curate_jsm_attachment_entry).collect(),
                None => {
                    eprintln!(
                        "warning: step-2 response echo had unrecognized shape \
                         (attachments.values missing); upload succeeded"
                    );
                    vec![]
                }
            },
            Err(_) => {
                eprintln!("warning: step-2 response echo was not valid JSON; upload succeeded");
                vec![]
            }
        };
        return Ok(attachments);
    }

    let status_u16 = status.as_u16();
    let body_bytes = response.bytes().await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body_bytes).to_string();

    // BC-3.9.006 step-2 failure taxonomy + append retry hint.
    let err: JrError = match status_u16 {
        401 => JrError::NotAuthenticated {
            hint: format!("{RETRY_HINT}\nRun \"jr auth login\" to reconnect."),
        },
        403 => JrError::ApiError {
            status: 403,
            message: format!("{body_str}\n{RETRY_HINT}"),
        },
        _ if status.is_client_error() => JrError::UserError(format!("{body_str}\n{RETRY_HINT}")),
        _ => JrError::ApiError {
            status: status_u16,
            message: format!("{body_str}\n{RETRY_HINT}"),
        },
    };
    Err(anyhow::anyhow!(err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::client::JiraClient;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // SEC-576-004 / CWE-93 unit pins for the safe_name transformation guard.
    //
    // The guard lives inline in `attach_temporary_file`:
    //   raw_name.chars().map(|c| if matches!(c, '\r' | '\n' | '\0' | '"' | '\\') { '_' } else { c }).collect()
    //
    // Mirrored here as a free function so the transformation is testable without
    // touching the filesystem (no tokio::fs::File, no actual multipart POST).
    fn safe_name(raw: &str) -> String {
        raw.chars()
            .map(|c| {
                if matches!(c, '\r' | '\n' | '\0' | '"' | '\\') {
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

    /// F5-R1-006 (JSM): double-quote (`"`) in a filename must be mapped to underscore (`_`)
    /// by the SEC-576-004 guard in `attach_temporary_file` to prevent quoted-string
    /// parameter injection in Content-Disposition (CWE-93).
    ///
    /// RED: current guard only maps `\r`, `\n`, `\0` — not `"`.
    /// Target: `"` is also mapped to `_`.
    ///
    /// When this test turns GREEN, also update
    /// `test_sec_576_004_safe_name_double_quote_not_in_guard` (if analogous) and
    /// the JSM integration test in `tests/attachment_jsm.rs`.
    #[test]
    fn test_f5_r1_006_jsm_safe_name_double_quote_mapped_to_underscore() {
        assert_eq!(
            safe_name("file\"name.txt"),
            "file_name.txt",
            "F5-R1-006 JSM: '\"' must be mapped to '_' in SEC-576-004 guard"
        );
        assert_eq!(
            safe_name("a\"b\"c"),
            "a_b_c",
            "F5-R1-006 JSM: multiple '\"' must each become '_'"
        );
    }

    /// F5-R2-002 (JSM): backslash (`\`) in a filename must be mapped to underscore (`_`)
    /// by the SEC-576-004 guard in `attach_temporary_file`.
    ///
    /// A raw `\` in Content-Disposition `filename=` is the RFC 2616 quoted-string
    /// escape character — a stray `\` lets a parser misread the next character as
    /// an escaped sequence (CWE-93). This is the JSM-path twin of
    /// `test_f5_r2_002_safe_name_backslash_mapped_to_underscore` in
    /// `src/api/jira/attachments.rs`.
    ///
    /// GREEN (FIX-F5-007): guard extended to `matches!(c, '\r' | '\n' | '\0' | '"' | '\\')` —
    /// `safe_name("file\\name.txt")` now returns `"file_name.txt"` as required.
    #[test]
    fn test_f5_r2_002_jsm_safe_name_backslash_mapped_to_underscore() {
        assert_eq!(
            safe_name("file\\name.txt"),
            "file_name.txt",
            "F5-R2-002 JSM: '\\' must be mapped to '_' in SEC-576-004 guard"
        );
        assert_eq!(
            safe_name("a\\b\\c"),
            "a_b_c",
            "F5-R2-002 JSM: multiple '\\' must each become '_'"
        );
        // Mixed with other guarded chars.
        assert_eq!(
            safe_name("a\\\0b"),
            "a__b",
            "F5-R2-002 JSM: '\\' and '\\0' each become '_'"
        );
    }

    /// Benign filenames are unmodified (regression guard — safe_name must not
    /// corrupt valid filenames, including names with spaces, dots, and Unicode).
    #[test]
    fn test_sec_576_004_safe_name_benign_filenames_pass_through() {
        assert_eq!(safe_name("report.pdf"), "report.pdf");
        assert_eq!(safe_name("my file (v2).txt"), "my file (v2).txt");
        assert_eq!(safe_name("résumé.docx"), "résumé.docx");
        assert_eq!(safe_name(""), "");
    }

    /// BC-3.9.006: network errors from step-2 (post_request_attachment) exit 1
    /// and produce a connectivity error message (F5-R1-007: uses JrError::NetworkError,
    /// not ApiError{status:0}). RETRY_HINT is no longer embedded in network errors
    /// (it is still present on 4xx/5xx HTTP error branches). NetworkError format:
    /// "Could not reach {host} — check your connection".
    /// Uses 127.0.0.1:1 (established no-listener pattern in this codebase) to
    /// trigger ECONNREFUSED without requiring a real server.
    /// BC-3.9.006 / F5-R1-007: step-2 transport errors use the connectivity message
    /// ("check your connection"), NOT the expired-ID retry hint.  This test was
    /// formerly named `test_bc_3_9_006_step2_network_error_appends_retry_hint`; the
    /// old name was misleading because the retry hint is no longer present on the
    /// network-error path after F5-R1-007.
    #[tokio::test]
    async fn test_bc_3_9_006_step2_network_error_uses_connectivity_message_no_retry_hint() {
        let client = JiraClient::new_for_test(
            "http://127.0.0.1:1".to_string(),
            "Basic dGVzdA==".to_string(),
        );
        let result = post_request_attachment(&client, "EJ-1", &["tmp-123".to_string()], true).await;
        assert!(
            result.is_err(),
            "expected an error when server is unreachable"
        );
        let err_string = format!("{}", result.unwrap_err());
        // F5-R1-007: NetworkError format is "Could not reach {host} — check your connection".
        assert!(
            err_string.contains("check your connection"),
            "BC-3.9.006 (updated F5-R1-007): step-2 network error must mention connectivity; \
             got: {err_string}"
        );
        // Confirm the retry hint ("Temporary attachment IDs may have expired") is absent
        // on the network-error path — it belongs only on HTTP 4xx/5xx branches (F5-R1-007).
        assert!(
            !err_string.contains("may have expired"),
            "BC-3.9.006 (F5-R1-007): retry hint must NOT appear on transport/network errors; \
             got: {err_string}"
        );
    }

    /// F5-R1-007: step-2 network/transport errors from `post_request_attachment`
    /// must use the codebase-canonical `JrError::NetworkError` variant, NOT the
    /// spurious `JrError::ApiError { status: 0 }` (status=0 is not a real HTTP
    /// status; it leaks implementation noise and is inconsistent with how
    /// `upload_attachments` and `attach_temporary_file` handle transport failures).
    ///
    /// The exit code must remain 1 (both `NetworkError` and `ApiError` map to 1
    /// via `JrError::exit_code()`'s catch-all arm).
    ///
    /// Companion to `test_bc_3_9_006_step2_network_error_uses_connectivity_message_no_retry_hint`
    /// which checks the formatted message; this test checks the error variant type.
    #[tokio::test]
    async fn test_f5_r1_007_step2_network_error_uses_canonical_network_error_variant() {
        use crate::error::JrError;

        let client = JiraClient::new_for_test(
            "http://127.0.0.1:1".to_string(),
            "Basic dGVzdA==".to_string(),
        );
        let result = post_request_attachment(&client, "EJ-1", &["tmp-123".to_string()], true).await;
        let err = result.expect_err("network error expected with unreachable server");

        // RED assertion: current code emits "API error (0):" — that must disappear.
        let err_string = format!("{err}");
        assert!(
            !err_string.contains("API error (0):"),
            "F5-R1-007: step-2 network error must NOT use ApiError{{status:0}} pattern \
             (status=0 is not a real HTTP code); got: {err_string}"
        );

        // Positive assertion: error must downcast to JrError::NetworkError.
        let jr_err = err
            .downcast_ref::<JrError>()
            .expect("error must downcast to JrError");
        assert!(
            matches!(jr_err, JrError::NetworkError(_)),
            "F5-R1-007: step-2 transport failure must be JrError::NetworkError, \
             matching the pattern used by upload_attachments and attach_temporary_file; \
             got variant: {jr_err:?}"
        );
    }

    // ---------------------------------------------------------------------------
    // Mutation-gate tests for attach_temporary_file retry loop (S-576-5 mutant kill).
    //
    // These tests cover the three comparison sites that cargo-mutants flagged as
    // missed on PR #640:
    //   line 108 (attempt < MAX_RETRIES)  — A1 + A2
    //   line 111 (delay > MAX_RETRY_AFTER_SECS) — A3 + A4
    //   line 228 (Some(Value::String(s)) arm of curate_jsm_attachment_entry) — B
    //
    // Tests A1/A2/A4 use `start_paused = true` so tokio::time::sleep completes
    // instantly (the real network I/O used by wiremock is unaffected).
    // ---------------------------------------------------------------------------

    /// A1 – line 108: a single 429 followed by a 200 must retry and succeed.
    ///
    /// Kills mutants that prevent retrying on 429 (e.g. `attempt < MAX_RETRIES`
    /// → `attempt > MAX_RETRIES` or `attempt >= MAX_RETRIES`).  With those
    /// mutants the first 429 is NOT retried → error returned → test fails.
    #[tokio::test(start_paused = true)]
    async fn test_attach_tmp_429_then_200_retries_and_succeeds() {
        let server = MockServer::start().await;
        let tmp = tempfile::NamedTempFile::new().unwrap();

        // First POST → 429 (no Retry-After; delay = DEFAULT_RETRY_SECS=1; clock paused → instant)
        Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
            ))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        // Second POST → 200 with temporaryAttachmentId
        Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
            ))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "temporaryAttachments": [{"temporaryAttachmentId": "tmp-a1-001", "fileName": "t"}]
            })))
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdA==".to_string());
        let result = attach_temporary_file(&client, "42", tmp.path()).await;
        assert!(
            result.is_ok(),
            "A1: 429→200 must succeed; got: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), "tmp-a1-001");

        let reqs = server.received_requests().await.unwrap();
        assert_eq!(
            reqs.len(),
            2,
            "A1: must make exactly 2 POSTs; got {}",
            reqs.len()
        );
    }

    /// A2 – line 108: MAX_RETRIES+1 consecutive 429 responses must exhaust
    /// retries and return an error after exactly 4 POSTs.
    ///
    /// Kills the `attempt < MAX_RETRIES` → `attempt <= MAX_RETRIES` mutant:
    /// with that mutant attempt=3 retries one more time, the loop exhausts, and
    /// the `unreachable!()` fires (panic → test fails).  Also kills any mutant
    /// that causes fewer than 4 POSTs.
    #[tokio::test(start_paused = true)]
    async fn test_attach_tmp_429_exhausts_retries_returns_error() {
        let server = MockServer::start().await;
        let tmp = tempfile::NamedTempFile::new().unwrap();

        // Always return 429; clock paused so each 1s sleep is instant.
        Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
            ))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdA==".to_string());
        let result = attach_temporary_file(&client, "42", tmp.path()).await;
        assert!(result.is_err(), "A2: must fail after MAX_RETRIES; got Ok");

        // Attempts 0,1,2 retry; attempt 3 falls through to error handler: 4 POSTs total.
        let reqs = server.received_requests().await.unwrap();
        assert_eq!(
            reqs.len(),
            4,
            "A2: must make exactly 4 POSTs (MAX_RETRIES+1); got {}",
            reqs.len()
        );
    }

    /// A3 – line 111: Retry-After: 61 exceeds MAX_RETRY_AFTER_SECS (60) cap →
    /// immediate error on the first attempt.
    ///
    /// Kills mutants that invert the cap check (e.g. `delay > 60` →
    /// `delay < 60`): those mutants do NOT return a cap error for delay=61,
    /// instead sleeping and retrying → test receives only 1 POST but the
    /// second mock would return a 429 again → eventual timeout or wrong result.
    #[tokio::test]
    async fn test_attach_tmp_retry_after_cap_exceeded_returns_error() {
        let server = MockServer::start().await;
        let tmp = tempfile::NamedTempFile::new().unwrap();

        // 429 with Retry-After: 61 — strictly above the 60s cap.
        Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
            ))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "61"))
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdA==".to_string());
        let result = attach_temporary_file(&client, "42", tmp.path()).await;
        assert!(
            result.is_err(),
            "A3: Retry-After:61 > cap must return error"
        );
        let err_str = format!("{}", result.unwrap_err());
        assert!(
            err_str.contains("exceeds") && err_str.contains("cap"),
            "A3: error must describe cap exceeded; got: {err_str}"
        );

        // Cap fires on attempt=0: exactly 1 POST (no retry attempted).
        let reqs = server.received_requests().await.unwrap();
        assert_eq!(
            reqs.len(),
            1,
            "A3: cap must fire on first attempt; got {} POSTs",
            reqs.len()
        );
    }

    /// A4 – line 111: Retry-After: 60 equals MAX_RETRY_AFTER_SECS boundary —
    /// must NOT fire the cap error and must retry successfully.
    ///
    /// Kills the `delay > MAX_RETRY_AFTER_SECS` → `delay >= MAX_RETRY_AFTER_SECS`
    /// mutant: with that mutant `60 >= 60 = true` → cap error returned →
    /// test expects success → fails.
    #[tokio::test(start_paused = true)]
    async fn test_attach_tmp_retry_after_60_boundary_not_capped_retries() {
        let server = MockServer::start().await;
        let tmp = tempfile::NamedTempFile::new().unwrap();

        // First POST → 429 with Retry-After: 60 (at boundary; 60 is NOT > 60).
        // Clock paused so the 60s sleep is instant.
        Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
            ))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "60"))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        // Second POST → 200 after the paused-clock 60s sleep.
        Mock::given(method("POST"))
            .and(path(
                "/rest/servicedeskapi/servicedesk/42/attachTemporaryFile",
            ))
            .and(header("X-Atlassian-Token", "no-check"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "temporaryAttachments": [{"temporaryAttachmentId": "tmp-a4-001", "fileName": "t"}]
            })))
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdA==".to_string());
        let result = attach_temporary_file(&client, "42", tmp.path()).await;
        assert!(
            result.is_ok(),
            "A4: Retry-After:60 equals cap boundary — must retry and succeed; got: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), "tmp-a4-001");
    }

    /// B – line 228: `curate_jsm_attachment_entry` must preserve a bare-string
    /// `created` field verbatim (the `Some(Value::String(s))` arm).
    ///
    /// Kills the mutant that removes this arm: without it, a string-form
    /// `created` falls through to `_ => String::new()` → curated field is
    /// empty → assertion fails.
    #[test]
    fn test_curate_jsm_attachment_entry_string_created_preserved() {
        let v = serde_json::json!({
            "filename": "report.pdf",
            "created": "2026-01-01T00:00:00Z",
            "_links": {
                "jiraRest": "https://example.atlassian.net/rest/api/3/attachment/99999",
                "content": "https://example.atlassian.net/rest/api/3/attachment/content/99999"
            },
            "size": 512_u64
        });
        let curated = curate_jsm_attachment_entry(&v);
        assert_eq!(
            curated.created, "2026-01-01T00:00:00Z",
            "B: string-form 'created' must be preserved verbatim; got: {}",
            curated.created
        );
        // Confirm object-form and absent paths are not confused with this arm.
        let v_obj = serde_json::json!({"created": {"iso8601": "2026-07-01T00:00:00Z"}});
        assert_eq!(
            curate_jsm_attachment_entry(&v_obj).created,
            "2026-07-01T00:00:00Z",
            "B: object-form created must still use iso8601 sub-key"
        );
        let v_null: serde_json::Value = serde_json::json!({"created": null});
        assert_eq!(
            curate_jsm_attachment_entry(&v_null).created,
            "",
            "B: null/absent created must fall through to empty string"
        );
    }
}
