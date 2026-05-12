use std::collections::HashMap;

/// Request body for POST /rest/api/3/bulk/issues/fields (bulk field edit).
///
/// CONFIRMED from OpenAPI JSON (2026-05-09) + Perplexity verification (2026-05-10, PR2):
///   - selectedIssueIdsOrKeys: string[], required, max 1000
///   - selectedActions: string[], required — list of field names being edited.
///     Without this, the API returns 400. Examples: ["summary"], ["labels"],
///     ["summary","priority","labels"]. The values mirror the keys used inside
///     `editedFieldsInput`. The bulk endpoint's canonical casing for the issuetype key
///     is unverified — the legacy single-key path uses lowercase "issuetype", which is
///     preserved here for consistency. See #331.
///   - editedFieldsInput: object, required (schema partially truncated in HTML docs).
///     Per Perplexity verification, the canonical 2025 production shape uses:
///     summary → plain string OR `{"value": "..."}` (sources differ).
///     priority → `{"priorityId": <int>}` per docs; we currently ship `{"name": "..."}`
///     as a best-guess and may receive 400 from real Jira tenants.
///     issueType → `{"issueTypeId": "..."}` per docs (camelCase key); we currently
///     ship `{"issuetype": {"name": "..."}}` (lowercase, name) as a best-guess.
///     labels → `{"labelsFields": [{"fieldId":"labels","labels":[...],
///     "bulkEditMultiSelectFieldOption":"ADD|REMOVE"}]}` per docs; we currently ship
///     the simpler `{"labels": ...}` shapes (single object for ADD-only/REMOVE-only,
///     array for ADD+REMOVE coalesced) as a best-guess.
///     Empirical verification against a live Jira sandbox + name→ID resolution
///     (priorities + issue types per project) is tracked at issue #331.
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BulkEditRequest {
    pub selected_issue_ids_or_keys: Vec<String>,
    pub selected_actions: Vec<String>,
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
/// Status values from OpenAPI JSON (verified Perplexity 2026-05-10):
///
/// - **Non-terminal** (continue polling): ENQUEUED, RUNNING, CANCEL_REQUESTED.
///   CANCEL_REQUESTED transitions to CANCELLED once cancellation completes.
/// - **Terminal** (polling stops): COMPLETE, FAILED, CANCELLED, DEAD.
///
/// NOTE: "COMPLETE" not "COMPLETED" per OpenAPI; `is_terminal()` accepts both
/// for empirical safety. Live API verification deferred to #331.
///
/// `failure_reason`: present on FAILED responses per Atlassian docs (Perplexity
///   verification 2026-05-10, PR2 audit follow-up). Treated as `Option<String>` because
///   the OpenAPI spec is partially undocumented on this field — older API versions or
///   alternate failure shapes may omit it.
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
    /// Human-readable failure reason from Atlassian when status is FAILED.
    /// `#[serde(default)]` so absence doesn't break deserialization on older API versions.
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub progress_percent: Option<i64>,
    #[serde(default)]
    pub total_issue_count: Option<i64>,
    #[serde(default)]
    pub invalid_or_inaccessible_issue_count: Option<i32>,
}

impl BulkOperationProgress {
    /// Whether this status is a terminal (final) state.
    ///
    /// Includes `PARTIAL_FAILURE` and `PROCESSED_WITH_ERRORS` (Perplexity-
    /// validated 2026-05-12 additions to the Atlassian OpenAPI enum — both
    /// indicate the operation finished, just with mixed per-item outcomes).
    /// Caller routing in `await_bulk_task` treats them like `COMPLETE` and
    /// returns `Ok(progress)`; partial-failure detail surfaces via
    /// `failed_accessible_issues` + `is_success()`.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status.as_str(),
            "COMPLETE"
                | "COMPLETED"
                | "FAILED"
                | "CANCELLED"
                | "DEAD"
                | "PARTIAL_FAILURE"
                | "PROCESSED_WITH_ERRORS"
        )
    }

    /// Whether this status is documented per the Atlassian OpenAPI spec or
    /// an empirical safety alias the client recognizes.
    ///
    /// Returns `false` for novel statuses Atlassian may introduce after this
    /// crate was published. Caller (`await_bulk_task`) treats those as
    /// suspicious: warn on first sighting and escalate to terminal-with-error
    /// after a grace period so a novel-but-actually-terminal status doesn't
    /// silently poll until the 5-minute timeout. Closes audit-followup #336.
    ///
    /// Set source: Atlassian Jira Cloud REST API v3 OpenAPI spec, verified
    /// Perplexity 2026-05-12. Update this list if Atlassian adds more
    /// statuses and the integration tests start hitting the warn+grace path.
    pub fn is_known_status(&self) -> bool {
        matches!(
            self.status.as_str(),
            // Terminal (per OpenAPI + COMPLETED empirical alias):
            "COMPLETE"
                | "COMPLETED"
                | "FAILED"
                | "CANCELLED"
                | "DEAD"
                | "PARTIAL_FAILURE"
                | "PROCESSED_WITH_ERRORS"
            // Non-terminal (per OpenAPI):
            | "ENQUEUED"
                | "RUNNING"
                | "CANCEL_REQUESTED"
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

#[cfg(test)]
mod tests {
    use super::*;

    fn progress_with_status(status: &str) -> BulkOperationProgress {
        BulkOperationProgress {
            task_id: None,
            status: status.to_string(),
            processed_accessible_issues: Vec::new(),
            failed_accessible_issues: std::collections::HashMap::new(),
            progress_percent: None,
            total_issue_count: None,
            invalid_or_inaccessible_issue_count: None,
            failure_reason: None,
        }
    }

    #[test]
    fn is_terminal_recognizes_documented_and_empirical_aliases() {
        // Documented terminal statuses per OpenAPI (verified Perplexity
        // 2026-05-12): COMPLETE, FAILED, CANCELLED, DEAD, PARTIAL_FAILURE,
        // PROCESSED_WITH_ERRORS. COMPLETED is included as an empirical-safety
        // alias (some live API responses observed it; tracked at #331 pending
        // sandbox verification). Do not remove COMPLETED without confirming
        // the live API never emits it.
        //
        // PARTIAL_FAILURE and PROCESSED_WITH_ERRORS are 2026 additions to
        // the Atlassian enum — added via audit-followup #336 alongside the
        // unknown-status grace-period fix.
        let terminal = [
            "COMPLETE",
            "COMPLETED",
            "FAILED",
            "CANCELLED",
            "DEAD",
            "PARTIAL_FAILURE",
            "PROCESSED_WITH_ERRORS",
        ];
        let non_terminal = [
            "RUNNING",
            "ENQUEUED",
            "PENDING",
            "IN_PROGRESS",
            "CANCEL_REQUESTED",
            "",
            "unknown",
        ];
        for s in terminal {
            assert!(
                progress_with_status(s).is_terminal(),
                "expected {s} terminal"
            );
        }
        for s in non_terminal {
            assert!(
                !progress_with_status(s).is_terminal(),
                "expected {s} non-terminal"
            );
        }
    }

    #[test]
    fn test_336_is_known_status_recognizes_documented_set() {
        // is_known_status() returns true for the full documented enum
        // (terminal ∪ non-terminal) and false for novel/unrecognized values.
        // Source: Atlassian Jira Cloud REST API v3 OpenAPI spec (Perplexity-
        // verified 2026-05-12).
        let known = [
            // Terminal
            "COMPLETE",
            "COMPLETED",
            "FAILED",
            "CANCELLED",
            "DEAD",
            "PARTIAL_FAILURE",
            "PROCESSED_WITH_ERRORS",
            // Non-terminal
            "ENQUEUED",
            "RUNNING",
            "CANCEL_REQUESTED",
        ];
        let unknown = [
            // Strings not in the OpenAPI enum — these trigger the
            // warn+grace-period path in await_bulk_task.
            "PAUSED",
            "PENDING",     // observed-in-test only, not in OpenAPI
            "IN_PROGRESS", // observed-in-test only, not in OpenAPI
            "FOO_BAR",
            "",
            "unknown",
            "complete", // case-sensitive: lowercase != COMPLETE
        ];
        for s in known {
            assert!(
                progress_with_status(s).is_known_status(),
                "expected {s} to be a known status"
            );
        }
        for s in unknown {
            assert!(
                !progress_with_status(s).is_known_status(),
                "expected {s} to be an unknown status"
            );
        }
    }
}
