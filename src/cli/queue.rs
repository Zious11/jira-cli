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

    let service_desk_id = servicedesks::require_service_desk(client, &project_key).await?;

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

async fn handle_view(
    service_desk_id: &str,
    name: Option<String>,
    id: Option<String>,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let queue_id = match id {
        Some(id) => id,
        None => {
            let name = name.ok_or_else(|| {
                JrError::UserError(
                    "Specify a queue name or use --id. \
                     Run \"jr queue list\" to see available queues."
                        .into(),
                )
            })?;
            resolve_queue_by_name(service_desk_id, &name, client).await?
        }
    };

    // Step 1: Fetch issue keys from the queue (preserves queue membership and ordering)
    let keys = client
        .get_queue_issue_keys(service_desk_id, &queue_id, limit)
        .await?;

    if keys.is_empty() {
        let headers = issue_table_headers(false, false);
        let empty: Vec<Vec<String>> = vec![];
        let empty_issues: Vec<crate::types::jira::Issue> = vec![];
        return output::print_output(output_format, &headers, &empty, &empty_issues);
    }

    // Step 2: Batch-fetch full issues via search API
    let jql = build_key_in_jql(&keys);
    let search_result = client
        .search_issues(&jql, Some(keys.len() as u32), &[])
        .await?;

    // Step 3: Re-order results to match original queue ordering
    let issues = reorder_by_queue_position(search_result.issues, &keys);

    // Step 4: Output
    let headers = issue_table_headers(false, false);
    let rows = format_issue_rows_public(&issues);
    output::print_output(output_format, &headers, &rows, &issues)
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

async fn resolve_queue_by_name(
    service_desk_id: &str,
    name: &str,
    client: &JiraClient,
) -> Result<String> {
    let queues = client.list_queues(service_desk_id).await?;
    let names: Vec<String> = queues.iter().map(|q| q.name.clone()).collect();

    match partial_match::partial_match(name, &names) {
        MatchResult::Exact(matched_name) => {
            let matching: Vec<&crate::types::jsm::Queue> =
                queues.iter().filter(|q| q.name == matched_name).collect();

            if matching.len() > 1 {
                let ids: Vec<String> = matching.iter().map(|q| q.id.clone()).collect();
                Err(JrError::UserError(format!(
                    "Multiple queues named \"{}\" found (IDs: {}). Use --id {} to specify.",
                    matched_name,
                    ids.join(", "),
                    ids[0]
                ))
                .into())
            } else {
                Ok(matching[0].id.clone())
            }
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
            crate::partial_match::MatchResult::Exact(matched_name) => {
                let matching: Vec<&Queue> =
                    queues.iter().filter(|q| q.name == matched_name).collect();
                if matching.len() > 1 {
                    Err(format!("duplicate: {}", matching.len()))
                } else {
                    Ok(matching[0].id.clone())
                }
            }
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
    fn partial_match() {
        let queues = vec![make_queue("10", "Triage"), make_queue("20", "In Progress")];
        assert_eq!(find_queue_id("tri", &queues).unwrap(), "10");
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

    use super::{build_key_in_jql, reorder_by_queue_position};
    use crate::types::jira::Issue;

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
}
