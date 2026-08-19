use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde_json::json;

use crate::adf;
use crate::api::client::JiraClient;
use crate::api::jira::bulk::{
    BULK_MAX_KEYS, BulkMultiSelectFieldOption, build_component_edited_fields,
    resolve_bulk_await_timeout,
};
use crate::api::jira::issues::ComponentRef;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::partial_match::MatchResult;

use super::create::parse_field_kv;
use super::format;
use super::helpers;
use super::json_output;

/// Number of issues above which a `--jql`-driven bulk edit requires explicit
/// `--yes` (or `--no-input` implicit-yes) to proceed. Below this threshold the
/// command runs without prompting because the blast radius is small.
///
/// Set to 5 as a conservative default — many real bulk operations target 10-50
/// issues from a saved JQL filter, so users will hit this prompt routinely. If
/// product feedback indicates the threshold is too aggressive, raise to 25-50.
const JQL_CONFIRM_THRESHOLD: usize = 5;

/// Sanity ceiling on `--max`/the resolved `--jql` match-set size for the
/// `--component` bulk path only (Step-4.5 Round-1 F3 fix, BC-3.4.023
/// Postcondition 6). Mirrors the `num_args = 0..=10000` widening already
/// applied to the positional `keys` argument in `src/cli/mod.rs` for the
/// same reason: `--component`'s bulk path chunks internally into
/// `<=BULK_MAX_KEYS`-key POSTs, so it can safely accept a much larger
/// resolved key set than every other bulk field path, which issues a
/// single un-chunked POST and stays hard-capped at `BULK_MAX_KEYS`.
const JQL_MAX_CEILING: u32 = 10_000;

pub(super) async fn handle_edit(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Edit {
        keys,
        jql,
        max,
        yes,
        dry_run,
        summary,
        issue_type,
        priority,
        label: labels,
        component: components,
        team,
        points,
        no_points,
        parent,
        no_parent,
        description,
        description_stdin,
        markdown,
        field: field_raw,
    } = command
    else {
        unreachable!()
    };

    // Parse --field NAME=VALUE pairs into a HashMap (last-wins on duplicate keys).
    // Per EC-3.4.017-10: duplicate keys are collapsed here before resolve_edit_fields sees them.
    let field_pairs = parse_field_kv(&field_raw)?;

    // Validate: at least one selector must be present (keys or --jql).
    // clap doesn't enforce this natively since both are optional — we validate here.
    if keys.is_empty() && jql.is_none() {
        return Err(
            JrError::UserError("Specify at least one issue key or --jql <query>.".into()).into(),
        );
    }

    // Validate: --max is only meaningful with --jql.  clap's `requires` attribute cannot
    // enforce this when positional keys are also present (because `keys` and `jql` have
    // `conflicts_with` between them, which causes clap to skip the `requires` check).
    // We enforce it here instead, before any HTTP calls.
    if max.is_some() && jql.is_none() {
        return Err(JrError::UserError(
            "--max requires --jql. It cannot be used with positional keys because \
             it only limits the number of issues matched by a JQL query. \
             Remove --max or switch to --jql <query>."
                .into(),
        )
        .into());
    }

    // Validate: --markdown is a modifier on --description/--description-stdin, NOT a
    // standalone field change.  Reject it early (before any HTTP calls) so the user
    // gets a clear error instead of a wasted JQL search followed by "No fields specified".
    if markdown && description.is_none() && !description_stdin {
        return Err(JrError::UserError(
            "--markdown requires --description or --description-stdin to take effect. \
             Pass a description alongside --markdown, or omit --markdown."
                .into(),
        )
        .into());
    }

    // Pre-HTTP guard: if no field-change flags are specified, error here BEFORE running
    // any JQL search or making any HTTP calls.  This is the single source of truth for
    // the "no fields" check — both the JQL path and the dry-run path rely on this guard;
    // there is no duplicate check inside the dry-run block.
    //
    // NOTE: `markdown` is intentionally NOT included here — it is a modifier on
    // --description, not an independent field change. The validation above already
    // rejects `--markdown` without a description, so if we reach this point with
    // `markdown == true`, a description must also be set.
    {
        let has_any_field_change = summary.is_some()
            || priority.is_some()
            || issue_type.is_some()
            || !labels.is_empty()
            || !components.is_empty() // BC-3.4.022: --component add:/remove:
            || team.is_some()
            || points.is_some()
            || no_points
            || parent.is_some()
            || no_parent
            || description.is_some()
            || description_stdin
            || !field_pairs.is_empty(); // S-396: --field NAME=VALUE pairs
        if !has_any_field_change {
            return Err(JrError::UserError(
                "No fields specified to update. Use --summary, --type, --priority, --label, \
                 --component, --team, --points, --no-points, --parent, --no-parent, \
                 --description, --description-stdin, or --field NAME=VALUE."
                    .into(),
            )
            .into());
        }
    }

    // --- Gate B: flag-overlap detection (BC-3.4.017). ---
    // Fires before any HTTP call when a dedicated flag AND --field target the same
    // system field. Covers exactly 5 first-party flags: summary, description,
    // issuetype (--type flag), priority, components (BC-3.4.017 amendment,
    // S-605-1, extended from four fields to five). Team and points use
    // dynamically-resolved IDs; overlap detection for those is deferred to v2
    // (requires an API call, breaking the "no HTTP before the guard" invariant).
    if !field_pairs.is_empty() {
        let field_keys_lower: std::collections::HashSet<String> =
            field_pairs.keys().map(|k| k.to_lowercase()).collect();
        if summary.is_some() && field_keys_lower.contains("summary") {
            return Err(JrError::UserError(
                "summary is set by both --summary and --field; use only one.".into(),
            )
            .into());
        }
        if (description.is_some() || description_stdin) && field_keys_lower.contains("description")
        {
            return Err(JrError::UserError(
                "description is set by both --description / --description-stdin and --field; \
                 use only one."
                    .into(),
            )
            .into());
        }
        if issue_type.is_some() && field_keys_lower.contains("issuetype") {
            return Err(JrError::UserError(
                "issuetype is set by both --type and --field; use only one.".into(),
            )
            .into());
        }
        if priority.is_some() && field_keys_lower.contains("priority") {
            return Err(JrError::UserError(
                "priority is set by both --priority and --field; use only one.".into(),
            )
            .into());
        }
        // BC-3.4.017 amendment (AC-014): `components` joins the flag-overlap
        // set as the 5th member. `--field Components=Y` (any case) also
        // trips this guard — field_keys_lower is already lowercased above.
        if !components.is_empty() && field_keys_lower.contains("components") {
            return Err(JrError::UserError(
                "components is set by both --component and --field; use only one.".into(),
            )
            .into());
        }
    }

    // --- Reject --label combined with non-label field flags. ---
    // --label is routed through a labels-only bulk path (handle_edit_bulk_labels) that
    // does not honour concurrent --summary/--priority/--type flags.  Combining them
    // would silently drop the non-label fields (exit 0, data loss).  Reject the
    // combination HERE, before any HTTP call (including the JQL search), rather than
    // silently discard the fields.
    // Mixed label + field bulk edits require the schema-correct combined payload tracked
    // at #331; until that lands, keep --label and field flags mutually exclusive.
    // NOTE: the variable name 'conflicting' is reserved for this block —
    // test_label_conflict_block_lists_every_relevant_flag uses a global scan of
    // conflicting.push("--...") in edit.rs. If a future cycle introduces a second
    // 'conflicting' variable elsewhere in this file, re-scope the meta-test to
    // brace-matched extraction.
    if !labels.is_empty() {
        let mut conflicting: Vec<&str> = Vec::new();
        if summary.is_some() {
            conflicting.push("--summary");
        }
        if priority.is_some() {
            conflicting.push("--priority");
        }
        if issue_type.is_some() {
            conflicting.push("--type");
        }
        if team.is_some() {
            conflicting.push("--team");
        }
        if points.is_some() {
            conflicting.push("--points");
        }
        if no_points {
            conflicting.push("--no-points");
        }
        if parent.is_some() {
            conflicting.push("--parent");
        }
        if no_parent {
            conflicting.push("--no-parent");
        }
        if description.is_some() {
            conflicting.push("--description");
        }
        if description_stdin {
            conflicting.push("--description-stdin");
        }
        if markdown {
            conflicting.push("--markdown");
        }
        if !field_pairs.is_empty() {
            conflicting.push("--field");
        }
        // BC-3.4.020 amendment (AC-015): --component joins the 13-flag
        // conflict list --label cannot be combined with, on ANY key count.
        // Without this guard the --label-bulk routing fork below would
        // silently drop a concurrent --component write (data-loss hazard,
        // VP-COMPONENT-027) — the same silent-drop shape the rest of this
        // block already guards against for the other 12 flags.
        if !components.is_empty() {
            conflicting.push("--component");
        }
        if !conflicting.is_empty() {
            return Err(JrError::UserError(format!(
                "--label cannot be combined with {} in the same call. \
                 Run separate `jr issue edit` commands, or open an issue to track \
                 combined label + field bulk edits (see #331).",
                conflicting.join(", ")
            ))
            .into());
        }
    }

    // --max is meaningless without --jql (positional keys use the existing 1001-key
    // hard cap, not --max). The handler-level guard earlier in this function already
    // rejects `--max` without `--jql` with JrError::UserError (exit 64) because
    // clap's `requires` attribute interacts poorly with the keys/jql `conflicts_with`
    // relationship. By the time we reach this branch we know jql.is_some() so the
    // unwrap_or(50) default is the right behavior.
    //
    // Step-4.5 Round-1 F3 fix: --component's bulk path chunks internally into
    // <=1000-key POSTs (BC-3.4.023 Postcondition 6) and therefore accepts a
    // --jql match set larger than the per-POST Atlassian limit, up to the
    // same sanity ceiling the positional `keys` argument already allows
    // (JQL_MAX_CEILING, mirroring src/cli/mod.rs's `num_args = 0..=10000`).
    // Every other bulk field path issues a single un-chunked POST, so --max
    // stays hard-capped at BULK_MAX_KEYS for them. clap's value_parser alone
    // cannot see whether --component is present, so it now accepts up to
    // JQL_MAX_CEILING unconditionally (src/cli/mod.rs); this runtime check
    // is what actually enforces the tighter ceiling for every other flag.
    if let Some(m) = max {
        if components.is_empty() && m > BULK_MAX_KEYS as u32 {
            return Err(JrError::UserError(format!(
                "--max {m} exceeds the {BULK_MAX_KEYS}-issue hard ceiling for this edit. \
                 --component bulk edits chunk internally and accept up to {JQL_MAX_CEILING}; \
                 every other bulk field is capped at {BULK_MAX_KEYS} per Atlassian's bulk \
                 API limit."
            ))
            .into());
        }
    }
    let effective_max = max.unwrap_or(50).min(if components.is_empty() {
        BULK_MAX_KEYS as u32
    } else {
        JQL_MAX_CEILING
    });

    // Resolve the working set of keys.
    // For --jql: execute the search (read-only), then enforce --max cap.
    // For positional keys: use them directly (no HTTP read needed).
    let effective_keys: Vec<String> = if let Some(ref jql_str) = jql {
        if jql_str.trim().is_empty() {
            return Err(JrError::UserError(
                "--jql query cannot be empty. Provide a JQL expression like \
                 'project = FOO AND status = \"To Do\"', or pass keys positionally."
                    .into(),
            )
            .into());
        }

        // --dry-run with --jql: search is read-only, allowed.
        let search_result = client
            .search_issue_keys(jql_str, Some(effective_max + 1))
            .await?;
        let matched_keys = search_result.keys;

        if matched_keys.is_empty() {
            return Err(JrError::UserError(format!(
                "JQL '{}' matched 0 issues. Refine your query or pass keys directly.",
                jql_str,
            ))
            .into());
        }

        if matched_keys.len() > effective_max as usize {
            let ceiling = if components.is_empty() {
                BULK_MAX_KEYS as u32
            } else {
                JQL_MAX_CEILING
            };
            return Err(JrError::UserError(format!(
                "JQL matched at least {} issues, which exceeds --max {}. \
                 Use --max <N> to allow up to {} issues, or refine your JQL.",
                matched_keys.len(),
                effective_max,
                ceiling,
            ))
            .into());
        }

        matched_keys
    } else {
        // Positional keys: enforce the Atlassian hard ceiling -- EXCEPT for
        // `--component`, whose bulk path (S-605-2, BC-3.4.023 Postcondition 6)
        // chunks internally into <=1000-key POSTs and therefore accepts a
        // larger resolved key set. Every other bulk field path below issues
        // a single un-chunked POST, so the hard ceiling still applies to them.
        if keys.len() > BULK_MAX_KEYS && components.is_empty() {
            return Err(JrError::UserError(format!(
                "Too many issue keys: {} provided, maximum is {}. \
                 Split into batches of {} or fewer and run multiple times.",
                keys.len(),
                BULK_MAX_KEYS,
                BULK_MAX_KEYS,
            ))
            .into());
        }
        keys.clone()
    };

    // --- C-1: Reject multi-key edits that include flags unsupported in bulk context. ---
    // These flags (parent, team, points, description, markdown) are only implemented
    // on the single-key path. Passing them with multiple keys previously caused silent
    // data loss: the flag was forwarded to handle_edit_bulk_fields which ignored it,
    // then returned Ok(). We now reject early with a clear error so users aren't surprised.
    //
    // This check runs BEFORE the dry-run block so that `--dry-run --no-parent` also
    // reports the unsupported-flag error consistently with the live path.
    if effective_keys.len() > 1 {
        let mut unsupported: Vec<&str> = Vec::new();
        if parent.is_some() {
            unsupported.push("--parent");
        }
        if no_parent {
            unsupported.push("--no-parent");
        }
        if team.is_some() {
            unsupported.push("--team");
        }
        if points.is_some() {
            unsupported.push("--points");
        }
        if no_points {
            unsupported.push("--no-points");
        }
        if description.is_some() || description_stdin {
            unsupported.push("--description / --description-stdin");
        }
        if markdown {
            unsupported.push("--markdown");
        }
        if !field_pairs.is_empty() {
            unsupported.push("--field");
        }
        // BC-3.4.022/BC-3.4.023: --component on 2+ keys no longer falls into
        // this "unsupported on bulk" bucket (S-605-2) — it now routes to its
        // own bulk multiselectComponents path (`handle_edit_bulk_components`,
        // dispatched below, near the --label routing). Intentionally NOT
        // added to `unsupported` here.
        if !unsupported.is_empty() {
            return Err(JrError::UserError(format!(
                "Multi-key bulk edit doesn't yet support: {}. \
                 Use a single key, or open an issue if this matters for your workflow.",
                unsupported.join(", ")
            ))
            .into());
        }
    }

    // --- Step-4.5 Round-1 F1 fix: --component bulk route mutual exclusion. ---
    // `handle_edit_bulk_components` (dispatched near the --label routing,
    // below) issues its OWN, separate multiselectComponents POST sequence
    // and returns immediately -- it never reaches `handle_edit_bulk_fields`,
    // which is the only place --summary/--priority/--type are honored on a
    // multi-key edit. Without this guard, `--component add:X --summary Y`
    // on 2+ keys would silently drop `--summary` (exit 0, data loss) because
    // the --component routing check (below) returns before the bulk-fields
    // routing is ever reached. --label is already covered by the
    // BC-3.4.020 amendment conflict block above (fires unconditionally on
    // any key count) -- included here too for defense-in-depth documentation
    // parity, though it is unreachable in practice (that earlier block
    // already returns before this point whenever both --label and
    // --component are set).
    //
    // NOTE: deliberately NOT named `conflicting` -- that identifier is
    // reserved by the `--label` conflict block above for
    // `test_label_conflict_block_lists_every_relevant_flag`'s global
    // `conflicting.push("--...")` source scan (see that block's own
    // guard comment). A second `conflicting` here would be picked up by
    // that scan and desync it from the `--label` block it actually audits.
    if !components.is_empty() && effective_keys.len() > 1 {
        let mut component_bulk_conflicts: Vec<&str> = Vec::new();
        if summary.is_some() {
            component_bulk_conflicts.push("--summary");
        }
        if priority.is_some() {
            component_bulk_conflicts.push("--priority");
        }
        if issue_type.is_some() {
            component_bulk_conflicts.push("--type");
        }
        if !labels.is_empty() {
            component_bulk_conflicts.push("--label");
        }
        if !component_bulk_conflicts.is_empty() {
            return Err(JrError::UserError(format!(
                "--component on multiple issues cannot be combined with {} in the \
                 same call -- the bulk component path issues its own, separate POST \
                 sequence and cannot also carry those fields. Run separate \
                 `jr issue edit` commands.",
                component_bulk_conflicts.join(", ")
            ))
            .into());
        }
    }

    // --- BC-3.4.023 cross-project guard for --component (fires in BOTH live
    // and dry-run; Step-4.5 Round-2 fix). Mirrors the `--type` guard
    // directly below -- component ids are project-scoped, and the bulk
    // `multiselectComponents` endpoint takes a single project's ids for the
    // entire batch. Before this hoist, this check lived ONLY inside
    // `handle_edit_bulk_components` (the live path), which the `--dry-run`
    // short-circuit below never reaches -- so a multi-key
    // `--component --dry-run` spanning 2+ projects previewed success
    // (resolved against only `effective_keys[0]`'s project) for an input
    // the live run refuses with exit 64. The check inside
    // `handle_edit_bulk_components` itself is KEPT as defense-in-depth --
    // this hoisted copy and that one are deliberately duplicated, not
    // shared, mirroring the `--type` guard's own precedent one block below.
    if !components.is_empty() && effective_keys.len() > 1 {
        let mut project_keys: Vec<&str> = effective_keys
            .iter()
            .map(|k| project_key_from_issue_key(k))
            .collect();
        project_keys.sort_unstable();
        project_keys.dedup();
        if project_keys.len() > 1 {
            return Err(JrError::UserError(format!(
                "--component requires all issues to be in the same project; \
                 the provided keys span {} distinct projects: {}. \
                 Component IDs differ per project, so a single bulk edit cannot \
                 target all of them — split the keys by project and run separate \
                 `jr issue edit` commands.",
                project_keys.len(),
                project_keys.join(", "),
            ))
            .into());
        }
    }

    // --- BC-3.4.019 cross-project guard for --type (fires in BOTH live and dry-run). ---
    // Issue-type IDs are project-scoped; the bulk endpoint takes ONE issueTypeId for
    // the entire batch. A cross-project set cannot be safely resolved to a single id,
    // so we error BEFORE any API call — including before the dry-run block short-circuits.
    //
    // ASYMMETRY (EC-3.4.018-5 vs EC-3.4.019-5): the unknown-type-NAME check requires
    // a createmeta HTTP call, so it is deliberately SKIPPED in dry-run (dry-run emits
    // a bare string for issueType with no id resolution). The cross-project guard is
    // purely client-side (no HTTP needed) and therefore MUST fire even in dry-run.
    if issue_type.is_some() && effective_keys.len() > 1 {
        let mut project_keys: Vec<&str> = effective_keys
            .iter()
            .map(|k| project_key_from_issue_key(k))
            .collect();
        project_keys.sort_unstable();
        project_keys.dedup();
        if project_keys.len() > 1 {
            return Err(JrError::UserError(format!(
                "--type requires all issues to be in the same project; \
                 the provided keys span {} distinct projects: {}. \
                 Issue-type IDs differ per project, so a single bulk edit cannot \
                 target all of them — split the keys by project and run separate \
                 `jr issue edit` commands.",
                project_keys.len(),
                project_keys.join(", "),
            ))
            .into());
        }
    }

    // --- Dry-run short-circuit: render diff, no HTTP mutations. ---
    if dry_run {
        // NOTE: The "no fields specified" guard already fired unconditionally above
        // (pre-HTTP guard, lines ~276-294) before execution reaches here.  No
        // duplicate check needed — any invocation with zero field flags exits before
        // this block is entered.
        //
        // BC-3.4.015 invariant 10: resolve_edit_fields MUST run INSIDE the dry-run
        // block.  Resolution errors exit 64 (NOT suppressed by --dry-run).
        // Gate A already rejected multi-key + --field, so effective_keys has exactly
        // 1 element when field_pairs is non-empty.
        //
        // H-3(b): resolve_edit_fields runs BEFORE the plannedChanges JSON is emitted
        // so that resolved --field entries can be merged into the `planned` map as
        // part of the single coherent JSON object.
        // H-3(a): table-mode --field echo uses println! (stdout), NOT eprintln!
        // (stderr), so the entire planned-changes preview is on one stream.
        let mut dr_changed: BTreeMap<String, String> = BTreeMap::new();
        if !field_pairs.is_empty() {
            let dr_key = &effective_keys[0];
            let mut dr_fields = json!({});
            helpers::resolve_edit_fields(
                client,
                &config.active_profile_name,
                dr_key,
                &field_pairs,
                &mut dr_fields,
                &mut dr_changed,
            )
            .await?;
        }

        // BC-3.4.021 (DEC-274, scope extended by adversary pass-3 MEDIUM-1):
        // resolve the description input — for `--description-stdin`, read
        // stdin via the same `spawn_blocking` + `read_to_string` idiom the
        // live path uses (see `desc_text` below, ~line 642); for bare
        // `--description`, the text is already available synchronously —
        // then render it to ADF via the identical `markdown_to_adf`/
        // `text_to_adf` selection the live path uses. This is a single,
        // unconditional PRE-STEP that MUST complete — including a possible
        // `markdown_to_adf` `Err` (MAX_ADF_DEPTH, BC-7.2.012) propagating as
        // an exit-64 error — BEFORE the `match output_format` block below
        // begins emitting ANY output. This ordering is load-bearing, not
        // cosmetic: `--output table`'s preview lines are printed
        // INCREMENTALLY via per-field `println!` calls, so performing this
        // read+conversion interleaved with (or after) that sequence would
        // risk a depth-guard `Err` leaking partial stdout before the exit-64
        // return, contradicting the "stdout EMPTY on error, in both modes"
        // postcondition (EC-3.4.021-15/-19, VP-692-002/-004). `--dry-run`
        // suppresses mutation HTTP calls only — it does NOT suppress this
        // resolution error (Invariant 2/3).
        let dr_desc_text: Option<String> = if description_stdin {
            let buf = tokio::task::spawn_blocking(|| {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                Ok::<_, std::io::Error>(buf)
            })
            .await??;
            Some(buf)
        } else {
            description.clone()
        };
        let dr_desc_adf: Option<serde_json::Value> = match &dr_desc_text {
            Some(text) => Some(if markdown {
                adf::markdown_to_adf(text)?
            } else {
                adf::text_to_adf(text)
            }),
            None => None,
        };

        // Step-4.5 Round 1, F1 fix (BC-3.4.021 EC-3.4.021-20): --component
        // name resolution (BC-8.4) still fires during dry-run -- it is a
        // read-only GET -- and an unresolvable/ambiguous name still exits 64
        // BEFORE any plannedChanges output, same as resolve_edit_fields and
        // the description ADF conversion above. This is another single,
        // unconditional PRE-STEP that MUST complete before the `match
        // output_format` block below emits ANY output (same load-bearing
        // ordering rationale as dr_desc_text/dr_desc_adf). The preview then
        // renders the RESOLVED canonical name -- parity with the live echo,
        // which also renders resolved names, never the raw CLI input.
        let dr_component_changes: Option<Vec<format::ComponentChange>> = if !components.is_empty() {
            let dr_key = &effective_keys[0];
            let dr_project_key = project_key_from_issue_key(dr_key);
            let dr_changes = format::normalize_component_changes(&components);
            // Step-4.5 Round-3 fix (F2): fetch the project's component
            // candidate list ONCE and reuse it for both the name-resolution
            // preview below and the numeric-id/parse validation that
            // follows -- previously each call independently re-fetched the
            // SAME `GET …/project/{key}/components` for the SAME project,
            // doubling the dry-run's HTTP cost on every multi-key
            // `--component --dry-run` invocation for no behavioral benefit.
            let dr_component_list = client.list_components(dr_project_key).await?;
            let resolved = resolve_component_change_names_with_list(
                &dr_component_list,
                dr_project_key,
                &dr_changes,
            )?;
            // Step-4.5 Round-2 fix (F2): the multi-key bulk LIVE path
            // additionally resolves each change to a numeric componentId via
            // `resolve_bulk_component_ids` (Invariant 2), which performs an
            // explicit `String -> u64` parse that can fail on an
            // oversized/non-parseable numeric-bypass id (e.g.
            // `add:99999999999999999999999999`) even when the NAME-only
            // resolution above succeeds. The dry-run preview must exercise
            // the SAME check -- by the cross-project guard hoisted above
            // this block, `effective_keys` is guaranteed single-project here
            // -- so it never promises success for an input the live run
            // rejects. Single-key dry-run doesn't need this: the single-key
            // live path wires a NAME, never a numeric id (see
            // `resolve_bulk_component_ids`'s doc comment).
            if effective_keys.len() > 1 {
                resolve_bulk_component_ids_with_list(
                    &dr_component_list,
                    dr_project_key,
                    &dr_changes,
                )?;
            }
            Some(resolved)
        } else {
            None
        };

        match output_format {
            OutputFormat::Json => {
                // C-3: --output json must produce machine-readable JSON on stdout,
                // not prose. Build a planned-changes object containing only the
                // fields the user actually requested.
                let mut planned = serde_json::Map::new();
                if let Some(ref s) = summary {
                    planned.insert("summary".into(), json!(s));
                }
                if let Some(ref p) = priority {
                    planned.insert("priority".into(), json!(p));
                }
                if !labels.is_empty() {
                    // NOTE: This entire dry-run preview block (labels here, plus
                    // `priority` and `issueType` below) emits INTENTIONALLY simplified
                    // shapes that DO NOT match the POST body shapes sent to Atlassian:
                    //   - `labels`: dry-run emits `[{"action": "ADD", "name": "foo"}]`
                    //     (flat array). POST body emits
                    //     `{"labelsFields": [{"fieldId": "labels",
                    //       "bulkEditMultiSelectFieldOption": "ADD",
                    //       "labels": [{"name": "foo"}]}]}` (nested array, or
                    //     two elements when ADD+REMOVE coalesce).
                    //   - `priority`: dry-run emits a bare string. POST body wraps as
                    //     `{"priorityId": "<id-string>"}` (name→id resolved via
                    //     GET /rest/api/3/priority; #331).
                    //   - `issueType`: dry-run emits a bare string (the type name).
                    //     POST body uses camelCase `"issueType"` key + `{"issueTypeId": "<id>"}`
                    //     (id resolved via GET /rest/api/3/issue/createmeta/{proj}/issuetypes;
                    //     verified against Atlassian Bulk Operations FAQ, issue #331).
                    //     Dry-run intentionally omits the id resolution call — no HTTP in dry-run.
                    // The dry-run JSON is a human-and-tool-friendly preview, NOT a
                    // byte-for-byte snapshot of the wire request. All three field shapes
                    // (priority, labels, issueType) are empirically verified: priority+issueType
                    // per Atlassian Bulk Operations FAQ (issue #331), labels per #446.
                    let label_entries: Vec<serde_json::Value> = labels
                        .iter()
                        .map(|l| {
                            if let Some(name) = l.strip_prefix("add:") {
                                json!({"action": "ADD", "name": name})
                            } else if let Some(name) = l.strip_prefix("remove:") {
                                json!({"action": "REMOVE", "name": name})
                            } else {
                                json!({"action": "ADD", "name": l})
                            }
                        })
                        .collect();
                    planned.insert("labels".into(), json!(label_entries));
                }
                if let Some(ref component_changes) = dr_component_changes {
                    // BC-3.4.021 amendment (AC-016): structured
                    // `[{"action":"ADD","name":"X"},{"action":"REMOVE","name":"Y"}]`
                    // array — DIFFERENT shape from the comma-joined live-echo
                    // string (format::format_component_changes_echo). Renders
                    // resolved (canonical) names, in CLI input order (F1/F2
                    // fixes) -- resolved above, before this match arm.
                    planned.insert(
                        "components".into(),
                        json!(format::component_changes_dry_run_json(component_changes)),
                    );
                }
                if let Some(ref t) = issue_type {
                    planned.insert("issueType".into(), json!(t));
                }
                if let Some(ref par) = parent {
                    planned.insert("parent".into(), json!(par));
                }
                if no_parent {
                    planned.insert("parent".into(), serde_json::Value::Null);
                }
                if let Some(pts) = points {
                    planned.insert("points".into(), json!(pts));
                }
                if no_points {
                    planned.insert("points".into(), serde_json::Value::Null);
                }
                // Single-key-only fields: team, description, description_stdin, markdown.
                // Multi-key bulk rejects these flags upstream (C-1 guard), so reaching
                // here with effective_keys.len() > 1 and these flags set is impossible.
                if let Some(ref t) = team {
                    planned.insert("team".into(), json!(t));
                }
                // BC-3.4.021 (DEC-274): `description` carries the RAW input string
                // verbatim (BC-3.4.013/#398 unaffected) for EITHER description-input
                // flag; the additive `descriptionAdf` key (nested inside
                // `plannedChanges`, never top-level) carries the real rendered ADF
                // document — byte-identical to what the live path would POST — and
                // is present iff a description input flag was supplied.
                if let Some(ref text) = dr_desc_text {
                    planned.insert("description".into(), json!(text));
                }
                if let Some(ref adf_val) = dr_desc_adf {
                    planned.insert("descriptionAdf".into(), adf_val.clone());
                }
                if markdown {
                    planned.insert("markdown".into(), json!(true));
                }
                // H-3(b): merge resolved --field entries into plannedChanges BEFORE
                // emitting the JSON object (resolve ran above, before this match arm).
                for (field, value) in &dr_changed {
                    planned.insert(field.clone(), json!(value));
                }

                let payload = json!({
                    "dryRun": true,
                    "issues": &effective_keys,
                    "plannedChanges": planned,
                });
                println!("{}", output::render_json(&payload)?);
            }
            OutputFormat::Table => {
                // Human-readable prose on stdout (profile-1 for dry-run: data on stdout is fine).
                println!("DRY RUN — no changes will be made.");
                println!("Issues affected ({}):", effective_keys.len());
                for k in &effective_keys {
                    println!("  {k}");
                }
                println!("Planned changes:");
                if let Some(ref s) = summary {
                    println!("  summary → {s}");
                }
                if let Some(ref p) = priority {
                    println!("  priority → {p}");
                }
                if !labels.is_empty() {
                    println!("  labels → {}", labels.join(", "));
                }
                if let Some(ref component_changes) = dr_component_changes {
                    // BC-3.4.021 amendment (AC-017): identical normalization
                    // to the live-edit echo (AC-012) — bare `X` renders as
                    // `add:X`, never bare. Resolved (canonical) names, in CLI
                    // input order (F1/F2 fixes) -- resolved above, before
                    // this match arm.
                    println!(
                        "  components → {}",
                        format::format_component_changes_echo(component_changes)
                    );
                }
                if let Some(ref t) = issue_type {
                    println!("  type → {t}");
                }
                if let Some(ref par) = parent {
                    println!("  parent → {par}");
                }
                if no_parent {
                    println!("  parent → (clear)");
                }
                if let Some(pts) = points {
                    println!("  points → {pts}");
                }
                if no_points {
                    println!("  points → (clear)");
                }
                // Single-key-only fields: team, description, description_stdin, markdown.
                // Multi-key bulk rejects these flags upstream (C-1 guard), so reaching
                // here with effective_keys.len() > 1 and these flags set is impossible.
                if let Some(ref t) = team {
                    println!("  team → {t}");
                }
                if let Some(ref text) = dr_desc_text {
                    // Truncate long descriptions to 60 codepoints for readability.
                    // Use chars().count() / chars().take(60) — NOT byte slicing —
                    // to avoid panics on multi-byte UTF-8 codepoints (Cyrillic,
                    // CJK, emoji, accented chars). Codepoint-aware is the correct
                    // Rust-stdlib idiom; grapheme clusters (unicode_segmentation)
                    // would be overkill for a display truncation.
                    let char_count = text.chars().count();
                    let preview = if char_count > 60 {
                        let truncated: String = text.chars().take(60).collect();
                        format!("{truncated}...")
                    } else {
                        text.clone()
                    };
                    println!("  description → {preview}");
                }
                if markdown {
                    println!("  markdown rendering: enabled");
                }
                // BC-3.4.021 (DEC-274): unconditional render-OK indicator — emitted
                // whenever a description input was supplied, regardless of whether
                // truncation fired (Postconditions-table item 2, adversary pass-5
                // LOW-1). Table mode never dumps the raw ADF JSON (poor UX); this
                // validated-indicator line confirms the same conversion succeeded.
                // MUST be printed AFTER the "markdown rendering: enabled" line
                // (pinned relative order, adversary pass-5 INFO-2).
                if dr_desc_adf.is_some() {
                    println!("  description (ADF): rendered OK");
                }
                // H-3(a): emit resolved --field entries to stdout (not stderr) so the
                // entire planned-changes preview is on a single coherent stream.
                // resolve ran above (before this match arm), so dr_changed is ready.
                for (field, value) in &dr_changed {
                    println!("  {} \u{2192} {}", field, value);
                }
            }
        }
        return Ok(());
    }

    // --- Confirmation for large JQL match sets. ---
    // Safety-net: when --jql is used AND match count > threshold (JQL_CONFIRM_THRESHOLD),
    // require explicit --yes or interactive confirmation.
    // --no-input without --yes on a large set emits a hint but proceeds
    // (implicit-yes policy for non-interactive mode on any size set).
    if jql.is_some() && effective_keys.len() > JQL_CONFIRM_THRESHOLD {
        if !yes && !no_input {
            // Interactive confirmation via dialoguer.
            let prompt = format!(
                "This will bulk-edit {} issues. Proceed?",
                effective_keys.len()
            );
            let confirmed =
                dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt(prompt)
                    .default(false)
                    .interact()
                    .map_err(|e| {
                        JrError::UserError(format!(
                            "Confirmation prompt failed: {e}. Use --yes to skip the prompt or \
                             --no-input to disable interactive confirmation."
                        ))
                    })?;
            if !confirmed {
                return Err(JrError::UserError(
                    "Bulk edit declined at confirmation prompt. No changes made.".into(),
                )
                .into());
            }
        } else if !yes && no_input {
            // Safety-net hint for --no-input without --yes on a large set.
            eprintln!(
                "Warning: bulk edit will affect {} issues (matched by --jql). \
                 Use --yes to skip this hint, or --dry-run to preview. Proceeding.",
                effective_keys.len()
            );
        }
        // --yes: skip prompt entirely.
    }

    // --- Route: labels → bulk API. ---
    if !labels.is_empty() {
        return handle_edit_bulk_labels(&effective_keys, labels, output_format, client, no_input)
            .await;
    }

    // --- Route: --component on 2+ keys → BC-3.4.023 bulk multiselectComponents
    // path (S-605-2). Entirely separate from `handle_edit_bulk_fields` below —
    // the `multiselectComponents` wire shape requires its own POST sequencing
    // (two sequential POSTs for mixed add:/remove:, plus 1000-issue chunking)
    // that cannot be folded into that function's generic
    // {summary,priority,issueType} single-POST composition (BC-3.4.023
    // Postcondition 2/3). A single effective key falls through to the
    // existing single-key `update`-verb path below (EC-3.4.023-3,
    // BC-3.4.022) — this branch only fires for 2+ keys.
    if !components.is_empty() && effective_keys.len() > 1 {
        return handle_edit_bulk_components(&effective_keys, &components, output_format, client)
            .await;
    }

    // Routing for non-label edits:
    // - 2+ keys (positional or --jql-resolved) → POST /rest/api/3/bulk/issues/fields (bulk API)
    // - 1 key (positional or single-match --jql) → PUT /rest/api/3/issue/{key} (legacy single-key)
    //
    // The single-match --jql case intentionally uses the legacy path because it's
    // per-issue more efficient (no taskId polling) and the bulk API has no advantage
    // for a single issue. Users mental-modeling "JQL → always bulk" should be aware
    // of this asymmetry; it's documented rather than enforced.

    // --- Multi-key non-label: route through bulk_edit_fields. ---
    if effective_keys.len() > 1 {
        return handle_edit_bulk_fields(
            &effective_keys,
            summary.as_deref(),
            priority.as_deref(),
            issue_type.as_deref(),
            output_format,
            client,
        )
        .await;
    }

    // --- Single-key non-label path (unchanged from before) ---
    let key = &effective_keys[0];

    let mut fields = json!({});
    let mut has_updates = false;

    // BC-3.4.012 / BC-3.4.013: track changed fields for echo on success.
    // Populated in parallel with `fields` as each user flag is resolved.
    // Only emitted AFTER PUT 204 — discarded on any error (AC-021, invariant 6).
    let mut changed_fields: BTreeMap<String, String> = BTreeMap::new();

    // Resolve description (see handle_create for rationale on spawn_blocking).
    let desc_text = if description_stdin {
        let buf = tokio::task::spawn_blocking(|| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            Ok::<_, std::io::Error>(buf)
        })
        .await??;
        Some(buf)
    } else {
        description
    };

    if let Some(ref text) = desc_text {
        let adf_body = if markdown {
            adf::markdown_to_adf(text)?
        } else {
            adf::text_to_adf(text)
        };
        fields["description"] = adf_body;
        has_updates = true;
        // BC-3.4.013: JSON changed_fields carries the RAW user-supplied input string —
        // NOT the (updated) marker and NOT an ADF→text round-trip (DECISION LOCKED, AC-016).
        // Table mode echoes the (updated) marker instead; see the emit loop below.
        changed_fields.insert("description".into(), text.clone());
    }

    if let Some(ref s) = summary {
        fields["summary"] = json!(s);
        has_updates = true;
        changed_fields.insert("summary".into(), s.clone());
    }

    if let Some(ref t) = issue_type {
        fields["issuetype"] = json!({ "name": t });
        has_updates = true;
        changed_fields.insert("issue_type".into(), t.clone());
    }

    if let Some(ref p) = priority {
        fields["priority"] = json!({ "name": p });
        has_updates = true;
        changed_fields.insert("priority".into(), p.clone());
    }

    if let Some(ref team_name) = team {
        let (field_id, team_id, resolved_team_name) =
            helpers::resolve_team_field(config, client, team_name, no_input).await?;
        fields[&field_id] = json!(team_id);
        has_updates = true;
        // Echo the RESOLVED display name, not the UUID or partial-match query (AC-002).
        changed_fields.insert("team".into(), resolved_team_name);
    }

    if let Some(pts) = points {
        let field_id = helpers::resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(pts);
        has_updates = true;
        // f64::to_string() at --points branch only (BC-3.4.012 MAJOR-1).
        changed_fields.insert("points".into(), pts.to_string());
    }

    if no_points {
        let field_id = helpers::resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(null);
        has_updates = true;
        // Cleared-field model: key "points", value "(cleared)" (BC-3.4.012 MED-1).
        changed_fields.insert("points".into(), "(cleared)".into());
    }

    if let Some(ref parent_key) = parent {
        fields["parent"] = json!({"key": parent_key});
        has_updates = true;
        changed_fields.insert("parent".into(), parent_key.clone());
    }

    if no_parent {
        fields["parent"] = serde_json::Value::Null;
        has_updates = true;
        // Cleared-field model: key "parent", value "(cleared)" (BC-3.4.012 MED-1).
        changed_fields.insert("parent".into(), "(cleared)".into());
    }

    // BC-3.4.015 invariant 10 (live path): resolve_edit_fields on the live path.
    // Errors here (field not found, absent from editmeta, bad type, etc.) exit 64
    // BEFORE the PUT is issued (all-or-nothing semantics per EC-3.4.015-12).
    //
    // Step-4.5 Round 3, F2 fix (superseded in spirit, not reverted, by the
    // Round-7 MEDIUM-1 single-PUT merge below): this block still runs
    // BEFORE the --component block, so a client-side --field validation
    // failure (unknown field, bad type) exits 64 before any HTTP mutation
    // at all -- unaffected by whether components ends up merged into the
    // same PUT.
    if !field_pairs.is_empty() {
        helpers::resolve_edit_fields(
            client,
            &config.active_profile_name,
            key,
            &field_pairs,
            &mut fields,
            &mut changed_fields,
        )
        .await?;
        has_updates = true;
    }

    // BC-3.4.022 (single-key path ONLY — effective_keys.len() == 1 is
    // guaranteed here by the C-1 rejection block above): components use a
    // DEDICATED wire shape (native `update` verb, or the RMW fallback's
    // full `fields.components` array) that `edit_issue_components` COMPUTES
    // but does NOT PUT itself (Step-4.5 Round 7, MEDIUM-1 fix). Its
    // contribution is merged into the SAME single PUT as every other field
    // change below: a native contribution becomes this PUT's `update`
    // object; a fallback contribution is folded directly into `fields`.
    // Merging into ONE PUT closes the partial-write window a prior
    // two-PUT design had (research: `.factory/research/S-605-1-atomic-
    // component-field-put.md` -- Jira officially supports `update` and
    // `fields` together in one PUT for DISTINCT fields, and validates all
    // fields up front, so a field-validation error, e.g. an invalid
    // priority, rejects the WHOLE edit -- component change included --
    // instead of the component change having already landed via its own,
    // separate, earlier PUT. This is a single-request guarantee scoped to
    // validation errors, per the research -- Atlassian publishes no
    // broader atomic/transactional/rollback guarantee for other failure
    // modes.
    let mut update_obj: Option<serde_json::Value> = None;
    if !components.is_empty() {
        let (component_changes, contribution) =
            edit_issue_components(client, key, &components).await?;
        has_updates = true;
        changed_fields.insert(
            "components".into(),
            format::format_component_changes_echo(&component_changes),
        );
        match contribution {
            ComponentContribution::Native(ops) => {
                // GUARD (research Q3 -- Jira rejects a field present in
                // both `update` and `fields`): `components` lands under
                // `update` here and is NEVER also written into `fields` in
                // this branch -- the fallback branch below is the only
                // other place `fields["components"]` is ever set, and the
                // two are mutually exclusive per invocation (one editmeta
                // gate picks exactly one path).
                update_obj = Some(json!({ "components": ops }));
            }
            ComponentContribution::Fallback(arr) => {
                fields["components"] = json!(arr);
            }
        }
    }

    if !has_updates {
        bail!(
            "No fields specified to update. Use --summary, --type, --priority, --label, --component, --team, --points, --no-points, --parent, --no-parent, --description, --description-stdin, or --field NAME=VALUE."
        );
    }

    // Step-4.5 Round 7, MEDIUM-1: ONE PUT, combining the optional native
    // `update` object with `fields` (`edit_issue_combined` omits `update`
    // when `None` and omits `fields` entirely when empty, so a
    // --component-only edit still sends exactly the same minimal body as
    // before -- AC-001/004/005 continue to assert exactly one PUT).
    let edit_result = client.edit_issue_combined(key, fields, update_obj).await;
    if let Err(ref e) = edit_result {
        // --type arm: evaluated FIRST (dual-gate precedence, BC-3.4.010 invariant).
        // HTTP-400 gate: downcast to JrError::ApiError { status: 400, .. }.
        // Non-400 (401, 403, 5xx, network) → R0b: no enrichment, fall through.
        if let Some(ref type_name) = issue_type {
            if let Some(JrError::ApiError {
                status: 400,
                message: api_msg,
            }) = e.downcast_ref::<JrError>()
            {
                let api_msg = api_msg.clone();
                let type_name_lower = type_name.to_ascii_lowercase();

                // Call ordering (BC-3.4.010 precondition):
                // 1. get_issue first; on Err → Indeterminate immediately (no project-types call).
                // 2. get_project_issue_types next; on Err → Indeterminate.
                // 3. Case-insensitive exact name match; not found → typo hint.
                // 4. Found → classify with is_cross_hierarchy_type_error.
                //
                // Fetch failure gate uses Result::is_err() (not a status downcast) so
                // JrError::NotAuthenticated, InsufficientScope, and all other Err variants
                // correctly trigger Indeterminate (BC-3.4.010 invariant 3).
                let issue_res = client.get_issue(key, &[]).await;
                if let Ok(issue) = issue_res {
                    let src_subtask = issue.fields.issue_type.as_ref().and_then(|t| t.subtask);
                    let project_key = issue
                        .fields
                        .project
                        .as_ref()
                        .map(|p| p.key.clone())
                        .unwrap_or_default();

                    let types_res = client.get_project_issue_types(&project_key).await;
                    if let Ok(project_types) = types_res {
                        if let Some(target) = project_types
                            .iter()
                            .find(|t| t.name.to_ascii_lowercase() == type_name_lower)
                        {
                            let tgt_subtask = target.subtask;
                            match is_cross_hierarchy_type_error(src_subtask, tgt_subtask, &api_msg)
                            {
                                Classification::CrossHierarchy => {
                                    eprintln!("{CROSS_HIERARCHY_HINT}");
                                    bail!("{api_msg}");
                                }
                                Classification::SameCategory => {
                                    eprintln!("{TYPO_HINT}");
                                    bail!("{api_msg}");
                                }
                                Classification::Indeterminate => {
                                    // src or tgt subtask field absent; surface raw
                                    // 400 unchanged — fall through to edit_result?.
                                }
                            }
                        } else {
                            // Type name not in project's list → unresolvable-name
                            // sub-path: typo hint (classifier is NOT invoked).
                            eprintln!("{TYPO_HINT}");
                            bail!("{api_msg}");
                        }
                    }
                    // types_res.is_err() → Indeterminate Cause-1 R2: fall through.
                }
                // issue_res.is_err() → Indeterminate Cause-1 R1: fall through.
            }
            // Non-400 → R0b: fall through.
        }

        // --no-parent arm: only reached when --type arm emitted no hint
        // (dual-gate first-hint-wins: if --type arm bailed, we never reach here).
        if no_parent && is_subtask_parent_error(e) {
            eprintln!("{NO_PARENT_CONTEXT_SENTENCE}");
            eprintln!("{CROSS_HIERARCHY_HINT}");
            bail!("{e}");
        }
    }
    // AC-021 / BC-3.4.012 invariant 6: echo fires ONLY after PUT 204.
    // On any error, edit_result? propagates before the emit loop — changed_fields is discarded.
    edit_result?;

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                output::render_json(&json_output::edit_response(key, &changed_fields))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Updated {}", key));
            // BC-3.4.012: emit one "  field → value" line per changed field, alphabetical.
            // Description asymmetry (AC-016 / CLAUDE.md Gotcha): table shows "(updated)" marker;
            // JSON changed_fields carries the raw input string (see the description insertion above).
            for (field, value) in &changed_fields {
                if field == "description" {
                    // Table mode: marker only — content never echoed (BC-3.4.012, AC-003).
                    eprintln!("  {} \u{2192} (updated)", field);
                } else {
                    eprintln!("  {} \u{2192} {}", field, value);
                }
            }
        }
    }

    Ok(())
}

/// The single-key `--component` edit's wire CONTRIBUTION (Step-4.5 Round 7,
/// MEDIUM-1 fix): `edit_issue_components` no longer PUTs anything itself --
/// it COMPUTES this and returns it so `handle_edit` can merge it into the
/// SAME single PUT as every other field change. `components` lands in
/// EXACTLY ONE of the two top-level PUT keys (`update` or `fields`), never
/// both -- Jira rejects a field present in both (research Q3) -- and this
/// enum's two variants structurally guarantee that: the caller matches on
/// it and merges into the ONE corresponding top-level key.
enum ComponentContribution {
    /// Native update-verb path (editmeta advertises add+remove): the
    /// `update.components` ops array, e.g.
    /// `[{"add":{"name":"X"}},{"remove":{"id":"20002"}}]`.
    Native(Vec<serde_json::Value>),
    /// RMW fallback path (editmeta lacks add/remove): the full computed
    /// `fields.components` array to fold into the caller's `fields` object.
    Fallback(Vec<serde_json::Value>),
}

/// Single-key `--component` add:/remove: wire-shape COMPUTATION (BC-3.4.022).
///
/// Called ONLY from the single-key path of [`handle_edit`] (`effective_keys.len()
/// == 1` is guaranteed by the caller's C-1 rejection block, which rejects
/// multi-key + `--component` upfront — 2+ keys route to S-605-2's
/// BC-3.4.023 bulk wire shape, out of scope here).
///
/// Behavior (BC-3.4.022):
/// 1. Parse/normalize `components` via [`format::normalize_component_changes`]
///    (add:/remove: prefix grammar, bare → ADD, CLI input order preserved --
///    Step-4.5 Round 1 F2 fix).
/// 2. Resolve each component NAME via [`resolve_component_change_names`]
///    (`helpers::resolve_component`, BC-8.4.001), scoped to the issue's own
///    project — extracted from `key` via the last-hyphen split (BC-3.4.018
///    Invariant 4 precedent) — using the project component-list GET
///    (BC-3.4.025), never editmeta, for name validation. Unknown name →
///    exit 64, zero HTTP mutation (AC-006).
/// 3. Evaluate the editmeta gate ONCE (`client.get_editmeta(key)`,
///    `fields.components.operations` containing `add`/`remove`):
///    - Present → build the native `update.components` ops array, zero
///      extra GET for current components (AC-004). `adds`/`removes` are
///      derived by filtering the resolved changes by `action` -- NOT by
///      relying on any pre-grouped order -- so the ops array stays
///      ADD-before-REMOVE regardless of CLI input order (AC-003).
///    - Absent → read-modify-write fallback: GET current `fields.components`,
///      compute the new full array client-side (AC-005).
///      No retry-with-different-shape on a subsequent 400 (Invariant 2).
///
/// Does NOT issue any PUT (Step-4.5 Round 7, MEDIUM-1 fix) -- the caller
/// (`handle_edit`) merges the returned [`ComponentContribution`] into ONE
/// combined PUT alongside every other field change, closing the two-PUT
/// partial-write window a prior design had: a field-validation error (e.g.
/// an invalid priority) now rejects the whole edit in one request instead
/// of a separate, earlier component-only PUT having already landed.
///
/// Returns the resolved changes in CLI input order (F2 fix) so the caller
/// can build `changed_fields`/table echo via
/// [`format::format_component_changes_echo`] without re-deriving the order.
async fn edit_issue_components(
    client: &JiraClient,
    key: &str,
    components: &[String],
) -> Result<(Vec<format::ComponentChange>, ComponentContribution)> {
    // Step 1: parse/normalize add:/remove: entries, CLI input order preserved.
    let changes = format::normalize_component_changes(components);

    // Step 2: resolve each component NAME via BC-8.4.001/BC-3.4.025, scoped
    // to the issue's own project (last-hyphen split, BC-3.4.018 Invariant 4).
    let project_key = project_key_from_issue_key(key);
    let resolved_changes = resolve_component_change_names(client, project_key, &changes).await?;

    // F1 fix: carry the id-vs-name discriminator into the wire-body
    // construction as a ComponentRef, not a bare String -- a numeric
    // resolved value wires as {"id":...}, a name wires as {"name":...}.
    let to_component_ref = |c: &format::ComponentChange| match c.ref_kind {
        format::ComponentRefKind::Id => ComponentRef::Id(c.name.clone()),
        format::ComponentRefKind::Name => ComponentRef::Name(c.name.clone()),
    };
    let adds: Vec<ComponentRef> = resolved_changes
        .iter()
        .filter(|c| c.action == format::ComponentAction::Add)
        .map(to_component_ref)
        .collect();
    let removes: Vec<ComponentRef> = resolved_changes
        .iter()
        .filter(|c| c.action == format::ComponentAction::Remove)
        .map(to_component_ref)
        .collect();

    // Step 3: evaluate the editmeta gate ONCE -- no retry-with-different-shape
    // on a subsequent 400 (Invariant 2).
    let editmeta = client.get_editmeta(key).await?;
    let native_supported = editmeta.fields.get("components").is_some_and(|f| {
        f.operations.iter().any(|op| op == "add") && f.operations.iter().any(|op| op == "remove")
    });

    if native_supported {
        let mut component_ops: Vec<serde_json::Value> = Vec::new();
        for r in &adds {
            component_ops.push(json!({"add": r.to_wire_object()}));
        }
        for r in &removes {
            component_ops.push(json!({"remove": r.to_wire_object()}));
        }
        Ok((
            resolved_changes,
            ComponentContribution::Native(component_ops),
        ))
    } else {
        // Read-modify-write fallback: GET current fields.components,
        // compute the new full array client-side.
        //
        // HIGH-1 fix (Step-4.5 Round 6, DEFINITIVE -- this is the third
        // fix-chain regression in this exact remove-matching logic; see the
        // superseded MED-1/B-LOW-1 history below for what NOT to do again).
        // The matching rule, precisely:
        //
        // 1. An EXISTING component `c` (which has BOTH `id: Option<String>`
        //    AND `name: String`) is REMOVED iff any remove target matches
        //    it against `c`'s OWN fields -- `ComponentRef::Id(id)` against
        //    `c.id`, `ComponentRef::Name(name)` against `c.name` -- checked
        //    directly on the embedded `Component`, never by first
        //    collapsing `c` to a single `ComponentRef` variant. Surviving
        //    existing components are re-emitted by IDENTITY (MED-1,
        //    Round 4): `{"id": ...}` when `c.id` is `Some`, else
        //    `{"name": c.name}` (Jira allows multiple same-named
        //    components -- a bare name is ambiguous when a same-named
        //    sibling also survives).
        // 2. An ADD target `a` is INCLUDED unless (a) a remove target is
        //    the SAME `ComponentRef` (same variant + value, i.e.
        //    `removes.contains(a)`) -- this gives `add:X --component
        //    remove:X` net-ABSENT parity with the native path (B-LOW-1,
        //    Round 5) for BOTH name and numeric X -- OR (b) it already
        //    matches a SURVIVING existing component by id-OR-name (LOW-2,
        //    Step-4.5 Round 7) -- deduped so `add:Backend` against an
        //    issue that already carries an id-bearing "Backend" does not
        //    emit it twice (Jira dedupes server-side regardless, but a
        //    clean payload is better). Condition (b) matches against the
        //    embedded `Component`'s OWN fields (id-OR-name), same as the
        //    remove predicate -- NOT `ComponentRef` equality, because a
        //    NAME add and an id-bearing existing component of that same
        //    name are different `ComponentRef` values that still refer to
        //    the SAME real component.
        // 3. Final `fields.components` = (existing survivors, by identity)
        //    followed by (add survivors, by their own wire shape). Order is
        //    irrelevant to Jira (components is a set-valued field) -- only
        //    the net SET matters.
        // 4. ACCEPTED DIVERGENCE (Step-4.5 Round 8, F-LOW-001): same-
        //    IDENTIFIER add==remove (point 2's `removes.contains(a)` half)
        //    is reconciled to net-ABSENT on BOTH the native and this RMW
        //    path (B-LOW-1). CROSS-identifier add/remove of the SAME
        //    component (e.g. `remove:100` + `add:Backend`, where numeric
        //    id 100 IS the component named Backend) is NOT reconciled
        //    between the two paths, and this divergence is INTENTIONAL,
        //    ACCEPTED, contradictory-input behavior -- not a bug:
        //      - Native path: Jira applies the ops array add-then-remove
        //        (Post 2) -> net ABSENT.
        //      - This RMW fallback: `add_survivors`'s filter only excludes
        //        an add matching a remove target by the SAME `ComponentRef`
        //        variant+value (point 2's same-identifier check) or a
        //        SURVIVING existing component by id-OR-name (point 2's
        //        LOW-2 dedup check) -- neither condition catches a
        //        cross-identifier collision, because "id 100" and "name
        //        Backend" are never resolved to each other here -- so the
        //        Backend add survives -> net PRESENT.
        //    Rationale for accepting rather than reconciling: (a) the
        //    input is self-contradictory -- the user names the SAME
        //    component by two different identifiers with opposite verbs in
        //    one command; there is no single "correct" outcome. (b)
        //    native's "absent" result is Jira-determined by its FIXED
        //    add-before-remove ops ordering (Post 2) -- this fallback
        //    cannot be made to match it without violating that ordering
        //    elsewhere (or reordering ops, which Post 2 forbids). (c)
        //    matching native here would require resolving a NAME add to
        //    its id (or vice versa) purely to detect a same-target
        //    collision -- fragile cross-identifier resolution added to a
        //    code path that has already regressed three times (Rounds 4,
        //    5, 6) -- not worth the risk for a nonsensical input. (d) no
        //    UNRELATED component is ever lost on either path. Pinned (both
        //    sides of the divergence) by
        //    `test_bc_3_4_022_issue_edit_component_rmw_cross_identifier_add_remove_accepted_divergence`
        //    and
        //    `test_bc_3_4_022_issue_edit_component_native_cross_identifier_add_remove_nets_absent`.
        //
        // THE BUG THIS SUPERSEDES: the Round-5 code collapsed each existing
        // component to ONE `ComponentRef` (`Id` when it had one, else
        // `Name`) BEFORE matching against `removes`. Since live Jira ALWAYS
        // returns an id for an issue's embedded components, every existing
        // component became `ComponentRef::Id(...)`. A NAME remove target
        // (`ComponentRef::Name(...)`) can never equal an `Id`-variant value
        // under `ComponentRef`'s derived, variant-sensitive `PartialEq` --
        // so `jr issue edit FOO-1 --component remove:Backend` against a
        // live, id-bearing Backend silently failed to remove it: exit 0,
        // false success echo, the component stayed on the issue. The old
        // code's comment claiming this "mirrors the per-kind matching the
        // old [pre-B-LOW-1] code spelled out explicitly" was WRONG -- the
        // pre-B-LOW-1 code matched removes against the embedded `Component`
        // directly (which has BOTH id and name), so a name-remove matched
        // by name regardless of the component's id; B-LOW-1's refactor
        // silently narrowed that to id-OR-name depending on which field
        // happened to be `Some`, not id-OR-name checked independently. The
        // fix above restores independent id-OR-name matching against the
        // embedded component while KEEPING B-LOW-1's add-before-remove
        // parity for the add side.
        let issue = client.get_issue(key, &[]).await?;
        let current: Vec<crate::types::jira::issue::Component> =
            issue.fields.components.unwrap_or_default();

        let existing_survivor_components: Vec<&crate::types::jira::issue::Component> = current
            .iter()
            .filter(|c| {
                !removes.iter().any(|r| match r {
                    ComponentRef::Id(id) => c.id.as_deref() == Some(id.as_str()),
                    ComponentRef::Name(name) => c.name == *name,
                })
            })
            .collect();

        let existing_survivors: Vec<serde_json::Value> = existing_survivor_components
            .iter()
            .map(|c| match &c.id {
                Some(id) => json!({"id": id}),
                None => json!({"name": &c.name}),
            })
            .collect();

        let add_survivors: Vec<serde_json::Value> = adds
            .iter()
            .filter(|a| {
                !removes.contains(a)
                    && !existing_survivor_components.iter().any(|c| match a {
                        ComponentRef::Id(id) => c.id.as_deref() == Some(id.as_str()),
                        ComponentRef::Name(name) => &c.name == name,
                    })
            })
            .map(ComponentRef::to_wire_object)
            .collect();

        let mut new_components = existing_survivors;
        new_components.extend(add_survivors);

        Ok((
            resolved_changes,
            ComponentContribution::Fallback(new_components),
        ))
    }
}

/// Resolve component change NAMES against the project's component list
/// (BC-8.4.001, BC-3.4.025) — shared by the live single-key wire-shape
/// handler ([`edit_issue_components`]) and the `--dry-run` preview path in
/// [`handle_edit`] (Step-4.5 Round 1, F1 fix: BC-3.4.021 EC-3.4.021-20 --
/// "Component NAME resolution (BC-8.4) still fires during dry-run (it is a
/// read-only GET…) — an unresolvable/ambiguous component name still exits
/// 64 before any plannedChanges output, --dry-run does not suppress this
/// resolution error." — `--dry-run` suppresses mutation HTTP calls only,
/// never this read-only resolution).
///
/// Returns `changes` with each `name` replaced by its resolved canonical
/// name, in the SAME order as the input `changes` (F2 fix: CLI input order
/// is preserved end-to-end through this function — ADD/REMOVE wire
/// reordering happens only at the wire-body construction site in
/// [`edit_issue_components`], never here).
async fn resolve_component_change_names(
    client: &JiraClient,
    project_key: &str,
    changes: &[format::ComponentChange],
) -> Result<Vec<format::ComponentChange>> {
    let component_list = client.list_components(project_key).await?;
    resolve_component_change_names_with_list(&component_list, project_key, changes)
}

/// Core of [`resolve_component_change_names`], parameterized on an
/// already-fetched `component_list` so a caller that needs BOTH this
/// resolution AND [`resolve_bulk_component_ids_with_list`] (the `--dry-run`
/// multi-key preview, Step-4.5 Round-3 F2) can fetch
/// `GET …/project/{key}/components` exactly ONCE and reuse the result for
/// both, instead of each function independently re-fetching the identical
/// list for the identical project.
fn resolve_component_change_names_with_list(
    component_list: &[crate::types::jira::component::Component],
    project_key: &str,
    changes: &[format::ComponentChange],
) -> Result<Vec<format::ComponentChange>> {
    let candidate_names: Vec<String> = component_list.iter().map(|c| c.name.clone()).collect();

    let mut resolved_changes: Vec<format::ComponentChange> = Vec::with_capacity(changes.len());
    for change in changes {
        let matched_name =
            match helpers::resolve_component(&change.name, project_key, &candidate_names) {
                MatchResult::Exact(matched) => matched,
                MatchResult::ExactMultiple(matched_name) => {
                    let ids: Vec<String> = component_list
                        .iter()
                        .filter(|c| c.name.to_lowercase() == matched_name.to_lowercase())
                        .map(|c| c.id.clone())
                        .collect();
                    return Err(JrError::UserError(format!(
                        "Multiple components named \"{}\" found (IDs: {}). \
                         Pass the numeric ID directly.",
                        matched_name,
                        ids.join(", ")
                    ))
                    .into());
                }
                MatchResult::Ambiguous(mut candidates) => {
                    candidates.sort_by_key(|s| s.to_lowercase());
                    return Err(JrError::UserError(format!(
                        "Ambiguous component '{}'. Matches: {}.",
                        change.name,
                        candidates.join(", ")
                    ))
                    .into());
                }
                MatchResult::None(mut available) => {
                    available.sort_by_key(|s| s.to_lowercase());
                    return Err(JrError::UserError(format!(
                        "Component '{}' not found in project {}. Available: {}.",
                        change.name,
                        project_key,
                        available.join(", ")
                    ))
                    .into());
                }
            };
        resolved_changes.push(format::ComponentChange {
            action: change.action.clone(),
            name: matched_name,
            // F1 fix: ref_kind is carried forward unchanged from the raw
            // input (determined at parse time in
            // format::normalize_component_changes) -- resolution never
            // changes whether a value is a name or a numeric id.
            ref_kind: change.ref_kind,
        });
    }
    Ok(resolved_changes)
}

/// Build the `editedFieldsInput` JSON object for a multi-key bulk-labels edit.
///
/// Returns the complete `editedFieldsInput` object to be passed directly to
/// `bulk_edit_fields`. Implements the verified Atlassian Bulk Operations schema:
///
/// ```json
/// {
///   "labelsFields": [
///     {"fieldId":"labels","bulkEditMultiSelectFieldOption":"ADD","labels":[{"name":"foo"}]},
///     {"fieldId":"labels","bulkEditMultiSelectFieldOption":"REMOVE","labels":[{"name":"bar"}]}
///   ]
/// }
/// ```
///
/// - Each action (ADD / REMOVE) is a separate element in the `labelsFields` array.
/// - Label items are `{"name": <string>}` objects — NOT bare strings.
///   (Bare strings are the PUT /rest/api/3/issue single-key path; see `update_issue_labels`.)
/// - `selectedActions: ["labels"]` is the caller's responsibility (passed to `bulk_edit_fields`).
///
/// Caller MUST bail BEFORE calling this if both inputs are empty.
///
/// Pure function — no I/O, no async, no client refs.
///
/// Verified schema source: Atlassian Bulk Operations FAQ,
/// https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/
/// (issue #446).
fn build_labels_edited_fields(adds: &[String], removes: &[String]) -> serde_json::Value {
    debug_assert!(
        !adds.is_empty() || !removes.is_empty(),
        "build_labels_edited_fields: caller MUST bail when both inputs are empty (BC-3.4.006)",
    );
    let mut labels_fields: Vec<serde_json::Value> = Vec::new();
    if !adds.is_empty() {
        let add_entries: Vec<serde_json::Value> = adds.iter().map(|n| json!({"name": n})).collect();
        labels_fields.push(json!({
            "fieldId": "labels",
            "bulkEditMultiSelectFieldOption": "ADD",
            "labels": add_entries
        }));
    }
    if !removes.is_empty() {
        let remove_entries: Vec<serde_json::Value> =
            removes.iter().map(|n| json!({"name": n})).collect();
        labels_fields.push(json!({
            "fieldId": "labels",
            "bulkEditMultiSelectFieldOption": "REMOVE",
            "labels": remove_entries
        }));
    }
    json!({ "labelsFields": labels_fields })
}

/// Route label edits through the Atlassian Bulk Fields API.
///
/// Supports 1..=1000 keys. `labels` is a list of "add:NAME" / "remove:NAME" / "NAME" strings.
///
/// NOTE: The `--dry-run --output json` `plannedChanges.labels` shape (built in the
/// dry-run block of `handle_edit` above) is a SIMPLIFIED preview using `{action, name}`
/// pairs in a flat array, NOT a byte-for-byte snapshot of the POST body built here.
/// Dry-run is a human-and-tool-friendly diff.
///
/// editedFieldsInput shape (verified against Atlassian Bulk Operations FAQ, issue #446):
///   ```json
///   {
///     "labelsFields": [
///       {"fieldId":"labels","bulkEditMultiSelectFieldOption":"ADD","labels":[{"name":"foo"}]},
///       {"fieldId":"labels","bulkEditMultiSelectFieldOption":"REMOVE","labels":[{"name":"bar"}]}
///     ]
///   }
///   ```
/// ADD and REMOVE are separate elements in the `labelsFields` array.
/// ADD+REMOVE coalesces into ONE bulk POST (`.expect(1)` enforced).
/// Label items are `{"name":...}` objects — NOT bare strings.
/// (Bare strings apply only to `PUT /rest/api/3/issue` single-key path.)
///
/// Source: https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/
///
/// Output:
/// - Table mode: per-key success/error lines.
/// - JSON mode: `{"taskId":"...","results":[{"key":"...","status":"success|error","error":"..."}]}`
/// - Single-key JSON mode: also includes `"key":"..."` at top level (backward-compat shape).
/// - Exit 0 if all succeeded; exit 1 if any failed.
async fn handle_edit_bulk_labels(
    keys: &[String],
    labels: Vec<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
    _no_input: bool,
) -> Result<()> {
    // Parse labels into add/remove buckets.
    let mut adds: Vec<String> = Vec::new();
    let mut removes: Vec<String> = Vec::new();

    for l in &labels {
        if let Some(name) = l.strip_prefix("add:") {
            adds.push(name.to_string());
        } else if let Some(name) = l.strip_prefix("remove:") {
            removes.push(name.to_string());
        } else {
            // Bare label treated as add.
            adds.push(l.clone());
        }
    }

    if adds.is_empty() && removes.is_empty() {
        bail!("No label changes specified.");
    }

    // --- Route: single key → PUT /rest/api/3/issue/{key} with update.labels ---
    //
    // Single-key label edits use PUT with the `update` verb (bare-string label values).
    // This avoids the bulk endpoint entirely: the bulk endpoint requires a different
    // payload shape (`labelsFields` array, `selectedActions`, `{"name":...}` objects)
    // and was causing HTTP 400 on real Jira instances (BUG-LABEL-400, live E2E run
    // 26730687481). The PUT path is synchronous (204 No Content) and simpler.
    //
    // Verified payload shape: Atlassian Cloud REST API v3 PUT /rest/api/3/issue/{key}
    // "update" verb (https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/#api-rest-api-3-issue-issueidorkey-put);
    // empirically confirmed by live E2E run 26730687481 (bulk-payload shape → HTTP 400).
    //   {"update": {"labels": [{"add": "foo"}, {"remove": "bar"}]}}
    // where label values are BARE STRINGS, not {"name": "..."} objects.
    if keys.len() == 1 {
        let key = &keys[0];
        client.update_issue_labels(key, &adds, &removes).await?;

        // Build changed_fields for the echo: record adds and removes as human-readable strings.
        let mut changed_fields: BTreeMap<String, String> = BTreeMap::new();
        let mut parts: Vec<String> = Vec::new();
        for a in &adds {
            parts.push(format!("add:{a}"));
        }
        for r in &removes {
            parts.push(format!("remove:{r}"));
        }
        changed_fields.insert("labels".into(), parts.join(", "));

        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    output::render_json(&json_output::edit_response(key, &changed_fields))?
                );
            }
            OutputFormat::Table => {
                output::print_success(&format!("Updated {}", key));
                eprintln!("  labels \u{2192} {}", parts.join(", "));
            }
        }
        return Ok(());
    }

    // --- Route: multi-key (2+) → POST /rest/api/3/bulk/issues/fields ---
    //
    // Coalesce ADD and REMOVE into a single bulk POST when both are present.
    // Both operations are submitted in one request as an array of label-action objects.
    // See build_labels_edited_fields doc-comment for the verbatim #331 schema caveat.
    let edited_fields = build_labels_edited_fields(&adds, &removes);

    // selectedActions for labels is always ["labels"] regardless of ADD/REMOVE/coalesce.
    let task_id = client
        .bulk_edit_fields(keys, vec!["labels".to_string()], edited_fields)
        .await?;
    // Poll with 5-minute timeout.
    let progress = client
        .await_bulk_task(&task_id, resolve_bulk_await_timeout())
        .await?;

    render_bulk_edit_results(keys, &task_id, &progress, output_format)
}

/// Extract the project key from an issue key by splitting on the last hyphen.
///
/// Examples:
///   `"FOO-1"` → `"FOO"`
///   `"PROJ2-100"` → `"PROJ2"`
///
/// This is used by the cross-project guard in `handle_edit_bulk_fields` to detect
/// when a multi-key `--type` bulk edit spans multiple projects (which is not supported
/// because issue-type IDs are project-scoped). Verified by `test_project_key_extraction`.
fn project_key_from_issue_key(key: &str) -> &str {
    match key.rfind('-') {
        Some(pos) => &key[..pos],
        None => key,
    }
}

/// `jr issue edit KEY1 KEY2 ... --component add:X` — multi-key/`--jql` bulk
/// `--component` edit (BC-3.4.023, S-605-2). Entirely separate wire path from
/// `handle_edit_bulk_fields`: the `multiselectComponents` schema holds only
/// ONE `bulkEditMultiSelectFieldOption` per POST (unlike `labelsFields`'
/// array-of-elements shape), so mixed `add:`/`remove:` specs require TWO
/// sequential POSTs rather than one coalesced POST (Postcondition 3).
///
/// Precondition (enforced by the caller, `handle_edit`): `keys.len() > 1`.
/// A single effective key is routed to the existing single-key `update`-verb
/// path (`edit_issue_components`, BC-3.4.022) instead — EC-3.4.023-3.
///
/// What this function does, step by step:
///
/// 1. **EC-3.4.023-1 cross-project guard**: `keys` spanning 2+ distinct
///    projects (via [`project_key_from_issue_key`]) → exit 64
///    (`JrError::UserError`) BEFORE any HTTP call — component ids are
///    project-scoped, mirroring `handle_edit_bulk_fields`'s `--type` guard
///    (BC-3.4.019).
/// 2. **Postcondition 4 / Invariant 2 — resolve + parse**: parse `components`
///    via [`format::normalize_component_changes`], resolve each NAME to a
///    numeric id via §8.4 ([`resolve_bulk_component_ids`],
///    `helpers::resolve_component`), then an explicit `String` -> `u64`
///    parse (`id.parse::<u64>()`) immediately before body assembly — the
///    bulk endpoint requires a JSON integer `componentId`, never a string or
///    `{"name":...}` object. A parse failure on the numeric-id-bypass path
///    (user input) surfaces as `JrError::UserError`; a parse failure on a
///    resolver-returned name's looked-up id (which should be unreachable)
///    surfaces as `JrError::Internal` (Step-4.5 Round-1 F4 fix).
/// 3. **Postcondition 1/2 — wire shape**: build the `editedFieldsInput` body
///    via [`crate::api::jira::bulk::build_component_edited_fields`] with
///    `selectedActions == ["components"]` (lowercase field id).
/// 4. **Postcondition 3 — two sequential POSTs for mixed add:/remove:**: when
///    both `add:` and `remove:` specs are present, the ADD POST is issued
///    first (fully polled via `await_bulk_task` to completion), THEN the
///    REMOVE POST — never coalesced into one POST.
/// 5. **Postcondition 6 / EC-3.4.023-4 — 1000-issue chunking**: `keys` is
///    split into sequential chunks of <= [`crate::api::jira::bulk::BULK_MAX_KEYS`],
///    each fully polled to completion before the next chunk's POST fires
///    (chunk-major, action-minor ordering when combined with item 4 above —
///    `2 * ceil(N/1000)` POSTs total for N>1000 issues with mixed
///    add:/remove:). A chunk failure ABORTS the remaining sequence (no
///    continue-on-error, unlike `component rename --all-projects`) —
///    surfaced via the existing `await_bulk_task` error path. Already-
///    successful earlier chunks are NOT rolled back.
/// 6. Every (chunk, action) cycle's outcome is accumulated into a
///    [`BulkComponentOpResult`] and rendered ONCE, after the loop, via
///    [`render_bulk_component_results`] — a single coherent `--output json`
///    document (or table-mode row sequence) for the whole invocation,
///    never one document per cycle (Step-4.5 Round-1 F2 fix).
///
/// **Mutual exclusion (Step-4.5 Round-1 F1 fix):** the caller (`handle_edit`)
/// rejects `--component` on 2+ keys combined with `--summary`/`--priority`/
/// `--type`/`--label` before this function is ever reached — this path's
/// POST sequence has no way to also carry those fields, so silently
/// proceeding would drop them.
///
/// **Release gate (DEC-280, BC-3.4.023 Delivery note):** this path MUST NOT
/// ship to release until a live smoke test (one ADD, one REMOVE, >= 2 issues,
/// one project with >= 1 component already defined) confirms the
/// `multiselectComponents` wire shape documented above (AC-010).
async fn handle_edit_bulk_components(
    keys: &[String],
    components: &[String],
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    // 1. EC-3.4.023-1: cross-project guard, BEFORE any HTTP call. Mirrors
    // `handle_edit_bulk_fields`'s `--type` guard (BC-3.4.019) exactly --
    // component ids are project-scoped.
    let mut project_keys: Vec<&str> = keys.iter().map(|k| project_key_from_issue_key(k)).collect();
    project_keys.sort_unstable();
    project_keys.dedup();
    if project_keys.len() > 1 {
        return Err(JrError::UserError(format!(
            "--component requires all issues to be in the same project; \
             the provided keys span {} distinct projects: {}. \
             Component IDs differ per project, so a single bulk edit cannot \
             target all of them — split the keys by project and run separate \
             `jr issue edit` commands.",
            project_keys.len(),
            project_keys.join(", "),
        ))
        .into());
    }
    // `keys.len() > 1` is guaranteed by the caller (`handle_edit`'s routing
    // block), so `project_keys` is non-empty here.
    let project_key = project_keys[0];

    // 2. Postcondition 4 / Invariant 2: resolve NAMEs to numeric componentIds
    // via §8.4 BEFORE any bulk POST is built (AC-004: an unknown/ambiguous
    // name must produce ZERO bulk POSTs).
    let (add_ids, remove_ids) = resolve_bulk_component_ids(client, project_key, components).await?;

    if add_ids.is_empty() && remove_ids.is_empty() {
        bail!("No component changes specified.");
    }

    // 3. Postcondition 6: split `keys` into sequential <= BULK_MAX_KEYS
    // chunks, chunk-major ordering. Within each chunk, ADD is issued (fully
    // polled) BEFORE REMOVE when both are present (Postcondition 3) -- never
    // coalesced into one POST, unlike the label bulk path. A chunk (or
    // action-within-chunk) failure propagates immediately via `?`, aborting
    // the remaining sequence (EC-3.4.023-4) -- already-successful earlier
    // chunks are NOT rolled back.
    //
    // Step-4.5 Round-1 F2 fix: each (chunk, action) cycle used to call
    // `render_bulk_edit_results` directly, which prints its own top-level
    // JSON document via `println!` -- a mixed add:/remove: edit (or a
    // >1000-issue chunked edit) therefore printed MULTIPLE concatenated
    // JSON documents on stdout, which no single `serde_json::from_str` call
    // can parse, and doubled up the table-mode success lines. Results are
    // now accumulated across every (chunk, action) cycle and rendered ONCE,
    // after the loop, as a single coherent output.
    let mut ops: Vec<BulkComponentOpResult> = Vec::new();
    for chunk in keys.chunks(BULK_MAX_KEYS) {
        if !add_ids.is_empty() {
            ops.push(
                run_bulk_component_action(chunk, &add_ids, BulkMultiSelectFieldOption::Add, client)
                    .await?,
            );
        }
        if !remove_ids.is_empty() {
            ops.push(
                run_bulk_component_action(
                    chunk,
                    &remove_ids,
                    BulkMultiSelectFieldOption::Remove,
                    client,
                )
                .await?,
            );
        }
    }

    render_bulk_component_results(&ops, output_format)
}

/// One (chunk, action) bulk POST + poll cycle's outcome, accumulated across
/// the whole `handle_edit_bulk_components` invocation (Step-4.5 Round-1 F2
/// fix) so the caller can render a single, coherent result once every cycle
/// has completed, instead of once per cycle.
struct BulkComponentOpResult {
    task_id: String,
    action: BulkMultiSelectFieldOption,
    keys: Vec<String>,
    progress: crate::types::jira::bulk::BulkOperationProgress,
}

/// Resolve `--component` add:/remove: specs to numeric `componentId`s for
/// the bulk `multiselectComponents` wire shape (BC-3.4.023 Postcondition 4,
/// Invariant 2). Returns `(add_ids, remove_ids)`, each in CLI input order
/// within its own action bucket.
///
/// Distinct from [`resolve_component_change_names`] (the single-key path's
/// resolver): that function returns canonical NAMEs (or a passed-through
/// numeric id string) for the `update`-verb wire shape, which wires a name
/// as `{"name": ...}` and never needs the id. This bulk path needs a numeric
/// `componentId` for EVERY resolved change -- name-resolved or
/// id-passed-through alike -- so it performs the name -> id lookup inline
/// against the SAME fetched candidate list, then the explicit `String` ->
/// `u64` parse Invariant 2 requires. A parse failure's error type depends on
/// WHICH branch produced the id string (Step-4.5 Round-1 F4 fix): a
/// resolver-returned NAME whose looked-up id is non-numeric is a genuine
/// internal-invariant violation (every candidate list entry's id is itself
/// a digit-only string on the wire) -- surfaced as `JrError::Internal`.
/// A value from the §8.4 numeric-id bypass (BC-8.4.001 step 1 --
/// all-ASCII-digit CLI input forwarded verbatim, skipping `partial_match`
/// entirely) IS user input, and CAN overflow `u64` (e.g.
/// `--component add:99999999999999999999999999`) -- that failure surfaces
/// as `JrError::UserError` (exit 64), never `JrError::Internal`.
async fn resolve_bulk_component_ids(
    client: &JiraClient,
    project_key: &str,
    components: &[String],
) -> Result<(Vec<u64>, Vec<u64>)> {
    let changes = format::normalize_component_changes(components);
    let component_list = client.list_components(project_key).await?;
    resolve_bulk_component_ids_with_list(&component_list, project_key, &changes)
}

/// Core of [`resolve_bulk_component_ids`], parameterized on an
/// already-fetched `component_list` and already-normalized `changes` so a
/// caller that needs BOTH this AND [`resolve_component_change_names_with_list`]
/// (the `--dry-run` multi-key preview, Step-4.5 Round-3 F2) can fetch
/// `GET …/project/{key}/components` exactly ONCE and reuse the result for
/// both, instead of each function independently re-fetching the identical
/// list for the identical project.
fn resolve_bulk_component_ids_with_list(
    component_list: &[crate::types::jira::component::Component],
    project_key: &str,
    changes: &[format::ComponentChange],
) -> Result<(Vec<u64>, Vec<u64>)> {
    let candidate_names: Vec<String> = component_list.iter().map(|c| c.name.clone()).collect();

    let mut add_ids: Vec<u64> = Vec::new();
    let mut remove_ids: Vec<u64> = Vec::new();

    for change in changes {
        let matched_name =
            match helpers::resolve_component(&change.name, project_key, &candidate_names) {
                MatchResult::Exact(matched) => matched,
                MatchResult::ExactMultiple(matched_name) => {
                    let ids: Vec<String> = component_list
                        .iter()
                        .filter(|c| c.name.to_lowercase() == matched_name.to_lowercase())
                        .map(|c| c.id.clone())
                        .collect();
                    return Err(JrError::UserError(format!(
                        "Multiple components named \"{}\" found (IDs: {}). \
                         Pass the numeric ID directly.",
                        matched_name,
                        ids.join(", ")
                    ))
                    .into());
                }
                MatchResult::Ambiguous(mut candidates) => {
                    candidates.sort_by_key(|s| s.to_lowercase());
                    return Err(JrError::UserError(format!(
                        "Ambiguous component '{}'. Matches: {}.",
                        change.name,
                        candidates.join(", ")
                    ))
                    .into());
                }
                MatchResult::None(mut available) => {
                    available.sort_by_key(|s| s.to_lowercase());
                    return Err(JrError::UserError(format!(
                        "Component '{}' not found in project {}. Available: {}.",
                        change.name,
                        project_key,
                        available.join(", ")
                    ))
                    .into());
                }
            };

        // `matched_name` is either the passed-through numeric id
        // (BC-8.4.001 step-1 bypass -- USER input, verbatim) or the resolved
        // canonical component NAME (resolver output). The bulk wire shape
        // needs a numeric componentId either way (Invariant 2) -- resolve a
        // name to its id via the same fetched candidate list.
        //
        // Step-4.5 Round-1 F4 fix: track WHICH of the two branches produced
        // `id_str` so a subsequent parse failure can be attributed
        // correctly. The numeric bypass forwards raw user input verbatim
        // (an all-ASCII-digit CLI value can still overflow u64, e.g.
        // `--component add:99999999999999999999999999`) -- that failure
        // came from user input and must be a `JrError::UserError` (exit
        // 64), not `JrError::Internal`. `JrError::Internal` is reserved for
        // the OTHER branch: a resolver-returned NAME whose looked-up id is
        // somehow non-numeric, which genuinely should be unreachable.
        let (id_str, id_is_user_input) =
            if !matched_name.is_empty() && matched_name.chars().all(|c| c.is_ascii_digit()) {
                (matched_name, true)
            } else {
                let id = component_list
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(&matched_name))
                    .map(|c| c.id.clone())
                    .ok_or_else(|| {
                        JrError::Internal(format!(
                            "Internal error: resolved component name {matched_name:?} was not \
                             found in the fetched component list for project {project_key} -- \
                             this should be unreachable (the resolver only returns names present \
                             in the same list)."
                        ))
                    })?;
                (id, false)
            };

        let id: u64 = id_str.parse().map_err(|e| {
            if id_is_user_input {
                JrError::UserError(format!(
                    "component id out of range or not found: {id_str} ({e})"
                ))
            } else {
                JrError::Internal(format!(
                    "Internal error: resolved componentId {id_str:?} is not numeric ({e}) -- \
                     every resolver-returned component id should be a digit-only string on the \
                     wire (BC-3.4.023 Invariant 2)."
                ))
            }
        })?;

        match change.action {
            format::ComponentAction::Add => add_ids.push(id),
            format::ComponentAction::Remove => remove_ids.push(id),
        }
    }

    Ok((add_ids, remove_ids))
}

/// Issue ONE bulk `multiselectComponents` POST for `chunk_keys` + `option`
/// and poll it to completion via the existing `await_bulk_task` machinery.
/// Returns the raw [`BulkComponentOpResult`] for the caller to accumulate --
/// this function does NOT render anything itself. Rendering is deferred to
/// a single call to [`render_bulk_component_results`], made once every
/// (chunk, action) cycle in `handle_edit_bulk_components` has completed
/// (Step-4.5 Round-1 F2 fix), so a multi-cycle invocation (a mixed
/// add:/remove: edit, or a >1000-issue chunked edit) never emits more than
/// one coherent result document. Shared by every (chunk, action) pair
/// `handle_edit_bulk_components` iterates over (BC-3.4.023 Postcondition 3
/// / Postcondition 6).
async fn run_bulk_component_action(
    chunk_keys: &[String],
    ids: &[u64],
    option: BulkMultiSelectFieldOption,
    client: &JiraClient,
) -> Result<BulkComponentOpResult> {
    let edited_fields = build_component_edited_fields(ids, option);
    let task_id = client
        .bulk_edit_fields(chunk_keys, vec!["components".to_string()], edited_fields)
        .await?;
    let progress = client
        .await_bulk_task(&task_id, resolve_bulk_await_timeout())
        .await?;
    Ok(BulkComponentOpResult {
        task_id,
        action: option,
        keys: chunk_keys.to_vec(),
        progress,
    })
}

/// Render every accumulated (chunk, action) cycle's outcome as ONE coherent
/// result (Step-4.5 Round-1 F2 fix) -- a single top-level JSON document in
/// `--output json` mode, or a single flat sequence of per-key table rows in
/// table mode. Distinct from [`render_bulk_edit_results`] (used by the
/// labels and generic-fields bulk paths, which only ever issue ONE bulk
/// POST + poll cycle per invocation and therefore have no multi-cycle
/// aggregation concern).
fn render_bulk_component_results(
    ops: &[BulkComponentOpResult],
    output_format: &OutputFormat,
) -> Result<()> {
    let mut any_failed = false;
    let mut operations_json: Vec<serde_json::Value> = Vec::new();

    for op in ops {
        let processed: std::collections::HashSet<&str> = op
            .progress
            .processed_accessible_issues
            .iter()
            .map(String::as_str)
            .collect();

        let mut results: Vec<serde_json::Value> = Vec::new();
        for key in &op.keys {
            if let Some(err) = op.progress.failed_accessible_issues.get(key.as_str()) {
                results.push(json!({
                    "key": key,
                    "status": "error",
                    "error": err.summary(),
                }));
                any_failed = true;
            } else if processed.contains(key.as_str()) {
                results.push(json!({
                    "key": key,
                    "status": "success",
                }));
            } else {
                results.push(json!({
                    "key": key,
                    "status": "inaccessible",
                }));
            }
        }
        // Also capture any failed keys that weren't in this op's chunk
        // (shouldn't happen, but Atlassian may return unexpected keys).
        for (failed_key, err) in &op.progress.failed_accessible_issues {
            if !op.keys.iter().any(|k| k == failed_key) {
                results.push(json!({
                    "key": failed_key,
                    "status": "error",
                    "error": err.summary(),
                }));
                any_failed = true;
            }
        }

        let action_str = match op.action {
            BulkMultiSelectFieldOption::Add => "ADD",
            BulkMultiSelectFieldOption::Remove => "REMOVE",
        };
        operations_json.push(json!({
            "taskId": op.task_id,
            "action": action_str,
            "results": results,
        }));
    }

    match output_format {
        OutputFormat::Json => {
            // Single top-level JSON document for the ENTIRE invocation --
            // never one `println!` per (chunk, action) cycle (Step-4.5
            // Round-1 F2 fix; JSON render invariant #526).
            let payload = json!({ "operations": operations_json });
            println!("{}", output::render_json(&payload)?);
        }
        OutputFormat::Table => {
            for op in &operations_json {
                for entry in op["results"]
                    .as_array()
                    .expect("results is always an array")
                {
                    let key = entry["key"].as_str().unwrap_or("?");
                    match entry["status"].as_str().unwrap_or("?") {
                        "success" => output::print_success(&format!("Updated {key}")),
                        "error" => {
                            let err_msg = entry["error"].as_str().unwrap_or("unknown error");
                            eprintln!("error: {key}: {err_msg}");
                        }
                        status => eprintln!("warning: {key}: {status}"),
                    }
                }
            }
        }
    }

    if any_failed {
        bail!("One or more issues failed during bulk edit. See output above for details.");
    }

    Ok(())
}

/// Supports 2..=1000 keys with --summary, --priority, --type.
///
/// editedFieldsInput shape (verified against Atlassian Bulk Operations FAQ, issue #331):
/// ```json
/// {
///   "summary": "New title",
///   "priority": {"priorityId": "3"},
///   "issueType": {"issueTypeId": "10001"}
/// }
/// ```
///
/// Priority resolution: calls `GET /rest/api/3/priority` (global, no cache).
/// Issue type resolution: calls `GET /rest/api/3/issue/createmeta/{proj}/issuetypes`
/// (project-scoped, no cache). Requires all keys to be from the same project
/// (BC-3.4.019 cross-project guard — exits 64 before any API call if guard fires).
///
/// The `selectedActions` element for issue type is lowercase `"issuetype"` (Atlassian
/// canonical), while the `editedFieldsInput` key is camelCase `"issueType"`. These
/// INTENTIONALLY differ per the Atlassian Bulk Operations FAQ — do NOT "fix" the
/// asymmetry. See `.factory/research/issue-331-issuetype-bulk-schema.md`.
async fn handle_edit_bulk_fields(
    keys: &[String],
    summary: Option<&str>,
    priority: Option<&str>,
    issue_type: Option<&str>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let mut edited = serde_json::Map::new();
    let mut selected_actions: Vec<String> = Vec::new();

    if let Some(s) = summary {
        edited.insert("summary".into(), json!(s));
        selected_actions.push("summary".to_string());
    }
    if let Some(p) = priority {
        // Bulk endpoint requires {"priorityId": "<id-string>"}, NOT {"name": "High"}.
        // Resolve name→id via GET /rest/api/3/priority (one extra HTTP call only when
        // --priority is used on the bulk path).
        // Source: Atlassian Bulk Operations FAQ (issue #331).
        let priorities = client.get_priorities().await?;
        let p_lower = p.to_lowercase();
        let priority_id = priorities
            .iter()
            .find(|pm| pm.name.to_lowercase() == p_lower)
            .map(|pm| pm.id.clone())
            .ok_or_else(|| {
                let valid: Vec<&str> = priorities.iter().map(|pm| pm.name.as_str()).collect();
                JrError::UserError(format!(
                    "Priority '{p}' not found. Valid priorities: {}. \
                     Run `jr project fields --project <KEY>` to see priorities for your project.",
                    valid.join(", ")
                ))
            })?;
        edited.insert("priority".into(), json!({"priorityId": priority_id}));
        selected_actions.push("priority".to_string());
    }
    if let Some(t) = issue_type {
        // BC-3.4.018: resolve issue type name → id via project-scoped createmeta endpoint.
        // No cache — one HTTP call per --type bulk invocation (matches priority resolver model).
        // Source: Atlassian Bulk Operations FAQ + createmeta issuetypes endpoint docs (issue #331).
        //
        // The BC-3.4.019 cross-project guard (ensuring all keys are same-project) already
        // fired in handle_edit before this function was called — so here we know all keys
        // share the same project key and we can safely use keys[0] to derive it.
        let project_key = project_key_from_issue_key(&keys[0]);
        let issue_types = client.get_issue_types_for_project(project_key).await?;
        let t_lower = t.to_lowercase();
        let type_id = issue_types
            .iter()
            .find(|it| it.name.to_lowercase() == t_lower)
            .map(|it| it.id.clone())
            .ok_or_else(|| {
                let valid: Vec<&str> = issue_types.iter().map(|it| it.name.as_str()).collect();
                JrError::UserError(format!(
                    "Issue type '{t}' not found for project {project_key}. Valid types: {}.",
                    valid.join(", "),
                ))
            })?;

        // Verified canonical shape (Atlassian Bulk Operations FAQ, 2026-06-01):
        //   editedFieldsInput key: camelCase "issueType"
        //   value: {"issueTypeId": "<id-string>"}
        // selectedActions element: lowercase "issuetype" (these INTENTIONALLY differ)
        // See `.factory/research/issue-331-issuetype-bulk-schema.md`.
        edited.insert("issueType".into(), json!({"issueTypeId": type_id}));
        selected_actions.push("issuetype".to_string());
    }

    if edited.is_empty() {
        bail!(
            "No fields specified to update. Use --summary, --type, --priority, --label, --team, \
             --points, --no-points, --parent, --no-parent, --description, or --description-stdin."
        );
    }

    let edited_fields = serde_json::Value::Object(edited);
    let task_id = client
        .bulk_edit_fields(keys, selected_actions, edited_fields)
        .await?;
    let progress = client
        .await_bulk_task(&task_id, resolve_bulk_await_timeout())
        .await?;

    render_bulk_edit_results(keys, &task_id, &progress, output_format)
}

/// Render bulk edit results to stdout/stderr and return the appropriate exit code.
///
/// - Table mode: print per-key success/error lines.
/// - JSON mode: `{"taskId":"...","results":[...]}` with optional `"key"` for single-key BC.
/// - Returns `Ok(())` if all succeeded; returns `Err(exit-1)` if any failed.
fn render_bulk_edit_results(
    keys: &[String],
    task_id: &str,
    progress: &crate::types::jira::bulk::BulkOperationProgress,
    output_format: &OutputFormat,
) -> Result<()> {
    let processed: std::collections::HashSet<&str> = progress
        .processed_accessible_issues
        .iter()
        .map(String::as_str)
        .collect();

    // Build per-key result list. Keys not in processed or failed are assumed
    // inaccessible/invalid (Atlassian may silently exclude them).
    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut any_failed = false;

    for key in keys {
        if let Some(err) = progress.failed_accessible_issues.get(key.as_str()) {
            let summary = err.summary();
            results.push(json!({
                "key": key,
                "status": "error",
                "error": summary,
            }));
            any_failed = true;
        } else if processed.contains(key.as_str()) {
            results.push(json!({
                "key": key,
                "status": "success",
            }));
        } else {
            // Not in processed and not in failed — inaccessible or invalid.
            results.push(json!({
                "key": key,
                "status": "inaccessible",
            }));
        }
    }

    // Also capture any failed keys that weren't in our input list
    // (shouldn't happen, but Atlassian may return unexpected keys).
    for (failed_key, err) in &progress.failed_accessible_issues {
        if !keys.iter().any(|k| k == failed_key) {
            results.push(json!({
                "key": failed_key,
                "status": "error",
                "error": err.summary(),
            }));
            any_failed = true;
        }
    }

    match output_format {
        OutputFormat::Json => {
            let mut payload = json!({
                "taskId": task_id,
                "results": results,
            });
            // Single-key backward-compat: include "key" at top level.
            if keys.len() == 1 {
                payload["key"] = json!(&keys[0]);
            }
            println!("{}", output::render_json(&payload)?);
        }
        OutputFormat::Table => {
            for entry in &results {
                let key = entry["key"].as_str().unwrap_or("?");
                match entry["status"].as_str().unwrap_or("?") {
                    "success" => output::print_success(&format!("Updated {key}")),
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
        // Return a non-Ok result that maps to exit code 1.
        bail!("One or more issues failed during bulk edit. See output above for details.");
    }

    Ok(())
}

/// Returns `true` when the error message indicates Jira rejected a parent-clear
/// operation because the issue is a subtask (subtasks are structurally bound to
/// a parent and cannot be un-parented without first converting to a regular issue).
///
/// Matches both common Atlassian error shapes (case-insensitive):
/// - `errors: { "parent": "<message containing 'subtask'>" }`
///   → extract_error_message yields "parent: Subtasks must have a parent."
/// - `errorMessages: ["... subtask ... parent ..."]`
fn is_subtask_parent_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("subtask") || (msg.contains("parent") && msg.contains("400"))
}

/// Context sentence prepended before `CROSS_HIERARCHY_HINT` on the `--no-parent` path only.
/// NOT emitted on the `edit --type` error path.
const NO_PARENT_CONTEXT_SENTENCE: &str = "Sub-tasks are structurally bound to a parent; clearing it requires converting the sub-task to a standard issue.";

/// Verbatim hint emitted when a cross-hierarchy `edit --type` 400 is detected,
/// and as the second line on the `--no-parent` clear-parent 400 path.
/// Shared constant — both call sites reference this exact text (BC-3.4.010 invariant 2).
const CROSS_HIERARCHY_HINT: &str = "The Jira Cloud REST API does not support changing the standard / sub-task hierarchy level via this endpoint (see JRACLOUD-27893). To convert it, open the issue in the Jira web UI and use the action menu to find the Convert option.";

/// Typo hint emitted on SameCategory and unresolvable-name sub-paths.
/// Verbatim from BC-3.4.011 (adversary-sealed, do not paraphrase).
const TYPO_HINT: &str = "Jira rejected the type change. If the type name is wrong, run `jr project types` to list valid types; the change may also be blocked by workflow or scheme constraints.";

/// Classification result for `is_cross_hierarchy_type_error`.
///
/// Derives `PartialEq + Debug` so `prop_assert_eq!` compiles in the proptest module.
#[derive(Debug, PartialEq)]
enum Classification {
    /// Source and target types are on opposite hierarchy levels (standard ↔ sub-task).
    CrossHierarchy,
    /// Source and target types are on the same hierarchy level; 400 is likely a typo or
    /// workflow/scheme constraint.
    SameCategory,
    /// One or both `subtask` flags could not be resolved; no confident classification.
    Indeterminate,
}

/// Pure classifier for cross-hierarchy `edit --type` 400 errors.
///
/// Rules (locale-independent, based solely on the `subtask` flag):
/// - Both flags `Some(a)` and `Some(b)` with `a != b` → `CrossHierarchy`
/// - Both flags `Some(a)` and `Some(b)` with `a == b` → `SameCategory`
/// - Either flag `None`                               → `Indeterminate`
///
/// The `err` argument MUST NOT influence the return value (BC-3.4.010 invariant 1 / P4).
/// It exists for potential future hint-composition use only.
fn is_cross_hierarchy_type_error(
    src_subtask: Option<bool>,
    tgt_subtask: Option<bool>,
    _err: &str,
) -> Classification {
    match (src_subtask, tgt_subtask) {
        (Some(a), Some(b)) if a != b => Classification::CrossHierarchy,
        (Some(_), Some(_)) => Classification::SameCategory,
        _ => Classification::Indeterminate,
    }
}

#[cfg(test)]
mod tests {
    use crate::error::JrError;
    use std::collections::BTreeSet;

    #[test]
    fn missing_project_returns_user_error() {
        let result: Option<String> = None;
        let err = result
            .ok_or_else(|| {
                JrError::UserError(
                    "Project key is required. Use --project or configure .jr.toml. \
                     Run \"jr project list\" to see available projects."
                        .into(),
                )
            })
            .unwrap_err();
        assert_eq!(err.exit_code(), 64);
        assert!(err.to_string().contains("Project key is required"));
    }

    /// Categorization meta-test for `IssueCommand::Edit` fields (issue #343).
    ///
    /// # Why this test exists
    ///
    /// The C-1 fix in issue #110 part 2 added a hand-rolled rejection list at
    /// `handle_edit` (`if effective_keys.len() > 1 { ... }`) that returns an
    /// error when multi-key bulk edit is invoked with flags that only the
    /// single-key path implements. The original silent-drop bug was: a user
    /// passes `--parent X` with multiple keys, the flag is silently ignored,
    /// no error fires, and the user thinks the edit succeeded.
    ///
    /// The C-1 list is hand-rolled and depends on the developer remembering
    /// to update it whenever they add a new field to `IssueCommand::Edit`. If
    /// they don't, the silent-drop bug returns. This test catches that drift
    /// at compile-and-test time.
    ///
    /// # Strategy
    ///
    /// Source-text inspection: read `src/cli/mod.rs` at compile time via
    /// `include_str!`, locate the `IssueCommand::Edit {` block, and extract
    /// every field name declared inside it. Compare the extracted set against
    /// three hand-maintained categorization sets:
    ///
    /// - **SELECTORS** — flags that select which issues to edit, not what
    ///   to change: `keys`, `jql`, `max`, `yes`, `dry_run`.
    /// - **BULK_SUPPORTED** — field flags that work on multi-key bulk path:
    ///   `summary`, `issue_type`, `priority`, `label`.
    /// - **REJECTED_IN_BULK** — field flags that only work on single-key
    ///   path; multi-key invocation must error: `parent`, `no_parent`,
    ///   `team`, `points`, `no_points`, `description`, `description_stdin`,
    ///   `markdown`.
    ///
    /// The test asserts:
    /// 1. The union of the three sets equals the extracted field set.
    /// 2. The three sets are pairwise disjoint (no field in two categories).
    /// 3. Every category contains at least one field (sanity check).
    ///
    /// # Failure modes this catches
    ///
    /// - A new flag is added to `Edit` but not categorized: union mismatch.
    /// - A flag is moved between categories without updating both lists:
    ///   intersection violation OR union mismatch.
    /// - A flag is renamed in `Edit` but not in the routing code: extracted
    ///   set differs from category sets.
    ///
    /// # Maintenance protocol
    ///
    /// When a future PR adds a flag to `IssueCommand::Edit`:
    /// 1. This test fails with a diff between expected and actual sets.
    /// 2. The PR author decides which category the new flag belongs in:
    ///    - Selector? Add to `SELECTORS` here.
    ///    - Bulk-safe field? Add to `BULK_SUPPORTED` AND wire the bulk path
    ///      in `handle_edit_bulk_fields` (or similar) to honor it.
    ///    - Single-key-only field? Add to `REJECTED_IN_BULK` AND extend the
    ///      C-1 rejection block in `handle_edit` to surface a clear error.
    /// 3. The test passes only when both the test list and the routing code
    ///    agree on the new flag's category.
    ///
    /// Closes audit-followup #343.
    #[test]
    fn test_343_every_edit_field_is_categorized() {
        let cli_source = include_str!("../mod.rs");

        let edit_fields = extract_edit_field_names(cli_source);

        // SELECTORS — flags that pick which issues to edit, not what changes.
        let selectors: BTreeSet<&str> = [
            "keys",    // positional issue keys (single or multi-key)
            "jql",     // JQL match set for bulk edit
            "max",     // upper bound on JQL match count
            "yes",     // skip interactive confirmation for large match sets
            "dry_run", // preview only, no HTTP mutations
        ]
        .into_iter()
        .collect();

        // BULK_SUPPORTED — field flags that work in multi-key bulk context.
        // These must be honored by both the single-key path AND the bulk path
        // (handle_edit_bulk_fields / handle_edit_bulk_labels).
        let bulk_supported: BTreeSet<&str> = [
            "summary",    // text summary update
            "issue_type", // issue type change (clap flag: --type)
            "priority",   // priority change
            "label",      // add/remove labels via labels coalesce
        ]
        .into_iter()
        .collect();

        // REJECTED_IN_BULK — field flags that ONLY the single-key path implements.
        // Multi-key invocation with any of these MUST return an error from the
        // C-1 rejection block in handle_edit (see lines ~426-465 of this file).
        // Adding to this set without extending the rejection block reintroduces
        // the silent-drop bug C-1 was meant to fix.
        let rejected_in_bulk: BTreeSet<&str> = [
            "parent",
            "no_parent",
            "team",
            "points",
            "no_points",
            "description",
            "description_stdin",
            "markdown",
            "field",     // --field NAME=VALUE (S-396): single-key only (BC-3.4.017 Gate A)
            "component", // --component add:/remove: (S-605-1): single-key only (BC-3.4.022)
        ]
        .into_iter()
        .collect();

        // --- ASSERTIONS ---

        // 1. Each category has at least one field (sanity check; protects
        //    against an empty hardcoded list slipping through unnoticed).
        assert!(!selectors.is_empty(), "SELECTORS must not be empty");
        assert!(
            !bulk_supported.is_empty(),
            "BULK_SUPPORTED must not be empty"
        );
        assert!(
            !rejected_in_bulk.is_empty(),
            "REJECTED_IN_BULK must not be empty"
        );

        // 2. Pairwise disjoint — no field categorized in more than one set.
        let s_b: BTreeSet<&&str> = selectors.intersection(&bulk_supported).collect();
        assert!(
            s_b.is_empty(),
            "SELECTORS and BULK_SUPPORTED overlap: {s_b:?} — every field belongs to exactly one category"
        );
        let s_r: BTreeSet<&&str> = selectors.intersection(&rejected_in_bulk).collect();
        assert!(
            s_r.is_empty(),
            "SELECTORS and REJECTED_IN_BULK overlap: {s_r:?} — every field belongs to exactly one category"
        );
        let b_r: BTreeSet<&&str> = bulk_supported.intersection(&rejected_in_bulk).collect();
        assert!(
            b_r.is_empty(),
            "BULK_SUPPORTED and REJECTED_IN_BULK overlap: {b_r:?} — every field belongs to exactly one category"
        );

        // 3. Union equals the extracted set — every Edit field is categorized
        //    AND no category lists a field that doesn't exist in Edit.
        let categorized: BTreeSet<String> = selectors
            .iter()
            .chain(bulk_supported.iter())
            .chain(rejected_in_bulk.iter())
            .map(|s| (*s).to_string())
            .collect();

        let missing_from_categories: Vec<&String> = edit_fields
            .iter()
            .filter(|f| !categorized.contains(*f))
            .collect();

        let spurious_in_categories: Vec<&String> = categorized
            .iter()
            .filter(|f| !edit_fields.contains(*f))
            .collect();

        assert!(
            missing_from_categories.is_empty() && spurious_in_categories.is_empty(),
            "\n\
             Edit field categorization is out of sync with IssueCommand::Edit in src/cli/mod.rs.\n\
             \n\
             Fields in Edit but NOT categorized (missing from SELECTORS/BULK_SUPPORTED/REJECTED_IN_BULK):\n  {:?}\n\
             \n\
             Fields in categories but NOT in Edit (spurious — remove from category list):\n  {:?}\n\
             \n\
             If you added a new flag to IssueCommand::Edit, add it to one of the three sets above.\n\
             If you removed a flag, remove it from its category set.\n\
             Closes audit-followup #343.",
            missing_from_categories,
            spurious_in_categories,
        );
    }

    // R2 pins for the formatting-tolerant closing-brace matcher
    // (extract_edit_field_names). These feed synthetic source text through the
    // extractor and confirm it copes with rustfmt-produced variants of the
    // closing `}` line.

    #[test]
    fn test_343_extractor_tolerates_no_trailing_comma() {
        // If `Edit` is the LAST variant in the enum, rustfmt may emit `}`
        // with no trailing comma. The matcher must still find it.
        let synthetic = "\
pub enum IssueCommand {
    Edit {
        keys: Vec<String>,
        summary: Option<String>,
    }
}
";
        let fields = extract_edit_field_names(synthetic);
        assert_eq!(
            fields,
            BTreeSet::from(["keys".to_string(), "summary".to_string()])
        );
    }

    #[test]
    fn test_343_extractor_tolerates_trailing_comment_on_closing() {
        // `},  // last variant` should still match.
        let synthetic = "\
pub enum IssueCommand {
    Edit {
        keys: Vec<String>,
        jql: Option<String>,
    },  // closing comment
}
";
        let fields = extract_edit_field_names(synthetic);
        assert_eq!(
            fields,
            BTreeSet::from(["keys".to_string(), "jql".to_string()])
        );
    }

    #[test]
    fn test_343_extractor_tolerates_trailing_whitespace_on_closing() {
        // `},   ` (closing with stray trailing spaces) — rustfmt usually strips
        // these but some editors may produce them; matcher must still cope.
        let synthetic =
            "pub enum IssueCommand {\n    Edit {\n        keys: Vec<String>,\n    },   \n}\n";
        let fields = extract_edit_field_names(synthetic);
        assert_eq!(fields, BTreeSet::from(["keys".to_string()]));
    }

    /// Helper: extract all field names declared inside the `IssueCommand::Edit {`
    /// variant in `src/cli/mod.rs`. Operates on the source text so it does not
    /// require any compile-time reflection or third-party derive macro.
    ///
    /// Strategy:
    /// 1. Locate the `Edit {` line (matched by `trim_start().starts_with("Edit {")`,
    ///    so the variant's own indent is irrelevant).
    /// 2. Walk forward until the matching closing brace via
    ///    `is_matching_closing_brace` — tolerant of rustfmt-equivalent shapes:
    ///    `}` followed by optional `,`, optional whitespace, and optional
    ///    line-comment, all at the same indent prefix as the opening line.
    ///    See the closure's inline comment for the exact rules.
    /// 3. Inside that range, treat any trimmed line of the form `<name>: <type>...`
    ///    (any indent — fields are detected by the `name:` shape, not by
    ///    column position) as a field declaration. Skip lines that start with
    ///    `#[` (attributes), `//` (line/doc comments), or are blank.
    ///
    /// Returns the extracted field names as a `BTreeSet<String>` so the
    /// iteration/`Debug` output order is deterministic — assertion failure
    /// messages produce stable, reviewable diffs across runs and machines.
    /// (`HashSet` would not satisfy this: its iteration order depends on the
    /// hash seed, which varies per process.)
    fn extract_edit_field_names(source: &str) -> BTreeSet<String> {
        let lines: Vec<&str> = source.lines().collect();

        let edit_start = lines
            .iter()
            .position(|l| l.trim_start().starts_with("Edit {"))
            .expect(
                "Could not locate `Edit {` in src/cli/mod.rs — has the variant been renamed?\n\
                 Update the extractor to match the new variant name.",
            );

        // The opening line is `    Edit {` (4-space indent for a clap subcommand
        // variant). The closing line begins with `}` at the SAME indent as the
        // opening line. Match tolerantly so the meta-test fails only on
        // semantic drift (the variant being renamed/removed/restructured), not
        // on benign rustfmt-produced formatting changes such as:
        //   - `}` followed by `,` and a comment: `    }, // comment`
        //   - last-variant `}` with no trailing comma (Rust allows this when
        //     `Edit` is the final variant in the enum)
        //   - trailing whitespace after the brace/comma
        //
        // Logic:
        //   1. Line must start with exactly `opening_indent_width` spaces
        //      followed by `}`. Field-internal braces sit at a deeper indent
        //      (more spaces than `opening_indent_width`), so the `}` is no
        //      longer at byte `closing_indent.len()` and `strip_prefix('}')`
        //      below rejects them. The opener's own indent isn't hard-coded
        //      — `opening_indent_width` is captured from the actual line.
        //   2. After the `}`, only allow: end-of-line, `,`, whitespace, or a
        //      line-comment (`//...`). Anything else means we hit a different
        //      construct and must keep scanning.
        let opening_indent_width = lines[edit_start].len() - lines[edit_start].trim_start().len();
        let closing_indent: String = " ".repeat(opening_indent_width);

        let is_matching_closing_brace = |line: &str| -> bool {
            // 1. Line must start with EXACTLY `closing_indent` spaces, and the
            //    next char must be `}`. A deeper-indented `}` (e.g., the closer
            //    of a nested struct inside a field) has more spaces after the
            //    prefix, so `strip_prefix('}')` fails and we reject below.
            if !line.starts_with(&closing_indent) {
                return false;
            }
            let rest = &line[closing_indent.len()..];
            let Some(after_brace) = rest.strip_prefix('}') else {
                return false;
            };
            // 2. After `}`, accept (in order): optional `,`, optional
            //    whitespace, optional `//`-comment, or end-of-line.
            let after_optional_comma = after_brace.strip_prefix(',').unwrap_or(after_brace);
            let trailing = after_optional_comma.trim_start();
            trailing.is_empty() || trailing.starts_with("//")
        };

        let edit_end = lines
            .iter()
            .enumerate()
            .skip(edit_start + 1)
            .find(|(_, l)| is_matching_closing_brace(l))
            .map(|(i, _)| i)
            .expect(
                "Could not locate matching closing brace for `Edit {{` block in src/cli/mod.rs.\n\
                 Expected a line starting with the same indent as `Edit {{`, containing `}}` \
                 optionally followed by `,` and optional whitespace/comment.\n\
                 The variant may have been renamed, removed, or significantly restructured \
                 — update the extractor to match the new shape.",
            );

        let mut fields = BTreeSet::new();

        for line in &lines[edit_start + 1..edit_end] {
            let trimmed = line.trim_start();
            // Skip attributes, doc comments, blank lines, and inline comments.
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#[") {
                continue;
            }
            // Match patterns like `name: Type,` or `name: Type<...>,`.
            // A field declaration line starts with an identifier followed by `:`.
            // We extract everything up to the first `:` and validate it as an
            // identifier.
            if let Some((ident, _rest)) = trimmed.split_once(':') {
                let ident = ident.trim();
                let is_valid_ident = !ident.is_empty()
                    && ident
                        .chars()
                        .next()
                        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
                    && ident.chars().all(|c| c == '_' || c.is_ascii_alphanumeric());
                if is_valid_ident {
                    fields.insert(ident.to_string());
                }
            }
        }

        assert!(
            !fields.is_empty(),
            "Field extraction returned an empty set for `IssueCommand::Edit` — \
             the extractor regex/parser likely no longer matches the variant's \
             formatting. Update extract_edit_field_names() to match the current source."
        );

        fields
    }

    // -------------------------------------------------------------------------
    // EC-3.4.017-14 — structural meta-test: --label conflict block completeness
    // -------------------------------------------------------------------------

    /// Meta-test: the `--label` conflict block in `handle_edit` (edit.rs) MUST
    /// enumerate every flag in `(BULK_SUPPORTED \ {"label"}) ∪ REJECTED_IN_BULK`.
    ///
    /// Strategy:
    /// - Read the source of this file via `include_str!("edit.rs")`.
    /// - Globally scan for every `conflicting.push("--<flag>")` literal (safe
    ///   because the variable name `conflicting` is reserved by the guard comment
    ///   at the `if !labels.is_empty()` block — see AC-014).
    /// - Build the expected set from the same constants used in
    ///   `test_343_every_edit_field_is_categorized`, applying the one non-mechanical
    ///   rename: `issue_type → "--type"` (the `#[arg(long = "type")]` override).
    /// - Assert set equality with a clear failure message.
    ///
    /// Failure modes caught:
    /// - Any `conflicting.push` line is deleted → extracted set loses a member → FAIL.
    /// - A new flag is added to BULK_SUPPORTED or REJECTED_IN_BULK without extending
    ///   the conflict block → expected set grows, extracted set does not → FAIL.
    ///
    /// Closes EC-3.4.017-14 (S-407).
    #[test]
    fn test_label_conflict_block_lists_every_relevant_flag() {
        let source = include_str!("edit.rs");

        // Extract every `conflicting.push("--<flag>")` literal from the entire file.
        // The guard comment at the `conflicting` variable declaration (AC-014) ensures
        // this name is ONLY used within the --label mutual-exclusion block, so a global
        // scan is unambiguous.
        let extracted: BTreeSet<String> = source
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                // Match lines of the form: conflicting.push("--<flag>");
                if let Some(rest) = trimmed.strip_prefix("conflicting.push(\"") {
                    if let Some(flag) = rest.strip_suffix("\");") {
                        if flag.starts_with("--") {
                            return Some(flag.to_string());
                        }
                    }
                }
                None
            })
            .collect();

        // Expected set: (BULK_SUPPORTED \ {"label"}) ∪ REJECTED_IN_BULK, mapped to
        // kebab-case CLI flag names. The one non-mechanical rename: issue_type → "--type"
        // (carries #[arg(long = "type")] in src/cli/mod.rs). All others: snake→kebab.
        let expected: BTreeSet<String> = [
            // BULK_SUPPORTED \ {"label"}  (label is the outer guard, not a pushed entry)
            "--summary",  // summary
            "--type",     // issue_type — explicit long = "type" override
            "--priority", // priority
            // REJECTED_IN_BULK
            "--parent",
            "--no-parent", // no_parent → no-parent
            "--team",
            "--points",
            "--no-points", // no_points → no-points
            "--description",
            "--description-stdin", // description_stdin → description-stdin
            "--markdown",
            "--field",
            "--component", // component (S-605-1): BC-3.4.020 amendment, AC-015
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        assert_eq!(
            extracted,
            expected,
            "\n\
             --label conflict block is out of sync with (BULK_SUPPORTED \\ {{\"label\"}}) ∪ REJECTED_IN_BULK.\n\
             \n\
             Flags in expected but NOT in conflict block (missing push lines):\n  {:?}\n\
             \n\
             Flags in conflict block but NOT in expected (spurious push lines):\n  {:?}\n\
             \n\
             If you added a new Edit flag, extend the --label conflict block in handle_edit\n\
             and update the expected set in this test. If you removed a flag, remove it from both.\n\
             Closes EC-3.4.017-14.",
            expected.difference(&extracted).collect::<Vec<_>>(),
            extracted.difference(&expected).collect::<Vec<_>>(),
        );
    }

    /// R2 pin: the `conflicting.push` extractor correctly identifies exactly 13 flags
    /// from the current source of edit.rs. This test pins the extractor against the
    /// actual file — if the extraction logic regresses (e.g., formatting drift changes
    /// the pattern), this fails distinctly from the set-equality meta-test.
    ///
    /// The 13 expected members are:
    ///   --field, --summary, --priority, --type, --team, --points, --no-points,
    ///   --parent, --no-parent, --description, --description-stdin, --markdown,
    ///   --component
    ///
    /// Closes EC-3.4.017-14 (R2 pin, S-407 AC-013). Extended to 13 by S-605-1
    /// (BC-3.4.020 amendment, AC-015).
    #[test]
    fn test_label_conflict_block_extractor_pin_13_members() {
        let source = include_str!("edit.rs");

        let extracted: BTreeSet<String> = source
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("conflicting.push(\"") {
                    if let Some(flag) = rest.strip_suffix("\");") {
                        if flag.starts_with("--") {
                            return Some(flag.to_string());
                        }
                    }
                }
                None
            })
            .collect();

        // The 13 current --label conflict block entries (as of S-605-1).
        // If the count changes, update both this test AND the meta-test above.
        let expected_13: BTreeSet<String> = [
            "--field",
            "--summary",
            "--priority",
            "--type",
            "--team",
            "--points",
            "--no-points",
            "--parent",
            "--no-parent",
            "--description",
            "--description-stdin",
            "--markdown",
            "--component",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        assert_eq!(
            extracted.len(),
            13,
            "R2 pin: expected exactly 13 conflicting.push entries in edit.rs, found {}.\n\
             Current extracted set: {:?}",
            extracted.len(),
            extracted,
        );

        assert_eq!(
            extracted, expected_13,
            "R2 pin: extracted flag set does not match the 13 expected members.\n\
             Extracted: {:?}\nExpected: {:?}",
            extracted, expected_13,
        );
    }
}

#[cfg(test)]
mod build_labels_proptests {
    use super::build_labels_edited_fields;
    use proptest::prelude::*;

    proptest! {
        /// Invariants for `build_labels_edited_fields` (verified labelsFields schema, issue #446).
        ///
        /// Schema: `editedFieldsInput` is `{"labelsFields": [...]}` where each element has:
        ///   - `fieldId`: `"labels"`
        ///   - `bulkEditMultiSelectFieldOption`: `"ADD"` or `"REMOVE"`
        ///   - `labels`: array of `{"name": <string>}` objects
        ///
        /// ADD entries appear iff `adds` is non-empty; REMOVE entries iff `removes` is non-empty.
        /// Both present → two elements (ADD first, REMOVE second).
        ///
        /// `prop_assume!` filters out the empty/empty case because the caller
        /// (`handle_edit_bulk_labels`) bails on `adds.is_empty() && removes.is_empty()`.
        ///
        /// Source: https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/
        #[test]
        fn build_labels_edited_fields_invariants(
            adds in proptest::collection::vec("[a-z]{1,10}", 0..5),
            removes in proptest::collection::vec("[a-z]{1,10}", 0..5),
        ) {
            prop_assume!(!adds.is_empty() || !removes.is_empty());

            let result = build_labels_edited_fields(&adds, &removes);

            // Invariant 0: top-level value is a JSON object with exactly one key ("labelsFields").
            let obj = result.as_object().expect("top-level value MUST be a JSON object");
            prop_assert_eq!(obj.len(), 1, "top-level object MUST have exactly one key ('labelsFields')");

            // Invariant 1: top-level "labelsFields" key is always present and is an array.
            let labels_fields = result
                .get("labelsFields")
                .and_then(|v| v.as_array())
                .expect("'labelsFields' MUST be a JSON array");

            // Expected number of elements: 1 if only adds or only removes; 2 if both.
            let expected_len = match (adds.is_empty(), removes.is_empty()) {
                (false, false) => 2,
                _ => 1,
            };
            prop_assert_eq!(
                labels_fields.len(),
                expected_len,
                "labelsFields MUST have {} element(s)",
                expected_len
            );

            // Helper: extract (bulkEditMultiSelectFieldOption, label names) from one element.
            let extract_elem = |elem: &serde_json::Value| -> (String, Vec<String>) {
                let e = elem.as_object().expect("labelsFields element MUST be an object");
                // fieldId MUST be "labels"
                assert_eq!(
                    e.get("fieldId").and_then(|v| v.as_str()),
                    Some("labels"),
                    "labelsFields[].fieldId MUST equal \"labels\""
                );
                let action = e
                    .get("bulkEditMultiSelectFieldOption")
                    .and_then(|v| v.as_str())
                    .expect("labelsFields element MUST have bulkEditMultiSelectFieldOption: String")
                    .to_string();
                let inner = e
                    .get("labels")
                    .and_then(|v| v.as_array())
                    .expect("labelsFields element MUST have labels: Array");
                let names: Vec<String> = inner
                    .iter()
                    .map(|item| {
                        let item_obj = item.as_object().expect("label item MUST be an object");
                        assert_eq!(item_obj.len(), 1, "label item MUST have exactly 1 key (name)");
                        item_obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .expect("label item MUST have name: String")
                            .to_string()
                    })
                    .collect();
                (action, names)
            };

            match (adds.is_empty(), removes.is_empty()) {
                // Both present: ADD at index 0, REMOVE at index 1.
                (false, false) => {
                    let (a0_action, a0_names) = extract_elem(&labels_fields[0]);
                    let (a1_action, a1_names) = extract_elem(&labels_fields[1]);
                    prop_assert_eq!(a0_action, "ADD",    "labelsFields[0] MUST be ADD");
                    prop_assert_eq!(a1_action, "REMOVE", "labelsFields[1] MUST be REMOVE");
                    prop_assert_eq!(a0_names, adds.clone(),    "ADD names MUST match input");
                    prop_assert_eq!(a1_names, removes.clone(), "REMOVE names MUST match input");
                }
                // ADD only.
                (false, true) => {
                    let (action, names) = extract_elem(&labels_fields[0]);
                    prop_assert_eq!(action, "ADD", "single-ADD MUST set bulkEditMultiSelectFieldOption=ADD");
                    prop_assert_eq!(names, adds.clone(), "ADD names MUST match input");
                }
                // REMOVE only.
                (true, false) => {
                    let (action, names) = extract_elem(&labels_fields[0]);
                    prop_assert_eq!(action, "REMOVE", "single-REMOVE MUST set bulkEditMultiSelectFieldOption=REMOVE");
                    prop_assert_eq!(names, removes.clone(), "REMOVE names MUST match input");
                }
                // Both empty: filtered by prop_assume!; unreachable.
                (true, true) => unreachable!("filtered by prop_assume! above"),
            }
        }
    }
}

/// Proptest suite for `is_cross_hierarchy_type_error` (AC-7, BC-3.4.010 invariant 1,
/// BC-3.4.011 invariants 1–3, verification-delta-388.md §2 P1–P4).
///
/// Mirrors the `build_labels_proptests` / `parse_field_kv_proptests` pattern.
/// NOT added to the existing `mod tests` block to avoid name collisions.
#[cfg(test)]
mod is_cross_hierarchy_type_error_proptests {
    use super::{Classification, is_cross_hierarchy_type_error};
    use proptest::prelude::*;

    fn opt_bool() -> impl Strategy<Value = Option<bool>> {
        prop_oneof![Just(None), Just(Some(true)), Just(Some(false))]
    }

    proptest! {
        #[test]
        fn prop_cross_hierarchy_decided_by_subtask_flag_mismatch(
            src in opt_bool(),
            tgt in opt_bool(),
            // Arbitrary message; includes the locale-fragile substring with
            // non-zero probability so P4 actively exercises the no-influence claim.
            err in prop_oneof![
                ".*",
                Just("issue type selected is invalid".to_string()),
                Just(String::new()),
            ],
        ) {
            let result = is_cross_hierarchy_type_error(src, tgt, &err);

            match (src, tgt) {
                (Some(a), Some(b)) if a != b => {
                    prop_assert_eq!(result, Classification::CrossHierarchy);  // P1
                }
                (Some(a), Some(b)) => {
                    let _ = (a, b);
                    prop_assert_eq!(result, Classification::SameCategory);    // P2
                }
                _ => {
                    prop_assert_eq!(result, Classification::Indeterminate);   // P3
                }
            }

            // P4: err must not change the verdict — re-run with a fixed
            // contrasting message and assert equality.
            let baseline = is_cross_hierarchy_type_error(src, tgt, "");
            prop_assert_eq!(
                is_cross_hierarchy_type_error(src, tgt, &err),
                baseline,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// AC-006 (BC-3.4.018 invariant 4): project key extraction unit tests.
// `project_key_from_issue_key` is defined above and used by the BC-3.4.019
// `--type` cross-project guard, the BC-3.4.023 `--component` bulk-edit
// cross-project guard (S-605-2), and the dry-run preview path.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod test_project_key_extraction {
    use super::project_key_from_issue_key;

    /// `FOO-1` → project key `"FOO"`.
    #[test]
    fn test_project_key_from_issue_key_simple() {
        assert_eq!(project_key_from_issue_key("FOO-1"), "FOO");
    }

    /// `PROJ2-100` → project key `"PROJ2"` (multi-char project key with digit, splits on LAST hyphen).
    #[test]
    fn test_project_key_from_issue_key_multi_char() {
        assert_eq!(project_key_from_issue_key("PROJ2-100"), "PROJ2");
    }

    /// `FOO-2` → project key `"FOO"` (same as FOO-1 — same-project check).
    #[test]
    fn test_project_key_from_issue_key_same_project_second_key() {
        assert_eq!(project_key_from_issue_key("FOO-2"), "FOO");
    }

    /// Two same-project keys extract the same project key.
    #[test]
    fn test_project_key_extraction_same_project_no_cross_project() {
        let k1 = project_key_from_issue_key("FOO-1");
        let k2 = project_key_from_issue_key("FOO-2");
        assert_eq!(
            k1, k2,
            "Same-project keys must extract the same project key"
        );
    }

    /// Two different-project keys extract different project keys.
    #[test]
    fn test_project_key_extraction_different_projects() {
        let k1 = project_key_from_issue_key("FOO-1");
        let k2 = project_key_from_issue_key("BAR-2");
        assert_ne!(
            k1, k2,
            "Different-project keys must extract different project keys"
        );
    }

    // --- Edge cases pinning BC-3.4.018 invariant 4 fail-safe behavior ---

    /// No hyphen: the whole string is returned (fail-safe — treats the input as its
    /// own project key). Real Jira keys always contain a hyphen, so this is a
    /// defensive no-panic contract.
    #[test]
    fn test_project_key_from_issue_key_no_hyphen() {
        assert_eq!(project_key_from_issue_key("FOO"), "FOO");
    }

    /// Trailing hyphen: `rfind('-')` returns the last position (the trailing hyphen),
    /// so everything before it is returned — `"FOO-"` → `"FOO"`. This pins that a
    /// malformed key doesn't panic and produces a stable (if semantically odd) result.
    #[test]
    fn test_project_key_from_issue_key_trailing_hyphen() {
        assert_eq!(project_key_from_issue_key("FOO-"), "FOO");
    }

    /// Empty string: no hyphen → returns `""`. Pins the no-panic contract.
    #[test]
    fn test_project_key_from_issue_key_empty_string() {
        assert_eq!(project_key_from_issue_key(""), "");
    }
}
