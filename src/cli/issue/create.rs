use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use serde_json::json;

use crate::adf;
use crate::api::assets::linked::get_or_fetch_cmdb_fields;
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::partial_match::MatchResult;

use super::field_resolve;
use super::format;
use super::helpers;
use super::jsm_create::{JsmCreateArgs, handle_jsm_create};

pub(super) async fn handle_create(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
    no_input: bool,
) -> Result<()> {
    let IssueCommand::Create {
        project,
        issue_type,
        summary,
        description,
        description_stdin,
        priority,
        label: labels,
        component: components,
        team,
        points,
        markdown,
        parent,
        to,
        account_id,
        request_type,
        field: field_pairs,
        on_behalf_of,
    } = command
    else {
        unreachable!()
    };

    // Pre-flight guard (BC-3.4.024 Postcondition 3, DEC-188 precedent —
    // mirrors the --field/--on-behalf-of guard below): --component is a
    // platform-path-only flag. It MUST be checked BEFORE the JSM
    // dispatch-fork immediately below, because that fork returns
    // unconditionally on `request_type.is_some()` — by the time execution
    // reaches the --field/--on-behalf-of guard, request_type is already
    // guaranteed None, too late to catch this combination. Exit 64, ZERO
    // HTTP (no service-desk lookup, no RT-id resolution, no component
    // resolution) — AC-009.
    if request_type.is_some() && !components.is_empty() {
        return Err(JrError::UserError(
            "--component is only valid on the platform create path and cannot be combined \
             with --request-type (JSM service-desk requests). Drop --request-type to create \
             a standard platform issue with --component, or drop --component and set it \
             afterward with `jr issue edit --component`."
                .into(),
        )
        .into());
    }

    // Dispatch fork: when --request-type is set, route to JSM path.
    // Platform path (when flag absent) is structurally unchanged. (BC-3.8.001, BC-3.3.001)
    if request_type.is_some() {
        return handle_jsm_create(
            client,
            config,
            output_format,
            project_override,
            no_input,
            JsmCreateArgs {
                project,
                request_type,
                summary,
                description,
                description_stdin,
                priority,
                labels,
                markdown,
                on_behalf_of,
                field_pairs,
                issue_type,
                team,
                points,
                parent,
                to,
                account_id,
            },
        )
        .await;
    }

    // Pre-flight guard (BC-3.8.013, UNCHANGED mechanism — DEC-310 reversal,
    // S-578-4): --on-behalf-of remains a self-declared JSM-only flag; on the
    // platform path (--request-type absent — this arm only runs when the
    // JSM dispatch fork above was NOT taken), supplying it alone is still a
    // categorical user error. Exit 64 BEFORE project-key resolution, BEFORE
    // any interactive prompt, BEFORE the blocking --description-stdin read,
    // and BEFORE any HTTP call.
    //
    // DEC-188's combined check (`--field` + `--on-behalf-of` → ONE error)
    // and its `--field`-alone check are REMOVED (DEC-310, BC-3.8.012
    // reversal, F3/F4 removal obligations) — `--field` no longer exits 64
    // pre-flight; it now resolves via createmeta (step 4b below). Per AC-003,
    // this standalone guard now fires even when `--field` is ALSO present,
    // since the combined check that used to pre-empt it is gone
    // (Architecture Compliance Rule 5: this guard's MECHANISM and verbatim
    // error string are untouched — only its trigger scope widens).
    //
    // MUST NOT be implemented via `#[arg(requires = "request_type")]` — that
    // yields clap exit 2, not the exit-64 JrError::UserError BC-3.8.013
    // requires.
    if on_behalf_of.is_some() {
        return Err(JrError::UserError(
            "--on-behalf-of is only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to raise a request on behalf of another user, or drop --on-behalf-of to create a standard platform issue."
                .into(),
        )
        .into());
    }

    // Step 2a (NEW, S-578-4): parse --field NAME[:kind]=VALUE hints (BC-3.3.010
    // Preconditions, SSOT "Platform-Path Guard Ordering" block, step 2a).
    // Malformed hint exits 64 HERE — before step 2b's collision guard and
    // before project/type resolution, interactive prompts, or any HTTP call.
    // `parse_field_kv` is the SAME pure parser BC-3.4.026 already validates
    // (S-578-1) and `edit.rs`/`jsm_create.rs` already call — reused verbatim,
    // no per-call-site parsing divergence.
    let field_spec_map = parse_field_kv(&field_pairs)?;

    // Step 2b (NEW, S-578-4): D2 create-path collision guard (BC-3.3.010
    // Invariant 5, BC-3.3.011 taxonomy row 1, ADR-0019 §"D2 correction").
    // Ten-member governed set (`field_resolve::CREATE_D2_GOVERNED_KEYS`),
    // DISTINCT from edit-path Gate B's five-member set (Architecture
    // Compliance Rule 2 — `detect_flag_field_overlap` is a shared MECHANISM,
    // never a shared governed-key SET). Zero HTTP; runs BEFORE project/type
    // resolution and BEFORE any interactive prompt.
    if !field_spec_map.is_empty() {
        // --points / --team are the two "resolved-id" governed keys
        // (AC-011) — asserted SEPARATELY via RESOLVED-ID equality
        // (bypass-form-only). `story_points_field_id` is read directly from
        // config (never via `helpers::resolve_story_points_field_id`, which
        // errors when unconfigured — the guard must be a silent no-op, not
        // an error, when the field isn't configured at all). `team_field_id`
        // is likewise config-only; `client.find_team_field_id()` (HTTP) is
        // NEVER invoked to service this guard.
        let points_resolved_id: Option<String> = if points.is_some() {
            config.active_profile().story_points_field_id.clone()
        } else {
            None
        };
        let team_resolved_id: Option<String> = if team.is_some() {
            config.active_profile().team_field_id.clone()
        } else {
            None
        };

        field_resolve::detect_flag_field_overlap(
            &field_spec_map,
            &[
                ("summary", summary.is_some()),
                ("description", description.is_some() || description_stdin),
                ("issuetype", issue_type.is_some()),
                ("priority", priority.is_some()),
                ("components", !components.is_empty()),
                ("labels", !labels.is_empty()),
                ("parent", parent.is_some()),
                ("assignee", to.is_some() || account_id.is_some()),
            ],
            field_resolve::CREATE_D2_GOVERNED_KEYS,
            &[
                ("points", points_resolved_id.as_deref()),
                ("team", team_resolved_id.as_deref()),
            ],
        )?;
    }

    // Resolve project key
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Project key").ok()
            }
        })
        .ok_or_else(|| {
            JrError::UserError(
                "Project key is required. Use --project or configure .jr.toml. \
                 Run \"jr project list\" to see available projects."
                    .into(),
            )
        })?;

    // Resolve issue type
    let issue_type_name = issue_type
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Issue type (e.g., Task, Bug, Story)").ok()
            }
        })
        .ok_or_else(|| JrError::UserError("Issue type is required. Use --type".into()))?;

    // Resolve summary
    let summary_text = summary
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Summary").ok()
            }
        })
        .ok_or_else(|| JrError::UserError("Summary is required. Use --summary".into()))?;

    // Resolve description. spawn_blocking isolates the blocking stdin read
    // from the tokio runtime so later async work isn't starved while waiting
    // on piped input.
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

    // BC-3.4.014: build echo map in parallel with `fields` as each flag is resolved.
    // Emitted after POST 201 in table mode only; JSON path unchanged (AC-015).
    // project is NOT echoed (analogous to not echoing the issue key on edit).
    let mut create_echo: BTreeMap<String, String> = BTreeMap::new();

    // Required fields: always present, always inserted.
    create_echo.insert("issue_type".into(), issue_type_name.clone());
    create_echo.insert("summary".into(), summary_text.clone());

    // Build fields
    let mut fields = json!({
        "project": { "key": project_key },
        "issuetype": { "name": issue_type_name },
        "summary": summary_text,
    });

    if let Some(ref text) = desc_text {
        let adf_body = if markdown {
            adf::markdown_to_adf(text)?
        } else {
            adf::text_to_adf(text)
        };
        fields["description"] = adf_body;
        // BC-3.4.014: table echo uses (updated) marker, same asymmetry as BC-3.4.012.
        // JSON create path is unchanged (AC-015) — no raw desc in JSON output.
        create_echo.insert("description".into(), "(updated)".into());
    }

    if let Some(ref prio) = priority {
        fields["priority"] = json!({ "name": prio });
        create_echo.insert("priority".into(), prio.clone());
    }

    if !labels.is_empty() {
        fields["labels"] = json!(labels);
        // BC-3.4.014: label echo is comma-space joined, command-line order (AC-011).
        create_echo.insert("label".into(), labels.join(", "));
    }

    if !components.is_empty() {
        // BC-3.4.024 Postcondition 1: fields.components = [{"name":"X"},...],
        // CLI input order. NO add:/remove: prefix grammar on create
        // (EC-3.4.024-2) — resolve_create_components resolves each value
        // literally; a stray "add:X" 400s as an unknown component name.
        let resolved = resolve_create_components(client, &project_key, &components).await?;
        fields["components"] = json!(resolved);
    }

    if let Some(ref team_name) = team {
        let (field_id, team_id, resolved_team_name) =
            helpers::resolve_team_field(config, client, team_name, no_input).await?;
        fields[&field_id] = json!(team_id);
        // Echo the RESOLVED display name, not the UUID or partial query (AC-002, BC-3.4.014).
        create_echo.insert("team".into(), resolved_team_name);
    }

    if let Some(pts) = points {
        let field_id = helpers::resolve_story_points_field_id(config)?;
        fields[&field_id] = json!(pts);
        create_echo.insert("points".into(), pts.to_string());
    }

    if let Some(ref parent_key) = parent {
        fields["parent"] = json!({"key": parent_key});
        create_echo.insert("parent".into(), parent_key.clone());
    }

    if let Some(ref id) = account_id {
        fields["assignee"] = json!({"accountId": id});
        // --account-id path: echo the raw account ID string (AC-012).
        create_echo.insert("assignee".into(), id.clone());
    } else if let Some(ref user_query) = to {
        // Rebind _display_name → display_name (AC-012): second tuple element is the
        // display name for both --to NAME and --to me paths (BC-3.4.014, OBS-1).
        let (acct_id, display_name) =
            helpers::resolve_assignee_by_project(client, user_query, &project_key, no_input)
                .await?;
        fields["assignee"] = json!({"accountId": acct_id});
        create_echo.insert("assignee".into(), display_name);
    }

    // Step 4b (S-578-4): --field createmeta field resolution
    // (BC-3.3.010 Steps 1–6, Invariant 1). Runs AFTER project/type resolution
    // and BEFORE the POST. Reuses `get_createmeta_fields` (S-580-1) and
    // `get_issue_types_for_project` (S-331) verbatim (Architecture
    // Compliance Rule 1) — both calls live inside the `FieldMetaSource::Create`
    // branch of `resolve_edit_fields` (`resolve_against_createmeta`).
    if !field_spec_map.is_empty() {
        field_resolve::resolve_edit_fields(
            client,
            &config.active_profile_name,
            field_resolve::FieldMetaSource::Create {
                project_key: &project_key,
                issue_type_name: &issue_type_name,
            },
            &field_spec_map,
            &mut fields,
            &mut create_echo,
            &mut BTreeMap::new(),
        )
        .await?;
    }

    let response = client.create_issue(fields).await?;

    let browse_url = format!(
        "{}/browse/{}",
        client.instance_url().trim_end_matches('/'),
        response.key
    );

    match output_format {
        OutputFormat::Json => {
            // Follow-up GET so the JSON output matches `issue view --output json`
            // (full Issue shape), plus `url`. On GET failure we keep the create
            // succeeding — warn on stderr and fall back to the old `{key, url}`
            // shape so downstream consumers always get at least the key + URL.
            //
            // Pre-existing pattern (same as handle_view, handle_list, project): a CMDB
            // discovery error silently degrades to an empty field list. Tracked as a
            // separate cleanup in the follow-up concerns documented on PR #253 — will
            // be addressed codebase-wide, not per-call-site.
            // AC-015: JSON output path UNCHANGED — no changed_fields key added here.
            let cmdb_fields = get_or_fetch_cmdb_fields(client).await.unwrap_or_default();
            let extra_owned = helpers::compose_extra_fields(config, &cmdb_fields);
            let extra: Vec<&str> = extra_owned.iter().map(String::as_str).collect();

            match client.get_issue(&response.key, &extra).await {
                Ok(issue) => {
                    let mut issue_json = serde_json::to_value(&issue)?;
                    if let Some(obj) = issue_json.as_object_mut() {
                        obj.insert("url".into(), serde_json::Value::String(browse_url.clone()));
                    }
                    println!("{}", output::render_json(&issue_json)?);
                }
                Err(err) => {
                    // Fallback JSON carries a top-level `fetch_error` string so
                    // scripts using `jq '.fields.status.name'` can tell this
                    // shape apart from success without parsing stderr. Recovery
                    // hint points users at `jr issue view` for the full payload.
                    let err_msg = format!("{err}");
                    eprintln!(
                        "warning: issue created ({}) but follow-up fetch failed: {err_msg}. \
                         Run `jr issue view {} --output json` to retrieve the full payload.",
                        response.key, response.key
                    );
                    let mut json_response = serde_json::to_value(&response)?;
                    json_response["url"] = json!(browse_url);
                    json_response["fetch_error"] = json!(err_msg);
                    println!("{}", output::render_json(&json_response)?);
                }
            }
        }
        OutputFormat::Table => {
            // BC-3.4.014: emit confirmation, then field echo lines (alphabetical via BTreeMap),
            // then browse URL. This matches BC-3.4.012's table-mode ordering invariant.
            output::print_success(&format!("Created issue {}", response.key));
            for (field, value) in &create_echo {
                eprintln!("  {} \u{2192} {}", field, value);
            }
            eprintln!("{}", browse_url);
        }
    }

    Ok(())
}

/// Resolve `--component` values into the `fields.components` array for
/// `issue create` (BC-3.4.024 Postcondition 1 / BC-3.4.025).
///
/// NO add:/remove: prefix grammar on create (EC-3.4.024-2, edit-only grammar)
/// — each value in `components` is resolved literally via
/// `helpers::resolve_component` (BC-8.4.001) against the project's component
/// list (`client.list_components(project_key)`, BC-3.4.025 — NEVER editmeta;
/// create's editmeta call is differently-shaped and doesn't cleanly extend to
/// a per-project component list). The list-components GET fires EXACTLY ONCE
/// per invocation regardless of how many `--component` values are supplied
/// (AC-010, VP-COMPONENT-025) — callers must not fetch it again elsewhere in
/// the same command. Unknown name → exit 64, zero POST (AC-008 variant).
///
/// Returns one entry per input value, in CLI input order (AC-007) --
/// `{"name": "<resolved-name>"}` for a NAME input, `{"id": "<n>"}` for a
/// numeric input (Step-4.5 Round 3, F1 fix: BC-8.4.001's numeric bypass
/// means all-ASCII-digit `--component` input is always a component id,
/// never a name -- BC-8.1.008; Jira's issue components field accepts
/// `{"id":...}`). Deliberately NOT confirmed against `GET /component/{id}`
/// before use -- Jira validates the id on the POST itself (an invalid id →
/// Jira 4xx → exit 1, same treatment as an unknown name today). Accepted
/// edge (BC-8.1.008's established gap): a component literally NAMED e.g.
/// `"10001"` is unreachable by name via `--component`.
async fn resolve_create_components(
    client: &JiraClient,
    project_key: &str,
    components: &[String],
) -> Result<Vec<serde_json::Value>> {
    let component_list = client.list_components(project_key).await?;
    let candidate_names: Vec<String> = component_list.iter().map(|c| c.name.clone()).collect();

    let mut resolved: Vec<serde_json::Value> = Vec::with_capacity(components.len());
    for input in components {
        // BC-3.4.024 EC-2: NO add:/remove: prefix stripping on create -- the
        // raw value is resolved LITERALLY against the project component list.
        // F1 fix: the ref kind is determined from the RAW input text, before
        // resolution -- mirrors edit.rs's resolve_component_change_names.
        let ref_kind = format::ComponentRefKind::for_input(input);
        let matched_name = match helpers::resolve_component(input, project_key, &candidate_names) {
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
                    input,
                    candidates.join(", ")
                ))
                .into());
            }
            MatchResult::None(mut available) => {
                available.sort_by_key(|s| s.to_lowercase());
                return Err(JrError::UserError(format!(
                    "Component '{}' not found in project {}. Available: {}.",
                    input,
                    project_key,
                    available.join(", ")
                ))
                .into());
            }
        };
        resolved.push(match ref_kind {
            format::ComponentRefKind::Id => json!({"id": matched_name}),
            format::ComponentRefKind::Name => json!({"name": matched_name}),
        });
    }
    Ok(resolved)
}

/// Parse `--field NAME=VALUE` pairs into a `HashMap<String, String>`.
///
/// Splitting rule (BC-3.8.008): the FIRST `=` in each pair separates name from
/// value. Any subsequent `=` characters are part of the value. Duplicate keys
/// use the last value provided (last-wins). A pair without `=` is a user error
/// (exit 64 via [`JrError::UserError`]).
///
/// # Errors
///
/// Returns `JrError::UserError` if any pair is missing `=`.
/// `--field NAME:kind=VALUE` hint tag (BC-3.4.026 parser contract, step 3).
///
/// Closed set `{option, id, name, asset}` — case-sensitive, lowercase-only
/// (BC-3.4.026 Invariant 3). `None` in [`FieldValueSpec::kind`] represents the
/// bare (unhinted) form, not a fifth variant of this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldValueKind {
    Option,
    Id,
    Name,
    Asset,
}

/// A single `--field` pair, parsed by [`parse_field_kv`] (BC-3.4.026).
///
/// `kind: Some(_)` for a well-formed hinted pair (`NAME:kind=VALUE`);
/// `kind: None` for a well-formed bare pair (`NAME=VALUE`, BC-3.4.015/016
/// auto-detect dispatch, unchanged). `value` is deliberately UNINTERPRETED —
/// `parse_field_kv` does not pre-split `:option`'s cascading `Parent>Child`
/// syntax or `:asset`'s `WORKSPACE:OBJECTID` compact form; those remain
/// call-site concerns (S-578-2/3/4), per ADR-0019 §2 Architecture Compliance
/// Rule 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldValueSpec {
    pub kind: Option<FieldValueKind>,
    pub value: String,
}

/// Parses `--field NAME[:kind]=VALUE` pairs into a `NAME → FieldValueSpec` map
/// (BC-3.4.026, BC-3.4.031).
///
/// Shared verbatim by `create.rs`'s platform path (BC-3.3.010), `edit.rs`
/// (BC-3.4.027-030), and `jsm_create.rs` (BC-3.8.008 amendment) — no
/// per-call-site parsing divergence (BC-3.4.026 Postconditions).
///
/// Parser contract (BC-3.4.026, 5 steps):
/// 1. Split each argument on the FIRST `=` — this splits `NAME[:kind]` from
///    `VALUE` (existing BC-3.8.008 behavior, unchanged).
/// 2. Within `NAME[:kind]`, split on the LAST `:` that appears before the
///    `=`. A field NAME may legitimately contain a colon (e.g. a custom
///    field literally named `"Region: EMEA"`); a real kind tag is always the
///    short, rightmost segment before `=`.
/// 3. If a `:`-delimited segment is found: validate it against the CLOSED set
///    `{option, id, name, asset}` (case-sensitive, lowercase only —
///    BC-3.4.026 Invariant 3). Unknown kind → exit 64 (BC-3.4.031 EC-1),
///    including the empty-segment case (BC-3.4.031 EC-5). Known kind → the
///    pair carries `Some(kind)`.
/// 4. No `:`-delimited segment found before `=` → `kind: None` (bare form —
///    UNCHANGED BC-3.4.015/016 auto-detect dispatch).
/// 5. **Multibyte-safety MUST**: all splitting in steps 1-2 operates on
///    Unicode scalar boundaries (`char_indices`/`str::find(char)`), NEVER
///    raw byte-index slicing — the FIX-F6-LRE-1 class (#734,
///    `jql::validate_duration`). See VP-578-005 /
///    `prop_field_hint_split_no_panic`.
///
/// The map key is ALWAYS the bare field name (ADR-0019 §2(b), normative) —
/// never a composite `"name:kind"` key. Last-wins on duplicate NAME across
/// kind boundaries (VP-578-006): the whole `FieldValueSpec` of the last
/// occurrence overwrites any prior entry for the same NAME.
///
/// `parse_field_kv` MUST stay pure — no HTTP, no cache access, no
/// `get_or_fetch_workspace_id` call (ADR-0019 §2 Architecture Compliance
/// Rule 1); workspace-id resolution for `:asset` is a call-site (S-578-2/3/4)
/// concern.
pub(crate) fn parse_field_kv(pairs: &[String]) -> Result<HashMap<String, FieldValueSpec>, JrError> {
    let mut map = HashMap::new();
    for pair in pairs {
        // Step 1 (unchanged BC-3.8.008 behavior): split on the FIRST '=' —
        // Unicode-scalar-safe via str::find(char), never a raw byte index.
        let Some(eq_idx) = pair.find('=') else {
            return Err(JrError::UserError(format!(
                "--field \"{pair}\" is not a valid NAME=VALUE pair: missing '='. \
                 Use --field NAME=VALUE (e.g., --field customfield_10200=foo)."
            )));
        };
        let name_part = &pair[..eq_idx];
        let value = &pair[eq_idx + 1..];

        // Step 2: within NAME[:kind], split on the LAST ':' that appears
        // before the '=' — Unicode-scalar-safe via str::rfind(char), never a
        // raw byte index (FIX-F6-LRE-1 class, #734).
        let (name, kind) = match name_part.rfind(':') {
            // Whitespace fallback (PR #739 fresh-eyes review, Blocker 1):
            // none of the four valid kind tags (`option`, `id`, `name`,
            // `asset`) contain ASCII whitespace, so a non-empty candidate
            // segment that DOES contain whitespace can never be a real kind
            // tag — it is ordinary name text after a colon that happens to
            // be followed by a space (the canonical example: a field
            // literally named "Region: EMEA"). Treat this exactly like "no
            // ':' found before '='" (step 4 below): `kind: None`, and the
            // FULL original `name_part` (colon included) is the field name.
            // This is additive — it only changes behavior for candidate
            // segments that contain whitespace; an empty segment (EC-5) or
            // a whitespace-free-but-invalid segment (EC-1/EC-7, e.g.
            // "bogus") still fall through to the unknown-kind error below.
            Some(colon_idx)
                if name_part[colon_idx + 1..]
                    .chars()
                    .any(|c| c.is_whitespace()) =>
            {
                (name_part, None)
            }
            Some(colon_idx) => {
                let candidate_name = &name_part[..colon_idx];
                let candidate_kind = &name_part[colon_idx + 1..];
                // Step 3: validate the candidate segment against the closed
                // set (case-sensitive, lowercase only — BC-3.4.026
                // Invariant 3). Unknown kind, including the empty-segment
                // case (BC-3.4.031 EC-5), exits 64.
                let kind = match candidate_kind {
                    "option" => FieldValueKind::Option,
                    "id" => FieldValueKind::Id,
                    "name" => FieldValueKind::Name,
                    "asset" => FieldValueKind::Asset,
                    _ => {
                        return Err(JrError::UserError(format!(
                            "Invalid --field value '{pair}': unknown field-value kind \
                             '{candidate_kind}' — valid kinds are: option, id, name, asset"
                        )));
                    }
                };
                (candidate_name, Some(kind))
            }
            // Step 4: no ':' found before '=' -> bare form (unchanged
            // BC-3.4.015/016 auto-detect dispatch).
            None => (name_part, None),
        };

        map.insert(
            name.to_string(),
            FieldValueSpec {
                kind,
                value: value.to_string(),
            },
        );
    }
    Ok(map)
}

/// S-578-1 tests for the `--field NAME:kind=VALUE` hint-tag parser
/// (BC-3.4.026 parser contract, BC-3.4.031 malformed-hint catalog).
///
/// These tests exercise the `:kind` tag parsing behavior implemented in Step
/// 3 of the BC-3.4.026 parser contract (last ':' before '=' split, closed-set
/// kind validation, bare-form fallback). Prior to Step 3, `parse_field_kv`
/// was a bare-form-only stub (always `kind: None`, no ':kind' suffix
/// stripping); every test below that supplies a `:kind`-tagged pair failed
/// cleanly against that stub (e.g. `assert_eq!` left=`None`), never via panic
/// or a build error.
#[cfg(test)]
mod field_value_kind_tests {
    use super::{FieldValueKind, FieldValueSpec, parse_field_kv};
    use crate::error::JrError;

    /// AC-001 (BC-3.4.026 Return-type change / Postconditions): a well-formed
    /// hinted pair produces `FieldValueSpec { kind: Some(_), value }` — never
    /// `kind: None`. Red Gate: the current stub always sets `kind: None`
    /// regardless of a `:kind` suffix, so this fails with
    /// `assert_eq!` left=`FieldValueSpec { kind: None, .. }` until Task 3
    /// implements the parser contract's steps 2-4.
    #[test]
    fn test_bc_3_4_026_parse_field_kv_returns_field_value_spec_map() {
        let pairs = vec!["cf:option=High".to_string()];
        let result = parse_field_kv(&pairs).expect("well-formed hinted pair must parse");
        assert_eq!(
            result.get("cf"),
            Some(&FieldValueSpec {
                kind: Some(FieldValueKind::Option),
                value: "High".to_string(),
            }),
            "AC-001: hinted pair must produce kind: Some(Option) under the bare map key 'cf'"
        );
    }

    /// AC-002 (BC-3.4.026 parser contract steps 1-3): the first '=' splits
    /// `NAME[:kind]` from `VALUE`; the segment after the last ':' before that
    /// '=' is validated as the kind tag and stripped from the map key — the
    /// key is NEVER a composite `"name:kind"` string (ADR-0019 §2(b)).
    #[test]
    fn test_bc_3_4_026_first_equals_then_last_colon_split() {
        let pairs = vec!["cf:id=10042".to_string()];
        let result = parse_field_kv(&pairs).expect("well-formed hinted pair must parse");
        assert_eq!(
            result.get("cf"),
            Some(&FieldValueSpec {
                kind: Some(FieldValueKind::Id),
                value: "10042".to_string(),
            }),
            "AC-002: 'cf:id=10042' must split into name 'cf', kind Id, value '10042'"
        );
        assert!(
            !result.contains_key("cf:id"),
            "AC-002/BC-3.4.026 Rule (ADR-0019 §2(b)): map key must never be composite 'name:kind'"
        );
    }

    /// AC-002 / VP-578-005 (BC-3.4.026 step 2, colon-in-NAME case flagged in
    /// the verification delta's coverage note): a field NAME may legitimately
    /// contain a colon (e.g. a custom field literally named "Region: EMEA").
    /// The parser MUST split on the LAST ':' before the '=', not the first,
    /// so the multi-colon NAME survives intact and only the short trailing
    /// segment is read as the candidate kind.
    #[test]
    fn test_bc_3_4_026_multi_colon_name_isolates_kind_from_last_colon() {
        let pairs = vec!["Region: EMEA:option=X".to_string()];
        let result =
            parse_field_kv(&pairs).expect("multi-colon NAME with a valid trailing kind must parse");
        assert_eq!(
            result.get("Region: EMEA"),
            Some(&FieldValueSpec {
                kind: Some(FieldValueKind::Option),
                value: "X".to_string(),
            }),
            "VP-578-005: last-':'-before-'=' split must isolate name 'Region: EMEA' \
             (internal colon preserved verbatim) and kind Option"
        );
    }

    /// Regression pin: the bare (unhinted) `NAME=VALUE` form is UNCHANGED —
    /// `kind: None`, name used verbatim, no stripping. This must already pass
    /// against the Step-2 stub (BC-3.4.026 Invariant 1, "bare form is
    /// permanent, not deprecated").
    #[test]
    fn test_bc_3_4_026_bare_form_no_kind_tag_unchanged() {
        let pairs = vec!["summary=New title".to_string()];
        let result = parse_field_kv(&pairs).expect("bare NAME=VALUE must parse");
        assert_eq!(
            result.get("summary"),
            Some(&FieldValueSpec {
                kind: None,
                value: "New title".to_string(),
            }),
            "Bare form regression pin: no ':kind' suffix must yield kind: None, name unchanged"
        );
    }

    /// AC-004 / VP-578-006 (BC-3.4.026 Rule, ADR-0019 §2(b)): the map key is
    /// ALWAYS the bare field name. Two occurrences of the same NAME with
    /// DIFFERENT kinds must collapse into exactly ONE map entry, and the
    /// LAST occurrence's whole `FieldValueSpec` (kind AND value) wins —
    /// kinds are never merged or compared across duplicate NAME occurrences.
    #[test]
    fn test_bc_3_4_026_last_wins_across_kinds_single_map_entry() {
        let pairs = vec!["cf:option=A".to_string(), "cf:id=B".to_string()];
        let result = parse_field_kv(&pairs).expect("repeated NAME with differing kinds must parse");
        assert_eq!(
            result.len(),
            1,
            "VP-578-006: duplicate NAME across kinds must collapse to exactly one map entry \
             (never a composite 'cf:option' + 'cf:id' pair of entries)"
        );
        assert_eq!(
            result.get("cf"),
            Some(&FieldValueSpec {
                kind: Some(FieldValueKind::Id),
                value: "B".to_string(),
            }),
            "VP-578-006: the LAST occurrence's whole FieldValueSpec (kind AND value) must win"
        );
    }

    /// AC-006 / BC-3.4.031 EC-1: an unknown kind tag exits 64
    /// (`JrError::UserError`) and the message lists the four valid kinds.
    /// Load-bearing substring: "unknown field-value kind".
    #[test]
    fn test_bc_3_4_031_ec1_unknown_kind_exits_64() {
        let pairs = vec!["cf:bogus=X".to_string()];
        let err = parse_field_kv(&pairs).expect_err("unknown kind must be rejected");
        assert_eq!(
            err.exit_code(),
            64,
            "EC-1: unknown kind must map to exit code 64"
        );
        if let JrError::UserError(msg) = &err {
            assert!(
                msg.contains("unknown field-value kind"),
                "EC-1: message must contain the load-bearing substring \
                 'unknown field-value kind', got: {msg}"
            );
            for kind in ["option", "id", "name", "asset"] {
                assert!(
                    msg.contains(kind),
                    "EC-1: message must list the valid kind '{kind}', got: {msg}"
                );
            }
        } else {
            panic!("EC-1: expected JrError::UserError, got: {err:?}");
        }
    }

    /// AC-007 / BC-3.4.031 EC-5: an empty `:kind` segment (`cf:=VALUE`) is
    /// treated as EC-1 (unknown kind — the empty string is not in the closed
    /// set `{option, id, name, asset}`) → exit 64 with the same
    /// four-valid-kinds message.
    #[test]
    fn test_bc_3_4_031_ec5_empty_kind_segment_treated_as_unknown_kind() {
        let pairs = vec!["cf:=VALUE".to_string()];
        let err = parse_field_kv(&pairs).expect_err("empty ':kind' segment must be rejected");
        assert_eq!(
            err.exit_code(),
            64,
            "EC-5: empty kind segment must map to exit code 64"
        );
        if let JrError::UserError(msg) = &err {
            assert!(
                msg.contains("unknown field-value kind"),
                "EC-5: empty kind segment must fire the SAME unknown-kind message as EC-1, got: {msg}"
            );
        } else {
            panic!("EC-5: expected JrError::UserError, got: {err:?}");
        }
    }

    /// AC-008 / BC-3.4.031 EC-6 (regression pin): a ':' appearing in VALUE
    /// (after the first '=') is NOT reinterpreted as a nested hint — the
    /// step-1 split on '=' happens before the step-2 ':kind' split, and step
    /// 2 only ever inspects the pre-'=' portion. This MUST resolve normally,
    /// not error.
    #[test]
    fn test_bc_3_4_031_ec6_colon_in_value_resolves_normally() {
        let pairs = vec!["cf:option=High:Priority".to_string()];
        let result = parse_field_kv(&pairs)
            .expect("EC-6 regression pin: a colon in VALUE (after '=') must resolve normally");
        assert_eq!(
            result.get("cf"),
            Some(&FieldValueSpec {
                kind: Some(FieldValueKind::Option),
                value: "High:Priority".to_string(),
            }),
            "EC-6: VALUE must be 'High:Priority' verbatim, kind must be Option — \
             the ':' after '=' is never treated as a second hint delimiter"
        );
    }

    /// AC-008 / BC-3.4.031 EC-7 (regression pin): multiple ':' in the NAME
    /// segment before '=', where the last-colon split isolates a segment
    /// that is NOT a valid kind, must fire the SPECIFIC unknown-kind error
    /// (EC-1) — not a different, wrong error (e.g. not silently treated as a
    /// bare NAME containing colons).
    #[test]
    fn test_bc_3_4_031_ec7_multi_colon_name_fires_unknown_kind_not_other_error() {
        let pairs = vec!["Region: EMEA:bogus=X".to_string()];
        let err = parse_field_kv(&pairs).expect_err(
            "EC-7 regression pin: last-colon split isolating an invalid kind must error",
        );
        assert_eq!(err.exit_code(), 64, "EC-7: must map to exit code 64");
        if let JrError::UserError(msg) = &err {
            assert!(
                msg.contains("unknown field-value kind"),
                "EC-7: must fire the SPECIFIC unknown-kind (EC-1) message, not a different error, got: {msg}"
            );
        } else {
            panic!("EC-7: expected JrError::UserError, got: {err:?}");
        }
    }

    /// AC-009 / BC-3.4.031 EC-8: an empty `:id` value (`cf:id=`) is a
    /// PASS-THROUGH at the parser level, NOT a `jr`-side exit-64 —
    /// `parse_field_kv` performs no empty-value rejection for `:id`; the pair
    /// carries `FieldValueSpec { kind: Some(Id), value: "" }` unchanged.
    #[test]
    fn test_bc_3_4_031_ec8_empty_id_value_passes_through_parser() {
        let pairs = vec!["cf:id=".to_string()];
        let result = parse_field_kv(&pairs)
            .expect("EC-8: empty ':id' value must PASS THROUGH the parser (not exit 64)");
        assert_eq!(
            result.get("cf"),
            Some(&FieldValueSpec {
                kind: Some(FieldValueKind::Id),
                value: String::new(),
            }),
            "EC-8: empty ':id' value must parse to kind Some(Id), value \"\" — \
             the server is the sole validator of an empty id (BC-3.4.028 Invariant 1)"
        );
    }

    /// AC-009 / BC-3.4.031 EC-9: an empty `:name` value (`cf:name=`) is a
    /// PASS-THROUGH at the parser level, identically to EC-8's `:id` case.
    #[test]
    fn test_bc_3_4_031_ec9_empty_name_value_passes_through_parser() {
        let pairs = vec!["cf:name=".to_string()];
        let result = parse_field_kv(&pairs)
            .expect("EC-9: empty ':name' value must PASS THROUGH the parser (not exit 64)");
        assert_eq!(
            result.get("cf"),
            Some(&FieldValueSpec {
                kind: Some(FieldValueKind::Name),
                value: String::new(),
            }),
            "EC-9: empty ':name' value must parse to kind Some(Name), value \"\" — \
             the server is the sole validator of an empty name (BC-3.4.029 Invariant 1)"
        );
    }

    /// BC-3.4.031 EC-2a, PARSER-LEVEL scope (AC-009 note + Architecture
    /// Compliance Rule 1/2): `:asset`'s empty-value exit-64 IS real (BC-3.4.031
    /// EC-2a), but it fires at the CALL-SITE composer (S-578-2/3/4 scope) —
    /// never inside `parse_field_kv` itself, per the story's explicit
    /// boundary ("Only `:asset`'s EC-2a is a `jr`-side exit-64 for empty
    /// value, and that check lives at the CALL SITE composer... never inside
    /// `parse_field_kv` itself"). `parse_field_kv` MUST stay pure (no HTTP, no
    /// structural array composition) and MUST NOT pre-validate `:asset`'s
    /// `WORKSPACE:OBJECTID` shape (Architecture Compliance Rule 2: the value
    /// is deliberately UNINTERPRETED). At THIS layer, an empty `:asset=`
    /// value therefore parses successfully, exactly like `:id`/`:name`.
    #[test]
    fn test_bc_3_4_031_ec2a_empty_asset_value_passes_through_parser_level() {
        let pairs = vec!["cf:asset=".to_string()];
        let result = parse_field_kv(&pairs).expect(
            "EC-2a at the PARSER level: parse_field_kv must not itself reject an empty \
             ':asset' value — the structural exit-64 belongs to the call-site composer \
             (S-578-2/3/4), out of this story's scope (AC-009)",
        );
        assert_eq!(
            result.get("cf"),
            Some(&FieldValueSpec {
                kind: Some(FieldValueKind::Asset),
                value: String::new(),
            }),
            "EC-2a (parser scope): empty ':asset' value must parse to kind Some(Asset), value \"\""
        );
    }

    /// AC-010 (BC-3.4.026 Invariant 3): kind validation is case-sensitive,
    /// lowercase-only. `:Option=`/`:OPTION=`/etc. are NOT recognized as the
    /// `option` kind — they fall through to the unknown-kind exit-64 path
    /// (EC-1), never silently treated as bare NAME text containing a colon.
    #[test]
    fn test_bc_3_4_026_kind_validation_case_sensitive_lowercase_only() {
        for variant in [
            "cf:Option=X",
            "cf:OPTION=X",
            "cf:Id=Y",
            "cf:ASSET=Z",
            "cf:Name=W",
        ] {
            let pairs = vec![variant.to_string()];
            let err = parse_field_kv(&pairs).expect_err(&format!(
                "AC-010: '{variant}' (mixed/upper-case kind) must be rejected"
            ));
            assert_eq!(
                err.exit_code(),
                64,
                "AC-010: '{variant}' must map to exit code 64"
            );
            if let JrError::UserError(msg) = &err {
                assert!(
                    msg.contains("unknown field-value kind"),
                    "AC-010: '{variant}' must fire the SAME unknown-kind message as a genuinely \
                     unknown kind (deliberate strictness — typos fail loud), got: {msg}"
                );
            } else {
                panic!("AC-010: '{variant}' expected JrError::UserError, got: {err:?}");
            }
        }
    }

    /// AC-003 / VP-578-005 regression pins: concrete multibyte inputs at the
    /// ':'/'=' split boundaries must never panic (the FIX-F6-LRE-1 class,
    /// #734, `jql::validate_duration`'s multibyte byte-index panic). Ok or
    /// Err is both acceptable; only a panic is forbidden.
    #[test]
    fn test_field_hint_multibyte_kind_and_value_no_panic() {
        for raw in ["cf:optioné=x", "世=界", "a:asset=W:🦀"] {
            let pairs = vec![raw.to_string()];
            let _ = parse_field_kv(&pairs); // must not panic
        }
    }

    /// S-578-1 LOW-finding remediation — completes the empty-value pass-through
    /// matrix. AC-009 (BC-3.4.031 EC completeness): an empty `:option=` value
    /// is a PASS-THROUGH at the parser level, identically to EC-8 (`:id=`),
    /// EC-9 (`:name=`), and EC-2a (`:asset=`) above —
    /// `parse_field_kv` performs no empty-value rejection for `:option`
    /// either; the pair carries `FieldValueSpec { kind: Some(Option), value: "" }`
    /// unchanged. The `is_err`-style structural classification of `:option`'s
    /// cascading `Parent>Child` syntax (whether an empty value is actually
    /// invalid) is a downstream call-site composer concern (S-578-2/3/4),
    /// out of this story's scope — this test only pins the PARSER-level
    /// pass-through.
    #[test]
    fn test_bc_3_4_031_option_empty_value_passes_through_parser() {
        let pairs = vec!["cf:option=".to_string()];
        let result = parse_field_kv(&pairs)
            .expect("S-578-1: empty ':option' value must PASS THROUGH the parser (not exit 64)");
        assert_eq!(
            result.get("cf"),
            Some(&FieldValueSpec {
                kind: Some(FieldValueKind::Option),
                value: String::new(),
            }),
            "S-578-1: empty ':option' value must parse to kind Some(Option), value \"\" — \
             completes the empty-value matrix alongside :id/:name/:asset (EC-8/9/2a)"
        );
    }

    /// PR #739 fresh-eyes review, Blocker 1 (regression vs `develop`): a
    /// field NAME containing a colon FOLLOWED BY WHITESPACE (e.g. the
    /// canonical `"Region: EMEA"` example BC-3.4.026's own rationale text
    /// cites) must keep working exactly as it did before this story's
    /// `:kind`-tag parser landed. None of the four valid kind tags
    /// (`option`, `id`, `name`, `asset`) contain ASCII whitespace, so a
    /// candidate segment with whitespace can never be a real kind tag —
    /// the parser must fall through to the bare-form branch (`kind: None`)
    /// using the WHOLE original `NAME[:kind]` text (colon included) as the
    /// field name, not just the portion before that colon.
    #[test]
    fn test_bc_3_4_026_colon_in_name_with_whitespace_after_falls_back_to_bare_form() {
        let pairs = vec!["Region: EMEA=x".to_string()];
        let result = parse_field_kv(&pairs)
            .expect("colon-in-name-with-trailing-whitespace must parse as bare form, not error");
        assert_eq!(result.len(), 1, "must produce exactly one map entry");
        assert_eq!(
            result.get("Region: EMEA"),
            Some(&FieldValueSpec {
                kind: None,
                value: "x".to_string(),
            }),
            "regression pin: 'Region: EMEA=x' must parse to name 'Region: EMEA' \
             (colon preserved verbatim), kind: None, value 'x' — matching develop's \
             pre-existing first-'='-only split behavior"
        );
    }

    /// PR #739 fresh-eyes review, Blocker 2: the missing-'=' error message
    /// must include actionable guidance per CLAUDE.md's "Errors: Always
    /// suggest what to do next" convention — a concrete `--field NAME=VALUE`
    /// example, not just a bare restatement of the expected shape.
    #[test]
    fn test_bc_3_8_008_missing_equals_error_includes_actionable_guidance() {
        let pairs = vec!["noequalssign".to_string()];
        let err = parse_field_kv(&pairs).expect_err("a pair with no '=' must be rejected");
        assert_eq!(err.exit_code(), 64, "missing '=' must map to exit code 64");
        if let JrError::UserError(msg) = &err {
            assert!(
                msg.contains("--field NAME=VALUE"),
                "message must contain actionable '--field NAME=VALUE' guidance, got: {msg}"
            );
            assert!(
                msg.contains("customfield_10200=foo"),
                "message must contain the concrete example 'customfield_10200=foo', got: {msg}"
            );
        } else {
            panic!("expected JrError::UserError, got: {err:?}");
        }
    }
}

/// Proptest properties for `parse_field_kv` (AC-013, BC-3.8.008).
///
/// Properties A.1–A.4 cover the four invariants stated in the verification delta.
#[cfg(test)]
mod parse_field_kv_proptests {
    use super::{FieldValueKind, FieldValueSpec, parse_field_kv};
    use proptest::prelude::*;

    proptest! {
        /// A.1 (BC-3.8.008): first `=` is the delimiter; subsequent `=` chars
        /// are part of the value. For any valid NAME and VALUE (which may contain
        /// `=` chars), round-tripping through parse_field_kv preserves the value.
        #[test]
        fn prop_parse_field_kv_first_equals_split(
            name in "[a-z][a-z0-9_]{0,19}",
            value_prefix in "[a-z]{1,10}",
            value_suffix in "[=a-z0-9]{0,10}",
        ) {
            let pair = format!("{name}={value_prefix}={value_suffix}");
            let pairs = vec![pair];
            let result = parse_field_kv(&pairs)
                .unwrap_or_else(|e| panic!("A.1: parse_field_kv must succeed for valid pair; got error: {e:?}"));
            let expected_value = format!("{value_prefix}={value_suffix}");
            prop_assert_eq!(
                result.get(&name).map(|spec| spec.value.as_str()),
                Some(expected_value.as_str()),
                "A.1: BC-3.8.008 first-equals split must yield full value after first '='"
            );
        }

        /// A.2 (BC-3.8.008): empty value is allowed. `key=` produces `{"key": ""}`.
        #[test]
        fn prop_parse_field_kv_empty_value_allowed(
            name in "[a-z][a-z0-9_]{0,19}",
        ) {
            let pair = format!("{name}=");
            let pairs = vec![pair];
            let result = parse_field_kv(&pairs)
                .unwrap_or_else(|e| panic!("A.2: parse_field_kv must accept 'name=' (empty value); got error: {e:?}"));
            prop_assert_eq!(
                result.get(&name).map(|spec| spec.value.as_str()),
                Some(""),
                "A.2: BC-3.8.008 empty value after '=' must be accepted and preserved"
            );
        }

        /// A.3 (BC-3.8.008): duplicate key — last value wins.
        /// Two pairs with the same key must result in only the second value.
        #[test]
        fn prop_parse_field_kv_last_value_wins_on_duplicates(
            name in "[a-z][a-z0-9_]{0,19}",
            first_val in "[a-z]{1,10}",
            last_val in "[a-z]{1,10}",
        ) {
            let pairs = vec![
                format!("{name}={first_val}"),
                format!("{name}={last_val}"),
            ];
            let result = parse_field_kv(&pairs)
                .unwrap_or_else(|e| panic!("A.3: parse_field_kv must succeed for duplicate key pairs; got error: {e:?}"));
            prop_assert_eq!(
                result.get(&name).map(|spec| spec.value.as_str()),
                Some(last_val.as_str()),
                "A.3: BC-3.8.008 duplicate key: last value must win"
            );
            prop_assert_eq!(
                result.len(),
                1,
                "A.3: BC-3.8.008 duplicate keys must collapse to one entry"
            );
        }

        /// A.4 (BC-3.8.008): no panic on arbitrary input — any string that
        /// contains at least one `=` must parse without panic (may return Ok or Err).
        #[test]
        fn prop_parse_field_kv_no_panic_on_arbitrary_input(
            raw in ".{0,80}",
        ) {
            // The function contract: no panic for any input.
            // Ok or Err is both acceptable; only panics are forbidden.
            let pairs = vec![raw];
            let _ = parse_field_kv(&pairs); // must not panic
        }

        /// AC-003 / VP-578-005 (BC-3.4.026 step 5, Multibyte-safety MUST): the
        /// hint-tag splitter must never panic on arbitrary UTF-8 input,
        /// including multibyte scalars landing adjacent to the new `:`/`='
        /// split points the S-578-1 kind-tag parser introduces (the
        /// FIX-F6-LRE-1 class, #734, `jql::validate_duration`'s byte-index
        /// panic on multibyte input). `\PC` generates arbitrary Unicode
        /// scalar values, including multibyte ones.
        #[test]
        fn prop_field_hint_split_no_panic(raw in "\\PC{0,80}") {
            let _ = parse_field_kv(&[raw]); // must not panic
        }

        /// VP-578-006 (BC-3.4.026 Rule, ADR-0019 §2(b)), PROPTEST form —
        /// sibling to `prop_parse_field_kv_last_value_wins_on_duplicates`
        /// (A.3, bare-form only). For a single NAME with N>=2 repeated
        /// `--field NAME[:kind]=VALUE` occurrences, where BOTH the `:kind`
        /// hint AND the value vary independently across occurrences,
        /// `parse_field_kv` must yield EXACTLY ONE map entry for that NAME,
        /// equal to the LAST occurrence's whole `FieldValueSpec` (kind AND
        /// value) — kinds are never merged or compared across duplicate NAME
        /// occurrences, only the last one survives.
        #[test]
        fn prop_field_kv_last_wins_across_kinds(
            name in "[a-z][a-z0-9_]{0,19}",
            occurrences in prop::collection::vec(
                (
                    prop_oneof![
                        Just(None),
                        Just(Some("option")),
                        Just(Some("id")),
                        Just(Some("name")),
                        Just(Some("asset")),
                    ],
                    "[a-z0-9]{0,10}",
                ),
                2..6,
            ),
        ) {
            let pairs: Vec<String> = occurrences
                .iter()
                .map(|(kind, value): &(Option<&str>, String)| match kind {
                    Some(k) => format!("{name}:{k}={value}"),
                    None => format!("{name}={value}"),
                })
                .collect();

            let result = parse_field_kv(&pairs).unwrap_or_else(|e| {
                panic!(
                    "VP-578-006: parse_field_kv must succeed for well-formed duplicate-NAME \
                     pairs across kinds; got error: {e:?}"
                )
            });

            prop_assert_eq!(
                result.len(),
                1,
                "VP-578-006: duplicate NAME across kinds must collapse to exactly one map entry"
            );

            let (last_kind_str, last_value) = occurrences.last().expect("2..6 is never empty");
            let expected_kind = last_kind_str.map(|k| match k {
                "option" => FieldValueKind::Option,
                "id" => FieldValueKind::Id,
                "name" => FieldValueKind::Name,
                "asset" => FieldValueKind::Asset,
                _ => unreachable!("strategy only generates the four known kind strings"),
            });

            prop_assert_eq!(
                result.get(&name),
                Some(&FieldValueSpec {
                    kind: expected_kind,
                    value: last_value.clone(),
                }),
                "VP-578-006: the LAST occurrence's whole FieldValueSpec (kind AND value) must \
                 win across kind boundaries, not just across value boundaries"
            );
        }

        /// VP-578-005 property 3 (BC-3.4.026 step 5, Multibyte-safety MUST):
        /// the parsed `value` is byte-for-byte the substring after the FIRST
        /// '=', including embedded '=', ':', and MULTIBYTE scalars — never
        /// re-encoded, trimmed, or otherwise altered. `\PC` generates
        /// arbitrary Unicode scalar values, including multibyte ones. This
        /// closes the verification-delta gap where multibyte VALUE
        /// preservation was previously only no-panic-checked
        /// (`prop_field_hint_split_no_panic`) but never asserted equal to
        /// the exact expected substring.
        #[test]
        fn prop_field_hint_value_bytes_preserved(
            name in "[a-z][a-z0-9_]{0,19}",
            value in "\\PC{0,80}",
        ) {
            let pair = format!("{name}={value}");
            let pairs = vec![pair.clone()];
            let result = parse_field_kv(&pairs).unwrap_or_else(|e| {
                panic!(
                    "VP-578-005 property 3: parse_field_kv must succeed for a bare \
                     NAME=VALUE pair; got error: {e:?}"
                )
            });

            // The substring after the FIRST '=' — since `name` is pure
            // `[a-z][a-z0-9_]*` (no '=' or ':'), this is unambiguous and
            // equals `value` byte-for-byte even when `value` itself embeds
            // '=' or ':' characters.
            let expected_value = &pair[name.len() + 1..];
            prop_assert_eq!(
                expected_value,
                value.as_str(),
                "sanity: substring-after-first-'=' must equal the original value verbatim"
            );

            prop_assert_eq!(
                result.get(&name).map(|spec| spec.value.as_str()),
                Some(expected_value),
                "VP-578-005 property 3: parsed value must be byte-for-byte the substring after \
                 the first '=', including embedded '=', ':', and multibyte scalars"
            );
        }
    }
}
