use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Top-level response from `GET /rest/api/3/issue/{key}/editmeta`.
///
/// Maps field IDs (e.g. `"customfield_10176"`) to their editmeta descriptors.
/// Used by `resolve_edit_fields` to validate field presence and resolve
/// `allowedValues` for option-type fields (BC-3.4.015, BC-3.4.016).
#[derive(Debug, Deserialize, Serialize)]
pub struct EditMeta {
    pub fields: HashMap<String, EditMetaField>,
}

/// Per-field descriptor from the editmeta response.
///
/// All five fields are structurally load-bearing for v1 resolution:
/// - `name`: used in error messages (edit-screen hint, operations-check hint).
/// - `schema.field_type`: drives type dispatch in `resolve_edit_fields` Step 4.
/// - `allowed_values`: option-value resolution in BC-3.4.016 Step 4a.
/// - `operations`: Step 3b — absence of `"set"` → exit 64 with hint.
/// - `required`: deserialized but not used in v1; retained for future
///   required-field validation. Add `#[allow(dead_code)]` ONLY if the compiler
///   warns — see prd-delta-396.md §5 P3-LOW-002.
#[derive(Debug, Deserialize, Serialize)]
pub struct EditMetaField {
    pub name: String,
    pub schema: EditMetaFieldSchema,
    /// CRITICAL rename: Jira API key is camelCase `"allowedValues"`. Without
    /// this annotation, the field always deserializes to `None`, causing
    /// BC-3.4.016 to fail with EC-3.4.016-3 on every valid option field.
    /// See prd-delta-396.md §5 OBS-1 and story AC-018.
    #[serde(rename = "allowedValues")]
    pub allowed_values: Option<Vec<AllowedValue>>,
    pub operations: Vec<String>,
    /// Future use: required-field validation. Retained to avoid dropping data
    /// returned by the Jira API. See prd-delta-396.md §5 P3-LOW-002.
    pub required: bool,
}

/// Schema descriptor for a field in the editmeta response.
///
/// `field_type` is the primary dispatch key in `resolve_edit_fields` Step 4.
/// Supported v1 values: `"string"`, `"number"`, `"option"`, `"date"`,
/// `"datetime"`, `"user"`. `"array"` and `"any"` → exit 64 with hint.
#[derive(Debug, Deserialize, Serialize)]
pub struct EditMetaFieldSchema {
    /// CRITICAL rename: Jira API key is `"type"` — a Rust keyword.
    #[serde(rename = "type")]
    pub field_type: String,
    /// Parsed from API response; not used in v1 resolution.
    pub system: Option<String>,
    /// Parsed from API response; not used in v1 resolution.
    pub custom: Option<String>,
}

/// A single allowed option value for a single-select (`option`) field.
///
/// Option-value resolution in BC-3.4.016 matches against `value` (case-
/// insensitive). `id` is placed on the wire as `{"id": "<id>"}`. `name` is
/// parsed but unused in v1 — retained for future cascade-select matching.
/// Add `#[allow(dead_code)]` on `name` ONLY if the compiler warns; see
/// prd-delta-396.md §5 O-2 amendment.
#[derive(Debug, Deserialize, Serialize)]
pub struct AllowedValue {
    pub id: String,
    /// Human-readable option label; used for case-insensitive matching.
    pub value: Option<String>,
    /// Secondary label present on some Jira option types (e.g. cascade-select
    /// children). Parsed from the API response; unused in v1 resolution logic.
    /// Future: v2 cascade-select name matching. See prd-delta-396.md §5 O-2.
    pub name: Option<String>,
}
