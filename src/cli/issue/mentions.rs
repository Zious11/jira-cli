//! Effectful mention resolution (Story B, ADR-0023 §1) — resolves every
//! `@Name` and bracket-form `[~accountid:<id>]` mention candidate that
//! `adf::find_mention_candidates` (Story A, pure, `S-cycle5-mention-pure-conversion`)
//! reports in a write command's text against real Jira users, BEFORE the text
//! is converted to ADF (`adf::markdown_to_adf_with_mentions`) and posted.
//!
//! This module is the "effectful shell" half of ADR-0023's pure/effectful
//! seam: it owns all network I/O (`GET /rest/api/3/user/search`,
//! `GET /rest/api/3/user?accountId=`) and all interactive
//! (`dialoguer::Select`) or hard-error disambiguation. `src/adf.rs` is never
//! modified by this module — it only calls the pure API Story A already
//! shipped (`find_mention_candidates`, `markdown_to_adf_with_mentions`,
//! `markdown_to_adf_no_mentions`, `MentionResolutions`).

use std::collections::HashSet;

use crate::adf::{self, MentionCandidateKind, MentionResolution, MentionResolutions};
use crate::api::client::JiraClient;
use crate::error::JrError;
use crate::types::jira::User;

use super::helpers;

/// Convert an `anyhow::Error` (the return type of `disambiguate_user`,
/// `client.search_users`, and `client.get_user`) into a `JrError`, preserving
/// the concrete variant when the error already wraps one (so exit-code
/// mapping and downstream `match`es on `JrError` still work), falling back to
/// `JrError::Internal` only for a genuinely foreign error (e.g. a
/// `dialoguer`/io context wrapper).
fn to_jr_error(e: anyhow::Error) -> JrError {
    match e.downcast::<JrError>() {
        Ok(jr) => jr,
        Err(e) => JrError::Internal(e.to_string()),
    }
}

/// Reduce `active_users` to those whose `display_name`
/// case-insensitively-substring-contains `query`, reusing
/// `partial_match::partial_match`'s existing classification (AC-002 step 2,
/// ADR-0023 §7 — the F2-gate human-approved single-result tightening).
///
/// Pure, in-memory reduction — zero I/O, zero `async`. Not part of `adf.rs`'s
/// formally-hardened pure core (this is a plain sync helper, like most of
/// `helpers.rs`), but it never touches the network or a `JiraClient`.
pub(super) fn filter_by_name_match(active_users: Vec<User>, query: &str) -> Vec<User> {
    let display_names: Vec<String> = active_users
        .iter()
        .map(|u| u.display_name.clone())
        .collect();
    match crate::partial_match::partial_match(query, &display_names) {
        crate::partial_match::MatchResult::Exact(m)
        | crate::partial_match::MatchResult::ExactMultiple(m) => {
            let m_lower = m.to_lowercase();
            active_users
                .into_iter()
                .filter(|u| u.display_name.to_lowercase() == m_lower)
                .collect()
        }
        crate::partial_match::MatchResult::Ambiguous(matches) => {
            let lower: HashSet<String> = matches.iter().map(|s| s.to_lowercase()).collect();
            active_users
                .into_iter()
                .filter(|u| lower.contains(&u.display_name.to_lowercase()))
                .collect()
        }
        crate::partial_match::MatchResult::None(_) => Vec::new(),
    }
}

/// Resolve a single bracket-form `[~accountid:<id>]` candidate via mandatory
/// preflight validation (AC-006, BC-X.7.010). 404/400 → `JrError::UserError`
/// ("not found", exit 64). Any other error (401/403/5xx/network) propagates
/// via the standard `JrError` mapping, NOT re-wrapped.
async fn resolve_bracket_candidate(
    client: &JiraClient,
    account_id: &str,
    resolutions: &mut MentionResolutions,
) -> Result<(), JrError> {
    match client.get_user(account_id).await {
        Ok(user) => {
            resolutions.insert_bracket(
                account_id.to_string(),
                MentionResolution {
                    account_id: user.account_id,
                    display_name: user.display_name,
                },
            );
            Ok(())
        }
        Err(e) => match to_jr_error(e) {
            JrError::ApiError { status, .. } if status == 404 || status == 400 => {
                Err(JrError::UserError(format!(
                    "Mention target \"{account_id}\" not found. The accountId may be invalid, \
                     stale, or the user may no longer exist."
                )))
            }
            other => Err(other),
        },
    }
}

/// Resolve a single `@Name` candidate (AC-002..AC-005, BC-X.7.007/008/009).
///
/// `span` is the candidate token AS REPORTED by `find_mention_candidates`
/// (leading `@` included) — used verbatim as the `MentionResolutions` key.
/// The search query sent to Jira strips the leading `@`.
async fn resolve_at_name_candidate(
    client: &JiraClient,
    span: &str,
    no_input: bool,
    resolutions: &mut MentionResolutions,
) -> Result<(), JrError> {
    let query = span.trim_start_matches('@');

    let raw_users = client.search_users(query).await.map_err(to_jr_error)?;
    let raw_empty = raw_users.is_empty();
    let all_deactivated = !raw_empty && raw_users.iter().all(|u| u.active != Some(true));
    let active_users: Vec<User> = raw_users
        .into_iter()
        .filter(|u| u.active == Some(true))
        .collect();
    let filtered = filter_by_name_match(active_users, query);

    // BC-X.7.009 point 2 — three-way empty_msg selection, based on a single
    // inspection of the RAW (pre-filter) search result: (i) genuinely no
    // user found, (ii) all matches were deactivated, (iii) an active match
    // exists but none name-matches the query. All three share the pinned
    // substring "No user found matching"; only (ii) carries the
    // "deactivated" hint.
    let empty_msg = if all_deactivated {
        format!(
            "No user found matching \"@{query}\" (a matching account exists but is deactivated) \
             — verify the spelling or use the [~accountid:<id>] form."
        )
    } else {
        format!(
            "No user found matching \"@{query}\" — verify the spelling or use the \
             [~accountid:<id>] form."
        )
    };

    // Determine the `name` value fed to `disambiguate_user`. BC-X.7.008
    // frames its ExactMultiple trigger as "two or more users share the exact
    // same display name" — a property of the CANDIDATES, not of exact
    // equality against the (possibly abbreviated) typed query. When the
    // name-matched, reduced set collapses to a single shared display name,
    // pass that display name so `disambiguate_user`'s own (UNCHANGED)
    // `partial_match` classification naturally lands on `ExactMultiple`;
    // otherwise pass the original query, which lands on `Ambiguous` for a
    // genuinely mixed set of matching names.
    let disambiguate_name: String = if filtered.len() >= 2 {
        let mut distinct: Vec<String> = filtered
            .iter()
            .map(|u| u.display_name.to_lowercase())
            .collect();
        distinct.sort();
        distinct.dedup();
        if distinct.len() == 1 {
            filtered[0].display_name.clone()
        } else {
            query.to_string()
        }
    } else {
        query.to_string()
    };

    let (account_id, display_name) = helpers::disambiguate_user(
        &filtered,
        &disambiguate_name,
        no_input,
        &empty_msg,
        |_all_names: &[String]| empty_msg.clone(),
    )
    .map_err(to_jr_error)?;

    resolutions.insert_at_name(
        span.to_string(),
        MentionResolution {
            account_id,
            display_name,
        },
    );
    Ok(())
}

/// Resolve every unique mention candidate found in `text` against real Jira
/// users (AC-001..AC-007; BC-X.7.007/008/009/010).
///
/// - Calls `adf::find_mention_candidates(text)` (pure, Story A) to discover
///   candidates, then deduplicates per unique bracket-form accountId and per
///   unique `@Name` token BEFORE any network call (AC-001).
/// - For each unique bracket-form accountId: mandatory `client.get_user(id)`
///   preflight validation (AC-006).
/// - For each unique `@Name` candidate: filters `client.search_users(name)`
///   results to `active == Some(true)`, reduces via `filter_by_name_match`,
///   then disambiguates via `disambiguate_user` (AC-002/003/004/005).
/// - All-or-nothing: EVERY candidate is attempted (no short-circuit on the
///   first failure — AC-007's zero-mutation guarantee is enforced by never
///   calling the mutation HTTP endpoint until this returns `Ok`, not by
///   skipping remaining candidates), and ANY resolution failure among
///   otherwise-resolvable candidates fails the whole call.
pub(super) async fn resolve_mentions(
    client: &JiraClient,
    text: &str,
    no_input: bool,
) -> Result<MentionResolutions, JrError> {
    let candidates = adf::find_mention_candidates(text)?;

    let mut seen_brackets: HashSet<String> = HashSet::new();
    let mut unique_brackets: Vec<String> = Vec::new();
    let mut seen_at_names: HashSet<String> = HashSet::new();
    let mut unique_at_names: Vec<String> = Vec::new();

    for c in &candidates.candidates {
        match c.kind {
            MentionCandidateKind::Bracket => {
                if seen_brackets.insert(c.span.clone()) {
                    unique_brackets.push(c.span.clone());
                }
            }
            MentionCandidateKind::AtName => {
                if seen_at_names.insert(c.span.clone()) {
                    unique_at_names.push(c.span.clone());
                }
            }
        }
    }

    let mut resolutions = MentionResolutions::empty();
    let mut first_err: Option<JrError> = None;

    for id in &unique_brackets {
        if let Err(e) = resolve_bracket_candidate(client, id, &mut resolutions).await {
            if first_err.is_none() {
                first_err = Some(e);
            }
        }
    }

    for span in &unique_at_names {
        if let Err(e) = resolve_at_name_candidate(client, span, no_input, &mut resolutions).await {
            if first_err.is_none() {
                first_err = Some(e);
            }
        }
    }

    if let Some(e) = first_err {
        return Err(e);
    }

    Ok(resolutions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_user(account_id: &str, display_name: &str) -> User {
        User {
            account_id: account_id.to_string(),
            display_name: display_name.to_string(),
            email_address: None,
            active: Some(true),
        }
    }

    /// F1 regression (adversarial-review finding, MEDIUM,
    /// correctness/notification-safety): `filter_by_name_match`'s
    /// `Exact | ExactMultiple` arm must classify names using the SAME
    /// Unicode `to_lowercase()` predicate as `partial_match` and the
    /// `Ambiguous` arm — never a second, independently-maintained
    /// ASCII-only fold. Two active accounts whose display names differ
    /// only by the case of a non-ASCII letter ("José" vs "JOSÉ") both
    /// case-insensitively (Unicode) match the query "josé" and must both
    /// survive the reduction, so the caller's later disambiguation step
    /// still sees 2 candidates rather than silently collapsing to 1.
    ///
    /// Before the fix, `eq_ignore_ascii_case` does not fold 'É' (U+00C9)
    /// and 'é' (U+00E9) — non-ASCII bytes must match exactly — so this
    /// incorrectly dropped "JOSÉ" and returned a single-element vec.
    #[test]
    fn test_filter_by_name_match_unicode_case_fold_keeps_both_ambiguous_users() {
        let active_users = vec![active_user("acc-1", "José"), active_user("acc-2", "JOSÉ")];

        let filtered = filter_by_name_match(active_users, "josé");

        assert_eq!(
            filtered.len(),
            2,
            "both accounts share the same Unicode-folded display name and must both \
             survive filtering, so disambiguate_user's len()==1 short-circuit is never \
             reached for a genuinely ambiguous pair; got {filtered:?}"
        );
    }
}
