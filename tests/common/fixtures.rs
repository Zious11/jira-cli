use serde_json::{Value, json};

pub fn user_response() -> Value {
    json!({
        "accountId": "abc123",
        "displayName": "Test User",
        "emailAddress": "test@test.com"
    })
}

pub fn issue_response(key: &str, summary: &str, status: &str) -> Value {
    json!({
        "key": key,
        "fields": {
            "summary": summary,
            "status": {"name": status},
            "issuetype": {"name": "Task"},
            "priority": {"name": "Medium"},
            "assignee": {"accountId": "abc123", "displayName": "Test User"},
            "project": {"key": key.split('-').next().unwrap_or("TEST")}
        }
    })
}

pub fn issue_search_response(issues: Vec<Value>) -> Value {
    json!({ "issues": issues, "nextPageToken": Value::Null })
}

/// Search response with `nextPageToken` set (indicating more results exist).
pub fn issue_search_response_with_next_page(issues: Vec<Value>) -> Value {
    json!({ "issues": issues, "nextPageToken": "next-page-token-abc" })
}

/// Response for the approximate-count endpoint.
pub fn approximate_count_response(count: u64) -> Value {
    json!({ "count": count })
}

pub fn transitions_response(transitions: Vec<(&str, &str)>) -> Value {
    json!({
        "transitions": transitions.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>()
    })
}

pub fn error_response(messages: &[&str]) -> Value {
    json!({ "errorMessages": messages })
}

pub fn graphql_org_metadata_json() -> Value {
    json!({
        "data": {
            "tenantContexts": [
                { "orgId": "test-org-id-456", "cloudId": "test-cloud-id-123" }
            ]
        }
    })
}

pub fn issue_response_with_points(
    key: &str,
    summary: &str,
    status: &str,
    points: Option<f64>,
) -> Value {
    let mut fields = json!({
        "summary": summary,
        "status": {
            "name": status,
            "statusCategory": {"name": status, "key": if status == "Done" { "done" } else { "new" }}
        },
        "issuetype": {"name": "Story"},
        "priority": {"name": "Medium"},
        "assignee": {"accountId": "abc123", "displayName": "Test User"},
        "project": {"key": key.split('-').next().unwrap_or("TEST")}
    });
    if let Some(pts) = points {
        fields["customfield_10031"] = json!(pts);
    }
    json!({
        "key": key,
        "fields": fields
    })
}

pub fn fields_response_with_story_points() -> Value {
    json!([
        {
            "id": "summary",
            "name": "Summary",
            "custom": false,
            "schema": {"type": "string"}
        },
        {
            "id": "customfield_10031",
            "name": "Story Points",
            "custom": true,
            "schema": {
                "type": "number",
                "custom": "com.atlassian.jira.plugin.system.customfieldtypes:float",
                "customId": 10031
            }
        }
    ])
}

pub fn link_types_response() -> Value {
    json!({
        "issueLinkTypes": [
            {
                "id": "1000",
                "name": "Blocks",
                "inward": "is blocked by",
                "outward": "blocks"
            },
            {
                "id": "1001",
                "name": "Duplicate",
                "inward": "is duplicated by",
                "outward": "duplicates"
            },
            {
                "id": "1002",
                "name": "Relates",
                "inward": "relates to",
                "outward": "relates to"
            }
        ]
    })
}

pub fn issue_with_links_response(key: &str, summary: &str) -> Value {
    json!({
        "key": key,
        "fields": {
            "summary": summary,
            "status": {"name": "To Do"},
            "issuetype": {"name": "Story"},
            "priority": {"name": "Medium"},
            "assignee": {"accountId": "abc123", "displayName": "Test User"},
            "project": {"key": key.split('-').next().unwrap_or("TEST")},
            "parent": {"key": "FOO-1", "fields": {"summary": "Parent Epic"}},
            "issuelinks": [
                {
                    "id": "20001",
                    "type": {"name": "Blocks", "inward": "is blocked by", "outward": "blocks"},
                    "outwardIssue": {"key": "FOO-3", "fields": {"summary": "Blocked issue"}}
                }
            ]
        }
    })
}

pub fn teams_list_json() -> Value {
    json!({
        "entities": [
            { "teamId": "team-uuid-alpha", "displayName": "Alpha Team" },
            { "teamId": "team-uuid-beta", "displayName": "Beta Team" },
            { "teamId": "team-uuid-security", "displayName": "Security Engineering" }
        ],
        "cursor": null
    })
}
