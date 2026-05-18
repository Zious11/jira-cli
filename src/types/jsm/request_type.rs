//! Serde structs for JSM request-type and request-submission API response shapes.
//!
//! Pure module — no I/O, no `JiraClient` imports, no `async` functions.
//! All field names follow the Atlassian API camelCase shape via
//! `#[serde(rename_all = "camelCase")]`.

use serde::{Deserialize, Serialize};

/// A JSM request type associated with a service desk.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestType {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub help_text: Option<String>,
    pub issue_type_id: Option<String>,
    #[serde(default)]
    pub group_ids: Vec<String>,
}

/// A single field definition for a JSM request type.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestTypeField {
    pub field_id: String,
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
    /// Whether this field is visible to the customer in the portal form.
    /// Consumers (pr2 CLI rendering) use this to decide whether to prompt for the field.
    #[serde(default)]
    pub visible: bool,
    pub default_values: Option<Vec<serde_json::Value>>,
    pub valid_values: Option<Vec<serde_json::Value>>,
    pub jira_schema: serde_json::Value,
}

/// Response envelope for `GET .../requesttype/{id}/field`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestTypeFieldsResponse {
    pub can_raise_on_behalf_of: bool,
    pub can_add_request_participants: bool,
    pub request_type_fields: Vec<RequestTypeField>,
}

/// Typed POST body for `create_jsm_request`.
///
/// Serializes to the shape expected by `POST /rest/servicedeskapi/request`.
/// For richer request bodies (e.g. ADF descriptions, `isAdfRequest: true`), pass
/// a `serde_json::Value` directly to `create_jsm_request` instead — this struct
/// covers the minimal fields required by the Atlassian API.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsmRequest {
    pub service_desk_id: String,
    pub request_type_id: String,
    pub request_field_values: serde_json::Value,
}

/// Response from `POST /rest/servicedeskapi/request` on success (HTTP 201).
///
/// Callers (pr4 dispatch) will extract `issue_key` to produce `{"key": "<issue_key>"}`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsmRequestCreated {
    /// The Jira issue key of the created request (e.g. `"SD-42"`).
    pub issue_key: String,
    /// The numeric Jira issue ID; optional — deserializes without failure if absent.
    pub issue_id: Option<String>,
}
