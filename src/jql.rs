/// Escape a value for interpolation into a JQL double-quoted string literal.
///
/// Backslashes are escaped first, then double quotes. Order matters: escaping
/// quotes first would introduce backslashes that the second pass re-escapes,
/// leaving the quote exposed (escape neutralization attack).
pub fn escape_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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
