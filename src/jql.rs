/// Escape a value for interpolation into a JQL double-quoted string literal.
///
/// Backslashes are escaped first, then double quotes. Order matters: escaping
/// quotes first would introduce backslashes that the second pass re-escapes,
/// leaving the quote exposed (escape neutralization attack).
pub fn escape_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Validate a JQL relative date duration string.
///
/// JQL relative dates use the format `<digits><unit>` where unit is one of:
/// `y` (years), `M` (months), `w` (weeks), `d` (days), `h` (hours), `m` (minutes).
/// Units are case-sensitive — `M` is months, `m` is minutes.
/// Combined units like `4w2d` are not supported by Jira.
pub fn validate_duration(s: &str) -> Result<(), String> {
    if s.len() < 2 {
        return Err(format!(
            "Invalid duration '{s}'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M)."
        ));
    }
    let (digits, unit) = s.split_at(s.len() - 1);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "Invalid duration '{s}'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M)."
        ));
    }
    if !matches!(unit, "y" | "M" | "w" | "d" | "h" | "m") {
        return Err(format!(
            "Invalid duration '{s}'. Use a number followed by y, M, w, d, h, or m (e.g., 7d, 4w, 2M)."
        ));
    }
    Ok(())
}

/// Validate an asset object key matches the SCHEMA-NUMBER format.
///
/// Asset keys are always `<uppercase-alpha>-<digits>` (e.g., CUST-5, SRV-42, ITSM-123).
pub fn validate_asset_key(key: &str) -> Result<(), String> {
    let Some((prefix, number)) = key.split_once('-') else {
        return Err(format!(
            "Invalid asset key \"{key}\". Expected format: SCHEMA-NUMBER (e.g., CUST-5, SRV-42)."
        ));
    };
    if prefix.is_empty()
        || !prefix.chars().all(|c| c.is_ascii_alphanumeric())
        || number.is_empty()
        || !number.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "Invalid asset key \"{key}\". Expected format: SCHEMA-NUMBER (e.g., CUST-5, SRV-42)."
        ));
    }
    Ok(())
}

/// Build a JQL clause that filters issues by a linked asset object key.
///
/// Uses `aqlFunction()` with the human-readable field name (required by Jira Cloud).
/// When multiple CMDB fields exist, OR them together and wrap in parentheses.
pub fn build_asset_clause(asset_key: &str, cmdb_fields: &[(String, String)]) -> String {
    let clauses: Vec<String> = cmdb_fields
        .iter()
        .map(|(_, name)| {
            format!(
                "\"{}\" IN aqlFunction(\"Key = \\\"{}\\\"\")",
                escape_value(name),
                escape_value(asset_key),
            )
        })
        .collect();

    if clauses.len() == 1 {
        clauses.into_iter().next().unwrap()
    } else {
        format!("({})", clauses.join(" OR "))
    }
}

/// Strip `ORDER BY` clause from JQL for use with count-only endpoints.
///
/// The approximate-count endpoint only needs the WHERE clause. ORDER BY is
/// meaningless for a count and may cause issues with bounded-JQL validation.
pub fn strip_order_by(jql: &str) -> &str {
    let upper = jql.to_ascii_uppercase();
    if let Some(pos) = upper.find(" ORDER BY") {
        jql[..pos].trim_end()
    } else if upper.starts_with("ORDER BY") {
        ""
    } else {
        jql
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_special_characters() {
        assert_eq!(escape_value("In Progress"), "In Progress");
    }

    #[test]
    fn double_quotes_escaped() {
        assert_eq!(escape_value(r#"He said "hello""#), r#"He said \"hello\""#);
    }

    #[test]
    fn backslashes_escaped() {
        assert_eq!(escape_value(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn escape_neutralization_prevented() {
        assert_eq!(escape_value(r#"foo\"bar"#), r#"foo\\\"bar"#);
    }

    #[test]
    fn trailing_backslash() {
        assert_eq!(escape_value(r"foo\"), r"foo\\");
    }

    #[test]
    fn strip_order_by_removes_clause() {
        assert_eq!(
            strip_order_by("project = PROJ ORDER BY updated DESC"),
            "project = PROJ"
        );
    }

    #[test]
    fn strip_order_by_no_clause() {
        assert_eq!(strip_order_by("project = PROJ"), "project = PROJ");
    }

    #[test]
    fn strip_order_by_case_insensitive() {
        assert_eq!(
            strip_order_by("project = PROJ order by rank ASC"),
            "project = PROJ"
        );
    }

    #[test]
    fn strip_order_by_trims_whitespace() {
        assert_eq!(
            strip_order_by("project = PROJ   ORDER BY rank ASC"),
            "project = PROJ"
        );
    }

    #[test]
    fn strip_order_by_at_position_zero() {
        assert_eq!(strip_order_by("ORDER BY created DESC"), "");
    }

    #[test]
    fn strip_order_by_at_position_zero_lowercase() {
        assert_eq!(strip_order_by("order by rank ASC"), "");
    }

    #[test]
    fn validate_duration_valid_days() {
        assert!(validate_duration("7d").is_ok());
    }

    #[test]
    fn validate_duration_valid_weeks() {
        assert!(validate_duration("4w").is_ok());
    }

    #[test]
    fn validate_duration_valid_months_uppercase() {
        assert!(validate_duration("2M").is_ok());
    }

    #[test]
    fn validate_duration_valid_years() {
        assert!(validate_duration("1y").is_ok());
    }

    #[test]
    fn validate_duration_valid_hours() {
        assert!(validate_duration("5h").is_ok());
    }

    #[test]
    fn validate_duration_valid_minutes() {
        assert!(validate_duration("10m").is_ok());
    }

    #[test]
    fn validate_duration_valid_zero() {
        assert!(validate_duration("0d").is_ok());
    }

    #[test]
    fn validate_duration_invalid_unit() {
        assert!(validate_duration("7x").is_err());
    }

    #[test]
    fn validate_duration_reversed() {
        assert!(validate_duration("d7").is_err());
    }

    #[test]
    fn validate_duration_empty() {
        assert!(validate_duration("").is_err());
    }

    #[test]
    fn validate_duration_combined_units() {
        assert!(validate_duration("4w2d").is_err());
    }

    #[test]
    fn validate_duration_no_digits() {
        assert!(validate_duration("d").is_err());
    }

    #[test]
    fn validate_asset_key_valid_simple() {
        assert!(validate_asset_key("CUST-5").is_ok());
    }

    #[test]
    fn validate_asset_key_valid_long() {
        assert!(validate_asset_key("SRV-42").is_ok());
    }

    #[test]
    fn validate_asset_key_valid_itsm() {
        assert!(validate_asset_key("ITSM-123").is_ok());
    }

    #[test]
    fn validate_asset_key_invalid_no_number() {
        assert!(validate_asset_key("CUST-").is_err());
    }

    #[test]
    fn validate_asset_key_invalid_no_prefix() {
        assert!(validate_asset_key("-5").is_err());
    }

    #[test]
    fn validate_asset_key_invalid_no_hyphen() {
        assert!(validate_asset_key("foo").is_err());
    }

    #[test]
    fn validate_asset_key_invalid_empty() {
        assert!(validate_asset_key("").is_err());
    }

    #[test]
    fn validate_asset_key_invalid_spaces() {
        assert!(validate_asset_key("CU ST-5").is_err());
    }

    #[test]
    fn build_asset_clause_single_field() {
        let fields = vec![("customfield_10191".to_string(), "Client".to_string())];
        let clause = build_asset_clause("CUST-5", &fields);
        assert_eq!(clause, r#""Client" IN aqlFunction("Key = \"CUST-5\"")"#);
    }

    #[test]
    fn build_asset_clause_multiple_fields() {
        let fields = vec![
            ("customfield_10191".to_string(), "Client".to_string()),
            ("customfield_10245".to_string(), "Server".to_string()),
        ];
        let clause = build_asset_clause("SRV-42", &fields);
        assert_eq!(
            clause,
            r#"("Client" IN aqlFunction("Key = \"SRV-42\"") OR "Server" IN aqlFunction("Key = \"SRV-42\""))"#
        );
    }

    #[test]
    fn build_asset_clause_field_name_with_quotes() {
        let fields = vec![(
            "customfield_10191".to_string(),
            r#"My "Assets""#.to_string(),
        )];
        let clause = build_asset_clause("OBJ-1", &fields);
        assert_eq!(
            clause,
            r#""My \"Assets\"" IN aqlFunction("Key = \"OBJ-1\"")"#
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn has_unescaped_quote(s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            if c == '"' {
                let mut backslash_count = 0;
                let mut j = i;
                while j > 0 && chars[j - 1] == '\\' {
                    backslash_count += 1;
                    j -= 1;
                }
                if backslash_count % 2 == 0 {
                    return true;
                }
            }
        }
        false
    }

    proptest! {
        #[test]
        fn escaped_value_never_has_unescaped_quote(s in "\\PC{0,100}") {
            let escaped = escape_value(&s);
            prop_assert!(
                !has_unescaped_quote(&escaped),
                "Found unescaped quote in escaped output: {:?} -> {:?}",
                s,
                escaped
            );
        }
    }
}
