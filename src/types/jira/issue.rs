use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::User;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Issue {
    pub key: String,
    pub fields: IssueFields,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ParentIssue {
    pub key: String,
    pub fields: Option<LinkedIssueFields>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkedIssueFields {
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueLink {
    pub id: String,
    #[serde(rename = "type")]
    pub link_type: IssueLinkType,
    #[serde(rename = "inwardIssue")]
    pub inward_issue: Option<LinkedIssue>,
    #[serde(rename = "outwardIssue")]
    pub outward_issue: Option<LinkedIssue>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkedIssue {
    pub key: String,
    pub fields: Option<LinkedIssueFields>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueLinkType {
    pub id: Option<String>,
    pub name: String,
    pub inward: Option<String>,
    pub outward: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueLinkTypesResponse {
    #[serde(rename = "issueLinkTypes")]
    pub issue_link_types: Vec<IssueLinkType>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct IssueFields {
    pub summary: String,
    pub description: Option<Value>,
    pub status: Option<Status>,
    #[serde(rename = "issuetype")]
    pub issue_type: Option<IssueType>,
    pub priority: Option<Priority>,
    pub assignee: Option<User>,
    pub reporter: Option<User>,
    pub project: Option<IssueProject>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub components: Option<Vec<Component>>,
    #[serde(rename = "fixVersions", default)]
    pub fix_versions: Option<Vec<Version>>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    pub parent: Option<ParentIssue>,
    pub issuelinks: Option<Vec<IssueLink>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl IssueFields {
    pub fn story_points(&self, field_id: &str) -> Option<f64> {
        self.extra.get(field_id)?.as_f64()
    }

    /// Extract the team UUID from the issue's team field.
    ///
    /// Accepts two shapes documented for Jira's Team custom field:
    /// - Scalar string UUID (legacy / some tenants).
    /// - Object `{"id": "<uuid>", "name": "..."}` (Atlas Teams platform).
    ///
    /// Returns `None` when the field is missing, null, or present but not one of
    /// the accepted shapes. On genuinely unexpected shapes (bool, number, array,
    /// or object without a string `id`), emits a once-per-process `[verbose]`
    /// hint on stderr when `verbose` is true.
    pub fn team_id(&self, field_id: &str, verbose: bool) -> Option<String> {
        use std::sync::atomic::{AtomicBool, Ordering};
        static LOGGED: AtomicBool = AtomicBool::new(false);
        let value = self.extra.get(field_id)?;
        if value.is_null() {
            return None;
        }
        // Scalar UUID shape (legacy and some tenants).
        if let Some(s) = value.as_str() {
            return Some(s.to_string());
        }
        // Atlas Teams object shape: {"id": "<uuid>", "name": "..."}
        // Per developer.atlassian.com/platform/teams/components/team-field-in-jira-rest-api,
        // the Team custom field returns an object on GET in tenants that use the Atlas
        // Teams platform. Extract `id` as the UUID.
        if let Some(obj) = value.as_object()
            && let Some(id) = obj.get("id").and_then(|v| v.as_str())
        {
            return Some(id.to_string());
        }
        if verbose && !LOGGED.swap(true, Ordering::Relaxed) {
            eprintln!(
                "[verbose] team field \"{field_id}\" has unexpected shape (got {})",
                value_kind(value)
            );
        }
        None
    }
}

fn value_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Status {
    pub name: String,
    #[serde(rename = "statusCategory")]
    pub status_category: Option<StatusCategory>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatusCategory {
    pub name: String,
    pub key: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueType {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Priority {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueProject {
    pub key: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct Resolution {
    /// Resolution id — populated by `GET /rest/api/3/resolution`; absent on
    /// `issue.fields.resolution` responses which only carry the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    /// Description — populated by `GET /rest/api/3/resolution`; absent on
    /// `issue.fields.resolution` responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Component {
    pub name: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Version {
    pub name: String,
    pub released: Option<bool>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Transition {
    pub id: String,
    pub name: String,
    pub to: Option<Status>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TransitionsResponse {
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityProperty {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Comment {
    pub id: Option<String>,
    pub body: Option<Value>,
    pub author: Option<User>,
    pub created: Option<String>,
    #[serde(default)]
    pub properties: Vec<EntityProperty>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateIssueResponse {
    pub key: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fields_with_extra(key: &str, value: serde_json::Value) -> IssueFields {
        let mut fields = IssueFields::default();
        fields.extra.insert(key.to_string(), value);
        fields
    }

    #[test]
    fn team_id_reads_string_value() {
        let fields = fields_with_extra(
            "customfield_10001",
            json!("36885b3c-1bf0-4f85-a357-c5b858c31de4"),
        );
        assert_eq!(
            fields.team_id("customfield_10001", false),
            Some("36885b3c-1bf0-4f85-a357-c5b858c31de4".to_string())
        );
    }

    #[test]
    fn team_id_returns_none_for_null_value() {
        let fields = fields_with_extra("customfield_10001", json!(null));
        assert_eq!(fields.team_id("customfield_10001", false), None);
    }

    #[test]
    fn team_id_returns_none_for_missing_key() {
        let fields = IssueFields::default();
        assert_eq!(fields.team_id("customfield_10001", false), None);
    }

    #[test]
    fn team_id_accepts_object_shape_with_string_id() {
        let fields = fields_with_extra(
            "customfield_10001",
            json!({"id": "team-uuid-abc", "name": "Platform Team"}),
        );
        assert_eq!(
            fields.team_id("customfield_10001", false),
            Some("team-uuid-abc".to_string())
        );
    }

    #[test]
    fn team_id_accepts_object_shape_without_name() {
        let fields = fields_with_extra("customfield_10001", json!({"id": "team-uuid-xyz"}));
        assert_eq!(
            fields.team_id("customfield_10001", false),
            Some("team-uuid-xyz".to_string())
        );
    }

    #[test]
    fn team_id_returns_none_for_object_with_null_id() {
        let fields = fields_with_extra(
            "customfield_10001",
            json!({"id": null, "name": "Platform Team"}),
        );
        assert_eq!(fields.team_id("customfield_10001", false), None);
    }

    #[test]
    fn team_id_returns_none_for_object_without_id_key() {
        let fields = fields_with_extra("customfield_10001", json!({"name": "Platform Team"}));
        assert_eq!(fields.team_id("customfield_10001", false), None);
    }

    #[test]
    fn team_id_returns_none_for_array_value() {
        let fields = fields_with_extra("customfield_10001", json!([1, 2, 3]));
        assert_eq!(fields.team_id("customfield_10001", false), None);
    }

    #[test]
    fn story_points_present() {
        let json = json!({"summary": "test", "customfield_10031": 5.0});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), Some(5.0));
    }

    #[test]
    fn story_points_missing() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), None);
    }

    #[test]
    fn story_points_null() {
        let json = json!({"summary": "test", "customfield_10031": null});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), None);
    }

    #[test]
    fn story_points_wrong_type() {
        let json = json!({"summary": "test", "customfield_10031": "not a number"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), None);
    }

    #[test]
    fn story_points_decimal() {
        let json = json!({"summary": "test", "customfield_10031": 3.5});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), Some(3.5));
    }

    #[test]
    fn story_points_integer_value() {
        let json = json!({"summary": "test", "customfield_10031": 13});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.story_points("customfield_10031"), Some(13.0));
    }

    #[test]
    fn flatten_does_not_break_labels_null() {
        let json = json!({"summary": "test", "labels": null});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.labels, None);
    }

    #[test]
    fn flatten_does_not_break_labels_present() {
        let json = json!({"summary": "test", "labels": ["bug", "frontend"]});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(
            fields.labels,
            Some(vec!["bug".to_string(), "frontend".to_string()])
        );
    }

    #[test]
    fn flatten_does_not_break_labels_missing() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.labels, None);
    }

    #[test]
    fn flatten_does_not_break_description_null() {
        let json = json!({"summary": "test", "description": null});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.description.is_none());
    }

    #[test]
    fn parent_deserializes() {
        let json = json!({
            "summary": "test",
            "parent": {"key": "FOO-42", "fields": {"summary": "Parent epic"}}
        });
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        let parent = fields.parent.unwrap();
        assert_eq!(parent.key, "FOO-42");
        assert_eq!(parent.fields.unwrap().summary.unwrap(), "Parent epic");
    }

    #[test]
    fn parent_missing() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.parent.is_none());
    }

    #[test]
    fn issuelinks_deserializes() {
        let json = json!({
            "summary": "test",
            "issuelinks": [{
                "id": "10001",
                "type": {"name": "Blocks", "inward": "is blocked by", "outward": "blocks"},
                "outwardIssue": {"key": "FOO-2", "fields": {"summary": "Other issue"}}
            }]
        });
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        let links = fields.issuelinks.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, "10001");
        assert_eq!(links[0].link_type.name, "Blocks");
        let outward = links[0].outward_issue.as_ref().unwrap();
        assert_eq!(outward.key, "FOO-2");
    }

    #[test]
    fn issuelinks_missing() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.issuelinks.is_none());
    }

    #[test]
    fn issuelinks_empty_array() {
        let json = json!({"summary": "test", "issuelinks": []});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.issuelinks.unwrap().len(), 0);
    }

    #[test]
    fn new_fields_present() {
        let json = json!({
            "summary": "test",
            "created": "2026-03-20T14:32:00.000+0000",
            "updated": "2026-03-25T09:15:22.000+0000",
            "reporter": {"accountId": "abc123", "displayName": "Jane Smith"},
            "resolution": {"name": "Fixed"},
            "components": [{"name": "Backend"}, {"name": "API"}],
            "fixVersions": [{"name": "v2.0", "released": false, "releaseDate": "2026-04-01"}]
        });
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(
            fields.created.as_deref(),
            Some("2026-03-20T14:32:00.000+0000")
        );
        assert_eq!(
            fields.updated.as_deref(),
            Some("2026-03-25T09:15:22.000+0000")
        );
        let reporter = fields.reporter.unwrap();
        assert_eq!(reporter.display_name, "Jane Smith");
        assert_eq!(reporter.account_id, "abc123");
        assert_eq!(fields.resolution.unwrap().name, "Fixed");
        let components = fields.components.unwrap();
        assert_eq!(components.len(), 2);
        assert_eq!(components[0].name, "Backend");
        assert_eq!(components[1].name, "API");
        let versions = fields.fix_versions.unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].name, "v2.0");
        assert_eq!(versions[0].released, Some(false));
        assert_eq!(versions[0].release_date.as_deref(), Some("2026-04-01"));
        // New typed fields should NOT appear in extra
        assert!(!fields.extra.contains_key("created"));
        assert!(!fields.extra.contains_key("updated"));
        assert!(!fields.extra.contains_key("reporter"));
        assert!(!fields.extra.contains_key("resolution"));
        assert!(!fields.extra.contains_key("components"));
        assert!(!fields.extra.contains_key("fixVersions"));
    }

    #[test]
    fn new_fields_absent() {
        let json = json!({"summary": "test"});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.created.is_none());
        assert!(fields.updated.is_none());
        assert!(fields.reporter.is_none());
        assert!(fields.resolution.is_none());
        assert!(fields.components.is_none());
        assert!(fields.fix_versions.is_none());
    }

    #[test]
    fn new_fields_null() {
        let json = json!({
            "summary": "test",
            "created": null,
            "updated": null,
            "reporter": null,
            "resolution": null,
            "components": null,
            "fixVersions": null
        });
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert!(fields.created.is_none());
        assert!(fields.updated.is_none());
        assert!(fields.reporter.is_none());
        assert!(fields.resolution.is_none());
        assert!(fields.components.is_none());
        assert!(fields.fix_versions.is_none());
    }

    #[test]
    fn components_empty_array() {
        let json = json!({"summary": "test", "components": []});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.components, Some(vec![]));
    }

    #[test]
    fn fix_versions_empty_array() {
        let json = json!({"summary": "test", "fixVersions": []});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        assert_eq!(fields.fix_versions, Some(vec![]));
    }

    #[test]
    fn version_optional_fields_absent() {
        let json = json!({"summary": "test", "fixVersions": [{"name": "v1.0"}]});
        let fields: IssueFields = serde_json::from_value(json).unwrap();
        let v = &fields.fix_versions.unwrap()[0];
        assert_eq!(v.name, "v1.0");
        assert!(v.released.is_none());
        assert!(v.release_date.is_none());
    }

    #[test]
    fn comment_deserialize_with_properties() {
        let json = json!({
            "id": "10001",
            "body": null,
            "properties": [
                {"key": "sd.public.comment", "value": {"internal": true}}
            ]
        });
        let comment: Comment = serde_json::from_value(json).unwrap();
        assert_eq!(comment.properties.len(), 1);
        assert_eq!(comment.properties[0].key, "sd.public.comment");
        assert_eq!(comment.properties[0].value["internal"], true);
    }

    #[test]
    fn comment_deserialize_without_properties() {
        let json = json!({
            "id": "10002",
            "body": null
        });
        let comment: Comment = serde_json::from_value(json).unwrap();
        assert!(comment.properties.is_empty());
    }

    #[test]
    fn comment_deserialize_empty_properties() {
        let json = json!({
            "id": "10003",
            "body": null,
            "properties": []
        });
        let comment: Comment = serde_json::from_value(json).unwrap();
        assert!(comment.properties.is_empty());
    }

    #[test]
    fn resolution_deserializes_full_shape_from_resolution_endpoint() {
        // GET /rest/api/3/resolution returns entries with id + name + description,
        // not just the {"name": "..."} shape that issue.fields.resolution uses.
        let json = r#"{
            "id": "10000",
            "name": "Done",
            "description": "Work has been completed.",
            "self": "https://example.atlassian.net/rest/api/3/resolution/10000"
        }"#;
        let r: Resolution = serde_json::from_str(json).unwrap();
        assert_eq!(r.id.as_deref(), Some("10000"));
        assert_eq!(r.name, "Done");
        assert_eq!(r.description.as_deref(), Some("Work has been completed."));
    }

    #[test]
    fn resolution_preserves_simple_shape_from_issue_fields() {
        // issue.fields.resolution comes back as {"name": "Fixed"} — no id/description.
        // Extending the struct must not break the existing path.
        let json = r#"{"name": "Fixed"}"#;
        let r: Resolution = serde_json::from_str(json).unwrap();
        assert_eq!(r.name, "Fixed");
        assert!(r.id.is_none());
        assert!(r.description.is_none());
    }
}
