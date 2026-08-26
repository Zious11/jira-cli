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
    /// Present for dynamic/lookup fields (user-picker, labels, etc.) whose
    /// options are resolved live via a suggestion endpoint rather than
    /// enumerated in `allowedValues`. Absent on fixed-value-set fields.
    /// Consumed by `jr field options`'s BC-X.14.004 graceful-degrade hint
    /// (AC-014: "+ autoCompleteUrl if present in the response").
    #[serde(rename = "autoCompleteUrl", default)]
    pub auto_complete_url: Option<String>,
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
///
/// `id` is `Option<String>` because the Jira Cloud OpenAPI schema for
/// `allowedValues` entries has no required properties. GDPR-era user/group
/// picker fields carry `accountId` instead of `id`, so `id` may be absent.
/// A `None` id causes the entry to be excluded from the numeric id-bypass
/// predicate and triggers exit 64 if the entry is matched at the wire-emission
/// site (EC-3.4.016-8). See issue #589 and BC-3.4.015/BC-3.4.016.
#[derive(Debug, Deserialize, Serialize)]
pub struct AllowedValue {
    pub id: Option<String>,
    /// Human-readable option label; used for case-insensitive matching.
    pub value: Option<String>,
    /// Secondary label present on some Jira option types (e.g. cascade-select
    /// children). Parsed from the API response; unused in v1 resolution logic.
    /// Future: v2 cascade-select name matching. See prd-delta-396.md §5 O-2.
    pub name: Option<String>,
    /// Cascading child options (cascading-select fields). Additive, behavior-
    /// preserving extension: `#[serde(default)]` means any existing caller
    /// deserializing an `AllowedValue` without a `children` key is unaffected
    /// (defaults to an empty `Vec`). Added by S-580-1 (`jr field options`,
    /// AC-010, ADR-0019 §Amendment D4) for the READ-side cascading option
    /// enumeration normalizer (`cli::field::normalize_from_allowed_values`);
    /// D4 also reserves this field for S-578-2's WRITE-side `:option`
    /// composer, which is a separate, later consumer of the same shape.
    #[serde(default)]
    pub children: Vec<AllowedValue>,
}

/// S-578-2 AC-011: `AllowedValue.children` serde round-trip.
///
/// Wire-absent `children` key and wire-present-but-empty `"children": []`
/// both deserialize to `Vec::new()` — an identical "no cascading children"
/// semantic, per ADR-0019 § Amendment D4 (`Vec`, NOT `Option<Vec<_>>`,
/// because the two wire forms carry no distinguishable information here).
///
/// This test already passes today — `children` was added by S-580-1, prior
/// to this story; it is not a Red Gate test for S-578-2's hinted-dispatch
/// work, only a regression pin confirming the type-level prerequisite (D4)
/// that BC-3.4.027's non-cascading-collision composer (`field_resolve.rs`)
/// depends on is in place.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_value_children_field_serde_default() {
        // `AllowedValue` does not derive `PartialEq`, so these assertions
        // compare structural properties (`is_empty`/`len`/field values)
        // rather than `assert_eq!`-ing whole `Vec<AllowedValue>` values.

        // Wire-absent `children` key.
        let absent: AllowedValue =
            serde_json::from_str(r#"{"id":"10286","value":"High","name":null}"#)
                .expect("must deserialize without a children key");
        assert!(
            absent.children.is_empty(),
            "wire-absent children must default to an empty Vec"
        );

        // Wire-present-but-empty `"children": []`.
        let empty: AllowedValue =
            serde_json::from_str(r#"{"id":"10286","value":"High","name":null,"children":[]}"#)
                .expect("must deserialize with an empty children array");
        assert!(
            empty.children.is_empty(),
            "wire-present-but-empty children must also be an empty Vec"
        );

        // Both wire forms carry the identical semantic — no information loss.
        assert_eq!(absent.children.len(), empty.children.len());

        // Wire-present, non-empty `children` (cascading case) round-trips.
        let cascading: AllowedValue = serde_json::from_str(
            r#"{"id":"1","value":"Parent","name":null,"children":[
                {"id":"2","value":"Child","name":null}
            ]}"#,
        )
        .expect("must deserialize a populated children array");
        assert_eq!(cascading.children.len(), 1);
        assert_eq!(cascading.children[0].value.as_deref(), Some("Child"));
    }
}
