use serde::{Deserialize, Serialize};

use crate::types::jira::User;
use crate::types::jira::issue::{IssueType, Priority, Status};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Queue {
    pub id: String,
    pub name: String,
    pub jql: Option<String>,
    pub fields: Option<Vec<String>>,
    #[serde(rename = "issueCount")]
    pub issue_count: Option<u64>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct QueueIssue {
    pub key: String,
    pub fields: QueueIssueFields,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct QueueIssueFields {
    pub summary: Option<String>,
    pub status: Option<Status>,
    pub issuetype: Option<IssueType>,
    pub priority: Option<Priority>,
    pub assignee: Option<User>,
    pub reporter: Option<User>,
    pub created: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_queue_with_all_fields() {
        let json = r#"{
            "id": "10",
            "name": "Triage",
            "jql": "project = HELPDESK AND status = New",
            "fields": ["issuetype", "issuekey", "summary", "status"],
            "issueCount": 12
        }"#;
        let queue: Queue = serde_json::from_str(json).unwrap();
        assert_eq!(queue.id, "10");
        assert_eq!(queue.name, "Triage");
        assert_eq!(queue.issue_count, Some(12));
        assert!(queue.jql.is_some());
    }

    #[test]
    fn deserialize_queue_without_optional_fields() {
        let json = r#"{
            "id": "20",
            "name": "All open"
        }"#;
        let queue: Queue = serde_json::from_str(json).unwrap();
        assert_eq!(queue.id, "20");
        assert!(queue.issue_count.is_none());
        assert!(queue.jql.is_none());
        assert!(queue.fields.is_none());
    }

    #[test]
    fn deserialize_queue_issue_minimal() {
        let json = r#"{
            "key": "HELPDESK-42",
            "fields": {
                "summary": "VPN not working"
            }
        }"#;
        let issue: QueueIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.key, "HELPDESK-42");
        assert_eq!(issue.fields.summary.as_deref(), Some("VPN not working"));
        assert!(issue.fields.status.is_none());
        assert!(issue.fields.assignee.is_none());
    }

    #[test]
    fn deserialize_queue_issue_full() {
        let json = r#"{
            "key": "HELPDESK-42",
            "fields": {
                "summary": "VPN not working",
                "status": { "name": "New", "statusCategory": { "name": "To Do", "key": "new" } },
                "issuetype": { "name": "Service Request" },
                "priority": { "name": "High" },
                "assignee": { "accountId": "abc123", "displayName": "Jane D." },
                "reporter": { "accountId": "def456", "displayName": "John S." },
                "created": "2026-03-24T10:00:00.000+0000"
            }
        }"#;
        let issue: QueueIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.key, "HELPDESK-42");
        assert_eq!(issue.fields.status.as_ref().unwrap().name, "New");
        assert_eq!(
            issue.fields.assignee.as_ref().unwrap().display_name,
            "Jane D."
        );
    }
}
