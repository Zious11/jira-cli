use std::collections::{BTreeMap, HashMap};

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::error::JrError;

use super::create::{FieldValueKind, FieldValueSpec};

/// Convert a parsed f64 numeric value into the JSON wire form expected by the Jira
/// REST API: emit a whole-number integer as i64 (Atlassian's editmeta `number`
/// schema accepts both i64 and f64, but i64 is the canonical form for whole values
/// and avoids implicit precision warnings on some Jira instances). Decimal values
/// stay as f64 via `serde_json::json!`.
///
/// # Context: Stage 2 (f64 fallback) caller contract
///
/// This helper is called from the Stage 2 (f64) fallback in `resolve_edit_fields`.
/// Stage 1 (`value.parse::<i64>()`) at the call site has already short-circuited all
/// *string forms* that parse cleanly as i64 (e.g., plain integer literals like `"5"`
/// or `"-9223372036854775807"`). Stage 1.5 (`strip_integer_decimal_suffix` retry) then
/// intercepts inputs matching `^[-+]?\d+\.0+$` (trailing-zero decimals like `"5.0"` or
/// `"-9223372036854775808.0"`) — those never reach Stage 2. Stage 2 therefore receives
/// values from strings that failed BOTH Stage 1 and Stage 1.5:
///
/// - Decimals with non-zero fractional digit: `"5.5"` → emits f64; `"5.01"` → emits f64
/// - Scientific notation: `"5e3"` → emits i64 (in range); `"1.5e3"` → emits i64 (in
///   range); `"1e20"` → emits f64 (overflow); `"-9.223372036854776e18"` → emits f64
///   (at the boundary, strict `>` predicate routes to f64)
/// - Integer strings outside i64 range (no decimal point): `"9223372036854775808"` →
///   emits f64; `"-9223372036854775809"` → emits f64
///
/// Note: inputs matching `^[-+]?\d+\.0+$` (e.g., `"5.0"`, `"9223372036854775807.0"`,
/// `"-9223372036854775808.0"`) are intercepted by Stage 1.5 before reaching Stage 2.
///
/// # Strict-inequality bounds (S-421, issue #421)
///
/// The predicate uses STRICT inequalities on both bounds (`> i64::MIN as f64` and
/// `< i64::MAX as f64`) to prevent the boundary-saturation bug described in S-421:
///
/// - **Upper bound:** `i64::MAX` is 9_223_372_036_854_775_807, which is NOT exactly
///   representable as f64 (f64 has 53-bit mantissa; integers above 2^53 are rounded).
///   `i64::MAX as f64` rounds UP to 9_223_372_036_854_775_808.0 (= 2^63). The
///   non-strict `<=` predicate would admit this value; `parsed as i64` then saturates
///   silently to `i64::MAX`, producing wrong output. Strict `<` excludes 2^63.
///
/// - **Lower bound:** `i64::MIN as f64` is -9_223_372_036_854_775_808.0 (= -2^63),
///   which IS exactly representable as f64. In Stage 2, a parsed f64 value of `-2^63`
///   may arrive from several string forms:
///
///   - (a) An underflowing integer string like `"-9223372036854775809"` — Stage 1
///     rejects it (parse fails); f64 rounds it to -2^63. Value is outside i64 range;
///     emitting f64 is correct.
///   - (b) Scientific notation: `"-9.223372036854776e18"` — Stage 1 rejects it (`e`
///     present). Value IS valid `i64::MIN` (approximately); strict `>` routes to f64.
///
///   For case (a) — underflowing integer strings like `"-9223372036854775809"` — the
///   value is outside i64 range; emitting f64 is correct. The wire form is scientific
///   notation `-9.223372036854776e+18` (`serde_json` formats large-magnitude finite f64s
///   using Rust's default f64 `Display`; it does NOT flatten integer-valued f64s to bare
///   integer literals).
///
///   For case (b) — scientific notation `"-9.223372036854776e18"` — the value IS
///   approximately `i64::MIN`, but the user supplied a non-integer string form; emitting
///   f64 preserves that choice. Wire form is also scientific notation.
///
///   (Note: `"-9223372036854775808.0"` — the decimal form of `i64::MIN` — is intercepted
///   by Stage 1.5 (`strip_integer_decimal_suffix`) and reaches the i64 wire path,
///   producing the integer literal `-9223372036854775808`. It does NOT reach Stage 2.)
///
///   Using a non-strict `>= i64::MIN as f64` would let case (a) silently saturate to
///   `i64::MIN` (silent data corruption: user supplied -9223372036854775809, wire carried
///   -9223372036854775808). The strict `>` is the safer trade-off — case (a) gets the
///   correct out-of-range f64 wire form, and case (b) is mathematically equivalent either
///   way.
///
///   Caveat on `serde_json` wire formatting: `serde_json::json!(5.0_f64)` produces `5.0`
///   (decimal point, not `5`); `serde_json::json!(-9223372036854775808.0_f64)` produces
///   `-9.223372036854776e+18` (scientific notation). `Number::from(i64::MIN)` produces
///   the bare integer literal `-9223372036854775808`. These wire forms are distinct even
///   though they encode mathematically equivalent values — downstream consumers that
///   distinguish JSON integers from JSON floats (e.g., tests 26 and 27 in
///   `tests/issue_edit_field.rs` using wiremock's `NumericMode::Strict`) will observe the
///   difference. This is why Stage 1 and Stage 1.5 preserve the i64 wire path wherever
///   possible.
///
/// Caller is responsible for rejecting NaN / Inf BEFORE calling this helper —
/// `serde_json::json!(f64)` panics on non-finite values (see `Number::from_f64`).
///
/// Extracted in S-409 (issue #409); bounds tightened to strict inequalities in S-421
/// (issue #421) — Perplexity-validated against the Rust language reference and IEEE 754
/// f64 representability for integers near 2^63.
pub(crate) fn parsed_number_to_wire_value(parsed: f64) -> serde_json::Value {
    debug_assert!(
        parsed.is_finite(),
        "parsed_number_to_wire_value requires a finite value; caller must reject NaN/Inf"
    );
    if parsed.fract() == 0.0 && parsed > (i64::MIN as f64) && parsed < (i64::MAX as f64) {
        serde_json::Value::Number(serde_json::Number::from(parsed as i64))
    } else {
        serde_json::json!(parsed)
    }
}

/// Returns the integer portion of a string in the form `^[-+]?\d+\.0+$` (an
/// integer with only trailing zeros after the decimal point), e.g. `"5.0"` →
/// `Some("5")`, `"9223372036854775807.0"` → `Some("9223372036854775807")`,
/// `"5.00"` → `Some("5")`. Returns `None` for any other shape, including
/// `"5.5"`, `"5."`, `".0"`, `"5e3"`, or empty/invalid input.
///
/// Used by the Stage 1.5 retry in `resolve_edit_fields`'s `"number"` branch
/// (S-421 followup) to preserve exact i64 precision for decimal-form integer
/// inputs that would otherwise lose precision via Stage 2's f64 round-trip.
fn strip_integer_decimal_suffix(s: &str) -> Option<&str> {
    let dot_pos = s.find('.')?;
    let (int_part, after_dot) = s.split_at(dot_pos);
    let dec_part = &after_dot[1..]; // skip the '.'
    if dec_part.is_empty() || !dec_part.chars().all(|c| c == '0') {
        return None;
    }
    // Validate int_part: at most ONE optional sign char + ≥1 digit, all ASCII digits.
    // Use a first-byte check rather than `trim_start_matches(['-', '+'])`, which would
    // strip ALL leading sign chars (e.g., "--5" → "5") and allow inputs like "--5.0"
    // to pass the digit check despite not matching the documented `^[-+]?\d+\.0+$` shape.
    let digits = match int_part.as_bytes().first() {
        Some(b'+') | Some(b'-') => &int_part[1..],
        _ => int_part,
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(int_part)
}

/// Source of field metadata for [`resolve_edit_fields`]'s Phase 2/3 (S-578-4).
///
/// This function was originally scoped to `issue edit`'s editmeta-only
/// resolution (BC-3.4.015/016). S-578-4 extends it with a second,
/// createmeta-sourced variant so `issue create`'s platform path (BC-3.3.010)
/// can reuse the SAME hinted-bypass dispatch algorithm S-578-2 built,
/// substituting `createmeta` for `editmeta` because the issue does not exist
/// yet at create time — one shared dispatch function, not two independently
/// implemented ones (see `.factory/stories/S-578-4-platform-create-field-support.md`
/// §"Architecture Compliance Rules" rule 1).
///
/// `Edit` preserves the PRE-S-578-4 behavior byte-for-byte — both `edit.rs`
/// call sites wrap their issue key in this variant instead of passing a bare
/// `&str`, with no other change to their surrounding code.
///
/// `Create`'s resolution logic (BC-3.3.010 Steps 3–6) is implemented by
/// `resolve_against_createmeta`, dispatched from [`resolve_edit_fields`]'s
/// `Create` match arm below.
pub(crate) enum FieldMetaSource<'a> {
    /// `issue edit` (BC-3.4.015/016 et al) — `GET /issue/{key}/editmeta`.
    Edit { key: &'a str },
    /// `issue create` platform path (BC-3.3.010) — resolves `issue_type_name`
    /// to an id via `get_issue_types_for_project` (S-331), then calls
    /// `get_createmeta_fields` (S-580-1). Both are REUSED VERBATIM by
    /// `resolve_against_createmeta` — never re-implemented, even a
    /// "simplified" create-path-specific fetcher (Architecture Compliance
    /// Rule 1).
    Create {
        project_key: &'a str,
        issue_type_name: &'a str,
    },
}

/// The ten-member governed-key set for `issue create`'s D2 collision guard
/// (BC-3.3.010 Invariant 5, BC-3.3.011 taxonomy row, ADR-0019 §"D2
/// correction"). Passed to [`detect_flag_field_overlap`] by `create.rs`'s
/// step 2b call site.
///
/// DISTINCT from `issue edit`'s Gate B five-member set (BC-3.4.017,
/// `edit.rs`, inline — NOT extracted into a shared constant by this story;
/// `edit.rs` is out of S-578-4's scope). `detect_flag_field_overlap` is a
/// shared MECHANISM, never a claim that the two governed-key sets are
/// identical (Architecture Compliance Rule 2) — `labels` in particular is
/// governed here but deliberately absent from Gate B's set (BUG-LABEL-400's
/// edit-path endpoint fork has no analog on create; Architecture Compliance
/// Rule 3).
///
/// `points`/`team` are the two "resolved-id" members (AC-011) — asserted
/// SEPARATELY per the story's documented algorithm (bypass-form-only
/// equality for `--points`; config-only field-id lookup for `--team`, never
/// an HTTP call to service this guard) via `detect_flag_field_overlap`'s
/// `resolved_id_flags` parameter, not this static-key set.
pub(crate) const CREATE_D2_GOVERNED_KEYS: &[&str] = &[
    "summary",
    "description",
    "issuetype",
    "priority",
    "components",
    "labels",
    "parent",
    "assignee",
    "points",
    "team",
];

/// Detects a dedicated-flag × `--field` wire-key collision (D2/D2-correction,
/// BC-3.3.010 Invariant 5, BC-3.3.011 taxonomy row, VP-578-021).
///
/// Called BEFORE resolution, BEFORE project/type resolution, with ZERO HTTP
/// (structural check only, over already-parsed CLI input). `dedicated_flags`
/// is a `(governed_key, is_present)` list describing which of the caller's
/// dedicated flags (`--summary`, `--priority`, etc.) were supplied on this
/// invocation; `governed_keys` is the caller's own governed-key set
/// ([`CREATE_D2_GOVERNED_KEYS`] for `create.rs`'s step 2b call site — NEVER
/// shared with `edit.rs`'s own five-member Gate B set, per Architecture
/// Compliance Rule 2).
///
/// Returns `Ok(())` when no dedicated flag whose governed key is ALSO a key
/// in `field_pairs` was supplied; `Err(JrError::UserError)` (exit 64) on the
/// first collision found, naming both the flag and the colliding `--field`
/// key (BC-3.3.011 taxonomy row 1, evaluated FIRST — before every other
/// error row, AC-012).
///
/// # Errors
/// Returns `JrError::UserError` on a collision — see above.
///
/// # Parameters
/// - `field_pairs`: the parsed `--field` map (`parse_field_kv` output).
/// - `dedicated_flags`: `(governed_key, is_present)` pairs for STATIC
///   wire-key collisions — the governed key is compared case-insensitively
///   against `field_pairs`' keys directly (e.g. `"summary"` vs a `--field
///   summary=...` pair).
/// - `governed_keys`: the caller's governed-key set (only entries also
///   present here participate in the `dedicated_flags` check).
/// - `resolved_id_flags`: `(label, Some(resolved_field_id))` pairs for
///   RESOLVED-ID collisions (`--points`/`--team`) — bypass-form-only
///   equality against `field_pairs`' keys. `None` means either the
///   dedicated flag was absent on this invocation or its backing field id
///   is not configured; either way it never trips the guard (the
///   documented non-firing residual for a human display-name spelling on
///   the `--field` side is a natural consequence of this bypass-only
///   equality, since a display name never equals a `customfield_NNNNN`
///   literal).
///
/// Non-firing residual generalizes to every static governed key too (AC-011
/// documents it only for `--points`/`--team`, but the same mechanism applies
/// across the board): `dedicated_flags` compares governed WIRE keys
/// (`"summary"`, `"priority"`, etc.) against `field_pairs`' keys directly, with
/// no name resolution — so `--field summ=Y` alongside `--summary X` does NOT
/// collide with the static check, and both writes reach step 4b, where the
/// wire key resolved from `"summ"` (a substring match against the field list)
/// last-write-wins against `--summary`'s write. This is BY DESIGN: catching a
/// display-name or substring spelling here would require running field-name
/// resolution before this guard, which would violate the zero-HTTP boundary
/// this function is defined to run within.
pub(crate) fn detect_flag_field_overlap(
    field_pairs: &HashMap<String, FieldValueSpec>,
    dedicated_flags: &[(&str, bool)],
    governed_keys: &[&str],
    resolved_id_flags: &[(&str, Option<&str>)],
) -> Result<()> {
    let field_keys_lower: std::collections::HashSet<String> =
        field_pairs.keys().map(|k| k.to_lowercase()).collect();

    // Static key collisions (the 8 governed keys asserted via presence
    // flags: summary/description/issuetype/priority/components/labels/
    // parent/assignee).
    for (key, present) in dedicated_flags {
        if !*present {
            continue;
        }
        if !governed_keys.iter().any(|g| g.eq_ignore_ascii_case(key)) {
            continue;
        }
        if field_keys_lower.contains(&key.to_lowercase()) {
            return Err(collision_error(key));
        }
    }

    // Resolved-id key collisions (points/team) — bypass-form-only equality,
    // never a display-name lookup (would require HTTP, violating the
    // zero-HTTP boundary this guard runs within).
    for (label, resolved_id) in resolved_id_flags {
        if let Some(id) = resolved_id {
            if field_keys_lower.contains(&id.to_lowercase()) {
                return Err(collision_error(label));
            }
        }
    }

    Ok(())
}

/// Builds the D2/Gate-B-shaped collision error naming both the colliding
/// `--field` key and its dedicated-flag counterpart.
fn collision_error(key: &str) -> anyhow::Error {
    let flag_hint = match key {
        "summary" => "--summary",
        "description" => "--description/--description-stdin",
        "issuetype" => "--type",
        "priority" => "--priority",
        "components" => "--component",
        "labels" => "--label",
        "parent" => "--parent",
        "assignee" => "--to/--account-id",
        "points" => "--points",
        "team" => "--team",
        _ => "a dedicated flag",
    };
    JrError::UserError(format!(
        "{key} is set by both {flag_hint} and --field; use only one."
    ))
    .into()
}

/// Resolve and apply `--field NAME=VALUE` pairs for `issue edit` (single-key path)
/// and, as of S-578-4, `issue create`'s platform path (BC-3.3.010).
///
/// Implements BC-3.4.015 Steps 1–6 and BC-3.4.016 option-value resolution for
/// the `Edit` source. Called from `handle_edit` in BOTH the `if dry_run { ... }`
/// block and the live path — see BC-3.4.015 invariant 10 and prd-delta-396.md §9.
///
/// # Parameters
/// - `client`: the authenticated Jira API client.
/// - `profile`: active profile name (CLAUDE.md cache-boundary rule — every
///   cache reader/writer takes `profile: &str`; cross-profile field-ID leakage
///   is a correctness bug because sandbox/prod custom-field IDs can differ).
/// - `source`: [`FieldMetaSource::Edit`] (issue key, editmeta) or
///   [`FieldMetaSource::Create`] (project key + issue type name, createmeta —
///   S-578-4). The `Create` arm resolves via `resolve_against_createmeta`:
///   `get_issue_types_for_project` (name → issue type id) followed by
///   `get_createmeta_fields`, both feeding the shared `dispatch_field_value`
///   dispatch (the same per-pair type-dispatch algorithm the `Edit` arm uses).
/// - `field_pairs`: `NAME → FieldValueSpec` map produced by `parse_field_kv`
///   (last-wins semantics; duplicates collapsed at parse time per
///   EC-3.4.017-10). `FieldValueSpec.kind` drives the S-578-2 hinted-bypass
///   dispatch (Phase 3 below); `kind: None` is the bare form and is resolved
///   exactly as before this story (BC-3.4.015/016, unchanged).
/// - `fields`: mutable reference to the shared `fields` JSON object that will
///   be PUT to Jira. Resolution results are merged in here (Step 5).
/// - `changed_fields`: mutable reference to the human-readable echo map
///   (`BTreeMap<String, String>`). Resolved pairs are inserted here INSIDE
///   `resolve_edit_fields` BEFORE the PUT is issued (Step 6). The caller's
///   discard-on-failure behaviour is realised by `edit_result?` short-circuiting
///   before the `changed_fields` echo/JSON emission in `handle_edit`: if the
///   PUT returns a non-2xx error, `?` propagates the error and the already-
///   populated `changed_fields` is never echoed. For option fields the value
///   is the human label, not the option id.
/// - `planned_preview`: mutable reference to the dry-run `plannedChanges`
///   preview map (S-578-2, BC-3.4.021 amended Postconditions, AC-012), keyed
///   identically to `changed_fields`. For a HINTED pair (`spec.kind.is_some()`)
///   the value is the composed wire shape itself (documented exception to the
///   general rule); for a bare pair it is the same simplified display-value
///   string `changed_fields` carries, JSON-wrapped. The live (non-dry-run)
///   call site passes a throwaway map — this is a pure additional output, it
///   never influences resolution or the PUT.
///
/// # Errors
/// Returns `Err` (which the caller propagates as exit 64) on any of:
/// - Field name not found in `list_fields()` or the per-profile cache (Step 2b).
/// - Field absent from `editmeta.fields` (Step 3).
/// - `"set"` absent from `operations` (Step 3b).
/// - Unsupported schema type `"array"` / `"any"` / unknown (Step 4).
/// - Option value not found in or ambiguous among `allowedValues` (Step 4a).
/// - Number parse failure — e.g. `NaN`, `Inf` (Step 4).
///
/// # Algorithm (prd-delta-396.md §6)
/// ```text
/// Step 1  customfield_\d+ literal? → bypass Steps 2/2b; use NAME as field ID.
/// Step 2  read_fields_cache(profile) hit → use cached list (no HTTP).
///         miss/stale → list_fields() → write_fields_cache (best-effort).
/// Step 2b case-insensitive exact match first, then substring. 0 → exit 64.
///         Multiple → exit 64 (ambiguous).
/// Step 3  get_editmeta(key). Field absent → exit 64 + Edit-screen hint.
/// Step 3b "set" ∉ operations → exit 64 + operations hint.
/// Step 4  schema.type dispatch (string/number/date/datetime/user/option/→exit64).
/// Step 4a option: id bypass (numeric literal) → exact → substring on value.
///         Empty allowedValues → exit 64. Ambiguous → exit 64.
/// Step 5  merge (field_id, wire_value) into `fields`.
/// Step 6  insert (human_name, display_value) into `changed_fields`.
/// ```
pub(crate) async fn resolve_edit_fields(
    client: &JiraClient,
    profile: &crate::profile::Profile,
    source: FieldMetaSource<'_>,
    field_pairs: &HashMap<String, FieldValueSpec>,
    fields: &mut serde_json::Value,
    changed_fields: &mut BTreeMap<String, String>,
    planned_preview: &mut BTreeMap<String, serde_json::Value>,
) -> Result<()> {
    use crate::cache::{read_fields_cache, write_fields_cache};

    if field_pairs.is_empty() {
        return Ok(());
    }

    // --- Phase 1: Resolve field IDs for all pairs (Steps 1–2b). ---
    // Fetch the field list once (cached or API) for all non-literal pairs.
    // This happens BEFORE get_editmeta so that name-resolution failures exit 64
    // without making the editmeta HTTP call.
    let mut field_list: Option<Vec<(String, String)>> = None;

    // Track whether we've already fetched a fresh list from the API this
    // invocation. Once true, any further miss is definitively "not found" — we
    // MUST NOT call list_fields() again (would violate the exactly-once HTTP
    // contract asserted by BC-3.4.015 / test 24).
    let mut api_fetched = false;

    // Resolved items: (field_id, human_name, spec). `spec` carries both the
    // uninterpreted VALUE and the S-578-2 `FieldValueSpec.kind` hint through
    // to Phase 3, where the hinted-bypass dispatch reads it.
    let mut resolved: Vec<(String, String, FieldValueSpec)> = Vec::with_capacity(field_pairs.len());

    for (name, spec) in field_pairs {
        // Step 1: customfield_NNNNN literal bypass.
        // BC-3.4.015 Step 1: requires `customfield_` followed by ONE OR MORE digits.
        // `.all(...)` on an empty iterator returns true, so we must also check that
        // the suffix is non-empty (name.len() > 12) to prevent `customfield_=VALUE`
        // from triggering the bypass and landing on the wrong "not on Edit screen" error.
        let is_literal_bypass = name.starts_with("customfield_")
            && name.len() > "customfield_".len()
            && name[12..].chars().all(|c| c.is_ascii_digit());

        if is_literal_bypass {
            // Literal: use NAME as-is; no list_fields() call.
            resolved.push((name.clone(), name.clone(), spec.clone()));
        } else {
            // EC-3.4.015-9: empty NAME guard.  `--field =VALUE` (no name before `=`)
            // is parsed by `parse_field_kv` into ("", VALUE).  Without this check,
            // `name_lower = ""` and `String::contains("")` returns true for EVERY
            // field name, causing a silent single-field match on 1-field instances or a
            // confusing "ambiguous" error listing every field on multi-field instances.
            // Both violate EC-3.4.015-9 which requires a zero-match error with an
            // actionable hint.
            if name.is_empty() {
                return Err(JrError::UserError(
                    "Field '' not found. The field name before '=' must not be empty. \
                     Check the field name with `jr project fields --output json` to list \
                     available fields. Zero matches for ''."
                        .into(),
                )
                .into());
            }

            // Step 2: load or fetch the field list (once per invocation, shared).
            // Algorithm: try on-disk cache; if field is found there, use it.
            // If field is NOT found in the on-disk cache (cache may be stale/
            // incomplete), fall back to a fresh API fetch and re-search.
            // The in-memory `field_list` is populated on first use and reused
            // for subsequent pairs.
            let name_lower = name.to_lowercase();

            // Load the field list (from memory cache → on-disk cache → API).
            // R2-C1: propagate genuine I/O errors from read_fields_cache with `?`
            // instead of silently discarding them via .ok().flatten().
            // read_cache already classifies: ENOENT → Ok(None); serde-corrupt →
            // warn + Ok(None) self-heal; genuine I/O → Err. The previous
            // .ok().flatten() negated the careful tri-state design by swallowing
            // the Err arm. Consistent with every other cache-reader call site in src/.
            if field_list.is_none() {
                if let Some(fc) = read_fields_cache(profile)? {
                    field_list = Some(fc.fields);
                }
                // If still None, we'll fetch from API when needed below.
            }

            // Try to find the field in whatever list we have so far.
            fn search_field(
                list: &[(String, String)],
                name_lower: &str,
                name: &str,
            ) -> Result<Option<(String, String)>> {
                let exact: Vec<&(String, String)> = list
                    .iter()
                    .filter(|(_, n)| n.to_lowercase() == name_lower)
                    .collect();
                if exact.len() == 1 {
                    return Ok(Some((exact[0].0.clone(), exact[0].1.clone())));
                }
                if exact.len() > 1 {
                    let candidates: Vec<String> =
                        exact.iter().map(|(id, n)| format!("{n} ({id})")).collect();
                    return Err(JrError::UserError(format!(
                        "Field name '{name}' matches multiple fields: {}. Use the field ID \
                         directly (e.g. customfield_NNNNN) to disambiguate.",
                        candidates.join(", ")
                    ))
                    .into());
                }
                // Substring match.
                let sub: Vec<&(String, String)> = list
                    .iter()
                    .filter(|(_, n)| n.to_lowercase().contains(name_lower))
                    .collect();
                if sub.len() == 1 {
                    return Ok(Some((sub[0].0.clone(), sub[0].1.clone())));
                }
                if sub.len() > 1 {
                    let candidates: Vec<String> =
                        sub.iter().map(|(id, n)| format!("{n} ({id})")).collect();
                    return Err(JrError::UserError(format!(
                        "Field name '{name}' is ambiguous — matches: {}. Use a more \
                         specific name or the field ID directly (e.g. customfield_NNNNN).",
                        candidates.join(", ")
                    ))
                    .into());
                }
                Ok(None) // not found, no error yet
            }

            // First pass: search in current (cached or memory) list.
            let found_in_cache = if let Some(ref fl) = field_list {
                search_field(fl, &name_lower, name)?
            } else {
                None
            };

            let (field_id, human_name) = if let Some(pair) = found_in_cache {
                pair
            } else if api_fetched {
                // We already have a fresh list from the API this invocation.
                // The field is definitively absent — do not call list_fields() again.
                return Err(JrError::UserError(format!(
                    "Field '{name}' not found. Check the field name with \
                     `jr issue edit --field customfield_NNNNN=VALUE` or use \
                     `--output json` on `jr project fields` to list available \
                     fields. Zero matches for '{name}'."
                ))
                .into());
            } else {
                // Field not found in cache (or no cache). Fetch fresh from API once.
                let raw_fields = client.list_fields().await?;
                let fresh: Vec<(String, String)> = raw_fields
                    .iter()
                    .map(|f| (f.id.clone(), f.name.clone()))
                    .collect();
                // Unconditional best-effort write: mirrors the cmdb_fields pattern.
                // write_fields_cache swallows I/O errors (returns Ok(())); the caller
                // is not penalized for a failed cache write (tests 18/19 pin this).
                write_fields_cache(profile, &fresh)?;
                field_list = Some(fresh);
                api_fetched = true;
                let fl = field_list.as_ref().unwrap();
                // Second pass: search fresh list.
                match search_field(fl, &name_lower, name)? {
                    Some(pair) => pair,
                    None => {
                        return Err(JrError::UserError(format!(
                            "Field '{name}' not found. Check the field name with \
                             `jr issue edit --field customfield_NNNNN=VALUE` or use \
                             `--output json` on `jr project fields` to list available \
                             fields. Zero matches for '{name}'."
                        ))
                        .into());
                    }
                }
            };

            resolved.push((field_id, human_name, spec.clone()));
        }
    }

    // --- Phase 2/3: fetch field metadata + per-pair validation/type dispatch
    // (Steps 3–6), branching on `source` (S-578-4). ---
    match source {
        FieldMetaSource::Edit { key } => {
            resolve_against_editmeta(
                client,
                key,
                resolved,
                fields,
                changed_fields,
                planned_preview,
            )
            .await
        }
        FieldMetaSource::Create {
            project_key,
            issue_type_name,
        } => {
            resolve_against_createmeta(
                client,
                project_key,
                issue_type_name,
                resolved,
                fields,
                changed_fields,
                planned_preview,
            )
            .await
        }
    }
}

/// The `Create`-source Phase 2/3 body (BC-3.3.010 Steps 3–6, S-578-4).
///
/// Resolves `issue_type_name` to an id via `get_issue_types_for_project`
/// (S-331, REUSED VERBATIM, case-insensitive; unknown name → exit 64 listing
/// valid types — AC-007), then calls `get_createmeta_fields` (S-580-1,
/// REUSED VERBATIM) for that project + issue type id, then runs the SAME
/// per-pair type dispatch (hinted-bypass + bare-form, AC-008) as the `Edit`
/// arm via the shared [`dispatch_field_value`] helper — no second,
/// independently-implemented dispatch (Architecture Compliance Rule 1).
///
/// **No `operations`/`"set"` check** — createmeta has no `operations` array
/// (BC-3.3.010 "No operations/set check on create"); any field returned in
/// createmeta's field list for the resolved issue type is assumed settable.
async fn resolve_against_createmeta(
    client: &JiraClient,
    project_key: &str,
    issue_type_name: &str,
    resolved: Vec<(String, String, FieldValueSpec)>,
    fields: &mut serde_json::Value,
    changed_fields: &mut BTreeMap<String, String>,
    planned_preview: &mut BTreeMap<String, serde_json::Value>,
) -> Result<()> {
    // Step 3 (issue-type name → id, S-331 reuse, AC-007): case-insensitive,
    // offset-paginated internally inside get_issue_types_for_project.
    let issue_types = client.get_issue_types_for_project(project_key).await?;
    let issue_type_id = issue_types
        .iter()
        .find(|it| it.name.eq_ignore_ascii_case(issue_type_name))
        .map(|it| it.id.clone())
        .ok_or_else(|| {
            let mut valid: Vec<&str> = issue_types.iter().map(|it| it.name.as_str()).collect();
            valid.sort_unstable();
            JrError::UserError(format!(
                "Issue type '{issue_type_name}' not found in project '{project_key}'. \
                 Valid issue types: {}.",
                valid.join(", ")
            ))
        })?;

    // Step 3 (field enumeration, S-580-1 reuse, AC-006/VP-578-001):
    // GET /rest/api/3/issue/createmeta/{project}/issuetypes/{issueTypeId},
    // offset-paginated internally — NEVER GET /issue/{key}/editmeta.
    let createmeta_fields = client
        .get_createmeta_fields(project_key, &issue_type_id)
        .await?;
    let meta_by_id: HashMap<String, crate::api::jira::issues::CreateMetaField> = createmeta_fields
        .into_iter()
        .map(|f| (f.field_id.clone(), f))
        .collect();

    for (field_id, human_name, spec) in resolved {
        // Step 3: validate field is on the resolved issue type's Create
        // screen (createmeta field list) — "Create screen" substituted for
        // "Edit screen" throughout (BC-3.3.011). Uses the non-consuming
        // `.get()` (mirroring the Edit arm's `editmeta.fields.get(&field_id)`
        // below, AC-008) rather than `.remove()` — two distinct `--field`
        // pairs can resolve to the SAME field_id (e.g. a `customfield_NNNNN`
        // bypass pair alongside a display-name pair for the same field), and
        // `.remove()` made the second such pair falsely report "not on the
        // Create screen" even though the field IS present (adversary Pass 2
        // LOW finding).
        let meta_field = meta_by_id.get(&field_id).ok_or_else(|| {
            JrError::UserError(format!(
                "Field '{human_name}' ({field_id}) is not on the Create screen for project \
                 '{project_key}' issue type '{issue_type_name}'. A project admin must add it \
                 to the Create screen before it can be set via `jr issue create --field`. \
                 Check the screen configuration in Jira project settings."
            ))
        })?;

        // Adapt CreateMetaField -> EditMetaField shape so the shared
        // dispatch (identical to the Edit arm, AC-008) can be reused
        // verbatim. `operations` is synthesized as `["set"]` — createmeta
        // has no operations concept, and every createmeta field is
        // assumed settable (see this function's own doc comment).
        let adapted = crate::types::jira::EditMetaField {
            name: meta_field.name.clone(),
            schema: meta_field.schema.clone(),
            allowed_values: meta_field.allowed_values.clone(),
            operations: vec!["set".to_string()],
            required: false,
            auto_complete_url: meta_field.auto_complete_url.clone(),
        };

        dispatch_field_value(
            client,
            &field_id,
            human_name,
            spec,
            &adapted,
            &mut FieldResolutionOutputs {
                fields,
                changed_fields,
                planned_preview,
            },
        )
        .await?;
    }

    Ok(())
}

/// The `Edit`-source Phase 2/3 body (BC-3.4.015 Steps 3–6, BC-3.4.016 option
/// resolution) — extracted verbatim (S-578-4, pure code motion, no behavior
/// change) from [`resolve_edit_fields`] so the function can dispatch on
/// [`FieldMetaSource`] without duplicating this already-tested body inline
/// in a match arm.
async fn resolve_against_editmeta(
    client: &JiraClient,
    key: &str,
    resolved: Vec<(String, String, FieldValueSpec)>,
    fields: &mut serde_json::Value,
    changed_fields: &mut BTreeMap<String, String>,
    planned_preview: &mut BTreeMap<String, serde_json::Value>,
) -> Result<()> {
    // --- Phase 2: Fetch editmeta once (Step 3). ---
    // Only reached when all field names were resolved successfully (Phase 1 has no errors).
    let editmeta = client.get_editmeta(key).await?;

    // --- Phase 3: Per-pair editmeta validation + type dispatch (Steps 3b–6). ---
    for (field_id, human_name, spec) in resolved {
        // Step 3: validate field is in editmeta (present on the Edit screen).
        let meta_field = editmeta.fields.get(&field_id).ok_or_else(|| {
            JrError::UserError(format!(
                "Field '{human_name}' ({field_id}) is not on the Edit screen for issue {key}. \
                 A project admin must add it to the Edit screen before it can be edited via \
                 `jr issue edit --field`. Check the screen configuration in Jira project settings."
            ))
        })?;

        // Step 3b: operations must include "set".
        if !meta_field.operations.iter().any(|op| op == "set") {
            return Err(JrError::UserError(format!(
                "Field '{human_name}' ({field_id}) does not support the 'set' operation. \
                 Available operations: [{}]. Only fields with 'set' in their operations list \
                 can be edited via `--field`.",
                meta_field.operations.join(", ")
            ))
            .into());
        }

        dispatch_field_value(
            client,
            &field_id,
            human_name,
            spec,
            meta_field,
            &mut FieldResolutionOutputs {
                fields,
                changed_fields,
                planned_preview,
            },
        )
        .await?;
    }

    Ok(())
}

/// Output/accumulator bundle for [`dispatch_field_value`].
///
/// Reduces argument count on `dispatch_field_value` to satisfy
/// `clippy::too_many_arguments` (CLAUDE.md policy: refactor rather than
/// `#[allow]`) by bundling the three `&mut` output sinks each call site
/// already threads through together. Pure signature refactor (S-578-4) —
/// no behavior change at either call site.
struct FieldResolutionOutputs<'a> {
    fields: &'a mut serde_json::Value,
    changed_fields: &'a mut BTreeMap<String, String>,
    planned_preview: &'a mut BTreeMap<String, serde_json::Value>,
}

/// Shared per-pair Step 4-6 dispatch (hinted-bypass + bare-form type
/// dispatch, BC-3.4.015 Step 4 / BC-3.4.016 Step 4a) — extracted (S-578-4,
/// pure code motion, no behavior change for the `Edit` call site) so
/// [`resolve_against_editmeta`] and [`resolve_against_createmeta`] share ONE
/// dispatch implementation instead of two independently-maintained copies
/// (Architecture Compliance Rule 1, AC-008).
///
/// Callers are responsible for their own Step 3/3b screen-membership and
/// (editmeta-only) operations checks BEFORE calling this — this function
/// only ever sees a `meta_field` already confirmed present/settable.
async fn dispatch_field_value(
    client: &JiraClient,
    field_id: &str,
    human_name: String,
    spec: FieldValueSpec,
    meta_field: &crate::types::jira::EditMetaField,
    outputs: &mut FieldResolutionOutputs<'_>,
) -> Result<()> {
    let value = spec.value.clone();

    // S-578-2 (AC-001): hinted-bypass dispatch runs BEFORE the existing
    // `schema.type` match below when `spec.kind` is present — the bare-form
    // dispatch (Step 4 and its `field_type` match, BC-3.4.015/016) stays
    // UNCHANGED and PERMANENT for `kind: None`, per Architecture Compliance
    // Rule 1.
    if let Some(kind) = spec.kind {
        let (wire_value, display_value): (serde_json::Value, String) = match kind {
            FieldValueKind::Option => compose_option_hint(&value, &human_name, meta_field)?,
            FieldValueKind::Id => compose_id_hint(&value),
            FieldValueKind::Name => compose_name_hint(&value),
            FieldValueKind::Asset => compose_asset_hint(client, &value).await?,
        };
        // AC-012: for a hinted field the dry-run preview IS the composed
        // wire shape itself (documented exception to the general rule).
        outputs
            .planned_preview
            .insert(human_name.clone(), wire_value.clone());
        outputs.fields[field_id] = wire_value;
        outputs.changed_fields.insert(human_name, display_value);
        return Ok(());
    }

    // Step 4: type dispatch.
    let field_type = meta_field.schema.field_type.as_str();
    let wire_value: serde_json::Value;
    let display_value: String;

    match field_type {
        "string" | "text" => {
            wire_value = serde_json::Value::String(value.clone());
            display_value = value.clone();
        }
        "number" => {
            // Stage 1: exact i64 parse first (no f64 precision loss).
            // S-421: this short-circuits the f64 round-trip for all i64-representable
            // inputs, eliminating both the boundary-saturation bug and the precision
            // loss for integers above 2^53 (e.g., "9007199254740993" was off-by-one
            // pre-fix when parsed through f64 first).
            if let Ok(n) = value.parse::<i64>() {
                wire_value = serde_json::Value::Number(serde_json::Number::from(n));
            } else if let Some(stripped) = strip_integer_decimal_suffix(&value) {
                // Stage 1.5 (S-421 followup, post-Copilot review):
                // Integer with trailing-zero decimal like "5.0" or "9223372036854775807.0".
                // Strip the ".0+" suffix and retry i64 parse. This preserves exact i64
                // semantics for decimal-form integer inputs that would otherwise lose
                // precision via the f64 round-trip in Stage 2.
                //
                // Background: all four boundary strings — "9223372036854775807",
                // "9223372036854775808", "9223372036854775807.0", "9223372036854775808.0"
                // — parse to the same f64 value (2^63 = 9223372036854775808.0) because
                // i64::MAX is not exactly representable in f64. Without Stage 1.5, the
                // strict `<` predicate in Stage 2 would reject this f64 and emit it as
                // f64 wire form — correct for the overflow case but a regression for
                // "9223372036854775807.0" (the decimal form of i64::MAX, which IS valid).
                if let Ok(n) = stripped.parse::<i64>() {
                    wire_value = serde_json::Value::Number(serde_json::Number::from(n));
                } else {
                    // Stripped integer still doesn't fit in i64 (e.g., "9223372036854775808.0"
                    // strips to "9223372036854775808" which overflows). Fall through to Stage 2.
                    let parsed: f64 = value.parse().map_err(|_| {
                        JrError::UserError(format!(
                            "Cannot parse '{value}' as a number for field '{human_name}'. \
                             Provide a valid numeric value (integer, decimal, or scientific \
                             notation like 1e10)."
                        ))
                    })?;
                    if !parsed.is_finite() {
                        return Err(JrError::UserError(format!(
                            "Value '{value}' for field '{human_name}' is not a finite number \
                             (NaN or Inf are not accepted). Provide a valid numeric value."
                        ))
                        .into());
                    }
                    wire_value = parsed_number_to_wire_value(parsed);
                }
            } else {
                // Stage 2: f64 fallback for decimals, scientific notation, and
                // integers outside the i64 range.
                let parsed: f64 = value.parse().map_err(|_| {
                    JrError::UserError(format!(
                        "Cannot parse '{value}' as a number for field '{human_name}'. \
                         Provide a valid numeric value (integer, decimal, or scientific \
                         notation like 1e10)."
                    ))
                })?;
                if !parsed.is_finite() {
                    return Err(JrError::UserError(format!(
                        "Value '{value}' for field '{human_name}' is not a finite number \
                         (NaN or Inf are not accepted). Provide a valid numeric value."
                    ))
                    .into());
                }
                // Emit integer wire form for whole numbers in range, f64 otherwise.
                // Helper extracted in S-409; bounds tightened to strict inequalities in S-421.
                wire_value = parsed_number_to_wire_value(parsed);
            }
            display_value = value.clone();
        }
        "date" | "datetime" => {
            // Pass-through: no client-side validation; server validates.
            wire_value = serde_json::Value::String(value.clone());
            display_value = value.clone();
        }
        "user" => {
            // Wire: {"accountId": VALUE}; display: raw accountId.
            wire_value = serde_json::json!({"accountId": value});
            display_value = value.clone();
        }
        "option" => {
            // Step 4a: option resolution (BC-3.4.016). Extracted into the
            // shared `resolve_option_value` helper (S-578-2) so the
            // `:option` hinted-bypass composer's non-cascading path
            // (`compose_option_hint`) can reuse the IDENTICAL algorithm —
            // AC-002 requires byte-for-byte identical wire output between
            // the bare form and `:option` for the same NAME/VALUE. This is
            // a pure code-motion refactor: the bare-form dispatch's
            // observable behavior is unchanged (Architecture Compliance
            // Rule 1), verified by the full pre-existing regression suite.
            let allowed = meta_field.allowed_values.as_deref().unwrap_or(&[]);
            if allowed.is_empty() {
                return Err(JrError::UserError(format!(
                    "Field '{human_name}' has no configured option values. \
                     An admin must populate the option list before values can be set."
                ))
                .into());
            }
            let (wv, dv) = resolve_option_value(&value, &human_name, allowed)?;
            wire_value = wv;
            display_value = dv;
        }
        other => {
            return Err(unsupported_field_type_error(other, &human_name).into());
        }
    }

    // Step 5: merge (field_id, wire_value) into the shared fields JSON object.
    // AC-012: the bare-form dry-run preview stays the SIMPLIFIED display
    // string (general rule, unchanged) — only a hinted field's preview
    // (above) is the real wire shape.
    outputs
        .planned_preview
        .insert(human_name.clone(), serde_json::json!(display_value.clone()));
    outputs.fields[field_id] = wire_value;

    // Step 6: insert (human_name, display_value) into changed_fields.
    outputs.changed_fields.insert(human_name, display_value);

    Ok(())
}

/// Result of matching a value against an editmeta option field's
/// `allowedValues` list (BC-3.4.016 Step 4a algorithm). Shared by the
/// bare-form `option` dispatch (Step 4a above), the `:option` hinted
/// composer's non-cascading path, and cascading parent/child resolution
/// (S-578-2, AC-002/AC-003).
struct OptionMatch<'a> {
    matched: &'a crate::types::jira::AllowedValue,
    /// `Some(raw_value)` when the numeric id-bypass path matched — the echo
    /// must be the raw value verbatim, no reverse label lookup (EC-3.4.016-4).
    /// `None` when a label match (exact or substring) fired — the echo is the
    /// matched entry's stored-casing label.
    id_bypass_echo: Option<String>,
}

/// Matches `value` against `allowed` using the exact BC-3.4.016 Step 4a
/// precedence: numeric id-bypass → case-insensitive exact label match →
/// case-insensitive substring label match. Extracted (S-578-2) so the
/// bare-form `option` dispatch and the `:option` hinted composer (both
/// non-cascading values and cascading parent/child segments) share one
/// algorithm — required for AC-002's byte-identical wire-output guarantee.
fn find_option_match<'a>(
    value: &str,
    human_name: &str,
    allowed: &'a [crate::types::jira::AllowedValue],
) -> Result<OptionMatch<'a>> {
    // Option id bypass: if VALUE is a purely numeric string AND matches an
    // allowedValues[].id exactly. EC-3.4.016-4: id-bypass fires only for
    // numeric strings — a label that happens to equal an option id would
    // otherwise silently route through id-bypass, echoing the raw VALUE
    // instead of the stored-casing label.
    let id_match = if !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) {
        allowed
            .iter()
            .find(|av| av.id.as_deref().map(|id| id == value).unwrap_or(false))
    } else {
        None
    };
    if let Some(av) = id_match {
        return Ok(OptionMatch {
            matched: av,
            id_bypass_echo: Some(value.to_string()),
        });
    }

    // Case-insensitive exact match on the value field.
    let value_lower = value.to_lowercase();
    let exact_av: Vec<&crate::types::jira::AllowedValue> = allowed
        .iter()
        .filter(|av| {
            av.value
                .as_deref()
                .map(|v| v.to_lowercase() == value_lower)
                .unwrap_or(false)
        })
        .collect();
    if exact_av.len() == 1 {
        return Ok(OptionMatch {
            matched: exact_av[0],
            id_bypass_echo: None,
        });
    }
    if exact_av.len() > 1 {
        let candidates: Vec<String> = exact_av
            .iter()
            .map(|av| {
                format!(
                    "{} (id: {})",
                    av.value.as_deref().unwrap_or("?"),
                    av.id.as_deref().unwrap_or("<no-id>")
                )
            })
            .collect();
        return Err(JrError::UserError(format!(
            "Option value '{value}' is ambiguous for field '{human_name}': {}. \
             Disambiguate via the option id (numeric).",
            candidates.join(", ")
        ))
        .into());
    }

    // Substring match.
    let sub_av: Vec<&crate::types::jira::AllowedValue> = allowed
        .iter()
        .filter(|av| {
            av.value
                .as_deref()
                .map(|v| v.to_lowercase().contains(&value_lower))
                .unwrap_or(false)
        })
        .collect();
    if sub_av.is_empty() {
        let allowed_labels: Vec<String> = allowed
            .iter()
            .map(|av| {
                av.value
                    .clone()
                    .unwrap_or_else(|| av.id.clone().unwrap_or_else(|| "<no-id>".to_string()))
            })
            .collect();
        return Err(JrError::UserError(format!(
            "Option value '{value}' not found for field '{human_name}'. Allowed values: {}.",
            allowed_labels.join(", ")
        ))
        .into());
    }
    if sub_av.len() > 1 {
        let candidates: Vec<String> = sub_av
            .iter()
            .map(|av| {
                format!(
                    "{} (id: {})",
                    av.value.as_deref().unwrap_or("?"),
                    av.id.as_deref().unwrap_or("<no-id>")
                )
            })
            .collect();
        return Err(JrError::UserError(format!(
            "Option value '{value}' is ambiguous for field '{human_name}': {}. \
             Use the option id directly.",
            candidates.join(", ")
        ))
        .into());
    }
    Ok(OptionMatch {
        matched: sub_av[0],
        id_bypass_echo: None,
    })
}

/// Resolves a single (non-cascading) option value via [`find_option_match`]
/// and composes the `{"id": "<optionId>"}` wire shape — the bare-form
/// `option` dispatch's Step 4a algorithm (BC-3.4.016), shared verbatim
/// (S-578-2) with the `:option` hinted composer's non-cascading path.
fn resolve_option_value(
    value: &str,
    human_name: &str,
    allowed: &[crate::types::jira::AllowedValue],
) -> Result<(serde_json::Value, String)> {
    let m = find_option_match(value, human_name, allowed)?;
    let Some(ref option_id) = m.matched.id else {
        return Err(JrError::UserError(format!(
            "option '{value}' has no machine-readable id and cannot be set \
             via --field. This typically occurs with user/group picker fields. \
             Use the Jira UI or the field's native picker to set this value."
        ))
        .into());
    };
    let wire_value = serde_json::json!({"id": option_id});
    let display_value = m
        .id_bypass_echo
        .unwrap_or_else(|| m.matched.value.clone().unwrap_or_else(|| value.to_string()));
    Ok((wire_value, display_value))
}

/// Builds BC-3.4.015's canonical "unsupported type" error (Step 4's bare-form
/// `other` type-dispatch arm). Extracted as a shared helper (S-578-2,
/// EC-3.4.027-1 / AC-019 sub-case (a)) so the `:option` hinted composer's
/// entry-point type gate can reuse this EXACT message for `array`/`any`
/// fields rather than re-deriving a similar-but-different string — the
/// literal `field_type` value must appear verbatim in both call sites.
fn unsupported_field_type_error(field_type: &str, human_name: &str) -> JrError {
    JrError::UserError(format!(
        "Field '{human_name}' has type '{field_type}' which is not supported by \
         `--field` in this version. Supported types: string, number, option, \
         date, datetime, user. Array and CMDB fields are not supported — \
         use the Jira UI for {field_type}-type fields."
    ))
}

/// Composes the `:option` hinted-bypass wire shape (S-578-2 Task 4).
///
/// Non-cascading (`VALUE` contains no `>`): byte-identical to the bare-form
/// `option` dispatch above (BC-3.4.027 Description, VP-578-007) — both share
/// [`resolve_option_value`]. Cascading (`VALUE` contains `>`):
/// `str::split_once('>')` (Architecture Compliance Rule 2 — never a
/// char-index/fixed-byte-offset scheme); the parent segment resolves against
/// `allowedValues[].value`, the child segment against the matched parent's
/// `children[].value` (`AllowedValue.children`). Non-cascading-field `>`
/// collision (D4, EC-3.4.027-7) is detected structurally via an empty
/// `children` list on the matched parent, not a `schema.type` lookup.
///
/// Returns `(wire_value, display_value)` — wire is `{"id": "<optionId>"}` for
/// the non-cascading case or `{"value":"<parent>","child":{"value":"<child>"}}`
/// for the cascading case; display is the matched label, or `"<parent> >
/// <child>"` for cascading (BC-3.4.027 Postconditions "changed_fields echo").
fn compose_option_hint(
    value: &str,
    human_name: &str,
    meta_field: &crate::types::jira::EditMetaField,
) -> Result<(serde_json::Value, String)> {
    // EC-3.4.027-1 (AC-019): entry-point `schema.type` gate. Runs BEFORE any
    // `allowedValues`/`children` inspection below — this is what makes a
    // non-option field with EMPTY/absent `allowedValues` (e.g. a "number"
    // field) get THIS gate's "is not an option field" message rather than
    // falling through to BC-3.4.016's "no configured option values" message,
    // which presupposes the field already passed this gate. Orthogonal to
    // AC-004's D4 structural `children.is_empty()` check further down (which
    // stays structural, per Invariant 6, and only ever runs for a field that
    // has already cleared this gate).
    let field_type = meta_field.schema.field_type.as_str();
    match field_type {
        "option" | "option-with-child" => {}
        "array" | "any" => {
            // Sub-case (a): reuse BC-3.4.015's EXACT "unsupported type"
            // message (EC-3.4.015-5) rather than inventing a new one — the
            // literal `field_type` string ("array"/"any") must match.
            return Err(unsupported_field_type_error(field_type, human_name).into());
        }
        other => {
            // Sub-case (b): a distinct message — this is NOT BC-3.4.015's
            // "unsupported `--field` type" case (the bare form CAN set a
            // string/number/date/datetime/user field); it's specifically
            // that `:option` doesn't apply to this field's type.
            return Err(JrError::UserError(format!(
                "Field '{human_name}' has type '{other}' which is not an option \
                 field — `:option` requires a field of type 'option' or \
                 'option-with-child'. Use the bare form `--field NAME=VALUE` \
                 (no `:option` hint) to set a '{other}'-type field instead."
            ))
            .into());
        }
    }

    let allowed = meta_field.allowed_values.as_deref().unwrap_or(&[]);
    if allowed.is_empty() {
        return Err(JrError::UserError(format!(
            "Field '{human_name}' has no configured option values. \
             An admin must populate the option list before values can be set."
        ))
        .into());
    }

    // D3 MUST: str::split_once('>'), never a char-index/fixed-byte-offset
    // scheme — the whole delimiter-locate-and-slice operation is one call,
    // eliminating the FIX-F6-LRE-1 panic class by construction.
    match value.split_once('>') {
        None => resolve_option_value(value, human_name, allowed),
        Some((parent_raw, child_raw)) => {
            // EC-3.4.027-6 (empty parent, `>Child`): same shape as
            // EC-3.4.027-2 (unresolvable parent) — an empty parent segment
            // can never legitimately match a real option label.
            if parent_raw.is_empty() {
                let allowed_labels: Vec<String> = allowed
                    .iter()
                    .map(|av| {
                        av.value.clone().unwrap_or_else(|| {
                            av.id.clone().unwrap_or_else(|| "<no-id>".to_string())
                        })
                    })
                    .collect();
                return Err(JrError::UserError(format!(
                    "Option value '' not found for field '{human_name}'. Allowed values: {}.",
                    allowed_labels.join(", ")
                ))
                .into());
            }

            let parent_match = find_option_match(parent_raw, human_name, allowed)?;
            let parent_av = parent_match.matched;

            // EC-3.4.027-6 (empty child, `Parent>`): BC-3.4.027's "empty
            // child segment falls through to the SAME unresolvable-child
            // exit-64 shape as EC-3.4.027-3 ... consistent with EC-3.4.027-3's
            // existing precedent rather than introducing a distinct
            // empty-segment error message" — the message text below is
            // therefore byte-shape-identical to `find_option_match`'s own
            // "not found" error (below: `"Option value '{value}' not found
            // for field '{human_name}'. Allowed values: {…}."`), NOT a
            // bespoke variant naming the parent or relabeling "Allowed
            // values" to "Allowed child values" (PR #741 review, S-578-2
            // fix-burst — an earlier revision of this branch did both and
            // was a real spec deviation, corrected here).
            //
            // This can't simply be `find_option_match(child_raw, human_name,
            // &parent_av.children)` — `find_option_match`'s substring-match
            // stage treats an empty needle as contained in every candidate
            // (`"anything".contains("")` is `true` in Rust), so an empty
            // child would hit its "ambiguous" branch (when the parent has
            // ≥2 children) instead of "not found". This early return
            // reproduces `find_option_match`'s not-found TEXT SHAPE by hand
            // while sidestepping that substring-match trap — an empty
            // segment can never legitimately match a real child label,
            // regardless of whether the field is genuinely cascading. This
            // check MUST precede the D4 structural check below: D4 fires only
            // for a NON-EMPTY child segment (its own precondition).
            if child_raw.is_empty() {
                let child_labels: Vec<String> = parent_av
                    .children
                    .iter()
                    .map(|c| {
                        c.value.clone().unwrap_or_else(|| {
                            c.id.clone().unwrap_or_else(|| "<no-id>".to_string())
                        })
                    })
                    .collect();
                return Err(JrError::UserError(format!(
                    "Option value '' not found for field '{human_name}'. Allowed values: {}.",
                    child_labels.join(", ")
                ))
                .into());
            }

            // D4 (adversary F-2): non-cascading-field `>` collision —
            // detected structurally via an empty `children` list on the
            // matched parent, never a `schema.type` lookup.
            if parent_av.children.is_empty() {
                return Err(JrError::UserError(format!(
                    "field '{human_name}' is not a cascading select — remove the \
                     '>{child_raw}' segment from the value."
                ))
                .into());
            }

            let child_match = find_option_match(child_raw, human_name, &parent_av.children)?;
            let parent_label = parent_av
                .value
                .clone()
                .unwrap_or_else(|| parent_raw.to_string());
            let child_label = child_match
                .matched
                .value
                .clone()
                .unwrap_or_else(|| child_raw.to_string());
            let wire_value = serde_json::json!({
                "value": parent_label,
                "child": {"value": child_label}
            });
            let display_value = format!("{parent_label} > {child_label}");
            Ok((wire_value, display_value))
        }
    }
}

/// Composes the `:id` hinted-bypass wire shape (S-578-2 Task 5) — `VALUE`
/// sent verbatim as `{"id": "<VALUE>"}`, with NO `allowedValues` lookup, NO
/// label matching, and NO ambiguity detection (BC-3.4.028
/// Description/Postconditions). `changed_fields` echo is `VALUE` itself (no
/// reverse lookup).
fn compose_id_hint(value: &str) -> (serde_json::Value, String) {
    (serde_json::json!({"id": value}), value.to_string())
}

/// Composes the `:name` hinted-bypass wire shape (S-578-2 Task 5) — `VALUE`
/// sent verbatim as `{"name": "<VALUE>"}` (BC-3.4.029
/// Description/Postconditions). This is the identical shape the dedicated
/// `--priority` flag already sends (`edit.rs`, `fields["priority"] =
/// json!({"name": p})`), so `--field priority:name=Medium` produces
/// byte-identical wire output to `--priority Medium` (AC-007) without a
/// separate reusable function existing to call — the shape itself is the
/// contract.
fn compose_name_hint(value: &str) -> (serde_json::Value, String) {
    (serde_json::json!({"name": value}), value.to_string())
}

/// Composes the `:asset` hinted-bypass wire shape (S-578-2 Task 6) —
/// `[{"workspaceId":"<resolved>","id":"<workspaceId>:<objectId>","objectId":"<objectId>"}]`
/// (BC-3.4.030 Parsing rules/Postconditions). `VALUE` splits on the FIRST
/// `:` via `str::split_once(':')` (Architecture Compliance Rule 2, mirrors
/// the `:option` cascading composer's `>` MUST); when no `:` is present,
/// `objectId` is the whole `VALUE` and `workspaceId` resolves via the
/// existing cached `get_or_fetch_workspace_id`
/// (`crate::api::assets::workspace::get_or_fetch_workspace_id`) — called AT
/// THIS L2 call site (Architecture Compliance Rule 3: never inside a sibling
/// L4/JSM module). On a cold cache this is a genuine HTTP round-trip and can
/// fail per BC-3.4.030's error taxonomy (AC-010) — 403/404 → "Assets is not
/// available…"; 200 + zero entries → "No Assets workspace found…"; 401/5xx/
/// network → standard mappings (via `?` propagation from
/// `get_or_fetch_workspace_id` itself).
///
/// # Malformed-shape errors (BC-3.4.031 EC-2/EC-3, AC-009)
/// Checked in this order (EC-2c's empty-workspace-segment check MUST run
/// BEFORE the objectId-segment checks — `:asset=:` triggers EC-2c, never
/// EC-2b):
/// 1. Empty `VALUE` → "asset reference cannot be empty" (EC-2a).
/// 2. `:` present, workspace segment empty → "workspace segment cannot be
///    empty…" (EC-2c).
/// 3. `:` present, remainder contains a SECOND `:` → "unexpected extra
///    ':'…" (EC-2d) — checked before the generic numeric check so a
///    multi-colon mistake gets its own message, not the generic one.
/// 4. objectId segment (ASCII `[0-9]+` only, NOT Unicode `\d`) empty or
///    non-numeric → "objectId must be numeric" (EC-2b/EC-3).
async fn compose_asset_hint(
    client: &JiraClient,
    value: &str,
) -> Result<(serde_json::Value, String)> {
    if value.is_empty() {
        return Err(JrError::UserError(
            "asset reference cannot be empty. Use --field NAME:asset=OBJECTID (workspace \
             id resolved from cache) or --field NAME:asset=WORKSPACE:OBJECTID."
                .into(),
        )
        .into());
    }

    let (workspace_id, object_id): (String, String) = match value.split_once(':') {
        Some((ws, rest)) => {
            if ws.is_empty() {
                return Err(JrError::UserError(
                    "workspace segment cannot be empty when ':' is present; omit the \
                     workspace prefix entirely to use the cached workspace id."
                        .into(),
                )
                .into());
            }
            if rest.contains(':') {
                return Err(JrError::UserError(format!(
                    "unexpected extra ':' in :asset value '{value}' — expected \
                     WORKSPACE:OBJECTID."
                ))
                .into());
            }
            if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
                return Err(JrError::UserError(format!(
                    "objectId must be numeric (ASCII digits only); got '{rest}'."
                ))
                .into());
            }
            (ws.to_string(), rest.to_string())
        }
        None => {
            if !value.chars().all(|c| c.is_ascii_digit()) {
                return Err(JrError::UserError(format!(
                    "objectId must be numeric (ASCII digits only); got '{value}'."
                ))
                .into());
            }
            let workspace_id =
                crate::api::assets::workspace::get_or_fetch_workspace_id(client).await?;
            (workspace_id, value.to_string())
        }
    };

    let wire_value = serde_json::json!([{
        "workspaceId": workspace_id,
        "id": format!("{workspace_id}:{object_id}"),
        "objectId": object_id
    }]);
    let display_value = format!("{workspace_id}:{object_id}");
    Ok((wire_value, display_value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_number_to_wire_value_whole_emits_i64() {
        let wire = parsed_number_to_wire_value(5.0);
        assert!(wire.is_i64() && !wire.is_f64(), "expected i64, got {wire}");
        assert_eq!(wire.as_i64(), Some(5));
    }

    #[test]
    fn parsed_number_to_wire_value_scientific_whole_emits_i64() {
        let wire = parsed_number_to_wire_value(5e3);
        assert!(wire.is_i64() && !wire.is_f64(), "expected i64, got {wire}");
        assert_eq!(wire.as_i64(), Some(5000));
    }

    #[test]
    fn parsed_number_to_wire_value_fractional_emits_f64() {
        let wire = parsed_number_to_wire_value(5.5);
        assert!(wire.is_f64(), "expected f64, got {wire}");
        assert_eq!(wire.as_f64(), Some(5.5));
    }

    #[test]
    fn parsed_number_to_wire_value_zero_emits_i64() {
        let wire = parsed_number_to_wire_value(0.0);
        assert!(wire.is_i64(), "expected i64 for 0.0, got {wire}");
        assert_eq!(wire.as_i64(), Some(0));
    }

    #[test]
    fn parsed_number_to_wire_value_negative_whole_emits_i64() {
        let wire = parsed_number_to_wire_value(-42.0);
        assert!(wire.is_i64(), "expected i64 for -42.0, got {wire}");
        assert_eq!(wire.as_i64(), Some(-42));
    }

    #[test]
    fn parsed_number_to_wire_value_out_of_i64_range_emits_f64() {
        // i64::MAX is 9_223_372_036_854_775_807; this f64 exceeds that.
        let wire = parsed_number_to_wire_value(1e20);
        assert!(
            wire.is_f64(),
            "expected f64 for 1e20 (overflow), got {wire}"
        );
    }

    // S-421: boundary regression pins for the strict-inequality predicate.

    #[test]
    fn test_parsed_number_to_wire_value_strict_upper_excludes_two_to_the_63() {
        // 2^63 = 9223372036854775808.0 = i64::MAX as f64 (rounds up because f64 can't
        // represent i64::MAX exactly). Strict-less-than predicate excludes it.
        let two_to_63 = 9223372036854775808.0_f64;
        let wire = parsed_number_to_wire_value(two_to_63);
        assert!(
            wire.is_f64(),
            "expected f64 for 2^63 (out of i64 range), got {wire}"
        );
    }

    #[test]
    fn test_parsed_number_to_wire_value_strict_lower_excludes_negative_two_to_the_63_in_stage2() {
        // -2^63 = i64::MIN as f64 (exact). In Stage 2 context, a parsed f64 value of
        // -2^63 may arrive from two string forms: (a) an underflowing integer like
        // "-9223372036854775809" (Stage 1 parse fails; f64 rounds to -2^63), or
        // (b) scientific notation like "-9.223372036854776e18" (Stage 1 rejects `e`).
        // The decimal form "-9223372036854775808.0" is intercepted by Stage 1.5
        // (strip_integer_decimal_suffix) and NEVER reaches Stage 2. The strict
        // > i64::MIN comparison routes both Stage 2 cases to f64. This test invokes
        // the helper directly with the f64 value -2^63 to pin the predicate behavior
        // independent of source string.
        let neg_two_to_63 = -9223372036854775808.0_f64;
        let wire = parsed_number_to_wire_value(neg_two_to_63);
        assert!(
            wire.is_f64(),
            "expected f64 for -2^63 in Stage-2 context, got {wire}"
        );
    }

    // S-421: two-stage (now three-stage) end-to-end boundary tests.
    // Tests call `parse_number_wire` which mirrors the Stage 1 → Stage 1.5 → Stage 2
    // dispatch from the production `"number"` branch of resolve_edit_fields.

    /// Test-only replica of the **happy-path** routing in `resolve_edit_fields`'s
    /// `"number"` branch — Stage 1 (i64 parse) → Stage 1.5 (strip-decimal + i64 retry) →
    /// Stage 2 (f64 parse + helper call). Used by the S-421 boundary regression tests to
    /// exercise the same decision tree without HTTP mocking.
    ///
    /// **Limitations vs production:**
    /// - Uses `unwrap()` on the f64 parse paths instead of returning a user error.
    ///   Inputs that fail `parse::<f64>()` (e.g., `"abc"`, `""`) will panic here.
    /// - Does NOT replicate the production `is_finite()` rejection guard. Inputs that
    ///   parse to `+Inf`/`-Inf` (`"1e309"`, `"-1e309"`) or `NaN` (`"NaN"`) will reach
    ///   `parsed_number_to_wire_value` directly, which then panics via its own
    ///   `debug_assert!(parsed.is_finite())`.
    ///
    /// Tests using this helper must supply only valid finite numeric strings. If a
    /// future test needs to exercise the NaN/Inf rejection path, call
    /// `resolve_edit_fields` end-to-end with HTTP mocking or build a separate helper.
    ///
    /// Must be kept in sync with the production code — if `resolve_edit_fields` adds
    /// a new stage or changes the dispatch order, update this helper accordingly.
    fn parse_number_wire(value: &str) -> serde_json::Value {
        if let Ok(n) = value.parse::<i64>() {
            serde_json::Value::Number(serde_json::Number::from(n))
        } else if let Some(stripped) = super::strip_integer_decimal_suffix(value) {
            if let Ok(n) = stripped.parse::<i64>() {
                serde_json::Value::Number(serde_json::Number::from(n))
            } else {
                let parsed: f64 = value.parse().unwrap();
                super::parsed_number_to_wire_value(parsed)
            }
        } else {
            let parsed: f64 = value.parse().unwrap();
            super::parsed_number_to_wire_value(parsed)
        }
    }

    #[test]
    fn test_s421_i64_max_emits_i64() {
        let value = "9223372036854775807";
        let wire = parse_number_wire(value);
        assert_eq!(
            wire.as_i64(),
            Some(i64::MAX),
            "expected i64::MAX, got {wire}"
        );
        assert!(wire.is_i64() && !wire.is_f64());
    }

    #[test]
    fn test_s421_i64_max_plus_one_emits_f64() {
        let value = "9223372036854775808"; // i64::MAX + 1 = 2^63
        let wire = parse_number_wire(value);
        assert!(
            wire.is_f64(),
            "expected f64 (not silently saturated i64), got {wire}"
        );
    }

    #[test]
    fn test_s421_i64_min_emits_i64() {
        let value = "-9223372036854775808";
        let wire = parse_number_wire(value);
        assert_eq!(wire.as_i64(), Some(i64::MIN));
    }

    #[test]
    fn test_s421_i64_min_minus_one_emits_f64() {
        let value = "-9223372036854775809"; // i64::MIN - 1
        let wire = parse_number_wire(value);
        assert!(
            wire.is_f64(),
            "expected f64 (not silently saturated i64::MIN), got {wire}"
        );
    }

    #[test]
    fn test_s421_two_to_53_plus_one_emits_exact_i64_no_precision_loss() {
        // 2^53 + 1 = 9007199254740993 — NOT exactly representable as f64 (rounds to 2^53).
        // Pre-S-421: parsed as f64 → 9007199254740992 (off by 1) → emitted as i64.
        // Post-S-421: Stage 1 parses as i64 exactly → emitted as i64 with correct value.
        let value = "9007199254740993";
        let wire = parse_number_wire(value);
        assert_eq!(wire.as_i64(), Some(9007199254740993));
    }

    #[test]
    fn test_s421_scientific_notation_one_e_ten_emits_i64() {
        // "1e10" parses as i64 → FAILS (parser doesn't accept scientific notation).
        // Falls to Stage 2: f64 parse → 10000000000.0 → fract == 0 → strict predicate
        // (10000000000.0 < 2^63) → emit as i64 10_000_000_000.
        let value = "1e10";
        let wire = parse_number_wire(value);
        assert_eq!(wire.as_i64(), Some(10_000_000_000));
    }

    // S-421 Stage 1.5 regression pins.

    #[test]
    fn test_s421_decimal_form_of_i64_max_uses_stage_1_5_and_emits_i64() {
        // Regression pin: "9223372036854775807.0" parses to f64 2^63 (rounded UP),
        // which strict Stage 2 would reject. Stage 1.5 strips the .0 suffix and
        // retries as i64, recovering exact i64::MAX.
        let value = "9223372036854775807.0";
        let wire = parse_number_wire(value);
        assert_eq!(
            wire.as_i64(),
            Some(i64::MAX),
            "decimal form of i64::MAX must emit i64, got {wire}"
        );
        assert!(wire.is_i64() && !wire.is_f64());
    }

    #[test]
    fn test_s421_decimal_form_of_i64_min_uses_stage_1_5_and_emits_i64() {
        // Mirror of the upper-bound regression.
        let value = "-9223372036854775808.0";
        let wire = parse_number_wire(value);
        assert_eq!(
            wire.as_i64(),
            Some(i64::MIN),
            "decimal form of i64::MIN must emit i64, got {wire}"
        );
        assert!(wire.is_i64() && !wire.is_f64());
    }

    #[test]
    fn test_s421_stage_1_5_decimal_form_overflow_falls_through_to_f64() {
        // "9223372036854775808.0" strips to "9223372036854775808" which still
        // overflows i64. Falls through to Stage 2 (f64). The wire form encodes
        // the f64 representation (2^63), distinct from emitting silently-saturated i64.
        let value = "9223372036854775808.0";
        let wire = parse_number_wire(value);
        assert!(
            wire.is_f64(),
            "out-of-i64-range decimal form must emit f64, got {wire}"
        );
    }

    // Unit tests for strip_integer_decimal_suffix.

    #[test]
    fn test_strip_integer_decimal_suffix_recognizes_trailing_zeros() {
        assert_eq!(super::strip_integer_decimal_suffix("5.0"), Some("5"));
        assert_eq!(super::strip_integer_decimal_suffix("5.00"), Some("5"));
        assert_eq!(super::strip_integer_decimal_suffix("-5.0"), Some("-5"));
        assert_eq!(super::strip_integer_decimal_suffix("+5.0"), Some("+5"));
        assert_eq!(
            super::strip_integer_decimal_suffix("9223372036854775807.0"),
            Some("9223372036854775807")
        );
    }

    #[test]
    fn test_strip_integer_decimal_suffix_rejects_non_integer_decimals() {
        assert_eq!(super::strip_integer_decimal_suffix("5.5"), None);
        assert_eq!(super::strip_integer_decimal_suffix("5.01"), None);
        assert_eq!(super::strip_integer_decimal_suffix("5.10"), None); // trailing zero but non-zero digit after dot
        assert_eq!(super::strip_integer_decimal_suffix("5.0e1"), None);
    }

    #[test]
    fn test_strip_integer_decimal_suffix_rejects_malformed_input() {
        assert_eq!(super::strip_integer_decimal_suffix(""), None);
        assert_eq!(super::strip_integer_decimal_suffix("5"), None); // no dot
        assert_eq!(super::strip_integer_decimal_suffix("5."), None); // empty decimal part
        assert_eq!(super::strip_integer_decimal_suffix(".0"), None); // empty integer part
        assert_eq!(super::strip_integer_decimal_suffix("-.0"), None); // sign only
        assert_eq!(super::strip_integer_decimal_suffix("5e3"), None); // scientific notation
        assert_eq!(super::strip_integer_decimal_suffix("5e3.0"), None); // mixed
        assert_eq!(super::strip_integer_decimal_suffix("abc.0"), None); // non-digit
        assert_eq!(super::strip_integer_decimal_suffix("1.0.0"), None); // multiple dots
        // S-421 R5: multi-sign inputs must return None (matches the ^[-+]?\d+\.0+$ contract).
        assert_eq!(
            super::strip_integer_decimal_suffix("--5.0"),
            None,
            "two leading minuses must reject"
        );
        assert_eq!(
            super::strip_integer_decimal_suffix("++5.0"),
            None,
            "two leading pluses must reject"
        );
        assert_eq!(
            super::strip_integer_decimal_suffix("+-5.0"),
            None,
            "plus-then-minus must reject"
        );
        assert_eq!(
            super::strip_integer_decimal_suffix("-+5.0"),
            None,
            "minus-then-plus must reject"
        );
    }
}
