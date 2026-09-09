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
//!
//! STUB NOTICE (S-cycle5-mention-resolution-wiring, stub-architect pass):
//! both functions below are `todo!()` per BC-5.38.001 — this file exists only
//! to establish a compilable surface for the four write-command call sites
//! (`create.rs`, `edit.rs`, `interactions.rs`, `jsm_create.rs`) to compile
//! against in a later wiring pass. Neither function contains any real logic;
//! implementing them (AC-001..AC-007) is TDD work for the test-writer /
//! implementer stages, not this stub-architect pass.

use crate::adf::MentionResolutions;
use crate::api::client::JiraClient;
use crate::error::JrError;
use crate::types::jira::User;

/// Resolve every unique mention candidate found in `text` against real Jira
/// users (AC-001..AC-007; BC-X.7.007/008/009/010).
///
/// Intended behavior (implemented by a later story task, NOT this stub):
/// - Calls `adf::find_mention_candidates(text)` (pure, Story A) to discover
///   candidates, then deduplicates per unique bracket-form accountId and per
///   unique `@Name` token BEFORE any network call (AC-001).
/// - For each unique `@Name` candidate: filters `client.search_users(name)`
///   results to `active == Some(true)`, reduces via `filter_by_name_match`,
///   then disambiguates via `disambiguate_user` (AC-002/003/004/005).
/// - For each unique bracket-form accountId: mandatory `client.get_user(id)`
///   preflight validation (AC-006).
/// - All-or-nothing: ANY resolution failure among otherwise-resolvable
///   candidates fails the whole call — callers MUST NOT issue any
///   mutation HTTP (POST/PUT) until this returns `Ok` (AC-007).
pub(super) async fn resolve_mentions(
    client: &JiraClient,
    text: &str,
    no_input: bool,
) -> Result<MentionResolutions, JrError> {
    let _ = (client, text, no_input);
    todo!("S-cycle5-mention-resolution-wiring: resolve_mentions (AC-001..AC-007)")
}

/// Reduce `active_users` to those whose `display_name`
/// case-insensitively-substring-contains `query`, reusing
/// `partial_match::partial_match`'s existing classification rather than a
/// second, independently-maintained name-match predicate (AC-002 step 2,
/// ADR-0023 §7 — the F2-gate human-approved single-result tightening).
///
/// Intended behavior (implemented by a later story task, NOT this stub): a
/// pure, in-memory reduction over already-fetched data — zero I/O.
pub(super) fn filter_by_name_match(active_users: Vec<User>, query: &str) -> Vec<User> {
    let _ = (active_users, query);
    todo!("S-cycle5-mention-resolution-wiring: filter_by_name_match (AC-002/003, ADR-0023 §7)")
}
