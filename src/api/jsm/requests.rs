//! JSM request-submission API client methods.
//!
//! Effectful module (L4) — HTTP calls via `api::client`. No business logic.
//! The single method wraps `POST /rest/servicedeskapi/request`.
//!
//! `JsmRequestBuilder` is a pure helper (no `JiraClient` dependency) for
//! constructing the POST body. It lives here so proptest properties (C.1–C.3)
//! can exercise it without a mock HTTP client.

use std::collections::HashMap;

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::types::jsm::JsmRequestCreated;

impl JiraClient {
    /// Submit a new JSM customer request.
    ///
    /// POSTs `body` to `/rest/servicedeskapi/request` and deserializes the
    /// HTTP 201 response into [`JsmRequestCreated`].
    ///
    /// Traces: BC-3.8.001
    pub async fn create_jsm_request(&self, body: serde_json::Value) -> Result<JsmRequestCreated> {
        self.post_to_instance("/rest/servicedeskapi/request", &body)
            .await
    }
}

/// Builder for the POST body sent to `POST /rest/servicedeskapi/request`.
///
/// Pure helper — no HTTP calls, no `JiraClient` dependency. Assembles the
/// `requestFieldValues` map from caller-supplied fields and wraps it with
/// the top-level `serviceDeskId`, `requestTypeId`, and (when provided)
/// `raiseOnBehalfOf`.
///
/// # Body shape
///
/// ```json
/// {
///   "serviceDeskId": "<service_desk_id>",
///   "requestTypeId": "<request_type_id>",
///   "requestFieldValues": {
///     "summary": "<summary>",
///     // optional: "description": <ADF root object>,
///     // optional: "priority": {"name": "<priority>"},
///     // optional: "labels": ["<label>", ...],
///     // any extra fields from --field NAME=VALUE pairs
///   },
///   // optional (top-level, NOT in requestFieldValues):
///   "isAdfRequest": true,
///   "raiseOnBehalfOf": "<accountId>"
/// }
/// ```
///
/// Per BC-3.8.006: `isAdfRequest: true` is included if and only if
/// `description` is `Some`. Per BC-3.8.009: `raiseOnBehalfOf` is included
/// if and only if `on_behalf_of` is `Some` (the key is completely absent
/// otherwise — NOT null). Per BC-3.8.007: `labels` is a plain string array,
/// NOT an object array.
///
/// Traces: BC-3.8.001, BC-3.8.005, BC-3.8.006, BC-3.8.007, BC-3.8.008,
///         BC-3.8.009
pub struct JsmRequestBuilder<'a> {
    pub service_desk_id: &'a str,
    pub request_type_id: &'a str,
    pub summary: &'a str,
    pub description: Option<&'a str>,
    /// When true, convert `description` via `markdown_to_adf`; otherwise use `text_to_adf`.
    pub markdown: bool,
    pub priority: Option<&'a str>,
    pub labels: &'a [String],
    pub on_behalf_of: Option<&'a str>,
    pub extra_fields: &'a HashMap<String, String>,
}

impl<'a> JsmRequestBuilder<'a> {
    /// Construct the JSM POST body from the builder fields.
    ///
    /// All business logic lives here — no free-standing function with > 7 args
    /// (satisfies `clippy::too_many_arguments` per CLAUDE.md policy).
    pub fn build(self) -> serde_json::Value {
        use crate::adf;
        use serde_json::json;

        // Build requestFieldValues — start with the mandatory summary (BC-3.8.005).
        let mut rfv = serde_json::Map::new();
        rfv.insert(
            "summary".to_string(),
            serde_json::Value::String(self.summary.to_string()),
        );

        // Optional description → ADF (BC-3.8.006).
        let is_adf_request = if let Some(desc_text) = self.description {
            let adf_body = if self.markdown {
                adf::markdown_to_adf(desc_text)
            } else {
                adf::text_to_adf(desc_text)
            };
            rfv.insert("description".to_string(), adf_body);
            true
        } else {
            false
        };

        // Optional priority → {"name": "<priority>"} (BC-3.8.007).
        if let Some(prio) = self.priority {
            rfv.insert("priority".to_string(), json!({"name": prio}));
        }

        // Optional labels → plain string array (BC-3.8.007).
        // BC-3.8.007 confirmed: plain strings, NOT object array.
        if !self.labels.is_empty() {
            rfv.insert("labels".to_string(), json!(self.labels));
        }

        // Merge extra fields from --field NAME=VALUE (BC-3.8.008, last-wins).
        for (k, v) in self.extra_fields {
            rfv.insert(k.clone(), serde_json::Value::String(v.clone()));
        }

        // Assemble top-level body.
        let mut body = serde_json::Map::new();
        body.insert(
            "serviceDeskId".to_string(),
            serde_json::Value::String(self.service_desk_id.to_string()),
        );
        body.insert(
            "requestTypeId".to_string(),
            serde_json::Value::String(self.request_type_id.to_string()),
        );
        body.insert(
            "requestFieldValues".to_string(),
            serde_json::Value::Object(rfv),
        );

        // isAdfRequest only when description is present (BC-3.8.006).
        if is_adf_request {
            body.insert("isAdfRequest".to_string(), serde_json::Value::Bool(true));
        }

        // raiseOnBehalfOf only when provided — key completely absent otherwise (BC-3.8.009).
        if let Some(obo) = self.on_behalf_of {
            body.insert(
                "raiseOnBehalfOf".to_string(),
                serde_json::Value::String(obo.to_string()),
            );
        }

        serde_json::Value::Object(body)
    }
}

/// Proptest properties for [`JsmRequestBuilder`] (AC-014, BC-3.8.001..009).
///
/// Properties C.1–C.3 cover the three invariants from the verification delta.
#[cfg(test)]
mod proptests {
    use super::JsmRequestBuilder;
    use proptest::prelude::*;

    proptest! {
        /// C.1 (BC-3.8.005): `summary` is always present in `requestFieldValues`
        /// and equals the passed-in `summary` argument.
        #[test]
        fn prop_build_jsm_request_body_summary_always_present(
            service_desk_id in "[0-9]{1,5}",
            request_type_id in "[0-9]{1,5}",
            summary in ".{1,100}",
        ) {
            let extra = std::collections::HashMap::new();
            let body = JsmRequestBuilder {
                service_desk_id: &service_desk_id,
                request_type_id: &request_type_id,
                summary: &summary,
                description: None,
                markdown: false,
                priority: None,
                labels: &[],
                on_behalf_of: None,
                extra_fields: &extra,
            }
            .build();
            let rfv_summary = body
                .get("requestFieldValues")
                .and_then(|rfv| rfv.get("summary"))
                .and_then(serde_json::Value::as_str);
            prop_assert_eq!(
                rfv_summary,
                Some(summary.as_str()),
                "C.1: BC-3.8.005 summary must always appear in requestFieldValues"
            );
        }

        /// C.2 (BC-3.8.006): When `description` is `Some`, the body must include
        /// `isAdfRequest: true` AND `requestFieldValues.description` must be a
        /// JSON object (ADF root). When `description` is `None`, both must be absent.
        #[test]
        fn prop_build_jsm_request_body_description_adf_presence(
            service_desk_id in "[0-9]{1,5}",
            request_type_id in "[0-9]{1,5}",
            summary in "[a-z ]{1,40}",
            desc in "[a-z ]{1,40}",
            has_desc in any::<bool>(),
        ) {
            let extra = std::collections::HashMap::new();
            let description = if has_desc { Some(desc.as_str()) } else { None };
            let body = JsmRequestBuilder {
                service_desk_id: &service_desk_id,
                request_type_id: &request_type_id,
                summary: &summary,
                description,
                markdown: false,
                priority: None,
                labels: &[],
                on_behalf_of: None,
                extra_fields: &extra,
            }
            .build();
            if has_desc {
                prop_assert_eq!(
                    body.get("isAdfRequest").and_then(serde_json::Value::as_bool),
                    Some(true),
                    "C.2: BC-3.8.006 isAdfRequest must be true when description is Some"
                );
                let desc_val = body.get("requestFieldValues").and_then(|rfv| rfv.get("description"));
                prop_assert!(
                    desc_val.map(|d| d.is_object()).unwrap_or(false),
                    "C.2: BC-3.8.006 description must be ADF object when Some; got: {:?}",
                    desc_val
                );
            } else {
                let is_adf = body.get("isAdfRequest").and_then(serde_json::Value::as_bool).unwrap_or(false);
                prop_assert!(
                    !is_adf,
                    "C.2: BC-3.8.006 isAdfRequest must be absent/false when description is None"
                );
                let rfv_desc = body.get("requestFieldValues").and_then(|rfv| rfv.get("description"));
                prop_assert!(
                    rfv_desc.is_none(),
                    "C.2: BC-3.8.006 requestFieldValues.description must be absent when None; got: {:?}",
                    rfv_desc
                );
            }
        }

        /// C.4 (adversary pass-03 M-02): BC-3.8.001 — serviceDeskId and requestTypeId
        /// MUST be top-level string fields in the request body, NOT inside requestFieldValues.
        /// Regression guard for any refactor that relocates either field to requestFieldValues
        /// (which would cause Atlassian to reject the request with a 4xx).
        #[test]
        fn prop_build_jsm_request_body_top_level_ids(
            sid in "[0-9]{1,6}",
            rtid in "[0-9]{1,6}",
            summary in "[a-zA-Z0-9 ]{1,40}",
        ) {
            let extra = std::collections::HashMap::new();
            let body = JsmRequestBuilder {
                service_desk_id: &sid,
                request_type_id: &rtid,
                summary: &summary,
                description: None,
                markdown: false,
                priority: None,
                labels: &[],
                on_behalf_of: None,
                extra_fields: &extra,
            }
            .build();

            // Top-level pin
            prop_assert_eq!(body.get("serviceDeskId").and_then(serde_json::Value::as_str), Some(sid.as_str()));
            prop_assert_eq!(body.get("requestTypeId").and_then(serde_json::Value::as_str), Some(rtid.as_str()));

            // Negative-space pin: must NOT appear inside requestFieldValues
            let rfv = body.get("requestFieldValues").and_then(serde_json::Value::as_object)
                .expect("requestFieldValues must exist");
            prop_assert!(
                !rfv.contains_key("serviceDeskId"),
                "BC-3.8.001: serviceDeskId MUST NOT appear inside requestFieldValues; got body: {body}"
            );
            prop_assert!(
                !rfv.contains_key("requestTypeId"),
                "BC-3.8.001: requestTypeId MUST NOT appear inside requestFieldValues; got body: {body}"
            );
        }

        /// C.3 (BC-3.8.009): When `on_behalf_of` is `Some`, `raiseOnBehalfOf` is
        /// present at the top level of the body. When `None`, the key is completely
        /// absent (NOT null).
        #[test]
        fn prop_build_jsm_request_body_raise_on_behalf_of_presence(
            service_desk_id in "[0-9]{1,5}",
            request_type_id in "[0-9]{1,5}",
            summary in "[a-z ]{1,40}",
            account_id in "[a-z0-9:]{1,30}",
            has_obo in any::<bool>(),
        ) {
            let extra = std::collections::HashMap::new();
            let on_behalf_of = if has_obo { Some(account_id.as_str()) } else { None };
            let body = JsmRequestBuilder {
                service_desk_id: &service_desk_id,
                request_type_id: &request_type_id,
                summary: &summary,
                description: None,
                markdown: false,
                priority: None,
                labels: &[],
                on_behalf_of,
                extra_fields: &extra,
            }
            .build();
            if has_obo {
                prop_assert_eq!(
                    body.get("raiseOnBehalfOf").and_then(serde_json::Value::as_str),
                    Some(account_id.as_str()),
                    "C.3: BC-3.8.009 raiseOnBehalfOf must equal accountId when Some"
                );
                // M-03 (adversary pass-01): negative-space pin — raiseOnBehalfOf must be at
                // the TOP level of the body, NEVER inside requestFieldValues. BC-3.8.009.
                let rfv = body
                    .get("requestFieldValues")
                    .and_then(serde_json::Value::as_object)
                    .expect("C.3 M-03: requestFieldValues must exist");
                prop_assert!(
                    !rfv.contains_key("raiseOnBehalfOf"),
                    "C.3 M-03: BC-3.8.009 raiseOnBehalfOf MUST NOT appear inside requestFieldValues; got body: {body:?}"
                );
            } else {
                prop_assert!(
                    body.get("raiseOnBehalfOf").is_none(),
                    "C.3: BC-3.8.009 raiseOnBehalfOf must be completely absent when None; got body: {body:?}"
                );
            }
        }
    }
}
