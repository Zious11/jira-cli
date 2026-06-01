use std::collections::HashMap;

/// Deserialize a taskId field that Jira Cloud may return as either a JSON string
/// or a JSON integer.
///
/// Real Jira Cloud tenants return `taskId` as a JSON integer (e.g. `10110`),
/// not a string. The Atlassian OpenAPI spec documents it as a string, but the
/// live API disagrees. Discovered via live E2E run 26733998365 which produced:
///   `Error: invalid type: integer 10110, expected a string at line 1 column 114`
///
/// This deserializer accepts both shapes and normalizes to `String` so downstream
/// code (`validate_task_id`, URL path construction, `urlencoding::encode`) all
/// receive a consistent type. "10110" is a valid `validate_task_id` input
/// (all ASCII alphanumeric).
///
/// Rejects floats, booleans, arrays, objects, and **null** with a serde error —
/// only `String` and integer types are plausible taskId representations. This
/// variant is used for required (`String`) fields; `null` or absent values are
/// therefore an error. For optional taskId fields that should map `null`/absent
/// → `None`, use `deserialize_opt_task_id_string_or_int` instead.
fn deserialize_task_id_string_or_int<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Unexpected, Visitor};
    use std::fmt;

    struct StringOrIntVisitor;

    impl<'de> Visitor<'de> for StringOrIntVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a string or integer taskId")
        }

        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(v.to_owned())
        }

        fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(v)
        }

        fn visit_i64<E: Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(v.to_string())
        }

        fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(v.to_string())
        }

        fn visit_f64<E: Error>(self, v: f64) -> Result<Self::Value, E> {
            Err(E::invalid_type(
                Unexpected::Float(v),
                &"a string or integer taskId",
            ))
        }

        fn visit_bool<E: Error>(self, v: bool) -> Result<Self::Value, E> {
            Err(E::invalid_type(
                Unexpected::Bool(v),
                &"a string or integer taskId",
            ))
        }
    }

    deserializer.deserialize_any(StringOrIntVisitor)
}

/// Option-wrapped variant of `deserialize_task_id_string_or_int` for
/// `BulkOperationProgress.task_id: Option<String>`. Handles `null` and
/// absent fields (via `#[serde(default)]`) → `None`; string → `Some(String)`;
/// integer → `Some(String)`.
fn deserialize_opt_task_id_string_or_int<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Unexpected, Visitor};
    use std::fmt;

    struct OptStringOrIntVisitor;

    impl<'de> Visitor<'de> for OptStringOrIntVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a string, integer, or null taskId")
        }

        fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D2: serde::Deserializer<'de>>(
            self,
            deserializer: D2,
        ) -> Result<Self::Value, D2::Error> {
            deserialize_task_id_string_or_int(deserializer).map(Some)
        }

        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(Some(v.to_owned()))
        }

        fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_i64<E: Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v.to_string()))
        }

        fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v.to_string()))
        }

        fn visit_f64<E: Error>(self, v: f64) -> Result<Self::Value, E> {
            Err(E::invalid_type(
                Unexpected::Float(v),
                &"a string, integer, or null taskId",
            ))
        }

        fn visit_bool<E: Error>(self, v: bool) -> Result<Self::Value, E> {
            Err(E::invalid_type(
                Unexpected::Bool(v),
                &"a string, integer, or null taskId",
            ))
        }
    }

    deserializer.deserialize_any(OptStringOrIntVisitor)
}

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
///     labels → `{"labelsFields": [{"fieldId":"labels","bulkEditMultiSelectFieldOption":"ADD|REMOVE","labels":[{"name":"..."}]}]}`
///     per Atlassian Bulk Operations FAQ (verified, issue #446). Each action (ADD / REMOVE)
///     is a separate element. Label items are `{"name":...}` objects.
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
///
/// NOTE: The Atlassian OpenAPI spec documents `taskId` as a string, but real Jira Cloud
/// tenants return it as a JSON integer (e.g. 10110). `deserialize_task_id_string_or_int`
/// normalizes both forms to `String`. Discovered via live E2E run 26733998365.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BulkSubmitResponse {
    #[serde(deserialize_with = "deserialize_task_id_string_or_int")]
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
    #[serde(default, deserialize_with = "deserialize_opt_task_id_string_or_int")]
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

    // --- taskId integer deserialization tests (live E2E 26733998365) ---
    //
    // Real Jira Cloud returns `taskId` as a JSON integer (e.g. 10110), not a
    // string. The wiremock-based tests always mock it as a string, so this
    // never surfaced offline. These tests pin the fix: both BulkSubmitResponse
    // and BulkOperationProgress must accept integer taskId and normalize to String.

    /// BulkSubmitResponse: integer taskId → String (the live E2E bug case).
    #[test]
    fn test_deserialize_bulk_submit_response_integer_task_id() {
        let json = r#"{"taskId": 10110}"#;
        let resp: BulkSubmitResponse = serde_json::from_str(json)
            .expect("BulkSubmitResponse should deserialize integer taskId");
        assert_eq!(resp.task_id, "10110");
    }

    /// BulkSubmitResponse: string taskId → unchanged (regression guard).
    #[test]
    fn test_deserialize_bulk_submit_response_string_task_id_regression() {
        let json = r#"{"taskId": "abc-1"}"#;
        let resp: BulkSubmitResponse = serde_json::from_str(json)
            .expect("BulkSubmitResponse should deserialize string taskId");
        assert_eq!(resp.task_id, "abc-1");
    }

    /// BulkOperationProgress: integer taskId → Some(String).
    #[test]
    fn test_deserialize_bulk_operation_progress_integer_task_id() {
        let json = r#"{"taskId": 10110, "status": "ENQUEUED"}"#;
        let prog: BulkOperationProgress = serde_json::from_str(json)
            .expect("BulkOperationProgress should deserialize integer taskId");
        assert_eq!(prog.task_id, Some("10110".to_string()));
    }

    /// BulkOperationProgress: string taskId → Some(String) (regression guard).
    #[test]
    fn test_deserialize_bulk_operation_progress_string_task_id_regression() {
        let json = r#"{"taskId": "abc-1", "status": "COMPLETE"}"#;
        let prog: BulkOperationProgress = serde_json::from_str(json)
            .expect("BulkOperationProgress should deserialize string taskId");
        assert_eq!(prog.task_id, Some("abc-1".to_string()));
    }

    /// BulkOperationProgress: absent taskId → None (field is Option).
    #[test]
    fn test_deserialize_bulk_operation_progress_absent_task_id() {
        let json = r#"{"status": "RUNNING"}"#;
        let prog: BulkOperationProgress =
            serde_json::from_str(json).expect("BulkOperationProgress should allow absent taskId");
        assert_eq!(prog.task_id, None);
    }

    // --- Rejection contract: floats and bools must never be silently accepted ---
    //
    // These tests pin the rustdoc claim that `deserialize_task_id_string_or_int`
    // (required-field variant) rejects floats and booleans with a serde error.
    // A future refactor that widens the visitor (e.g. accepting `visit_f64` to
    // "be lenient") would silently break callers that depend on strict typing;
    // these tests catch that regression at the unit level.

    /// BulkSubmitResponse: float taskId must be rejected.
    #[test]
    fn test_deserialize_bulk_submit_response_rejects_float_task_id() {
        assert!(
            serde_json::from_str::<BulkSubmitResponse>(r#"{"taskId": 1.5}"#).is_err(),
            "BulkSubmitResponse must reject a float taskId"
        );
    }

    /// BulkSubmitResponse: bool taskId must be rejected.
    #[test]
    fn test_deserialize_bulk_submit_response_rejects_bool_task_id() {
        assert!(
            serde_json::from_str::<BulkSubmitResponse>(r#"{"taskId": true}"#).is_err(),
            "BulkSubmitResponse must reject a bool taskId"
        );
    }

    /// BulkOperationProgress: float taskId must be rejected.
    #[test]
    fn test_deserialize_bulk_operation_progress_rejects_float_task_id() {
        assert!(
            serde_json::from_str::<BulkOperationProgress>(
                r#"{"taskId": 1.5, "status": "ENQUEUED"}"#
            )
            .is_err(),
            "BulkOperationProgress must reject a float taskId"
        );
    }

    /// BulkOperationProgress: bool taskId must be rejected.
    #[test]
    fn test_deserialize_bulk_operation_progress_rejects_bool_task_id() {
        assert!(
            serde_json::from_str::<BulkOperationProgress>(
                r#"{"taskId": false, "status": "ENQUEUED"}"#
            )
            .is_err(),
            "BulkOperationProgress must reject a bool taskId"
        );
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
