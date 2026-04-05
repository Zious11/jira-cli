use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Queue {
    pub id: String,
    pub name: String,
    pub jql: Option<String>,
    pub fields: Option<Vec<String>>,
    #[serde(rename = "issueCount")]
    pub issue_count: Option<u64>,
}

/// Lightweight struct for extracting only the issue key from JSM queue issue
/// representations. The JSM queue endpoint returns issues containing only the
/// fields configured as queue columns, and we only need the key for the
/// two-step fetch (keys → search_issues).
#[derive(Debug, Default, Deserialize)]
pub struct QueueIssueKey {
    pub key: String,
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
    fn deserialize_queue_issue_key() {
        let json = r#"{
            "key": "HELPDESK-42",
            "fields": {
                "summary": "VPN not working",
                "status": { "name": "New" }
            }
        }"#;
        let issue_key: QueueIssueKey = serde_json::from_str(json).unwrap();
        assert_eq!(issue_key.key, "HELPDESK-42");
    }

    #[test]
    fn deserialize_queue_issue_key_ignores_extra_fields() {
        let json = r#"{
            "key": "SD-10",
            "id": "17227",
            "self": "https://example.atlassian.net/rest/api/2/issue/17227",
            "fields": {
                "summary": "Printer broken",
                "issuetype": null,
                "priority": null
            }
        }"#;
        let issue_key: QueueIssueKey = serde_json::from_str(json).unwrap();
        assert_eq!(issue_key.key, "SD-10");
    }
}
