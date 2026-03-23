mod create;
mod format;
mod helpers;
mod list;

pub use format::{format_issue_row, format_issue_rows_public, format_points, issue_table_headers};

use anyhow::{Result, bail};
use serde_json::json;

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::output;
use crate::partial_match::{self, MatchResult};

/// Handle all issue subcommands.
pub async fn handle(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    match command {
        IssueCommand::List { .. } => {
            list::handle_list(
                command,
                output_format,
                config,
                client,
                project_override,
                no_input,
            )
            .await
        }
        IssueCommand::View { .. } => {
            list::handle_view(command, output_format, config, client).await
        }
        IssueCommand::Create { .. } => {
            create::handle_create(
                command,
                output_format,
                config,
                client,
                project_override,
                no_input,
            )
            .await
        }
        IssueCommand::Edit { .. } => {
            create::handle_edit(command, output_format, config, client, no_input).await
        }
        IssueCommand::Move { .. } => handle_move(command, output_format, client, no_input).await,
        IssueCommand::Transitions { .. } => {
            handle_transitions(command, output_format, client).await
        }
        IssueCommand::Assign { .. } => handle_assign(command, output_format, client).await,
        IssueCommand::Comment { .. } => handle_comment(command, output_format, client).await,
        IssueCommand::Open { .. } => handle_open(command, client).await,
        IssueCommand::Link { .. } => handle_link(command, output_format, client, no_input).await,
        IssueCommand::Unlink { .. } => {
            handle_unlink(command, output_format, client, no_input).await
        }
        IssueCommand::LinkTypes => handle_link_types(output_format, client).await,
    }
}

// ── Move (Transition) ────────────────────────────────────────────────

async fn handle_move(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Move { key, status } = command else {
        unreachable!()
    };

    // Get available transitions
    let transitions_resp = client.get_transitions(&key).await?;
    let transitions = &transitions_resp.transitions;

    if transitions.is_empty() {
        bail!("No transitions available for {key}.");
    }

    // Check current status first
    let issue = client.get_issue(&key, &[]).await?;
    let current_status = issue
        .fields
        .status
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_default();

    let target_status = match status {
        Some(s) => s,
        None => {
            if no_input {
                bail!("Target status is required in non-interactive mode.");
            }
            // Show transitions and prompt
            eprintln!("Available transitions for {}:", key);
            for (i, t) in transitions.iter().enumerate() {
                let to_name =
                    t.to.as_ref()
                        .map(|s| s.name.as_str())
                        .unwrap_or("(unknown)");
                eprintln!("  {}. {} -> {}", i + 1, t.name, to_name);
            }

            helpers::prompt_input("Select transition (name or number)")?
        }
    };

    // Idempotent: if already in target status, exit 0
    if current_status.to_lowercase() == target_status.to_lowercase() {
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "key": key,
                        "status": current_status,
                        "changed": false
                    }))?
                );
            }
            OutputFormat::Table => {
                output::print_success(&format!(
                    "{} is already in status \"{}\"",
                    key, current_status
                ));
            }
        }
        return Ok(());
    }

    // Try to match by number first
    let selected_transition = if let Ok(num) = target_status.parse::<usize>() {
        if num >= 1 && num <= transitions.len() {
            Some(&transitions[num - 1])
        } else {
            None
        }
    } else {
        None
    };

    let selected_transition = if let Some(t) = selected_transition {
        t
    } else {
        // Use partial matching on transition names
        let transition_names: Vec<String> = transitions.iter().map(|t| t.name.clone()).collect();
        match partial_match::partial_match(&target_status, &transition_names) {
            MatchResult::Exact(name) => transitions.iter().find(|t| t.name == name).unwrap(),
            MatchResult::Ambiguous(matches) => {
                if no_input {
                    bail!(
                        "Ambiguous transition \"{}\". Matches: {}",
                        target_status,
                        matches.join(", ")
                    );
                }
                // Interactive disambiguation
                eprintln!(
                    "Ambiguous match for \"{}\". Did you mean one of:",
                    target_status
                );
                for (i, m) in matches.iter().enumerate() {
                    eprintln!("  {}. {}", i + 1, m);
                }
                let choice = helpers::prompt_input("Select (number)")?;
                let idx: usize = choice
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid selection"))?;
                if idx < 1 || idx > matches.len() {
                    bail!("Selection out of range");
                }
                transitions
                    .iter()
                    .find(|t| t.name == matches[idx - 1])
                    .unwrap()
            }
            MatchResult::None(all) => {
                bail!(
                    "No transition matching \"{}\". Available: {}",
                    target_status,
                    all.join(", ")
                );
            }
        }
    };

    client
        .transition_issue(&key, &selected_transition.id)
        .await?;

    let new_status = selected_transition
        .to
        .as_ref()
        .map(|s| s.name.as_str())
        .unwrap_or(&selected_transition.name);

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "key": key,
                    "status": new_status,
                    "changed": true
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Moved {} to \"{}\"", key, new_status));
        }
    }

    Ok(())
}

// ── Transitions ───────────────────────────────────────────────────────

async fn handle_transitions(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Transitions { key } = command else {
        unreachable!()
    };

    let resp = client.get_transitions(&key).await?;

    let rows: Vec<Vec<String>> = resp
        .transitions
        .iter()
        .map(|t| {
            vec![
                t.id.clone(),
                t.name.clone(),
                t.to.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "Name", "To Status"],
        &rows,
        &resp.transitions,
    )?;

    Ok(())
}

// ── Assign ────────────────────────────────────────────────────────────

async fn handle_assign(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Assign { key, to, unassign } = command else {
        unreachable!()
    };

    if unassign {
        client.assign_issue(&key, None).await?;
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "key": key,
                        "assignee": null,
                        "changed": true
                    }))?
                );
            }
            OutputFormat::Table => {
                output::print_success(&format!("Unassigned {}", key));
            }
        }
        return Ok(());
    }

    let account_id = if let Some(ref user_query) = to {
        // Assign to another user — use the provided value as account ID
        user_query.clone()
    } else {
        // Assign to self
        let me = client.get_myself().await?;

        // Idempotent: check if already assigned to self
        let issue = client.get_issue(&key, &[]).await?;
        if let Some(ref assignee) = issue.fields.assignee {
            if assignee.account_id == me.account_id {
                match output_format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "key": key,
                                "assignee": me.display_name,
                                "changed": false
                            }))?
                        );
                    }
                    OutputFormat::Table => {
                        output::print_success(&format!("{} is already assigned to you", key));
                    }
                }
                return Ok(());
            }
        }

        me.account_id
    };

    client.assign_issue(&key, Some(&account_id)).await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "key": key,
                    "assignee": account_id,
                    "changed": true
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Assigned {} to {}", key, account_id));
        }
    }

    Ok(())
}

// ── Comment ───────────────────────────────────────────────────────────

async fn handle_comment(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::Comment {
        key,
        message,
        markdown,
        file,
        stdin,
    } = command
    else {
        unreachable!()
    };

    // Resolve comment text from the various sources
    let text = if stdin {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
        buf
    } else if let Some(ref path) = file {
        std::fs::read_to_string(path)?
    } else if let Some(ref msg) = message {
        msg.clone()
    } else {
        bail!("Comment text is required. Use a positional argument, --file, or --stdin.");
    };

    let text = text.trim().to_string();
    if text.is_empty() {
        bail!("Comment text cannot be empty.");
    }

    let adf_body = if markdown {
        adf::markdown_to_adf(&text)
    } else {
        adf::text_to_adf(&text)
    };

    let comment = client.add_comment(&key, adf_body).await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&comment)?);
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Added comment to {} (id: {})",
                key,
                comment.id.as_deref().unwrap_or("unknown")
            ));
        }
    }

    Ok(())
}

// ── Open ──────────────────────────────────────────────────────────────

async fn handle_open(command: IssueCommand, client: &JiraClient) -> Result<()> {
    let IssueCommand::Open { key, url_only } = command else {
        unreachable!()
    };

    let url = format!("{}/browse/{}", client.base_url(), key);

    if url_only {
        println!("{}", url);
    } else {
        open::that(&url)?;
        eprintln!("Opened {} in browser", key);
    }

    Ok(())
}

// ── Link Types ────────────────────────────────────────────────────

async fn handle_link_types(output_format: &OutputFormat, client: &JiraClient) -> Result<()> {
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

async fn handle_link(
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
        MatchResult::Ambiguous(matches) => {
            if no_input {
                bail!(
                    "Ambiguous link type \"{}\". Matches: {}",
                    link_type_name,
                    matches.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt(format!("Multiple types match \"{link_type_name}\""))
                .items(&matches)
                .interact()?;
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
                serde_json::to_string_pretty(&json!({
                    "key1": key1,
                    "key2": key2,
                    "type": resolved_name,
                    "linked": true
                }))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Linked {} → {} ({})", key1, key2, resolved_name));
        }
    }

    Ok(())
}

// ── Unlink ────────────────────────────────────────────────────────

async fn handle_unlink(
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
            MatchResult::Ambiguous(matches) => {
                if no_input {
                    bail!(
                        "Ambiguous link type \"{}\". Matches: {}",
                        type_name,
                        matches.join(", ")
                    );
                }
                let selection = dialoguer::Select::new()
                    .with_prompt(format!("Multiple types match \"{type_name}\""))
                    .items(&matches)
                    .interact()?;
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
                    serde_json::to_string_pretty(&json!({
                        "unlinked": false,
                        "count": 0
                    }))?
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
                serde_json::to_string_pretty(&json!({
                    "unlinked": true,
                    "count": count
                }))?
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
