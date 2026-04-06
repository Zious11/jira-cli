use super::json_output;
use anyhow::{Result, bail};

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};

use super::helpers;

// ── Move (Transition) ────────────────────────────────────────────────

pub(super) async fn handle_move(
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

    // Idempotent: if already in target status, exit 0.
    // Check both direct match and whether the input is a transition name whose
    // target status matches the current status.
    let current_lower = current_status.to_lowercase();
    let target_lower = target_status.to_lowercase();
    let already_in_target = current_lower == target_lower
        || transitions.iter().any(|t| {
            t.name.to_lowercase() == target_lower
                && t.to
                    .as_ref()
                    .is_some_and(|s| s.name.to_lowercase() == current_lower)
        });
    if already_in_target {
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json_output::move_response(
                        &key,
                        &current_status,
                        false,
                    ))?
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
        // Build unified candidate pool: transition names + target status names.
        // Each candidate maps to its transition index.
        let mut candidates: Vec<(String, usize)> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (i, t) in transitions.iter().enumerate() {
            let t_lower = t.name.to_lowercase();
            if seen.insert(t_lower) {
                candidates.push((t.name.clone(), i));
            }
            if let Some(ref status) = t.to {
                let s_lower = status.name.to_lowercase();
                if seen.insert(s_lower) {
                    candidates.push((status.name.clone(), i));
                }
            }
        }

        let candidate_names: Vec<String> =
            candidates.iter().map(|(name, _)| name.clone()).collect();
        match partial_match::partial_match(&target_status, &candidate_names) {
            MatchResult::Exact(name) => {
                let idx = candidates
                    .iter()
                    .find(|(n, _)| n == &name)
                    .map(|(_, i)| *i)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Internal error: matched candidate \"{}\" not found. Please report this as a bug.",
                            name
                        )
                    })?;
                &transitions[idx]
            }
            // Case-insensitive dedup upstream; treat like Exact if case-variant duplicates slip through
            MatchResult::ExactMultiple(name) => {
                let idx = candidates
                    .iter()
                    .find(|(n, _)| n == &name)
                    .map(|(_, i)| *i)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Internal error: matched candidate \"{}\" not found. Please report this as a bug.",
                            name
                        )
                    })?;
                &transitions[idx]
            }
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
                    .map_err(|_| JrError::UserError("Invalid selection".into()))?;
                if idx < 1 || idx > matches.len() {
                    return Err(JrError::UserError("Selection out of range".into()).into());
                }
                let selected_name = &matches[idx - 1];
                let tidx = candidates
                    .iter()
                    .find(|(n, _)| n == selected_name)
                    .map(|(_, i)| *i)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Internal error: selected candidate \"{}\" not found. Please report this as a bug.",
                            selected_name
                        )
                    })?;
                &transitions[tidx]
            }
            MatchResult::None(_) => {
                let labels: Vec<String> = transitions
                    .iter()
                    .map(|t| match t.to.as_ref() {
                        Some(status) => format!("{} (→ {})", t.name, status.name),
                        None => t.name.clone(),
                    })
                    .collect();
                bail!(
                    "No transition matching \"{}\". Available: {}",
                    target_status,
                    labels.join(", ")
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
                serde_json::to_string_pretty(&json_output::move_response(&key, new_status, true,))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Moved {} to \"{}\"", key, new_status));
        }
    }

    Ok(())
}

// ── Transitions ───────────────────────────────────────────────────────

pub(super) async fn handle_transitions(
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

pub(super) async fn handle_assign(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Assign {
        key,
        to,
        account_id,
        unassign,
    } = command
    else {
        unreachable!()
    };

    if unassign {
        // Idempotent: check if already unassigned
        let issue = client.get_issue(&key, &[]).await?;
        if issue.fields.assignee.is_none() {
            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json_output::unassign_response(&key, false))?
                    );
                }
                OutputFormat::Table => {
                    output::print_success(&format!("{} is already unassigned", key));
                }
            }
            return Ok(());
        }

        client.assign_issue(&key, None).await?;
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json_output::unassign_response(&key, true))?
                );
            }
            OutputFormat::Table => {
                output::print_success(&format!("Unassigned {}", key));
            }
        }
        return Ok(());
    }

    // Resolve account ID and display name.
    // When --account-id is provided, no search is performed so the raw
    // account ID is used as the display name (no name available).
    let (account_id, display_name) = if let Some(ref id) = account_id {
        (id.clone(), id.clone())
    } else if let Some(ref user_query) = to {
        helpers::resolve_assignee(client, user_query, &key, no_input).await?
    } else {
        let me = client.get_myself().await?;
        (me.account_id, me.display_name)
    };

    // Idempotent: check if already assigned to target user
    let issue = client.get_issue(&key, &[]).await?;
    if let Some(ref assignee) = issue.fields.assignee {
        if assignee.account_id == account_id {
            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json_output::assign_unchanged_response(
                            &key,
                            &display_name,
                            &account_id,
                        ),)?
                    );
                }
                OutputFormat::Table => {
                    output::print_success(&format!(
                        "{} is already assigned to {}",
                        key, display_name
                    ));
                }
            }
            return Ok(());
        }
    }

    client.assign_issue(&key, Some(&account_id)).await?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json_output::assign_changed_response(
                    &key,
                    &display_name,
                    &account_id,
                ))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Assigned {} to {}", key, display_name));
        }
    }

    Ok(())
}

// ── Comment ───────────────────────────────────────────────────────────

pub(super) async fn handle_comment(
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

    let comment = client.add_comment(&key, adf_body, false).await?;

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

pub(super) async fn handle_open(command: IssueCommand, client: &JiraClient) -> Result<()> {
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
