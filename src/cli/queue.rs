use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::jsm::servicedesks;
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

    let issues = client
        .get_queue_issues(service_desk_id, &queue_id, limit)
        .await?;

    let rows: Vec<Vec<String>> = issues
        .iter()
        .map(|i| {
            vec![
                i.key.clone(),
                i.fields
                    .issuetype
                    .as_ref()
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "\u{2014}".into()),
                i.fields
                    .summary
                    .clone()
                    .unwrap_or_else(|| "\u{2014}".into()),
                i.fields
                    .status
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "\u{2014}".into()),
                i.fields
                    .assignee
                    .as_ref()
                    .map(|u| u.display_name.clone())
                    .unwrap_or_else(|| "\u{2014}".into()),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["Key", "Type", "Summary", "Status", "Assignee"],
        &rows,
        &issues,
    )
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
}
