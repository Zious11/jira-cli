/// Escape a value for interpolation into a JQL double-quoted string literal.
///
/// Backslashes are escaped first, then double quotes. Order matters: escaping
/// quotes first would introduce backslashes that the second pass re-escapes,
/// leaving the quote exposed (escape neutralization attack).
pub fn escape_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Strip `ORDER BY` clause from JQL for use with count-only endpoints.
///
/// The approximate-count endpoint only needs the WHERE clause. ORDER BY is
/// meaningless for a count and may cause issues with bounded-JQL validation.
pub fn strip_order_by(jql: &str) -> &str {
    let upper = jql.to_uppercase();
    if let Some(pos) = upper.find(" ORDER BY") {
        jql[..pos].trim_end()
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
