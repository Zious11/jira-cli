use std::collections::HashMap;

/// Request body for POST /rest/api/3/bulk/issues/fields (bulk field edit).
///
/// CONFIRMED from OpenAPI JSON (2026-05-09):
///   - selectedIssueIdsOrKeys: string[], required, max 1000
///   - editedFieldsInput: object, required (schema partially truncated in HTML docs)
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BulkEditRequest {
    pub selected_issue_ids_or_keys: Vec<String>,
    pub edited_fields_input: serde_json::Value,
}

/// Request body for POST /rest/api/3/bulk/issues/transition (bulk transition).
///
/// CONFIRMED from OpenAPI JSON (2026-05-09):
///   - selectedIssueIdsOrKeys: string[], required, writeOnly
///   - transitionId: string, required, writeOnly (top-level, NOT nested)
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BulkTransitionRequest {
    pub selected_issue_ids_or_keys: Vec<String>,
    pub transition_id: String,
}

/// Response from POST /rest/api/3/bulk/issues/fields or POST /rest/api/3/bulk/issues/transition.
///
/// CONFIRMED from OpenAPI JSON: taskId is a top-level field in the BulkOperationProgress body.
/// HTTP 200 is the success status for both bulk submission endpoints.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BulkSubmitResponse {
    pub task_id: String,
}

/// Response from GET /rest/api/3/bulk/queue/{taskId} (poll task status).
///
/// CONFIRMED terminal status values from OpenAPI JSON:
///   ENQUEUED | RUNNING | COMPLETE | FAILED | CANCEL_REQUESTED | CANCELLED | DEAD
///   NOTE: "COMPLETE" not "COMPLETED" per OpenAPI; empirical verify pending.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BulkOperationProgress {
    pub task_id: Option<String>,
    /// Status string — terminal when one of: COMPLETE, FAILED, CANCELLED, DEAD
    pub status: String,
    #[serde(default)]
    pub processed_accessible_issues: Vec<String>,
    #[serde(default)]
    pub failed_accessible_issues: HashMap<String, BulkActionError>,
    #[serde(default)]
    pub progress_percent: Option<i64>,
    #[serde(default)]
    pub total_issue_count: Option<i64>,
    #[serde(default)]
    pub invalid_or_inaccessible_issue_count: Option<i32>,
}

impl BulkOperationProgress {
    /// Whether this status is a terminal (final) state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status.as_str(),
            "COMPLETE" | "FAILED" | "CANCELLED" | "DEAD" | "COMPLETED"
        )
    }

    /// Whether all accessible issues were processed successfully.
    pub fn is_success(&self) -> bool {
        self.failed_accessible_issues.is_empty()
    }
}

/// Per-issue error detail from the failedAccessibleIssues map.
///
/// CONFIRMED schema: BulkEditActionError in OpenAPI JSON.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BulkActionError {
    #[serde(default)]
    pub error_messages: Vec<String>,
    #[serde(default)]
    pub errors: serde_json::Value,
}

impl BulkActionError {
    /// Returns a human-readable error summary.
    pub fn summary(&self) -> String {
        if !self.error_messages.is_empty() {
            return self.error_messages.join("; ");
        }
        if let Some(obj) = self.errors.as_object() {
            if !obj.is_empty() {
                let pairs: Vec<String> = obj.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                return pairs.join("; ");
            }
        }
        "unknown error".to_string()
    }
}
