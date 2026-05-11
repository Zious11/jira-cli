//! Atlassian Issue Bulk Operations API.
//!
//! Supports up to 1,000 issues per call. Operations are async on the Jira side:
//! - Submit bulk operation → receive taskId
//! - Poll GET /rest/api/3/bulk/queue/{taskId} until terminal status
//!
//! Terminal statuses: COMPLETE | FAILED | CANCELLED | DEAD (also accepts COMPLETED
//! for empirical safety — OpenAPI says COMPLETE but live API unverified).
//!
//! Per CLAUDE.md: blanket-401 → auto-refresh is already wired into JiraClient::send.
//! 429 during polling: JiraClient::send retries up to MAX_RETRIES times using
//! Retry-After, subject to MAX_RETRY_AFTER_SECS=60 cap.

use std::time::{Duration, Instant};

use crate::api::client::JiraClient;
use crate::types::jira::bulk::{
    BulkEditRequest, BulkOperationProgress, BulkSubmitResponse, BulkTransitionRequest,
};

/// Maximum number of issue keys allowed in a single bulk operation call.
///
/// Sourced from the Atlassian per-call cap documented on both
/// POST /rest/api/3/bulk/issues/fields and POST /rest/api/3/bulk/issues/transition
/// (Issue Bulk Operations API). Reused by all bulk CLI handlers
/// (`issue edit` multi-key + `--jql`, `issue move` multi-key) so that a future
/// Atlassian cap change only requires editing this single constant.
pub const BULK_MAX_KEYS: usize = 1000;

/// Exponential backoff base interval for poll retries (used when task is not yet terminal).
const POLL_BASE_SECS: u64 = 1;
/// Maximum backoff interval (caps exponential growth).
const POLL_MAX_SECS: u64 = 10;

/// Maximum byte length for a taskId received from Atlassian, used as a sanity
/// cap before embedding the value into the bulk-queue URL path.
///
/// Set generously (256 bytes) because Atlassian doesn't officially document the
/// taskId format. Empirical/inferred evidence (Perplexity 2026-05-11, citing
/// Atlassian cloud-identifier conventions) suggests a `{numericPrefix}:{uuid}`
/// shape on the order of ~40-50 chars (e.g.,
/// `"123456:4ac97bc8-ab12-ab12-8d38-eda562abc123"`). The 256-byte cap leaves
/// generous headroom for future format evolution while still rejecting
/// obviously oversized responses that could indicate a hostile/spoofed server.
const MAX_TASK_ID_LEN: usize = 256;

/// Validate that a taskId received in a `BulkSubmitResponse` (or accepted by a
/// poll caller) is safe to embed in the bulk-queue URL path.
///
/// This is defense-in-depth against a hostile or spoofed Atlassian response:
/// the URL is constructed via `urlencoding::encode(task_id)` which defangs
/// path separators, but the allowlist below also rejects oversized responses,
/// embedded NUL/control bytes, identifiers containing path-traversal segment
/// separators (`/`, `\`), and the specific dot-segment values (`.`, `..`)
/// that RFC 3986 §5.2.4 URL normalizers resolve away BEFORE transmission
/// (verified Perplexity 2026-05-11: reqwest/hyper/curl all apply §5.2.4
/// before send, so `/bulk/queue/..` becomes `/bulk/` at the client — a
/// path-confusion vulnerability if `.` or `..` reaches this function).
/// Cross-relevant threat model: a `JR_BASE_URL`-controlled MitM proxy in a
/// test scenario.
///
/// Allowlist: ASCII alphanumeric + `-`, `_`, `:`, `.`. Covers UUIDs, the
/// `domainId:uuid` cloud-identifier pattern, numeric IDs, and other opaque
/// printable-ASCII tokens. The `.` character is permitted within longer
/// tokens (e.g., `v1.0`) but the standalone values `.` and `..` are
/// explicitly rejected as dot-segments.
fn validate_task_id(task_id: &str) -> anyhow::Result<()> {
    if task_id.is_empty() {
        anyhow::bail!(
            "Bulk operation response taskId is empty; cannot poll task status. \
             Re-run the bulk command — if the same error recurs, the Atlassian \
             endpoint may be misbehaving or a proxy may be intercepting responses."
        );
    }
    // Reject dot-segments BEFORE the length and charset checks. Per RFC 3986
    // §5.2.4 (Remove Dot Segments), HTTP clients including reqwest/hyper/curl
    // normalize `.` and `..` path segments BEFORE transmission, rewriting
    // `/rest/api/3/bulk/queue/..` to `/rest/api/3/bulk/` — a path-confusion
    // attack vector if a hostile/spoofed response returned `..` as taskId.
    // urlencoding::encode does NOT encode `.` so the value reaches the
    // normalizer intact. Reject as a single special case.
    if task_id == "." || task_id == ".." {
        anyhow::bail!(
            "Bulk operation response taskId is a dot-segment ({task_id:?}); URL \
             normalizers (RFC 3986 §5.2.4) resolve dot-segments before transmission, \
             which would rewrite the bulk-queue URL path to a different endpoint. \
             Rejecting as hostile/malformed."
        );
    }
    if task_id.len() > MAX_TASK_ID_LEN {
        anyhow::bail!(
            "Bulk operation response taskId is {} bytes (max allowed: {}); rejecting \
             as potentially hostile/malformed response. Re-run the bulk command — if \
             the same error recurs, the Atlassian endpoint may be misbehaving or a \
             proxy may be intercepting responses.",
            task_id.len(),
            MAX_TASK_ID_LEN
        );
    }
    if !task_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
    {
        anyhow::bail!(
            "Bulk operation response taskId contains disallowed characters; \
             allowlist is ASCII alphanumeric + '-', '_', ':', '.'. Got: {task_id:?}. \
             This indicates an unexpected Atlassian response or a hostile proxy."
        );
    }
    Ok(())
}

impl JiraClient {
    /// Submit a bulk field-edit operation.
    ///
    /// POST /rest/api/3/bulk/issues/fields
    /// Returns taskId for polling via `await_bulk_task`.
    ///
    /// Required fields per Atlassian docs (verified Perplexity 2026-05-10, PR2):
    ///   - `selected_issue_ids_or_keys` — issue keys (already taken via `keys`)
    ///   - `selected_actions` — list of field names being edited; without this
    ///     the API returns 400. Mirrors keys used inside `edited_fields`.
    ///   - `edited_fields` — per-field edit payload (shape varies by field type).
    ///
    /// `edited_fields` shape (labels example, labelsAction casing best-guess "ADD"/"REMOVE";
    /// see SCHEMA NOTES in `types/jira/bulk.rs::BulkEditRequest` for the canonical
    /// production shape and the schema-empirical-verification follow-up issue):
    /// ```json
    /// {"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}}
    /// ```
    pub async fn bulk_edit_fields(
        &self,
        keys: &[String],
        selected_actions: Vec<String>,
        edited_fields: serde_json::Value,
    ) -> anyhow::Result<String> {
        let body = BulkEditRequest {
            selected_issue_ids_or_keys: keys.to_vec(),
            selected_actions,
            edited_fields_input: edited_fields,
        };
        let resp: BulkSubmitResponse = self.post("/rest/api/3/bulk/issues/fields", &body).await?;
        validate_task_id(&resp.task_id)?;
        Ok(resp.task_id)
    }

    /// Submit a bulk transition operation.
    ///
    /// POST /rest/api/3/bulk/issues/transition
    /// Returns taskId for polling via `await_bulk_task`.
    ///
    /// `transition_id` is a numeric string (e.g., "31").
    /// CONFIRMED schema: top-level `transitionId` field (not nested).
    pub async fn bulk_transition(
        &self,
        keys: &[String],
        transition_id: &str,
    ) -> anyhow::Result<String> {
        let body = BulkTransitionRequest {
            selected_issue_ids_or_keys: keys.to_vec(),
            transition_id: transition_id.to_string(),
        };
        let resp: BulkSubmitResponse = self
            .post("/rest/api/3/bulk/issues/transition", &body)
            .await?;
        validate_task_id(&resp.task_id)?;
        Ok(resp.task_id)
    }

    /// Poll a single bulk task status snapshot.
    ///
    /// GET /rest/api/3/bulk/queue/{task_id}
    /// This is a low-level single-poll; use `await_bulk_task` for the full polling loop.
    pub async fn poll_bulk_task(&self, task_id: &str) -> anyhow::Result<BulkOperationProgress> {
        // Defense-in-depth: validate again at the URL-construction site, in case
        // a caller obtained the taskId via some path that bypassed validation
        // (e.g., a test fixture, a future deserialized-from-cache scenario).
        // bulk_edit_fields and bulk_transition already validate at the receive
        // boundary; this call is cheap and idempotent.
        validate_task_id(task_id)?;
        let path = format!("/rest/api/3/bulk/queue/{}", urlencoding::encode(task_id));
        self.get(&path).await
    }

    /// Poll a bulk task with exponential backoff until terminal state or timeout.
    ///
    /// Uses `JiraClient::get` → `send` for each poll, which already handles:
    ///   - 429 with Retry-After (up to MAX_RETRIES=3 and MAX_RETRY_AFTER_SECS=60 cap)
    ///   - 401 blanket auto-refresh (S-3.03 v2)
    ///
    /// On non-terminal status, sleeps with exponential backoff (1s→2s→4s→8s→10s cap).
    ///
    /// Returns `Ok(BulkOperationProgress)` when a terminal status is reached.
    /// Returns `Err(...)` when timeout is exceeded or an HTTP error occurs.
    pub async fn await_bulk_task(
        &self,
        task_id: &str,
        timeout: Duration,
    ) -> anyhow::Result<BulkOperationProgress> {
        // Validate task_id at function entry — BEFORE the deadline is computed.
        // The timeout-exceeded error message below interpolates task_id via
        // `Display` ({task_id}), which renders ASCII control characters literally
        // (CR/LF/ANSI escapes would be terminal-interpreted in stderr/logs).
        // If timeout is very small (or 0), the deadline check fires before any
        // call to poll_bulk_task — without this guard, a hostile/spoofed task_id
        // (e.g., `"abc\r\n[jr] FAKE LOG"`) would reach the error sink unsanitized.
        // CWE-117 defense-in-depth.
        validate_task_id(task_id)?;

        let deadline = Instant::now() + timeout;
        let mut backoff = POLL_BASE_SECS;

        loop {
            // Check timeout before each poll attempt.
            if Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Bulk task {task_id} did not complete within {}s timeout. \
                     Check Jira for task status.",
                    timeout.as_secs()
                ));
            }

            // poll_bulk_task → self.get → self.send (handles 429 + 401 auto-refresh).
            let progress = self.poll_bulk_task(task_id).await?;

            if progress.is_terminal() {
                // C-2: FAILED/CANCELLED/DEAD are terminal but unsuccessful.
                // Return an Err so callers surface this as a non-zero exit.
                // COMPLETE and COMPLETED (empirical safety alias) are the only
                // successful terminals — everything else is a task-level failure.
                let status = &progress.status;
                if matches!(status.as_str(), "FAILED" | "CANCELLED" | "DEAD") {
                    // Surface failureReason first if the API provided one (Perplexity-verified
                    // 2026-05-10: Atlassian FAILED responses include `failureReason: String`).
                    // Fall back to per-issue detail or the raw-API hint for older API versions
                    // or alternate failure shapes where failureReason is absent.
                    let hint = if let Some(reason) = progress.failure_reason.as_deref() {
                        reason.to_string()
                    } else if !progress.failed_accessible_issues.is_empty() {
                        let keys: Vec<&str> = progress
                            .failed_accessible_issues
                            .keys()
                            .map(String::as_str)
                            .collect();
                        format!("Failed issues: {}.", keys.join(", "))
                    } else {
                        format!(
                            "Run `jr api /rest/api/3/bulk/queue/{task_id}` to inspect the raw task state."
                        )
                    };
                    return Err(anyhow::anyhow!(
                        "Bulk task {task_id} ended with status {status}. {hint}"
                    ));
                }
                return Ok(progress);
            }

            // Not terminal yet: sleep with exponential backoff before retrying.
            let sleep_secs = backoff.min(POLL_MAX_SECS);
            tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
            backoff = (backoff * 2).min(POLL_MAX_SECS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_TASK_ID_LEN, validate_task_id};

    #[test]
    fn test_validate_task_id_accepts_uuid() {
        validate_task_id("4ac97bc8-ab12-ab12-8d38-eda562abc123").expect("plain UUID rejected");
    }

    #[test]
    fn test_validate_task_id_accepts_domain_prefixed_uuid() {
        // Atlassian cloud-identifier pattern: numericPrefix:uuid
        validate_task_id("123456:4ac97bc8-ab12-ab12-8d38-eda562abc123")
            .expect("domain-prefixed UUID rejected");
    }

    #[test]
    fn test_validate_task_id_accepts_numeric_token() {
        validate_task_id("12345").expect("numeric token rejected");
    }

    #[test]
    fn test_validate_task_id_accepts_alphanumeric_opaque() {
        validate_task_id("abcDEF123_test-token").expect("opaque token rejected");
    }

    #[test]
    fn test_validate_task_id_rejects_empty() {
        let err = validate_task_id("").expect_err("empty taskId accepted");
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_task_id_rejects_oversized() {
        let oversized = "a".repeat(MAX_TASK_ID_LEN + 1);
        let err = validate_task_id(&oversized).expect_err("oversized taskId accepted");
        assert!(err.to_string().contains("bytes"));
    }

    #[test]
    fn test_validate_task_id_accepts_at_max_length() {
        let at_max = "a".repeat(MAX_TASK_ID_LEN);
        validate_task_id(&at_max).expect("boundary-length taskId rejected");
    }

    #[test]
    fn test_validate_task_id_rejects_forward_slash() {
        let err = validate_task_id("task/123").expect_err("forward slash accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_backslash() {
        let err = validate_task_id("task\\123").expect_err("backslash accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_path_traversal_attempt() {
        // "../etc/passwd" contains "/" which is outside the allowlist, so the
        // charset rejection fires first.
        let err = validate_task_id("../etc/passwd").expect_err("path traversal accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_single_dot_segment() {
        // Standalone "." is a dot-segment per RFC 3986 §5.2.4: HTTP client
        // libraries (reqwest, hyper, curl) normalize "/path/." to "/path/"
        // BEFORE transmission. Reject explicitly so a hostile/spoofed response
        // can't rewrite the bulk-queue URL path.
        let err = validate_task_id(".").expect_err("dot-segment accepted");
        assert!(err.to_string().contains("dot-segment"));
    }

    #[test]
    fn test_validate_task_id_rejects_double_dot_segment() {
        // Standalone ".." is a dot-segment per RFC 3986 §5.2.4: HTTP client
        // libraries normalize "/path/.." to its parent BEFORE transmission.
        // urlencoding::encode does NOT escape ".", so the dot-segment reaches
        // the normalizer intact. Reject explicitly to prevent path-confusion
        // attacks (verified Perplexity 2026-05-11 against RFC 3986 §5.2.4).
        let err = validate_task_id("..").expect_err("double-dot segment accepted");
        assert!(err.to_string().contains("dot-segment"));
    }

    #[test]
    fn test_validate_task_id_accepts_dot_within_longer_token() {
        // Dots are permitted WITHIN longer tokens (e.g., a hypothetical
        // version-prefixed taskId like "v1.0:abcd-..."). Only the standalone
        // values "." and ".." are dot-segments per RFC 3986.
        validate_task_id("v1.0:abc123").expect("dotted token rejected");
    }

    #[test]
    fn test_validate_task_id_rejects_null_byte() {
        let err = validate_task_id("task\x00123").expect_err("null byte accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_newline() {
        let err = validate_task_id("task\n123").expect_err("newline accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_carriage_return() {
        // CWE-117 adjacent: CR injection into URL paths can mess with downstream
        // logging or terminal escape interpretation; defang at the source.
        let err = validate_task_id("task\r123").expect_err("CR accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_space() {
        let err = validate_task_id("task 123").expect_err("space accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_non_ascii() {
        // Unicode is outside the URL-safe allowlist for this identifier; Atlassian
        // taskIds are all ASCII per observed and inferred formats.
        let err = validate_task_id("tâsk-123").expect_err("non-ASCII accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }
}
