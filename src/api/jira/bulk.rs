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
        Ok(resp.task_id)
    }

    /// Poll a single bulk task status snapshot.
    ///
    /// GET /rest/api/3/bulk/queue/{task_id}
    /// This is a low-level single-poll; use `await_bulk_task` for the full polling loop.
    pub async fn poll_bulk_task(&self, task_id: &str) -> anyhow::Result<BulkOperationProgress> {
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
