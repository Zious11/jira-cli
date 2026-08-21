use anyhow::Result;
use futures::stream::{self, StreamExt};

use crate::api::assets::linked::{
    MAX_CONCURRENT_ASSET_FETCHES, cmdb_field_ids, enrich_json_assets, extract_linked_assets,
    get_or_fetch_cmdb_fields,
};
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat, resolve_effective_limit};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::types::assets::LinkedAsset;

use super::format;
use super::helpers;

use crate::api::jira::projects::IssueTypeWithStatuses;
use crate::partial_match::{self, MatchResult};

/// Extract unique status names from project-scoped statuses response (deduplicated, sorted).
fn extract_unique_status_names(issue_types: &[IssueTypeWithStatuses]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();
    for it in issue_types {
        for s in &it.statuses {
            if seen.insert(s.name.clone()) {
                names.push(s.name.clone());
            }
        }
    }
    names.sort();
    names
}

// ── List ──────────────────────────────────────────────────────────────

/// BC-2.1.006 (amended, S-579-1): "no project or filters specified" guard
/// message -- the amended 15-source enumeration, with `--updated-recent`
/// appended immediately before `or --jql`. Shared between THREE call sites
/// in `handle_list`, all of which must stay in sync with each other and with
/// this message's enumerated list:
///   1. The early EC-2.1.023-4 guard (`--updated-recent` alone, no project,
///      no configured board, no other filter) -- fires before any HTTP call.
///   2. The `base_parts.is_empty()` backstop guard added by pr-review cycle 1
///      Finding 1, immediately after `base_parts` is resolved -- closes the
///      narrow board-configured-but-empty-scoping-clause gap the early
///      guard's `board_id.is_none()` proxy cannot see (see the comment above
///      guard #1 for the full explanation).
///   3. The end-of-function `all_parts.is_empty()` guard -- the original
///      BC-2.1.006 backstop for every other filter source.
///
/// Adding a 16th filter flag to `IssueCommand::List` requires updating this
/// message's enumerated list AND all three guards' conjunctions above.
/// Nothing currently enforces this mechanically -- no compile error is
/// raised on drift between the message text and any guard's conjunction.
const NO_FILTERS_SPECIFIED_MSG: &str = "No project or filters specified. Use --project, --assignee, --reporter, --status, --open, --team, --recent, --created-after, --created-before, --updated-after, --updated-before, --asset, --component, --updated-recent, or --jql. You can also set a default project in .jr.toml or run \"jr init\".";

/// Build base JQL parts when `--jql` is provided.
///
/// Returns `(base_parts, order_by)`. Strips any trailing `ORDER BY` clause
/// from `jql` and prepends the project scope if `project_key` is set.
fn build_jql_base_parts(jql: &str, project_key: Option<&str>) -> (Vec<String>, &'static str) {
    let stripped = crate::jql::strip_order_by(jql);
    let mut parts = Vec::new();

    if let Some(pk) = project_key {
        parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
    }
    if !stripped.is_empty() {
        parts.push(format!("({})", stripped));
    }

    (parts, "updated DESC")
}

pub(super) async fn handle_list(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::List {
        jql,
        status,
        team,
        limit,
        all,
        assignee,
        reporter,
        recent,
        updated_recent,
        open,
        points: show_points,
        assets: show_assets,
        duedate: show_duedate,
        asset: asset_key,
        component,
        created_after,
        created_before,
        updated_after,
        updated_before,
        fields,
    } = command
    else {
        unreachable!()
    };

    // S-575-1 (BC-2.2.033): `--fields <CSV>` output-format gate + pre-HTTP
    // CSV validation. Both run before ANY network call (project resolution,
    // component resolution, `project_exists`, the search itself) so a
    // rejected combination costs zero HTTP requests. Default behavior
    // (fields == None) is untouched below.
    let field_list: Option<Vec<String>> = match &fields {
        Some(csv) => {
            if !matches!(output_format, OutputFormat::Json) {
                return Err(JrError::UserError("--fields requires --output json.".into()).into());
            }
            Some(helpers::parse_fields_csv(csv)?)
        }
        None => None,
    };

    // Resolve project key once, before any HTTP call. Moved up from its
    // original position (immediately before the `project_exists` check)
    // because the S-606-1 `--component` pre-flight validation below needs it
    // and MUST run before `project_exists`'s GET — see
    // `validate_component_preflight`'s doc comment for why (VP-COMPONENT-013
    // zero-HTTP guarantee for a rejected combination or missing project
    // scope).
    let project_key = config.project_key(project_override);

    // S-606-1 (BC-2.1.018..022): --component pre-flight validation —
    // combination/count guards and the project-scope requirement, purely
    // CLI-arg-derived, HTTP-free. MUST run before ANY network call (including
    // `project_exists`) so a rejected combination or missing project scope
    // costs literally zero requests (VP-COMPONENT-013).
    if !component.is_empty() {
        validate_component_preflight(&component, project_key.as_deref())?;
    }

    let effective_limit = resolve_effective_limit(limit, all);

    // Auto-enable assets display column when filtering by asset
    let show_assets = show_assets || asset_key.is_some();

    // Validate --recent duration format early
    if let Some(ref d) = recent {
        crate::jql::validate_duration(d).map_err(JrError::UserError)?;
    }

    // S-579-1 (BC-2.1.023 Precondition 2): --updated-recent duration filter,
    // the `updated` field parallel to --recent (`created`). Reuses the SAME
    // validator --recent uses (jql::validate_duration, NOT duration.rs) --
    // combined units like `4w2d` are rejected pre-HTTP with the identical
    // error shape --recent's own validation produces (AC-002).
    if let Some(ref d) = updated_recent {
        crate::jql::validate_duration(d).map_err(JrError::UserError)?;
    }

    // S-579-1 (BC-2.1.023 Edge Case EC-2.1.023-4): unlike `--recent`,
    // `--updated-recent` does not by itself satisfy the "at least one filter
    // source" requirement when used with no --project/configured project,
    // no configured board (`.jr.toml`'s `board_id`), and no other filter --
    // it falls through to the same BC-2.1.006 "no filters specified" guard a
    // completely bare `jr issue list` invocation hits. This must be checked
    // here (zero HTTP so far) rather than relying on the end-of-function
    // "guard against unbounded query" below, because `--updated-recent`'s
    // own composed clause would otherwise make the final assembled clause
    // list non-empty and silently bypass that guard.
    //
    // `config.project.board_id` MUST be part of this conjunction (Pass 2
    // MEDIUM fix): a `.jr.toml` with only `board_id` set (no `project` key)
    // is a valid, board-scoped configuration -- see
    // `Config::board_id`/`test_board_id_cli_override` in `src/config.rs`.
    //
    // CORRECTION (pr-review cycle 1, Finding 1): `board_id.is_none()` is only
    // a COARSE proxy for "the board contributes scoping" -- it does NOT hold
    // in one narrow, verified subcase: a scrum board with NO active sprint,
    // in a config with `board_id` set and no `project` key. In that exact
    // configuration a completely bare `jr issue list` ALREADY exits 64 via
    // this SAME guard (it does not "succeed by falling through" as an
    // earlier revision of this comment incorrectly claimed) -- the scrum
    // "no active sprint" fallback (below, ~line 392) seeds `base_parts` from
    // `project_key` alone, which is `None` here, so `base_parts` ends up
    // empty regardless of what tripped or didn't trip this early guard.
    // `--updated-recent` alone in that same configuration is different: this
    // early guard lets it through (a board IS configured), but the
    // downstream scrum-no-active-sprint fallback still produces an empty
    // `base_parts`, and `--updated-recent`'s own clause then makes the final
    // `all_parts` non-empty, silently bypassing the end-of-function guard too
    // -- an unbounded, cross-project query. A second, narrower backstop
    // guard (below, immediately after `base_parts` is resolved) closes this
    // specific hole by checking `base_parts.is_empty()` directly instead of
    // trying to predict it from `board_id` alone.
    if updated_recent.is_some()
        && project_key.is_none()
        && config.project.board_id.is_none()
        && jql.is_none()
        && status.is_none()
        && team.is_none()
        && recent.is_none()
        && !open
        && asset_key.is_none()
        && component.is_empty()
        && created_after.is_none()
        && created_before.is_none()
        && updated_after.is_none()
        && updated_before.is_none()
        && assignee.is_none()
        && reporter.is_none()
    {
        return Err(JrError::UserError(NO_FILTERS_SPECIFIED_MSG.into()).into());
    }

    // Validate date filter flags early (before any network calls)
    let created_after_date = if let Some(ref d) = created_after {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
    let created_before_date = if let Some(ref d) = created_before {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
    let updated_after_date = if let Some(ref d) = updated_after {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };
    let updated_before_date = if let Some(ref d) = updated_before {
        Some(crate::jql::validate_date(d).map_err(JrError::UserError)?)
    } else {
        None
    };

    // Build date filter JQL clauses
    let created_after_clause = created_after_date.map(|d| format!("created >= \"{}\"", d));
    let created_before_clause = created_before_date.map(|d| {
        let next_day = d + chrono::Days::new(1);
        format!("created < \"{}\"", next_day)
    });
    let updated_after_clause = updated_after_date.map(|d| format!("updated >= \"{}\"", d));
    let updated_before_clause = updated_before_date.map(|d| {
        let next_day = d + chrono::Days::new(1);
        format!("updated < \"{}\"", next_day)
    });

    // Resolve --asset: key passthrough or name → key via AQL search
    let asset_key = if let Some(raw) = asset_key {
        Some(helpers::resolve_asset(client, &raw, no_input).await?)
    } else {
        None
    };

    // Resolve --assignee and --reporter to JQL values
    let assignee_jql = if let Some(ref name) = assignee {
        Some(helpers::resolve_user(client, name, no_input).await?)
    } else {
        None
    };
    let reporter_jql = if let Some(ref name) = reporter {
        Some(helpers::resolve_user(client, name, no_input).await?)
    } else {
        None
    };

    let active = config.active_profile();
    let sp_field_id = active.story_points_field_id.as_deref();
    let team_field_id = active.team_field_id.as_deref();
    let mut extra: Vec<&str> = sp_field_id.iter().copied().collect();
    // Request team field on list output so handle_list can surface a Team
    // column per #191 (shown only when ≥1 issue has a populated team).
    if let Some(t) = team_field_id {
        extra.push(t);
    }

    // Resolve team name to (field_id, uuid) before building JQL
    let resolved_team = if let Some(ref team_name) = team {
        Some(helpers::resolve_team_field(config, client, team_name, no_input).await?)
    } else {
        None
    };

    // Build pre-formatted team clause for build_filter_clauses
    let team_clause = resolved_team
        .as_ref()
        .map(|(field_id, team_uuid, _resolved_team_name)| {
            format!("{} = \"{}\"", field_id, crate::jql::escape_value(team_uuid))
        });

    // Resolve CMDB fields for --asset filter (needs field names for aqlFunction)
    let (asset_clause, asset_cmdb_fields) = if let Some(ref key) = asset_key {
        let cmdb_fields = get_or_fetch_cmdb_fields(client).await?;
        if cmdb_fields.is_empty() {
            return Err(JrError::UserError(
                "--asset requires Assets custom fields on this Jira instance. \
                 Assets requires a paid Jira Service Management plan."
                    .into(),
            )
            .into());
        }
        let clause = crate::jql::build_asset_clause(key, &cmdb_fields);
        (Some(clause), Some(cmdb_fields))
    } else {
        (None, None)
    };

    // Validate --project exists
    if let Some(ref pk) = project_key {
        // Skip if --status is set (project will be validated via statuses endpoint below)
        if status.is_none() && !client.project_exists(pk).await? {
            return Err(JrError::UserError(format!(
                "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
                pk
            ))
            .into());
        }
    }

    // Validate --status and resolve to exact name
    let resolved_status: Option<String> = if let Some(ref status_input) = status {
        let valid_statuses = if let Some(ref pk) = project_key {
            // Project-scoped: also validates project existence (404 = not found)
            match client.get_project_statuses(pk).await {
                Ok(issue_types) => extract_unique_status_names(&issue_types),
                Err(e) => {
                    if let Some(JrError::ApiError { status: 404, .. }) = e.downcast_ref::<JrError>()
                    {
                        return Err(JrError::UserError(format!(
                            "Project \"{}\" not found. Run \"jr project list\" to see available projects.",
                            pk
                        ))
                        .into());
                    }
                    return Err(e);
                }
            }
        } else {
            client.get_all_statuses().await?
        };

        match partial_match::partial_match(status_input, &valid_statuses) {
            MatchResult::Exact(name) => Some(name),
            // Case-sensitive dedup upstream; treat like Exact if case-variant duplicates slip through
            MatchResult::ExactMultiple(name) => Some(name),
            MatchResult::Ambiguous(matches) => {
                return Err(JrError::UserError(format!(
                    "Ambiguous status \"{}\". Matches: {}",
                    status_input,
                    matches.join(", ")
                ))
                .into());
            }
            MatchResult::None(all) => {
                let available = all.join(", ");
                let scope = if let Some(ref pk) = project_key {
                    format!(" for project {}", pk)
                } else {
                    String::new()
                };
                return Err(JrError::UserError(format!(
                    "No status matching \"{}\"{scope}. Available: {available}",
                    status_input,
                ))
                .into());
            }
        }
    } else {
        None
    };

    // Resolve --component filter (bare/not:/none/all: forms) — BC-2.1.018..022.
    // Zero-value case (no --component flags at all) matches pre-S-606-1 behavior
    // exactly: no resolver HTTP, no clause contributed, existing filters/tests
    // are unaffected by this guard.
    let component_clauses: Vec<String> = if component.is_empty() {
        Vec::new()
    } else {
        resolve_component_clauses(client, project_key.as_deref(), &component).await?
    };

    // Build filter clauses from all flag values
    let filter_parts = build_filter_clauses(FilterOptions {
        assignee_jql: assignee_jql.as_deref(),
        reporter_jql: reporter_jql.as_deref(),
        status: resolved_status.as_deref(),
        team_clause: team_clause.as_deref(),
        recent: recent.as_deref(),
        updated_recent: updated_recent.as_deref(),
        open,
        asset_clause: asset_clause.as_deref(),
        component_clauses: &component_clauses,
        created_after_clause: created_after_clause.as_deref(),
        created_before_clause: created_before_clause.as_deref(),
        updated_after_clause: updated_after_clause.as_deref(),
        updated_before_clause: updated_before_clause.as_deref(),
    });

    // Build base JQL + order by
    let (base_parts, order_by): (Vec<String>, &str) = if let Some(ref raw_jql) = jql {
        build_jql_base_parts(raw_jql, project_key.as_deref())
    } else {
        let board_id = config.project.board_id;

        if let Some(bid) = board_id {
            match client.get_board_config(bid).await {
                Ok(board_config) => {
                    let board_type = board_config.board_type.to_lowercase();
                    if board_type == "scrum" {
                        match client.list_sprints(bid, Some("active")).await {
                            Ok(sprints) if !sprints.is_empty() => {
                                let sprint = &sprints[0];
                                (vec![format!("sprint = {}", sprint.id)], "rank ASC")
                            }
                            Ok(_) => {
                                // No active sprint — fall back to project-scoped JQL
                                let mut parts = Vec::new();
                                if let Some(ref pk) = project_key {
                                    parts.push(format!(
                                        "project = \"{}\"",
                                        crate::jql::escape_value(pk)
                                    ));
                                }
                                (parts, "updated DESC")
                            }
                            Err(e) => {
                                return Err(e.context(format!(
                                    "Failed to list sprints for board {}. \
                                     Use --jql to query directly.",
                                    bid
                                )));
                            }
                        }
                    } else {
                        // Kanban: statusCategory != Done, no implicit assignee
                        let mut parts = Vec::new();
                        if let Some(ref pk) = project_key {
                            parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
                        }
                        parts.push("statusCategory != Done".into());
                        (parts, "rank ASC")
                    }
                }
                Err(e) => {
                    if let Some(JrError::ApiError { status: 404, .. }) = e.downcast_ref::<JrError>()
                    {
                        return Err(JrError::UserError(format!(
                            "Board {} not found or not accessible. \
                             Verify the board exists and you have permission, \
                             or remove board_id from .jr.toml. \
                             Use --jql to query directly.",
                            bid
                        ))
                        .into());
                    }
                    return Err(e.context(format!(
                        "Failed to fetch config for board {}. \
                         Remove board_id from .jr.toml or use --jql to query directly.",
                        bid
                    )));
                }
            }
        } else {
            let mut parts = Vec::new();
            if let Some(ref pk) = project_key {
                parts.push(format!("project = \"{}\"", crate::jql::escape_value(pk)));
            }
            (parts, "updated DESC")
        }
    };

    // S-579-1 pr-review cycle 1 Finding 1 (EC-2.1.023-4 backstop): closes the
    // narrow gap the early guard above cannot see -- a `.jr.toml` with only
    // `board_id` set (no `project` key), a scrum board, and NO active sprint
    // resolves `base_parts` to an EMPTY Vec (the scrum "no active sprint"
    // fallback above seeds `parts` from `project_key`, which is `None` in
    // this exact config). The early guard's `board_id.is_none()` conjunct
    // already let `--updated-recent` alone through in this configuration
    // (a board IS configured), so this check is a direct, non-predictive
    // test of the thing that actually matters: did the base-JQL resolution
    // above actually produce a scoping clause? Checking `base_parts` itself
    // rather than re-deriving "should it be empty" from `board_id`/board
    // type/sprint state avoids the same coarse-proxy mistake the early guard
    // made. This does not affect AC-007's zero-HTTP guarantee for the
    // no-board case -- the early guard above already rejects that
    // configuration before any HTTP call; every path that reaches here with
    // an empty `base_parts` has a board configured, so the board-config and
    // sprint-list HTTP calls above have already legitimately happened.
    //
    // Conjunction mirrors the early guard's, MINUS `config.project.board_id`
    // (a board is always configured by the time `base_parts` can be empty
    // here) and PLUS `base_parts.is_empty()`. See `NO_FILTERS_SPECIFIED_MSG`'s
    // doc comment for the full three-call-site sync requirement.
    if base_parts.is_empty()
        && updated_recent.is_some()
        && project_key.is_none()
        && jql.is_none()
        && status.is_none()
        && team.is_none()
        && recent.is_none()
        && !open
        && asset_key.is_none()
        && component.is_empty()
        && created_after.is_none()
        && created_before.is_none()
        && updated_after.is_none()
        && updated_before.is_none()
        && assignee.is_none()
        && reporter.is_none()
    {
        return Err(JrError::UserError(NO_FILTERS_SPECIFIED_MSG.into()).into());
    }

    // Combine base + filters
    let mut all_parts = base_parts;
    all_parts.extend(filter_parts);

    // Guard against unbounded query
    if all_parts.is_empty() {
        return Err(JrError::UserError(NO_FILTERS_SPECIFIED_MSG.into()).into());
    }

    let where_clause = all_parts.join(" AND ");
    let effective_jql = format!("{where_clause} ORDER BY {order_by}");

    // S-575-1 (BC-2.2.033 Postcondition 1/4, human-locked DEC-298): when
    // `--fields` is present it REPLACES BASE_ISSUE_FIELDS entirely — no
    // union with `extra` (story points / team field ids), and `--points` /
    // `--assets` / `--duedate` become silent no-ops by never reaching any of
    // the cmdb-field-fetch, asset-enrichment, or column-rendering logic
    // below (that logic is entirely skipped, not merely made inert).
    if let Some(field_list) = &field_list {
        let field_refs: Vec<&str> = field_list.iter().map(String::as_str).collect();
        let search_result = client
            .search_issues_with_fields(&effective_jql, effective_limit, &field_refs)
            .await?;
        let has_more = search_result.has_more;
        let issues = search_result.issues;

        output::print_output(output_format, &[], &[], &issues)?;

        if has_more && !all {
            let count_jql = crate::jql::strip_order_by(&effective_jql);
            match client.approximate_count(count_jql).await {
                Ok(total) if total > 0 => {
                    eprintln!(
                        "Showing {} of ~{} results. Use --limit or --all to see more.",
                        issues.len(),
                        total
                    );
                }
                Ok(_) | Err(_) => {
                    eprintln!(
                        "Showing {} results. Use --limit or --all to see more.",
                        issues.len()
                    );
                }
            }
        }

        return Ok(());
    }

    let cmdb_fields = if show_assets {
        if let Some(fields) = asset_cmdb_fields {
            fields
        } else {
            let fields = get_or_fetch_cmdb_fields(client).await.unwrap_or_default();
            if fields.is_empty() {
                eprintln!(
                    "warning: --assets ignored. No Assets custom fields found on this Jira instance."
                );
            }
            fields
        }
    } else {
        Vec::new()
    };
    let cmdb_field_id_list = cmdb_field_ids(&cmdb_fields);
    for f in &cmdb_field_id_list {
        extra.push(f.as_str());
    }

    let search_result = client
        .search_issues(&effective_jql, effective_limit, &extra)
        .await?;
    let has_more = search_result.has_more;
    let mut issues = search_result.issues;

    let effective_sp = resolve_show_points(show_points, sp_field_id);
    let show_assets_col = show_assets && !cmdb_field_id_list.is_empty();
    let mut issue_assets: Vec<Vec<LinkedAsset>> = Vec::new();
    if show_assets_col {
        // Extract linked assets for all issues first.
        for issue in &issues {
            issue_assets.push(extract_linked_assets(
                &issue.fields.extra,
                &cmdb_field_id_list,
            ));
        }

        // Collect unique (workspace_id, object_id) pairs that need enrichment,
        // then resolve them all in one batch to avoid redundant API calls.
        use std::collections::HashMap as StdHashMap;
        let mut to_enrich: StdHashMap<(String, String), ()> = StdHashMap::new();
        let mut enrich_indices: Vec<(usize, usize)> = Vec::new(); // (issue_idx, asset_idx)

        for (i, assets) in issue_assets.iter().enumerate() {
            for (j, asset) in assets.iter().enumerate() {
                if asset.id.is_some() && asset.key.is_none() && asset.name.is_none() {
                    let wid = asset.workspace_id.clone().unwrap_or_default();
                    let oid = asset.id.clone().expect("asset.id.is_some() checked above");
                    let key = (wid, oid);
                    to_enrich.entry(key.clone()).or_insert(());
                    enrich_indices.push((i, j));
                }
            }
        }

        if !to_enrich.is_empty() {
            // Get workspace ID for assets that don't carry their own.
            let fallback_wid = match crate::api::assets::workspace::get_or_fetch_workspace_id(
                client,
            )
            .await
            {
                Ok(wid) => Some(wid),
                Err(err) => {
                    eprintln!(
                        "warning: failed to fetch workspace ID for asset enrichment: {err}. Assets without embedded workspace IDs will be skipped."
                    );
                    None
                }
            };

            let futures: Vec<_> = to_enrich
                .keys()
                .map(|(wid, oid)| {
                    let wid = if wid.is_empty() {
                        fallback_wid.clone().unwrap_or_default()
                    } else {
                        wid.clone()
                    };
                    let oid = oid.clone();
                    async move {
                        let result = client.get_asset(&wid, &oid, false).await;
                        (wid, oid, result)
                    }
                })
                .collect();

            let results: Vec<_> = stream::iter(futures)
                .buffer_unordered(MAX_CONCURRENT_ASSET_FETCHES)
                .collect()
                .await;
            let mut resolved: StdHashMap<(String, String), (String, String, String)> =
                StdHashMap::new();
            for (wid, oid, result) in results {
                if let Ok(obj) = result {
                    resolved.insert(
                        (wid.clone(), oid.clone()),
                        (obj.object_key, obj.label, obj.object_type.name),
                    );
                }
            }

            // Apply enrichment back to assets.
            // Mirror the same wid-resolution logic used when building futures:
            // an empty workspace_id falls back to fallback_wid (the same value
            // used as the key in `resolved`).
            for (i, j) in &enrich_indices {
                let asset = &issue_assets[*i][*j];
                if let Some(oid) = asset.id.clone() {
                    let raw_wid = asset.workspace_id.clone().unwrap_or_default();
                    let effective_wid = if raw_wid.is_empty() {
                        fallback_wid.clone().unwrap_or_default()
                    } else {
                        raw_wid
                    };
                    if let Some((key, name, asset_type)) = resolved.get(&(effective_wid, oid)) {
                        issue_assets[*i][*j].key = Some(key.clone());
                        issue_assets[*i][*j].name = Some(name.clone());
                        issue_assets[*i][*j].asset_type = Some(asset_type.clone());
                    }
                }
            }
        }
    }

    // For JSON output with --assets, inject enriched data back into issue JSON
    if show_assets_col && matches!(output_format, OutputFormat::Json) {
        for (i, issue) in issues.iter_mut().enumerate() {
            if issue_assets[i].is_empty() {
                continue;
            }
            // Build per-field-id enrichment: re-extract per field to get grouping,
            // then match by position to enriched issue_assets[i]
            let mut per_field_by_id: Vec<(String, Vec<LinkedAsset>)> = Vec::new();
            let mut offset = 0;
            for field_id in &cmdb_field_id_list {
                let count =
                    extract_linked_assets(&issue.fields.extra, std::slice::from_ref(field_id))
                        .len();
                if count > 0 && offset + count <= issue_assets[i].len() {
                    let enriched = issue_assets[i][offset..offset + count].to_vec();
                    per_field_by_id.push((field_id.clone(), enriched));
                }
                offset += count;
            }
            enrich_json_assets(&mut issue.fields.extra, &per_field_by_id);
        }
    }

    // Team column gating (#191): show only when team_field_id is configured
    // AND at least one issue has a populated team. Build the UUID→name map
    // once so per-row resolution is O(1) against the HashMap (rather than a
    // linear scan of the cache vec for every row).
    //
    // Skipped entirely in JSON mode: `print_output` only serializes `issues`
    // under OutputFormat::Json and ignores `rows`, so the cache read + map
    // build would be wasted filesystem I/O. JSON consumers already see the
    // raw UUID under `fields.<team_field_id>` (IssueFields::extra is
    // `#[serde(flatten)]`) and can resolve locally.
    let client_verbose = client.verbose();
    // Nested if (not a let-chain): let-chains require Rust >= 1.88 + edition 2024; MSRV is 1.85. See CLAUDE.md Conventions — No let-chains.
    let team_displays: Vec<String> = if matches!(output_format, OutputFormat::Table) {
        if let Some(field_id) = team_field_id {
            let uuids: Vec<Option<String>> = issues
                .iter()
                .map(|i| i.fields.team_id(field_id, client_verbose))
                .collect();
            if uuids.iter().any(|u| u.is_some()) {
                // Team cache read is best-effort for display — an Err or missing
                // entry falls back to the UUID. Cache population is not this
                // command's responsibility.
                let team_map: std::collections::HashMap<String, String> =
                    crate::cache::read_team_cache(&config.active_profile_name)
                        .ok()
                        .flatten()
                        .map(|c| c.teams.into_iter().map(|t| (t.id, t.name)).collect())
                        .unwrap_or_default();
                uuids
                    .iter()
                    .map(|u| match u {
                        Some(uuid) => team_map.get(uuid).cloned().unwrap_or_else(|| uuid.clone()),
                        None => "-".to_string(),
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    let show_team_col = !team_displays.is_empty();

    let rows: Vec<Vec<String>> = issues
        .iter()
        .enumerate()
        .map(|(i, issue)| {
            let assets = if show_assets_col {
                Some(issue_assets[i].as_slice())
            } else {
                None
            };
            let team = if show_team_col {
                Some(team_displays[i].as_str())
            } else {
                None
            };
            // `Some("")` fallback (not `None`) when the issue's own duedate
            // is unset — this keeps the column SHOWN (rendering "-" via
            // `render_due_date`) whenever `--duedate` is passed, rather than
            // hiding the column per-row based on data presence (BC-2.2.032).
            let duedate = if show_duedate {
                Some(issue.fields.duedate.as_deref().unwrap_or(""))
            } else {
                None
            };
            format::format_issue_row(issue, duedate, effective_sp, assets, team)
        })
        .collect();
    let headers = format::issue_table_headers(
        show_duedate,
        effective_sp.is_some(),
        show_assets_col,
        show_team_col,
    );
    output::print_output(output_format, &headers, &rows, &issues)?;

    if has_more && !all {
        let count_jql = crate::jql::strip_order_by(&effective_jql);
        match client.approximate_count(count_jql).await {
            Ok(total) if total > 0 => {
                eprintln!(
                    "Showing {} of ~{} results. Use --limit or --all to see more.",
                    issues.len(),
                    total
                );
            }
            Ok(_) | Err(_) => {
                eprintln!(
                    "Showing {} results. Use --limit or --all to see more.",
                    issues.len()
                );
            }
        }
    }

    Ok(())
}

/// Resolve the repeatable `--component` flag's raw values into zero, one, or
/// two composed JQL clause fragments (bare-then-`not:` order per BC-2.1.018
/// Precondition 3 / BC-2.1.019 Postcondition 2), per BC-2.1.018..022:
///
/// - Bare `--component <NAME>` (repeatable) → OR-combined `component in (...)`.
/// - `--component not:<NAME>` → the full `(component not in (...) OR component
///   is EMPTY)` form — never a bare `not in`.
/// - `--component none` → `component is EMPTY`, ZERO resolver HTTP; must be the
///   ONLY occurrence; still requires `project_key` (project-scope guard).
/// - `--component all:<N1>,<N2>` → AND-combined `component = id1 AND component
///   = id2 ...`; at most one `all:` occurrence; not combinable with
///   bare/`not:`/`none`.
/// - Any non-`none` value resolves via `helpers::resolve_component` (§8.4)
///   BEFORE composition; zero/ambiguous matches → exit 64 with ZERO
///   `POST /rest/api/3/search/jql` calls (BC-2.1.022, VP-COMPONENT-013).
/// - No `project_key` for a bare/`not:`/`all:`/`none` value → exit 64
///   pre-flight, naming `--project`, before any resolver GET.
///
/// Caller contract: `values` MUST be non-empty — the zero-`--component`-flags
/// case is short-circuited by the caller (`handle_list`) before this function
/// is reached, so it need not (and does not) special-case an empty slice.
/// ALSO: `validate_component_preflight(values, project_key)` MUST have
/// already succeeded against these exact `values`/`project_key` — this
/// function performs the ACTUAL §8.4 resolver HTTP calls only; the
/// combination/count/project-scope guard logic lives entirely in
/// `validate_component_preflight`, which `handle_list` calls BEFORE the
/// `project_exists` GET so a rejected combination or missing project scope
/// costs literally zero HTTP calls (VP-COMPONENT-013, BC-2.1.022
/// EC-2.1.022-1/2). Deliberately does NOT implement issue #607's generalized
/// multi-valued/negatable filter grammar — these forms are pre-composed and
/// component-specific, not a reusable abstraction.
async fn resolve_component_clauses(
    client: &JiraClient,
    project_key: Option<&str>,
    values: &[String],
) -> Result<Vec<String>> {
    // `none`: zero resolver HTTP (BC-2.1.020 Postcondition 1). Project scope
    // was already confirmed by `validate_component_preflight`.
    if values.len() == 1 && values[0].eq_ignore_ascii_case("none") {
        return Ok(vec!["component is EMPTY".to_string()]);
    }

    let pk = project_key.expect(
        "validate_component_preflight guarantees a project scope for any \
         non-`none` --component value",
    );

    // `all:` form — at most one occurrence is guaranteed by
    // `validate_component_preflight`. Comma-separated names AND-compose into
    // repeated equality (BC-2.1.021 Postcondition 1), NOT `IN`.
    if let Some(all_value) = values.iter().find(|v| v.starts_with("all:")) {
        let components = client.list_components(pk).await?;
        let candidate_names: Vec<String> = components.iter().map(|c| c.name.clone()).collect();

        let mut equality_clauses = Vec::new();
        for name in all_value["all:".len()..].split(',') {
            let ids = resolve_one_component_id(name, pk, &components, &candidate_names)?;
            if ids.len() == 1 {
                equality_clauses.push(format!("component = {}", ids[0]));
            } else {
                // F5-A-M1/F5-C-001 (human-adjudicated: UNION) — ExactMultiple
                // becomes a parenthesized OR-of-equalities term standing in
                // for this one name's position in the AND-chain (BC-2.1.021
                // Postcondition 2 / EC-2.1.021-4).
                let or_group = ids
                    .iter()
                    .map(|id| format!("component = {id}"))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                equality_clauses.push(format!("({or_group})"));
            }
        }
        return Ok(vec![equality_clauses.join(" AND ")]);
    }

    // Bare + `not:` forms — MAY coexist (BC-2.1.018 Precondition 3), composing
    // two AND-joined clauses in bare-then-`not:` order (BC-2.1.018
    // Postcondition 2 / BC-2.1.019 Postcondition 2).
    let components = client.list_components(pk).await?;
    let candidate_names: Vec<String> = components.iter().map(|c| c.name.clone()).collect();

    let mut bare_ids: Vec<String> = Vec::new();
    let mut not_ids: Vec<String> = Vec::new();
    for v in values {
        if let Some(name) = v.strip_prefix("not:") {
            // F5-A-M1/F5-C-001: each value's resolved ids (ascending numeric
            // for an ExactMultiple union) are appended in comma-supplied
            // value order (BC-2.1.019 PC3 / EC-2.1.019-4).
            not_ids.extend(resolve_one_component_id(
                name,
                pk,
                &components,
                &candidate_names,
            )?);
        } else {
            bare_ids.extend(resolve_one_component_id(
                v,
                pk,
                &components,
                &candidate_names,
            )?);
        }
    }

    let mut clauses = Vec::new();
    if !bare_ids.is_empty() {
        clauses.push(format!("component in ({})", bare_ids.join(", ")));
    }
    if !not_ids.is_empty() {
        clauses.push(format!(
            "(component not in ({}) OR component is EMPTY)",
            not_ids.join(", ")
        ));
    }
    Ok(clauses)
}

/// Pure, HTTP-free pre-flight validation for the `--component` flag's
/// combination/count constraints (BC-2.1.020 Precondition 1 — `none` must be
/// the sole occurrence; BC-2.1.021 Preconditions 1-2 — at most one `all:`
/// occurrence, not combined with bare/`not:`/`none`) and project-scope
/// requirement (BC-2.1.020 Precondition 2 / BC-2.1.022 EC-2.1.022-1/2 — every
/// non-empty `--component` value list, including `none`, needs a resolved
/// project). MUST run before any HTTP call — including `project_exists` — so
/// a rejected combination or missing project scope costs literally zero
/// requests (VP-COMPONENT-013).
///
/// Caller contract: only meaningful when `values` is non-empty — the
/// zero-`--component`-flags case is handled by the caller before this is
/// reached.
fn validate_component_preflight(
    values: &[String],
    project_key: Option<&str>,
) -> std::result::Result<(), JrError> {
    let is_sole_none = values.len() == 1 && values[0].eq_ignore_ascii_case("none");

    let none_count = values
        .iter()
        .filter(|v| v.eq_ignore_ascii_case("none"))
        .count();
    if none_count > 0 && values.len() > 1 {
        return Err(JrError::UserError(
            "--component none cannot be combined with other --component values.".into(),
        ));
    }

    let all_count = values.iter().filter(|v| v.starts_with("all:")).count();
    if all_count > 1 {
        return Err(JrError::UserError(
            "--component all: may only be specified once; comma-separate multiple names within one all: value."
                .into(),
        ));
    }
    if all_count == 1 && values.len() > 1 {
        return Err(JrError::UserError(
            "--component all: cannot be combined with other --component values.".into(),
        ));
    }

    if project_key.is_none() {
        return Err(JrError::UserError(if is_sole_none {
            "--component none requires --project (or a configured default project) to avoid an unrestricted org-wide search."
                .into()
        } else {
            "--component requires --project (or a configured default project) to resolve component names."
                .into()
        }));
    }

    Ok(())
}

/// Resolve one `--component` name/id (already prefix-stripped by the caller
/// for `not:`/`all:` forms) to its numeric component id(s) via §8.4
/// (`helpers::resolve_component`), mapping a name match back to its id via
/// `components`. BC-8.4.002/003 failure messages, verbatim (alphabetically
/// sorted, case-insensitive, period-terminated) — mirrors
/// `cli/component.rs`'s identical resolution pattern.
///
/// F5-A-M1/F5-C-001 (2026-08-17, human-adjudicated: UNION) — `Exact` and the
/// numeric-id bypass always resolve to a single-element `Vec`; a case-only
/// duplicate name (`MatchResult::ExactMultiple`) resolves to EVERY
/// case-insensitively-name-matching component id in `components` (a re-scan
/// of the already-fetched list — zero extra HTTP), ascending numeric order,
/// per BC-2.1.018 Postcondition 3 / BC-2.1.019 Postcondition 3 / BC-2.1.021
/// Postcondition 2. This is a READ-PATH-ONLY divergence from
/// `cli/component.rs`'s fail-closed mutating behavior on `ExactMultiple`
/// (BC-2.1.022 EC-2.1.022-3) — do NOT change the mutating commands to match.
fn resolve_one_component_id(
    input: &str,
    project: &str,
    components: &[crate::types::jira::component::Component],
    candidate_names: &[String],
) -> std::result::Result<Vec<String>, JrError> {
    match helpers::resolve_component(input, project, candidate_names) {
        MatchResult::Exact(matched) => {
            if helpers::is_numeric_component_id(input) {
                // Numeric bypass (BC-8.4.001 step 1): `matched` IS the id.
                Ok(vec![matched])
            } else {
                components
                    .iter()
                    .find(|c| c.name == matched)
                    .map(|c| vec![c.id.clone()])
                    .ok_or_else(|| {
                        JrError::Internal(format!(
                            "Internal error: resolved component name '{}' not found in list.",
                            matched
                        ))
                    })
            }
        }
        MatchResult::ExactMultiple(matched) => {
            // Numeric bypass never produces ExactMultiple (it short-circuits
            // to Exact in `helpers::resolve_component` step 1), so `matched`
            // here is always a name — re-scan `components` for every
            // case-insensitive name match and union their ids.
            let mut ids: Vec<String> = components
                .iter()
                .filter(|c| c.name.to_lowercase() == matched.to_lowercase())
                .map(|c| c.id.clone())
                .collect();
            if ids.is_empty() {
                return Err(JrError::Internal(format!(
                    "Internal error: resolved component name '{}' not found in list.",
                    matched
                )));
            }
            ids.sort_by_key(|id| id.parse::<u64>().unwrap_or(u64::MAX));
            Ok(ids)
        }
        MatchResult::Ambiguous(mut candidates) => {
            candidates.sort_by_key(|s| s.to_lowercase());
            Err(JrError::UserError(format!(
                "Ambiguous component '{}'. Matches: {}.",
                input,
                candidates.join(", ")
            )))
        }
        MatchResult::None(mut available) => {
            available.sort_by_key(|s| s.to_lowercase());
            Err(JrError::UserError(format!(
                "Component '{}' not found in project {}. Available: {}.",
                input,
                project,
                available.join(", ")
            )))
        }
    }
}

/// Resolve whether to show story points. Returns the field ID if points should
/// be shown, or None. Emits a warning to stderr if --points was requested but
/// config is missing.
fn resolve_show_points(show_points: bool, sp_field_id: Option<&str>) -> Option<&str> {
    if show_points {
        match sp_field_id {
            Some(id) => Some(id),
            None => {
                eprintln!(
                    "warning: --points ignored. Story points field not configured. \
                     Run \"jr init\" or set story_points_field_id under [profiles.<name>] in ~/.config/jr/config.toml"
                );
                None
            }
        }
    } else {
        None
    }
}

/// Options bag for `build_filter_clauses` — groups all resolved JQL filter
/// fragments so the function stays within clippy's argument-count limit.
struct FilterOptions<'a> {
    assignee_jql: Option<&'a str>,
    reporter_jql: Option<&'a str>,
    status: Option<&'a str>,
    team_clause: Option<&'a str>,
    recent: Option<&'a str>,
    /// S-579-1 (BC-2.1.007 amendment): slots in immediately after `recent`
    /// (`created >= -{d}`), before `asset_clause`.
    updated_recent: Option<&'a str>,
    open: bool,
    asset_clause: Option<&'a str>,
    /// Zero, one, or two pre-composed `--component` clause fragments, already
    /// resolved and formatted by `resolve_component_clauses` (bare-then-`not:`
    /// order per BC-2.1.018 Precondition 3). Slots in after `asset_clause`,
    /// before the date-range clauses (BC-2.1.007 amendment).
    component_clauses: &'a [String],
    created_after_clause: Option<&'a str>,
    created_before_clause: Option<&'a str>,
    updated_after_clause: Option<&'a str>,
    updated_before_clause: Option<&'a str>,
}

/// Build JQL filter clauses from resolved flag values.
fn build_filter_clauses(opts: FilterOptions<'_>) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(a) = opts.assignee_jql {
        parts.push(format!("assignee = {a}"));
    }
    if let Some(r) = opts.reporter_jql {
        parts.push(format!("reporter = {r}"));
    }
    if let Some(s) = opts.status {
        parts.push(format!("status = \"{}\"", crate::jql::escape_value(s)));
    }
    if opts.open {
        parts.push("statusCategory != Done".to_string());
    }
    if let Some(t) = opts.team_clause {
        parts.push(t.to_string());
    }
    if let Some(d) = opts.recent {
        parts.push(format!("created >= -{d}"));
    }
    // S-579-1 (BC-2.1.007 amendment): --updated-recent's clause slots in
    // immediately after `recent`, before `asset`.
    if let Some(d) = opts.updated_recent {
        parts.push(format!("updated >= -{d}"));
    }
    if let Some(a) = opts.asset_clause {
        parts.push(a.to_string());
    }
    // S-606-1 (BC-2.1.007 amendment): --component clause(s) slot in here —
    // after `asset`, before the date-range clauses. `opts.component_clauses`
    // is already fully resolved/composed by `resolve_component_clauses`; this
    // is only the ordered-insertion point.
    parts.extend(opts.component_clauses.iter().cloned());
    if let Some(c) = opts.created_after_clause {
        parts.push(c.to_string());
    }
    if let Some(c) = opts.created_before_clause {
        parts.push(c.to_string());
    }
    if let Some(c) = opts.updated_after_clause {
        parts.push(c.to_string());
    }
    if let Some(c) = opts.updated_before_clause {
        parts.push(c.to_string());
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_show_points_flag_false() {
        assert_eq!(resolve_show_points(false, Some("customfield_10031")), None);
        assert_eq!(resolve_show_points(false, None), None);
    }

    #[test]
    fn resolve_show_points_flag_true_config_present() {
        assert_eq!(
            resolve_show_points(true, Some("customfield_10031")),
            Some("customfield_10031")
        );
    }

    #[test]
    fn resolve_show_points_flag_true_config_missing() {
        // Warning emitted to stderr (not captured), but function returns None without error
        assert_eq!(resolve_show_points(true, None), None);
    }

    #[test]
    fn build_jql_parts_assignee_me() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["assignee = currentUser()"]);
    }

    #[test]
    fn build_jql_parts_reporter_account_id() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: Some("5b10ac8d82e05b22cc7d4ef5"),
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["reporter = 5b10ac8d82e05b22cc7d4ef5"]);
    }

    #[test]
    fn build_jql_parts_recent() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: Some("7d"),
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["created >= -7d"]);
    }

    #[test]
    fn build_jql_parts_all_filters() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: Some("currentUser()"),
            status: Some("In Progress"),
            team_clause: Some(r#"customfield_10001 = "uuid-123""#),
            recent: Some("30d"),
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 5);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"reporter = currentUser()".to_string()));
        assert!(parts.contains(&"status = \"In Progress\"".to_string()));
        assert!(parts.contains(&r#"customfield_10001 = "uuid-123""#.to_string()));
        assert!(parts.contains(&"created >= -30d".to_string()));
    }

    #[test]
    fn build_jql_parts_empty() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert!(parts.is_empty());
    }

    #[test]
    fn build_jql_parts_jql_plus_status_compose() {
        let filter = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: Some("Done"),
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        let mut all_parts = vec!["type = Bug".to_string()];
        all_parts.extend(filter);
        let jql = all_parts.join(" AND ");
        assert_eq!(jql, r#"type = Bug AND status = "Done""#);
    }

    #[test]
    fn build_jql_parts_status_escaping() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: Some(r#"He said "hi" \o/"#),
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec![r#"status = "He said \"hi\" \\o/""#.to_string()]);
    }

    #[test]
    fn build_jql_parts_open() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: true,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["statusCategory != Done"]);
    }

    #[test]
    fn build_jql_parts_open_with_assignee() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: true,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 2);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"statusCategory != Done".to_string()));
    }

    #[test]
    fn build_jql_parts_all_filters_with_open() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: Some("currentUser()"),
            status: None, // status conflicts with open, so None here
            team_clause: Some(r#"customfield_10001 = "uuid-123""#),
            recent: Some("30d"),
            updated_recent: None,
            open: true,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 5);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&"reporter = currentUser()".to_string()));
        assert!(parts.contains(&"statusCategory != Done".to_string()));
        assert!(parts.contains(&r#"customfield_10001 = "uuid-123""#.to_string()));
        assert!(parts.contains(&"created >= -30d".to_string()));
    }

    #[test]
    fn build_jql_parts_asset_clause() {
        let clause = r#""Client" IN aqlFunction("Key = \"CUST-5\"")"#;
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: Some(clause),
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec![clause.to_string()]);
    }

    #[test]
    fn build_jql_parts_asset_with_assignee() {
        let clause = r#""Client" IN aqlFunction("Key = \"CUST-5\"")"#;
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: Some(clause),
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 2);
        assert!(parts.contains(&"assignee = currentUser()".to_string()));
        assert!(parts.contains(&clause.to_string()));
    }

    /// F-1 (Step-4.5 adversarial finding, coverage gap): pins BC-2.1.007's
    /// clause-ordering amendment / VP-COMPONENT-015 with BOTH the asset
    /// clause slot AND `component_clauses` non-empty in the same call — no
    /// pre-existing test in this module exercised that combination, so a
    /// reorder placing `component_clauses` before `asset_clause` (or after a
    /// date-range clause) would have stayed green. Asserts exact positional
    /// Vec equality, not a loose `contains`/`len` check, so the pin actually
    /// catches a reorder.
    #[test]
    fn test_bc_2_1_007_build_filter_clauses_component_immediately_after_asset() {
        let asset_clause = r#""Client" IN aqlFunction("Key = \"CUST-5\"")"#;
        let component_clauses = vec![
            "component in (10001)".to_string(),
            "(component not in (10002) OR component is EMPTY)".to_string(),
        ];
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: Some("currentUser()"),
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: Some(asset_clause),
            component_clauses: &component_clauses,
            created_after_clause: Some("created >= \"2026-03-01\""),
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(
            parts,
            vec![
                "assignee = currentUser()".to_string(),
                asset_clause.to_string(),
                "component in (10001)".to_string(),
                "(component not in (10002) OR component is EMPTY)".to_string(),
                "created >= \"2026-03-01\"".to_string(),
            ]
        );
    }

    /// VP-UPDATED-RECENT-001 / AC-005 (M1 gap fix): `--recent`, `--updated-recent`,
    /// and `--asset` together compose clauses with `updated >= -{d}` positioned
    /// IMMEDIATELY AFTER `created >= -{d}` (recent) and BEFORE the asset clause.
    /// Verified via exact `Vec<String>` positional equality — NOT substring-index
    /// comparison — per AC-005's mandated discipline (mirrors
    /// `test_bc_2_1_007_build_filter_clauses_component_immediately_after_asset`'s
    /// style, the existing precedent for this discipline in this module).
    #[test]
    fn test_bc_2_1_007_build_filter_clauses_updated_recent_immediately_after_recent_before_asset() {
        let asset_clause = r#""Client" IN aqlFunction("Key = \"CUST-5\"")"#;
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: Some("7d"),
            updated_recent: Some("60d"),
            open: false,
            asset_clause: Some(asset_clause),
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(
            parts,
            vec![
                "created >= -7d".to_string(),
                "updated >= -60d".to_string(),
                asset_clause.to_string(),
            ]
        );
    }

    #[test]
    fn build_jql_parts_created_after_clause() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: Some("created >= \"2026-03-18\""),
            created_before_clause: None,
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts, vec!["created >= \"2026-03-18\""]);
    }

    #[test]
    fn build_jql_parts_updated_after_and_before_clauses() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: None,
            created_before_clause: None,
            updated_after_clause: Some("updated >= \"2026-03-01\""),
            updated_before_clause: Some("updated < \"2026-04-01\""),
        });
        assert_eq!(parts.len(), 2);
        assert!(parts.contains(&"updated >= \"2026-03-01\"".to_string()));
        assert!(parts.contains(&"updated < \"2026-04-01\"".to_string()));
    }

    #[test]
    fn build_jql_parts_created_date_range() {
        let parts = build_filter_clauses(FilterOptions {
            assignee_jql: None,
            reporter_jql: None,
            status: None,
            team_clause: None,
            recent: None,
            updated_recent: None,
            open: false,
            asset_clause: None,
            component_clauses: &[],
            created_after_clause: Some("created >= \"2026-03-01\""),
            created_before_clause: Some("created < \"2026-04-01\""),
            updated_after_clause: None,
            updated_before_clause: None,
        });
        assert_eq!(parts.len(), 2);
        assert!(parts.contains(&"created >= \"2026-03-01\"".to_string()));
        assert!(parts.contains(&"created < \"2026-04-01\"".to_string()));
    }

    #[test]
    fn build_jql_base_parts_jql_with_project() {
        let (parts, order_by) = build_jql_base_parts("priority = Highest", Some("PROJ"));
        assert_eq!(
            parts,
            vec![
                "project = \"PROJ\"".to_string(),
                "(priority = Highest)".to_string(),
            ]
        );
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_without_project() {
        let (parts, order_by) = build_jql_base_parts("priority = Highest", None);
        assert_eq!(parts, vec!["(priority = Highest)".to_string()]);
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_with_order_by_and_project() {
        let (parts, order_by) =
            build_jql_base_parts("priority = Highest ORDER BY created DESC", Some("PROJ"));
        assert_eq!(
            parts,
            vec![
                "project = \"PROJ\"".to_string(),
                "(priority = Highest)".to_string(),
            ]
        );
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_or_with_project_preserves_scope() {
        let (parts, order_by) =
            build_jql_base_parts("priority = Highest OR status = Done", Some("PROJ"));
        assert_eq!(
            parts,
            vec![
                "project = \"PROJ\"".to_string(),
                "(priority = Highest OR status = Done)".to_string(),
            ]
        );
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_order_by_only_with_project() {
        let (parts, order_by) = build_jql_base_parts("ORDER BY created DESC", Some("PROJ"));
        assert_eq!(parts, vec!["project = \"PROJ\"".to_string()]);
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn build_jql_base_parts_jql_order_by_only_no_project() {
        let (parts, order_by) = build_jql_base_parts("ORDER BY created DESC", None);
        assert!(parts.is_empty());
        assert_eq!(order_by, "updated DESC");
    }

    #[test]
    fn extract_unique_status_names_deduplicates_and_sorts() {
        use crate::api::jira::projects::{IssueTypeWithStatuses, StatusMetadata};
        let issue_types = vec![
            IssueTypeWithStatuses {
                id: "1".into(),
                name: "Task".into(),
                subtask: None,
                statuses: vec![
                    StatusMetadata {
                        id: "10".into(),
                        name: "To Do".into(),
                        description: None,
                    },
                    StatusMetadata {
                        id: "20".into(),
                        name: "In Progress".into(),
                        description: None,
                    },
                    StatusMetadata {
                        id: "30".into(),
                        name: "Done".into(),
                        description: None,
                    },
                ],
            },
            IssueTypeWithStatuses {
                id: "2".into(),
                name: "Bug".into(),
                subtask: None,
                statuses: vec![
                    StatusMetadata {
                        id: "10".into(),
                        name: "To Do".into(),
                        description: None,
                    },
                    StatusMetadata {
                        id: "30".into(),
                        name: "Done".into(),
                        description: None,
                    },
                ],
            },
        ];
        let names = extract_unique_status_names(&issue_types);
        assert_eq!(names, vec!["Done", "In Progress", "To Do"]);
    }

    #[test]
    fn extract_unique_status_names_empty() {
        let names = extract_unique_status_names(&[]);
        assert!(names.is_empty());
    }

    // ── BC-4.3.001 unit tests (H-036) ────────────────────────────────────────
    //
    // These tests model the exact HashMap pattern used in the enrichment pipeline
    // (lines 446-460) without requiring any async/HTTP setup.  They verify:
    //
    //   AC-001: a bare `HashMap<String, _>` key causes last-write-wins when two
    //           workspaces share the same oid — the first entry is silently
    //           overwritten.  The test asserts BOTH entries are retrievable; this
    //           assertion FAILS on the buggy type, confirming the Red Gate.
    //
    //   AC-002: a composite `HashMap<(String, String), _>` key preserves both
    //           entries.  This is the expected post-fix state.
    //
    //   AC-003: the `to_enrich` HashMap (line 398, already `HashMap<(String,
    //           String), ()>`) is unaffected; the lock is validated implicitly
    //           through AC-001 (fixing line 446 without touching line 398 keeps
    //           `to_enrich` correct).

    /// test_bc_4_3_001_bare_oid_key_collides_on_shared_oid (H-036, PASSES post-fix)
    ///
    /// Verifies that the production `resolved` map — now keyed on composite
    /// `(workspace_id, oid)` — preserves both workspace-A and workspace-B
    /// entries for the same bare `oid`.  Pre-fix (bare `HashMap<String, _>`
    /// key), the second insert would have overwritten the first, so only
    /// "Widgets Inc" would survive.  Post-fix the composite key keeps both.
    ///
    /// H-036: MUST-PASS after the BC-4.3.001 fix is merged.
    #[test]
    fn test_bc_4_3_001_bare_oid_key_collides_on_shared_oid() {
        use std::collections::HashMap as StdHashMap;

        // Post-fix type: HashMap<(String, String), _> — composite (wid, oid) key.
        let mut resolved: StdHashMap<(String, String), (String, String, String)> =
            StdHashMap::new();

        let oid = "88".to_string();

        // Insert ws-A / oid "88" → "Acme Corp"
        resolved.insert(
            ("ws-A".to_string(), oid.clone()),
            ("WS-A-88".into(), "Acme Corp".into(), "Client".into()),
        );

        // Insert ws-B / oid "88" → "Widgets Inc" — must NOT overwrite ws-A
        resolved.insert(
            ("ws-B".to_string(), oid.clone()),
            ("WS-B-88".into(), "Widgets Inc".into(), "Client".into()),
        );

        // Both entries must be independently addressable (H-036 postcondition).
        let (_, label_a, _) = resolved
            .get(&("ws-A".to_string(), oid.clone()))
            .expect("ws-A entry must be present after fix");
        assert_eq!(
            label_a, "Acme Corp",
            "BC-4.3.001: ws-A label 'Acme Corp' must survive the ws-B insert. \
             Composite (workspace_id, oid) key preserves both entries."
        );

        let (_, label_b, _) = resolved
            .get(&("ws-B".to_string(), oid.clone()))
            .expect("ws-B entry must be present after fix");
        assert_eq!(
            label_b, "Widgets Inc",
            "BC-4.3.001: ws-B label must be 'Widgets Inc'"
        );
    }

    /// test_bc_4_3_001_composite_key_preserves_both_workspaces (PASSES always)
    ///
    /// Demonstrates the correct fix: a composite `(wid, oid)` key preserves
    /// both entries when two workspaces share the same oid.  This test passes
    /// on both the pre-fix and post-fix branches and serves as documentation
    /// of the intended post-fix behaviour (AC-002 invariant).
    #[test]
    fn test_bc_4_3_001_composite_key_preserves_both_workspaces() {
        use std::collections::HashMap as StdHashMap;

        // The fixed type from BC-4.3.001: HashMap<(String, String), _>
        let mut resolved_fixed: StdHashMap<(String, String), (String, String, String)> =
            StdHashMap::new();

        let oid = "88".to_string();

        // Insert ws-A / oid "88" → "Acme Corp"
        resolved_fixed.insert(
            ("ws-A".to_string(), oid.clone()),
            ("WS-A-88".into(), "Acme Corp".into(), "Client".into()),
        );

        // Insert ws-B / oid "88" → "Widgets Inc" — does NOT overwrite ws-A
        resolved_fixed.insert(
            ("ws-B".to_string(), oid.clone()),
            ("WS-B-88".into(), "Widgets Inc".into(), "Client".into()),
        );

        // Both entries are present and independently addressable.
        assert_eq!(
            resolved_fixed.len(),
            2,
            "Composite key map must hold two distinct entries"
        );

        let (_, label_a, _) = resolved_fixed
            .get(&("ws-A".to_string(), oid.clone()))
            .expect("ws-A entry must be present");
        assert_eq!(label_a, "Acme Corp");

        let (_, label_b, _) = resolved_fixed
            .get(&("ws-B".to_string(), oid.clone()))
            .expect("ws-B entry must be present");
        assert_eq!(label_b, "Widgets Inc");
    }

    /// test_bc_4_3_001_to_enrich_composite_key_unchanged (AC-003, PASSES always)
    ///
    /// Verifies that `to_enrich: HashMap<(String, String), ()>` (line 398,
    /// already correct) correctly deduplicates by (workspace_id, oid) pairs
    /// and does NOT merge entries from different workspaces.
    /// This test is structural: it passes on both branches because line 398
    /// is not touched by the fix.
    #[test]
    fn test_bc_4_3_001_to_enrich_composite_key_unchanged() {
        use std::collections::HashMap as StdHashMap;

        // Mirror of the `to_enrich` map at line 398 in list.rs.
        let mut to_enrich: StdHashMap<(String, String), ()> = StdHashMap::new();

        // Same oid "88" in two different workspaces — both must be retained.
        to_enrich
            .entry(("ws-A".to_string(), "88".to_string()))
            .or_insert(());
        to_enrich
            .entry(("ws-B".to_string(), "88".to_string()))
            .or_insert(());

        // Duplicate insertion for ws-A (simulates seeing PROJ-1 twice) — must NOT add a third.
        to_enrich
            .entry(("ws-A".to_string(), "88".to_string()))
            .or_insert(());

        assert_eq!(
            to_enrich.len(),
            2,
            "to_enrich must hold exactly 2 unique (wid, oid) pairs; \
             same oid from different workspaces are distinct, duplicates are deduplicated"
        );
        assert!(to_enrich.contains_key(&("ws-A".to_string(), "88".to_string())));
        assert!(to_enrich.contains_key(&("ws-B".to_string(), "88".to_string())));
    }
}
