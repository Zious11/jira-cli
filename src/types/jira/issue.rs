use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::User;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Issue {
    pub key: String,
    pub fields: IssueFields,
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
    pub project: Option<IssueProject>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl IssueFields {
    pub fn story_points(&self, field_id: &str) -> Option<f64> {
        self.extra.get(field_id)?.as_f64()
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Comment {
    pub id: Option<String>,
    pub body: Option<Value>,
    pub author: Option<User>,
    pub created: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateIssueResponse {
    pub key: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
