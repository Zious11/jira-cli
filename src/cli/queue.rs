use std::collections::HashMap;

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::jsm::servicedesks;
use crate::cli::issue::{format_issue_rows_public, issue_table_headers};
use crate::cli::{OutputFormat, QueueCommand};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};

pub async fn handle(
    command: QueueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
) -> Result<()> {
    let project_key = config.project_key(project_override).ok_or_else(|| {
        JrError::UserError(
            "No project configured. Run \"jr init\" or pass --project. \
             Run \"jr project list\" to see available projects."
                .into(),
        )
    })?;

    let service_desk_id = servicedesks::require_service_desk(
        client,
        &project_key,
        "Queue commands (`jr queue`) require",
    )
    .await?;

    match command {
        QueueCommand::List => handle_list(&service_desk_id, output_format, client).await,
        QueueCommand::View { name, id, limit } => {
            handle_view(&service_desk_id, name, id, limit, output_format, client).await
        }
    }
}

async fn handle_list(
    service_desk_id: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let queues = client.list_queues(service_desk_id).await?;

    let rows: Vec<Vec<String>> = queues
        .iter()
        .map(|q| {
            vec![
                q.name.clone(),
                q.issue_count
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "\u{2014}".into()),
            ]
        })
        .collect();

    output::print_output(output_format, &["Queue", "Issues"], &rows, &queues)
}

/// Where a resolved queue's declared `fields[]` (for `extra_fields`
/// purposes) came from — determines whether an auxiliary `list_queues`
/// lookup is still needed once we know the queue has issues to fetch.
enum QueueIdSource {
    /// `--id` was supplied directly; `queue.fields` is not yet known and
    /// costs one additional `list_queues` call (deferred until AFTER the
    /// zero-issue short-circuit — BC-X.8.009 EC-X.8.009-1/AC-7).
    ById(String),
    /// Resolved via `resolve_queue_by_name`, which already fetched the full
    /// `Queue` (including `fields[]`) as part of name resolution — zero
    /// additional HTTP cost.
    ByName {
        id: String,
        fields: Option<Vec<String>>,
    },
}

impl QueueIdSource {
    fn id(&self) -> &str {
        match self {
            QueueIdSource::ById(id) => id,
            QueueIdSource::ByName { id, .. } => id,
        }
    }
}

async fn handle_view(
    service_desk_id: &str,
    name: Option<String>,
    id: Option<String>,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let source = match id {
        Some(id) => QueueIdSource::ById(id),
        None => {
            let name = name.ok_or_else(|| {
                JrError::UserError(
                    "Specify a queue name or use --id. \
                     Run \"jr queue list\" to see available queues."
                        .into(),
                )
            })?;
            let (id, fields) = resolve_queue_by_name(service_desk_id, &name, client).await?;
            QueueIdSource::ByName { id, fields }
        }
    };
    let queue_id = source.id().to_string();

    // Apply default limit consistent with other commands (issue list, board view, sprint current)
    let effective_limit = limit.or(Some(crate::cli::DEFAULT_LIMIT));

    // Step 1: Fetch issue keys from the queue (preserves queue membership and ordering)
    let keys = client
        .get_queue_issue_keys(service_desk_id, &queue_id, effective_limit)
        .await?;

    if keys.is_empty() {
        let headers = issue_table_headers(false, false, false, false);
        let empty: Vec<Vec<String>> = vec![];
        let empty_issues: Vec<crate::types::jira::Issue> = vec![];
        return output::print_output(output_format, &headers, &empty, &empty_issues);
    }

    // Step 1.5: resolve the queue's declared custom-field columns. Name path
    // already has them (zero cost); `--id` path pays one auxiliary
    // `list_queues` call here, fail-open on error/no-match (BC-X.8.009
    // EC-X.8.009-1) — deliberately deferred until now so a zero-issue queue
    // never pays this cost (AC-7).
    //
    // `list_queues` (list-all + local id match) is used here pending
    // verification that the single-queue `GET .../queue/{queueId}` endpoint
    // also returns `fields[]` — if confirmed, that endpoint would let the
    // `--id` path fetch by id directly instead of listing and filtering
    // client-side (pr-reviewer S1 follow-up; deferred, not re-litigated here).
    let queue_fields: Option<Vec<String>> = match source {
        QueueIdSource::ByName { fields, .. } => fields,
        QueueIdSource::ById(id) => match client.list_queues(service_desk_id).await {
            Ok(queues) => match queues.into_iter().find(|q| q.id == id) {
                Some(q) => q.fields,
                None => {
                    eprintln!(
                        "warning: could not fetch queue field configuration for --id {id} \
                         (no matching queue); showing base fields only."
                    );
                    None
                }
            },
            Err(e) => {
                eprintln!(
                    "warning: could not fetch queue field configuration for --id {id} \
                     ({}); showing base fields only.",
                    describe_aux_lookup_error(&e)
                );
                None
            }
        },
    };
    let extra_fields = extra_fields_allow_list(queue_fields.as_deref());

    // Step 2: Batch-fetch full issues via search API.
    let jql = build_key_in_jql(&keys);
    let search_result = client
        .search_issues(&jql, Some(keys.len() as u32), &extra_fields)
        .await?;

    // Step 3: Re-order results to match original queue ordering
    let mut issues = reorder_by_queue_position(search_result.issues, &keys);

    // Scope each issue's `IssueFields::extra` (`#[serde(flatten)]`) to
    // exactly the `extra_fields` this call requested. Jira itself only
    // returns fields present in the request's `fields` list, so this is
    // belt-and-suspenders in production — but it's load-bearing for the
    // `--id`-path degrade (BC-X.8.009 EC-X.8.009-1): once a failed/no-match
    // auxiliary lookup drops `extra_fields` back to `&[]`, the resulting
    // `Issue` objects must show base fields only, never a stray customfield_*
    // key.
    for issue in &mut issues {
        issue
            .fields
            .extra
            .retain(|k, _| extra_fields.contains(&k.as_str()));
    }

    // Step 4: Output
    let headers = issue_table_headers(false, false, false, false);
    let rows = format_issue_rows_public(&issues);
    output::print_output(output_format, &headers, &rows, &issues)
}

/// Terse, model-b-style cause description for a failed queue-fields
/// auxiliary lookup — never the raw HTTP error body (same convention as
/// `write_cmdb_fields_cache`/`write_object_type_attr_cache`, see CLAUDE.md).
fn describe_aux_lookup_error(err: &anyhow::Error) -> String {
    let cause = match err.downcast_ref::<JrError>() {
        Some(JrError::ApiError { status, .. }) => format!("API error ({status})"),
        Some(JrError::NotAuthenticated { .. }) => "not authenticated".to_string(),
        Some(other) => other.to_string(),
        None => err.to_string(),
    };
    // The two named branches above never contain newlines, but the
    // fallthrough `other`/`err.to_string()` arms can carry an arbitrary
    // (possibly multi-line, possibly very long) error message — collapse
    // and cap it so it can never split the one-line `warning: …`
    // diagnostic across multiple stderr lines or dominate it.
    collapse_and_truncate(&cause)
}

/// Collapses all whitespace (including embedded newlines) to single spaces
/// and caps the result at [`MAX_CAUSE_LEN`] characters (UTF-8-safe, appending
/// `…` when truncated). Used by [`describe_aux_lookup_error`] to keep the
/// `<cause>` slot in the degrade warning terse and single-line, matching the
/// model-b convention (never a raw multi-line HTTP body dump).
const MAX_CAUSE_LEN: usize = 200;

fn collapse_and_truncate(s: &str) -> String {
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > MAX_CAUSE_LEN {
        let truncated: String = collapsed.chars().take(MAX_CAUSE_LEN).collect();
        format!("{truncated}\u{2026}")
    } else {
        collapsed
    }
}

/// Allow-list filter (NOT a drop-list, BC-X.8.009 step 3): keep only tokens
/// matching the anchored, case-sensitive pattern `^customfield_\d+$` (full
/// string, one or more ASCII digits, no upper bound). Every other token —
/// pseudo-columns (`issuekey`), base fields, and malformed near-misses
/// (`customfield_`, `customfield_10050_x`, `Customfield_99`) — is dropped.
/// `fields: None` (or an empty/all-rejected array) yields an empty `Vec`,
/// byte-identical to the pre-#693 `extra_fields = &[]` behavior. Duplicate
/// tokens in the source `fields[]` are deduplicated, preserving first-seen
/// order — a queue declaring `customfield_10050` twice must not send it
/// twice to `search_issues`.
fn extra_fields_allow_list(fields: Option<&[String]>) -> Vec<&str> {
    let Some(fs) = fields else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    fs.iter()
        .filter(|f| is_customfield_token(f))
        .map(|s| s.as_str())
        .filter(|s| seen.insert(*s))
        .collect()
}

/// True iff `s` matches `^customfield_\d+$` exactly (anchored, one or more
/// ASCII digits, case-sensitive).
fn is_customfield_token(s: &str) -> bool {
    match s.strip_prefix("customfield_") {
        Some(rest) => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

/// Build a JQL `key IN (...)` clause from a list of issue keys.
/// Issue keys are identifiers in JQL and must NOT be quoted.
fn build_key_in_jql(keys: &[String]) -> String {
    format!("key IN ({})", keys.join(", "))
}

/// Re-order issues to match the original queue key ordering.
/// Issues not found in the search results (e.g., permission-denied) are silently omitted.
fn reorder_by_queue_position(
    mut issues: Vec<crate::types::jira::Issue>,
    queue_keys: &[String],
) -> Vec<crate::types::jira::Issue> {
    let position: HashMap<&str, usize> = queue_keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();
    issues.sort_by_key(|issue| {
        position
            .get(issue.key.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    issues
}

pub async fn resolve_queue_by_name(
    service_desk_id: &str,
    name: &str,
    client: &JiraClient,
) -> Result<(String, Option<Vec<String>>)> {
    let queues = client.list_queues(service_desk_id).await?;
    let names: Vec<String> = queues.iter().map(|q| q.name.clone()).collect();

    match partial_match::partial_match(name, &names) {
        MatchResult::Exact(matched_name) => {
            let queue = queues
                .into_iter()
                .find(|q| q.name == matched_name)
                .expect("matched name must exist in queues");
            Ok((queue.id, queue.fields))
        }
        MatchResult::ExactMultiple(matched_name) => {
            let name_lower = name.to_lowercase();
            let matching: Vec<&crate::types::jsm::Queue> = queues
                .iter()
                .filter(|q| q.name.to_lowercase() == name_lower)
                .collect();
            let ids: Vec<String> = matching.iter().map(|q| q.id.clone()).collect();
            Err(JrError::UserError(format!(
                "Multiple queues named \"{}\" found (IDs: {}). Use --id {} to specify.",
                matched_name,
                ids.join(", "),
                ids[0]
            ))
            .into())
        }
        MatchResult::Ambiguous(matches) => Err(JrError::UserError(format!(
            "\"{}\" matches multiple queues: {}. Be more specific or use --id.",
            name,
            matches
                .iter()
                .map(|m| format!("\"{}\"", m))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into()),
        MatchResult::None(_) => Err(JrError::UserError(format!(
            "No queue matching \"{}\" found. \
             Run \"jr queue list\" to see available queues.",
            name
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_key_in_jql, extra_fields_allow_list, is_customfield_token, reorder_by_queue_position,
    };
    use crate::types::jira::Issue;
    use crate::types::jsm::Queue;

    fn make_queue(id: &str, name: &str) -> Queue {
        Queue {
            id: id.into(),
            name: name.into(),
            jql: None,
            fields: None,
            issue_count: None,
        }
    }

    fn find_queue_id(name: &str, queues: &[Queue]) -> Result<String, String> {
        let names: Vec<String> = queues.iter().map(|q| q.name.clone()).collect();
        match crate::partial_match::partial_match(name, &names) {
            crate::partial_match::MatchResult::Exact(matched_name) => Ok(queues
                .iter()
                .find(|q| q.name == matched_name)
                .expect("matched name must exist in queues")
                .id
                .clone()),
            crate::partial_match::MatchResult::ExactMultiple(_) => Err("duplicate".into()),
            crate::partial_match::MatchResult::Ambiguous(m) => {
                Err(format!("ambiguous: {}", m.len()))
            }
            crate::partial_match::MatchResult::None(_) => Err("none".into()),
        }
    }

    #[test]
    fn exact_match() {
        let queues = vec![make_queue("10", "Triage"), make_queue("20", "In Progress")];
        assert_eq!(find_queue_id("Triage", &queues).unwrap(), "10");
    }

    #[test]
    fn single_substring_is_ambiguous() {
        // Single substring hits are now Ambiguous — callers must use the exact name.
        let queues = vec![make_queue("10", "Triage"), make_queue("20", "In Progress")];
        let err = find_queue_id("tri", &queues).unwrap_err();
        assert!(err.starts_with("ambiguous"), "got: {err}");
    }

    #[test]
    fn ambiguous_match() {
        let queues = vec![
            make_queue("10", "Escalated - Client"),
            make_queue("20", "Escalated - External"),
        ];
        let err = find_queue_id("esc", &queues).unwrap_err();
        assert!(err.starts_with("ambiguous"));
    }

    #[test]
    fn no_match() {
        let queues = vec![make_queue("10", "Triage")];
        let err = find_queue_id("nonexistent", &queues).unwrap_err();
        assert_eq!(err, "none");
    }

    #[test]
    fn duplicate_names() {
        let queues = vec![make_queue("10", "Triage"), make_queue("20", "Triage")];
        let err = find_queue_id("Triage", &queues).unwrap_err();
        assert!(err.starts_with("duplicate"));
    }

    fn make_issue(key: &str) -> Issue {
        Issue {
            key: key.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn build_jql_single_key() {
        let jql = build_key_in_jql(&["FOO-1".to_string()]);
        assert_eq!(jql, "key IN (FOO-1)");
    }

    #[test]
    fn build_jql_multiple_keys() {
        let keys = vec![
            "FOO-1".to_string(),
            "FOO-2".to_string(),
            "BAR-99".to_string(),
        ];
        let jql = build_key_in_jql(&keys);
        assert_eq!(jql, "key IN (FOO-1, FOO-2, BAR-99)");
    }

    #[test]
    fn reorder_matches_queue_order() {
        let issues = vec![
            make_issue("FOO-3"),
            make_issue("FOO-1"),
            make_issue("FOO-2"),
        ];
        let queue_keys = vec!["FOO-1".into(), "FOO-2".into(), "FOO-3".into()];
        let result = reorder_by_queue_position(issues, &queue_keys);
        let keys: Vec<&str> = result.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, vec!["FOO-1", "FOO-2", "FOO-3"]);
    }

    #[test]
    fn reorder_omits_nothing_on_full_match() {
        let issues = vec![make_issue("A-1"), make_issue("A-2")];
        let queue_keys = vec!["A-2".into(), "A-1".into()];
        let result = reorder_by_queue_position(issues, &queue_keys);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].key, "A-2");
        assert_eq!(result[1].key, "A-1");
    }

    #[test]
    fn reorder_with_missing_key_from_search() {
        let issues = vec![make_issue("A-1"), make_issue("A-3")];
        let queue_keys = vec!["A-1".into(), "A-2".into(), "A-3".into()];
        let result = reorder_by_queue_position(issues, &queue_keys);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].key, "A-1");
        assert_eq!(result[1].key, "A-3");
    }

    #[test]
    fn reorder_empty_issues() {
        let issues: Vec<Issue> = vec![];
        let queue_keys = vec!["A-1".into()];
        let result = reorder_by_queue_position(issues, &queue_keys);
        assert!(result.is_empty());
    }

    // ─── BC-X.8.009 (#693) — pure allow-list helper coverage ───────────────

    #[test]
    fn test_is_customfield_token_accepts_valid_shapes() {
        for accepted in ["customfield_10050", "customfield_1", "customfield_0"] {
            assert!(
                is_customfield_token(accepted),
                "expected {accepted:?} to be accepted"
            );
        }
    }

    #[test]
    fn test_is_customfield_token_rejects_malformed_and_non_matching_shapes() {
        for rejected in [
            "customfield_",        // zero digits
            "customfield_10050_x", // trailing non-digit content
            "Customfield_99",      // wrong case
            "issuekey",            // pseudo-column
            "summary",             // BASE_ISSUE_FIELDS member
            "status",              // BASE_ISSUE_FIELDS member
            "customfield_-1",      // signed — '-' is not an ASCII digit
            "customfield_+1",      // signed — '+' is not an ASCII digit
            "customfield_ 1",      // padded with a space
            "customfield_10050 ",  // trailing space
            "customfield_१०",      // Unicode (Devanagari) digits, not ASCII
            "",                    // empty string
        ] {
            assert!(
                !is_customfield_token(rejected),
                "expected {rejected:?} to be rejected"
            );
        }
    }

    #[test]
    fn test_extra_fields_allow_list_none_yields_empty() {
        assert!(extra_fields_allow_list(None).is_empty());
    }

    #[test]
    fn test_extra_fields_allow_list_keeps_only_customfield_tokens() {
        let fields = vec![
            "issuekey".to_string(),
            "summary".to_string(),
            "customfield_10050".to_string(),
            "customfield_".to_string(),
            "customfield_10050_x".to_string(),
            "Customfield_99".to_string(),
        ];
        assert_eq!(
            extra_fields_allow_list(Some(&fields)),
            vec!["customfield_10050"]
        );
    }

    #[test]
    fn test_extra_fields_allow_list_empty_slice_yields_empty() {
        let fields: Vec<String> = vec![];
        assert!(extra_fields_allow_list(Some(&fields)).is_empty());
    }

    #[test]
    fn test_extra_fields_allow_list_all_rejected_yields_empty() {
        let fields = vec!["issuekey".to_string(), "status".to_string()];
        assert!(extra_fields_allow_list(Some(&fields)).is_empty());
    }

    #[test]
    fn test_extra_fields_allow_list_dedups_preserving_first_seen_order() {
        let fields = vec![
            "customfield_10050".to_string(),
            "customfield_20099".to_string(),
            "customfield_10050".to_string(),
        ];
        assert_eq!(
            extra_fields_allow_list(Some(&fields)),
            vec!["customfield_10050", "customfield_20099"]
        );
    }
}
