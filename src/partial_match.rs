/// Result of attempting a partial match against a list of candidates.
#[derive(Debug)]
pub enum MatchResult {
    /// Exactly one match found
    Exact(String),
    /// Multiple candidates share the same exact (case-insensitive) name — carries the first matching candidate
    ExactMultiple(String),
    /// Non-exact matches (one or more substring hits) — caller should prompt
    /// for disambiguation (TTY) or error with the candidate list (--no-input)
    Ambiguous(Vec<String>),
    /// No matches
    None(Vec<String>),
}

/// Case-insensitive substring match against candidates.
pub fn partial_match(input: &str, candidates: &[String]) -> MatchResult {
    let lower_input = input.to_lowercase();

    // Collect all exact matches (case-insensitive)
    let exact_matches: Vec<String> = candidates
        .iter()
        .filter(|c| c.to_lowercase() == lower_input)
        .cloned()
        .collect();

    match exact_matches.len() {
        1 => return MatchResult::Exact(exact_matches.into_iter().next().unwrap()),
        n if n > 1 => return MatchResult::ExactMultiple(exact_matches.into_iter().next().unwrap()),
        _ => {}
    }

    // Try substring match
    let matches: Vec<String> = candidates
        .iter()
        .filter(|c| c.to_lowercase().contains(&lower_input))
        .cloned()
        .collect();

    match matches.len() {
        0 => MatchResult::None(candidates.to_vec()),
        _ => MatchResult::Ambiguous(matches),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<String> {
        vec![
            "In Progress".into(),
            "In Review".into(),
            "Blocked".into(),
            "Done".into(),
        ]
    }

    #[test]
    fn test_exact_match_case_insensitive() {
        match partial_match("in progress", &candidates()) {
            MatchResult::Exact(s) => assert_eq!(s, "In Progress"),
            _ => panic!("Expected exact match"),
        }
    }

    #[test]
    fn test_partial_match_single_substring_is_ambiguous() {
        // Single substring hits route through Ambiguous so callers can
        // prompt (TTY) or error (--no-input) — never silently resolve.
        match partial_match("prog", &candidates()) {
            MatchResult::Ambiguous(matches) => {
                assert_eq!(matches, vec!["In Progress".to_string()]);
            }
            other => panic!("Expected Ambiguous, got {:?}", other),
        }
    }

    #[test]
    fn test_partial_match_ambiguous() {
        match partial_match("In", &candidates()) {
            MatchResult::Ambiguous(matches) => {
                assert_eq!(matches.len(), 2);
                assert!(matches.contains(&"In Progress".to_string()));
                assert!(matches.contains(&"In Review".to_string()));
            }
            _ => panic!("Expected ambiguous match"),
        }
    }

    #[test]
    fn test_no_match() {
        match partial_match("Deployed", &candidates()) {
            MatchResult::None(all) => assert_eq!(all.len(), 4),
            _ => panic!("Expected no match"),
        }
    }

    #[test]
    fn test_blocked_single_substring_is_ambiguous() {
        match partial_match("block", &candidates()) {
            MatchResult::Ambiguous(matches) => {
                assert_eq!(matches, vec!["Blocked".to_string()]);
            }
            other => panic!("Expected Ambiguous, got {:?}", other),
        }
    }

    #[test]
    fn test_exact_match_duplicate_returns_exact_multiple() {
        let candidates = vec!["John Smith".into(), "Jane Doe".into(), "John Smith".into()];
        match partial_match("John Smith", &candidates) {
            MatchResult::ExactMultiple(name) => {
                assert_eq!(name, "John Smith");
            }
            other => panic!("Expected ExactMultiple, got {:?}", other),
        }
    }

    #[test]
    fn test_exact_match_duplicate_case_insensitive() {
        let candidates = vec!["John Smith".into(), "john smith".into()];
        match partial_match("john smith", &candidates) {
            MatchResult::ExactMultiple(name) => {
                // Preserves casing of the first match
                assert_eq!(name, "John Smith");
            }
            other => panic!("Expected ExactMultiple, got {:?}", other),
        }
    }

    #[test]
    fn test_exact_match_three_duplicates() {
        let candidates = vec![
            "John Smith".into(),
            "Jane Doe".into(),
            "John Smith".into(),
            "John Smith".into(),
        ];
        match partial_match("John Smith", &candidates) {
            MatchResult::ExactMultiple(name) => {
                assert_eq!(name, "John Smith");
            }
            other => panic!("Expected ExactMultiple, got {:?}", other),
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn exact_match_always_found(idx in 0usize..4) {
            let candidates: Vec<String> = vec![
                "In Progress".into(), "In Review".into(),
                "Blocked".into(), "Done".into(),
            ];
            let input = candidates[idx].clone();
            match partial_match(&input, &candidates) {
                MatchResult::Exact(s) => prop_assert_eq!(s, input),
                _ => prop_assert!(false, "Expected exact match for '{}'", input),
            }
        }

        #[test]
        fn never_panics_on_arbitrary_input(s in "\\PC{0,50}") {
            let candidates = vec!["In Progress".into(), "Done".into()];
            let _ = partial_match(&s, &candidates); // must not panic
        }

        #[test]
        fn empty_candidates_always_returns_none(s in "[a-z]{1,10}") {
            let candidates: Vec<String> = vec![];
            match partial_match(&s, &candidates) {
                MatchResult::None(all) => prop_assert!(all.is_empty()),
                _ => prop_assert!(false, "Expected None for empty candidates"),
            }
        }

        #[test]
        fn duplicate_candidates_yield_exact_multiple(idx in 0usize..4) {
            let base: Vec<String> = vec![
                "In Progress".into(), "In Review".into(),
                "Blocked".into(), "Done".into(),
            ];
            // Duplicate one candidate
            let mut candidates = base.clone();
            candidates.push(base[idx].clone());
            let input = base[idx].clone();
            match partial_match(&input, &candidates) {
                MatchResult::ExactMultiple(name) => {
                    prop_assert_eq!(name.to_lowercase(), input.to_lowercase());
                }
                _ => prop_assert!(false, "Expected ExactMultiple for duplicated '{}'", input),
            }
        }
    }
}
