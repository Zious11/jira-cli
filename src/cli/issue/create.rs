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

    // Pre-flight guard (DEC-188, BC-3.8.012, BC-3.8.013): --field and
    // --on-behalf-of are self-declared JSM-only flags. On the platform path
    // (--request-type absent — this arm only runs when that fork above was
    // NOT taken), supplying either flag is a categorical user error, not an
    // ambiguous choice. Exit 64 BEFORE project-key resolution, BEFORE any
    // interactive prompt, BEFORE the blocking --description-stdin read, and
    // BEFORE any HTTP call. Combined check fires first so both flags produce
    // ONE error, not two. Presence-only (`!field_pairs.is_empty()` /
    // `on_behalf_of.is_some()`) — malformed/empty values still trip the guard,
    // and repeated --field occurrences still yield exactly one error.
    //
    // MUST NOT be implemented via `#[arg(requires = "request_type")]` — that
    // yields clap exit 2, not the exit-64 JrError::UserError BC-3.8.012/013
    // require.
    if !field_pairs.is_empty() && on_behalf_of.is_some() {
        return Err(JrError::UserError(
            "--field and --on-behalf-of are only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to use these flags, or drop them to create a standard platform issue."
                .into(),
        )
        .into());
    }
    if !field_pairs.is_empty() {
        return Err(JrError::UserError(
            "--field is only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to submit a JSM request with custom fields, or drop --field to create a standard platform issue."
                .into(),
        )
        .into());
    }
    if on_behalf_of.is_some() {
        return Err(JrError::UserError(
            "--on-behalf-of is only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to raise a request on behalf of another user, or drop --on-behalf-of to create a standard platform issue."
                .into(),
        )
        .into());
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
pub(crate) fn parse_field_kv(pairs: &[String]) -> Result<HashMap<String, String>, JrError> {
    let mut map = HashMap::new();
    for pair in pairs {
        let Some(eq_pos) = pair.find('=') else {
            return Err(JrError::UserError(format!(
                "--field \"{pair}\" is not a valid NAME=VALUE pair: missing '='. \
                 Use --field NAME=VALUE (e.g., --field customfield_10200=foo)."
            )));
        };
        let key = pair[..eq_pos].to_string();
        let value = pair[eq_pos + 1..].to_string();
        // Last-wins for duplicate keys (BC-3.8.008).
        map.insert(key, value);
    }
    Ok(map)
}

/// Proptest properties for `parse_field_kv` (AC-013, BC-3.8.008).
///
/// Properties A.1–A.4 cover the four invariants stated in the verification delta.
#[cfg(test)]
mod parse_field_kv_proptests {
    use super::parse_field_kv;
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
                result.get(&name).map(String::as_str),
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
                result.get(&name).map(String::as_str),
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
                result.get(&name).map(String::as_str),
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
    }
}
