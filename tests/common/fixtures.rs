use serde_json::{json, Value};

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

pub fn transitions_response(transitions: Vec<(&str, &str)>) -> Value {
    json!({
        "transitions": transitions.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>()
    })
}

pub fn error_response(messages: &[&str]) -> Value {
    json!({ "errorMessages": messages })
}
