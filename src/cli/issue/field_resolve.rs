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
