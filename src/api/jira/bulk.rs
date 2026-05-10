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
    /// `edited_fields` shape (labels example, labelsAction casing best-guess "ADD"/"REMOVE"):
    /// ```json
    /// {"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}}
    /// ```
    pub async fn bulk_edit_fields(
        &self,
        keys: &[String],
        edited_fields: serde_json::Value,
    ) -> anyhow::Result<String> {
        let body = BulkEditRequest {
            selected_issue_ids_or_keys: keys.to_vec(),
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
                return Ok(progress);
            }

            // Not terminal yet: sleep with exponential backoff before retrying.
            let sleep_secs = backoff.min(POLL_MAX_SECS);
            tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
            backoff = (backoff * 2).min(POLL_MAX_SECS);
        }
    }
}
