/// Result of attempting a partial match against a list of candidates.
pub enum MatchResult {
    /// Exactly one match found
    Exact(String),
    /// Multiple matches — caller should prompt for disambiguation
    Ambiguous(Vec<String>),
    /// No matches
    None(Vec<String>),
}

/// Case-insensitive substring match against candidates.
pub fn partial_match(input: &str, candidates: &[String]) -> MatchResult {
    let lower_input = input.to_lowercase();

    // Try exact match first (case-insensitive)
    for candidate in candidates {
        if candidate.to_lowercase() == lower_input {
            return MatchResult::Exact(candidate.clone());
        }
    }

    // Try substring match
    let matches: Vec<String> = candidates
        .iter()
        .filter(|c| c.to_lowercase().contains(&lower_input))
        .cloned()
        .collect();

    match matches.len() {
        0 => MatchResult::None(candidates.to_vec()),
        1 => MatchResult::Exact(matches.into_iter().next().unwrap()),
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
    fn test_partial_match_unique() {
        match partial_match("prog", &candidates()) {
            MatchResult::Exact(s) => assert_eq!(s, "In Progress"),
            _ => panic!("Expected unique match"),
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
    fn test_blocked_unique() {
        match partial_match("block", &candidates()) {
            MatchResult::Exact(s) => assert_eq!(s, "Blocked"),
            _ => panic!("Expected unique match"),
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
    }
}
