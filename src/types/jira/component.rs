use serde::{Deserialize, Serialize};

/// Full-resource component as returned by
/// `GET /rest/api/3/project/{key}/components` and
/// `GET /rest/api/3/component/{id}`.
///
/// **Distinct from `src/types/jira/issue.rs::Component`** (the embedded
/// `fields.components[]` type) — see BC-2.3.040 Precondition 1.
/// This type keeps `id: String` REQUIRED (not `Option`) because the
/// §8.4 resolver depends on a real id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    pub name: String,
    // BC-8.1.002: no field is dropped for JSON mode — skip_serializing_if
    // is intentionally absent so None serializes as explicit JSON null.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lead: Option<ComponentLead>,
    #[serde(rename = "assigneeType", default)]
    pub assignee_type: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(
        rename = "relatedIssueCount",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub related_issue_count: Option<u64>,
    #[serde(
        rename = "isAssigneeTypeValid",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub is_assignee_type_valid: Option<bool>,
}

/// Lead (account) information embedded on a Component resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentLead {
    #[serde(rename = "accountId", default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(
        rename = "displayName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub display_name: Option<String>,
}

/// Response shape from
/// `GET /rest/api/3/component/{id}/relatedIssueCounts`.
///
/// The live Jira Cloud endpoint returns `{"self": "…", "issueCount": N}` —
/// the `id` field is NOT present in the response (F-3 fix: mock-vs-live drift).
/// Making `id` `Option<String>` with `#[serde(default)]` prevents a
/// deserialization failure when the field is absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedIssueCounts {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "issueCount")]
    pub issue_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── S-604-1: BC-2.3.040 Precondition 1 test (AC-018) ─────────────────────

    /// AC-018 / BC-2.3.040 Precondition 1: the FULL resource `Component.id` is a
    /// required `String` (NOT `Option<String>`).  A JSON fixture omitting `"id"`
    /// MUST fail to deserialize — this type is DISTINCT from
    /// `types/jira/issue::Component` (the embedded type that accepts absent id).
    #[test]
    fn test_bc_2_3_040_full_resource_component_id_required_not_optional() {
        // JSON with no "id" key — must fail because Component.id: String (required)
        let json = serde_json::json!({"name": "Backend"});
        let result = serde_json::from_value::<Component>(json);
        assert!(
            result.is_err(),
            "Full-resource Component.id is required (String, not Option); \
             deserialization must fail when id is absent"
        );
    }

    // ── S-604-1: RelatedIssueCounts id-absent regression pin (F-3 / FIX-576-DL) ─

    /// F-3 / mock-vs-live drift: the live Jira Cloud endpoint
    /// `GET /rest/api/3/component/{id}/relatedIssueCounts` returns
    /// `{"self": "…", "issueCount": N}` — the `"id"` field is NOT present.
    /// This test pins that the id-absent shape deserializes correctly, so a
    /// future revert of `#[serde(default)]` on `RelatedIssueCounts.id` fails CI
    /// before it can break live `--counts` calls (FIX-576-DL class).
    #[test]
    fn test_related_issue_counts_deserializes_without_id_field() {
        let json = serde_json::json!({"issueCount": 5});
        let result = serde_json::from_value::<RelatedIssueCounts>(json)
            .expect("id-absent shape must deserialize successfully");
        assert_eq!(result.id, None, "id must be None when absent from the JSON");
        assert_eq!(result.issue_count, 5, "issue_count must be 5");
    }

    /// Regression guard (FIX-576-DL two-test pattern): the WITH-id form that the
    /// mock server historically emitted must continue to deserialize correctly.
    /// This ensures the `#[serde(default)]` change did not break the id-present path.
    #[test]
    fn test_related_issue_counts_deserializes_with_id_field() {
        let json = serde_json::json!({"id": "10001", "issueCount": 7});
        let result = serde_json::from_value::<RelatedIssueCounts>(json)
            .expect("id-present shape must still deserialize successfully");
        assert_eq!(
            result.id,
            Some("10001".to_string()),
            "id must be Some(\"10001\") when present"
        );
        assert_eq!(result.issue_count, 7, "issue_count must be 7");
    }
}
