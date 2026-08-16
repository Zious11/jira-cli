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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead: Option<ComponentLead>,
    #[serde(
        rename = "assigneeType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub assignee_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedIssueCounts {
    pub id: String,
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
}
