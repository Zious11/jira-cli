use std::collections::{BTreeMap, HashMap};

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::error::JrError;

/// Convert a parsed f64 numeric value into the JSON wire form expected by the Jira
/// REST API: emit a whole-number integer as i64 (Atlassian's editmeta `number`
/// schema accepts both i64 and f64, but i64 is the canonical form for whole values
/// and avoids implicit precision warnings on some Jira instances). Decimal values
/// stay as f64 via `serde_json::json!`.
///
/// Caller is responsible for rejecting NaN / Inf BEFORE calling this helper —
/// `serde_json::json!(f64)` panics on non-finite values (see `Number::from_f64`).
///
/// Extracted in S-409 (issue #409) so the integer-vs-float wire-form invariant
/// can be unit-tested without reimplementing the conversion in the test body.
pub(crate) fn parsed_number_to_wire_value(parsed: f64) -> serde_json::Value {
    debug_assert!(
        parsed.is_finite(),
        "parsed_number_to_wire_value requires a finite value; caller must reject NaN/Inf"
    );
    if parsed.fract() == 0.0 && parsed >= i64::MIN as f64 && parsed <= i64::MAX as f64 {
        serde_json::Value::Number(serde_json::Number::from(parsed as i64))
    } else {
        serde_json::json!(parsed)
    }
}

/// Resolve and apply `--field NAME=VALUE` pairs for `issue edit` (single-key path).
///
/// Implements BC-3.4.015 Steps 1–6 and BC-3.4.016 option-value resolution.
/// Called from `handle_edit` in BOTH the `if dry_run { ... }` block and the
/// live path — see BC-3.4.015 invariant 10 and prd-delta-396.md §9.
///
/// # Parameters
/// - `client`: the authenticated Jira API client.
/// - `profile`: active profile name (CLAUDE.md cache-boundary rule — every
///   cache reader/writer takes `profile: &str`; cross-profile field-ID leakage
///   is a correctness bug because sandbox/prod custom-field IDs can differ).
/// - `key`: the issue key being edited (used for `get_editmeta` call).
/// - `field_pairs`: `NAME → VALUE` map produced by `parse_field_kv` (last-wins
///   semantics; duplicates collapsed at parse time per EC-3.4.017-10).
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
    profile: &str,
    key: &str,
    field_pairs: &HashMap<String, String>,
    fields: &mut serde_json::Value,
    changed_fields: &mut BTreeMap<String, String>,
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

    // Resolved items: (field_id, human_name, value)
    let mut resolved: Vec<(String, String, String)> = Vec::with_capacity(field_pairs.len());

    for (name, value) in field_pairs {
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
            resolved.push((name.clone(), name.clone(), value.clone()));
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

            resolved.push((field_id, human_name, value.clone()));
        }
    }

    // --- Phase 2: Fetch editmeta once (Step 3). ---
    // Only reached when all field names were resolved successfully (Phase 1 has no errors).
    let editmeta = client.get_editmeta(key).await?;

    // --- Phase 3: Per-pair editmeta validation + type dispatch (Steps 3b–6). ---
    for (field_id, human_name, value) in resolved {
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
                // Parse as f64; reject NaN and Inf; emit integer form when possible.
                let parsed: f64 = value.parse().map_err(|_| {
                    JrError::UserError(format!(
                        "Cannot parse '{value}' as a number for field '{human_name}'. \
                         Provide a valid numeric value (integer or decimal)."
                    ))
                })?;
                if !parsed.is_finite() {
                    return Err(JrError::UserError(format!(
                        "Value '{value}' for field '{human_name}' is not a finite number \
                         (NaN or Inf are not accepted). Provide a valid numeric value."
                    ))
                    .into());
                }
                // Emit integer wire form for whole numbers, f64 otherwise.
                // Helper extracted in S-409 so the invariant can be unit-tested.
                wire_value = parsed_number_to_wire_value(parsed);
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
                // Step 4a: option resolution.
                // Precedence: exact id match → case-insensitive exact value match → substring.
                let allowed = meta_field.allowed_values.as_deref().unwrap_or(&[]);
                if allowed.is_empty() {
                    return Err(JrError::UserError(format!(
                        "Field '{human_name}' has no configured option values. \
                         An admin must populate the option list before values can be set."
                    ))
                    .into());
                }

                // Option id bypass: if VALUE is a purely numeric string AND matches
                // an allowedValues[].id exactly.  EC-3.4.016-4: id-bypass fires only
                // for numeric strings.  Without the pre-filter, a label that happens to
                // equal an option id would silently route through id-bypass, echoing the
                // raw VALUE instead of the stored-casing label.  Mirroring the H-1
                // customfield_NNNNN guard: non-empty + all-digits.
                let id_match = if !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) {
                    allowed.iter().find(|av| av.id == value)
                } else {
                    None
                };
                if let Some(av) = id_match {
                    wire_value = serde_json::json!({"id": av.id});
                    // Echo raw value (no reverse label lookup) when id-bypass fires.
                    display_value = value.clone();
                } else {
                    // Case-insensitive exact match on value field.
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
                        let av = exact_av[0];
                        wire_value = serde_json::json!({"id": av.id});
                        // Echo human label (stored casing), not id.
                        display_value = av.value.clone().unwrap_or_else(|| value.clone());
                    } else if exact_av.len() > 1 {
                        let candidates: Vec<String> = exact_av
                            .iter()
                            .map(|av| {
                                format!("{} (id: {})", av.value.as_deref().unwrap_or("?"), av.id)
                            })
                            .collect();
                        return Err(JrError::UserError(format!(
                            "Option value '{value}' is ambiguous for field '{human_name}': {}. \
                             Disambiguate via the option id (numeric).",
                            candidates.join(", ")
                        ))
                        .into());
                    } else {
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
                                .map(|av| av.value.clone().unwrap_or_else(|| av.id.clone()))
                                .collect();
                            return Err(JrError::UserError(format!(
                                "Option value '{value}' not found for field '{human_name}'. \
                                 Allowed values: {}.",
                                allowed_labels.join(", ")
                            ))
                            .into());
                        } else if sub_av.len() > 1 {
                            let candidates: Vec<String> = sub_av
                                .iter()
                                .map(|av| {
                                    format!(
                                        "{} (id: {})",
                                        av.value.as_deref().unwrap_or("?"),
                                        av.id
                                    )
                                })
                                .collect();
                            return Err(JrError::UserError(format!(
                                "Option value '{value}' is ambiguous for field \
                                 '{human_name}': {}. Use the option id directly.",
                                candidates.join(", ")
                            ))
                            .into());
                        } else {
                            let av = sub_av[0];
                            wire_value = serde_json::json!({"id": av.id});
                            display_value = av.value.clone().unwrap_or_else(|| value.clone());
                        }
                    }
                }
            }
            other => {
                return Err(JrError::UserError(format!(
                    "Field '{human_name}' has type '{other}' which is not supported by \
                     `--field` in this version. Supported types: string, number, option, \
                     date, datetime, user. Array and CMDB fields are not supported — \
                     use the Jira UI for {other}-type fields."
                ))
                .into());
            }
        }

        // Step 5: merge (field_id, wire_value) into the shared fields JSON object.
        fields[&field_id] = wire_value;

        // Step 6: insert (human_name, display_value) into changed_fields.
        changed_fields.insert(human_name, display_value);
    }

    Ok(())
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
}
