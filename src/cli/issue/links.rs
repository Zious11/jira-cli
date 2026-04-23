use super::json_output;
use anyhow::{Context, Result, bail};

use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};

// ── Link Types ────────────────────────────────────────────────────

pub(super) async fn handle_link_types(
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let link_types = client.list_link_types().await?;

    let rows: Vec<Vec<String>> = link_types
        .iter()
        .map(|lt| {
            vec![
                lt.id.clone().unwrap_or_default(),
                lt.name.clone(),
                lt.outward.clone().unwrap_or_default(),
                lt.inward.clone().unwrap_or_default(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "Name", "Outward", "Inward"],
        &rows,
        &link_types,
    )?;

    Ok(())
}

// ── Link ──────────────────────────────────────────────────────────

pub(super) async fn handle_link(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Link {
        key1,
        key2,
        r#type: link_type_name,
    } = command
    else {
        unreachable!()
    };

    if key1.eq_ignore_ascii_case(&key2) {
        bail!("Cannot link an issue to itself.");
    }

    let link_types = client.list_link_types().await?;
    let type_names: Vec<String> = link_types.iter().map(|lt| lt.name.clone()).collect();
    let resolved_name = match partial_match::partial_match(&link_type_name, &type_names) {
        MatchResult::Exact(name) => name,
        // Link types are unique per Jira API; treat like Exact if duplicates ever occur
        MatchResult::ExactMultiple(name) => name,
        MatchResult::Ambiguous(matches) => {
            if no_input {
                return Err(JrError::UserError(format!(
                    "Ambiguous link type \"{}\". Matches: {}",
                    link_type_name,
                    matches.join(", ")
                ))
                .into());
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple types match \"{link_type_name}\""))
                .items(&matches)
                .interact()
                .context("failed to prompt for link type selection")?;
            matches[selection].clone()
        }
        MatchResult::None(_) => {
            bail!(
                "Unknown link type \"{}\". Run \"jr issue link-types\" to see available types.",
                link_type_name
            );
        }
    };

    client
        .create_issue_link(&key1, &key2, &resolved_name)
        .await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json_output::link_response(
                    &key1,
                    &key2,
                    &resolved_name,
                ))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Linked {} → {} ({})", key1, key2, resolved_name));
        }
    }

    Ok(())
}

// ── Unlink ────────────────────────────────────────────────────────

pub(super) async fn handle_unlink(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Unlink {
        key1,
        key2,
        r#type: link_type_filter,
    } = command
    else {
        unreachable!()
    };

    let resolved_type_filter = if let Some(ref type_name) = link_type_filter {
        let link_types = client.list_link_types().await?;
        let type_names: Vec<String> = link_types.iter().map(|lt| lt.name.clone()).collect();
        let resolved = match partial_match::partial_match(type_name, &type_names) {
            MatchResult::Exact(name) => name,
            // Link types are unique per Jira API; treat like Exact if duplicates ever occur
            MatchResult::ExactMultiple(name) => name,
            MatchResult::Ambiguous(matches) => {
                if no_input {
                    return Err(JrError::UserError(format!(
                        "Ambiguous link type \"{}\". Matches: {}",
                        type_name,
                        matches.join(", ")
                    ))
                    .into());
                }
                let selection = dialoguer::Select::new()
                    .with_prompt(format!("Multiple types match \"{type_name}\""))
                    .items(&matches)
                    .interact()
                    .context("failed to prompt for link type selection")?;
                matches[selection].clone()
            }
            MatchResult::None(_) => {
                bail!(
                    "Unknown link type \"{}\". Run \"jr issue link-types\" to see available types.",
                    type_name
                );
            }
        };
        Some(resolved)
    } else {
        None
    };

    let issue = client.get_issue(&key1, &[]).await?;
    let links = issue.fields.issuelinks.unwrap_or_default();

    let matching_links: Vec<&crate::types::jira::issue::IssueLink> = links
        .iter()
        .filter(|link| {
            let matches_key = link
                .outward_issue
                .as_ref()
                .map(|i| i.key.eq_ignore_ascii_case(&key2))
                .unwrap_or(false)
                || link
                    .inward_issue
                    .as_ref()
                    .map(|i| i.key.eq_ignore_ascii_case(&key2))
                    .unwrap_or(false);

            let matches_type = resolved_type_filter
                .as_ref()
                .map(|t| link.link_type.name.eq_ignore_ascii_case(t))
                .unwrap_or(true);

            matches_key && matches_type
        })
        .collect();

    if matching_links.is_empty() {
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json_output::unlink_response(false, 0))?
                );
            }
            OutputFormat::Table => {
                output::print_success(&format!("No link found between {} and {}", key1, key2));
            }
        }
        return Ok(());
    }

    let count = matching_links.len();
    for link in &matching_links {
        client.delete_issue_link(&link.id).await?;
    }

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json_output::unlink_response(true, count))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Removed {} link(s) between {} and {}",
                count, key1, key2
            ));
        }
    }

    Ok(())
}

// ── Remote Link ───────────────────────────────────────────────────

pub(super) async fn handle_remote_link(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::RemoteLink { key, url, title } = command else {
        unreachable!()
    };

    // Input validation — Jira's /remotelink endpoint accepts any string for
    // `object.url` without verifying it's a real URL. Creating a link to
    // "not-a-url" would succeed silently and produce a broken remote link
    // in the Jira UI. Validate on the CLI boundary instead.
    let url = url.trim();
    if url.is_empty() {
        return Err(JrError::UserError("--url must not be empty.".into()).into());
    }
    let parsed = url::Url::parse(url).map_err(|err| {
        JrError::UserError(format!(
            "--url is not a valid URL: {err}. Expected something like https://example.com/path."
        ))
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(JrError::UserError(format!(
            "--url must use http or https (got {}).",
            parsed.scheme()
        ))
        .into());
    }
    // Use the normalized form so the API request, stdout JSON, and the table
    // success line all agree. The raw `url: &str` may contain quirks the url
    // crate silently normalized away (e.g. tabs/newlines stripped from path).
    let url = parsed.as_str();

    // Default the title to the URL for script-friendly single-flag use.
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| url.to_string());

    let response = client.create_remote_link(&key, url, &title).await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json_output::remote_link_response(
                    &key,
                    response.id,
                    url,
                    &title,
                    &response.self_url,
                ))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Linked {} → {} (id: {})", key, url, response.id));
        }
    }

    Ok(())
}
