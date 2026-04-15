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

/// Transitions response with target status names.
/// Each tuple is (transition_id, transition_name, target_status_name).
pub fn transitions_response_with_status(transitions: Vec<(&str, &str, &str)>) -> Value {
    json!({
        "transitions": transitions.iter().map(|(id, name, status_name)| json!({
            "id": id,
            "name": name,
            "to": {"name": status_name}
        })).collect::<Vec<_>>()
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

/// User search response — flat array of User objects.
pub fn user_search_response(users: Vec<(&str, &str, bool)>) -> Value {
    let user_objects: Vec<Value> = users
        .into_iter()
        .map(|(account_id, display_name, active)| {
            json!({
                "accountId": account_id,
                "displayName": display_name,
                "emailAddress": format!("{}@test.com", display_name.to_lowercase().replace(' ', ".")),
                "active": active,
            })
        })
        .collect();
    json!(user_objects)
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

/// Project search response — paginated envelope with `values` array.
pub fn project_search_response(projects: Vec<Value>) -> Value {
    let total = projects.len() as u32;
    json!({
        "values": projects,
        "startAt": 0,
        "maxResults": 50,
        "total": total,
    })
}

pub fn project_response(key: &str, name: &str, type_key: &str, lead_name: Option<&str>) -> Value {
    let lead = lead_name.map(|name| {
        json!({
            "accountId": format!("acc-{}", key.to_lowercase()),
            "displayName": name,
        })
    });
    json!({
        "key": key,
        "name": name,
        "projectTypeKey": type_key,
        "lead": lead,
    })
}

/// Project statuses response — top-level array of issue types with nested statuses.
pub fn project_statuses_response() -> Value {
    json!([
        {
            "id": "3",
            "name": "Task",
            "self": "https://test.atlassian.net/rest/api/3/issueType/3",
            "subtask": false,
            "statuses": [
                {
                    "id": "10000",
                    "name": "To Do",
                    "description": "Work that has not been started.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/open.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10000"
                },
                {
                    "id": "10001",
                    "name": "In Progress",
                    "description": "The issue is currently being worked on.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/inprogress.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10001"
                },
                {
                    "id": "10002",
                    "name": "Done",
                    "description": "Work has been completed.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/closed.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10002"
                }
            ]
        },
        {
            "id": "1",
            "name": "Bug",
            "self": "https://test.atlassian.net/rest/api/3/issueType/1",
            "subtask": false,
            "statuses": [
                {
                    "id": "10000",
                    "name": "To Do",
                    "description": "Work that has not been started.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/open.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10000"
                },
                {
                    "id": "10002",
                    "name": "Done",
                    "description": "Work has been completed.",
                    "iconUrl": "https://test.atlassian.net/images/icons/statuses/closed.png",
                    "self": "https://test.atlassian.net/rest/api/3/status/10002"
                }
            ]
        }
    ])
}

/// Board configuration response.
pub fn board_config_response(board_type: &str) -> Value {
    json!({
        "id": 382,
        "name": "Test Board",
        "type": board_type
    })
}

/// Sprint list response (offset-paginated).
pub fn sprint_list_response(sprints: Vec<Value>) -> Value {
    let total = sprints.len() as u32;
    json!({
        "startAt": 0,
        "maxResults": 50,
        "total": total,
        "values": sprints
    })
}

/// Single sprint object.
pub fn sprint(id: u64, name: &str, state: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "state": state,
        "startDate": "2026-03-20T00:00:00.000Z",
        "endDate": "2026-04-03T00:00:00.000Z"
    })
}

/// Sprint issues response (offset-paginated).
pub fn sprint_issues_response(issues: Vec<Value>, total: u32) -> Value {
    json!({
        "startAt": 0,
        "maxResults": 50,
        "total": total,
        "issues": issues
    })
}

pub fn board_response(id: u64, name: &str, board_type: &str, project_key: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "type": board_type,
        "location": {
            "projectKey": project_key,
            "projectName": format!("{} Project", project_key)
        }
    })
}

pub fn board_list_response(boards: Vec<Value>) -> Value {
    let total = boards.len() as u32;
    json!({
        "values": boards,
        "startAt": 0,
        "maxResults": 50,
        "total": total
    })
}

/// Issue response with a specific assignee (or null if None).
pub fn issue_response_with_assignee(
    key: &str,
    summary: &str,
    assignee: Option<(&str, &str)>,
) -> Value {
    let assignee_value = match assignee {
        Some((account_id, display_name)) => json!({
            "accountId": account_id,
            "displayName": display_name,
        }),
        None => Value::Null,
    };
    json!({
        "key": key,
        "fields": {
            "summary": summary,
            "status": {"name": "To Do"},
            "issuetype": {"name": "Task"},
            "priority": {"name": "Medium"},
            "assignee": assignee_value,
            "project": {"key": key.split('-').next().unwrap_or("TEST")}
        }
    })
}

pub fn issue_response_with_standard_fields(key: &str, summary: &str) -> Value {
    json!({
        "key": key,
        "fields": {
            "summary": summary,
            "status": {"name": "In Progress", "statusCategory": {"name": "In Progress", "key": "indeterminate"}},
            "issuetype": {"name": "Bug"},
            "priority": {"name": "High"},
            "assignee": {"accountId": "abc123", "displayName": "John Doe"},
            "reporter": {"accountId": "def456", "displayName": "Jane Smith"},
            "project": {"key": key.split('-').next().unwrap_or("TEST"), "name": "Test Project"},
            "created": "2026-03-20T14:32:00.000+0000",
            "updated": "2026-03-25T09:15:22.000+0000",
            "resolution": {"name": "Fixed"},
            "components": [{"name": "Backend"}, {"name": "API"}],
            "fixVersions": [{"name": "v2.0", "released": false, "releaseDate": "2026-04-01"}],
            "labels": ["bug"],
            "parent": null,
            "issuelinks": []
        }
    })
}

pub fn issue_response_with_labels_parent_links(key: &str, summary: &str) -> Value {
    json!({
        "key": key,
        "fields": {
            "summary": summary,
            "status": {"name": "To Do"},
            "issuetype": {"name": "Story"},
            "priority": {"name": "Medium"},
            "assignee": {"accountId": "abc123", "displayName": "Test User"},
            "project": {"key": key.split('-').next().unwrap_or("TEST")},
            "labels": ["bug", "frontend"],
            "parent": {"key": "FOO-1", "fields": {"summary": "Parent Epic"}},
            "issuelinks": [
                {
                    "id": "30001",
                    "type": {"name": "Blocks", "inward": "is blocked by", "outward": "blocks"},
                    "outwardIssue": {"key": "FOO-3", "fields": {"summary": "Blocked issue"}}
                }
            ]
        }
    })
}

/// Multi-project assignable user search response — flat array of User objects.
/// Simpler than `user_search_response`: takes (account_id, display_name) pairs
/// and always sets active=true. No email field generated.
pub fn multi_project_user_search_response(users: Vec<(&str, &str)>) -> Value {
    let user_objects: Vec<Value> = users
        .into_iter()
        .map(|(account_id, display_name)| {
            json!({
                "accountId": account_id,
                "displayName": display_name,
                "active": true,
            })
        })
        .collect();
    json!(user_objects)
}

/// Create issue response.
pub fn create_issue_response(key: &str) -> Value {
    json!({
        "id": "10001",
        "key": key,
        "self": format!("https://test.atlassian.net/rest/api/3/issue/{}", key)
    })
}

/// Issue response with a team custom field set to a UUID string.
pub fn issue_response_with_team(
    key: &str,
    summary: &str,
    team_field_id: &str,
    team_uuid: &str,
) -> Value {
    let mut response = issue_response(key, summary, "To Do");
    response["fields"][team_field_id] = json!(team_uuid);
    response
}
