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
            "duedate": "2027-07-30",
            "resolution": {"name": "Fixed"},
            "components": [{"name": "Backend"}, {"name": "API"}],
            "fixVersions": [{"name": "v2.0", "released": false, "releaseDate": "2026-04-01"}],
            "labels": ["bug"],
            "parent": null,
            "issuelinks": []
        }
    })
}

/// Issue response with `duedate` explicitly set to a given value, or JSON
/// `null` when `None`. Built on top of the minimal `issue_response` shape
/// (BC-2.2.032 / BC-2.3.039, issue #668).
pub fn issue_response_with_duedate(
    key: &str,
    summary: &str,
    status: &str,
    duedate: Option<&str>,
) -> Value {
    let mut response = issue_response(key, summary, status);
    response["fields"]["duedate"] = match duedate {
        Some(d) => json!(d),
        None => Value::Null,
    };
    response
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

/// Multi-project assignable user search response with emailAddress fields.
/// Like `multi_project_user_search_response` but each entry includes an
/// `emailAddress`, enabling BC-X.7.004 "email + accountId" candidate list
/// assertions (used in BC-8.1.006 ambiguous-lead tests).
pub fn multi_project_user_search_response_with_email(users: Vec<(&str, &str, &str)>) -> Value {
    let user_objects: Vec<Value> = users
        .into_iter()
        .map(|(account_id, display_name, email)| {
            json!({
                "accountId": account_id,
                "displayName": display_name,
                "emailAddress": email,
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

/// Write a pre-migrated `[profiles.default]`-shaped jr config to a temp
/// `XDG_CONFIG_HOME`, unlike the legacy `[instance]` shape some fixtures write.
///
/// Required by JSON-mode tests that strict-parse stderr via
/// `common::assertions::assert_json_error_envelope`: the legacy `[instance]`
/// shape triggers a one-time "Migrated config to multi-profile layout…" line
/// from `src/config.rs`, which would poison strict JSON parsing of stderr
/// (S-639-1).
pub fn write_profile_config(config_home: &std::path::Path, base_url: &str) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        format!(
            "default_profile = \"default\"\n[profiles.default]\nurl = \"{base_url}\"\nauth_method = \"api_token\"\n"
        ),
    )
    .unwrap();
}

// ── S-604-1: Component fixtures ──────────────────────────────────────────────

/// Build one component resource object as returned by
/// `GET /rest/api/3/project/{key}/components` and
/// `GET /rest/api/3/component/{id}`.
///
/// Includes all fields BC-8.1.002 lists as part of the component resource:
/// id, name, description, lead, assigneeType, project.  Fields passed as
/// `None` appear in the JSON as `null` (never omitted) — mirroring Jira's
/// actual response shape and making BC-8.1.002's "no field is dropped"
/// assertion testable.
pub fn component_response(
    id: &str,
    name: &str,
    description: Option<&str>,
    lead_name: Option<&str>,
    assignee_type: Option<&str>,
) -> Value {
    let lead = lead_name.map(|n| {
        json!({
            "accountId": format!("acc-{}", id),
            "displayName": n,
        })
    });
    json!({
        "id": id,
        "name": name,
        "description": description,
        "lead": lead,
        "assigneeType": assignee_type,
        "project": serde_json::Value::Null,
    })
}

/// Array response for `GET /rest/api/3/project/{key}/components`.
/// Jira returns a plain JSON array (no envelope wrapper).
pub fn component_list_response(components: Vec<Value>) -> Value {
    json!(components)
}

/// Response for `GET /rest/api/3/component/{id}/relatedIssueCounts`.
///
/// The live Jira Cloud endpoint returns `{"self": "…", "issueCount": N}` —
/// the `"id"` field is NOT present (F-3 fix: mock-vs-live drift). This fixture
/// exercises the real id-absent shape so tests catch any revert of the
/// `#[serde(default)]` guard on `RelatedIssueCounts.id`.
pub fn related_issue_counts_response(count: u64) -> Value {
    json!({ "issueCount": count })
}

/// Extended component response fixture supporting populated `project` and
/// `isAssigneeTypeValid` fields.
///
/// Used by F-B2 (adversarial pass-3 — counts JSON superset invariant) and
/// F-B3 (populated-project round-trip coverage).  All fields passed as `None`
/// behave identically to `component_response` — this fixture is a strict
/// superset of that one.
///
/// `is_assignee_type_valid: Some(true/false)` causes `"isAssigneeTypeValid"`
/// to appear in the JSON body so `Component.is_assignee_type_valid` deserializes
/// as `Some(...)` and is then re-emitted by the Component serializer (which has
/// `skip_serializing_if = "Option::is_none"`).
pub fn component_response_with_flags(
    id: &str,
    name: &str,
    description: Option<&str>,
    lead_name: Option<&str>,
    assignee_type: Option<&str>,
    project: Option<&str>,
    is_assignee_type_valid: Option<bool>,
) -> Value {
    let lead = lead_name.map(|n| {
        json!({
            "accountId": format!("acc-{}", id),
            "displayName": n,
        })
    });
    let project_value: Value = match project {
        Some(p) => json!(p),
        None => Value::Null,
    };
    let mut obj = json!({
        "id": id,
        "name": name,
        "description": description,
        "lead": lead,
        "assigneeType": assignee_type,
        "project": project_value,
    });
    if let Some(v) = is_assignee_type_valid {
        obj["isAssigneeTypeValid"] = json!(v);
    }
    obj
}

// ── S-604-2: Component create/edit fixtures ───────────────────────────────────

/// 201 response body returned by `POST /rest/api/3/component` on success.
///
/// Used by BC-8.1.005 create tests to satisfy the wiremock mock and to verify
/// the success output shape (`{"id","name","project"}`).
pub fn component_create_response(id: &str, name: &str, project_key: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": Value::Null,
        "lead": Value::Null,
        "assigneeType": Value::Null,
        "project": project_key,
    })
}

/// 200 response body returned by `PUT /rest/api/3/component/{id}` on success.
///
/// Used by BC-8.1.007 edit tests.
pub fn component_edit_response(id: &str, name: &str, project_key: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": Value::Null,
        "lead": Value::Null,
        "assigneeType": Value::Null,
        "project": project_key,
    })
}

/// Component list response with two components sharing the same name
/// case-insensitively (e.g. "Backend" and "backend").
///
/// Used by BC-X.10.003 ExactMultiple fail-closed tests (Finding A, PR#704):
/// `partial_match` returns `ExactMultiple` when two candidates match the input
/// exactly (case-insensitively).  The handler MUST refuse to pick one silently.
pub fn component_list_two_same_name(id1: &str, name1: &str, id2: &str, name2: &str) -> Value {
    json!([
        {
            "id": id1,
            "name": name1,
            "description": Value::Null,
            "lead": Value::Null,
            "assigneeType": Value::Null,
            "project": Value::Null,
        },
        {
            "id": id2,
            "name": name2,
            "description": Value::Null,
            "lead": Value::Null,
            "assigneeType": Value::Null,
            "project": Value::Null,
        }
    ])
}

/// Component GET body that deliberately omits the `"project"` key entirely.
///
/// Used by the BC-8.1.007 numeric-missing-project test (Finding C, PR#704):
/// when the confirming GET returns no project field AND the user supplies
/// `--project`, the handler must fail closed (exit 64) rather than silently
/// adopting the unverified `--project` value as the component's project.
///
/// Note: `#[serde(default)]` on `Component.project: Option<String>` treats
/// both an absent key and `"project": null` as `None`; this fixture uses
/// the absent-key form for maximum fidelity to the stated finding.
pub fn component_response_no_project_field(id: &str, name: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": Value::Null,
        "lead": Value::Null,
        "assigneeType": Value::Null,
        // "project" key intentionally absent — triggers the null-project path
    })
}

// ── S-604-3: Component delete-safety fixtures ─────────────────────────────────

/// Build one `/rest/api/3/search/jql` response page for the BC-8.2.007
/// pre-delete affected-issue snapshot (`component = <id> ORDER BY key ASC`).
///
/// Mirrors the minimal wire shape `tests/search_issue_keys.rs::jql_keys_response`
/// uses for the underlying `search_issue_keys`-style pagination the snapshot
/// reuses (top-level `key`, empty `fields {}`, `isLast` + `nextPageToken`
/// cursor metadata). Kept as a separate, S-604-3-scoped fixture rather than
/// exporting the private helper in `search_issue_keys.rs`, per that file's
/// own comment explaining the shape is intentionally minimal/local.
///
/// `next_page_token: None` → terminal page (`isLast: true`, no `nextPageToken`
/// key). `next_page_token: Some(token)` → non-terminal page carrying that
/// cursor for the next fetch.
pub fn component_delete_snapshot_page(keys: &[&str], next_page_token: Option<&str>) -> Value {
    let issues: Vec<Value> = keys
        .iter()
        .map(|k| {
            json!({
                "id": "90000",
                "key": k,
                "self": format!("https://example.atlassian.net/rest/api/3/issue/{}", k),
                "fields": {}
            })
        })
        .collect();
    let mut body = json!({
        "issues": issues,
        "isLast": next_page_token.is_none(),
    });
    if let Some(t) = next_page_token {
        body["nextPageToken"] = json!(t);
    }
    body
}

// ── S-608-1: `jr component rename` --all-projects fan-out fixtures ───────────

/// Build a `GET /rest/api/3/project/search` response containing one project
/// per given key (all `projectTypeKey: "software"`, no lead) — reduces
/// `project_response` boilerplate across BC-8.3.002/003/004/006's
/// `--all-projects` fan-out tests, which only care about the set of
/// accessible project KEYS `list_projects` returns, not project metadata.
pub fn projects_search_response_for_keys(keys: &[&str]) -> Value {
    let projects: Vec<Value> = keys
        .iter()
        .map(|k| project_response(k, &format!("Project {k}"), "software", None))
        .collect();
    project_search_response(projects)
}

/// Build a `GET /rest/api/3/project/search` response containing `n` projects
/// with keys `P0`..`P{n-1}` — used by AC-018's O(N)-scale fixture (S-608-1,
/// BC-8.3.002 Behavior "genuinely O(N) HTTP calls").
pub fn n_projects_search_response(n: u32) -> Value {
    let keys: Vec<String> = (0..n).map(|i| format!("P{i}")).collect();
    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    projects_search_response_for_keys(&key_refs)
}
