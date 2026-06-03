use super::json_output;
use anyhow::{Result, bail};

use crate::adf;
use crate::api::client::JiraClient;
use crate::api::jira::bulk::{BULK_MAX_KEYS, resolve_bulk_await_timeout};
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
    let profile = client.profile_name();
    if !refresh {
        if let Some(c) = crate::cache::read_resolutions_cache(profile)? {
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
    crate::cache::write_resolutions_cache(profile, &cacheable)?;
    Ok(fetched)
}

// ── Pure helpers ─────────────────────────────────────────────────────

/// Resolve a dialoguer selection index to `Option<String>`.
///
/// Returns `None` when the selected item equals `none_sentinel` — signalling
/// "proceed without a resolution" (BC-3.2.013 OPTIONAL interactive branch).
/// Returns `Some(name)` for any other selection.
///
/// This is a pure function extracted for unit-testability (O-5 fix). The
/// security-relevant "(none)" decision is mutation-coverable without a TTY.
///
/// # Parameters
/// - `options`: the full prompt list (including the sentinel as the last item).
/// - `none_sentinel`: the sentinel string that signals "no resolution".
/// - `selection`: the index returned by `dialoguer::Select`.
pub fn resolve_interactive_choice(
    options: &[String],
    none_sentinel: &str,
    selection: usize,
) -> Option<String> {
    options
        .get(selection)
        .filter(|name| name.as_str() != none_sentinel)
        .cloned()
}

/// Build a resolution prompt list for `dialoguer::Select`.
///
/// When `allow_none` is `false` (REQUIRED branch): returns `base` unchanged —
/// the user must pick one of the listed resolutions; no opt-out is offered.
///
/// When `allow_none` is `true` (OPTIONAL branch): appends `NONE_LABEL` as the
/// final entry — the user may opt out of setting a resolution.
///
/// This is a pure function extracted for unit-testability (E-F2 fix). The
/// REQUIRED-vs-OPTIONAL "(none)" sentinel inclusion is the security-relevant
/// distinction that determines whether the POST body carries a resolution field.
/// Both the inline interactive branches and unit tests call this function so that
/// mutating `allow_none` is caught by tests without requiring a TTY.
pub fn build_resolution_prompt(base: &[String], allow_none: bool) -> Vec<String> {
    if allow_none {
        let mut v = base.to_vec();
        v.push(NONE_LABEL.to_string());
        v
    } else {
        base.to_vec()
    }
}

/// Sentinel label for the "no resolution" option in OPTIONAL interactive prompts.
///
/// Defined at module level so `build_resolution_prompt`, `resolve_interactive_choice`,
/// and the interactive branches all share the same constant without duplication.
pub const NONE_LABEL: &str = "(none — no resolution)";

/// Decide whether to refuse an interactive prompt because the caller is non-interactive.
///
/// Returns `true` (refuse — emit exit 64) when either the `--no-input` flag was set
/// OR stdin is not a TTY.  Returns `false` (allow — show the prompt) only when both
/// conditions are clear.
///
/// Extracted as a pure function so tests can vary each operand independently without
/// needing a real TTY, killing `||`→`&&`, `!`-deletion, and operand-swap mutants.
///
/// # Parameters
/// - `no_input`: value of the `--no-input` CLI flag for this invocation.
/// - `stdin_is_tty`: result of `std::io::IsTerminal::is_terminal(&std::io::stdin())`
///   evaluated once in the caller and passed in to avoid re-checking in each branch.
pub fn refuse_noninteractive(no_input: bool, stdin_is_tty: bool) -> bool {
    no_input || !stdin_is_tty
}

/// Choose the base resolution name list for the interactive prompt.
///
/// When `allowed_from_transition` is non-empty (the transition's `fields.resolution.
/// allowedValues` list), those names are used — they are already scoped to this
/// transition and avoid a network call.  When empty (allowedValues absent or empty),
/// falls back to the instance-global `instance_list` obtained from `load_resolutions`.
///
/// Extracted as a pure function to kill the `!`-deletion mutant on the emptiness check
/// without requiring a live Jira API or a dialoguer prompt in the test.
///
/// # Parameters
/// - `allowed_from_transition`: names from the transition's `allowedValues`; may be empty.
/// - `instance_list`: fallback list from `load_resolutions(client, false)`.
pub fn select_prompt_base_names<'a>(
    allowed_from_transition: &'a [String],
    instance_list: &'a [String],
) -> &'a [String] {
    if !allowed_from_transition.is_empty() {
        allowed_from_transition
    } else {
        instance_list
    }
}

/// Return the default-selected index for the OPTIONAL resolution prompt.
///
/// The OPTIONAL prompt places `"(none — no resolution)"` last so it is the default
/// selection (i.e. `len - 1`).  Using `saturating_sub` means an unexpectedly empty list
/// yields 0 rather than panicking.
///
/// Extracted as a pure function to kill `-`→`+` and `-`→`*` index mutants without
/// a live dialoguer interaction.
///
/// # Parameters
/// - `len`: length of the full prompt list (base names + NONE_LABEL sentinel).
pub fn optional_prompt_default_index(len: usize) -> usize {
    len.saturating_sub(1)
}

/// Fire the actual transition POST and render success output.
///
/// Extracted to avoid duplicating the POST + error handling + output block
/// across REQUIRED interactive, OPTIONAL interactive, and F-2 allowedValues
/// branches. BC-3.2.009 reactive handler lives here as the downstream backstop.
async fn finish_transition(
    client: &JiraClient,
    key: &str,
    selected_transition: &crate::types::jira::Transition,
    resolution_fields: Option<&serde_json::Value>,
    output_format: &OutputFormat,
) -> Result<()> {
    let transition_result = client
        .transition_issue(key, &selected_transition.id, resolution_fields)
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
                serde_json::to_string_pretty(&json_output::move_response(key, new_status, true))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Moved {} to \"{}\"", key, new_status));
        }
    }
    Ok(())
}

// ── Move (Transition) ────────────────────────────────────────────────

pub(super) async fn handle_move(
    command: IssueCommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Move {
        keys,
        to,
        resolution,
        no_resolution,
    } = command
    else {
        unreachable!()
    };
    // no_resolution is consumed by the enforcement gate below (single-key path).
    // The variable is valid from here; we move it into the gate block rather than
    // suppressing with `let _ =`.

    // --- Resolve (key, status) from the new Vec<String> + --to design ---
    //
    // Single-key legacy form: jr issue move KEY STATUS  → keys=[KEY,STATUS], to=None
    // Multi-key form:         jr issue move K1 K2 K3 --to STATUS → keys=[K1,K2,K3], to=Some("STATUS")
    // Error: jr issue move KEY --no-status (keys.len()==1, to=None) → prompt or error
    let (move_keys, target_status_input) = if let Some(ref t) = to {
        // --to flag provided: all positionals are keys.
        (keys.clone(), t.clone())
    } else if keys.len() >= 2 {
        // Legacy form: last positional is the status.
        let mut ks = keys.clone();
        let status = ks.pop().expect("checked len >= 2");
        (ks, status)
    } else {
        // Single key, no --to, no status positional.
        if no_input {
            bail!("Target status is required in non-interactive mode.");
        }
        // keys.len() == 1 here (clap requires at least 1).
        let key = &keys[0];
        let transitions_resp = client.get_transitions(key).await?;
        let transitions = &transitions_resp.transitions;
        if transitions.is_empty() {
            bail!("No transitions available for {key}.");
        }
        eprintln!("Available transitions for {}:", key);
        for (i, t) in transitions.iter().enumerate() {
            let to_name =
                t.to.as_ref()
                    .map(|s| s.name.as_str())
                    .unwrap_or("(unknown)");
            eprintln!("  {}. {} -> {}", i + 1, t.name, to_name);
        }
        let status = helpers::prompt_input("Select transition (name or number)")?;
        (keys.clone(), status)
    };

    // --- Validate key count ---
    if move_keys.len() > BULK_MAX_KEYS {
        return Err(JrError::UserError(format!(
            "Too many issue keys: {} provided, maximum is {}. \
             Split into batches of {} or fewer and run multiple times.",
            move_keys.len(),
            BULK_MAX_KEYS,
            BULK_MAX_KEYS,
        ))
        .into());
    }

    // --- Route: multi-key → bulk transition; single-key → existing per-issue path ---
    if move_keys.len() > 1 {
        return handle_move_bulk(&move_keys, &target_status_input, output_format, client).await;
    }

    // --- Single-key path ---
    let key = &move_keys[0];

    // Get available transitions with field metadata expanded so the enforcement gate
    // (BC-3.2.013) can inspect statusCategory and resolution field presence.
    // `handle_transitions` (the `jr issue transitions` read command) continues to call
    // `get_transitions` — these are DISTINCT methods per ADR-0015 §4.
    let transitions_resp = client.get_transitions_with_fields(key).await?;
    let transitions = &transitions_resp.transitions;

    if transitions.is_empty() {
        bail!("No transitions available for {key}.");
    }

    // Check current status first
    let issue = client.get_issue(key, &[]).await?;
    let current_status = issue
        .fields
        .status
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_default();

    let target_status = target_status_input;

    // Idempotent: if already in target status, exit 0.
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
                        key,
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

    // ── BC-3.2.013 Proactive resolution enforcement gate ─────────────────────
    //
    // Applies ONLY on the single-key path (bulk excluded per EC-3.2.013-8 / ADR-0015 §6).
    // Idempotency check (above) fires first — a no-op move never prompts.
    //
    // Gate condition:
    //   is_done_category  = to.statusCategory.key == "done"
    //   offers_resolution = fields map contains key "resolution"
    //   is_conditional    = is_conditional == Some(true)
    //   needs_resolution  = is_done_category && (offers_resolution || is_conditional)
    //
    // Conservative: if statusCategory is absent, gate does NOT fire.
    // If done-cat but fields absent AND not conditional, gate does NOT fire.
    {
        let is_done_category = selected_transition
            .to
            .as_ref()
            .and_then(|s| s.status_category.as_ref())
            .is_some_and(|sc| sc.key == "done");

        let offers_resolution = selected_transition
            .fields
            .as_ref()
            .is_some_and(|f| f.contains_key("resolution"));

        let is_conditional = selected_transition.is_conditional == Some(true);

        let needs_resolution = is_done_category && (offers_resolution || is_conditional);

        // Extract the transition's allowedValues for the resolution field (may be absent).
        // Used by both the --resolution validation branch and the interactive prompt.
        let allowed_names_from_transition: Vec<String> = selected_transition
            .fields
            .as_ref()
            .and_then(|f| f.get("resolution"))
            .and_then(|r| r.get("allowedValues"))
            .and_then(|av| av.as_array())
            .filter(|arr| !arr.is_empty())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        if needs_resolution {
            // ── Branch A: --resolution <name> IS provided — validate against allowedValues.
            // (EC-3.2.013-3, F-2 fix)
            if let Some(ref query) = resolution {
                if !allowed_names_from_transition.is_empty() {
                    // Validate case-insensitively against the transition's allowedValues.
                    // If the value is not found → exit 64 listing the allowed values.
                    // If allowedValues absent/empty → skip validation here; fall through
                    // to the existing instance-global resolve_resolution_by_name below.
                    //
                    // NOTE: we send {resolution:{name}} (not {resolution:{id}}) because
                    // the Atlassian API accepts name-based resolution on transitions and
                    // id-based resolution is left as a future hardening opportunity.
                    let to_label = selected_transition
                        .to
                        .as_ref()
                        .map(|s| s.name.as_str())
                        .unwrap_or(&selected_transition.name);
                    let matched = allowed_names_from_transition
                        .iter()
                        .find(|n| n.eq_ignore_ascii_case(query));
                    let chosen_name = match matched {
                        Some(n) => n.clone(),
                        None => {
                            return Err(JrError::UserError(format!(
                                "Resolution '{}' is not allowed on the '{}' transition. \
                                 Allowed: {}.\n\n\
                                 Run `jr issue resolutions` to see all instance resolutions.",
                                query,
                                to_label,
                                allowed_names_from_transition.join(", ")
                            ))
                            .into());
                        }
                    };
                    // Validated — send the resolution atomically in the transition body.
                    let resolution_fields_value = serde_json::json!({
                        "resolution": { "name": chosen_name }
                    });
                    finish_transition(
                        client,
                        key,
                        selected_transition,
                        Some(&resolution_fields_value),
                        output_format,
                    )
                    .await?;
                    return Ok(());
                }
                // allowedValues absent/empty — fall through to the regular
                // `resolve_resolution_by_name` path after the gate block.
            } else {
                // ── Branch B: resolution NOT provided — enforce / prompt.
                let resolution_required_flag = selected_transition
                    .fields
                    .as_ref()
                    .and_then(|f| f.get("resolution"))
                    .and_then(|r| r.get("required"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let required = resolution_required_flag || is_conditional;

                let to_label = selected_transition
                    .to
                    .as_ref()
                    .map(|s| s.name.as_str())
                    .unwrap_or(&selected_transition.name);

                // Evaluate TTY status once so refuse_noninteractive can be called
                // with independent operands in both branches (kills ||→&& mutant).
                let stdin_is_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());

                if required {
                    // ── REQUIRED branch ─────────────────────────────────────────
                    if no_resolution {
                        return Err(JrError::UserError(format!(
                            "The transition to \"{to_label}\" requires a resolution and \
                             --no-resolution cannot be used here.\n\n\
                             Try:\n    jr issue move {key} {to_label} --resolution <name>\n\n\
                             Run `jr issue resolutions` to see available values."
                        ))
                        .into());
                    }
                    if refuse_noninteractive(no_input, stdin_is_tty) {
                        return Err(JrError::UserError(format!(
                            "The transition to \"{to_label}\" requires a resolution.\n\n\
                             Try:\n    jr issue move {key} {to_label} --resolution <name>\n\n\
                             Run `jr issue resolutions` to see available values."
                        ))
                        .into());
                    }
                    // Interactive REQUIRED: prompt with no "(none)" option (allow_none=false).
                    let instance_resolutions = load_resolutions(client, false).await?;
                    let instance_names: Vec<String> = instance_resolutions
                        .iter()
                        .map(|r| r.name.clone())
                        .collect();
                    let base_names =
                        select_prompt_base_names(&allowed_names_from_transition, &instance_names);
                    let prompt_names = build_resolution_prompt(base_names, false);
                    // OBS-1: guard against empty prompt list (no resolutions on instance).
                    if prompt_names.is_empty() {
                        return Err(JrError::UserError(format!(
                            "No resolutions available on this instance; cannot satisfy \
                             the required resolution for transition \"{to_label}\".\n\n\
                             Contact your Jira administrator to configure resolution values."
                        ))
                        .into());
                    }
                    let selection = dialoguer::Select::new()
                        .with_prompt("Select a resolution (required)")
                        .items(&prompt_names)
                        .default(0)
                        .interact()
                        .map_err(|_| JrError::Interrupted)?;
                    let chosen_name = prompt_names[selection].clone();
                    let resolution_fields_value = serde_json::json!({
                        "resolution": { "name": chosen_name }
                    });
                    finish_transition(
                        client,
                        key,
                        selected_transition,
                        Some(&resolution_fields_value),
                        output_format,
                    )
                    .await?;
                    return Ok(());
                } else {
                    // ── OPTIONAL branch ─────────────────────────────────────────
                    if no_resolution {
                        // Explicit opt-out: proceed without resolution (fall through).
                        // The POST fires after the gate with no resolution fields.
                    } else if refuse_noninteractive(no_input, stdin_is_tty) {
                        return Err(JrError::UserError(format!(
                            "The transition to \"{to_label}\" offers a resolution field. \
                             You must explicitly choose:\n\n\
                             --resolution <name>   set a resolution\n\
                             --no-resolution       opt out (intentional null-resolution close)\n\n\
                             Run `jr issue resolutions` to see available values."
                        ))
                        .into());
                    } else {
                        // Interactive OPTIONAL: prompt with a "(none)" sentinel.
                        let instance_resolutions = load_resolutions(client, false).await?;
                        let instance_names: Vec<String> = instance_resolutions
                            .iter()
                            .map(|r| r.name.clone())
                            .collect();
                        let base_names = select_prompt_base_names(
                            &allowed_names_from_transition,
                            &instance_names,
                        );
                        let prompt_names = build_resolution_prompt(base_names, true);
                        let default_idx = optional_prompt_default_index(prompt_names.len());
                        let selection = dialoguer::Select::new()
                            .with_prompt("Select a resolution (optional)")
                            .items(&prompt_names)
                            .default(default_idx)
                            .interact()
                            .map_err(|_| JrError::Interrupted)?;
                        let chosen =
                            resolve_interactive_choice(&prompt_names, NONE_LABEL, selection);
                        let resolution_value = chosen
                            .map(|name| serde_json::json!({ "resolution": { "name": name } }));
                        finish_transition(
                            client,
                            key,
                            selected_transition,
                            resolution_value.as_ref(),
                            output_format,
                        )
                        .await?;
                        return Ok(());
                    }
                }
            }
        }
    }
    // ── End BC-3.2.013 enforcement gate ──────────────────────────────────────

    // Resolve --resolution if provided (non-gate path: non-done-cat transitions,
    // or done-cat with no resolution field, or done-cat with allowedValues absent
    // falling through from Gate Branch A).
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

    finish_transition(
        client,
        key,
        selected_transition,
        resolution_fields.as_ref(),
        output_format,
    )
    .await
}

/// Multi-key bulk transition handler.
///
/// Looks up transitions for the first key to resolve the status name → transitionId,
/// then fires a single POST /rest/api/3/bulk/issues/transition call.
/// Polls until terminal status, renders per-key results.
async fn handle_move_bulk(
    keys: &[String],
    target_status: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    // Discover transition ID from first key.
    let first_key = &keys[0];
    let transitions_resp = client.get_transitions(first_key).await?;
    let transitions = &transitions_resp.transitions;

    if transitions.is_empty() {
        bail!("No transitions available for {first_key} (used to discover transition ID).");
    }

    // Match target_status by number or name (same logic as single-key path).
    let selected_id: String = if let Ok(num) = target_status.parse::<usize>() {
        if num >= 1 && num <= transitions.len() {
            transitions[num - 1].id.clone()
        } else {
            bail!(
                "Transition number {num} out of range (1..={}).",
                transitions.len()
            );
        }
    } else {
        // Name-based match using same candidate pool strategy.
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
        match partial_match::partial_match(target_status, &candidate_names) {
            MatchResult::Exact(name) | MatchResult::ExactMultiple(name) => {
                let idx = candidates
                    .iter()
                    .find(|(n, _)| n == &name)
                    .map(|(_, i)| *i)
                    .ok_or_else(|| {
                        JrError::Internal(format!(
                            "Internal error: matched transition \"{}\" not found.",
                            name
                        ))
                    })?;
                transitions[idx].id.clone()
            }
            MatchResult::Ambiguous(matches) => {
                return Err(JrError::UserError(format!(
                    "Ambiguous transition \"{target_status}\". Matches: {}. \
                     Use --to with an exact name.",
                    matches.join(", ")
                ))
                .into());
            }
            MatchResult::None(_) => {
                let labels: Vec<String> = transitions
                    .iter()
                    .map(|t| match t.to.as_ref() {
                        Some(s) => format!("{} (→ {})", t.name, s.name),
                        None => t.name.clone(),
                    })
                    .collect();
                bail!(
                    "No transition matching \"{target_status}\". Available: {}",
                    labels.join(", ")
                );
            }
        }
    };

    // Fire the bulk transition.
    let task_id = client.bulk_transition(keys, &selected_id).await?;

    // Poll with 5-minute timeout.
    let progress = client
        .await_bulk_task(&task_id, resolve_bulk_await_timeout())
        .await?;

    // Render results (similar to bulk edit).
    let processed: std::collections::HashSet<&str> = progress
        .processed_accessible_issues
        .iter()
        .map(String::as_str)
        .collect();

    let mut any_failed = false;
    let mut results: Vec<serde_json::Value> = Vec::new();

    for key in keys {
        if let Some(err) = progress.failed_accessible_issues.get(key.as_str()) {
            let summary = err.summary();
            results.push(serde_json::json!({
                "key": key,
                "status": "error",
                "error": summary,
            }));
            any_failed = true;
        } else if processed.contains(key.as_str()) {
            results.push(serde_json::json!({
                "key": key,
                "status": "success",
            }));
        } else {
            results.push(serde_json::json!({
                "key": key,
                "status": "inaccessible",
            }));
        }
    }

    match output_format {
        OutputFormat::Json => {
            let payload = serde_json::json!({
                "taskId": task_id,
                "results": results,
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        OutputFormat::Table => {
            for entry in &results {
                let key = entry["key"].as_str().unwrap_or("?");
                match entry["status"].as_str().unwrap_or("?") {
                    "success" => {
                        output::print_success(&format!("Moved {key} to \"{target_status}\""))
                    }
                    "error" => {
                        let err_msg = entry["error"].as_str().unwrap_or("unknown error");
                        eprintln!("error: {key}: {err_msg}");
                    }
                    status => eprintln!("warning: {key}: {status}"),
                }
            }
        }
    }

    if any_failed {
        bail!("One or more issues failed during bulk transition. See output above for details.");
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

    let url = format!("{}/browse/{}", client.instance_url(), key);

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
