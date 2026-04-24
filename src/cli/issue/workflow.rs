use super::json_output;
use anyhow::{Result, bail};

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};
use crate::types::jira::Resolution;

use super::helpers;

// ── Resolution resolver ───────────────────────────────────────────────

/// Resolve a user-supplied resolution name against a list of resolutions.
///
/// Matching strategy (via `partial_match`): case-insensitive exact wins.
/// Anything else — prefix, substring, multiple exact duplicates, or no
/// match — surfaces the candidate list via `JrError::UserError` (exit 64),
/// matching the spec (docs/specs/issue-move-resolution.md) and sibling
/// resolvers (`handle_move` status block, `handle_link` link-type block).
///
/// Notably, a single-substring hit is NOT silently promoted to success —
/// that would diverge from every other resolver in the codebase and
/// bypass the operator's intent to be explicit about which resolution to
/// apply. The caller is expected to propagate the error (no interactive
/// prompt for `--resolution`; the flag is purely explicit).
fn resolve_resolution_by_name(resolutions: &[Resolution], query: &str) -> Result<Resolution> {
    let names: Vec<String> = resolutions.iter().map(|r| r.name.clone()).collect();
    match partial_match::partial_match(query, &names) {
        MatchResult::Exact(name) => resolutions
            .iter()
            .find(|r| r.name == name)
            .cloned()
            .ok_or_else(|| {
                JrError::Internal(format!(
                    "Internal error: matched resolution \"{}\" not found. Please report this as a bug.",
                    name
                ))
                .into()
            }),
        // Multiple case-insensitive exact duplicates — list ONLY the
        // duplicate entries that actually collide with the query, so the
        // operator sees which conflicting values need cleanup (not the
        // whole instance-wide resolution list).
        MatchResult::ExactMultiple(_) => {
            // Include the id alongside each duplicate name so the operator
            // can tell two same-named entries apart in Jira admin and pick
            // which one to delete / rename.
            let duplicates: Vec<String> = resolutions
                .iter()
                .filter(|r| r.name.eq_ignore_ascii_case(query))
                .map(|r| match r.id.as_deref() {
                    Some(id) => format!("{} (id={})", r.name, id),
                    None => r.name.clone(),
                })
                .collect();
            Err(JrError::UserError(format!(
                "Multiple resolutions named \"{}\" exist: {}",
                query,
                duplicates.join(", ")
            ))
            .into())
        }
        // Ambiguous always errors — including single-substring hits. Project
        // convention is that only case-insensitive EXACT matches auto-resolve.
        MatchResult::Ambiguous(matches) => Err(JrError::UserError(format!(
            "Ambiguous resolution \"{}\". Matches: {}",
            query,
            matches.join(", ")
        ))
        .into()),
        MatchResult::None(all) => Err(JrError::UserError(format!(
            "No resolution matching \"{}\". Available: {}",
            query,
            all.join(", ")
        ))
        .into()),
    }
}

// ── Resolutions loader ───────────────────────────────────────────────

/// Load the instance-global list of resolutions, honouring the 7-day cache.
///
/// When `refresh` is false (the common read-through path), a cache hit is
/// converted directly to `Vec<Resolution>`. A miss falls through to the
/// refresh path so the cache is warmed for the next caller.
///
/// When `refresh` is true (explicit bypass), the cache is ignored on read
/// but still written through so subsequent reads see the fresh data.
///
/// Entries returned from the API without an id are dropped on write —
/// the cache's `CachedResolution` type has a non-optional id field so an
/// id-less resolution cannot be persisted. `GET /rest/api/3/resolution`
/// always returns an id in practice; this is a defensive fallback that
/// warns on stderr rather than silently dropping so a partial Atlassian
/// response is visible.
async fn load_resolutions(client: &JiraClient, refresh: bool) -> Result<Vec<Resolution>> {
    if !refresh {
        if let Some(c) = crate::cache::read_resolutions_cache()? {
            return Ok(c
                .resolutions
                .into_iter()
                .map(|r| Resolution {
                    id: Some(r.id),
                    name: r.name,
                    description: r.description,
                })
                .collect());
        }
    }

    let fetched = client.get_resolutions().await?;
    let before = fetched.len();
    let cacheable: Vec<crate::cache::CachedResolution> = fetched
        .iter()
        .filter_map(|r| {
            r.id.as_ref().map(|id| crate::cache::CachedResolution {
                id: id.clone(),
                name: r.name.clone(),
                description: r.description.clone(),
            })
        })
        .collect();
    if cacheable.len() != before {
        eprintln!(
            "warning: {} resolution(s) lacked an id and were not cached",
            before - cacheable.len()
        );
    }
    crate::cache::write_resolutions_cache(&cacheable)?;
    Ok(fetched)
}

// ── Move (Transition) ────────────────────────────────────────────────

pub(super) async fn handle_move(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Move {
        key,
        status,
        resolution,
    } = command
    else {
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
                        JrError::Internal(format!(
                            "Internal error: matched candidate \"{}\" not found. Please report this as a bug.",
                            name
                        ))
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
                        JrError::Internal(format!(
                            "Internal error: matched candidate \"{}\" not found. Please report this as a bug.",
                            name
                        ))
                    })?;
                &transitions[idx]
            }
            MatchResult::Ambiguous(matches) => {
                if no_input {
                    return Err(JrError::UserError(format!(
                        "Ambiguous transition \"{}\". Matches: {}",
                        target_status,
                        matches.join(", ")
                    ))
                    .into());
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
                        JrError::Internal(format!(
                            "Internal error: selected candidate \"{}\" not found. Please report this as a bug.",
                            selected_name
                        ))
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

    // Resolve --resolution against the cached resolutions list if provided.
    let resolution_fields: Option<serde_json::Value> = match resolution.as_deref() {
        None => None,
        Some(query) => {
            let resolutions = load_resolutions(client, false).await?;
            let matched = resolve_resolution_by_name(&resolutions, query)?;
            Some(serde_json::json!({
                "resolution": { "name": matched.name }
            }))
        }
    };

    // Transform Atlassian's "Field 'resolution' is required" 400 into an
    // actionable hint pointing at `--resolution` and `jr issue resolutions`.
    // Heuristic: lowercased error body contains both "resolution" and
    // "required". Other 400s pass through unchanged.
    let transition_result = client
        .transition_issue(&key, &selected_transition.id, resolution_fields.as_ref())
        .await;

    if let Err(err) = transition_result {
        let msg = format!("{err:#}").to_lowercase();
        if msg.contains("resolution") && msg.contains("required") {
            let to_label = selected_transition
                .to
                .as_ref()
                .map(|s| s.name.as_str())
                .unwrap_or(&selected_transition.name);
            return Err(JrError::UserError(format!(
                "The \"{to_label}\" transition requires a resolution.\n\n\
                 Try:\n    jr issue move {key} {to_label} --resolution <name>\n\n\
                 Run `jr issue resolutions` to see available values."
            ))
            .into());
        }
        return Err(err);
    }

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

// ── Resolutions ───────────────────────────────────────────────────────

pub(super) async fn handle_resolutions(
    refresh: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let resolutions = load_resolutions(client, refresh).await?;

    let rows: Vec<Vec<String>> = resolutions
        .iter()
        .map(|r| vec![r.name.clone(), r.description.clone().unwrap_or_default()])
        .collect();

    output::print_output(output_format, &["Name", "Description"], &rows, &resolutions)?;

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
        internal,
    } = command
    else {
        unreachable!()
    };

    // Resolve comment text from the various sources. spawn_blocking isolates
    // the blocking stdin read from the tokio runtime.
    let text = if stdin {
        tokio::task::spawn_blocking(|| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            Ok::<_, std::io::Error>(buf)
        })
        .await??
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

    let comment = client.add_comment(&key, adf_body, internal).await?;

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

#[cfg(test)]
mod resolution_resolver_tests {
    use super::*;
    use crate::types::jira::Resolution;

    fn sample_resolutions() -> Vec<Resolution> {
        vec![
            Resolution {
                id: Some("10000".into()),
                name: "Done".into(),
                description: None,
            },
            Resolution {
                id: Some("10001".into()),
                name: "Won't Do".into(),
                description: None,
            },
            Resolution {
                id: Some("10002".into()),
                name: "Duplicate".into(),
                description: None,
            },
            Resolution {
                id: Some("10003".into()),
                name: "Cannot Reproduce".into(),
                description: None,
            },
        ]
    }

    #[test]
    fn resolve_resolution_exact_match_returns_it() {
        let r = resolve_resolution_by_name(&sample_resolutions(), "Done").unwrap();
        assert_eq!(r.name, "Done");
    }

    #[test]
    fn resolve_resolution_case_insensitive_exact() {
        let r = resolve_resolution_by_name(&sample_resolutions(), "done").unwrap();
        assert_eq!(r.name, "Done");
    }

    #[test]
    fn resolve_resolution_unique_substring_errors_as_ambiguous() {
        // "Dup" uniquely matches Duplicate (prefix/substring), but per
        // project convention only case-insensitive EXACT matches
        // auto-resolve. A unique non-exact hit still errors so the caller
        // is explicit about which resolution they want.
        let err = resolve_resolution_by_name(&sample_resolutions(), "Dup").unwrap_err();
        let jr_err = err
            .downcast_ref::<crate::error::JrError>()
            .expect("expected JrError wrapper");
        assert!(
            matches!(jr_err, crate::error::JrError::UserError(_)),
            "expected UserError variant, got: {jr_err:?}"
        );
        let root = err.root_cause().to_string().to_lowercase();
        assert!(
            root.contains("ambiguous"),
            "expected ambiguous error, got: {err:?}"
        );
        assert!(
            root.contains("duplicate"),
            "error should list the matching candidate, got: {err:?}"
        );
    }

    #[test]
    fn resolve_resolution_ambiguous_substring_errors_with_exit_64() {
        // "o" matches Done, Won't Do, Cannot Reproduce — disambiguation required.
        let err = resolve_resolution_by_name(&sample_resolutions(), "o").unwrap_err();
        let root = err.root_cause().to_string().to_lowercase();
        assert!(
            root.contains("ambiguous") || root.contains("multiple"),
            "expected ambiguous error, got: {err:?}"
        );
        // Exit code 64 comes from JrError::UserError — verify by downcasting.
        // Use .expect() so a regression that drops the JrError wrapper fails
        // the test instead of silently skipping the inner assertion.
        let jr_err = err
            .downcast_ref::<crate::error::JrError>()
            .expect("expected JrError wrapper");
        assert!(
            matches!(jr_err, crate::error::JrError::UserError(_)),
            "expected UserError variant, got: {jr_err:?}"
        );
    }

    #[test]
    fn resolve_resolution_no_match_errors_with_candidates() {
        let err = resolve_resolution_by_name(&sample_resolutions(), "nonexistent").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("Done"), "error should list candidates: {msg}");
        assert!(
            msg.contains("Duplicate"),
            "error should list candidates: {msg}"
        );
    }

    /// When an instance has two resolutions with the same name (different ids,
    /// same display label) the error must list ONLY the colliding entries, not
    /// every resolution on the instance. Otherwise operators can't tell which
    /// records to clean up.
    #[test]
    fn resolve_resolution_exact_multiple_lists_only_duplicates() {
        let resolutions = vec![
            Resolution {
                id: Some("10000".into()),
                name: "Done".into(),
                description: None,
            },
            Resolution {
                id: Some("10100".into()),
                name: "done".into(), // case-insensitive duplicate of "Done"
                description: None,
            },
            Resolution {
                id: Some("10001".into()),
                name: "Won't Do".into(),
                description: None,
            },
        ];

        let err = resolve_resolution_by_name(&resolutions, "Done").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("Done") && msg.contains("done"),
            "error should list both duplicates: {msg}"
        );
        // Ids disambiguate same-name entries so the operator can fix the
        // correct one in Jira admin.
        assert!(
            msg.contains("id=10000") && msg.contains("id=10100"),
            "error should include ids to disambiguate same-name entries: {msg}"
        );
        assert!(
            !msg.contains("Won't Do"),
            "error must NOT list non-duplicate entries, but did: {msg}"
        );
    }
}
