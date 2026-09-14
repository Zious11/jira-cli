//! JSM request-submission API client methods.
//!
//! Effectful module (L4) — HTTP calls via `api::client`. No business logic.
//! The single method wraps `POST /rest/servicedeskapi/request`.
//!
//! `JsmRequestBuilder` is a pure helper (no `JiraClient` dependency) for
//! constructing the POST body. It lives here so proptest properties (C.1–C.3)
//! can exercise it without a mock HTTP client.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;

use crate::adf::MentionResolutions;
use crate::api::client::JiraClient;
use crate::cli::issue::create::{FieldValueKind, FieldValueSpec};
use crate::error::JrError;
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
/// Per BC-3.8.006/BC-3.8.022: `isAdfRequest: true` is included if EITHER
/// `description` is `Some` OR the resolution layer ADF-converted at least
/// one `--field` extra field (`self.is_adf_request`) — ABSENT (never
/// explicit `false`) when neither is true. Per BC-3.8.009: `raiseOnBehalfOf`
/// is included if and only if `on_behalf_of` is `Some` (the key is
/// completely absent otherwise — NOT null). Per BC-3.8.007: `labels` is a
/// plain string array, NOT an object array.
///
/// Traces: BC-3.8.001, BC-3.8.005, BC-3.8.006, BC-3.8.007, BC-3.8.008,
///         BC-3.8.009, BC-3.8.019, BC-3.8.020, BC-3.8.021, BC-3.8.022
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
    /// S-cycle5-mention-resolution-wiring (AC-013, BC-3.8.018): when `true`,
    /// `description` is converted via `adf::markdown_to_adf_no_mentions`
    /// (skipping mention resolution entirely — the `--no-mentions` CLI
    /// flag). When `false`, `description` is converted via
    /// `adf::markdown_to_adf_with_mentions` using `mentions` (pre-resolved
    /// by the async caller BEFORE this synchronous, effect-free `build()`
    /// runs — `build()` gains no `async`/`Client` capability).
    pub no_mentions: bool,
    /// Pre-resolved mention data (`None` is treated as
    /// `MentionResolutions::empty()`, preserving pre-#674 byte-identical
    /// behavior for every caller — proptests included — that does not set
    /// this field).
    pub mentions: Option<&'a MentionResolutions>,
    /// S-578-3 (BC-3.8.008 amendment): `FieldValueSpec` map, not a plain
    /// `String` map — carries the shared `:option`/`:id`/`:name`/`:asset`
    /// hint-kind tag produced by `parse_field_kv` (S-578-1), so `build()`
    /// can dispatch kind-aware `requestFieldValues` serialization instead of
    /// the old unconditional string-wrap.
    pub(crate) extra_fields: &'a HashMap<String, FieldValueSpec>,
    /// ADF-converted values for non-empty bare (`kind.is_none()`) ADF-backed
    /// extra fields, keyed by field name (S-cycle12-jsm-adf-autoconvert
    /// AC-001/004/013, ADR-0024 DQ-6 Option (b) — see the DQ-6 decision
    /// rustdoc on [`crate::cli::issue::jsm_create::JsmAdfFieldResolution`]).
    /// Populated exclusively by the resolution layer (`jsm_create.rs`);
    /// `build()` inserts these into `requestFieldValues` (after the
    /// `extra_fields` loop), superseding any string-wrap for the same key —
    /// it never derives ADF-ness by inspecting value shapes itself (AC-013).
    /// Every pre-cycle-012 caller passes an empty map, preserving `build()`
    /// output byte-for-byte.
    pub(crate) resolved_adf_values: &'a BTreeMap<String, serde_json::Value>,
    /// Accumulated `isAdfRequest` contribution from the resolution layer's
    /// `--field` extra-field ADF conversions (S-cycle12-jsm-adf-autoconvert
    /// AC-001/013, BC-3.8.022). `build()` ORs this with its own
    /// `self.description`-derived flag — it never derives this flag by
    /// inspecting `requestFieldValues`/`extra_fields` value shapes
    /// (`.is_object()` derivation is the specific mutant class AC-013 exists
    /// to kill; see VP-FIELD-ADF-004 Axis (c) I-2). Every pre-cycle-012
    /// caller passes `false`, preserving `isAdfRequest` output byte-for-byte.
    pub(crate) is_adf_request: bool,
}

impl<'a> JsmRequestBuilder<'a> {
    /// Construct the JSM POST body from the builder fields.
    ///
    /// All business logic lives here — no free-standing function with > 7 args
    /// (satisfies `clippy::too_many_arguments` per CLAUDE.md policy).
    pub fn build(self) -> Result<serde_json::Value, JrError> {
        use crate::adf;
        use serde_json::json;

        // Build requestFieldValues — start with the mandatory summary (BC-3.8.005).
        let mut rfv = serde_json::Map::new();
        rfv.insert(
            "summary".to_string(),
            serde_json::Value::String(self.summary.to_string()),
        );

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
        //
        // Kind-aware dispatch (S-578-3, AC-002): `None`/`Some(Option)` is the
        // pre-existing bare-form string-wrap (AC-008/VP-578-015 byte-identity
        // regression pin). `Id`/`Name` are pure by-analogy wraps
        // (VP-578-016 parity-PENDING, see `compose_id_wire`/
        // `compose_name_wire`). `Asset` performs PURE array-wrapping ONLY of
        // an already-L2-resolved `WORKSPACE:OBJECTID` value — the caller
        // (`jsm_create.rs`, L2) resolves the workspace id BEFORE this
        // function ever sees the value (Architecture Compliance Rule 1).
        for (k, spec) in self.extra_fields {
            let wire_value = match spec.kind {
                None | Some(FieldValueKind::Option) => {
                    serde_json::Value::String(spec.value.clone())
                }
                Some(FieldValueKind::Id) => compose_id_wire(&spec.value),
                Some(FieldValueKind::Name) => compose_name_wire(&spec.value),
                Some(FieldValueKind::Asset) => compose_asset_wire(&spec.value),
            };
            rfv.insert(k.clone(), wire_value);
        }

        // S-cycle12-jsm-adf-autoconvert AC-001/004/013: insert the
        // resolution-layer's ADF-converted extra-field values, superseding
        // any string-wrap for the same key from the loop above. By
        // construction (`jsm_create.rs::resolve_jsm_adf_extra_fields`) these
        // keys never overlap `self.extra_fields`, but insertion order still
        // guarantees supersession if that invariant is ever violated.
        for (k, adf_value) in self.resolved_adf_values {
            rfv.insert(k.clone(), adf_value.clone());
        }

        // Optional description → ADF (BC-3.8.006).
        //
        // AC-006 (EC-3.8.019-4 assembly-order rule): this insert MUST run
        // AFTER the extra_fields loop (and the resolved_adf_values merge
        // above) so `self.description` deterministically supersedes any
        // `requestFieldValues["description"]` entry produced by the
        // resolution layer — including a fail-open `Value::String(Y)` under
        // EC-3.8.019-2. Only this insert moves; every other dedicated-flag
        // insert (summary/priority/labels) stays before the loop, preserving
        // last-wins for non-description keys (BC-3.8.008).
        let description_is_adf = if let Some(desc_text) = self.description {
            let adf_body = if self.markdown {
                if self.no_mentions {
                    adf::markdown_to_adf_no_mentions(desc_text)?
                } else {
                    let empty = MentionResolutions::empty();
                    let resolved = self.mentions.unwrap_or(&empty);
                    adf::markdown_to_adf_with_mentions(desc_text, resolved)?
                }
            } else {
                adf::text_to_adf(desc_text)
            };
            rfv.insert("description".to_string(), adf_body);
            true
        } else {
            false
        };

        // AC-001/013: isAdfRequest reflects the OR of the description channel
        // and the resolution layer's pre-computed flag — build() NEVER
        // derives it by inspecting requestFieldValues/extra_fields value
        // shapes (the specific mutant class AC-013 exists to kill; see
        // VP-FIELD-ADF-004 Axis (c) I-2).
        let is_adf_request = description_is_adf || self.is_adf_request;

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

        // isAdfRequest key is present only when true (BC-3.8.006); its value
        // is `is_adf_request`, computed above from BOTH the description
        // channel AND the resolution layer's pre-computed ADF flag — NOT
        // gated on description alone (see the comment at its computation).
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

        Ok(serde_json::Value::Object(body))
    }
}

/// `:id` kind-aware `requestFieldValues` composer (S-578-3, AC-002).
///
/// Shape (by analogy to `field_resolve.rs::compose_id_hint`, VP-578-016
/// parity-PENDING): `{"id": VALUE}`.
fn compose_id_wire(value: &str) -> serde_json::Value {
    serde_json::json!({"id": value})
}

/// `:name` kind-aware `requestFieldValues` composer (S-578-3, AC-002).
///
/// Shape (by analogy to `field_resolve.rs::compose_name_hint`, VP-578-016
/// parity-PENDING): `{"name": VALUE}`.
fn compose_name_wire(value: &str) -> serde_json::Value {
    serde_json::json!({"name": value})
}

/// `:asset` kind-aware `requestFieldValues` composer (S-578-3, AC-002/AC-006).
///
/// Performs PURE array-wrapping ONLY (Architecture Compliance Rule 1) — it
/// never calls `get_or_fetch_workspace_id` or otherwise reaches into
/// `api::assets::*`. Workspace-id resolution is owned exclusively by
/// `jsm_create.rs` (L2, Architecture Compliance Rule 2); by the time a value
/// reaches this function it is ALWAYS an already-resolved
/// `WORKSPACE:OBJECTID` pair (`jsm_create.rs::resolve_asset_field_l2`'s
/// output — explicit `WORKSPACE:OBJECTID` composed directly, or bare
/// `<objectId>` resolved via `get_or_fetch_workspace_id` before this
/// function ever runs).
///
/// Shape (by analogy to `field_resolve.rs::compose_asset_hint`, VP-578-016
/// parity-PENDING):
/// `[{"workspaceId":"<ws>","id":"<ws>:<objectId>","objectId":"<objectId>"}]`.
///
/// # Panics
///
/// If `value` does not contain a `:` — this is an internal invariant
/// violation (the L2 caller is required to always resolve/qualify the value
/// before it reaches `build()`), not a user-facing error condition.
fn compose_asset_wire(value: &str) -> serde_json::Value {
    let (workspace_id, object_id) = value.split_once(':').unwrap_or_else(|| {
        panic!(
            "compose_asset_wire: internal invariant violation — value '{value}' must already \
             be a resolved WORKSPACE:OBJECTID pair (resolve_asset_field_l2 must qualify it \
             before build() runs)"
        )
    });
    serde_json::json!([{
        "workspaceId": workspace_id,
        "id": format!("{workspace_id}:{object_id}"),
        "objectId": object_id
    }])
}

/// Proptest properties for [`JsmRequestBuilder`] (AC-014, BC-3.8.001..009).
///
/// Properties C.1–C.3 cover the three invariants from the verification delta.
#[cfg(test)]
mod proptests {
    use super::JsmRequestBuilder;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

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
                no_mentions: false,
                mentions: None,
                extra_fields: &extra,
                resolved_adf_values: &BTreeMap::new(),
                is_adf_request: false,
            }
            .build()
            .unwrap();
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
                no_mentions: false,
                mentions: None,
                extra_fields: &extra,
                resolved_adf_values: &BTreeMap::new(),
                is_adf_request: false,
            }
            .build()
            .unwrap();
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
                // AC-010 (VP-FIELD-ADF-004 pass-14 M-1 finding): the negative-case
                // ABSENT-check MUST use `.is_none()`, NOT
                // `.and_then(as_bool).unwrap_or(false)` — the lax form passes on
                // BOTH an absent key AND an explicit `false` value, so it cannot
                // kill a mutant that inserts `"isAdfRequest": false`.
                prop_assert!(
                    body.get("isAdfRequest").is_none(),
                    "C.2: BC-3.8.006 isAdfRequest must be ABSENT (NOT explicit false) when description is None"
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
                no_mentions: false,
                mentions: None,
                extra_fields: &extra,
                resolved_adf_values: &BTreeMap::new(),
                is_adf_request: false,
            }
            .build()
            .unwrap();

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
                no_mentions: false,
                mentions: None,
                extra_fields: &extra,
                resolved_adf_values: &BTreeMap::new(),
                is_adf_request: false,
            }
            .build()
            .unwrap();
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

/// S-cycle12-jsm-adf-autoconvert Red Gate: AC-006 (assembly-order fix, VP-FIELD-ADF-004
/// Axis g) and AC-013 (`build()` never derives `isAdfRequest` from value shapes,
/// VP-FIELD-ADF-004 Axis c I-2 discriminator-precision regression).
///
/// Both target `JsmRequestBuilder::build` directly — pure, no network, no
/// `RequestTypeField` metadata.
#[cfg(test)]
mod adf_build_tests {
    use super::JsmRequestBuilder;
    use crate::cli::issue::create::{FieldValueKind, FieldValueSpec};
    use std::collections::{BTreeMap, HashMap};

    /// AC-006 (EC-3.8.019-4 assembly-order rule, pass-13 MEDIUM-3,
    /// VP-FIELD-ADF-004 Axis g sub-case 1): `self.description`'s ADF value
    /// (the BC-3.8.006 channel) MUST supersede any `extra_fields`-loop write
    /// to `requestFieldValues["description"]` — `build()` must write the
    /// `self.description` insert AFTER the `extra_fields` loop, not before.
    ///
    /// Currently RED: `build()` writes `self.description`'s ADF BEFORE the
    /// `extra_fields` loop, so the loop's string-wrap for a `description`
    /// key overwrites it with `Value::String("Y")`.
    #[test]
    fn test_bc_3_8_019_build_description_supersedes_extra_field_description_entry() {
        let mut extra_fields = HashMap::new();
        extra_fields.insert(
            "description".to_string(),
            FieldValueSpec {
                kind: None,
                value: "Y".to_string(),
            },
        );
        let resolved_adf_values: BTreeMap<String, serde_json::Value> = BTreeMap::new();

        let body = JsmRequestBuilder {
            service_desk_id: "10",
            request_type_id: "11002",
            summary: "test",
            description: Some("text X"),
            markdown: false,
            priority: None,
            labels: &[],
            on_behalf_of: None,
            no_mentions: false,
            mentions: None,
            extra_fields: &extra_fields,
            resolved_adf_values: &resolved_adf_values,
            is_adf_request: false,
        }
        .build()
        .unwrap();

        let desc = body
            .get("requestFieldValues")
            .and_then(|rfv| rfv.get("description"))
            .expect("requestFieldValues.description must be present");
        assert_eq!(
            desc.get("type").and_then(serde_json::Value::as_str),
            Some("doc"),
            "EC-3.8.019-4 assembly-order rule: self.description's ADF value (from \
             \"text X\") must supersede the extra_fields['description'] string-wrap \
             entry (\"Y\"); got requestFieldValues.description: {desc}"
        );
    }

    /// AC-013 (VP-FIELD-ADF-004 Axis c I-2 discriminator-precision
    /// regression): `build()` must NEVER derive `isAdfRequest` by inspecting
    /// value shapes in `requestFieldValues`/`extra_fields` — it must reflect
    /// ONLY the explicit `is_adf_request` bool it receives. A hinted `:id`
    /// extra field serializes to a JSON OBJECT (`{"id": "123"}`), which a
    /// naive `self.extra_fields.values().any(|v| v.is_object())`-style
    /// derivation in `build()` would mistake for an ADF value. With
    /// `is_adf_request: false` passed explicitly and no ADF value present,
    /// `isAdfRequest` must be ABSENT even though `requestFieldValues`
    /// contains an object.
    #[test]
    fn test_bc_3_8_022_is_adf_request_absent_with_hinted_object_field_no_adf() {
        let mut extra_fields = HashMap::new();
        extra_fields.insert(
            "cf".to_string(),
            FieldValueSpec {
                kind: Some(FieldValueKind::Id),
                value: "123".to_string(),
            },
        );
        let resolved_adf_values: BTreeMap<String, serde_json::Value> = BTreeMap::new();

        let body = JsmRequestBuilder {
            service_desk_id: "10",
            request_type_id: "11002",
            summary: "test",
            description: None,
            markdown: false,
            priority: None,
            labels: &[],
            on_behalf_of: None,
            no_mentions: false,
            mentions: None,
            extra_fields: &extra_fields,
            resolved_adf_values: &resolved_adf_values,
            is_adf_request: false,
        }
        .build()
        .unwrap();

        let rfv = body
            .get("requestFieldValues")
            .and_then(serde_json::Value::as_object)
            .expect("requestFieldValues must exist");
        assert!(
            rfv.get("cf")
                .map(serde_json::Value::is_object)
                .unwrap_or(false),
            "sanity: the hinted ':id' field must serialize to a JSON object; got: {rfv:?}"
        );
        assert!(
            body.get("isAdfRequest").is_none(),
            "AC-013 I-2: isAdfRequest must be ABSENT — build() must not derive it by \
             inspecting requestFieldValues/extra_fields value shapes (a hinted-object \
             field is NOT ADF); got body: {body}"
        );
    }
}
