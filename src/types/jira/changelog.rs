use crate::types::jira::User;
use serde::{Deserialize, Serialize};

/// A single entry in an issue's changelog — one actor, one timestamp,
/// one or more field-level changes.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChangelogEntry {
    pub id: String,
    /// May be `null` for automation, workflow post-functions, or migrated data.
    #[serde(default)]
    pub author: Option<User>,
    /// ISO-8601 timestamp as returned by the API.
    pub created: String,
    #[serde(default)]
    pub items: Vec<ChangelogItem>,
}

/// A single field-level change within a `ChangelogEntry`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChangelogItem {
    pub field: String,
    pub fieldtype: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(rename = "fromString", default)]
    pub from_string: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(rename = "toString", default)]
    pub to_string: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_standard_entry() {
        let json = r#"{
            "id": "10000",
            "author": {
                "accountId": "abc",
                "displayName": "Alice",
                "emailAddress": "a@test.com",
                "active": true
            },
            "created": "2026-04-16T14:02:11.000+0000",
            "items": [
                {
                    "field": "status",
                    "fieldtype": "jira",
                    "from": "1",
                    "fromString": "To Do",
                    "to": "3",
                    "toString": "In Progress"
                }
            ]
        }"#;
        let entry: ChangelogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "10000");
        assert_eq!(entry.author.as_ref().unwrap().display_name, "Alice");
        assert_eq!(entry.items.len(), 1);
        assert_eq!(entry.items[0].field, "status");
        assert_eq!(entry.items[0].from_string.as_deref(), Some("To Do"));
        assert_eq!(entry.items[0].to_string.as_deref(), Some("In Progress"));
    }

    #[test]
    fn deserializes_null_author_for_automation() {
        let json = r#"{
            "id": "10001",
            "author": null,
            "created": "2026-04-14T11:10:00.000+0000",
            "items": [
                { "field": "assignee", "fieldtype": "jira",
                  "from": null, "fromString": null,
                  "to": "abc", "toString": "Alice" }
            ]
        }"#;
        let entry: ChangelogEntry = serde_json::from_str(json).unwrap();
        assert!(entry.author.is_none());
        assert_eq!(entry.items[0].from, None);
        assert_eq!(entry.items[0].from_string, None);
    }

    #[test]
    fn deserializes_missing_from_to_strings() {
        // fromString/toString may be absent entirely for some fields
        let json = r#"{
            "id": "10002",
            "author": {
                "accountId": "abc",
                "displayName": "Alice",
                "active": true
            },
            "created": "2026-04-15T09:00:00.000+0000",
            "items": [
                { "field": "labels", "fieldtype": "jira",
                  "from": "", "to": "backend" }
            ]
        }"#;
        let entry: ChangelogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.items[0].from_string, None);
        assert_eq!(entry.items[0].to_string, None);
    }

    #[test]
    fn deserializes_multiple_items_in_one_entry() {
        let json = r#"{
            "id": "10003",
            "author": { "accountId": "abc", "displayName": "Alice", "active": true },
            "created": "2026-04-16T14:02:11.000+0000",
            "items": [
                { "field": "status", "fieldtype": "jira",
                  "from": "1", "fromString": "To Do",
                  "to": "3", "toString": "Done" },
                { "field": "resolution", "fieldtype": "jira",
                  "from": null, "fromString": null,
                  "to": "10000", "toString": "Done" }
            ]
        }"#;
        let entry: ChangelogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.items.len(), 2);
        assert_eq!(entry.items[1].field, "resolution");
    }
}
